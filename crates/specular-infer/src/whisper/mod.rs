//! OpenAI's Whisper speech recogniser (generated subtitles), from the Hugging Face
//! safetensors checkpoints (`openai/whisper-*`: names `model.encoder.*`, `model.decoder.*`).
//!
//! - [`mel`]: the log-mel spectrogram of 16 kHz mono sound.
//! - The **encoder** is a [`Graph`]: two 1-D convolutions (as 2-D ones of height 1, the
//!   second with stride 2: 3,000 frames to 1,500 positions), GELU, sinusoidal positions,
//!   pre-norm blocks (fused QKV, the key without bias), a final LayerNorm. The graph's
//!   outputs are each decoder layer's cross-attention keys and values (`[1, 1500, 2D]`),
//!   computed once per 30 s window. It runs on the GPU when there is one, else on the CPU
//!   reference.
//! - The **decoder** runs token by token on the CPU with a key/value cache: per token it
//!   is a few matrix–vector products (f16 weights read in place), small next to the encoder.
//!   Its token embedding doubles as the output projection.
//! - [`tokens`]: the vocabulary (decoding only) and the special tokens.
//! - [`decode`]: language detection, timestamped greedy decoding with Whisper's rules and
//!   temperature fallback, segments.
//! - [`vad`]: which stretches of sound hold speech (energy against the noise floor), and
//!   the 30 s windows that cover them.
//! - [`cues`]: segments into subtitle cues of readable length, and SRT/WebVTT text.
//!
//! Heads are 64 wide in every Whisper; LayerNorm eps 1e-5; exact GELU.

pub mod cues;
pub mod decode;
pub mod mel;
pub mod tokens;
pub mod vad;

use std::path::Path;

use crate::cpu;
use crate::gpu::{Context, GpuGraph};
use crate::graph::{Act, Conv2d, Graph, MatMul, Value};
use crate::models::Weights;
use crate::safetensors::{Header, SafeTensorsFile};
use crate::tensor::Tensor;
use crate::{DType, Error, Feed};

pub use tokens::{Specials, Vocab};

/// LayerNorm epsilon.
pub const EPS: f32 = 1e-5;
/// Width of an attention head.
pub const HEAD: usize = 64;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

/// The sizes of a Whisper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub n_mels: usize,
    /// Model width.
    pub width: usize,
    pub heads: usize,
    pub encoder_layers: usize,
    pub decoder_layers: usize,
    /// Encoder positions (1,500: 30 s).
    pub audio_ctx: usize,
    /// Decoder positions (448).
    pub text_ctx: usize,
    pub vocab: usize,
    /// MLP width.
    pub mlp: usize,
}

impl Config {
    /// Mel frames of one window (twice the encoder's positions).
    pub fn frames(&self) -> usize {
        self.audio_ctx * 2
    }

    /// The sizes `h` describes.
    pub fn detect(h: &Header) -> Result<Config, Error> {
        let shape = |n: &str| -> Result<Vec<usize>, Error> {
            h.get(n)
                .map(|t| t.shape.clone())
                .ok_or_else(|| Error::Weights(format!("not a Whisper checkpoint: no {n:?}")))
        };
        let conv1 = shape("model.encoder.conv1.weight")?;
        let pos = shape("model.encoder.embed_positions.weight")?;
        let emb = shape("model.decoder.embed_tokens.weight")?;
        let tpos = shape("model.decoder.embed_positions.weight")?;
        let fc1 = shape("model.encoder.layers.0.fc1.weight")?;
        if conv1.len() != 3 || pos.len() != 2 || emb.len() != 2 || tpos.len() != 2 {
            return bad("unexpected Whisper tensor ranks");
        }
        let count = |p: &str| {
            (0..)
                .take_while(|i| h.get(&format!("{p}.layers.{i}.fc1.weight")).is_some())
                .count()
        };
        let width = conv1[0];
        let c = Config {
            n_mels: conv1[1],
            width,
            heads: width / HEAD,
            encoder_layers: count("model.encoder"),
            decoder_layers: count("model.decoder"),
            audio_ctx: pos[0],
            text_ctx: tpos[0],
            vocab: emb[0],
            mlp: fc1[0],
        };
        if width % HEAD != 0
            || c.heads == 0
            || pos[1] != width
            || emb[1] != width
            || c.encoder_layers == 0
            || c.decoder_layers == 0
            || !(c.n_mels == 80 || c.n_mels == 128)
            || Specials::of(c.vocab).is_none()
        {
            return bad(format!("not a Whisper this runtime reads: {c:?}"));
        }
        Ok(c)
    }

    /// A small Whisper for tests: the real vocabulary, 1 s windows.
    pub fn test() -> Config {
        Config {
            n_mels: 80,
            width: 128,
            heads: 2,
            encoder_layers: 2,
            decoder_layers: 2,
            audio_ctx: 50,
            text_ctx: 64,
            vocab: 51_865,
            mlp: 256,
        }
    }

    /// Multiply-adds of one window through the encoder (cross keys and values included).
    pub fn encoder_macs(&self) -> u64 {
        let (d, t, f) = (
            self.width as u64,
            self.audio_ctx as u64,
            self.frames() as u64,
        );
        let convs = f * d * self.n_mels as u64 * 3 + t * d * d * 3;
        let block = t * (4 * d * d + 2 * d * self.mlp as u64) + 2 * t * t * d;
        convs + self.encoder_layers as u64 * block + self.decoder_layers as u64 * 2 * t * d * d
    }

    /// Multiply-adds of one decoded token (at the start of a window).
    pub fn token_macs(&self) -> u64 {
        let (d, t) = (self.width as u64, self.audio_ctx as u64);
        let layer = 6 * d * d + 2 * d * self.mlp as u64 + 2 * t * d;
        self.decoder_layers as u64 * layer + self.vocab as u64 * d
    }
}

/// One decoder layer's weights.
struct Layer {
    ln1: (Vec<f32>, Vec<f32>),
    /// Fused `[3D, D]` query, key, value; the key's bias zero.
    qkv: Tensor,
    qkv_b: Tensor,
    out: Tensor,
    out_b: Tensor,
    ln2: (Vec<f32>, Vec<f32>),
    cq: Tensor,
    cq_b: Tensor,
    cout: Tensor,
    cout_b: Tensor,
    ln3: (Vec<f32>, Vec<f32>),
    fc1: Tensor,
    fc1_b: Tensor,
    fc2: Tensor,
    fc2_b: Tensor,
}

/// Where the encoder runs.
enum Encoder {
    Cpu(Graph),
    /// The chain of [`encoder_graphs`], one submission each.
    Gpu(Vec<GpuGraph>),
}

/// Multiply-adds per GPU submission at most (a few tens of milliseconds on a modest GPU).
pub const GPU_MACS: u64 = 40_000_000_000;

/// A loaded Whisper.
pub struct Whisper {
    pub config: Config,
    pub specials: Specials,
    pub vocab: Vocab,
    mel: mel::Mel,
    encoder: Encoder,
    /// `[V, D]`, the embedding and the output projection.
    embed: Tensor,
    /// `[text_ctx, D]`.
    pos: Vec<f32>,
    layers: Vec<Layer>,
    ln_f: (Vec<f32>, Vec<f32>),
    /// CPU threads for the decoder (and a CPU encoder).
    pub threads: usize,
    /// Tokens never emitted (non-speech symbols).
    pub(crate) suppress: Vec<u32>,
    stats: std::cell::Cell<Stats>,
}

/// Where the time went, since the model was loaded.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub windows: u32,
    /// Decoding passes (more than windows when the temperature fallback ran).
    pub passes: u32,
    pub encode: std::time::Duration,
    pub tokens: u64,
    pub decode: std::time::Duration,
}

/// The encoder's result for one window: per decoder layer, keys and values `[T, D]`.
pub struct Audio {
    pub(crate) kv: Vec<(Vec<f32>, Vec<f32>)>,
}

/// Per-sequence decoder state: the self-attention cache.
pub struct State {
    len: usize,
    kv: Vec<(Vec<f32>, Vec<f32>)>,
}

impl State {
    /// Tokens fed so far.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

fn vecf(w: &mut dyn Weights, name: &str, d: usize) -> Result<Vec<f32>, Error> {
    Ok(w.get(name, &[d])?.to_f32())
}

fn lnw(w: &mut dyn Weights, p: &str, d: usize) -> Result<(Vec<f32>, Vec<f32>), Error> {
    Ok((
        vecf(w, &format!("{p}.weight"), d)?,
        vecf(w, &format!("{p}.bias"), d)?,
    ))
}

fn mat(t: Tensor, f16: bool) -> Tensor {
    if f16 { t.to_f16() } else { t }
}

/// The fused query/key/value weight and bias of an attention at `p` (`k_proj` has no bias).
fn qkv(w: &mut dyn Weights, p: &str, d: usize) -> Result<(Tensor, Tensor), Error> {
    let mut wv = Vec::with_capacity(3 * d * d);
    let mut bv = Vec::with_capacity(3 * d);
    for s in ["q", "k", "v"] {
        wv.extend(w.get(&format!("{p}.{s}_proj.weight"), &[d, d])?.to_f32());
        if s == "k" {
            bv.extend(std::iter::repeat_n(0.0, d));
        } else {
            bv.extend(vecf(w, &format!("{p}.{s}_proj.bias"), d)?);
        }
    }
    Ok((Tensor::f32(&[3 * d, d], wv), Tensor::f32(&[3 * d], bv)))
}

/// Builds the encoder graph: input mel `[1, n_mels, frames]`, one output per decoder layer,
/// `[1, audio_ctx, 2D]` (keys then values). Attention whose scores would pass `max_scores`
/// bytes is split into groups of heads.
pub fn encoder_graph(
    c: &Config,
    w: &mut dyn Weights,
    f16: bool,
    max_scores: u64,
) -> Result<Graph, Error> {
    let mut gs = encoder_graphs(c, w, f16, max_scores, u64::MAX)?;
    match gs.len() {
        1 => Ok(gs.remove(0)),
        _ => Err(Error::Shape("encoder split".into())),
    }
}

/// Multiply-adds of one encoder block over a window.
fn block_macs(c: &Config) -> u64 {
    let (d, t) = (c.width as u64, c.audio_ctx as u64);
    t * (4 * d * d + 2 * d * c.mlp as u64) + 2 * t * t * d
}

/// The encoder as a chain of graphs of at most `max_macs` multiply-adds each (at least one
/// block): the first takes the mel, each passes the hidden states `[1, audio_ctx, D]` to the
/// next, the last gives the cross-attention keys and values ([`encoder_graph`]'s outputs).
/// Each runs as its own GPU submission, so a large model on a slow GPU never holds the
/// device long enough for the driver to reset it.
pub fn encoder_graphs(
    c: &Config,
    w: &mut dyn Weights,
    f16: bool,
    max_scores: u64,
    max_macs: u64,
) -> Result<Vec<Graph>, Error> {
    let per = (max_macs / block_macs(c).max(1)).max(1) as usize;
    let mut out = Vec::new();
    let mut from = 0;
    loop {
        let to = (from + per).min(c.encoder_layers);
        out.push(encoder_part(c, w, f16, max_scores, from, to)?);
        if to >= c.encoder_layers {
            return Ok(out);
        }
        from = to;
    }
}

/// Blocks `from..to` of the encoder (with the convolutions when `from` is 0, the final norm
/// and the keys and values when `to` is the last).
fn encoder_part(
    c: &Config,
    w: &mut dyn Weights,
    f16: bool,
    max_scores: u64,
    from: usize,
    to: usize,
) -> Result<Graph, Error> {
    let (d, t, f) = (c.width, c.audio_ctx, c.frames());
    let mut g = Graph::new();
    if from > 0 {
        let x = g.input(&[1, t, d], DType::F32)?;
        return encoder_rest(c, w, f16, max_scores, from, to, g, x);
    }
    let x = g.input(&[1, c.n_mels, f], DType::F32)?;
    let x = g.reshape(x, &[1, c.n_mels, 1, f])?;
    let conv = |g: &mut Graph,
                w: &mut dyn Weights,
                x: Value,
                n: &str,
                cin: usize,
                stride: usize|
     -> Result<Value, Error> {
        let k = w.get(&format!("model.encoder.{n}.weight"), &[d, cin, 3])?;
        let k = g.constant(mat(k.reshaped(&[d, cin, 1, 3]), f16))?;
        let b = g.constant(Tensor::f32(
            &[d],
            vecf(w, &format!("model.encoder.{n}.bias"), d)?,
        ))?;
        g.conv2d(
            x,
            k,
            Some(b),
            Conv2d {
                stride: [1, stride],
                pad: [0, 1, 0, 1],
                act: Act::Gelu,
                ..Default::default()
            },
        )
    };
    let x = conv(&mut g, w, x, "conv1", c.n_mels, 1)?;
    let x = conv(&mut g, w, x, "conv2", d, 2)?;
    let x = g.reshape(x, &[1, d, t])?;
    let x = g.transpose(x, &[0, 2, 1])?;
    let pos = w.get("model.encoder.embed_positions.weight", &[t, d])?;
    let pos = g.constant(Tensor::f32(&[t, d], pos.to_f32()))?;
    let x = g.add(x, pos)?;
    encoder_rest(c, w, f16, max_scores, from, to, g, x)
}

/// The blocks `from..to` on `x` in `g`, and the end when `to` is the last.
#[allow(clippy::too_many_arguments)]
fn encoder_rest(
    c: &Config,
    w: &mut dyn Weights,
    f16: bool,
    max_scores: u64,
    from: usize,
    to: usize,
    mut g: Graph,
    mut x: Value,
) -> Result<Graph, Error> {
    let (d, t) = (c.width, c.audio_ctx);
    let lin = |g: &mut Graph,
               w: &mut dyn Weights,
               x: Value,
               p: &str,
               (o, i): (usize, usize),
               act: Act|
     -> Result<Value, Error> {
        let wt = g.constant(mat(w.get(&format!("{p}.weight"), &[o, i])?, f16))?;
        let b = g.constant(Tensor::f32(&[o], vecf(w, &format!("{p}.bias"), o)?))?;
        g.matmul(x, wt, Some(b), MatMul { trans_b: true, act })
    };
    let ln = |g: &mut Graph, w: &mut dyn Weights, x: Value, p: &str| -> Result<Value, Error> {
        let (a, b) = lnw(w, p, d)?;
        let a = g.constant(Tensor::f32(&[d], a))?;
        let b = g.constant(Tensor::f32(&[d], b))?;
        g.layer_norm(x, a, b, EPS)
    };
    // Heads per attention so the scores [H, T, T] stay under the limit.
    let per_head = (t * t * 4) as u64;
    let group = ((max_scores / per_head.max(1)) as usize).clamp(1, c.heads);
    for i in from..to {
        let p = format!("model.encoder.layers.{i}");
        let y = ln(&mut g, w, x, &format!("{p}.self_attn_layer_norm"))?;
        let (wq, bq) = qkv(w, &format!("{p}.self_attn"), d)?;
        let wq = g.constant(mat(wq, f16))?;
        let bq = g.constant(bq)?;
        let qkv = g.matmul(
            y,
            wq,
            Some(bq),
            MatMul {
                trans_b: true,
                act: Act::Identity,
            },
        )?;
        let a = if group >= c.heads {
            g.attention_packed(qkv, c.heads, None, false)?
        } else {
            let mut parts = Vec::new();
            let mut h0 = 0;
            while h0 < c.heads {
                let h1 = (h0 + group).min(c.heads);
                let (a0, a1) = (h0 * HEAD, h1 * HEAD);
                let q = g.slice_axis(qkv, 2, a0, a1)?;
                let k = g.slice_axis(qkv, 2, d + a0, d + a1)?;
                let v = g.slice_axis(qkv, 2, 2 * d + a0, 2 * d + a1)?;
                parts.push(g.attention(q, k, v, h1 - h0, None, false)?);
                h0 = h1;
            }
            g.concat(&parts, 2)?
        };
        let o = lin(
            &mut g,
            w,
            a,
            &format!("{p}.self_attn.out_proj"),
            (d, d),
            Act::Identity,
        )?;
        x = g.add(x, o)?;
        let y = ln(&mut g, w, x, &format!("{p}.final_layer_norm"))?;
        let m = lin(&mut g, w, y, &format!("{p}.fc1"), (c.mlp, d), Act::Gelu)?;
        let m = lin(&mut g, w, m, &format!("{p}.fc2"), (d, c.mlp), Act::Identity)?;
        x = g.add(x, m)?;
    }
    if to < c.encoder_layers {
        g.output(x)?;
        return Ok(g);
    }
    let x = ln(&mut g, w, x, "model.encoder.layer_norm")?;
    for i in 0..c.decoder_layers {
        let p = format!("model.decoder.layers.{i}.encoder_attn");
        let mut wv = w.get(&format!("{p}.k_proj.weight"), &[d, d])?.to_f32();
        wv.extend(w.get(&format!("{p}.v_proj.weight"), &[d, d])?.to_f32());
        let mut bv = vec![0f32; d];
        bv.extend(vecf(w, &format!("{p}.v_proj.bias"), d)?);
        let wt = g.constant(mat(Tensor::f32(&[2 * d, d], wv), f16))?;
        let b = g.constant(Tensor::f32(&[2 * d], bv))?;
        let kv = g.matmul(
            x,
            wt,
            Some(b),
            MatMul {
                trans_b: true,
                act: Act::Identity,
            },
        )?;
        g.output(kv)?;
    }
    Ok(g)
}

impl Whisper {
    /// Builds a Whisper from `w` (sizes `c`) and its vocabulary. `gpu`: run the encoder
    /// there (`None`: the CPU). `f16`: keep the encoder's matrices in f16.
    pub fn build(
        c: Config,
        w: &mut dyn Weights,
        vocab: Vocab,
        gpu: Option<&Context>,
        f16: bool,
    ) -> Result<Whisper, Error> {
        let Some(specials) = Specials::of(c.vocab) else {
            return bad(format!("a vocabulary of {} ids is not Whisper's", c.vocab));
        };
        if vocab.len() > specials.eot as usize + 1 {
            return bad("the vocabulary file does not match the model");
        }
        let d = c.width;
        let max_scores = gpu.map_or(u64::MAX, |g| {
            let l = g.device().limits();
            l.max_storage_buffer_binding_size.min(l.max_buffer_size) / 2
        });
        let encoder = match gpu {
            Some(ctx) => Encoder::Gpu(
                encoder_graphs(&c, w, f16, max_scores, GPU_MACS)?
                    .iter()
                    .map(|g| GpuGraph::new(ctx, g))
                    .collect::<Result<_, _>>()?,
            ),
            None => Encoder::Cpu(encoder_graph(&c, w, f16, max_scores)?),
        };
        // The decoder's matrices stay f32: read once per token, converting f16 as they are
        // read costs more than the memory saved.
        let embed = w
            .get("model.decoder.embed_tokens.weight", &[c.vocab, d])?
            .to_f32();
        let embed = Tensor::f32(&[c.vocab, d], embed);
        let pos = w
            .get("model.decoder.embed_positions.weight", &[c.text_ctx, d])?
            .to_f32();
        let mut layers = Vec::with_capacity(c.decoder_layers);
        let full = |t: Tensor| {
            let s = t.shape.clone();
            Tensor::f32(&s, t.to_f32())
        };
        let lin = |w: &mut dyn Weights, p: &str, (o, i): (usize, usize)| {
            Ok::<_, Error>((
                full(w.get(&format!("{p}.weight"), &[o, i])?),
                Tensor::f32(&[o], vecf(w, &format!("{p}.bias"), o)?),
            ))
        };
        for i in 0..c.decoder_layers {
            let p = format!("model.decoder.layers.{i}");
            let (qkv_w, qkv_b) = qkv(w, &format!("{p}.self_attn"), d)?;
            let (out, out_b) = lin(w, &format!("{p}.self_attn.out_proj"), (d, d))?;
            let (cq, cq_b) = lin(w, &format!("{p}.encoder_attn.q_proj"), (d, d))?;
            let (cout, cout_b) = lin(w, &format!("{p}.encoder_attn.out_proj"), (d, d))?;
            let (fc1, fc1_b) = lin(w, &format!("{p}.fc1"), (c.mlp, d))?;
            let (fc2, fc2_b) = lin(w, &format!("{p}.fc2"), (d, c.mlp))?;
            layers.push(Layer {
                ln1: lnw(w, &format!("{p}.self_attn_layer_norm"), d)?,
                qkv: qkv_w,
                qkv_b,
                out,
                out_b,
                ln2: lnw(w, &format!("{p}.encoder_attn_layer_norm"), d)?,
                cq,
                cq_b,
                cout,
                cout_b,
                ln3: lnw(w, &format!("{p}.final_layer_norm"), d)?,
                fc1,
                fc1_b,
                fc2,
                fc2_b,
            });
        }
        let ln_f = lnw(w, "model.decoder.layer_norm", d)?;
        let suppress = vocab.non_speech();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().clamp(1, 8));
        Ok(Whisper {
            mel: mel::Mel::new(c.n_mels),
            config: c,
            specials,
            vocab,
            encoder,
            embed,
            pos,
            layers,
            ln_f,
            threads,
            suppress,
            stats: std::cell::Cell::new(Stats::default()),
        })
    }

    /// Loads a checkpoint (`model.safetensors`, f32 or f16) and its `vocab.json`.
    pub fn load(
        weights: &Path,
        vocab: &Path,
        gpu: Option<&Context>,
        f16: bool,
    ) -> Result<Whisper, Error> {
        let mut file = SafeTensorsFile::open(weights)?;
        let c = Config::detect(&file.header)?;
        let text = std::fs::read(vocab)
            .map_err(|e| Error::Weights(format!("{}: {e}", vocab.display())))?;
        Whisper::build(c, &mut file, Vocab::parse(&text)?, gpu, f16)
    }

    /// Whether the encoder runs on the GPU.
    pub fn on_gpu(&self) -> bool {
        matches!(self.encoder, Encoder::Gpu(_))
    }

    /// The log-mel of up to one window of 16 kHz mono sound.
    pub fn mel(&self, audio: &[f32]) -> Vec<f32> {
        self.mel.log_mel(audio, self.config.frames())
    }

    /// Runs the encoder on a window's log-mel (`[n_mels, frames]`).
    pub fn encode(&mut self, mel: &[f32]) -> Result<Audio, Error> {
        let t0 = std::time::Instant::now();
        let a = self.encode_inner(mel)?;
        self.note(|s| s.encode += t0.elapsed());
        Ok(a)
    }

    /// The time spent so far.
    pub fn stats(&self) -> Stats {
        self.stats.get()
    }

    pub(crate) fn note(&self, f: impl FnOnce(&mut Stats)) {
        let mut s = self.stats.get();
        f(&mut s);
        self.stats.set(s);
    }

    fn encode_inner(&mut self, mel: &[f32]) -> Result<Audio, Error> {
        let c = &self.config;
        let x = Tensor::f32(&[1, c.n_mels, c.frames()], mel.to_vec());
        let outs = match &mut self.encoder {
            Encoder::Cpu(g) => cpu::run(g, &[Feed::Tensor(&x)], self.threads)?,
            Encoder::Gpu(parts) => {
                let mut cur = vec![x];
                for g in parts.iter_mut() {
                    cur = g.run(&[Feed::Tensor(&cur[0])])?;
                }
                cur
            }
        };
        let (d, t) = (c.width, c.audio_ctx);
        let kv = outs
            .into_iter()
            .map(|o| {
                let v = o.to_f32();
                let mut k = Vec::with_capacity(t * d);
                let mut vv = Vec::with_capacity(t * d);
                for row in v.chunks_exact(2 * d) {
                    k.extend_from_slice(&row[..d]);
                    vv.extend_from_slice(&row[d..]);
                }
                (k, vv)
            })
            .collect();
        Ok(Audio { kv })
    }

    /// A fresh decoder state.
    pub fn state(&self) -> State {
        let n = self.config.text_ctx * self.config.width;
        State {
            len: 0,
            kv: (0..self.config.decoder_layers)
                .map(|_| (vec![0f32; n], vec![0f32; n]))
                .collect(),
        }
    }

    /// `x · Wᵀ + b` on the CPU: a dot product per output, over contiguous rows of `W`
    /// (f32), rows split among the threads.
    fn linear(&self, x: &[f32], w: &Tensor, b: Option<&Tensor>, act: Act) -> Vec<f32> {
        let (n, k) = (w.shape[0], w.shape[1]);
        let wf = w.as_f32();
        let bias = b.map(|b| b.as_f32());
        let mut out = vec![0f32; n];
        const ROWS: usize = 256;
        let threads = if n * k < 1 << 17 { 1 } else { self.threads };
        let work: Vec<(usize, &mut [f32])> = out.chunks_mut(ROWS).enumerate().collect();
        cpu::par_work(work, threads, |(c, o)| {
            for (i, v) in o.iter_mut().enumerate() {
                let r = c * ROWS + i;
                let s = dot4(x, &wf[r * k..(r + 1) * k]) + bias.as_ref().map_or(0.0, |b| b[r]);
                *v = act.apply(s);
            }
        });
        out
    }

    /// Feeds `token` at the state's next position; the logits of the token after it.
    pub fn step(&self, st: &mut State, audio: &Audio, token: u32) -> Result<Vec<f32>, Error> {
        let t0 = std::time::Instant::now();
        let r = self.step_inner(st, audio, token);
        self.note(|s| {
            s.tokens += 1;
            s.decode += t0.elapsed();
        });
        r
    }

    fn step_inner(&self, st: &mut State, audio: &Audio, token: u32) -> Result<Vec<f32>, Error> {
        let c = &self.config;
        let d = c.width;
        let p = st.len;
        if p >= c.text_ctx {
            return Err(Error::Feed("the decoder's context is full".into()));
        }
        if token as usize >= c.vocab {
            return Err(Error::Feed(format!("token {token} out of range")));
        }
        let row = token as usize * d;
        let mut x: Vec<f32> = match &self.embed.data {
            crate::tensor::Data::F32(v) => v[row..row + d].to_vec(),
            crate::tensor::Data::F16(v) => v[row..row + d]
                .iter()
                .map(|&b| half::f16::from_bits(b).to_f32())
                .collect(),
        };
        for (a, b) in x.iter_mut().zip(&self.pos[p * d..(p + 1) * d]) {
            *a += b;
        }
        let scale = 1.0 / (HEAD as f32).sqrt();
        for (l, layer) in self.layers.iter().enumerate() {
            // Self-attention over the cache.
            let y = layer_norm(&x, &layer.ln1);
            let qkv = self.linear(&y, &layer.qkv, Some(&layer.qkv_b), Act::Identity);
            let (kc, vc) = &mut st.kv[l];
            kc[p * d..(p + 1) * d].copy_from_slice(&qkv[d..2 * d]);
            vc[p * d..(p + 1) * d].copy_from_slice(&qkv[2 * d..]);
            let a = attend_par(&qkv[..d], kc, vc, p + 1, c.heads, scale, self.threads);
            let o = self.linear(&a, &layer.out, Some(&layer.out_b), Act::Identity);
            add(&mut x, &o);
            // Cross-attention over the window.
            let y = layer_norm(&x, &layer.ln2);
            let q = self.linear(&y, &layer.cq, Some(&layer.cq_b), Act::Identity);
            let (ka, va) = &audio.kv[l];
            let a = attend_par(&q, ka, va, c.audio_ctx, c.heads, scale, self.threads);
            let o = self.linear(&a, &layer.cout, Some(&layer.cout_b), Act::Identity);
            add(&mut x, &o);
            // MLP.
            let y = layer_norm(&x, &layer.ln3);
            let m = self.linear(&y, &layer.fc1, Some(&layer.fc1_b), Act::Gelu);
            let m = self.linear(&m, &layer.fc2, Some(&layer.fc2_b), Act::Identity);
            add(&mut x, &m);
        }
        st.len = p + 1;
        let y = layer_norm(&x, &self.ln_f);
        Ok(self.linear(&y, &self.embed, None, Act::Identity))
    }
}

fn add(x: &mut [f32], y: &[f32]) {
    for (a, b) in x.iter_mut().zip(y) {
        *a += b;
    }
}

fn layer_norm(x: &[f32], (g, b): &(Vec<f32>, Vec<f32>)) -> Vec<f32> {
    let n = x.len() as f32;
    let mean = x.iter().sum::<f32>() / n;
    let var = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n;
    let r = 1.0 / (var + EPS).sqrt();
    x.iter()
        .zip(g.iter().zip(b))
        .map(|(v, (g, b))| (v - mean) * r * g + b)
        .collect()
}

/// One query `q [D]` against the first `n` rows of keys and values `[.., D]`, per head
/// (heads on up to `threads` threads).
fn attend_par(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    n: usize,
    heads: usize,
    scale: f32,
    threads: usize,
) -> Vec<f32> {
    let d = q.len();
    let hd = d / heads;
    let mut out = vec![0f32; d];
    // Small attentions (a short self-attention) are not worth a thread each.
    let threads = if n * d < 1 << 16 { 1 } else { threads };
    let work: Vec<(usize, &mut [f32])> = out.chunks_mut(hd).enumerate().collect();
    cpu::par_work(work, threads, |(h, oh)| {
        let qh = &q[h * hd..(h + 1) * hd];
        let mut s = vec![0f32; n];
        let mut top = f32::MIN;
        for (j, sj) in s.iter_mut().enumerate() {
            let kj = &k[j * d + h * hd..j * d + (h + 1) * hd];
            *sj = dot(qh, kj) * scale;
            top = top.max(*sj);
        }
        let mut sum = 0.0;
        for sj in s.iter_mut() {
            *sj = (*sj - top).exp();
            sum += *sj;
        }
        let inv = 1.0 / sum;
        for (j, sj) in s.iter().enumerate() {
            axpy(oh, sj * inv, &v[j * d + h * hd..j * d + (h + 1) * hd]);
        }
    });
    out
}

/// One query against keys and values, on this thread.
#[cfg(test)]
fn attend(q: &[f32], k: &[f32], v: &[f32], n: usize, heads: usize, scale: f32) -> Vec<f32> {
    attend_par(q, k, v, n, heads, scale, 1)
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    use std::simd::f32x8;
    use std::simd::num::SimdFloat;
    let n = a.len().min(b.len());
    let mut acc = f32x8::splat(0.0);
    let (ca, cb) = (a[..n].chunks_exact(8), b[..n].chunks_exact(8));
    let (ra, rb) = (ca.remainder(), cb.remainder());
    for (x, y) in ca.zip(cb) {
        acc += f32x8::from_slice(x) * f32x8::from_slice(y);
    }
    acc.reduce_sum() + ra.iter().zip(rb).map(|(x, y)| x * y).sum::<f32>()
}

/// A dot product with four accumulators (rows of a weight matrix).
fn dot4(a: &[f32], b: &[f32]) -> f32 {
    use std::simd::f32x8;
    use std::simd::num::SimdFloat;
    let n = a.len().min(b.len());
    let mut acc = [f32x8::splat(0.0); 4];
    let mut i = 0;
    while i + 32 <= n {
        for (j, ac) in acc.iter_mut().enumerate() {
            let o = i + 8 * j;
            *ac += f32x8::from_slice(&a[o..o + 8]) * f32x8::from_slice(&b[o..o + 8]);
        }
        i += 32;
    }
    let mut s = ((acc[0] + acc[1]) + (acc[2] + acc[3])).reduce_sum();
    while i < n {
        s += a[i] * b[i];
        i += 1;
    }
    s
}

/// `y += a·x`.
fn axpy(y: &mut [f32], a: f32, x: &[f32]) {
    for (o, v) in y.iter_mut().zip(x) {
        *o += a * v;
    }
}

/// Helpers for tests: a tiny random Whisper with the real vocabulary's layout.
#[doc(hidden)]
pub mod testing {
    use super::*;
    use crate::models::RandomWeights;

    /// A vocabulary with ordinary tokens for the bytes and a few words, padded to `eot`.
    pub fn vocab() -> Vocab {
        let mut s = String::from("{");
        let words = ["Hello", "Ġworld", ".", "Ġ(", "Ġthe"];
        for (i, w) in words.iter().enumerate() {
            s.push_str(&format!("\"{w}\": {i},"));
        }
        s.push_str("\"pad\": 50256}");
        Vocab::parse(s.as_bytes()).expect("vocab")
    }

    /// A tiny random Whisper ([`Config::test`]).
    pub fn tiny(seed: u64, gpu: Option<&Context>, f16: bool) -> Whisper {
        let mut w = RandomWeights { seed, f16: false };
        Whisper::build(Config::test(), &mut w, vocab(), gpu, f16).expect("builds")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RandomWeights;

    #[test]
    fn macs_of_known_sizes() {
        let base = Config {
            n_mels: 80,
            width: 512,
            heads: 8,
            encoder_layers: 6,
            decoder_layers: 6,
            audio_ctx: 1500,
            text_ctx: 448,
            vocab: 51_865,
            mlp: 2048,
        };
        // About 48 GMACs per window and 58 MMACs per token.
        assert!((40_000_000_000..60_000_000_000).contains(&base.encoder_macs()));
        assert!((40_000_000..60_000_000).contains(&base.token_macs()));
    }

    /// The decoder's cached steps against a full pass over every position at once.
    #[test]
    fn cached_steps_match_a_full_pass() {
        let mut m = testing::tiny(3, None, false);
        let audio_in: Vec<f32> = (0..16_000)
            .map(|i| ((i as f32) * 0.05).sin() * 0.3)
            .collect();
        let mel = m.mel(&audio_in);
        let a = m.encode(&mel).unwrap();
        let toks = [50_258u32, 50_259, 50_359, 50_364, 0, 1];
        let mut st = m.state();
        let mut last = Vec::new();
        for &t in &toks {
            last = m.step(&mut st, &a, t).unwrap();
        }
        assert_eq!(st.len(), toks.len());
        let want = full_pass(&m, &a, &toks);
        let d = crate::tensor::compare(&last, &want, 1e-4, 1e-4);
        assert!(d.worst <= 1.0, "{d:?}");
        assert!(last.iter().all(|v| v.is_finite()));
    }

    /// The last position's logits with every position's states recomputed (no cache).
    fn full_pass(m: &Whisper, a: &Audio, toks: &[u32]) -> Vec<f32> {
        let c = &m.config;
        let d = c.width;
        let emb = m.embed.to_f32();
        let mut xs: Vec<Vec<f32>> = toks
            .iter()
            .enumerate()
            .map(|(p, &t)| {
                (0..d)
                    .map(|j| emb[t as usize * d + j] + m.pos[p * d + j])
                    .collect()
            })
            .collect();
        let scale = 1.0 / (HEAD as f32).sqrt();
        for (l, layer) in m.layers.iter().enumerate() {
            let qkv: Vec<Vec<f32>> = xs
                .iter()
                .map(|x| {
                    m.linear(
                        &layer_norm(x, &layer.ln1),
                        &layer.qkv,
                        Some(&layer.qkv_b),
                        Act::Identity,
                    )
                })
                .collect();
            let k: Vec<f32> = qkv.iter().flat_map(|r| r[d..2 * d].to_vec()).collect();
            let v: Vec<f32> = qkv.iter().flat_map(|r| r[2 * d..].to_vec()).collect();
            for (i, x) in xs.iter_mut().enumerate() {
                let at = attend(&qkv[i][..d], &k, &v, i + 1, c.heads, scale);
                add(
                    x,
                    &m.linear(&at, &layer.out, Some(&layer.out_b), Act::Identity),
                );
                let q = m.linear(
                    &layer_norm(x, &layer.ln2),
                    &layer.cq,
                    Some(&layer.cq_b),
                    Act::Identity,
                );
                let at = attend(&q, &a.kv[l].0, &a.kv[l].1, c.audio_ctx, c.heads, scale);
                add(
                    x,
                    &m.linear(&at, &layer.cout, Some(&layer.cout_b), Act::Identity),
                );
                let h = m.linear(
                    &layer_norm(x, &layer.ln3),
                    &layer.fc1,
                    Some(&layer.fc1_b),
                    Act::Gelu,
                );
                add(
                    x,
                    &m.linear(&h, &layer.fc2, Some(&layer.fc2_b), Act::Identity),
                );
            }
        }
        let y = layer_norm(xs.last().unwrap(), &m.ln_f);
        (0..c.vocab)
            .map(|r| (0..d).map(|j| emb[r * d + j] * y[j]).sum())
            .collect()
    }

    /// The encoder graph on the CPU against a direct implementation of Whisper's encoder.
    #[test]
    fn encoder_graph_matches_a_direct_reference() {
        let c = Config::test();
        let mut w = RandomWeights {
            seed: 9,
            f16: false,
        };
        let g = encoder_graph(&c, &mut w, false, u64::MAX).unwrap();
        let mel: Vec<f32> = (0..c.n_mels * c.frames())
            .map(|i| ((i * 7919 % 1000) as f32 / 500.0) - 1.0)
            .collect();
        let x = Tensor::f32(&[1, c.n_mels, c.frames()], mel.clone());
        let outs = cpu::run(&g, &[Feed::Tensor(&x)], 2).unwrap();
        let want = reference_encoder(&c, &mut w, &mel);
        for (o, wv) in outs.iter().zip(&want) {
            let d = crate::tensor::compare(&o.to_f32(), wv, 1e-3, 1e-3);
            assert!(d.worst <= 1.0, "{d:?}");
        }
        // A chain of one block per graph gives the same as one graph.
        let parts = encoder_graphs(&c, &mut w, false, u64::MAX, 1).unwrap();
        assert_eq!(parts.len(), c.encoder_layers);
        let mut cur = vec![x.clone()];
        for p in &parts {
            cur = cpu::run(p, &[Feed::Tensor(&cur[0])], 2).unwrap();
        }
        for (a, b) in outs.iter().zip(&cur) {
            let d = crate::tensor::compare(&a.to_f32(), &b.to_f32(), 1e-5, 1e-5);
            assert!(d.worst <= 1.0, "{d:?}");
        }
        // Split attention gives the same as whole.
        let g2 = encoder_graph(&c, &mut w, false, (c.audio_ctx * c.audio_ctx * 4) as u64).unwrap();
        let outs2 = cpu::run(&g2, &[Feed::Tensor(&x)], 2).unwrap();
        for (a, b) in outs.iter().zip(&outs2) {
            let d = crate::tensor::compare(&a.to_f32(), &b.to_f32(), 1e-5, 1e-5);
            assert!(d.worst <= 1.0, "{d:?}");
        }
    }

    /// Whisper's encoder written out plainly (f64 accumulation).
    fn reference_encoder(c: &Config, w: &mut RandomWeights, mel: &[f32]) -> Vec<Vec<f32>> {
        let (d, t, f, nm) = (c.width, c.audio_ctx, c.frames(), c.n_mels);
        let get = |w: &mut RandomWeights, n: &str, s: &[usize]| w.get(n, s).unwrap().to_f32();
        let gelu = |x: f32| Act::Gelu.apply(x);
        let conv = |w: &mut RandomWeights,
                    x: &[f32],
                    cin: usize,
                    len: usize,
                    stride: usize,
                    n: &str|
         -> (Vec<f32>, usize) {
            let k = get(w, &format!("model.encoder.{n}.weight"), &[d, cin, 3]);
            let b = get(w, &format!("model.encoder.{n}.bias"), &[d]);
            let out_len = (len + 2 - 3) / stride + 1;
            let mut y = vec![0f32; d * out_len];
            for o in 0..d {
                for j in 0..out_len {
                    let mut s = f64::from(b[o]);
                    for ci in 0..cin {
                        for kk in 0..3 {
                            let pos = (j * stride + kk) as isize - 1;
                            if pos >= 0 && (pos as usize) < len {
                                s += f64::from(k[(o * cin + ci) * 3 + kk])
                                    * f64::from(x[ci * len + pos as usize]);
                            }
                        }
                    }
                    y[o * out_len + j] = gelu(s as f32);
                }
            }
            (y, out_len)
        };
        let (h1, l1) = conv(w, mel, nm, f, 1, "conv1");
        let (h2, l2) = conv(w, &h1, d, l1, 2, "conv2");
        assert_eq!(l2, t);
        let pos = get(w, "model.encoder.embed_positions.weight", &[t, d]);
        let mut x: Vec<Vec<f32>> = (0..t)
            .map(|i| (0..d).map(|j| h2[j * t + i] + pos[i * d + j]).collect())
            .collect();
        let lin = |w: &mut RandomWeights, x: &[f32], p: &str, o: usize, bias: bool| -> Vec<f32> {
            let wt = get(w, &format!("{p}.weight"), &[o, x.len()]);
            let b = if bias {
                get(w, &format!("{p}.bias"), &[o])
            } else {
                vec![0.0; o]
            };
            (0..o)
                .map(|r| {
                    let s: f64 = (0..x.len())
                        .map(|k| f64::from(wt[r * x.len() + k]) * f64::from(x[k]))
                        .sum();
                    (s + f64::from(b[r])) as f32
                })
                .collect()
        };
        let ln = |w: &mut RandomWeights, x: &[f32], p: &str| {
            let g = get(w, &format!("{p}.weight"), &[d]);
            let b = get(w, &format!("{p}.bias"), &[d]);
            layer_norm(x, &(g, b))
        };
        for li in 0..c.encoder_layers {
            let p = format!("model.encoder.layers.{li}");
            let y: Vec<Vec<f32>> = x
                .iter()
                .map(|r| ln(w, r, &format!("{p}.self_attn_layer_norm")))
                .collect();
            let q: Vec<Vec<f32>> = y
                .iter()
                .map(|r| lin(w, r, &format!("{p}.self_attn.q_proj"), d, true))
                .collect();
            let k: Vec<Vec<f32>> = y
                .iter()
                .map(|r| lin(w, r, &format!("{p}.self_attn.k_proj"), d, false))
                .collect();
            let v: Vec<Vec<f32>> = y
                .iter()
                .map(|r| lin(w, r, &format!("{p}.self_attn.v_proj"), d, true))
                .collect();
            let kf: Vec<f32> = k.concat();
            let vf: Vec<f32> = v.concat();
            for i in 0..t {
                let a = attend(&q[i], &kf, &vf, t, c.heads, 1.0 / (HEAD as f32).sqrt());
                let o = lin(w, &a, &format!("{p}.self_attn.out_proj"), d, true);
                add(&mut x[i], &o);
            }
            for r in x.iter_mut() {
                let y = ln(w, r, &format!("{p}.final_layer_norm"));
                let m: Vec<f32> = lin(w, &y, &format!("{p}.fc1"), c.mlp, true)
                    .into_iter()
                    .map(gelu)
                    .collect();
                let m = lin(w, &m, &format!("{p}.fc2"), d, true);
                add(r, &m);
            }
        }
        let x: Vec<Vec<f32>> = x
            .iter()
            .map(|r| ln(w, r, "model.encoder.layer_norm"))
            .collect();
        (0..c.decoder_layers)
            .map(|l| {
                let p = format!("model.decoder.layers.{l}.encoder_attn");
                let mut out = Vec::new();
                for r in &x {
                    out.extend(lin(w, r, &format!("{p}.k_proj"), d, false));
                    out.extend(lin(w, r, &format!("{p}.v_proj"), d, true));
                }
                out
            })
            .collect()
    }

    #[test]
    fn detects_sizes_from_names() {
        use crate::safetensors::{parse_header, serialize};
        let c = Config::test();
        struct Rec(RandomWeights, Vec<(String, Tensor)>);
        impl Weights for Rec {
            fn get(&mut self, n: &str, s: &[usize]) -> Result<Tensor, Error> {
                let t = self.0.get(n, s)?;
                if !self.1.iter().any(|(m, _)| m == n) {
                    self.1.push((n.to_owned(), t.clone()));
                }
                Ok(t)
            }
        }
        let mut r = Rec(RandomWeights::new(1), Vec::new());
        Whisper::build(c.clone(), &mut r, testing::vocab(), None, false).unwrap();
        let ts: Vec<(&str, &Tensor)> = r.1.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let bytes = serialize(&ts, &[]);
        let n = crate::safetensors::header_len(&bytes, bytes.len() as u64).unwrap() as usize;
        let h = parse_header(&bytes[8..8 + n], bytes.len() as u64).unwrap();
        assert_eq!(Config::detect(&h).unwrap(), c);
        let empty = parse_header(b"{}", 2 + 8).unwrap();
        assert!(Config::detect(&empty).is_err());
    }
}
