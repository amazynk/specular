//! YUV4MPEG2 (`.y4m`): uncompressed video as a text stream header ("YUV4MPEG2 W… H… F…:…
//! I… A…:… C…") and frames each led by a "FRAME" line. Every frame is a sync sample of the
//! planar layout its colour space names ([`crate::rawvid::planar_fourcc`]), played through
//! the raw decoder. Interlaced and mixed-field streams are marked interlaced; 4:1:1 and
//! alpha (`444alpha`) are refused in words.

use specular_hwdec::video::Chroma;

use crate::mp4::{Codec, Colour, Error, Layout, MAX_SAMPLES, Movie, Result, Sample, VideoTrack};
use crate::rawvid::PixDesc;

const MAGIC: &[u8] = b"YUV4MPEG2 ";

pub fn sniff(d: &[u8]) -> bool {
    d.starts_with(MAGIC)
}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

/// The stream header's facts.
struct Header {
    width: u32,
    height: u32,
    rate: (u32, u32),
    aspect: Option<(u32, u32)>,
    interlace: u8,
    chroma: Chroma,
    bits: u8,
    mono: bool,
    full_range: bool,
}

fn ratio(v: &str) -> Option<(u32, u32)> {
    let (a, b) = v.split_once(':')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn header(line: &str) -> Result<Header> {
    let mut h = Header {
        width: 0,
        height: 0,
        rate: (25, 1),
        aspect: None,
        interlace: b'p',
        chroma: Chroma::Yuv420,
        bits: 8,
        mono: false,
        full_range: false,
    };
    for tok in line.split_ascii_whitespace().skip(1) {
        let (k, v) = tok.split_at(1);
        match k {
            "W" => h.width = v.parse().map_err(|_| bad("Y4M width"))?,
            "H" => h.height = v.parse().map_err(|_| bad("Y4M height"))?,
            "F" => {
                if let Some(r) = ratio(v).filter(|r| r.0 > 0 && r.1 > 0) {
                    h.rate = r;
                }
            }
            "A" => h.aspect = ratio(v).filter(|r| r.0 > 0 && r.1 > 0),
            "I" => h.interlace = v.bytes().next().unwrap_or(b'p'),
            "C" => {
                let (base, depth) = match v.split_once('p') {
                    Some((b, d)) if !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()) => {
                        (b, d.parse::<u8>().unwrap_or(8))
                    }
                    _ => (v, 8),
                };
                h.bits = depth;
                h.chroma = match base {
                    "420" | "420jpeg" | "420paldv" | "420mpeg2" => Chroma::Yuv420,
                    "422" => Chroma::Yuv422,
                    "444" => Chroma::Yuv444,
                    "mono" | "mono16" => {
                        h.mono = true;
                        if base == "mono16" {
                            h.bits = 16;
                        }
                        Chroma::Yuv420
                    }
                    "411" => return Err(Error::Unsupported("4:1:1 YUV4MPEG2".into())),
                    "444alpha" => {
                        return Err(Error::Unsupported("YUV4MPEG2 with alpha (444alpha)".into()));
                    }
                    other => {
                        return Err(Error::Unsupported(format!(
                            "YUV4MPEG2 colour space {other}"
                        )));
                    }
                };
            }
            "X" if v == "COLORRANGE=FULL" => h.full_range = true,
            _ => {}
        }
    }
    if h.width == 0 || h.height == 0 || h.width > 16_384 || h.height > 16_384 {
        return Err(bad("Y4M picture size"));
    }
    if h.mono && h.bits != 8 {
        return Err(Error::Unsupported(format!(
            "{}-bit greyscale YUV4MPEG2",
            h.bits
        )));
    }
    Ok(h)
}

/// Bytes of one frame's picture.
fn frame_bytes(h: &Header) -> usize {
    let (w, ht) = (h.width as usize, h.height as usize);
    let bytes = if h.bits > 8 { 2 } else { 1 };
    let c = match h.chroma {
        _ if h.mono => 0,
        Chroma::Yuv420 => 2 * w.div_ceil(2) * ht.div_ceil(2),
        Chroma::Yuv422 => 2 * w.div_ceil(2) * ht,
        _ => 2 * w * ht,
    };
    (w * ht + c) * bytes
}

pub fn parse(d: &[u8], samples: bool) -> Result<Movie> {
    let end = d
        .iter()
        .position(|&b| b == b'\n')
        .ok_or_else(|| bad("Y4M header line"))?;
    let line = std::str::from_utf8(&d[..end]).map_err(|_| bad("Y4M header text"))?;
    let h = header(line)?;
    let fourcc = if h.mono {
        *b"GREY"
    } else {
        crate::rawvid::planar_fourcc(h.chroma, h.bits)
            .ok_or_else(|| Error::Unsupported(format!("{}-bit YUV4MPEG2", h.bits)))?
    };
    let size = frame_bytes(&h);
    let (num, den) = h.rate;
    let mut list = Vec::new();
    let mut at = end + 1;
    while at + 5 <= d.len() && list.len() <= MAX_SAMPLES {
        if &d[at..at + 5] != b"FRAME" {
            break;
        }
        let Some(nl) = d[at..].iter().take(4096).position(|&b| b == b'\n') else {
            break;
        };
        let body = at + nl + 1;
        if body + size > d.len() {
            break;
        }
        let t = list.len() as u64 * u64::from(den);
        list.push(Sample {
            offset: body as u64,
            size: size as u32,
            dts: t,
            cts: t as i64,
            duration: den,
            sync: true,
        });
        at = body + size;
    }
    if list.is_empty() {
        return Err(bad("a YUV4MPEG2 stream without frames"));
    }
    let desc = PixDesc {
        fourcc,
        depth: u16::from(h.bits),
        bottom_up: false,
        palette: Vec::new(),
    };
    let mut v = VideoTrack {
        codec: Some(Codec::Raw),
        config: desc.to_config(),
        width: h.width,
        height: h.height,
        display: (h.width, h.height),
        timescale: num,
        layout: Layout::Contiguous,
        pixel_aspect: h.aspect,
        interlaced: matches!(h.interlace, b't' | b'b' | b'm'),
        bottom_first: h.interlace == b'b',
        colour: h.full_range.then_some(Colour {
            primaries: 2,
            transfer: 2,
            matrix: 2,
            full_range: true,
        }),
        ..VideoTrack::default()
    };
    v.duration = list.len() as u64 * u64::from(den);
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    let duration_us = v.duration_us();
    Ok(Movie {
        brand: *b"YUV4",
        timescale: num,
        duration_us,
        video: Some(v),
        ..Movie::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_tiny_stream() {
        let mut d = b"YUV4MPEG2 W4 H2 F30000:1001 Ip A1:1 C420jpeg XYSCSS=420JPEG\n".to_vec();
        for k in 0..3u8 {
            d.extend_from_slice(b"FRAME\n");
            d.extend(std::iter::repeat_n(k, 4 * 2 + 2 * 2));
        }
        let m = parse(&d, true).unwrap();
        let v = m.video.unwrap();
        assert_eq!((v.width, v.height, v.samples.len()), (4, 2, 3));
        assert_eq!(v.timescale, 30_000);
        assert_eq!(v.samples[1].dts, 1001);
        let desc = PixDesc::from_config(&v.config).unwrap();
        assert_eq!(&desc.fourcc, b"Y408");
    }
}
