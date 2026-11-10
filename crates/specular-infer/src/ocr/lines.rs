//! Line images for the recognisers: a detected quad warped upright (bicubic, edges
//! replicated, as OpenCV's `warpPerspective` with `INTER_CUBIC` and `BORDER_REPLICATE`),
//! turned when it is much taller than wide, resized to the recogniser's height (bilinear,
//! half-pixel centres) and padded.

use super::db::Pt;
use crate::tensor::Tensor;

/// An RGBA8 image, rows top to bottom.
#[derive(Clone, Copy, Debug)]
pub struct Rgba<'a> {
    pub width: usize,
    pub height: usize,
    pub data: &'a [u8],
}

/// An 8-bit image with `channels` interleaved channels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Img8 {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub data: Vec<u8>,
}

impl Img8 {
    pub fn px(&self, x: usize, y: usize, c: usize) -> u8 {
        self.data[(y * self.width + x) * self.channels + c]
    }

    /// Turned a quarter counter-clockwise (numpy's `rot90`).
    pub fn rot90(&self) -> Img8 {
        let (w, h, ch) = (self.width, self.height, self.channels);
        let mut data = vec![0u8; w * h * ch];
        // New size h' = w, w' = h; new[i][j] = old[j][w-1-i].
        for i in 0..w {
            for j in 0..h {
                for c in 0..ch {
                    data[(i * h + j) * ch + c] = self.data[(j * w + (w - 1 - i)) * ch + c];
                }
            }
        }
        Img8 {
            width: h,
            height: w,
            channels: ch,
            data,
        }
    }

    /// Turned half a turn (upside down).
    pub fn rot180(&self) -> Img8 {
        let ch = self.channels.max(1);
        let mut data = Vec::with_capacity(self.data.len());
        for px in self.data.chunks_exact(ch).rev() {
            data.extend_from_slice(px);
        }
        Img8 {
            data,
            ..self.clone()
        }
    }

    /// One channel: luma (ITU-R 601 weights, as OpenCV's and Leptonica's grey conversions).
    pub fn grey(&self) -> Img8 {
        if self.channels == 1 {
            return self.clone();
        }
        let data = self
            .data
            .chunks_exact(self.channels)
            .map(|p| {
                let v = 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
                v.round().clamp(0.0, 255.0) as u8
            })
            .collect();
        Img8 {
            width: self.width,
            height: self.height,
            channels: 1,
            data,
        }
    }
}

/// The homography taking (u, v) to (x, y) for four correspondences, or None when they are
/// degenerate.
fn homography(src: [Pt; 4], dst: [Pt; 4]) -> Option<[f64; 9]> {
    let mut a = [[0f64; 9]; 8];
    for i in 0..4 {
        let (u, v) = (f64::from(src[i][0]), f64::from(src[i][1]));
        let (x, y) = (f64::from(dst[i][0]), f64::from(dst[i][1]));
        a[2 * i] = [u, v, 1.0, 0.0, 0.0, 0.0, -u * x, -v * x, x];
        a[2 * i + 1] = [0.0, 0.0, 0.0, u, v, 1.0, -u * y, -v * y, y];
    }
    // Gaussian elimination with partial pivoting on the 8×8 system.
    for c in 0..8 {
        let p = (c..8).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        let pivot = a[c];
        for (r, row) in a.iter_mut().enumerate() {
            if r != c {
                let f = row[c] / pivot[c];
                for (x, p) in row.iter_mut().zip(pivot.iter()).skip(c) {
                    *x -= f * p;
                }
            }
        }
    }
    let mut h = [0f64; 9];
    for i in 0..8 {
        h[i] = a[i][8] / a[i][i];
    }
    h[8] = 1.0;
    Some(h)
}

/// OpenCV's bicubic weights (A = −0.75) for fraction `t`.
fn cubic(t: f32) -> [f32; 4] {
    const A: f32 = -0.75;
    let w0 = ((A * (t + 1.0) - 5.0 * A) * (t + 1.0) + 8.0 * A) * (t + 1.0) - 4.0 * A;
    let w1 = ((A + 2.0) * t - (A + 3.0)) * t * t + 1.0;
    let w2 = ((A + 2.0) * (1.0 - t) - (A + 3.0)) * (1.0 - t) * (1.0 - t) + 1.0;
    [w0, w1, w2, 1.0 - w0 - w1 - w2]
}

/// The quad `pts` (top-left, top-right, bottom-right, bottom-left) warped to an upright
/// RGB image as wide as its longer horizontal side and as high as its longer vertical
/// side; turned a quarter when its height is at least 1.5 times its width.
pub fn crop_quad(img: &Rgba, pts: [Pt; 4]) -> Option<Img8> {
    let d = |a: Pt, b: Pt| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let w = d(pts[0], pts[1]).max(d(pts[2], pts[3])) as usize;
    let h = d(pts[0], pts[3]).max(d(pts[1], pts[2])) as usize;
    if w == 0 || h == 0 || img.width == 0 || img.height == 0 || w > 16384 || h > 16384 {
        return None;
    }
    let (wf, hf) = (w as f32, h as f32);
    let hm = homography([[0.0, 0.0], [wf, 0.0], [wf, hf], [0.0, hf]], pts)?;
    let mut out = vec![0u8; w * h * 3];
    let (iw, ih) = (img.width as isize, img.height as isize);
    for v in 0..h {
        for u in 0..w {
            let (uf, vf) = (u as f64, v as f64);
            let z = hm[6] * uf + hm[7] * vf + hm[8];
            let sx = ((hm[0] * uf + hm[1] * vf + hm[2]) / z) as f32;
            let sy = ((hm[3] * uf + hm[4] * vf + hm[5]) / z) as f32;
            let (x0, y0) = (sx.floor(), sy.floor());
            let (wx, wy) = (cubic(sx - x0), cubic(sy - y0));
            let (x0, y0) = (x0 as isize - 1, y0 as isize - 1);
            for c in 0..3 {
                let mut acc = 0f32;
                for (j, wyj) in wy.iter().enumerate() {
                    let yy = (y0 + j as isize).clamp(0, ih - 1) as usize;
                    let row = &img.data[yy * img.width * 4..];
                    let mut r = 0f32;
                    for (i, wxi) in wx.iter().enumerate() {
                        let xx = (x0 + i as isize).clamp(0, iw - 1) as usize;
                        r += wxi * f32::from(row[xx * 4 + c]);
                    }
                    acc += wyj * r;
                }
                out[(v * w + u) * 3 + c] = acc.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let crop = Img8 {
        width: w,
        height: h,
        channels: 3,
        data: out,
    };
    Some(if hf / wf >= 1.5 { crop.rot90() } else { crop })
}

/// Bilinear resize with half-pixel centres, edges clamped (OpenCV's `INTER_LINEAR`, which
/// does not antialias when shrinking), rounded to 8 bits.
pub fn resize(img: &Img8, w: usize, h: usize) -> Img8 {
    let ch = img.channels;
    let mut data = vec![0u8; w * h * ch];
    if img.width == 0 || img.height == 0 {
        return Img8 {
            width: w,
            height: h,
            channels: ch,
            data,
        };
    }
    let sx = img.width as f32 / w as f32;
    let sy = img.height as f32 / h as f32;
    let tap = |o: usize, s: f32, n: usize| -> (usize, usize, f32) {
        let f = ((o as f32 + 0.5) * s - 0.5).max(0.0);
        let i0 = (f.floor() as usize).min(n - 1);
        let i1 = (i0 + 1).min(n - 1);
        (i0, i1, f - i0 as f32)
    };
    for y in 0..h {
        let (y0, y1, fy) = tap(y, sy, img.height);
        for x in 0..w {
            let (x0, x1, fx) = tap(x, sx, img.width);
            for c in 0..ch {
                let p = |xx: usize, yy: usize| f32::from(img.px(xx, yy, c));
                let top = p(x0, y0) * (1.0 - fx) + p(x1, y0) * fx;
                let bot = p(x0, y1) * (1.0 - fx) + p(x1, y1) * fx;
                data[(y * w + x) * ch + c] =
                    (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Img8 {
        width: w,
        height: h,
        channels: ch,
        data,
    }
}

/// Paddle's input width for one line of aspect `w/h`: at least 320 (⅔ of 48 × 10),
/// otherwise 48 · ratio, at most [`super::paddle::REC_MAX_WIDTH`].
pub fn rec_width(w: usize, h: usize) -> usize {
    let ratio = w as f64 / h.max(1) as f64;
    ((48.0 * ratio.max(320.0 / 48.0)) as usize).clamp(320, super::paddle::REC_MAX_WIDTH)
}

/// The recogniser input for `lines` at `width`: each line resized to 48 high and as wide
/// as its aspect asks (at most `width`), normalised to [−1, 1], the rest zeros.
pub fn rec_batch(lines: &[&Img8], width: usize) -> Tensor {
    let hh = super::paddle::REC_HEIGHT;
    let n = lines.len();
    let plane = hh * width;
    let mut v = vec![0f32; n * 3 * plane];
    for (b, l) in lines.iter().enumerate() {
        let ratio = l.width as f64 / l.height.max(1) as f64;
        let rw = ((hh as f64 * ratio).ceil() as usize).clamp(1, width);
        let r = resize(l, rw, hh);
        for y in 0..hh {
            for x in 0..rw {
                for c in 0..3 {
                    v[b * 3 * plane + c * plane + y * width + x] =
                        super::paddle::rec_normalise(r.px(x, y, c.min(r.channels - 1)));
                }
            }
        }
    }
    Tensor::f32(&[n, 3, hh, width], v)
}

/// The orientation classifier's input for `lines`: each stretched to 160 × 80 and
/// normalised as the detector's input (ImageNet mean and deviation), `[N, 3, 80, 160]`.
pub fn cls_batch(lines: &[&Img8]) -> Tensor {
    let (hh, ww) = (super::CLS_HEIGHT, super::CLS_WIDTH);
    let plane = hh * ww;
    let mut v = vec![0f32; lines.len() * 3 * plane];
    for (b, l) in lines.iter().enumerate() {
        let r = resize(l, ww, hh);
        for y in 0..hh {
            for x in 0..ww {
                for c in 0..3 {
                    let p = f32::from(r.px(x, y, c.min(r.channels - 1))) / 255.0;
                    v[b * 3 * plane + c * plane + y * ww + x] =
                        (p - super::paddle::DET_MEAN[c]) / super::paddle::DET_STD[c];
                }
            }
        }
    }
    Tensor::f32(&[lines.len(), 3, hh, ww], v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rot180_twice_is_the_same() {
        let a = Img8 {
            width: 3,
            height: 2,
            channels: 2,
            data: (0..12).collect(),
        };
        let b = a.rot180();
        assert_eq!(b.data, vec![10, 11, 8, 9, 6, 7, 4, 5, 2, 3, 0, 1]);
        assert_eq!(b.rot180(), a);
    }

    #[test]
    fn axis_aligned_crop_copies_pixels() {
        let (w, h) = (20, 10);
        let mut d = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                d[(y * w + x) * 4] = (x * 10) as u8;
                d[(y * w + x) * 4 + 1] = (y * 20) as u8;
            }
        }
        let img = Rgba {
            width: w,
            height: h,
            data: &d,
        };
        let c = crop_quad(&img, [[2.0, 3.0], [12.0, 3.0], [12.0, 8.0], [2.0, 8.0]]).unwrap();
        assert_eq!((c.width, c.height), (10, 5));
        assert_eq!(c.px(0, 0, 0), 20);
        assert_eq!(c.px(4, 2, 0), 60);
        assert_eq!(c.px(4, 2, 1), 100);
    }

    #[test]
    fn tall_crops_are_turned() {
        let d = vec![9u8; 10 * 40 * 4];
        let img = Rgba {
            width: 10,
            height: 40,
            data: &d,
        };
        let c = crop_quad(&img, [[0.0, 0.0], [6.0, 0.0], [6.0, 30.0], [0.0, 30.0]]).unwrap();
        assert_eq!((c.width, c.height), (30, 6));
    }

    #[test]
    fn widths_follow_paddle() {
        assert_eq!(rec_width(100, 48), 320);
        assert_eq!(rec_width(1000, 48), 1000);
        assert_eq!(rec_width(100_000, 48), 3200);
    }
}
