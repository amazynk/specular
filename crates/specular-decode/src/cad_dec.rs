//! CAD exchange files, and DWG through the ODA File Converter.
//!
//! - **STEP and IGES** are recognised (STEP by its "ISO-10303-21;" first line, IGES by
//!   extension) and read by OpenCASCADE in the sandboxed decode host ([`crate::occt_dec`]).
//! - **IFC** shares STEP's first line but describes buildings in its own entities (walls,
//!   extrusions, openings), which OpenCASCADE's STEP reader does not know: Specular's own IFC
//!   reader draws it ([`crate::ifc_dec`]).
//! - **DWG** is opened through the ODA File Converter, an optional decoder
//!   ([`crate::optional`]): used when it is installed on this computer, unless the user
//!   switched it off ([`set_dwg_converter`]). The converter is never bundled or downloaded. A
//!   copy of the drawing goes into a private folder under the app's cache, the converter
//!   writes a DXF beside it, the DXF is read by the DXF path, and the folder is removed.
//!   Without the converter the file is refused with [`DecodeError::NeedsDecoder`] (the app
//!   offers to get it); switched off, a plain refusal says where to switch it on.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use specular_core::Source;

use crate::{DecodeError, Decoder, Format, Result, Session};

pub struct CadExchangeDecoder;

impl Decoder for CadExchangeDecoder {
    fn format(&self) -> Format {
        Format::CadExchange
    }
    fn probe(&self, head: &[u8]) -> bool {
        head.trim_ascii_start().starts_with(b"ISO-10303-21;")
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ifc", "ifczip", "ifcxml", "stp", "step", "igs", "iges"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let ext = source
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        match kind(&ext, source.bytes()) {
            Kind::Ifc => crate::ifc_dec::open(source),
            Kind::Iges => crate::occt_dec::open(source, specular_sandbox::CadFormat::Iges),
            Kind::Step => crate::occt_dec::open(source, specular_sandbox::CadFormat::Step),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Step,
    Iges,
    Ifc,
}

/// Which exchange format a file is: IGES by extension; IFC by extension or by its
/// FILE_SCHEMA (`IFC2X3`, `IFC4`…) in the header; otherwise STEP.
fn kind(ext: &str, bytes: &[u8]) -> Kind {
    if matches!(ext, "igs" | "iges") {
        return Kind::Iges;
    }
    if ext.starts_with("ifc") {
        return Kind::Ifc;
    }
    let head = bytes.get(..8192).unwrap_or(bytes);
    let schema = head
        .windows(11)
        .position(|w| w == b"FILE_SCHEMA")
        .map(|i| &head[i..(i + 200).min(head.len())]);
    if schema.is_some_and(|s| s.windows(4).any(|w| w.eq_ignore_ascii_case(b"'IFC"))) {
        Kind::Ifc
    } else {
        Kind::Step
    }
}

/// How DWG files may be opened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DwgConverter {
    /// The user allows running the converter (Settings › Optional decoders; on unless
    /// switched off).
    pub enabled: bool,
    /// The converter's program, when the user named one; else it is looked for where its
    /// installers put it.
    pub program: Option<PathBuf>,
    /// The app's cache folder: private working folders are made under it.
    pub cache: PathBuf,
}

static DWG: RwLock<Option<DwgConverter>> = RwLock::new(None);

/// Sets how DWG files may be opened (the app calls this from its settings).
pub fn set_dwg_converter(c: Option<DwgConverter>) {
    *DWG.write() = c;
}

/// The ODA File Converter's program on this computer, if found: `named` when it is a file,
/// else where the official installers put it, else on the PATH.
pub fn find_oda_converter(named: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = named {
        return p.is_file().then(|| p.to_path_buf());
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    let exe = if cfg!(windows) {
        "ODAFileConverter.exe"
    } else {
        "ODAFileConverter"
    };
    let versioned = |root: &Path, out: &mut Vec<PathBuf>| {
        // Folders named "ODAFileConverter 25.x.0" and the like, newest first.
        if let Ok(rd) = std::fs::read_dir(root) {
            let mut dirs: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("ODAFileConverter"))
                })
                .collect();
            dirs.sort();
            dirs.reverse();
            out.extend(dirs.into_iter().map(|d| d.join(exe)));
        }
    };
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(var) {
                versioned(&PathBuf::from(pf).join("ODA"), &mut candidates);
            }
        }
    } else if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from(
            "/Applications/ODAFileConverter.app/Contents/MacOS/ODAFileConverter",
        ));
    } else {
        candidates.push(PathBuf::from("/usr/bin/ODAFileConverter"));
        candidates.push(PathBuf::from("/usr/local/bin/ODAFileConverter"));
        versioned(Path::new("/opt"), &mut candidates);
        versioned(Path::new("/opt/ODAFileConverter"), &mut candidates);
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Longest a conversion may run.
const TIMEOUT: Duration = Duration::from_secs(120);

pub struct DwgDecoder;

impl Decoder for DwgDecoder {
    fn format(&self) -> Format {
        Format::Dwg
    }
    /// "AC10" and a release number.
    fn probe(&self, head: &[u8]) -> bool {
        head.len() >= 6 && head.starts_with(b"AC10") && head[4..6].iter().all(u8::is_ascii_digit)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dwg"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let cfg = DWG.read().clone().unwrap_or_default();
        let program = find_oda_converter(cfg.program.as_deref());
        let program = match (cfg.enabled, program) {
            (true, Some(p)) => p,
            (false, Some(_)) => {
                return Err(DecodeError::Unsupported(
                    "DWG drawings open with the ODA File Converter, which is switched off in \
                     Settings › Optional decoders"
                        .into(),
                ));
            }
            (_, None) => return Err(DecodeError::NeedsDecoder(crate::OptionalId::Dwg)),
        };
        let t0 = Instant::now();
        let dxf = convert(&program, &cfg.cache, source.bytes())?;
        let scene =
            specular_cad::parse(&dxf, &specular_cad::Limits::default()).map_err(|e| match e {
                specular_cad::CadError::TooLarge => DecodeError::TooLarge,
                specular_cad::CadError::Malformed(m) => {
                    DecodeError::Malformed(format!("converted DXF: {m}"))
                }
                _ => DecodeError::Malformed("the converter wrote no DXF".into()),
            })?;
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        crate::dxf_dec::session(source, scene, Format::Dwg, ms)
    }
}

/// A private working folder, removed when dropped.
struct WorkDir(PathBuf);

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn private_dir(cache: &Path) -> Result<WorkDir> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let root = if cache.as_os_str().is_empty() {
        return Err(DecodeError::Unsupported(
            "DWG conversion has no working folder".into(),
        ));
    } else {
        cache.join("dwg")
    };
    std::fs::create_dir_all(&root).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    let dir = root.join(format!(
        "{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    #[cfg_attr(not(unix), allow(unused_mut))] // `mode` is Unix-only
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(&dir)
        .map_err(|e| DecodeError::Malformed(format!("DWG working folder: {e}")))?;
    Ok(WorkDir(dir))
}

/// Runs the converter on a copy of `dwg` in a private folder; the DXF it writes.
fn convert(program: &Path, cache: &Path, dwg: &[u8]) -> Result<Vec<u8>> {
    let work = private_dir(cache)?;
    let (inp, out) = (work.0.join("in"), work.0.join("out"));
    for d in [&inp, &out] {
        std::fs::create_dir(d).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    }
    std::fs::write(inp.join("drawing.dwg"), dwg)
        .map_err(|e| DecodeError::Malformed(format!("DWG copy: {e}")))?;
    // ODAFileConverter <in> <out> <version> <type> <recurse> <audit> [filter]
    let mut child = std::process::Command::new(program)
        .arg(&inp)
        .arg(&out)
        .arg("ACAD2018")
        .arg("DXF")
        .arg("0")
        .arg("1")
        .arg("*.DWG")
        // The converter is a Qt program: no window.
        .env("QT_QPA_PLATFORM", "offscreen")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            DecodeError::Unsupported(format!("the ODA File Converter did not start: {e}"))
        })?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DecodeError::Malformed(
                    "the ODA File Converter took too long".into(),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(DecodeError::Malformed(e.to_string())),
        }
    }
    let dxf = std::fs::read_dir(&out)
        .ok()
        .and_then(|rd| {
            rd.flatten().map(|e| e.path()).find(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("dxf"))
            })
        })
        .ok_or_else(|| DecodeError::Malformed("the ODA File Converter wrote no DXF".into()))?;
    std::fs::read(dxf).map_err(|e| DecodeError::Malformed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    fn open(name: &str, bytes: &[u8]) -> Result<Arc<dyn Session>> {
        Registry::default().open(Arc::new(Source::from_vec(name, bytes.to_vec())))
    }

    /// The converter setting is process-wide: its tests take turns.
    static TURN: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

    #[test]
    fn exchange_kinds() {
        let step = b"ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\n";
        assert_eq!(kind("stp", step), Kind::Step);
        assert_eq!(kind("dat", step), Kind::Step);
        assert_eq!(
            kind("step", b"ISO-10303-21;\nFILE_SCHEMA(('IFC4'));"),
            Kind::Ifc
        );
        assert_eq!(kind("ifczip", b"PK"), Kind::Ifc);
        assert_eq!(kind("IGS".to_ascii_lowercase().as_str(), b""), Kind::Iges);
        // "IFC" elsewhere in the header (a file name) does not make it IFC.
        let named = b"ISO-10303-21;\nFILE_NAME('IFC-test.stp');\nFILE_SCHEMA(('AP242'));";
        assert_eq!(kind("stp", named), Kind::Step);
    }

    #[test]
    fn exchange_files_are_refused_with_the_reason() {
        let _turn = TURN.lock();
        // IFC goes to its own reader: one without elements is refused with the reason.
        let e = open(
            "house.ifc",
            b"ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\nENDSEC;\n",
        )
        .err()
        .expect("refused")
        .to_string();
        assert!(e.contains("IFC model has no elements"), "{e}");
        // STEP and IGES with OpenCASCADE turned off: the setting is named.
        crate::set_occt(Some(crate::OcctConfig {
            enabled: false,
            library_dir: None,
        }));
        let step = b"ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\n";
        for (name, bytes, what) in [
            ("part.stp", &step[..], "STEP"),
            ("part.igs", b"   S      1", "IGES"),
        ] {
            let e = open(name, bytes).err().expect("refused").to_string();
            assert!(
                e.contains(what) && e.contains("OpenCASCADE") && e.contains("Settings"),
                "{name}: {e}"
            );
        }
        crate::set_occt(None);
    }

    #[test]
    fn dwg_without_the_converter_says_what_is_needed() {
        let _turn = TURN.lock();
        // A named program that is not there stands for "not installed".
        set_dwg_converter(Some(DwgConverter {
            enabled: true,
            program: Some(PathBuf::from("/nonexistent/ODAFileConverter")),
            cache: std::env::temp_dir(),
        }));
        let e = open("plan.dwg", b"AC1032\0\0\0\0").err().expect("refused");
        assert!(
            matches!(e, DecodeError::NeedsDecoder(crate::OptionalId::Dwg)),
            "{e}"
        );
        let e = e.to_string();
        assert!(
            e.contains("ODA File Converter") && e.contains("not installed"),
            "{e}"
        );
        set_dwg_converter(None);
        assert!(DwgDecoder.probe(b"AC1018xxxx"));
        assert!(!DwgDecoder.probe(b"AC10xx"));
    }

    /// A stand-in converter (a shell script) writes a DXF: the drawing opens through the DXF
    /// path, and the working folder is gone afterwards.
    #[cfg(unix)]
    #[test]
    fn dwg_opens_through_a_converter() {
        use std::os::unix::fs::PermissionsExt;
        let _turn = TURN.lock();
        let root = std::env::temp_dir().join(format!("specular-dwg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n10\n21\n5\n31\n0\n0\nENDSEC\n0\nEOF\n";
        std::fs::write(root.join("out.dxf"), dxf).unwrap();
        let script = root.join("ODAFileConverter");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\ncp '{}' \"$2/drawing.dxf\"\n",
                root.join("out.dxf").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cache = root.join("cache");
        set_dwg_converter(Some(DwgConverter {
            enabled: false,
            program: Some(script.clone()),
            cache: cache.clone(),
        }));
        let e = open("plan.dwg", b"AC1032\0\0")
            .err()
            .expect("off")
            .to_string();
        assert!(e.contains("Settings › Optional decoders"), "{e}");
        set_dwg_converter(Some(DwgConverter {
            enabled: true,
            program: Some(script),
            cache: cache.clone(),
        }));
        let s = open("plan.dwg", b"AC1032\0\0").unwrap();
        assert_eq!(s.info().format, Format::Dwg);
        assert!(s.drawing().is_some());
        let left: Vec<_> = std::fs::read_dir(cache.join("dwg")).unwrap().collect();
        assert!(left.is_empty(), "working folders left: {left:?}");
        set_dwg_converter(None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
