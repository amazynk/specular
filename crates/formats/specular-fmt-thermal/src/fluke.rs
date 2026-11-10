//! Fluke `.is2` (the Ti and TiS cameras since about 2012): a ZIP of protocol-buffer
//! records, the counts and JPEG photos.
//!
//! Members read (layout learnt from the MIT-licensed `fluke_thermal_reader`, from which
//! only the member names and field meanings are taken; the protocol buffers are read by
//! their wire format, not at fixed offsets):
//!
//! * `CameraInfo.gpbenc`: maker (field 22), serial (23) and engine serial (24).
//! * `CalibrationData.gpbenc`: calibration tables (field 3), each with its range (fields 3
//!   and 4, °C) and curves (field 9: from and to temperature, then c, b and a of
//!   `counts = a·T² + b·T + c`, T in °C).
//! * `Images/Main/IRImageInfo.gpbenc`: emissivity (field 10), background temperature (11),
//!   transmission (12); fields 1 and 2 name the calibration table, as fields 1 and 2 of
//!   each table do.
//! * `Images/Main/IR.data`: 16-bit counts, one header row then the image, little-endian.
//! * `ImageProperties.json` (UTF-16, when present): sensor size, model, lens, date, title.
//! * `Images/Main/<w><h>.jpg`: the visible photos (the largest is shown).
//!
//! Counts become temperatures in signal space: the object's signal is the measured counts
//! less the reflected background's share, `(1 − ε)` of the curve's counts at the background
//! temperature, divided by `ε · τ`; the curve then inverts it. Results are held to the
//! table's range, as Fluke's software shows them (checked against SmartView exports).
//!
//! The older IS2 layout (not a ZIP) and IS2 files without `IR.data` have no published
//! description and are refused with that reason.

use specular_ora::zip::{Entry, Zip};

use crate::{
    Camera, Conditions, Error, Kind, Mapping, Pixels, Result, Thermal, Visible, jpeg_size,
    malformed,
};

const F: &str = "Fluke IS2";

/// Largest member read (the counts of a 1280 × 960 image are 2.4 MB).
const MAX_MEMBER: u64 = 64 << 20;
/// Curves per calibration table kept.
pub const MAX_CURVES: usize = 16;

/// The reason the older IS2 layout is refused.
pub const OLD_IS2_REASON: &str =
    "older Fluke IS2 layout (not a ZIP): no published description of its temperatures";
/// The reason an IS2 without `IR.data` is refused.
pub const NO_IR_DATA_REASON: &str =
    "this newer Fluke IS2 layout (no IR.data) has no published description";

/// Whether `d` starts like an older Fluke `.is2` (before the ZIP layout). Only meaningful
/// together with the `.is2` extension.
pub fn is_old_is2(d: &[u8]) -> bool {
    d.starts_with(&[0, 1, 0, 1, 0, 0, 0, 0])
}

/// A ZIP whose directory (at its end) names `CameraInfo.gpbenc`.
pub(crate) fn is_is2(d: &[u8]) -> bool {
    const NAME: &[u8] = b"CameraInfo.gpbenc";
    if !d.starts_with(b"PK\x03\x04") {
        return false;
    }
    let tail = &d[d.len().saturating_sub(64 << 10)..];
    tail.windows(NAME.len()).any(|w| w == NAME)
}

/// One curve, `counts = a·T² + b·T + c` for T (°C) from `t0` to `t1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Curve {
    pub t0: f64,
    pub t1: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

impl Curve {
    fn counts(&self, t: f64) -> f64 {
        (self.a * t + self.b) * t + self.c
    }

    /// The temperature whose counts are `s` (the rising root).
    fn temperature(&self, s: f64) -> f64 {
        if self.a.abs() < 1e-12 {
            return (s - self.c) / self.b;
        }
        let disc = self.b * self.b - 4.0 * self.a * (self.c - s);
        if disc < 0.0 {
            return f64::NAN;
        }
        (-self.b + disc.sqrt()) / (2.0 * self.a)
    }
}

/// A Fluke calibration table: its curves and the range results are held to.
#[derive(Clone, Debug, PartialEq)]
pub struct FlukeCalibration {
    curves: Vec<Curve>,
    /// The table's range, °C.
    pub range: (f64, f64),
}

impl FlukeCalibration {
    pub fn curves(&self) -> &[Curve] {
        &self.curves
    }

    /// Counts at temperature `t` (°C), from the curve covering it (the end curves beyond).
    pub fn counts(&self, t: f64) -> f64 {
        let c = self.curves();
        let k = c
            .iter()
            .position(|k| t >= k.t0 && t < k.t1)
            .unwrap_or(if t < c[0].t0 { 0 } else { c.len() - 1 });
        c[k].counts(t)
    }

    /// The temperature (°C) of signal `s`, from the curve whose counts cover it (the last
    /// such curve, the end curves beyond).
    pub fn temperature(&self, s: f64) -> f64 {
        let c = self.curves();
        let k = c
            .iter()
            .rposition(|k| s >= k.counts(k.t0) && s < k.counts(k.t1))
            .unwrap_or(if s < c[0].counts(c[0].t0) {
                0
            } else {
                c.len() - 1
            });
        c[k].temperature(s)
    }

    /// The object temperature (°C) of `count` under `cond`: emissivity ε (1 if unset),
    /// reflected background (20 °C if unset) and transmission τ (`window_transmission`, 1
    /// if unset); held to the table's range. NaN where the curve has no root.
    pub fn celsius(&self, count: f64, cond: &Conditions) -> f64 {
        let eps = cond.emissivity.unwrap_or(1.0).clamp(1e-3, 1.0);
        let tau = cond.window_transmission.unwrap_or(1.0).clamp(1e-3, 1.0);
        let bg = cond.reflected.unwrap_or(20.0);
        let s = (count - (1.0 - eps) * self.counts(bg)) / (eps * tau);
        let t = self.temperature(s);
        if t.is_nan() {
            t
        } else {
            t.clamp(self.range.0, self.range.1)
        }
    }
}

/// A protocol-buffer field value.
#[derive(Clone, Copy, Debug)]
enum Val<'a> {
    Int(u64),
    F32(f32),
    Bytes(&'a [u8]),
}

fn varint(b: &[u8], p: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let c = *b.get(*p)?;
        *p += 1;
        v |= u64::from(c & 0x7f) << shift;
        if c < 0x80 {
            return Some(v);
        }
    }
    None
}

/// The fields of a message in order; `None` if it is not a well-formed message.
fn fields(b: &[u8]) -> Option<Vec<(u64, Val<'_>)>> {
    let mut out = Vec::new();
    let mut p = 0;
    while p < b.len() {
        let tag = varint(b, &mut p)?;
        let v = match tag & 7 {
            0 => Val::Int(varint(b, &mut p)?),
            1 => {
                p = p.checked_add(8).filter(|&e| e <= b.len())?;
                continue;
            }
            2 => {
                let n = usize::try_from(varint(b, &mut p)?).ok()?;
                let end = p.checked_add(n).filter(|&e| e <= b.len())?;
                let s = &b[p..end];
                p = end;
                Val::Bytes(s)
            }
            5 => {
                let a: [u8; 4] = b.get(p..p + 4)?.try_into().ok()?;
                p += 4;
                Val::F32(f32::from_le_bytes(a))
            }
            _ => return None,
        };
        out.push((tag >> 3, v));
    }
    Some(out)
}

fn int(f: &[(u64, Val<'_>)], n: u64) -> Option<u64> {
    f.iter().find_map(|(k, v)| match v {
        Val::Int(i) if *k == n => Some(*i),
        _ => None,
    })
}

fn float(f: &[(u64, Val<'_>)], n: u64) -> Option<f64> {
    f.iter().find_map(|(k, v)| match v {
        Val::F32(x) if *k == n && x.is_finite() => Some(f64::from(*x)),
        _ => None,
    })
}

fn text(f: &[(u64, Val<'_>)], n: u64) -> String {
    f.iter()
        .find_map(|(k, v)| match v {
            Val::Bytes(s) if *k == n => Some(String::from_utf8_lossy(s).trim().to_owned()),
            _ => None,
        })
        .unwrap_or_default()
}

/// A calibration table and its key (fields 1 and 2).
fn table(msg: &[u8]) -> Option<((u64, u64), FlukeCalibration)> {
    let f = fields(msg)?;
    let mut curves = Vec::new();
    for (k, v) in &f {
        if let (9, Val::Bytes(b)) = (k, v) {
            let c = fields(b)?;
            let curve = Curve {
                t0: float(&c, 1)?,
                t1: float(&c, 2)?,
                c: float(&c, 3)?,
                b: float(&c, 4)?,
                a: float(&c, 5)?,
            };
            // Unset curves are written as −1s.
            if curve.t0 < curve.t1
                && curves.len() < MAX_CURVES
                && !(curve.a == -1.0 && curve.b == -1.0)
            {
                curves.push(curve);
            }
        }
    }
    let range = float(&f, 3).zip(float(&f, 4)).filter(|(lo, hi)| lo < hi)?;
    (!curves.is_empty()).then(|| {
        (
            (int(&f, 1).unwrap_or(0), int(&f, 2).unwrap_or(0)),
            FlukeCalibration { curves, range },
        )
    })
}

/// `"KEY":"value"` from ImageProperties.json, unescaped and without inner quotes.
fn json_str(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let at = json.find(&pat)? + pat.len();
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '"' => break,
            c => out.push(c),
        }
    }
    let v = out.trim().trim_matches('"').trim().to_owned();
    (!v.is_empty()).then_some(v)
}

/// UTF-16 (with its byte-order mark) or UTF-8 text.
fn decode_text(b: &[u8]) -> String {
    let utf16 = |be: bool| {
        let units: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|p| {
                if be {
                    u16::from_be_bytes([p[0], p[1]])
                } else {
                    u16::from_le_bytes([p[0], p[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match b {
        [0xff, 0xfe, ..] => utf16(false),
        [0xfe, 0xff, ..] => utf16(true),
        _ => String::from_utf8_lossy(b).into_owned(),
    }
}

struct Archive<'d> {
    data: &'d [u8],
    zip: Zip,
}

impl<'d> Archive<'d> {
    fn entry(&self, name: &str) -> Option<&Entry> {
        self.zip
            .entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
    }
    fn read(&self, name: &str) -> Result<Option<std::borrow::Cow<'d, [u8]>>> {
        let Some(e) = self.entry(name) else {
            return Ok(None);
        };
        self.zip
            .read(self.data, e, MAX_MEMBER)
            .map(Some)
            .map_err(|err| malformed(F, format!("{}: {err}", e.name)))
    }
    fn need(&self, name: &str) -> Result<std::borrow::Cow<'d, [u8]>> {
        self.read(name)?
            .ok_or_else(|| malformed(F, format!("no {name}")))
    }
}

/// Width and height of an `IR.data` of `units` 16-bit values: a header row of `w` values
/// and `h` rows. From ImageProperties.json, else IRImageInfo's fields 4 and 5 (row bytes
/// and twice the rows with the header), each only if it fits exactly.
fn size(
    units: usize,
    json: Option<(usize, usize)>,
    info: Option<(u64, u64)>,
) -> Option<(usize, usize)> {
    let fits = |(w, h): (usize, usize)| {
        w > 0 && h > 0 && h.checked_add(1).and_then(|r| w.checked_mul(r)) == Some(units)
    };
    let from_info = info.and_then(|(rb, r2)| {
        let w = usize::try_from(rb / 2).ok()?;
        let h = usize::try_from(r2 / 2).ok()?.checked_sub(1)?;
        Some((w, h))
    });
    json.filter(|&s| fits(s)).or(from_info.filter(|&s| fits(s)))
}

pub(crate) fn read(d: &[u8]) -> Result<Thermal> {
    let zip = Zip::parse(d).map_err(|e| malformed(F, e.to_string()))?;
    let a = Archive { data: d, zip };
    let Some(ir_data) = a.read("Images/Main/IR.data")? else {
        return Err(Error::Unsupported {
            format: F,
            what: NO_IR_DATA_REASON.into(),
        });
    };
    let camera_info = a.read("CameraInfo.gpbenc")?;
    let ci = camera_info.as_deref().and_then(fields).unwrap_or_default();
    let cal_bytes = a.need("CalibrationData.gpbenc")?;
    let cal = fields(&cal_bytes).ok_or_else(|| malformed(F, "calibration data"))?;
    let tables: Vec<((u64, u64), FlukeCalibration)> = cal
        .iter()
        .filter_map(|(k, v)| match (k, v) {
            (3, Val::Bytes(b)) => table(b),
            _ => None,
        })
        .collect();
    let info_bytes = a.need("Images/Main/IRImageInfo.gpbenc")?;
    let info = fields(&info_bytes).ok_or_else(|| malformed(F, "image info"))?;
    let key = (int(&info, 1).unwrap_or(0), int(&info, 2).unwrap_or(0));
    let calibration = match tables.as_slice() {
        [] => return Err(malformed(F, "no calibration curves")),
        [(_, t)] => t.clone(),
        many => many
            .iter()
            .find(|(k, _)| *k == key)
            .ok_or_else(|| Error::Unsupported {
                format: F,
                what: "several calibration ranges and none named by the image".into(),
            })?
            .1
            .clone(),
    };
    let json = a
        .read("ImageProperties.json")?
        .map(|b| decode_text(&b))
        .unwrap_or_default();
    let j = |k: &str| json_str(&json, k);
    let num = |k: &str| j(k).and_then(|v| v.parse::<usize>().ok());
    let units = ir_data.len() / 2;
    let json_size = num("IRPROP_IR_SENSOR_WIDTH").zip(num("IRPROP_IR_SENSOR_HEIGHT"));
    let info_size = int(&info, 4).zip(int(&info, 5));
    let (width, height) = size(units, json_size, info_size)
        .ok_or_else(|| malformed(F, "IR.data does not match the image size"))?;
    let values: Vec<u16> = ir_data[2 * width..2 * width * (height + 1)]
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();

    let mut visible: Option<Visible> = None;
    for e in &a.zip.entries {
        let name = e.name.to_ascii_lowercase();
        if !(name.starts_with("images/main/") && name.ends_with(".jpg")) {
            continue;
        }
        let Some(bytes) = a.read(&e.name)? else {
            continue;
        };
        let Some((w, h)) = jpeg_size(&bytes) else {
            continue;
        };
        if visible
            .as_ref()
            .is_none_or(|v| u64::from(w) * u64::from(h) > u64::from(v.width) * u64::from(v.height))
        {
            visible = Some(Visible {
                bytes: bytes.into_owned(),
                width: w,
                height: h,
            });
        }
    }

    let maker = text(&ci, 22);
    let model = j("IRPROP_THERMAL_IMAGER_MODEL").unwrap_or_default();
    let camera = Camera {
        model: match (maker.is_empty(), model.is_empty()) {
            (_, false) if !model.to_ascii_lowercase().contains("fluke") => {
                format!("Fluke {model}")
            }
            (_, false) => model,
            (false, true) => maker,
            (true, true) => "Fluke".to_owned(),
        },
        serial: j("IRPROP_THERMAL_IMAGER_SN").unwrap_or_else(|| text(&ci, 23)),
        part_number: String::new(),
        software: String::new(),
        lens: j("IRPROP_THERMAL_IMAGER_IR_LENSES").unwrap_or_default(),
        lens_serial: j("IRPROP_THERMAL_IMAGER_IR_LENSES_SN").unwrap_or_default(),
        range: Some(calibration.range),
        date: j("IRPROP_THERMAL_IMAGE_CAPTURE_DATE_TIME"),
        ..Camera::default()
    };
    Ok(Thermal {
        kind: Kind::FlukeIs2,
        width,
        height,
        pixels: Pixels::Counts {
            values,
            mapping: Mapping::Fluke(calibration),
        },
        conditions: Conditions {
            emissivity: float(&info, 10).filter(|e| *e > 0.0 && *e <= 1.0),
            reflected: float(&info, 11),
            window_transmission: float(&info, 12).filter(|t| *t > 0.0 && *t <= 1.0),
            ..Conditions::default()
        },
        camera,
        palette: None,
        visible,
        pip: None,
    })
}
