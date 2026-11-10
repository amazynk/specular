//! The CPU reference: every op in plain Rust, f32 arithmetic (f64 for row statistics).
//!
//! It is the oracle the GPU kernels are tested against and the engine used where there is
//! no trusted GPU, so the matrix products are blocked and threaded. Results do not depend on
//! the thread count: every output element is summed in the same order whatever the split.

use std::borrow::Cow;

use std::simd::{StdFloat, f32x8};

use half::f16;

use crate::graph::{
    Act, BinaryOp, Conv2d, Def, Dft, Graph, MatMul, Op, Pool2d, PoolKind, Preprocess, ReduceOp,
    ResizeMode,
};
use crate::pool::Pool;
use crate::tensor::{Data, Tensor, numel, strides};
use crate::{Error, Feed};

/// Runs `graph` on the CPU with up to `threads` threads; returns the outputs in order.
pub fn run(graph: &Graph, feeds: &[Feed], threads: usize) -> Result<Vec<Tensor>, Error> {
    // The calling thread drives the run; each op's helpers come from the pool (see
    // `par_work`).
    run_graph(graph, feeds, threads.max(1))
}

fn run_graph(graph: &Graph, feeds: &[Feed], threads: usize) -> Result<Vec<Tensor>, Error> {
    if feeds.len() != graph.inputs.len() {
        return Err(Error::Feed(format!(
            "{} inputs fed, the graph has {}",
            feeds.len(),
            graph.inputs.len()
        )));
    }
    // Last node reading each value, so intermediates are dropped as soon as possible.
    let n = graph.values.len();
    let mut last_use = vec![usize::MAX; n];
    for (i, node) in graph.nodes.iter().enumerate() {
        for v in &node.inputs {
            last_use[v.index()] = i;
        }
    }
    let mut keep = vec![false; n];
    for v in &graph.outputs {
        keep[v.index()] = true;
    }
    let mut vals: Vec<Option<Cow<Tensor>>> = vec![None; n];
    let mut images: Vec<Option<Image>> = vec![None; n];
    for (i, info) in graph.values.iter().enumerate() {
        match &info.def {
            Def::Const(t) => vals[i] = Some(Cow::Borrowed(t)),
            Def::Input(k) => match feeds[*k] {
                Feed::Tensor(t) if t.shape == info.shape && t.dtype() == info.dtype => {
                    vals[i] = Some(Cow::Borrowed(t));
                }
                _ => {
                    return Err(Error::Feed(format!(
                        "input {k} wants a {:?} tensor {:?}",
                        info.dtype, info.shape
                    )));
                }
            },
            Def::Image(k) => match feeds[*k] {
                Feed::Rgba {
                    width,
                    height,
                    data,
                } => {
                    let (w, h) = (width as usize, height as usize);
                    if w == 0 || h == 0 || Some(data.len()) != w.checked_mul(h * 4) {
                        return Err(Error::Feed(format!("image {k}: bad size")));
                    }
                    images[i] = Some(Image { w, h, data });
                }
                _ => return Err(Error::Feed(format!("input {k} wants an RGBA image"))),
            },
            Def::Node(_) => {}
        }
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        let out_shape = &graph.values[node.output.index()].shape;
        let get = |k: usize| -> &Tensor { vals[node.inputs[k].index()].as_deref().expect("value") };
        let t = match &node.op {
            Op::Preprocess(p) => {
                let img = images[node.inputs[0].index()].as_ref().expect("image");
                preprocess(img, p)
            }
            Op::Reshape => get(0).clone().reshaped(out_shape),
            op => {
                let ins: Vec<&Tensor> = (0..node.inputs.len()).map(get).collect();
                eval(op, &ins, out_shape, threads)
            }
        };
        debug_assert_eq!(&t.shape, out_shape);
        vals[node.output.index()] = Some(Cow::Owned(t));
        for v in &node.inputs {
            if last_use[v.index()] == i && !keep[v.index()] {
                vals[v.index()] = None;
            }
        }
    }
    Ok(graph
        .outputs
        .iter()
        .map(|v| vals[v.index()].as_deref().expect("output").clone())
        .collect())
}

#[derive(Clone, Copy)]
struct Image<'a> {
    w: usize,
    h: usize,
    data: &'a [u8],
}

/// One op on host tensors (not Reshape or Preprocess).
pub fn eval(op: &Op, ins: &[&Tensor], out_shape: &[usize], threads: usize) -> Tensor {
    let out = |v: Vec<f32>| Tensor::f32(out_shape, v);
    match op {
        Op::Conv2d(p) => out(conv2d(
            ins[0],
            ins[1],
            ins.get(2).copied(),
            p,
            out_shape,
            threads,
        )),
        Op::MatMul(p) => out(matmul(
            ins[0],
            ins[1],
            ins.get(2).copied(),
            p,
            out_shape,
            threads,
        )),
        Op::Binary(b) => out(binary(*b, ins[0], ins[1], out_shape)),
        Op::Unary { act, mul, add } => out(ins[0]
            .as_f32()
            .iter()
            .map(|&x| act.apply(x * mul + add))
            .collect()),
        Op::LayerNorm { eps } => out(layer_norm(ins[0], ins[1], ins[2], *eps, threads)),
        Op::L2Norm { eps } => out(l2_norm(ins[0], *eps)),
        Op::ChannelNorm { eps } => out(channel_norm(ins[0], ins[1], ins[2], *eps, threads)),
        Op::GroupNorm { groups, eps } => out(group_norm(ins, *groups, *eps, threads)),
        Op::Gate => out(gate(ins[0])),
        Op::DepthToSpace { block } => {
            let (dims, src) = depth_to_space_strides(&ins[0].shape, *block);
            Tensor::f32(out_shape, copy_out(ins[0], &dims, &src, 0).to_f32())
        }
        Op::ConvTranspose2d { stride, pad, act } => out(conv_transpose2d(
            ins[0],
            ins[1],
            ins.get(2).copied(),
            (*stride, *pad, *act),
            out_shape,
            threads,
        )),
        Op::Softmax => {
            let x = ins[0].as_f32();
            let d = *out_shape.last().expect("rank");
            let mut v = x.into_owned();
            for row in v.chunks_mut(d) {
                softmax_row(row, 1.0, d);
            }
            out(v)
        }
        Op::Attention {
            heads,
            scale,
            causal,
            packed,
        } => out(attention(ins, *heads, *scale, *causal, *packed, threads)),
        Op::Pool2d(p) => out(pool2d(ins[0], p, out_shape)),
        Op::Reduce { op, axis } => out(reduce(ins[0], *op, *axis)),
        Op::Resize { mode, .. } => out(resize(ins[0], *mode, out_shape)),
        Op::Transpose { perm } => {
            let s = &ins[0].shape;
            let st = strides(s);
            let src_strides: Vec<usize> = perm.iter().map(|&p| st[p]).collect();
            copy_out(ins[0], out_shape, &src_strides, 0)
        }
        Op::Slice { starts, steps, .. } => {
            let st = strides(&ins[0].shape);
            let base = starts.iter().zip(&st).map(|(a, b)| a * b).sum();
            let src_strides: Vec<usize> = st.iter().zip(steps).map(|(a, b)| a * b).collect();
            copy_out(ins[0], out_shape, &src_strides, base)
        }
        Op::Concat { axis } => concat(ins, *axis, out_shape),
        Op::Gather => out(gather(ins[0], ins[1])),
        Op::Lstm {
            reverse,
            summary,
            per,
        } => out(lstm(ins, *reverse, *summary, *per, threads)),
        Op::CastF16 => Tensor::f16_bits(
            out_shape,
            ins[0]
                .as_f32()
                .iter()
                .map(|&x| f16::from_f32(x).to_bits())
                .collect(),
        ),
        Op::ReflectPad { pads } => out(reflect_pad(ins[0], *pads, out_shape)),
        Op::Dft(k) => out(dft(ins[0], *k, out_shape)),
        Op::Reshape | Op::Preprocess(_) => unreachable!("handled by run"),
    }
}

/// Mirror padding: output pixel `i` reads input `i − pad` reflected about the edges.
fn reflect_pad(x: &Tensor, pads: [usize; 4], out_shape: &[usize]) -> Vec<f32> {
    let (h, w) = (x.shape[2], x.shape[3]);
    let (oh, ow) = (out_shape[2], out_shape[3]);
    let refl = |i: usize, p: usize, n: usize| {
        let s = i as isize - p as isize;
        let s = if s < 0 { -s } else { s };
        let s = if s >= n as isize {
            2 * (n as isize - 1) - s
        } else {
            s
        };
        s as usize
    };
    let xv = x.as_f32();
    let mut v = Vec::with_capacity(numel(out_shape).unwrap_or(0));
    for plane in xv.chunks(h * w) {
        for oy in 0..oh {
            let iy = refl(oy, pads[0], h);
            for ox in 0..ow {
                v.push(plane[iy * w + refl(ox, pads[1], w)]);
            }
        }
    }
    v
}

/// e^{sign·2πi·k/n} for `k` taken modulo `n`, in f64.
fn twiddle(k: usize, n: usize, sign: f64) -> (f64, f64) {
    let a = std::f64::consts::TAU * (k % n) as f64 / n as f64;
    (a.cos(), sign * a.sin())
}

/// The reference transforms: direct sums in f64 (the GPU sums the same terms in f32).
fn dft(x: &Tensor, kind: Dft, out_shape: &[usize]) -> Vec<f32> {
    let xv = x.as_f32();
    let s = &x.shape;
    let r = s.len();
    let mut v = vec![0f32; numel(out_shape).unwrap_or(0)];
    match kind {
        Dft::RealToComplex => {
            let n = s[r - 1];
            let k = n / 2 + 1;
            let norm = 1.0 / (n as f64).sqrt();
            for (row, dst) in xv.chunks(n).zip(v.chunks_mut(2 * k)) {
                for b in 0..k {
                    let (mut re, mut im) = (0f64, 0f64);
                    for (t, &a) in row.iter().enumerate() {
                        let (c, sn) = twiddle(b * t, n, -1.0);
                        re += a as f64 * c;
                        im += a as f64 * sn;
                    }
                    dst[2 * b] = (re * norm) as f32;
                    dst[2 * b + 1] = (im * norm) as f32;
                }
            }
        }
        Dft::Complex { inverse } => {
            let (n, m) = (s[r - 3], s[r - 2]);
            let sign = if inverse { 1.0 } else { -1.0 };
            let norm = 1.0 / (n as f64).sqrt();
            for (blk, dst) in xv.chunks(2 * n * m).zip(v.chunks_mut(2 * n * m)) {
                for b in 0..n {
                    for j in 0..m {
                        let (mut re, mut im) = (0f64, 0f64);
                        for t in 0..n {
                            let (c, sn) = twiddle(b * t, n, sign);
                            let (xr, xi) =
                                (blk[2 * (t * m + j)] as f64, blk[2 * (t * m + j) + 1] as f64);
                            re += xr * c - xi * sn;
                            im += xr * sn + xi * c;
                        }
                        dst[2 * (b * m + j)] = (re * norm) as f32;
                        dst[2 * (b * m + j) + 1] = (im * norm) as f32;
                    }
                }
            }
        }
        Dft::ComplexToReal { n } => {
            let k = n / 2 + 1;
            let norm = 1.0 / (n as f64).sqrt();
            for (row, dst) in xv.chunks(2 * k).zip(v.chunks_mut(n)) {
                for (t, o) in dst.iter_mut().enumerate() {
                    let mut acc = row[0] as f64;
                    for b in 1..k {
                        let (c, sn) = twiddle(b * t, n, 1.0);
                        let term = row[2 * b] as f64 * c - row[2 * b + 1] as f64 * sn;
                        // The Nyquist bin of an even length appears once, every other bin
                        // twice (with its conjugate).
                        acc += if 2 * b == n {
                            row[2 * b] as f64 * c
                        } else {
                            2.0 * term
                        };
                    }
                    *o = (acc * norm) as f32;
                }
            }
        }
    }
    v
}

// ---- parallel helper --------------------------------------------------------------------

/// Calls `f(i, chunk)` for every `chunk`-sized piece of `out` on up to `threads` threads
/// (each thread takes the next piece when done with one).
fn par_chunks<F>(out: &mut [f32], chunk: usize, threads: usize, f: F)
where
    F: Fn(usize, &mut [f32]) + Sync,
{
    let chunk = chunk.max(1);
    let pieces = out.len().div_ceil(chunk);
    let threads = threads.min(pieces).max(1);
    if threads == 1 {
        for (i, c) in out.chunks_mut(chunk).enumerate() {
            f(i, c);
        }
        return;
    }
    par_work(
        out.chunks_mut(chunk).enumerate().collect(),
        threads,
        |(i, c)| f(i, c),
    );
}

/// Runs `f` on every item of `work` on the calling thread and up to `threads - 1` pool
/// workers ([`Pool::global`]), each taking the next item when it is done with one (a busy
/// machine can stall any thread). Workers that had not joined by the time the calling
/// thread ran out of items are not waited for, so a thread the OS has not scheduled yet
/// delays nothing.
pub(crate) fn par_work<T: Send>(mut work: Vec<T>, threads: usize, f: impl Fn(T) + Sync) {
    let threads = threads.min(work.len()).max(1);
    if threads == 1 {
        work.into_iter().for_each(f);
        return;
    }
    work.reverse();
    let queue = std::sync::Mutex::new(work);
    let next = || queue.lock().unwrap_or_else(|e| e.into_inner()).pop();
    Pool::global().run(threads - 1, &|| {
        while let Some(w) = next() {
            f(w);
        }
    });
}

// ---- GEMM ---------------------------------------------------------------------------------
//
// B is packed into panels of NR columns ([k][NR], zero-padded), then a 6×16 register-blocked
// micro-kernel (12 accumulators of 8 lanes, FMA) walks rows of A against one panel, K in
// chunks of KC so the panel chunk stays in L1. Each output element is one FMA chain over k
// in order whatever the blocking or the thread split, so results do not depend on `threads`.

type V = f32x8;
const LANES: usize = 8;
/// Rows of the micro-kernel.
const MR: usize = 6;
/// Columns of the micro-kernel (one packed panel).
const NR: usize = 2 * LANES;
/// K chunk: one panel chunk is KC·NR·4 = 16 KiB.
const KC: usize = 256;

/// One matrix product of a batched GEMM: where its A starts, which packed B it uses, where
/// its C starts and (per-row bias) which bias its first row adds.
#[derive(Clone, Copy)]
struct Job {
    a: usize,
    b: usize,
    c: usize,
    bias: usize,
}

/// What a GEMM does to each finished sum x: act(x + bias), the bias per row (a
/// convolution's output channel) or per column (a matmul's output feature), 0 without one.
/// Applied as the micro-kernel stores its last K chunk, on whichever thread did the piece.
#[derive(Clone, Copy)]
struct Epilogue<'a> {
    bias: Option<&'a [f32]>,
    per_row: bool,
    act: Act,
}

impl Epilogue<'_> {
    const NONE: Epilogue<'static> = Epilogue {
        bias: None,
        per_row: false,
        act: Act::Identity,
    };

    fn is_none(&self) -> bool {
        self.bias.is_none() && self.act == Act::Identity
    }
}

/// `t[i] = act(t[i] + bias(i))` (the matching branch hoisted out of the loop).
#[inline(always)]
fn finish(t: &mut [f32], bias: impl Fn(usize) -> f32, act: Act) {
    match act {
        Act::Identity => t.iter_mut().enumerate().for_each(|(i, x)| *x += bias(i)),
        Act::Relu => t
            .iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = Act::Relu.apply(*x + bias(i))),
        Act::Relu6 => t
            .iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = Act::Relu6.apply(*x + bias(i))),
        act => t
            .iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = act.apply(*x + bias(i))),
    }
}

/// Elements below which packing stays on the calling thread.
const PAR_PACK: usize = 1 << 16;

/// Multiply-adds per thread below which a GEMM uses fewer threads: starting a thread and
/// handing it work costs about as much as this many multiply-adds.
const MACS_PER_THREAD: usize = 1 << 20;

/// Threads worth starting for `macs` multiply-adds.
fn gemm_threads(macs: usize, threads: usize) -> usize {
    threads.min(macs.div_ceil(MACS_PER_THREAD)).max(1)
}

/// B as the micro-kernel reads it: packed NR-column panels, or (row-major B, read in
/// place) full panels straight from `b` with only the last partial panel packed.
enum PackedB<'a> {
    Packed(Vec<f32>),
    Direct {
        b: &'a [f32],
        off: usize,
        ld: usize,
        n: usize,
        /// The last partial panel (k × NR), if n is not a multiple of NR.
        tail: Vec<f32>,
    },
}

impl PackedB<'_> {
    /// Panel `p` from row `k0` on, and its row stride.
    fn panel(&self, p: usize, k0: usize, k: usize) -> (&[f32], usize) {
        match self {
            PackedB::Packed(v) => (&v[p * k * NR + k0 * NR..], NR),
            PackedB::Direct {
                b,
                off,
                ld,
                n,
                tail,
            } => {
                if (p + 1) * NR <= *n {
                    (&b[off + k0 * ld + p * NR..], *ld)
                } else {
                    (&tail[k0 * NR..], NR)
                }
            }
        }
    }
}

/// Rows of A below which a row-major B is read in place rather than packed: packing costs
/// a pass over B, which only pays when many row blocks reuse the panels.
const DIRECT_M: usize = 96;

/// B(kk, j) = b[off + kk·bk + j·bn] (k × n) for an m-row product: read in place when it is
/// row-major and A is short, else packed.
#[allow(clippy::too_many_arguments)]
fn prepare_b(
    b: &[f32],
    off: usize,
    bk: usize,
    bn: usize,
    k: usize,
    n: usize,
    m: usize,
    threads: usize,
) -> PackedB<'_> {
    if bn == 1 && m <= DIRECT_M && n >= NR && k > 0 {
        let full = n / NR * NR;
        let tail = if full < n {
            pack_b(b, off + full, bk, 1, k, n - full, 1)
        } else {
            Vec::new()
        };
        return PackedB::Direct {
            b,
            off,
            ld: bk,
            n,
            tail,
        };
    }
    PackedB::Packed(pack_b(b, off, bk, bn, k, n, threads))
}

/// Packs B(kk, j) = b[off + kk·bk + j·bn] (k × n) into NR-column panels.
fn pack_b(
    b: &[f32],
    off: usize,
    bk: usize,
    bn: usize,
    k: usize,
    n: usize,
    threads: usize,
) -> Vec<f32> {
    let panels = n.div_ceil(NR);
    let mut out = vec![0f32; panels * k * NR];
    if k == 0 {
        return out;
    }
    let threads = if k * n < PAR_PACK { 1 } else { threads };
    par_chunks(&mut out, k * NR, threads, |p, panel| {
        let j0 = p * NR;
        let w = NR.min(n - j0);
        if bn == 1 {
            for (kk, dst) in panel.chunks_mut(NR).enumerate() {
                let s = off + kk * bk + j0;
                dst[..w].copy_from_slice(&b[s..s + w]);
            }
        } else {
            for j in 0..w {
                let s = off + (j0 + j) * bn;
                for kk in 0..k {
                    panel[kk * NR + j] = b[s + kk * bk];
                }
            }
        }
    });
    out
}

/// The im2col matrix of one image's channel group (`cg` channels of h × wd from `xin`),
/// [cg·kh·kw, oh·ow], straight into packed panels (as `pack_b` would pack it).
fn pack_im2col(
    xin: &[f32],
    (cg, h, wd): (usize, usize, usize),
    (kh, kw): (usize, usize),
    p: &Conv2d,
    (oh, ow): (usize, usize),
    threads: usize,
) -> Vec<f32> {
    let (k, n) = (cg * kh * kw, oh * ow);
    let mut out = vec![0f32; n.div_ceil(NR) * k * NR];
    if k == 0 {
        return out;
    }
    let threads = if k * n < PAR_PACK { 1 } else { threads };
    par_chunks(&mut out, k * NR, threads, |pi, panel| {
        let j0 = pi * NR;
        for j in 0..NR.min(n - j0) {
            let (oy, ox) = ((j0 + j) / ow, (j0 + j) % ow);
            for ky in 0..kh {
                let iy = (oy * p.stride[0] + ky * p.dilation[0]) as isize - p.pad[0] as isize;
                if iy < 0 || iy >= h as isize {
                    continue;
                }
                for kx in 0..kw {
                    let ix = (ox * p.stride[1] + kx * p.dilation[1]) as isize - p.pad[1] as isize;
                    if ix < 0 || ix >= wd as isize {
                        continue;
                    }
                    let src = iy as usize * wd + ix as usize;
                    for ci in 0..cg {
                        panel[((ci * kh + ky) * kw + kx) * NR + j] = xin[ci * h * wd + src];
                    }
                }
            }
        }
    });
    out
}

/// The im2col matrix of one image's channel group (`cg` channels of h × wd from `xin`),
/// [cg·kh·kw, oh·ow] row-major, 0 for taps outside the input. One row per tap: input row
/// segments copied (stride 1) or gathered, which costs far less than packing it.
fn im2col(
    xin: &[f32],
    (cg, h, wd): (usize, usize, usize),
    (kh, kw): (usize, usize),
    p: &Conv2d,
    (oh, ow): (usize, usize),
    threads: usize,
) -> Vec<f32> {
    let (k, n) = (cg * kh * kw, oh * ow);
    let mut out = vec![0f32; k * n];
    if n == 0 {
        return out;
    }
    let (sx, dx, px) = (p.stride[1], p.dilation[1], p.pad[1]);
    let threads = if k * n < PAR_PACK { 1 } else { threads };
    par_chunks(&mut out, n, threads, |r, row| {
        let (ci, ky, kx) = (r / (kh * kw), r / kw % kh, r % kw);
        let off = kx * dx;
        // The output columns whose input column is inside the row: ox·sx + off ≥ px and
        // ox·sx + off − px < wd.
        let lo = px.saturating_sub(off).div_ceil(sx).min(ow);
        let hi = (wd + px).saturating_sub(off).div_ceil(sx).clamp(lo, ow);
        if lo == hi {
            return;
        }
        let i0 = lo * sx + off - px;
        let plane = &xin[ci * h * wd..(ci + 1) * h * wd];
        for (oy, orow) in row.chunks_mut(ow).enumerate() {
            let iy = (oy * p.stride[0] + ky * p.dilation[0]) as isize - p.pad[0] as isize;
            if iy < 0 || iy >= h as isize {
                continue;
            }
            let srow = &plane[iy as usize * wd..(iy as usize + 1) * wd];
            let dst = &mut orow[lo..hi];
            if sx == 1 {
                dst.copy_from_slice(&srow[i0..i0 + (hi - lo)]);
            } else {
                for (d, &s) in dst.iter_mut().zip(srow[i0..].iter().step_by(sx)) {
                    *d = s;
                }
            }
        }
    });
    out
}

/// Where the micro-kernel's sums go: C rows (one slice each, columns `cj..cj + w`), whether
/// they start from zero or from what C holds (an earlier K chunk), and on the last K chunk
/// the epilogue with the bias index of the first row and column.
struct Out<'s, 'c> {
    c: &'s mut [&'c mut [f32]],
    cj: usize,
    w: usize,
    first: bool,
    last: Option<(&'s Epilogue<'s>, usize, usize)>,
}

/// `o.c` rows = (or += unless `o.first`) A rows at `arow` (`kn` values each) · the panel
/// chunk `bp` (`kn` rows of NR, `ldb` apart), for R ≤ MR rows.
#[inline(always)]
fn micro<const R: usize>(a: &[f32], arow: [usize; R], bp: &[f32], ldb: usize, kn: usize, o: Out) {
    let Out {
        c,
        cj,
        w,
        first,
        last,
    } = o;
    let mut acc = [[V::splat(0.0); 2]; R];
    if !first {
        for (r, acc) in acc.iter_mut().enumerate() {
            if w == NR {
                let s = &c[r][cj..cj + NR];
                *acc = [V::from_slice(&s[..LANES]), V::from_slice(&s[LANES..])];
            } else {
                let mut t = [0f32; NR];
                t[..w].copy_from_slice(&c[r][cj..cj + w]);
                *acc = [V::from_slice(&t[..LANES]), V::from_slice(&t[LANES..])];
            }
        }
    }
    let ar: [&[f32]; R] = std::array::from_fn(|r| &a[arow[r]..arow[r] + kn]);
    for kk in 0..kn {
        let b = &bp[kk * ldb..kk * ldb + NR];
        let b0 = V::from_slice(&b[..LANES]);
        let b1 = V::from_slice(&b[LANES..]);
        for r in 0..R {
            let av = V::splat(ar[r][kk]);
            acc[r][0] = av.mul_add(b0, acc[r][0]);
            acc[r][1] = av.mul_add(b1, acc[r][1]);
        }
    }
    for (r, acc) in acc.iter().enumerate() {
        let mut t = [0f32; NR];
        acc[0].copy_to_slice(&mut t[..LANES]);
        acc[1].copy_to_slice(&mut t[LANES..]);
        if let Some((e, br, bc)) = last {
            match (e.bias, e.per_row) {
                (None, _) => finish(&mut t, |_| 0.0, e.act),
                (Some(b), true) => {
                    let v = b[br + r];
                    finish(&mut t, |_| v, e.act);
                }
                (Some(b), false) => {
                    let mut v = [0f32; NR];
                    v[..w].copy_from_slice(&b[bc..bc + w]);
                    finish(&mut t, |i| v[i], e.act);
                }
            }
        }
        if w == NR {
            c[r][cj..cj + NR].copy_from_slice(&t);
        } else {
            c[r][cj..cj + w].copy_from_slice(&t[..w]);
        }
    }
}

/// Offsets of R rows `lda` apart from `at`.
fn rows_at<const R: usize>(at: usize, lda: usize) -> [usize; R] {
    std::array::from_fn(|i| at + i * lda)
}

/// One piece of a GEMM: rows `r0..` of `job`'s C (one slice per row, covering columns
/// `j0..j0 + len`), A rows `lda` apart, packed B `bp`, then the epilogue.
#[allow(clippy::too_many_arguments)]
fn gemm_piece(
    a: &[f32],
    job: Job,
    lda: usize,
    r0: usize,
    bp: &PackedB,
    k: usize,
    j0: usize,
    c: &mut [&mut [f32]],
    epi: &Epilogue,
) {
    let rows = c.len();
    let cols = c.first().map_or(0, |r| r.len());
    let mut kc0 = 0;
    while kc0 < k {
        let kn = KC.min(k - kc0);
        let last_chunk = kc0 + kn == k && !epi.is_none();
        for pj in (0..cols).step_by(NR) {
            let w = NR.min(cols - pj);
            let (panel, ldb) = bp.panel((j0 + pj) / NR, kc0, k);
            let mut r = 0;
            while r < rows {
                let mr = MR.min(rows - r);
                let at = job.a + (r0 + r) * lda + kc0;
                let o = Out {
                    c: &mut c[r..r + mr],
                    cj: pj,
                    w,
                    first: kc0 == 0,
                    last: last_chunk.then_some((epi, job.bias + r0 + r, j0 + pj)),
                };
                match mr {
                    6 => micro::<6>(a, rows_at(at, lda), panel, ldb, kn, o),
                    5 => micro::<5>(a, rows_at(at, lda), panel, ldb, kn, o),
                    4 => micro::<4>(a, rows_at(at, lda), panel, ldb, kn, o),
                    3 => micro::<3>(a, rows_at(at, lda), panel, ldb, kn, o),
                    2 => micro::<2>(a, rows_at(at, lda), panel, ldb, kn, o),
                    _ => micro::<1>(a, rows_at(at, lda), panel, ldb, kn, o),
                }
                r += mr;
            }
        }
        kc0 += kn;
    }
}

/// A piece of work: its job, first row and column, and its C rows (column segments).
type Piece<'a> = (Job, usize, usize, Vec<&'a mut [f32]>);

/// Batched GEMM: per job, C[m, n] (`c[job.c..]`, rows n apart, jobs not overlapping) =
/// epi(A[m, k] (`a[job.a..]`, rows `lda` apart) · packed B `packed[job.b]`). Pieces of
/// rows × columns are shared out to `threads` threads.
#[allow(clippy::too_many_arguments)]
fn gemm_jobs(
    a: &[f32],
    lda: usize,
    packed: &[PackedB],
    jobs: &[Job],
    c: &mut [f32],
    (m, k, n): (usize, usize, usize),
    epi: &Epilogue,
    threads: usize,
) {
    if m == 0 || n == 0 {
        return;
    }
    if k == 0 {
        for j in jobs {
            for (r, row) in c[j.c..j.c + m * n].chunks_mut(n).enumerate() {
                for (i, x) in row.iter_mut().enumerate() {
                    let b = epi
                        .bias
                        .map_or(0.0, |b| b[if epi.per_row { j.bias + r } else { i }]);
                    *x = epi.act.apply(0.0 + b);
                }
            }
        }
        return;
    }
    let threads = gemm_threads(jobs.len() * m * k * n, threads);
    // Piece size: halve the larger side until there are a few pieces per thread.
    let (mut mc, mut nc) = (m.next_multiple_of(MR), n.next_multiple_of(NR));
    let pieces = |mc: usize, nc: usize| jobs.len() * m.div_ceil(mc) * n.div_ceil(nc);
    while pieces(mc, nc) < 3 * threads && (mc > 4 * MR || nc > 4 * NR) {
        if mc >= nc / 4 && mc > 4 * MR {
            mc = (mc / 2).next_multiple_of(MR);
        } else {
            nc = (nc / 2).next_multiple_of(NR);
        }
    }
    let per_row = n.div_ceil(nc);
    let mut order: Vec<Job> = jobs.to_vec();
    order.sort_by_key(|j| j.c);
    let mut work: Vec<Piece> = Vec::new();
    let mut rest: &mut [f32] = c;
    let mut at = 0;
    for job in order {
        let (_, tail) = std::mem::take(&mut rest).split_at_mut(job.c - at);
        let (mine, tail) = tail.split_at_mut(m * n);
        rest = tail;
        at = job.c + m * n;
        let first = work.len();
        for r0 in (0..m).step_by(mc) {
            for j0 in (0..n).step_by(nc) {
                work.push((job, r0, j0, Vec::new()));
            }
        }
        for (r, mut row) in mine.chunks_mut(n).enumerate() {
            let base = first + (r / mc) * per_row;
            for p in 0..per_row {
                let (seg, tail) = row.split_at_mut(nc.min(row.len()));
                row = tail;
                work[base + p].3.push(seg);
            }
        }
    }
    let run = |(job, r0, j0, mut rows): Piece| {
        gemm_piece(a, job, lda, r0, &packed[job.b], k, j0, &mut rows, epi);
    };
    par_work(work, threads, run);
}

/// C[m, n] (rows n apart) = A[m, k] (rows `lda` apart from `a0`) · B with
/// B(kk, j) = b[b0 + kk·bk + j·bn].
#[allow(clippy::too_many_arguments)]
fn gemm(
    a: &[f32],
    a0: usize,
    lda: usize,
    b: &[f32],
    (b0, bk, bn): (usize, usize, usize),
    c: &mut [f32],
    (m, k, n): (usize, usize, usize),
    threads: usize,
) {
    let threads = gemm_threads(m * k * n, threads);
    let packed = [prepare_b(b, b0, bk, bn, k, n, m, threads)];
    let job = Job {
        a: a0,
        b: 0,
        c: 0,
        bias: 0,
    };
    gemm_jobs(
        a,
        lda,
        &packed,
        &[job],
        c,
        (m, k, n),
        &Epilogue::NONE,
        threads,
    );
}

/// Broadcast batch index → element offset of an operand's batch.
fn batch_offset(out_batch: &[usize], op_batch: &[usize], bi: usize, mat: usize) -> usize {
    // Left-pad the operand's batch shape.
    let r = out_batch.len();
    let pad = r - op_batch.len();
    let mut rem = bi;
    let mut off = 0;
    let mut stride = mat;
    let mut idx = vec![0; r];
    for d in (0..r).rev() {
        idx[d] = rem % out_batch[d];
        rem /= out_batch[d];
    }
    for d in (pad..r).rev() {
        let od = op_batch[d - pad];
        if od != 1 {
            off += idx[d] * stride;
        }
        stride *= od;
    }
    off
}

fn matmul(
    a: &Tensor,
    b: &Tensor,
    bias: Option<&Tensor>,
    p: &MatMul,
    out_shape: &[usize],
    threads: usize,
) -> Vec<f32> {
    let (sa, sb) = (&a.shape, &b.shape);
    let r = out_shape.len();
    let (m, n) = (out_shape[r - 2], out_shape[r - 1]);
    let k = sa[sa.len() - 1];
    let out_batch = &out_shape[..r - 2];
    let batch: usize = out_batch.iter().product();
    let ab = &sa[..sa.len() - 2];
    let bb = &sb[..sb.len() - 2];
    let af = a.as_f32();
    let bias = bias.map(|t| t.as_f32());
    let epi = Epilogue {
        bias: bias.as_deref(),
        per_row: false,
        act: p.act,
    };
    if p.trans_b && batch == 1 && m <= GEMV_M {
        // A few rows against a weight matrix: dot products with its rows in place (f16
        // ones converted as they are read).
        let mut c = vec![0f32; m * n];
        match &b.data {
            Data::F32(w) => gemv_t(&af, w, |x| x, &mut c, (m, k, n), &epi, threads),
            Data::F16(w) => gemv_t(
                &af,
                w,
                |x| f16::from_bits(x).to_f32(),
                &mut c,
                (m, k, n),
                &epi,
                threads,
            ),
        }
        return c;
    }
    let bf = b.as_f32();
    let a_batches = ab.iter().product::<usize>();
    let b_batches = bb.iter().product::<usize>();
    // B packed once per distinct batch it has.
    let (bk, bn) = if p.trans_b { (1, k) } else { (n, 1) };
    let threads = gemm_threads(batch * m * k * n, threads);
    let packed: Vec<PackedB> = (0..b_batches)
        .map(|i| prepare_b(&bf, i * k * n, bk, bn, k, n, m, threads))
        .collect();
    let jobs: Vec<Job> = (0..batch)
        .map(|bi| Job {
            a: if a_batches > 1 {
                batch_offset(out_batch, ab, bi, m * k)
            } else {
                0
            },
            b: if b_batches > 1 {
                batch_offset(out_batch, bb, bi, k * n) / (k * n).max(1)
            } else {
                0
            },
            c: bi * m * n,
            bias: 0,
        })
        .collect();
    let mut c = vec![0f32; batch * m * n];
    gemm_jobs(&af, k, &packed, &jobs, &mut c, (m, k, n), &epi, threads);
    c
}

/// Rows of A up to which a product with a transposed weight matrix is done as dot
/// products (packing the weights would cost more than the product).
const GEMV_M: usize = 4;

/// Output columns per piece of `gemv_t`.
const GEMV_COLS: usize = 8 * LANES;

/// C[m, n] = epi(A[m, k] · Wᵀ), W[n, k] row-major (`cvt` reads one element): LANES outputs
/// at a time, each one FMA chain over k in order as the GEMM computes it.
fn gemv_t<W: Copy + Sync>(
    a: &[f32],
    w: &[W],
    cvt: impl Fn(W) -> f32 + Sync,
    c: &mut [f32],
    (m, k, n): (usize, usize, usize),
    epi: &Epilogue,
    threads: usize,
) {
    if m == 0 || n == 0 {
        return;
    }
    let threads = gemm_threads(m * k * n, threads);
    let work: Vec<(usize, usize, &mut [f32])> = c
        .chunks_mut(n)
        .enumerate()
        .flat_map(|(i, row)| {
            row.chunks_mut(GEMV_COLS)
                .enumerate()
                .map(move |(p, seg)| (i, p * GEMV_COLS, seg))
        })
        .collect();
    par_work(work, threads, |(i, j0, seg)| {
        let ar = &a[i * k..(i + 1) * k];
        for (q, out) in seg.chunks_mut(LANES).enumerate() {
            let j = j0 + q * LANES;
            // Rows past n repeat the last one; their sums are dropped.
            let rows: [&[W]; LANES] = std::array::from_fn(|t| {
                let r = (j + t).min(n - 1);
                &w[r * k..(r + 1) * k]
            });
            let mut acc = V::splat(0.0);
            for (kk, &x) in ar.iter().enumerate() {
                let wv = V::from_array(std::array::from_fn(|t| cvt(rows[t][kk])));
                acc = V::splat(x).mul_add(wv, acc);
            }
            let mut t = [0f32; LANES];
            acc.copy_to_slice(&mut t);
            if !epi.is_none() {
                match epi.bias {
                    Some(b) => finish(&mut t, |l| b[(j + l).min(n - 1)], epi.act),
                    None => finish(&mut t, |_| 0.0, epi.act),
                }
            }
            out.copy_from_slice(&t[..out.len()]);
        }
    });
}

// ---- convolution ----------------------------------------------------------------------------

/// Depthwise taps per piece of work (whole channels; small ones are grouped).
const DW_PIECE: usize = 1 << 14;

/// Depthwise taps per thread below which a convolution uses fewer threads (a tap costs a
/// few GEMM multiply-adds: the loops are short and stride 2 gathers).
const DW_PER_THREAD: usize = 1 << 18;

fn conv2d(
    x: &Tensor,
    w: &Tensor,
    bias: Option<&Tensor>,
    p: &Conv2d,
    out_shape: &[usize],
    threads: usize,
) -> Vec<f32> {
    let [n, c, h, wd] = x.shape[..] else {
        unreachable!()
    };
    let [o, cg, kh, kw] = w.shape[..] else {
        unreachable!()
    };
    let [_, _, oh, ow] = out_shape[..] else {
        unreachable!()
    };
    let g = p.groups;
    let og = o / g;
    let xf = x.as_f32();
    let wf = w.as_f32();
    let bias = bias.map(|b| b.to_f32());
    let kk = cg * kh * kw;
    let pix = oh * ow;
    let mut out = vec![0f32; n * o * pix];
    let pointwise =
        kh == 1 && kw == 1 && p.stride == [1, 1] && p.pad == [0; 4] && p.dilation == [1, 1];
    if g == c && cg == 1 && og == 1 && !pointwise {
        // Depthwise: direct, a few channels per piece.
        let (sx, dx, px) = (p.stride[1], p.dilation[1], p.pad[1]);
        // Per tap column: the output columns whose input column is inside the row.
        let cols: Vec<(usize, usize)> = (0..kw)
            .map(|kx| {
                let off = kx * dx;
                // ox·sx + off ≥ px and ox·sx + off − px < wd.
                let lo = px.saturating_sub(off).div_ceil(sx).min(ow);
                let hi = (wd + px).saturating_sub(off).div_ceil(sx).clamp(lo, ow);
                (lo, hi)
            })
            .collect();
        let channel = |i: usize, oc: &mut [f32]| {
            let (img, ch) = (i / o, i % o);
            let src = &xf[(img * c + ch) * h * wd..(img * c + ch + 1) * h * wd];
            let wk = &wf[ch * kh * kw..(ch + 1) * kh * kw];
            let b = bias.as_ref().map_or(0.0, |b| b[ch]);
            for (oy, orow) in oc.chunks_mut(ow).enumerate() {
                // Every output sums its taps in (ky, kx) order, skipping those outside.
                orow.fill(0.0);
                for ky in 0..kh {
                    let iy = (oy * p.stride[0] + ky * p.dilation[0]) as isize - p.pad[0] as isize;
                    if iy < 0 || iy >= h as isize {
                        continue;
                    }
                    let srow = &src[iy as usize * wd..(iy as usize + 1) * wd];
                    for (kx, &(lo, hi)) in cols.iter().enumerate() {
                        let wv = wk[ky * kw + kx];
                        if lo == hi {
                            continue;
                        }
                        // Input column of output `lo`.
                        let i0 = lo * sx + kx * dx - px;
                        let out = &mut orow[lo..hi];
                        if sx == 1 {
                            for (o, &x) in out.iter_mut().zip(&srow[i0..i0 + (hi - lo)]) {
                                *o += x * wv;
                            }
                        } else {
                            for (o, &x) in out.iter_mut().zip(srow[i0..].iter().step_by(sx)) {
                                *o += x * wv;
                            }
                        }
                    }
                }
                for o in orow {
                    *o = p.act.apply(*o + b);
                }
            }
        };
        // Pieces of whole channels with at least DW_PIECE taps, on threads with at least
        // DW_PER_THREAD each.
        let taps = pix * kh * kw;
        let per = DW_PIECE.div_ceil(taps.max(1));
        let threads = threads.min((n * o * taps).div_ceil(DW_PER_THREAD)).max(1);
        par_chunks(&mut out, per * pix, threads, |piece, chans| {
            for (pc, oc) in chans.chunks_mut(pix).enumerate() {
                channel(piece * per + pc, oc);
            }
        });
        return out;
    }
    let epi = Epilogue {
        bias: bias.as_deref(),
        per_row: true,
        act: p.act,
    };
    for img in 0..n {
        for gi in 0..g {
            // im2col of this image's group ([K, P]): row-major and read in place by a short
            // GEMM, else packed.
            let xin = &xf[(img * c + gi * cg) * h * wd..(img * c + (gi + 1) * cg) * h * wd];
            let threads = gemm_threads(og * kk * pix, threads);
            let cols;
            let packed = [if pointwise {
                prepare_b(xin, 0, pix, 1, kk, pix, og, threads)
            } else if og <= DIRECT_M {
                cols = im2col(xin, (cg, h, wd), (kh, kw), p, (oh, ow), threads);
                prepare_b(&cols, 0, pix, 1, kk, pix, og, threads)
            } else {
                PackedB::Packed(pack_im2col(
                    xin,
                    (cg, h, wd),
                    (kh, kw),
                    p,
                    (oh, ow),
                    threads,
                ))
            }];
            let dst = &mut out[(img * o + gi * og) * pix..(img * o + (gi + 1) * og) * pix];
            let job = Job {
                a: gi * og * kk,
                b: 0,
                c: 0,
                bias: gi * og,
            };
            gemm_jobs(&wf, kk, &packed, &[job], dst, (og, kk, pix), &epi, threads);
        }
    }
    out
}

// ---- elementwise ----------------------------------------------------------------------------

/// Strides of `shape` broadcast to `out` (0 on broadcast axes), left-padded.
pub(crate) fn broadcast_strides(shape: &[usize], out: &[usize]) -> Vec<usize> {
    let st = strides(shape);
    let pad = out.len() - shape.len();
    (0..out.len())
        .map(|d| {
            if d < pad || shape[d - pad] == 1 {
                0
            } else {
                st[d - pad]
            }
        })
        .collect()
}

fn binary(op: BinaryOp, a: &Tensor, b: &Tensor, out_shape: &[usize]) -> Vec<f32> {
    let (af, bf) = (a.as_f32(), b.as_f32());
    if a.shape == b.shape {
        return af
            .iter()
            .zip(bf.iter())
            .map(|(&x, &y)| op.apply(x, y))
            .collect();
    }
    let sa = broadcast_strides(&a.shape, out_shape);
    let sb = broadcast_strides(&b.shape, out_shape);
    let n = numel(out_shape).unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    let r = out_shape.len();
    let mut idx = vec![0usize; r];
    let (mut oa, mut ob) = (0usize, 0usize);
    for _ in 0..n {
        out.push(op.apply(af[oa], bf[ob]));
        // Odometer.
        for d in (0..r).rev() {
            idx[d] += 1;
            oa += sa[d];
            ob += sb[d];
            if idx[d] < out_shape[d] {
                break;
            }
            oa -= sa[d] * idx[d];
            ob -= sb[d] * idx[d];
            idx[d] = 0;
        }
    }
    out
}

/// [`Op::Gate`]: per image, the first half of the channels times the second.
fn gate(x: &Tensor) -> Vec<f32> {
    let s = &x.shape;
    let half = s[1] / 2 * s[2] * s[3];
    let v = x.as_f32();
    let mut out = Vec::with_capacity(v.len() / 2);
    for img in v.chunks_exact(2 * half) {
        let (a, b) = img.split_at(half);
        out.extend(a.iter().zip(b).map(|(a, b)| a * b));
    }
    out
}

/// [`Op::DepthToSpace`] as a strided copy: the output viewed as `[N, C, H, r, W, r]`, and
/// the input's stride for each of those axes.
pub(crate) fn depth_to_space_strides(s: &[usize], r: usize) -> ([usize; 6], [usize; 6]) {
    let (n, c, h, w) = (s[0], s[1] / (r * r), s[2], s[3]);
    let hw = h * w;
    (
        [n, c, h, r, w, r],
        [c * r * r * hw, r * r * hw, w, r * hw, 1, hw],
    )
}

/// Copies `src` viewed with `src_strides` (per output axis) from `base` into a contiguous
/// f32 tensor of `out_shape`.
fn copy_out(src: &Tensor, out_shape: &[usize], src_strides: &[usize], base: usize) -> Tensor {
    let n = numel(out_shape).unwrap_or(0);
    let r = out_shape.len();
    let v = src.as_f32();
    let mut out = Vec::with_capacity(n);
    let mut idx = vec![0usize; r];
    let mut off = base;
    for _ in 0..n {
        out.push(v[off]);
        for d in (0..r).rev() {
            idx[d] += 1;
            off += src_strides[d];
            if idx[d] < out_shape[d] {
                break;
            }
            off -= src_strides[d] * idx[d];
            idx[d] = 0;
        }
    }
    Tensor::f32(out_shape, out)
}

fn concat(ins: &[&Tensor], axis: usize, out_shape: &[usize]) -> Tensor {
    let outer: usize = out_shape[..axis].iter().product();
    let inner: usize = out_shape[axis + 1..].iter().product();
    let vals: Vec<_> = ins.iter().map(|t| t.as_f32()).collect();
    let mut out = Vec::with_capacity(numel(out_shape).unwrap_or(0));
    for o in 0..outer {
        for (t, v) in ins.iter().zip(&vals) {
            let len = t.shape[axis] * inner;
            out.extend_from_slice(&v[o * len..(o + 1) * len]);
        }
    }
    Tensor::f32(out_shape, out)
}

fn gather(table: &Tensor, idx: &Tensor) -> Vec<f32> {
    let t = table.as_f32();
    let (v, d) = (table.shape[0], table.shape[1]);
    let mut out = Vec::with_capacity(idx.len() * d);
    for &x in idx.as_f32().iter() {
        let i = gather_index(x, v);
        out.extend_from_slice(&t[i * d..(i + 1) * d]);
    }
    out
}

fn lstm(ins: &[&Tensor], reverse: bool, summary: bool, per: usize, threads: usize) -> Vec<f32> {
    let pre = ins[0].as_f32();
    let wh = ins[1].as_f32();
    let lens = ins.get(2).map(|t| t.as_f32());
    let (s, t, ns) = (ins[0].shape[0], ins[0].shape[1], ins[1].shape[0]);
    let steps = if summary { 1 } else { t };
    let mut out = vec![0f32; s * steps * ns];
    par_chunks(&mut out, steps * ns, threads, |q, o| {
        let len = lens
            .as_ref()
            .map_or(t, |l| gather_index(l[q / per.max(1)], t + 1));
        let (mut c, mut h) = (vec![0f32; ns], vec![0f32; ns]);
        let mut g = vec![0f32; 4 * ns];
        for step in 0..len {
            let tt = if reverse { len - 1 - step } else { step };
            g.copy_from_slice(&pre[(q * t + tt) * 4 * ns..(q * t + tt + 1) * 4 * ns]);
            for (j, &hj) in h.iter().enumerate() {
                for (gv, &w) in g.iter_mut().zip(&wh[j * 4 * ns..(j + 1) * 4 * ns]) {
                    *gv += hj * w;
                }
            }
            for i in 0..ns {
                let ci = g[i].tanh();
                let gi = crate::graph::Act::Sigmoid.apply(g[ns + i]);
                let gf = crate::graph::Act::Sigmoid.apply(g[2 * ns + i]);
                let go = crate::graph::Act::Sigmoid.apply(g[3 * ns + i]);
                c[i] = (c[i] * gf + ci * gi).clamp(-100.0, 100.0);
                h[i] = c[i].tanh() * go;
            }
            if !summary {
                o[tt * ns..(tt + 1) * ns].copy_from_slice(&h);
            }
        }
        if summary {
            o.copy_from_slice(&h);
        }
    });
    out
}

/// The row an index value picks: truncated, clamped to the table, NaN → 0.
pub fn gather_index(x: f32, rows: usize) -> usize {
    if x.is_nan() {
        0
    } else {
        x.clamp(0.0, (rows - 1) as f32) as usize
    }
}

// ---- rows -----------------------------------------------------------------------------------

/// Elements per piece of a row-wise op, and per thread below which it uses fewer threads.
const ROWS_PIECE: usize = 1 << 13;
const ROWS_PER_THREAD: usize = 1 << 15;

fn layer_norm(x: &Tensor, g: &Tensor, b: &Tensor, eps: f32, threads: usize) -> Vec<f32> {
    let d = *x.shape.last().expect("rank");
    let (g, b) = (g.as_f32(), b.as_f32());
    let mut v = x.as_f32().into_owned();
    if d == 0 {
        return v;
    }
    let threads = threads.min(v.len().div_ceil(ROWS_PER_THREAD)).max(1);
    let rows = (ROWS_PIECE / d).next_multiple_of(4).max(4);
    par_chunks(&mut v, rows * d, threads, |_, block| {
        // Four rows side by side: each row's sums run in order, but the four chains of
        // f64 adds overlap instead of waiting on each other.
        for quad in block.chunks_mut(4 * d) {
            let n = quad.len() / d;
            let mut mean = [0f64; 4];
            for i in 0..d {
                for r in 0..n {
                    mean[r] += quad[r * d + i] as f64;
                }
            }
            mean.iter_mut().for_each(|m| *m /= d as f64);
            let mut var = [0f64; 4];
            for i in 0..d {
                for r in 0..n {
                    let c = quad[r * d + i] as f64 - mean[r];
                    var[r] += c * c;
                }
            }
            for (r, row) in quad.chunks_mut(d).enumerate() {
                let inv = 1.0 / (var[r] / d as f64 + eps as f64).sqrt();
                for (i, x) in row.iter_mut().enumerate() {
                    *x = ((*x as f64 - mean[r]) * inv) as f32 * g[i] + b[i];
                }
            }
        }
    });
    v
}

/// Per pixel of an NCHW map, across its channels: (x − mean)/√(var + ε)·γ + β, sums in f64.
fn channel_norm(x: &Tensor, g: &Tensor, b: &Tensor, eps: f32, threads: usize) -> Vec<f32> {
    let s = &x.shape;
    let (c, hw) = (s[1], s[2] * s[3]);
    let (g, b) = (g.as_f32(), b.as_f32());
    let xv = x.as_f32();
    let mut v = vec![0f32; xv.len()];
    if hw == 0 {
        return v;
    }
    let threads = threads.min(v.len().div_ceil(ROWS_PER_THREAD)).max(1);
    par_chunks(&mut v, c * hw, threads, |n, img| {
        let src = &xv[n * c * hw..(n + 1) * c * hw];
        let mut mean = vec![0f64; hw];
        for ch in 0..c {
            for (m, &x) in mean.iter_mut().zip(&src[ch * hw..(ch + 1) * hw]) {
                *m += x as f64;
            }
        }
        mean.iter_mut().for_each(|m| *m /= c as f64);
        let mut var = vec![0f64; hw];
        for ch in 0..c {
            for ((q, &m), &x) in var.iter_mut().zip(&mean).zip(&src[ch * hw..(ch + 1) * hw]) {
                let d = x as f64 - m;
                *q += d * d;
            }
        }
        for q in var.iter_mut() {
            *q = 1.0 / (*q / c as f64 + eps as f64).sqrt();
        }
        for ch in 0..c {
            let dst = &mut img[ch * hw..(ch + 1) * hw];
            let sx = &src[ch * hw..(ch + 1) * hw];
            for i in 0..hw {
                dst[i] = ((sx[i] as f64 - mean[i]) * var[i]) as f32 * g[ch] + b[ch];
            }
        }
    });
    v
}

/// Per image and group: statistics over the group's channels and pixels (f64 sums), then
/// each channel's own γ and β.
fn group_norm(ins: &[&Tensor], groups: usize, eps: f32, threads: usize) -> Vec<f32> {
    let s = &ins[0].shape;
    let (c, hw) = (s[1], s[2] * s[3]);
    let (g, b) = (ins[1].as_f32(), ins[2].as_f32());
    let mut v = ins[0].as_f32().into_owned();
    let per = c / groups * hw;
    if per == 0 {
        return v;
    }
    let threads = threads.min(v.len().div_ceil(ROWS_PER_THREAD)).max(1);
    par_chunks(&mut v, per, threads, |row, x| {
        let mean = x.iter().map(|&a| a as f64).sum::<f64>() / per as f64;
        let var = x.iter().map(|&a| (a as f64 - mean).powi(2)).sum::<f64>() / per as f64;
        let inv = 1.0 / (var + eps as f64).sqrt();
        let c0 = (row % groups) * (c / groups);
        for (i, a) in x.iter_mut().enumerate() {
            let ch = c0 + i / hw;
            *a = ((*a as f64 - mean) * inv) as f32 * g[ch] + b[ch];
        }
    });
    v
}

/// Gathers each output pixel from the input taps that land on it, in input-channel then
/// kernel-row then kernel-column order (the GPU kernel's order).
fn conv_transpose2d(
    x: &Tensor,
    w: &Tensor,
    bias: Option<&Tensor>,
    (stride, pad, act): ([usize; 2], [usize; 4], Act),
    out_shape: &[usize],
    threads: usize,
) -> Vec<f32> {
    let (xs, ws) = (&x.shape, &w.shape);
    let (c, ih, iw) = (xs[1], xs[2], xs[3]);
    let (o, kh, kw) = (ws[1], ws[2], ws[3]);
    let (oh, ow) = (out_shape[2], out_shape[3]);
    let (xv, wv) = (x.as_f32(), w.as_f32());
    let bv = bias.map(|b| b.as_f32());
    let mut v = vec![0f32; numel(out_shape).unwrap_or(0)];
    par_chunks(&mut v, oh * ow, threads, |plane, dst| {
        let (n, oc) = (plane / o, plane % o);
        let b0 = bv.as_ref().map_or(0.0, |b| b[oc]);
        for oy in 0..oh {
            for ox in 0..ow {
                let mut acc = 0f32;
                for ic in 0..c {
                    let xb = (n * c + ic) * ih * iw;
                    let wb = (ic * o + oc) * kh * kw;
                    for ky in 0..kh {
                        let fy = oy + pad[0];
                        if fy < ky || (fy - ky) % stride[0] != 0 || (fy - ky) / stride[0] >= ih {
                            continue;
                        }
                        let iy = (fy - ky) / stride[0];
                        for kx in 0..kw {
                            let fx = ox + pad[1];
                            if fx < kx || (fx - kx) % stride[1] != 0 {
                                continue;
                            }
                            let ix = (fx - kx) / stride[1];
                            if ix >= iw {
                                continue;
                            }
                            acc += xv[xb + iy * iw + ix] * wv[wb + ky * kw + kx];
                        }
                    }
                }
                dst[oy * ow + ox] = act.apply(acc + b0);
            }
        }
    });
    v
}

fn l2_norm(x: &Tensor, eps: f32) -> Vec<f32> {
    let d = *x.shape.last().expect("rank");
    let mut v = x.as_f32().into_owned();
    for row in v.chunks_mut(d) {
        let n = row.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
        let s = 1.0 / n.max(eps as f64);
        for x in row {
            *x = (*x as f64 * s) as f32;
        }
    }
    v
}

/// softmax(scale·x) over the row's first `valid` entries; the rest become 0 (a causal mask).
fn softmax_row(row: &mut [f32], scale: f32, valid: usize) {
    let max = row[..valid]
        .iter()
        .fold(f32::NEG_INFINITY, |m, &x| m.max(x * scale));
    let mut sum = 0f64;
    for x in row[..valid].iter_mut() {
        *x = (*x * scale - max).exp();
        sum += *x as f64;
    }
    let inv = (1.0 / sum) as f32;
    for x in row[..valid].iter_mut() {
        *x *= inv;
    }
    row[valid..].fill(0.0);
}

fn attention(
    ins: &[&Tensor],
    heads: usize,
    scale: f32,
    causal: bool,
    packed: bool,
    threads: usize,
) -> Vec<f32> {
    let (q, k, v, tq, tk, d, rq, rk, oq, ok, ov) = if packed {
        let s = &ins[0].shape;
        let x = ins[0].as_f32();
        let d = s[2] / 3;
        (
            x.clone(),
            x.clone(),
            x,
            s[1],
            s[1],
            d,
            3 * d,
            3 * d,
            0,
            d,
            2 * d,
        )
    } else {
        let (sq, sk) = (&ins[0].shape, &ins[1].shape);
        (
            ins[0].as_f32(),
            ins[1].as_f32(),
            ins[2].as_f32(),
            sq[1],
            sk[1],
            sq[2],
            sq[2],
            sk[2],
            0,
            0,
            0,
        )
    };
    let n = ins[0].shape[0];
    let hd = d / heads;
    // Each (image, head) writes [tq, hd]; scattered into [N, Tq, D] after.
    let mut parts = vec![0f32; n * heads * tq * hd];
    // Pieces of query rows (each row's result does not depend on the others), enough for
    // a few per thread.
    let threads = gemm_threads(n * heads * tq * tk * hd * 2, threads);
    let per_head = (3 * threads).div_ceil(n * heads).max(1);
    let rows = tq.div_ceil(per_head).next_multiple_of(MR).max(1);
    let mut work = Vec::new();
    for (i, head) in parts.chunks_mut((tq * hd).max(1)).enumerate() {
        for (j, o) in head.chunks_mut((rows * hd).max(1)).enumerate() {
            work.push((i / heads, i % heads, j * rows, o));
        }
    }
    par_work(work, threads, |(b, h, t0, o)| {
        let rows = o.len() / hd.max(1);
        let mut s = vec![0f32; rows * tk];
        // S = Q·Kᵀ: B(e, t) = K[t, e].
        let qa = (b * tq + t0) * rq + oq + h * hd;
        let kb = b * tk * rk + ok + h * hd;
        gemm(&q, qa, rq, &k, (kb, 1, rk), &mut s, (rows, hd, tk), 1);
        for (t, row) in s.chunks_mut(tk).enumerate() {
            let valid = if causal { (t0 + t + 1).min(tk) } else { tk };
            softmax_row(row, scale, valid);
        }
        let vb = b * tk * rk + ov + h * hd;
        gemm(&s, 0, tk, &v, (vb, rk, 1), o, (rows, tk, hd), 1);
    });
    let mut out = vec![0f32; n * tq * d];
    for b in 0..n {
        for h in 0..heads {
            for t in 0..tq {
                let src = ((b * heads + h) * tq + t) * hd;
                let dst = (b * tq + t) * d + h * hd;
                out[dst..dst + hd].copy_from_slice(&parts[src..src + hd]);
            }
        }
    }
    out
}

// ---- spatial --------------------------------------------------------------------------------

fn pool2d(x: &Tensor, p: &Pool2d, out_shape: &[usize]) -> Vec<f32> {
    let [n, c, h, w] = x.shape[..] else {
        unreachable!()
    };
    let [_, _, oh, ow] = out_shape[..] else {
        unreachable!()
    };
    let xf = x.as_f32();
    let mut out = Vec::with_capacity(n * c * oh * ow);
    for plane in xf.chunks(h * w) {
        for oy in 0..oh {
            for ox in 0..ow {
                let (mut acc, mut cnt) = (
                    if p.kind == PoolKind::Max {
                        f32::NEG_INFINITY
                    } else {
                        0.0
                    },
                    0u32,
                );
                for ky in 0..p.kernel[0] {
                    let iy = (oy * p.stride[0] + ky) as isize - p.pad[0] as isize;
                    if iy < 0 || iy >= h as isize {
                        continue;
                    }
                    for kx in 0..p.kernel[1] {
                        let ix = (ox * p.stride[1] + kx) as isize - p.pad[1] as isize;
                        if ix < 0 || ix >= w as isize {
                            continue;
                        }
                        let v = plane[iy as usize * w + ix as usize];
                        match p.kind {
                            PoolKind::Max => acc = acc.max(v),
                            PoolKind::Avg => acc += v,
                        }
                        cnt += 1;
                    }
                }
                out.push(match p.kind {
                    PoolKind::Max => acc,
                    PoolKind::Avg => acc / cnt.max(1) as f32,
                });
            }
        }
    }
    out
}

fn reduce(x: &Tensor, op: ReduceOp, axis: usize) -> Vec<f32> {
    let s = &x.shape;
    let outer: usize = s[..axis].iter().product();
    let len = s[axis];
    let inner: usize = s[axis + 1..].iter().product();
    let xf = x.as_f32();
    let mut out = Vec::with_capacity(outer * inner);
    for o in 0..outer {
        for i in 0..inner {
            let mut acc = if op == ReduceOp::Max {
                f32::NEG_INFINITY
            } else {
                0.0
            };
            for l in 0..len {
                let v = xf[(o * len + l) * inner + i];
                acc = if op == ReduceOp::Max {
                    acc.max(v)
                } else {
                    acc + v
                };
            }
            if op == ReduceOp::Mean {
                acc /= len as f32;
            }
            out.push(acc);
        }
    }
    out
}

/// Source coordinate scale of a resize, as both engines compute it.
pub(crate) fn resize_scale(len_in: usize, len_out: usize) -> f32 {
    len_in as f32 / len_out as f32
}

/// The step of a corner-aligned resize: (in − 1)/(out − 1), 0 for one output.
pub(crate) fn aligned_scale(len_in: usize, len_out: usize) -> f32 {
    if len_out <= 1 {
        0.0
    } else {
        (len_in - 1) as f32 / (len_out - 1) as f32
    }
}

fn resize(x: &Tensor, mode: ResizeMode, out_shape: &[usize]) -> Vec<f32> {
    let [_, _, h, w] = x.shape[..] else {
        unreachable!()
    };
    let [_, _, oh, ow] = out_shape[..] else {
        unreachable!()
    };
    let (sy, sx) = if mode == ResizeMode::BilinearAligned {
        (aligned_scale(h, oh), aligned_scale(w, ow))
    } else {
        (resize_scale(h, oh), resize_scale(w, ow))
    };
    let xf = x.as_f32();
    let mut out = Vec::with_capacity(numel(out_shape).unwrap_or(0));
    for plane in xf.chunks(h * w) {
        for oy in 0..oh {
            for ox in 0..ow {
                out.push(match mode {
                    ResizeMode::Nearest => {
                        let iy = (((oy as f32 + 0.5) * sy) as usize).min(h - 1);
                        let ix = (((ox as f32 + 0.5) * sx) as usize).min(w - 1);
                        plane[iy * w + ix]
                    }
                    ResizeMode::Bilinear
                    | ResizeMode::BilinearAsymmetric
                    | ResizeMode::BilinearAligned => {
                        let half = if mode == ResizeMode::Bilinear {
                            0.5
                        } else {
                            0.0
                        };
                        let cy = ((oy as f32 + half) * sy - half).clamp(0.0, (h - 1) as f32);
                        let cx = ((ox as f32 + half) * sx - half).clamp(0.0, (w - 1) as f32);
                        let (y0, x0) = (cy.floor() as usize, cx.floor() as usize);
                        let (y1, x1) = ((y0 + 1).min(h - 1), (x0 + 1).min(w - 1));
                        let (ty, tx) = (cy - y0 as f32, cx - x0 as f32);
                        let top = plane[y0 * w + x0] * (1.0 - tx) + plane[y0 * w + x1] * tx;
                        let bot = plane[y1 * w + x0] * (1.0 - tx) + plane[y1 * w + x1] * tx;
                        top * (1.0 - ty) + bot * ty
                    }
                });
            }
        }
    }
    out
}

/// The taps and weights of one output sample of a [`Preprocess`] axis: centre `c`, filter
/// half-width `f`, source length `len`. Weights are not normalised.
pub(crate) fn taps(c: f32, f: f32, len: usize) -> (usize, Vec<f32>) {
    let lo = ((c - f - 0.5).floor().max(0.0)) as usize;
    let hi = ((c + f - 0.5).ceil().max(0.0) as usize).min(len - 1);
    let w = (lo..=hi)
        .map(|i| (1.0 - ((i as f32 + 0.5) - c).abs() / f).max(0.0))
        .collect();
    (lo, w)
}

/// Per-axis parameters of a [`Preprocess`] for one source size: (x0, y0, sx, sy, fx, fy).
pub(crate) fn preprocess_params(p: &Preprocess, sw: usize, sh: usize) -> [f32; 6] {
    let [x0, y0, rw, rh] = p.source_rect(sw, sh);
    let sx = rw / p.width as f32;
    let sy = rh / p.height as f32;
    let (fx, fy) = if p.antialias {
        (sx.max(1.0), sy.max(1.0))
    } else {
        (1.0, 1.0)
    };
    [x0, y0, sx, sy, fx, fy]
}

fn preprocess(img: &Image, p: &Preprocess) -> Tensor {
    let [x0, y0, sx, sy, fx, fy] = preprocess_params(p, img.w, img.h);
    let (ow, oh) = (p.width, p.height);
    let xt: Vec<(usize, Vec<f32>)> = (0..ow)
        .map(|o| taps(x0 + (o as f32 + 0.5) * sx, fx, img.w))
        .collect();
    let yt: Vec<(usize, Vec<f32>)> = (0..oh)
        .map(|o| taps(y0 + (o as f32 + 0.5) * sy, fy, img.h))
        .collect();
    let mut out = vec![0f32; 3 * oh * ow];
    for (oy, (ylo, yw)) in yt.iter().enumerate() {
        for (ox, (xlo, xw)) in xt.iter().enumerate() {
            let mut acc = [0f32; 3];
            let mut wsum = 0f32;
            for (j, &wy) in yw.iter().enumerate() {
                let row = (ylo + j) * img.w;
                for (i, &wx) in xw.iter().enumerate() {
                    let wgt = wx * wy;
                    let px = &img.data[(row + xlo + i) * 4..][..3];
                    for ch in 0..3 {
                        acc[ch] += wgt * (px[ch] as f32 / 255.0);
                    }
                    wsum += wgt;
                }
            }
            for ch in 0..3 {
                let v = acc[ch] / wsum;
                out[(ch * oh + oy) * ow + ox] = (v - p.mean[ch]) / p.std[ch];
            }
        }
    }
    Tensor::f32(&[1, 3, oh, ow], out)
}
