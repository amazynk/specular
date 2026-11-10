//! Speculative row checkpoints for AC refinement scans.
//!
//! A refinement scan's bit consumption depends on each block's nonzero history (one correction
//! bit per previously nonzero coefficient), so a run started at a guessed byte position must
//! also guess *which block* it is in. Once the history masks of all blocks are known (every
//! earlier scan of the component has been decoded), runs start at chunk boundaries with a block
//! index guessed from the byte position and parse with those masks. A state on the true decode
//! path is the triple (canonical bit position, EOB run, block index), and two runs that reach
//! the same triple continue identically, so equality with a state of a run already known to be
//! true proves a later run true from there on. Starting a run *behind* the true block index
//! lets it catch up (EOB runs parsed from misaligned data consume blocks faster than bits) and
//! synchronise; a run whose index is wrong almost immediately parses data that valid files
//! cannot contain (an *anomaly*: a symbol size other than 1, a coefficient beyond `Se`, an EOB
//! run past the last block).
//!
//! 1. Every chunk is parsed concurrently (masks only, nothing stored) from its guessed state.
//! 2. Every run then continues, concurrently, past the end of its chunk until one of its states
//!    equals a state of a later chunk's run (a *link*), it reaches the end of the scan, or it
//!    meets an anomaly (it is not on the true path, or the data is corrupt).
//! 3. Following the links from the first run (which starts at the true scan start) gives the
//!    true state at every block, hence every row start. A run that never links (all later runs
//!    failed to synchronise) simply parsed the rest of the scan in step 2, so step 3 is exact
//!    without any sequential re-parse; only anomalies on the true path (corrupt data) or
//!    truncation make the whole scan fall back to sequential decoding.

use super::*;

/// Minimum scan length (bytes) for speculative parsing of a refinement scan.
pub(crate) const REFINE_MIN: usize = 256 << 10;
/// Minimum bytes per chunk.
pub(crate) const REFINE_MIN_CHUNK: usize = 64 << 10;
/// Heap bytes of the speculative parse of a refinement scan of `blocks` blocks in `rows` rows
/// (for the memory estimate): kept states of the step-1 runs (with vector growth), states of
/// the extensions (row starts only) and the estimator.
pub(crate) fn spec_bytes(blocks: u64, rows: u64) -> u64 {
    let state = std::mem::size_of::<State>() as u64;
    (blocks / KEEP + rows) * state * 3 / 2 + rows * state * 2 + blocks / KEEP * 4
}

/// Runs keep the state at every block whose index is a multiple of this (and at row starts).
const KEEP: u64 = 64;
/// A run counts as synchronised after this many consecutive blocks without an anomaly (runs at
/// a wrong block index meet one every ~10 blocks; the longest clean stretch seen on wrong paths
/// of the test photos was 208 blocks).
const CLEAN: u32 = 384;
/// ... and at least this many correction bits in those blocks: where the history is mostly
/// zero (flat image areas) the block index makes no difference to the parse, and a run at a
/// wrong index can stay clean for thousands of blocks.
const CLEAN_BITS: u32 = 2048;

/// How runs guess their starting block index.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tune {
    /// Synchronisation attempts (rounds): blocks behind the (corrected) estimate to start at,
    /// and the byte budget of the attempt (a run behind the true index by d blocks
    /// synchronises after ~10-20 d bytes, if at all).
    pub backs: &'static [(i64, usize)],
    /// Last attempt of chunks that never synchronised (no budget).
    pub final_back: i64,
}

impl Tune {
    pub const DEFAULT: Tune = Tune {
        backs: &[
            (256, 16 << 10),
            (256, 16 << 10),
            (512, 16 << 10),
            (512, 16 << 10),
            (1024, 32 << 10),
            (1024, 32 << 10),
            (2048, 64 << 10),
        ],
        final_back: 512,
    };
}

/// A kept state: block index, EOB run, canonical bit position.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct State {
    idx: u64,
    eob: u32,
    pos: u64,
}

/// Nonzero history masks of the blocks of a scan, one slice of `ux` blocks per scan row (the
/// rows live in different bands' storage; no copy is made).
pub(super) struct Masks<'a> {
    pub rows: Vec<&'a [u64]>,
    pub ux: usize,
}

impl Masks<'_> {
    fn len(&self) -> u64 {
        (self.rows.len() * self.ux) as u64
    }
    /// Mask of block `idx` (scan order).
    #[inline(always)]
    fn get(&self, idx: u64) -> u64 {
        let (r, c) = (idx / self.ux as u64, idx % self.ux as u64);
        self.rows[r as usize][c as usize]
    }
}

/// Parameters of the scan being parsed.
struct Ctx<'a> {
    data: &'a [u8],
    t: &'a HuffTable,
    ss: u32,
    se: u32,
    /// Nonzero history per block.
    masks: &'a Masks<'a>,
    ux: u64,
    /// Blocks from the first given row to the end of the scan (EOB runs may run past the
    /// given rows).
    scan_left: u64,
    cancel: &'a [&'a AtomicBool],
}

/// Bit consumption of [`ac_refine`] for one block with nonzero history `nz`, without storing
/// anything. Returns `false` on an anomaly (impossible in valid data; the bits consumed are
/// still exactly those of `ac_refine`): a symbol size other than 1, a coefficient or ZRL
/// beyond `se`, or an EOB run longer than the `left` blocks remaining.
#[inline(always)]
pub(super) fn refine_probe(
    br: &mut BitReader,
    t: &HuffTable,
    (ss, se): (u32, u32),
    eobrun: &mut u32,
    nz: u64,
    left: u64,
) -> bool {
    let range = (u64::MAX >> (63 - se)) & (u64::MAX << ss);
    let nz = nz & range;
    let mut ok = true;
    let mut k = ss;
    if *eobrun == 0 {
        let zs = !nz & range;
        while k <= se {
            br.ensure(32);
            let e = t.lut[br.peek(LUT_BITS) as usize];
            let r;
            if e >> 16 != 0 {
                let s = (e >> 8) & 15;
                br.consume((e & 31) - s + 1);
                r = (e >> 12) & 15;
                ok &= s == 1;
            } else {
                let sym = br.decode_symbol(t);
                r = sym >> 4;
                let s = sym & 15;
                if s == 0 && r != 15 {
                    *eobrun = 1 << r;
                    if r != 0 {
                        *eobrun += br.get(r);
                    }
                    ok &= *eobrun as u64 <= left;
                    break;
                }
                if s != 0 {
                    br.consume(1);
                }
                ok &= s <= 1;
            }
            let pos = select(zs & (u64::MAX << k), r);
            ok &= pos <= se;
            let pos = pos.min(se + 1);
            // Correction bits of the coefficients k..pos (pos >= k >= 1).
            let n = (nz & (u64::MAX << k) & !(u64::MAX << (pos - 1) << 1)).count_ones();
            skip_bits(br, n);
            k = pos + 1;
        }
    }
    if *eobrun > 0 {
        if k <= se {
            skip_bits(br, (nz & (u64::MAX << k)).count_ones());
        }
        *eobrun -= 1;
    }
    ok
}

/// Consume `n` (<= 64) bits.
#[inline(always)]
fn skip_bits(br: &mut BitReader, mut n: u32) {
    if n > 32 {
        br.ensure(32);
        br.consume(32);
        n -= 32;
    }
    br.ensure(n);
    br.consume(n);
}

/// Why a run stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum End {
    /// Passed its stop byte.
    Stop,
    /// Parsed the last block of the given rows.
    Total,
    /// Linked to a state of run `.0`.
    Link(usize),
    /// Anomaly (in step 2), truncated data or cancelled.
    Dead,
    /// Step 1 attempt: not synchronised within the byte budget.
    Budget,
    /// Step 1 attempt: synchronised (see [`CLEAN`]).
    Synced,
}

/// One run: kept states (strictly increasing block index) and where it stopped.
struct Run<'a> {
    states: Vec<State>,
    st: DecState<'a>,
    idx: u64,
    end: End,
}

impl<'a> Run<'a> {
    fn new(data: &'a [u8], pos: usize, idx: u64) -> Self {
        Run {
            states: Vec::new(),
            st: DecState::from_cp(data, &Checkpoint::at(pos, 0)),
            idx,
            end: End::Stop,
        }
    }
    /// Kept state with block index `idx`.
    fn find(&self, idx: u64) -> Option<&State> {
        let i = self.states.partition_point(|s| s.idx < idx);
        self.states.get(i).filter(|s| s.idx == idx)
    }
}

/// The current state of `st` at block `idx` (`None` once the reader is into the zero fill).
#[inline(always)]
fn state_of(data: &[u8], st: &DecState, idx: u64) -> Option<State> {
    let r = rec_of(st)?;
    Some(State {
        idx,
        eob: st.eobrun,
        pos: canon(data, r.pos as usize, r.bits as u32),
    })
}

/// How far [`advance`] parses.
#[derive(Clone, Copy)]
enum Goal<'r, 'a> {
    /// Step 1 attempt: until [`CLEAN`] consecutive blocks without an anomaly (`End::Synced`),
    /// byte `budget` (`End::Budget`) or byte `stop`.
    Probe { stop: usize, budget: usize },
    /// Step 1: until byte `stop`, whatever the anomalies.
    Fill { stop: usize },
    /// Step 2: until a kept state equals one of `others` (the step-1 runs of chunks
    /// `first..`), stopping at the first anomaly.
    Extend { others: &'r [Run<'a>], first: usize },
}

fn advance<'a>(c: &Ctx<'a>, run: &mut Run<'a>, goal: Goal<'_, 'a>, starts: &[usize]) {
    let total = c.masks.len();
    let (ss, se) = (c.ss, c.se);
    let (stop, budget) = match goal {
        Goal::Probe { stop, budget } => (stop, budget),
        Goal::Fill { stop } => (stop, usize::MAX),
        Goal::Extend { .. } => (usize::MAX, usize::MAX),
    };
    let (mut clean, mut clean_bits) = (0u32, 0u32);
    let range = (u64::MAX >> (63 - se)) & (u64::MAX << ss);
    loop {
        if run.idx >= total {
            run.end = End::Total;
            return;
        }
        if run.st.br.pos >= stop {
            run.end = End::Stop;
            return;
        }
        if run.st.br.pos >= budget {
            run.end = End::Budget;
            return;
        }
        let idx = run.idx;
        let (row, col) = (idx / c.ux, idx % c.ux);
        let nz = c.masks.rows[row as usize][col as usize];
        if idx.is_multiple_of(KEEP) || col == 0 {
            let Some(s) = state_of(c.data, &run.st, idx) else {
                run.end = End::Dead;
                return;
            };
            // Extensions are only compared with other runs and read at row starts: keeping
            // just the row starts bounds memory when many of them reach the end of the scan.
            if col == 0 || !matches!(goal, Goal::Extend { .. }) {
                run.states.push(s);
            }
            if let Goal::Extend { others, first } = goal {
                // Compare with the runs of the chunk the state lies in and the one before.
                let ch = starts.partition_point(|&b| b as u64 * 8 <= s.pos);
                for o in ch.saturating_sub(1).max(first)..(ch + 1).min(first + others.len()) {
                    if others[o - first].find(idx) == Some(&s) {
                        if col != 0 {
                            run.states.push(s);
                        }
                        run.end = End::Link(o);
                        return;
                    }
                }
            }
            if idx.is_multiple_of(4096) && c.cancel.iter().any(|x| x.load(Ordering::Relaxed)) {
                run.end = End::Dead;
                return;
            }
        }
        let ok = refine_probe(
            &mut run.st.br,
            c.t,
            (ss, se),
            &mut run.st.eobrun,
            nz,
            c.scan_left - idx,
        );
        run.idx += 1;
        match goal {
            Goal::Extend { .. } if !ok => {
                run.end = End::Dead;
                return;
            }
            Goal::Probe { .. } => {
                if ok {
                    clean += 1;
                    clean_bits += (nz & range).count_ones();
                } else {
                    (clean, clean_bits) = (0, 0);
                }
                if clean >= CLEAN && clean_bits >= CLEAN_BITS {
                    run.end = End::Synced;
                    return;
                }
            }
            _ => {}
        }
    }
}

/// Block index estimates from byte positions: every previously nonzero coefficient costs
/// exactly one correction bit, the rest of the scan's bits are spread evenly over the blocks.
struct Estimator {
    /// Correction bits before every `KEEP`-th block (and the total at the end).
    pre: Vec<u32>,
    /// Blocks.
    total: u64,
    /// Other bits per block.
    beta: f64,
}

impl Estimator {
    fn new(masks: &Masks, (ss, se): (u32, u32), len: usize) -> Option<Self> {
        let range = (u64::MAX >> (63 - se)) & (u64::MAX << ss);
        let total = masks.len();
        let mut pre = Vec::with_capacity((total / KEEP) as usize + 2);
        let mut sum = 0u32;
        for idx in 0..total {
            if idx.is_multiple_of(KEEP) {
                pre.push(sum);
            }
            sum = sum.checked_add((masks.get(idx) & range).count_ones())?;
        }
        pre.push(sum);
        let beta = ((len as f64 * 8.0) - sum as f64).max(0.0) / total.max(1) as f64;
        Some(Estimator { pre, total, beta })
    }
    /// Index of sample `m` and the estimated bits before it.
    fn sample(&self, m: usize) -> (u64, f64) {
        let b = (m as u64 * KEEP).min(self.total);
        (b, self.pre[m] as f64 + self.beta * b as f64)
    }
    /// Estimated index of the block at bit `bit` of the scan.
    fn at(&self, bit: u64) -> u64 {
        let want = bit as f64;
        let n = self.pre.len() - 1;
        let (mut lo, mut hi) = (0usize, n);
        while lo < hi {
            let m = (lo + hi) / 2;
            if self.sample(m).1 >= want {
                hi = m;
            } else {
                lo = m + 1;
            }
        }
        if lo == 0 {
            return 0;
        }
        // Linear between the samples around `bit`.
        let ((b0, x0), (b1, x1)) = (self.sample(lo - 1), self.sample(lo));
        let f = if x1 > x0 {
            ((want - x0) / (x1 - x0)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        b0 + ((b1 - b0) as f64 * f) as u64
    }
}

/// Estimate correction at bit `x` from the errors (estimate minus true index) at the anchors
/// (bit position, error), sorted by position: linear between the neighbours, else the nearest.
fn correction(anchors: &[(u64, i64)], x: u64) -> i64 {
    let i = anchors.partition_point(|a| a.0 <= x);
    let line = |l: (u64, i64), r: (u64, i64)| {
        if r.0 == l.0 {
            return l.1;
        }
        let f = (x as f64 - l.0 as f64) / (r.0 as f64 - l.0 as f64);
        l.1 + ((r.1 - l.1) as f64 * f) as i64
    };
    match i {
        _ if anchors.is_empty() => 0,
        0 => anchors[0].1,
        _ if i == anchors.len() => anchors[i - 1].1,
        _ => line(anchors[i - 1], anchors[i]),
    }
}

/// Options of [`spec_refine`].
#[derive(Clone, Copy)]
pub(super) struct Opts<'a> {
    /// Chunks parsed concurrently.
    pub nchunks: usize,
    /// Minimum bytes per chunk.
    pub min_chunk: usize,
    pub tune: Tune,
    /// The parse stops (`None`) when any of these is set.
    pub cancel: &'a [&'a AtomicBool],
    /// Estimated bytes of the given rows when they end before the scan does (the chunks are
    /// spread over that many bytes).
    pub len: Option<usize>,
}

/// Row-start checkpoints of `masks.rows.len()` rows from row `first` of a restart-free AC
/// refinement scan, whose blocks' nonzero histories (before this scan) are `masks` (scan
/// order), given the true state `from` at row `first`; one more checkpoint follows: the state
/// after the last given row. `None` if the data is corrupt or truncated (those rows are then
/// decoded sequentially) or the parse was cancelled.
pub(super) fn spec_refine(
    data: &[u8],
    plan: &ScanPlan,
    masks: &Masks,
    (first, from): (u32, &Checkpoint),
    o: &Opts,
) -> Option<Vec<Checkpoint>> {
    let s = plan.scan;
    let t = plan.comps[0].4?;
    let (ss, se) = (s.ss as u32, s.se.min(63) as u32);
    let ux = plan.units_x as u64;
    let nrows = masks.rows.len();
    let total = ux * nrows as u64;
    if s.restart_interval != 0
        || ss == 0
        || ss > se
        || masks.ux as u64 != ux
        || nrows + first as usize > plan.rows as usize
        || masks.rows.iter().any(|r| r.len() != masks.ux)
        || data.len() >= (u32::MAX as usize) / 2
    {
        return None;
    }
    if total == 0 {
        return Some(vec![*from]);
    }
    let start = from.pos.max(s.data_start);
    let end = s.data_end.min(data.len()).max(start);
    let end = o.len.map_or(end, |l| end.min(start.saturating_add(l)));
    let len = end - start;
    let (tune, cancel) = (o.tune, o.cancel);
    let nchunks = o.nchunks.clamp(1, (len / o.min_chunk.max(1)).max(1));
    let mut starts: Vec<usize> = (0..nchunks).map(|i| start + len * i / nchunks).collect();
    for s in starts.iter_mut().skip(1) {
        while *s < end && (data[*s - 1] == 0xFF || data[*s] == 0xFF) {
            *s += 1;
        }
    }
    let est = Estimator::new(masks, (ss, se), len)?;
    let c = Ctx {
        data,
        t,
        ss,
        se,
        masks,
        ux,
        scan_left: (plan.rows - first) as u64 * ux,
        cancel,
    };
    let stop_of = |i: usize| starts.get(i + 1).copied().unwrap_or(usize::MAX);
    let rel = |b: usize| (b - start) as u64 * 8;

    // Step 1, in rounds: every chunk not yet synchronised makes an attempt from a guess behind
    // its estimate, corrected by the errors seen at the synchronisation points of other chunks
    // (the estimate's error drifts slowly along the scan). Run 0 starts at the true start.
    let mut runs: Vec<Option<Run>> = (0..nchunks).map(|_| None).collect();
    let mut run0 = Run::new(data, start, 0);
    run0.st = DecState::from_cp(data, from);
    runs[0] = Some(run0);
    let mut anchors: Vec<(u64, i64)> = vec![(0, 0)];
    let guess = |i: usize, anchors: &[(u64, i64)], back: i64| {
        let x = rel(starts[i]);
        (est.at(x) as i64 - correction(anchors, x) - back).max(0) as u64
    };
    for &(back, budget) in tune.backs {
        let pending: Vec<usize> = (1..nchunks).filter(|&i| runs[i].is_none()).collect();
        if pending.is_empty() {
            break;
        }
        let tried: Vec<(usize, Run)> = pending
            .into_par_iter()
            .map(|i| {
                let mut run = Run::new(data, starts[i], guess(i, &anchors, back));
                let budget = starts[i].saturating_add(budget);
                advance(
                    &c,
                    &mut run,
                    Goal::Probe {
                        stop: stop_of(i),
                        budget,
                    },
                    &starts,
                );
                (i, run)
            })
            .collect();
        for (i, run) in tried {
            tracing::trace!(target: "jpeg", scan = plan.si, chunk = i, back, end = ?run.end, "refine probe");
            if run.end == End::Synced
                && let Some(s) = state_of(data, &run.st, run.idx)
            {
                anchors.push((
                    s.pos - start as u64 * 8,
                    est.at(s.pos - start as u64 * 8) as i64 - s.idx as i64,
                ));
                runs[i] = Some(run);
            }
        }
        anchors.sort_unstable();
    }
    // Every run continues to its chunk end; chunks that never synchronised make a last attempt.
    let runs: Vec<Run> = runs
        .into_par_iter()
        .enumerate()
        .map(|(i, run)| {
            let mut run = run
                .unwrap_or_else(|| Run::new(data, starts[i], guess(i, &anchors, tune.final_back)));
            advance(&c, &mut run, Goal::Fill { stop: stop_of(i) }, &starts);
            run
        })
        .collect();
    let mut runs = runs;

    // Step 2: every run that stopped at its chunk end continues, compared with the step-1
    // states of the later runs.
    let heads: &[Run] = &runs;
    let ext: Vec<Option<Run>> = (0..nchunks)
        .into_par_iter()
        .map(|i| {
            let r = &heads[i];
            (r.end == End::Stop).then(|| {
                let mut e = Run {
                    states: Vec::new(),
                    st: DecState {
                        br: r.st.br.clone(),
                        dc: r.st.dc,
                        restart_left: r.st.restart_left,
                        eobrun: r.st.eobrun,
                    },
                    idx: r.idx,
                    end: End::Stop,
                };
                advance(
                    &c,
                    &mut e,
                    Goal::Extend {
                        others: &heads[i + 1..],
                        first: i + 1,
                    },
                    &starts,
                );
                e
            })
        })
        .collect();
    for (r, e) in runs.iter_mut().zip(ext) {
        if let Some(e) = e {
            r.states.extend_from_slice(&e.states);
            r.idx = e.idx;
            r.end = e.end;
            r.st = e.st;
        }
    }

    // Step 3: follow the links from the first run. Segments: (first block index, run).
    let mut segs: Vec<(u64, usize)> = vec![(0, 0)];
    let mut cur = 0usize;
    loop {
        match runs[cur].end {
            End::Total => break,
            End::Link(m) => {
                let at = runs[cur].states.last()?.idx;
                segs.push((at, m));
                cur = m;
            }
            e => {
                tracing::debug!(target: "jpeg", scan = plan.si, chunk = cur, end = ?e, linked = segs.len(), nchunks, "refine speculation: chain broken");
                return None;
            }
        }
    }
    tracing::trace!(target: "jpeg", scan = plan.si, nchunks, linked = segs.len(), "refine speculation linked");
    let mut out = Vec::with_capacity(nrows + 1);
    let mut sg = 0usize;
    for row in 0..nrows as u64 {
        let idx = row * ux;
        while sg + 1 < segs.len() && segs[sg + 1].0 <= idx {
            sg += 1;
        }
        let st = runs[segs[sg].1].find(idx)?;
        out.push(checkpoint_at(data, st.pos, st.eob));
    }
    let last = state_of(data, &runs[cur].st, total)?;
    out.push(checkpoint_at(data, last.pos, last.eob));
    Some(out)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::geom::OutGeom;

    /// Per refinement scan: plan index, nonzero masks before the scan, true row keys.
    pub(crate) type Truth = Vec<(usize, Vec<u64>, Vec<(u64, u32)>)>;

    /// Per refinement scan of `data`: the scan's plan index, the nonzero history masks before it
    /// (scan order) and the true row-start keys (canonical position, EOB run).
    pub(crate) fn refine_truth(data: &[u8]) -> Truth {
        let p = crate::marker::parse(data, true).unwrap();
        let g = OutGeom::new(&p.frame, crate::Scale::Full, None).unwrap();
        let plans = plan_scans(&p, &g, false, true);
        let mut store: Vec<Vec<u64>> = p
            .frame
            .comps
            .iter()
            .map(|c| vec![0u64; (p.frame.mcus_y * c.v * c.bw_pad) as usize])
            .collect();
        let mut out = Vec::new();
        for (n, pl) in plans.iter().enumerate() {
            if !matches!(pl.kind, Kind::AcFirst | Kind::AcRefine) {
                continue;
            }
            let ci = pl.comps[0].0;
            let bwp = p.frame.comps[ci].bw_pad as usize;
            let ux = pl.units_x as usize;
            let s = pl.scan;
            let a = AcScan {
                t: pl.comps[0].4.unwrap(),
                ss: s.ss as u32,
                se: s.se.min(63) as u32,
                al: s.al as u32,
                ri: s.restart_interval,
                refine: pl.kind == Kind::AcRefine,
            };
            let before: Vec<u64> = (0..pl.rows as usize)
                .flat_map(|r| store[ci][r * bwp..r * bwp + ux].iter().copied())
                .collect();
            let mut st = DecState::from_cp(data, &Checkpoint::at(s.data_start, s.restart_interval));
            let mut keys = Vec::new();
            for row in 0..pl.rows as usize {
                let cp = st.checkpoint();
                keys.push((canon(data, cp.pos, cp.bits - cp.zero_fill * 8), cp.eobrun));
                ac_line(&a, &mut st, &mut store[ci][row * bwp..row * bwp + ux]);
            }
            if a.refine {
                out.push((n, before, keys));
            }
        }
        out
    }

    pub(crate) fn key(data: &[u8], cp: &Checkpoint) -> (u64, u32) {
        (canon(data, cp.pos, cp.bits - cp.zero_fill * 8), cp.eobrun)
    }

    /// Options for `nchunks` chunks of at least `min_chunk` bytes, never cancelled.
    pub(crate) fn opts(nchunks: usize, min_chunk: usize, tune: Tune) -> Opts<'static> {
        Opts {
            nchunks,
            min_chunk,
            tune,
            cancel: &[],
            len: None,
        }
    }

    /// `spec_refine` of the whole scan.
    pub(crate) fn spec_all(
        data: &[u8],
        plan: &ScanPlan,
        masks: &Masks,
        o: &Opts,
    ) -> Option<Vec<Checkpoint>> {
        let from = Checkpoint::at(plan.scan.data_start, 0);
        let mut cps = spec_refine(data, plan, masks, (0, &from), o)?;
        cps.pop();
        Some(cps)
    }

    /// Check `spec_refine` against the sequential parse for every refinement scan of `data`,
    /// from the scan start and from a third of the way in.
    pub(crate) fn check_file(data: &[u8], configs: &[(usize, usize, Tune)]) -> usize {
        let p = crate::marker::parse(data, true).unwrap();
        let g = OutGeom::new(&p.frame, crate::Scale::Full, None).unwrap();
        let plans = plan_scans(&p, &g, false, true);
        let mut n = 0;
        for (pi, masks, truth) in refine_truth(data) {
            let plan = &plans[pi];
            for &(nchunks, min_chunk, tune) in configs {
                let m = Masks {
                    rows: masks.chunks(plan.units_x as usize).collect(),
                    ux: plan.units_x as usize,
                };
                let o = opts(nchunks, min_chunk, tune);
                let cps = spec_all(data, plan, &m, &o).expect("checkpoints");
                let got: Vec<_> = cps.iter().map(|c| key(data, c)).collect();
                assert_eq!(got, truth, "scan {} nchunks {nchunks}", plan.si);
                // Rows r0..r1 only, with an estimated length.
                let (r0, r1) = (truth.len() / 3, truth.len() * 2 / 3);
                let from = checkpoint_at(data, truth[r0].0, truth[r0].1);
                let m = Masks {
                    rows: m.rows[r0..r1].to_vec(),
                    ux: m.ux,
                };
                let o = Opts {
                    len: Some(((truth[r1].0 - truth[r0].0) / 8) as usize * 9 / 10),
                    ..o
                };
                let cps = spec_refine(data, plan, &m, (r0 as u32, &from), &o).expect("checkpoints");
                let got: Vec<_> = cps.iter().map(|c| key(data, c)).collect();
                assert_eq!(got, truth[r0..=r1], "scan {} rows {r0}..{r1}", plan.si);
            }
            n += 1;
        }
        n
    }

    #[test]
    fn refine_sync_failure_and_bad_data() {
        let Some(data) = crate::spec::tests::make_jpeg_with(
            800,
            600,
            &["-quality", "95", "-sample", "2x2", "-progressive"],
        ) else {
            eprintln!("skipping: magick/cjpeg not available");
            return;
        };
        // No chunk ever synchronises (every guess is far off): run 0 alone parses the scan.
        let never = Tune {
            backs: &[(1 << 20, 1 << 20)],
            final_back: -(1 << 20),
        };
        assert!(check_file(&data, &[(16, 1, never)]) >= 3);

        let p = crate::marker::parse(&data, true).unwrap();
        let g = OutGeom::new(&p.frame, crate::Scale::Full, None).unwrap();
        let plans = plan_scans(&p, &g, false, true);
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let (mut none, mut exact) = (0, 0);
        for (pi, _, _) in refine_truth(&data) {
            let s = plans[pi].scan;
            let (a, b) = (s.data_start, s.data_end);
            // Corrupt data: the result is either a failure (sequential fallback) or exactly
            // the sequential parse of the corrupt data.
            for k in 0..6 {
                let mut bad = data.clone();
                let at = a + (b - a) * (2 * k + 1) / 12;
                for x in &mut bad[at..(at + 24).min(b)] {
                    *x = (rnd() % 255) as u8;
                }
                let truth = refine_truth(&bad).into_iter().find(|t| t.0 == pi).unwrap();
                let pb = crate::marker::parse(&bad, true).unwrap();
                let plan = &plan_scans(&pb, &g, false, true)[pi];
                let m = Masks {
                    rows: truth.1.chunks(plan.units_x as usize).collect(),
                    ux: plan.units_x as usize,
                };
                match spec_all(&bad, plan, &m, &opts(16, 1, Tune::DEFAULT)) {
                    None => none += 1,
                    Some(cps) => {
                        let got: Vec<_> = cps.iter().map(|c| key(&bad, c)).collect();
                        assert_eq!(got, truth.2, "corrupt scan {} at {at}", plan.si);
                        exact += 1;
                    }
                }
            }
            // Truncated inside the scan: the true path runs into the zero fill, so the parse
            // fails and the stage falls back to sequential decoding.
            let cut = &data[..(a + b) / 2];
            let pc = crate::marker::parse(cut, true).unwrap();
            let plan = &plan_scans(&pc, &g, false, true)[pi];
            let masks = refine_truth(&data)
                .into_iter()
                .find(|t| t.0 == pi)
                .unwrap()
                .1;
            let m = Masks {
                rows: masks.chunks(plan.units_x as usize).collect(),
                ux: plan.units_x as usize,
            };
            assert!(spec_all(cut, plan, &m, &opts(16, 1, Tune::DEFAULT)).is_none());
        }
        eprintln!("corrupt scans: {none} failed, {exact} exact");
    }

    /// The engine's refinement stages: parallel decodes equal the single-threaded decode
    /// exactly, also when speculation fails (corrupt or truncated data).
    #[test]
    fn refine_stages_exact_and_fallback() {
        let Some(data) = crate::spec::tests::make_jpeg_with(
            2000,
            1500,
            &["-quality", "95", "-sample", "2x2", "-progressive"],
        ) else {
            eprintln!("skipping: magick/cjpeg not available");
            return;
        };
        let p = crate::marker::parse(&data, true).unwrap();
        let g = OutGeom::new(&p.frame, crate::Scale::Full, None).unwrap();
        let plans = plan_scans(&p, &g, false, true);
        let big: Vec<_> = refine_truth(&data)
            .into_iter()
            .map(|t| plans[t.0].scan)
            .filter(|s| s.data_end - s.data_start >= REFINE_MIN)
            .map(|s| (s.data_start, s.data_end))
            .collect();
        assert!(!big.is_empty(), "no refinement scan of {REFINE_MIN} bytes");
        let (a, b) = *big.last().unwrap();
        let mut corrupt = data.clone();
        for x in &mut corrupt[(a + b) / 2..(a + b) / 2 + 64] {
            *x = 0x5a;
        }
        let o = |par| crate::DecodeOptions {
            scale: crate::Scale::Full,
            region: None,
            parallelism: par,
            cancel: None,
        };
        let same = |x: &crate::DecodedPlanes, y: &crate::DecodedPlanes, what: &str| {
            assert_eq!(x.truncated, y.truncated, "{what}");
            for (u, v) in x.planes.iter().zip(&y.planes) {
                assert!(u.data == v.data, "{what}: planes differ");
            }
        };
        REFINE_WAIT.store(true, Ordering::Relaxed);
        let before = REFINE_STATS.each_ref().map(|c| c.load(Ordering::Relaxed));
        for (what, d) in [
            ("clean", &data[..]),
            ("corrupt", &corrupt[..]),
            ("truncated", &data[..(a + b) / 2]),
        ] {
            let single = crate::decode(d, &o(crate::Parallelism::Single)).unwrap();
            for t in [2, 8] {
                let par = crate::decode(d, &o(crate::Parallelism::Threads(t))).unwrap();
                same(&single, &par, &format!("{what} threads {t}"));
            }
            if what != "truncated" {
                let ix = crate::build_index_with(d, &o(crate::Parallelism::Threads(8))).unwrap();
                let ix1 = crate::build_index_with(d, &o(crate::Parallelism::Single)).unwrap();
                let wi = crate::decode_with_index(d, &ix, &o(crate::Parallelism::Threads(8)));
                same(&single, &wi.unwrap(), &format!("{what} with index"));
                let wi = crate::decode_with_index(d, &ix1, &o(crate::Parallelism::Threads(8)));
                same(
                    &single,
                    &wi.unwrap(),
                    &format!("{what} with single-thread index"),
                );
            }
        }
        let after = REFINE_STATS.each_ref().map(|c| c.load(Ordering::Relaxed));
        assert!(
            after[0] > before[0],
            "no refinement stage was decoded in parallel"
        );
        assert!(
            after[1] > before[1],
            "no refinement stage fell back to sequential"
        );
    }

    #[test]
    fn speculative_refine_checkpoints_match_sequential() {
        let Some(data) = crate::spec::tests::make_jpeg_with(
            1200,
            900,
            &["-quality", "95", "-sample", "2x2", "-progressive"],
        ) else {
            eprintln!("skipping: magick/cjpeg not available");
            return;
        };
        let far = Tune {
            backs: &[],
            final_back: 1 << 20,
        };
        let near = Tune {
            backs: &[(0, 1), (1, 1), (2, 1)],
            final_back: 0,
        };
        // Many tiny chunks (most fail to synchronise; earlier runs cover them), guesses far
        // behind and right at the estimate, and a single chunk.
        let n = check_file(
            &data,
            &[
                (40, 1, Tune::DEFAULT),
                (64, 1, far),
                (64, 1, near),
                (7, 1, Tune::DEFAULT),
                (1, 1, Tune::DEFAULT),
            ],
        );
        assert!(n >= 3, "{n} refinement scans");
    }
}
