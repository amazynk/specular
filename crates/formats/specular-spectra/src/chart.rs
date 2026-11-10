//! Plots as sheets of a vector scene: the traces, sticks and contours are items in sheet
//! coordinates inside a frame, and the sheet's [`Chart`] maps them back to values, so the
//! viewer draws the axes, ticks and peak marks over them at the exact zoom.
//!
//! A sheet is a layout: its frame is [`FRAME_W`] × [`FRAME_H`] units (y up) and its paper
//! leaves room around the frame for the axes. Each trace is a layer (its name, else a made-up
//! "Trace n"), contours are two layers (above and below zero), and the frame one more, so
//! the layers panel shows and hides them. Long traces are cut into runs of
//! [`RUN`] points so a zoomed view culls what it does not show.

use specular_vector::kurbo::{BezPath, Point, Rect};
use specular_vector::{
    Chart, ChartAxis, ChartMark, Color, FontSpec, Generated, Item, Layer, Shape, Sheet, SheetKind,
    Units, VectorScene,
};

use crate::model::{Axis, Grid, Plot, SpectraFile, Trace};

/// The plot area, sheet units.
pub const FRAME_W: f64 = 1600.0;
pub const FRAME_H: f64 = 1000.0;
/// Room around the frame for the viewer's tick labels and axis titles, sheet units: left,
/// bottom, right, top.
const MARGIN: [f64; 4] = [150.0, 120.0, 40.0, 60.0];

/// The sheet's paper: the frame and the room around it, so fitting the sheet shows the axes.
fn paper() -> Rect {
    Rect::new(
        -MARGIN[0],
        -MARGIN[1],
        FRAME_W + MARGIN[2],
        FRAME_H + MARGIN[3],
    )
}

/// Points per item of a long trace.
pub const RUN: usize = 4096;
/// Contour levels on each side of zero at most.
const LEVELS: usize = 12;
/// Ratio between successive contour levels.
const LEVEL_STEP: f64 = 1.4;

/// Trace colours after the first (which is the foreground): Okabe–Ito, readable on light and
/// dark paper and by most colour-blind readers.
const PALETTE: [(u8, u8, u8); 6] = [
    (0, 114, 178),
    (213, 94, 0),
    (0, 158, 115),
    (204, 121, 167),
    (230, 159, 0),
    (86, 180, 233),
];
/// Negative contour levels.
const NEGATIVE: (u8, u8, u8) = (213, 94, 0);

/// The scene of `pages` of `file` (page errors become warnings and empty sheets), with the
/// file's format and version.
pub fn scene(file: &dyn SpectraFile, pages: impl IntoIterator<Item = usize>) -> VectorScene {
    let facts = file.facts();
    let mut b = Builder::default();
    let mut warnings = facts.warnings.clone();
    for p in pages {
        let sheet = match file.page(p) {
            Ok(plot) => b.sheet(&plot),
            Err(e) => {
                crate::model::warn(&mut warnings, format!("{}: {e}", file.page_name(p)));
                let mut s = Sheet::new(file.page_name(p), SheetKind::Layout);
                s.extents = Some(Rect::new(0.0, 0.0, FRAME_W, FRAME_H));
                s.paper = Some(paper());
                s
            }
        };
        b.sheets.push(sheet);
    }
    VectorScene {
        format: facts.format.clone(),
        version: facts.version.clone(),
        units: Units { code: 0 },
        layers: b.layers,
        fonts: vec![FontSpec::default()],
        sheets: b.sheets,
        warnings,
        ..VectorScene::default()
    }
}

#[derive(Default)]
struct Builder {
    layers: Vec<Layer>,
    sheets: Vec<Sheet>,
}

fn layer(name: String, generated: Option<Generated>, color: Color) -> Layer {
    Layer {
        name,
        generated,
        color,
        linetype: None,
        lineweight: 18,
        on: true,
        frozen: false,
        plot: true,
        items: 0,
    }
}

impl Builder {
    /// The index of the layer named `name`, added with `color` when new.
    fn layer(&mut self, name: &str, generated: Option<Generated>, color: Color) -> u32 {
        if let Some(i) = self
            .layers
            .iter()
            .position(|l| l.name == name && l.generated == generated)
        {
            return i as u32;
        }
        self.layers.push(layer(name.to_owned(), generated, color));
        (self.layers.len() - 1) as u32
    }

    /// The layer of a name Specular made up.
    fn made_up(&mut self, g: Generated, color: Color) -> u32 {
        self.layer(&g.english(), Some(g), color)
    }

    fn push(&mut self, sheet: &mut Sheet, layer: u32, shape: Shape) {
        let color = self.layers[layer as usize].color;
        self.layers[layer as usize].items += 1;
        sheet.items.push(Item {
            layer,
            outer: layer,
            color,
            shape,
        });
    }

    fn sheet(&mut self, plot: &Plot) -> Sheet {
        let frame = Rect::new(0.0, 0.0, FRAME_W, FRAME_H);
        let chart = chart_of(plot, frame);
        let mut sheet = Sheet::new(plot.title.clone(), SheetKind::Layout);
        sheet.extents = Some(frame);
        sheet.paper = Some(paper());
        let frame_layer = self.made_up(Generated::ChartFrame, Color::Foreground);
        let mut fp = BezPath::new();
        fp.move_to((frame.x0, frame.y0));
        fp.line_to((frame.x1, frame.y0));
        fp.line_to((frame.x1, frame.y1));
        fp.line_to((frame.x0, frame.y1));
        fp.close_path();
        self.push(
            &mut sheet,
            frame_layer,
            Shape::Stroke {
                path: fp,
                weight: 18,
                dash: None,
            },
        );
        if let Some(g) = &plot.grid {
            self.contours(&mut sheet, &chart, g);
        }
        let mut legend = Vec::new();
        for (i, t) in plot.traces.iter().enumerate() {
            let color = if i == 0 {
                Color::Foreground
            } else {
                let (r, g, b) = PALETTE[(i - 1) % PALETTE.len()];
                Color::Rgb(r, g, b)
            };
            let l = if t.name.is_empty() {
                self.made_up(Generated::Trace(i as u32 + 1), color)
            } else {
                self.layer(&t.name, None, color)
            };
            if plot.traces.len() > 1 && !legend.contains(&l) {
                legend.push(l);
            }
            for path in trace_paths(&chart, t) {
                self.push(
                    &mut sheet,
                    l,
                    Shape::Stroke {
                        path,
                        weight: 18,
                        dash: None,
                    },
                );
            }
        }
        sheet.chart = Some(Box::new(Chart { legend, ..chart }));
        sheet
    }

    fn contours(&mut self, sheet: &mut Sheet, chart: &Chart, g: &Grid) {
        let Some(levels) = contour_levels(&g.z) else {
            return;
        };
        let pos = self.made_up(Generated::Contours { negative: false }, Color::Foreground);
        let (r, gr, b) = NEGATIVE;
        let neg = self.made_up(Generated::Contours { negative: true }, Color::Rgb(r, gr, b));
        for (lv, layer) in levels
            .iter()
            .map(|&l| (l, pos))
            .chain(levels.iter().map(|&l| (-l, neg)))
        {
            for path in contour_paths(chart, g, lv) {
                self.push(
                    sheet,
                    layer,
                    Shape::Stroke {
                        path,
                        weight: 13,
                        dash: None,
                    },
                );
            }
        }
    }
}

/// Values range of finite numbers.
fn range(v: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    let mut r: Option<(f64, f64)> = None;
    for x in v.filter(|x| x.is_finite()) {
        r = Some(match r {
            None => (x, x),
            Some((a, b)) => (a.min(x), b.max(x)),
        });
    }
    r
}

/// An axis spanning `lo..hi` in `a`'s direction.
fn axis(a: &Axis, lo: f64, hi: f64) -> ChartAxis {
    let (lo, hi) = if hi > lo {
        (lo, hi)
    } else {
        let d = if lo == 0.0 { 1.0 } else { lo.abs() * 0.01 };
        (lo - d, lo + d)
    };
    let (start, end) = if a.reversed { (hi, lo) } else { (lo, hi) };
    ChartAxis {
        title: a.title.clone(),
        unit: a.unit.clone(),
        start,
        end,
    }
}

/// The chart of `plot` in `frame`: x over every trace's and the grid's extent (5 % wider
/// either side when there are sticks, which would otherwise stand on the frame's sides), y
/// over the traces with 5 % headroom (from 0 for sticks), or the grid's y extent for a 2-D
/// plot.
pub fn chart_of(plot: &Plot, frame: Rect) -> Chart {
    let xs = plot.traces.iter().flat_map(|t| t.x.iter().copied()).chain(
        plot.grid
            .iter()
            .flat_map(|g| [g.x_range.0, g.x_range.1].into_iter()),
    );
    let (mut x0, mut x1) = range(xs).unwrap_or((0.0, 1.0));
    if plot.traces.iter().any(|t| t.sticks) {
        let pad = if x1 > x0 { (x1 - x0) * 0.05 } else { 1.0 };
        (x0, x1) = (x0 - pad, x1 + pad);
    }
    let (y0, y1) = match &plot.grid {
        Some(g) => {
            let (a, b) = g.y_range;
            (a.min(b), a.max(b))
        }
        None => {
            let ys = plot.traces.iter().flat_map(|t| t.y.iter().copied());
            let (mut a, mut b) = range(ys).unwrap_or((0.0, 1.0));
            if plot.traces.iter().any(|t| t.sticks) {
                a = a.min(0.0);
                b = b.max(0.0);
            }
            let pad = (b - a) * 0.05;
            (if a == 0.0 { 0.0 } else { a - pad }, b + pad)
        }
    };
    let marks = plot
        .peaks
        .iter()
        .map(|p| ChartMark {
            x: p.x,
            y: p.y,
            label: p.label.clone(),
        })
        .collect();
    Chart {
        frame,
        x: axis(&plot.x, x0, x1),
        y: axis(&plot.y, y0, y1),
        marks,
        legend: Vec::new(),
    }
}

/// A trace's paths: runs of [`RUN`] points (a line), or of sticks from zero.
fn trace_paths(chart: &Chart, t: &Trace) -> Vec<BezPath> {
    let n = t.x.len().min(t.y.len());
    let mut out = Vec::new();
    let zero = chart
        .to_sheet(0.0, 0.0)
        .y
        .clamp(chart.frame.y0, chart.frame.y1);
    let mut start = 0;
    while start < n {
        // Runs overlap by one point so the line has no gaps.
        let end = (start + RUN).min(n);
        let mut p = BezPath::new();
        let mut pen = false;
        for i in start..end {
            let (x, y) = (t.x[i], t.y[i]);
            if !(x.is_finite() && y.is_finite()) {
                pen = false;
                continue;
            }
            let q = chart.to_sheet(x, y);
            if t.sticks {
                p.move_to(Point::new(q.x, zero));
                p.line_to(q);
            } else if pen {
                p.line_to(q);
            } else {
                p.move_to(q);
                pen = true;
            }
        }
        if !p.elements().is_empty() {
            out.push(p);
        }
        start = if t.sticks || end == n { end } else { end - 1 };
    }
    out
}

/// Positive contour levels of `z`: from a few times its noise (else 2 % of its peak) up by
/// [`LEVEL_STEP`], at most [`LEVELS`]; `None` for a flat grid.
pub fn contour_levels(z: &[f32]) -> Option<Vec<f64>> {
    let max = z
        .iter()
        .filter(|v| v.is_finite())
        .fold(0f64, |m, &v| m.max(f64::from(v).abs()));
    if max <= 0.0 {
        return None;
    }
    // Noise: the median absolute value of a sample (most of a 2-D spectrum is noise).
    let step = (z.len() / 65_536).max(1);
    let mut s: Vec<f64> = z
        .iter()
        .step_by(step)
        .filter(|v| v.is_finite())
        .map(|&v| f64::from(v).abs())
        .collect();
    let mid = s.len() / 2;
    let noise = if s.is_empty() {
        0.0
    } else {
        *s.select_nth_unstable_by(mid, f64::total_cmp).1 / 0.6745
    };
    let base = (noise * 6.0).max(max * 0.02).min(max * 0.5);
    let mut levels = Vec::new();
    let mut l = base;
    while l < max && levels.len() < LEVELS {
        levels.push(l);
        l *= LEVEL_STEP;
    }
    Some(levels)
}

/// Marching squares at `level`: the segments where the grid crosses it, in sheet
/// coordinates, a path per run of rows.
fn contour_paths(chart: &Chart, g: &Grid, level: f64) -> Vec<BezPath> {
    let (w, h) = (g.width, g.height);
    if w < 2 || h < 2 || g.z.len() < w * h {
        return Vec::new();
    }
    let xv = |c: f64| g.x_range.0 + (g.x_range.1 - g.x_range.0) * c / (w - 1) as f64;
    let yv = |r: f64| g.y_range.0 + (g.y_range.1 - g.y_range.0) * r / (h - 1) as f64;
    let at = |c: usize, r: usize| f64::from(g.z[r * w + c]);
    let mut out = Vec::new();
    let mut p = BezPath::new();
    let mut segs = 0usize;
    for r in 0..h - 1 {
        for c in 0..w - 1 {
            let v = [at(c, r), at(c + 1, r), at(c + 1, r + 1), at(c, r + 1)];
            let mut case = 0u8;
            for (k, &x) in v.iter().enumerate() {
                if x >= level {
                    case |= 1 << k;
                }
            }
            if case == 0 || case == 15 {
                continue;
            }
            // Crossing points on the cell's edges: 0 bottom, 1 right, 2 top, 3 left.
            let edge = |e: usize| -> Point {
                let (a, b, (ca, ra), (cb, rb)) = match e {
                    0 => (v[0], v[1], (c, r), (c + 1, r)),
                    1 => (v[1], v[2], (c + 1, r), (c + 1, r + 1)),
                    2 => (v[3], v[2], (c, r + 1), (c + 1, r + 1)),
                    _ => (v[0], v[3], (c, r), (c, r + 1)),
                };
                let t = if b == a { 0.5 } else { (level - a) / (b - a) };
                let cc = ca as f64 + (cb as f64 - ca as f64) * t;
                let rr = ra as f64 + (rb as f64 - ra as f64) * t;
                chart.to_sheet(xv(cc), yv(rr))
            };
            let pairs: &[(usize, usize)] = match case {
                1 | 14 => &[(3, 0)],
                2 | 13 => &[(0, 1)],
                3 | 12 => &[(3, 1)],
                4 | 11 => &[(1, 2)],
                6 | 9 => &[(0, 2)],
                7 | 8 => &[(3, 2)],
                5 => &[(3, 2), (0, 1)],
                10 => &[(3, 0), (1, 2)],
                _ => &[],
            };
            for &(a, b) in pairs {
                p.move_to(edge(a));
                p.line_to(edge(b));
                segs += 1;
            }
        }
        if segs >= RUN {
            out.push(std::mem::take(&mut p));
            segs = 0;
        }
    }
    if segs > 0 {
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Facts, Peak, Spectra};

    fn ir() -> Plot {
        let x: Vec<f64> = (0..10_000).map(|i| 400.0 + i as f64 * 0.36).collect();
        let y = x
            .iter()
            .map(|&v| 1.0 - (-((v - 1700.0) / 8.0).powi(2)).exp())
            .collect();
        Plot {
            title: "Spectrum".into(),
            x: Axis::new("Wavenumber", "cm⁻¹", true),
            y: Axis::new("Transmittance", "", false),
            traces: vec![Trace {
                x,
                y,
                ..Trace::default()
            }],
            peaks: vec![Peak {
                x: 1700.0,
                y: 0.0,
                label: "1700".into(),
            }],
            ..Plot::default()
        }
    }

    #[test]
    fn a_reversed_axis_maps_both_ways() {
        let c = chart_of(&ir(), Rect::new(0.0, 0.0, FRAME_W, FRAME_H));
        // Wavenumbers run right to left: the largest is at the left edge.
        assert!(c.x.start > c.x.end);
        let p = c.to_sheet(1700.0, 0.5);
        let (x, y) = c.to_values(p);
        assert!((x - 1700.0).abs() < 1e-9 && (y - 0.5).abs() < 1e-9);
        assert!(c.to_sheet(c.x.start, 0.0).x.abs() < 1e-9);
    }

    /// A stick spectrum's first and last peaks stand inside the frame, not on its sides
    /// (a two-peak GC-MS page drew nothing visible: the visual review sheets).
    #[test]
    fn sticks_keep_clear_of_the_frame() {
        let plot = Plot {
            title: "MS".into(),
            x: Axis::new("m/z", "", false),
            y: Axis::new("Abundance", "", false),
            traces: vec![Trace {
                x: vec![28.0, 32.0],
                y: vec![100.0, 25.0],
                sticks: true,
                ..Trace::default()
            }],
            ..Plot::default()
        };
        let frame = Rect::new(0.0, 0.0, FRAME_W, FRAME_H);
        let c = chart_of(&plot, frame);
        for x in [28.0, 32.0] {
            let sx = c.to_sheet(x, 0.0).x;
            assert!(
                sx > frame.x0 + FRAME_W * 0.03 && sx < frame.x1 - FRAME_W * 0.03,
                "{sx}"
            );
        }
        // A single stick too.
        let mut one = plot.clone();
        one.traces[0].x.truncate(1);
        one.traces[0].y.truncate(1);
        let c = chart_of(&one, frame);
        let sx = c.to_sheet(28.0, 0.0).x;
        assert!((sx - FRAME_W / 2.0).abs() < 1e-6, "{sx}");
        // Lines still span the frame edge to edge.
        let c = chart_of(&ir(), frame);
        assert!(c.to_sheet(c.x.start, 0.0).x.abs() < 1e-9);
    }

    #[test]
    fn long_traces_are_cut_into_runs_without_gaps() {
        let s = Spectra {
            facts: Facts {
                format: "Test".into(),
                ..Facts::default()
            },
            pages: vec![ir()],
        };
        let scene = scene(&s, 0..1);
        let sheet = &scene.sheets[0];
        let chart = sheet.chart.as_ref().expect("a chart");
        assert_eq!(chart.marks.len(), 1);
        // The frame, then 10 000 points in runs of 4096 overlapping by one.
        assert_eq!(sheet.items.len(), 1 + 3);
        let points: usize = sheet.items[1..]
            .iter()
            .map(|it| match &it.shape {
                Shape::Stroke { path, .. } => path.elements().len(),
                _ => 0,
            })
            .sum();
        assert_eq!(points, 10_000 + 2);
        assert_eq!(scene.layers.len(), 2);
        assert_eq!(scene.layers[1].generated, Some(Generated::Trace(1)));
    }

    /// Contour geometry against scikit-image's marching squares (`measure.find_contours`):
    /// the total length, in grid cells, at twelve levels of the oracle's choosing on seeded
    /// synthetic 2-D spectra (tools/corpus/fetch-oracle4.sh spectra). Reads
    /// `SPECULAR_ORACLE4_CORPUS`/spectra, else `$CARGO_TARGET_DIR/corpus/oracle4/spectra`.
    #[test]
    #[ignore]
    fn contours_match_scikit_image() {
        let Some(dir) = std::env::var_os("SPECULAR_ORACLE4_CORPUS")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("CARGO_TARGET_DIR")
                    .map(|t| std::path::PathBuf::from(t).join("corpus/oracle4"))
            })
            .map(|b| b.join("spectra"))
            .filter(|d| d.join("expect.tsv").is_file())
        else {
            eprintln!("oracle4 spectra corpus missing: run tools/corpus/fetch-oracle4.sh spectra");
            return;
        };
        let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
        let mut per: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for line in tsv.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let (w, h): (usize, usize) = (f[1].parse().unwrap(), f[2].parse().unwrap());
            let (level, want): (f64, f64) = (f[3].parse().unwrap(), f[4].parse().unwrap());
            let raw = std::fs::read(dir.join("files").join(f[0])).unwrap();
            let z: Vec<f32> = raw
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            let plot = Plot {
                x: Axis::new("F2", "ppm", true),
                y: Axis::new("F1", "ppm", true),
                grid: Some(Grid {
                    width: w,
                    height: h,
                    z,
                    x_range: (12.5, -0.5),
                    y_range: (180.0, 5.0),
                }),
                ..Plot::default()
            };
            let g = plot.grid.as_ref().unwrap();
            let c = chart_of(&plot, Rect::new(0.0, 0.0, FRAME_W, FRAME_H));
            // Back from sheet points to grid cells.
            let cell = |p: Point| {
                let (x, y) = c.to_values(p);
                (
                    (x - g.x_range.0) / (g.x_range.1 - g.x_range.0) * (w - 1) as f64,
                    (y - g.y_range.0) / (g.y_range.1 - g.y_range.0) * (h - 1) as f64,
                )
            };
            let mut got = 0.0;
            for path in contour_paths(&c, g, level) {
                let mut last = None;
                for e in path.elements() {
                    match *e {
                        specular_vector::kurbo::PathEl::MoveTo(p) => last = Some(cell(p)),
                        specular_vector::kurbo::PathEl::LineTo(p) => {
                            let q = cell(p);
                            if let Some(a) = last {
                                got += f64::hypot(q.0 - a.0, q.1 - a.1);
                            }
                            last = Some(q);
                        }
                        _ => {}
                    }
                }
            }
            let errs = per.entry(f[0].to_owned()).or_default();
            if (got - want).abs() > 1e-6 * want.max(1.0) {
                errs.push(format!(
                    "level {level}: length {got} vs scikit-image {want}"
                ));
            }
        }
        let mut failures = Vec::new();
        for (name, errs) in &per {
            let ok = if errs.is_empty() { "pass" } else { "fail" };
            println!("ORACLE\tSpectra/contours\t{name}\t{ok}");
            failures.extend(errs.iter().map(|e| format!("{name}: {e}")));
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn contours_ring_a_peak() {
        let (w, h) = (64, 48);
        let z: Vec<f32> = (0..w * h)
            .map(|i| {
                let (c, r) = ((i % w) as f32 - 32.0, (i / w) as f32 - 24.0);
                1000.0 * (-(c * c + r * r) / 30.0).exp()
            })
            .collect();
        let levels = contour_levels(&z).expect("levels");
        assert!(levels.len() >= 5, "{levels:?}");
        let plot = Plot {
            x: Axis::new("F2", "ppm", true),
            y: Axis::new("F1", "ppm", true),
            grid: Some(Grid {
                width: w,
                height: h,
                z,
                x_range: (10.0, 0.0),
                y_range: (200.0, 0.0),
            }),
            ..Plot::default()
        };
        let c = chart_of(&plot, Rect::new(0.0, 0.0, FRAME_W, FRAME_H));
        let paths = contour_paths(&c, plot.grid.as_ref().unwrap(), levels[0]);
        let pts: Vec<Point> = paths
            .iter()
            .flat_map(|p| p.elements().iter().filter_map(|e| e.end_point()))
            .collect();
        assert!(pts.len() > 20);
        // The ring is centred on the peak (5 ppm, 100 ppm-ish in the middle of the grid).
        let cx = pts.iter().map(|p| p.x).sum::<f64>() / pts.len() as f64;
        let (x, _) = c.to_values(Point::new(cx, 0.0));
        let mid = 10.0 - 10.0 * 32.0 / 63.0;
        assert!((x - mid).abs() < 0.1, "{x} vs {mid}");
    }
}
