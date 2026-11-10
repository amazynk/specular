//! Real World Value Mapping (PS3.3 C.7.6.16.2.11): stored values to quantities in physical
//! units (ADC in mm²/s, T1 in ms, SUV in g/ml), for parametric maps and quantitative images.
//! Each mapping covers a range of stored values with a linear function or a LUT, labelled
//! and with a unit from the Measurement Units Code Sequence.

use crate::parse::{DataSet, Values};

fn text_of(ds: &DataSet, v: &Values, tag: u32) -> Option<String> {
    ds.get(tag)
        .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
        .map(|s| s.trim_end_matches('\0').trim().to_owned())
        .filter(|s| !s.is_empty())
}

const RWVM_SEQUENCE: u32 = 0x0040_9096;
const FIRST_MAPPED: u32 = 0x0040_9216;
const LAST_MAPPED: u32 = 0x0040_9211;
const DOUBLE_LAST_MAPPED: u32 = 0x0040_9213;
const DOUBLE_FIRST_MAPPED: u32 = 0x0040_9214;
const LUT_DATA: u32 = 0x0040_9212;
const LUT_LABEL: u32 = 0x0040_9210;
const LUT_EXPLANATION: u32 = 0x0028_3003;
const INTERCEPT: u32 = 0x0040_9224;
const SLOPE: u32 = 0x0040_9225;
const UNITS_CODE: u32 = 0x0040_08EA;
const QUANTITY_DEFINITION: u32 = 0x0040_9220;
/// Largest LUT kept (entries).
const MAX_LUT: usize = 1 << 20;

/// How a mapping turns a stored value into a real-world value.
#[derive(Clone, Debug, PartialEq)]
pub enum RealWorldFunction {
    Linear {
        slope: f64,
        intercept: f64,
    },
    /// `lut[stored - first]`.
    Lut(Vec<f64>),
}

/// One item of the Real World Value Mapping Sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct RealWorld {
    /// Stored values covered, inclusive.
    pub first: f64,
    pub last: f64,
    pub function: RealWorldFunction,
    /// LUT Label, else LUT Explanation, else the quantity's code meaning.
    pub label: Option<String>,
    /// The unit as shown: the UCUM code ("mm2/s", "ms"), else the code meaning; `None` for
    /// UCUM "1" (no unit).
    pub unit: Option<String>,
    /// Code meaning of the unit ("millimeter squared per second").
    pub unit_meaning: Option<String>,
}

impl RealWorld {
    /// Whether `stored` lies in this mapping's range.
    pub fn covers(&self, stored: f64) -> bool {
        stored >= self.first && stored <= self.last
    }

    /// The real-world value of `stored` (which should be [`RealWorld::covers`]ed).
    pub fn apply(&self, stored: f64) -> f64 {
        match &self.function {
            RealWorldFunction::Linear { slope, intercept } => stored * slope + intercept,
            RealWorldFunction::Lut(l) => {
                let i = (stored.round() - self.first).clamp(0.0, (l.len().max(1) - 1) as f64);
                l.get(i as usize).copied().unwrap_or(f64::NAN)
            }
        }
    }
}

/// A real-world value with its label and unit.
#[derive(Clone, Debug, PartialEq)]
pub struct RealWorldValue {
    pub value: f64,
    pub label: Option<String>,
    pub unit: Option<String>,
}

/// The mappings of `frame`: the per-frame functional group's, else the shared one's, else
/// the image's own sequence. `signed`: Pixel Representation, which decides how US/SS range
/// values read.
pub(crate) fn read(ds: &DataSet, v: &Values, frame: u32, signed: bool) -> Vec<RealWorld> {
    let per = ds
        .get(0x5200_9230)
        .and_then(|e| e.items.get(frame as usize))
        .and_then(|i| i.get(RWVM_SEQUENCE));
    let shared = ds.item(0x5200_9229).and_then(|i| i.get(RWVM_SEQUENCE));
    let Some(seq) = per.or(shared).or_else(|| ds.get(RWVM_SEQUENCE)) else {
        return Vec::new();
    };
    seq.items
        .iter()
        .take(64)
        .filter_map(|item| item_of(item, v, signed))
        .collect()
}

fn item_of(item: &DataSet, v: &Values, signed: bool) -> Option<RealWorld> {
    let int = |tag| {
        item.get(tag).and_then(|e| v.int(e)).map(|x| {
            // US or SS by Pixel Representation; an implicit VR or US-read value of a signed
            // image is two's complement.
            if signed && (0x8000..=0xFFFF).contains(&x) {
                x - 0x1_0000
            } else {
                x
            }
        })
    };
    let num = |tag| item.get(tag).and_then(|e| v.number(e, 0));
    let (first, last) = match (int(FIRST_MAPPED), int(LAST_MAPPED)) {
        (Some(a), Some(b)) => (a as f64, b as f64),
        _ => (num(DOUBLE_FIRST_MAPPED)?, num(DOUBLE_LAST_MAPPED)?),
    };
    let (first, last) = (first.min(last), first.max(last));
    let function = match (num(SLOPE), item.get(LUT_DATA)) {
        (Some(slope), _) => RealWorldFunction::Linear {
            slope,
            intercept: num(INTERCEPT).unwrap_or(0.0),
        },
        (None, Some(e)) => {
            let b = v.bytes(e);
            let lut: Vec<f64> = b
                .chunks_exact(8)
                .take(MAX_LUT)
                .map(|c| {
                    let a: [u8; 8] = c.try_into().unwrap_or([0; 8]);
                    if v.big {
                        f64::from_be_bytes(a)
                    } else {
                        f64::from_le_bytes(a)
                    }
                })
                .collect();
            if lut.is_empty() {
                return None;
            }
            RealWorldFunction::Lut(lut)
        }
        (None, None) => return None,
    };
    let text = |d: &DataSet, tag| text_of(d, v, tag);
    let units = item.item(UNITS_CODE);
    let scheme = units.and_then(|u| text(u, 0x0008_0102));
    let code = units.and_then(|u| text(u, 0x0008_0100));
    let unit_meaning = units.and_then(|u| text(u, 0x0008_0104));
    let unit = match (scheme.as_deref(), code) {
        (Some("UCUM"), Some(c)) if c == "1" => None,
        (Some("UCUM"), Some(c)) => Some(c),
        (_, c) => unit_meaning.clone().or(c),
    };
    // The quantity (a concept name / value pair whose value names it, as "ADC").
    let quantity = item.get(QUANTITY_DEFINITION).and_then(|q| {
        q.items.iter().find_map(|i| {
            i.item(0x0040_A168)
                .and_then(|c| text(c, 0x0008_0104))
                .filter(|_| text(i, 0x0040_A040).as_deref() == Some("CODE"))
        })
    });
    let label = text(item, LUT_LABEL)
        .or_else(|| text(item, LUT_EXPLANATION))
        .or(quantity);
    Some(RealWorld {
        first,
        last,
        function,
        label,
        unit,
        unit_meaning,
    })
}

/// The value of the first mapping that covers `stored`.
pub(crate) fn value(maps: &[RealWorld], stored: f64) -> Option<RealWorldValue> {
    let m = maps.iter().find(|m| m.covers(stored))?;
    Some(RealWorldValue {
        value: m.apply(stored),
        label: m.label.clone(),
        unit: m.unit.clone(),
    })
}
