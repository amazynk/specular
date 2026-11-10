//! Flat-field and dark-frame calibration (the idea from RawTherapee, written
//! from its public description).
//!
//! - **Dark frame**: a raw shot with the cap on at the same ISO and exposure. Normalised
//!   with the picture's own black and white levels, it is the thermal signal and hot
//!   pixels above black; it is subtracted per pixel. At the same ISO but another exposure
//!   time it is scaled by the ratio of the times (thermal signal grows with time:
//!   [`dark_scale`]).
//! - **Flat frame**: a raw of an even, unfocused light (a white wall, a light box) with
//!   the same lens and aperture. Normalised per colour to its centre, it is the falloff
//!   (vignetting, lens cast) and the shadows of dust; the picture is divided by it.
//!   **Vignetting only** keeps just its smooth falloff: per colour, a surface fitted
//!   through medians of blocks of [`BLUR_BLOCK`] pixels (dust drops out).
//!
//! Both act on the CFA, after black and white and before white balance, as
//! `v = (v − dark) / flat` (the raw front end's prepare step). On the GPU each pixel of the
//! crop holds one word: `1/flat` and `dark` as two f16.
//!
//! **Automatic choice** ([`best_match`]): frames in the calibration folder (Settings; never
//! fetched) whose path names "flat" or "dark", from the same camera at the same size; flats
//! by lens then aperture, darks by ISO then exposure time.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use specular_raw::SensorData;
use specular_raw::sensor::Layout;

/// The flat's smoothing block in pixels (vignetting only).
pub const BLUR_BLOCK: u32 = 32;
/// The smallest flat value used (darker pixels are left as they are).
const MIN_FLAT: f32 = 0.05;

/// Why frames cannot calibrate a picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalibError {
    /// The frame's sensor size, crop or colour layout differs from the picture's.
    Mismatch,
    /// The flat frame is black where the picture's centre is.
    DarkFlat,
}

impl std::fmt::Display for CalibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CalibError::Mismatch => "the frame is from another camera or size",
            CalibError::DarkFlat => "the flat frame is too dark",
        })
    }
}

impl std::error::Error for CalibError {}

/// The calibration of one picture: per crop pixel `pack2x16float(1/flat, dark)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Calibration {
    pub width: u32,
    pub height: u32,
    pub words: Vec<u32>,
}

fn same_sensor(a: &SensorData, b: &SensorData) -> bool {
    let layout = |l: &Layout| match l {
        Layout::Bayer(p) => (0, p.to_vec()),
        Layout::Pattern(c) => (1, c.colors.to_vec()),
        Layout::Linear => (2, Vec::new()),
        Layout::Mono => (3, Vec::new()),
    };
    a.width == b.width
        && a.height == b.height
        && a.cpp == b.cpp
        && a.cpp == 1
        && a.crop == b.crop
        && layout(&a.meta.layout) == layout(&b.meta.layout)
}

/// The colour of crop pixel (x, y).
fn colour(s: &SensorData, x: u32, y: u32) -> usize {
    usize::from(s.meta.layout.color(s.crop.x + x, s.crop.y + y)).min(2)
}

/// Crop pixel (x, y) of `f` normalised with `by`'s black and white levels.
fn norm(f: &SensorData, by: &SensorData, x: u32, y: u32) -> f32 {
    let (sx, sy) = (f.crop.x + x, f.crop.y + y);
    let b = by.meta.black.at(sx, sy, 0);
    (f32::from(f.sample(sx, sy, 0)) - b) / (by.meta.white - b).max(1.0)
}

/// Builds the calibration of `target` from a flat and a dark frame (either may be absent).
pub fn build(
    target: &SensorData,
    flat: Option<&SensorData>,
    dark: Option<&SensorData>,
    blur: bool,
) -> Result<Calibration, CalibError> {
    build_scaled(target, flat, dark, blur, 1.0)
}

/// How much a dark frame's signal is scaled for a picture: the ratio of the exposure times
/// when both are known and the ISO is the same (within a third of a stop), else 1. Kept
/// within 1/64 to 64 (beyond that the frame is not a fit, scaled or not).
pub fn dark_scale(picture: &FrameInfo, dark: &FrameInfo) -> f32 {
    let same_iso = match (picture.iso, dark.iso) {
        (Some(a), Some(b)) if a > 0.0 && b > 0.0 => (a / b).log2().abs() <= 1.0 / 3.0 + 1e-3,
        _ => false,
    };
    match (picture.exposure, dark.exposure) {
        (Some(t), Some(td)) if same_iso && t > 0.0 && td > 0.0 => (t / td).clamp(1.0 / 64.0, 64.0),
        _ => 1.0,
    }
}

/// [`build`] with the dark frame's signal times `dark_scale` ([`dark_scale`]).
pub fn build_scaled(
    target: &SensorData,
    flat: Option<&SensorData>,
    dark: Option<&SensorData>,
    blur: bool,
    dark_scale: f32,
) -> Result<Calibration, CalibError> {
    for f in flat.iter().chain(dark.iter()) {
        if !same_sensor(target, f) {
            return Err(CalibError::Mismatch);
        }
    }
    let (w, h) = (target.crop.w, target.crop.h);
    let n = w as usize * h as usize;
    let mut dark_v = vec![0f32; n];
    if let Some(d) = dark {
        for y in 0..h {
            for x in 0..w {
                dark_v[(y * w + x) as usize] = norm(d, target, x, y) * dark_scale;
            }
        }
    }
    let mut inv = vec![1f32; n];
    if let Some(f) = flat {
        let mut v: Vec<f32> = (0..n)
            .map(|i| {
                let (x, y) = (i as u32 % w, i as u32 / w);
                norm(f, f, x, y)
            })
            .collect();
        if blur {
            v = smooth(target, &v, w, h);
        }
        // Per-colour means over the centre quarter.
        let (mut sum, mut cnt) = ([0f64; 3], [0f64; 3]);
        for y in h * 3 / 8..h * 5 / 8 {
            for x in w * 3 / 8..w * 5 / 8 {
                let c = colour(target, x, y);
                sum[c] += f64::from(v[(y * w + x) as usize]);
                cnt[c] += 1.0;
            }
        }
        let mean: [f32; 3] = std::array::from_fn(|c| {
            if cnt[c] > 0.0 {
                (sum[c] / cnt[c]) as f32
            } else {
                0.0
            }
        });
        if mean.iter().any(|&m| m <= 1e-4) {
            return Err(CalibError::DarkFlat);
        }
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let g = v[i] / mean[colour(target, x, y)];
                inv[i] = if g >= MIN_FLAT { 1.0 / g } else { 1.0 };
            }
        }
    }
    let words = inv
        .iter()
        .zip(&dark_v)
        .map(|(&g, &d)| {
            u32::from(half::f16::from_f32(g).to_bits())
                | (u32::from(half::f16::from_f32(d).to_bits()) << 16)
        })
        .collect();
    Ok(Calibration {
        width: w,
        height: h,
        words,
    })
}

/// Degree of the falloff surface (vignetting only).
const SURFACE_DEGREE: usize = 6;

/// Monomials `x^i y^j`, `i + j ≤ SURFACE_DEGREE`.
fn monomials(x: f64, y: f64, out: &mut Vec<f64>) {
    out.clear();
    for d in 0..=SURFACE_DEGREE {
        for i in 0..=d {
            out.push(x.powi(i as i32) * y.powi((d - i) as i32));
        }
    }
}

/// Weighted least squares of a surface through `pts` (x, y, value, weight).
fn fit_surface(pts: &[(f64, f64, f64, f64)]) -> Option<Vec<f64>> {
    let mut m = Vec::new();
    monomials(0.0, 0.0, &mut m);
    let n = m.len();
    let mut a = vec![0f64; n * (n + 1)];
    for &(x, y, v, w) in pts {
        monomials(x, y, &mut m);
        for r in 0..n {
            for c in 0..n {
                a[r * (n + 1) + c] += w * m[r] * m[c];
            }
            a[r * (n + 1) + n] += w * m[r] * v;
        }
    }
    // Gauss–Jordan with partial pivoting.
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| {
            a[i * (n + 1) + c]
                .abs()
                .total_cmp(&a[j * (n + 1) + c].abs())
        })?;
        if a[p * (n + 1) + c].abs() < 1e-14 {
            return None;
        }
        for k in 0..=n {
            a.swap(c * (n + 1) + k, p * (n + 1) + k);
        }
        for r in 0..n {
            if r != c {
                let f = a[r * (n + 1) + c] / a[c * (n + 1) + c];
                for k in c..=n {
                    a[r * (n + 1) + k] -= f * a[c * (n + 1) + k];
                }
            }
        }
    }
    Some(
        (0..n)
            .map(|i| a[i * (n + 1) + n] / a[i * (n + 1) + i])
            .collect(),
    )
}

/// Per colour: block medians over [`BLUR_BLOCK`]² (dust and specks drop out), a smooth
/// surface of degree [`SURFACE_DEGREE`] fitted through them (twice, the second time without
/// blocks far off the first fit), sampled every 8 pixels and interpolated.
fn smooth(s: &SensorData, v: &[f32], w: u32, h: u32) -> Vec<f32> {
    let bs = BLUR_BLOCK;
    let (bw, bh) = (w.div_ceil(bs), h.div_ceil(bs));
    let (sx, sy) = (2.0 / f64::from(w.max(2) - 1), 2.0 / f64::from(h.max(2) - 1));
    let norm = |x: f64, y: f64| (x * sx - 1.0, y * sy - 1.0);
    let mut pts: [Vec<(f64, f64, f64, f64)>; 3] = Default::default();
    let mut vals: [Vec<f32>; 3] = Default::default();
    for by in 0..bh {
        for bx in 0..bw {
            for c in &mut vals {
                c.clear();
            }
            let (x0, x1) = (bx * bs, ((bx + 1) * bs).min(w));
            let (y0, y1) = (by * bs, ((by + 1) * bs).min(h));
            for y in y0..y1 {
                for x in x0..x1 {
                    vals[colour(s, x, y)].push(v[(y * w + x) as usize]);
                }
            }
            let (cx, cy) = norm(
                f64::from(x0 + x1) / 2.0 - 0.5,
                f64::from(y0 + y1) / 2.0 - 0.5,
            );
            for (c, vs) in vals.iter_mut().enumerate() {
                if vs.len() < 4 {
                    continue;
                }
                let (k, wgt) = (vs.len() / 2, vs.len() as f64);
                let (_, m, _) = vs.select_nth_unstable_by(k, f32::total_cmp);
                pts[c].push((cx, cy, f64::from(*m), wgt));
            }
        }
    }
    let mut mono = Vec::new();
    let eval = |coef: &[f64], x: f64, y: f64, mono: &mut Vec<f64>| -> f64 {
        monomials(x, y, mono);
        coef.iter().zip(mono.iter()).map(|(a, b)| a * b).sum()
    };
    let surfaces: Vec<Option<Vec<f64>>> = pts
        .iter()
        .map(|p| {
            let first = fit_surface(p)?;
            let mut m = Vec::new();
            let mut r: Vec<f64> = p
                .iter()
                .map(|q| (q.2 - eval(&first, q.0, q.1, &mut m)).abs())
                .collect();
            r.sort_by(f64::total_cmp);
            let mad = r[r.len() / 2].max(1e-6);
            let kept: Vec<_> = p
                .iter()
                .copied()
                .filter(|q| (q.2 - eval(&first, q.0, q.1, &mut m)).abs() <= 4.0 * 1.4826 * mad)
                .collect();
            fit_surface(&kept).or(Some(first))
        })
        .collect();
    // The surfaces on a grid every 8 pixels (nodes cover the last pixel).
    const STEP: u32 = 8;
    let (gw, gh) = ((w - 1) / STEP + 2, (h - 1) / STEP + 2);
    let mut grid = vec![[0f32; 3]; (gw * gh) as usize];
    for gy in 0..gh {
        for gx in 0..gw {
            let (x, y) = norm(f64::from(gx * STEP), f64::from(gy * STEP));
            for c in 0..3 {
                grid[(gy * gw + gx) as usize][c] = surfaces[c]
                    .as_ref()
                    .map_or(1.0, |k| eval(k, x, y, &mut mono) as f32);
            }
        }
    }
    let mut out = vec![0f32; v.len()];
    for y in 0..h {
        let (gy, ay) = (y / STEP, (y % STEP) as f32 / STEP as f32);
        for x in 0..w {
            let (gx, ax) = (x / STEP, (x % STEP) as f32 / STEP as f32);
            let c = colour(s, x, y);
            let g = |xx: u32, yy: u32| grid[(yy * gw + xx) as usize][c];
            let top = g(gx, gy) + (g(gx + 1, gy) - g(gx, gy)) * ax;
            let bot = g(gx, gy + 1) + (g(gx + 1, gy + 1) - g(gx, gy + 1)) * ax;
            out[(y * w + x) as usize] = top + (bot - top) * ay;
        }
    }
    out
}

/// What a calibration frame (or the picture) was shot with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameInfo {
    pub path: PathBuf,
    /// Make and model.
    pub camera: String,
    pub lens: String,
    pub f_number: Option<f32>,
    pub iso: Option<f32>,
    /// Exposure time in seconds.
    pub exposure: Option<f32>,
    /// Stored sensor size.
    pub width: u32,
    pub height: u32,
}

/// Flat or dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    Flat,
    Dark,
}

/// The kind a calibration file's path names ("flat" or "dark" in a folder or file name).
pub fn kind_of(path: &std::path::Path) -> Option<FrameKind> {
    let s = path.to_string_lossy().to_lowercase();
    match (s.contains("flat"), s.contains("dark")) {
        (true, false) => Some(FrameKind::Flat),
        (false, true) => Some(FrameKind::Dark),
        _ => None,
    }
}

fn stops(a: Option<f32>, b: Option<f32>) -> f32 {
    match (a, b) {
        (Some(a), Some(b)) if a > 0.0 && b > 0.0 => (a / b).log2().abs(),
        _ => 2.0,
    }
}

/// The best candidate of `kind` for `target`: same camera and size; flats ranked by lens,
/// then aperture; darks by ISO, then exposure time. `None` when nothing fits.
pub fn best_match(target: &FrameInfo, kind: FrameKind, candidates: &[FrameInfo]) -> Option<usize> {
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            kind_of(&c.path) == Some(kind)
                && same(&c.camera, &target.camera)
                && c.width == target.width
                && c.height == target.height
        })
        .map(|(i, c)| {
            let score = match kind {
                FrameKind::Flat => {
                    let lens = if c.lens.is_empty() || target.lens.is_empty() {
                        5.0
                    } else if same(&c.lens, &target.lens) {
                        0.0
                    } else {
                        20.0
                    };
                    // f-numbers: two stops per doubling of the area.
                    lens + 2.0 * stops(c.f_number, target.f_number)
                }
                FrameKind::Dark => {
                    4.0 * stops(c.iso, target.iso) + stops(c.exposure, target.exposure)
                }
            };
            (i, score)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

// ---- Finding and loading frames ---------------------------------------------------------------

/// Raw extensions a calibration folder is searched for.
const RAW_EXT: &[&str] = &[
    "3fr", "arw", "cr2", "cr3", "dng", "erf", "iiq", "nef", "nrw", "orf", "pef", "raf", "rw2",
    "rwl", "sr2", "srw",
];

/// At most this many files are read from the calibration folder.
const MAX_FRAMES: usize = 400;

static FOLDER: Mutex<Option<PathBuf>> = Mutex::new(None);
type Scan = (PathBuf, Vec<FrameInfo>);
static SCAN: Mutex<Option<Scan>> = Mutex::new(None);
type BuiltKey = (String, String, bool, u32, u32, u32, u32);
static BUILT: Mutex<Option<(BuiltKey, Arc<Calibration>)>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sets the calibration folder (Settings › Metadata; `None`: no automatic choice). A new
/// folder is searched again on next use.
pub fn set_folder(folder: Option<PathBuf>) {
    let mut f = lock(&FOLDER);
    if *f != folder {
        *lock(&SCAN) = None;
    }
    *f = folder;
}

/// The calibration folder.
pub fn folder() -> Option<PathBuf> {
    lock(&FOLDER).clone()
}

/// The facts of a raw file's header.
pub fn frame_info(path: &Path, data: &[u8]) -> Option<FrameInfo> {
    let f = specular_raw::parse(data).ok()?;
    let i = f.info();
    let (width, height) = i.sensor.as_ref().map_or((0, 0), |s| (s.width, s.height));
    Some(FrameInfo {
        path: path.to_path_buf(),
        camera: i.camera_name(),
        lens: i.lens.clone().unwrap_or_default(),
        f_number: i.f_number.map(|v| v as f32),
        iso: i.iso.map(|v| v as f32),
        exposure: i.exposure_time.map(|v| v as f32),
        width,
        height,
    })
}

/// The flat and dark frames under `folder` (two levels deep), read once per folder.
pub fn scan(folder: &Path) -> Vec<FrameInfo> {
    if let Some((f, v)) = lock(&SCAN).as_ref()
        && f == folder
    {
        return v.clone();
    }
    let mut files = Vec::new();
    let mut dirs = vec![(folder.to_path_buf(), 0)];
    while let Some((d, depth)) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 2 {
                    dirs.push((p, depth + 1));
                }
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| RAW_EXT.contains(&x.to_ascii_lowercase().as_str()))
                && kind_of(&p).is_some()
                && files.len() < MAX_FRAMES
            {
                files.push(p);
            }
        }
    }
    files.sort();
    let found: Vec<FrameInfo> = files
        .iter()
        .filter_map(|p| frame_info(p, &std::fs::read(p).ok()?))
        .collect();
    *lock(&SCAN) = Some((folder.to_path_buf(), found.clone()));
    found
}

/// The flat and dark paths `ff` asks for: its own, or (automatic) the best in the
/// calibration folder for `target`.
pub fn resolve(
    ff: &super::FlatField,
    target: Option<&FrameInfo>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let own = |s: &str| (!s.is_empty()).then(|| PathBuf::from(s));
    if !ff.auto {
        return (own(&ff.flat), own(&ff.dark));
    }
    let (Some(t), Some(dir)) = (target, folder()) else {
        return (None, None);
    };
    let cands = scan(&dir);
    let pick = |k| best_match(t, k, &cands).map(|i| cands[i].path.clone());
    (pick(FrameKind::Flat), pick(FrameKind::Dark))
}

/// Why a frame could not be used, in plain words.
pub fn describe(e: &CalibError, which: &Path) -> String {
    let name = which.file_name().map_or_else(
        || which.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    match e {
        CalibError::Mismatch => format!("{name} is from another camera or sensor size"),
        CalibError::DarkFlat => format!("{name} is too dark to be a flat frame"),
    }
}

/// The calibration an edit's flat-field module asks for on `sensor` (`source` is the
/// picture's file, for automatic matching): `Ok(None)` when the module is off or names no
/// frames. The last calibration built is kept for the next picture with the same frames.
pub fn for_edit(
    edit: &crate::model::Edit,
    sensor: &SensorData,
    source: Option<(&Path, &[u8])>,
) -> Result<Option<Arc<Calibration>>, String> {
    if super::flat_field(edit).is_none() {
        return Ok(None);
    }
    let target = source.and_then(|(p, d)| frame_info(p, d));
    for_edit_at(edit, sensor, target.as_ref())
}

/// [`for_edit`] with the picture's facts already read ([`frame_info`]).
pub fn for_edit_at(
    edit: &crate::model::Edit,
    sensor: &SensorData,
    target: Option<&FrameInfo>,
) -> Result<Option<Arc<Calibration>>, String> {
    let Some(ff) = super::flat_field(edit) else {
        return Ok(None);
    };
    let (flat, dark) = resolve(&ff, target);
    if flat.is_none() && dark.is_none() {
        return Ok(None);
    }
    let name = |p: &Option<PathBuf>| {
        p.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    };
    let key = (
        name(&flat),
        name(&dark),
        ff.blur,
        sensor.crop.w,
        sensor.crop.h,
        sensor.meta.white.to_bits(),
        target.and_then(|t| t.exposure).unwrap_or(0.0).to_bits(),
    );
    if let Some((k, c)) = lock(&BUILT).as_ref()
        && *k == key
    {
        return Ok(Some(c.clone()));
    }
    let load = |p: &Option<PathBuf>| -> Result<Option<SensorData>, String> {
        let Some(p) = p else { return Ok(None) };
        let data = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        match crate::preview::decode_raw(&data) {
            Ok(crate::preview::Input::Raw(s)) => Ok(Some(*s)),
            Ok(_) => Err(format!("{} is not a raw file", p.display())),
            Err(e) => Err(format!("{}: {e}", p.display())),
        }
    };
    let (fs, ds) = (load(&flat)?, load(&dark)?);
    // A dark frame at the same ISO but another exposure time: scaled by the times.
    let scale = match (target, dark.as_ref()) {
        (Some(t), Some(d)) => std::fs::read(d)
            .ok()
            .and_then(|b| frame_info(d, &b))
            .map_or(1.0, |di| dark_scale(t, &di)),
        _ => 1.0,
    };
    let flat_off = fs.as_ref().is_some_and(|f| !same_sensor(sensor, f));
    let cal = build_scaled(sensor, fs.as_ref(), ds.as_ref(), ff.blur, scale).map_err(|e| {
        let which = match (&e, &flat, &dark) {
            (CalibError::DarkFlat, Some(f), _) => f.clone(),
            (_, Some(f), _) if flat_off => f.clone(),
            (_, _, Some(d)) => d.clone(),
            (_, Some(f), None) => f.clone(),
            _ => PathBuf::new(),
        };
        describe(&e, &which)
    })?;
    let cal = Arc::new(cal);
    *lock(&BUILT) = Some((key, cal.clone()));
    Ok(Some(cal))
}
