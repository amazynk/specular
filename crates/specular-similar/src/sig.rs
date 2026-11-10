//! Tier 1 and tier 2 signatures and the quality measures, all from one oriented thumbnail.
//!
//! The thumbnail engine already decodes every image to a small, oriented RGBA thumbnail; the
//! signature is computed from those pixels, so hashing never decodes a file a second time.
//! Everything here works on a luma (and, for the colour moments, YCbCr) plane averaged down
//! with an area filter, which is what makes the hashes stable across re-encodes and resizes.

/// The thumbnail class (longest side, px) the signatures are computed from.
pub const SIG_CLASS: u32 = 256;

/// Bytes of [`Signature::to_bytes`].
pub const SIG_BYTES: usize = 8 + 8 + 32 + 6 + 4 + 4 * QUALITY_FLOATS;
const QUALITY_FLOATS: usize = 16 + 6;

/// Luma step (0–255) a dHash comparison must clear.
const DHASH_DEAD_ZONE: f32 = 1.5;

/// Hashes and measures of one image.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Signature {
    /// Tier 1: the pixels at a canonical 16×16, coarsely quantised. Equal for the same pixels
    /// through the same decode path (a metadata-only edit, a pixel-equal re-save); across
    /// decoders (JPEG DCT scaling vs a PNG export) it may differ, and tier 2 + SSIM catch it.
    pub tier1: u64,
    /// Tier 2: 64-bit DCT perceptual hash of the 32×32 luma.
    pub phash: u64,
    /// Tier 2: 256-bit horizontal difference hash of the 17×16 luma.
    pub dhash: [u64; 4],
    /// Tier 2: mean and standard deviation of Y, Cb and Cr (0–255 each).
    pub moments: [u8; 6],
    /// Width / height of the oriented image.
    pub aspect: f32,
    pub quality: Quality,
}

/// Measures for the best-pick score, from the same thumbnail.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Quality {
    /// RMS of the Laplacian per cell of a 4×4 grid (row-major): detail, i.e. focus, by region.
    pub grid: [f32; 16],
    /// 0 when gradients point every way, towards 1 when they all share one direction (the
    /// smear of motion blur, relative to the other shots of the scene).
    pub anisotropy: f32,
    /// Mean luma, 0–1.
    pub mean: f32,
    /// Fraction of pixels crushed to black (≤ 4) and blown to white (≥ 251).
    pub clip_lo: f32,
    pub clip_hi: f32,
    /// Estimated noise sigma (0–255 units, Immerkær's method).
    pub noise: f32,
    /// Global RMS of the Laplacian.
    pub detail: f32,
}

impl Quality {
    /// Detail at `cell` (0–15) of the 4×4 grid.
    pub fn at(&self, cell: usize) -> f32 {
        self.grid[cell.min(15)]
    }
}

/// A single-channel plane, 0–255.
#[derive(Clone, Debug, PartialEq)]
pub struct Luma {
    pub w: u32,
    pub h: u32,
    pub px: Vec<f32>,
}

impl Luma {
    /// Rec. 601 luma of RGBA8 pixels, composited over mid grey where transparent.
    pub fn from_rgba(w: u32, h: u32, rgba: &[u8]) -> Luma {
        let n = (w as usize) * (h as usize);
        let mut px = Vec::with_capacity(n);
        for p in rgba.chunks_exact(4).take(n) {
            let y = 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
            px.push(over_grey(y, p[3]));
        }
        px.resize(n, 128.0);
        Luma { w, h, px }
    }

    /// Area-averaged copy at `w`×`h`.
    pub fn resized(&self, w: u32, h: u32) -> Luma {
        if w == self.w && h == self.h {
            return self.clone();
        }
        Luma {
            w,
            h,
            px: resize_plane(
                &self.px,
                self.w as usize,
                self.h as usize,
                w as usize,
                h as usize,
            ),
        }
    }
}

fn over_grey(v: f32, a: u8) -> f32 {
    if a == 255 {
        v
    } else {
        let a = f32::from(a) / 255.0;
        v * a + 128.0 * (1.0 - a)
    }
}

/// Box-filter weights mapping `src` samples onto `dst`: for each output, (first input, weights).
fn box_weights(src: usize, dst: usize) -> Vec<(usize, Vec<f32>)> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|i| {
            let x0 = i as f64 * scale;
            let x1 = ((i + 1) as f64 * scale).min(src as f64);
            if scale <= 1.0 {
                // Upsampling: the nearest sample.
                return ((x0 as usize).min(src - 1), vec![1.0]);
            }
            let first = x0.floor() as usize;
            let last = (x1.ceil() as usize).min(src);
            let w: Vec<f32> = (first..last)
                .map(|j| ((x1.min(j as f64 + 1.0) - x0.max(j as f64)) / scale) as f32)
                .collect();
            (first, w)
        })
        .collect()
}

/// Area-average resize of one plane.
pub fn resize_plane(src: &[f32], w: usize, h: usize, dw: usize, dh: usize) -> Vec<f32> {
    if w == 0 || h == 0 || dw == 0 || dh == 0 {
        return vec![128.0; dw * dh];
    }
    let wx = box_weights(w, dw);
    let wy = box_weights(h, dh);
    // Horizontal pass: h × dw.
    let mut tmp = vec![0.0f32; h * dw];
    for y in 0..h {
        let row = &src[y * w..y * w + w];
        let out = &mut tmp[y * dw..y * dw + dw];
        for (o, (first, ws)) in out.iter_mut().zip(&wx) {
            *o = ws.iter().zip(&row[*first..]).map(|(a, b)| a * b).sum();
        }
    }
    // Vertical pass.
    let mut out = vec![0.0f32; dw * dh];
    for (y, (first, ws)) in wy.iter().enumerate() {
        let o = &mut out[y * dw..y * dw + dw];
        for (k, wgt) in ws.iter().enumerate() {
            let r = &tmp[(first + k) * dw..(first + k) * dw + dw];
            for (a, b) in o.iter_mut().zip(r) {
                *a += wgt * b;
            }
        }
    }
    out
}

/// Three planes (R, G, B; transparent pixels over mid grey) of RGBA8 pixels.
fn rgb_planes(w: u32, h: u32, rgba: &[u8]) -> [Vec<f32>; 3] {
    let n = (w as usize) * (h as usize);
    let mut p = [
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    ];
    for px in rgba.chunks_exact(4).take(n) {
        for c in 0..3 {
            p[c].push(over_grey(f32::from(px[c]), px[3]));
        }
    }
    for c in &mut p {
        c.resize(n, 128.0);
    }
    p
}

/// FNV-1a over bytes.
fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(h, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

impl Signature {
    /// The signature of an oriented RGBA8 thumbnail (`w`×`h`, at most [`SIG_CLASS`] a side
    /// in practice; any size works).
    pub fn of_rgba(w: u32, h: u32, rgba: &[u8]) -> Signature {
        let (w, h) = (w.max(1), h.max(1));
        let luma = Luma::from_rgba(w, h, rgba);
        let rgb = rgb_planes(w, h, rgba);
        let (wu, hu) = (w as usize, h as usize);

        // Tier 1: 16×16 RGB, 6 bits a channel, with the aspect ratio.
        let mut t1 = 0xcbf2_9ce4_8422_2325u64;
        let aspect = w as f32 / h as f32;
        t1 = fnv(t1, &((aspect * 100.0).round() as u32).to_le_bytes());
        let small: Vec<Vec<f32>> = rgb
            .iter()
            .map(|p| resize_plane(p, wu, hu, 16, 16))
            .collect();
        for i in 0..256 {
            for c in &small {
                t1 = fnv(t1, &[((c[i] + 2.0) / 4.0).min(63.0) as u8]);
            }
        }

        // pHash: the 8×8 lowest frequencies of the 32×32 DCT, against their median.
        let l32 = resize_plane(&luma.px, wu, hu, 32, 32);
        let phash = phash_of(&l32);

        // dHash: 16 rows of 16 left-right comparisons. A step must clear a small dead zone:
        // flat areas (sky, walls) otherwise flip on every re-encode.
        let l17 = resize_plane(&luma.px, wu, hu, 17, 16);
        let mut dhash = [0u64; 4];
        for y in 0..16 {
            for x in 0..16 {
                if l17[y * 17 + x + 1] > l17[y * 17 + x] + DHASH_DEAD_ZONE {
                    let bit = y * 16 + x;
                    dhash[bit / 64] |= 1 << (bit % 64);
                }
            }
        }

        // Colour moments in YCbCr at 32×32.
        let c32: Vec<Vec<f32>> = rgb
            .iter()
            .map(|p| resize_plane(p, wu, hu, 32, 32))
            .collect();
        let mut sum = [0.0f64; 3];
        let mut sq = [0.0f64; 3];
        for ((&r, &g), &b) in c32[0].iter().zip(&c32[1]).zip(&c32[2]) {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            let cb = 128.0 - 0.168_736 * r - 0.331_264 * g + 0.5 * b;
            let cr = 128.0 + 0.5 * r - 0.418_688 * g - 0.081_312 * b;
            for (k, v) in [y, cb, cr].into_iter().enumerate() {
                sum[k] += f64::from(v);
                sq[k] += f64::from(v) * f64::from(v);
            }
        }
        let mut moments = [0u8; 6];
        for k in 0..3 {
            let m = sum[k] / 1024.0;
            let sd = (sq[k] / 1024.0 - m * m).max(0.0).sqrt();
            moments[k] = m.round().clamp(0.0, 255.0) as u8;
            moments[3 + k] = (sd * 2.0).round().clamp(0.0, 255.0) as u8;
        }

        Signature {
            tier1: t1,
            phash,
            dhash,
            moments,
            aspect,
            quality: quality_of(&luma),
        }
    }

    pub fn phash_distance(&self, o: &Signature) -> u32 {
        (self.phash ^ o.phash).count_ones()
    }

    pub fn dhash_distance(&self, o: &Signature) -> u32 {
        self.dhash
            .iter()
            .zip(&o.dhash)
            .map(|(a, b)| (a ^ b).count_ones())
            .sum()
    }

    /// Colour-moment distance, 0 (same) to 1 (very different).
    pub fn colour_distance(&self, o: &Signature) -> f32 {
        let d: u32 = self
            .moments
            .iter()
            .zip(&o.moments)
            .map(|(&a, &b)| u32::from(a.abs_diff(b)))
            .sum();
        (d as f32 / 240.0).min(1.0)
    }

    /// Relative difference of the aspect ratios (0 = same shape).
    pub fn aspect_difference(&self, o: &Signature) -> f32 {
        let (a, b) = (self.aspect.max(1e-3), o.aspect.max(1e-3));
        (a / b).max(b / a) - 1.0
    }

    /// Combined tier 2 distance, about 0 for the same image and about 0.9 for unrelated ones.
    pub fn visual_distance(&self, o: &Signature) -> f32 {
        let p = self.phash_distance(o) as f32 / 32.0;
        let d = self.dhash_distance(o) as f32 / 128.0;
        let c = self.colour_distance(o);
        let a = (self.aspect_difference(o) * 2.0).min(1.0);
        (0.42 * p + 0.33 * d + 0.15 * c + 0.10 * a).min(1.5)
    }

    pub fn to_bytes(&self) -> [u8; SIG_BYTES] {
        let mut o = [0u8; SIG_BYTES];
        let mut i = 0;
        let mut put = |b: &[u8]| {
            o[i..i + b.len()].copy_from_slice(b);
            i += b.len();
        };
        put(&self.tier1.to_le_bytes());
        put(&self.phash.to_le_bytes());
        for d in self.dhash {
            put(&d.to_le_bytes());
        }
        put(&self.moments);
        put(&self.aspect.to_le_bytes());
        let q = &self.quality;
        for v in q.grid.iter().chain(&[
            q.anisotropy,
            q.mean,
            q.clip_lo,
            q.clip_hi,
            q.noise,
            q.detail,
        ]) {
            put(&v.to_le_bytes());
        }
        o
    }

    pub fn from_bytes(b: &[u8]) -> Option<Signature> {
        if b.len() < SIG_BYTES {
            return None;
        }
        let u64_at = |i: usize| u64::from_le_bytes(b[i..i + 8].try_into().unwrap_or([0; 8]));
        let f32_at = |i: usize| f32::from_le_bytes(b[i..i + 4].try_into().unwrap_or([0; 4]));
        let mut moments = [0u8; 6];
        moments.copy_from_slice(&b[48..54]);
        let mut f = [0.0f32; QUALITY_FLOATS];
        for (k, v) in f.iter_mut().enumerate() {
            *v = f32_at(58 + 4 * k);
        }
        let mut grid = [0.0f32; 16];
        grid.copy_from_slice(&f[..16]);
        Some(Signature {
            tier1: u64_at(0),
            phash: u64_at(8),
            dhash: [u64_at(16), u64_at(24), u64_at(32), u64_at(40)],
            moments,
            aspect: f32_at(54),
            quality: Quality {
                grid,
                anisotropy: f[16],
                mean: f[17],
                clip_lo: f[18],
                clip_hi: f[19],
                noise: f[20],
                detail: f[21],
            },
        })
    }
}

/// DCT-II basis, 8 frequencies × 32 samples.
fn dct_basis() -> [[f32; 32]; 8] {
    let mut c = [[0.0f32; 32]; 8];
    for (u, row) in c.iter_mut().enumerate() {
        for (x, v) in row.iter_mut().enumerate() {
            *v = (std::f32::consts::PI * (2 * x + 1) as f32 * u as f32 / 64.0).cos();
        }
    }
    c
}

fn phash_of(l32: &[f32]) -> u64 {
    let c = dct_basis();
    // Rows: 32 × 8.
    let mut rows = [[0.0f32; 8]; 32];
    for y in 0..32 {
        let r = &l32[y * 32..y * 32 + 32];
        for u in 0..8 {
            rows[y][u] = r.iter().zip(&c[u]).map(|(a, b)| a * b).sum();
        }
    }
    let mut coef = [0.0f32; 64];
    for v in 0..8 {
        for u in 0..8 {
            coef[v * 8 + u] = (0..32).map(|y| rows[y][u] * c[v][y]).sum();
        }
    }
    let mut ac: Vec<f32> = coef[1..].to_vec();
    ac.sort_by(f32::total_cmp);
    let median = ac[ac.len() / 2];
    let mut h = 0u64;
    for (i, &v) in coef.iter().enumerate().skip(1) {
        if v > median {
            h |= 1 << i;
        }
    }
    h
}

fn quality_of(l: &Luma) -> Quality {
    let (w, h) = (l.w as usize, l.h as usize);
    let px = &l.px;
    let mut q = Quality::default();
    let n = (w * h).max(1) as f32;
    let (mut lo, mut hi, mut sum) = (0usize, 0usize, 0.0f64);
    for &v in px {
        sum += f64::from(v);
        if v <= 4.0 {
            lo += 1;
        } else if v >= 251.0 {
            hi += 1;
        }
    }
    q.mean = (sum / f64::from(n) / 255.0) as f32;
    q.clip_lo = lo as f32 / n;
    q.clip_hi = hi as f32 / n;
    if w < 3 || h < 3 {
        return q;
    }
    let mut cell = [0.0f64; 16];
    let mut count = [0u32; 16];
    let (mut gxx, mut gyy, mut gxy) = (0.0f64, 0.0f64, 0.0f64);
    let mut noise = 0.0f64;
    let mut all = 0.0f64;
    for y in 1..h - 1 {
        let cy = (y * 4 / h).min(3);
        for x in 1..w - 1 {
            let i = y * w + x;
            let c = px[i];
            let (nn, s, e, ww) = (px[i - w], px[i + w], px[i + 1], px[i - 1]);
            let lap = 4.0 * c - nn - s - e - ww;
            let l2 = f64::from(lap * lap);
            let k = cy * 4 + (x * 4 / w).min(3);
            cell[k] += l2;
            count[k] += 1;
            all += l2;
            let gx = f64::from(e - ww) * 0.5;
            let gy = f64::from(s - nn) * 0.5;
            gxx += gx * gx;
            gyy += gy * gy;
            gxy += gx * gy;
            // Immerkær: [1 -2 1; -2 4 -2; 1 -2 1].
            let m = px[i - w - 1] + px[i - w + 1] + px[i + w - 1] + px[i + w + 1]
                - 2.0 * (nn + s + e + ww)
                + 4.0 * c;
            noise += f64::from(m.abs());
        }
    }
    for k in 0..16 {
        q.grid[k] = (cell[k] / f64::from(count[k].max(1))).sqrt() as f32;
    }
    let inner = ((w - 2) * (h - 2)) as f64;
    q.detail = (all / inner).sqrt() as f32;
    let tr = gxx + gyy;
    q.anisotropy = if tr > 1e-9 {
        (((gxx - gyy).powi(2) + 4.0 * gxy * gxy).sqrt() / tr) as f32
    } else {
        0.0
    };
    q.noise = (noise * (std::f64::consts::FRAC_PI_2).sqrt() / (6.0 * inner)) as f32;
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let c = f(x, y);
                v.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        v
    }

    #[test]
    fn roundtrip_bytes() {
        let px = gradient(64, 48, |x, y| {
            [(x * 4) as u8, (y * 5) as u8, ((x ^ y) * 3) as u8]
        });
        let s = Signature::of_rgba(64, 48, &px);
        let b = s.to_bytes();
        assert_eq!(Signature::from_bytes(&b), Some(s));
    }

    #[test]
    fn same_pixels_same_tier1_resize_close() {
        // A scene with structure at several scales (as photos have): blocks of hashed
        // brightness over a gradient.
        let f = |x: u32, y: u32| {
            let b = ((x / 24) * 7919 + (y / 20) * 104_729).wrapping_mul(2_654_435_761) >> 24;
            let v = ((b & 0x7f) + x / 4 + y / 6).min(255) as u8;
            [v, v / 2 + 40, 255 - v]
        };
        let big = gradient(256, 192, f);
        let a = Signature::of_rgba(256, 192, &big);
        let b = Signature::of_rgba(256, 192, &gradient(256, 192, f));
        assert_eq!(a.tier1, b.tier1);
        // Half size, 2×2 box (a resized copy).
        let mut half = Vec::new();
        for y in 0..96usize {
            for x in 0..128usize {
                for c in 0..4 {
                    let at = |xx: usize, yy: usize| u32::from(big[(yy * 256 + xx) * 4 + c]);
                    let s = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    half.push(((s + 2) / 4) as u8);
                }
            }
        }
        let c = Signature::of_rgba(128, 96, &half);
        assert!(a.phash_distance(&c) <= 6, "{}", a.phash_distance(&c));
        assert!(a.visual_distance(&c) < 0.15, "{}", a.visual_distance(&c));
        // An unrelated scene is far.
        let d = Signature::of_rgba(256, 192, &gradient(256, 192, |x, y| f(y + 37, x + 11)));
        assert!(a.visual_distance(&d) > 0.3, "{}", a.visual_distance(&d));
    }

    #[test]
    fn resize_preserves_mean() {
        let src: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let out = resize_plane(&src, 10, 10, 3, 3);
        let m1: f32 = src.iter().sum::<f32>() / 100.0;
        let m2: f32 = out.iter().sum::<f32>() / 9.0;
        assert!((m1 - m2).abs() < 0.01, "{m1} {m2}");
    }
}
