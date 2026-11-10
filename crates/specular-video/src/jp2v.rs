//! JPEG 2000 video (Motion JPEG 2000 `mjp2` in MP4/MOV, JPEG 2000 in MXF: digital cinema
//! packages, broadcast and archive masters) through the in-tree decoder `specular-jp2`.
//!
//! Every sample is one codestream (bare, in a `jp2c` box, or two of them for the two fields
//! of an interlaced frame, which are woven). Components come out at their own sampling and
//! precision, after the codestream's colour transform:
//!
//! - subsampled three-component pictures are YCbCr (4:2:2 or 4:2:0);
//! - full-resolution three-component pictures are RGB, as FFmpeg reads them, and are turned
//!   into full-range BT.709 YCbCr 4:4:4 two bits deeper for the player;
//! - digital cinema codestreams (Rsiz 2K/4K cinema profiles) are X'Y'Z' (DCI, gamma 2.6) and
//!   are mapped to BT.709 RGB first;
//! - one component is grey.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};

/// The codestreams of a sample: one, or two fields.
fn codestreams(d: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut p = 0usize;
    // Boxes (`jp2c`, and whatever else MJ2 puts beside them, such as `fiel`).
    while p + 8 <= d.len() && out.len() < 2 {
        let size = u32::from_be_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]) as usize;
        let kind = &d[p + 4..p + 8];
        if !kind.iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
            break;
        }
        let (body, end) = match size {
            0 => (p + 8, d.len()),
            1 => {
                let Some(b) = d.get(p + 8..p + 16) else { break };
                let big = u64::from_be_bytes(b.try_into().unwrap_or_default()) as usize;
                (p + 16, p.saturating_add(big))
            }
            n => (p + 8, p.saturating_add(n)),
        };
        if end > d.len() || end < body {
            break;
        }
        if kind == b"jp2c" {
            out.push(&d[body..end]);
        }
        p = end;
    }
    if !out.is_empty() {
        return out;
    }
    // Bare codestreams: SOC SIZ.
    let starts: Vec<usize> = d
        .windows(4)
        .enumerate()
        .filter(|(_, w)| *w == [0xFF, 0x4F, 0xFF, 0x51])
        .map(|(i, _)| i)
        .take(2)
        .collect();
    match starts.as_slice() {
        [a, b] => vec![&d[*a..*b], &d[*b..]],
        [a] => vec![&d[*a..]],
        _ => Vec::new(),
    }
}

/// Rsiz of a codestream (SIZ follows SOC).
fn rsiz(cs: &[u8]) -> u16 {
    match cs.get(6..8) {
        Some(b) => u16::from_be_bytes([b[0], b[1]]),
        None => 0,
    }
}

/// A digital cinema profile (2K, 4K, scalable or long-term storage, ISO 15444-1 Table A.10).
fn cinema(rsiz: u16) -> bool {
    matches!(rsiz & 0xFF0F, 0x0003 | 0x0004) || matches!(rsiz, 0x0005..=0x0006)
}

/// The decoded components of one codestream: their sizes, samples and precision.
struct Comps {
    comps: Vec<specular_jp2::RawComponent>,
    precision: u8,
    xyz: bool,
}

fn decode_comps(cs: &[u8]) -> Result<Comps, DecodeError> {
    let dec = specular_jp2::Decoder::new(cs).map_err(jp2_err)?;
    let info = dec.info();
    let precision = info
        .components
        .iter()
        .map(|c| c.precision)
        .max()
        .unwrap_or(8);
    if precision > 16 {
        return Err(DecodeError::Unsupported(format!(
            "{precision}-bit JPEG 2000 video"
        )));
    }
    // The components and their 16-bit working copies.
    let px: u64 = info
        .components
        .iter()
        .map(|c| {
            u64::from(info.width.div_ceil(u32::from(c.dx.max(1))))
                * u64::from(info.height.div_ceil(u32::from(c.dy.max(1))))
        })
        .sum();
    let _res = specular_core::budget::reserve(px * 6).map_err(DecodeError::NeedsMemory)?;
    let comps = dec.decode_components(0, None).map_err(jp2_err)?;
    Ok(Comps {
        comps,
        precision,
        xyz: cinema(rsiz(cs)),
    })
}

fn jp2_err(e: specular_jp2::Jp2Error) -> DecodeError {
    match e {
        specular_jp2::Jp2Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        specular_jp2::Jp2Error::Unsupported(m) => {
            DecodeError::Unsupported(format!("JPEG 2000 video: {m}"))
        }
        e => DecodeError::Malformed(e.to_string()),
    }
}

/// Little-endian samples (or bytes at 8 bits) of a plane at `bits`.
fn pack(samples: impl Iterator<Item = u16>, bits: u8, n: usize) -> Vec<u8> {
    if bits <= 8 {
        samples.map(|s| s as u8).collect()
    } else {
        let mut v = Vec::with_capacity(2 * n);
        for s in samples {
            v.extend_from_slice(&s.to_le_bytes());
        }
        v
    }
}

/// DCI X'Y'Z' (gamma 2.6, 52.37/48 normalisation) to BT.709 RGB with the BT.709 transfer, in
/// [0, 1].
fn xyz_to_rgb(x: f32, y: f32, z: f32) -> [f32; 3] {
    let lin = |v: f32| v.max(0.0).powf(2.6) * (52.37 / 48.0);
    let (x, y, z) = (lin(x), lin(y), lin(z));
    let r = 3.240_97 * x - 1.537_383 * y - 0.498_611 * z;
    let g = -0.969_244 * x + 1.875_968 * y + 0.041_555 * z;
    let b = 0.055_63 * x - 0.203_977 * y + 1.056_972 * z;
    [r, g, b].map(|v| {
        let v = v.clamp(0.0, 1.0);
        if v < 0.018 {
            4.5 * v
        } else {
            1.099 * v.powf(0.45) - 0.099
        }
    })
}

/// One picture from a codestream's components.
fn frame(c: Comps) -> Result<VideoFrame, DecodeError> {
    let Comps {
        comps,
        precision,
        xyz,
    } = c;
    let Some(y) = comps.first() else {
        return Err(DecodeError::Malformed(
            "JPEG 2000 frame without components".into(),
        ));
    };
    let (w, h) = (y.width, y.height);
    let n = (w * h) as usize;
    let bits = precision.max(8);
    if comps.len() < 3 {
        // Grey (an alpha component is left out).
        let cn = (w.div_ceil(2) * h.div_ceil(2)) as usize;
        let mid = 1u16 << (bits - 1);
        return Ok(VideoFrame {
            width: w,
            height: h,
            bit_depth: bits,
            chroma: Chroma::Yuv420,
            planes: [
                pack(y.samples.iter().copied(), bits, n),
                pack(std::iter::repeat_n(mid, cn), bits, cn),
                pack(std::iter::repeat_n(mid, cn), bits, cn),
            ],
            signal: Some(Signal {
                full_range: true,
                primaries: None,
                transfer: None,
                matrix: Some(1),
            }),
        });
    }
    let (cb, cr) = (&comps[1], &comps[2]);
    let chroma = match (cb.width < w, cb.height < h) {
        (true, true) => Some(Chroma::Yuv420),
        (true, false) => Some(Chroma::Yuv422),
        (false, false) => None,
        (false, true) => {
            return Err(DecodeError::Unsupported(
                "JPEG 2000 video with 4:4:0 sampling".into(),
            ));
        }
    };
    if let Some(chroma) = chroma {
        let cn = (cb.width * cb.height) as usize;
        if cr.width != cb.width || cr.height != cb.height {
            return Err(DecodeError::Unsupported(
                "JPEG 2000 video with unequal chroma sampling".into(),
            ));
        }
        return Ok(VideoFrame {
            width: w,
            height: h,
            bit_depth: bits,
            chroma,
            planes: [
                pack(y.samples.iter().copied(), bits, n),
                pack(cb.samples.iter().copied(), bits, cn),
                pack(cr.samples.iter().copied(), bits, cn),
            ],
            // Broadcast JPEG 2000 is studio-range YCbCr; the size picks the matrix.
            signal: Some(Signal {
                full_range: false,
                primaries: None,
                transfer: None,
                matrix: None,
            }),
        });
    }
    let (r, g, b) = (&comps[0].samples, &comps[1].samples, &comps[2].samples);
    if r.len() < n || g.len() < n || b.len() < n {
        return Err(DecodeError::Malformed(
            "JPEG 2000 components cut short".into(),
        ));
    }
    Ok(rgb_frame(w, h, bits, [r, g, b], xyz))
}

/// An RGB picture (or DCI X'Y'Z' with `xyz`) of `bits`-deep samples as full-range BT.709
/// YCbCr 4:4:4, two bits deeper (at most 16), for the player, which shows YCbCr only.
pub(crate) fn rgb_frame(w: u32, h: u32, bits: u8, rgb: [&[u16]; 3], xyz: bool) -> VideoFrame {
    let n = (w * h) as usize;
    let out_bits = (bits + 2).min(16);
    let max_in = ((1u32 << bits) - 1) as f32;
    let max_out = ((1u32 << out_bits) - 1) as f32;
    let half = (1u32 << (out_bits - 1)) as f32;
    let [r, g, b] = rgb;
    let mut planes = [
        Vec::with_capacity(2 * n),
        Vec::with_capacity(2 * n),
        Vec::with_capacity(2 * n),
    ];
    for i in 0..n.min(r.len()).min(g.len()).min(b.len()) {
        let s = [r[i], g[i], b[i]].map(|v| f32::from(v) / max_in);
        let [rr, gg, bb] = if xyz { xyz_to_rgb(s[0], s[1], s[2]) } else { s };
        let yy = 0.2126 * rr + 0.7152 * gg + 0.0722 * bb;
        let u = (bb - yy) / 1.8556;
        let v = (rr - yy) / 1.5748;
        let q = |x: f32| (x.round().clamp(0.0, max_out)) as u16;
        for (p, val) in planes.iter_mut().zip([
            q(yy * max_out),
            q(half + u * max_out),
            q(half + v * max_out),
        ]) {
            p.extend_from_slice(&val.to_le_bytes());
        }
    }
    VideoFrame {
        width: w,
        height: h,
        bit_depth: out_bits,
        chroma: Chroma::Yuv444,
        planes,
        signal: Some(Signal {
            full_range: true,
            primaries: Some(1),
            transfer: Some(1),
            matrix: Some(1),
        }),
    }
}

/// Bytes per sample of a frame.
fn bps(f: &VideoFrame) -> usize {
    if f.bit_depth > 8 { 2 } else { 1 }
}

/// Weaves two field pictures: `a` on the top lines.
fn weave(a: VideoFrame, b: VideoFrame) -> Option<VideoFrame> {
    if a.width != b.width
        || a.height != b.height
        || a.chroma != b.chroma
        || a.bit_depth != b.bit_depth
    {
        return None;
    }
    let bps = bps(&a);
    let cw = match a.chroma {
        Chroma::Yuv444 => a.width,
        _ => a.width.div_ceil(2),
    } as usize;
    let mut planes: [Vec<u8>; 3] = Default::default();
    for (i, out) in planes.iter_mut().enumerate() {
        let row = bps * if i == 0 { a.width as usize } else { cw };
        let (t, u) = (&a.planes[i], &b.planes[i]);
        for (x, y) in t.chunks(row).zip(u.chunks(row)) {
            out.extend_from_slice(x);
            out.extend_from_slice(y);
        }
    }
    Some(VideoFrame {
        height: 2 * a.height,
        planes,
        ..a
    })
}

/// Decodes one sample. A damaged sample costs its picture (`Ok(None)`).
pub fn decode(data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
    let streams = codestreams(data);
    let mut pics = Vec::with_capacity(2);
    for cs in &streams {
        match decode_comps(cs).and_then(frame) {
            Ok(f) => pics.push(f),
            Err(e @ (DecodeError::Unsupported(_) | DecodeError::NeedsMemory(_))) => return Err(e),
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: JPEG 2000 frame lost: {e}");
                return Ok(None);
            }
        }
    }
    Ok(match (pics.pop(), pics.pop()) {
        (Some(second), Some(first)) => weave(first, second),
        (Some(only), None) => Some(only),
        _ => None,
    })
}

/// Whether a sample holds two field codestreams.
pub fn two_fields(data: &[u8]) -> bool {
    codestreams(data).len() == 2
}

/// "Digital cinema 2K", "Broadcast", ... from the first sample's codestream.
pub fn profile(first: &[u8]) -> Option<String> {
    let cs = *codestreams(first).first()?;
    let r = rsiz(cs);
    Some(
        match r {
            0 => "Part 1",
            1 | 2 => "Part 1 restricted",
            3 => "Digital cinema 2K",
            4 => "Digital cinema 4K",
            5 | 6 => "Digital cinema scalable",
            7 => "Long-term storage",
            0x0100..=0x01FF => "Broadcast",
            0x0400..=0x04FF => "IMF",
            r if r & 0x4000 != 0 => "High-throughput",
            r if r & 0x0300 != 0 => "Digital cinema",
            _ => return None,
        }
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_split_into_codestreams() {
        let cs = [0xFF, 0x4F, 0xFF, 0x51, 0, 41, 0, 3];
        assert_eq!(codestreams(&cs).len(), 1);
        let two = [&cs[..], &cs[..]].concat();
        assert_eq!(codestreams(&two).len(), 2);
        let mut boxed = (8 + cs.len() as u32).to_be_bytes().to_vec();
        boxed.extend_from_slice(b"jp2c");
        boxed.extend_from_slice(&cs);
        assert_eq!(codestreams(&boxed), vec![&cs[..]]);
        assert_eq!(rsiz(&cs), 3);
        assert!(cinema(3) && cinema(4) && !cinema(0) && !cinema(0x0102));
        assert!(matches!(decode(b"junk"), Ok(None)));
    }

    #[test]
    fn dci_white_is_white() {
        // D65 white at 48 cd/m² (Y = 1 after the normalisation) maps to full RGB.
        let enc = |v: f32| (v * 48.0 / 52.37).powf(1.0 / 2.6);
        let [r, g, b] = xyz_to_rgb(enc(0.9505), enc(1.0), enc(1.089));
        for c in [r, g, b] {
            assert!((c - 1.0).abs() < 0.01, "{r} {g} {b}");
        }
    }
}
