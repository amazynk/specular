//! SAGA GIS grids: a `.sgrd` text header (`KEY = VALUE`: data format, byte order, cell
//! counts, the lower-left cell centre, cell size, z factor, no-data value or range, row
//! order) beside a `.sdat` raw band, and the `.sg-grd-z` ZIP holding both (with a `.prj`).

use std::io::Read;

use crate::geotiff::Affine2;
use crate::rawgrid::{Interleave, RawLayout, SampleType};

/// A parsed header.
#[derive(Clone, Debug)]
pub struct Header {
    pub layout: RawLayout,
    /// Values from the first to the second (inclusive) are no data.
    pub nodata: Option<(f64, f64)>,
    /// Stored values times this are the grid's values.
    pub z_factor: f64,
    /// Rows run from the top (else the first row is the bottom one).
    pub top_to_bottom: bool,
    pub transform: Affine2,
    pub name: Option<String>,
    pub description: Option<String>,
    pub unit: Option<String>,
    pub keys: Vec<(String, String)>,
}

fn entries(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            let k = k.trim();
            (!k.is_empty()).then(|| (k.to_ascii_uppercase(), v.trim().to_owned()))
        })
        .take(4096)
        .collect()
}

/// Whether the text is a SAGA grid header.
pub fn sniff(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    if t.contains('\0') {
        return false;
    }
    let e = entries(&t);
    let has = |k: &str| e.iter().any(|(n, _)| n == k);
    has("CELLCOUNT_X") && has("CELLCOUNT_Y") && has("DATAFORMAT") && has("POSITION_XMIN")
}

fn data_format(v: &str) -> Result<SampleType, String> {
    Ok(match v.trim().to_ascii_uppercase().as_str() {
        "BYTE_UNSIGNED" => SampleType::U8,
        "BYTE" => SampleType::I8,
        "SHORTINT_UNSIGNED" => SampleType::U16,
        "SHORTINT" => SampleType::I16,
        "INTEGER_UNSIGNED" => SampleType::U32,
        "INTEGER" => SampleType::I32,
        "LONGINT_UNSIGNED" => SampleType::U64,
        "LONGINT" => SampleType::I64,
        "FLOAT" => SampleType::F32,
        "DOUBLE" => SampleType::F64,
        other => return Err(format!("SAGA data format {other} is not supported")),
    })
}

fn yes(v: &str) -> bool {
    matches!(v.trim().to_ascii_uppercase().as_str(), "TRUE" | "1" | "YES")
}

/// Parses a `.sgrd` header.
pub fn parse(text: &str) -> Result<Header, String> {
    let keys = entries(text);
    let get = |k: &str| keys.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    let num = |k: &str| {
        get(k)
            .and_then(|v| v.parse::<f64>().ok())
            .ok_or_else(|| format!("no {k} in the header"))
    };
    let count = |k: &str| {
        get(k)
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|n| *n > 0)
            .ok_or_else(|| format!("no {k} in the header"))
    };
    let (w, h) = (count("CELLCOUNT_X")?, count("CELLCOUNT_Y")?);
    let sample = data_format(get("DATAFORMAT").unwrap_or(""))?;
    let offset = get("DATAFILE_OFFSET")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let big = get("BYTEORDER_BIG").is_some_and(yes);
    let layout = RawLayout::packed(w, h, 1, sample, big, Interleave::Bsq, offset);
    let cs = num("CELLSIZE")?;
    if cs <= 0.0 || !cs.is_finite() {
        return Err("cell size".into());
    }
    let (xmin, ymin) = (num("POSITION_XMIN")?, num("POSITION_YMIN")?);
    // The positions are cell centres; the top of the grid is the last row's centre plus
    // half a cell.
    let transform = Affine2 {
        a: cs,
        b: 0.0,
        c: xmin - cs / 2.0,
        d: 0.0,
        e: -cs,
        f: ymin + (f64::from(h) - 0.5) * cs,
    };
    let nodata = get("NODATA_VALUE").and_then(|v| {
        let mut it = v.split(';').map(|s| s.trim().parse::<f64>());
        let a = it.next()?.ok()?;
        let b = it.next().and_then(Result::ok).unwrap_or(a);
        Some((a.min(b), a.max(b)))
    });
    let z_factor = get("Z_FACTOR")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|z| z.is_finite() && *z != 0.0)
        .unwrap_or(1.0);
    let text_of = |k: &str| get(k).filter(|s| !s.is_empty()).map(str::to_owned);
    Ok(Header {
        layout,
        nodata,
        z_factor,
        top_to_bottom: get("TOPTOBOTTOM").is_some_and(yes),
        transform,
        name: text_of("NAME"),
        description: text_of("DESCRIPTION"),
        unit: text_of("UNIT"),
        keys,
    })
}

impl Header {
    /// The grid's values, row 0 the top: no data NaN, times the z factor. Rows past the end
    /// of `data` are NaN, and the result says whether any were.
    pub fn values(&self, data: &[u8]) -> (Vec<f32>, bool) {
        let (mut v, truncated) = self.layout.band(data, 0, None);
        let w = self.layout.width as usize;
        if !self.top_to_bottom && w > 0 {
            let h = v.len() / w;
            for y in 0..h / 2 {
                let (a, b) = v.split_at_mut((h - 1 - y) * w);
                a[y * w..(y + 1) * w].swap_with_slice(&mut b[..w]);
            }
        }
        let z = self.z_factor as f32;
        for x in &mut v {
            let f = f64::from(*x);
            if self.nodata.is_some_and(|(lo, hi)| f >= lo && f <= hi) {
                *x = f32::NAN;
            } else if z != 1.0 {
                *x *= z;
            }
        }
        (v, truncated)
    }
}

/// The members of a `.sg-grd-z` ZIP: the header text, the data and a `.prj` if any.
pub struct Zipped {
    pub header: String,
    pub data: Vec<u8>,
    pub prj: Option<String>,
}

/// Reads a `.sg-grd-z` (a ZIP of `.sgrd`, `.sdat` and `.prj`), each member at most `max`
/// bytes.
pub fn read_zip(bytes: &[u8], max: u64) -> Result<Zipped, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("not a ZIP archive: {e}"))?;
    let names: Vec<String> = z.file_names().flatten().map(String::from).collect();
    let find = |ext: &str| {
        names
            .iter()
            .find(|n| n.to_ascii_lowercase().ends_with(ext))
            .cloned()
    };
    let mut read = |name: &str| -> Result<Vec<u8>, String> {
        let f = z.by_name(name).map_err(|e| e.to_string())?;
        if f.size() > max {
            return Err("a member of the archive is too large".into());
        }
        let mut v = Vec::new();
        f.take(max).read_to_end(&mut v).map_err(|e| e.to_string())?;
        Ok(v)
    };
    let h = find(".sgrd").ok_or("no .sgrd header in the archive")?;
    let d = find(".sdat").ok_or("no .sdat data in the archive")?;
    let header = String::from_utf8_lossy(&read(&h)?).into_owned();
    let data = read(&d)?;
    let prj = match find(".prj") {
        Some(p) => Some(String::from_utf8_lossy(&read(&p)?).into_owned()),
        None => None,
    };
    Ok(Zipped { header, data, prj })
}

/// Whether bytes are a `.sg-grd-z`: a ZIP naming a `.sgrd` member near its start.
pub fn sniff_zip(head: &[u8]) -> bool {
    head.starts_with(b"PK\x03\x04")
        && head
            .get(30..30 + usize::from(u16::from_le_bytes([head[26], head[27]])))
            .is_some_and(|n| n.to_ascii_lowercase().ends_with(b".sgrd"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR: &str = "NAME\t= dem\nDESCRIPTION\t= \nUNIT\t= m\nDATAFILE_OFFSET\t= 0\nDATAFORMAT\t= SHORTINT\nBYTEORDER_BIG\t= TRUE\nPOSITION_XMIN\t= 105.0\nPOSITION_YMIN\t= 205.0\nCELLCOUNT_X\t= 3\nCELLCOUNT_Y\t= 2\nCELLSIZE\t= 10.0\nZ_FACTOR\t= 0.5\nNODATA_VALUE\t= -10;-5\nTOPTOBOTTOM\t= FALSE\n";

    #[test]
    fn header_and_values() {
        assert!(sniff(HDR.as_bytes()));
        let h = parse(HDR).unwrap();
        assert_eq!(h.layout.sample, SampleType::I16);
        assert_eq!(h.transform.apply(0.0, 0.0), (100.0, 220.0));
        assert_eq!(h.name.as_deref(), Some("dem"));
        assert_eq!(h.description, None);
        // Bottom row first: 1 2 3 / -7 5 6.
        let raw: Vec<u8> = [1i16, 2, 3, -7, 5, 6]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let (v, t) = h.values(&raw);
        assert!(!t);
        assert!(v[0].is_nan());
        assert_eq!(&v[1..], &[2.5, 3.0, 0.5, 1.0, 1.5]);
        assert!(h.values(&raw[..6]).1);
        assert!(parse(&HDR.replace("SHORTINT", "BIT")).is_err());
    }
}
