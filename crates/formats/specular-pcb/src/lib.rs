//! Electronics files, read with our own parsers into `specular-vector` scenes:
//!
//! * [`gerber`]: Gerber RS-274X with X2/X3 attributes: standard and macro apertures (with
//!   expressions), regions, step-and-repeat, block apertures, polarity, mirroring, rotation
//!   and scaling, linear and circular interpolation (single and multi-quadrant).
//! * [`excellon`]: Excellon drill and rout files.
//! * [`stack`]: a board's Gerber and drill files together: each file's role (X2
//!   `.FileFunction`, else the usual file names), a layer view, and the top and bottom in
//!   board colours.
//!
//! KiCad boards and schematics are `specular-kicad`'s.

pub mod excellon;
pub mod gerber;
pub mod ipc2581;
pub mod odb;
pub mod stack;

mod features;
mod geom;
mod text;

/// Why a file could not be read.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum Error {
    /// Not this format at all (the next decoder may try).
    #[error("not this format")]
    NotThisFormat,
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("{0}")]
    Malformed(String),
    /// A variant we refuse, said in plain English ("KiCad 5 boards (the legacy format) are
    /// not supported; open it in KiCad 6 or later and save it again").
    #[error("{0}")]
    Unsupported(String),
    #[error("too large")]
    TooLarge,
}

/// Limits on what one file may make.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Items (shapes) in a scene; past it the rest is left out with a warning.
    pub items: usize,
    /// Step-and-repeat copies and block aperture flashes expanded, in total.
    pub copies: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            items: 4_000_000,
            copies: 1_000_000,
        }
    }
}

/// Adds `w` to `warnings` unless it is there already (at most 64).
pub(crate) fn warn(warnings: &mut Vec<String>, w: impl Into<String>) {
    let w = w.into();
    if warnings.len() < 64 && !warnings.contains(&w) {
        warnings.push(w);
    }
}
