//! Tile and macroblock parsing (T.832 8.5-8.12) with DC/LP/HP prediction, dequantisation and
//! the HP coefficient prediction of clause 9.4-9.8.
//!
//! Every image plane keeps one full-resolution `i32` coefficient plane per component (see
//! `transform`): the DC/LP coefficient of a 4x4 block at its top-left sample, the HP coefficients
//! at the other 15 positions. Parsing writes quantised, already DC/LP-predicted values; a post
//! pass per macroblock dequantises and applies the HP prediction.

// Index loops over components and blocks mirror the spec's pseudo-code and touch several
// parallel arrays; iterator rewrites obscure that.
#![allow(clippy::needless_range_loop)]

use crate::Error;
use crate::bits::Bits;
use crate::header::{
    Bands, ClrFmt, ImageHeader, PlaneHeader, Qp, TileQp, image_header, plane_header, tile_dc,
    tile_hp, tile_lp,
};
use crate::vlc::{
    ABS_LEVEL_DELTA, Adapt, FIRST_INDEX_DELTA, INDEX1_DELTA, NUM_BLK_CBPHP_DELTA9, NUM_CBPHP_DELTA,
    REF_CBPHP1_VAL, T,
};

const SCAN0: [u8; 16] = [0, 4, 1, 5, 8, 2, 9, 6, 12, 3, 10, 13, 7, 14, 11, 15];
const SCAN1: [u8; 16] = [0, 1, 2, 5, 4, 3, 6, 9, 8, 7, 12, 15, 13, 10, 11, 14];
const TOTALS: [u16; 16] = [0, 32, 30, 28, 26, 24, 22, 20, 18, 16, 14, 12, 10, 8, 6, 4];
const T444: [usize; 16] = [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15];
const T422: [usize; 8] = [0, 2, 1, 3, 4, 6, 5, 7];
const T420: [usize; 4] = [0, 2, 1, 3];
const HIER: [usize; 16] = [0, 1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15];

/// Adaptive inverse scan (8.11).
#[derive(Clone, Copy)]
struct Scan {
    order: [u8; 16],
    totals: [u16; 16],
}

impl Scan {
    fn new(order: [u8; 16]) -> Scan {
        Scan {
            order,
            totals: TOTALS,
        }
    }

    fn reset(&mut self) {
        self.totals = TOTALS;
    }

    /// `i` is 1..=15.
    #[inline]
    fn put(&mut self, i: usize, v: i32, out: &mut [i32; 16]) {
        out[self.order[i] as usize] = v;
        self.totals[i] += 1;
        if i > 1 && self.totals[i] > self.totals[i - 1] {
            self.totals.swap(i, i - 1);
            self.order.swap(i, i - 1);
        }
    }
}

/// Adaptive coefficient normalisation state (8.12).
#[derive(Clone, Copy)]
struct Model {
    ms: [i32; 2],
    bits: [i32; 2],
}

impl Model {
    fn new(band: i32) -> Model {
        Model {
            ms: [0; 2],
            bits: [(2 - band) * 4; 2],
        }
    }

    fn update(&mut self, mut lap: [i32; 2], band: usize, clr: ClrFmt, nc: usize) {
        const W0: [i32; 3] = [240, 12, 1];
        const W1: [[i32; 16]; 3] = [
            [
                0, 240, 120, 80, 60, 48, 40, 34, 30, 27, 24, 22, 20, 18, 17, 16,
            ],
            [0, 12, 6, 4, 3, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1],
            [0, 16, 8, 5, 4, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1, 1],
        ];
        lap[0] *= W0[band];
        match clr {
            ClrFmt::Yuv420 => lap[1] *= [120, 37, 2][band],
            ClrFmt::Yuv422 => lap[1] *= [120, 18, 1][band],
            _ => {
                lap[1] *= W1[band][(nc - 1).min(15)];
                if band == 2 {
                    lap[1] >>= 4;
                }
            }
        }
        let models = if clr == ClrFmt::YOnly { 1 } else { 2 };
        for j in 0..models {
            let mut ms = self.ms[j];
            let mut delta = (lap[j] - 70) >> 2;
            if delta <= -8 {
                delta = (delta + 4).max(-16);
                ms += delta;
                if ms < -8 {
                    if self.bits[j] == 0 {
                        ms = -8;
                    } else {
                        ms = 0;
                        self.bits[j] -= 1;
                    }
                }
            } else if delta >= 8 {
                delta = (delta - 4).min(15);
                ms += delta;
                if ms > 8 {
                    if self.bits[j] >= 15 {
                        self.bits[j] = 15;
                        ms = 8;
                    } else {
                        ms = 0;
                        self.bits[j] += 1;
                    }
                }
            }
            self.ms[j] = ms;
        }
    }
}

/// The adaptive VLC state, scans and model of the LP or HP band.
#[derive(Clone, Copy)]
struct BandVlc {
    first: [Adapt; 2],
    ind: [[Adapt; 2]; 2],
    abs: [Adapt; 2],
    /// LP uses `scan[0]`; HP uses `scan[0]` (horizontal) and `scan[1]` (vertical).
    scan: [Scan; 2],
    model: Model,
}

impl BandVlc {
    fn new(band: i32) -> BandVlc {
        BandVlc {
            first: [Adapt::table2(); 2],
            ind: [[Adapt::table2(); 2]; 2],
            abs: [Adapt::table1(); 2],
            scan: [Scan::new(SCAN0), Scan::new(SCAN1)],
            model: Model::new(band),
        }
    }

    fn adapt(&mut self) {
        for f in &mut self.first {
            f.adapt2(4);
        }
        for row in &mut self.ind {
            for a in row {
                a.adapt2(3);
            }
        }
        for a in &mut self.abs {
            a.adapt1();
        }
    }
}

/// Per-plane, per-tile parsing context. Every band's state is initialised at the tile's top-left
/// macroblock, so a fresh context per tile is exactly the spec's initialisation.
struct Ctx {
    dc_abs: [Adapt; 2],
    dc_model: Model,
    cbplp_zero: i32,
    cbplp_max: i32,
    lp: BandVlc,
    hp: BandVlc,
    num_cbphp: Adapt,
    num_blk_cbphp: Adapt,
    cbp_state: [u8; 2],
    ones: [i32; 2],
    zeroes: [i32; 2],
}

impl Ctx {
    fn new() -> Ctx {
        Ctx {
            dc_abs: [Adapt::table1(); 2],
            dc_model: Model::new(0),
            cbplp_zero: 1,
            cbplp_max: 1,
            lp: BandVlc::new(1),
            hp: BandVlc::new(2),
            num_cbphp: Adapt::table1(),
            num_blk_cbphp: Adapt::table1(),
            cbp_state: [0; 2],
            ones: [-4; 2],
            zeroes: [4; 2],
        }
    }

    fn update_cbphp_model(&mut self, i: usize, n: i32) {
        self.ones[i] = (self.ones[i] + n - 3).clamp(-16, 15);
        self.zeroes[i] = (self.zeroes[i] + 16 - n - 3).clamp(-16, 15);
        self.cbp_state[i] = if self.ones[i] < 0 {
            if self.ones[i] < self.zeroes[i] { 1 } else { 2 }
        } else if self.zeroes[i] < 0 {
            2
        } else {
            0
        };
    }
}

fn malformed(what: &str) -> Error {
    Error::Malformed(what.into())
}

#[inline]
fn abs_level(b: &mut Bits, a: &mut Adapt) -> i32 {
    const REMAP: [i32; 6] = [2, 3, 4, 6, 10, 14];
    const FIXED: [u32; 6] = [0, 0, 1, 2, 2, 2];
    let x = T.abs_level[a.idx as usize].get(b);
    a.d1 += ABS_LEVEL_DELTA[x];
    if x < 6 {
        REMAP[x] + b.u(FIXED[x]) as i32
    } else {
        let mut f = b.u(4) + 4;
        if f == 19 {
            f += b.u(2);
            if f == 22 {
                f += b.u(3);
            }
        }
        2i32.wrapping_add(1 << f).wrapping_add(b.u(f) as i32)
    }
}

#[inline]
fn decode_dc(b: &mut Bits, a: &mut Adapt, mbits: i32, abs: bool) -> i32 {
    let mut v = if abs { abs_level(b, a) - 1 } else { 0 };
    if mbits > 0 {
        v = (v << mbits) | b.u(mbits as u32) as i32;
    }
    if v != 0 && b.bit() {
        v = -v;
    }
    v
}

#[inline]
fn decode_run(b: &mut Bits, max: i32) -> Result<i32, Error> {
    const RUN_BIN: [i32; 15] = [-1, -1, -1, -1, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0];
    const REMAP: [i32; 15] = [1, 2, 3, 5, 7, 1, 2, 3, 5, 7, 1, 2, 3, 4, 5];
    const FIXED: [u32; 15] = [0, 0, 1, 1, 3, 0, 0, 1, 1, 2, 0, 0, 0, 0, 1];
    match max {
        ..=0 => Err(malformed("coefficient run past the block")),
        1 => Ok(1),
        2..=4 => Ok(T.run[max as usize - 2].get(b) as i32 + 1),
        _ => {
            let idx = (T.run_index.get(b) as i32 + 5 * RUN_BIN[max as usize]) as usize;
            Ok(REMAP[idx] + b.u(FIXED[idx]) as i32)
        }
    }
}

#[inline]
fn decode_index(b: &mut Bits, loc: i32, a: &mut Adapt) -> usize {
    if loc < 15 {
        let x = T.index_a[a.idx as usize].get(b);
        a.d1 += INDEX1_DELTA[a.delta as usize][x];
        a.d2 += INDEX1_DELTA[a.delta2 as usize][x];
        x
    } else if loc == 15 {
        T.index_b.get(b)
    } else {
        b.u(1) as usize
    }
}

/// DECODE_BLOCK (8.7.18.5): (run, level) pairs into `rl`, returning their count.
fn decode_block(
    b: &mut Bits,
    v: &mut BandVlc,
    chroma: bool,
    mut loc: i32,
    rl: &mut [(i32, i32); 16],
) -> Result<usize, Error> {
    let c = chroma as usize;
    let fa = &mut v.first[c];
    let fi = T.first_index[fa.idx as usize].get(b);
    fa.d1 += FIRST_INDEX_DELTA[fa.delta as usize][fi];
    fa.d2 += FIRST_INDEX_DELTA[fa.delta2 as usize][fi];
    let sign = b.bit();
    let sr = fi & 1;
    let mut srn = fi >> 2;
    let mut ctx = sr & srn;
    let mut lvl = if fi & 2 != 0 {
        abs_level(b, &mut v.abs[ctx])
    } else {
        1
    };
    if sign {
        lvl = -lvl;
    }
    let run = if sr == 0 { decode_run(b, 15 - loc)? } else { 0 };
    loc += run + 1;
    rl[0] = (run, lvl);
    let mut n = 1;
    while srn != 0 {
        if n >= 16 || loc > 15 {
            return Err(malformed("too many coefficients in a block"));
        }
        let sr = srn & 1;
        let run = if sr == 0 { decode_run(b, 15 - loc)? } else { 0 };
        loc += run + 1;
        let idx = decode_index(b, loc, &mut v.ind[c][ctx]);
        srn = idx >> 1;
        ctx &= srn;
        let sign = b.bit();
        let mut lvl = if idx & 1 != 0 {
            abs_level(b, &mut v.abs[ctx])
        } else {
            1
        };
        if sign {
            lvl = -lvl;
        }
        rl[n] = (run, lvl);
        n += 1;
    }
    if loc > 16 {
        return Err(malformed("coefficient past the block"));
    }
    Ok(n)
}

#[inline]
fn refine(b: &mut Bits, c: i32, bits: u32) -> i32 {
    let r = b.u(bits) as i32;
    if c > 0 {
        (c << bits) + r
    } else if c < 0 {
        (c << bits) - r
    } else if r != 0 && b.bit() {
        -r
    } else {
        r
    }
}

fn refine_cbphp(b: &mut Bits, n: usize) -> u32 {
    match n {
        1 => 1 << b.u(2),
        2 => REF_CBPHP1_VAL[T.ref_cbphp1.get(b)],
        3 => 0xF ^ (1 << b.u(2)),
        4 => 0xF,
        _ => 0,
    }
}

fn decode_qp_index(b: &mut Bits, n: usize) -> u8 {
    const BITS: [u32; 17] = [0, 0, 1, 1, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4];
    if !b.bit() {
        return 0;
    }
    let v = b.u(BITS[n.min(16)]) as usize + 1;
    v.min(n - 1) as u8
}

/// QuantMap (9.8.4).
pub(crate) fn quant_map(qp: u8, scaled_shift: bool, scaled: bool) -> i32 {
    let qp = qp as i32;
    if qp == 0 {
        1
    } else if !scaled {
        if qp < 32 {
            (qp + 3) >> 2
        } else if qp < 48 {
            ((16 + qp % 16 + 1) >> 1) << ((qp >> 4) - 2)
        } else {
            (16 + qp % 16) << ((qp >> 4) - 3)
        }
    } else {
        let s = scaled_shift as i32;
        if qp < 16 {
            qp << s
        } else {
            (16 + qp % 16) << ((qp >> 4) - 1 + s)
        }
    }
}

#[derive(Clone, Copy, Default)]
struct MbInfo {
    dc_mode: u8,
    lp_idx: u8,
    hp_idx: u8,
    hp_mode: u8,
    hp_bits: [u8; 2],
}

/// Tile rectangle in macroblocks (half-open).
#[derive(Clone, Copy)]
pub(crate) struct TileRect {
    pub x0: usize,
    pub x1: usize,
    pub y0: usize,
    pub y1: usize,
}

impl TileRect {
    #[inline]
    fn adapt_at(&self, mbx: usize) -> bool {
        mbx == self.x1 - 1 || (mbx - self.x0).is_multiple_of(16)
    }
}

/// One image plane's coefficients and parsing state.
pub(crate) struct Plane {
    pub ph: PlaneHeader,
    pub mbw: usize,
    pub mbh: usize,
    /// Coefficient / sample plane per component.
    pub comp: Vec<Vec<i32>>,
    /// Width of each component plane.
    pub cw: Vec<usize>,
    mb: Vec<MbInfo>,
    /// MBCBPHP per macroblock and component.
    cbp: Vec<u32>,
    tile_qp: Vec<TileQp>,
}

impl Plane {
    pub(crate) fn new(ph: PlaneHeader, ih: &ImageHeader) -> Result<Plane, Error> {
        let mbw = ih.mb_width() as usize;
        let mbh = ih.mb_height() as usize;
        let n = mbw * mbh;
        let mut p = Plane {
            mbw,
            mbh,
            comp: Vec::new(),
            cw: Vec::new(),
            mb: vec![MbInfo::default(); n],
            cbp: vec![0; n * ph.nc],
            tile_qp: vec![TileQp::default(); ih.num_tiles()],
            ph,
        };
        for i in 0..p.ph.nc {
            let (sx, sy) = p.sub(i);
            let w = (mbw * 16) >> sx;
            let h = (mbh * 16) >> sy;
            let mut v = Vec::new();
            v.try_reserve_exact(w * h).map_err(|_| Error::TooLarge)?;
            v.resize(w * h, 0);
            p.comp.push(v);
            p.cw.push(w);
        }
        Ok(p)
    }

    /// Chroma subsampling shifts of component `i`.
    pub(crate) fn sub(&self, i: usize) -> (u32, u32) {
        match (i, self.ph.clr) {
            (0, _) => (0, 0),
            (_, ClrFmt::Yuv420) => (1, 1),
            (_, ClrFmt::Yuv422) => (1, 0),
            _ => (0, 0),
        }
    }

    /// 4x4 blocks per macroblock of component `i`.
    pub(crate) fn nblk(&self, i: usize) -> usize {
        let (sx, sy) = self.sub(i);
        16 >> (sx + sy)
    }

    /// Offset of the top-left sample of block `blk` (raster within the macroblock).
    #[inline]
    pub(crate) fn blk(&self, i: usize, mbx: usize, mby: usize, blk: usize) -> usize {
        let (sx, sy) = self.sub(i);
        let bcols = 4 >> sx;
        (mby * (16 >> sy) + 4 * (blk / bcols)) * self.cw[i] + mbx * (16 >> sx) + 4 * (blk % bcols)
    }

    fn info(&mut self, mbx: usize, mby: usize) -> &mut MbInfo {
        &mut self.mb[mby * self.mbw + mbx]
    }

    fn dc_at(&self, i: usize, mbx: usize, mby: usize) -> i32 {
        self.comp[i][self.blk(i, mbx, mby, 0)]
    }

    fn mb_dc(&mut self, b: &mut Bits, c: &mut Ctx, t: &TileRect, mbx: usize, mby: usize) {
        let nc = self.ph.nc;
        let clr = self.ph.clr;
        let mut lap = [0i32; 2];
        let mut dc = [0i32; 16];
        match clr {
            ClrFmt::YOnly | ClrFmt::Yuvk | ClrFmt::NComp => {
                for (n, d) in dc.iter_mut().enumerate().take(nc) {
                    let abs = b.bit();
                    let m = (n > 0) as usize;
                    lap[m] += abs as i32;
                    *d = decode_dc(b, &mut c.dc_abs[0], c.dc_model.bits[m], abs);
                }
            }
            _ => {
                let v = T.val_dc_yuv.get(b);
                lap[0] += (v >> 2 & 1) as i32;
                dc[0] = decode_dc(b, &mut c.dc_abs[0], c.dc_model.bits[0], v & 4 != 0);
                lap[1] += (v >> 1 & 1) as i32;
                dc[1] = decode_dc(b, &mut c.dc_abs[1], c.dc_model.bits[1], v & 2 != 0);
                lap[1] += (v & 1) as i32;
                dc[2] = decode_dc(b, &mut c.dc_abs[1], c.dc_model.bits[1], v & 1 != 0);
            }
        }
        c.dc_model.update(lap, 0, clr, nc);
        if t.adapt_at(mbx) {
            c.dc_abs[0].adapt1();
            c.dc_abs[1].adapt1();
        }
        // DC prediction (9.6.1).
        let left = mbx == t.x0;
        let top = mby == t.y0;
        let mode = if left && top {
            3
        } else if left {
            1
        } else if top {
            0
        } else {
            let strength = |p: &Plane, i: usize| {
                let tl = p.dc_at(i, mbx - 1, mby - 1);
                (
                    (tl - p.dc_at(i, mbx - 1, mby)).abs(),
                    (tl - p.dc_at(i, mbx, mby - 1)).abs(),
                )
            };
            let (mut h, mut v) = strength(self, 0);
            if !matches!(clr, ClrFmt::YOnly | ClrFmt::NComp) {
                let scale = match clr {
                    ClrFmt::Yuv420 => 8,
                    ClrFmt::Yuv422 => 4,
                    _ => 2,
                };
                let (hu, vu) = strength(self, 1);
                let (hv, vv) = strength(self, 2);
                h = h * scale + hu + hv;
                v = v * scale + vu + vv;
            }
            if h * 4 < v {
                1
            } else if v * 4 < h {
                0
            } else {
                2
            }
        };
        let sub = matches!(clr, ClrFmt::Yuv420 | ClrFmt::Yuv422);
        for (i, &d) in dc.iter().enumerate().take(nc) {
            let pred = match mode {
                0 => self.dc_at(i, mbx - 1, mby),
                1 => self.dc_at(i, mbx, mby - 1),
                2 => {
                    let s = self.dc_at(i, mbx - 1, mby) + self.dc_at(i, mbx, mby - 1);
                    if i > 0 && sub { (s + 1) >> 1 } else { s >> 1 }
                }
                _ => 0,
            };
            let o = self.blk(i, mbx, mby, 0);
            self.comp[i][o] = d + pred;
        }
        self.info(mbx, mby).dc_mode = mode;
    }

    fn mb_lp(
        &mut self,
        b: &mut Bits,
        c: &mut Ctx,
        t: &TileRect,
        mbx: usize,
        mby: usize,
    ) -> Result<(), Error> {
        let nc = self.ph.nc;
        let clr = self.ph.clr;
        if (mbx - t.x0).is_multiple_of(16) {
            c.lp.scan[0].reset();
        }
        let sub = matches!(clr, ClrFmt::Yuv420 | ClrFmt::Yuv422);
        let full = if sub { 2 } else { nc };
        let cbp = if matches!(clr, ClrFmt::Yuv420 | ClrFmt::Yuv422 | ClrFmt::Yuv444) {
            let imax = 4 * full as i32 - 5;
            let v = if c.cbplp_zero <= 0 || c.cbplp_max < 0 {
                let s = if clr == ClrFmt::Yuv444 {
                    T.cbplp444.get(b)
                } else {
                    T.cbplp42x.get(b)
                };
                if c.cbplp_max < c.cbplp_zero {
                    imax - s as i32
                } else {
                    s as i32
                }
            } else {
                b.u(full as u32) as i32
            };
            c.cbplp_zero = (c.cbplp_zero + 1 - 4 * (v == 0) as i32).clamp(-8, 7);
            c.cbplp_max = (c.cbplp_max + 1 - 4 * (v == imax) as i32).clamp(-8, 7);
            v as u32
        } else {
            let mut v = 0;
            for n in 0..nc {
                v |= b.u(1) << n;
            }
            v
        };
        let mut lpin = [[0i32; 16]; 16];
        let mut lap = [0i32; 2];
        let mut rl = [(0i32, 0i32); 16];
        for n in 0..full {
            let idx = (n > 0) as usize;
            let mut nnz = 0;
            if cbp >> n & 1 != 0 {
                let loc = match (clr, n) {
                    (_, 0) => 1,
                    (ClrFmt::Yuv420, _) => 10,
                    (ClrFmt::Yuv422, _) => 2,
                    _ => 1,
                };
                nnz = decode_block(b, &mut c.lp, idx == 1, loc, &mut rl)?;
                if sub && n > 0 {
                    const REMAP: [usize; 7] = [4, 1, 2, 3, 5, 6, 7];
                    let (cnt, off) = if clr == ClrFmt::Yuv422 {
                        (14, 0)
                    } else {
                        (6, 1)
                    };
                    let mut tmp = [0i32; 14];
                    let mut i = 0usize;
                    for &(run, lvl) in &rl[..nnz] {
                        i += run as usize;
                        if i >= cnt {
                            return Err(malformed("chroma LP coefficient past the block"));
                        }
                        tmp[i] = lvl;
                        i += 1;
                    }
                    for (k, &v) in tmp.iter().enumerate().take(cnt) {
                        let r = REMAP[(k >> 1) + off];
                        let pos = if clr == ClrFmt::Yuv420 {
                            T420[r]
                        } else {
                            T422[r]
                        };
                        lpin[(k & 1) + 1][pos] = v;
                    }
                } else {
                    let mut i = 1usize;
                    for &(run, lvl) in &rl[..nnz] {
                        i += run as usize;
                        if i > 15 {
                            return Err(malformed("LP coefficient past the block"));
                        }
                        c.lp.scan[0].put(i, lvl, &mut lpin[n]);
                        i += 1;
                    }
                }
            }
            lap[idx] += nnz as i32;
            let mb = c.lp.model.bits[idx];
            if mb > 0 {
                let mb = mb as u32;
                match (clr, n) {
                    (ClrFmt::Yuv420, 1..) => {
                        for &k in &T420[1..] {
                            lpin[1][k] = refine(b, lpin[1][k], mb);
                            lpin[2][k] = refine(b, lpin[2][k], mb);
                        }
                    }
                    (ClrFmt::Yuv422, 1..) => {
                        for &k in &T422[1..] {
                            lpin[1][k] = refine(b, lpin[1][k], mb);
                            lpin[2][k] = refine(b, lpin[2][k], mb);
                        }
                    }
                    _ => {
                        for &k in &T444[1..] {
                            lpin[n][k] = refine(b, lpin[n][k], mb);
                        }
                    }
                }
            }
        }
        c.lp.model.update(lap, 1, clr, nc);
        if t.adapt_at(mbx) {
            c.lp.adapt();
        }
        // Store, then LP prediction (9.6.2) in the quantised domain.
        let info = *self.info(mbx, mby);
        // (The neighbour checks only matter when a missing DC packet left `dc_mode` unset.)
        let mode = if info.dc_mode == 0 && mbx > 0 && info.lp_idx == self.info(mbx - 1, mby).lp_idx
        {
            0
        } else if info.dc_mode == 1 && mby > 0 && info.lp_idx == self.info(mbx, mby - 1).lp_idx {
            1
        } else {
            2
        };
        for (i, lp) in lpin.iter().enumerate().take(nc) {
            let nb = self.nblk(i);
            let o: [usize; 16] =
                std::array::from_fn(|j| if j < nb { self.blk(i, mbx, mby, j) } else { 0 });
            let ol: [usize; 16] = if mode == 0 {
                std::array::from_fn(|j| {
                    if j < nb {
                        self.blk(i, mbx - 1, mby, j)
                    } else {
                        0
                    }
                })
            } else {
                [0; 16]
            };
            let ot: [usize; 16] = if mode == 1 {
                std::array::from_fn(|j| {
                    if j < nb {
                        self.blk(i, mbx, mby - 1, j)
                    } else {
                        0
                    }
                })
            } else {
                [0; 16]
            };
            let p = &mut self.comp[i];
            for j in 1..nb {
                p[o[j]] = lp[j];
            }
            match (nb, mode) {
                (16, 0) => {
                    for j in [4, 8, 12] {
                        p[o[j]] += p[ol[j]];
                    }
                }
                (16, 1) => {
                    for j in [1, 2, 3] {
                        p[o[j]] += p[ot[j]];
                    }
                }
                (4, 0) => p[o[2]] += p[ol[2]],
                (4, 1) => p[o[1]] += p[ot[1]],
                (8, 0) => {
                    for j in [4, 2, 6] {
                        p[o[j]] += p[ol[j]];
                    }
                }
                (8, 1) => {
                    p[o[4]] += p[ot[4]];
                    p[o[1]] += p[ot[5]];
                    p[o[5]] += p[o[1]];
                }
                (8, _) if info.dc_mode == 1 => p[o[5]] += p[o[1]],
                _ => {}
            }
        }
        Ok(())
    }

    fn mb_cbphp(&mut self, b: &mut Bits, c: &mut Ctx, t: &TileRect, mbx: usize, mby: usize) {
        const FLC: [u32; 6] = [0, 2, 1, 2, 2, 0];
        const OFF: [usize; 6] = [0, 4, 2, 8, 12, 1];
        const OUT: [u32; 16] = [0, 15, 3, 12, 1, 2, 4, 8, 5, 6, 9, 10, 7, 11, 13, 14];
        const SH: [u32; 4] = [0, 1, 4, 5];
        let nc = self.ph.nc;
        let clr = self.ph.clr;
        let five = matches!(clr, ClrFmt::YOnly | ClrFmt::Yuvk | ClrFmt::NComp);
        let ncomp = if matches!(clr, ClrFmt::Yuvk | ClrFmt::NComp) {
            nc
        } else {
            1
        };
        let mut diff = [0u32; 16];
        for i in 0..ncomp {
            let s = T.num_cbphp[c.num_cbphp.idx as usize].get(b);
            c.num_cbphp.d1 += NUM_CBPHP_DELTA[s];
            let cbp = refine_cbphp(b, s);
            for blk in 0..4 {
                if cbp >> blk & 1 == 0 {
                    continue;
                }
                let a = &mut c.num_blk_cbphp;
                let s = if five {
                    let s = T.num_cbphp[a.idx as usize].get(b);
                    a.d1 += NUM_CBPHP_DELTA[s];
                    s
                } else {
                    let s = T.num_blk_cbphp[a.idx as usize].get(b);
                    a.d1 += NUM_BLK_CBPHP_DELTA9[s];
                    s
                };
                let mut val = s + 1;
                let mut bc = 0u32;
                if val >= 6 {
                    bc = 0x10 * (T.three.get(b) as u32 + 1);
                    if val >= 9 {
                        val += T.three.get(b);
                    }
                    val -= 6;
                }
                let code = OFF[val] + b.u(FLC[val]) as usize;
                bc += OUT[code];
                match clr {
                    ClrFmt::Yuv444 => {
                        diff[0] |= (bc & 0xF) << (blk * 4);
                        for k in 0..2 {
                            if bc >> (k + 4) & 1 != 0 {
                                let n = T.num_ch_blk.get(b) + 1;
                                diff[k + 1] |= refine_cbphp(b, n) << (blk * 4);
                            }
                        }
                    }
                    ClrFmt::Yuv422 => {
                        diff[0] |= (bc & 0xF) << (blk * 4);
                        for k in 0..2 {
                            if bc >> (k + 4) & 1 != 0 {
                                let v = SH[T.three.get(b) + 1];
                                diff[k + 1] |= v << SH[blk];
                            }
                        }
                    }
                    ClrFmt::Yuv420 => {
                        diff[0] |= (bc & 0xF) << (blk * 4);
                        diff[1] |= (bc >> 4 & 1) << blk;
                        diff[2] |= (bc >> 5 & 1) << blk;
                    }
                    _ => diff[i] |= bc << (blk * 4),
                }
            }
        }
        // PredCBPHP (8.7.17.5).
        let left = mbx == t.x0;
        let top = mby == t.y0;
        let base = (mby * self.mbw + mbx) * nc;
        let nbr = |p: &Plane, i: usize, up_shift: u32, left_shift: u32| -> u32 {
            if left {
                if top {
                    1
                } else {
                    p.cbp[base - p.mbw * nc + i] >> up_shift & 1
                }
            } else {
                p.cbp[base - nc + i] >> left_shift & 1
            }
        };
        let full = if matches!(clr, ClrFmt::Yuv420 | ClrFmt::Yuv422) {
            1
        } else {
            nc
        };
        for (i, &d) in diff.iter().enumerate().take(full) {
            let c1 = (i > 0) as usize;
            let mut x = d;
            match c.cbp_state[c1] {
                0 => {
                    x ^= nbr(self, i, 10, 5);
                    x ^= 0x02 & (x << 1);
                    x ^= 0x10 & (x << 3);
                    x ^= 0x20 & (x << 1);
                    x ^= (x & 0x33) << 2;
                    x ^= (x & 0xCC) << 6;
                    x ^= (x & 0x3300) << 2;
                }
                2 => x ^= 0xFFFF,
                _ => {}
            }
            c.update_cbphp_model(c1, x.count_ones() as i32);
            self.cbp[base + i] = x;
        }
        if matches!(clr, ClrFmt::Yuv420 | ClrFmt::Yuv422) {
            let is422 = clr == ClrFmt::Yuv422;
            for i in 1..3 {
                let mut x = diff[i];
                match c.cbp_state[1] {
                    0 => {
                        x ^= nbr(self, i, if is422 { 6 } else { 2 }, 1);
                        if is422 {
                            x ^= (x & 0x01) << 1;
                            x ^= (x & 0x03) << 2;
                            x ^= (x & 0x0C) << 2;
                            x ^= (x & 0x30) << 2;
                        } else {
                            x ^= 0x02 & (x << 1);
                            x ^= (x & 0x3) << 2;
                        }
                    }
                    2 => x ^= if is422 { 0xFF } else { 0xF },
                    _ => {}
                }
                let n = x.count_ones() as i32 * if is422 { 2 } else { 4 };
                c.update_cbphp_model(1, n);
                self.cbp[base + i] = x;
            }
        }
    }

    /// CalcHPPredMode (9.6.3.2) from the macroblock's (predicted) LP coefficients.
    fn hp_mode(&self, mbx: usize, mby: usize) -> u8 {
        let clr = self.ph.clr;
        // From the quantised LP values, as the reference software does (T.832 9.6.3.2 uses
        // dequantised ones; the two differ when luma and chroma LP step sizes differ).
        let a =
            |i: usize, j: usize| -> i64 { (self.comp[i][self.blk(i, mbx, mby, j)] as i64).abs() };
        let mut h = a(0, 1) + a(0, 2) + a(0, 3);
        let mut v = a(0, 4) + a(0, 8) + a(0, 12);
        if !matches!(clr, ClrFmt::YOnly | ClrFmt::NComp) {
            for i in 1..3 {
                h += a(i, 1);
                match clr {
                    ClrFmt::Yuv420 => v += a(i, 2),
                    ClrFmt::Yuv422 => {
                        v += a(i, 2) + a(i, 6);
                        h += a(i, 5);
                    }
                    _ => v += a(i, 4),
                }
            }
        }
        if h * 4 < v {
            0
        } else if v * 4 < h {
            1
        } else {
            2
        }
    }

    /// MB_HP / MB_HP_FLEX; `flex` is the trim when flexbits are interleaved (spatial mode).
    #[allow(clippy::too_many_arguments)]
    fn mb_hp(
        &mut self,
        b: &mut Bits,
        c: &mut Ctx,
        t: &TileRect,
        mbx: usize,
        mby: usize,
        flex: Option<u32>,
    ) -> Result<(), Error> {
        let nc = self.ph.nc;
        let clr = self.ph.clr;
        if (mbx - t.x0).is_multiple_of(16) {
            c.hp.scan[0].reset();
            c.hp.scan[1].reset();
        }
        let mode = self.hp_mode(mbx, mby);
        let si = (mode == 1) as usize;
        let mut lap = [0i32; 2];
        let mut rl = [(0i32, 0i32); 16];
        let base = (mby * self.mbw + mbx) * nc;
        for i in 0..nc {
            let idx = (i > 0) as usize;
            let mbits = c.hp.model.bits[idx] as u32;
            let nb = self.nblk(i);
            let mut cbp = self.cbp[base + i];
            let cw = self.cw[i];
            for ib in 0..nb {
                let map = if nb == 16 { HIER[ib] } else { ib };
                let mut blk = [0i32; 16];
                let mut nnz = 0;
                if cbp & 1 != 0 {
                    nnz = decode_block(b, &mut c.hp, idx == 1, 1, &mut rl)?;
                    let mut k = 1usize;
                    for &(run, lvl) in &rl[..nnz] {
                        k += run as usize;
                        if k > 15 {
                            return Err(malformed("HP coefficient past the block"));
                        }
                        c.hp.scan[si].put(k, lvl, &mut blk);
                        k += 1;
                    }
                }
                let o = self.blk(i, mbx, mby, map);
                let p = &mut self.comp[i];
                for k in 1..16 {
                    p[o + (k >> 2) * cw + (k & 3)] = blk[k] << mbits;
                }
                if let Some(trim) = flex {
                    block_flex(b, p, o, cw, mbits, trim);
                }
                lap[idx] += nnz as i32;
                cbp >>= 1;
            }
        }
        let bits = c.hp.model.bits;
        let info = self.info(mbx, mby);
        info.hp_mode = mode;
        info.hp_bits = [bits[0] as u8, bits[1] as u8];
        c.hp.model.update(lap, 2, clr, nc);
        if t.adapt_at(mbx) {
            c.hp.adapt();
            c.num_cbphp.adapt1();
            c.num_blk_cbphp.adapt1();
        }
        Ok(())
    }

    fn mb_flexbits(&mut self, b: &mut Bits, mbx: usize, mby: usize, trim: u32) {
        let info = self.mb[mby * self.mbw + mbx];
        for i in 0..self.ph.nc {
            let mbits = info.hp_bits[(i > 0) as usize] as u32;
            let nb = self.nblk(i);
            let cw = self.cw[i];
            for ib in 0..nb {
                let map = if nb == 16 { HIER[ib] } else { ib };
                let o = self.blk(i, mbx, mby, map);
                block_flex(b, &mut self.comp[i], o, cw, mbits, trim);
            }
        }
    }

    /// Dequantisation (9.8) and HP prediction (9.6.3.3) of one macroblock.
    fn finish_mb(&mut self, tq: &TileQp, mbx: usize, mby: usize) {
        let info = self.mb[mby * self.mbw + mbx];
        let bands = self.ph.bands;
        let scaled = self.ph.scaled;
        let sub = matches!(self.ph.clr, ClrFmt::Yuv420 | ClrFmt::Yuv422);
        for i in 0..self.ph.nc {
            let q = |qp: &Qp, idx: u8| -> u8 {
                qp.qp
                    .get(i)
                    .and_then(|v| v.get(idx as usize).or(v.first()))
                    .copied()
                    .unwrap_or(0)
            };
            let nb = self.nblk(i);
            let o: [usize; 16] =
                std::array::from_fn(|j| if j < nb { self.blk(i, mbx, mby, j) } else { 0 });
            let cw = self.cw[i];
            let p = &mut self.comp[i];
            p[o[0]] *= quant_map(q(&tq.dc, 0), i == 0, scaled);
            if bands == Bands::DcOnly {
                continue;
            }
            let s = quant_map(q(&tq.lp, info.lp_idx), i == 0, scaled);
            for &oj in &o[1..nb] {
                p[oj] *= s;
            }
            if bands == Bands::NoHp {
                continue;
            }
            let s = quant_map(q(&tq.hp, info.hp_idx), true, scaled);
            for &ob in &o[..nb] {
                for k in 1..16 {
                    p[ob + (k >> 2) * cw + (k & 3)] *= s;
                }
            }
            // HP prediction; for 4:2:0 / 4:2:2 chroma the blocks form 2 columns.
            let (left, top): (&[usize], &[usize]) = match (nb, i > 0 && sub) {
                (16, _) => (
                    &[1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15],
                    &[4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
                ),
                (4, _) => (&[1, 3], &[2, 3]),
                _ => (&[1, 3, 5, 7], &[2, 4, 6, 3, 5, 7]),
            };
            let up = if nb == 16 { 4 } else { 2 };
            match info.hp_mode {
                0 => {
                    for &j in left {
                        for k in [4, 8, 12] {
                            let r = (k >> 2) * cw + (k & 3);
                            p[o[j] + r] += p[o[j - 1] + r];
                        }
                    }
                }
                1 => {
                    for &j in top {
                        for k in 1..4 {
                            p[o[j] + k] += p[o[j - up] + k];
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// BLOCK_FLEXBITS (8.7.19.2) on the block at `o`, whose HP values hold `vlc << mbits`.
fn block_flex(b: &mut Bits, p: &mut [i32], o: usize, cw: usize, mbits: u32, trim: u32) {
    let left = mbits.saturating_sub(trim);
    if left == 0 {
        return;
    }
    for &k in &T444[1..] {
        let at = o + (k >> 2) * cw + (k & 3);
        let r = b.u(left) as i32;
        let v = p[at];
        // The sign bit is only present for a new nonzero refinement (v == 0, r != 0).
        let f = if v > 0 {
            r
        } else if v < 0 || (r != 0 && b.bit()) {
            -r
        } else {
            r
        };
        p[at] += f << trim;
    }
}

/// A decoded codestream: headers plus the reconstructed-to-be coefficient planes.
pub(crate) struct Codestream {
    pub ih: ImageHeader,
    pub planes: Vec<Plane>,
    pub truncated: bool,
}

fn vlw_esc(d: &[u8], at: &mut usize) -> Result<u64, Error> {
    let get = |i: usize| {
        d.get(i)
            .copied()
            .ok_or_else(|| Error::Truncated("index table".into()))
    };
    let first = get(*at)?;
    *at += 1;
    Ok(match first {
        0..0xFB => {
            let v = (first as u64) << 8 | get(*at)? as u64;
            *at += 1;
            v
        }
        0xFB | 0xFC => {
            let n = if first == 0xFB { 4 } else { 8 };
            let mut v = 0u64;
            for _ in 0..n {
                v = v << 8 | get(*at)? as u64;
                *at += 1;
            }
            v
        }
        _ => 0,
    })
}

/// Headers only.
pub(crate) fn read_headers(
    data: &[u8],
) -> Result<(ImageHeader, PlaneHeader, Option<PlaneHeader>), Error> {
    let mut b = Bits::new(data);
    let ih = image_header(&mut b)?;
    let ph = plane_header(&mut b, &ih, false)?;
    let ah = if ih.alpha_plane {
        Some(plane_header(&mut b, &ih, true)?)
    } else {
        None
    };
    Ok((ih, ph, ah))
}

/// Parses a whole CODED_IMAGE into dequantised, predicted coefficient planes.
pub(crate) fn parse_codestream(
    data: &[u8],
    cancel: &dyn Fn() -> bool,
) -> Result<Codestream, Error> {
    let mut b = Bits::new(data);
    let ih = image_header(&mut b)?;
    let ph = plane_header(&mut b, &ih, false)?;
    let ah = if ih.alpha_plane {
        Some(plane_header(&mut b, &ih, true)?)
    } else {
        None
    };
    let mut at = (b.bit_pos() / 8) as usize;
    let ntiles = ih.num_tiles();
    let nb = if ih.frequency { ph.bands.count() } else { 1 };
    let mut index = Vec::new();
    if ih.index_table {
        let sc = data
            .get(at..at + 2)
            .ok_or_else(|| Error::Truncated("index table".into()))?;
        if sc != [0, 1] {
            return Err(malformed("index table start code"));
        }
        at += 2;
        let n = ntiles * nb;
        if n > data.len() {
            return Err(Error::Truncated("index table".into()));
        }
        for _ in 0..n {
            index.push(vlw_esc(data, &mut at)?);
        }
    } else if ih.frequency {
        return Err(Error::Unsupported(
            "frequency-mode codestream without an index table".into(),
        ));
    }
    let skip = vlw_esc(data, &mut at)?;
    let tiles_at = (at as u64).saturating_add(skip).min(data.len() as u64) as usize;
    let tiles = &data[tiles_at..];
    let mut planes = vec![Plane::new(ph, &ih)?];
    if let Some(ah) = ah {
        planes.push(Plane::new(ah, &ih)?);
    }
    let mut truncated = tiles_at as u64 != at as u64 + skip;
    let ncols = ih.tile_cols.len() - 1;
    let mut seq = 0usize;
    for ty in 0..ih.tile_rows.len() - 1 {
        for tx in 0..ncols {
            if cancel() {
                return Err(Error::Cancelled);
            }
            let t = TileRect {
                x0: ih.tile_cols[tx] as usize,
                x1: ih.tile_cols[tx + 1] as usize,
                y0: ih.tile_rows[ty] as usize,
                y1: ih.tile_rows[ty + 1] as usize,
            };
            let ti = ty * ncols + tx;
            let mut ctxs: Vec<Ctx> = planes.iter().map(|_| Ctx::new()).collect();
            if ih.frequency {
                for band in 0..nb {
                    let off = index[ti * nb + band];
                    let Some(pk) = packet(tiles, off) else {
                        truncated = true;
                        continue;
                    };
                    let mut b = Bits::new(pk);
                    start_code(&mut b)?;
                    match band {
                        0 => tile_dc_packet(&mut b, &mut planes, &mut ctxs, &t, ti),
                        1 => tile_lp_packet(&mut b, &mut planes, &mut ctxs, &t, ti)?,
                        2 => tile_hp_packet(&mut b, &mut planes, &mut ctxs, &t, ti)?,
                        _ => {
                            let trim = if ih.trim_flex { b.u(4) } else { 0 };
                            for mby in t.y0..t.y1 {
                                for mbx in t.x0..t.x1 {
                                    // TRIM_FLEXBITS covers every plane of the tile, as the
                                    // reference encoder writes it. (The reference decoder keeps
                                    // it in the primary plane's context only and reads interleaved
                                    // alpha flexbits untrimmed, so it misdecodes such files.)
                                    for p in planes.iter_mut() {
                                        if p.ph.bands == Bands::All {
                                            p.mb_flexbits(&mut b, mbx, mby, trim);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    truncated |= b.overrun();
                }
            } else {
                let start = if ih.index_table {
                    index[ti]
                } else {
                    seq as u64
                };
                let Some(pk) = packet(tiles, start) else {
                    truncated = true;
                    continue;
                };
                let mut b = Bits::new(pk);
                start_code(&mut b)?;
                tile_spatial(&mut b, &ih, &mut planes, &mut ctxs, &t, ti)?;
                b.align();
                truncated |= b.overrun();
                seq = start as usize + (b.bit_pos() / 8) as usize;
            }
            for p in planes.iter_mut() {
                let tq = std::mem::take(&mut p.tile_qp[ti]);
                for mby in t.y0..t.y1 {
                    for mbx in t.x0..t.x1 {
                        p.finish_mb(&tq, mbx, mby);
                    }
                }
                p.tile_qp[ti] = tq;
            }
        }
    }
    Ok(Codestream {
        ih,
        planes,
        truncated,
    })
}

fn packet(tiles: &[u8], off: u64) -> Option<&[u8]> {
    let off = usize::try_from(off).ok()?;
    if off.checked_add(4)? > tiles.len() {
        None
    } else {
        Some(&tiles[off..])
    }
}

fn start_code(b: &mut Bits) -> Result<(), Error> {
    if b.u(24) != 1 {
        return Err(malformed("tile start code"));
    }
    b.u(8);
    Ok(())
}

fn read_indices(
    b: &mut Bits,
    p: &mut Plane,
    tq: &TileQp,
    mbx: usize,
    mby: usize,
    lp: bool,
    hp: bool,
) {
    let bands = p.ph.bands;
    let info = p.info(mbx, mby);
    if lp && bands != Bands::DcOnly {
        let n = tq.lp.count();
        info.lp_idx = if n > 1 { decode_qp_index(b, n) } else { 0 };
    }
    if hp && !matches!(bands, Bands::DcOnly | Bands::NoHp) {
        let n = tq.hp.count();
        info.hp_idx = if tq.hp_from_lp {
            info.lp_idx
        } else if n > 1 {
            decode_qp_index(b, n)
        } else {
            0
        };
    }
}

fn tile_spatial(
    b: &mut Bits,
    ih: &ImageHeader,
    planes: &mut [Plane],
    ctxs: &mut [Ctx],
    t: &TileRect,
    ti: usize,
) -> Result<(), Error> {
    let trim = if ih.trim_flex { b.u(4) } else { 0 };
    // Tile headers go band by band, each with every plane's (not plane by plane).
    let mut tqs: Vec<TileQp> = planes.iter().map(|_| TileQp::default()).collect();
    for (p, tq) in planes.iter().zip(tqs.iter_mut()) {
        tile_dc(b, &p.ph, tq);
    }
    for (p, tq) in planes.iter().zip(tqs.iter_mut()) {
        if p.ph.bands != Bands::DcOnly {
            tile_lp(b, &p.ph, tq);
        }
    }
    for (p, tq) in planes.iter().zip(tqs.iter_mut()) {
        if !matches!(p.ph.bands, Bands::DcOnly | Bands::NoHp) {
            tile_hp(b, &p.ph, tq);
        }
    }
    for (p, tq) in planes.iter_mut().zip(tqs) {
        p.tile_qp[ti] = tq;
    }
    for mby in t.y0..t.y1 {
        for mbx in t.x0..t.x1 {
            for (p, c) in planes.iter_mut().zip(ctxs.iter_mut()) {
                let tq = std::mem::take(&mut p.tile_qp[ti]);
                read_indices(b, p, &tq, mbx, mby, true, true);
                p.mb_dc(b, c, t, mbx, mby);
                let r: Result<(), Error> = (|| {
                    if p.ph.bands != Bands::DcOnly {
                        p.mb_lp(b, c, t, mbx, mby)?;
                        if p.ph.bands != Bands::NoHp {
                            p.mb_cbphp(b, c, t, mbx, mby);
                            let flex = (p.ph.bands == Bands::All).then_some(trim);
                            p.mb_hp(b, c, t, mbx, mby, flex)?;
                        }
                    }
                    Ok(())
                })();
                p.tile_qp[ti] = tq;
                r?;
            }
        }
    }
    Ok(())
}

fn tile_dc_packet(b: &mut Bits, planes: &mut [Plane], ctxs: &mut [Ctx], t: &TileRect, ti: usize) {
    for p in planes.iter_mut() {
        let mut tq = TileQp::default();
        tile_dc(b, &p.ph, &mut tq);
        p.tile_qp[ti] = tq;
    }
    for mby in t.y0..t.y1 {
        for mbx in t.x0..t.x1 {
            for (p, c) in planes.iter_mut().zip(ctxs.iter_mut()) {
                p.mb_dc(b, c, t, mbx, mby);
            }
        }
    }
}

fn tile_lp_packet(
    b: &mut Bits,
    planes: &mut [Plane],
    ctxs: &mut [Ctx],
    t: &TileRect,
    ti: usize,
) -> Result<(), Error> {
    for p in planes.iter_mut() {
        if p.ph.bands != Bands::DcOnly {
            let mut tq = std::mem::take(&mut p.tile_qp[ti]);
            tile_lp(b, &p.ph, &mut tq);
            p.tile_qp[ti] = tq;
        }
    }
    for mby in t.y0..t.y1 {
        for mbx in t.x0..t.x1 {
            for (p, c) in planes.iter_mut().zip(ctxs.iter_mut()) {
                if p.ph.bands != Bands::DcOnly {
                    let tq = std::mem::take(&mut p.tile_qp[ti]);
                    read_indices(b, p, &tq, mbx, mby, true, false);
                    p.tile_qp[ti] = tq;
                    p.mb_lp(b, c, t, mbx, mby)?;
                }
            }
        }
    }
    Ok(())
}

fn tile_hp_packet(
    b: &mut Bits,
    planes: &mut [Plane],
    ctxs: &mut [Ctx],
    t: &TileRect,
    ti: usize,
) -> Result<(), Error> {
    for p in planes.iter_mut() {
        if !matches!(p.ph.bands, Bands::DcOnly | Bands::NoHp) {
            let mut tq = std::mem::take(&mut p.tile_qp[ti]);
            tile_hp(b, &p.ph, &mut tq);
            p.tile_qp[ti] = tq;
        }
    }
    for mby in t.y0..t.y1 {
        for mbx in t.x0..t.x1 {
            for (p, c) in planes.iter_mut().zip(ctxs.iter_mut()) {
                if !matches!(p.ph.bands, Bands::DcOnly | Bands::NoHp) {
                    let tq = std::mem::take(&mut p.tile_qp[ti]);
                    read_indices(b, p, &tq, mbx, mby, false, true);
                    p.mb_cbphp(b, c, t, mbx, mby);
                    let r = p.mb_hp(b, c, t, mbx, mby, None);
                    p.tile_qp[ti] = tq;
                    r?;
                }
            }
        }
    }
    Ok(())
}
