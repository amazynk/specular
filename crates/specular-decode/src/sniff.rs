//! Folder listings: files whose extension alone does not say whether we read them.
//!
//! - DICOM Part 10 files without a known extension (patient discs name them "IM000001",
//!   "I0000001" or by UID, "1.2.840...5"). Only files with no extension or an all-digit one
//!   are read: 132 bytes, for `DICM` at offset 128.
//! - `.map` and `.map.gz` files, which are CCP4/MRC density maps only when their 1024-byte
//!   header says so ("MAP " at byte 208 and a sane size and mode); other `.map` files
//!   (linker maps, game levels, GIS) are never listed.
//! - `.cube` files, which are Gaussian cube volumes only when their header lines say so
//!   (colour lookup tables share the extension), and `.bag` files, which are BAG grids only
//!   when they are HDF5 naming `BAG_root` (ROS bags share the extension).
//! - `.img`, `.edf` and all-digit extensions, which are diffraction detector frames (SMV,
//!   R-AXIS, ESRF EDF, Bruker) only when their header says so.
//! - `.log` files: a Bruker SkyScan reconstruction's log (the volume of the slices beside it)
//!   or plain text.
//! - `.sub` files beside a VobSub `.idx`, which are left out: the `.idx` opens the pair.
//! - `.raw` and `.vol` files, which are left out when they are the data of a header beside
//!   them (`.xtekct`, `.xtekVolume`, `.vgi`, `.mhd`, `.nhdr`): the header is the volume.
//! - Planetary images: `.img` and `.qub` files with an attached PDS3 label or a VICAR label,
//!   and `.xml` files that are PDS4 observational labels; `.lbl` files that are not PDS3
//!   labels (label-printer files) are left out.
//!
//! Answers are cached by path, size and modification time.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

/// Entries before the cache starts over (about 10 MB).
const CACHE_MAX: usize = 100_000;

/// Compressed bytes read from a `.map.gz` to inflate its 1024-byte header.
const GZ_HEAD: u64 = 128 << 10;

type Key = (u64, Option<SystemTime>);

static CACHE: LazyLock<Mutex<HashMap<PathBuf, (Key, bool)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether a file named like `path` might be an extensionless DICOM file: no extension, or an
/// all-digit one (UID file names); not hidden.
pub fn dicom_candidate(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if name.starts_with('.') {
        return false;
    }
    match path.extension().and_then(|e| e.to_str()) {
        None => true,
        Some(e) => !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit()),
    }
}

/// Whether `path` is named `.map` (`Some(false)`) or `.map.gz` (`Some(true)`).
pub fn map_candidate(path: &Path) -> Option<bool> {
    let [compound, ext] = crate::path_extensions(path);
    if compound.as_deref() == Some("map.gz") {
        Some(true)
    } else if ext.as_deref() == Some("map") {
        Some(false)
    } else {
        None
    }
}

/// Whether the start of a file (inflated first when it is gzip) is a CCP4/MRC header.
pub fn is_mrc_head(head: &[u8]) -> bool {
    if head.starts_with(&[0x1f, 0x8b]) {
        return specular_png::inflate_gzip_prefix(head, specular_em::mrc::HEADER)
            .is_ok_and(|h| specular_em::mrc::sniff(&h));
    }
    specular_em::mrc::sniff(head)
}

/// Reads at most `n` bytes from the start of `path`.
fn read_head(path: &Path, n: u64) -> Option<Vec<u8>> {
    let mut head = Vec::new();
    specular_core::open_regular(path)
        .ok()?
        .take(n)
        .read_to_end(&mut head)
        .ok()?;
    Some(head)
}

/// Runs `check` on `path` once per size and modification time.
fn cached(path: &Path, min_len: u64, check: impl FnOnce() -> bool) -> bool {
    cached_in(&CACHE, path, min_len, check)
}

/// [`cached`] with its own cache (a path may be asked two questions: a numbered Bruker
/// frame is also a DICOM candidate).
fn cached_in(
    cache: &Mutex<HashMap<PathBuf, (Key, bool)>>,
    path: &Path,
    min_len: u64,
    check: impl FnOnce() -> bool,
) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() < min_len {
        return false;
    }
    let key = (meta.len(), meta.modified().ok());
    if let Some((k, v)) = cache.lock().ok().and_then(|c| c.get(path).copied())
        && k == key
    {
        return v;
    }
    let v = check();
    if let Ok(mut c) = cache.lock() {
        if c.len() >= CACHE_MAX {
            c.clear();
        }
        c.insert(path.to_path_buf(), (key, v));
    }
    v
}

/// Whether `path`, a file without a known image extension, is a DICOM Part 10 file.
pub fn is_extensionless_dicom(path: &Path) -> bool {
    if !dicom_candidate(path) {
        return false;
    }
    cached(path, 132, || {
        read_head(path, 132).is_some_and(|h| h.len() == 132 && &h[128..132] == b"DICM")
    })
}

/// Whether `path`, named `.map` or `.map.gz`, is a CCP4/MRC density map.
pub fn is_mrc_map(path: &Path) -> bool {
    let Some(gz) = map_candidate(path) else {
        return false;
    };
    let n = if gz {
        GZ_HEAD
    } else {
        specular_em::mrc::HEADER as u64
    };
    cached(path, if gz { 20 } else { n }, || {
        read_head(path, n).is_some_and(|h| is_mrc_head(&h))
    })
}

/// Bytes read from a `.bag` to find its `BAG_root` group name.
const BAG_HEAD: u64 = 64 << 10;
/// Bytes read from a `.cube` to check its header lines.
const CUBE_HEAD: u64 = 16 << 10;

/// Whether the start of a file is an HDF5 file naming a `BAG_root` group (a BAG bathymetry
/// grid; ROS bags share the extension).
pub fn is_bag_head(head: &[u8]) -> bool {
    specular_hdf5::find_superblock(head).is_some() && head.windows(8).any(|w| w == b"BAG_root")
}

/// Whether `path`, named `.bag`, is a BAG grid.
pub fn is_bag(path: &Path) -> bool {
    if crate::path_extensions(path)[1].as_deref() != Some("bag") {
        return false;
    }
    cached(path, 512, || {
        read_head(path, BAG_HEAD).is_some_and(|h| is_bag_head(&h))
    })
}

/// Whether `path`, named `.cube`, is a Gaussian cube file (not a colour lookup table).
pub fn is_gaussian_cube(path: &Path) -> bool {
    if crate::path_extensions(path)[1].as_deref() != Some("cube") {
        return false;
    }
    cached(path, 64, || {
        read_head(path, CUBE_HEAD).is_some_and(|h| specular_em::cube::sniff(&h))
    })
}

/// Whether `path` is the `info.json` of a CNR-ISTI Relight image (opens the folder's image).
pub fn is_relight_info(path: &Path) -> bool {
    if path.file_name().and_then(|n| n.to_str()) != Some("info.json") {
        return false;
    }
    cached(path, 16, || {
        read_head(path, 4 << 10).is_some_and(|h| specular_core::is_relight_head(&h))
    })
}

/// Whether `path` is the `table.dat` of a CASA image (opens the folder's image; the
/// folder's other files are not listed).
pub fn is_casa_table(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()) == Some("table.dat")
        && path.parent().is_some_and(specular_core::is_casa_folder)
}

static XTAL_CACHE: LazyLock<Mutex<HashMap<PathBuf, (Key, bool)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether a file named like `path` might be a diffraction frame that only its content
/// identifies: `.img` (SMV, R-AXIS; disk images share it), `.edf` (ESRF; EEG recordings
/// share it) or an all-digit extension (Bruker's numbered frames, `.0001`).
pub fn detector_candidate(path: &Path) -> bool {
    let [compound, ext] = crate::path_extensions(path);
    let frame_ext = |e: &str| {
        matches!(e, "img" | "edf") || (!e.is_empty() && e.bytes().all(|b| b.is_ascii_digit()))
    };
    match ext.as_deref() {
        // Archived frames compressed whole: `.img.Z`, `.cbf.Z`, `.0001.Z`, `.img.gz`.
        Some("z" | "gz") => compound.as_deref().is_some_and(|c| {
            let inner = c.rsplit_once('.').map_or("", |(i, _)| i);
            frame_ext(inner)
                || (c.ends_with(".z") && matches!(inner, "cbf" | "sfrm" | "mccd" | "osc"))
        }),
        Some(e) => frame_ext(e),
        None => false,
    }
}

/// Whether `path`, named `.img`, `.edf` or `.NNNN` (or one of them compressed whole by
/// gzip or compress), is a diffraction detector frame.
pub fn is_detector_frame(path: &Path) -> bool {
    if !detector_candidate(path) {
        return false;
    }
    cached_in(&XTAL_CACHE, path, 64, || {
        read_head(path, specular_xtal::SNIFF_HEAD as u64).is_some_and(|h| {
            specular_xtal::sniff(&h).is_some() || specular_xtal::sniff_gzip(&h).is_some()
        })
    })
}

/// The Analyze 7.5 or NIfTI-pair `.hdr` beside `path` (an `.img`), when there is one: the
/// `.img` is its data.
pub fn analyze_header_of(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    let dir = path.parent().unwrap_or(Path::new(""));
    ["hdr", "HDR", "Hdr"].iter().find_map(|e| {
        let h = dir.join(format!("{stem}.{e}"));
        let head = read_head(&h, specular_medvol::HEAD as u64)?;
        matches!(
            specular_medvol::sniff(&head),
            Some(
                specular_medvol::Kind::Analyze
                    | specular_medvol::Kind::Nifti1
                    | specular_medvol::Kind::Nifti2
            )
        )
        .then_some(h)
    })
}

/// Whether `path` is a Zeiss Cirrus OCT raw export (by its name and size; it has no
/// header).
pub fn is_cirrus(path: &Path) -> bool {
    // The name first: only an `.img` is stat'ed for its size.
    crate::path_extensions(path)[1].as_deref() == Some("img")
        && std::fs::metadata(path)
            .is_ok_and(|m| m.is_file() && specular_medvol::cirrus::scan(path, m.len()).is_some())
}

/// Bytes read from an `.asc` or `.img` to check its header.
const GRID_HEAD: u64 = 4 << 10;

/// Whether `path` is a georeferenced grid its extension does not give away: an `.asc`
/// that is an Esri ASCII grid (not PGP armour), an `.img` that is ERDAS IMAGINE, or raw
/// data (`.img`, `.dat`, `.raw`…) beside an ENVI or ESRI `.hdr` header.
pub fn is_georaster(path: &Path) -> bool {
    let ext = crate::path_extensions(path)[1].clone().unwrap_or_default();
    let raw = specular_gis::envi::DATA_EXTENSIONS.contains(&ext.as_str()) && !ext.is_empty();
    if ext != "asc" && !raw {
        return false;
    }
    cached(path, 16, || {
        let head = || read_head(path, GRID_HEAD).unwrap_or_default();
        match ext.as_str() {
            "asc" => specular_gis::asc::sniff(&head()),
            "img" => specular_gis::hfa::sniff(&head()) || crate::georaster_dec::envi_data(path),
            _ => crate::georaster_dec::envi_data(path),
        }
    })
}

/// Bytes read from a `.log` to recognise a SkyScan reconstruction log.
const LOG_HEAD: u64 = 16 << 10;

/// Whether `path`, named `.log`, is a Bruker SkyScan reconstruction log.
pub fn is_skyscan_log(path: &Path) -> bool {
    if crate::path_extensions(path)[1].as_deref() != Some("log") {
        return false;
    }
    cached(path, 16, || {
        read_head(path, LOG_HEAD).is_some_and(|h| specular_medvol::skyscan::sniff(&h))
    })
}

/// Whether `path` is an `.scc` that is an MSX screen rather than Scenarist captions (which
/// always start `Scenarist_SCC`): such a file is a picture, not a text file.
pub fn is_msx_scc(path: &Path) -> bool {
    crate::path_extensions(path)[1].as_deref() == Some("scc")
        && read_head(path, 13).is_some_and(|h| !h.starts_with(b"Scenarist_SCC"))
}

/// Whether `path` is the pictures of a VobSub pair (a `.sub` with its `.idx` beside it): the
/// `.idx` stands for the pair in folders.
pub fn is_vobsub_pictures(path: &Path) -> bool {
    crate::path_extensions(path)[1].as_deref() == Some("sub")
        && specular_subs::idx_of(path).is_some()
}

/// Whether `path`, named `.v`, is an ECAT 7 image (the extension is Verilog's too).
pub fn is_ecat(path: &Path) -> bool {
    if crate::path_extensions(path)[1].as_deref() != Some("v") {
        return false;
    }
    cached(path, 512, || {
        read_head(path, 512).is_some_and(|h| specular_medvol::ecat::sniff(&h))
    })
}

/// [`is_volume_sidecar`]'s answers, apart from [`CACHE`]: a `.raw` is also asked whether it
/// is ENVI data ([`is_georaster`]).
static SIDECAR_CACHE: LazyLock<Mutex<HashMap<PathBuf, (Key, bool)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether `path`, named `.raw` or `.vol`, holds the samples of a header beside it, or is a
/// Philips `.REC` beside its `.PAR`.
pub fn is_volume_sidecar(path: &Path) -> bool {
    if specular_medvol::parrec::is_rec_name(path) {
        return specular_medvol::parrec::par_of(path).is_some();
    }
    if !specular_medvol::raw::is_raw_name(path) {
        return false;
    }
    cached_in(&SIDECAR_CACHE, path, 0, || {
        specular_medvol::raw::sibling_header(path).is_some()
    })
}

static PLANETARY_CACHE: LazyLock<Mutex<HashMap<PathBuf, (Key, bool)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Bytes read to recognise a planetary label.
const PLANETARY_HEAD: u64 = specular_planetary::pds4::SNIFF as u64;

/// Whether `path` is a planetary image its extension does not give away: an `.img` or `.qub`
/// starting with a PDS3 or VICAR label, or an `.xml` PDS4 observational label.
pub fn is_planetary(path: &Path) -> bool {
    let ext = crate::path_extensions(path)[1].clone().unwrap_or_default();
    if !matches!(ext.as_str(), "img" | "qub" | "xml") {
        return false;
    }
    cached_in(&PLANETARY_CACHE, path, 16, || {
        let Some(h) = read_head(path, PLANETARY_HEAD) else {
            return false;
        };
        match ext.as_str() {
            "xml" => specular_planetary::pds4::sniff_observational(&h),
            _ => specular_planetary::vicar::sniff(&h) || specular_planetary::pds3::sniff(&h),
        }
    })
}

/// Whether `path`, named `.lbl`, is not a PDS3 label (a label printer's file, say).
pub fn is_foreign_label(path: &Path) -> bool {
    if crate::path_extensions(path)[1].as_deref() != Some("lbl") {
        return false;
    }
    !cached_in(&PLANETARY_CACHE, path, 0, || {
        read_head(path, specular_planetary::pds3::SNIFF as u64)
            .is_some_and(|h| specular_planetary::pds3::sniff(&h))
    })
}

static RETRO_CACHE: LazyLock<Mutex<HashMap<PathBuf, (Key, bool)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether `path` is a GEM bit image (`.img`, shared with disk images) or an X cursor (no
/// extension), by their headers.
pub fn is_retro(path: &Path) -> bool {
    let ext = crate::path_extensions(path)[1].clone();
    let want: fn(&[u8]) -> bool = match ext.as_deref() {
        Some("img") => specular_retro::gem::probe,
        None => specular_retro::xcursor::probe,
        _ => return false,
    };
    cached_in(&RETRO_CACHE, path, 16, || {
        read_head(path, 64).is_some_and(|h| want(&h))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planetary_files() {
        let dir = std::env::temp_dir().join(format!("specular-sniff-pds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let w = |n: &str, b: &[u8]| std::fs::write(dir.join(n), b).unwrap();
        w("a.IMG", b"PDS_VERSION_ID = PDS3\r\nRECORD_BYTES = 512\r\n");
        w("b.img", b"LBLSIZE=2000            FORMAT='BYTE'");
        w("disk.img", &[0u8; 512]);
        w("p.xml", br#"<?xml version="1.0"?><Product_Observational xmlns="http://pds.nasa.gov/pds4/pds/v1">"#);
        w(
            "c.xml",
            br#"<?xml version="1.0"?><Product_Collection xmlns="http://pds.nasa.gov/pds4/pds/v1">"#,
        );
        w("pds.lbl", b"PDS_VERSION_ID = PDS3\n");
        w("printer.lbl", b"\x00\x01label printer");
        for (n, want) in [
            ("a.IMG", true),
            ("b.img", true),
            ("disk.img", false),
            ("p.xml", true),
            ("c.xml", false),
            ("pds.lbl", false),
        ] {
            assert_eq!(is_planetary(&dir.join(n)), want, "{n}");
        }
        assert!(!is_foreign_label(&dir.join("pds.lbl")));
        assert!(is_foreign_label(&dir.join("printer.lbl")));
        let reg = crate::Registry::new();
        assert!(reg.supports_file(&dir.join("a.IMG")));
        assert!(reg.supports_file(&dir.join("pds.lbl")));
        assert!(!reg.supports_file(&dir.join("printer.lbl")));
        assert!(!reg.supports_file(&dir.join("c.xml")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Telling the files of a folder that are not images (sidecars, notes) costs about one
    /// `stat` each, not a probe for every folder format: [`crate::Registry::supports_file`]
    /// on them against `symlink_metadata` of the same files, paired rounds, best of each.
    #[test]
    fn non_images_cost_about_one_stat_each() {
        let dir = std::env::temp_dir().join(format!("specular-sniffcost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let names: Vec<std::path::PathBuf> = (0..1500)
            .flat_map(|i| {
                [
                    format!("DSC_{i:04}.NEF.xmp"),
                    format!("IMG_{i:04}.CR2.pp3"),
                    format!("IMG_{i:04}.dop"),
                ]
            })
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"<x:xmpmeta/>").unwrap();
                p
            })
            .collect();
        let reg = crate::Registry::new();
        let max = std::time::Duration::MAX;
        let (mut probes, mut known, mut stats) = (max, max, max);
        for _ in 0..11 {
            let t0 = std::time::Instant::now();
            let n = names.iter().filter(|p| reg.supports_file(p)).count();
            probes = probes.min(t0.elapsed());
            // As a listing asks, knowing each is a file.
            let t0 = std::time::Instant::now();
            let k = names
                .iter()
                .filter(|p| reg.supports_entry(p, Some(false)))
                .count();
            known = known.min(t0.elapsed());
            assert_eq!(k, 0);
            let t0 = std::time::Instant::now();
            let m = names
                .iter()
                .filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
                .count();
            stats = stats.min(t0.elapsed());
            assert_eq!((n, m), (0, 0));
        }
        let ratio = probes.as_secs_f64() / stats.as_secs_f64();
        let ratio_known = known.as_secs_f64() / stats.as_secs_f64();
        eprintln!(
            "{} non-images: supports_file {probes:.2?} ({ratio:.2}×), as files {known:.2?} \
             ({ratio_known:.2}×), stat {stats:.2?}",
            names.len()
        );
        // Before: every folder probe ran for each file, about 12× a stat (8 failed system
        // calls and a scan of every decoder's extensions); now 2-4× in a test build.
        assert!(ratio < 6.0, "supports_file costs {ratio:.2}× a stat");
        assert!(
            ratio_known < 6.0,
            "supports_entry costs {ratio_known:.2}× a stat"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An extensionless FIFO is not opened to sniff it (opening one blocks until a writer
    /// comes, and the gallery's listing sniffs every extensionless name).
    #[cfg(unix)]
    #[test]
    fn fifos_are_not_sniffed() {
        let d = std::env::temp_dir().join(format!("specular-sniff-fifo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let fifo = d.join("IM0002");
        let ok = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "mkfifo");
        let (tx, rx) = std::sync::mpsc::channel();
        let p = fifo.clone();
        #[allow(clippy::disallowed_methods)]
        std::thread::spawn(move || {
            let _ = tx.send((is_extensionless_dicom(&p), read_head(&p, 16).is_none()));
        });
        let r = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("blocked on a FIFO");
        assert_eq!(r, (false, true));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn detector_frames() {
        for p in [
            "a.img",
            "b.EDF",
            "frame.0001",
            "x/y.123",
            "a.img.Z",
            "c.cbf.Z",
            "d.0001.Z",
            "e.img.gz",
        ] {
            assert!(detector_candidate(Path::new(p)), "{p}");
        }
        for p in [
            "a.cbf", "b.jpg", "README", "x.5a", "x.tar.gz", "y.Z", "c.cbf.gz",
        ] {
            assert!(!detector_candidate(Path::new(p)), "{p}");
        }
        let data =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-xtal/tests/data");
        for f in [
            "adsc.img",
            "plate.img",
            "saxs.edf",
            "frame86.0001",
            "adsc32.img.Z",
            "noise.img.Z",
        ] {
            assert!(is_detector_frame(&data.join(f)), "{f}");
        }
        // A numbered frame that is also a DICOM candidate: each question has its own answer.
        assert!(!is_extensionless_dicom(&data.join("frame86.0001")));
        assert!(is_detector_frame(&data.join("frame86.0001")));
    }

    #[test]
    fn candidates() {
        for p in [
            "IM000001",
            "DICOM/I0000001",
            "1.2.840.113619.2.55.5",
            "dir/7",
        ] {
            assert!(dicom_candidate(Path::new(p)), "{p}");
        }
        for p in [".hidden", "a.jpg", "notes.txt", "x.5a", "DICOMDIR.bak"] {
            assert!(!dicom_candidate(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn sniffs_and_caches() {
        let dir = std::env::temp_dir().join(format!("specular-sniff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut dicom = vec![0u8; 128];
        dicom.extend(b"DICM");
        dicom.extend([0u8; 64]);
        std::fs::write(dir.join("IM000001"), &dicom).unwrap();
        std::fs::write(dir.join("README"), vec![b'x'; 300]).unwrap();
        std::fs::write(dir.join("short"), b"DICM").unwrap();
        std::fs::write(dir.join("a.txt"), &dicom).unwrap();
        assert!(is_extensionless_dicom(&dir.join("IM000001")));
        assert!(is_extensionless_dicom(&dir.join("IM000001")));
        assert!(!is_extensionless_dicom(&dir.join("README")));
        assert!(!is_extensionless_dicom(&dir.join("short")));
        assert!(!is_extensionless_dicom(&dir.join("a.txt")));
        assert!(!is_extensionless_dicom(&dir.join("missing")));
        // A changed file is read again.
        std::fs::write(dir.join("IM000001"), vec![b'y'; 400]).unwrap();
        assert!(!is_extensionless_dicom(&dir.join("IM000001")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Listing cost of 10k-file folders, extension only vs. with the sniff:
    /// `cargo test -p specular-decode --lib sniff::tests::listing_10k -- --ignored --nocapture`
    /// (folders under TMPDIR).
    #[test]
    #[ignore]
    fn listing_10k() {
        use crate::Registry;
        use std::time::Instant;
        let reg = Registry::new();
        let root = std::env::temp_dir().join(format!("specular-sniff-10k-{}", std::process::id()));
        let mut dicom = vec![0u8; 128];
        dicom.extend(b"DICM");
        dicom.extend([0u8; 380]);
        let other = vec![b'x'; 512];
        let kinds: [(&str, &str, &[u8]); 4] = [
            ("photos", "IMG_{}.jpg", &other),
            ("mixed", "file{}", &other),
            ("disc", "IM{}", &dicom),
            ("uids", "1.2.840.10008.{}", &dicom),
        ];
        for (name, pattern, bytes) in kinds {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            for i in 0..10_000 {
                std::fs::write(dir.join(pattern.replace("{}", &format!("{i:06}"))), bytes).unwrap();
            }
            let list = |f: &dyn Fn(&Path) -> bool| {
                let t = Instant::now();
                let n = std::fs::read_dir(&dir)
                    .unwrap()
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()) && f(&e.path()))
                    .count();
                (n, t.elapsed().as_secs_f64() * 1e3)
            };
            let best = |f: &dyn Fn(&Path) -> bool| {
                let first = list(f);
                let rest = (0..4).map(|_| list(f).1).fold(f64::MAX, f64::min);
                (first.0, first.1, rest)
            };
            let (n0, _, before) = best(&|p| reg.supports_extension(p));
            let (n1, first, after) = best(&|p| reg.supports_file(p));
            eprintln!(
                "{name:7} 10k files: extension only {before:.1} ms ({n0} listed); with the sniff \
                 {first:.1} ms first, {after:.1} ms cached ({n1} listed)"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
