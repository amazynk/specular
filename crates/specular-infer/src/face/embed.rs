//! Face recognisers: an aligned 112×112 crop in, a unit embedding out; faces of one person
//! have embeddings with a high cosine.
//!
//! - **SFace** (`face_recognition_sface_2021dec.onnx`, OpenCV Zoo): MobileFaceNet, 128-d. It
//!   takes RGB 0..255 and normalises inside the graph.
//! - **AuraFace v1** (`glintr100.onnx`): IResNet-100, 512-d. It takes RGB scaled to [-1, 1].
//!
//! [`Embedder::embed_all`] runs a photo's faces as batches of [`BATCH`] (one graph per batch
//! size, built on first use; a partly filled batch is padded with zeros).

use super::{Image, Runner, align, blazeface::Face, normalize};
use crate::gpu::Context;
use crate::onnx::Model;
use crate::{DType, Error, Feed, Graph, Tensor};

/// Which recogniser, for its input scaling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Input 0..255 (the graph normalises).
    Sface,
    /// Input (v − 127.5) / 127.5.
    AuraFace,
}

impl Kind {
    /// (scale, offset) applied to 0..255 values.
    pub fn input_map(self) -> (f32, f32) {
        match self {
            Kind::Sface => (1.0, 0.0),
            Kind::AuraFace => (1.0 / 127.5, -1.0),
        }
    }
}

/// Faces per batched run.
pub const BATCH: usize = 8;

/// A loaded recogniser.
pub struct Embedder {
    pub kind: Kind,
    pub dims: usize,
    runner: Runner,
    /// The model, kept to build the batched graph.
    onnx: Vec<u8>,
    f16: bool,
    gpu: Option<Context>,
    threads: usize,
    /// The graph for [`BATCH`] faces (None: not built yet; Some(None): it can't be).
    batched: Option<Option<Runner>>,
}

fn build(onnx: &[u8], n: usize, f16: bool) -> Result<(Graph, usize), Error> {
    let m = Model::parse(onnx)?;
    let mut g = Graph::new();
    let x = g.input(&[n, 3, align::SIDE, align::SIDE], DType::F32)?;
    let outs = m.build(&mut g, x, f16)?;
    let [y] = outs[..] else {
        return Err(Error::Weights("a face recogniser has one output".into()));
    };
    let s = g.shape(y).to_vec();
    if s.len() != 2 || s[0] != n || s[1] < 16 {
        return Err(Error::Weights(format!("embedding of shape {s:?}")));
    }
    g.output(y)?;
    Ok((g, s[1]))
}

impl Embedder {
    /// Builds the network from an `.onnx` file's bytes; weights are kept as f16 when `f16`.
    pub fn new(
        onnx: &[u8],
        kind: Kind,
        f16: bool,
        gpu: Option<&Context>,
        threads: usize,
    ) -> Result<Embedder, Error> {
        let (g, dims) = build(onnx, 1, f16)?;
        Ok(Embedder {
            kind,
            dims,
            runner: Runner::new(g, gpu, threads),
            onnx: onnx.to_vec(),
            f16,
            gpu: gpu.cloned(),
            threads,
            batched: None,
        })
    }

    pub fn on_gpu(&self) -> bool {
        self.runner.on_gpu()
    }

    /// The network input for face `f` of `img` (None for a degenerate face).
    pub fn input(&self, img: Image, f: &Face) -> Option<Tensor> {
        let m = align::crop_to_image(f)?;
        let (s, o) = self.kind.input_map();
        Some(super::warp(img, &m, align::SIDE, align::SIDE, s, o))
    }

    /// The raw (not normalised) embedding of a prepared input, on the engine or the CPU.
    pub fn raw(&mut self, x: &Tensor, cpu: bool) -> Result<Vec<f32>, Error> {
        let out = if cpu {
            self.runner.run_cpu(&[Feed::Tensor(x)], 4)?
        } else {
            self.runner.run(&[Feed::Tensor(x)])?
        };
        Ok(out[0].to_f32())
    }

    /// The raw embeddings of up to [`BATCH`] prepared inputs in one run (the batched graph,
    /// on the same engine as the single one); None when the model can't be batched.
    pub fn raw_batch(&mut self, xs: &[Tensor]) -> Result<Option<Vec<Vec<f32>>>, Error> {
        if xs.is_empty() || xs.len() > BATCH {
            return Ok(None);
        }
        if self.batched.is_none() {
            let gpu = if self.runner.on_gpu() {
                self.gpu.as_ref()
            } else {
                None
            };
            self.batched = Some(
                build(&self.onnx, BATCH, self.f16)
                    .ok()
                    .map(|(g, _)| Runner::new(g, gpu, self.threads)),
            );
        }
        let Some(Some(r)) = self.batched.as_mut() else {
            return Ok(None);
        };
        let per = 3 * align::SIDE * align::SIDE;
        let mut v = vec![0f32; BATCH * per];
        for (k, x) in xs.iter().enumerate() {
            v[k * per..(k + 1) * per].copy_from_slice(&x.to_f32());
        }
        let t = Tensor::f32(&[BATCH, 3, align::SIDE, align::SIDE], v);
        let out = r.run(&[Feed::Tensor(&t)])?[0].to_f32();
        let d = self.dims;
        Ok(Some(
            (0..xs.len())
                .map(|k| out[k * d..(k + 1) * d].to_vec())
                .collect(),
        ))
    }

    /// The unit embeddings of `faces` of `img` (None for a degenerate face): one face alone
    /// through the single graph, more in batches of [`BATCH`].
    pub fn embed_all(
        &mut self,
        img: Image,
        faces: &[Face],
    ) -> Result<Vec<Option<Vec<f32>>>, Error> {
        let inputs: Vec<Option<Tensor>> = faces.iter().map(|f| self.input(img, f)).collect();
        let idx: Vec<usize> = (0..faces.len()).filter(|&i| inputs[i].is_some()).collect();
        let mut out: Vec<Option<Vec<f32>>> = vec![None; faces.len()];
        let single = |me: &mut Embedder, i: usize| -> Result<Option<Vec<f32>>, Error> {
            let x = inputs[i].as_ref().expect("kept");
            let mut v = me.raw(x, false)?;
            normalize(&mut v);
            Ok(Some(v))
        };
        if idx.len() < 2 {
            for &i in &idx {
                out[i] = single(self, i)?;
            }
            return Ok(out);
        }
        for chunk in idx.chunks(BATCH) {
            let xs: Vec<Tensor> = chunk
                .iter()
                .map(|&i| inputs[i].clone().expect("kept"))
                .collect();
            match self.raw_batch(&xs)? {
                Some(vs) => {
                    for (&i, mut v) in chunk.iter().zip(vs) {
                        normalize(&mut v);
                        out[i] = Some(v);
                    }
                }
                None => {
                    for &i in chunk {
                        out[i] = single(self, i)?;
                    }
                }
            }
        }
        Ok(out)
    }

    /// The unit embedding of face `f` (None for a degenerate face).
    pub fn embed(&mut self, img: Image, f: &Face) -> Result<Option<Vec<f32>>, Error> {
        let Some(x) = self.input(img, f) else {
            return Ok(None);
        };
        let mut v = self.raw(&x, false)?;
        normalize(&mut v);
        Ok(Some(v))
    }
}
