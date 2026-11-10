//! The export's worker: the show rendered offscreen frame by frame, encoded and written.
//!
//! Three threads in a pipeline:
//! - **decode**: opens the slides in order, decodes each at the size the video needs (an
//!   animation's first frame, its later frames follow on the render thread) and its caption;
//! - **render** (the worker itself): the live show's drawing on its own compositor at the
//!   video's size — the image renderer over the full-screen canvas, Ken Burns through
//!   [`ken_burns_target`], the cross-fade through [`Crossfade`], the caption through
//!   [`draw_overlay`] — read back as 8-bit sRGB and converted to BT.709 limited-range 4:2:0;
//!   a frame whose picture did not change is not drawn again;
//! - **encode**: the GPU's hardware encoder or [`specular_h264enc`], as the path selector
//!   chooses ([`VideoEncoder`]: an error mid-export continues on the CPU), and the MP4 writer
//!   into a temporary file next to the destination, renamed over it (or linked without
//!   replacing) when done.
//!
//! Frame `n` is at `n / fps` seconds whatever the machine's speed; shuffle and Ken Burns
//! follow the seed fixed when the export started, so two exports of a show are the same file.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use specular_anim::DelayPolicy;
use specular_core::{CancelToken, Orientation, Raster};
use specular_decode::{DecodeRequest, Frames, Registry};
use specular_gfx::{Compositor, Crossfade, DrawList, Gpu, Text, UiRenderer, wgpu};
use specular_h264enc::{Config, MotionHint, Profile};
use specular_render::icc::SourceLut;
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_settings::{SlideshowCaption, SlideshowTransition};
use specular_video::encode::{Cue, VideoEncoder};
use specular_video::mp4::Colour;
use specular_video::mp4write::{Mp4Writer, Track};

use super::super::slideshow::{ANIM_CAP_S, caption_lines, draw_overlay, due, ken_burns_target};
use super::timeline::{SlideClock, frames_for, ken_burns_seed, slide_seconds};

/// The final pass's output: 8-bit, sRGB-encoded by the shader.
const OUT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Ken Burns zooms in by up to this much past fit: slides are decoded with these pixels.
const KB_HEADROOM: f32 = 1.1;

/// Everything the worker needs, fixed when the export starts.
#[derive(Clone, Debug)]
pub(crate) struct Job {
    /// `(listing index, path)` in show order.
    pub slides: Vec<(usize, PathBuf)>,
    /// The run's seed (Ken Burns directions).
    pub seed: u64,
    pub size: (u32, u32),
    pub fps: u32,
    pub qp: u8,
    /// High (smallest) or Constrained Baseline (plays on old players).
    pub profile: Profile,
    /// Seconds a still is on.
    pub interval: f64,
    pub transition: SlideshowTransition,
    pub caption: SlideshowCaption,
    pub delays: DelayPolicy,
    /// Small images are enlarged to fit (the viewer's initial zoom is Fit).
    pub upscale_small: bool,
    /// The scene's options: the full-screen canvas and what shows through transparency.
    pub opts: RenderOptions,
    /// Physical pixels per logical one (the caption's size).
    pub ui_scale: f32,
    pub text_scale: f32,
    pub rtl: bool,
    pub lang: specular_i18n::LanguageIdentifier,
    pub dest: PathBuf,
    /// The user agreed to replace `dest`.
    pub replace: bool,
}

/// Shared with the dialog: progress and the cancel flag.
#[derive(Debug, Default)]
pub(crate) struct Progress {
    pub cancel: AtomicBool,
    /// Frames encoded.
    pub frames: AtomicU64,
    /// Frames the show is expected to have (stills exact; animations at the interval).
    pub planned: AtomicU64,
}

impl Progress {
    /// 0–1.
    pub fn fraction(&self) -> f32 {
        let p = self.planned.load(Ordering::Relaxed);
        if p == 0 {
            return 0.0;
        }
        (self.frames.load(Ordering::Relaxed) as f32 / p as f32).min(0.99)
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// How the export ended.
#[derive(Debug)]
pub(crate) enum Outcome {
    Done(Done),
    Cancelled,
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Done {
    pub path: PathBuf,
    pub bytes: u64,
    pub frames: u64,
    pub seconds: f64,
    /// Files that could not be read (left out).
    pub skipped: usize,
    /// Wall time of the export.
    pub elapsed: Duration,
}

/// Starts the export on its own thread; the outcome arrives on the receiver (then `wake`).
/// While it runs, `wake` is also called a few times a second for the dialog's progress.
pub(crate) fn spawn(
    gpu: Gpu,
    registry: Arc<Registry>,
    job: Job,
    progress: Arc<Progress>,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> std::io::Result<crossbeam_channel::Receiver<Outcome>> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    specular_thread::builder("vx-render").spawn(move || {
        let text = Text::new();
        let finished = AtomicBool::new(false);
        let out = std::thread::scope(|s| {
            specular_thread::spawn_scoped(s, "vx-tick", || {
                while !finished.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(250));
                    wake();
                }
            });
            // A panic (a decoder bug on a slide) fails the export instead of leaving vx-tick
            // waking the dialog forever with no outcome.
            let out = specular_core::isolate::run("video export", || {
                run(gpu, &registry, &job, &progress, text)
            })
            .unwrap_or_else(|_| Outcome::Failed(specular_i18n::t!("error-reason-internal")));
            finished.store(true, Ordering::Relaxed);
            out
        });
        let _ = tx.send(out);
        wake();
    })?;
    Ok(rx)
}

/// The export from start to finish (also the tests' entry point, with bundled fonts).
pub(crate) fn run(
    gpu: Gpu,
    registry: &Registry,
    job: &Job,
    progress: &Progress,
    mut text: Text,
) -> Outcome {
    let t0 = Instant::now();
    text.set_locale(Some(job.lang.to_string()));
    let tmp = temp_path(&job.dest);
    let r = export(gpu, registry, job, progress, text, &tmp);
    let out = match r {
        Ok(Some((frames, skipped))) => match finalize(&tmp, &job.dest, job.replace) {
            Ok(bytes) => Outcome::Done(Done {
                path: job.dest.clone(),
                bytes,
                frames,
                seconds: frames as f64 / f64::from(job.fps),
                skipped,
                elapsed: t0.elapsed(),
            }),
            Err(e) => Outcome::Failed(crate::error_text::reason(&e)),
        },
        Ok(None) => Outcome::Cancelled,
        Err(e) => Outcome::Failed(e),
    };
    if !matches!(out, Outcome::Done(_)) {
        let _ = std::fs::remove_file(&tmp);
    }
    if let Outcome::Done(d) = &out {
        let secs = d.elapsed.as_secs_f64().max(1e-3);
        specular_log::info!(
            "video export: {} frames ({:.1} s) {}×{} at {} fps in {secs:.1} s ({:.1} frames/s), {} bytes",
            d.frames,
            d.seconds,
            job.size.0,
            job.size.1,
            job.fps,
            d.frames as f64 / secs,
            d.bytes
        );
    }
    out
}

/// `.name.specular-<pid>.tmp` next to `dest`.
pub(crate) fn temp_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export.mp4".into());
    dest.with_file_name(format!(".{name}.specular-{}.tmp", std::process::id()))
}

/// Moves the finished temporary file to `dest`: over it when replacing was agreed, else only
/// when nothing appeared there meanwhile (a hard link fails on an existing name). Returns
/// the file's size.
pub(crate) fn finalize(tmp: &Path, dest: &Path, replace: bool) -> std::io::Result<u64> {
    let bytes = std::fs::metadata(tmp)?.len();
    if replace {
        std::fs::rename(tmp, dest)?;
    } else {
        match std::fs::hard_link(tmp, dest) {
            Ok(()) => {
                let _ = std::fs::remove_file(tmp);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(e),
            // File systems without hard links: check, then rename (a small race).
            Err(_) if !dest.exists() => std::fs::rename(tmp, dest)?,
            Err(e) => return Err(e),
        }
    }
    Ok(bytes)
}

/// A slide decoded for the video.
struct Slide {
    listing: usize,
    stored: (u32, u32),
    orientation: Orientation,
    lut: Option<SourceLut>,
    first: Raster,
    /// An animation's first delay and its later frames.
    anim: Option<(Duration, Box<dyn Frames>)>,
    caption: Vec<String>,
}

/// What the probe found about a file.
struct Probe {
    animated: bool,
}

fn probe(registry: &Registry, path: &Path) -> Option<Probe> {
    let s = registry.open_path(path).ok()?;
    Some(Probe {
        animated: s.animation().is_some(),
    })
}

fn decode(registry: &Registry, job: &Job, listing: usize, path: &Path) -> Result<Slide, String> {
    let session = registry
        .open_path(path)
        .map_err(|e| crate::error_text::reason(&e))?;
    let info = session.info().clone();
    let (dw, dh) = info.orientation.display_size(info.width, info.height);
    // The display long side the video needs: the image fit to the frame, plus Ken Burns.
    let fit = (job.size.0 as f32 / dw.max(1) as f32).min(job.size.1 as f32 / dh.max(1) as f32);
    let need = (dw.max(dh) as f32 * fit * KB_HEADROOM).ceil() as u32;
    let scale =
        specular_print::files::pick_scale(info.scales, info.width.max(info.height), need.max(16));
    let lut = info
        .icc_profile
        .as_ref()
        .and_then(|icc| specular_render::icc::build_lut(icc));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let caption = match job.caption {
        SlideshowCaption::NameExif => {
            let loc = crate::loader::meta_localizer(job.lang.clone());
            let (report, _) = crate::loader::metadata_report(session.as_ref(), &loc);
            caption_lines(&name, Some(&report), job.caption)
        }
        c => caption_lines(&name, None, c),
    };
    let cancel = CancelToken::new();
    let (first, anim) = match session.animation().map(|_| session.frames(scale)) {
        Some(Ok(mut frames)) => match frames.next_frame(&cancel) {
            Some(Ok(f)) => (f.raster, Some((f.delay, frames))),
            Some(Err(e)) => return Err(crate::error_text::reason(&e)),
            None => return Err("no frames".into()),
        },
        _ => (
            session
                .decode(&DecodeRequest::full(scale), &cancel)
                .map_err(|e| crate::error_text::reason(&e))?,
            None,
        ),
    };
    Ok(Slide {
        listing,
        stored: (info.width, info.height),
        orientation: info.orientation,
        lut,
        first,
        anim,
        caption,
    })
}

/// A picture for the encoder: Y, then U, then V (tightly packed).
struct Yuv {
    w: usize,
    h: usize,
    data: Vec<u8>,
}

impl Yuv {
    fn planes(&self) -> (&[u8], &[u8], &[u8]) {
        let ys = self.w * self.h;
        let cs = (self.w / 2) * (self.h / 2);
        let (y, rest) = self.data.split_at(ys);
        let (u, v) = rest.split_at(cs);
        (y, u, &v[..cs])
    }
}

/// 8-bit R'G'B' rows (`pitch` bytes apart, RGBA) to BT.709 limited-range 4:2:0; chroma from
/// the mean of each 2×2 block.
fn to_yuv(rgba: &[u8], pitch: usize, w: usize, h: usize) -> Yuv {
    // Fixed point, 16 fractional bits: Y = 16 + 219/255 (0.2126 R + 0.7152 G + 0.0722 B),
    // Cb = 128 + 224/255 (B − Y')/1.8556, Cr = 128 + 224/255 (R − Y')/1.5748.
    const KY: [i32; 3] = [11_966, 40_254, 4_064];
    const KB: [i32; 3] = [-6_596, -22_189, 28_784];
    const KR: [i32; 3] = [28_784, -26_145, -2_639];
    let ys = w * h;
    let (cw, ch) = (w / 2, h / 2);
    let mut data = vec![0u8; ys + 2 * cw * ch];
    let (yp, c) = data.split_at_mut(ys);
    let (up, vp) = c.split_at_mut(cw * ch);
    yp.par_chunks_mut(w)
        .zip(rgba.par_chunks(pitch))
        .for_each(|(dst, src)| {
            for (d, p) in dst.iter_mut().zip(src.chunks_exact(4)) {
                let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
                *d = ((KY[0] * r + KY[1] * g + KY[2] * b + (16 << 16) + (1 << 15)) >> 16) as u8;
            }
        });
    up.par_chunks_mut(cw)
        .zip(vp.par_chunks_mut(cw))
        .enumerate()
        .for_each(|(cy, (u, v))| {
            let r0 = &rgba[2 * cy * pitch..];
            let r1 = &rgba[(2 * cy + 1) * pitch..];
            for cx in 0..cw {
                let o = cx * 8;
                let sum = |k: usize| {
                    i32::from(r0[o + k])
                        + i32::from(r0[o + 4 + k])
                        + i32::from(r1[o + k])
                        + i32::from(r1[o + 4 + k])
                };
                let (r, g, b) = (sum(0), sum(1), sum(2));
                // Sums of four: two more fractional bits.
                let round = (128 << 18) + (1 << 17);
                u[cx] = ((KB[0] * r + KB[1] * g + KB[2] * b + round) >> 18).clamp(0, 255) as u8;
                v[cx] = ((KR[0] * r + KR[1] * g + KR[2] * b + round) >> 18).clamp(0, 255) as u8;
            }
        });
    Yuv { w, h, data }
}

/// The GPU side: the live show's renderers on a compositor of the video's size.
struct Stage {
    gpu: Gpu,
    images: ImageRenderer,
    comp: Compositor,
    ui: UiRenderer,
    fade: Crossfade,
    text: Text,
    out: wgpu::Texture,
    out_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    pitch: u32,
    size: (u32, u32),
}

impl Stage {
    fn new(gpu: Gpu, size: (u32, u32), text: Text) -> Stage {
        let images = ImageRenderer::new(&gpu, 64 << 20);
        // 16 bits: no dither noise (it would cost bits and defeat unchanged frames).
        let comp = Compositor::new(&gpu, OUT, 16, size.0, size.1);
        let ui = UiRenderer::new(&gpu, &comp);
        let fade = Crossfade::new(&gpu);
        let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video frame"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let out_view = out.create_view(&Default::default());
        let pitch = (size.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video readback"),
            size: u64::from(pitch) * u64::from(size.1),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Stage {
            gpu,
            images,
            comp,
            ui,
            fade,
            text,
            out,
            out_view,
            readback,
            pitch,
            size,
        }
    }

    fn wait_for_fonts(&self) {
        let t0 = Instant::now();
        while self.text.fonts_pending() && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// One frame: the scene (when `scene` is given: the image at its view; a snapshot of the
    /// previous scene first when `snapshot`), the fade of the outgoing slide, the caption.
    #[allow(clippy::too_many_arguments)]
    fn frame(
        &mut self,
        job: &Job,
        img: Option<&specular_render::GpuImage>,
        view: &View,
        snapshot: bool,
        fade: Option<f32>,
        caption: &[String],
    ) -> Result<Yuv, String> {
        let gpu = self.gpu.clone();
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        if snapshot {
            self.fade.snapshot(&gpu, &mut enc, &self.comp);
        }
        self.images.render(
            &gpu,
            &mut enc,
            &self.comp.scene.view,
            self.size,
            img,
            view,
            &job.opts,
        );
        self.comp.begin(&mut enc);
        if let Some(k) = fade {
            self.fade.draw(&mut enc, &self.comp, k);
        }
        let s = job.ui_scale;
        self.text.begin_frame(s);
        let mut dl = DrawList::new(s);
        let area = (0.0, 0.0, self.size.0 as f32 / s, self.size.1 as f32 / s);
        draw_overlay(
            &mut dl,
            &mut self.text,
            caption,
            false,
            area,
            job.rtl,
            job.text_scale,
        );
        self.ui.render(
            &gpu,
            &mut enc,
            &self.comp.comp.view,
            self.size,
            &dl,
            &mut self.text,
        );
        self.text.end_frame();
        self.comp.finish(&mut enc, &self.out_view);
        enc.copy_texture_to_buffer(
            self.out.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.pitch),
                    rows_per_image: Some(self.size.1),
                },
            },
            wgpu::Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([enc.finish()]);
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        gpu.wait().map_err(|e| format!("GPU readback: {e}"))?;
        let yuv = {
            let data = slice
                .get_mapped_range()
                .map_err(|e| format!("GPU readback: {e}"))?;
            to_yuv(
                &data,
                self.pitch as usize,
                self.size.0 as usize,
                self.size.1 as usize,
            )
        };
        self.readback.unmap();
        Ok(yuv)
    }
}

/// Where a view puts the image on the frame: `screen = display · scale + origin` (physical
/// px, continuous coordinates: sample `i` covers `i..i + 1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub scale: f32,
    pub origin: (f32, f32),
}

impl Placement {
    pub fn of(view: &View) -> Placement {
        Placement {
            scale: view.scale(),
            origin: view.display_to_screen((0.0, 0.0)),
        }
    }

    /// The encoder's motion hint for a frame placed as `self` after one placed as `prev`
    /// (same image): sample `(x, y)` of this frame shows what sample
    /// `(x·scale + dx, y·scale + dy)` of the previous one showed (sample indices, centres at
    /// `i + ½`).
    pub fn hint_after(&self, prev: &Placement) -> MotionHint {
        let k = prev.scale / self.scale;
        let d = |o: f32, p: f32| (0.5 - o) * k + p - 0.5;
        MotionHint {
            scale: k,
            dx: d(self.origin.0, prev.origin.0),
            dy: d(self.origin.1, prev.origin.1),
        }
    }
}

/// What the encoder thread gets.
struct Picture {
    /// Shared when repeated.
    yuv: Arc<Yuv>,
    /// It must be an IDR picture (a cut).
    idr: bool,
    /// The Ken Burns motion from the previous picture.
    hint: Option<MotionHint>,
    /// The cross-fade runs.
    fading: bool,
}

/// Renders, encodes and writes the show into `tmp`. `Ok(None)` when cancelled; otherwise
/// the frames written and the files left out.
fn export(
    gpu: Gpu,
    registry: &Registry,
    job: &Job,
    progress: &Progress,
    text: Text,
    tmp: &Path,
) -> Result<Option<(u64, usize)>, String> {
    let (w, h) = job.size;
    // Headers first: what can be read, and the frames to reserve the index for.
    let mut slides = Vec::new();
    let mut reserve: u64 = 0;
    let mut planned: u64 = 0;
    for (i, p) in &job.slides {
        if progress.cancelled() {
            return Ok(None);
        }
        let Some(pr) = probe(registry, p) else {
            continue;
        };
        let longest = if pr.animated {
            slide_seconds(job.interval, Some(f64::INFINITY))
        } else {
            job.interval
        };
        reserve += frames_for(longest, job.fps);
        planned += frames_for(job.interval, job.fps);
        slides.push((*i, p.clone()));
    }
    let mut skipped = job.slides.len() - slides.len();
    if slides.is_empty() {
        return Err(specular_i18n::t!("video-export-no-images"));
    }
    progress.planned.store(planned, Ordering::Relaxed);
    let mut cfg = Config::new(w, h, job.fps, 1, job.qp);
    cfg.keyint = job.fps * 5;
    cfg.profile = job.profile;
    let mut encoder = VideoEncoder::new(cfg)?;
    let track = Track {
        width: w,
        height: h,
        fps_num: job.fps,
        fps_den: 1,
        avcc: encoder.avcc(),
        colour: Colour {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: false,
        },
    };
    let file = File::create(tmp).map_err(|e| crate::error_text::reason(&e))?;
    let writer = Mp4Writer::new(
        BufWriter::with_capacity(1 << 20, file),
        track,
        reserve.min(u64::from(u32::MAX)) as u32,
    )
    .map_err(|e| crate::error_text::reason(&e))?;

    let skipped_decode = Mutex::new(0usize);
    let result = std::thread::scope(|s| -> Result<Option<u64>, String> {
        let (slide_tx, slide_rx) = sync_channel::<Slide>(1);
        let (pic_tx, pic_rx) = sync_channel::<Picture>(3);
        let slides = &slides;
        let skipped_decode = &skipped_decode;
        specular_thread::builder("vx-decode")
            .spawn_scoped(s, move || {
                for (i, p) in slides {
                    if progress.cancelled() {
                        break;
                    }
                    match decode(registry, job, *i, p) {
                        Ok(sl) => {
                            if slide_tx.send(sl).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            specular_log::warn!("video export: {}: {e}", p.display());
                            *skipped_decode.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                        }
                    }
                }
            })
            .map_err(|e| crate::error_text::reason(&e))?;
        let enc_thread = specular_thread::builder("vx-encode")
            .spawn_scoped(s, move || encode(&mut encoder, writer, pic_rx, progress))
            .map_err(|e| crate::error_text::reason(&e))?;
        let rendered = render(gpu, job, progress, text, slide_rx, pic_tx);
        let encoded = enc_thread
            .join()
            .map_err(|_| "the encoder stopped".to_owned())?;
        let rendered = rendered?;
        let (frames, mut w) = match encoded? {
            Some(x) => x,
            None => return Ok(None),
        };
        if rendered.is_none() || progress.cancelled() {
            return Ok(None);
        }
        w.flush().map_err(|e| crate::error_text::reason(&e))?;
        let f = w.into_inner().map_err(|e| crate::error_text::reason(&e))?;
        f.sync_all().map_err(|e| crate::error_text::reason(&e))?;
        Ok(Some(frames))
    })?;
    skipped += *skipped_decode.lock().unwrap_or_else(|e| e.into_inner());
    Ok(result.map(|f| (f, skipped)))
}

/// The encoder thread: pictures in, samples out; the writer when the pictures end.
fn encode(
    encoder: &mut VideoEncoder,
    mut writer: Mp4Writer<BufWriter<File>>,
    rx: Receiver<Picture>,
    progress: &Progress,
) -> Result<Option<(u64, BufWriter<File>)>, String> {
    let (mut n, mut last): (u64, Option<Arc<Yuv>>) = (0, None);
    for Picture {
        yuv: pic,
        idr,
        hint,
        fading,
    } in rx
    {
        if progress.cancelled() {
            return Ok(None);
        }
        let (y, u, v) = pic.planes();
        let p = encoder.encode_cued(
            &specular_h264enc::Frame {
                y,
                u,
                v,
                y_stride: pic.w,
                c_stride: pic.w / 2,
            },
            Cue {
                force_idr: idr,
                hint,
                fading,
            },
        );
        writer
            .write_sample(&p.data, p.key)
            .map_err(|e| crate::error_text::reason(&e))?;
        n += 1;
        progress.frames.store(n, Ordering::Relaxed);
        last = Some(pic);
    }
    drop(last);
    if n == 0 {
        return Ok(None);
    }
    // Both encoders' parameter sets when the hardware gave up mid-export.
    writer.set_avcc(encoder.avcc());
    let w = writer.finish().map_err(|e| crate::error_text::reason(&e))?;
    Ok(Some((n, w)))
}

/// An animation on its slide: the frame on screen and when the next one starts.
struct Playing {
    frames: Box<dyn Frames>,
    /// Start of the next frame (seconds into the slide).
    next: f64,
    /// The whole play-through's length, once the last frame was read.
    total: Option<f64>,
}

/// The render loop; `Ok(None)` when cancelled. Dropping `pics` ends the encoder.
fn render(
    gpu: Gpu,
    job: &Job,
    progress: &Progress,
    text: Text,
    slides: Receiver<Slide>,
    pics: SyncSender<Picture>,
) -> Result<Option<()>, String> {
    let mut st = Stage::new(gpu, job.size, text);
    st.wait_for_fonts();
    let cancel = CancelToken::new();
    let viewport = PxRect {
        x: 0.0,
        y: 0.0,
        w: job.size.0 as f32,
        h: job.size.1 as f32,
    };
    let mut id = 0u64;
    let mut first = true;
    for slide in slides {
        if progress.cancelled() {
            return Ok(None);
        }
        id += 1;
        let mut img = st.images.create_image(
            &st.gpu,
            id,
            slide.stored.0,
            slide.stored.1,
            slide.lut.as_ref(),
        );
        st.images.upload_overview(&st.gpu, &mut img, &slide.first);
        let mut view = View::new();
        view.upscale_small = job.upscale_small;
        view.set_viewport(viewport);
        view.set_image(slide.stored.0, slide.stored.1, slide.orientation);
        let mut playing = slide.anim.map(|(d, frames)| Playing {
            frames,
            next: job.delays.apply(d).as_secs_f64(),
            total: None,
        });
        let clock = SlideClock {
            fps: job.fps,
            interval: job.interval,
            transition: job.transition,
            after_another: !first,
        };
        let kb_seed = ken_burns_seed(job.seed, slide.listing);
        let mut last: Option<Arc<Yuv>> = None;
        // Where the last picture of this slide had the image.
        let mut placed: Option<Placement> = None;
        let mut k = 0u64;
        loop {
            if progress.cancelled() {
                return Ok(None);
            }
            let m = clock.moment(k);
            // A slide is on for its interval, an animation for one play-through (capped).
            if k > 0 {
                let played = playing
                    .as_ref()
                    .is_none_or(|p| p.total.is_some_and(|t| m.tau + 1e-9 >= t));
                let tau = m.tau + 1e-9;
                let animated = playing.is_some();
                if due(tau, job.interval, animated, played) || tau >= job.interval.max(ANIM_CAP_S) {
                    break;
                }
            }
            // The animation's frames up to now.
            let mut changed = k == 0;
            if let Some(p) = playing.as_mut() {
                while p.total.is_none() && p.next <= m.tau + 1e-9 {
                    match p.frames.next_frame(&cancel) {
                        Some(Ok(f)) => {
                            st.images.upload_overview(&st.gpu, &mut img, &f.raster);
                            p.next += job.delays.apply(f.delay).as_secs_f64();
                            changed = true;
                        }
                        Some(Err(e)) => {
                            specular_log::warn!("video export: animation frame: {e}");
                            p.total = Some(p.next);
                        }
                        None => p.total = Some(p.next),
                    }
                }
            }
            if let Some(pk) = m.ken_burns {
                let (s, c) = ken_burns_target(&view, pk, kb_seed);
                view.zoom_direct_by(s / view.scale(), None);
                view.center_on(c, false);
                changed = true;
            }
            changed |= m.fade.is_some();
            let mut hint = None;
            let pic = match (&last, changed) {
                (Some(l), false) => l.clone(),
                _ => {
                    let snapshot = k == 0 && m.fade.is_some();
                    let now = Placement::of(&view);
                    // The image moved since the last picture: the encoder searches there first.
                    // Not while cross-fading: the outgoing slide doesn't follow that motion,
                    // and the hint costs more than it saves there (measured).
                    hint = placed
                        .filter(|p| *p != now && m.fade.is_none())
                        .map(|p| now.hint_after(&p));
                    placed = Some(now);
                    Arc::new(st.frame(job, Some(&img), &view, snapshot, m.fade, &slide.caption)?)
                }
            };
            last = Some(pic.clone());
            let p = Picture {
                yuv: pic,
                idr: m.cut,
                hint,
                fading: m.fade.is_some(),
            };
            if pics.send(p).is_err() {
                // The encoder stopped (an error, or cancelled).
                return Ok(None);
            }
            k += 1;
        }
        st.images.release_image(id);
        first = false;
    }
    Ok(Some(()))
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    #[test]
    fn colours_convert_to_bt709_limited_range() {
        let (w, h) = (4usize, 2usize);
        let pitch = 32;
        let mut rgba = vec![0u8; pitch * h];
        // A black 2×2 block, then a white one.
        for y in 0..h {
            for x in 2..w {
                rgba[y * pitch + x * 4..][..3].copy_from_slice(&[255; 3]);
            }
        }
        let yuv = to_yuv(&rgba, pitch, w, h);
        let (y, u, v) = yuv.planes();
        assert_eq!(&y[..2], &[16, 16], "black");
        assert_eq!(&y[2..4], &[235, 235], "white");
        assert_eq!((u[0], v[0]), (128, 128), "black is neutral");
        assert_eq!((u[1], v[1]), (128, 128), "white is neutral");
        let mut red = vec![0u8; pitch * 2];
        let mut blue = vec![0u8; pitch * 2];
        for k in 0..2 {
            for x in 0..2 {
                red[k * pitch + x * 4] = 255;
                blue[k * pitch + x * 4 + 2] = 255;
            }
        }
        let r = to_yuv(&red, pitch, 2, 2);
        let b = to_yuv(&blue, pitch, 2, 2);
        // BT.709: red Y' 63, Cb 102, Cr 240; blue Y' 32, Cb 240, Cr 118.
        assert_eq!(r.planes().0[0], 63);
        assert_eq!((r.planes().1[0], r.planes().2[0]), (102, 240));
        assert_eq!(b.planes().0[0], 32);
        assert_eq!((b.planes().1[0], b.planes().2[0]), (240, 118));
    }

    #[test]
    fn ken_burns_hint_follows_the_image() {
        // Zooming in by 1 % and panning: every image point lands where the hint says.
        let prev = Placement {
            scale: 0.50,
            origin: (-120.0, -40.0),
        };
        let now = Placement {
            scale: 0.505,
            origin: (-131.5, -43.25),
        };
        let h = now.hint_after(&prev);
        for d in [(0.0, 0.0), (1000.0, 300.0), (3000.0, 2000.0)] {
            // The sample (index) showing image point `d` in each frame; centres at i + ½.
            let at = |p: &Placement| {
                (
                    d.0 * p.scale + p.origin.0 - 0.5,
                    d.1 * p.scale + p.origin.1 - 0.5,
                )
            };
            let (x, y) = at(&now);
            let (px, py) = at(&prev);
            assert!((x * h.scale + h.dx - px).abs() < 1e-2, "{d:?}: x");
            assert!((y * h.scale + h.dy - py).abs() < 1e-2, "{d:?}: y");
        }
        // Same placement: no motion.
        let still = prev.hint_after(&prev);
        assert_eq!((still.scale, still.dx, still.dy), (1.0, 0.0, 0.0));
    }

    #[test]
    fn finishing_never_replaces_unless_agreed() {
        let dir = std::env::temp_dir().join(format!(
            "specular-video-export-finalize-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("show.mp4");
        let tmp = temp_path(&dest);
        assert_eq!(
            tmp.parent(),
            dest.parent(),
            "the temporary file is next to it"
        );
        assert!(
            tmp.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".show.mp4.")
        );
        std::fs::write(&tmp, b"new").unwrap();
        assert_eq!(finalize(&tmp, &dest, false).unwrap(), 3);
        assert!(!tmp.exists());
        std::fs::write(&tmp, b"newer").unwrap();
        assert!(
            finalize(&tmp, &dest, false).is_err(),
            "an existing file stays"
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        assert_eq!(finalize(&tmp, &dest, true).unwrap(), 5);
        assert_eq!(std::fs::read(&dest).unwrap(), b"newer");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A gradient PPM with a coloured block that moves with `k`.
    fn ppm(path: &Path, w: usize, h: usize, k: usize) {
        let mut d = format!("P6\n{w} {h}\n255\n").into_bytes();
        for y in 0..h {
            for x in 0..w {
                let inside = (x / 40 + k).is_multiple_of(3) && y > h / 3 && y < 2 * h / 3;
                let px = if inside {
                    [230, 40 + 80 * k as u8, 30]
                } else {
                    [(x * 255 / w) as u8, (y * 255 / h) as u8, (60 * k) as u8]
                };
                d.extend_from_slice(&px);
            }
        }
        std::fs::write(path, d).unwrap();
    }

    fn job(dir: &Path, transition: SlideshowTransition, dest: &str) -> Job {
        let slides = (0..3)
            .map(|k| {
                let p = dir.join(format!("{k}.ppm"));
                ppm(&p, 300 + 20 * k, 200 - 30 * k, k);
                (k, p)
            })
            .collect();
        Job {
            slides,
            seed: 7,
            size: (320, 180),
            fps: 24,
            qp: 26,
            profile: Profile::High,
            interval: 1.0,
            transition,
            caption: SlideshowCaption::Name,
            delays: DelayPolicy::default(),
            upscale_small: true,
            opts: RenderOptions {
                hq: true,
                canvas: [0.0, 0.0, 0.0, 1.0],
                ..RenderOptions::default()
            },
            ui_scale: 180.0 / 1080.0,
            text_scale: 1.0,
            rtl: false,
            lang: "en".parse().unwrap(),
            dest: dir.join(dest),
            replace: true,
        }
    }

    /// The whole export on lavapipe: every transition makes a playable MP4 of the planned
    /// length, the same file twice, and ffmpeg (when installed) decodes it without errors.
    #[test]
    fn shows_export_to_playable_deterministic_mp4() {
        let Some(h) = specular_shots::Harness::new() else {
            return;
        };
        let dir =
            std::env::temp_dir().join(format!("specular-video-export-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let registry = Registry::new();
        for (t, name) in [
            (SlideshowTransition::None, "none"),
            (SlideshowTransition::CrossFade, "fade"),
            (SlideshowTransition::KenBurns, "kb"),
        ] {
            let mut bytes = Vec::new();
            for run_no in 0..2 {
                let j = job(&dir, t, &format!("{name}-{run_no}.mp4"));
                let progress = Progress::default();
                let out = run(
                    h.gpu.clone(),
                    &registry,
                    &j,
                    &progress,
                    Text::bundled_only(),
                );
                let Outcome::Done(d) = out else {
                    panic!("{name}: {out:?}");
                };
                assert_eq!(d.frames, 72, "{name}: 3 slides × 1 s × 24 fps");
                assert_eq!(d.skipped, 0);
                assert_eq!(progress.frames.load(Ordering::Relaxed), 72);
                let data = std::fs::read(&j.dest).unwrap();
                assert_eq!(data.len() as u64, d.bytes);
                let m = specular_video::mp4::parse(&data, true).unwrap();
                let v = m.video.unwrap();
                assert_eq!((v.width, v.height), (320, 180));
                assert_eq!(v.samples.len(), 72);
                assert!(v.samples[0].sync);
                let syncs: Vec<usize> = (0..72).filter(|&i| v.samples[i].sync).collect();
                if t == SlideshowTransition::None {
                    assert_eq!(syncs, [0, 24, 48], "{name}: a cut is an IDR picture");
                } else {
                    assert_eq!(syncs, [0], "{name}: fades are predicted");
                }
                bytes.push(data);
                if run_no == 0 && have_ffmpeg() {
                    let out = std::process::Command::new("ffmpeg")
                        .args(["-v", "error", "-err_detect", "explode", "-i"])
                        .arg(&j.dest)
                        .args(["-f", "null", "-"])
                        .output()
                        .unwrap();
                    assert!(
                        out.status.success() && out.stderr.is_empty(),
                        "{name}: {}",
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
            }
            assert!(
                bytes[0] == bytes[1],
                "{name}: the same show makes the same file"
            );
        }
        // Cancelled before the first frame: nothing is left behind.
        let j = job(&dir, SlideshowTransition::KenBurns, "cancelled.mp4");
        let progress = Progress::default();
        progress.cancel.store(true, Ordering::Relaxed);
        let out = run(
            h.gpu.clone(),
            &registry,
            &j,
            &progress,
            Text::bundled_only(),
        );
        assert!(matches!(out, Outcome::Cancelled), "{out:?}");
        assert!(!j.dest.exists() && !temp_path(&j.dest).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// An export on a lost device (a driver reset, as `Device::destroy` leaves it) fails
    /// without a panic or a hang and leaves nothing behind; the same job then runs on a new
    /// device (what the app does once, after a reset, with a toast).
    #[test]
    fn an_export_ended_by_a_reset_runs_again_on_a_new_device() {
        let old = match specular_gfx::Gpu::new() {
            Ok(g) => g,
            Err(_) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => return,
            Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
        };
        let dir = std::env::temp_dir().join(format!(
            "specular-video-export-reset-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let registry = Registry::new();
        let j = job(&dir, SlideshowTransition::CrossFade, "reset.mp4");
        old.device.destroy();
        let t = std::time::Instant::now();
        let out = run(
            old.clone(),
            &registry,
            &j,
            &Progress::default(),
            Text::bundled_only(),
        );
        assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
        assert!(t.elapsed().as_secs() < 30, "{:?}", t.elapsed());
        assert!(!j.dest.exists() && !temp_path(&j.dest).exists());
        let new = specular_gfx::Gpu::new().expect("a new device");
        let out = run(
            new,
            &registry,
            &j,
            &Progress::default(),
            Text::bundled_only(),
        );
        let Outcome::Done(d) = out else {
            panic!("{out:?}");
        };
        assert_eq!(d.frames, 72);
        assert!(j.dest.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Renders 1080p slideshows of the Kodak photos through the export's own path and writes
    /// each as raw 4:2:0 (`<show>.yuv`) with one line per picture (`<show>.txt`: IDR, fading,
    /// Ken Burns hint) for the encoder benchmark (`specular-video` `bench_slideshow_fades`):
    /// a cross-fade between stills, Ken Burns, and fades to and from black (black slides
    /// between the photos). `SPECULAR_FADE_BENCH_OUT=<dir>`; the photos from `SPECULAR_KODAK`
    /// (default tools/corpus/conformance/kodak), or `SPECULAR_FADE_BENCH_PHOTOS` (paths, comma
    /// separated); `SPECULAR_FADE_BENCH_SLIDES` photos per show (default and at most 8; the
    /// show through black takes at most 5); `SPECULAR_FADE_BENCH_SHOWS` (default all three).
    #[test]
    #[ignore = "writes benchmark input (several GB)"]
    fn dump_slideshows_for_encoder_bench() {
        let Ok(out) = std::env::var("SPECULAR_FADE_BENCH_OUT") else {
            eprintln!("SPECULAR_FADE_BENCH_OUT not set; skipped");
            return;
        };
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        let kodak = std::env::var("SPECULAR_KODAK").map_or_else(
            |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance/kodak"),
            PathBuf::from,
        );
        let slides: usize = std::env::var("SPECULAR_FADE_BENCH_SLIDES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);
        let photos: Vec<PathBuf> = match std::env::var("SPECULAR_FADE_BENCH_PHOTOS") {
            Ok(list) => list.split(',').map(PathBuf::from).collect(),
            Err(_) => [1, 3, 5, 8, 13, 14, 19, 23]
                .iter()
                .map(|k| kodak.join(format!("kodim{k:02}.png")))
                .collect(),
        };
        let photos: Vec<PathBuf> = photos.into_iter().take(slides.max(2)).collect();
        let only = std::env::var("SPECULAR_FADE_BENCH_SHOWS").ok();
        assert!(photos[0].exists(), "no Kodak photos in {}", kodak.display());
        let black = out.join("black.ppm");
        let mut d = b"P6\n768 512\n255\n".to_vec();
        d.resize(d.len() + 768 * 512 * 3, 0);
        std::fs::write(&black, d).unwrap();
        let h = specular_shots::Harness::new().expect("a GPU");
        let registry = Registry::new();
        let mut through_black = Vec::new();
        for (i, p) in photos.iter().take(5).enumerate() {
            if i > 0 {
                through_black.push(black.clone());
            }
            through_black.push(p.clone());
        }
        let shows = [
            ("crossfade", SlideshowTransition::CrossFade, photos.clone()),
            ("kenburns", SlideshowTransition::KenBurns, photos.clone()),
            ("black", SlideshowTransition::CrossFade, through_black),
        ];
        for (name, transition, paths) in shows {
            if only
                .as_ref()
                .is_some_and(|o| !o.split(',').any(|s| s == name))
            {
                continue;
            }
            let t0 = Instant::now();
            let mut j = job(&out, transition, "unused.mp4");
            j.slides = paths.into_iter().enumerate().collect();
            j.size = (1920, 1080);
            j.fps = 30;
            j.interval = 2.0;
            j.caption = SlideshowCaption::Off;
            j.ui_scale = 1.0;
            let progress = Progress::default();
            let (slide_tx, slide_rx) = sync_channel::<Slide>(1);
            let (pic_tx, pic_rx) = sync_channel::<Picture>(3);
            let (frames, meta) = std::thread::scope(|s| {
                let (j, registry, out) = (&j, &registry, &out);
                s.spawn(move || {
                    for (i, p) in &j.slides {
                        let sl = decode(registry, j, *i, p).unwrap();
                        if slide_tx.send(sl).is_err() {
                            break;
                        }
                    }
                });
                let writer = s.spawn(move || {
                    let path = out.join(format!("{name}.yuv"));
                    let mut f = BufWriter::new(File::create(path).unwrap());
                    let mut meta = String::new();
                    let mut n = 0;
                    for p in pic_rx {
                        f.write_all(&p.yuv.data).unwrap();
                        let hint = p
                            .hint
                            .map_or("-".to_owned(), |m| format!("{} {} {}", m.scale, m.dx, m.dy));
                        let (idr, fading) = (u8::from(p.idr), u8::from(p.fading));
                        meta += &format!("{idr} {fading} {hint}\n");
                        n += 1;
                    }
                    f.flush().unwrap();
                    (n, meta)
                });
                let r = render(
                    h.gpu.clone(),
                    j,
                    &progress,
                    Text::bundled_only(),
                    slide_rx,
                    pic_tx,
                );
                assert!(matches!(r, Ok(Some(()))), "{r:?}");
                writer.join().unwrap()
            });
            let txt = out.join(format!("{name}.txt"));
            std::fs::write(txt, format!("1920 1080 30\n{meta}")).unwrap();
            let secs = t0.elapsed().as_secs_f64();
            eprintln!("{name}: {frames} frames in {secs:.1} s");
        }
    }

    fn have_ffmpeg() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
    }
}
