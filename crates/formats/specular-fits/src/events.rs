//! X-ray event lists (OGIP `EVENTS` tables: Chandra, XMM-Newton, eROSITA, NICER, Swift),
//! binned into images.
//!
//! An event list is a table with a row per detected photon: sky position `X` / `Y` (or the
//! detector's `RAWX` / `RAWY` when there is no sky position), time, and an energy (`ENERGY`
//! in eV, or a pulse-invariant channel `PI`). [`Events::from_hdu`] finds the columns, the
//! extent of the positions and the energy scale, in one pass over the rows;
//! [`Events::bin`] counts the events of an energy band into bins of a size, also in one
//! pass, reading values from the file's bytes.
//!
//! The image grid is fixed: bins of `base` sky pixels (a power of two that keeps the image
//! within 4096 pixels), with edges aligned to the column's `TLMIN`. Coarser bins (2, 4, 8
//! times `base`) lie on the same grid, each count shown over the block of base bins it
//! covers, so every binning shares one size and one WCS. The WCS comes from the columns'
//! `TCTYPn`, `TCRVLn`, `TCRPXn`, `TCDLTn` (FITS 4.0 §8.2 "pixel list" keywords), scaled to
//! the base bin.

use crate::header::Header;
use crate::table::Table;
use crate::{Error, Hdu, Result};

/// Largest image side at the base bin.
pub const MAX_SIDE: f64 = 4096.0;

/// Bin factors over the base bin.
pub const FACTORS: [u32; 4] = [1, 2, 4, 8];

/// An energy band of the events.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    /// Bounds in the energy column's stored units (`lo ≤ e < hi`); `None` for every event.
    pub range: Option<(f64, f64)>,
    /// Bounds in keV, when the energy scale is known.
    pub kev: Option<(f64, f64)>,
}

/// The column-WCS of one position axis.
#[derive(Clone, Debug, PartialEq)]
struct AxisWcs {
    ctype: String,
    crval: f64,
    crpix: f64,
    cdelt: f64,
    cunit: Option<String>,
}

/// An event list.
#[derive(Clone, Debug)]
pub struct Events {
    pub table: Table,
    /// Position columns.
    pub x: usize,
    pub y: usize,
    /// Sky positions (`X` / `Y`), rather than detector ones.
    pub sky: bool,
    /// Energy column and its eV per stored unit, when known.
    pub energy: Option<(usize, Option<f64>)>,
    /// Left / bottom edge of the first bin, the base bin size, and the image size in bins.
    pub x0: f64,
    pub y0: f64,
    pub base: f64,
    pub width: usize,
    pub height: usize,
    /// Energy bands, the last one every event.
    pub bands: Vec<Band>,
    /// Events with a position in the image.
    pub count: usize,
    wcs: [Option<AxisWcs>; 2],
}

fn axis_wcs(h: &Header, col: usize) -> Option<AxisWcs> {
    let n = col + 1;
    let ctype = h.str(&format!("TCTYP{n}"))?.trim().to_owned();
    let f = |k: &str| h.f64(&format!("{k}{n}")).filter(|v| v.is_finite());
    let cdelt = f("TCDLT")?;
    if cdelt == 0.0 {
        return None;
    }
    Some(AxisWcs {
        ctype,
        crval: f("TCRVL")?,
        crpix: f("TCRPX")?,
        cdelt,
        cunit: h
            .str(&format!("TCUNI{n}"))
            .map(|u| u.trim().to_owned())
            .filter(|u| !u.is_empty()),
    })
}

/// eV per `PI` channel by mission, when the column does not state it.
fn pi_scale(h: &Header) -> Option<f64> {
    let t = h.str("TELESCOP").unwrap_or("").trim().to_ascii_uppercase();
    let i = h.str("INSTRUME").unwrap_or("").trim().to_ascii_uppercase();
    if t.contains("XMM") || t.contains("SRG") || t.contains("EROSITA") || i.contains("EROSITA") {
        Some(1.0)
    } else if t.contains("NICER") || t.contains("SWIFT") {
        Some(10.0)
    } else {
        None
    }
}

/// eV per unit of an energy column's `TUNIT`.
fn energy_unit(u: &str) -> Option<f64> {
    match u.trim().to_ascii_lowercase().as_str() {
        "ev" => Some(1.0),
        "kev" => Some(1000.0),
        "mev" => Some(1.0e6),
        _ => None,
    }
}

impl Events {
    /// The event list of a table HDU; `None` when the HDU is not one.
    pub fn from_hdu(data: &[u8], hdu: &Hdu) -> Option<Result<Events>> {
        let h = &hdu.header;
        if !Self::is_events(hdu) {
            return None;
        }
        let table = match Table::from_hdu(hdu) {
            Ok(t) => t,
            Err(e) => return Some(Err(e)),
        };
        let pair = |a: &str, b: &str| Some((table.column(a)?, table.column(b)?));
        let (x, y, sky) = match (pair("X", "Y"), pair("RAWX", "RAWY")) {
            (Some((x, y)), _) => (x, y, true),
            (None, Some((x, y))) => (x, y, false),
            _ => return None,
        };
        Some(Self::read(data, h, table, x, y, sky))
    }

    /// Whether a table HDU is an event list (`EXTNAME = 'EVENTS'` or `'STDEVT'`, or
    /// `HDUCLAS1 = 'EVENTS'`) with positions (`X` / `Y`, else `RAWX` / `RAWY`), from its
    /// header alone.
    pub fn is_events(hdu: &Hdu) -> bool {
        let h = &hdu.header;
        let key = |k: &str| h.str(k).unwrap_or("").trim().to_ascii_uppercase();
        let (ext, class) = (key("EXTNAME"), key("HDUCLAS1"));
        if !(ext == "EVENTS" || ext == "STDEVT" || class == "EVENTS") {
            return false;
        }
        let Ok(t) = Table::from_hdu(hdu) else {
            return false;
        };
        let has = |a: &str, b: &str| t.column(a).is_some() && t.column(b).is_some();
        has("X", "Y") || has("RAWX", "RAWY")
    }

    fn read(
        data: &[u8],
        h: &Header,
        table: Table,
        x: usize,
        y: usize,
        sky: bool,
    ) -> Result<Events> {
        // Energy: ENERGY (in TUNIT, eV by default), else PI (per TUNIT, else per mission).
        let energy = if let Some(c) = table.column("ENERGY") {
            let u = &table.columns[c].unit;
            Some((
                c,
                if u.is_empty() {
                    Some(1.0)
                } else {
                    energy_unit(u)
                },
            ))
        } else if let Some(c) = table.column("PI") {
            let u = &table.columns[c].unit;
            Some((c, energy_unit(u).or_else(|| pi_scale(h))))
        } else {
            None
        };
        // Extent of the positions.
        let (mut xmin, mut xmax, mut ymin, mut ymax) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for r in 0..table.rows_in(data) {
            if let (Some(a), Some(b)) = (table.f64(data, r, x, 0), table.f64(data, r, y, 0)) {
                xmin = xmin.min(a);
                xmax = xmax.max(a);
                ymin = ymin.min(b);
                ymax = ymax.max(b);
            }
        }
        if xmin > xmax || ymin > ymax {
            return Err(Error::Unsupported("event list without positions".into()));
        }
        let ext = (xmax - xmin).max(ymax - ymin).max(1.0);
        let base = (ext / MAX_SIDE).log2().ceil().exp2().max(1.0);
        // XMM SAS writes TLMINn of X / Y as a string ('1').
        let lim = |c: usize, k: &str| {
            let key = format!("{k}{}", c + 1);
            h.f64(&key)
                .or_else(|| h.str(&key).and_then(|s| s.trim().parse().ok()))
                .filter(|v: &f64| v.is_finite())
        };
        let origin = |c: usize, min: f64| match lim(c, "TLMIN") {
            Some(t) if t <= min => t + ((min - t) / base).floor() * base,
            _ => (min / base).floor() * base,
        };
        let x0 = origin(x, xmin);
        let y0 = origin(y, ymin);
        let side = |min: f64, max: f64| ((max - min) / base).floor() as usize + 1;
        let (width, height) = (side(x0, xmax), side(y0, ymax));
        if width as f64 > 2.0 * MAX_SIDE || height as f64 > 2.0 * MAX_SIDE {
            return Err(Error::TooLarge);
        }
        let wcs = if sky {
            [axis_wcs(h, x), axis_wcs(h, y)]
        } else {
            [None, None]
        };
        let bands = match energy {
            Some((_, Some(ev))) => {
                let mut b: Vec<Band> = [(0.5, 1.2), (1.2, 2.0), (2.0, 7.0)]
                    .iter()
                    .map(|&(lo, hi)| Band {
                        range: Some((lo * 1000.0 / ev, hi * 1000.0 / ev)),
                        kev: Some((lo, hi)),
                    })
                    .collect();
                b.push(Band {
                    range: None,
                    kev: None,
                });
                b
            }
            Some((c, None)) => tertiles(data, &table, c),
            None => vec![Band {
                range: None,
                kev: None,
            }],
        };
        let mut ev = Events {
            table,
            x,
            y,
            sky,
            energy,
            x0,
            y0,
            base,
            width,
            height,
            bands,
            count: 0,
            wcs,
        };
        ev.count = (0..ev.table.rows_in(data))
            .filter(|&r| ev.bin_of(data, r, 1).is_some())
            .count();
        Ok(ev)
    }

    /// The base bin `(column, row from the bottom)` of event `r`, for bins `k` times the base.
    fn bin_of(&self, data: &[u8], r: usize, k: u32) -> Option<(usize, usize)> {
        let b = self.base * f64::from(k);
        let fx = ((self.table.f64(data, r, self.x, 0)? - self.x0) / b).floor();
        let fy = ((self.table.f64(data, r, self.y, 0)? - self.y0) / b).floor();
        if fx < 0.0 || fy < 0.0 {
            return None;
        }
        let (i, j) = (fx as usize, fy as usize);
        let (w, h) = (
            self.width.div_ceil(k as usize),
            self.height.div_ceil(k as usize),
        );
        (i < w && j < h).then_some((i, j))
    }

    /// Counts of the events in band `band` in bins `FACTORS[factor]` times the base bin,
    /// on the base grid: `width × height` values, rows top-down. `cancel` is polled.
    pub fn bin(
        &self,
        data: &[u8],
        factor: usize,
        band: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<f32>> {
        let k = FACTORS.get(factor).copied().unwrap_or(1);
        let range = self.bands.get(band).and_then(|b| b.range);
        let ecol = self.energy.map(|e| e.0);
        let (cw, ch) = (
            self.width.div_ceil(k as usize),
            self.height.div_ceil(k as usize),
        );
        let mut coarse = vec![0u32; cw * ch];
        for r in 0..self.table.rows_in(data) {
            if r % (1 << 16) == 0 && cancel() {
                return Err(Error::Cancelled);
            }
            if let (Some((lo, hi)), Some(c)) = (range, ecol) {
                match self.table.f64(data, r, c, 0) {
                    Some(e) if e >= lo && e < hi => {}
                    _ => continue,
                }
            }
            if let Some((i, j)) = self.bin_of(data, r, k) {
                coarse[j * cw + i] += 1;
            }
        }
        let (w, h) = (self.width, self.height);
        let k = k as usize;
        let mut out = vec![0f32; w * h];
        for (row, line) in out.chunks_mut(w).enumerate() {
            let j = (h - 1 - row) / k;
            for (i, v) in line.iter_mut().enumerate() {
                *v = coarse[j * cw + i / k] as f32;
            }
        }
        Ok(out)
    }

    /// The image's WCS header (sky event lists whose columns give one), for the base bin.
    pub fn wcs_header(&self, h: &Header) -> Option<Header> {
        let [Some(a), Some(b)] = &self.wcs else {
            return None;
        };
        let f = |v: f64| format!("{v:.17E}");
        let q = |s: &str| format!("'{}'", s.replace('\'', "''"));
        let mut cards: Vec<(String, String)> = vec![
            ("NAXIS".into(), "2".into()),
            ("NAXIS1".into(), self.width.to_string()),
            ("NAXIS2".into(), self.height.to_string()),
        ];
        for (n, (ax, o)) in [(a, self.x0), (b, self.y0)].into_iter().enumerate() {
            let n = n + 1;
            cards.push((format!("CTYPE{n}"), q(&ax.ctype)));
            cards.push((format!("CRVAL{n}"), f(ax.crval)));
            cards.push((format!("CRPIX{n}"), f((ax.crpix - o) / self.base + 0.5)));
            cards.push((format!("CDELT{n}"), f(ax.cdelt * self.base)));
            if let Some(u) = &ax.cunit {
                cards.push((format!("CUNIT{n}"), q(u)));
            }
        }
        for k in ["RADESYS", "RADECSYS", "EQUINOX", "MJD-OBS", "DATE-OBS"] {
            if let Some(c) = h.cards.iter().find(|c| c.key == k)
                && let Some(v) = &c.value
            {
                cards.push((
                    k.to_owned(),
                    match v {
                        crate::header::Value::Str(s) => q(s),
                        v => v.display(),
                    },
                ));
            }
        }
        Some(Header::from_pairs(
            cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")),
        ))
    }
}

/// Three bands of a `PI` column of unknown scale, holding a third of the events each, and
/// every event.
fn tertiles(data: &[u8], t: &Table, c: usize) -> Vec<Band> {
    let mut v: Vec<f64> = (0..t.rows_in(data))
        .filter_map(|r| t.f64(data, r, c, 0))
        .collect();
    let all = Band {
        range: None,
        kev: None,
    };
    if v.len() < 3 {
        return vec![all];
    }
    v.sort_unstable_by(f64::total_cmp);
    let q = |f: f64| v[((v.len() as f64 * f) as usize).min(v.len() - 1)];
    let (a, b) = (q(1.0 / 3.0), q(2.0 / 3.0));
    let (lo, hi) = (v[0], v[v.len() - 1]);
    let next = |x: f64| {
        x + if x.fract() == 0.0 {
            1.0
        } else {
            f64::EPSILON * x.abs()
        }
    };
    let mut out: Vec<Band> = [(lo, a), (a, b), (b, next(hi))]
        .iter()
        .filter(|(l, h)| h > l)
        .map(|&(l, h)| Band {
            range: Some((l, h)),
            kev: None,
        })
        .collect();
    out.push(all);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdr(cards: &[(&str, &str)]) -> Header {
        Header::from_pairs(cards.iter().map(|(k, v)| (*k, *v, "")))
    }

    /// Events with float X / Y and integer PI (XMM-like, eV).
    fn events(points: &[(f32, f32, i32)]) -> (Hdu, Vec<u8>) {
        let n = points.len().to_string();
        let h = hdr(&[
            ("XTENSION", "'BINTABLE'"),
            ("NAXIS1", "12"),
            ("NAXIS2", &n),
            ("TFIELDS", "3"),
            ("EXTNAME", "'EVENTS'"),
            ("TELESCOP", "'XMM'"),
            ("TTYPE1", "'X'"),
            ("TFORM1", "'E'"),
            ("TTYPE2", "'Y'"),
            ("TFORM2", "'E'"),
            ("TTYPE3", "'PI'"),
            ("TFORM3", "'J'"),
            ("TLMIN1", "0.5"),
            ("TLMIN2", "0.5"),
            ("TCTYP1", "'RA---TAN'"),
            ("TCTYP2", "'DEC--TAN'"),
            ("TCRVL1", "10.0"),
            ("TCRVL2", "20.0"),
            ("TCRPX1", "4.5"),
            ("TCRPX2", "4.5"),
            ("TCDLT1", "-0.001"),
            ("TCDLT2", "0.001"),
        ]);
        let mut d = Vec::new();
        for &(x, y, pi) in points {
            d.extend(x.to_be_bytes());
            d.extend(y.to_be_bytes());
            d.extend(pi.to_be_bytes());
        }
        let hdu = Hdu {
            index: 1,
            kind: crate::Kind::BinTable,
            header: h,
            data_offset: 0,
            data_len: d.len(),
            image: None,
        };
        (hdu, d)
    }

    #[test]
    fn bins_and_bands() {
        let pts = [
            (1.0, 1.0, 600),
            (1.2, 1.4, 1500),
            (2.0, 1.0, 3000),
            (5.0, 3.0, 9000),
            (5.9, 3.9, 800),
        ];
        let (hdu, d) = events(&pts);
        let ev = Events::from_hdu(&d, &hdu).unwrap().unwrap();
        assert_eq!((ev.x0, ev.y0, ev.base), (0.5, 0.5, 1.0));
        assert_eq!((ev.width, ev.height), (6, 4));
        assert_eq!(ev.bands.len(), 4);
        let all = ev.bin(&d, 0, 3, &|| false).unwrap();
        // Bottom row is the last displayed row.
        assert_eq!(all[3 * 6], 2.0);
        assert_eq!(all[3 * 6 + 1], 1.0);
        assert_eq!(all[5], 1.0);
        let soft = ev.bin(&d, 0, 0, &|| false).unwrap();
        assert_eq!(soft.iter().sum::<f32>(), 2.0);
        let x2 = ev.bin(&d, 1, 3, &|| false).unwrap();
        assert_eq!(x2[3 * 6], 3.0);
        assert_eq!(x2[2 * 6 + 1], 3.0);
        assert_eq!(x2.iter().sum::<f32>(), 5.0 * 4.0);
        let w = ev.wcs_header(&hdu.header).unwrap();
        assert_eq!(w.f64("CRPIX1"), Some(4.5));
        assert_eq!(w.f64("CDELT1"), Some(-0.001));
    }
}
