//! A CASA image's `coords` keyword (a casacore `CoordinateSystem`) as FITS WCS cards, and
//! its axes: direction (any projection, J2000, ICRS, B1950, galactic, ecliptic,
//! supergalactic), spectral (linear in frequency, or tabulated), Stokes, linear and tabular.

use crate::record::Record;

/// Speed of light, km/s.
const C_KMS: f64 = 299_792.458;

/// Most axes a linear coordinate may have (its `pc` matrix is square in them).
const MAX_LINEAR_AXES: usize = 32;

/// What an image axis is.
#[derive(Clone, Debug, PartialEq)]
pub enum AxisKind {
    Longitude,
    Latitude,
    Spectral,
    Stokes,
    Linear,
    Tabular,
    /// Not described by the coordinate system.
    Unknown,
}

/// One image axis as the coordinate system describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub kind: AxisKind,
    /// The name (`Right Ascension`, `Frequency`, `Stokes`).
    pub name: String,
    /// The unit of [`Axis::values`] and of the WCS cards (deg for direction axes).
    pub unit: String,
    /// Stokes parameters (`I`, `Q`, `RR`...), one per pixel along a Stokes axis.
    pub stokes: Vec<String>,
    /// Tabulated world values (a tabular coordinate, or a spectral one that is not linear),
    /// as pixel → world pairs to interpolate.
    pub table: Option<(Vec<f64>, Vec<f64>)>,
}

/// The spectral coordinate's reference frame and rest frequency.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spectral {
    /// Velocity reference frame (`LSRK`, `BARY`, `TOPO`...).
    pub system: String,
    /// Rest frequency, Hz (0 when unknown).
    pub rest: f64,
    /// casacore's velocity definition: 0 radio, 1 optical, 3 relativistic.
    pub velocity_type: i64,
    /// Velocity unit (`km/s`).
    pub velocity_unit: String,
    /// The axis is in frequency (Hz); else wavelength or something not converted.
    pub frequency: bool,
}

/// What `coords` holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coords {
    /// FITS WCS cards (key, value as FITS value text) for every image axis.
    pub cards: Vec<(String, String)>,
    pub axes: Vec<Axis>,
    pub telescope: Option<String>,
    pub observer: Option<String>,
    /// Observation date, ISO 8601 (from the epoch in `obsdate`).
    pub date: Option<String>,
    /// Direction reference system (`J2000`) and projection (`SIN`).
    pub system: Option<String>,
    pub projection: Option<String>,
    pub spectral: Option<Spectral>,
}

/// FITS value text for a string.
pub fn fits_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// FITS value text for a number.
pub fn fits_num(x: f64) -> String {
    if x.is_finite() {
        format!("{x:?}")
    } else {
        "0.0".into()
    }
}

/// Degrees per `unit` of an angle; `None` when not an angle unit.
pub fn degrees_per(unit: &str) -> Option<f64> {
    Some(match unit.trim() {
        "deg" | "degree" | "degrees" => 1.0,
        "rad" | "radian" | "radians" => 180.0 / std::f64::consts::PI,
        "'" | "arcmin" | "amin" => 1.0 / 60.0,
        "\"" | "''" | "arcsec" | "asec" => 1.0 / 3600.0,
        "mas" | "marcsec" => 1.0 / 3.6e6,
        "uas" | "µas" => 1.0 / 3.6e9,
        "h" | "hour" => 15.0,
        _ => return None,
    })
}

/// Scale to Hz of a frequency unit.
fn hertz_per(unit: &str) -> Option<f64> {
    Some(match unit.trim() {
        "Hz" => 1.0,
        "kHz" => 1e3,
        "MHz" => 1e6,
        "GHz" => 1e9,
        "THz" => 1e12,
        _ => return None,
    })
}

/// FITS Stokes codes (FITS WCS paper I, table 7).
pub fn stokes_code(s: &str) -> Option<f64> {
    Some(match s.trim() {
        "I" => 1.0,
        "Q" => 2.0,
        "U" => 3.0,
        "V" => 4.0,
        "RR" => -1.0,
        "LL" => -2.0,
        "RL" => -3.0,
        "LR" => -4.0,
        "XX" => -5.0,
        "YY" => -6.0,
        "XY" => -7.0,
        "YX" => -8.0,
        _ => return None,
    })
}

/// FITS `SPECSYS` for a casacore frequency reference.
fn specsys(system: &str) -> Option<&'static str> {
    Some(match system.trim() {
        "LSRK" => "LSRK",
        "LSRD" => "LSRD",
        "BARY" => "BARYCENT",
        "GEO" => "GEOCENTR",
        "TOPO" => "TOPOCENT",
        "GALACTO" => "GALACTOC",
        "LGROUP" => "LOCALGRP",
        "CMB" => "CMBDIPOL",
        "REST" | "SOURCE" => "SOURCE",
        _ => return None,
    })
}

/// A proleptic Gregorian date and time from a Modified Julian Date.
pub fn mjd_to_iso(mjd: f64) -> Option<String> {
    if !mjd.is_finite() || mjd.abs() > 1e7 {
        return None;
    }
    let day = mjd.floor();
    let mut ms = ((mjd - day) * 86_400_000.0).round() as i64;
    let mut days = day as i64 + 678_881; // days from 0000-03-01 to 1858-11-17
    if ms >= 86_400_000 {
        ms -= 86_400_000;
        days += 1;
    }
    // Civil date from days since 0000-03-01 (Hinnant's algorithm).
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let (h, rest) = (ms / 3_600_000, ms % 3_600_000);
    let (mi, rest) = (rest / 60_000, rest % 60_000);
    let (s, milli) = (rest / 1000, rest % 1000);
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{milli:03}"
    ))
}

/// A quantity record (`{value, unit}`) as a number in `unit` scaled by `to`.
pub fn quantity(r: &Record, to: impl Fn(&str) -> Option<f64>) -> Option<f64> {
    let v = r.f64("value")?;
    let u = r.str("unit").unwrap_or("");
    Some(v * to(u)?)
}

/// Integer map arrays (`worldmapN`, `pixelmapN`).
fn ints(r: &Record, key: &str) -> Vec<i64> {
    r.numbers(key)
        .unwrap_or_default()
        .into_iter()
        .map(|x| x as i64)
        .collect()
}

/// A square `pc` matrix of size `n`, `pc[i][j]` (world `i`, pixel `j`); identity when
/// missing or malformed.
fn pc_matrix(r: &Record, n: usize) -> Vec<Vec<f64>> {
    let mut m: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    if let Some(v) = r.numbers("pc")
        && v.len() == n * n
    {
        // Stored first index fastest: element (i, j) at i + n j.
        for (i, row) in m.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = v[j + n * i];
            }
        }
    }
    m
}

/// Builds the cards and axes of an image of `naxis` axes.
pub fn parse(coords: &Record, naxis: usize) -> Coords {
    let mut c = Coords {
        axes: vec![
            Axis {
                kind: AxisKind::Unknown,
                name: String::new(),
                unit: String::new(),
                stokes: Vec::new(),
                table: None,
            };
            naxis
        ],
        telescope: coords
            .str("telescope")
            .map(str::to_owned)
            .filter(|s| !s.trim().is_empty()),
        observer: coords
            .str("observer")
            .map(str::to_owned)
            .filter(|s| !s.trim().is_empty()),
        date: coords
            .record("obsdate")
            .and_then(|e| e.record("m0"))
            .and_then(|m| quantity(m, |u| (u == "d").then_some(1.0)))
            .filter(|&mjd| mjd != 0.0)
            .and_then(mjd_to_iso),
        ..Default::default()
    };
    let mut cards: Vec<(String, String)> = vec![("NAXIS".into(), naxis.to_string())];
    let mut pcs: Vec<(usize, usize, f64)> = Vec::new();
    for (name, value) in &coords.fields {
        let Some(rec) = value.as_record() else {
            continue;
        };
        let Some(n) = ["direction", "spectral", "stokes", "linear", "tabular"]
            .iter()
            .find_map(|p| name.strip_prefix(p).filter(|d| d.parse::<u32>().is_ok()))
        else {
            continue;
        };
        let pixel = ints(coords, &format!("pixelmap{n}"));
        // Image axis of the coordinate's axis k (none when removed).
        let axis = |k: usize| -> Option<usize> {
            pixel
                .get(k)
                .and_then(|&p| usize::try_from(p).ok())
                .filter(|&p| p < naxis)
        };
        if name.starts_with("direction") {
            direction(&mut c, &mut cards, &mut pcs, rec, &axis);
        } else if name.starts_with("spectral") {
            spectral(&mut c, &mut cards, rec, &axis);
        } else if name.starts_with("stokes") {
            if let Some(a) = axis(0) {
                stokes(&mut c, &mut cards, rec, a);
            }
        } else {
            linear(
                &mut c,
                &mut cards,
                &mut pcs,
                rec,
                &axis,
                name.starts_with("tabular"),
            );
        }
    }
    for (i, j, v) in pcs {
        if (i == j && v != 1.0) || (i != j && v != 0.0) {
            cards.push((format!("PC{}_{}", i + 1, j + 1), fits_num(v)));
        }
    }
    c.cards = cards;
    c
}

type Pcs = Vec<(usize, usize, f64)>;

fn direction(
    c: &mut Coords,
    cards: &mut Vec<(String, String)>,
    pcs: &mut Pcs,
    r: &Record,
    axis: &dyn Fn(usize) -> Option<usize>,
) {
    let (Some(lng), Some(lat)) = (axis(0), axis(1)) else {
        return;
    };
    let system = r.str("system").unwrap_or("J2000").trim().to_owned();
    let proj = r.str("projection").unwrap_or("SIN").trim().to_owned();
    let (lname, bname, radesys, equinox) = match system.as_str() {
        "GALACTIC" => ("GLON", "GLAT", None, None),
        "ECLIPTIC" | "MECLIPTIC" | "TECLIPTIC" => ("ELON", "ELAT", None, None),
        "SUPERGAL" => ("SLON", "SLAT", None, None),
        "ICRS" => ("RA", "DEC", Some("ICRS"), None),
        "B1950" | "B1950_VLA" | "BMEAN" | "BTRUE" => ("RA", "DEC", Some("FK4"), Some(1950.0)),
        _ => ("RA", "DEC", Some("FK5"), Some(2000.0)),
    };
    let units = r.strings("units").unwrap_or_default();
    let scale = |k: usize| {
        units
            .get(k)
            .and_then(|u| degrees_per(u))
            .unwrap_or(180.0 / std::f64::consts::PI)
    };
    let crval = r.numbers("crval").unwrap_or_default();
    let crpix = r.numbers("crpix").unwrap_or_default();
    let cdelt = r.numbers("cdelt").unwrap_or_default();
    if crval.len() < 2 || crpix.len() < 2 || cdelt.len() < 2 {
        return;
    }
    let pad = |s: &str| format!("{s:-<5}{proj}");
    for (k, (a, base)) in [(lng, lname), (lat, bname)].into_iter().enumerate() {
        let n = a + 1;
        cards.push((format!("CTYPE{n}"), fits_str(&pad(base))));
        cards.push((format!("CRVAL{n}"), fits_num(crval[k] * scale(k))));
        cards.push((format!("CRPIX{n}"), fits_num(crpix[k] + 1.0)));
        cards.push((format!("CDELT{n}"), fits_num(cdelt[k] * scale(k))));
        cards.push((format!("CUNIT{n}"), fits_str("deg")));
        let names = r.strings("axes").unwrap_or_default();
        c.axes[a] = Axis {
            kind: if k == 0 {
                AxisKind::Longitude
            } else {
                AxisKind::Latitude
            },
            name: names.get(k).cloned().unwrap_or_else(|| base.to_owned()),
            unit: "deg".into(),
            stokes: Vec::new(),
            table: None,
        };
    }
    let pc = pc_matrix(r, 2);
    let ax = [lng, lat];
    for i in 0..2 {
        for j in 0..2 {
            pcs.push((ax[i], ax[j], pc[i][j]));
        }
    }
    if let Some(v) = r.f64("longpole") {
        cards.push(("LONPOLE".into(), fits_num(v)));
    }
    if let Some(v) = r.f64("latpole") {
        cards.push(("LATPOLE".into(), fits_num(v)));
    }
    if let Some(p) = r.numbers("projection_parameters") {
        let first = usize::from(proj != "ZPN");
        for (m, v) in p.iter().enumerate() {
            cards.push((format!("PV{}_{}", lat + 1, m + first), fits_num(*v)));
        }
    }
    if let Some(s) = radesys {
        cards.push(("RADESYS".into(), fits_str(s)));
    }
    if let Some(e) = equinox {
        cards.push(("EQUINOX".into(), fits_num(e)));
    }
    c.system = Some(system);
    c.projection = Some(proj);
}

fn spectral(
    c: &mut Coords,
    cards: &mut Vec<(String, String)>,
    r: &Record,
    axis: &dyn Fn(usize) -> Option<usize>,
) {
    let Some(a) = axis(0) else {
        return;
    };
    let unit = r.str("unit").unwrap_or("Hz").trim().to_owned();
    let hz = hertz_per(&unit);
    let rest = r.f64("restfreq").unwrap_or(0.0);
    let system = r.str("system").unwrap_or("").trim().to_owned();
    c.spectral = Some(Spectral {
        system: system.clone(),
        rest,
        velocity_type: r.f64("velType").unwrap_or(0.0) as i64,
        velocity_unit: r.str("velUnit").unwrap_or("km/s").trim().to_owned(),
        frequency: hz.is_some(),
    });
    let mut axis_info = Axis {
        kind: AxisKind::Spectral,
        name: r.str("name").unwrap_or("Frequency").trim().to_owned(),
        unit: if hz.is_some() {
            "Hz".into()
        } else {
            unit.clone()
        },
        stokes: Vec::new(),
        table: None,
    };
    let scale = hz.unwrap_or(1.0);
    let n = a + 1;
    // A tabulated (non-linear) axis: world values are interpolated in the table (what
    // casacore does); the linear cards below still describe it approximately.
    if let Some(t) = r.record("tabular")
        && let (Some(p), Some(w)) = (t.numbers("pixelvalues"), t.numbers("worldvalues"))
        && p.len() == w.len()
        && p.len() >= 2
        && !linear_table(&p, &w)
    {
        axis_info.table = Some((p, w.iter().map(|x| x * scale).collect()));
    }
    let w = r.record("wcs").unwrap_or(r);
    let first = |rec: &Record, k: &str| rec.f64(k).or_else(|| rec.numbers(k)?.first().copied());
    // Images written before the `wcs` sub-record (casacore before 2004 or so) keep the linear
    // description only in the `tabular` record, as one-element arrays.
    let lin = match (w.f64("crval"), w.f64("crpix"), w.f64("cdelt")) {
        (Some(v), Some(p), Some(d)) => Some((v, p, d, w.f64("pc").unwrap_or(1.0))),
        _ => r.record("tabular").and_then(|t| {
            let pc = t.numbers("pc").and_then(|m| m.first().copied());
            Some((
                first(t, "crval")?,
                first(t, "crpix")?,
                first(t, "cdelt")?,
                pc.unwrap_or(1.0),
            ))
        }),
    };
    let Some((crval, crpix, cdelt, pc)) = lin else {
        c.axes[a] = axis_info;
        return;
    };
    let ctype = w
        .str("ctype")
        .map(|s| s.trim_end_matches('\0').trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "FREQ".into());
    cards.push((format!("CTYPE{n}"), fits_str(&ctype)));
    cards.push((format!("CRVAL{n}"), fits_num(crval * scale)));
    cards.push((format!("CRPIX{n}"), fits_num(crpix + 1.0)));
    cards.push((format!("CDELT{n}"), fits_num(cdelt * scale * pc)));
    cards.push((
        format!("CUNIT{n}"),
        fits_str(if hz.is_some() { "Hz" } else { &unit }),
    ));
    if rest > 0.0 {
        cards.push(("RESTFRQ".into(), fits_num(rest)));
    }
    if let Some(s) = specsys(&system) {
        cards.push(("SPECSYS".into(), fits_str(s)));
    }
    c.axes[a] = axis_info;
}

/// Whether tabulated `world` values are linear in `pixel` (to a part in 10⁹ of the range).
fn linear_table(pixel: &[f64], world: &[f64]) -> bool {
    let (p0, p1) = (pixel[0], pixel[pixel.len() - 1]);
    let (w0, w1) = (world[0], world[world.len() - 1]);
    if p1 == p0 {
        return false;
    }
    let tol = (w1 - w0).abs() * 1e-9;
    pixel
        .iter()
        .zip(world)
        .all(|(&p, &w)| (w0 + (p - p0) * (w1 - w0) / (p1 - p0) - w).abs() <= tol)
}

fn stokes(c: &mut Coords, cards: &mut Vec<(String, String)>, r: &Record, a: usize) {
    let s = r.strings("stokes").unwrap_or_default();
    let codes: Vec<f64> = s.iter().filter_map(|x| stokes_code(x)).collect();
    let n = a + 1;
    cards.push((format!("CTYPE{n}"), fits_str("STOKES")));
    if codes.len() == s.len() && !codes.is_empty() {
        let step = if codes.len() >= 2 {
            codes[1] - codes[0]
        } else {
            1.0
        };
        if codes.windows(2).all(|w| (w[1] - w[0] - step).abs() < 1e-9) {
            cards.push((format!("CRVAL{n}"), fits_num(codes[0])));
            cards.push((format!("CRPIX{n}"), "1.0".into()));
            cards.push((format!("CDELT{n}"), fits_num(step)));
        }
    }
    c.axes[a] = Axis {
        kind: AxisKind::Stokes,
        name: "Stokes".into(),
        unit: String::new(),
        stokes: s,
        table: None,
    };
}

fn linear(
    c: &mut Coords,
    cards: &mut Vec<(String, String)>,
    pcs: &mut Pcs,
    r: &Record,
    axis: &dyn Fn(usize) -> Option<usize>,
    tabular: bool,
) {
    let names = r.strings("axes").unwrap_or_default();
    let units = r.strings("units").unwrap_or_default();
    let crval = r.numbers("crval").unwrap_or_default();
    let crpix = r.numbers("crpix").unwrap_or_default();
    let cdelt = r.numbers("cdelt").unwrap_or_default();
    let n = names.len().max(crval.len());
    if tabular {
        // One axis, tabulated pixel → world.
        if let Some(a) = axis(0) {
            let table = match (r.numbers("pixelvalues"), r.numbers("worldvalues")) {
                (Some(p), Some(w)) if p.len() == w.len() && p.len() >= 2 => Some((p, w)),
                _ => None,
            };
            c.axes[a] = Axis {
                kind: AxisKind::Tabular,
                name: names.first().cloned().unwrap_or_default(),
                unit: units.first().cloned().unwrap_or_default(),
                stokes: Vec::new(),
                table,
            };
        }
        return;
    }
    if crval.len() < n || crpix.len() < n || cdelt.len() < n || n > MAX_LINEAR_AXES {
        return;
    }
    let pc = pc_matrix(r, n);
    let ax: Vec<Option<usize>> = (0..n).map(axis).collect();
    for k in 0..n {
        let Some(a) = ax[k] else { continue };
        let i = a + 1;
        let name = names.get(k).cloned().unwrap_or_default();
        let ctype: String = name
            .chars()
            .map(|ch| {
                if ch == ' ' {
                    '-'
                } else {
                    ch.to_ascii_uppercase()
                }
            })
            .take(8)
            .collect();
        cards.push((format!("CTYPE{i}"), fits_str(&ctype)));
        cards.push((format!("CRVAL{i}"), fits_num(crval[k])));
        cards.push((format!("CRPIX{i}"), fits_num(crpix[k] + 1.0)));
        cards.push((format!("CDELT{i}"), fits_num(cdelt[k])));
        let unit = units.get(k).cloned().unwrap_or_default();
        if !unit.is_empty() {
            cards.push((format!("CUNIT{i}"), fits_str(&unit)));
        }
        for (j, aj) in ax.iter().enumerate() {
            if let Some(aj) = aj {
                pcs.push((a, *aj, pc[k][j]));
            }
        }
        c.axes[a] = Axis {
            kind: AxisKind::Linear,
            name,
            unit,
            stokes: Vec::new(),
            table: None,
        };
    }
}

impl Axis {
    /// The tabulated world value at pixel `p`, interpolated (extrapolated at the ends).
    pub fn table_value(&self, p: f64) -> Option<f64> {
        let (px, wx) = self.table.as_ref()?;
        let n = px.len();
        let k = px
            .windows(2)
            .position(|w| p <= w[1])
            .unwrap_or(n - 2)
            .min(n - 2);
        let (p0, p1, w0, w1) = (px[k], px[k + 1], wx[k], wx[k + 1]);
        if p1 == p0 {
            return Some(w0);
        }
        Some(w0 + (p - p0) * (w1 - w0) / (p1 - p0))
    }
}

impl Spectral {
    /// The velocity of frequency `hz` (Hz) in `velocity_unit`; `None` without a rest
    /// frequency.
    pub fn velocity(&self, hz: f64) -> Option<f64> {
        if self.rest <= 0.0 || !self.frequency || hz <= 0.0 {
            return None;
        }
        let f0 = self.rest;
        let v = match self.velocity_type {
            1 | 2 => C_KMS * (f0 / hz - 1.0),
            3 => C_KMS * (f0 * f0 - hz * hz) / (f0 * f0 + hz * hz),
            _ => C_KMS * (1.0 - hz / f0),
        };
        Some(if self.velocity_unit == "m/s" {
            v * 1000.0
        } else {
            v
        })
    }

    /// Short name of the velocity definition.
    pub fn velocity_name(&self) -> &'static str {
        match self.velocity_type {
            1 | 2 => "optical",
            3 => "relativistic",
            _ => "radio",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mjd_dates() {
        assert_eq!(mjd_to_iso(51544.0).unwrap(), "2000-01-01T00:00:00.000");
        assert_eq!(mjd_to_iso(0.0).unwrap(), "1858-11-17T00:00:00.000");
        assert_eq!(mjd_to_iso(59945.5).unwrap(), "2023-01-01T12:00:00.000");
        assert_eq!(
            mjd_to_iso(51544.99999999999).unwrap(),
            "2000-01-02T00:00:00.000"
        );
        assert!(mjd_to_iso(f64::NAN).is_none());
    }

    #[test]
    fn velocities() {
        let s = Spectral {
            rest: 1.0e9,
            frequency: true,
            velocity_unit: "km/s".into(),
            ..Default::default()
        };
        assert!((s.velocity(0.999e9).unwrap() - C_KMS * 0.001).abs() < 1e-9);
        assert!(Spectral::default().velocity(1e9).is_none());
    }

    #[test]
    fn units_and_tables() {
        assert_eq!(degrees_per("'"), Some(1.0 / 60.0));
        assert!(degrees_per("Jy").is_none());
        assert!(linear_table(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0]));
        assert!(!linear_table(&[0.0, 1.0, 2.0], &[5.0, 6.0, 8.0]));
        let a = Axis {
            kind: AxisKind::Tabular,
            name: String::new(),
            unit: String::new(),
            stokes: Vec::new(),
            table: Some((vec![0.0, 1.0, 2.0], vec![5.0, 6.0, 8.0])),
        };
        assert_eq!(a.table_value(1.5), Some(7.0));
        assert_eq!(a.table_value(3.0), Some(10.0));
    }
}
