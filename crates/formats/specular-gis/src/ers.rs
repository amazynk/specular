//! ER Mapper raster headers (`.ers`): nested `Name Begin` … `Name End` blocks of
//! `key = value` lines (quoted strings, `{ … }` values over several lines, `#` comments)
//! describing a band-interleaved-by-line raw data file beside them: cell type and byte
//! order, size, cell size and the registration point, datum and projection, no-data and
//! band names.

use std::path::{Path, PathBuf};

use crate::crs::{self, Crs, LinearUnit};
use crate::envi;
use crate::geotiff::Affine2;
use crate::rawgrid::{Interleave, RawLayout, SampleType};

/// A parsed header.
#[derive(Clone, Debug)]
pub struct Header {
    pub layout: RawLayout,
    pub nodata: Option<f64>,
    /// Pixel corner to map.
    pub transform: Option<Affine2>,
    pub crs: Option<Crs>,
    /// The datum and projection as named, when they are not known here.
    pub crs_note: Option<String>,
    pub band_names: Vec<String>,
    /// The unit of the first band, when given.
    pub unit: Option<String>,
    /// The data file's name when the header gives one (`DataFile`).
    pub data_file: Option<String>,
    /// Every entry as (`Block.Sub.Key`, value), in file order, for the info panel.
    pub keys: Vec<(String, String)>,
}

/// One token of the header text.
#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    Str(String),
    Eq,
    Open,
    Close,
    Newline,
}

fn tokens(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut it = text.chars().peekable();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut Vec<Tok>| {
        if !w.is_empty() {
            out.push(Tok::Word(std::mem::take(w)));
        }
    };
    while let Some(c) = it.next() {
        match c {
            '#' => {
                flush(&mut word, &mut out);
                while it.peek().is_some_and(|&c| c != '\n') {
                    it.next();
                }
            }
            '"' => {
                flush(&mut word, &mut out);
                let mut s = String::new();
                while let Some(c) = it.next() {
                    match c {
                        '\\' => {
                            if let Some(n) = it.next() {
                                s.push(n);
                            }
                        }
                        '"' => break,
                        c => s.push(c),
                    }
                }
                out.push(Tok::Str(s));
            }
            '=' => {
                flush(&mut word, &mut out);
                out.push(Tok::Eq);
            }
            '{' => {
                flush(&mut word, &mut out);
                out.push(Tok::Open);
            }
            '}' => {
                flush(&mut word, &mut out);
                out.push(Tok::Close);
            }
            '\n' => {
                flush(&mut word, &mut out);
                out.push(Tok::Newline);
            }
            c if c.is_whitespace() => flush(&mut word, &mut out),
            c => word.push(c),
        }
        if out.len() > 1_000_000 {
            break;
        }
    }
    flush(&mut word, &mut out);
    out
}

/// The entries as (path, value): `RasterInfo.CellInfo.Xdimension`. Repeated blocks (one
/// `BandId` per band) repeat their paths.
fn entries(text: &str) -> Result<Vec<(String, String)>, String> {
    let toks = tokens(text);
    let mut path: Vec<String> = Vec::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let Tok::Word(name) = &toks[i] else {
            i += 1;
            continue;
        };
        match toks.get(i + 1) {
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("begin") => {
                if path.len() >= 32 {
                    return Err("the header nests too deeply".into());
                }
                path.push(name.clone());
                i += 2;
            }
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("end") => {
                if path.last().is_some_and(|p| p.eq_ignore_ascii_case(name)) {
                    path.pop();
                }
                i += 2;
            }
            Some(Tok::Eq) => {
                i += 2;
                let mut parts = Vec::new();
                let mut depth = 0usize;
                while let Some(t) = toks.get(i) {
                    match t {
                        Tok::Newline if depth == 0 => break,
                        Tok::Newline => {}
                        Tok::Open => depth += 1,
                        Tok::Close => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        Tok::Word(w) | Tok::Str(w) => parts.push(w.clone()),
                        Tok::Eq => parts.push("=".into()),
                    }
                    i += 1;
                }
                let mut key = path.join(".");
                if !key.is_empty() {
                    key.push('.');
                }
                key.push_str(name);
                out.push((key, parts.join(" ")));
                if out.len() > 100_000 {
                    return Err("the header has too many entries".into());
                }
            }
            _ => i += 1,
        }
    }
    Ok(out)
}

/// Whether the text starts (after comments and blank lines) with `DatasetHeader Begin`.
pub fn sniff(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    let t = t.strip_prefix('\u{feff}').unwrap_or(&t);
    t.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .is_some_and(|l| {
            let mut w = l.split_whitespace();
            w.next()
                .is_some_and(|a| a.eq_ignore_ascii_case("DatasetHeader"))
                && w.next().is_some_and(|b| b.eq_ignore_ascii_case("Begin"))
        })
}

fn cell_type(v: &str) -> Option<SampleType> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "unsigned8bitinteger" => SampleType::U8,
        "signed8bitinteger" => SampleType::I8,
        "unsigned16bitinteger" => SampleType::U16,
        "signed16bitinteger" => SampleType::I16,
        "unsigned32bitinteger" => SampleType::U32,
        "signed32bitinteger" => SampleType::I32,
        "ieee4bytereal" => SampleType::F32,
        "ieee8bytereal" => SampleType::F64,
        _ => return None,
    })
}

/// Degrees from `dd:mm:ss.s` (or a plain number).
pub fn dms(v: &str) -> Option<f64> {
    let v = v.trim();
    let (neg, body) = match v.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, v.strip_prefix('+').unwrap_or(v)),
    };
    let mut total = 0.0;
    for (i, p) in body.split(':').enumerate() {
        if i > 2 {
            return None;
        }
        let x: f64 = p.trim().parse().ok()?;
        total += x / 60f64.powi(i as i32);
    }
    total
        .is_finite()
        .then_some(if neg { -total } else { total })
}

/// The CRS of a datum and an ER Mapper projection name: `GEODETIC`, `NUTM11`/`SUTM55`,
/// `MGA55` (GDA94), `MGA2020_55`, `TMAMG55` (AMG on AGD66 or AGD84); `RAW` and `LOCAL`
/// have none.
fn crs_of(datum: &str, projection: &str, units: &LinearUnit) -> (Option<Crs>, Option<String>) {
    let p = projection.trim().to_ascii_uppercase();
    let d = datum.trim();
    if p.is_empty() || p == "RAW" || p == "LOCAL" {
        return (None, None);
    }
    let zone = |s: &str| s.parse::<u32>().ok().filter(|z| (1..=60).contains(z));
    let crs = if p == "GEODETIC" || p == "GEOGRAPHIC" {
        envi::datum_epsg(d).and_then(crs::from_epsg)
    } else if let Some(z) = p.strip_prefix("NUTM").and_then(zone) {
        envi::utm_crs(z, true, d, units)
    } else if let Some(z) = p.strip_prefix("SUTM").and_then(zone) {
        envi::utm_crs(z, false, d, units)
    } else if let Some(z) = p.strip_prefix("MGA2020_").and_then(zone) {
        envi::utm_crs(z, false, "GDA2020", units)
    } else if let Some(z) = p.strip_prefix("MGA").and_then(zone) {
        envi::utm_crs(z, false, "GDA94", units)
    } else if let Some(z) = p.strip_prefix("TMAMG").and_then(zone) {
        let base = if d.eq_ignore_ascii_case("AGD84") {
            20300
        } else {
            20200
        };
        crs::from_epsg(base + z).or_else(|| envi::utm_crs(z, false, d, units))
    } else {
        None
    };
    let note = crs.is_none().then(|| format!("{d} / {projection}"));
    (crs, note)
}

/// Parses an ER Mapper header.
pub fn parse(text: &str) -> Result<Header, String> {
    let keys = entries(text)?;
    let get = |k: &str| {
        keys.iter()
            .find(|(p, _)| p.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_str())
    };
    let num = |k: &str| get(k).and_then(|v| v.trim().parse::<f64>().ok());
    if get("DatasetHeader.DataType").is_some_and(|t| !t.trim().eq_ignore_ascii_case("Raster")) {
        return Err("an ER Mapper vector dataset, not a raster".into());
    }
    if get("DatasetHeader.DataSetType").is_some_and(|t| t.trim().eq_ignore_ascii_case("Translated"))
    {
        return Err(
            "ER Mapper translated datasets (other formats wrapped) are not supported".into(),
        );
    }
    let ri = "DatasetHeader.RasterInfo";
    let int = |k: &str| {
        get(&format!("{ri}.{k}"))
            .and_then(|v| v.trim().parse::<u32>().ok())
            .ok_or_else(|| format!("no {k} in the header"))
    };
    let width = int("NrOfCellsPerLine")?;
    let height = int("NrOfLines")?;
    let bands = int("NrOfBands")?;
    if bands == 0 || bands > 100_000 {
        return Err("band count".into());
    }
    let ct = get(&format!("{ri}.CellType")).ok_or("no CellType in the header")?;
    let sample = cell_type(ct).ok_or_else(|| format!("cell type {ct} is not supported"))?;
    let big_endian =
        get("DatasetHeader.ByteOrder").is_some_and(|b| b.trim().eq_ignore_ascii_case("MSBFirst"));
    let offset = num("DatasetHeader.HeaderOffset")
        .or_else(|| num(&format!("{ri}.HeaderOffset")))
        .filter(|o| *o >= 0.0)
        .map_or(0, |o| o as u64);
    let layout = RawLayout::packed(
        width,
        height,
        bands,
        sample,
        big_endian,
        Interleave::Bil,
        offset,
    );
    let nodata = get(&format!("{ri}.NullCellValue")).and_then(|v| v.trim().parse::<f64>().ok());
    // The transform: the registration point is at cell (RegistrationCellX, Y), (0, 0) the
    // top-left corner of the top-left cell.
    let cs = "DatasetHeader.CoordinateSpace";
    let units = envi::unit_named(get(&format!("{cs}.Units")));
    let (xd, yd) = (
        num(&format!("{ri}.CellInfo.Xdimension")),
        num(&format!("{ri}.CellInfo.Ydimension")),
    );
    let rc = format!("{ri}.RegistrationCoord");
    let origin = match (
        num(&format!("{rc}.Eastings")),
        num(&format!("{rc}.Northings")),
    ) {
        (Some(e), Some(n)) => Some((e, n)),
        _ => match (
            get(&format!("{rc}.Longitude")).and_then(dms),
            get(&format!("{rc}.Latitude")).and_then(dms),
        ) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => num(&format!("{rc}.MetersX")).zip(num(&format!("{rc}.MetersY"))),
        },
    };
    let reg_x = num(&format!("{ri}.RegistrationCellX")).unwrap_or(0.0);
    let reg_y = num(&format!("{ri}.RegistrationCellY")).unwrap_or(0.0);
    let transform = match (xd, yd, origin) {
        (Some(xd), Some(yd), Some((x0, y0))) if xd > 0.0 && yd > 0.0 => Some(Affine2 {
            a: xd,
            b: 0.0,
            c: x0 - reg_x * xd,
            d: 0.0,
            e: -yd,
            f: y0 + reg_y * yd,
        }),
        _ => None,
    };
    let (crs, crs_note) = crs_of(
        get(&format!("{cs}.Datum")).unwrap_or("WGS84"),
        get(&format!("{cs}.Projection")).unwrap_or(""),
        &units,
    );
    let band_names: Vec<String> = keys
        .iter()
        .filter(|(p, _)| p.eq_ignore_ascii_case(&format!("{ri}.BandId.Value")))
        .map(|(_, v)| v.clone())
        .collect();
    let unit = get(&format!("{ri}.BandId.Units"))
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_owned);
    let data_file = get("DatasetHeader.DataFile")
        .or_else(|| get(&format!("{ri}.DataFile")))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let keys = keys
        .into_iter()
        .map(|(k, v)| (k.trim_start_matches("DatasetHeader.").to_owned(), v))
        .collect();
    Ok(Header {
        layout,
        nodata,
        transform,
        crs,
        crs_note,
        band_names,
        unit,
        data_file,
        keys,
    })
}

/// The data file a header at `path` describes: the name its `DataFile` gives (its base name
/// only, looked for beside it), else the header's name without `.ers`.
pub fn data_file(path: &Path, h: &Header) -> Option<PathBuf> {
    if let Some(n) = &h.data_file {
        let base = n.rsplit(['/', '\\']).next().unwrap_or(n);
        if let Some(p) = crate::sidecar::find_name(path, base) {
            return Some(p);
        }
    }
    let stem = path.file_stem()?.to_str()?;
    crate::sidecar::find_name(path, stem)
}

/// The header of a data file: `<name>.ers` beside it.
pub fn header_file(data: &Path) -> Option<PathBuf> {
    crate::sidecar::find_full(data, "ers")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRTM: &str = "# a comment first\nDatasetHeader Begin\n  Version = \"5.2\"\n  ByteOrder = MSBFirst\n  Foo = { \"x\\\\\\\"\ny\" }\n  Bar = \"# not a comment\"\n  CoordinateSpace Begin\n    Datum = \"WGS84\"\n    Projection = \"GEODETIC\"\n    CoordinateType = LL\n  CoordinateSpace End\n  RasterInfo Begin\n    CellType = Signed16BitInteger   # trailing\n    NullCellValue = +9999\n    CellInfo Begin\n      Xdimension = 0.5\n      Ydimension = 0.25\n    CellInfo End\n    NrOfCellsPerLine = 12\n    NrOfLines = 10\n    RegistrationCellX = 1\n    RegistrationCellY = 2\n    NrOfBands = 2\n    BandId Begin\n      Value = \"Topography\"\n      Units = \"meters\"\n    BandId End\n    BandId Begin\n      Value = \"Other\"\n    BandId End\n    RegistrationCoord Begin\n      Latitude = -60:30:00\n      Longitude = -180:00:00\n    RegistrationCoord End\n    RegionInfo Begin\n      Stats Begin\n        MinimumValue = { -4315 # in braces\n        }\n      Stats End\n    RegionInfo End\n  RasterInfo End\nDatasetHeader End\n";

    #[test]
    fn header() {
        assert!(sniff(SRTM.as_bytes()));
        assert!(!sniff(b"ENVI\nsamples = 2\n"));
        let h = parse(SRTM).unwrap();
        let l = h.layout;
        assert_eq!((l.width, l.height, l.bands), (12, 10, 2));
        assert_eq!(l.sample, SampleType::I16);
        assert!(l.big_endian);
        assert_eq!(l.row_stride, 12 * 2 * 2);
        assert_eq!(h.nodata, Some(9999.0));
        assert_eq!(h.band_names, ["Topography", "Other"]);
        assert_eq!(h.unit.as_deref(), Some("meters"));
        assert_eq!(h.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        let t = h.transform.unwrap();
        assert_eq!(t.apply(1.0, 2.0), (-180.0, -60.5));
        assert_eq!(t.apply(0.0, 0.0), (-180.5, -60.0));
        let k = |name: &str| {
            h.keys
                .iter()
                .find(|(p, _)| p == name)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(k("Foo").as_deref(), Some("x\\\"\ny"));
        assert_eq!(k("Bar").as_deref(), Some("# not a comment"));
        assert_eq!(
            k("RasterInfo.RegionInfo.Stats.MinimumValue").as_deref(),
            Some("-4315")
        );
    }

    #[test]
    fn projections() {
        let m = LinearUnit::metre();
        assert_eq!(crs_of("NAD27", "NUTM11", &m).0.unwrap().epsg, Some(26711));
        assert_eq!(crs_of("WGS84", "SUTM33", &m).0.unwrap().epsg, Some(32733));
        assert_eq!(crs_of("GDA94", "MGA55", &m).0.unwrap().epsg, Some(28355));
        assert!(crs_of("WGS84", "RAW", &m).0.is_none());
        let (c, note) = crs_of("WGS84", "LAMBERT_X", &m);
        assert!(c.is_none() && note.as_deref() == Some("WGS84 / LAMBERT_X"));
        assert_eq!(dms("-60:30:00"), Some(-60.5));
        assert_eq!(dms("12.25"), Some(12.25));
        assert_eq!(dms("1:2:3:4"), None);
    }

    #[test]
    fn hostile() {
        assert!(parse("DatasetHeader Begin\nDataType = Vector\nDatasetHeader End").is_err());
        let deep = "A Begin\n".repeat(100);
        assert!(parse(&deep).is_err());
        assert!(parse("DatasetHeader Begin\nDatasetHeader End").is_err());
    }
}
