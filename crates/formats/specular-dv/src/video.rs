//! Video segments: the compressed macro blocks of a DIF sequence, their three-pass bit
//! distribution (SMPTE 314M 5.6), dequantisation and placement in the frame (5.1).

use crate::bits::Bits;
use crate::tables::{idct, inverse_weights, scans, step};
use crate::vlc::{Sym, lookup};
use crate::{Chroma, Layout};

/// Byte offsets (in the 80-byte DIF block) and bit lengths of the six compressed-data areas.
pub(crate) const AREA_START: [usize; 6] = [4, 18, 32, 46, 60, 70];
pub(crate) const AREA_BITS: [usize; 6] = [112, 112, 112, 112, 80, 80];

/// One decoded 8×8 block and where its samples go.
pub(crate) struct Placed {
    pub plane: u8,
    /// The block's top-left sample in its plane.
    pub x: u16,
    pub y: u16,
    /// 4:1:1 rightmost chroma: a 4×16 area folded into the 8×8 block (figure 16).
    pub folded: bool,
    pub px: [u8; 64],
}

/// Where a macro block lies: its luma top-left and shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MbPos {
    pub x: u32,
    pub y: u32,
    /// 16×16 luma (4:2:0, and the rightmost 4:1:1 macro blocks) instead of 32×8 or 16×8.
    pub square: bool,
}

/// Figure 25: macro block k of a 9-by-3 super block (4:2:2, and 4:2:0 with 16-line rows).
fn pos_9x3(k: u32) -> (u32, u32) {
    let col = k / 3;
    let r = k % 3;
    (col, if col.is_multiple_of(2) { r } else { 2 - r })
}

/// The position of macro block k of super block (i, j).
pub(crate) fn mb_pos(chroma: Chroma, i: u32, j: u32, k: u32) -> MbPos {
    match chroma {
        Chroma::C422 => {
            let (c, r) = pos_9x3(k);
            MbPos {
                x: j * 144 + c * 16,
                y: i * 24 + r * 8,
                square: false,
            }
        }
        Chroma::C420 => {
            let (c, r) = pos_9x3(k);
            MbPos {
                x: j * 144 + c * 16,
                y: i * 48 + r * 16,
                square: true,
            }
        }
        Chroma::C411 => {
            // Figure 26: columns of six 32×8 macro blocks, snaking down and up; super blocks
            // 0 and 1 (and 2 and 3) share their middle column, the right one ends with three
            // 16×16 macro blocks.
            let y0 = i * 48;
            let snake = |col: u32, r: u32, down_first: bool| {
                let down = col.is_multiple_of(2) == down_first;
                if down { r } else { 5 - r }
            };
            match j {
                0 | 2 | 4 => {
                    let base = (j / 2) * 9;
                    if k < 24 {
                        let col = k / 6;
                        MbPos {
                            x: (base + col) * 32,
                            y: y0 + snake(col, k % 6, true) * 8,
                            square: false,
                        }
                    } else if j == 4 {
                        MbPos {
                            x: 704,
                            y: y0 + (k - 24) * 16,
                            square: true,
                        }
                    } else {
                        MbPos {
                            x: (base + 4) * 32,
                            y: y0 + (k - 24) * 8,
                            square: false,
                        }
                    }
                }
                _ => {
                    let base = (j / 2) * 9 + 4;
                    if k < 3 {
                        MbPos {
                            x: base * 32,
                            y: y0 + (3 + k) * 8,
                            square: false,
                        }
                    } else {
                        let kk = k - 3;
                        let col = kk / 6;
                        MbPos {
                            x: (base + 1 + col) * 32,
                            y: y0 + snake(col, kk % 6, false) * 8,
                            square: false,
                        }
                    }
                }
            }
        }
    }
}

/// The 8×8 blocks of a macro block in coding order: (plane, x, y, folded) in samples of
/// their plane. Planes: 0 Y, 1 Cb, 2 Cr; coding order is Y…, Cr, Cb.
pub(crate) fn block_places(chroma: Chroma, m: MbPos) -> ([(u8, u32, u32, bool); 6], usize) {
    let (x, y) = (m.x, m.y);
    match chroma {
        Chroma::C422 => (
            [
                (0, x, y, false),
                (0, x + 8, y, false),
                (2, x / 2, y, false),
                (1, x / 2, y, false),
                (0, 0, 0, false),
                (0, 0, 0, false),
            ],
            4,
        ),
        Chroma::C420 => (
            [
                (0, x, y, false),
                (0, x + 8, y, false),
                (0, x, y + 8, false),
                (0, x + 8, y + 8, false),
                (2, x / 2, y / 2, false),
                (1, x / 2, y / 2, false),
            ],
            6,
        ),
        Chroma::C411 if m.square => (
            [
                (0, x, y, false),
                (0, x + 8, y, false),
                (0, x, y + 8, false),
                (0, x + 8, y + 8, false),
                (2, x / 4, y, true),
                (1, x / 4, y, true),
            ],
            6,
        ),
        Chroma::C411 => (
            [
                (0, x, y, false),
                (0, x + 8, y, false),
                (0, x + 16, y, false),
                (0, x + 24, y, false),
                (2, x / 4, y, false),
                (1, x / 4, y, false),
            ],
            6,
        ),
    }
}

/// The decoding state of one DCT block across the passes.
#[derive(Clone)]
struct Block {
    coef: [f32; 64],
    /// Next scan position (1 after the DC).
    pos: usize,
    mode248: bool,
    class: u8,
    qno: u8,
    done: bool,
    /// Bits of an unfinished code word carried into the next pass (at most 15).
    pending: u32,
    pending_n: usize,
    damaged: bool,
}

impl Block {
    fn new() -> Block {
        Block {
            coef: [0.0; 64],
            pos: 1,
            mode248: false,
            class: 0,
            qno: 0,
            done: false,
            pending: 0,
            pending_n: 0,
            damaged: false,
        }
    }

    /// Decodes code words from `src` at `*p` up to its end. Stops at the end of block, at an
    /// invalid word (the block is then damaged and done) or before a code word that does not
    /// fit (its first bits become `pending`).
    fn run(&mut self, src: &Bits, p: &mut usize) {
        let scan = &scans()[usize::from(self.mode248)];
        let iw = &inverse_weights()[usize::from(self.mode248)];
        let end = src.len();
        while !self.done {
            let avail = end - *p;
            let sym = lookup(src.peek16(*p));
            let len = sym.len() as usize;
            if sym == Sym::Invalid {
                if avail >= 16 {
                    self.damaged = true;
                    self.done = true;
                } else {
                    self.keep(src, *p, avail);
                    *p = end;
                }
                return;
            }
            if len > avail {
                self.keep(src, *p, avail);
                *p = end;
                return;
            }
            *p += len;
            match sym {
                Sym::EndOfBlock => self.done = true,
                Sym::Coef { run, amp, neg, .. } => {
                    let at = self.pos + usize::from(run);
                    if at >= 64 {
                        self.damaged = true;
                        self.done = true;
                        return;
                    }
                    if amp != 0 {
                        let r = usize::from(scan.raster[at]);
                        let q = i32::from(amp) * step(self.class, scan.area[at], self.qno);
                        let q = if neg { -q } else { q };
                        self.coef[r] = q as f32 * iw[r];
                    }
                    self.pos = at + 1;
                }
                Sym::Invalid => {}
            }
        }
    }

    fn keep(&mut self, src: &Bits, p: usize, n: usize) {
        self.pending = if n == 0 { 0 } else { src.read(p, n) };
        self.pending_n = n;
    }
}

/// One compressed macro block's decoding state.
struct Cm {
    blocks: [Block; 6],
    /// Which areas hold a DCT block (4:2:2 has data areas 1 and 3 for overflow only).
    real: [bool; 6],
    /// Free bits left after pass 2.
    left: Bits,
    damaged: bool,
    missing: bool,
}

/// Scratch buffers reused across segments.
#[derive(Default)]
pub(crate) struct Scratch {
    area: Bits,
    pool: Bits,
    stream: Bits,
    seg: Bits,
}

/// Decodes the five compressed macro blocks of one video segment (`cms` are the five DIF
/// blocks, 80 bytes each, `None` when missing) and returns their blocks' samples.
pub(crate) fn segment(
    layout: &Layout,
    cms: [Option<&[u8]>; 5],
    places: [MbPos; 5],
    s: &mut Scratch,
    out: &mut Vec<Placed>,
) -> u32 {
    let chroma = layout.chroma;
    let real = if chroma == Chroma::C422 {
        [true, false, true, false, true, true]
    } else {
        [true; 6]
    };
    let mut damaged = 0u32;
    let mut st: Vec<Cm> = Vec::with_capacity(5);
    for cm in cms.iter() {
        let mut c = Cm {
            blocks: std::array::from_fn(|_| Block::new()),
            real,
            left: Bits::default(),
            damaged: cm.is_none(),
            missing: cm.is_none(),
        };
        let Some(d) = cm else {
            for b in &mut c.blocks {
                b.done = true;
            }
            st.push(c);
            continue;
        };
        let sta = d[3] >> 4;
        let qno = d[3] & 15;
        // 0111 and 1111: errors in the macro block (Table 26).
        if sta == 0b0111 || sta == 0b1111 {
            c.damaged = true;
        }
        // Pass 1: each block in its own area; the free bits that remain form the pool.
        s.pool.clear();
        for a in 0..6 {
            let from = AREA_START[a] * 8;
            let to = from + AREA_BITS[a];
            if !c.real[a] {
                // X0/X1: a reserved word and EOB, then free space.
                s.pool.push_slice(d, from + 16, to);
                continue;
            }
            s.area.clear();
            s.area.push_slice(d, from, to);
            let b = &mut c.blocks[a];
            let dc = s.area.read(0, 9) as i32;
            let dc = if dc >= 256 { dc - 512 } else { dc };
            b.mode248 = s.area.read(9, 1) != 0;
            b.class = s.area.read(10, 2) as u8;
            b.qno = qno;
            if dc == -256 {
                // The video error code: no data for this block.
                b.damaged = true;
                b.done = true;
                continue;
            }
            b.coef[0] = dc as f32 * inverse_weights()[usize::from(b.mode248)][0];
            let mut p = 12;
            b.run(&s.area, &mut p);
            if b.done {
                s.pool.push_bits(&s.area, p);
            }
        }
        // Pass 2: unfinished blocks go on in the macro block's pool, in order.
        let mut ptr = 0;
        for a in 0..6 {
            let b = &mut c.blocks[a];
            if !c.real[a] || b.done {
                continue;
            }
            if ptr >= s.pool.len() {
                break;
            }
            s.stream.clear();
            s.stream.push(b.pending, b.pending_n);
            s.stream.push_bits(&s.pool, ptr);
            let pre = b.pending_n;
            b.pending_n = 0;
            let mut p = 0;
            b.run(&s.stream, &mut p);
            ptr += p.saturating_sub(pre);
            if !b.done {
                ptr = s.pool.len();
            }
        }
        c.left.push_bits(&s.pool, ptr);
        st.push(c);
    }
    // Pass 3: what is still unfinished goes on in the free bits of the whole segment.
    s.seg.clear();
    for c in &st {
        s.seg.push_bits(&c.left, 0);
    }
    let mut ptr = 0;
    'pass3: for c in st.iter_mut() {
        for a in 0..6 {
            if ptr >= s.seg.len() {
                break 'pass3;
            }
            let b = &mut c.blocks[a];
            if !c.real[a] || b.done {
                continue;
            }
            s.stream.clear();
            s.stream.push(b.pending, b.pending_n);
            s.stream.push_bits(&s.seg, ptr);
            let pre = b.pending_n;
            b.pending_n = 0;
            let mut p = 0;
            b.run(&s.stream, &mut p);
            ptr += p.saturating_sub(pre);
            if !b.done {
                break 'pass3;
            }
        }
    }
    // Inverse transforms and placement.
    for (c, &mpos) in st.iter().zip(places.iter()) {
        if c.damaged {
            damaged += 1;
        }
        let (where_, n) = block_places(chroma, mpos);
        let mut bi = 0;
        for (a, b) in c.blocks.iter().enumerate() {
            if !c.real[a] {
                continue;
            }
            if bi >= n {
                break;
            }
            let (plane, x, y, folded) = where_[bi];
            bi += 1;
            if b.damaged {
                damaged += 1;
            }
            let mut px = [128u8; 64];
            if !c.missing {
                idct(&b.coef, b.mode248, &mut px);
            }
            out.push(Placed {
                plane,
                x: x as u16,
                y: y as u16,
                folded,
                px,
            });
        }
    }
    damaged
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every luma sample of the frame belongs to exactly one macro block.
    fn covers(chroma: Chroma, rows: u32, w: u32, h: u32) {
        let mut seen = vec![0u8; (w * h) as usize];
        for i in 0..rows {
            for j in 0..5 {
                for k in 0..27 {
                    let m = mb_pos(chroma, i, j, k);
                    let (bw, bh) = match (chroma, m.square) {
                        (_, true) => (16, 16),
                        (Chroma::C411, false) => (32, 8),
                        _ => (16, 8),
                    };
                    for y in m.y..m.y + bh {
                        for x in m.x..m.x + bw {
                            seen[(y * w + x) as usize] += 1;
                        }
                    }
                }
            }
        }
        assert!(seen.iter().all(|&s| s == 1), "{chroma:?}");
    }

    #[test]
    fn macro_blocks_tile_the_frame() {
        covers(Chroma::C411, 10, 720, 480);
        covers(Chroma::C411, 12, 720, 576);
        covers(Chroma::C420, 12, 720, 576);
        covers(Chroma::C422, 20, 720, 480);
        covers(Chroma::C422, 24, 720, 576);
    }
}
