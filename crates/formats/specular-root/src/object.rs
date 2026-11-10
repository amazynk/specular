//! The classes read from an object's bytes: histograms (TH1, TH2, TH3 and their storage
//! types, TProfile, TProfile2D) with their axes, graphs, and the head of a TTree.
//!
//! Each class version is checked against the versions whose layout this reader knows; parts
//! that are not needed (attributes, functions, buffers) are skipped by their byte counts.

use crate::buf::{Any, Cur, Storage};
use crate::{Error, Result};

/// One histogram axis.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Axis {
    pub name: String,
    pub title: String,
    pub nbins: usize,
    pub min: f64,
    pub max: f64,
    /// The nbins + 1 bin edges of a variable-width axis; empty when the bins are equal.
    pub edges: Vec<f64>,
    /// Alphanumeric bin labels: (bin, 1-based, label).
    pub labels: Vec<(usize, String)>,
}

impl Axis {
    pub fn is_variable(&self) -> bool {
        !self.edges.is_empty()
    }

    /// The low edge of bin `i` (1-based; `nbins + 1` gives the upper edge of the last bin).
    pub fn edge(&self, i: usize) -> f64 {
        if self.is_variable() {
            let k = i.saturating_sub(1).min(self.edges.len() - 1);
            self.edges[k]
        } else {
            self.min + (self.max - self.min) * (i.saturating_sub(1) as f64) / self.nbins as f64
        }
    }

    pub fn centre(&self, i: usize) -> f64 {
        0.5 * (self.edge(i) + self.edge(i + 1))
    }

    pub fn width(&self, i: usize) -> f64 {
        self.edge(i + 1) - self.edge(i)
    }

    /// The label of bin `i`, if any.
    pub fn label(&self, i: usize) -> Option<&str> {
        self.labels
            .iter()
            .find(|(b, _)| *b == i)
            .map(|(_, s)| s.as_str())
    }

    /// The unit in the title: `x [cm]` or `E (GeV)` gives `cm` or `GeV`.
    pub fn unit(&self) -> Option<&str> {
        split_unit(&self.title).1
    }

    /// The title without its unit.
    pub fn quantity(&self) -> &str {
        split_unit(&self.title).0
    }
}

/// `p_{T} [GeV]` gives (`p_{T}`, `GeV`); the last bracketed (or failing that, parenthesised)
/// group at the end of the title is the unit.
pub fn split_unit(title: &str) -> (&str, Option<&str>) {
    let t = title.trim_end();
    for (open, close) in [('[', ']'), ('(', ')')] {
        if let Some(body) = t.strip_suffix(close)
            && let Some(at) = body.rfind(open)
        {
            let unit = body[at + 1..].trim();
            let head = body[..at].trim_end();
            if !unit.is_empty() && unit.len() <= 24 && !head.is_empty() {
                return (head, Some(unit));
            }
        }
    }
    (t, None)
}

/// TH1, TH2 or TH3 (any storage) or a profile.
#[derive(Clone, Debug, Default)]
pub struct Hist {
    pub class: String,
    pub name: String,
    pub title: String,
    /// 1, 2 or 3.
    pub dims: usize,
    pub storage: Option<Storage>,
    /// TProfile or TProfile2D: `cells` hold the mean per bin (sum of w·y over sum of w,
    /// zero where nothing was filled), `bin_entries` the sum of weights per bin.
    pub profile: bool,
    pub axes: Vec<Axis>,
    /// Every cell with the under- and overflow bins, x fastest: (nx + 2)(ny + 2)(nz + 2).
    pub cells: Vec<f64>,
    /// The sum of squared weights per cell, when stored.
    pub sumw2: Vec<f64>,
    pub bin_entries: Vec<f64>,
    pub entries: f64,
    /// The stored statistics (fTsumw, fTsumw2, fTsumwx, ...) by member name.
    pub sums: Vec<(&'static str, f64)>,
    /// fMaximum and fMinimum (-1111 when not set).
    pub maximum: f64,
    pub minimum: f64,
    /// TProfile's fErrorMode.
    pub error_mode: i32,
}

/// The value ROOT stores for an unset fMaximum or fMinimum.
pub const UNSET: f64 = -1111.0;

impl Hist {
    pub fn sum(&self, member: &str) -> Option<f64> {
        self.sums
            .iter()
            .find(|(m, _)| *m == member)
            .map(|&(_, v)| v)
    }

    /// The number of cells along axis `k` with flow (nbins + 2; 1 for missing axes).
    pub fn stride_len(&self, k: usize) -> usize {
        self.axes.get(k).map_or(1, |a| a.nbins + 2)
    }

    /// Cell (ix, iy, iz), indices with flow (0 underflow, nbins + 1 overflow).
    pub fn cell(&self, ix: usize, iy: usize, iz: usize) -> f64 {
        let nx = self.stride_len(0);
        let ny = self.stride_len(1);
        self.cells
            .get(ix + nx * (iy + ny * iz))
            .copied()
            .unwrap_or(f64::NAN)
    }

    /// The mean and RMS along axis `k` from the stored sums (fTsumwx / fTsumw, ...).
    pub fn mean_rms(&self, k: usize) -> Option<(f64, f64)> {
        let c = ["x", "y", "z"].get(k)?;
        let (s1, s2) = match *c {
            "x" => ("fTsumwx", "fTsumwx2"),
            "y" => ("fTsumwy", "fTsumwy2"),
            _ => ("fTsumwz", "fTsumwz2"),
        };
        let w = self.sum("fTsumw")?;
        if w == 0.0 || !w.is_finite() {
            return None;
        }
        let m = self.sum(s1)? / w;
        let var = self.sum(s2)? / w - m * m;
        Some((m, var.max(0.0).sqrt()))
    }

    /// The number of under- and overflow cells that hold something (any axis out of range),
    /// and their total.
    pub fn flow(&self) -> (usize, f64) {
        let (nx, ny, nz) = (self.stride_len(0), self.stride_len(1), self.stride_len(2));
        let mut n = 0;
        let mut sum = 0.0;
        let edge = |i: usize, len: usize| len > 1 && (i == 0 || i == len - 1);
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    if !(edge(ix, nx) || edge(iy, ny) || edge(iz, nz)) {
                        continue;
                    }
                    let v = self.cells[ix + nx * (iy + ny * iz)];
                    if v != 0.0 {
                        n += 1;
                        sum += v;
                    }
                }
            }
        }
        (n, sum)
    }

    /// The range of the in-range cells (no flow), ignoring non-finite values.
    pub fn range(&self) -> Option<(f64, f64)> {
        let (nx, ny, nz) = (self.stride_len(0), self.stride_len(1), self.stride_len(2));
        let inner = |len: usize| if len > 1 { 1..len - 1 } else { 0..1 };
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for iz in inner(nz) {
            for iy in inner(ny) {
                for ix in inner(nx) {
                    let v = self.cells[ix + nx * (iy + ny * iz)];
                    if v.is_finite() {
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
        }
        (lo <= hi).then_some((lo, hi))
    }
}

/// TGraph (and its error variants, read for their points) or TGraph2D.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub class: String,
    pub name: String,
    pub title: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    /// TGraph2D only.
    pub z: Vec<f64>,
}

impl Graph {
    /// A TGraph2D whose points fill a regular grid exactly once each, as a TH2D-like
    /// histogram (no flow content); `None` for scattered points.
    pub fn to_grid(&self) -> Option<Hist> {
        if self.z.is_empty() || self.x.len() != self.z.len() || self.y.len() != self.z.len() {
            return None;
        }
        let uniq = |v: &[f64]| {
            let mut u: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
            u.sort_by(f64::total_cmp);
            u.dedup();
            u
        };
        let xs = uniq(&self.x);
        let ys = uniq(&self.y);
        if xs.len() < 2 || ys.len() < 2 || xs.len() * ys.len() != self.z.len() {
            return None;
        }
        let step = |u: &[f64]| {
            let d = (u[u.len() - 1] - u[0]) / (u.len() - 1) as f64;
            u.windows(2)
                .all(|w| ((w[1] - w[0]) - d).abs() <= 1e-6 * d.abs().max(1e-300))
                .then_some(d)
        };
        let (dx, dy) = (step(&xs)?, step(&ys)?);
        let (nx, ny) = (xs.len(), ys.len());
        let mut cells = vec![0.0; (nx + 2) * (ny + 2)];
        let mut seen = vec![false; nx * ny];
        for k in 0..self.z.len() {
            let ix = ((self.x[k] - xs[0]) / dx).round() as usize;
            let iy = ((self.y[k] - ys[0]) / dy).round() as usize;
            if ix >= nx || iy >= ny || std::mem::replace(&mut seen[ix + nx * iy], true) {
                return None;
            }
            cells[(ix + 1) + (nx + 2) * (iy + 1)] = self.z[k];
        }
        let axis = |name: &str, u: &[f64], d: f64| Axis {
            name: name.into(),
            nbins: u.len(),
            min: u[0] - d / 2.0,
            max: u[u.len() - 1] + d / 2.0,
            ..Axis::default()
        };
        let (title, tx, ty) = split_titles(&self.title);
        let mut ax = axis("xaxis", &xs, dx);
        ax.title = tx;
        let mut ay = axis("yaxis", &ys, dy);
        ay.title = ty;
        Some(Hist {
            class: self.class.clone(),
            name: self.name.clone(),
            title,
            dims: 2,
            storage: Some(Storage::D),
            axes: vec![ax, ay],
            cells,
            entries: self.z.len() as f64,
            maximum: UNSET,
            minimum: UNSET,
            ..Hist::default()
        })
    }
}

/// `title;x title;y title` (as TH1::SetTitle takes it) split; the axis parts are empty when
/// absent.
pub fn split_titles(t: &str) -> (String, String, String) {
    let mut it = t.splitn(4, ';');
    let a = it.next().unwrap_or("").to_owned();
    let b = it.next().unwrap_or("").to_owned();
    let c = it.next().unwrap_or("").to_owned();
    (a, b, c)
}

/// A TTree (or TNtuple) header.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub class: String,
    pub name: String,
    pub title: String,
    pub entries: i64,
}

/// What a key holds, as far as this reader goes.
#[derive(Clone, Debug)]
pub enum Object {
    Hist(Box<Hist>),
    Graph(Box<Graph>),
    Tree(Tree),
}

/// The classes this reader displays or lists.
pub fn is_hist_class(class: &str) -> bool {
    hist_kind(class).is_some()
}

pub fn is_graph_class(class: &str) -> bool {
    matches!(
        class,
        "TGraph" | "TGraphErrors" | "TGraphAsymmErrors" | "TGraph2D"
    )
}

pub fn is_tree_class(class: &str) -> bool {
    matches!(class, "TTree" | "TNtuple" | "TNtupleD")
}

/// (dims, storage, profile) of a histogram class.
pub fn hist_kind(class: &str) -> Option<(usize, Storage, bool)> {
    match class {
        "TProfile" => return Some((1, Storage::D, true)),
        "TProfile2D" => return Some((2, Storage::D, true)),
        _ => {}
    }
    let rest = class.strip_prefix("TH")?;
    let mut ch = rest.chars();
    let dims = match ch.next()? {
        '1' => 1,
        '2' => 2,
        '3' => 3,
        _ => return None,
    };
    let s = Storage::from_letter(ch.next()?)?;
    ch.next().is_none().then_some((dims, s, false))
}

/// Reads a histogram of class `class` from `c`.
pub(crate) fn read_hist(c: &mut Cur, class: &str) -> Result<Hist> {
    let Some((dims, storage, profile)) = hist_kind(class) else {
        return Err(Error::Unsupported(format!("class {class}")));
    };
    let mut h = Hist {
        class: class.to_owned(),
        dims,
        storage: Some(storage),
        profile,
        ..Hist::default()
    };
    if profile {
        let (v, end) = if dims == 1 {
            c.version_of("TProfile", (5, 7))?
        } else {
            c.version_of("TProfile2D", (6, 8))?
        };
        let inner = if dims == 1 { "TH1D" } else { "TH2D" };
        typed(c, &mut h, inner, Storage::D)?;
        h.bin_entries = c.tarray(Storage::D)?;
        h.error_mode = c.i32()?;
        let (lo, hi) = (c.f64()?, c.f64()?);
        let _ = (lo, hi);
        let new = if dims == 1 { 6 } else { 7 };
        if v >= new {
            let (a, b) = if dims == 1 {
                ("fTsumwy", "fTsumwy2")
            } else {
                ("fTsumwz", "fTsumwz2")
            };
            let (s1, s2) = (c.f64()?, c.f64()?);
            // A profile's own sums of the profiled value; TProfile2D keeps its y sums from TH2.
            h.sums.retain(|(m, _)| *m != a && *m != b);
            h.sums.push((a, s1));
            h.sums.push((b, s2));
        }
        if let Some(end) = end {
            c.seek_end(end, class)?;
        }
        if h.bin_entries.len() != h.cells.len() {
            return Err(Error::Corrupt(format!(
                "{class}: {} bin entries for {} cells",
                h.bin_entries.len(),
                h.cells.len()
            )));
        }
        for (v, &w) in h.cells.iter_mut().zip(&h.bin_entries) {
            *v = if w != 0.0 { *v / w } else { 0.0 };
        }
    } else {
        typed(c, &mut h, class, storage)?;
    }
    h.class = class.to_owned();
    Ok(h)
}

/// TH1F and kin: the wrapper version, TH1/TH2/TH3, then the TArray of cells.
fn typed(c: &mut Cur, h: &mut Hist, class: &str, storage: Storage) -> Result<()> {
    let (_, end) = c.version_of(class, (1, 4))?;
    match h.dims {
        1 => th1(c, h)?,
        2 => {
            let (_, e2) = c.version_of("TH2", (3, 5))?;
            th1(c, h)?;
            let _scale = c.f64()?;
            for m in ["fTsumwy", "fTsumwy2", "fTsumwxy"] {
                let v = c.f64()?;
                h.sums.push((m, v));
            }
            if let Some(e) = e2 {
                c.seek_end(e, "TH2")?;
            }
        }
        _ => {
            let (_, e3) = c.version_of("TH3", (5, 6))?;
            th1(c, h)?;
            c.skip_versioned("TAtt3D")?;
            for m in [
                "fTsumwy", "fTsumwy2", "fTsumwxy", "fTsumwz", "fTsumwz2", "fTsumwxz", "fTsumwyz",
            ] {
                let v = c.f64()?;
                h.sums.push((m, v));
            }
            if let Some(e) = e3 {
                c.seek_end(e, "TH3")?;
            }
        }
    }
    h.cells = c.tarray(storage)?;
    if let Some(end) = end {
        c.seek_end(end, class)?;
    }
    let want = (0..h.dims)
        .try_fold(1usize, |a, k| a.checked_mul(h.stride_len(k)))
        .ok_or_else(|| Error::Corrupt(format!("{class}: axes too large")))?;
    if h.cells.len() != want {
        return Err(Error::Corrupt(format!(
            "{class}: {} cells for axes of {want}",
            h.cells.len()
        )));
    }
    if !h.sumw2.is_empty() && h.sumw2.len() != want {
        return Err(Error::Corrupt(format!(
            "{class}: {} sums of squared weights for {want} cells",
            h.sumw2.len()
        )));
    }
    Ok(())
}

/// TH1's own members, as far as fSumw2; the rest (options, functions, buffer) is skipped.
fn th1(c: &mut Cur, h: &mut Hist) -> Result<()> {
    let (v, end) = c.version_of("TH1", (3, 8))?;
    if end.is_none() && v != 3 {
        return Err(Error::Unsupported(format!(
            "TH1 version {v} without a byte count"
        )));
    }
    let (name, title) = c.tnamed()?;
    h.name = name;
    h.title = title;
    c.skip_versioned("TAttLine")?;
    c.skip_versioned("TAttFill")?;
    c.skip_versioned("TAttMarker")?;
    let _ncells = c.i32()?;
    let ax = axis(c)?;
    let ay = axis(c)?;
    let az = axis(c)?;
    h.axes = [ax, ay, az].into_iter().take(h.dims).collect();
    let _bar_offset = c.i16()?;
    let _bar_width = c.i16()?;
    h.entries = c.f64()?;
    for m in ["fTsumw", "fTsumw2", "fTsumwx", "fTsumwx2"] {
        let v = c.f64()?;
        h.sums.push((m, v));
    }
    h.maximum = c.f64()?;
    h.minimum = c.f64()?;
    let _norm = c.f64()?;
    let _contour = c.tarray(Storage::D)?;
    h.sumw2 = c.tarray(Storage::D)?;
    if let Some(end) = end {
        return c.seek_end(end, "TH1");
    }
    // Version 3 without a byte count (Geant4's g4tools): fOption, then the fFunctions TList
    // streamed in place, with its own byte count or else empty.
    let _option = c.tstring()?;
    match c.version()? {
        (_, Some(end)) => c.seek_end(end, "TH1 functions"),
        (_, None) => {
            c.tobject()?;
            let _name = c.tstring()?;
            match c.i32()? {
                0 => Ok(()),
                _ => Err(Error::Unsupported(
                    "TH1 functions without a byte count".into(),
                )),
            }
        }
    }
}

/// TAxis: the bins, edges and (version 9 on) labels.
fn axis(c: &mut Cur) -> Result<Axis> {
    let (v, end) = c.version_of("TAxis", (6, 10))?;
    let Some(end) = end else {
        return Err(Error::Unsupported("TAxis without a byte count".into()));
    };
    let (name, title) = c.tnamed()?;
    c.skip_versioned("TAttAxis")?;
    let n = c.i32()?;
    let nbins = usize::try_from(n)
        .ok()
        .filter(|&n| n >= 1)
        .ok_or_else(|| Error::Corrupt(format!("an axis of {n} bins")))?;
    let min = c.f64()?;
    let max = c.f64()?;
    let edges = c.tarray(Storage::D)?;
    if !edges.is_empty() && edges.len() != nbins + 1 {
        return Err(Error::Corrupt(format!(
            "an axis of {nbins} bins with {} edges",
            edges.len()
        )));
    }
    let mut a = Axis {
        name,
        title,
        nbins,
        min,
        max,
        edges,
        labels: Vec::new(),
    };
    if v >= 9 {
        let _first = c.i32()?;
        let _last = c.i32()?;
        let _bits2 = c.u16()?;
        let _time = c.u8()?;
        let _fmt = c.tstring()?;
        // Labels are a THashList of TObjString numbered by bin; anything odd there leaves the
        // axis without labels rather than failing the histogram.
        let at = c.p;
        if let Ok(l) = labels(c) {
            a.labels = l;
        } else {
            c.p = at;
        }
    }
    c.seek_end(end, "TAxis")?;
    Ok(a)
}

fn labels(c: &mut Cur) -> Result<Vec<(usize, String)>> {
    let Any::Obj { class, end } = c.any()? else {
        return Ok(Vec::new());
    };
    if class != "THashList" && class != "TList" {
        return Err(Error::Unsupported(format!("labels in a {class}")));
    }
    let (_, lend) = c.version()?;
    c.tobject()?;
    let _name = c.tstring()?;
    let n = c.i32()?;
    let n = usize::try_from(n).map_err(|_| Error::Corrupt("label count".into()))?;
    if n > c.left() {
        return Err(Error::Truncated("labels".into()));
    }
    // Reserved for what the bytes left can hold (a label takes well over 16 bytes).
    let mut out = Vec::with_capacity(n.min(c.left() / 16));
    for _ in 0..n {
        match c.any()? {
            Any::Obj { class, end } if class == "TObjString" => {
                let (_, oend) = c.version()?;
                let id = c.tobject()?;
                let s = c.tstring()?;
                if let Some(e) = oend.or(end) {
                    c.seek_end(e, "TObjString")?;
                }
                // TAxis::SetBinLabel numbers each label by its bin; writers that leave the
                // number at 0 (uproot) list the labels in bin order.
                let bin = if id == 0 { out.len() + 1 } else { id as usize };
                out.push((bin, s));
            }
            Any::Obj { end: Some(e), .. } => c.seek_end(e, "label")?,
            Any::Obj { .. } => return Err(Error::Unsupported("label object".into())),
            Any::Null | Any::Ref => {}
        }
        let _option = c.tstring()?;
    }
    if let Some(e) = lend.or(end) {
        c.seek_end(e, "THashList")?;
    }
    Ok(out)
}

/// TGraph and its error variants (points only), TGraph2D.
pub(crate) fn read_graph(c: &mut Cur, class: &str) -> Result<Graph> {
    let mut g = Graph {
        class: class.to_owned(),
        ..Graph::default()
    };
    if class == "TGraph2D" {
        let (_, end) = c.version_of("TGraph2D", (1, 1))?;
        (g.name, g.title) = c.tnamed()?;
        c.skip_versioned("TAttLine")?;
        c.skip_versioned("TAttFill")?;
        c.skip_versioned("TAttMarker")?;
        let n = points(c)?;
        let _npx = c.i32()?;
        let _npy = c.i32()?;
        let _iter = c.i32()?;
        g.x = c.fast_array(n)?;
        g.y = c.fast_array(n)?;
        g.z = c.fast_array(n)?;
        if let Some(e) = end {
            c.seek_end(e, class)?;
        }
        return Ok(g);
    }
    let outer = if class == "TGraph" {
        None
    } else {
        Some(c.version_of(class, (1, 3))?.1)
    };
    let (_, end) = c.version_of("TGraph", (4, 4))?;
    (g.name, g.title) = c.tnamed()?;
    c.skip_versioned("TAttLine")?;
    c.skip_versioned("TAttFill")?;
    c.skip_versioned("TAttMarker")?;
    let n = points(c)?;
    g.x = c.fast_array(n)?;
    g.y = c.fast_array(n)?;
    if let Some(e) = end {
        c.seek_end(e, "TGraph")?;
    }
    if let Some(Some(e)) = outer {
        c.seek_end(e, class)?;
    }
    Ok(g)
}

fn points(c: &mut Cur) -> Result<usize> {
    let n = c.i32()?;
    usize::try_from(n).map_err(|_| Error::Corrupt(format!("{n} points")))
}

/// The head of a TTree: name, title and entries.
pub(crate) fn read_tree(c: &mut Cur, class: &str) -> Result<Tree> {
    if class != "TTree" {
        let _ = c.version()?;
    }
    // Version 5 (as Geant4's g4tools writes) keeps fEntries as a double; 16 on as Long64_t.
    let (v, _end) = c.version()?;
    if v != 5 && !(16..=20).contains(&v) {
        return Err(Error::Version {
            class: "TTree".into(),
            version: v,
            known: "5, 16 to 20".into(),
        });
    }
    let (name, title) = c.tnamed()?;
    c.skip_versioned("TAttLine")?;
    c.skip_versioned("TAttFill")?;
    c.skip_versioned("TAttMarker")?;
    let entries = if v == 5 { c.f64()? as i64 } else { c.i64()? };
    Ok(Tree {
        class: class.to_owned(),
        name,
        title,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_from_titles() {
        assert_eq!(split_unit("p_{T} [GeV]"), ("p_{T}", Some("GeV")));
        assert_eq!(split_unit("E (GeV)"), ("E", Some("GeV")));
        assert_eq!(split_unit("#eta"), ("#eta", None));
        assert_eq!(split_unit("[cm]"), ("[cm]", None));
        assert_eq!(split_unit(""), ("", None));
    }

    #[test]
    fn class_kinds() {
        assert_eq!(hist_kind("TH2F"), Some((2, Storage::F, false)));
        assert_eq!(hist_kind("TH3C"), Some((3, Storage::C, false)));
        assert_eq!(hist_kind("TProfile2D"), Some((2, Storage::D, true)));
        assert_eq!(hist_kind("TH1"), None);
        assert_eq!(hist_kind("TH2Poly"), None);
        assert_eq!(hist_kind("TProfile3D"), None);
    }

    #[test]
    fn graph_grid() {
        let mut g = Graph {
            class: "TGraph2D".into(),
            ..Graph::default()
        };
        for j in 0..3 {
            for i in 0..4 {
                g.x.push(i as f64 * 0.5);
                g.y.push(j as f64);
                g.z.push((i + 10 * j) as f64);
            }
        }
        let h = g.to_grid().expect("grid");
        assert_eq!((h.axes[0].nbins, h.axes[1].nbins), (4, 3));
        assert_eq!(h.cell(2, 3, 0), 21.0);
        g.x[0] = 0.1;
        assert!(g.to_grid().is_none());
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A versioned object with a byte count.
    fn obj(version: i16, body: &[u8]) -> Vec<u8> {
        let mut o = (0x4000_0000u32 | (body.len() as u32 + 2))
            .to_be_bytes()
            .to_vec();
        o.extend_from_slice(&version.to_be_bytes());
        o.extend_from_slice(body);
        o
    }

    fn named() -> Vec<u8> {
        let mut b = obj(1, &[0; 8]);
        b.extend_from_slice(&[1, b'h', 0]);
        obj(1, &b)
    }

    fn axis(nbins: i32) -> Vec<u8> {
        let mut b = named();
        b.extend(obj(4, &[0; 34]));
        b.extend_from_slice(&nbins.to_be_bytes());
        b.extend_from_slice(&0f64.to_be_bytes());
        b.extend_from_slice(&1f64.to_be_bytes());
        b.extend_from_slice(&0i32.to_be_bytes());
        obj(6, &b)
    }

    /// A TH3D of three 2^31 - 1 bin axes with no cells: the cell count overflowed usize.
    #[test]
    fn th3_cell_count_overflow() {
        let mut th1 = named();
        th1.extend(obj(2, &[0; 6]));
        th1.extend(obj(2, &[0; 4]));
        th1.extend(obj(2, &[0; 8]));
        th1.extend_from_slice(&0i32.to_be_bytes());
        for _ in 0..3 {
            th1.extend(axis(i32::MAX));
        }
        // Bar offset and width, eight doubles, empty contour and sumw2 arrays.
        th1.extend_from_slice(&[0; 4 + 8 * 8 + 8]);
        let mut th3 = obj(8, &th1);
        th3.extend(obj(1, &[]));
        th3.extend_from_slice(&[0; 7 * 8]);
        let mut body = obj(6, &th3);
        body.extend_from_slice(&[0; 4]);
        let data = obj(3, &body);
        let mut c = Cur::new(&data, 0);
        assert!(matches!(
            read_hist(&mut c, "TH3D"),
            Err(Error::Corrupt(m)) if m.contains("axes too large")
        ));
    }
}
