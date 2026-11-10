//! A CPU rasteriser (tiny-skia) for thumbnails, the first picture before the GPU renderer
//! is ready, and machines without one.

use kurbo::{Affine, BezPath, PathEl, Point};
use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::draw::{self, Sink, Stats, View};
use crate::prep::{PathRef, PreparedSheet};

/// Renders a view into RGBA8 (opaque, the view's background).
pub fn render(sheet: &PreparedSheet, view: &View<'_>) -> Option<(Vec<u8>, Stats)> {
    let (w, h) = view.size;
    let mut pixmap = Pixmap::new(w.max(1), h.max(1))?;
    let [r, g, b] = view.background;
    pixmap.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    let mut sink = Raster {
        pixmap,
        mask: None,
        paint: Paint {
            anti_alias: true,
            ..Default::default()
        },
        below: Vec::new(),
    };
    let stats = draw::draw(sheet, view, &mut sink);
    Some((sink.pixmap.take(), stats))
}

struct Raster {
    pixmap: Pixmap,
    mask: Option<Mask>,
    paint: Paint<'static>,
    /// Pictures under the open groups, innermost last, with the group's opacity.
    below: Vec<(Pixmap, f32)>,
}

fn ts(a: Affine) -> Transform {
    let c = a.as_coeffs();
    Transform::from_row(
        c[0] as f32,
        c[1] as f32,
        c[2] as f32,
        c[3] as f32,
        c[4] as f32,
        c[5] as f32,
    )
}

fn to_path(els: impl Iterator<Item = PathEl>) -> Option<tiny_skia::Path> {
    let mut b = PathBuilder::new();
    let f = |p: Point| (p.x as f32, p.y as f32);
    for el in els {
        match el {
            PathEl::MoveTo(p) => {
                let (x, y) = f(p);
                b.move_to(x, y);
            }
            PathEl::LineTo(p) => {
                let (x, y) = f(p);
                b.line_to(x, y);
            }
            PathEl::QuadTo(a, p) => {
                let ((ax, ay), (x, y)) = (f(a), f(p));
                b.quad_to(ax, ay, x, y);
            }
            PathEl::CurveTo(a, c, p) => {
                let ((ax, ay), (cx, cy), (x, y)) = (f(a), f(c), f(p));
                b.cubic_to(ax, ay, cx, cy, x, y);
            }
            PathEl::ClosePath => b.close(),
        }
    }
    b.finish()
}

impl Raster {
    fn color(&mut self, c: [u8; 4]) {
        self.paint.set_color_rgba8(c[0], c[1], c[2], c[3]);
    }
}

impl Sink for Raster {
    fn stroke(&mut self, path: PathRef<'_>, transform: Affine, width: f64, color: [u8; 4]) {
        let Some(p) = to_path(path.iter()) else {
            return;
        };
        self.color(color);
        let stroke = Stroke {
            width: width as f32,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        self.pixmap
            .stroke_path(&p, &self.paint, &stroke, ts(transform), self.mask.as_ref());
    }

    fn fill(&mut self, path: PathRef<'_>, transform: Affine, even_odd: bool, color: [u8; 4]) {
        let Some(p) = to_path(path.iter()) else {
            return;
        };
        self.color(color);
        let rule = if even_odd {
            FillRule::EvenOdd
        } else {
            FillRule::Winding
        };
        self.pixmap
            .fill_path(&p, &self.paint, rule, ts(transform), self.mask.as_ref());
    }

    fn glyph(&mut self, path: &BezPath, transform: Affine, color: [u8; 4]) {
        let Some(p) = to_path(path.elements().iter().copied()) else {
            return;
        };
        self.color(color);
        self.pixmap.fill_path(
            &p,
            &self.paint,
            FillRule::Winding,
            ts(transform),
            self.mask.as_ref(),
        );
    }

    fn dot(&mut self, center: Point, radius: f64, color: [u8; 4]) {
        let Some(p) =
            PathBuilder::from_circle(center.x as f32, center.y as f32, radius.max(0.5) as f32)
        else {
            return;
        };
        self.color(color);
        self.pixmap.fill_path(
            &p,
            &self.paint,
            FillRule::Winding,
            Transform::identity(),
            self.mask.as_ref(),
        );
    }

    fn push_clip(&mut self, clip: &BezPath, transform: Affine) {
        let Some(mut m) = Mask::new(self.pixmap.width(), self.pixmap.height()) else {
            return;
        };
        if let Some(p) = to_path(clip.elements().iter().copied()) {
            m.fill_path(&p, FillRule::Winding, true, ts(transform));
        }
        self.mask = Some(m);
    }

    fn pop_clip(&mut self) {
        self.mask = None;
    }

    fn push_group(&mut self, opacity: f32) {
        let Some(layer) = Pixmap::new(self.pixmap.width(), self.pixmap.height()) else {
            return;
        };
        let under = std::mem::replace(&mut self.pixmap, layer);
        self.below.push((under, opacity));
    }

    fn pop_group(&mut self) {
        let Some((under, opacity)) = self.below.pop() else {
            return;
        };
        let layer = std::mem::replace(&mut self.pixmap, under);
        let paint = tiny_skia::PixmapPaint {
            opacity,
            ..Default::default()
        };
        self.pixmap
            .draw_pixmap(0, 0, layer.as_ref(), &paint, Transform::identity(), None);
    }

    fn set_erase(&mut self, erase: bool) {
        self.paint.blend_mode = if erase {
            tiny_skia::BlendMode::DestinationOut
        } else {
            tiny_skia::BlendMode::SourceOver
        };
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kurbo::Rect;

    use super::*;
    use crate::fonts::Fonts;
    use crate::prep::{PrepLimits, prepare};
    use crate::scene::*;

    #[test]
    fn draws_a_line_and_hides_its_layer() {
        let mut s = VectorScene::default();
        for name in ["0", "WALLS"] {
            s.layers.push(Layer {
                name: name.into(),
                generated: None,
                color: Color::Foreground,
                linetype: None,
                lineweight: LW_DEFAULT,
                on: true,
                frozen: false,
                plot: true,
                items: 1,
            });
        }
        let mut sheet = Sheet::new("Model", SheetKind::Model);
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((100.0, 100.0));
        sheet.items.push(Item {
            layer: 1,
            outer: 1,
            color: Color::Foreground,
            shape: Shape::Stroke {
                path: p,
                weight: LW_DEFAULT,
                dash: None,
            },
        });
        s.sheets.push(sheet);
        let prep = prepare(Arc::new(s), &Fonts::bundled_only(), PrepLimits::default());
        let sheet = &prep.sheets[0];
        let fit = sheet.fit.unwrap_or(Rect::ZERO);
        let xf = draw::fit_transform(fit, (64, 64), 4.0);
        let layers = [true, true];
        let mut view = View {
            transform: xf,
            size: (64, 64),
            dark: false,
            background: [255, 255, 255],
            lineweights: false,
            dpi_scale: 1.0,
            layers: &layers,
        };
        let (px, stats) = render(sheet, &view).unwrap();
        assert_eq!(stats.drawn, 1);
        let dark = px.chunks(4).filter(|p| p[0] < 128).count();
        assert!(dark > 50, "{dark}");
        // The diagonal runs from the bottom left (y up) to the top right.
        let at = |x: usize, y: usize| px[(y * 64 + x) * 4];
        assert!(at(5, 58) < 128 && at(58, 5) < 128);
        let hidden = [true, false];
        view.layers = &hidden;
        let (px, stats) = render(sheet, &view).unwrap();
        assert_eq!(stats.hidden, 1);
        assert!(px.chunks(4).all(|p| p[0] == 255));
        view.dark = true;
        view.background = [0, 0, 0];
        view.layers = &layers;
        let (px, _) = render(sheet, &view).unwrap();
        assert!(px.chunks(4).filter(|p| p[0] > 128).count() > 50);
    }

    /// Grouped layers: two overlapping squares of a half-opaque layer do not darken each
    /// other, and a clear square erases what its group drew but not the layer under it.
    #[test]
    fn groups_composite_once_and_clear_erases_within_its_group() {
        let mut s = VectorScene::default();
        for name in ["under", "over"] {
            s.layers.push(Layer {
                name: name.into(),
                generated: None,
                color: Color::Rgb(0, 0, 0),
                linetype: None,
                lineweight: LW_DEFAULT,
                on: true,
                frozen: false,
                plot: true,
                items: 1,
            });
        }
        s.layer_opacity = vec![1.0, 0.5];
        let square = |x0: f64, y0: f64, x1: f64, y1: f64| {
            kurbo::Shape::to_path(&Rect::new(x0, y0, x1, y1), 0.1)
        };
        let mut sheet = Sheet::new("Model", SheetKind::Model);
        let fill = |layer: u32, color: Color, path: BezPath| Item {
            layer,
            outer: layer,
            color,
            shape: Shape::Fill {
                path,
                even_odd: false,
            },
        };
        // Layer 0: a bar along the bottom. Layer 1: two overlapping squares, then a clear
        // square over their overlap.
        sheet
            .items
            .push(fill(0, Color::Rgb(0, 0, 0), square(0.0, 0.0, 64.0, 8.0)));
        sheet
            .items
            .push(fill(1, Color::Rgb(0, 0, 0), square(0.0, 0.0, 40.0, 64.0)));
        sheet
            .items
            .push(fill(1, Color::Rgb(0, 0, 0), square(24.0, 0.0, 64.0, 64.0)));
        sheet
            .items
            .push(fill(1, Color::Clear, square(28.0, 0.0, 36.0, 64.0)));
        s.sheets.push(sheet);
        let p = prepare(Arc::new(s), &Fonts::bundled_only(), PrepLimits::default());
        let sheet = &p.sheets[0];
        let layers = [true, true];
        let view = View {
            transform: kurbo::Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, 64.0]),
            size: (64, 64),
            dark: false,
            background: [255, 255, 255],
            lineweights: false,
            dpi_scale: 1.0,
            layers: &layers,
        };
        let (px, _) = render(sheet, &view).unwrap();
        // y down in pixels: row 32 is mid-height, row 60 is on the bar.
        let at = |x: usize, y: usize| px[(y * 64 + x) * 4];
        let half = at(10, 32);
        assert!((120..=136).contains(&half), "{half}");
        // The overlap is no darker than either square alone.
        assert_eq!(at(30 - 4, 32), half);
        // The clear square shows the paper, and the bar of the layer under it.
        assert_eq!(at(32, 32), 255);
        assert!(at(32, 60) < 10);
    }

    /// A layout: a cell with one square placed as a 50 × 40 array, then the array cell
    /// placed twice (once mirrored).
    fn layout_scene() -> VectorScene {
        let mut s = VectorScene::default();
        s.layers.push(Layer {
            name: "1/0".into(),
            generated: None,
            color: Color::Rgb(200, 0, 0),
            linetype: None,
            lineweight: 0,
            on: true,
            frozen: false,
            plot: true,
            items: 1,
        });
        let mut sq = BezPath::new();
        sq.move_to((0.0, 0.0));
        sq.line_to((8.0, 0.0));
        sq.line_to((8.0, 8.0));
        sq.line_to((0.0, 8.0));
        sq.close_path();
        let unit = Cell {
            name: "UNIT".into(),
            items: vec![Item {
                layer: 0,
                outer: 0,
                color: Color::Rgb(200, 0, 0),
                shape: Shape::Fill {
                    path: sq,
                    even_odd: false,
                },
            }],
            refs: Vec::new(),
        };
        let arr = Cell {
            name: "ARRAY".into(),
            items: Vec::new(),
            refs: vec![CellRef {
                cell: 0,
                transform: Affine::IDENTITY,
                array: Some(CellArray {
                    cols: 50,
                    rows: 40,
                    col: kurbo::Vec2::new(10.0, 0.0),
                    row: kurbo::Vec2::new(0.0, 10.0),
                }),
            }],
        };
        let top = Cell {
            name: "TOP".into(),
            items: Vec::new(),
            refs: vec![
                CellRef {
                    cell: 1,
                    transform: Affine::IDENTITY,
                    array: None,
                },
                CellRef {
                    cell: 1,
                    transform: Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, -10.0]),
                    array: None,
                },
            ],
        };
        s.layout = Some(Box::new(Layout {
            cells: vec![unit, arr, top],
            fill_alpha: 128,
            label_min_px: 6.0,
            info: LayoutInfo::default(),
        }));
        let mut sheet = Sheet::new("TOP", SheetKind::Model);
        sheet.refs.push(CellRef {
            cell: 2,
            transform: Affine::IDENTITY,
            array: None,
        });
        s.sheets.push(sheet);
        s
    }

    #[test]
    fn draws_layout_cells_instanced() {
        let prep = prepare(
            Arc::new(layout_scene()),
            &Fonts::bundled_only(),
            PrepLimits::default(),
        );
        let sheet = &prep.sheets[0];
        let fit = sheet.fit.unwrap();
        assert_eq!(
            (fit.x0, fit.y0, fit.x1, fit.y1),
            (0.0, -408.0, 498.0, 398.0)
        );
        let layers = [true];
        let mut view = View {
            transform: draw::fit_transform(fit, (100, 100), 0.0),
            size: (100, 100),
            dark: false,
            background: [255, 255, 255],
            lineweights: false,
            dpi_scale: 1.0,
            layers: &layers,
        };
        // Fitted, a square is a pixel, spaced under two: one rectangle per array.
        let (_, stats) = render(sheet, &view).unwrap();
        assert_eq!((stats.instances, stats.stand_ins), (3, 2), "{stats:?}");
        view.size = (200, 200);
        // Zoomed in on a corner, only the visible elements are drawn in detail.
        view.transform =
            draw::fit_transform(kurbo::Rect::new(0.0, 0.0, 40.0, 40.0), (200, 200), 0.0);
        let (px, stats) = render(sheet, &view).unwrap();
        assert!((18..=40).contains(&stats.instances), "{stats:?}");
        assert_eq!(stats.stand_ins, 0);
        assert!(!stats.truncated);
        // Semi-transparent fill with an opaque outline.
        let at = |x: usize, y: usize| &px[(y * 200 + x) * 4..(y * 200 + x) * 4 + 3];
        let inside = at(20, 180);
        assert!(
            inside[0] > 200 && inside[1] > 100 && inside[1] < 160,
            "{inside:?}"
        );
        // Hidden layer: nothing drawn, not even the stand-ins.
        let hidden = [false];
        view.layers = &hidden;
        view.transform = draw::fit_transform(fit, (200, 200), 0.0);
        let (px, stats) = render(sheet, &view).unwrap();
        assert_eq!(stats.drawn, 0);
        assert!(px.chunks(4).all(|p| p[0] == 255 && p[1] == 255));
    }

    #[test]
    fn lattice_ranges() {
        use kurbo::{Rect, Vec2};
        let (c, r) = draw::lattice_range(
            Rect::new(25.0, -5.0, 45.0, 15.0),
            10,
            10,
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, 10.0),
        );
        assert_eq!((c, r), (2..6, 0..3));
        // One row, the row step meaningless.
        let (c, r) = draw::lattice_range(
            Rect::new(25.0, -5.0, 45.0, 5.0),
            10,
            1,
            Vec2::new(10.0, 0.0),
            Vec2::ZERO,
        );
        assert_eq!((c, r), (2..6, 0..1));
        // Outside.
        let (c, _) = draw::lattice_range(
            Rect::new(-50.0, -5.0, -20.0, 5.0),
            10,
            1,
            Vec2::new(10.0, 0.0),
            Vec2::ZERO,
        );
        assert!(c.is_empty());
    }
}
