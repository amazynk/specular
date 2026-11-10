//! Progressive JPEG decoding: a parallel, pipelined coefficient decoder.
//!
//! The image is cut into bands of MCU rows. Every band's coefficient storage (a *slab*) flows
//! through a pipeline of scans: one pipeline for the DC scans (all components) and one per
//! component for its AC scans (progressive AC scans are single-component). A band leaves a
//! pipeline once every scan of that pipeline has decoded it; when both the DC and the AC
//! pipeline delivered a band of a component, that band is dequantised, inverse transformed and
//! stored while later bands are still being entropy-decoded.
//!
//! Each scan of a pipeline is a *stage* of one of two kinds:
//! - **Parallel**: the decoder state at the start of every row of the scan is known, so all
//!   bands are decoded independently on the rayon pool. Row states come from a
//!   [`RowIndex`](crate::RowIndex) or, for AC first scans (whose bit consumption does not
//!   depend on earlier scans), from speculative decoding: the scan data is cut into chunks
//!   that are parsed concurrently from guessed starting points, and Huffman self-synchronisation
//!   (same bit position *and* same EOB run) proves the guesses right (see `spec_ac`).
//! - **Sequential**: refinement scans (whose bit consumption depends on each block's history)
//!   and DC first scans run on a dedicated thread per scan that decodes bands strictly in order
//!   as they arrive from the previous stage. Consecutive refinement scans of a component, and
//!   scans of different components, therefore run concurrently as a wavefront.
//!
//! Coefficients of progressive files are stored in zigzag order, which lets the refinement
//! decoder work on 64-bit "nonzero history" masks (popcount/select) instead of walking the
//! block coefficient by coefficient.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::coef::{CoefOutcome, CoefParams, Kind, ScanIndex, ScanPlan, plan_scans};
use crate::cols::{ColCache, ColRec};
use crate::error::{JpegError, Result};
use crate::geom::{OutGeom, alloc_i16, alloc_u64};
use crate::huffman::{BitReader, HuffTable, LUT_BITS, SKIP_BITS, skip_entry};
use crate::idct::{Dst, Simd, dc_only_value, dc_only_value12, idct_12bit, idct_block};
use crate::marker::{Parsed, ZIGZAG};
use crate::sequential::{Checkpoint, DecState};
use crate::spec::canon;
use crate::spec::{Run, Zones, expected_recs};

mod refine;
pub(crate) use refine::{REFINE_MIN, spec_bytes as refine_spec_bytes};

/// Refinement stages decoded in parallel / sequentially after failed speculation (tests).
#[cfg(test)]
static REFINE_STATS: [std::sync::atomic::AtomicUsize; 2] = [
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
];
/// Tests: the sequential decode of a refinement stage waits for its speculation's result (so
/// that the parallel path is taken deterministically when speculation succeeds).
#[cfg(test)]
static REFINE_WAIT: AtomicBool = AtomicBool::new(false);

type Bufs<'b> = [Option<&'b mut [i16]>; 4];

/// One band's coefficient storage for one pipeline.
struct Slab<'b> {
    k: usize,
    bufs: Bufs<'b>,
    /// Index-only builds: per block, the nonzero mask of the AC coefficients (bit = zigzag
    /// index) instead of the coefficients themselves.
    masks: [Option<&'b mut [u64]>; 4],
}

/// A sequential stage's decoder: (state, dead after an error, recorded row checkpoints).
type SeqState<'d> = (DecState<'d>, bool, Option<Vec<Checkpoint>>);

enum Mode {
    /// Row-start checkpoints of the scan (one per scan row).
    Par(Vec<Checkpoint>),
    Seq,
    /// Refinement scan with speculative checkpoints: decoded sequentially like `Seq`, but once
    /// every band has passed the earlier stages (the blocks' nonzero history is final), the
    /// row checkpoints of the rest of the scan are derived from that history on the pool
    /// (`refine::spec_refine`) while the sequential decode goes on; if they arrive first, the
    /// remaining bands are decoded in parallel.
    Refine,
}

/// A running speculation of a `Mode::Refine` stage.
struct SpecJob {
    /// Row checkpoints from row `first` on (`None` if speculation failed) and the time the
    /// speculation took.
    rx: mpsc::Receiver<SpecResult>,
    /// Tells the job to give up (the sequential decode finished or the decode was aborted).
    stop: Arc<AtomicBool>,
    /// First row and the end (exclusive) of the bands speculated on.
    first: u32,
    end: usize,
    /// Estimated bytes of the rows speculated on.
    bytes: usize,
    /// The sequential decode waits for the result instead of decoding on.
    wait: bool,
}

type SpecResult = (Option<Vec<Checkpoint>>, Duration);

struct Stage<'p> {
    plan: &'p ScanPlan<'p>,
    mode: Mode,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Ok,
    Overrun,
    Cancelled,
}

/// Static band / storage geometry.
struct Geo {
    /// MCU-row range of every band.
    bands: Vec<(u32, u32)>,
    /// Frame sampling factors and padded block widths per component.
    h: [u32; 4],
    v: [u32; 4],
    bwp: [u32; 4],
    ncomp: usize,
}

impl Geo {
    /// Rows of `plan` (in the scan's own row numbering) that lie in band `k`.
    fn scan_rows(&self, plan: &ScanPlan, k: usize) -> Range<u32> {
        let (m0, m1) = self.bands[k];
        if plan.interleaved {
            m0.min(plan.rows)..m1.min(plan.rows)
        } else {
            let v = self.v[plan.comps[0].0];
            (m0 * v).min(plan.rows)..(m1 * v).min(plan.rows)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Block-level entropy decoding (coefficients in zigzag order)

/// Bit `i` set iff `b[i] != 0`.
#[inline(always)]
fn nzmask(b: &[i16; 64]) -> u64 {
    use std::simd::prelude::*;
    std::simd::Simd::<i16, 64>::from_array(*b)
        .simd_ne(std::simd::Simd::splat(0))
        .to_bitmask()
}

/// Index of the `r`-th (0-based) set bit of `z`, 64 if there are not enough.
#[inline(always)]
fn select(z: u64, r: u32) -> u32 {
    // With BMI2, depositing bit `r` into the set bits of `z` lands on the `r`-th one (and gives
    // 0, so 64, when there are fewer).
    #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
    {
        pdep(1u64 << (r & 63), z).trailing_zeros()
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
    {
        select_soft(z, r)
    }
}

/// `select` by clearing the lowest set bit `r` times.
#[cfg_attr(all(target_arch = "x86_64", target_feature = "bmi2"), allow(dead_code))]
fn select_soft(z: u64, r: u32) -> u32 {
    let mut z = z;
    for _ in 0..r {
        z &= z.wrapping_sub(1);
    }
    z.trailing_zeros()
}

/// Storage of one block's AC coefficients (zigzag order): the coefficients themselves, or for
/// index-only builds just their nonzero mask, which is all the bit-level parse of later scans
/// depends on (correction bits never make a coefficient zero; see `apply_corrections`).
trait Block {
    /// Bit `i` set iff coefficient `i` is nonzero.
    fn nz(&self) -> u64;
    fn put(&mut self, k: usize, v: i16);
    /// Apply refinement corrections (mask in normal bit order).
    fn correct(&mut self, m: u64, p1: i32);
}

impl Block for [i16; 64] {
    #[inline(always)]
    fn nz(&self) -> u64 {
        nzmask(self)
    }
    #[inline(always)]
    fn put(&mut self, k: usize, v: i16) {
        self[k] = v;
    }
    #[inline(always)]
    fn correct(&mut self, m: u64, p1: i32) {
        apply_corrections(self, m, p1);
    }
}

impl Block for u64 {
    #[inline(always)]
    fn nz(&self) -> u64 {
        *self
    }
    #[inline(always)]
    fn put(&mut self, k: usize, v: i16) {
        *self = (*self & !(1u64 << k)) | (((v != 0) as u64) << k);
    }
    #[inline(always)]
    fn correct(&mut self, _m: u64, _p1: i32) {}
}

/// AC first scan, one block.
#[inline(always)]
fn ac_first<B: Block>(
    br: &mut BitReader,
    t: &HuffTable,
    ss: u32,
    se: u32,
    al: u32,
    eobrun: &mut u32,
    blk: &mut B,
) {
    if *eobrun > 0 {
        *eobrun -= 1;
        return;
    }
    let mut k = ss;
    while k <= se {
        br.ensure(32);
        let e = t.lut[br.peek(LUT_BITS) as usize];
        if e >> 16 != 0 {
            // Code and magnitude fit the lookahead: value precomputed.
            br.consume(e & 31);
            k += (e >> 12) & 15;
            blk.put(k.min(63) as usize, (((e >> 16) as i16 as i32) << al) as i16);
            k += 1;
            continue;
        }
        let sym = br.decode_symbol(t);
        let r = sym >> 4;
        let s = sym & 15;
        if s != 0 {
            k += r;
            let v = br.get_extend(s);
            blk.put(k.min(63) as usize, (v << al) as i16);
        } else if r == 15 {
            k += 15;
        } else {
            *eobrun = 1 << r;
            if r != 0 {
                *eobrun += br.get(r);
            }
            *eobrun -= 1;
            break;
        }
        k += 1;
    }
}

/// AC first scan, one block, without storing coefficients (consumes exactly the bits of
/// [`ac_first`]); uses the multi-symbol skip table.
#[inline(always)]
fn ac_first_skip(br: &mut BitReader, t: &HuffTable, ss: u32, se: u32, eobrun: &mut u32) {
    if *eobrun > 0 {
        *eobrun -= 1;
        return;
    }
    let skip = &t.skip[..1 << SKIP_BITS];
    let mut k = ss;
    while k <= se {
        br.ensure(32);
        // Several complete symbols when all but the last lie inside the band; an EOB in an
        // entry is always EOB0 (ends this block only).
        let (bits, adv, lim, eob) = skip_entry(skip[br.peek(SKIP_BITS) as usize]);
        if bits != 0 && k + lim <= se {
            br.consume(bits);
            k += adv;
            if eob {
                return;
            }
            continue;
        }
        let e = t.lut[br.peek(LUT_BITS) as usize];
        if e >> 16 != 0 {
            br.consume(e & 31);
            k += ((e >> 12) & 15) + 1;
            continue;
        }
        let sym = br.decode_symbol(t);
        let r = sym >> 4;
        let s = sym & 15;
        if s != 0 {
            br.consume(s);
            k += r + 1;
        } else if r == 15 {
            k += 16;
        } else {
            *eobrun = 1 << r;
            if r != 0 {
                *eobrun += br.get(r);
            }
            *eobrun -= 1;
            return;
        }
    }
}

/// Parallel bit deposit: BMI2 `pdep` (part of the x86-64-v3 baseline), else a bit loop.
#[inline(always)]
fn pdep(a: u64, m: u64) -> u64 {
    #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
    #[allow(unsafe_code)]
    {
        // SAFETY: `_pdep_u64` only requires the BMI2 target feature, which this branch is
        // compiled under (`cfg(target_feature = "bmi2")`, the x86-64-v3 baseline); it has no
        // memory or other preconditions.
        unsafe { std::arch::x86_64::_pdep_u64(a, m) }
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
    {
        pdep_soft(a, m)
    }
}

/// Portable `pdep`. Builds with BMI2 use the instruction instead, but the tests still compare
/// against this, hence the `dead_code` allowance there.
#[cfg_attr(all(target_arch = "x86_64", target_feature = "bmi2"), allow(dead_code))]
#[inline(always)]
fn pdep_soft(a: u64, m: u64) -> u64 {
    let (mut a, mut m, mut r) = (a, m, 0u64);
    while m != 0 {
        let low = m & m.wrapping_neg();
        r |= low & (a & 1).wrapping_neg();
        a >>= 1;
        m ^= low;
    }
    r
}

/// Read one correction bit per set bit of `mr`, a mask in *reversed* index space (bit `63 - i`
/// stands for coefficient `i`): the first stream bit belongs to the lowest coefficient, i.e.
/// the highest set bit, so the next `n` stream bits deposit directly with `pdep`. Returns the
/// (reversed) mask of coefficients whose bit is 1.
#[inline(always)]
fn take_bits(br: &mut BitReader, mut mr: u64) -> u64 {
    let mut n = mr.count_ones();
    let mut out = 0u64;
    if n > 56 {
        // The lowest coefficients' 32 bits first, then the rest.
        let lo = pdep((1u64 << (n - 32)) - 1, mr);
        br.ensure(32);
        out = pdep(br.buf >> 32, mr ^ lo);
        br.consume(32);
        mr = lo;
        n -= 32;
    }
    br.ensure(n);
    // Top n bits (n == 0: garbage, but pdep into an empty mask yields 0; no branch).
    out |= pdep((br.buf >> 1) >> (63 - n), mr);
    br.consume(n);
    out
}

/// Apply the correction bits of one refinement scan: every coefficient in `m` that lacks bit
/// `p1` grows by `p1` in magnitude. This never turns a nonzero coefficient into 0 (the i16
/// result `c + sign(c) * p1` could only wrap to 0 for `c == -sign(c) * p1`, or `c == -2^15`
/// with `p1 == 2^15`, and those have bit `p1` set already), so the nonzero masks used by
/// index-only builds need no update here.
#[inline(always)]
fn apply_corrections(blk: &mut [i16; 64], mut m: u64, p1: i32) {
    while m != 0 {
        let i = m.trailing_zeros() as usize;
        m &= m - 1;
        let c = blk[i] as i32;
        let sgn = c >> 31;
        let delta = ((p1 ^ sgn) - sgn) & -(((c & p1) == 0) as i32);
        blk[i] = (c + delta) as i16;
    }
}

/// AC refinement scan, one block: libjpeg's decode_mcu_AC_refine, but on 64-bit masks.
/// Correction bits never interact with the new coefficients of the same scan (those go to
/// zero-history positions only), so they are gathered as a mask and applied at the end.
#[inline(always)]
fn ac_refine<B: Block>(
    br: &mut BitReader,
    t: &HuffTable,
    ss: u32,
    se: u32,
    al: u32,
    eobrun: &mut u32,
    blk: &mut B,
) {
    let p1 = 1i32 << al;
    let range = (u64::MAX >> (63 - se)) & (u64::MAX << ss);
    let nz = blk.nz() & range;
    let nzr = nz.reverse_bits();
    let mut corr = 0u64;
    let mut k = ss;
    if *eobrun == 0 {
        let zs = !nz & range;
        while k <= se {
            br.ensure(32);
            let e = t.lut[br.peek(LUT_BITS) as usize];
            let (r, val);
            if e >> 16 != 0 {
                // Code and sign bit fit the lookahead: the entry holds the value +-1 (size 1;
                // other sizes are invalid in refinement scans and treated like libjpeg would).
                // Code plus one sign bit (libjpeg reads exactly one bit for any size; sizes
                // other than 1 are invalid here). The entry's value has that bit's sign.
                br.consume((e & 31) - ((e >> 8) & 15) + 1);
                r = (e >> 12) & 15;
                val = if (e >> 31) != 0 { -p1 } else { p1 };
            } else {
                let sym = br.decode_symbol(t);
                r = sym >> 4;
                let s = sym & 15;
                if s == 0 && r != 15 {
                    *eobrun = 1 << r;
                    if r != 0 {
                        *eobrun += br.get(r);
                    }
                    break;
                }
                val = if s != 0 {
                    if br.get(1) != 0 { p1 } else { -p1 }
                } else {
                    0
                };
            }
            // Position of the (r+1)-th coefficient with zero history at or after k.
            let pos = select(zs & (u64::MAX << k), r).min(se + 1);
            // Coefficients k..pos in reversed space.
            // (pos >= k >= 1, and pos == 64 must give an empty upper part.)
            let rr = (u64::MAX >> k) & !((u64::MAX >> 1) >> (pos - 1));
            corr |= take_bits(br, nzr & rr);
            if val != 0 {
                blk.put(pos.min(63) as usize, val as i16);
            }
            k = pos + 1;
        }
    }
    if *eobrun > 0 {
        if k <= se {
            corr |= take_bits(br, nzr & (u64::MAX >> k));
        }
        *eobrun -= 1;
    }
    blk.correct(corr.reverse_bits(), p1);
}

/// Parameters of an AC scan.
struct AcScan<'t> {
    t: &'t HuffTable,
    ss: u32,
    se: u32,
    al: u32,
    ri: u32,
    refine: bool,
}

/// Per-row bookkeeping: record the row checkpoint, check for cancellation.
#[inline(always)]
fn row_start(
    st: &DecState,
    rec: &mut Option<&mut Vec<Checkpoint>>,
    cancel: Option<&AtomicBool>,
) -> Option<Status> {
    if let Some(r) = rec.as_deref_mut() {
        r.push(st.checkpoint());
    }
    cancel
        .is_some_and(|c| c.load(Ordering::Relaxed))
        .then_some(Status::Cancelled)
}

/// One row of blocks of an AC scan; `Some(Overrun)` if the data ran out.
#[inline(always)]
fn ac_line<B: Block>(a: &AcScan, st: &mut DecState, blocks: &mut [B]) -> Option<Status> {
    if a.refine {
        for blk in blocks {
            st.before_mcu(a.ri);
            ac_refine(&mut st.br, a.t, a.ss, a.se, a.al, &mut st.eobrun, blk);
        }
    } else {
        let mut i = 0;
        while i < blocks.len() {
            // Blocks of an EOB run in a first scan read nothing: skip the run in one step (up
            // to the next restart marker, which ends it).
            if st.eobrun > 0 {
                let mut n = (st.eobrun as usize).min(blocks.len() - i);
                if a.ri != 0 {
                    n = n.min(st.restart_left as usize);
                }
                if n > 0 {
                    st.eobrun -= n as u32;
                    if a.ri != 0 {
                        st.restart_left -= n as u32;
                    }
                    i += n;
                    continue;
                }
            }
            st.before_mcu(a.ri);
            ac_first(
                &mut st.br,
                a.t,
                a.ss,
                a.se,
                a.al,
                &mut st.eobrun,
                &mut blocks[i],
            );
            i += 1;
        }
    }
    st.br.overrun().then_some(Status::Overrun)
}

/// Decode `rows` of `plan` from `st` into `bufs` (band `m0..` storage), or for AC scans into
/// `masks` when the band has no coefficient storage (index-only builds). DC scans without
/// storage are parsed only.
#[inline(always)]
fn rows(
    geo: &Geo,
    plan: &ScanPlan,
    st: &mut DecState,
    rows: Range<u32>,
    slab: &mut Slab,
    mut rec: Option<&mut Vec<Checkpoint>>,
    cancel: Option<&AtomicBool>,
) -> Status {
    let m0 = geo.bands[slab.k].0;
    let (bufs, masks) = (&mut slab.bufs, &mut slab.masks);
    let s = plan.scan;
    let ri = s.restart_interval;
    let (ss, se, al) = (s.ss as u32, s.se.min(63) as u32, s.al as u32);
    match plan.kind {
        Kind::AcFirst | Kind::AcRefine => {
            let (c, _, _, _, act) = plan.comps[0];
            let Some(t) = act else {
                return Status::Overrun;
            };
            if ss > se {
                return Status::Overrun;
            }
            let row0 = m0 * geo.v[c];
            let bwp = geo.bwp[c] as usize;
            let ux = plan.units_x as usize;
            let refine = plan.kind == Kind::AcRefine;
            let a = AcScan {
                t,
                ss,
                se,
                al,
                ri,
                refine,
            };
            if let Some(buf) = bufs[c].as_deref_mut() {
                let line = |row: u32| {
                    let base = (row - row0) as usize * bwp;
                    base * 64..(base + ux) * 64
                };
                for row in rows {
                    if let Some(s) = row_start(st, &mut rec, cancel) {
                        return s;
                    }
                    let blocks = buf[line(row)].as_chunks_mut::<64>().0;
                    if let Some(s) = ac_line(&a, st, blocks) {
                        return s;
                    }
                }
            } else if let Some(buf) = masks[c].as_deref_mut() {
                for row in rows {
                    if let Some(s) = row_start(st, &mut rec, cancel) {
                        return s;
                    }
                    let base = (row - row0) as usize * bwp;
                    if let Some(s) = ac_line(&a, st, &mut buf[base..base + ux]) {
                        return s;
                    }
                }
            } else {
                return Status::Overrun;
            }
        }
        Kind::DcFirst | Kind::DcRefine => {
            for row in rows {
                if let Some(r) = rec.as_deref_mut() {
                    r.push(st.checkpoint());
                }
                if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                    return Status::Cancelled;
                }
                for ux in 0..plan.units_x {
                    st.before_mcu(ri);
                    for (slot, &(ci, h, v, dct, _)) in plan.comps.iter().enumerate() {
                        let mut buf = bufs[ci].as_deref_mut();
                        let (nh, nv) = if plan.interleaved { (h, v) } else { (1, 1) };
                        let row0 = m0 * geo.v[ci];
                        let bwp = geo.bwp[ci] as usize;
                        for bv in 0..nv {
                            for bh in 0..nh {
                                let (bx, by) = if plan.interleaved {
                                    (ux * h + bh, row * v + bv)
                                } else {
                                    (ux, row)
                                };
                                let i = (by - row0) as usize * bwp + bx as usize;
                                if plan.kind == Kind::DcFirst {
                                    let Some(t) = dct else {
                                        return Status::Overrun;
                                    };
                                    st.br.ensure(32);
                                    let pred = &mut st.dc[slot];
                                    *pred = pred.wrapping_add(st.br.decode_dc_diff(t));
                                    if let Some(b) = buf.as_deref_mut() {
                                        b[i] = (*pred << al) as i16;
                                    }
                                } else {
                                    st.br.ensure(1);
                                    if st.br.get(1) != 0
                                        && let Some(b) = buf.as_deref_mut()
                                    {
                                        b[i] |= (1i32 << al) as i16;
                                    }
                                }
                            }
                        }
                    }
                }
                if st.br.overrun() {
                    return Status::Overrun;
                }
            }
        }
        Kind::Seq => unreachable!("sequential scan in progressive decoder"),
    }
    Status::Ok
}

/// Where every row of one scan starts in a region decode with column checkpoints.
struct ScanCols {
    /// First scan row described by `starts`.
    row0: u32,
    /// Units to parse (exclusive end: the region's right edge).
    end: u32,
    /// Per row: the unit parsing starts at, the decoder state there, and the first unit whose
    /// coefficients are stored (earlier ones are only parsed).
    starts: Vec<(u32, Checkpoint, u32)>,
}

/// Region decodes with an index: every scan row starts at the last recorded column checkpoint
/// before the region and stops at its right edge (see [`crate::cols`]).
struct ColPlan<'e> {
    cache: &'e ColCache,
    /// Per scan (`ScanPlan::si`).
    scans: Vec<Option<ScanCols>>,
}

impl<'e> ColPlan<'e> {
    /// Start columns for the rows of every stage that `geo`'s bands cover. AC refinement scans
    /// need every earlier AC scan of their component to have decoded (not only parsed) the
    /// blocks they refine, so all scans of such a component store from one common column at
    /// which every refinement scan has a checkpoint.
    fn new(
        cache: &'e ColCache,
        ix: &ScanIndex,
        g: &OutGeom,
        geo: &Geo,
        plans: &[&ScanPlan],
        nscans: usize,
    ) -> Option<ColPlan<'e>> {
        let nb = geo.bands.len();
        if nb == 0 {
            return None;
        }
        let span = |pl: &ScanPlan| {
            let a = geo.scan_rows(pl, 0).start;
            let b = geo.scan_rows(pl, nb - 1).end;
            a..b.max(a)
        };
        let units = |pl: &ScanPlan| -> (u32, u32) {
            let h = if pl.interleaved {
                1
            } else {
                geo.h[pl.comps[0].0]
            };
            ((g.mx0 * h).min(pl.units_x), (g.mx1 * h).min(pl.units_x))
        };
        let row_cp = |pl: &ScanPlan, r: u32| ix.get(pl.si).and_then(|v| v.get(r as usize)).copied();
        let best = |pl: &ScanPlan, r: u32, u: u32| -> Option<(u32, Checkpoint)> {
            match cache.best(pl.si, r, u) {
                Some(b) => Some(b),
                None => row_cp(pl, r).map(|c| (0, c)),
            }
        };
        // Common store column per component with refinement scans, per row (in the rows of
        // the component's block rows; AC scans are single-component, rows = block rows).
        let mut common: [Option<(u32, Vec<u32>)>; 4] = Default::default();
        #[allow(clippy::needless_range_loop)] // the index also feeds closures/other arrays
        for c in 0..geo.ncomp {
            let refs: Vec<&&ScanPlan> = plans
                .iter()
                .filter(|pl| pl.kind == Kind::AcRefine && pl.comps[0].0 == c)
                .collect();
            let Some(first) = refs.first() else {
                continue;
            };
            let rows = span(first);
            let (u0, _) = units(first);
            let mut v = Vec::with_capacity(rows.len());
            for r in rows.clone() {
                let mut s = u0;
                loop {
                    let mut m = s;
                    for pl in &refs {
                        m = m.min(best(pl, r, s)?.0);
                    }
                    if m == s {
                        break;
                    }
                    s = m;
                }
                v.push(s);
            }
            common[c] = Some((rows.start, v));
        }
        let mut scans: Vec<Option<ScanCols>> = (0..nscans).map(|_| None).collect();
        for pl in plans {
            let rows = span(pl);
            let (u0, end) = units(pl);
            let mut starts = Vec::with_capacity(rows.len());
            for r in rows.clone() {
                let from = match pl.kind {
                    Kind::AcFirst | Kind::AcRefine => match &common[pl.comps[0].0] {
                        Some((r0, v)) => *v.get((r - r0) as usize)?,
                        None => u0,
                    },
                    _ => u0,
                };
                let (u, cp) = best(pl, r, from)?;
                if pl.kind == Kind::AcRefine && u != from {
                    return None;
                }
                starts.push((u, cp, from));
            }
            scans[pl.si] = Some(ScanCols {
                row0: rows.start,
                end,
                starts,
            });
        }
        if tracing::enabled!(target: "jpeg", tracing::Level::DEBUG) {
            let (mut rows, mut mid, mut refine_mid) = (0usize, 0usize, 0usize);
            for (pl, sc) in plans
                .iter()
                .filter_map(|pl| Some((pl, scans[pl.si].as_ref()?)))
            {
                rows += sc.starts.len();
                let m = sc.starts.iter().filter(|s| s.0 > 0).count();
                mid += m;
                if pl.kind == Kind::AcRefine {
                    refine_mid += m;
                }
            }
            tracing::debug!(target: "jpeg", rows, mid, refine_mid, "region rows from column checkpoints");
        }
        Some(ColPlan { cache, scans })
    }
}

/// Decode `rows` of `plan` with column checkpoints: each row from its own start (`sc`) to the
/// region's right edge, recording new column checkpoints on the way.
#[allow(clippy::too_many_arguments)]
fn rows_cols(
    data: &[u8],
    geo: &Geo,
    plan: &ScanPlan,
    sc: &ScanCols,
    cache: &ColCache,
    rows: Range<u32>,
    slab: &mut Slab,
    cancel: Option<&AtomicBool>,
) -> Status {
    let m0 = geo.bands[slab.k].0;
    let s = plan.scan;
    let ri = s.restart_interval;
    let (ss, se, al) = (s.ss as u32, s.se.min(63) as u32, s.al as u32);
    let mut rec = ColRec::new(cache.step(plan.si), 0);
    for row in rows {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Status::Cancelled;
        }
        let Some(&(u_start, cp, from)) = row
            .checked_sub(sc.row0)
            .and_then(|i| sc.starts.get(i as usize))
        else {
            return Status::Overrun;
        };
        let mut st = DecState::from_cp(data, &cp);
        rec.reset(u_start);
        match plan.kind {
            Kind::AcFirst | Kind::AcRefine => {
                let (c, _, _, _, act) = plan.comps[0];
                let Some(t) = act else {
                    return Status::Overrun;
                };
                if ss > se {
                    return Status::Overrun;
                }
                let Some(buf) = slab.bufs[c].as_deref_mut() else {
                    return Status::Overrun;
                };
                let bwp = geo.bwp[c] as usize;
                let base = (row - m0 * geo.v[c]) as usize * bwp;
                let blocks = buf[base * 64..(base + bwp) * 64].as_chunks_mut::<64>().0;
                let refine = plan.kind == Kind::AcRefine;
                for u in u_start..sc.end {
                    rec.at(u, || st.checkpoint());
                    st.before_mcu(ri);
                    if u < from {
                        ac_first_skip(&mut st.br, t, ss, se, &mut st.eobrun);
                    } else if refine {
                        ac_refine(
                            &mut st.br,
                            t,
                            ss,
                            se,
                            al,
                            &mut st.eobrun,
                            &mut blocks[u as usize],
                        );
                    } else {
                        ac_first(
                            &mut st.br,
                            t,
                            ss,
                            se,
                            al,
                            &mut st.eobrun,
                            &mut blocks[u as usize],
                        );
                    }
                }
            }
            Kind::DcFirst | Kind::DcRefine => {
                for ux in u_start..sc.end {
                    rec.at(ux, || st.checkpoint());
                    st.before_mcu(ri);
                    for (slot, &(ci, h, v, dct, _)) in plan.comps.iter().enumerate() {
                        let mut buf = slab.bufs[ci].as_deref_mut();
                        let (nh, nv) = if plan.interleaved { (h, v) } else { (1, 1) };
                        let row0 = m0 * geo.v[ci];
                        let bwp = geo.bwp[ci] as usize;
                        for bv in 0..nv {
                            for bh in 0..nh {
                                let (bx, by) = if plan.interleaved {
                                    (ux * h + bh, row * v + bv)
                                } else {
                                    (ux, row)
                                };
                                let i = (by - row0) as usize * bwp + bx as usize;
                                if plan.kind == Kind::DcFirst {
                                    let Some(t) = dct else {
                                        return Status::Overrun;
                                    };
                                    st.br.ensure(32);
                                    let pred = &mut st.dc[slot];
                                    *pred = pred.wrapping_add(st.br.decode_dc_diff(t));
                                    if let Some(b) = buf.as_deref_mut() {
                                        b[i] = (*pred << al) as i16;
                                    }
                                } else {
                                    st.br.ensure(1);
                                    if st.br.get(1) != 0
                                        && let Some(b) = buf.as_deref_mut()
                                    {
                                        b[i] |= (1i32 << al) as i16;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Kind::Seq => unreachable!("sequential scan in progressive decoder"),
        }
        if st.br.overrun() {
            return Status::Overrun;
        }
        rec.flush(cache, plan.si, row);
    }
    Status::Ok
}

// ---------------------------------------------------------------------------------------------
// Speculative row checkpoints for AC first scans

/// Decoder state at a block start: raw byte position, buffered real bits, EOB run.
#[derive(Clone, Copy)]
struct Rec {
    pos: u32,
    bits: u8,
    eob: u16,
}

/// Speculative parsing of AC-first scans: minimum bytes per chunk, and how far each chunk's run
/// extends into the next one (AC-first data resynchronised within 430 bytes on the test photos;
/// smaller than the baseline margin since a record per block, even inside EOB runs, makes the
/// zones denser).
pub(crate) const AC_MIN_CHUNK: usize = 64 << 10;
pub(crate) const AC_MARGIN: usize = 2 << 10;

/// Bytes per kept record of a [`Run`] of AC-first block starts.
pub(crate) const RUN_REC_BYTES: u64 = std::mem::size_of::<(u32, Rec)>() as u64;

impl Rec {
    /// Strictly increasing along a decode: bit position, then (within an EOB run) block count.
    #[inline]
    fn key(&self, data: &[u8]) -> u64 {
        (canon(data, self.pos as usize, self.bits as u32) << 16) | (0xFFFF - self.eob) as u64
    }
}

#[inline(always)]
fn spec_run(
    plan: &ScanPlan,
    st: &mut DecState,
    stop: usize,
    max: u64,
    zones: Zones,
    est: usize,
    cancel: Option<&AtomicBool>,
) -> Run<Rec> {
    let s = plan.scan;
    let (ss, se) = (s.ss as u32, s.se.min(63) as u32);
    let t = plan.comps[0].4.expect("checked by caller");
    let mut out = Run::new(est, st.br.pos, stop.min(s.data_end), zones);
    while out.count < max && st.br.pos < stop {
        let Some(r) = rec_of(st) else {
            break;
        };
        if out.count.is_multiple_of(4096) && cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            break;
        }
        out.record(st.br.pos, r);
        ac_first_skip(&mut st.br, t, ss, se, &mut st.eobrun);
    }
    out
}

/// The record of the current state (`None` once the reader is into the zero fill).
#[inline(always)]
fn rec_of(st: &DecState) -> Option<Rec> {
    let real = st.br.bits as i64 - st.br.zero_fill as i64 * 8;
    (real >= 0).then(|| Rec {
        pos: st.br.pos as u32,
        bits: real as u8,
        eob: st.eobrun.min(0xFFFF) as u16,
    })
}

/// Checkpoint at a canonical bit position with an EOB run.
fn checkpoint_at(data: &[u8], c: u64, eob: u32) -> Checkpoint {
    let mut st = DecState::from_cp(data, &Checkpoint::at(c as usize / 8, 0));
    st.br.refill();
    st.br.consume((c % 8) as u32);
    st.eobrun = eob;
    st.checkpoint()
}

/// Row-start checkpoints of a restart-free AC first scan, found by speculative parallel
/// parsing (see [`crate::spec`]). `None` if the data is truncated/corrupt (the scan is then
/// decoded sequentially).
fn spec_ac(
    data: &[u8],
    plan: &ScanPlan,
    nchunks: usize,
    min_chunk: usize,
    margin: usize,
    cancel: Option<&AtomicBool>,
) -> Option<Vec<Checkpoint>> {
    let s = plan.scan;
    if s.restart_interval != 0
        || plan.comps[0].4.is_none()
        || s.ss as u32 > s.se.min(63) as u32
        || data.len() >= (u32::MAX as usize) / 2
    {
        return None;
    }
    let start = s.data_start;
    let end = s.data_end.min(data.len()).max(start);
    let len = end - start;
    let bw = plan.units_x as u64;
    let total = bw * plan.rows as u64;
    if total == 0 {
        return Some(Vec::new());
    }
    let nchunks = nchunks.clamp(1, (len / min_chunk.max(1)).max(1));
    let mut starts: Vec<usize> = (0..nchunks).map(|i| start + len * i / nchunks).collect();
    for s in starts.iter_mut().skip(1) {
        while *s < end && (data[*s - 1] == 0xFF || data[*s] == 0xFF) {
            *s += 1;
        }
    }
    let stops: Vec<usize> = (0..nchunks)
        .map(|i| {
            if i + 1 < nchunks {
                starts[i + 1] + margin
            } else {
                usize::MAX
            }
        })
        .collect();
    let job = |i: usize| -> Run<Rec> {
        let mut st = DecState::from_cp(data, &Checkpoint::at(starts[i], 0));
        let est = expected_recs(total, len, stops[i].min(end) - starts[i], margin);
        let zones = Zones::chunk(&starts, i, margin);
        spec_run(plan, &mut st, stops[i], total, zones, est, cancel)
    };
    let mut runs: Vec<Run<Rec>> = (0..nchunks).into_par_iter().map(job).collect();

    // Sequential merge; segments are (absolute block index of run index `from` of runs[c], c,
    // from).
    let mut segs: Vec<(u64, usize, u64)> = vec![(0, 0, 0)];
    for t in 1..nchunks {
        let (abs0, c, from) = *segs.last().unwrap();
        let truth = &runs[c].recs;
        let next = &runs[t].recs;
        let first_next = next.first().map_or(u64::MAX, |r| r.1.key(data));
        let mut i = runs[c].first_from(from);
        while i < truth.len() && truth[i].1.key(data) < first_next {
            i += 1;
        }
        let mut j = 0usize;
        let mut found = None;
        while i < truth.len() && j < next.len() {
            let (a, b) = (truth[i].1.key(data), next[j].1.key(data));
            if a == b {
                found = Some((i, j));
                break;
            }
            if a < b {
                i += 1;
            } else {
                j += 1;
            }
        }
        if let Some((i, j)) = found {
            tracing::trace!(target: "jpeg", scan = plan.si, chunk = t, block = abs0 + (truth[i].0 as u64 - from), skipped = j, "AC chunk synchronised");
            segs.push((abs0 + (truth[i].0 as u64 - from), t, next[j].0 as u64));
        } else {
            // No synchronisation: continue sequentially from the last known-true state.
            let &(last, r) = truth.last().filter(|r| r.0 as u64 >= from)?;
            let abs = abs0 + (last as u64 - from);
            tracing::debug!(target: "jpeg", scan = plan.si, chunk = t, block = abs, offset = starts[t], "AC chunk did not synchronise: re-parsing it sequentially");
            let cp = checkpoint_at(
                data,
                canon(data, r.pos as usize, r.bits as u32),
                r.eob as u32,
            );
            let mut st = DecState::from_cp(data, &cp);
            let est = expected_recs(
                total,
                len,
                stops[t].min(end).saturating_sub(st.br.pos),
                margin,
            );
            let zones = Zones {
                head_end: 0,
                ..Zones::chunk(&starts, t, margin)
            };
            let more = spec_run(plan, &mut st, stops[t], total - abs, zones, est, cancel);
            runs.push(more);
            segs.push((abs, runs.len() - 1, 0));
        }
    }
    let (abs, c, from) = *segs.last().unwrap();
    if abs + runs[c].count.saturating_sub(from) < total {
        tracing::debug!(target: "jpeg", scan = plan.si, blocks = abs + runs[c].count.saturating_sub(from), total, "AC speculation: data ends early");
        return None;
    }
    let (ss, se) = (s.ss as u32, s.se.min(63) as u32);
    let t = plan.comps[0].4.expect("checked above");
    let mut out = Vec::with_capacity(plan.rows as usize);
    let mut sg = 0usize;
    for row in 0..plan.rows {
        let m = row as u64 * bw;
        while sg + 1 < segs.len() && segs[sg + 1].0 <= m {
            sg += 1;
        }
        let (abs, c, from) = segs[sg];
        let k = from + (m - abs);
        if k >= runs[c].count {
            return None;
        }
        // `from` is kept, so this is a record of the synchronised part.
        let &(kr, r) = runs[c].at_or_before(k)?;
        let mut cp = checkpoint_at(
            data,
            canon(data, r.pos as usize, r.bits as u32),
            r.eob as u32,
        );
        if (kr as u64) < k {
            let mut st = DecState::from_cp(data, &cp);
            for _ in kr as u64..k {
                ac_first_skip(&mut st.br, t, ss, se, &mut st.eobrun);
            }
            let r = rec_of(&st)?;
            cp = checkpoint_at(
                data,
                canon(data, r.pos as usize, r.bits as u32),
                r.eob as u32,
            );
        }
        out.push(cp);
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// Output (dequantise + IDCT + store)

struct OutCtx<'e> {
    p: &'e Parsed,
    g: &'e OutGeom,
    simd: Simd,
    dc_only: bool,
}

/// One band of one component ready for output.
struct OutJob<'b> {
    ci: usize,
    k: usize,
    dc: &'b [i16],
    ac: Option<&'b mut [i16]>,
    plane: &'b mut [u8],
    /// Zero every AC block after reading it (storage reused for the next band).
    clear: bool,
}

/// Natural-order index -> zigzag index.
const UNZIGZAG: [usize; 64] = {
    let mut t = [0usize; 64];
    let mut k = 0;
    while k < 64 {
        t[ZIGZAG[k] as usize] = k;
        k += 1;
    }
    t
};

/// A zigzag-order block in natural order: one constant shuffle instead of a scatter loop
/// whose trip count (and exit branch) depends on the data.
#[inline(always)]
fn unzigzag(zz: &[i16; 64]) -> [i16; 64] {
    use std::simd::{Simd, simd_swizzle};
    simd_swizzle!(Simd::<i16, 64>::from_array(*zz), UNZIGZAG).to_array()
}

/// Output one band of one component. Returns whether it read (and, with `clear`, zeroed) every
/// AC block of the band: false for region decodes that output only part of it.
#[inline(always)]
fn out_band(o: &OutCtx, geo: &Geo, j: OutJob) -> bool {
    let (g, f) = (o.g, &o.p.frame);
    let ci = j.ci;
    let (m0, m1) = geo.bands[j.k];
    let (h, v) = (geo.h[ci], geo.v[ci]);
    let q = &o.p.quant[ci];
    let twelve = f.precision == 12;
    let n = g.comps[ci].n;
    let stride = g.comps[ci].stride;
    let row_bytes = stride * n;
    let bwp = geo.bwp[ci] as usize;
    let first = m0.max(g.my0);
    let last = m1.min(g.my1);
    let bx0 = (g.mx0 * h) as usize;
    let nbx = ((g.mx1 - g.mx0) * h) as usize;
    let mut ac = j.ac.filter(|_| !o.dc_only);
    for by in first * v..last * v {
        let lr = (by - m0 * v) as usize;
        let pr = (by - first * v) as usize;
        let dst = &mut j.plane[pr * row_bytes..(pr + 1) * row_bytes];
        for i in 0..nbx {
            let idx = lr * bwp + bx0 + i;
            let off = i * n;
            let dc = j.dc[idx];
            let Some(ac) = ac.as_deref_mut() else {
                dst[off] = if twelve {
                    dc_only_value12(dc, q[0])
                } else {
                    dc_only_value(dc, q[0])
                };
                continue;
            };
            let zz: &mut [i16; 64] = (&mut ac[idx * 64..idx * 64 + 64]).try_into().unwrap();
            let has_ac = nzmask(zz) & !1 != 0;
            let mut blk = if has_ac { unzigzag(zz) } else { [0i16; 64] };
            // (Index 0 is unused, so a block without AC is already all zero.)
            if j.clear && has_ac {
                *zz = [0; 64];
            }
            blk[0] = dc;
            if twelve {
                idct_12bit(&blk, q, n, dst, off, stride);
            } else {
                idct_block(
                    o.simd,
                    n,
                    &blk,
                    q,
                    has_ac,
                    Dst {
                        buf: dst,
                        off,
                        stride,
                    },
                );
            }
        }
    }
    first == m0 && last == m1 && bx0 == 0 && nbx == bwp
}

// ---------------------------------------------------------------------------------------------
// Pipeline engine

#[derive(Default)]
struct Slot<'b> {
    dc: Option<&'b [i16]>,
    ac: Option<&'b mut [i16]>,
    plane: Option<&'b mut [u8]>,
}

struct Engine<'e, 'b> {
    data: &'e [u8],
    /// Pipeline 0: DC scans; pipeline 1 + c: AC scans of component c.
    pipes: Vec<Vec<Stage<'e>>>,
    geo: Geo,
    cancel: Option<&'e AtomicBool>,
    cancelled: AtomicBool,
    truncated: AtomicBool,
    /// Set when a task panicked: sequential stage threads stop waiting.
    abort: AtomicBool,
    tx: Vec<Vec<Option<mpsc::Sender<Slab<'b>>>>>,
    /// Output join state per [component][band]; None when no pixels are produced.
    join: Option<Mutex<Vec<Vec<Slot<'b>>>>>,
    out: OutCtx<'e>,
    /// Index build without output: a trailing refinement stage whose checkpoints were found
    /// speculatively need not be decoded.
    no_output: bool,
    threads: usize,
    /// Refinement speculation in this decode has paid off so far (see `seq_thread`).
    refine_good: AtomicBool,
    /// Sequential refinement decode rate measured in this decode (bytes per µs, f64 bits).
    seq_rate: AtomicU64,
    /// Region decode with column checkpoints (index mode only).
    cols: Option<ColPlan<'e>>,
}

/// Sets the abort flag if dropped while panicking.
struct PanicGuard<'a>(&'a AtomicBool);

impl Drop for PanicGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.store(true, Ordering::Relaxed);
        }
    }
}

impl<'e, 'b> Engine<'e, 'b> {
    fn halted(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Records the outcome `s` of decoding band `k` of `plan` (`pos`: reader position after it).
    fn note(&self, s: Status, plan: &ScanPlan, k: usize, pos: usize) -> bool {
        tracing::trace!(target: "jpeg", scan = plan.si, band = k, pos, status = ?s, "band");
        match s {
            Status::Ok => true,
            Status::Overrun => {
                tracing::debug!(target: "jpeg", scan = plan.si, band = k, offset = pos, "entropy data ends early");
                self.truncated.store(true, Ordering::Relaxed);
                false
            }
            Status::Cancelled => {
                self.cancelled.store(true, Ordering::Relaxed);
                false
            }
        }
    }

    /// Decode band `slab` with a parallel (checkpointed) stage.
    fn run_par(&self, p: usize, s: usize, slab: &mut Slab<'_>) {
        let stage = &self.pipes[p][s];
        let Mode::Par(cps) = &stage.mode else {
            unreachable!("parallel stage")
        };
        let r = self.geo.scan_rows(stage.plan, slab.k);
        if let Some(cols) = &self.cols
            && let Some(sc) = &cols.scans[stage.plan.si]
        {
            if r.is_empty() || self.halted() {
                return;
            }
            let res = rows_cols(
                self.data,
                &self.geo,
                stage.plan,
                sc,
                cols.cache,
                r,
                slab,
                self.cancel,
            );
            self.note(res, stage.plan, slab.k, 0);
            return;
        }
        if let Some(cp) = cps.get(r.start as usize) {
            self.run_at(p, s, slab, cp);
        }
    }

    /// Decode band `slab` of stage `s` of pipeline `p` from checkpoint `cp` (its first row).
    fn run_at(&self, p: usize, s: usize, slab: &mut Slab<'_>, cp: &Checkpoint) {
        let plan = self.pipes[p][s].plan;
        let r = self.geo.scan_rows(plan, slab.k);
        if r.is_empty() || self.halted() {
            return;
        }
        let mut st = DecState::from_cp(self.data, cp);
        let res = rows(&self.geo, plan, &mut st, r, slab, None, self.cancel);
        self.note(res, plan, slab.k, st.br.pos);
    }

    /// Decode band `slab` with a sequential stage whose state is `st`.
    fn run_seq(
        &self,
        p: usize,
        s: usize,
        slab: &mut Slab<'_>,
        st: &mut DecState<'e>,
        dead: &mut bool,
        rec: Option<&mut Vec<Checkpoint>>,
    ) {
        let plan = self.pipes[p][s].plan;
        let r = self.geo.scan_rows(plan, slab.k);
        if r.is_empty() || *dead || self.halted() {
            return;
        }
        let res = rows(&self.geo, plan, st, r, slab, rec, self.cancel);
        if !self.note(res, plan, slab.k, st.br.pos) {
            *dead = true;
        }
    }

    /// A band left pipeline `p`: hand its storage to the output join; returns the bands of
    /// components that are now complete.
    fn finish(&self, p: usize, mut slab: Slab<'b>) -> Vec<OutJob<'b>> {
        let Some(join) = &self.join else {
            return Vec::new();
        };
        let k = slab.k;
        let need_ac = !self.out.dc_only;
        let mut ready = Vec::new();
        let mut j = join.lock().unwrap_or_else(|e| e.into_inner());
        let comps: Range<usize> = if p == 0 { 0..self.geo.ncomp } else { p - 1..p };
        for ci in comps {
            let buf = slab.bufs[ci].take();
            let sl = &mut j[ci][k];
            if p == 0 {
                sl.dc = buf.map(|b| &*b);
            } else {
                sl.ac = buf;
            }
            if sl.dc.is_some()
                && (sl.ac.is_some() || !need_ac)
                && let Some(plane) = sl.plane.take()
            {
                ready.push(OutJob {
                    ci,
                    k,
                    dc: sl.dc.unwrap(),
                    ac: sl.ac.take(),
                    plane,
                    clear: false,
                });
            }
        }
        ready
    }

    fn output(&self, j: OutJob) -> bool {
        out_band(&self.out, &self.geo, j)
    }

    /// Move `slab` into stage `s` of pipeline `p`. With `inline`, parallel stages run on the
    /// current (rayon) thread; otherwise they are spawned.
    fn deliver<'s>(
        &'s self,
        rs: &rayon::Scope<'s>,
        p: usize,
        mut s: usize,
        mut slab: Slab<'b>,
        inline: bool,
    ) where
        'b: 's,
    {
        let stages = &self.pipes[p];
        if inline {
            while s < stages.len() && matches!(stages[s].mode, Mode::Par(_)) {
                self.run_par(p, s, &mut slab);
                s += 1;
            }
        }
        if s == stages.len() {
            for j in self.finish(p, slab) {
                if inline {
                    self.output(j);
                } else {
                    rs.spawn(move |_| {
                        let _g = PanicGuard(&self.abort);
                        self.output(j);
                    });
                }
            }
            return;
        }
        match stages[s].mode {
            Mode::Par(_) => rs.spawn(move |rs| {
                let _g = PanicGuard(&self.abort);
                self.deliver(rs, p, s, slab, true)
            }),
            Mode::Seq | Mode::Refine => {
                // The receiver only disappears after an abort; the slab is dropped then.
                let _ = self.tx[p][s].as_ref().expect("seq stage").send(slab);
            }
        }
    }

    /// Body of a sequential stage's thread: decode bands in order as they arrive.
    ///
    /// A refinement stage (`Mode::Refine`) also speculates: whenever enough bands are queued
    /// (their nonzero history is final once they arrive), a pool job derives the row
    /// checkpoints of those bands from the true state where the sequential decode will start
    /// them (`refine::spec_refine`) while the decode goes on. If the checkpoints arrive before
    /// the decode reached the bands, the bands not yet decoded are decoded in parallel and the
    /// sequential decode continues after them from the last checkpoint.
    ///
    /// Speculation pays off only when the image's statistics let chunks synchronise quickly
    /// (the parse is then several times faster than the sequential decode). Once it has done so
    /// in this decode, later refinement stages stop decoding sequentially until every band has
    /// arrived and speculate on the whole scan at once (the sequential decode waits for the
    /// result); a slow or failed speculation switches back to streaming.
    fn seq_thread<'s>(
        &'s self,
        rs: &rayon::Scope<'s>,
        p: usize,
        s: usize,
        rx: mpsc::Receiver<Slab<'b>>,
        start: Checkpoint,
        want_rec: bool,
    ) -> Option<Vec<Checkpoint>>
    where
        'b: 's,
    {
        let _g = PanicGuard(&self.abort);
        let nb = self.geo.bands.len();
        let mut pending: Vec<Option<Slab<'b>>> = (0..nb).map(|_| None).collect();
        let mut next = 0usize;
        let mut st = DecState::from_cp(self.data, &start);
        let mut rec = want_rec.then(Vec::new);
        let mut dead = false;
        let mut refine = matches!(self.pipes[p][s].mode, Mode::Refine);
        let mut spec: Option<SpecJob> = None;
        // Sequential decode: bytes and time (for the rate that speculation is judged by).
        let (mut seq_bytes, mut seq_time) = (0usize, Duration::ZERO);
        let whole = || {
            #[cfg(test)]
            if REFINE_WAIT.load(Ordering::Relaxed) {
                return true;
            }
            self.refine_good.load(Ordering::Relaxed)
        };
        let stop = |spec: &Option<SpecJob>| {
            if let Some(j) = spec {
                j.stop.store(true, Ordering::Relaxed);
            }
        };
        loop {
            // Decode the bands that are next in order.
            while next < nb && pending[next].is_some() {
                if refine
                    && spec.is_none()
                    && whole()
                    && pending[next..].iter().any(Option::is_none)
                {
                    // Wait for every band, then speculate on the whole rest of the scan.
                    break;
                }
                if refine && spec.is_none() && !dead {
                    spec = self.refine_start(rs, p, s, &mut pending, next, &st);
                    if let Some(job) = spec.as_mut() {
                        job.wait = whole();
                    }
                }
                if let Some(job) = &spec {
                    let res = if job.wait {
                        job.rx.recv().map_err(|_| mpsc::TryRecvError::Disconnected)
                    } else {
                        job.rx.try_recv()
                    };
                    if let Ok((ref cps, took)) = res {
                        self.judge(cps.as_deref(), job, took, (seq_bytes, seq_time));
                    }
                    match res.map(|r| r.0) {
                        Ok(Some(cps)) => {
                            if !dead && next < job.end {
                                let (k0, k1) = (next, job.end);
                                if let Some(cp) = self.refine_switch(
                                    rs,
                                    (p, s),
                                    &mut pending[k0..k1],
                                    job,
                                    &cps,
                                    &mut rec,
                                ) {
                                    tracing::debug!(target: "jpeg", comp = p, stage = s, bands = ?(k0..k1), "refine: speculative bands decoded in parallel");
                                    st = DecState::from_cp(self.data, &cp);
                                    next = k1;
                                }
                            }
                            spec = None;
                            continue;
                        }
                        Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                            // Corrupt data (or cancelled): no more speculation in this scan.
                            tracing::debug!(target: "jpeg", comp = p, stage = s, "refine: speculation failed, sequential for the rest of the scan");
                            #[cfg(test)]
                            REFINE_STATS[1].fetch_add(1, Ordering::Relaxed);
                            refine = false;
                            spec = None;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                }
                let Some(mut slab) = pending[next].take() else {
                    break;
                };
                let (t0, b0) = (Instant::now(), st.br.pos);
                self.run_seq(p, s, &mut slab, &mut st, &mut dead, rec.as_mut());
                if refine {
                    seq_time += t0.elapsed();
                    seq_bytes += st.br.pos.saturating_sub(b0);
                }
                next += 1;
                self.deliver(rs, p, s + 1, slab, false);
                if spec.as_ref().is_some_and(|j| next >= j.end) {
                    // The decode passed the speculated bands.
                    stop(&spec);
                    spec = None;
                }
            }
            if next == nb {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(slab) => {
                    let k = slab.k;
                    pending[k] = Some(slab);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if self.abort.load(Ordering::Relaxed) {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        stop(&spec);
        rec
    }

    /// Start speculating on refinement stage `s` of pipeline `p` over the queued bands
    /// `next..` (up to the first band not yet arrived), given the true state `st` at the start
    /// of band `next`: the bands' nonzero history is copied on the pool (the sequential decode
    /// goes on changing it) and handed to a pool job that sends the row checkpoints of those
    /// bands (`None` if speculation failed). `None` if the queued bands are too short to be
    /// worth it.
    fn refine_start<'s>(
        &'s self,
        rs: &rayon::Scope<'s>,
        p: usize,
        s: usize,
        pending: &mut [Option<Slab<'b>>],
        next: usize,
        st: &DecState,
    ) -> Option<SpecJob>
    where
        'b: 's,
    {
        let plan = self.pipes[p][s].plan;
        let nb = pending.len();
        let end = (next..nb).find(|&k| pending[k].is_none()).unwrap_or(nb);
        let first = self.geo.scan_rows(plan, next).start;
        let last = self.geo.scan_rows(plan, end - 1).end;
        if end - next < 2 || last <= first || self.halted() {
            return None;
        }
        // Estimated bytes of the queued rows: the average so far (or of the whole scan).
        let from = st.checkpoint();
        let sc = plan.scan;
        let ux = plan.units_x as u64;
        let (done, left) = (first as u64 * ux, (plan.rows - first) as u64 * ux);
        let rate = if done >= ux * 8 {
            from.pos.saturating_sub(sc.data_start) as f64 / done as f64
        } else {
            (sc.data_end - sc.data_start) as f64 / (plan.rows as u64 * ux).max(1) as f64
        };
        let blocks = (last - first) as u64 * ux;
        let est = (rate * blocks as f64) as usize;
        if est < REFINE_MIN || sc.data_end.saturating_sub(from.pos) < REFINE_MIN {
            return None;
        }
        let len = (blocks < left).then_some(est);
        tracing::debug!(target: "jpeg", scan = plan.si, bands = ?(next..end), rows = ?(first..last), est_bytes = est, "refine: speculating on queued bands");
        // Copy the history band by band on the pool; the bands come back over `btx`.
        let (btx, brx) = mpsc::channel();
        for (k, slot) in pending.iter_mut().enumerate().take(end).skip(next) {
            let slab = slot.take()?;
            let btx = btx.clone();
            rs.spawn(move |_| {
                let _g = PanicGuard(&self.abort);
                let m = self.band_masks(plan, &slab);
                let _ = btx.send((k, slab, m));
            });
        }
        drop(btx);
        let mut masks: Vec<Option<Vec<u64>>> = (next..end).map(|_| None).collect();
        let mut ok = true;
        for _ in next..end {
            let (k, slab, m) = loop {
                match brx.recv_timeout(Duration::from_millis(20)) {
                    Ok(x) => break x,
                    Err(mpsc::RecvTimeoutError::Timeout) if !self.halted() => {}
                    // Aborted (a panic dropped a band): nothing is decoded any more.
                    Err(_) => return None,
                }
            };
            pending[k] = Some(slab);
            ok &= m.is_some();
            masks[k - next] = m;
        }
        if !ok {
            return None;
        }
        let masks: Vec<Vec<u64>> = masks.into_iter().flatten().collect();
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel(1);
        let job_flag = flag.clone();
        rs.spawn(move |_| {
            let _g = PanicGuard(&self.abort);
            let t0 = Instant::now();
            let ux = ux as usize;
            let rows: Vec<&[u64]> = masks.iter().flat_map(|m| m.chunks(ux.max(1))).collect();
            let cancel: Vec<&AtomicBool> = [Some(&*job_flag), Some(&self.abort), self.cancel]
                .into_iter()
                .flatten()
                .collect();
            let o = refine::Opts {
                nchunks: self.threads * 2,
                min_chunk: refine::REFINE_MIN_CHUNK,
                tune: refine::Tune::DEFAULT,
                cancel: &cancel,
                len,
            };
            let m = refine::Masks { rows, ux };
            let cps = refine::spec_refine(self.data, plan, &m, (first, &from), &o)
                .filter(|c| c.len() == (last - first) as usize + 1);
            let _ = tx.send((cps, t0.elapsed()));
        });
        Some(SpecJob {
            rx,
            stop: flag,
            first,
            end,
            bytes: est,
            wait: false,
        })
    }

    /// Records whether speculation `job` paid off: it succeeded and took less than half the
    /// time the sequential decode (at its rate so far, `seq` = bytes and time) needs for the
    /// same rows. Later refinement stages speculate on whole scans only while it does.
    fn judge(
        &self,
        cps: Option<&[Checkpoint]>,
        job: &SpecJob,
        took: Duration,
        seq: (usize, Duration),
    ) {
        let mut rate = f64::from_bits(self.seq_rate.load(Ordering::Relaxed));
        if seq.0 >= REFINE_MIN && !seq.1.is_zero() {
            rate = seq.0 as f64 / seq.1.as_secs_f64();
            self.seq_rate.store(rate.to_bits(), Ordering::Relaxed);
        }
        let good =
            cps.is_some() && rate > 0.0 && took.as_secs_f64() * 2.0 < job.bytes as f64 / rate;
        tracing::debug!(
            target: "jpeg",
            synced = cps.is_some(),
            rows = ?(job.first..job.first + cps.map_or(0, |c| c.len().saturating_sub(1)) as u32),
            ms = took.as_secs_f64() * 1e3,
            seq_mb_s = rate / 1e6,
            est_bytes = job.bytes,
            whole_scans_next = good,
            "refine: speculation result"
        );
        self.refine_good.store(good, Ordering::Relaxed);
    }

    /// Nonzero history of the rows of `plan` in band `slab` (scan order).
    fn band_masks(&self, plan: &ScanPlan, slab: &Slab<'b>) -> Option<Vec<u64>> {
        let c = plan.comps[0].0;
        let ux = plan.units_x as usize;
        let bwp = self.geo.bwp[c] as usize;
        let r = self.geo.scan_rows(plan, slab.k);
        let row0 = self.geo.bands[slab.k].0 * self.geo.v[c];
        let mut out = Vec::new();
        out.try_reserve_exact(ux * r.len()).ok()?;
        for row in r {
            let base = (row - row0) as usize * bwp;
            if let Some(buf) = slab.bufs[c].as_deref() {
                let line = buf.get(base * 64..(base + ux) * 64)?.as_chunks::<64>().0;
                out.extend(line.iter().map(nzmask));
            } else {
                out.extend_from_slice(slab.masks[c].as_deref()?.get(base..base + ux)?);
            }
        }
        Some(out)
    }

    /// Speculation `job` on refinement stage `s` of pipeline `p` succeeded with the
    /// checkpoints `cps` (rows from `job.first` on, then the state after them): decode `bands`
    /// (the speculated bands the sequential decode has not reached) in parallel and append the
    /// checkpoints of their rows to `rec`. Returns the state after them.
    fn refine_switch<'s>(
        &'s self,
        rs: &rayon::Scope<'s>,
        (p, s): (usize, usize),
        bands: &mut [Option<Slab<'b>>],
        job: &SpecJob,
        cps: &[Checkpoint],
        rec: &mut Option<Vec<Checkpoint>>,
    ) -> Option<Checkpoint>
    where
        'b: 's,
    {
        let plan = self.pipes[p][s].plan;
        let k0 = bands.first()?.as_ref()?.k;
        let r0 = self.geo.scan_rows(plan, k0).start;
        let rel = |row: u32| (row - job.first) as usize;
        if let Some(rec) = rec.as_mut() {
            rec.extend_from_slice(&cps[rel(r0)..cps.len() - 1]);
        }
        #[cfg(test)]
        REFINE_STATS[0].fetch_add(1, Ordering::Relaxed);
        // Nothing later needs this stage's coefficients when only the index is built.
        let skip = self.no_output && s + 1 == self.pipes[p].len();
        for slab in bands.iter_mut().filter_map(Option::take) {
            let r = self.geo.scan_rows(plan, slab.k);
            let cp = cps[rel(r.start).min(cps.len() - 1)];
            rs.spawn(move |rs| {
                let _g = PanicGuard(&self.abort);
                let mut slab = slab;
                if !skip {
                    self.run_at(p, s, &mut slab, &cp);
                }
                self.deliver(rs, p, s + 1, slab, true)
            });
        }
        cps.last().copied()
    }
}

// ---------------------------------------------------------------------------------------------
// Entry point

/// Decode a progressive image into `planes` (one per frame component, geometry `g`).
pub(crate) fn decode(
    p: &Parsed,
    data: &[u8],
    g: &OutGeom,
    planes: &mut [&mut [u8]],
    cp: &CoefParams,
) -> Result<CoefOutcome> {
    let f = &p.frame;
    let ncomp = f.comps.len();
    let dc_only = g.n == 1 && f.precision == 8 && !cp.want_index;
    let index_mode = cp.index.is_some() && !cp.want_index;
    let (m_lo, m_hi) = if index_mode {
        (g.my0, g.my1)
    } else if cp.want_index {
        (0, f.mcus_y)
    } else {
        (0, g.my1)
    };
    let plans = plan_scans(p, g, dc_only, true);
    if let Some(ix) = cp.index.filter(|_| index_mode) {
        if ix.len() != p.scans.len() {
            return Err(JpegError::IndexMismatch);
        }
        for pl in &plans {
            if ix[pl.si].len() != pl.rows as usize {
                return Err(JpegError::IndexMismatch);
            }
        }
    }
    let threads = cp.threads.max(1);

    // Bands of MCU rows: ~8K blocks each, and enough of them to keep every thread busy.
    let blocks_per_row: u64 =
        f.comps.iter().map(|c| (c.h * c.v) as u64).sum::<u64>() * f.mcus_x as u64;
    let nrows = m_hi.saturating_sub(m_lo);
    let mut bs = (8192 / blocks_per_row.max(1)).max(1) as u32;
    if threads > 1 {
        bs = bs.min(nrows.div_ceil(threads as u32 * 4).max(1));
    }
    let bands: Vec<(u32, u32)> = (m_lo..m_hi)
        .step_by(bs as usize)
        .map(|m| (m, (m + bs).min(m_hi)))
        .collect();
    let nb = bands.len();
    let mut geo = Geo {
        bands,
        h: [0; 4],
        v: [0; 4],
        bwp: [0; 4],
        ncomp,
    };
    for (ci, c) in f.comps.iter().enumerate() {
        geo.h[ci] = c.h;
        geo.v[ci] = c.v;
        geo.bwp[ci] = c.bw_pad;
    }

    // Stages.
    let stage_mode = |pl: &ScanPlan| -> Mode {
        match cp.index {
            Some(ix) if index_mode => Mode::Par(ix[pl.si].clone()),
            _ => Mode::Seq,
        }
    };
    let mut pipes: Vec<Vec<Stage>> = (0..=ncomp).map(|_| Vec::new()).collect();
    for pl in &plans {
        let pi = match pl.kind {
            Kind::DcFirst | Kind::DcRefine => 0,
            _ => 1 + pl.comps[0].0,
        };
        pipes[pi].push(Stage {
            plan: pl,
            mode: stage_mode(pl),
        });
    }
    if dc_only {
        pipes.truncate(1);
    }
    // Speculative checkpoints make AC first scans parallel stages.
    if threads > 1 && !index_mode {
        let cand: Vec<(usize, usize)> = pipes
            .iter()
            .enumerate()
            .flat_map(|(pi, st)| {
                st.iter()
                    .enumerate()
                    .filter(|(_, s)| s.plan.kind == Kind::AcFirst)
                    .map(move |(si, _)| (pi, si))
            })
            .collect();
        let found: Vec<Option<Vec<Checkpoint>>> = cand
            .par_iter()
            .map(|&(pi, si)| {
                spec_ac(
                    data,
                    pipes[pi][si].plan,
                    threads * 2,
                    AC_MIN_CHUNK,
                    AC_MARGIN,
                    cp.cancel,
                )
            })
            .collect();
        for ((pi, si), cps) in cand.into_iter().zip(found) {
            let scan = pipes[pi][si].plan.si;
            if let Some(cps) = cps {
                tracing::debug!(target: "jpeg", scan, "AC first scan: speculative row checkpoints found, parallel");
                pipes[pi][si].mode = Mode::Par(cps);
            } else {
                tracing::debug!(target: "jpeg", scan, "AC first scan: speculation failed or not applicable, sequential");
            }
        }
        // Refinement scans long enough to speculate on, over every row of the scan.
        for st in pipes.iter_mut() {
            for s in st.iter_mut() {
                let pl = s.plan;
                let len = pl.scan.data_end.saturating_sub(pl.scan.data_start);
                let covered = geo.bands.first().is_some_and(|b| b.0 == 0)
                    && geo.scan_rows(pl, nb - 1).end == pl.rows;
                if pl.kind == Kind::AcRefine
                    && pl.scan.restart_interval == 0
                    && pl.comps[0].4.is_some()
                    && len >= refine::REFINE_MIN
                    && covered
                    && matches!(s.mode, Mode::Seq)
                {
                    s.mode = Mode::Refine;
                }
            }
        }
    }
    if cp.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err(JpegError::Cancelled);
    }
    if tracing::enabled!(target: "jpeg", tracing::Level::DEBUG) {
        let stages: Vec<String> = pipes
            .iter()
            .flatten()
            .map(|s| {
                let m = match &s.mode {
                    Mode::Par(_) => "par",
                    Mode::Seq => "seq",
                    Mode::Refine => "refine",
                };
                format!("{}:{:?}:{m}", s.plan.si, s.plan.kind)
            })
            .collect();
        tracing::debug!(
            target: "jpeg",
            threads,
            bands = nb,
            rows = ?(m_lo..m_hi),
            index_mode,
            want_index = cp.want_index,
            no_output = cp.no_output,
            ?stages,
            "progressive pipeline"
        );
    }
    // Index building without output: trailing parallel stages already have all their row
    // checkpoints and nothing later depends on their coefficients, so they are not decoded.
    let mut spare: Vec<(usize, Vec<Checkpoint>)> = Vec::new();
    if cp.no_output {
        for st in pipes.iter_mut() {
            let keep = st
                .iter()
                .rposition(|s| !matches!(s.mode, Mode::Par(_)))
                .map_or(0, |i| i + 1);
            for s in st.drain(keep..) {
                if let Mode::Par(v) = s.mode {
                    spare.push((s.plan.si, v));
                }
            }
        }
    }

    // Coefficient storage: DC per block, AC 64 per block (zigzag order, index 0 unused).
    // Index-only builds keep no DC values (DC scans are only parsed) and, for components that
    // still have AC stages to run, one nonzero mask per block (8 B instead of 128 B).
    // The parallel engine keeps every band's storage (bands are in flight in all scans at
    // once); the single-thread engine decodes band by band (every scan of a band, then its
    // output), so it needs storage for one band only, zeroed and reused for every band. That
    // keeps the coefficients in cache across scans and saves the page faults of a whole-image
    // buffer (a third of a single-threaded 50 MP 4:4:4 decode).
    let parallel = threads > 1;
    let band_rows = geo.bands.clone();
    let band_blocks = |ci: usize, k: usize| {
        let (m0, m1) = band_rows[k];
        (m1 - m0) as usize * f.comps[ci].v as usize * f.comps[ci].bw_pad as usize
    };
    let mut dc: Vec<Vec<i16>> = Vec::with_capacity(ncomp);
    let mut ac: Vec<Vec<i16>> = Vec::with_capacity(ncomp);
    let mut nzm: Vec<Vec<u64>> = Vec::with_capacity(ncomp);
    for (ci, c) in f.comps.iter().enumerate() {
        let blocks = if parallel {
            (nrows * c.v) as usize * c.bw_pad as usize
        } else {
            (0..nb).map(|k| band_blocks(ci, k)).max().unwrap_or(0)
        };
        let has_ac = pipes.get(1 + ci).is_some_and(|st| !st.is_empty());
        if cp.no_output {
            dc.push(Vec::new());
            ac.push(Vec::new());
            nzm.push(if has_ac {
                alloc_u64(blocks)?
            } else {
                Vec::new()
            });
        } else {
            dc.push(take_i16(cp.reuse, blocks)?);
            ac.push(if dc_only {
                Vec::new()
            } else {
                take_i16(cp.reuse, blocks * 64)?
            });
            nzm.push(Vec::new());
        }
    }

    // Output planes per component and band.
    let mut plane_bands: Vec<Vec<Option<&mut [u8]>>> = Vec::new();
    if !cp.no_output {
        for (ci, plane) in planes.iter_mut().enumerate() {
            let row_bytes = g.mcu_row_bytes(ci);
            let mut rest: &mut [u8] = &mut plane[..];
            let mut v: Vec<Option<&mut [u8]>> = Vec::with_capacity(nb);
            for &(m0, m1) in &geo.bands {
                let a = m0.max(g.my0);
                let b = m1.min(g.my1);
                let mut sl = None;
                if b > a {
                    let take = ((b - a) as usize * row_bytes).min(rest.len());
                    let (x, y) = std::mem::take(&mut rest).split_at_mut(take);
                    sl = Some(x);
                    rest = y;
                }
                v.push(sl);
            }
            plane_bands.push(v);
        }
    }
    // The parallel engine joins a band's DC and AC storage with its planes in `join`; the
    // single-thread one outputs every band itself.
    let (join, mut single_planes) = if cp.no_output {
        (None, Vec::new())
    } else if parallel {
        let j: Vec<Vec<Slot>> = plane_bands
            .into_iter()
            .map(|v| {
                v.into_iter()
                    .map(|plane| Slot {
                        plane,
                        ..Default::default()
                    })
                    .collect()
            })
            .collect();
        (Some(Mutex::new(j)), Vec::new())
    } else {
        (None, plane_bands)
    };

    // Region decodes with an index and column checkpoints (whole-width decodes record them).
    let cols = match (cp.index, cp.cols) {
        (Some(ix), Some(cache)) if index_mode && !cp.no_output => {
            let stage_plans: Vec<&ScanPlan> = pipes.iter().flatten().map(|s| s.plan).collect();
            ColPlan::new(cache, ix, g, &geo, &stage_plans, p.scans.len())
        }
        _ => None,
    };
    let start_cp = |pl: &ScanPlan| Checkpoint::at(pl.scan.data_start, pl.scan.restart_interval);
    let want = cp.want_index;
    // Channels for sequential stages (parallel engine only).
    let mut rxs = Vec::new();
    let tx: Vec<Vec<Option<mpsc::Sender<Slab>>>> = pipes
        .iter()
        .enumerate()
        .map(|(pi, st)| {
            st.iter()
                .enumerate()
                .map(|(si, s)| {
                    if parallel && !matches!(s.mode, Mode::Par(_)) {
                        let (t, r) = mpsc::channel();
                        rxs.push((pi, si, r, start_cp(s.plan)));
                        Some(t)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let npipes = pipes.len();
    let engine = Engine {
        data,
        pipes,
        geo,
        cancel: cp.cancel,
        cancelled: AtomicBool::new(false),
        truncated: AtomicBool::new(false),
        abort: AtomicBool::new(false),
        tx,
        join,
        out: OutCtx {
            p,
            g,
            simd: cp.simd,
            dc_only,
        },
        no_output: cp.no_output,
        refine_good: AtomicBool::new(false),
        seq_rate: AtomicU64::new(0),
        threads,
        cols,
    };

    // Recorded checkpoints of sequential stages: (pipe, stage, rows).
    let mut recorded: Vec<(usize, usize, Option<Vec<Checkpoint>>)> = Vec::new();
    if parallel {
        // Slabs per pipeline and band.
        let mut slabs: Vec<Vec<Slab>> = (0..npipes)
            .map(|_| {
                (0..nb)
                    .map(|k| Slab {
                        k,
                        bufs: Default::default(),
                        masks: Default::default(),
                    })
                    .collect()
            })
            .collect();
        for (ci, ((d, a), m)) in dc
            .iter_mut()
            .zip(ac.iter_mut())
            .zip(nzm.iter_mut())
            .enumerate()
        {
            let mut rest_d: &mut [i16] = d;
            let mut rest_a: &mut [i16] = a;
            let mut rest_m: &mut [u64] = m;
            for k in band_rows.iter().enumerate().map(|(k, _)| k) {
                let blocks = band_blocks(ci, k);
                if !rest_d.is_empty() {
                    let (x, y) = std::mem::take(&mut rest_d).split_at_mut(blocks);
                    slabs[0][k].bufs[ci] = Some(x);
                    rest_d = y;
                }
                if !rest_a.is_empty() {
                    let (x, y) = std::mem::take(&mut rest_a).split_at_mut(blocks * 64);
                    slabs[1 + ci][k].bufs[ci] = Some(x);
                    rest_a = y;
                }
                if !rest_m.is_empty() {
                    let (x, y) = std::mem::take(&mut rest_m).split_at_mut(blocks);
                    slabs[1 + ci][k].masks[ci] = Some(x);
                    rest_m = y;
                }
            }
        }
        let eng = &engine;
        let mut panicked = None;
        rayon::scope(|rs| {
            std::thread::scope(|ts| {
                let waiter = std::thread::current();
                let handles: Vec<_> = rxs
                    .into_iter()
                    .map(|(pi, si, rx, start)| {
                        let waiter = waiter.clone();
                        let h = specular_thread::spawn_scoped(ts, "dec-jpeg-seq", move || {
                            let r = eng.seq_thread(rs, pi, si, rx, start, want);
                            waiter.unpark();
                            r
                        });
                        (pi, si, h)
                    })
                    .collect();
                let mut slabs: Vec<std::vec::IntoIter<Slab>> =
                    slabs.into_iter().map(|v| v.into_iter()).collect();
                for _ in 0..nb {
                    for (pi, it) in slabs.iter_mut().enumerate() {
                        let slab = it.next().unwrap();
                        eng.deliver(rs, pi, 0, slab, false);
                    }
                }
                // This is (usually) a pool thread. Blocking it in `join` while the stage
                // threads wait for pool jobs deadlocks once concurrent decodes sharing the pool
                // have blocked every worker this way, so it runs pool jobs while it waits.
                while !handles.iter().all(|(_, _, h)| h.is_finished()) {
                    match rayon::yield_now() {
                        Some(rayon::Yield::Executed) => {}
                        _ => std::thread::park_timeout(Duration::from_micros(200)),
                    }
                }
                for (pi, si, h) in handles {
                    match h.join() {
                        Ok(r) => recorded.push((pi, si, r)),
                        Err(e) => {
                            eng.abort.store(true, Ordering::Relaxed);
                            panicked = Some(e);
                        }
                    }
                }
            });
        });
        if let Some(e) = panicked {
            std::panic::resume_unwind(e);
        }
    } else {
        // Single thread: band-major order (every scan of a band, then its output) over one
        // band's storage.
        let mut states: Vec<Vec<SeqState>> = engine
            .pipes
            .iter()
            .map(|st| {
                st.iter()
                    .map(|s| {
                        (
                            DecState::from_cp(data, &start_cp(s.plan)),
                            false,
                            want.then(Vec::new),
                        )
                    })
                    .collect()
            })
            .collect();
        drop(rxs); // empty: no channels without the parallel engine
        // Output zeroes the AC blocks it reads, so a band's AC storage needs a memset only
        // after a band that was not (fully) output: region decodes, bands outside the region.
        let mut ac_dirty = vec![false; ncomp];
        for k in 0..nb {
            let mut slabs: Vec<Slab> = (0..npipes)
                .map(|_| Slab {
                    k,
                    bufs: Default::default(),
                    masks: Default::default(),
                })
                .collect();
            for (ci, ((d, a), m)) in dc
                .iter_mut()
                .zip(ac.iter_mut())
                .zip(nzm.iter_mut())
                .enumerate()
            {
                let n = band_blocks(ci, k);
                // The first band's storage is fresh (zeroed).
                if !d.is_empty() {
                    let b = &mut d[..n];
                    if k > 0 {
                        b.fill(0);
                    }
                    slabs[0].bufs[ci] = Some(b);
                }
                if !a.is_empty() {
                    let b = &mut a[..n * 64];
                    if ac_dirty[ci] {
                        b.fill(0);
                    }
                    ac_dirty[ci] = true;
                    slabs[1 + ci].bufs[ci] = Some(b);
                }
                if !m.is_empty() {
                    let b = &mut m[..n];
                    if k > 0 {
                        b.fill(0);
                    }
                    slabs[1 + ci].masks[ci] = Some(b);
                }
            }
            for (pi, slab) in slabs.iter_mut().enumerate() {
                for (si, (st, dead, rec)) in states[pi].iter_mut().enumerate() {
                    if matches!(engine.pipes[pi][si].mode, Mode::Par(_)) {
                        engine.run_par(pi, si, slab);
                    } else {
                        engine.run_seq(pi, si, slab, st, dead, rec.as_mut());
                    }
                }
            }
            drop(slabs);
            for (ci, bands) in single_planes.iter_mut().enumerate() {
                let Some(plane) = bands[k].take() else {
                    continue;
                };
                let n = band_blocks(ci, k);
                let a = &mut ac[ci];
                let full = engine.output(OutJob {
                    ci,
                    k,
                    dc: &dc[ci][..n],
                    ac: (!a.is_empty()).then(|| &mut a[..n * 64]),
                    plane,
                    clear: true,
                });
                if full {
                    ac_dirty[ci] = false;
                }
            }
            if engine.halted() {
                break;
            }
        }
        for (pi, st) in states.into_iter().enumerate() {
            for (si, (_, _, rec)) in st.into_iter().enumerate() {
                recorded.push((pi, si, rec));
            }
        }
    }

    if engine.cancelled.load(Ordering::Relaxed)
        || cp.cancel.is_some_and(|c| c.load(Ordering::Relaxed))
    {
        return Err(JpegError::Cancelled);
    }
    let truncated = engine.truncated.load(Ordering::Relaxed);
    let index = if want && !truncated {
        let mut index: ScanIndex = vec![Vec::new(); p.scans.len()];
        for (si, v) in spare {
            index[si] = v;
        }
        // The rest of the engine (borrowing the coefficient storage) drops here.
        let pipes = {
            let e = engine;
            let Engine { pipes, .. } = e;
            pipes
        };
        let mut recorded = recorded;
        for (pi, st) in pipes.into_iter().enumerate() {
            for (si, s) in st.into_iter().enumerate() {
                let v = match s.mode {
                    Mode::Par(v) => v,
                    Mode::Seq | Mode::Refine => recorded
                        .iter_mut()
                        .find(|r| r.0 == pi && r.1 == si)
                        .and_then(|r| r.2.take())
                        .unwrap_or_default(),
                };
                index[s.plan.si] = v;
            }
        }
        Some(index)
    } else {
        drop(engine);
        None
    };
    if let Some(pool) = cp.reuse {
        let mut pool = pool.lock().unwrap_or_else(|e| e.into_inner());
        pool.extend(dc.into_iter().chain(ac).filter(|v| v.capacity() > 0));
    }
    Ok(CoefOutcome { truncated, index })
}

/// A zeroed buffer of `n` coefficients: a large enough one from `reuse`, or a new one.
fn take_i16(reuse: Option<&Mutex<Vec<Vec<i16>>>>, n: usize) -> Result<Vec<i16>> {
    if let Some(pool) = reuse {
        let mut pool = pool.lock().unwrap_or_else(|e| e.into_inner());
        // Best fit: a DC request must not take a stripe's AC buffer.
        let fit = (0..pool.len())
            .filter(|&i| pool[i].capacity() >= n)
            .min_by_key(|&i| pool[i].capacity());
        if let Some(i) = fit {
            let mut v = pool.swap_remove(i);
            drop(pool);
            v.clear();
            v.resize(n, 0);
            return Ok(v);
        }
    }
    alloc_i16(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference refinement decoder (libjpeg's coefficient walk), natural == zigzag storage here.
    fn refine_ref(
        br: &mut BitReader,
        t: &HuffTable,
        ss: u32,
        se: u32,
        al: u32,
        eobrun: &mut u32,
        blk: &mut [i16; 64],
    ) {
        let p1 = 1i32 << al;
        let m1 = -1i32 << al;
        let one = |br: &mut BitReader, c: &mut i16| {
            br.ensure(1);
            if br.get(1) != 0 && (*c as i32 & p1) == 0 {
                *c = (*c as i32 + if *c >= 0 { p1 } else { m1 }) as i16;
            }
        };
        let mut k = ss;
        if *eobrun == 0 {
            while k <= se {
                br.ensure(32);
                let sym = br.decode_symbol(t);
                let mut r = sym >> 4;
                let s = sym & 15;
                let mut val = 0i32;
                if s != 0 {
                    val = if br.get(1) != 0 { p1 } else { m1 };
                } else if r != 15 {
                    *eobrun = 1 << r;
                    if r != 0 {
                        *eobrun += br.get(r);
                    }
                    break;
                }
                while k <= se {
                    if blk[k as usize] != 0 {
                        one(br, &mut blk[k as usize]);
                    } else {
                        if r == 0 {
                            break;
                        }
                        r -= 1;
                    }
                    k += 1;
                }
                if val != 0 {
                    blk[k.min(63) as usize] = val as i16;
                }
                k += 1;
            }
        }
        if *eobrun > 0 {
            while k <= se {
                if blk[k as usize] != 0 {
                    one(br, &mut blk[k as usize]);
                }
                k += 1;
            }
            *eobrun -= 1;
        }
    }

    #[test]
    fn mask_refine_matches_reference() {
        // A table with every (run, size<=1) symbol plus EOBn/ZRL, so random bits form valid
        // symbol streams.
        let mut vals: Vec<u8> = Vec::new();
        for r in 0..15u8 {
            vals.push(r << 4); // EOBr
            vals.push((r << 4) | 1);
        }
        vals.push(0xF0);
        vals.push(0xF1);
        let mut counts = [0u8; 16];
        counts[4] = 16; // 16 codes of length 5
        counts[5] = (vals.len() - 16) as u8; // the rest length 6
        let t = HuffTable::new(&counts, &vals, true).unwrap();
        let mut seed = 0x1234_5678u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for iter in 0..4000 {
            let data: Vec<u8> = (0..512)
                .map(|_| {
                    let b = rnd() as u8;
                    if b == 0xFF { 0xFE } else { b }
                })
                .collect();
            let (ss, se) = if iter % 4 == 0 {
                (1, 63)
            } else {
                let ss = 1 + rnd() % 20;
                (ss, (ss + rnd() % 50).min(63))
            };
            let al = rnd() % 3;
            let mut a = [[0i16; 64]; 8];
            let dens = 1 + rnd() % 4;
            for blk in a.iter_mut() {
                for c in blk.iter_mut().skip(1) {
                    if rnd() % dens == 0 {
                        let v = (rnd() % 7) as i32 - 3;
                        *c = ((v * 2) << al) as i16;
                    }
                }
            }
            let pos = |st: &DecState| {
                st.br.pos as i64 * 8 + st.br.zero_fill as i64 * 8 - st.br.bits as i64
            };
            let mut rb = DecState::from_cp(&data, &Checkpoint::at(0, 0));
            let mut ra = DecState::from_cp(&data, &Checkpoint::at(0, 0));
            for (i, blk) in a.iter_mut().enumerate() {
                let mut b = *blk;
                refine_ref(&mut ra.br, &t, ss, se, al, &mut ra.eobrun, blk);
                ac_refine(&mut rb.br, &t, ss, se, al, &mut rb.eobrun, &mut b);
                assert_eq!(
                    (*blk, ra.eobrun, pos(&ra)),
                    (b, rb.eobrun, pos(&rb)),
                    "iter {iter} block {i}"
                );
            }
        }
    }

    #[test]
    fn skip_parse_matches_ac_first() {
        let mut vals: Vec<u8> = Vec::new();
        for r in 0..15u8 {
            vals.push(r << 4);
            vals.push((r << 4) | 1);
            vals.push((r << 4) | 3);
        }
        vals.push(0xF0);
        let mut counts = [0u8; 16];
        counts[3] = 6;
        counts[4] = 10;
        counts[5] = 10;
        counts[6] = 19;
        counts[7] = 1;
        assert_eq!(vals.len(), 46);
        let t = HuffTable::new(&counts, &vals, true).unwrap();
        let mut seed = 0xBEEFu32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for iter in 0..3000 {
            let data: Vec<u8> = (0..256).map(|_| (rnd() % 255) as u8).collect();
            let ss = 1 + rnd() % 30;
            let se = (ss + rnd() % 40).min(63);
            let mut a = DecState::from_cp(&data, &Checkpoint::at(0, 0));
            let mut b = DecState::from_cp(&data, &Checkpoint::at(0, 0));
            let mut blk = [0i16; 64];
            for i in 0..40 {
                ac_first(&mut a.br, &t, ss, se, 0, &mut a.eobrun, &mut blk);
                ac_first_skip(&mut b.br, &t, ss, se, &mut b.eobrun);
                let pa = a.br.pos * 8 + a.br.zero_fill as usize * 8 - a.br.bits as usize;
                let pb = b.br.pos * 8 + b.br.zero_fill as usize * 8 - b.br.bits as usize;
                assert_eq!((pa, a.eobrun), (pb, b.eobrun), "iter {iter} block {i}");
            }
        }
    }

    #[test]
    fn bit_helpers_match_scalar() {
        let mut seed = 99u64;
        let mut rnd = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed
        };
        for _ in 0..20000 {
            let mut b = [0i16; 64];
            let dens = rnd() % 4;
            for c in b.iter_mut() {
                if rnd() % 4 < dens {
                    *c = (rnd() >> 40) as i16 | 1;
                }
            }
            let mut m = 0u64;
            for (i, &c) in b.iter().enumerate() {
                m |= ((c != 0) as u64) << i;
            }
            assert_eq!(nzmask(&b), m);
            let (x, y) = (rnd(), rnd());
            assert_eq!(pdep(x, y), pdep_soft(x, y));
            let z = y & rnd();
            for r in [0, 1, 2, 7, 15, (x % 64) as u32] {
                assert_eq!(select(z, r), select_soft(z, r), "z {z:#x} r {r}");
            }
        }
    }

    #[test]
    fn speculative_ac_checkpoints_match_sequential() {
        let Some(data) = crate::spec::tests::make_jpeg_with(
            900,
            700,
            &["-quality", "90", "-sample", "2x2", "-progressive"],
        ) else {
            eprintln!("skipping: magick/cjpeg not available");
            return;
        };
        let p = crate::marker::parse(&data, true).unwrap();
        let g = OutGeom::new(&p.frame, crate::Scale::Full, None).unwrap();
        let key = |cp: &Checkpoint| (canon(&data, cp.pos, cp.bits - cp.zero_fill * 8), cp.eobrun);
        let mut tested = 0;
        for plan in plan_scans(&p, &g, false, true) {
            if plan.kind != Kind::AcFirst {
                continue;
            }
            // Truth: a plain sequential parse of the scan.
            let s = plan.scan;
            let t = plan.comps[0].4.unwrap();
            let (ss, se) = (s.ss as u32, s.se.min(63) as u32);
            let mut st = DecState::from_cp(&data, &Checkpoint::at(s.data_start, 0));
            let mut truth = Vec::new();
            for _ in 0..plan.rows {
                truth.push(key(&st.checkpoint()));
                for _ in 0..plan.units_x {
                    ac_first_skip(&mut st.br, t, ss, se, &mut st.eobrun);
                }
            }
            // Normal synchronisation, margins too small to synchronise (sequential re-decode
            // path), and a single chunk.
            for (nchunks, margin) in [(40, 4096), (40, 1), (7, 0), (1, 0)] {
                let cps = spec_ac(&data, &plan, nchunks, 1, margin, None).expect("checkpoints");
                let got: Vec<_> = cps.iter().map(&key).collect();
                assert_eq!(
                    got, truth,
                    "scan {} nchunks {nchunks} margin {margin}",
                    plan.si
                );
            }
            tested += 1;
        }
        assert!(tested >= 3);
    }
}
