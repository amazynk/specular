//! Batch convert: decode each file through the registry at the smallest level that still
//! covers the requested size, resize in linear light, convert or keep its colour, and encode
//! it as a new file with the chosen metadata. Sources are never written to: every output is
//! created next to nothing it could replace (see [`write`]), and a name that would be one of
//! the inputs is refused.
//!
//! [`run`] works in two passes on a background [`JobPool`]: names are settled first (in batch
//! order, so suffixes are deterministic), then the files are converted in parallel, with
//! progress after each file and an end report of what was written, skipped or failed.

mod av1_seq;
mod avif_mux;
pub mod encode;
pub mod exif;
pub mod exr;
pub mod gain_map;
mod pattern;
pub mod pixels;
pub mod recipe;
pub mod write;
pub mod xmp;

pub use pattern::{NameFacts, Pattern, PatternError, civil_from_unix, parse_exif_date};

use specular_color::{ColorSpace, Profile, SourceLut};
use specular_core::hdr::HdrEncoding;
use specular_core::placement::{CoreSet, place_current_thread};
use specular_core::{CancelToken, JobPool, PixelLayout, Priority};
use specular_decode::{DecodeError, DecodeRequest, Registry, Session};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Output formats there is an encoder for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Jxl,
    Avif,
    Tiff,
}

impl Format {
    pub const ALL: [Format; 6] = [
        Format::Jpeg,
        Format::Png,
        Format::Webp,
        Format::Jxl,
        Format::Avif,
        Format::Tiff,
    ];

    /// File extension (without the dot).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
            Format::Jxl => "jxl",
            Format::Avif => "avif",
            Format::Tiff => "tif",
        }
    }

    /// Settings value.
    pub fn id(self) -> &'static str {
        match self {
            Format::Jpeg => "jpeg",
            Format::Png => "png",
            Format::Webp => "webp",
            Format::Jxl => "jxl",
            Format::Avif => "avif",
            Format::Tiff => "tiff",
        }
    }

    pub fn from_id(s: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|f| f.id() == s)
    }

    /// Holds transparency.
    pub fn alpha(self) -> bool {
        self != Format::Jpeg
    }

    /// Holds 16-bit samples.
    pub fn sixteen(self) -> bool {
        matches!(self, Format::Png | Format::Tiff | Format::Jxl)
    }

    /// Embeds ICC profiles (all of them do).
    pub fn icc(self) -> bool {
        true
    }

    /// Holds HDR as such (PQ in a 16-bit PNG with `cICP`, or in JPEG XL's colour encoding).
    pub fn hdr(self) -> bool {
        matches!(self, Format::Png | Format::Jxl)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resize {
    None,
    /// Fit within a box, keeping the aspect ratio.
    Fit {
        width: u32,
        height: u32,
    },
    /// The longer side becomes this long.
    LongEdge(u32),
    Percent(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// Keep the pixel values and embed the source's profile (converted to sRGB only where
    /// the output can't carry the profile).
    Keep,
    /// Convert to sRGB through the source's profile.
    Srgb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdrMode {
    /// Keep HDR where the format can hold it (PNG), tone-map elsewhere.
    Keep,
    ToneMap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaMode {
    All,
    /// Everything but GPS and place names.
    NoLocation,
    /// No Exif or XMP (the colour profile stays: it is part of the picture).
    Strip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// 16 bits when the source has more than 8 and the format allows it.
    Auto,
    Eight,
    Sixteen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// Leave an existing file alone and don't convert.
    Skip,
    /// Add `-2`, `-3`, … to the name.
    Suffix,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputDir {
    /// A folder of this name next to each source (empty: the source's own folder).
    Subfolder(String),
    Folder(PathBuf),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub format: Format,
    /// JPEG quality 1–100.
    pub jpeg_quality: u8,
    /// JPEG without chroma subsampling (4:4:4).
    pub jpeg_full_chroma: bool,
    /// PNG compression effort 1–9 (the zlib level).
    pub png_effort: u8,
    /// Lossy WebP quality 1–100; `None` for lossless.
    pub webp_quality: Option<u8>,
    /// Lossy WebP effort 1–9 (libwebp's method 0–6).
    pub webp_effort: u8,
    /// JPEG XL quality 1–100 (90 is distance 1.0); `None` for lossless.
    pub jxl_quality: Option<u8>,
    /// JPEG XL effort 1–9 (libjxl's).
    pub jxl_effort: u8,
    /// AVIF quality 1–100.
    pub avif_quality: u8,
    /// AVIF effort 1–9 (rav1e speed 10 down to 6).
    pub avif_effort: u8,
    /// TIFF with Deflate compression.
    pub tiff_deflate: bool,
    pub depth: Depth,
    pub resize: Resize,
    /// Allow making images larger.
    pub upscale: bool,
    pub color: ColorMode,
    pub hdr: HdrMode,
    pub metadata: MetaMode,
    pub output: OutputDir,
    pub pattern: String,
    pub conflict: Conflict,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            format: Format::Jpeg,
            jpeg_quality: 90,
            jpeg_full_chroma: false,
            png_effort: 6,
            webp_quality: None,
            webp_effort: 5,
            jxl_quality: None,
            jxl_effort: 1,
            avif_quality: 80,
            avif_effort: 1,
            tiff_deflate: true,
            depth: Depth::Auto,
            resize: Resize::None,
            upscale: false,
            color: ColorMode::Keep,
            hdr: HdrMode::Keep,
            metadata: MetaMode::All,
            output: OutputDir::Subfolder("converted".into()),
            pattern: "{name}".into(),
            conflict: Conflict::Suffix,
        }
    }
}

/// The output size for a `w × h` (as displayed) image.
pub fn output_size(w: u32, h: u32, resize: Resize, upscale: bool) -> (u32, u32) {
    let (w, h) = (w.max(1), h.max(1));
    let s = match resize {
        Resize::None => return (w, h),
        Resize::Fit { width, height } => {
            (width.max(1) as f64 / w as f64).min(height.max(1) as f64 / h as f64)
        }
        Resize::LongEdge(n) => n.max(1) as f64 / w.max(h) as f64,
        Resize::Percent(p) => p.max(1) as f64 / 100.0,
    };
    if s >= 1.0 && !upscale {
        return (w, h);
    }
    let mut ow = ((w as f64 * s).round() as u32).max(1);
    let mut oh = ((h as f64 * s).round() as u32).max(1);
    if let Resize::Fit { width, height } = resize {
        ow = ow.min(width.max(1));
        oh = oh.min(height.max(1));
    }
    if let Resize::LongEdge(n) = resize {
        if w >= h {
            ow = n.max(1);
        } else {
            oh = n.max(1);
        }
    }
    (ow, oh)
}

/// The output folder for `src`.
pub fn output_dir(src: &Path, out: &OutputDir) -> PathBuf {
    let parent = src.parent().unwrap_or(Path::new("."));
    match out {
        OutputDir::Subfolder(s) if s.trim().is_empty() => parent.to_path_buf(),
        OutputDir::Subfolder(s) => parent.join(pattern::sanitize(s.trim())),
        OutputDir::Folder(p) => p.clone(),
    }
}

/// Why a file wasn't converted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Open(String),
    Decode(String),
    Encode(String),
    Write(String),
    /// The output name is one of the inputs.
    SameAsSource(PathBuf),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Open(e) => write!(f, "can't open: {e}"),
            Failure::Decode(e) => write!(f, "can't decode: {e}"),
            Failure::Encode(e) => write!(f, "can't encode: {e}"),
            Failure::Write(e) => write!(f, "can't write: {e}"),
            Failure::SameAsSource(p) => write!(f, "would replace the source {}", p.display()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Converted {
        out: PathBuf,
        bytes: u64,
    },
    /// A file of that name exists (or another file of the batch takes it) and conflicts
    /// are skipped.
    Skipped {
        existing: PathBuf,
    },
    Failed(Failure),
    /// Not done: the batch was cancelled first.
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed: Duration,
    /// Estimated time left, once something has been converted.
    pub eta: Option<Duration>,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    /// One entry per input, in input order.
    pub results: Vec<(PathBuf, Outcome)>,
    pub cancelled: bool,
    pub elapsed: Duration,
}

impl Report {
    fn count(&self, f: impl Fn(&Outcome) -> bool) -> usize {
        self.results.iter().filter(|(_, o)| f(o)).count()
    }
    pub fn converted(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Converted { .. }))
    }
    pub fn skipped(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Skipped { .. }))
    }
    pub fn failed(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Failed(_)))
    }
    pub fn failures(&self) -> impl Iterator<Item = (&Path, &Failure)> {
        self.results.iter().filter_map(|(p, o)| match o {
            Outcome::Failed(f) => Some((p.as_path(), f)),
            _ => None,
        })
    }
}

/// A validated batch.
pub struct Batch {
    pub inputs: Vec<PathBuf>,
    pub options: Options,
    pattern: Pattern,
}

impl Batch {
    pub fn new(inputs: Vec<PathBuf>, options: Options) -> Result<Batch, PatternError> {
        let pattern = Pattern::parse(&options.pattern)?;
        Ok(Batch {
            inputs,
            options,
            pattern,
        })
    }

    /// The name `src` would get from its facts (for the dialog's preview).
    pub fn preview_name(&self, facts: &NameFacts<'_>) -> String {
        self.pattern.render(facts)
    }
}

/// Facts about one source that naming may need.
#[derive(Clone, Debug, Default)]
struct Facts {
    date: Option<[u32; 6]>,
    size: (u32, u32),
    camera: Option<String>,
}

/// The capture date in the file's Exif, else its modification time (UTC).
pub fn capture_date(bytes: &[u8], modified: Option<std::time::SystemTime>) -> Option<[u32; 6]> {
    specular_decode::exif_tiff(bytes)
        .and_then(|t| specular_decode::exif_date_taken(&t))
        .and_then(|s| parse_exif_date(&s))
        .or_else(|| {
            let t = modified?;
            let secs = match t.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => d.as_secs() as i64,
                Err(e) => -(e.duration().as_secs() as i64),
            };
            Some(civil_from_unix(secs))
        })
}

/// The camera ("Make Model" without repeating the make) in the file's Exif.
pub fn camera_model(bytes: &[u8]) -> Option<String> {
    specular_decode::exif_tiff(bytes)
        .and_then(|t| specular_decode::exif_facts(&t))
        .map(|(c, _)| c.camera)
        .filter(|c| !c.trim().is_empty())
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Work for one item of [`parallel`].
type Job<T> = Arc<dyn Fn(usize, &CancelToken) -> T + Send + Sync>;

/// Runs `f(i)` for every `i < n` on up to `threads` jobs of `pool` (pinned to efficiency
/// cores where there are any), stopping early when `cancel` fires; `None` for items not run.
fn parallel<T: Send + 'static>(
    pool: &JobPool,
    threads: usize,
    n: usize,
    cancel: &CancelToken,
    f: Job<T>,
) -> Vec<Option<T>> {
    let next = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel::<(usize, T)>();
    for _ in 0..threads.clamp(1, n.max(1)) {
        let (next, tx, f) = (next.clone(), tx.clone(), f.clone());
        pool.submit_labeled(Priority::Idle, cancel.clone(), "convert", move |c| {
            place_current_thread(CoreSet::Efficiency);
            loop {
                if c.is_cancelled() {
                    return;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    return;
                }
                let v = f(i, c);
                if tx.send((i, v)).is_err() {
                    return;
                }
            }
        });
    }
    drop(tx);
    let mut out: Vec<Option<T>> = (0..n).map(|_| None).collect();
    // Ends when every job has finished or was dropped unrun (its sender with it).
    for (i, v) in rx {
        out[i] = Some(v);
    }
    out
}

/// Converts `batch`, calling `progress` after each file (from worker threads). Blocks until
/// done or cancelled; call it off the UI thread.
pub fn run(
    batch: Arc<Batch>,
    registry: Arc<Registry>,
    pool: &JobPool,
    threads: usize,
    cancel: &CancelToken,
    progress: Arc<dyn Fn(&Progress) + Send + Sync>,
) -> Report {
    let t0 = Instant::now();
    let n = batch.inputs.len();
    let opts = &batch.options;

    // Pass 1: what naming needs (only opened when the pattern asks for size or date).
    let facts: Vec<Option<Result<Facts, Failure>>> =
        if batch.pattern.uses_size() || batch.pattern.uses_date() {
            let (b, r) = (batch.clone(), registry.clone());
            parallel(
                pool,
                threads,
                n,
                cancel,
                Arc::new(move |i, _c| facts_for(&r, &b.inputs[i], &b.options)),
            )
        } else {
            (0..n).map(|_| Some(Ok(Facts::default()))).collect()
        };

    // Names, in batch order.
    let inputs: HashSet<PathBuf> = batch.inputs.iter().map(|p| canonical(p)).collect();
    let mut reserved: HashSet<PathBuf> = HashSet::new();
    let mut plan: Vec<Result<(PathBuf, String), Outcome>> = Vec::with_capacity(n);
    for (i, (src, f)) in batch.inputs.iter().zip(facts).enumerate() {
        let f = match f {
            None => {
                plan.push(Err(Outcome::Cancelled));
                continue;
            }
            Some(Err(e)) => {
                plan.push(Err(Outcome::Failed(e)));
                continue;
            }
            Some(Ok(f)) => f,
        };
        let dir = output_dir(src, &opts.output);
        let st = stem(src);
        let name = batch.pattern.render(&NameFacts {
            stem: &st,
            index: i + 1,
            date: f.date,
            width: f.size.0,
            height: f.size.1,
            ext: opts.format.ext(),
            camera: f.camera.as_deref(),
            ..Default::default()
        });
        let first = canonical(&dir.join(&name));
        if inputs.contains(&first) {
            plan.push(Err(Outcome::Failed(Failure::SameAsSource(src.clone()))));
            continue;
        }
        let mut k = 1;
        let resolved = loop {
            let cand = write::suffixed(&name, k);
            let path = dir.join(&cand);
            let free = !reserved.contains(&canonical(&path))
                && std::fs::symlink_metadata(&path).is_err()
                && !inputs.contains(&canonical(&path));
            if free {
                break Ok(cand);
            }
            if opts.conflict == Conflict::Skip {
                break Err(Outcome::Skipped { existing: path });
            }
            k += 1;
        };
        match resolved {
            Ok(cand) => {
                reserved.insert(canonical(&dir.join(&cand)));
                plan.push(Ok((dir, cand)));
            }
            Err(o) => plan.push(Err(o)),
        }
    }

    // Pass 2: convert.
    let todo: Vec<usize> = (0..n).filter(|&i| plan[i].is_ok()).collect();
    let total = n;
    let pre_skipped = plan
        .iter()
        .filter(|p| matches!(p, Err(Outcome::Skipped { .. })))
        .count();
    let pre_failed = plan
        .iter()
        .filter(|p| matches!(p, Err(Outcome::Failed(_))))
        .count();
    let counters = Arc::new(Mutex::new((n - todo.len(), pre_failed, pre_skipped)));
    progress(&Progress {
        done: n - todo.len(),
        total,
        failed: pre_failed,
        skipped: pre_skipped,
        elapsed: t0.elapsed(),
        eta: None,
    });
    let plan = Arc::new(plan);
    let reserved = Arc::new(reserved);
    let todo = Arc::new(todo);
    let first_done = n - todo.len();
    // The encoders split one image across a rayon pool of the batch's own size and cores, so
    // a single big file uses every thread while a batch of many still runs one per thread.
    let encoders = specular_thread::rayon_pool("enc-conv")
        .num_threads(threads.max(1))
        .start_handler(|_| place_current_thread(CoreSet::Efficiency))
        .build()
        .ok()
        .map(Arc::new);
    let results = {
        let (b, r, plan, reserved, todo, counters, progress, encoders) = (
            batch.clone(),
            registry.clone(),
            plan.clone(),
            reserved.clone(),
            todo.clone(),
            counters.clone(),
            progress.clone(),
            encoders.clone(),
        );
        parallel(
            pool,
            threads,
            todo.len(),
            cancel,
            Arc::new(move |j, c| {
                let i = todo[j];
                let Ok((dir, name)) = &plan[i] else {
                    unreachable!()
                };
                let own = canonical(&dir.join(name));
                let taken = |p: &Path| {
                    let p = canonical(p);
                    p != own && reserved.contains(&p)
                };
                let one = || convert_one(&r, &b.inputs[i], &b.options, dir, name, &taken, c);
                // A decoder or encoder panic fails this file alone (counted as failed, not
                // as cancelled), and the batch goes on.
                let what = format!("convert {}", b.inputs[i].display());
                let out = specular_core::isolate::run(what, || match &encoders {
                    Some(p) => p.install(one),
                    None => one(),
                })
                .unwrap_or_else(|p| Outcome::Failed(Failure::Decode(p.brief())));
                let mut k = counters.lock().unwrap_or_else(|e| e.into_inner());
                if !matches!(out, Outcome::Cancelled) {
                    k.0 += 1;
                }
                match out {
                    Outcome::Failed(_) => k.1 += 1,
                    Outcome::Skipped { .. } => k.2 += 1,
                    _ => {}
                }
                let elapsed = t0.elapsed();
                let worked = k.0 - first_done;
                let left = total - k.0;
                let eta = (worked > 0).then(|| elapsed.mul_f64(left as f64 / worked as f64));
                let p = Progress {
                    done: k.0,
                    total,
                    failed: k.1,
                    skipped: k.2,
                    elapsed,
                    eta,
                };
                drop(k);
                progress(&p);
                out
            }),
        )
    };
    let mut by_index: HashMap<usize, Outcome> = HashMap::new();
    for (j, r) in results.into_iter().enumerate() {
        by_index.insert(todo[j], r.unwrap_or(Outcome::Cancelled));
    }
    let plan = Arc::try_unwrap(plan).unwrap_or_else(|p| (*p).clone());
    let results: Vec<(PathBuf, Outcome)> = plan
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let o = match p {
                Ok(_) => by_index.remove(&i).unwrap_or(Outcome::Cancelled),
                Err(o) => o,
            };
            (batch.inputs[i].clone(), o)
        })
        .collect();
    let cancelled =
        cancel.is_cancelled() || results.iter().any(|(_, o)| matches!(o, Outcome::Cancelled));
    Report {
        results,
        cancelled,
        elapsed: t0.elapsed(),
    }
}

/// The path with its folder resolved (the file itself may not exist yet).
fn canonical(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    match (p.parent(), p.file_name()) {
        (Some(d), Some(n)) => d
            .canonicalize()
            .map(|d| d.join(n))
            .unwrap_or_else(|_| p.to_path_buf()),
        _ => p.to_path_buf(),
    }
}

fn open(registry: &Registry, src: &Path) -> Result<Arc<dyn Session>, Failure> {
    registry
        .open_path(src)
        .map_err(|e| Failure::Open(e.to_string()))
}

fn facts_for(registry: &Registry, src: &Path, opts: &Options) -> Result<Facts, Failure> {
    let s = open(registry, src)?;
    let (w, h) = s.info().display_size();
    Ok(Facts {
        date: capture_date(s.source().bytes(), s.source().modified()),
        size: output_size(w, h, opts.resize, opts.upscale),
        camera: camera_model(s.source().bytes()),
    })
}

/// The displayed size, capture date and camera of an image (for the dialog's name preview).
pub type Probe = ((u32, u32), Option<[u32; 6]>, Option<String>);

/// The [`Probe`] of `src` (headers only).
pub fn probe(registry: &Registry, src: &Path) -> Option<Probe> {
    let s = registry.open_path(src).ok()?;
    let size = s.info().display_size();
    Some((
        size,
        capture_date(s.source().bytes(), s.source().modified()),
        camera_model(s.source().bytes()),
    ))
}

/// The coarsest decoder scale whose result still covers `need` (stored orientation).
pub fn decode_scale(scales: &[u32], size: (u32, u32), need: (u32, u32)) -> u32 {
    scales
        .iter()
        .copied()
        .filter(|&s| s >= 1)
        .filter(|&s| size.0.div_ceil(s) >= need.0 && size.1.div_ceil(s) >= need.1)
        .max()
        .unwrap_or(1)
}

/// Source metadata in the forms the encoders take.
pub struct SourceMeta {
    pub exif: Option<exif::Exif>,
    pub xmp: Option<Vec<u8>>,
}

/// The Exif and XMP of a source file (any container the decoders know).
pub fn source_meta(bytes: &[u8]) -> SourceMeta {
    let exif = specular_decode::exif_tiff(bytes).and_then(|t| exif::Exif::parse(&t));
    let xmp = match specular_metadata::jpeg::scan(bytes) {
        Ok(s) => s.xmp.map(<[u8]>::to_vec),
        Err(_) => specular_decode::embedded_metadata(bytes)
            .xmp
            .map(|x| x.into_owned()),
    };
    SourceMeta { exif, xmp }
}

#[allow(clippy::too_many_arguments)]
fn convert_one(
    registry: &Registry,
    src: &Path,
    opts: &Options,
    dir: &Path,
    name: &str,
    taken: &dyn Fn(&Path) -> bool,
    cancel: &CancelToken,
) -> Outcome {
    match convert_inner(registry, src, opts, dir, name, taken, cancel) {
        Ok(o) => o,
        Err(f) => Outcome::Failed(f),
    }
}

/// Encodes one source to bytes: the decode, pixel and metadata work without the write.
pub fn convert_bytes(
    session: &dyn Session,
    opts: &Options,
    cancel: &CancelToken,
) -> Result<Option<Vec<u8>>, Failure> {
    let info = session.info();
    let (dw, dh) = info.display_size();
    let (ow, oh) = output_size(dw, dh, opts.resize, opts.upscale);
    let swap = info.orientation.swaps_axes();
    let need = if swap { (oh, ow) } else { (ow, oh) };
    let scale = decode_scale(info.scales, (info.width, info.height), need);
    let req = DecodeRequest {
        scale,
        region: None,
        parallel: false,
        prepare_regions: false,
    };
    let raster = match session.decode(&req, cancel) {
        Ok(r) => r,
        Err(DecodeError::Cancelled) => return Ok(None),
        Err(e) => return Err(Failure::Decode(e.to_string())),
    };
    // A truncated file gives what decoded (the rest grey), as the viewer shows it.
    let layout = raster.layout;
    let fmt = opts.format;
    let keep_hdr = layout.is_hdr() && opts.hdr == HdrMode::Keep && fmt.hdr();

    // Colour: which transform makes linear BT.709, and which profile goes into the file.
    let gray = matches!(
        layout,
        PixelLayout::Gray | PixelLayout::Gray16 | PixelLayout::GrayAlpha16
    );
    let icc_bytes: Option<Vec<u8>> = info
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| session.embedded_icc().map(<[u8]>::to_vec));
    let icc_applies = match layout {
        PixelLayout::Rgba16(e) => e == HdrEncoding::SRGB,
        PixelLayout::Rgba16F(_) => false,
        _ => true,
    };
    let profile = icc_bytes
        .as_deref()
        .filter(|_| icc_applies && !keep_hdr)
        .and_then(|b| Profile::parse(b).ok())
        // A grey raster takes an RGB profile on its neutral axis, as the viewer does (a PNG's
        // gAMA/cHRM profile is RGB whatever the colour type).
        .filter(|p| match p.color_space {
            ColorSpace::Gray => gray,
            ColorSpace::Rgb => true,
            _ => false,
        });
    let (to_linear, embed_icc) = match &profile {
        None => (pixels::ToLinear::Srgb, None),
        Some(p)
            if opts.color == ColorMode::Keep
                && fmt.icc()
                && (p.color_space == ColorSpace::Gray) == gray =>
        {
            (pixels::ToLinear::Srgb, icc_bytes.as_deref())
        }
        Some(p) => {
            let pol = specular_color::policy();
            let lut: Option<SourceLut> =
                specular_color::gpu::source_lut_for(p, pol.intent, pol.bpc);
            (
                lut.map_or(pixels::ToLinear::Srgb, pixels::ToLinear::Lut),
                None,
            )
        }
    };

    let src_wide = layout.is_wide() && info.bit_depth > 8;
    let sixteen = keep_hdr
        || (fmt.sixteen()
            && match opts.depth {
                Depth::Auto => src_wide,
                Depth::Eight => false,
                Depth::Sixteen => true,
            });
    // Resampling happens in stored orientation; the size there is the output's, unswapped.
    let target = pixels::Target {
        width: need.0.max(1),
        height: need.1.max(1),
        gray,
        alpha: fmt.alpha(),
        sixteen,
        transfer: if keep_hdr {
            pixels::OutTransfer::Pq
        } else {
            pixels::OutTransfer::Srgb
        },
        tone_map: !keep_hdr,
    };
    let Ok(px) = pixels::render(&raster, &to_linear, &target, info.orientation, cancel) else {
        return Ok(None);
    };
    drop(raster);

    let sm = if opts.metadata == MetaMode::Strip {
        SourceMeta {
            exif: None,
            xmp: None,
        }
    } else {
        source_meta(session.source().bytes())
    };
    let keep_location = opts.metadata == MetaMode::All;
    let exif = sm
        .exif
        .as_ref()
        .and_then(|e| e.for_output(px.width, px.height, keep_location));
    let xmp = sm
        .xmp
        .as_deref()
        .and_then(|x| xmp::sanitize(x, keep_location));
    // The keywords given in Specular (the sidecar's), private ones left out, as Develop's
    // exports have them.
    let xmp = if opts.metadata == MetaMode::Strip {
        xmp
    } else {
        xmp::with_sidecar_keywords(session.source().path(), xmp)
    };
    let meta = encode::Meta {
        exif: exif.as_ref(),
        xmp: xmp.as_deref(),
        icc: embed_icc,
        cicp: keep_hdr.then_some([9, 16, 0, 1]),
    };
    encode::encode(opts, &px, &meta)
        .map(Some)
        .map_err(Failure::Encode)
}

#[allow(clippy::too_many_arguments)]
fn convert_inner(
    registry: &Registry,
    src: &Path,
    opts: &Options,
    dir: &Path,
    name: &str,
    taken: &dyn Fn(&Path) -> bool,
    cancel: &CancelToken,
) -> Result<Outcome, Failure> {
    let session = open(registry, src)?;
    let Some(bytes) = convert_bytes(session.as_ref(), opts, cancel)? else {
        return Ok(Outcome::Cancelled);
    };
    drop(session);
    if cancel.is_cancelled() {
        return Ok(Outcome::Cancelled);
    }
    std::fs::create_dir_all(dir).map_err(|e| Failure::Write(e.to_string()))?;
    match write::write_new(dir, name, &bytes, opts.conflict, taken) {
        Ok(write::Written::Created(out)) => Ok(Outcome::Converted {
            out,
            bytes: bytes.len() as u64,
        }),
        Ok(write::Written::Exists(existing)) => Ok(Outcome::Skipped { existing }),
        Err(e) => Err(Failure::Write(e.to_string())),
    }
}

#[cfg(test)]
mod tests;
