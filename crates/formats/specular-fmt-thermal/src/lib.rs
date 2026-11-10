//! Radiometric thermal stills.
//!
//! * **FLIR**: an FFF container ("FFF\0" or "AFF\0": a 64-byte header and a directory of
//!   32-byte record entries), either standalone (`.fff`, `.img`) or split over APP1
//!   segments of a JPEG ("R-JPEG": each payload starts `FLIR\0`, 0x01, its index and the
//!   last index, and the payloads after those 8 bytes concatenate to the FFF). The records
//!   read are RawData (16-bit counts, raw in either byte order, PNG with its bytes swapped,
//!   or an uncompressed TIFF), CameraInfo (the Planck constants R1, R2, B, F and O, the
//!   object parameters and the atmospheric transmission constants, camera and lens names),
//!   PaletteInfo (the camera's YCrCb palette), EmbeddedImage (the visible photo) and PiP
//!   (where the thermal image sits on it).
//! * **InfiRay** `.irg`: a header, an 8-bit contrast image, 16-bit absolute counts in
//!   steps of 1/10 or 1/16 kelvin, and optionally a visible JPEG.
//! * **InfraTec** `.irb`: a block directory whose first IMAGE block holds a header, a
//!   256-entry kelvin table, a metadata block and the pixels: an 8-bit index and fraction
//!   per pixel interpolated in the table (plain or run-length coded) or 16-bit
//!   centikelvin coded as deltas in a bit stream.
//! * **Testo** `.bmt` (testo 865–883): an XML schema and the data it describes; the
//!   thermal image is 16-bit values spanning −101 °C to 1001 °C.
//! * **Fluke** `.is2` (ZIP layout): protocol-buffer records, calibration curves and 16-bit
//!   counts; temperatures follow emissivity and background temperature.
//!
//! FLIR counts become temperatures by the published radiometric chain (the object signal is
//! what remains of the measured counts after the atmosphere's and the window's own
//! radiance and the reflected radiance are taken away, divided by emissivity and the
//! transmissions; the Planck curve fitted by the camera's calibration then inverts it), so
//! emissivity and reflected temperature can be changed afterwards. The other formats store
//! temperatures already.

mod export;
mod flir;
mod fluke;
mod irb;
mod irg;
mod planck;
mod seq;
mod testo;
mod tiff16;

#[cfg(test)]
mod tests;

use thiserror::Error;

pub use export::{csv, tiff_f32};
pub use flir::{fff_from_jpeg, is_flir_jpeg};
pub use fluke::{
    Curve as FlukeCurve, FlukeCalibration, NO_IR_DATA_REASON, OLD_IS2_REASON, is_old_is2,
};
pub use planck::{Planck, h2o, transmission};
pub use seq::{DEFAULT_RATE, FrameRef, Sequence, index as index_sequence, is_sequence, utc_text};
pub use testo::{OLD_BMT_REASON, is_old_bmt};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("not a thermal image")]
    NotThermal,
    #[error("malformed {format}: {what}")]
    Malformed { format: &'static str, what: String },
    #[error("the FLIR file has no raw thermal data")]
    NoRawData,
    #[error("unsupported {format} data: {what}")]
    Unsupported { format: &'static str, what: String },
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn malformed(format: &'static str, what: impl Into<String>) -> Error {
    Error::Malformed {
        format,
        what: what.into(),
    }
}

/// Where a thermal image came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// FFF in the APP1 segments of a JPEG.
    FlirJpeg,
    /// A standalone FFF (`.fff`, `.img`).
    FlirFff,
    /// A frame of a FLIR `.seq` sequence (FFF frames, raw counts).
    FlirSeq,
    /// A frame of a FLIR `.csq` sequence (FFF frames, JPEG-LS coded counts).
    FlirCsq,
    InfirayIrg,
    InfratecIrb,
    /// Testo `.bmt` (testo 865, 868, 871, 872, 883).
    TestoBmt,
    /// Fluke `.is2` (the ZIP layout).
    FlukeIs2,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::FlirJpeg => "FLIR radiometric JPEG",
            Kind::FlirFff => "FLIR FFF",
            Kind::FlirSeq => "FLIR SEQ sequence",
            Kind::FlirCsq => "FLIR CSQ sequence",
            Kind::InfirayIrg => "InfiRay IRG",
            Kind::InfratecIrb => "InfraTec IRB",
            Kind::TestoBmt => "Testo BMT",
            Kind::FlukeIs2 => "Fluke IS2",
        }
    }
}

/// The pixels as stored.
#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    /// 16-bit counts and how they become degrees Celsius.
    Counts { values: Vec<u16>, mapping: Mapping },
    /// Degrees Celsius, already computed by the format's own rule.
    Celsius(Vec<f32>),
}

/// From counts to degrees Celsius.
#[derive(Clone, Debug, PartialEq)]
pub enum Mapping {
    /// The FLIR radiometric chain (it depends on the [`Conditions`]).
    Planck(Planck),
    /// `counts / divisor - offset`.
    Linear { divisor: f64, offset: f64 },
    /// Fluke's calibration curves (it depends on the [`Conditions`]).
    Fluke(FlukeCalibration),
}

/// Object and environment parameters, as recorded (`None` where the format has none).
/// Temperatures are in degrees Celsius, distance in metres, humidity a fraction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Conditions {
    pub emissivity: Option<f64>,
    pub distance: Option<f64>,
    pub reflected: Option<f64>,
    pub atmospheric: Option<f64>,
    pub window_temperature: Option<f64>,
    pub window_transmission: Option<f64>,
    pub humidity: Option<f64>,
}

/// Camera description, as recorded (empty strings where absent).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Camera {
    pub model: String,
    pub part_number: String,
    pub serial: String,
    pub software: String,
    pub lens: String,
    pub lens_part_number: String,
    pub lens_serial: String,
    pub filter: String,
    /// Horizontal field of view, degrees.
    pub field_of_view: Option<f64>,
    /// The camera's calibrated range, degrees Celsius.
    pub range: Option<(f64, f64)>,
    /// Centre wavelength, micrometres.
    pub wavelength: Option<f64>,
    /// Date and time as recorded, "YYYY-MM-DD HH:MM:SS", with the offset from UTC when known.
    pub date: Option<String>,
}

/// The camera's colour palette.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub name: String,
    /// Colours from cold to hot, sRGB.
    pub colours: Vec<[u8; 3]>,
    /// Shown above the span, below it, and for isotherms (sRGB), when recorded.
    pub above: Option<[u8; 3]>,
    pub below: Option<[u8; 3]>,
    pub isotherm: Option<[u8; 3]>,
}

impl Palette {
    /// The palette resampled to `n` entries (nearest, end points kept).
    pub fn resampled(&self, n: usize) -> Vec<[u8; 3]> {
        let m = self.colours.len();
        if m == 0 || n == 0 {
            return vec![[0; 3]; n];
        }
        (0..n)
            .map(|i| {
                let t = if n == 1 {
                    0.0
                } else {
                    i as f64 / (n - 1) as f64
                };
                self.colours[((t * (m - 1) as f64).round() as usize).min(m - 1)]
            })
            .collect()
    }
}

/// The embedded visible photo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visible {
    /// A complete JPEG or PNG file.
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Picture-in-picture placement of the thermal image on the visible photo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pip {
    /// Visible pixels per thermal pixel.
    pub real_to_ir: f64,
    pub offset_x: i32,
    pub offset_y: i32,
    pub x1: u32,
    pub x2: u32,
    pub y1: u32,
    pub y2: u32,
}

/// A thermal still.
#[derive(Clone, Debug, PartialEq)]
pub struct Thermal {
    pub kind: Kind,
    pub width: usize,
    pub height: usize,
    pub pixels: Pixels,
    pub conditions: Conditions,
    pub camera: Camera,
    pub palette: Option<Palette>,
    pub visible: Option<Visible>,
    pub pip: Option<Pip>,
}

/// Which format `data` is, cheaply; R-JPEG only when a FLIR APP1 segment comes before the
/// scan (an ordinary JPEG costs a walk over its marker segment headers). A Testo `.bmt`
/// is found here only without its JPEG preview (see [`is_kind`]).
pub fn sniff(data: &[u8]) -> Option<Kind> {
    if flir::is_fff(data) {
        Some(if seq::is_sequence(data) {
            seq::kind(data)
        } else {
            Kind::FlirFff
        })
    } else if irb::is_irb(data) {
        Some(Kind::InfratecIrb)
    } else if is_flir_jpeg(data) {
        Some(Kind::FlirJpeg)
    } else if irg::is_irg(data) {
        Some(Kind::InfirayIrg)
    } else if fluke::is_is2(data) {
        Some(Kind::FlukeIs2)
    } else if data.starts_with(b"<ToFo") && testo::is_bmt(data) {
        // A `.bmt` led by its JPEG preview needs a search for the header: only
        // [`is_kind`] and [`read`] look for it.
        Some(Kind::TestoBmt)
    } else {
        None
    }
}

/// Whether `data` is of `kind` (only that kind's check runs: for R-JPEG a walk over the
/// marker segment headers before the scan; for a Testo `.bmt` a search of the first 4 MiB
/// for its `<ToFo>` header, so callers check the extension first).
pub fn is_kind(data: &[u8], kind: Kind) -> bool {
    match kind {
        Kind::FlirFff => flir::is_fff(data),
        Kind::FlirSeq | Kind::FlirCsq => seq::is_sequence(data),
        Kind::InfratecIrb => irb::is_irb(data),
        Kind::FlirJpeg => is_flir_jpeg(data),
        Kind::InfirayIrg => irg::is_irg(data),
        Kind::TestoBmt => testo::is_bmt(data),
        Kind::FlukeIs2 => fluke::is_is2(data),
    }
}

/// Reads a thermal still of any supported kind (the first frame of a sequence). A JPEG
/// without FLIR raw data is [`Error::NotThermal`] or [`Error::NoRawData`].
pub fn read(data: &[u8]) -> Result<Thermal> {
    match sniff(data) {
        Some(Kind::FlirFff) => flir::read_fff(data, Kind::FlirFff),
        Some(Kind::FlirSeq | Kind::FlirCsq) => seq::index(data)?.frame(data, 0),
        Some(Kind::FlirJpeg) => {
            let fff = fff_from_jpeg(data)?;
            flir::read_fff(&fff, Kind::FlirJpeg)
        }
        Some(Kind::InfirayIrg) => irg::read(data),
        Some(Kind::InfratecIrb) => irb::read(data),
        Some(Kind::TestoBmt) => testo::read(data),
        Some(Kind::FlukeIs2) => fluke::read(data),
        None if testo::is_bmt(data) => testo::read(data),
        None => Err(Error::NotThermal),
    }
}

impl Thermal {
    /// Whether temperatures depend on the [`Conditions`] (FLIR and Fluke counts).
    pub fn adjustable(&self) -> bool {
        matches!(
            self.pixels,
            Pixels::Counts {
                mapping: Mapping::Planck(_) | Mapping::Fluke(_),
                ..
            }
        )
    }

    /// The smallest and largest stored count (`None` for stored temperatures).
    pub fn raw_range(&self) -> Option<(u16, u16)> {
        match &self.pixels {
            Pixels::Counts { values, .. } => {
                let mut it = values.iter().copied();
                let first = it.next()?;
                Some(it.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v))))
            }
            Pixels::Celsius(_) => None,
        }
    }

    /// Degrees Celsius for every pixel, row-major, using `conditions` where the format
    /// allows them to change (FLIR) and the stored rule otherwise. Counts the calibration
    /// cannot invert are NaN.
    pub fn temperatures(&self, conditions: &Conditions) -> Vec<f32> {
        match &self.pixels {
            Pixels::Celsius(t) => t.clone(),
            Pixels::Counts { values, mapping } => {
                let Some((lo, hi)) = self.raw_range() else {
                    return Vec::new();
                };
                let table: Vec<f32> = match mapping {
                    Mapping::Planck(p) => {
                        let s = p.solver(conditions);
                        (lo..=hi).map(|v| s.celsius(f64::from(v)) as f32).collect()
                    }
                    Mapping::Linear { divisor, offset } => (lo..=hi)
                        .map(|v| (f64::from(v) / divisor - offset) as f32)
                        .collect(),
                    Mapping::Fluke(f) => (lo..=hi)
                        .map(|v| f.celsius(f64::from(v), conditions) as f32)
                        .collect(),
                };
                values.iter().map(|&v| table[usize::from(v - lo)]).collect()
            }
        }
    }

    /// The temperature of one count under `conditions` (FLIR), or the stored rule.
    pub fn celsius_of_count(&self, count: u16, conditions: &Conditions) -> Option<f64> {
        match &self.pixels {
            Pixels::Counts {
                mapping: Mapping::Planck(p),
                ..
            } => Some(p.solver(conditions).celsius(f64::from(count))),
            Pixels::Counts {
                mapping: Mapping::Linear { divisor, offset },
                ..
            } => Some(f64::from(count) / divisor - offset),
            Pixels::Counts {
                mapping: Mapping::Fluke(f),
                ..
            } => Some(f.celsius(f64::from(count), conditions)),
            Pixels::Celsius(_) => None,
        }
    }
}

/// Width and height of a JPEG from its first frame header.
pub(crate) fn jpeg_size(d: &[u8]) -> Option<(u32, u32)> {
    if !d.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut p = 2;
    loop {
        while *d.get(p)? != 0xff {
            p += 1;
        }
        while *d.get(p)? == 0xff {
            p += 1;
        }
        let m = *d.get(p)?;
        p += 1;
        if m == 0xd8 || (0xd0..=0xd7).contains(&m) || m == 0x01 {
            continue;
        }
        let len = usize::from(u16::from_be_bytes([*d.get(p)?, *d.get(p + 1)?]));
        if matches!(m, 0xc0..=0xcf) && !matches!(m, 0xc4 | 0xc8 | 0xcc) {
            let h = u16::from_be_bytes([*d.get(p + 3)?, *d.get(p + 4)?]);
            let w = u16::from_be_bytes([*d.get(p + 5)?, *d.get(p + 6)?]);
            return (w > 0 && h > 0).then_some((u32::from(w), u32::from(h)));
        }
        if m == 0xda || m == 0xd9 || len < 2 {
            return None;
        }
        p += len;
    }
}
