//! Search with a PE Core model end to end through the pack, against `file://` sources (no
//! network): the hash-checked `.pt` download converted to an f16 safetensors file (the
//! original deleted, the converted file's pin checked, its size shown as on disk), then the
//! installed files loaded as the search runtime and text encoder, with the same embeddings
//! as the original checkpoint.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

mod common;

use std::sync::Arc;

use common::{Dir, sha_hex};
use specular_ai::infer::InferRuntime;
use specular_ai::manifest::Norm;
use specular_ai::runtime::{Outputs, Runtime, Thumb};
use specular_ai::testing::tiny_pecore;
use specular_ai::transport::Transports;
use specular_ai::{Feature, Manifest, Pack, Tier};
use specular_core::paths::{Engine, PathSelector};
use specular_infer::clip::Layout;
use specular_infer::safetensors::{SafeTensorsFile, StDType};

fn manifest(src: &std::path::Path, weights: &[u8], tok: &[u8], pin: Option<&str>) -> String {
    let file = |n: &str| format!("file://{}", src.join(n).display());
    let pin = pin.map_or(String::new(), |p| format!("converted_sha256 = {p:?}\n"));
    format!(
        r#"schema = 3

[[model]]
feature = "search"
tier = "standard"
id = "merges"
version = "1"
role = "tokenizer"
urls = [{tu:?}]
sha256 = "{ts}"
size = {tn}
licence = "Apache-2.0"
licence_url = "https://models.invalid/l"
placeholder = true

[model.runtime]
format = "txt"
gpu = "none"

[[model]]
feature = "search"
tier = "standard"
id = "pe"
version = "1"
role = "image-text-encoder"
urls = [{wu:?}]
sha256 = "{ws}"
size = {wn}
licence = "Apache-2.0"
licence_url = "https://models.invalid/l"
placeholder = true

[model.runtime]
format = "pt"
arch = "pecore"
min_memory_mb = 64
input = 32
dims = 32
norm = "inception"
mmacs = 100
convert = "f16"
{pin}
[[choice]]
feature = "search"
tier = "standard"
name = "Tiny PE"
default = true
languages = "english"
"#,
        tu = file("merges.txt"),
        ts = sha_hex(tok),
        tn = tok.len(),
        wu = file("pe.pt"),
        ws = sha_hex(weights),
        wn = weights.len(),
    )
}

fn embedding(o: &Outputs) -> &[u16] {
    match o {
        Outputs::Embedding(v) => v,
        o => panic!("not an embedding: {o:?}"),
    }
}

#[test]
fn pt_download_installs_converted_and_searches() {
    let d = Dir::new("pecore");
    let src = d.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let (weights, tok) = tiny_pecore(&src, "pe", 4);
    let m = Manifest::parse(&manifest(&src, &weights, &tok, None)).unwrap();
    let data = d.path().join("data");
    let pack =
        Pack::new(m, &data, None, Transports::default(), Arc::new(|| {})).with_placeholders();
    assert_eq!(
        pack.download_size(Feature::Search, Tier::Standard),
        Some((weights.len() + tok.len()) as u64)
    );
    assert_eq!(pack.disk_size(Feature::Search, Tier::Standard), None);
    assert!(pack.install(Feature::Search, Tier::Standard));
    pack.join();
    for (what, r) in pack.take_finished() {
        assert!(r.is_ok(), "{what}: {r:?}");
    }
    assert_eq!(pack.installed(), [Feature::Search]);
    let left: Vec<_> = std::fs::read_dir(data.join("ai/downloads"))
        .unwrap()
        .collect();
    assert!(left.is_empty(), "{left:?}");
    let (im, _) = pack.index_models(Feature::Search);
    let im = im.unwrap();
    let (w, t, norm) = pack.clip_parts(&im).unwrap();
    assert_eq!(norm, Norm::Inception);
    // Kept as safetensors, whatever was downloaded.
    assert!(w.file.ends_with("/model.safetensors"), "{}", w.file);
    let installed = data.join("ai").join(&w.file);
    let f = SafeTensorsFile::open(&installed).unwrap();
    let dtype = |n: &str| f.header.get(n).unwrap().dtype;
    assert_eq!(dtype("token_embedding.weight"), StDType::F16);
    assert_eq!(
        dtype("visual.transformer.resblocks.0.mlp.c_fc.weight"),
        StDType::F16
    );
    assert_eq!(dtype("visual.positional_embedding"), StDType::F32);
    assert_eq!(dtype("visual.attn_pool.probe"), StDType::F32);
    let converted = std::fs::read(&installed).unwrap();
    assert!(converted.len() < weights.len() * 3 / 4);
    // "On disk" is the converted file and the merges, not the download.
    assert_eq!(pack.download_size(Feature::Search, Tier::Standard), Some(0));
    assert_eq!(
        pack.disk_size(Feature::Search, Tier::Standard),
        Some((converted.len() + tok.len()) as u64)
    );

    // Loaded as the app loads it, it gives what the original checkpoint gives.
    static SEL: PathSelector = PathSelector::new();
    let (rt, text) = InferRuntime::load_clip(
        &installed,
        &data.join("ai").join(&t.file),
        "pe-1",
        norm,
        None,
        &SEL,
        2,
    )
    .unwrap();
    let (rt0, text0) = InferRuntime::load_clip(
        &src.join("pe.pt"),
        &src.join("merges.txt"),
        "pe-1",
        norm,
        None,
        &SEL,
        2,
    )
    .unwrap();
    assert_eq!(text.layout(), Layout::PeCore);
    assert!(!text.siglip());
    assert_eq!(rt.input_side(), 32);
    let px: Vec<u8> = (0..40 * 30 * 4).map(|i| (i * 37 % 251) as u8).collect();
    let thumb = Thumb {
        width: 40,
        height: 30,
        rgba: &px,
    };
    let a = rt.run(Engine::Cpu, &thumb).unwrap();
    let b = rt0.run(Engine::Cpu, &thumb).unwrap();
    assert_eq!(embedding(&a), embedding(&b));
    assert_eq!(embedding(&a).len(), 32);
    let q = text.encode_on(Engine::Cpu, "A cat").unwrap();
    assert_eq!(q, text0.encode_on(Engine::Cpu, "A cat").unwrap());
    // CLIP's tokenizer lower-cases too.
    assert_eq!(q, text.encode_on(Engine::Cpu, "a CAT").unwrap());

    // A wrong pin fails the install; the right one installs.
    let d2 = Dir::new("pecore-pin");
    let m = Manifest::parse(&manifest(&src, &weights, &tok, Some(&"0".repeat(64)))).unwrap();
    let pack =
        Pack::new(m, d2.path(), None, Transports::default(), Arc::new(|| {})).with_placeholders();
    assert!(pack.install(Feature::Search, Tier::Standard));
    pack.join();
    assert!(pack.installed().is_empty());
    let d3 = Dir::new("pecore-pin-ok");
    let good = sha_hex(&converted);
    let m = Manifest::parse(&manifest(&src, &weights, &tok, Some(&good))).unwrap();
    let pack =
        Pack::new(m, d3.path(), None, Transports::default(), Arc::new(|| {})).with_placeholders();
    assert!(pack.install(Feature::Search, Tier::Standard));
    pack.join();
    assert_eq!(pack.installed(), [Feature::Search]);
}

/// Peak memory of PE Core S16 in use, for its manifest's `min_memory_mb`: the converted f16
/// file (`SPECULAR_PECORE_F16`) loaded as the search runtime and text encoder on the CPU, then
/// 8 photos embedded and 8 queries encoded. Measured as the rise of this process's resident
/// high-water mark (Linux: VmHWM, reset through /proc/self/clear_refs first) over what it held
/// before loading. Run alone (`--test-threads 1`, this test only) so nothing else adds to it.
#[test]
fn real_pecore_peak_memory() {
    let (Some(w), Some(merges)) = (
        std::env::var_os("SPECULAR_PECORE_F16").map(std::path::PathBuf::from),
        std::env::var_os("SPECULAR_PECORE_MERGES").map(std::path::PathBuf::from),
    ) else {
        eprintln!("SPECULAR_PECORE_F16 / SPECULAR_PECORE_MERGES not set: skipped");
        return;
    };
    let status = |key: &str| -> u64 {
        let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        s.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    let reset = || {
        let _ = std::fs::write("/proc/self/clear_refs", "5");
    };
    let sel = PathSelector::new();
    let side = 384u32;
    let px: Vec<u8> = (0..side * side)
        .flat_map(|i| {
            let (x, y) = (i % side, i / side);
            [
                (x * 255 / side) as u8,
                (y * 255 / side) as u8,
                ((x ^ y) & 255) as u8,
                255,
            ]
        })
        .collect();
    reset();
    let before = status("VmRSS:");
    let base_hwm = status("VmHWM:");
    let (rt, text) = InferRuntime::load_clip(
        &w,
        &merges,
        "pe-core-s16-384-1",
        Norm::Inception,
        None,
        &sel,
        8,
    )
    .unwrap();
    let loaded = status("VmRSS:");
    for _ in 0..8 {
        let t = Thumb {
            width: side,
            height: side,
            rgba: &px,
        };
        rt.run(Engine::Cpu, &t).unwrap();
    }
    for q in [
        "a dog on a beach",
        "red car",
        "two people",
        "a mountain lake at dusk",
    ]
    .iter()
    .cycle()
    .take(8)
    {
        text.encode(q).unwrap();
    }
    let peak = status("VmHWM:");
    eprintln!(
        "PE Core S16 on the CPU: resident before {} MB (high-water {} MB), loaded {} MB, \
         peak {} MB: the model needs {} MB at its peak",
        before / 1024,
        base_hwm / 1024,
        loaded / 1024,
        peak / 1024,
        peak.saturating_sub(before) / 1024
    );
}
