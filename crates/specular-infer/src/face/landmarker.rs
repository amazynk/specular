//! MediaPipe Face Landmarker (`face_landmarker.task`, a zip of three TFLite models):
//! the short-range BlazeFace, FaceMesh V2 (478 3-D points from a 256×256 crop) and
//! Blendshape V2 (52 expression scores from 146 of those points). Culling uses
//! `eyeBlinkLeft` and `eyeBlinkRight` for closed eyes.
//!
//! The mesh crop follows the face: centred on the detector's box, turned so the eyes are
//! level, a square 1.5 times the box's long side. The points come back in crop pixels and are
//! mapped to the image; the blendshape model reads the chosen 146 as image pixels.

use super::blazeface::{Detector, Face, LEFT_EYE, RIGHT_EYE, Range};
use super::{Affine, Image, Runner};
use crate::gpu::Context;
use crate::tflite::{Model, zip_entry};
use crate::{DType, Error, Feed, Graph, Tensor};

/// The bundle's files.
pub const DETECTOR: &str = "face_detector.tflite";
pub const MESH: &str = "face_landmarks_detector.tflite";
pub const BLENDSHAPES: &str = "face_blendshapes.tflite";

/// Number of mesh points.
pub const POINTS: usize = 478;
/// Number of blendshape scores.
pub const SHAPES: usize = 52;
/// Blendshape indices of closed eyes (the person's left and right).
pub const EYE_BLINK_LEFT: usize = 9;
pub const EYE_BLINK_RIGHT: usize = 10;

/// The mesh points the blendshape model reads, in its order. Data from MediaPipe's face
/// blendshapes graph (Apache-2.0), checked against the shipped library: the model is
/// trained on exactly these.
pub const BLENDSHAPE_POINTS: [u16; 146] = [
    0, 1, 4, 5, 6, 7, 8, 10, 13, 14, 17, 21, 33, 37, 39, 40, 46, 52, 53, 54, 55, 58, 61, 63, 65,
    66, 67, 70, 78, 80, 81, 82, 84, 87, 88, 91, 93, 95, 103, 105, 107, 109, 127, 132, 133, 136,
    144, 145, 146, 148, 149, 150, 152, 153, 154, 155, 157, 158, 159, 160, 161, 162, 163, 168, 172,
    173, 176, 178, 181, 185, 191, 195, 197, 234, 246, 249, 251, 263, 267, 269, 270, 276, 282, 283,
    284, 285, 288, 291, 293, 295, 296, 297, 300, 308, 310, 311, 312, 314, 317, 318, 321, 323, 324,
    332, 334, 336, 338, 356, 361, 362, 365, 373, 374, 375, 377, 378, 379, 380, 381, 382, 384, 385,
    386, 387, 388, 389, 390, 397, 398, 400, 402, 405, 409, 415, 454, 466, 468, 469, 470, 471, 472,
    473, 474, 475, 476, 477,
];

/// How much larger than the detector's box the mesh crop is.
pub const ROI_SCALE: f32 = 1.5;

/// The mesh of one face.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Points in image pixels (z in the same scale, relative).
    pub points: Vec<[f32; 3]>,
    /// The model's face-presence score, 0..1.
    pub presence: f32,
}

/// Closed-eye scores of one face, 0 (open) to 1 (closed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eyes {
    /// The person's left eye.
    pub left: f32,
    pub right: f32,
    pub presence: f32,
}

impl Eyes {
    /// Both eyes' closure: the more open eye decides (a wink is not closed eyes).
    pub fn closed(&self) -> f32 {
        self.left.min(self.right)
    }
}

/// A loaded Face Landmarker.
pub struct Landmarker {
    pub detector: Detector,
    mesh: Runner,
    shapes: Runner,
    mesh_size: usize,
}

impl Landmarker {
    /// Reads the three models from a `.task` bundle's bytes.
    pub fn new(task: &[u8], gpu: Option<&Context>, threads: usize) -> Result<Landmarker, Error> {
        let detector = Detector::new(zip_entry(task, DETECTOR)?, Range::Short, gpu, threads)?;
        let (mesh, mesh_size) = {
            let m = Model::parse(zip_entry(task, MESH)?)?;
            let [inp] = m.inputs[..] else {
                return Err(Error::Weights("FaceMesh has one input".into()));
            };
            let s = m.tensors[inp].shape.clone();
            if s.len() != 4 || s[1] != s[2] || s[3] != 3 {
                return Err(Error::Weights(format!("FaceMesh input {s:?}")));
            }
            let mut g = Graph::new();
            let x = g.input(&[1, 3, s[1], s[1]], DType::F32)?;
            let outs = crate::tflite_graph::build(&m, &mut g, x, false)?;
            if outs.len() < 2 || g.shape(outs[0]).iter().product::<usize>() != POINTS * 3 {
                return Err(Error::Weights("FaceMesh outputs".into()));
            }
            g.output(outs[0])?;
            g.output(outs[1])?;
            (Runner::new(g, gpu, threads), s[1])
        };
        let shapes = {
            let m = Model::parse(zip_entry(task, BLENDSHAPES)?)?;
            let mut g = Graph::new();
            let x = g.input(&[1, BLENDSHAPE_POINTS.len(), 2], DType::F32)?;
            let outs = crate::tflite_graph::build(&m, &mut g, x, false)?;
            let [y] = outs[..] else {
                return Err(Error::Weights("Blendshape has one output".into()));
            };
            if g.shape(y).iter().product::<usize>() != SHAPES {
                return Err(Error::Weights("Blendshape output".into()));
            }
            g.output(y)?;
            // Small (one face's points): the CPU is as quick as a GPU round trip.
            Runner::new(g, None, threads)
        };
        Ok(Landmarker {
            detector,
            mesh,
            shapes,
            mesh_size,
        })
    }

    pub fn on_gpu(&self) -> bool {
        self.mesh.on_gpu()
    }

    /// The map from mesh-crop pixels to image pixels for face `f`.
    pub fn roi(&self, f: &Face) -> Affine {
        roi(f, self.mesh_size)
    }

    /// The mesh input for face `f`: the crop, RGB in [0, 1].
    pub fn mesh_input(&self, img: Image, f: &Face) -> Tensor {
        let m = self.roi(f);
        super::warp(img, &m, self.mesh_size, self.mesh_size, 1.0 / 255.0, 0.0)
    }

    /// Raw mesh outputs for a prepared input (points in crop pixels, presence).
    pub fn mesh_raw(&mut self, x: &Tensor, cpu: bool) -> Result<(Vec<f32>, f32), Error> {
        let out = if cpu {
            self.mesh.run_cpu(&[Feed::Tensor(x)], 4)?
        } else {
            self.mesh.run(&[Feed::Tensor(x)])?
        };
        let p = out[1].to_f32().first().copied().unwrap_or(0.0);
        Ok((out[0].to_f32(), p))
    }

    /// The 478-point mesh of face `f`, in image pixels.
    pub fn mesh(&mut self, img: Image, f: &Face) -> Result<Mesh, Error> {
        let x = self.mesh_input(img, f);
        let (raw, presence) = self.mesh_raw(&x, false)?;
        let m = self.roi(f);
        // z is in crop pixels; scale it like x and y.
        let k = (m.0[0] * m.0[0] + m.0[3] * m.0[3]).sqrt();
        let points = raw
            .chunks_exact(3)
            .map(|p| {
                let (x, y) = m.apply(p[0], p[1]);
                [x, y, p[2] * k]
            })
            .collect();
        Ok(Mesh {
            points,
            presence: presence_prob(presence),
        })
    }

    /// The blendshape model's input for a mesh.
    pub fn shapes_input(mesh: &Mesh) -> Tensor {
        let mut v = Vec::with_capacity(BLENDSHAPE_POINTS.len() * 2);
        for &i in &BLENDSHAPE_POINTS {
            let p = mesh.points.get(i as usize).copied().unwrap_or([0.0; 3]);
            v.push(p[0]);
            v.push(p[1]);
        }
        Tensor::f32(&[1, BLENDSHAPE_POINTS.len(), 2], v)
    }

    /// The 52 blendshape scores of a prepared input.
    pub fn shapes_raw(&mut self, x: &Tensor) -> Result<Vec<f32>, Error> {
        Ok(self.shapes.run(&[Feed::Tensor(x)])?[0].to_f32())
    }

    /// Closed-eye scores of face `f`.
    pub fn eyes(&mut self, img: Image, f: &Face) -> Result<Eyes, Error> {
        let mesh = self.mesh(img, f)?;
        let s = self.shapes_raw(&Self::shapes_input(&mesh))?;
        Ok(Eyes {
            left: s.get(EYE_BLINK_LEFT).copied().unwrap_or(0.0),
            right: s.get(EYE_BLINK_RIGHT).copied().unwrap_or(0.0),
            presence: mesh.presence,
        })
    }
}

/// The presence output as a probability (the model may give a logit).
fn presence_prob(v: f32) -> f32 {
    if (0.0..=1.0).contains(&v) {
        v
    } else {
        1.0 / (1.0 + (-v).exp())
    }
}

/// The map from a `size`×`size` mesh crop to image pixels: centred on the face's box,
/// rotated so the eyes are level, `ROI_SCALE` times the box's long side.
pub fn roi(f: &Face, size: usize) -> Affine {
    let [x, y, w, h] = f.rect;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let [rx, ry] = f.points[RIGHT_EYE];
    let [lx, ly] = f.points[LEFT_EYE];
    // The angle of the line from the image-left eye to the image-right one.
    let angle = (ly - ry).atan2(lx - rx);
    let side = w.max(h) * ROI_SCALE;
    let s = side / size as f32;
    let (c, sn) = (angle.cos() * s, angle.sin() * s);
    let half = size as f32 / 2.0;
    // crop (u, v) → image: rotate and scale about the crop centre, then move to (cx, cy).
    Affine([
        c,
        -sn,
        cx - c * half + sn * half,
        sn,
        c,
        cy - sn * half - c * half,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subset_is_sorted_and_in_range() {
        assert!(BLENDSHAPE_POINTS.windows(2).all(|w| w[0] < w[1]));
        assert!(BLENDSHAPE_POINTS.iter().all(|&i| (i as usize) < POINTS));
    }

    #[test]
    fn roi_is_centred_and_follows_the_eyes() {
        let mut f = Face {
            rect: [100.0, 100.0, 40.0, 40.0],
            points: [[0.0; 2]; 6],
            score: 1.0,
        };
        f.points[RIGHT_EYE] = [110.0, 110.0];
        f.points[LEFT_EYE] = [130.0, 110.0];
        let m = roi(&f, 256);
        let (x, y) = m.apply(128.0, 128.0);
        assert!((x - 120.0).abs() < 1e-3 && (y - 120.0).abs() < 1e-3);
        // Level eyes: the crop's top-left is up and left of the box by half the side.
        let (x0, y0) = m.apply(0.0, 0.0);
        assert!((x0 - 90.0).abs() < 1e-3 && (y0 - 90.0).abs() < 1e-3);
        // Eyes on a 90° line: the crop turns with them.
        f.points[LEFT_EYE] = [110.0, 130.0];
        let m = roi(&f, 256);
        let (x1, y1) = m.apply(256.0, 128.0);
        assert!((x1 - 120.0).abs() < 1e-3 && (y1 - 150.0).abs() < 1e-3);
    }
}
