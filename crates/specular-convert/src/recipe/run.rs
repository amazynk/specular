//! Running recipes on a selection: [`plan`] settles every output's folder and name (image by
//! image, recipe by recipe, so suffixes are deterministic) and finds the files a recipe
//! would replace, so they can be confirmed first; [`execute`] then renders each image once,
//! at the largest size any of its recipes needs, and finishes and writes it per recipe on a
//! few worker threads, with progress after each file, cancel, and an end report.
//!
//! An original is never written to: an output name that is one of the inputs fails that
//! output. A replaced file is replaced atomically (written beside it, then renamed over it).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use specular_core::CancelToken;
use specular_core::placement::{CoreSet, place_current_thread};
use specular_decode::{DecodeError, DecodeRequest, Registry, Session};

use super::finish::{self, Linear};
use super::size::needed_scale;
use super::space::Primaries;
use super::watermark::{self, Mark};
use super::{Collision, Destination, Recipe};
use crate::{
    Failure, NameFacts, Outcome, Pattern, PatternError, Progress, SourceMeta, pixels, write,
};

/// One image to export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub path: PathBuf,
    /// Its star rating when the caller knows it (else read from the file and its sidecar).
    pub rating: Option<u8>,
}

impl Item {
    pub fn new(path: impl Into<PathBuf>) -> Item {
        Item {
            path: path.into(),
            rating: None,
        }
    }
}

/// What naming and sizing need to know of an image before it is rendered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// Upright, full size.
    pub size: (u32, u32),
    pub date: Option<[u32; 6]>,
    pub camera: Option<String>,
    pub rating: Option<u8>,
}

/// A rendered image.
pub struct Rendered {
    pub image: Linear,
    /// Upright full size (the image may be smaller).
    pub full: (u32, u32),
    pub meta: SourceMeta,
}

/// What renders images for recipes: plain files ([`Files`]), or Develop's edits.
pub trait Render: Send + Sync {
    /// Headers only.
    fn facts(&self, item: &Item) -> Result<Facts, Failure>;
    /// The image upright in linear light, at least `scale` of its full size (1.0: full).
    /// `None` when cancelled.
    fn render(
        &self,
        item: &Item,
        scale: f64,
        cancel: &CancelToken,
    ) -> Result<Option<Rendered>, Failure>;
}

/// Plain files through the decoders (no edits).
pub struct Files {
    pub registry: Arc<Registry>,
}

fn open(registry: &Registry, p: &Path) -> Result<Arc<dyn Session>, Failure> {
    registry
        .open_path(p)
        .map_err(|e| Failure::Open(e.to_string()))
}

impl Render for Files {
    fn facts(&self, item: &Item) -> Result<Facts, Failure> {
        let s = open(&self.registry, &item.path)?;
        let bytes = s.source().bytes();
        let rating = item
            .rating
            .or_else(|| super::meta::rating_of(&item.path, &crate::source_meta(bytes)));
        Ok(Facts {
            size: s.info().display_size(),
            date: crate::capture_date(bytes, s.source().modified()),
            camera: crate::camera_model(bytes),
            rating,
        })
    }

    fn render(
        &self,
        item: &Item,
        scale: f64,
        cancel: &CancelToken,
    ) -> Result<Option<Rendered>, Failure> {
        let session = open(&self.registry, &item.path)?;
        let info = session.info();
        let full = info.display_size();
        let need = (
            (f64::from(full.0) * scale).ceil() as u32,
            (f64::from(full.1) * scale).ceil() as u32,
        );
        let need = if info.orientation.swaps_axes() {
            (need.1, need.0)
        } else {
            need
        };
        let req = DecodeRequest {
            scale: crate::decode_scale(info.scales, (info.width, info.height), need),
            region: None,
            parallel: true,
            prepare_regions: false,
        };
        let raster = match session.decode(&req, cancel) {
            Ok(r) => r,
            Err(DecodeError::Cancelled) => return Ok(None),
            Err(e) => return Err(Failure::Decode(e.to_string())),
        };
        let to_linear = to_linear(session.as_ref(), raster.layout);
        let Ok(rgba) = pixels::linear_rgba(&raster, &to_linear, true, cancel) else {
            return Ok(None);
        };
        let (w, h) = (raster.width as usize, raster.height as usize);
        drop(raster);
        let (w, h, rgba) = pixels::orient(w, h, 4, &rgba, info.orientation);
        let meta = crate::source_meta(session.source().bytes());
        Ok(Some(Rendered {
            image: Linear {
                width: w as u32,
                height: h as u32,
                rgba,
                primaries: Primaries::Bt709,
            },
            full,
            meta,
        }))
    }
}

/// How the raster's values become linear BT.709: through its profile when it has one.
fn to_linear(session: &dyn Session, layout: specular_core::PixelLayout) -> pixels::ToLinear {
    use specular_color::{ColorSpace, Profile};
    use specular_core::PixelLayout;
    use specular_core::hdr::HdrEncoding;
    let info = session.info();
    let gray = matches!(
        layout,
        PixelLayout::Gray | PixelLayout::Gray16 | PixelLayout::GrayAlpha16
    );
    let applies = match layout {
        PixelLayout::Rgba16(e) => e == HdrEncoding::SRGB,
        PixelLayout::Rgba16F(_) => false,
        _ => true,
    };
    let bytes: Option<Vec<u8>> = info
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| session.embedded_icc().map(<[u8]>::to_vec));
    let profile = bytes
        .as_deref()
        .filter(|_| applies)
        .and_then(|b| Profile::parse(b).ok())
        .filter(|p| match p.color_space {
            ColorSpace::Gray => gray,
            ColorSpace::Rgb => true,
            _ => false,
        });
    match profile {
        None => pixels::ToLinear::Srgb,
        Some(p) => {
            let pol = specular_color::policy();
            specular_color::gpu::source_lut_for(&p, pol.intent, pol.bpc)
                .map_or(pixels::ToLinear::Srgb, pixels::ToLinear::Lut)
        }
    }
}

/// What a planned output will do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Planned {
    /// A new file.
    Write,
    /// Replace this existing file (the recipe overwrites; confirmed first).
    Replace,
    /// Not written: the name is taken and the recipe skips taken names.
    Skip,
    Fail(Failure),
}

/// One output: an image through one recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub item: usize,
    pub recipe: usize,
    pub path: PathBuf,
    pub planned: Planned,
}

/// Every output of a run, before anything is written.
#[derive(Clone, Debug)]
pub struct Plan {
    pub items: Vec<Item>,
    pub recipes: Vec<Recipe>,
    pub facts: Vec<Result<Facts, Failure>>,
    pub outputs: Vec<Output>,
}

impl Plan {
    /// The existing files the run would replace.
    pub fn replacing(&self) -> Vec<&Path> {
        self.outputs
            .iter()
            .filter(|o| o.planned == Planned::Replace)
            .map(|o| o.path.as_path())
            .collect()
    }
}

/// Why a run can't be planned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// A recipe's name pattern does not parse.
    Pattern {
        recipe: usize,
        error: PatternError,
    },
    /// A recipe asks for its folder and none was given.
    NoFolder {
        recipe: usize,
    },
    Cancelled,
}

/// The folder an output of `r` goes to for `src`; `None` for a folder still to be asked.
pub fn folder(src: &Path, r: &Recipe) -> Option<PathBuf> {
    let parent = src.parent().unwrap_or(Path::new("."));
    match &r.destination {
        Destination::Same => Some(parent.to_path_buf()),
        Destination::Subfolder(s) if s.trim().is_empty() => Some(parent.to_path_buf()),
        Destination::Subfolder(s) => Some(parent.join(crate::pattern::sanitize(s.trim()))),
        Destination::Fixed(p) => Some(p.clone()),
        Destination::Ask => None,
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

/// The name an image gets from `pattern` (also the dialog's preview).
pub fn name_for(pattern: &Pattern, r: &Recipe, src: &Path, index: usize, f: &Facts) -> String {
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let t = finish::target_of(r, f.size);
    pattern.render(&NameFacts {
        stem: &stem,
        index,
        date: f.date,
        width: t.width,
        height: t.height,
        ext: r.format.ext(),
        camera: f.camera.as_deref(),
        rating: f.rating,
        text: &r.text,
    })
}

/// Settles every output (recipes whose destination is [`Destination::Ask`] must have been
/// given a [`Destination::Fixed`] folder first).
pub fn plan(
    items: Vec<Item>,
    recipes: Vec<Recipe>,
    render: &dyn Render,
    cancel: &CancelToken,
) -> Result<Plan, PlanError> {
    let recipes: Vec<Recipe> = recipes.into_iter().map(Recipe::normalised).collect();
    let patterns = recipes
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if r.destination == Destination::Ask {
                return Err(PlanError::NoFolder { recipe: i });
            }
            Pattern::parse(&r.pattern).map_err(|error| PlanError::Pattern { recipe: i, error })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut facts = Vec::with_capacity(items.len());
    for it in &items {
        if cancel.is_cancelled() {
            return Err(PlanError::Cancelled);
        }
        facts.push(render.facts(it));
    }
    let inputs: HashSet<PathBuf> = items.iter().map(|i| canonical(&i.path)).collect();
    let mut reserved: HashSet<PathBuf> = HashSet::new();
    let mut outputs = Vec::with_capacity(items.len() * recipes.len());
    for (i, (it, f)) in items.iter().zip(&facts).enumerate() {
        for (k, (r, pat)) in recipes.iter().zip(&patterns).enumerate() {
            let dir = folder(&it.path, r).unwrap_or_default();
            let f = match f {
                Ok(f) => f,
                Err(e) => {
                    outputs.push(Output {
                        item: i,
                        recipe: k,
                        path: dir,
                        planned: Planned::Fail(e.clone()),
                    });
                    continue;
                }
            };
            let name = name_for(pat, r, &it.path, i + 1, f);
            let first = dir.join(&name);
            if inputs.contains(&canonical(&first)) {
                outputs.push(Output {
                    item: i,
                    recipe: k,
                    path: first,
                    planned: Planned::Fail(Failure::SameAsSource(it.path.clone())),
                });
                continue;
            }
            let mut n = 1;
            let (path, planned) = loop {
                let path = dir.join(write::suffixed(&name, n));
                let c = canonical(&path);
                let ours = reserved.contains(&c) || inputs.contains(&c);
                let exists = std::fs::symlink_metadata(&path).is_ok();
                match r.collision {
                    _ if !ours && !exists => break (path, Planned::Write),
                    Collision::Overwrite if !ours => break (path, Planned::Replace),
                    Collision::Skip => break (path, Planned::Skip),
                    // Taken by another output of this run (or an original): next name.
                    _ => n += 1,
                }
            };
            if matches!(planned, Planned::Write | Planned::Replace) {
                reserved.insert(canonical(&path));
            }
            outputs.push(Output {
                item: i,
                recipe: k,
                path,
                planned,
            });
        }
    }
    Ok(Plan {
        items,
        recipes,
        facts,
        outputs,
    })
}

/// The end of a run.
#[derive(Clone, Debug, Default)]
pub struct RecipeReport {
    /// One entry per output, in plan order: the source, the recipe and what happened.
    pub results: Vec<(PathBuf, usize, Outcome)>,
    pub cancelled: bool,
    pub elapsed: Duration,
}

impl RecipeReport {
    fn count(&self, f: impl Fn(&Outcome) -> bool) -> usize {
        self.results.iter().filter(|(_, _, o)| f(o)).count()
    }
    pub fn written(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Converted { .. }))
    }
    pub fn skipped(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Skipped { .. }))
    }
    pub fn failed(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Failed(_)))
    }
    /// The folders files were written to, in order of first use.
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Vec::new();
        for (_, _, o) in &self.results {
            if let Outcome::Converted { out, .. } = o
                && let Some(d) = out.parent()
                && !v.iter().any(|x| x == d)
            {
                v.push(d.to_path_buf());
            }
        }
        v
    }
}

/// Replaces `path` with `bytes` atomically (a temporary file beside it, renamed over it).
fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.specular-tmp-{}", std::process::id()));
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Writes one finished output as planned.
fn write_out(o: &Output, bytes: &[u8], reserved: &HashSet<PathBuf>) -> Outcome {
    let dir = o.path.parent().unwrap_or(Path::new("."));
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Outcome::Failed(Failure::Write(e.to_string()));
    }
    let n = bytes.len() as u64;
    if o.planned == Planned::Replace {
        return match replace(&o.path, bytes) {
            Ok(()) => Outcome::Converted {
                out: o.path.clone(),
                bytes: n,
            },
            Err(e) => Outcome::Failed(Failure::Write(e.to_string())),
        };
    }
    let name = o
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let own = canonical(&o.path);
    let taken = |p: &Path| {
        let c = canonical(p);
        c != own && reserved.contains(&c)
    };
    match write::write_new(dir, &name, bytes, crate::Conflict::Suffix, &taken) {
        Ok(write::Written::Created(out)) => Outcome::Converted { out, bytes: n },
        Ok(write::Written::Exists(existing)) => Outcome::Skipped { existing },
        Err(e) => Outcome::Failed(Failure::Write(e.to_string())),
    }
}

/// Runs `plan`, calling `progress` after each output (from worker threads). Blocks until
/// done or cancelled; call it off the UI thread. `registry` decodes image watermarks.
pub fn execute(
    plan: &Plan,
    render: &dyn Render,
    registry: Option<&Registry>,
    threads: usize,
    cancel: &CancelToken,
    progress: &(dyn Fn(&Progress) + Sync),
) -> RecipeReport {
    let t0 = Instant::now();
    let total = plan.outputs.len();
    let mut results: Vec<Option<Outcome>> = plan
        .outputs
        .iter()
        .map(|o| match &o.planned {
            Planned::Skip => Some(Outcome::Skipped {
                existing: o.path.clone(),
            }),
            Planned::Fail(f) => Some(Outcome::Failed(f.clone())),
            _ => None,
        })
        .collect();
    // Watermarks, drawn once per recipe.
    let marks: Vec<Result<Option<Mark>, String>> = plan
        .recipes
        .iter()
        .map(|r| watermark::prepare(&r.watermark, registry).map_err(|e| e.to_string()))
        .collect();
    let reserved: HashSet<PathBuf> = plan
        .outputs
        .iter()
        .filter(|o| matches!(o.planned, Planned::Write | Planned::Replace))
        .map(|o| canonical(&o.path))
        .collect();
    let pre_done = results.iter().filter(|r| r.is_some()).count();
    let state = Mutex::new((
        pre_done,
        results
            .iter()
            .filter(|r| matches!(r, Some(Outcome::Failed(_))))
            .count(),
        results
            .iter()
            .filter(|r| matches!(r, Some(Outcome::Skipped { .. })))
            .count(),
    ));
    let report = |delta: &[&Outcome]| {
        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
        for o in delta {
            s.0 = (s.0 + 1).min(total);
            match o {
                Outcome::Failed(_) => s.1 += 1,
                Outcome::Skipped { .. } => s.2 += 1,
                _ => {}
            }
        }
        let elapsed = t0.elapsed();
        let worked = s.0 - pre_done;
        let eta = (worked > 0 && s.0 < total)
            .then(|| elapsed.mul_f64((total - s.0) as f64 / worked as f64));
        let p = Progress {
            done: s.0,
            total,
            failed: s.1,
            skipped: s.2,
            elapsed,
            eta,
        };
        drop(s);
        progress(&p);
    };
    report(&[]);

    // Per image: the outputs still to make.
    let n = plan.items.len();
    let mut todo: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (j, o) in plan.outputs.iter().enumerate() {
        if results[j].is_none() {
            todo[o.item].push(j);
        }
    }
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Outcome)>> = Mutex::new(Vec::new());
    let pool = specular_thread::rayon_pool("enc-recipe")
        .num_threads(threads.max(1))
        .start_handler(|_| place_current_thread(CoreSet::Efficiency))
        .build()
        .ok();
    let work = || {
        place_current_thread(CoreSet::Efficiency);
        loop {
            if cancel.is_cancelled() {
                return;
            }
            let i = next.fetch_add(1, Ordering::Relaxed);
            if i >= n {
                return;
            }
            let outs = &todo[i];
            if outs.is_empty() {
                continue;
            }
            let item = &plan.items[i];
            let one = || -> Vec<(usize, Outcome)> {
                let Ok(f) = &plan.facts[i] else {
                    return Vec::new();
                };
                let sizes: Vec<_> = outs
                    .iter()
                    .map(|&j| {
                        let r = &plan.recipes[plan.outputs[j].recipe];
                        (r.size, r.enlarge)
                    })
                    .collect();
                let scale = needed_scale(f.size.0, f.size.1, &sizes);
                let rendered = match render.render(item, scale, cancel) {
                    Ok(Some(r)) => r,
                    Ok(None) => return Vec::new(),
                    Err(e) => {
                        let o = Outcome::Failed(e);
                        report(&vec![&o; outs.len()]);
                        return outs.iter().map(|&j| (j, o.clone())).collect();
                    }
                };
                let mut v = Vec::new();
                for &j in outs {
                    if cancel.is_cancelled() {
                        break;
                    }
                    let o = &plan.outputs[j];
                    let r = &plan.recipes[o.recipe];
                    let outcome = match &marks[o.recipe] {
                        Err(e) => Outcome::Failed(Failure::Encode(e.clone())),
                        Ok(mark) => {
                            let t = finish::target_of(r, rendered.full);
                            let (exif, xmp) = super::meta::for_output(
                                &rendered.meta,
                                Some(&item.path),
                                t.width,
                                t.height,
                                &r.metadata,
                            );
                            match finish::finish(
                                &rendered.image,
                                rendered.full,
                                r,
                                mark.as_ref(),
                                exif.as_ref(),
                                xmp.as_deref(),
                            ) {
                                Ok(fin) if !cancel.is_cancelled() => {
                                    write_out(o, &fin.bytes, &reserved)
                                }
                                Ok(_) => break,
                                Err(e) => Outcome::Failed(Failure::Encode(e)),
                            }
                        }
                    };
                    report(&[&outcome]);
                    v.push((j, outcome));
                }
                v
            };
            let what = format!("export {}", item.path.display());
            let got = match specular_core::isolate::run(what, || match &pool {
                Some(p) => p.install(one),
                None => one(),
            }) {
                Ok(v) => v,
                Err(p) => {
                    // A decoder or encoder panic fails this image's outputs not made yet.
                    let o = Outcome::Failed(Failure::Decode(p.brief()));
                    report(&vec![&o; outs.len()]);
                    outs.iter().map(|&j| (j, o.clone())).collect()
                }
            };
            done.lock().unwrap_or_else(|e| e.into_inner()).extend(got);
        }
    };
    let workers = threads.clamp(1, n.max(1));
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|k| specular_thread::spawn_scoped(scope, format!("recipe-{k}"), work))
            .collect();
        for h in handles {
            let _ = h.join();
        }
    });
    for (j, o) in done.into_inner().unwrap_or_else(|e| e.into_inner()) {
        results[j] = Some(o);
    }
    let results: Vec<(PathBuf, usize, Outcome)> = plan
        .outputs
        .iter()
        .zip(results)
        .map(|(o, r)| {
            (
                plan.items[o.item].path.clone(),
                o.recipe,
                r.unwrap_or(Outcome::Cancelled),
            )
        })
        .collect();
    let cancelled = cancel.is_cancelled()
        || results
            .iter()
            .any(|(_, _, o)| matches!(o, Outcome::Cancelled));
    RecipeReport {
        results,
        cancelled,
        elapsed: t0.elapsed(),
    }
}
