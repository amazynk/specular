//! Electronics files through `specular-pcb` and `specular-kicad`, drawn by `specular-vector`
//! like a DXF drawing:
//!
//! * One Gerber or Excellon file opens alone: one layer in its role's colour.
//! * A board's Gerber and drill files together (a folder, see `Registry::open`, or a ZIP
//!   recognised by its file names) open as one board: the layers stacked, then the top and
//!   the bottom in board colours, each file a layer of the layers panel.
//! * KiCad 6–9 boards (`.kicad_pcb`) and schematics (`.kicad_sch`, a page per sheet; the
//!   sheets' files are read from beside it). KiCad 4 and 5 files are refused by name.

use std::path::Path;
use std::sync::Arc;

use specular_core::Source;
use specular_vector::{Generated, SheetKind, VectorScene};

use crate::{DecodeError, Decoder, Format, Result, Session};

/// Files of a board read at most, and bytes per file.
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: u64 = 256 << 20;

fn pcb_err(e: specular_pcb::Error) -> DecodeError {
    use specular_pcb::Error as E;
    match e {
        E::NotThisFormat => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
    }
}

fn kicad_err(e: specular_kicad::Error) -> DecodeError {
    use specular_kicad::Error as E;
    match e {
        E::NotThisFormat => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
    }
}

/// The file name of a path, for role detection.
fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Board views get names shown in the reader's language; the top and bottom in board
/// colours draw their layers opaque (the layer view is translucent).
pub fn name_board_views(scene: &mut VectorScene) {
    let opaque = vec![1.0; scene.layers.len()];
    for s in &mut scene.sheets {
        if matches!(s.name.as_str(), "Top" | "Bottom") {
            s.layer_opacity = Some(opaque.clone());
        }
        let g = match s.name.as_str() {
            "Layers" => Generated::BoardLayers,
            "Top" => Generated::BoardTop,
            "Bottom" => Generated::BoardBottom,
            _ => continue,
        };
        s.generated = Some(g);
    }
}

/// One Gerber file (`Format::Gerber`) or one Excellon drill file (`Format::Excellon`).
pub struct GerberDecoder(pub Format);

impl Decoder for GerberDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Format::Excellon => specular_pcb::excellon::is_excellon(head),
            _ => specular_pcb::gerber::is_gerber(head),
        }
    }
    /// Text formats: their signs may come after comments or a long prolog.
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(crate::PROBE_TEXT)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Excellon => &["drl", "xln", "drd", "exc"],
            _ => &[
                "gbr", "ger", "gbx", "pho", "art", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gtp",
                "gbp", "gko", "gm1", "gm2", "gm3", "gm4", "gm13", "gm15", "gp1", "gp2", "g1", "g2",
                "g3", "g4", "g5", "g6", "gd1", "gg1", "cmp", "sol", "stc", "sts", "plc", "pls",
                "crc", "crs",
            ],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let data = source.bytes();
        // Opened by extension: the content must say it is this format.
        if !self.probe(&data[..data.len().min(64 << 10)]) {
            return Err(DecodeError::UnsupportedFormat);
        }
        let limits = specular_pcb::Limits::default();
        let scene = specular_pcb::stack::single_scene(&file_name(source.path()), data, &limits)
            .map_err(pcb_err)?;
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        crate::dxf_dec::session(source, scene, self.0, parse_ms)
    }
}

/// A board's Gerber and drill files: a folder of them or a ZIP.
pub struct BoardDecoder;

/// The names in a ZIP's directory (files only, their own names without folders; macOS
/// resource forks left out), or `None` if `data` is not a ZIP.
fn zip_names(data: &[u8]) -> Option<(specular_ora::zip::Zip, Vec<(usize, String)>)> {
    if !data.starts_with(b"PK\x03\x04") {
        return None;
    }
    let zip = specular_ora::zip::Zip::parse(data).ok()?;
    let names = zip
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.name.ends_with('/') && !e.name.starts_with("__MACOSX/"))
        .map(|(i, e)| (i, e.name.rsplit('/').next().unwrap_or(&e.name).to_owned()))
        .filter(|(_, n)| !n.is_empty() && !n.starts_with('.'))
        .collect();
    Some((zip, names))
}

fn names_look_like_board(names: &[(usize, String)]) -> bool {
    let n: Vec<&str> = names.iter().map(|(_, n)| n.as_str()).collect();
    specular_pcb::stack::looks_like_board(&n)
}

/// Whether the archive at `path` is one board's Gerber files (a ZIP whose names say so), to be
/// opened as a board rather than browsed as a book.
pub fn is_board_archive(path: &Path) -> bool {
    let is_zip = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if !is_zip {
        return false;
    }
    let Ok(src) = Source::open(path) else {
        return false;
    };
    zip_names(src.bytes()).is_some_and(|(_, n)| names_look_like_board(&n))
}

/// Whether a file of a board's set is worth reading: its name gives a role, or its start
/// says Gerber or Excellon.
fn board_file(name: &str, head: &[u8]) -> bool {
    specular_pcb::stack::role_of(name, None) != specular_pcb::stack::Role::Unknown
        || specular_pcb::gerber::is_gerber(head)
        || specular_pcb::excellon::is_excellon(head)
}

impl BoardDecoder {
    /// The files of a board's folder (regular files, size-capped), sorted by name.
    fn folder_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .map_err(DecodeError::Io)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= MAX_FILE_BYTES)
            })
            .collect();
        paths.sort();
        let mut out = Vec::new();
        for p in paths {
            if out.len() >= MAX_FILES {
                break;
            }
            let name = file_name(&p);
            if name.starts_with('.') {
                continue;
            }
            let Ok(bytes) = std::fs::read(&p) else {
                continue;
            };
            if board_file(&name, &bytes[..bytes.len().min(64 << 10)]) {
                out.push((name, bytes));
            }
        }
        Ok(out)
    }

    fn zip_files(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
        let (zip, names) = zip_names(data).ok_or(DecodeError::UnsupportedFormat)?;
        let mut out = Vec::new();
        for (i, name) in names {
            if out.len() >= MAX_FILES {
                break;
            }
            let e = &zip.entries[i];
            let Ok(bytes) = zip.read(data, e, MAX_FILE_BYTES) else {
                continue;
            };
            if board_file(&name, &bytes[..bytes.len().min(64 << 10)]) {
                out.push((name, bytes.into_owned()));
            }
        }
        Ok(out)
    }
}

impl Decoder for BoardDecoder {
    fn format(&self) -> Format {
        Format::GerberSet
    }
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        zip_names(data).is_some_and(|(_, n)| names_look_like_board(&n))
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let files = if source.is_folder() {
            Self::folder_files(source.path())?
        } else {
            Self::zip_files(source.bytes())?
        };
        if files.is_empty() {
            return Err(DecodeError::Unsupported(
                "no Gerber or drill files in it".into(),
            ));
        }
        let limits = specular_pcb::Limits::default();
        let mut scene = specular_pcb::stack::board_scene(&files, &limits).map_err(pcb_err)?;
        drop(files);
        name_board_views(&mut scene);
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        crate::dxf_dec::session(source, scene, Format::GerberSet, parse_ms)
    }
}

/// Files of an ODB++ job read at most, and bytes in all (unpacked).
const ODB_MAX_FILES: usize = 50_000;
const ODB_MAX_BYTES: usize = 2 << 30;
/// Bytes of a `.tgz` inflated to recognise a job by its first paths.
const ODB_PROBE_BYTES: usize = 2 << 20;

/// The paths of an archive's first entries: a ZIP's whole directory, a tar's (or a gzipped
/// tar's) headers within its first [`ODB_PROBE_BYTES`].
fn archive_names(data: &[u8]) -> Vec<String> {
    if let Some((zip, _)) = zip_names(data) {
        return zip.entries.iter().map(|e| e.name.clone()).collect();
    }
    if data.starts_with(&[0x1F, 0x8B]) {
        return specular_png::inflate_gzip_prefix(data, ODB_PROBE_BYTES)
            .map(|t| specular_pcb::odb::tar_names(&t))
            .unwrap_or_default();
    }
    specular_pcb::odb::tar_names(data)
}

/// Whether the archive at `path` is an ODB++ job (a `.tgz`, `.tar` or `.zip` whose paths say
/// so), to be opened as a board rather than browsed as a book.
pub fn is_odb_archive(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    if ![".tgz", ".tar.gz", ".tar", ".zip"]
        .iter()
        .any(|e| lower.ends_with(e))
    {
        return false;
    }
    let Ok(src) = Source::open(path) else {
        return false;
    };
    let names = archive_names(src.bytes());
    let n: Vec<&str> = names.iter().map(String::as_str).collect();
    specular_pcb::odb::looks_like_job_start(&n)
}

/// ODB++ jobs: a folder, or a `.tgz`, `.tar` or `.zip` of one.
pub struct OdbDecoder;

impl OdbDecoder {
    /// Every file under the job folder `dir` (paths relative to it), size-capped.
    fn folder_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
        let mut out = Vec::new();
        let mut total = 0usize;
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let rd = std::fs::read_dir(&d).map_err(DecodeError::Io)?;
            for e in rd.flatten() {
                let p = e.path();
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() {
                    stack.push(p);
                } else if ft.is_file() {
                    if out.len() >= ODB_MAX_FILES {
                        return Err(DecodeError::TooLarge);
                    }
                    let bytes = std::fs::read(&p).map_err(DecodeError::Io)?;
                    total += bytes.len();
                    if total > ODB_MAX_BYTES {
                        return Err(DecodeError::TooLarge);
                    }
                    let rel = p.strip_prefix(dir).unwrap_or(&p);
                    out.push((rel.to_string_lossy().replace('\\', "/"), bytes));
                }
            }
        }
        Ok(out)
    }

    fn archive_files(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
        if let Some((zip, _)) = zip_names(data) {
            let mut out = Vec::new();
            let mut total = 0usize;
            for e in zip.entries.iter().filter(|e| !e.name.ends_with('/')) {
                if out.len() >= ODB_MAX_FILES {
                    return Err(DecodeError::TooLarge);
                }
                let b = zip
                    .read(data, e, MAX_FILE_BYTES)
                    .map_err(|e| DecodeError::Malformed(e.to_string()))?;
                total += b.len();
                if total > ODB_MAX_BYTES {
                    return Err(DecodeError::TooLarge);
                }
                out.push((e.name.clone(), b.into_owned()));
            }
            return Ok(out);
        }
        let tar;
        let bytes = if data.starts_with(&[0x1F, 0x8B]) {
            tar = specular_png::inflate_gzip_members(data, ODB_MAX_BYTES, None).map_err(|e| {
                DecodeError::Malformed(format!("the job's gzip stream is damaged: {e}"))
            })?;
            &tar[..]
        } else {
            data
        };
        specular_pcb::odb::untar(bytes, ODB_MAX_BYTES).map_err(pcb_err)
    }
}

impl Decoder for OdbDecoder {
    fn format(&self) -> Format {
        Format::Odb
    }
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        let tarish = data.len() > 262 && &data[257..262] == b"ustar";
        if !(data.starts_with(b"PK\x03\x04") || data.starts_with(&[0x1F, 0x8B]) || tarish) {
            return false;
        }
        let names = archive_names(data);
        let n: Vec<&str> = names.iter().map(String::as_str).collect();
        specular_pcb::odb::looks_like_job_start(&n)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let files = if source.is_folder() {
            Self::folder_files(source.path())?
        } else {
            Self::archive_files(source.bytes())?
        };
        let mut unpacked = Vec::with_capacity(files.len());
        for (n, b) in files {
            // Only the job's own data; `.z` files are Unix-compressed.
            let lower = n.to_ascii_lowercase();
            if lower.contains("/.") || lower.starts_with('.') {
                continue;
            }
            let (n, b) = specular_pcb::odb::unpack(&n, b, ODB_MAX_BYTES).map_err(pcb_err)?;
            unpacked.push((n, b));
        }
        let names: Vec<&str> = unpacked.iter().map(|(n, _)| n.as_str()).collect();
        if !specular_pcb::odb::looks_like_job(&names) {
            return Err(DecodeError::Unsupported(
                "This is not an ODB++ job: it has no matrix/matrix file and steps folder.".into(),
            ));
        }
        let limits = specular_pcb::Limits::default();
        let mut scene = specular_pcb::odb::scene(&unpacked, &limits).map_err(pcb_err)?;
        drop(unpacked);
        name_board_views(&mut scene);
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        crate::dxf_dec::session(source, scene, Format::Odb, parse_ms)
    }
}

/// IPC-2581 files (XML).
pub struct Ipc2581Decoder;

impl Decoder for Ipc2581Decoder {
    fn format(&self) -> Format {
        Format::Ipc2581
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_pcb::ipc2581::is_ipc2581(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cvg"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let data = source.bytes();
        if !self.probe(&data[..data.len().min(4096)]) {
            return Err(DecodeError::UnsupportedFormat);
        }
        let limits = specular_pcb::Limits::default();
        let mut scene = specular_pcb::ipc2581::scene(data, &limits).map_err(pcb_err)?;
        name_board_views(&mut scene);
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        crate::dxf_dec::session(source, scene, Format::Ipc2581, parse_ms)
    }
}

/// KiCad boards (`Format::KicadPcb`) and schematics (`Format::KicadSch`).
pub struct KicadDecoder(pub Format);

/// Bytes of a schematic's sheet file read at most.
const MAX_SHEET_BYTES: u64 = 64 << 20;

impl Decoder for KicadDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Format::KicadSch => specular_kicad::is_kicad_sch(head),
            _ => specular_kicad::is_kicad_pcb(head),
        }
    }
    /// Text formats: their signs may come after comments or a long prolog.
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(crate::PROBE_TEXT)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::KicadSch => &["kicad_sch"],
            _ => &["kicad_pcb"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let data = source.bytes();
        let head = &data[..data.len().min(64 << 10)];
        if let Some(kind) = specular_kicad::legacy_kind(head) {
            return Err(DecodeError::Unsupported(kind.to_owned()));
        }
        if !self.probe(head) {
            return Err(DecodeError::UnsupportedFormat);
        }
        let limits = specular_kicad::Limits::default();
        let mut scene = match self.0 {
            Format::KicadSch => {
                let dir = source.path().parent().map(Path::to_path_buf);
                let on_disk = crate::geo::on_disk(&source);
                let resolve = |name: &str| -> Option<Vec<u8>> {
                    let dir = dir.as_ref().filter(|_| on_disk)?;
                    let p = dir.join(name);
                    let m = std::fs::metadata(&p).ok()?;
                    (m.is_file() && m.len() <= MAX_SHEET_BYTES)
                        .then(|| std::fs::read(&p).ok())
                        .flatten()
                };
                specular_kicad::sch::scene(data, &resolve, &limits).map_err(kicad_err)?
            }
            _ => specular_kicad::pcb::scene(data, &limits).map_err(kicad_err)?,
        };
        if self.0 == Format::KicadSch {
            // Sheets are shown by their names, not as model space.
            for s in &mut scene.sheets {
                s.kind = SheetKind::Layout;
            }
        }
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        crate::dxf_dec::session(source, scene, self.0, parse_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gerber and drill text is claimed by these decoders and no other before them.
    #[test]
    fn electronics_text_is_ours_first() {
        let reg = crate::Registry::default();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-pcb/tests/data");
        for (name, want) in [("x2.gbr", Format::Gerber), ("kicad.drl", Format::Excellon)] {
            let data = std::fs::read(dir.join(name)).expect("fixture");
            let first = reg
                .decoders
                .iter()
                .find(|d| d.probe_file(&data))
                .map(|d| d.format());
            assert_eq!(first, Some(want), "{name}");
        }
    }
}
