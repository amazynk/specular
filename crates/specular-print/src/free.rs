//! Free layouts: a sheet with image areas placed by hand (pure maths, no drawing).
//!
//! A [`FreeLayout`] is a paper, margins, a gap and a list of [`Area`]s in mm from the
//! sheet's top-left corner. Each area shows one photo, fitted or filling it with a crop
//! pan ([`place`]). Photos fill the areas in reading order ([`FreeLayout::fill_order`]),
//! page after page ([`FreeLayout::document`]), so the result goes through the same page
//! renderer and PDF writer as the other layouts.
//!
//! Areas are moved and resized with [`drag`], which snaps edges to the margins, the
//! sheet's centre lines, the other areas (edge to edge, or a gap apart) and a grid.
//! Layouts are saved by name as plain text ([`write_all`], [`parse_all`]), one section per
//! layout; [`builtin`] gives the shipped ones.

use crate::layout::{
    Document, Item, MM_PER_IN, Page, PageOrientation, PageSetup, Paper, Placed, RectMm,
};

/// Effective resolutions below this (dpi) are flagged as too low for a good print.
pub const LOW_DPI: f64 = 200.0;
/// The smallest area side (mm).
pub const MIN_AREA_MM: f64 = 5.0;

/// How a photo sits in its area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// Whole, as large as fits, centred.
    #[default]
    Fit,
    /// Covering the area, cropped; the crop moves with the area's pan.
    Fill,
}

/// One image area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    /// On the sheet, mm.
    pub rect: RectMm,
    pub placement: Placement,
    /// Where the crop of a filled photo sits, per axis: -1 its left (top) part, 0 the
    /// middle, 1 the right (bottom) part.
    pub pan: (f64, f64),
}

impl Area {
    pub fn new(rect: RectMm, placement: Placement) -> Area {
        Area {
            rect,
            placement,
            pan: (0.0, 0.0),
        }
    }
}

/// A sheet with image areas.
#[derive(Clone, Debug, PartialEq)]
pub struct FreeLayout {
    /// A shipped layout's stable id (its name is translated); empty for the user's.
    pub id: String,
    pub name: String,
    pub paper: Paper,
    pub orientation: PageOrientation,
    /// Margin on every side (mm); ignored when borderless.
    pub margin: f64,
    /// The gap areas keep between them when snapped, and in shipped grids (mm).
    pub gap: f64,
    /// Printed to the paper's edge (only offered when the printer says it can).
    pub borderless: bool,
    pub areas: Vec<Area>,
}

impl Default for FreeLayout {
    fn default() -> Self {
        FreeLayout {
            id: String::new(),
            name: String::new(),
            paper: Paper::A4,
            orientation: PageOrientation::Portrait,
            margin: 10.0,
            gap: 5.0,
            borderless: false,
            areas: Vec::new(),
        }
    }
}

impl FreeLayout {
    /// The sheet, oriented (mm).
    pub fn sheet_mm(&self) -> (f64, f64) {
        self.setup(300).size_mm()
    }

    /// The margin in effect: none when borderless, else at most a third of the shorter side.
    pub fn margin_mm(&self) -> f64 {
        if self.borderless {
            0.0
        } else {
            let (w, h) = self.sheet_mm();
            self.margin.clamp(0.0, w.min(h) / 3.0)
        }
    }

    /// The page setup at `dpi`.
    pub fn setup(&self, dpi: u32) -> PageSetup {
        PageSetup {
            paper: self.paper,
            orientation: self.orientation,
            margin: if self.borderless { 0.0 } else { self.margin },
            dpi,
        }
    }

    /// The area inside the margins (the whole sheet when borderless).
    pub fn printable(&self) -> RectMm {
        let (w, h) = self.sheet_mm();
        let m = self.margin_mm();
        RectMm::new(m, m, (w - 2.0 * m).max(1.0), (h - 2.0 * m).max(1.0))
    }

    /// Replaces the areas with a `cols`×`rows` grid filling the printable area, `gap` apart.
    pub fn grid(&mut self, cols: u32, rows: u32, placement: Placement) {
        let (cols, rows) = (cols.max(1), rows.max(1));
        let p = self.printable();
        let gap = self.gap.max(0.0);
        let cw = ((p.w - gap * f64::from(cols - 1)) / f64::from(cols)).max(MIN_AREA_MM);
        let ch = ((p.h - gap * f64::from(rows - 1)) / f64::from(rows)).max(MIN_AREA_MM);
        self.areas = (0..rows)
            .flat_map(|r| (0..cols).map(move |c| (c, r)))
            .map(|(c, r)| {
                Area::new(
                    RectMm::new(
                        p.x + f64::from(c) * (cw + gap),
                        p.y + f64::from(r) * (ch + gap),
                        cw,
                        ch,
                    ),
                    placement,
                )
            })
            .collect();
    }

    /// Changes the paper or orientation: the areas keep their place relative to the
    /// printable area (a grid still fills it).
    pub fn set_sheet(&mut self, paper: Paper, orientation: PageOrientation) {
        let from = self.printable();
        self.paper = paper;
        self.orientation = orientation;
        let to = self.printable();
        let (sx, sy) = (to.w / from.w.max(1e-9), to.h / from.h.max(1e-9));
        for a in &mut self.areas {
            let r = a.rect;
            a.rect = RectMm::new(
                to.x + (r.x - from.x) * sx,
                to.y + (r.y - from.y) * sy,
                (r.w * sx).max(MIN_AREA_MM),
                (r.h * sy).max(MIN_AREA_MM),
            );
        }
        self.keep_on_sheet();
    }

    /// Changes the margin (or borderless): area edges that lay on the old margin lines
    /// move to the new ones.
    pub fn set_margin(&mut self, margin: f64, borderless: bool) {
        let old = self.printable();
        self.margin = margin.clamp(0.0, 100.0);
        self.borderless = borderless;
        let new = self.printable();
        let on = |a: f64, b: f64| (a - b).abs() < 0.01;
        for a in &mut self.areas {
            let r = a.rect;
            let (mut l, mut t, mut rt, mut b) = (r.x, r.y, r.right(), r.bottom());
            if on(l, old.x) {
                l = new.x;
            }
            if on(t, old.y) {
                t = new.y;
            }
            if on(rt, old.right()) {
                rt = new.right();
            }
            if on(b, old.bottom()) {
                b = new.bottom();
            }
            if rt - l >= MIN_AREA_MM && b - t >= MIN_AREA_MM {
                a.rect = RectMm::new(l, t, rt - l, b - t);
            }
        }
        self.keep_on_sheet();
    }

    /// Keeps every area on the sheet and at least the smallest size.
    pub fn keep_on_sheet(&mut self) {
        let (w, h) = self.sheet_mm();
        for a in &mut self.areas {
            a.rect = clamp_to(a.rect, w, h);
        }
    }

    /// A new area: in the middle of the printable area, half its size, nudged so it does
    /// not cover the last one exactly.
    pub fn add_area(&mut self) -> usize {
        let p = self.printable();
        let mut r = p.centred((p.w * 0.5).max(MIN_AREA_MM), (p.h * 0.5).max(MIN_AREA_MM));
        while self
            .areas
            .iter()
            .any(|a| (a.rect.x - r.x).abs() < 0.5 && (a.rect.y - r.y).abs() < 0.5)
            && r.right() + 5.0 <= p.right()
            && r.bottom() + 5.0 <= p.bottom()
        {
            r.x += 5.0;
            r.y += 5.0;
        }
        self.areas.push(Area::new(r, Placement::Fill));
        self.areas.len() - 1
    }

    /// The areas in the order photos fill them: reading order, rows from the top, within a
    /// row from the left (from the right when `rtl`). Areas whose tops are within half the
    /// shorter one's height share a row.
    pub fn fill_order(&self, rtl: bool) -> Vec<usize> {
        let mut by_top: Vec<usize> = (0..self.areas.len()).collect();
        by_top.sort_by(|&a, &b| self.areas[a].rect.y.total_cmp(&self.areas[b].rect.y));
        let mut out = Vec::with_capacity(by_top.len());
        let mut row: Vec<usize> = Vec::new();
        let flush = |row: &mut Vec<usize>, out: &mut Vec<usize>| {
            row.sort_by(|&a, &b| {
                let (x, y) = (self.areas[a].rect.x, self.areas[b].rect.x);
                if rtl {
                    (y + self.areas[b].rect.w).total_cmp(&(x + self.areas[a].rect.w))
                } else {
                    x.total_cmp(&y)
                }
            });
            out.append(row);
        };
        for i in by_top {
            let r = self.areas[i].rect;
            if let Some(&first) = row.first() {
                let f = self.areas[first].rect;
                if (r.y - f.y).abs() > 0.5 * r.h.min(f.h) {
                    flush(&mut row, &mut out);
                }
            }
            row.push(i);
        }
        flush(&mut row, &mut out);
        out
    }

    /// The pages: `items` fill the areas in [`fill_order`](Self::fill_order), one page
    /// after another; a layout without areas, or no items, gives one empty page. Filled
    /// photos are clipped to their area, and to the margins unless borderless.
    pub fn document(&self, dpi: u32, items: &[Item], rtl: bool) -> Document {
        let setup = self.setup(dpi);
        let order = self.fill_order(rtl);
        let n = order.len();
        let printable = self.printable();
        let pages = if n == 0 {
            1
        } else {
            items.len().div_ceil(n).max(1)
        };
        let pages = (0..pages)
            .map(|p| Page {
                images: order
                    .iter()
                    .enumerate()
                    .filter_map(|(k, &a)| {
                        let i = p * n + k;
                        let item = items.get(i)?;
                        let area = &self.areas[a];
                        let (image, clip) = place(area, item.size);
                        let clip = clip.intersect(&printable);
                        (!clip.is_empty()).then(|| Placed {
                            item: i,
                            image,
                            clip,
                            caption: Vec::new(),
                            caption_rect: RectMm::default(),
                        })
                    })
                    .collect(),
                header: None,
                scale: None,
            })
            .collect();
        Document { setup, pages, rtl }
    }

    /// Which area a photo `item` (in selection order) lands in, and on which page.
    pub fn area_of(&self, item: usize, rtl: bool) -> Option<(usize, usize)> {
        let order = self.fill_order(rtl);
        let n = order.len();
        (n > 0).then(|| (item / n, order[item % n]))
    }
}

/// `r` kept on a `w`×`h` sheet with at least the smallest size (moved, then shrunk).
fn clamp_to(r: RectMm, w: f64, h: f64) -> RectMm {
    let rw = r.w.clamp(MIN_AREA_MM.min(w), w);
    let rh = r.h.clamp(MIN_AREA_MM.min(h), h);
    RectMm::new(r.x.clamp(0.0, w - rw), r.y.clamp(0.0, h - rh), rw, rh)
}

/// The photo's rectangle in `area` and what of it shows: fitted and centred, or covering
/// the area with its crop at the area's pan.
pub fn place(area: &Area, size: (u32, u32)) -> (RectMm, RectMm) {
    let a = area.rect;
    let (iw, ih) = (f64::from(size.0.max(1)), f64::from(size.1.max(1)));
    match area.placement {
        Placement::Fit => {
            let s = (a.w / iw).min(a.h / ih);
            let r = a.centred(iw * s, ih * s);
            (r, r)
        }
        Placement::Fill => {
            let s = (a.w / iw).max(a.h / ih);
            let (w, h) = (iw * s, ih * s);
            let (ox, oy) = (w - a.w, h - a.h);
            let px = area.pan.0.clamp(-1.0, 1.0);
            let py = area.pan.1.clamp(-1.0, 1.0);
            let r = RectMm::new(
                a.x - ox * (px + 1.0) * 0.5,
                a.y - oy * (py + 1.0) * 0.5,
                w,
                h,
            );
            (r, a)
        }
    }
}

/// The pan after dragging a filled photo by `(dx, dy)` mm (the photo follows the pointer);
/// an axis with nothing to crop stays centred.
pub fn pan_by(area: &Area, size: (u32, u32), (dx, dy): (f64, f64)) -> (f64, f64) {
    let (image, _) = place(
        &Area {
            placement: Placement::Fill,
            ..*area
        },
        size,
    );
    let (ox, oy) = (image.w - area.rect.w, image.h - area.rect.h);
    let axis = |pan: f64, d: f64, over: f64| {
        if over < 1e-6 {
            0.0
        } else {
            (pan - 2.0 * d / over).clamp(-1.0, 1.0)
        }
    };
    (axis(area.pan.0, dx, ox), axis(area.pan.1, dy, oy))
}

/// The photo's own pixels per inch when its `size` (pixels) is printed as `image` (mm).
pub fn effective_dpi(size: (u32, u32), image: &RectMm) -> f64 {
    let inches = image.w.max(1e-6) / MM_PER_IN;
    f64::from(size.0.max(1)) / inches
}

/// Which part of an area a drag holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Move,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Handle {
    /// The edges it moves: left, top, right, bottom.
    fn edges(self) -> [bool; 4] {
        match self {
            Handle::Move => [true, true, true, true],
            Handle::N => [false, true, false, false],
            Handle::S => [false, false, false, true],
            Handle::E => [false, false, true, false],
            Handle::W => [true, false, false, false],
            Handle::NE => [false, true, true, false],
            Handle::NW => [true, true, false, false],
            Handle::SE => [false, false, true, true],
            Handle::SW => [true, false, false, true],
        }
    }

    /// The eight resize handles, in drawing order.
    pub const RESIZE: [Handle; 8] = [
        Handle::NW,
        Handle::N,
        Handle::NE,
        Handle::E,
        Handle::SE,
        Handle::S,
        Handle::SW,
        Handle::W,
    ];

    /// Where the handle sits on `r` (fractions of its width and height).
    pub fn at(self) -> (f64, f64) {
        match self {
            Handle::Move => (0.5, 0.5),
            Handle::N => (0.5, 0.0),
            Handle::S => (0.5, 1.0),
            Handle::E => (1.0, 0.5),
            Handle::W => (0.0, 0.5),
            Handle::NE => (1.0, 0.0),
            Handle::NW => (0.0, 0.0),
            Handle::SE => (1.0, 1.0),
            Handle::SW => (0.0, 1.0),
        }
    }
}

/// What edges snap to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    /// How near (mm) an edge must come to a line to snap to it.
    pub threshold: f64,
    /// Snap to the margins, the sheet's centre lines and the other areas.
    pub guides: bool,
    /// Snap to a grid of this step (mm) where no guide is near.
    pub grid: Option<f64>,
}

impl Snap {
    pub const OFF: Snap = Snap {
        threshold: 0.0,
        guides: false,
        grid: None,
    };
}

/// A line an edge snapped to, to draw while dragging.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// A vertical line at this x (mm).
    X(f64),
    /// A horizontal line at this y (mm).
    Y(f64),
}

/// The lines edges snap to across one axis (x when `vertical`), for area `index`.
fn targets(layout: &FreeLayout, index: usize, vertical: bool) -> Vec<f64> {
    let (w, h) = layout.sheet_mm();
    let len = if vertical { w } else { h };
    let m = layout.margin_mm();
    let mut t = vec![0.0, len, len * 0.5];
    if m > 0.0 {
        t.extend([m, len - m]);
    }
    for (i, a) in layout.areas.iter().enumerate() {
        if i == index {
            continue;
        }
        let (lo, hi) = if vertical {
            (a.rect.x, a.rect.right())
        } else {
            (a.rect.y, a.rect.bottom())
        };
        t.extend([lo, hi, (lo + hi) * 0.5]);
        if layout.gap > 0.0 {
            t.extend([lo - layout.gap, hi + layout.gap]);
        }
    }
    t
}

/// The nearest of `targets` to any of `values` within `threshold`: (target, which value).
fn nearest(values: &[f64], targets: &[f64], threshold: f64) -> Option<(f64, usize)> {
    let mut best: Option<(f64, usize, f64)> = None;
    for (k, &v) in values.iter().enumerate() {
        for &t in targets {
            let d = (v - t).abs();
            if d <= threshold && best.is_none_or(|b| d < b.2 - 1e-9) {
                best = Some((t, k, d));
            }
        }
    }
    best.map(|b| (b.0, b.1))
}

/// Area `index` after a drag of `handle` by `delta` (mm) from where it was (`start`),
/// snapped: a guide within the threshold wins, else the grid. Returns the new rectangle,
/// kept on the sheet and at least [`MIN_AREA_MM`] a side, and the guides it snapped to.
pub fn drag(
    layout: &FreeLayout,
    index: usize,
    start: RectMm,
    handle: Handle,
    delta: (f64, f64),
    snap: &Snap,
) -> (RectMm, Vec<Guide>) {
    let (sw, sh) = layout.sheet_mm();
    let [ml, mt, mr, mb] = handle.edges();
    let mut guides = Vec::new();
    // One axis: (low edge, high edge) moved, then snapped.
    let mut axis = |lo: f64, hi: f64, d: f64, move_lo: bool, move_hi: bool, vertical: bool| {
        let len = if vertical { sw } else { sh };
        let t = if snap.guides {
            targets(layout, index, vertical)
        } else {
            Vec::new()
        };
        let grid = |v: f64| snap.grid.filter(|g| *g > 0.0).map(|g| (v / g).round() * g);
        let guide = |v: f64| if vertical { Guide::X(v) } else { Guide::Y(v) };
        if move_lo && move_hi {
            let size = hi - lo;
            let mut x = (lo + d).clamp(0.0, (len - size).max(0.0));
            if let Some((tv, k)) = nearest(&[x, x + size, x + size * 0.5], &t, snap.threshold) {
                x = tv - [0.0, size, size * 0.5][k];
                guides.push(guide(tv));
            } else if let Some(g) = grid(x) {
                x = g;
            }
            let x = x.clamp(0.0, (len - size).max(0.0));
            (x, x + size)
        } else if move_lo {
            let mut v = (lo + d).clamp(0.0, hi - MIN_AREA_MM);
            if let Some((tv, _)) = nearest(&[v], &t, snap.threshold) {
                v = tv;
                guides.push(guide(tv));
            } else if let Some(g) = grid(v) {
                v = g;
            }
            (v.clamp(0.0, hi - MIN_AREA_MM), hi)
        } else if move_hi {
            let mut v = (hi + d).clamp(lo + MIN_AREA_MM, len);
            if let Some((tv, _)) = nearest(&[v], &t, snap.threshold) {
                v = tv;
                guides.push(guide(tv));
            } else if let Some(g) = grid(v) {
                v = g;
            }
            (lo, v.clamp(lo + MIN_AREA_MM, len))
        } else {
            (lo, hi)
        }
    };
    let (l, r) = axis(start.x, start.right(), delta.0, ml, mr, true);
    let (t, b) = axis(start.y, start.bottom(), delta.1, mt, mb, false);
    let rect = clamp_to(RectMm::new(l, t, r - l, b - t), sw, sh);
    (rect, guides)
}

/// The shipped layouts on `paper` (the locale's): one with a border, two up, four up, a
/// 3×3 contact sheet; then 8×10 in on Letter and A4 with a mat.
pub fn builtin(paper: Paper) -> Vec<FreeLayout> {
    let base = |id: &str, name: &str, paper: Paper| FreeLayout {
        id: id.into(),
        name: name.into(),
        paper,
        ..FreeLayout::default()
    };
    let mut one = FreeLayout {
        margin: 20.0,
        ..base("one-border", "1-up with a border", paper)
    };
    one.grid(1, 1, Placement::Fit);
    let mut two = base("two-up", "2-up", paper);
    two.grid(1, 2, Placement::Fill);
    let mut four = base("four-up", "4-up", paper);
    four.grid(2, 2, Placement::Fill);
    let mut sheet = FreeLayout {
        gap: 4.0,
        ..base("contact-3x3", "3×3 contact sheet", paper)
    };
    sheet.grid(3, 3, Placement::Fit);
    // 8×10 in, centred on Letter.
    let mut letter = FreeLayout {
        margin: 6.35,
        ..base("8x10-letter", "8×10 on Letter", Paper::Letter)
    };
    let (lw, lh) = letter.sheet_mm();
    letter.areas = vec![Area::new(
        RectMm::new(0.0, 0.0, lw, lh).centred(8.0 * MM_PER_IN, 10.0 * MM_PER_IN),
        Placement::Fill,
    )];
    // A4 with a mat: wide even borders, a little more below (as a framer cuts a mat).
    let mut mat = FreeLayout {
        margin: 30.0,
        ..base("a4-mat", "A4 with a mat", Paper::A4)
    };
    let (aw, ah) = mat.sheet_mm();
    mat.areas = vec![Area::new(
        RectMm::new(30.0, 30.0, aw - 60.0, ah - 70.0),
        Placement::Fit,
    )];
    vec![one, two, four, sheet, letter, mat]
}

fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
    if s == "-0" { "0".into() } else { s }
}

/// One layout as text (see [`write_all`]).
pub fn to_text(l: &FreeLayout) -> String {
    let name: String = l
        .name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut s = format!("[{name}]\n");
    if !l.id.is_empty() {
        s.push_str(&format!("id = {}\n", l.id));
    }
    let paper = match l.paper {
        Paper::A4 => "a4".to_owned(),
        Paper::Letter => "letter".to_owned(),
        Paper::A3 => "a3".to_owned(),
        Paper::Custom { w, h } => format!("custom {} {}", num(w), num(h)),
    };
    s.push_str(&format!("paper = {paper}\n"));
    s.push_str(&format!(
        "orientation = {}\n",
        match l.orientation {
            PageOrientation::Portrait => "portrait",
            PageOrientation::Landscape => "landscape",
        }
    ));
    s.push_str(&format!("margin = {}\n", num(l.margin)));
    s.push_str(&format!("gap = {}\n", num(l.gap)));
    s.push_str(&format!(
        "borderless = {}\n",
        if l.borderless { "yes" } else { "no" }
    ));
    for a in &l.areas {
        let r = a.rect;
        s.push_str(&format!(
            "area = {} {} {} {} {} {} {}\n",
            num(r.x),
            num(r.y),
            num(r.w),
            num(r.h),
            match a.placement {
                Placement::Fit => "fit",
                Placement::Fill => "fill",
            },
            num(a.pan.0),
            num(a.pan.1)
        ));
    }
    s
}

/// Layouts as text, one section per layout:
///
/// ```text
/// [Two portraits]
/// paper = a4
/// orientation = landscape
/// margin = 10
/// gap = 5
/// borderless = no
/// area = 10 10 136 190 fill 0 -0.5
/// area = 151 10 136 190 fill 0 0
/// ```
///
/// `area` is x, y, width, height (mm from the sheet's top left), `fit` or `fill`, and the
/// crop's pan across and down (-1…1).
pub fn write_all(layouts: &[FreeLayout]) -> String {
    layouts.iter().map(to_text).collect::<Vec<_>>().join("\n")
}

/// Reads [`write_all`]'s text. Unknown keys and bad values are skipped, so a file from a
/// newer Specular still loads; a section without a name is dropped.
pub fn parse_all(text: &str) -> Vec<FreeLayout> {
    let mut out: Vec<FreeLayout> = Vec::new();
    let mut cur: Option<FreeLayout> = None;
    let f = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite());
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[')
            && let Some(end) = rest.rfind(']')
        {
            out.extend(cur.take().filter(|l| !l.name.trim().is_empty()));
            cur = Some(FreeLayout {
                name: rest[..end].trim().to_owned(),
                areas: Vec::new(),
                ..FreeLayout::default()
            });
            continue;
        }
        let (Some(l), Some((k, v))) = (cur.as_mut(), line.split_once('=')) else {
            continue;
        };
        let v = v.trim();
        let words: Vec<&str> = v.split_whitespace().collect();
        match k.trim() {
            "id" => l.id = v.to_owned(),
            "paper" => match words.as_slice() {
                ["a4"] => l.paper = Paper::A4,
                ["letter"] => l.paper = Paper::Letter,
                ["a3"] => l.paper = Paper::A3,
                ["custom", w, h] => {
                    if let (Some(w), Some(h)) = (f(w), f(h)) {
                        l.paper = Paper::Custom { w, h };
                    }
                }
                _ => {}
            },
            "orientation" => match v {
                "portrait" => l.orientation = PageOrientation::Portrait,
                "landscape" => l.orientation = PageOrientation::Landscape,
                _ => {}
            },
            "margin" => {
                if let Some(m) = f(v).filter(|m| (0.0..=100.0).contains(m)) {
                    l.margin = m;
                }
            }
            "gap" => {
                if let Some(g) = f(v).filter(|g| (0.0..=100.0).contains(g)) {
                    l.gap = g;
                }
            }
            "borderless" => l.borderless = v == "yes",
            "area" => {
                let n: Vec<Option<f64>> = words.iter().map(|w| f(w)).collect();
                if let [Some(x), Some(y), Some(w), Some(h), ..] = n[..]
                    && w > 0.0
                    && h > 0.0
                {
                    let placement = match words.get(4) {
                        Some(&"fill") => Placement::Fill,
                        _ => Placement::Fit,
                    };
                    let pan = (
                        n.get(5).copied().flatten().unwrap_or(0.0).clamp(-1.0, 1.0),
                        n.get(6).copied().flatten().unwrap_or(0.0).clamp(-1.0, 1.0),
                    );
                    l.areas.push(Area {
                        rect: RectMm::new(x, y, w, h),
                        placement,
                        pan,
                    });
                }
            }
            _ => {}
        }
    }
    out.extend(cur.take().filter(|l| !l.name.trim().is_empty()));
    for l in &mut out {
        l.keep_on_sheet();
    }
    out
}

#[cfg(test)]
mod tests;
