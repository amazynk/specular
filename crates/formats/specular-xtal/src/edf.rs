//! ESRF Data Format (EDF): each frame is a `{ … }` text header of `key = value ;`
//! statements followed by its binary block (`Size` or `EDF_BinarySize` bytes). Headers are
//! usually space-padded to a multiple of 512 bytes and end with `}` and a newline. A file may
//! hold many frames; a leading block without data (`EDF_DataFormatVersion`, no `Dim_1`) is the
//! "general block" whose keys apply to every frame.
//!
//! Geometry comes from the SAXS keys of the ESRF beamlines (`Center_1/2` in pixels,
//! `PSize_1/2`, `SampleDistance` and `WaveLength` in metres, `Offset_1/2`) or from the
//! Oxford Diffraction keys (`Beam center x/y`, `Real pixel size x/y (mm)`, `Distance in mm`).
//! A nonzero `Dummy` marks pixels without data (within `DDummy`).

use crate::geometry::Geometry;
use crate::{Coding, Error, Frame, Image, Kind, Result, Sample, bad, finite, positive, smv, text};

/// How a frame's block is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
    Gzip,
    Zlib,
    Bzip2,
}

/// Whether the file start is an EDF header: optional whitespace, `{`, then a
/// `key = value ;` statement.
pub fn sniff(head: &[u8]) -> bool {
    let start = head.iter().position(|b| !b.is_ascii_whitespace());
    let Some(s) = start.filter(|&s| head[s] == b'{') else {
        return false;
    };
    let body = &head[s + 1..head.len().min(s + 1 + 4096)];
    // The first statement: printable text, an `=`, and a `;` (not SMV's `HEADER_BYTES=`).
    let Some(semi) = body.iter().position(|&b| b == b';') else {
        return false;
    };
    let stmt = &body[..semi];
    !smv::sniff(head)
        && stmt.contains(&b'=')
        && stmt
            .iter()
            .all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace())
}

/// One header block: its statements, where its data starts.
struct Block {
    pairs: Vec<(String, String)>,
    data_start: usize,
}

/// The header block starting at or after `at` (whitespace before `{` skipped).
fn block(data: &[u8], at: usize) -> Option<Block> {
    let rest = data.get(at..)?;
    let s = rest.iter().position(|b| !b.is_ascii_whitespace())?;
    if rest[s] != b'{' {
        return None;
    }
    let body_start = at + s + 1;
    // The header ends at the first `}` followed by a newline (or the file end).
    let mut i = body_start;
    let end = loop {
        let rel = data.get(i..)?.iter().position(|&b| b == b'}')?;
        let close = i + rel;
        match data.get(close + 1) {
            Some(b'\n') => break (close, close + 2),
            Some(b'\r') if data.get(close + 2) == Some(&b'\n') => break (close, close + 3),
            None => break (close, close + 1),
            _ => i = close + 1,
        }
    };
    let text = String::from_utf8_lossy(&data[body_start..end.0]);
    let pairs = text
        .split(';')
        .filter_map(|st| {
            let (k, v) = st.split_once('=')?;
            let k = k.trim();
            (!k.is_empty()).then(|| (k.to_owned(), v.trim().to_owned()))
        })
        .collect();
    Some(Block {
        pairs,
        data_start: end.1,
    })
}

fn get<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .rev()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.as_str())
}

fn sample(t: &str) -> Option<Sample> {
    Some(match t.to_ascii_lowercase().as_str() {
        "signedbyte" | "signed8" | "signedchar" => Sample::I8,
        "unsignedbyte" | "unsigned8" | "unsignedchar" => Sample::U8,
        "signedshort" | "signed16" | "signedshortinteger" | "short" => Sample::I16,
        "unsignedshort" | "unsigned16" | "unsignedshortinteger" => Sample::U16,
        "signedinteger" | "signed32" | "signedlong" | "signedlonginteger" | "integer" | "long" => {
            Sample::I32
        }
        "unsignedinteger" | "unsigned32" | "unsignedlong" | "unsignedlonginteger" => Sample::U32,
        "signed64" | "signedlonglong" => Sample::I64,
        "unsigned64" | "unsignedlonglong" => Sample::U64,
        "floatvalue" | "float" | "floatieee32" | "float32" | "real" => Sample::F32,
        "double" | "doublevalue" | "floatieee64" | "doubleieee64" | "float64" => Sample::F64,
        _ => return None,
    })
}

/// Whether a `ByteOrder` value is big-endian (`HighByteFirst`).
fn big_endian(v: &str) -> bool {
    let v = v.trim();
    v.eq_ignore_ascii_case("HighByteFirst") || v.eq_ignore_ascii_case("BigEndian")
}

/// How a block's data are coded.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    None,
    Packed(Packing),
    /// CBF byte-offset deltas (fabio's `BYTE_OFFSET`).
    ByteOffset,
}

/// Slices of a volume block (`Dim_3`) read at most.
const MAX_SLICES: u64 = 1 << 16;

/// A block's frames from its (general block merged) header: width, height, sample type,
/// one coding per frame (each slice of a `Dim_3` volume is a frame) and the block's length.
fn coding(
    pairs: &[(String, String)],
    data_start: usize,
    file_len: usize,
) -> Result<(u32, u32, Sample, Vec<Coding>, usize)> {
    let dim = |k: &str| get(pairs, k).and_then(|v| v.trim().parse::<u64>().ok());
    let w = dim("Dim_1").ok_or_else(|| bad("no Dim_1"))?;
    let h = dim("Dim_2").unwrap_or(1);
    let d3 = dim("Dim_3").unwrap_or(1).max(1);
    if d3 > MAX_SLICES {
        return Err(Error::Unsupported(format!("EDF volume of {d3} slices")));
    }
    if dim("Dim_4").is_some_and(|d| d > 1) {
        return Err(Error::Unsupported("EDF data in four dimensions".into()));
    }
    let w = u32::try_from(w).map_err(|_| bad("Dim_1"))?;
    let h = u32::try_from(h).map_err(|_| bad("Dim_2"))?;
    let t = get(pairs, "DataType").unwrap_or("UnsignedShort");
    let sample = sample(t).ok_or_else(|| Error::Unsupported(format!("EDF data type \"{t}\"")))?;
    let big = get(pairs, "ByteOrder").is_some_and(big_endian);
    let slice_len = (u64::from(w) * u64::from(h)).saturating_mul(sample.bytes() as u64);
    let raw_len = slice_len.saturating_mul(d3);
    let size = get(pairs, "Size")
        .or_else(|| get(pairs, "EDF_BinarySize"))
        .and_then(|v| v.trim().parse::<u64>().ok());
    let method = match get(pairs, "Compression").map(str::to_ascii_lowercase) {
        None => Method::None,
        Some(c) => match c.as_str() {
            "none" | "nocompression" | "" | "raw" => Method::None,
            "gzipcompression" | "gzip" | "gz" => Method::Packed(Packing::Gzip),
            "zcompression" | "z" | "zlib" | "zlibcompression" => Method::Packed(Packing::Zlib),
            "bzipcompression" | "bzip2compression" | "bz2" | "bzip2" | "bz" => {
                Method::Packed(Packing::Bzip2)
            }
            _ if c.contains("offset") => Method::ByteOffset,
            _ if c.starts_with("no") => Method::None,
            // Other names (Oxford "TY1", …) are fine when the block holds every sample.
            _ if size.is_none_or(|s| s == raw_len) => Method::None,
            _ => return Err(Error::Unsupported(format!("EDF compression \"{c}\""))),
        },
    };
    if method == Method::ByteOffset && matches!(sample, Sample::F32 | Sample::F64) {
        return Err(Error::Unsupported("byte-offset coded floats".into()));
    }
    let avail = file_len.saturating_sub(data_start) as u64;
    let len = match (size, method) {
        (Some(s), _) => s,
        (None, Method::None) => raw_len,
        (None, _) => avail,
    };
    let len = usize::try_from(len).map_err(|_| bad("block size"))?;
    let slice = usize::try_from(slice_len).map_err(|_| bad("frame size"))?;
    let pixels = (w as usize).saturating_mul(h as usize);
    let codings = (0..d3 as usize)
        .map(|k| match method {
            Method::None => Coding::Raw {
                offset: data_start.saturating_add(k.saturating_mul(slice)),
                sample,
                big,
            },
            Method::Packed(m) => Coding::Packed {
                offset: data_start,
                len,
                method: m,
                sample,
                big,
                skip: k.saturating_mul(slice),
            },
            Method::ByteOffset => Coding::ByteOffset {
                offset: data_start,
                len,
                sample,
                skip: k.saturating_mul(pixels),
            },
        })
        .collect();
    Ok((w, h, sample, codings, len))
}

/// Binning written as text: "2x2", "2 x 2", "2 2" or "2".
pub(crate) fn binning_text(v: &str) -> Option<(u32, u32)> {
    let n: Vec<f64> = crate::numbers(&v.replace(['x', 'X', '*'], " ")).collect();
    let ok = |b: f64| ((1.0..=1024.0).contains(&b) && b.fract() == 0.0).then_some(b as u32);
    match n.as_slice() {
        [x] => Some((ok(*x)?, ok(*x)?)),
        [x, y, ..] => Some((ok(*x)?, ok(*y)?)),
        _ => None,
    }
}

pub(crate) fn parse(data: &[u8]) -> Result<Image> {
    let mut general: Vec<(String, String)> = Vec::new();
    let mut at = 0usize;
    let mut frames: Vec<Frame> = Vec::new();
    // The first frame's width, height, sample type and header.
    type First = (u32, u32, Sample, Vec<(String, String)>);
    let mut first: Option<First> = None;
    while let Some(b) = block(data, at) {
        let has_dims = get(&b.pairs, "Dim_1").is_some();
        if !has_dims && frames.is_empty() && first.is_none() {
            // The general block: defaults for every frame; its data (if any) skipped.
            let size = get(&b.pairs, "EDF_BinarySize")
                .or_else(|| get(&b.pairs, "Size"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            general = b.pairs;
            at = b.data_start.saturating_add(size);
            continue;
        }
        let mut pairs = general.clone();
        pairs.extend(b.pairs.iter().cloned());
        let parsed = coding(&pairs, b.data_start, data.len());
        let (w, h, sample, codings, len) = match parsed {
            Ok(x) => x,
            // A broken later frame ends the series; a broken first frame is the error.
            Err(e) if first.is_none() => return Err(e),
            Err(_) => break,
        };
        match &first {
            None => first = Some((w, h, sample, pairs)),
            Some((fw, fh, _, _)) if (*fw, *fh) != (w, h) => break,
            Some(_) => {}
        }
        // The first block's header is the image's; a later block's goes with its first frame.
        let mut own = (!frames.is_empty()).then_some(b.pairs);
        for c in codings {
            frames.push(Frame {
                coding: c,
                header: own.take().unwrap_or_default(),
            });
        }
        let next = b.data_start.saturating_add(len);
        if next >= data.len() {
            break;
        }
        at = next;
    }
    let (w, h, sample, pairs) = first.ok_or_else(|| bad("no EDF frame"))?;
    // A truncated last frame is dropped (the first is kept and fails on decode).
    while frames.len() > 1
        && frames
            .last()
            .is_some_and(|f| frame_end(&f.coding, w, h) > data.len())
    {
        frames.pop();
    }
    let num = |k: &str| get(&pairs, k).and_then(crate::first_number);
    let mut g = Geometry::default();
    // ESRF SAXS keys (metres, pixel coordinates with offsets).
    let off = (
        num("Offset_1").unwrap_or(0.0),
        num("Offset_2").unwrap_or(0.0),
    );
    if let (Some(c1), Some(c2)) = (finite(num("Center_1")), finite(num("Center_2"))) {
        g.beam = Some((c1 - off.0, c2 - off.1));
    }
    if let (Some(p1), Some(p2)) = (positive(num("PSize_1")), positive(num("PSize_2"))) {
        g.pixel = Some((p1 * 1000.0, p2 * 1000.0));
    }
    g.distance = positive(num("SampleDistance")).map(|d| d * 1000.0);
    g.wavelength = positive(num("WaveLength")).map(|l| l * 1e10);
    // Oxford Diffraction keys (millimetres, pixels, Å).
    if g.beam.is_none()
        && let (Some(x), Some(y)) = (
            positive(num("Beam center x")),
            positive(num("Beam center y")),
        )
    {
        g.beam = Some((x, y));
    }
    if g.pixel.is_none()
        && let (Some(x), Some(y)) = (
            positive(num("Real pixel size x (mm)")),
            positive(num("Real pixel size y (mm)")),
        )
    {
        g.pixel = Some((x, y));
    }
    if g.distance.is_none() {
        g.distance = positive(num("Distance in mm"));
    }
    if g.wavelength.is_none() {
        g.wavelength = positive(num("Wavelength alpha1")).or(positive(num("Wavelength alpha")));
    }
    g.exposure = positive(num("count_time")).or(positive(num("Exposure time in sec")));
    let dummy = finite(num("Dummy"))
        .filter(|d| *d != 0.0)
        .map(|d| (d, finite(num("DDummy")).unwrap_or(0.0).abs()));
    // Detector binning (ESRF `BSize_1/2`, else a "Binning" line).
    let bin = |k: &str| {
        num(k)
            .filter(|b| (1.0..=1024.0).contains(b) && b.fract() == 0.0)
            .map(|b| b as u32)
    };
    g.binning = match (bin("BSize_1"), bin("BSize_2")) {
        (Some(x), Some(y)) => Some((x, y)),
        (Some(x), None) => Some((x, 1)),
        _ => get(&pairs, "Binning").and_then(binning_text),
    };
    let first_text = |keys: &[&str]| keys.iter().find_map(|k| get(&pairs, k).and_then(text));
    let date = first_text(&["Date", "Time", "DATE (scan begin)", "time_of_day"]);
    let title = first_text(&["Title"]);
    let detector = first_text(&["Detector type", "Detector", "DetectorName", "Detector name"]);
    Ok(Image {
        kind: Kind::Edf,
        width: w,
        height: h,
        sample,
        frames,
        header: pairs,
        geometry: g,
        detector,
        date,
        mask_negative: false,
        dummy,
        title,
    })
}

/// Where a frame's block ends.
fn frame_end(c: &Coding, w: u32, h: u32) -> usize {
    match *c {
        Coding::Raw { offset, sample, .. } => {
            // Dimensions are not capped yet: u32 × u32 × 8 overflows usize.
            let n = (w as usize).saturating_mul(h as usize);
            offset.saturating_add(n.saturating_mul(sample.bytes()))
        }
        Coding::Packed { offset, len, .. } | Coding::ByteOffset { offset, len, .. } => {
            offset.saturating_add(len)
        }
        _ => 0,
    }
}

/// Decompresses a frame block (at least `expected` bytes wanted).
pub(crate) fn unpack(src: &[u8], method: Packing, expected: usize) -> Result<Vec<u8>> {
    let max = expected.saturating_mul(2).clamp(1 << 20, crate::MAX_BLOCK);
    let is_gzip = src.starts_with(&[0x1f, 0x8b]);
    let is_bzip2 = src.starts_with(b"BZh");
    let fail = |e: &dyn std::fmt::Display| bad(format!("compressed frame: {e}"));
    // Go by the stream's own signature first: writers mix up the names.
    if is_gzip {
        return specular_png::inflate_gzip_prefix(src, expected).map_err(|e| fail(&e));
    }
    if is_bzip2 || method == Packing::Bzip2 {
        // Bounded like the other methods (a frame wants `expected` bytes, not the 4 GiB a
        // whole `.xcf.bz2` may hold).
        if !is_bzip2 {
            return Err(bad("compressed frame: not a bzip2 stream"));
        }
        return specular_xcf::bunzip2(src, max as u64).map_err(|e| fail(&e));
    }
    specular_png::inflate_zlib(src, max).map_err(|e| fail(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(lines: &[&str]) -> Vec<u8> {
        let mut t = String::from("{\n");
        for l in lines {
            t += l;
            t += " ;\n";
        }
        while (t.len() + 2) % 512 != 0 {
            t.push(' ');
        }
        t += "}\n";
        t.into_bytes()
    }

    #[test]
    fn multi_frame_with_geometry() {
        let mut d = header(&[
            "HeaderID = EH:000001:000000:000000",
            "Image = 1",
            "ByteOrder = HighByteFirst",
            "DataType = SignedShort",
            "Dim_1 = 3",
            "Dim_2 = 2",
            "Size = 12",
            "Dummy = -1",
            "DDummy = 0.1",
            "Center_1 = 1.5",
            "Center_2 = 1",
            "PSize_1 = 1e-4",
            "PSize_2 = 1e-4",
            "SampleDistance = 0.1",
            "WaveLength = 1e-10",
            "Title = test",
        ]);
        assert!(sniff(&d));
        for v in [1i16, -1, 3, 4, 5, 6] {
            d.extend(v.to_be_bytes());
        }
        d.extend(header(&[
            "Image = 2",
            "ByteOrder = HighByteFirst",
            "DataType = SignedShort",
            "Dim_1 = 3",
            "Dim_2 = 2",
            "Size = 12",
        ]));
        for v in [10i16, 20, 30, 40, 50, 60] {
            d.extend(v.to_be_bytes());
        }
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.kind, Kind::Edf);
        assert_eq!(im.frames.len(), 2);
        assert_eq!(im.title.as_deref(), Some("test"));
        let g = &im.geometry;
        assert_eq!(g.beam, Some((1.5, 1.0)));
        assert_eq!(g.distance, Some(100.0));
        assert!((g.wavelength.unwrap() - 1.0).abs() < 1e-12);
        assert!((g.pixel.unwrap().0 - 0.1).abs() < 1e-12);
        let f0 = im.frame(&d, 0).unwrap();
        assert!(f0[1].is_nan());
        assert_eq!(f0[5], 6.0);
        assert_eq!(im.frame(&d, 1).unwrap()[2], 30.0);
        assert!(
            im.frames[1]
                .header
                .iter()
                .any(|(k, v)| k == "Image" && v == "2")
        );
    }

    #[test]
    fn general_block_and_truncation() {
        let mut d = header(&["EDF_DataFormatVersion = 2.0", "DataType = UnsignedByte"]);
        d.extend(header(&["Dim_1 = 2", "Dim_2 = 2", "EDF_BinarySize = 4"]));
        d.extend([1, 2, 3, 4]);
        d.extend(header(&["Dim_1 = 2", "Dim_2 = 2", "EDF_BinarySize = 4"]));
        d.extend([5, 6]);
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.sample, Sample::U8);
        assert_eq!(im.frames.len(), 1);
        assert_eq!(im.frame(&d, 0).unwrap(), [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn volume_slices_are_frames() {
        let mut d = header(&[
            "ByteOrder = LowByteFirst",
            "DataType = UnsignedShort",
            "Dim_1 = 2",
            "Dim_2 = 2",
            "Dim_3 = 3",
            "Size = 24",
            "BSize_1 = 2",
            "BSize_2 = 4",
        ]);
        for v in 0u16..12 {
            d.extend(v.to_le_bytes());
        }
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.frames.len(), 3);
        assert_eq!(im.geometry.binning, Some((2, 4)));
        assert_eq!(im.frame(&d, 2).unwrap(), [8.0, 9.0, 10.0, 11.0]);
        // The same volume gzipped as one block, and byte-offset coded.
        let raw: Vec<u8> = (0u16..12).flat_map(|v| v.to_le_bytes()).collect();
        let gz = gzip(&raw);
        let mut z = header(&[
            "DataType = UnsignedShort",
            "Dim_1 = 2",
            "Dim_2 = 2",
            "Dim_3 = 3",
            &format!("Size = {}", gz.len()),
            "Compression = GzipCompression",
        ]);
        z.extend(&gz);
        let im = Image::parse(&z).unwrap();
        assert_eq!(im.frames.len(), 3);
        assert_eq!(im.frame(&z, 1).unwrap(), [4.0, 5.0, 6.0, 7.0]);
        let mut b = header(&[
            "DataType = SignedInteger",
            "Dim_1 = 2",
            "Dim_2 = 1",
            "Dim_3 = 2",
            "Size = 6",
            "Compression = BYTE_OFFSET",
        ]);
        b.extend([5u8, 1, 0x80, 0x10, 0x27, 0xfe]);
        let im = Image::parse(&b).unwrap();
        assert_eq!(im.compression(), "byte offset");
        assert_eq!(im.frame(&b, 0).unwrap(), [5.0, 6.0]);
        assert_eq!(im.frame(&b, 1).unwrap(), [10006.0, 10004.0]);
    }

    /// A gzip member around `raw`, stored (no compression), written here from RFC 1952.
    fn gzip(raw: &[u8]) -> Vec<u8> {
        let mut g = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
        g.push(1);
        g.extend((raw.len() as u16).to_le_bytes());
        g.extend((!(raw.len() as u16)).to_le_bytes());
        g.extend(raw);
        let mut crc = !0u32;
        for &b in raw {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        g.extend((!crc).to_le_bytes());
        g.extend((raw.len() as u32).to_le_bytes());
        g
    }

    #[test]
    fn binning_and_types() {
        assert_eq!(binning_text("2x2"), Some((2, 2)));
        assert_eq!(binning_text("1 x 4"), Some((1, 4)));
        assert_eq!(binning_text("none"), None);
        for (t, s) in [
            ("Signed64", Sample::I64),
            ("UnsignedLong", Sample::U32),
            ("DoubleIEEE64", Sample::F64),
            ("FLOAT", Sample::F32),
            ("Unsigned8", Sample::U8),
        ] {
            assert_eq!(sample(t), Some(s), "{t}");
        }
        assert_eq!(sample("FloatIEEE128"), None);
        assert!(big_endian("HighByteFirst") && !big_endian("LowByteFirst"));
    }

    #[test]
    fn not_edf() {
        assert!(!sniff(b"{\nHEADER_BYTES=512;\n"));
        assert!(!sniff(b"\x89PNG"));
        assert!(sniff(b"   {\r\nDim_1 = 2;"));
    }

    mod hostile {
        use super::*;

        #[test]
        fn frames_of_the_largest_dimensions() {
            let frame = || {
                header(&[
                    "DataType = DoubleValue",
                    "Dim_1 = 4294967295",
                    "Dim_2 = 4294967295",
                    "Size = 8",
                ])
            };
            let mut d = frame();
            d.extend([0; 8]);
            d.extend(frame());
            d.extend([0; 8]);
            assert!(parse(&d).is_ok());
            assert!(Image::parse(&d).is_err());
        }
    }
}
