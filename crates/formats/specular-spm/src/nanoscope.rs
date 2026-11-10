//! Bruker / Veeco Nanoscope files (version 5 and later: `.spm`, `.001`, `.002`…).
//!
//! The file opens with a Latin-1 text header of `\Key: value` lines grouped into `\*Section`
//! lists, ending at `\*File list end`. Each `\*Ciao image list` describes one channel: where its
//! samples are (`Data offset`; little-endian 16- or 32-bit integers as `Data length` says,
//! the first line stored at the bottom), its size (`Samps/line`, `Number of lines`), the scan
//! size and the value scale `\@2:Z scale: V [Sens. Zsens] (… V/LSB) 24.576 V`: the hard value
//! divided by 256^`Bytes/pixel` gives volts per count, and the soft scale named in brackets
//! (`\@Sens. Zsens: V 6.13 nm/V`, looked up anywhere in the header) turns volts into the
//! channel's unit. Force-curve sections are skipped.

use specular_em::dataset::{Axis, AxisKind, Dataset, Field, Page, Plane, PlaneData};
use specular_em::samples::{Order, Sample};

use crate::units::{centres, latin1, si};
use crate::{Error, Result};

/// The end of the header.
const END: &[u8] = b"\\*File list end";

/// The most samples in a channel (1 GiB of `f32`), as for the other formats here.
const MAX_PLANE: usize = 1 << 28;

/// Whether `head` starts a Nanoscope image file.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"\\*File list") || head.starts_with(b"\\*EC File list")
}

/// One `\*Section` of the header.
#[derive(Debug, Default)]
struct Section {
    name: String,
    fields: Vec<(String, String)>,
}

impl Section {
    fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn number(&self, key: &str) -> Option<f64> {
        self.get(key)?.split_whitespace().next()?.parse().ok()
    }
}

fn sections(text: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for line in text.split(['\r', '\n']) {
        let line = line.trim();
        let Some(l) = line.strip_prefix('\\') else {
            continue;
        };
        if let Some(name) = l.strip_prefix('*') {
            out.push(Section {
                name: name.to_string(),
                fields: Vec::new(),
            });
            continue;
        }
        let (k, v) = match l.split_once(": ") {
            Some((k, v)) => (k, v.trim()),
            None => match l.strip_suffix(':') {
                Some(k) => (k, ""),
                None => continue,
            },
        };
        if let Some(s) = out.last_mut() {
            s.fields.push((k.to_string(), v.to_string()));
        }
    }
    out
}

/// A parsed `Z scale` value: `V [soft scale] (per-count V/LSB) hard value V`.
#[derive(Debug, Default, PartialEq)]
struct ZScale {
    soft: Option<String>,
    per_lsb: Option<f64>,
    hard: Option<f64>,
    unit: String,
}

fn z_scale(v: &str) -> ZScale {
    let mut z = ZScale::default();
    let mut rest = v.trim();
    // The value type letter (V, C, S).
    if let Some((t, r)) = rest.split_once(char::is_whitespace)
        && t.len() == 1
    {
        rest = r.trim_start();
    }
    if let Some(r) = rest.strip_prefix('[')
        && let Some((soft, r)) = r.split_once(']')
    {
        z.soft = Some(soft.trim().to_string());
        rest = r.trim_start();
    }
    // The per-count value, whose unit may hold parentheses: `(7.45e-9 log(Arb)/LSB)`.
    if let Some(r) = rest.strip_prefix('(') {
        let mut depth = 1;
        let close = r.char_indices().find(|&(_, c)| {
            depth += match c {
                '(' => 1,
                ')' => -1,
                _ => 0,
            };
            depth == 0
        });
        if let Some((i, _)) = close {
            z.per_lsb = r[..i]
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok());
            rest = r[i + 1..].trim_start();
        }
    }
    let mut t = rest.split_whitespace();
    z.hard = t.next().and_then(|s| s.parse().ok());
    z.unit = t.collect::<Vec<_>>().join(" ");
    z
}

/// The soft scale `@name` anywhere in the header: (value, unit per volt).
fn soft_scale(secs: &[Section], name: &str) -> Option<(f64, String)> {
    let key = format!("@{name}");
    let v = secs.iter().find_map(|s| s.get(&key))?;
    let mut t = v.split_whitespace();
    t.next()?;
    let value: f64 = t.next()?.parse().ok()?;
    let unit = t.collect::<Vec<_>>().join(" ");
    Some((value, unit))
}

/// The channel's name from `S [ZSensor] "Height Sensor"`.
fn channel_name(v: &str) -> String {
    if let Some((_, r)) = v.split_once('"')
        && let Some((name, _)) = r.split_once('"')
    {
        return name.to_string();
    }
    if let Some((_, r)) = v.split_once('[')
        && let Some((name, _)) = r.split_once(']')
    {
        return name.to_string();
    }
    v.trim().to_string()
}

/// Scan size: (x, y, unit) from `378.9 378.9 nm` or `15000 nm` (then square pixels).
fn scan_size(v: &str, samps: usize, lines: usize) -> Option<(f64, f64, String)> {
    let t: Vec<&str> = v.split_whitespace().collect();
    let nums: Vec<f64> = t.iter().map_while(|s| s.parse().ok()).collect();
    let unit = t.get(nums.len()).copied().unwrap_or("").to_string();
    match nums[..] {
        [x, y, ..] => Some((x, y, unit)),
        [x] if samps > 0 => Some((x, x * lines as f64 / samps as f64, unit)),
        _ => None,
    }
}

/// One channel's values: where, how many bytes per count and the scale to SI.
struct Channel {
    width: usize,
    height: usize,
    plane: Plane,
    x: Option<(f64, f64, String)>,
}

fn channel(secs: &[Section], s: &Section) -> Option<Channel> {
    let width = s.number("Samps/line")? as usize;
    let height = s.number("Number of lines")? as usize;
    let offset = s.number("Data offset")? as usize;
    let n = width
        .checked_mul(height)
        .filter(|&n| n > 0 && n <= MAX_PLANE)?;
    // Newer files store 32-bit samples while `Bytes/pixel` still says 2: the stored size
    // follows `Data length`, the value scale follows `Bytes/pixel`.
    let bpp = s.number("Bytes/pixel").map(|b| b as usize);
    let stored = match s.number("Data length").map(|l| l as usize / n) {
        Some(b @ (2 | 4)) => b,
        _ => bpp?,
    };
    let bpp = bpp.unwrap_or(stored);
    let sample = match stored {
        2 => Sample::I16,
        4 => Sample::I32,
        _ => return None,
    };
    let (key, data) = s
        .fields
        .iter()
        .find(|(k, _)| k.to_ascii_lowercase().ends_with("image data"))?;
    let prefix = &key[..key.len() - "image data".len()];
    let mut label = channel_name(data);
    if s.get("Line Direction")
        .is_some_and(|d| d.eq_ignore_ascii_case("retrace"))
    {
        label.push_str(" (retrace)");
    }
    let z = s
        .get(&format!("{prefix}Z scale"))
        .map(z_scale)
        .unwrap_or_default();
    let full = 256f64.powi(bpp as i32);
    let volts = match (z.hard, z.per_lsb) {
        (Some(h), _) => h / full,
        (None, Some(p)) => p,
        (None, None) => 1.0,
    };
    let hard_unit = if z.unit.is_empty() {
        "V".to_string()
    } else {
        z.unit.clone()
    };
    // A soft scale without a unit (`V 1.000000`) leaves the hard value's unit.
    let (scale, unit) = match z.soft.as_deref().and_then(|n| soft_scale(secs, n)) {
        Some((sens, u)) => match u.split('/').next().map(str::trim) {
            Some(num) if !num.is_empty() => (volts * sens, num.to_string()),
            _ => (volts * sens, hard_unit),
        },
        None => (volts, hard_unit),
    };
    let (unit, f) = si(&unit);
    let size = s
        .get("Scan Size")
        .and_then(|v| scan_size(v, width, height))
        .or_else(|| {
            let scan = secs
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case("Ciao scan list"))?;
            scan_size(scan.get("Scan Size")?, width, height)
        });
    Some(Channel {
        width,
        height,
        plane: Plane {
            label,
            unit,
            data: PlaneData::Stored {
                offset,
                sample,
                order: Order::Little,
                bottom_up: true,
                scale: scale * f,
                add: 0.0,
            },
        },
        x: size,
    })
}

/// Reads a Nanoscope file's header: one page per channel size.
pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotNanoscope);
    }
    let end = file
        .windows(END.len())
        .position(|w| w == END)
        .ok_or(Error::Truncated("header"))?;
    let secs = sections(&latin1(&file[..end + END.len()]));
    let first = |name: &str| secs.iter().find(|s| s.name.eq_ignore_ascii_case(name));
    let files = first("File list");
    let scan = first("Ciao scan list");
    let mut meta = Vec::new();
    for s in [files, first("Equipment list"), scan].into_iter().flatten() {
        for (k, v) in &s.fields {
            if !k.starts_with('@') && !v.is_empty() && meta.len() < 400 {
                meta.push((k.clone(), v.clone()));
            }
        }
    }
    let date = files.and_then(|s| s.get("Date")).map(str::to_string);
    let instrument = secs
        .iter()
        .find_map(|s| s.get("Microscope").or_else(|| s.get("Description")))
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let mode = scan
        .and_then(|s| s.get("Operating mode"))
        .map(str::to_string);
    let mut pages: Vec<Page> = Vec::new();
    for s in secs
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case("Ciao image list"))
    {
        let Some(c) = channel(&secs, s) else {
            continue;
        };
        let page = match pages
            .iter_mut()
            .position(|p| p.width == c.width && p.height == c.height)
        {
            Some(i) => &mut pages[i],
            None => {
                let (x, y) = match &c.x {
                    Some((sx, sy, u)) => {
                        let (u, f) = si(u);
                        (
                            centres(0.0, sx * f, c.width, &u),
                            centres(0.0, sy * f, c.height, &u),
                        )
                    }
                    None => (None, None),
                };
                pages.push(Page {
                    field: Field::ScanningProbe,
                    width: c.width,
                    height: c.height,
                    axis: Axis {
                        kind: AxisKind::Channel,
                        name: String::new(),
                        linear: None,
                    },
                    x,
                    y,
                    instrument: instrument.clone(),
                    date: date.clone(),
                    mode: mode.clone(),
                    meta: meta.clone(),
                    ..Page::default()
                });
                pages.last_mut().unwrap()
            }
        };
        if page.planes.is_empty() {
            page.levelling = c.plane.unit == "m";
            if let PlaneData::Stored { sample, .. } = c.plane.data {
                page.sample = Some(sample);
            }
        }
        page.planes.push(c.plane);
    }
    if pages.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(Dataset { pages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales() {
        let z = z_scale("V [Sens. ZsensSens] (0.00000000572205 V/LSB) 24.57600 V");
        assert_eq!(z.soft.as_deref(), Some("Sens. ZsensSens"));
        assert_eq!(z.per_lsb, Some(0.00000000572205));
        assert_eq!(z.hard, Some(24.576));
        assert_eq!(z.unit, "V");
        let z = z_scale("V (0.0006713867 V/LSB) 2.000000 V");
        assert_eq!((z.soft, z.hard), (None, Some(2.0)));
        let z =
            z_scale("V [Sens. LogStiffnessSens] (0.00000000745058 log(Arb)/LSB) 32.00000 log(Arb)");
        assert_eq!((z.hard, z.unit.as_str()), (Some(32.0), "log(Arb)"));
        assert_eq!(
            channel_name("S [ZSensor] \"Height Sensor\""),
            "Height Sensor"
        );
        assert_eq!(channel_name("S [Height]"), "Height");
        assert_eq!(
            scan_size("15 15 ~m", 4, 2),
            Some((15.0, 15.0, "~m".to_string()))
        );
        assert_eq!(
            scan_size("10 nm", 4, 2),
            Some((10.0, 5.0, "nm".to_string()))
        );
    }

    #[test]
    fn refuses_garbage() {
        assert_eq!(read(b"hello").unwrap_err(), Error::NotNanoscope);
        assert!(read(b"\\*File list\r\n\\Version: 1\r\n").is_err());
        assert_eq!(
            read(b"\\*File list\r\n\\*File list end\r\n").unwrap_err(),
            Error::NoImage
        );
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A channel of 2^20 × 2^20 samples: each plane read would have been a 4 TiB buffer.
    #[test]
    fn channel_of_a_huge_grid() {
        let header = |side: u64| {
            format!(
                "\\*File list\r\n\\*Ciao image list\r\n\\Data offset: 4096\r\n\\Data length: {}\r\n\
\\Bytes/pixel: 2\r\n\\Samps/line: {side}\r\n\\Number of lines: {side}\r\n\
\\@2:Image Data: S [Height] \"Height\"\r\n\\*File list end\r\n",
                side * side * 2
            )
        };
        assert!(read(header(4).as_bytes()).is_ok());
        assert_eq!(
            read(header(1 << 20).as_bytes()).unwrap_err(),
            Error::NoImage
        );
    }
}
