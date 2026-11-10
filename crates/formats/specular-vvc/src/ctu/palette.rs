//! Palette mode: `palette_coding()` (7.3.11.6, 7.4.12.6) and its decoding (8.4.5.3).

use crate::cabac::{self, Engine};
use crate::error::{Error, Result};

use super::cu::Cu;
use super::{Dec, TreeType};

/// EGk bypass bins (9.3.3.5).
fn egk(e: &mut Engine<'_>, mut k: u32) -> Result<u32> {
    let mut v = 0u32;
    while e.bypass() == 1 {
        v = v
            .checked_add(1 << k)
            .ok_or(Error::InvalidData("Exp-Golomb value too large"))?;
        k += 1;
        if k > 30 {
            return Err(Error::InvalidData("Exp-Golomb value too large"));
        }
    }
    Ok(v + e.bypass_bits(k))
}

/// Horizontal (or, transposed, vertical) traverse scan of a `w x h` block (6.5.3).
fn traverse(w: usize, h: usize, transpose: bool) -> Vec<(u16, u16)> {
    let mut v = Vec::with_capacity(w * h);
    if !transpose {
        for y in 0..h {
            if y % 2 == 0 {
                (0..w).for_each(|x| v.push((x as u16, y as u16)));
            } else {
                (0..w).rev().for_each(|x| v.push((x as u16, y as u16)));
            }
        }
    } else {
        for x in 0..w {
            if x % 2 == 0 {
                (0..h).for_each(|y| v.push((x as u16, y as u16)));
            } else {
                (0..h).rev().for_each(|y| v.push((x as u16, y as u16)));
            }
        }
    }
    v
}

const RUN_CTX: [[usize; 5]; 2] = [[0, 1, 2, 3, 4], [5, 6, 6, 7, 7]];

impl Dec<'_> {
    /// Parses and reconstructs a palette coding unit (the luma or single tree call, or the
    /// dual-tree chroma one).
    pub(super) fn palette_cu(&mut self, e: &mut Engine<'_>, cu: &mut Cu) -> Result<()> {
        let sps = self.c.sps;
        let tree = cu.tree;
        let single = tree == TreeType::Single;
        let chroma_tree = tree == TreeType::DualChroma;
        let start = usize::from(chroma_tree);
        let ncomp = if single {
            if self.chroma { 3 } else { 1 }
        } else if chroma_tree {
            2
        } else {
            1
        };
        let max_entries = if single { 31 } else { 15 };
        let (bw, bh) = if chroma_tree {
            ((cu.w >> self.sw) as usize, (cu.h >> self.sh) as usize)
        } else {
            (cu.w as usize, cu.h as usize)
        };
        let local_dual = tree != TreeType::Single && !self.dual_tree;
        let psize = self.pal.size[start];
        let mut reuse = vec![false; psize];
        let mut num_pred = 0usize;
        let mut i = 0usize;
        while i < psize && num_pred < max_entries {
            let run = egk(e, 0)? as usize;
            if run == 1 {
                break;
            }
            if run > 1 {
                i += run - 1;
            }
            if i >= psize {
                return Err(Error::InvalidData(
                    "palette predictor run past the predictor",
                ));
            }
            reuse[i] = true;
            num_pred += 1;
            i += 1;
        }
        let num_sig = if num_pred < max_entries {
            egk(e, 0)? as usize
        } else {
            0
        };
        let size = num_pred + num_sig;
        if size > max_entries {
            return Err(Error::InvalidData("palette too large"));
        }
        let mut cur: [Vec<u16>; 3] = Default::default();
        let comps: Vec<usize> = if local_dual {
            (0..3).collect()
        } else {
            (start..start + ncomp).collect()
        };
        for (k, &r) in reuse.iter().enumerate() {
            if r {
                for &c in &comps {
                    let v = self.pal.entries[c].get(k).copied().unwrap_or(0);
                    cur[c].push(v);
                }
            }
        }
        for c in start..start + ncomp {
            cur[c].resize(num_pred, 0);
            for _ in 0..num_sig {
                cur[c].push(e.bypass_bits(self.bd) as u16);
            }
        }
        let escape = if size > 0 { e.bypass() == 1 } else { true };
        let max_idx = size as i32 - 1 + i32::from(escape);
        let mut transpose = false;
        if max_idx > 0 {
            transpose = e.decode(&mut self.ctx.0[cabac::PALETTE_TRANSPOSE_FLAG]) == 1;
        }
        if tree != TreeType::DualChroma
            && escape
            && self.c.pps.cu_qp_delta_enabled
            && !self.is_cu_qp_delta_coded
        {
            self.cu_qp_delta(e)?;
        }
        if tree != TreeType::DualLuma
            && escape
            && self.c.sh.cu_chroma_qp_offset_enabled
            && !self.is_cu_chroma_qp_offset_coded
        {
            self.cu_chroma_qp_offset(e);
        }
        let n = bw * bh;
        let scan = traverse(bw, bh, transpose);
        let mut map = vec![0u8; n];
        let mut copy_above = vec![false; n];
        let mut run_copy = vec![false; n];
        let mut esc: [Vec<u16>; 3] = [vec![0; n], vec![0; n], vec![0; n]];
        let mut prev_run_type = 0usize;
        let mut prev_run_pos = 0usize;
        let mut adjust = false;
        let mut curr_idx: i32 = 0;
        let at = |p: (u16, u16)| usize::from(p.1) * bw + usize::from(p.0);
        let (subw, subh) = (1usize << self.sw, 1usize << self.sh);
        let mut sub = 0;
        while sub * 16 < n {
            let min_pos = sub * 16;
            let max_pos = (min_pos + 16).min(n);
            for pos in min_pos..max_pos {
                let q = at(scan[pos]);
                let (xc, yc) = scan[pos];
                if max_idx > 0 && pos > 0 {
                    let dist = (pos - prev_run_pos - 1).min(4);
                    let inc = RUN_CTX[prev_run_type][dist];
                    run_copy[q] = e.decode(&mut self.ctx.0[cabac::RUN_COPY_FLAG + inc]) == 1;
                }
                copy_above[q] = false;
                if max_idx > 0 && !run_copy[q] {
                    let qp = if pos > 0 { at(scan[pos - 1]) } else { 0 };
                    if ((!transpose && yc > 0) || (transpose && xc > 0))
                        && pos > 0
                        && !copy_above[qp]
                    {
                        copy_above[q] =
                            e.decode(&mut self.ctx.0[cabac::COPY_ABOVE_PALETTE_INDICES_FLAG]) == 1;
                    }
                    prev_run_type = usize::from(copy_above[q]);
                    prev_run_pos = pos;
                } else if pos > 0 {
                    copy_above[q] = copy_above[at(scan[pos - 1])];
                }
            }
            for pos in min_pos..max_pos {
                let q = at(scan[pos]);
                let (xc, yc) = scan[pos];
                if !run_copy[q] && !copy_above[q] {
                    let mut idc = 0i32;
                    if max_idx > 0 {
                        if max_idx - i32::from(adjust) > 0 {
                            let cmax = if adjust { max_idx - 1 } else { max_idx };
                            idc = super::tb_bypass(e, cmax as u32) as i32;
                        }
                        adjust = true;
                    }
                    // adjustedRefPaletteIndex (7.4.12.6).
                    let mut r = max_idx + 1;
                    if pos > 0 {
                        let qp = at(scan[pos - 1]);
                        r = if !copy_above[qp] {
                            i32::from(map[qp])
                        } else if !transpose {
                            i32::from(map[q - bw])
                        } else {
                            i32::from(map[q - 1])
                        };
                    }
                    if max_idx > 0 && idc >= r {
                        idc += 1;
                    }
                    curr_idx = idc;
                }
                map[q] = if !copy_above[q] {
                    curr_idx as u8
                } else if !transpose {
                    map[q - bw]
                } else {
                    map[q - 1]
                };
                let _ = (xc, yc);
            }
            if escape {
                for c in start..start + ncomp {
                    for pos in min_pos..max_pos {
                        let (xc, yc) = scan[pos];
                        let (xc, yc) = (usize::from(xc), usize::from(yc));
                        if single && c != 0 && (xc % subw != 0 || yc % subh != 0) {
                            continue;
                        }
                        let q = yc * bw + xc;
                        if i32::from(map[q]) == max_idx {
                            let v = egk(e, 5)?;
                            esc[c][q] = v.min(u32::from(u16::MAX)) as u16;
                        }
                    }
                }
            }
            sub += 1;
        }
        // Local dual tree luma: chroma entries of the signalled colours are mid-grey.
        if local_dual && tree == TreeType::DualLuma {
            for c in 1..3 {
                cur[c].resize(num_pred, 1 << (self.bd - 1));
                for _ in 0..num_sig {
                    cur[c].push(1 << (self.bd - 1));
                }
            }
        }
        self.cu_qp(cu);
        // Reconstruction (8.4.5.3).
        let bd = self.bd;
        let levels = [40i64, 45, 51, 57, 64, 72];
        for c in start..start + ncomp {
            let (sx, sy) = if c > 0 && !chroma_tree {
                (self.sw, self.sh)
            } else {
                (0, 0)
            };
            let (cw, ch) = (bw >> sx, bh >> sy);
            let (ox, oy) = if c == 0 {
                (cu.x0 as usize, cu.y0 as usize)
            } else {
                ((cu.x0 >> self.sw) as usize, (cu.y0 >> self.sh) as usize)
            };
            let qp = sps.min_qp_prime_ts.max(cu.qp[c]);
            let plane = &mut self.st.pic.planes[c];
            let stride = plane.stride;
            for y in 0..ch {
                for x in 0..cw {
                    let q = (y << sy) * bw + (x << sx);
                    let idx = usize::from(map[q]);
                    let v = if escape && idx as i32 == max_idx {
                        let t =
                            ((i64::from(esc[c][q]) * levels[(qp % 6) as usize]) << (qp / 6)) + 32;
                        (t >> 6).clamp(0, (1 << bd) - 1) as u16
                    } else {
                        cur[c].get(idx).copied().unwrap_or(0)
                    };
                    plane.data[(oy + y) * stride + ox + x] = v;
                }
            }
            self.ibc_store(c, ox, oy, cw, ch);
            let ci = c;
            let m = &mut self.st.maps;
            for i in m.cells(cu.x0, cu.y0, cu.w, cu.h) {
                m.avail[ci][i] = true;
            }
        }
        // Predictor update.
        let (pstart, pcomps, max_pred) = if local_dual {
            (0, 3, 63)
        } else if single {
            (0, ncomp, 63)
        } else {
            (start, ncomp, 31)
        };
        let cur_size = size;
        let mut newp: [Vec<u16>; 3] = Default::default();
        for c in pstart..pstart + pcomps {
            newp[c] = cur[c][..cur_size.min(cur[c].len())].to_vec();
            newp[c].resize(cur_size, 0);
        }
        let mut nsize = cur_size;
        let old = self.pal.size[pstart];
        for k in 0..old {
            if nsize >= max_pred {
                break;
            }
            if !reuse.get(k).copied().unwrap_or(false) {
                for c in pstart..pstart + pcomps {
                    let v = self.pal.entries[c].get(k).copied().unwrap_or(0);
                    newp[c].push(v);
                }
                nsize += 1;
            }
        }
        for c in pstart..pstart + pcomps {
            self.pal.entries[c] = std::mem::take(&mut newp[c]);
        }
        self.pal.size[pstart] = nsize;
        if !sps.qtbtt_dual_tree_intra || !self.intra_slice {
            self.pal.size[1] = nsize;
        }
        Ok(())
    }
}
