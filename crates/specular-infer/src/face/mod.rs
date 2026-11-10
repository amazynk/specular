//! Faces: detection, alignment, embeddings for grouping into people, and eye-blink scores
//! for culling (the Local AI pack's People and Culling features).
//!
//! - [`blazeface`]: MediaPipe BlazeFace (full-range, or the short-range model of the Face
//!   Landmarker bundle), read from its `.tflite` file; SSD anchors, score and box decoding,
//!   weighted non-maximum suppression; large photos are searched whole and in overlapping
//!   tiles so small faces in group photos are found.
//! - [`align`]: the similarity transform taking a face's eyes, nose and mouth to the 112×112
//!   template recognisers are trained on, and the warp.
//! - [`embed`]: face recognisers (SFace, AuraFace) read from their `.onnx` files: an aligned
//!   crop in, a unit embedding out.
//! - [`landmarker`]: FaceMesh V2 (478 points) and Blendshape V2 (52 expression scores) from
//!   a Face Landmarker `.task` bundle; `eyeBlinkLeft` / `eyeBlinkRight` give closed eyes.
//! - [`cluster`]: agglomerative grouping of embeddings into people.
//! - [`quality`]: blur and exposure from pixels alone (no model).
//!
//! Every model runs on the CPU reference or the GPU through [`Engine`]; the GPU is checked
//! against the CPU by the caller (`specular-ai`).

pub mod align;
pub mod blazeface;
pub mod cluster;
pub mod embed;
pub mod landmarker;
pub mod quality;

use crate::gpu::{Context, GpuGraph};
use crate::{Error, Feed, Graph, Tensor};

/// An RGBA8 image, rows top to bottom, no padding.
#[derive(Clone, Copy, Debug)]
pub struct Image<'a> {
    pub width: usize,
    pub height: usize,
    pub rgba: &'a [u8],
}

impl<'a> Image<'a> {
    /// Fails when the data is shorter than the size says.
    pub fn new(width: usize, height: usize, rgba: &'a [u8]) -> Result<Image<'a>, Error> {
        let n = width.checked_mul(height).and_then(|p| p.checked_mul(4));
        if width == 0 || height == 0 || n.is_none_or(|n| rgba.len() < n) {
            return Err(Error::Feed(format!("image of {width}×{height}")));
        }
        Ok(Image {
            width,
            height,
            rgba,
        })
    }

    /// Channel `c` (0 red, 1 green, 2 blue) at pixel (x, y), as 0..=255.
    #[inline]
    fn at(&self, x: usize, y: usize, c: usize) -> f32 {
        self.rgba[(y * self.width + x) * 4 + c] as f32
    }

    /// Bilinear sample of the three colour channels at (x, y) in pixel units (pixel centres
    /// at +0.5); outside the image is black.
    pub fn sample(&self, x: f32, y: f32) -> [f32; 3] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let mut out = [0f32; 3];
        for (dy, wy) in [(0i64, 1.0 - ty), (1, ty)] {
            for (dx, wx) in [(0i64, 1.0 - tx), (1, tx)] {
                let (px, py) = (x0 + dx, y0 + dy);
                if px < 0 || py < 0 || px >= self.width as i64 || py >= self.height as i64 {
                    continue;
                }
                let w = wx * wy;
                for (c, o) in out.iter_mut().enumerate() {
                    *o += w * self.at(px as usize, py as usize, c);
                }
            }
        }
        out
    }
}

/// An owned RGBA8 image.
#[derive(Clone, Debug, PartialEq)]
pub struct Owned {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Owned {
    pub fn view(&self) -> Image<'_> {
        Image {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
        }
    }
}

/// `img` turned `quarters` quarter turns clockwise (0–3).
pub fn turn(img: Image, quarters: u8) -> Owned {
    let (w, h) = (img.width, img.height);
    let q = quarters % 4;
    let (nw, nh) = if q % 2 == 1 { (h, w) } else { (w, h) };
    let mut rgba = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        for x in 0..nw {
            // The source pixel of (x, y) in the turned image.
            let (sx, sy) = match q {
                1 => (y, h - 1 - x),
                2 => (w - 1 - x, h - 1 - y),
                3 => (w - 1 - y, x),
                _ => (x, y),
            };
            let s = (sy * w + sx) * 4;
            rgba[(y * nw + x) * 4..(y * nw + x) * 4 + 4].copy_from_slice(&img.rgba[s..s + 4]);
        }
    }
    Owned {
        width: nw,
        height: nh,
        rgba,
    }
}

/// A point of an image turned `quarters` clockwise (of size `w × h` before turning) back in
/// the unturned image (pixel edges: the inverse of [`turn`] on continuous coordinates).
pub fn unturn_point(p: [f32; 2], quarters: u8, w: f32, h: f32) -> [f32; 2] {
    match quarters % 4 {
        1 => [p[1], h - p[0]],
        2 => [w - p[0], h - p[1]],
        3 => [w - p[1], p[0]],
        _ => p,
    }
}

/// Faces of `img` whatever their roll. The detector runs on the picture as it is; when it
/// finds nothing, a quarter of what it finds is rolled more than [`STRONG_ROLL`] (faces on
/// their side are found poorly and askew), or what it finds scores [`WEAK_MEAN`] or less on
/// average (in a picture on its side or upside down it finds a face or two, weakly and with
/// upright keypoints), it also runs on the picture turned a quarter clockwise, a quarter
/// back and upside down, and the pass with the highest total score wins (a turned pass only
/// when one of its faces scores at least [`TURNED_BEST`]).
/// Faces of a turned pass are mapped back: boxes around the turned face, keypoints where
/// they are in `img` (so alignment straightens the face, and [`blazeface::Face::roll`] says
/// how it lies).
pub fn detect_any_roll(
    det: &mut blazeface::Detector,
    img: Image,
) -> Result<Vec<blazeface::Face>, Error> {
    let faces = det.detect(img)?;
    let rolled = faces
        .iter()
        .filter(|f| f.roll().abs() > STRONG_ROLL)
        .count();
    let total = |v: &[blazeface::Face]| v.iter().map(|f| f.score).sum::<f32>();
    if !faces.is_empty()
        && rolled * 4 <= faces.len()
        && total(&faces) > WEAK_MEAN * faces.len() as f32
    {
        return Ok(faces);
    }
    let (w, h) = (img.width as f32, img.height as f32);
    let mut best = (total(&faces), faces);
    for q in [1u8, 3, 2] {
        let t = turn(img, q);
        let found = det.detect(t.view())?;
        let sure = found.iter().any(|f| f.score >= TURNED_BEST);
        if !sure || total(&found) <= best.0 {
            continue;
        }
        let back: Vec<blazeface::Face> = found
            .into_iter()
            .map(|f| {
                let [x, y, fw, fh] = f.rect;
                let c: Vec<[f32; 2]> = [[x, y], [x + fw, y], [x + fw, y + fh], [x, y + fh]]
                    .iter()
                    .map(|&p| unturn_point(p, q, w, h))
                    .collect();
                let x0 = c.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
                let y0 = c.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
                let x1 = c.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
                let y1 = c.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
                blazeface::Face {
                    rect: [x0, y0, x1 - x0, y1 - y0],
                    points: f.points.map(|p| unturn_point(p, q, w, h)),
                    score: f.score,
                }
            })
            .collect();
        best = (total(&back), back);
    }
    Ok(best.1)
}

/// A face's roll past which [`detect_any_roll`] also looks at the picture turned (50°).
pub const STRONG_ROLL: f32 = 0.87;

/// A mean score at or under which [`detect_any_roll`] also looks at the picture turned. On
/// 12 group photos the upright pass scores 0.68–0.95 upright, 0.60–0.70 on a picture turned
/// a quarter or upside down.
pub const WEAK_MEAN: f32 = 0.75;

/// The best score a turned pass of [`detect_any_roll`] needs to count: on 16 photos without
/// faces and their turns, every false face scores 0.63–0.74.
pub const TURNED_BEST: f32 = 0.8;

/// `img` shrunk so its long side is at most `max_side` (never enlarged): an exact box
/// average by the largest whole factor, then a triangle filter for the rest.
pub fn shrink(img: Image, max_side: usize) -> Owned {
    let long = img.width.max(img.height);
    if long <= max_side {
        return Owned {
            width: img.width,
            height: img.height,
            rgba: img.rgba[..img.width * img.height * 4].to_vec(),
        };
    }
    let k = (long / max_side).max(1);
    let boxed = if k > 1 { box_down(img, k) } else { None };
    let src = boxed.as_ref().map(Owned::view).unwrap_or(img);
    let s = max_side as f32 / src.width.max(src.height) as f32;
    let w = ((src.width as f32 * s).round() as usize).max(1);
    let h = ((src.height as f32 * s).round() as usize).max(1);
    resample(src, w, h)
}

/// Averages `k`×`k` blocks (the last partial row and column dropped).
fn box_down(img: Image, k: usize) -> Option<Owned> {
    let (w, h) = (img.width / k, img.height / k);
    if w == 0 || h == 0 {
        return None;
    }
    let mut out = vec![0u8; w * h * 4];
    let area = (k * k) as u32;
    let mut acc = vec![0u32; w * 4];
    for oy in 0..h {
        acc.iter_mut().for_each(|a| *a = 0);
        for y in oy * k..(oy + 1) * k {
            let row = &img.rgba[y * img.width * 4..];
            for (ox, a) in acc.chunks_exact_mut(4).enumerate() {
                let px = &row[ox * k * 4..(ox + 1) * k * 4];
                for p in px.chunks_exact(4) {
                    a[0] += p[0] as u32;
                    a[1] += p[1] as u32;
                    a[2] += p[2] as u32;
                    a[3] += p[3] as u32;
                }
            }
        }
        let dst = &mut out[oy * w * 4..(oy + 1) * w * 4];
        for (d, a) in dst.iter_mut().zip(&acc) {
            *d = ((a + area / 2) / area) as u8;
        }
    }
    Some(Owned {
        width: w,
        height: h,
        rgba: out,
    })
}

/// Triangle-filter resample to `w`×`h` (the filter widened by the scale when shrinking).
pub fn resample(img: Image, w: usize, h: usize) -> Owned {
    let taps = |src: usize, dst: usize| -> Vec<(usize, Vec<f32>)> {
        let scale = src as f32 / dst as f32;
        let f = scale.max(1.0);
        (0..dst)
            .map(|o| {
                let c = (o as f32 + 0.5) * scale;
                let lo = ((c - f - 0.5).floor().max(0.0)) as usize;
                let hi = ((c + f - 0.5).ceil().max(0.0) as usize).min(src - 1);
                let mut ws: Vec<f32> = (lo..=hi)
                    .map(|i| (1.0 - ((i as f32 + 0.5) - c).abs() / f).max(0.0))
                    .collect();
                let sum: f32 = ws.iter().sum();
                if sum > 0.0 {
                    ws.iter_mut().for_each(|x| *x /= sum);
                }
                (lo, ws)
            })
            .collect()
    };
    let tx = taps(img.width, w);
    let ty = taps(img.height, h);
    // Horizontal pass into f32 rows, then vertical.
    let mut mid = vec![0f32; w * img.height * 4];
    for y in 0..img.height {
        let row = &img.rgba[y * img.width * 4..(y + 1) * img.width * 4];
        for (ox, (lo, ws)) in tx.iter().enumerate() {
            let mut a = [0f32; 4];
            for (k, &wt) in ws.iter().enumerate() {
                let p = &row[(lo + k) * 4..(lo + k) * 4 + 4];
                for c in 0..4 {
                    a[c] += wt * p[c] as f32;
                }
            }
            mid[(y * w + ox) * 4..(y * w + ox) * 4 + 4].copy_from_slice(&a);
        }
    }
    let mut out = vec![0u8; w * h * 4];
    for (oy, (lo, ws)) in ty.iter().enumerate() {
        for ox in 0..w {
            let mut a = [0f32; 4];
            for (k, &wt) in ws.iter().enumerate() {
                let p = &mid[((lo + k) * w + ox) * 4..((lo + k) * w + ox) * 4 + 4];
                for c in 0..4 {
                    a[c] += wt * p[c];
                }
            }
            for c in 0..4 {
                out[(oy * w + ox) * 4 + c] = a[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Owned {
        width: w,
        height: h,
        rgba: out,
    }
}

/// A 2-D affine map: (x, y) ↦ (a·x + b·y + c, d·x + e·y + f).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [f32; 6]);

impl Affine {
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let m = &self.0;
        (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5])
    }

    /// The inverse map (`None` when singular).
    pub fn inverse(&self) -> Option<Affine> {
        let [a, b, c, d, e, f] = self.0;
        let det = a * e - b * d;
        if det.abs() < 1e-12 || !det.is_finite() {
            return None;
        }
        let (ia, ib, id, ie) = (e / det, -b / det, -d / det, a / det);
        Some(Affine([
            ia,
            ib,
            -(ia * c + ib * f),
            id,
            ie,
            -(id * c + ie * f),
        ]))
    }
}

/// Warps `img` into a `w`×`h` crop: output pixel centre (x+½, y+½) reads the image at
/// `to_image` of it (bilinear, black outside). Returns `[1, 3, h, w]` with each channel
/// value `v` (0..=255) stored as `v·scale + offset`.
pub fn warp(img: Image, to_image: &Affine, w: usize, h: usize, scale: f32, offset: f32) -> Tensor {
    let mut out = vec![0f32; 3 * w * h];
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = to_image.apply(x as f32 + 0.5, y as f32 + 0.5);
            let p = img.sample(sx, sy);
            for c in 0..3 {
                out[c * w * h + y * w + x] = p[c] * scale + offset;
            }
        }
    }
    Tensor::f32(&[1, 3, h, w], out)
}

/// Where a model runs.
pub enum Engine {
    /// The CPU reference, with this many threads.
    Cpu(usize),
    Gpu(Box<GpuGraph>),
}

/// A built graph and the engine that runs it.
pub struct Runner {
    pub graph: Graph,
    pub engine: Engine,
}

impl Runner {
    /// Compiles `graph` for the GPU when a context is given (falls back to the CPU when the
    /// graph cannot be compiled), else runs it on the CPU.
    pub fn new(graph: Graph, gpu: Option<&Context>, threads: usize) -> Runner {
        let engine = match gpu.map(|c| GpuGraph::new(c, &graph)) {
            Some(Ok(g)) => Engine::Gpu(Box::new(g)),
            _ => Engine::Cpu(threads.max(1)),
        };
        Runner { graph, engine }
    }

    pub fn on_gpu(&self) -> bool {
        matches!(self.engine, Engine::Gpu(_))
    }

    pub fn run(&mut self, feeds: &[Feed]) -> Result<Vec<Tensor>, Error> {
        match &mut self.engine {
            Engine::Cpu(t) => crate::cpu::run(&self.graph, feeds, *t),
            Engine::Gpu(g) => g.run(feeds),
        }
    }

    /// Runs on the CPU reference whatever the engine (for checks).
    pub fn run_cpu(&self, feeds: &[Feed], threads: usize) -> Result<Vec<Tensor>, Error> {
        crate::cpu::run(&self.graph, feeds, threads.max(1))
    }
}

/// Unit length (left as it is when zero).
pub fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

/// The cosine of two vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut ab, mut aa, mut bb) = (0f32, 0f32, 0f32);
    for (&x, &y) in a.iter().zip(b) {
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    ab / (aa.sqrt() * bb.sqrt()).max(f32::MIN_POSITIVE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_inverse_round_trips() {
        let a = Affine([0.8, -0.3, 12.0, 0.3, 0.8, -4.0]);
        let i = a.inverse().unwrap();
        let (x, y) = a.apply(7.0, 9.0);
        let (u, v) = i.apply(x, y);
        assert!((u - 7.0).abs() < 1e-4 && (v - 9.0).abs() < 1e-4);
        assert!(Affine([1.0, 2.0, 0.0, 2.0, 4.0, 0.0]).inverse().is_none());
    }

    #[test]
    fn shrink_keeps_flat_colour_and_aspect() {
        let (w, h) = (1000, 600);
        let px = vec![[10u8, 200, 30, 255]; w * h].concat();
        let s = shrink(Image::new(w, h, &px).unwrap(), 256);
        assert_eq!((s.width, s.height), (256, 154));
        assert!(s.rgba.chunks(4).all(|p| p == [10, 200, 30, 255]));
        let same = shrink(Image::new(w, h, &px).unwrap(), 2000);
        assert_eq!((same.width, same.height), (w, h));
    }

    #[test]
    fn sampling_is_bilinear_between_pixel_centres() {
        let px = [0u8, 0, 0, 255, 100, 100, 100, 255];
        let img = Image::new(2, 1, &px).unwrap();
        assert_eq!(img.sample(1.0, 0.5)[0], 50.0);
        assert_eq!(img.sample(0.5, 0.5)[0], 0.0);
        assert_eq!(img.sample(1.5, 0.5)[0], 100.0);
        assert!(Image::new(3, 1, &px).is_err());
    }
}
