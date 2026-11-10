//! What the viewer needs to draw a drawing page itself ([`crate::Session::drawing`]).

use std::sync::Arc;

pub use crate::pcb_dec::name_board_views;
pub use specular_cad;
pub use specular_gds;
pub use specular_jcamp;
pub use specular_kicad;
pub use specular_landxml;
pub use specular_measure;
use specular_measure::calib::{Calibration, Source, Unit};
use specular_measure::scale::NumberFormat;
pub use specular_nmr;
pub use specular_pcb;
pub use specular_spectra;
pub use specular_vector;
use specular_vector::draw::fit_transform;
use specular_vector::kurbo::{Affine, Rect};
use specular_vector::prep::{Prepared, PreparedSheet};
use specular_vector::{SheetKind, SheetUnits};

/// The paper behind a drawing on a light background (white, like plotted paper).
pub const LIGHT_PAPER: [u8; 3] = [255, 255, 255];
/// The paper behind a drawing on a dark background (CAD model-space dark, slightly blue).
pub const DARK_PAPER: [u8; 3] = [33, 40, 48];

/// The long side of a sheet's stored picture, pixels.
pub const LONG_SIDE: f64 = 4096.0;
/// Hairline width (the vector renderer's `dpi_scale`) for a CPU raster of a drawing whose
/// long side is `canvas_long` pixels: as if the picture were shown 1024 pixels across, and at
/// least 1.5 pixels. A 1-pixel line on the 4096-pixel stored picture nearly vanished once
/// fitted, exported or thumbnailed (GPX tracks, spectra; the visual review).
pub fn raster_line_scale(canvas_long: f64) -> f64 {
    (canvas_long / 1024.0).max(1.5)
}

/// The short side's minimum (a very long drawing stays visible across).
const MIN_SIDE: f64 = 64.0;
/// Pixels between the drawing and the edge of the picture.
const MARGIN: f64 = 24.0;

/// The paper colour for a background.
pub fn paper_colour(dark: bool) -> [u8; 3] {
    if dark { DARK_PAPER } else { LIGHT_PAPER }
}

/// One sheet of a drawing as a session shows it.
#[derive(Clone, Debug)]
pub struct DrawingPage {
    /// The whole drawing, shared by every page of the file.
    pub prepared: Arc<Prepared>,
    /// Which of [`Prepared::sheets`] this page is.
    pub sheet: usize,
    /// Sheet (drawing units, y up) to the session's stored pixels (y down).
    pub to_stored: Affine,
    /// Parsing and preparing took this long, milliseconds (0 for pages opened after it).
    pub open_ms: f64,
}

impl DrawingPage {
    /// Sheet `sheet` of `prepared` as a session shows it, with the size of its stored
    /// picture: what it draws fitted into [`LONG_SIDE`] on the long side.
    pub fn new(prepared: Arc<Prepared>, sheet: usize, open_ms: f64) -> ((u32, u32), DrawingPage) {
        let fit = prepared
            .sheets
            .get(sheet)
            .and_then(|s| s.fit)
            .filter(|r| r.width().is_finite() && r.height().is_finite())
            .unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0));
        let (fw, fh) = (fit.width().max(1e-9), fit.height().max(1e-9));
        let inner = LONG_SIDE - 2.0 * MARGIN;
        let k = inner / fw.max(fh);
        let w = (fw * k + 2.0 * MARGIN).max(MIN_SIDE).round();
        let h = (fh * k + 2.0 * MARGIN).max(MIN_SIDE).round();
        let (w, h) = (w as u32, h as u32);
        let to_stored = fit_transform(fit, (w, h), MARGIN);
        let page = DrawingPage {
            prepared,
            sheet,
            to_stored,
            open_ms,
        };
        ((w, h), page)
    }

    pub fn sheet(&self) -> Option<&Arc<PreparedSheet>> {
        self.prepared.sheets.get(self.sheet)
    }

    /// A CAD sheet's scale: model space in its own units; a layout's viewports in model units
    /// at their scale, and the paper outside them in paper units.
    pub fn drawing_scale(&self) -> Option<DrawingScale> {
        let sheet = self.prepared.scene.sheets.get(self.sheet)?;
        let su = sheet.units?;
        let px_per_unit = linear_scale(&self.to_stored);
        if !(px_per_unit.is_finite() && px_per_unit > 0.0) {
            return None;
        }
        let (unit, k) = unit_of_code(su.code)?;
        let formats = |u: Unit| specular_measure::scale::cad_formats(su.lunits, su.luprec, u);
        let per = k / px_per_unit;
        let sheet_cal = Calibration::spacing(per, per, unit, Source::DrawingUnits).ok()?;
        let unit_mm = SheetUnits::metres(su.code)? * 1e3;
        // Paper millimetres per sheet unit: a layout's plot scale, else the unit itself.
        let paper_mm = su
            .paper_mm
            .filter(|m| m.is_finite() && *m > 0.0)
            .unwrap_or(unit_mm);
        let mut regions = Vec::new();
        let mut main: Option<(f64, f64, Unit)> = None;
        if sheet.kind == SheetKind::Layout
            && let Some((model_unit, mk)) = unit_of_code(su.model)
        {
            let model_mm = SheetUnits::metres(su.model).unwrap_or(1.0) * 1e3;
            for (i, vp) in sheet.viewports.iter().enumerate() {
                // Paper units per model unit.
                let s = linear_scale(&vp.transform);
                if !(s.is_finite() && s > 0.0) {
                    continue;
                }
                let r = vp.paper;
                let bounds = stored_box(self.to_stored, [r.x0, r.y0, r.x1, r.y1]);
                let per = mk / (px_per_unit * s);
                let Ok(cal) = Calibration::spacing(per, per, model_unit, Source::DrawingScale)
                else {
                    continue;
                };
                let n = model_mm / (paper_mm * s);
                let area = (bounds[2] - bounds[0]) * (bounds[3] - bounds[1]);
                if main.is_none_or(|(a, ..)| area > a) {
                    main = Some((area, n, model_unit));
                }
                regions.push(ScaleRegion {
                    bounds: Some(bounds),
                    viewport: Some(i + 1),
                    name: None,
                    ratio: Some(specular_measure::scale::ratio_text(n)),
                    cal,
                    distance: formats(model_unit),
                    area: Vec::new(),
                });
            }
        }
        regions.push(ScaleRegion {
            bounds: None,
            viewport: None,
            name: None,
            ratio: None,
            cal: sheet_cal,
            distance: formats(unit),
            area: Vec::new(),
        });
        let mm_per_px = unit_mm / px_per_unit;
        let (ratio, unit, paper) = match (sheet.kind, main) {
            (SheetKind::Layout, Some((_, n, u))) => (n, u, paper_mm / px_per_unit),
            (SheetKind::Layout, None) => (unit_mm / paper_mm, unit, paper_mm / px_per_unit),
            // Model space prints as drawn at its dimension scale.
            _ => {
                let n = if su.dimscale.is_finite() && su.dimscale > 0.0 {
                    su.dimscale
                } else {
                    1.0
                };
                (n, unit, mm_per_px / n)
            }
        };
        Some(DrawingScale {
            regions,
            paper_mm_per_px: Some(paper),
            ratio,
            unit,
        })
    }
}

/// A region of a page with its own drawing scale: a viewport, or the whole sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleRegion {
    /// Its box in stored pixels (x0, y0, x1, y1); `None` for the whole page.
    pub bounds: Option<[f64; 4]>,
    /// The viewport's number on the sheet (1-based), for viewports.
    pub viewport: Option<usize>,
    /// The viewport's name in the file.
    pub name: Option<String>,
    /// The scale as shown: `1:50`, or the file's own text (`1 in = 4 ft`).
    pub ratio: Option<String>,
    /// Stored pixels to the drawing's units.
    pub cal: Calibration,
    /// How the file writes distances (values in `cal.unit`) and areas (in `cal.unit`²);
    /// empty for the usual style.
    pub distance: Vec<NumberFormat>,
    pub area: Vec<NumberFormat>,
}

impl ScaleRegion {
    /// Whether stored point `(x, y)` is inside (the whole page always is).
    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.bounds
            .is_none_or(|[x0, y0, x1, y1]| x >= x0 && x <= x1 && y >= y0 && y <= y1)
    }

    fn area_px(&self) -> f64 {
        self.bounds
            .map_or(f64::INFINITY, |[x0, y0, x1, y1]| (x1 - x0) * (y1 - y0))
    }
}

/// A page's drawing scale.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingScale {
    /// Viewports first (in file order), then the whole sheet when it has its own units.
    pub regions: Vec<ScaleRegion>,
    /// Millimetres of paper per stored pixel when the sheet prints as drawn.
    pub paper_mm_per_px: Option<f64>,
    /// Real size over paper size of the main (largest) viewport; 1 when the sheet has none.
    pub ratio: f64,
    /// The main viewport's unit (a scale bar's unit).
    pub unit: Unit,
}

impl DrawingScale {
    /// The region a measurement at stored point `(x, y)` goes through: the smallest viewport
    /// containing it, else the whole sheet.
    pub fn region_at(&self, x: f64, y: f64) -> Option<usize> {
        self.regions
            .iter()
            .enumerate()
            .filter(|(_, r)| r.contains(x, y))
            .min_by(|a, b| a.1.area_px().total_cmp(&b.1.area_px()))
            .map(|(i, _)| i)
    }
}

/// The measure unit of an `$INSUNITS` code and how many of it make one unit of the code.
fn unit_of_code(code: i16) -> Option<(Unit, f64)> {
    Some(match code {
        1 => (Unit::Inch, 1.0),
        2 => (Unit::Ft, 1.0),
        3 => (Unit::Mi, 1.0),
        4 => (Unit::Mm, 1.0),
        5 => (Unit::Cm, 1.0),
        6 => (Unit::M, 1.0),
        7 => (Unit::Km, 1.0),
        9 => (Unit::Inch, 1e-3),
        10 => (Unit::Yd, 1.0),
        12 => (Unit::Nm, 1.0),
        13 => (Unit::Um, 1.0),
        14 => (Unit::Cm, 10.0),
        _ => return None,
    })
}

/// The linear scale of an affine map (the square root of its determinant's size).
fn linear_scale(a: &Affine) -> f64 {
    let c = a.as_coeffs();
    (c[0] * c[3] - c[1] * c[2]).abs().sqrt()
}

/// The bounding box (x0, y0, x1, y1) of the box `b` mapped through `t`.
fn stored_box(t: Affine, b: [f64; 4]) -> [f64; 4] {
    let [ux0, uy0, ux1, uy1] = b;
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for (x, y) in [(ux0, uy0), (ux1, uy1), (ux0, uy1), (ux1, uy0)] {
        let p = t * specular_vector::kurbo::Point::new(x, y);
        out = [
            out[0].min(p.x),
            out[1].min(p.y),
            out[2].max(p.x),
            out[3].max(p.y),
        ];
    }
    out
}

/// A PDF page's drawing scale from its rectilinear viewports. `stored_to_user` maps stored
/// pixels to user space (points); `px_per_pt` is the stored pixels per point.
pub(crate) fn pdf_scale(
    vps: &[specular_pdf::measure::ScaleViewport],
    stored_to_user: Affine,
    px_per_pt: f64,
) -> DrawingScale {
    use specular_measure::scale::{Fraction, ratio_text};
    let conv = |f: &specular_pdf::measure::NumFmt| {
        let d = NumberFormat::default();
        NumberFormat {
            unit: f.unit.clone(),
            factor: f.factor,
            fraction: match f.fraction {
                'F' => Fraction::Fraction,
                'R' => Fraction::Round,
                'T' => Fraction::Truncate,
                _ => Fraction::Decimal,
            },
            precision: f.precision,
            reduce: !f.no_reduce,
            thousands: f.thousands.clone().unwrap_or(d.thousands),
            decimal: f.decimal.clone().unwrap_or(d.decimal),
            before: f.before.clone().unwrap_or(d.before),
            after: f.after.clone().unwrap_or(d.after),
            prefix: f.prefix,
        }
    };
    let pt_mm = 25.4 / 72.0;
    let user_to_stored = stored_to_user.inverse();
    let c = stored_to_user.as_coeffs();
    let mut regions = Vec::new();
    let mut main: Option<(f64, f64, Unit)> = None;
    for vp in vps {
        let chain = if vp.distance.is_empty() {
            &vp.x
        } else {
            &vp.distance
        };
        let Some(first) = chain.first() else { continue };
        let Some(unit) = Unit::from_key(first.unit.trim()) else {
            continue;
        };
        if unit == Unit::Px || unit == Unit::Arcsec {
            continue;
        }
        let kx = first.factor;
        // Different x and y units: y converted to x's largest unit by `/CYX`.
        let ky = match (vp.y.first(), vp.cyx, vp.x.first()) {
            (Some(y), Some(cyx), Some(x)) => kx * y.factor * cyx / x.factor,
            _ => kx,
        };
        // Stored displacement → user space → drawing units.
        let cal = Calibration {
            m: [[kx * c[0], kx * c[2]], [ky * c[1], ky * c[3]]],
            unit,
            source: Source::DrawingScale,
        };
        if !(cal.pixel_area().is_finite() && cal.pixel_area() > 0.0) {
            continue;
        }
        let mut distance: Vec<NumberFormat> = chain.iter().map(conv).collect();
        distance[0].factor = 1.0;
        let mut area: Vec<NumberFormat> = vp.area.iter().map(conv).collect();
        if let Some(a) = area.first_mut() {
            a.factor /= kx * ky;
        }
        let n = unit.metres().map(|m| kx * m * 1e3 / pt_mm);
        let bounds = stored_box(user_to_stored, vp.bbox);
        if let Some(n) = n {
            let a = (bounds[2] - bounds[0]) * (bounds[3] - bounds[1]);
            if main.is_none_or(|(b, ..)| a > b) {
                main = Some((a, n, unit));
            }
        }
        regions.push(ScaleRegion {
            bounds: Some(bounds),
            viewport: Some(vp.index + 1),
            name: vp.name.clone(),
            ratio: n.map(ratio_text).or_else(|| vp.ratio.clone()),
            cal,
            distance,
            area,
        });
    }
    let (_, ratio, unit) = main.unwrap_or((0.0, 1.0, Unit::Mm));
    DrawingScale {
        regions,
        paper_mm_per_px: (px_per_pt > 0.0).then(|| pt_mm / px_per_pt),
        ratio,
        unit,
    }
}
