//! Flow Cytometry Standard files (FCS 2.0–3.2) through `specular-fcs`: each dataset a page,
//! shown as a density plot on the science path.
//!
//! A plane is a [`BINS`]² image of event counts: the x parameter across, the y parameter up
//! (the highest values at the top), each through its axis scale. Its axes are the plot's
//! choices, stepped like any plane slider:
//!
//! * **X** and **Y**: the parameters (`$PnN`, with `$PnS`); Y's last position is
//!   **Histogram**, the x parameter's histogram drawn as bars (each column's count as its
//!   value).
//! * **X scale** and **Y scale**: Auto (linear for scatter and time, log for log-amplified
//!   parameters and those whose `$PnD` asks for it, logicle for the rest), Linear, Log,
//!   Logicle, Arcsinh.
//! * **Compensation** (when the dataset has a spillover matrix): applied or off.
//!
//! The plot opens on FSC against SSC (their `-A` parameters when there are). Counts go
//! through viridis; science mode shows them with a log stretch between the plane's least
//! and greatest count until another look is chosen. [`CytometryFacts`] gives the readout
//! (the parameter values under the pointer, the events in the bin) and the axis ticks.
//!
//! Events are decoded once per dataset (scale values: `$PnE`, gain, `$TIMESTEP`), and
//! compensated once when asked; a plane is one pass over them.

use std::sync::{Arc, OnceLock};

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_fcs::scale::{self, Axis, Scale, Stats};
use specular_fcs::{Dataset, Events, Fcs};

use crate::imzml_dec::viridis_lut;
use crate::science::{AxisKind, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// Bins per side of a plot.
pub const BINS: u32 = 512;
/// Keywords listed at most per dataset.
const MAX_KEYWORDS: usize = 8000;

pub struct FcsDecoder;

fn map_err(e: specular_fcs::Error) -> DecodeError {
    use specular_fcs::Error as E;
    match e {
        E::NotFcs => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
    }
}

impl Decoder for FcsDecoder {
    fn format(&self) -> Format {
        Format::Fcs
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_fcs::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["fcs", "lmd"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let fcs = specular_fcs::parse(source.bytes()).map_err(map_err)?;
        let sets = fcs.datasets.len().min(MAX_SETS);
        let shared = Arc::new(Shared {
            fcs,
            sets: (0..sets).map(|_| OnceLock::new()).collect(),
        });
        Ok(Arc::new(FcsSession::new(source, shared, 0)?))
    }
}

/// Datasets whose events are kept decoded at once (one per page opened).
const MAX_SETS: usize = specular_fcs::MAX_DATASETS;

struct Shared {
    fcs: Fcs,
    /// Per dataset: its events (raw scale values, compensated) and facts, made when its
    /// page opens.
    sets: Vec<OnceLock<Arc<SetData>>>,
}

struct SetData {
    events: Arc<Events>,
    compensated: Option<Arc<Events>>,
    facts: Arc<CytometryFacts>,
}

/// What science mode needs of a cytometry plot: the axes under each index, for the readout
/// and the ticks.
#[derive(Debug)]
pub struct CytometryFacts {
    /// `FCS3.1`.
    pub version: String,
    pub events: u64,
    /// The parameters' names as shown.
    pub names: Vec<String>,
    pub bins: u32,
    /// The keyword of the spillover matrix the compensation axis applies.
    pub compensation: Option<String>,
    /// A compensation keyword that is not applied, and why.
    pub compensation_note: Option<String>,
    /// Per parameter: the statistics the axes come from, as read and compensated.
    stats: Vec<[Stats; 2]>,
    /// Per parameter: the scale "Auto" stands for.
    auto: Vec<Scale>,
}

/// A parameter's name and a value of it.
pub type NamedValue = (String, f64);
/// A tick line in stored pixels: start, end and label.
pub type Tick = ((f32, f32), (f32, f32), String);

/// The positions of the scale axes: Auto, then [`Scale::ALL`].
fn scale_of(auto: Scale, pos: u32) -> Scale {
    match pos {
        0 => auto,
        k => Scale::ALL
            .get(k as usize - 1)
            .copied()
            .unwrap_or(Scale::Linear),
    }
}

impl CytometryFacts {
    fn param(&self, index: &[u32], which: usize) -> Option<usize> {
        let p = *index.get(which)? as usize;
        (p < self.names.len()).then_some(p)
    }

    /// Whether the plot at `index` is the x parameter's histogram.
    pub fn histogram(&self, index: &[u32]) -> bool {
        index
            .get(1)
            .is_some_and(|&p| p as usize == self.names.len())
    }

    fn compensated(&self, index: &[u32]) -> bool {
        self.compensation.is_some() && index.get(4).copied().unwrap_or(0) == 0
    }

    /// The parameter and axis of the plot's x (`which` 0) or y (1) at `index`; `None` for
    /// the y of a histogram.
    pub fn axis(&self, index: &[u32], which: usize) -> Option<(usize, Axis)> {
        let p = self.param(index, which)?;
        let s = scale_of(self.auto[p], index.get(2 + which).copied().unwrap_or(0));
        let st = &self.stats[p][usize::from(self.compensated(index))];
        Some((p, Axis::new(s, st)))
    }

    /// The x and y parameter values at stored pixel (`x`, `y`) (continuous, (0, 0) the top
    /// left corner): name and value; y is `None` on a histogram.
    pub fn values_at(
        &self,
        index: &[u32],
        x: f64,
        y: f64,
    ) -> (Option<NamedValue>, Option<NamedValue>) {
        let n = f64::from(self.bins);
        let xv = self
            .axis(index, 0)
            .map(|(p, a)| (self.names[p].clone(), a.value(x / n)));
        let yv = self
            .axis(index, 1)
            .map(|(p, a)| (self.names[p].clone(), a.value(1.0 - y / n)));
        (xv, yv)
    }

    /// Tick lines in stored pixels, each (start, end, label): x ticks run up from the bottom
    /// edge, y ticks right from the left edge, so their labels sit on those edges.
    pub fn ticks(&self, index: &[u32]) -> Vec<Tick> {
        let n = self.bins as f32;
        let mut out = Vec::new();
        if let Some((_, a)) = self.axis(index, 0) {
            for (u, label) in a.ticks() {
                let x = u as f32 * n;
                out.push(((x, n), (x, 0.0), label));
            }
        }
        if let Some((_, a)) = self.axis(index, 1) {
            for (u, label) in a.ticks() {
                let y = (1.0 - u as f32) * n;
                out.push(((0.0, y), (n, y), label));
            }
        }
        out
    }

    /// The scale names of the x and y axes at `index` (y empty on a histogram).
    pub fn scale_names(&self, index: &[u32]) -> (&'static str, &'static str) {
        let name = |w: usize| self.axis(index, w).map_or("", |(_, a)| a.scale.name());
        (name(0), name(1))
    }
}

/// The scale "Auto" picks for a parameter.
fn auto_scale(d: &Dataset, p: usize) -> Scale {
    let par = &d.params[p];
    let name = par.name.trim().to_ascii_uppercase();
    if name.starts_with("FSC") || name.starts_with("SSC") || name == "TIME" {
        return Scale::Linear;
    }
    if par.log_amplified() {
        return Scale::Log;
    }
    match par.display.as_deref().map(|s| s.to_ascii_lowercase()) {
        Some(s) if s.starts_with("log") => return Scale::Log,
        Some(s) if s.starts_with("lin") => return Scale::Linear,
        _ => {}
    }
    match d.datatype {
        specular_fcs::DataType::Float | specular_fcs::DataType::Double => Scale::Logicle,
        // Channel numbers of linear-amplified detectors.
        _ => Scale::Linear,
    }
}

/// The first parameter whose name starts with `prefix`, its `-A` (area) one first.
fn find_param(d: &Dataset, prefix: &str) -> Option<usize> {
    let names: Vec<String> = d
        .params
        .iter()
        .map(|p| p.name.trim().to_ascii_uppercase())
        .collect();
    names
        .iter()
        .position(|n| n.starts_with(prefix) && n.ends_with("-A"))
        .or_else(|| names.iter().position(|n| n.starts_with(prefix)))
}

fn set_data(fcs: &Fcs, data: &[u8], k: usize) -> Result<SetData> {
    let d = &fcs.datasets[k];
    let events = Arc::new(d.read_events(data, false).map_err(map_err)?);
    let compensated = match &d.spillover {
        Some(_) => match d.read_events(data, true) {
            Ok(e) => Some(Arc::new(e)),
            // A singular matrix: no compensation axis.
            Err(_) => None,
        },
        None => None,
    };
    let stats: Vec<[Stats; 2]> = (0..d.params.len())
        .map(|p| {
            let par = &d.params[p];
            let top = par.scale_top();
            let a = Stats::of(top, par.log, events.column(p));
            let b = compensated
                .as_ref()
                .map_or(a, |c| Stats::of(top, par.log, c.column(p)));
            [a, b]
        })
        .collect();
    let facts = CytometryFacts {
        version: d.version.clone(),
        events: d.events as u64,
        names: d.params.iter().map(|p| p.title()).collect(),
        bins: BINS,
        compensation: compensated
            .as_ref()
            .and(d.spillover.as_ref())
            .map(|s| s.keyword.clone()),
        compensation_note: d.compensation_note.clone().or_else(|| {
            (d.spillover.is_some() && compensated.is_none())
                .then(|| "the spillover matrix is singular".to_owned())
        }),
        stats,
        auto: (0..d.params.len()).map(|p| auto_scale(d, p)).collect(),
    };
    Ok(SetData {
        events,
        compensated,
        facts: Arc::new(facts),
    })
}

struct FcsSession {
    source: Arc<Source>,
    shared: Arc<Shared>,
    page: u32,
    set: Arc<SetData>,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

fn kw(k: impl Into<String>, v: impl ToString) -> (String, String, String) {
    (k.into(), v.to_string(), String::new())
}

impl FcsSession {
    fn new(source: Arc<Source>, shared: Arc<Shared>, page: u32) -> Result<FcsSession> {
        let k = page as usize;
        let d = shared
            .fcs
            .datasets
            .get(k)
            .ok_or_else(|| DecodeError::Unsupported("no such dataset".into()))?;
        let set = match shared.sets.get(k) {
            Some(cell) => match cell.get() {
                Some(s) => s.clone(),
                None => {
                    let s = Arc::new(set_data(&shared.fcs, source.bytes(), k)?);
                    cell.get_or_init(|| s).clone()
                }
            },
            None => Arc::new(set_data(&shared.fcs, source.bytes(), k)?),
        };
        if d.params.is_empty() {
            return Err(DecodeError::Unsupported(
                "an FCS dataset without parameters".into(),
            ));
        }
        let names = set.facts.names.clone();
        let n = names.len();
        let mut y_labels = names.clone();
        y_labels.push("Histogram".into());
        let scale_labels: Vec<String> = std::iter::once("Auto")
            .chain(Scale::ALL.iter().map(|s| s.name()))
            .map(str::to_owned)
            .collect();
        let ax = |name: &str, labels: Vec<String>| NdAxis {
            name: name.to_owned(),
            len: labels.len() as u32,
            unit: String::new(),
            labels,
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Cube,
        };
        let mut axes = vec![
            ax("X", names.clone()),
            ax("Y", y_labels),
            ax("X scale", scale_labels.clone()),
            ax("Y scale", scale_labels),
        ];
        if set.facts.compensation.is_some() {
            axes.push(ax(
                "Compensation",
                vec!["Applied".to_owned(), "Off".to_owned()],
            ));
        }
        let fx = find_param(d, "FSC").unwrap_or(0);
        let fy = find_param(d, "SSC")
            .filter(|&y| y != fx)
            .unwrap_or(if n > 1 { usize::from(fx == 0) } else { n });
        let mut start = vec![fx as u32, fy as u32, 0, 0];
        if set.facts.compensation.is_some() {
            start.push(0);
        }
        let mut keywords: Vec<(String, String, String)> = d
            .text
            .iter()
            .take(MAX_KEYWORDS)
            .map(|(k, v)| kw(k.clone(), v))
            .collect();
        keywords.extend(
            d.analysis
                .iter()
                .take(MAX_KEYWORDS)
                .map(|(k, v)| kw(format!("ANALYSIS {k}"), v)),
        );
        if let Some(note) = &set.facts.compensation_note {
            keywords.insert(0, kw("Compensation", format!("not applied: {note}")));
        } else if let Some(c) = &set.facts.compensation {
            keywords.insert(0, kw("Compensation", format!("{c} applied")));
        }
        let get = |key: &str| d.keyword(key).map(str::to_owned).filter(|v| !v.is_empty());
        let mut sample = format!(
            "{}, {} parameters, {} events, {}",
            d.version,
            n,
            d.events,
            d.datatype.name()
        );
        if d.truncated {
            sample += ", truncated";
        }
        let sets = shared.fcs.datasets.len();
        let summary = ScienceSummary {
            width: BINS,
            height: BINS,
            start,
            axes,
            sample: sample.clone(),
            title: get("$SRC").or_else(|| get("TUBE NAME")),
            dataset: (sets > 1).then(|| format!("Dataset {} of {sets}", k + 1)),
            instrument: get("$CYT"),
            date_obs: match (get("$DATE"), get("$BTIM")) {
                (Some(d), Some(t)) => Some(format!("{d} {t}")),
                (d, _) => d,
            },
            kind: ScienceKind::Cytometry,
            luts: vec![viridis_lut()],
            keywords: Arc::new(keywords),
            cytometry: Some(set.facts.clone()),
            ..Default::default()
        };
        let info = ImageInfo {
            format: Format::Fcs,
            width: BINS,
            height: BINS,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 32,
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(FcsSession {
            source,
            shared,
            page,
            set,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        for (k, a) in self.summary.axes.iter().enumerate() {
            if index.get(k).copied().unwrap_or(0) >= a.len {
                return Err(DecodeError::Malformed("no such plane".into()));
            }
        }
        let f = &self.set.facts;
        let ev = match &self.set.compensated {
            Some(c) if f.compensated(index) => c,
            _ => &self.set.events,
        };
        let n = BINS as usize;
        let (px, xa) = f
            .axis(index, 0)
            .ok_or_else(|| DecodeError::Malformed("no x parameter".into()))?;
        let values = match f.axis(index, 1) {
            Some((py, ya)) => scale::density(ev, px, &xa, py, &ya, n),
            None => scale::histogram_image(&scale::histogram(ev, px, &xa, n), n),
        };
        Ok(NdPlane {
            width: BINS,
            height: BINS,
            values,
            truncated: false,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(index, cancel, || self.read(index))
    }

    /// The opening plot through viridis, counts on a log scale (zero bins dark): the
    /// thumbnail and the look before science mode.
    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let p = self.plane(&self.summary.start.clone(), cancel)?;
        Ok(density_raster(&p))
    }
}

/// Counts through viridis on a log scale, `ln(1 + c) / ln(1 + max)`.
pub(crate) fn density_raster(p: &NdPlane) -> Raster {
    let lut = viridis_lut();
    let max = p.values.iter().copied().fold(0f32, f32::max);
    let k = if max > 0.0 {
        1.0 / (1.0 + max).ln()
    } else {
        0.0
    };
    let mut rgba = Vec::with_capacity(p.values.len() * 4);
    for &v in &p.values {
        let i = ((1.0 + v.max(0.0)).ln() * k * 255.0)
            .round()
            .clamp(0.0, 255.0) as usize;
        let c = lut[i];
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    whole::interleaved(p.width, p.height, PixelLayout::Rgba, rgba, p.truncated)
}

impl Session for FcsSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(BINS) * u64::from(BINS);
        px * 8 + whole::level_bytes(BINS, BINS, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        let ev = self.set.events.values.len() * 4
            + self
                .set
                .compensated
                .as_ref()
                .map_or(0, |c| c.values.len() * 4);
        self.levels.memory_bytes() + self.planes.memory_bytes() + ev
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.shared.fcs.datasets.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        let n = self.shared.fcs.datasets.len();
        (n > 1).then(|| {
            let d = &self.shared.fcs.datasets[self.page as usize];
            let name = d
                .keyword("$SRC")
                .or_else(|| d.keyword("$FIL"))
                .filter(|s| !s.is_empty())
                .map_or_else(|| format!("Dataset {}", self.page + 1), str::to_owned);
            PageLabel::Sheet {
                name: Arc::from(name.as_str()),
                model: false,
            }
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.shared.fcs.datasets.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(FcsSession::new(
            self.source.clone(),
            self.shared.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> Vec<u8> {
        // FSC-A, SSC-A, FITC-A, PE-A (float), 3 events, a spillover between FITC and PE.
        let text: Vec<(&str, String)> = [
            ("$BYTEORD", "1,2,3,4"),
            ("$DATATYPE", "F"),
            ("$PAR", "4"),
            ("$TOT", "3"),
            ("$CYT", "Synthetic"),
            ("$P1N", "FSC-A"),
            ("$P1B", "32"),
            ("$P1R", "262144"),
            ("$P2N", "SSC-A"),
            ("$P2B", "32"),
            ("$P2R", "262144"),
            ("$P3N", "FITC-A"),
            ("$P3S", "CD3"),
            ("$P3B", "32"),
            ("$P3R", "262144"),
            ("$P4N", "PE-A"),
            ("$P4B", "32"),
            ("$P4R", "262144"),
            ("$SPILLOVER", "2,FITC-A,PE-A,1,0.1,0.05,1"),
        ]
        .iter()
        .map(|(k, v)| (*k, (*v).to_owned()))
        .collect();
        let ev = [
            1000.0f32, 500.0, 100.0, 50.0, 200_000.0, 100_000.0, 10.0, 2000.0, 50_000.0, 70_000.0,
            -20.0, 1.0,
        ];
        let data: Vec<u8> = ev.iter().flat_map(|v| v.to_le_bytes()).collect();
        specular_fcs::write_dataset(&text, &data)
    }

    #[test]
    fn opens_as_a_density_plot_with_axes() {
        let src = Arc::new(Source::from_vec(
            std::path::PathBuf::from("cells.fcs"),
            file(),
        ));
        let s = crate::Registry::default().open(src).unwrap();
        assert_eq!(s.info().format, Format::Fcs);
        let sci = s.science().unwrap();
        assert_eq!(sci.kind, ScienceKind::Cytometry);
        let names: Vec<&str> = sci.axes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["X", "Y", "X scale", "Y scale", "Compensation"]);
        assert_eq!(sci.start, vec![0, 1, 0, 0, 0]);
        assert_eq!(sci.axes[0].labels[2], "FITC-A (CD3)");
        let c = CancelToken::new();
        let p = s.nd_plane(&sci.start, &c).unwrap().unwrap();
        assert_eq!(p.values.iter().sum::<f32>(), 3.0);
        // The histogram of FITC on logicle; compensation off.
        let h = s.nd_plane(&[2, 4, 0, 0, 1], &c).unwrap().unwrap();
        assert!(h.values.iter().any(|&v| v > 0.0));
        let f = sci.cytometry.as_ref().unwrap();
        assert!(f.histogram(&[2, 4, 0, 0, 1]));
        assert_eq!(f.scale_names(&[2, 3, 0, 0, 0]), ("Logicle", "Logicle"));
        let (x, y) = f.values_at(&sci.start, 256.0, 256.0);
        assert!((x.unwrap().1 - 131_072.0).abs() < 1.0);
        assert_eq!(y.unwrap().0, "SSC-A");
        assert!(!f.ticks(&sci.start).is_empty());
        s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert!(s.nd_plane(&[9, 0, 0, 0, 0], &c).unwrap().is_err());
    }
}
