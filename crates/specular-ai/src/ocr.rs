//! Text in images (OCR) for the app: the Text feature's installed models (the shared
//! detector, the size's recogniser, and the script add-ons) loaded on our own engine
//! (`specular_infer::ocr`), and one call that reads a picture.
//!
//! Every line found is read by each installed recogniser (the size's and every add-on's) and
//! the most confident reading kept (the size's on a tie), so a photo with Arabic and Latin
//! signs reads both once the Arabic add-on is installed. Right-to-left lines come back in
//! logical order.
//!
//! With a GPU the detector is checked against the CPU on a built-in picture when loaded
//! (`Task::Inference`, as Search's models are): a match runs it on the GPU, else the CPU.
//! Which detector runs is [`pick_detector`]'s rule: PP-OCRv6 small where it runs on the GPU,
//! else the PP-OCRv5 mobile fallback (on the GPU if it matches there, else the CPU, where it
//! is about a quarter faster than v6), else v6 on the CPU.
//!
//! With the orientation classifier installed, lines it finds upside down or isn't sure about
//! ([`BOTH_WAYS`]) and whose upright reading is unsure too ([`BOTH_WAYS_BELOW`]) are read
//! turned round as well, the more confident reading kept. Tesseract's
//! network runs on the GPU when it matches the CPU there and is faster on a test line.

use std::path::{Path, PathBuf};

use specular_core::paths::{Engine, PathSelector, Task, Trust};
use specular_infer::Error;
use specular_infer::gpu::Context;
use specular_infer::ocr::lines::{Img8, Rgba, crop_quad};
use specular_infer::ocr::tess::Tesseract;
use specular_infer::ocr::{
    BOTH_WAYS, BOTH_WAYS_BELOW, Detector, Orientation, PaddleNet, Recognizer, Runner,
};

use crate::install::Installed;
use crate::manifest::{Feature, Manifest};

/// A recogniser's files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecFiles {
    Paddle {
        weights: PathBuf,
        config: PathBuf,
        chars: PathBuf,
    },
    Tesseract(PathBuf),
}

/// The Text feature's installed files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OcrFiles {
    pub detector: PathBuf,
    pub detector_config: PathBuf,
    /// PP-OCRv5 mobile's detector (weights, config), when installed.
    pub detector_fallback: Option<(PathBuf, PathBuf)>,
    /// The text-line orientation classifier (weights, config), when installed.
    pub orientation: Option<(PathBuf, PathBuf)>,
    pub main: RecFiles,
    /// Installed add-ons, by id.
    pub addons: Vec<(String, RecFiles)>,
}

/// An installed file: role, format, path.
type File = (String, String, PathBuf);

fn rec_files(models: &[File]) -> Option<RecFiles> {
    let get = |role: &str| {
        models
            .iter()
            .find(|(r, _, _)| r == role)
            .map(|(_, _, p)| p.clone())
    };
    let w = models.iter().find(|(r, _, _)| r == "recognizer")?;
    if w.1 == "traineddata" {
        return Some(RecFiles::Tesseract(w.2.clone()));
    }
    Some(RecFiles::Paddle {
        weights: w.2.clone(),
        config: get("recognizer-config")?,
        chars: get("recognizer-chars")?,
    })
}

impl OcrFiles {
    /// The files among `installed` (the store's current models; paths relative to `root`),
    /// their roles from `manifest`. None when the Text feature is not (fully) installed.
    pub fn find(manifest: &Manifest, root: &Path, installed: &[Installed]) -> Option<OcrFiles> {
        let mut main = Vec::new();
        let mut addons: Vec<(String, Vec<File>)> = Vec::new();
        for i in installed.iter().filter(|i| i.feature == Feature::Text) {
            let m = manifest.model(&i.id)?;
            let entry = (m.role.clone(), m.runtime.format.clone(), root.join(&i.file));
            match &i.addon {
                Some(a) => match addons.iter_mut().find(|(id, _)| id == a) {
                    Some((_, v)) => v.push(entry),
                    None => addons.push((a.clone(), vec![entry])),
                },
                None => main.push(entry),
            }
        }
        let role = |r: &str| {
            main.iter()
                .find(|(x, _, _)| x == r)
                .map(|(_, _, p)| p.clone())
        };
        let mut addons: Vec<(String, RecFiles)> = addons
            .into_iter()
            .filter_map(|(id, v)| rec_files(&v).map(|r| (id, r)))
            .collect();
        addons.sort_by(|a, b| a.0.cmp(&b.0));
        let pair = |w: &str, c: &str| role(w).zip(role(c));
        Some(OcrFiles {
            detector_fallback: pair("detector-fallback", "detector-fallback-config"),
            orientation: pair("orientation", "orientation-config"),
            detector: role("detector")?,
            detector_config: role("detector-config")?,
            main: rec_files(&main)?,
            addons,
        })
    }
}

/// A line of text found in a picture.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    /// Corners in picture pixels: top-left, top-right, bottom-right, bottom-left of the
    /// line as read (a turned line's "top" is its reading top).
    pub quad: [[f32; 2]; 4],
    pub text: String,
    pub confidence: f32,
    /// Which recogniser read it: `None` the size's, else the add-on's id.
    pub addon: Option<String>,
}

impl TextLine {
    /// Whether the line reads right to left (its first strong letter is Hebrew or Arabic).
    pub fn rtl(&self) -> bool {
        self.text
            .chars()
            .find(|c| c.is_alphabetic())
            .is_some_and(specular_infer::ocr::ctc::is_rtl)
    }
}

enum Rec {
    Paddle(Box<Recognizer>),
    /// With the GPU context when it reads there.
    Tess(Box<Tesseract>, Option<Context>),
}

/// Which text detector runs, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetPick {
    /// PP-OCRv6 small.
    V6 { gpu: bool },
    /// PP-OCRv5 mobile, the fallback.
    V5 { gpu: bool },
}

/// The detector rule: v6 where it runs on the GPU (`v6_gpu`: its check there passed); else
/// v5 when installed (`v5_gpu`: its own check passed), on the GPU or the CPU; else v6 on the
/// CPU. A check is `None` when there is no (trusted) GPU.
pub fn pick_detector(v6_gpu: Option<bool>, v5_gpu: Option<bool>, have_v5: bool) -> DetPick {
    if v6_gpu == Some(true) {
        return DetPick::V6 { gpu: true };
    }
    if have_v5 {
        return DetPick::V5 {
            gpu: v5_gpu == Some(true),
        };
    }
    DetPick::V6 { gpu: false }
}

fn open_rec(f: &RecFiles) -> Result<Rec, Error> {
    Ok(match f {
        RecFiles::Paddle {
            weights,
            config,
            chars,
        } => Rec::Paddle(Box::new(Recognizer::open(config, weights, chars, true)?)),
        RecFiles::Tesseract(p) => Rec::Tess(Box::new(Tesseract::open(p)?), None),
    })
}

/// The loaded models.
pub struct Ocr {
    runner: Runner,
    threads: usize,
    det: Detector,
    pick: DetPick,
    orient: Option<Orientation>,
    recs: Vec<(Option<String>, Rec)>,
    unreadable: usize,
}

/// Lowest confidence a line is kept at.
pub const MIN_CONFIDENCE: f32 = 0.5;

/// A kept line read with less confidence than this is unsure: its photo is read again when
/// the orientation classifier or another detector comes (`low` in [`reading_tokens`]).
pub const LOW_CONFIDENCE: f32 = 0.8;

/// A picture with a few dark bars on light, for the GPU check.
fn probe() -> (usize, usize, Vec<u8>) {
    let (w, h) = (160usize, 96usize);
    let mut px = vec![235u8; w * h * 4];
    for (y0, x0, x1) in [(14usize, 12usize, 140usize), (52, 30, 120)] {
        for y in y0..y0 + 18 {
            for x in x0..x1 {
                if (x / 5) % 3 != 0 && (4..14).contains(&(y - y0)) {
                    let i = (y * w + x) * 4;
                    px[i..i + 3].copy_from_slice(&[25, 25, 25]);
                }
            }
        }
    }
    (w, h, px)
}

impl Ocr {
    /// Loads the models in `files`; with `gpu`, checks the detector there against the CPU
    /// first (the outcome goes to `selector`).
    pub fn load(
        files: &OcrFiles,
        gpu: Option<&Context>,
        selector: &PathSelector,
        threads: usize,
    ) -> Result<Ocr, Error> {
        let det = Detector::new(PaddleNet::open(
            &files.detector_config,
            &files.detector,
            true,
        )?)?;
        let fallback = files.detector_fallback.as_ref().and_then(|(w, c)| {
            PaddleNet::open(c, w, true)
                .and_then(Detector::new)
                .map_err(|e| {
                    specular_log::warn!(target: specular_log::JOBS, "ai: text fallback detector: {e}")
                })
                .ok()
        });
        let orient = files.orientation.as_ref().and_then(|(w, c)| {
            Orientation::open(c, w, true)
                .map_err(|e| {
                    specular_log::warn!(target: specular_log::JOBS, "ai: text orientation: {e}")
                })
                .ok()
        });
        let mut recs = vec![(None, open_rec(&files.main)?)];
        for (id, f) in &files.addons {
            match open_rec(f) {
                Ok(r) => recs.push((Some(id.clone()), r)),
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS, "ai: text add-on {id}: {e}")
                }
            }
        }
        let threads = threads.max(1);
        let mut det = det;
        let trusted = gpu.filter(|_| {
            let path = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
            selector.trust(Task::Inference, path) != Trust::Blocked
        });
        let v6_gpu = trusted.map(|ctx| check_detector(&mut det, "v6", ctx, selector, threads));
        let mut fallback = fallback;
        let v5_gpu = match (&mut fallback, trusted) {
            (Some(f), Some(ctx)) if v6_gpu != Some(true) => {
                Some(check_detector(f, "v5", ctx, selector, threads))
            }
            _ => None,
        };
        let pick = pick_detector(v6_gpu, v5_gpu, fallback.is_some());
        let (det, gpu_on) = match (pick, fallback) {
            (DetPick::V5 { gpu }, Some(f)) => (f, gpu),
            (DetPick::V6 { gpu }, _) | (DetPick::V5 { gpu }, None) => (det, gpu),
        };
        specular_log::info!(target: specular_log::JOBS, "ai: text detector {pick:?}");
        let runner = match (gpu_on, trusted) {
            (true, Some(ctx)) => Runner::Gpu(ctx.clone()),
            _ => Runner::Cpu(threads),
        };
        let mut ocr = Ocr {
            runner,
            threads,
            det,
            pick,
            orient,
            recs,
            unreadable: 0,
        };
        if let Some(ctx) = trusted {
            ocr.check_tesseract(ctx);
        }
        Ok(ocr)
    }

    /// Whether the detector runs on the GPU.
    pub fn on_gpu(&self) -> bool {
        matches!(self.runner, Runner::Gpu(_))
    }

    /// Which detector runs, and where.
    pub fn detector(&self) -> DetPick {
        self.pick
    }

    /// Lines the last [`Ocr::read`] found but could not read (below [`MIN_CONFIDENCE`]).
    pub fn unreadable(&self) -> usize {
        self.unreadable
    }

    /// Whether lines are checked for being upside down.
    pub fn orientation(&self) -> bool {
        self.orient.is_some()
    }

    /// Tesseract on the GPU where it matches the CPU on a test line and reads a batch of
    /// them faster there.
    fn check_tesseract(&mut self, ctx: &Context) {
        let (w, h, px) = probe();
        let img = Rgba {
            width: w,
            height: h,
            data: &px,
        };
        let Some(line) = crop_quad(
            &img,
            [[8.0, 10.0], [150.0, 10.0], [150.0, 36.0], [8.0, 36.0]],
        ) else {
            return;
        };
        let lines: Vec<Img8> = (0..8).map(|_| line.clone()).collect();
        let threads = self.threads;
        for (id, rec) in &mut self.recs {
            let Rec::Tess(t, on) = rec else { continue };
            let gpu = t.probs_gpu(ctx, &lines[..1], threads);
            let cpu = t.probs(&line);
            let worst = match (&gpu, &cpu) {
                (Ok((g, _)), Ok((c, _))) => g[0].as_ref().map_or(f32::MAX, |g| {
                    g.iter()
                        .zip(c)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0f32, f32::max)
                }),
                _ => f32::MAX,
            };
            if worst > 1e-3 {
                specular_log::info!(target: specular_log::JOBS,
                    "ai: text add-on {id:?} on the GPU differs from the CPU ({worst:.2e}): CPU");
                continue;
            }
            // Warmed up above (graphs built); time a batch each way.
            let t0 = std::time::Instant::now();
            let ok = t.probs_gpu(ctx, &lines, threads).is_ok();
            let tg = t0.elapsed();
            let t0 = std::time::Instant::now();
            let _ = t.read(&lines, threads);
            let tc = t0.elapsed();
            let use_gpu = ok && tg < tc;
            specular_log::info!(target: specular_log::JOBS,
                "ai: text add-on {id:?}: GPU {tg:?}, CPU {tc:?} ({threads} threads): {}",
                if use_gpu { "GPU" } else { "CPU" });
            if use_gpu {
                *on = Some(ctx.clone());
            }
        }
    }

    /// Reads `crops` with every recogniser, keeping each line's most confident reading in
    /// `best`: crop k is line k, or line `of[k]` read turned round. False when cancelled.
    fn read_all(
        &mut self,
        crops: &[Img8],
        of: Option<&[usize]>,
        best: &mut [Best],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, Error> {
        for (id, rec) in &mut self.recs {
            if cancelled() {
                return Ok(false);
            }
            let read = match rec {
                Rec::Paddle(r) => r.read(&self.runner, crops)?,
                Rec::Tess(t, Some(ctx)) => match t.read_gpu(ctx, crops, self.threads) {
                    Ok(r) => r,
                    Err(e) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: text on the GPU: {e}: CPU");
                        t.read(crops, self.threads)
                    }
                },
                Rec::Tess(t, None) => t.read(crops, self.threads),
            };
            for (k, d) in read.into_iter().enumerate() {
                let line = of.map_or(k, |o| o[k]);
                let slot = &mut best[line];
                let better = match slot {
                    None => true,
                    Some((c, ..)) => d.confidence > *c,
                };
                if better && !d.text.trim().is_empty() {
                    *slot = Some((d.confidence, id.clone(), d.text, of.is_some()));
                }
            }
        }
        Ok(true)
    }

    /// Reads the text in an RGBA picture (`width × height`). `cancelled` is asked between
    /// the steps.
    pub fn read(
        &mut self,
        width: usize,
        height: usize,
        rgba: &[u8],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<TextLine>, Error> {
        let img = Rgba {
            width,
            height,
            data: rgba,
        };
        self.unreadable = 0;
        let boxes = self.det.detect(&self.runner, &img)?;
        if cancelled() || boxes.is_empty() {
            return Ok(Vec::new());
        }
        let crops: Vec<Img8> = boxes
            .iter()
            .map(|b| {
                crop_quad(&img, b.pts).unwrap_or(Img8 {
                    width: 1,
                    height: 1,
                    channels: 3,
                    data: vec![255; 3],
                })
            })
            .collect();
        let n = crops.len();
        // The classifier's probability of each line being upside down.
        let flip = match &mut self.orient {
            Some(o) => o.upside_down(&self.runner, &crops)?,
            None => Vec::new(),
        };
        // Per line: (confidence, add-on, text, read turned).
        let mut best: Vec<Best> = vec![None; n];
        if !self.read_all(&crops, None, &mut best, cancelled)? {
            return Ok(Vec::new());
        }
        // Lines that may be upside down and weren't read confidently are also read turned
        // round.
        let turned_of: Vec<usize> = (0..flip.len())
            .filter(|&i| {
                flip[i] >= BOTH_WAYS && best[i].as_ref().is_none_or(|b| b.0 < BOTH_WAYS_BELOW)
            })
            .collect();
        if !turned_of.is_empty() {
            let turned: Vec<Img8> = turned_of.iter().map(|&i| crops[i].rot180()).collect();
            if !self.read_all(&turned, Some(&turned_of), &mut best, cancelled)? {
                return Ok(Vec::new());
            }
        }
        self.unreadable = best
            .iter()
            .filter(|b| b.as_ref().is_none_or(|b| b.0 < MIN_CONFIDENCE))
            .count();
        Ok(boxes
            .iter()
            .zip(best)
            .filter_map(|(b, r)| {
                let (confidence, addon, text, turned) = r?;
                let p = b.pts;
                (confidence >= MIN_CONFIDENCE).then(|| TextLine {
                    // A line read turned round: its reading top is the box's bottom.
                    quad: if turned { [p[2], p[3], p[0], p[1]] } else { p },
                    text: text.trim().to_owned(),
                    confidence,
                    addon,
                })
            })
            .collect())
    }
}

/// Per line: the most confident reading so far (confidence, add-on, text, read turned).
type Best = Option<(f32, Option<String>, String, bool)>;

/// The scripts of `text`'s letters (ISO 15924 codes, lower case), in the order first seen.
pub fn scripts_of(text: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        let u = c as u32;
        let s = match u {
            0x0041..=0x024F | 0x1E00..=0x1EFF => "latn",
            0x0370..=0x03FF | 0x1F00..=0x1FFF => "grek",
            0x0400..=0x052F => "cyrl",
            0x0590..=0x05FF | 0xFB1D..=0xFB4F => "hebr",
            0x0600..=0x06FF | 0x0750..=0x077F | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => "arab",
            0x0900..=0x097F => "deva",
            0x0B80..=0x0BFF => "taml",
            0x0C00..=0x0C7F => "telu",
            0x0E00..=0x0E7F => "thai",
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => "hang",
            0x3040..=0x30FF => "kana",
            0x3400..=0x4DBF | 0x4E00..=0x9FFF => "hani",
            _ => continue,
        };
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// The scripts a text add-on reads.
pub fn addon_scripts(id: &str) -> &'static [&'static str] {
    match id {
        "arabic" => &["arab"],
        "korean" => &["hang"],
        "cyrillic" | "eslav" => &["cyrl"],
        "latin" => &["latn"],
        "greek" => &["grek"],
        "thai" => &["thai"],
        "devanagari" => &["deva"],
        "tamil" => &["taml"],
        "telugu" => &["telu"],
        "hebrew" => &["hebr"],
        _ => &[],
    }
}

/// What a reading found, as the catalogue keeps it (space-separated): each script read, then
/// `addon:<key>` for each add-on that read a line (`addon_key` gives the add-on's part of the
/// models' key), then `low` when a line was read with less than [`LOW_CONFIDENCE`], then `?`
/// when `unreadable` lines were found.
pub fn reading_tokens(
    lines: &[TextLine],
    unreadable: usize,
    addon_key: &dyn Fn(&str) -> Option<String>,
) -> String {
    let mut t: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !t.contains(&s) {
            t.push(s);
        }
    };
    for l in lines {
        for s in scripts_of(&l.text) {
            push(s.to_owned());
        }
    }
    for l in lines {
        if let Some(k) = l.addon.as_deref().and_then(addon_key) {
            push(format!("addon:{k}"));
        }
    }
    if lines.iter().any(|l| l.confidence < LOW_CONFIDENCE) {
        push("low".to_owned());
    }
    if unreadable > 0 {
        push("?".to_owned());
    }
    t.join(" ")
}

/// What a part of the models' key after the detector and the size's recogniser is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPart {
    /// A script add-on, with the scripts it reads.
    Addon(&'static [&'static str]),
    /// The text-line orientation classifier.
    Orientation,
    /// An installed text detector (the first part of a key).
    Detector,
}

/// Whether a reading stored under models' key `old` (what it found: `tokens`, see
/// [`reading_tokens`]) stands for the models of key `new`: keys are `+`-joined parts, the
/// detector that ran and the size's recogniser first, then the orientation classifier when
/// it ran and one per add-on. True when the recogniser is the same and none of the changes
/// matters to it: no removed add-on read one of its lines; no added add-on reads a script it
/// has (or, when it had lines it couldn't read, nothing was added); and, when the
/// orientation classifier was added or another installed detector runs (PP-OCRv5 for v6 or
/// back), it had no unreadable or unsure lines (`?`, `low`: what turning a line or finding
/// its box better can mend). A removed classifier never needs a new reading. `part` says
/// what a part of either key is (a part it doesn't know, like a removed add-on, is an
/// add-on reading nothing).
pub fn reading_still_good(
    old: &str,
    new: &str,
    tokens: &str,
    part: &dyn Fn(&str) -> KeyPart,
) -> bool {
    let (o, n): (Vec<&str>, Vec<&str>) = (old.split('+').collect(), new.split('+').collect());
    if o.len() < 2 || n.len() < 2 || o[1] != n[1] {
        return false;
    }
    let tok: Vec<&str> = tokens.split_whitespace().collect();
    let unsure = tok.contains(&"?") || tok.contains(&"low");
    if o[0] != n[0] {
        let swapped = part(o[0]) == KeyPart::Detector && part(n[0]) == KeyPart::Detector;
        if !swapped || unsure {
            return false;
        }
    }
    let removed = o[2..].iter().filter(|p| !n[2..].contains(p));
    for r in removed {
        if tok.iter().any(|t| t.strip_prefix("addon:") == Some(*r)) {
            return false;
        }
    }
    let added: Vec<KeyPart> = n[2..]
        .iter()
        .filter(|p| !o[2..].contains(p))
        .map(|p| part(p))
        .collect();
    if added.is_empty() {
        return true;
    }
    if tok.contains(&"?") {
        return false;
    }
    !added.iter().any(|a| match a {
        KeyPart::Addon(scripts) => scripts.iter().any(|s| tok.contains(s)),
        KeyPart::Orientation => unsure,
        KeyPart::Detector => true,
    })
}

/// Checks a detector on the GPU against the CPU on the built-in picture; the outcome goes
/// to `sel`. Whether it matched.
fn check_detector(
    det: &mut Detector,
    name: &str,
    ctx: &Context,
    sel: &PathSelector,
    threads: usize,
) -> bool {
    let (w, h, px) = probe();
    let img = Rgba {
        width: w,
        height: h,
        data: &px,
    };
    let cpu = det.prob_map_at(&Runner::Cpu(threads), &img, w, h);
    let gpu = det.prob_map_at(&Runner::Gpu(ctx.clone()), &img, w, h);
    match (cpu, gpu) {
        (Ok((a, ..)), Ok((b, ..))) => {
            let worst = a
                .iter()
                .zip(&b)
                .map(|(x, y)| (x - y).abs())
                .fold(0f32, f32::max);
            let ok = worst < 2e-3;
            sel.verified(Task::Inference, Engine::Gpu, ok);
            specular_log::info!(target: specular_log::JOBS,
                "ai: text detector {name} on the GPU {} the CPU (max difference {worst:.2e})",
                if ok { "matches" } else { "differs from" });
            ok
        }
        (_, Err(e)) => {
            if matches!(e, Error::Gpu(_)) {
                sel.block(Task::Inference, Engine::Gpu, &format!("text detector: {e}"));
            }
            specular_log::warn!(target: specular_log::JOBS, "ai: text detector {name} on the GPU: {e}");
            false
        }
        (Err(e), _) => {
            specular_log::warn!(target: specular_log::JOBS, "ai: text detector {name}: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_and_tokens() {
        assert_eq!(scripts_of("Exit 12"), ["latn"]);
        assert_eq!(scripts_of("12 שלום Café"), ["hebr", "latn"]);
        assert_eq!(scripts_of("東京 タワー"), ["hani", "kana"]);
        let l = |t: &str, addon: Option<&str>| TextLine {
            quad: [[0.0; 2]; 4],
            text: t.into(),
            confidence: 0.9,
            addon: addon.map(str::to_owned),
        };
        let key = |id: &str| (id == "hebrew").then(|| "tesseract-heb-1".to_owned());
        let t = reading_tokens(&[l("Exit", None), l("יציאה", Some("hebrew"))], 0, &key);
        assert_eq!(t, "latn hebr addon:tesseract-heb-1");
        assert_eq!(reading_tokens(&[], 2, &key), "?");
    }

    #[test]
    fn readings_kept_when_an_addon_changes() {
        let parts = |p: &str| {
            KeyPart::Addon(if p.contains("arabic") {
                &["arab"]
            } else if p.contains("heb") {
                &["hebr"]
            } else {
                &[]
            })
        };
        let base = "det-1+rec-1";
        let ar = "det-1+rec-1+ar-arabic-1";
        let ar_he = "det-1+rec-1+ar-arabic-1+tess-heb-1";
        // Arabic added: a Latin-only photo stays; one with Arabic, or unreadable lines, not.
        assert!(reading_still_good(base, ar, "latn", &parts));
        assert!(!reading_still_good(base, ar, "latn arab", &parts));
        assert!(!reading_still_good(base, ar, "latn ?", &parts));
        assert!(reading_still_good(base, ar, "", &parts));
        // Hebrew removed: photos it read are read again, the others stay.
        assert!(reading_still_good(
            ar_he,
            ar,
            "latn addon:ar-arabic-1",
            &parts
        ));
        assert!(!reading_still_good(
            ar_he,
            ar,
            "hebr addon:tess-heb-1",
            &parts
        ));
        // Removing never needs a re-read for unreadable lines.
        assert!(reading_still_good(ar_he, ar, "?", &parts));
        // Another detector or size: everything again.
        assert!(!reading_still_good("det-2+rec-1", base, "latn", &parts));
        assert!(!reading_still_good("det-1+rec-2", base, "latn", &parts));
        // Rows from before the tokens were kept ("?"): read again when something is added.
        assert!(!reading_still_good(base, ar, "?", &parts));
    }

    #[test]
    fn readings_kept_when_the_classifier_or_detector_changes() {
        let parts = |p: &str| {
            if p == "orient-1" {
                KeyPart::Orientation
            } else if p.starts_with("det-") {
                KeyPart::Detector
            } else if p.contains("arabic") {
                KeyPart::Addon(&["arab"])
            } else {
                KeyPart::Addon(&[])
            }
        };
        let base = "det-v6+rec-1+ar-arabic-1";
        let turned = "det-v6+rec-1+orient-1+ar-arabic-1";
        // The classifier installed: only photos with unreadable or unsure lines are read
        // again; nothing found, or lines read surely, stay.
        assert!(reading_still_good(base, turned, "", &parts));
        assert!(reading_still_good(base, turned, "latn", &parts));
        assert!(reading_still_good(
            base,
            turned,
            "arab addon:ar-arabic-1",
            &parts
        ));
        assert!(!reading_still_good(base, turned, "?", &parts));
        assert!(!reading_still_good(base, turned, "latn low", &parts));
        assert!(!reading_still_good(base, turned, "latn low ?", &parts));
        // Removed again: nothing is read again.
        assert!(reading_still_good(turned, base, "latn", &parts));
        assert!(reading_still_good(turned, base, "?", &parts));
        // With an add-on at the same time: both rules.
        let both = "det-v6+rec-1+orient-1+ar-arabic-1+tess-heb-1";
        assert!(reading_still_good(base, both, "", &parts));
        assert!(!reading_still_good(base, both, "low", &parts));
        // PP-OCRv5 running instead of v6 (or back): the same rule.
        let v5 = "det-v5+rec-1+ar-arabic-1";
        assert!(reading_still_good(base, v5, "", &parts));
        assert!(reading_still_good(base, v5, "latn", &parts));
        assert!(!reading_still_good(base, v5, "latn ?", &parts));
        assert!(!reading_still_good(base, v5, "low", &parts));
        assert!(reading_still_good(v5, base, "latn", &parts));
        // A detector that isn't installed any more, or another size's recogniser:
        // everything again.
        let other = |p: &str| {
            if p == "det-v5" {
                KeyPart::Addon(&[])
            } else {
                parts(p)
            }
        };
        assert!(!reading_still_good(v5, base, "latn", &other));
        assert!(!reading_still_good(
            base,
            "det-v6+rec-2+ar-arabic-1",
            "",
            &parts
        ));
    }

    #[test]
    fn unsure_lines_are_marked() {
        let l = |t: &str, c: f32| TextLine {
            quad: [[0.0; 2]; 4],
            text: t.into(),
            confidence: c,
            addon: None,
        };
        let none = |_: &str| None;
        assert_eq!(reading_tokens(&[l("Exit", 0.95)], 0, &none), "latn");
        assert_eq!(
            reading_tokens(&[l("Exit", 0.95), l("Exlt", 0.6)], 0, &none),
            "latn low"
        );
        assert_eq!(reading_tokens(&[l("Exlt", 0.6)], 2, &none), "latn low ?");
    }

    #[test]
    fn detector_rule() {
        // v6 where it runs on the GPU.
        assert_eq!(
            pick_detector(Some(true), None, true),
            DetPick::V6 { gpu: true }
        );
        assert_eq!(
            pick_detector(Some(true), None, false),
            DetPick::V6 { gpu: true }
        );
        // v6 fails on the GPU: v5 there if it matches, else v5 on the CPU.
        assert_eq!(
            pick_detector(Some(false), Some(true), true),
            DetPick::V5 { gpu: true }
        );
        assert_eq!(
            pick_detector(Some(false), Some(false), true),
            DetPick::V5 { gpu: false }
        );
        // No GPU: v5 on the CPU (faster there), v6 when v5 isn't installed.
        assert_eq!(pick_detector(None, None, true), DetPick::V5 { gpu: false });
        assert_eq!(pick_detector(None, None, false), DetPick::V6 { gpu: false });
        assert_eq!(
            pick_detector(Some(false), None, false),
            DetPick::V6 { gpu: false }
        );
    }

    #[test]
    fn files_by_role_from_the_builtin_manifest() {
        let m = Manifest::parse(crate::manifest::BUILTIN).unwrap();
        let inst = |id: &str, addon: Option<&str>| {
            let model = m.model(id).unwrap();
            Installed {
                feature: Feature::Text,
                tier: addon.is_none().then_some(crate::Tier::Standard),
                id: id.into(),
                version: "1".into(),
                sha256: model.sha256,
                size: model.size,
                file: format!("models/{id}-1/model.{}", model.runtime.format),
                unverified: false,
                addon: addon.map(str::to_owned),
            }
        };
        let mut v = vec![
            inst("ppocrv6-small-det", None),
            inst("ppocrv6-small-det-config", None),
            inst("ppocrv6-small-rec", None),
            inst("ppocrv6-small-rec-config", None),
        ];
        let root = Path::new("/ai");
        // The character list missing: not complete.
        assert!(OcrFiles::find(&m, root, &v).is_none());
        v.push(inst("ppocrv6-small-rec-chars", None));
        v.push(inst("tesseract-heb", Some("hebrew")));
        v.push(inst("ppocrv5-arabic-rec", Some("arabic")));
        let f = OcrFiles::find(&m, root, &v).unwrap();
        assert_eq!(
            f.detector,
            root.join("models/ppocrv6-small-det-1/model.safetensors")
        );
        assert!(matches!(f.main, RecFiles::Paddle { .. }));
        // Hebrew is whole; Arabic lacks its config and characters, so it is left out.
        assert_eq!(f.addons.len(), 1);
        assert_eq!(f.addons[0].0, "hebrew");
        assert!(matches!(f.addons[0].1, RecFiles::Tesseract(_)));
    }

    #[test]
    fn rtl_lines() {
        let l = |t: &str| TextLine {
            quad: [[0.0; 2]; 4],
            text: t.into(),
            confidence: 1.0,
            addon: None,
        };
        assert!(l("12 שלום").rtl());
        assert!(l("مرحبا").rtl());
        assert!(!l("Hello").rtl());
    }
}
