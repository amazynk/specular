//! IVF (`.ivf`, the raw VP8, VP9 and AV1 stream container of libvpx and libaom) into the
//! same [`Movie`] the MP4 demuxer gives: a 32-byte file header (`DKIF`, the codec, size,
//! time base), then frames of a 12-byte header (size, presentation time) and the frame.
//! IVF carries no sound.

use crate::mp4::{Codec, Error, Layout, MAX_SAMPLES, Movie, Result, Sample, VideoTrack};

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Whether `data` is an IVF file.
pub fn sniff(data: &[u8]) -> bool {
    data.len() >= 32 && &data[..4] == b"DKIF"
}

fn u16le(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32le(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Whether an AV1 temporal unit starts a coded video sequence (holds a sequence header OBU),
/// where decoding can begin.
fn av1_sync(tu: &[u8]) -> bool {
    let mut at = 0;
    while at < tu.len() {
        let h = tu[at];
        let kind = (h >> 3) & 15;
        if kind == 1 {
            return true;
        }
        let ext = usize::from(h & 4 != 0);
        if h & 2 == 0 {
            // no size field: the OBU runs to the end
            return false;
        }
        let mut p = at + 1 + ext;
        let mut size = 0u64;
        for i in 0..8 {
            let Some(&b) = tu.get(p) else {
                return false;
            };
            p += 1;
            size |= u64::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                break;
            }
        }
        at = p.saturating_add(usize::try_from(size).unwrap_or(usize::MAX));
    }
    false
}

/// Parses an IVF file. With `samples`, every frame is listed.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    if !sniff(data) {
        return Err(bad("not an IVF file"));
    }
    let header = usize::from(u16le(data, 6)).max(32);
    let fourcc: [u8; 4] = data[8..12].try_into().map_err(|_| bad("IVF header"))?;
    let codec = match &fourcc {
        b"VP80" => Codec::Vp8,
        b"VP90" => Codec::Vp9,
        b"AV01" => Codec::Av1,
        _ => Codec::Other(fourcc),
    };
    let width = u32::from(u16le(data, 12));
    let height = u32::from(u16le(data, 14));
    let mut rate = u32le(data, 16);
    let mut scale = u32le(data, 20);
    if rate == 0 || scale == 0 {
        (rate, scale) = (30, 1);
    }
    let mut list: Vec<Sample> = Vec::new();
    let mut at = header;
    while at + 12 <= data.len() {
        let size = u32le(data, at) as usize;
        let pts = u64::from_le_bytes(data[at + 4..at + 12].try_into().unwrap_or_default());
        let body = at + 12;
        if body + size > data.len() {
            // a frame cut off at the end
            break;
        }
        let frame = &data[body..body + size];
        let sync = match codec {
            Codec::Vp8 => specular_vp8::is_keyframe(frame),
            Codec::Vp9 => specular_vp9::is_keyframe(frame),
            Codec::Av1 => av1_sync(frame),
            _ => list.is_empty(),
        };
        let t = (pts as i128 * i128::from(scale)).clamp(0, i128::from(i64::MAX)) as i64;
        list.push(Sample {
            offset: body as u64,
            size: size as u32,
            dts: t as u64,
            cts: t,
            duration: 0,
            sync,
        });
        if list.len() > MAX_SAMPLES {
            return Err(Error::Unsupported(format!("{} frames", list.len())));
        }
        at = body + size;
    }
    if list.is_empty() {
        return Err(bad("an IVF file without frames"));
    }
    // each frame lasts until the next in presentation order; the last one the typical gap
    let mut order: Vec<i64> = list.iter().map(|s| s.cts).collect();
    order.sort_unstable();
    order.dedup();
    let typical = order
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&d| d > 0)
        .min()
        .unwrap_or(i64::from(scale));
    for s in &mut list {
        let k = order.partition_point(|&c| c <= s.cts);
        s.duration = order
            .get(k)
            .map_or(typical, |n| n - s.cts)
            .clamp(0, i64::from(u32::MAX)) as u32;
    }
    // decode order is file order; decode times must not fall behind presentation
    let mut dts: Vec<u64> = list.iter().map(|s| s.dts).collect();
    dts.sort_unstable();
    for (s, d) in list.iter_mut().zip(dts) {
        s.dts = d;
    }
    if !list.iter().any(|s| s.sync) {
        list[0].sync = true;
    }
    let mut v = VideoTrack {
        codec: Some(codec),
        width,
        height,
        display: (width, height),
        timescale: rate,
        layout: Layout::Contiguous,
        ..VideoTrack::default()
    };
    v.duration = list
        .iter()
        .map(|s| s.cts as u64 + u64::from(s.duration))
        .max()
        .unwrap_or(0);
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    let duration_us = v.duration_us();
    Ok(Movie {
        brand: *b"DKIF",
        timescale: rate,
        duration_us,
        created: None,
        video: Some(v),
        has_audio: false,
        audio: None,
        content_id: None,
        date: None,
        make: None,
        model: None,
        xmp: None,
        tags: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_times() {
        let mut d = b"DKIF".to_vec();
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&32u16.to_le_bytes());
        d.extend_from_slice(b"VP80");
        d.extend_from_slice(&64u16.to_le_bytes());
        d.extend_from_slice(&48u16.to_le_bytes());
        d.extend_from_slice(&30u32.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&2u32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        for (pts, tag) in [(0u64, 0u8), (1, 1)] {
            // a VP8 frame tag (bit 0 clear on keyframes), start code, size
            d.extend_from_slice(&10u32.to_le_bytes());
            d.extend_from_slice(&pts.to_le_bytes());
            d.extend_from_slice(&[tag, 0, 0, 0x9D, 0x01, 0x2A, 64, 0, 48, 0]);
        }
        let m = parse(&d, true).unwrap();
        let v = m.video.unwrap();
        assert_eq!(v.codec, Some(Codec::Vp8));
        assert_eq!(v.samples.len(), 2);
        assert!(v.samples[0].sync && !v.samples[1].sync);
        assert_eq!((v.samples[1].cts, v.timescale), (1, 30));
        assert_eq!(m.duration_us, Some(66_666));
    }

    #[test]
    fn av1_sequence_headers_mark_sync() {
        // temporal delimiter, then a sequence header OBU with a size field
        assert!(av1_sync(&[0x12, 0x00, 0x0A, 0x01, 0x00]));
        assert!(!av1_sync(&[0x12, 0x00, 0x32, 0x01, 0x00]));
    }
}
