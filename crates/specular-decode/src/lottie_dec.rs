//! Lottie animations (`specular-lottie`): Bodymovin JSON and `.lottie` packages, played
//! as animations, each frame drawn from the layer tree. A `.json` file opens here only when
//! its content is a Lottie animation (the extension is not claimed: other JSON stays text,
//! and "Open as…" text still shows the source).

use std::sync::Arc;
use std::time::Duration;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_lottie::Animation;

use crate::whole::{self, Levels};
use crate::{
    AnimationInfo, Confidence, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames,
    HeaderFacts, ImageInfo, Result, Session,
};

pub struct LottieDecoder;

fn map_err(e: specular_lottie::Error) -> DecodeError {
    match e {
        specular_lottie::Error::Unsupported(s) => DecodeError::Unsupported(s),
        e => DecodeError::Malformed(e.to_string()),
    }
}

impl Decoder for LottieDecoder {
    fn format(&self) -> Format {
        Format::Lottie
    }
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_lottie::is_lottie_json(data) || specular_lottie::is_dotlottie(data)
    }
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        self.probe_file(data).then_some(Confidence::Certain)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["lottie"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let images: specular_drawpage::ImageDecoder =
            Arc::new(|d: &[u8]| crate::svg::decode_rgba(d));
        let d = source.bytes();
        let anim = if d.starts_with(b"PK") {
            specular_lottie::open_dotlottie(d, Some(&images))
        } else {
            specular_lottie::open_json(d, Some(&images), &|_| None)
        }
        .map_err(map_err)?;
        let anim = Arc::new(anim);
        let (w, h) = size(&anim);
        let frames = anim.frames();
        let mut fields = vec![
            ("Frames".to_owned(), frames.to_string()),
            ("Frame rate".to_owned(), format!("{} fps", anim.frame_rate)),
            (
                "Duration".to_owned(),
                format!("{:.2} s", f64::from(frames) / anim.frame_rate),
            ),
        ];
        if !anim.version.is_empty() {
            fields.push(("Bodymovin version".into(), anim.version.clone()));
        }
        if !anim.name.is_empty() {
            fields.push(("Name".into(), anim.name.clone()));
        }
        if !anim.not_drawn.is_empty() {
            fields.push(("Not drawn".into(), anim.not_drawn.join(", ")));
        }
        let info = ImageInfo {
            format: Format::Lottie,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding: format!(
                "Lottie animation, {frames} frames at {} fps{}",
                anim.frame_rate,
                if d.starts_with(b"PK") {
                    ", .lottie package"
                } else {
                    ""
                }
            ),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(LottieSession {
            anim,
            source,
            info,
            header: HeaderFacts {
                summary: Vec::new(),
                fields,
            },
            animation: AnimationInfo {
                frame_count: Some(frames),
                loop_count: 0,
            },
            levels: Levels::new(),
        }))
    }
}

/// The canvas in whole pixels.
fn size(a: &Animation) -> (u32, u32) {
    (
        a.width.round().max(1.0) as u32,
        a.height.round().max(1.0) as u32,
    )
}

/// Frame `k` drawn at 1/`scale` of the canvas.
fn draw(a: &Animation, k: u32, scale: u32, cancel: &CancelToken) -> Result<Raster> {
    let (w, h) = size(a);
    let s = scale.max(1);
    let (rw, rh) = (w.div_ceil(s), h.div_ceil(s));
    let page = specular_lottie::frame_page(a, a.in_point + f64::from(k));
    let req = specular_drawpage::RenderRequest {
        scale_x: f64::from(rw) / a.width,
        scale_y: f64::from(rh) / a.height,
        x: 0,
        y: 0,
        width: rw,
        height: rh,
    };
    let px =
        specular_drawpage::render(&page, req, &|| cancel.is_cancelled()).map_err(|e| match e {
            specular_drawpage::RenderError::Cancelled => DecodeError::Cancelled,
            e => DecodeError::Malformed(e.to_string()),
        })?;
    Ok(whole::interleaved(rw, rh, PixelLayout::Rgba, px, false))
}

struct LottieSession {
    anim: Arc<Animation>,
    source: Arc<Source>,
    info: ImageInfo,
    header: HeaderFacts,
    animation: AnimationInfo,
    levels: Levels,
}

impl Session for LottieSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.header)
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    /// The still picture: the first frame.
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| draw(&self.anim, 0, 1, cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        whole::level_bytes(w, h, req.scale.max(1), 4) + whole::level_bytes(w, h, 1, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        Some(&self.animation)
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        Ok(Box::new(LottieFrames {
            anim: self.anim.clone(),
            scale: scale.max(1),
            next: 0,
        }))
    }
}

struct LottieFrames {
    anim: Arc<Animation>,
    scale: u32,
    next: u32,
}

impl Frames for LottieFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.next >= self.anim.frames() {
            return None;
        }
        let k = self.next;
        self.next += 1;
        Some(draw(&self.anim, k, self.scale, cancel).map(|raster| Frame {
            index: k,
            raster,
            delay: Duration::from_secs_f64(self.anim.frame_seconds()),
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        Ok(())
    }
    /// Every frame stands alone: any one is drawn directly.
    fn seek(&mut self, index: u32) -> Result<bool> {
        self.next = index.min(self.anim.frames());
        Ok(true)
    }
    fn delays(&self) -> Option<Vec<Duration>> {
        Some(vec![
            Duration::from_secs_f64(self.anim.frame_seconds());
            self.anim.frames() as usize
        ])
    }
    fn sync_frames(&self) -> Option<Vec<u32>> {
        Some((0..self.anim.frames()).collect())
    }
}
