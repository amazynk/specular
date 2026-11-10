//! CBF and miniCBF (imgCIF with a MIME binary section).
//!
//! The text part names the data block and, in miniCBF files from Pilatus and Eiger
//! detectors, carries the detector's header as `# Key value` lines inside
//! `_array_data.header_contents` (a `;`-delimited text field; a trailing `\` continues a
//! line). The binary section follows `--CIF-BINARY-FORMAT-SECTION--`: MIME header lines
//! (dimensions, element type, conversions), a blank line, the bytes `0C 1A 04 D5`, then
//! `X-Binary-Size` bytes of data. Byte-offset coding (`x-CBF_BYTE_OFFSET`) stores each pixel
//! as the difference from the previous one in 1, 2, 4 or 8 bytes (little-endian; the
//! values -128, -32768 and -2³¹ escape to the next width). `x-CBF_NONE` stores samples as
//! they are. Only the first binary section is read.

use crate::geometry::Geometry;
use crate::{Coding, Error, Frame, Image, Kind, Result, Sample, bad, finite, positive, text};

const SECTION: &[u8] = b"--CIF-BINARY-FORMAT-SECTION--";
const MARKER: [u8; 4] = [0x0c, 0x1a, 0x04, 0xd5];

/// Whether the file start is a CBF file: `###CBF`, or a binary section within the head.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"###CBF")
        || (find(head, SECTION).is_some()
            && (find(head, b"_array_data").is_some() || find(head, b"data_").is_some()))
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// The miniCBF header lines in `_array_data.header_contents`, continuation lines joined.
fn header_contents(text: &str) -> Vec<String> {
    let Some(i) = text.find("_array_data.header_contents") else {
        return Vec::new();
    };
    let rest = &text[i..];
    // The field is either on the same line (quoted) or between `;` lines.
    let mut lines = rest.lines().skip(1);
    let Some(first) = lines.next() else {
        return Vec::new();
    };
    if !first.starts_with(';') {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut cont = false;
    let first_tail = first[1..].trim_end();
    let body = std::iter::once(first_tail).chain(lines.map(str::trim_end));
    for l in body {
        if l.starts_with(';') {
            break;
        }
        let (l, more) = match l.strip_suffix('\\') {
            Some(x) => (x, true),
            None => (l, false),
        };
        if cont && let Some(last) = out.last_mut() {
            last.push_str(l.trim_start());
        } else if !l.trim().is_empty() {
            out.push(l.to_owned());
        }
        cont = more;
    }
    out
}

/// A Pilatus header line as (key, value): "# Wavelength 1.0332 A" → ("Wavelength",
/// "1.0332 A"); "# Excluded_pixels: x.tif" and "# Tau = 1e-7 s" too. Lines that are a date or
/// free text keep their text as the value of an empty key.
fn pilatus_pair(line: &str) -> (String, String) {
    let l = line.trim_start_matches('#').trim();
    let key_end = l
        .find(|c: char| c.is_whitespace() || c == ':' || c == '=')
        .unwrap_or(l.len());
    let key = &l[..key_end];
    let rest = l[key_end..].trim_start();
    let rest = rest
        .strip_prefix(':')
        .or_else(|| rest.strip_prefix('='))
        .unwrap_or(rest)
        .trim();
    if key.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return (String::new(), l.to_owned());
    }
    (key.to_owned(), rest.to_owned())
}

/// A length in metres from "0.30000 m" / "172e-6 m" / "300 mm", as mm.
fn mm(v: &str, default_m: bool) -> Option<f64> {
    let n = crate::first_number(v)?;
    let unit = v
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .trim_end_matches('.');
    Some(match unit {
        "mm" => n,
        "um" | "µm" | "microns" => n / 1000.0,
        "m" => n * 1000.0,
        _ if default_m => n * 1000.0,
        _ => n,
    })
}

/// The geometry from Pilatus header pairs.
fn pilatus_geometry(pairs: &[(String, String)]) -> (Geometry, Option<String>, Option<String>) {
    let mut g = Geometry::default();
    let mut detector = None;
    let mut date = None;
    for (k, v) in pairs {
        let num = || crate::first_number(v);
        match k.to_ascii_lowercase().as_str() {
            "" if date.is_none() => date = text(v),
            "detector" => detector = text(v),
            "wavelength" => {
                g.wavelength = positive(num()).map(|w| {
                    // Unit "A"; very old headers in metres or nm are not seen in practice.
                    if v.contains("nm") { w * 10.0 } else { w }
                })
            }
            "detector_distance" => g.distance = positive(mm(v, true)),
            "beam_xy" => {
                let n: Vec<f64> = crate::numbers(v).collect();
                if n.len() >= 2 {
                    g.beam = Some((n[0], n[1]));
                }
            }
            "pixel_size" => {
                let n: Vec<f64> = v.split(" x ").filter_map(|p| mm(p.trim(), true)).collect();
                match n[..] {
                    [a, b] if a > 0.0 && b > 0.0 => g.pixel = Some((a, b)),
                    [a] if a > 0.0 => g.pixel = Some((a, a)),
                    _ => {}
                }
            }
            "detector_2theta" => g.two_theta = finite(num()),
            "start_angle" => g.osc_start = finite(num()),
            "angle_increment" => g.osc_width = finite(num()),
            "oscillation_axis" => g.osc_axis = text(v),
            "phi" => g.phi = finite(num()),
            "omega" => g.omega = finite(num()),
            "chi" => g.chi = finite(num()),
            "kappa" => g.kappa = finite(num()),
            "exposure_time" => g.exposure = positive(num()),
            "count_cutoff" => g.overload = positive(num()),
            _ => {}
        }
    }
    (g, detector, date)
}

/// A CIF data item on one line: `_name value` (quotes removed).
fn cif_item<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.lines().find_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix(name)?;
        rest.starts_with(char::is_whitespace)
            .then(|| rest.trim().trim_matches(['"', '\'']))
    })
}

pub(crate) fn parse(data: &[u8]) -> Result<Image> {
    let sec = find(data, SECTION).ok_or_else(|| bad("no binary section"))?;
    let text_part = String::from_utf8_lossy(&data[..sec]);
    let rel = find(&data[sec..], &MARKER).ok_or_else(|| bad("no binary data marker"))?;
    let bin = sec + rel + MARKER.len();
    let mime = String::from_utf8_lossy(&data[sec..sec + rel]);
    // MIME header fields, case-insensitive; `conversions` may be on a continuation line.
    let field = |name: &str| -> Option<String> {
        let lname = name.to_ascii_lowercase();
        mime.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().to_ascii_lowercase() == lname).then(|| v.trim().trim_matches('"').to_owned())
        })
    };
    let conversions = {
        let m = mime.to_ascii_lowercase();
        m.find("conversions=").map(|i| {
            m[i + 12..]
                .trim_start_matches('"')
                .split(['"', ';', '\r', '\n'])
                .next()
                .unwrap_or("")
                .trim()
                .to_owned()
        })
    };
    let int = |name: &str| field(name).and_then(|v| v.trim().parse::<u64>().ok());
    let w = int("X-Binary-Size-Fastest-Dimension");
    let h = int("X-Binary-Size-Second-Dimension");
    let count = int("X-Binary-Number-of-Elements");
    let size = int("X-Binary-Size").map(|s| s as usize);
    let (w, h) = match (w, h, count) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some(n)) if w > 0 => (w, n / w),
        (None, None, Some(n)) => (n, 1),
        _ => return Err(bad("binary section without dimensions")),
    };
    let w = u32::try_from(w).map_err(|_| bad("width"))?;
    let h = u32::try_from(h).map_err(|_| bad("height"))?;
    let etype = field("X-Binary-Element-Type")
        .unwrap_or_else(|| "signed 32-bit integer".into())
        .to_ascii_lowercase();
    let big = field("X-Binary-Element-Byte-Order")
        .is_some_and(|b| b.to_ascii_uppercase().starts_with("BIG"));
    let sample = element(&etype)
        .ok_or_else(|| Error::Unsupported(format!("CBF element type \"{etype}\"")))?;
    let conv = conversions.unwrap_or_else(|| "x-cbf_none".into());
    let coding = match conv.as_str() {
        "x-cbf_byte_offset" => {
            if matches!(sample, Sample::F32 | Sample::F64) {
                return Err(Error::Unsupported("byte-offset coded floats".into()));
            }
            Coding::ByteOffset {
                offset: bin,
                len: size.unwrap_or(data.len() - bin),
                sample,
                skip: 0,
            }
        }
        "x-cbf_none" => Coding::Raw {
            offset: bin,
            sample,
            big,
        },
        other => {
            return Err(Error::Unsupported(format!(
                "CBF compression \"{}\"",
                other.trim_start_matches("x-cbf_")
            )));
        }
    };

    let lines = header_contents(&text_part);
    let pairs: Vec<(String, String)> = lines.iter().map(|l| pilatus_pair(l)).collect();
    let (mut geometry, mut detector, date) = pilatus_geometry(&pairs);
    let mut header: Vec<(String, String)> = Vec::new();
    // Other CIF items on one line (`_array_data.header_convention`, `_Misc.*`,
    // `_diffrn_*`), then the detector header.
    for l in text_part.lines() {
        let l = l.trim();
        if l.starts_with('_')
            && !l.starts_with("_array_data.header_contents")
            && !l.starts_with("_array_data.data")
            && let Some((k, v)) = l.split_once(char::is_whitespace)
            && !v.trim().is_empty()
        {
            header.push((k.to_owned(), v.trim().trim_matches(['"', '\'']).to_owned()));
        }
    }
    header.extend(pairs.iter().filter(|(k, _)| !k.is_empty()).cloned());
    for (k, v) in [
        ("Element type", Some(etype.clone())),
        (
            "Compression",
            Some(conv.trim_start_matches("x-").to_owned()),
        ),
    ] {
        if let Some(v) = v {
            header.push((k.into(), v));
        }
    }
    // Full imgCIF files: the wavelength item.
    if geometry.wavelength.is_none() {
        geometry.wavelength = positive(
            cif_item(&text_part, "_diffrn_radiation_wavelength.wavelength")
                .and_then(crate::first_number),
        );
    }
    let convention = cif_item(&text_part, "_array_data.header_convention")
        .unwrap_or("")
        .to_ascii_uppercase();
    let pilatus_like = |s: &str| {
        let s = s.to_ascii_uppercase();
        s.contains("PILATUS") || s.contains("EIGER")
    };
    let mask_negative = convention.starts_with("PILATUS")
        || convention.starts_with("SLS")
        || detector.as_deref().is_some_and(pilatus_like)
        || geometry.overload.is_some();
    if detector.is_none() && convention.starts_with("PILATUS") {
        detector = Some("PILATUS".into());
    }
    Ok(Image {
        kind: Kind::Cbf,
        width: w,
        height: h,
        sample,
        frames: vec![Frame {
            coding,
            header: Vec::new(),
        }],
        header,
        geometry,
        detector,
        date,
        mask_negative,
        dummy: None,
        title: None,
    })
}

/// The sample type of an `X-Binary-Element-Type`.
fn element(t: &str) -> Option<Sample> {
    let signed = !t.contains("unsigned");
    let real = t.contains("real") || t.contains("ieee");
    let bits = if t.contains("64") {
        64
    } else if t.contains("32") {
        32
    } else if t.contains("16") {
        16
    } else if t.contains('8') {
        8
    } else {
        return None;
    };
    Some(match (real, signed, bits) {
        (true, _, 32) => Sample::F32,
        (true, _, 64) => Sample::F64,
        (true, _, _) => return None,
        (false, true, 8) => Sample::I8,
        (false, false, 8) => Sample::U8,
        (false, true, 16) => Sample::I16,
        (false, false, 16) => Sample::U16,
        (false, true, 32) => Sample::I32,
        (false, false, 32) => Sample::U32,
        (false, true, _) => Sample::I64,
        (false, false, _) => Sample::U64,
    })
}

/// Decodes `n` byte-offset coded values of element type `sample`. Deltas accumulate in 64
/// bits and each value is stored as the element type, wrapping as CBFlib's cast does
/// (some Mar CBFs declare 16-bit elements but count past 65535).
pub(crate) fn byte_offset(src: &[u8], n: usize, sample: Sample) -> Result<Vec<f32>> {
    match sample {
        Sample::U8 => byte_offset_as(src, n, |v| f32::from(v as u8)),
        Sample::I8 => byte_offset_as(src, n, |v| f32::from(v as i8)),
        Sample::U16 => byte_offset_as(src, n, |v| f32::from(v as u16)),
        Sample::I16 => byte_offset_as(src, n, |v| f32::from(v as i16)),
        Sample::U32 => byte_offset_as(src, n, |v| v as u32 as f32),
        Sample::I32 => byte_offset_as(src, n, |v| v as i32 as f32),
        Sample::U64 => byte_offset_as(src, n, |v| v as u64 as f32),
        _ => byte_offset_as(src, n, |v| v as f32),
    }
}

#[inline(always)]
fn byte_offset_as(src: &[u8], n: usize, store: impl Fn(i64) -> f32) -> Result<Vec<f32>> {
    // Every value takes at least one byte: more values than bytes is a truncated stream,
    // found before the output is allocated (the count is the header's word).
    if n > src.len() {
        return Err(Error::Truncated);
    }
    let mut out = Vec::with_capacity(n);
    let mut v: i64 = 0;
    let mut i = 0usize;
    let len = src.len();
    while out.len() < n {
        // Fast path: runs of one-byte deltas.
        if i < len {
            let d = src[i] as i8;
            if d != i8::MIN {
                v = v.wrapping_add(i64::from(d));
                i += 1;
                out.push(store(v));
                continue;
            }
        } else {
            return Err(Error::Truncated);
        }
        i += 1;
        let d16 = i16::from_le_bytes(
            src.get(i..i + 2)
                .ok_or(Error::Truncated)?
                .try_into()
                .map_err(|_| Error::Truncated)?,
        );
        i += 2;
        if d16 != i16::MIN {
            v = v.wrapping_add(i64::from(d16));
        } else {
            let d32 = i32::from_le_bytes(
                src.get(i..i + 4)
                    .ok_or(Error::Truncated)?
                    .try_into()
                    .map_err(|_| Error::Truncated)?,
            );
            i += 4;
            if d32 != i32::MIN {
                v = v.wrapping_add(i64::from(d32));
            } else {
                let d64 = i64::from_le_bytes(
                    src.get(i..i + 8)
                        .ok_or(Error::Truncated)?
                        .try_into()
                        .map_err(|_| Error::Truncated)?,
                );
                i += 8;
                v = v.wrapping_add(d64);
            }
        }
        out.push(store(v));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2^40 values claimed over 16 bytes (an EDF skip of many frames): refused before 4 TiB
    /// of output is asked for.
    #[test]
    fn value_count_bomb() {
        let r = byte_offset(&[1u8; 16], 1 << 40, Sample::I32);
        assert!(matches!(r, Err(Error::Truncated)));
        assert_eq!(byte_offset(&[1u8; 16], 16, Sample::I32).unwrap().len(), 16);
    }

    /// Byte-offset codes `values` (the writer side, for tests).
    pub(crate) fn encode(values: &[i64]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut prev = 0i64;
        for &v in values {
            let d = v - prev;
            prev = v;
            if (-127..=127).contains(&d) {
                out.push(d as i8 as u8);
                continue;
            }
            out.push(0x80);
            if (-32767..=32767).contains(&d) {
                out.extend((d as i16).to_le_bytes());
                continue;
            }
            out.extend(i16::MIN.to_le_bytes());
            if (-(i32::MAX as i64)..=i32::MAX as i64).contains(&d) {
                out.extend((d as i32).to_le_bytes());
                continue;
            }
            out.extend(i32::MIN.to_le_bytes());
            out.extend(d.to_le_bytes());
        }
        out
    }

    #[test]
    fn byte_offset_widths() {
        let vals = [0i64, 5, -3, 200, -40000, 40000, 3_000_000_000, -1, -2, 7];
        let enc = encode(&vals);
        let dec = byte_offset(&enc, vals.len(), Sample::I64).unwrap();
        for (a, b) in vals.iter().zip(&dec) {
            assert_eq!(*a as f32, *b);
        }
        assert!(matches!(
            byte_offset(&enc[..enc.len() - 3], vals.len(), Sample::I64),
            Err(Error::Truncated)
        ));
        // Values past the element type wrap.
        let enc = encode(&[65_578, -1]);
        assert_eq!(byte_offset(&enc, 2, Sample::U16).unwrap(), [42.0, 65_535.0]);
        assert_eq!(byte_offset(&enc, 2, Sample::I32).unwrap(), [65_578.0, -1.0]);
        assert!(matches!(
            byte_offset(&enc[..1], 2, Sample::I32),
            Err(Error::Truncated)
        ));
    }

    fn minicbf(w: usize, h: usize, vals: &[i64]) -> Vec<u8> {
        let bin = encode(vals);
        let mut t = String::from(
            "###CBF: VERSION 1.5\r\ndata_x\r\n\r\n_array_data.header_convention \"PILATUS_1.2\"\r\n_array_data.header_contents\r\n;\r\n",
        );
        t += "# Detector: PILATUS 300K, S/N 3-0101\r\n# 2024-01-02T03:04:05.678\r\n";
        t += "# Pixel_size 172e-6 m x 172e-6 m\r\n# Exposure_time 0.5 s\r\n";
        t += "# Count_cutoff 1048500 counts\r\n# Wavelength 0.9795 A\r\n";
        t += "# Detector_distance 0.25 m\r\n# Beam_xy (10.50, 20.25) pixels\r\n";
        t += "# Start_angle 12.0000 deg.\r\n# Angle_increment 0.1000 deg.\r\n";
        t += "# Image_path: /a/very/long/pa\\\r\nth/\r\n;\r\n\r\n_array_data.data\r\n;\r\n";
        t += "--CIF-BINARY-FORMAT-SECTION--\r\nContent-Type: application/octet-stream;\r\n     conversions=\"x-CBF_BYTE_OFFSET\"\r\n";
        t += &format!(
            "X-Binary-Size: {}\r\nX-Binary-Element-Type: \"signed 32-bit integer\"\r\nX-Binary-Number-of-Elements: {}\r\nX-Binary-Size-Fastest-Dimension: {w}\r\nX-Binary-Size-Second-Dimension: {h}\r\n\r\n",
            bin.len(),
            w * h
        );
        let mut d = t.into_bytes();
        d.extend(MARKER);
        d.extend(bin);
        d.extend(b"\r\n--CIF-BINARY-FORMAT-SECTION----\r\n;\r\n");
        d
    }

    #[test]
    fn pilatus_minicbf() {
        let vals: Vec<i64> = (0..12)
            .map(|i| if i == 5 { -1 } else { i * 1000 })
            .collect();
        let d = minicbf(4, 3, &vals);
        assert!(sniff(&d[..64]));
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.kind, Kind::Cbf);
        assert_eq!((im.width, im.height), (4, 3));
        assert_eq!(im.detector.as_deref(), Some("PILATUS 300K, S/N 3-0101"));
        assert_eq!(im.date.as_deref(), Some("2024-01-02T03:04:05.678"));
        let g = &im.geometry;
        assert_eq!(g.wavelength, Some(0.9795));
        assert_eq!(g.distance, Some(250.0));
        assert_eq!(g.beam, Some((10.5, 20.25)));
        let (px, py) = g.pixel.unwrap();
        assert!((px - 0.172).abs() < 1e-12 && (py - 0.172).abs() < 1e-12);
        assert_eq!(g.overload, Some(1048500.0));
        assert_eq!(g.osc_start, Some(12.0));
        assert_eq!(g.osc_width, Some(0.1));
        assert_eq!(g.exposure, Some(0.5));
        assert!(
            im.header
                .iter()
                .any(|(k, v)| k == "Image_path" && v == "/a/very/long/path/")
        );
        let v = im.frame(&d, 0).unwrap();
        assert_eq!(v[4], 4000.0);
        assert!(v[5].is_nan());
        assert_eq!(v[11], 11000.0);
    }

    #[test]
    fn headerless() {
        // A bare `_array_data.data` start (no `###CBF`), uncompressed 16-bit.
        let mut d = b"_array_data.data\n;\n--CIF-BINARY-FORMAT-SECTION--\nContent-Type: application/octet-stream;\n     conversions=\"x-CBF_NONE\"\nX-Binary-Element-Type: \"unsigned 16-bit integer\"\nX-Binary-Size-Fastest-Dimension: 2\nX-Binary-Size-Second-Dimension: 2\n\n".to_vec();
        d.extend(MARKER);
        for v in [1u16, 2, 65535, 4] {
            d.extend(v.to_le_bytes());
        }
        assert!(sniff(&d));
        let im = Image::parse(&d).unwrap();
        assert!(!im.mask_negative);
        assert_eq!(im.frame(&d, 0).unwrap(), [1.0, 2.0, 65535.0, 4.0]);
        assert!(!im.geometry.complete());
    }

    #[test]
    fn packed_unsupported() {
        let mut d = b"###CBF\n--CIF-BINARY-FORMAT-SECTION--\n conversions=\"x-CBF_PACKED\"\nX-Binary-Size-Fastest-Dimension: 2\nX-Binary-Size-Second-Dimension: 2\n\n".to_vec();
        d.extend(MARKER);
        assert!(matches!(Image::parse(&d), Err(Error::Unsupported(_))));
    }
}
