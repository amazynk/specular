//! MetaImage headers (`.mha` with the data attached, `.mhd` with it in other files).

use std::path::PathBuf;

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Result,
    Skip, numbers, text_lines,
};

/// Keys a MetaImage header starts with.
const FIRST: [&str; 7] = [
    "objecttype",
    "ndims",
    "comment",
    "objectsubtype",
    "transformtype",
    "name",
    "id",
];

fn split(line: &str) -> Option<(String, &str)> {
    let (k, v) = line.split_once('=')?;
    let k = k.trim();
    if k.is_empty() || !k.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return None;
    }
    Some((k.to_ascii_lowercase(), v.trim()))
}

/// Whether `data` starts like a MetaImage header.
pub fn sniff(data: &[u8]) -> bool {
    let Some((line, _)) = text_lines(data).find(|(l, _)| !l.trim().is_empty()) else {
        return false;
    };
    let Some((k, _)) = split(line) else {
        return false;
    };
    if !FIRST.contains(&k.as_str()) {
        return false;
    }
    // `Name`, `ID` and `Comment` start other key = value headers too (SAGA grids): then a
    // MetaImage key must follow.
    matches!(
        k.as_str(),
        "objecttype" | "ndims" | "objectsubtype" | "transformtype"
    ) || text_lines(data).take(64).any(|(l, _)| {
        split(l).is_some_and(|(k, _)| {
            matches!(
                k.as_str(),
                "ndims" | "objecttype" | "dimsize" | "elementtype"
            )
        })
    })
}

/// Where attached data starts: the line after `ElementDataFile`.
pub(crate) fn data_start(file: &[u8]) -> Result<usize> {
    for (line, end) in text_lines(file) {
        if split(line).is_some_and(|(k, _)| k == "elementdatafile") {
            return Ok(end);
        }
    }
    Err(Error::Malformed("MetaImage: no ElementDataFile".into()))
}

fn dtype(s: &str) -> Result<DataType> {
    Ok(match s.trim().to_ascii_uppercase().as_str() {
        "MET_CHAR" => DataType::I8,
        "MET_UCHAR" => DataType::U8,
        "MET_SHORT" => DataType::I16,
        "MET_USHORT" => DataType::U16,
        "MET_INT" | "MET_LONG" => DataType::I32,
        "MET_UINT" | "MET_ULONG" => DataType::U32,
        "MET_LONG_LONG" => DataType::I64,
        "MET_ULONG_LONG" => DataType::U64,
        "MET_FLOAT" => DataType::F32,
        "MET_DOUBLE" => DataType::F64,
        t => return Err(Error::Unsupported(format!("MetaImage element type {t}"))),
    })
}

fn truth(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

/// Parses a MetaImage header (`data`: the file, or at least its header).
pub fn parse(data: &[u8]) -> Result<Header> {
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut list: Option<Vec<String>> = None;
    for (line, _) in text_lines(data) {
        if let Some(names) = list.as_mut() {
            let t = line.trim();
            if !t.is_empty() {
                names.push(t.to_owned());
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let (k, v) = split(line)
            .ok_or_else(|| Error::Malformed(format!("MetaImage header line {line:?}")))?;
        let last = k == "elementdatafile";
        let is_list = last
            && v.split_whitespace()
                .next()
                .is_some_and(|w| w.eq_ignore_ascii_case("LIST"));
        fields.push((k, v.to_owned()));
        if is_list {
            list = Some(Vec::new());
        } else if last {
            break;
        }
    }
    let get = |k: &str| {
        fields
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    let need = |k: &str| get(k).ok_or_else(|| Error::Malformed(format!("MetaImage: no {k}")));
    let nums = |k: &str, n: usize| -> Result<Option<Vec<f64>>> {
        match get(k) {
            None => Ok(None),
            Some(v) => numbers(v)
                .filter(|x| x.len() >= n)
                .map(Some)
                .ok_or_else(|| Error::Malformed(format!("MetaImage {k}"))),
        }
    };

    let n: usize = need("ndims")?
        .parse()
        .map_err(|_| Error::Malformed("MetaImage NDims".into()))?;
    if !(1..=8).contains(&n) {
        return Err(Error::Malformed(format!("MetaImage NDims {n}")));
    }
    let sizes: Vec<usize> = need("dimsize")?
        .split_whitespace()
        .map(|t| t.parse::<usize>().ok().filter(|&x| x > 0))
        .collect::<Option<Vec<_>>>()
        .filter(|v| v.len() == n)
        .ok_or_else(|| Error::Malformed("MetaImage DimSize".into()))?;
    let dt = dtype(need("elementtype")?)?;
    let channels: usize = match get("elementnumberofchannels") {
        Some(c) => c
            .parse()
            .ok()
            .filter(|&c| c >= 1)
            .ok_or_else(|| Error::Malformed("MetaImage ElementNumberOfChannels".into()))?,
        None => 1,
    };
    let size = [0, 1, 2].map(|a| sizes.get(a).copied().unwrap_or(1));
    let extra_axes: Vec<usize> = sizes.iter().skip(3).copied().collect();
    let mut layout = Layout::dense(size, &extra_axes, channels).ok_or(Error::TooLarge)?;
    if channels > 1 {
        if dt == DataType::U8 && matches!(channels, 3 | 4) {
            layout.colour = Some((channels, 1));
        } else {
            layout.extra.insert(0, (channels, 1));
        }
    }

    // Geometry: columns are the axes' directions, scaled by the spacing.
    let spacing_v = nums("elementspacing", n)?
        .or(nums("elementsize", n)?)
        .unwrap_or_else(|| vec![1.0; n]);
    let offset = match nums("offset", n)? {
        Some(o) => o,
        None => match nums("origin", n)? {
            Some(o) => o,
            None => nums("position", n)?.unwrap_or_else(|| vec![0.0; n]),
        },
    };
    let matrix = match nums("transformmatrix", n * n)? {
        Some(m) => Some(m),
        None => match nums("rotation", n * n)? {
            Some(m) => Some(m),
            None => nums("orientation", n * n)?,
        },
    };
    let mut affine = [[0.0; 4]; 3];
    let d = n.min(3);
    for t in 0..3 {
        let sp = if t < n { spacing_v[t] } else { 1.0 };
        let sp = if sp.is_finite() && sp != 0.0 { sp } else { 1.0 };
        for r in 0..3 {
            let dir = if t < d && r < d {
                matrix
                    .as_ref()
                    .map_or(if r == t { 1.0 } else { 0.0 }, |m| m[t * n + r])
            } else if r == t {
                1.0
            } else {
                0.0
            };
            affine[r][t] = dir * sp;
        }
    }
    for r in 0..d {
        affine[r][3] = offset[r];
    }
    if affine.iter().flatten().any(|x| !x.is_finite()) {
        return Err(Error::Malformed("MetaImage geometry".into()));
    }
    let spacing = [0, 1, 2].map(|c| {
        let l = (affine[0][c].powi(2) + affine[1][c].powi(2) + affine[2][c].powi(2)).sqrt();
        if l > 0.0 { l } else { 1.0 }
    });

    // The data.
    let compressed = get("compresseddata").is_some_and(truth);
    let binary = get("binarydata").is_none_or(truth);
    let big = get("binarydatabyteordermsb")
        .or(get("elementbyteordermsb"))
        .is_some_and(truth);
    let encoding = match (binary, compressed) {
        (false, _) => Encoding::Ascii,
        (true, true) => Encoding::Zlib,
        (true, false) => Encoding::Raw,
    };
    let header_size = get("headersize")
        .map(|s| s.trim().parse::<i64>())
        .transpose()
        .map_err(|_| Error::Malformed("MetaImage HeaderSize".into()))?;
    let edf = need("elementdatafile")?.trim().to_owned();
    let words: Vec<&str> = edf.split_whitespace().collect();
    let file = if edf.eq_ignore_ascii_case("LOCAL") {
        DataFile::Attached
    } else if words
        .first()
        .is_some_and(|w| w.eq_ignore_ascii_case("LIST"))
    {
        let names = list.unwrap_or_default();
        if names.is_empty() {
            return Err(Error::Malformed("MetaImage ElementDataFile list".into()));
        }
        DataFile::Detached(names.into_iter().map(PathBuf::from).collect())
    } else if words.len() >= 4 && words[0].contains('%') {
        return Err(Error::Unsupported("MetaImage numbered data files".into()));
    } else {
        DataFile::Detached(vec![PathBuf::from(&edf)])
    };
    let skip = match (header_size, &file) {
        (_, DataFile::Attached) => Skip::Start { lines: 0, bytes: 0 },
        (Some(-1), _) => Skip::FromEnd,
        (Some(b), _) if b >= 0 => Skip::Start {
            lines: 0,
            bytes: b as usize,
        },
        (Some(b), _) => return Err(Error::Malformed(format!("MetaImage HeaderSize {b}"))),
        (None, _) => Skip::Start { lines: 0, bytes: 0 },
    };

    Ok(Header {
        kind: Kind::MetaImage,
        dtype: dt,
        layout,
        affine,
        orientation: OrientationSource::Transform,
        dims: sizes,
        spacing,
        scale: None,
        cal: None,
        description: get("comment")
            .or(get("name"))
            .map(str::to_owned)
            .filter(|s| !s.is_empty()),
        intent: None,
        anatomical: get("anatomicalorientation")
            .map(str::to_owned)
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("???")),
        params: Vec::new(),
        data: DataRef {
            file,
            skip,
            encoding,
            big_endian: big,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_and_parses() {
        let h = b"ObjectType = Image\nNDims = 3\nDimSize = 2 3 4\nElementType = MET_SHORT\nElementSpacing = 1 2 3\nOffset = 5 6 7\nTransformMatrix = 0 1 0 -1 0 0 0 0 1\nElementDataFile = LOCAL\n";
        assert!(sniff(h));
        let p = parse(h).unwrap();
        // Axis i runs along +y, axis j along -x.
        assert_eq!(p.affine[0], [0.0, -2.0, 0.0, 5.0]);
        assert_eq!(p.affine[1], [1.0, 0.0, 0.0, 6.0]);
        assert_eq!(p.affine[2], [0.0, 0.0, 3.0, 7.0]);
        assert_eq!(data_start(h).unwrap(), h.len());
    }
}
