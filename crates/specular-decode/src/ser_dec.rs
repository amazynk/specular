//! SER planetary and lunar video (`.ser`, "LUCAM-RECORDER") through `specular-ser`.
//!
//! Frames play in the animation player, timed by their UTC timestamps when the file has
//! them; [`Session::decode`] gives the first frame (thumbnails). Mono, RGB and BGR frames
//! and raw colour filter array frames show linear, white at the smallest `2^k − 1` (k ≥ 8,
//! at most the stored depth) above the first frame's brightest sample, so 12-bit data in
//! 16-bit samples is not shown dark. Bayer frames are debayered (bilinear) with each colour
//! linear up to its own bright point (white balanced), fixed from the first frame so the
//! video does not flicker; [`Session::set_debayer`] shows them raw or with another pattern.
//! CMY colour filter arrays show raw.
//!
//! The FEI / TIA `.ser` of electron microscopes is not this format and is not read.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rayon::prelude::*;
use specular_core::nd::Cfa;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_ser as ser;

use crate::science::{ScienceKind, ScienceSummary, VideoFacts};
use crate::whole::{self, Levels};
use crate::{
    AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo, Result,
    Session,
};

/// How long a frame shows when the file has no timestamps.
const DEFAULT_FRAME: Duration = Duration::from_millis(40);

pub struct SerDecoder;

impl Decoder for SerDecoder {
    fn format(&self) -> Format {
        Format::Ser
    }
    fn probe(&self, head: &[u8]) -> bool {
        ser::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ser"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(SerSession::new(source)?))
    }
}

fn map_err(e: ser::Error) -> DecodeError {
    match e {
        ser::Error::NotSer => DecodeError::UnsupportedFormat,
        ser::Error::Malformed(m) => DecodeError::Malformed(m),
        ser::Error::Unsupported(m) => DecodeError::Unsupported(m),
    }
}

/// How samples become display values; fixed per debayer setting from the first frame.
#[derive(Clone, Debug)]
struct Look {
    /// Debayer with this pattern (one-plane frames only).
    cfa: Option<Cfa>,
    /// Linear: display level of each stored value (mono, RGB, raw frames).
    lut: Arc<Vec<u8>>,
    /// Debayered: each colour's linear stretch ([`balance`]).
    stf: [Option<(f32, f32, f32)>; 3],
}

pub struct SerSession {
    source: Arc<Source>,
    ser: Arc<ser::Ser>,
    info: ImageInfo,
    anim: AnimationInfo,
    summary: ScienceSummary,
    delays: Arc<Vec<Duration>>,
    look: Mutex<Arc<Look>>,
    levels: Levels,
}

impl SerSession {
    fn new(source: Arc<Source>) -> Result<SerSession> {
        let s = ser::parse(source.bytes()).map_err(map_err)?;
        let colour = s.colour.name();
        let coding = format!("{}-bit {colour}", s.depth);
        let info = ImageInfo {
            format: Format::Ser,
            width: s.width,
            height: s.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: s.depth as u8,
            progressive: false,
            coding: coding.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let cfa = match s.colour {
            ser::Colour::Bayer(p) => Cfa::parse(p),
            _ => None,
        };
        let text = |t: &str| (!t.is_empty()).then(|| t.to_owned());
        let start = s.timestamps.first().copied().or(Some(s.date_utc));
        let summary = ScienceSummary {
            width: s.width,
            height: s.height,
            sample: coding,
            kind: ScienceKind::Astronomy,
            telescope: text(&s.telescope),
            instrument: text(&s.instrument),
            observer: text(&s.observer),
            cfa,
            video: Some(VideoFacts {
                frames: s.frames,
                declared: s.declared_frames,
                start: start.and_then(ser::ticks_text),
                end: s.timestamps.last().copied().and_then(ser::ticks_text),
                frame_rate: s.frame_rate(),
            }),
            ..Default::default()
        };
        let delays = Arc::new(delays(&s.timestamps, s.frames));
        let anim = AnimationInfo {
            frame_count: Some(s.frames),
            loop_count: 0,
        };
        let ser = Arc::new(s);
        let look = Mutex::new(Arc::new(look(&ser, source.bytes(), cfa)));
        Ok(SerSession {
            source,
            ser,
            info,
            anim,
            summary,
            delays,
            look,
            levels: Levels::new(),
        })
    }

    fn first(&self) -> Result<Raster> {
        let look = self.look.lock().clone();
        render(&self.ser, self.source.bytes(), 0, &look)
    }
}

/// Frame times from the timestamps: the gap to the next frame; frames without a sensible
/// gap (none, backwards, over ten seconds) take the median gap, or 40 ms.
fn delays(ts: &[i64], frames: u32) -> Vec<Duration> {
    let n = frames as usize;
    let gap = |i: usize| -> Option<i64> {
        let d = ts.get(i + 1)? - ts.get(i)?;
        (d > 0 && d <= 100_000_000).then_some(d)
    };
    let mut gaps: Vec<i64> = (0..n).filter_map(gap).collect();
    let fallback = if gaps.is_empty() {
        DEFAULT_FRAME
    } else {
        let m = gaps.len() / 2;
        Duration::from_nanos(*gaps.select_nth_unstable(m).1 as u64 * 100)
    };
    (0..n)
        .map(|i| gap(i).map_or(fallback, |d| Duration::from_nanos(d as u64 * 100)))
        .collect()
}

/// The display look of `ser` under `cfa`, from the first frame.
/// Each colour of a debayered frame linear from its darkest sample to its own bright point
/// (the 99.99th percentile, so a few hot pixels do not set it): white balanced on the
/// planet, Moon or Sun. Not the deep-sky automatic stretch, which on a black sky (median
/// and spread both zero) turns everything lit white.
fn balance(c: Cfa, f: &[f32], width: usize) -> [Option<(f32, f32, f32)>; 3] {
    [0, 1, 2].map(|ch| {
        let mut s = c.samples(f, width, ch, 1 << 20);
        if s.is_empty() {
            return None;
        }
        let lo = s.iter().copied().fold(f32::INFINITY, f32::min);
        let k = ((s.len() - 1) as f64 * 0.9999).round() as usize;
        let hi = *s.select_nth_unstable_by(k, f32::total_cmp).1;
        Some((lo, hi.max(lo + 1.0), 0.5))
    })
}

fn look(s: &ser::Ser, data: &[u8], cfa: Option<Cfa>) -> Look {
    let mut v = Vec::new();
    s.samples(data, 0, &mut v);
    let cfa = cfa.filter(|_| s.colour.planes() == 1);
    let stf = match cfa {
        Some(c) => {
            let f: Vec<f32> = v.iter().map(|&x| f32::from(x)).collect();
            balance(c, &f, s.width as usize)
        }
        None => [None; 3],
    };
    // White at the smallest 2^k − 1 (8 ≤ k ≤ depth) holding the brightest sample.
    let max = v.par_iter().copied().max().unwrap_or(0);
    let mut k = 8.min(s.depth);
    while k < s.depth && u32::from(max) > (1 << k) - 1 {
        k += 1;
    }
    let white = ((1u32 << k) - 1) as f32;
    let lut = (0..=u16::MAX)
        .map(|x| ((f32::from(x) / white).min(1.0) * 255.0 + 0.5) as u8)
        .collect();
    Look {
        cfa,
        lut: Arc::new(lut),
        stf,
    }
}

/// Frame `i` as shown under `look`.
fn render(s: &ser::Ser, data: &[u8], i: u32, look: &Look) -> Result<Raster> {
    let mut v = Vec::new();
    s.samples(data, i, &mut v)
        .ok_or_else(|| DecodeError::Malformed("frame out of range".into()))?;
    let (w, h) = (s.width as usize, s.height as usize);
    if let Some(cfa) = look.cfa {
        let f: Vec<f32> = v.par_iter().map(|&x| f32::from(x)).collect();
        let rgba = crate::science::cfa_rgba(&f, w, h, cfa, look.stf);
        return Ok(whole::interleaved(
            s.width,
            s.height,
            PixelLayout::Rgba,
            rgba,
            false,
        ));
    }
    let lut = &look.lut;
    if s.colour.planes() == 3 {
        let mut rgba = vec![255u8; w * h * 4];
        rgba.par_chunks_mut(w * 4)
            .zip(v.par_chunks(w * 3))
            .for_each(|(o, i)| {
                for (o, i) in o.chunks_exact_mut(4).zip(i.chunks_exact(3)) {
                    for c in 0..3 {
                        o[c] = lut[usize::from(i[c])];
                    }
                }
            });
        Ok(whole::interleaved(
            s.width,
            s.height,
            PixelLayout::Rgba,
            rgba,
            false,
        ))
    } else {
        let grey: Vec<u8> = v.par_iter().map(|&x| lut[usize::from(x)]).collect();
        Ok(whole::interleaved(
            s.width,
            s.height,
            PixelLayout::Gray,
            grey,
            false,
        ))
    }
}

impl Session for SerSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.first(),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        // Samples as u16 and f32, and the 8-bit frame.
        px * 6 * self.ser.colour.planes() as u64
            + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        (self.ser.frames > 1).then_some(&self.anim)
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        Ok(Box::new(SerFrames {
            source: self.source.clone(),
            ser: self.ser.clone(),
            look: self.look.lock().clone(),
            delays: self.delays.clone(),
            next: 0,
            scale: scale.max(1),
        }))
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(match self.ser.colour {
            ser::Colour::Mono => "Greyscale",
            ser::Colour::Bayer(_) => "Bayer",
            ser::Colour::Cmy(_) => "CFA",
            ser::Colour::Rgb | ser::Colour::Bgr => "RGB",
        })
    }
    fn debayer(&self) -> Option<Cfa> {
        self.look.lock().cfa
    }
    fn can_debayer(&self) -> bool {
        self.ser.colour.planes() == 1
    }
    fn set_debayer(&self, cfa: Option<Cfa>) {
        if !self.can_debayer() || self.look.lock().cfa == cfa {
            return;
        }
        let l = look(&self.ser, self.source.bytes(), cfa);
        *self.look.lock() = Arc::new(l);
        self.levels.clear();
    }
}

struct SerFrames {
    source: Arc<Source>,
    ser: Arc<ser::Ser>,
    look: Arc<Look>,
    delays: Arc<Vec<Duration>>,
    next: u32,
    scale: u32,
}

impl Frames for SerFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.next >= self.ser.frames {
            return None;
        }
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let index = self.next;
        self.next += 1;
        let mut raster = match render(&self.ser, self.source.bytes(), index, &self.look) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };
        if self.scale > 1 {
            raster = whole::downscale(&raster, self.ser.width, self.ser.height, self.scale);
        }
        Some(Ok(Frame {
            index,
            raster,
            delay: self
                .delays
                .get(index as usize)
                .copied()
                .unwrap_or(DEFAULT_FRAME),
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(bytes: Vec<u8>) -> Arc<dyn Session> {
        SerDecoder
            .open(Arc::new(Source::from_vec("t.ser", bytes)))
            .expect("opens")
    }

    fn header(colour_id: i32, w: u32, h: u32, depth: u32) -> ser::Header {
        ser::Header {
            colour_id,
            width: w,
            height: h,
            depth,
            ..Default::default()
        }
    }

    #[test]
    fn mono_frames_play_with_their_timestamps() {
        let frames: Vec<Vec<u8>> = (0..3u8).map(|k| vec![k * 100; 4 * 2]).collect();
        let t0 = 638_501_941_845_000_000i64;
        let ts = [t0, t0 + 100_000, t0 + 300_000];
        let s = open(ser::write(&header(0, 4, 2, 8), &frames, Some(&ts)));
        assert_eq!(s.info().format, Format::Ser);
        assert_eq!(s.animation().unwrap().frame_count, Some(3));
        assert_eq!(s.colour_model(), Some("Greyscale"));
        let v = s.science().unwrap().video.clone().unwrap();
        assert_eq!(v.start.as_deref(), Some("2024-05-01 21:03:04.500"));
        assert_eq!(v.end.as_deref(), Some("2024-05-01 21:03:04.530"));
        let mut f = s.frames(1).unwrap();
        let cancel = CancelToken::new();
        let a = f.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(a.delay, Duration::from_millis(10));
        let b = f.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(b.delay, Duration::from_millis(20));
        assert_eq!(b.raster.planes[0].data, vec![100; 8]);
        // The last frame takes the median gap.
        let c = f.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(c.delay, Duration::from_millis(20));
        assert!(f.next_frame(&cancel).is_none());
        f.rewind().unwrap();
        assert_eq!(f.next_frame(&cancel).unwrap().unwrap().index, 0);
        // The first frame is the still image.
        let r = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        assert_eq!(r.planes[0].data, vec![0; 8]);
    }

    /// 12-bit samples in 16 bits show white at 4095, not 65535.
    #[test]
    fn twelve_bit_data_is_not_dark() {
        let vals: Vec<u16> = (0..16).map(|i| i * 273).collect();
        let bytes: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
        let s = open(ser::write(&header(0, 4, 4, 16), &[bytes], None));
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r.planes[0].data[15], 255);
        assert_eq!(r.planes[0].data[0], 0);
        assert!(s.animation().is_none());
    }

    #[test]
    fn bgr_frames_come_out_red_first() {
        let s = open(ser::write(
            &header(101, 1, 1, 8),
            &[vec![10, 20, 255], vec![0, 0, 0]],
            None,
        ));
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r.layout, PixelLayout::Rgba);
        assert_eq!(&r.planes[0].data[..3], &[255, 20, 10]);
    }

    /// A flat-field Bayer frame (each colour constant) debayers to its colours; the
    /// toggle shows it raw.
    #[test]
    fn bayer_frames_debayer_and_toggle() {
        let (w, h) = (8u32, 6u32);
        // RGGB with red bright, green mid, blue dark, plus a little noise per colour.
        let frame: Vec<u8> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let base = [200u8, 120, 120, 40][(x & 1) as usize + 2 * (y & 1) as usize];
                base + (i % 3) as u8
            })
            .collect();
        let s = open(ser::write(
            &header(8, w, h, 8),
            &[frame.clone(), frame],
            None,
        ));
        assert_eq!(s.colour_model(), Some("Bayer"));
        assert_eq!(s.science().unwrap().cfa, Some(Cfa::RGGB));
        assert_eq!(s.debayer(), Some(Cfa::RGGB));
        let cancel = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        assert_eq!(r.layout, PixelLayout::Rgba);
        s.set_debayer(None);
        assert_eq!(s.debayer(), None);
        let r = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        assert_eq!(r.layout, PixelLayout::Gray);
        assert_eq!(r.planes[0].data[0], 200);
        // Frames follow the setting they start with.
        s.set_debayer(Some(Cfa::BGGR));
        let f = s.frames(1).unwrap().next_frame(&cancel).unwrap().unwrap();
        assert_eq!(f.raster.layout, PixelLayout::Rgba);
    }
}
