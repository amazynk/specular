//! The Local AI pack's infrastructure (DESIGN.md "Local AI pack", §10).
//!
//! The pack is an optional download, off until installed, and nothing it does leaves the
//! machine. Per feature (search by description, text in images, culling assistance,
//! people) the user picks a model in the model manager, shown with its download size,
//! licence, training data and host, and nothing downloads until they opt in (DESIGN.md,
//! "Local AI pack"). This crate is everything around the models; the networks are
//! `specular-infer`'s. The built-in manifest offers SigLIP 2 for search and lists the other
//! features' models as coming.
//!
//! - [`manifest`]: the model manifest format and the in-tree registry.
//! - [`install`]: resumable, hash-verified downloads and atomic install into the data
//!   directory; removal drops the feature's tables and files.
//! - [`transport`]: where downloads come from (`file://` here; the app adds HTTPS).
//! - [`runtime`]: the "model on a thumbnail" trait, a stub, and dispatch through the
//!   PathSelector's `Task::Inference`.
//! - [`infer`]: that trait over our own engine (`specular-infer`): a model loaded from a
//!   safetensors pack, run on the CPU reference or the GPU once it matched the CPU.
//! - [`job`]: background indexing from cached thumbnails, yielding to the user and the
//!   machine; cancellable and resumable, with progress for the UI.
//! - [`search`]: exact vector search over f16 embeddings, GPU and CPU, identical results.
//! - [`pack`]: the pack as Settings sees it.
//! - [`speech`]: the Speech feature's Whisper, for generated subtitles.
//! - [`masks`]: the Masks feature's networks, for Develop's AI masks.
//! - [`denoise`]: the Denoise feature's network, for Develop's AI denoise.

pub mod choose;
pub mod denoise;
pub mod faces;
pub mod infer;
pub mod install;
pub mod job;
pub mod manifest;
pub mod masks;
pub mod ocr;
pub mod pack;
pub mod remove;
pub mod runtime;
pub mod search;
pub mod sideload;
pub mod speech;
pub mod transport;

pub use choose::{MachineFacts, SizeChoice, Unfit};
pub use manifest::{Feature, Manifest, Model, Offer, Tier, TierOffer};
pub use pack::{AddonStatus, FeatureState, FeatureStatus, IndexModel, Pack, Sideload};

/// Helpers for tests and benchmarks.
#[doc(hidden)]
pub mod testing {
    use crate::search::{Vectors, normalize_f16};

    /// `n` unit vectors of `dims` from a seeded generator.
    pub fn random_vectors(n: usize, dims: usize, seed: u64) -> Vectors {
        let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64 / (1u64 << 53) as f64) as f32 * 2.0 - 1.0
        };
        let mut v = Vectors::new(dims);
        let mut x = vec![0f32; dims];
        for i in 0..n {
            for d in x.iter_mut() {
                *d = next();
            }
            v.push(i as i64, &normalize_f16(&x));
        }
        v
    }

    /// A tiny tokenizer's merges (`hello`, `cat` and their parts).
    pub const TINY_MERGES: &str = "#version: 0.2\nh e\nl l\nhe ll\nhell o</w>\nc a\nca t</w>\n";

    /// A tiny random CLIP checkpoint (32² images, 2 blocks per tower, OpenCLIP layout) and
    /// its merges, written into `dir` as `<name>.safetensors` and `merges.txt`; never a
    /// download. Returns their sizes in bytes, for a manifest entry.
    pub fn tiny_clip(dir: &std::path::Path, name: &str, seed: u64) -> (u64, u64) {
        use specular_infer::Tensor;
        use specular_infer::bpe::{Bpe, CLIP_MERGES};
        use specular_infer::clip::{ClipConfig, Layout};
        use specular_infer::models::{RandomWeights, Weights};

        struct Recorder(RandomWeights, Vec<(String, Tensor)>);
        impl Weights for Recorder {
            fn get(&mut self, n: &str, shape: &[usize]) -> Result<Tensor, specular_infer::Error> {
                let t = self.0.get(n, shape)?;
                self.1.push((n.to_owned(), t.clone()));
                Ok(t)
            }
        }
        static SEL: specular_core::paths::PathSelector = specular_core::paths::PathSelector::new();
        let mut rec = Recorder(RandomWeights { seed, f16: false }, Vec::new());
        let bpe = Bpe::from_merges(TINY_MERGES, CLIP_MERGES).expect("merges");
        crate::infer::InferRuntime::build_clip(
            "probe",
            ClipConfig::test(Layout::OpenClip, 520),
            crate::manifest::Norm::Openai,
            &mut rec,
            bpe,
            None,
            &SEL,
            1,
        )
        .expect("builds");
        let tensors: Vec<(&str, &Tensor)> = rec.1.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let bytes = specular_infer::safetensors::serialize(&tensors, &[]);
        std::fs::write(dir.join(format!("{name}.safetensors")), &bytes).expect("writes");
        std::fs::write(dir.join("merges.txt"), TINY_MERGES).expect("writes");
        (bytes.len() as u64, TINY_MERGES.len() as u64)
    }

    /// A tiny random PE Core checkpoint as Meta publishes it (a PyTorch `.pt`, f32; 32²
    /// images, 2 blocks per tower, logit scale included) and its merges, written into `dir`
    /// as `<name>.pt` and `merges.txt`. Returns their bytes.
    pub fn tiny_pecore(dir: &std::path::Path, name: &str, seed: u64) -> (Vec<u8>, Vec<u8>) {
        use specular_infer::Tensor;
        use specular_infer::bpe::{Bpe, CLIP_MERGES};
        use specular_infer::clip::{Clip, ImageInput};
        use specular_infer::models::{RandomWeights, Weights};

        struct Recorder(RandomWeights, Vec<(String, Tensor)>);
        impl Weights for Recorder {
            fn get(&mut self, n: &str, shape: &[usize]) -> Result<Tensor, specular_infer::Error> {
                let t = self.0.get(n, shape)?;
                self.1.push((n.to_owned(), t.clone()));
                Ok(t)
            }
        }
        let bpe = Bpe::from_merges(TINY_MERGES, CLIP_MERGES).expect("merges");
        let c = specular_infer::pecore::test_config(bpe.vocab_size());
        let mut rec = Recorder(RandomWeights { seed, f16: false }, Vec::new());
        Clip::build(c, &mut rec, bpe, ImageInput::Tensor { batch: 1 }, false).expect("builds");
        rec.1
            .push(("logit_scale".into(), Tensor::f32(&[], vec![4.6])));
        let mut seen = std::collections::HashSet::new();
        rec.1.retain(|(n, _)| seen.insert(n.clone()));
        let tensors: Vec<(&str, &Tensor)> = rec.1.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let bytes = specular_infer::pt::serialize(&tensors);
        std::fs::write(dir.join(format!("{name}.pt")), &bytes).expect("writes");
        std::fs::write(dir.join("merges.txt"), TINY_MERGES).expect("writes");
        (bytes, TINY_MERGES.as_bytes().to_vec())
    }

    /// A tiny random SigLIP checkpoint (32² images, 2 blocks per tower, f32 like the
    /// upstream download, logit scale and bias included) and its SentencePiece model,
    /// written into `dir` as `<name>.safetensors` and `tokenizer.model`. Returns their bytes.
    pub fn tiny_siglip(dir: &std::path::Path, name: &str, seed: u64) -> (Vec<u8>, Vec<u8>) {
        use specular_infer::Tensor;
        use specular_infer::clip::Clip;
        use specular_infer::clip::ImageInput;
        use specular_infer::models::{RandomWeights, Weights};
        use specular_infer::spm::{self, SentencePiece};

        struct Recorder(RandomWeights, Vec<(String, Tensor)>);
        impl Weights for Recorder {
            fn get(&mut self, n: &str, shape: &[usize]) -> Result<Tensor, specular_infer::Error> {
                let t = self.0.get(n, shape)?;
                self.1.push((n.to_owned(), t.clone()));
                Ok(t)
            }
        }
        let tok = spm::tiny(300);
        let sp = SentencePiece::from_bytes(&tok).expect("tokenizer");
        let mut rec = Recorder(RandomWeights { seed, f16: false }, Vec::new());
        Clip::build(
            specular_infer::siglip::test_config(300),
            &mut rec,
            sp,
            ImageInput::Tensor { batch: 1 },
            false,
        )
        .expect("builds");
        rec.1
            .push(("logit_scale".into(), Tensor::f32(&[1], vec![4.7])));
        rec.1
            .push(("logit_bias".into(), Tensor::f32(&[1], vec![-16.5])));
        // Once each, in the order first asked for.
        let mut seen = std::collections::HashSet::new();
        rec.1.retain(|(n, _)| seen.insert(n.clone()));
        let tensors: Vec<(&str, &Tensor)> = rec.1.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let bytes = specular_infer::safetensors::serialize(&tensors, &[("format", "pt")]);
        std::fs::write(dir.join(format!("{name}.safetensors")), &bytes).expect("writes");
        std::fs::write(dir.join("tokenizer.model"), &tok).expect("writes");
        (bytes, tok)
    }
}
