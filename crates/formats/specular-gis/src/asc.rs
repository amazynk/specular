//! Esri ASCII grids (`.asc`, Arc/Info ASCII GRID): a few `key value` header lines
//! (`ncols`, `nrows`, `xllcorner`/`xllcenter`, `yllcorner`/`yllcenter`, `cellsize` or
//! GDAL's `dx`/`dy`, optional `nodata_value`), then the values row by row from the top,
//! separated by any white space.

use rayon::prelude::*;

use crate::geotiff::Affine2;

/// Bytes of text parsed per parallel chunk.
const CHUNK: usize = 4 << 20;

/// The header.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub ncols: u32,
    pub nrows: u32,
    /// Pixel corner to map.
    pub transform: Affine2,
    pub nodata: Option<f64>,
    /// The position keys named pixel centres (`xllcenter`).
    pub centre: bool,
    /// Byte offset of the first value.
    pub data_start: usize,
    /// The header lines, for the info panel.
    pub keys: Vec<(String, String)>,
}

const KEYS: &[&str] = &[
    "ncols",
    "nrows",
    "xllcorner",
    "yllcorner",
    "xllcenter",
    "yllcenter",
    "cellsize",
    "dx",
    "dy",
    "nodata_value",
];

/// Whether a file starts like an ASCII grid (an `ncols` or `nrows` key first).
pub fn sniff(head: &[u8]) -> bool {
    let t = head
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map_or(&[][..], |i| &head[i..]);
    let word: Vec<u8> = t
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
        .map(u8::to_ascii_lowercase)
        .collect();
    if !((word == b"ncols" || word == b"nrows")
        && t[word.len()..].first().is_some_and(u8::is_ascii_whitespace))
    {
        return false;
    }
    // A complete header, then a number (an ESRI `.hdr` label also starts with NROWS or
    // NCOLS, but goes on with keys of its own).
    let Ok(h) = header(head) else {
        return false;
    };
    let rest = &head[h.data_start.min(head.len())..];
    match rest.iter().position(|b| !b.is_ascii_whitespace()) {
        None => true,
        Some(i) => matches!(
            rest[i],
            b'0'..=b'9' | b'-' | b'+' | b'.' | b',' | b'n' | b'N'
        ),
    }
}

/// Parses the header lines.
pub fn header(data: &[u8]) -> Result<Header, String> {
    let mut pos = 0;
    let mut vals: Vec<(String, String)> = Vec::new();
    loop {
        // Skip blank space, then read a word.
        while pos < data.len() && data[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let start = pos;
        let mut end = pos;
        while end < data.len() && !data[end].is_ascii_whitespace() {
            end += 1;
        }
        let word = std::str::from_utf8(&data[start..end])
            .unwrap_or("")
            .to_ascii_lowercase();
        if !KEYS.contains(&word.as_str()) {
            break;
        }
        // The value.
        let mut v = end;
        while v < data.len() && (data[v] == b' ' || data[v] == b'\t') {
            v += 1;
        }
        let mut ve = v;
        while ve < data.len() && !data[ve].is_ascii_whitespace() {
            ve += 1;
        }
        let value = String::from_utf8_lossy(&data[v..ve]).into_owned();
        vals.push((word, value));
        pos = ve;
        if vals.len() > KEYS.len() {
            return Err("repeated header keys".into());
        }
    }
    let get = |k: &str| {
        vals.iter()
            .find(|(n, _)| n == k)
            .and_then(|(_, v)| v.replace(',', ".").parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    let count = |k: &str| -> Result<u32, String> {
        get(k)
            .filter(|v| *v >= 1.0 && *v <= 1e7 && v.fract() == 0.0)
            .map(|v| v as u32)
            .ok_or_else(|| format!("no valid {k}"))
    };
    let ncols = count("ncols")?;
    let nrows = count("nrows")?;
    let (dx, dy) = match (get("cellsize"), get("dx"), get("dy")) {
        (Some(c), _, _) => (c, c),
        (None, Some(x), Some(y)) => (x, y),
        _ => return Err("no cellsize".into()),
    };
    if dx <= 0.0 || dy <= 0.0 {
        return Err("cell size not positive".into());
    }
    let (x0, y0, centre) = match (
        get("xllcorner"),
        get("yllcorner"),
        get("xllcenter"),
        get("yllcenter"),
    ) {
        (Some(x), Some(y), _, _) => (x, y, false),
        (_, _, Some(x), Some(y)) => (x - 0.5 * dx, y - 0.5 * dy, true),
        _ => return Err("no lower-left corner".into()),
    };
    Ok(Header {
        ncols,
        nrows,
        transform: Affine2 {
            a: dx,
            b: 0.0,
            c: x0,
            d: 0.0,
            e: -dy,
            f: y0 + dy * f64::from(nrows),
        },
        // Compared and reported at the float32 precision GDAL reads grids in; "nan" allowed.
        nodata: vals
            .iter()
            .find(|(n, _)| n == "nodata_value")
            .and_then(|(_, v)| v.replace(',', ".").parse::<f64>().ok())
            .filter(|v| !v.is_infinite())
            .map(|v| f64::from(v as f32)),
        centre,
        data_start: pos,
        keys: vals,
    })
}

/// The values parsed from one chunk of text: parsed numbers, and whether any was
/// written as a decimal (with `.` or an exponent).
fn parse_chunk(text: &[u8], nodata: Option<f64>) -> (Vec<f32>, bool) {
    let mut out = Vec::with_capacity(text.len() / 4);
    let mut float = false;
    for tok in text
        .split(|b| b.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
    {
        if !float && tok.iter().any(|b| matches!(b, b'.' | b',' | b'e' | b'E')) {
            float = true;
        }
        let s = std::str::from_utf8(tok).unwrap_or("");
        let v = s
            .parse::<f64>()
            .or_else(|_| s.replace(',', ".").parse())
            .unwrap_or(f64::NAN) as f32;
        out.push(match nodata {
            Some(n) if v == n as f32 => f32::NAN,
            _ => v,
        });
    }
    (out, float)
}

/// The grid's values, row 0 the northern edge, no-data as NaN; whether the file ended
/// early (missing values are NaN); whether the values are decimals rather than integers.
pub fn values(data: &[u8], h: &Header) -> (Vec<f32>, bool, bool) {
    let text = &data[h.data_start.min(data.len())..];
    // Chunks cut at white space.
    let mut cuts = vec![0];
    let mut at = 0;
    while at + CHUNK < text.len() {
        let mut c = at + CHUNK;
        while c < text.len() && !text[c].is_ascii_whitespace() {
            c += 1;
        }
        cuts.push(c);
        at = c;
    }
    cuts.push(text.len());
    let parts: Vec<(Vec<f32>, bool)> = cuts
        .par_windows(2)
        .map(|w| parse_chunk(&text[w[0]..w[1]], h.nodata))
        .collect();
    let n = h.ncols as usize * h.nrows as usize;
    let mut out = Vec::with_capacity(n);
    let mut float = false;
    for (p, f) in parts {
        float |= f;
        let take = (n - out.len()).min(p.len());
        out.extend_from_slice(&p[..take]);
    }
    let truncated = out.len() < n;
    out.resize(n, f32::NAN);
    (out, truncated, float)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid() {
        let t = b"ncols 3\r\nNROWS 2\nxllcorner 100.0\nyllcorner 200\ncellsize 10\nNODATA_value -9999\n1 2 3\n4 -9999 6.5\n";
        assert!(sniff(t));
        assert!(!sniff(b"-----BEGIN PGP SIGNATURE-----"));
        // An ESRI `.hdr` label is not a grid.
        assert!(!sniff(b"NROWS 2\nNCOLS 3\nNBANDS 1\nNBITS 16\n"));
        let h = header(t).unwrap();
        assert_eq!((h.ncols, h.nrows), (3, 2));
        assert_eq!(h.transform.apply(0.0, 0.0), (100.0, 220.0));
        assert_eq!(h.nodata, Some(-9999.0));
        let (v, trunc, float) = values(t, &h);
        assert!(!trunc && float);
        assert_eq!(v[..4], [1.0, 2.0, 3.0, 4.0]);
        assert!(v[4].is_nan());
        assert_eq!(v[5], 6.5);
        let c = b"ncols 2\nnrows 2\nxllcenter 0\nyllcenter 0\ndx 2\ndy 1\n1 2 3";
        let h = header(c).unwrap();
        assert!(h.centre);
        assert_eq!(h.transform.apply(0.0, 0.0), (-1.0, 1.5));
        let (v, trunc, float) = values(c, &h);
        assert!(trunc && !float);
        assert!(v[3].is_nan());
        assert!(header(b"ncols 2\nnrows 2\n1 2").is_err());
        // Decimal commas, and NaN as the no-data value.
        let d = b"ncols 2\nnrows 1\nxllcorner 0,100000E+06\nyllcorner 0,5E+02\ncellsize ,500E+02\nnodata_value nan\n  ,000E+00 1,5\n";
        assert!(sniff(d));
        let h = header(d).unwrap();
        assert_eq!(h.transform.apply(0.0, 0.0), (100000.0, 100.0));
        assert!(h.nodata.is_some_and(f64::is_nan));
        let (v, trunc, float) = values(d, &h);
        assert!(!trunc && float);
        assert_eq!(v, [0.0, 1.5]);
        // No-data compared at single precision, as the samples are.
        let f = b"ncols 2\nnrows 1\nxllcorner 0\nyllcorner 0\ncellsize 1\nnodata_value -3.4028234663852886e+38\n-3.4028234663852886e+38 2.5\n";
        let h = header(f).unwrap();
        let (v, _, _) = values(f, &h);
        assert!(v[0].is_nan() && v[1] == 2.5);
    }
}
