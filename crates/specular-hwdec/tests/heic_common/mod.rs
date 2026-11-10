//! Shared by the `heic_*` tests: corpus files and decoding with a given core.

#![allow(dead_code)]

use specular_heic::{DEFAULT_CORE, DecodeOptions, HeicFile, HevcCore, Image};
use std::path::{Path, PathBuf};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Points the crash canary at the test's scratch folder (never the user's cache); call
/// before anything else touches the hardware path.
pub fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    specular_hwdec::heic::set_cache_dir(d.clone());
    d
}

/// Whether the hardware HEVC path can run here; prints why not.
pub fn hardware() -> bool {
    let ok = specular_hwdec::heic::usable();
    if !ok {
        eprintln!("skipped: no hardware HEVC decoder (or the path is off)");
    }
    ok
}

/// The first existing file of `names` (relative to the repository).
pub fn find(names: &[&str]) -> Option<PathBuf> {
    names.iter().map(|n| root().join(n)).find(|p| p.is_file())
}

/// A 12-megapixel 4:2:0 grid, if one is on this machine.
pub fn grid() -> Option<PathBuf> {
    find(&[
        "target/agent-jpeg/heic-eval/gen/grid12mp_420.heic",
        "tools/corpus/conformance/zune-image/crates/zune-heic/fuzz-samples/IMG_4862.HEIC",
    ])
}

/// Every HEIC file of the corpus present here: the reference manifest of
/// `tools/corpus/heic-refs.sh`, the generated evaluation set and the fixtures.
pub fn corpus() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let r = root();
    if let Ok(t) = std::fs::read_to_string(r.join("tools/corpus/out/heic-ref/manifest.tsv")) {
        for l in t.lines() {
            if let Some(p) = l.split('\t').nth(1) {
                v.push(r.join(p));
            }
        }
    }
    for dir in [
        "target/agent-jpeg/heic-eval/gen",
        "crates/specular-decode-host/tests/data",
    ] {
        if let Ok(rd) = std::fs::read_dir(r.join(dir)) {
            let mut fs: Vec<_> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("heic"))
                })
                .collect();
            fs.sort();
            v.extend(fs);
        }
    }
    v.push(
        r.join("tools/corpus/conformance/zune-image/crates/zune-heic/fuzz-samples/IMG_4862.HEIC"),
    );
    v.retain(|p| p.is_file());
    v.dedup();
    v
}

pub fn decode(path: &Path, core: Option<&dyn HevcCore>) -> Option<Image> {
    let data = std::fs::read(path).ok()?;
    let f = HeicFile::open(data).ok()?;
    let opts = DecodeOptions {
        parallel: true,
        cancel: None,
        core,
    };
    f.decode(&opts).ok()
}

/// Software reference: the default core.
pub fn software(path: &Path) -> Option<Image> {
    decode(path, Some(&DEFAULT_CORE))
}

pub fn same(a: &Image, b: &Image) -> bool {
    a.frame == b.frame && a.warnings == b.warnings && a.premultiplied_alpha == b.premultiplied_alpha
}
