//! Vendor maker notes: where their IFD lives, which offset base and byte order they use, and the
//! few tags the RAW reader needs (previews, white balance, black levels, sensor crop, Nikon/Pentax
//! decoder tables, Sony's encrypted SR2 block).

use std::ops::Range;

use crate::tiff::{self, Endian, Entry, Ifd, read_ifd, read_ifd_with};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vendor {
    Canon,
    Nikon,
    Sony,
    Olympus,
    Pentax,
    Panasonic,
    Fujifilm,
    Other,
}

pub(crate) fn vendor(make: &str) -> Vendor {
    let m = make.to_ascii_lowercase();
    if m.starts_with("canon") {
        Vendor::Canon
    } else if m.starts_with("nikon") {
        Vendor::Nikon
    } else if m.starts_with("sony") {
        Vendor::Sony
    } else if m.starts_with("olympus") || m.starts_with("om digital") || m.starts_with("om system")
    {
        Vendor::Olympus
    } else if m.starts_with("pentax") || m.starts_with("ricoh") || m.starts_with("asahi") {
        Vendor::Pentax
    } else if m.starts_with("panasonic") {
        Vendor::Panasonic
    } else if m.starts_with("fujifilm") {
        Vendor::Fujifilm
    } else {
        Vendor::Other
    }
}

/// Locate and read the maker-note IFD from the Exif `MakerNote` entry. `file_base` / `file_le`
/// are those of the TIFF structure containing the Exif IFD.
pub(crate) fn maker_ifd(
    d: &[u8],
    e: &Entry,
    file_base: usize,
    file_le: bool,
    v: Vendor,
) -> Option<Ifd> {
    let p = e.pos;
    let mn = e.bytes(d)?;
    let starts = |s: &[u8]| mn.starts_with(s);
    let order = |at: usize| match mn.get(at..at + 2) {
        Some(b"II") => Some(true),
        Some(b"MM") => Some(false),
        _ => None,
    };
    match v {
        Vendor::Nikon => {
            if starts(b"Nikon\0\x02") {
                let (le, first) = tiff::header(d, p + 10)?;
                read_ifd(d, first, p + 10, le)
            } else if starts(b"Nikon\0\x01") {
                read_ifd(d, p + 8, file_base, file_le)
            } else {
                read_ifd(d, p, file_base, file_le)
            }
        }
        Vendor::Sony => {
            let at = if starts(b"SONY") || starts(b"VHAB") {
                12
            } else {
                0
            };
            read_ifd(d, p + at, file_base, file_le)
        }
        Vendor::Olympus => {
            if starts(b"OLYMPUS\0") {
                read_ifd(d, p + 12, p, order(8)?)
            } else if starts(b"OM SYSTEM\0") {
                read_ifd(d, p + 16, p, order(12)?)
            } else if starts(b"OLYMP\0") {
                read_ifd(d, p + 8, file_base, file_le)
            } else {
                None
            }
        }
        Vendor::Pentax => {
            if starts(b"AOC\0") {
                read_ifd(d, p + 6, file_base, order(4).unwrap_or(file_le))
            } else if starts(b"PENTAX \0") {
                read_ifd(d, p + 10, p, order(8)?)
            } else {
                read_ifd(d, p, file_base, file_le)
            }
        }
        Vendor::Panasonic => {
            if starts(b"Panasonic\0") {
                read_ifd(d, p + 12, file_base, file_le)
            } else {
                None
            }
        }
        Vendor::Fujifilm => {
            if starts(b"FUJIFILM") {
                let off = Endian { le: true }.u32(d, p + 8)? as usize;
                read_ifd(d, p + off, p, true)
            } else {
                None
            }
        }
        Vendor::Other if starts(b"EPSON\0") => {
            // No byte-order mark: the R-D1 writes a big-endian maker note in a little-endian
            // file. Take the order whose entry count is plausible.
            let le = match (tiff::be_u16(d, p + 8), mn.get(8..10)) {
                (Some(n), _) if (1..512).contains(&n) => false,
                (_, Some(b)) if (1..512).contains(&u16::from_le_bytes([b[0], b[1]])) => true,
                _ => file_le,
            };
            read_ifd(d, p + 8, file_base, le)
        }
        Vendor::Canon | Vendor::Other => read_ifd(d, p, file_base, file_le),
    }
}

/// A sub-IFD referenced from a maker note entry: either stored inline (UNDEFINED) or by offset.
pub(crate) fn sub_ifd(d: &[u8], parent: &Ifd, tag: u16) -> Option<Ifd> {
    let e = parent.get(tag)?;
    let off = if e.typ == 7 {
        e.pos
    } else {
        parent.base.checked_add(e.uint(d, 0)? as usize)?
    };
    read_ifd(d, off, parent.base, parent.le)
}

// ---------------------------------------------------------------------------------------------
// Canon

#[derive(Clone, Debug, Default)]
pub(crate) struct CanonInfo {
    /// Sensor size and visible borders (left, top, right, bottom inclusive).
    pub sensor: Option<[u32; 6]>,
    /// As-shot white balance levels, R G G B.
    pub wb_rggb: Option<[f64; 4]>,
    /// Per-channel black level (R G G B) from ColorData (newer bodies). ColorData's white
    /// levels are not used: the specular white sits below where the data clips (EOS RP at
    /// ISO 100: 14558, samples run evenly to 16383), so the white is the 14-bit maximum, as
    /// in LibRaw, and the develop lowers it to a clip the data shows.
    pub black: Option<[u32; 4]>,
    /// 0x0010 ModelID.
    pub model_id: Option<u32>,
    /// 0x0007 FirmwareVersion.
    pub firmware: Option<String>,
    /// sRAW/mRAW reference levels (R, G, B in 1/1024): the ColorData record whose green is
    /// 1170, which maps the camera's white-balanced luma/chroma back to sensor RGB.
    pub sraw_levels: Option<[u32; 3]>,
}

/// The lens name of a Canon maker note (0x0095 LensModel; the camera pads it with leftover
/// bytes after the NUL), for files whose Exif has no LensModel.
pub(crate) fn canon_lens(d: &[u8], mn: &Ifd) -> Option<String> {
    mn.string(d, 0x0095)
        .filter(|s| s.chars().any(|c| c.is_alphanumeric()))
}

pub(crate) fn canon(d: &[u8], mn: &Ifd) -> CanonInfo {
    let mut out = CanonInfo {
        model_id: mn.uint(d, 0x0010),
        firmware: mn.string(d, 0x0007),
        ..CanonInfo::default()
    };
    // 0x00e0 SensorInfo: [1] width, [2] height, [5] left, [6] top, [7] right, [8] bottom.
    let s = mn.uints(d, 0x00e0, 17);
    if s.len() >= 9 {
        let v = [s[1], s[2], s[5], s[6], s[7], s[8]];
        if v[0] > 0 && v[1] > 0 && v[2] < v[4] && v[3] < v[5] {
            out.sensor = Some(v);
        }
    }
    // 0x4001 ColorData: the as-shot WB index depends on the (camera-generation) table length.
    if let Some(e) = mn.get(0x4001) {
        // Some PowerShots (G7 X Mark II) write ColorData as UNDEFINED[5120]: the same 16-bit
        // table, with the byte length as the count that selects the layout.
        let e = if matches!(e.typ, 1 | 7) {
            Entry {
                typ: 3,
                count: e.count / 2,
                ..*e
            }
        } else {
            *e
        };
        let count = mn.get(0x4001).map_or(0, |e| e.count);
        let version = e.uint(d, 0).map(|v| v as u16 as i16);
        let idx = match count {
            582 => Some(25),
            653 => Some(34),
            5120 => Some(71),
            1816 | 1820 | 1824 => Some(71),
            1770 | 2024 | 3656 => Some(85),
            3973 | 3778 | 4528 => Some(105),
            n if n > 500 => Some(63),
            _ => None,
        };
        if let Some(i) = idx {
            let v: Option<Vec<f64>> = (0..4).map(|k| e.uint(d, i + k).map(f64::from)).collect();
            if let Some(v) = v
                && v.iter().all(|&x| x > 0.0 && x < 65535.0)
            {
                out.wb_rggb = Some([v[0], v[1], v[2], v[3]]);
            }
        }
        // sRAW reference levels: after the as-shot entry come records of five values (a colour
        // temperature, then R G G B); the one with green 1170 is the sRAW reference.
        let first = match count {
            582 => 25,
            653 => 34,
            5120 => 71,
            _ => 63,
        } + 9;
        out.sraw_levels = (0..256)
            .map(|k| first + 5 * k)
            .take_while(|&i| i + 5 <= e.count as usize)
            .find_map(|i| {
                let v: Option<Vec<u32>> = (1..5).map(|k| e.uint(d, i + k)).collect();
                v.filter(|v| v[1] == 1170 && [v[0], v[3]].iter().all(|x| (1..65536).contains(x)))
                    .map(|v| [v[0], v[1], v[3]])
            });
        // Black / white levels: offsets relative to the as-shot WB entry, per ColorData version.
        // ColorData12 (R1, R5 Mark II: 4528 entries; R50 V: 3778 with version 65) moved them
        // forward; ColorData11 shares the 3778 length.
        let cd12 = count == 4528 || count == 3778 && version == Some(65);
        let lv = match count {
            1816 | 1820 | 1824 | 1770 | 2024 | 3656 => idx.map(|i| (i + 0x2d1, i + 0x2d5)),
            // ColorData5 (G-series PowerShots, EOS M): (black, normal white) by version; -3
            // has only a specular white at 0x296.
            5120 if version == Some(-4) => Some((0x14d, 0x569)),
            5120 if version == Some(-3) => Some((0x108, 0x295)),
            _ if cd12 => idx.map(|i| (i + 0x116, i + 0x22b)),
            3973 | 3778 => idx.map(|i| (i + 0x213, i + 0x217)),
            // ColorData8 of the 1300D (version 14): levels at 0x22c / 0x230 (checked against
            // the file's own values: 2046 2045 2046 2046, specular white 12279).
            1353 => idx.map(|i| (i + 0x1ed, i + 0x1f1)),
            _ => None,
        };
        if let Some((b, wl)) = lv {
            let v: Option<Vec<u32>> = (0..4).map(|k| e.uint(d, b + k)).collect();
            // The specular white only checks that the levels were read from the right place.
            let white = e
                .uint(d, wl + 1)
                .filter(|&v| v > 0)
                .or_else(|| e.uint(d, wl));
            if let (Some(v), Some(white)) = (v, white)
                && v.iter().all(|&x| x > 0 && x < white)
                && white < 65536
            {
                out.black = Some([v[0], v[1], v[2], v[3]]);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Nikon

#[derive(Clone, Debug, Default)]
pub(crate) struct NikonInfo {
    /// WB_RBLevels: red and blue multipliers.
    pub wb_rb: Option<(f64, f64)>,
    /// NEFLinearizationTable (Huffman decoder setup + curve).
    pub lin_table: Option<Range<usize>>,
    /// NEFCompression (1 lossy type 1, 2 uncompressed, 3 lossless, 4 lossy type 2, ...).
    pub compression: Option<u32>,
    /// Black level per channel (R, G, G, B order as stored).
    pub black: Option<[u32; 4]>,
    /// PreviewIFD.
    pub preview_ifd: Option<Ifd>,
}

pub(crate) fn nikon(d: &[u8], mn: &Ifd) -> NikonInfo {
    let mut out = NikonInfo::default();
    let wb = mn.floats(d, 0x000c, 4);
    if wb.len() >= 2 && wb[0] > 0.0 && wb[1] > 0.0 && wb[0] < 16.0 && wb[1] < 16.0 {
        out.wb_rb = Some((wb[0], wb[1]));
    }
    if let Some(e) = mn.get(0x0096) {
        out.lin_table = Some(e.pos..e.pos + e.byte_len());
    }
    if out.wb_rb.is_none() {
        out.wb_rb = nikon_color_balance(d, mn).or_else(|| nikon_nrw_wb(d, mn));
    }
    // Newer bodies (Z 8, Z 9, Z f, ...) drop 0x0093 and keep NEFCompression as a u16 at byte
    // 10 of the MakerNotes0x51 block; that is where High Efficiency (13 = HE, 14 = HE*) shows.
    out.compression = mn.uint(d, 0x0093).or_else(|| {
        let b = mn.get(0x0051)?.bytes(d)?;
        Endian { le: mn.le }.u16(b, 10).map(u32::from)
    });
    let b = mn.uints(d, 0x003d, 4);
    if b.len() == 4 {
        out.black = Some([b[0], b[1], b[2], b[3]]);
    }
    out.preview_ifd = sub_ifd(d, mn, 0x0011);
    out
}

/// WB from a Coolpix NRW's NRWData block (0x0014, "NRW 0100" or "NRW 0101"): four 32-bit
/// levels R, G, G, B, with red and blue stored at half the greens' scale (as LibRaw and
/// exiftool read them: 430 512 510 567 on a P7000 is R 1.68, B 2.22).
fn nikon_nrw_wb(d: &[u8], mn: &Ifd) -> Option<(f64, f64)> {
    let b = mn.get(0x0014)?.bytes(d)?;
    if b.get(..4)? != b"NRW " {
        return None;
    }
    let at = if b.get(4..8)? == b"0100" { 1556 } else { 56 };
    let en = Endian { le: mn.le };
    let v = |i: usize| en.u32(b, at + 4 * i).map(f64::from);
    let (r, g1, g2, bl) = (v(0)?, v(1)?, v(2)?, v(3)?);
    let g = (g1 + g2) / 2.0;
    let (r, bl) = (2.0 * r / g, 2.0 * bl / g);
    (g > 0.0 && r > 0.0 && bl > 0.0 && r < 16.0 && bl < 16.0).then_some((r, bl))
}

/// WB from the unencrypted ColorBalance (0x0097) versions 0100, 0102 and 0103.
fn nikon_color_balance(d: &[u8], mn: &Ifd) -> Option<(f64, f64)> {
    let b = mn.get(0x0097)?.bytes(d)?;
    let ver = std::str::from_utf8(b.get(..4)?).ok()?.parse::<u32>().ok()?;
    let en = Endian { le: mn.le };
    let g = |i: usize| en.u16(b, i).map(f64::from);
    // (offset, index of R, G, B among four 16-bit values)
    let (at, r, gi, bl) = match ver {
        100 => (72, 0, 2, 1),
        102 => (10, 0, 1, 3),
        103 => (20, 0, 1, 2),
        _ => return None,
    };
    let (r, gg, bb) = (g(at + 2 * r)?, g(at + 2 * gi)?, g(at + 2 * bl)?);
    (r > 0.0 && gg > 0.0 && bb > 0.0).then(|| (r / gg, bb / gg))
}

// ---------------------------------------------------------------------------------------------
// Sony SR2 (encrypted private IFD referenced from IFD0's DNGPrivateData)

#[derive(Clone, Debug, Default)]
pub(crate) struct SonySr2 {
    pub black: Option<[u32; 4]>,
    /// R G G B.
    pub wb_rggb: Option<[f64; 4]>,
    pub white: Option<u32>,
}

/// Sony's SR2 key stream (as documented by dcraw/exiftool): a 127-word lagged generator seeded
/// with `key`; each 32-bit data word is XORed with the next pad word (big-endian bytes).
pub(crate) fn sony_decrypt(buf: &mut [u8], key: u32) {
    let mut pad = [0u32; 128];
    let mut k = key;
    for p in pad.iter_mut().take(4) {
        k = k.wrapping_mul(48828125).wrapping_add(1);
        *p = k;
    }
    pad[3] = (pad[3] << 1) | ((pad[0] ^ pad[2]) >> 31);
    for p in 4..127 {
        pad[p] = ((pad[p - 4] ^ pad[p - 2]) << 1) | ((pad[p - 3] ^ pad[p - 1]) >> 31);
    }
    let mut p: usize = 127;
    for w in buf.chunks_exact_mut(4) {
        p += 1;
        let v = pad[p & 127] ^ pad[(p + 64) & 127];
        pad[(p - 1) & 127] = v;
        for (b, k) in w.iter_mut().zip(v.to_be_bytes()) {
            *b ^= k;
        }
    }
}

/// A 32-bit value stored as LONG or (on some Sony bodies) as BYTE[4] / UNDEFINED[4] in the
/// file's byte order.
fn u32_field(d: &[u8], e: &Entry) -> Option<u32> {
    if matches!(e.typ, 1 | 7) && e.count == 4 {
        Endian { le: e.le }.u32(d, e.pos)
    } else {
        e.uint(d, 0)
    }
}

pub(crate) fn sony_sr2(d: &[u8], ifd0: &Ifd) -> Option<SonySr2> {
    let e = ifd0.get(tiff::tag::DNG_PRIVATE_DATA)?;
    let raw_off = u32_field(d, e)?;
    let off = ifd0.base.checked_add(raw_off as usize)?;
    let sr2 = read_ifd(d, off, ifd0.base, ifd0.le)?;
    let sub_off = ifd0.base.checked_add(sr2.uint(d, 0x7200)? as usize)?;
    let len = sr2.uint(d, 0x7201)? as usize;
    let key = u32_field(d, sr2.get(0x7221)?)?;
    if len == 0 || len > (1 << 20) {
        return None;
    }
    let mut buf = d.get(sub_off..sub_off.checked_add(len)?)?.to_vec();
    sony_decrypt(&mut buf, key);
    let ifd = read_ifd_with(&buf, 0, ifd0.le, &|o| (o as usize).checked_sub(sub_off))?;
    let mut out = SonySr2::default();
    for tag in [0x7310, 0x7300] {
        let b = ifd.uints(&buf, tag, 4);
        if b.len() == 4 {
            out.black = Some([b[0], b[1], b[2], b[3]]);
            break;
        }
    }
    // 0x7313 WB_RGGBLevels, 0x7303 WB_GRBGLevels (stored G R B G).
    let w = ifd.floats(&buf, 0x7313, 4);
    if w.len() == 4 && w.iter().all(|&v| v > 0.0) {
        out.wb_rggb = Some([w[0], w[1], w[2], w[3]]);
    } else {
        let w = ifd.floats(&buf, 0x7303, 4);
        if w.len() == 4 && w.iter().all(|&v| v > 0.0) {
            out.wb_rggb = Some([w[1], w[0], w[3], w[2]]);
        }
    }
    // 0x7800 ColorMatrix is not read: it is the camera's own rendering matrix into its
    // selected output space, not a calibration. On an ILCE-7RM5 set to Adobe RGB it is close
    // to identity (mean ΔE00 6.7 against LibRaw, blue twice as strong; 0.6 with the generic
    // matrix), and even on an sRGB DSC-RX100M7 the generic matrix agrees better (0.9 vs 1.4).
    // LibRaw ignores it as well.
    out.white = ifd.uint(&buf, 0x787f).filter(|&w| w > 0);
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// Olympus

#[derive(Clone, Debug, Default)]
pub(crate) struct OlympusInfo {
    pub preview: Option<Range<usize>>,
    pub wb_rb: Option<(f64, f64)>,
    pub black: Option<[u32; 4]>,
}

pub(crate) fn olympus(d: &[u8], mn: &Ifd) -> OlympusInfo {
    let mut out = OlympusInfo::default();
    if let Some(cs) = sub_ifd(d, mn, 0x2020) {
        let valid = cs.uint(d, 0x0100).unwrap_or(1);
        if let (Some(s), Some(l)) = (cs.uint(d, 0x0101), cs.uint(d, 0x0102))
            && valid != 0
            && l > 0
        {
            let s = mn.base.saturating_add(s as usize);
            out.preview = Some(s..s.saturating_add(l as usize));
        }
    }
    if let Some(ip) = sub_ifd(d, mn, 0x2040) {
        let wb = ip.floats(d, 0x0100, 2);
        if wb.len() == 2 && wb[0] > 0.0 && wb[1] > 0.0 {
            out.wb_rb = Some((wb[0] / 256.0, wb[1] / 256.0));
        }
        let b = ip.uints(d, 0x0600, 4);
        if b.len() == 4 {
            out.black = Some([b[0], b[1], b[2], b[3]]);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Pentax

#[derive(Clone, Debug, Default)]
pub(crate) struct PentaxInfo {
    pub preview: Option<Range<usize>>,
    pub black: Option<[u32; 4]>,
    pub wb_rggb: Option<[f64; 4]>,
    pub huff_table: Option<Range<usize>>,
    /// ImageAreaOffset (0x0038) and RawImageSize (0x0039): the picture inside the sensor
    /// data, x, y, width, height.
    pub area: Option<[u32; 4]>,
}

pub(crate) fn pentax(d: &[u8], mn: &Ifd) -> PentaxInfo {
    let mut out = PentaxInfo::default();
    if let (Some(l), Some(s)) = (mn.uint(d, 0x0003), mn.uint(d, 0x0004))
        && l > 0
    {
        let s = mn.base.saturating_add(s as usize);
        out.preview = Some(s..s.saturating_add(l as usize));
    }
    let b = mn.uints(d, 0x0200, 4);
    if b.len() == 4 {
        out.black = Some([b[0], b[1], b[2], b[3]]);
    }
    let w = mn.floats(d, 0x0201, 4);
    if w.len() == 4 && w.iter().all(|&v| v > 0.0) {
        out.wb_rggb = Some([w[0], w[1], w[2], w[3]]);
    }
    if let Some(e) = mn.get(0x0220) {
        out.huff_table = Some(e.pos..e.pos + e.byte_len());
    }
    let (o, z) = (mn.uints(d, 0x0038, 2), mn.uints(d, 0x0039, 2));
    if o.len() == 2 && z.len() == 2 && z[0] > 0 && z[1] > 0 {
        out.area = Some([o[0], o[1], z[0], z[1]]);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Embedded lens corrections (Sony ARW, Fujifilm RAF)

/// Whose lens model a [`LensCorrection`] holds; the values mean different things per maker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LensMaker {
    /// Sony ARW raw-IFD tags 0x7032 VignettingCorrParams, 0x7035
    /// ChromaticAberrationCorrParams, 0x7037 DistortionCorrParams (int16s, a count then
    /// the knots). Knots are spread evenly from the centre (0) to the corner (1). The
    /// distortion factor is `1 + v / 2^14` and the colour factors `1 + v / 2^21`, both radial
    /// scales from the corrected position to where the lens put it (published by Y. Galka,
    /// stannum.io, and the pixls.us thread "Sony RAW chromatic aberration correction model").
    Sony,
    /// Fujifilm RAF FujiIFD tags 0xf00b GeometricDistortionParams, 0xf00f
    /// ChromaticAberrationParams, 0xf010 VignettingParams (rationals: a scale, n knot radii,
    /// then n values per table). The last knot is the corner; `scale × n` is the half
    /// diagonal in pixels. Distortion in percent, looked up at the radius where the lens put
    /// a point; vignetting in percent of the centre's illumination; colour as relative radial
    /// shifts of red and blue.
    Fujifilm,
}

/// A raw file's own lens correction data (maker notes), as stored. Radii are fractions of
/// the half diagonal of the recorded image (the crop), measured from its centre.
#[derive(Clone, Debug, PartialEq)]
pub struct LensCorrection {
    pub maker: LensMaker,
    /// Knot radii, ascending, the last 1 (the corner).
    pub knots: Vec<f64>,
    /// One value per knot (maker units, see [`LensMaker`]); `None`: not recorded.
    pub distortion: Option<Vec<f64>>,
    pub vignetting: Option<Vec<f64>>,
    pub ca_red: Option<Vec<f64>>,
    pub ca_blue: Option<Vec<f64>>,
    /// Fujifilm: `scale × knots` in pixels (the half diagonal the camera assumed).
    pub half_diagonal_px: Option<f64>,
}

impl LensCorrection {
    /// Whether any table is present.
    pub fn is_empty(&self) -> bool {
        self.distortion.is_none()
            && self.vignetting.is_none()
            && self.ca_red.is_none()
            && self.ca_blue.is_none()
    }
}

/// A Sony table: `count, v0 .. v(count-1)` (int16s), the count 2..=16 per table part.
fn sony_table(d: &[u8], raw: &Ifd, tag: u16, parts: usize) -> Option<Vec<f64>> {
    let v: Vec<i16> = raw
        .uints(d, tag, 1 + 16 * parts)
        .into_iter()
        .map(|u| u as u16 as i16)
        .collect();
    let n = usize::try_from(*v.first()?).ok()?;
    if n < 2 * parts || n > 16 * parts || n % parts != 0 || v.len() < 1 + n {
        return None;
    }
    Some(v[1..=n].iter().map(|&x| f64::from(x)).collect())
}

/// Sony's lens correction tables from the ARW raw IFD.
pub(crate) fn sony_lens(d: &[u8], raw: &Ifd) -> Option<LensCorrection> {
    let distortion = sony_table(d, raw, 0x7037, 1);
    let vignetting = sony_table(d, raw, 0x7032, 1);
    let ca = sony_table(d, raw, 0x7035, 2);
    let n = distortion
        .as_ref()
        .or(vignetting.as_ref())
        .map(Vec::len)
        .or(ca.as_ref().map(|c| c.len() / 2))?;
    let fits = |t: &Option<Vec<f64>>| t.as_ref().is_none_or(|t| t.len() == n);
    if !fits(&distortion) || !fits(&vignetting) || ca.as_ref().is_some_and(|c| c.len() != 2 * n) {
        return None;
    }
    let (ca_red, ca_blue) = match ca {
        Some(c) => (Some(c[..n].to_vec()), Some(c[n..].to_vec())),
        None => (None, None),
    };
    let out = LensCorrection {
        maker: LensMaker::Sony,
        knots: (0..n).map(|i| i as f64 / (n - 1) as f64).collect(),
        distortion,
        vignetting,
        ca_red,
        ca_blue,
        half_diagonal_px: None,
    };
    (!out.is_empty()).then_some(out)
}

/// Fujifilm's lens correction tables from the RAF's FujiIFD.
pub(crate) fn fuji_lens(d: &[u8], ifd: &Ifd) -> Option<LensCorrection> {
    // Each table: scale, n knots, n values (red and blue: 2n values, plus a trailing copy of
    // the scale on the bodies seen).
    let read = |tag: u16, parts: usize| -> Option<(f64, Vec<f64>, Vec<f64>)> {
        let v = ifd.floats(d, tag, 64);
        let scale = *v.first()?;
        // n from the count: 1 + n + parts * n (+ 1).
        let n = [0usize, 1]
            .iter()
            .find_map(|&extra| {
                let body = v.len().checked_sub(1 + extra)?;
                (body % (1 + parts) == 0).then_some(body / (1 + parts))
            })
            .filter(|&n| (2..=20).contains(&n))?;
        let knots = v[1..=n].to_vec();
        let ok = scale.is_finite()
            && scale > 0.0
            && knots.iter().all(|k| k.is_finite() && *k > 0.0)
            && knots.windows(2).all(|w| w[0] < w[1]);
        ok.then(|| (scale, knots, v[1 + n..1 + n + parts * n].to_vec()))
    };
    let dist = read(0xf00b, 1);
    let vig = read(0xf010, 1);
    let ca = read(0xf00f, 2);
    let (scale, knots) = [&dist, &vig, &ca]
        .into_iter()
        .flatten()
        .map(|(s, k, _)| (*s, k.clone()))
        .next()?;
    let same = |t: &Option<(f64, Vec<f64>, Vec<f64>)>| {
        t.as_ref()
            .and_then(|(_, k, v)| (*k == knots).then(|| v.clone()))
    };
    let last = *knots.last()?;
    let n = knots.len();
    let ca = same(&ca);
    let out = LensCorrection {
        maker: LensMaker::Fujifilm,
        knots: knots.iter().map(|k| k / last).collect(),
        distortion: same(&dist),
        vignetting: same(&vig),
        ca_red: ca.as_ref().map(|c| c[..n].to_vec()),
        ca_blue: ca.as_ref().map(|c| c[n..].to_vec()),
        half_diagonal_px: Some(scale * n as f64),
    };
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A little-endian IFD at 0 with one UNDEFINED entry `tag` whose bytes follow it.
    fn one_entry(tag: u16, bytes: &[u8]) -> Vec<u8> {
        let mut d = vec![1, 0];
        d.extend_from_slice(&tag.to_le_bytes());
        d.extend_from_slice(&7u16.to_le_bytes());
        d.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        d.extend_from_slice(&18u32.to_le_bytes());
        d.extend_from_slice(&[0; 4]);
        d.extend_from_slice(bytes);
        d
    }

    /// Coolpix P7000: NRWData "NRW 0101", levels 430 512 510 567 at byte 56 (LibRaw and
    /// exiftool: R 860/512, B 1134/512 of the greens' mean).
    #[test]
    fn coolpix_nrw_white_balance() {
        let mut b = vec![0u8; 384];
        b[..8].copy_from_slice(b"NRW 0101");
        for (i, v) in [430u32, 512, 510, 567].iter().enumerate() {
            b[56 + 4 * i..60 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        let d = one_entry(0x0014, &b);
        let mn = read_ifd(&d, 0, 0, true).unwrap();
        let (r, bl) = nikon(&d, &mn).wb_rb.unwrap();
        assert!((r - 860.0 / 511.0).abs() < 1e-9 && (bl - 1134.0 / 511.0).abs() < 1e-9);
        // Version 0100 keeps them at byte 1556.
        let mut b = vec![0u8; 1600];
        b[..8].copy_from_slice(b"NRW 0100");
        for (i, v) in [400u32, 500, 500, 600].iter().enumerate() {
            b[1556 + 4 * i..1560 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        let d = one_entry(0x0014, &b);
        let mn = read_ifd(&d, 0, 0, true).unwrap();
        assert_eq!(nikon(&d, &mn).wb_rb, Some((1.6, 2.4)));
        // Not NRWData.
        let d = one_entry(0x0014, &[0u8; 64]);
        let mn = read_ifd(&d, 0, 0, true).unwrap();
        assert_eq!(nikon(&d, &mn).wb_rb, None);
    }

    /// A little-endian IFD at 0 with entries (tag, type, count, value bytes), values after it.
    fn ifd(entries: &[(u16, u16, u32, Vec<u8>)]) -> Vec<u8> {
        let n = entries.len();
        let mut d = (n as u16).to_le_bytes().to_vec();
        let mut data_at = 2 + 12 * n + 4;
        let mut data = Vec::new();
        for (tag, typ, count, bytes) in entries {
            d.extend_from_slice(&tag.to_le_bytes());
            d.extend_from_slice(&typ.to_le_bytes());
            d.extend_from_slice(&count.to_le_bytes());
            if bytes.len() <= 4 {
                let mut v = bytes.clone();
                v.resize(4, 0);
                d.extend_from_slice(&v);
            } else {
                d.extend_from_slice(&(data_at as u32).to_le_bytes());
                data.extend_from_slice(bytes);
                data_at += bytes.len();
            }
        }
        d.extend_from_slice(&[0; 4]);
        d.extend_from_slice(&data);
        d
    }

    fn sshorts(v: &[i16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn srationals(v: &[f64]) -> Vec<u8> {
        v.iter()
            .flat_map(|x| {
                let den = 1_000_000i32;
                let num = (x * f64::from(den)).round() as i32;
                [num.to_le_bytes(), den.to_le_bytes()].concat()
            })
            .collect()
    }

    /// The ILCE-6400 file's tables (raw IFD, as exiftool lists them).
    #[test]
    fn sony_lens_tables() {
        let dist = [
            11, 29, 29, 58, 144, 259, 417, 633, 891, 1222, 1639, 2156, 0, 0, 0, 0, 0,
        ];
        let vig = [
            11, 0, 320, 768, 1376, 2080, 2816, 3744, 4992, 6528, 8384, 10592, 0, 0, 0, 0, 0,
        ];
        let mut ca = vec![22i16];
        ca.extend([-1280i16; 11]);
        ca.extend([-640i16; 11]);
        ca.resize(33, 0);
        let d = ifd(&[
            (0x7032, 8, 17, sshorts(&vig)),
            (0x7035, 8, 33, sshorts(&ca)),
            (0x7037, 8, 17, sshorts(&dist)),
        ]);
        let raw = read_ifd(&d, 0, 0, true).unwrap();
        let l = sony_lens(&d, &raw).expect("tables");
        assert_eq!(l.maker, LensMaker::Sony);
        assert_eq!(l.knots.len(), 11);
        assert_eq!(l.knots[10], 1.0);
        assert_eq!(l.distortion.as_ref().unwrap()[10], 2156.0);
        assert_eq!(l.vignetting.as_ref().unwrap()[1], 320.0);
        assert_eq!(l.ca_red.as_ref().unwrap(), &vec![-1280.0; 11]);
        assert_eq!(l.ca_blue.as_ref().unwrap(), &vec![-640.0; 11]);
        // A count that does not fit is refused.
        let mut bad = dist;
        bad[0] = 17;
        let d = ifd(&[(0x7037, 8, 17, sshorts(&bad))]);
        assert_eq!(sony_lens(&d, &read_ifd(&d, 0, 0, true).unwrap()), None);
    }

    /// The X-T4 file's tables (FujiIFD, as exiftool lists them).
    #[test]
    fn fujifilm_lens_tables() {
        let scale = 416.5555556;
        let knots: Vec<f64> = (1..=9).map(|k| (f64::from(k) / 8.0).sqrt()).collect();
        let dist = [
            -1.17, -2.35, -3.46, -4.43, -5.27, -5.92, -6.33, -6.43, -6.43,
        ];
        let vig = [98.5, 96.9, 94.9, 91.0, 84.7, 78.4, 73.0, 67.5, 67.5];
        let red = [-3e-5; 9];
        let blue = [7e-4; 9];
        let t = |vals: &[&[f64]], trailing: bool| {
            let mut v = vec![scale];
            v.extend(&knots);
            for x in vals {
                v.extend(*x);
            }
            if trailing {
                v.push(scale);
            }
            v
        };
        let (dt, vt, ct) = (
            t(&[&dist], false),
            t(&[&vig], false),
            t(&[&red, &blue], true),
        );
        let d = ifd(&[
            (0xf00b, 10, dt.len() as u32, srationals(&dt)),
            (0xf00f, 10, ct.len() as u32, srationals(&ct)),
            (0xf010, 10, vt.len() as u32, srationals(&vt)),
        ]);
        let f = read_ifd(&d, 0, 0, true).unwrap();
        let l = fuji_lens(&d, &f).expect("tables");
        assert_eq!(l.maker, LensMaker::Fujifilm);
        assert_eq!(l.knots.len(), 9);
        assert!((l.knots[8] - 1.0).abs() < 1e-12);
        assert!((l.knots[0] - (1.0f64 / 9.0).sqrt()).abs() < 1e-6);
        assert!((l.distortion.as_ref().unwrap()[0] + 1.17).abs() < 1e-6);
        assert!((l.vignetting.as_ref().unwrap()[8] - 67.5).abs() < 1e-6);
        assert!((l.ca_blue.as_ref().unwrap()[3] - 7e-4).abs() < 1e-6);
        assert!((l.half_diagonal_px.unwrap() - scale * 9.0).abs() < 1e-3);
    }

    #[test]
    fn sony_decrypt_is_an_involution() {
        let orig: Vec<u8> = (0..400u32).map(|i| (i * 7) as u8).collect();
        let mut b = orig.clone();
        sony_decrypt(&mut b, 0x1234_5678);
        assert_ne!(b, orig);
        sony_decrypt(&mut b, 0x1234_5678);
        assert_eq!(b, orig);
    }
}
