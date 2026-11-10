//! OpenColorIO v2 colour management, read in-tree: a config file (`.ocio`, YAML) and the
//! display/view transforms it describes, run on the CPU as a reference ([`Processor`]) and
//! baked into a log2 shaper and a 3D table for the GPU ([`bake`]).
//!
//! What is read: colour spaces (scene- and display-referred, aliases, `isdata`), roles,
//! displays with their views and shared views, view transforms, and the transforms they
//! chain: group, matrix, exponent (with linear segment), log (plain, affine, camera),
//! range, ASC CDL, colour-space, file (`.cube`, `.spi1d`, `.spi3d`, CLF/CTF with matrix,
//! 1D and 3D tables, range, exponent, log and ASC CDL nodes) and the built-in transforms
//! of the ACES 1.3 configs (ACEScg, ACEScc, ACEScct, the ACES 1.0 SDR video output
//! transform from the Academy's published CTL, and the sRGB, Display P3, Rec.1886 and
//! gamma 2.6 displays). Anything else is refused by name ([`Error`]): HDR output
//! transforms, ACES 2.0, looks, context variables in file names beyond `$SHOT`-free paths.

pub mod builtin;
pub mod config;
pub mod files;
pub mod ops;
pub mod processor;
pub mod yaml;

pub use config::{Config, View};
pub use processor::{Baked, Processor, bake};

/// Why a config or a transform cannot be used, in plain words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The file is not readable as an OCIO config.
    Config(String),
    /// A colour space, display or view the request names is not in the config.
    Missing(String),
    /// A transform the config uses is not supported here.
    Unsupported(String),
    /// A LUT file is missing or unreadable.
    File(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Config(s) | Error::Missing(s) | Error::Unsupported(s) | Error::File(s) => {
                f.write_str(s)
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
