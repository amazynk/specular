//! Gain maps: Ultra HDR / Adobe "hdrgm" JPEG and ISO 21496-1.
//!
//! A gain-map JPEG is an ordinary (SDR) JPEG followed by a second, usually smaller JPEG — the
//! gain map — located through the primary's MPF (CIPA DC-007) index, the XMP
//! `Container:Directory`, or simply the first `FFD8 FF` after the primary's end. The metadata
//! (how to turn the map into per-pixel gains) is in the map's ISO 21496-1 APP2 segment or in
//! its XMP (`hdrgm:` namespace, Adobe "Gain Map" 1.0 / Android Ultra HDR 1.0).
//!
//! [`parse_iso21496`] is the format-independent binary parser: AVIF (`tmap` item) and HEIC
//! files carry the same payload.

use specular_core::{
    CancelToken, GainMap, GainMapParams, PixelLayout, Primaries, Raster, RasterPlane,
};
use specular_jpeg as sj;

/// Largest gain map we decode (each side).
pub(crate) const MAX_MAP_SIDE: u32 = 16_384;

const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const ISO_SIG: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";
const MPF_SIG: &[u8] = b"MPF\0";

/// Segments of interest in one JPEG's header (up to the first SOS).
#[derive(Default, Debug)]
struct Header<'a> {
    xmp: Option<&'a [u8]>,
    /// The TIFF payload of the `Exif` APP1 segment.
    exif: Option<&'a [u8]>,
    iso: Option<&'a [u8]>,
    /// A single-segment ICC profile.
    icc: Option<&'a [u8]>,
    /// MPF payload (after the signature) and its absolute offset in the file.
    mpf: Option<(&'a [u8], usize)>,
    /// Offset of the first SOS marker.
    sos: usize,
}

fn read_header(data: &[u8], base: usize) -> Option<Header<'_>> {
    let d = data.get(base..)?;
    if d.len() < 4 || d[0] != 0xFF || d[1] != 0xD8 {
        return None;
    }
    let mut h = Header::default();
    let mut i = 2;
    while i + 4 <= d.len() {
        if d[i] != 0xFF {
            return None;
        }
        let m = d[i + 1];
        if m == 0xFF {
            i += 1; // fill byte
            continue;
        }
        if m == 0xD8 || (0xD0..=0xD7).contains(&m) || m == 0x01 {
            i += 2;
            continue;
        }
        if m == 0xDA || m == 0xD9 {
            h.sos = base + i;
            return Some(h);
        }
        let len = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
        if len < 2 || i + 2 + len > d.len() {
            return None;
        }
        let body = &d[i + 4..i + 2 + len];
        match m {
            0xE1 if body.starts_with(XMP_SIG) && h.xmp.is_none() => {
                h.xmp = Some(&body[XMP_SIG.len()..]);
            }
            0xE1 if body.starts_with(b"Exif\0\0") && h.exif.is_none() => {
                h.exif = Some(&body[6..]);
            }
            0xE2 if body.starts_with(ISO_SIG) => h.iso = Some(&body[ISO_SIG.len()..]),
            // "ICC_PROFILE\0", chunk 1 of 1.
            0xE2 if body.starts_with(b"ICC_PROFILE\0\x01\x01") => h.icc = Some(&body[14..]),
            0xE2 if body.starts_with(MPF_SIG) => {
                h.mpf = Some((&body[MPF_SIG.len()..], base + i + 4 + MPF_SIG.len()));
            }
            _ => {}
        }
        i += 2 + len;
    }
    None
}

/// Whether a JPEG announces a gain map (cheap: header segments only). Used for the info
/// panel and to skip the search in ordinary files.
pub fn jpeg_has_gain_map(data: &[u8]) -> bool {
    let Some(h) = read_header(data, 0) else {
        return false;
    };
    if h.iso.is_some() || h.xmp.is_some_and(|x| find(x, b"hdrgm:").is_some()) {
        return true;
    }
    // Plain MPF files hold stereo pairs or previews: only a secondary image with gain-map
    // metadata of its own counts.
    let Some((mpf, tiff_start)) = h.mpf else {
        return false;
    };
    mpf_images(mpf).into_iter().skip(1).any(|(offset, _)| {
        read_header(data, tiff_start.saturating_add(offset as usize)).is_some_and(|s| {
            s.iso.is_some()
                || s.xmp
                    .is_some_and(|x| find(x, b"hdrgm:").is_some() || is_apple_map_xmp(x))
        })
    })
}

/// Finds and decodes the gain map of a JPEG. `None` when there is none (or it is unusable).
pub fn jpeg_gain_map(data: &[u8], cancel: &CancelToken) -> Option<GainMap> {
    let primary = read_header(data, 0)?;
    let candidates = secondary_offsets(data, &primary);
    for off in candidates {
        if cancel.is_cancelled() {
            return None;
        }
        let Some(h) = read_header(data, off) else {
            continue;
        };
        let (params, source) = match h.iso.and_then(parse_iso21496) {
            Some(mut p) => {
                // Per-channel gains in the base's colour space (ISO 21496-1
                // `use_base_colour_space`): the primaries of the primary image's profile.
                let flags = h.iso.and_then(|m| m.get(4)).copied().unwrap_or(0);
                if flags & 0xC0 == 0xC0 {
                    p.space = primary
                        .icc
                        .and_then(icc_primaries)
                        .unwrap_or(Primaries::Bt709);
                }
                (p, ISO_SOURCE)
            }
            None => match h.xmp.and_then(parse_hdrgm_xmp) {
                Some(p) => (p, HDRGM_SOURCE),
                None => {
                    if h.xmp.is_some_and(is_apple_map_xmp) {
                        let headroom = apple_headroom(h.xmp, primary.exif)?;
                        let map = decode_map(&data[off..], cancel)?;
                        return apple_gain_map(&map, headroom);
                    }
                    continue;
                }
            },
        };
        if !params.is_sane() {
            return None;
        }
        let map = decode_map(&data[off..], cancel)?;
        return Some(GainMap {
            map,
            params,
            source,
        });
    }
    None
}

/// The primaries of an RGB matrix ICC profile when they are BT.709 / sRGB, Display P3 or
/// BT.2020 (red's x and green's y of its D50 colorants within 0.01); `None` otherwise.
pub fn icc_primaries(icc: &[u8]) -> Option<Primaries> {
    let be =
        |o: usize| -> Option<u32> { Some(u32::from_be_bytes(icc.get(o..o + 4)?.try_into().ok()?)) };
    let n = be(128)? as usize;
    let xyz = |sig: &[u8; 4]| -> Option<[f64; 3]> {
        let e = (0..n.min(256))
            .map(|i| 132 + 12 * i)
            .find(|&e| icc.get(e..e + 4) == Some(sig.as_slice()))?;
        let at = be(e + 4)? as usize;
        if icc.get(at..at + 4)? != b"XYZ " {
            return None;
        }
        let f = |k: usize| be(at + 8 + 4 * k).map(|v| f64::from(v as i32) / 65536.0);
        Some([f(0)?, f(1)?, f(2)?])
    };
    let (r, g) = (xyz(b"rXYZ")?, xyz(b"gXYZ")?);
    let x = |c: [f64; 3]| c[0] / (c[0] + c[1] + c[2]);
    let y = |c: [f64; 3]| c[1] / (c[0] + c[1] + c[2]);
    // Red x, green y of each space's colorants adapted to D50 (Bradford).
    [
        (Primaries::Bt709, 0.6484, 0.5979),
        (Primaries::DisplayP3, 0.6821, 0.6747),
        (Primaries::Bt2020, 0.7085, 0.7754),
    ]
    .into_iter()
    .find(|&(_, rx, gy)| (x(r) - rx).abs() < 0.01 && (y(g) - gy).abs() < 0.01)
    .map(|(p, _, _)| p)
}

/// [`GainMap::source`] of ISO 21496-1 metadata (JPEG APP2, AVIF / HEIF `tmap`).
pub const ISO_SOURCE: &str = "ISO 21496-1";
/// [`GainMap::source`] of Adobe / Ultra HDR `hdrgm:` XMP metadata.
pub const HDRGM_SOURCE: &str = "Ultra HDR";
/// [`GainMap::source`] of an Apple HDR gain map.
pub const APPLE_SOURCE: &str = "Apple";

/// Auxiliary-image type of Apple's HDR gain map (HEIC `auxC`, JPEG XMP).
const APPLE_URN: &[u8] = b"urn:com:apple:photo:2020:aux:hdrgainmap";

/// Whether the XMP of an image describes it as an Apple HDR gain map.
fn is_apple_map_xmp(x: &[u8]) -> bool {
    find(x, APPLE_URN).is_some() || find(x, b"HDRGainMap:HDRGainMapVersion").is_some()
}

/// The headroom (linear, > 1) an Apple HDR gain map is meant for: `HDRGainMapHeadroom` in
/// the map's own XMP (gain map version 2, iOS 18), else derived from the primary image's
/// maker notes (`exif`: its TIFF structure). `None` when neither says.
pub fn apple_headroom(map_xmp: Option<&[u8]>, exif: Option<&[u8]>) -> Option<f32> {
    let from_xmp = map_xmp
        .and_then(|x| std::str::from_utf8(x).ok())
        .and_then(|x| xmp_prop(x, "HDRGainMap:HDRGainMapHeadroom"))
        .and_then(|v| v.trim().parse::<f32>().ok());
    let h = from_xmp.or_else(|| exif.and_then(crate::exif::apple_hdr_headroom))?;
    (h.is_finite() && h > 1.0 + 1e-3).then_some(h.min(256.0))
}

/// An Apple HDR gain map in ISO 21496-1 terms. Apple's documented reconstruction is
/// `hdr = sdr · (1 + (H − 1) · g)` with `H` the headroom and `g` the map sample linearised by
/// the sRGB transfer function; that is a log2 gain of `log2(1 + (H − 1) · g)` between 0 and
/// `log2 H`, which this precomputes per map level so the shared ISO path (gamma 1, zero
/// offsets, base headroom 0, alternate headroom `log2 H`) applies it. `map` is the decoded
/// map as RGBA8.
pub fn apple_gain_map(map: &Raster, headroom: f32) -> Option<GainMap> {
    let stops = headroom.log2();
    if stops.is_nan() || stops <= 1e-3 || map.planes.len() != 1 || map.layout != PixelLayout::Rgba {
        return None;
    }
    let lut: [u8; 256] = std::array::from_fn(|v| {
        let g = specular_core::hdr::srgb_to_linear(v as f32 / 255.0);
        let l = (1.0 + (headroom - 1.0) * g).log2() / stops;
        (l * 255.0).round().clamp(0.0, 255.0) as u8
    });
    let mut map = map.clone();
    for px in map.planes[0].data.chunks_exact_mut(4) {
        for c in &mut px[..3] {
            *c = lut[*c as usize];
        }
    }
    let params = GainMapParams {
        gain_min: [0.0; 3],
        gain_max: [stops; 3],
        gamma: [1.0; 3],
        base_offset: [0.0; 3],
        alt_offset: [0.0; 3],
        base_headroom: 0.0,
        alt_headroom: stops,
        ..Default::default()
    };
    params.is_sane().then_some(GainMap {
        map,
        params,
        source: APPLE_SOURCE,
    })
}

/// Candidate offsets of the gain-map JPEG, most trusted first.
fn secondary_offsets(data: &[u8], primary: &Header) -> Vec<usize> {
    let mut v = Vec::new();
    if let Some((mpf, tiff_start)) = primary.mpf {
        for (i, (offset, size)) in mpf_images(mpf).into_iter().enumerate() {
            if i == 0 || offset == 0 {
                continue;
            }
            let abs = tiff_start.saturating_add(offset as usize);
            if size > 0 && abs < data.len() && !v.contains(&abs) {
                v.push(abs);
            }
        }
    }
    if let Some(len) = primary.xmp.and_then(xmp_gain_map_length)
        && len > 0
        && len < data.len()
    {
        // Container:Directory lists the map last; any padding sits between the two images.
        let abs = data.len() - len;
        if data[abs..].starts_with(&[0xFF, 0xD8, 0xFF]) && !v.contains(&abs) {
            v.push(abs);
        }
    }
    if let Some(abs) = after_eoi(data, primary.sos)
        && !v.contains(&abs)
    {
        v.push(abs);
    }
    v
}

/// The first `FFD8 FF` after the primary's EOI (entropy-coded data never holds `FFD9`).
fn after_eoi(data: &[u8], from: usize) -> Option<usize> {
    let rest = data.get(from..)?;
    let eoi = find(rest, &[0xFF, 0xD9])? + from + 2;
    let tail = data.get(eoi..)?;
    // Allow some padding between the images.
    let window = &tail[..tail.len().min(4096)];
    find(window, &[0xFF, 0xD8, 0xFF]).map(|p| eoi + p)
}

/// (offset, size) of every image in an MPF index. Offsets are relative to the MPF TIFF
/// header; the first (primary) image has offset 0.
fn mpf_images(m: &[u8]) -> Vec<(u32, u32)> {
    mpf_entries(m)
        .into_iter()
        .map(|(_, offset, size)| (offset, size))
        .collect()
}

/// MP image type (the low 24 bits of an entry's attribute) of a large thumbnail: CIPA
/// DC-007 class 1, VGA (0x010001) or full HD (0x010002) previews of the primary.
const MP_LARGE_THUMBNAIL: u32 = 0x01_0000;

/// How far from its stated offset an MPF image is looked for.
const MPF_SLACK: usize = 64;

/// Pages of a Multi-Picture (CIPA DC-007, `.mpo`) file after the primary: the byte range of
/// each further image of the MPF index (stereo views, multi-angle shots, panorama frames,
/// undefined extras). Large-thumbnail previews of the primary, gain maps and auxiliary
/// images (depth, mattes) are left out; so are entries that point outside the file, back at
/// the primary or at something that is not a JPEG header (an image a few bytes off its stated
/// offset is still found). A range is cut at the end of the file (a damaged last image reads
/// as truncated). At most 15.
pub fn mpo_pages(data: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut v: Vec<std::ops::Range<usize>> = Vec::new();
    let Some((mpf, tiff_start)) = read_header(data, 0).and_then(|h| h.mpf) else {
        return v;
    };
    for (attr, offset, size) in mpf_entries(mpf).into_iter().skip(1) {
        if offset == 0 || size == 0 || attr & 0x00FF_0000 == MP_LARGE_THUMBNAIL {
            continue;
        }
        let mut start = tiff_start.saturating_add(offset as usize);
        if start >= data.len() {
            continue;
        }
        if read_header(data, start).is_none() {
            // Writers that grew the MPF segment without updating the offsets: the image
            // starts a few bytes away.
            let lo = start.saturating_sub(MPF_SLACK);
            let window = &data[lo..data.len().min(start + MPF_SLACK)];
            match find(window, &[0xFF, 0xD8, 0xFF]) {
                Some(p) if lo + p > 0 => start = lo + p,
                _ => continue,
            }
        }
        if v.iter().any(|r| r.start == start) {
            continue;
        }
        let Some(h) = read_header(data, start) else {
            continue;
        };
        let aux = h.iso.is_some()
            || h.xmp.is_some_and(|x| {
                is_apple_map_xmp(x)
                    || find(x, b"hdrgm:").is_some()
                    || find(x, b"uxiliaryImageType").is_some()
                    || find(x, b":aux:").is_some()
                    || find(x, b"GDepth:").is_some()
            });
        if aux {
            continue;
        }
        let end = start.saturating_add(size as usize).min(data.len());
        v.push(start..end);
    }
    v
}

/// Whether the MPF index describes a stereo pair: exactly two images, both of the
/// "multi-frame disparity" type (CIPA DC-007 type code 0x020002), numbered left to right.
pub fn mpo_is_stereo(data: &[u8]) -> bool {
    let Some((mpf, _)) = read_header(data, 0).and_then(|h| h.mpf) else {
        return false;
    };
    let e = mpf_entries(mpf);
    e.len() == 2 && e.iter().all(|(attr, _, _)| attr & 0x00FF_FFFF == 0x02_0002)
}

/// (attribute, offset, size) of every image in an MPF index (at most 16).
fn mpf_entries(m: &[u8]) -> Vec<(u32, u32, u32)> {
    let mut out = Vec::new();
    if m.len() < 8 {
        return out;
    }
    let le = match &m[..4] {
        b"II*\0" => true,
        b"MM\0*" => false,
        _ => return out,
    };
    let u16_at = |o: usize| {
        m.get(o..o + 2).map(|b| {
            if le {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            }
        })
    };
    let u32_at = |o: usize| {
        m.get(o..o + 4).map(|b| {
            let a = [b[0], b[1], b[2], b[3]];
            if le {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            }
        })
    };
    let Some(ifd) = u32_at(4) else { return out };
    let ifd = ifd as usize;
    let Some(n) = u16_at(ifd) else { return out };
    let (mut entries, mut count) = (None, 0u32);
    for e in 0..n.min(64) as usize {
        let o = ifd + 2 + e * 12;
        let (Some(tag), Some(cnt), Some(val)) = (u16_at(o), u32_at(o + 4), u32_at(o + 8)) else {
            return out;
        };
        match tag {
            0xB001 => count = val,
            0xB002 => entries = Some((val as usize, cnt as usize)),
            _ => {}
        }
    }
    let Some((at, bytes)) = entries else {
        return out;
    };
    let n = (bytes / 16).min(count.max(1) as usize).min(16);
    for i in 0..n {
        let o = at + i * 16;
        let (Some(attr), Some(size), Some(offset)) = (u32_at(o), u32_at(o + 4), u32_at(o + 8))
        else {
            break;
        };
        out.push((attr, offset, size));
    }
    out
}

/// Parses an ISO 21496-1 gain-map metadata payload (after the APP2 signature; the same
/// bytes as an AVIF `tmap` item after its version byte). `None` for the version-only stub
/// a primary image carries, and for unknown versions.
pub fn parse_iso21496(b: &[u8]) -> Option<GainMapParams> {
    parse_iso21496_len(b).map(|(p, _)| p)
}

/// [`parse_iso21496`], also returning how many bytes the metadata took.
fn parse_iso21496_len(b: &[u8]) -> Option<(GainMapParams, usize)> {
    let mut r = Reader { b, i: 0 };
    let min_version = r.u16()?;
    let _writer_version = r.u16()?;
    if min_version != 0 {
        return None;
    }
    let flags = r.u8()?;
    let channels = if flags & 0x80 != 0 { 3 } else { 1 };
    let backward = flags & 0x04 != 0;
    let common = flags & 0x08 != 0;
    let mut p = GainMapParams::default();
    let ratio = |n: f64, d: u32| -> Option<f32> {
        if d == 0 {
            None
        } else {
            Some((n / d as f64) as f32)
        }
    };
    if common {
        let d = r.u32()?;
        p.base_headroom = ratio(r.u32()? as f64, d)?;
        p.alt_headroom = ratio(r.u32()? as f64, d)?;
        for c in 0..channels {
            p.gain_min[c] = ratio(r.i32()? as f64, d)?;
            p.gain_max[c] = ratio(r.i32()? as f64, d)?;
            p.gamma[c] = ratio(r.u32()? as f64, d)?;
            p.base_offset[c] = ratio(r.i32()? as f64, d)?;
            p.alt_offset[c] = ratio(r.i32()? as f64, d)?;
        }
    } else {
        p.base_headroom = ratio(r.u32()? as f64, r.u32()?)?;
        p.alt_headroom = ratio(r.u32()? as f64, r.u32()?)?;
        for c in 0..channels {
            p.gain_min[c] = ratio(r.i32()? as f64, r.u32()?)?;
            p.gain_max[c] = ratio(r.i32()? as f64, r.u32()?)?;
            p.gamma[c] = ratio(r.u32()? as f64, r.u32()?)?;
            p.base_offset[c] = ratio(r.i32()? as f64, r.u32()?)?;
            p.alt_offset[c] = ratio(r.i32()? as f64, r.u32()?)?;
        }
    }
    if channels == 1 {
        for c in 1..3 {
            p.gain_min[c] = p.gain_min[0];
            p.gain_max[c] = p.gain_max[0];
            p.gamma[c] = p.gamma[0];
            p.base_offset[c] = p.base_offset[0];
            p.alt_offset[c] = p.alt_offset[0];
        }
    }
    // The 0x04 flag (written by libultrahdr for an HDR base) keeps the headrooms and offsets
    // in SDR, HDR order; ISO 21496-1 proper stores them base first, the direction following
    // from which headroom is larger. Normalise to the latter.
    if backward && p.alt_headroom > p.base_headroom {
        std::mem::swap(&mut p.base_headroom, &mut p.alt_headroom);
        std::mem::swap(&mut p.base_offset, &mut p.alt_offset);
    }
    p.base_is_hdr = p.base_headroom > p.alt_headroom;
    Some((p, r.i))
}

/// The gain map of a HEIF / AVIF `tmap` item: its body (a version byte, then the ISO 21496-1
/// metadata) as parameters, with the application space (Annex B) set from the primaries of
/// the base or of the alternate (the `tmap` item's own `colr`), as the metadata's
/// `use_base_colour_space` flag says. Only per-channel metadata needs that space; unknown
/// primaries keep BT.709. `None` for other versions and malformed or out-of-range metadata.
pub fn tmap_params(
    body: &[u8],
    base: Option<Primaries>,
    alternate: Option<Primaries>,
) -> Option<GainMapParams> {
    let (&version, meta) = body.split_first()?;
    if version != 0 {
        return None;
    }
    let mut p = parse_iso21496(meta)?;
    let flags = *meta.get(4)?;
    if flags & 0x80 != 0 {
        let space = if flags & 0x40 != 0 { base } else { alternate };
        p.space = space.unwrap_or(Primaries::Bt709);
    }
    p.is_sane().then_some(p)
}

/// The gain map of a JPEG XL `jhgm` box: its ISO 21496-1 metadata as parameters, with the
/// application space chosen as for [`tmap_params`] (`alternate`: the primaries of the
/// bundle's colour encoding). The metadata is the plain ISO 21496-1 payload; one prefixed
/// with a `tmap`-style version byte is accepted too (the two are told apart by which one
/// accounts for every byte). `None` for unknown versions and malformed or out-of-range
/// metadata.
pub fn jhgm_params(
    meta: &[u8],
    base: Option<Primaries>,
    alternate: Option<Primaries>,
) -> Option<GainMapParams> {
    let exact = |m: &[u8]| parse_iso21496_len(m).filter(|&(_, n)| n == m.len());
    let tmap = || match meta.split_first() {
        Some((0, m)) => exact(m).map(|(p, _)| (p, m)),
        _ => None,
    };
    let (mut p, m) = exact(meta)
        .map(|(p, _)| (p, meta))
        .or_else(tmap)
        .or_else(|| parse_iso21496(meta).map(|p| (p, meta)))?;
    let flags = *m.get(4)?;
    if flags & 0x80 != 0 {
        let space = if flags & 0x40 != 0 { base } else { alternate };
        p.space = space.unwrap_or(Primaries::Bt709);
    }
    p.is_sane().then_some(p)
}

/// Colour primaries of an H.273 code point the renderer can convert to (BT.709, Display P3,
/// BT.2020).
pub fn cicp_primaries(code: u16) -> Option<Primaries> {
    match code {
        1 => Some(Primaries::Bt709),
        12 => Some(Primaries::DisplayP3),
        9 => Some(Primaries::Bt2020),
        _ => None,
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.b.get(self.i..self.i + N)?;
        self.i += N;
        s.try_into().ok()
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes(self.take()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.take()?))
    }
}

/// Parses `hdrgm:` gain-map metadata from XMP (attributes or elements, single values or
/// per-channel `rdf:Seq`). `None` without `hdrgm:Version` / `hdrgm:GainMapMax`.
pub fn parse_hdrgm_xmp(x: &[u8]) -> Option<GainMapParams> {
    let x = std::str::from_utf8(x).ok()?;
    xmp_value(x, "Version")?;
    let max = xmp_values(x, "GainMapMax")?;
    let min = xmp_values(x, "GainMapMin").unwrap_or([0.0; 3]);
    let gamma = xmp_values(x, "Gamma").unwrap_or([1.0; 3]);
    let off_sdr = xmp_values(x, "OffsetSDR").unwrap_or([1.0 / 64.0; 3]);
    let off_hdr = xmp_values(x, "OffsetHDR").unwrap_or([1.0 / 64.0; 3]);
    let cap_min = xmp_value(x, "HDRCapacityMin")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(0.0);
    let cap_max = xmp_value(x, "HDRCapacityMax")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or_else(|| max.iter().copied().fold(f32::MIN, f32::max));
    let base_is_hdr =
        xmp_value(x, "BaseRenditionIsHDR").is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    let (base_offset, alt_offset, base_headroom, alt_headroom) = if base_is_hdr {
        (off_hdr, off_sdr, cap_max, cap_min)
    } else {
        (off_sdr, off_hdr, cap_min, cap_max)
    };
    Some(GainMapParams {
        gain_min: min,
        gain_max: max,
        gamma,
        base_offset,
        alt_offset,
        base_headroom,
        alt_headroom,
        base_is_hdr,
        ..Default::default()
    })
}

/// The raw text of `hdrgm:<name>`: an attribute value, or an element's content.
fn xmp_value<'a>(x: &'a str, name: &str) -> Option<&'a str> {
    xmp_prop(x, &format!("hdrgm:{name}"))
}

/// The raw text of the XMP property `key` (`prefix:Name`).
fn xmp_prop<'a>(x: &'a str, key: &str) -> Option<&'a str> {
    let b = x.as_bytes();
    let mut from = 0;
    while let Some(p) = x[from..].find(key) {
        let start = from + p;
        let at = start + key.len();
        from = at;
        // Longer names sharing the prefix, and closing tags, are not it.
        let next = b.get(at).copied().unwrap_or(b' ');
        if next.is_ascii_alphanumeric() || next == b'_' || (start > 0 && b[start - 1] == b'/') {
            continue;
        }
        let rest = x[at..].trim_start();
        if let Some(r) = rest.strip_prefix('=') {
            let r = r.trim_start();
            let q = r.chars().next()?;
            if q != '"' && q != '\'' {
                return None;
            }
            let end = r[1..].find(q)?;
            return Some(&r[1..1 + end]);
        }
        if start > 0 && b[start - 1] == b'<' {
            let open = at + x[at..].find('>')? + 1;
            let end = x[open..].find(&format!("</{key}"))?;
            return Some(&x[open..open + end]);
        }
    }
    None
}

/// Per-channel values: a single number is replicated; an `rdf:Seq` gives up to three.
fn xmp_values(x: &str, name: &str) -> Option<[f32; 3]> {
    let v = xmp_value(x, name)?;
    if !v.contains("<rdf:li") {
        let f = v.trim().parse::<f32>().ok()?;
        return Some([f; 3]);
    }
    let mut out = Vec::new();
    let mut rest = v;
    while let Some(p) = rest.find("<rdf:li") {
        let r = &rest[p..];
        let open = r.find('>')? + 1;
        let close = r.find("</rdf:li>")?;
        out.push(r.get(open..close)?.trim().parse::<f32>().ok()?);
        rest = &r[close + 9..];
    }
    match out.len() {
        1 => Some([out[0]; 3]),
        3 => Some([out[0], out[1], out[2]]),
        _ => None,
    }
}

/// `Item:Length` of the `GainMap` item in a primary image's XMP container directory.
fn xmp_gain_map_length(x: &[u8]) -> Option<usize> {
    let x = std::str::from_utf8(x).ok()?;
    let p = x.find("Item:Semantic=\"GainMap\"")?;
    // The item's attributes sit in the same element: search a window around it.
    let start = x[..p].rfind('<')?;
    let end = p + x[p..].find('>')?;
    let item = &x[start..end];
    let l = item.find("Item:Length=\"")? + 13;
    let e = item[l..].find('"')?;
    item[l..l + e].trim().parse().ok()
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    let (&first, tail) = n.split_first()?;
    let mut i = 0;
    while let Some(p) = h.get(i..)?.iter().position(|&c| c == first) {
        let at = i + p;
        if h.get(at + 1..at + n.len()) == Some(tail) {
            return Some(at);
        }
        i = at + 1;
    }
    None
}

/// Decodes the map JPEG to an RGBA8 raster (grey replicated, YCbCr converted to RGB with
/// the JFIF matrix and no transfer function: the samples are the encoded gains).
fn decode_map(jpeg: &[u8], cancel: &CancelToken) -> Option<Raster> {
    let info = sj::read_info(jpeg).ok()?;
    if info.width == 0
        || info.height == 0
        || info.width > MAX_MAP_SIDE
        || info.height > MAX_MAP_SIDE
    {
        return None;
    }
    let opts = sj::DecodeOptions {
        cancel: Some(cancel.flag()),
        ..Default::default()
    };
    let d = sj::decode(jpeg, &opts).ok()?;
    let r = crate::jpeg::to_raster(d, 1);
    Some(to_rgba8(&r))
}

/// Expands any 8-bit raster to one interleaved RGBA8 plane.
pub(crate) fn to_rgba8(r: &Raster) -> Raster {
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = vec![255u8; w * h * 4];
    let sample = |p: usize, x: usize, y: usize| -> f32 {
        let Some(pl) = r.planes.get(p) else {
            return 0.0;
        };
        let px = (x / pl.h_div.max(1) as usize).min(pl.width.saturating_sub(1) as usize);
        let py = (y / pl.v_div.max(1) as usize).min(pl.height.saturating_sub(1) as usize);
        pl.data.get(py * pl.stride + px).copied().unwrap_or(0) as f32
    };
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            let px = match r.layout {
                PixelLayout::YCbCr => {
                    let yy = sample(0, x, y);
                    let cb = sample(1, x, y) - 128.0;
                    let cr = sample(2, x, y) - 128.0;
                    [
                        q(yy + 1.402 * cr),
                        q(yy - 0.344_136 * cb - 0.714_136 * cr),
                        q(yy + 1.772 * cb),
                    ]
                }
                PixelLayout::Rgb => [q(sample(0, x, y)), q(sample(1, x, y)), q(sample(2, x, y))],
                PixelLayout::Rgba => {
                    let pl = &r.planes[0];
                    let i = y * pl.stride + x * 4;
                    [pl.data[i], pl.data[i + 1], pl.data[i + 2]]
                }
                _ => {
                    let v = q(sample(0, x, y));
                    [v, v, v]
                }
            };
            out[o..o + 3].copy_from_slice(&px);
        }
    }
    Raster {
        width: r.width,
        height: r.height,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: r.width,
            height: r.height,
            stride: w * 4,
            data: out,
            h_div: 1,
            v_div: 1,
        }],
        truncated: r.truncated,
    }
}
