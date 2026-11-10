//! Frame buffers (padded to whole macroblocks) and half-pel motion compensation (§7.6.4).

use crate::Chroma;

/// A decoded frame: Y, Cb, Cr at the coded (macroblock-aligned) size.
#[derive(Clone)]
pub(crate) struct Frame {
    pub planes: [Vec<u8>; 3],
    pub stride: [usize; 3],
    pub lines: [usize; 3],
}

impl Frame {
    pub fn new(mb_w: usize, mb_h: usize, chroma: Chroma) -> Frame {
        let (sx, sy) = chroma.shift();
        let w = mb_w * 16;
        let h = mb_h * 16;
        let (cw, ch) = (w >> sx, h >> sy);
        Frame {
            planes: [vec![128; w * h], vec![128; cw * ch], vec![128; cw * ch]],
            stride: [w, cw, cw],
            lines: [h, ch, ch],
        }
    }

    /// Plane `c` as a frame (`parity` None) or as one of its fields.
    pub fn view(&self, c: usize, parity: Option<u8>) -> View<'_> {
        let s = self.stride[c];
        match parity {
            None => View {
                data: &self.planes[c],
                off: 0,
                step: s,
                w: s as i32,
                h: self.lines[c] as i32,
            },
            Some(p) => View {
                data: &self.planes[c],
                off: usize::from(p) * s,
                step: 2 * s,
                w: s as i32,
                h: (self.lines[c] / 2) as i32,
            },
        }
    }
}

/// A plane, or a field of one, to predict from.
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    data: &'a [u8],
    off: usize,
    /// Bytes from one line of the view to the next.
    step: usize,
    w: i32,
    h: i32,
}

/// Forms a `w`×`h` prediction at integer position (`x`, `y`) of `src` with half-pel offsets
/// `hx`, `hy` into `dst` (lines `dstride` apart); with `avg`, averages it with what `dst`
/// holds (the second of two predictions). Positions outside the plane repeat its edge.
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) fn mc(
    dst: &mut [u8],
    dstride: usize,
    src: &View,
    x: i32,
    y: i32,
    hx: bool,
    hy: bool,
    w: usize,
    h: usize,
    avg: bool,
) {
    let nw = w + usize::from(hx);
    let nh = h + usize::from(hy);
    let mut patch = [0u8; 17 * 17];
    let (data, base, step): (&[u8], usize, usize) = if x >= 0
        && y >= 0
        && x as usize + nw <= src.w as usize
        && y as usize + nh <= src.h as usize
    {
        (
            src.data,
            src.off + y as usize * src.step + x as usize,
            src.step,
        )
    } else {
        for i in 0..nh {
            let sy = (y + i as i32).clamp(0, src.h - 1) as usize;
            let row = src.off + sy * src.step;
            for j in 0..nw {
                let sx = (x + j as i32).clamp(0, src.w - 1) as usize;
                patch[i * 17 + j] = src.data[row + sx];
            }
        }
        (&patch[..], 0, 17)
    };
    for i in 0..h {
        let s0 = &data[base + i * step..];
        let d = &mut dst[i * dstride..i * dstride + w];
        match (hx, hy) {
            (false, false) => {
                if avg {
                    for (o, &a) in d.iter_mut().zip(&s0[..w]) {
                        *o = ((u16::from(*o) + u16::from(a) + 1) >> 1) as u8;
                    }
                } else {
                    d.copy_from_slice(&s0[..w]);
                }
            }
            (true, false) => {
                for (j, o) in d.iter_mut().enumerate() {
                    let p = (u16::from(s0[j]) + u16::from(s0[j + 1]) + 1) >> 1;
                    *o = if avg {
                        ((u16::from(*o) + p + 1) >> 1) as u8
                    } else {
                        p as u8
                    };
                }
            }
            (false, true) => {
                let s1 = &data[base + (i + 1) * step..];
                for (j, o) in d.iter_mut().enumerate() {
                    let p = (u16::from(s0[j]) + u16::from(s1[j]) + 1) >> 1;
                    *o = if avg {
                        ((u16::from(*o) + p + 1) >> 1) as u8
                    } else {
                        p as u8
                    };
                }
            }
            (true, true) => {
                let s1 = &data[base + (i + 1) * step..];
                for (j, o) in d.iter_mut().enumerate() {
                    let p = (u16::from(s0[j])
                        + u16::from(s0[j + 1])
                        + u16::from(s1[j])
                        + u16::from(s1[j + 1])
                        + 2)
                        >> 2;
                    *o = if avg {
                        ((u16::from(*o) + p + 1) >> 1) as u8
                    } else {
                        p as u8
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_pel_and_edges() {
        let mut f = Frame::new(1, 1, Chroma::Yuv420);
        for (i, v) in f.planes[0].iter_mut().enumerate() {
            *v = (i % 16 * 10 + i / 16) as u8;
        }
        let v = f.view(0, None);
        let mut d = [0u8; 4];
        mc(&mut d, 2, &v, 0, 0, true, false, 2, 2, false);
        assert_eq!(d, [5, 15, 6, 16]);
        mc(&mut d, 2, &v, -3, -3, false, false, 2, 2, false);
        assert_eq!(d, [0, 0, 0, 0]);
        // A field: every other line.
        let top = f.view(0, Some(1));
        mc(&mut d, 2, &top, 0, 0, false, false, 2, 2, false);
        assert_eq!(d, [1, 11, 3, 13]);
        mc(&mut d, 2, &top, 0, 0, false, true, 2, 2, true);
        assert_eq!(d, [2, 12, 4, 14]);
    }
}
