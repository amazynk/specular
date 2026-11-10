//! Reconstructed frames: three planes at the macroblock-aligned coded size with a border of
//! repeated edge pixels, so most motion vectors read without bounds handling.

/// One plane; pixel (x, y) is at `data[(y + border) * stride + x + border]`.
pub(crate) struct Plane {
    pub data: Vec<u8>,
    pub stride: usize,
    pub border: usize,
    pub width: usize,
    pub height: usize,
}

impl Plane {
    fn new(width: usize, height: usize, border: usize) -> Plane {
        let stride = (width + 2 * border).next_multiple_of(32);
        Plane {
            data: vec![0; stride * (height + 2 * border)],
            stride,
            border,
            width,
            height,
        }
    }

    /// Index of pixel (0, y).
    pub(crate) fn row(&self, y: usize) -> usize {
        (y + self.border) * self.stride + self.border
    }

    /// Repeats the edge pixels into the border.
    pub(crate) fn extend(&mut self) {
        let (b, s, w, h) = (self.border, self.stride, self.width, self.height);
        for y in b..b + h {
            let row = &mut self.data[y * s..y * s + s];
            let (l, r) = (row[b], row[b + w - 1]);
            row[..b].fill(l);
            row[b + w..].fill(r);
        }
        let (top, rest) = self.data.split_at_mut(b * s);
        let first = &rest[..s];
        for line in top.chunks_exact_mut(s) {
            line.copy_from_slice(first);
        }
        let (body, bottom) = self.data.split_at_mut((b + h) * s);
        let last = &body[(b + h - 1) * s..];
        for line in bottom.chunks_exact_mut(s) {
            line.copy_from_slice(last);
        }
    }

    /// Copies the top-left `w`x`h` pixels into a tightly packed buffer.
    pub(crate) fn crop(&self, w: usize, h: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(w * h);
        for y in 0..h {
            let o = self.row(y);
            out.extend_from_slice(&self.data[o..o + w]);
        }
        out
    }
}

pub(crate) const LUMA_BORDER: usize = 48;
pub(crate) const CHROMA_BORDER: usize = 32;

pub(crate) struct Frame {
    pub planes: [Plane; 3],
}

impl Frame {
    /// A frame for `cols` x `rows` macroblocks.
    pub(crate) fn new(cols: usize, rows: usize) -> Frame {
        Frame {
            planes: [
                Plane::new(cols * 16, rows * 16, LUMA_BORDER),
                Plane::new(cols * 8, rows * 8, CHROMA_BORDER),
                Plane::new(cols * 8, rows * 8, CHROMA_BORDER),
            ],
        }
    }

    /// Bytes a frame of this size takes.
    pub(crate) fn bytes(cols: usize, rows: usize) -> u64 {
        let y = (cols * 16 + 2 * LUMA_BORDER).next_multiple_of(32) * (rows * 16 + 2 * LUMA_BORDER);
        let c =
            (cols * 8 + 2 * CHROMA_BORDER).next_multiple_of(32) * (rows * 8 + 2 * CHROMA_BORDER);
        (y + 2 * c) as u64
    }
}

/// Copies `n` bytes; the common small sizes inline instead of calling `memcpy`.
#[inline(always)]
pub(crate) fn copy_n(dst: &mut [u8], src: &[u8], n: usize) {
    #[inline(always)]
    fn fixed<const N: usize>(dst: &mut [u8], src: &[u8]) {
        if let (Some(d), Some(s)) = (dst.first_chunk_mut::<N>(), src.first_chunk::<N>()) {
            *d = *s;
        }
    }
    match n {
        16 => fixed::<16>(dst, src),
        8 => fixed::<8>(dst, src),
        4 => fixed::<4>(dst, src),
        _ => dst[..n].copy_from_slice(&src[..n]),
    }
}
