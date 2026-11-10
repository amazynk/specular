//! ISO 25178-72 X3P (ISO 5436-2): a ZIP holding `main.xml` (at the root, or in one folder
//! as some tools zip it) and the binary point data it links to, or the points inline as a
//! `DataList`.
//!
//! Points run x fastest, then y, then z (layers become planes), little-endian `I` (int16),
//! `L` (int32), `F` (float32) or `D` (float64). Integer heights are `raw × Increment +
//! Offset`; float heights are metres as stored (the increment only states the resolution)
//! plus the offset. A validity bitmap (`ValidPointsLink`, least significant bit first, 1 =
//! measured) or an empty `Datum` marks points as NaN, as do NaN floats. Row `y = 0` is the
//! top row, as the reference viewers show it.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use specular_em::dataset::{AxisKind, Dataset, Lateral};
use specular_em::samples::{self, Order, Sample};
use specular_ora::zip::{Entry, Zip};

use crate::{Error, Result, alloc, area, height, meta, page};

/// The largest `main.xml` read (inline data lists can be long).
const MAX_XML: u64 = 256 << 20;
/// Deepest element nesting of `main.xml` read (the schema nests 5 deep).
const MAX_XML_DEPTH: usize = 64;
/// Longest element path kept as a field.
const MAX_PATH: usize = 4096;

/// A ZIP's first bytes (the X3P check needs the whole file: [`probe_file`]).
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"PK\x03\x04")
}

/// The `main.xml` entry, at the root or one folder down, ignoring macOS resource forks.
fn main_entry(zip: &Zip) -> Option<&Entry> {
    zip.entries.iter().find(|e| {
        !e.name.starts_with("__MACOSX")
            && (e.name == "main.xml"
                || e.name
                    .strip_suffix("/main.xml")
                    .is_some_and(|dir| !dir.contains('/')))
    })
}

/// A ZIP holding an X3P `main.xml`.
pub fn probe_file(file: &[u8]) -> bool {
    probe(file) && Zip::parse(file).is_ok_and(|z| main_entry(&z).is_some())
}

/// The XML as element paths (local names joined by `/`, below the root) to text, with the
/// `Datum` values of an inline list kept in order.
struct Xml {
    fields: HashMap<String, String>,
    data: Vec<String>,
}

fn parse_xml(xml: &[u8]) -> Result<Xml> {
    let mut r = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut fields = HashMap::new();
    let mut data = Vec::new();
    let mut text = String::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                // Each closing element stores its whole path: deep nesting would cost the
                // square of the depth.
                if stack.len() >= MAX_XML_DEPTH {
                    return Err(Error::Malformed("main.xml nests too deeply".into()));
                }
                stack.push(String::from_utf8_lossy(e.local_name().as_ref()).into_owned());
                text.clear();
            }
            Ok(Event::Empty(e)) => {
                if e.local_name().as_ref() == b"Datum" {
                    data.push(String::new());
                }
            }
            Ok(Event::Text(t)) => text.push_str(&String::from_utf8_lossy(&t)),
            Ok(Event::End(_)) => {
                if let Some(name) = stack.pop() {
                    if name == "Datum" {
                        data.push(text.trim().to_string());
                    } else if !stack.is_empty() {
                        let path = stack[1..]
                            .iter()
                            .chain(std::iter::once(&name))
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join("/");
                        if path.len() <= MAX_PATH {
                            fields
                                .entry(path)
                                .or_insert_with(|| text.trim().to_string());
                        }
                    }
                }
                text.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Malformed(format!("main.xml: {e}"))),
            Ok(_) => {}
        }
        buf.clear();
    }
    Ok(Xml { fields, data })
}

impl Xml {
    fn get(&self, path: &str) -> Option<&str> {
        self.fields
            .get(path)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    fn num(&self, path: &str) -> Option<f64> {
        self.get(path)?.parse().ok().filter(|v: &f64| v.is_finite())
    }
}

fn zip_err(e: specular_ora::Error) -> Error {
    Error::Malformed(format!("zip: {e}"))
}

pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotFormat("X3P"));
    }
    let zip = Zip::parse(file).map_err(zip_err)?;
    let main = main_entry(&zip).ok_or(Error::NotFormat("X3P"))?;
    let dir = main.name.strip_suffix("main.xml").unwrap_or("").to_string();
    let xml = zip.read(file, main, MAX_XML).map_err(zip_err)?;
    let x = parse_xml(&xml)?;
    match x.get("Record1/FeatureType") {
        Some("SUR") | None => {}
        Some(f) => return Err(Error::Unsupported(format!("X3P feature type {f}"))),
    }
    for a in ["CX", "CY"] {
        if x.get(&format!("Record1/Axes/{a}/AxisType")) == Some("A") {
            return Err(Error::Unsupported("X3P with listed x/y coordinates".into()));
        }
    }
    let dim = |k: &str| {
        x.get(&format!("Record3/MatrixDimension/{k}"))
            .and_then(|v| v.parse::<usize>().ok())
    };
    let (w, h) = match (dim("SizeX"), dim("SizeY")) {
        (Some(w), Some(h)) => (w, h),
        _ => return Err(Error::Malformed("no matrix dimension".into())),
    };
    let layers = dim("SizeZ").unwrap_or(1).max(1);
    if layers > 64 {
        return Err(Error::Unsupported(format!("{layers} layers")));
    }
    let n = area(w, h)?;
    let total = n.checked_mul(layers).ok_or(Error::TooLarge)?;
    if total > crate::MAX_VALUES {
        return Err(Error::TooLarge);
    }
    let axis = |a: &str, k: &str| x.num(&format!("Record1/Axes/{a}/{k}"));
    let inc = axis("CZ", "Increment").unwrap_or(1.0);
    let off = axis("CZ", "Offset").unwrap_or(0.0);
    let dtype = x.get("Record1/Axes/CZ/DataType").unwrap_or("D");
    let (sample, integer) = match dtype {
        "I" => (Sample::I16, true),
        "L" => (Sample::I32, true),
        "F" => (Sample::F32, false),
        "D" => (Sample::F64, false),
        t => return Err(Error::Unsupported(format!("X3P data type {t}"))),
    };
    let (scale, add) = if integer { (inc, off) } else { (1.0, off) };

    let mut values = alloc(total)?;
    if let Some(link) = x.get("Record3/DataLink/PointDataLink") {
        let e = find(&zip, &dir, link).ok_or(Error::Truncated("point data"))?;
        let need = total * sample.bytes();
        let bytes = zip.read_prefix(file, e, need).map_err(zip_err)?;
        if bytes.len() < need {
            return Err(Error::Truncated("point data"));
        }
        values.resize(total, 0.0);
        if samples::convert(&bytes, sample, Order::Little, sample.bytes(), &mut values) {
            return Err(Error::Truncated("point data"));
        }
        if scale != 1.0 || add != 0.0 {
            for v in values.iter_mut() {
                *v = (f64::from(*v) * scale + add) as f32;
            }
        }
    } else if !x.data.is_empty() {
        // Each point is a `<Datum/>` at least, so more points than the XML has bytes
        // aren't there (padding them would size the plane by the header alone).
        if total > xml.len() {
            return Err(Error::Truncated("data list"));
        }
        values.extend(x.data.iter().take(total).map(|d| {
            // A datum may list x;y;z for other feature types: the height is the last.
            let z = d.rsplit(';').next().unwrap_or("").trim();
            match z.parse::<f64>() {
                Ok(v) => (v * scale + add) as f32,
                Err(_) => f32::NAN,
            }
        }));
        values.resize(total, f32::NAN);
    } else {
        return Err(Error::NoImage);
    }
    if let Some(link) = x.get("Record3/DataLink/ValidPointsLink") {
        let e = find(&zip, &dir, link).ok_or(Error::Truncated("validity bitmap"))?;
        let bits = zip
            .read_prefix(file, e, total.div_ceil(8))
            .map_err(zip_err)?;
        for (i, v) in values.iter_mut().enumerate() {
            if bits.get(i / 8).is_some_and(|b| b >> (i % 8) & 1 == 0) {
                *v = f32::NAN;
            }
        }
    }

    let dx = axis("CX", "Increment").unwrap_or(0.0);
    let dy = axis("CY", "Increment").unwrap_or(0.0);
    let mut pg = page(w, h, dx, dy);
    if let Some(l) = pg.x.as_mut() {
        l.offset = axis("CX", "Offset").unwrap_or(0.0);
    }
    if let Some(l) = pg.y.as_mut() {
        l.offset = axis("CY", "Offset").unwrap_or(0.0);
    }
    pg.sample = Some(sample);
    pg.levelling = true;
    pg.date = x.get("Record2/Date").map(str::to_string);
    let inst: Vec<&str> = ["Manufacturer", "Model"]
        .iter()
        .filter_map(|k| x.get(&format!("Record2/Instrument/{k}")))
        .collect();
    pg.instrument = (!inst.is_empty()).then(|| inst.join(" "));
    pg.detector = x
        .get("Record2/ProbingSystem/Identification")
        .map(|s| s.trim().to_string());
    pg.mode = x.get("Record2/ProbingSystem/Type").map(str::to_string);
    for (name, path) in [
        ("Revision", "Record1/Revision"),
        ("Creator", "Record2/Creator"),
        ("Serial", "Record2/Instrument/Serial"),
        ("Software version", "Record2/Instrument/Version"),
        ("Calibration date", "Record2/CalibrationDate"),
        ("Comment", "Record2/Comment"),
    ] {
        meta(&mut pg, name, x.get(path).unwrap_or(""));
    }
    meta(&mut pg, "Z data type", dtype);
    meta(&mut pg, "Z increment", format!("{inc} m"));
    if layers > 1 {
        pg.axis.kind = AxisKind::Series;
        pg.axis.name = "Layer".into();
        pg.axis.linear = Some(Lateral::new(0.0, 1.0, ""));
    }
    let mut rest = values;
    for _ in 0..layers {
        let tail = rest.split_off(n.min(rest.len()));
        pg.planes.push(height(rest));
        rest = tail;
    }
    Ok(Dataset { pages: vec![pg] })
}

/// A linked file, relative to the folder of `main.xml` (or the archive root).
fn find<'z>(zip: &'z Zip, dir: &str, link: &str) -> Option<&'z Entry> {
    let link = link.trim().trim_start_matches("./").replace('\\', "/");
    zip.find(&format!("{dir}{link}"))
        .or_else(|| zip.find(&link))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// main.xml nested 200 000 deep: refused at the depth cap instead of storing a path of
    /// every depth (quadratic in the depth).
    #[test]
    fn deep_main_xml_refused() {
        let d = 200_000;
        let x = format!("<a>{}{}</a>", "<b>".repeat(d), "</b>".repeat(d));
        assert!(matches!(parse_xml(x.as_bytes()), Err(Error::Malformed(_))));
        let ok = format!("<a>{}x{}</a>", "<b>".repeat(10), "</b>".repeat(10));
        assert!(parse_xml(ok.as_bytes()).is_ok());
    }

    #[test]
    fn parses_fields_and_data_list() {
        let x = parse_xml(
            b"<?xml version=\"1.0\"?><p:ISO5436_2 xmlns:p=\"x\"><Record1><FeatureType>SUR\
              </FeatureType><Axes><CZ><Increment>1e-9</Increment></CZ></Axes></Record1>\
              <Record3><DataList><Datum>1.5</Datum><Datum/><Datum></Datum></DataList>\
              </Record3></p:ISO5436_2>",
        )
        .unwrap();
        assert_eq!(x.get("Record1/FeatureType"), Some("SUR"));
        assert_eq!(x.num("Record1/Axes/CZ/Increment"), Some(1e-9));
        assert_eq!(x.data, ["1.5", "", ""]);
    }

    #[test]
    fn rejects_non_zip() {
        assert!(!probe_file(b"PK\x03\x04garbage"));
        assert!(read(b"nope").is_err());
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A ZIP of stored members.
    fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut f = Vec::new();
        let mut dir = Vec::new();
        for (name, data) in members {
            let local = f.len() as u32;
            let size = (data.len() as u32).to_le_bytes();
            f.extend_from_slice(b"PK\x03\x04");
            f.extend_from_slice(&[0; 14]);
            f.extend_from_slice(&size);
            f.extend_from_slice(&size);
            f.extend_from_slice(&(name.len() as u16).to_le_bytes());
            f.extend_from_slice(&[0; 2]);
            f.extend_from_slice(name.as_bytes());
            f.extend_from_slice(data);
            dir.extend_from_slice(b"PK\x01\x02");
            dir.extend_from_slice(&[0; 16]);
            dir.extend_from_slice(&size);
            dir.extend_from_slice(&size);
            dir.extend_from_slice(&(name.len() as u16).to_le_bytes());
            dir.extend_from_slice(&[0; 12]);
            dir.extend_from_slice(&local.to_le_bytes());
            dir.extend_from_slice(name.as_bytes());
        }
        let at = f.len() as u32;
        f.extend_from_slice(&dir);
        f.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        let n = (members.len() as u16).to_le_bytes();
        f.extend_from_slice(&n);
        f.extend_from_slice(&n);
        f.extend_from_slice(&(dir.len() as u32).to_le_bytes());
        f.extend_from_slice(&at.to_le_bytes());
        f.extend_from_slice(&[0; 2]);
        f
    }

    fn main_xml(side: usize, data: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?><p:ISO5436_2 xmlns:p=\"x\"><Record1><FeatureType>SUR\
             </FeatureType><Axes><CZ><DataType>D</DataType></CZ></Axes></Record1><Record3>\
             <MatrixDimension><SizeX>{side}</SizeX><SizeY>{side}</SizeY><SizeZ>1</SizeZ>\
             </MatrixDimension>{data}</Record3></p:ISO5436_2>"
        )
    }

    /// A 16384² surface with three listed points: the rest would have been padded to a
    /// 1 GiB plane.
    #[test]
    fn data_list_far_shorter_than_the_matrix() {
        let list = "<DataList><Datum>1</Datum><Datum>2</Datum><Datum>3</Datum></DataList>";
        let f = zip(&[("main.xml", main_xml(16384, list).as_bytes())]);
        assert_eq!(read(&f).unwrap_err(), Error::Truncated("data list"));
        let f = zip(&[("main.xml", main_xml(2, list).as_bytes())]);
        assert_eq!(read(&f).unwrap().pages[0].width, 2);
    }

    /// A 16384² surface whose binary point data is 8 bytes.
    #[test]
    fn point_data_far_shorter_than_the_matrix() {
        let link = "<DataLink><PointDataLink>bindata/data.bin</PointDataLink></DataLink>";
        let f = zip(&[
            ("main.xml", main_xml(16384, link).as_bytes()),
            ("bindata/data.bin", &[0; 8]),
        ]);
        assert_eq!(read(&f).unwrap_err(), Error::Truncated("point data"));
    }
}
