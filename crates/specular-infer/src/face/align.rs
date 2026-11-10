//! Face alignment for recognisers: the least-squares similarity transform (scale, rotation,
//! shift; no shear) taking the detector's eyes, nose and mouth to where a 112×112 training
//! crop has them, and the crop warped through it.
//!
//! The template is the usual one for 112×112 face recognisers (eye centres, nose tip and
//! mouth corners); BlazeFace gives the mouth centre, matched to the corners' midpoint.

use super::blazeface::{Face, LEFT_EYE, MOUTH, NOSE, RIGHT_EYE};
use super::{Affine, Image, warp};
use crate::Tensor;

/// Side of the aligned crop.
pub const SIDE: usize = 112;

/// Template positions in the 112×112 crop: the image-left eye (the person's right), the
/// image-right eye, the nose tip, the mouth centre.
pub const TEMPLATE: [[f32; 2]; 4] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [56.1396, 92.2848],
];

/// The similarity `s·R·p + t` closest (least squares) to taking each `from[i]` to `to[i]`.
pub fn similarity(from: &[[f32; 2]], to: &[[f32; 2]]) -> Option<Affine> {
    let n = from.len().min(to.len());
    if n < 2 {
        return None;
    }
    let mean = |p: &[[f32; 2]]| {
        let (sx, sy) = p[..n]
            .iter()
            .fold((0f64, 0f64), |(a, b), q| (a + q[0] as f64, b + q[1] as f64));
        (sx / n as f64, sy / n as f64)
    };
    let (fx, fy) = mean(from);
    let (tx, ty) = mean(to);
    let (mut a, mut b, mut var) = (0f64, 0f64, 0f64);
    for i in 0..n {
        let (px, py) = (from[i][0] as f64 - fx, from[i][1] as f64 - fy);
        let (qx, qy) = (to[i][0] as f64 - tx, to[i][1] as f64 - ty);
        a += px * qx + py * qy;
        b += px * qy - py * qx;
        var += px * px + py * py;
    }
    if var <= 1e-12 {
        return None;
    }
    let (c, s) = (a / var, b / var);
    let ox = tx - (c * fx - s * fy);
    let oy = ty - (s * fx + c * fy);
    Some(Affine([
        c as f32, -s as f32, ox as f32, s as f32, c as f32, oy as f32,
    ]))
}

/// The detector's four points used for alignment, in template order.
pub fn points(f: &Face) -> [[f32; 2]; 4] {
    [
        f.points[RIGHT_EYE],
        f.points[LEFT_EYE],
        f.points[NOSE],
        f.points[MOUTH],
    ]
}

/// The map from crop pixels to image pixels for face `f` (None for degenerate points).
pub fn crop_to_image(f: &Face) -> Option<Affine> {
    similarity(&points(f), &TEMPLATE)?.inverse()
}

/// The aligned 112×112 crop of face `f`, `[1, 3, 112, 112]` RGB, values 0..=255.
pub fn crop(img: Image, f: &Face) -> Option<Tensor> {
    let m = crop_to_image(f)?;
    Some(warp(img, &m, SIDE, SIDE, 1.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_recovers_a_known_transform() {
        let truth = Affine([0.6, -0.8, 30.0, 0.8, 0.6, -5.0]);
        let from = [[10.0, 20.0], [50.0, 22.0], [31.0, 40.0], [30.0, 60.0]];
        let to: Vec<[f32; 2]> = from
            .iter()
            .map(|p| {
                let (x, y) = truth.apply(p[0], p[1]);
                [x, y]
            })
            .collect();
        let m = similarity(&from, &to).unwrap();
        for (a, b) in m.0.iter().zip(truth.0) {
            assert!((a - b).abs() < 1e-3, "{:?}", m);
        }
        assert!(similarity(&[[1.0, 1.0]; 4], &TEMPLATE).is_none());
    }

    #[test]
    fn a_face_at_the_template_crops_to_itself() {
        let mut points = [[0f32; 2]; 6];
        points[..4].copy_from_slice(&TEMPLATE);
        let f = Face {
            rect: [0.0, 0.0, 112.0, 112.0],
            points,
            score: 1.0,
        };
        let m = crop_to_image(&f).unwrap();
        let (x, y) = m.apply(56.0, 56.0);
        assert!((x - 56.0).abs() < 1e-3 && (y - 56.0).abs() < 1e-3);
    }
}
