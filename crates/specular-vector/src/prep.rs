//! Preparing a scene for drawing: linetype dashes applied, text laid out into glyph
//! placements, points turned into their display shape, and everything packed into a
//! compact display list ([`Geometry`]) that a renderer walks once per frame.
//!
//! Paths are stored as `f32` relative to the sheet's centre, which keeps detail to about
//! 1e-7 of the drawing's size; glyph outlines are shared and placed with a transform.
//! A layout draws the model geometry once per viewport ([`Pass`]) and then its own.

use std::collections::HashMap;
use std::sync::Arc;

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Vec2};

use crate::fonts::{Fonts, Glyph};
use crate::scene::{
    CellRef, Color, FontSpec, HAlign, Item, Lineweight, Shape, Sheet, SheetKind, TextBlock, VAlign,
    VectorScene,
};

/// How an item is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Stroke,
    Fill,
    FillEvenOdd,
    /// A glyph outline placed with a transform (non-zero fill).
    Glyph,
    /// A dot of fixed screen size (a point entity in display mode 0).
    Dot,
}

/// One entry of the display list.
#[derive(Clone, Copy, Debug)]
pub struct DrawItem {
    pub kind: Kind,
    pub color: Color,
    /// Strokes: the lineweight. Dots: the radius in hundredths of a CSS pixel (0 for 1 px).
    pub weight: Lineweight,
    pub layer: u32,
    pub outer: u32,
    /// Paths: verbs `start..end`, points from `pts`. Glyphs: the placement `start`. Dots:
    /// the point `pts`.
    pub start: u32,
    pub end: u32,
    pub pts: u32,
    /// Bounds relative to [`Geometry::origin`]: min x, min y, max x, max y.
    pub bbox: [f32; 4],
}

impl DrawItem {
    /// The bounds as a rectangle relative to the origin.
    pub fn rect(&self) -> Rect {
        let b = self.bbox;
        Rect::new(
            f64::from(b[0]),
            f64::from(b[1]),
            f64::from(b[2]),
            f64::from(b[3]),
        )
    }
}

pub(crate) const MOVE: u8 = 0;
pub(crate) const LINE: u8 = 1;
const QUAD: u8 = 2;
const CUBIC: u8 = 3;
pub(crate) const CLOSE: u8 = 4;

/// The display list of one sheet's own items.
#[derive(Debug, Default)]
pub struct Geometry {
    /// Item coordinates are relative to this point of the sheet.
    pub origin: Point,
    verbs: Vec<u8>,
    pts: Vec<[f32; 2]>,
    glyphs: Vec<Arc<Glyph>>,
    placements: Vec<(u32, [f32; 6])>,
    pub items: Vec<DrawItem>,
    /// Bounds of everything (instances included), sheet coordinates (cell coordinates for
    /// a layout cell).
    pub bounds: Option<Rect>,
    /// Layout cells placed here, drawn after the items.
    pub instances: Vec<Instance>,
    /// Colour and layer that stand for this geometry when it is too small to draw (the
    /// layer covering the most area).
    pub tint: Option<(Color, u32)>,
    /// Fill opacity with outlines ([`crate::scene::Layout::fill_alpha`]); 0 for opaque
    /// fills.
    pub fill_alpha: u8,
    /// Glyphs smaller than this many pixels are not drawn.
    pub label_min_px: f32,
    /// Runs of items on one layer draw as groups at these opacities
    /// ([`crate::scene::VectorScene::layer_opacity`]); empty for no groups.
    pub layer_opacity: Vec<f32>,
}

/// A layout cell placed in a [`Geometry`] (once or as an array).
#[derive(Clone, Debug)]
pub struct Instance {
    pub geometry: Arc<Geometry>,
    /// The cell's item coordinates → the parent's item coordinates.
    pub transform: Affine,
    /// Columns, rows and the steps between them (parent coordinates).
    pub array: Option<(u32, u32, Vec2, Vec2)>,
    /// Bounds of every element, parent item coordinates.
    pub bbox: Rect,
}

impl Instance {
    /// Bounds of the first element, parent item coordinates.
    pub fn element_bbox(&self) -> Option<Rect> {
        let b = self.geometry.bounds? - self.geometry.origin.to_vec2();
        Some(self.transform.transform_rect_bbox(b))
    }
}

impl Geometry {
    /// The path of a stroke or fill item.
    pub fn path(&self, it: &DrawItem) -> PathRef<'_> {
        let (s, e) = (it.start as usize, it.end as usize);
        PathRef {
            verbs: self.verbs.get(s..e).unwrap_or(&[]),
            pts: self.pts.get(it.pts as usize..).unwrap_or(&[]),
        }
    }

    /// The outline (font units) and its placement (font units → item coordinates) of a
    /// glyph item.
    pub fn glyph(&self, it: &DrawItem) -> Option<(&BezPath, Affine)> {
        let (g, m) = self.placements.get(it.start as usize)?;
        let glyph = self.glyphs.get(*g as usize)?;
        Some((&glyph.path, Affine::new(m.map(f64::from))))
    }

    /// The position of a dot item, relative to the origin.
    pub fn dot(&self, it: &DrawItem) -> Point {
        let p = self.pts.get(it.pts as usize).copied().unwrap_or([0.0; 2]);
        Point::new(f64::from(p[0]), f64::from(p[1]))
    }

    /// Bytes held, roughly.
    pub fn heap_bytes(&self) -> usize {
        self.verbs.len()
            + self.pts.len() * 8
            + self.placements.len() * 28
            + self.items.len() * std::mem::size_of::<DrawItem>()
            + self.instances.len() * std::mem::size_of::<Instance>()
            + self
                .glyphs
                .iter()
                .map(|g| g.path.elements().len() * 56)
                .sum::<usize>()
    }
}

/// A path in the compact store, usable wherever a `kurbo::Shape` is.
#[derive(Clone, Copy, Debug)]
pub struct PathRef<'a> {
    pub(crate) verbs: &'a [u8],
    pub(crate) pts: &'a [[f32; 2]],
}

/// Iterates a [`PathRef`].
#[derive(Clone, Debug)]
pub struct PathIter<'a> {
    verbs: std::slice::Iter<'a, u8>,
    pts: &'a [[f32; 2]],
    i: usize,
}

impl PathIter<'_> {
    fn pt(&mut self) -> Point {
        let p = self.pts.get(self.i).copied().unwrap_or([0.0; 2]);
        self.i += 1;
        Point::new(f64::from(p[0]), f64::from(p[1]))
    }
}

impl Iterator for PathIter<'_> {
    type Item = PathEl;
    fn next(&mut self) -> Option<PathEl> {
        Some(match *self.verbs.next()? {
            MOVE => PathEl::MoveTo(self.pt()),
            LINE => PathEl::LineTo(self.pt()),
            QUAD => {
                let a = self.pt();
                PathEl::QuadTo(a, self.pt())
            }
            CUBIC => {
                let a = self.pt();
                let b = self.pt();
                PathEl::CurveTo(a, b, self.pt())
            }
            _ => PathEl::ClosePath,
        })
    }
}

impl<'a> PathRef<'a> {
    pub fn iter(&self) -> PathIter<'a> {
        PathIter {
            verbs: self.verbs.iter(),
            pts: self.pts,
            i: 0,
        }
    }

    pub fn to_bez(&self) -> BezPath {
        BezPath::from_vec(self.iter().collect())
    }
}

impl kurbo::Shape for PathRef<'_> {
    type PathElementsIter<'iter>
        = PathIter<'iter>
    where
        Self: 'iter;

    fn path_elements(&self, _tolerance: f64) -> PathIter<'_> {
        self.iter()
    }
    fn area(&self) -> f64 {
        self.to_bez().area()
    }
    fn perimeter(&self, accuracy: f64) -> f64 {
        self.to_bez().perimeter(accuracy)
    }
    fn winding(&self, pt: Point) -> i32 {
        self.to_bez().winding(pt)
    }
    fn bounding_box(&self) -> Rect {
        self.to_bez().bounding_box()
    }
}

/// Geometry drawn with a transform, a clip and some layers frozen.
#[derive(Clone, Debug)]
pub struct Pass {
    pub geometry: Arc<Geometry>,
    /// Item coordinates → sheet coordinates (the origin offset included).
    pub transform: Affine,
    /// Sheet coordinates; items outside are not drawn.
    pub clip: Option<BezPath>,
    /// The clip's bounds (for culling).
    pub clip_rect: Option<Rect>,
    /// Layers frozen in this viewport.
    pub frozen: Vec<u32>,
}

/// One sheet ready to draw.
#[derive(Clone, Debug)]
pub struct PreparedSheet {
    pub name: String,
    pub generated: Option<crate::scene::Generated>,
    /// [`crate::Sheet::group`].
    pub group: Option<String>,
    pub kind: SheetKind,
    pub passes: Vec<Pass>,
    /// What fitting shows: the paper and what is drawn (layouts), what is drawn (model).
    pub fit: Option<Rect>,
    pub paper: Option<Rect>,
}

/// A whole scene ready to draw.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub scene: Arc<VectorScene>,
    pub sheets: Vec<Arc<PreparedSheet>>,
    /// Fonts the drawing names that are not what draws its text (SHX or TrueType not found).
    pub missing_fonts: Vec<crate::fonts::MissingFont>,
}

/// Limits on preparation.
#[derive(Clone, Copy, Debug)]
pub struct PrepLimits {
    /// Dashes per item; denser linetypes draw continuous (they look solid anyway).
    pub dashes_per_item: usize,
    /// Glyphs in the whole sheet; text beyond is not laid out.
    pub glyphs: usize,
}

impl Default for PrepLimits {
    fn default() -> Self {
        PrepLimits {
            dashes_per_item: 20_000,
            glyphs: 4_000_000,
        }
    }
}

/// Prepares every sheet.
pub fn prepare(scene: Arc<VectorScene>, fonts: &Fonts, limits: PrepLimits) -> Prepared {
    let mut sheets: Vec<Arc<PreparedSheet>> = Vec::with_capacity(scene.sheets.len());
    let mut model: Option<Arc<Geometry>> = None;
    let cells = build_cells(&scene, fonts, limits);
    for sheet in &scene.sheets {
        let geo = Arc::new(build(&scene, sheet, fonts, limits, &cells));
        if sheet.kind == SheetKind::Model && model.is_none() {
            model = Some(geo.clone());
        }
        sheets.push(Arc::new(sheet_passes(sheet, geo, model.as_ref())));
    }
    let missing_fonts = crate::fonts::missing_fonts(fonts, &used_fonts(&scene));
    Prepared {
        scene,
        sheets,
        missing_fonts,
    }
}

/// The text styles some text uses (unused styles do not make a font missing).
fn used_fonts(scene: &VectorScene) -> Vec<FontSpec> {
    let mut used = vec![false; scene.fonts.len()];
    let cells = scene
        .layout
        .iter()
        .flat_map(|l| l.cells.iter().map(|c| &c.items));
    for items in scene.sheets.iter().map(|s| &s.items).chain(cells) {
        for it in items {
            if let Shape::Text(t) = &it.shape {
                for p in &t.paragraphs {
                    for sp in &p.spans {
                        if let Some(u) = used.get_mut(sp.font as usize) {
                            *u = true;
                        }
                    }
                }
            }
        }
    }
    scene
        .fonts
        .iter()
        .zip(used)
        .filter(|(_, u)| *u)
        .map(|(f, _)| f.clone())
        .collect()
}

fn sheet_passes(sheet: &Sheet, own: Arc<Geometry>, model: Option<&Arc<Geometry>>) -> PreparedSheet {
    let mut passes = Vec::new();
    if sheet.kind == SheetKind::Layout
        && let Some(model) = model
    {
        for vp in &sheet.viewports {
            let clip = vp
                .clip
                .clone()
                .unwrap_or_else(|| kurbo::Shape::to_path(&vp.paper, 0.1));
            let clip_rect = kurbo::Shape::bounding_box(&clip).intersect(vp.paper);
            if clip_rect.width() <= 0.0 || clip_rect.height() <= 0.0 {
                continue;
            }
            passes.push(Pass {
                geometry: model.clone(),
                transform: vp.transform * Affine::translate(model.origin.to_vec2()),
                clip: Some(clip),
                clip_rect: Some(clip_rect),
                frozen: vp.frozen_layers.clone(),
            });
        }
    }
    let bounds = own.bounds;
    passes.push(Pass {
        transform: Affine::translate(own.origin.to_vec2()),
        geometry: own,
        clip: None,
        clip_rect: None,
        frozen: Vec::new(),
    });
    let fit = match (sheet.kind, sheet.paper) {
        (SheetKind::Layout, Some(p)) => Some(bounds.map_or(p, |b| b.union(p))),
        _ => bounds.or_else(|| sheet.fit_rect()),
    };
    PreparedSheet {
        name: sheet.name.clone(),
        generated: sheet.generated,
        group: sheet.group.clone(),
        kind: sheet.kind,
        passes,
        fit,
        paper: sheet.paper,
    }
}

struct Builder<'a> {
    g: Geometry,
    glyph_ids: HashMap<*const Glyph, u32>,
    scene: &'a VectorScene,
    fonts: &'a Fonts,
    limits: PrepLimits,
    glyph_count: usize,
    bounds: Option<Rect>,
    dash_buf: Vec<f64>,
}

fn union(a: Option<Rect>, b: Rect) -> Option<Rect> {
    if !(b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite()) {
        return a;
    }
    Some(match a {
        Some(a) => a.union(b),
        None => b,
    })
}

fn rect_f32(r: Rect) -> [f32; 4] {
    [r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32]
}

/// The colour and layer covering the most area of a geometry's own items.
fn tint(g: &Geometry) -> Option<(Color, u32)> {
    let mut areas: HashMap<u32, (f64, Color)> = HashMap::new();
    for it in &g.items {
        let a = it.rect().area().max(1e-30);
        areas.entry(it.layer).or_insert((0.0, it.color)).0 += a;
    }
    let mut best: Option<(f64, Color, u32)> = None;
    for (layer, (a, c)) in areas {
        if best.is_none_or(|b| a > b.0 || (a == b.0 && layer < b.2)) {
            best = Some((a, c, layer));
        }
    }
    best.map(|(_, c, l)| (c, l))
}

fn finite_rect(r: Rect) -> bool {
    r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()
}

impl Builder<'_> {
    /// Places a layout cell.
    fn instance(&mut self, r: &CellRef, cells: &[Option<Arc<Geometry>>]) {
        let Some(Some(child)) = cells.get(r.cell as usize) else {
            return;
        };
        let Some(cb) = child.bounds else {
            return;
        };
        let o = self.g.origin.to_vec2();
        let co = child.origin.to_vec2();
        let transform = Affine::translate(-o) * r.transform * Affine::translate(co);
        if !transform.as_coeffs().iter().all(|v| v.is_finite()) {
            return;
        }
        let elem = transform.transform_rect_bbox(cb - co);
        let (array, bbox) = match r.array {
            None => (None, elem),
            Some(a) if a.cols == 0 || a.rows == 0 => return,
            Some(a) => {
                let last_c = a.col * f64::from(a.cols - 1);
                let last_r = a.row * f64::from(a.rows - 1);
                let bb = elem
                    .union(elem + last_c)
                    .union(elem + last_r)
                    .union(elem + last_c + last_r);
                (Some((a.cols, a.rows, a.col, a.row)), bb)
            }
        };
        if !finite_rect(bbox) {
            return;
        }
        self.bounds = union(self.bounds, bbox + o);
        self.g.instances.push(Instance {
            geometry: child.clone(),
            transform,
            array,
            bbox,
        });
    }

    /// Adds a path (sheet coordinates); `None` when it has no drawable element.
    fn push_path(
        &mut self,
        kind: Kind,
        it: &Item,
        color: Color,
        weight: Lineweight,
        els: impl Iterator<Item = PathEl>,
    ) {
        let o = self.g.origin.to_vec2();
        let start = self.g.verbs.len();
        let pts = self.g.pts.len();
        let mut bb: Option<Rect> = None;
        let mut drawn = false;
        let push = |g: &mut Geometry, p: Point, bb: &mut Option<Rect>| {
            let q = p - o;
            g.pts.push([q.x as f32, q.y as f32]);
            if q.x.is_finite() && q.y.is_finite() {
                *bb = Some(match *bb {
                    Some(r) => r.union_pt(q),
                    None => Rect::from_points(q, q),
                });
            }
        };
        for el in els {
            match el {
                PathEl::MoveTo(p) => {
                    self.g.verbs.push(MOVE);
                    push(&mut self.g, p.to_vec2().to_point(), &mut bb);
                }
                PathEl::LineTo(p) => {
                    self.g.verbs.push(LINE);
                    push(&mut self.g, p, &mut bb);
                    drawn = true;
                }
                PathEl::QuadTo(a, p) => {
                    self.g.verbs.push(QUAD);
                    push(&mut self.g, a, &mut bb);
                    push(&mut self.g, p, &mut bb);
                    drawn = true;
                }
                PathEl::CurveTo(a, b, p) => {
                    self.g.verbs.push(CUBIC);
                    push(&mut self.g, a, &mut bb);
                    push(&mut self.g, b, &mut bb);
                    push(&mut self.g, p, &mut bb);
                    drawn = true;
                }
                PathEl::ClosePath => self.g.verbs.push(CLOSE),
            }
        }
        let Some(bb) = bb.filter(|_| drawn) else {
            self.g.verbs.truncate(start);
            self.g.pts.truncate(pts);
            return;
        };
        self.bounds = union(self.bounds, bb + o);
        self.g.items.push(DrawItem {
            kind,
            color,
            weight,
            layer: it.layer,
            outer: it.outer,
            start: start as u32,
            end: self.g.verbs.len() as u32,
            pts: pts as u32,
            bbox: rect_f32(bb),
        });
    }

    fn push_dot(&mut self, it: &Item, p: Point) {
        if !(p.x.is_finite() && p.y.is_finite()) {
            return;
        }
        let q = p - self.g.origin.to_vec2();
        let pts = self.g.pts.len();
        self.g.pts.push([q.x as f32, q.y as f32]);
        self.bounds = union(self.bounds, Rect::from_points(p, p));
        self.g.items.push(DrawItem {
            kind: Kind::Dot,
            color: it.color,
            // Dots carry their radius in hundredths of a CSS pixel.
            weight: (self.scene.dot_radius * 100.0).clamp(0.0, 3000.0) as i16,
            layer: it.layer,
            outer: it.outer,
            start: 0,
            end: 0,
            pts: pts as u32,
            bbox: rect_f32(Rect::from_points(q, q)),
        });
    }

    fn push_glyph(&mut self, it: &Item, color: Color, glyph: &Arc<Glyph>, m: Affine) {
        if glyph.path.elements().is_empty() {
            return;
        }
        let bb = m.transform_rect_bbox(kurbo::Shape::bounding_box(&glyph.path));
        if !(bb.x0.is_finite() && bb.y0.is_finite() && bb.x1.is_finite() && bb.y1.is_finite()) {
            return;
        }
        let key = Arc::as_ptr(glyph);
        let next = self.g.glyphs.len() as u32;
        let gi = *self.glyph_ids.entry(key).or_insert(next);
        if gi == next {
            self.g.glyphs.push(glyph.clone());
        }
        let local = Affine::translate(-self.g.origin.to_vec2()) * m;
        let c = local.as_coeffs();
        let pi = self.g.placements.len() as u32;
        self.g.placements.push((
            gi,
            [
                c[0] as f32,
                c[1] as f32,
                c[2] as f32,
                c[3] as f32,
                c[4] as f32,
                c[5] as f32,
            ],
        ));
        self.bounds = union(self.bounds, bb);
        let lb = bb - self.g.origin.to_vec2();
        self.g.items.push(DrawItem {
            kind: Kind::Glyph,
            color,
            weight: 0,
            layer: it.layer,
            outer: it.outer,
            start: pi,
            end: pi + 1,
            pts: 0,
            bbox: rect_f32(lb),
        });
    }

    fn item(&mut self, it: &Item) {
        match &it.shape {
            Shape::Stroke { path, weight, dash } => {
                let pattern = dash.and_then(|d| self.dash_pattern(d.linetype, d.scale));
                match pattern {
                    Some(offset) if self.dash_count_ok(path) => {
                        let dashes = std::mem::take(&mut self.dash_buf);
                        let els: Vec<PathEl> =
                            kurbo::dash(path.elements().iter().copied(), offset, &dashes).collect();
                        self.dash_buf = dashes;
                        self.push_path(Kind::Stroke, it, it.color, *weight, els.into_iter());
                    }
                    _ => self.push_path(
                        Kind::Stroke,
                        it,
                        it.color,
                        *weight,
                        path.elements().iter().copied(),
                    ),
                }
            }
            Shape::Fill { path, even_odd } => {
                let k = if *even_odd {
                    Kind::FillEvenOdd
                } else {
                    Kind::Fill
                };
                self.push_path(k, it, it.color, 0, path.elements().iter().copied());
            }
            Shape::Text(t) => self.text(it, t),
            Shape::Point(p) => self.point(it, *p),
        }
    }

    /// Fills `dash_buf` with the linetype's on/off lengths (scaled, starting with a dash)
    /// and returns the offset; `None` when it draws continuous.
    fn dash_pattern(&mut self, linetype: u32, scale: f64) -> Option<f64> {
        let lt = self.scene.linetypes.get(linetype as usize)?;
        if lt.continuous() || !(scale.is_finite() && scale > 0.0) {
            return None;
        }
        let total = lt.length() * scale;
        if !(total.is_finite() && total > 0.0) {
            return None;
        }
        let dot = total * 1e-3;
        // Signed lengths (positive on), merged by sign.
        let mut runs: Vec<f64> = Vec::with_capacity(lt.pattern.len());
        for &d in &lt.pattern {
            let v = if d == 0.0 { dot } else { d * scale };
            match runs.last_mut() {
                Some(last) if (*last > 0.0) == (v > 0.0) => *last += v,
                _ => runs.push(v),
            }
        }
        let k = runs.iter().position(|&v| v > 0.0)?;
        let prefix: f64 = runs[..k].iter().map(|v| v.abs()).sum();
        runs.rotate_left(k);
        let mut offset = (total - prefix).rem_euclid(total);
        // Even length: a trailing dash joins the leading one.
        if runs.len() > 1 && runs.last().is_some_and(|&v| v > 0.0) {
            let last = runs.pop().unwrap_or(0.0);
            runs[0] += last;
            offset = (offset + last).rem_euclid(total);
        }
        if runs.len() < 2 {
            return None;
        }
        self.dash_buf.clear();
        self.dash_buf
            .extend(runs.iter().map(|v| v.abs().max(1e-12)));
        Some(offset)
    }

    fn dash_count_ok(&self, path: &BezPath) -> bool {
        let period: f64 = self.dash_buf.iter().sum();
        if period <= 0.0 {
            return false;
        }
        let len = kurbo::Shape::perimeter(path, 1e-3);
        len / period < self.limits.dashes_per_item as f64
    }

    fn point(&mut self, it: &Item, p: Point) {
        let mode = self.scene.point_mode;
        let base = mode & 7;
        if mode == 0 || mode & !(7 | 32 | 64) != 0 {
            self.push_dot(it, p);
            return;
        }
        let size = if self.scene.point_size > 0.0 {
            self.scene.point_size
        } else {
            // Relative sizes are a share of the screen in AutoCAD; a share of the
            // drawing's extents is the closest fixed size.
            let pct = if self.scene.point_size < 0.0 {
                -self.scene.point_size / 100.0
            } else {
                0.05
            };
            let h = self
                .scene
                .sheets
                .first()
                .and_then(Sheet::fit_rect)
                .map_or(1.0, |r| r.height().max(r.width() * 0.5));
            h * pct
        };
        let r = size / 2.0;
        let mut path = BezPath::new();
        match base {
            0 => {
                self.push_dot(it, p);
            }
            2 => {
                path.move_to(p - Vec2::new(r, 0.0));
                path.line_to(p + Vec2::new(r, 0.0));
                path.move_to(p - Vec2::new(0.0, r));
                path.line_to(p + Vec2::new(0.0, r));
            }
            3 => {
                path.move_to(p + Vec2::new(-r, -r));
                path.line_to(p + Vec2::new(r, r));
                path.move_to(p + Vec2::new(-r, r));
                path.line_to(p + Vec2::new(r, -r));
            }
            4 => {
                path.move_to(p);
                path.line_to(p + Vec2::new(0.0, r));
            }
            _ => {}
        }
        if mode & 32 != 0 {
            path.extend(kurbo::Shape::path_elements(&kurbo::Circle::new(p, r), 0.1));
        }
        if mode & 64 != 0 {
            path.extend(kurbo::Shape::path_elements(
                &Rect::new(p.x - r, p.y - r, p.x + r, p.y + r),
                0.1,
            ));
        }
        if !path.elements().is_empty() {
            self.push_path(
                Kind::Stroke,
                it,
                it.color,
                0,
                path.elements().iter().copied(),
            );
        }
    }

    fn text(&mut self, it: &Item, t: &TextBlock) {
        if self.glyph_count >= self.limits.glyphs {
            return;
        }
        let laid = layout(t, &self.scene.fonts, self.fonts);
        // SHX shapes are strokes: consecutive ones of a colour become one stroked path.
        let mut strokes: Option<(Color, BezPath)> = None;
        for g in &laid.glyphs {
            if self.glyph_count >= self.limits.glyphs {
                break;
            }
            self.glyph_count += 1;
            let color = g.color.unwrap_or(it.color);
            let m = t.transform * g.place;
            if g.glyph.stroke {
                if strokes.as_ref().is_some_and(|(c, _)| *c != color)
                    && let Some((c, p)) = strokes.take()
                {
                    self.push_path(Kind::Stroke, it, c, 0, p.elements().iter().copied());
                }
                let (_, p) = strokes.get_or_insert_with(|| (color, BezPath::new()));
                p.extend((m * g.glyph.path.clone()).elements().iter().copied());
            } else {
                self.push_glyph(it, color, &g.glyph, m);
            }
        }
        if let Some((c, p)) = strokes {
            self.push_path(Kind::Stroke, it, c, 0, p.elements().iter().copied());
        }
        for (line, color) in &laid.lines {
            let color = color.unwrap_or(it.color);
            let path = t.transform * line.clone();
            self.push_path(Kind::Stroke, it, color, 0, path.elements().iter().copied());
        }
    }
}

fn build(
    scene: &VectorScene,
    sheet: &Sheet,
    fonts: &Fonts,
    limits: PrepLimits,
    cells: &[Option<Arc<Geometry>>],
) -> Geometry {
    let origin = sheet
        .extents
        .or(sheet.file_extents)
        .map_or(Point::ORIGIN, |r| r.center());
    let mut g = build_items(
        scene,
        &sheet.items,
        &sheet.refs,
        origin,
        fonts,
        limits,
        cells,
    );
    if let Some(o) = &sheet.layer_opacity {
        g.layer_opacity.clone_from(o);
    }
    g
}

/// Prepares the layout's cells, each once, children before parents; a reference that
/// closes a cycle is not drawn.
fn build_cells(
    scene: &VectorScene,
    fonts: &Fonts,
    limits: PrepLimits,
) -> Vec<Option<Arc<Geometry>>> {
    let Some(layout) = &scene.layout else {
        return Vec::new();
    };
    let n = layout.cells.len();
    let mut out: Vec<Option<Arc<Geometry>>> = vec![None; n];
    // 0 unvisited, 1 on the stack, 2 done.
    let mut state = vec![0u8; n];
    for root in 0..n {
        if state[root] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        state[root] = 1;
        while let Some(top) = stack.last_mut() {
            let (c, k) = *top;
            if let Some(r) = layout.cells[c].refs.get(k) {
                top.1 += 1;
                let child = r.cell as usize;
                if child < n && state[child] == 0 {
                    state[child] = 1;
                    stack.push((child, 0));
                }
                continue;
            }
            stack.pop();
            state[c] = 2;
            let cell = &layout.cells[c];
            let own = bounds_of_items(&cell.items);
            let origin = own.map_or(Point::ORIGIN, |r| r.center());
            // Children still on the stack (cycles) are missing from `out` and skipped.
            let g = build_items(scene, &cell.items, &cell.refs, origin, fonts, limits, &out);
            out[c] = Some(Arc::new(g));
        }
    }
    out
}

/// Rough bounds of items (control points; text by its insertion point).
fn bounds_of_items(items: &[Item]) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    for it in items {
        let b = match &it.shape {
            Shape::Stroke { path, .. } | Shape::Fill { path, .. } => {
                kurbo::Shape::bounding_box(path)
            }
            Shape::Text(t) => {
                let p = t.transform * Point::ORIGIN;
                Rect::from_points(p, p)
            }
            Shape::Point(p) => Rect::from_points(*p, *p),
        };
        r = union(r, b);
    }
    r
}

fn build_items(
    scene: &VectorScene,
    items: &[Item],
    refs: &[CellRef],
    origin: Point,
    fonts: &Fonts,
    limits: PrepLimits,
    cells: &[Option<Arc<Geometry>>],
) -> Geometry {
    let origin = if origin.x.is_finite() && origin.y.is_finite() {
        origin
    } else {
        Point::ORIGIN
    };
    let (fill_alpha, label_min_px) = scene
        .layout
        .as_ref()
        .map_or((0, 0.0), |l| (l.fill_alpha, l.label_min_px));
    let mut b = Builder {
        g: Geometry {
            origin,
            fill_alpha,
            label_min_px,
            layer_opacity: scene.layer_opacity.clone(),
            ..Default::default()
        },
        glyph_ids: HashMap::new(),
        scene,
        fonts,
        limits,
        glyph_count: 0,
        bounds: None,
        dash_buf: Vec::new(),
    };
    for it in items {
        b.item(it);
    }
    b.g.tint = tint(&b.g);
    for r in refs {
        b.instance(r, cells);
    }
    if b.g.tint.is_none() {
        b.g.tint = b.g.instances.iter().find_map(|i| i.geometry.tint);
    }
    b.g.instances.shrink_to_fit();
    b.g.bounds = b.bounds;
    b.g.verbs.shrink_to_fit();
    b.g.pts.shrink_to_fit();
    b.g.items.shrink_to_fit();
    b.g
}

/// A placed glyph of laid-out text, in text space.
pub struct PlacedGlyph {
    pub glyph: Arc<Glyph>,
    /// Font units → text space.
    pub place: Affine,
    pub color: Option<Color>,
}

/// Text laid out in text space (insertion point at the origin).
#[derive(Default)]
pub struct Laid {
    pub glyphs: Vec<PlacedGlyph>,
    /// Underlines, overlines and strike-throughs.
    pub lines: Vec<(BezPath, Option<Color>)>,
}

struct Run {
    scale: f64,
    width: f64,
    skew: f64,
    color: Option<Color>,
    height: f64,
    deco: [bool; 3],
}

struct LineGlyph {
    glyph: Arc<Glyph>,
    x: f64,
    run: usize,
}

#[derive(Default)]
struct Line {
    glyphs: Vec<LineGlyph>,
    width: f64,
    cap: f64,
    align: Option<HAlign>,
    /// Decoration spans: run, x0, x1.
    decos: Vec<(usize, f64, f64)>,
}

/// Lays out a text block with the fonts its styles resolve to.
pub fn layout(t: &TextBlock, styles: &[FontSpec], fonts: &Fonts) -> Laid {
    let default_spec = FontSpec::default();
    let mut runs: Vec<Run> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    let wrap = if t.wrap > 0.0 && t.wrap.is_finite() {
        t.wrap
    } else {
        f64::INFINITY
    };
    for para in &t.paragraphs {
        let mut line = Line {
            align: para.align,
            ..Default::default()
        };
        // Position after the last break opportunity: (glyph index, x at that point).
        let mut brk: Option<(usize, f64)> = None;
        for sp in &para.spans {
            if !(sp.height.is_finite() && sp.height > 0.0) {
                continue;
            }
            let spec = styles.get(sp.font as usize).unwrap_or(&default_spec);
            let r = fonts.resolve(spec, sp.family.as_deref(), sp.bold, sp.italic);
            let face = r.face;
            let scale = sp.height / f64::from(face.cap_height);
            let wf = if sp.width_factor.is_finite() && sp.width_factor > 0.0 {
                sp.width_factor
            } else {
                1.0
            };
            let mut oblique = if sp.oblique.is_finite() {
                sp.oblique
            } else {
                0.0
            };
            if (sp.italic || spec.italic) && face.fake_italic && oblique == 0.0 {
                oblique = 12.0;
            }
            let run = runs.len();
            runs.push(Run {
                scale,
                width: wf,
                skew: oblique.clamp(-85.0, 85.0).to_radians().tan(),
                color: sp.color,
                height: sp.height,
                deco: [sp.underline, sp.overline, sp.strike],
            });
            let deco_start = line.width;
            for c in sp.text.chars() {
                if c == '\n' {
                    finish_deco(&mut line, run, deco_start, &runs);
                    lines.push(std::mem::take(&mut line));
                    line.align = para.align;
                    brk = None;
                    continue;
                }
                if c == ' ' || c == '\u{3000}' {
                    brk = Some((line.glyphs.len(), line.width));
                }
                let (gface, gscale) = match fonts.fallback_for(&face, c) {
                    Some(f) => {
                        let s = scale * f64::from(face.units_per_em) / f64::from(f.units_per_em);
                        (f, s)
                    }
                    None => (face.clone(), scale),
                };
                let glyph = gface.glyph(c).or_else(|| face.glyph('?'));
                let Some(glyph) = glyph else { continue };
                let adv = f64::from(glyph.advance) * gscale * wf;
                if line.width + adv > wrap && c != ' ' {
                    if let Some((gi, _)) = brk.filter(|(gi, _)| *gi > 0) {
                        // Move the word after the break to a new line.
                        let tail: Vec<LineGlyph> = line.glyphs.drain(gi..).collect();
                        let shift = tail
                            .iter()
                            .position(|g| !is_space_glyph(g, &runs))
                            .unwrap_or(tail.len());
                        let tail: Vec<LineGlyph> = tail.into_iter().skip(shift).collect();
                        let x0 = tail.first().map_or(0.0, |g| g.x);
                        line.width = brk.map_or(line.width, |b| b.1);
                        let cap = line.cap;
                        lines.push(std::mem::take(&mut line));
                        line.align = para.align;
                        line.cap = cap;
                        for mut g in tail {
                            g.x -= x0;
                            line.width = g.x + glyph_adv(&g, &runs);
                            line.glyphs.push(g);
                        }
                    }
                    brk = None;
                }
                let run_ref = &runs[run];
                if !std::ptr::eq(&*gface, &*face) {
                    // A fallback face: record a run of its own scale.
                    runs.push(Run {
                        scale: gscale,
                        width: wf,
                        skew: run_ref.skew,
                        color: run_ref.color,
                        height: run_ref.height,
                        deco: [false; 3],
                    });
                    let fr = runs.len() - 1;
                    line.glyphs.push(LineGlyph {
                        glyph,
                        x: line.width,
                        run: fr,
                    });
                } else {
                    line.glyphs.push(LineGlyph {
                        glyph,
                        x: line.width,
                        run,
                    });
                }
                line.width += adv;
                line.cap = line.cap.max(sp.height);
            }
            finish_deco(&mut line, run, deco_start, &runs);
        }
        lines.push(line);
    }
    // Trailing spaces do not count towards alignment.
    for l in &mut lines {
        while let Some(g) = l.glyphs.last() {
            if is_space_glyph(g, &runs) {
                l.width = g.x;
                l.glyphs.pop();
            } else {
                break;
            }
        }
        if l.cap <= 0.0 {
            l.cap = runs.first().map_or(1.0, |r| r.height);
        }
    }
    let n = lines.len();
    if n == 0 {
        return Laid::default();
    }
    let spacing_f = if t.line_spacing.is_finite() && t.line_spacing > 0.0 {
        t.line_spacing
    } else {
        1.0
    };
    // Baselines going down from the first.
    let mut base = Vec::with_capacity(n);
    let mut y = 0.0;
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            y -= l.cap.max(lines[i - 1].cap) * 5.0 / 3.0 * spacing_f;
        }
        base.push(y);
    }
    let first_cap = lines[0].cap;
    let last_y = base[n - 1];
    let descent = lines[n - 1].cap * 0.3;
    let dy = match t.valign {
        VAlign::Baseline => 0.0,
        VAlign::Top => -first_cap,
        VAlign::Middle => -(first_cap + last_y) / 2.0,
        VAlign::Bottom => -last_y + descent,
    };
    let max_w = lines.iter().map(|l| l.width).fold(0.0, f64::max);
    let box_w = if wrap.is_finite() && t.paragraphs.len() + n > 2 {
        wrap.max(max_w)
    } else {
        max_w
    };
    let fit = match (t.halign, t.fit) {
        (HAlign::Fit, Some((len, aligned))) if len > 0.0 && max_w > 0.0 => {
            let sx = len / max_w;
            Some((sx, if aligned { sx } else { 1.0 }))
        }
        _ => None,
    };
    let box_x = match t.halign {
        HAlign::Left | HAlign::Fit => 0.0,
        HAlign::Center => -box_w / 2.0,
        HAlign::Right => -box_w,
    };
    let mut out = Laid::default();
    for (l, by) in lines.iter().zip(&base) {
        let align = l.align.unwrap_or(t.halign);
        let lx = box_x
            + match align {
                HAlign::Left | HAlign::Fit => 0.0,
                HAlign::Center => (box_w - l.width) / 2.0,
                HAlign::Right => box_w - l.width,
            };
        let (sx, sy) = fit.unwrap_or((1.0, 1.0));
        let line_xf = Affine::scale_non_uniform(sx, sy) * Affine::translate((lx, by + dy));
        for g in &l.glyphs {
            let r = &runs[g.run];
            let m = line_xf
                * Affine::translate((g.x, 0.0))
                * Affine::new([1.0, 0.0, r.skew, 1.0, 0.0, 0.0])
                * Affine::scale_non_uniform(r.scale * r.width, r.scale);
            out.glyphs.push(PlacedGlyph {
                glyph: g.glyph.clone(),
                place: m,
                color: r.color,
            });
        }
        for &(run, x0, x1) in &l.decos {
            let r = &runs[run];
            for (on, yy) in r
                .deco
                .iter()
                .zip([-0.2 * r.height, 1.2 * r.height, 0.5 * r.height])
            {
                if *on {
                    let mut p = BezPath::new();
                    p.move_to((x0, yy));
                    p.line_to((x1, yy));
                    out.lines.push((line_xf * p, r.color));
                }
            }
        }
    }
    out
}

fn glyph_adv(g: &LineGlyph, runs: &[Run]) -> f64 {
    let r = &runs[g.run];
    f64::from(g.glyph.advance) * r.scale * r.width
}

fn is_space_glyph(g: &LineGlyph, _runs: &[Run]) -> bool {
    g.glyph.path.elements().is_empty()
}

fn finish_deco(line: &mut Line, run: usize, x0: f64, runs: &[Run]) {
    if runs[run].deco.iter().any(|d| *d) && line.width > x0 {
        line.decos.push((run, x0, line.width));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::*;

    fn scene_with(items: Vec<Item>) -> Arc<VectorScene> {
        let mut s = VectorScene {
            layer_opacity: Vec::new(),
            pcb: None,
            layers: vec![Layer {
                name: "0".into(),
                generated: None,
                color: Color::Foreground,
                linetype: None,
                lineweight: LW_DEFAULT,
                on: true,
                frozen: false,
                plot: true,
                items: 0,
            }],
            linetypes: vec![Linetype {
                name: "DASHED".into(),
                description: String::new(),
                pattern: vec![-0.25, 0.5, -0.25, 0.0],
            }],
            fonts: vec![FontSpec::default()],
            ..Default::default()
        };
        let mut sheet = Sheet::new("Model", SheetKind::Model);
        sheet.items = items;
        s.sheets.push(sheet);
        Arc::new(s)
    }

    fn line(a: (f64, f64), b: (f64, f64)) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(a);
        p.line_to(b);
        p
    }

    #[test]
    fn dashes_start_with_the_pattern() {
        let item = Item {
            layer: 0,
            outer: 0,
            color: Color::Foreground,
            shape: Shape::Stroke {
                path: line((0.0, 0.0), (10.0, 0.0)),
                weight: 25,
                dash: Some(Dash {
                    linetype: 0,
                    scale: 1.0,
                }),
            },
        };
        let p = prepare(
            scene_with(vec![item]),
            &Fonts::bundled_only(),
            PrepLimits::default(),
        );
        let g = &p.sheets[0].passes[0].geometry;
        assert_eq!(g.items.len(), 1);
        let path = g.path(&g.items[0]).to_bez();
        let o = g.origin.to_vec2();
        let starts: Vec<f64> = path
            .elements()
            .iter()
            .filter_map(|e| match e {
                PathEl::MoveTo(p) => Some((p.x + o.x * 0.0 + g.origin.x).round_ties_even()),
                _ => None,
            })
            .collect();
        // Gap 0.25, dash 0.5, gap 0.25, dot: a dash starts at 0.25 of each unit.
        assert!(starts.len() >= 10, "{starts:?}");
        let first = path.elements()[0];
        let PathEl::MoveTo(p0) = first else { panic!() };
        assert!(
            (p0.x + g.origin.x - 0.0).abs() < 1e-3 || (p0.x + g.origin.x - 0.25).abs() < 1e-3,
            "{p0:?}"
        );
    }

    #[test]
    fn text_aligns_and_scales_to_cap_height() {
        let mut tb = TextBlock {
            transform: Affine::IDENTITY,
            halign: HAlign::Center,
            valign: VAlign::Baseline,
            wrap: 0.0,
            fit: None,
            line_spacing: 1.0,
            paragraphs: vec![Paragraph {
                spans: vec![Span::new("HH", 0, 10.0)],
                align: None,
            }],
        };
        let fonts = Fonts::bundled_only();
        let laid = layout(&tb, &[FontSpec::default()], &fonts);
        assert_eq!(laid.glyphs.len(), 2);
        let mut bb: Option<Rect> = None;
        for g in &laid.glyphs {
            bb = union(
                bb,
                g.place
                    .transform_rect_bbox(kurbo::Shape::bounding_box(&g.glyph.path)),
            );
        }
        let bb = bb.unwrap();
        assert!((bb.y1 - 10.0).abs() < 0.2, "{bb:?}");
        assert!(bb.y0.abs() < 0.2, "{bb:?}");
        assert!((bb.x0 + bb.x1).abs() < 2.0, "centred: {bb:?}");
        tb.valign = VAlign::Top;
        let laid = layout(&tb, &[FontSpec::default()], &fonts);
        let top = laid
            .glyphs
            .iter()
            .map(|g| {
                g.place
                    .transform_rect_bbox(kurbo::Shape::bounding_box(&g.glyph.path))
                    .y1
            })
            .fold(f64::MIN, f64::max);
        assert!(top.abs() < 0.2, "{top}");
    }

    #[test]
    fn wraps_at_spaces() {
        let tb = TextBlock {
            transform: Affine::IDENTITY,
            halign: HAlign::Left,
            valign: VAlign::Top,
            wrap: 60.0,
            fit: None,
            line_spacing: 1.0,
            paragraphs: vec![Paragraph {
                spans: vec![Span::new("one two three four five", 0, 10.0)],
                align: None,
            }],
        };
        let laid = layout(&tb, &[FontSpec::default()], &Fonts::bundled_only());
        let ys: std::collections::BTreeSet<i64> = laid
            .glyphs
            .iter()
            .map(|g| (g.place.translation().y * 10.0) as i64)
            .collect();
        assert!(ys.len() >= 3, "{ys:?}");
        for g in &laid.glyphs {
            assert!(g.place.translation().x < 60.0);
        }
    }
}
