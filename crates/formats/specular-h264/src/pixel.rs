//! Sample types: 8-bit pictures store `u8`, deeper ones (High 10, High 4:2:2) `u16`. The
//! decoding code is generic over [`Pix`], so the 8-bit path keeps its byte-sized buffers
//! and its clipping to 255 regardless of the bit depth passed along.

/// The samples of one plane (or plane section) of a picture.
pub enum Samples {
    B(Box<[u8]>),
    W(Box<[u16]>),
}

pub trait Pix: Copy + Default + PartialEq + Send + Sync + std::fmt::Debug + 'static {
    fn i(self) -> i32;
    /// `v` clipped to 0..=max (`max` is 255 for `u8`, which ignores it).
    fn clip(v: i32, max: i32) -> Self;
    /// `v`, known to be in range.
    fn of(v: i32) -> Self;
    /// The samples of `s`, when they are of this type.
    fn slice(s: &Samples) -> Option<&[Self]>;
    fn wrap(v: Box<[Self]>) -> Samples;
    /// A line of zeros (stand-in when a reference has another sample type).
    fn zeros() -> &'static [Self];
    /// Eight samples widened (or not) to 16 bits.
    fn wide8(l: &[Self; 8]) -> Simd<u16, 8>;
    /// Eight samples from 16 bits, known to be in range.
    fn narrow8(v: Simd<u16, 8>) -> [Self; 8];
    /// Appends samples to an output plane as bytes (`u16` little-endian).
    fn put_le(out: &mut Vec<u8>, s: &[Self]);
    /// Buffers of a picture's work area, for the pool.
    fn bufs_in(b: Bufs<Self>) -> AnyBufs;
    /// The buffers, when they hold samples of this type.
    fn bufs_out(b: AnyBufs) -> Option<Bufs<Self>>;
}

use std::simd::{Simd, num::SimdUint};

use crate::slicedec::{AnyBufs, Bufs};

static ZEROS8: [u8; 16 * 1024] = [0; 16 * 1024];
static ZEROS16: [u16; 16 * 1024] = [0; 16 * 1024];

impl Pix for u8 {
    #[inline(always)]
    fn i(self) -> i32 {
        self as i32
    }
    #[inline(always)]
    fn clip(v: i32, _: i32) -> u8 {
        v.clamp(0, 255) as u8
    }
    #[inline(always)]
    fn of(v: i32) -> u8 {
        v as u8
    }
    #[inline(always)]
    fn slice(s: &Samples) -> Option<&[u8]> {
        match s {
            Samples::B(b) => Some(b),
            Samples::W(_) => None,
        }
    }
    fn wrap(v: Box<[u8]>) -> Samples {
        Samples::B(v)
    }
    fn zeros() -> &'static [u8] {
        &ZEROS8
    }
    #[inline(always)]
    fn wide8(l: &[u8; 8]) -> Simd<u16, 8> {
        Simd::from_array(*l).cast()
    }
    #[inline(always)]
    fn narrow8(v: Simd<u16, 8>) -> [u8; 8] {
        v.cast::<u8>().to_array()
    }
    #[inline(always)]
    fn put_le(out: &mut Vec<u8>, s: &[u8]) {
        out.extend_from_slice(s);
    }
    fn bufs_in(b: Bufs<u8>) -> AnyBufs {
        AnyBufs::B(b)
    }
    fn bufs_out(b: AnyBufs) -> Option<Bufs<u8>> {
        match b {
            AnyBufs::B(b) => Some(b),
            AnyBufs::W(_) => None,
        }
    }
}

impl Pix for u16 {
    #[inline(always)]
    fn i(self) -> i32 {
        self as i32
    }
    #[inline(always)]
    fn clip(v: i32, max: i32) -> u16 {
        v.clamp(0, max) as u16
    }
    #[inline(always)]
    fn of(v: i32) -> u16 {
        v as u16
    }
    #[inline(always)]
    fn slice(s: &Samples) -> Option<&[u16]> {
        match s {
            Samples::W(b) => Some(b),
            Samples::B(_) => None,
        }
    }
    fn wrap(v: Box<[u16]>) -> Samples {
        Samples::W(v)
    }
    fn zeros() -> &'static [u16] {
        &ZEROS16
    }
    #[inline(always)]
    fn wide8(l: &[u16; 8]) -> Simd<u16, 8> {
        Simd::from_array(*l)
    }
    #[inline(always)]
    fn narrow8(v: Simd<u16, 8>) -> [u16; 8] {
        v.to_array()
    }
    /// Zero-extends the output first: converting into a slice vectorises to plain vector
    /// moves, appending sample by sample doesn't.
    #[inline(always)]
    fn put_le(out: &mut Vec<u8>, s: &[u16]) {
        let n = out.len();
        out.resize(n + 2 * s.len(), 0);
        for (d, v) in out[n..].chunks_exact_mut(2).zip(s) {
            d.copy_from_slice(&v.to_le_bytes());
        }
    }
    fn bufs_in(b: Bufs<u16>) -> AnyBufs {
        AnyBufs::W(b)
    }
    fn bufs_out(b: AnyBufs) -> Option<Bufs<u16>> {
        match b {
            AnyBufs::W(b) => Some(b),
            AnyBufs::B(_) => None,
        }
    }
}

/// Sample format of a picture: chroma format and bit depths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// chroma_format_idc: 0 (monochrome, stored as 4:2:0 with mid-grey chroma), 1, 2 or 3.
    pub cfi: u8,
    /// BitDepthY, BitDepthC.
    pub depth: [u8; 2],
}

impl Format {
    /// Samples are stored as `u16`.
    #[inline(always)]
    pub fn wide(&self) -> bool {
        self.depth[0] > 8 || self.depth[1] > 8
    }

    /// log2 of the vertical chroma subsampling: 1 for 4:2:0, 0 for 4:2:2 and 4:4:4.
    #[inline(always)]
    pub fn cshift(&self) -> usize {
        (self.cfi < 2) as usize
    }

    /// log2 of the horizontal chroma subsampling: 0 for 4:4:4, else 1.
    #[inline(always)]
    pub fn cxshift(&self) -> usize {
        (self.cfi != 3) as usize
    }

    /// Chroma samples per macroblock row: 8, or 16 in 4:4:4.
    #[inline(always)]
    pub fn mbcw(&self) -> usize {
        16 >> self.cxshift()
    }

    /// The largest sample value of luma (0) or chroma (1).
    #[inline(always)]
    pub fn max(&self, c: usize) -> i32 {
        (1 << self.depth[c]) - 1
    }

    /// Mid-grey of luma (0) or chroma (1).
    #[inline(always)]
    pub fn grey(&self, c: usize) -> i32 {
        1 << (self.depth[c] - 1)
    }
}
