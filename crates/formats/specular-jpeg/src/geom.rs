//! Output geometry: which MCUs are decoded and how big each output plane is.

use crate::error::{JpegError, Result};
use crate::marker::Frame;
use crate::{Region, Scale};

#[derive(Clone, Debug)]
pub(crate) struct CompOut {
    /// Visible size of this component's plane in samples.
    pub w: u32,
    pub h: u32,
    /// Row stride (= padded width, a whole number of MCUs).
    pub stride: usize,
    /// Padded height in rows (whole MCUs).
    pub padded_h: usize,
    /// Output samples per block edge for this component (its scaled IDCT size).
    pub n: usize,
    /// Vertical blocks per MCU (the component's frame sampling factor).
    pub vb: u32,
    /// Effective sampling factors of the decoded plane relative to `max_h`/`max_v` (equal to the
    /// frame factors unless the component was decoded with a larger IDCT, see [`comp_block_size`]).
    pub h_samp: u32,
    pub v_samp: u32,
}

/// libjpeg's per-component scaled IDCT size (jdmaster.c): for scaled decodes a subsampled
/// component uses a larger IDCT (up to 8) so that less (or no) upsampling is needed, e.g. 4:2:0
/// chroma at 1/2 scale is decoded with the full 8x8 IDCT and needs no upsampling at all.
/// Not applied at 1/8 scale, which stays DC-only for every component.
pub(crate) fn comp_block_size(f: &Frame, h: u32, v: u32, n: usize) -> usize {
    let mut s = n as u32;
    if n == 1 {
        return 1;
    }
    while s < 8
        && (f.max_h * n as u32).is_multiple_of(h * s * 2)
        && (f.max_v * n as u32).is_multiple_of(v * s * 2)
    {
        s *= 2;
    }
    s as usize
}

#[derive(Clone, Debug)]
pub(crate) struct OutGeom {
    /// Output block size in samples: 8 / scale.
    pub n: usize,
    pub mx0: u32,
    pub mx1: u32,
    pub my0: u32,
    pub my1: u32,
    pub origin: (u32, u32),
    pub width: u32,
    pub height: u32,
    pub comps: Vec<CompOut>,
}

impl OutGeom {
    pub fn new(f: &Frame, scale: Scale, region: Option<Region>) -> Result<OutGeom> {
        let s = scale as u32;
        let n = (8 / s) as usize;
        let out_w = f.width.div_ceil(s);
        let out_h = f.height.div_ceil(s);
        let mcu_w = f.max_h * n as u32;
        let mcu_h = f.max_v * n as u32;
        let (mx0, mx1, my0, my1) = match region {
            None => (0, f.mcus_x, 0, f.mcus_y),
            Some(r) => {
                if r.w == 0 || r.h == 0 || r.x >= out_w || r.y >= out_h {
                    return Err(JpegError::Unsupported("empty or out-of-bounds region"));
                }
                let x1 = (r.x as u64 + r.w as u64).min(out_w as u64) as u32;
                let y1 = (r.y as u64 + r.h as u64).min(out_h as u64) as u32;
                (
                    r.x / mcu_w,
                    x1.div_ceil(mcu_w).min(f.mcus_x),
                    r.y / mcu_h,
                    y1.div_ceil(mcu_h).min(f.mcus_y),
                )
            }
        };
        let origin = (mx0 * mcu_w, my0 * mcu_h);
        let width = (mx1 * mcu_w).min(out_w) - origin.0;
        let height = (my1 * mcu_h).min(out_h) - origin.1;
        let comps = f
            .comps
            .iter()
            .map(|c| {
                let nc = comp_block_size(f, c.h, c.v, n);
                let full_w =
                    (f.width as u64 * c.h as u64 * nc as u64).div_ceil(f.max_h as u64 * 8) as u32;
                let full_h =
                    (f.height as u64 * c.v as u64 * nc as u64).div_ceil(f.max_v as u64 * 8) as u32;
                let bw = c.h * nc as u32;
                let bh = c.v * nc as u32;
                CompOut {
                    w: (mx1 * bw).min(full_w) - mx0 * bw,
                    h: (my1 * bh).min(full_h) - my0 * bh,
                    stride: ((mx1 - mx0) * bw) as usize,
                    padded_h: ((my1 - my0) * bh) as usize,
                    n: nc,
                    vb: c.v,
                    h_samp: c.h * (nc / n) as u32,
                    v_samp: c.v * (nc / n) as u32,
                }
            })
            .collect();
        Ok(OutGeom {
            n,
            mx0,
            mx1,
            my0,
            my1,
            origin,
            width,
            height,
            comps,
        })
    }

    pub fn total_bytes(&self) -> u64 {
        self.comps
            .iter()
            .map(|c| c.stride as u64 * c.padded_h as u64)
            .sum()
    }

    /// Bytes of plane `c` covering one MCU row.
    pub fn mcu_row_bytes(&self, c: usize) -> usize {
        self.comps[c].stride * self.comps[c].vb as usize * self.comps[c].n
    }
}

/// Allocate a zeroed buffer, returning an error instead of aborting when it can't be allocated.
pub(crate) fn alloc_zeroed(n: usize) -> Result<Vec<u8>> {
    let mut probe: Vec<u8> = Vec::new();
    probe
        .try_reserve_exact(n)
        .map_err(|_| JpegError::TooLarge(n as u64))?;
    drop(probe);
    Ok(vec![0u8; n])
}

pub(crate) fn alloc_u64(n: usize) -> Result<Vec<u64>> {
    let mut probe: Vec<u64> = Vec::new();
    probe
        .try_reserve_exact(n)
        .map_err(|_| JpegError::TooLarge(n as u64 * 8))?;
    drop(probe);
    Ok(vec![0u64; n])
}

pub(crate) fn alloc_i16(n: usize) -> Result<Vec<i16>> {
    let mut probe: Vec<i16> = Vec::new();
    probe
        .try_reserve_exact(n)
        .map_err(|_| JpegError::TooLarge(n as u64 * 2))?;
    drop(probe);
    Ok(vec![0i16; n])
}

/// Split each plane into consecutive row bands of `band_rows[i]` MCU rows.
pub(crate) fn split_bands<'a>(
    planes: &'a mut [Vec<u8>],
    g: &OutGeom,
    band_rows: &[u32],
) -> Vec<Vec<&'a mut [u8]>> {
    let mut out: Vec<Vec<&'a mut [u8]>> = band_rows
        .iter()
        .map(|_| Vec::with_capacity(planes.len()))
        .collect();
    for (ci, p) in planes.iter_mut().enumerate() {
        let row_bytes = g.mcu_row_bytes(ci);
        let mut rest: &'a mut [u8] = p.as_mut_slice();
        for (bi, &r) in band_rows.iter().enumerate() {
            let take = (r as usize * row_bytes).min(rest.len());
            let (a, b) = std::mem::take(&mut rest).split_at_mut(take);
            out[bi].push(a);
            rest = b;
        }
    }
    out
}
