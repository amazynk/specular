//! NRRD headers (`.nrrd` with the data attached, `.nhdr` with it in other files).

use std::path::PathBuf;

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Result,
    Skip, numbers, text_lines,
};

/// Where attached data starts: after the first empty line.
pub(crate) fn data_start(file: &[u8]) -> Result<usize> {
    for (line, end) in text_lines(file) {
        if line.is_empty() {
            return Ok(end);
        }
    }
    Err(Error::Truncated(
        "header (no empty line before the data)".into(),
    ))
}

fn dtype(s: &str) -> Result<DataType> {
    let s = s.trim().to_ascii_lowercase();
    Ok(match s.as_str() {
        "signed char" | "int8" | "int8_t" => DataType::I8,
        "uchar" | "unsigned char" | "uint8" | "uint8_t" => DataType::U8,
        "short" | "short int" | "signed short" | "signed short int" | "int16" | "int16_t" => {
            DataType::I16
        }
        "ushort" | "unsigned short" | "unsigned short int" | "uint16" | "uint16_t" => DataType::U16,
        "int" | "signed int" | "int32" | "int32_t" => DataType::I32,
        "uint" | "unsigned int" | "uint32" | "uint32_t" => DataType::U32,
        "longlong"
        | "long long"
        | "long long int"
        | "signed long long"
        | "signed long long int"
        | "int64"
        | "int64_t" => DataType::I64,
        "ulonglong" | "unsigned long long" | "unsigned long long int" | "uint64" | "uint64_t" => {
            DataType::U64
        }
        "float" => DataType::F32,
        "double" => DataType::F64,
        "block" => return Err(Error::Unsupported("NRRD block samples".into())),
        _ => return Err(Error::Malformed(format!("NRRD type {s:?}"))),
    })
}

fn encoding(s: &str) -> Result<Encoding> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "raw" => Encoding::Raw,
        "txt" | "text" | "ascii" => Encoding::Ascii,
        "hex" => Encoding::Hex,
        "gz" | "gzip" => Encoding::Gzip,
        "bz2" | "bzip2" => Encoding::Bzip2,
        e => return Err(Error::Unsupported(format!("NRRD encoding {e:?}"))),
    })
}

/// The matrix taking a space's coordinates to LPS (diagonal: sign per axis).
fn space_to_lps(space: &str) -> Result<[f64; 3]> {
    Ok(match space.trim().to_ascii_lowercase().as_str() {
        "right-anterior-superior" | "ras" | "right-anterior-superior-time" | "rast" => {
            [-1.0, -1.0, 1.0]
        }
        "left-anterior-superior" | "las" | "left-anterior-superior-time" | "last" => {
            [1.0, -1.0, 1.0]
        }
        "left-posterior-superior" | "lps" | "left-posterior-superior-time" | "lpst" => {
            [1.0, 1.0, 1.0]
        }
        "scanner-xyz"
        | "scanner-xyz-time"
        | "3d-right-handed"
        | "3d-left-handed"
        | "3d-right-handed-time"
        | "3d-left-handed-time" => [1.0, 1.0, 1.0],
        s => return Err(Error::Unsupported(format!("NRRD space {s:?}"))),
    })
}

/// A vector "(x,y,z)", or `None` for "none".
fn vector(s: &str) -> Result<Option<Vec<f64>>> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let inner = s
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| Error::Malformed(format!("NRRD vector {s:?}")))?;
    numbers(inner)
        .map(Some)
        .ok_or_else(|| Error::Malformed(format!("NRRD vector {s:?}")))
}

/// The vectors of `space directions`: "(a,b,c) none (d,e,f)".
fn vectors(s: &str) -> Result<Vec<Option<Vec<f64>>>> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while !rest.is_empty() {
        if rest.starts_with('(') {
            let end = rest
                .find(')')
                .ok_or_else(|| Error::Malformed("NRRD space directions".into()))?;
            out.push(vector(&rest[..=end])?);
            rest = rest[end + 1..].trim_start();
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(vector(&rest[..end])?);
            rest = rest[end..].trim_start();
        }
    }
    Ok(out)
}

/// Kinds that make an axis something other than space.
fn component_kind(k: &str) -> bool {
    !matches!(
        k.to_ascii_lowercase().as_str(),
        "domain" | "space" | "none" | "???"
    )
}

fn colour_kind(k: &str) -> bool {
    matches!(
        k.to_ascii_lowercase().as_str(),
        "rgb-color" | "3-color" | "rgba-color" | "4-color"
    )
}

/// A `printf`-style file name with one integer field ("img%03d.raw").
fn format_name(pattern: &str, n: i64) -> Result<String> {
    let at = pattern
        .find('%')
        .ok_or_else(|| Error::Malformed(format!("NRRD data file pattern {pattern:?}")))?;
    let rest = &pattern[at + 1..];
    let spec_end = rest
        .find(|c: char| c.is_ascii_alphabetic())
        .ok_or_else(|| Error::Malformed(format!("NRRD data file pattern {pattern:?}")))?;
    let (flags, conv) = (&rest[..spec_end], &rest[spec_end..=spec_end]);
    if !matches!(conv, "d" | "i" | "u") {
        return Err(Error::Unsupported(format!(
            "NRRD data file pattern {pattern:?}"
        )));
    }
    let zero = flags.starts_with('0');
    let width: usize = flags.trim_start_matches('0').parse().unwrap_or(0);
    let num = if zero {
        format!("{n:0width$}")
    } else {
        format!("{n:width$}")
    };
    Ok(format!(
        "{}{}{}",
        &pattern[..at],
        num,
        &rest[spec_end + 1..]
    ))
}

/// Parses an NRRD header (`data`: the file, or at least its header).
pub fn parse(data: &[u8]) -> Result<Header> {
    if !data.starts_with(b"NRRD000") {
        return Err(Error::NotRecognised);
    }
    let mut lines = text_lines(data);
    lines.next();
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut list: Option<Vec<String>> = None;
    for (line, _) in lines.by_ref() {
        if let Some(names) = list.as_mut() {
            if !line.trim().is_empty() {
                names.push(line.trim().to_owned());
            }
            continue;
        }
        if line.is_empty() {
            break;
        }
        if line.starts_with('#') {
            continue;
        }
        // Key/value pairs ("key:=value") are not fields.
        if let Some(p) = line.find(": ") {
            if line[..p].ends_with(":=") || line[..p].contains(":=") {
                continue;
            }
            let key = line[..p].trim().to_ascii_lowercase();
            let value = line[p + 2..].trim().to_owned();
            let is_list = (key == "data file" || key == "datafile")
                && value.split_whitespace().next() == Some("LIST");
            fields.push((key, value));
            if is_list {
                list = Some(Vec::new());
            }
        } else if line.contains(":=") {
            continue;
        } else {
            return Err(Error::Malformed(format!("NRRD header line {line:?}")));
        }
    }
    let get = |k: &str| {
        fields
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    let need = |k: &str| get(k).ok_or_else(|| Error::Malformed(format!("NRRD: no {k:?}")));

    let dt = dtype(need("type")?)?;
    let n: usize = need("dimension")?
        .trim()
        .parse()
        .map_err(|_| Error::Malformed("NRRD dimension".into()))?;
    if !(1..=16).contains(&n) {
        return Err(Error::Malformed(format!("NRRD dimension {n}")));
    }
    let sizes: Vec<usize> = need("sizes")?
        .split_whitespace()
        .map(|t| t.parse::<usize>().ok().filter(|&x| x > 0))
        .collect::<Option<_>>()
        .filter(|v: &Vec<usize>| v.len() == n)
        .ok_or_else(|| Error::Malformed("NRRD sizes".into()))?;
    let enc = encoding(need("encoding")?)?;
    let big = match get("endian").map(|e| e.trim().to_ascii_lowercase()) {
        Some(e) if e == "big" => true,
        Some(e) if e == "little" => false,
        Some(e) => return Err(Error::Malformed(format!("NRRD endian {e:?}"))),
        None => false,
    };
    let kinds: Option<Vec<String>> =
        get("kinds").map(|k| k.split_whitespace().map(str::to_owned).collect::<Vec<_>>());
    let kinds = kinds.filter(|k| k.len() == n);
    let space_signs = match get("space") {
        Some(s) => Some(space_to_lps(s)?),
        None => get("space dimension").map(|_| [1.0; 3]),
    };
    let directions = match get("space directions") {
        Some(s) => {
            let v = vectors(s)?;
            if v.len() != n {
                return Err(Error::Malformed("NRRD space directions".into()));
            }
            Some(v)
        }
        None => None,
    };
    let spacings: Option<Vec<f64>> = get("spacings").and_then(|s| {
        s.split_whitespace()
            .map(|t| t.parse::<f64>().ok())
            .collect::<Option<Vec<_>>>()
            .filter(|v| v.len() == n)
    });

    // Which axes are space, which colour, which the rest.
    let mut spatial: Vec<usize> = (0..n)
        .filter(|&a| match (&directions, &kinds) {
            (Some(d), _) => d[a].is_some(),
            (None, Some(k)) => !component_kind(&k[a]),
            (None, None) => true,
        })
        .collect();
    let colour_axis = kinds.as_ref().and_then(|k| {
        (0..n).find(|&a| {
            colour_kind(&k[a])
                && dt == DataType::U8
                && matches!(sizes[a], 3 | 4)
                && !spatial[..spatial.len().min(3)].contains(&a)
        })
    });
    if directions.is_none() && kinds.is_none() && n > 3 {
        // No hint: a short first axis of a 4-D array is the components; else the last.
        if sizes[0] <= 4 {
            spatial.remove(0);
        }
    }
    spatial.truncate(3);
    let mut strides = Vec::with_capacity(n);
    let mut s = 1usize;
    for &z in &sizes {
        strides.push(s);
        s = s.checked_mul(z).ok_or(Error::TooLarge)?;
    }
    let samples = s;
    let mut size = [1usize; 3];
    let mut stride = [samples; 3];
    for (t, &a) in spatial.iter().enumerate() {
        size[t] = sizes[a];
        stride[t] = strides[a];
    }
    let extra: Vec<(usize, usize)> = (0..n)
        .filter(|a| !spatial.contains(a) && Some(*a) != colour_axis)
        .map(|a| (sizes[a], strides[a]))
        .collect();
    let layout = Layout {
        size,
        stride,
        extra,
        colour: colour_axis.map(|a| (sizes[a], strides[a])),
        samples,
    };

    // Geometry.
    let mut affine = [[0.0; 4]; 3];
    let orientation;
    let signs = space_signs.unwrap_or([1.0; 3]);
    match &directions {
        Some(d) => {
            orientation = if space_signs.is_some() {
                OrientationSource::Space
            } else {
                OrientationSource::Spacings
            };
            for (t, &a) in spatial.iter().enumerate() {
                let v = d[a].as_ref().expect("spatial axes have directions");
                for r in 0..3 {
                    affine[r][t] = v.get(r).copied().unwrap_or(0.0) * signs[r];
                }
            }
            if let Some(o) = get("space origin") {
                let o = vector(o)?.unwrap_or_default();
                for r in 0..3 {
                    affine[r][3] = o.get(r).copied().unwrap_or(0.0) * signs[r];
                }
            }
        }
        None => {
            orientation = OrientationSource::Spacings;
            for (t, &a) in spatial.iter().enumerate() {
                let sp = spacings
                    .as_ref()
                    .map(|s| s[a])
                    .filter(|x| x.is_finite() && *x != 0.0)
                    .unwrap_or(1.0);
                affine[t][t] = sp;
            }
        }
    }
    // Axes padded to three: unit steps along whatever is free.
    for t in spatial.len()..3 {
        let cols: Vec<[f64; 3]> = (0..t)
            .map(|c| [affine[0][c], affine[1][c], affine[2][c]])
            .collect();
        let pad = match cols.as_slice() {
            [a, b] => {
                let c = crate::cross(*a, *b);
                let l = crate::norm(c);
                if l > 0.0 {
                    c.map(|x| x / l)
                } else {
                    [0.0, 0.0, 1.0]
                }
            }
            _ => {
                let mut e = [0.0; 3];
                e[t] = 1.0;
                e
            }
        };
        for r in 0..3 {
            affine[r][t] = pad[r];
        }
    }
    if affine.iter().flatten().any(|x| !x.is_finite()) {
        return Err(Error::Malformed("NRRD geometry".into()));
    }
    let spacing = [0, 1, 2].map(|c| {
        let l = (affine[0][c].powi(2) + affine[1][c].powi(2) + affine[2][c].powi(2)).sqrt();
        if l > 0.0 { l } else { 1.0 }
    });

    // The data.
    let lines_skip = get("line skip")
        .or(get("lineskip"))
        .map(|s| s.trim().parse::<usize>())
        .transpose()
        .map_err(|_| Error::Malformed("NRRD line skip".into()))?
        .unwrap_or(0);
    let byte_skip = get("byte skip")
        .or(get("byteskip"))
        .map(|s| s.trim().parse::<i64>())
        .transpose()
        .map_err(|_| Error::Malformed("NRRD byte skip".into()))?
        .unwrap_or(0);
    let skip = match byte_skip {
        -1 => Skip::FromEnd,
        b if b >= 0 => Skip::Start {
            lines: lines_skip,
            bytes: b as usize,
        },
        b => return Err(Error::Malformed(format!("NRRD byte skip {b}"))),
    };
    let file = match get("data file").or(get("datafile")) {
        None => DataFile::Attached,
        Some(v) => {
            let parts: Vec<&str> = v.split_whitespace().collect();
            match parts.as_slice() {
                ["LIST", ..] => DataFile::Detached(
                    list.unwrap_or_default()
                        .into_iter()
                        .map(PathBuf::from)
                        .collect(),
                ),
                [pattern, min, max, step, ..] if pattern.contains('%') => {
                    let p = |s: &str| {
                        s.parse::<i64>()
                            .map_err(|_| Error::Malformed("NRRD data file range".into()))
                    };
                    let (lo, hi, st) = (p(min)?, p(max)?, p(step)?);
                    if st == 0 || (hi - lo) / st > 1 << 20 || (hi - lo).signum() * st.signum() < 0 {
                        return Err(Error::Malformed("NRRD data file range".into()));
                    }
                    let mut names = Vec::new();
                    let mut i = lo;
                    while (st > 0 && i <= hi) || (st < 0 && i >= hi) {
                        names.push(PathBuf::from(format_name(pattern, i)?));
                        i += st;
                    }
                    DataFile::Detached(names)
                }
                _ => DataFile::Detached(vec![PathBuf::from(v.trim())]),
            }
        }
    };
    if matches!(&file, DataFile::Detached(f) if f.is_empty()) {
        return Err(Error::Malformed("NRRD data file list".into()));
    }

    let mut dims = sizes.clone();
    dims.shrink_to_fit();
    Ok(Header {
        kind: Kind::Nrrd,
        dtype: dt,
        layout,
        affine,
        orientation,
        dims,
        spacing,
        scale: None,
        cal: None,
        description: get("content").map(str::to_owned).filter(|s| !s.is_empty()),
        intent: None,
        anatomical: get("space").map(str::to_owned),
        params: Vec::new(),
        data: DataRef {
            file,
            skip,
            encoding: enc,
            big_endian: big,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_a_pattern() {
        assert_eq!(format_name("s%03d.raw", 7).unwrap(), "s007.raw");
        assert_eq!(format_name("s%d.raw", 12).unwrap(), "s12.raw");
    }

    #[test]
    fn directions_with_none() {
        let v = vectors("none (1,0,0) (0, 2,0) (0,0,3)").unwrap();
        assert_eq!(v.len(), 4);
        assert!(v[0].is_none());
        assert_eq!(v[2].as_deref(), Some(&[0.0, 2.0, 0.0][..]));
    }

    #[test]
    fn ras_is_flipped_to_lps() {
        let h = b"NRRD0004\ntype: short\ndimension: 3\nsizes: 2 3 4\nencoding: raw\nendian: little\nspace: RAS\nspace directions: (1,0,0) (0,2,0) (0,0,3)\nspace origin: (10,20,30)\n\n";
        let h = parse(h).unwrap();
        assert_eq!(h.affine[0], [-1.0, 0.0, 0.0, -10.0]);
        assert_eq!(h.affine[1], [0.0, -2.0, 0.0, -20.0]);
        assert_eq!(h.affine[2], [0.0, 0.0, 3.0, 30.0]);
        assert_eq!(h.orientation, OrientationSource::Space);
    }
}
