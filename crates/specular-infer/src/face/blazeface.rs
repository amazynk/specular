//! BlazeFace face detection (MediaPipe's `blaze_face_full_range.tflite`, or the short-range
//! `face_detector.tflite` inside a Face Landmarker bundle).
//!
//! The network gives, per SSD anchor, a box (centre offset and size) with six keypoints and
//! a logit. Anchors are one per feature-map cell of fixed unit size, laid out by stride:
//! full range has one layer of stride 4 (48×48 cells at 192 px), short range strides 8 (two
//! anchors per cell) and 16 (six per cell). Boxes are decoded relative to their anchor,
//! scored by a sigmoid, and overlapping ones blended by weighted non-maximum suppression
//! (each kept face is the score-weighted mean of the boxes it absorbed).
//!
//! The input is the image fitted into the square model input with black bars, values in
//! [-1, 1]. A photo is searched whole and, when large, in overlapping square tiles at two
//! finer scales ([`Detector::detect`]), so faces a few percent of the frame wide are found;
//! the results are merged by the same suppression.

use super::{Image, Runner, shrink};
use crate::gpu::Context;
use crate::tflite::Model;
use crate::{DType, Error, Feed, Graph, Tensor};

/// Keypoints in the network's order. "Right" is the person's right (the left of the image
/// for a face looking at the camera).
pub const RIGHT_EYE: usize = 0;
pub const LEFT_EYE: usize = 1;
pub const NOSE: usize = 2;
pub const MOUTH: usize = 3;
pub const RIGHT_EAR: usize = 4;
pub const LEFT_EAR: usize = 5;

/// A face found: a box and six keypoints in image pixels, and a score in 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// Left, top, width, height.
    pub rect: [f32; 4],
    pub points: [[f32; 2]; 6],
    pub score: f32,
}

impl Face {
    pub fn iou(&self, o: &Face) -> f32 {
        iou(&self.rect, &o.rect)
    }

    /// The face's roll: the angle of the line from its first eye keypoint to its second in
    /// the image, radians, clockwise (0 upright, ±π/2 on its side, ±π upside down).
    pub fn roll(&self) -> f32 {
        let [a, b] = [self.points[0], self.points[1]];
        (b[1] - a[1]).atan2(b[0] - a[0])
    }
}

/// Intersection over union of two (left, top, width, height) boxes.
pub fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    let inter = (x1 - x0).max(0.0) * (y1 - y0).max(0.0);
    let union = a[2] * a[3] + b[2] * b[3] - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// Which BlazeFace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    /// 192 px input, faces up to about 5 m.
    Full,
    /// 128 px input, selfie distance.
    Short,
}

impl Range {
    /// (stride, anchors per cell) per feature map.
    fn layers(self) -> &'static [(usize, usize)] {
        match self {
            Range::Full => &[(4, 1)],
            Range::Short => &[(8, 2), (16, 6)],
        }
    }

    /// The score below which a box is dropped.
    pub fn min_score(self) -> f32 {
        match self {
            Range::Full => 0.6,
            Range::Short => 0.5,
        }
    }
}

/// Anchor centres (x, y in 0..1) for a square input of `size`.
pub fn anchors(range: Range, size: usize) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    for &(stride, per) in range.layers() {
        let n = size.div_ceil(stride);
        for y in 0..n {
            for x in 0..n {
                for _ in 0..per {
                    out.push([(x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32]);
                }
            }
        }
    }
    out
}

/// IoU above which boxes are merged.
pub const NMS_IOU: f32 = 0.3;

/// Weighted non-maximum suppression: highest score first, every box overlapping it by more
/// than `iou_min` is absorbed, the kept box and keypoints are the score-weighted mean.
pub fn weighted_nms(mut faces: Vec<Face>, iou_min: f32) -> Vec<Face> {
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut out = Vec::new();
    let mut left = faces;
    while let Some(top) = left.first().cloned() {
        let (group, rest): (Vec<Face>, Vec<Face>) =
            left.into_iter().partition(|f| f.iou(&top) > iou_min);
        let group = if group.is_empty() {
            vec![top.clone()]
        } else {
            group
        };
        let total: f32 = group.iter().map(|f| f.score).sum();
        let mut m = Face {
            rect: [0.0; 4],
            points: [[0.0; 2]; 6],
            score: top.score,
        };
        for f in &group {
            let w = f.score / total;
            for i in 0..4 {
                m.rect[i] += w * f.rect[i];
            }
            for k in 0..6 {
                m.points[k][0] += w * f.points[k][0];
                m.points[k][1] += w * f.points[k][1];
            }
        }
        out.push(m);
        left = rest;
    }
    out
}

/// A loaded detector.
pub struct Detector {
    pub range: Range,
    /// Faces scoring below this are dropped (the range's default at first).
    pub min_score: f32,
    /// The model's square input side.
    pub size: usize,
    anchors: Vec<[f32; 2]>,
    runner: Runner,
}

/// A square region of the image fed to the network: its top-left corner and side, in image
/// pixels (it may reach past the image; the outside is black).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub x: f32,
    pub y: f32,
    pub side: f32,
}

impl Detector {
    /// Builds the network from a `.tflite` file's bytes.
    pub fn new(
        tflite: &[u8],
        range: Range,
        gpu: Option<&Context>,
        threads: usize,
    ) -> Result<Detector, Error> {
        let m = Model::parse(tflite)?;
        let [inp] = m.inputs[..] else {
            return Err(Error::Weights("BlazeFace has one input".into()));
        };
        let s = m.tensors[inp].shape.clone();
        if s.len() != 4 || s[1] != s[2] || s[3] != 3 {
            return Err(Error::Weights(format!("BlazeFace input {s:?}")));
        }
        let size = s[1];
        let anchors = anchors(range, size);
        let mut g = Graph::new();
        let x = g.input(&[1, 3, size, size], DType::F32)?;
        let outs = crate::tflite_graph::build(&m, &mut g, x, false)?;
        let [boxes, scores] = outs[..] else {
            return Err(Error::Weights("BlazeFace has two outputs".into()));
        };
        if g.shape(boxes) != [1, anchors.len(), 16] || g.shape(scores) != [1, anchors.len(), 1] {
            return Err(Error::Weights(format!(
                "BlazeFace outputs {:?}, {:?} for {} anchors",
                g.shape(boxes),
                g.shape(scores),
                anchors.len()
            )));
        }
        g.output(boxes)?;
        g.output(scores)?;
        Ok(Detector {
            range,
            min_score: range.min_score(),
            size,
            anchors,
            runner: Runner::new(g, gpu, threads),
        })
    }

    pub fn on_gpu(&self) -> bool {
        self.runner.on_gpu()
    }

    /// The network's input for `win` of `img`: the window resampled to the model's side,
    /// values in [-1, 1].
    pub fn input(&self, img: Image, win: Window) -> Tensor {
        let s = win.side / self.size as f32;
        let to_image = super::Affine([s, 0.0, win.x, 0.0, s, win.y]);
        // Average 2×2 sub-samples per output pixel when shrinking a lot (cheap antialias).
        if s > 2.0 {
            let mut acc: Option<Vec<f32>> = None;
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let a = super::Affine([
                    s,
                    0.0,
                    win.x + (dx - 0.5) * s,
                    0.0,
                    s,
                    win.y + (dy - 0.5) * s,
                ]);
                let t = super::warp(img, &a, self.size, self.size, 2.0 / 255.0, -1.0).to_f32();
                match &mut acc {
                    None => acc = Some(t),
                    Some(v) => v.iter_mut().zip(&t).for_each(|(a, b)| *a += b),
                }
            }
            let v: Vec<f32> = acc.unwrap_or_default().iter().map(|x| x * 0.25).collect();
            return Tensor::f32(&[1, 3, self.size, self.size], v);
        }
        super::warp(img, &to_image, self.size, self.size, 2.0 / 255.0, -1.0)
    }

    /// Faces in one window, in image pixels, before suppression.
    pub fn run_window(
        &mut self,
        img: Image,
        win: Window,
        min_score: f32,
    ) -> Result<Vec<Face>, Error> {
        let x = self.input(img, win);
        self.run_tensor(&x, win, min_score)
    }

    /// Faces from a prepared input tensor (see [`Detector::input`]).
    pub fn run_tensor(
        &mut self,
        x: &Tensor,
        win: Window,
        min_score: f32,
    ) -> Result<Vec<Face>, Error> {
        let out = self.runner.run(&[Feed::Tensor(x)])?;
        Ok(self.decode(&out[0].to_f32(), &out[1].to_f32(), win, min_score))
    }

    /// Raw outputs (boxes `[1, A, 16]`, logits `[1, A, 1]`) for a prepared input, on the
    /// engine or the CPU reference.
    pub fn raw(&mut self, x: &Tensor, cpu: bool) -> Result<Vec<Tensor>, Error> {
        if cpu {
            self.runner.run_cpu(&[Feed::Tensor(x)], 4)
        } else {
            self.runner.run(&[Feed::Tensor(x)])
        }
    }

    /// Decodes raw outputs for window `win`.
    pub fn decode(&self, boxes: &[f32], logits: &[f32], win: Window, min_score: f32) -> Vec<Face> {
        let s = self.size as f32;
        let mut out = Vec::new();
        for (i, a) in self.anchors.iter().enumerate() {
            let Some(&l) = logits.get(i) else { break };
            let score = 1.0 / (1.0 + (-l.clamp(-100.0, 100.0)).exp());
            if score < min_score {
                continue;
            }
            let r = &boxes[i * 16..i * 16 + 16];
            let cx = r[0] / s + a[0];
            let cy = r[1] / s + a[1];
            let (w, h) = (r[2] / s, r[3] / s);
            if !(w > 0.0 && h > 0.0 && cx.is_finite() && cy.is_finite()) {
                continue;
            }
            let px = |u: f32| win.x + u * win.side;
            let py = |v: f32| win.y + v * win.side;
            let mut points = [[0f32; 2]; 6];
            for (k, p) in points.iter_mut().enumerate() {
                *p = [px(r[4 + 2 * k] / s + a[0]), py(r[5 + 2 * k] / s + a[1])];
            }
            out.push(Face {
                rect: [
                    px(cx - w / 2.0),
                    py(cy - h / 2.0),
                    w * win.side,
                    h * win.side,
                ],
                points,
                score,
            });
        }
        out
    }

    /// The window fitting the whole image (letterboxed).
    pub fn whole(img: Image) -> Window {
        let (w, h) = (img.width as f32, img.height as f32);
        let side = w.max(h);
        Window {
            x: (w - side) / 2.0,
            y: (h - side) / 2.0,
            side,
        }
    }

    /// Faces in the whole image only (as MediaPipe's face detector does), suppressed.
    pub fn detect_whole(&mut self, img: Image) -> Result<Vec<Face>, Error> {
        let min = self.min_score;
        let small = shrink(img, self.size);
        let k = img.width.max(img.height) as f32 / small.width.max(small.height) as f32;
        let f = self.run_window(small.view(), Self::whole(small.view()), min)?;
        Ok(weighted_nms(
            f.into_iter().map(|f| f.scaled(k)).collect(),
            NMS_IOU,
        ))
    }

    /// Faces in the whole image and in overlapping tiles of finer levels (see [`levels`]),
    /// suppressed together. Each level is the photo shrunk so a tile is exactly the model's
    /// input, so nothing is sampled twice or aliased.
    pub fn detect(&mut self, img: Image) -> Result<Vec<Face>, Error> {
        let min = self.min_score;
        let long = img.width.max(img.height) as f32;
        let mut all = Vec::new();
        let mut prev: Option<super::Owned> = None;
        for div in levels(img.width, img.height, self.size).into_iter().rev() {
            let src = prev.as_ref().map(|p| p.view()).unwrap_or(img);
            let lvl = shrink(src, self.size * div);
            let v = lvl.view();
            let k = long / lvl.width.max(lvl.height) as f32;
            let wins = if div == 1 {
                vec![Self::whole(v)]
            } else {
                grid(lvl.width, lvl.height, self.size)
            };
            for win in wins {
                let f = self.run_window(v, win, min)?;
                // A face cut by a tile's inner edge is found whole by a neighbour or the
                // coarser level: keep faces well inside the tile, or at the image's edge.
                all.extend(
                    f.into_iter()
                        .filter(|f| inside(f, win, v))
                        .map(|f| f.scaled(k)),
                );
            }
            prev = Some(lvl);
        }
        Ok(weighted_nms(all, NMS_IOU))
    }
}

impl Face {
    /// The face with every coordinate multiplied by `k`.
    pub fn scaled(mut self, k: f32) -> Face {
        self.rect.iter_mut().for_each(|v| *v *= k);
        for p in &mut self.points {
            p[0] *= k;
            p[1] *= k;
        }
        self
    }
}

/// Whether `f` lies inside `win` (with a margin) except where the window meets the image's
/// own edge.
fn inside(f: &Face, win: Window, img: Image) -> bool {
    let m = 0.02 * win.side;
    let (l, t, r, b) = (win.x, win.y, win.x + win.side, win.y + win.side);
    let (fl, ft) = (f.rect[0], f.rect[1]);
    let (fr, fb) = (fl + f.rect[2], ft + f.rect[3]);
    (fl >= l + m || l <= 0.0)
        && (ft >= t + m || t <= 0.0)
        && (fr <= r - m || r >= img.width as f32)
        && (fb <= b - m || b >= img.height as f32)
}

/// The levels a `w`×`h` photo is searched at, as the long side over the model's input:
/// 1 (whole), 2 and 4 (tiles about ½ and ¼ of the long side), a finer level only when the
/// photo has the pixels for it.
pub fn levels(w: usize, h: usize, model: usize) -> Vec<usize> {
    let long = w.max(h);
    [1usize, 2, 4]
        .into_iter()
        .filter(|&d| d == 1 || long * 10 >= model * d * 9)
        .collect()
}

/// Square windows of `side` covering a `w`×`h` image, overlapping by at least a quarter.
pub fn grid(w: usize, h: usize, side: usize) -> Vec<Window> {
    let side = side as f32;
    let step = side * 0.75;
    let along = |len: f32| -> Vec<f32> {
        if len <= side {
            return vec![(len - side) / 2.0];
        }
        let n = ((len - side) / step).ceil() as usize + 1;
        let gap = (len - side) / (n - 1) as f32;
        (0..n).map(|i| i as f32 * gap).collect()
    };
    let mut out = Vec::new();
    for y in along(h as f32) {
        for x in along(w as f32) {
            out.push(Window { x, y, side });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_counts_match_the_models() {
        assert_eq!(anchors(Range::Full, 192).len(), 2304);
        assert_eq!(anchors(Range::Short, 128).len(), 896);
        let a = anchors(Range::Full, 192);
        assert_eq!(a[0], [0.5 / 48.0, 0.5 / 48.0]);
        assert_eq!(a[49], [1.5 / 48.0, 1.5 / 48.0]);
    }

    fn face(x: f32, y: f32, s: f32, score: f32) -> Face {
        Face {
            rect: [x, y, s, s],
            points: [[x, y]; 6],
            score,
        }
    }

    #[test]
    fn weighted_nms_blends_overlaps_and_keeps_apart_faces() {
        let out = weighted_nms(
            vec![
                face(0.0, 0.0, 10.0, 0.9),
                face(1.0, 0.0, 10.0, 0.3),
                face(50.0, 50.0, 10.0, 0.8),
            ],
            0.3,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].score, 0.9);
        assert!((out[0].rect[0] - 0.25).abs() < 1e-5, "{:?}", out[0].rect);
        assert_eq!(out[1].rect[0], 50.0);
    }

    #[test]
    fn levels_and_tiles_cover_large_photos_only() {
        assert_eq!(levels(300, 200, 192), [1]);
        assert_eq!(levels(400, 300, 192), [1, 2]);
        assert_eq!(levels(6000, 4000, 192), [1, 2, 4]);
        // The finest level of a 3:2 photo: 768×512 in 192 px tiles.
        let t = grid(768, 512, 192);
        assert_eq!(t.len(), 5 * 4);
        for (x, y) in [(0.0, 0.0), (767.0, 511.0), (384.0, 256.0), (10.0, 500.0)] {
            assert!(
                t.iter()
                    .any(|w| x >= w.x && y >= w.y && x < w.x + w.side && y < w.y + w.side)
            );
        }
        assert_eq!(grid(150, 100, 192).len(), 1);
    }

    #[test]
    fn iou_of_boxes() {
        assert_eq!(iou(&[0.0, 0.0, 2.0, 2.0], &[0.0, 0.0, 2.0, 2.0]), 1.0);
        assert_eq!(iou(&[0.0, 0.0, 2.0, 2.0], &[2.0, 0.0, 2.0, 2.0]), 0.0);
        assert!((iou(&[0.0, 0.0, 2.0, 2.0], &[1.0, 0.0, 2.0, 2.0]) - 1.0 / 3.0).abs() < 1e-6);
    }
}
