//! Frame decode (spec chapter 7): header, coded block flags, macro block modes, motion
//! vectors, block-level qi, DCT tokens, DC prediction, reconstruction and loop filter.

use crate::bits::Bits;
use crate::layout::Layout;
use crate::recon;
use crate::setup::Setup;
use crate::{Chroma, Error, Info, Picture, malformed};

/// Zig-zag index -> natural index; entries 64.. catch overruns.
const DEZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Mode alphabets of schemes 1..6 (Table 7.19).
const SCHEMES: [[u8; 8]; 6] = [
    [3, 4, 2, 0, 1, 5, 6, 7],
    [3, 4, 0, 2, 1, 5, 6, 7],
    [3, 2, 4, 0, 1, 5, 6, 7],
    [3, 2, 0, 4, 1, 5, 6, 7],
    [0, 3, 4, 2, 1, 5, 6, 7],
    [0, 5, 3, 4, 2, 1, 6, 7],
];

const MODE_INTRA: u8 = 1;
const MODE_GOLDEN_MV: u8 = 6;
const MODE_FOUR_MV: u8 = 7;

/// Reference frame index per mode (Table 7.46): 0 none, 1 previous, 2 golden.
const MODE_REF: [u8; 8] = [1, 0, 1, 1, 1, 2, 2, 1];
/// `bref` of an uncoded block: matches no reference.
const NOT_CODED: u8 = 3;

/// DC predictor weights and divisor per neighbour set (Table 7.47), L | DL<<1 | D<<2 | DR<<3.
const DC_WEIGHTS: [([i32; 4], i32); 16] = [
    ([0, 0, 0, 0], 1),
    ([1, 0, 0, 0], 1),
    ([0, 1, 0, 0], 1),
    ([1, 0, 0, 0], 1),
    ([0, 0, 1, 0], 1),
    ([1, 0, 1, 0], 2),
    ([0, 0, 1, 0], 1),
    ([29, -26, 29, 0], 32),
    ([0, 0, 0, 1], 1),
    ([75, 0, 0, 53], 128),
    ([0, 1, 0, 1], 2),
    ([75, 0, 0, 53], 128),
    ([0, 0, 1, 0], 1),
    ([75, 0, 0, 53], 128),
    ([0, 3, 10, 3], 16),
    ([29, -26, 29, 0], 32),
];

type Frame = [Vec<u8>; 3];

pub(crate) struct State {
    pub(crate) info: Info,
    layout: Layout,
    setup: Setup,
    coded: Vec<u8>,
    bref: Vec<u8>,
    mvs: Vec<[i8; 2]>,
    qii: Vec<u8>,
    ncoef: Vec<u8>,
    tis: Vec<u8>,
    coefs: Vec<i16>,
    mb_mode: Vec<u8>,
    coded_list: Vec<u32>,
    active: Vec<u32>,
    bits: Vec<u8>,
    bits2: Vec<u8>,
    frames: [Frame; 3],
    prev: Option<usize>,
    gold: Option<usize>,
    last: Option<Picture>,
    /// The memory budget's share of the buffers above, held for the decoder's life.
    _res: specular_core::Reservation,
}

impl State {
    pub(crate) fn new(info: Info, setup: Setup) -> Result<State, Error> {
        let layout = Layout::new(
            info.frame_width as usize / 16,
            info.frame_height as usize / 16,
            info.chroma,
        );
        let nbs = layout.nbs;
        let nmbs = layout.mbw * layout.mbh;
        // Three frames, then per block: 64 coefficients and the state and lists below.
        let pixels: usize = layout.planes.iter().map(|p| p.len).sum();
        let bytes = 3 * pixels as u64 + nbs as u64 * (64 * 2 + 2 + 6 + 4 * 4) + nmbs as u64;
        let res = specular_core::budget::reserve(bytes).map_err(Error::NeedsMemory)?;
        let frame = || -> Frame { [0, 1, 2].map(|p| vec![0u8; layout.planes[p].len]) };
        let frames = [frame(), frame(), frame()];
        Ok(State {
            info,
            setup,
            coded: vec![0; nbs],
            bref: vec![NOT_CODED; nbs],
            mvs: vec![[0, 0]; nbs],
            qii: vec![0; nbs],
            ncoef: vec![0; nbs],
            tis: vec![0; nbs],
            coefs: vec![0; nbs * 64],
            mb_mode: vec![0; nmbs],
            coded_list: Vec::with_capacity(nbs),
            active: Vec::with_capacity(nbs),
            bits: Vec::with_capacity(nbs),
            bits2: Vec::with_capacity(nbs),
            frames,
            prev: None,
            gold: None,
            last: None,
            layout,
            _res: res,
        })
    }

    pub(crate) fn reset(&mut self) {
        self.prev = None;
        self.gold = None;
        self.last = None;
    }

    pub(crate) fn decode(&mut self, packet: &[u8]) -> Result<Picture, Error> {
        if packet.is_empty() {
            let mut p = self
                .last
                .clone()
                .ok_or_else(|| malformed("empty frame before the first key frame"))?;
            p.keyframe = false;
            return Ok(p);
        }
        let mut br = Bits::new(packet);
        let (intra, qis, nqis) = if self.info.vp3 {
            vp3_header(&mut br)?
        } else {
            if br.bit() != 0 {
                return Err(malformed("not a frame packet"));
            }
            let intra = br.bit() == 0;
            let mut qis = [br.read(6) as usize; 3];
            let mut nqis = 1;
            if br.bit() == 1 {
                qis[1] = br.read(6) as usize;
                nqis = 2;
                if br.bit() == 1 {
                    qis[2] = br.read(6) as usize;
                    nqis = 3;
                }
            }
            if intra && br.read(3) != 0 {
                return Err(Error::Unsupported(
                    "Theora frames with reserved header bits".into(),
                ));
            }
            (intra, qis, nqis)
        };
        if !intra && self.prev.is_none() {
            return Err(malformed("inter frame before the first key frame"));
        }
        let r = self.decode_body(&mut br, intra, &qis[..nqis]);
        if r.is_err() {
            self.coefs.fill(0);
        }
        r
    }

    fn decode_body(&mut self, br: &mut Bits, intra: bool, qis: &[usize]) -> Result<Picture, Error> {
        self.coded_blocks(br, intra)?;
        self.modes(br, intra);
        if !intra {
            self.motion_vectors(br);
        }
        for &b in &self.coded_list {
            let b = b as usize;
            self.bref[b] = MODE_REF[self.mb_mode[self.layout.blk_mb[b] as usize] as usize];
        }
        self.block_qi(br, qis.len())?;
        self.tokens(br)?;
        self.undo_dc_prediction();

        // Reconstruct into a buffer that is neither reference.
        let cur = (0..3)
            .find(|&i| Some(i) != self.prev && Some(i) != self.gold)
            .unwrap_or(0);
        let mut frame = std::mem::take(&mut self.frames[cur]);
        self.reconstruct(&mut frame, qis);
        let l = i32::from(self.setup.lflims[qis[0]]);
        for p in 0..3 {
            let g = self.layout.planes[p];
            if l > 0 {
                let lf = recon::lflim_table(l);
                let coded = &self.coded[g.first..g.first + g.bw * g.bh];
                recon::loop_filter(&mut frame[p], &g, coded, &lf);
            }
            recon::extend_borders(&mut frame[p], &g);
        }
        let pic = self.picture(&frame, intra);
        self.frames[cur] = frame;
        self.prev = Some(cur);
        if intra {
            self.gold = Some(cur);
        }
        self.last = Some(pic.clone());
        Ok(pic)
    }

    /// Coded block flags (7.3); fills `coded` and `coded_list` (coded order).
    fn coded_blocks(&mut self, br: &mut Bits, intra: bool) -> Result<(), Error> {
        let lay = &self.layout;
        // Clear the previous frame's flags.
        for &b in &self.coded_list {
            self.coded[b as usize] = 0;
            self.bref[b as usize] = NOT_CODED;
        }
        self.coded_list.clear();
        if intra {
            self.coded.fill(1);
            self.coded_list.extend_from_slice(&lay.coded_order);
            return Ok(());
        }
        let nsbs = lay.sb_end.len();
        long_run(br, nsbs, &mut self.bits, self.info.vp3)?;
        let nfull = self.bits.iter().filter(|&&b| b == 0).count();
        long_run(br, nfull, &mut self.bits2, self.info.vp3)?;
        let mut npart = 0;
        let mut start = 0;
        for (sb, &end) in lay.sb_end.iter().enumerate() {
            if self.bits[sb] == 1 {
                npart += end as usize - start;
            }
            start = end as usize;
        }
        let sbp = std::mem::take(&mut self.bits);
        short_run(br, npart, &mut self.bits)?;
        let (mut fi, mut pi) = (0, 0);
        let mut start = 0;
        for (sb, &end) in lay.sb_end.iter().enumerate() {
            let blocks = &lay.coded_order[start..end as usize];
            start = end as usize;
            if sbp[sb] == 1 {
                for &b in blocks {
                    if self.bits[pi] == 1 {
                        self.coded[b as usize] = 1;
                        self.coded_list.push(b);
                    }
                    pi += 1;
                }
            } else {
                let f = self.bits2[fi];
                fi += 1;
                if f == 1 {
                    for &b in blocks {
                        self.coded[b as usize] = 1;
                    }
                    self.coded_list.extend_from_slice(blocks);
                }
            }
        }
        self.bits2 = sbp;
        Ok(())
    }

    /// Macro block coding modes (7.4).
    fn modes(&mut self, br: &mut Bits, intra: bool) {
        if intra {
            self.mb_mode.fill(MODE_INTRA);
            return;
        }
        let scheme = br.read(3) as usize;
        let mut alpha = [0u8; 8];
        if scheme == 0 {
            for mode in 0..8 {
                alpha[br.read(3) as usize] = mode;
            }
        } else if scheme < 7 {
            alpha = SCHEMES[scheme - 1];
        }
        let lay = &self.layout;
        for &mb in &lay.mb_order {
            let mb = mb as usize;
            let (mx, my) = (mb % lay.mbw, mb / lay.mbw);
            let a = lay.blk(0, 2 * mx, 2 * my);
            let c = a + lay.planes[0].bw;
            let any = (self.coded[a] | self.coded[a + 1] | self.coded[c] | self.coded[c + 1]) != 0;
            self.mb_mode[mb] = if !any {
                0
            } else if scheme == 7 {
                br.read(3) as u8
            } else {
                let mi = (!br.peek32()).leading_zeros().min(7);
                br.skip((mi + 1).min(7));
                alpha[mi as usize]
            };
        }
    }

    /// The blocks of a macro block: luma in raster order, then Cb, then Cr.
    fn mb_blocks(&self, mb: usize) -> ([usize; 12], usize) {
        let lay = &self.layout;
        let (mx, my) = (mb % lay.mbw, mb / lay.mbw);
        let mut out = [0usize; 12];
        let a = lay.blk(0, 2 * mx, 2 * my);
        let w = lay.planes[0].bw;
        out[..4].copy_from_slice(&[a, a + 1, a + w, a + w + 1]);
        let mut n = 4;
        for p in 1..3 {
            match self.info.chroma {
                Chroma::C420 => {
                    out[n] = lay.blk(p, mx, my);
                    n += 1;
                }
                Chroma::C422 => {
                    let b = lay.blk(p, mx, 2 * my);
                    out[n] = b;
                    out[n + 1] = b + lay.planes[p].bw;
                    n += 2;
                }
                Chroma::C444 => {
                    let b = lay.blk(p, 2 * mx, 2 * my);
                    let w = lay.planes[p].bw;
                    out[n..n + 4].copy_from_slice(&[b, b + 1, b + w, b + w + 1]);
                    n += 4;
                }
            }
        }
        (out, n)
    }

    /// Motion vectors (7.5).
    fn motion_vectors(&mut self, br: &mut Bits) {
        let raw = br.bit() == 1;
        let read = |br: &mut Bits| -> [i8; 2] { [read_mv(br, raw), read_mv(br, raw)] };
        let mut last1 = [0i8; 2];
        let mut last2 = [0i8; 2];
        for i in 0..self.layout.mb_order.len() {
            let mb = self.layout.mb_order[i] as usize;
            let mode = self.mb_mode[mb];
            let (blocks, n) = self.mb_blocks(mb);
            let v = match mode {
                MODE_FOUR_MV => {
                    let mut l = [[0i8; 2]; 4];
                    let mut lastv = [0i8; 2];
                    // VP3 reads all four vectors, coded blocks or not (7.5.2).
                    let vp3 = self.info.vp3;
                    for k in 0..4 {
                        if vp3 || self.coded[blocks[k]] != 0 {
                            l[k] = read(br);
                            lastv = l[k];
                        }
                        self.mvs[blocks[k]] = l[k];
                    }
                    match self.info.chroma {
                        Chroma::C420 => {
                            let v = [avg4(l, 0), avg4(l, 1)];
                            self.mvs[blocks[4]] = v;
                            self.mvs[blocks[5]] = v;
                        }
                        Chroma::C422 => {
                            let lo = [avg2(l[0][0], l[1][0]), avg2(l[0][1], l[1][1])];
                            let hi = [avg2(l[2][0], l[3][0]), avg2(l[2][1], l[3][1])];
                            for p in 0..2 {
                                self.mvs[blocks[4 + 2 * p]] = lo;
                                self.mvs[blocks[5 + 2 * p]] = hi;
                            }
                        }
                        Chroma::C444 => {
                            for k in 0..4 {
                                self.mvs[blocks[4 + k]] = l[k];
                                self.mvs[blocks[8 + k]] = l[k];
                            }
                        }
                    }
                    last2 = last1;
                    last1 = lastv;
                    continue;
                }
                MODE_GOLDEN_MV => read(br),
                4 => {
                    // INTER_MV_LAST2: the two last vectors trade places.
                    std::mem::swap(&mut last1, &mut last2);
                    last1
                }
                3 => last1,
                2 => {
                    let v = read(br);
                    last2 = last1;
                    last1 = v;
                    v
                }
                _ => [0, 0],
            };
            for &b in &blocks[..n] {
                self.mvs[b] = v;
            }
        }
    }

    /// Block-level qi indices (7.6).
    fn block_qi(&mut self, br: &mut Bits, nqis: usize) -> Result<(), Error> {
        for &b in &self.coded_list {
            self.qii[b as usize] = 0;
        }
        for q in 0..nqis.saturating_sub(1) {
            let q = q as u8;
            let n = self
                .coded_list
                .iter()
                .filter(|&&b| self.qii[b as usize] == q)
                .count();
            long_run(br, n, &mut self.bits, self.info.vp3)?;
            let mut k = 0;
            for &b in &self.coded_list {
                let b = b as usize;
                if self.qii[b] == q {
                    self.qii[b] += self.bits[k];
                    k += 1;
                }
            }
        }
        Ok(())
    }

    /// DCT tokens (7.7.3) into `coefs` (natural order), `ncoef`.
    fn tokens(&mut self, br: &mut Bits) -> Result<(), Error> {
        let nlbs = self.layout.planes[0].bw * self.layout.planes[0].bh;
        let huff = &self.setup.huff;
        let tis = &mut self.tis;
        let ncoef = &mut self.ncoef;
        let coefs = &mut self.coefs;
        let active = &mut self.active;
        active.clear();
        active.extend_from_slice(&self.coded_list);
        for &b in active.iter() {
            tis[b as usize] = 0;
        }
        let mut remaining = active.len();
        let mut eobs = 0usize;
        let (mut htl, mut htc) = (0usize, 0usize);
        for ti in 0..64u32 {
            if ti <= 1 {
                htl = br.read(4) as usize;
                htc = br.read(4) as usize;
            }
            let group = match ti {
                0 => 0,
                1..=5 => 1,
                6..=14 => 2,
                15..=27 => 3,
                _ => 4,
            };
            let (tl, tc) = (16 * group + htl, 16 * group + htc);
            let mut w = 0;
            for r in 0..active.len() {
                let b32 = active[r];
                let b = b32 as usize;
                if u32::from(tis[b]) != ti {
                    active[w] = b32;
                    w += 1;
                    continue;
                }
                ncoef[b] = ti as u8;
                if eobs > 0 {
                    tis[b] = 64;
                    eobs -= 1;
                    remaining -= 1;
                    continue;
                }
                let t = huff.token(br, if b < nlbs { tl } else { tc });
                if t < 7 {
                    eobs = match t {
                        0 => 1,
                        1 => 2,
                        2 => 3,
                        3 => br.read(2) as usize + 4,
                        4 => br.read(3) as usize + 8,
                        5 => br.read(4) as usize + 16,
                        _ => match br.read(12) as usize {
                            0 => remaining,
                            v => v,
                        },
                    };
                    tis[b] = 64;
                    eobs -= 1;
                    remaining -= 1;
                    continue;
                }
                let neg = |s: u32, m: i32| if s == 1 { -m } else { m };
                let (run, val) = match t {
                    7 => (br.read(3) + 1, 0),
                    8 => (br.read(6) + 1, 0),
                    9 => (0, 1),
                    10 => (0, -1),
                    11 => (0, 2),
                    12 => (0, -2),
                    13..=16 => (0, neg(br.bit(), t as i32 - 10)),
                    17..=22 => {
                        let s = br.bit();
                        let m = match t {
                            17 => br.read(1) + 7,
                            18 => br.read(2) + 9,
                            19 => br.read(3) + 13,
                            20 => br.read(4) + 21,
                            21 => br.read(5) + 37,
                            _ => br.read(9) + 69,
                        };
                        (0, neg(s, m as i32))
                    }
                    23..=27 => (t - 22, neg(br.bit(), 1)),
                    28 => {
                        let s = br.bit();
                        (br.read(2) + 6, neg(s, 1))
                    }
                    29 => {
                        let s = br.bit();
                        (br.read(3) + 10, neg(s, 1))
                    }
                    30 => {
                        let s = br.bit();
                        (1, neg(s, br.read(1) as i32 + 2))
                    }
                    _ => {
                        let s = br.bit();
                        let m = br.read(1) as i32 + 2;
                        (br.read(1) + 2, neg(s, m))
                    }
                };
                let next = if val == 0 {
                    ti + run
                } else {
                    let pos = ti + run;
                    if pos > 63 {
                        return Err(malformed("coefficients run past the block"));
                    }
                    coefs[b * 64 + DEZIGZAG[pos as usize] as usize] = val as i16;
                    ncoef[b] = (pos + 1) as u8;
                    pos + 1
                };
                if next > 64 {
                    return Err(malformed("coefficients run past the block"));
                }
                tis[b] = next as u8;
                if next < 64 {
                    active[w] = b32;
                    w += 1;
                } else {
                    remaining -= 1;
                }
            }
            active.truncate(w);
        }
        Ok(())
    }

    /// Undoes DC prediction (7.8), plane by plane in raster order.
    fn undo_dc_prediction(&mut self) {
        let bref = &self.bref;
        let coefs = &mut self.coefs;
        for g in &self.layout.planes {
            let mut last = [0i32; 3];
            for y in 0..g.bh {
                for x in 0..g.bw {
                    let b = g.first + y * g.bw + x;
                    let rfi = bref[b];
                    if rfi == NOT_CODED {
                        continue;
                    }
                    let mut idx = 0;
                    if x > 0 && bref[b - 1] == rfi {
                        idx |= 1;
                    }
                    if y > 0 {
                        let d = b - g.bw;
                        if x > 0 && bref[d - 1] == rfi {
                            idx |= 2;
                        }
                        if bref[d] == rfi {
                            idx |= 4;
                        }
                        if x + 1 < g.bw && bref[d + 1] == rfi {
                            idx |= 8;
                        }
                    }
                    let pred = if idx == 0 {
                        last[rfi as usize]
                    } else {
                        let dc = |o: usize| i32::from(coefs[o * 64]);
                        let (w, div) = DC_WEIGHTS[idx];
                        let l = if idx & 1 != 0 { dc(b - 1) } else { 0 };
                        let dl = if idx & 2 != 0 { dc(b - g.bw - 1) } else { 0 };
                        let d = if idx & 4 != 0 { dc(b - g.bw) } else { 0 };
                        let dr = if idx & 8 != 0 { dc(b - g.bw + 1) } else { 0 };
                        let mut p = (w[0] * l + w[1] * dl + w[2] * d + w[3] * dr) / div;
                        if idx & 7 == 7 {
                            if (p - d).abs() > 128 {
                                p = d;
                            } else if (p - l).abs() > 128 {
                                p = l;
                            } else if (p - dl).abs() > 128 {
                                p = dl;
                            }
                        }
                        p
                    };
                    let v = (i32::from(coefs[b * 64]) + pred) as i16;
                    coefs[b * 64] = v;
                    last[rfi as usize] = i32::from(v);
                }
            }
        }
    }

    /// Reconstructs every block (7.9.4) into `frame`, clearing the used coefficients.
    fn reconstruct(&mut self, frame: &mut Frame, qis: &[usize]) {
        let empty: &[u8] = &[];
        for p in 0..3 {
            let g = self.layout.planes[p];
            let prev = self.prev.map_or(empty, |i| &self.frames[i][p][..]);
            let gold = self.gold.map_or(empty, |i| &self.frames[i][p][..]);
            let dst = &mut frame[p][..];
            let s = g.stride;
            let mut pred = [0u8; 64];
            let mut res = [0i32; 64];
            for by in 0..g.bh {
                for bx in 0..g.bw {
                    let b = g.first + by * g.bw + bx;
                    let pos = g.origin + by * 8 * s + bx * 8;
                    let rfi = self.bref[b];
                    if rfi == NOT_CODED {
                        recon::copy_block(dst, prev, s, pos);
                        continue;
                    }
                    let qti = usize::from(rfi != 0);
                    if rfi == 0 {
                        pred = [128; 64];
                    } else {
                        let r = if rfi == 1 { prev } else { gold };
                        recon::predict(r, s, pos, self.mvs[b], &g, &mut pred);
                    }
                    let base = (qti * 3 + p) * 64;
                    let c = &mut self.coefs[b * 64..b * 64 + 64];
                    let dcq = i32::from(self.setup.qmat[base + qis[0]][0]);
                    if self.ncoef[b] < 2 {
                        let dc = t16((i32::from(c[0]) * dcq + 15) >> 5);
                        c[0] = 0;
                        recon::put_dc(dst, s, pos, &pred, dc);
                    } else {
                        let q = &self.setup.qmat[base + qis[self.qii[b] as usize]];
                        res[0] = t16(i32::from(c[0]) * dcq);
                        for i in 1..64 {
                            res[i] = t16(i32::from(c[i]) * i32::from(q[i]));
                        }
                        c.fill(0);
                        recon::idct2(&mut res);
                        recon::put_res(dst, s, pos, &pred, &res);
                    }
                }
            }
        }
    }

    /// The picture region, top row first.
    fn picture(&self, frame: &Frame, keyframe: bool) -> Picture {
        let info = &self.info;
        let (pw, ph) = (info.pic_width as usize, info.pic_height as usize);
        let top = (info.frame_height - info.pic_y - info.pic_height) as usize;
        let planes = [0, 1, 2].map(|p| {
            let g = &self.layout.planes[p];
            let (w, h) = (pw.div_ceil(1 << g.sx), ph.div_ceil(1 << g.sy));
            let (x0, t0) = (info.pic_x as usize >> g.sx, top >> g.sy);
            let mut out = Vec::with_capacity(w * h);
            for r in 0..h {
                let y = g.h - 1 - (t0 + r);
                let o = g.origin + y * g.stride + x0;
                out.extend_from_slice(&frame[p][o..o + w]);
            }
            out
        });
        Picture {
            width: info.pic_width,
            height: info.pic_height,
            chroma: info.chroma,
            planes,
            keyframe,
        }
    }
}

#[inline(always)]
fn t16(v: i32) -> i32 {
    v as i16 as i32
}

/// round(s / 4), ties away from zero.
fn avg4(l: [[i8; 2]; 4], c: usize) -> i8 {
    let s: i32 = l.iter().map(|v| i32::from(v[c])).sum();
    (if s >= 0 {
        (s + 2) >> 2
    } else {
        -((-s + 2) >> 2)
    }) as i8
}

/// round((a + b) / 2), ties away from zero.
fn avg2(a: i8, b: i8) -> i8 {
    let s = i32::from(a) + i32::from(b);
    (if s >= 0 {
        (s + 1) >> 1
    } else {
        -((-s + 1) >> 1)
    }) as i8
}

/// One motion vector component (7.5.1).
#[inline]
fn read_mv(br: &mut Bits, raw: bool) -> i8 {
    if raw {
        let m = br.read(5) as i8;
        return if br.bit() == 1 { -m } else { m };
    }
    let (mag, sign) = match br.read(3) {
        0 => return 0,
        1 => return 1,
        2 => return -1,
        3 => (2, br.bit()),
        4 => (3, br.bit()),
        5 => {
            let v = br.read(3);
            (4 + (v >> 1), v & 1)
        }
        6 => {
            let v = br.read(4);
            (8 + (v >> 1), v & 1)
        }
        _ => {
            let v = br.read(5);
            (16 + (v >> 1), v & 1)
        }
    };
    if sign == 1 { -(mag as i8) } else { mag as i8 }
}

/// VP3.1's frame header (the Theora specification leaves it out, 7.1): the frame type (0
/// for a key frame), an unused bit and the frame's one qi; key frames go on with the frame
/// size codes (4 + 4 bits, unused: the container gives the size), the version (5 bits) and
/// the key frame coding type (1 bit, 0 for DCT, the only one) with 2 spare bits. Found
/// against the FATE sample's frames, which carry version 1.
fn vp3_header(br: &mut Bits) -> Result<(bool, [usize; 3], usize), Error> {
    let intra = br.bit() == 0;
    br.skip(1);
    let qi = br.read(6) as usize;
    if intra {
        br.skip(4 + 4 + 5);
        if br.bit() != 0 {
            return Err(Error::Unsupported(
                "VP3 key frames of another coding type".into(),
            ));
        }
        br.skip(2);
    }
    Ok((intra, [qi; 3], 1))
}

/// Long-run bit string (7.2.1) of `n` bits.
fn long_run(br: &mut Bits, n: usize, out: &mut Vec<u8>, vp3: bool) -> Result<(), Error> {
    out.clear();
    if n == 0 {
        return Ok(());
    }
    let mut bit = br.bit() as u8;
    loop {
        let ones = (!br.peek32()).leading_zeros();
        let (start, extra, len) = match ones {
            0 => (1, 0, 1),
            1 => (2, 1, 2),
            2 => (4, 1, 3),
            3 => (6, 2, 4),
            4 => (10, 3, 5),
            5 => (18, 4, 6),
            _ => (34, 12, 6),
        };
        br.skip(len);
        let rlen = start + br.read(extra) as usize;
        if out.len() + rlen > n {
            return Err(malformed("coded flags overrun"));
        }
        out.resize(out.len() + rlen, bit);
        if out.len() == n {
            return Ok(());
        }
        // VP3 reads no new bit after the longest run (Theora specification 7.2.1).
        if rlen == 4129 && !vp3 {
            bit = br.bit() as u8;
        } else {
            bit ^= 1;
        }
    }
}

/// Short-run bit string (7.2.2) of `n` bits.
fn short_run(br: &mut Bits, n: usize, out: &mut Vec<u8>) -> Result<(), Error> {
    out.clear();
    if n == 0 {
        return Ok(());
    }
    let mut bit = br.bit() as u8;
    loop {
        let ones = (!br.peek32()).leading_zeros();
        let (start, extra, len) = match ones {
            0 => (1, 1, 1),
            1 => (3, 1, 2),
            2 => (5, 1, 3),
            3 => (7, 2, 4),
            4 => (11, 2, 5),
            _ => (15, 4, 5),
        };
        br.skip(len);
        let rlen = start + br.read(extra) as usize;
        if out.len() + rlen > n {
            return Err(malformed("coded flags overrun"));
        }
        out.resize(out.len() + rlen, bit);
        if out.len() == n {
            return Ok(());
        }
        bit ^= 1;
    }
}
