//! A picture under reconstruction: one `u16` per sample, planes with a stride.

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default)]
pub struct Plane {
    pub data: Vec<u16>,
    pub stride: usize,
    pub w: usize,
    pub h: usize,
}

impl Plane {
    pub fn new(w: usize, h: usize) -> Plane {
        Plane {
            data: vec![0; w * h],
            stride: w,
            w,
            h,
        }
    }
    #[cfg(test)]
    pub fn set(&mut self, x: usize, y: usize, v: u16) {
        self.data[y * self.stride + x] = v;
    }
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> u16 {
        self.data[y * self.stride + x]
    }
}

/// The decoded picture: Y, and Cb and Cr unless monochrome.
#[derive(Clone, Debug, Default)]
pub struct Picture {
    pub planes: Vec<Plane>,
    /// sps_chroma_format_idc.
    pub chroma_format_idc: u8,
    pub bit_depth: u32,
}

impl Picture {
    pub fn new(w: u32, h: u32, chroma_format_idc: u8, bit_depth: u32) -> Result<Picture> {
        let (w, h) = (w as usize, h as usize);
        let luma = w.checked_mul(h).ok_or(Error::TooLarge)?;
        let (sw, sh) = match chroma_format_idc {
            1 => (2, 2),
            2 => (2, 1),
            _ => (1, 1),
        };
        let chroma = if chroma_format_idc == 0 {
            0
        } else {
            2 * w.div_ceil(sw) * h.div_ceil(sh)
        };
        let _ = luma + chroma;
        let mut planes = vec![Plane::new(w, h)];
        if chroma_format_idc != 0 {
            planes.push(Plane::new(w.div_ceil(sw), h.div_ceil(sh)));
            planes.push(Plane::new(w.div_ceil(sw), h.div_ceil(sh)));
        }
        Ok(Picture {
            planes,
            chroma_format_idc,
            bit_depth,
        })
    }
}
