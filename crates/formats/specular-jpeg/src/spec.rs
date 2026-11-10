//! Speculative parallel entropy decoding for sequential scans *without* restart markers.
//!
//! The scan data is cut into byte chunks. Every chunk is parsed (no output) on its own thread,
//! starting at the chunk's first byte under the assumption that an MCU starts there. Such a
//! speculative decoder produces garbage at first, but Huffman decoding self-synchronises: after a
//! short while it lands on the same bit position *and* MCU phase as the true decode, and from
//! then on both are identical (except for a constant DC-prediction offset per component).
//!
//! Each thread records the decoder state at MCU starts (all of them where chunks overlap, every
//! [`SPARSE`]-th elsewhere, see [`Run`]). A cheap sequential merge then walks
//! the chunks: the (known-correct) decode of chunk `t` is continued a little into chunk `t+1`,
//! and the first MCU-start state shared with chunk `t+1`'s speculative records proves the rest
//! of those records correct. If no common state is found (pathological data), chunk `t+1` is
//! re-decoded sequentially from the true state. The result is a checkpoint for every MCU row,
//! after which rows are decoded in parallel exactly like restart-interval files.

use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::sequential::{Checkpoint, DecState, SeqScan, skip_mcus};

/// Compact decoder state at an MCU start.
#[derive(Clone, Copy)]
struct Rec {
    /// Next unread raw byte.
    pos: u32,
    /// Buffered bits that came from real data (excludes zero fill).
    bits: u8,
    dc: [i32; 4],
}

impl Rec {
    #[inline]
    fn canon(&self, data: &[u8]) -> u64 {
        canon(data, self.pos as usize, self.bits as u32)
    }
}

/// Bytes per kept record of a [`Run`] of baseline MCU starts.
pub(crate) const RUN_REC_BYTES: u64 = std::mem::size_of::<(u32, Rec)>() as u64;

/// Raw bit position (`byte * 8 + bit`) of the next unconsumed bit, given the reader's raw byte
/// position and the number of buffered real-data bits.
#[inline]
pub(crate) fn canon(data: &[u8], pos: usize, bits: u32) -> u64 {
    let nbytes = bits.div_ceil(8);
    let mut q = pos;
    for _ in 0..nbytes {
        if q == 0 {
            break;
        }
        q -= 1;
        // A 0x00 after 0xFF is a stuffing byte: the data byte is the 0xFF before it.
        if data[q] == 0 && q > 0 && data[q - 1] == 0xFF {
            q -= 1;
        }
    }
    q as u64 * 8 + (nbytes * 8 - bits) as u64
}

/// A full checkpoint for a canonical bit position.
fn checkpoint_at(sc: &SeqScan, c: u64, dc: [i32; 4]) -> Checkpoint {
    let mut st = DecState::from_cp(sc.data, &Checkpoint::at(c as usize / 8, 0));
    st.br.refill();
    st.br.consume((c % 8) as u32);
    st.dc = dc;
    st.checkpoint()
}

/// The records of one speculative run. Every record inside the *zones* where runs are matched
/// against each other (the start of a chunk and its overlap into the next one) is kept, elsewhere
/// only every [`SPARSE`]-th: a row start between two kept records is re-derived by parsing
/// forward from the earlier one (fewer than `SPARSE` units), which keeps the memory at a few
/// percent of one record per unit.
pub(crate) struct Run<R> {
    /// `(index of the record in the run, record)`, by increasing index.
    pub recs: Vec<(u32, R)>,
    /// Records produced in total (units parsed from the run's start).
    pub count: u64,
    /// Byte range the run parses (`stop` clamped to the data) and its zones, for sizing `recs`.
    start: usize,
    stop: usize,
    zones: Zones,
}

/// How far each chunk's run of a baseline scan extends into the next chunk to find a
/// synchronisation point. Huffman data resynchronises quickly: on the test photos within 700
/// bytes (and if not, the merge falls back to a sequential re-decode of that chunk).
pub(crate) const MARGIN: usize = 4 << 10;
/// Minimum bytes per chunk of a baseline scan.
pub(crate) const MIN_CHUNK: usize = 128 << 10;

/// Keep every this many records outside the zones.
pub(crate) const SPARSE: u64 = 32;

/// Byte ranges of a run in which every record is kept: before `head_end` and from `tail_begin`.
#[derive(Clone, Copy)]
pub(crate) struct Zones {
    pub head_end: usize,
    pub tail_begin: usize,
}

impl Zones {
    /// Zones of chunk `i` of `starts.len()` chunks whose runs extend `margin` bytes into the
    /// next chunk. A record's canonical position is within 8 bytes before its raw position, so
    /// 16 bytes of slack keep every record that can take part in a match.
    pub fn chunk(starts: &[usize], i: usize, margin: usize) -> Zones {
        Zones {
            head_end: if i == 0 { 0 } else { starts[i] + margin + 16 },
            tail_begin: starts
                .get(i + 1)
                .map_or(usize::MAX, |&s| s.saturating_sub(16)),
        }
    }
    #[inline(always)]
    pub fn keep(&self, pos: usize, count: u64) -> bool {
        count.is_multiple_of(SPARSE) || pos < self.head_end || pos >= self.tail_begin
    }
}

impl<R> Run<R> {
    /// A run from byte `start` to `stop` with room for `est` records.
    pub fn new(est: usize, start: usize, stop: usize, zones: Zones) -> Self {
        Run {
            recs: Vec::with_capacity(est),
            count: 0,
            start,
            stop,
            zones,
        }
    }
    /// Count the next record (the state at raw byte `pos`) and keep it if [`Zones::keep`] says
    /// so.
    #[inline(always)]
    pub fn record(&mut self, pos: usize, r: R) {
        if self.zones.keep(pos, self.count) {
            if self.recs.len() == self.recs.capacity() {
                self.grow(pos);
            }
            self.recs.push((self.count as u32, r));
        }
        self.count += 1;
    }
    /// Make room for the rest of the run, extrapolated from the records per byte so far, instead
    /// of doubling (the records are a large share of the speculative parsing's memory, and runs
    /// whose zones are denser than their share would otherwise end up with up to twice the
    /// capacity they need).
    #[cold]
    #[inline(never)]
    fn grow(&mut self, pos: usize) {
        let per_byte = self.count as f64 / pos.saturating_sub(self.start).max(1) as f64;
        let stop = self.stop.max(pos);
        let head = self.zones.head_end.min(stop).saturating_sub(pos);
        let tail = stop.saturating_sub(self.zones.tail_begin.max(pos));
        let rest = (stop - pos).saturating_sub(head + tail);
        let need = per_byte * ((head + tail) as f64 + rest as f64 / SPARSE as f64);
        let need = (need * 1.125) as usize + 64;
        self.recs.reserve_exact(need.max(self.recs.len() / 4 + 16));
    }
    /// Position in `recs` of the first kept record with run index `>= k`.
    pub fn first_from(&self, k: u64) -> usize {
        self.recs.partition_point(|r| (r.0 as u64) < k)
    }
    /// The last kept record with run index `<= k`.
    pub fn at_or_before(&self, k: u64) -> Option<&(u32, R)> {
        let i = self.recs.partition_point(|r| (r.0 as u64) <= k);
        i.checked_sub(1).map(|i| &self.recs[i])
    }
}

/// Capacity for the kept records of a chunk of `chunk` bytes out of a `len`-byte scan of
/// `total` units, whose runs extend `margin` bytes into the next chunk: every [`SPARSE`]-th
/// record of its proportional share, plus the share of the zones (`2 * margin + 48` bytes,
/// see [`Zones::chunk`]).
pub(crate) fn expected_recs(total: u64, len: usize, chunk: usize, margin: usize) -> usize {
    let per_byte = |b: usize| (total as u128 * b as u128 / len.max(1) as u128) as u64;
    let share = per_byte(chunk);
    (share / SPARSE + per_byte(2 * margin + 48) + 64).min(total + 1) as usize
}

/// Parse MCUs from `st`, recording the state at MCU starts (see [`Run`]), until the reader is
/// past byte `stop`, `max` MCU starts were recorded, or the real data is exhausted.
#[inline(always)]
fn run<'a>(
    sc: &SeqScan<'a>,
    st: &mut DecState<'a>,
    stop: usize,
    max: u64,
    zones: Zones,
    est: usize,
    cancel: Option<&AtomicBool>,
) -> Run<Rec> {
    let mut out = Run::new(est, st.br.pos, stop.min(sc.data.len()), zones);
    while out.count < max && st.br.pos < stop {
        let Some(r) = rec_of(st) else {
            break;
        };
        if out.count.is_multiple_of(1024) && cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            break;
        }
        out.record(st.br.pos, r);
        skip_mcus(sc, st, 1);
    }
    out
}

/// The record of the current state (`None` once the reader is into the zero fill).
#[inline(always)]
fn rec_of(st: &DecState) -> Option<Rec> {
    let real = st.br.bits as i64 - st.br.zero_fill as i64 * 8;
    (real >= 0).then_some(Rec {
        pos: st.br.pos as u32,
        bits: real as u8,
        dc: st.dc,
    })
}

/// Build row-start checkpoints for a restart-free sequential scan using up to `nchunks`
/// speculative decoders (run in parallel if `parallel`). Returns `None` if the data looks
/// truncated or corrupt; callers then fall back to plain sequential decoding.
pub(crate) fn row_checkpoints(
    sc: &SeqScan,
    nchunks: usize,
    parallel: bool,
    cancel: Option<&AtomicBool>,
) -> Option<Vec<Checkpoint>> {
    let t0 = std::time::Instant::now();
    let r = row_checkpoints_with(sc, nchunks, parallel, MIN_CHUNK, MARGIN, cancel);
    tracing::debug!(
        target: "jpeg",
        ok = r.is_some(),
        rows = sc.mcus_y,
        ms = t0.elapsed().as_secs_f64() * 1e3,
        "speculative row search {}",
        if r.is_some() { "found every row start" } else { "failed" }
    );
    r
}

/// `min_chunk`: minimum bytes per chunk; `margin`: how far each chunk's decoder runs into the
/// next chunk to find a synchronisation point.
fn row_checkpoints_with(
    sc: &SeqScan,
    nchunks: usize,
    parallel: bool,
    min_chunk: usize,
    margin: usize,
    cancel: Option<&AtomicBool>,
) -> Option<Vec<Checkpoint>> {
    let data = sc.data;
    if sc.ri != 0 || data.len() >= (u32::MAX as usize) / 2 {
        return None;
    }
    let end = crate::marker::find_scan_end(data, sc.start);
    let len = end.saturating_sub(sc.start);
    let total = sc.mcus_x as u64 * sc.mcus_y as u64;
    let nchunks = nchunks.clamp(1, (len / min_chunk.max(1)).max(1));
    tracing::debug!(target: "jpeg", nchunks, bytes = len, "speculative row search");
    // Chunk starts, nudged so that no chunk begins at or right after an 0xFF.
    let mut starts: Vec<usize> = (0..nchunks).map(|i| sc.start + len * i / nchunks).collect();
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
        let cp = if i == 0 {
            sc.start_cp()
        } else {
            Checkpoint::at(starts[i], 0)
        };
        let mut st = DecState::from_cp(data, &cp);
        let est = expected_recs(total, len, stops[i].min(end) - starts[i], margin);
        let zones = Zones::chunk(&starts, i, margin);
        run(sc, &mut st, stops[i], total, zones, est, cancel)
    };
    // `runs[c]`: chunk c's records (runs >= nchunks are sequential re-decodes).
    let mut runs: Vec<Run<Rec>> = if parallel {
        (0..nchunks).into_par_iter().map(job).collect()
    } else {
        (0..nchunks).map(job).collect()
    };

    // Sequential merge. Each segment: (absolute MCU index of run index `from` of runs[c], dc
    // offset, c, from).
    let mut segs: Vec<(u64, [i32; 4], usize, u64)> = vec![(0, [0; 4], 0, 0)];
    for t in 1..nchunks {
        let (abs0, off, c, from) = *segs.last().unwrap();
        let truth = &runs[c].recs;
        let next = &runs[t].recs;
        let first_next = next.first().map_or(u64::MAX, |r| r.1.canon(data));
        // Two-pointer search for a common MCU-start bit position.
        let mut i = runs[c].first_from(from);
        while i < truth.len() && truth[i].1.canon(data) < first_next {
            i += 1;
        }
        let mut j = 0usize;
        let mut found = None;
        while i < truth.len() && j < next.len() {
            let (a, b) = (truth[i].1.canon(data), next[j].1.canon(data));
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
            let mut o = [0i32; 4];
            for k in 0..4 {
                o[k] = truth[i].1.dc[k]
                    .wrapping_add(off[k])
                    .wrapping_sub(next[j].1.dc[k]);
            }
            tracing::trace!(target: "jpeg", chunk = t, mcu = abs0 + (truth[i].0 as u64 - from), skipped = j, "chunk synchronised");
            segs.push((abs0 + (truth[i].0 as u64 - from), o, t, next[j].0 as u64));
        } else {
            // No synchronisation: re-decode sequentially from the last known-true state.
            let &(last, r) = truth.last().filter(|r| r.0 as u64 >= from)?;
            let mut dc = [0i32; 4];
            for k in 0..4 {
                dc[k] = r.dc[k].wrapping_add(off[k]);
            }
            let abs = abs0 + (last as u64 - from);
            tracing::debug!(target: "jpeg", chunk = t, mcu = abs, offset = starts[t], "chunk did not synchronise: re-decoding it sequentially");
            let mut st = DecState::from_cp(data, &checkpoint_at(sc, r.canon(data), dc));
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
            let more = run(sc, &mut st, stops[t], total - abs, zones, est, cancel);
            runs.push(more);
            segs.push((abs, [0; 4], runs.len() - 1, 0));
        }
    }

    // The last segment must reach the final MCU (otherwise the data is truncated).
    let (abs, _, c, from) = *segs.last().unwrap();
    if abs + runs[c].count.saturating_sub(from) < total {
        tracing::debug!(target: "jpeg", mcus = abs + runs[c].count.saturating_sub(from), total, "speculative row search: data ends early");
        return None;
    }
    // Row starts.
    let mut out = Vec::with_capacity(sc.mcus_y as usize);
    let mut s = 0usize;
    for row in 0..sc.mcus_y {
        let m = row as u64 * sc.mcus_x as u64;
        while s + 1 < segs.len() && segs[s + 1].0 <= m {
            s += 1;
        }
        let (abs, off, c, from) = segs[s];
        let k = from + (m - abs);
        if k >= runs[c].count {
            return None;
        }
        // `from` is kept, so this is a record of the synchronised part.
        let &(kr, r) = runs[c].at_or_before(k)?;
        let mut dc = [0i32; 4];
        for q in 0..4 {
            dc[q] = r.dc[q].wrapping_add(off[q]);
        }
        let mut cp = checkpoint_at(sc, r.canon(data), dc);
        if (kr as u64) < k {
            let mut st = DecState::from_cp(data, &cp);
            skip_mcus(sc, &mut st, (k - kr as u64) as u32);
            let r = rec_of(&st)?;
            cp = checkpoint_at(sc, r.canon(data), r.dc);
        }
        out.push(cp);
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
pub(crate) mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    /// A restart-free JPEG made by ImageMagick + cjpeg (piped, no temp files), if available.
    fn make_jpeg(w: u32, h: u32, sample: &str) -> Option<Vec<u8>> {
        make_jpeg_with(w, h, &["-quality", "90", "-sample", sample])
    }

    pub(crate) fn make_jpeg_with(w: u32, h: u32, args: &[&str]) -> Option<Vec<u8>> {
        let src = Command::new("magick")
            .args([
                "-seed",
                "3",
                "-size",
                &format!("{w}x{h}"),
                "plasma:fractal",
                "-attenuate",
                "0.3",
                "+noise",
                "Gaussian",
                "ppm:-",
            ])
            .output()
            .ok()?;
        let mut cj = Command::new("cjpeg")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;
        let mut stdin = cj.stdin.take()?;
        let ppm = src.stdout;
        let feeder = std::thread::spawn(move || std::io::Write::write_all(&mut stdin, &ppm));
        let out = cj.wait_with_output().ok()?;
        feeder.join().ok()?.ok()?;
        out.status.success().then_some(out.stdout)
    }

    fn key(data: &[u8], cp: &Checkpoint) -> (u64, [i32; 4]) {
        let real = cp.bits - cp.zero_fill * 8;
        (canon(data, cp.pos, real), cp.dc)
    }

    #[test]
    fn speculative_checkpoints_match_sequential() {
        let Some(data) = make_jpeg(700, 500, "2x2") else {
            eprintln!("skipping: magick/cjpeg not available");
            return;
        };
        let p = crate::marker::parse(&data, false).unwrap();
        let sc = SeqScan::new(&p, &data).unwrap();
        let params = crate::sequential::SeqParams {
            simd: crate::idct::Simd::detect(),
            threads: 1,
            cancel: None,
            index: None,
            want_index: true,
            cols: None,
        };
        let truth: Vec<_> = crate::sequential::build_index(&sc, &params)
            .unwrap()
            .iter()
            .map(|c| key(&data, c))
            .collect();
        assert_eq!(truth.len(), sc.mcus_y as usize);
        // Many tiny chunks; generous margin (normal sync), then a margin too small to sync (forces
        // the sequential re-decode path), each serial and parallel.
        for (nchunks, margin) in [(40, 4096), (40, 1), (7, 0), (1, 0)] {
            for parallel in [false, true] {
                let cps = row_checkpoints_with(&sc, nchunks, parallel, 1, margin, None)
                    .expect("checkpoints");
                let got: Vec<_> = cps.iter().map(|c| key(&data, c)).collect();
                assert_eq!(got, truth, "nchunks {nchunks} margin {margin}");
            }
        }
        // Truncated data must be rejected, not produce bogus checkpoints.
        let cut = &data[..data.len() * 2 / 3];
        let p = crate::marker::parse(cut, false).unwrap();
        let sc = SeqScan::new(&p, cut).unwrap();
        assert!(row_checkpoints_with(&sc, 16, true, 1, 4096, None).is_none());
    }
}
