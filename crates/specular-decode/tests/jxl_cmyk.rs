//! JPEG XL CMYK (a black extra channel) through the registry: the inks converted to sRGB
//! with the embedded CMYK profile (specular-color), against libjxl's own rendering. Fidelity
//! round 7; the corpus is libjxl's conformance case `cmyk_layers` (input.jxl and its
//! ref.png), fetched by tools/corpus/fetch-fidelity7.sh into `<corpus>/jxl/conformance/`.
//!
//! ```text
//! SPECULAR_FIDELITY7_CORPUS=<dir> cargo test -p specular-decode --test jxl_cmyk -- --ignored
//! ```
//!
//! One `ORACLE` line per case. libjxl converts through skcms or Little CMS and specular-color
//! through its own CMM, so the two agree within a few codes, not bit for bit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Registry};

fn corpus() -> Option<PathBuf> {
    let marked = |p: PathBuf| p.join(".fidelity7").is_file().then_some(p);
    if let Some(d) = std::env::var_os("SPECULAR_FIDELITY7_CORPUS") {
        return Some(PathBuf::from(d));
    }
    if let Some(d) = std::env::var_os("SPECULAR_FIDELITY_CORPUS").and_then(|d| marked(d.into())) {
        return Some(d);
    }
    let t = std::env::var_os("CARGO_TARGET_DIR")?;
    marked(Path::new(&t).join("corpus/fidelity7"))
}

fn rgba(reg: &Registry, path: &Path) -> (u32, u32, Vec<u8>, String) {
    let s = reg.open(Arc::new(Source::open(path).unwrap())).unwrap();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(r.layout, PixelLayout::Rgba, "{}", path.display());
    let p = &r.planes[0];
    let mut v = Vec::with_capacity(r.width as usize * r.height as usize * 4);
    for row in p.data.chunks(p.stride).take(r.height as usize) {
        v.extend_from_slice(&row[..r.width as usize * 4]);
    }
    (r.width, r.height, v, s.info().coding.clone())
}

#[test]
#[ignore = "needs the fidelity7 corpus (tools/corpus/fetch-fidelity7.sh)"]
fn cmyk_matches_libjxl() {
    let Some(root) = corpus() else {
        eprintln!("jxl_cmyk: no corpus; skipped");
        return;
    };
    let dir = root.join("jxl/conformance");
    let reg = Registry::new();
    let mut failed = 0;
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    names.sort();
    for jxl in names
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "jxl"))
    {
        let name = jxl.file_name().unwrap().to_string_lossy().into_owned();
        let (w, h, got, coding) = rgba(&reg, jxl);
        let (rw, rh, want, _) = rgba(&reg, &jxl.with_extension("png"));
        assert!(coding.contains("CMYK"), "{name}: {coding}");
        assert_eq!((w, h), (rw, rh), "{name}");
        let mut d: Vec<u32> = got
            .chunks_exact(4)
            .zip(want.chunks_exact(4))
            .flat_map(|(a, b)| (0..4).map(move |c| u32::from(a[c].abs_diff(b[c]))))
            .collect();
        let mean = d.iter().sum::<u32>() as f64 / d.len() as f64;
        d.sort_unstable();
        let p99 = d[d.len() * 99 / 100];
        let max = *d.last().unwrap();
        // Two CMMs: within a couple of codes on average, a few at the 99th percentile.
        let ok = mean <= 1.5 && p99 <= 6;
        failed += usize::from(!ok);
        println!(
            "ORACLE\tJXL/conformance\t{name}\t{}",
            if ok { "pass" } else { "fail" }
        );
        println!("#\t{name}\tmean {mean:.2}, 99% {p99}, max {max} (of 255) against libjxl");
    }
    assert_eq!(failed, 0, "CMYK JPEG XL differs from libjxl");
}
