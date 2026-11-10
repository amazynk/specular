//! MRtrix `.tck` streamlines, from MRtrix's file format documentation.
//!
//! A text header (`mrtrix tracks`, then `key: value` lines up to `END`) whose `file: . <offset>`
//! says where the points start and whose `datatype` is `Float32LE`, `Float32BE`, `Float64LE`
//! or `Float64BE`. The points are x, y, z triplets in scanner space (RAS millimetres); a
//! triplet of NaNs ends a streamline and one of infinities ends the file.

use crate::{Error, Result, Tractogram};

/// Longest header accepted.
const MAX_HEADER: usize = 1 << 20;
/// Most points accepted.
pub const MAX_POINTS: usize = 1 << 28;

#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// Every `key: value` line but `file` and `datatype`, in order (repeated keys kept).
    pub fields: Vec<(String, String)>,
    pub offset: usize,
    pub big_endian: bool,
    pub double: bool,
    /// The `count` the header states.
    pub count: Option<usize>,
}

impl Header {
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.0 == key)
            .map(|f| f.1.as_str())
    }
}

pub fn sniff(data: &[u8]) -> bool {
    data.starts_with(b"mrtrix tracks")
}

/// Reads the text header.
pub fn header(data: &[u8]) -> Result<Header> {
    if !sniff(data) {
        return Err(Error::NotRecognised);
    }
    let end = data[..data.len().min(MAX_HEADER)]
        .windows(4)
        .position(|w| w == b"END\n" || w == b"END\r")
        .ok_or_else(|| Error::Truncated("MRtrix tracks header".into()))?;
    let text = String::from_utf8_lossy(&data[..end]);
    let mut fields = Vec::new();
    let mut offset = None;
    let mut dtype = None;
    for line in text.lines().skip(1) {
        let Some((k, v)) = crate::key_value(line) else {
            continue;
        };
        match k {
            "file" => {
                let mut it = v.split_whitespace();
                if it.next() != Some(".") {
                    return Err(Error::Unsupported(
                        "MRtrix tracks whose points are in another file are not supported".into(),
                    ));
                }
                offset = it.next().and_then(|o| o.parse::<usize>().ok());
            }
            "datatype" => dtype = Some(v.to_owned()),
            _ => fields.push((k.to_owned(), v.to_owned())),
        }
    }
    let offset = offset.ok_or_else(|| Error::Malformed("MRtrix tracks: no data offset".into()))?;
    if offset < end + 4 {
        return Err(Error::Malformed("MRtrix tracks: data offset".into()));
    }
    let (double, big_endian) = match dtype.as_deref() {
        Some("Float32LE") => (false, false),
        Some("Float32BE") => (false, true),
        Some("Float64LE") => (true, false),
        Some("Float64BE") => (true, true),
        Some(t) => {
            return Err(Error::Unsupported(format!(
                "MRtrix tracks of type {t} are not supported"
            )));
        }
        None => return Err(Error::Malformed("MRtrix tracks: no datatype".into())),
    };
    let count = fields
        .iter()
        .find(|f| f.0 == "count")
        .and_then(|f| f.1.parse().ok());
    Ok(Header {
        fields,
        offset,
        big_endian,
        double,
        count,
    })
}

/// Reads a whole `.tck` file.
pub fn parse(data: &[u8]) -> Result<(Header, Tractogram)> {
    let h = header(data)?;
    let body = data
        .get(h.offset..)
        .ok_or_else(|| Error::Truncated("MRtrix tracks".into()))?;
    let size = if h.double { 8 } else { 4 };
    let value = |b: &[u8]| -> f32 {
        if h.double {
            let a: [u8; 8] = b.try_into().unwrap_or([0; 8]);
            (if h.big_endian {
                f64::from_be_bytes(a)
            } else {
                f64::from_le_bytes(a)
            }) as f32
        } else {
            let a: [u8; 4] = b.try_into().unwrap_or([0; 4]);
            if h.big_endian {
                f32::from_be_bytes(a)
            } else {
                f32::from_le_bytes(a)
            }
        }
    };
    let mut t = Tractogram {
        offsets: vec![0],
        ..Tractogram::default()
    };
    let triplets = body.chunks_exact(3 * size);
    if triplets.len() > MAX_POINTS {
        return Err(Error::TooLarge);
    }
    t.points.reserve(triplets.len());
    let mut ended = false;
    for c in triplets {
        let p = [
            value(&c[..size]),
            value(&c[size..2 * size]),
            value(&c[2 * size..]),
        ];
        if p.iter().all(|v| v.is_infinite()) {
            ended = true;
            break;
        }
        if p.iter().all(|v| v.is_nan()) {
            // An empty streamline is skipped, as MRtrix does.
            if *t.offsets.last().unwrap_or(&0) as usize != t.points.len() {
                t.offsets.push(t.points.len() as u32);
            }
            continue;
        }
        t.points.push(p);
    }
    // A file still being written may stop without the end marker: its last streamline is
    // dropped when it has no terminator.
    if *t.offsets.last().unwrap_or(&0) as usize != t.points.len() {
        let last = *t.offsets.last().unwrap_or(&0) as usize;
        t.points.truncate(last);
    }
    if !ended && t.streamlines() == 0 && h.count.unwrap_or(0) > 0 {
        return Err(Error::Truncated("MRtrix tracks".into()));
    }
    Ok((h, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(points: &[[f32; 3]]) -> Vec<u8> {
        let mut b =
            b"mrtrix tracks\ncount: 0000000001\ndatatype: Float32LE\nfile: . 80\nEND\n".to_vec();
        b.resize(80, 0);
        for p in points {
            for v in p {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b
    }

    #[test]
    fn reads_streamlines() {
        let n = f32::NAN;
        let i = f32::INFINITY;
        let (h, t) = parse(&file(&[
            [1.0, 2.0, 3.0],
            [4.0, 5.0, 6.0],
            [n, n, n],
            [7.0, 8.0, 9.0],
            [n, n, n],
            [i, i, i],
        ]))
        .unwrap();
        assert_eq!(h.count, Some(1));
        assert_eq!(t.streamlines(), 2);
        assert_eq!(t.streamline(1), &[[7.0, 8.0, 9.0]]);
    }

    #[test]
    fn refuses_bad_headers() {
        assert!(parse(b"mrtrix tracks\nfile: . 10\n").is_err());
        assert!(parse(b"mrtrix tracks\ndatatype: Int16\nfile: . 64\nEND\n").is_err());
        assert!(parse(b"mrtrix tracks\ndatatype: Float32LE\nfile: . 2\nEND\n").is_err());
    }
}
