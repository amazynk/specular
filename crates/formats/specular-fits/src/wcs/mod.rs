//! World coordinates: FITS WCS Papers I–III (Greisen & Calabretta 2002, Calabretta &
//! Greisen 2002, Greisen et al. 2006), HEALPix (Calabretta & Roukema 2007), and the
//! distortion conventions in use: SIP (Shupe et al. 2005), TPV (and pre-2012 SCAMP `-TAN`
//! headers carrying it), the HST `D2IM` / `CPDIS` lookup tables, and Digitized Sky Survey
//! plate solutions (read as TPV, see `dss`).
//!
//! Pixel → world goes: `D2IM` table offsets → pixel offsets from `CRPIX` → SIP and `CPDIS`
//! offsets → linear transform (`PC` × `CDELT`, else `CD`, else `CDELT` with the `CROTA`
//! rotation) → intermediate world coordinates. Non-celestial axes end there: `CRVAL` + the
//! offset (logarithmic spectral axes `-LOG` apply their exponential, Paper III's
//! non-linear `P2X` algorithms convert through the linear type, see `spectral`; `-TAB`
//! and grism axes are not evaluated). The celestial pair
//! goes through TPV when given, is deprojected to native spherical coordinates by any of
//! Paper II's projections (and HPX / XPH), and is rotated to celestial ones from `CRVAL`,
//! `LONPOLE`, `LATPOLE` and the fiducial point (`PVi_0..4` of the longitude axis).
//!
//! What cannot be followed (IRAF `TNX` / `ZPX`, `TPD` polynomials, a missing lookup
//! table, bad projection parameters) leaves the celestial axes out and says why in
//! [`Wcs::unsupported`]: coordinates are never shown from a half-understood header.
//!
//! Coordinates in this module's display helpers are in the top-down frame of
//! [`crate::read_plane`]: `x` right and `y` down from the image's top-left corner, pixel
//! `(i, j)` covering `[i, i + 1) × [j, j + 1)`.

mod distort;
mod dss;
mod proj;
mod spectral;

use crate::header::Header;
pub use distort::Table;
use distort::{Lookups, Sip, Tpv};
pub use proj::Projection;
use proj::{Params, Prj};
use spectral::Spectral;

const D2R: f64 = std::f64::consts::PI / 180.0;
const R2D: f64 = 180.0 / std::f64::consts::PI;

fn wrap180(a: f64) -> f64 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

fn wrap360(a: f64) -> f64 {
    let a = a % 360.0;
    if a < 0.0 { a + 360.0 } else { a }
}

/// Why a header's celestial coordinates cannot be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// A projection code this reader does not implement (`TNX`, `ZPX`, …); empty when the
    /// celestial axes name none.
    Projection(String),
    /// The projection's parameters (`PVi_m`) are missing or out of range.
    Parameters(String),
    /// A distortion convention this reader does not implement (`TPD`, `CQDIS`, …).
    Distortion(String),
    /// A distortion lookup table the header refers to is not in the file (`WCSDVARR,1`).
    Table(String),
    /// A longitude without its latitude (or the reverse), or the two name different
    /// projections.
    Axes,
    /// Celestial axes in a unit other than an angle.
    Units(String),
    /// `LONPOLE` / `LATPOLE` inconsistent with the reference point.
    Pole,
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unsupported::Projection(c) if c.is_empty() => f.write_str("no projection"),
            Unsupported::Projection(c) => write!(f, "projection {c}"),
            Unsupported::Parameters(c) => write!(f, "{c} parameters"),
            Unsupported::Distortion(d) => write!(f, "distortion {d}"),
            Unsupported::Table(t) => write!(f, "missing table {t}"),
            Unsupported::Axes => f.write_str("unpaired celestial axes"),
            Unsupported::Units(u) => write!(f, "unit {u}"),
            Unsupported::Pole => f.write_str("pole"),
        }
    }
}

/// The celestial frame of the longitude / latitude pair.
#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    /// Equatorial: `RADESYS` (ICRS, FK5, FK4, …) and equinox.
    Equatorial {
        system: String,
        equinox: Option<f64>,
    },
    Galactic,
    Ecliptic,
    /// Some other `xLON` / `xLAT` pair (`HLON`, `SLON`, planetary…).
    Other(String),
}

/// The celestial part of a WCS.
#[derive(Clone, Debug, PartialEq)]
pub struct Celestial {
    /// World axes (0-based) of longitude and latitude.
    pub lng: usize,
    pub lat: usize,
    pub projection: Projection,
    pub frame: Frame,
    prj: Prj,
    /// Celestial coordinates of the native pole, and the native longitude of the celestial
    /// pole (`LONPOLE`), degrees.
    alpha_p: f64,
    delta_p: f64,
    phi_p: f64,
    tpv: Option<Tpv>,
}

/// One world axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    /// `CTYPEi` (trimmed), may be empty.
    pub ctype: String,
    /// `CUNITi` (trimmed), may be empty.
    pub cunit: String,
    pub crval: f64,
}

/// A FITS world coordinate system.
#[derive(Clone, Debug, PartialEq)]
pub struct Wcs {
    pub axes: Vec<Axis>,
    crpix: Vec<f64>,
    /// Row-major `n × n` linear transform, `CDELT` folded in.
    m: Vec<f64>,
    pub celestial: Option<Celestial>,
    /// Why the header's celestial axes are left out, when they are.
    pub unsupported: Option<Unsupported>,
    /// Pixel-side distortions of the first two axes.
    sip: Option<Sip>,
    d2im: Lookups,
    cpdis: Lookups,
    /// Read from a DSS plate solution (as TPV).
    dss: bool,
    /// Image size, for the top-down display frame and the scale at the centre.
    width: usize,
    height: usize,
    /// Each axis' Paper III non-linear spectral algorithm (`F2W`, `V2F`, …), when it has
    /// one this reader evaluates.
    spectral: Vec<Option<Spectral>>,
}

/// Splits `CTYPE` into its 4-character axis name, its algorithm code and anything after
/// (`RA---TAN-SIP` → `RA`, `TAN`, `-SIP`).
fn ctype_parts(ctype: &str) -> (&str, &str, &str) {
    let t = ctype.trim();
    if t.len() >= 8 && t.as_bytes()[4] == b'-' && t.is_char_boundary(8) {
        let name = t[..4].trim_end_matches('-');
        let code = t[5..8].trim_start_matches('-');
        (name, code, &t[8..])
    } else {
        (t, "", "")
    }
}

fn is_lng(name: &str) -> bool {
    name == "RA" || (name.len() == 4 && name.ends_with("LON"))
}

fn is_lat(name: &str) -> bool {
    name == "DEC" || (name.len() == 4 && name.ends_with("LAT"))
}

/// Degrees per unit of a celestial `CUNIT`.
fn angle_unit(u: &str) -> Option<f64> {
    Some(match u.trim() {
        "" | "deg" | "DEG" | "degree" | "degrees" | "DEGREE" | "DEGREES" => 1.0,
        "arcmin" => 1.0 / 60.0,
        "arcsec" => 1.0 / 3600.0,
        "mas" => 1.0 / 3.6e6,
        "rad" => R2D,
        _ => return None,
    })
}

/// Paper III algorithm codes, which make an axis non-linear (those `spectral` evaluates
/// are tried first).
const NONLINEAR: [&str; 15] = [
    "F2W", "F2V", "F2A", "W2F", "W2V", "W2A", "V2F", "V2W", "V2A", "A2F", "A2W", "A2V", "GRI",
    "GRA", "TAB",
];

impl Wcs {
    /// Reads the primary WCS of an image with dimensions `axes`. `None` when the header has
    /// no WCS keywords at all, or a degenerate (singular) transform. Distortion lookup
    /// tables are not available from a header alone: a header that needs them gets
    /// [`Wcs::unsupported`] (see [`Wcs::from_hdu`]).
    pub fn from_header(h: &Header, axes: &[usize]) -> Option<Wcs> {
        Self::from_header_with(h, axes, &|_, _| None)
    }

    /// The WCS of image HDU `hdu` of `fits` (the file's bytes `data`), with the `D2IM` and
    /// `CPDIS` lookup tables from the file's `D2IMARR` / `WCSDVARR` extensions.
    pub fn from_hdu(fits: &crate::Fits, data: &[u8], hdu: &crate::Hdu) -> Option<Wcs> {
        let axes = hdu.image.as_ref()?.axes.clone();
        Self::from_header_with(&hdu.header, &axes, &|ext, ver| {
            load_table(fits, data, ext, ver)
        })
    }

    /// [`Wcs::from_header`] with `load(extname, extver)` reading lookup tables.
    pub fn from_header_with(
        h: &Header,
        axes: &[usize],
        load: &dyn Fn(&str, i64) -> Option<Table>,
    ) -> Option<Wcs> {
        if let Some(t) = dss::to_tpv(h) {
            let w = Self::from_header_with(&t, axes, load)?;
            return Some(Wcs { dss: true, ..w });
        }
        // Keyword lookups through an index: there are O(n²) of them for n axes (NAXIS up
        // to 999) and a scan of the header each was cubic.
        let k = h.keys();
        let n = (k.i64("WCSAXES").unwrap_or(0).clamp(0, 99) as usize).max(axes.len());
        if n < 2 {
            return None;
        }
        let any = (1..=n).any(|i| {
            k.has(&format!("CTYPE{i}"))
                || k.has(&format!("CRVAL{i}"))
                || k.has(&format!("CD{i}_{i}"))
        });
        if !any {
            return None;
        }
        let mut world = Vec::with_capacity(n);
        let mut crpix = Vec::with_capacity(n);
        let mut cdelt = Vec::with_capacity(n);
        for i in 1..=n {
            world.push(Axis {
                ctype: k.str(&format!("CTYPE{i}")).unwrap_or("").trim().to_owned(),
                cunit: k.str(&format!("CUNIT{i}")).unwrap_or("").trim().to_owned(),
                crval: k.f64(&format!("CRVAL{i}")).unwrap_or(0.0),
            });
            crpix.push(k.f64(&format!("CRPIX{i}")).unwrap_or(0.0));
            cdelt.push(
                k.f64(&format!("CDELT{i}"))
                    .filter(|d| *d != 0.0)
                    .unwrap_or(1.0),
            );
        }
        // Celestial axes.
        let mut lng = None;
        let mut lat = None;
        let mut unsupported = None;
        for (i, a) in world.iter().enumerate() {
            let (name, _, _) = ctype_parts(&a.ctype);
            if is_lng(name) {
                if lng.is_some() {
                    unsupported = Some(Unsupported::Axes);
                }
                lng.get_or_insert(i);
            } else if is_lat(name) {
                if lat.is_some() {
                    unsupported = Some(Unsupported::Axes);
                }
                lat.get_or_insert(i);
            }
        }
        // Linear transform: PC wins over CD when a header has both (as wcslib), CROTA only
        // without either.
        let has_cd = (1..=n).any(|i| (1..=n).any(|j| k.has(&format!("CD{i}_{j}"))));
        let has_pc = (1..=n).any(|i| {
            (1..=n).any(|j| k.has(&format!("PC{i}_{j}")) || k.has(&format!("PC{i:03}{j:03}")))
        });
        let mut m = vec![0.0; n * n];
        if has_cd && !has_pc {
            for i in 0..n {
                for j in 0..n {
                    m[i * n + j] = k.f64(&format!("CD{}_{}", i + 1, j + 1)).unwrap_or(0.0);
                }
            }
            // Axes the CD matrix leaves out (spectral axes of a cube with a 2×2 CD) keep
            // their CDELT, as wcslib does for a zero row.
            for i in 0..n {
                if (0..n).all(|j| m[i * n + j] == 0.0) {
                    m[i * n + i] = cdelt[i];
                }
            }
        } else {
            for i in 0..n {
                for j in 0..n {
                    let pc = k
                        .f64(&format!("PC{}_{}", i + 1, j + 1))
                        .or_else(|| k.f64(&format!("PC{:03}{:03}", i + 1, j + 1)))
                        .unwrap_or(if i == j { 1.0 } else { 0.0 });
                    m[i * n + j] = cdelt[i] * pc;
                }
            }
            if !has_pc && let (Some(lo), Some(la)) = (lng, lat) {
                let rho = k
                    .f64(&format!("CROTA{}", la + 1))
                    .or_else(|| k.f64(&format!("CROTA{}", lo + 1)))
                    .unwrap_or(0.0);
                if rho != 0.0 {
                    let (s, c) = (rho * D2R).sin_cos();
                    // Rotation of the (lng, lat) pair, as AIPS: x = Δ₁c − Δ₂s, y = Δ₁s + Δ₂c
                    // with Δ the CDELT-scaled offsets.
                    let (dl, da) = (cdelt[lo], cdelt[la]);
                    m[lo * n + lo] = dl * c;
                    m[lo * n + la] = -da * s;
                    m[la * n + lo] = dl * s;
                    m[la * n + la] = da * c;
                }
            }
        }
        let mut celestial = None;
        let mut sip = None;
        let mut d2im = Lookups::default();
        let mut cpdis = Lookups::default();
        let cel = match (lng, lat) {
            _ if unsupported.is_some() => Err(Unsupported::Axes),
            // Plain `RA` / `DEC` with no projection code are linear axes, as in wcslib.
            (Some(lo), Some(la))
                if ctype_parts(&world[lo].ctype).1.is_empty()
                    && ctype_parts(&world[la].ctype).1.is_empty() =>
            {
                Ok(None)
            }
            (Some(lo), Some(la)) => Ok(Some((lo, la))),
            (None, None) => Ok(None),
            // A lone RA / DEC with a projection code is half a celestial pair.
            (Some(i), None) | (None, Some(i)) => {
                if ctype_parts(&world[i].ctype).1.is_empty() {
                    Ok(None)
                } else {
                    Err(Unsupported::Axes)
                }
            }
        };
        match cel.and_then(|c| match c {
            Some((lo, la)) => celestial_axes(h, &mut world, &mut m, n, lo, la, load).map(Some),
            None => Ok(None),
        }) {
            Ok(Some(c)) => {
                celestial = Some(c.0);
                sip = c.1;
                d2im = c.2;
                cpdis = c.3;
            }
            Ok(None) => {}
            Err(e) => unsupported = Some(e),
        }
        let restfrq = k.f64("RESTFRQ").or_else(|| k.f64("RESTFREQ"));
        let restwav = k.f64("RESTWAV");
        let spectral = world
            .iter()
            .map(|a| {
                let (name, code, _) = ctype_parts(&a.ctype);
                Spectral::new(name, code, &a.cunit, a.crval, restfrq, restwav)
            })
            .collect();
        let wcs = Wcs {
            spectral,
            axes: world,
            crpix,
            m,
            celestial,
            unsupported,
            sip,
            d2im,
            cpdis,
            dss: false,
            width: axes.first().copied().unwrap_or(1),
            height: axes.get(1).copied().unwrap_or(1),
        };
        let wcs = Wcs {
            unsupported: if wcs.celestial.is_some() {
                None
            } else {
                wcs.unsupported.clone()
            },
            ..wcs
        };
        (wcs.det2().abs() > 0.0).then_some(wcs)
    }

    fn n(&self) -> usize {
        self.axes.len()
    }

    /// Determinant of the image plane's 2 × 2 block (celestial rows when present).
    fn det2(&self) -> f64 {
        let n = self.n();
        let (r0, r1) = self.plane_rows();
        self.m[r0 * n] * self.m[r1 * n + 1] - self.m[r0 * n + 1] * self.m[r1 * n]
    }

    /// The world rows mapped from the image plane: the celestial pair, else the first two.
    fn plane_rows(&self) -> (usize, usize) {
        match &self.celestial {
            Some(c) => (c.lng, c.lat),
            None => (0, 1),
        }
    }

    /// The distortions applied, by convention name (`SIP`, `TPV`, `D2IM`, `CPDIS`, `DSS`).
    pub fn distortions(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.dss {
            v.push("DSS");
        } else if self.celestial.as_ref().is_some_and(|c| c.tpv.is_some()) {
            v.push("TPV");
        }
        if self.sip.is_some() {
            v.push("SIP");
        }
        if !self.d2im.is_empty() {
            v.push("D2IM");
        }
        if !self.cpdis.is_empty() {
            v.push("CPDIS");
        }
        v
    }

    fn distorted(&self) -> bool {
        self.sip.is_some() || !self.d2im.is_empty() || !self.cpdis.is_empty()
    }

    /// Pixel-side distortion: 1-based pixel (x, y) → offsets from `CRPIX` in the linear
    /// transform's input.
    fn pixel_offsets(&self, p: [f64; 2]) -> [f64; 2] {
        let mut q = p;
        if !self.d2im.is_empty() {
            let d = self.d2im.delta(p);
            q = [p[0] + d[0], p[1] + d[1]];
        }
        let mut d = [q[0] - self.crpix[0], q[1] - self.crpix[1]];
        let base = d;
        if let Some(s) = &self.sip {
            let (du, dv) = s.delta(base[0], base[1]);
            d[0] += du;
            d[1] += dv;
        }
        if !self.cpdis.is_empty() {
            let c = self.cpdis.delta(q);
            d[0] += c[0];
            d[1] += c[1];
        }
        d
    }

    /// The inverse of [`Self::pixel_offsets`] by Newton's method, started from SIP's AP / BP.
    fn invert_offsets(&self, t: [f64; 2]) -> Option<[f64; 2]> {
        let c = [self.crpix[0], self.crpix[1]];
        if !self.distorted() {
            return Some([t[0] + c[0], t[1] + c[1]]);
        }
        let g = self
            .sip
            .as_ref()
            .map_or((t[0], t[1]), |s| s.guess(t[0], t[1]));
        let mut p = [g.0 + c[0], g.1 + c[1]];
        let e = 1e-3;
        for _ in 0..40 {
            let f = self.pixel_offsets(p);
            let (fu, fv) = (f[0] - t[0], f[1] - t[1]);
            if fu.abs() < 1e-11 && fv.abs() < 1e-11 {
                return Some(p);
            }
            let a = self.pixel_offsets([p[0] + e, p[1]]);
            let b = self.pixel_offsets([p[0] - e, p[1]]);
            let cc = self.pixel_offsets([p[0], p[1] + e]);
            let d = self.pixel_offsets([p[0], p[1] - e]);
            let (j11, j21) = ((a[0] - b[0]) / (2.0 * e), (a[1] - b[1]) / (2.0 * e));
            let (j12, j22) = ((cc[0] - d[0]) / (2.0 * e), (cc[1] - d[1]) / (2.0 * e));
            let det = j11 * j22 - j12 * j21;
            if det == 0.0 || !det.is_finite() {
                return None;
            }
            p[0] -= (j22 * fu - j12 * fv) / det;
            p[1] -= (j11 * fv - j21 * fu) / det;
        }
        let f = self.pixel_offsets(p);
        ((f[0] - t[0]).abs() < 1e-6 && (f[1] - t[1]).abs() < 1e-6).then_some(p)
    }

    /// World coordinates of FITS pixel coordinates `p` (1-based, one per axis; missing axes
    /// at their first pixel). Celestial axes in degrees, longitude in [0, 360). `None` where
    /// the celestial projection is undefined; non-linear spectral axes this reader does not
    /// evaluate come back NaN.
    pub fn pixel_to_world(&self, p: &[f64]) -> Option<Vec<f64>> {
        let n = self.n();
        let mut d: Vec<f64> = (0..n)
            .map(|i| p.get(i).copied().unwrap_or(1.0) - self.crpix[i])
            .collect();
        if self.distorted() {
            let o = self.pixel_offsets([
                p.first().copied().unwrap_or(1.0),
                p.get(1).copied().unwrap_or(1.0),
            ]);
            d[0] = o[0];
            d[1] = o[1];
        }
        let mut x: Vec<f64> = (0..n)
            .map(|i| (0..n).map(|j| self.m[i * n + j] * d[j]).sum())
            .collect();
        if let Some(c) = &self.celestial {
            let (mut xl, mut xa) = (x[c.lng], x[c.lat]);
            if let Some(t) = &c.tpv {
                (xl, xa) = t.apply(xl, xa);
            }
            let (phi, theta) = c.prj.deproject(xl, xa)?;
            let (a, dd) = c.native_to_celestial(phi, theta);
            if !(a.is_finite() && dd.is_finite()) {
                return None;
            }
            x[c.lng] = a;
            x[c.lat] = dd;
        }
        for (i, v) in x.iter_mut().enumerate() {
            if self
                .celestial
                .as_ref()
                .is_none_or(|c| i != c.lng && i != c.lat)
            {
                *v = self.linear_world(i, *v);
            }
        }
        Some(x)
    }

    /// A non-celestial axis' world value from its intermediate coordinate.
    fn linear_world(&self, i: usize, w: f64) -> f64 {
        let a = &self.axes[i];
        let (name, code, _) = ctype_parts(&a.ctype);
        if code == "LOG" {
            // Paper III: S = S_r · exp(w / S_r).
            return if a.crval == 0.0 {
                f64::NAN
            } else {
                a.crval * (w / a.crval).exp()
            };
        }
        if let Some(s) = self.spectral.get(i).and_then(Option::as_ref) {
            return s.world(w);
        }
        if NONLINEAR.contains(&code) || name == "FELO" {
            return f64::NAN;
        }
        a.crval + w
    }

    /// FITS pixel coordinates of the image plane where the celestial coordinates
    /// `(lng, lat)` fall, with the other pixel axes at `rest` (1-based). `None` without a
    /// celestial WCS or where the projection is undefined.
    pub fn world_to_pixel(&self, lng: f64, lat: f64, rest: &[f64]) -> Option<(f64, f64)> {
        let c = self.celestial.as_ref()?;
        let n = self.n();
        let (phi, theta) = c.celestial_to_native(lng, lat);
        let (mut x, mut y) = c.prj.project(phi, theta)?;
        if let Some(t) = &c.tpv {
            (x, y) = t.invert(x, y)?;
        }
        // Solve the lng / lat rows for the first two pixel offsets, the others fixed.
        let d_rest: Vec<f64> = (2..n)
            .map(|j| rest.get(j - 2).copied().unwrap_or(1.0) - self.crpix[j])
            .collect();
        let row = |i: usize| -> f64 {
            (2..n)
                .map(|j| self.m[i * n + j] * d_rest[j - 2])
                .sum::<f64>()
        };
        let (bx, by) = (x - row(c.lng), y - row(c.lat));
        let (a, b, cc, dd) = (
            self.m[c.lng * n],
            self.m[c.lng * n + 1],
            self.m[c.lat * n],
            self.m[c.lat * n + 1],
        );
        let det = a * dd - b * cc;
        if det == 0.0 {
            return None;
        }
        let u1 = (dd * bx - b * by) / det;
        let v1 = (a * by - cc * bx) / det;
        let p = self.invert_offsets([u1, v1])?;
        Some((p[0], p[1]))
    }

    /// Celestial coordinates (degrees) at the display position `(x, y)` (top-down frame),
    /// with the other axes at the 0-based positions `rest`.
    pub fn sky_at(&self, x: f64, y: f64, rest: &[usize]) -> Option<(f64, f64)> {
        let c = self.celestial.as_ref()?;
        let w = self.pixel_to_world(&self.fits_pixel(x, y, rest))?;
        Some((w[c.lng], w[c.lat]))
    }

    /// The display position (top-down frame) of celestial coordinates `(lng, lat)`.
    pub fn display_of(&self, lng: f64, lat: f64, rest: &[usize]) -> Option<(f64, f64)> {
        let r: Vec<f64> = rest.iter().map(|&k| k as f64 + 1.0).collect();
        let (p1, p2) = self.world_to_pixel(lng, lat, &r)?;
        Some((p1 - 0.5, self.height as f64 + 0.5 - p2))
    }

    /// World coordinates of every axis at a display position.
    pub fn world_at(&self, x: f64, y: f64, rest: &[usize]) -> Option<Vec<f64>> {
        self.pixel_to_world(&self.fits_pixel(x, y, rest))
    }

    fn fits_pixel(&self, x: f64, y: f64, rest: &[usize]) -> Vec<f64> {
        let mut p = vec![x + 0.5, self.height as f64 - y + 0.5];
        p.extend(rest.iter().map(|&k| k as f64 + 1.0));
        p
    }

    /// World value of non-spatial axis `axis` (0-based, ≥ 2) at 0-based pixel `index`, with
    /// the image plane at its reference pixel. `None` for celestial axes and for non-linear
    /// algorithms this reader does not evaluate.
    pub fn axis_value(&self, axis: usize, index: usize) -> Option<f64> {
        let n = self.n();
        if axis >= n
            || self
                .celestial
                .as_ref()
                .is_some_and(|c| axis == c.lng || axis == c.lat)
        {
            return None;
        }
        let d = index as f64 + 1.0 - self.crpix[axis];
        let v = self.linear_world(axis, self.m[axis * n + axis] * d);
        v.is_finite().then_some(v)
    }

    /// Pixel scale of the celestial plane in arcseconds per pixel at the image's centre
    /// (the square root of the sky area one pixel covers there, distortions and projection
    /// included). `None` without a celestial WCS.
    pub fn arcsec_per_pixel(&self) -> Option<f64> {
        let rest = vec![0; self.n().saturating_sub(2)];
        self.scale_at(self.width as f64 / 2.0, self.height as f64 / 2.0, &rest)
            .or_else(|| {
                // The centre is off the projection (an all-sky map's corner): the linear part.
                self.celestial.as_ref()?;
                let s = self.det2().abs().sqrt() * 3600.0;
                (s.is_finite() && s > 0.0).then_some(s)
            })
    }

    /// Arcseconds per pixel at display position `(x, y)`: the square root of the solid
    /// angle of the parallelogram spanned by one pixel step in x and in y.
    pub fn scale_at(&self, x: f64, y: f64, rest: &[usize]) -> Option<f64> {
        let h = 0.5;
        let v = |x: f64, y: f64| -> Option<[f64; 3]> {
            let (l, b) = self.sky_at(x, y, rest)?;
            let (sl, cl) = (l * D2R).sin_cos();
            let (sb, cb) = (b * D2R).sin_cos();
            Some([cb * cl, cb * sl, sb])
        };
        let (x0, x1) = (v(x - h, y)?, v(x + h, y)?);
        let (y0, y1) = (v(x, y - h)?, v(x, y + h)?);
        let a = [x1[0] - x0[0], x1[1] - x0[1], x1[2] - x0[2]];
        let b = [y1[0] - y0[0], y1[1] - y0[1], y1[2] - y0[2]];
        let cr = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let area = (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt();
        let s = area.sqrt() * R2D * 3600.0;
        (s.is_finite() && s > 0.0).then_some(s)
    }
}

/// Reads lookup table extension `ext` (`WCSDVARR`, `D2IMARR`) version `ver` of `fits`.
fn load_table(fits: &crate::Fits, data: &[u8], ext: &str, ver: i64) -> Option<Table> {
    let hdu = fits.hdus.iter().find(|h| {
        h.header
            .str("EXTNAME")
            .is_some_and(|n| n.trim().eq_ignore_ascii_case(ext))
            && h.header.i64("EXTVER").unwrap_or(1) == ver
    })?;
    hdu.image.as_ref()?;
    let plane = crate::read_plane(data, hdu, 0).ok()?;
    if plane.truncated || plane.values.is_empty() {
        return None;
    }
    let (w, ht) = (plane.width, plane.height);
    // read_plane is top-down; tables are indexed in FITS row order.
    let mut values = Vec::with_capacity(w * ht);
    for r in (0..ht).rev() {
        values.extend_from_slice(&plane.values[r * w..(r + 1) * w]);
    }
    let hh = &hdu.header;
    Some(Table {
        width: w,
        height: ht,
        values,
        crpix: [
            hh.f64("CRPIX1").unwrap_or(0.0),
            hh.f64("CRPIX2").unwrap_or(0.0),
        ],
        crval: [
            hh.f64("CRVAL1").unwrap_or(0.0),
            hh.f64("CRVAL2").unwrap_or(0.0),
        ],
        cdelt: [
            hh.f64("CDELT1").filter(|d| *d != 0.0).unwrap_or(1.0),
            hh.f64("CDELT2").filter(|d| *d != 0.0).unwrap_or(1.0),
        ],
    })
}

type CelParts = (Celestial, Option<Sip>, Lookups, Lookups);

/// The celestial pair at world axes `lng`, `lat`: units to degrees, projection and its
/// parameters, distortions, the native pole.
fn celestial_axes(
    h: &Header,
    world: &mut [Axis],
    m: &mut [f64],
    n: usize,
    lng: usize,
    lat: usize,
    load: &dyn Fn(&str, i64) -> Option<Table>,
) -> Result<CelParts, Unsupported> {
    let (_, code, suffix) = ctype_parts(&world[lng].ctype);
    let (_, code2, suffix2) = ctype_parts(&world[lat].ctype);
    let (code, suffix, code2, suffix2) = (
        code.to_owned(),
        suffix.to_owned(),
        code2.to_owned(),
        suffix2.to_owned(),
    );
    let (n1, n2) = (
        ctype_parts(&world[lng].ctype).0,
        ctype_parts(&world[lat].ctype).0,
    );
    let paired = (n1 == "RA" && n2 == "DEC") || (n1.len() == 4 && n1[..2] == n2[..2]);
    if code != code2 || !paired {
        return Err(Unsupported::Axes);
    }
    for s in [&suffix, &suffix2] {
        if !s.is_empty() && s != "-SIP" {
            return Err(Unsupported::Distortion(
                s.trim_start_matches('-').to_owned(),
            ));
        }
    }
    // Celestial axes in degrees.
    for i in [lng, lat] {
        let f = angle_unit(&world[i].cunit)
            .ok_or_else(|| Unsupported::Units(world[i].cunit.clone()))?;
        if f != 1.0 {
            world[i].crval *= f;
            for v in &mut m[i * n..(i + 1) * n] {
                *v *= f;
            }
        }
    }
    // Pre-2012 SCAMP headers: `-TAN` carrying TPV's polynomial (a `PVi_m` with m ≥ 5 on a
    // celestial axis) is TPV, SIP ignored (Shupe et al. 2012, §7; astropy does the same).
    let scamp = || {
        let prefixes = [format!("PV{}_", lng + 1), format!("PV{}_", lat + 1)];
        h.cards.iter().any(|c| {
            prefixes.iter().any(|p| {
                c.key
                    .strip_prefix(p.as_str())
                    .and_then(|m| m.parse::<u32>().ok())
                    .is_some_and(|m| m >= 5)
            })
        })
    };
    let tpv = code == "TPV" || (code == "TAN" && suffix.is_empty() && scamp());
    // `-TAN-SIP` with SCAMP's `PVi_m` beside it: the SIP polynomial already carries the
    // distortion and the PVs are redundant (astropy drops them; read as projection
    // parameters they would move the fiducial point).
    let sip_pv = code == "TAN" && suffix == "-SIP";
    let projection = if tpv {
        Projection::Tan
    } else {
        Projection::from_code(&code).ok_or_else(|| Unsupported::Projection(code.clone()))?
    };
    let (a0, d0) = (world[lng].crval, world[lat].crval);
    let pv_lat = |m: usize| {
        h.f64(&format!("PV{}_{m}", lat + 1))
            .or_else(|| (m >= 1).then(|| h.f64(&format!("PROJP{m}"))).flatten())
    };
    let pv_lng = |m: usize| {
        if tpv || sip_pv {
            None
        } else {
            h.f64(&format!("PV{}_{m}", lng + 1))
        }
    };
    let none = |_: usize| None;
    let params = Params {
        pv: if tpv || sip_pv {
            &none as &dyn Fn(usize) -> Option<f64>
        } else {
            &pv_lat
        },
        delta0: d0,
    };
    let fiducial = (pv_lng(1), pv_lng(2), pv_lng(0).is_some_and(|v| v != 0.0));
    let prj = Prj::new(projection, &params, fiducial)?;
    let (phi0, theta0) = (prj.phi0, prj.theta0);
    let lonpole = h
        .f64("LONPOLE")
        .or_else(|| pv_lng(3))
        .unwrap_or(if d0 >= theta0 { phi0 } else { phi0 + 180.0 });
    let latpole = h.f64("LATPOLE").or_else(|| pv_lng(4)).unwrap_or(90.0);
    let (alpha_p, delta_p) = if theta0 == 90.0 {
        (a0, d0)
    } else {
        pole(a0, d0, phi0, theta0, lonpole, latpole).ok_or(Unsupported::Pole)?
    };
    let (name, _, _) = ctype_parts(&world[lng].ctype);
    let frame = match name {
        "RA" => {
            let equinox = h.f64("EQUINOX").or_else(|| h.f64("EPOCH"));
            let system = h
                .str("RADESYS")
                .or_else(|| h.str("RADECSYS"))
                .map(|s| s.trim().to_owned())
                .unwrap_or_else(|| match equinox {
                    Some(e) if e < 1984.0 => "FK4".into(),
                    Some(_) => "FK5".into(),
                    None => "ICRS".into(),
                });
            Frame::Equatorial { system, equinox }
        }
        "GLON" => Frame::Galactic,
        "ELON" => Frame::Ecliptic,
        other => Frame::Other(other.to_owned()),
    };
    // Pixel-side distortions apply to the first two pixel axes.
    let sip = (!tpv).then(|| Sip::read(h)).flatten();
    let d2im = distort::read_lookups(h, true, load)?;
    let cpdis = distort::read_lookups(h, false, load)?;
    let tpv = tpv.then(|| Tpv::read(h, lng, lat));
    Ok((
        Celestial {
            lng,
            lat,
            projection,
            frame,
            prj,
            alpha_p,
            delta_p,
            phi_p: lonpole,
            tpv,
        },
        sip,
        d2im,
        cpdis,
    ))
}

impl Celestial {
    fn native_to_celestial(&self, phi: f64, theta: f64) -> (f64, f64) {
        let (st, ct) = (theta * D2R).sin_cos();
        let (sdp, cdp) = (self.delta_p * D2R).sin_cos();
        let (sd, cd) = ((phi - self.phi_p) * D2R).sin_cos();
        // Latitudes from atan2 of the unit vector, not asin: asin loses half the digits
        // near ±90°, which a TAN image's native latitudes all are.
        let (x, y) = (st * cdp - ct * sdp * cd, -ct * sd);
        let a = self.alpha_p + y.atan2(x) * R2D;
        let d = (st * sdp + ct * cdp * cd).atan2(x.hypot(y)) * R2D;
        (wrap360(a), d)
    }

    fn celestial_to_native(&self, a: f64, d: f64) -> (f64, f64) {
        let (sd, cd) = (d * D2R).sin_cos();
        let (sdp, cdp) = (self.delta_p * D2R).sin_cos();
        let (sa, ca) = ((a - self.alpha_p) * D2R).sin_cos();
        let (x, y) = (sd * cdp - cd * sdp * ca, -cd * sa);
        let phi = self.phi_p + y.atan2(x) * R2D;
        let theta = (sd * sdp + cd * cdp * ca).atan2(x.hypot(y)) * R2D;
        (phi, theta)
    }
}

/// Celestial coordinates of the native pole for a reference point off the pole (θ₀ ≠ 90°),
/// Paper II §2.4 / eq. 8–10, choosing the solution nearer `LATPOLE`.
fn pole(a0: f64, d0: f64, phi0: f64, theta0: f64, phi_p: f64, latpole: f64) -> Option<(f64, f64)> {
    let tol = 1e-10;
    let (sd0, cd0) = (d0 * D2R).sin_cos();
    let (sphip, cphip) = ((phi_p - phi0) * D2R).sin_cos();
    let (sth0, cth0) = (theta0 * D2R).sin_cos();
    let x = cth0 * cphip;
    let y = sth0;
    let z = x.hypot(y);
    let mut delta_p;
    if z == 0.0 {
        if sd0 != 0.0 {
            return None;
        }
        delta_p = latpole;
    } else {
        let r = sd0 / z;
        if r.abs() > 1.0 + tol {
            return None;
        }
        let u = y.atan2(x) * R2D;
        let v = r.clamp(-1.0, 1.0).acos() * R2D;
        let mut p1 = u + v;
        let mut p2 = u - v;
        if p1 > 180.0 {
            p1 -= 360.0;
        }
        if p2 < -180.0 {
            p2 += 360.0;
        }
        let ok1 = p1.abs() <= 90.0 + tol;
        let ok2 = p2.abs() <= 90.0 + tol;
        delta_p = match (ok1, ok2) {
            (true, true) => {
                if (latpole - p1).abs() <= (latpole - p2).abs() {
                    p1
                } else {
                    p2
                }
            }
            (true, false) => p1,
            (false, true) => p2,
            (false, false) => return None,
        };
        delta_p = delta_p.clamp(-90.0, 90.0);
    }
    let zz = (delta_p * D2R).cos() * cd0;
    let alpha_p = if zz.abs() < tol {
        if cd0.abs() < tol {
            a0
        } else if delta_p > 0.0 {
            delta_p = 90.0;
            a0 + phi_p - phi0 - 180.0
        } else {
            delta_p = -90.0;
            a0 - phi_p + phi0
        }
    } else {
        let xx = (sth0 - (delta_p * D2R).sin() * sd0) / zz;
        let yy = sphip * cth0 / cd0;
        if xx == 0.0 && yy == 0.0 {
            return None;
        }
        a0 - yy.atan2(xx) * R2D
    };
    Some((alpha_p, delta_p))
}

/// Right ascension in degrees as `hh:mm:ss.ss`.
pub fn hms(deg: f64) -> String {
    let cs = (wrap360(deg) / 15.0 * 360000.0).round() as i64 % (24 * 360000);
    format!(
        "{:02}:{:02}:{:02}.{:02}",
        cs / 360000,
        cs / 6000 % 60,
        cs / 100 % 60,
        cs % 100
    )
}

/// A latitude in degrees as `±dd:mm:ss.s`.
pub fn dms(deg: f64) -> String {
    let sign = if deg < 0.0 { '-' } else { '+' };
    let ds = (deg.abs() * 36000.0).round() as i64;
    format!(
        "{sign}{:02}:{:02}:{:02}.{}",
        ds / 36000,
        ds / 600 % 60,
        ds / 10 % 60,
        ds % 10
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdr(cards: &[(&str, &str)]) -> Header {
        Header::from_pairs(cards.iter().map(|(k, v)| (*k, *v, "")))
    }

    #[test]
    fn sexagesimal() {
        assert_eq!(hms(0.0), "00:00:00.00");
        assert_eq!(hms(359.99999999), "00:00:00.00");
        assert_eq!(hms(83.63308), "05:34:31.94");
        assert_eq!(dms(22.0145), "+22:00:52.2");
        assert_eq!(dms(-0.5), "-00:30:00.0");
        assert_eq!(dms(-29.9999999), "-30:00:00.0");
    }

    #[test]
    fn tan_reference_point() {
        let h = hdr(&[
            ("CTYPE1", "'RA---TAN'"),
            ("CTYPE2", "'DEC--TAN'"),
            ("CRVAL1", "83.6"),
            ("CRVAL2", "22.0"),
            ("CRPIX1", "50.5"),
            ("CRPIX2", "50.5"),
            ("CDELT1", "-0.001"),
            ("CDELT2", "0.001"),
        ]);
        let w = Wcs::from_header(&h, &[100, 100]).unwrap();
        let r = w.pixel_to_world(&[50.5, 50.5]).unwrap();
        assert!((r[0] - 83.6).abs() < 1e-12 && (r[1] - 22.0).abs() < 1e-12);
        let p = w.world_to_pixel(83.61, 22.01, &[]).unwrap();
        let back = w.pixel_to_world(&[p.0, p.1]).unwrap();
        assert!((back[0] - 83.61).abs() < 1e-10 && (back[1] - 22.01).abs() < 1e-10);
        assert!((w.arcsec_per_pixel().unwrap() - 3.6).abs() < 1e-9);
        // Display frame: the centre of the top-left pixel is FITS (1, 100).
        let s = w.sky_at(0.5, 0.5, &[]).unwrap();
        let f = w.pixel_to_world(&[1.0, 100.0]).unwrap();
        assert_eq!((s.0, s.1), (f[0], f[1]));
    }

    /// A radio cube's third axis, optical velocity sampled linearly in frequency
    /// (`VOPT-F2W`), gives velocities; `-TAB` still gives none.
    #[test]
    fn nonlinear_spectral_axis() {
        let cube = |ctype3: &str| {
            Wcs::from_header(
                &hdr(&[
                    ("CTYPE1", "'RA---TAN'"),
                    ("CTYPE2", "'DEC--TAN'"),
                    ("CTYPE3", ctype3),
                    ("CUNIT3", "'km/s'"),
                    ("CRVAL3", "1000.0"),
                    ("CRPIX3", "1.0"),
                    ("CDELT3", "-2.0"),
                    ("RESTFRQ", "1.420405751768E9"),
                ]),
                &[10, 10, 5],
            )
            .unwrap()
        };
        let w = cube("'VOPT-F2W'");
        let (c, nu0) = (299_792.458_f64, 1.420_405_751_768e9_f64);
        // ν linear in the channel: ν = ν(V_r) + k·dν/dV·ΔV.
        let nu_r = nu0 / (1.0 + 1000.0 / c);
        let dnu = -nu0 / c / (1.0 + 1000.0 / c).powi(2) * -2.0;
        assert!((w.axis_value(2, 0).unwrap() - 1000.0).abs() < 1e-9);
        for k in 1..5u32 {
            let nu = nu_r + f64::from(k) * dnu;
            let want = c * (nu0 / nu - 1.0);
            let got = w.axis_value(2, k as usize).unwrap();
            assert!((got - want).abs() < 1e-9, "{k}: {got} {want}");
            // Not the linear value: the curvature is a few m/s over 8 km/s.
            assert!((got - (1000.0 - 2.0 * f64::from(k))).abs() > 1e-6);
        }
        assert_eq!(cube("'VOPT-TAB'").axis_value(2, 1), None);
        assert_eq!(cube("'VOPT-F2V'").axis_value(2, 1), None);
    }

    /// Headers this reader cannot follow keep their linear axes, drop the celestial ones and
    /// say why.
    #[test]
    fn unsupported_reasons() {
        let base = |extra: &[(&str, &str)], code: &str| {
            let t1 = format!("'RA---{code}'");
            let t2 = format!("'DEC--{code}'");
            let mut c = vec![
                ("CTYPE1", t1.as_str()),
                ("CTYPE2", t2.as_str()),
                ("CRVAL1", "10.0"),
                ("CRVAL2", "20.0"),
                ("CDELT1", "-0.001"),
                ("CDELT2", "0.001"),
            ];
            c.extend_from_slice(extra);
            let w = Wcs::from_header(&hdr(&c), &[100, 100]).unwrap();
            assert!(w.celestial.is_none());
            assert!(w.sky_at(1.0, 1.0, &[]).is_none() && w.arcsec_per_pixel().is_none());
            w.unsupported.unwrap().to_string()
        };
        assert_eq!(base(&[], "TNX"), "projection TNX");
        assert_eq!(base(&[], "ZPX"), "projection ZPX");
        assert_eq!(base(&[("PV2_1", "-1.0")], "AZP"), "AZP parameters");
        assert_eq!(
            base(&[("CPDIS1", "'Polynomial'")], "TAN"),
            "distortion CPDIS Polynomial"
        );
        assert_eq!(
            base(&[("CPDIS1", "'Lookup'"), ("DP1", "'EXTVER: 3'")], "TAN"),
            "missing table WCSDVARR,3"
        );
        assert_eq!(base(&[("CUNIT1", "'pc'")], "TAN"), "unit pc");
        let t1 = "'RA---TAN-TPD'";
        let w = Wcs::from_header(
            &hdr(&[("CTYPE1", t1), ("CTYPE2", "'DEC--TAN-TPD'")]),
            &[10, 10],
        )
        .unwrap();
        assert_eq!(w.unsupported.unwrap().to_string(), "distortion TPD");
        let w = Wcs::from_header(
            &hdr(&[("CTYPE1", "'RA---TAN'"), ("CTYPE2", "'GLAT-TAN'")]),
            &[10, 10],
        )
        .unwrap();
        assert_eq!(w.unsupported, Some(Unsupported::Axes));
    }

    /// `from_hdu` finds a `CPDIS` table in its `WCSDVARR` extension, in FITS row order.
    #[test]
    fn lookup_table_from_file() {
        fn unit(cards: &[String], data: &[u8]) -> Vec<u8> {
            let mut b: Vec<u8> = Vec::new();
            for c in cards.iter().map(String::as_str).chain(["END"]) {
                let mut c = c.as_bytes().to_vec();
                c.resize(80, b' ');
                b.extend(c);
            }
            b.resize(b.len().div_ceil(2880) * 2880, b' ');
            let start = b.len();
            b.extend_from_slice(data);
            b.resize(start + data.len().div_ceil(2880) * 2880, 0);
            b
        }
        let kv = |k: &str, v: &str| format!("{k:<8}= {v:>20}");
        let wcs = [
            kv("CTYPE1", "'RA---TAN'"),
            kv("CTYPE2", "'DEC--TAN'"),
            kv("CRVAL1", "10.0"),
            kv("CRVAL2", "20.0"),
            kv("CRPIX1", "4.0"),
            kv("CRPIX2", "3.0"),
            kv("CDELT1", "-0.001"),
            kv("CDELT2", "0.001"),
        ];
        let mut primary = vec![
            kv("SIMPLE", "T"),
            kv("BITPIX", "-32"),
            kv("NAXIS", "2"),
            kv("NAXIS1", "8"),
            kv("NAXIS2", "6"),
        ];
        primary.extend(wcs.iter().cloned());
        primary.push(kv("CPDIS1", "'Lookup'"));
        primary.push(kv("DP1", "'EXTVER: 1'"));
        let table: Vec<u8> = [0.0f32, 1.0, 2.0, 10.0, 11.0, 12.0]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let ext = [
            kv("XTENSION", "'IMAGE'"),
            kv("BITPIX", "-32"),
            kv("NAXIS", "2"),
            kv("NAXIS1", "3"),
            kv("NAXIS2", "2"),
            kv("PCOUNT", "0"),
            kv("GCOUNT", "1"),
            kv("EXTNAME", "'WCSDVARR'"),
            kv("EXTVER", "1"),
            kv("CRPIX1", "1.0"),
            kv("CRPIX2", "1.0"),
            kv("CRVAL1", "0.0"),
            kv("CRVAL2", "0.0"),
            kv("CDELT1", "1.0"),
            kv("CDELT2", "1.0"),
        ];
        let mut bytes = unit(&primary, &[0; 8 * 6 * 4]);
        bytes.extend(unit(&ext, &table));
        let fits = crate::parse(&bytes).unwrap();
        let w = Wcs::from_hdu(&fits, &bytes, &fits.hdus[0]).unwrap();
        assert_eq!(w.distortions(), ["CPDIS"]);
        let plain_h: Vec<(&str, &str, &str)> = wcs
            .iter()
            .map(|c| (c[..8].trim(), c[10..].trim(), ""))
            .collect();
        let plain = Wcs::from_header(&Header::from_pairs(plain_h), &[8, 6]).unwrap();
        // Table index = (p − CRVAL) / CDELT + CRPIX − 1 (as astropy): 1-based pixel (1, 1)
        // reads cell (1, 1), in the second FITS row: 11 pixels along x.
        let a = w.pixel_to_world(&[1.0, 1.0]).unwrap();
        let b = plain.pixel_to_world(&[12.0, 1.0]).unwrap();
        assert!((a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12);
        // The header alone cannot be followed.
        let alone = Wcs::from_header(&fits.hdus[0].header, &[8, 6]).unwrap();
        assert_eq!(
            alone.unsupported,
            Some(Unsupported::Table("WCSDVARR,1".into()))
        );
    }

    #[test]
    fn no_wcs() {
        assert!(Wcs::from_header(&hdr(&[("OBJECT", "'M1'")]), &[10, 10]).is_none());
    }

    #[test]
    fn hostile_many_axes_and_cards() {
        // NAXIS = 999 with 10000 cards: the PC / CD search alone was 3e6 lookups, each a scan
        // of every card.
        let n = 999;
        let mut cards: Vec<(String, String)> = (0..10_000)
            .map(|k| (format!("HISTORY{k}"), format!("{k}")))
            .collect();
        cards.push(("CTYPE1".into(), "'RA---TAN'".into()));
        cards.push(("CTYPE2".into(), "'DEC--TAN'".into()));
        cards.push(("CD1_1".into(), "-0.001".into()));
        cards.push(("CD2_2".into(), "0.001".into()));
        cards.push(("CD3_3".into(), "1.0".into()));
        cards.push(("CDELT3".into(), "5.0".into()));
        cards.push(("CD2_2".into(), "0.002".into()));
        cards.push(("CRVAL3".into(), "7.0".into()));
        let h = Header::from_pairs(cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
        let t = std::time::Instant::now();
        let w = Wcs::from_header(&h, &vec![4; n]).unwrap();
        let el = t.elapsed();
        assert_eq!(w.n(), n);
        // The later CD2_2 wins; CD3_3 is kept, the other axes take their CDELT (1).
        assert_eq!(w.m[n + 1], 0.002);
        assert_eq!(w.m[2 * n + 2], 1.0);
        assert_eq!(w.m[3 * n + 3], 1.0);
        assert_eq!(w.axes[2].crval, 7.0);
        assert!(el < std::time::Duration::from_secs(5), "{el:?}");
    }
}
