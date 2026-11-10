//! An OpenEXR writer for linear-light exports: one scanline part, RGB or RGBA in half or
//! float samples, ZIP compression (blocks of 16 lines, byte-split and delta predicted, then
//! zlib, compressed in parallel), and the primaries and white as the `chromaticities`
//! attribute. With [`encode_with`], the standard string and float attributes a photo's Exif
//! fills (`owner`, `comments`, `capDate`, `utcOffset`, `latitude`, `longitude`, `altitude`)
//! and its XMP packet as an `xmp` string attribute. Written from the OpenEXR file layout
//! document; read back by `specular-exr`.

use rayon::prelude::*;

/// Sample type of every channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    Half,
    Float,
}

/// What [`encode`] writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExrOptions {
    pub sample: Sample,
    /// ZIP compression (else none).
    pub zip: bool,
    /// CIE xy of red, green, blue and white (the `chromaticities` attribute).
    pub chromaticities: Option<[f32; 8]>,
}

/// The descriptive attributes of a file (see [`ExrMeta::from_exif`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExrMeta {
    /// `owner`: who owns the image (the copyright, else the artist).
    pub owner: Option<String>,
    /// `comments`: a description of the image.
    pub comments: Option<String>,
    /// `capDate`: when it was taken, local time, `YYYY:MM:DD hh:mm:ss`.
    pub cap_date: Option<String>,
    /// `utcOffset`: seconds to add to `capDate` for UTC.
    pub utc_offset: Option<f32>,
    /// Degrees north, degrees east, and metres above sea level.
    pub latitude: Option<f32>,
    pub longitude: Option<f32>,
    pub altitude: Option<f32>,
    /// The XMP packet (an `xmp` string attribute).
    pub xmp: Option<Vec<u8>>,
}

impl ExrMeta {
    /// The attributes `exif` and `xmp` give (both already cut to what the export keeps:
    /// GPS is in `exif` only when location is kept).
    pub fn from_exif(exif: Option<&crate::exif::Exif>, xmp: Option<&[u8]>) -> ExrMeta {
        let mut m = ExrMeta {
            xmp: xmp.filter(|x| !x.is_empty()).map(<[u8]>::to_vec),
            ..ExrMeta::default()
        };
        let Some(e) = exif else { return m };
        let find =
            |dir: &[crate::exif::Entry], tag: u16| dir.iter().find(|x| x.tag == tag).cloned();
        let text = |dir: &[crate::exif::Entry], tag: u16| {
            find(dir, tag)
                .filter(|x| x.ty == 2)
                .map(|x| {
                    let end = x.data.iter().position(|&b| b == 0).unwrap_or(x.data.len());
                    String::from_utf8_lossy(&x.data[..end]).trim().to_owned()
                })
                .filter(|s| !s.is_empty())
        };
        let rationals = |dir: &[crate::exif::Entry], tag: u16| -> Option<Vec<f64>> {
            let x = find(dir, tag).filter(|x| x.ty == 5 || x.ty == 10)?;
            let v: Vec<f64> = x
                .data
                .chunks_exact(8)
                .map(|c| {
                    let (n, d) = if x.ty == 10 {
                        (
                            f64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])),
                            f64::from(i32::from_le_bytes([c[4], c[5], c[6], c[7]])),
                        )
                    } else {
                        (
                            f64::from(u32::from_le_bytes([c[0], c[1], c[2], c[3]])),
                            f64::from(u32::from_le_bytes([c[4], c[5], c[6], c[7]])),
                        )
                    };
                    if d == 0.0 { f64::NAN } else { n / d }
                })
                .collect();
            (!v.is_empty() && v.iter().all(|x| x.is_finite())).then_some(v)
        };
        m.owner = text(&e.ifd0, 0x8298).or_else(|| text(&e.ifd0, 0x013B));
        m.comments = text(&e.ifd0, 0x010E);
        m.cap_date = text(&e.exif, 0x9003).or_else(|| text(&e.ifd0, 0x0132));
        // OffsetTimeOriginal "+02:00": UTC is local time minus two hours.
        m.utc_offset = text(&e.exif, 0x9011).and_then(|o| {
            let (sign, rest) = match o.as_bytes().first()? {
                b'+' => (1.0, &o[1..]),
                b'-' => (-1.0, &o[1..]),
                _ => return None,
            };
            let (h, mi) = rest.split_once(':')?;
            let secs = h.parse::<f32>().ok()? * 3600.0 + mi.parse::<f32>().ok()? * 60.0;
            Some(-sign * secs)
        });
        let degrees = |v: Vec<f64>| {
            v.iter()
                .zip([1.0, 60.0, 3600.0])
                .map(|(x, k)| x / k)
                .sum::<f64>()
        };
        let reference = |tag: u16| text(&e.gps, tag).unwrap_or_default();
        if let Some(v) = rationals(&e.gps, 0x0002) {
            let s = if reference(0x0001).eq_ignore_ascii_case("S") {
                -1.0
            } else {
                1.0
            };
            m.latitude = Some((s * degrees(v)) as f32);
        }
        if let Some(v) = rationals(&e.gps, 0x0004) {
            let s = if reference(0x0003).eq_ignore_ascii_case("W") {
                -1.0
            } else {
                1.0
            };
            m.longitude = Some((s * degrees(v)) as f32);
        }
        if let Some(v) = rationals(&e.gps, 0x0006) {
            let below = find(&e.gps, 0x0005).and_then(|x| x.data.first().copied()) == Some(1);
            m.altitude = Some((if below { -v[0] } else { v[0] }) as f32);
        }
        m
    }
}

/// Lines per ZIP block.
const ZIP_LINES: usize = 16;

/// An EXR file of `width × height` pixels with `channels` (3 or 4) interleaved linear values
/// per pixel in `data` (RGB or RGBA), rows top to bottom.
pub fn encode(
    width: u32,
    height: u32,
    channels: usize,
    data: &[f32],
    o: &ExrOptions,
) -> Result<Vec<u8>, String> {
    encode_with(width, height, channels, data, o, &ExrMeta::default())
}

/// [`encode`] with the descriptive attributes of `meta`.
pub fn encode_with(
    width: u32,
    height: u32,
    channels: usize,
    data: &[f32],
    o: &ExrOptions,
    meta: &ExrMeta,
) -> Result<Vec<u8>, String> {
    let (w, h) = (width as usize, height as usize);
    if !(3..=4).contains(&channels) {
        return Err(format!("{channels} channels for EXR"));
    }
    if w == 0 || h == 0 || data.len() != w * h * channels {
        return Err("the samples do not match the size".into());
    }
    if width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err("too large for EXR".into());
    }
    // Channels are stored in alphabetical order: A, B, G, R.
    let order: &[usize] = if channels == 4 {
        &[3, 2, 1, 0]
    } else {
        &[2, 1, 0]
    };
    let names: &[&[u8]] = if channels == 4 {
        &[b"A", b"B", b"G", b"R"]
    } else {
        &[b"B", b"G", b"R"]
    };
    let (ptype, ssize) = match o.sample {
        Sample::Half => (1i32, 2usize),
        Sample::Float => (2i32, 4usize),
    };

    let mut out = Vec::with_capacity(data.len() * ssize / 2 + 1024);
    out.extend_from_slice(&[0x76, 0x2f, 0x31, 0x01]);
    out.extend_from_slice(&2u32.to_le_bytes());
    // Header.
    let mut chlist = Vec::new();
    for n in names {
        chlist.extend_from_slice(n);
        chlist.push(0);
        chlist.extend_from_slice(&ptype.to_le_bytes());
        chlist.extend_from_slice(&[0, 0, 0, 0]); // pLinear, reserved
        chlist.extend_from_slice(&1i32.to_le_bytes());
        chlist.extend_from_slice(&1i32.to_le_bytes());
    }
    chlist.push(0);
    attr(&mut out, "channels", "chlist", &chlist);
    if let Some(c) = o.chromaticities {
        let v: Vec<u8> = c.iter().flat_map(|f| f.to_le_bytes()).collect();
        attr(&mut out, "chromaticities", "chromaticities", &v);
    }
    attr(
        &mut out,
        "compression",
        "compression",
        &[if o.zip { 3 } else { 0 }],
    );
    let bx: Vec<u8> = [0i32, 0, width as i32 - 1, height as i32 - 1]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    attr(&mut out, "dataWindow", "box2i", &bx);
    attr(&mut out, "displayWindow", "box2i", &bx);
    attr(&mut out, "lineOrder", "lineOrder", &[0]);
    for (name, v) in [
        ("owner", &meta.owner),
        ("comments", &meta.comments),
        ("capDate", &meta.cap_date),
    ] {
        if let Some(v) = v {
            attr(&mut out, name, "string", v.as_bytes());
        }
    }
    for (name, v) in [
        ("utcOffset", meta.utc_offset),
        ("latitude", meta.latitude),
        ("longitude", meta.longitude),
        ("altitude", meta.altitude),
    ] {
        if let Some(v) = v.filter(|v| v.is_finite()) {
            attr(&mut out, name, "float", &v.to_le_bytes());
        }
    }
    if let Some(x) = &meta.xmp {
        attr(&mut out, "xmp", "string", x);
    }
    attr(&mut out, "pixelAspectRatio", "float", &1f32.to_le_bytes());
    attr(&mut out, "screenWindowCenter", "v2f", &[0; 8]);
    attr(&mut out, "screenWindowWidth", "float", &1f32.to_le_bytes());
    out.push(0);

    let lines = if o.zip { ZIP_LINES } else { 1 };
    let blocks = h.div_ceil(lines);
    let table_at = out.len();
    out.resize(table_at + blocks * 8, 0);
    let row_bytes = w * channels * ssize;
    // Each block: per line, per channel (alphabetical), the samples left to right.
    let chunks: Vec<Vec<u8>> = (0..blocks)
        .into_par_iter()
        .map(|b| {
            let y0 = b * lines;
            let y1 = (y0 + lines).min(h);
            let mut raw = Vec::with_capacity((y1 - y0) * row_bytes);
            for y in y0..y1 {
                let row = &data[y * w * channels..][..w * channels];
                for &c in order {
                    for x in 0..w {
                        let v = row[x * channels + c];
                        match o.sample {
                            Sample::Half => {
                                raw.extend_from_slice(&half::f16::from_f32(v).to_le_bytes())
                            }
                            Sample::Float => raw.extend_from_slice(&v.to_le_bytes()),
                        }
                    }
                }
            }
            if o.zip { zip(&raw) } else { raw }
        })
        .collect();
    for (b, c) in chunks.iter().enumerate() {
        let at = out.len() as u64;
        out[table_at + b * 8..][..8].copy_from_slice(&at.to_le_bytes());
        out.extend_from_slice(&((b * lines) as i32).to_le_bytes());
        out.extend_from_slice(&(c.len() as u32).to_le_bytes());
        out.extend_from_slice(c);
    }
    Ok(out)
}

fn attr(out: &mut Vec<u8>, name: &str, ty: &str, value: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out.extend_from_slice(ty.as_bytes());
    out.push(0);
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value);
}

/// ZIP: bytes split into even and odd halves, each byte the difference to the one before
/// (+128), then zlib. A block that does not shrink is stored as it is.
fn zip(raw: &[u8]) -> Vec<u8> {
    let n = raw.len();
    let mut t: Vec<u8> = Vec::with_capacity(n);
    t.extend(raw.iter().step_by(2));
    t.extend(raw.iter().skip(1).step_by(2));
    for i in (1..n).rev() {
        t[i] = t[i].wrapping_sub(t[i - 1]).wrapping_add(128);
    }
    let z = zlib(&t);
    if z.len() < n { z } else { raw.to_vec() }
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; zlib_rs::compress_bound(data.len())];
    let (c, rc) = zlib_rs::compress_slice(&mut out, data, zlib_rs::DeflateConfig::new(4));
    assert!(
        rc == zlib_rs::ReturnCode::Ok,
        "zlib compress_bound too small"
    );
    let n = c.len();
    out.truncate(n);
    out
}
