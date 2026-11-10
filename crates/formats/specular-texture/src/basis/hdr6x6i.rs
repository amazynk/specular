//! UASTC HDR 6×6 intermediate ("6×6i", Basis GPU Photo 6×6): a stream of variable-length block
//! commands (run, solid, reuse, block) transcoded to standard ASTC HDR 6×6 blocks, written from
//! Binomial's published "UASTC HDR 6x6i (Intermediate) Specification v1.2" (basis_universal
//! wiki). Both stream versions are read: 0xABCD (v1.6) and 0xABCE (v2.0, which changed how 2×2
//! weight grids are upsampled).
//!
//! Each coded ("logical") block is kept next to its output block: later blocks reuse a
//! configuration from up to four rows back, or endpoints from the left or upper block, in the
//! coded ISE ranges, which can differ from the ranges the output block needs.

// Index loops follow the specification's pseudo-code.
#![allow(clippy::needless_range_loop)]

use super::astc_pack::{LogBlock, R256, cem_values, endpoint_quant, levels, pack, range_of};
use super::hdr6x6i_tables::{BLOCK_MODES, PART2_SEEDS, PART3_SEEDS};
use crate::astc::RANGES;
use crate::{Error, Result};

const V16: u32 = 0xabcd;
const V20: u32 = 0xabce;
const END_MARKER: u32 = 0xa742;
const HALF_ONE: u16 = 0x3c00;

/// Reuse "delta" codes: (dx, dy) of the block whose configuration is reused.
const REUSE: [(i8, i8); 32] = [
    (-1, 0),
    (-2, 0),
    (-3, 0),
    (-4, 0),
    (3, -1),
    (2, -1),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-2, -1),
    (-3, -1),
    (-4, -1),
    (3, -2),
    (2, -2),
    (1, -2),
    (0, -2),
    (-1, -2),
    (-2, -2),
    (-3, -2),
    (-4, -2),
    (3, -3),
    (2, -3),
    (1, -3),
    (0, -3),
    (-1, -3),
    (-2, -3),
    (-3, -3),
    (-4, -3),
    (3, -4),
    (2, -4),
    (1, -4),
    (0, -4),
];

/// Bits LSB first, as Deflate stores them.
pub(crate) struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(d: &'a [u8]) -> Self {
        Bits { d, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        (self.d.len() * 8).saturating_sub(self.pos)
    }

    pub(crate) fn get(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        if self.remaining() < n as usize {
            return Err(Error::Truncated("UASTC HDR 6×6 intermediate stream"));
        }
        let mut v = 0u32;
        for k in 0..n {
            let p = self.pos + k as usize;
            v |= u32::from((self.d[p >> 3] >> (p & 7)) & 1) << k;
        }
        self.pos += n as usize;
        Ok(v)
    }

    /// Truncated binary code of a value in 0..n (n ≥ 2).
    pub(crate) fn truncated_binary(&mut self, n: u32) -> Result<u32> {
        let k = 31 - n.leading_zeros();
        let u = (1 << (k + 1)) - n;
        let mut r = self.get(k)?;
        if r >= u {
            r = ((r << 1) | self.get(1)?) - u;
        }
        Ok(r)
    }

    /// Run length code: 6-bit packets of 5 value bits and a "more" flag, at most 30 bits.
    fn vlc5(&mut self) -> Result<u32> {
        let (mut v, mut ofs) = (0u32, 0u32);
        loop {
            let s = self.get(6)?;
            v |= (s & 31) << ofs;
            ofs += 5;
            if s & 32 == 0 {
                return Ok(v);
            }
            if ofs >= 30 {
                return Err(Error::Malformed("UASTC HDR 6×6 run length"));
            }
        }
    }
}

/// The simplified ISE of UASTC (all trit/quint packets first, then the plain bits per value).
pub(crate) fn decode_values(b: &mut Bits, out: &mut [u8], r: u8) -> Result<()> {
    let (_, trits, quints, nb) = RANGES[usize::from(r)];
    let n = out.len() as u32;
    let (groups, bundle, mul) = if trits {
        (n.div_ceil(5), 5, 3)
    } else if quints {
        (n.div_ceil(3), 3, 5)
    } else {
        (0, 0, 0)
    };
    let mut tq = [0u32; 32];
    for i in 0..groups {
        let mut bits = if trits { 8 } else { 7 };
        if i == groups - 1 {
            let left = n - (groups - 1) * bundle;
            bits = match (trits, left) {
                (true, 1) => 2,
                (true, 2) => 4,
                (true, 3) => 5,
                (true, 4) => 7,
                (false, 1) => 3,
                (false, 2) => 5,
                _ => bits,
            };
        }
        tq[i as usize] = b.get(bits)?;
    }
    let (mut acc, mut left, mut next) = (0u32, 0u32, 0usize);
    for v in out.iter_mut() {
        let mut x = b.get(nb)?;
        if groups > 0 {
            if left == 0 {
                acc = tq[next];
                next += 1;
                left = bundle;
            }
            x |= (acc % mul) << nb;
            acc /= mul;
            left -= 1;
        }
        if x as usize >= levels(r) {
            return Err(Error::Malformed("UASTC HDR 6×6 ISE value"));
        }
        *v = x as u8;
    }
    Ok(())
}

struct Config {
    dual: bool,
    cem: u8,
    parts: u8,
    gw: u8,
    gh: u8,
    er: u8,
    wr: u8,
    ter: u8,
    twr: u8,
    ccs: u8,
}

fn config(i: u32) -> Config {
    let (dual, cem, parts, gw, gh, e, w, te, tw, ccs) = BLOCK_MODES[i as usize];
    Config {
        dual,
        cem,
        parts,
        gw,
        gh,
        er: range_of(e),
        wr: range_of(w),
        ter: range_of(te),
        twr: range_of(tw),
        ccs,
    }
}

/// `g_quantize_tables_preserve2/3`: the nearest ISE value of range `r` whose unquantised value
/// keeps the top `keep` bits of `v` (the first in ISE order on a tie).
pub(crate) fn preserve_tables() -> &'static [[[u8; 256]; 2]; 21] {
    static T: std::sync::OnceLock<Box<[[[u8; 256]; 2]; 21]>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([[[0u8; 256]; 2]; 21]);
        for r in range_of(6)..=range_of(192) {
            let q = &endpoint_quant()[usize::from(r)].ise_to_val;
            for (k, mask) in [(0usize, 0xc0u8), (1, 0xe0)] {
                for v in 0..256usize {
                    let best = (0..q.len())
                        .filter(|&i| q[i] & mask == v as u8 & mask)
                        .min_by_key(|&i| (i32::from(q[i]) - v as i32).unsigned_abs());
                    t[usize::from(r)][k][v] = best.unwrap_or(0) as u8;
                }
            }
        }
        t
    })
}

/// `requantize_ise_endpoints`: endpoint ISE values from range `from` to range `to`, keeping
/// the high bits that carry the HDR mode meaning.
fn requantize_endpoints(cem: u8, from: u8, src: &[u8], to: u8, dst: &mut [u8]) -> Result<()> {
    let n = cem_values(cem);
    if from == to {
        dst[..n].copy_from_slice(&src[..n]);
        return Ok(());
    }
    if to < range_of(12) {
        return Err(Error::Malformed("UASTC HDR 6×6 endpoint range"));
    }
    let mut v = [0u8; 6];
    for i in 0..n {
        v[i] = if from == R256 {
            src[i]
        } else {
            endpoint_quant()[usize::from(from)].ise_to_val[usize::from(src[i])]
        };
    }
    if to == R256 {
        dst[..n].copy_from_slice(&v[..n]);
        return Ok(());
    }
    let q = &endpoint_quant()[usize::from(to)].val_to_ise;
    let p = &preserve_tables()[usize::from(to)];
    let (p2, p3) = (&p[0], &p[1]);
    let at = |x: u8| usize::from(x);
    match cem {
        11 => {
            if v[4] >> 7 == 1 && v[5] >> 7 == 1 {
                for i in 0..6 {
                    dst[i] = q[at(v[i])];
                }
            } else {
                dst[0] = q[at(v[0])];
                for i in 1..4 {
                    dst[i] = p2[at(v[i])];
                }
                for i in 4..6 {
                    dst[i] = p3[at(v[i])];
                }
            }
        }
        7 => {
            dst[0] = p2[at(v[0])];
            for i in 1..4 {
                dst[i] = p3[at(v[i])];
            }
        }
        _ => return Err(Error::Malformed("UASTC HDR 6×6 endpoint mode")),
    }
    Ok(())
}

/// `requantize_astc_weights`.
fn requantize_weights(src: &[u8], from: u8, dst: &mut [u8], to: u8) {
    let wq = super::astc_pack::weight_quant();
    for (d, &s) in dst.iter_mut().zip(src) {
        *d = if from == to {
            s
        } else {
            wq[usize::from(to)].val_to_ise
                [usize::from(wq[usize::from(from)].ise_to_val[usize::from(s)])]
        };
    }
}

/// ASTC's bilinear weight-grid upsampling weights for a `bw`×`bh` block from a `gw`×`gh` grid:
/// per texel the top-left source sample and the four weights (×16).
fn upsample_weights(bw: u32, bh: u32, gw: u32, gh: u32, out: &mut [(u8, u8, [u8; 4])]) {
    let sx = (1024 + bw / 2) / (bw - 1);
    let sy = (1024 + bh / 2) / (bh - 1);
    for ty in 0..bh {
        for tx in 0..bw {
            let gx = (sx * tx * (gw - 1) + 32) >> 6;
            let gy = (sy * ty * (gh - 1) + 32) >> 6;
            let (fx, fy) = (gx & 15, gy & 15);
            let w11 = (fx * fy + 8) >> 4;
            let w10 = fy - w11;
            let w01 = fx - w11;
            let w00 = 16 + w11 - fx - fy;
            out[(tx + ty * bw) as usize] = (
                (gx >> 4) as u8,
                (gy >> 4) as u8,
                [w00 as u8, w01 as u8, w10 as u8, w11 as u8],
            );
        }
    }
}

/// `copy_weight_grid`: the output weights, a single-plane 2×2 grid upsampled to 4×4.
fn copy_weight_grid(dual: bool, gw: u8, gh: u8, w: &[u8], out: &mut LogBlock, version: u32) {
    if !dual && gw == 2 && gh == 2 {
        out.grid_w = 4;
        out.grid_h = 4;
        let q = &super::astc_pack::weight_quant()[usize::from(out.wrange)];
        let mut s = [(0u8, 0u8, [0u8; 4]); 16];
        upsample_weights(4, 4, 2, 2, &mut s);
        for y in 0..4usize {
            for x in 0..4usize {
                let (sx, sy, ws) = s[x + y * 4];
                let mut total = 8u32;
                for yo in 0..2usize {
                    for xo in 0..2usize {
                        let k = ws[yo * 2 + xo];
                        if k == 0 {
                            continue;
                        }
                        let i = if version == V16 {
                            // v1.6 indexed with a bounds test's truth value (sample 0 or 1).
                            usize::from((x + xo) + (y + yo) * 2 < 4)
                        } else {
                            (usize::from(sx) + xo) + (usize::from(sy) + yo) * 2
                        };
                        total += u32::from(q.ise_to_val[usize::from(w[i])]) * u32::from(k);
                    }
                }
                out.weights[x + y * 4] = q.val_to_ise[(total >> 4).min(64) as usize];
            }
        }
    } else {
        let n = usize::from(gw) * usize::from(gh) * if dual { 2 } else { 1 };
        out.grid_w = gw;
        out.grid_h = gh;
        out.weights[..n].copy_from_slice(&w[..n]);
    }
}

/// The output block for coded block `log` of configuration `c`, its endpoints already in the
/// output range.
fn output_block(c: &Config, log: &LogBlock, endpoints: &[u8], version: u32) -> LogBlock {
    let mut out = LogBlock {
        parts: log.parts,
        seed: log.seed,
        cem: c.cem,
        erange: c.ter,
        wrange: c.twr,
        dual: c.dual,
        ccs: c.ccs,
        ..LogBlock::default()
    };
    let n = cem_values(c.cem) * usize::from(log.parts);
    out.endpoints[..n].copy_from_slice(&endpoints[..n]);
    let nw = log.weight_count();
    let mut tw = [0u8; 72];
    requantize_weights(&log.weights[..nw], c.wr, &mut tw[..nw], c.twr);
    copy_weight_grid(c.dual, c.gw, c.gh, &tw, &mut out, version);
    out
}

/// Transcode one image's stream to ASTC HDR 6×6 blocks (16 bytes each, row-major). The stream's
/// width and height must be `width`×`height`.
pub(crate) fn transcode(d: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut b = Bits::new(d);
    if d.len() <= 6 {
        return Err(Error::Truncated("UASTC HDR 6×6 intermediate header"));
    }
    let version = b.get(16)?;
    if version != V16 && version != V20 {
        return Err(Error::Unsupported(format!(
            "UASTC HDR 6×6 intermediate stream version {version:#06x}"
        )));
    }
    let (w, h) = (b.get(16)?, b.get(16)?);
    if (w, h) != (width, height) || w == 0 || h == 0 {
        return Err(Error::Malformed("UASTC HDR 6×6 stream size"));
    }
    let (bx, by) = (w.div_ceil(6) as usize, h.div_ceil(6) as usize);
    let total = bx * by;
    let mut out = vec![0u8; total * 16];
    // Logical and output blocks of the last five rows (only up to four rows back are read).
    let rows = 5usize;
    let mut log = vec![LogBlock::default(); bx * rows];
    let mut phys = vec![LogBlock::default(); bx * rows];
    let slot = |x: usize, y: usize| (y % rows) * bx + x;
    let mut cur = 0usize;
    let mut emit = |cur: &mut usize,
                    l: LogBlock,
                    p: LogBlock,
                    log: &mut [LogBlock],
                    phys: &mut [LogBlock]|
     -> Result<()> {
        let (x, y) = (*cur % bx, *cur / bx);
        let bytes = pack(&p).ok_or(Error::Malformed("UASTC HDR 6×6 block"))?;
        out[*cur * 16..][..16].copy_from_slice(&bytes);
        log[slot(x, y)] = l;
        phys[slot(x, y)] = p;
        *cur += 1;
        Ok(())
    };
    while cur < total {
        if b.remaining() < 1 {
            return Err(Error::Truncated("UASTC HDR 6×6 intermediate stream"));
        }
        let (x, y) = (cur % bx, cur / bx);
        if b.get(1)? == 1 {
            // BLOCK
            let c = config(b.truncated_binary(BLOCK_MODES.len() as u32)?);
            let em = b.truncated_binary(5)?;
            let nw = usize::from(c.gw) * usize::from(c.gh) * if c.dual { 2 } else { 1 };
            let mut l = LogBlock {
                parts: if em == 0 { c.parts } else { 1 },
                cem: c.cem,
                erange: c.er,
                wrange: c.wr,
                grid_w: c.gw,
                grid_h: c.gh,
                dual: c.dual,
                ccs: c.ccs,
                ..LogBlock::default()
            };
            let n = cem_values(c.cem);
            if em == 0 {
                l.seed = match c.parts {
                    2 => PART2_SEEDS[b.truncated_binary(PART2_SEEDS.len() as u32)? as usize],
                    3 => PART3_SEEDS[b.truncated_binary(PART3_SEEDS.len() as u32)? as usize],
                    _ => 0,
                };
                decode_values(&mut b, &mut l.endpoints[..n * usize::from(c.parts)], c.er)?;
            } else {
                if c.parts != 1 {
                    return Err(Error::Malformed(
                        "UASTC HDR 6×6 endpoint reuse with subsets",
                    ));
                }
                let (nx, ny) = match em {
                    1 | 3 => (x.checked_sub(1), Some(y)),
                    _ => (Some(x), y.checked_sub(1)),
                };
                let (Some(nx), Some(ny)) = (nx, ny) else {
                    return Err(Error::Malformed("UASTC HDR 6×6 endpoint neighbour"));
                };
                let nb = log[slot(nx, ny)];
                if nb.cem == 0 || nb.cem != c.cem {
                    return Err(Error::Malformed("UASTC HDR 6×6 endpoint neighbour"));
                }
                if em <= 2 {
                    l.erange = nb.erange;
                    l.endpoints[..n].copy_from_slice(&nb.endpoints[..n]);
                } else {
                    let mut e = [0u8; 18];
                    requantize_endpoints(c.cem, nb.erange, &nb.endpoints, c.er, &mut e)?;
                    let q = &endpoint_quant()[usize::from(c.er)];
                    let lv = levels(c.er) as i32;
                    for i in 0..n {
                        let r = i32::from(q.ise_to_rank[usize::from(e[i])]) + b.get(5)? as i32 - 16;
                        if !(0..lv).contains(&r) {
                            return Err(Error::Malformed("UASTC HDR 6×6 endpoint delta"));
                        }
                        e[i] = q.rank_to_ise[r as usize];
                    }
                    l.endpoints = e;
                }
            }
            decode_values(&mut b, &mut l.weights[..nw], c.wr)?;
            let mut e = [0u8; 18];
            for p in 0..usize::from(l.parts) {
                requantize_endpoints(
                    c.cem,
                    l.erange,
                    &l.endpoints[p * n..],
                    c.ter,
                    &mut e[p * n..],
                )?;
            }
            let p = output_block(&c, &l, &e, version);
            emit(&mut cur, l, p, &mut log, &mut phys)?;
        } else if b.get(1)? == 1 {
            // REUSE
            if cur == 0 {
                return Err(Error::Malformed("UASTC HDR 6×6 reuse at the first block"));
            }
            let (dx, dy) = REUSE[b.get(5)? as usize];
            let (px, py) = (x as isize + isize::from(dx), y as isize + isize::from(dy));
            if px < 0 || px >= bx as isize || py < 0 {
                return Err(Error::Malformed("UASTC HDR 6×6 reuse position"));
            }
            let (px, py) = (px as usize, py as usize);
            let (prev_l, prev_p) = (log[slot(px, py)], phys[slot(px, py)]);
            if prev_l.solid || prev_l.cem == 0 {
                return Err(Error::Malformed("UASTC HDR 6×6 reuse of a solid block"));
            }
            let mut l = prev_l;
            let nw = l.weight_count();
            decode_values(&mut b, &mut l.weights[..nw], l.wrange)?;
            let mut p = prev_p;
            let mut tw = [0u8; 72];
            requantize_weights(&l.weights[..nw], l.wrange, &mut tw[..nw], p.wrange);
            copy_weight_grid(l.dual, l.grid_w, l.grid_h, &tw, &mut p, version);
            emit(&mut cur, l, p, &mut log, &mut phys)?;
        } else if b.get(1)? == 1 {
            // SOLID
            let mut l = LogBlock {
                solid: true,
                solid_hdr: true,
                ..LogBlock::default()
            };
            for c in 0..3 {
                l.solid_colour[c] = b.get(15)? as u16;
            }
            l.solid_colour[3] = HALF_ONE;
            emit(&mut cur, l, l, &mut log, &mut phys)?;
        } else {
            // RUN
            if cur == 0 {
                return Err(Error::Malformed("UASTC HDR 6×6 run at the first block"));
            }
            let n = b.vlc5()? as usize + 1;
            if n > total - cur {
                return Err(Error::Malformed("UASTC HDR 6×6 run length"));
            }
            let (qx, qy) = if x > 0 { (x - 1, y) } else { (bx - 1, y - 1) };
            let (l, p) = (log[slot(qx, qy)], phys[slot(qx, qy)]);
            for _ in 0..n {
                emit(&mut cur, l, p, &mut log, &mut phys)?;
            }
        }
    }
    if b.get(16)? != END_MARKER {
        return Err(Error::Malformed("UASTC HDR 6×6 end marker"));
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Level 0, image 0 of a KTX2 or `.basis` file of a Basis Universal intermediate codec:
    /// (texture, the image's stream).
    pub(crate) fn first_image(d: &[u8]) -> (crate::Texture, Vec<u8>) {
        let t = if crate::basis_file::is_basis(d) {
            crate::basis_file::parse(d).unwrap()
        } else {
            crate::ktx2::parse(d).unwrap()
        };
        let l = &t.levels[0];
        let (o, n) = t.seek.as_ref().unwrap()[0];
        let s = d[l.range.clone()][o..o + n].to_vec();
        (t, s)
    }

    /// Blocks that differ, and texels whose half-float RGBA bits differ, between two block
    /// arrays of ASTC HDR `b`×`b` blocks.
    pub(crate) fn compare_hdr(ours: &[u8], want: &[u8], b: usize) -> (usize, usize) {
        assert_eq!(ours.len(), want.len());
        let (mut blocks, mut texels) = (0, 0);
        let (mut p, mut q) = (vec![0u16; b * b * 4], vec![0u16; b * b * 4]);
        for (x, y) in ours.chunks_exact(16).zip(want.chunks_exact(16)) {
            if x == y {
                continue;
            }
            blocks += 1;
            crate::astc::decode_hdr(x, (b, b, 1), 0, &mut p, b * 4);
            crate::astc::decode_hdr(y, (b, b, 1), 0, &mut q, b * 4);
            texels += p
                .chunks_exact(4)
                .zip(q.chunks_exact(4))
                .filter(|(a, c)| a != c)
                .count();
        }
        (blocks, texels)
    }

    /// basisu's 37×29 6×6i file (tests/data, written as in tests/hdr_basis.rs), in KTX2 and
    /// `.basis`: every block equals the opengl_basisu wheel's transcode to ASTC HDR 6×6
    /// (`tests/data/hdr6i.blocks`, from tools/corpus/texture/basisu2_ref.py).
    #[test]
    fn fixture_blocks_match_the_transcoder() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
        let want = std::fs::read(format!("{dir}hdr6i.blocks")).unwrap();
        for ext in ["ktx2", "basis"] {
            let d = std::fs::read(format!("{dir}hdr6i.{ext}")).unwrap();
            let (t, s) = first_image(&d);
            let ours = transcode(&s, t.width, t.height).unwrap();
            assert_eq!(compare_hdr(&ours, &want, 6), (0, 0), "{ext}");
            assert_eq!(ours, want, "{ext}");
        }
    }

    /// The fetched corpus (tools/corpus/fetch-textures.sh src_basisu2 oracle_basisu2): the
    /// 644×874 desk in KTX2 and `.basis`, against the wheel's blocks. Skips when absent.
    #[test]
    fn corpus_blocks_match_the_transcoder() {
        let Some(base) = std::env::var_os("SPECULAR_LARGE_CORPUS") else {
            return;
        };
        let dir = std::path::Path::new(&base).join("texture/basisu2");
        for (f, r) in [
            ("Desk_uastc_hdr6x6i.ktx2", "Desk_uastc_hdr6x6i.blocks"),
            ("desk_6x6i.basis", "desk_6x6i.basis.blocks"),
        ] {
            let (Ok(d), Ok(want)) = (std::fs::read(dir.join(f)), std::fs::read(dir.join(r))) else {
                continue;
            };
            let (t, s) = first_image(&d);
            let t0 = std::time::Instant::now();
            let ours = transcode(&s, t.width, t.height).unwrap();
            let el = t0.elapsed();
            let (b, x) = compare_hdr(&ours, &want, 6);
            eprintln!(
                "{f}: {} blocks, {b} differ, {x} texels differ, {el:?}",
                ours.len() / 16
            );
            assert_eq!((b, x), (0, 0), "{f}");
        }
    }
}
