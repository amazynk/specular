//! Making a gallery folder from photos and their edits.
//!
//! 1. Develop's export queue ([`batch::run`]) renders every photo with its edit at the
//!    chosen long edge into a staging folder, as lossless 8-bit sRGB PNGs; the watermark text
//!    goes in through the edit's borders-and-watermark module.
//! 2. Each one is encoded as the gallery's JPEG or WebP (`images/`) and a thumbnail
//!    (`thumbs/`) by the batch encoders, with the source's metadata for an output of that
//!    size: GPS and place names removed unless asked for. Thumbnails carry no metadata.
//! 3. The page is written ([`crate::site`]) and the staging folder removed.
//!
//! The gallery goes in a new folder named after its title inside the folder chosen; nothing
//! that exists is replaced. A cancelled or wholly failed run removes the folder it made.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use specular_convert::encode::{self, Meta};
use specular_convert::pixels::{self, OutTransfer, Target, ToLinear};
use specular_convert::{Conflict, Format, MetaMode, OutputDir, Resize};
use specular_core::{CancelToken, Orientation};
use specular_develop::export::batch::{self, Job, Outcome};
use specular_develop::export::{Depth, FileFormat, Preset, Space};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::{Edit, ModuleInstance, ModuleKind, Registry, Value};
use specular_metadata::Localizer;

use crate::site::{self, Entry, Site};
use crate::{ImageFormat, Options, QUALITY, THUMB_EDGE};

/// How far the gallery is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    /// 0–1 over the whole job.
    pub fraction: f32,
}

/// What a run made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    /// The gallery's folder (removed again when nothing was written or the run was stopped).
    pub dir: PathBuf,
    pub written: usize,
    /// Photos left out, with why.
    pub failed: Vec<(PathBuf, String)>,
    pub cancelled: bool,
}

/// The staging folder's name inside the gallery.
const STAGING: &str = ".staging";

/// A new folder for a gallery titled `title` inside `parent`: the title as a file name,
/// with `-2`, `-3`, … when that is taken.
pub fn gallery_dir(parent: &Path, title: &str) -> PathBuf {
    let base = if title.trim().is_empty() {
        "gallery".to_owned()
    } else {
        site::file_stem(title.trim())
    };
    let mut k = 1;
    loop {
        let name = if k == 1 {
            base.clone()
        } else {
            format!("{base}-{k}")
        };
        let p = parent.join(name);
        if !p.exists() {
            return p;
        }
        k += 1;
    }
}

/// `edit` with `text` drawn on it by the borders-and-watermark module (added when the edit
/// has none; its borders, if any, kept).
pub fn with_watermark(edit: &mut Edit, text: &str) {
    let reg = Registry::builtin();
    if edit.module(ModuleKind::FRAME, 0).is_none() {
        edit.insert(reg, ModuleInstance::new(reg, ModuleKind::FRAME, 0));
    }
    if let Some(m) = edit.module_mut(ModuleKind::FRAME, 0) {
        let f = m
            .params
            .to_fields()
            .with("watermark", Value::Bool(true))
            .with("svg", Value::Text(String::new()))
            .with("text", Value::Text(text.to_owned()));
        m.params.set_fields(f);
        m.enabled = true;
    }
}

/// A unique file stem per photo (case-insensitively, for case-insensitive disks).
fn file_names(jobs: &[Job], ext: &str) -> Vec<String> {
    let mut taken = HashSet::new();
    jobs.iter()
        .map(|j| {
            let stem = site::file_stem(
                &j.source
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );
            let mut k = 1;
            loop {
                let name = if k == 1 {
                    format!("{stem}.{ext}")
                } else {
                    format!("{stem}-{k}.{ext}")
                };
                if taken.insert(name.to_lowercase()) {
                    break name;
                }
                k += 1;
            }
        })
        .collect()
}

/// An id for the browser's storage of this gallery's picks.
fn gallery_id(title: &str, dir: &Path) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    title.hash(&mut h);
    dir.hash(&mut h);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
        .hash(&mut h);
    format!("{:016x}", h.finish())
}

/// The title and caption a photo was given (XMP `dc:title`, `dc:description`; the sidecar's
/// first, then the file's own).
pub fn texts(source: &Path, bytes: &[u8]) -> (Option<String>, Option<String>) {
    let mut packets: Vec<Vec<u8>> = Vec::new();
    if let Some(name) = source.file_name().map(|n| n.to_string_lossy().into_owned()) {
        for c in [
            source.with_extension("xmp"),
            source.with_extension("XMP"),
            source.with_file_name(format!("{name}.xmp")),
            source.with_file_name(format!("{name}.XMP")),
        ] {
            if let Ok(b) = std::fs::read(&c) {
                packets.push(b);
            }
        }
    }
    if let Some(x) = specular_convert::source_meta(bytes).xmp {
        packets.push(x);
    }
    let mut title = None;
    let mut caption = None;
    for p in &packets {
        let x = specular_metadata::xmp::parse(p);
        let get = |k: &str| {
            x.get(k)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        title = title.or_else(|| get("dc:title").or_else(|| get("photoshop:Headline")));
        caption = caption.or_else(|| get("dc:description"));
    }
    (title, caption)
}

/// Camera, lens, exposure and date, in `loc`'s language; never the place.
pub fn exif_summary(bytes: &[u8], loc: &Localizer) -> Vec<String> {
    let tiff = specular_decode::exif_tiff(bytes);
    let xmp = specular_convert::source_meta(bytes).xmp;
    let input = specular_metadata::MetadataInput {
        exif_tiff: tiff.as_deref(),
        xmp: xmp.as_deref(),
        ..Default::default()
    };
    let mut r = specular_metadata::read(&input, loc);
    r.strip_location(loc);
    let item = |group: &str, key: &str| {
        r.summary
            .iter()
            .filter(|g| g.key == group)
            .flat_map(|g| g.items.iter())
            .find(|i| i.key == key || key.is_empty())
            .map(|i| i.value.clone())
    };
    [
        item("camera", "camera"),
        item("lens", "lens"),
        r.exposure_line.clone(),
        item("date", ""),
    ]
    .into_iter()
    .flatten()
    // The page lays its own text out: no bidi isolates from the app's formatting.
    .map(|s| specular_metadata::strip_isolates(&s))
    .filter(|s| !s.trim().is_empty())
    .collect()
}

/// One staged render made into the gallery's photo and thumbnail.
#[allow(clippy::too_many_arguments)]
fn finish_one(
    registry: &specular_decode::Registry,
    staged: &Path,
    job: &Job,
    file: &str,
    dir: &Path,
    opts: &Options,
    loc: &Localizer,
    cancel: &CancelToken,
) -> Result<Entry, String> {
    let bytes = std::fs::read(&job.source).map_err(|e| e.to_string())?;
    let s = registry.open_path(staged).map_err(|e| e.to_string())?;
    let raster = s
        .decode(&specular_decode::DecodeRequest::full(1), cancel)
        .map_err(|e| e.to_string())?;
    let (w, h) = (raster.width, raster.height);
    let target = |width, height| Target {
        width,
        height,
        gray: false,
        alpha: false,
        sixteen: false,
        transfer: OutTransfer::Srgb,
        tone_map: false,
    };
    let render = |t: Target| {
        pixels::render(&raster, &ToLinear::Srgb, &t, Orientation::NORMAL, cancel)
            .map_err(|_| "stopped".to_owned())
    };
    let big = render(target(w, h))?;
    let (tw, th) = specular_convert::output_size(w, h, Resize::LongEdge(THUMB_EDGE), false);
    let small = render(target(tw, th))?;
    let mode = if opts.keep_location {
        MetaMode::All
    } else {
        MetaMode::NoLocation
    };
    let (exif, xmp) = batch::file_meta(&bytes, w, h, mode);
    let icc = Space::Srgb.icc(false);
    let eo = specular_convert::Options {
        format: match opts.format {
            ImageFormat::Jpeg => Format::Jpeg,
            ImageFormat::Webp => Format::Webp,
        },
        jpeg_quality: QUALITY,
        webp_quality: Some(QUALITY),
        webp_effort: 5,
        ..specular_convert::Options::default()
    };
    let photo = encode::encode(
        &eo,
        &big,
        &Meta {
            exif: exif.as_ref(),
            xmp: xmp.as_deref(),
            icc: Some(&icc),
            cicp: None,
        },
    )?;
    let thumb = encode::encode(
        &eo,
        &small,
        &Meta {
            exif: None,
            xmp: None,
            icc: Some(&icc),
            cicp: None,
        },
    )?;
    std::fs::write(dir.join("images").join(file), photo).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("thumbs").join(file), thumb).map_err(|e| e.to_string())?;
    let (title, caption) = texts(&job.source, &bytes);
    let name = job
        .source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Entry {
        key: name.clone(),
        name,
        file: file.to_owned(),
        width: w,
        height: h,
        thumb_width: tw,
        thumb_height: th,
        title,
        caption,
        exif: if opts.exif {
            exif_summary(&bytes, loc)
        } else {
            Vec::new()
        },
    })
}

/// The entries with their picks' names: a file name another photo shares gets its folder's
/// name in front (`2026-05-01/IMG_1.jpg`), so the client's heart is on one photo.
fn pick_keys(entries: Vec<(Entry, Option<PathBuf>)>) -> Vec<Entry> {
    let mut count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (e, _) in &entries {
        *count.entry(e.name.to_lowercase()).or_default() += 1;
    }
    let mut taken = HashSet::new();
    entries
        .into_iter()
        .map(|(mut e, dir)| {
            if count.get(&e.name.to_lowercase()).copied().unwrap_or(0) > 1
                && let Some(d) = dir
                    .as_deref()
                    .and_then(Path::file_name)
                    .map(|d| d.to_string_lossy().into_owned())
                    .filter(|d| !d.is_empty())
            {
                e.key = format!("{d}/{}", e.name);
            }
            // Two folders of one name: the file in `images/` tells them apart.
            if !taken.insert(e.key.to_lowercase()) {
                e.key = format!("{}/{}", e.key, e.file);
                taken.insert(e.key.to_lowercase());
            }
            e
        })
        .collect()
}

/// Makes the gallery of `jobs` in a new folder inside `parent` (see the module docs).
/// `count` says how many photos the page shows ("24 photos") in the page's language; `loc`
/// writes the EXIF summary. Blocks until done or `cancel` fires; run it off the UI thread.
#[allow(clippy::too_many_arguments)]
pub fn build(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    budget: Budget,
    mut jobs: Vec<Job>,
    opts: &Options,
    registry: &specular_decode::Registry,
    parent: &Path,
    count: &(dyn Fn(usize) -> String + Sync),
    loc: &Localizer,
    cancel: &CancelToken,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Built, String> {
    let dir = gallery_dir(parent, &opts.title);
    let staging = dir.join(STAGING);
    for d in [dir.join("images"), dir.join("thumbs"), staging.clone()] {
        std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let wm = opts.watermark.trim();
    if !wm.is_empty() {
        for j in &mut jobs {
            with_watermark(&mut j.edit, wm);
        }
    }
    let n = jobs.len();
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Eight,
        space: Space::Srgb,
        size: Resize::LongEdge(opts.long_edge.max(16)),
        upscale: false,
        metadata: MetaMode::Strip,
        pattern: "{n:05}".into(),
        folder: OutputDir::Folder(staging.clone()),
        conflict: Conflict::Suffix,
        ..Preset::default()
    };
    // Rendering is most of the work.
    let report = batch::run(
        device,
        queue,
        budget,
        jobs.clone(),
        &preset,
        registry,
        cancel,
        &|p: &batch::Progress| {
            progress(Progress {
                done: p.done,
                total: n,
                fraction: p.fraction * 0.8,
            })
        },
    );
    let names = file_names(&jobs, opts.format.ext());
    let mut built = Built {
        dir: dir.clone(),
        cancelled: report.cancelled || cancel.is_cancelled(),
        ..Built::default()
    };
    let mut entries = Vec::new();
    for (i, ((src, outcome), job)) in report.results.iter().zip(&jobs).enumerate() {
        if built.cancelled || cancel.is_cancelled() {
            built.cancelled = true;
            break;
        }
        match outcome {
            Outcome::Written { out, .. } => {
                match finish_one(registry, out, job, &names[i], &dir, opts, loc, cancel) {
                    Ok(e) => entries.push((e, job.source.parent().map(Path::to_path_buf))),
                    Err(e) => built.failed.push((src.clone(), e)),
                }
                let _ = std::fs::remove_file(out);
            }
            Outcome::Skipped { .. } => built.failed.push((src.clone(), "skipped".into())),
            Outcome::Failed(f) => built.failed.push((src.clone(), f.to_string())),
            Outcome::Cancelled => built.cancelled = true,
        }
        progress(Progress {
            done: i + 1,
            total: n,
            fraction: 0.8 + 0.2 * (i + 1) as f32 / n.max(1) as f32,
        });
    }
    let _ = std::fs::remove_dir_all(&staging);
    let entries = pick_keys(entries);
    built.written = entries.len();
    if built.cancelled || entries.is_empty() {
        // Only what this run made: the folder was new.
        let _ = std::fs::remove_dir_all(&dir);
        return Ok(built);
    }
    let mut words = opts.words.clone();
    words.count = count(entries.len());
    let site = Site {
        title: opts.title.trim().to_owned(),
        id: gallery_id(&opts.title, &dir),
        theme: opts.theme,
        captions: opts.captions,
        entries,
        words,
    };
    site::write(&dir, &site).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(built)
}
