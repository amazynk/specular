//! Printing a finished PDF.
//!
//! The viewer lays out and rasterises pages itself (`specular-print`); this module only hands the
//! PDF to the system:
//!
//! * Linux and the BSDs: the XDG Print portal (`org.freedesktop.portal.Print`): `PreparePrint`
//!   shows the desktop's own print dialog (printer, copies, paper), `Print` then passes the PDF
//!   as a file descriptor, so it works in sandboxes too. Only where there is no portal at all
//!   (no session bus, or no backend implementing the interface), and `lp` is installed, the file
//!   goes to CUPS's default printer through `lp`; the viewer's own dialog was the confirmation.
//! * Windows and macOS: not yet ([`Error::Unsupported`]); the viewer still saves PDFs there.

use std::fmt;
use std::path::{Path, PathBuf};

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(test)]
mod tests;

/// Paper as the system's print dialog knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Paper {
    /// PWG 5101.1 self-describing name, e.g. `iso_a4_210x297mm`; empty for "custom".
    pub pwg: String,
    /// Shown in the dialog, e.g. `A4`.
    pub display: String,
    pub width_mm: f64,
    pub height_mm: f64,
}

/// What to print.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// The job's name in the print queue (usually the folder or file name).
    pub title: String,
    /// The initial number of copies; the system dialog may change it.
    pub copies: u32,
    pub paper: Paper,
    pub landscape: bool,
}

/// How the job left the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Printed {
    /// Through the print portal (the desktop's print dialog was shown).
    Portal,
    /// Through `lp` to the default CUPS printer.
    Lp(PathBuf),
}

/// A print failure.
#[derive(Debug)]
pub enum Error {
    /// This platform cannot print yet.
    Unsupported,
    /// No print portal and no `lp`.
    NoPrinting,
    /// The user cancelled the system print dialog.
    Cancelled,
    /// D-Bus or the portal failed.
    Bus(String),
    Io(std::io::Error),
    /// `lp` failed.
    Tool(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported => f.write_str("printing is not supported on this platform yet"),
            Error::NoPrinting => f.write_str("no print portal and no lp command"),
            Error::Cancelled => f.write_str("cancelled"),
            Error::Bus(e) => write!(f, "print portal: {e}"),
            Error::Io(e) => e.fmt(f),
            Error::Tool(e) => write!(f, "lp: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Whether [`print`] has a way to print at all (to enable the Print button; cheap, no D-Bus
/// round trip: a session bus counts, whether its portal can print is only known by trying).
pub fn available() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux::available()
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        false
    }
}

/// Prints `pdf` (blocks until the system dialog is answered and the job is handed over; call
/// off the UI thread). The file may be removed once this returns.
pub fn print(pdf: &Path, job: &Job) -> Result<Printed, Error> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux::print(pdf, job)
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = (pdf, job);
        Err(Error::Unsupported)
    }
}

/// Whether the printer can print to the paper's edge, as far as the system says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Borderless {
    /// The default printer lists borderless paper sizes or a borderless option.
    Yes,
    /// It lists its paper sizes and none is borderless.
    No,
    /// Nothing was reported (no printer, no CUPS, or a platform that does not say).
    Unknown,
}

/// Whether the default printer reports borderless printing. Blocks on a CUPS query (call
/// off the UI thread). Linux and the BSDs ask CUPS (`lpoptions -l`); elsewhere, and when
/// that fails, [`Borderless::Unknown`]: never guessed.
pub fn borderless() -> Borderless {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux::borderless()
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        Borderless::Unknown
    }
}

/// Reads `lpoptions -l`: lines like `PageSize/Media Size: *A4 A4.Borderless Letter`, or a
/// driver option such as `Borderless/Borderless: *False True` or `FullBleed/…`.
pub fn borderless_from_lpoptions(text: &str) -> Borderless {
    let mut sizes = false;
    for line in text.lines() {
        let Some((key, values)) = line.split_once(':') else {
            continue;
        };
        let key = key.split('/').next().unwrap_or("").trim();
        let values: Vec<&str> = values
            .split_whitespace()
            .map(|v| v.trim_start_matches('*'))
            .collect();
        match key {
            "PageSize" | "PageRegion" | "media" => {
                sizes |= !values.is_empty();
                let edge = |v: &str| {
                    let l = v.to_ascii_lowercase();
                    l.contains("borderless") || l.ends_with(".fb") || l.contains("fullbleed")
                };
                if values.iter().any(|v| edge(v)) {
                    return Borderless::Yes;
                }
            }
            "Borderless" | "FullBleed" | "CNIJBorderless" | "EPBorderless" => {
                let on = |v: &str| {
                    matches!(
                        v.to_ascii_lowercase().as_str(),
                        "true" | "on" | "yes" | "1" | "borderless"
                    )
                };
                if values.iter().any(|v| on(v)) {
                    return Borderless::Yes;
                }
            }
            _ => {}
        }
    }
    if sizes {
        Borderless::No
    } else {
        Borderless::Unknown
    }
}

/// `program` on `PATH`, if it is there.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
pub(crate) fn find_program(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(program))
        .find(|p| p.is_file())
}
