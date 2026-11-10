//! The built-in manifest's converted-file pins: every Search, Speech and Text model
//! converted after download is pinned (always checked), the conversion is deterministic, and
//! each pin is what converting the real download gives (when the downloads are here).
//!
//! `SPECULAR_AI_ORIGINALS` names a folder searched (two levels deep) for the downloads, each
//! found by its size and checked by its SHA-256 before converting, for example:
//!
//! ```text
//! SPECULAR_SIGLIP2_DIR=target/originals/siglip2 tools/corpus/fetch-siglip2.sh b16 b32
//! SPECULAR_PECORE_DIR=target/originals/pecore tools/corpus/fetch-pecore.sh model
//! tools/corpus/fetch-whisper.sh base small
//! SPECULAR_OCR_DIR=target/originals/ocr tools/corpus/fetch-ocr.sh
//! SPECULAR_AI_ORIGINALS=target/originals cargo test -p specular-ai --test pins -- --nocapture
//! ```
//!
//! Without the folder (or a download in it) that pin is skipped with a note.

mod common;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::Dir;
use sha2::{Digest, Sha256};
use specular_ai::Feature;
use specular_ai::install::convert_download;
use specular_ai::manifest::{BUILTIN, Manifest};

fn sha_file(p: &Path) -> String {
    let mut f = std::fs::File::open(p).unwrap();
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    h.finalize().iter().map(|x| format!("{x:02x}")).collect()
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Files up to two levels under `dir`.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dirs = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 2 {
                dirs.push((p, depth + 1));
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn converted_models_are_pinned() {
    let m = Manifest::parse(BUILTIN).unwrap();
    let converted: Vec<_> = m
        .models
        .iter()
        .filter(|m| {
            matches!(
                m.feature,
                Feature::Search
                    | Feature::Speech
                    | Feature::Text
                    | Feature::Masks
                    | Feature::Denoise
                    | Feature::Remove
                    | Feature::Depth
            ) && m.runtime.convert.is_some()
        })
        .collect();
    assert!(converted.len() >= 5 + 15);
    assert!(converted.iter().any(|m| m.id == "whisper-base"));
    assert!(converted.iter().any(|m| m.id == "whisper-small"));
    assert!(converted.iter().any(|m| m.id == "ppocrv6-small-det"));
    for c in converted {
        assert!(
            c.runtime.converted_sha256.is_some(),
            "{} is not pinned",
            c.id
        );
    }
}

/// The same download converts to the same bytes every time, and the tensors' order in the
/// download's header doesn't change them (the pins hold on every computer).
#[test]
fn conversion_is_deterministic() {
    let tmp = Dir::new("pins-determinism");
    // Two tensors of f32 values with fractions f16 must round, and metadata.
    let a: Vec<f32> = (0..300)
        .map(|i| (i as f32 * 0.123_457).sin() * 7.1)
        .collect();
    let b: Vec<f32> = (0..64).map(|i| 1.0 / (i as f32 + 0.3)).collect();
    let bytes = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let (ab, bb) = (bytes(&a), bytes(&b));
    let file = |first_a: bool| {
        let ea = format!(
            r#""a.weight":{{"dtype":"F32","shape":[20,15],"data_offsets":[0,{}]}}"#,
            ab.len()
        );
        let eb = format!(
            r#""b.bias":{{"dtype":"F32","shape":[64],"data_offsets":[{},{}]}}"#,
            ab.len(),
            ab.len() + bb.len()
        );
        let meta = r#""__metadata__":{"format":"pt"}"#;
        let mut h = if first_a {
            format!("{{{meta},{ea},{eb}}}")
        } else {
            format!("{{{eb},{ea},{meta}}}")
        };
        while h.len() % 8 != 0 {
            h.push(' ');
        }
        let mut out = (h.len() as u64).to_le_bytes().to_vec();
        out.extend_from_slice(h.as_bytes());
        out.extend_from_slice(&ab);
        out.extend_from_slice(&bb);
        out
    };
    let m = Manifest::parse(BUILTIN).unwrap();
    let model = m.models.iter().find(|m| m.id == "whisper-base").unwrap();
    let mut hashes = Vec::new();
    for (i, first_a) in [true, true, false].into_iter().enumerate() {
        let src = tmp.path().join(format!("src{i}.safetensors"));
        std::fs::write(&src, file(first_a)).unwrap();
        let dst = tmp.path().join(format!("dst{i}.safetensors"));
        convert_download(model, &src, &dst, &|| false).unwrap();
        hashes.push(sha_file(&dst));
    }
    assert_eq!(hashes[0], hashes[1]);
    assert_eq!(hashes[0], hashes[2]);
}

#[test]
fn pins_match_converting_the_downloads() {
    let Some(root) = std::env::var_os("SPECULAR_AI_ORIGINALS").map(PathBuf::from) else {
        eprintln!("skipped: SPECULAR_AI_ORIGINALS not set");
        return;
    };
    let all = files(&root);
    let m = Manifest::parse(BUILTIN).unwrap();
    let tmp = Dir::new("pins");
    let mut wrong = Vec::new();
    for model in m.models.iter().filter(|m| m.runtime.convert.is_some()) {
        let want = hex(&model.sha256);
        let Some(src) = all.iter().find(|p| {
            std::fs::metadata(p).is_ok_and(|md| md.len() == model.size) && sha_file(p) == want
        }) else {
            eprintln!("{}: download not found under {}", model.id, root.display());
            continue;
        };
        let dst = tmp.path().join(format!("{}.safetensors", model.id));
        let t0 = Instant::now();
        let n = convert_download(model, src, &dst, &|| false).unwrap();
        let got = sha_file(&dst);
        eprintln!(
            "{}: converted {} → {n} bytes in {:?}, sha256 {got}",
            model.id,
            model.size,
            t0.elapsed()
        );
        // Again: the same bytes.
        let again = tmp.path().join(format!("{}-2.safetensors", model.id));
        convert_download(model, src, &again, &|| false).unwrap();
        assert_eq!(
            sha_file(&again),
            got,
            "{}: converting twice differs",
            model.id
        );
        let _ = std::fs::remove_file(&dst);
        let _ = std::fs::remove_file(&again);
        let pin = model.runtime.converted_sha256.map(|p| hex(&p));
        if pin.as_deref() != Some(got.as_str()) {
            wrong.push(format!("{}: pinned {pin:?}, converts to {got}", model.id));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
