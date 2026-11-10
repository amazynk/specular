//! Frames and motion compensation (ISO/IEC 14496-2 §7.6.2): half-sample bilinear prediction
//! with rounding control, quarter-sample prediction through the 8-tap filter mirrored at the
//! block's edges, and positions outside the VOP taken from its nearest edge sample
//! (unrestricted motion vectors).

/// A decoded VOP: Y, Cb, Cr padded to whole macroblocks; `width`/`height` the VOP's own size
/// (references outside it repeat its edge).
#[derive(Clone)]
pub(crate) struct Frame {
    pub planes: [Vec<u8>; 3],
    pub stride: [usize; 3],
    pub width: [i32; 3],
    pub height: [i32; 3],
}

impl Frame {
    pub fn new(mb_w: usize, mb_h: usize, w: u32, h: u32) -> Frame {
        let (pw, ph) = (mb_w * 16, mb_h * 16);
        Frame {
            planes: [
                vec![128; pw * ph],
                vec![128; pw * ph / 4],
                vec![128; pw * ph / 4],
            ],
            stride: [pw, pw / 2, pw / 2],
            width: [w as i32, w.div_ceil(2) as i32, w.div_ceil(2) as i32],
            height: [h as i32, h.div_ceil(2) as i32, h.div_ceil(2) as i32],
        }
    }
}

/// A plane (or one of its fields) to predict from. A field's lines outside the picture are
/// those of the padded frame: its edge line, whichever field that is.
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    pub data: &'a [u8],
    pub off: usize,
    pub step: usize,
    pub w: i32,
    pub h: i32,
    /// Field views: the parity, the frame's stride and lines.
    field: Option<(i32, usize, i32)>,
}

impl Frame {
    pub fn view(&self, c: usize, parity: Option<u8>) -> View<'_> {
        let s = self.stride[c];
        match parity {
            None => View {
                data: &self.planes[c],
                off: 0,
                step: s,
                w: self.width[c],
                h: self.height[c],
                field: None,
            },
            Some(p) => View {
                data: &self.planes[c],
                off: usize::from(p) * s,
                step: 2 * s,
                w: self.width[c],
                h: (self.height[c] + 1 - i32::from(p)) / 2,
                field: Some((i32::from(p), s, self.height[c])),
            },
        }
    }
}

impl View<'_> {
    #[inline(always)]
    pub fn at(&self, x: i32, y: i32) -> u8 {
        let x = x.clamp(0, self.w - 1) as usize;
        if let Some((p, stride, lines)) = self.field {
            let fy = (2 * y + p).clamp(0, lines - 1) as usize;
            return self.data[fy * stride + x];
        }
        let y = y.clamp(0, self.h - 1) as usize;
        self.data[self.off + y * self.step + x]
    }

    /// The `w`×`h` integer samples from (`x`, `y`) into `out` (stride `w`).
    fn fetch(&self, x: i32, y: i32, w: usize, h: usize, out: &mut [u8]) {
        if x >= 0
            && y >= 0
            && x as usize + w <= self.w as usize
            && y as usize + h <= self.h as usize
        {
            for r in 0..h {
                let o = self.off + (y as usize + r) * self.step + x as usize;
                out[r * w..r * w + w].copy_from_slice(&self.data[o..o + w]);
            }
        } else {
            for r in 0..h {
                for c in 0..w {
                    out[r * w + c] = self.at(x + c as i32, y + r as i32);
                }
            }
        }
    }
}

/// Half-sample prediction of a `w`×`h` block at half-sample vector (`vx`, `vy`) from
/// integer position (`x`, `y`) into `dst` (lines `ds` apart); `rnd` is the rounding control.
#[allow(clippy::too_many_arguments)]
pub(crate) fn half(
    dst: &mut [u8],
    ds: usize,
    src: &View,
    x: i32,
    y: i32,
    vx: i32,
    vy: i32,
    w: usize,
    h: usize,
    rnd: bool,
) {
    let (ix, iy) = (x + (vx >> 1), y + (vy >> 1));
    let (hx, hy) = (vx & 1 != 0, vy & 1 != 0);
    let mut buf = [0u8; 17 * 17];
    let bw = w + 1;
    src.fetch(ix, iy, bw, h + 1, &mut buf);
    let r = i32::from(rnd);
    for i in 0..h {
        let s0 = &buf[i * bw..];
        let s1 = &buf[(i + 1) * bw..];
        let d = &mut dst[i * ds..i * ds + w];
        for (j, o) in d.iter_mut().enumerate() {
            let a = i32::from(s0[j]);
            *o = match (hx, hy) {
                (false, false) => a,
                (true, false) => (a + i32::from(s0[j + 1]) + 1 - r) >> 1,
                (false, true) => (a + i32::from(s1[j]) + 1 - r) >> 1,
                (true, true) => {
                    (a + i32::from(s0[j + 1]) + i32::from(s1[j]) + i32::from(s1[j + 1]) + 2 - r)
                        >> 2
                }
            } as u8;
        }
    }
}

/// The 8-tap half-sample filter over `p[-3..=4]` around the gap after `p[0]`.
#[inline(always)]
fn tap(p: [i32; 8], rnd: i32) -> u8 {
    let v = 20 * (p[3] + p[4]) - 6 * (p[2] + p[5]) + 3 * (p[1] + p[6]) - (p[0] + p[7]);
    ((v + 16 - rnd) >> 5).clamp(0, 255) as u8
}

/// One line of `n + 1` samples (`line[0..=n]`) filtered to the `n` half positions between
/// them, mirrored at both ends.
fn filter_line(line: &[i32], n: usize, rnd: i32, out: &mut [u8]) {
    let get = |k: isize| -> i32 {
        let last = n as isize;
        let k = if k < 0 {
            -1 - k
        } else if k > last {
            2 * last + 1 - k
        } else {
            k
        };
        line[k as usize]
    };
    for (i, o) in out.iter_mut().enumerate().take(n) {
        let i = i as isize;
        *o = tap(
            [
                get(i - 3),
                get(i - 2),
                get(i - 1),
                get(i),
                get(i + 1),
                get(i + 2),
                get(i + 3),
                get(i + 4),
            ],
            rnd,
        );
    }
}

/// Quarter-sample luma prediction (§7.6.2.2) of a `w`×`h` block at quarter-sample vector
/// (`vx`, `vy`) from integer position (`x`, `y`): horizontally to the x position first, then
/// vertically, each half sample through the filter mirrored at the block's window.
#[allow(clippy::too_many_arguments)]
pub(crate) fn quarter(
    dst: &mut [u8],
    ds: usize,
    src: &View,
    x: i32,
    y: i32,
    vx: i32,
    vy: i32,
    w: usize,
    h: usize,
    rnd: bool,
) {
    let (ix, iy) = (x + (vx >> 2), y + (vy >> 2));
    let (fx, fy) = ((vx & 3) as usize, (vy & 3) as usize);
    let r = i32::from(rnd);
    let avg = |a: u8, b: u8| ((i32::from(a) + i32::from(b) + 1 - r) >> 1) as u8;
    // The window: (w + 1) × (h + 1) integer samples.
    let mut win = [0u8; 17 * 17];
    let ww = w + 1;
    src.fetch(ix, iy, ww, h + 1, &mut win);
    // Stage 1: each of the h + 1 rows at the horizontal position.
    let mut rows = [0u8; 17 * 17];
    let mut line = [0i32; 17];
    let mut halfs = [0u8; 16];
    for rr in 0..=h {
        let src_row = &win[rr * ww..rr * ww + ww];
        let out = &mut rows[rr * w..rr * w + w];
        match fx {
            0 => out.copy_from_slice(&src_row[..w]),
            _ => {
                for (l, &s) in line.iter_mut().zip(src_row) {
                    *l = i32::from(s);
                }
                filter_line(&line, w, r, &mut halfs);
                for j in 0..w {
                    out[j] = match fx {
                        1 => avg(src_row[j], halfs[j]),
                        2 => halfs[j],
                        _ => avg(src_row[j + 1], halfs[j]),
                    };
                }
            }
        }
    }
    // Stage 2: vertically.
    let mut col = [0i32; 17];
    let mut vhalf = [0u8; 16];
    for j in 0..w {
        if fy == 0 {
            for i in 0..h {
                dst[i * ds + j] = rows[i * w + j];
            }
            continue;
        }
        for (i, c) in col.iter_mut().enumerate().take(h + 1) {
            *c = i32::from(rows[i * w + j]);
        }
        filter_line(&col, h, r, &mut vhalf);
        for i in 0..h {
            dst[i * ds + j] = match fy {
                1 => avg(rows[i * w + j], vhalf[i]),
                2 => vhalf[i],
                _ => avg(rows[(i + 1) * w + j], vhalf[i]),
            };
        }
    }
}

/// Averages `src` into `dst` (bidirectional prediction): (a + b + 1) / 2.
pub(crate) fn average(dst: &mut [u8], ds: usize, src: &[u8], ss: usize, w: usize, h: usize) {
    for i in 0..h {
        for j in 0..w {
            let o = &mut dst[i * ds + j];
            *o = ((u16::from(*o) + u16::from(src[i * ss + j]) + 1) >> 1) as u8;
        }
    }
}

/// The H.263 rounding of a sum of four luma vectors to a chroma vector (§7.6.2.1,
/// Table 7-9): sixteenths to the nearest half sample.
pub(crate) fn round_chroma_4(sum: i32) -> i32 {
    const TAB: [i32; 16] = [0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2];
    if sum >= 0 {
        TAB[(sum & 15) as usize] + ((sum >> 3) & !1)
    } else {
        let s = -sum;
        -(TAB[(s & 15) as usize] + ((s >> 3) & !1))
    }
}

/// A one-vector chroma vector from a luma half-sample vector: halved, a quarter rounded to
/// the half.
pub(crate) fn chroma_of(v: i32) -> i32 {
    (v >> 1) | (v & 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_areas_stay_flat() {
        let mut f = Frame::new(2, 2, 32, 32);
        f.planes[0].fill(77);
        let v = f.view(0, None);
        let mut d = [0u8; 256];
        for (vx, vy) in [(1, 0), (3, 2), (-5, 7), (2, 2), (1, 3)] {
            quarter(&mut d, 16, &v, 0, 0, vx, vy, 16, 16, false);
            assert!(d.iter().all(|&p| p == 77), "{vx},{vy}");
            half(&mut d, 16, &v, 4, 4, vx, vy, 16, 16, true);
            assert!(d.iter().all(|&p| p == 77), "{vx},{vy}");
        }
        assert_eq!(round_chroma_4(0), 0);
        assert_eq!(round_chroma_4(4), 1);
        assert_eq!(round_chroma_4(-4), -1);
        assert_eq!(round_chroma_4(16), 2);
        assert_eq!(chroma_of(3), 1);
        assert_eq!(chroma_of(-3), -1);
        assert_eq!(chroma_of(4), 2);
    }
}
