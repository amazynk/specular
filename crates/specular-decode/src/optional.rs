//! Optional decoders: free software from other vendors that opens a few formats, used only
//! when the user installed it themselves. Specular never bundles, redistributes or downloads
//! it; it looks where the vendors' installers put it (or where the user points), uses what it
//! finds automatically, and lets the user switch each one off.
//!
//! | Decoder | Formats | How it runs |
//! |---|---|---|
//! | Blackmagic RAW SDK | `.braw` | loaded by the sandboxed decode host (`Host::vendor`) |
//! | ODA File Converter | `.dwg` | run as a program on a private copy ([`crate::cad_dec`]) |
//! | RED R3D SDK | `.r3d` | refused: its licence (see [`OptionalId::R3d`]) |
//! | ECW/JP2 SDK (Hexagon) | `.ecw` | refused: its licence (see [`OptionalId::Ecw`]) |
//! | MrSID Decode SDK | `.sid` | refused: its licence (see [`OptionalId::MrSid`]) |
//! | DJI Thermal SDK | DJI R-JPEG temperatures | refused: its licence (see [`OptionalId::DjiThermal`]) |
//! | OpenCASCADE | STEP, IGES | the sandboxed decode host's CAD pool, through the bridge registered with [`set_occt_bridge`] |
//! | libCZI | Zeiss `.czi` | the sandboxed decode host's CZI pool ([`crate::czi_dec`]) |
//!
//! The app hands over its settings ([`configure`]), asks again when its window gains focus
//! ([`refresh`], so an install is picked up without a restart), and shows [`entries`] on
//! Settings › Optional decoders. Every change bumps [`revision`].

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

/// One optional decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptionalId {
    /// Blackmagic RAW (`.braw`) through the Blackmagic RAW SDK (6.0 or later). Its developer
    /// licence lets programs use the SDK's API libraries; Specular uses the copy the user
    /// installed with Blackmagic's own installer.
    Braw,
    /// RED R3D. Not possible: RED's R3D SDK is a static library that must be built into the
    /// program, which must then be distributed under an end-user licence that forbids
    /// redistribution (R3D SDK licence §1.4, §2), and its runtime libraries may only be
    /// installed in a folder private to that program. Specular's licence allows sharing it,
    /// and there is no copy another program could load.
    R3d,
    /// DWG drawings through the ODA File Converter.
    Dwg,
    /// STEP and IGES through OpenCASCADE (LGPL, the user's own install). The bridge that
    /// runs it registers itself with [`set_occt_bridge`]; without one the page lists it as
    /// not available yet.
    OpenCascade,
    /// Zeiss CZI microscopy files through libCZI (LGPL-3.0, the user's own install), loaded
    /// by the sandboxed decode host through Specular's bridge. Found by its library on disk
    /// ([`crate::find_czi`]).
    Czi,
    /// ECW aerial images. Not possible: Hexagon's ECW/JP2 SDK licences let only the
    /// licensee's own applications use the SDK. The Desktop Read-Only Redistributable
    /// licence grants it for Desktop Applications "that You develop", shipped with
    /// Hexagon's end-user terms (§1.2, §2.1, §3.3), and forbids any other use (§3.5); the
    /// Server End-user licence forbids use with another product (§3.7). A copy the user
    /// installed, or one inside another program, is licensed to that program only.
    Ecw,
    /// MrSID aerial images. Not possible: the MrSID Decode SDK licence covers the licensee's
    /// own development and its distribution with the licensee's products, never apart from
    /// them (§1.2, §1.3(d)), and keeps the SDK confidential (§1.4).
    MrSid,
    /// The temperatures of DJI radiometric JPEGs (R-JPEG: Zenmuse H20T/H30T/XT S, Mavic 2/3
    /// Enterprise thermal, Matrice 30T…). Not possible: the DJI Thermal SDK's `License.txt`
    /// puts only its headers and samples under MIT; `libdirp` and the `libv_*` libraries
    /// fall under DJI's SDK End User License Agreement (developer.dji.com/policies/eula,
    /// 2017-12-12), which licenses "you" to use the SDK "solely to develop, test, and
    /// operate Applications", an Application being "a software program that is developed by
    /// you using the SDK" (§2, §3.1), and forbids redistributing or sublicensing it (§3.2(c)).
    /// A copy the user downloaded covers only programs that user develops; Specular could
    /// use the SDK only by shipping its object code inside Specular (§3.3), which is ruled
    /// out (never bundled). The visible photo and the metadata still show (it is a JPEG).
    DjiThermal,
}

impl OptionalId {
    pub const ALL: [OptionalId; 8] = [
        OptionalId::Braw,
        OptionalId::R3d,
        OptionalId::Dwg,
        OptionalId::OpenCascade,
        OptionalId::Czi,
        OptionalId::Ecw,
        OptionalId::MrSid,
        OptionalId::DjiThermal,
    ];

    /// Stable key (settings, strings): "braw", "r3d", "dwg", "occt", "czi", "ecw", "mrsid",
    /// "dji-thermal".
    pub fn key(self) -> &'static str {
        match self {
            OptionalId::Braw => "braw",
            OptionalId::R3d => "r3d",
            OptionalId::Dwg => "dwg",
            OptionalId::OpenCascade => "occt",
            OptionalId::Czi => "czi",
            OptionalId::Ecw => "ecw",
            OptionalId::MrSid => "mrsid",
            OptionalId::DjiThermal => "dji-thermal",
        }
    }

    pub fn from_key(k: &str) -> Option<OptionalId> {
        Self::ALL.into_iter().find(|i| i.key() == k)
    }

    /// The format, as people call it.
    pub fn format_name(self) -> &'static str {
        match self {
            OptionalId::Braw => "Blackmagic RAW",
            OptionalId::R3d => "RED R3D",
            OptionalId::Dwg => "DWG",
            OptionalId::OpenCascade => "STEP and IGES",
            OptionalId::Czi => "Zeiss CZI",
            OptionalId::Ecw => "ECW",
            OptionalId::MrSid => "MrSID",
            OptionalId::DjiThermal => "DJI R-JPEG",
        }
    }

    /// Who makes the decoder.
    pub fn vendor(self) -> &'static str {
        match self {
            OptionalId::Braw => "Blackmagic Design",
            OptionalId::R3d => "RED",
            OptionalId::Dwg => "Open Design Alliance",
            OptionalId::OpenCascade => "Open Cascade",
            OptionalId::Czi => "ZEISS",
            OptionalId::Ecw => "Hexagon",
            OptionalId::MrSid => "Extensis",
            OptionalId::DjiThermal => "DJI",
        }
    }

    /// The decoder's own name.
    pub fn decoder_name(self) -> &'static str {
        match self {
            OptionalId::Braw => "Blackmagic RAW SDK",
            OptionalId::R3d => "R3D SDK",
            OptionalId::Dwg => "ODA File Converter",
            OptionalId::OpenCascade => "OpenCASCADE",
            OptionalId::Czi => "libCZI",
            OptionalId::Ecw => "ECW/JP2 SDK",
            OptionalId::MrSid => "MrSID Decode SDK",
            OptionalId::DjiThermal => "DJI Thermal SDK",
        }
    }

    /// File extensions it opens (lower case).
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            OptionalId::Braw => &["braw"],
            OptionalId::R3d => &["r3d"],
            OptionalId::Dwg => &["dwg"],
            OptionalId::OpenCascade => &["stp", "step", "igs", "iges"],
            OptionalId::Czi => &["czi"],
            OptionalId::Ecw => &["ecw"],
            OptionalId::MrSid => &["sid"],
            // R-JPEGs are `.jpg` files, told apart by their content.
            OptionalId::DjiThermal => &[],
        }
    }

    /// The vendor's official download page ("Get it"), when the user can get it.
    pub fn download_url(self) -> Option<&'static str> {
        match self {
            OptionalId::Braw => Some("https://www.blackmagicdesign.com/blackmagicrawinstaller"),
            OptionalId::Dwg => Some("https://www.opendesign.com/guestfiles/oda_file_converter"),
            OptionalId::OpenCascade => Some("https://dev.opencascade.org/release"),
            OptionalId::Czi => Some("https://github.com/ZEISS/libczi"),
            OptionalId::R3d | OptionalId::Ecw | OptionalId::MrSid | OptionalId::DjiThermal => None,
        }
    }

    /// Whether Specular can use it at all (else it is listed with the reason).
    pub fn usable(self) -> bool {
        match self {
            OptionalId::Braw | OptionalId::Dwg => true,
            // Only where this build has the libCZI bridge (Linux and macOS builds made with
            // libCZI's headers); without it the file says so.
            OptionalId::Czi => true,
            OptionalId::OpenCascade => OCCT.read().is_some(),
            OptionalId::R3d | OptionalId::Ecw | OptionalId::MrSid | OptionalId::DjiThermal => false,
        }
    }

    /// The optional decoder for a file name's extension.
    pub fn for_path(path: &Path) -> Option<OptionalId> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|i| i.extensions().contains(&ext.as_str()))
    }
}

impl std::fmt::Display for OptionalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.format_name())
    }
}

/// What the user chose (the app's settings).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// Use the Blackmagic RAW SDK when found (on by default).
    pub braw: bool,
    /// The SDK picked by hand (its library or folder); else it is looked for.
    pub braw_path: Option<PathBuf>,
    /// Use the ODA File Converter when found (on by default).
    pub dwg: bool,
    pub dwg_path: Option<PathBuf>,
    /// Use OpenCASCADE when found (on by default).
    pub occt: bool,
    /// A folder holding OpenCASCADE's libraries, when they are not where the system looks.
    pub occt_path: Option<PathBuf>,
    /// Use libCZI when found (on by default).
    pub czi: bool,
    /// A folder holding libCZI's library, when it is not where the system looks.
    pub czi_path: Option<PathBuf>,
    /// The app's cache folder (DWG conversions work in a private folder under it).
    pub cache: PathBuf,
}

/// Where an optional decoder stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Found (or picked by hand) and used.
    Ready {
        path: PathBuf,
        /// Once known (the SDK reports it when the decode host loads it).
        version: Option<String>,
    },
    /// Found, but switched off.
    Off {
        path: PathBuf,
        version: Option<String>,
    },
    /// Found or picked, but it does not work (why, in the decoder's words).
    Broken { path: PathBuf, why: String },
    /// Not on this computer.
    NotFound,
    /// Can't be supported (its licence): [`OptionalId::R3d`].
    Refused,
    /// Not supported yet: [`OptionalId::OpenCascade`] without its bridge.
    Planned,
    /// Being looked for (OpenCASCADE: the decode host is asked in the background).
    Checking,
}

/// What the OpenCASCADE bridge found: its libraries' folder and version, or why not.
pub type OcctFound = Result<(PathBuf, String), String>;

/// How OpenCASCADE is run (`occt_dec`), registered by it with [`set_occt_bridge`]. Finding
/// it means loading it, which only the decode host may do: `probe` asks a host (it may take
/// a second) and answers the libraries' folder and OpenCASCADE's version, or why not.
#[derive(Clone, Copy)]
pub struct OcctBridge {
    /// Applies the settings (on or off, the folder picked by hand).
    pub apply: fn(enabled: bool, library_dir: Option<PathBuf>),
    pub probe: fn(library_dir: Option<&Path>) -> OcctFound,
}

static OCCT: parking_lot::RwLock<Option<OcctBridge>> = parking_lot::RwLock::new(None);

/// Registers the OpenCASCADE bridge (once, at start-up): it is then listed, looked for and
/// applied like the other optional decoders.
pub fn set_occt_bridge(b: OcctBridge) {
    *OCCT.write() = Some(b);
    bump();
    let s = MODEL.lock().settings.clone();
    if let Some(s) = s {
        (b.apply)(s.occt, s.occt_path.clone());
        probe_occt(&s);
    }
}

/// Asks the bridge in the background where OpenCASCADE is (unless asked already for this
/// folder, or asking now).
fn probe_occt(s: &Settings) {
    let Some(b) = *OCCT.read() else {
        return;
    };
    let dir = s.occt_path.clone();
    {
        let mut m = MODEL.lock();
        let known = m.occt_probe.as_ref().is_some_and(|(d, _)| *d == dir);
        if known || m.occt_probing {
            return;
        }
        m.occt_probing = true;
    }
    specular_thread::spawn("occt-probe", move || {
        // A panic is the probe's answer (shown as why), never "probing" forever.
        let r = specular_core::isolate::run("OpenCASCADE probe", || (b.probe)(dir.as_deref()))
            .unwrap_or_else(|p| Err(p.brief()));
        let mut m = MODEL.lock();
        m.occt_probing = false;
        m.occt_probe = Some((dir, r));
        drop(m);
        bump();
    });
}

impl State {
    pub fn path(&self) -> Option<&Path> {
        match self {
            State::Ready { path, .. } | State::Off { path, .. } | State::Broken { path, .. } => {
                Some(path)
            }
            _ => None,
        }
    }

    pub fn version(&self) -> Option<&str> {
        match self {
            State::Ready { version, .. } | State::Off { version, .. } => version.as_deref(),
            _ => None,
        }
    }
}

/// One row of Settings › Optional decoders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: OptionalId,
    pub state: State,
    /// The user picked the path by hand (Locate…).
    pub picked: bool,
    /// The "use it" switch.
    pub enabled: bool,
}

#[derive(Default)]
struct Model {
    settings: Option<Settings>,
    found_braw: Option<PathBuf>,
    found_dwg: Option<PathBuf>,
    /// What the decode host said about the SDK at `found_braw`: its version or why not.
    braw_probe: Option<(PathBuf, Result<String, String>)>,
    /// A probe of this path is running.
    probing: Option<PathBuf>,
    /// What the OpenCASCADE bridge answered for this folder setting.
    occt_probe: Option<(Option<PathBuf>, OcctFound)>,
    occt_probing: bool,
    /// libCZI's folder and version from its file name, or why not (a look on disk).
    found_czi: Option<OcctFound>,
}

static MODEL: Mutex<Model> = Mutex::new(Model {
    settings: None,
    found_braw: None,
    found_dwg: None,
    braw_probe: None,
    probing: None,
    occt_probe: None,
    occt_probing: false,
    found_czi: None,
});
static REVISION: AtomicU64 = AtomicU64::new(0);

/// Bumped by every change of what [`entries`] returns.
pub fn revision() -> u64 {
    REVISION.load(Ordering::Acquire)
}

fn bump() {
    REVISION.fetch_add(1, Ordering::AcqRel);
}

/// Takes the app's settings, looks for the decoders, and applies what it found. Returns
/// whether anything changed.
pub fn configure(s: Settings) -> bool {
    let changed = {
        let mut m = MODEL.lock();
        let same = m.settings.as_ref() == Some(&s);
        m.settings = Some(s);
        !same
    };
    refresh() || changed
}

/// Looks for the decoders again (the window gained focus: something may have been installed
/// or removed) and applies the result. Returns whether anything changed.
pub fn refresh() -> bool {
    let Some(s) = MODEL.lock().settings.clone() else {
        return false;
    };
    let braw = find_braw(s.braw_path.as_deref());
    let dwg = crate::find_oda_converter(s.dwg_path.as_deref());
    let czi = Some(crate::find_czi(s.czi_path.as_deref()));
    let changed = {
        let mut m = MODEL.lock();
        let changed = m.found_braw != braw || m.found_dwg != dwg || m.found_czi != czi;
        m.found_braw = braw.clone();
        m.found_dwg = dwg;
        m.found_czi = czi;
        // OpenCASCADE not found (or another folder now): ask again.
        if m.occt_probe
            .as_ref()
            .is_some_and(|(d, r)| r.is_err() || *d != s.occt_path)
        {
            m.occt_probe = None;
        }
        changed
    };
    apply(&s, braw);
    probe_occt(&s);
    if changed {
        bump();
    }
    changed
}

/// Hands the found decoders to whatever runs them.
fn apply(s: &Settings, braw: Option<PathBuf>) {
    crate::set_dwg_converter(Some(crate::DwgConverter {
        enabled: s.dwg,
        program: s.dwg_path.clone(),
        cache: s.cache.clone(),
    }));
    if let Some(b) = *OCCT.read() {
        (b.apply)(s.occt, s.occt_path.clone());
    }
    crate::set_czi(Some(crate::CziConfig {
        enabled: s.czi,
        library_dir: s.czi_path.clone(),
    }));
    let libs = specular_sandbox::VendorLibs {
        braw: braw.filter(|_| s.braw),
    };
    specular_sandbox::Host::vendor().set_vendor_libs(libs);
}

/// The SDK library the decode host should load, when the Blackmagic RAW SDK is found and on.
pub fn braw_library() -> Option<PathBuf> {
    let m = MODEL.lock();
    let on = m.settings.as_ref().is_some_and(|s| s.braw);
    m.found_braw.clone().filter(|_| on)
}

/// Records what the decode host said about the SDK at `path` (a decode or a probe did it).
pub fn note_braw(path: &Path, r: Result<String, String>) {
    let mut m = MODEL.lock();
    if m.braw_probe.as_ref().map(|(p, r)| (p.as_path(), r)) != Some((path, &r)) {
        m.braw_probe = Some((path.to_path_buf(), r));
        drop(m);
        bump();
    }
}

/// Starts the decode host in the background to learn the SDK's version (Settings shows it),
/// once per found library. [`revision`] changes when it answers.
pub fn probe_versions() {
    let path = {
        let mut m = MODEL.lock();
        let Some(p) = m.found_braw.clone() else {
            return;
        };
        let on = m.settings.as_ref().is_some_and(|s| s.braw);
        let known = m.braw_probe.as_ref().is_some_and(|(q, _)| *q == p);
        if !on || known || m.probing.as_ref() == Some(&p) {
            return;
        }
        m.probing = Some(p.clone());
        p
    };
    specular_thread::spawn("braw-probe", move || {
        let r = specular_core::isolate::run("Blackmagic RAW probe", || {
            match specular_sandbox::Host::vendor().info() {
                Ok(i) => match i.braw {
                    Some(v) => Ok(v),
                    None if i.braw_error.is_empty() => Err("not loaded".to_owned()),
                    None => Err(i.braw_error),
                },
                Err(e) => Err(e.to_string()),
            }
        })
        .unwrap_or_else(|p| Err(p.brief()));
        MODEL.lock().probing = None;
        note_braw(&path, r);
    });
}

/// Every optional decoder and where it stands, in display order.
pub fn entries() -> Vec<Entry> {
    OptionalId::ALL.into_iter().map(entry).collect()
}

/// Where `id` stands.
pub fn state(id: OptionalId) -> State {
    entry(id).state
}

fn entry(id: OptionalId) -> Entry {
    let m = MODEL.lock();
    let s = m.settings.clone().unwrap_or_default();
    let (found, enabled, picked) = match id {
        OptionalId::Braw => (m.found_braw.clone(), s.braw, s.braw_path.is_some()),
        OptionalId::Dwg => (m.found_dwg.clone(), s.dwg, s.dwg_path.is_some()),
        OptionalId::R3d | OptionalId::Ecw | OptionalId::MrSid | OptionalId::DjiThermal => {
            return Entry {
                id,
                state: State::Refused,
                picked: false,
                enabled: false,
            };
        }
        OptionalId::OpenCascade => {
            let state = if OCCT.read().is_none() {
                State::Planned
            } else {
                match &m.occt_probe {
                    None => State::Checking,
                    Some((_, Ok((path, v)))) => {
                        let (path, version) = (path.clone(), Some(v.clone()));
                        if s.occt {
                            State::Ready { path, version }
                        } else {
                            State::Off { path, version }
                        }
                    }
                    Some((Some(dir), Err(why))) => State::Broken {
                        path: dir.clone(),
                        why: why.clone(),
                    },
                    Some((None, Err(_))) => State::NotFound,
                }
            };
            return Entry {
                id,
                state,
                picked: s.occt_path.is_some(),
                enabled: s.occt,
            };
        }
        OptionalId::Czi => {
            let state = match &m.found_czi {
                None => State::Checking,
                Some(Ok((path, v))) => {
                    let (path, version) = (path.clone(), Some(v.clone()));
                    if s.czi {
                        State::Ready { path, version }
                    } else {
                        State::Off { path, version }
                    }
                }
                Some(Err(why)) => match &s.czi_path {
                    Some(dir) => State::Broken {
                        path: dir.clone(),
                        why: why.clone(),
                    },
                    None => State::NotFound,
                },
            };
            return Entry {
                id,
                state,
                picked: s.czi_path.is_some(),
                enabled: s.czi,
            };
        }
    };
    let picked_path = match id {
        OptionalId::Braw => s.braw_path.clone(),
        _ => s.dwg_path.clone(),
    };
    let state = match found {
        None => match picked_path {
            // Picked by hand, but nothing usable there.
            Some(p) => State::Broken {
                why: format!("no {} there", id.decoder_name()),
                path: p,
            },
            None => State::NotFound,
        },
        Some(path) => {
            let (version, broken) = match id {
                OptionalId::Braw => match &m.braw_probe {
                    Some((p, Ok(v))) if *p == path => (Some(v.clone()), None),
                    Some((p, Err(e))) if *p == path => (None, Some(e.clone())),
                    _ => (None, None),
                },
                _ => (oda_version(&path), None),
            };
            match (broken, enabled) {
                (Some(why), true) => State::Broken { path, why },
                (_, true) => State::Ready { path, version },
                (_, false) => State::Off { path, version },
            }
        }
    };
    Entry {
        id,
        state,
        picked,
        enabled,
    }
}

/// The optional decoder a file needs and that is not on this computer (or not usable where
/// the user pointed): the gallery's "Needs decoder" badge and the viewer's card. None for
/// formats that open, for a decoder the user switched off (the file's error says so), and
/// for formats no optional decoder can open (RED R3D, STEP/IGES for now).
pub fn needs_decoder(path: &Path) -> Option<OptionalId> {
    let id = OptionalId::for_path(path)?;
    if !id.usable() {
        return None;
    }
    match state(id) {
        State::NotFound | State::Broken { .. } => Some(id),
        _ => None,
    }
}

/// "25.4.0" from an ODA File Converter installed in "ODAFileConverter 25.4.0".
fn oda_version(program: &Path) -> Option<String> {
    let dir = program.parent()?.file_name()?.to_str()?;
    let v = dir
        .strip_prefix("ODAFileConverter")?
        .trim_start_matches([' ', '_', '-']);
    v.starts_with(|c: char| c.is_ascii_digit())
        .then(|| v.trim_end_matches(".app").to_owned())
}

// ---------------------------------------------------------------------------------------------
// Finding the Blackmagic RAW SDK

/// The library's file name on this platform (inside `BlackmagicRawAPI.framework` on macOS).
const BRAW_LIB: &str = if cfg!(windows) {
    "BlackmagicRawAPI.dll"
} else if cfg!(target_os = "macos") {
    "BlackmagicRawAPI"
} else {
    "libBlackmagicRawAPI.so"
};

/// The SDK library for a path the user picked: the library itself, a folder holding it (an
/// SDK's `Libraries` folder, the SDK's own top folder), or a macOS framework.
pub fn braw_library_in(p: &Path) -> Option<PathBuf> {
    if p.is_file() {
        let name = p.file_name()?.to_str()?;
        return name.eq_ignore_ascii_case(BRAW_LIB).then(|| p.to_path_buf());
    }
    if !p.is_dir() {
        return None;
    }
    if p.extension().is_some_and(|e| e == "framework") {
        let f = p.join(BRAW_LIB);
        return f.is_file().then_some(f);
    }
    find_below(p, 4, &mut 2000)
}

/// Where the SDK is looked for: the library the user picked, else the folders Blackmagic's
/// installers use on this platform.
pub fn find_braw(named: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = named {
        return braw_library_in(p);
    }
    for (root, depth) in braw_roots() {
        let mut budget = 2000;
        if let Some(f) = find_below(&root, depth, &mut budget) {
            return Some(f);
        }
    }
    None
}

/// Folders Blackmagic's installers put the SDK in, with how deep to look. DaVinci Resolve's
/// own copy is not used: its licence covers Resolve, not other programs (a user may still
/// pick it by hand).
fn braw_roots() -> Vec<(PathBuf, usize)> {
    let mut v: Vec<(PathBuf, usize)> = Vec::new();
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(var) {
                // Blackmagic RAW\Blackmagic RAW SDK\Win\Libraries, and the common components
                // the Blackmagic RAW installer adds for other programs.
                v.push((
                    PathBuf::from(pf)
                        .join("Blackmagic Design")
                        .join("Blackmagic RAW"),
                    4,
                ));
            }
        }
    } else if cfg!(target_os = "macos") {
        v.push(("/Library/Frameworks/BlackmagicRawAPI.framework".into(), 0));
        if let Some(h) = &home {
            v.push((h.join("Library/Frameworks/BlackmagicRawAPI.framework"), 0));
        }
        // Blackmagic RAW/Blackmagic RAW SDK/Mac/Libraries/BlackmagicRawAPI.framework
        v.push(("/Applications/Blackmagic RAW".into(), 5));
    } else {
        for r in [
            "/usr/lib64/blackmagic",
            "/usr/lib/blackmagic",
            "/opt/blackmagic",
            "/usr/local/lib/blackmagic",
        ] {
            v.push((r.into(), 4));
        }
        for r in ["/usr/local/lib", "/usr/lib64", "/usr/lib"] {
            v.push((r.into(), 0));
        }
    }
    // An SDK unpacked by hand: "Blackmagic RAW SDK" in the home or downloads folder.
    if let Some(h) = home {
        for d in [h.clone(), h.join("Downloads")] {
            if let Ok(rd) = std::fs::read_dir(&d) {
                let mut sdks: Vec<PathBuf> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_dir()
                            && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                                n.to_ascii_lowercase().starts_with("blackmagic raw")
                            })
                    })
                    .collect();
                sdks.sort();
                v.extend(sdks.into_iter().rev().map(|p| (p, 5)));
            }
        }
    }
    v
}

/// The SDK library in `dir` or below it (at most `depth` folders down, at most `budget`
/// entries looked at), skipping DaVinci Resolve and the Player's private copies. A folder's
/// own library wins over deeper ones; folders are searched in name order, newest version
/// names last so they are tried first.
fn find_below(dir: &Path, depth: usize, budget: &mut usize) -> Option<PathBuf> {
    if cfg!(target_os = "macos") && dir.extension().is_some_and(|e| e == "framework") {
        let f = dir.join(BRAW_LIB);
        return f.is_file().then_some(f);
    }
    let direct = dir.join(BRAW_LIB);
    if !cfg!(target_os = "macos") && direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    let rd = std::fs::read_dir(dir).ok()?;
    let mut subs: Vec<PathBuf> = Vec::new();
    for e in rd.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let p = e.path();
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        // Another program's private copy (Resolve, the Player app bundle), or not the SDK.
        if name.contains("resolve") || name.ends_with(".app") {
            continue;
        }
        subs.push(p);
    }
    subs.sort();
    for s in subs.into_iter().rev() {
        if let Some(f) = find_below(&s, depth - 1, budget) {
            return Some(f);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("optional-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn ids_round_trip_and_name_their_formats() {
        for id in OptionalId::ALL {
            assert_eq!(OptionalId::from_key(id.key()), Some(id));
            // DJI R-JPEGs are `.jpg` files, told apart by their content only.
            assert_eq!(id.extensions().is_empty(), id == OptionalId::DjiThermal);
        }
        assert!(!OptionalId::DjiThermal.usable());
        assert_eq!(OptionalId::DjiThermal.download_url(), None);
        assert_eq!(state(OptionalId::DjiThermal), State::Refused);
        assert_eq!(
            OptionalId::for_path(Path::new("/x/A001.BRAW")),
            Some(OptionalId::Braw)
        );
        assert_eq!(
            OptionalId::for_path(Path::new("a.r3d")),
            Some(OptionalId::R3d)
        );
        assert_eq!(OptionalId::for_path(Path::new("a.jpg")), None);
        assert!(OptionalId::Braw.usable() && !OptionalId::R3d.usable());
        assert!(OptionalId::R3d.download_url().is_none());
        for id in [OptionalId::Braw, OptionalId::Dwg] {
            assert!(id.download_url().unwrap().starts_with("https://www."));
        }
    }

    #[test]
    fn finds_the_sdk_in_an_sdk_folder_and_skips_resolve() {
        let root = tmp("find");
        // Resolve's copy (first in name order) is skipped; the SDK's is found.
        let resolve = root.join("DaVinci Resolve/libs");
        std::fs::create_dir_all(&resolve).unwrap();
        std::fs::write(resolve.join(BRAW_LIB), b"").unwrap();
        assert_eq!(find_below(&root, 4, &mut 100), None);
        let libs = if cfg!(target_os = "macos") {
            root.join("Blackmagic RAW SDK/Mac/Libraries/BlackmagicRawAPI.framework")
        } else {
            root.join("Blackmagic RAW SDK/Linux/Libraries")
        };
        std::fs::create_dir_all(&libs).unwrap();
        std::fs::write(libs.join(BRAW_LIB), b"").unwrap();
        assert_eq!(find_below(&root, 4, &mut 100), Some(libs.join(BRAW_LIB)));
        // Too shallow, or out of budget.
        assert_eq!(find_below(&root, 1, &mut 100), None);
        assert_eq!(find_below(&root, 4, &mut 1), None);
        // Picked by hand: the folder, the library, or not it.
        assert_eq!(braw_library_in(&root), Some(libs.join(BRAW_LIB)));
        assert_eq!(
            braw_library_in(&libs.join(BRAW_LIB)),
            Some(libs.join(BRAW_LIB))
        );
        std::fs::write(root.join("other.so"), b"").unwrap();
        assert_eq!(braw_library_in(&root.join("other.so")), None);
        assert_eq!(braw_library_in(&root.join("missing")), None);
        assert_eq!(find_braw(Some(&root)), Some(libs.join(BRAW_LIB)));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn oda_versions_come_from_the_install_folder() {
        assert_eq!(
            oda_version(Path::new("/opt/ODAFileConverter 25.4.0/ODAFileConverter")).as_deref(),
            Some("25.4.0")
        );
        assert_eq!(
            oda_version(Path::new(
                r"ODA/ODAFileConverter_26.1.0/ODAFileConverter.exe"
            ))
            .as_deref(),
            Some("26.1.0")
        );
        assert_eq!(oda_version(Path::new("/usr/bin/ODAFileConverter")), None);
    }
}
