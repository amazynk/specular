//! RT dose over MPR: an RTDOSE grid of the volume's frame of reference, found by the
//! analysis search, drawn as isodose lines at the standard percentages of the plan's
//! prescription (else of the maximum dose) with a light colour wash, or as a colour map
//! through the fusion renderer; its levels in the panel's legend (each can be hidden), the
//! dose under the pointer in the readout, and the cumulative DVH of every RTSTRUCT ROI on
//! the plot sheet (`Shift+K` in MPR) with D95, Dmean, Dmax and V20.

use std::path::PathBuf;

use specular_dicom::Dicom;
use specular_dicom::rtdose::{self, DoseGrid, Dvh, Prescription};
use specular_dicom::rtstruct::StructureSet;
use specular_dicom::volume::Volume;
use specular_gfx::{Gpu, wgpu};
use specular_i18n::{I18n, t};
use specular_render::{VoiShape, Window as WlWindow};
use specular_render_volume::{
    Colormap, FusionRenderer, IsodoseLevel, IsodoseRenderer, IsodoseStyle, Plane, VolumeTexture,
    reference,
};
use specular_widgets::{CaliperKind, PlotCell, PlotGrid, PlotSheet, PlotTrace, nice_ticks};

use super::analysis::BLENDS;

/// Most plans and structure sets read for one dose.
const MAX_PLANS: usize = 16;
/// The colour wash between isodose lines.
const WASH: f32 = 0.18;
/// The DVH table's V-dose (Gy).
pub(crate) const V_GY: f64 = 20.0;

/// An RT dose of the volume's frame of reference, read on the search's thread.
pub(crate) struct DoseFound {
    pub volume: Volume,
    pub max: f64,
    /// GY or RELATIVE.
    pub units: String,
    pub prescription: Option<Prescription>,
    /// Each contoured ROI's DVH: name, colour, histogram.
    pub dvhs: Vec<(String, [u8; 3], Dvh)>,
}

impl DoseFound {
    /// The dose the isodose percentages are of: the prescription, else the maximum.
    pub(crate) fn reference(&self) -> f64 {
        self.prescription
            .as_ref()
            .map(|p| p.dose)
            .filter(|d| *d > 0.0 && self.gray())
            .unwrap_or(self.max)
    }

    fn gray(&self) -> bool {
        self.units.eq_ignore_ascii_case("GY")
    }

    /// The dose unit's symbol.
    pub(crate) fn unit(&self) -> &'static str {
        if self.gray() { "Gy" } else { "%" }
    }
}

fn parse(p: &PathBuf) -> Option<(Vec<u8>, Dicom)> {
    let f = std::fs::read(p).ok()?;
    let d = Dicom::parse(&f).ok()?;
    Some((f, d))
}

/// Reads the dose of the grids at `doses` (a plan's sum before a beam's or fraction's), its
/// plan's prescription among `plans`, and the DVHs of the structure sets at `structs` that
/// share its frame of reference.
pub(crate) fn read(doses: &[PathBuf], plans: &[PathBuf], structs: &[PathBuf]) -> Option<DoseFound> {
    let mut grids: Vec<DoseGrid> = doses
        .iter()
        .filter_map(|p| {
            let (f, d) = parse(p)?;
            DoseGrid::read(&d, &f).ok()
        })
        .collect();
    grids.sort_by_key(|g| g.summation != "PLAN");
    let grid = grids.into_iter().next()?;
    let volume = grid.volume().ok()?;
    let prescriptions: Vec<Prescription> = plans
        .iter()
        .take(MAX_PLANS)
        .filter_map(|p| {
            let (f, d) = parse(p)?;
            Prescription::read(&d, &f)
        })
        .collect();
    let prescription = prescriptions
        .iter()
        .find(|p| grid.plan.as_deref() == Some(p.sop_instance.as_str()))
        .or(prescriptions.first())
        .cloned();
    let mut dvhs = Vec::new();
    for p in structs.iter().take(MAX_PLANS) {
        let Some((f, d)) = parse(p) else { continue };
        let Ok(set) = StructureSet::read(&d, &f) else {
            continue;
        };
        if set.frame_of_reference != grid.frame_of_reference {
            continue;
        }
        for (k, dvh) in rtdose::dvhs(&grid, &set) {
            if !dvh.is_empty() {
                let s = &set.rois[k].segment;
                dvhs.push((s.label.clone(), s.colour, dvh));
            }
        }
    }
    Some(DoseFound {
        max: grid.max_dose(),
        units: grid.units.clone(),
        volume,
        prescription,
        dvhs,
    })
}

/// How the dose is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DoseMode {
    /// Isodose lines over a light wash.
    Lines,
    /// A colour map (the fusion renderer) under the lines.
    Colour,
    Off,
}

/// The dose over MPR.
pub(crate) struct Dose {
    pub found: DoseFound,
    pub tex: Option<VolumeTexture>,
    pub mode: DoseMode,
    /// Levels hidden from the legend (by index in [`rtdose::ISODOSE_LEVELS`]).
    pub hidden: Vec<bool>,
    pub colormap: Colormap,
    pub blend: usize,
    iso_r: IsodoseRenderer,
    colour_r: FusionRenderer,
}

/// The colour of a level as sRGB.
fn srgb(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

impl Dose {
    pub(crate) fn new(gpu: &Gpu, found: DoseFound) -> Result<Dose, String> {
        let mut tex = VolumeTexture::new(gpu, &found.volume)?;
        tex.sync(gpu, &found.volume)?;
        Ok(Dose {
            found,
            tex: Some(tex),
            mode: DoseMode::Lines,
            hidden: vec![false; rtdose::ISODOSE_LEVELS.len()],
            colormap: Colormap::Rainbow,
            blend: 0,
            iso_r: IsodoseRenderer::new(),
            colour_r: FusionRenderer::new(),
        })
    }

    /// The GPU device was replaced: the dose and its renderers are made again on `gpu`.
    pub(crate) fn gpu_reset(&mut self, gpu: &Gpu) -> Result<(), String> {
        self.iso_r = IsodoseRenderer::new();
        self.colour_r = FusionRenderer::new();
        self.tex = None;
        let mut tex = VolumeTexture::new(gpu, &self.found.volume)?;
        tex.sync(gpu, &self.found.volume)?;
        self.tex = Some(tex);
        Ok(())
    }

    /// The isodose levels drawn.
    pub(crate) fn style(&self) -> IsodoseStyle {
        let levels = rtdose::isodose_lines(self.found.reference())
            .into_iter()
            .zip(&self.hidden)
            .filter(|(_, h)| !**h)
            .map(|((dose, _, c), _)| IsodoseLevel {
                dose,
                srgb: srgb(c),
            })
            .collect();
        IsodoseStyle {
            levels,
            width: 1.5,
            wash: if self.mode == DoseMode::Lines {
                WASH
            } else {
                0.0
            },
        }
    }

    /// The dose's planes for `planes`: thin, windowed from `low` to the maximum dose (the
    /// colour map leaves doses at or under the window's lower edge clear).
    fn planes_from(&self, planes: &[Plane], low: f64) -> Vec<Plane> {
        let top = self.found.max.max(1e-6);
        let low = low.clamp(0.0, top * 0.999);
        planes
            .iter()
            .map(|p| Plane {
                thickness: 0.0,
                window: WlWindow {
                    center: (low + top) / 2.0,
                    width: top - low,
                    shape: VoiShape::Linear,
                    invert: false,
                },
                ..*p
            })
            .collect()
    }

    /// The dose's planes over the whole dose range (the lines and the readout).
    fn planes(&self, planes: &[Plane]) -> Vec<Plane> {
        self.planes_from(planes, 0.0)
    }

    /// Draws the dose over `planes` (after the volume and any fused series).
    pub(crate) fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        planes: &[Plane],
        scale: f64,
    ) {
        if self.mode == DoseMode::Off {
            return;
        }
        let Some(tex) = &self.tex else { return };
        let planes = self.planes(planes);
        if self.mode == DoseMode::Colour {
            // The colour map starts at the lowest isodose level: lower doses stay clear.
            let low = rtdose::ISODOSE_LEVELS
                .iter()
                .map(|l| l.0)
                .fold(f64::INFINITY, f64::min);
            let washed = self.planes_from(&planes, self.found.reference() * low / 100.0);
            let blend = BLENDS[self.blend % BLENDS.len()];
            self.colour_r
                .draw(gpu, enc, target, size, tex, self.colormap, blend, &washed);
        }
        let mut style = self.style();
        style.width = (1.5 * scale) as f32;
        self.iso_r
            .draw(gpu, enc, target, size, tex, &style, &planes);
    }

    /// The dose at target point `at` of `plane`, for the readout.
    pub(crate) fn readout(&self, plane: &Plane, at: (f64, f64)) -> Option<String> {
        if self.mode == DoseMode::Off {
            return None;
        }
        let p = self.planes(std::slice::from_ref(plane))[0];
        let v = f64::from(reference(&self.found.volume, &p, at.0, at.1)?);
        let r = self.found.reference();
        Some(t!(
            "dose-value",
            value = specular_i18n::fixed(v, 2),
            unit = self.found.unit(),
            percent = specular_i18n::fixed(if r > 0.0 { 100.0 * v / r } else { 0.0 }, 0)
        ))
    }

    /// The panel's lines.
    pub(crate) fn panel(&self) -> Vec<String> {
        let mut out = Vec::new();
        let max = specular_i18n::fixed(self.found.max, 2);
        out.push(match (&self.found.prescription, self.mode) {
            (_, DoseMode::Off) => t!("dose-hidden"),
            (Some(p), _) if self.found.gray() => t!(
                "dose-on-rx",
                dose = specular_i18n::fixed(p.dose, 2),
                max = max,
                unit = self.found.unit()
            ),
            _ => t!("dose-on-max", max = max, unit = self.found.unit()),
        });
        if self.mode == DoseMode::Colour {
            out.push(t!(
                "dose-colour",
                colormap = I18n::global().tr(&format!("colormap-{}", self.colormap.id())),
                blend = super::super::medical::number(
                    (BLENDS[self.blend % BLENDS.len()] * 100.0).round().into()
                )
            ));
        }
        if !self.found.dvhs.is_empty() {
            out.push(t!("dose-dvh-hint", count = self.found.dvhs.len() as i64));
        }
        out
    }

    /// The legend: each isodose level's name and colour, shown or hidden.
    pub(crate) fn legend(&self) -> Vec<(usize, String, [u8; 3], bool)> {
        rtdose::isodose_lines(self.found.reference())
            .into_iter()
            .enumerate()
            .map(|(k, (dose, percent, c))| {
                (
                    k,
                    t!(
                        "dose-level",
                        percent = specular_i18n::fixed(percent, 0),
                        dose = specular_i18n::fixed(dose, 1),
                        unit = self.found.unit()
                    ),
                    srgb(c),
                    self.mode != DoseMode::Off && !self.hidden[k],
                )
            })
            .collect()
    }

    /// Shows or hides isodose level `k`.
    pub(crate) fn toggle(&mut self, k: usize) {
        if let Some(h) = self.hidden.get_mut(k) {
            *h = !*h;
        }
    }

    /// `Shift+U` without a series to fuse: lines, colour, off.
    pub(crate) fn cycle(&mut self) -> String {
        self.mode = match self.mode {
            DoseMode::Lines => DoseMode::Colour,
            DoseMode::Colour => DoseMode::Off,
            DoseMode::Off => DoseMode::Lines,
        };
        match self.mode {
            DoseMode::Lines => t!("dose-mode-lines"),
            DoseMode::Colour => t!("dose-mode-colour"),
            DoseMode::Off => t!("dose-hidden"),
        }
    }
}

/// The DVH plot sheet: each ROI's cumulative DVH (volume % against dose), the legend, and a
/// table of volume, D95, Dmean, Dmax and V20.
pub(crate) fn dvh_sheet(found: &DoseFound) -> Option<PlotSheet> {
    if found.dvhs.is_empty() {
        return None;
    }
    let unit = found.unit();
    let top = found
        .dvhs
        .iter()
        .map(|(_, _, d)| d.max())
        .fold(found.reference() * 1.1, f64::max)
        .max(1e-3);
    let x_ticks_v = nice_ticks(0.0, top, 7);
    let top = x_ticks_v
        .last()
        .copied()
        .filter(|t| *t >= top)
        .unwrap_or(top);
    // Page: the dose across 160 units, 0–100 % up 100.
    let (x0, y0, w, h) = (14.0, 4.0, 160.0, 100.0);
    let per = w / top;
    let traces = found
        .dvhs
        .iter()
        .map(|(name, colour, d)| {
            let c = d.cumulative();
            let total = c.first().copied().unwrap_or(0.0).max(1e-12);
            let n = c.len();
            // At most ~600 points: the curve is smooth at the page's resolution.
            let step = n.div_ceil(600).max(1);
            let mut points: Vec<[f32; 2]> = (0..n)
                .step_by(step)
                .map(|i| {
                    let dose = i as f64 * rtdose::BIN;
                    [
                        (x0 + dose * per) as f32,
                        (y0 + h - 100.0 * c[i] / total) as f32,
                    ]
                })
                .collect();
            points.push([(x0 + n as f64 * rtdose::BIN * per) as f32, (y0 + h) as f32]);
            PlotTrace {
                points,
                srgb: *colour,
                name: name.clone(),
            }
        })
        .collect();
    let x_ticks = x_ticks_v
        .into_iter()
        .filter(|g| *g <= top + 1e-9)
        .map(|g| (x0 + g * per, trim(g)))
        .collect();
    let y_ticks = nice_ticks(0.0, 100.0, 5)
        .into_iter()
        .map(|p| (y0 + h - p, trim(p)))
        .collect();
    let mut table = vec![vec![
        t!("dvh-col-roi"),
        t!("dvh-col-volume"),
        "D95".to_owned(),
        "Dmean".to_owned(),
        "Dmax".to_owned(),
        format!("V{}", trim(V_GY)),
    ]];
    for (name, _, d) in &found.dvhs {
        let total = d.cumulative().first().copied().unwrap_or(0.0);
        let v20 = if total > 0.0 {
            100.0 * d.volume_at(V_GY) / total
        } else {
            0.0
        };
        table.push(vec![
            name.clone(),
            format!("{} cm³", specular_i18n::fixed(d.volume, 1)),
            format!("{} {unit}", specular_i18n::fixed(d.dose_at(95.0), 2)),
            format!("{} {unit}", specular_i18n::fixed(d.mean(), 2)),
            format!("{} {unit}", specular_i18n::fixed(d.max(), 2)),
            specular_i18n::I18n::global()
                .localizer()
                .percent(v20 / 100.0, 1),
        ]);
    }
    Some(PlotSheet {
        title: t!("dvh-title"),
        page: [x0 + w + 4.0, y0 + h + 12.0],
        grid: PlotGrid::Ticks,
        cells: vec![PlotCell {
            rect: [x0, y0, w, h],
            traces,
            x_ticks,
            y_ticks,
            x_title: t!("dvh-axis-dose", unit = unit),
            y_title: t!("dvh-axis-volume"),
            x_value: (0.0, 1.0 / per),
            y_value: (0.0, 1.0),
            x_unit: unit.to_owned(),
            y_unit: "%".to_owned(),
            ..PlotCell::default()
        }],
        caliper: CaliperKind::Value,
        subtitle: match &found.prescription {
            Some(p) if found.gray() => {
                t!("dvh-prescription", dose = specular_i18n::fixed(p.dose, 2))
            }
            _ => t!(
                "dvh-max",
                dose = specular_i18n::fixed(found.max, 2),
                unit = unit
            ),
        },
        legend: found.dvhs.iter().map(|(n, c, _)| (n.clone(), *c)).collect(),
        table,
        ..PlotSheet::default()
    })
}

/// A tick's label: whole numbers without decimals.
fn trim(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        specular_i18n::fixed(v, 0)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

    use super::*;

    fn study() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/formats/specular-dicom/tests/data/dicomrt/study")
    }

    /// The analysis search beside the fixture CT finds the plan's dose, its prescription and
    /// the structure set's DVHs; the DVH sheet's table reads them back.
    #[test]
    fn search_finds_the_dose_and_its_histograms() {
        let found =
            super::super::analysis::find(&study().join("ct_00.dcm"), &[], &AtomicBool::new(false));
        let d = found.dose.expect("the fixture study's dose");
        assert!(d.max > 0.0);
        let rx = d.prescription.as_ref().expect("the plan's prescription");
        assert!((d.reference() - rx.dose).abs() < 1e-9);
        assert!(!d.dvhs.is_empty());
        let sheet = dvh_sheet(&d).unwrap();
        assert_eq!(sheet.table.len(), d.dvhs.len() + 1);
        assert_eq!(sheet.cells[0].traces.len(), d.dvhs.len());
        for ((name, _, dvh), row) in d.dvhs.iter().zip(&sheet.table[1..]) {
            assert_eq!(&row[0], name);
            assert_eq!(
                row[2],
                format!(
                    "{} {}",
                    specular_i18n::fixed(dvh.dose_at(95.0), 2),
                    d.unit()
                )
            );
            assert_eq!(
                row[4],
                format!("{} {}", specular_i18n::fixed(dvh.max(), 2), d.unit())
            );
        }
        // The curves start at 100 % (the whole structure gets at least no dose).
        let c = &sheet.cells[0];
        for t in &c.traces {
            let y = c.y_value.0 + (c.rect[1] + c.rect[3] - f64::from(t.points[0][1])) * c.y_value.1;
            assert!((y - 100.0).abs() < 1e-3, "{}: {y}", t.name);
        }
        // Nine standard levels, of the prescription.
        let levels = rtdose::isodose_lines(d.reference());
        assert_eq!(levels.len(), 9);
        assert!(levels.iter().any(|l| (l.1 - 95.0).abs() < 1e-9));
    }
}
