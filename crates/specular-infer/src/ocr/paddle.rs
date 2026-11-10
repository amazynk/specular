//! PaddleOCR networks from their Hugging Face safetensors exports (`PaddlePaddle/*_safetensors`):
//! the PP-OCRv6 small and PP-OCRv5 mobile text detectors, the PP-OCRv6 small/tiny and
//! PP-OCRv5 mobile line recognisers and the PP-LCNet text-line orientation classifier,
//! built as [`Graph`]s for one input size at a time.
//!
//! The architecture comes from the export's `config.json` (stage layout and strides), the
//! weights from `model.safetensors` (batch norms folded into the convolutions here, the
//! PP-LCNetV3 re-parameterised branches summed into one kernel), the character list from
//! `preprocessor_config.json`.
//!
//! - **Backbones**: PP-LCNetV4 (large stem: conv, two 2×2 convs beside a 2×2 max pool,
//!   concatenated; or the small stem: two strided convs with an erf GELU) and PP-LCNetV3
//!   (every depthwise and pointwise conv a sum of parallel branches with a learnable scalar
//!   affine before and after the hard-swish).
//! - **Detector** (DB): each stage through a 1×1 conv and a residual squeeze-excitation
//!   (gate clamp(0.2x + 0.5)), added top-down with nearest ×2 upsampling, each level through
//!   a 7×7 depthwise conv, a 1×1 conv to a quarter of the channels and another residual SE,
//!   upsampled to ¼ resolution and concatenated; then a 3×3 conv and two 2×2 stride-2
//!   transposed convs (each a 1×1 conv to four times the channels, then a pixel shuffle)
//!   to a sigmoid probability map at full resolution. PP-OCRv5 mobile's detector (the
//!   fallback) first maps each PP-LCNetV3 stage through a 1×1 conv with bias and has a
//!   plain 3×3 conv where v6 has the depthwise and 1×1 pair.
//! - **Recognisers** (CTC): the last stage average-pooled 3×2 to one row of W/8 steps;
//!   PP-OCRv6 small and PP-OCRv5 mobile through an SVTR encoder (convs, two pre-norm
//!   transformer blocks with a SiLU MLP), PP-OCRv6 tiny through two 1-D convs and two linear
//!   layers; then the class scores and a softmax.
//! - **Orientation classifier** (PP-LCNet x1.0, 80×160 input): depthwise-separable blocks
//!   (hard-swish, squeeze-excitation in the last stage), global mean, a 1×1 conv to 1280 with
//!   hard-swish, a linear layer to 0° / 180° and a softmax.
//!
//! Paddle reads images as BGR: the first convolution's input channels are swapped at load,
//! so the graphs take RGB.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::Error;
use crate::graph::{
    Act, BatchNorm, Conv2d, Graph, Pool2d, PoolKind, Preprocess, ResizeMode, Value, fold_batchnorm,
};
use crate::json::{self, Json};
use crate::safetensors::SafeTensorsFile;
use crate::tensor::{DType, Tensor};

/// Batch-norm epsilon of every Paddle model here.
const BN_EPS: f32 = 1e-5;
/// Height of a recogniser's input line.
pub const REC_HEIGHT: usize = 48;
/// Largest recogniser input width (Paddle's `max_image_width`).
pub const REC_MAX_WIDTH: usize = 3200;
/// Bounds on the numbers a config may give.
const MAX_CHANNELS: usize = 4096;
const MAX_CLASSES: usize = 100_000;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

/// The weights of one model, read once and kept for building graphs of any input size.
/// A random bank (tests) makes up any tensor it is asked for, from its name and a seed.
#[derive(Debug, Default)]
pub struct Bank {
    tensors: Mutex<HashMap<String, Tensor>>,
    random: Option<u64>,
}

impl Bank {
    pub fn open(path: &Path) -> Result<Bank, Error> {
        let mut f = SafeTensorsFile::open(path)?;
        let names: Vec<String> = f.header.tensors.iter().map(|t| t.name.clone()).collect();
        let mut tensors = HashMap::with_capacity(names.len());
        for n in names {
            if n.ends_with("num_batches_tracked") {
                continue;
            }
            let t = f.tensor(&n)?;
            tensors.insert(n, t);
        }
        Ok(Bank {
            tensors: Mutex::new(tensors),
            random: None,
        })
    }

    pub fn from_tensors(tensors: impl IntoIterator<Item = (String, Tensor)>) -> Bank {
        Bank {
            tensors: Mutex::new(tensors.into_iter().collect()),
            random: None,
        }
    }

    /// Fixed-seed random weights of whatever shapes a build asks for.
    pub fn random(seed: u64) -> Bank {
        Bank {
            tensors: Mutex::new(HashMap::new()),
            random: Some(seed),
        }
    }

    /// The tensors held (for a random bank: those made so far), by name.
    pub fn tensors(&self) -> Vec<(String, Tensor)> {
        let m = self.tensors.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<_> = m.iter().map(|(k, t)| (k.clone(), t.clone())).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    fn shaped(&self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
        let mut m = self.tensors.lock().unwrap_or_else(|e| e.into_inner());
        if let (Some(seed), false) = (self.random, m.contains_key(name)) {
            use crate::models::{RandomWeights, Weights};
            let mut t = RandomWeights::new(seed).get(name, shape)?;
            if name.ends_with(".scale") && shape == [1] {
                t = Tensor::f32(&[1], vec![1.0 + t.to_f32()[0]]);
            }
            m.insert(name.to_owned(), t);
        }
        let Some(t) = m.get(name) else {
            return bad(format!("no tensor {name:?}"));
        };
        if t.shape != shape {
            return bad(format!("{name:?} is {:?}, wanted {shape:?}", t.shape));
        }
        Ok(t.clone())
    }

    fn vec(&self, name: &str, n: usize) -> Result<Vec<f32>, Error> {
        Ok(self.shaped(name, &[n])?.to_f32())
    }

    fn scalar(&self, name: &str) -> Result<f32, Error> {
        Ok(self.shaped(name, &[1])?.to_f32()[0])
    }

    fn bn(&self, prefix: &str, n: usize) -> Result<BatchNorm, Error> {
        Ok(BatchNorm {
            gamma: self.vec(&format!("{prefix}.weight"), n)?,
            beta: self.vec(&format!("{prefix}.bias"), n)?,
            mean: self.vec(&format!("{prefix}.running_mean"), n)?,
            var: self.vec(&format!("{prefix}.running_var"), n)?,
            eps: BN_EPS,
        })
    }
}

/// A stride of a config block: one number, or (y, x).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stride {
    One(usize),
    Pair(usize, usize),
}

impl Stride {
    fn yx(self) -> [usize; 2] {
        match self {
            Stride::One(s) => [s, s],
            Stride::Pair(y, x) => [y, x],
        }
    }

    /// Python's `stride == n` (a pair never equals a number).
    fn is(self, n: usize) -> bool {
        self == Stride::One(n)
    }
}

/// One block of a backbone stage: kernel, in and out channels, stride, squeeze-excitation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockSpec {
    pub kernel: usize,
    pub cin: usize,
    pub cout: usize,
    pub stride: Stride,
    pub se: bool,
}

/// A backbone.
#[derive(Clone, Debug, PartialEq)]
pub enum Backbone {
    /// PP-LCNetV4.
    V4 {
        large_stem: bool,
        /// Stem channels: input, middle, output.
        stem: [usize; 3],
        stem_strides: [usize; 5],
        act: Act,
        reduction: usize,
        stages: Vec<Vec<BlockSpec>>,
    },
    /// PP-LCNet (the first version; channel counts already scaled), as the orientation
    /// classifier has it.
    V1 {
        stem: usize,
        stem_stride: usize,
        act: Act,
        reduction: usize,
        stages: Vec<Vec<BlockSpec>>,
    },
    /// PP-LCNetV3 (channel counts already scaled).
    V3 {
        stem: usize,
        stem_stride: usize,
        act: Act,
        reduction: usize,
        branches: usize,
        stages: Vec<Vec<BlockSpec>>,
    },
}

impl Backbone {
    fn out_channels(&self) -> usize {
        let stages = match self {
            Backbone::V4 { stages, .. }
            | Backbone::V3 { stages, .. }
            | Backbone::V1 { stages, .. } => stages,
        };
        stages.last().and_then(|s| s.last()).map_or(0, |b| b.cout)
    }
}

/// The part after the backbone.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadSpec {
    /// DB text detection; `v5`: PP-OCRv5 mobile's (stage 1×1 convs, plain 3×3 neck convs).
    Det {
        v5: bool,
        layer_channels: Vec<usize>,
        neck: usize,
        kernels: [usize; 3],
        dw_kernel: usize,
        reduction: usize,
    },
    /// SVTR encoder and CTC head: PP-OCRv6 small (`v6`) or PP-OCRv5 mobile.
    Svtr {
        v6: bool,
        hidden: usize,
        depth: usize,
        heads: usize,
        mlp: usize,
        kernel: [usize; 2],
        act: Act,
        eps: f32,
        classes: usize,
    },
    /// PP-OCRv6 tiny's 1-D conv head.
    Tiny { hidden: usize, classes: usize },
    /// An image classifier: mean, 1×1 conv to `expand` + hard-swish, times `keep` (Paddle's
    /// "downscale in inference" dropout), linear to `classes`.
    Cls {
        expand: usize,
        classes: usize,
        keep: f32,
    },
}

/// A model's architecture, from its `config.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct Arch {
    pub model_type: String,
    pub backbone: Backbone,
    pub head: HeadSpec,
}

fn act_named(s: &str) -> Result<Act, Error> {
    Ok(match s {
        "relu" => Act::Relu,
        "gelu" => Act::Gelu,
        "silu" | "swish" => Act::Silu,
        "hardswish" | "hard_swish" => Act::HardSwish,
        "sigmoid" => Act::Sigmoid,
        other => return bad(format!("activation {other:?}")),
    })
}

fn num(j: &Json, what: &str) -> Result<usize, Error> {
    match j.as_u64() {
        Some(n) if (n as usize) <= MAX_CLASSES => Ok(n as usize),
        _ => bad(format!("{what}: not a size")),
    }
}

fn field<'a>(j: &'a Json, key: &str) -> Option<&'a Json> {
    j.get(key)
}

fn usize_or(j: &Json, key: &str, default: usize) -> Result<usize, Error> {
    match field(j, key) {
        Some(v) => num(v, key),
        None => Ok(default),
    }
}

fn f64_or(j: &Json, key: &str, default: f64) -> f64 {
    field(j, key).and_then(Json::as_f64).unwrap_or(default)
}

fn usizes(j: &Json, key: &str) -> Result<Option<Vec<usize>>, Error> {
    match field(j, key) {
        None => Ok(None),
        Some(v) => {
            let Some(a) = v.as_array() else {
                return bad(format!("{key}: not a list"));
            };
            a.iter()
                .map(|x| num(x, key))
                .collect::<Result<_, _>>()
                .map(Some)
        }
    }
}

/// `make_divisible` of PP-LCNetV3: the nearest multiple of `divisor`, at least `divisor`,
/// never under 90 % of `value`.
pub fn make_divisible(value: f64, divisor: usize) -> usize {
    let d = divisor as f64;
    let mut n = (((value + d / 2.0) as usize) / divisor * divisor).max(divisor);
    if (n as f64) < 0.9 * value {
        n += divisor;
    }
    n
}

fn parse_blocks(j: &Json, scale: Option<(f64, usize)>) -> Result<Vec<Vec<BlockSpec>>, Error> {
    let Some(stages) = j.get("block_configs").and_then(Json::as_array) else {
        return bad("backbone without block_configs");
    };
    let mut out = Vec::new();
    for st in stages {
        let Some(blocks) = st.as_array() else {
            return bad("block_configs: stage is not a list");
        };
        let mut v = Vec::new();
        for b in blocks {
            let Some(f) = b.as_array().filter(|f| f.len() == 5) else {
                return bad("block_configs: block is not [k, in, out, stride, se]");
            };
            let stride = match f[3].as_array() {
                Some(p) if p.len() == 2 => {
                    Stride::Pair(num(&p[0], "stride")?, num(&p[1], "stride")?)
                }
                Some(_) => return bad("block stride"),
                None => Stride::One(num(&f[3], "stride")?),
            };
            let [sy, sx] = stride.yx();
            let (mut cin, mut cout) = (num(&f[1], "in")?, num(&f[2], "out")?);
            if let Some((s, d)) = scale {
                cin = make_divisible(cin as f64 * s, d);
                cout = make_divisible(cout as f64 * s, d);
            }
            let kernel = num(&f[0], "kernel")?;
            if kernel == 0 || kernel > 15 || sy == 0 || sx == 0 || sy > 4 || sx > 4 {
                return bad("block kernel or stride out of range");
            }
            if cin == 0 || cout == 0 || cin > MAX_CHANNELS || cout > MAX_CHANNELS {
                return bad("block channels out of range");
            }
            v.push(BlockSpec {
                kernel,
                cin,
                cout,
                stride,
                se: f[4].as_bool().unwrap_or(false),
            });
        }
        out.push(v);
    }
    if out.is_empty() || out.iter().any(Vec::is_empty) {
        return bad("empty backbone stage");
    }
    Ok(out)
}

/// PP-LCNetV3's default stage layout (transformers' `PPLCNetV3Config`).
const V3_BLOCKS: &str = r#"{"block_configs": [
 [[3, 16, 32, 1, false]],
 [[3, 32, 64, 2, false], [3, 64, 64, 1, false]],
 [[3, 64, 128, 2, false], [3, 128, 128, 1, false]],
 [[3, 128, 256, 2, false], [5, 256, 256, 1, false], [5, 256, 256, 1, false],
  [5, 256, 256, 1, false], [5, 256, 256, 1, false]],
 [[5, 256, 512, 2, true], [5, 512, 512, 1, true], [5, 512, 512, 1, false],
  [5, 512, 512, 1, false]]]}"#;

fn parse_backbone(j: &Json) -> Result<Backbone, Error> {
    let ty = j.get("model_type").and_then(Json::as_str).unwrap_or("");
    let act = act_named(j.get("hidden_act").and_then(Json::as_str).unwrap_or(
        if ty == "pp_lcnet_v3" || ty == "pp_lcnet" {
            "hardswish"
        } else {
            "relu"
        },
    ))?;
    let reduction = usize_or(j, "reduction", 4)?.max(1);
    match ty {
        "pp_lcnet_v4" => {
            let stem = usizes(j, "stem_channels")?.unwrap_or(vec![3, 32, 64]);
            let ss = usizes(j, "stem_strides")?.unwrap_or(vec![2, 1, 1, 2, 1]);
            if stem.len() != 3 || ss.len() != 5 || stem[0] != 3 {
                return bad("stem_channels / stem_strides");
            }
            let large = match j.get("stem_type").and_then(Json::as_str) {
                Some("small") => false,
                Some("large") | None => true,
                Some(o) => return bad(format!("stem_type {o:?}")),
            };
            if j.get("use_learnable_affine_block").and_then(Json::as_bool) == Some(true) {
                return bad("PP-LCNetV4 with learnable affine blocks");
            }
            if (f64_or(j, "scale", 1.0) - 1.0).abs() > 1e-9 {
                return bad("PP-LCNetV4 with a width scale");
            }
            Ok(Backbone::V4 {
                large_stem: large,
                stem: [stem[0], stem[1], stem[2]],
                stem_strides: [ss[0], ss[1], ss[2], ss[3], ss[4]],
                act,
                reduction,
                stages: parse_blocks(j, None)?,
            })
        }
        "pp_lcnet_v3" | "pp_lcnet" => {
            let scale = f64_or(j, "scale", 1.0);
            let divisor = usize_or(j, "divisor", 8)?.max(1);
            if !(0.1..=4.0).contains(&scale) {
                return bad("PP-LCNet scale");
            }
            let stem = make_divisible(usize_or(j, "stem_channels", 16)? as f64 * scale, divisor);
            let stem_stride = usize_or(j, "stem_stride", 2)?.clamp(1, 4);
            let stages = if j.get("block_configs").is_some() {
                parse_blocks(j, Some((scale, divisor)))?
            } else if ty == "pp_lcnet_v3" {
                // The detector's export leaves PP-LCNetV3's default layout out.
                let d = json::parse(V3_BLOCKS.as_bytes()).map_err(Error::Weights)?;
                parse_blocks(&d, Some((scale, divisor)))?
            } else {
                return bad("PP-LCNet without block_configs");
            };
            if ty == "pp_lcnet" {
                return Ok(Backbone::V1 {
                    stem,
                    stem_stride,
                    act,
                    reduction,
                    stages,
                });
            }
            Ok(Backbone::V3 {
                stem,
                stem_stride,
                act,
                reduction,
                branches: usize_or(j, "conv_symmetric_num", 4)?.clamp(1, 8),
                stages,
            })
        }
        other => bad(format!("backbone {other:?}")),
    }
}

impl Arch {
    /// Reads a `config.json`.
    pub fn parse(text: &[u8]) -> Result<Arch, Error> {
        let j = json::parse(text).map_err(|e| Error::Weights(format!("config.json: {e}")))?;
        let ty = j
            .get("model_type")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_owned();
        if ty == "pp_lcnet" {
            // A classifier: the backbone's settings at the top level.
            let classes = match j.get("id2label").and_then(Json::as_object) {
                Some(m) => m.len(),
                None => usize_or(&j, "num_labels", 0)?,
            };
            if classes < 2 {
                return bad("classifier without labels");
            }
            return Ok(Arch {
                model_type: ty,
                backbone: parse_backbone(&j)?,
                head: HeadSpec::Cls {
                    expand: usize_or(&j, "class_expand", 1280)?,
                    classes,
                    keep: (1.0 - f64_or(&j, "hidden_dropout_prob", 0.0).clamp(0.0, 0.99)) as f32,
                },
            });
        }
        let Some(bb) = j.get("backbone_config") else {
            return bad("config.json without backbone_config");
        };
        let backbone = parse_backbone(bb)?;
        let head = match ty.as_str() {
            "pp_ocrv6_small_det" | "pp_ocrv5_mobile_det" => {
                let k = usizes(&j, "kernel_list")?.unwrap_or(vec![3, 2, 2]);
                if k.len() != 3 || k[1] != 2 || k[2] != 2 {
                    return bad("detector head kernels other than [k, 2, 2]");
                }
                let layer_channels =
                    usizes(&j, "layer_list_out_channels")?.unwrap_or(vec![48, 96, 192, 384]);
                if layer_channels.len() != 4 {
                    return bad("detector with other than four levels");
                }
                HeadSpec::Det {
                    v5: ty == "pp_ocrv5_mobile_det",
                    layer_channels,
                    neck: usize_or(&j, "neck_out_channels", 96)?,
                    kernels: [k[0], k[1], k[2]],
                    dw_kernel: usize_or(&j, "dilated_kernel_size", 7)?,
                    reduction: usize_or(&j, "reduction", 4)?.max(1),
                }
            }
            "pp_ocrv6_small_rec" | "pp_ocrv5_mobile_rec" | "pp_ocrv5_server_rec" => {
                let hidden = usize_or(&j, "hidden_size", 120)?;
                let heads = usize_or(&j, "num_attention_heads", 8)?;
                if heads == 0 || hidden == 0 || hidden % heads != 0 {
                    return bad("SVTR heads");
                }
                let k = usizes(&j, "conv_kernel_size")?.unwrap_or(vec![1, 3]);
                if k.len() != 2 {
                    return bad("conv_kernel_size");
                }
                if j.get("qkv_bias").and_then(Json::as_bool) == Some(false) {
                    return bad("SVTR without a qkv bias");
                }
                HeadSpec::Svtr {
                    v6: ty == "pp_ocrv6_small_rec",
                    hidden,
                    depth: usize_or(&j, "depth", 2)?,
                    heads,
                    mlp: (hidden as f64 * f64_or(&j, "mlp_ratio", 2.0)) as usize,
                    kernel: [k[0], k[1]],
                    act: act_named(j.get("hidden_act").and_then(Json::as_str).unwrap_or("silu"))?,
                    eps: f64_or(&j, "layer_norm_eps", 1e-6) as f32,
                    classes: usize_or(&j, "head_out_channels", 0)?,
                }
            }
            "pp_ocrv6_tiny_rec" => HeadSpec::Tiny {
                hidden: usize_or(&j, "hidden_size", 80)?,
                classes: usize_or(&j, "head_out_channels", 0)?,
            },
            other => return bad(format!("model type {other:?}")),
        };
        Ok(Arch {
            model_type: ty,
            backbone,
            head,
        })
    }

    pub fn is_detector(&self) -> bool {
        matches!(self.head, HeadSpec::Det { .. })
    }

    pub fn is_classifier(&self) -> bool {
        matches!(self.head, HeadSpec::Cls { .. })
    }

    /// Output classes of a recogniser (blank included).
    pub fn classes(&self) -> usize {
        match self.head {
            HeadSpec::Svtr { classes, .. }
            | HeadSpec::Tiny { classes, .. }
            | HeadSpec::Cls { classes, .. } => classes,
            HeadSpec::Det { .. } => 0,
        }
    }
}

/// The recogniser's character list from `preprocessor_config.json` (`character_list`,
/// index 0 the CTC blank).
pub fn read_characters(text: &[u8]) -> Result<Vec<String>, Error> {
    let j =
        json::parse(text).map_err(|e| Error::Weights(format!("preprocessor_config.json: {e}")))?;
    let Some(list) = j.get("character_list").and_then(Json::as_array) else {
        return bad("preprocessor_config.json without character_list");
    };
    let v: Vec<String> = list
        .iter()
        .map(|c| c.as_str().map(str::to_owned))
        .collect::<Option<_>>()
        .ok_or(Error::Weights("character_list: not all strings".into()))?;
    if v.len() < 2 || v.len() > MAX_CLASSES {
        return bad("character_list size");
    }
    Ok(v)
}

/// Graph-building state: the graph, the weights, whether to store matrices as f16.
struct Net<'a> {
    g: &'a mut Graph,
    w: &'a Bank,
    f16: bool,
    /// Swap the input channels (RGB ↔ BGR) of the next convolution reading the image.
    bgr: bool,
}

/// One convolution's folded parameters.
struct Folded {
    w: Tensor,
    b: Tensor,
}

impl Net<'_> {
    fn konst(&mut self, t: Tensor) -> Result<Value, Error> {
        let t = if self.f16 && t.shape.len() >= 2 && t.dtype() == DType::F32 {
            t.to_f16()
        } else {
            t
        };
        self.g.constant(t)
    }

    fn swap_bgr(&mut self, w: Tensor) -> Tensor {
        if !std::mem::take(&mut self.bgr) || w.shape.len() != 4 || w.shape[1] != 3 {
            return w;
        }
        let s = w.shape.clone();
        let per = s[2] * s[3];
        let mut v = w.to_f32();
        for o in 0..s[0] {
            for i in 0..per {
                v.swap(o * 3 * per + i, o * 3 * per + 2 * per + i);
            }
        }
        let t = Tensor::f32(&s, v);
        if w.dtype() == DType::F16 {
            t.to_f16()
        } else {
            t
        }
    }

    fn conv(&mut self, x: Value, f: Folded, p: Conv2d) -> Result<Value, Error> {
        let w = self.swap_bgr(f.w);
        let w = self.konst(w)?;
        let b = self.konst(f.b)?;
        self.g.conv2d(x, w, Some(b), p)
    }

    /// A conv followed by a batch norm: `{prefix}.{conv}.weight` and `{prefix}.{norm}.*`.
    fn conv_bn_weights(
        &self,
        conv: &str,
        norm: &str,
        cout: usize,
        cin_g: usize,
        k: [usize; 2],
        bias: bool,
    ) -> Result<Folded, Error> {
        let w = self
            .w
            .shaped(&format!("{conv}.weight"), &[cout, cin_g, k[0], k[1]])?;
        let b = if bias {
            Some(self.w.shaped(&format!("{conv}.bias"), &[cout])?)
        } else {
            None
        };
        let bn = self.w.bn(norm, cout)?;
        let (w, b) = fold_batchnorm(&w, b.as_ref(), &bn)?;
        Ok(Folded { w, b })
    }

    fn plain_weights(
        &self,
        conv: &str,
        cout: usize,
        cin_g: usize,
        k: [usize; 2],
        bias: bool,
    ) -> Result<Folded, Error> {
        let w = self
            .w
            .shaped(&format!("{conv}.weight"), &[cout, cin_g, k[0], k[1]])?;
        let b = if bias {
            self.w.shaped(&format!("{conv}.bias"), &[cout])?
        } else {
            Tensor::zeros(&[cout])
        };
        Ok(Folded { w, b })
    }

    /// `ConvLayer`: conv (no bias) + BN + act, symmetric padding `pad`.
    #[allow(clippy::too_many_arguments)]
    fn conv_bn(
        &mut self,
        x: Value,
        prefix: &str,
        cout: usize,
        k: [usize; 2],
        stride: [usize; 2],
        pad: [usize; 2],
        groups: usize,
        act: Act,
    ) -> Result<Value, Error> {
        let cin = self.g.shape(x)[1];
        let f = self.conv_bn_weights(
            &format!("{prefix}.convolution"),
            &format!("{prefix}.normalization"),
            cout,
            cin / groups.max(1),
            k,
            false,
        )?;
        self.conv(
            x,
            f,
            Conv2d {
                stride,
                pad: [pad[0], pad[1], pad[0], pad[1]],
                groups,
                act,
                ..Default::default()
            },
        )
    }

    fn unary(&mut self, x: Value, act: Act, mul: f32, add: f32) -> Result<Value, Error> {
        self.g.unary(x, act, mul, add)
    }

    /// Squeeze-excitation: global mean → 1×1 conv + ReLU → 1×1 conv → gate → x·gate.
    /// `gate` maps the second conv's output (`HardSigmoid` with `mul` 1 is torch's
    /// x/6 + ½; `mul` 1.2 gives Paddle's clamp(0.2x + ½)).
    fn se(
        &mut self,
        x: Value,
        c1: &str,
        c2: &str,
        reduction: usize,
        gate_mul: f32,
    ) -> Result<Value, Error> {
        let c = self.g.shape(x)[1];
        let r = c / reduction.max(1);
        let m = self.g.global_avg_pool(x)?;
        let f1 = self.plain_weights(c1, r, c, [1, 1], true)?;
        let h = self.conv(m, f1, Conv2d::default().act(Act::Relu))?;
        let f2 = self.plain_weights(c2, c, r, [1, 1], true)?;
        let s = if gate_mul == 1.0 {
            self.conv(h, f2, Conv2d::default().act(Act::HardSigmoid))?
        } else {
            let s = self.conv(h, f2, Conv2d::default())?;
            self.unary(s, Act::HardSigmoid, gate_mul, 0.0)?
        };
        self.g.mul(x, s)
    }

    // ---- PP-LCNetV4 ----

    fn v4_stem(&mut self, x: Value, bb: &Backbone) -> Result<Value, Error> {
        let Backbone::V4 {
            large_stem,
            stem,
            stem_strides: s,
            act,
            ..
        } = bb
        else {
            return bad("not a V4 backbone");
        };
        let p = "model.backbone.encoder.convolution";
        if *large_stem {
            let e1 = self.conv_bn(
                x,
                &format!("{p}.stem1"),
                stem[1],
                [3, 3],
                [s[0]; 2],
                [1, 1],
                1,
                *act,
            )?;
            // stem2a / stem2b: 2×2 convs on the input padded by one on the right and bottom.
            let pad_rb = |stride: usize| Conv2d {
                stride: [stride; 2],
                pad: [0, 0, 1, 1],
                act: *act,
                ..Default::default()
            };
            let f = self.conv_bn_weights(
                &format!("{p}.stem2a.convolution"),
                &format!("{p}.stem2a.normalization"),
                stem[1] / 2,
                stem[1],
                [2, 2],
                false,
            )?;
            let a = self.conv(e1, f, pad_rb(s[1]))?;
            let f = self.conv_bn_weights(
                &format!("{p}.stem2b.convolution"),
                &format!("{p}.stem2b.normalization"),
                stem[1],
                stem[1] / 2,
                [2, 2],
                false,
            )?;
            let b = self.conv(a, f, pad_rb(s[2]))?;
            // The pool reads zero padding; its input is ≥ 0 after ReLU, so ignoring the
            // padded taps gives the same maximum.
            if *act != Act::Relu && *act != Act::Relu6 {
                return bad("PP-LCNetV4 large stem without ReLU");
            }
            let pooled = self.g.pool2d(
                e1,
                Pool2d {
                    kind: PoolKind::Max,
                    kernel: [2, 2],
                    stride: [1, 1],
                    pad: [0, 0, 1, 1],
                },
            )?;
            let cat = self.g.concat(&[pooled, b], 1)?;
            let y = self.conv_bn(
                cat,
                &format!("{p}.stem3"),
                stem[1],
                [3, 3],
                [s[3]; 2],
                [1, 1],
                1,
                *act,
            )?;
            self.conv_bn(
                y,
                &format!("{p}.stem4"),
                stem[2],
                [1, 1],
                [s[4]; 2],
                [0, 0],
                1,
                *act,
            )
        } else {
            let y = self.conv_bn(
                x,
                &format!("{p}.conv1"),
                stem[1],
                [3, 3],
                [2, 2],
                [1, 1],
                1,
                Act::Identity,
            )?;
            let y = self.g.act(y, Act::Gelu)?;
            self.conv_bn(
                y,
                &format!("{p}.conv2"),
                stem[2],
                [3, 3],
                [2, 2],
                [1, 1],
                1,
                Act::Identity,
            )
        }
    }

    fn v4_block(
        &mut self,
        x: Value,
        pre: &str,
        b: &BlockSpec,
        reduction: usize,
    ) -> Result<Value, Error> {
        let same = b.cin == b.cout && b.stride.is(1);
        let k = b.kernel;
        let mut h = if same {
            let f = self.plain_weights(&format!("{pre}.token_conv"), b.cin, 1, [k, k], true)?;
            self.conv(x, f, Conv2d::same(k, 1).groups(b.cin))?
        } else {
            let pad = (k - 1) / 2;
            self.conv_bn(
                x,
                &format!("{pre}.token_conv"),
                b.cin,
                [k, k],
                b.stride.yx(),
                [pad, pad],
                b.cin,
                Act::Identity,
            )?
        };
        if b.se {
            h = self.se(
                h,
                &format!("{pre}.token_squeeze_excitation.convolutions.0"),
                &format!("{pre}.token_squeeze_excitation.convolutions.2"),
                reduction,
                1.0,
            )?;
        }
        let r = h;
        let y = self.conv_bn(
            h,
            &format!("{pre}.channel_conv1"),
            2 * b.cin,
            [1, 1],
            [1, 1],
            [0, 0],
            1,
            Act::Gelu,
        )?;
        let y = self.conv_bn(
            y,
            &format!("{pre}.channel_conv2"),
            b.cout,
            [1, 1],
            [1, 1],
            [0, 0],
            1,
            Act::Identity,
        )?;
        if same { self.g.add(r, y) } else { Ok(y) }
    }

    // ---- PP-LCNetV3 ----

    /// A learnable re-parameterisation layer folded into one conv: the k×k branches, the
    /// 1×1 branch (centred) and the identity batch norm summed, then the scalar affine.
    #[allow(clippy::too_many_arguments)]
    fn v3_rep(
        &mut self,
        x: Value,
        pre: &str,
        cout: usize,
        k: usize,
        stride: Stride,
        groups: usize,
        branches: usize,
        act: Act,
    ) -> Result<Value, Error> {
        let cin = self.g.shape(x)[1];
        let cg = cin / groups;
        let n = cout * cg * k * k;
        let mut wsum = vec![0f32; n];
        let mut bsum = vec![0f32; cout];
        let mut add = |f: Folded, kk: usize| {
            let w = f.w.to_f32();
            let off = (k - kk) / 2;
            for o in 0..cout {
                for c in 0..cg {
                    for y in 0..kk {
                        for xx in 0..kk {
                            wsum[((o * cg + c) * k + y + off) * k + xx + off] +=
                                w[((o * cg + c) * kk + y) * kk + xx];
                        }
                    }
                }
            }
            for (s, b) in bsum.iter_mut().zip(f.b.to_f32()) {
                *s += b;
            }
        };
        for i in 0..branches {
            let f = self.conv_bn_weights(
                &format!("{pre}.conv_symmetric.{i}.convolution"),
                &format!("{pre}.conv_symmetric.{i}.normalization"),
                cout,
                cg,
                [k, k],
                false,
            )?;
            add(f, k);
        }
        if k > 1 {
            let f = self.conv_bn_weights(
                &format!("{pre}.conv_small_symmetric.convolution"),
                &format!("{pre}.conv_small_symmetric.normalization"),
                cout,
                cg,
                [1, 1],
                false,
            )?;
            add(f, 1);
        }
        if cin == cout && stride.is(1) {
            let bn = self.w.bn(&format!("{pre}.identity"), cout)?;
            let (scale, shift) = bn.scale_shift();
            let c = k / 2;
            for o in 0..cout {
                let ci = if groups == 1 { o } else { 0 };
                wsum[((o * cg + ci) * k + c) * k + c] += scale[o];
                bsum[o] += shift[o];
            }
        }
        let (a, b0) = (
            self.w.scalar(&format!("{pre}.lab.scale"))?,
            self.w.scalar(&format!("{pre}.lab.bias"))?,
        );
        for w in &mut wsum {
            *w *= a;
        }
        for b in &mut bsum {
            *b = *b * a + b0;
        }
        let f = Folded {
            w: Tensor::f32(&[cout, cg, k, k], wsum),
            b: Tensor::f32(&[cout], bsum),
        };
        let p = k / 2;
        let y = self.conv(
            x,
            f,
            Conv2d {
                stride: stride.yx(),
                pad: [p, p, p, p],
                groups,
                act: if stride.is(2) { Act::Identity } else { act },
                ..Default::default()
            },
        )?;
        if stride.is(2) {
            return Ok(y);
        }
        let (a, b) = (
            self.w.scalar(&format!("{pre}.act.lab.scale"))?,
            self.w.scalar(&format!("{pre}.act.lab.bias"))?,
        );
        if a == 1.0 && b == 0.0 {
            Ok(y)
        } else {
            self.unary(y, Act::Identity, a, b)
        }
    }

    fn backbone(&mut self, x: Value, bb: &Backbone) -> Result<Vec<Value>, Error> {
        let mut outs = Vec::new();
        match bb {
            Backbone::V4 {
                stages, reduction, ..
            } => {
                let mut h = self.v4_stem(x, bb)?;
                for (si, st) in stages.iter().enumerate() {
                    for (bi, b) in st.iter().enumerate() {
                        let pre = format!("model.backbone.encoder.blocks.{si}.blocks.{bi}");
                        h = self.v4_block(h, &pre, b, *reduction)?;
                    }
                    outs.push(h);
                }
            }
            Backbone::V1 {
                stem,
                stem_stride,
                act,
                reduction,
                stages,
            } => {
                let mut h = self.conv_bn(
                    x,
                    "encoder.convolution",
                    *stem,
                    [3, 3],
                    [*stem_stride; 2],
                    [1, 1],
                    1,
                    *act,
                )?;
                for (si, st) in stages.iter().enumerate() {
                    for (bi, b) in st.iter().enumerate() {
                        let pre = format!("encoder.blocks.{si}.layers.{bi}");
                        let k = b.kernel;
                        h = self.conv_bn(
                            h,
                            &format!("{pre}.depthwise_convolution"),
                            b.cin,
                            [k, k],
                            b.stride.yx(),
                            [k / 2, k / 2],
                            b.cin,
                            *act,
                        )?;
                        if b.se {
                            h = self.se(
                                h,
                                &format!("{pre}.squeeze_excitation_module.convolutions.0"),
                                &format!("{pre}.squeeze_excitation_module.convolutions.2"),
                                *reduction,
                                1.0,
                            )?;
                        }
                        h = self.conv_bn(
                            h,
                            &format!("{pre}.pointwise_convolution"),
                            b.cout,
                            [1, 1],
                            [1, 1],
                            [0, 0],
                            1,
                            *act,
                        )?;
                    }
                    outs.push(h);
                }
            }
            Backbone::V3 {
                stem,
                stem_stride,
                act,
                reduction,
                branches,
                stages,
            } => {
                let mut h = self.conv_bn(
                    x,
                    "model.backbone.encoder.convolution",
                    *stem,
                    [3, 3],
                    [*stem_stride; 2],
                    [1, 1],
                    1,
                    Act::Identity,
                )?;
                for (si, st) in stages.iter().enumerate() {
                    for (bi, b) in st.iter().enumerate() {
                        let pre = format!("model.backbone.encoder.blocks.{si}.layers.{bi}");
                        h = self.v3_rep(
                            h,
                            &format!("{pre}.depthwise_convolution"),
                            b.cin,
                            b.kernel,
                            b.stride,
                            b.cin,
                            *branches,
                            *act,
                        )?;
                        if b.se {
                            h = self.se(
                                h,
                                &format!("{pre}.squeeze_excitation_module.convolutions.0"),
                                &format!("{pre}.squeeze_excitation_module.convolutions.2"),
                                *reduction,
                                1.0,
                            )?;
                        }
                        h = self.v3_rep(
                            h,
                            &format!("{pre}.pointwise_convolution"),
                            b.cout,
                            1,
                            Stride::One(1),
                            1,
                            *branches,
                            *act,
                        )?;
                    }
                    outs.push(h);
                }
            }
        }
        Ok(outs)
    }

    // ---- DB detector ----

    /// A 2×2 stride-2 transposed conv (weights `[in, out, 2, 2]`) as a 1×1 conv to
    /// `4·out` channels and a pixel shuffle; an optional batch norm folded in.
    fn conv_transpose2(
        &mut self,
        x: Value,
        conv: &str,
        norm: Option<&str>,
        cout: usize,
        act: Act,
    ) -> Result<Value, Error> {
        let s = self.g.shape(x).to_vec();
        let cin = s[1];
        let w = self
            .w
            .shaped(&format!("{conv}.weight"), &[cin, cout, 2, 2])?
            .to_f32();
        let mut b = self.w.vec(&format!("{conv}.bias"), cout)?;
        let mut scale = vec![1f32; cout];
        if let Some(n) = norm {
            let bn = self.w.bn(n, cout)?;
            let (sc, sh) = bn.scale_shift();
            for o in 0..cout {
                b[o] = b[o] * sc[o] + sh[o];
            }
            scale = sc;
        }
        // Output channel (o, dy, dx) ← input channel c.
        let mut w2 = vec![0f32; cout * 4 * cin];
        let mut b2 = vec![0f32; cout * 4];
        for o in 0..cout {
            for d in 0..4 {
                b2[o * 4 + d] = b[o];
                for c in 0..cin {
                    w2[(o * 4 + d) * cin + c] = w[(c * cout + o) * 4 + d] * scale[o];
                }
            }
        }
        let f = Folded {
            w: Tensor::f32(&[cout * 4, cin, 1, 1], w2),
            b: Tensor::f32(&[cout * 4], b2),
        };
        let y = self.conv(x, f, Conv2d::default().act(act))?;
        let (n, h, wd) = (s[0], s[2], s[3]);
        let y = self.g.reshape(y, &[n, cout, 2, 2, h, wd])?;
        let y = self.g.transpose(y, &[0, 1, 4, 2, 5, 3])?;
        self.g.reshape(y, &[n, cout, 2 * h, 2 * wd])
    }

    fn up(&mut self, x: Value, factor: usize) -> Result<Value, Error> {
        if factor == 1 {
            return Ok(x);
        }
        let s = self.g.shape(x).to_vec();
        self.g
            .resize(x, s[2] * factor, s[3] * factor, ResizeMode::Nearest)
    }

    fn det_head(&mut self, feats: &[Value], head: &HeadSpec) -> Result<Value, Error> {
        let HeadSpec::Det {
            v5,
            layer_channels,
            neck,
            kernels,
            dw_kernel,
            reduction,
        } = head
        else {
            return bad("not a detector");
        };
        // PP-LCNetV3 has five stages; the detector reads the last four.
        let feats = &feats[feats.len().saturating_sub(4)..];
        if feats.len() != 4 {
            return bad("detector backbone with fewer than four stages");
        }
        let neck = *neck;
        let mut fused = Vec::new();
        for (i, (&f, &c)) in feats.iter().zip(layer_channels).enumerate() {
            let f = if *v5 {
                let cin = self.g.shape(f)[1];
                let wts = self.plain_weights(&format!("model.layer.{i}"), c, cin, [1, 1], true)?;
                self.conv(f, wts, Conv2d::default())?
            } else {
                f
            };
            if self.g.shape(f)[1] != c {
                return bad("detector level channels");
            }
            let p = format!("model.neck.insert_conv.{i}");
            let wts = self.plain_weights(&format!("{p}.in_conv"), neck, c, [1, 1], false)?;
            let y = self.conv(f, wts, Conv2d::default())?;
            let s = self.se(
                y,
                &format!("{p}.squeeze_excitation_block.conv1"),
                &format!("{p}.squeeze_excitation_block.conv2"),
                *reduction,
                1.2,
            )?;
            fused.push(self.g.add(y, s)?);
        }
        for i in (0..3).rev() {
            let u = self.up(fused[i + 1], 2)?;
            fused[i] = self.g.add(fused[i], u)?;
        }
        let mut levels = Vec::new();
        for (i, &f) in fused.iter().enumerate() {
            let p = format!("model.neck.input_conv.{i}");
            if *v5 {
                let wts =
                    self.plain_weights(&format!("{p}.in_conv"), neck / 4, neck, [3, 3], false)?;
                let y = self.conv(f, wts, Conv2d::same(3, 1))?;
                let s = self.se(
                    y,
                    &format!("{p}.squeeze_excitation_block.conv1"),
                    &format!("{p}.squeeze_excitation_block.conv2"),
                    *reduction,
                    1.2,
                )?;
                let y = self.g.add(y, s)?;
                levels.push(self.up(y, 1 << i)?);
                continue;
            }
            let k = *dw_kernel;
            let wts =
                self.plain_weights(&format!("{p}.depthwise_convolution"), neck, 1, [k, k], true)?;
            let y = self.conv(f, wts, Conv2d::same(k, 1).groups(neck))?;
            let wts = self.plain_weights(
                &format!("{p}.pointwise_convolution"),
                neck / 4,
                neck,
                [1, 1],
                false,
            )?;
            let y = self.conv(y, wts, Conv2d::default())?;
            let s = self.se(
                y,
                &format!("{p}.squeeze_excitation_module.conv1"),
                &format!("{p}.squeeze_excitation_module.conv2"),
                *reduction,
                1.2,
            )?;
            let y = self.g.add(y, s)?;
            levels.push(self.up(y, 1 << i)?);
        }
        levels.reverse();
        let cat = self.g.concat(&levels, 1)?;
        let q = neck / 4;
        let k = kernels[0];
        let f = self.conv_bn_weights(
            "head.conv_down.convolution",
            "head.conv_down.norm",
            q,
            neck,
            [k, k],
            false,
        )?;
        let y = self.conv(cat, f, Conv2d::same(k, 1).act(Act::Relu))?;
        let y = self.conv_transpose2(
            y,
            "head.conv_up.convolution",
            Some("head.conv_up.norm"),
            q,
            Act::Relu,
        )?;
        self.conv_transpose2(y, "head.conv_final", None, 1, Act::Sigmoid)
    }

    // ---- recognisers ----

    fn linear(
        &mut self,
        x: Value,
        name: &str,
        out: usize,
        inp: usize,
        act: Act,
    ) -> Result<Value, Error> {
        let w = self.w.shaped(&format!("{name}.weight"), &[out, inp])?;
        let b = self.w.shaped(&format!("{name}.bias"), &[out])?;
        let w = self.konst(w)?;
        let b = self.g.constant(b)?;
        self.g
            .matmul(x, w, Some(b), crate::graph::MatMul { trans_b: true, act })
    }

    fn layer_norm(&mut self, x: Value, name: &str, d: usize, eps: f32) -> Result<Value, Error> {
        let g = self.w.shaped(&format!("{name}.weight"), &[d])?;
        let b = self.w.shaped(&format!("{name}.bias"), &[d])?;
        let g = self.g.constant(g)?;
        let b = self.g.constant(b)?;
        self.g.layer_norm(x, g, b, eps)
    }

    /// `[N, C, 1, T]` → `[N, T, C]`.
    fn tokens(&mut self, x: Value) -> Result<Value, Error> {
        let s = self.g.shape(x).to_vec();
        let y = self.g.reshape(x, &[s[0], s[1], s[2] * s[3]])?;
        self.g.transpose(y, &[0, 2, 1])
    }

    /// `[N, T, C]` → `[N, C, 1, T]`.
    fn untokens(&mut self, x: Value) -> Result<Value, Error> {
        let s = self.g.shape(x).to_vec();
        let y = self.g.transpose(x, &[0, 2, 1])?;
        self.g.reshape(y, &[s[0], s[2], 1, s[1]])
    }

    #[allow(clippy::too_many_arguments)]
    fn svtr_blocks(
        &mut self,
        mut t: Value,
        hidden: usize,
        depth: usize,
        heads: usize,
        mlp: usize,
        act: Act,
        eps: f32,
    ) -> Result<Value, Error> {
        for i in 0..depth {
            let p = format!("head.encoder.svtr_block.{i}");
            let h = self.layer_norm(t, &format!("{p}.layer_norm1"), hidden, eps)?;
            let qkv = self.linear(
                h,
                &format!("{p}.self_attn.qkv"),
                3 * hidden,
                hidden,
                Act::Identity,
            )?;
            let a = self.g.attention_packed(qkv, heads, None, false)?;
            let a = self.linear(
                a,
                &format!("{p}.self_attn.projection"),
                hidden,
                hidden,
                Act::Identity,
            )?;
            t = self.g.add(t, a)?;
            let h = self.layer_norm(t, &format!("{p}.layer_norm2"), hidden, eps)?;
            let h = self.linear(h, &format!("{p}.mlp.fc1"), mlp, hidden, act)?;
            let h = self.linear(h, &format!("{p}.mlp.fc2"), hidden, mlp, Act::Identity)?;
            t = self.g.add(t, h)?;
        }
        self.layer_norm(t, "head.encoder.norm", hidden, eps)
    }

    fn rec_head(&mut self, x: Value, head: &HeadSpec) -> Result<Value, Error> {
        let c = self.g.shape(x)[1];
        let logits = match *head {
            HeadSpec::Svtr {
                v6,
                hidden,
                depth,
                heads,
                mlp,
                kernel,
                act,
                eps,
                classes,
            } => {
                let pad = [kernel[0] / 2, kernel[1] / 2];
                let cb = |i: usize| format!("head.encoder.conv_block.{i}");
                if v6 {
                    let r = self.conv_bn(x, &cb(0), hidden, [1, 1], [1, 1], [0, 0], 1, act)?;
                    let h = self.conv_bn(x, &cb(1), hidden, [1, 1], [1, 1], [0, 0], 1, act)?;
                    let d = self.conv_bn(h, &cb(2), hidden, kernel, [1, 1], pad, hidden, act)?;
                    let h = self.g.add(h, d)?;
                    let t = self.tokens(h)?;
                    let t = self.svtr_blocks(t, hidden, depth, heads, mlp, act, eps)?;
                    let rt = self.tokens(r)?;
                    let t = self.g.add(t, rt)?;
                    self.linear(t, "head.head", classes, hidden, Act::Identity)?
                } else {
                    let h = self.conv_bn(x, &cb(0), c / 8, kernel, [1, 1], pad, 1, act)?;
                    let h = self.conv_bn(h, &cb(1), hidden, [1, 1], [1, 1], [0, 0], 1, act)?;
                    let t = self.tokens(h)?;
                    let t = self.svtr_blocks(t, hidden, depth, heads, mlp, act, eps)?;
                    let h = self.untokens(t)?;
                    let h = self.conv_bn(h, &cb(2), c, [1, 1], [1, 1], [0, 0], 1, act)?;
                    let cat = self.g.concat(&[x, h], 1)?;
                    let h = self.conv_bn(cat, &cb(3), c / 8, kernel, [1, 1], pad, 1, act)?;
                    let h = self.conv_bn(h, &cb(4), hidden, [1, 1], [1, 1], [0, 0], 1, act)?;
                    let t = self.tokens(h)?;
                    self.linear(t, "head.head", classes, hidden, Act::Identity)?
                }
            }
            HeadSpec::Tiny { hidden, classes } => {
                let one_d = |net: &mut Net,
                             conv: &str,
                             norm: &str,
                             cg: usize,
                             k: usize|
                 -> Result<Folded, Error> {
                    let w = net.w.shaped(&format!("{conv}.weight"), &[c, cg, k])?;
                    let w = w.reshaped(&[c, cg, 1, k]);
                    let bn = net.w.bn(norm, c)?;
                    let (w, b) = fold_batchnorm(&w, None, &bn)?;
                    Ok(Folded { w, b })
                };
                let f = one_d(self, "head.conv1", "head.norm1", 1, 5)?;
                let h = self.conv(
                    x,
                    f,
                    Conv2d {
                        pad: [0, 2, 0, 2],
                        groups: c,
                        act: Act::HardSwish,
                        ..Default::default()
                    },
                )?;
                let f = one_d(self, "head.conv2", "head.norm2", c, 1)?;
                let h = self.conv(h, f, Conv2d::default().act(Act::HardSwish))?;
                let t = self.tokens(h)?;
                let t = self.linear(t, "head.fc1", hidden, c, Act::Identity)?;
                self.linear(t, "head.fc2", classes, hidden, Act::Identity)?
            }
            HeadSpec::Det { .. } | HeadSpec::Cls { .. } => return bad("not a recogniser"),
        };
        self.g.softmax(logits)
    }
}

/// ImageNet mean and std in RGB order (the detector's normalisation, read as BGR by Paddle;
/// the graph takes RGB and its first conv's channels are swapped).
pub const DET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
pub const DET_STD: [f32; 3] = [0.229, 0.224, 0.225];

/// What a graph takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// An RGBA8 image of any size, stretched to `height × width` (bilinear, half-pixel
    /// centres, no antialiasing: OpenCV's `INTER_LINEAR`) and normalised.
    Image { height: usize, width: usize },
    /// An f32 `[N, 3, H, W]` tensor, RGB, already normalised.
    Tensor {
        batch: usize,
        height: usize,
        width: usize,
    },
}

/// Builds the network `arch` for `input`, with weights from `bank`. Detector output:
/// `[N, 1, H, W]` probabilities; recogniser output: `[N, W/8, classes]` probabilities;
/// classifier output: `[N, classes]` probabilities.
pub fn build(arch: &Arch, bank: &Bank, input: Input, f16: bool) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let x = match input {
        Input::Image { height, width } => {
            if !arch.is_detector() {
                return bad("an image input is for the detector");
            }
            if height % 32 != 0 || width % 32 != 0 || height == 0 || width == 0 {
                return bad("detector input sides must be multiples of 32");
            }
            let img = g.image_input()?;
            g.preprocess(
                img,
                Preprocess {
                    width,
                    height,
                    crop: crate::graph::Crop::Stretch,
                    antialias: false,
                    mean: DET_MEAN,
                    std: DET_STD,
                },
            )?
        }
        Input::Tensor {
            batch,
            height,
            width,
        } => g.input(&[batch, 3, height, width], DType::F32)?,
    };
    let mut net = Net {
        g: &mut g,
        w: bank,
        f16,
        bgr: true,
    };
    let feats = net.backbone(x, &arch.backbone)?;
    let out = if arch.is_detector() {
        net.det_head(&feats, &arch.head)?
    } else if let HeadSpec::Cls {
        expand,
        classes,
        keep,
    } = arch.head
    {
        let Some(&last) = feats.last() else {
            return bad("empty backbone");
        };
        let c = net.g.shape(last)[1];
        let n = net.g.shape(last)[0];
        let m = net.g.global_avg_pool(last)?;
        let f = net.plain_weights("last_convolution", expand, c, [1, 1], false)?;
        let h = net.conv(m, f, Conv2d::default().act(Act::HardSwish))?;
        let h = net.g.reshape(h, &[n, expand])?;
        let w = net.w.shaped("head.weight", &[classes, expand])?;
        let w = Tensor::f32(&w.shape, w.to_f32().iter().map(|v| v * keep).collect());
        let b = net.w.shaped("head.bias", &[classes])?;
        let w = net.konst(w)?;
        let b = net.konst(b)?;
        let y = net.g.linear(h, w, Some(b))?;
        net.g.softmax(y)?
    } else {
        let Some(&last) = feats.last() else {
            return bad("empty backbone");
        };
        let s = net.g.shape(last).to_vec();
        if s[2] != 3 || s[3] < 2 || s[1] != arch.backbone.out_channels() {
            return bad(format!(
                "recogniser features {s:?}: the input must be {REC_HEIGHT} high and at least 16 wide"
            ));
        }
        let pooled = net.g.pool2d(
            last,
            Pool2d {
                kind: PoolKind::Avg,
                kernel: [3, 2],
                stride: [3, 2],
                pad: [0; 4],
            },
        )?;
        net.rec_head(pooled, &arch.head)?
    };
    g.output(out)?;
    Ok(g)
}

/// A recogniser input value from an 8-bit channel: `(v/255 − ½)/½`.
pub fn rec_normalise(v: u8) -> f32 {
    (f32::from(v) / 255.0 - 0.5) / 0.5
}
