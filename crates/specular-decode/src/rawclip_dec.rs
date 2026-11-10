//! Raw video clips through the in-tree `specular-raw` reader: Canon Cinema RAW Light
//! (`.crm`, CRX frames) and Apple ProRes RAW (QuickTime with an `aprn` or `aprh` track, the
//! frames through `specular-prores`).
//!
//! A clip is a movie of sensor frames. The session shows its first frame the way a camera
//! RAW shows (a [`crate::raw_dec`] session: the embedded thumbnail first, then the developed
//! sensor, regions and levels); [`Session::frames`] develops each frame in turn at the
//! player's scale, so the clip plays, steps and seeks in the movie player (every frame is a
//! sync frame). Every frame is developed with the first frame's white balance and levels,
//! so the picture does not flicker. The clip's facts are in [`Session::video`] (frames,
//! rate, length, sound) and [`Session::header`] (the clip's own fields); the camera's in
//! [`Session::camera`]. Sound is not played.
//!
//! Develop opens the first frame (its sensor data is what `specular_raw::parse` reads).

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use specular_core::{CancelToken, Raster, Source};
use specular_raw as sr;

use crate::video_dec::VideoFacts;
use crate::{
    AnimationInfo, CameraInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames,
    HeaderFacts, ImageInfo, Result, Session,
};

/// How long a frame shows when the clip has no frame rate.
const DEFAULT_FRAME: Duration = Duration::from_millis(40);

/// Which raw video format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Canon Cinema RAW Light (`.crm`).
    CinemaRawLight,
    /// Apple ProRes RAW and ProRes RAW HQ (QuickTime).
    ProResRaw,
}

impl Kind {
    fn raw_format(self) -> sr::RawFormat {
        match self {
            Kind::CinemaRawLight => sr::RawFormat::Crm,
            Kind::ProResRaw => sr::RawFormat::ProResRaw,
        }
    }
}

pub struct RawClipDecoder(pub Kind);

impl Decoder for RawClipDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::CinemaRawLight => Format::CinemaRawLight,
            Kind::ProResRaw => Format::ProResRaw,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        sr::detect(head) == Some(self.0.raw_format())
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        sr::detect(data) == Some(self.0.raw_format())
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::CinemaRawLight => &["crm"],
            // A QuickTime movie: told from other movies by its track's sample entry.
            Kind::ProResRaw => &["mov"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        if sr::detect(data) != Some(self.0.raw_format()) {
            return Err(DecodeError::Malformed(match self.0 {
                Kind::CinemaRawLight => "not a Cinema RAW Light clip".into(),
                Kind::ProResRaw => "not a ProRes RAW movie".into(),
            }));
        }
        let file = sr::parse(data).map_err(crate::raw_dec::map_err)?;
        let Some(clip) = file.info().clip.clone() else {
            return Err(DecodeError::Malformed("raw video without frames".into()));
        };
        let codec = clip.codec.clone();
        if clip.frames.is_empty() {
            return Err(DecodeError::Unsupported(format!(
                "{codec}: none of the clip's {} frames is in the file (it was cut short)",
                clip.listed
            )));
        }
        if let Some(s) = &file.info().sensor
            && !s.decodable
        {
            let why = s
                .unsupported_reason
                .unwrap_or("this frame layout is not supported");
            return Err(DecodeError::Unsupported(why.into()));
        }
        let first = crate::raw_dec::open_parsed(source.clone(), file.clone())?;
        let mut info = first.info().clone();
        info.format = self.format();
        info.coding = coding(&clip, &info.coding);
        let delay = clip
            .frame_rate
            .filter(|r| r.is_finite() && *r > 0.0 && *r < 1000.0)
            .map_or(DEFAULT_FRAME, |r| Duration::from_secs_f64(1.0 / r));
        let n = clip.frames.len() as u32;
        let video = VideoFacts {
            duration_us: Some(u64::from(n) * delay.as_micros() as u64),
            codec: codec.clone(),
            frame_rate: clip.frame_rate,
            frames: n,
            audio: clip.audio.clone(),
            // The sound tracks are not played (raw clips record PCM beside the frames).
            silent_reason: clip.audio.as_ref().map(|_| "PCM".to_string()),
            make: Some(file.info().make.clone()).filter(|m| !m.is_empty()),
            model: Some(file.info().model.clone()).filter(|m| !m.is_empty()),
            date: file.info().date_time.clone(),
            ..Default::default()
        };
        let header = header_facts(&clip, file.info());
        Ok(Arc::new(ClipSession {
            first,
            file,
            info,
            anim: AnimationInfo {
                frame_count: Some(n),
                loop_count: 0,
            },
            video,
            header,
            delay,
            wb: Arc::new(OnceLock::new()),
        }))
    }
}

/// The info line: the clip's coding, frame count and the first frame's sensor coding.
fn coding(clip: &sr::Clip, frame: &str) -> String {
    let mut s = format!("{}, {} frames", clip.codec, clip.frames.len());
    if clip.listed as usize > clip.frames.len() {
        s += &format!(" of {} (the file is cut short)", clip.listed);
    }
    if let Some(r) = clip.frame_rate {
        s += &format!(", {r:.3} fps");
    }
    // The frame's own coding without the format name the clip already gave.
    if let Some((_, rest)) = frame.split_once(", ") {
        s += ", ";
        s += rest;
    }
    s
}

fn header_facts(clip: &sr::Clip, ri: &sr::RawInfo) -> HeaderFacts {
    let mut summary = Vec::new();
    let camera = ri.camera_name();
    if !camera.is_empty() {
        summary.push(("device", camera));
    }
    if let Some(r) = clip.frame_rate {
        summary.push(("frame-rate", format!("{r:.3}")));
    }
    summary.push(("compression", clip.codec.clone()));
    let mut fields = vec![
        ("Frames".to_string(), clip.frames.len().to_string()),
        ("Frames listed".to_string(), clip.listed.to_string()),
    ];
    if let Some(a) = &clip.audio {
        fields.push(("Sound".into(), a.clone()));
    }
    for (k, v) in &clip.facts {
        fields.push(((*k).to_string(), v.clone()));
    }
    if let Some(s) = &ri.software {
        fields.push(("Recorded by".into(), s.clone()));
    }
    HeaderFacts { summary, fields }
}

/// A clip: its first frame's session, with the movie's frames and facts.
struct ClipSession {
    first: Arc<dyn Session>,
    file: sr::RawFile,
    info: ImageInfo,
    anim: AnimationInfo,
    video: VideoFacts,
    header: HeaderFacts,
    delay: Duration,
    /// The white balance every frame plays with: the clip's, or (a clip that states none)
    /// the first frame's grey-world estimate, made once.
    wb: Arc<OnceLock<Option<[f32; 3]>>>,
}

impl Session for ClipSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        self.first.source()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.first.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.first.decode(req, cancel)
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        self.first.prepare_regions(cancel)
    }
    fn regions_ready(&self) -> bool {
        self.first.regions_ready()
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.first.decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.first.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.first.trim_memory()
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.first.colour_model()
    }
    fn camera(&self) -> Option<&CameraInfo> {
        self.first.camera()
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.header)
    }
    fn video(&self) -> Option<&VideoFacts> {
        Some(&self.video)
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        (self.video.frames > 1).then_some(&self.anim)
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        Ok(Box::new(ClipFrames {
            source: self.first.source().clone(),
            file: self.file.clone(),
            count: self.video.frames,
            next: 0,
            scale,
            delay: self.delay,
            wb: self.wb.clone(),
        }))
    }
}

/// Develops a clip's frames one after another.
struct ClipFrames {
    source: Arc<Source>,
    file: sr::RawFile,
    count: u32,
    next: u32,
    scale: u32,
    delay: Duration,
    wb: Arc<OnceLock<Option<[f32; 3]>>>,
}

/// The first frame's grey-world white balance (from a half-size decode), for a clip that
/// states none; `None` when it does state one.
fn estimated_wb(file: &sr::RawFile, data: &[u8]) -> Option<[f32; 3]> {
    let opts = sr::DecodeOptions {
        size: sr::OutputSize::Half,
        ..Default::default()
    };
    let raw = file.decode_raw(data, &opts).ok()?;
    if raw.wb.is_some() {
        return None;
    }
    sr::DevelopStats::of(&raw).ok().map(|s| s.wb)
}

impl ClipFrames {
    fn develop(&self, index: u32, cancel: &CancelToken) -> Result<Raster> {
        let wb = *self
            .wb
            .get_or_init(|| estimated_wb(&self.file, self.source.bytes()));
        let f = self
            .file
            .frame(index as usize)
            .ok_or_else(|| DecodeError::Malformed("frame out of range".into()))?;
        let f = match wb {
            Some(wb) => f.with_white_balance(wb),
            None => f,
        };
        let s = crate::raw_dec::open_parsed(self.source.clone(), f)?;
        s.decode(&DecodeRequest::full(self.scale), cancel)
    }
}

impl Frames for ClipFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.next >= self.count {
            return None;
        }
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let index = self.next;
        let r = self.develop(index, cancel);
        if matches!(r, Err(DecodeError::Cancelled)) {
            // The same frame again on the next call.
            return Some(Err(DecodeError::Cancelled));
        }
        self.next += 1;
        Some(r.map(|raster| Frame {
            index,
            raster,
            delay: self.delay,
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        Ok(())
    }
    fn seek(&mut self, index: u32) -> Result<bool> {
        self.next = index.min(self.count);
        Ok(true)
    }
    fn delays(&self) -> Option<Vec<Duration>> {
        Some(vec![self.delay; self.count as usize])
    }
    fn sync_frames(&self) -> Option<Vec<u32>> {
        Some((0..self.count).collect())
    }
}
