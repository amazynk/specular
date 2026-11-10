//! Specular's own inference runtime for the Local AI pack (DESIGN.md §10 "Local AI pack").
//!
//! A deliberately small op set, enough for the vision encoders the pack considers
//! (CLIP/SigLIP-class ViTs and their text towers, MobileNet/EfficientNet-style CNNs, face
//! detection and embedding nets), instead of a general framework:
//!
//! - [`tensor`]: host tensors, f32 or f16 storage (f16 is read as f32 and computed in f32).
//! - [`graph`]: a graph of ops with shapes inferred and checked as it is built; never
//!   panics on bad shapes from a weights file.
//! - [`cpu`]: the pure-Rust reference of every op. It is the oracle the GPU is checked
//!   against (`Task::Inference`'s reference path, DESIGN §12.3.1) and the fallback engine.
//! - [`pool`]: the CPU engine's worker threads, parked when idle.
//! - [`gpu`]: the same graph compiled to WGSL compute dispatches: buffers planned once (with
//!   reuse), bind groups prebuilt; a run goes in short submissions (`gpu::SUBMIT_MS`).
//! - [`safetensors`]: a bounded reader for `.safetensors` weights with its own JSON parser.
//! - [`clip`]: CLIP image and text towers from upstream checkpoints, with [`bpe`], its
//!   tokenizer; [`siglip`]: SigLIP 2's, read through the same loader, with [`spm`], our own
//!   SentencePiece tokenizer.
//! - [`models`]: builders for a ViT and a MobileNetV2-style CNN from named weights (random
//!   fixed-seed weights for tests and benchmarks).
//! - [`calibrate`]: the `Task::Inference` self-test hook.
//! - [`ocr`]: text in images: PaddleOCR's detector and line recognisers, Tesseract's LSTM.
//! - [`tflite`] and [`onnx`]: bounded readers of TensorFlow Lite and ONNX model files, and
//!   their translation into a graph ([`tflite_graph`], [`onnx::Model::build`]).
//! - [`tiles`]: tiled image-to-image inference with seamless joins (AI denoise).
//! - [`face`]: face detection, alignment, recognition, eye-blink scores, grouping and
//!   blur/exposure measures (People and Culling).
//! - [`ffdnet`]: FFDNet, a denoiser told the noise level (Develop's AI denoise).
//! - [`nafnet`]: NAFNet, the SIDD denoisers (Develop's AI denoise).
//! - [`pecore`]: Meta's Perception Encoder (PE Core), read from its PyTorch checkpoint by
//!   [`pt`], a pickle reader that never runs code.
//! - [`whisper`]: OpenAI's Whisper speech recogniser (generated subtitles): the encoder as a
//!   graph, the decoder token by token on the CPU with a key/value cache.
//!
//! Search's models are SigLIP 2 and PE Core (DESIGN.md, "Local AI pack").

#![feature(portable_simd)]

pub mod bpe;
pub mod calibrate;
pub mod clip;
pub mod cpu;
pub mod depth;
pub mod face;
pub mod ffdnet;
pub mod gpu;
pub mod graph;
pub mod isnet;
pub mod lama;
pub mod models;
pub mod nafnet;
pub mod ocr;
pub mod onnx;
pub mod pecore;
pub mod pool;
pub mod pt;
pub mod safetensors;
pub mod sam;
pub mod siglip;
pub mod spm;
pub mod tensor;
pub mod tflite;
pub mod tflite_graph;
pub mod tiles;
pub mod upernet;
pub mod whisper;

mod json;

pub use graph::{Act, Graph, Value};
pub use tensor::{DType, Tensor};

/// Errors from building or running a graph, or reading weights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A shape or parameter the op cannot take.
    Shape(String),
    /// A bad or unsupported weights file.
    Weights(String),
    /// A wrong feed for a run.
    Feed(String),
    /// The GPU could not build or run the graph.
    Gpu(String),
    /// The GPU device was lost (a driver reset): the graph must be built again on a new
    /// device. Not a fault of the device's driver or of the model.
    Lost,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Shape(s) => write!(f, "shape: {s}"),
            Error::Weights(s) => write!(f, "weights: {s}"),
            Error::Feed(s) => write!(f, "input: {s}"),
            Error::Gpu(s) => write!(f, "gpu: {s}"),
            Error::Lost => f.write_str("gpu: the device was lost"),
        }
    }
}

impl std::error::Error for Error {}

/// One graph input for a run, in the order the inputs were declared.
#[derive(Clone, Copy, Debug)]
pub enum Feed<'a> {
    Tensor(&'a Tensor),
    /// An RGBA8 image, rows top to bottom, no padding.
    Rgba {
        width: u32,
        height: u32,
        data: &'a [u8],
    },
    /// A GPU texture, `Rgba8Unorm` (not sRGB: the bytes are read as they are), for an image
    /// input. Only the GPU engine takes it.
    Texture(&'a wgpu::Texture),
}
