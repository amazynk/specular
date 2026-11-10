//! Text in images (OCR): PaddleOCR's DB detector and CTC line recognisers ([`paddle`]), and
//! Tesseract's LSTM line recogniser for the scripts PaddleOCR lacks ([`tess`]), run on the
//! GPU (compiled graphs, cached per input size) or the CPU reference with the same graphs.
//!
//! - [`Detector::detect`]: an RGBA image → text boxes (four corners each).
//! - [`lines::crop_quad`]: each box warped to an upright line image.
//! - [`Orientation::straighten`]: lines the classifier finds upside down turned round.
//! - [`Recognizer::read`] / [`tess::Tesseract::read`]: line images → text and confidence,
//!   in logical order (right-to-left lines turned from the visual order they are read in).

pub mod ctc;
pub mod db;
pub mod lines;
pub mod paddle;
pub mod tess;

use std::path::Path;

use crate::gpu::{Context, GpuGraph};
use crate::tensor::Tensor;
use crate::{Error, Feed, Graph, cpu};

use self::ctc::Decoded;
use self::db::{DbParams, TextBox};
use self::lines::{Img8, Rgba};
use self::paddle::{Arch, Bank, Input};

/// Where graphs run.
#[derive(Clone)]
pub enum Runner {
    /// The CPU reference, with this many threads.
    Cpu(usize),
    Gpu(Context),
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Runner::Cpu(n) => write!(f, "Cpu({n})"),
            Runner::Gpu(_) => write!(f, "Gpu"),
        }
    }
}

/// Graphs kept per model (input sizes seen most recently).
const CACHED: usize = 12;

/// The detector's largest input side, whatever [`DetSize::max_side`] asks: a 4000-pixel
/// input ran long enough on an integrated GPU (Radeon 860M) for the driver to reset the
/// device.
pub const DET_MAX_SIDE: usize = 2560;

/// Line widths the recognisers run at (a line takes the smallest that holds it), so a few
/// graphs serve every line.
pub const REC_WIDTHS: [usize; 8] = [320, 448, 640, 960, 1280, 1920, 2560, 3200];

struct Built {
    input: Input,
    graph: Graph,
    gpu: Option<GpuGraph>,
}

/// A Paddle network with its built graphs.
pub struct PaddleNet {
    pub arch: Arch,
    bank: Bank,
    f16: bool,
    cache: Vec<Built>,
}

impl PaddleNet {
    pub fn new(arch: Arch, bank: Bank, f16: bool) -> PaddleNet {
        PaddleNet {
            arch,
            bank,
            f16,
            cache: Vec::new(),
        }
    }

    /// Reads `config.json` and the weights.
    pub fn open(config: &Path, weights: &Path, f16: bool) -> Result<PaddleNet, Error> {
        let text = std::fs::read(config)
            .map_err(|e| Error::Weights(format!("{}: {e}", config.display())))?;
        let arch = Arch::parse(&text)?;
        Ok(PaddleNet::new(arch, Bank::open(weights)?, f16))
    }

    /// The graph for `input` (built on first use), moved to the front of the cache.
    pub fn graph(&mut self, input: Input) -> Result<&Graph, Error> {
        let i = self.slot(input, None)?;
        Ok(&self.cache[i].graph)
    }

    fn slot(&mut self, input: Input, ctx: Option<&Context>) -> Result<usize, Error> {
        let i = match self.cache.iter().position(|b| b.input == input) {
            Some(i) => i,
            None => {
                let graph = paddle::build(&self.arch, &self.bank, input, self.f16)?;
                if self.cache.len() >= CACHED {
                    self.cache.pop();
                }
                self.cache.push(Built {
                    input,
                    graph,
                    gpu: None,
                });
                self.cache.len() - 1
            }
        };
        let b = self.cache.remove(i);
        self.cache.insert(0, b);
        if let Some(ctx) = ctx
            && self.cache[0].gpu.is_none()
        {
            self.cache[0].gpu = Some(GpuGraph::new(ctx, &self.cache[0].graph)?);
        }
        Ok(0)
    }

    /// Runs the graph for `input` with `feeds`; its single output.
    pub fn run(&mut self, runner: &Runner, input: Input, feeds: &[Feed]) -> Result<Tensor, Error> {
        let out = match runner {
            Runner::Cpu(threads) => {
                let i = self.slot(input, None)?;
                cpu::run(&self.cache[i].graph, feeds, *threads)?
            }
            Runner::Gpu(ctx) => {
                let i = self.slot(input, Some(ctx))?;
                let g = self.cache[i].gpu.as_mut().expect("built above");
                g.run(feeds)?
            }
        };
        out.into_iter()
            .next()
            .ok_or(Error::Feed("graph without output".into()))
    }
}

/// How large the detector looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetSize {
    /// The short side is enlarged to at least this (Paddle's `limit_side_len` with
    /// `limit_type = "min"`).
    pub min_side: usize,
    /// The long side is reduced to at most this.
    pub max_side: usize,
}

impl Default for DetSize {
    fn default() -> DetSize {
        DetSize {
            min_side: 736,
            max_side: 1920,
        }
    }
}

impl DetSize {
    /// The detector input (height, width) for an image of `w × h`: scaled so the short side
    /// is at least `min_side` and the long side at most `max_side`, each side rounded to a
    /// multiple of 32 (at least 32).
    pub fn input(&self, w: usize, h: usize) -> (usize, usize) {
        let (wf, hf) = (w.max(1) as f64, h.max(1) as f64);
        let mut ratio = 1.0f64;
        if wf.min(hf) < self.min_side as f64 {
            ratio = self.min_side as f64 / wf.min(hf);
        }
        let max_side = self.max_side.clamp(32, DET_MAX_SIDE) as f64;
        if wf.max(hf) * ratio > max_side {
            ratio = max_side / wf.max(hf);
        }
        let r32 = |v: f64| (((v * ratio / 32.0).round() as usize) * 32).max(32);
        (r32(hf), r32(wf))
    }
}

/// The text detector.
pub struct Detector {
    pub net: PaddleNet,
    pub params: DbParams,
    pub size: DetSize,
}

impl Detector {
    pub fn new(net: PaddleNet) -> Result<Detector, Error> {
        if !net.arch.is_detector() {
            return Err(Error::Weights("not a text detector".into()));
        }
        Ok(Detector {
            net,
            params: DbParams::default(),
            size: DetSize::default(),
        })
    }

    /// The probability map for `img` at the detector's input size: (map, width, height).
    pub fn prob_map(
        &mut self,
        runner: &Runner,
        img: &Rgba,
    ) -> Result<(Vec<f32>, usize, usize), Error> {
        let (h, w) = self.size.input(img.width, img.height);
        self.prob_map_at(runner, img, w, h)
    }

    /// As [`Detector::prob_map`] at a given input size (multiples of 32).
    pub fn prob_map_at(
        &mut self,
        runner: &Runner,
        img: &Rgba,
        w: usize,
        h: usize,
    ) -> Result<(Vec<f32>, usize, usize), Error> {
        let feed = Feed::Rgba {
            width: img.width as u32,
            height: img.height as u32,
            data: img.data,
        };
        let t = self.net.run(
            runner,
            Input::Image {
                height: h,
                width: w,
            },
            &[feed],
        )?;
        Ok((t.to_f32(), w, h))
    }

    /// Text boxes in `img`, in reading order, in image pixels.
    pub fn detect(&mut self, runner: &Runner, img: &Rgba) -> Result<Vec<TextBox>, Error> {
        let (map, w, h) = self.prob_map(runner, img)?;
        Ok(db::boxes(&map, w, h, img.width, img.height, &self.params))
    }
}

/// The orientation classifier's input size.
pub const CLS_HEIGHT: usize = 80;
pub const CLS_WIDTH: usize = 160;

/// A line is turned when the classifier gives "upside down" at least this probability
/// (PaddleOCR's `cls_thresh`).
pub const UPSIDE_DOWN: f32 = 0.9;

/// A line the classifier gives at least this probability of being upside down is unsure
/// enough to be read both ways, the more confident reading kept (the classifier is weak
/// on some scripts, such as Arabic and Korean).
pub const BOTH_WAYS: f32 = 0.05;

/// ...and only when its upright reading is less confident than this: a confident upright
/// reading is kept without reading the line turned. On 240 synthetic lines in 12 scripts
/// (2026-11-02): turned lines caught 238/240 as without the gate, upright lines read twice
/// 41 → 24; a gate of 0.9 catches 231, 0.95 237.
pub const BOTH_WAYS_BELOW: f32 = 0.98;

/// PaddleOCR's text-line orientation classifier: is a line upside down?
pub struct Orientation {
    pub net: PaddleNet,
    /// Lines per run at most.
    pub batch: usize,
}

impl Orientation {
    pub fn new(net: PaddleNet) -> Result<Orientation, Error> {
        if !net.arch.is_classifier() || net.arch.classes() != 2 {
            return Err(Error::Weights(
                "not a two-way orientation classifier".into(),
            ));
        }
        Ok(Orientation { net, batch: 8 })
    }

    /// Reads `config.json` and the weights.
    pub fn open(config: &Path, weights: &Path, f16: bool) -> Result<Orientation, Error> {
        Orientation::new(PaddleNet::open(config, weights, f16)?)
    }

    /// The probability each line is upside down, in the order given.
    pub fn upside_down(&mut self, runner: &Runner, lines: &[Img8]) -> Result<Vec<f32>, Error> {
        let mut out = Vec::with_capacity(lines.len());
        let size = self.batch.max(1);
        for chunk in lines.chunks(size) {
            // Batches of 1 or `batch` lines, so two graphs serve every call.
            let n = if chunk.len() == 1 { 1 } else { size };
            let mut refs: Vec<&Img8> = chunk.iter().collect();
            while refs.len() < n {
                refs.push(&chunk[0]);
            }
            let x = lines::cls_batch(&refs);
            let input = Input::Tensor {
                batch: n,
                height: CLS_HEIGHT,
                width: CLS_WIDTH,
            };
            let p = self.net.run(runner, input, &[Feed::Tensor(&x)])?.to_f32();
            out.extend(p.chunks_exact(2).take(chunk.len()).map(|c| c[1]));
        }
        Ok(out)
    }

    /// Turns the lines the classifier finds upside down; how many it turned.
    pub fn straighten(&mut self, runner: &Runner, lines: &mut [Img8]) -> Result<usize, Error> {
        let p = self.upside_down(runner, lines)?;
        let mut n = 0;
        for (l, p) in lines.iter_mut().zip(p) {
            if p >= UPSIDE_DOWN {
                *l = l.rot180();
                n += 1;
            }
        }
        Ok(n)
    }
}

/// How line widths are chosen for a recogniser batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widths {
    /// Paddle's width for each line exactly (one graph per width; for checks against Paddle).
    Exact,
    /// Rounded up to a multiple of this, so fewer graphs are built.
    Bucket(usize),
    /// The smallest of [`REC_WIDTHS`] that holds the line (the default).
    Steps,
}

/// A Paddle line recogniser.
pub struct Recognizer {
    pub net: PaddleNet,
    /// Index 0 is the CTC blank.
    pub chars: Vec<String>,
    pub widths: Widths,
    /// Lines per run at most.
    pub batch: usize,
}

impl Recognizer {
    pub fn new(net: PaddleNet, chars: Vec<String>) -> Result<Recognizer, Error> {
        if net.arch.is_detector() || net.arch.is_classifier() {
            return Err(Error::Weights("not a line recogniser".into()));
        }
        if net.arch.classes() != chars.len() {
            return Err(Error::Weights(format!(
                "the model has {} classes, its character list {}",
                net.arch.classes(),
                chars.len()
            )));
        }
        Ok(Recognizer {
            net,
            chars,
            widths: Widths::Steps,
            batch: 8,
        })
    }

    /// Opens a recogniser export: `config.json`, `model.safetensors` (or a converted file
    /// given as `weights`) and `preprocessor_config.json`.
    pub fn open(
        config: &Path,
        weights: &Path,
        preproc: &Path,
        f16: bool,
    ) -> Result<Recognizer, Error> {
        let net = PaddleNet::open(config, weights, f16)?;
        let text = std::fs::read(preproc)
            .map_err(|e| Error::Weights(format!("{}: {e}", preproc.display())))?;
        Recognizer::new(net, paddle::read_characters(&text)?)
    }

    fn width_for(&self, l: &Img8) -> usize {
        let w = lines::rec_width(l.width, l.height);
        match self.widths {
            Widths::Exact => w,
            Widths::Bucket(b) => w.div_ceil(b.max(8)) * b.max(8),
            Widths::Steps => REC_WIDTHS.into_iter().find(|&s| s >= w).unwrap_or(w),
        }
        .min(paddle::REC_MAX_WIDTH.div_ceil(8) * 8)
    }

    /// The per-step probabilities for `lines` (all at `width`): `[N, T, classes]`.
    pub fn probs(
        &mut self,
        runner: &Runner,
        lines: &[&Img8],
        width: usize,
    ) -> Result<Tensor, Error> {
        let x = lines::rec_batch(lines, width);
        let input = Input::Tensor {
            batch: lines.len(),
            height: paddle::REC_HEIGHT,
            width,
        };
        self.net.run(runner, input, &[Feed::Tensor(&x)])
    }

    /// Reads each line; results in the order given. Right-to-left lines come back in
    /// logical order.
    pub fn read(&mut self, runner: &Runner, lines: &[Img8]) -> Result<Vec<Decoded>, Error> {
        let mut out: Vec<Option<Decoded>> = vec![None; lines.len()];
        // Group by width, widest first; batches of the same width.
        let mut order: Vec<(usize, usize)> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| (self.width_for(l), i))
            .collect();
        order.sort_unstable_by(|a, b| b.cmp(a));
        let classes = self.chars.len();
        let mut k = 0;
        while k < order.len() {
            let width = order[k].0;
            let mut end = k + 1;
            while end < order.len() && end - k < self.batch.max(1) && order[end].0 == width {
                end += 1;
            }
            // Batches of 1, 4 or 8 lines only, so the graphs are reused.
            let n = end - k;
            let size = if n == 1 {
                1
            } else if n <= 4 {
                4.min(self.batch.max(1))
            } else {
                self.batch.max(1)
            };
            let mut refs: Vec<&Img8> = order[k..end].iter().map(|&(_, i)| &lines[i]).collect();
            let blank = Img8 {
                width: 1,
                height: 1,
                channels: 3,
                data: vec![128; 3],
            };
            while refs.len() < size {
                refs.push(&blank);
            }
            let p = self.probs(runner, &refs, width)?;
            let v = p.to_f32();
            let per = v.len() / size.max(1);
            for (j, &(_, i)) in order[k..end].iter().enumerate() {
                let mut d = ctc::greedy(&v[j * per..(j + 1) * per], classes, &self.chars);
                d.text = ctc::visual_to_logical(&d.text);
                out[i] = Some(d);
            }
            k = end;
        }
        Ok(out
            .into_iter()
            .map(|d| {
                d.unwrap_or(Decoded {
                    text: String::new(),
                    confidence: 0.0,
                    steps: Vec::new(),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detector_sizes() {
        let d = DetSize::default();
        // A 12 MP photo: the long side to 1920, both multiples of 32.
        assert_eq!(d.input(4000, 3000), (1440, 1920));
        // A small screenshot: the short side up to 736.
        assert_eq!(d.input(640, 480), (736, 992));
        // Never past the cap, whatever is asked.
        let big = DetSize {
            min_side: 736,
            max_side: 9000,
        };
        assert_eq!(big.input(4000, 3000).1, DET_MAX_SIDE);
    }
}
