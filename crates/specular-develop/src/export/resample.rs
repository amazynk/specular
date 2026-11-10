//! Linear-light resizing for export, streamed: rows arrive band by band from the GPU and
//! leave as soon as every input row under their filter has arrived. Separable Lanczos-3
//! (`specular_convert::pixels::taps`, the kernel stretched when shrinking), RGB only.

use std::collections::VecDeque;

use rayon::prelude::*;
use specular_convert::pixels::{Taps, taps};

/// A `iw × ih` → `ow × oh` resize fed with RGBA rows in order.
pub struct Resampler {
    iw: usize,
    ow: usize,
    hx: Vec<Taps>,
    vy: Vec<Taps>,
    /// Horizontally resized rows still needed: (input row, `ow × 3` values).
    rows: VecDeque<(usize, Vec<f32>)>,
    next_in: usize,
    next_out: usize,
}

impl Resampler {
    pub fn new(iw: u32, ih: u32, ow: u32, oh: u32) -> Resampler {
        let (iw, ih, ow, oh) = (iw as usize, ih as usize, ow as usize, oh as usize);
        Resampler {
            iw,
            ow,
            hx: taps(iw, ow),
            vy: taps(ih, oh),
            rows: VecDeque::new(),
            next_in: 0,
            next_out: 0,
        }
    }

    /// The horizontal taps (one per output column), for the GPU's half
    /// ([`super::hresize`]).
    pub fn columns(&self) -> &[Taps] {
        &self.hx
    }

    /// Takes the next input rows (`iw × 4` values each) and returns the output rows they
    /// complete: the first one's index and `ow × 3` values per row.
    pub fn push(&mut self, rgba: &[f32]) -> (usize, Vec<f32>) {
        let (iw, ow) = (self.iw, self.ow);
        let n = rgba.len() / (iw * 4);
        let hx = &self.hx;
        let resized: Vec<Vec<f32>> = rgba
            .par_chunks_exact(iw * 4)
            .map(|row| {
                let mut out = vec![0f32; ow * 3];
                for (o, t) in out.chunks_exact_mut(3).zip(hx) {
                    let mut a = [0f32; 3];
                    for (j, &w) in t.w.iter().enumerate() {
                        let p = (t.start + j) * 4;
                        a[0] += w * row[p];
                        a[1] += w * row[p + 1];
                        a[2] += w * row[p + 2];
                    }
                    o.copy_from_slice(&a);
                }
                out
            })
            .collect();
        debug_assert!(n > 0 || rgba.is_empty());
        self.vertical(resized)
    }

    /// [`Resampler::push`] with the next input rows already resized horizontally (`ow × 3`
    /// values each, the GPU's half).
    pub fn push_resized(&mut self, rgb: &[f32]) -> (usize, Vec<f32>) {
        let rows = rgb.chunks_exact(self.ow * 3).map(<[f32]>::to_vec).collect();
        self.vertical(rows)
    }

    fn vertical(&mut self, resized: Vec<Vec<f32>>) -> (usize, Vec<f32>) {
        let ow = self.ow;
        for r in resized {
            self.rows.push_back((self.next_in, r));
            self.next_in += 1;
        }
        // Output rows whose taps have all arrived.
        let first = self.next_out;
        let mut last = first;
        while last < self.vy.len() {
            let t = &self.vy[last];
            if t.start + t.w.len() > self.next_in {
                break;
            }
            last += 1;
        }
        let base = self.rows.front().map_or(0, |r| r.0);
        let rows = &self.rows;
        let vy = &self.vy[first..last];
        let mut out = vec![0f32; (last - first) * ow * 3];
        out.par_chunks_exact_mut(ow * 3).zip(vy).for_each(|(o, t)| {
            for (j, &w) in t.w.iter().enumerate() {
                let (y, r) = &rows[t.start + j - base];
                debug_assert_eq!(*y, t.start + j);
                for (a, v) in o.iter_mut().zip(r) {
                    *a += w * v;
                }
            }
        });
        self.next_out = last;
        // Rows no later output row reads.
        let keep_from = self.vy.get(last).map_or(usize::MAX, |t| t.start);
        while self.rows.front().is_some_and(|r| r.0 < keep_from) {
            self.rows.pop_front();
        }
        (first, out)
    }

    /// Every output row has been made.
    pub fn done(&self) -> bool {
        self.next_out == self.vy.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fed in uneven bands, the stream gives the same rows as one push of everything, and a
    /// flat image stays flat.
    #[test]
    fn bands_give_the_whole_image_result() {
        let (iw, ih, ow, oh) = (61u32, 47u32, 23u32, 17u32);
        let mut px = Vec::new();
        for y in 0..ih {
            for x in 0..iw {
                px.extend([x as f32 / 61.0, y as f32 / 47.0, ((x * y) % 7) as f32, 1.0]);
            }
        }
        let mut whole = Resampler::new(iw, ih, ow, oh);
        let (f, all) = whole.push(&px);
        assert_eq!(f, 0);
        assert!(whole.done());
        assert_eq!(all.len(), (ow * oh * 3) as usize);
        let mut s = Resampler::new(iw, ih, ow, oh);
        let mut got = Vec::new();
        let mut y = 0usize;
        for band in [5usize, 1, 13, 9, 19] {
            let rows = &px[y * iw as usize * 4..(y + band) * iw as usize * 4];
            let (first, out) = s.push(rows);
            assert_eq!(first, got.len() / (ow as usize * 3));
            got.extend(out);
            y += band;
        }
        assert_eq!(y, ih as usize);
        assert!(s.done());
        assert_eq!(got, all);

        let flat = vec![0.25f32; (iw * ih * 4) as usize];
        let (_, out) = Resampler::new(iw, ih, ow, oh).push(&flat);
        assert!(out.iter().all(|v| (v - 0.25).abs() < 1e-5));
    }
}
