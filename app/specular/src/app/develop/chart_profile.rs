//! "Make profile from chart…" (Phase 3): a camera profile from a photo of a ColorChecker
//! or IT8 chart, in a panel over the picture.
//!
//! 1. **Chart and reference.** The chart type, and its reference file (CGATS text: the file
//!    that came with the chart, or the maker's published values). SpyderCHECKR is refused
//!    with a reason. No values are bundled: the panel says where to get a file.
//! 2. **Corners.** The user clicks the centres of the four corner patches, named in order
//!    (ColorChecker: dark skin, bluish green, black, white). Each click snaps to its patch.
//! 3. **Measure.** The photo is developed with the camera matrix only
//!    ([`specular_develop::chart::chart_edit`]), and the chart found, sampled and solved
//!    off the UI thread ([`measure`]).
//! 4. **Report.** Reference and measured colours as split squares, ΔE2000 before and after,
//!    cross-validated, patches left out and why, uneven light.
//! 5. **Save.** A name; raws get a `.dcp` in the camera's profile folder that the edit then
//!    uses (`InputProfile::File`); other images get the colour calibration mixer and a
//!    `.cube` in the LUT module. "Use for photos from this camera" also saves a style with
//!    only the input profile and an auto-apply rule for the camera.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use specular_develop::chart::cgats::{self, CgatsError, Reference};
use specular_develop::chart::layout::{COLORCHECKER_NAMES, ChartKind, Layout, LayoutError};
use specular_develop::chart::sample::{self, Flag, Picture};
use specular_develop::chart::solve::{self, Fit, SolveError};
use specular_develop::chart::{self as chart, SensorMeta, camera_key, colour};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD};
use specular_theme::{Color, Palette};

use super::controls;
use crate::app::App;

/// The chart types, in the order the chooser cycles them.
pub(super) const KINDS: [ChartKind; 4] = ChartKind::ALL;

/// Panel width (logical px at text scale 1).
const WIDTH: f32 = 380.0;

/// Where the flow is.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum Step {
    /// Choosing the chart and its reference file.
    Reference,
    /// Clicking the corners.
    Marking,
    /// Developing and solving.
    Measuring,
    /// The fit and its report.
    Report,
    /// The chart could not be measured: why.
    Failed(String),
}

/// A panel button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Button {
    Kind,
    Reference,
    Again,
    UseForCamera,
    Save,
    Close,
}

/// A measured chart.
#[derive(Clone, Debug)]
pub(in crate::app) struct Measured {
    pub fit: Fit,
    /// The patches' names, in layout order.
    pub ids: Vec<String>,
    pub cols: u32,
    pub rows: u32,
    /// Grid position of each patch (for the drawing).
    pub at: Vec<(f64, f64)>,
    /// The raw's sensor colour (None for other images).
    pub meta: Option<Box<SensorMeta>>,
}

/// The flow's state.
#[derive(Debug)]
pub(in crate::app) struct Flow {
    pub raw: bool,
    pub kind: ChartKind,
    /// The reference file's name and contents, parsed.
    pub reference: Option<(String, Reference)>,
    pub layout: Option<Layout>,
    /// Why the reference (or the chart type) cannot be used.
    pub problem: Option<String>,
    /// Corner clicks, source-frame fractions.
    pub clicks: Vec<(f64, f64)>,
    pub step: Step,
    pub measured: Option<Measured>,
    pub use_for_camera: bool,
    /// The camera, as the panel names it.
    pub camera: String,
    /// Buttons where they were last drawn.
    buttons: Vec<(Button, Rect)>,
    /// The panel where it was last drawn.
    pub rect: Rect,
}

/// The reason for a refused reference file, in the UI language.
pub(super) fn cgats_problem(e: &CgatsError) -> String {
    match e {
        CgatsError::TooLarge => t!("develop-chart-ref-too-large"),
        CgatsError::NotCgats | CgatsError::NoFormat | CgatsError::NoData => {
            t!("develop-chart-ref-not-cgats")
        }
        CgatsError::NoIds => t!("develop-chart-ref-no-ids"),
        CgatsError::NoColour => t!("develop-chart-ref-no-colour"),
        CgatsError::BadRow { row, .. } | CgatsError::BadNumber { row } => {
            t!("develop-chart-ref-bad-row", row = *row as i64)
        }
        CgatsError::Count { declared, found } => t!(
            "develop-chart-ref-count",
            declared = *declared as i64,
            found = *found as i64
        ),
        CgatsError::Duplicate { id } => t!("develop-chart-ref-duplicate", id = id.clone()),
        CgatsError::Illuminant { name } => t!("develop-chart-ref-illuminant", name = name.clone()),
        CgatsError::Datacolor => t!("develop-chart-spyder"),
    }
}

fn layout_problem(e: &LayoutError) -> String {
    match e {
        LayoutError::Datacolor => t!("develop-chart-spyder"),
        LayoutError::Count { found, expected } => t!(
            "develop-chart-ref-wrong-chart",
            found = *found as i64,
            expected = *expected as i64
        ),
        LayoutError::Names => t!("develop-chart-ref-names"),
        LayoutError::Corner { id } => t!("develop-chart-ref-corner", id = id.clone()),
    }
}

fn solve_problem(e: &SolveError) -> String {
    match e {
        SolveError::TooFewPatches { usable } => {
            t!("develop-chart-too-few", count = *usable as i64)
        }
        SolveError::NoNeutrals => t!("develop-chart-no-greys"),
        SolveError::Degenerate => t!("develop-chart-degenerate"),
    }
}

pub(super) fn kind_name(k: ChartKind) -> String {
    match k {
        ChartKind::ColorChecker24 => t!("develop-chart-kind-cc24"),
        ChartKind::ColorCheckerSg => t!("develop-chart-kind-sg"),
        ChartKind::It8 => t!("develop-chart-kind-it8"),
        ChartKind::SpyderCheckr => t!("develop-chart-kind-spyder"),
    }
}

/// Where to get a reference file for chart type `k`.
fn kind_hint(k: ChartKind) -> String {
    match k {
        ChartKind::ColorChecker24 | ChartKind::ColorCheckerSg => t!("develop-chart-hint-xrite"),
        ChartKind::It8 => t!("develop-chart-hint-it8"),
        ChartKind::SpyderCheckr => t!("develop-chart-spyder"),
    }
}

impl Flow {
    pub fn new(raw: bool, camera: String) -> Flow {
        Flow {
            raw,
            kind: ChartKind::ColorChecker24,
            reference: None,
            layout: None,
            problem: None,
            clicks: Vec::new(),
            step: Step::Reference,
            measured: None,
            use_for_camera: false,
            camera,
            buttons: Vec::new(),
            rect: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// The next chart type; the reference is checked against it again.
    pub fn next_kind(&mut self) {
        let i = KINDS.iter().position(|k| *k == self.kind).unwrap_or(0);
        self.kind = KINDS[(i + 1) % KINDS.len()];
        self.relayout();
    }

    fn relayout(&mut self) {
        self.layout = None;
        self.clicks.clear();
        self.measured = None;
        self.problem = None;
        if self.kind == ChartKind::SpyderCheckr {
            self.problem = Some(t!("develop-chart-spyder"));
            self.step = Step::Reference;
            return;
        }
        let Some((_, r)) = &self.reference else {
            self.step = Step::Reference;
            return;
        };
        match Layout::new(self.kind, r) {
            Ok(l) => {
                self.layout = Some(l);
                self.step = Step::Marking;
            }
            Err(e) => {
                self.problem = Some(layout_problem(&e));
                self.step = Step::Reference;
            }
        }
    }

    /// A reference file read: parsed, the chart type guessed from it when it fits another.
    pub fn load_reference(&mut self, name: String, bytes: &[u8]) {
        match cgats::parse(bytes) {
            Ok(r) => {
                if let Some(k) = ChartKind::guess(&r) {
                    self.kind = k;
                }
                self.reference = Some((name, r));
                self.relayout();
            }
            Err(e) => {
                self.reference = None;
                self.layout = None;
                self.problem = Some(cgats_problem(&e));
                self.step = Step::Reference;
            }
        }
    }

    /// The name of corner `i` (click order).
    pub fn corner_name(&self, i: usize) -> String {
        let Some(l) = &self.layout else {
            return String::new();
        };
        let p = &l.patches[l.corners[i.min(3)]];
        if l.kind == ChartKind::ColorChecker24 {
            let at = (p.at.1 as usize) * 6 + p.at.0 as usize;
            return match COLORCHECKER_NAMES.get(at) {
                Some(&"dark skin") => t!("develop-chart-corner-dark-skin"),
                Some(&"bluish green") => t!("develop-chart-corner-bluish-green"),
                Some(&"black") => t!("develop-chart-corner-black"),
                Some(&"white") => t!("develop-chart-corner-white"),
                _ => p.id.clone(),
            };
        }
        p.id.clone()
    }

    /// A corner clicked at source-frame point `f`; true when all four are in.
    pub fn click(&mut self, f: (f64, f64)) -> bool {
        if self.step != Step::Marking {
            return false;
        }
        self.clicks.push(f);
        if self.clicks.len() == 4 {
            self.step = Step::Measuring;
            return true;
        }
        false
    }

    /// Back to marking (the reference kept).
    pub fn again(&mut self) {
        self.clicks.clear();
        self.measured = None;
        self.step = if self.layout.is_some() {
            Step::Marking
        } else {
            Step::Reference
        };
    }

    /// The measurement came back.
    pub fn measured(&mut self, r: Result<Measured, String>) {
        match r {
            Ok(m) => {
                self.measured = Some(m);
                self.step = Step::Report;
            }
            Err(e) => self.step = Step::Failed(e),
        }
    }

    /// What the user does next, in one line.
    pub fn prompt(&self) -> String {
        match &self.step {
            Step::Reference => match &self.problem {
                Some(p) => p.clone(),
                None => t!("develop-chart-step-reference"),
            },
            Step::Marking => t!(
                "develop-chart-step-corner",
                name = self.corner_name(self.clicks.len()),
                number = (self.clicks.len() + 1) as i64
            ),
            Step::Measuring => t!("develop-chart-measuring"),
            Step::Report => t!("develop-chart-step-save"),
            Step::Failed(why) => why.clone(),
        }
    }

    /// The button at `lp`, from the last draw.
    pub fn hit(&self, lp: (f32, f32)) -> Option<Button> {
        self.buttons
            .iter()
            .find(|(_, r)| r.contains(lp))
            .map(|(b, _)| *b)
    }

    pub fn contains(&self, lp: (f32, f32)) -> bool {
        self.rect.contains(lp)
    }

    /// The report's summary lines.
    pub fn report_lines(&self) -> Vec<String> {
        let Some(m) = &self.measured else {
            return Vec::new();
        };
        let r = &m.fit.report;
        let n = |v: f64| specular_i18n::fixed(v, 2);
        let mut lines = vec![
            t!(
                "develop-chart-report-after",
                mean = n(r.after.mean),
                median = n(r.after.median),
                max = n(r.after.max)
            ),
            t!(
                "develop-chart-report-before",
                mean = n(r.before.mean),
                median = n(r.before.median),
                max = n(r.before.max)
            ),
            t!("develop-chart-report-cv", mean = n(r.cross_validated.mean)),
        ];
        let left: Vec<String> = r
            .patches
            .iter()
            .zip(&m.ids)
            .filter_map(|(p, id)| {
                p.left_out.map(|f| {
                    let why = match f {
                        Flag::Clipped => t!("develop-chart-left-clipped"),
                        Flag::Uneven => t!("develop-chart-left-uneven"),
                        Flag::Outside => t!("develop-chart-left-outside"),
                    };
                    t!("develop-chart-left-out", id = id.clone(), why = why)
                })
            })
            .collect();
        lines.extend(left);
        if let Some(u) = r.uneven_light {
            lines.push(t!(
                "develop-chart-uneven-light",
                percent = (u * 100.0).round() as i64
            ));
        }
        lines
    }

    /// Draws the panel in `area` (the picture's part of the window) and keeps its buttons.
    pub fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        area: Rect,
        rtl: bool,
        ts: f32,
    ) {
        let g = ts.max(1.0);
        let w = (WIDTH * g).min(area.w - 16.0 * g).max(200.0);
        let pad = PAD * g;
        let st = TextStyle::new(FONT_SMALL * g);
        let bold = TextStyle::new(FONT_SMALL * g * 1.15).weight(600.0);
        let x = if rtl {
            area.right() - w - 12.0 * g
        } else {
            area.x + 12.0 * g
        };
        let inner = w - 2.0 * pad;
        let align = if rtl { Align::End } else { Align::Start };
        // Lay out first, to know the height.
        let mut items: Vec<Item> = Vec::new();
        items.push(Item::Text(t!("develop-chart-title"), bold, pal.text));
        items.push(Item::Row(
            t!("develop-chart-kind"),
            kind_name(self.kind),
            Button::Kind,
        ));
        let file = self
            .reference
            .as_ref()
            .map_or_else(|| t!("develop-chart-ref-load"), |(n, _)| n.clone());
        items.push(Item::Row(t!("develop-chart-ref"), file, Button::Reference));
        if self.reference.is_none() || self.problem.is_some() {
            items.push(Item::Text(kind_hint(self.kind), st, pal.text_muted));
        }
        let warn = matches!(self.step, Step::Failed(_)) || self.problem.is_some();
        items.push(Item::Text(
            self.prompt(),
            st,
            if warn { pal.warning } else { pal.text },
        ));
        if self.step == Step::Report {
            items.push(Item::Patches);
            for l in self.report_lines() {
                items.push(Item::Text(l, st, pal.text));
            }
            if self.raw {
                items.push(Item::Check(
                    t!("develop-chart-use-camera", camera = self.camera.clone()),
                    self.use_for_camera,
                ));
            }
        }
        // From the end side: Save (the step's action), Mark again, Close.
        let mut buttons = Vec::new();
        if self.step == Step::Report {
            buttons.push((Button::Save, t!("develop-chart-save")));
        }
        if matches!(self.step, Step::Report | Step::Failed(_)) || !self.clicks.is_empty() {
            buttons.push((Button::Again, t!("develop-chart-again")));
        }
        buttons.push((Button::Close, t!("develop-chart-close")));
        let line = text.layout("Ag", st, None, Align::Start).height;
        let row_h = line + 8.0 * g;
        let patches_h = self.patch_size(inner).1;
        let mut blocks = Vec::new();
        let mut h = pad;
        for it in &items {
            let ih = match it {
                Item::Text(s, style, _) => {
                    let b = text.layout(s, *style, Some(inner), align);
                    let bh = b.height;
                    blocks.push(Some(b));
                    bh
                }
                Item::Row(..) | Item::Check(..) => {
                    blocks.push(None);
                    row_h
                }
                Item::Patches => {
                    blocks.push(None);
                    patches_h
                }
            };
            h += ih + 6.0 * g;
        }
        h += row_h + pad;
        let y0 = area.y + 12.0 * g;
        let panel = Rect::new(x, y0, w, h);
        self.rect = panel;
        self.buttons.clear();
        dl.glass(panel, RADIUS_MD * g, pal.glass, pal.glass_border, 1.0);
        let mut y = y0 + pad;
        let lx = x + pad;
        for (it, b) in items.iter().zip(&blocks) {
            match (it, b) {
                (Item::Text(_, _, c), Some(b)) => {
                    text.draw(dl, b, lx, y, *c);
                    y += b.height;
                }
                (Item::Row(label, value, button), _) => {
                    let lb = text.layout(label, st, None, Align::Start);
                    let vb = text.layout(value, st, Some(inner * 0.62), Align::Start);
                    let bw = (vb.width + 16.0 * g).min(inner * 0.66);
                    let (label_x, bx) = if rtl {
                        (x + w - pad - lb.width, lx)
                    } else {
                        (lx, x + w - pad - bw)
                    };
                    text.draw(
                        dl,
                        &lb,
                        label_x,
                        y + (row_h - lb.height) / 2.0,
                        pal.text_muted,
                    );
                    let r = Rect::new(bx, y, bw, row_h);
                    dl.rect_border(
                        r,
                        RADIUS_MD * g * 0.6,
                        pal.surface_raised,
                        pal.separator,
                        1.0,
                    );
                    text.draw(
                        dl,
                        &vb,
                        bx + 8.0 * g,
                        y + (row_h - vb.height) / 2.0,
                        pal.text,
                    );
                    self.buttons.push((*button, r));
                    y += row_h;
                }
                (Item::Check(label, on), _) => {
                    let bs = line;
                    let lb = text.layout(label, st, Some(inner - bs - 8.0 * g), Align::Start);
                    let (bx, tx) = if rtl {
                        (x + w - pad - bs, x + w - pad - bs - 8.0 * g - lb.width)
                    } else {
                        (lx, lx + bs + 8.0 * g)
                    };
                    let b = Rect::new(bx, y + (row_h - bs) / 2.0, bs, bs);
                    if *on {
                        dl.rect(b, 3.0 * g, pal.accent_fill);
                        dl.line(
                            (b.x + bs * 0.22, b.y + bs * 0.52),
                            (b.x + bs * 0.42, b.y + bs * 0.72),
                            2.0 * g,
                            pal.accent_text,
                        );
                        dl.line(
                            (b.x + bs * 0.42, b.y + bs * 0.72),
                            (b.x + bs * 0.8, b.y + bs * 0.3),
                            2.0 * g,
                            pal.accent_text,
                        );
                    } else {
                        dl.rect_border(b, 3.0 * g, pal.surface, pal.text_muted, 1.0);
                    }
                    text.draw(dl, &lb, tx, y + (row_h - lb.height) / 2.0, pal.text);
                    self.buttons
                        .push((Button::UseForCamera, Rect::new(x + pad, y, inner, row_h)));
                    y += row_h;
                }
                (Item::Patches, _) => {
                    self.draw_patches(dl, Rect::new(lx, y, inner, patches_h), rtl);
                    y += patches_h;
                }
                _ => {}
            }
            y += 6.0 * g;
        }
        // Buttons along the bottom, on the reading direction's end side.
        let mut bx = if rtl { lx } else { x + w - pad };
        for (b, label) in &buttons {
            let lb = text.layout(label, st, None, Align::Start);
            let bw = lb.width + 20.0 * g;
            let r = if rtl {
                Rect::new(bx, y, bw, row_h)
            } else {
                Rect::new(bx - bw, y, bw, row_h)
            };
            let primary = *b == Button::Save;
            let (fill, ink) = if primary {
                (pal.accent_fill, pal.accent_text)
            } else {
                (pal.surface_raised, pal.text)
            };
            dl.rect_border(r, RADIUS_MD * g * 0.6, fill, pal.separator, 1.0);
            text.draw(dl, &lb, r.x + 10.0 * g, y + (row_h - lb.height) / 2.0, ink);
            self.buttons.push((*b, r));
            bx = if rtl {
                bx + bw + 8.0 * g
            } else {
                bx - bw - 8.0 * g
            };
        }
    }

    /// The patch drawing's cell size and height for width `w`.
    fn patch_size(&self, w: f32) -> (f32, f32) {
        let Some(m) = &self.measured else {
            return (0.0, 0.0);
        };
        let rows = m.at.iter().map(|a| a.1).fold(0f64, f64::max).ceil() as f32 + 1.0;
        let cell = (w / m.cols.max(1) as f32).min(40.0);
        (cell, cell * rows.max(m.rows as f32))
    }

    /// Each patch as a split square: reference on top, the profile's result below; a
    /// left-out patch is crossed.
    fn draw_patches(&self, dl: &mut DrawList, r: Rect, rtl: bool) {
        let Some(m) = &self.measured else { return };
        let (cell, _) = self.patch_size(r.w);
        let gap = (cell * 0.08).max(1.0);
        for (p, res) in m.at.iter().zip(&m.fit.report.patches) {
            let col = if rtl {
                f64::from(m.cols) - 1.0 - p.0
            } else {
                p.0
            };
            // Right to left, the grid sits against the panel's right edge.
            let x0 = if rtl {
                r.right() - m.cols as f32 * cell
            } else {
                r.x
            };
            let x = x0 + col as f32 * cell;
            let y = r.y + p.1 as f32 * cell;
            let s = cell - gap;
            dl.rect(Rect::new(x, y, s, s / 2.0), 0.0, lab_colour(res.reference));
            dl.rect(
                Rect::new(x, y + s / 2.0, s, s / 2.0),
                0.0,
                lab_colour(res.measured),
            );
            if res.left_out.is_some() {
                let ink = Color::rgba(0.9, 0.2, 0.2, 1.0);
                dl.line((x, y), (x + s, y + s), 1.5, ink);
                dl.line((x + s, y), (x, y + s), 1.5, ink);
            }
        }
    }
}

enum Item {
    Text(String, TextStyle, Color),
    /// Label, value, the value's button.
    Row(String, String, Button),
    Check(String, bool),
    Patches,
}

/// An L*a*b* (D50) colour for the screen (sRGB).
fn lab_colour(lab: colour::Lab) -> Color {
    let xyz = colour::lab_to_xyz(lab);
    let rgb = colour::mulv(&colour::xyz_to_rec709(), xyz);
    let e = rgb.map(|v| specular_color::math::linear_to_srgb(v.clamp(0.0, 1.0) as f32));
    Color::rgba(e[0], e[1], e[2], 1.0)
}

/// Source-frame fractions → picture pixels.
pub(in crate::app) type ToPicture = Box<dyn Fn((f64, f64)) -> (f64, f64) + Send>;
/// A reference file read: its name, and its bytes or why not.
type RefRead = (String, Result<Vec<u8>, String>);

/// What [`measure`] works on: the chart picture and how to map clicks onto it.
pub(in crate::app) struct Shot {
    /// Linear Rec.2020 RGBA from the chart edit.
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<f32>,
    /// Source-frame fractions → picture pixels.
    pub to_picture: ToPicture,
    pub meta: Option<SensorMeta>,
}

/// Finds, samples and solves the chart (CPU; off the UI thread).
pub(in crate::app) fn measure(
    layout: &Layout,
    mut shot: Shot,
    clicks: &[(f64, f64)],
) -> Result<Measured, String> {
    let meta = shot.meta.as_ref();
    chart::camera_values(&mut shot.rgba, meta);
    let pic = Picture {
        w: shot.w,
        h: shot.h,
        rgba: &shot.rgba,
    };
    let c: Vec<(f64, f64)> = clicks.iter().map(|f| (shot.to_picture)(*f)).collect();
    let corners = [c[0], c[1], c[2], c[3]];
    let at = sample::locate(layout, &pic, corners).ok_or_else(|| t!("develop-chart-not-found"))?;
    let samples = sample::sample(layout, &pic, &at, chart::clip_levels(meta));
    let fit = solve::solve(&solve::Input {
        layout,
        samples: &samples,
        before: chart::before_matrix(meta),
        table: true,
    })
    .map_err(|e| solve_problem(&e))?;
    Ok(Measured {
        fit,
        ids: layout.patches.iter().map(|p| p.id.clone()).collect(),
        cols: layout.cols,
        rows: layout.rows,
        at: layout.patches.iter().map(|p| p.at).collect(),
        meta: shot.meta.map(Box::new),
    })
}

/// A name not yet taken in `dir` for `stem` with extension `ext` (`stem.ext`, `stem 2.ext`…).
pub(super) fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem} {n}.{ext}"));
        n += 1;
    }
    p
}

/// The open flow's channels: the reference file read, the measurement, a profile file
/// chosen for the edit.
#[derive(Default)]
pub(in crate::app) struct Channels {
    pub reference: Option<Receiver<RefRead>>,
    pub measured: Option<Receiver<Result<Measured, String>>>,
    pub profile: Option<Receiver<PathBuf>>,
}

impl std::fmt::Debug for Channels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channels").finish_non_exhaustive()
    }
}

/// Specular's camera profile folder for a camera (None in tests).
fn profiles_dir(camera_key: &str) -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    let data = specular_settings::Locations::detect().data_dir;
    Some(specular_develop::chart::output::profile_dir(
        &data, camera_key,
    ))
}

impl App {
    /// "Make profile from chart…": the panel opens over the picture.
    pub(super) fn develop_chart_start(&mut self) {
        let Some(s) = self.develop.session.as_ref() else {
            return;
        };
        let raw = controls::is_raw(&s.path);
        let f = super::batch::photo_facts(&s.path);
        let camera = if f.model.is_empty() {
            t!("develop-rule-any-camera")
        } else {
            f.model.clone()
        };
        self.develop.chart = Some(Flow::new(raw, camera));
        self.develop.picking = false;
        self.develop.crop = None;
        self.chrome
            .announce_status(t!("develop-chart-step-reference"));
        self.redraw = true;
    }

    /// Closes the panel (nothing saved is touched).
    pub(super) fn develop_chart_close(&mut self) {
        self.develop.chart = None;
        self.develop.chart_io.reference = None;
        self.develop.chart_io.measured = None;
        self.redraw = true;
    }

    /// A press at `lp`; whether the chart flow took it.
    pub(super) fn develop_chart_press(&mut self, lp: (f32, f32)) -> bool {
        let Some(f) = self.develop.chart.as_mut() else {
            return false;
        };
        if f.contains(lp) {
            match f.hit(lp) {
                Some(Button::Kind) => {
                    f.next_kind();
                    let said = f.prompt();
                    self.chrome.announce_status(said);
                }
                Some(Button::Reference) => self.develop_chart_reference_dialog(),
                Some(Button::Again) => f.again(),
                Some(Button::UseForCamera) => f.use_for_camera = !f.use_for_camera,
                Some(Button::Save) => self.develop_chart_ask_name(),
                Some(Button::Close) => self.develop_chart_close(),
                None => {}
            }
            self.redraw = true;
            return true;
        }
        let pic = self.develop.pic;
        if f.step != Step::Marking || !pic.contains(lp) {
            return false;
        }
        let Some(m) = self.develop.map.as_ref() else {
            return false;
        };
        let frame = m.to_frame(lp);
        if f.click(frame)
            && let Some(s) = self.develop.session.as_ref()
        {
            let e = specular_develop::chart::chart_edit(&s.edit, f.raw);
            self.develop.preview.want_chart_picture(e);
        }
        let said = f.prompt();
        self.chrome.announce_status(said);
        self.redraw = true;
        true
    }

    /// Escape closes the panel.
    pub(super) fn develop_chart_escape(&mut self) -> bool {
        if self.develop.chart.is_none() {
            return false;
        }
        self.develop_chart_close();
        true
    }

    /// The reference file: the platform's file dialog, read off the UI thread.
    fn develop_chart_reference_dialog(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.develop.chart_io.reference = Some(rx);
        let title = t!("develop-chart-ref-dialog");
        let kind = t!("develop-chart-ref-kind");
        let too_large = t!("develop-chart-ref-too-large");
        self.loader.background("dlg-chart-ref", move |l| {
            let d = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(&kind, &["txt", "cie", "ti3", "cgats", "it8"]);
            if let Some(p) = d.pick_file() {
                let name = p
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let r = match std::fs::metadata(&p) {
                    Ok(m) if m.len() > cgats::MAX_BYTES as u64 => Err(too_large),
                    _ => std::fs::read(&p).map_err(|e| e.to_string()),
                };
                let _ = tx.send((name, r));
                l.waker()();
            }
        });
    }

    /// "Profile file…" in the camera profile choice: the platform's file dialog.
    pub(super) fn develop_input_profile_dialog(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.develop.chart_io.profile = Some(rx);
        let dir = self.develop.session.as_ref().and_then(|s| {
            let f = super::batch::photo_facts(&s.path);
            profiles_dir(&camera_key(&f.maker, &f.model))
                .filter(|d| d.is_dir())
                .or_else(|| s.path.parent().map(Path::to_path_buf))
        });
        let title = t!("develop-input-profile-dialog");
        let kind = t!("develop-input-profile-kind");
        self.loader.background("dlg-dcp", move |l| {
            let mut d = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(&kind, &["dcp"]);
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
    }

    /// Results that came in: the reference file, the chart picture, the measurement, a
    /// profile file for the edit.
    pub(super) fn develop_chart_tick(&mut self) {
        let ms = self.develop_ms();
        let now = self.now();
        let chosen = self
            .develop
            .chart_io
            .profile
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(p) = chosen {
            self.develop.chart_io.profile = None;
            match specular_develop::ops::raw::profile_file::read(&p) {
                Ok(l) => {
                    if let Some(s) = self.develop.session.as_mut()
                        && s.set_input_profile_file(&p, &l.hash, ms)
                    {
                        self.develop.unsaved = Some(now);
                    }
                }
                Err(e) => self
                    .chrome
                    .announce_status(t!("develop-input-profile-unusable", reason = e.to_string())),
            }
            self.redraw = true;
        }
        if self.develop.chart.is_none() {
            return;
        }
        let read = self
            .develop
            .chart_io
            .reference
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some((name, r)) = read
            && let Some(f) = self.develop.chart.as_mut()
        {
            self.develop.chart_io.reference = None;
            match r {
                Ok(bytes) => f.load_reference(name, &bytes),
                Err(e) => f.problem = Some(t!("develop-chart-ref-unreadable", reason = e)),
            }
            let said = f.prompt();
            self.chrome.announce_status(said);
            self.redraw = true;
        }
        let shot = self.develop.preview.take_chart_picture();
        if let Some(r) = shot
            && let Some(f) = self.develop.chart.as_mut()
        {
            match (r, f.layout.clone()) {
                (Ok(shot), Some(layout)) => {
                    let clicks = f.clicks.clone();
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.develop.chart_io.measured = Some(rx);
                    let waker = self.loader.waker();
                    let spawned = specular_thread::builder("chart-profile").spawn(move || {
                        let _ = tx.send(measure(&layout, shot, &clicks));
                        waker();
                    });
                    if spawned.is_err() {
                        f.measured(Err(t!("develop-chart-not-found")));
                    }
                }
                (Err(e), _) => f.measured(Err(e)),
                (Ok(_), None) => f.again(),
            }
            self.redraw = true;
        }
        let done = self
            .develop
            .chart_io
            .measured
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(r) = done
            && let Some(f) = self.develop.chart.as_mut()
        {
            self.develop.chart_io.measured = None;
            f.measured(r);
            let said = match &f.step {
                Step::Report => f.report_lines().first().cloned().unwrap_or_default(),
                _ => f.prompt(),
            };
            self.chrome.announce_status(said);
            self.redraw = true;
        }
    }

    /// Save: asks for the profile's name.
    fn develop_chart_ask_name(&mut self) {
        let Some(f) = self.develop.chart.as_ref() else {
            return;
        };
        let name = t!("develop-chart-default-name", camera = f.camera.clone());
        let s = super::sheet::DevelopSheet::name(
            super::sheet::Purpose::ChartProfile,
            t!("develop-chart-save-title"),
            t!("develop-name"),
            &name,
            t!("develop-chart-save"),
        );
        self.fileops_open_develop(s);
    }

    /// The name sheet's OK: the profile written and used.
    pub(super) fn develop_chart_save(&mut self, name: &str) {
        let Some(f) = self.develop.chart.as_ref() else {
            return;
        };
        let Some(m) = f.measured.clone() else {
            return;
        };
        let use_for_camera = f.use_for_camera;
        let Some(path) = self.develop.session.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        let facts = super::batch::photo_facts(&path);
        let key = camera_key(&facts.maker, &facts.model);
        let Some(dir) = profiles_dir(&key) else {
            return;
        };
        let stem = specular_develop::chart::output::file_stem(name);
        let ms = self.develop_ms();
        let now = self.now();
        let written = match &m.meta {
            Some(meta) => self.develop_chart_write_dcp(&dir, &stem, name, &key, &m, meta, ms),
            None => self.develop_chart_write_internal(&dir, &stem, &m, ms),
        };
        match written {
            Ok(file) => {
                self.develop.unsaved = Some(now);
                if use_for_camera && m.meta.is_some() {
                    self.develop_chart_style(name, &facts);
                }
                self.develop_say(t!(
                    "develop-chart-saved",
                    name = name.to_owned(),
                    file = file.display().to_string()
                ));
                self.develop_chart_close();
                self.develop_cards();
            }
            Err(e) => self
                .chrome
                .announce_status(t!("develop-chart-save-failed", reason = e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn develop_chart_write_dcp(
        &mut self,
        dir: &Path,
        stem: &str,
        name: &str,
        key: &str,
        m: &Measured,
        meta: &SensorMeta,
        ms: i64,
    ) -> Result<PathBuf, String> {
        use specular_develop::chart::output;
        let facts = self
            .develop
            .session
            .as_ref()
            .map(|s| super::batch::photo_facts(&s.path))
            .unwrap_or_default();
        let raw_neutral = chart::raw_neutral(m.fit.neutral, meta);
        let shot = output::Shot {
            fit: &m.fit,
            raw_neutral,
            white_xy: chart::white_xy(meta, &facts.maker, &facts.model, raw_neutral),
        };
        let dcp = output::dcp(key, name, &[shot]).ok_or_else(|| t!("develop-chart-degenerate"))?;
        let bytes = dcp.write();
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = unique_path(dir, stem, "dcp");
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        let hash = output::content_hash(&bytes);
        if let Some(s) = self.develop.session.as_mut() {
            s.set_input_profile_file(&path, &hash, ms);
        }
        Ok(path)
    }

    fn develop_chart_write_internal(
        &mut self,
        dir: &Path,
        stem: &str,
        m: &Measured,
        ms: i64,
    ) -> Result<PathBuf, String> {
        use specular_develop::chart::output;
        let cube = output::cube(&m.fit, output::CubeModel::Table);
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = unique_path(dir, stem, "cube");
        std::fs::write(&path, cube.as_bytes()).map_err(|e| e.to_string())?;
        if let Some(s) = self.develop.session.as_mut() {
            s.set_chart_mixer(output::mixer(&m.fit), &path, ms);
        }
        Ok(path)
    }

    /// "Use for photos from this camera": a style with only the camera profile, and an
    /// auto-apply rule for the camera.
    fn develop_chart_style(&mut self, name: &str, f: &specular_develop::model::PhotoFacts) {
        use specular_develop::model::{AutoRule, FileType, ModuleKind, Style};
        let Some(e) = self.develop.session.as_ref().map(|s| s.edit.clone()) else {
            return;
        };
        let style = Style::from_kinds(name, &e, &[ModuleKind::RAW_FRONT_END]);
        let shelf = &mut self.develop.batch.shelf;
        shelf.load();
        shelf.put_style(style);
        shelf.rules.push(AutoRule {
            style: name.to_owned(),
            maker: f.maker.clone(),
            model: f.model.clone(),
            file_type: FileType::Raw,
            ..AutoRule::default()
        });
        shelf.save_rules();
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// X-Rite's published ColorChecker values (test data only).
    const CHECKER_LAB: [[f64; 3]; 24] = [
        [37.54, 14.37, 14.92],
        [64.66, 19.27, 17.5],
        [49.32, -3.82, -22.54],
        [43.46, -12.74, 22.72],
        [54.94, 9.61, -24.79],
        [70.48, -32.26, -0.37],
        [62.73, 35.83, 56.5],
        [39.43, 10.75, -45.17],
        [50.57, 48.64, 16.67],
        [30.1, 22.54, -20.87],
        [71.77, -24.13, 58.19],
        [71.51, 18.24, 67.37],
        [28.37, 15.42, -49.8],
        [54.38, -39.72, 32.27],
        [42.43, 51.05, 28.62],
        [81.8, 2.67, 80.41],
        [50.63, 51.28, -14.12],
        [49.57, -29.71, -28.32],
        [95.19, -1.03, 2.93],
        [81.29, -0.57, 0.44],
        [66.89, -0.75, -0.06],
        [50.76, -0.13, 0.14],
        [35.63, -0.46, -0.48],
        [20.64, 0.07, -0.46],
    ];

    pub(in crate::app) fn cc24_file() -> Vec<u8> {
        let mut s = String::from(
            "CGATS.17\nORIGINATOR \"test\"\nBEGIN_DATA_FORMAT\nSAMPLE_ID LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nNUMBER_OF_SETS 24\nBEGIN_DATA\n",
        );
        for (i, l) in CHECKER_LAB.iter().enumerate() {
            s.push_str(&format!("{} {} {} {}\n", i + 1, l[0], l[1], l[2]));
        }
        s.push_str("END_DATA\n");
        s.into_bytes()
    }

    #[test]
    fn reference_then_four_corners_in_order() {
        let mut f = Flow::new(true, "Canon EOS R5".into());
        assert_eq!(f.step, Step::Reference);
        f.load_reference("ColorChecker.txt".into(), b"not a file");
        assert_eq!(f.step, Step::Reference);
        assert!(f.problem.is_some());
        f.load_reference("ColorChecker.txt".into(), &cc24_file());
        assert_eq!(f.step, Step::Marking);
        assert!(f.problem.is_none());
        let names: Vec<String> = (0..4).map(|i| f.corner_name(i)).collect();
        assert_eq!(names[0], t!("develop-chart-corner-dark-skin"));
        assert_eq!(names[1], t!("develop-chart-corner-bluish-green"));
        assert_eq!(names[2], t!("develop-chart-corner-black"));
        assert_eq!(names[3], t!("develop-chart-corner-white"));
        assert!(f.prompt().contains(&names[0]));
        for i in 0..3 {
            assert!(!f.click((0.1 * i as f64, 0.2)));
        }
        assert!(f.click((0.5, 0.5)));
        assert_eq!(f.step, Step::Measuring);
        f.measured(Err("no chart".into()));
        assert_eq!(f.step, Step::Failed("no chart".into()));
        f.again();
        assert_eq!(f.step, Step::Marking);
        assert!(f.clicks.is_empty());
    }

    #[test]
    fn spyder_is_refused_with_a_reason() {
        let mut f = Flow::new(false, String::new());
        f.load_reference("ColorChecker.txt".into(), &cc24_file());
        while f.kind != ChartKind::SpyderCheckr {
            f.next_kind();
        }
        assert_eq!(f.step, Step::Reference);
        assert_eq!(
            f.problem.as_deref(),
            Some(t!("develop-chart-spyder").as_str())
        );
        f.next_kind();
        assert_eq!(f.kind, ChartKind::ColorChecker24);
        assert_eq!(f.step, Step::Marking);
    }

    #[test]
    fn unique_paths_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("specular-chart-app-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = unique_path(&dir, "Studio", "dcp");
        std::fs::write(&a, b"x").unwrap();
        let b = unique_path(&dir, "Studio", "dcp");
        assert_ne!(a, b);
        assert!(b.ends_with("Studio 2.dcp"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
