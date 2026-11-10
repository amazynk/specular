//! The detail ops:
//!
//! - [`noise`]: the Poisson–Gaussian noise model (DNG NoiseProfile or a fit on the image)
//!   and the generalised Anscombe transform;
//! - [`denoise`]: wavelet shrinkage of luma and chroma, then non-local means on the luma,
//!   in sensor geometry;
//! - [`capture`]: capture sharpening by Richardson–Lucy deconvolution;
//! - [`sharpen`]: unsharp masking with a details mask and no halos;
//! - [`diffuse`]: diffuse or sharpen, anisotropic diffusion on wavelet scales;
//! - [`surface`]: surface blur, a bilateral grid;
//! - [`ai`]: denoise's AI mode, a network's result blended with the classic one.
//!
//! The dual demosaic is part of the raw front end ([`crate::ops::raw`]).
//!
//! The noise model and the lens's blur belong to the image, not to the edit: the preview and
//! export set them on a copy of the edit before building the graph ([`with_image`]), so they
//! are never stored, pasted or put in a style.

pub mod ai;
pub mod capture;
pub mod denoise;
pub mod diffuse;
pub mod kernels;
pub mod noise;
pub mod sharpen;
pub mod surface;

#[cfg(test)]
pub(crate) mod cpu;
#[cfg(test)]
pub(crate) mod cpu_diffuse;
#[cfg(test)]
pub(crate) mod diffuse_legacy;
#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod tests_ai_denoise;
#[cfg(test)]
mod tests_atrous_groups;
#[cfg(test)]
mod tests_diffuse;
#[cfg(test)]
mod tests_diffuse_budget;
#[cfg(test)]
mod tests_diffuse_speed;
#[cfg(test)]
mod tests_rawdenoise;
#[cfg(test)]
mod tests_realnoise;
#[cfg(test)]
mod tests_stripes;

pub use noise::{NoiseModel, RawNoise};

use crate::model::{Edit, ModuleKind, Value};

/// What the detail ops need to know about the image being developed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ImageDetail {
    /// Noise of the developed source buffer.
    pub noise: Option<NoiseModel>,
    /// The lens's blur σ in source pixels ([`capture::auto_sigma`]).
    pub capture_sigma: Option<f32>,
    /// The photo's identity (the app's, [`crate::mask::ai::photo_id`]): keys denoise's AI
    /// result ([`ai::key`]).
    pub photo: Option<[u8; 32]>,
}

/// `edit` with the image's facts set on its detail modules (`None` when it has none of
/// them enabled, so the caller uses the edit as it is).
pub fn with_image(edit: &Edit, info: &ImageDetail) -> Option<Edit> {
    let wants = |k: ModuleKind| {
        edit.modules
            .iter()
            .any(|m| m.kind == k && m.enabled && m.version == 1)
    };
    let noise = info.noise.filter(|_| wants(ModuleKind::DENOISE));
    let sigma = info
        .capture_sigma
        .filter(|_| wants(ModuleKind::CAPTURE_SHARPENING));
    // Denoise's AI result is keyed by the photo and what comes before denoise.
    let ai_key = ai::ai_module(edit)
        .zip(info.photo)
        .map(|(a, photo)| ai::key(&photo, edit, &a.model));
    if noise.is_none() && sigma.is_none() && ai_key.is_none() {
        return None;
    }
    let mut e = edit.clone();
    for m in &mut e.modules {
        if m.version != 1 {
            continue;
        }
        let mut f = m.params.to_fields();
        let keyed = m.kind == ModuleKind::DENOISE && ai_key.is_some();
        if keyed {
            ai::write_hidden(&mut f, ai_key.as_ref(), None);
        }
        match (m.kind, noise, sigma) {
            (ModuleKind::DENOISE, Some(n), _) => n.write(&mut f),
            (ModuleKind::CAPTURE_SHARPENING, _, Some(s)) => f.set("auto_sigma", Value::Float(s)),
            _ if keyed => {}
            _ => continue,
        }
        m.params.set_fields(f);
    }
    Some(e)
}
