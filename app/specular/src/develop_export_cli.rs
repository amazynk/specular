//! `specular develop export FILE… [--preset NAME | --format …]`: exports
//! photos with their Develop edits (from the `.specular.xmp` sidecar beside each, else the
//! default edit) through `specular_develop::export`, without opening a window.
//!
//! Also the presets file the app's Export… sheet shares (`develop-export-presets.txt` in the
//! settings folder): the user's presets by name, and the sheet's last choices.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use specular_convert::{OutputDir, Resize};
use specular_core::CancelToken;
use specular_develop::export::Preset;
use specular_develop::export::batch::{self, Job, Outcome};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::{Edit, Registry, SourceKey};

/// The presets file in the settings folder.
const FILE: &str = "develop-export-presets.txt";
/// The id that marks the sheet's last choices in the file.
pub(crate) const LAST: &str = "last";

pub const USAGE: &str = "Develop export:\n  \
     develop export FILE… [OPTIONS]\n                         \
     write each photo with its Develop edit (its .specular.xmp\n                         \
     sidecar, else the default edit) as a new file, and exit\n    \
     --preset NAME        jpeg, web, tiff16, exr, or a preset saved in Export…\n    \
     --format F           jpeg, png, avif, jxl, tiff or exr\n    \
     --depth D            8, 16, half or float (as the format allows)\n    \
     --quality N          1–100 for JPEG, AVIF and JPEG XL\n    \
     --lossless           lossless JPEG XL\n    \
     --space S            srgb, display-p3, rec2020 or adobe-rgb\n    \
     --hdr H              off, pq, hlg (AVIF, JPEG XL, PNG) or gain-map (JPEG, AVIF, JPEG XL)\n    \
     --size S             full, N (long edge), WxH (fit within) or N%\n    \
     --metadata M         all, no-location or none\n    \
     --folder DIR         where files go (default: an export folder beside each)\n    \
     --name PATTERN       file names, e.g. {name}-{w} (as in Convert…)\n    \
     --skip-existing      skip an image whose name is taken (default: add a number)";

/// Where presets are kept (none in tests).
fn presets_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(specular_settings::Locations::detect().config_dir.join(FILE))
}

/// The user's presets and the sheet's last choices.
pub(crate) fn load_presets() -> (Vec<Preset>, Option<Preset>) {
    let Some(text) = presets_file().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return (Vec::new(), None);
    };
    let mut user = Vec::new();
    let mut last = None;
    for p in Preset::parse_all(&text) {
        if p.id == LAST {
            last = Some(p);
        } else if p.id.is_empty() && !p.name.is_empty() {
            user.push(p);
        }
    }
    (user, last)
}

/// Writes the user's presets and the last choices (atomically).
pub(crate) fn save_presets(user: &[Preset], last: &Preset) {
    let Some(path) = presets_file() else { return };
    let mut all = user.to_vec();
    all.push(Preset {
        id: LAST.into(),
        name: String::new(),
        ..last.clone()
    });
    let text = format!(
        "# Specular: Develop export presets. One [section] per preset.\n{}",
        Preset::write_all(&all)
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("txt.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Raw extensions (the raw front end joins the default edit).
fn is_raw(path: &Path) -> bool {
    const RAW: &[&str] = &[
        "3fr", "ari", "arw", "cr2", "cr3", "crw", "dcr", "dng", "erf", "fff", "iiq", "k25", "kdc",
        "mef", "mos", "mrw", "nef", "nrw", "orf", "pef", "raf", "raw", "rw2", "rwl", "sr2", "srf",
        "srw", "x3f",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RAW.contains(&e.to_ascii_lowercase().as_str()))
}

/// The photo's saved edit (its sidecar), else the default one.
fn edit_for(path: &Path) -> Edit {
    let reg = Registry::builtin();
    match specular_develop::sidecar::read(path, reg) {
        Ok(Some(l)) => l.edit,
        _ => Edit::default_for(reg, SourceKey::default(), is_raw(path)),
    }
}

/// What the command line asked for.
#[derive(Debug, PartialEq)]
struct Request {
    files: Vec<PathBuf>,
    preset: Preset,
}

/// `--size`: full, N, WxH or N%.
fn parse_size(s: &str) -> Option<Resize> {
    let s = s.trim();
    let n = |t: &str| t.parse::<u32>().ok().filter(|&v| v > 0);
    if s == "full" {
        return Some(Resize::None);
    }
    if let Some(p) = s.strip_suffix('%') {
        return n(p).filter(|&v| v <= 400).map(Resize::Percent);
    }
    if let Some((w, h)) = s.split_once(['x', 'X', '×']) {
        return Some(Resize::Fit {
            width: n(w)?,
            height: n(h)?,
        });
    }
    n(s).map(Resize::LongEdge)
}

/// Parses `develop export …` (after those two words).
fn parse(args: &[OsString], user: &[Preset]) -> Result<Request, String> {
    let mut files = Vec::new();
    let mut base: Option<Preset> = None;
    let mut set: Vec<(String, String)> = Vec::new();
    let mut it = args.iter();
    let mut only_files = false;
    while let Some(a) = it.next() {
        let s = a.to_string_lossy().into_owned();
        if only_files || !s.starts_with("--") {
            files.push(PathBuf::from(a));
            continue;
        }
        if s == "--" {
            only_files = true;
            continue;
        }
        let (name, inline) = match s.split_once('=') {
            Some((n, v)) => (n.to_owned(), Some(v.to_owned())),
            None => (s.clone(), None),
        };
        let mut value = || -> Result<String, String> {
            inline
                .clone()
                .or_else(|| it.next().map(|v| v.to_string_lossy().into_owned()))
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match name.as_str() {
            "--preset" => {
                let v = value()?;
                base =
                    Some(Preset::find(&v, user).ok_or_else(|| format!("no preset named “{v}”"))?);
            }
            "--lossless" => set.push(("lossless".into(), "true".into())),
            "--skip-existing" => set.push(("conflict".into(), "skip".into())),
            "--format" | "--depth" | "--quality" | "--space" | "--hdr" | "--size"
            | "--metadata" | "--folder" | "--name" => set.push((name.clone(), value()?)),
            _ => return Err(format!("unknown option {name}")),
        }
    }
    let mut p = base.unwrap_or_default();
    // The format first: it picks its own depth, which --depth may then change.
    set.sort_by_key(|(k, _)| k != "--format");
    for (k, v) in set {
        let ok = match k.as_str() {
            "--format" => {
                let ok = p.set("format", &v);
                // A format's own depth unless one is given.
                p.depth = p.format.depths()[0];
                ok
            }
            "--depth" => p.set("depth", &v),
            "--quality" => p.set("quality", &v),
            "--space" => p.set("space", &v),
            "--hdr" => p.set("hdr", &v) && p.hdr.fits(p.format),
            "--size" => match parse_size(&v) {
                Some(s) => {
                    p.size = s;
                    true
                }
                None => false,
            },
            "--metadata" => match v.as_str() {
                "all" => p.set("metadata", "all"),
                "no-location" => p.set("metadata", "no_location"),
                "none" => p.set("metadata", "strip"),
                _ => false,
            },
            "--folder" => {
                p.folder = OutputDir::Folder(PathBuf::from(&v));
                true
            }
            "--name" => {
                p.pattern = v.clone();
                specular_convert::Pattern::parse(&v).is_ok()
            }
            "lossless" => p.set("lossless", "true"),
            "conflict" => p.set("conflict", &v),
            _ => false,
        };
        if !ok {
            return Err(format!("bad value for {k}: {v}"));
        }
    }
    let wanted = p.depth;
    let p = p.normalised();
    // PQ and HLG pick their own depth.
    let hdr_depth = matches!(
        p.hdr,
        specular_develop::export::HdrOutput::Pq | specular_develop::export::HdrOutput::Hlg
    );
    if p.depth != wanted && !hdr_depth {
        return Err(format!(
            "{} cannot hold {} samples",
            p.format.id(),
            wanted.id()
        ));
    }
    if files.is_empty() {
        return Err("no files to export".into());
    }
    Ok(Request { files, preset: p })
}

/// Handles `specular develop …`. `Some(code)`: done, exit with `code`.
pub fn from_args() -> Option<i32> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.first().and_then(|a| a.to_str()) != Some("develop") {
        return None;
    }
    if args.get(1).and_then(|a| a.to_str()) != Some("export")
        || args.iter().any(|a| a == "-h" || a == "--help")
    {
        println!("Usage: specular {USAGE}");
        return Some(if args.len() <= 2 { 2 } else { 0 });
    }
    let (user, _) = load_presets();
    let req = match parse(&args[2..], &user) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("specular develop export: {e}");
            return Some(2);
        }
    };
    let (device, queue, integrated) = match batch::open_device() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("specular develop export: {e}");
            return Some(1);
        }
    };
    let jobs: Vec<Job> = req
        .files
        .iter()
        .map(|p| Job {
            source: p.clone(),
            edit: edit_for(p),
            orient: None,
        })
        .collect();
    let registry = specular_decode::Registry::new();
    let report = batch::run(
        &device,
        &queue,
        Budget::for_device(integrated, None, None),
        jobs,
        &req.preset,
        &registry,
        &CancelToken::new(),
        &|_| {},
    );
    let mut failed = false;
    for (src, o) in &report.results {
        match o {
            Outcome::Written { out, .. } => println!("{} → {}", src.display(), out.display()),
            Outcome::Skipped { existing } => {
                println!("{}: skipped, {} exists", src.display(), existing.display())
            }
            Outcome::Failed(f) => {
                failed = true;
                eprintln!("{}: {f}", src.display());
            }
            Outcome::Cancelled => failed = true,
        }
    }
    println!(
        "{} written, {} skipped, {} failed in {:.1} s",
        report.written(),
        report.skipped(),
        report.failed(),
        report.elapsed.as_secs_f64()
    );
    Some(i32::from(failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_convert::{Conflict, MetaMode};
    use specular_develop::export::{Depth, FileFormat, Space};

    fn args(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn options_make_the_preset() {
        let r = parse(
            &args(&[
                "a.CR3",
                "--format",
                "tiff",
                "--depth=float",
                "--space",
                "rec2020",
                "--size",
                "3000x2000",
                "b.NEF",
                "--folder",
                "/out",
                "--skip-existing",
            ]),
            &[],
        )
        .unwrap();
        assert_eq!(r.files, [PathBuf::from("a.CR3"), PathBuf::from("b.NEF")]);
        let p = r.preset;
        assert_eq!(
            (p.format, p.depth, p.space),
            (FileFormat::Tiff, Depth::Float, Space::Rec2020)
        );
        assert_eq!(
            p.size,
            Resize::Fit {
                width: 3000,
                height: 2000
            }
        );
        assert_eq!(p.folder, OutputDir::Folder("/out".into()));
        assert_eq!(p.conflict, Conflict::Skip);
        // A shipped preset, changed by an option.
        let r = parse(&args(&["x.dng", "--preset", "web", "--quality", "70"]), &[]).unwrap();
        assert_eq!(r.preset.size, Resize::LongEdge(2048));
        assert_eq!(r.preset.quality, 70);
        assert_eq!(r.preset.metadata, MetaMode::NoLocation);
        // A user's preset by name.
        let mine = Preset {
            name: "Mine".into(),
            format: FileFormat::Png,
            depth: Depth::Sixteen,
            ..Preset::default()
        };
        let r = parse(&args(&["x.dng", "--preset", "mine"]), &[mine]).unwrap();
        assert_eq!(
            (r.preset.format, r.preset.depth),
            (FileFormat::Png, Depth::Sixteen)
        );
        assert_eq!(parse_size("50%"), Some(Resize::Percent(50)));
        assert_eq!(parse_size("2048"), Some(Resize::LongEdge(2048)));
        assert_eq!(parse_size("0"), None);
        // Refusals say why.
        for bad in [
            &["x", "--format", "bmp"][..],
            &["x", "--format", "jpeg", "--depth", "16"],
            &["x", "--preset", "nope"],
            &["x", "--frobnicate"],
            &["--format", "png"],
            &["x", "--name", "{oops"],
        ] {
            assert!(parse(&args(bad), &[]).is_err(), "{bad:?}");
        }
    }
}
