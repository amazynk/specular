//! Calibration: from pixels to physical units, with where the scale came from.
//!
//! A calibration is a linear map from a pixel displacement `(dx, dy)` to a physical one,
//! `m · (dx, dy)`. Square and rectangular pixels are diagonal maps; a FITS CD matrix may also
//! rotate and shear. Lengths are `|m · d|`, areas scale by `|det m|`, and angles are measured
//! between the mapped vectors, so non-square pixels give the physical angle.

use std::fmt;

use crate::geom::P;

/// Where a calibration comes from: shown as a badge next to every value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// No calibration: values are in pixels.
    Uncalibrated,
    /// DICOM Pixel Spacing (0028,0030) in the patient.
    DicomPixelSpacing,
    /// DICOM Pixel Spacing calibrated by the modality or an operator (Pixel Spacing
    /// Calibration Type, 0028,0A02).
    DicomCalibrated,
    /// DICOM Imager Pixel Spacing (0018,1164): at the detector, so lengths in the patient are
    /// smaller by the magnification.
    DicomImagerPixelSpacing,
    /// Imager Pixel Spacing divided by the Estimated Radiographic Magnification Factor
    /// (0018,1114): an estimate.
    DicomMagnification,
    /// A spatially calibrated DICOM ultrasound region (0018,6011).
    DicomUltrasoundRegion,
    /// TIFF XResolution / YResolution / ResolutionUnit.
    TiffResolution,
    /// EXIF XResolution / YResolution / ResolutionUnit (usually only print density).
    ExifResolution,
    /// OME-XML PhysicalSizeX / PhysicalSizeY.
    OmePhysicalSize,
    /// A FITS world coordinate system (CDi_j or CDELTi / CROTA2), on the tangent plane.
    FitsWcs,
    /// A vendor-specific tag.
    VendorTag,
    /// The units of a drawing (CAD).
    DrawingUnits,
    /// A drawing's stated scale: a PDF viewport's measure dictionary, or a CAD layout
    /// viewport onto model space.
    DrawingScale,
    /// A map's scale at the image's centre (a projection's scale varies across the map).
    MapScale,
    /// Set by the user from a known length.
    User,
}

impl Source {
    pub const ALL: [Source; 15] = [
        Source::Uncalibrated,
        Source::DicomPixelSpacing,
        Source::DicomCalibrated,
        Source::DicomImagerPixelSpacing,
        Source::DicomMagnification,
        Source::DicomUltrasoundRegion,
        Source::TiffResolution,
        Source::ExifResolution,
        Source::OmePhysicalSize,
        Source::FitsWcs,
        Source::VendorTag,
        Source::DrawingUnits,
        Source::DrawingScale,
        Source::MapScale,
        Source::User,
    ];

    /// A stable identifier for files (CSV, GeoJSON) and string ids (`measure-source-<key>`).
    pub fn key(self) -> &'static str {
        match self {
            Source::Uncalibrated => "uncalibrated",
            Source::DicomPixelSpacing => "dicom-pixel-spacing",
            Source::DicomCalibrated => "dicom-calibrated",
            Source::DicomImagerPixelSpacing => "dicom-imager-pixel-spacing",
            Source::DicomMagnification => "dicom-magnification",
            Source::DicomUltrasoundRegion => "dicom-ultrasound-region",
            Source::TiffResolution => "tiff-resolution",
            Source::ExifResolution => "exif-resolution",
            Source::OmePhysicalSize => "ome-physical-size",
            Source::FitsWcs => "fits-wcs",
            Source::VendorTag => "vendor-tag",
            Source::DrawingUnits => "drawing-units",
            Source::DrawingScale => "drawing-scale",
            Source::MapScale => "map-scale",
            Source::User => "user",
        }
    }

    pub fn from_key(key: &str) -> Option<Source> {
        Source::ALL.into_iter().find(|s| s.key() == key)
    }

    /// Whether values through this source need a caveat (at the detector, or estimated).
    pub fn is_approximate(self) -> bool {
        matches!(
            self,
            Source::DicomImagerPixelSpacing
                | Source::DicomMagnification
                | Source::ExifResolution
                | Source::MapScale
        )
    }
}

/// A unit of length (areas are its square).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Px,
    Nm,
    Um,
    Mm,
    Cm,
    M,
    Inch,
    /// Seconds of arc on the sky (FITS WCS).
    Arcsec,
    /// Feet, yards, kilometres and miles: drawing and map units.
    Ft,
    Yd,
    Km,
    Mi,
}

impl Unit {
    pub const ALL: [Unit; 12] = [
        Unit::Px,
        Unit::Nm,
        Unit::Um,
        Unit::Mm,
        Unit::Cm,
        Unit::M,
        Unit::Inch,
        Unit::Arcsec,
        Unit::Ft,
        Unit::Yd,
        Unit::Km,
        Unit::Mi,
    ];

    /// The symbol shown with values.
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Px => "px",
            Unit::Nm => "nm",
            Unit::Um => "µm",
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::M => "m",
            Unit::Inch => "in",
            Unit::Arcsec => "arcsec",
            Unit::Ft => "ft",
            Unit::Yd => "yd",
            Unit::Km => "km",
            Unit::Mi => "mi",
        }
    }

    /// An ASCII identifier for files (`um` for micrometres).
    pub fn key(self) -> &'static str {
        match self {
            Unit::Um => "um",
            u => u.symbol(),
        }
    }

    pub fn from_key(key: &str) -> Option<Unit> {
        match key {
            "µm" | "μm" | "micron" | "microns" => Some(Unit::Um),
            "pixel" | "pixels" => Some(Unit::Px),
            "inch" | "inches" | "\"" => Some(Unit::Inch),
            "foot" | "feet" | "'" => Some(Unit::Ft),
            "yard" | "yards" => Some(Unit::Yd),
            "mile" | "miles" => Some(Unit::Mi),
            "millimetre" | "millimetres" | "millimeter" | "millimeters" => Some(Unit::Mm),
            "centimetre" | "centimetres" | "centimeter" | "centimeters" => Some(Unit::Cm),
            "metre" | "metres" | "meter" | "meters" => Some(Unit::M),
            "kilometre" | "kilometres" | "kilometer" | "kilometers" => Some(Unit::Km),
            k => Unit::ALL.into_iter().find(|u| u.key() == k),
        }
    }

    /// Metres per unit, for the metric units and inches.
    pub fn metres(self) -> Option<f64> {
        match self {
            Unit::Nm => Some(1e-9),
            Unit::Um => Some(1e-6),
            Unit::Mm => Some(1e-3),
            Unit::Cm => Some(1e-2),
            Unit::M => Some(1.0),
            Unit::Inch => Some(0.0254),
            Unit::Ft => Some(0.3048),
            Unit::Yd => Some(0.9144),
            Unit::Km => Some(1e3),
            Unit::Mi => Some(1609.344),
            Unit::Px | Unit::Arcsec => None,
        }
    }
}

/// What a quantity measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Length,
    Area,
    /// Degrees.
    Angle,
}

/// A measured value with its unit and the calibration it went through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub kind: Kind,
    /// The length unit (squared for areas; unused for angles).
    pub unit: Unit,
    pub source: Source,
}

impl Quantity {
    /// The unit as shown: `mm`, `mm²`, `°`.
    pub fn unit_text(&self) -> String {
        match self.kind {
            Kind::Length => self.unit.symbol().to_owned(),
            Kind::Area => format!("{}²", self.unit.symbol()),
            Kind::Angle => "°".to_owned(),
        }
    }
}

impl fmt::Display for Quantity {
    /// Four significant digits (at least one decimal for angles).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = format_sig(self.value, 4);
        match self.kind {
            Kind::Angle => write!(f, "{v}°"),
            _ => write!(f, "{v} {}", self.unit_text()),
        }
    }
}

/// `v` with `sig` significant digits, without exponent for everyday magnitudes.
pub fn format_sig(v: f64, sig: i32) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    if v == 0.0 {
        return "0".to_owned();
    }
    let mag = v.abs().log10().floor() as i32;
    if !(-4..=9).contains(&mag) {
        return format!("{:.*e}", (sig - 1).max(0) as usize, v);
    }
    let decimals = (sig - 1 - mag).max(0) as usize;
    format!("{v:.decimals$}")
}

/// Why a calibration could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibError {
    /// The two points of a known length coincide.
    ZeroLength,
    /// A length, spacing or resolution that is not a positive finite number.
    NotPositive,
    /// Pixels are not a physical unit.
    PixelUnit,
}

impl fmt::Display for CalibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CalibError::ZeroLength => "the two points coincide",
            CalibError::NotPositive => "not a positive number",
            CalibError::PixelUnit => "pixels are not a physical unit",
        })
    }
}

impl std::error::Error for CalibError {}

/// A linear map from pixel displacements to physical ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    /// `[[a, b], [c, d]]`: physical `(a·dx + b·dy, c·dx + d·dy)`.
    pub m: [[f64; 2]; 2],
    pub unit: Unit,
    pub source: Source,
}

impl Default for Calibration {
    fn default() -> Self {
        Calibration::uncalibrated()
    }
}

fn positive(v: f64) -> Result<f64, CalibError> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(CalibError::NotPositive)
    }
}

impl Calibration {
    /// Pixels.
    pub const fn uncalibrated() -> Calibration {
        Calibration {
            m: [[1.0, 0.0], [0.0, 1.0]],
            unit: Unit::Px,
            source: Source::Uncalibrated,
        }
    }

    /// Rectangular pixels: `x` units between columns, `y` between rows.
    pub fn spacing(x: f64, y: f64, unit: Unit, source: Source) -> Result<Calibration, CalibError> {
        let (x, y) = (positive(x)?, positive(y)?);
        if unit == Unit::Px && source != Source::Uncalibrated {
            return Err(CalibError::PixelUnit);
        }
        Ok(Calibration {
            m: [[x, 0.0], [0.0, y]],
            unit,
            source,
        })
    }

    /// From a DICOM spacing pair as stored, `(row spacing, column spacing)` in millimetres
    /// (Pixel Spacing, Imager Pixel Spacing and Pixel Measures all store rows first). `source`
    /// is one of the DICOM sources.
    pub fn dicom(row_mm: f64, col_mm: f64, source: Source) -> Result<Calibration, CalibError> {
        Calibration::spacing(col_mm, row_mm, Unit::Mm, source)
    }

    /// Imager Pixel Spacing `(row, column)` in mm, corrected by the Estimated Radiographic
    /// Magnification Factor.
    pub fn dicom_magnified(
        row_mm: f64,
        col_mm: f64,
        factor: f64,
    ) -> Result<Calibration, CalibError> {
        let f = positive(factor)?;
        Calibration::dicom(row_mm / f, col_mm / f, Source::DicomMagnification)
    }

    /// From TIFF or EXIF resolution: pixels per `unit` (an inch or a centimetre, as
    /// ResolutionUnit 2 and 3 say). Shown in the most readable metric unit.
    pub fn resolution(
        x_per_unit: f64,
        y_per_unit: f64,
        unit: Unit,
        source: Source,
    ) -> Result<Calibration, CalibError> {
        let (x, y) = (positive(x_per_unit)?, positive(y_per_unit)?);
        let metres = unit.metres().ok_or(CalibError::PixelUnit)?;
        let mm = metres * 1e3;
        Ok(Calibration::spacing(mm / x, mm / y, Unit::Mm, source)?.readable())
    }

    /// From OME-XML PhysicalSizeX / PhysicalSizeY (units per pixel).
    pub fn ome(x: f64, y: f64, unit: Unit) -> Result<Calibration, CalibError> {
        Calibration::spacing(x, y, unit, Source::OmePhysicalSize)
    }

    /// From a FITS CD matrix (degrees per pixel). Lengths and areas are on the tangent plane at
    /// the reference pixel, accurate for fields of a few degrees. A decoder that stores rows
    /// bottom-up (FITS's +y is up) flips the sign of the second column, which changes no
    /// length, area or unsigned angle.
    pub fn fits_cd(cd: [[f64; 2]; 2]) -> Result<Calibration, CalibError> {
        let m = cd.map(|r| r.map(|v| v * 3600.0));
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        if !det.is_finite() || det == 0.0 {
            return Err(CalibError::NotPositive);
        }
        Ok(Calibration {
            m,
            unit: Unit::Arcsec,
            source: Source::FitsWcs,
        })
    }

    /// From FITS CDELT1 / CDELT2 (degrees per pixel) and CROTA2 (degrees).
    pub fn fits_cdelt(cdelt1: f64, cdelt2: f64, crota2: f64) -> Result<Calibration, CalibError> {
        let (s, c) = crota2.to_radians().sin_cos();
        Calibration::fits_cd([[cdelt1 * c, -cdelt2 * s], [cdelt1 * s, cdelt2 * c]])
    }

    /// Square pixels from a known length: the segment `a`–`b` is `length` `unit`s long.
    pub fn from_known_length(
        a: P,
        b: P,
        length: f64,
        unit: Unit,
    ) -> Result<Calibration, CalibError> {
        Calibration::uncalibrated().rescaled(a, b, length, unit)
    }

    /// This calibration's pixel shape (aspect, rotation) scaled so that `a`–`b` is `length`
    /// `unit`s: a user calibration that keeps known non-square pixels.
    pub fn rescaled(&self, a: P, b: P, length: f64, unit: Unit) -> Result<Calibration, CalibError> {
        let length = positive(length)?;
        if unit == Unit::Px {
            return Err(CalibError::PixelUnit);
        }
        let now = self.map(b.x - a.x, b.y - a.y).hypot();
        if now == 0.0 || !now.is_finite() {
            return Err(CalibError::ZeroLength);
        }
        let k = length / now;
        Ok(Calibration {
            m: self.m.map(|r| r.map(|v| v * k)),
            unit,
            source: Source::User,
        })
    }

    /// The same calibration expressed in another unit (metric and inches only).
    pub fn in_unit(&self, unit: Unit) -> Option<Calibration> {
        if unit == self.unit {
            return Some(*self);
        }
        let k = self.unit.metres()? / unit.metres()?;
        Some(Calibration {
            m: self.m.map(|r| r.map(|v| v * k)),
            unit,
            ..*self
        })
    }

    /// In the metric unit where a pixel measures at least 0.01 of it (mm for 300 dpi scans, µm
    /// for microscopy). Non-metric units are kept (feet, yards and miles too: a drawing's
    /// own unit).
    pub fn readable(&self) -> Calibration {
        if self.unit.metres().is_none() || matches!(self.unit, Unit::Ft | Unit::Yd | Unit::Mi) {
            return *self;
        }
        let px = self.pixel_size();
        let metres = px * self.unit.metres().unwrap_or(1.0);
        for u in [Unit::M, Unit::Mm, Unit::Um, Unit::Nm] {
            if metres / u.metres().unwrap_or(1.0) >= 0.01 || u == Unit::Nm {
                return self.in_unit(u).unwrap_or(*self);
            }
        }
        *self
    }

    pub fn is_calibrated(&self) -> bool {
        self.source != Source::Uncalibrated
    }

    /// The physical displacement of a pixel displacement.
    pub fn map(&self, dx: f64, dy: f64) -> P {
        P::new(
            self.m[0][0] * dx + self.m[0][1] * dy,
            self.m[1][0] * dx + self.m[1][1] * dy,
        )
    }

    /// Physical units per pixel along x and along y.
    pub fn axis_spacing(&self) -> (f64, f64) {
        (self.map(1.0, 0.0).hypot(), self.map(0.0, 1.0).hypot())
    }

    /// The side of a square of a pixel's physical area.
    pub fn pixel_size(&self) -> f64 {
        self.pixel_area().sqrt()
    }

    /// The physical area of one pixel.
    pub fn pixel_area(&self) -> f64 {
        (self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0]).abs()
    }

    /// The physical length of the segment `a`–`b`.
    pub fn distance(&self, a: P, b: P) -> f64 {
        self.map(b.x - a.x, b.y - a.y).hypot()
    }

    pub fn length(&self, value: f64) -> Quantity {
        Quantity {
            value,
            kind: Kind::Length,
            unit: self.unit,
            source: self.source,
        }
    }

    pub fn area(&self, value: f64) -> Quantity {
        Quantity {
            value,
            kind: Kind::Area,
            unit: self.unit,
            source: self.source,
        }
    }

    pub fn angle(&self, degrees: f64) -> Quantity {
        Quantity {
            value: degrees,
            kind: Kind::Angle,
            unit: self.unit,
            source: self.source,
        }
    }
}
