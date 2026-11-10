//! Plate solving through a solver the user installed: ASTAP (`astap`, `astap_cli`) or
//! astrometry.net (`solve-field`). Nothing is bundled or downloaded; this module gives the
//! command line for one image and reads the solution back as a FITS header, which the caller
//! keeps beside the image (the original file is never written).
//!
//! - ASTAP: `-f IMAGE -o OUT/solution -wcs` writes `solution.ini` (`PLTSOLVD=T` or `F`, the
//!   solution's keywords, `ERROR=`) and, when solved, `solution.wcs` (a FITS header).
//! - astrometry.net: `solve-field --no-plots --new-fits none --dir OUT --out solution IMAGE`
//!   writes `solution.wcs` and `solution.solved` when solved.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use specular_fits::wcs::Wcs;
use specular_fits::{Header, Value};

/// A plate solver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Solver {
    Astap,
    AstrometryNet,
}

/// The base name of the solution files in the run's folder.
pub const BASE: &str = "solution";
/// The largest solution file read.
const MAX_FILE: u64 = 4 << 20;

impl Solver {
    pub const ALL: [Solver; 2] = [Solver::Astap, Solver::AstrometryNet];

    /// A stable name for settings and logs.
    pub fn key(self) -> &'static str {
        match self {
            Solver::Astap => "astap",
            Solver::AstrometryNet => "astrometry-net",
        }
    }

    /// Its name for people.
    pub fn name(self) -> &'static str {
        match self {
            Solver::Astap => "ASTAP",
            Solver::AstrometryNet => "astrometry.net",
        }
    }

    /// Program names looked up on `PATH`, in order.
    pub fn programs(self) -> &'static [&'static str] {
        match self {
            Solver::Astap => &["astap_cli", "astap"],
            Solver::AstrometryNet => &["solve-field"],
        }
    }

    /// Where an installer puts the program when it is not on `PATH`.
    pub fn usual_places(self) -> Vec<PathBuf> {
        let mut v = Vec::new();
        match self {
            Solver::Astap => {
                if cfg!(windows) {
                    v.push(PathBuf::from(r"C:\Program Files\astap\astap_cli.exe"));
                    v.push(PathBuf::from(r"C:\Program Files\astap\astap.exe"));
                } else if cfg!(target_os = "macos") {
                    v.push(PathBuf::from(
                        "/Applications/ASTAP.app/Contents/MacOS/astap",
                    ));
                } else {
                    v.push(PathBuf::from("/opt/astap/astap_cli"));
                    v.push(PathBuf::from("/opt/astap/astap"));
                }
            }
            Solver::AstrometryNet => {
                v.push(PathBuf::from("/usr/local/astrometry/bin/solve-field"));
                v.push(PathBuf::from("/opt/homebrew/bin/solve-field"));
            }
        }
        v
    }
}

/// What is known about the field beforehand (from the image's header): it narrows the
/// search.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hint {
    /// Right ascension and declination of the centre, degrees.
    pub centre: Option<(f64, f64)>,
    /// Pixel scale, arcseconds per pixel.
    pub scale: Option<f64>,
    /// Image height in pixels (for ASTAP's field of view).
    pub height: u32,
}

/// Search radius around a hinted centre, degrees.
const RADIUS: f64 = 15.0;

fn os(s: impl Into<OsString>) -> OsString {
    s.into()
}

fn num(v: f64) -> OsString {
    os(format!("{v:.6}"))
}

/// The arguments that solve `image` into `out` (a fresh, empty folder).
pub fn args(solver: Solver, image: &Path, out: &Path, hint: &Hint) -> Vec<OsString> {
    let base = out.join(BASE);
    let mut a = Vec::new();
    match solver {
        Solver::Astap => {
            a.extend([os("-f"), image.as_os_str().to_owned()]);
            a.extend([os("-o"), base.into_os_string()]);
            a.push(os("-wcs"));
            if let Some((ra, dec)) = hint.centre {
                // Hours, and the south pole distance.
                a.extend([os("-ra"), num(ra / 15.0), os("-spd"), num(dec + 90.0)]);
                a.extend([os("-r"), num(RADIUS)]);
            } else {
                a.extend([os("-r"), num(180.0)]);
            }
            match hint.scale {
                Some(s) if hint.height > 0 => {
                    a.extend([os("-fov"), num(s * f64::from(hint.height) / 3600.0)]);
                }
                // Let ASTAP find the field of view itself.
                _ => a.extend([os("-fov"), os("0")]),
            }
        }
        Solver::AstrometryNet => {
            a.extend(
                [
                    "--no-plots",
                    "--overwrite",
                    "--no-verify",
                    "--new-fits",
                    "none",
                    "--index-xyls",
                    "none",
                    "--axy",
                    "none",
                    "--rdls",
                    "none",
                    "--match",
                    "none",
                    "--corr",
                    "none",
                    "--cpulimit",
                    "300",
                ]
                .map(os),
            );
            a.extend([os("--dir"), out.as_os_str().to_owned()]);
            a.extend([os("--temp-dir"), out.as_os_str().to_owned()]);
            a.extend([os("--out"), os(BASE)]);
            if let Some((ra, dec)) = hint.centre {
                a.extend([
                    os("--ra"),
                    num(ra),
                    os("--dec"),
                    num(dec),
                    os("--radius"),
                    num(RADIUS),
                ]);
            }
            if let Some(s) = hint.scale {
                a.extend([
                    os("--scale-units"),
                    os("arcsecperpix"),
                    os("--scale-low"),
                    num(s * 0.8),
                    os("--scale-high"),
                    num(s * 1.25),
                ]);
            }
            a.push(image.as_os_str().to_owned());
        }
    }
    a
}

/// Why a run gave no solution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The solver ran and found no match.
    NotSolved(Option<String>),
    /// It left no solution file it could be asked about.
    NoOutput,
    /// The solution file does not hold a usable WCS.
    Unreadable(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::NotSolved(None) => f.write_str("no match was found"),
            Failure::NotSolved(Some(why)) => write!(f, "no match was found ({why})"),
            Failure::NoOutput => f.write_str("the solver wrote no solution"),
            Failure::Unreadable(why) => write!(f, "the solution could not be read: {why}"),
        }
    }
}

fn read_small(p: &Path) -> Option<Vec<u8>> {
    let m = std::fs::metadata(p).ok()?;
    (m.is_file() && m.len() <= MAX_FILE)
        .then(|| std::fs::read(p).ok())
        .flatten()
}

/// A FITS header file, padded or with one card per line.
pub fn header_file(bytes: &[u8]) -> Option<Header> {
    if bytes.contains(&b'\n') {
        let mut cards = Vec::new();
        for l in bytes.split(|&b| b == b'\n') {
            let l = l.strip_suffix(b"\r").unwrap_or(l);
            let mut c = l[..l.len().min(80)].to_vec();
            c.resize(80, b' ');
            let end = c.starts_with(b"END ");
            cards.extend(c);
            if end {
                break;
            }
        }
        if !cards.chunks(80).any(|c| c.starts_with(b"END ")) {
            cards.extend(format!("{:<80}", "END").bytes());
        }
        return specular_fits::header::parse(&cards).map(|(h, _)| h);
    }
    let mut b = bytes.to_vec();
    b.resize(b.len().div_ceil(80) * 80, b' ');
    if !b.chunks(80).any(|c| c.starts_with(b"END ")) {
        b.extend(format!("{:<80}", "END").bytes());
    }
    specular_fits::header::parse(&b).map(|(h, _)| h)
}

/// ASTAP's `.ini`: `KEY=value` lines.
fn ini(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim().to_owned()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// Reads the solution a run left in `out`.
pub fn read_solution(solver: Solver, out: &Path) -> Result<Header, Failure> {
    let wcs = out.join(format!("{BASE}.wcs"));
    match solver {
        Solver::Astap => {
            let ini = read_small(&out.join(format!("{BASE}.ini")))
                .map(|b| ini(&String::from_utf8_lossy(&b)));
            if let Some(kv) = &ini {
                let get = |k: &str| kv.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
                if get("PLTSOLVD").is_some_and(|v| v.trim_matches('\'').starts_with('F')) {
                    let why = get("ERROR")
                        .or(get("WARNING"))
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty());
                    return Err(Failure::NotSolved(why));
                }
            }
            if let Some(b) = read_small(&wcs) {
                return header_file(&b).ok_or_else(|| Failure::Unreadable("no END card".into()));
            }
            match ini {
                Some(kv) if kv.iter().any(|(k, _)| k == "CRVAL1") => Ok(Header::from_pairs(
                    kv.iter()
                        .filter(|(k, _)| k.len() <= 8)
                        .map(|(k, v)| (k.as_str(), v.as_str(), "")),
                )),
                Some(_) => Err(Failure::NotSolved(None)),
                None => Err(Failure::NoOutput),
            }
        }
        Solver::AstrometryNet => match read_small(&wcs) {
            Some(b) => header_file(&b).ok_or_else(|| Failure::Unreadable("no END card".into())),
            None if out.join(format!("{BASE}.solved")).is_file() => Err(Failure::NoOutput),
            None => Err(Failure::NotSolved(None)),
        },
    }
}

/// The WCS of a solution for an image of `width × height`.
pub fn solution_wcs(h: &Header, width: usize, height: usize) -> Result<Wcs, Failure> {
    let w = Wcs::from_header(h, &[width, height])
        .ok_or_else(|| Failure::Unreadable("no world coordinates".into()))?;
    if w.celestial.is_none() {
        let why = w
            .unsupported
            .as_ref()
            .map_or_else(|| "no sky axes".to_owned(), |u| u.to_string());
        return Err(Failure::Unreadable(why));
    }
    Ok(w)
}

/// A value as a card writes it.
fn card_value(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("'{:<8}'", s.replace('\'', "''")),
        Value::Float(f) => {
            let s = format!("{f:E}");
            if s.contains('.') {
                s
            } else {
                s.replacen('E', ".0E", 1)
            }
        }
        other => other.display(),
    }
}

/// The cards of a solution worth keeping beside an image: the WCS ones (and the solver's
/// own notes), as FITS card text, 80 columns per card, ending in `END`. Written as
/// `IMAGE.wcs`, the name both solvers use.
pub fn sidecar(h: &Header) -> Vec<u8> {
    let keep = |k: &str| {
        let wcs = [
            "WCSAXES", "CTYPE", "CUNIT", "CRVAL", "CRPIX", "CDELT", "CROTA", "CD1_", "CD2_",
            "PC1_", "PC2_", "LONPOLE", "LATPOLE", "RADESYS", "EQUINOX", "A_", "B_", "AP_", "BP_",
            "PV1_", "PV2_", "IMAGEW", "IMAGEH", "NAXIS",
        ];
        wcs.iter().any(|p| k.starts_with(p)) || matches!(k, "COMMENT" | "HISTORY")
    };
    let mut out = Vec::new();
    for c in &h.cards {
        if !keep(&c.key) || c.key == "NAXIS" {
            continue;
        }
        let line = match &c.value {
            Some(v) => format!("{:<8}= {:>20}", c.key, card_value(v)),
            None => format!("{:<8}{}", c.key, c.comment),
        };
        let mut l = line.into_bytes();
        l.truncate(80);
        l.resize(80, b' ');
        out.extend(l);
    }
    out.extend(format!("{:<80}", "END").bytes());
    out.resize(out.len().div_ceil(2880) * 2880, b' ');
    out
}

#[cfg(test)]
mod tests;
