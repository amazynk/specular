//! Walking a prepared sheet for one view: culling, layer visibility, colours on the
//! background, and stroke widths in pixels. Backends ([`crate::raster`], the GPU renderer)
//! implement [`Sink`].

use kurbo::{Affine, BezPath, Point, Rect, Vec2};

use crate::prep::{CLOSE, Geometry, Instance, Kind, LINE, MOVE, PathRef, PreparedSheet};
use crate::scene::{Color, Lineweight};

/// What a backend draws. Transforms map the given coordinates to pixels.
pub trait Sink {
    /// Strokes `path` `width` units wide (in the path's own coordinates).
    fn stroke(&mut self, path: PathRef<'_>, transform: Affine, width: f64, color: [u8; 4]);
    fn fill(&mut self, path: PathRef<'_>, transform: Affine, even_odd: bool, color: [u8; 4]);
    fn glyph(&mut self, path: &BezPath, transform: Affine, color: [u8; 4]);
    /// A round dot, centre and radius in pixels.
    fn dot(&mut self, center: Point, radius: f64, color: [u8; 4]);
    /// Starts clipping to `clip` (sheet coordinates under `transform`).
    fn push_clip(&mut self, clip: &BezPath, transform: Affine);
    fn pop_clip(&mut self);
    /// Starts a group: what is drawn until [`Sink::pop_group`] is composited at `opacity`.
    fn push_group(&mut self, opacity: f32) {
        let _ = opacity;
    }
    fn pop_group(&mut self) {}
    /// While on, fills and strokes erase what the open group drew (their colour is ignored).
    fn set_erase(&mut self, erase: bool) {
        let _ = erase;
    }
}

/// How to draw a view.
#[derive(Clone, Debug)]
pub struct View<'a> {
    /// Sheet coordinates → pixels (y down).
    pub transform: Affine,
    /// The pixels to cover.
    pub size: (u32, u32),
    /// A dark background (ACI 7 draws white).
    pub dark: bool,
    pub background: [u8; 3],
    /// Draw lineweights (else every line is a hairline).
    pub lineweights: bool,
    /// Physical pixels per logical pixel (hairlines are one logical pixel).
    pub dpi_scale: f64,
    /// Layers shown, by index; layers past the end are shown.
    pub layers: &'a [bool],
}

/// A layer's style chosen in the viewer, over what the scene gives (maps' layer panel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerStyle {
    /// Strokes and dots in this colour.
    pub stroke: Option<[u8; 3]>,
    /// Fills in this colour and alpha.
    pub fill: Option<[u8; 4]>,
    /// Fills in their own colour at this alpha (when `fill` is not set).
    pub fill_alpha: Option<u8>,
    /// Everything of the layer at this opacity (0–1).
    pub opacity: f32,
    /// Lines this many logical pixels wide.
    pub line_px: Option<f64>,
    /// Dots of this radius in logical pixels.
    pub dot_px: Option<f64>,
}

impl Default for LayerStyle {
    fn default() -> Self {
        LayerStyle {
            stroke: None,
            fill: None,
            fill_alpha: None,
            opacity: 1.0,
            line_px: None,
            dot_px: None,
        }
    }
}

impl LayerStyle {
    fn alpha(&self, a: u8) -> u8 {
        (f32::from(a) * self.opacity.clamp(0.0, 1.0)).round() as u8
    }
}

/// What a walk drew.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub drawn: usize,
    pub culled: usize,
    pub hidden: usize,
    /// Layout cell placements drawn in detail, and those drawn as a rectangle.
    pub instances: usize,
    pub stand_ins: usize,
    /// The frame budget ran out before everything was drawn.
    pub truncated: bool,
}

/// A lineweight's width in pixels at 96 dpi.
pub fn lineweight_px(w: Lineweight, dpi_scale: f64) -> f64 {
    let mm = f64::from(w.max(0)) / 100.0;
    (mm * 96.0 / 25.4 * dpi_scale).max(dpi_scale)
}

fn rgba(c: Color, view: &View<'_>) -> [u8; 4] {
    let [r, g, b] = c.on(view.dark, view.background);
    [r, g, b, 255]
}

/// Draws the sheet's passes in order.
pub fn draw(sheet: &PreparedSheet, view: &View<'_>, sink: &mut impl Sink) -> Stats {
    draw_styled(sheet, view, &[], sink)
}

/// [`draw`] with `styles` over the scene's own, by layer index (layers past the end keep
/// theirs).
pub fn draw_styled(
    sheet: &PreparedSheet,
    view: &View<'_>,
    styles: &[LayerStyle],
    sink: &mut impl Sink,
) -> Stats {
    let mut stats = Stats::default();
    let screen = Rect::new(0.0, 0.0, f64::from(view.size.0), f64::from(view.size.1));
    let Some(inv) = invertible(view.transform) else {
        return stats;
    };
    let visible_sheet = inv.transform_rect_bbox(screen);
    for pass in &sheet.passes {
        let mut area = visible_sheet;
        if let Some(c) = pass.clip_rect {
            area = area.intersect(c);
            if area.width() <= 0.0 || area.height() <= 0.0 {
                continue;
            }
        }
        let Some(pinv) = invertible(pass.transform) else {
            continue;
        };
        let local = pinv.transform_rect_bbox(area);
        let xf = view.transform * pass.transform;
        if let Some(clip) = &pass.clip {
            sink.push_clip(clip, view.transform);
        }
        walk(
            &pass.geometry,
            xf,
            local,
            &pass.frozen,
            (view, styles),
            sink,
            &mut stats,
        );
        if pass.clip.is_some() {
            sink.pop_clip();
        }
    }
    stats
}

fn invertible(a: Affine) -> Option<Affine> {
    let d = a.determinant();
    (d.is_finite() && d.abs() > 1e-300).then(|| a.inverse())
}

/// Instances nested deeper than this are not drawn (a guard; layouts nest a dozen deep).
const MAX_DEPTH: u32 = 64;
/// Primitives drawn in one frame after which layout cells draw as their outline only.
pub const DETAIL_BUDGET: usize = 400_000;
/// Primitives drawn in one frame after which nothing more is drawn.
pub const FRAME_BUDGET: usize = 1_000_000;
/// A layout cell smaller than this many pixels draws as a tinted rectangle.
const CELL_MIN_PX: f64 = 2.0;

/// The rectangle verbs of a level-of-detail stand-in.
const RECT_VERBS: [u8; 5] = [MOVE, LINE, LINE, LINE, CLOSE];

/// What a walk carries down the instance tree.
struct Walk<'v, 'a, S> {
    frozen: &'v [u32],
    view: &'v View<'a>,
    styles: &'v [LayerStyle],
    sink: &'v mut S,
    stats: &'v mut Stats,
    /// The layer whose group is open, and whether erasing is on.
    group: Option<u32>,
    erasing: bool,
}

fn walk(
    g: &Geometry,
    xf: Affine,
    local: Rect,
    frozen: &[u32],
    (view, styles): (&View<'_>, &[LayerStyle]),
    sink: &mut impl Sink,
    stats: &mut Stats,
) {
    let mut w = Walk {
        frozen,
        view,
        styles,
        sink,
        stats,
        group: None,
        erasing: false,
    };
    w.geometry(g, xf, local, 0);
    w.close_group();
}

impl<S: Sink> Walk<'_, '_, S> {
    /// Ends the open group, if any.
    fn close_group(&mut self) {
        if self.erasing {
            self.sink.set_erase(false);
            self.erasing = false;
        }
        if self.group.take().is_some() {
            self.sink.pop_group();
        }
    }

    /// Before drawing an item of `layer` in `color` with groups on: opens its layer's group
    /// (closing another's) and turns erasing on or off. False when the item draws nothing
    /// (a clear shape at the start of a group has nothing to erase).
    fn enter_group(&mut self, g: &Geometry, layer: u32, color: Color) -> bool {
        let clear = color == Color::Clear;
        if self.group != Some(layer) {
            self.close_group();
            if clear {
                return false;
            }
            let opacity = g.layer_opacity.get(layer as usize).copied().unwrap_or(1.0);
            self.sink.push_group(opacity.clamp(0.0, 1.0));
            self.group = Some(layer);
        }
        if clear != self.erasing {
            self.sink.set_erase(clear);
            self.erasing = clear;
        }
        true
    }

    fn shown(&self, l: u32) -> bool {
        self.view.layers.get(l as usize).copied().unwrap_or(true) && !self.frozen.contains(&l)
    }

    fn over_budget(&mut self) -> bool {
        if self.stats.drawn >= FRAME_BUDGET {
            self.stats.truncated = true;
            return true;
        }
        false
    }

    fn geometry(&mut self, g: &Geometry, xf: Affine, local: Rect, depth: u32) {
        // Pixels per item unit (uniform part of the transform).
        let ppu = xf.determinant().abs().sqrt();
        if !(ppu.is_finite() && ppu > 0.0) {
            return;
        }
        let view = self.view;
        let hair = view.dpi_scale / ppu;
        // Items smaller than this many units in both directions are dots at most.
        let tiny = 0.35 / ppu;
        let label_min = f64::from(g.label_min_px) * view.dpi_scale / ppu;
        for it in &g.items {
            if !self.shown(it.layer) || !self.shown(it.outer) {
                self.stats.hidden += 1;
                continue;
            }
            let b = it.bbox;
            let pad = if it.kind == Kind::Stroke {
                hair * 8.0
            } else {
                0.0
            };
            if f64::from(b[2]) + pad < local.x0
                || f64::from(b[0]) - pad > local.x1
                || f64::from(b[3]) + pad < local.y0
                || f64::from(b[1]) - pad > local.y1
            {
                self.stats.culled += 1;
                continue;
            }
            if self.over_budget() {
                return;
            }
            if !g.layer_opacity.is_empty() && !self.enter_group(g, it.layer, it.color) {
                continue;
            }
            let (bw, bh) = (f64::from(b[2] - b[0]), f64::from(b[3] - b[1]));
            let small = bw < tiny && bh < tiny;
            let style = self.styles.get(it.layer as usize);
            let mut color = rgba(it.color, view);
            if let Some(s) = style {
                if let (Some([r, g, b]), Kind::Stroke | Kind::Dot | Kind::Glyph) =
                    (s.stroke, it.kind)
                {
                    color = [r, g, b, color[3]];
                }
                color[3] = s.alpha(color[3]);
            }
            self.stats.drawn += 1;
            match it.kind {
                Kind::Stroke => {
                    let px = match style.and_then(|s| s.line_px) {
                        Some(w) => (w * view.dpi_scale).max(0.5),
                        None if view.lineweights => lineweight_px(it.weight, view.dpi_scale),
                        None => view.dpi_scale,
                    };
                    if small {
                        let c = xf * Point::new(f64::from(b[0]), f64::from(b[1]));
                        self.sink.dot(c, px * 0.5, color);
                        continue;
                    }
                    self.sink.stroke(g.path(it), xf, px / ppu, color);
                }
                Kind::Fill | Kind::FillEvenOdd => {
                    if small {
                        continue;
                    }
                    let eo = it.kind == Kind::FillEvenOdd;
                    if let Some(f) = style.and_then(|s| s.fill) {
                        let a = style.map_or(f[3], |s| s.alpha(f[3]));
                        self.sink.fill(g.path(it), xf, eo, [f[0], f[1], f[2], a]);
                    } else if let Some((s, a)) = style.and_then(|s| Some((s, s.fill_alpha?))) {
                        let [r, gr, bl, _] = color;
                        self.sink.fill(g.path(it), xf, eo, [r, gr, bl, s.alpha(a)]);
                    } else if g.fill_alpha == 0 {
                        self.sink.fill(g.path(it), xf, eo, color);
                    } else {
                        let [r, gr, bl, _] = color;
                        self.sink
                            .fill(g.path(it), xf, eo, [r, gr, bl, g.fill_alpha]);
                        self.sink.stroke(g.path(it), xf, hair, color);
                    }
                }
                Kind::Glyph => {
                    if small || bw.max(bh) < label_min {
                        continue;
                    }
                    if let Some((path, m)) = g.glyph(it) {
                        self.sink.glyph(path, xf * m, color);
                    }
                }
                Kind::Dot => {
                    let c = xf * g.dot(it);
                    let r = match style.and_then(|s| s.dot_px) {
                        Some(r) => r,
                        None if it.weight > 0 => f64::from(it.weight) / 100.0,
                        None => 1.0,
                    };
                    self.sink.dot(c, view.dpi_scale.max(1.0) * r, color);
                }
            }
        }
        if !g.layer_opacity.is_empty() {
            self.close_group();
        }
        if depth >= MAX_DEPTH {
            return;
        }
        for inst in &g.instances {
            if self.stats.truncated {
                return;
            }
            self.instance(inst, xf, local, ppu, depth);
        }
    }

    /// Draws a layout cell's placements that meet `local` (the parent's item coordinates).
    fn instance(&mut self, inst: &Instance, xf: Affine, local: Rect, ppu: f64, depth: u32) {
        if !overlaps(inst.bbox, local) {
            self.stats.culled += 1;
            return;
        }
        let Some(elem) = inst.element_bbox() else {
            return;
        };
        let min_px = CELL_MIN_PX * self.view.dpi_scale;
        let elem_tiny = elem.width().max(elem.height()) * ppu < min_px;
        let Some((cols, rows, col, row)) = inst.array else {
            if elem_tiny || self.stats.drawn >= DETAIL_BUDGET {
                self.stand_in(&inst.geometry, elem, xf);
            } else {
                self.place(inst, inst.transform, xf, local, depth);
            }
            return;
        };
        // Elements closer than a couple of pixels and too small to see: one rectangle.
        let pitch = |v: Vec2| v.hypot() * ppu;
        let dense = |v: Vec2, n: u32| n <= 1 || pitch(v) < min_px;
        if elem_tiny && dense(col, cols) && dense(row, rows) {
            self.stand_in(&inst.geometry, inst.bbox, xf);
            return;
        }
        // Offsets at which an element meets the area.
        let want = Rect::new(
            local.x0 - elem.x1,
            local.y0 - elem.y1,
            local.x1 - elem.x0,
            local.y1 - elem.y0,
        );
        let (ci, ri) = lattice_range(want, cols, rows, col, row);
        for j in ri {
            for i in ci.clone() {
                if self.over_budget() {
                    return;
                }
                let off = col * f64::from(i) + row * f64::from(j);
                let e = elem + off;
                if !overlaps(e, local) {
                    continue;
                }
                if elem_tiny || self.stats.drawn >= DETAIL_BUDGET {
                    self.stand_in(&inst.geometry, e, xf);
                } else {
                    let t = Affine::translate(off) * inst.transform;
                    self.place(inst, t, xf, local, depth);
                }
            }
        }
    }

    /// Draws one placement of a cell with `t` (its item coordinates → the parent's).
    fn place(&mut self, inst: &Instance, t: Affine, xf: Affine, local: Rect, depth: u32) {
        let Some(inv) = invertible(t) else {
            return;
        };
        self.stats.instances += 1;
        self.geometry(
            &inst.geometry,
            xf * t,
            inv.transform_rect_bbox(local),
            depth + 1,
        );
    }

    /// A cell too small (or too many) to draw: a rectangle in its main layer's colour.
    fn stand_in(&mut self, g: &Geometry, r: Rect, xf: Affine) {
        let Some((c, layer)) = g.tint else {
            return;
        };
        if !self.shown(layer) {
            self.stats.hidden += 1;
            return;
        }
        if self.over_budget() {
            return;
        }
        let pts = [
            [r.x0 as f32, r.y0 as f32],
            [r.x1 as f32, r.y0 as f32],
            [r.x1 as f32, r.y1 as f32],
            [r.x0 as f32, r.y1 as f32],
        ];
        let path = PathRef {
            verbs: &RECT_VERBS,
            pts: &pts,
        };
        let [cr, cg, cb, _] = rgba(c, self.view);
        let a = if g.fill_alpha == 0 { 255 } else { g.fill_alpha };
        self.stats.drawn += 1;
        self.stats.stand_ins += 1;
        self.sink.fill(path, xf, false, [cr, cg, cb, a]);
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x1 >= b.x0 && a.x0 <= b.x1 && a.y1 >= b.y0 && a.y0 <= b.y1
}

/// The columns and rows of an array whose offset `i·col + j·row` may fall in `want`.
pub fn lattice_range(
    want: Rect,
    cols: u32,
    rows: u32,
    col: Vec2,
    row: Vec2,
) -> (std::ops::Range<u32>, std::ops::Range<u32>) {
    let clamp = |lo: f64, hi: f64, n: u32| -> std::ops::Range<u32> {
        if !(lo.is_finite() && hi.is_finite()) || hi < 0.0 || lo > f64::from(n) - 1.0 {
            return 0..0;
        }
        let a = lo.floor().max(0.0) as u32;
        let b = (hi.ceil().min(f64::from(n) - 1.0) as u32).saturating_add(1);
        a.min(n)..b.min(n)
    };
    // Steps along one vector alone (one row or one column, or parallel vectors).
    let along = |v: Vec2, n: u32| -> (f64, f64) {
        let mut lo = 0.0f64;
        let mut hi = f64::from(n) - 1.0;
        for (c, w0, w1) in [(v.x, want.x0, want.x1), (v.y, want.y0, want.y1)] {
            if c.abs() < 1e-12 {
                if w0 > 0.0 || w1 < 0.0 {
                    return (1.0, 0.0);
                }
                continue;
            }
            let (a, b) = (w0 / c, w1 / c);
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
        }
        (lo, hi)
    };
    let det = col.cross(row);
    if rows <= 1 || cols <= 1 || det.abs() < 1e-12 * col.hypot() * row.hypot() {
        if rows <= 1 {
            let (lo, hi) = along(col, cols);
            return (clamp(lo, hi, cols), 0..rows.min(1));
        }
        if cols <= 1 {
            let (lo, hi) = along(row, rows);
            return (0..cols.min(1), clamp(lo, hi, rows));
        }
        // Degenerate lattice: every element (bounded by the frame budget).
        return (0..cols, 0..rows);
    }
    // Solve p = i·col + j·row for the corners of `want`.
    let (mut ilo, mut ihi, mut jlo, mut jhi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in [
        Vec2::new(want.x0, want.y0),
        Vec2::new(want.x1, want.y0),
        Vec2::new(want.x0, want.y1),
        Vec2::new(want.x1, want.y1),
    ] {
        let i = p.cross(row) / det;
        let j = col.cross(p) / det;
        ilo = ilo.min(i);
        ihi = ihi.max(i);
        jlo = jlo.min(j);
        jhi = jhi.max(j);
    }
    (clamp(ilo, ihi, cols), clamp(jlo, jhi, rows))
}

/// The view transform that fits `rect` (sheet coordinates, y up) into `size` pixels with
/// `margin` pixels around it.
pub fn fit_transform(rect: Rect, size: (u32, u32), margin: f64) -> Affine {
    let w = f64::from(size.0.max(1));
    let h = f64::from(size.1.max(1));
    let rw = rect.width().max(1e-9);
    let rh = rect.height().max(1e-9);
    let s = ((w - 2.0 * margin).max(1.0) / rw).min((h - 2.0 * margin).max(1.0) / rh);
    let s = if s.is_finite() && s > 0.0 { s } else { 1.0 };
    let c = rect.center();
    Affine::translate((w / 2.0, h / 2.0))
        * Affine::scale_non_uniform(s, -s)
        * Affine::translate(-c.to_vec2())
}

#[cfg(test)]
mod style_tests {
    use super::*;
    use crate::fonts::Fonts;
    use crate::prep::{PrepLimits, PreparedSheet, prepare};
    use crate::scene::{Generated, Item, Layer, Shape, Sheet, SheetKind, Units, VectorScene};
    use std::sync::Arc;

    /// What the sink was asked to draw: kind, width or radius in pixels, colour.
    #[derive(Default)]
    struct Log(Vec<(&'static str, f64, [u8; 4])>);

    impl Sink for Log {
        fn stroke(&mut self, _: PathRef<'_>, t: Affine, width: f64, c: [u8; 4]) {
            self.0
                .push(("stroke", width * t.determinant().abs().sqrt(), c));
        }
        fn fill(&mut self, _: PathRef<'_>, _: Affine, _: bool, c: [u8; 4]) {
            self.0.push(("fill", 0.0, c));
        }
        fn glyph(&mut self, _: &BezPath, _: Affine, _: [u8; 4]) {}
        fn dot(&mut self, _: Point, r: f64, c: [u8; 4]) {
            self.0.push(("dot", r, c));
        }
        fn push_clip(&mut self, _: &BezPath, _: Affine) {}
        fn pop_clip(&mut self) {}
    }

    fn sheet() -> Arc<PreparedSheet> {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((50.0, 0.0));
        path.line_to((50.0, 50.0));
        path.close_path();
        let item = |layer, shape| Item {
            layer,
            outer: layer,
            color: Color::Rgb(10, 20, 30),
            shape,
        };
        let mut sheet = Sheet::generated(Generated::Map, SheetKind::Model);
        sheet.items = vec![
            item(
                0,
                Shape::Fill {
                    path: path.clone(),
                    even_odd: false,
                },
            ),
            item(
                0,
                Shape::Stroke {
                    path,
                    weight: 25,
                    dash: None,
                },
            ),
            item(1, Shape::Point(Point::new(10.0, 10.0))),
        ];
        sheet.extents = Some(Rect::new(0.0, 0.0, 50.0, 50.0));
        let layer = |name: &str| Layer {
            name: name.into(),
            generated: None,
            color: Color::Rgb(10, 20, 30),
            linetype: None,
            lineweight: 25,
            on: true,
            frozen: false,
            plot: true,
            items: 1,
        };
        let s = VectorScene {
            layer_opacity: Vec::new(),
            pcb: None,
            survey: None,
            format: "test".into(),
            version: String::new(),
            release: None,
            units: Units { code: 0 },
            layers: vec![layer("a"), layer("b")],
            linetypes: Vec::new(),
            fonts: vec![crate::FontSpec::default()],
            sheets: vec![sheet],
            counts: Vec::new(),
            warnings: Vec::new(),
            lineweight_display: false,
            point_mode: 0,
            point_size: 0.0,
            codepage: None,
            dot_radius: 3.0,
            plot: None,
            layout: None,
        };
        prepare(Arc::new(s), &Fonts::bundled_only(), PrepLimits::default()).sheets[0].clone()
    }

    #[test]
    fn layer_styles_override_colour_width_size_and_opacity() {
        let sheet = sheet();
        let layers = [true, true];
        let view = View {
            transform: Affine::IDENTITY,
            size: (100, 100),
            dark: false,
            background: [255, 255, 255],
            lineweights: false,
            dpi_scale: 2.0,
            layers: &layers,
        };
        let mut plain = Log::default();
        draw(&sheet, &view, &mut plain);
        let styles = [
            LayerStyle {
                stroke: Some([200, 0, 0]),
                fill: Some([0, 200, 0, 128]),
                fill_alpha: None,
                opacity: 0.5,
                line_px: Some(3.0),
                dot_px: None,
            },
            LayerStyle {
                dot_px: Some(6.0),
                ..LayerStyle::default()
            },
        ];
        let mut styled = Log::default();
        draw_styled(&sheet, &view, &styles, &mut styled);
        let find = |l: &Log, k: &str| *l.0.iter().find(|e| e.0 == k).expect(k);
        // Unstyled: the scene's colour, a hairline, the scene's dot.
        assert_eq!(find(&plain, "fill").2, [10, 20, 30, 255]);
        assert_eq!(find(&plain, "stroke").1, 2.0);
        assert_eq!(find(&plain, "dot").1, 6.0);
        // Styled: colours replaced, opacity applied, widths in logical px times the scale.
        assert_eq!(find(&styled, "fill").2, [0, 200, 0, 64]);
        let s = find(&styled, "stroke");
        assert_eq!((s.1, s.2), (6.0, [200, 0, 0, 128]));
        let d = find(&styled, "dot");
        assert_eq!((d.1, d.2), (12.0, [10, 20, 30, 255]));
        // A fill in its own colour, translucent.
        let own = [LayerStyle {
            fill_alpha: Some(100),
            ..LayerStyle::default()
        }];
        let mut log = Log::default();
        draw_styled(&sheet, &view, &own, &mut log);
        assert_eq!(find(&log, "fill").2, [10, 20, 30, 100]);
    }
}
