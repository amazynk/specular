//! PET standardised uptake values (SUV): body weight (SUVbw), lean body mass (SUVlbm, James)
//! and body surface area (SUVbsa, Du Bois), following the QIBA vendor-neutral SUV pseudo-code.
//!
//! An SUV is the modality value (activity concentration, Bq/mL, after Rescale) times a factor:
//! the body measure over the injected dose decayed to the scan start. Every input is read
//! from the image; when one is missing or out of range the SUV is refused with the reason
//! ([`Refusal`]), never guessed. Read-only analysis (DESIGN §7): a number, not a finding.

use crate::anon::days_from_civil;
use crate::labels::numbers;
use crate::parse::DataSet;
use crate::{Dicom, Values};

const MODALITY: u32 = 0x0008_0060;
const SERIES_DATE: u32 = 0x0008_0021;
const ACQUISITION_DATE: u32 = 0x0008_0022;
const SERIES_TIME: u32 = 0x0008_0031;
const ACQUISITION_TIME: u32 = 0x0008_0032;
const PATIENT_SEX: u32 = 0x0010_0040;
const PATIENT_SIZE: u32 = 0x0010_1020;
const PATIENT_WEIGHT: u32 = 0x0010_1030;
const START_TIME: u32 = 0x0018_1072;
const TOTAL_DOSE: u32 = 0x0018_1074;
const HALF_LIFE: u32 = 0x0018_1075;
const START_DATETIME: u32 = 0x0018_1078;
const CORRECTED_IMAGE: u32 = 0x0028_0051;
const RADIOPHARMACEUTICAL: u32 = 0x0054_0016;
const UNITS: u32 = 0x0054_1001;
const DECAY_CORRECTION: u32 = 0x0054_1102;
const GE_GROUP: u16 = 0x0009;
const GE_CREATOR: &str = "GEMS_PETD_01";
const GE_SCAN_DATETIME: u8 = 0x0D;
const PHILIPS_GROUP: u16 = 0x7053;
const PHILIPS_CREATOR: &str = "Philips PET Private Group";
const PHILIPS_SUV_FACTOR: u8 = 0x00;
const PHILIPS_ACTIVITY_FACTOR: u8 = 0x09;

/// The injection may be at most this long before the scan, s.
const MAX_DELAY: f64 = 24.0 * 3600.0;

/// Why there is no SUV.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotPet,
    /// Units (0054,1001) missing or not BQML, CNTS (Philips) or GML.
    Units(String),
    /// Corrected Image (0028,0051) lacks DECY or ATTN.
    NotCorrected,
    /// Decay Correction (0054,1102) is NONE or unknown.
    DecayCorrection(String),
    NoWeight,
    NoDose,
    NoHalfLife,
    NoInjectionTime,
    NoScanTime,
    /// The injection is after the scan, or more than a day before it.
    Delay,
    /// CNTS without Philips' activity or SUV scale factor.
    NoScaleFactor,
    /// SUVlbm / SUVbsa: Patient's Size (0010,1020) or Sex (0010,0040) missing.
    NoHeight,
    NoSex,
    /// The lean body mass formula gives no positive mass for this weight and height.
    LeanMass,
}

impl Refusal {
    /// The i18n message id (crates/specular-i18n, `suv-refused-*`).
    pub fn key(&self) -> &'static str {
        match self {
            Refusal::NotPet => "suv-refused-not-pet",
            Refusal::Units(_) => "suv-refused-units",
            Refusal::NotCorrected => "suv-refused-corrected",
            Refusal::DecayCorrection(_) => "suv-refused-decay",
            Refusal::NoWeight => "suv-refused-weight",
            Refusal::NoDose => "suv-refused-dose",
            Refusal::NoHalfLife => "suv-refused-half-life",
            Refusal::NoInjectionTime => "suv-refused-injection",
            Refusal::NoScanTime => "suv-refused-scan-time",
            Refusal::Delay => "suv-refused-delay",
            Refusal::NoScaleFactor => "suv-refused-scale",
            Refusal::NoHeight => "suv-refused-height",
            Refusal::NoSex => "suv-refused-sex",
            Refusal::LeanMass => "suv-refused-lbm",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key())?;
        match self {
            Refusal::Units(u) | Refusal::DecayCorrection(u) if !u.is_empty() => write!(f, " ({u})"),
            _ => Ok(()),
        }
    }
}

/// The image's unit before SUV.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Units {
    /// Bq/mL.
    Bqml,
    /// Philips counts with an activity concentration scale factor (Bq/mL per count).
    Counts { bqml_per_count: f64 },
    /// Philips counts with an SUVbw scale factor.
    CountsSuv { suv_per_count: f64 },
    /// Already SUVbw (g/mL).
    Gml,
}

/// What SUV needs, read from the image ([`Inputs::read`]) or given (tests).
#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    pub units: Units,
    /// START (decayed to the scan start) or ADMIN (to the injection).
    pub decay_correction: String,
    pub weight_kg: Option<f64>,
    pub height_m: Option<f64>,
    /// "M" or "F".
    pub sex: Option<String>,
    pub dose_bq: Option<f64>,
    pub half_life_s: Option<f64>,
    /// Seconds since 1970-01-01 (local time, as the image states it).
    pub injection: Option<f64>,
    pub scan: Option<f64>,
}

/// SUV factors: SUV = modality value × factor.
#[derive(Clone, Debug, PartialEq)]
pub struct Suv {
    /// SUVbw, g/mL.
    pub bw: f64,
    /// SUVlbm (James), g/mL.
    pub lbm: Result<f64, Refusal>,
    /// SUVbsa (Du Bois), cm²/mL.
    pub bsa: Result<f64, Refusal>,
    /// Injection to scan start, s (0 for GML and Philips' SUV factor).
    pub delay_s: f64,
    /// The dose's decay over the delay (1 for ADMIN).
    pub decay: f64,
    pub inputs: Inputs,
}

/// Which SUV.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Bw,
    Lbm,
    Bsa,
}

impl Suv {
    pub fn factor(&self, kind: Kind) -> Result<f64, Refusal> {
        match kind {
            Kind::Bw => Ok(self.bw),
            Kind::Lbm => self.lbm.clone(),
            Kind::Bsa => self.bsa.clone(),
        }
    }

    /// From the image: `earliest_acquisition`, when known, is the series' earliest Acquisition
    /// Date/Time (s since 1970), the scan start when the Series Time is later than it.
    pub fn read(d: &Dicom, file: &[u8], earliest_acquisition: Option<f64>) -> Result<Suv, Refusal> {
        Suv::compute(Inputs::read(
            &d.dataset,
            &d.values(file),
            earliest_acquisition,
        )?)
    }

    /// The series' earliest acquisition (s since 1970) for [`Suv::read`]: this image's
    /// Acquisition Date and Time, moved back by how much earlier the series' earliest
    /// Acquisition Time of day (`series_times`, s, as [`crate::series::Instance`] reads
    /// them) is. `None` without this image's acquisition date and time.
    pub fn earliest_acquisition(d: &Dicom, file: &[u8], series_times: &[f64]) -> Option<f64> {
        let v = d.values(file);
        let date = text(&d.dataset, &v, ACQUISITION_DATE)?;
        let time = text(&d.dataset, &v, ACQUISITION_TIME)?;
        let own = date_time(date, time)?;
        let tod = seconds(time)?;
        let min = series_times.iter().copied().fold(tod, f64::min);
        Some(own - (tod - min))
    }

    pub fn compute(inputs: Inputs) -> Result<Suv, Refusal> {
        let weight = inputs
            .weight_kg
            .filter(|w| *w > 0.0)
            .ok_or(Refusal::NoWeight)?;
        let lbm_kg = lean_mass(&inputs);
        let bsa_m2 = match inputs.height_m.filter(|h| *h > 0.0) {
            Some(h) => Ok(du_bois(weight, h)),
            None => Err(Refusal::NoHeight),
        };
        let (bw, delay_s, decay) = match inputs.units {
            Units::Gml => (1.0, 0.0, 1.0),
            Units::CountsSuv { suv_per_count } => (suv_per_count, 0.0, 1.0),
            Units::Bqml | Units::Counts { .. } => {
                let dose = inputs.dose_bq.filter(|d| *d > 0.0).ok_or(Refusal::NoDose)?;
                let (delay, decay) = match inputs.decay_correction.as_str() {
                    "ADMIN" => (0.0, 1.0),
                    "START" => {
                        let half = inputs
                            .half_life_s
                            .filter(|h| *h > 0.0)
                            .ok_or(Refusal::NoHalfLife)?;
                        let inj = inputs.injection.ok_or(Refusal::NoInjectionTime)?;
                        let scan = inputs.scan.ok_or(Refusal::NoScanTime)?;
                        let delay = scan - inj;
                        if !(0.0..MAX_DELAY).contains(&delay) {
                            return Err(Refusal::Delay);
                        }
                        (delay, (-delay * std::f64::consts::LN_2 / half).exp())
                    }
                    other => return Err(Refusal::DecayCorrection(other.to_owned())),
                };
                let per = match inputs.units {
                    Units::Counts { bqml_per_count } => bqml_per_count,
                    _ => 1.0,
                };
                (per * weight * 1000.0 / (dose * decay), delay, decay)
            }
        };
        // SUVbw × (measure / weight): the same decayed dose over another body measure.
        let w_g = weight * 1000.0;
        let lbm = lbm_kg.map(|l| bw * l * 1000.0 / w_g);
        let bsa = bsa_m2.map(|b| bw * b * 10_000.0 / w_g);
        Ok(Suv {
            bw,
            lbm,
            bsa,
            delay_s,
            decay,
            inputs,
        })
    }
}

/// Lean body mass, kg (James 1976): men 1.10 W − 128 (W/H)², women 1.07 W − 148 (W/H)², W in
/// kg, H in cm.
fn lean_mass(i: &Inputs) -> Result<f64, Refusal> {
    let w = i.weight_kg.ok_or(Refusal::NoWeight)?;
    let h = i.height_m.filter(|h| *h > 0.0).ok_or(Refusal::NoHeight)? * 100.0;
    let r = (w / h).powi(2);
    let l = match i.sex.as_deref() {
        Some("M") => 1.10 * w - 128.0 * r,
        Some("F") => 1.07 * w - 148.0 * r,
        _ => return Err(Refusal::NoSex),
    };
    if l > 0.0 {
        Ok(l)
    } else {
        Err(Refusal::LeanMass)
    }
}

/// Body surface area, m² (Du Bois 1916): 0.007184 W^0.425 H^0.725, H in cm.
fn du_bois(w: f64, h_m: f64) -> f64 {
    0.007184 * w.powf(0.425) * (h_m * 100.0).powf(0.725)
}

fn text<'a>(ds: &DataSet, v: &Values<'a>, tag: u32) -> Option<&'a str> {
    ds.get(tag)
        .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn number(ds: &DataSet, v: &Values, tag: u32) -> Option<f64> {
    ds.get(tag)
        .and_then(|e| numbers(v, e).first().copied())
        .filter(|x| x.is_finite())
}

/// The tag of private element `offset` of `creator`'s block in `group`.
fn private(ds: &DataSet, v: &Values, group: u16, creator: &str, offset: u8) -> Option<u32> {
    (0x10u32..=0xFF).find_map(|b| {
        let tag = (u32::from(group) << 16) | b;
        let e = ds.get(tag)?;
        let c = std::str::from_utf8(v.text_bytes(e)).ok()?.trim();
        c.eq_ignore_ascii_case(creator)
            .then_some((u32::from(group) << 16) | (b << 8) | u32::from(offset))
    })
}

/// Seconds since 1970-01-01 of DA `date` and TM `time` (HHMMSS.FFFFFF, or the old HH:MM:SS).
pub fn date_time(date: &str, time: &str) -> Option<f64> {
    Some(day(date)? as f64 * 86400.0 + seconds(time)?)
}

fn day(date: &str) -> Option<i64> {
    let d: String = date.chars().filter(|c| c.is_ascii_digit()).collect();
    if d.len() != 8 {
        return None;
    }
    let y = d[0..4].parse().ok()?;
    let m: i64 = d[4..6].parse().ok()?;
    let dd: i64 = d[6..8].parse().ok()?;
    days_from_civil(y, m, dd)
}

/// Seconds into the day of a TM value.
pub fn seconds(time: &str) -> Option<f64> {
    let t: String = time.trim().chars().filter(|c| *c != ':').collect();
    let (whole, frac) = t.split_once('.').unwrap_or((&t, ""));
    if whole.len() < 2
        || whole.len() % 2 == 1
        || whole.len() > 6
        || !whole.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let field = |i: usize| -> f64 {
        whole
            .get(i..i + 2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
    };
    let (h, m, s) = (field(0), field(2), field(4));
    if h >= 24.0 || m >= 60.0 || s >= 61.0 {
        return None;
    }
    let f: f64 = if frac.is_empty() {
        0.0
    } else {
        format!("0.{frac}").parse().ok()?
    };
    Some(h * 3600.0 + m * 60.0 + s + f)
}

/// Seconds since 1970 of a DT value (YYYYMMDDHHMMSS.FFFFFF, any UTC offset ignored: the other
/// times in the image are local too).
pub fn dt(value: &str) -> Option<f64> {
    let v = value.trim();
    let v = v.split(['+', '-']).next()?;
    if v.len() < 8 {
        return None;
    }
    let time = if v.len() > 8 { &v[8..] } else { "00" };
    date_time(&v[..8], time)
}

impl Inputs {
    /// Reads the inputs; refuses what no SUV can come from (not PET, units, corrections).
    pub fn read(
        ds: &DataSet,
        v: &Values,
        earliest_acquisition: Option<f64>,
    ) -> Result<Inputs, Refusal> {
        if text(ds, v, MODALITY) != Some("PT") {
            return Err(Refusal::NotPet);
        }
        let units_text = text(ds, v, UNITS).unwrap_or("").to_ascii_uppercase();
        let units = match units_text.as_str() {
            "BQML" => Units::Bqml,
            "GML" => Units::Gml,
            "CNTS" => {
                let factor = |o| {
                    private(ds, v, PHILIPS_GROUP, PHILIPS_CREATOR, o)
                        .and_then(|t| number(ds, v, t))
                        .filter(|f| *f > 0.0)
                };
                match (factor(PHILIPS_ACTIVITY_FACTOR), factor(PHILIPS_SUV_FACTOR)) {
                    (Some(a), _) => Units::Counts { bqml_per_count: a },
                    (None, Some(s)) => Units::CountsSuv { suv_per_count: s },
                    _ => return Err(Refusal::NoScaleFactor),
                }
            }
            _ => return Err(Refusal::Units(units_text)),
        };
        if matches!(units, Units::Bqml | Units::Counts { .. }) {
            let corrected = ds
                .get(CORRECTED_IMAGE)
                .map(|e| {
                    let n = v.count(e);
                    (0..n)
                        .filter_map(|i| v.text_n(e, i))
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !corrected.iter().any(|c| c == "DECY") || !corrected.iter().any(|c| c == "ATTN") {
                return Err(Refusal::NotCorrected);
            }
        }
        let decay_correction = text(ds, v, DECAY_CORRECTION)
            .unwrap_or("")
            .to_ascii_uppercase();
        let rp = ds.item(RADIOPHARMACEUTICAL);
        let rp_text = |tag| rp.and_then(|r| text(r, v, tag));
        let rp_number = |tag| rp.and_then(|r| number(r, v, tag));
        let series = text(ds, v, SERIES_DATE)
            .zip(text(ds, v, SERIES_TIME))
            .and_then(|(d, t)| date_time(d, t));
        let acquisition = text(ds, v, ACQUISITION_DATE)
            .zip(text(ds, v, ACQUISITION_TIME))
            .and_then(|(d, t)| date_time(d, t));
        let first_acquisition = earliest_acquisition.or(acquisition);
        // QIBA: the series time, unless a later reconstruction moved it past the acquisition.
        let scan = match (series, first_acquisition) {
            (Some(s), Some(a)) if s <= a => Some(s),
            (Some(s), None) => Some(s),
            (_, a) => private(ds, v, GE_GROUP, GE_CREATOR, GE_SCAN_DATETIME)
                .and_then(|t| text(ds, v, t))
                .and_then(dt)
                .or(a),
        };
        // The injection: its date-time when that gives a plausible delay, else its time on the
        // scan's day (anonymisation often shifts one date and not the other).
        let scan_day = scan.map(|s| (s / 86400.0).floor() * 86400.0);
        let plausible = |inj: f64| scan.is_some_and(|s| (0.0..MAX_DELAY).contains(&(s - inj)));
        let injection = rp_text(START_DATETIME)
            .and_then(dt)
            .filter(|i| plausible(*i))
            .or_else(|| {
                let t = seconds(rp_text(START_TIME)?)?;
                let i = scan_day? + t;
                // Injected before midnight, scanned after.
                Some(if scan.is_some_and(|s| i > s) {
                    i - 86400.0
                } else {
                    i
                })
            });
        Ok(Inputs {
            units,
            decay_correction,
            weight_kg: number(ds, v, PATIENT_WEIGHT).filter(|w| *w > 0.0),
            height_m: number(ds, v, PATIENT_SIZE).filter(|h| *h > 0.0),
            sex: text(ds, v, PATIENT_SEX).map(str::to_owned),
            dose_bq: rp_number(TOTAL_DOSE),
            half_life_s: rp_number(HALF_LIFE),
            injection,
            scan,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Inputs {
        Inputs {
            units: Units::Bqml,
            decay_correction: "START".into(),
            weight_kg: Some(70.0),
            height_m: Some(1.75),
            sex: Some("M".into()),
            dose_bq: Some(370e6),
            half_life_s: Some(6586.2),
            injection: date_time("20240102", "100000"),
            scan: date_time("20240102", "110000"),
        }
    }

    #[test]
    fn hand_computed() {
        // 1 h after injection of 370 MBq F-18: decay 2^(−3600/6586.2) = 0.684633…;
        // SUVbw factor = 70000 g / (370e6 × 0.684633) = 2.763367e-4 mL/Bq.
        let s = Suv::compute(inputs()).unwrap();
        assert_eq!(s.delay_s, 3600.0);
        assert!((s.decay - 0.684_632_9).abs() < 1e-6, "{}", s.decay);
        assert!((s.bw - 2.763_367e-4).abs() < 1e-9, "{}", s.bw);
        // James LBM 1.10×70 − 128×(70/175)² = 56.52 kg.
        let lbm = s.lbm.clone().unwrap();
        assert!((lbm - s.bw * 56.52 / 70.0).abs() < 1e-12);
        // Du Bois 0.007184 × 70^0.425 × 175^0.725 = 1.8481 m².
        let bsa = s.bsa.clone().unwrap();
        assert!(
            (bsa / (s.bw / 70_000.0) / 1e4 - 1.848_143).abs() < 1e-5,
            "{bsa}"
        );
        // 5000 Bq/mL there is SUVbw 1.3817.
        assert!((5000.0 * s.bw - 1.381_683).abs() < 1e-5);
    }

    #[test]
    fn admin_and_gml_and_counts() {
        let mut i = inputs();
        i.decay_correction = "ADMIN".into();
        let s = Suv::compute(i).unwrap();
        assert_eq!(s.decay, 1.0);
        assert!((s.bw - 70_000.0 / 370e6).abs() < 1e-15);
        let mut i = inputs();
        i.units = Units::Gml;
        assert_eq!(Suv::compute(i).unwrap().bw, 1.0);
        let mut i = inputs();
        i.units = Units::Counts {
            bqml_per_count: 2.0,
        };
        let c = Suv::compute(i).unwrap().bw;
        assert!((c - 2.0 * 2.763_367e-4).abs() < 1e-9);
    }

    #[test]
    fn refusals() {
        let mut i = inputs();
        i.weight_kg = None;
        assert_eq!(Suv::compute(i).unwrap_err(), Refusal::NoWeight);
        let mut i = inputs();
        i.decay_correction = "NONE".into();
        assert_eq!(
            Suv::compute(i).unwrap_err(),
            Refusal::DecayCorrection("NONE".into())
        );
        let mut i = inputs();
        i.injection = date_time("20240102", "120000");
        assert_eq!(Suv::compute(i).unwrap_err(), Refusal::Delay);
        let mut i = inputs();
        i.dose_bq = None;
        assert_eq!(Suv::compute(i).unwrap_err(), Refusal::NoDose);
        let mut i = inputs();
        i.sex = Some("O".into());
        assert_eq!(Suv::compute(i).unwrap().lbm, Err(Refusal::NoSex));
        let mut i = inputs();
        i.height_m = None;
        let s = Suv::compute(i).unwrap();
        assert_eq!(s.bsa, Err(Refusal::NoHeight));
        // Very high BMI: James' formula turns down, then negative.
        let mut i = inputs();
        i.weight_kg = Some(250.0);
        i.height_m = Some(1.5);
        assert_eq!(Suv::compute(i).unwrap().lbm, Err(Refusal::LeanMass));
    }

    #[test]
    fn times() {
        assert_eq!(seconds("125400"), Some(12.0 * 3600.0 + 54.0 * 60.0));
        assert_eq!(seconds("12:54:00.5"), Some(46440.5));
        assert_eq!(seconds("1254"), Some(46440.0));
        assert_eq!(seconds("25"), None);
        assert_eq!(dt("20031231125400.00"), date_time("20031231", "125400"));
        assert_eq!(dt("20031231125400+0100"), date_time("20031231", "125400"));
        assert_eq!(date_time("19700102", "000000"), Some(86400.0));
    }
}
