//! DICOM (`.dcm`, `.dicom`, and extensionless Part 10 files) through `specular-dicom`.
//!
//! A session shows one frame: the first, or the one [`open_dicom_frame`] names
//! ([`Session::page_count`] is the number of frames). Grey frames are rendered through the
//! modality LUT (rescale or Modality LUT Sequence, giving HU for CT) and the default VOI (the
//! first window, else the VOI LUT, else the value range without padding), inverted for
//! MONOCHROME1, as 16-bit grey; colour frames as RGBA (8 or 16 bits). The modality values stay
//! on the session ([`Session::modality_frame`]) for later window / level and value readouts.
//! Patient-identifying attributes are never read into the session; the info panel gets only
//! the [`MedicalSummary`].

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use specular_core::{CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, Source};
use specular_dicom as dcm;

use crate::whole::{self, Levels};
use crate::{
    AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo, Result,
    Session,
};

pub use dcm::{
    Calibration, Graphics, ModalityFrame, SpacingKind, Summary as MedicalSummary, Uncalibrated,
    Voi, VoiFunction, VoiSource,
};

pub struct DicomDecoder;

fn map_err(e: dcm::Error) -> DecodeError {
    match e {
        dcm::Error::NotDicom => DecodeError::UnsupportedFormat,
        dcm::Error::Truncated => DecodeError::Truncated("DICOM data set".into()),
        dcm::Error::Malformed(s) => DecodeError::Malformed(s.into()),
        dcm::Error::Unsupported(s) => DecodeError::Unsupported(s),
        dcm::Error::NoPixelData => DecodeError::Unsupported("DICOM file without an image".into()),
        dcm::Error::TooLarge(_) => DecodeError::TooLarge,
        dcm::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        dcm::Error::Codec(s) => DecodeError::Malformed(s),
        dcm::Error::Cancelled => DecodeError::Cancelled,
    }
}

impl Decoder for DicomDecoder {
    fn format(&self) -> Format {
        Format::Dicom
    }
    fn probe(&self, head: &[u8]) -> bool {
        // Never true for an empty head, so `.dcm` files without a recognisable header still
        // reach this decoder by extension.
        !head.is_empty() && dcm::sniff(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        !data.is_empty() && dcm::sniff(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dcm", "dicom"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        // A whole-slide instance opens its series as one pyramid; one the slide reader cannot
        // make sense of still shows its first frame.
        if specular_wsi::dicomwsi::probe(source.bytes()) {
            match crate::wsi_dec::open(source.clone()) {
                Ok(s) => return Ok(s),
                Err(DecodeError::Cancelled) => return Err(DecodeError::Cancelled),
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "DICOM WSI: {e}; showing the first frame")
                }
            }
        }
        open_dicom_frame(source, 0)
    }
}

/// Opens frame `frame` (0-based) of a DICOM file as its own session.
pub fn open_dicom_frame(source: Arc<Source>, frame: u32) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let d = dcm::Dicom::parse(data).map_err(map_err)?;
    if d.pixel.is_none()
        && let Some(doc) = d.document(data)
    {
        return crate::dicom_doc::open(source.clone(), &d, doc);
    }
    // A structured report or a waveform: a document of its content.
    if d.pixel.is_none()
        && let Some(r) = crate::dicom_report::open(&source, &d)
    {
        return r;
    }
    d.check_supported().map_err(map_err)?;
    let p = d.pixel.clone().ok_or(DecodeError::Unsupported(
        "DICOM file without an image".into(),
    ))?;
    if frame >= p.frames {
        return Err(DecodeError::Malformed("no such frame".into()));
    }
    let summary = d.summary(data);
    let mut coding = d.transfer_syntax.name.clone();
    if p.frames > 1 {
        coding += &format!(", frame {} of {}", frame + 1, p.frames);
    }
    let info = ImageInfo {
        format: Format::Dicom,
        width: p.columns,
        height: p.rows,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: p.bits_stored.min(32) as u8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    let model = match &p.photometric {
        dcm::Photometric::Monochrome1 | dcm::Photometric::Monochrome2 => "Greyscale",
        dcm::Photometric::Palette => "Palette",
        dcm::Photometric::Rgb => "RGB",
        _ if p.samples == 1 => "Greyscale",
        _ => "YCbCr",
    };
    let timing = d.frame_durations(data);
    // Colour cine (ultrasound loops) plays through the animation player; grey multi-frame
    // images step and play in medical mode on their modality values.
    let anim = (p.frames > 1 && p.is_colour()).then_some(AnimationInfo {
        frame_count: Some(p.frames),
        loop_count: 0,
    });
    Ok(Arc::new(DicomSession {
        source,
        info,
        model,
        dicom: Arc::new(d),
        frame,
        summary,
        timing,
        anim,
        values: Mutex::new(None),
        levels: Levels::new(),
    }))
}

struct DicomSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: &'static str,
    dicom: Arc<dcm::Dicom>,
    frame: u32,
    summary: MedicalSummary,
    /// Per-frame cine durations, when the file states them.
    timing: Option<Vec<Duration>>,
    anim: Option<AnimationInfo>,
    /// The frame's stored / modality values, kept after the first decode.
    values: Mutex<Option<Arc<ModalityFrame>>>,
    levels: Levels,
}

impl DicomSession {
    fn values(&self, cancel: &CancelToken) -> Result<Arc<ModalityFrame>> {
        let mut g = self.values.lock();
        if let Some(v) = &*g {
            return Ok(v.clone());
        }
        let m = self
            .dicom
            .modality_frame(self.source.bytes(), self.frame, Some(cancel.flag()))
            .map_err(map_err)?;
        let m = Arc::new(m);
        *g = Some(m.clone());
        Ok(m)
    }

    fn decode_full(&self, cancel: &CancelToken) -> Result<Raster> {
        let m = self.values(cancel)?;
        let img = m.render_default().map_err(map_err)?;
        Ok(raster(img))
    }
}

/// A rendered DICOM image as a one-plane raster.
pub(crate) fn raster(img: dcm::Image) -> Raster {
    let (layout, data) = match img.pixels {
        dcm::Rendered::Gray16(v) => (PixelLayout::Gray16, ne_bytes(&v)),
        dcm::Rendered::Rgba8(v) => (PixelLayout::Rgba, v),
        dcm::Rendered::Rgba16(v) => (PixelLayout::Rgba16(HdrEncoding::SRGB), ne_bytes(&v)),
    };
    whole::interleaved(img.width, img.height, layout, data, img.truncated)
}

fn ne_bytes(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_ne_bytes()).collect()
}

impl Session for DicomSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    /// The Icon Image Sequence, when the file has one.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        let icon = self.dicom.icon(self.source.bytes())?;
        Some(icon.map(raster).map_err(map_err))
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.decode_full(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let bpp = if self.dicom.pixel.as_ref().is_some_and(|p| p.is_colour()) {
            8
        } else {
            2
        };
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            m += whole::level_bytes(w, h, 1, bpp);
            if self.values.lock().is_none() {
                m += self.dicom.frame_memory();
            }
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
            + self.values.lock().as_ref().map_or(0, |v| v.memory_bytes())
            + self.dicom.video_memory()
    }
    /// Drops the cached levels, the frame's modality values and the decoded video frames kept
    /// for stepping.
    fn trim_memory(&self) -> usize {
        let values = self.values.lock().take().map_or(0, |v| v.memory_bytes());
        self.levels.clear() + values + self.dicom.trim_video()
    }
    /// Expanding a frame at a reduced scale costs as much as at full size.
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.dicom.frame_count().max(1)
    }
    fn medical(&self) -> Option<&MedicalSummary> {
        Some(&self.summary)
    }
    fn modality_frame(&self, cancel: &CancelToken) -> Option<Result<Arc<ModalityFrame>>> {
        Some(self.values(cancel))
    }
    fn modality_frame_at(
        &self,
        index: u32,
        cancel: &CancelToken,
    ) -> Option<Result<Arc<ModalityFrame>>> {
        if index == self.frame {
            return Some(self.values(cancel));
        }
        Some(
            self.dicom
                .modality_frame(self.source.bytes(), index, Some(cancel.flag()))
                .map(Arc::new)
                .map_err(map_err),
        )
    }
    fn cine_timing(&self) -> Option<&[Duration]> {
        self.timing.as_deref()
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.anim.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        let Some(a) = &self.anim else {
            return Err(DecodeError::Unsupported("not animated".into()));
        };
        Ok(Box::new(DicomFrames {
            source: self.source.clone(),
            dicom: self.dicom.clone(),
            next: 0,
            count: a.frame_count.unwrap_or(1),
            delays: self.timing.clone(),
            width: self.info.width,
            height: self.info.height,
            scale: scale.clamp(1, 8),
            video: self.dicom.video_sequence(),
        }))
    }
}

/// Default cine rate for files without timing: 10 frames a second.
pub const DEFAULT_CINE_FRAME: Duration = Duration::from_millis(100);

/// The frames of a colour multi-frame DICOM (ultrasound cine), each rendered as the still is.
struct DicomFrames {
    source: Arc<Source>,
    dicom: Arc<dcm::Dicom>,
    next: u32,
    count: u32,
    delays: Option<Vec<Duration>>,
    width: u32,
    height: u32,
    scale: u32,
    /// H.264 / HEVC pixel data: decoded front to back with its own decoder.
    video: Option<dcm::video::Sequence>,
}

impl Frames for DicomFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.next >= self.count {
            return None;
        }
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let mut index = self.next;
        self.next += 1;
        let data = self.source.bytes();
        let img = match &mut self.video {
            Some(seq) => match seq.next_frame(self.dicom.buf(data), Some(cancel.flag()))? {
                Ok((i, f)) => {
                    index = i;
                    self.next = i + 1;
                    self.dicom
                        .modality_of(data, i, f)
                        .and_then(|m| m.render_default())
                }
                Err(e) => Err(e),
            },
            None => self
                .dicom
                .modality_frame(data, index, Some(cancel.flag()))
                .and_then(|m| m.render_default()),
        };
        let mut raster = match img {
            Ok(i) => raster(i),
            Err(e) => return Some(Err(map_err(e))),
        };
        if self.scale > 1 {
            raster = whole::downscale(&raster, self.width, self.height, self.scale);
        }
        let delay = self
            .delays
            .as_ref()
            .and_then(|d| d.get(index as usize).copied())
            .unwrap_or(DEFAULT_CINE_FRAME);
        Some(Ok(Frame {
            index,
            raster,
            delay,
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        if let Some(seq) = &mut self.video {
            seq.rewind();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    /// A 4×2 explicit little-endian CT (signed, rescale -1024, window 40/400).
    fn ct() -> Vec<u8> {
        fn el(out: &mut Vec<u8>, tag: u32, vr: &[u8; 2], val: &[u8]) {
            out.extend(((tag >> 16) as u16).to_le_bytes());
            out.extend((tag as u16).to_le_bytes());
            out.extend(vr);
            if matches!(vr, b"OB" | b"OW" | b"SQ" | b"UN" | b"UT") {
                out.extend([0, 0]);
                out.extend((val.len() as u32).to_le_bytes());
            } else {
                out.extend((val.len() as u16).to_le_bytes());
            }
            out.extend(val);
        }
        let s = |v: &str| {
            let mut b = v.as_bytes().to_vec();
            if b.len() % 2 == 1 {
                b.push(b' ');
            }
            b
        };
        let mut meta = Vec::new();
        el(&mut meta, 0x0002_0010, b"UI", b"1.2.840.10008.1.2.1\0");
        let mut f = vec![0u8; 128];
        f.extend(b"DICM");
        el(
            &mut f,
            0x0002_0000,
            b"UL",
            &(meta.len() as u32).to_le_bytes(),
        );
        f.extend(&meta);
        el(&mut f, 0x0008_0060, b"CS", &s("CT"));
        el(&mut f, 0x0010_0010, b"PN", &s("Doe^Jane"));
        el(&mut f, 0x0018_0050, b"DS", &s("2.5"));
        el(&mut f, 0x0028_0002, b"US", &1u16.to_le_bytes());
        el(&mut f, 0x0028_0004, b"CS", &s("MONOCHROME2"));
        el(&mut f, 0x0028_0010, b"US", &2u16.to_le_bytes());
        el(&mut f, 0x0028_0011, b"US", &4u16.to_le_bytes());
        el(&mut f, 0x0028_0030, b"DS", &s("0.5\\0.6"));
        el(&mut f, 0x0028_0100, b"US", &16u16.to_le_bytes());
        el(&mut f, 0x0028_0101, b"US", &12u16.to_le_bytes());
        el(&mut f, 0x0028_0102, b"US", &11u16.to_le_bytes());
        el(&mut f, 0x0028_0103, b"US", &1u16.to_le_bytes());
        el(&mut f, 0x0028_1050, b"DS", &s("40"));
        el(&mut f, 0x0028_1051, b"DS", &s("400"));
        el(&mut f, 0x0028_1052, b"DS", &s("-1024"));
        el(&mut f, 0x0028_1053, b"DS", &s("1"));
        let px: Vec<u8> = [0i16, 864, 1064, 1264, 2047, -2048, 1000, 1100]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        el(&mut f, 0x7FE0_0010, b"OW", &px);
        f
    }

    fn open(name: &str, data: Vec<u8>) -> Result<Arc<dyn Session>> {
        Registry::default().open(Arc::new(Source::from_vec(name, data)))
    }

    #[test]
    fn opens_renders_and_keeps_values() {
        let s = open("x.bin", ct()).unwrap();
        assert_eq!(s.info().format, Format::Dicom);
        assert_eq!((s.info().width, s.info().height), (4, 2));
        assert_eq!(s.info().coding, "Explicit VR Little Endian");
        assert_eq!(s.colour_model(), Some("Greyscale"));
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.layout, PixelLayout::Gray16);
        let px: Vec<u16> = r.planes[0]
            .data
            .chunks_exact(2)
            .map(|b| u16::from_ne_bytes([b[0], b[1]]))
            .collect();
        // HU -1024, -160 (window bottom), 40 (centre), 240 (top), ...
        assert_eq!(px[0], 0);
        assert_eq!(px[1], 0);
        assert!((32000..33500).contains(&px[2]), "{}", px[2]);
        assert_eq!(px[3], 65535);
        let m = s.modality_frame(&c).unwrap().unwrap();
        assert_eq!(m.value_at(2, 0), Some(40.0));
        assert_eq!(m.unit(), Some("HU"));
        let med = s.medical().unwrap();
        assert_eq!(med.modality.as_deref(), Some("CT"));
        assert_eq!(med.slice_thickness, Some(2.5));
        assert_eq!(med.pixel_spacing, Some((0.5, 0.6)));
        // Nothing patient-identifying reaches the summary.
        assert!(!format!("{med:?}").contains("Doe"));
        let r2 = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!((r2.width, r2.height), (2, 1));
    }

    #[test]
    fn extension_without_signature_and_refusals() {
        // By extension when the header is not recognised: a clean error, not another decoder.
        let e = open("x.dcm", vec![1, 2, 3, 4]).err().unwrap();
        assert!(
            matches!(
                e,
                DecodeError::UnsupportedFormat
                    | DecodeError::Truncated(_)
                    | DecodeError::Malformed(_)
            ),
            "{e:?}"
        );
        // A JPEG is never taken for DICOM.
        assert!(!DicomDecoder.probe(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F']));
        assert!(!DicomDecoder.probe(&[]));
    }

    /// A 2×1 RGB file of three frames at 40 ms (frame k is grey 10 k + 10).
    fn rgb_cine(frame_time: bool) -> Vec<u8> {
        let mut f = ct();
        let at = f.windows(4).position(|w| w == [8, 0, 0x60, 0]).unwrap();
        f.truncate(at);
        let mut el = |tag: u32, vr: &[u8; 2], val: &[u8]| {
            f.extend(((tag >> 16) as u16).to_le_bytes());
            f.extend((tag as u16).to_le_bytes());
            f.extend(vr);
            if vr == b"OB" {
                f.extend([0, 0]);
                f.extend((val.len() as u32).to_le_bytes());
            } else {
                f.extend((val.len() as u16).to_le_bytes());
            }
            f.extend(val);
        };
        el(0x0008_0060, b"CS", b"US");
        if frame_time {
            el(0x0018_1063, b"DS", b"40");
        }
        el(0x0028_0002, b"US", &3u16.to_le_bytes());
        el(0x0028_0004, b"CS", b"RGB ");
        el(0x0028_0006, b"US", &0u16.to_le_bytes());
        el(0x0028_0008, b"IS", b"3 ");
        el(0x0028_0010, b"US", &1u16.to_le_bytes());
        el(0x0028_0011, b"US", &2u16.to_le_bytes());
        el(0x0028_0100, b"US", &8u16.to_le_bytes());
        el(0x0028_0101, b"US", &8u16.to_le_bytes());
        el(0x0028_0102, b"US", &7u16.to_le_bytes());
        el(0x0028_0103, b"US", &0u16.to_le_bytes());
        let px: Vec<u8> = (0..3u8).flat_map(|k| [10 * k + 10; 6]).collect();
        el(0x7FE0_0010, b"OB", &px);
        f
    }

    #[test]
    fn colour_cine_plays_as_an_animation() {
        let s = open("cine.dcm", rgb_cine(true)).unwrap();
        assert_eq!(s.animation().and_then(|a| a.frame_count), Some(3));
        assert_eq!(s.cine_timing().map(<[_]>::len), Some(3));
        let c = CancelToken::new();
        let mut fr = s.frames(1).unwrap();
        for k in 0..3u8 {
            let f = fr.next_frame(&c).unwrap().unwrap();
            assert_eq!(f.index, u32::from(k));
            assert_eq!(f.delay, Duration::from_millis(40));
            assert_eq!(
                f.raster.planes[0].data[..4],
                [10 * k + 10, 10 * k + 10, 10 * k + 10, 255]
            );
        }
        assert!(fr.next_frame(&c).is_none());
        fr.rewind().unwrap();
        assert_eq!(fr.next_frame(&c).unwrap().unwrap().index, 0);
        // Without timing: the default rate.
        let s = open("cine.dcm", rgb_cine(false)).unwrap();
        assert_eq!(s.cine_timing(), None);
        let f = s.frames(1).unwrap().next_frame(&c).unwrap().unwrap();
        assert_eq!(f.delay, DEFAULT_CINE_FRAME);
        // Grey multi-frame files are not animations; their frames come from the session.
        let g = open("x.dcm", ct()).unwrap();
        assert!(g.animation().is_none());
        let m = s.modality_frame_at(2, &c).unwrap().unwrap();
        assert_eq!(m.rgb_at(1, 0), Some(([30, 30, 30], 8)));
    }

    /// An H.264 DICOM of `n` 64x48 frames (IDR every 4), frame k grey-ish level 40 + 15 k.
    fn h264_dicom(n: u32) -> Vec<u8> {
        use specular_h264enc as enc;
        let mut cfg = enc::Config::new(64, 48, 25, 1, 16);
        cfg.keyint = 4;
        let mut e = enc::Encoder::new(cfg).unwrap();
        let packets: Vec<_> = (0..n)
            .map(|k| {
                let y = vec![(40 + 15 * k) as u8; 64 * 48];
                let c = vec![128u8; 32 * 24];
                let f = enc::Frame {
                    y: &y,
                    u: &c,
                    v: &c,
                    y_stride: 64,
                    c_stride: 32,
                };
                e.encode(&f, false)
            })
            .collect();
        let stream = enc::annex_b(&e.sps(), &e.pps(), &packets);
        dcm::fixture::video(
            dcm::fixture::H264_HIGH_41,
            &stream,
            1000,
            64,
            48,
            Some(n),
            33.0,
            8,
        )
    }

    #[test]
    fn h264_video_plays_and_steps() {
        let s = open("video.dcm", h264_dicom(9)).unwrap();
        assert_eq!(s.info().format, Format::Dicom);
        assert_eq!(s.colour_model(), Some("YCbCr"));
        assert!(s.info().coding.contains("H.264"), "{}", s.info().coding);
        assert_eq!(s.page_count(), 9);
        assert_eq!(s.animation().and_then(|a| a.frame_count), Some(9));
        assert_eq!(s.cine_timing().unwrap()[0], Duration::from_millis(33));
        let c = CancelToken::new();
        let grey = |r: &Raster| r.planes[0].data[(24 * 64 + 32) * 4];
        let still = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((still.width, still.height), (64, 48));
        let mut fr = s.frames(1).unwrap();
        let mut levels = Vec::new();
        while let Some(f) = fr.next_frame(&c) {
            let f = f.unwrap();
            assert_eq!(f.index as usize, levels.len());
            assert_eq!(f.delay, Duration::from_millis(33));
            levels.push(grey(&f.raster));
        }
        assert_eq!(levels.len(), 9);
        assert_eq!(levels[0], grey(&still));
        // Limited-range Y 40 + 15 k: RGB (Y - 16) * 255 / 219.
        for (k, &l) in levels.iter().enumerate() {
            let want = (f32::from(40 + 15 * k as u8) - 16.0) * 255.0 / 219.0;
            assert!(
                (f32::from(l) - want).abs() <= 3.0,
                "frame {k}: {l} vs {want}"
            );
        }
        // Stepping to any frame matches the cine frame.
        for k in [7u32, 2, 8, 0, 5] {
            let m = s.modality_frame_at(k, &c).unwrap().unwrap();
            let ([r, _, _], _) = m.rgb_at(32, 24).unwrap();
            assert_eq!(r as u8, levels[k as usize], "frame {k}");
        }
        fr.rewind().unwrap();
        assert_eq!(fr.next_frame(&c).unwrap().unwrap().index, 0);
        assert!(s.memory_bytes() > 0);
        let fresh = open("video.dcm", h264_dicom(3)).unwrap();
        crate::trim_check::assert_trims(&*fresh, &DecodeRequest::full(1));
    }

    #[test]
    fn trims() {
        let s = open("x.dcm", ct()).unwrap();
        crate::trim_check::assert_trims(&*s, &DecodeRequest::full(1));
    }
}
