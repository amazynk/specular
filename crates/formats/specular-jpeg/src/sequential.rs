//! Single-scan sequential (baseline / extended Huffman, 8-bit) decoding: the hot path.
//!
//! Every strategy (single thread, restart-parallel, index-parallel, pipelined, region) is built
//! from one primitive: [`decode_rows`], which resumes from a [`Checkpoint`] and decodes a range
//! of MCU rows into a [`Sink`] — either straight to IDCT+store, or into coefficient buffers.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::cols::{ColCache, ColRec};
use crate::error::{JpegError, Result};
use crate::geom::{OutGeom, split_bands};
use crate::huffman::{BitReader, HuffTable, LUT_BITS, SKIP_BITS, skip_entry};
use crate::idct::{Dst, Simd, dc_only_value, idct_block};
use crate::marker::{Parsed, ZIGZAG};

/// Resumable decoder state at an MCU boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Checkpoint {
    pub pos: usize,
    pub buf: u64,
    pub bits: u32,
    pub marker_hit: bool,
    pub zero_fill: u32,
    pub dc: [i32; 4],
    /// MCUs left before the next restart marker is expected.
    pub restart_left: u32,
    pub eobrun: u32,
    /// MCUs (or units) to parse from this state before reaching the checkpoint's row start.
    pub skip: u32,
}

impl Checkpoint {
    pub fn at(pos: usize, ri: u32) -> Checkpoint {
        Checkpoint {
            pos,
            buf: 0,
            bits: 0,
            marker_hit: false,
            zero_fill: 0,
            dc: [0; 4],
            restart_left: ri,
            eobrun: 0,
            skip: 0,
        }
    }
}

pub(crate) struct DecState<'a> {
    pub br: BitReader<'a>,
    pub dc: [i32; 4],
    pub restart_left: u32,
    pub eobrun: u32,
}

impl<'a> DecState<'a> {
    pub fn from_cp(data: &'a [u8], cp: &Checkpoint) -> Self {
        DecState {
            br: BitReader {
                data,
                pos: cp.pos,
                buf: cp.buf,
                bits: cp.bits,
                marker_hit: cp.marker_hit,
                zero_fill: cp.zero_fill,
            },
            dc: cp.dc,
            restart_left: cp.restart_left,
            eobrun: cp.eobrun,
        }
    }
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.br.pos,
            buf: self.br.buf,
            bits: self.br.bits,
            marker_hit: self.br.marker_hit,
            zero_fill: self.br.zero_fill,
            dc: self.dc,
            restart_left: self.restart_left,
            eobrun: self.eobrun,
            skip: 0,
        }
    }
    /// Handle the restart-interval bookkeeping before an MCU.
    #[inline(always)]
    pub fn before_mcu(&mut self, ri: u32) {
        if ri != 0 {
            if self.restart_left == 0 {
                self.br.restart();
                self.dc = [0; 4];
                self.eobrun = 0;
                self.restart_left = ri;
            }
            self.restart_left -= 1;
        }
    }
}

pub(crate) struct ScanCompRef<'a> {
    pub ci: usize,
    pub h: u32,
    pub v: u32,
    pub dc: &'a HuffTable,
    pub ac: &'a HuffTable,
    pub q: &'a [u16; 64],
}

pub(crate) struct SeqScan<'a> {
    pub data: &'a [u8],
    pub comps: Vec<ScanCompRef<'a>>,
    pub mcus_x: u32,
    pub mcus_y: u32,
    pub ri: u32,
    pub start: usize,
}

impl<'a> SeqScan<'a> {
    pub fn new(p: &'a Parsed, data: &'a [u8]) -> Result<SeqScan<'a>> {
        let s = &p.scans[0];
        let mut comps = Vec::new();
        for sc in &s.comps {
            let fc = &p.frame.comps[sc.ci];
            comps.push(ScanCompRef {
                ci: sc.ci,
                h: fc.h,
                v: fc.v,
                dc: s.dc[sc.td]
                    .as_deref()
                    .ok_or(JpegError::Malformed("missing DC table"))?,
                ac: s.ac[sc.ta]
                    .as_deref()
                    .ok_or(JpegError::Malformed("missing AC table"))?,
                q: &p.quant[sc.ci],
            });
        }
        Ok(SeqScan {
            data,
            comps,
            mcus_x: p.frame.mcus_x,
            mcus_y: p.frame.mcus_y,
            ri: s.restart_interval,
            start: s.data_start,
        })
    }
    pub fn start_cp(&self) -> Checkpoint {
        Checkpoint::at(self.start, self.ri)
    }
}

// ---------------------------------------------------------------------------------------------
// Block decoding

/// Decode one block into `coefs` (natural order, must be zero on entry). Returns true if any AC
/// coefficient was coded.
#[inline(always)]
pub(crate) fn decode_block(
    br: &mut BitReader,
    dct: &HuffTable,
    act: &HuffTable,
    pred: &mut i32,
    coefs: &mut [i16; 64],
) -> bool {
    if br.bits < 32 {
        br.refill();
    }
    *pred = pred.wrapping_add(br.decode_dc_diff(dct));
    coefs[0] = *pred as i16;
    let lut = &act.lut;
    let mut k = 1usize;
    let mut has_ac = false;
    while k < 64 {
        if br.bits < 32 {
            br.refill();
        }
        let e = lut[br.peek(LUT_BITS) as usize];
        if e >> 16 != 0 {
            br.consume(e & 31);
            k += ((e >> 12) & 15) as usize;
            coefs[ZIGZAG[k] as usize] = (e >> 16) as i16;
            k += 1;
            has_ac = true;
            continue;
        }
        let (sym, len) = if e != 0 {
            ((e >> 8) & 0xFF, e & 31)
        } else {
            act.decode_slow(br.peek(16))
        };
        br.consume(len);
        let r = (sym >> 4) as usize;
        let s = sym & 15;
        if s == 0 {
            if r != 15 {
                break;
            }
            k += 16;
            continue;
        }
        k += r;
        br.ensure(s);
        coefs[ZIGZAG[k] as usize] = br.get_extend(s) as i16;
        k += 1;
        has_ac = true;
    }
    has_ac
}

/// Parse one block, keeping only the DC prediction up to date (no coefficient storage).
#[inline(always)]
pub(crate) fn skip_block(br: &mut BitReader, dct: &HuffTable, act: &HuffTable, pred: &mut i32) {
    if br.bits < 32 {
        br.refill();
    }
    *pred = pred.wrapping_add(br.decode_dc_diff(dct));
    let lut = &act.lut;
    let skip = &act.skip[..1 << SKIP_BITS];
    let mut k = 1u32;
    while k < 64 {
        if br.bits < 32 {
            br.refill();
        }
        // Several complete symbols at once when they all lie inside this block.
        let (bits, adv, lim, eob) = skip_entry(skip[br.peek(SKIP_BITS) as usize]);
        if bits != 0 && k + lim < 64 {
            br.consume(bits);
            k += adv;
            if eob {
                break;
            }
            continue;
        }
        let e = lut[br.peek(LUT_BITS) as usize];
        if e >> 16 != 0 {
            br.consume(e & 31);
            k += ((e >> 12) & 15) + 1;
            continue;
        }
        let (sym, len) = if e != 0 {
            ((e >> 8) & 0xFF, e & 31)
        } else {
            act.decode_slow(br.peek(16))
        };
        br.consume(len);
        let r = sym >> 4;
        let s = sym & 15;
        if s == 0 {
            if r != 15 {
                break;
            }
            k += 16;
            continue;
        }
        k += r + 1;
        br.ensure(s);
        br.consume(s);
    }
}

// ---------------------------------------------------------------------------------------------
// Sinks

pub(crate) trait Sink {
    /// Whether full coefficients are needed; if false only the DC value is provided.
    const NEED_AC: bool;
    /// `slot` = component position in the scan; (bx, by) block coordinates local to the band.
    fn block(&mut self, slot: usize, bx: usize, by: usize, coefs: &[i16; 64], has_ac: bool);
}

/// Parse only.
pub(crate) struct NullSink;
impl Sink for NullSink {
    const NEED_AC: bool = false;
    #[inline(always)]
    fn block(&mut self, _: usize, _: usize, _: usize, _: &[i16; 64], _: bool) {}
}

/// IDCT straight into the output planes.
pub(crate) struct IdctSink<'a, 'b> {
    pub out: [&'b mut [u8]; 4],
    pub stride: [usize; 4],
    pub q: [&'a [u16; 64]; 4],
    /// Output block size per slot.
    pub n: [usize; 4],
    pub simd: Simd,
}

impl Sink for IdctSink<'_, '_> {
    const NEED_AC: bool = true;
    #[inline(always)]
    fn block(&mut self, slot: usize, bx: usize, by: usize, coefs: &[i16; 64], has_ac: bool) {
        let n = self.n[slot];
        let stride = self.stride[slot];
        let off = by * n * stride + bx * n;
        idct_block(
            self.simd,
            n,
            coefs,
            self.q[slot],
            has_ac,
            Dst {
                buf: self.out[slot],
                off,
                stride,
            },
        );
    }
}

/// 1/8 scale: one output sample per block from the DC coefficient.
pub(crate) struct DcSink<'a, 'b> {
    pub out: [&'b mut [u8]; 4],
    pub stride: [usize; 4],
    pub q0: [u16; 4],
    pub _p: std::marker::PhantomData<&'a ()>,
}

impl Sink for DcSink<'_, '_> {
    const NEED_AC: bool = false;
    #[inline(always)]
    fn block(&mut self, slot: usize, bx: usize, by: usize, coefs: &[i16; 64], _: bool) {
        let v = dc_only_value(coefs[0], self.q0[slot]);
        let stride = self.stride[slot];
        self.out[slot][by * stride + bx] = v;
    }
}

/// Store coefficients for a later (parallel) IDCT.
pub(crate) struct CoefSink<'b> {
    pub coefs: [&'b mut [i16]; 4],
    pub flags: [&'b mut [u8]; 4],
    /// Blocks per row in the band buffer.
    pub bw: [usize; 4],
}

impl Sink for CoefSink<'_> {
    const NEED_AC: bool = true;
    #[inline(always)]
    fn block(&mut self, slot: usize, bx: usize, by: usize, coefs: &[i16; 64], has_ac: bool) {
        let b = by * self.bw[slot] + bx;
        self.coefs[slot][b * 64..b * 64 + 64].copy_from_slice(coefs);
        self.flags[slot][b] = has_ac as u8;
    }
}

// ---------------------------------------------------------------------------------------------
// Row decoding primitive

pub(crate) struct RowJob<'c> {
    /// MCU rows to decode (output rows are local to `row_base`).
    pub rows: Range<u32>,
    pub row_base: u32,
    /// MCU columns to output; others are only parsed.
    pub cols: Range<u32>,
    /// Row-start checkpoints that allow jumping to the next row once `cols.end` is reached.
    pub jump: Option<&'c [Checkpoint]>,
    /// Column checkpoints to start rows from and to record into (see [`crate::cols`]).
    pub learn: Option<&'c ColCache>,
    pub cancel: Option<&'c AtomicBool>,
}

/// Outcome of a row decode: rows completed and whether data ran out.
pub(crate) struct RowsDone {
    pub rows: u32,
    pub truncated: bool,
}

#[inline(always)]
fn decode_mcu<S: Sink>(
    sc: &SeqScan,
    st: &mut DecState,
    sink: &mut S,
    bx0: usize,
    by0: usize,
    scratch: &mut [i16; 64],
) {
    for (slot, c) in sc.comps.iter().enumerate() {
        for bv in 0..c.v as usize {
            for bh in 0..c.h as usize {
                let bx = bx0 * c.h as usize + bh;
                let by = by0 * c.v as usize + bv;
                if S::NEED_AC {
                    let has_ac = decode_block(&mut st.br, c.dc, c.ac, &mut st.dc[slot], scratch);
                    sink.block(slot, bx, by, scratch, has_ac);
                    if has_ac {
                        *scratch = [0; 64];
                    }
                } else {
                    skip_block(&mut st.br, c.dc, c.ac, &mut st.dc[slot]);
                    scratch[0] = st.dc[slot] as i16;
                    sink.block(slot, bx, by, scratch, false);
                }
            }
        }
    }
    scratch[0] = 0;
}

#[inline(always)]
fn skip_mcu(sc: &SeqScan, st: &mut DecState) {
    for (slot, c) in sc.comps.iter().enumerate() {
        for _ in 0..c.v * c.h {
            skip_block(&mut st.br, c.dc, c.ac, &mut st.dc[slot]);
        }
    }
}

#[inline(always)]
pub(crate) fn skip_mcus<'a>(sc: &SeqScan<'a>, st: &mut DecState<'a>, n: u32) {
    for _ in 0..n {
        st.before_mcu(sc.ri);
        skip_mcu(sc, st);
    }
}

/// Decode `job.rows` starting from `st` (which must be at the start of `job.rows.start`).
/// Optionally records a checkpoint at the start of every row into `index`.
#[inline(always)]
pub(crate) fn decode_rows<'a, S: Sink>(
    sc: &SeqScan<'a>,
    st: &mut DecState<'a>,
    job: &RowJob,
    sink: &mut S,
    mut index: Option<&mut Vec<Checkpoint>>,
) -> Result<RowsDone> {
    let mut scratch = [0i16; 64];
    let cols = job.cols.clone();
    let learn = job.learn.filter(|_| index.is_none());
    let mut rec = learn.map(|c| ColRec::new(c.step(0), 0));
    for r in job.rows.clone() {
        if let Some(ix) = index.as_deref_mut() {
            ix.push(st.checkpoint());
        }
        if let Some(c) = job.cancel
            && c.load(Ordering::Relaxed)
        {
            return Err(JpegError::Cancelled);
        }
        let by0 = (r - job.row_base) as usize;
        let mut mx = 0;
        // Start at the last recorded column before the region (the state at the row start
        // is then not needed: rows after this one jump to their own checkpoints or continue
        // from this row's end, which is the same state).
        if let Some(c) = learn
            && let Some((u, cp)) = c.best(0, r, cols.start)
        {
            *st = DecState::from_cp(sc.data, &cp);
            mx = u;
        }
        if let Some(rc) = rec.as_mut() {
            rc.reset(mx);
        }
        while mx < sc.mcus_x {
            if let Some(rc) = rec.as_mut() {
                rc.at(mx, || st.checkpoint());
            }
            st.before_mcu(sc.ri);
            if mx >= cols.start && mx < cols.end {
                decode_mcu(sc, st, sink, (mx - cols.start) as usize, by0, &mut scratch);
                if mx + 1 == cols.end
                    && mx + 1 < sc.mcus_x
                    && let Some(j) = job.jump
                    && let Some(cp) = j.get(r as usize + 1)
                    && cp.skip == 0
                {
                    *st = DecState::from_cp(sc.data, cp);
                    break;
                }
            } else {
                skip_mcu(sc, st);
            }
            mx += 1;
        }
        if let (Some(rc), Some(c)) = (rec.as_mut(), learn) {
            rc.flush(c, 0, r);
        }
        tracing::trace!(target: "jpeg", row = r, pos = st.br.pos, "row");
        if st.br.overrun() {
            tracing::debug!(target: "jpeg", row = r, offset = st.br.pos, "entropy data ends early");
            return Ok(RowsDone {
                rows: r + 1 - job.rows.start,
                truncated: true,
            });
        }
    }
    Ok(RowsDone {
        rows: job.rows.len() as u32,
        truncated: false,
    })
}

// ---------------------------------------------------------------------------------------------
// Restart marker scanning

/// Positions just after every RSTn marker of the scan beginning at `start`, in order, until the
/// scan's terminating marker. Scans in parallel chunks.
pub(crate) fn find_restarts(data: &[u8], start: usize, parallel: bool) -> Vec<usize> {
    let body = &data[start.min(data.len())..];
    const CHUNK: usize = 1 << 20;
    let scan_chunk = |ci: usize| -> (Vec<usize>, Option<usize>) {
        let lo = ci * CHUNK;
        let hi = (lo + CHUNK).min(body.len());
        let mut v = Vec::new();
        let mut p = lo;
        while p < hi {
            // SWAR skip to the next 0xFF.
            while p + 8 <= hi {
                let w = u64::from_le_bytes(body[p..p + 8].try_into().unwrap());
                if crate::marker::has_ff(w) {
                    break;
                }
                p += 8;
            }
            while p < hi && body[p] != 0xFF {
                p += 1;
            }
            if p >= hi {
                break;
            }
            match body.get(p + 1) {
                None => return (v, Some(start + p)),
                Some(&m) => {
                    if (0xD0..=0xD7).contains(&m) {
                        v.push(start + p + 2);
                    } else if m != 0 && m != 0xFF {
                        return (v, Some(start + p));
                    }
                }
            }
            p += 1;
        }
        (v, None)
    };
    let nchunks = body.len().div_ceil(CHUNK).max(1);
    let parts: Vec<(Vec<usize>, Option<usize>)> = if parallel && nchunks > 1 {
        (0..nchunks).into_par_iter().map(scan_chunk).collect()
    } else {
        (0..nchunks).map(scan_chunk).collect()
    };
    let mut out = Vec::new();
    for (v, end) in parts {
        out.extend(v);
        if end.is_some() {
            break;
        }
    }
    out
}

/// Row-start checkpoints derived from restart marker positions (no Huffman decoding needed).
pub(crate) fn restart_checkpoints(sc: &SeqScan, restarts: &[usize]) -> Vec<Checkpoint> {
    let ri = sc.ri as u64;
    (0..sc.mcus_y)
        .map(|r| {
            let mcu = r as u64 * sc.mcus_x as u64;
            let seg = ((mcu / ri) as usize).min(restarts.len());
            let pos = if seg == 0 {
                sc.start
            } else {
                restarts[seg - 1]
            };
            let mut cp = Checkpoint::at(pos, sc.ri);
            cp.skip = (mcu - seg as u64 * ri) as u32;
            cp
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Strategies

pub(crate) struct SeqOutcome {
    /// If the data ran out: number of region MCU rows that were (at least partially) decoded.
    pub truncated_at: Option<u32>,
    /// Row-start checkpoints for every MCU row, if requested and complete.
    pub index: Option<Vec<Checkpoint>>,
}

#[derive(Clone, Copy)]
pub(crate) struct SeqParams<'c> {
    pub simd: Simd,
    pub threads: usize,
    pub cancel: Option<&'c AtomicBool>,
    pub index: Option<&'c [Checkpoint]>,
    pub want_index: bool,
    /// Column checkpoints of the index (`index` is from a [`crate::RowIndex`]).
    pub cols: Option<&'c ColCache>,
}

impl<'c> RowJob<'c> {
    /// Output job for MCU rows `rows` (output rows local to `rows.start`) of region `g`.
    fn output(
        g: &OutGeom,
        p: &SeqParams<'c>,
        rows: Range<u32>,
        jump: Option<&'c [Checkpoint]>,
    ) -> Self {
        RowJob {
            row_base: rows.start,
            rows,
            cols: g.mx0..g.mx1,
            jump,
            learn: jump.and(p.cols),
            cancel: p.cancel,
        }
    }
}

/// Decode `job`'s rows from `st` into one band of output slices (one per frame component, in
/// frame order).
fn run_rows<'a>(
    sc: &SeqScan<'a>,
    g: &OutGeom,
    simd: Simd,
    out: Vec<&mut [u8]>,
    st: &mut DecState<'a>,
    job: &RowJob,
    record: Option<&mut Vec<Checkpoint>>,
) -> Result<RowsDone> {
    let mut stride = [0usize; 4];
    let mut q = [sc.comps[0].q; 4];
    let mut q0 = [1u16; 4];
    let mut nb = [1usize; 4];
    let mut slots: Vec<Option<&mut [u8]>> = out.into_iter().map(Some).collect();
    let mut outs: [&mut [u8]; 4] = Default::default();
    for (slot, c) in sc.comps.iter().enumerate() {
        stride[slot] = g.comps[c.ci].stride;
        nb[slot] = g.comps[c.ci].n;
        q[slot] = c.q;
        q0[slot] = c.q[0];
        outs[slot] = slots[c.ci].take().unwrap_or_default();
    }
    if g.n == 1 {
        let mut sink = DcSink {
            out: outs,
            stride,
            q0,
            _p: std::marker::PhantomData,
        };
        decode_rows(sc, st, job, &mut sink, record)
    } else {
        let mut sink = IdctSink {
            out: outs,
            stride,
            q,
            n: nb,
            simd,
        };
        decode_rows(sc, st, job, &mut sink, record)
    }
}

/// Parse rows without output (to reach a region or to build an index).
fn parse_rows<'a>(
    sc: &SeqScan<'a>,
    p: &SeqParams,
    st: &mut DecState<'a>,
    rows: Range<u32>,
    record: Option<&mut Vec<Checkpoint>>,
) -> Result<RowsDone> {
    let job = RowJob {
        rows: rows.clone(),
        row_base: rows.start,
        cols: 0..0,
        jump: None,
        learn: None,
        cancel: p.cancel,
    };
    decode_rows(sc, st, &job, &mut NullSink, record)
}

/// Build a full row index by parsing the whole scan (no output).
pub(crate) fn build_index(sc: &SeqScan, p: &SeqParams) -> Result<Vec<Checkpoint>> {
    if sc.ri > 0 {
        let rs = find_restarts(sc.data, sc.start, p.threads > 1);
        return Ok(restart_checkpoints(sc, &rs));
    }
    if p.threads > 1
        && let Some(ix) = crate::spec::row_checkpoints(sc, p.threads * 4, true, p.cancel)
    {
        return Ok(ix);
    }
    tracing::debug!(target: "jpeg", threads = p.threads, "index: sequential parse");
    let mut ix = Vec::with_capacity(sc.mcus_y as usize);
    let mut st = DecState::from_cp(sc.data, &sc.start_cp());
    parse_rows(sc, p, &mut st, 0..sc.mcus_y, Some(&mut ix))?;
    Ok(ix)
}

fn split_rows(start: u32, nrows: u32, nbands: usize) -> (Vec<u32>, Vec<u32>) {
    let nbands = (nbands as u32).clamp(1, nrows.max(1));
    let base = nrows / nbands;
    let extra = nrows % nbands;
    let rows: Vec<u32> = (0..nbands)
        .map(|i| base + (i < extra) as u32)
        .filter(|&r| r > 0)
        .collect();
    let mut starts = Vec::with_capacity(rows.len());
    let mut acc = start;
    for &r in &rows {
        starts.push(acc);
        acc += r;
    }
    (rows, starts)
}

/// Minimum region size (in MCUs) for speculative parallel entropy decoding of restart-free scans.
const SPECULATIVE_MIN_MCUS: u64 = 4096;

pub(crate) fn decode<'a>(
    sc: &SeqScan<'a>,
    g: &OutGeom,
    planes: &mut [Vec<u8>],
    p: &SeqParams,
) -> Result<SeqOutcome> {
    let nrows = g.my1 - g.my0;

    // Row-start checkpoints available without full Huffman decoding?
    let synthetic: Option<Vec<Checkpoint>>;
    let cps: Option<&[Checkpoint]> = if let Some(ix) = p.index {
        tracing::debug!(target: "jpeg", threads = p.threads, rows = ?(g.my0..g.my1), "sequential: from the row index");
        Some(ix)
    } else if sc.ri > 0 && (p.threads > 1 || g.my0 > 0 || p.want_index) {
        let rs = find_restarts(sc.data, sc.start, p.threads > 1);
        tracing::debug!(
            target: "jpeg",
            threads = p.threads,
            restart_interval = sc.ri,
            markers = rs.len(),
            "sequential: restart-parallel (row starts from RST markers)"
        );
        synthetic = Some(restart_checkpoints(sc, &rs));
        synthetic.as_deref()
    } else if sc.ri == 0
        && p.threads > 1
        && (nrows as u64 * sc.mcus_x as u64) >= SPECULATIVE_MIN_MCUS
    {
        // No restart markers: find row starts with speculative parallel parsing.
        synthetic = crate::spec::row_checkpoints(sc, p.threads * 4, true, p.cancel);
        if synthetic.is_none() {
            tracing::debug!(target: "jpeg", "sequential: speculative row search failed, decoding from the start");
        }
        synthetic.as_deref()
    } else {
        None
    };

    if let Some(cps) = cps {
        if cps.len() != sc.mcus_y as usize {
            return Err(JpegError::IndexMismatch);
        }
        let threads = p.threads.max(1);
        let nbands = if threads == 1 { 1 } else { threads * 4 };
        let (band_rows, starts) = split_rows(g.my0, nrows, nbands);
        let bands = split_bands(planes, g, &band_rows);
        let run = |(out, (s, r)): (Vec<&mut [u8]>, (u32, u32))| -> Result<RowsDone> {
            let cp = &cps[s as usize];
            let mut st = DecState::from_cp(sc.data, cp);
            skip_mcus(sc, &mut st, cp.skip);
            let job = RowJob::output(g, p, s..s + r, Some(cps));
            run_rows(sc, g, p.simd, out, &mut st, &job, None)
        };
        let jobs: Vec<_> = bands
            .into_iter()
            .zip(starts.iter().copied().zip(band_rows.iter().copied()))
            .collect();
        let results: Vec<Result<RowsDone>> = if threads > 1 && jobs.len() > 1 {
            jobs.into_par_iter().map(run).collect()
        } else {
            jobs.into_iter().map(run).collect()
        };
        let mut truncated_at = None;
        for (i, res) in results.into_iter().enumerate() {
            let d = res?;
            if d.truncated && truncated_at.is_none() {
                truncated_at = Some(starts[i] - g.my0 + d.rows);
            }
        }
        let index = if p.want_index {
            Some(cps.to_vec())
        } else {
            None
        };
        return Ok(SeqOutcome {
            truncated_at,
            index,
        });
    }

    // Sequential entropy decoding from the start of the scan.
    let mut index: Option<Vec<Checkpoint>> = if p.want_index {
        Some(Vec::with_capacity(sc.mcus_y as usize))
    } else {
        None
    };
    let mut st = DecState::from_cp(sc.data, &sc.start_cp());
    if g.my0 > 0 {
        let d = parse_rows(sc, p, &mut st, 0..g.my0, index.as_mut())?;
        if d.truncated {
            return Ok(SeqOutcome {
                truncated_at: Some(0),
                index: None,
            });
        }
    }
    let pipelined = p.threads > 1 && g.n > 1 && nrows >= 4;
    tracing::debug!(
        target: "jpeg",
        threads = p.threads,
        pipelined,
        skipped_rows = g.my0,
        "sequential: single entropy pass{}",
        if pipelined { ", IDCT pipelined on the pool" } else { "" }
    );
    let done = if pipelined {
        decode_pipelined(sc, g, planes, p, &mut st, index.as_mut())?
    } else {
        let out: Vec<&mut [u8]> = planes.iter_mut().map(|v| v.as_mut_slice()).collect();
        let job = RowJob::output(g, p, g.my0..g.my1, None);
        run_rows(sc, g, p.simd, out, &mut st, &job, index.as_mut())?
    };
    let truncated_at = if done.truncated {
        Some(done.rows)
    } else {
        None
    };
    if let Some(ix) = index.as_mut()
        && !done.truncated
        && g.my1 < sc.mcus_y
    {
        parse_rows(sc, p, &mut st, g.my1..sc.mcus_y, Some(ix))?;
    }
    if index
        .as_ref()
        .is_some_and(|ix| ix.len() != sc.mcus_y as usize)
    {
        index = None;
    }
    Ok(SeqOutcome {
        truncated_at,
        index,
    })
}

/// Fill rows from `from_row` (MCU rows relative to the region) to the end with mid-grey.
pub(crate) fn fill_grey_from(planes: &mut [Vec<u8>], g: &OutGeom, from_row: u32) {
    for (ci, pl) in planes.iter_mut().enumerate() {
        let start = (from_row as usize * g.mcu_row_bytes(ci)).min(pl.len());
        pl[start..].fill(128);
    }
}

struct CoefBand {
    coefs: Vec<Vec<i16>>,
    flags: Vec<Vec<u8>>,
}

/// Entropy decode on this thread, IDCT+store bands on the rayon pool.
fn decode_pipelined<'a>(
    sc: &SeqScan<'a>,
    g: &OutGeom,
    planes: &mut [Vec<u8>],
    p: &SeqParams,
    st: &mut DecState<'a>,
    mut index: Option<&mut Vec<Checkpoint>>,
) -> Result<RowsDone> {
    let ncols = (g.mx1 - g.mx0) as usize;
    let nrows = g.my1 - g.my0;
    let blocks_per_row: usize = sc.comps.iter().map(|c| (c.h * c.v) as usize * ncols).sum();
    // ~256 KB of coefficients per band (L2 resident), at least one MCU row.
    let band = ((256 * 1024 / (blocks_per_row * 128).max(1)) as u32)
        .clamp(1, 64)
        .min(nrows.div_ceil(p.threads as u32 * 2).max(1));
    let band_rows: Vec<u32> = (0..nrows.div_ceil(band))
        .map(|i| band.min(nrows - i * band))
        .collect();
    let bands = split_bands(planes, g, &band_rows);
    let nslots = sc.comps.len();
    let bw: Vec<usize> = sc.comps.iter().map(|c| c.h as usize * ncols).collect();
    let bh: Vec<usize> = sc
        .comps
        .iter()
        .map(|c| c.v as usize * band as usize)
        .collect();
    let cv: Vec<usize> = sc.comps.iter().map(|c| c.v as usize).collect();
    let max_buffers = p.threads + 2;
    let free: std::sync::Mutex<Vec<CoefBand>> = std::sync::Mutex::new(Vec::new());
    let free = &free;
    let mut allocated = 0usize;
    let mut result: Result<RowsDone> = Ok(RowsDone {
        rows: nrows,
        truncated: false,
    });
    let simd = p.simd;
    let ns: Vec<usize> = sc.comps.iter().map(|c| g.comps[c.ci].n).collect();
    let ns = &ns;
    let comp_q: Vec<&[u16; 64]> = sc.comps.iter().map(|c| c.q).collect();
    let comp_ci: Vec<usize> = sc.comps.iter().map(|c| c.ci).collect();
    let strides: Vec<usize> = g.comps.iter().map(|c| c.stride).collect();
    let (comp_q, comp_ci, strides, bw_r, cv) = (&comp_q, &comp_ci, &strides, &bw, &cv);

    rayon::scope(|s| {
        let mut row = g.my0;
        for (bi, out) in bands.into_iter().enumerate() {
            let rows_here = band_rows[bi];
            let mut buf = loop {
                if let Some(b) = free.lock().unwrap_or_else(|e| e.into_inner()).pop() {
                    break b;
                }
                if allocated < max_buffers {
                    allocated += 1;
                    let coefs = (0..nslots)
                        .map(|i| vec![0i16; bw[i] * bh[i] * 64])
                        .collect();
                    let flags = (0..nslots).map(|i| vec![0u8; bw[i] * bh[i]]).collect();
                    break CoefBand { coefs, flags };
                }
                match rayon::yield_now() {
                    Some(rayon::Yield::Executed) => {}
                    _ => std::thread::yield_now(),
                }
            };
            let job = RowJob {
                rows: row..row + rows_here,
                row_base: row,
                cols: g.mx0..g.mx1,
                jump: None,
                learn: None,
                cancel: p.cancel,
            };
            let d = {
                let mut coefs: [&mut [i16]; 4] = Default::default();
                let mut flags: [&mut [u8]; 4] = Default::default();
                let mut bwa = [0usize; 4];
                for (slot, (c, f)) in buf.coefs.iter_mut().zip(buf.flags.iter_mut()).enumerate() {
                    coefs[slot] = c;
                    flags[slot] = f;
                    bwa[slot] = bw[slot];
                }
                let mut sink = CoefSink {
                    coefs,
                    flags,
                    bw: bwa,
                };
                decode_rows(sc, st, &job, &mut sink, index.as_deref_mut())
            };
            let d = match d {
                Ok(d) => d,
                Err(e) => {
                    result = Err(e);
                    break;
                }
            };
            let rows_done = d.rows as usize;
            s.spawn(move |_| {
                let mut slots: Vec<Option<&mut [u8]>> = out.into_iter().map(Some).collect();
                for (slot, &ci) in comp_ci.iter().enumerate() {
                    let Some(dst) = slots[ci].take() else {
                        continue;
                    };
                    let stride = strides[ci];
                    let n = ns[slot];
                    let bwi = bw_r[slot];
                    let coefs = &buf.coefs[slot];
                    let flags = &buf.flags[slot];
                    for by in 0..rows_done * cv[slot] {
                        for bx in 0..bwi {
                            let b = by * bwi + bx;
                            let blk: &[i16; 64] = coefs[b * 64..b * 64 + 64].try_into().unwrap();
                            idct_block(
                                simd,
                                n,
                                blk,
                                comp_q[slot],
                                flags[b] != 0,
                                Dst {
                                    buf: dst,
                                    off: by * n * stride + bx * n,
                                    stride,
                                },
                            );
                        }
                    }
                }
                free.lock().unwrap_or_else(|e| e.into_inner()).push(buf);
            });
            row += d.rows;
            if d.truncated {
                result = Ok(RowsDone {
                    rows: row - g.my0,
                    truncated: true,
                });
                break;
            }
        }
    });
    result
}
