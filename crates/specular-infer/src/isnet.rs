//! IS-Net (Qin et al. 2022, "Highly Accurate Dichotomous Image Segmentation", Apache-2.0),
//! the general-use weights: Develop's "Subject" mask.
//!
//! Read from its ONNX export by [`crate::onnx`] (residual U-blocks of dilated 3×3 convs with
//! batch norms folded, max pools, bilinear upsampling, concatenations, sigmoid side
//! outputs). The input is the image stretched to 1024², scaled to 0..1 minus 0.5; the
//! first output is the foreground probability at 1024².

use crate::Error;
use crate::graph::{Crop, Graph, Preprocess};
use crate::onnx::Model;

/// Side of the square the network sees.
pub const INPUT: usize = 1024;

/// The preprocessing (the reference divides by 255, subtracts 0.5, divides by 1).
pub fn preprocess() -> Preprocess {
    Preprocess {
        width: INPUT,
        height: INPUT,
        crop: Crop::Stretch,
        antialias: true,
        mean: [0.5; 3],
        std: [1.0; 3],
    }
}

/// The graph: an RGBA image in, the probability `[1, 1, 1024, 1024]` out. Weights are
/// kept in f16 when `f16`.
pub fn graph(model: &Model, f16: bool) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let img = g.image_input()?;
    let x = g.preprocess(img, preprocess())?;
    let out = model.build(&mut g, x, f16)?;
    let p = *out
        .first()
        .ok_or_else(|| Error::Weights("isnet: no output".into()))?;
    if g.shape(p) != [1, 1, INPUT, INPUT] {
        return Err(Error::Weights(format!(
            "isnet: output {:?}, wanted [1, 1, {INPUT}, {INPUT}]",
            g.shape(p)
        )));
    }
    g.output(p)?;
    Ok(g)
}

/// The graph from a normalised `[1, 3, 1024, 1024]` tensor input (oracle tests).
pub fn graph_from_tensor(model: &Model, f16: bool) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let x = g.input(&[1, 3, INPUT, INPUT], crate::DType::F32)?;
    let out = model.build(&mut g, x, f16)?;
    let p = *out
        .first()
        .ok_or_else(|| Error::Weights("isnet: no output".into()))?;
    g.output(p)?;
    Ok(g)
}
