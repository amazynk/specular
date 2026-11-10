//! The export queue: images rendered one at a time on the GPU while the previous one is
//! encoded and written on a thread of its own and the next one is read and decoded on
//! another (one image ahead), so the three overlap. Each output is a new
//! file (`specular_convert::write::write_new`: never replaces anything, a taken name gets a
//! suffix or is skipped); a name that would be the source itself is refused.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use specular_convert::exif::Exif;
use specular_convert::{Conflict, Failure, MetaMode, NameFacts, Pattern, write};
use specular_core::CancelToken;

use super::preset::{Depth, FileFormat, Preset};
use super::render::{Exported, Exporter, Samples};
use crate::gpu::pool::Budget;
use crate::model::{Edit, ModuleKind, Value};
use crate::preview::{self, Input, Orient};

/// One image to export.
#[derive(Clone, Debug)]
pub struct Job {
    pub source: PathBuf,
    pub edit: Edit,
    /// How to turn it; `None`: as the file's orientation says.
    pub orient: Option<Orient>,
}

/// What became of one image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Written {
        out: PathBuf,
        bytes: u64,
    },
    /// The name was taken and the preset skips taken names.
    Skipped {
        existing: PathBuf,
    },
    Failed(Failure),
    /// Not done: stopped first.
    Cancelled,
}

/// The queue so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub failed: usize,
    pub skipped: usize,
    /// All of it, the image being rendered counted by its bands.
    pub fraction: f32,
    pub elapsed: Duration,
    /// Once something is done.
    pub eta: Option<Duration>,
}

/// Where one image's time went (wall clock; render and encode of different images overlap).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timing {
    pub read_ms: f64,
    pub source_ms: f64,
    pub render_ms: f64,
    pub encode_ms: f64,
    pub write_ms: f64,
    pub pixels: u64,
}

/// The end of a queue.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// One per job, in order.
    pub results: Vec<(PathBuf, Outcome)>,
    pub timings: Vec<Timing>,
    pub cancelled: bool,
    pub elapsed: Duration,
}

impl Report {
    pub fn written(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Written { .. }))
    }
    pub fn skipped(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Skipped { .. }))
    }
    pub fn failed(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Failed(_)))
    }
    fn count(&self, f: impl Fn(&Outcome) -> bool) -> usize {
        self.results.iter().filter(|(_, o)| f(o)).count()
    }
}

/// Extensions that are never raws (read through the registry straight away).
const NOT_RAW: [&str; 16] = [
    "jpg", "jpeg", "png", "webp", "avif", "jxl", "heic", "heif", "tif", "tiff", "gif", "bmp",
    "exr", "hdr", "psd", "jp2",
];

/// A file read for export.
pub struct Read {
    pub input: Input,
    /// EXIF orientation (1..=8).
    pub orientation: u16,
    /// The raw's as-shot white (CIE xy), when known.
    pub as_shot: Option<[f64; 2]>,
    pub bytes: Vec<u8>,
}

/// Reads `path` at full size: a raw's sensor data, else the decoded image through its
/// profile in linear Rec.2020 ([`crate::source`]).
pub fn read_input(path: &Path, registry: &specular_decode::Registry) -> Result<Read, Failure> {
    let bytes = std::fs::read(path).map_err(|e| Failure::Open(e.to_string()))?;
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !NOT_RAW.contains(&ext.as_str())
        && let Ok(Input::Raw(s)) = preview::decode_raw(&bytes)
    {
        let as_shot = preview::as_shot_white_of(&s, &bytes);
        let orientation = s.meta.orientation;
        return Ok(Read {
            input: Input::Raw(s),
            orientation,
            as_shot,
            bytes,
        });
    }
    let s = registry
        .open_path(path)
        .map_err(|e| Failure::Open(e.to_string()))?;
    let info = s.info();
    let orientation = u16::from(info.orientation.0);
    let cancel = CancelToken::new();
    let raster = s
        .decode(&specular_decode::DecodeRequest::full(1), &cancel)
        .map_err(|e| Failure::Decode(e.to_string()))?;
    let icc: Option<Vec<u8>> = info
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| s.embedded_icc().map(<[u8]>::to_vec));
    // Through its profile into linear Rec.2020, unclipped, transparency kept.
    let input = crate::source::input(&raster, icc.as_deref(), None, &cancel)
        .ok_or_else(|| Failure::Decode("cancelled".to_owned()))?;
    drop(raster);
    Ok(Read {
        input,
        orientation,
        as_shot: None,
        bytes,
    })
}

/// Sets the image's as-shot white in `edit` (and the white, unless the user chose one), as
/// Develop does when it opens the image.
pub fn set_as_shot(edit: &mut Edit, xy: [f64; 2]) {
    let Some(m) = edit.module_mut(ModuleKind::COLOUR_CALIBRATION, 0) else {
        return;
    };
    let mut f = m.params.to_fields();
    let (x, y) = (Value::Float(xy[0] as f32), Value::Float(xy[1] as f32));
    f.set("as_shot_x", x.clone());
    f.set("as_shot_y", y.clone());
    if f.text("white_source") != Some("custom") {
        f.set("white_x", x);
        f.set("white_y", y);
    }
    m.params.set_fields(f);
}

/// Exif and XMP for an output of `w × h` (orientation 1), as `mode` allows.
pub fn file_meta(source: &[u8], w: u32, h: u32, mode: MetaMode) -> (Option<Exif>, Option<Vec<u8>>) {
    if mode == MetaMode::Strip {
        return (None, None);
    }
    let keep = mode == MetaMode::All;
    let sm = specular_convert::source_meta(source);
    let exif = sm.exif.as_ref().and_then(|e| e.for_output(w, h, keep));
    let xmp = sm
        .xmp
        .as_deref()
        .and_then(|x| specular_convert::xmp::sanitize(x, keep));
    (exif, xmp)
}

/// The file bytes of `img` as `preset` asks, with `exif` and `xmp` (where the format holds
/// them) and the space's profile.
pub fn encode(
    img: &Exported,
    preset: &Preset,
    exif: Option<&Exif>,
    xmp: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    encode_samples(img, None, preset, exif, xmp)
}

/// [`encode`], taking `img`'s samples instead of copying them (72 MB at 24 MP in 8 bits)
/// when the file gets them as they are.
pub(crate) fn encode_taking(
    mut img: Exported,
    preset: &Preset,
    exif: Option<&Exif>,
    xmp: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let samples = std::mem::replace(&mut img.samples, Samples::Eight(Vec::new()));
    encode_samples(&img, Some(samples), preset, exif, xmp)
}

/// [`encode`] of `img` with its samples `owned` (taken out of it) or, for `None`, its own.
fn encode_samples(
    img: &Exported,
    owned: Option<Samples>,
    preset: &Preset,
    exif: Option<&Exif>,
    xmp: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    use specular_convert::encode::{self as enc, Meta};
    let p = preset.clone().normalised();
    let float = p.depth.float();
    let icc = p.space.icc(float);
    let cicp = p.hdr.cicp();
    let meta = Meta {
        exif,
        xmp,
        // PQ and HLG are described by their code points alone.
        icc: cicp.is_none().then_some(&icc[..]),
        cicp,
    };
    let (w, h) = (img.width, img.height);
    // The source's transparency, where the file can hold it (not with a gain map or an
    // HDR signal, whose encoders take RGB).
    let alpha = img
        .alpha
        .as_deref()
        .filter(|a| a.len() == w as usize * h as usize)
        .filter(|_| keeps_alpha(p.format) && img.gain_map.is_none() && cicp.is_none());
    let c = if alpha.is_some() { 4 } else { 3 };
    let samples = |owned: Option<Samples>| -> Samples {
        let Some(a) = alpha else {
            return owned.unwrap_or_else(|| img.samples.clone());
        };
        match owned.as_ref().unwrap_or(&img.samples) {
            Samples::Eight(v) => Samples::Eight(with_alpha(v, a, |x| (x * 255.0 + 0.5) as u8)),
            Samples::Sixteen(v) => {
                Samples::Sixteen(with_alpha(v, a, |x| (x * 65535.0 + 0.5) as u16))
            }
            Samples::Float(v) => Samples::Float(with_alpha(v, a, |x| x)),
        }
    };
    match (samples(owned), p.format) {
        (Samples::Float(v), FileFormat::Exr) => specular_convert::exr::encode_with(
            w,
            h,
            c,
            &v,
            &specular_convert::exr::ExrOptions {
                sample: if p.depth == Depth::Half {
                    specular_convert::exr::Sample::Half
                } else {
                    specular_convert::exr::Sample::Float
                },
                zip: true,
                chromaticities: Some(p.space.chromaticities()),
            },
            // The metadata the preset keeps (its level cut `exif` and `xmp` already).
            &specular_convert::exr::ExrMeta::from_exif(exif, xmp),
        ),
        (Samples::Float(v), FileFormat::Tiff) => Ok(enc::tiff_float(w, h, c, &v, &meta, true)),
        (Samples::Eight(v), _) if img.gain_map.is_some() => enc::encode_with_gain_map(
            &p.convert_options(),
            &specular_convert::pixels::Pixels {
                width: w,
                height: h,
                channels: 3,
                samples: specular_convert::pixels::Samples::Eight(v),
            },
            &meta,
            img.gain_map.as_ref().expect("checked"),
            super::hdr::cicp_primaries(p.space),
        ),
        (Samples::Eight(v), _) => enc::encode(
            &p.convert_options(),
            &specular_convert::pixels::Pixels {
                width: w,
                height: h,
                channels: c,
                samples: specular_convert::pixels::Samples::Eight(v),
            },
            &meta,
        ),
        (Samples::Sixteen(v), _) => enc::encode(
            &p.convert_options(),
            &specular_convert::pixels::Pixels {
                width: w,
                height: h,
                channels: c,
                samples: specular_convert::pixels::Samples::Sixteen(v),
            },
            &meta,
        ),
        _ => Err("these samples do not fit the format".into()),
    }
}

/// Formats Develop exports with transparency (JPEG has none).
pub fn keeps_alpha(f: FileFormat) -> bool {
    matches!(
        f,
        FileFormat::Png | FileFormat::Avif | FileFormat::Jxl | FileFormat::Tiff | FileFormat::Exr
    )
}

/// RGB samples with a fourth, the coverage `a` coded by `code`.
fn with_alpha<T: Copy>(rgb: &[T], a: &[f32], code: impl Fn(f32) -> T) -> Vec<T> {
    let mut out = Vec::with_capacity(rgb.len() / 3 * 4);
    for (p, &a) in rgb.chunks_exact(3).zip(a) {
        out.extend_from_slice(p);
        out.push(code(a.clamp(0.0, 1.0)));
    }
    out
}

/// The name `job`'s output gets (before any suffix).
fn name_for(
    pattern: &Pattern,
    src: &Path,
    index: usize,
    bytes: &[u8],
    size: (u32, u32),
    ext: &str,
) -> String {
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let modified = std::fs::metadata(src).and_then(|m| m.modified()).ok();
    let date = specular_convert::capture_date(bytes, modified);
    let camera = specular_convert::camera_model(bytes);
    pattern.render(&NameFacts {
        stem: &stem,
        index,
        date,
        width: size.0,
        height: size.1,
        ext,
        camera: camera.as_deref(),
        ..Default::default()
    })
}

/// Same file (when both resolve).
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// What the reader thread hands over: the job's index, the file read (or why not), the
/// read's milliseconds, and a raw's noise model when denoise wants it.
type Ahead = (
    usize,
    Result<Read, Failure>,
    f64,
    Option<crate::ops::detail::RawNoise>,
);

/// Whether `edit` denoises (the reader then fits a raw's noise model).
fn denoises(edit: &Edit) -> bool {
    edit.modules
        .iter()
        .any(|m| m.kind == ModuleKind::DENOISE && m.enabled)
}

/// What the writer thread is handed.
struct Rendered {
    index: usize,
    img: Exported,
    bytes: Vec<u8>,
    timing: Timing,
}

/// Exports `jobs` as `preset` says on `device`, calling `progress` as it goes (from this
/// thread and the writer's). Blocks until done or `cancel` fires; run it off the UI thread.
#[allow(clippy::too_many_arguments)]
pub fn run(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    budget: Budget,
    jobs: Vec<Job>,
    preset: &Preset,
    registry: &specular_decode::Registry,
    cancel: &CancelToken,
    progress: &(dyn Fn(&Progress) + Sync),
) -> Report {
    let t0 = Instant::now();
    let preset = preset.clone().normalised();
    let n = jobs.len();
    let sources: Vec<PathBuf> = jobs.iter().map(|j| j.source.clone()).collect();
    let mut results: Vec<Outcome> = vec![Outcome::Cancelled; n];
    let mut timings = vec![Timing::default(); n];
    let pattern = match Pattern::parse(&preset.pattern) {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("the name pattern is not valid: {e:?}");
            return Report {
                results: sources
                    .into_iter()
                    .map(|s| (s, Outcome::Failed(Failure::Write(msg.clone()))))
                    .collect(),
                timings,
                cancelled: false,
                elapsed: t0.elapsed(),
            };
        }
    };
    let state = std::sync::Mutex::new((0usize, 0usize, 0usize, 0f32));
    let report_progress = |done_delta: usize, failed: usize, skipped: usize, part: Option<f32>| {
        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
        s.0 += done_delta;
        s.1 += failed;
        s.2 += skipped;
        if let Some(p) = part {
            s.3 = p;
        } else if done_delta > 0 {
            s.3 = 0.0;
        }
        let done = s.0;
        let elapsed = t0.elapsed();
        let fraction = ((done as f32 + s.3 * 0.8) / n.max(1) as f32).clamp(0.0, 1.0);
        let eta = (done > 0 && done < n).then(|| elapsed.mul_f64((n - done) as f64 / done as f64));
        let p = Progress {
            done,
            total: n,
            failed: s.1,
            skipped: s.2,
            fraction,
            elapsed,
            eta,
        };
        drop(s);
        progress(&p);
    };
    report_progress(0, 0, 0, None);

    let (tx, rx) = mpsc::sync_channel::<Rendered>(1);
    let written: Vec<(usize, Outcome, Timing)> = std::thread::scope(|scope| {
        let writer = specular_thread::spawn_scoped(scope, "dv-export-write", || {
            let mut out = Vec::new();
            for r in rx {
                let mut timing = r.timing;
                let job = &jobs[r.index];
                let size = (r.img.width, r.img.height);
                let res = (|| {
                    let te = Instant::now();
                    let (exif, mut xmp) = file_meta(&r.bytes, size.0, size.1, preset.metadata);
                    // The keywords given in Specular (the sidecar's), private ones left out.
                    if preset.metadata != MetaMode::Strip {
                        xmp = specular_convert::xmp::with_sidecar_keywords(&job.source, xmp);
                    }
                    // The picture is not needed after this: the file takes its samples.
                    let file = encode_taking(r.img, &preset, exif.as_ref(), xmp.as_deref())
                        .map_err(Failure::Encode)?;
                    timing.encode_ms = te.elapsed().as_secs_f64() * 1e3;
                    if cancel.is_cancelled() {
                        return Ok(Outcome::Cancelled);
                    }
                    let tw = Instant::now();
                    let dir = specular_convert::output_dir(&job.source, &preset.folder);
                    let name = name_for(
                        &pattern,
                        &job.source,
                        r.index + 1,
                        &r.bytes,
                        size,
                        preset.format.ext(),
                    );
                    if same_file(&dir.join(&name), &job.source) {
                        return Err(Failure::SameAsSource(job.source.clone()));
                    }
                    std::fs::create_dir_all(&dir).map_err(|e| Failure::Write(e.to_string()))?;
                    let src = job.source.clone();
                    let taken = move |p: &Path| same_file(p, &src);
                    let o = match write::write_new(&dir, &name, &file, preset.conflict, &taken) {
                        Ok(write::Written::Created(out)) => Outcome::Written {
                            out,
                            bytes: file.len() as u64,
                        },
                        Ok(write::Written::Exists(existing)) => Outcome::Skipped { existing },
                        Err(e) => return Err(Failure::Write(e.to_string())),
                    };
                    timing.write_ms = tw.elapsed().as_secs_f64() * 1e3;
                    Ok(o)
                })();
                let o = res.unwrap_or_else(Outcome::Failed);
                let (f, s) = match &o {
                    Outcome::Failed(_) => (1, 0),
                    Outcome::Skipped { .. } => (0, 1),
                    _ => (0, 0),
                };
                if o != Outcome::Cancelled {
                    report_progress(1, f, s, None);
                }
                out.push((r.index, o, timing));
            }
            out
        });

        // The reader: one image read and decoded ahead of the one rendering (a rendezvous
        // channel, so at most one waits).
        let (rtx, rrx) = mpsc::sync_channel::<Ahead>(0);
        let jobs_r = &jobs;
        let reader = specular_thread::spawn_scoped(scope, "dv-export-read", move || {
            for (i, job) in jobs_r.iter().enumerate() {
                if cancel.is_cancelled() {
                    break;
                }
                let tr = Instant::now();
                let read = read_input(&job.source, registry);
                let noise = match &read {
                    Ok(Read {
                        input: Input::Raw(s),
                        ..
                    }) if denoises(&job.edit) => Some(crate::ops::detail::RawNoise::of_sensor(s)),
                    _ => None,
                };
                let ms = tr.elapsed().as_secs_f64() * 1e3;
                if rtx.send((i, read, ms, noise)).is_err() {
                    break;
                }
            }
        });

        let mut exporter = Exporter::new(device, queue, budget);
        for (i, read, read_ms, noise) in &rrx {
            let job = &jobs[i];
            if cancel.is_cancelled() {
                break;
            }
            let read = match read {
                Ok(r) => r,
                Err(e) => {
                    results[i] = Outcome::Failed(e);
                    report_progress(1, 1, 0, None);
                    continue;
                }
            };
            let mut timing = Timing {
                read_ms,
                ..Timing::default()
            };
            let mut part = |f: f32| {
                report_progress(0, 0, 0, Some(f));
                !cancel.is_cancelled()
            };
            match render_job(&mut exporter, job, read, noise, &preset, &mut part) {
                Ok((img, bytes)) => {
                    timing.source_ms = img.stats.source_ms;
                    timing.render_ms = img.stats.render_ms;
                    timing.pixels = u64::from(img.width) * u64::from(img.height);
                    let r = Rendered {
                        index: i,
                        img,
                        bytes,
                        timing,
                    };
                    if tx.send(r).is_err() {
                        break;
                    }
                }
                Err(_) if cancel.is_cancelled() => break,
                Err(e) => {
                    results[i] = Outcome::Failed(e);
                    report_progress(1, 1, 0, None);
                }
            }
        }
        drop(rrx);
        let _ = reader.join();
        drop(tx);
        writer.join().unwrap_or_default()
    });
    for (i, o, t) in written {
        results[i] = o;
        timings[i] = t;
    }
    let cancelled = cancel.is_cancelled() || results.contains(&Outcome::Cancelled);
    Report {
        results: sources.into_iter().zip(results).collect(),
        timings,
        cancelled,
        elapsed: t0.elapsed(),
    }
}

/// Renders one image of the queue as `preset` says, with what the edit needs set up on
/// `exporter` first: the raw's as-shot white, flat and dark frames, lens data (its own or
/// Lensfun's), the AI denoise key, and borders and watermark (left out when they cannot be
/// drawn). `read` is the file read by [`read_input`] (its bytes are handed back with the
/// result); `noise` a raw's noise model when the edit denoises. `progress` gets the
/// fraction done and returns false to stop.
pub fn render_job(
    exporter: &mut Exporter,
    job: &Job,
    read: Read,
    noise: Option<crate::ops::detail::RawNoise>,
    preset: &Preset,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<(Exported, Vec<u8>), Failure> {
    exporter.raw_noise = noise;
    let mut edit = job.edit.clone();
    if let Some(xy) = read.as_shot {
        set_as_shot(&mut edit, xy);
    }
    // The flat and dark frames the edit asks for.
    exporter.calibration = None;
    if let Input::Raw(s) = &read.input {
        exporter.calibration =
            crate::ops::correct::calib::for_edit(&edit, s, Some((&job.source, &read.bytes)))
                .map_err(Failure::Decode)?;
    }
    let orient = job
        .orient
        .unwrap_or_else(|| Orient::from_exif(read.orientation));
    let vars = crate::ops::effects::frame::Vars::of_file(&job.source, &read.bytes);
    // Capture sharpening's radius from the lens.
    let (sw, sh) = super::render::source_size(&read.input);
    // The photo keys denoise's AI result.
    exporter.photo = Some(crate::ops::detail::ai::photo_of_path(&job.source));
    // Phase 3 item 11: a Lensfun profile for a raw without lens data of its own.
    exporter.lensfun = match &read.input {
        Input::Raw(s) => crate::ops::geometry::lensfun::shot_of_raw(s, &read.bytes),
        _ => None,
    };
    exporter.focal35 = crate::ops::geometry::persp::focal35_of_file(&read.bytes);
    exporter.lens = Some(crate::ops::detail::capture::lens_of_file(
        &read.bytes,
        sw.max(sh),
    ));
    // Borders and watermark, on every kind of export (a watermark that cannot be
    // drawn is left out).
    super::hdr::render_framed(
        exporter,
        read.input,
        &edit,
        orient,
        preset,
        Some(&vars),
        progress,
    )
    .map(|img| (img, read.bytes))
    .map_err(|e| Failure::Decode(e.to_string()))
}

/// Opens a device for exports outside the app (the command line): the high-performance
/// adapter, else the software one.
pub fn open_device() -> Result<(wgpu::Device, wgpu::Queue, bool), String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.flags = wgpu::InstanceFlags::empty().with_env();
    desc.backends = wgpu::Backends::PRIMARY.with_env();
    let instance = wgpu::Instance::new(desc);
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    })) {
        Ok(a) => a,
        Err(_) => {
            let (d, q) = preview::open_software_device()?;
            return Ok((d, q, true));
        }
    };
    let integrated = matches!(
        adapter.get_info().device_type,
        wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::Cpu
    );
    let a = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(a.clone());
    limits.max_storage_buffer_binding_size = a.max_storage_buffer_binding_size;
    limits.max_buffer_size = a.max_buffer_size;
    let (d, q) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("develop export"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("cannot open the GPU: {e}"))?;
    Ok((d, q, integrated))
}

/// Conflicts in the text form (for the command line's `--conflict`).
pub fn conflict_from_id(s: &str) -> Option<Conflict> {
    match s {
        "suffix" => Some(Conflict::Suffix),
        "skip" => Some(Conflict::Skip),
        _ => None,
    }
}
