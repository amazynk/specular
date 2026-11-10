//! Picture parameter set (7.3.2.5) and the picture partitioning it describes (6.5.1).

use crate::bits::BitReader;
use crate::error::{Error, Result};
use crate::ps::sps::Sps;

/// The deblocking offsets of one level (PPS, PH or SH): luma, Cb, Cr (beta, tc) halves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DbfOffsets {
    pub disabled: bool,
    pub beta_div2: [i32; 3],
    pub tc_div2: [i32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct Pps {
    pub id: u8,
    pub sps_id: u8,
    pub mixed_nalu_types: bool,
    pub width: u32,
    pub height: u32,
    pub conf_win: Option<[u32; 4]>,
    pub scaling_win: Option<[i32; 4]>,
    pub output_flag_present: bool,
    pub no_pic_partition: bool,
    pub subpic_id_mapping_present: bool,
    pub subpic_ids: Vec<u32>,
    pub log2_ctu_size: u32,
    pub tile_col_widths: Vec<u32>,
    pub tile_row_heights: Vec<u32>,
    pub loop_filter_across_tiles: bool,
    pub rect_slice: bool,
    pub single_slice_per_subpic: bool,
    pub num_slices_in_pic: usize,
    pub tile_idx_delta_present: bool,
    /// Per signalled slice: width and height in tiles minus 1, explicit slice heights, tile
    /// index delta.
    pub slice_w: Vec<u32>,
    pub slice_h: Vec<u32>,
    pub exp_slice_heights: Vec<Vec<u32>>,
    pub tile_idx_delta: Vec<i32>,
    pub loop_filter_across_slices: bool,
    pub cabac_init_present: bool,
    pub num_ref_idx_default_active: [u32; 2],
    pub rpl1_idx_present: bool,
    pub weighted_pred: bool,
    pub weighted_bipred: bool,
    pub ref_wraparound: bool,
    pub pic_width_minus_wraparound_offset: u32,
    pub init_qp: i32,
    pub cu_qp_delta_enabled: bool,
    pub chroma_tool_offsets_present: bool,
    pub cb_qp_offset: i32,
    pub cr_qp_offset: i32,
    pub joint_cbcr_qp_offset_present: bool,
    pub joint_cbcr_qp_offset: i32,
    pub slice_chroma_qp_offsets_present: bool,
    pub cu_chroma_qp_offset_list_enabled: bool,
    /// (cb, cr, joint) per list entry.
    pub chroma_qp_offset_list: Vec<[i32; 3]>,
    pub deblocking_override_enabled: bool,
    pub dbf: DbfOffsets,
    pub dbf_info_in_ph: bool,
    pub rpl_info_in_ph: bool,
    pub sao_info_in_ph: bool,
    pub alf_info_in_ph: bool,
    pub wp_info_in_ph: bool,
    pub qp_delta_info_in_ph: bool,
    pub picture_header_extension_present: bool,
    pub slice_header_extension_present: bool,
}

/// Explicit plus uniform sizes (equations 14 and 15).
fn sizes(explicit: &[u32], total: u32) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    let mut rem = total as i64;
    for &w in explicit {
        out.push(w);
        rem -= i64::from(w);
    }
    if rem < 0 {
        return Err(Error::InvalidData("tiles larger than the picture"));
    }
    let uni = i64::from(*explicit.last().unwrap_or(&1));
    while rem >= uni && uni > 0 {
        out.push(uni as u32);
        rem -= uni;
    }
    if rem > 0 {
        out.push(rem as u32);
    }
    Ok(out)
}

pub fn parse_pps(rbsp: &[u8]) -> Result<Pps> {
    let mut rd = BitReader::new(rbsp);
    let r = &mut rd;
    let mut p = Pps {
        id: r.u(6)? as u8,
        sps_id: r.u(4)? as u8,
        mixed_nalu_types: r.flag()?,
        width: r.ue_max(1 << 16, "picture width out of range")?,
        height: r.ue_max(1 << 16, "picture height out of range")?,
        ..Pps::default()
    };
    if p.width == 0 || p.height == 0 {
        return Err(Error::InvalidData("zero picture size"));
    }
    if r.flag()? {
        let mut w = [0; 4];
        for v in w.iter_mut() {
            *v = r.ue_max(1 << 16, "conformance window out of range")?;
        }
        p.conf_win = Some(w);
    }
    if r.flag()? {
        let mut w = [0; 4];
        for v in w.iter_mut() {
            *v = r.se()?;
        }
        p.scaling_win = Some(w);
    }
    p.output_flag_present = r.flag()?;
    p.no_pic_partition = r.flag()?;
    p.subpic_id_mapping_present = r.flag()?;
    if p.subpic_id_mapping_present {
        let n = if !p.no_pic_partition {
            r.ue_max(599, "pps_num_subpics_minus1 out of range")? + 1
        } else {
            1
        };
        let len = r.ue_max(15, "pps_subpic_id_len_minus1 out of range")? + 1;
        for _ in 0..n {
            p.subpic_ids.push(r.u(len)?);
        }
    }
    p.rect_slice = true;
    p.single_slice_per_subpic = true;
    p.log2_ctu_size = 0;
    if !p.no_pic_partition {
        p.log2_ctu_size = r.u(2)? + 5;
        let ctb = 1u32 << p.log2_ctu_size;
        let wc = p.width.div_ceil(ctb);
        let hc = p.height.div_ceil(ctb);
        let ncols = r.ue_max(wc - 1, "too many tile columns")? as usize + 1;
        let nrows = r.ue_max(hc - 1, "too many tile rows")? as usize + 1;
        let mut cw = Vec::with_capacity(ncols);
        for _ in 0..ncols {
            cw.push(r.ue_max(wc - 1, "tile column too wide")? + 1);
        }
        let mut rh = Vec::with_capacity(nrows);
        for _ in 0..nrows {
            rh.push(r.ue_max(hc - 1, "tile row too tall")? + 1);
        }
        p.tile_col_widths = sizes(&cw, wc)?;
        p.tile_row_heights = sizes(&rh, hc)?;
        let num_cols = p.tile_col_widths.len() as u32;
        let num_rows = p.tile_row_heights.len() as u32;
        let num_tiles = num_cols * num_rows;
        if num_tiles > 1 {
            p.loop_filter_across_tiles = r.flag()?;
            p.rect_slice = r.flag()?;
        }
        p.single_slice_per_subpic = false;
        if p.rect_slice {
            p.single_slice_per_subpic = r.flag()?;
        }
        if p.rect_slice && !p.single_slice_per_subpic {
            p.num_slices_in_pic = r.ue_max(599, "too many slices")? as usize + 1;
            if p.num_slices_in_pic > 2 {
                p.tile_idx_delta_present = r.flag()?;
            }
            let n = p.num_slices_in_pic;
            p.slice_w = vec![0; n];
            p.slice_h = vec![0; n];
            p.exp_slice_heights = vec![Vec::new(); n];
            p.tile_idx_delta = vec![0; n];
            // SliceTopLeftTileIdx needs the walk of equation 21 while parsing.
            let mut tile_idx: i64 = 0;
            let mut i = 0;
            while i + 1 < n {
                if tile_idx < 0 || tile_idx >= i64::from(num_tiles) {
                    return Err(Error::InvalidData("slice starts outside the tiles"));
                }
                let t = tile_idx as u32;
                if t % num_cols != num_cols - 1 {
                    p.slice_w[i] = r.ue_max(num_cols - 1, "slice too wide")?;
                }
                if t / num_cols != num_rows - 1
                    && (p.tile_idx_delta_present || t.is_multiple_of(num_cols))
                {
                    p.slice_h[i] = r.ue_max(num_rows - 1, "slice too tall")?;
                } else if t / num_cols != num_rows - 1 && i > 0 {
                    p.slice_h[i] = p.slice_h[i - 1];
                }
                let mut slices_in_tile = 1;
                let rows_in_tile = p.tile_row_heights[(t / num_cols) as usize];
                if p.slice_w[i] == 0 && p.slice_h[i] == 0 && rows_in_tile > 1 {
                    let ne = r.ue_max(rows_in_tile - 1, "too many slices in a tile")? as usize;
                    let mut hs = Vec::with_capacity(ne);
                    for _ in 0..ne {
                        hs.push(r.ue_max(rows_in_tile - 1, "slice height out of range")? + 1);
                    }
                    if ne > 0 {
                        slices_in_tile = sizes(&hs, rows_in_tile)?.len();
                    }
                    p.exp_slice_heights[i] = hs;
                }
                let last = i + slices_in_tile - 1;
                if last >= n {
                    return Err(Error::InvalidData(
                        "more slices in a tile than in the picture",
                    ));
                }
                for k in i + 1..=last {
                    p.slice_w[k] = 0;
                    p.slice_h[k] = 0;
                }
                i = last;
                if p.tile_idx_delta_present && i + 1 < n {
                    p.tile_idx_delta[i] = r.se_range(
                        -(num_tiles as i32) + 1,
                        num_tiles as i32 - 1,
                        "pps_tile_idx_delta_val out of range",
                    )?;
                }
                // Advance tileIdx as equation 21 does.
                if p.tile_idx_delta_present {
                    tile_idx += i64::from(p.tile_idx_delta[i]);
                } else {
                    tile_idx += i64::from(p.slice_w[i] + 1);
                    if tile_idx % i64::from(num_cols) == 0 {
                        tile_idx += i64::from(p.slice_h[i]) * i64::from(num_cols);
                    }
                }
                i += 1;
            }
        }
        if !p.rect_slice || p.single_slice_per_subpic || p.num_slices_in_pic > 1 {
            p.loop_filter_across_slices = r.flag()?;
        }
    }
    p.cabac_init_present = r.flag()?;
    for i in 0..2 {
        p.num_ref_idx_default_active[i] = r.ue_max(14, "default ref idx out of range")? + 1;
    }
    p.rpl1_idx_present = r.flag()?;
    p.weighted_pred = r.flag()?;
    p.weighted_bipred = r.flag()?;
    p.ref_wraparound = r.flag()?;
    if p.ref_wraparound {
        p.pic_width_minus_wraparound_offset = r.ue()?;
    }
    p.init_qp = 26 + r.se_range(-90, 37, "pps_init_qp_minus26 out of range")?;
    p.cu_qp_delta_enabled = r.flag()?;
    p.chroma_tool_offsets_present = r.flag()?;
    if p.chroma_tool_offsets_present {
        p.cb_qp_offset = r.se_range(-12, 12, "pps_cb_qp_offset out of range")?;
        p.cr_qp_offset = r.se_range(-12, 12, "pps_cr_qp_offset out of range")?;
        p.joint_cbcr_qp_offset_present = r.flag()?;
        if p.joint_cbcr_qp_offset_present {
            p.joint_cbcr_qp_offset = r.se_range(-12, 12, "joint CbCr QP offset out of range")?;
        }
        p.slice_chroma_qp_offsets_present = r.flag()?;
        p.cu_chroma_qp_offset_list_enabled = r.flag()?;
        if p.cu_chroma_qp_offset_list_enabled {
            let n = r.ue_max(5, "chroma QP offset list too long")? + 1;
            for _ in 0..n {
                let cb = r.se_range(-12, 12, "chroma QP offset out of range")?;
                let cr = r.se_range(-12, 12, "chroma QP offset out of range")?;
                let j = if p.joint_cbcr_qp_offset_present {
                    r.se_range(-12, 12, "chroma QP offset out of range")?
                } else {
                    0
                };
                p.chroma_qp_offset_list.push([cb, cr, j]);
            }
        }
    }
    if r.flag()? {
        p.deblocking_override_enabled = r.flag()?;
        p.dbf.disabled = r.flag()?;
        if !p.no_pic_partition && p.deblocking_override_enabled {
            p.dbf_info_in_ph = r.flag()?;
        }
        if !p.dbf.disabled {
            let b = r.se_range(-12, 12, "deblocking offset out of range")?;
            let t = r.se_range(-12, 12, "deblocking offset out of range")?;
            p.dbf.beta_div2 = [b; 3];
            p.dbf.tc_div2 = [t; 3];
            if p.chroma_tool_offsets_present {
                for c in 1..3 {
                    p.dbf.beta_div2[c] = r.se_range(-12, 12, "deblocking offset out of range")?;
                    p.dbf.tc_div2[c] = r.se_range(-12, 12, "deblocking offset out of range")?;
                }
            }
        }
    }
    if !p.no_pic_partition {
        p.rpl_info_in_ph = r.flag()?;
        p.sao_info_in_ph = r.flag()?;
        p.alf_info_in_ph = r.flag()?;
        if (p.weighted_pred || p.weighted_bipred) && p.rpl_info_in_ph {
            p.wp_info_in_ph = r.flag()?;
        }
        p.qp_delta_info_in_ph = r.flag()?;
    }
    p.picture_header_extension_present = r.flag()?;
    p.slice_header_extension_present = r.flag()?;
    Ok(p)
}

/// The tiles, slices and subpictures of a picture, in CTBs (6.5.1).
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub log2_ctb: u32,
    pub w_ctbs: u32,
    pub h_ctbs: u32,
    /// Tile column and row boundaries (TileColBdVal, TileRowBdVal), one past the last.
    pub col_bd: Vec<u32>,
    pub row_bd: Vec<u32>,
    /// Tile column / row index of each CTB column / row.
    pub ctb_to_col: Vec<u32>,
    pub ctb_to_row: Vec<u32>,
    /// Rectangular slices: the CTBs of each slice, in decoding order.
    pub slice_ctbs: Vec<Vec<u32>>,
    /// Subpicture of each rectangular slice and its index within the subpicture.
    pub slice_subpic: Vec<usize>,
    pub subpic_slice_count: Vec<usize>,
}

impl Layout {
    pub fn num_tile_cols(&self) -> u32 {
        self.col_bd.len() as u32 - 1
    }
    pub fn num_tile_rows(&self) -> u32 {
        self.row_bd.len() as u32 - 1
    }
    pub fn num_tiles(&self) -> u32 {
        self.num_tile_cols() * self.num_tile_rows()
    }
    /// Tile index of a CTB.
    pub fn tile_of(&self, x: u32, y: u32) -> u32 {
        self.ctb_to_row[y as usize] * self.num_tile_cols() + self.ctb_to_col[x as usize]
    }
    /// The CTBs of tile `t` in raster order within the tile.
    pub fn tile_ctbs(&self, t: u32, out: &mut Vec<u32>) {
        let tx = (t % self.num_tile_cols()) as usize;
        let ty = (t / self.num_tile_cols()) as usize;
        for y in self.row_bd[ty]..self.row_bd[ty + 1] {
            for x in self.col_bd[tx]..self.col_bd[tx + 1] {
                out.push(y * self.w_ctbs + x);
            }
        }
    }

    pub fn new(sps: &Sps, pps: &Pps) -> Result<Layout> {
        let log2_ctb = sps.log2_ctu_size;
        if !pps.no_pic_partition && pps.log2_ctu_size != log2_ctb {
            return Err(Error::InvalidData("PPS and SPS CTU sizes differ"));
        }
        let ctb = 1u32 << log2_ctb;
        let w_ctbs = pps.width.div_ceil(ctb);
        let h_ctbs = pps.height.div_ceil(ctb);
        let cols = if pps.no_pic_partition {
            vec![w_ctbs]
        } else {
            pps.tile_col_widths.clone()
        };
        let rows = if pps.no_pic_partition {
            vec![h_ctbs]
        } else {
            pps.tile_row_heights.clone()
        };
        let mut col_bd = vec![0];
        for c in &cols {
            col_bd.push(col_bd.last().unwrap_or(&0) + c);
        }
        let mut row_bd = vec![0];
        for r in &rows {
            row_bd.push(row_bd.last().unwrap_or(&0) + r);
        }
        if *col_bd.last().unwrap_or(&0) != w_ctbs || *row_bd.last().unwrap_or(&0) != h_ctbs {
            return Err(Error::InvalidData("tiles do not cover the picture"));
        }
        let map = |bd: &[u32], n: u32| -> Vec<u32> {
            let mut v = Vec::with_capacity(n as usize + 1);
            let mut t = 0usize;
            for a in 0..=n {
                if t + 2 < bd.len() && a == bd[t + 1] {
                    t += 1;
                }
                v.push(t as u32);
            }
            v
        };
        let mut l = Layout {
            log2_ctb,
            w_ctbs,
            h_ctbs,
            ctb_to_col: map(&col_bd, w_ctbs),
            ctb_to_row: map(&row_bd, h_ctbs),
            col_bd,
            row_bd,
            ..Layout::default()
        };
        if pps.rect_slice {
            l.rect_slices(sps, pps)?;
        }
        Ok(l)
    }

    fn add(&self, s: &mut Vec<u32>, x0: u32, x1: u32, y0: u32, y1: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                s.push(y * self.w_ctbs + x);
            }
        }
    }

    fn rect_slices(&mut self, sps: &Sps, pps: &Pps) -> Result<()> {
        let nc = self.num_tile_cols();
        let nr = self.num_tile_rows();
        let mut slices: Vec<Vec<u32>> = Vec::new();
        if pps.single_slice_per_subpic {
            if !sps.subpic_info_present {
                let mut s = Vec::new();
                for j in 0..nr as usize {
                    for i in 0..nc as usize {
                        self.add(
                            &mut s,
                            self.col_bd[i],
                            self.col_bd[i + 1],
                            self.row_bd[j],
                            self.row_bd[j + 1],
                        );
                    }
                }
                slices.push(s);
            } else {
                for sp in &sps.subpics {
                    let mut s = Vec::new();
                    let (lx, ty) = (sp.ctu_x, sp.ctu_y);
                    let (rx, by) = (lx + sp.w - 1, ty + sp.h - 1);
                    if rx >= self.w_ctbs || by >= self.h_ctbs {
                        return Err(Error::InvalidData("subpicture outside the picture"));
                    }
                    let tx = self.ctb_to_col[lx as usize];
                    let tyi = self.ctb_to_row[ty as usize];
                    let wt = self.ctb_to_col[rx as usize] + 1 - tx;
                    let ht = self.ctb_to_row[by as usize] + 1 - tyi;
                    let less =
                        ht == 1 && sp.h < self.row_bd[tyi as usize + 1] - self.row_bd[tyi as usize];
                    if less {
                        self.add(&mut s, lx, lx + sp.w, ty, ty + sp.h);
                    } else {
                        for j in 0..ht {
                            for k in 0..wt {
                                let (a, b) = ((tx + k) as usize, (tyi + j) as usize);
                                self.add(
                                    &mut s,
                                    self.col_bd[a],
                                    self.col_bd[a + 1],
                                    self.row_bd[b],
                                    self.row_bd[b + 1],
                                );
                            }
                        }
                    }
                    slices.push(s);
                }
            }
        } else {
            let n = pps.num_slices_in_pic;
            slices = vec![Vec::new(); n];
            let mut tile_idx: i64 = 0;
            let mut i = 0;
            while i < n {
                if tile_idx < 0 || tile_idx >= i64::from(nc * nr) {
                    return Err(Error::InvalidData("slice starts outside the tiles"));
                }
                let tx = (tile_idx as u32 % nc) as usize;
                let ty = (tile_idx as u32 / nc) as usize;
                let (sw, sh) = if i + 1 < n {
                    (pps.slice_w[i] + 1, pps.slice_h[i] + 1)
                } else {
                    (nc - tx as u32, nr - ty as u32)
                };
                if tx + sw as usize > nc as usize || ty + sh as usize > nr as usize {
                    return Err(Error::InvalidData("slice outside the tiles"));
                }
                let mut advance_w = sw;
                let mut advance_h = sh;
                if sw == 1 && sh == 1 {
                    let tile_h = self.row_bd[ty + 1] - self.row_bd[ty];
                    let heights = if i + 1 < n && !pps.exp_slice_heights[i].is_empty() {
                        sizes(&pps.exp_slice_heights[i], tile_h)?
                    } else {
                        vec![tile_h]
                    };
                    let mut y = self.row_bd[ty];
                    for (j, h) in heights.iter().enumerate() {
                        if i + j >= n {
                            return Err(Error::InvalidData("too many slices in a tile"));
                        }
                        let mut s = Vec::new();
                        self.add(&mut s, self.col_bd[tx], self.col_bd[tx + 1], y, y + h);
                        slices[i + j] = s;
                        y += h;
                    }
                    i += heights.len() - 1;
                    advance_w = 1;
                    advance_h = 1;
                } else {
                    let mut s = Vec::new();
                    for j in 0..sh as usize {
                        for k in 0..sw as usize {
                            self.add(
                                &mut s,
                                self.col_bd[tx + k],
                                self.col_bd[tx + k + 1],
                                self.row_bd[ty + j],
                                self.row_bd[ty + j + 1],
                            );
                        }
                    }
                    slices[i] = s;
                }
                if i + 1 < n {
                    if pps.tile_idx_delta_present {
                        tile_idx += i64::from(pps.tile_idx_delta[i]);
                    } else {
                        tile_idx += i64::from(advance_w);
                        if tile_idx % i64::from(nc) == 0 {
                            tile_idx += i64::from(advance_h - 1) * i64::from(nc);
                        }
                    }
                }
                i += 1;
            }
        }
        let total: usize = slices.iter().map(Vec::len).sum();
        if total != (self.w_ctbs * self.h_ctbs) as usize || slices.iter().any(Vec::is_empty) {
            return Err(Error::InvalidData("slices do not cover the picture"));
        }
        self.slice_subpic = vec![0; slices.len()];
        self.subpic_slice_count = vec![0; sps.subpics.len()];
        for (j, s) in slices.iter().enumerate() {
            let x = s[0] % self.w_ctbs;
            let y = s[0] / self.w_ctbs;
            for (k, sp) in sps.subpics.iter().enumerate() {
                if x >= sp.ctu_x && x < sp.ctu_x + sp.w && y >= sp.ctu_y && y < sp.ctu_y + sp.h {
                    self.slice_subpic[j] = k;
                    self.subpic_slice_count[k] += 1;
                    break;
                }
            }
        }
        self.slice_ctbs = slices;
        Ok(())
    }
}
