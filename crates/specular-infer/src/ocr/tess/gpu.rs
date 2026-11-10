//! Tesseract's LSTM network as a graph, for the GPU: the layers after the network's
//! convolution (which reads Tesseract's pseudo-random numbers outside the line, so it stays
//! on the CPU, see [`Front`]) become matrix products, pools and [`Op::Lstm`] steps on a
//! batch of lines.
//!
//! Lines of a batch are padded to one width. Everything before an LSTM along the line is
//! per column, and a forward LSTM's first steps never see later columns, so padding
//! changes nothing a line keeps; a reversed LSTM starts at each line's own end (the
//! lengths input). The CPU forward pass ([`Layer::forward`]) stays the reference.
//!
//! [`Op::Lstm`]: crate::graph::Op::Lstm

use super::net::{Act, Grid, Layer, Matrix, Plumb, TRand};
use crate::Error;
use crate::graph::{self, Graph, MatMul, Pool2d, PoolKind, Value};
use crate::tensor::{DType, Tensor};

fn unsupported<T>(what: &str) -> Result<T, Error> {
    Err(Error::Weights(format!(
        "Tesseract network on the GPU: {what}"
    )))
}

/// The top-level layers run on the CPU (up to and including the last convolution) and the
/// ones the graph runs.
pub struct Front<'a> {
    pub cpu: Vec<&'a Layer>,
    pub graph: Vec<&'a Layer>,
}

impl<'a> Front<'a> {
    /// Splits a network: it must be a series whose convolutions all come first.
    pub fn split(net: &'a Layer) -> Result<Front<'a>, Error> {
        let mut layers = Vec::new();
        flatten(net, &mut layers);
        let last_conv = layers
            .iter()
            .rposition(|l| matches!(l, Layer::Convolve { .. } | Layer::Input { .. }))
            .map_or(0, |i| i + 1);
        let (cpu, rest) = layers.split_at(last_conv);
        if rest.iter().any(|l| has_convolve(l)) {
            return unsupported("a convolution after the first layers");
        }
        Ok(Front {
            cpu: cpu.to_vec(),
            graph: rest.to_vec(),
        })
    }

    /// The CPU part on one line's input grid.
    pub fn run_cpu(&self, g: Grid, rng: &mut TRand) -> Result<Grid, Error> {
        let mut g = g;
        for l in &self.cpu {
            g = l.forward(g, rng)?;
        }
        Ok(g)
    }
}

/// A series' layers, nested series opened.
fn flatten<'a>(l: &'a Layer, out: &mut Vec<&'a Layer>) {
    match l {
        Layer::Plumbing {
            kind: Plumb::Series,
            layers,
        } => layers.iter().for_each(|l| flatten(l, out)),
        other => out.push(other),
    }
}

fn has_convolve(l: &Layer) -> bool {
    match l {
        Layer::Convolve { .. } => true,
        Layer::Plumbing { layers, .. } => layers.iter().any(has_convolve),
        Layer::Transpose(l) | Layer::ReverseX(l) | Layer::ReverseY(l) => has_convolve(l),
        _ => false,
    }
}

/// Graph-building state: the current value is `[N, H, W, D]`.
struct Build<'g> {
    g: &'g mut Graph,
    lens: Value,
    /// Input columns per step so far, and for the whole network.
    x_scale: usize,
    total_x_scale: usize,
    /// Inside an odd number of transposes (the W axis is the line's height).
    transposed: bool,
}

fn transposed_weights(m: &Matrix, cols: std::ops::Range<usize>) -> Tensor {
    // [rows, cols] (a slice of the columns) → [cols, rows].
    let k = cols.len();
    let mut v = vec![0f32; k * m.rows];
    for r in 0..m.rows {
        for (j, c) in cols.clone().enumerate() {
            v[j * m.rows + r] = m.w[r * m.cols + c];
        }
    }
    Tensor::f32(&[k, m.rows], v)
}

impl Build<'_> {
    fn shape4(&self, v: Value) -> [usize; 4] {
        let s = self.g.shape(v);
        [s[0], s[1], s[2], s[3]]
    }

    fn full(&mut self, v: Value, act: Act, m: &Matrix) -> Result<Value, Error> {
        let [n, h, w, d] = self.shape4(v);
        if m.cols != d + 1 {
            return unsupported("layer input width");
        }
        let x = self.g.reshape(v, &[n * h * w, d])?;
        let wt = self.g.constant(transposed_weights(m, 0..d))?;
        let b = self.g.constant(Tensor::f32(
            &[m.rows],
            (0..m.rows).map(|r| m.w[r * m.cols + d]).collect(),
        ))?;
        let gact = match act {
            Act::Tanh => graph::Act::Tanh,
            Act::Logistic => graph::Act::Sigmoid,
            Act::Relu => graph::Act::Relu,
            Act::Linear | Act::Softmax => graph::Act::Identity,
        };
        let y = self.g.matmul(
            x,
            wt,
            Some(b),
            MatMul {
                trans_b: false,
                act: gact,
            },
        )?;
        let y = if act == Act::Softmax {
            self.g.softmax(y)?
        } else {
            y
        };
        self.g.reshape(y, &[n, h, w, m.rows])
    }

    fn maxpool(&mut self, v: Value, sx: usize, sy: usize) -> Result<Value, Error> {
        let x = self.g.transpose(v, &[0, 3, 1, 2])?;
        let y = self.g.pool2d(
            x,
            Pool2d {
                kind: PoolKind::Max,
                kernel: [sy, sx],
                stride: [sy, sx],
                pad: [0; 4],
            },
        )?;
        self.x_scale *= if self.transposed { sy } else { sx };
        self.g.transpose(y, &[0, 2, 3, 1])
    }

    fn lstm(
        &mut self,
        v: Value,
        gates: &[Matrix; 4],
        ni: usize,
        ns: usize,
        summary: bool,
        reverse: bool,
    ) -> Result<Value, Error> {
        let [n, h, w, d] = self.shape4(v);
        if d != ni {
            return unsupported("LSTM input width");
        }
        let x = self.g.reshape(v, &[n * h, w, d])?;
        // Input weights of the four gates side by side: [ni, 4ns]; biases [4ns]; recurrent
        // weights [ns, 4ns].
        let (mut wx, mut wh, mut bias) = (
            vec![0f32; ni * 4 * ns],
            vec![0f32; ns * 4 * ns],
            vec![0f32; 4 * ns],
        );
        for (k, m) in gates.iter().enumerate() {
            for i in 0..ns {
                let row = &m.w[i * m.cols..(i + 1) * m.cols];
                let col = k * ns + i;
                for j in 0..ni {
                    wx[j * 4 * ns + col] = row[j];
                }
                for j in 0..ns {
                    wh[j * 4 * ns + col] = row[ni + j];
                }
                bias[col] = row[ni + ns];
            }
        }
        let wx = self.g.constant(Tensor::f32(&[ni, 4 * ns], wx))?;
        let b = self.g.constant(Tensor::f32(&[4 * ns], bias))?;
        let wh = self.g.constant(Tensor::f32(&[ns, 4 * ns], wh))?;
        let pre = self.g.matmul(
            x,
            wx,
            Some(b),
            MatMul {
                trans_b: false,
                act: graph::Act::Identity,
            },
        )?;
        let lens = if self.transposed {
            // Along the line's height: never padded.
            None
        } else {
            if self.x_scale != self.total_x_scale {
                return unsupported("an LSTM along the line before the last pooling");
            }
            Some((self.lens, h))
        };
        let y = self.g.lstm(pre, wh, lens, reverse, summary)?;
        self.g.reshape(y, &[n, h, if summary { 1 } else { w }, ns])
    }

    fn layer(&mut self, l: &Layer, v: Value) -> Result<Value, Error> {
        match l {
            Layer::Input { .. } => Ok(v),
            Layer::Full { act, m } => self.full(v, *act, m),
            Layer::Maxpool { sx, sy } => self.maxpool(v, *sx, *sy),
            Layer::Lstm {
                gates,
                ni,
                ns,
                summary,
            } => self.lstm(v, gates, *ni, *ns, *summary, false),
            Layer::ReverseX(inner) if !self.transposed => match inner.as_ref() {
                Layer::Lstm {
                    gates,
                    ni,
                    ns,
                    summary,
                } => self.lstm(v, gates, *ni, *ns, *summary, true),
                _ => unsupported("a reversed layer other than an LSTM"),
            },
            Layer::ReverseY(inner) if self.transposed => match inner.as_ref() {
                Layer::Lstm {
                    gates,
                    ni,
                    ns,
                    summary,
                } => self.lstm(v, gates, *ni, *ns, *summary, true),
                _ => unsupported("a reversed layer other than an LSTM"),
            },
            Layer::Transpose(inner) => {
                let t = self.g.transpose(v, &[0, 2, 1, 3])?;
                self.transposed = !self.transposed;
                let y = self.layer(inner, t);
                self.transposed = !self.transposed;
                self.g.transpose(y?, &[0, 2, 1, 3])
            }
            Layer::Plumbing {
                kind: Plumb::Series,
                layers,
            } => {
                let mut v = v;
                for l in layers {
                    v = self.layer(l, v)?;
                }
                Ok(v)
            }
            Layer::Plumbing {
                kind: Plumb::Parallel,
                layers,
            } => {
                let scale = self.x_scale;
                let mut outs = Vec::new();
                for l in layers {
                    self.x_scale = scale;
                    outs.push(self.layer(l, v)?);
                }
                self.g.concat(&outs, 3)
            }
            Layer::Convolve { .. } => unsupported("a convolution"),
            Layer::Reconfig { .. } => unsupported("a reconfiguration layer"),
            Layer::ReverseX(_) | Layer::ReverseY(_) => unsupported("a reversed layer"),
        }
    }
}

/// The graph for `n` lines `width` columns wide (after the CPU part: `height` rows, `depth`
/// values each). Inputs: the CPU part's output `[n, height, width, depth]` and each line's
/// output steps `[n]`; output `[n, 1, width / x_scale, classes]`.
pub fn build(
    front: &Front,
    total_x_scale: usize,
    n: usize,
    height: usize,
    width: usize,
    depth: usize,
) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let x = g.input(&[n, height, width, depth], DType::F32)?;
    let lens = g.input(&[n], DType::F32)?;
    let mut b = Build {
        g: &mut g,
        lens,
        x_scale: 1,
        total_x_scale: total_x_scale.max(1),
        transposed: false,
    };
    let mut v = x;
    for l in &front.graph {
        v = b.layer(l, v)?;
    }
    let s = g.shape(v).to_vec();
    if s[1] != 1 {
        return unsupported("the network does not reduce the height to one row");
    }
    g.output(v)?;
    Ok(g)
}
