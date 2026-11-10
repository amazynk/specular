//! Scientific arrays through the N-D science path: NumPy `.npy` / `.npz`, MATLAB `.mat` (levels
//! 4 and 5 through `specular-arrays`, v7.3 through `specular-hdf5`) and plain Zarr stores; the
//! session here also serves NI TDMS channels ([`crate::tdms_dec`]) and VTK grids
//! ([`crate::vtk_dec`]).
//!
//! Every array of two or more values is a page, named by its path (`s.field`, `c{2}`, an
//! `.npz` member, a Zarr array's path); the page list is the browser. Scalars and short texts
//! go to the header keywords, and arrays that cannot be shown are listed there with the
//! reason.
//!
//! * **Layout:** NumPy and Zarr arrays put the last dimension along x and the one before it
//!   down y, the others on sliders; MATLAB arrays put rows down y and columns along x, further
//!   dimensions on sliders. Dimensions of length 1 are left out. A trailing dimension of 3 or
//!   4 (MATLAB: a third dimension of 3) on 8- or 16-bit integers is a colour axis.
//! * **1-D arrays** are a strip of their values (bins of the mean past [`MAX_STRIP`] values)
//!   whose spectrum plot is the whole array, under the pointer anywhere on the strip.
//! * **Complex arrays** get a slider of their parts: magnitude first, then real, imaginary
//!   and phase.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use specular_arrays::mat::{self, Kind as MatKind};
use specular_arrays::{Part, PlaneSpec, npy, npz, zarr};
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_hdf5 as hdf5;

use crate::science::{
    self, AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PixelSpectra, PlaneCache,
    ScienceKind, ScienceSummary, Spectrum, SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// Values a 1-D strip shows one per column at most; longer arrays are binned.
pub(crate) const MAX_STRIP: usize = 4096;
/// Points a 1-D plot draws at most; longer arrays are reduced to bin minima and maxima.
const MAX_PLOT: usize = 1 << 16;
/// Pages listed at most.
const MAX_PAGES: usize = 4096;
/// Header lines listed at most.
const MAX_KEYWORDS: usize = 4096;
/// Text kept from a char variable or an attribute.
const MAX_TEXT: usize = 512;
/// A compressed `.npz` member or MATLAB variable inflated at most.
const MAX_INFLATE: u64 = 16 << 30;

pub(crate) fn arr_err(e: specular_arrays::Error) -> DecodeError {
    use specular_arrays::Error as E;
    match e {
        E::NotArray => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        E::Io(s) => DecodeError::Io(std::io::Error::other(s)),
    }
}

/// Reads planes of one page's array.
pub(crate) trait PlaneRead: Send + Sync {
    /// The plane `spec` of the array (logical shape) as values; true when some were missing.
    fn read(&self, spec: &PlaneSpec, part: Part, out: &mut Vec<f32>) -> Result<bool>;
    /// Every `step`-th value of the plane along x and y, when that is cheap (thumbnails).
    fn read_sampled(
        &self,
        _spec: &PlaneSpec,
        _step: u64,
        _part: Part,
        _out: &mut Vec<f32>,
    ) -> Option<Result<(usize, usize)>> {
        None
    }
}

/// How a page's dimensions are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Convention {
    /// NumPy, Zarr, VTK, TDMS: the last dimension along x.
    LastIsX,
    /// MATLAB: rows (the first dimension) down y, columns along x.
    Matlab,
}

/// One array shown as a page.
pub(crate) struct Page {
    /// The array's name or path.
    pub label: String,
    /// What it is, for the page list ("double", "16-bit unsigned").
    pub class: String,
    pub shape: Vec<u64>,
    pub convention: Convention,
    pub complex: bool,
    /// 8- or 16-bit integers: a colour dimension may be one.
    pub rgb_ok: bool,
    pub bits: u32,
    /// Sample type for the info panel.
    pub sample: String,
    pub reader: Arc<dyn PlaneRead>,
    /// A name per dimension, when the file gives them.
    pub dim_names: Vec<Option<String>>,
    /// Physical positions along a dimension and their unit.
    pub dim_cal: Vec<Option<(Linear, String)>>,
    /// Labels for the positions of a dimension.
    pub dim_labels: Vec<Vec<String>>,
    pub keywords: Vec<(String, String, String)>,
    pub unit: Option<String>,
    /// Stored rows run bottom-up (VTK's y axis points up).
    pub flip_y: bool,
    pub title: Option<String>,
    /// For a 2-D page whose rows are separate series (TDMS channels): each row is plotted
    /// under the pointer, against this x axis (name, unit).
    pub row_plot: Option<(String, String)>,
    /// The x value of each value a 1-D read returns (for the plot), when not the x
    /// dimension's own calibration (a page whose columns are bins of a longer series).
    pub plot_cal: Option<Linear>,
}

impl Page {
    pub(crate) fn new(
        label: String,
        shape: Vec<u64>,
        convention: Convention,
        reader: Arc<dyn PlaneRead>,
    ) -> Page {
        let n = shape.len();
        Page {
            label,
            class: String::new(),
            shape,
            convention,
            complex: false,
            rgb_ok: false,
            bits: 32,
            sample: String::new(),
            reader,
            dim_names: vec![None; n],
            dim_cal: vec![None; n],
            dim_labels: vec![Vec::new(); n],
            keywords: Vec::new(),
            unit: None,
            flip_y: false,
            title: None,
            row_plot: None,
            plot_cal: None,
        }
    }

    /// Whether the array has at least two values (a page rather than a keyword).
    pub(crate) fn showable(shape: &[u64]) -> bool {
        specular_arrays::count(shape).is_some_and(|n| n >= 2)
    }
}

/// What an axis of the summary stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AxisOf {
    Dim(usize),
    Part,
}

#[derive(Clone, Debug)]
struct Layout {
    x: usize,
    y: Option<usize>,
    axes: Vec<AxisOf>,
    channel: Option<usize>,
    /// A 1-D page: values per strip column.
    bin: Option<usize>,
    width: u32,
    height: u32,
}

fn layout(p: &Page) -> Result<Layout> {
    let ns: Vec<usize> = (0..p.shape.len()).filter(|&d| p.shape[d] > 1).collect();
    let part = |axes: &mut Vec<AxisOf>| {
        if p.complex {
            axes.push(AxisOf::Part);
        }
    };
    let side = |d: usize| u32::try_from(p.shape[d]).map_err(|_| DecodeError::TooLarge);
    if ns.len() <= 1 {
        let x = ns.first().copied().unwrap_or(0);
        let n = p.shape.get(x).copied().unwrap_or(1) as usize;
        if n as u64 > specular_arrays::MAX_PLANE {
            return Err(DecodeError::TooLarge);
        }
        let bin = n.div_ceil(MAX_STRIP).max(1);
        let w = n.div_ceil(bin).max(1);
        let mut axes = Vec::new();
        part(&mut axes);
        return Ok(Layout {
            x,
            y: None,
            axes,
            channel: None,
            bin: Some(bin),
            width: w as u32,
            height: (w / 8).clamp(1, 64) as u32,
        });
    }
    let (x, y, sliders, channel) = match p.convention {
        Convention::LastIsX => {
            let k = ns.len();
            let last = ns[k - 1];
            if p.rgb_ok && k >= 3 && (p.shape[last] == 3 || p.shape[last] == 4) {
                (ns[k - 2], ns[k - 3], ns[..k - 3].to_vec(), Some(last))
            } else {
                (last, ns[k - 2], ns[..k - 2].to_vec(), None)
            }
        }
        Convention::Matlab => {
            if p.rgb_ok && ns.len() >= 3 && p.shape[ns[2]] == 3 {
                (ns[1], ns[0], ns[3..].to_vec(), Some(ns[2]))
            } else {
                (ns[1], ns[0], ns[2..].to_vec(), None)
            }
        }
    };
    let (w, h) = (side(x)?, side(y)?);
    if u64::from(w) * u64::from(h) > specular_arrays::MAX_PLANE {
        return Err(DecodeError::TooLarge);
    }
    // The other dimensions on sliders, in the file's order.
    let mut axes: Vec<AxisOf> = sliders.into_iter().map(AxisOf::Dim).collect();
    let mut ch = None;
    if let Some(c) = channel {
        ch = Some(axes.len());
        axes.push(AxisOf::Dim(c));
    }
    part(&mut axes);
    Ok(Layout {
        x,
        y: Some(y),
        axes,
        channel: ch,
        bin: None,
        width: w,
        height: h,
    })
}

/// What every page of a file shares.
pub(crate) struct Shared {
    pub format: Format,
    pub pages: Vec<Page>,
    /// The file's own header lines (after each page's).
    pub keywords: Vec<(String, String, String)>,
    pub kind: ScienceKind,
}

impl Shared {
    pub(crate) fn new(format: Format) -> Shared {
        Shared {
            format,
            pages: Vec::new(),
            keywords: Vec::new(),
            kind: ScienceKind::Gridded,
        }
    }

    pub(crate) fn kw(&mut self, k: impl Into<String>, v: impl Into<String>) {
        if self.keywords.len() < MAX_KEYWORDS {
            self.keywords
                .push((k.into(), clip(v.into()), String::new()));
        }
    }
}

fn clip(mut s: String) -> String {
    if s.chars().count() > MAX_TEXT {
        s = s.chars().take(MAX_TEXT).collect::<String>() + "…";
    }
    s
}

/// Opens the first page of `shared` (an error when it has none: `empty` says why).
pub(crate) fn session(
    source: Arc<Source>,
    shared: Shared,
    empty: &str,
) -> Result<Arc<dyn Session>> {
    if shared.pages.is_empty() {
        return Err(DecodeError::Unsupported(empty.to_owned()));
    }
    let shared = Arc::new(shared);
    // The first page that lays out (a page too large to show still lists).
    let mut first = None;
    for i in 0..shared.pages.len() {
        match ArraySession::new(source.clone(), shared.clone(), i as u32) {
            Ok(s) => {
                first = Some(s);
                break;
            }
            Err(e) if i + 1 == shared.pages.len() => return Err(e),
            Err(_) => {}
        }
    }
    Ok(Arc::new(first.ok_or_else(|| {
        DecodeError::Unsupported(empty.to_owned())
    })?))
}

pub(crate) struct ArraySession {
    source: Arc<Source>,
    shared: Arc<Shared>,
    page: u32,
    layout: Layout,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

impl ArraySession {
    fn new(source: Arc<Source>, shared: Arc<Shared>, page: u32) -> Result<ArraySession> {
        let p = &shared.pages[page as usize];
        let l = layout(p)?;
        let mut axes = Vec::new();
        for a in &l.axes {
            axes.push(match *a {
                AxisOf::Dim(d) => {
                    let (linear, unit) = match &p.dim_cal[d] {
                        Some((lin, u)) => (Some(*lin), u.clone()),
                        None => (None, String::new()),
                    };
                    let mut labels = p.dim_labels[d].clone();
                    let kind = if Some(axes.len()) == l.channel {
                        if labels.is_empty() {
                            labels = ["Red", "Green", "Blue", "Alpha"]
                                .iter()
                                .take(p.shape[d] as usize)
                                .map(|s| (*s).to_owned())
                                .collect();
                        }
                        AxisKind::Channel
                    } else {
                        AxisKind::Cube
                    };
                    NdAxis {
                        name: p.dim_names[d].clone().unwrap_or_default(),
                        len: u32::try_from(p.shape[d]).map_err(|_| DecodeError::TooLarge)?,
                        unit,
                        labels,
                        wcs_axis: None,
                        linear,
                        kind,
                    }
                }
                AxisOf::Part => NdAxis {
                    name: String::new(),
                    len: Part::ALL.len() as u32,
                    unit: String::new(),
                    labels: Vec::new(),
                    wcs_axis: None,
                    linear: None,
                    kind: AxisKind::Part,
                },
            });
        }
        let cal = |d: Option<usize>| d.and_then(|d| p.dim_cal[d].clone());
        let pixel = match (cal(Some(l.x)), cal(l.y)) {
            (None, None) => None,
            (cx, cy) if l.y.is_some() => {
                let one = (
                    Linear {
                        offset: 0.0,
                        step: 1.0,
                    },
                    String::new(),
                );
                let (x, ux) = cx.unwrap_or_else(|| one.clone());
                let (mut y, uy) = cy.unwrap_or(one);
                if p.flip_y {
                    y = Linear {
                        offset: y.at(f64::from(l.height) - 1.0),
                        step: -y.step,
                    };
                }
                Some(PixelCalibration {
                    x,
                    y,
                    unit_x: ux,
                    unit_y: uy,
                })
            }
            _ => None,
        };
        let mut keywords = p.keywords.clone();
        keywords.extend(shared.keywords.iter().cloned());
        let spectra: Option<Arc<dyn SpectrumSource>> = if l.bin.is_some() {
            let name = p.dim_names[l.x].clone().unwrap_or_default();
            let unit = p.dim_cal[l.x]
                .as_ref()
                .map_or_else(String::new, |(_, u)| u.clone());
            Some(Arc::new(LinePlot {
                reader: p.reader.clone(),
                shape: p.shape.clone(),
                x: l.x,
                y: None,
                cal: p.plot_cal.or(p.dim_cal[l.x].as_ref().map(|c| c.0)),
                axis: (name, unit),
                grid: (l.width as usize, l.height as usize),
                part_axis: l.axes.iter().position(|a| *a == AxisOf::Part),
                rows: 1,
                flip: false,
                cache: Mutex::new(None),
            }))
        } else if let (Some((name, unit)), Some(y)) = (&p.row_plot, l.y) {
            Some(Arc::new(LinePlot {
                reader: p.reader.clone(),
                shape: p.shape.clone(),
                x: l.x,
                y: Some(y),
                cal: p.plot_cal.or(p.dim_cal[l.x].as_ref().map(|c| c.0)),
                axis: (name.clone(), unit.clone()),
                grid: (l.width as usize, l.height as usize),
                part_axis: l.axes.iter().position(|a| *a == AxisOf::Part),
                rows: l.height as usize,
                flip: p.flip_y,
                cache: Mutex::new(None),
            }))
        } else {
            None
        };
        let summary = ScienceSummary {
            width: l.width,
            height: l.height,
            axes,
            channel_axis: l.channel,
            sample: p.sample.clone(),
            dataset: Some(p.label.clone()).filter(|s| !s.is_empty()),
            title: p.title.clone(),
            unit: p.unit.clone(),
            keywords: Arc::new(keywords),
            kind: shared.kind,
            pixel,
            spectra: spectra.map(|s| PixelSpectra::new(s, l.width, l.height)),
            ..Default::default()
        };
        let info = ImageInfo {
            format: shared.format,
            width: l.width,
            height: l.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: p.bits.clamp(1, 64) as u8,
            progressive: false,
            coding: p.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(ArraySession {
            source,
            shared,
            page,
            layout: l,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn page(&self) -> &Page {
        &self.shared.pages[self.page as usize]
    }

    /// The array plane and complex part for a position on the summary's axes.
    fn spec_of(&self, index: &[u32]) -> Result<(PlaneSpec, Part)> {
        let p = self.page();
        let mut idx = vec![0u64; p.shape.len()];
        let mut part = Part::Magnitude;
        for (k, a) in self.layout.axes.iter().enumerate() {
            let i = index.get(k).copied().unwrap_or(0);
            match *a {
                AxisOf::Dim(d) => {
                    if u64::from(i) >= p.shape[d] {
                        return Err(DecodeError::Malformed("no such plane".into()));
                    }
                    idx[d] = u64::from(i);
                }
                AxisOf::Part => {
                    part = *Part::ALL
                        .get(i as usize)
                        .ok_or_else(|| DecodeError::Malformed("no such plane".into()))?;
                }
            }
        }
        Ok((
            PlaneSpec {
                x_dim: self.layout.x,
                y_dim: self.layout.y,
                index: idx,
            },
            part,
        ))
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        let (spec, part) = self.spec_of(index)?;
        let p = self.page();
        let mut values = Vec::new();
        let truncated = p.reader.read(&spec, part, &mut values)?;
        let (w, h) = (self.layout.width as usize, self.layout.height as usize);
        if let Some(bin) = self.layout.bin {
            let row = bin_means(&values, bin, w);
            values = row.repeat(h);
        } else if values.len() != w * h {
            return Err(DecodeError::Malformed("plane size".into()));
        }
        if p.flip_y && w > 0 {
            flip_rows(&mut values, w, h);
        }
        Ok(NdPlane {
            width: self.layout.width,
            height: self.layout.height,
            values,
            truncated,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(index, cancel, || self.read(index))
    }

    fn first_index(&self) -> Vec<u32> {
        vec![0; self.layout.axes.len()]
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let planes = match self.summary.channel_axis {
            Some(c) => (0..self.summary.axes[c].len.min(3))
                .map(|k| {
                    let mut idx = self.first_index();
                    idx[c] = k;
                    self.plane(&idx, cancel)
                })
                .collect::<Result<Vec<_>>>()?,
            None => vec![self.plane(&self.first_index(), cancel)?],
        };
        Ok(science::auto_raster(&planes, None))
    }

    /// A strided read of the first plane (or colour planes) for a thumbnail.
    fn sampled(&self, step: u64, cancel: &CancelToken) -> Option<Result<Raster>> {
        if self.layout.bin.is_some() || self.page().flip_y {
            return None;
        }
        let idxs: Vec<Vec<u32>> = match self.summary.channel_axis {
            Some(c) => (0..self.summary.axes[c].len.min(3))
                .map(|k| {
                    let mut i = self.first_index();
                    i[c] = k;
                    i
                })
                .collect(),
            None => vec![self.first_index()],
        };
        let mut planes = Vec::new();
        for idx in idxs {
            if cancel.is_cancelled() {
                return Some(Err(DecodeError::Cancelled));
            }
            let (spec, part) = match self.spec_of(&idx) {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            let mut v = Vec::new();
            let (w, h) = match self.page().reader.read_sampled(&spec, step, part, &mut v)? {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            planes.push(Arc::new(NdPlane {
                width: w as u32,
                height: h as u32,
                values: v,
                truncated: false,
            }));
        }
        Some(Ok(science::auto_raster(&planes, None)))
    }
}

fn flip_rows(values: &mut [f32], w: usize, h: usize) {
    for y in 0..h / 2 {
        let (top, bottom) = values.split_at_mut((h - 1 - y) * w);
        top[y * w..(y + 1) * w].swap_with_slice(&mut bottom[..w]);
    }
}

/// The mean of each run of `bin` values (NaN ignored), `w` of them.
fn bin_means(v: &[f32], bin: usize, w: usize) -> Vec<f32> {
    if bin <= 1 {
        let mut r = v.to_vec();
        r.resize(w, f32::NAN);
        return r;
    }
    (0..w)
        .map(|c| {
            let s = &v[(c * bin).min(v.len())..((c + 1) * bin).min(v.len())];
            let (sum, n) = s
                .iter()
                .filter(|x| x.is_finite())
                .fold((0f64, 0u32), |(a, n), &x| (a + f64::from(x), n + 1));
            if n == 0 {
                f32::NAN
            } else {
                (sum / f64::from(n)) as f32
            }
        })
        .collect()
}

/// The last plot made: its row and plane, and the series.
type PlotCache = ((usize, Vec<u32>), Spectrum);

/// A series (a 1-D array, or one row of a page of series) plotted under the pointer.
struct LinePlot {
    reader: Arc<dyn PlaneRead>,
    shape: Vec<u64>,
    x: usize,
    /// The dimension of the series (rows), when there are several.
    y: Option<usize>,
    cal: Option<Linear>,
    axis: (String, String),
    grid: (usize, usize),
    part_axis: Option<usize>,
    rows: usize,
    flip: bool,
    cache: Mutex<Option<PlotCache>>,
}

impl std::fmt::Debug for LinePlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinePlot")
            .field("shape", &self.shape)
            .finish()
    }
}

impl LinePlot {
    fn x_of(&self, i: f64) -> f64 {
        self.cal.map_or(i, |c| c.at(i))
    }
}

impl SpectrumSource for LinePlot {
    fn grid(&self) -> (usize, usize) {
        self.grid
    }
    fn channels(&self) -> usize {
        (self.shape[self.x] as usize).min(MAX_PLOT)
    }
    fn x_axis(&self) -> (String, String) {
        self.axis.clone()
    }
    fn at(&self, _x: usize, y: usize, index: &[u32]) -> Option<Spectrum> {
        let row = match self.y {
            Some(_) if self.flip => self.rows.checked_sub(1 + y)?,
            Some(_) => y,
            None => 0,
        };
        let key = (row, index.to_vec());
        if let Some((k, s)) = &*self.cache.lock()
            && *k == key
        {
            return Some(s.clone());
        }
        let mut idx = vec![0u64; self.shape.len()];
        if let Some(yd) = self.y {
            idx[yd] = row as u64;
        }
        let part = self
            .part_axis
            .and_then(|k| index.get(k))
            .and_then(|&i| Part::ALL.get(i as usize).copied())
            .unwrap_or_default();
        let mut v = Vec::new();
        self.reader
            .read(
                &PlaneSpec {
                    x_dim: self.x,
                    y_dim: None,
                    index: idx,
                },
                part,
                &mut v,
            )
            .ok()?;
        let s = if v.len() <= MAX_PLOT {
            match self.cal {
                Some(c) => Spectrum {
                    y: v,
                    x: SpectrumX::Linear(c),
                    sticks: false,
                },
                None => Spectrum {
                    y: v,
                    x: SpectrumX::Linear(Linear {
                        offset: 0.0,
                        step: 1.0,
                    }),
                    sticks: false,
                },
            }
        } else {
            // Bin minima and maxima, in order, so peaks survive.
            let bins = MAX_PLOT / 2;
            let per = v.len().div_ceil(bins);
            let (mut xs, mut ys) = (Vec::new(), Vec::new());
            for b in 0..v.len().div_ceil(per) {
                let s = &v[b * per..((b + 1) * per).min(v.len())];
                let (mut lo, mut hi) = ((f32::INFINITY, 0usize), (f32::NEG_INFINITY, 0usize));
                for (k, &x) in s.iter().enumerate() {
                    if x < lo.0 {
                        lo = (x, k);
                    }
                    if x > hi.0 {
                        hi = (x, k);
                    }
                }
                if !lo.0.is_finite() {
                    continue;
                }
                let (a, b2) = if lo.1 <= hi.1 { (lo, hi) } else { (hi, lo) };
                for (val, k) in [a, b2] {
                    xs.push(self.x_of((b * per + k) as f64));
                    ys.push(val);
                }
            }
            Spectrum {
                y: ys,
                x: SpectrumX::Values(xs.into()),
                sticks: false,
            }
        };
        *self.cache.lock() = Some((key, s.clone()));
        Some(s)
    }
}

impl Session for ArraySession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode_sampled(&self, long_side: u32, cancel: &CancelToken) -> Option<Result<Raster>> {
        let want = u64::from(long_side.max(1)) * 2;
        let long = u64::from(self.info.width.max(self.info.height));
        let step = long / want;
        if step <= 1 {
            return None;
        }
        self.sampled(step, cancel)
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
        let planes = if self.summary.channel_axis.is_some() {
            3
        } else {
            1
        };
        px * 4 * planes + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes()
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
        let p = self.page();
        (self.shared.pages.len() > 1).then(|| PageLabel::Object {
            name: Arc::from(p.label.as_str()),
            class: Arc::from(p.class.as_str()),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.shared.pages.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(ArraySession::new(
            self.source.clone(),
            self.shared.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.summary.channel_axis.is_some() {
            "RGB"
        } else {
            "Greyscale"
        })
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

// ----- Contiguous arrays (npy, stored npz members, MATLAB variables) --------------------

/// Where an array's element bytes are.
enum Bytes {
    /// A range of the source file.
    InFile(Arc<Source>, std::ops::Range<usize>),
    /// Inflated on first use, cached while the session lives (with its memory reserved).
    Inflated(Arc<Inflate>),
    /// Bytes made at open (a densified sparse matrix).
    Owned(Arc<Vec<u8>>),
}

/// Inflates a member from the whole file.
type InflateFn = dyn Fn(&[u8]) -> specular_arrays::Result<Vec<u8>> + Send + Sync;

/// A compressed member, inflated once.
pub(crate) struct Inflate {
    source: Arc<Source>,
    size: u64,
    run: Box<InflateFn>,
    done: Mutex<Option<(Arc<Vec<u8>>, specular_core::budget::Reservation)>>,
}

impl Inflate {
    fn get(&self) -> Result<Arc<Vec<u8>>> {
        let mut g = self.done.lock();
        if let Some((b, _)) = &*g {
            return Ok(b.clone());
        }
        let r = specular_core::budget::reserve(self.size)?;
        let b = Arc::new((self.run)(self.source.bytes()).map_err(arr_err)?);
        *g = Some((b.clone(), r));
        Ok(b)
    }
}

/// A contiguous array: its element type, logical shape and element strides; a split complex
/// array (MATLAB) has a second one for the imaginary parts.
struct Contig {
    bytes: Bytes,
    /// Offset of the elements within `bytes`.
    start: usize,
    dtype: specular_arrays::Dtype,
    shape: Vec<u64>,
    strides: Vec<u64>,
    imag: Option<Box<Contig>>,
}

impl Contig {
    fn with<R>(&self, f: impl FnOnce(&[u8]) -> R) -> Result<R> {
        Ok(match &self.bytes {
            Bytes::InFile(s, r) => {
                let b = s.bytes().get(r.clone()).unwrap_or(&[]);
                f(b.get(self.start..).unwrap_or(&[]))
            }
            Bytes::Inflated(i) => {
                let b = i.get()?;
                f(b.get(self.start..).unwrap_or(&[]))
            }
            Bytes::Owned(b) => f(b.get(self.start..).unwrap_or(&[])),
        })
    }

    fn read_one(&self, spec: &PlaneSpec, part: Part, out: &mut Vec<f32>) -> Result<bool> {
        self.with(|b| {
            specular_arrays::read_plane(b, self.dtype, &self.shape, &self.strides, spec, part, out)
        })?
        .map_err(arr_err)
    }
}

impl PlaneRead for Contig {
    fn read(&self, spec: &PlaneSpec, part: Part, out: &mut Vec<f32>) -> Result<bool> {
        match &self.imag {
            None => self.read_one(spec, part, out),
            Some(im) => {
                let mut t = self.read_one(spec, Part::Real, out)?;
                let mut iv = Vec::new();
                t |= im.read_one(spec, Part::Real, &mut iv)?;
                specular_arrays::combine(out, &iv, part);
                Ok(t)
            }
        }
    }

    fn read_sampled(
        &self,
        spec: &PlaneSpec,
        step: u64,
        part: Part,
        out: &mut Vec<f32>,
    ) -> Option<Result<(usize, usize)>> {
        if self.imag.is_some() || matches!(self.bytes, Bytes::Inflated(_)) {
            return None;
        }
        let (w, h) = spec.size(&self.shape);
        let (sw, sh) = (
            (w as u64).div_ceil(step) as usize,
            (h as u64).div_ceil(step) as usize,
        );
        let base = spec.base(&self.strides);
        let sx = self.strides[spec.x_dim] * step;
        let sy = spec.y_dim.map_or(0, |d| self.strides[d] * step);
        out.clear();
        out.resize(sw * sh, 0.0);
        let r =
            self.with(|b| specular_arrays::gather_rows(b, self.dtype, base, sx, sy, sw, part, out));
        Some(r.map(|_| (sw, sh)))
    }
}

// ----- NumPy ------------------------------------------------------------------------------

fn npy_class(h: &npy::Npy) -> String {
    match &h.time_unit {
        Some(u) => format!("{} [{u}]", h.descr.trim_start_matches(['<', '>', '|', '='])),
        None => h.descr.trim_start_matches(['<', '>', '|', '=']).to_owned(),
    }
}

fn npy_page(label: String, h: &npy::Npy, bytes: Bytes) -> Page {
    let reader = Arc::new(Contig {
        bytes,
        start: h.data_offset,
        dtype: h.dtype,
        shape: h.shape.clone(),
        strides: h.strides(),
        imag: None,
    });
    let mut p = Page::new(label, h.shape.clone(), Convention::LastIsX, reader);
    p.class = npy_class(h);
    p.complex = h.dtype.elem.is_complex();
    p.rgb_ok = matches!(
        h.dtype.elem,
        specular_arrays::Elem::U8 | specular_arrays::Elem::U16
    );
    p.bits = h.dtype.elem.bits();
    p.sample = h.describe();
    p
}

fn shape_text(shape: &[u64]) -> String {
    if shape.is_empty() {
        return "scalar".into();
    }
    shape
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(" × ")
}

fn open_npy(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let h = npy::parse(data).map_err(arr_err)?;
    let mut shared = Shared::new(Format::Npy);
    shared.kw("NumPy format", format!("{}.{}", h.version.0, h.version.1));
    shared.kw("descr", h.descr.clone());
    shared.kw("shape", shape_text(&h.shape));
    if h.data_len() > (data.len() - h.data_offset.min(data.len())) as u64 {
        shared.kw("note", "the file is shorter than its header says");
    }
    if Page::showable(&h.shape) {
        let len = data.len();
        shared.pages.push(npy_page(
            String::new(),
            &h,
            Bytes::InFile(source.clone(), 0..len),
        ));
    } else if h.count() == 1 {
        let mut v = Vec::new();
        let _ = specular_arrays::read_plane(
            data.get(h.data_offset..).unwrap_or(&[]),
            h.dtype,
            &[1],
            &[1],
            &PlaneSpec {
                x_dim: 0,
                y_dim: None,
                index: vec![0],
            },
            Part::Real,
            &mut v,
        );
        if let Some(x) = v.first() {
            shared.kw("value", x.to_string());
        }
    }
    session(source, shared, "the array holds fewer than two values")
}

fn open_npz(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let z = Arc::new(npz::parse(data).map_err(arr_err)?);
    let mut shared = Shared::new(Format::Npz);
    shared.kw("arrays", z.members.len().to_string());
    for (i, m) in z.members.iter().enumerate() {
        let h = match &m.npy {
            Ok(h) => h,
            Err(why) => {
                shared.kw(m.name.clone(), format!("not shown: {why}"));
                continue;
            }
        };
        if !Page::showable(&h.shape) {
            shared.kw(
                m.name.clone(),
                format!("{} {}", npy_class(h), shape_text(&h.shape)),
            );
            continue;
        }
        if shared.pages.len() >= MAX_PAGES {
            break;
        }
        let bytes = if m.compressed() {
            let zz = z.clone();
            Bytes::Inflated(Arc::new(Inflate {
                source: source.clone(),
                size: m.entry.size,
                run: Box::new(move |d| zz.inflate(d, &zz.members[i], MAX_INFLATE)),
                done: Mutex::new(None),
            }))
        } else {
            let r = z.zip.data_range(data, &m.entry).map_err(arr_err)?;
            Bytes::InFile(source.clone(), r)
        };
        let mut p = npy_page(m.name.clone(), h, bytes);
        p.keywords.push((
            "compression".into(),
            if m.compressed() { "DEFLATE" } else { "stored" }.into(),
            String::new(),
        ));
        shared.pages.push(p);
    }
    session(source, shared, "the archive holds no arrays to show")
}

// ----- MATLAB -----------------------------------------------------------------------------

fn mat_page_keywords(v: &mat::Var) -> Vec<(String, String, String)> {
    let mut k = vec![("MATLAB class".to_owned(), v.describe(), String::new())];
    if v.global {
        k.push(("global".into(), "yes".into(), String::new()));
    }
    k
}

fn mat_dims(dims: &[u64]) -> String {
    dims.iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join("×")
}

fn mat_rgb_ok(v: &mat::Var) -> bool {
    !v.logical && matches!(v.class, mat::Class::UInt8 | mat::Class::UInt16)
}

fn open_mat(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    if mat::version(data) == Some(mat::Version::V73) {
        return open_mat73(source);
    }
    let m = Arc::new(mat::parse(data).map_err(arr_err)?);
    let mut shared = Shared::new(Format::Matlab);
    shared.kw(
        "MAT-file",
        match m.version {
            mat::Version::V4 => "level 4".to_owned(),
            _ => format!("level 5{}", if m.big { ", big-endian" } else { "" }),
        },
    );
    if !m.header.is_empty() {
        shared.kw("header", m.header.clone());
    }
    for n in &m.notes {
        shared.kw("note", n.clone());
    }
    // One inflater per compressed variable, shared by its elements.
    let inflaters: Vec<Arc<Inflate>> = (0..m.compressed.len())
        .map(|i| {
            let mm = m.clone();
            Arc::new(Inflate {
                source: source.clone(),
                size: m.compressed[i].len as u64,
                run: Box::new(move |d| mm.inflate(d, i, MAX_INFLATE as usize)),
                done: Mutex::new(None),
            })
        })
        .collect();
    let bytes_of = |s: &mat::Span| match s.buf {
        None => Bytes::InFile(source.clone(), 0..data.len()),
        Some(b) => Bytes::Inflated(inflaters[b].clone()),
    };
    for v in &m.vars {
        if shared.pages.len() >= MAX_PAGES {
            break;
        }
        let label = v.path.clone();
        match &v.kind {
            MatKind::Numeric { re, im } => {
                if !Page::showable(&v.dims) {
                    if v.count() == 1 {
                        let mut out = Vec::new();
                        let r = match re.buf {
                            None => m.read_plane(
                                data,
                                None,
                                v,
                                &PlaneSpec {
                                    x_dim: 1,
                                    y_dim: Some(0),
                                    index: vec![0; v.dims.len()],
                                },
                                Part::Magnitude,
                                &mut out,
                            ),
                            Some(b) => inflaters[b]
                                .get()
                                .map_err(|_| specular_arrays::Error::TooLarge)
                                .and_then(|buf| {
                                    m.read_plane(
                                        data,
                                        Some(&buf),
                                        v,
                                        &PlaneSpec {
                                            x_dim: 1,
                                            y_dim: Some(0),
                                            index: vec![0; v.dims.len()],
                                        },
                                        Part::Magnitude,
                                        &mut out,
                                    )
                                }),
                        };
                        if r.is_ok()
                            && let Some(x) = out.first()
                        {
                            shared.kw(label, x.to_string());
                        }
                    }
                    continue;
                }
                let strides = specular_arrays::strides(&v.dims, true);
                let contig = |s: &mat::Span| Contig {
                    bytes: bytes_of(s),
                    start: s.range.start,
                    dtype: s.dtype,
                    shape: v.dims.clone(),
                    strides: strides.clone(),
                    imag: None,
                };
                let mut c = contig(re);
                if let Some(im) = im {
                    c.imag = Some(Box::new(contig(im)));
                }
                let mut p = Page::new(label, v.dims.clone(), Convention::Matlab, Arc::new(c));
                p.class = format!("{} {}", v.describe(), mat_dims(&v.dims));
                p.complex = im.is_some();
                p.rgb_ok = mat_rgb_ok(v);
                p.bits = if v.logical { 1 } else { v.class.bits() };
                p.sample = v.describe();
                p.keywords = mat_page_keywords(v);
                shared.pages.push(p);
            }
            MatKind::Sparse { .. } | MatKind::Triplets { .. } => {
                let (rows, cols) = (v.dims[0], v.dims.get(1).copied().unwrap_or(1));
                let n = rows.saturating_mul(cols);
                if n < 2 {
                    continue;
                }
                // Densified at open: its memory counts against the budget first.
                let bytes = n.saturating_mul(8);
                let _r = specular_core::budget::reserve(bytes)?;
                let buf = match re_buf(&v.kind) {
                    Some(b) => Some(inflaters[b].get()?),
                    None => None,
                };
                let mk = |part: Part| -> Result<Vec<u8>> {
                    let d = m
                        .sparse_dense(data, buf.as_deref().map(|b| b.as_slice()), v, part)
                        .map_err(arr_err)?;
                    Ok(d.iter().flat_map(|x| x.to_le_bytes()).collect())
                };
                let shape = vec![rows, cols];
                let strides = specular_arrays::strides(&shape, true);
                let real = Contig {
                    bytes: Bytes::Owned(Arc::new(mk(Part::Real)?)),
                    start: 0,
                    dtype: specular_arrays::Dtype::le(specular_arrays::Elem::F32),
                    shape: shape.clone(),
                    strides: strides.clone(),
                    imag: if v.complex {
                        Some(Box::new(Contig {
                            bytes: Bytes::Owned(Arc::new(mk(Part::Imaginary)?)),
                            start: 0,
                            dtype: specular_arrays::Dtype::le(specular_arrays::Elem::F32),
                            shape: shape.clone(),
                            strides: strides.clone(),
                            imag: None,
                        }))
                    } else {
                        None
                    },
                };
                let mut p = Page::new(label, shape, Convention::Matlab, Arc::new(real));
                p.class = format!("{} {}", v.describe(), mat_dims(&v.dims));
                p.complex = v.complex;
                p.bits = 64;
                p.sample = format!("{}, shown dense", v.describe());
                p.keywords = mat_page_keywords(v);
                shared.pages.push(p);
            }
            MatKind::Text(t) => shared.kw(label, t.replace('\n', " / ")),
            MatKind::Refused(why) => shared.kw(label, format!("not shown: {why}")),
            MatKind::Container | MatKind::Empty => {}
        }
    }
    session(source, shared, "the file holds no arrays to show")
}

fn re_buf(k: &MatKind) -> Option<usize> {
    match k {
        MatKind::Sparse { re, .. } => re.buf,
        _ => None,
    }
}

// ----- MATLAB v7.3 (HDF5) -----------------------------------------------------------------

struct H5Var {
    file: Arc<hdf5::File>,
    source: Arc<Source>,
    index: usize,
    /// HDF5 dimensions (MATLAB's reversed).
    dims: Vec<u64>,
    /// A compound {real, imag} of this float type.
    complex: Option<specular_arrays::Elem>,
}

impl PlaneRead for H5Var {
    fn read(&self, spec: &PlaneSpec, part: Part, out: &mut Vec<f32>) -> Result<bool> {
        let nd = self.dims.len();
        let h5 = |d: usize| nd - 1 - d;
        // MATLAB arrays have two dimensions or more: a vector's other one has length 1.
        let y = spec.y_dim.unwrap_or_else(|| {
            (0..nd)
                .find(|&d| d != spec.x_dim && self.dims[h5(d)] == 1)
                .unwrap_or(0)
        });
        let data = self.source.bytes();
        let cancel = || false;
        let index: Vec<u64> = (0..nd)
            .map(|k| spec.index.get(nd - 1 - k).copied().unwrap_or(0))
            .collect();
        let (w, h) = (self.dims[h5(spec.x_dim)], self.dims[h5(y)]);
        match self.complex {
            None => {
                let pl = self
                    .file
                    .read_plane::<f32>(
                        data,
                        self.index,
                        &hdf5::PlaneSpec {
                            x_dim: h5(spec.x_dim),
                            y_dim: h5(y),
                            index,
                        },
                        &cancel,
                    )
                    .map_err(crate::hdf5_dec::map_hdf5)?;
                *out = pl.values;
                Ok(pl.truncated)
            }
            Some(elem) => {
                // A box over the plane, then the members split.
                let mut start = index.clone();
                let mut count = vec![1u64; nd];
                start[h5(spec.x_dim)] = 0;
                start[h5(y)] = 0;
                count[h5(spec.x_dim)] = w;
                count[h5(y)] = h;
                let raw = self
                    .file
                    .read_box_bytes(data, self.index, &start, &count, &cancel)
                    .map_err(crate::hdf5_dec::map_hdf5)?;
                let size = elem.size();
                let n = (w * h) as usize;
                out.clear();
                out.resize(n, f32::NAN);
                // Row-major over (count): the x dimension is h5(x), y is h5(y).
                let (xs, ys) = if h5(spec.x_dim) > h5(y) {
                    (1usize, w as usize)
                } else {
                    (h as usize, 1usize)
                };
                for r in 0..h as usize {
                    for c in 0..w as usize {
                        let k = r * ys + c * xs;
                        let Some(b) = raw.get(k * 2 * size..(k + 1) * 2 * size) else {
                            continue;
                        };
                        let (re, im) = if size == 8 {
                            (
                                f64::from_le_bytes(b[..8].try_into().unwrap_or_default()),
                                f64::from_le_bytes(b[8..16].try_into().unwrap_or_default()),
                            )
                        } else {
                            (
                                f64::from(f32::from_le_bytes(
                                    b[..4].try_into().unwrap_or_default(),
                                )),
                                f64::from(f32::from_le_bytes(
                                    b[4..8].try_into().unwrap_or_default(),
                                )),
                            )
                        };
                        out[r * w as usize + c] = part.of(re, im);
                    }
                }
                Ok(false)
            }
        }
    }
}

fn attr_str(o: &hdf5::Object, name: &str) -> Option<String> {
    match o.attr(name)? {
        hdf5::Value::Str(s) => s.first().cloned(),
        _ => None,
    }
}

fn open_mat73(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let file = Arc::new(hdf5::File::open(data).map_err(crate::hdf5_dec::map_hdf5)?);
    let mut shared = Shared::new(Format::Matlab);
    shared.kw("MAT-file", "7.3 (HDF5)");
    let head = data.get(..116).unwrap_or(&[]);
    let end = head.iter().position(|&b| b == 0).unwrap_or(head.len());
    shared.kw(
        "header",
        String::from_utf8_lossy(&head[..end]).trim().to_owned(),
    );
    let mut stack: Vec<(String, String, usize)> = Vec::new();
    // Top-level variables: the root's children, not MATLAB's own groups.
    let mut roots: Vec<&hdf5::Object> = file
        .children("/")
        .filter(|o| !o.name().starts_with('#'))
        .collect();
    roots.sort_by(|a, b| a.path.cmp(&b.path));
    for o in roots.iter().rev() {
        stack.push((o.path.clone(), o.name().to_owned(), 0));
    }
    let mut seen = 0usize;
    while let Some((path, label, depth)) = stack.pop() {
        seen += 1;
        if seen > 100_000 || shared.pages.len() >= MAX_PAGES || depth > 32 {
            break;
        }
        let Some(o) = file.get(&path) else { continue };
        let class = attr_str(o, "MATLAB_class").unwrap_or_default();
        match &o.kind {
            hdf5::Kind::Group => {
                if let Some(rows) = o.attr("MATLAB_sparse").and_then(|v| v.as_f64()) {
                    match sparse73(&file, data, &path, rows as u64, &class) {
                        Ok(Some(p)) => {
                            let mut p = p;
                            p.label = label;
                            shared.pages.push(p);
                        }
                        Ok(None) => {}
                        Err(e) => shared.kw(label, format!("not shown: {e}")),
                    }
                    continue;
                }
                if class != "struct" && !class.is_empty() {
                    shared.kw(
                        label,
                        format!("not shown: MATLAB {class} objects are not supported"),
                    );
                    continue;
                }
                let mut kids: Vec<&hdf5::Object> = file.children(&path).collect();
                kids.sort_by(|a, b| a.path.cmp(&b.path));
                for k in kids.iter().rev() {
                    stack.push((k.path.clone(), format!("{label}.{}", k.name()), depth + 1));
                }
            }
            hdf5::Kind::Dataset(ds) => {
                let h5dims = ds.shape().to_vec();
                let dims: Vec<u64> = h5dims.iter().rev().copied().collect();
                if o.attr("MATLAB_empty")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
                    != 0.0
                {
                    continue;
                }
                let idx = file.index_of(&path).unwrap_or(usize::MAX);
                match (&ds.dtype, class.as_str()) {
                    (hdf5::Dtype::Ref { .. }, _) => {
                        // A cell array (or a struct array's field): its elements by reference.
                        let n: u64 = h5dims.iter().product();
                        let cancel = || false;
                        let raw = file
                            .read_box_bytes(data, idx, &vec![0; h5dims.len()], &h5dims, &cancel)
                            .unwrap_or_default();
                        let refs: Vec<u64> = raw
                            .chunks_exact(8)
                            .map(|c| u64::from_le_bytes(c.try_into().unwrap_or_default()))
                            .collect();
                        let mut items = Vec::new();
                        for (k, &addr) in refs.iter().enumerate().take(n as usize) {
                            let Some(t) = file.at_addr(addr) else {
                                continue;
                            };
                            // HDF5 row-major over reversed dims = MATLAB column-major order.
                            let sub = mat_subscript(&dims, k as u64);
                            let l = if class == "cell" {
                                format!("{label}{{{sub}}}")
                            } else {
                                format!("{label}({sub})")
                            };
                            items.push((t.path.clone(), l, depth + 1));
                        }
                        for it in items.into_iter().rev() {
                            stack.push(it);
                        }
                    }
                    (hdf5::Dtype::Num { .. }, "char") => {
                        let v = file.read_all::<f32>(data, idx, 1 << 16).unwrap_or_default();
                        let rows = dims.first().copied().unwrap_or(1).max(1) as usize;
                        let cols = v.len() / rows;
                        // HDF5 row-major [cols][rows]: element (r, c) at c * rows + r.
                        let text: Vec<String> = (0..rows)
                            .map(|r| {
                                (0..cols)
                                    .filter_map(|c| char::from_u32(v[c * rows + r] as u32))
                                    .collect::<String>()
                                    .trim_end()
                                    .to_owned()
                            })
                            .collect();
                        shared.kw(label, text.join(" / "));
                    }
                    (dt, c) => {
                        let complex = match dt {
                            hdf5::Dtype::Other { class: 6, size: 16 } => {
                                Some(specular_arrays::Elem::F64)
                            }
                            hdf5::Dtype::Other { class: 6, size: 8 } => {
                                Some(specular_arrays::Elem::F32)
                            }
                            hdf5::Dtype::Num { .. } => None,
                            _ => {
                                shared.kw(label, "not shown: a type that is not numeric");
                                continue;
                            }
                        };
                        if c == "function_handle"
                            || attr_str(o, "MATLAB_object_decode").is_some()
                            || (!c.is_empty() && mat::Class::from_name(c).is_none())
                        {
                            shared.kw(
                                label,
                                format!("not shown: MATLAB {c} objects are not supported"),
                            );
                            continue;
                        }
                        if !Page::showable(&dims) {
                            if let Ok(v) = file.read_all::<f32>(data, idx, 1)
                                && let Some(x) = v.first()
                            {
                                shared.kw(label, x.to_string());
                            }
                            continue;
                        }
                        let num = match dt {
                            hdf5::Dtype::Num { num, .. } => Some(*num),
                            _ => None,
                        };
                        let reader = Arc::new(H5Var {
                            file: file.clone(),
                            source: source.clone(),
                            index: idx,
                            dims: h5dims.clone(),
                            complex,
                        });
                        let mut p =
                            Page::new(label.clone(), dims.clone(), Convention::Matlab, reader);
                        let logical = c == "logical";
                        let cname = if c.is_empty() {
                            "double".to_owned()
                        } else {
                            c.to_owned()
                        };
                        let describe = if complex.is_some() {
                            format!("complex {cname}")
                        } else {
                            cname.clone()
                        };
                        p.class = format!("{describe} {}", mat_dims(&dims));
                        p.complex = complex.is_some();
                        p.rgb_ok = !logical && matches!(num, Some(hdf5::Num::U8 | hdf5::Num::U16));
                        p.bits = if logical {
                            1
                        } else {
                            num.map_or(64, |n| n.size() as u32 * 8)
                        };
                        p.sample = describe.clone();
                        let filters = ds.filter_names();
                        p.keywords = vec![
                            ("MATLAB class".into(), describe, String::new()),
                            ("HDF5 path".into(), path.clone(), String::new()),
                            (
                                "storage".into(),
                                if filters.is_empty() {
                                    ds.storage().to_owned()
                                } else {
                                    format!("{}, {}", ds.storage(), filters.join(", "))
                                },
                                String::new(),
                            ),
                        ];
                        shared.pages.push(p);
                    }
                }
            }
            _ => {}
        }
    }
    session(source, shared, "the file holds no arrays to show")
}

/// A v7.3 sparse matrix (a group with `data`, `ir` and `jc` and the row count in
/// `MATLAB_sparse`) densified within the memory budget; `None` when it has fewer than two
/// values.
fn sparse73(
    file: &Arc<hdf5::File>,
    data: &[u8],
    path: &str,
    rows: u64,
    class: &str,
) -> Result<Option<Page>> {
    let idx = |n: &str| {
        file.index_of(&format!("{path}/{n}"))
            .ok_or_else(|| DecodeError::Malformed(format!("sparse matrix without {n}")))
    };
    let jc = file
        .read_all::<f64>(data, idx("jc")?, 1 << 32)
        .map_err(crate::hdf5_dec::map_hdf5)?;
    let cols = (jc.len() as u64).saturating_sub(1);
    let n = rows.saturating_mul(cols);
    if n < 2 {
        return Ok(None);
    }
    if n > specular_arrays::MAX_PLANE {
        return Err(DecodeError::TooLarge);
    }
    let _r = specular_core::budget::reserve(n.saturating_mul(8))?;
    let nnz = jc.last().copied().unwrap_or(0.0).max(0.0) as usize;
    let (ir, vals): (Vec<f64>, Vec<(f64, f64)>) = if nnz == 0 {
        (Vec::new(), Vec::new())
    } else {
        let ir = file
            .read_all::<f64>(data, idx("ir")?, nnz as u64)
            .map_err(crate::hdf5_dec::map_hdf5)?;
        let di = idx("data")?;
        let ds = file.objects[di]
            .dataset()
            .ok_or_else(|| DecodeError::Malformed("sparse data".into()))?;
        let vals = match &ds.dtype {
            hdf5::Dtype::Other { class: 6, size } if *size == 16 || *size == 8 => {
                let shape = ds.shape().to_vec();
                let raw = file
                    .read_box_bytes(data, di, &vec![0; shape.len()], &shape, &|| false)
                    .map_err(crate::hdf5_dec::map_hdf5)?;
                let h = size / 2;
                raw.chunks_exact(*size)
                    .map(|b| {
                        if h == 8 {
                            (
                                f64::from_le_bytes(b[..8].try_into().unwrap_or_default()),
                                f64::from_le_bytes(b[8..].try_into().unwrap_or_default()),
                            )
                        } else {
                            (
                                f64::from(f32::from_le_bytes(
                                    b[..4].try_into().unwrap_or_default(),
                                )),
                                f64::from(f32::from_le_bytes(
                                    b[4..].try_into().unwrap_or_default(),
                                )),
                            )
                        }
                    })
                    .collect()
            }
            _ => file
                .read_all::<f64>(data, di, nnz as u64)
                .map_err(crate::hdf5_dec::map_hdf5)?
                .into_iter()
                .map(|v| (v, 0.0))
                .collect(),
        };
        (ir, vals)
    };
    let complex = vals.iter().any(|v| v.1 != 0.0)
        || matches!(
            file.objects[idx("data")?].dataset().map(|d| &d.dtype),
            Some(hdf5::Dtype::Other { class: 6, .. })
        );
    let (mut re, mut im) = (vec![0f32; n as usize], vec![0f32; n as usize]);
    for c in 0..cols as usize {
        let (a, b) = (jc[c].max(0.0) as usize, jc[c + 1].max(0.0) as usize);
        for k in a..b.min(ir.len()).min(vals.len()) {
            let r = ir[k] as u64;
            if r < rows {
                let at = c * rows as usize + r as usize;
                re[at] = vals[k].0 as f32;
                im[at] = vals[k].1 as f32;
            }
        }
    }
    let shape = vec![rows, cols];
    let strides = specular_arrays::strides(&shape, true);
    let f32le = specular_arrays::Dtype::le(specular_arrays::Elem::F32);
    let bytes =
        |v: Vec<f32>| Bytes::Owned(Arc::new(v.iter().flat_map(|x| x.to_le_bytes()).collect()));
    let reader = Contig {
        bytes: bytes(re),
        start: 0,
        dtype: f32le,
        shape: shape.clone(),
        strides: strides.clone(),
        imag: complex.then(|| {
            Box::new(Contig {
                bytes: bytes(im),
                start: 0,
                dtype: f32le,
                shape: shape.clone(),
                strides: strides.clone(),
                imag: None,
            })
        }),
    };
    let cname = if class.is_empty() { "double" } else { class };
    let describe = format!("sparse {}{cname}", if complex { "complex " } else { "" });
    let mut p = Page::new(
        String::new(),
        shape.clone(),
        Convention::Matlab,
        Arc::new(reader),
    );
    p.class = format!("{describe} {}", mat_dims(&shape));
    p.complex = complex;
    p.bits = 64;
    p.sample = format!("{describe}, shown dense");
    p.keywords = vec![
        ("MATLAB class".into(), describe, String::new()),
        ("HDF5 path".into(), path.to_owned(), String::new()),
        ("stored values".into(), nnz.to_string(), String::new()),
    ];
    Ok(Some(p))
}

/// MATLAB's 1-based subscript of linear index `i` (column-major) of an array of `dims`.
fn mat_subscript(dims: &[u64], i: u64) -> String {
    if dims.iter().filter(|&&d| d != 1).count() <= 1 {
        return (i + 1).to_string();
    }
    let mut r = i;
    let mut parts = Vec::new();
    for &d in dims {
        let d = d.max(1);
        parts.push((r % d + 1).to_string());
        r /= d;
    }
    parts.join(",")
}

// ----- Zarr -------------------------------------------------------------------------------

struct ZarrRead {
    zarr: Arc<zarr::Zarr>,
    index: usize,
}

impl PlaneRead for ZarrRead {
    fn read(&self, spec: &PlaneSpec, part: Part, out: &mut Vec<f32>) -> Result<bool> {
        self.zarr
            .read_plane(&self.zarr.arrays[self.index], spec, part, out)
            .map_err(arr_err)
    }
}

/// The source's bytes as a zipped store (mapped files are read in place).
struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

fn json_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        v => v.to_string(),
    }
}

/// Opens a plain Zarr store: a folder, a zipped store, or the folder of a metadata file.
pub(crate) fn open_zarr(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let store: Arc<dyn zarr::Store> = if source.is_folder() {
        Arc::new(zarr::DirStore::new(source.path()))
    } else if source.bytes().starts_with(b"PK") {
        Arc::new(zarr::ZipStore::new(SourceBytes(source.clone())).map_err(arr_err)?)
    } else {
        return Err(DecodeError::UnsupportedFormat);
    };
    open_zarr_store(source, store, None)
}

/// A Zarr store's arrays as pages; `location` (a remote store's address) goes to the info
/// panel.
pub(crate) fn open_zarr_store(
    source: Arc<Source>,
    store: Arc<dyn zarr::Store>,
    location: Option<&str>,
) -> Result<Arc<dyn Session>> {
    let z = Arc::new(zarr::open(store).map_err(arr_err)?);
    let mut shared = Shared::new(Format::Zarr);
    if let Some(l) = location {
        shared.kw("location", l.to_owned());
    }
    shared.kw("Zarr format", z.zarr_format.to_string());
    if z.consolidated {
        shared.kw("metadata", "consolidated");
    }
    if let serde_json::Value::Object(a) = &z.attrs {
        for (k, v) in a.iter().take(256) {
            shared.kw(k.clone(), json_text(v));
        }
    }
    for (p, why) in &z.refused {
        shared.kw(
            if p.is_empty() {
                "array".to_owned()
            } else {
                p.clone()
            },
            format!("not shown: {why}"),
        );
    }
    for (i, a) in z.arrays.iter().enumerate() {
        if !Page::showable(&a.shape) {
            shared.kw(
                if a.path.is_empty() {
                    "array".to_owned()
                } else {
                    a.path.clone()
                },
                format!("{} {}", a.dtype_name, shape_text(&a.shape)),
            );
            continue;
        }
        if shared.pages.len() >= MAX_PAGES {
            break;
        }
        let reader = Arc::new(ZarrRead {
            zarr: z.clone(),
            index: i,
        });
        let mut p = Page::new(a.path.clone(), a.shape.clone(), Convention::LastIsX, reader);
        p.class = format!("{} {}", a.dtype_name, shape_text(&a.shape));
        p.complex = a.dtype.elem.is_complex();
        p.rgb_ok = matches!(
            a.dtype.elem,
            specular_arrays::Elem::U8 | specular_arrays::Elem::U16
        );
        p.bits = a.dtype.elem.bits();
        p.sample = format!(
            "{}{}",
            a.dtype.elem.describe(),
            if a.dtype.size() > 1 && a.dtype.big {
                ", big-endian"
            } else {
                ""
            }
        );
        p.dim_names = a.dim_names.clone();
        p.dim_names.resize(a.shape.len(), None);
        p.keywords
            .push(("chunks".into(), shape_text(&a.chunks), String::new()));
        p.keywords
            .push(("compression".into(), a.compression(), String::new()));
        p.keywords
            .push(("fill value".into(), a.fill.0.to_string(), String::new()));
        if let serde_json::Value::Object(at) = &a.attrs {
            for (k, v) in at.iter().take(256) {
                if k == "_ARRAY_DIMENSIONS" {
                    continue;
                }
                if k == "units" {
                    p.unit = Some(json_text(v));
                }
                if k == "long_name" || k == "title" {
                    p.title = Some(json_text(v));
                }
                p.keywords
                    .push((k.clone(), clip(json_text(v)), String::new()));
            }
        }
        shared.pages.push(p);
    }
    session(source, shared, "the Zarr store holds no arrays to show")
}

// ----- Decoder ----------------------------------------------------------------------------

/// Which format a decoder reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Npy,
    Npz,
    Mat,
    /// A Zarr metadata document (`.zarray`, `.zgroup`, `zarr.json`) opened on its own: the
    /// store is its folder.
    ZarrMeta,
}

pub struct ArrayDecoder(pub Kind);

impl Decoder for ArrayDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Npy => Format::Npy,
            Kind::Npz => Format::Npz,
            Kind::Mat => Format::Matlab,
            Kind::ZarrMeta => Format::Zarr,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Npy => npy::sniff(head),
            Kind::Npz => npz::sniff(head),
            // The first bytes give the header text; `probe_file` checks the byte-order mark.
            Kind::Mat => head.starts_with(b"MATLAB ") && mat::sniff_text(head),
            // `zarr_format` may come last: see `probe_file`.
            Kind::ZarrMeta => false,
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Kind::Mat => mat::sniff(&data[..data.len().min(128)]),
            Kind::ZarrMeta => zarr::is_metadata_head(&data[..data.len().min(4096)]),
            _ => self.probe(&data[..data.len().min(crate::PROBE_HEAD)]),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Npy => &["npy"],
            Kind::Npz => &["npz"],
            // Level 4 files have no signature: opened by extension.
            Kind::Mat => &["mat"],
            Kind::ZarrMeta => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        match self.0 {
            Kind::Npy => open_npy(source),
            Kind::Npz => open_npz(source),
            Kind::Mat => open_mat(source),
            Kind::ZarrMeta => {
                let dir = source
                    .path()
                    .parent()
                    .filter(|d| zarr::is_zarr_dir(d))
                    .ok_or(DecodeError::UnsupportedFormat)?;
                let folder = Source::open(dir)?;
                if !folder.is_folder() {
                    return Err(DecodeError::UnsupportedFormat);
                }
                crate::wsi_dec::open_zarr_any(Arc::new(folder))
            }
        }
    }
}

/// Whether `path` is a Zarr metadata file the viewer may list and open (`.zarray` or
/// `zarr.json` inside a Zarr folder).
pub fn is_zarr_metadata_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some(".zarray" | "zarr.json")
    ) && path.parent().is_some_and(zarr::is_zarr_dir)
        && path.is_file()
}

#[cfg(test)]
mod tests;
