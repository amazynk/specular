//! GoPro CineForm (`specular-cineform`), Vidvox HAP (`specular-hap`) and SMPTE VC-2 / Dirac
//! (`specular-vc2`): pictures in the hardware decoder's layout for [`crate::play`]. RGB
//! pictures keep their G, B, R planes under the identity matrix; alpha comes beside them.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};

use crate::mp4::VideoTrack;
use crate::play::PlayError;

fn u16_bytes(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|s| s.to_le_bytes()).collect()
}

fn u8_plane(v: &[u16]) -> Vec<u8> {
    v.iter().map(|&s| s as u8).collect()
}

/// Samples at `bits` as plane bytes (bytes at 8 bits, else little-endian `u16`).
fn plane(v: &[u16], bits: u8) -> Vec<u8> {
    if bits <= 8 { u8_plane(v) } else { u16_bytes(v) }
}

const RGB: Signal = Signal {
    full_range: true,
    primaries: Some(1),
    transfer: Some(13),
    matrix: Some(0),
};

/// The first sample's bytes.
fn first_sample(data: &[u8], track: &VideoTrack) -> Option<Vec<u8>> {
    let s = track.samples.first()?;
    track.sample_data(data, s).map(|c| c.into_owned())
}

// ---- CineForm ----

/// "YUV 4:2:2, 10-bit" (from the first sample).
pub fn cineform_profile(data: &[u8], track: &VideoTrack) -> Option<String> {
    specular_cineform::probe(&first_sample(data, track)?)
        .ok()
        .map(|i| i.describe())
}

pub fn cineform_refusal(data: &[u8], track: &VideoTrack) -> Option<String> {
    specular_cineform::refusal(&first_sample(data, track)?)
}

pub fn cineform_info(data: &[u8], track: &VideoTrack) -> Option<specular_cineform::Info> {
    specular_cineform::probe(&first_sample(data, track)?).ok()
}

#[derive(Default)]
pub struct CineFormDec {
    dec: specular_cineform::Decoder,
    alpha: Option<Vec<u8>>,
}

impl CineFormDec {
    pub fn reset(&mut self) {
        self.dec.reset();
        self.alpha = None;
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        self.alpha = None;
        let pic = match self.dec.decode(data) {
            Ok(p) => p,
            Err(specular_cineform::Error::Unsupported(m)) => {
                return Err(DecodeError::Unsupported(m));
            }
            Err(specular_cineform::Error::NeedsMemory(n)) => {
                return Err(DecodeError::NeedsMemory(n));
            }
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: CineForm frame lost: {e}");
                return Ok(None);
            }
        };
        let bits = pic.bits;
        let p = &pic.planes;
        let (chroma, planes, signal) = match pic.layout {
            specular_cineform::Layout::Yuv422 => (
                Chroma::Yuv422,
                [plane(&p[0], bits), plane(&p[1], bits), plane(&p[2], bits)],
                None,
            ),
            _ => {
                if let Some(a) = p.get(3) {
                    self.alpha = Some(plane(a, bits));
                }
                (
                    Chroma::Yuv444,
                    [plane(&p[1], bits), plane(&p[2], bits), plane(&p[0], bits)],
                    Some(RGB),
                )
            }
        };
        Ok(Some(VideoFrame {
            width: pic.width,
            height: pic.height,
            bit_depth: bits,
            chroma,
            planes,
            signal,
        }))
    }

    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        self.alpha.take()
    }
}

// ---- HAP ----

#[derive(Default)]
pub struct HapDec {
    alpha: Option<Vec<u8>>,
    size: (u32, u32),
}

impl HapDec {
    pub fn new(track: &VideoTrack) -> HapDec {
        HapDec {
            alpha: None,
            size: (track.width, track.height),
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        self.alpha = None;
        let (w, h) = self.size;
        let img = match specular_hap::decode(data, w, h) {
            Ok(i) => i,
            Err(specular_hap::Error::Unsupported(m)) => return Err(DecodeError::Unsupported(m)),
            Err(specular_hap::Error::NeedsMemory(n)) => return Err(DecodeError::NeedsMemory(n)),
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: HAP frame lost: {e}");
                return Ok(None);
            }
        };
        let n = (img.width * img.height) as usize;
        let (mut g, mut b, mut r) = (vec![0u8; n], vec![0u8; n], vec![0u8; n]);
        let mut a = img.kind.has_alpha().then(|| vec![0u8; n]);
        for (i, px) in img.rgba.chunks_exact(4).enumerate().take(n) {
            (r[i], g[i], b[i]) = (px[0], px[1], px[2]);
            if let Some(a) = a.as_mut() {
                a[i] = px[3];
            }
        }
        self.alpha = a;
        Ok(Some(VideoFrame {
            width: img.width,
            height: img.height,
            bit_depth: 8,
            chroma: Chroma::Yuv444,
            planes: [g, b, r],
            signal: Some(RGB),
        }))
    }

    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        self.alpha.take()
    }
}

/// The HAP variant of a track, from its first frame (else its four-character code).
pub fn hap_kind(data: &[u8], track: &VideoTrack, coded: specular_hap::Kind) -> specular_hap::Kind {
    first_sample(data, track)
        .and_then(|f| specular_hap::probe(&f).ok())
        .unwrap_or(coded)
}

pub fn hap_refusal(data: &[u8], track: &VideoTrack) -> Option<String> {
    match specular_hap::probe(&first_sample(data, track)?) {
        Err(specular_hap::Error::Unsupported(m)) => Some(m),
        _ => None,
    }
}

// ---- VC-2 / Dirac ----

/// The stream bytes to look into: the configuration (a sequence header in Ogg) and the first
/// sample.
fn vc2_head(data: &[u8], track: &VideoTrack) -> Vec<u8> {
    let mut v = track.config.clone();
    if let Some(s) = first_sample(data, track) {
        v.extend_from_slice(&s);
    }
    v
}

pub fn vc2_info(data: &[u8], track: &VideoTrack) -> Option<specular_vc2::Info> {
    specular_vc2::probe(&vc2_head(data, track))
}

/// "VC-2 high quality, 4:2:2, 10-bit".
pub fn vc2_profile(data: &[u8], track: &VideoTrack) -> Option<String> {
    let i = vc2_info(data, track)?;
    let c = match i.chroma {
        specular_vc2::Chroma::C444 => "4:4:4",
        specular_vc2::Chroma::C422 => "4:2:2",
        specular_vc2::Chroma::C420 => "4:2:0",
    };
    let mut s = format!("{}, {c}, {}-bit", i.profile, i.bit_depth);
    if i.interlaced {
        s.push_str(", interlaced");
    }
    Some(s)
}

pub fn vc2_refusal(data: &[u8], track: &VideoTrack) -> Option<String> {
    specular_vc2::refusal(&vc2_head(data, track))
}

pub struct Vc2Dec {
    dec: specular_vc2::Decoder,
    /// A sequence header the container keeps apart (Ogg's first packet).
    config: Vec<u8>,
}

impl Vc2Dec {
    pub fn new(track: &VideoTrack) -> Result<Vc2Dec, PlayError> {
        let mut d = Vc2Dec {
            dec: specular_vc2::Decoder::new(),
            config: track.config.clone(),
        };
        d.reset();
        Ok(d)
    }

    pub fn reset(&mut self) {
        self.dec.reset();
        if !self.config.is_empty() {
            let _ = self.dec.decode(&self.config);
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        let pic = match self.dec.decode(data) {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(None),
            Err(specular_vc2::Error::Unsupported(m)) => return Err(DecodeError::Unsupported(m)),
            Err(specular_vc2::Error::NeedsMemory(n)) => return Err(DecodeError::NeedsMemory(n)),
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "video: VC-2 picture lost: {e}");
                return Ok(None);
            }
        };
        // The player takes 8 to 14 bits: shallower samples are scaled up, deeper ones down.
        let (bits, shift) = match pic.bit_depth {
            b @ 0..=7 => (8u8, i32::from(8 - b)),
            b @ 8..=14 => (b, 0),
            b => (14, 14 - i32::from(b)),
        };
        let fit = |v: &[u16]| -> Vec<u8> {
            if shift == 0 {
                return plane(v, bits);
            }
            let w: Vec<u16> = v
                .iter()
                .map(|&x| if shift > 0 { x << shift } else { x >> -shift })
                .collect();
            plane(&w, bits)
        };
        let chroma = match pic.chroma {
            specular_vc2::Chroma::C444 => Chroma::Yuv444,
            specular_vc2::Chroma::C422 => Chroma::Yuv422,
            specular_vc2::Chroma::C420 => Chroma::Yuv420,
        };
        let s = pic.signal;
        let [y, cb, cr] = &pic.planes;
        Ok(Some(VideoFrame {
            width: pic.width,
            height: pic.height,
            bit_depth: bits,
            chroma,
            planes: [fit(y), fit(cb), fit(cr)],
            signal: Some(Signal {
                full_range: s.full_range,
                primaries: s.primaries,
                transfer: s.transfer,
                matrix: s.matrix,
            }),
        }))
    }
}
