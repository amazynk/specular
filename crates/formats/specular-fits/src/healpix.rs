//! HEALPix maps in FITS binary tables (Górski et al. 2005, ApJ 622, 759), and their
//! Mollweide all-sky raster.
//!
//! A map is a table HDU with `PIXTYPE = 'HEALPIX'`: `ORDERING` `RING` or `NESTED`, `NSIDE`,
//! and either every pixel in order (`INDXSCHM = 'IMPLICIT'`, possibly several pixels per
//! row as `1024E` cells) or a `PIXEL` column naming the pixels present (`'EXPLICIT'`, a
//! partial sky). Multi-order maps (LIGO/Virgo/KAGRA sky localisations) name each pixel by
//! its `UNIQ` index, `4·4^order + ipix` in the nested scheme, with `ORDERING = 'NUNIQ'`.
//! Values are read lazily from the file through [`Table`]; only explicit and multi-order
//! maps build an index (one entry per row).
//!
//! The pixelisation functions are written from the paper's equations: pixel centres on
//! rings of constant latitude, the equatorial zone `|z| ≤ 2/3` and two polar caps, with
//! `sin θ/2` used near the poles so pixels at high order stay exact.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use rayon::prelude::*;

use crate::header::Header;
use crate::table::Table;
use crate::{Error, Hdu, Result};

/// The HEALPix "unseen" value of missing pixels.
pub const UNSEEN: f64 = -1.6375e30;

/// Largest order (`NSIDE = 2^29`) this reader handles.
pub const MAX_ORDER: u32 = 29;

const JRLL: [i64; 12] = [2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4];
const JPLL: [i64; 12] = [1, 3, 5, 7, 0, 2, 4, 6, 1, 3, 5, 7];

/// Position of a direction for the pixel functions: `z = cos θ`, `√(3(1−|z|))` computed
/// from `θ/2`, and `φ` in quarter turns, `[0, 4)`.
struct Loc {
    z: f64,
    s: f64,
    tt: f64,
}

fn loc(theta: f64, phi: f64) -> Loc {
    let z = theta.cos();
    let half = 0.5 * theta;
    // 1 − |z| = 2 sin²(θ/2) in the north, 2 cos²(θ/2) in the south.
    let s = 6f64.sqrt() * if z >= 0.0 { half.sin() } else { half.cos() }.abs();
    let mut tt = phi.rem_euclid(TAU) / FRAC_PI_2;
    if tt >= 4.0 {
        tt = 0.0;
    }
    Loc { z, s, tt }
}

/// The ring-scheme pixel of colatitude `theta` and longitude `phi` (radians).
pub fn ang2pix_ring(nside: u64, theta: f64, phi: f64) -> u64 {
    let ns = nside as i64;
    let nsf = nside as f64;
    let npix = 12 * ns * ns;
    let ncap = 2 * ns * (ns - 1);
    let Loc { z, s, tt } = loc(theta, phi);
    let pix = if z.abs() <= 2.0 / 3.0 {
        let t1 = nsf * (0.5 + tt);
        let t2 = nsf * z * 0.75;
        let jp = (t1 - t2).floor() as i64;
        let jm = (t1 + t2).floor() as i64;
        let ir = ns + 1 + jp - jm;
        let kshift = 1 - (ir & 1);
        let ip = ((jp + jm - ns + kshift + 1) / 2).rem_euclid(4 * ns);
        ncap + (ir - 1) * 4 * ns + ip
    } else {
        let tp = tt - tt.floor();
        let tmp = nsf * s;
        let jp = (tp * tmp).floor() as i64;
        let jm = ((1.0 - tp) * tmp).floor() as i64;
        let ir = (jp + jm + 1).min(ns);
        let ip = ((tt * ir as f64).floor() as i64).rem_euclid(4 * ir);
        if z > 0.0 {
            2 * ir * (ir - 1) + ip
        } else {
            npix - 2 * ir * (ir + 1) + ip
        }
    };
    pix as u64
}

/// Spreads the low 32 bits of `v` to the even bits.
fn spread(v: u64) -> u64 {
    let mut x = v & 0xffff_ffff;
    x = (x | (x << 16)) & 0x0000_ffff_0000_ffff;
    x = (x | (x << 8)) & 0x00ff_00ff_00ff_00ff;
    x = (x | (x << 4)) & 0x0f0f_0f0f_0f0f_0f0f;
    x = (x | (x << 2)) & 0x3333_3333_3333_3333;
    (x | (x << 1)) & 0x5555_5555_5555_5555
}

/// Gathers the even bits of `v`.
fn compact(v: u64) -> u64 {
    let mut x = v & 0x5555_5555_5555_5555;
    x = (x | (x >> 1)) & 0x3333_3333_3333_3333;
    x = (x | (x >> 2)) & 0x0f0f_0f0f_0f0f_0f0f;
    x = (x | (x >> 4)) & 0x00ff_00ff_00ff_00ff;
    x = (x | (x >> 8)) & 0x0000_ffff_0000_ffff;
    (x | (x >> 16)) & 0xffff_ffff
}

/// The nested-scheme pixel of colatitude `theta` and longitude `phi` (radians); `nside` a
/// power of two.
pub fn ang2pix_nest(nside: u64, theta: f64, phi: f64) -> u64 {
    let ns = nside as i64;
    let nsf = nside as f64;
    let Loc { z, s, tt } = loc(theta, phi);
    let (face, ix, iy) = if z.abs() <= 2.0 / 3.0 {
        let t1 = nsf * (0.5 + tt);
        let t2 = nsf * z * 0.75;
        let jp = (t1 - t2).floor() as i64;
        let jm = (t1 + t2).floor() as i64;
        let ifp = jp / ns;
        let ifm = jm / ns;
        let face = if ifp == ifm {
            ifp | 4
        } else if ifp < ifm {
            ifp
        } else {
            ifm + 8
        };
        (face, jm & (ns - 1), ns - (jp & (ns - 1)) - 1)
    } else {
        let ntt = (tt.floor() as i64).min(3);
        let tp = tt - ntt as f64;
        let tmp = nsf * s;
        let jp = ((tp * tmp).floor() as i64).min(ns - 1);
        let jm = (((1.0 - tp) * tmp).floor() as i64).min(ns - 1);
        if z >= 0.0 {
            (ntt, ns - jm - 1, ns - jp - 1)
        } else {
            (ntt + 8, jp, jm)
        }
    };
    (face as u64) * nside * nside + (spread(ix as u64) | (spread(iy as u64) << 1))
}

/// Colatitude from `t = 1 − |z|` without cancellation near the poles.
fn theta_of(t: f64, north: bool) -> f64 {
    let a = 2.0 * (0.5 * t).max(0.0).sqrt().min(1.0).asin();
    if north { a } else { PI - a }
}

/// Colatitude and longitude (radians) of the centre of ring-scheme pixel `pix`.
pub fn pix2ang_ring(nside: u64, pix: u64) -> (f64, f64) {
    let ns = nside as i64;
    let nsf = nside as f64;
    let p = pix as i64;
    let npix = 12 * ns * ns;
    let ncap = 2 * ns * (ns - 1);
    let fact2 = 4.0 / npix as f64;
    if p < ncap {
        let iring = (1 + (1 + 2 * p as u64).isqrt() as i64) >> 1;
        let iphi = p + 1 - 2 * iring * (iring - 1);
        let tmp = (iring * iring) as f64 * fact2;
        (
            theta_of(tmp, true),
            (iphi as f64 - 0.5) * FRAC_PI_2 / iring as f64,
        )
    } else if p < npix - ncap {
        let ip = p - ncap;
        let tmp = ip / (4 * ns);
        let iring = tmp + ns;
        let iphi = ip - 4 * ns * tmp + 1;
        let fodd = if (iring + ns) & 1 != 0 { 1.0 } else { 0.5 };
        let z = (2 * ns - iring) as f64 * 2.0 / (3.0 * nsf);
        (z.acos(), (iphi as f64 - fodd) * PI / (2.0 * nsf))
    } else {
        let ip = npix - p;
        let iring = (1 + (2 * ip as u64 - 1).isqrt() as i64) >> 1;
        let iphi = 4 * iring + 1 - (ip - 2 * iring * (iring - 1));
        let tmp = (iring * iring) as f64 * fact2;
        (
            theta_of(tmp, false),
            (iphi as f64 - 0.5) * FRAC_PI_2 / iring as f64,
        )
    }
}

/// Colatitude and longitude (radians) of the centre of nested-scheme pixel `pix`.
pub fn pix2ang_nest(nside: u64, pix: u64) -> (f64, f64) {
    let ns = nside as i64;
    let npface = nside * nside;
    let face = (pix / npface) as usize;
    let sub = pix % npface;
    let ix = compact(sub) as i64;
    let iy = compact(sub >> 1) as i64;
    let jr = JRLL[face] * ns - ix - iy - 1;
    let (nr, kshift, theta) = if jr < ns {
        let t = (jr * jr) as f64 / (3.0 * npface as f64);
        (jr, 0, theta_of(t, true))
    } else if jr > 3 * ns {
        let nr = 4 * ns - jr;
        let t = (nr * nr) as f64 / (3.0 * npface as f64);
        (nr, 0, theta_of(t, false))
    } else {
        let z = (2 * ns - jr) as f64 * 2.0 / (3.0 * ns as f64);
        (ns, (jr - ns) & 1, z.acos())
    };
    let mut jp = (JPLL[face] * nr + ix - iy + 1 + kshift) / 2;
    if jp > 4 * ns {
        jp -= 4 * ns;
    }
    if jp < 1 {
        jp += 4 * ns;
    }
    let phi = (jp as f64 - (kshift + 1) as f64 * 0.5) * (FRAC_PI_2 / nr as f64);
    (theta, phi)
}

/// Order and nested pixel of a `UNIQ` index; `None` below 4 or past [`MAX_ORDER`].
pub fn from_uniq(uniq: u64) -> Option<(u32, u64)> {
    if uniq < 4 {
        return None;
    }
    let order = (63 - uniq.leading_zeros() - 2) / 2;
    if order > MAX_ORDER {
        return None;
    }
    Some((order, uniq - (4u64 << (2 * order))))
}

/// The `UNIQ` index of nested pixel `ipix` at `order`.
pub fn to_uniq(order: u32, ipix: u64) -> u64 {
    (4u64 << (2 * order)) + ipix
}

/// Pixel numbering of a map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ordering {
    Ring,
    Nested,
    /// Multi-order, by `UNIQ` index.
    Uniq,
}

/// How rows name their pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// Pixel `n` is element `n` of the table, in order.
    Implicit,
    /// A `PIXEL` column (partial sky).
    Explicit(usize),
    /// A `UNIQ` column.
    Uniq(usize),
}

/// The map's coordinate system (`COORDSYS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordSys {
    Equatorial,
    Galactic,
    Ecliptic,
    /// Not stated.
    Unknown,
}

/// A HEALPix map in a table HDU.
#[derive(Clone, Debug)]
pub struct Map {
    pub table: Table,
    /// `NSIDE` (of the finest order present, for a multi-order map; found by
    /// [`Map::index`]).
    pub nside: u64,
    pub ordering: Ordering,
    pub scheme: Scheme,
    pub coordsys: CoordSys,
    /// The numeric columns with map values, in table order.
    pub fields: Vec<usize>,
}

/// The pixel lookup of an explicit or multi-order map.
#[derive(Clone, Debug, Default)]
pub struct Index {
    /// Sorted `(first pixel at the finest order, row)`; the row's pixel count is
    /// `4^(order_max − order)`.
    starts: Vec<(u64, u32)>,
    /// Pixels at the finest order each row covers, parallel to `starts`.
    spans: Vec<u64>,
    /// Finest order present (multi-order maps).
    pub order: u32,
}

fn str_key(h: &Header, k: &str) -> String {
    h.str(k).unwrap_or("").trim().to_ascii_uppercase()
}

impl Map {
    /// The HEALPix map of a table HDU; `None` when the HDU is not one, an error when it
    /// claims to be one but is malformed.
    pub fn from_hdu(hdu: &Hdu) -> Option<Result<Map>> {
        let h = &hdu.header;
        let pixtype = str_key(h, "PIXTYPE");
        let ordering = str_key(h, "ORDERING");
        let healpix = pixtype == "HEALPIX"
            || (h.has("NSIDE") && matches!(ordering.as_str(), "RING" | "NESTED" | "NEST"))
            || ordering == "NUNIQ";
        if !healpix {
            return None;
        }
        Some(Self::read(hdu, &ordering))
    }

    fn read(hdu: &Hdu, ordering: &str) -> Result<Map> {
        let h = &hdu.header;
        let table = Table::from_hdu(hdu)?;
        let uniq = table.column("UNIQ");
        let ordering = match ordering {
            "RING" => Ordering::Ring,
            "NESTED" | "NEST" => Ordering::Nested,
            "NUNIQ" => Ordering::Uniq,
            _ if uniq.is_some() => Ordering::Uniq,
            o => return Err(Error::Unsupported(format!("HEALPix ORDERING = {o}"))),
        };
        let scheme = if ordering == Ordering::Uniq {
            Scheme::Uniq(uniq.ok_or_else(|| Error::Malformed("no UNIQ column".into()))?)
        } else if str_key(h, "INDXSCHM") == "EXPLICIT"
            || (str_key(h, "OBJECT") == "PARTIAL" && table.column("PIXEL").is_some())
        {
            Scheme::Explicit(
                table
                    .column("PIXEL")
                    .ok_or_else(|| Error::Malformed("no PIXEL column".into()))?,
            )
        } else {
            Scheme::Implicit
        };
        let nside = if ordering == Ordering::Uniq {
            1
        } else {
            let n = h
                .i64("NSIDE")
                .filter(|n| (1..=1 << MAX_ORDER).contains(n))
                .ok_or_else(|| Error::Malformed("no NSIDE".into()))? as u64;
            if ordering == Ordering::Nested && !n.is_power_of_two() {
                return Err(Error::Malformed(format!("nested NSIDE = {n}")));
            }
            n
        };
        let coordsys = match str_key(h, "COORDSYS").as_str() {
            "C" | "Q" | "CELESTIAL" | "EQUATORIAL" | "ICRS" | "FK5" => CoordSys::Equatorial,
            "G" | "GALACTIC" => CoordSys::Galactic,
            "E" | "ECLIPTIC" => CoordSys::Ecliptic,
            _ => CoordSys::Unknown,
        };
        let skip = |i: usize| match scheme {
            Scheme::Explicit(c) | Scheme::Uniq(c) => c == i,
            Scheme::Implicit => false,
        };
        let fields: Vec<usize> = (0..table.columns.len())
            .filter(|&i| !skip(i) && table.columns[i].is_numeric() && !table.columns[i].is_var())
            .collect();
        if fields.is_empty() {
            return Err(Error::Malformed("HEALPix map without values".into()));
        }
        Ok(Map {
            table,
            nside,
            ordering,
            scheme,
            coordsys,
            fields,
        })
    }

    /// The field names (`TTYPE`, or `field N`).
    pub fn field_names(&self) -> Vec<String> {
        self.fields
            .iter()
            .map(|&c| {
                let n = &self.table.columns[c].name;
                if n.is_empty() {
                    format!("{}", c + 1)
                } else {
                    n.clone()
                }
            })
            .collect()
    }

    /// The field holding a probability (`PROB`, `PROBABILITY`) or a probability density
    /// (`PROBDENSITY`), whose credible regions can be drawn.
    pub fn probability_field(&self) -> Option<usize> {
        self.fields.iter().position(|&c| {
            let n = self.table.columns[c].name.to_ascii_uppercase();
            matches!(n.as_str(), "PROB" | "PROBABILITY" | "PROBDENSITY")
        })
    }

    /// Number of pixels at `nside`.
    pub fn npix(&self) -> u64 {
        12 * self.nside * self.nside
    }

    /// Builds the pixel lookup (explicit and multi-order maps; empty for implicit ones), and
    /// for a multi-order map sets [`Map::nside`] to the finest order's.
    pub fn index(&mut self, data: &[u8]) -> Result<Index> {
        let rows = self.table.rows_in(data);
        if u32::try_from(rows).is_err() {
            return Err(Error::TooLarge);
        }
        let mut idx = Index::default();
        match self.scheme {
            Scheme::Implicit => {}
            Scheme::Explicit(c) => {
                let npix = self.npix();
                for r in 0..rows {
                    if let Some(p) = self.table.f64(data, r, c, 0)
                        && p >= 0.0
                        && (p as u64) < npix
                    {
                        idx.starts.push((p as u64, r as u32));
                    }
                }
                idx.starts.sort_unstable();
                idx.spans = vec![1; idx.starts.len()];
            }
            Scheme::Uniq(c) => {
                let mut cells = Vec::with_capacity(rows);
                let mut order = 0;
                for r in 0..rows {
                    let Some(u) = self.uniq(data, r, c) else {
                        continue;
                    };
                    if let Some((o, p)) = from_uniq(u) {
                        order = order.max(o);
                        cells.push((o, p, r as u32));
                    }
                }
                idx.order = order;
                self.nside = 1 << order;
                idx.starts = cells
                    .iter()
                    .map(|&(o, p, r)| (p << (2 * (order - o)), r))
                    .collect();
                idx.starts.sort_unstable();
                idx.spans = idx
                    .starts
                    .iter()
                    .map(|&(_, r)| {
                        let o = from_uniq(self.uniq(data, r as usize, c).unwrap_or(4))
                            .map_or(0, |x| x.0);
                        1u64 << (2 * (order - o))
                    })
                    .collect();
            }
        }
        Ok(idx)
    }

    /// The `UNIQ` value of row `r` (64-bit columns read exactly).
    fn uniq(&self, data: &[u8], r: usize, c: usize) -> Option<u64> {
        match self.table.cell(data, r, c).first()? {
            crate::table::Datum::Int(i) => u64::try_from(*i).ok(),
            crate::table::Datum::UInt(u) => Some(*u),
            d => d.as_f64().filter(|v| *v >= 0.0).map(|v| v as u64),
        }
    }

    /// The row and element of pixel `pix` (in the map's numbering at [`Map::nside`]).
    fn locate(&self, data_rows: usize, idx: &Index, pix: u64) -> Option<(usize, usize)> {
        match self.scheme {
            Scheme::Implicit => {
                let c = &self.table.columns[self.fields[0]];
                let per = c.repeat.max(1) as u64;
                let row = (pix / per) as usize;
                (row < data_rows).then_some((row, (pix % per) as usize))
            }
            Scheme::Explicit(_) | Scheme::Uniq(_) => {
                let k = idx.starts.partition_point(|&(s, _)| s <= pix);
                let k = k.checked_sub(1)?;
                let (s, r) = idx.starts[k];
                (pix < s + idx.spans[k]).then_some((r as usize, 0))
            }
        }
    }

    /// The value of field `field` (an index into [`Map::fields`]) at a row and element;
    /// `None` when undefined or [`UNSEEN`].
    fn value(&self, data: &[u8], field: usize, row: usize, elem: usize) -> Option<f64> {
        let col = self.fields[field];
        let c = &self.table.columns[col];
        let elem = if c.repeat > 1 { elem } else { 0 };
        let v = self.table.f64(data, row, col, elem)?;
        (v.is_finite() && (v - UNSEEN).abs() > 1e25).then_some(v)
    }

    /// The pixel of a direction (colatitude, longitude in radians) in the map's numbering.
    fn pixel_of(&self, theta: f64, phi: f64) -> u64 {
        match self.ordering {
            Ordering::Ring => ang2pix_ring(self.nside, theta, phi),
            Ordering::Nested | Ordering::Uniq => ang2pix_nest(self.nside, theta, phi),
        }
    }

    /// The value of field `field` in the direction of longitude / latitude (degrees, in the
    /// map's own frame).
    pub fn value_at(
        &self,
        data: &[u8],
        idx: &Index,
        field: usize,
        lng: f64,
        lat: f64,
    ) -> Option<f64> {
        let theta = (90.0 - lat).to_radians();
        let pix = self.pixel_of(theta, lng.to_radians());
        let (r, e) = self.locate(self.table.rows, idx, pix)?;
        self.value(data, field, r, e)
    }

    /// A good Mollweide raster size for the map: about one image pixel per map pixel,
    /// within 512–4096 wide (twice as wide as high).
    pub fn raster_size(&self) -> (usize, usize) {
        let npix = self.npix() as f64;
        let w = (8.0 * npix / PI).sqrt().clamp(512.0, 4096.0) as usize;
        let w = w & !1;
        (w, w / 2)
    }

    /// The Mollweide raster of field `field`, `width × width/2`, rows top-down, longitude
    /// increasing to the left from the centre at 0; NaN outside the sky and where the map has
    /// no value. `cancel` is polled per row.
    pub fn mollweide(
        &self,
        data: &[u8],
        idx: &Index,
        field: usize,
        width: usize,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<f32>> {
        let h = width / 2;
        let mut out = vec![f32::NAN; width * h];
        let rows = self.table.rows;
        out.par_chunks_mut(width.max(1))
            .enumerate()
            .try_for_each(|(j, line)| {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                for (i, v) in line.iter_mut().enumerate() {
                    let Some((lng, lat)) =
                        mollweide_inverse(width, h, i as f64 + 0.5, j as f64 + 0.5)
                    else {
                        continue;
                    };
                    let theta = FRAC_PI_2 - lat;
                    let pix = self.pixel_of(theta, lng);
                    if let Some((r, e)) = self.locate(rows, idx, pix)
                        && let Some(x) = self.value(data, field, r, e)
                    {
                        *v = x as f32;
                    }
                }
                Ok(())
            })?;
        Ok(out)
    }

    /// Thresholds of field `field` enclosing the given fractions of its total (credible
    /// regions of a probability map, `levels` such as 0.5 and 0.9): the smallest value whose
    /// region of higher-or-equal values holds the fraction. Densities of a multi-order map
    /// are weighted by pixel area. `None` when the field has no positive total.
    pub fn credible(&self, data: &[u8], field: usize, levels: &[f64]) -> Option<Vec<f64>> {
        let col = self.fields[field];
        let per = self.table.columns[col].repeat.max(1);
        let mut cells: Vec<(f64, f64)> = Vec::new();
        for r in 0..self.table.rows_in(data) {
            let area = match self.scheme {
                Scheme::Uniq(c) => {
                    let o = from_uniq(self.uniq(data, r, c)?)?.0;
                    4.0 * PI / (12u64 << (2 * o)) as f64
                }
                _ => 1.0,
            };
            for e in 0..per {
                if let Some(v) = self.value(data, field, r, e)
                    && v > 0.0
                {
                    cells.push((v, area));
                }
            }
        }
        cells.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
        let total: f64 = cells.iter().map(|c| c.0 * c.1).sum();
        if total <= 0.0 {
            return None;
        }
        let mut cum = Vec::with_capacity(cells.len());
        let mut acc = 0.0;
        for c in &cells {
            acc += c.0 * c.1;
            cum.push(acc);
        }
        Some(
            levels
                .iter()
                .map(|&p| {
                    let k = cum.partition_point(|&s| s < p * total).min(cells.len() - 1);
                    cells[k].0
                })
                .collect(),
        )
    }

    /// The header of the Mollweide raster's WCS (`RA---MOL` / `DEC--MOL`, or the galactic,
    /// ecliptic or unnamed pair), centred on longitude 0.
    pub fn mollweide_header(&self, width: usize) -> Header {
        mollweide_header(self.coordsys, width)
    }
}

/// The WCS header of a `width × width/2` Mollweide all-sky raster.
pub fn mollweide_header(coordsys: CoordSys, width: usize) -> Header {
    let h = width / 2;
    let r0 = 180.0 / PI;
    let (c1, c2) = match coordsys {
        CoordSys::Equatorial => ("'RA---MOL'", "'DEC--MOL'"),
        CoordSys::Galactic => ("'GLON-MOL'", "'GLAT-MOL'"),
        CoordSys::Ecliptic => ("'ELON-MOL'", "'ELAT-MOL'"),
        CoordSys::Unknown => ("'XLON-MOL'", "'XLAT-MOL'"),
    };
    let f = |v: f64| format!("{v:.17E}");
    let cards = [
        ("NAXIS", "2".to_owned()),
        ("NAXIS1", width.to_string()),
        ("NAXIS2", h.to_string()),
        ("CTYPE1", c1.to_owned()),
        ("CTYPE2", c2.to_owned()),
        ("CRPIX1", f(width as f64 / 2.0 + 0.5)),
        ("CRPIX2", f(h as f64 / 2.0 + 0.5)),
        ("CDELT1", f(-4.0 * 2f64.sqrt() * r0 / width as f64)),
        ("CDELT2", f(2.0 * 2f64.sqrt() * r0 / h as f64)),
        ("CRVAL1", "0.0".to_owned()),
        ("CRVAL2", "0.0".to_owned()),
        ("CUNIT1", "'deg'".to_owned()),
        ("CUNIT2", "'deg'".to_owned()),
    ];
    let mut v: Vec<(&str, String)> = cards.to_vec();
    if coordsys == CoordSys::Equatorial {
        v.push(("RADESYS", "'ICRS'".to_owned()));
    }
    Header::from_pairs(v.iter().map(|(k, x)| (*k, x.as_str(), "")))
}

/// Longitude in `[0, 2π)` and latitude (radians) at a position of a `w × h` Mollweide
/// raster (top-down, longitude increasing to the left); `None` outside the ellipse.
pub fn mollweide_inverse(w: usize, h: usize, x: f64, y: f64) -> Option<(f64, f64)> {
    let s2 = 2f64.sqrt();
    let xx = (x - w as f64 / 2.0) * (4.0 * s2 / w as f64);
    let yy = (h as f64 / 2.0 - y) * (2.0 * s2 / h as f64);
    let sg = yy / s2;
    if xx * xx / 8.0 + yy * yy / 2.0 > 1.0 || sg.abs() > 1.0 {
        return None;
    }
    let g = sg.asin();
    let lat = ((2.0 * g + (2.0 * g).sin()) / PI).clamp(-1.0, 1.0).asin();
    let cg = g.cos();
    let lng = if cg <= 1e-12 {
        0.0
    } else {
        -PI * xx / (2.0 * s2 * cg)
    };
    if lng.abs() > PI + 1e-12 {
        return None;
    }
    Some((lng.rem_euclid(TAU), lat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for nside in [1u64, 2, 4, 16, 1024, 1 << 20, 1 << 29] {
            let npix = 12 * nside * nside;
            let step = (npix / 997).max(1);
            let mut p = 0;
            while p < npix {
                let (t, f) = pix2ang_ring(nside, p);
                assert_eq!(ang2pix_ring(nside, t, f), p, "ring nside {nside} pix {p}");
                let (t, f) = pix2ang_nest(nside, p);
                assert_eq!(ang2pix_nest(nside, t, f), p, "nest nside {nside} pix {p}");
                p += step;
            }
            for p in [0, npix - 1, npix / 2] {
                let (t, f) = pix2ang_ring(nside, p);
                assert_eq!(ang2pix_ring(nside, t, f), p);
                let (t, f) = pix2ang_nest(nside, p);
                assert_eq!(ang2pix_nest(nside, t, f), p);
            }
        }
        // Nside 3 (ring only).
        for p in 0..108 {
            let (t, f) = pix2ang_ring(3, p);
            assert_eq!(ang2pix_ring(3, t, f), p);
        }
    }

    #[test]
    fn known_pixels() {
        // Base pixels at nside 1: face centres.
        let (t, f) = pix2ang_nest(1, 4);
        assert!((t - FRAC_PI_2).abs() < 1e-12 && f.abs() < 1e-12);
        assert_eq!(ang2pix_ring(1, 0.0, 0.0), 0);
        assert_eq!(ang2pix_nest(1, PI, 0.0), 8);
        assert_eq!(from_uniq(to_uniq(10, 12345)), Some((10, 12345)));
        assert_eq!(from_uniq(4), Some((0, 0)));
        assert_eq!(from_uniq(15), Some((0, 11)));
        assert_eq!(from_uniq(16), Some((1, 0)));
    }

    #[test]
    fn mollweide_matches_wcs() {
        let w = 64;
        let hdr = mollweide_header(CoordSys::Equatorial, w);
        let wcs = crate::wcs::Wcs::from_header(&hdr, &[w, w / 2]).unwrap();
        let mut n = 0;
        for j in 0..w / 2 {
            for i in 0..w {
                let (x, y) = (i as f64 + 0.5, j as f64 + 0.5);
                let Some((lng, lat)) = mollweide_inverse(w, w / 2, x, y) else {
                    continue;
                };
                let (a, d) = wcs.sky_at(x, y, &[]).unwrap();
                let dl = (a - lng.to_degrees()).rem_euclid(360.0);
                assert!(
                    dl.min(360.0 - dl) < 1e-9,
                    "{i},{j}: {a} vs {}",
                    lng.to_degrees()
                );
                assert!((d - lat.to_degrees()).abs() < 1e-9);
                let (px, py) = wcs.display_of(a, d, &[]).unwrap();
                assert!((px - x).abs() < 1e-6 && (py - y).abs() < 1e-6);
                n += 1;
            }
        }
        assert!(n > 1000);
        assert!(mollweide_inverse(w, w / 2, 0.5, 0.5).is_none());
    }
}
