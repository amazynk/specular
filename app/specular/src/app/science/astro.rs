//! Astronomy interop in science mode: DS9 region files overlaid on sky images (and turned
//! into measurements to edit, and saved back), plate solving through a solver the user
//! installed, SAMP ([`samp`]) and HiPS sky surveys around the image ([`hips`]).
//!
//! Regions are kept across images like catalogues: sky regions are placed through each
//! image's WCS, image regions as they are. A plate solution is kept beside the image in
//! memory (the file is never written) and can be saved as `IMAGE.wcs`, which is read back
//! for an image without sky coordinates of its own.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use specular_astro::regions::{self, Drawn, Marker, Placed, Region, Target};
use specular_astro::solve::{self, Hint, Solver};
use specular_decode::science::wcs::{Wcs, dms, hms};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_log::{UI, info};
use specular_measure::geom::{P, Shape};
use specular_theme::Color;
use specular_theme::metrics::FONT_SMALL;

use super::super::App;
use super::wcs_rest;
use crate::loader::ImageId;

pub(crate) mod hips;
pub(crate) mod samp;

/// The largest region file read.
const MAX_REGION_FILE: u64 = 16 << 20;
/// The longest a solver may run.
const SOLVE_TIMEOUT: Duration = Duration::from_secs(600);
/// The toast key of the running solve's "Stop".
const SOLVE_KEY: u64 = 0x5a57_0001;

/// A region file read for the overlay.
pub(crate) struct RegionSet {
    pub regions: Arc<Vec<Region>>,
}

/// What a background read of a region file gave.
enum RegionsRead {
    Read(String, regions::Parsed),
    Failed(String, String),
    /// A dropped file that is not a region file: opened as usual.
    Open(PathBuf),
}

/// Placed regions in stored px: set, region, shape.
type PlacedCache = (ImageId, Vec<usize>, usize, Arc<Vec<(usize, usize, Placed)>>);

/// A plate solve in progress.
struct SolveRun {
    solver: Solver,
    started: Instant,
    shown: u64,
    cancel: Arc<AtomicBool>,
}

/// A finished solve: the image and its solution, or why not.
struct SolveDone {
    path: PathBuf,
    result: Result<specular_fits::Header, String>,
}

pub(crate) struct Astro {
    pub regions: Vec<RegionSet>,
    regions_rx: Option<Receiver<RegionsRead>>,
    region_cache: Option<PlacedCache>,
    solve: Option<SolveRun>,
    solve_tx: Sender<SolveDone>,
    solve_rx: Receiver<SolveDone>,
    /// Plate solutions by image file (kept in memory; saved only when asked).
    solutions: HashMap<PathBuf, Arc<specular_fits::Header>>,
    /// Files whose `.wcs` beside them was looked for.
    sidecar_checked: HashSet<PathBuf>,
    sidecar_rx: Option<Receiver<(PathBuf, Option<specular_fits::Header>)>>,
    save_rx: Option<Receiver<Result<String, String>>>,
    pub samp: samp::Link,
    pub hips: hips::Survey,
    /// Catalogue rows highlighted through SAMP: catalogue index and row.
    pub highlight: HashSet<(u16, u32)>,
    /// SAMP table ids and URLs of the overlaid catalogues, by catalogue index.
    pub tables: Vec<(usize, Option<String>, String)>,
    run: u64,
}

impl Default for Astro {
    fn default() -> Astro {
        let (solve_tx, solve_rx) = crossbeam_channel::unbounded();
        Astro {
            regions: Vec::new(),
            regions_rx: None,
            region_cache: None,
            solve: None,
            solve_tx,
            solve_rx,
            solutions: HashMap::new(),
            sidecar_checked: HashSet::new(),
            sidecar_rx: None,
            save_rx: None,
            samp: samp::Link::default(),
            hips: hips::Survey::default(),
            highlight: HashSet::new(),
            tables: Vec::new(),
            run: 0,
        }
    }
}

/// A region as drawn (logical px).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScreenRegion {
    pub pts: Vec<(f32, f32)>,
    pub closed: bool,
    pub color: Color,
    pub width: f32,
    pub dash: bool,
    pub include: bool,
    pub marker: Option<(Marker, f32)>,
    pub text: Option<String>,
    pub arrow: bool,
}

/// What astronomy draws over (and the survey under) the image.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AstroOverlay {
    pub survey: Option<(specular_gfx::TexId, Rect)>,
    pub credit: Option<String>,
    pub regions: Vec<ScreenRegion>,
    /// Highlighted catalogue sources (logical px).
    pub highlights: Vec<(f32, f32)>,
}

/// Whether a dropped file may be a region file, by its name.
fn regions_like(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("reg"))
}

fn read_regions(path: PathBuf, drop: bool) -> RegionsRead {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = std::fs::metadata(&path)
        .map_err(|e| crate::error_text::io_reason(&e))
        .and_then(|m| {
            if m.len() > MAX_REGION_FILE {
                Err(t!("measure-tool-too-large"))
            } else {
                std::fs::read(&path).map_err(|e| crate::error_text::io_reason(&e))
            }
        });
    match bytes {
        Ok(b) if !drop || regions::sniff(&b) => {
            let p = regions::parse(&String::from_utf8_lossy(&b));
            if p.regions.is_empty() && drop {
                return RegionsRead::Open(path);
            }
            RegionsRead::Read(name, p)
        }
        Ok(_) => RegionsRead::Open(path),
        Err(e) if !drop => RegionsRead::Failed(name, e),
        Err(_) => RegionsRead::Open(path),
    }
}

fn rgb(c: [u8; 3]) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        0.95,
    )
}

/// A placed region as a measurement shape (stored px), when it maps: axis-aligned ellipses
/// and boxes as they are, turned ones as polygons, texts as labelled points.
pub(crate) fn measure_shape(p: &Placed) -> Shape {
    let pt = |q: (f64, f64)| P::new(q.0, q.1);
    let square = |a: f64| {
        let r = a.rem_euclid(180.0);
        if r.abs() < 1e-6 || (r - 180.0).abs() < 1e-6 {
            Some(false)
        } else if (r - 90.0).abs() < 1e-6 {
            Some(true)
        } else {
            None
        }
    };
    match p {
        Placed::Ellipse { c, r, angle } => match square(*angle) {
            Some(swap) => {
                let (rx, ry) = if swap { (r.1, r.0) } else { *r };
                Shape::Ellipse {
                    cx: c.0,
                    cy: c.1,
                    rx,
                    ry,
                }
            }
            None => Shape::Polygon(p.outline(72).into_iter().map(pt).collect()),
        },
        Placed::Rect { c, size, angle } => match square(*angle) {
            Some(swap) => {
                let (w, h) = if swap { (size.1, size.0) } else { *size };
                Shape::Rect {
                    x0: c.0 - w / 2.0,
                    y0: c.1 - h / 2.0,
                    x1: c.0 + w / 2.0,
                    y1: c.1 + h / 2.0,
                }
            }
            None => Shape::Polygon(p.outline(4).into_iter().map(pt).collect()),
        },
        Placed::Polygon(v) => Shape::Polygon(v.iter().map(|q| pt(*q)).collect()),
        Placed::Line { a, b, .. } => Shape::Line(pt(*a), pt(*b)),
        Placed::Point { c, .. } | Placed::Text { c } => Shape::Point(pt(*c)),
    }
}

/// A measurement shape (stored px) as shapes to write.
pub(crate) fn drawn_of(shape: &Shape, label: &str) -> Vec<Drawn> {
    let q = |p: &P| (p.x, p.y);
    let one = |shape: Placed| Drawn {
        shape,
        text: label.to_owned(),
        color: None,
    };
    match shape {
        Shape::Point(p) => vec![one(Placed::Point {
            c: q(p),
            marker: Marker::Cross,
            size: 11.0,
        })],
        Shape::Line(a, b) => vec![one(Placed::Line {
            a: q(a),
            b: q(b),
            arrows: (false, false),
        })],
        Shape::Polyline(v) => v
            .windows(2)
            .map(|w| {
                one(Placed::Line {
                    a: q(&w[0]),
                    b: q(&w[1]),
                    arrows: (false, false),
                })
            })
            .collect(),
        Shape::Angle { a, vertex, b } => [a, b]
            .iter()
            .map(|e| {
                one(Placed::Line {
                    a: q(vertex),
                    b: q(e),
                    arrows: (false, false),
                })
            })
            .collect(),
        Shape::Rect { x0, y0, x1, y1 } => vec![one(Placed::Rect {
            c: ((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            size: ((x1 - x0).abs(), (y1 - y0).abs()),
            angle: 0.0,
        })],
        Shape::Ellipse { cx, cy, rx, ry } => vec![one(Placed::Ellipse {
            c: (*cx, *cy),
            r: (*rx, *ry),
            angle: 0.0,
        })],
        Shape::Polygon(v) | Shape::Freehand(v) => {
            vec![one(Placed::Polygon(v.iter().map(q).collect()))]
        }
    }
}

/// A placed region in logical px through `to_screen` (stored → logical).
fn on_screen(p: &Placed, r: &Region, to_screen: &dyn Fn((f64, f64)) -> (f32, f32)) -> ScreenRegion {
    let pts: Vec<(f32, f32)> = match p {
        Placed::Ellipse { .. } => p.outline(72),
        _ => p.outline(4),
    }
    .into_iter()
    .map(to_screen)
    .collect();
    ScreenRegion {
        closed: p.closed(),
        color: rgb(r.props.color),
        width: r.props.width as f32,
        dash: r.props.dash,
        include: r.props.include,
        marker: match p {
            Placed::Point { marker, size, .. } => Some((*marker, *size as f32)),
            _ => None,
        },
        text: (!r.props.text.is_empty()).then(|| r.props.text.clone()),
        arrow: matches!(
            p,
            Placed::Line {
                arrows: (_, true),
                ..
            }
        ),
        pts,
    }
}

/// A line drawn in dashes (`dash`) or whole.
fn stroke(dl: &mut DrawList, a: (f32, f32), b: (f32, f32), w: f32, c: Color, dash: bool) {
    if !dash {
        dl.line(a, b, w, c);
        return;
    }
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let (on, off) = (8.0, 3.0);
    let mut t = 0.0;
    while t < len {
        let t1 = (t + on).min(len);
        let p = |s: f32| (a.0 + (b.0 - a.0) * s / len, a.1 + (b.1 - a.1) * s / len);
        dl.line(p(t), p(t1), w, c);
        t = t1 + off;
    }
}

/// Draws the regions (logical px), clipped to `clip`.
pub(crate) fn draw_regions(
    dl: &mut DrawList,
    text: &mut Text,
    regions: &[ScreenRegion],
    clip: Rect,
    ts: f32,
) {
    dl.push_clip(clip);
    let shadow = Color::rgba(0.0, 0.0, 0.0, 0.55);
    for r in regions {
        let n = r.pts.len();
        let w = r.width.clamp(1.0, 8.0);
        if let (Some((m, size)), Some(&(x, y))) = (r.marker, r.pts.first()) {
            let h = (size / 2.0).clamp(3.0, 20.0);
            let segs: Vec<((f32, f32), (f32, f32))> = match m {
                Marker::Cross => vec![((x - h, y), (x + h, y)), ((x, y - h), (x, y + h))],
                Marker::X | Marker::Arrow => {
                    vec![
                        ((x - h, y - h), (x + h, y + h)),
                        ((x - h, y + h), (x + h, y - h)),
                    ]
                }
                Marker::Diamond => vec![
                    ((x, y - h), (x + h, y)),
                    ((x + h, y), (x, y + h)),
                    ((x, y + h), (x - h, y)),
                    ((x - h, y), (x, y - h)),
                ],
                Marker::Box | Marker::BoxCircle | Marker::Circle => vec![
                    ((x - h, y - h), (x + h, y - h)),
                    ((x + h, y - h), (x + h, y + h)),
                    ((x + h, y + h), (x - h, y + h)),
                    ((x - h, y + h), (x - h, y - h)),
                ],
            };
            for (a, b) in &segs {
                dl.line(*a, *b, w + 2.0, shadow);
            }
            for (a, b) in &segs {
                dl.line(*a, *b, w, r.color);
            }
            if matches!(m, Marker::Circle | Marker::BoxCircle) {
                let rr = if m == Marker::Circle { h } else { h * 0.7 };
                dl.stroke_rect(
                    Rect::new(x - rr, y - rr, 2.0 * rr, 2.0 * rr),
                    rr,
                    r.color,
                    w,
                );
            }
        } else if n >= 2 {
            let segs = if r.closed { n } else { n - 1 };
            for pass in 0..2 {
                let (c, lw) = if pass == 0 {
                    (shadow, w + 2.0)
                } else {
                    (r.color, w)
                };
                for i in 0..segs {
                    stroke(dl, r.pts[i], r.pts[(i + 1) % n], lw, c, r.dash && pass == 1);
                }
            }
            if r.arrow {
                let (a, b) = (r.pts[n - 2], r.pts[n - 1]);
                let ang = (b.1 - a.1).atan2(b.0 - a.0);
                for s in [-0.45f32, 0.45] {
                    let t = ang + std::f32::consts::PI + s;
                    dl.line(b, (b.0 + 9.0 * t.cos(), b.1 + 9.0 * t.sin()), w, r.color);
                }
            }
        }
        // An excluded region: struck through, as DS9 draws it.
        if !r.include && n >= 2 {
            let (x0, y0, x1, y1) = r.pts.iter().fold(
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                |(a, b, c, d), p| (a.min(p.0), b.min(p.1), c.max(p.0), d.max(p.1)),
            );
            dl.line((x0, y1), (x1, y0), w, Color::rgba(1.0, 0.25, 0.25, 0.95));
        }
        if let (Some(s), Some(&(x, y))) = (&r.text, r.pts.first()) {
            let st = TextStyle::new(FONT_SMALL * ts);
            let b = text.layout(s, st, Some(320.0), Align::Start);
            let (cx, cy) = if n > 1 && r.marker.is_none() {
                let sx: f32 = r.pts.iter().map(|p| p.0).sum();
                let top = r.pts.iter().map(|p| p.1).fold(f32::MAX, f32::min);
                (sx / n as f32, top - b.height - 2.0)
            } else if r.marker.is_some() {
                (x, y - b.height - 10.0)
            } else {
                (x, y - b.height / 2.0)
            };
            let (tx, ty) = (cx - b.width / 2.0, cy);
            dl.rect(
                Rect::new(tx - 3.0, ty - 1.0, b.width + 6.0, b.height + 2.0),
                3.0,
                Color::rgba(0.0, 0.0, 0.0, 0.5),
            );
            text.draw(dl, &b, tx, ty, r.color);
        }
    }
    dl.pop_clip();
}

/// Rings around highlighted catalogue sources.
pub(crate) fn draw_highlights(dl: &mut DrawList, at: &[(f32, f32)], clip: Rect) {
    dl.push_clip(clip);
    for &(x, y) in at {
        let r = 10.0;
        let ring = Rect::new(x - r, y - r, 2.0 * r, 2.0 * r);
        dl.stroke_rect(ring, r, Color::rgba(0.0, 0.0, 0.0, 0.6), 4.0);
        dl.stroke_rect(ring, r, Color::rgba(1.0, 0.3, 1.0, 1.0), 2.5);
    }
    dl.pop_clip();
}

/// The solver to use: picked in settings, else found on `PATH`, else where installers put it.
pub(crate) fn find_solver(
    astap: Option<&Path>,
    solve_field: Option<&Path>,
) -> Option<(Solver, PathBuf)> {
    for s in Solver::ALL {
        let picked = match s {
            Solver::Astap => astap,
            Solver::AstrometryNet => solve_field,
        };
        if let Some(p) = picked.filter(|p| p.is_file()) {
            return Some((s, p.to_path_buf()));
        }
    }
    for s in Solver::ALL {
        for name in s.programs() {
            if let Some(p) = specular_platform::launch::find_program(name) {
                return Some((s, p));
            }
        }
        if let Some(p) = s.usual_places().into_iter().find(|p| p.is_file()) {
            return Some((s, p));
        }
    }
    None
}

/// Runs a solver and waits for it (on its own thread), stopping it when `cancel` is set or
/// after [`SOLVE_TIMEOUT`].
pub(crate) fn run_solver(
    program: &Path,
    solver: Solver,
    image: &Path,
    out: &Path,
    hint: &Hint,
    cancel: &AtomicBool,
    tick: &dyn Fn(),
) -> Result<specular_fits::Header, String> {
    let args = solve::args(solver, image, out, hint);
    info!(target: UI, "plate solve: {} {args:?}", program.display());
    let mut cmd = std::process::Command::new(program);
    specular_platform::launch::scrub_env(&mut cmd);
    cmd.args(&args)
        .current_dir(out)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|e| crate::error_text::spawn_reason(&e))?;
    let until = Instant::now() + SOLVE_TIMEOUT;
    let mut second = Instant::now();
    let status = loop {
        // The window shows the time so far.
        if second.elapsed() >= Duration::from_secs(1) {
            second = Instant::now();
            tick();
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(t!("astro-solve-stopped"));
        }
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() >= until => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(t!(
                    "recipe-timeout",
                    seconds = SOLVE_TIMEOUT.as_secs() as i64
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(crate::error_text::io_reason(&e)),
        }
    };
    // ASTAP exits 1 when it finds no solution, with the reason in its .ini.
    match solve::read_solution(solver, out) {
        Ok(h) => Ok(h),
        Err(e) if status.success() => Err(e.to_string()),
        Err(solve::Failure::NotSolved(why)) => Err(solve::Failure::NotSolved(why).to_string()),
        Err(_) => Err(status.to_string()),
    }
}

/// The `.wcs` beside `path` (`IMAGE.wcs`, the name both solvers write).
pub(crate) fn sidecar_path(path: &Path) -> PathBuf {
    path.with_extension("wcs")
}

impl App {
    /// The science image's WCS with the further axes' positions, and its size.
    fn astro_wcs(&self) -> Option<(Arc<Wcs>, Vec<usize>, u32, u32)> {
        let s = self.science.summary.as_ref()?;
        let w = s.wcs.clone().filter(|w| w.celestial.is_some());
        let rest = w
            .as_ref()
            .map(|w| wcs_rest(s, w, &self.science.index))
            .unwrap_or_default();
        Some((w?, rest, s.width, s.height))
    }

    /// The current science image's file.
    fn astro_file(&self) -> Option<PathBuf> {
        self.science.path.clone()
    }

    /// Once per turn: region reads, solves, saves, sidecars, SAMP and the survey.
    pub(crate) fn astro_tick(&mut self) {
        self.astro_regions_poll();
        self.astro_solve_poll();
        self.astro_sidecar_poll();
        if let Some(Ok(r)) = self.astro.save_rx.as_ref().map(|r| r.try_recv()) {
            self.astro.save_rx = None;
            let now = self.now();
            let msg = match r {
                Ok(m) => m,
                Err(e) => t!("astro-regions-save-failed", reason = e),
            };
            self.chrome.toast(msg, None, now);
        }
        self.samp_tick();
        self.hips_tick();
    }

    // -----------------------------------------------------------------------------------------
    // Regions.

    /// Asks for a region file and reads it.
    fn astro_regions_open(&mut self) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let filter = t!("astro-regions-filter");
        let dir = self
            .astro_file()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        self.loader.background("dlg-regions", move |l| {
            let mut d = rfd::FileDialog::new().add_filter(filter, &["reg"]);
            if let Some(d2) = dir {
                d = d.set_directory(d2);
            }
            let Some(p) = d.pick_file() else { return };
            let _ = tx.send(read_regions(p, false));
            l.waker()();
        });
        self.astro.regions_rx = Some(rx);
    }

    /// A `.reg` file dropped on a science image: overlaid. True when taken.
    pub(crate) fn astro_regions_drop(&mut self, path: &Path) -> bool {
        if !self.science_shown() || !regions_like(path) || self.astro.regions_rx.is_some() {
            return false;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let p = path.to_path_buf();
        self.loader.background("io-regions", move |l| {
            let _ = tx.send(read_regions(p, true));
            l.waker()();
        });
        self.astro.regions_rx = Some(rx);
        true
    }

    fn astro_regions_poll(&mut self) {
        let Some(Ok(read)) = self.astro.regions_rx.as_ref().map(|r| r.try_recv()) else {
            return;
        };
        self.astro.regions_rx = None;
        let now = self.now();
        let (name, parsed) = match read {
            RegionsRead::Open(p) => {
                self.open_target(p);
                return;
            }
            RegionsRead::Failed(name, reason) => {
                let msg = t!("astro-regions-failed", name = name, reason = reason);
                self.chrome.toast(msg, None, now);
                return;
            }
            RegionsRead::Read(n, p) => (n, p),
        };
        self.astro_regions_add(name, parsed);
    }

    /// Overlays regions read from a file, saying what was and was not read.
    pub(crate) fn astro_regions_add(&mut self, name: String, parsed: regions::Parsed) {
        let now = self.now();
        let count = parsed.regions.len();
        let approx = parsed
            .regions
            .iter()
            .filter(|r| r.approximate.is_some())
            .count();
        let mut msg = t!(
            "astro-regions-loaded",
            name = name.clone(),
            count = count as i64
        );
        if let Some((line, why)) = parsed.refused.first() {
            msg = format!(
                "{msg}\n{}",
                t!(
                    "astro-regions-refused",
                    count = parsed.refused.len() as i64,
                    line = *line as i64,
                    reason = why.to_string()
                )
            );
        }
        if approx > 0 {
            msg = format!(
                "{msg}\n{}",
                t!("astro-regions-approximate", count = approx as i64)
            );
        }
        if count > 0 {
            self.astro.regions.push(RegionSet {
                regions: Arc::new(parsed.regions),
            });
            self.astro.region_cache = None;
            if self
                .astro_placed()
                .is_some_and(|p| !p.iter().any(|(s, _, _)| *s == self.astro.regions.len() - 1))
            {
                msg = format!("{msg}\n{}", t!("astro-regions-none-here", name = name));
            }
        }
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    fn astro_regions_clear(&mut self) {
        self.astro.regions.clear();
        self.astro.region_cache = None;
        let now = self.now();
        self.chrome.toast(t!("astro-regions-cleared"), None, now);
        self.redraw = true;
    }

    /// The overlaid regions placed on the science image (stored px), cached per image and
    /// position on the further axes.
    fn astro_placed(&mut self) -> Option<Arc<Vec<(usize, usize, Placed)>>> {
        if self.astro.regions.is_empty() {
            return None;
        }
        let id = self.science.id?;
        let s = self.science.summary.clone()?;
        let w = s.wcs.clone().filter(|w| w.celestial.is_some());
        let rest = w
            .as_ref()
            .map(|w| wcs_rest(&s, w, &self.science.index))
            .unwrap_or_default();
        let n = self.astro.regions.len();
        if let Some((i, r, k, p)) = &self.astro.region_cache
            && *i == id
            && *r == rest
            && *k == n
        {
            return Some(p.clone());
        }
        let t = Target {
            height: f64::from(s.height),
            wcs: w.as_deref().map(|w| (w, &rest[..])),
            physical: Self::physical_of(&s),
        };
        let mut out = Vec::new();
        for (k, set) in self.astro.regions.iter().enumerate() {
            for (j, r) in set.regions.iter().enumerate() {
                out.extend(r.place(&t).into_iter().map(|p| (k, j, p)));
            }
        }
        let p = Arc::new(out);
        self.astro.region_cache = Some((id, rest, n, p.clone()));
        Some(p)
    }

    /// The header's image-to-physical transform (`LTM`/`LTV`), for DS9 `physical` regions.
    fn physical_of(s: &specular_decode::science::ScienceSummary) -> Option<([f64; 2], [f64; 2])> {
        let h = specular_fits::Header::from_pairs(
            s.keywords
                .iter()
                .map(|(k, v, c)| (k.as_str(), v.as_str(), c.as_str())),
        );
        regions::physical_of(&h)
    }

    /// Turns the regions on this image into measurements, and removes them from the overlay.
    fn astro_regions_edit(&mut self) {
        let now = self.now();
        let Some(placed) = self.astro_placed() else {
            self.chrome.toast(t!("astro-regions-nothing"), None, now);
            return;
        };
        let shapes: Vec<(Shape, String)> = placed
            .iter()
            .map(|(k, j, p)| {
                let r = &self.astro.regions[*k].regions[*j];
                (measure_shape(p), r.props.text.clone())
            })
            .collect();
        if shapes.is_empty() {
            self.chrome.toast(t!("astro-regions-nothing"), None, now);
            return;
        }
        let n = self.measure_add(shapes);
        self.astro.regions.clear();
        self.astro.region_cache = None;
        self.chrome
            .toast(t!("astro-regions-edited", count = n as i64), None, now);
    }

    /// Saves the image's measurements and the overlaid regions as a DS9 region file.
    fn astro_regions_export(&mut self) {
        let now = self.now();
        let mut shapes: Vec<Drawn> = self
            .measure_items()
            .iter()
            .flat_map(|m| drawn_of(&m.shape, &m.label))
            .collect();
        if let Some(placed) = self.astro_placed() {
            for (k, j, p) in placed.iter() {
                let r = &self.astro.regions[*k].regions[*j];
                shapes.push(Drawn {
                    shape: p.clone(),
                    text: r.props.text.clone(),
                    color: Some(r.props.color),
                });
            }
        }
        if shapes.is_empty() {
            self.chrome.toast(t!("astro-regions-nothing"), None, now);
            return;
        }
        let Some(s) = self.science.summary.clone() else {
            return;
        };
        let w = s.wcs.clone().filter(|w| w.celestial.is_some());
        let rest = w
            .as_ref()
            .map(|w| wcs_rest(&s, w, &self.science.index))
            .unwrap_or_default();
        let t = Target {
            height: f64::from(s.height),
            wcs: w.as_deref().map(|w| (w, &rest[..])),
            physical: Self::physical_of(&s),
        };
        let (text, skipped) = regions::write(&shapes, &t);
        let file = self.astro_file();
        let stem = file.as_ref().and_then(|p| p.file_stem()).map_or_else(
            || "regions".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        let dir = file.and_then(|p| p.parent().map(Path::to_path_buf));
        let (tx, rx) = crossbeam_channel::bounded(1);
        let filter = t!("astro-regions-filter");
        self.loader.background("dlg-regions-out", move |l| {
            let mut d = rfd::FileDialog::new()
                .add_filter(filter, &["reg"])
                .set_file_name(format!("{stem}.reg"));
            if let Some(d2) = dir {
                d = d.set_directory(d2);
            }
            let Some(out) = d.save_file() else { return };
            let r = std::fs::write(&out, text.as_bytes())
                .map_err(|e| crate::error_text::io_reason(&e))
                .map(|()| {
                    let name = out
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let mut m = t!("astro-regions-saved", name = name);
                    if skipped > 0 {
                        m = format!(
                            "{m} {}",
                            t!("astro-regions-skipped", count = skipped as i64)
                        );
                    }
                    m
                });
            let _ = tx.send(r);
            l.waker()();
        });
        self.astro.save_rx = Some(rx);
    }

    // -----------------------------------------------------------------------------------------
    // Plate solving.

    /// Starts solving the current image, or stops the solve running.
    fn astro_solve(&mut self) {
        let now = self.now();
        if let Some(run) = &self.astro.solve {
            run.cancel.store(true, Ordering::Relaxed);
            return;
        }
        let Some(path) = self.astro_file().filter(|p| p.is_file()) else {
            self.chrome.toast(t!("astro-solve-not-local"), None, now);
            return;
        };
        let ig = &self.settings.integration;
        let Some((solver, program)) =
            find_solver(ig.astap_path.as_deref(), ig.solve_field_path.as_deref())
        else {
            self.chrome.toast(t!("astro-solve-no-solver"), None, now);
            return;
        };
        // What the header already says narrows the search.
        let hint = self
            .science
            .summary
            .as_ref()
            .map(|s| {
                let w = s.wcs.as_ref().filter(|w| w.celestial.is_some());
                Hint {
                    centre: w.and_then(|w| {
                        w.sky_at(f64::from(s.width) / 2.0, f64::from(s.height) / 2.0, &[])
                    }),
                    scale: w.and_then(|w| w.arcsec_per_pixel()),
                    height: s.height,
                }
            })
            .unwrap_or_default();
        self.astro.run += 1;
        let out = specular_settings::Locations::detect()
            .cache_dir
            .join("plate-solve")
            .join(format!("{}-{}", std::process::id(), self.astro.run));
        let _ = std::fs::remove_dir_all(&out);
        if let Err(e) = std::fs::create_dir_all(&out) {
            let msg = t!(
                "astro-solve-failed",
                reason = crate::error_text::io_reason(&e)
            );
            self.chrome.toast(msg, None, now);
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, c, p, wake) = (
            self.astro.solve_tx.clone(),
            cancel.clone(),
            path.clone(),
            self.loader.waker(),
        );
        let spawned = specular_thread::builder("int-solve").spawn(move || {
            let result = run_solver(&program, solver, &p, &out, &hint, &c, &|| wake());
            let _ = std::fs::remove_dir_all(&out);
            let _ = tx.send(SolveDone { path: p, result });
            wake();
        });
        if let Err(e) = spawned {
            let msg = t!("astro-solve-failed", reason = e.to_string());
            self.chrome.toast(msg, None, now);
            return;
        }
        self.astro.solve = Some(SolveRun {
            solver,
            started: Instant::now(),
            shown: u64::MAX,
            cancel,
        });
        self.astro_solve_progress();
    }

    /// The running solve's toast, with the time so far and "Stop".
    fn astro_solve_progress(&mut self) {
        let Some(run) = self.astro.solve.as_mut() else {
            return;
        };
        let secs = run.started.elapsed().as_secs();
        if secs == run.shown {
            return;
        }
        run.shown = secs;
        let text = t!(
            "astro-solve-running",
            solver = run.solver.name(),
            seconds = secs as i64
        );
        let now = self.now();
        self.chrome.dismiss_undo(SOLVE_KEY);
        self.chrome
            .toast_action(text, t!("astro-solve-stop"), SOLVE_KEY, now);
        self.redraw = true;
    }

    /// The solve toast's "Stop".
    pub(crate) fn astro_toast_action(&mut self, key: u64) -> bool {
        if key != SOLVE_KEY {
            return false;
        }
        self.chrome.dismiss_undo(key);
        if let Some(run) = &self.astro.solve {
            run.cancel.store(true, Ordering::Relaxed);
        }
        true
    }

    fn astro_solve_poll(&mut self) {
        if self.astro.solve.is_some() {
            self.astro_solve_progress();
        }
        let Ok(done) = self.astro.solve_rx.try_recv() else {
            return;
        };
        self.astro.solve = None;
        self.chrome.dismiss_undo(SOLVE_KEY);
        let now = self.now();
        match done.result {
            Ok(h) => {
                let size = self
                    .science
                    .summary
                    .as_ref()
                    .map_or((0, 0), |s| (s.width as usize, s.height as usize));
                match solve::solution_wcs(&h, size.0, size.1) {
                    Ok(w) => {
                        let (ra, dec) = w
                            .sky_at(size.0 as f64 / 2.0, size.1 as f64 / 2.0, &[])
                            .unwrap_or((0.0, 0.0));
                        let scale = w.arcsec_per_pixel().map_or_else(String::new, |s| {
                            format!("{}\"", specular_i18n::fixed(s, 3))
                        });
                        self.astro.solutions.insert(done.path.clone(), Arc::new(h));
                        self.astro_wcs_changed(&done.path);
                        let msg = t!(
                            "astro-solve-done",
                            ra = hms(ra),
                            dec = dms(dec),
                            scale = scale
                        );
                        self.chrome.toast(msg, None, now);
                    }
                    Err(e) => {
                        let msg = t!("astro-solve-failed", reason = e.to_string());
                        self.chrome.toast(msg, None, now);
                    }
                }
            }
            Err(e) => {
                let msg = t!("astro-solve-failed", reason = e);
                self.chrome.toast(msg, None, now);
            }
        }
    }

    /// The sky coordinates of `path` changed: its science facts are read again.
    fn astro_wcs_changed(&mut self, path: &Path) {
        if self.science.path.as_deref() == Some(path) {
            self.science.summary = None;
            self.science.grid_cache = None;
            self.science.marker_cache = None;
            self.astro.region_cache = None;
            self.hips_reset_view();
            self.redraw = true;
        }
    }

    /// The facts with a plate solution of the file applied (over the file's own WCS), and
    /// the sidecar looked for when the file has no sky coordinates.
    pub(crate) fn astro_with_solution(
        &mut self,
        mut s: specular_decode::science::ScienceSummary,
    ) -> specular_decode::science::ScienceSummary {
        let Some(path) = self.science.path.clone() else {
            return s;
        };
        if let Some(h) = self.astro.solutions.get(&path)
            && let Ok(w) = solve::solution_wcs(h, s.width as usize, s.height as usize)
        {
            s.wcs = Some(Arc::new(w));
            return s;
        }
        let sky = s.wcs.as_ref().is_some_and(|w| w.celestial.is_some());
        if !sky
            && self.astro.sidecar_rx.is_none()
            && self.astro.sidecar_checked.insert(path.clone())
        {
            let (tx, rx) = crossbeam_channel::bounded(1);
            self.loader.background("io-wcs-sidecar", move |l| {
                let side = sidecar_path(&path);
                let h = std::fs::metadata(&side)
                    .ok()
                    .filter(|m| m.is_file() && m.len() <= 4 << 20 && side != path)
                    .and_then(|_| std::fs::read(&side).ok())
                    .and_then(|b| solve::header_file(&b));
                let _ = tx.send((path, h));
                l.waker()();
            });
            self.astro.sidecar_rx = Some(rx);
        }
        s
    }

    fn astro_sidecar_poll(&mut self) {
        let Some(Ok((path, h))) = self.astro.sidecar_rx.as_ref().map(|r| r.try_recv()) else {
            return;
        };
        self.astro.sidecar_rx = None;
        let Some(h) = h else {
            return;
        };
        if h.cards.iter().any(|c| c.key.starts_with("CRVAL")) {
            let name = sidecar_path(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.astro.solutions.insert(path.clone(), Arc::new(h));
            self.astro_wcs_changed(&path);
            let now = self.now();
            self.chrome
                .toast(t!("astro-solve-sidecar", name = name), None, now);
        }
    }

    /// Saves the plate solution beside the image (asks where, `IMAGE.wcs` first).
    fn astro_solve_save(&mut self) {
        let now = self.now();
        let Some(path) = self.astro_file() else {
            return;
        };
        let Some(h) = self.astro.solutions.get(&path).cloned() else {
            self.chrome.toast(t!("astro-solve-none"), None, now);
            return;
        };
        let bytes = solve::sidecar(&h);
        let side = sidecar_path(&path);
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-wcs-out", move |l| {
            let mut d = rfd::FileDialog::new().add_filter("WCS", &["wcs"]);
            if let (Some(dir), Some(name)) = (side.parent(), side.file_name()) {
                d = d
                    .set_directory(dir)
                    .set_file_name(name.to_string_lossy().into_owned());
            }
            let Some(out) = d.save_file() else { return };
            let r = std::fs::write(&out, &bytes)
                .map_err(|e| crate::error_text::io_reason(&e))
                .map(|()| {
                    t!(
                        "astro-solve-saved",
                        name = out
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    )
                });
            let _ = tx.send(r);
            l.waker()();
        });
        self.astro.save_rx = Some(rx);
    }

    // -----------------------------------------------------------------------------------------
    // Actions and drawing.

    /// The palette's and keymap's astronomy actions; true when taken.
    pub(crate) fn astro_action(&mut self, a: specular_input::Action) -> bool {
        use specular_input::Action as A;
        match a {
            A::ScienceLoadRegions => self.astro_regions_open(),
            A::ScienceExportRegions => self.astro_regions_export(),
            A::ScienceEditRegions => self.astro_regions_edit(),
            A::ScienceClearRegions => self.astro_regions_clear(),
            A::PlateSolve => self.astro_solve(),
            A::PlateSolveSave => self.astro_solve_save(),
            A::SkySurvey => self.hips_next(),
            A::SampSendImage => self.samp_send_image(),
            A::SampSendPosition => self.samp_send_position(),
            _ => return false,
        }
        self.redraw = true;
        true
    }

    /// What astronomy draws for the science image `id`: `to_screen` maps stored px to
    /// logical px; `markers` are the catalogue sources in logical px.
    pub(crate) fn astro_overlay(
        &mut self,
        id: ImageId,
        markers: &[super::sky::Marker],
        to_screen: &dyn Fn((f64, f64)) -> (f32, f32),
    ) -> AstroOverlay {
        let _ = id;
        let mut o = AstroOverlay::default();
        if let Some(placed) = self.astro_placed() {
            o.regions = placed
                .iter()
                .map(|(k, j, p)| on_screen(p, &self.astro.regions[*k].regions[*j], to_screen))
                .collect();
        }
        if !self.astro.highlight.is_empty() {
            o.highlights = markers
                .iter()
                .filter(|m| self.astro.highlight.contains(&(m.cat, m.row)))
                .map(|m| m.at)
                .collect();
        }
        let (survey, credit) = self.hips_overlay();
        o.survey = survey;
        o.credit = credit;
        o
    }

    /// A press on the science image: a catalogue source under the pointer is highlighted in
    /// the other SAMP applications.
    pub(crate) fn astro_press(&mut self, hovered: Option<(u16, u32)>) {
        if let Some((cat, row)) = hovered {
            self.samp_highlight(cat, row);
        }
    }

    /// The image's sky position under the pointer, else at the view's centre (ICRS degrees).
    pub(crate) fn astro_pointer_sky(&self) -> Option<(f64, f64)> {
        let (w, rest, iw, ih) = self.astro_wcs()?;
        let vp = self.view.viewport;
        let from = self.view.stored_from_screen();
        let at = self
            .pointer
            .map(|p| from.apply(p))
            .filter(|s| s.0 >= 0.0 && s.1 >= 0.0 && s.0 < iw as f32 && s.1 < ih as f32)
            .unwrap_or_else(|| from.apply((vp.x + vp.w / 2.0, vp.y + vp.h / 2.0)));
        let (a, b) = w.sky_at(f64::from(at.0), f64::from(at.1), &rest)?;
        let frame = specular_astro::frames::of_wcs(&w)?;
        Some(specular_astro::frames::convert(
            frame,
            specular_astro::frames::Sky::Icrs,
            a,
            b,
        ))
    }

    /// Centres the view on an ICRS position; false when it is not on the image.
    pub(crate) fn astro_point_at(&mut self, ra: f64, dec: f64) -> bool {
        let Some((w, rest, iw, ih)) = self.astro_wcs() else {
            return false;
        };
        let Some(frame) = specular_astro::frames::of_wcs(&w) else {
            return false;
        };
        let (a, b) =
            specular_astro::frames::convert(specular_astro::frames::Sky::Icrs, frame, ra, dec);
        let Some((x, y)) = w.display_of(a, b, &rest) else {
            return false;
        };
        if !(x >= 0.0 && y >= 0.0 && x < f64::from(iw) && y < f64::from(ih)) {
            return false;
        }
        let screen = self
            .view
            .stored_from_screen()
            .invert()
            .apply((x as f32, y as f32));
        let d = self.view.screen_to_display(screen);
        self.view.center_on(d, true);
        self.redraw = true;
        true
    }
}

/// Draws the survey texture and its credit (logical px).
pub(crate) fn draw_survey(
    dl: &mut DrawList,
    text: &mut Text,
    o: &AstroOverlay,
    area: Rect,
    ts: f32,
    rtl: bool,
) {
    if let Some((tex, r)) = o.survey {
        dl.push_clip(area);
        dl.image(r, [0.0, 0.0, 1.0, 1.0], tex, 0.0);
        dl.pop_clip();
    }
    // The credit in the bottom corner on the end side.
    if let Some(c) = &o.credit {
        let st = TextStyle::new(FONT_SMALL * ts * 0.9);
        let b = text.layout(c, st, Some(area.w * 0.7), Align::Start);
        let x = if rtl {
            area.x + 10.0
        } else {
            area.x + area.w - b.width - 10.0
        };
        let y = area.y + area.h - b.height - 8.0;
        dl.rect(
            Rect::new(x - 4.0, y - 2.0, b.width + 8.0, b.height + 4.0),
            3.0,
            Color::rgba(0.0, 0.0, 0.0, 0.55),
        );
        text.draw(dl, &b, x, y, Color::rgba(0.92, 0.92, 0.92, 1.0));
    }
}

#[cfg(test)]
mod tests;
