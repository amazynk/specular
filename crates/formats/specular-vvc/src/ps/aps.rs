//! Adaptation parameter sets (7.3.2.6): ALF ([`crate::ps::alf`]), LMCS and scaling lists.

use crate::bits::BitReader;
use crate::error::{Error, Result};
use crate::ps::alf::{AlfAps, parse_alf_data};

pub const ALF_APS: u32 = 0;
pub const LMCS_APS: u32 = 1;
pub const SCALING_APS: u32 = 2;

/// `lmcs_data()` as signalled (7.3.2.19); the piecewise model is built per picture (it
/// depends on the bit depth), see [`crate::lmcs`].
#[derive(Clone, Debug, Default)]
pub struct LmcsAps {
    pub min_bin_idx: u32,
    pub max_bin_idx: u32,
    pub delta_cw: [i32; 16],
    pub delta_crs: i32,
}

/// The scaling matrices of a scaling list APS (7.4.3.20): ScalingMatrixRec[ id ] (id 0..27)
/// as `matrixSize x matrixSize` in raster order (`[y * size + x]`), and the DC values for
/// ids 14 to 27.
#[derive(Clone, Debug)]
pub struct ScalingAps {
    pub rec: Vec<Vec<u8>>,
    pub dc: [u8; 14],
}

#[derive(Clone, Debug)]
pub enum Aps {
    Alf(Box<AlfAps>),
    Lmcs(LmcsAps),
    Scaling(Box<ScalingAps>),
}

/// Parses an APS RBSP: (type, id, content).
pub fn parse_aps(rbsp: &[u8]) -> Result<(u32, u8, Aps)> {
    let mut rd = BitReader::new(rbsp);
    let r = &mut rd;
    let ty = r.u(3)?;
    let id = r.u(5)? as u8;
    let chroma = r.flag()?;
    let aps = match ty {
        ALF_APS => {
            if id > 7 {
                return Err(Error::InvalidData("ALF APS id above 7"));
            }
            Aps::Alf(Box::new(parse_alf_data(r, chroma)?))
        }
        LMCS_APS => {
            if id > 3 {
                return Err(Error::InvalidData("LMCS APS id above 3"));
            }
            Aps::Lmcs(parse_lmcs(r, chroma)?)
        }
        SCALING_APS => {
            if id > 7 {
                return Err(Error::InvalidData("scaling list APS id above 7"));
            }
            Aps::Scaling(Box::new(parse_scaling(r, chroma)?))
        }
        _ => {
            return Err(Error::Unsupported(
                "an adaptation parameter set of an unknown type",
            ));
        }
    };
    Ok((ty, id, aps))
}

fn parse_lmcs(r: &mut BitReader<'_>, chroma: bool) -> Result<LmcsAps> {
    let min = r.ue_max(15, "lmcs_min_bin_idx out of range")?;
    let max = 15 - r.ue_max(15, "lmcs_delta_max_bin_idx out of range")?;
    if max < min {
        return Err(Error::InvalidData("LMCS bins out of order"));
    }
    let bits = r.ue_max(14, "lmcs_delta_cw_prec_minus1 out of range")? + 1;
    let mut l = LmcsAps {
        min_bin_idx: min,
        max_bin_idx: max,
        ..LmcsAps::default()
    };
    for i in min..=max {
        let a = r.u(bits)? as i32;
        let neg = if a > 0 { r.flag()? } else { false };
        l.delta_cw[i as usize] = if neg { -a } else { a };
    }
    if chroma {
        let a = r.u(3)? as i32;
        let neg = if a > 0 { r.flag()? } else { false };
        l.delta_crs = if neg { -a } else { a };
    }
    Ok(l)
}

/// Up-right diagonal scan of a `size x size` block (6.5.2): (x, y) per position.
pub fn diag_scan(w: usize, h: usize) -> Vec<(u8, u8)> {
    let mut out = Vec::with_capacity(w * h);
    let (mut x, mut y) = (0i32, 0i32);
    while out.len() < w * h {
        while y >= 0 {
            if (x as usize) < w && (y as usize) < h {
                out.push((x as u8, y as u8));
            }
            y -= 1;
            x += 1;
        }
        y = x;
        x = 0;
    }
    out
}

fn parse_scaling(r: &mut BitReader<'_>, chroma: bool) -> Result<ScalingAps> {
    let mut rec: Vec<Vec<u8>> = Vec::with_capacity(28);
    let mut dc = [0u8; 14];
    for id in 0..28usize {
        let size = if id < 2 {
            2
        } else if id < 8 {
            4
        } else {
            8
        };
        let mut copy = true;
        let mut pred = false;
        let mut delta = 0usize;
        let mut list = vec![0i32; size * size];
        let mut dc_coef = 0i32;
        if chroma || id % 3 == 2 || id == 27 {
            copy = r.flag()?;
            if !copy {
                pred = r.flag()?;
            }
            if (copy || pred) && id != 0 && id != 2 && id != 8 {
                let max = if id < 2 {
                    id
                } else if id < 8 {
                    id - 2
                } else {
                    id - 8
                };
                delta = r.ue_max(max as u32, "scaling_list_pred_id_delta out of range")? as usize;
            }
            if !copy {
                let mut next = 0i32;
                if id > 13 {
                    dc_coef = r.se_range(-128, 127, "scaling_list_dc_coef out of range")?;
                    next += dc_coef;
                }
                let scan = diag_scan(8, 8);
                for (i, l) in list.iter_mut().enumerate() {
                    // The skip test uses DiagScanOrder[ 3 ][ 3 ] whatever the matrix size.
                    let (x, y) = scan[i];
                    if !(id > 25 && x >= 4 && y >= 4) {
                        next += r.se_range(-128, 127, "scaling_list_delta_coef out of range")?;
                    }
                    *l = next;
                }
            }
        }
        // scalingMatrixPred / scalingMatrixDcPred.
        let ref_id = id - delta;
        let (pred_m, pred_dc): (Vec<i32>, i32) = if !copy && !pred {
            (vec![8; size * size], 8)
        } else if delta == 0 {
            (vec![16; size * size], 16)
        } else {
            let m: Vec<i32> = rec[ref_id].iter().map(|&v| i32::from(v)).collect();
            let d = if ref_id > 13 {
                i32::from(dc[ref_id - 14])
            } else {
                m[0]
            };
            (m, d)
        };
        let scan = diag_scan(size, size);
        let mut m = vec![0u8; size * size];
        for (k, &(x, y)) in scan.iter().enumerate() {
            let p = usize::from(y) * size + usize::from(x);
            let v = (pred_m[p] + list[k]) & 255;
            if v == 0 {
                return Err(Error::InvalidData("zero scaling matrix entry"));
            }
            m[p] = v as u8;
        }
        if id > 13 {
            let v = (pred_dc + dc_coef) & 255;
            if v == 0 {
                return Err(Error::InvalidData("zero scaling matrix DC"));
            }
            dc[id - 14] = v as u8;
        }
        rec.push(m);
    }
    Ok(ScalingAps { rec, dc })
}
