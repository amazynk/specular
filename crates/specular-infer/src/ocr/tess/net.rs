//! Tesseract's LSTM network: our own reader of its serialisation (the `lstm` component of a
//! `.traineddata` file) and forward pass on the CPU.
//!
//! A network is a tree of layers, read depth first. Every layer starts with the same
//! header (a type byte, 0 followed by the type's name as a length-prefixed string; a
//! training-state byte; a back-propagation byte; four little-endian i32: flags, inputs,
//! outputs, weight count; the layer's name), then its own data:
//!
//! - **plumbing** (`Series`, `Parallel`, `Replicated`, `ParBidiLSTM`, `DepParUDLSTM`,
//!   `Par2dLSTM`, `XYTranspose`, `RTLReversed`, `TTBReversed`): an i32 count and that many
//!   layers, then per-layer learning rates (u32 count, f32 each) when flag 64 is set;
//! - `Input`: its shape (batch, height, width, depth, loss type);
//! - `Convolve`: half widths x, y; `Maxpool` / `Reconfig`: scales x, y;
//! - fully connected (`Tanh`, `Logistic`, `Relu`, `Linear`, `Softmax`, …): one weight matrix;
//! - `LSTM` / `SummLSTM`: an i32 gate input width, then four weight matrices (cell input,
//!   input gate, forget gate, output gate).
//!
//! A weight matrix is a mode byte (bit 7: f64 weights, bit 0: i8, bit 2: Adam), its two
//! dimensions (u32 outputs, u32 inputs + 1, the last column the bias), one "empty" value
//! and the values. Only f64 ("best") matrices are read; i8 ("fast") ones are refused.
//!
//! The data flows as a grid of `height × width` positions, `depth` values each, rows top
//! to bottom. 1-D LSTMs run along each row, starting from a zero state; a summarising LSTM
//! keeps only each row's last output; `XYTranspose` swaps rows and columns around the
//! layer inside it, `RTLReversed` / `TTBReversed` mirror the grid around it.

use crate::Error;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

/// Largest values the reader accepts (no real model is near them).
const MAX_DIM: usize = 1 << 16;
const MAX_LAYERS: usize = 256;
const MAX_DEPTH: usize = 32;

/// A little-endian reader over a byte slice.
pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        let Some(end) = end else {
            return bad("the network data ends early");
        };
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }

    pub fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(
            self.bytes(4)?.try_into().expect("4 bytes"),
        ))
    }

    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().expect("4 bytes"),
        ))
    }

    pub fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_le_bytes(
            self.bytes(4)?.try_into().expect("4 bytes"),
        ))
    }

    pub fn f64(&mut self) -> Result<f64, Error> {
        Ok(f64::from_le_bytes(
            self.bytes(8)?.try_into().expect("8 bytes"),
        ))
    }

    pub fn size(&mut self) -> Result<usize, Error> {
        let v = self.i32()?;
        match usize::try_from(v) {
            Ok(n) if n <= MAX_DIM => Ok(n),
            _ => bad(format!("size {v} out of range")),
        }
    }

    pub fn string(&mut self) -> Result<String, Error> {
        let n = self.u32()? as usize;
        if n > MAX_DIM {
            return bad("string too long");
        }
        Ok(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
}

/// A weight matrix: `rows` outputs, `cols` = inputs + 1 (the bias last), row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub w: Vec<f32>,
}

impl Matrix {
    fn read(r: &mut Reader) -> Result<Matrix, Error> {
        let mode = r.u8()?;
        if mode & 1 != 0 {
            return bad("an integer (\"fast\") model: use the \"best\" traineddata");
        }
        if mode & 128 == 0 {
            return bad("an old weight format");
        }
        let rows = r.u32()? as usize;
        let cols = r.u32()? as usize;
        if rows == 0 || cols == 0 || rows > MAX_DIM || cols > MAX_DIM {
            return bad("weight matrix size");
        }
        let _empty = r.f64()?;
        let n = rows * cols;
        let raw = r.bytes(n * 8)?;
        let w = raw
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().expect("8 bytes")) as f32)
            .collect();
        Ok(Matrix { rows, cols, w })
    }

    /// `out[i] = act(W[i, ..k] · x + W[i, cols−1])` for an input `x` of `cols − 1` values.
    fn apply(&self, x: &[f32], out: &mut [f32]) {
        let k = self.cols - 1;
        for (i, o) in out.iter_mut().enumerate().take(self.rows) {
            let row = &self.w[i * self.cols..(i + 1) * self.cols];
            *o = dot(&row[..k], x) + row[k];
        }
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (a, b) = (&a[..n], &b[..n]);
    let mut acc = [0f32; 8];
    let mut ca = a.chunks_exact(8);
    let mut cb = b.chunks_exact(8);
    for (x, y) in (&mut ca).zip(&mut cb) {
        for i in 0..8 {
            acc[i] += x[i] * y[i];
        }
    }
    let mut s: f32 = acc.iter().sum();
    for (x, y) in ca.remainder().iter().zip(cb.remainder()) {
        s += x * y;
    }
    s
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Tanh,
    Logistic,
    Relu,
    Linear,
    Softmax,
}

impl Act {
    fn apply(self, v: &mut [f32]) {
        match self {
            Act::Tanh => v.iter_mut().for_each(|x| *x = x.tanh()),
            Act::Logistic => v.iter_mut().for_each(|x| *x = sigmoid(*x)),
            Act::Relu => v.iter_mut().for_each(|x| *x = x.max(0.0)),
            Act::Linear => {}
            Act::Softmax => {
                let m = v.iter().copied().fold(f32::MIN, f32::max);
                let mut s = 0f32;
                for x in v.iter_mut() {
                    *x = (*x - m).exp();
                    s += *x;
                }
                if s > 0.0 {
                    v.iter_mut().for_each(|x| *x /= s);
                }
            }
        }
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// How plumbing combines the layers inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plumb {
    /// One after the other.
    Series,
    /// All on the same input, outputs side by side in depth.
    Parallel,
}

/// A layer.
#[derive(Clone, Debug, PartialEq)]
pub enum Layer {
    Input {
        height: usize,
        depth: usize,
    },
    Convolve {
        half_x: usize,
        half_y: usize,
    },
    Maxpool {
        sx: usize,
        sy: usize,
    },
    Reconfig {
        sx: usize,
        sy: usize,
    },
    Full {
        act: Act,
        m: Matrix,
    },
    /// Gates: cell input, input, forget, output; `ns` states.
    Lstm {
        gates: Box<[Matrix; 4]>,
        ni: usize,
        ns: usize,
        summary: bool,
    },
    Plumbing {
        kind: Plumb,
        layers: Vec<Layer>,
    },
    /// Rows and columns swapped around the layer.
    Transpose(Box<Layer>),
    /// Mirrored left to right around the layer.
    ReverseX(Box<Layer>),
    /// Mirrored top to bottom around the layer.
    ReverseY(Box<Layer>),
}

struct Head {
    ty: String,
    flags: i32,
    ni: usize,
    no: usize,
}

const KINDS: [&str; 27] = [
    "Invalid",
    "Input",
    "Convolve",
    "Maxpool",
    "Parallel",
    "Replicated",
    "ParBidiLSTM",
    "DepParUDLSTM",
    "Par2dLSTM",
    "Series",
    "Reconfig",
    "RTLReversed",
    "TTBReversed",
    "XYTranspose",
    "LSTM",
    "SummLSTM",
    "Logistic",
    "LinLogistic",
    "LinTanh",
    "Tanh",
    "Relu",
    "Linear",
    "Softmax",
    "SoftmaxNoCTC",
    "LSTMSoftmax",
    "LSTMBinarySoftmax",
    "TensorFlow",
];

fn head(r: &mut Reader) -> Result<Head, Error> {
    let t = r.u8()?;
    let ty = if t == 0 {
        r.string()?
    } else {
        match KINDS.get(t as usize) {
            Some(k) => (*k).to_owned(),
            None => return bad(format!("layer type {t}")),
        }
    };
    let _training = r.u8()?;
    let _backprop = r.u8()?;
    let flags = r.i32()?;
    let ni = r.size()?;
    let no = r.size()?;
    let _weights = r.i32()?;
    let _name = r.string()?;
    Ok(Head { ty, flags, ni, no })
}

impl Layer {
    /// Reads one layer (and everything inside it).
    pub fn read(r: &mut Reader) -> Result<Layer, Error> {
        Layer::read_at(r, 0)
    }

    fn read_at(r: &mut Reader, depth: usize) -> Result<Layer, Error> {
        if depth > MAX_DEPTH {
            return bad("network nested too deeply");
        }
        let h = head(r)?;
        let pair = |r: &mut Reader| -> Result<(usize, usize), Error> {
            let (x, y) = (r.size()?, r.size()?);
            if x == 0 || y == 0 || x > 64 || y > 64 {
                return bad("scale out of range");
            }
            Ok((x, y))
        };
        Ok(match h.ty.as_str() {
            "Series" | "Parallel" | "Replicated" | "ParBidiLSTM" | "DepParUDLSTM"
            | "XYTranspose" | "RTLReversed" | "TTBReversed" => {
                let n = r.size()?;
                if n > MAX_LAYERS {
                    return bad("too many layers");
                }
                let mut layers = Vec::with_capacity(n);
                for _ in 0..n {
                    layers.push(Layer::read_at(r, depth + 1)?);
                }
                if h.flags & 64 != 0 {
                    let k = r.u32()? as usize;
                    r.bytes(k.checked_mul(4).ok_or(Error::Weights("rates".into()))?)?;
                }
                match h.ty.as_str() {
                    "Series" => Layer::Plumbing {
                        kind: Plumb::Series,
                        layers,
                    },
                    "XYTranspose" | "RTLReversed" | "TTBReversed" => {
                        let [inner] = <[Layer; 1]>::try_from(layers).map_err(|_| {
                            Error::Weights(format!("{} with other than one layer", h.ty))
                        })?;
                        let b = Box::new(inner);
                        match h.ty.as_str() {
                            "XYTranspose" => Layer::Transpose(b),
                            "RTLReversed" => Layer::ReverseX(b),
                            _ => Layer::ReverseY(b),
                        }
                    }
                    _ => Layer::Plumbing {
                        kind: Plumb::Parallel,
                        layers,
                    },
                }
            }
            "Input" => {
                let _batch = r.i32()?;
                let height = r.size()?;
                let _width = r.i32()?;
                let depth = r.size()?;
                let _loss = r.i32()?;
                Layer::Input { height, depth }
            }
            "Convolve" => {
                let (half_x, half_y) = (r.size()?, r.size()?);
                if half_x > 16 || half_y > 16 {
                    return bad("convolution too wide");
                }
                Layer::Convolve { half_x, half_y }
            }
            "Maxpool" => {
                let (sx, sy) = pair(r)?;
                Layer::Maxpool { sx, sy }
            }
            "Reconfig" => {
                let (sx, sy) = pair(r)?;
                Layer::Reconfig { sx, sy }
            }
            "Logistic" | "Tanh" | "Relu" | "Linear" | "Softmax" | "SoftmaxNoCTC"
            | "LinLogistic" | "LinTanh" => {
                let m = Matrix::read(r)?;
                if m.rows != h.no || m.cols != h.ni + 1 {
                    return bad(format!(
                        "{} weights {}×{} for {}→{}",
                        h.ty, m.rows, m.cols, h.ni, h.no
                    ));
                }
                let act = match h.ty.as_str() {
                    "Logistic" | "LinLogistic" => Act::Logistic,
                    "Tanh" | "LinTanh" => Act::Tanh,
                    "Relu" => Act::Relu,
                    "Linear" => Act::Linear,
                    _ => Act::Softmax,
                };
                Layer::Full { act, m }
            }
            "LSTM" | "SummLSTM" => {
                let na = r.size()?;
                let gates = [
                    Matrix::read(r)?,
                    Matrix::read(r)?,
                    Matrix::read(r)?,
                    Matrix::read(r)?,
                ];
                let ns = gates[0].rows;
                if na != h.ni + ns
                    || gates.iter().any(|g| g.rows != ns || g.cols != na + 1)
                    || ns != h.no
                {
                    return bad("a 2-D or softmax-feedback LSTM (not supported)");
                }
                Layer::Lstm {
                    gates: Box::new(gates),
                    ni: h.ni,
                    ns,
                    summary: h.ty == "SummLSTM",
                }
            }
            other => return bad(format!("layer {other:?} not supported")),
        })
    }
}

/// Tesseract's pseudo-random numbers (a 64-bit linear congruential generator): what its
/// convolutions read outside the image. Seeded per line as Tesseract seeds it, so the same
/// line always reads the same.
#[derive(Clone, Copy, Debug)]
pub struct TRand(pub u64);

impl TRand {
    /// The generator as Tesseract's line recogniser sets it up: seeded from the model's
    /// sample counter, one number drawn.
    pub fn for_line(sample_iteration: i32) -> TRand {
        let mut r = TRand((i64::from(sample_iteration).wrapping_mul(0x1000_0001)) as u64);
        r.int();
        r
    }

    fn int(&mut self) -> i32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as i32
    }

    /// Uniform in [−range, range].
    pub fn signed(&mut self, range: f64) -> f32 {
        (range * 2.0 * f64::from(self.int()) / f64::from(i32::MAX) - range) as f32
    }
}

/// A grid of values: `h × w` positions of `d` values, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub h: usize,
    pub w: usize,
    pub d: usize,
    pub v: Vec<f32>,
}

impl Grid {
    pub fn new(h: usize, w: usize, d: usize) -> Grid {
        Grid {
            h,
            w,
            d,
            v: vec![0.0; h * w * d],
        }
    }

    pub fn at(&self, y: usize, x: usize) -> &[f32] {
        let i = (y * self.w + x) * self.d;
        &self.v[i..i + self.d]
    }

    fn at_mut(&mut self, y: usize, x: usize) -> &mut [f32] {
        let i = (y * self.w + x) * self.d;
        &mut self.v[i..i + self.d]
    }

    fn transposed(&self) -> Grid {
        let mut g = Grid::new(self.w, self.h, self.d);
        for y in 0..self.h {
            for x in 0..self.w {
                g.at_mut(x, y).copy_from_slice(self.at(y, x));
            }
        }
        g
    }

    fn mirrored_x(&self) -> Grid {
        let mut g = Grid::new(self.h, self.w, self.d);
        for y in 0..self.h {
            for x in 0..self.w {
                g.at_mut(y, self.w - 1 - x).copy_from_slice(self.at(y, x));
            }
        }
        g
    }

    fn mirrored_y(&self) -> Grid {
        self.transposed().mirrored_x().transposed()
    }
}

fn lstm(g: &Grid, gates: &[Matrix; 4], ni: usize, ns: usize, summary: bool) -> Grid {
    let mut out = Grid::new(g.h, if summary { 1 } else { g.w }, ns);
    let cols = ni + ns + 1;
    let mut pre = vec![0f32; 4 * ns];
    let (mut c, mut hprev) = (vec![0f32; ns], vec![0f32; ns]);
    for y in 0..g.h {
        c.iter_mut().for_each(|v| *v = 0.0);
        hprev.iter_mut().for_each(|v| *v = 0.0);
        for x in 0..g.w {
            let xin = g.at(y, x);
            for (k, m) in gates.iter().enumerate() {
                for i in 0..ns {
                    let row = &m.w[i * cols..(i + 1) * cols];
                    pre[k * ns + i] =
                        dot(&row[..ni], xin) + dot(&row[ni..ni + ns], &hprev) + row[ni + ns];
                }
            }
            for i in 0..ns {
                let ci = pre[i].tanh();
                let gi = sigmoid(pre[ns + i]);
                let gf = sigmoid(pre[2 * ns + i]);
                let go = sigmoid(pre[3 * ns + i]);
                c[i] = (c[i] * gf + ci * gi).clamp(-100.0, 100.0);
                hprev[i] = c[i].tanh() * go;
            }
            if !summary {
                out.at_mut(y, x).copy_from_slice(&hprev);
            }
        }
        if summary {
            out.at_mut(y, 0).copy_from_slice(&hprev);
        }
    }
    out
}

impl Layer {
    /// Runs the layer on `g`. Convolutions read random values from `rng` outside the grid.
    pub fn forward(&self, g: Grid, rng: &mut TRand) -> Result<Grid, Error> {
        Ok(match self {
            Layer::Input { depth, .. } => {
                if g.d != *depth {
                    return bad("input depth");
                }
                g
            }
            Layer::Convolve { half_x, half_y } => {
                let (kx, ky) = (2 * half_x + 1, 2 * half_y + 1);
                let mut out = Grid::new(g.h, g.w, g.d * kx * ky);
                for y in 0..g.h {
                    for x in 0..g.w {
                        let o = out.at_mut(y, x);
                        for ix in 0..kx {
                            let xx = x as isize + ix as isize - *half_x as isize;
                            let col = ix * ky * g.d;
                            if xx < 0 || xx >= g.w as isize {
                                for v in &mut o[col..col + ky * g.d] {
                                    *v = rng.signed(1.0);
                                }
                                continue;
                            }
                            for iy in 0..ky {
                                let yy = y as isize + iy as isize - *half_y as isize;
                                let base = col + iy * g.d;
                                if yy < 0 || yy >= g.h as isize {
                                    for v in &mut o[base..base + g.d] {
                                        *v = rng.signed(1.0);
                                    }
                                } else {
                                    o[base..base + g.d]
                                        .copy_from_slice(g.at(yy as usize, xx as usize));
                                }
                            }
                        }
                    }
                }
                out
            }
            Layer::Maxpool { sx, sy } => {
                let (h, w) = (g.h / sy, g.w / sx);
                let mut out = Grid::new(h, w, g.d);
                for y in 0..h {
                    for x in 0..w {
                        let o = out.at_mut(y, x);
                        o.copy_from_slice(g.at(y * sy, x * sx));
                        for dy in 0..*sy {
                            for dx in 0..*sx {
                                for (a, &b) in o.iter_mut().zip(g.at(y * sy + dy, x * sx + dx)) {
                                    *a = a.max(b);
                                }
                            }
                        }
                    }
                }
                out
            }
            Layer::Reconfig { sx, sy } => {
                let (h, w) = (g.h / sy, g.w / sx);
                let mut out = Grid::new(h, w, g.d * sx * sy);
                for y in 0..h {
                    for x in 0..w {
                        let o = out.at_mut(y, x);
                        let mut k = 0;
                        for dx in 0..*sx {
                            for dy in 0..*sy {
                                o[k..k + g.d].copy_from_slice(g.at(y * sy + dy, x * sx + dx));
                                k += g.d;
                            }
                        }
                    }
                }
                out
            }
            Layer::Full { act, m } => {
                if g.d + 1 != m.cols {
                    return bad("layer input width");
                }
                let mut out = Grid::new(g.h, g.w, m.rows);
                for y in 0..g.h {
                    for x in 0..g.w {
                        let xin = g.at(y, x).to_vec();
                        let o = out.at_mut(y, x);
                        m.apply(&xin, o);
                        act.apply(o);
                    }
                }
                out
            }
            Layer::Lstm {
                gates,
                ni,
                ns,
                summary,
            } => {
                if g.d != *ni {
                    return bad("LSTM input width");
                }
                lstm(&g, gates, *ni, *ns, *summary)
            }
            Layer::Plumbing {
                kind: Plumb::Series,
                layers,
            } => {
                let mut g = g;
                for l in layers {
                    g = l.forward(g, rng)?;
                }
                g
            }
            Layer::Plumbing {
                kind: Plumb::Parallel,
                layers,
            } => {
                let outs: Vec<Grid> = layers
                    .iter()
                    .map(|l| l.forward(g.clone(), rng))
                    .collect::<Result<_, _>>()?;
                let Some(first) = outs.first() else {
                    return bad("empty parallel layer");
                };
                let (h, w) = (first.h, first.w);
                if outs.iter().any(|o| o.h != h || o.w != w) {
                    return bad("parallel layers disagree in size");
                }
                let d: usize = outs.iter().map(|o| o.d).sum();
                let mut out = Grid::new(h, w, d);
                for y in 0..h {
                    for x in 0..w {
                        let mut k = 0;
                        for o in &outs {
                            out.at_mut(y, x)[k..k + o.d].copy_from_slice(o.at(y, x));
                            k += o.d;
                        }
                    }
                }
                out
            }
            Layer::Transpose(l) => l.forward(g.transposed(), rng)?.transposed(),
            Layer::ReverseX(l) => l.forward(g.mirrored_x(), rng)?.mirrored_x(),
            Layer::ReverseY(l) => l.forward(g.mirrored_y(), rng)?.mirrored_y(),
        })
    }

    /// The input height the network wants (its `Input` layer), if fixed.
    pub fn input_height(&self) -> Option<usize> {
        match self {
            Layer::Input { height, .. } => (*height > 0).then_some(*height),
            Layer::Plumbing { layers, .. } => layers.first().and_then(Layer::input_height),
            _ => None,
        }
    }

    /// How many input columns make one output step (the product of the x scales).
    pub fn x_scale(&self) -> usize {
        match self {
            Layer::Maxpool { sx, .. } | Layer::Reconfig { sx, .. } => *sx,
            Layer::Plumbing {
                kind: Plumb::Series,
                layers,
            } => layers.iter().map(Layer::x_scale).product(),
            Layer::Plumbing { layers, .. } => layers.first().map_or(1, Layer::x_scale),
            Layer::ReverseX(l) | Layer::ReverseY(l) => l.x_scale(),
            Layer::Transpose(l) => l.y_scale(),
            _ => 1,
        }
    }

    fn y_scale(&self) -> usize {
        match self {
            Layer::Maxpool { sy, .. } | Layer::Reconfig { sy, .. } => *sy,
            Layer::Plumbing {
                kind: Plumb::Series,
                layers,
            } => layers.iter().map(Layer::y_scale).product(),
            Layer::Plumbing { layers, .. } => layers.first().map_or(1, Layer::y_scale),
            Layer::ReverseX(l) | Layer::ReverseY(l) => l.y_scale(),
            Layer::Transpose(l) => l.x_scale(),
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_and_transposes_are_inverses() {
        let mut g = Grid::new(2, 3, 1);
        for (i, v) in g.v.iter_mut().enumerate() {
            *v = i as f32;
        }
        assert_eq!(g.transposed().transposed(), g);
        assert_eq!(g.mirrored_x().at(0, 0), &[2.0]);
        assert_eq!(g.mirrored_y().at(0, 0), &[3.0]);
    }

    #[test]
    fn maxpool_floors() {
        let mut g = Grid::new(3, 7, 1);
        for (i, v) in g.v.iter_mut().enumerate() {
            *v = i as f32;
        }
        let o = Layer::Maxpool { sx: 3, sy: 3 }
            .forward(g, &mut TRand(1))
            .unwrap();
        assert_eq!((o.h, o.w), (1, 2));
        assert_eq!(o.v, vec![16.0, 19.0]);
    }

    #[test]
    fn lstm_with_zero_weights_outputs_zero() {
        let m = Matrix {
            rows: 2,
            cols: 4,
            w: vec![0.0; 8],
        };
        let l = Layer::Lstm {
            gates: Box::new([m.clone(), m.clone(), m.clone(), m]),
            ni: 1,
            ns: 2,
            summary: true,
        };
        let o = l.forward(Grid::new(2, 5, 1), &mut TRand(1)).unwrap();
        assert_eq!((o.h, o.w, o.d), (2, 1, 2));
        assert!(o.v.iter().all(|&v| v == 0.0));
    }
}
