//! A library as a vector scene: one sheet per top cell, the cells as layout cells drawn
//! instanced, micrometre units, and a colour per layer/datatype from an own palette.

use specular_vector::kurbo::{Affine, BezPath, Rect, Vec2};
use specular_vector::{
    CellArray, CellRef, Color, FontSpec, HAlign, Item, Layer, Layout, LayoutInfo, Paragraph, Shape,
    Sheet, SheetKind, Span, TextBlock, Units, VAlign, VectorScene, draw,
};

use crate::{Library, ShapeKind, hier, warn};

/// Sheets made at most (a library of unplaced cells has a top cell each).
pub const MAX_SHEETS: usize = 4096;

/// Fill opacity of shapes (outlines are opaque).
pub const FILL_ALPHA: u8 = 96;

/// Labels smaller than this many pixels are hidden.
pub const LABEL_MIN_PX: f32 = 6.0;

/// A stable colour for a layer/datatype pair: the layer picks the hue (golden-ratio
/// steps, so neighbouring layers differ), the datatype the lightness and saturation.
pub fn layer_color(layer: u16, datatype: u16) -> Color {
    const PHI: f64 = 0.618_033_988_749_895;
    let h = (0.13 + f64::from(layer) * PHI).fract();
    let d = f64::from(datatype % 4);
    let s = 0.80 - 0.12 * d;
    let l = [0.58, 0.45, 0.70, 0.36][datatype as usize % 4];
    let (r, g, b) = hsl(h, s, l);
    Color::Rgb(r, g, b)
}

fn hsl(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h * 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let q = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (q(r), q(g), q(b))
}

fn text_block(lib: &Library, t: &crate::Text) -> TextBlock {
    // MAG sizes a label in user units; 1 user unit when not given.
    let mag = if t.strans.mag.is_finite() && t.strans.mag > 0.0 {
        t.strans.mag
    } else {
        1.0
    };
    let height = mag / lib.user_unit.max(1e-12);
    let mut s = t.strans.clone();
    s.mag = 1.0;
    let halign = match t.presentation & 3 {
        1 => HAlign::Center,
        2 => HAlign::Right,
        _ => HAlign::Left,
    };
    let valign = match (t.presentation >> 2) & 3 {
        0 => VAlign::Top,
        1 => VAlign::Middle,
        _ => VAlign::Baseline,
    };
    TextBlock {
        transform: crate::geom::placement(t.origin, &s),
        halign,
        valign,
        wrap: 0.0,
        fit: None,
        line_spacing: 1.0,
        paragraphs: t
            .text
            .split('\n')
            .map(|line| Paragraph {
                spans: vec![Span::new(line.trim_end_matches('\r'), 0, height)],
                align: None,
            })
            .collect(),
    }
}

/// The library as a scene.
pub fn to_scene(lib: &Library) -> VectorScene {
    let um = lib.db_unit_m * 1e6;
    let colors: Vec<Color> = lib.layers.iter().map(|&(l, d)| layer_color(l, d)).collect();
    let mut layer_items = vec![0u64; lib.layers.len()];
    let mut cells = Vec::with_capacity(lib.cells.len());
    for cell in &lib.cells {
        let mut items = Vec::with_capacity(cell.shapes.len() + cell.texts.len());
        for s in &cell.shapes {
            let poly = hier::shape_polygon(cell, s, Affine::IDENTITY);
            if matches!(s.kind, ShapeKind::Path { width: 0, .. }) && poly.len() >= 2 {
                // A zero-width path (how KLayout writes edges) is a hairline along it.
                let n = poly.len() / 2 + 1;
                let mut path = BezPath::new();
                path.move_to(poly[0]);
                for &p in &poly[1..n] {
                    path.line_to(p);
                }
                layer_items[s.layer as usize] += 1;
                items.push(Item {
                    layer: s.layer,
                    outer: s.layer,
                    color: colors[s.layer as usize],
                    shape: Shape::Stroke {
                        path,
                        weight: 0,
                        dash: None,
                    },
                });
                continue;
            }
            if poly.len() < 3 {
                continue;
            }
            let mut path = BezPath::new();
            path.move_to(poly[0]);
            for &p in &poly[1..] {
                path.line_to(p);
            }
            path.close_path();
            layer_items[s.layer as usize] += 1;
            items.push(Item {
                layer: s.layer,
                outer: s.layer,
                color: colors[s.layer as usize],
                shape: Shape::Fill {
                    path,
                    even_odd: false,
                },
            });
        }
        for t in &cell.texts {
            if t.text.trim().is_empty() {
                continue;
            }
            layer_items[t.layer as usize] += 1;
            items.push(Item {
                layer: t.layer,
                outer: t.layer,
                color: colors[t.layer as usize],
                shape: Shape::Text(Box::new(text_block(lib, t))),
            });
        }
        let refs = cell
            .refs
            .iter()
            .filter_map(|r| {
                Some(CellRef {
                    cell: r.cell?,
                    transform: hier::ref_transform(Affine::IDENTITY, r),
                    array: r.array.map(|a| CellArray {
                        cols: u32::from(a.cols),
                        rows: u32::from(a.rows),
                        col: Vec2::new(a.col[0], a.col[1]),
                        row: Vec2::new(a.row[0], a.row[1]),
                    }),
                })
            })
            .collect();
        cells.push(specular_vector::Cell {
            name: cell.name.clone(),
            items,
            refs,
        });
    }

    let hulls = hier::hulls(lib);
    let flat = hier::flat_counts(lib);
    let mut warnings = lib.warnings.clone();
    let mut sheets = Vec::new();
    let mut bbox_um: Option<Rect> = None;
    let mut flat_shapes = 0u64;
    for &t in &lib.top {
        let ext =
            hier::hull_bounds(&hulls[t as usize]).map(|r| Affine::scale(um).transform_rect_bbox(r));
        flat_shapes = flat_shapes.saturating_add(flat[t as usize]);
        if let Some(e) = ext {
            bbox_um = Some(bbox_um.map_or(e, |b| b.union(e)));
        }
        if sheets.len() >= MAX_SHEETS {
            continue;
        }
        let mut sheet = Sheet::new(lib.cells[t as usize].name.clone(), SheetKind::Model);
        sheet.extents = ext;
        sheet.refs.push(CellRef {
            cell: t,
            transform: Affine::scale(um),
            array: None,
        });
        sheets.push(sheet);
    }
    if lib.top.len() > MAX_SHEETS {
        warn(
            &mut warnings,
            format!(
                "{} top cells; only the first {MAX_SHEETS} are shown as pages",
                lib.top.len()
            ),
        );
    }
    if sheets.is_empty() {
        sheets.push(Sheet::new(lib.name.clone(), SheetKind::Model));
    }
    if flat_shapes > draw::DETAIL_BUDGET as u64 {
        warn(
            &mut warnings,
            format!(
                "{flat_shapes} shapes when expanded: zoomed out, small or numerous cells are drawn as filled rectangles"
            ),
        );
    }

    let c = lib.counts;
    let mut counts: Vec<(String, u64)> = [
        ("AREF", c.arefs),
        ("BOUNDARY", c.boundaries),
        ("BOX", c.boxes),
        ("NODE", c.nodes),
        ("PATH", c.paths),
        ("PROPERTY", c.properties),
        ("SREF", c.srefs),
        ("TEXT", c.texts),
    ]
    .into_iter()
    .filter(|e| e.1 > 0)
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    counts.sort();

    let layers = lib
        .layers
        .iter()
        .enumerate()
        .map(|(i, _)| Layer {
            name: lib.layer_name(i as u32),
            generated: None,
            color: colors[i],
            linetype: None,
            lineweight: 0,
            on: true,
            frozen: false,
            plot: true,
            items: layer_items[i],
        })
        .collect();

    let placements = lib
        .cells
        .iter()
        .flat_map(|c| c.refs.iter())
        .fold(0u64, |n, r| n.saturating_add(r.count()));
    let info = LayoutInfo {
        library: lib.name.clone(),
        db_unit_m: lib.db_unit_m,
        user_unit: lib.user_unit,
        cells: lib.cells.len(),
        top_cells: lib
            .top
            .iter()
            .map(|&t| lib.cells[t as usize].name.clone())
            .collect(),
        layers: lib.layers.len(),
        shapes: c.boundaries + c.paths + c.boxes,
        texts: c.texts,
        references: c.srefs + c.arefs,
        placements,
        flat_shapes,
        bbox_um,
    };
    VectorScene {
        layer_opacity: Vec::new(),
        pcb: None,
        survey: None,
        format: "GDSII".into(),
        version: lib.version.to_string(),
        release: None,
        units: Units { code: 13 },
        layers,
        linetypes: Vec::new(),
        fonts: vec![FontSpec::default()],
        sheets,
        counts,
        warnings,
        lineweight_display: false,
        point_mode: 0,
        point_size: 0.0,
        codepage: None,
        dot_radius: 0.0,
        plot: None,
        layout: Some(Box::new(Layout {
            cells,
            fill_alpha: FILL_ALPHA,
            label_min_px: LABEL_MIN_PX,
            info,
        })),
    }
}
