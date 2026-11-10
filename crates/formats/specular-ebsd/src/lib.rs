//! EBSD orientation maps: EDAX/TSL `.ang` and Oxford HKL `.ctf` text files read into the
//! format-neutral [`Dataset`] of `specular-em`.
//!
//! A map becomes four pages: inverse pole figure colourings along the sample Z, X and Y axes
//! (red, green and blue planes in 0..=1, [`Page::rgb`]) and the scalar maps of the file (image
//! quality, confidence index, fit and phase for .ang; band contrast, band slope, MAD, bands,
//! error and phase for .ctf). Every page carries the Euler angles and phase of each pixel
//! ([`Orientations`]) for the readout.
//!
//! Hexagonal grids (.ang `HexGrid`) are resampled to a square raster by nearest neighbour:
//! one displayed column per point of the long rows (`NCOLS_ODD`), one displayed row per scan
//! row (`YSTEP` = `XSTEP`·√3/2 apart, so displayed pixels are that much shorter than wide in
//! physical units: [`Page::x`] and [`Page::y`] keep the true steps). The short rows, offset by
//! half a step, show each point at the column to its left and repeat their last point in the
//! final column.
//!
//! Points not indexed (.ang: negative confidence index or the 4π marker; .ctf: phase 0) have
//! no orientation and are black in the IPF pages; points missing from a partial scan are NaN
//! in every plane.

pub mod ipf;
mod text;

use std::sync::Arc;

use rayon::prelude::*;
use specular_em::dataset::{
    Axis, AxisKind, Dataset, Field, Lateral, Orientations, Page, Phase, Plane,
};

use ipf::Symmetry;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not an EDAX/TSL .ang file")]
    NotAng,
    #[error("not an Oxford .ctf file")]
    NotCtf,
    #[error("malformed EBSD file: {0}")]
    Malformed(&'static str),
    #[error("EBSD map of {0} × {1} points is too large")]
    TooLarge(usize, usize),
    #[error("EBSD file holds no points")]
    NoData,
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

/// Largest map accepted (points of the displayed raster).
const MAX_POINTS: usize = 1 << 28;

/// The start of an .ang file: `#` header lines with TSL keys.
pub fn probe_ang(head: &[u8]) -> bool {
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    if head.first() != Some(&b'#') {
        return false;
    }
    const KEYS: [&[u8]; 6] = [
        b"TEM_PIXperUM",
        b"x-star",
        b"GRID:",
        b"NCOLS_ODD",
        b"MaterialName",
        b"WorkingDistance",
    ];
    KEYS.iter().any(|k| head.windows(k.len()).any(|w| w == *k))
}

/// The start of a .ctf file: "Channel Text File".
pub fn probe_ctf(head: &[u8]) -> bool {
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    head.starts_with(b"Channel Text File")
}

/// Reads an .ang or .ctf file, whichever it is.
pub fn read(file: &[u8]) -> Result<Dataset, Error> {
    if probe_ctf(file) {
        read_ctf(file)
    } else {
        read_ang(file)
    }
}

/// Reads an EDAX/TSL .ang file.
pub fn read_ang(file: &[u8]) -> Result<Dataset, Error> {
    if !probe_ang(file) {
        return Err(Error::NotAng);
    }
    let file = file.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(file);
    Ok(build(ang::parse(file)?))
}

/// Reads an Oxford HKL .ctf file.
pub fn read_ctf(file: &[u8]) -> Result<Dataset, Error> {
    if !probe_ctf(file) {
        return Err(Error::NotCtf);
    }
    let file = file.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(file);
    Ok(build(ctf::parse(file)?))
}

mod ang;
mod ctf;

/// A parsed map on its displayed raster.
#[derive(Default)]
struct Scan {
    width: usize,
    height: usize,
    x: Lateral,
    y: Lateral,
    /// Bunge radians per pixel, NaN where not indexed or missing.
    euler: Vec<[f32; 3]>,
    phase: Vec<u8>,
    phases: Vec<(Phase, Option<Symmetry>)>,
    maps: Vec<Plane>,
    title: Option<String>,
    instrument: Option<String>,
    date: Option<String>,
    voltage: Option<f64>,
    magnification: Option<f64>,
    meta: Vec<(String, String)>,
}

/// Where points go on the displayed raster.
struct Grid {
    width: usize,
    height: usize,
    x0: f64,
    y0: f64,
    xstep: f64,
    ystep: f64,
    hex: bool,
}

impl Grid {
    /// Lays out points at (`xs`, `ys`); steps or sizes the header lacks (zero) come from the
    /// coordinates.
    fn new(xs: &[f32], ys: &[f32], hint: Grid) -> Result<Grid, Error> {
        let finite = |v: &[f32]| {
            v.iter()
                .filter(|v| v.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
                    (lo.min(f64::from(v)), hi.max(f64::from(v)))
                })
        };
        let (x0, x1) = finite(xs);
        let (y0, y1) = finite(ys);
        if !x0.is_finite() || !y0.is_finite() {
            return Err(Error::NoData);
        }
        let step = |v: &[f32], given: f64| {
            if given > 0.0 && given.is_finite() {
                return given;
            }
            v.windows(2)
                .map(|w| f64::from((w[1] - w[0]).abs()))
                .filter(|d| *d > 1e-9)
                .fold(f64::INFINITY, f64::min)
        };
        let xstep = step(xs, hint.xstep);
        let mut ystep = step(ys, hint.ystep);
        if !xstep.is_finite() {
            // A single column: any step will do.
            return Err(Error::Malformed("no x step"));
        }
        if !ystep.is_finite() {
            ystep = if hint.hex {
                xstep * 3f64.sqrt() / 2.0
            } else {
                xstep
            };
        }
        let span = |lo: f64, hi: f64, s: f64| (((hi - lo) / s).round() as usize).saturating_add(1);
        let width = hint.width.max(span(x0, x1, xstep));
        let height = hint.height.max(span(y0, y1, ystep));
        // A scan lists its points (indexed or not), so a raster far larger than the points
        // read is a header or step out of all proportion (each pixel costs ~60 bytes).
        let most = MAX_POINTS.min(xs.len().saturating_mul(64).max(1 << 20));
        if width.checked_mul(height).is_none_or(|n| n > most) {
            return Err(Error::TooLarge(width, height));
        }
        Ok(Grid {
            width,
            height,
            x0,
            y0,
            xstep,
            ystep,
            hex: hint.hex,
        })
    }

    /// Reserves the raster's maps (each pixel costs ~60 bytes) from the memory budget.
    fn reserve(&self) -> Result<specular_core::Reservation, Error> {
        Ok(specular_core::budget::reserve_n(
            (self.width * self.height) as u64,
            60,
        )?)
    }

    fn index(&self, x: f32, y: f32) -> Option<usize> {
        let r = ((f64::from(y) - self.y0) / self.ystep).round();
        if !(r >= 0.0 && r < self.height as f64) {
            return None;
        }
        let row = r as usize;
        let u = (f64::from(x) - self.x0) / self.xstep;
        let c = if self.hex && row % 2 == 1 {
            (u + 0.25).floor()
        } else {
            u.round()
        };
        if !(c >= 0.0 && c < self.width as f64) {
            return None;
        }
        Some(row * self.width + c as usize)
    }

    /// Repeats the last point of each short hexagonal row into the final column.
    fn fill_hex<T: Copy>(&self, v: &mut [T], missing: impl Fn(&T) -> bool) {
        if !self.hex || self.width < 2 {
            return;
        }
        for row in (1..self.height).step_by(2) {
            let last = row * self.width + self.width - 1;
            if missing(&v[last]) {
                v[last] = v[last - 1];
            }
        }
    }

    fn lateral(&self) -> (Lateral, Lateral) {
        (
            Lateral::new(self.x0, self.xstep, "µm"),
            Lateral::new(self.y0, self.ystep, "µm"),
        )
    }
}

/// Values of one table column placed on the grid (NaN where no point lands).
fn place(grid: &Grid, at: &[Option<usize>], values: impl Iterator<Item = f32>) -> Vec<f32> {
    let mut out = vec![f32::NAN; grid.width * grid.height];
    for (i, v) in at.iter().zip(values) {
        if let Some(i) = *i {
            out[i] = v;
        }
    }
    grid.fill_hex(&mut out, |v| v.is_nan());
    out
}

fn build(scan: Scan) -> Dataset {
    let keys: Vec<Option<ipf::Key>> = scan
        .phases
        .iter()
        .map(|(_, s)| s.and_then(Symmetry::key))
        .collect();
    let n = scan.width * scan.height;
    let dirs = [[0.0f32, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let colours: Vec<[[f32; 3]; 3]> = scan
        .euler
        .par_iter()
        .zip(scan.phase.par_iter())
        .map(|(e, &p)| {
            if e.iter().any(|v| !v.is_finite()) {
                return [[0.0; 3]; 3];
            }
            let Some(Some(key)) = keys.get(usize::from(p)) else {
                return [ipf::GREY; 3];
            };
            let g = ipf::matrix(*e);
            dirs.map(|d| key.direction_colour(ipf::mul(&g, d.map(f64::from))))
        })
        .collect();
    let orientations = Arc::new(Orientations {
        width: scan.width,
        height: scan.height,
        euler: scan.euler,
        phase: scan.phase,
        phases: scan.phases.into_iter().map(|(p, _)| p).collect(),
    });
    let mut meta = scan.meta;
    for (i, p) in orientations.phases.iter().enumerate() {
        meta.push((
            format!("Phase {}", i + 1),
            format!("{} ({})", p.name, p.symmetry),
        ));
    }
    let base = Page {
        field: Field::Ebsd,
        width: scan.width,
        height: scan.height,
        x: Some(scan.x),
        y: Some(scan.y),
        instrument: scan.instrument,
        date: scan.date,
        voltage: scan.voltage,
        magnification: scan.magnification,
        meta,
        orientations: Some(orientations),
        ..Page::default()
    };
    let mut pages = Vec::with_capacity(4);
    for (d, name) in ["IPF-Z", "IPF-X", "IPF-Y"].iter().enumerate() {
        let planes = ["Red", "Green", "Blue"]
            .iter()
            .enumerate()
            .map(|(c, label)| {
                let mut v = Vec::with_capacity(n);
                v.extend(colours.iter().map(|k| k[d][c]));
                Plane::values(*label, "", v)
            })
            .collect();
        pages.push(Page {
            title: Some((*name).to_owned()),
            planes,
            axis: Axis {
                kind: AxisKind::Channel,
                name: "Colour".into(),
                linear: None,
            },
            rgb: true,
            ..base.clone()
        });
    }
    if !scan.maps.is_empty() {
        pages.push(Page {
            title: Some("Maps".to_owned()),
            planes: scan.maps,
            axis: Axis {
                kind: AxisKind::Channel,
                name: "Map".into(),
                linear: None,
            },
            ..base.clone()
        });
    }
    if let Some(t) = scan.title {
        for p in &mut pages {
            p.meta.insert(0, ("Scan".into(), t.clone()));
        }
    }
    Dataset { pages }
}

/// Per-pixel orientation and phase from per-point columns.
fn orientations(
    grid: &Grid,
    at: &[Option<usize>],
    points: impl Iterator<Item = ([f32; 3], u8)>,
) -> (Vec<[f32; 3]>, Vec<u8>) {
    let n = grid.width * grid.height;
    let mut euler = vec![[f32::NAN; 3]; n];
    let mut phase = vec![u8::MAX; n];
    for (i, (e, p)) in at.iter().zip(points) {
        if let Some(i) = *i {
            euler[i] = e;
            phase[i] = p;
        }
    }
    grid.fill_hex(&mut euler, |e| e[0].is_nan());
    grid.fill_hex(&mut phase, |p| *p == u8::MAX);
    (euler, phase)
}

/// A header value as text, trimmed, `None` when empty.
fn text(v: &str) -> Option<String> {
    let v = v.trim();
    (!v.is_empty()).then(|| v.to_owned())
}

fn num(v: &str) -> Option<f64> {
    text::number(v.trim().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes() {
        assert!(probe_ang(b"# TEM_PIXperUM          1.000000\n"));
        assert!(probe_ang(b"\xEF\xBB\xBF# x-star 0.5\n"));
        assert!(!probe_ang(b"# hello\n"));
        assert!(probe_ctf(b"Channel Text File\r\nPrj x\r\n"));
        assert!(!probe_ctf(b"Channel"));
        assert!(matches!(read_ang(b"nope"), Err(Error::NotAng)));
        assert!(matches!(read_ctf(b"nope"), Err(Error::NotCtf)));
    }

    #[test]
    fn hex_layout() {
        // Two long rows of 3 and one short row of 2 between them.
        let s = 3f32.sqrt() / 2.0;
        let xs = [0.0, 1.0, 2.0, 0.5, 1.5, 0.0, 1.0, 2.0];
        let ys = [0.0, 0.0, 0.0, s, s, 2.0 * s, 2.0 * s, 2.0 * s];
        let hint = Grid {
            width: 3,
            height: 3,
            x0: 0.0,
            y0: 0.0,
            xstep: 1.0,
            ystep: f64::from(s),
            hex: true,
        };
        let g = Grid::new(&xs, &ys, hint).unwrap();
        let at: Vec<_> = xs.iter().zip(&ys).map(|(&x, &y)| g.index(x, y)).collect();
        assert_eq!(
            at,
            [0, 1, 2, 3, 4, 6, 7, 8].map(Some).to_vec(),
            "short row points sit at the column to their left"
        );
        let v = place(&g, &at, (0..8).map(|v| v as f32));
        assert_eq!(v, [0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 5.0, 6.0, 7.0]);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    fn hint(width: usize, height: usize, xstep: f64) -> Grid {
        Grid {
            width,
            height,
            x0: 0.0,
            y0: 0.0,
            xstep,
            ystep: 0.0,
            hex: false,
        }
    }

    /// A step of 1e-300 µm across a 1 µm span: the column count saturates.
    #[test]
    fn step_far_below_the_span() {
        let r = Grid::new(&[0.0, 1.0], &[0.0, 0.0], hint(0, 0, 1e-300));
        assert!(matches!(r, Err(Error::TooLarge(..))));
    }

    /// XCells and YCells of 16384 with two points: no 2^28-pixel raster for them.
    #[test]
    fn header_raster_far_larger_than_the_points() {
        let r = Grid::new(&[0.0, 1.0], &[0.0, 0.0], hint(16384, 16384, 1.0));
        assert!(matches!(r, Err(Error::TooLarge(16384, 16384))));
        let g = Grid::new(&[0.0, 1.0], &[0.0, 0.0], hint(1000, 1000, 1.0)).unwrap();
        assert_eq!((g.width, g.height), (1000, 1000));
    }

    /// A 1000 × 1000 raster (~60 MB of maps, in proportion to its points' cap) is refused
    /// under a 16 MiB memory budget and fits in 256 MiB.
    #[test]
    fn raster_within_memory_budget() {
        use specular_core::budget::with_limit;
        let g = Grid::new(&[0.0, 1.0], &[0.0, 0.0], hint(1000, 1000, 1.0)).unwrap();
        let Err(Error::NeedsMemory(n)) = with_limit(16 << 20, || g.reserve()) else {
            panic!("reserved")
        };
        assert!(n.needed >= 60_000_000 && n.needed > n.free, "{n:?}");
        assert!(with_limit(256 << 20, || g.reserve()).is_ok());
    }

    /// COLUMN_COUNT of 2^40: no rows padded to that many columns.
    #[test]
    fn column_count_of_a_huge_header() {
        let f = b"# TEM_PIXperUM 1\n# COLUMN_COUNT 1099511627776\n0 0 0 0 0\n0 0 0 1 0\n";
        assert!(matches!(read_ang(f), Err(Error::Malformed(_))));
    }
}
