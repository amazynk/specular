//! `specular-em`: electron-microscopy readers for Specular.
//!
//! * **[`dm`]: Gatan DigitalMicrograph DM3 / DM4.** The tag tree (both versions' word widths,
//!   either data byte order); every image of the `ImageList` except the thumbnails; integer,
//!   float, boolean, complex (shown as magnitude), packed complex FFT (old and new layouts) and
//!   BGRA colour data; stacks and spectrum images as planes; per-dimension calibrations
//!   (origin, scale, units) and the intensity unit; microscope, voltage, magnification,
//!   exposure and acquisition time from the image tags.
//! * **[`mrc`]: MRC / MRCS.** MRC2014 and the older IMOD and CCP4 layouts; modes 0, 1, 2, 3,
//!   4 (complex shown as magnitude), 6, 12 (half float) and 101 (4-bit); either byte order
//!   (machine stamp, else a guess); extended headers skipped by their length, with the FEI1 /
//!   FEI2 (EPU, Velox) records read for the microscope, voltage, magnification, pixel size and
//!   exposure; the voxel size from the cell and sampling; sections as planes.
//! * **[`dv`]: DeltaVision.** The Priism header (an MRC-like layout with its own fields):
//!   wavelengths, time points and the Z/W/T section order, pixel sizes, the per-section
//!   extended header (time stamps, stage position, exposure, excitation and emission).
//! * **[`mdoc`]: SerialEM `.mdoc`.** The text sidecar beside an MRC stack: global values,
//!   titles and per-section values (tilt angle, defocus, dose, magnification, time).
//! * **[`cube`]: Gaussian cube.** The grid (Bohr or Ångström), the atoms, the orbital list;
//!   the first orbital's (or value's) volume.
//!
//! Each format parses its header first ([`dm::parse`], [`mrc::parse`]) without touching the
//! pixels; planes are then read one at a time as `f32` values in the file's row order.
//! Malformed input never panics; sizes are checked before anything is allocated.

pub mod bcf;
pub mod cube;
pub mod dataset;
pub mod dm;
pub mod dv;
pub mod mdoc;
pub mod mrc;
pub mod samples;
pub mod spider;
pub mod tia;
mod xml;

/// Largest number of samples in one plane.
pub const MAX_PLANE: u64 = 1 << 30;

/// Errors from the readers.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a DigitalMicrograph file")]
    NotDm,
    #[error("not an MRC file")]
    NotMrc,
    #[error("truncated: {0}")]
    Truncated(&'static str),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
    #[error("no image in the file")]
    NoImage,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A linear calibration of one dimension: sample `k` is at `(k - origin) * scale` `units`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calibration {
    pub origin: f64,
    pub scale: f64,
    pub units: String,
}

impl Calibration {
    /// The physical coordinate of sample index `k`.
    pub fn at(&self, k: f64) -> f64 {
        (k - self.origin) * self.scale
    }

    /// Whether this is the identity (scale 1, origin 0, no unit).
    pub fn is_identity(&self) -> bool {
        self.scale == 1.0 && self.origin == 0.0 && self.units.is_empty()
    }
}
