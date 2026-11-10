//! The RAW variant corpus (`tools/corpus/fetch-raw-variants.sh`, gitignored): every file
//! decodes (or is refused with a reason, then shows its preview), develops, and survives
//! truncation. Skipped unless `SPECULAR_RAW_VARIANTS` names the fetched directory. The
//! sensor data itself is checked against LibRaw and the reference codecs by
//! `tools/quality/raw_variants_oracle.py`.

use std::path::{Path, PathBuf};

use specular_raw::{DecodeOptions, OutputSize};

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Files whose sensor data stays refused, with a word their reason must contain.
const REFUSED: [(&str, &str); 2] = [(".GPR", "GPR"), ("F700", "SR")];

#[test]
fn variant_corpus_decodes() {
    let Some(dir) = std::env::var_os("SPECULAR_RAW_VARIANTS").map(PathBuf::from) else {
        eprintln!("SPECULAR_RAW_VARIANTS not set: skipped");
        return;
    };
    for path in files(&dir) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let data = std::fs::read(&path).unwrap();
        let Ok(raw) = specular_raw::parse(&data) else {
            // A JPEG named .dng (SM-G973U) is not a RAW file.
            assert!(name.ends_with(".dng"), "{name}: not parsed");
            continue;
        };
        let si = raw.info().sensor.clone().expect("sensor");
        if let Some((_, why)) = REFUSED.iter().find(|(k, _)| name.contains(k)) {
            let r = si.unsupported_reason.unwrap_or("");
            assert!(!si.decodable && r.contains(why), "{name}: {r:?}");
            if let Some(p) = raw.info().largest_preview() {
                let img = specular_raw::decode_preview(&data, p, 0).expect("preview");
                assert!(img.width > 0 && img.height > 0, "{name}");
            }
            continue;
        }
        assert!(si.decodable, "{name}: {:?}", si.unsupported_reason);
        let img = raw.decode(
            &data,
            &DecodeOptions {
                size: OutputSize::Half,
                ..Default::default()
            },
        );
        let img = img.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!img.truncated && img.width > 0, "{name}");
        // Truncated copies: no panic.
        for frac in [2, 5] {
            let cut = &data[..data.len() * (frac - 1) / frac];
            if let Ok(r) = specular_raw::parse(cut) {
                let _ = r.decode(cut, &DecodeOptions::default());
            }
        }
    }
}
