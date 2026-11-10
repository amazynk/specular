//! Tesseract's LSTM line recogniser, read from a `.traineddata` file (tessdata_best), for
//! scripts PaddleOCR has no model for (Hebrew).
//!
//! Our own reader of the file: a directory (i32 count, then i64 offsets, −1 for absent
//! components) of components; we read the LSTM network (17), its character set (21, the
//! text unicharset format: a count, then one line per character, `NULL` standing for the
//! space) and its recoder (22: per character, a byte, an i32 code length and that many i32
//! codes). After the network come its spec string and, among the training counters, the
//! code of the CTC "null" (no character).
//!
//! A line is read as Tesseract's LSTM path does: grey, scaled to the network's input height
//! (36), each value mapped so the line's typical dark is −1 and its typical light +1 (from
//! the local minima and maxima along its middle row), the network run, and the best code per
//! step decoded CTC-style. Dark-on-light is assumed; a line whose background is darker than
//! its text is inverted first. No dictionary, no beam search.

pub mod gpu;
pub mod net;

use std::path::Path;

use self::net::{Grid, Layer, Reader, TRand};
use super::ctc::{self, Decoded};
use super::lines::Img8;
use crate::gpu::{Context, GpuGraph};
use crate::tensor::Tensor;
use crate::{Error, Feed};

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

const LSTM: usize = 17;
const LSTM_UNICHARSET: usize = 21;
const LSTM_RECODER: usize = 22;
const VERSION: usize = 23;

/// The components of a `.traineddata` file: byte ranges by index.
pub fn components(data: &[u8]) -> Result<Vec<Option<(usize, usize)>>, Error> {
    let mut r = Reader::new(data);
    let n = r.i32()?;
    if !(1..=64).contains(&n) {
        return bad("not a traineddata file");
    }
    let mut offs = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let b = r.bytes(8)?;
        offs.push(i64::from_le_bytes(b.try_into().expect("8 bytes")));
    }
    let len = data.len() as i64;
    let mut out = vec![None; n as usize];
    for i in 0..n as usize {
        let start = offs[i];
        if start < 0 {
            continue;
        }
        // Ends at the next present component, or the file's end.
        let end = offs[i + 1..]
            .iter()
            .copied()
            .find(|&o| o >= 0)
            .unwrap_or(len);
        if start > end || end > len || start < r.pos as i64 {
            return bad("traineddata directory out of order");
        }
        out[i] = Some((start as usize, end as usize));
    }
    Ok(out)
}

/// A Tesseract LSTM model.
pub struct Tesseract {
    pub net: Layer,
    pub spec: String,
    pub version: String,
    /// Unichar strings by id (id 0 is the space).
    pub unichars: Vec<String>,
    /// Code sequences → unichar id.
    decode: std::collections::HashMap<Vec<i32>, usize>,
    longest: usize,
    pub null_code: usize,
    /// The training sample counter: seeds the per-line random numbers.
    pub sample_iteration: i32,
    pub height: usize,
    pub classes: usize,
    /// GPU graphs by (lines, padded width), most recent first.
    gpu: Vec<(usize, usize, GpuGraph)>,
}

/// One line's per-step probabilities (`steps × classes`), None for a blank line.
pub type LineProbs = Option<Vec<f32>>;

/// GPU graphs kept per model.
const GPU_CACHED: usize = 12;

/// Lines per GPU run at most.
pub const GPU_BATCH: usize = 16;

/// Padded widths (input columns after the convolution) the GPU graphs are built for; a
/// line takes the smallest that holds it, wider lines a multiple of the largest.
const GPU_WIDTHS: [usize; 12] = [
    96, 144, 192, 288, 384, 576, 768, 1152, 1536, 2304, 3072, 4096,
];

fn empty() -> Decoded {
    Decoded {
        text: String::new(),
        confidence: 0.0,
        steps: Vec::new(),
    }
}

fn parse_unicharset(text: &str) -> Result<Vec<String>, Error> {
    let mut lines = text.lines();
    let n: usize = lines
        .next()
        .and_then(|l| l.trim().parse().ok())
        .ok_or(Error::Weights("unicharset without a count".into()))?;
    if n == 0 || n > 1 << 20 {
        return bad("unicharset size");
    }
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let l = lines
            .next()
            .ok_or(Error::Weights("unicharset ends early".into()))?;
        let first = l.split(' ').next().unwrap_or("");
        v.push(if first == "NULL" {
            " ".to_owned()
        } else {
            first.to_owned()
        });
    }
    Ok(v)
}

impl Tesseract {
    pub fn open(path: &Path) -> Result<Tesseract, Error> {
        let data =
            std::fs::read(path).map_err(|e| Error::Weights(format!("{}: {e}", path.display())))?;
        Tesseract::from_bytes(&data)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Tesseract, Error> {
        let comps = components(data)?;
        let get = |i: usize| comps.get(i).copied().flatten().map(|(s, e)| &data[s..e]);
        let Some(lstm) = get(LSTM) else {
            return bad("no LSTM model in the traineddata (a legacy-only file)");
        };
        let (Some(uni), Some(rec)) = (get(LSTM_UNICHARSET), get(LSTM_RECODER)) else {
            return bad("the traineddata has no LSTM character set");
        };
        let version = get(VERSION)
            .map(|v| String::from_utf8_lossy(v).into_owned())
            .unwrap_or_default();
        let mut r = Reader::new(lstm);
        let net = Layer::read(&mut r)?;
        let spec = r.string()?;
        let _flags = r.i32()?;
        let _iteration = r.i32()?;
        let sample_iteration = r.i32()?;
        let null = r.i32()?;
        let unichars = parse_unicharset(&String::from_utf8_lossy(uni))?;
        let mut rr = Reader::new(rec);
        let n = rr.u32()? as usize;
        if n != unichars.len() {
            return bad(format!(
                "recoder has {n} entries, the character set {}",
                unichars.len()
            ));
        }
        let mut decode = std::collections::HashMap::new();
        let mut longest = 1;
        let mut max_code = 0;
        for id in 0..n {
            let _self_normalised = rr.u8()?;
            let len = rr.size()?;
            if len == 0 || len > 9 {
                return bad("recoder code length");
            }
            let codes: Vec<i32> = (0..len).map(|_| rr.i32()).collect::<Result<_, _>>()?;
            max_code = max_code.max(codes.iter().copied().max().unwrap_or(0));
            longest = longest.max(len);
            decode.entry(codes).or_insert(id);
        }
        let height = net.input_height().ok_or(Error::Weights(
            "the network has no fixed input height".into(),
        ))?;
        let classes = max_code.max(null) as usize + 1;
        if !(1..=10_000).contains(&null) {
            return bad("null code out of range");
        }
        Ok(Tesseract {
            net,
            spec,
            version,
            unichars,
            decode,
            longest,
            null_code: null as usize,
            sample_iteration,
            height,
            classes,
            gpu: Vec::new(),
        })
    }

    /// The network input for a grey line image: scaled to the input height, values mapped to
    /// [−1, 1] from the line's own dark and light.
    pub fn input(&self, line: &Img8) -> Grid {
        let g = line.grey();
        let g = if dark_background(&g) { invert(&g) } else { g };
        let g = trim_to_ink(&g, 4);
        let s = scale_to_height(&g, self.height);
        let (black, white) = black_white(&s);
        let contrast = if white - black > 0.0 {
            (white - black) / 2.0
        } else {
            1.0
        };
        let mut grid = Grid::new(s.height, s.width, 1);
        for (o, &p) in grid.v.iter_mut().zip(&s.data) {
            *o = (f32::from(p) - black) / contrast - 1.0;
        }
        grid
    }

    /// The network input for a line, padded to at least one output step.
    fn padded_input(&self, line: &Img8) -> Grid {
        let mut x = self.input(line);
        let min_w = self.net.x_scale().max(1);
        if x.w < min_w {
            // Too short for one step: pad with the background.
            let mut p = Grid::new(x.h, min_w, 1);
            for y in 0..x.h {
                for xx in 0..min_w {
                    p.v[y * min_w + xx] = if xx < x.w { x.v[y * x.w + xx] } else { 1.0 };
                }
            }
            x = p;
        }
        x
    }

    /// The per-step code probabilities for a line: (`steps × classes` row-major, classes).
    pub fn probs(&self, line: &Img8) -> Result<(Vec<f32>, usize), Error> {
        let x = self.padded_input(line);
        let out = self
            .net
            .forward(x, &mut TRand::for_line(self.sample_iteration))?;
        if out.h != 1 {
            return bad("the network does not reduce the height to one row");
        }
        Ok((out.v, out.d))
    }

    /// Reads one line (in logical order).
    pub fn read_line(&self, line: &Img8) -> Result<Decoded, Error> {
        if !has_contrast(&line.grey()) {
            // A blank line: nothing to read (the network would make something up).
            return Ok(empty());
        }
        let (p, classes) = self.probs(line)?;
        Ok(self.decode(&p, classes))
    }

    /// Best code per step, CTC-style, codes to characters (longest sequence first).
    fn decode(&self, p: &[f32], classes: usize) -> Decoded {
        let mut codes = Vec::new();
        let mut confs = Vec::new();
        let mut steps = Vec::new();
        let mut prev = usize::MAX;
        for (t, row) in p.chunks_exact(classes).enumerate() {
            let (mut best, mut bp) = (0usize, f32::MIN);
            for (i, &v) in row.iter().enumerate() {
                if v > bp {
                    bp = v;
                    best = i;
                }
            }
            if best != self.null_code && best != prev {
                codes.push(best as i32);
                confs.push(bp);
                steps.push(t);
            }
            prev = best;
        }
        let mut text = String::new();
        let mut i = 0;
        let (mut conf, mut kept, mut ksteps) = (0f64, 0usize, Vec::new());
        while i < codes.len() {
            let mut took = 0;
            for len in (1..=self.longest.min(codes.len() - i)).rev() {
                if let Some(&id) = self.decode.get(&codes[i..i + len]) {
                    if id > 2 || id == 0 {
                        text.push_str(&self.unichars[id]);
                        conf += f64::from(confs[i..i + len].iter().copied().fold(1.0, f32::min));
                        kept += 1;
                        ksteps.push(steps[i]);
                    }
                    took = len;
                    break;
                }
            }
            i += took.max(1);
        }
        let text = ctc::visual_to_logical(text.trim());
        Decoded {
            text,
            confidence: if kept == 0 {
                0.0
            } else {
                (conf / kept as f64) as f32
            },
            steps: ksteps,
        }
    }

    /// The per-step probabilities of `lines` on the GPU, each `steps × classes` (None for a
    /// blank line), with the convolution on up to `threads` CPU threads. The same values as
    /// [`Tesseract::probs`] up to float rounding.
    pub fn probs_gpu(
        &mut self,
        ctx: &Context,
        lines: &[Img8],
        threads: usize,
    ) -> Result<(Vec<LineProbs>, usize), Error> {
        let xs = self.net.x_scale().max(1);
        let mut classes = 0;
        let grids = self.fronts(lines, threads)?;
        let mut out: Vec<LineProbs> = vec![None; lines.len()];
        // Widest first, up to GPU_BATCH lines a run padded to the widest's width (columns
        // past a line's end cost the products only: the LSTMs stop at each line's end).
        let mut order: Vec<(usize, usize)> = grids
            .iter()
            .enumerate()
            .filter_map(|(i, g)| g.as_ref().map(|g| (gpu_width(g.w, xs), i)))
            .collect();
        order.sort_unstable_by(|a, b| b.cmp(a));
        let mut k = 0;
        while k < order.len() {
            let width = order[k].0;
            let mut end = k + 1;
            while end < order.len() && end - k < GPU_BATCH {
                end += 1;
            }
            // Batches of 1, 4 or 16 lines only, so the graphs are reused.
            let n = match end - k {
                1 => 1,
                2..=4 => 4,
                _ => GPU_BATCH,
            };
            let first = grids[order[k].1].as_ref().expect("kept");
            let (h, d) = (first.h, first.d);
            let mut x = vec![0f32; n * h * width * d];
            let mut lens = vec![0f32; n];
            for (j, &(_, i)) in order[k..end].iter().enumerate() {
                let g = grids[i].as_ref().expect("kept");
                if g.h != h || g.d != d {
                    return bad("lines of different heights");
                }
                for y in 0..h {
                    let src = &g.v[y * g.w * d..(y + 1) * g.w * d];
                    let at = ((j * h + y) * width) * d;
                    x[at..at + src.len()].copy_from_slice(src);
                }
                lens[j] = (g.w / xs) as f32;
            }
            let graph = self.gpu_graph(ctx, n, h, width, d, xs)?;
            let xt = Tensor::f32(&[n, h, width, d], x);
            let lt = Tensor::f32(&[n], lens.clone());
            let res = graph.run(&[Feed::Tensor(&xt), Feed::Tensor(&lt)])?;
            let Some(r) = res.first() else {
                return bad("graph without output");
            };
            let c = *r.shape.last().expect("rank");
            classes = c;
            if c <= self.null_code {
                return bad("output classes");
            }
            let v = r.to_f32();
            let steps = width / xs;
            for (j, &(_, i)) in order[k..end].iter().enumerate() {
                let len = lens[j] as usize;
                out[i] = Some(v[j * steps * c..(j * steps + len) * c].to_vec());
            }
            k = end;
        }
        Ok((out, classes))
    }

    /// Each line's input through the network's CPU part (None for a blank line).
    fn fronts(&self, lines: &[Img8], threads: usize) -> Result<Vec<Option<Grid>>, Error> {
        let front = gpu::Front::split(&self.net)?;
        let mut res: Vec<Option<Result<Grid, Error>>> = (0..lines.len()).map(|_| None).collect();
        let threads = threads.clamp(1, lines.len().max(1));
        let chunk = lines.len().div_ceil(threads).max(1);
        let front = &front;
        std::thread::scope(|s| {
            for (ls, os) in lines.chunks(chunk).zip(res.chunks_mut(chunk)) {
                specular_thread::spawn_scoped(s, "ocr-lstm", move || {
                    for (l, o) in ls.iter().zip(os.iter_mut()) {
                        if has_contrast(&l.grey()) {
                            let x = self.padded_input(l);
                            let mut rng = TRand::for_line(self.sample_iteration);
                            *o = Some(front.run_cpu(x, &mut rng));
                        }
                    }
                });
            }
        });
        res.into_iter().map(Option::transpose).collect()
    }

    fn gpu_graph(
        &mut self,
        ctx: &Context,
        n: usize,
        h: usize,
        width: usize,
        d: usize,
        xs: usize,
    ) -> Result<&mut GpuGraph, Error> {
        let i = match self.gpu.iter().position(|(a, b, _)| (*a, *b) == (n, width)) {
            Some(i) => i,
            None => {
                let front = gpu::Front::split(&self.net)?;
                let g = gpu::build(&front, xs, n, h, width, d)?;
                let gg = GpuGraph::new(ctx, &g)?;
                if self.gpu.len() >= GPU_CACHED {
                    self.gpu.pop();
                }
                self.gpu.push((n, width, gg));
                self.gpu.len() - 1
            }
        };
        let e = self.gpu.remove(i);
        self.gpu.insert(0, e);
        Ok(&mut self.gpu[0].2)
    }

    /// Reads lines on the GPU (the convolution on up to `threads` CPU threads).
    pub fn read_gpu(
        &mut self,
        ctx: &Context,
        lines: &[Img8],
        threads: usize,
    ) -> Result<Vec<Decoded>, Error> {
        let (probs, classes) = self.probs_gpu(ctx, lines, threads)?;
        Ok(probs
            .into_iter()
            .map(|p| p.map_or_else(empty, |p| self.decode(&p, classes)))
            .collect())
    }

    /// Reads lines (each independent) on up to `threads` threads.
    pub fn read(&self, lines: &[Img8], threads: usize) -> Vec<Decoded> {
        let mut out: Vec<Decoded> = (0..lines.len()).map(|_| empty()).collect();
        let threads = threads.clamp(1, lines.len().max(1));
        let chunk = lines.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            for (ls, os) in lines.chunks(chunk).zip(out.chunks_mut(chunk)) {
                specular_thread::spawn_scoped(s, "ocr-lstm", move || {
                    for (l, o) in ls.iter().zip(os.iter_mut()) {
                        *o = self.read_line(l).unwrap_or_else(|_| empty());
                    }
                });
            }
        });
        out
    }
}

/// The padded width a line of `w` columns runs at on the GPU (a multiple of `xs`).
fn gpu_width(w: usize, xs: usize) -> usize {
    let top = *GPU_WIDTHS.last().expect("widths");
    let pick = GPU_WIDTHS
        .into_iter()
        .find(|&s| s >= w)
        .unwrap_or_else(|| w.div_ceil(top) * top);
    pick.div_ceil(xs) * xs
}

/// Whether the line is light on dark: its border is darker than its middle on average.
fn dark_background(g: &Img8) -> bool {
    let (w, h) = (g.width, g.height);
    if w < 3 || h < 3 {
        return false;
    }
    let mut border = (0u64, 0u64);
    for x in 0..w {
        for y in [0, h - 1] {
            border.0 += u64::from(g.data[y * w + x]);
            border.1 += 1;
        }
    }
    for y in 0..h {
        for x in [0, w - 1] {
            border.0 += u64::from(g.data[y * w + x]);
            border.1 += 1;
        }
    }
    let all: u64 = g.data.iter().map(|&v| u64::from(v)).sum();
    let mean_border = border.0 as f64 / border.1 as f64;
    let mean_all = all as f64 / g.data.len() as f64;
    mean_border < 128.0 && mean_border < mean_all
}

/// Whether a grey line has any ink: its 5th and 95th percentiles at least 16 apart.
fn has_contrast(g: &Img8) -> bool {
    let mut v = g.data.clone();
    if v.len() < 2 {
        return false;
    }
    v.sort_unstable();
    let at = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
    at(0.95) > at(0.05).saturating_add(16)
}

/// The line cut to its ink (pixels darker than halfway between its 5th and 95th
/// percentiles) plus `pad` pixels each side, as Tesseract cuts a line from the page.
pub fn trim_to_ink(g: &Img8, pad: usize) -> Img8 {
    let (w, h) = (g.width, g.height);
    if w < 3 || h < 3 {
        return g.clone();
    }
    let mut hist = [0usize; 256];
    for &v in &g.data {
        hist[v as usize] += 1;
    }
    let pct = |f: f64| {
        let target = (g.data.len() as f64 * f) as usize;
        let mut acc = 0;
        for (i, &c) in hist.iter().enumerate() {
            acc += c;
            if acc > target {
                return i;
            }
        }
        255
    };
    let (lo, hi) = (pct(0.05), pct(0.95));
    if hi <= lo + 16 {
        return g.clone();
    }
    let mid = ((lo + hi) / 2) as u8;
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if g.data[y * w + x] < mid {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x0 > x1 {
        return g.clone();
    }
    let (x0, y0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (x1, y1) = ((x1 + pad).min(w - 1), (y1 + pad).min(h - 1));
    let (nw, nh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut data = Vec::with_capacity(nw * nh);
    for y in y0..=y1 {
        data.extend_from_slice(&g.data[y * w + x0..y * w + x1 + 1]);
    }
    Img8 {
        width: nw,
        height: nh,
        channels: 1,
        data,
    }
}

fn invert(g: &Img8) -> Img8 {
    Img8 {
        data: g.data.iter().map(|&v| 255 - v).collect(),
        ..g.clone()
    }
}

/// Scales a grey image to `height` rows (width in proportion, rounded): area averaging
/// when shrinking, bilinear when enlarging.
pub fn scale_to_height(g: &Img8, height: usize) -> Img8 {
    let f = height as f64 / g.height.max(1) as f64;
    let w = ((g.width as f64 * f).round() as usize).max(1);
    if f >= 1.0 {
        return super::lines::resize(g, w, height);
    }
    let (sx, sy) = (g.width as f64 / w as f64, g.height as f64 / height as f64);
    let mut data = vec![0u8; w * height];
    for y in 0..height {
        let (y0, y1) = (y as f64 * sy, (y + 1) as f64 * sy);
        for x in 0..w {
            let (x0, x1) = (x as f64 * sx, (x + 1) as f64 * sx);
            let (mut acc, mut area) = (0f64, 0f64);
            let mut yy = y0.floor() as usize;
            while (yy as f64) < y1 && yy < g.height {
                let wy = (y1.min(yy as f64 + 1.0) - y0.max(yy as f64)).max(0.0);
                let mut xx = x0.floor() as usize;
                while (xx as f64) < x1 && xx < g.width {
                    let wx = (x1.min(xx as f64 + 1.0) - x0.max(xx as f64)).max(0.0);
                    acc += wx * wy * f64::from(g.data[yy * g.width + xx]);
                    area += wx * wy;
                    xx += 1;
                }
                yy += 1;
            }
            data[y * w + x] = if area > 0.0 {
                (acc / area).round().clamp(0.0, 255.0) as u8
            } else {
                0
            };
        }
    }
    Img8 {
        width: w,
        height,
        channels: 1,
        data,
    }
}

/// Typical dark and light of a line: the lower quartile of the local minima and the upper
/// quartile of the local maxima along its middle row (0 and 255 when there are none).
pub fn black_white(g: &Img8) -> (f32, f32) {
    let (w, h) = (g.width, g.height);
    let mut mins = Vec::new();
    let mut maxs = Vec::new();
    if w >= 3 {
        let row = &g.data[(h / 2) * w..(h / 2) * w + w];
        for x in 1..w - 1 {
            let (p, c, n) = (row[x - 1], row[x], row[x + 1]);
            if (c < p && c <= n) || (c <= p && c < n) {
                mins.push(c);
            }
            if (c > p && c >= n) || (c >= p && c > n) {
                maxs.push(c);
            }
        }
    }
    if mins.is_empty() {
        mins.push(0);
    }
    if maxs.is_empty() {
        maxs.push(255);
    }
    mins.sort_unstable();
    maxs.sort_unstable();
    let q = |v: &[u8], f: f64| f32::from(v[((v.len() - 1) as f64 * f).round() as usize]);
    (q(&mins, 0.25), q(&maxs, 0.75))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicharset_null_is_space() {
        let u = parse_unicharset("3\nNULL 0 Common 0\nJoined 7 0,255\nא 1 0,255 Hebrew\n").unwrap();
        assert_eq!(u, vec![" ", "Joined", "א"]);
    }

    #[test]
    fn directory_ranges() {
        let mut d = Vec::new();
        d.extend_from_slice(&3i32.to_le_bytes());
        for o in [-1i64, 28, 30] {
            d.extend_from_slice(&o.to_le_bytes());
        }
        d.extend_from_slice(b"abcd");
        let c = components(&d).unwrap();
        assert_eq!(c, vec![None, Some((28, 30)), Some((30, 32))]);
    }

    #[test]
    fn scale_and_levels() {
        let g = Img8 {
            width: 8,
            height: 4,
            channels: 1,
            data: (0..32).map(|i| if i % 2 == 0 { 20 } else { 230 }).collect(),
        };
        let s = scale_to_height(&g, 2);
        assert_eq!((s.width, s.height), (4, 2));
        let (b, w) = black_white(&g);
        assert_eq!((b, w), (20.0, 230.0));
    }
}
