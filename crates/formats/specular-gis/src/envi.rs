//! Raw bands described by a text header: ENVI (`ENVI` first line, `key = value` lines
//! with `{...}` lists: size, data type, byte order, interleave, header offset, map info or
//! a WKT coordinate system string, wavelengths, band names, data ignore value) and ESRI's
//! `.hdr` labelled `.bil`/`.bsq`/`.bip` files (`NROWS`, `NCOLS`, `NBANDS`, `NBITS`,
//! `PIXELTYPE`, `BYTEORDER`, `LAYOUT`, row and band gaps, `ULXMAP`/`ULYMAP`/`XDIM`/`YDIM`,
//! `NODATA`).

use std::path::{Path, PathBuf};

use crate::crs::{self, Crs, LinearUnit};
use crate::geotiff::Affine2;
use crate::proj::{Ellipsoid, Method};
use crate::rawgrid::{Interleave, RawLayout, SampleType};

/// Which header it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderKind {
    Envi,
    Esri,
}

/// A parsed header.
#[derive(Clone, Debug)]
pub struct Labelled {
    pub kind: HeaderKind,
    pub layout: RawLayout,
    pub nodata: Option<f64>,
    /// Pixel corner to map.
    pub transform: Option<Affine2>,
    pub crs: Option<Crs>,
    /// The projection named in the header when it is not supported here.
    pub crs_note: Option<String>,
    pub wavelengths: Vec<f64>,
    /// The wavelength unit as a symbol ("nm", "µm"), when given.
    pub wavelength_unit: Option<String>,
    pub fwhm: Vec<f64>,
    pub band_names: Vec<String>,
    pub description: Option<String>,
    /// Every header entry, in order, for the info panel.
    pub keys: Vec<(String, String)>,
    /// ENVI `default bands` (1-based), for an RGB composite.
    pub default_bands: Vec<u32>,
    /// ENVI `bbl`: per band, whether it is good (empty when the header has none or its
    /// length is not the band count).
    pub good_bands: Vec<bool>,
}

/// Whether the bytes are an ENVI header.
pub fn sniff_envi(head: &[u8]) -> bool {
    let h = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    h.starts_with(b"ENVI") && h.get(4).is_none_or(|b| b.is_ascii_whitespace())
}

/// Whether the text is an ESRI `.hdr` label (NROWS and NCOLS keys).
pub fn sniff_esri(text: &str) -> bool {
    let mut rows = false;
    let mut cols = false;
    for l in text.lines().take(64) {
        let k = l
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        rows |= k == "NROWS";
        cols |= k == "NCOLS";
    }
    rows && cols
}

/// `key = value` entries; `{...}` values may span lines. Keys lower case.
pub fn entries(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines().skip(1);
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let mut v = v.trim().to_string();
        if v.starts_with('{') {
            while !v.contains('}') {
                match lines.next() {
                    Some(more) => {
                        v.push(' ');
                        v.push_str(more.trim());
                    }
                    None => break,
                }
            }
            let inner = v.trim_start_matches('{');
            v = inner
                .rsplit_once('}')
                .map_or(inner, |(a, _)| a)
                .trim()
                .to_string();
        }
        out.push((k.trim().to_ascii_lowercase(), v));
        if out.len() > 10_000 {
            break;
        }
    }
    out
}

/// A `{a, b, c}` list's items.
pub fn list(v: &str) -> Vec<String> {
    v.split(',').map(|s| s.trim().to_string()).collect()
}

fn numbers(v: &str) -> Vec<f64> {
    v.split([',', ' ', '\t'])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn envi_type(code: u32) -> Option<SampleType> {
    Some(match code {
        1 => SampleType::U8,
        2 => SampleType::I16,
        3 => SampleType::I32,
        4 => SampleType::F32,
        5 => SampleType::F64,
        6 => SampleType::C32,
        9 => SampleType::C64,
        12 => SampleType::U16,
        13 => SampleType::U32,
        14 => SampleType::I64,
        15 => SampleType::U64,
        _ => return None,
    })
}

fn wavelength_unit(v: &str) -> Option<String> {
    Some(
        match v.trim().to_ascii_lowercase().as_str() {
            "nanometers" | "nanometer" | "nm" => "nm",
            "micrometers" | "micrometer" | "microns" | "micron" | "um" => "µm",
            "millimeters" | "mm" => "mm",
            "centimeters" | "cm" => "cm",
            "meters" | "m" => "m",
            "wavenumber" => "cm⁻¹",
            "ghz" => "GHz",
            "mhz" => "MHz",
            "index" | "unknown" | "" => return None,
            other => return Some(other.to_string()),
        }
        .to_string(),
    )
}

/// The geographic CRS of a datum name (ENVI, ERDAS).
pub(crate) fn datum_epsg(name: &str) -> Option<u32> {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    Some(match n.as_str() {
        "wgs84" | "wgs1984" | "dwgs1984" => 4326,
        "wgs72" => 4322,
        "nad83" | "northamerica1983" | "northamerican1983" | "dnorthamerican1983" => 4269,
        "nad27" | "northamerica1927" | "northamerican1927" | "dnorthamerican1927" => 4267,
        "etrs89" => 4258,
        "ed50" | "european1950" => 4230,
        "gda94" | "geocentricdatumofaustralia1994" => 4283,
        "gda2020" | "geocentricdatumofaustralia2020" => 7844,
        _ => return None,
    })
}

pub(crate) fn datum_ellipsoid(name: &str) -> Ellipsoid {
    datum_epsg(name)
        .and_then(crs::from_epsg)
        .map_or(Ellipsoid::WGS84, |c| c.ellipsoid)
}

/// A linear unit from a name ("meters", "feet", "us feet", "km"); metres by default.
pub(crate) fn unit_named(name: Option<&str>) -> LinearUnit {
    match name.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("feet" | "us feet" | "us_survey_feet" | "us survey feet") => {
            LinearUnit::us_survey_foot()
        }
        Some("international feet" | "international_feet") => LinearUnit::foot(),
        Some("km" | "kilometers" | "kilometres") => LinearUnit {
            name: "kilometre".into(),
            metres: 1000.0,
        },
        _ => LinearUnit::metre(),
    }
}

/// UTM zone `zone` on a named datum: the EPSG CRS when there is one, else a custom one.
pub(crate) fn utm_crs(zone: u32, north: bool, datum: &str, unit: &LinearUnit) -> Option<Crs> {
    if !(1..=60).contains(&zone) {
        return None;
    }
    let code = match (datum_epsg(datum), north) {
        (Some(4326), true) => 32600 + zone,
        (Some(4326), false) => 32700 + zone,
        (Some(4269), true) if (1..=23).contains(&zone) => 26900 + zone,
        (Some(4267), true) if (1..=22).contains(&zone) => 26700 + zone,
        (Some(4258), true) if (28..=38).contains(&zone) => 25800 + zone,
        // MGA zones.
        (Some(4283), false) if (48..=58).contains(&zone) => 28300 + zone,
        (Some(7844), false) if (46..=59).contains(&zone) => 7800 + zone,
        _ => 0,
    };
    let metric = unit.metres == 1.0;
    Some(crs::from_epsg(code).filter(|_| metric).unwrap_or_else(|| {
        Crs::custom_projected(
            format!("{datum} / UTM zone {zone}{}", if north { 'N' } else { 'S' }),
            datum_ellipsoid(datum),
            Method::utm(zone, north),
            unit.clone(),
        )
    }))
}

/// `map info = {name, ref x, ref y, map x, map y, size x, size y, [zone, North,] datum,
/// units=..., rotation=...}` → transform and, for UTM and geographic, CRS.
fn map_info(v: &str) -> (Option<Affine2>, Option<Crs>, Option<String>) {
    let items = list(v);
    let name = items.first().cloned().unwrap_or_default();
    let n = |i: usize| items.get(i).and_then(|s| s.parse::<f64>().ok());
    let (Some(rx), Some(ry), Some(mx), Some(my), Some(sx), Some(sy)) =
        (n(1), n(2), n(3), n(4), n(5), n(6))
    else {
        return (None, None, None);
    };
    let mut rotation = 0.0;
    let mut units = None;
    for it in &items {
        if let Some((k, val)) = it.split_once('=') {
            match k.trim().to_ascii_lowercase().as_str() {
                "rotation" => rotation = val.trim().parse().unwrap_or(0.0),
                "units" => units = Some(val.trim().to_string()),
                _ => {}
            }
        }
    }
    // Pixel (1, 1) is the top-left corner of the top-left pixel in ENVI's convention; the
    // rotation turns the north-up grid about the reference pixel (GDAL's reading).
    let (s, c) = f64::to_radians(rotation).sin_cos();
    let (a, b, d, e) = (c * sx, s * sy, s * sx, -c * sy);
    let (px, py) = (rx - 1.0, ry - 1.0);
    let t = Affine2 {
        a,
        b,
        c: mx - a * px - b * py,
        d,
        e,
        f: my - d * px - e * py,
    };
    let plain: Vec<&String> = items.iter().filter(|s| !s.contains('=')).collect();
    let lname = name.to_ascii_lowercase();
    let crs = if lname == "utm" {
        let zone = plain.get(7).and_then(|s| s.parse::<u32>().ok());
        let north = !plain
            .get(8)
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("south"));
        let datum = plain.get(9).map(|s| s.as_str()).unwrap_or("WGS-84");
        zone.and_then(|z| utm_crs(z, north, datum, &unit_named(units.as_deref())))
    } else if lname.starts_with("geographic") {
        let datum = plain.get(7).map(|s| s.as_str()).unwrap_or("WGS-84");
        crs::from_epsg(datum_epsg(datum).unwrap_or(4326))
    } else {
        None
    };
    let note = (crs.is_none() && !lname.is_empty() && lname != "arbitrary" && lname != "pixel")
        .then_some(name);
    let t = (lname != "arbitrary" && lname != "pixel").then_some(t);
    (t, crs, note)
}

/// `projection info = {type, a, b, lat0, lon0, x0, y0, ...params, datum, name}` for
/// Transverse Mercator (3), Lambert conformal conic (4) and Albers (9).
fn projection_info(v: &str, units: &LinearUnit) -> Option<Crs> {
    let items = list(v);
    let f = |i: usize| items.get(i).and_then(|s| s.parse::<f64>().ok());
    let kind = f(0)? as u32;
    let (a, b) = (f(1)?, f(2)?);
    let ell = if a > 0.0 && b > 0.0 && a != b {
        Ellipsoid::inv(a, a / (a - b))
    } else if a > 0.0 {
        Ellipsoid::inv(a, 0.0)
    } else {
        Ellipsoid::WGS84
    };
    let (lat0, lon0, fe, fn_) = (f(3)?, f(4)?, f(5)?, f(6)?);
    let m = units.metres;
    let method = match kind {
        3 => Method::TransverseMercator {
            lat0,
            lon0,
            k0: f(7).unwrap_or(1.0),
            fe: fe * m,
            fn_: fn_ * m,
        },
        4 => Method::LambertConic {
            lat0,
            lon0,
            lat1: f(7)?,
            lat2: f(8)?,
            k0: 1.0,
            fe: fe * m,
            fn_: fn_ * m,
        },
        9 => Method::Albers {
            lat0,
            lon0,
            lat1: f(7)?,
            lat2: f(8)?,
            fe: fe * m,
            fn_: fn_ * m,
        },
        _ => return None,
    };
    let name = items
        .last()
        .filter(|s| s.parse::<f64>().is_err())
        .cloned()
        .unwrap_or_else(|| method.name().to_string());
    Some(Crs::custom_projected(name, ell, method, units.clone()))
}

/// Parses an ENVI header.
pub fn parse_envi(text: &str) -> Result<Labelled, String> {
    if !sniff_envi(text.as_bytes()) {
        return Err("not an ENVI header".into());
    }
    let keys = entries(text);
    let get = |k: &str| keys.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    let int = |k: &str| get(k).and_then(|v| v.trim().parse::<u64>().ok());
    let width = int("samples")
        .filter(|v| (1..=1 << 24).contains(v))
        .ok_or("no samples")?;
    let height = int("lines")
        .filter(|v| (1..=1 << 24).contains(v))
        .ok_or("no lines")?;
    let bands = int("bands").unwrap_or(1);
    if !(1..=1 << 16).contains(&bands) {
        return Err("bad band count".into());
    }
    let dt = int("data type").ok_or("no data type")?;
    let sample = envi_type(dt as u32).ok_or_else(|| format!("data type {dt} not supported"))?;
    let interleave = get("interleave")
        .and_then(Interleave::parse)
        .unwrap_or(Interleave::Bsq);
    let big_endian = int("byte order") == Some(1);
    let offset = int("header offset").unwrap_or(0);
    let layout = RawLayout::packed(
        width as u32,
        height as u32,
        bands as u32,
        sample,
        big_endian,
        interleave,
        offset,
    );
    let nodata = get("data ignore value").and_then(crate::geotiff::parse_nodata);
    let (mut transform, mut crs, mut crs_note) =
        get("map info").map_or((None, None, None), map_info);
    if let Some(w) = get("coordinate system string")
        && let Some(c) = crate::wkt::crs_from_wkt(w)
    {
        crs = Some(c);
        crs_note = None;
    } else if crs.is_none()
        && let Some(p) = get("projection info")
    {
        let units = get("map info")
            .and_then(|m| {
                list(m).into_iter().find_map(|i| {
                    let (k, v) = i.split_once('=')?;
                    (k.trim().eq_ignore_ascii_case("units")).then(|| v.trim().to_ascii_lowercase())
                })
            })
            .map(|u| unit_named(Some(&u)))
            .unwrap_or_else(LinearUnit::metre);
        if let Some(c) = projection_info(p, &units) {
            crs = Some(c);
            crs_note = None;
        }
    }
    if transform.is_none() && crs.is_none() {
        transform = None;
    }
    let wavelengths = get("wavelength").map(numbers).unwrap_or_default();
    let fwhm = get("fwhm").map(numbers).unwrap_or_default();
    let band_names = get("band names").map(list).unwrap_or_default();
    Ok(Labelled {
        kind: HeaderKind::Envi,
        layout,
        nodata,
        transform,
        crs,
        crs_note,
        wavelength_unit: get("wavelength units").and_then(wavelength_unit),
        wavelengths: if wavelengths.len() == bands as usize {
            wavelengths
        } else {
            Vec::new()
        },
        fwhm,
        band_names: if band_names.len() == bands as usize {
            band_names
        } else {
            Vec::new()
        },
        description: get("description")
            .map(str::to_string)
            .filter(|s| !s.is_empty()),
        default_bands: get("default bands")
            .map(|v| numbers(v).into_iter().map(|b| b as u32).collect())
            .unwrap_or_default(),
        good_bands: get("bbl")
            .map(numbers)
            .filter(|v| v.len() == bands as usize)
            .map(|v| v.into_iter().map(|g| g != 0.0).collect())
            .unwrap_or_default(),
        keys,
    })
}

/// Parses an ESRI `.hdr` label; `ext_layout` is the interleave the data file's extension
/// names (the default when LAYOUT is missing), `data_len` the data file's size (which gives
/// the sample size when NBITS is missing).
pub fn parse_esri(
    text: &str,
    ext_layout: Option<Interleave>,
    data_len: Option<u64>,
) -> Result<Labelled, String> {
    let mut keys = Vec::new();
    for l in text.lines() {
        let mut it = l.split_whitespace();
        let (Some(k), Some(v)) = (it.next(), it.next()) else {
            continue;
        };
        keys.push((k.to_ascii_uppercase(), v.to_string()));
    }
    let get = |k: &str| keys.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    let num = |k: &str| get(k).and_then(|v| v.parse::<f64>().ok());
    let int = |k: &str| {
        num(k)
            .filter(|v| *v >= 0.0 && v.fract() == 0.0)
            .map(|v| v as u64)
    };
    let height = int("NROWS")
        .filter(|v| (1..=1 << 24).contains(v))
        .ok_or("no NROWS")?;
    let width = int("NCOLS")
        .filter(|v| (1..=1 << 24).contains(v))
        .ok_or("no NCOLS")?;
    let bands = int("NBANDS").unwrap_or(1);
    if !(1..=1 << 16).contains(&bands) {
        return Err("bad NBANDS".into());
    }
    // Without NBITS the file size tells (a 32-bit guess is floating point, as in GDAL).
    // Checked: 2^24 × 2^24 × 2^16 overflows a u64.
    let samples = width
        .checked_mul(height)
        .and_then(|v| v.checked_mul(bands))
        .unwrap_or(0);
    let inferred = data_len
        .filter(|n| samples > 0 && n % samples == 0)
        .map(|n| n / samples * 8)
        .filter(|b| matches!(b, 8 | 16 | 32 | 64));
    let bits = int("NBITS").or(inferred).unwrap_or(8);
    let pt = get("PIXELTYPE").unwrap_or("").to_ascii_uppercase();
    let signed = pt.starts_with("SIGNED");
    let float = pt.starts_with("FLOAT") || (pt.is_empty() && int("NBITS").is_none() && bits >= 32);
    let sample = match (bits, signed, float) {
        (8, true, _) => SampleType::I8,
        (8, _, _) => SampleType::U8,
        (16, true, _) => SampleType::I16,
        (16, _, _) => SampleType::U16,
        (32, _, true) => SampleType::F32,
        (32, true, _) => SampleType::I32,
        (32, _, _) => SampleType::U32,
        (64, _, true) => SampleType::F64,
        _ => return Err(format!("{bits}-bit samples not supported")),
    };
    let big_endian = get("BYTEORDER").is_some_and(|v| v.to_ascii_uppercase().starts_with('M'));
    let interleave = get("LAYOUT")
        .or(get("INTERLEAVING"))
        .and_then(Interleave::parse)
        .or(ext_layout)
        .unwrap_or(Interleave::Bil);
    let skip = int("SKIPBYTES").unwrap_or(0);
    let mut layout = RawLayout::packed(
        width as u32,
        height as u32,
        bands as u32,
        sample,
        big_endian,
        interleave,
        skip,
    );
    let s = sample.bytes() as u64;
    // Gaps between rows and bands.
    let band_row = int("BANDROWBYTES");
    let total_row = int("TOTALROWBYTES");
    let band_gap = int("BANDGAPBYTES").unwrap_or(0);
    match interleave {
        Interleave::Bil => {
            if let Some(b) = band_row.filter(|b| *b >= width * s) {
                layout.band_stride = b;
            }
            if let Some(t) = total_row.filter(|t| {
                layout
                    .band_stride
                    .checked_mul(bands)
                    .is_some_and(|m| *t >= m)
            }) {
                layout.row_stride = t;
            }
        }
        Interleave::Bsq => {
            if let Some(b) = band_row.filter(|b| *b >= width * s) {
                layout.row_stride = b;
            }
            layout.band_stride = layout
                .row_stride
                .checked_mul(height)
                .and_then(|v| v.checked_add(band_gap))
                .ok_or("band layout too large")?;
        }
        Interleave::Bip => {
            if let Some(t) = total_row.filter(|t| *t >= width * bands * s) {
                layout.row_stride = t;
            }
        }
    }
    let nodata = get("NODATA")
        .or(get("NODATA_VALUE"))
        .and_then(crate::geotiff::parse_nodata);
    let cell = num("CELLSIZE");
    let (dx, dy) = (
        num("XDIM").or(cell).unwrap_or(1.0),
        num("YDIM").or(cell).unwrap_or(1.0),
    );
    // The upper-left pixel's centre, else the lower-left corner or centre (grid style).
    let rows = height as f64 * dy;
    let corner = match (num("ULXMAP"), num("ULYMAP")) {
        (Some(x), Some(y)) => Some((x - 0.5 * dx, y + 0.5 * dy)),
        _ => match (
            num("XLLCORNER"),
            num("YLLCORNER"),
            num("XLLCENTER"),
            num("YLLCENTER"),
        ) {
            (Some(x), Some(y), _, _) => Some((x, y + rows)),
            (_, _, Some(x), Some(y)) => Some((x - 0.5 * dx, y - 0.5 * dy + rows)),
            _ => None,
        },
    };
    let transform = corner.map(|(c, f)| Affine2 {
        a: dx,
        b: 0.0,
        c,
        d: 0.0,
        e: -dy,
        f,
    });
    // `Projection GEOGRAPHIC` / `Datum WGS84` lines, as in an ArcInfo .prj.
    let crs = get("PROJECTION").and_then(|_| crate::worldfile::legacy(text));
    Ok(Labelled {
        kind: HeaderKind::Esri,
        layout,
        nodata,
        transform,
        crs,
        crs_note: None,
        wavelengths: Vec::new(),
        wavelength_unit: None,
        fwhm: Vec::new(),
        good_bands: Vec::new(),
        band_names: Vec::new(),
        description: None,
        keys: keys
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect(),
        default_bands: Vec::new(),
    })
}

/// Data file extensions tried beside an ENVI header, in order.
pub const DATA_EXTENSIONS: &[&str] = &[
    "img", "dat", "bsq", "bil", "bip", "raw", "bin", "envi", "sli", "rdn", "ref", "",
];

/// The data file an ENVI header at `hdr` describes: `<stem>` (`scene.img.hdr` →
/// `scene.img`), else `<stem>.<ext>` for the usual extensions.
pub fn data_file(hdr: &Path) -> Option<PathBuf> {
    let stem = hdr.file_stem()?.to_str()?;
    if Path::new(stem).extension().is_some()
        && let Some(p) = crate::sidecar::find_name(hdr, stem)
    {
        return Some(p);
    }
    DATA_EXTENSIONS.iter().find_map(|e| {
        if e.is_empty() {
            crate::sidecar::find_name(hdr, stem)
        } else {
            crate::sidecar::find(hdr, e)
        }
    })
}

/// The header of a data file: `<stem>.hdr`, else `<name>.hdr`.
pub fn header_file(data: &Path) -> Option<PathBuf> {
    crate::sidecar::find(data, "hdr").or_else(|| crate::sidecar::find_full(data, "hdr"))
}

/// The interleave a data file's extension names.
pub fn ext_interleave(path: &Path) -> Option<Interleave> {
    path.extension()
        .and_then(|e| e.to_str())
        .and_then(Interleave::parse)
}

/// Reads the header beside (or at) `path`: an ENVI header, or an ESRI label for
/// `.bil`/`.bsq`/`.bip` data.
pub fn read_header(header_text: &str, data_path: &Path) -> Result<Labelled, String> {
    if sniff_envi(header_text.as_bytes()) {
        parse_envi(header_text)
    } else if sniff_esri(header_text) {
        let len = std::fs::metadata(data_path).ok().map(|m| m.len());
        parse_esri(header_text, ext_interleave(data_path), len)
    } else {
        Err("not an ENVI or ESRI header".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR: &str = "ENVI\ndescription = {\n  Synthetic scene\n  for tests}\nsamples = 4\nlines   = 3\nbands   = 2\nheader offset = 0\nfile type = ENVI Standard\ndata type = 12\ninterleave = bil\nbyte order = 1\nmap info = {UTM, 1.000, 1.000, 500000.000, 4200000.000, 3.0000000000e+001, 3.0000000000e+001, 33, North, WGS-84, units=Meters}\nwavelength units = Nanometers\nwavelength = {\n 450.5, 550.25}\nband names = {Blue, Green}\ndata ignore value = 0\n";

    #[test]
    fn envi_header() {
        let h = parse_envi(HDR).unwrap();
        assert_eq!(h.kind, HeaderKind::Envi);
        let l = h.layout;
        assert_eq!((l.width, l.height, l.bands), (4, 3, 2));
        assert_eq!(l.sample, SampleType::U16);
        assert!(l.big_endian);
        assert_eq!(l.interleave, Interleave::Bil);
        assert_eq!(l.end(), 48);
        assert_eq!(h.nodata, Some(0.0));
        assert_eq!(h.wavelengths, [450.5, 550.25]);
        assert_eq!(h.wavelength_unit.as_deref(), Some("nm"));
        assert_eq!(h.band_names, ["Blue", "Green"]);
        assert_eq!(h.description.as_deref(), Some("Synthetic scene for tests"));
        assert_eq!(h.crs.as_ref().unwrap().epsg, Some(32633));
        let t = h.transform.unwrap();
        assert_eq!(t.apply(1.0, 1.0), (500030.0, 4199970.0));
        // A reference pixel other than (1, 1).
        let (t, c, _) = map_info("Geographic Lat/Lon, 2.5, 3.5, 10.0, 50.0, 0.1, 0.1, WGS-84");
        let t = t.unwrap();
        assert_eq!(c.unwrap().epsg, Some(4326));
        let (x, y) = t.apply(1.5, 2.5);
        assert!((x - 10.0).abs() < 1e-12 && (y - 50.0).abs() < 1e-12);
    }

    #[test]
    fn bad_bands_and_complex() {
        let t = "ENVI\nsamples = 2\nlines = 2\nbands = 3\ndata type = 6\ninterleave = bip\nbbl = {1, 0, 1}\ndefault bands = {3, 2, 1}\n";
        let h = parse_envi(t).unwrap();
        assert_eq!(h.good_bands, [true, false, true]);
        assert_eq!(h.default_bands, [3, 2, 1]);
        assert!(h.layout.sample.is_complex());
        assert_eq!(h.layout.pixel_stride, 24);
        // A bbl of the wrong length is ignored.
        let t = t.replace("{1, 0, 1}", "{1, 0}");
        assert!(parse_envi(&t).unwrap().good_bands.is_empty());
    }

    #[test]
    fn esri_label() {
        let t = "BYTEORDER      I\nLAYOUT         BIL\nNROWS          2\nNCOLS          3\nNBANDS         2\nNBITS          16\nBANDROWBYTES   8\nTOTALROWBYTES  16\nPIXELTYPE      SIGNEDINT\nULXMAP         10.05\nULYMAP         49.95\nXDIM           0.1\nYDIM           0.1\nNODATA         -9999\n";
        assert!(sniff_esri(t));
        let h = parse_esri(t, None, None).unwrap();
        assert_eq!(h.layout.sample, SampleType::I16);
        assert_eq!(h.layout.band_stride, 8);
        assert_eq!(h.layout.row_stride, 16);
        assert_eq!(h.nodata, Some(-9999.0));
        let tr = h.transform.unwrap();
        let (x, y) = tr.apply(0.0, 0.0);
        assert!((x - 10.0).abs() < 1e-12 && (y - 50.0).abs() < 1e-12);
        // Grid-style keys, the sample size from the file size, the CRS from the label.
        let g = "ncols 3\nnrows 2\ncellsize 5\nxllcorner 100\nyllcorner 200\nbyteorder msbfirst\nProjection GEOGRAPHIC\nDatum WGS84\n";
        let h = parse_esri(g, None, Some(24)).unwrap();
        assert_eq!(h.layout.sample, SampleType::F32);
        assert!(h.layout.big_endian);
        assert_eq!(h.crs.and_then(|c| c.epsg), Some(4326));
        let tr = h.transform.unwrap();
        assert_eq!((tr.c, tr.f, tr.a, tr.e), (100.0, 210.0, 5.0, -5.0));
        assert_eq!(
            parse_esri(g, None, Some(12)).unwrap().layout.sample,
            SampleType::U16
        );
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// 2^24 × 2^24 × 2^16 samples: the size check must not overflow.
    #[test]
    fn esri_sample_count_overflow() {
        let t = "NROWS 16777216\nNCOLS 16777216\nNBANDS 65536\n";
        let _ = parse_esri(t, None, Some(1000));
    }

    /// Row and band strides near u64::MAX.
    #[test]
    fn esri_huge_strides() {
        let bil = "NROWS 2\nNCOLS 2\nNBANDS 2\nNBITS 8\nLAYOUT BIL\n\
                   BANDROWBYTES 1e19\nTOTALROWBYTES 1e19\n";
        let _ = parse_esri(bil, None, None);
        let bsq = "NROWS 4\nNCOLS 2\nNBITS 8\nLAYOUT BSQ\nBANDROWBYTES 1e19\n";
        assert!(parse_esri(bsq, None, None).is_err());
        let gap = "NROWS 1\nNCOLS 1\nNBITS 8\nLAYOUT BSQ\nBANDGAPBYTES 1e20\n";
        assert!(parse_esri(gap, None, None).is_err());
    }
}
