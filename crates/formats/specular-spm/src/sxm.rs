//! Nanonis `.sxm` scans.
//!
//! A Latin-1 header of `:KEY:` lines, each followed by its value lines, ends at
//! `:SCANIT_END:`; the samples start after the `1A 04` that follows. `SCANIT_TYPE` gives the
//! sample type (`FLOAT MSBFIRST`, big-endian `f32`, the usual), `SCAN_PIXELS` the size,
//! `SCAN_RANGE` and `SCAN_OFFSET` (the scan's centre) the lateral calibration in metres,
//! `SCAN_DIR` whether the first stored line is the top (`down`) or the bottom (`up`), and the
//! `DATA_INFO` table the channels: name, unit and whether both directions were recorded. The
//! values are already calibrated. Per channel the forward plane comes first, then the
//! backward one, whose lines were recorded right to left and are mirrored here.

use specular_em::dataset::{Axis, AxisKind, Dataset, Field, Lateral, Page, Plane, PlaneData};
use specular_em::samples::{Order, Sample};

use crate::units::{latin1, mirror_rows, si};
use crate::{Error, Result};

const END: &[u8] = b":SCANIT_END:";

/// The largest plane accepted (samples).
const MAX_PLANE: usize = 1 << 28;

/// Whether `head` starts a Nanonis scan.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b":NANONIS_VERSION:")
}

/// The header's `:KEY:` entries with their value lines.
fn entries(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let l = line.trim_end_matches('\r');
        if l.len() > 2
            && l.starts_with(':')
            && l.ends_with(':')
            && !l[1..l.len() - 1].contains('\t')
        {
            out.push((l[1..l.len() - 1].to_string(), Vec::new()));
        } else if let Some((_, v)) = out.last_mut() {
            v.push(l.to_string());
        }
    }
    out
}

fn numbers(v: &[String]) -> Vec<f64> {
    v.iter()
        .flat_map(|l| l.split_whitespace())
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// A `DATA_INFO` row.
#[derive(Debug, PartialEq)]
struct Channel {
    name: String,
    unit: String,
    both: bool,
}

fn channels(rows: &[String]) -> Vec<Channel> {
    let mut lines = rows.iter().map(|l| l.trim()).filter(|l| !l.is_empty());
    let Some(head) = lines.next() else {
        return Vec::new();
    };
    let cols: Vec<&str> = head.split('\t').map(str::trim).collect();
    let col = |n: &str| cols.iter().position(|c| c.eq_ignore_ascii_case(n));
    let (Some(name), unit, dir) = (col("Name"), col("Unit"), col("Direction")) else {
        return Vec::new();
    };
    lines
        .map(|l| {
            let f: Vec<&str> = l.split('\t').map(str::trim).collect();
            let get = |i: Option<usize>| i.and_then(|i| f.get(i)).copied().unwrap_or("");
            Channel {
                name: get(Some(name)).to_string(),
                unit: get(unit).to_string(),
                both: get(dir).eq_ignore_ascii_case("both"),
            }
        })
        .collect()
}

/// Reads a Nanonis scan: one page, a forward plane per channel and a backward plane after it
/// when recorded.
pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotSxm);
    }
    let end = file
        .windows(END.len())
        .position(|w| w == END)
        .ok_or(Error::Truncated("header"))?;
    let start = file[end..]
        .windows(2)
        .position(|w| w == b"\x1a\x04")
        .map(|p| end + p + 2)
        .ok_or(Error::Truncated("data marker"))?;
    let entries = entries(&latin1(&file[..end]));
    let get = |k: &str| {
        entries
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_slice())
    };
    let pixels = numbers(get("SCAN_PIXELS").ok_or(Error::Malformed("no SCAN_PIXELS".into()))?);
    let (width, height) = match pixels[..] {
        [w, h, ..] if w >= 1.0 && h >= 1.0 => (w as usize, h as usize),
        _ => return Err(Error::Malformed("SCAN_PIXELS".into())),
    };
    let n = width
        .checked_mul(height)
        .filter(|&n| n <= MAX_PLANE)
        .ok_or(Error::TooLarge)?;
    let ty = get("SCANIT_TYPE")
        .map(|v| v.join(" ").to_ascii_uppercase())
        .unwrap_or_default();
    let order = if ty.contains("LSBFIRST") {
        Order::Little
    } else {
        Order::Big
    };
    let sample = if ty.contains("INT") {
        Sample::I32
    } else {
        Sample::F32
    };
    let bottom_up = get("SCAN_DIR")
        .and_then(|v| v.iter().map(|l| l.trim()).find(|l| !l.is_empty()))
        .is_some_and(|d| d.eq_ignore_ascii_case("up"));
    let chans = channels(get("DATA_INFO").unwrap_or(&[]));
    if chans.is_empty() {
        return Err(Error::NoImage);
    }
    let mut planes = Vec::new();
    let mut offset = start;
    for c in &chans {
        let (unit, f) = si(&c.unit);
        let stored = Plane {
            label: if c.both {
                format!("{} forward", c.name)
            } else {
                c.name.clone()
            },
            unit: unit.clone(),
            data: PlaneData::Stored {
                offset,
                sample,
                order,
                bottom_up,
                scale: f,
                add: 0.0,
            },
        };
        offset += n * 4;
        planes.push(stored);
        if c.both {
            let back = Plane {
                label: String::new(),
                unit: unit.clone(),
                data: PlaneData::Stored {
                    offset,
                    sample,
                    order,
                    bottom_up,
                    scale: f,
                    add: 0.0,
                },
            };
            // Read only when some of it is in the file (the forward planes before it are, so
            // the plane fits too); a missing plane reads as missing without a scan-sized buffer.
            let v = if offset < file.len() {
                let mut v = vec![0f32; n];
                back.read(file, width, height, &mut v);
                mirror_rows(&mut v, width);
                v
            } else {
                Vec::new()
            };
            planes.push(Plane::values(format!("{} backward", c.name), unit, v));
            offset += n * 4;
        }
    }
    let range = numbers(get("SCAN_RANGE").unwrap_or(&[]));
    let centre = numbers(get("SCAN_OFFSET").unwrap_or(&[]));
    let lateral = |i: usize, count: usize| {
        let r = *range.get(i)?;
        let c = centre.get(i).copied().unwrap_or(0.0);
        let step = r / count as f64;
        // From the scan's top-left corner, rows downwards.
        let l = Lateral::new(c - r / 2.0 + step / 2.0, step, "m");
        l.is_usable().then_some(l)
    };
    let text = |k: &str| {
        get(k)
            .map(|v| v.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" "))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let date = match (text("REC_DATE"), text("REC_TIME")) {
        (Some(d), Some(t)) => Some(format!("{d} {t}")),
        (d, _) => d,
    };
    let mut meta = Vec::new();
    for (k, v) in &entries {
        if matches!(k.as_str(), "DATA_INFO" | "SCANIT_TYPE") || v.len() != 1 {
            continue;
        }
        let v = v[0].trim();
        if !v.is_empty() && meta.len() < 400 {
            meta.push((k.clone(), v.to_string()));
        }
    }
    let levelling = planes.first().is_some_and(|p| p.unit == "m");
    Ok(Dataset {
        pages: vec![Page {
            field: Field::ScanningProbe,
            title: text("COMMENT"),
            width,
            height,
            planes,
            axis: Axis {
                kind: AxisKind::Channel,
                name: String::new(),
                linear: None,
            },
            x: lateral(0, width),
            y: lateral(1, height),
            sample: Some(sample),
            instrument: Some("Nanonis".into()),
            date,
            meta,
            levelling,
            ..Page::default()
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_info() {
        let rows = [
            "\tChannel\tName\tUnit\tDirection\tCalibration\tOffset".to_string(),
            "\t14\tZ\tm\tboth\t8.970E-9\t0.000E+0".to_string(),
            "\t0\tCurrent\tA\tforward\t1.000E-9\t0".to_string(),
            String::new(),
        ];
        let c = channels(&rows);
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[0],
            Channel {
                name: "Z".into(),
                unit: "m".into(),
                both: true
            }
        );
        assert!(!c[1].both);
    }

    #[test]
    fn small_scan() {
        let mut f = b":NANONIS_VERSION:\n2\n:SCANIT_TYPE:\n FLOAT MSBFIRST\n:SCAN_PIXELS:\n 2 2\n\
:SCAN_RANGE:\n 2E-9 2E-9\n:SCAN_OFFSET:\n 0 0\n:SCAN_DIR:\nup\n:DATA_INFO:\n\
\tChannel\tName\tUnit\tDirection\tCalibration\tOffset\n\t14\tZ\tm\tboth\t1\t0\n\n:SCANIT_END:\n\n\x1a\x04"
            .to_vec();
        for v in [1f32, 2., 3., 4., 5., 6., 7., 8.] {
            f.extend_from_slice(&v.to_be_bytes());
        }
        let d = read(&f).unwrap();
        let p = &d.pages[0];
        assert_eq!(p.planes.len(), 2);
        assert_eq!(p.read_plane(&f, 0).unwrap().0, [3., 4., 1., 2.]);
        assert_eq!(p.read_plane(&f, 1).unwrap().0, [8., 7., 6., 5.]);
        let x = p.x.as_ref().unwrap();
        assert_eq!((x.offset, x.step), (-0.5e-9, 1e-9));
        let y = p.y.as_ref().unwrap();
        assert_eq!((y.offset, y.step), (-0.5e-9, 1e-9));
        assert!(p.levelling);
        assert!(read(&f[..f.len() - 8]).is_ok());
        assert_eq!(read(b"junk").unwrap_err(), Error::NotSxm);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A 16384² scan whose many two-way channels have no data: each backward plane would
    /// have been a 1 GiB buffer.
    #[test]
    fn two_way_channels_of_a_huge_scan_without_data() {
        let mut f = b":NANONIS_VERSION:\n2\n:SCANIT_TYPE:\n FLOAT MSBFIRST\n:SCAN_PIXELS:\n 16384 16384\n:DATA_INFO:\n\
\tChannel\tName\tUnit\tDirection\tCalibration\tOffset\n"
            .to_vec();
        for k in 0..64 {
            f.extend_from_slice(format!("\t{k}\tZ{k}\tm\tboth\t1\t0\n").as_bytes());
        }
        f.extend_from_slice(b"\n:SCANIT_END:\n\n\x1a\x04");
        let d = read(&f).unwrap();
        let p = &d.pages[0];
        assert_eq!(p.planes.len(), 128);
        assert!(matches!(&p.planes[1].data, PlaneData::Values(v) if v.is_empty()));
    }
}
