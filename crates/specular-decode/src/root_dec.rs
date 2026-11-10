//! CERN ROOT files (`.root`) through `specular-root`: one page per histogram or graph, the
//! object picker being the page list (`path (class)`), opening on the first 2-D histogram.
//!
//! * TH2 and TProfile2D (and a TGraph2D on a regular grid) are an image of their bins, y up,
//!   through viridis on the science path, so the stretches (log z on `s`) apply. Under- and
//!   overflow bins are left out and said so in the readout when they hold anything.
//!   Variable-width bins are resampled onto an even grid of pixels (said so too), so that
//!   pixels stay proportional to the axis values.
//! * TH3 is a cube: its x–y planes with z on a slider.
//! * TH1, TProfile and TGraph are a strip of their bins (points) whose spectrum plot is the
//!   histogram (graph) itself, under the pointer anywhere on the strip.
//!
//! Trees are listed with their entry counts only. Objects are read when their page opens.

use std::sync::Arc;

use specular_core::nd::Limits;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_root::{self as root, Graph, Hist, Object, RootFile};

use crate::imzml_dec::viridis_lut;
use crate::science::{
    AxisKind, ColourTable, Linear, NdAxis, NdPlane, PixelSpectra, PlaneCache, RootAxis, RootFacts,
    ScienceKind, ScienceSummary, Spectrum, SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

pub struct RootDecoder;

/// Objects offered as pages at most.
const MAX_PAGES: usize = 10_000;
/// Trees whose entry counts are read at most.
const MAX_TREES: usize = 256;
/// Keys listed as keywords at most.
const MAX_KEYWORDS: usize = 4000;
/// Pixels along an axis at most (a resampled variable axis, a long TH1).
const MAX_SIDE: usize = 4096;
/// Bins along an axis shown one pixel each at most.
const MAX_BINS: usize = 1 << 15;

fn map_err(e: root::Error) -> DecodeError {
    match e {
        root::Error::NotRoot => DecodeError::UnsupportedFormat,
        root::Error::Truncated(s) => DecodeError::Malformed(format!("truncated: {s}")),
        root::Error::Corrupt(s) => DecodeError::Malformed(s),
        root::Error::Compression(s) => DecodeError::Malformed(format!("compression: {s}")),
        e @ (root::Error::Unsupported(_) | root::Error::Version { .. }) => {
            DecodeError::Unsupported(e.to_string())
        }
    }
}

impl Decoder for RootDecoder {
    fn format(&self) -> Format {
        Format::Root
    }
    fn probe(&self, head: &[u8]) -> bool {
        root::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["root"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let file = RootFile::open(data).map_err(map_err)?;
        let pages = pages(&file);
        if pages.is_empty() {
            return Err(DecodeError::Unsupported(
                "no histogram or graph in the ROOT file".into(),
            ));
        }
        let trees = trees(&file, data);
        let keywords = keywords(&file, &trees);
        let shared = Arc::new(Shared {
            file,
            pages,
            trees,
            keywords: Arc::new(keywords),
        });
        // The first 2-D histogram (the thumbnail's), else the first object that reads.
        let order = (0..shared.pages.len()).filter(|&p| is_2d(&shared.key(p).class));
        let rest = (0..shared.pages.len()).filter(|&p| !is_2d(&shared.key(p).class));
        let mut first_err = None;
        for p in order.chain(rest).take(64) {
            match RootSession::new(source.clone(), shared.clone(), p as u32) {
                Ok(s) => return Ok(Arc::new(s)),
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
        Err(first_err.unwrap_or(DecodeError::Unsupported("no readable object".into())))
    }
}

fn is_2d(class: &str) -> bool {
    root::hist_kind(class).is_some_and(|(d, _, _)| d == 2)
}

/// The keys shown as pages: histograms and graphs, the highest cycle of each path, in file
/// order.
fn pages(file: &RootFile) -> Vec<usize> {
    let mut best: std::collections::HashMap<String, (i16, usize)> = Default::default();
    for (i, k) in file.keys.iter().enumerate() {
        if !(root::is_hist_class(&k.class) || root::is_graph_class(&k.class)) {
            continue;
        }
        let e = best.entry(k.path()).or_insert((k.cycle, i));
        if k.cycle > e.0 {
            *e = (k.cycle, i);
        }
    }
    let mut v: Vec<usize> = best.into_values().map(|(_, i)| i).collect();
    v.sort_unstable();
    v.truncate(MAX_PAGES);
    v
}

/// The trees of the file and their entry counts (-1 when the header cannot be read).
fn trees(file: &RootFile, data: &[u8]) -> Vec<(String, i64)> {
    let mut seen = std::collections::HashSet::new();
    file.keys
        .iter()
        .rev()
        .filter(|k| root::is_tree_class(&k.class) && seen.insert(k.path()))
        .take(MAX_TREES)
        .map(|k| {
            let n = match file.read(data, k) {
                Ok(Object::Tree(t)) => t.entries,
                _ => -1,
            };
            (k.path(), n)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// Every key as a keyword: path (with its cycle), class (with a tree's entries), title.
fn keywords(file: &RootFile, trees: &[(String, i64)]) -> Vec<(String, String, String)> {
    file.keys
        .iter()
        .take(MAX_KEYWORDS)
        .map(|k| {
            let path = k.path();
            let mut class = k.class.clone();
            if let Some((_, n)) = trees.iter().find(|(p, _)| *p == path)
                && *n >= 0
            {
                class = format!("{class}, {n} entries");
            }
            (format!("{path};{}", k.cycle), class, k.title.clone())
        })
        .collect()
}

/// What every page of a file shares.
struct Shared {
    file: RootFile,
    pages: Vec<usize>,
    trees: Vec<(String, i64)>,
    keywords: Arc<Vec<(String, String, String)>>,
}

impl Shared {
    fn key(&self, page: usize) -> &root::Key {
        &self.file.keys[self.pages[page]]
    }
}

/// What a page shows.
enum View {
    /// A histogram: planes of its cells (x–y, z on the slider).
    Hist {
        hist: Box<Hist>,
        /// The bin (from 1) under each column and each displayed row.
        bx: Arc<[u32]>,
        by: Arc<[u32]>,
    },
    /// A graph's points, sorted by x, as a strip.
    Graph { points: Arc<[(f64, f64)]> },
}

struct RootSession {
    source: Arc<Source>,
    shared: Arc<Shared>,
    page: u32,
    view: View,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

/// The bin (from 1) under each pixel of an axis, and whether variable bins were resampled.
fn pixel_bins(a: &root::Axis) -> Result<(Vec<u32>, bool)> {
    let n = a.nbins;
    let variable = a.is_variable() && {
        let w0 = a.width(1);
        (2..=n).any(|i| (a.width(i) - w0).abs() > 1e-9 * w0.abs().max(1e-300))
    };
    if !variable {
        if n > MAX_BINS {
            return Err(DecodeError::TooLarge);
        }
        return Ok(((1..=n as u32).collect(), false));
    }
    let lo = a.edge(1);
    let hi = a.edge(n + 1);
    let narrow = (1..=n)
        .map(|i| a.width(i))
        .filter(|w| *w > 0.0)
        .fold(f64::INFINITY, f64::min);
    if !(hi > lo && narrow.is_finite()) {
        return Err(DecodeError::Malformed("an axis with no width".into()));
    }
    let w = ((hi - lo) / narrow)
        .ceil()
        .clamp(n.min(MAX_SIDE) as f64, MAX_SIDE as f64) as usize;
    let edges = &a.edges;
    let bins = (0..w)
        .map(|c| {
            let x = lo + (hi - lo) * (c as f64 + 0.5) / w as f64;
            edges.partition_point(|&e| e <= x).clamp(1, n) as u32
        })
        .collect();
    Ok((bins, true))
}

/// A histogram axis for the facts.
fn root_axis(h: &Hist, k: usize, pixel_bins: Vec<u32>) -> RootAxis {
    let a = &h.axes[k];
    let edges: Vec<f64> = (1..=a.nbins + 1).map(|i| a.edge(i)).collect();
    RootAxis {
        title: a.quantity().to_owned(),
        unit: a.unit().unwrap_or("").to_owned(),
        bins: a.nbins as u32,
        min: edges[0],
        max: edges[a.nbins],
        variable: a.is_variable(),
        mean_rms: h.mean_rms(k),
        edges: edges.into(),
        labels: a
            .labels
            .iter()
            .map(|(b, s)| (*b as u32, s.clone()))
            .collect::<Vec<_>>()
            .into(),
        pixel_bins: pixel_bins.into(),
    }
}

/// The strip height of a 1-D object `w` pixels wide: an eighth of its width, 1 to 64 rows.
fn strip(w: usize) -> usize {
    (w / 8).clamp(1, 64)
}

impl RootSession {
    fn new(source: Arc<Source>, shared: Arc<Shared>, page: u32) -> Result<RootSession> {
        let key = shared.key(page as usize);
        let obj = shared.file.read(source.bytes(), key).map_err(map_err)?;
        let f = &shared.file;
        let mut facts = RootFacts {
            version: f.version_string(),
            compression: f.compression(),
            keys: f.keys.len() as u64,
            class: key.class.clone(),
            path: key.path(),
            trees: shared.trees.clone(),
            ..Default::default()
        };
        let mut axes = Vec::new();
        let mut spectra: Option<Arc<dyn SpectrumSource>> = None;
        let (view, width, height, sample) = match obj {
            Object::Hist(h) => {
                let (w, h2) = Self::hist_view(&h, &mut facts, &mut axes, &mut spectra)?;
                let sample = match h.storage {
                    _ if h.profile => "profile (mean per bin), float64".to_owned(),
                    Some(s) => s.describe().to_owned(),
                    None => "float64".to_owned(),
                };
                (w, h2.0, h2.1, sample)
            }
            Object::Graph(g) => match g.to_grid() {
                Some(h) => {
                    let h = Box::new(h);
                    let (w, h2) = Self::hist_view(&h, &mut facts, &mut axes, &mut spectra)?;
                    facts.class = g.class.clone();
                    (w, h2.0, h2.1, "float64".to_owned())
                }
                None if !g.z.is_empty() => {
                    return Err(DecodeError::Unsupported(
                        "a TGraph2D of scattered points".into(),
                    ));
                }
                None => {
                    let (view, w, h) = Self::graph_view(&g, &mut facts, &mut spectra)?;
                    (view, w, h, "float64".to_owned())
                }
            },
            Object::Tree(_) => return Err(DecodeError::Unsupported("a tree".into())),
        };
        let (width, height) = (width as u32, height as u32);
        let range = match &view {
            View::Hist { hist, .. } => {
                let (lo, hi) = hist.range().unwrap_or((0.0, 1.0));
                let lo = if hist.minimum != root::UNSET && hist.minimum < hist.maximum {
                    hist.minimum
                } else if hist.profile {
                    lo
                } else {
                    lo.min(0.0)
                };
                let hi = if hist.maximum != root::UNSET && hist.maximum > lo {
                    hist.maximum
                } else {
                    hi
                };
                (lo, if hi > lo { hi } else { lo + 1.0 })
            }
            View::Graph { points } => {
                let (lo, hi) = points
                    .iter()
                    .map(|p| p.1)
                    .filter(|v| v.is_finite())
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
                        (l.min(v), h.max(v))
                    });
                if hi > lo {
                    (lo, hi)
                } else {
                    (lo.min(0.0), lo.max(0.0) + 1.0)
                }
            }
        };
        let facts = Arc::new(facts);
        let summary = ScienceSummary {
            width,
            height,
            start: vec![0; axes.len()],
            axes,
            sample: sample.clone(),
            title: Some(facts.title.clone()).filter(|t| !t.trim().is_empty()),
            dataset: Some(format!("{} ({})", facts.path, facts.class)),
            kind: ScienceKind::Gridded,
            luts: vec![viridis_lut()],
            display_ranges: vec![range],
            keywords: shared.keywords.clone(),
            spectra: spectra.map(|s| PixelSpectra::new(s, width, height)),
            root: Some(facts),
            ..Default::default()
        };
        let info = ImageInfo {
            format: Format::Root,
            width,
            height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 64,
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(RootSession {
            source,
            shared,
            page,
            view,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    /// A histogram's view and size, filling the facts, the cube axis and (1-D) the spectrum.
    fn hist_view(
        h: &Hist,
        facts: &mut RootFacts,
        axes: &mut Vec<NdAxis>,
        spectra: &mut Option<Arc<dyn SpectrumSource>>,
    ) -> Result<(View, (usize, usize))> {
        let (title, tx, ty) = root::split_titles(&h.title);
        let _ = (tx, ty);
        facts.title = title;
        facts.entries = Some(h.entries);
        facts.sum_weights = h.sum("fTsumw");
        facts.profile = h.profile;
        let (n, sum) = h.flow();
        facts.flow = (n > 0).then_some((n as u64, sum));
        let (bx, rx) = pixel_bins(&h.axes[0])?;
        let (by, ry) = if h.dims >= 2 {
            let (mut b, r) = pixel_bins(&h.axes[1])?;
            // Displayed rows run top-down: the highest bin first.
            b.reverse();
            (b, r)
        } else {
            (vec![1; strip(bx.len())], false)
        };
        facts.resampled = rx || ry;
        facts.axes.push(root_axis(h, 0, bx.clone()));
        if h.dims >= 2 {
            facts.axes.push(root_axis(h, 1, by.clone()));
        }
        if h.dims == 3 {
            let a = &h.axes[2];
            facts.axes.push(root_axis(h, 2, Vec::new()));
            let uniform = !a.is_variable();
            axes.push(NdAxis {
                name: if a.quantity().is_empty() {
                    "z".to_owned()
                } else {
                    a.quantity().to_owned()
                },
                len: u32::try_from(a.nbins).map_err(|_| DecodeError::TooLarge)?,
                unit: a.unit().unwrap_or("").to_owned(),
                labels: if uniform {
                    Vec::new()
                } else {
                    (1..=a.nbins)
                        .map(|i| format!("{} … {}", short(a.edge(i)), short(a.edge(i + 1))))
                        .collect()
                },
                wcs_axis: None,
                linear: uniform.then(|| Linear {
                    offset: a.centre(1),
                    step: a.width(1),
                }),
                kind: AxisKind::Cube,
            });
        }
        if h.dims == 1 {
            let a = &h.axes[0];
            let x: Vec<f64> = (1..=a.nbins).map(|i| a.centre(i)).collect();
            let y: Vec<f32> = (1..=a.nbins).map(|i| h.cell(i, 0, 0) as f32).collect();
            *spectra = Some(Arc::new(Whole {
                spectrum: Spectrum {
                    y,
                    x: SpectrumX::Values(x.into()),
                    sticks: false,
                },
                axis: (a.quantity().to_owned(), a.unit().unwrap_or("").to_owned()),
                grid: (bx.len(), by.len()),
            }));
        }
        let size = (bx.len(), by.len());
        Ok((
            View::Hist {
                hist: Box::new(h.clone()),
                bx: bx.into(),
                by: by.into(),
            },
            size,
        ))
    }

    /// A TGraph's points as a strip, sorted by x.
    fn graph_view(
        g: &Graph,
        facts: &mut RootFacts,
        spectra: &mut Option<Arc<dyn SpectrumSource>>,
    ) -> Result<(View, usize, usize)> {
        let (title, tx, _) = root::split_titles(&g.title);
        facts.title = title;
        let mut pts: Vec<(f64, f64)> =
            g.x.iter()
                .zip(&g.y)
                .map(|(&x, &y)| (x, y))
                .filter(|p| p.0.is_finite())
                .collect();
        if pts.is_empty() {
            return Err(DecodeError::Unsupported("a graph without points".into()));
        }
        if pts.len() > MAX_BINS {
            return Err(DecodeError::TooLarge);
        }
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (q, u) = root::split_unit(&tx);
        let n = pts.len();
        facts.axes.push(RootAxis {
            title: q.to_owned(),
            unit: u.unwrap_or("").to_owned(),
            bins: n as u32,
            min: pts[0].0,
            max: pts[n - 1].0,
            pixel_bins: (1..=n as u32).collect::<Vec<_>>().into(),
            ..Default::default()
        });
        let points: Arc<[(f64, f64)]> = pts.into();
        facts.points = points.clone();
        let h = strip(n);
        *spectra = Some(Arc::new(Whole {
            spectrum: Spectrum {
                y: points.iter().map(|p| p.1 as f32).collect(),
                x: SpectrumX::Values(points.iter().map(|p| p.0).collect::<Vec<_>>().into()),
                sticks: false,
            },
            axis: (q.to_owned(), u.unwrap_or("").to_owned()),
            grid: (n, h),
        }));
        Ok((View::Graph { points }, n, h))
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let values = match &self.view {
            View::Hist { hist, bx, by } => {
                let iz = if hist.dims == 3 {
                    match index.first() {
                        Some(&i) if (i as usize) < hist.axes[2].nbins => i as usize + 1,
                        _ => return Err(DecodeError::Malformed("no such plane".into())),
                    }
                } else {
                    0
                };
                let iy_of = |r: usize| if hist.dims >= 2 { by[r] as usize } else { 0 };
                let mut v = Vec::with_capacity(w * h);
                for r in 0..h {
                    let iy = iy_of(r);
                    v.extend(bx.iter().map(|&ix| hist.cell(ix as usize, iy, iz) as f32));
                }
                v
            }
            View::Graph { points } => {
                let row: Vec<f32> = points.iter().map(|p| p.1 as f32).collect();
                row.repeat(h)
            }
        };
        Ok(NdPlane {
            width: self.info.width,
            height: self.info.height,
            values,
            truncated: false,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(index, cancel, || self.read(index))
    }

    /// The first plane through viridis at the file's display range: the thumbnail.
    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let p = self.plane(&self.summary.start.clone(), cancel)?;
        let range = self.summary.display_ranges.first().copied();
        Ok(lut_raster(&p, &viridis_lut(), range))
    }
}

/// A plane as RGB through `lut` between `range` (min–max when not given).
fn lut_raster(p: &NdPlane, lut: &ColourTable, range: Option<(f64, f64)>) -> Raster {
    let (lo, hi) = range
        .map(|(l, h)| (l as f32, h as f32))
        .or_else(|| Limits::MinMax.compute(&p.values, p.width as usize))
        .filter(|(l, h)| h > l)
        .unwrap_or((0.0, 1.0));
    let scale = 255.0 / (hi - lo);
    let mut rgba = Vec::with_capacity(p.values.len() * 4);
    for v in &p.values {
        let i = if v.is_finite() {
            ((v - lo) * scale).round().clamp(0.0, 255.0) as usize
        } else {
            0
        };
        let c = lut[i];
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    whole::interleaved(p.width, p.height, PixelLayout::Rgba, rgba, p.truncated)
}

/// A value for an axis label: up to six significant digits.
fn short(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if s == "-0" { "0".to_owned() } else { s }
}

/// One spectrum for every pixel: a TH1 or a graph, plotted wherever the pointer is.
#[derive(Debug)]
struct Whole {
    spectrum: Spectrum,
    axis: (String, String),
    grid: (usize, usize),
}

impl SpectrumSource for Whole {
    fn grid(&self) -> (usize, usize) {
        self.grid
    }
    fn channels(&self) -> usize {
        self.spectrum.y.len()
    }
    fn x_axis(&self) -> (String, String) {
        self.axis.clone()
    }
    fn at(&self, _x: usize, _y: usize, _index: &[u32]) -> Option<Spectrum> {
        Some(self.spectrum.clone())
    }
}

impl Session for RootSession {
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
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        px * 8 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        let cells = match &self.view {
            View::Hist { hist, .. } => {
                (hist.cells.len() + hist.sumw2.len() + hist.bin_entries.len()) * 8
            }
            View::Graph { points } => points.len() * 16,
        };
        self.levels.memory_bytes() + self.planes.memory_bytes() + cells
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.shared.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        let k = self.shared.key(self.page as usize);
        Some(PageLabel::Object {
            name: k.path().into(),
            class: k.class.as_str().into(),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.shared.pages.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(RootSession::new(
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

    fn fixture(name: &str) -> Arc<Source> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-root/tests/data")
            .join(name);
        Arc::new(Source::open(&p).expect("fixture"))
    }

    fn page(s: &Arc<dyn Session>, name: &str) -> Arc<dyn Session> {
        (0..s.page_count())
            .map(|p| s.open_page(p).expect("page"))
            .find(|p| matches!(p.page_label(), Some(PageLabel::Object { name: n, .. }) if &*n == name))
            .unwrap_or_else(|| panic!("no page {name}"))
    }

    fn plane(s: &Arc<dyn Session>, index: &[u32]) -> Arc<NdPlane> {
        s.nd_plane(index, &CancelToken::new())
            .expect("nd")
            .expect("plane")
    }

    #[test]
    fn pages_picker_and_first_2d() {
        let s = RootDecoder.open(fixture("types.root")).expect("open");
        assert_eq!(s.page_count(), 16);
        // Opens on the first 2-D histogram.
        assert_eq!(
            s.page_label(),
            Some(PageLabel::Object {
                name: "h2f".into(),
                class: "TH2F".into()
            })
        );
        let f = s.science().and_then(|x| x.root.clone()).expect("facts");
        assert_eq!((f.class.as_str(), f.keys), ("TH2F", 16));
        assert!(f.version.starts_with("6.") && f.compression.starts_with("zlib"));
        assert_eq!(f.axes.len(), 2);
        assert!(f.entries.is_some_and(|e| e > 0.0));
        assert_eq!(s.science().expect("sc").keywords.len(), 16);
        // Directories and newer cycles: one page per path, trees and RNTuples listed only.
        let s = RootDecoder.open(fixture("nested.root")).expect("nested");
        let names: Vec<String> = (0..s.page_count())
            .filter_map(|p| match s.open_page(p).expect("page").page_label() {
                Some(PageLabel::Object { name, .. }) => Some(name.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            names,
            [
                "top",
                "run1/hits",
                "run1/tracker/layer0",
                "run1/tracker/inner/occ",
                "run2/hits"
            ]
        );
        let f = s.science().and_then(|x| x.root.clone()).expect("facts");
        assert_eq!(f.trees, vec![("events".to_owned(), 1234)]);
        assert!(RootDecoder.open(fixture("empty.root")).is_err());
        assert!(RootDecoder.probe(b"root\0\0\xf5\x1e"));
        assert!(!RootDecoder.probe(b"TFile"));
    }

    #[test]
    fn th2_is_y_up_without_flow() {
        let src = fixture("types.root");
        let data = src.bytes();
        let file = RootFile::open(data).expect("file");
        let k = file.keys.iter().find(|k| k.name == "h2d").expect("key");
        let Ok(Object::Hist(h)) = file.read(data, k) else {
            panic!("hist")
        };
        let s = page(&RootDecoder.open(src.clone()).expect("open"), "h2d");
        let (nx, ny) = (h.axes[0].nbins, h.axes[1].nbins);
        assert_eq!(
            (s.info().width as usize, s.info().height as usize),
            (nx, ny)
        );
        let p = plane(&s, &[]);
        // The top row is the highest y bin; the bottom-left pixel is bin (1, 1).
        assert_eq!(p.values[0], h.cell(1, ny, 0) as f32);
        assert_eq!(p.values[(ny - 1) * nx], h.cell(1, 1, 0) as f32);
        assert_eq!(p.values[nx - 1], h.cell(nx, ny, 0) as f32);
        let sc = s.science().expect("sc");
        let f = sc.root.as_ref().expect("facts");
        assert_eq!(f.axes[1].pixel_bins[0] as usize, ny);
        assert!(!f.resampled);
        // The fixture fills under- and overflow.
        assert!(f.flow.is_some_and(|(n, _)| n > 0));
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .expect("raster");
        assert_eq!((r.width as usize, r.height as usize), (nx, ny));
    }

    #[test]
    fn variable_bins_resample() {
        let s = RootDecoder.open(fixture("variable.root")).expect("open");
        let s = page(&s, "v2xy");
        let f = s.science().and_then(|x| x.root.clone()).expect("facts");
        assert!(f.resampled && f.axes[0].variable);
        // Pixels are even steps of the axis: the narrowest bin is one pixel wide or more.
        let w = s.info().width as usize;
        assert!(w > f.axes[0].bins as usize);
        let bins = &f.axes[0].pixel_bins;
        assert_eq!((bins[0], bins[w - 1]), (1, f.axes[0].bins));
        assert!(bins.windows(2).all(|b| b[1] == b[0] || b[1] == b[0] + 1));
        assert!((f.axes[0].min, f.axes[0].max) == (0.0, 100.0));
    }

    #[test]
    fn th3_cube_and_th1_spectrum() {
        let s = RootDecoder.open(fixture("types.root")).expect("open");
        let s3 = page(&s, "h3d");
        let sc = s3.science().expect("sc");
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].kind, AxisKind::Cube);
        let nz = sc.axes[0].len;
        assert!(sc.axes[0].linear.is_some());
        let a = plane(&s3, &[0]);
        let b = plane(&s3, &[nz - 1]);
        assert_eq!(a.values.len(), b.values.len());
        assert!(
            s3.nd_plane(&[nz], &CancelToken::new())
                .expect("nd")
                .is_err()
        );

        let s1 = page(&s, "h1d");
        let sc = s1.science().expect("sc");
        let sp = sc.spectra.as_ref().expect("spectra");
        let spec = sp.spectra.at(0, 0, &[]).expect("spectrum");
        let n = sc.root.as_ref().expect("facts").axes[0].bins as usize;
        assert_eq!(spec.y.len(), n);
        assert_eq!(s1.info().width as usize, n);

        let v = RootDecoder.open(fixture("variable.root")).expect("open");
        let v3 = page(&v, "v3z");
        let sc = v3.science().expect("sc");
        assert!(sc.axes[0].linear.is_none() && !sc.axes[0].labels.is_empty());
    }

    #[test]
    fn profiles_and_labels() {
        let s = RootDecoder.open(fixture("profiles.root")).expect("open");
        let p = page(&s, "prof2d");
        let f = p.science().and_then(|x| x.root.clone()).expect("facts");
        assert!(f.profile);
        let s = RootDecoder.open(fixture("labels.root")).expect("open");
        let p = page(&s, "cutflow");
        let f = p.science().and_then(|x| x.root.clone()).expect("facts");
        assert_eq!(f.axes[0].label(3), Some("2 muons"));
    }
}
