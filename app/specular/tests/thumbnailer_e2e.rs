//! End-to-end run of the desktop thumbnailer, `specular --thumbnail IN OUT SIZE`, the way a
//! file manager calls it: the built binary, a home and XDG folders of its own (nothing of the
//! user's is read or written), no display and no session bus. Runs on the first file of each
//! format folder of the large corpus (`SPECULAR_LARGE_CORPUS`) and prints the time per file;
//! skipped without it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Extensions tried (case-insensitive).
const EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "tif", "tiff", "exr", "hdr", "gif", "svg", "cr2", "cr3", "nef",
    "arw", "raf", "rw2", "orf", "dng", "pef",
];

fn first_file(dir: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    let wanted = |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
    };
    entries
        .iter()
        .find(|p| p.is_file() && wanted(p))
        .cloned()
        .or_else(|| {
            entries
                .iter()
                .filter(|p| p.is_dir())
                .find_map(|d| first_file(d))
        })
}

#[test]
fn thumbnails_of_corpus_files() {
    let Some(corpus) = std::env::var_os("SPECULAR_LARGE_CORPUS").map(PathBuf::from) else {
        eprintln!("SPECULAR_LARGE_CORPUS not set: skipped");
        return;
    };
    assert!(
        !corpus.to_string_lossy().contains("corpus/real"),
        "the private corpus is not used for tests"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-app/itest-thumbnailer")
        .join(std::process::id().to_string());
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    for d in ["config", "cache", "data", "state", "out"] {
        std::fs::create_dir_all(home.join(d)).unwrap();
    }
    let mut formats: Vec<PathBuf> = std::fs::read_dir(&corpus)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    formats.sort();
    let files: Vec<PathBuf> = formats.iter().filter_map(|d| first_file(d)).collect();
    assert!(!files.is_empty(), "no images in {}", corpus.display());
    let exe = env!("CARGO_BIN_EXE_specular");
    let mut failed = Vec::new();
    println!("{:<52} {:>6} {:>9} {:>9}", "file", "size", "ms", "result");
    for (i, file) in files.iter().enumerate() {
        for size in [128u32, 256] {
            let out = home.join("out").join(format!("{i}-{size}.png"));
            let t = Instant::now();
            let status = Command::new(exe)
                .arg("--thumbnail")
                .arg(file)
                .arg(&out)
                .arg(size.to_string())
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", &home)
                .env("XDG_CONFIG_HOME", home.join("config"))
                .env("XDG_CACHE_HOME", home.join("cache"))
                .env("XDG_DATA_HOME", home.join("data"))
                .env("XDG_STATE_HOME", home.join("state"))
                .output()
                .unwrap();
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let name = file
                .strip_prefix(&corpus)
                .unwrap_or(file)
                .to_string_lossy()
                .into_owned();
            let short: String = name
                .chars()
                .rev()
                .take(52)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let result = if status.status.success() {
                let png = std::fs::read(&out).unwrap();
                let r = png::Decoder::new(std::io::Cursor::new(&png))
                    .read_info()
                    .unwrap();
                let info = r.info();
                assert!(info.width.max(info.height) <= size, "{name}: too large");
                assert!(
                    info.uncompressed_latin1_text
                        .iter()
                        .any(|t| t.keyword == "Thumb::URI"),
                    "{name}: no Thumb::URI"
                );
                format!("{}x{}", info.width, info.height)
            } else if String::from_utf8_lossy(&status.stderr).ends_with(": too large\n") {
                // Over the generator's decode-memory budget (the gallery's limit too):
                // a clean refusal, the file manager shows its generic icon.
                assert!(!out.exists(), "{name}: output left behind");
                "too large".into()
            } else {
                failed.push(format!(
                    "{name} @{size}: {}",
                    String::from_utf8_lossy(&status.stderr).trim()
                ));
                "FAILED".into()
            };
            println!("{short:<52} {size:>6} {ms:>9.1} {result:>9}");
        }
    }
    // Nothing but the outputs was written: no settings, cache, logs or state.
    for d in ["config", "cache", "data", "state"] {
        let n = std::fs::read_dir(home.join(d)).unwrap().count();
        assert_eq!(n, 0, "the thumbnailer wrote to {d}");
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(failed.is_empty(), "failed: {failed:#?}");
}
