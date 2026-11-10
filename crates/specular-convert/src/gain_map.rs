//! Gain-map writing (ISO 21496-1): one file that shows an SDR image on SDR screens and an
//! HDR rendition where there is headroom. The base is an ordinary SDR image; the map holds
//! per-pixel log2 gains towards the HDR rendition, and the metadata says how to apply them
//! (the reverse of `specular_decode::gainmap`, whose maths reads them back).
//!
//! - **JPEG** (Ultra HDR / ISO 21496-1): the primary JPEG carries the ISO 21496-1 version
//!   stub (APP2) and an MPF index (CIPA DC-007) pointing at the map, a second JPEG appended
//!   after it with the full ISO 21496-1 metadata (APP2) and the same values as `hdrgm:` XMP
//!   for readers that only know the Adobe/Android form. The primary's XMP gets
//!   `hdrgm:Version` and the `Container:Directory` of the two images (Android's Ultra HDR
//!   format): a packet of its own when the primary has none, otherwise a second
//!   `rdf:Description` in its packet (a directory or version copied from the source is
//!   replaced, its lengths would be wrong).
//! - **Colour space**: the gains are computed and applied in the base image's own space
//!   (its primaries, linear). Every form says so: ISO 21496-1's `use_base_colour_space` flag
//!   (set in the JPEG APP2, the AVIF `tmap` and the JPEG XL `jhgm` metadata), with the space
//!   given by the base's ICC profile (JPEG, JPEG XL) or `nclx` (AVIF, also on the `tmap`
//!   item). Adobe's `hdrgm:` form has no field for it.
//! - **AVIF**: a hidden map item and a `tmap` derived item over `[primary, map]`, grouped
//!   with the primary in an `altr` group (written by `avif_mux`).
//! - **JPEG XL**: a `jhgm` box after the codestream, holding the metadata and the map as a
//!   bare codestream.

use specular_core::GainMapParams;

/// The ISO 21496-1 APP2 signature (also used in AVIF and JPEG XL without it).
pub const ISO_SIG: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";
const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const MPF_SIG: &[u8] = b"MPF\0";
/// The common denominator of the written fractions.
const DENOM: u32 = 1 << 20;

/// A gain map ready to encode: `channels` (1 or 3) interleaved 8-bit samples and the
/// metadata that turns them into gains.
#[derive(Clone, Debug, PartialEq)]
pub struct GainMapImage {
    pub width: u32,
    pub height: u32,
    pub channels: usize,
    pub samples: Vec<u8>,
    pub params: GainMapParams,
}

/// The ISO 21496-1 metadata payload (after the APP2 signature; the `tmap` body after its
/// version byte; the `jhgm` metadata as is), with one common denominator. `use_base` sets
/// the "gains apply in the base's colour space" flag.
pub fn iso21496(p: &GainMapParams, multichannel: bool, use_base: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(64);
    b.extend_from_slice(&0u16.to_be_bytes());
    b.extend_from_slice(&0u16.to_be_bytes());
    let mut flags = 0x08;
    if multichannel {
        flags |= 0x80;
    }
    if use_base {
        flags |= 0x40;
    }
    b.push(flags);
    let n = |v: f32| (f64::from(v) * f64::from(DENOM)).round();
    let u = |v: f32| n(v).clamp(0.0, f64::from(u32::MAX)) as u32;
    let i = |v: f32| n(v).clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    b.extend_from_slice(&DENOM.to_be_bytes());
    // Headrooms are base first (ISO 21496-1 proper; the direction follows from their order).
    b.extend_from_slice(&u(p.base_headroom).to_be_bytes());
    b.extend_from_slice(&u(p.alt_headroom).to_be_bytes());
    for c in 0..if multichannel { 3 } else { 1 } {
        b.extend_from_slice(&i(p.gain_min[c]).to_be_bytes());
        b.extend_from_slice(&i(p.gain_max[c]).to_be_bytes());
        b.extend_from_slice(&u(p.gamma[c]).to_be_bytes());
        b.extend_from_slice(&i(p.base_offset[c]).to_be_bytes());
        b.extend_from_slice(&i(p.alt_offset[c]).to_be_bytes());
    }
    b
}

/// The same values as Adobe / Android `hdrgm:` XMP (for the map image).
pub fn hdrgm_xmp(p: &GainMapParams, multichannel: bool) -> String {
    let one = |v: [f32; 3]| -> String {
        if multichannel {
            format!(
                "<rdf:Seq><rdf:li>{}</rdf:li><rdf:li>{}</rdf:li><rdf:li>{}</rdf:li></rdf:Seq>",
                v[0], v[1], v[2]
            )
        } else {
            v[0].to_string()
        }
    };
    format!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF \
         xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description \
         xmlns:hdrgm=\"http://ns.adobe.com/hdr-gain-map/1.0/\" hdrgm:Version=\"1.0\" \
         hdrgm:HDRCapacityMin=\"{}\" hdrgm:HDRCapacityMax=\"{}\" \
         hdrgm:BaseRenditionIsHDR=\"False\"><hdrgm:GainMapMin>{}</hdrgm:GainMapMin>\
         <hdrgm:GainMapMax>{}</hdrgm:GainMapMax><hdrgm:Gamma>{}</hdrgm:Gamma>\
         <hdrgm:OffsetSDR>{}</hdrgm:OffsetSDR><hdrgm:OffsetHDR>{}</hdrgm:OffsetHDR>\
         </rdf:Description></rdf:RDF></x:xmpmeta>",
        p.base_headroom,
        p.alt_headroom,
        one(p.gain_min),
        one(p.gain_max),
        one(p.gamma),
        one(p.base_offset),
        one(p.alt_offset),
    )
}

/// The primary's XMP when it has none: `hdrgm:Version` and the container directory.
fn primary_xmp(map_len: usize) -> String {
    format!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF \
         xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">{}</rdf:RDF></x:xmpmeta>",
        primary_description(map_len)
    )
}

/// The `rdf:Description` with `hdrgm:Version` and the container directory.
fn primary_description(map_len: usize) -> String {
    format!(
        "<rdf:Description rdf:about=\"\" \
         xmlns:hdrgm=\"http://ns.adobe.com/hdr-gain-map/1.0/\" \
         xmlns:Container=\"http://ns.google.com/photos/1.0/container/\" \
         xmlns:Item=\"http://ns.google.com/photos/1.0/container/item/\" hdrgm:Version=\"1.0\">\
         <Container:Directory><rdf:Seq>\
         <rdf:li rdf:parseType=\"Resource\"><Container:Item Item:Semantic=\"Primary\" \
         Item:Mime=\"image/jpeg\"/></rdf:li>\
         <rdf:li rdf:parseType=\"Resource\"><Container:Item Item:Semantic=\"GainMap\" \
         Item:Mime=\"image/jpeg\" Item:Length=\"{map_len}\"/></rdf:li>\
         </rdf:Seq></Container:Directory></rdf:Description>"
    )
}

/// The primary's own XMP packet with the gain-map description added (any container
/// directory or `hdrgm:Version` it already has removed); `None` when it isn't an RDF packet.
fn merged_xmp(packet: &str, map_len: usize) -> Option<String> {
    let mut x = packet.to_owned();
    // A directory copied from an earlier gain-map file (its lengths are stale).
    while let Some(a) = x.find("<Container:Directory") {
        let close = "</Container:Directory>";
        let b = x[a..].find(close).map(|i| a + i + close.len())?;
        x.replace_range(a..b, "");
    }
    while let Some(a) = x.find("hdrgm:Version=") {
        let q = x[a + 14..].chars().next()?;
        let end = x[a + 15..].find(q).map(|i| a + 15 + i + 1)?;
        x.replace_range(a..end, "");
    }
    while let Some(a) = x.find("<hdrgm:Version>") {
        let close = "</hdrgm:Version>";
        let b = x[a..].find(close).map(|i| a + i + close.len())?;
        x.replace_range(a..b, "");
    }
    let at = x.rfind("</rdf:RDF>")?;
    x.insert_str(at, &primary_description(map_len));
    Some(x)
}

fn segment(marker: u8, parts: &[&[u8]]) -> Result<Vec<u8>, String> {
    let len: usize = parts.iter().map(|p| p.len()).sum::<usize>() + 2;
    let len = u16::try_from(len).map_err(|_| "JPEG: gain-map segment too large".to_owned())?;
    let mut s = vec![0xFF, marker];
    s.extend_from_slice(&len.to_be_bytes());
    parts.iter().for_each(|p| s.extend_from_slice(p));
    Ok(s)
}

/// Where new APP segments go: after SOI and the APPn segments the encoder wrote. Also
/// whether there is an XMP segment already.
fn after_app(jpeg: &[u8]) -> Result<(usize, bool), String> {
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return Err("JPEG: no SOI".into());
    }
    let mut i = 2;
    let mut xmp = false;
    while i + 4 <= jpeg.len() && jpeg[i] == 0xFF && (0xE0..=0xEF).contains(&jpeg[i + 1]) {
        let len = usize::from(u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]));
        if jpeg[i + 1] == 0xE1 && jpeg[i + 4..].starts_with(XMP_SIG) {
            xmp = true;
        }
        i += 2 + len;
    }
    if i > jpeg.len() {
        return Err("JPEG: bad segment".into());
    }
    Ok((i, xmp))
}

/// The MPF APP2 segment for two images: the primary (`primary_len` bytes, offset 0) and the
/// map right after it. `tiff_at` is where the segment's TIFF header will sit in the file.
fn mpf_segment(primary_len: usize, map_len: usize, tiff_at: usize) -> Result<Vec<u8>, String> {
    let big = |v: usize| u32::try_from(v).map_err(|_| "JPEG over 4 GiB".to_owned());
    let mut t = b"MM\0\x2A\0\0\0\x08".to_vec();
    t.extend_from_slice(&3u16.to_be_bytes());
    // MPFVersion "0100", NumberOfImages 2, MPEntry (32 bytes after the IFD).
    let entries_at: u32 = 8 + 2 + 3 * 12 + 4;
    t.extend_from_slice(&[0xB0, 0x00, 0, 7, 0, 0, 0, 4]);
    t.extend_from_slice(b"0100");
    t.extend_from_slice(&[0xB0, 0x01, 0, 4, 0, 0, 0, 1, 0, 0, 0, 2]);
    t.extend_from_slice(&[0xB0, 0x02, 0, 7, 0, 0, 0, 32]);
    t.extend_from_slice(&entries_at.to_be_bytes());
    t.extend_from_slice(&0u32.to_be_bytes());
    // Primary: representative baseline image; the map: undefined type, after the primary.
    t.extend_from_slice(&0x2003_0000u32.to_be_bytes());
    t.extend_from_slice(&big(primary_len)?.to_be_bytes());
    t.extend_from_slice(&0u32.to_be_bytes());
    t.extend_from_slice(&[0, 0, 0, 0]);
    t.extend_from_slice(&0u32.to_be_bytes());
    t.extend_from_slice(&big(map_len)?.to_be_bytes());
    t.extend_from_slice(&big(primary_len - tiff_at)?.to_be_bytes());
    t.extend_from_slice(&[0, 0, 0, 0]);
    segment(0xE2, &[MPF_SIG, &t])
}

/// A gain-map JPEG from the encoded primary (SDR base, with its own Exif/XMP/ICC) and the
/// encoded map (no metadata), with `params`.
pub fn jpeg(
    primary: &[u8],
    map: &[u8],
    params: &GainMapParams,
    multichannel: bool,
) -> Result<Vec<u8>, String> {
    // The map: ISO metadata and hdrgm XMP after its SOI.
    let (at, _) = after_app(map)?;
    let iso = iso21496(params, multichannel, true);
    let xmp = hdrgm_xmp(params, multichannel);
    let mut m = map[..at].to_vec();
    m.extend_from_slice(&segment(0xE1, &[XMP_SIG, xmp.as_bytes()])?);
    m.extend_from_slice(&segment(0xE2, &[ISO_SIG, &iso])?);
    m.extend_from_slice(&map[at..]);
    // The primary: its XMP with the directory, the ISO version stub, then MPF.
    let (at, has_xmp) = after_app(primary)?;
    let mut head = Vec::with_capacity(at + 1024);
    head.extend_from_slice(&primary[..2]);
    let mut i = 2;
    while i < at {
        let len = usize::from(u16::from_be_bytes([primary[i + 2], primary[i + 3]]));
        let seg = &primary[i..i + 2 + len];
        let body = &seg[4..];
        let merged = (seg[1] == 0xE1 && body.starts_with(XMP_SIG))
            .then(|| std::str::from_utf8(&body[XMP_SIG.len()..]).ok())
            .flatten()
            .and_then(|x| merged_xmp(x, m.len()))
            .and_then(|x| segment(0xE1, &[XMP_SIG, x.as_bytes()]).ok());
        // A packet that can't take the directory (not RDF, or past 64 KiB) stays as it was.
        head.extend_from_slice(merged.as_deref().unwrap_or(seg));
        i += 2 + len;
    }
    if !has_xmp {
        head.extend_from_slice(&segment(0xE1, &[XMP_SIG, primary_xmp(m.len()).as_bytes()])?);
    }
    head.extend_from_slice(&segment(0xE2, &[ISO_SIG, &[0, 0, 0, 0]])?);
    // The MPF segment's size does not depend on the values in it.
    let mpf_len = mpf_segment(0, 0, 0)?.len();
    let tiff_at = head.len() + 4 + MPF_SIG.len();
    let primary_len = head.len() + mpf_len + (primary.len() - at);
    head.extend_from_slice(&mpf_segment(primary_len, m.len(), tiff_at)?);
    let mut out = Vec::with_capacity(primary_len + m.len());
    out.extend_from_slice(&head);
    out.extend_from_slice(&primary[at..]);
    debug_assert_eq!(out.len(), primary_len);
    out.extend_from_slice(&m);
    Ok(out)
}

/// A JPEG XL container (`base`, as libjxl wrote it with the ISOBMFF container) with a
/// `jhgm` box holding `params` and the map codestream `map` (a bare codestream).
pub fn jxl(
    base: &[u8],
    map: &[u8],
    params: &GainMapParams,
    multichannel: bool,
) -> Result<Vec<u8>, String> {
    if !base.starts_with(&[0, 0, 0, 0x0C, b'J', b'X', b'L', b' ']) {
        return Err("JPEG XL: the base is not in a container".into());
    }
    if !map.starts_with(&[0xFF, 0x0A]) {
        return Err("JPEG XL: the map is not a bare codestream".into());
    }
    // Give a last box that runs to the end of the file its real size.
    let mut out = base.to_vec();
    let mut i = 0;
    while i + 8 <= out.len() {
        let size = u32::from_be_bytes([out[i], out[i + 1], out[i + 2], out[i + 3]]) as usize;
        let len = match size {
            0 => {
                let l =
                    u32::try_from(out.len() - i).map_err(|_| "JPEG XL over 4 GiB".to_owned())?;
                out[i..i + 4].copy_from_slice(&l.to_be_bytes());
                break;
            }
            1 => {
                let b = out.get(i + 8..i + 16).ok_or("JPEG XL: bad box")?;
                usize::try_from(u64::from_be_bytes(
                    b.try_into().map_err(|_| "JPEG XL: bad box")?,
                ))
                .map_err(|_| "JPEG XL: bad box".to_owned())?
            }
            s => s,
        };
        if len < 8 {
            return Err("JPEG XL: bad box".into());
        }
        i += len;
    }
    let iso = iso21496(params, multichannel, true);
    let mut body = vec![0u8];
    body.extend_from_slice(
        &u16::try_from(iso.len())
            .map_err(|_| "JPEG XL: metadata too large".to_owned())?
            .to_be_bytes(),
    );
    body.extend_from_slice(&iso);
    body.push(0);
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(map);
    let len = u32::try_from(body.len() + 8).map_err(|_| "JPEG XL over 4 GiB".to_owned())?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(b"jhgm");
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> GainMapParams {
        GainMapParams {
            gain_min: [-0.25, -0.5, 0.0],
            gain_max: [2.0, 1.75, 2.5],
            gamma: [1.0, 1.0, 1.0],
            base_offset: [1.0 / 64.0; 3],
            alt_offset: [1.0 / 64.0; 3],
            base_headroom: 0.0,
            alt_headroom: 2.0,
            base_is_hdr: false,
            space: specular_core::Primaries::Bt709,
        }
    }

    /// Our metadata reads back with the decoder's parsers (APP2, `tmap` and `jhgm` forms).
    #[test]
    fn iso_metadata_reads_back() {
        let p = params();
        for multi in [false, true] {
            let b = iso21496(&p, multi, true);
            let back = specular_decode::parse_iso21496(&b).expect("parses");
            for c in 0..3 {
                let k = if multi { c } else { 0 };
                assert!((back.gain_min[c] - p.gain_min[k]).abs() < 1e-6);
                assert!((back.gain_max[c] - p.gain_max[k]).abs() < 1e-6);
                assert!((back.base_offset[c] - p.base_offset[k]).abs() < 1e-6);
            }
            assert_eq!(back.alt_headroom, 2.0);
            assert!(!back.base_is_hdr);
            let tmap = [&[0u8][..], &b].concat();
            let t = specular_decode::tmap_params(
                &tmap,
                Some(specular_core::Primaries::DisplayP3),
                None,
            )
            .expect("tmap");
            if multi {
                assert_eq!(t.space, specular_core::Primaries::DisplayP3);
            }
            assert!(specular_decode::jhgm_params(&b, None, None).is_some());
            let x = hdrgm_xmp(&p, multi);
            let h = specular_decode::parse_hdrgm_xmp(x.as_bytes()).expect("xmp");
            assert!((h.gain_max[0] - 2.0).abs() < 1e-6);
            assert!((h.alt_headroom - 2.0).abs() < 1e-6);
        }
    }

    /// A primary with XMP of its own gets the directory in that packet; a stale one copied
    /// from an earlier gain-map file is replaced.
    #[test]
    fn primary_xmp_gets_the_directory() {
        let own = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
            <rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" \
            xmlns:hdrgm=\"http://ns.adobe.com/hdr-gain-map/1.0/\" hdrgm:Version=\"1.0\" xmp:Rating=\"4\">\
            <Container:Directory><rdf:Seq><rdf:li>old</rdf:li></rdf:Seq></Container:Directory>\
            </rdf:Description></rdf:RDF></x:xmpmeta>";
        let x = merged_xmp(own, 1234).expect("merged");
        assert!(x.contains("xmp:Rating=\"4\""));
        assert!(!x.contains("old"));
        assert_eq!(x.matches("hdrgm:Version").count(), 1);
        assert_eq!(x.matches("<Container:Directory>").count(), 1);
        assert!(x.contains("Item:Length=\"1234\""));
        assert!(x.ends_with("</rdf:RDF></x:xmpmeta>"));
        let primary = specular_jpeg::encode::encode(
            8,
            8,
            specular_jpeg::encode::Samples::Gray(&[90; 64]),
            &specular_jpeg::encode::EncodeOptions {
                quality: 90,
                subsampling: specular_jpeg::encode::Subsampling::S444,
                exif: None,
                xmp: Some(own.as_bytes()),
                icc: None,
            },
        )
        .unwrap();
        let map = specular_jpeg::encode::encode(
            8,
            8,
            specular_jpeg::encode::Samples::Gray(&[200; 64]),
            &specular_jpeg::encode::EncodeOptions {
                quality: 90,
                subsampling: specular_jpeg::encode::Subsampling::S444,
                exif: None,
                xmp: None,
                icc: None,
            },
        )
        .unwrap();
        let f = jpeg(&primary, &map, &params(), false).unwrap();
        let text = String::from_utf8_lossy(&f);
        assert_eq!(text.matches("<Container:Directory>").count(), 1);
        assert!(text.contains("xmp:Rating=\"4\""));
        assert!(specular_decode::jpeg_gain_map(&f, &specular_core::CancelToken::new()).is_some());
    }

    #[test]
    fn mpf_points_at_the_map() {
        let jpeg_of = |v: u8| {
            specular_jpeg::encode::encode(
                8,
                8,
                specular_jpeg::encode::Samples::Gray(&[v; 64]),
                &specular_jpeg::encode::EncodeOptions {
                    quality: 90,
                    subsampling: specular_jpeg::encode::Subsampling::S444,
                    exif: None,
                    xmp: None,
                    icc: None,
                },
            )
            .unwrap()
        };
        let (p, m) = (jpeg_of(100), jpeg_of(200));
        let f = jpeg(&p, &m, &params(), false).unwrap();
        assert!(specular_decode::jpeg_has_gain_map(&f));
        // A gain map is not a page of its own.
        assert!(specular_decode::mpo_pages(&f).is_empty());
        // The MPF index: the primary at 0 with its whole length, the map right after it.
        let at = f.windows(4).position(|w| w == MPF_SIG).unwrap() + 4;
        let be = |o: usize| u32::from_be_bytes(f[at + o..at + o + 4].try_into().unwrap()) as usize;
        let entries = be(8 + 2 + 2 * 12 + 8);
        let (size0, off0) = (be(entries + 4), be(entries + 8));
        let (size1, off1) = (be(entries + 20), be(entries + 24));
        assert_eq!(off0, 0);
        assert_eq!(at + off1, size0);
        assert_eq!(size0 + size1, f.len());
        assert!(f[size0..].starts_with(&[0xFF, 0xD8]));
        let g = specular_decode::jpeg_gain_map(&f, &specular_core::CancelToken::new())
            .expect("gain map");
        assert!((g.params.gain_max[0] - 2.0).abs() < 1e-6);
        assert_eq!(g.source, "ISO 21496-1");
    }
}
