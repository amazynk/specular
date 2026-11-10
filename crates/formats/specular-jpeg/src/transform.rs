//! Lossless rotation and mirroring in the DCT domain (the operations behind `jpegtran -rotate`,
//! written from ITU T.81 and the DCT's symmetries).
//!
//! Every quantised coefficient is read (any Huffman or arithmetic DCT frame, sequential or
//! progressive, 8 or 12 bits), then each 8x8 block is moved to its new place in the block grid
//! and transformed in place:
//! * a mirror along x negates the coefficients of odd horizontal frequency (`C(u, v) (-1)^u`),
//!   a mirror along y those of odd vertical frequency;
//! * a transpose swaps the frequency axes of every block (`C(v, u)`), swaps the block grid and
//!   the sampling factors of every component, and transposes the quantisation tables.
//!
//! No coefficient is requantised, so decoding the result gives exactly the transformed pixels
//! of the original. The one restriction is the edge: a mirrored axis must be a whole number of
//! iMCUs long, because the partial iMCU at the right or bottom cannot move to the left or top
//! (the padding would show). [`transform_alignment`] reports this; the caller then either
//! trims that partial iMCU ([`TransformOptions::trim`], the image loses up to 15 pixels on
//! that edge) or records the orientation in metadata instead.
//!
//! The output is a sequential Huffman JPEG with optimised tables (baseline for 8-bit data).
//! Application segments and comments are copied verbatim (the JFIF pixel density is swapped
//! by a transpose); orientation tags and Exif dimensions are the caller's to update.

use crate::encode::{BitWriter, Huff, magnitude, optimal_table, segment};
use crate::error::{JpegError, Result};
use crate::huffman::{BitReader, HuffTable};
use crate::marker::{Parsed, ZIGZAG};
use crate::sequential::decode_block;

/// An element of the dihedral group of the square: optionally transpose (swap x and y), then
/// mirror along x, then mirror along y. Applied to stored pixels, Exif orientation `o` is
/// [`Transform::from_exif`]`(o)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Transform {
    pub transpose: bool,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        transpose: false,
        flip_h: false,
        flip_v: false,
    };
    /// A quarter turn clockwise.
    pub const ROTATE_90: Transform = Transform {
        transpose: true,
        flip_h: true,
        flip_v: false,
    };
    pub const ROTATE_180: Transform = Transform {
        transpose: false,
        flip_h: true,
        flip_v: true,
    };
    /// A quarter turn anticlockwise.
    pub const ROTATE_270: Transform = Transform {
        transpose: true,
        flip_h: false,
        flip_v: true,
    };
    pub const FLIP_H: Transform = Transform {
        transpose: false,
        flip_h: true,
        flip_v: false,
    };
    pub const FLIP_V: Transform = Transform {
        transpose: false,
        flip_h: false,
        flip_v: true,
    };

    /// The transform that displays stored pixels with Exif orientation `o` (1..=8; anything
    /// else is the identity).
    pub fn from_exif(o: u8) -> Transform {
        let (transpose, flip_h, flip_v) = match o {
            2 => (false, true, false),
            3 => (false, true, true),
            4 => (false, false, true),
            5 => (true, false, false),
            6 => (true, true, false),
            7 => (true, true, true),
            8 => (true, false, true),
            _ => (false, false, false),
        };
        Transform {
            transpose,
            flip_h,
            flip_v,
        }
    }

    /// The Exif orientation value (1..=8) of this transform.
    pub fn to_exif(self) -> u8 {
        match (self.transpose, self.flip_h, self.flip_v) {
            (false, false, false) => 1,
            (false, true, false) => 2,
            (false, true, true) => 3,
            (false, false, true) => 4,
            (true, false, false) => 5,
            (true, true, false) => 6,
            (true, true, true) => 7,
            (true, false, true) => 8,
        }
    }

    /// As a signed permutation matrix acting on centred (x, y).
    fn matrix(self) -> [[i8; 2]; 2] {
        let mut m = if self.transpose {
            [[0, 1], [1, 0]]
        } else {
            [[1, 0], [0, 1]]
        };
        if self.flip_h {
            m[0] = [-m[0][0], -m[0][1]];
        }
        if self.flip_v {
            m[1] = [-m[1][0], -m[1][1]];
        }
        m
    }

    fn from_matrix(m: [[i8; 2]; 2]) -> Transform {
        if m[0][0] == 0 {
            // m = D * T: the flips are the signs of the off-diagonal entries.
            Transform {
                transpose: true,
                flip_h: m[0][1] < 0,
                flip_v: m[1][0] < 0,
            }
        } else {
            Transform {
                transpose: false,
                flip_h: m[0][0] < 0,
                flip_v: m[1][1] < 0,
            }
        }
    }

    /// `self` first, then `next`.
    pub fn then(self, next: Transform) -> Transform {
        let (a, b) = (next.matrix(), self.matrix());
        let mut m = [[0i8; 2]; 2];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = a[i][0] * b[0][j] + a[i][1] * b[1][j];
            }
        }
        Transform::from_matrix(m)
    }

    /// The transform that undoes this one.
    pub fn inverse(self) -> Transform {
        let m = self.matrix();
        Transform::from_matrix([[m[0][0], m[1][0]], [m[0][1], m[1][1]]])
    }

    /// Whether the input axes are mirrored: (x, y) of the stored image.
    fn input_flips(self) -> (bool, bool) {
        if self.transpose {
            (self.flip_v, self.flip_h)
        } else {
            (self.flip_h, self.flip_v)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransformOptions {
    /// Drop the partial iMCU on a mirrored edge instead of refusing.
    pub trim: bool,
}

/// Whether a lossless transform is perfect, from the headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    /// Every pixel is kept.
    Perfect,
    /// Only possible by trimming the stored image to `width` x `height` (before the transform).
    NeedsTrim { width: u32, height: u32 },
    /// Even trimming leaves nothing (the mirrored axis is shorter than one iMCU).
    Impossible,
}

/// Why a file cannot be transformed losslessly (beyond [`Alignment`]).
fn supported(p: &Parsed) -> Result<()> {
    let f = &p.frame;
    if p.hierarchical {
        return Err(JpegError::Unsupported(
            "hierarchical JPEG has no single set of DCT coefficients",
        ));
    }
    if f.lossless {
        return Err(JpegError::Unsupported(
            "lossless JPEG has no DCT coefficients",
        ));
    }
    if f.comps.len() > 4 {
        return Err(JpegError::Unsupported("component count"));
    }
    for c in &f.comps {
        if !f.max_h.is_multiple_of(c.h) || !f.max_v.is_multiple_of(c.v) {
            return Err(JpegError::Unsupported("fractional sampling ratio"));
        }
    }
    Ok(())
}

fn imcu(p: &Parsed) -> (u32, u32) {
    let f = &p.frame;
    (8 * f.max_h, 8 * f.max_v)
}

/// The edge situation of transform `t` on the JPEG in `data` (headers only).
pub fn transform_alignment(data: &[u8], t: Transform) -> Result<Alignment> {
    let p = crate::marker::parse(data, false)?;
    supported(&p)?;
    Ok(alignment(&p, t))
}

fn alignment(p: &Parsed, t: Transform) -> Alignment {
    let (fx, fy) = t.input_flips();
    let (iw, ih) = imcu(p);
    let (w, h) = (p.frame.width, p.frame.height);
    let tw = if fx { w - w % iw } else { w };
    let th = if fy { h - h % ih } else { h };
    if tw == w && th == h {
        Alignment::Perfect
    } else if tw == 0 || th == 0 {
        Alignment::Impossible
    } else {
        Alignment::NeedsTrim {
            width: tw,
            height: th,
        }
    }
}

/// Quantised coefficients of one component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentCoefficients {
    pub id: u8,
    pub h_samp: u8,
    pub v_samp: u8,
    /// Quantisation table, natural order.
    pub quant: [u16; 64],
    /// Blocks covering the component (no MCU padding).
    pub blocks_w: u32,
    pub blocks_h: u32,
    /// The MCU-padded block grid `coefs` covers.
    pub stride_blocks: u32,
    pub rows_blocks: u32,
    /// 64 coefficients per block, natural order, blocks row-major over the padded grid.
    pub coefs: Vec<i16>,
}

/// Every quantised DCT coefficient of the JPEG in `data`. Errors on truncated or corrupt
/// entropy-coded data (a transform must not write a damaged copy).
pub fn read_coefficients(data: &[u8]) -> Result<(u32, u32, Vec<ComponentCoefficients>)> {
    let p = crate::marker::parse(data, true)?;
    supported(&p)?;
    let comps = coefficients(&p, data)?;
    Ok((p.frame.width, p.frame.height, comps))
}

pub(crate) fn coefficients(p: &Parsed, data: &[u8]) -> Result<Vec<ComponentCoefficients>> {
    let f = &p.frame;
    let mut out: Vec<ComponentCoefficients> = Vec::with_capacity(f.comps.len());
    for (ci, c) in f.comps.iter().enumerate() {
        let n = c.bw_pad as usize * c.bh_pad as usize * 64;
        let mut coefs = Vec::new();
        coefs
            .try_reserve_exact(n)
            .map_err(|_| JpegError::TooLarge(n as u64 * 2))?;
        coefs.resize(n, 0);
        out.push(ComponentCoefficients {
            id: c.id,
            h_samp: c.h as u8,
            v_samp: c.v as u8,
            quant: p.quant[ci],
            blocks_w: c.bw,
            blocks_h: c.bh,
            stride_blocks: c.bw_pad,
            rows_blocks: c.bh_pad,
            coefs,
        });
    }
    if f.arithmetic {
        let (stores, truncated) = crate::arith::coefficients(p, data)?;
        if truncated {
            return Err(JpegError::Truncated("entropy-coded data"));
        }
        for (cc, (dc, ac)) in out.iter_mut().zip(stores) {
            for (b, blk) in cc.coefs.chunks_exact_mut(64).enumerate() {
                blk.copy_from_slice(&ac[b * 64..b * 64 + 64]);
                blk[0] = dc[b];
            }
        }
        return Ok(out);
    }
    for s in &p.scans {
        huffman_scan(p, s, data, &mut out)?;
    }
    Ok(out)
}

/// Decode one Huffman scan (sequential or any progressive kind) into `out`.
fn huffman_scan(
    p: &Parsed,
    s: &crate::marker::Scan,
    data: &[u8],
    out: &mut [ComponentCoefficients],
) -> Result<()> {
    let f = &p.frame;
    let end = s.data_end.min(data.len());
    let mut br = BitReader {
        data: &data[..end],
        pos: s.data_start,
        buf: 0,
        bits: 0,
        marker_hit: false,
        zero_fill: 0,
    };
    let mut dc_pred = [0i32; 4];
    let mut eobrun = 0u32;
    let ri = s.restart_interval;
    let mut restart_left = ri;
    let interleaved = s.comps.len() > 1;
    let (units_x, units_y) = if interleaved {
        (f.mcus_x, f.mcus_y)
    } else {
        let c = &f.comps[s.comps[0].ci];
        (c.bw, c.bh)
    };
    let missing = || JpegError::Malformed("missing Huffman table");
    let tables: Vec<(Option<&HuffTable>, Option<&HuffTable>)> = s
        .comps
        .iter()
        .map(|sc| (s.dc[sc.td].as_deref(), s.ac[sc.ta].as_deref()))
        .collect();
    let (ss, se, ah, al) = (s.ss as usize, (s.se as usize).min(63), s.ah, s.al);
    // Differential frames (hierarchical mode) code DC without prediction (T.81 J.2.3.1).
    let predict = !f.differential;
    for uy in 0..units_y {
        for ux in 0..units_x {
            if ri != 0 {
                if restart_left == 0 {
                    br.restart();
                    dc_pred = [0; 4];
                    eobrun = 0;
                    restart_left = ri;
                }
                restart_left -= 1;
            }
            for (slot, sc) in s.comps.iter().enumerate() {
                let fc = &f.comps[sc.ci];
                let (nh, nv) = if interleaved { (fc.h, fc.v) } else { (1, 1) };
                let (dct, act) = tables[slot];
                for bv in 0..nv {
                    for bh in 0..nh {
                        let (bx, by) = if interleaved {
                            (ux * fc.h + bh, uy * fc.v + bv)
                        } else {
                            (ux, uy)
                        };
                        let cc = &mut out[sc.ci];
                        let i = (by as usize * cc.stride_blocks as usize + bx as usize) * 64;
                        let blk: &mut [i16; 64] = (&mut cc.coefs[i..i + 64]).try_into().unwrap();
                        if !predict {
                            dc_pred[slot] = 0;
                        }
                        if !f.progressive {
                            let mut scratch = [0i16; 64];
                            decode_block(
                                &mut br,
                                dct.ok_or_else(missing)?,
                                act.ok_or_else(missing)?,
                                &mut dc_pred[slot],
                                &mut scratch,
                            );
                            *blk = scratch;
                        } else if ss == 0 {
                            if ah == 0 {
                                br.ensure(32);
                                dc_pred[slot] = dc_pred[slot]
                                    .wrapping_add(br.decode_dc_diff(dct.ok_or_else(missing)?));
                                blk[0] = (dc_pred[slot].wrapping_shl(u32::from(al))) as i16;
                            } else {
                                br.ensure(1);
                                if br.get(1) != 0 {
                                    blk[0] |= (1i32 << al) as i16;
                                }
                            }
                        } else if ah == 0 {
                            ac_first(
                                &mut br,
                                act.ok_or_else(missing)?,
                                blk,
                                ss,
                                se,
                                al,
                                &mut eobrun,
                            );
                        } else {
                            ac_refine(
                                &mut br,
                                act.ok_or_else(missing)?,
                                blk,
                                ss,
                                se,
                                al,
                                &mut eobrun,
                            );
                        }
                    }
                }
            }
        }
        if br.overrun() {
            return Err(JpegError::Truncated("entropy-coded data"));
        }
    }
    Ok(())
}

fn ac_first(
    br: &mut BitReader,
    t: &HuffTable,
    blk: &mut [i16; 64],
    ss: usize,
    se: usize,
    al: u8,
    eobrun: &mut u32,
) {
    if *eobrun > 0 {
        *eobrun -= 1;
        return;
    }
    let mut k = ss;
    while k <= se {
        br.ensure(32);
        let sym = br.decode_symbol(t);
        let (r, s) = ((sym >> 4) as usize, sym & 15);
        if s != 0 {
            k += r;
            br.ensure(s);
            let v = br.get_extend(s);
            blk[ZIGZAG[k] as usize] = v.wrapping_shl(u32::from(al)) as i16;
            k += 1;
        } else if r == 15 {
            k += 16;
        } else {
            let mut run = 1u32 << r;
            if r > 0 {
                br.ensure(r as u32);
                run += br.get(r as u32);
            }
            *eobrun = run - 1;
            break;
        }
    }
}

fn ac_refine(
    br: &mut BitReader,
    t: &HuffTable,
    blk: &mut [i16; 64],
    ss: usize,
    se: usize,
    al: u8,
    eobrun: &mut u32,
) {
    let p1 = (1i32 << al) as i16;
    let m1 = (-1i32 << al) as i16;
    let mut k = ss;
    // Correction bits for an already nonzero coefficient.
    let correct = |br: &mut BitReader, c: &mut i16| {
        br.ensure(1);
        if br.get(1) != 0 && (*c & p1) == 0 {
            *c = c.wrapping_add(if *c >= 0 { p1 } else { m1 });
        }
    };
    if *eobrun == 0 {
        while k <= se {
            br.ensure(32);
            let sym = br.decode_symbol(t);
            let (mut r, s) = ((sym >> 4) as i32, sym & 15);
            let mut val = 0i16;
            if s != 0 {
                br.ensure(1);
                val = if br.get(1) != 0 { p1 } else { m1 };
            } else if r != 15 {
                let mut run = 1u32 << r;
                if r > 0 {
                    br.ensure(r as u32);
                    run += br.get(r as u32);
                }
                *eobrun = run;
                break;
            }
            while k <= se {
                let z = ZIGZAG[k] as usize;
                if blk[z] != 0 {
                    correct(br, &mut blk[z]);
                } else {
                    if r == 0 {
                        break;
                    }
                    r -= 1;
                }
                k += 1;
            }
            if val != 0 && k <= se {
                blk[ZIGZAG[k] as usize] = val;
            }
            k += 1;
        }
    }
    if *eobrun > 0 {
        while k <= se {
            let z = ZIGZAG[k] as usize;
            if blk[z] != 0 {
                correct(br, &mut blk[z]);
            }
            k += 1;
        }
        *eobrun -= 1;
    }
}

/// Transform the JPEG in `data` losslessly; see the module docs.
pub fn transform(data: &[u8], t: Transform, opts: TransformOptions) -> Result<Vec<u8>> {
    let p = crate::marker::parse(data, true)?;
    supported(&p)?;
    let (tw, th) = match alignment(&p, t) {
        Alignment::Perfect => (p.frame.width, p.frame.height),
        Alignment::NeedsTrim { width, height } if opts.trim => (width, height),
        Alignment::NeedsTrim { .. } => {
            return Err(JpegError::Unsupported(
                "a mirrored edge is not a whole number of MCUs",
            ));
        }
        Alignment::Impossible => {
            return Err(JpegError::Unsupported("image smaller than one MCU"));
        }
    };
    let src = coefficients(&p, data)?;
    let f = &p.frame;
    let (ow, oh) = if t.transpose { (th, tw) } else { (tw, th) };

    // Output geometry: sampling factors swap with a transpose; a single component is 1x1.
    let n = src.len();
    let samp: Vec<(u32, u32)> = src
        .iter()
        .map(|c| {
            let (h, v) = (u32::from(c.h_samp), u32::from(c.v_samp));
            if n == 1 {
                (1, 1)
            } else if t.transpose {
                (v, h)
            } else {
                (h, v)
            }
        })
        .collect();
    let omax_h = samp.iter().map(|s| s.0).max().unwrap_or(1);
    let omax_v = samp.iter().map(|s| s.1).max().unwrap_or(1);
    let mcus_x = ow.div_ceil(8 * omax_h);
    let mcus_y = oh.div_ceil(8 * omax_v);

    let mut comps: Vec<OutComp> = Vec::with_capacity(n);
    for (c, &(h, v)) in src.iter().zip(&samp) {
        let cw = (u64::from(ow) * u64::from(h)).div_ceil(u64::from(omax_h)) as u32;
        let ch = (u64::from(oh) * u64::from(v)).div_ceil(u64::from(omax_v)) as u32;
        let (bw, bh) = (cw.div_ceil(8), ch.div_ceil(8));
        let (bw_pad, bh_pad) = if n == 1 {
            (bw, bh)
        } else {
            (mcus_x * h, mcus_y * v)
        };
        // Blocks spanning the mirrored output axes (exact: those axes are iMCU-aligned).
        let nbx = (u64::from(ow) * u64::from(h) / u64::from(omax_h) / 8) as u32;
        let nby = (u64::from(oh) * u64::from(v) / u64::from(omax_v) / 8) as u32;
        let len = bw_pad as usize * bh_pad as usize * 64;
        let mut coefs = Vec::new();
        coefs
            .try_reserve_exact(len)
            .map_err(|_| JpegError::TooLarge(len as u64 * 2))?;
        coefs.resize(len, 0i16);
        for oy in 0..bh_pad {
            let py = if t.flip_v {
                match nby.checked_sub(oy + 1) {
                    Some(v) => v,
                    None => continue,
                }
            } else {
                oy
            };
            for ox in 0..bw_pad {
                let px = if t.flip_h {
                    match nbx.checked_sub(ox + 1) {
                        Some(v) => v,
                        None => continue,
                    }
                } else {
                    ox
                };
                let (sx, sy) = if t.transpose { (py, px) } else { (px, py) };
                if sx >= c.stride_blocks || sy >= c.rows_blocks {
                    continue;
                }
                let si = (sy as usize * c.stride_blocks as usize + sx as usize) * 64;
                let di = (oy as usize * bw_pad as usize + ox as usize) * 64;
                let s = &c.coefs[si..si + 64];
                let d = &mut coefs[di..di + 64];
                for v in 0..8 {
                    for u in 0..8 {
                        let val = if t.transpose {
                            s[u * 8 + v]
                        } else {
                            s[v * 8 + u]
                        };
                        let neg = (t.flip_h && u & 1 == 1) != (t.flip_v && v & 1 == 1);
                        d[v * 8 + u] = if neg { val.wrapping_neg() } else { val };
                    }
                }
            }
        }
        let quant = if t.transpose {
            std::array::from_fn(|i| c.quant[(i % 8) * 8 + i / 8])
        } else {
            c.quant
        };
        comps.push(OutComp {
            id: c.id,
            h,
            v,
            quant,
            bw,
            bh,
            bw_pad,
            coefs,
        });
    }

    let restart = p.scans.iter().any(|s| s.restart_interval > 0);
    let mut out = Vec::with_capacity(data.len() + 1024);
    out.extend_from_slice(&[0xFF, 0xD8]);
    copy_app_segments(data, p.info.header_len, t.transpose, &mut out);
    write_frame(
        &mut out,
        f.precision,
        ow,
        oh,
        &comps,
        mcus_x,
        mcus_y,
        restart,
    )?;
    out.extend_from_slice(&[0xFF, 0xD9]);
    Ok(out)
}

struct OutComp {
    id: u8,
    h: u32,
    v: u32,
    quant: [u16; 64],
    bw: u32,
    bh: u32,
    bw_pad: u32,
    coefs: Vec<i16>,
}

/// Copy APPn and COM segments before the first scan; swap the JFIF density on a transpose.
fn copy_app_segments(data: &[u8], header_len: usize, transpose: bool, out: &mut Vec<u8>) {
    let mut pos = 2usize;
    while pos + 4 <= header_len.min(data.len()) {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let m = data[pos + 1];
        if m == 0xFF {
            pos += 1;
            continue;
        }
        if matches!(m, 0xD8 | 0x01 | 0xD0..=0xD7) {
            pos += 2;
            continue;
        }
        let len = usize::from(u16::from_be_bytes([data[pos + 2], data[pos + 3]]));
        let end = (pos + 2 + len).min(data.len());
        if (0xE0..=0xEF).contains(&m) || m == 0xFE {
            let start = out.len();
            out.extend_from_slice(&data[pos..end]);
            let seg = &mut out[start + 4..];
            if transpose && m == 0xE0 && seg.len() >= 12 && seg.starts_with(b"JFIF\0") {
                // Payload: "JFIF\0", version (2), units, X density (2), Y density (2).
                seg.swap(8, 10);
                seg.swap(9, 11);
            }
        }
        pos = end;
    }
}

#[allow(clippy::too_many_arguments)]
fn write_frame(
    out: &mut Vec<u8>,
    precision: u8,
    w: u32,
    h: u32,
    comps: &[OutComp],
    mcus_x: u32,
    mcus_y: u32,
    restart: bool,
) -> Result<()> {
    if w > 65535 || h > 65535 {
        return Err(JpegError::Unsupported("size"));
    }
    // Quantisation tables, deduplicated.
    let mut tables: Vec<[u16; 64]> = Vec::new();
    let mut tq = Vec::with_capacity(comps.len());
    for c in comps {
        let i = match tables.iter().position(|t| *t == c.quant) {
            Some(i) => i,
            None => {
                tables.push(c.quant);
                tables.len() - 1
            }
        };
        if i > 3 {
            return Err(JpegError::Unsupported("more than four quantisation tables"));
        }
        tq.push(i as u8);
    }
    let wide_q = tables.iter().any(|t| t.iter().any(|&q| q > 255));
    let baseline = precision == 8 && !wide_q;
    let mut dqt = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        dqt.push(((wide_q as u8) << 4) | i as u8);
        for k in 0..64 {
            let q = t[ZIGZAG[k] as usize];
            if wide_q {
                dqt.extend_from_slice(&q.to_be_bytes());
            } else {
                dqt.push(q as u8);
            }
        }
    }
    segment(out, 0xDB, &dqt);
    let mut sof = vec![precision];
    sof.extend_from_slice(&(h as u16).to_be_bytes());
    sof.extend_from_slice(&(w as u16).to_be_bytes());
    sof.push(comps.len() as u8);
    for (c, &q) in comps.iter().zip(&tq) {
        sof.extend_from_slice(&[c.id, ((c.h as u8) << 4) | c.v as u8, q]);
    }
    segment(out, if baseline { 0xC0 } else { 0xC1 }, &sof);

    // Scan layout: one interleaved scan when an MCU holds at most ten blocks.
    let blocks_per_mcu: u32 = comps.iter().map(|c| c.h * c.v).sum();
    let scans: Vec<Vec<usize>> = if comps.len() > 1 && blocks_per_mcu <= 10 {
        vec![(0..comps.len()).collect()]
    } else {
        (0..comps.len()).map(|i| vec![i]).collect()
    };
    // Table set per component: the first component one, the others the other.
    let set = |ci: usize| usize::from(ci > 0);
    // Restart interval per scan: one per row of units when the input had restarts.
    let ris: Vec<u32> = scans
        .iter()
        .map(|scan| {
            let units_x = if scan.len() == 1 {
                comps[scan[0]].bw
            } else {
                mcus_x
            };
            if restart { units_x.min(65535) } else { 0 }
        })
        .collect();
    let mut dc_count = [[0u32; 257]; 2];
    let mut ac_count = [[0u32; 257]; 2];
    for (scan, &ri) in scans.iter().zip(&ris) {
        visit_scan_units(
            comps,
            scan,
            mcus_x,
            mcus_y,
            |_, ci, blk, pred| {
                let s = set(ci);
                dc_count[s][magnitude(i32::from(blk[0]) - pred).0 as usize] += 1;
                ac_symbols(blk, |sym| ac_count[s][sym as usize] += 1);
            },
            ri,
        );
    }
    let sets = if comps.len() > 1 { 2 } else { 1 };
    let mut dc_h = Vec::new();
    let mut ac_h = Vec::new();
    let mut dht = Vec::new();
    for s in 0..sets {
        for (class, counts) in [(0u8, &mut dc_count[s]), (1u8, &mut ac_count[s])] {
            if counts[..256].iter().all(|&c| c == 0) {
                counts[0] = 1;
            }
            let (bits, vals) = optimal_table(counts);
            dht.push((class << 4) | s as u8);
            dht.extend_from_slice(&bits);
            dht.extend_from_slice(&vals);
            let huff = Huff::new(&bits, &vals);
            if class == 0 {
                dc_h.push(huff);
            } else {
                ac_h.push(huff);
            }
        }
    }
    segment(out, 0xC4, &dht);
    for (scan, &ri) in scans.iter().zip(&ris) {
        segment(out, 0xDD, &(ri as u16).to_be_bytes());
        let mut sos = vec![scan.len() as u8];
        for &ci in scan {
            let s = set(ci) as u8;
            sos.extend_from_slice(&[comps[ci].id, (s << 4) | s]);
        }
        sos.extend_from_slice(&[0, 63, 0]);
        segment(out, 0xDA, &sos);
        let mut bw = BitWriter {
            out: std::mem::take(out),
            acc: 0,
            n: 0,
        };
        let mut unit = 0u32;
        let mut rst = 0u8;
        let mut emit = |start_of_unit: bool, ci: usize, blk: &[i16; 64], pred: i32| {
            if start_of_unit {
                if ri > 0 && unit > 0 && unit.is_multiple_of(ri) {
                    bw.flush();
                    bw.out.extend_from_slice(&[0xFF, 0xD0 + rst]);
                    rst = (rst + 1) & 7;
                }
                unit += 1;
            }
            let s = set(ci);
            let (nb, bits) = magnitude(i32::from(blk[0]) - pred);
            dc_h[s].put(&mut bw, nb as u8);
            bw.put(bits, nb);
            encode_ac(&mut bw, blk, &ac_h[s]);
        };
        visit_scan_units(comps, scan, mcus_x, mcus_y, &mut emit, ri);
        bw.flush();
        *out = bw.out;
    }
    Ok(())
}

/// Zigzag-ordered copy of a natural-order block.
fn zigzag(blk: &[i16]) -> [i16; 64] {
    std::array::from_fn(|k| blk[ZIGZAG[k] as usize])
}

fn ac_symbols(b: &[i16; 64], mut f: impl FnMut(u8)) {
    let mut run = 0u32;
    for &c in &b[1..] {
        if c == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            f(0xF0);
            run -= 16;
        }
        f((run << 4 | magnitude(i32::from(c)).0) as u8);
        run = 0;
    }
    if run > 0 {
        f(0x00);
    }
}

fn encode_ac(bw: &mut BitWriter, b: &[i16; 64], ac: &Huff) {
    let mut run = 0u32;
    for &c in &b[1..] {
        if c == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            ac.put(bw, 0xF0);
            run -= 16;
        }
        let (n, bits) = magnitude(i32::from(c));
        ac.put(bw, (run << 4 | n) as u8);
        bw.put(bits, n);
        run = 0;
    }
    if run > 0 {
        ac.put(bw, 0x00);
    }
}

/// Visit the blocks of a scan in coding order: (first block of a unit, component, zigzag block,
/// DC predictor). Predictors reset every `ri` units when `ri > 0`.
fn visit_scan_units(
    comps: &[OutComp],
    scan: &[usize],
    mcus_x: u32,
    mcus_y: u32,
    mut f: impl FnMut(bool, usize, &[i16; 64], i32),
    ri: u32,
) {
    let mut pred = [0i32; 4];
    let interleaved = scan.len() > 1;
    let (ux_n, uy_n) = if interleaved {
        (mcus_x, mcus_y)
    } else {
        (comps[scan[0]].bw, comps[scan[0]].bh)
    };
    let mut unit = 0u32;
    for uy in 0..uy_n {
        for ux in 0..ux_n {
            if ri > 0 && unit.is_multiple_of(ri) {
                pred = [0; 4];
            }
            unit += 1;
            let mut first = true;
            for (slot, &ci) in scan.iter().enumerate() {
                let c = &comps[ci];
                let (nh, nv) = if interleaved { (c.h, c.v) } else { (1, 1) };
                for bv in 0..nv {
                    for bh in 0..nh {
                        let (bx, by) = if interleaved {
                            (ux * c.h + bh, uy * c.v + bv)
                        } else {
                            (ux, uy)
                        };
                        let i = (by as usize * c.bw_pad as usize + bx as usize) * 64;
                        let blk = zigzag(&c.coefs[i..i + 64]);
                        f(first, ci, &blk, pred[slot]);
                        first = false;
                        pred[slot] = i32::from(blk[0]);
                    }
                }
            }
        }
    }
}
