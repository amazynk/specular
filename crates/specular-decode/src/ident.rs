//! Telling apart formats that share an extension.
//!
//! Most formats have a signature, and [`Registry::open`](crate::Registry::open) opens a file
//! with the first decoder whose content check matches. Some extensions are shared by
//! several formats (`.pic`, `.img`, `.raw`, `.pls`…) and some formats have no signature at
//! all; for those, decoders say how sure their check is ([`Decoder::confidence`]):
//!
//! - [`Confidence::Certain`]: a signature or magic number. Opened at once, whatever else
//!   the extension could be.
//! - [`Confidence::Likely`] / [`Confidence::Possible`]: structural heuristics (header
//!   fields that are consistent, sizes that fit the file).
//! - [`Confidence::Guess`]: only the extension says so ([`Shared::by_name`]).
//!
//! The folder is evidence too ([`FolderHint`]): the files a format comes with lying beside
//! it ([`COMPANIONS`]: a `.raw` beside `.mhd` headers, a `.mif` beside its `.mid`), or other
//! files of the same extension here already identified by signature or chosen by hand. It
//! never outweighs the content; it orders the formats the content leaves.
//!
//! With no certain match and more than one candidate, the file is opened by the choice made
//! for it ([`choose`]), else by a remembered rule for its extension ([`set_rules`]: in its
//! folder before everywhere), else `open` fails with [`DecodeError::Ambiguous`] carrying
//! the candidates ranked ([`rank_in`]), and the app asks "Which kind of file is …?".
//!
//! # Adding a decoder whose extension another decoder claims
//!
//! 1. Give it a content check. With a signature, nothing else is needed; without one,
//!    override [`Decoder::confidence`] to return `Likely` or `Possible` (never `Certain`).
//! 2. Add or extend the extension's line in [`SHARED`]: the formats most likely first (that
//!    is the prompt's order), those a file may be on its name alone, and how they are told
//!    apart. `shared_extensions_are_audited` fails until the table matches the registry.
//! 3. Optionally list the extensions of the files it comes with in [`COMPANIONS`].
//! 4. Add `file-kind-about-<format key in kebab case>` to the app's `file-kinds.ftl` (the
//!    prompt's one line on what the format is; the app's test lists any missing).
//!
//! [`Decoder::confidence`]: crate::Decoder::confidence
//! [`DecodeError::Ambiguous`]: crate::DecodeError::Ambiguous

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::SystemTime;

use crate::Format;

/// How sure a content check is, weakest first (so `Ord` ranks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// Only the extension says so.
    Guess,
    /// The content does not rule it out.
    Possible,
    /// Structural checks pass, without a signature.
    Likely,
    /// A signature matched.
    Certain,
}

impl Confidence {
    /// The word logs use.
    pub fn key(self) -> &'static str {
        match self {
            Confidence::Guess => "guess",
            Confidence::Possible => "possible",
            Confidence::Likely => "likely",
            Confidence::Certain => "certain",
        }
    }
}

/// What the file's folder says for a format, weakest first (so `Ord` ranks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FolderHint {
    /// Files the format comes with lie beside it ([`COMPANIONS`]).
    Companions,
    /// Other files of this extension here are this format (by signature, or chosen).
    SameKind,
}

/// A format a file may be, how sure the check is, and what its folder says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub format: Format,
    pub confidence: Confidence,
    pub hint: Option<FolderHint>,
}

impl Candidate {
    pub fn new(format: Format, confidence: Confidence) -> Candidate {
        Candidate {
            format,
            confidence,
            hint: None,
        }
    }
}

/// A file the content does not settle: its extension (lower case, without the dot; empty
/// for none) and the formats it may be, most likely first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ambiguity {
    pub ext: String,
    pub candidates: Vec<Candidate>,
}

impl std::fmt::Display for Ambiguity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the .{} file may be ", self.ext)?;
        for (i, c) in self.candidates.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{} ({}", c.format.name(), c.confidence.key())?;
            if let Some(h) = c.hint {
                write!(f, ", {h:?}")?;
            }
            f.write_str(")")?;
        }
        Ok(())
    }
}

/// An extension several decoders claim: its formats, most likely first, those a file may
/// be on its name alone, and how the content tells them apart.
pub struct Shared {
    pub ext: &'static str,
    pub formats: &'static [Format],
    /// Those of `formats` a file may be on its extension alone (no content check matched):
    /// one keeps the old way (its decoder explains the failure), two or more are asked
    /// about.
    pub by_name: &'static [Format],
    pub how: &'static str,
}

/// Every extension more than one decoder claims. Checked against the registry by
/// `shared_extensions_are_audited`; the order of `formats` is the prompt's.
pub const SHARED: &[Shared] = &[
    Shared {
        ext: "fff",
        formats: &[Format::Raw, Format::FlirFff],
        by_name: &[Format::FlirFff],
        how: "Hasselblad raw is a TIFF; FLIR FFF starts \"FFF\\0\" or \"AFF\\0\"",
    },
    Shared {
        ext: "fit",
        formats: &[Format::Fits, Format::Fit],
        by_name: &[Format::Fits],
        how: "FITS starts \"SIMPLE  =\"; Garmin FIT has \".FIT\" at byte 8",
    },
    Shared {
        ext: "mif",
        formats: &[Format::Mif, Format::MapInfoMif],
        by_name: &[Format::Mif],
        how: "MRtrix starts \"mrtrix image\"; MapInfo MIF is text with a Version line",
    },
    Shared {
        ext: "mod",
        formats: &[Format::Tracker, Format::MpegPs],
        by_name: &[Format::Tracker],
        how: "camcorder MOD is an MPEG program stream (00 00 01 BA); a tracker module has \
              its tag (\"M.K.\", \"8CHN\"…) at byte 1080",
    },
    Shared {
        ext: "mov",
        formats: &[Format::QuickTime, Format::ProResRaw],
        by_name: &[Format::QuickTime],
        how: "both are QuickTime; ProRes RAW's video track has an \"aprn\" or \"aprh\" \
              sample entry",
    },
    Shared {
        ext: "nc",
        formats: &[Format::NetCdf, Format::Hdf5],
        by_name: &[Format::Hdf5],
        how: "classic netCDF starts \"CDF\"; netCDF-4 is HDF5 (its signature)",
    },
    Shared {
        ext: "pic",
        formats: &[Format::Hdr, Format::SoftimagePic],
        by_name: &[Format::Hdr],
        how: "Radiance starts \"#?\"; Softimage PIC starts 53 80 F6 34",
    },
    Shared {
        ext: "scr",
        formats: &[Format::ZxScreen, Format::CpcScreen],
        by_name: &[Format::ZxScreen, Format::CpcScreen],
        how: "an AMSDOS header or exactly 16 KB is an Amstrad CPC screen; exactly 6912 bytes a ZX Spectrum screen",
    },
    Shared {
        ext: "scc",
        formats: &[Format::MsxScreen, Format::Scc],
        by_name: &[Format::Scc],
        how: "Scenarist captions start \"Scenarist_SCC\"; an MSX screen has a BSAVE header or a screen's exact size",
    },
    Shared {
        ext: "pcd",
        formats: &[Format::Pcd, Format::PhotoCd],
        by_name: &[Format::Pcd],
        how: "a PCD point cloud has a text header (VERSION, FIELDS); Kodak Photo CD has \"PCD_IPI\" at byte 2048",
    },
    Shared {
        ext: "ase",
        formats: &[Format::Aseprite, Format::SwatchExchange],
        by_name: &[Format::Aseprite],
        how: "Aseprite has A5E0 at byte 4; Adobe Swatch Exchange starts \"ASEF\"",
    },
    Shared {
        ext: "gbr",
        formats: &[Format::Gerber, Format::GimpBrush],
        by_name: &[Format::Gerber],
        how: "a GIMP brush has \"GIMP\" at byte 20 (version 1: a binary header whose sizes \
              add up); Gerber is text commands",
    },
    Shared {
        ext: "pls",
        formats: &[Format::Playlist, Format::Gerber],
        by_name: &[Format::Playlist, Format::Gerber],
        how: "a PLS playlist starts \"[playlist]\"; Gerber paste layers have Gerber commands",
    },
    Shared {
        ext: "ser",
        formats: &[Format::Ser, Format::Tia],
        by_name: &[Format::Tia],
        how: "Lucam SER starts \"LUCAM-RECORDER\"; FEI TIA series have their byte-order mark",
    },
];

/// Formats, and the extensions of files they come with: one of those beside a file is a
/// hint ([`FolderHint::Companions`]).
pub const COMPANIONS: &[(Format, &[&str])] = &[
    // Headerless volume data beside the headers of other volumes.
    (
        Format::RawVolume,
        &["mhd", "nhdr", "xtekct", "xtekvolume", "vgi", "dat"],
    ),
    (Format::Analyze, &["hdr"]),
    (Format::Envi, &["hdr"]),
    (Format::Pds3, &["lbl"]),
    // Camcorder MOD/TOD video comes with `.moi` index files.
    (Format::MpegPs, &["moi"]),
    (Format::MapInfoMif, &["mid"]),
    (Format::Tia, &["emi"]),
    (
        Format::Playlist,
        &["mp3", "flac", "ogg", "m4a", "opus", "wav"],
    ),
    (
        Format::Gerber,
        &[
            "gbr", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gko", "drl",
        ],
    ),
    (Format::Fit, &["gpx", "tcx"]),
    (Format::Fits, &["fits", "fts", "xisf"]),
    (Format::Raw, &["3fr"]),
    (Format::FlirFff, &["seq", "csq"]),
    // Dr. Halo pictures and their palettes; Quake 2 textures and the game's palette.
    (Format::DrHalo, &["pal"]),
    (Format::Wal, &["pcx"]),
];

/// Extensions other software uses for files Specular does not read, with what they
/// usually are, and how Specular tells its own apart. A decoder claiming one of these
/// must say how here (the audit checks it).
pub const FOREIGN: &[(&str, &str, &str)] = &[
    (
        "img",
        "disc and disk images",
        "content: SMV, R-AXIS, ERDAS, VICAR; a .hdr or label beside",
    ),
    ("dat", "any program's data", "an ENVI or ESRI .hdr beside"),
    (
        "raw",
        "camera raw, disc images",
        "a header beside, a known camera's exact size, or the layout asked for",
    ),
    (
        "bin",
        "firmware, disc images",
        "nothing reads it: raw samples on request",
    ),
    (
        "mod",
        "Go modules, 15-sample tracker modules",
        "content (MPEG program stream, tracker tag) and .moi beside",
    ),
    ("pic", "Pictor, Biorad, Houdini pictures", "content"),
    // GIS formats.
    (
        "000",
        "split archives and numbered backups",
        "the ISO 8211 header of an S-57 chart",
    ),
    ("vrt", "OGR virtual vector layers", "an XML root VRTDataset"),
    // fmt500e
    (
        "stm",
        "STEP models from some CAD programs",
        "Scream Tracker 2's header",
    ),
    (
        "med",
        "MED songs saved without samples",
        "an MMD0 to MMD3 header",
    ),
    (
        "str",
        "game and program data",
        "a PlayStation STR sector header",
    ),
    ("bdf", "Nastran bulk data", "the STARTFONT line"),
    (
        "psf",
        "PSF music and PostScript files",
        "the PSF 1 or 2 magic",
    ),
    (
        "fnt",
        "AngelCode and game bitmap font text",
        "a Windows FNT header",
    ),
    (
        "rwz",
        "Outlook Rules Wizard exports",
        "the Rawzor signature (refused either way, in plain words)",
    ),
    (
        "avs",
        "AviSynth scripts",
        "the AVS header's size fits the file",
    ),
    ("mac", "macro files", "a MacPaint header and PackBits rows"),
    (
        "tp",
        "Turbo Pascal units, other programs",
        "MPEG-TS sync bytes every 188 or 192 bytes",
    ),
    (
        "sln",
        "Visual Studio solutions",
        "not text; headerless 16-bit sound at 8 kHz",
    ),
    ("cut", "cut lists", "a Dr. Halo header and its first line"),
    ("rle", "RLE bitmaps", "the Utah RLE magic"),
    (
        "dcr",
        "Shockwave (Director) movies",
        "a Kodak TIFF raw's colour filter array",
    ),
    (
        "scr",
        "Windows screen savers",
        "a ZX Spectrum screen's exact size, an AMSDOS header or exactly a 16 KB Amstrad CPC screen",
    ),
    ("abc", "ABC music notation", "the Ogawa signature"),
    (
        "mic",
        "Microsoft Image Composer pictures",
        "exactly an Atari 7680-byte screen",
    ),
    ("asc", "PGP armour, plain text", "an Esri ASCII grid header"),
    (
        "act",
        "voice recorder sound, Microsoft Agent characters",
        "exactly 768 or 772 bytes (an Adobe colour table)",
    ),
    (
        "gd",
        "Godot scripts",
        "GD 2's 0xFFFE/0xFFFF id, else exactly a GD 1 image's size",
    ),
    (
        "pat",
        "Photoshop patterns, other pattern files",
        "\"GPAT\" at byte 20 (Photoshop's \"8BPT\" is refused in plain words)",
    ),
    (
        "cal",
        "calendars, calibration data",
        "a CALS header record (srcdocid:, rtype:…) and its rpelcnt",
    ),
    (
        "im",
        "other software's .im files",
        "an \"Image type:\" header line",
    ),
    (
        "spi",
        "other software's .spi files",
        "a SPIDER header whose record sizes add up",
    ),
    (
        "dem",
        "game demo recordings",
        "a USGS DEM A record's fixed columns",
    ),
    (
        "map",
        "linker maps, game levels, MapInfo",
        "the CCP4/MRC header (sniff)",
    ),
    (
        "cube",
        "colour lookup tables",
        "Gaussian cube header lines (sniff)",
    ),
    ("bag", "ROS bags", "HDF5 naming BAG_root (sniff)"),
    ("edf", "EEG recordings", "the ESRF header (sniff)"),
    (
        "log",
        "any program's log",
        "a SkyScan reconstruction's lines (sniff)",
    ),
    ("lbl", "label printer files", "a PDS3 label (sniff)"),
    ("sub", "subtitles (MicroDVD, VobSub)", "content"),
    (
        "ps",
        "PostScript documents",
        "an EPS header with a preview; other PostScript is refused",
    ),
    (
        "iff",
        "Maya IFF, AIFF sound, other IFF chunks",
        "the FORM type (ILBM, PBM)",
    ),
    (
        "tif",
        "GeoTIFF, OME-TIFF, camera raw, pyramids",
        "TIFF tags",
    ),
    (
        "tiff",
        "GeoTIFF, OME-TIFF, camera raw, pyramids",
        "TIFF tags",
    ),
    ("v", "Verilog source", "the ECAT 7 header (sniff)"),
    (
        "hdr",
        "Analyze and ENVI headers",
        "Radiance's #? signature; headers open their data",
    ),
    (
        "vol",
        "other programs' volumes",
        "a header beside, or the layout asked for",
    ),
];

/// [`SHARED`]'s line for `ext`.
pub fn shared(ext: &str) -> Option<&'static Shared> {
    SHARED.iter().find(|s| s.ext == ext)
}

/// Whether a file named `.ext` may be `format` on its name alone, when the content matched
/// no check: always for an extension one decoder has, else as [`SHARED`] says.
pub fn by_name(format: Format, ext: Option<&str>) -> bool {
    ext.and_then(shared)
        .is_none_or(|s| s.by_name.contains(&format))
}

/// A format's stable name for settings and stores (`Format`'s variant name).
pub fn key(format: Format) -> String {
    format!("{format:?}")
}

/// Sorts `candidates` most likely first: by confidence, then by what the folder says, then
/// by [`SHARED`]'s order for `ext`, then as given (the registry's probe order). Duplicate
/// formats keep the best.
pub fn rank(ext: &str, candidates: &mut Vec<Candidate>) {
    let mut best: Vec<Candidate> = Vec::with_capacity(candidates.len());
    for c in candidates.drain(..) {
        match best.iter_mut().find(|b| b.format == c.format) {
            Some(b) => {
                b.confidence = b.confidence.max(c.confidence);
                b.hint = b.hint.max(c.hint);
            }
            None => best.push(c),
        }
    }
    let order = shared(ext).map_or(&[][..], |s| s.formats);
    let pos = |f: Format| order.iter().position(|o| *o == f).unwrap_or(order.len());
    // A stable sort keeps the registry's order among equals.
    best.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then(b.hint.cmp(&a.hint))
            .then(pos(a.format).cmp(&pos(b.format)))
    });
    *candidates = best;
}

/// [`rank`] for the file at `path`, with what its folder says ([`folder_hints`]).
pub fn rank_in(path: &Path, candidates: &mut Vec<Candidate>) {
    let ext = crate::path_extensions(path)[1].clone().unwrap_or_default();
    folder_hints(path, candidates);
    rank(&ext, candidates);
}

/// Whether ranked candidates need a person to choose: no signature matched and more than
/// one format is left, unless the first passed structural checks and the rest are only the
/// extension's guesses (a camera raw of a known camera's exact size against a raw volume).
pub fn needs_choice(candidates: &[Candidate]) -> bool {
    let Some((top, rest)) = candidates.split_first() else {
        return false;
    };
    !rest.is_empty()
        && top.confidence < Confidence::Certain
        && !(top.confidence >= Confidence::Likely
            && rest.iter().all(|c| c.confidence == Confidence::Guess))
}

// ----- The folder as evidence -----------------------------------------------------------

/// Folders whose facts are kept before the cache starts over.
const FOLDERS_MAX: usize = 64;

/// What is known of a folder: the extensions in it (read once, again when it changes) and
/// the formats files of each extension were found to be.
#[derive(Default)]
struct Folder {
    /// The folder's modification time when listed; `None` when not listed yet.
    listed: Option<Option<SystemTime>>,
    exts: HashSet<String>,
    seen: HashMap<String, HashMap<Format, u32>>,
}

fn folders() -> &'static Mutex<HashMap<PathBuf, Folder>> {
    static F: OnceLock<Mutex<HashMap<PathBuf, Folder>>> = OnceLock::new();
    F.get_or_init(Default::default)
}

fn with_folder<R>(dir: &Path, f: impl FnOnce(&mut Folder) -> R) -> Option<R> {
    let mut all = folders().lock().ok()?;
    if all.len() >= FOLDERS_MAX && !all.contains_key(dir) {
        all.clear();
    }
    Some(f(all.entry(dir.to_path_buf()).or_default()))
}

fn ext_of(name: &Path) -> Option<String> {
    name.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
}

/// Hands over a folder's listing the caller already has (`names` are file names or paths
/// in `dir`): what [`folder_hints`] would otherwise read once itself.
pub fn set_listing<'a>(dir: &Path, names: impl IntoIterator<Item = &'a Path>) {
    let exts: HashSet<String> = names.into_iter().filter_map(ext_of).collect();
    let mtime = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    with_folder(dir, |f| {
        f.exts = exts;
        f.listed = Some(mtime);
    });
}

/// Notes that the file at `path` is `format` by signature (shared extensions only): other
/// files of its extension in its folder lean the same way.
pub(crate) fn note_identified(path: &Path, format: Format) {
    let (Some(dir), Some(ext)) = (path.parent(), ext_of(path)) else {
        return;
    };
    if shared(&ext).is_none() && !COMPANIONS.iter().any(|(f, _)| *f == format) {
        return;
    }
    with_folder(dir, |f| {
        *f.seen.entry(ext).or_default().entry(format).or_default() += 1;
    });
}

/// The extensions in `dir`, listed at most once per change of the folder.
fn folder_exts(dir: &Path) -> HashSet<String> {
    let mtime = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    let fresh = with_folder(dir, |f| (f.listed == Some(mtime)).then(|| f.exts.clone())).flatten();
    if let Some(e) = fresh {
        return e;
    }
    let exts: HashSet<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| ext_of(Path::new(&e.file_name())))
        .collect();
    with_folder(dir, |f| {
        f.exts = exts.clone();
        f.listed = Some(mtime);
    });
    exts
}

/// Sets each candidate's [`FolderHint`] from the folder of `path`: other files of its
/// extension found to be the format (by signature, or chosen by hand for files here), else
/// the format's companion files beside it.
pub fn folder_hints(path: &Path, candidates: &mut [Candidate]) {
    let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) else {
        return;
    };
    let ext = ext_of(path).unwrap_or_default();
    let mut same: HashMap<Format, u32> =
        with_folder(dir, |f| f.seen.get(&ext).cloned().unwrap_or_default()).unwrap_or_default();
    if let Ok(c) = choices().lock() {
        for (p, f) in c.iter() {
            if p != path && p.parent() == Some(dir) && ext_of(p).as_deref() == Some(&ext) {
                *same.entry(*f).or_default() += 1;
            }
        }
    }
    let wants_companions = candidates
        .iter()
        .any(|c| COMPANIONS.iter().any(|(f, _)| *f == c.format));
    let exts = if wants_companions {
        folder_exts(dir)
    } else {
        HashSet::new()
    };
    let most = same.values().copied().max().unwrap_or(0);
    for c in candidates.iter_mut() {
        let hint = if most > 0 && same.get(&c.format) == Some(&most) {
            Some(FolderHint::SameKind)
        } else if COMPANIONS
            .iter()
            .filter(|(f, _)| *f == c.format)
            .any(|(_, list)| list.iter().any(|e| *e != ext && exts.contains(*e)))
        {
            Some(FolderHint::Companions)
        } else {
            None
        };
        c.hint = c.hint.max(hint);
    }
}

// ----- Choices and rules ----------------------------------------------------------------

fn choices() -> &'static Mutex<HashMap<PathBuf, Format>> {
    static C: OnceLock<Mutex<HashMap<PathBuf, Format>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Opens the file at `path` as `format` from now on (this process): the prompt's answer,
/// or "Open as…". Wins over any content check.
pub fn choose(path: &Path, format: Format) {
    if let Ok(mut c) = choices().lock() {
        c.insert(path.to_path_buf(), format);
    }
    // Answered: a failure from now on is the chosen decoder's, not a question.
    if let Ok(mut p) = pending().lock() {
        p.remove(path);
    }
}

/// Drops [`choose`]'s choice for `path`.
pub fn unchoose(path: &Path) {
    if let Ok(mut c) = choices().lock() {
        c.remove(path);
    }
}

/// The format chosen for `path` this process, if any.
pub fn chosen(path: &Path) -> Option<Format> {
    choices().lock().ok()?.get(path).copied()
}

/// "Always open .`ext` files as `format`": in `folder`, or everywhere when `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Lower case, without the dot.
    pub ext: String,
    pub folder: Option<PathBuf>,
    pub format: Format,
}

fn rules() -> &'static RwLock<Vec<Rule>> {
    static R: OnceLock<RwLock<Vec<Rule>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Replaces the remembered rules (the app loads them from its data folder and sets them
/// again whenever one is added or forgotten).
pub fn set_rules(list: Vec<Rule>) {
    if let Ok(mut r) = rules().write() {
        *r = list;
    }
}

/// The rule's format for `path` among `candidates`: its folder's rule, else one for
/// everywhere. A rule naming a format the file cannot be is passed over.
pub fn rule_for(path: &Path, candidates: &[Candidate]) -> Option<Format> {
    let ext = crate::path_extensions(path)[1].clone()?;
    let dir = path.parent();
    let r = rules().read().ok()?;
    let fits = |f: Format| candidates.iter().any(|c| c.format == f);
    let mine = |x: &&Rule| x.ext == ext && fits(x.format);
    r.iter()
        .filter(mine)
        .find(|x| x.folder.is_some() && x.folder.as_deref() == dir)
        .or_else(|| r.iter().filter(mine).find(|x| x.folder.is_none()))
        .map(|x| x.format)
}

/// Ambiguities last met, by path: the app reads the candidates when the open fails.
fn pending() -> &'static Mutex<HashMap<PathBuf, Ambiguity>> {
    static P: OnceLock<Mutex<HashMap<PathBuf, Ambiguity>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

/// Entries [`pending`] keeps before it starts over.
const PENDING_MAX: usize = 256;

pub(crate) fn note(path: &Path, a: &Ambiguity) {
    if let Ok(mut p) = pending().lock() {
        if p.len() >= PENDING_MAX {
            p.clear();
        }
        p.insert(path.to_path_buf(), a.clone());
    }
}

/// What made the last open of `path` fail with [`crate::DecodeError::Ambiguous`].
pub fn ambiguity_of(path: &Path) -> Option<Ambiguity> {
    pending().lock().ok()?.get(path).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(format: Format, confidence: Confidence) -> Candidate {
        Candidate::new(format, confidence)
    }

    /// A folder of its own under the scratch space.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ident-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn ranking_is_confidence_then_folder_then_popularity_then_order() {
        let mut v = vec![
            c(Format::Tga, Confidence::Guess),
            c(Format::Png, Confidence::Possible),
            c(Format::Jpeg, Confidence::Guess),
            c(Format::Tga, Confidence::Likely),
        ];
        rank("zzz", &mut v);
        let order: Vec<Format> = v.iter().map(|c| c.format).collect();
        assert_eq!(order, [Format::Tga, Format::Png, Format::Jpeg]);
        assert_eq!(v[0].confidence, Confidence::Likely);
        assert!(needs_choice(&v));
        assert!(!needs_choice(&v[..1]));
        // A structural match against guesses by name alone is not a question.
        assert!(!needs_choice(&[v[0], v[2]]));
        assert!(needs_choice(&[v[1], v[2]]));
        // The folder's word orders equals, never a stronger check.
        let mut v = vec![
            c(Format::Playlist, Confidence::Guess),
            Candidate {
                hint: Some(FolderHint::Companions),
                ..c(Format::Gerber, Confidence::Guess)
            },
        ];
        rank("pls", &mut v);
        assert_eq!(v[0].format, Format::Gerber);
        let mut v = vec![
            c(Format::Playlist, Confidence::Possible),
            Candidate {
                hint: Some(FolderHint::SameKind),
                ..c(Format::Gerber, Confidence::Guess)
            },
        ];
        rank("pls", &mut v);
        assert_eq!(v[0].format, Format::Playlist);
        // Popularity: SHARED's order among equals.
        let mut v = vec![
            c(Format::Gerber, Confidence::Guess),
            c(Format::Playlist, Confidence::Guess),
        ];
        rank("pls", &mut v);
        assert_eq!(v[0].format, Format::Playlist);
        let mut sure = vec![
            c(Format::Png, Confidence::Certain),
            c(Format::Tga, Confidence::Guess),
        ];
        rank("zzz", &mut sure);
        assert!(!needs_choice(&sure));
    }

    #[test]
    fn companions_beside_a_file_are_a_hint() {
        let d = scratch("companions");
        for n in ["board.gtl", "board.gbl", "paste.pls"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        let mut v = vec![
            c(Format::Playlist, Confidence::Guess),
            c(Format::Gerber, Confidence::Guess),
        ];
        rank_in(&d.join("paste.pls"), &mut v);
        assert_eq!(v[0].format, Format::Gerber);
        assert_eq!(v[0].hint, Some(FolderHint::Companions));
        assert_eq!(v[1].hint, None);
        // A listing handed over replaces reading the folder.
        set_listing(&d, [Path::new("song.mp3"), Path::new("paste.pls")]);
        let mut v = vec![
            c(Format::Gerber, Confidence::Guess),
            c(Format::Playlist, Confidence::Guess),
        ];
        rank_in(&d.join("paste.pls"), &mut v);
        assert_eq!(v[0].format, Format::Playlist);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn files_of_the_same_extension_lean_the_folder() {
        let d = scratch("same-kind");
        // One identified by signature, two chosen by hand: the most seen wins.
        note_identified(&d.join("a.pls"), Format::Playlist);
        choose(&d.join("b.pls"), Format::Gerber);
        choose(&d.join("c.pls"), Format::Gerber);
        let mut v = vec![
            c(Format::Playlist, Confidence::Guess),
            c(Format::Gerber, Confidence::Guess),
        ];
        rank_in(&d.join("x.pls"), &mut v);
        assert_eq!(v[0].format, Format::Gerber);
        assert_eq!(v[0].hint, Some(FolderHint::SameKind));
        // Another folder is not swayed.
        let mut v = vec![
            c(Format::Gerber, Confidence::Guess),
            c(Format::Playlist, Confidence::Guess),
        ];
        rank_in(&d.join("sub").join("x.pls"), &mut v);
        assert_eq!(v[0].format, Format::Playlist);
        assert_eq!(v[0].hint, None);
        for n in ["b.pls", "c.pls"] {
            unchoose(&d.join(n));
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_folder_rule_beats_one_for_everywhere() {
        let cands = [
            c(Format::Tga, Confidence::Guess),
            c(Format::Pcx, Confidence::Guess),
        ];
        set_rules(vec![
            Rule {
                ext: "zq1".into(),
                folder: None,
                format: Format::Tga,
            },
            Rule {
                ext: "zq1".into(),
                folder: Some(PathBuf::from("/pics")),
                format: Format::Pcx,
            },
            // Not a candidate: passed over.
            Rule {
                ext: "zq2".into(),
                folder: None,
                format: Format::Png,
            },
        ]);
        assert_eq!(
            rule_for(Path::new("/pics/a.ZQ1"), &cands),
            Some(Format::Pcx)
        );
        assert_eq!(
            rule_for(Path::new("/other/a.zq1"), &cands),
            Some(Format::Tga)
        );
        assert_eq!(rule_for(Path::new("/pics/a.zq2"), &cands), None);
        set_rules(Vec::new());
        assert_eq!(rule_for(Path::new("/pics/a.zq1"), &cands), None);
    }

    #[test]
    fn choices_are_per_file_and_answer_the_question() {
        let p = Path::new("/x/choice-test.zq3");
        assert_eq!(chosen(p), None);
        note(
            p,
            &Ambiguity {
                ext: "zq3".into(),
                candidates: Vec::new(),
            },
        );
        assert!(ambiguity_of(p).is_some());
        choose(p, Format::Tga);
        assert_eq!(chosen(p), Some(Format::Tga));
        assert!(ambiguity_of(p).is_none());
        unchoose(p);
        assert_eq!(chosen(p), None);
    }

    /// `.mod` and `.pic` are told apart by signatures; a check without one counts only for
    /// the extensions its decoder claims; a headerless camera `.raw` is likely, not certain.
    #[test]
    fn new_formats_rank_by_content() {
        let reg = crate::Registry::new();
        let first = |name: &str, data: Vec<u8>| {
            let src = specular_core::Source::from_vec(name, data);
            reg.identify(&src).first().map(|c| (c.format, c.confidence))
        };
        let mut ps = vec![0u8; 2048];
        ps[..7].copy_from_slice(&[0, 0, 1, 0xBA, 0x44, 0, 4]);
        assert_eq!(
            first("clip.mod", ps),
            Some((Format::MpegPs, Confidence::Certain))
        );
        let mut module = vec![0u8; 2048];
        module[950] = 1;
        module[1080..1084].copy_from_slice(b"M.K.");
        assert_eq!(
            first("song.mod", module),
            Some((Format::Tracker, Confidence::Certain))
        );
        let mut pic = vec![0u8; 256];
        pic[..4].copy_from_slice(&[0x53, 0x80, 0xf6, 0x34]);
        pic[88..92].copy_from_slice(b"PICT");
        assert_eq!(
            first("render.pic", pic),
            Some((Format::SoftimagePic, Confidence::Certain))
        );
        // A ZX Spectrum screen is a size: likely as `.scr`, nothing as another name.
        assert_eq!(
            first("game.scr", vec![0u8; 6912]),
            Some((Format::ZxScreen, Confidence::Likely))
        );
        let src = specular_core::Source::from_vec("photo.tga", vec![0u8; 6912]);
        assert!(
            reg.identify(&src)
                .iter()
                .all(|c| c.format != Format::ZxScreen)
        );
        // A Pentax Optio S4 dump: camera raw first, the volume reader still offered.
        let src = specular_core::Source::from_vec("shot.raw", vec![0u8; 6_114_240]);
        let c = reg.identify(&src);
        assert_eq!(
            c.first().map(|c| (c.format, c.confidence)),
            Some((Format::Raw, Confidence::Likely))
        );
        assert!(c.iter().any(|c| c.format == Format::RawVolume));
        // The camera's size outweighs the volume reader's guess by name: opened, not asked.
        assert!(!needs_choice(&c));
    }

    /// The shared-extension audit: every extension more than one decoder claims has its
    /// line in SHARED naming exactly those formats; every format a file may be by name is
    /// one of them; a FOREIGN extension a decoder claims says how its files are told apart.
    #[test]
    fn shared_extensions_are_audited() {
        let claims = crate::Registry::new().extension_claims();
        let mut problems = Vec::new();
        for (ext, formats) in &claims {
            if formats.len() < 2 {
                continue;
            }
            match shared(ext) {
                None => problems.push(format!(
                    ".{ext} is claimed by {formats:?}: add its line to ident::SHARED"
                )),
                Some(s) => {
                    let mut a: Vec<String> = s.formats.iter().map(|f| key(*f)).collect();
                    let mut b: Vec<String> = formats.iter().map(|f| key(*f)).collect();
                    a.sort();
                    b.sort();
                    if a != b {
                        problems.push(format!(
                            ".{ext}: SHARED lists {a:?}, the decoders claiming it are {b:?}"
                        ));
                    }
                }
            }
        }
        for s in SHARED {
            if claims.get(s.ext).is_none_or(|f| f.len() < 2) {
                problems.push(format!(".{}: in SHARED but not shared any more", s.ext));
            }
            if s.by_name.iter().any(|f| !s.formats.contains(f)) || s.how.is_empty() {
                problems.push(format!(".{}: by_name outside formats, or no how", s.ext));
            }
        }
        for (ext, others, how) in FOREIGN {
            if claims.contains_key(ext) && (how.is_empty() || others.is_empty()) {
                problems.push(format!(
                    ".{ext}: a decoder claims it; say how it is told apart"
                ));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
