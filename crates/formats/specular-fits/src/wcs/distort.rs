//! Distortions: SIP polynomials (Shupe et al. 2005), the TPV polynomial on intermediate
//! world coordinates (SCAMP / the TPV registration), and the HST lookup tables (`D2IM`
//! detector-to-image tables in `D2IMARR` extensions, `CPDIS` prior-distortion tables in
//! `WCSDVARR` extensions, Paper IV draft conventions as STScI writes them).
//!
//! The pixel-side pipeline, as `astropy.wcs` composes it:
//! `p' = p + D2IM(p)`, then `p'' = p' + SIP(p' − CRPIX) + CPDIS(p')`.

use super::Unsupported;
use crate::header::Header;

/// A SIP polynomial: coefficients `[p][q]` of u^p v^q.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Poly {
    order: usize,
    c: Vec<f64>,
}

impl Poly {
    fn read(h: &Header, name: &str) -> Option<Poly> {
        let order = h.i64(&format!("{name}_ORDER"))?;
        if !(1..=9).contains(&order) {
            return None;
        }
        let order = order as usize;
        let n = order + 1;
        let mut c = vec![0.0; n * n];
        for p in 0..n {
            for q in 0..n - p {
                c[p * n + q] = h.f64(&format!("{name}_{p}_{q}")).unwrap_or(0.0);
            }
        }
        Some(Poly { order, c })
    }

    pub(super) fn eval(&self, u: f64, v: f64) -> f64 {
        let n = self.order + 1;
        let mut sum = 0.0;
        let mut up = 1.0;
        for p in 0..n {
            let mut t = 0.0;
            let mut vq = 1.0;
            for q in 0..n - p {
                t += self.c[p * n + q] * vq;
                vq *= v;
            }
            sum += t * up;
            up *= u;
        }
        sum
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Sip {
    a: Poly,
    b: Poly,
    ap: Option<Poly>,
    bp: Option<Poly>,
}

impl Sip {
    /// SIP when the header has `A_ORDER` and `B_ORDER` (astropy uses them whether or not
    /// `CTYPE` says `-SIP`).
    pub(super) fn read(h: &Header) -> Option<Sip> {
        Some(Sip {
            a: Poly::read(h, "A")?,
            b: Poly::read(h, "B")?,
            ap: Poly::read(h, "AP"),
            bp: Poly::read(h, "BP"),
        })
    }

    /// The SIP offsets at pixel offsets (u, v) from `CRPIX`.
    pub(super) fn delta(&self, u: f64, v: f64) -> (f64, f64) {
        (self.a.eval(u, v), self.b.eval(u, v))
    }

    /// A first guess for the inverse from AP / BP, when given.
    pub(super) fn guess(&self, u1: f64, v1: f64) -> (f64, f64) {
        match (&self.ap, &self.bp) {
            (Some(ap), Some(bp)) => (u1 + ap.eval(u1, v1), v1 + bp.eval(u1, v1)),
            _ => (u1, v1),
        }
    }
}

/// TPV: ξ′ = Σ PV1_k · term_k(ξ, η), η′ = Σ PV2_k · term_k(η, ξ), k = 0..39, on the
/// intermediate world coordinates in degrees.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Tpv {
    /// Coefficients of the longitude and latitude axes.
    c: [[f64; 40]; 2],
    /// Highest used term + 1 (so evaluation stops early).
    n: usize,
}

impl Tpv {
    /// Reads `PVi_k` for the longitude axis `lng` and latitude axis `lat` (0-based).
    pub(super) fn read(h: &Header, lng: usize, lat: usize) -> Tpv {
        let mut c = [[0.0; 40]; 2];
        let mut n = 2;
        for (a, axis) in [lng, lat].into_iter().enumerate() {
            let mut any = false;
            for (k, v) in c[a].iter_mut().enumerate() {
                if let Some(x) = h.f64(&format!("PV{}_{k}", axis + 1)) {
                    *v = x;
                    any = true;
                    if x != 0.0 {
                        n = n.max(k + 1);
                    }
                }
            }
            if !any {
                // No coefficients: the identity.
                c[a][1] = 1.0;
            }
        }
        Tpv { c, n }
    }

    pub(super) fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            tpv_poly(&self.c[0][..self.n], x, y),
            tpv_poly(&self.c[1][..self.n], y, x),
        )
    }

    /// Inverse by Newton's method (numerical Jacobian), from the identity.
    pub(super) fn invert(&self, xt: f64, yt: f64) -> Option<(f64, f64)> {
        let (mut x, mut y) = (xt, yt);
        for _ in 0..50 {
            let (fx, fy) = self.apply(x, y);
            let (ex, ey) = (fx - xt, fy - yt);
            if ex.abs() < 1e-13 && ey.abs() < 1e-13 {
                return Some((x, y));
            }
            let h = 1e-7 * (1.0 + x.abs().max(y.abs()));
            let (ax, ay) = self.apply(x + h, y);
            let (bx, by) = self.apply(x - h, y);
            let (cx, cy) = self.apply(x, y + h);
            let (dx, dy) = self.apply(x, y - h);
            let (j11, j21) = ((ax - bx) / (2.0 * h), (ay - by) / (2.0 * h));
            let (j12, j22) = ((cx - dx) / (2.0 * h), (cy - dy) / (2.0 * h));
            let det = j11 * j22 - j12 * j21;
            if det == 0.0 || !det.is_finite() {
                return None;
            }
            x -= (j22 * ex - j12 * ey) / det;
            y -= (j11 * ey - j21 * ex) / det;
        }
        let (fx, fy) = self.apply(x, y);
        ((fx - xt).abs() < 1e-9 && (fy - yt).abs() < 1e-9).then_some((x, y))
    }
}

/// The TPV terms in order: 1, x, y, r, x², xy, y², x³, x²y, xy², y³, r³, x⁴ … y⁴, x⁵ … y⁵,
/// r⁵, x⁶ … y⁶, x⁷ … y⁷, r⁷.
fn tpv_poly(c: &[f64], x: f64, y: f64) -> f64 {
    let r = x.hypot(y);
    let mut sum = 0.0;
    let mut k = 0;
    let mut add = |coef: Option<&f64>, t: f64| {
        if let Some(&c) = coef {
            sum += c * t;
        }
    };
    // Degree 0 and 1 (with r).
    add(c.first(), 1.0);
    add(c.get(1), x);
    add(c.get(2), y);
    add(c.get(3), r);
    k += 4;
    for d in 2..=7i32 {
        if k >= c.len() {
            break;
        }
        // x^d, x^(d−1) y, …, y^d.
        for j in 0..=d {
            add(c.get(k), x.powi(d - j) * y.powi(j));
            k += 1;
        }
        if d % 2 == 1 {
            add(c.get(k), r.powi(d));
            k += 1;
        }
    }
    sum
}

/// A bilinear lookup table over image pixels: table index = (p − CRVAL)/CDELT + CRPIX − 1,
/// clamped to the table.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub width: usize,
    pub height: usize,
    /// Row-major in FITS order (row 0 is the first FITS row).
    pub values: Vec<f32>,
    pub crpix: [f64; 2],
    pub crval: [f64; 2],
    pub cdelt: [f64; 2],
}

impl Table {
    /// The table's value at 1-based image pixel `p`.
    pub(super) fn at(&self, p: [f64; 2]) -> f64 {
        let dims = [self.width, self.height];
        let mut i0 = [0usize; 2];
        let mut i1 = [0usize; 2];
        let mut w = [0.0; 2];
        for a in 0..2 {
            let n = dims[a].max(1);
            let t = ((p[a] - self.crval[a]) / self.cdelt[a] + self.crpix[a] - 1.0)
                .clamp(0.0, (n - 1) as f64);
            let f = t.floor();
            i0[a] = f as usize;
            i1[a] = (i0[a] + 1).min(n - 1);
            w[a] = t - f;
        }
        let v = |x: usize, y: usize| f64::from(self.values[y * self.width + x]);
        let a = v(i0[0], i0[1]) * (1.0 - w[0]) + v(i1[0], i0[1]) * w[0];
        let b = v(i0[0], i1[1]) * (1.0 - w[0]) + v(i1[0], i1[1]) * w[0];
        a * (1.0 - w[1]) + b * w[1]
    }
}

/// Lookup tables per image axis (x, y).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Lookups {
    pub t: [Option<Table>; 2],
}

impl Lookups {
    pub(super) fn is_empty(&self) -> bool {
        self.t.iter().all(Option::is_none)
    }

    pub(super) fn delta(&self, p: [f64; 2]) -> [f64; 2] {
        [
            self.t[0].as_ref().map_or(0.0, |t| t.at(p)),
            self.t[1].as_ref().map_or(0.0, |t| t.at(p)),
        ]
    }
}

/// The fields of a record-valued keyword (`DP1 = 'EXTVER: 1'`): every card named `key`.
fn records(h: &Header, key: &str) -> Vec<(String, String)> {
    h.cards
        .iter()
        .filter(|c| c.key == key)
        .filter_map(|c| c.value.as_ref()?.as_str())
        .filter_map(|s| {
            let (f, v) = s.split_once(':')?;
            Some((f.trim().to_ascii_uppercase(), v.trim().to_owned()))
        })
        .collect()
}

impl Table {
    /// The table with its rows and columns swapped (values only, as astropy does when a
    /// table's `AXIS.j` names the other image axis).
    fn transposed(self) -> Table {
        let (w, h) = (self.width, self.height);
        let mut values = Vec::with_capacity(w * h);
        for x in 0..w {
            for y in 0..h {
                values.push(self.values[y * w + x]);
            }
        }
        Table {
            width: h,
            height: w,
            values,
            ..self
        }
    }
}

/// Reads the `CPDISj` (records `DPj`, extension `WCSDVARR`) or `D2IMDISj` (records `D2IMj`,
/// extension `D2IMARR`, or the older single-table `AXISCORR` form) lookup tables of the two
/// image axes. `load(extname, extver)` reads a table extension. `Ok(empty)` when the header
/// has none.
pub(super) fn read_lookups(
    h: &Header,
    d2im: bool,
    load: &dyn Fn(&str, i64) -> Option<Table>,
) -> Result<Lookups, Unsupported> {
    let (dis, rec, ext) = if d2im {
        ("D2IMDIS", "D2IM", "D2IMARR")
    } else {
        ("CPDIS", "DP", "WCSDVARR")
    };
    let mut out = Lookups::default();
    if d2im && let Some(axis) = h.i64("AXISCORR") {
        // The old form: one table, D2IMARR version 1, for axis AXISCORR.
        let t = load(ext, 1).ok_or_else(|| Unsupported::Table(format!("{ext},1")))?;
        match axis {
            1 => out.t[0] = Some(t),
            2 => out.t[1] = Some(t),
            _ => return Err(Unsupported::Distortion(format!("AXISCORR {axis}"))),
        }
        return Ok(out);
    }
    for j in 0..2 {
        let Some(kind) = h.str(&format!("{dis}{}", j + 1)) else {
            continue;
        };
        let kind = kind.trim();
        if kind.is_empty() {
            continue;
        }
        if !kind.eq_ignore_ascii_case("lookup") {
            return Err(Unsupported::Distortion(format!("{dis} {kind}")));
        }
        let recs = records(h, &format!("{rec}{}", j + 1));
        let field = |name: &str| {
            recs.iter()
                .find(|(f, _)| f == name)
                .and_then(|(_, v)| v.parse::<f64>().ok())
        };
        let extver = field("EXTVER").map_or(1, |v| v as i64);
        let table =
            load(ext, extver).ok_or_else(|| Unsupported::Table(format!("{ext},{extver}")))?;
        let table = match field(&format!("AXIS.{}", j + 1)) {
            Some(a) if a as usize != j + 1 => table.transposed(),
            _ => table,
        };
        out.t[j] = Some(table);
    }
    // Sequent distortions (CQDIS) are not written by any mission we know of.
    for j in 1..=2 {
        if h.str(&format!("CQDIS{j}"))
            .is_some_and(|s| !s.trim().is_empty())
        {
            return Err(Unsupported::Distortion("CQDIS".into()));
        }
    }
    Ok(out)
}
