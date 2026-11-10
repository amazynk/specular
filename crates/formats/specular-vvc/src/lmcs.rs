//! Luma mapping with chroma scaling: the piecewise model (7.4.3.19), forward mapping of
//! inter predictions (8.7.5.2), chroma residual scaling (8.7.5.3) and inverse mapping
//! (8.8.2).

use crate::error::{Error, Result};
use crate::picture::Picture;
use crate::ps::aps::LmcsAps;
use crate::ps::pps::Layout;

#[derive(Clone, Debug)]
pub struct Lmcs {
    pub min_bin: usize,
    pub max_bin: usize,
    pub log2_org_cw: u32,
    pub pivot: [i32; 17],
    pub scale: [i32; 16],
    pub inv_scale: [i32; 16],
    pub chroma_scale: [i32; 16],
    pub bit_depth: u32,
}

impl Lmcs {
    pub fn new(aps: &LmcsAps, bit_depth: u32) -> Result<Lmcs> {
        let org = (1i32 << bit_depth) / 16;
        let log2_org = org.trailing_zeros();
        let mut cw = [0i32; 16];
        for i in aps.min_bin_idx as usize..=aps.max_bin_idx as usize {
            cw[i] = org + aps.delta_cw[i];
            if cw[i] < org >> 3 || cw[i] > (org << 3) - 1 {
                return Err(Error::InvalidData("LMCS codeword out of range"));
            }
        }
        if cw.iter().sum::<i32>() > (1 << bit_depth) - 1 {
            return Err(Error::InvalidData("LMCS codewords exceed the sample range"));
        }
        let mut l = Lmcs {
            min_bin: aps.min_bin_idx as usize,
            max_bin: aps.max_bin_idx as usize,
            log2_org_cw: log2_org,
            pivot: [0; 17],
            scale: [0; 16],
            inv_scale: [0; 16],
            chroma_scale: [0; 16],
            bit_depth,
        };
        for i in 0..16 {
            l.pivot[i + 1] = l.pivot[i] + cw[i];
            l.scale[i] = (cw[i] * (1 << 11) + (1 << (log2_org - 1))) >> log2_org;
            l.inv_scale[i] = if cw[i] == 0 {
                0
            } else {
                org * (1 << 11) / cw[i]
            };
            l.chroma_scale[i] = if cw[i] == 0 {
                1 << 11
            } else {
                let d = cw[i] + aps.delta_crs;
                if d <= 0 {
                    return Err(Error::InvalidData("LMCS chroma scale out of range"));
                }
                org * (1 << 11) / d
            };
        }
        Ok(l)
    }

    /// Piecewise function index of a mapped luma sample (8.8.2.3).
    #[inline]
    pub fn inv_idx(&self, v: i32) -> usize {
        let mut i = self.min_bin;
        while i <= self.max_bin {
            if v < self.pivot[i + 1] {
                break;
            }
            i += 1;
        }
        i.min(15)
    }

    #[inline]
    pub fn inverse(&self, v: i32) -> u16 {
        let i = self.inv_idx(v);
        let s = (i as i32) * (1 << self.log2_org_cw)
            + ((self.inv_scale[i] * (v - self.pivot[i]) + (1 << 10)) >> 11);
        s.clamp(0, (1 << self.bit_depth) - 1) as u16
    }

    /// Forward mapping of a predicted luma sample (equation 1213), for inter prediction.
    #[allow(dead_code)]
    #[inline]
    pub fn forward(&self, v: i32) -> i32 {
        let i = ((v >> self.log2_org_cw) as usize).min(15);
        self.pivot[i]
            + ((self.scale[i] * (v - (i as i32) * (1 << self.log2_org_cw)) + (1 << 10)) >> 11)
    }

    /// varScale from the average of the neighbouring mapped luma (equation 1218).
    pub fn chroma_scale_for(&self, avg: i32) -> i32 {
        self.chroma_scale[self.inv_idx(avg)]
    }

    /// Inverse maps the luma of every CTB whose slice used LMCS.
    pub fn inverse_picture(
        &self,
        pic: &mut Picture,
        ctb_slice: &[u32],
        used: &[bool],
        layout: &Layout,
    ) {
        let n = 1usize << layout.log2_ctb;
        let lut: Vec<u16> = (0..1i32 << self.bit_depth)
            .map(|v| self.inverse(v))
            .collect();
        let p = &mut pic.planes[0];
        for cy in 0..layout.h_ctbs as usize {
            for cx in 0..layout.w_ctbs as usize {
                let s = ctb_slice[cy * layout.w_ctbs as usize + cx] as usize;
                if !used.get(s).copied().unwrap_or(false) {
                    continue;
                }
                let (x0, y0) = (cx * n, cy * n);
                for y in y0..(y0 + n).min(p.h) {
                    let row = &mut p.data[y * p.stride + x0..y * p.stride + (x0 + n).min(p.w)];
                    for v in row {
                        *v = lut[usize::from(*v)];
                    }
                }
            }
        }
    }
}
