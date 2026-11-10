//! VP8 and VP9 (WebM, Matroska, MP4, IVF) through the in-tree decoders `specular-vp8` and
//! `specular-vp9`, Theora (Ogg, Matroska) through `specular-theora`, JPEG XS (MP4 `jxsm`)
//! through `specular-jpegxs`, ProRes through [`crate::prores`], FFV1 through
//! [`crate::ffv1`], uncompressed video through [`crate::rawvid`] and the old codecs through
//! [`crate::oldvid`]: pictures in the hardware decoder's layout for [`crate::play`].
//!
//! Every coded block gives at most one picture to show (hidden frames come out with the
//! frame after them; a VP9 superframe shows one). VP9 4:4:0 stays 4:4:0; RGB keeps
//! its G, B, R planes under the identity matrix. JPEG XS in RGB, Bayer or greyscale is
//! refused in words.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};
use specular_vp9::ColorSpace;

use crate::mp4::Codec;
use crate::play::PlayError;

/// One clip's software decoder.
pub enum Vpx {
    Vp8(Box<specular_vp8::Decoder>),
    Vp9(Box<specular_vp9::Decoder>, Vec<specular_vp9::Picture>),
    /// JPEG XS keeps the frame buffer temporal prediction (TDC profiles) works from.
    JpegXs(Box<specular_jpegxs::Sequence>),
    Theora(Box<specular_theora::Decoder>),
    /// Every ProRes frame stands alone; 4444 frames may carry alpha.
    ProRes(Box<crate::prores::ProResDec>),
    /// Motion JPEG: every sample a picture or a pair of fields.
    Mjpeg(Box<crate::mjpeg::MjpegDec>),
    /// JPEG 2000: every sample one or two codestreams.
    Jp2,
    /// DV: every sample a frame.
    Dv,
    /// FFV1: frames stand alone, but the contexts carry over between keyframes.
    Ffv1(Box<crate::ffv1::Ffv1Dec>),
    /// Uncompressed: every sample a frame.
    Raw(Box<crate::rawvid::RawDec>),
    /// The old codecs: most frames change part of the picture before.
    Old(Box<crate::oldvid::OldDec>),
    /// CineForm: the second frame of a two-frame group waits in the decoder.
    CineForm(Box<crate::newvid::CineFormDec>),
    /// APV: every access unit stands alone; 4:4:4:4 frames carry alpha.
    Apv(Box<crate::apv::ApvDec>),
    /// HAP: every sample a texture frame.
    Hap(Box<crate::newvid::HapDec>),
    /// VC-2 / Dirac: the sequence header in force carries over.
    Vc2(Box<crate::newvid::Vc2Dec>),
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
}

impl Vpx {
    /// The decoder of `codec` with the track's configuration (Theora's headers).
    pub fn new(codec: Codec, config: &[u8]) -> Result<Vpx, PlayError> {
        match codec {
            Codec::Theora => {
                let [ident, _, setup] = specular_theora::split_xiph_laced(config)
                    .ok_or_else(|| PlayError::Malformed("Theora headers".into()))?;
                specular_theora::Decoder::new(ident, setup)
                    .map(|d| Vpx::Theora(Box::new(d)))
                    .map_err(|e| match e {
                        specular_theora::Error::Unsupported(m) => PlayError::Unsupported(m),
                        specular_theora::Error::Malformed(m) => PlayError::Malformed(m),
                        specular_theora::Error::NeedsMemory(n) => PlayError::NeedsMemory(n),
                    })
            }
            Codec::Vp8 => Ok(Vpx::Vp8(Box::new(specular_vp8::Decoder::new(threads())))),
            Codec::Vp9 => Ok(Vpx::Vp9(
                Box::new(specular_vp9::Decoder::new(threads())),
                Vec::new(),
            )),
            Codec::JpegXs => Ok(Vpx::JpegXs(Box::default())),
            Codec::ProRes => Ok(Vpx::ProRes(Box::default())),
            Codec::Mjpeg => Ok(Vpx::Mjpeg(Box::default())),
            Codec::Jpeg2000 => Ok(Vpx::Jp2),
            Codec::Dv => Ok(Vpx::Dv),
            c => Err(PlayError::Unsupported(c.name())),
        }
    }

    /// The decoder of a track: [`Vpx::new`] with what the container says beside the
    /// configuration (the field order).
    pub fn for_track(track: &crate::mp4::VideoTrack) -> Result<Vpx, PlayError> {
        let codec = track
            .codec
            .ok_or_else(|| PlayError::Malformed("no sample entry".into()))?;
        match codec {
            Codec::Mjpeg => Ok(Vpx::Mjpeg(Box::new(crate::mjpeg::MjpegDec::new(
                track.bottom_first,
            )))),
            Codec::Vp3 => specular_theora::Decoder::new_vp3(track.width, track.height)
                .map(|d| Vpx::Theora(Box::new(d)))
                .map_err(|e| PlayError::Unsupported(format!("VP3: {e}"))),
            Codec::Ffv1 => Ok(Vpx::Ffv1(Box::new(crate::ffv1::Ffv1Dec::new(track)?))),
            Codec::Raw => Ok(Vpx::Raw(Box::new(crate::rawvid::RawDec::new(track)?))),
            Codec::Classic(c) => Ok(Vpx::Old(Box::new(crate::oldvid::OldDec::new(c, track)?))),
            Codec::CineForm => Ok(Vpx::CineForm(Box::default())),
            Codec::Apv => Ok(Vpx::Apv(Box::default())),
            Codec::Hap(_) => Ok(Vpx::Hap(Box::new(crate::newvid::HapDec::new(track)))),
            Codec::Vc2 => Ok(Vpx::Vc2(Box::new(crate::newvid::Vc2Dec::new(track)?))),
            c => Vpx::new(c, &track.config),
        }
    }

    /// The codecs this backend decodes.
    pub fn handles(codec: Codec) -> bool {
        matches!(
            codec,
            Codec::Vp8
                | Codec::Vp9
                | Codec::JpegXs
                | Codec::Theora
                | Codec::ProRes
                | Codec::Mjpeg
                | Codec::Jpeg2000
                | Codec::Dv
                | Codec::Vp3
                | Codec::Ffv1
                | Codec::Raw
                | Codec::Classic(_)
                | Codec::CineForm
                | Codec::Hap(_)
                | Codec::Vc2
                | Codec::Apv
        )
    }

    /// Decodes one block; the picture it shows, if any. A damaged block costs its picture
    /// (`Ok(None)`); a coding the player can't show is an error.
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        match self {
            Vpx::Theora(d) => match d.decode(data) {
                Ok(p) => Ok(Some(theora_frame(p, d.info()))),
                Err(specular_theora::Error::Unsupported(m)) => Err(DecodeError::Unsupported(m)),
                Err(specular_theora::Error::NeedsMemory(n)) => Err(DecodeError::NeedsMemory(n)),
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "video: Theora frame lost: {e}");
                    Ok(None)
                }
            },
            Vpx::Vp8(d) => match d.decode(data) {
                Ok(p) => Ok(p.map(vp8_frame)),
                Err(specular_vp8::Error::Unsupported(m)) => Err(DecodeError::Unsupported(m)),
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "video: VP8 frame lost: {e}");
                    Ok(None)
                }
            },
            Vpx::Vp9(d, out) => {
                out.clear();
                match d.decode(data, out) {
                    Ok(()) => {}
                    Err(specular_vp9::Error::Unsupported(m)) => {
                        return Err(DecodeError::Unsupported(m));
                    }
                    Err(e) => {
                        specular_log::debug!(target: specular_log::DECODE, "video: VP9 frame lost: {e}");
                    }
                }
                // A block shows one picture; should a stream show more, the last stands.
                out.pop().map(vp9_frame).transpose()
            }
            Vpx::ProRes(d) => d.decode(data),
            Vpx::Mjpeg(d) => d.decode(data),
            Vpx::Jp2 => crate::jp2v::decode(data),
            Vpx::Dv => crate::dvfile::decode(data),
            Vpx::Ffv1(d) => d.decode(data),
            Vpx::Raw(d) => d.decode(data),
            Vpx::Old(d) => d.decode(data),
            Vpx::CineForm(d) => d.decode(data),
            Vpx::Apv(d) => d.decode(data),
            Vpx::Hap(d) => d.decode(data),
            Vpx::Vc2(d) => d.decode(data),
            Vpx::JpegXs(seq) => match seq.decode(data) {
                Ok(s) => jpegxs_frame(data, s).map(Some),
                Err(specular_jpegxs::Error::Unsupported(m)) => Err(DecodeError::Unsupported(m)),
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "video: JPEG XS frame lost: {e}");
                    Ok(None)
                }
            },
        }
    }

    /// Forgets everything (decoding goes on from a keyframe).
    pub fn reset(&mut self) {
        match self {
            Vpx::Vp8(d) => d.reset(),
            Vpx::Vp9(d, out) => {
                d.reset();
                out.clear();
            }
            Vpx::JpegXs(seq) => **seq = specular_jpegxs::Sequence::new(),
            Vpx::Theora(d) => d.reset(),
            Vpx::ProRes(d) => **d = Default::default(),
            Vpx::Mjpeg(_) | Vpx::Jp2 | Vpx::Dv | Vpx::Raw(_) => {}
            Vpx::Ffv1(d) => d.reset(),
            Vpx::Old(d) => d.reset(),
            Vpx::CineForm(d) => d.reset(),
            Vpx::Apv(d) => **d = Default::default(),
            Vpx::Hap(_) => {}
            Vpx::Vc2(d) => d.reset(),
        }
    }

    /// The alpha plane of the picture last decoded, when it has one (ProRes 4444, FFV1 and
    /// raw layouts with alpha, QuickTime Animation in ARGB).
    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        match self {
            Vpx::ProRes(d) => d.take_alpha(),
            Vpx::Ffv1(d) => d.take_alpha(),
            Vpx::Raw(d) => d.take_alpha(),
            Vpx::Old(d) => d.take_alpha(),
            Vpx::CineForm(d) => d.take_alpha(),
            Vpx::Apv(d) => d.take_alpha(),
            Vpx::Hap(d) => d.take_alpha(),
            _ => None,
        }
    }

    /// Chroma sits between luma columns (VP8, like JPEG) or on even ones (VP9 and JPEG XS,
    /// like MPEG-2).
    pub fn cosited_x(&self) -> bool {
        matches!(
            self,
            Vpx::Vp9(..)
                | Vpx::JpegXs(_)
                | Vpx::ProRes(_)
                | Vpx::Jp2
                | Vpx::Raw(_)
                | Vpx::Ffv1(_)
                | Vpx::CineForm(_)
                | Vpx::Apv(_)
        )
    }
}

fn theora_frame(p: specular_theora::Picture, info: &specular_theora::Info) -> VideoFrame {
    use specular_theora::Chroma as C;
    VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: 8,
        chroma: match p.chroma {
            C::C420 => Chroma::Yuv420,
            C::C422 => Chroma::Yuv422,
            C::C444 => Chroma::Yuv444,
        },
        planes: p.planes,
        // Theora's colour spaces (Rec. 470 M and BG) are BT.601, studio range.
        signal: Some(Signal {
            full_range: false,
            primaries: match info.colour_space {
                1 => Some(4),
                2 => Some(5),
                _ => None,
            },
            transfer: None,
            matrix: Some(6),
        }),
    }
}

/// Why a Theora track can't play, in words, from its identification header.
pub fn theora_refusal(config: &[u8]) -> Option<String> {
    let Some([ident, _, _]) = specular_theora::split_xiph_laced(config) else {
        return Some("Theora video without its headers".into());
    };
    match specular_theora::Info::parse(ident) {
        Ok(_) => None,
        Err(specular_theora::Error::Unsupported(m)) => Some(m),
        Err(_) => Some("damaged Theora video".into()),
    }
}

fn vp8_frame(p: specular_vp8::Picture) -> VideoFrame {
    VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: 8,
        chroma: Chroma::Yuv420,
        planes: p.planes,
        // VP8 is BT.601, studio range.
        signal: Some(Signal {
            full_range: false,
            primaries: None,
            transfer: None,
            matrix: Some(6),
        }),
    }
}

fn vp9_frame(p: specular_vp9::Picture) -> Result<VideoFrame, DecodeError> {
    if matches!(p.color_space, ColorSpace::Srgb) {
        // Planes G, B, R as coded, full range: the identity matrix (H.273 MC 0) shows them
        // as they are. (They were converted to BT.709 YCbCr two bits deeper, which changed
        // the decoded samples and rounded the colours.)
        return Ok(VideoFrame {
            width: p.width,
            height: p.height,
            bit_depth: p.bit_depth,
            chroma: Chroma::Yuv444,
            planes: p.planes,
            signal: Some(Signal {
                full_range: true,
                primaries: Some(1),
                transfer: Some(13),
                matrix: Some(0),
            }),
        });
    }
    let chroma = match (p.ss_x, p.ss_y) {
        (true, true) => Chroma::Yuv420,
        (true, false) => Chroma::Yuv422,
        // 4:4:0: every chroma row covers two luma rows (the viewer interpolates them).
        (false, true) => Chroma::Yuv440,
        (false, false) => Chroma::Yuv444,
    };
    let matrix = match p.color_space {
        ColorSpace::Bt601 | ColorSpace::Smpte170 => Some(6),
        ColorSpace::Bt709 => Some(1),
        ColorSpace::Smpte240 => Some(7),
        ColorSpace::Bt2020 => Some(9),
        _ => None,
    };
    Ok(VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: p.bit_depth,
        chroma,
        planes: p.planes,
        signal: Some(Signal {
            full_range: p.full_range,
            primaries: None,
            transfer: None,
            matrix,
        }),
    })
}

/// The layout of a JPEG XS codestream the player can show: YCbCr 4:2:2 or 4:2:0.
fn jpegxs_chroma(h: &specular_jpegxs::Header) -> Result<Chroma, String> {
    let c = &h.components;
    if h.cpih == 3 || h.cfa.is_some() {
        return Err("JPEG XS Bayer video".into());
    }
    if c.len() < 3 {
        return Err("greyscale JPEG XS video".into());
    }
    if h.cpih != 0 {
        return Err("JPEG XS video in RGB".into());
    }
    match ((c[1].sx, c[1].sy), (c[2].sx, c[2].sy)) {
        ((2, 1), (2, 1)) => Ok(Chroma::Yuv422),
        ((2, 2), (2, 2)) => Ok(Chroma::Yuv420),
        ((1, 1), (1, 1)) => Ok(Chroma::Yuv444),
        _ => Err("JPEG XS video with this chroma sampling".into()),
    }
}

/// Why a JPEG XS track can't play, from its first sample: anything but YCbCr 4:4:4, 4:2:2
/// and 4:2:0 (what the player shows), or what the decoder refuses.
pub fn jpegxs_refusal(first: &[u8]) -> Option<String> {
    let h = match specular_jpegxs::header::parse(first) {
        Ok(h) => h,
        Err(specular_jpegxs::Error::Unsupported(m)) => return Some(m),
        Err(_) => return None,
    };
    jpegxs_chroma(&h).err()
}

/// A decoded JPEG XS frame as Y, Cb and Cr planes (chroma `ceil(w/2)` wide).
fn jpegxs_frame(data: &[u8], s: specular_jpegxs::Samples) -> Result<VideoFrame, DecodeError> {
    let h =
        specular_jpegxs::header::parse(data).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    let chroma = jpegxs_chroma(&h).map_err(DecodeError::Unsupported)?;
    let (w, ht) = (h.width as usize, h.height as usize);
    let (cw, ch) = chroma.plane_size(h.width, h.height);
    let (cw, ch) = (cw as usize, ch as usize);
    let wide = s.depth > 8;
    // Copies a plane at `tw`×`th`, repeating its last column and row where it is smaller.
    let plane = |p: &specular_jpegxs::SamplePlane, tw: usize, th: usize| -> Vec<u8> {
        let mut out = Vec::with_capacity(tw * th * if wide { 2 } else { 1 });
        for y in 0..th {
            let sy = y.min(p.height.saturating_sub(1));
            for x in 0..tw {
                let sx = x.min(p.width.saturating_sub(1));
                let v = p.data.get(sy * p.width + sx).copied().unwrap_or(0);
                if wide {
                    out.extend_from_slice(&v.to_le_bytes());
                } else {
                    out.push(v as u8);
                }
            }
        }
        out
    };
    let [y, cb, cr, ..] = s.planes.as_slice() else {
        return Err(DecodeError::Malformed(
            "a JPEG XS frame without chroma".into(),
        ));
    };
    Ok(VideoFrame {
        width: w as u32,
        height: ht as u32,
        bit_depth: s.depth,
        chroma,
        planes: [plane(y, w, ht), plane(cb, cw, ch), plane(cr, cw, ch)],
        // The codestream says nothing of colour: the container's `colr`, else studio range
        // with the matrix players assume for the size.
        signal: None,
    })
}

/// Why a VP9 track can't play, from its first keyframe: nothing the decoder knows of (every
/// profile, sampling and colour space plays; 4:4:0 and RGB are converted for the player).
pub fn vp9_refusal(first_key: &[u8]) -> Option<String> {
    specular_vp9::probe(first_key)?;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4::build::{Spec, movie};

    const JXS_422: &[u8] =
        include_bytes!("../../formats/specular-jpegxs/tests/fixtures/main422-10.jxs");
    const JXS_420: &[u8] =
        include_bytes!("../../formats/specular-jpegxs/tests/fixtures/main420-12.jxs");
    const JXS_RGB: &[u8] =
        include_bytes!("../../formats/specular-jpegxs/tests/fixtures/main444-12-rct.jxs");
    const JXS_444: &[u8] =
        include_bytes!("../../formats/specular-jpegxs/tests/fixtures/high444-12-none.jxs");

    #[test]
    fn jpeg_xs_tracks_play_frame_by_frame() {
        let m = movie(&Spec {
            codec: b"jxsm",
            config: b"\x35\x40\x00\x00",
            samples: &[JXS_422, JXS_422],
            ..Spec::default()
        });
        let parsed = crate::mp4::parse(&m, true).unwrap();
        let v = parsed.video.unwrap();
        assert_eq!(v.codec, Some(Codec::JpegXs));
        assert_eq!(v.codec.unwrap().name(), "JPEG XS");
        assert_eq!(v.samples.len(), 2);
        let mut dec = Vpx::new(Codec::JpegXs, &[]).unwrap();
        assert!(dec.cosited_x());
        for s in &v.samples {
            let bytes = v.sample_data(&m, s).unwrap();
            let f = dec.decode(&bytes).unwrap().expect("a picture");
            assert_eq!((f.width, f.height, f.bit_depth), (64, 32, 10));
            assert_eq!(f.chroma, Chroma::Yuv422);
            assert_eq!(f.planes[0].len(), 64 * 32 * 2);
            assert_eq!(f.planes[1].len(), 32 * 32 * 2);
        }
        let mut dec = Vpx::new(Codec::JpegXs, &[]).unwrap();
        let f = dec.decode(JXS_420).unwrap().unwrap();
        assert_eq!(f.chroma, Chroma::Yuv420);
        assert_eq!(f.planes[2].len(), 32 * 16 * 2);
        dec.reset();
        // A damaged frame costs its picture only.
        assert!(dec.decode(&JXS_422[..20]).unwrap().is_none());
    }

    #[test]
    fn jpeg_xs_rgb_is_refused_in_words_and_444_plays() {
        assert_eq!(jpegxs_refusal(JXS_422), None);
        assert_eq!(jpegxs_refusal(JXS_420), None);
        assert_eq!(jpegxs_refusal(JXS_444), None);
        assert_eq!(
            jpegxs_refusal(JXS_RGB).as_deref(),
            Some("JPEG XS video in RGB")
        );
        let mut dec = Vpx::new(Codec::JpegXs, &[]).unwrap();
        let f = dec.decode(JXS_444).unwrap().unwrap();
        assert_eq!(f.chroma, Chroma::Yuv444);
        assert_eq!(f.planes[1].len(), f.planes[0].len());
    }
}
