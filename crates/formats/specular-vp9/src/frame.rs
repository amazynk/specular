//! Frame buffers: 8-bit (`u8`) or high bit depth (`u16`) planes sized to whole superblocks.

use std::simd::{Simd, num::SimdUint};

/// A sample type: `u8` for 8-bit video, `u16` above.
pub(crate) trait Pixel: Copy + Default + Send + Sync + 'static + std::fmt::Debug {
    const HBD: bool;
    fn get(self) -> i32;
    /// From a value already in range.
    fn put(v: i32) -> Self;
    fn load8(s: &[Self]) -> Simd<i32, 8>;
    fn write_le(s: &[Self], out: &mut Vec<u8>);
}

impl Pixel for u8 {
    const HBD: bool = false;
    #[inline(always)]
    fn get(self) -> i32 {
        self as i32
    }
    #[inline(always)]
    fn put(v: i32) -> Self {
        v as u8
    }
    #[inline(always)]
    fn load8(s: &[Self]) -> Simd<i32, 8> {
        Simd::<u8, 8>::from_slice(&s[..8]).cast()
    }
    fn write_le(s: &[Self], out: &mut Vec<u8>) {
        out.extend_from_slice(s);
    }
}

impl Pixel for u16 {
    const HBD: bool = true;
    #[inline(always)]
    fn get(self) -> i32 {
        self as i32
    }
    #[inline(always)]
    fn put(v: i32) -> Self {
        v as u16
    }
    #[inline(always)]
    fn load8(s: &[Self]) -> Simd<i32, 8> {
        Simd::<u16, 8>::from_slice(&s[..8]).cast()
    }
    fn write_le(s: &[Self], out: &mut Vec<u8>) {
        out.reserve(s.len() * 2);
        for &v in s {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

/// One plane: `stride` x `rows` samples, of which `w` x `h` are the picture.
pub(crate) struct Plane<P> {
    pub data: Vec<P>,
    pub stride: usize,
    pub rows: usize,
    pub w: usize,
    pub h: usize,
}

impl<P: Pixel> Plane<P> {
    fn new(stride: usize, rows: usize, w: usize, h: usize) -> Plane<P> {
        Plane {
            data: vec![P::default(); stride * rows],
            stride,
            rows,
            w,
            h,
        }
    }

    #[inline(always)]
    pub(crate) fn row(&self, y: usize) -> &[P] {
        &self.data[y * self.stride..(y + 1) * self.stride]
    }
}

/// A decoded frame.
pub(crate) struct FrameBuf<P> {
    pub width: u32,
    pub height: u32,
    pub render_w: u32,
    pub render_h: u32,
    pub ss_x: bool,
    pub ss_y: bool,
    pub bit_depth: u8,
    pub color: crate::header::ColorConfig,
    pub planes: [Plane<P>; 3],
    /// The planes' bytes, charged to the memory budget while the frame lives (as the
    /// current frame or one of eight references).
    _budget: specular_core::Reservation,
}

impl<P: Pixel> FrameBuf<P> {
    /// A frame for `width` x `height` with planes padded to whole 64x64 superblocks, or
    /// an error when the memory budget has no room for it.
    pub(crate) fn new(
        width: u32,
        height: u32,
        color: crate::header::ColorConfig,
    ) -> crate::Result<FrameBuf<P>> {
        let aw = (width as usize).div_ceil(64) * 64;
        let ah = (height as usize).div_ceil(64) * 64;
        let (sx, sy) = (color.ss_x as usize, color.ss_y as usize);
        let cw = (width as usize + sx) >> sx;
        let ch = (height as usize + sy) >> sy;
        let samples = (aw * ah + 2 * ((aw >> sx) * (ah >> sy))) as u64;
        let budget = specular_core::budget::reserve_n(samples, std::mem::size_of::<P>() as u64)
            .map_err(|n| crate::Error::Unsupported(format!("this VP9 video {n}")))?;
        Ok(FrameBuf {
            width,
            height,
            render_w: width,
            render_h: height,
            ss_x: color.ss_x,
            ss_y: color.ss_y,
            bit_depth: color.bit_depth,
            color,
            planes: [
                Plane::new(aw, ah, width as usize, height as usize),
                Plane::new(aw >> sx, ah >> sy, cw, ch),
                Plane::new(aw >> sx, ah >> sy, cw, ch),
            ],
            _budget: budget,
        })
    }

    /// The picture as tightly packed planes (u8, or u16 little-endian).
    pub(crate) fn packed(&self) -> [Vec<u8>; 3] {
        let mut out: [Vec<u8>; 3] = Default::default();
        for (p, o) in self.planes.iter().zip(out.iter_mut()) {
            o.reserve(p.w * p.h * if P::HBD { 2 } else { 1 });
            for y in 0..p.h {
                P::write_le(&p.row(y)[..p.w], o);
            }
        }
        out
    }
}

/// The frame in either sample type.
pub(crate) enum AnyFrame {
    B8(std::sync::Arc<FrameBuf<u8>>),
    B16(std::sync::Arc<FrameBuf<u16>>),
}

impl Clone for AnyFrame {
    fn clone(&self) -> Self {
        match self {
            AnyFrame::B8(f) => AnyFrame::B8(f.clone()),
            AnyFrame::B16(f) => AnyFrame::B16(f.clone()),
        }
    }
}

impl AnyFrame {
    pub(crate) fn size(&self) -> (u32, u32) {
        match self {
            AnyFrame::B8(f) => (f.width, f.height),
            AnyFrame::B16(f) => (f.width, f.height),
        }
    }

    pub(crate) fn format(&self) -> (u8, bool, bool) {
        match self {
            AnyFrame::B8(f) => (f.bit_depth, f.ss_x, f.ss_y),
            AnyFrame::B16(f) => (f.bit_depth, f.ss_x, f.ss_y),
        }
    }

    pub(crate) fn picture(&self) -> crate::Picture {
        fn pic<P: Pixel>(f: &FrameBuf<P>) -> crate::Picture {
            crate::Picture {
                width: f.width,
                height: f.height,
                render_width: f.render_w,
                render_height: f.render_h,
                bit_depth: f.bit_depth,
                ss_x: f.ss_x,
                ss_y: f.ss_y,
                planes: f.packed(),
                color_space: f.color.color_space,
                full_range: f.color.full_range,
            }
        }
        match self {
            AnyFrame::B8(f) => pic(f),
            AnyFrame::B16(f) => pic(f),
        }
    }
}

/// Mutable access to columns `x0..` of every row of a plane, one slice per row (a tile
/// column's share of a frame being decoded).
pub(crate) struct PlaneCols<'a, P> {
    pub rows: Vec<&'a mut [P]>,
    pub x0: usize,
}

/// Splits each row of `data` (rows of `stride`) at the given column boundaries; returns one
/// row set per column range.
pub(crate) fn split_cols<'a, T>(
    data: &'a mut [T],
    stride: usize,
    bounds: &[usize],
) -> Vec<Vec<&'a mut [T]>> {
    let n = bounds.len() - 1;
    let rows = data.len() / stride.max(1);
    let mut out: Vec<Vec<&'a mut [T]>> = (0..n).map(|_| Vec::with_capacity(rows)).collect();
    for row in data.chunks_mut(stride) {
        let mut rest = row;
        for (i, o) in out.iter_mut().enumerate() {
            let w = bounds[i + 1] - bounds[i];
            let (a, b) = rest.split_at_mut(w.min(rest.len()));
            o.push(a);
            rest = b;
        }
    }
    out
}
