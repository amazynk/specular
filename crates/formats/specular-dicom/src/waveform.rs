//! Waveforms (PS3.3 C.10.9): the multiplex groups of a waveform object (12-lead and general
//! ECG, ambulatory ECG, haemodynamic, cardiac electrophysiology, arterial pulse, respiratory,
//! audio), each a set of channels sampled together, and the paper layout they are shown in.
//!
//! A channel's sample in its sensitivity units is `raw × Channel Sensitivity × Channel
//! Sensitivity Correction Factor + Channel Baseline` (missing factors are 1, a missing
//! baseline 0). Samples are 8 or 16 bit signed or unsigned, µ-law or A-law (expanded per
//! G.711), or 64 bit.
//!
//! The layout is in mm on a paper page: the standard 12-lead sheet is three rows of four
//! columns (I aVR V1 V4 / II aVL V2 V5 / III aVF V3 V6), each column a quarter of the record,
//! with a lead II rhythm strip under them; anything else is one row per channel over the full
//! width. A row is 25 mm high, 10 mm per mV by default; channels not in volts are scaled to
//! fill their row.

use crate::charset::Charset;
use crate::dict::Vr;
use crate::parse::{DataSet, Element};
use crate::sr::Code;
use crate::{Dicom, Error, Result, Values};

const SOP_CLASS: u32 = 0x0008_0016;
const CODE_VALUE: u32 = 0x0008_0100;
const CODING_SCHEME: u32 = 0x0008_0102;
const CODE_MEANING: u32 = 0x0008_0104;
const MULTIPLEX_TIME_OFFSET: u32 = 0x0018_1068;
const ORIGINALITY: u32 = 0x003A_0004;
const CHANNELS: u32 = 0x003A_0005;
const SAMPLES: u32 = 0x003A_0010;
const FREQUENCY: u32 = 0x003A_001A;
const GROUP_LABEL: u32 = 0x003A_0020;
const CHANNEL_DEFINITION: u32 = 0x003A_0200;
const CHANNEL_LABEL: u32 = 0x003A_0203;
const CHANNEL_STATUS: u32 = 0x003A_0205;
const CHANNEL_SOURCE: u32 = 0x003A_0208;
const SENSITIVITY: u32 = 0x003A_0210;
const SENSITIVITY_UNITS: u32 = 0x003A_0211;
const CORRECTION: u32 = 0x003A_0212;
const BASELINE: u32 = 0x003A_0213;
const TIME_SKEW: u32 = 0x003A_0214;
const BITS_STORED: u32 = 0x003A_021A;
const FILTER_LOW: u32 = 0x003A_0220;
const FILTER_HIGH: u32 = 0x003A_0221;
const NOTCH: u32 = 0x003A_0222;
const ANNOTATIONS: u32 = 0x0040_B020;
const CONCEPT_NAME: u32 = 0x0040_A043;
const MEASUREMENT_UNITS: u32 = 0x0040_08EA;
const NUMERIC_VALUE: u32 = 0x0040_A30A;
const UNFORMATTED_TEXT: u32 = 0x0070_0006;
const CONCEPT_CODE: u32 = 0x0040_A168;
const WAVEFORM_SEQUENCE: u32 = 0x5400_0100;
const BITS_ALLOCATED: u32 = 0x5400_1004;
const INTERPRETATION: u32 = 0x5400_1006;
const WAVEFORM_DATA: u32 = 0x5400_1010;

/// Most samples read per file (all groups and channels).
const MAX_SAMPLES: u64 = 1 << 28;

/// The waveform storage classes.
const CLASSES: &[(&str, Kind)] = &[
    ("1.2.840.10008.5.1.4.1.1.9.1.1", Kind::TwelveLead),
    ("1.2.840.10008.5.1.4.1.1.9.1.2", Kind::GeneralEcg),
    ("1.2.840.10008.5.1.4.1.1.9.1.3", Kind::AmbulatoryEcg),
    ("1.2.840.10008.5.1.4.1.1.9.1.4", Kind::GeneralEcg),
    ("1.2.840.10008.5.1.4.1.1.9.2.1", Kind::Hemodynamic),
    ("1.2.840.10008.5.1.4.1.1.9.3.1", Kind::CardiacEp),
    ("1.2.840.10008.5.1.4.1.1.9.4.1", Kind::Audio),
    ("1.2.840.10008.5.1.4.1.1.9.4.2", Kind::Audio),
    ("1.2.840.10008.5.1.4.1.1.9.5.1", Kind::ArterialPulse),
    ("1.2.840.10008.5.1.4.1.1.9.6.1", Kind::Respiratory),
    ("1.2.840.10008.5.1.4.1.1.9.6.2", Kind::Respiratory),
    ("1.2.840.10008.5.1.4.1.1.9.7.1", Kind::Multichannel),
    ("1.2.840.10008.5.1.4.1.1.9.7.2", Kind::Multichannel),
    ("1.2.840.10008.5.1.4.1.1.9.7.3", Kind::Multichannel),
    ("1.2.840.10008.5.1.4.1.1.9.7.4", Kind::Multichannel),
    ("1.2.840.10008.5.1.4.1.1.9.8.1", Kind::Multichannel),
];

/// What a waveform object records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    TwelveLead,
    GeneralEcg,
    AmbulatoryEcg,
    Hemodynamic,
    CardiacEp,
    Audio,
    ArterialPulse,
    Respiratory,
    Multichannel,
}

impl Kind {
    /// English name of the storage class (DICOM terms, not translated).
    pub fn name(self) -> &'static str {
        match self {
            Kind::TwelveLead => "12-lead ECG",
            Kind::GeneralEcg => "General ECG",
            Kind::AmbulatoryEcg => "Ambulatory ECG",
            Kind::Hemodynamic => "Hemodynamic waveform",
            Kind::CardiacEp => "Cardiac electrophysiology waveform",
            Kind::Audio => "Audio waveform",
            Kind::ArterialPulse => "Arterial pulse waveform",
            Kind::Respiratory => "Respiratory waveform",
            Kind::Multichannel => "Multichannel waveform",
        }
    }
}

/// The waveform kind of `sop_class`, if it is a waveform storage class.
pub fn kind_of(sop_class: &str) -> Option<Kind> {
    CLASSES
        .iter()
        .find(|(uid, _)| *uid == sop_class)
        .map(|&(_, k)| k)
}

/// One of the twelve standard ECG leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lead {
    I,
    II,
    III,
    AVR,
    AVL,
    AVF,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
}

impl Lead {
    pub const ALL: [Lead; 12] = [
        Lead::I,
        Lead::II,
        Lead::III,
        Lead::AVR,
        Lead::AVL,
        Lead::AVF,
        Lead::V1,
        Lead::V2,
        Lead::V3,
        Lead::V4,
        Lead::V5,
        Lead::V6,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Lead::I => "I",
            Lead::II => "II",
            Lead::III => "III",
            Lead::AVR => "aVR",
            Lead::AVL => "aVL",
            Lead::AVF => "aVF",
            Lead::V1 => "V1",
            Lead::V2 => "V2",
            Lead::V3 => "V3",
            Lead::V4 => "V4",
            Lead::V5 => "V5",
            Lead::V6 => "V6",
        }
    }

    /// The lead named by a channel source code (MDC 2:n, SCPECG 5.6.3-9-n) or a label
    /// ("Lead I", "II", "aVR", "Lead V1 (…)").
    pub fn parse(source: Option<&Code>, label: &str) -> Option<Lead> {
        if let Some(c) = source {
            let by_code = match (c.scheme.as_str(), c.value.as_str()) {
                ("MDC", v) => match v
                    .strip_prefix("2:")
                    .or_else(|| v.strip_prefix("MDC_ECG_LEAD_"))
                {
                    Some("1") | Some("I") => Some(Lead::I),
                    Some("2") | Some("II") => Some(Lead::II),
                    Some("61") | Some("III") => Some(Lead::III),
                    Some("62") | Some("AVR") => Some(Lead::AVR),
                    Some("63") | Some("AVL") => Some(Lead::AVL),
                    Some("64") | Some("AVF") => Some(Lead::AVF),
                    Some("3") | Some("V1") => Some(Lead::V1),
                    Some("4") | Some("V2") => Some(Lead::V2),
                    Some("5") | Some("V3") => Some(Lead::V3),
                    Some("6") | Some("V4") => Some(Lead::V4),
                    Some("7") | Some("V5") => Some(Lead::V5),
                    Some("8") | Some("V6") => Some(Lead::V6),
                    _ => None,
                },
                ("SCPECG", v) => match v.strip_prefix("5.6.3-9-") {
                    Some("1") => Some(Lead::I),
                    Some("2") => Some(Lead::II),
                    Some("61") => Some(Lead::III),
                    Some("62") => Some(Lead::AVR),
                    Some("63") => Some(Lead::AVL),
                    Some("64") => Some(Lead::AVF),
                    Some("3") => Some(Lead::V1),
                    Some("4") => Some(Lead::V2),
                    Some("5") => Some(Lead::V3),
                    Some("6") => Some(Lead::V4),
                    Some("7") => Some(Lead::V5),
                    Some("8") => Some(Lead::V6),
                    _ => None,
                },
                _ => None,
            };
            if by_code.is_some() {
                return by_code;
            }
            if let Some(l) = Lead::from_text(&c.meaning) {
                return Some(l);
            }
        }
        Lead::from_text(label)
    }

    fn from_text(s: &str) -> Option<Lead> {
        let s = s.trim();
        let s = s
            .strip_prefix("Lead ")
            .or_else(|| s.strip_prefix("lead "))
            .unwrap_or(s);
        let word = s.split([' ', '(', ',']).next().unwrap_or("");
        Lead::ALL
            .into_iter()
            .find(|l| l.label().eq_ignore_ascii_case(word))
    }
}

/// One channel of a multiplex group.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel {
    pub label: String,
    pub source: Option<Code>,
    /// Sensitivity units (UCUM "uV", "mV", "mm[Hg]"…).
    pub unit: Option<Code>,
    pub sensitivity: f64,
    pub correction: f64,
    pub baseline: f64,
    /// Time skew (s) of this channel's first sample.
    pub skew: f64,
    pub bits_stored: u16,
    pub filter_low: Option<f64>,
    pub filter_high: Option<f64>,
    pub notch: Option<f64>,
    pub status: String,
    /// Samples in the sensitivity units.
    pub samples: Vec<f32>,
}

impl Channel {
    /// The standard lead this channel records, if any.
    pub fn lead(&self) -> Option<Lead> {
        Lead::parse(self.source.as_ref(), &self.label)
    }

    /// The label to show: the channel label, else the source's meaning, else the lead.
    pub fn display_label(&self) -> String {
        if let Some(l) = self.lead() {
            return l.label().to_owned();
        }
        if !self.label.is_empty() {
            return self.label.clone();
        }
        self.source
            .as_ref()
            .map(|c| c.label().to_owned())
            .unwrap_or_default()
    }

    /// Millivolts per sensitivity unit, for channels in volts.
    pub fn millivolts(&self) -> Option<f64> {
        let u = self.unit.as_ref()?;
        match u.value.as_str() {
            "uV" | "µV" => Some(1e-3),
            "mV" => Some(1.0),
            "V" => Some(1e3),
            "nV" => Some(1e-6),
            _ => None,
        }
    }

    /// The unit symbol ("mV", "mm[Hg]"), empty when unknown.
    pub fn unit_symbol(&self) -> String {
        self.unit
            .as_ref()
            .map(Code::unit_symbol)
            .unwrap_or_default()
    }

    /// Smallest and largest sample over samples `range` (clamped), or None when empty.
    pub fn extent(&self, range: std::ops::Range<usize>) -> Option<(f32, f32)> {
        let end = range.end.min(self.samples.len());
        let s = self.samples.get(range.start.min(end)..end)?;
        let mut it = s.iter().copied().filter(|v| v.is_finite());
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v))))
    }
}

/// Channels sampled together.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub label: String,
    /// Samples per second.
    pub frequency: f64,
    pub samples: usize,
    /// Offset (s) of the first sample from the acquisition time.
    pub time_offset: f64,
    /// ORIGINAL or DERIVED.
    pub originality: String,
    /// The sample interpretation as written (SS, US, SB, UB, MB, AB, SV, UV).
    pub interpretation: String,
    pub bits_allocated: u16,
    pub channels: Vec<Channel>,
}

impl Group {
    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        if self.frequency > 0.0 {
            self.samples as f64 / self.frequency
        } else {
            0.0
        }
    }

    /// The channel of each standard lead, when the group has all twelve.
    pub fn twelve_leads(&self) -> Option<[usize; 12]> {
        let mut out = [usize::MAX; 12];
        for (i, c) in self.channels.iter().enumerate() {
            if let Some(l) = c.lead() {
                let k = Lead::ALL.iter().position(|&x| x == l)?;
                if out[k] == usize::MAX {
                    out[k] = i;
                }
            }
        }
        out.iter().all(|&i| i != usize::MAX).then_some(out)
    }
}

/// An annotation of the waveform (measurements and statements such as heart rate or QT).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotation {
    pub concept: Option<Code>,
    pub text: String,
}

/// The waveforms of a file.
#[derive(Clone, Debug, PartialEq)]
pub struct Waveforms {
    pub kind: Kind,
    pub groups: Vec<Group>,
    pub annotations: Vec<Annotation>,
}

/// Expands a G.711 µ-law byte to 14-bit linear.
pub fn mulaw(b: u8) -> i16 {
    let u = !b;
    let sign = u & 0x80;
    let exponent = (u >> 4) & 7;
    let mantissa = u & 0x0F;
    let magnitude = ((((mantissa as i32) << 3) + 0x84) << exponent) - 0x84;
    (if sign != 0 { -magnitude } else { magnitude }) as i16
}

/// Expands a G.711 A-law byte to 13-bit linear (scaled to 16).
pub fn alaw(b: u8) -> i16 {
    let a = b ^ 0x55;
    let sign = a & 0x80;
    let exponent = (a >> 4) & 7;
    let mantissa = (a & 0x0F) as i32;
    let magnitude = if exponent == 0 {
        (mantissa << 4) + 8
    } else {
        ((mantissa << 4) + 0x108) << (exponent - 1)
    };
    (if sign != 0 { magnitude } else { -magnitude }) as i16
}

impl Waveforms {
    /// Whether `d` has a Waveform Sequence.
    pub fn present(d: &Dicom) -> bool {
        d.dataset
            .get(WAVEFORM_SEQUENCE)
            .is_some_and(|e| !e.items.is_empty())
    }

    /// Reads the waveforms of `d` (parsed from `file`).
    pub fn read(d: &Dicom, file: &[u8]) -> Result<Waveforms> {
        let ds = &d.dataset;
        let v = d.values(file);
        let cs = Charset::of(ds, &v);
        let text = |ds: &DataSet, tag: u32, cs: &Charset| -> String {
            ds.get(tag)
                .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
                .unwrap_or_default()
        };
        let code = |ds: &DataSet, tag: u32, cs: &Charset| -> Option<Code> {
            let it = ds.item(tag)?;
            let cs = cs.within(it, &v);
            Some(Code {
                value: text(it, CODE_VALUE, &cs),
                scheme: text(it, CODING_SCHEME, &cs),
                meaning: text(it, CODE_MEANING, &cs),
            })
        };
        let num = |ds: &DataSet, tag: u32| ds.get(tag).and_then(|e| v.number(e, 0));
        let kind = kind_of(&text(ds, SOP_CLASS, &cs)).unwrap_or(Kind::Multichannel);
        let seq = ds
            .get(WAVEFORM_SEQUENCE)
            .filter(|e| !e.items.is_empty())
            .ok_or(Error::Malformed("no Waveform Sequence"))?;
        let mut groups = Vec::new();
        let mut total: u64 = 0;
        for it in &seq.items {
            let cs = cs.within(it, &v);
            let n_channels = it.get(CHANNELS).and_then(|e| v.int(e)).unwrap_or(0).max(0) as usize;
            let samples = it.get(SAMPLES).and_then(|e| v.int(e)).unwrap_or(0).max(0) as usize;
            let bits = it.get(BITS_ALLOCATED).and_then(|e| v.int(e)).unwrap_or(16) as u16;
            let interpretation = text(it, INTERPRETATION, &cs);
            let defs = it
                .get(CHANNEL_DEFINITION)
                .map(|e| e.items.as_slice())
                .unwrap_or_default();
            let n_channels = if n_channels == 0 {
                defs.len()
            } else {
                n_channels
            };
            // Both counts come from the file (any VR, up to i64::MAX); a channel costs a few
            // hundred bytes even without samples, and Number of Waveform Channels is a US.
            total = total.saturating_add((n_channels as u64).saturating_mul(samples as u64));
            if total > MAX_SAMPLES || n_channels > 0xFFFF {
                return Err(Error::TooLarge(
                    total
                        .max(n_channels as u64)
                        .saturating_mul(u64::from(bits / 8).max(1)),
                ));
            }
            let mut channels: Vec<Channel> = (0..n_channels)
                .map(|c| {
                    let Some(def) = defs.get(c) else {
                        return Channel {
                            sensitivity: 1.0,
                            correction: 1.0,
                            ..Channel::default()
                        };
                    };
                    Channel {
                        label: text(def, CHANNEL_LABEL, &cs),
                        source: code(def, CHANNEL_SOURCE, &cs),
                        unit: code(def, SENSITIVITY_UNITS, &cs),
                        sensitivity: num(def, SENSITIVITY).unwrap_or(1.0),
                        correction: num(def, CORRECTION).unwrap_or(1.0),
                        baseline: num(def, BASELINE).unwrap_or(0.0),
                        skew: num(def, TIME_SKEW).unwrap_or(0.0),
                        bits_stored: def
                            .get(BITS_STORED)
                            .and_then(|e| v.int(e))
                            .unwrap_or(bits as i64) as u16,
                        filter_low: num(def, FILTER_LOW),
                        filter_high: num(def, FILTER_HIGH),
                        notch: num(def, NOTCH),
                        status: text(def, CHANNEL_STATUS, &cs),
                        samples: Vec::with_capacity(samples),
                    }
                })
                .collect();
            if let Some(data) = it.get(WAVEFORM_DATA) {
                read_samples(&v, data, bits, &interpretation, samples, &mut channels)?;
            }
            groups.push(Group {
                label: text(it, GROUP_LABEL, &cs),
                frequency: num(it, FREQUENCY).unwrap_or(0.0),
                samples,
                time_offset: num(it, MULTIPLEX_TIME_OFFSET).unwrap_or(0.0) / 1000.0,
                originality: text(it, ORIGINALITY, &cs),
                interpretation,
                bits_allocated: bits,
                channels,
            });
        }
        let mut annotations = Vec::new();
        for it in ds
            .get(ANNOTATIONS)
            .map(|e| e.items.as_slice())
            .unwrap_or_default()
        {
            let cs = cs.within(it, &v);
            let concept = code(it, CONCEPT_NAME, &cs);
            let mut t = text(it, UNFORMATTED_TEXT, &cs);
            if t.is_empty()
                && let Some(c) = code(it, CONCEPT_CODE, &cs)
            {
                t = c.label().to_owned();
            }
            if t.is_empty()
                && let Some(n) = it.get(NUMERIC_VALUE)
            {
                t = (0..v.count(n))
                    .filter_map(|i| v.text_n(n, i))
                    .collect::<Vec<_>>()
                    .join(", ");
                let unit = code(it, MEASUREMENT_UNITS, &cs)
                    .map(|c| c.unit_symbol())
                    .unwrap_or_default();
                if !unit.is_empty() {
                    t = format!("{t} {unit}");
                }
            }
            if concept.is_some() || !t.is_empty() {
                annotations.push(Annotation { concept, text: t });
            }
        }
        Ok(Waveforms {
            kind,
            groups,
            annotations,
        })
    }
}

/// Reads interleaved samples into each channel, scaled to its sensitivity units.
fn read_samples(
    v: &Values,
    e: &Element,
    bits: u16,
    interpretation: &str,
    samples: usize,
    channels: &mut [Channel],
) -> Result<()> {
    let n = channels.len();
    if n == 0 {
        return Ok(());
    }
    let bytes = v.bytes(e);
    // OW data in a big endian file is swapped per 16-bit word; OB is a byte stream.
    let big = v.big && e.vr != Vr::OB;
    let raw: Box<dyn Fn(usize) -> Option<f64>> = match (bits, interpretation) {
        (8, "SB") => Box::new(|i| bytes.get(i).map(|&b| b as i8 as f64)),
        (8, "UB") => Box::new(|i| bytes.get(i).map(|&b| b as f64)),
        (8, "MB") => Box::new(|i| bytes.get(i).map(|&b| mulaw(b) as f64)),
        (8, "AB") => Box::new(|i| bytes.get(i).map(|&b| alaw(b) as f64)),
        (16, "SS") | (16, "US") => {
            let signed = interpretation == "SS";
            Box::new(move |i| {
                let c = bytes.get(i * 2..i * 2 + 2)?;
                let u = if big {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                };
                Some(if signed { u as i16 as f64 } else { u as f64 })
            })
        }
        (32, "SL") | (32, "UL") => {
            let signed = interpretation == "SL";
            Box::new(move |i| {
                let c: [u8; 4] = bytes.get(i * 4..i * 4 + 4)?.try_into().ok()?;
                let u = if big {
                    u32::from_be_bytes(c)
                } else {
                    u32::from_le_bytes(c)
                };
                Some(if signed { u as i32 as f64 } else { u as f64 })
            })
        }
        (64, "SV") | (64, "UV") => {
            let signed = interpretation == "SV";
            Box::new(move |i| {
                let c: [u8; 8] = bytes.get(i * 8..i * 8 + 8)?.try_into().ok()?;
                let u = if big {
                    u64::from_be_bytes(c)
                } else {
                    u64::from_le_bytes(c)
                };
                Some(if signed { u as i64 as f64 } else { u as f64 })
            })
        }
        _ => {
            return Err(Error::Unsupported(format!(
                "waveform samples of {bits} bits interpreted as {interpretation}"
            )));
        }
    };
    let scale: Vec<(f64, f64)> = channels
        .iter()
        .map(|c| (c.sensitivity * c.correction, c.baseline))
        .collect();
    for s in 0..samples {
        for (c, ch) in channels.iter_mut().enumerate() {
            let Some(r) = raw(s * n + c) else {
                return Ok(());
            };
            let (k, b) = scale[c];
            ch.samples.push((r * k + b) as f32);
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------ layout

/// Paper and trace settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    /// mm per second.
    pub speed: f64,
    /// mm per mV.
    pub gain: f64,
}

impl Default for Paper {
    fn default() -> Paper {
        Paper {
            speed: 25.0,
            gain: 10.0,
        }
    }
}

/// Standard speeds (mm/s) and gains (mm/mV) offered.
pub const SPEEDS: [f64; 4] = [12.5, 25.0, 50.0, 100.0];
pub const GAINS: [f64; 5] = [2.5, 5.0, 10.0, 20.0, 40.0];

/// Width of the trace area (mm): ten seconds at 25 mm/s.
pub const TRACE_WIDTH: f64 = 250.0;
/// Height of one row (mm).
pub const ROW_HEIGHT: f64 = 25.0;
/// Margins (mm): left (labels and calibration pulse), top, right, bottom.
pub const MARGIN: [f64; 4] = [15.0, 8.0, 5.0, 7.0];
/// Most rows on a page.
pub const MAX_ROWS: usize = 16;

/// One trace on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// Channel index in the group.
    pub channel: usize,
    /// Label to print.
    pub label: String,
    /// Left, top of the cell (mm on the page), its width and height.
    pub rect: [f64; 4],
    /// Baseline (0 value) height in mm from the top of the page.
    pub zero: f64,
    /// Time (s from the group start) at the left edge.
    pub t0: f64,
    /// Seconds shown.
    pub seconds: f64,
    /// mm per sensitivity unit (upwards).
    pub scale: f64,
    /// For channels not in volts: the value at the baseline height.
    pub offset: f64,
}

impl Cell {
    /// The page point (mm) of sample `i` with value `y`.
    pub fn point(&self, frequency: f64, i: usize, y: f32) -> [f64; 2] {
        let t = i as f64 / frequency - self.t0;
        [
            self.rect[0] + t / self.seconds * self.rect[2],
            self.zero - (y as f64 - self.offset) * self.scale,
        ]
    }

    /// The sample range shown (start inclusive, end exclusive, clamped to `samples`).
    pub fn range(&self, frequency: f64, samples: usize) -> std::ops::Range<usize> {
        let a = ((self.t0 * frequency).floor().max(0.0) as usize).min(samples);
        let b = (((self.t0 + self.seconds) * frequency).ceil().max(0.0) as usize + 1).min(samples);
        a..b.max(a)
    }
}

/// A page layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Page size (mm).
    pub width: f64,
    pub height: f64,
    pub cells: Vec<Cell>,
    /// Whether this is the standard 12-lead sheet.
    pub twelve_lead: bool,
    /// Seconds across the full trace width at this speed.
    pub seconds: f64,
}

/// Page size (mm) for a group: fixed so speed and gain changes keep the page.
pub fn page_size(group: &Group) -> (f64, f64) {
    let rows = if group.twelve_leads().is_some() {
        4
    } else {
        group.channels.len().clamp(1, MAX_ROWS)
    };
    (
        MARGIN[0] + TRACE_WIDTH + MARGIN[2],
        MARGIN[1] + rows as f64 * ROW_HEIGHT + MARGIN[3],
    )
}

/// Lays out `group` at `paper`, the traces starting `start` seconds into the record (for the
/// one-row-per-channel layout; the 12-lead sheet always shows the record from its start).
pub fn layout(group: &Group, paper: Paper, start: f64) -> Layout {
    let (width, height) = page_size(group);
    let seconds = TRACE_WIDTH / paper.speed;
    let mut cells = Vec::new();
    let x0 = MARGIN[0];
    let y0 = MARGIN[1];
    let scale_of = |ch: &Channel| ch.millivolts().map(|mv| mv * paper.gain);
    if let Some(leads) = group.twelve_leads() {
        // Columns of a quarter of the trace width, each a quarter of the record.
        const COLUMNS: [[Lead; 3]; 4] = [
            [Lead::I, Lead::II, Lead::III],
            [Lead::AVR, Lead::AVL, Lead::AVF],
            [Lead::V1, Lead::V2, Lead::V3],
            [Lead::V4, Lead::V5, Lead::V6],
        ];
        let col_w = TRACE_WIDTH / 4.0;
        let col_s = col_w / paper.speed;
        let quarter = group.duration() / 4.0;
        for (c, col) in COLUMNS.iter().enumerate() {
            for (r, lead) in col.iter().enumerate() {
                let k = Lead::ALL.iter().position(|l| l == lead).unwrap_or(0);
                let ch = leads[k];
                let top = y0 + r as f64 * ROW_HEIGHT;
                cells.push(Cell {
                    channel: ch,
                    label: lead.label().to_owned(),
                    rect: [x0 + c as f64 * col_w, top, col_w, ROW_HEIGHT],
                    zero: top + ROW_HEIGHT * 0.6,
                    t0: c as f64 * quarter,
                    seconds: col_s,
                    scale: scale_of(&group.channels[ch]).unwrap_or(paper.gain),
                    offset: 0.0,
                });
            }
        }
        let ch = leads[1];
        let top = y0 + 3.0 * ROW_HEIGHT;
        cells.push(Cell {
            channel: ch,
            label: Lead::II.label().to_owned(),
            rect: [x0, top, TRACE_WIDTH, ROW_HEIGHT],
            zero: top + ROW_HEIGHT * 0.6,
            t0: 0.0,
            seconds,
            scale: scale_of(&group.channels[ch]).unwrap_or(paper.gain),
            offset: 0.0,
        });
        return Layout {
            width,
            height,
            cells,
            twelve_lead: true,
            seconds,
        };
    }
    for (r, ch) in group.channels.iter().take(MAX_ROWS).enumerate() {
        let top = y0 + r as f64 * ROW_HEIGHT;
        let mut cell = Cell {
            channel: r,
            label: ch.display_label(),
            rect: [x0, top, TRACE_WIDTH, ROW_HEIGHT],
            zero: top + ROW_HEIGHT * 0.5,
            t0: start,
            seconds,
            scale: 1.0,
            offset: 0.0,
        };
        if let Some(s) = scale_of(ch) {
            cell.scale = s;
        } else {
            // Fill 80 % of the row with the shown range of the whole record.
            let (lo, hi) = ch.extent(0..group.samples).unwrap_or((0.0, 1.0));
            let span = (hi - lo).max(f32::EPSILON) as f64;
            cell.scale = ROW_HEIGHT * 0.8 / span;
            cell.offset = (lo as f64 + hi as f64) / 2.0;
        }
        cells.push(cell);
    }
    Layout {
        width,
        height,
        cells,
        twelve_lead: false,
        seconds,
    }
}

/// Heart rate (beats per minute) for an RR interval in seconds.
pub fn rate(interval: f64) -> Option<f64> {
    (interval > 0.0).then(|| 60.0 / interval)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g711_expands_known_values() {
        assert_eq!(mulaw(0xFF), 0);
        assert_eq!(mulaw(0x7F), 0);
        assert_eq!(mulaw(0x00), -32124);
        assert_eq!(mulaw(0x80), 32124);
        assert_eq!(alaw(0xD5), 8);
        assert_eq!(alaw(0x55), -8);
        assert_eq!(alaw(0xAA), 32256);
        assert_eq!(alaw(0x2A), -32256);
    }

    #[test]
    fn leads_parse_from_codes_and_labels() {
        let mdc = Code {
            value: "2:62".into(),
            scheme: "MDC".into(),
            meaning: "Lead aVR".into(),
        };
        assert_eq!(Lead::parse(Some(&mdc), ""), Some(Lead::AVR));
        let scp = Code {
            value: "5.6.3-9-61".into(),
            scheme: "SCPECG".into(),
            meaning: "Lead III".into(),
        };
        assert_eq!(Lead::parse(Some(&scp), ""), Some(Lead::III));
        assert_eq!(Lead::parse(None, "Lead V4 (Unipolar)"), Some(Lead::V4));
        assert_eq!(Lead::parse(None, "avf"), Some(Lead::AVF));
        assert_eq!(Lead::parse(None, "ART"), None);
        assert_eq!(rate(0.8), Some(75.0));
    }

    fn group(labels: &[&str], unit: &str, samples: usize) -> Group {
        Group {
            frequency: 500.0,
            samples,
            channels: labels
                .iter()
                .map(|l| Channel {
                    label: (*l).into(),
                    unit: Some(Code {
                        value: unit.into(),
                        scheme: "UCUM".into(),
                        meaning: String::new(),
                    }),
                    samples: (0..samples).map(|i| i as f32).collect(),
                    ..Channel::default()
                })
                .collect(),
            ..Group::default()
        }
    }

    #[test]
    fn twelve_leads_lay_out_in_four_columns_and_a_rhythm_strip() {
        let g = group(
            &[
                "I", "II", "III", "aVR", "aVL", "aVF", "V1", "V2", "V3", "V4", "V5", "V6",
            ],
            "uV",
            5000,
        );
        let l = layout(&g, Paper::default(), 0.0);
        assert!(l.twelve_lead);
        assert_eq!(l.cells.len(), 13);
        assert_eq!(l.cells[3].label, "aVR");
        assert_eq!(l.cells[3].t0, 2.5);
        assert_eq!(l.cells[3].seconds, 2.5);
        assert_eq!(l.cells[12].label, "II");
        assert_eq!(l.cells[12].seconds, 10.0);
        // 1000 µV is 1 mV, 10 mm at the default gain.
        assert!((l.cells[0].scale * 1000.0 - 10.0).abs() < 1e-9);
        let (w, h) = page_size(&g);
        assert_eq!((w, h), (270.0, 115.0));
        let fast = layout(
            &g,
            Paper {
                speed: 50.0,
                gain: 20.0,
            },
            0.0,
        );
        assert_eq!((fast.width, fast.height), (w, h));
        assert_eq!(fast.cells[12].seconds, 5.0);
    }

    #[test]
    fn other_channels_fill_their_rows() {
        let g = group(&["ART", "PA"], "mm[Hg]", 1000);
        let l = layout(&g, Paper::default(), 1.0);
        assert!(!l.twelve_lead);
        assert_eq!(l.cells.len(), 2);
        let c = &l.cells[0];
        assert_eq!(c.t0, 1.0);
        assert!((c.offset - 499.5).abs() < 1e-9);
        let hi = c.point(500.0, 999, 999.0);
        let lo = c.point(500.0, 0, 0.0);
        assert!((lo[1] - hi[1] - ROW_HEIGHT * 0.8).abs() < 1e-6);
        assert_eq!(c.range(500.0, 1000), 500..1000);
    }
}
