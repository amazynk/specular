//! Search with a SigLIP model end to end through the pack, against `file://` sources (no
//! network): the manifest's default choice, the hash-checked download converted to f16 with
//! the original deleted (and the converted file's pin checked), then the installed files
//! loaded as the search runtime and text encoder, with the same embeddings as the original
//! file.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

mod common;

use std::sync::Arc;

use common::{Dir, sha_hex};
use specular_ai::choose::{self, MachineFacts};
use specular_ai::infer::InferRuntime;
use specular_ai::manifest::Norm;
use specular_ai::runtime::{Outputs, Runtime, Thumb};
use specular_ai::testing::tiny_siglip;
use specular_ai::transport::Transports;
use specular_ai::{Feature, Manifest, Pack, Tier};
use specular_core::paths::{Engine, PathSelector};
use specular_infer::safetensors::{SafeTensorsFile, StDType};

fn manifest(src: &std::path::Path, weights: &[u8], tok: &[u8], pin: Option<&str>) -> String {
    let file = |n: &str| format!("file://{}", src.join(n).display());
    let pin = pin.map_or(String::new(), |p| format!("converted_sha256 = {p:?}\n"));
    format!(
        r#"schema = 3

[[model]]
feature = "search"
tier = ["small", "large"]
id = "tok"
version = "1"
role = "tokenizer"
urls = [{tu:?}]
sha256 = "{ts}"
size = {tn}
licence = "Apache-2.0"
licence_url = "https://models.invalid/l"
placeholder = true

[model.runtime]
format = "spm"
gpu = "none"

[[model]]
feature = "search"
tier = "large"
id = "sig"
version = "1"
role = "image-text-encoder"
urls = [{wu:?}]
sha256 = "{ws}"
size = {wn}
licence = "Apache-2.0"
licence_url = "https://models.invalid/l"
placeholder = true

[model.runtime]
format = "safetensors"
arch = "siglip"
min_memory_mb = 64
input = 32
dims = 128
norm = "inception"
mmacs = 9000
convert = "f16"
{pin}
[[model]]
feature = "search"
tier = "small"
id = "sig-small"
version = "1"
role = "image-text-encoder"
urls = [{wu:?}]
sha256 = "{ws}"
size = {wn}
licence = "Apache-2.0"
licence_url = "https://models.invalid/l"
placeholder = true

[model.runtime]
format = "safetensors"
arch = "siglip"
min_memory_mb = 64
input = 32
dims = 128
norm = "inception"
mmacs = 10
convert = "f16"

[[choice]]
feature = "search"
tier = "large"
name = "Tiny SigLIP"
default = true
languages = "multilingual"
data = "noise"

[[choice]]
feature = "search"
tier = "small"
name = "Tiny SigLIP fast"
hint = "faster"
"#,
        tu = file("tokenizer.model"),
        ts = sha_hex(tok),
        tn = tok.len(),
        wu = file("sig.safetensors"),
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
fn default_choice_installs_converted_and_searches() {
    let d = Dir::new("siglip");
    let src = d.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let (weights, tok) = tiny_siglip(&src, "sig", 4);
    let m = Manifest::parse(&manifest(&src, &weights, &tok, None)).unwrap();
    let offer = m.offer(Feature::Search, true).unwrap();
    // The default wins over the speed rule (which would pick the small one on a slow GPU)
    // and the power rule.
    let facts = MachineFacts {
        gpu: specular_core::paths::GpuClass::Integrated,
        gpu_usable: true,
        gpu_ms: Some(1000.0),
        ram: Some(16 << 30),
        low_power: true,
        ..MachineFacts::default()
    };
    assert_eq!(choose::recommend(&offer, &facts), Some(Tier::Large));
    let data = d.path().join("data");
    let pack =
        Pack::new(m, &data, None, Transports::default(), Arc::new(|| {})).with_placeholders();
    assert!(pack.install(Feature::Search, Tier::Large));
    pack.join();
    for (what, r) in pack.take_finished() {
        assert!(r.is_ok(), "{what}: {r:?}");
    }
    assert_eq!(pack.installed(), [Feature::Search]);
    // Downloads gone: the original was converted and deleted.
    let left: Vec<_> = std::fs::read_dir(data.join("ai/downloads"))
        .unwrap()
        .collect();
    assert!(left.is_empty(), "{left:?}");
    let (im, _) = pack.index_models(Feature::Search);
    let im = im.unwrap();
    let (w, t, norm) = pack.clip_parts(&im).unwrap();
    assert_eq!(norm, Norm::Inception);
    let installed = data.join("ai").join(&w.file);
    let f = SafeTensorsFile::open(&installed).unwrap();
    let dtype = |n: &str| f.header.get(n).unwrap().dtype;
    assert_eq!(
        dtype("text_model.embeddings.token_embedding.weight"),
        StDType::F16
    );
    assert_eq!(
        dtype("vision_model.encoder.layers.0.mlp.fc1.weight"),
        StDType::F16
    );
    assert_eq!(
        dtype("vision_model.embeddings.position_embedding.weight"),
        StDType::F32
    );
    assert_eq!(dtype("vision_model.head.probe"), StDType::F32);
    assert_eq!(
        dtype("vision_model.encoder.layers.0.mlp.fc1.bias"),
        StDType::F32
    );
    let converted = std::fs::read(&installed).unwrap();
    assert!(converted.len() < weights.len() * 3 / 4);

    // Loaded as the app loads it, it gives what the original file gives.
    static SEL: PathSelector = PathSelector::new();
    let (rt, text) = InferRuntime::load_clip(
        &installed,
        &data.join("ai").join(&t.file),
        "sig-1",
        norm,
        None,
        &SEL,
        2,
    )
    .unwrap();
    let (rt0, text0) = InferRuntime::load_clip(
        &src.join("sig.safetensors"),
        &src.join("tokenizer.model"),
        "sig-1",
        norm,
        None,
        &SEL,
        2,
    )
    .unwrap();
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
    assert_eq!(embedding(&a).len(), 128);
    let q = text.encode_on(Engine::Cpu, "A cat").unwrap();
    assert_eq!(q, text0.encode_on(Engine::Cpu, "A cat").unwrap());
    // SigLIP lower-cases: case does not matter.
    assert_eq!(q, text.encode_on(Engine::Cpu, "a CAT").unwrap());
    assert_eq!(text.tokens("cat")[..2], text0.tokens("cat")[..2]);

    // A pinned converted hash is checked: a wrong pin fails the install, and nothing is
    // installed.
    let d2 = Dir::new("siglip-pin");
    let bad = "0".repeat(64);
    let m = Manifest::parse(&manifest(&src, &weights, &tok, Some(&bad))).unwrap();
    let pack =
        Pack::new(m, d2.path(), None, Transports::default(), Arc::new(|| {})).with_placeholders();
    assert!(pack.install(Feature::Search, Tier::Large));
    pack.join();
    assert!(pack.installed().is_empty());
    // The right pin installs.
    let good = sha_hex(&converted);
    let d3 = Dir::new("siglip-pin-ok");
    let m = Manifest::parse(&manifest(&src, &weights, &tok, Some(&good))).unwrap();
    let pack =
        Pack::new(m, d3.path(), None, Transports::default(), Arc::new(|| {})).with_placeholders();
    assert!(pack.install(Feature::Search, Tier::Large));
    pack.join();
    assert_eq!(pack.installed(), [Feature::Search]);
}
