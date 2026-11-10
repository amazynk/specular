//! Speech to text for the app (generated subtitles): the Speech feature's installed Whisper
//! (its weights and vocabulary) loaded on our own engine (`specular_infer::whisper`).
//!
//! The encoder runs on the GPU unless the path selector blocked inference there; before its
//! first use a small random Whisper's encoder is run on both engines and compared (the same
//! ops, cheaply), the outcome told to the selector as other models' checks are. The
//! decoder always runs on the CPU.

use std::path::{Path, PathBuf};

use specular_core::paths::{Engine, PathSelector, Task, Trust};
use specular_infer::Error;
use specular_infer::gpu::Context;
use specular_infer::whisper::Whisper;

use crate::install::Installed;
use crate::manifest::{Feature, Manifest};

/// The Speech feature's installed files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeechFiles {
    pub model: PathBuf,
    pub vocab: PathBuf,
    /// The model's id in the manifest ("whisper-base").
    pub id: String,
}

impl SpeechFiles {
    /// The files among `installed` (paths relative to `root`), their roles from `manifest`.
    /// None when the Speech feature is not (fully) installed.
    pub fn find(manifest: &Manifest, root: &Path, installed: &[Installed]) -> Option<SpeechFiles> {
        let mut model = None;
        let mut vocab = None;
        for i in installed.iter().filter(|i| i.feature == Feature::Speech) {
            let m = manifest.model(&i.id)?;
            match m.role.as_str() {
                "speech-model" => model = Some((root.join(&i.file), m.id.clone())),
                "vocabulary" => vocab = Some(root.join(&i.file)),
                _ => {}
            }
        }
        let (model, id) = model?;
        Some(SpeechFiles {
            model,
            vocab: vocab?,
            id,
        })
    }

    /// Whether the model reads other languages than English.
    pub fn multilingual(&self) -> bool {
        !self.id.ends_with(".en")
    }

    /// Whether the model translates speech to English: large-v3 turbo was not trained to
    /// (it transcribes instead), English-only models have nothing to translate.
    pub fn translates(&self) -> bool {
        self.multilingual() && !self.id.contains("turbo")
    }
}

/// Whether the GPU computes a Whisper encoder as the CPU does (a small random one).
fn gpu_matches(ctx: &Context) -> Result<f32, Error> {
    use specular_infer::whisper::testing;
    let audio: Vec<f32> = (0..16_000)
        .map(|i| ((i as f32) * 0.031).sin() * 0.25)
        .collect();
    let mut g = testing::tiny(11, Some(ctx), true);
    let mut c = testing::tiny(11, None, true);
    let a = g.encode(&g.mel(&audio))?;
    let b = c.encode(&c.mel(&audio))?;
    let la = g.detect_language(&a)?;
    let lb = c.detect_language(&b)?;
    let worst = la
        .iter()
        .map(|(l, p)| {
            let q = lb.iter().find(|x| x.0 == *l).map_or(0.0, |x| x.1);
            (p - q).abs()
        })
        .fold(0f32, f32::max);
    Ok(worst)
}

/// Loads Whisper from `files`: the encoder on `gpu` when the selector allows and the GPU
/// matched the CPU, else the CPU.
pub fn load(
    files: &SpeechFiles,
    gpu: Option<&Context>,
    selector: &PathSelector,
    threads: usize,
) -> Result<Whisper, Error> {
    let mut use_gpu = None;
    if let Some(ctx) = gpu {
        let path = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
        if selector.trust(Task::Inference, path) != Trust::Blocked {
            match gpu_matches(ctx) {
                Ok(worst) => {
                    let ok = worst < 1e-3;
                    selector.verified(Task::Inference, Engine::Gpu, ok);
                    specular_log::info!(target: specular_log::JOBS,
                        "ai: speech encoder on the GPU {} the CPU (max difference {worst:.2e})",
                        if ok { "matches" } else { "differs from" });
                    if ok {
                        use_gpu = Some(ctx);
                    }
                }
                Err(e) => {
                    if matches!(e, Error::Gpu(_)) {
                        selector.block(Task::Inference, Engine::Gpu, &format!("speech: {e}"));
                    }
                    specular_log::warn!(target: specular_log::JOBS, "ai: speech on the GPU: {e}");
                }
            }
        }
    }
    let mut w = match Whisper::load(&files.model, &files.vocab, use_gpu, true) {
        Ok(w) => w,
        // Out of GPU memory, say: the CPU.
        Err(Error::Gpu(e)) if use_gpu.is_some() => {
            specular_log::warn!(target: specular_log::JOBS, "ai: speech encoder on the CPU: {e}");
            Whisper::load(&files.model, &files.vocab, None, true)?
        }
        Err(e) => return Err(e),
    };
    w.threads = threads.max(1);
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_installed_files() {
        let m = Manifest::load();
        let root = Path::new("/data/ai");
        let inst = |id: &str, file: &str| Installed {
            feature: Feature::Speech,
            tier: None,
            addon: None,
            id: id.into(),
            version: "1".into(),
            file: file.into(),
            sha256: [0; 32],
            size: 0,
            unverified: false,
        };
        let files = SpeechFiles::find(
            &m,
            root,
            &[
                inst("whisper-vocab", "models/whisper-vocab-1/model.json"),
                inst("whisper-base", "models/whisper-base-1/model.safetensors"),
            ],
        )
        .unwrap();
        assert_eq!(files.id, "whisper-base");
        assert_eq!(files.vocab, root.join("models/whisper-vocab-1/model.json"));
        assert!(SpeechFiles::find(&m, root, &[inst("whisper-base", "x")]).is_none());
    }

    #[test]
    fn turbo_does_not_translate() {
        let f = |id: &str| SpeechFiles {
            model: PathBuf::new(),
            vocab: PathBuf::new(),
            id: id.into(),
        };
        assert!(f("whisper-base").translates());
        assert!(f("whisper-small").translates());
        assert!(!f("whisper-large-v3-turbo").translates());
        assert!(f("whisper-large-v3-turbo").multilingual());
        assert!(!f("whisper-base.en").translates());
        // every speech model in the manifest is one of these
        let m = Manifest::load();
        let ids: Vec<_> = m
            .models
            .iter()
            .filter(|m| m.feature == Feature::Speech && m.role == "speech-model")
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(
            ids,
            ["whisper-base", "whisper-small", "whisper-large-v3-turbo"]
        );
    }
}
