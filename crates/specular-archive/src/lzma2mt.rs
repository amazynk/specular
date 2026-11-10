//! A solid 7z block's LZMA2 stream decoded unit by unit, on several threads where the
//! encoder cut it into independent units, and from any unit on.
//!
//! A multi-threaded LZMA2 encoder (7-Zip's default) cuts a stream into units that each begin
//! with a dictionary reset (control byte `0x01`, or `0xE0` and up); the chunk headers declare
//! every chunk's packed and unpacked size, so the units are found without decoding anything,
//! walking the headers only as far as the stream is wanted ([`Plan`]). Each unit decodes on
//! its own, which gives two things:
//!
//! * **Parallel decoding.** [`UnitReader`] reads the stream in order: the unit it is in is
//!   decoded on the reading thread as it is read (so the first entry of a unit is ready as
//!   soon as it is decoded, not when the whole unit is), while helper threads decode the next
//!   units into memory: at most [`MT_MEMORY`] bytes of them at a time, and none past the offset
//!   the reader allows ([`Limiter`]).
//! * **Starting part-way.** A reader may start at any unit's first byte, so reaching an entry
//!   in the middle of a block decodes from the start of its unit, not of the block.
//!
//! A filter after LZMA2 (BCJ for executables, Delta) is position-dependent and keeps state
//! from byte to byte, so it runs over the units' output in order on the reading thread
//! ([`Filter::wrap`]): the units still decode in parallel, but such a stream is only ever
//! read from its start (`Plan::seekable`).
//!
//! The helper threads are this module's own, spawned per stream: they run
//! [`crate::Options::thread_start`] first (the app lowers thumbnail decoders' priority with
//! it) and stop when the reader is dropped (checked every [`STEP`] bytes).

use std::collections::{HashMap, HashSet};
use std::io::{self, Read};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use lzma_rust2::Lzma2Reader;
use lzma_rust2::filter::bcj::BcjReader;
use lzma_rust2::filter::delta::DeltaReader;
use parking_lot::{Condvar, Mutex};

use crate::ByteSource;

/// Bytes of decoded units the helper threads of one stream may hold at once (finished units
/// waiting for the reader, and the ones being decoded).
pub(crate) const MT_MEMORY: u64 = 256 << 20;

/// Bytes a helper thread decodes between checks that its reader still wants them.
const STEP: usize = 1 << 20;

/// One independent unit of an LZMA2 stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Unit {
    /// Its chunks, as offsets into the packed stream.
    pub packed: Range<usize>,
    /// Offset of its first byte in the unpacked stream.
    pub start: u64,
    /// Unpacked bytes.
    pub len: u64,
}

/// The independent units of an LZMA2 stream, from its chunk headers alone ([`Scan`]).
/// `None` when the headers do not walk cleanly to an end marker (or the end of `packed`).
#[cfg(test)]
pub(crate) fn units(packed: &[u8]) -> Option<Vec<Unit>> {
    let mut s = Scan::default();
    while s.end.is_none() {
        s.step(packed);
    }
    (s.end == Some(true)).then_some(s.units)
}

/// A walk over an LZMA2 stream's chunk headers, one chunk at a time ([`Scan::step`]), so a
/// block's units are found as far as they are needed: the first when the archive is opened,
/// the others as the stream is read or an entry further on is asked for. (Walking a whole
/// 300 MB book touches some 5000 pages of it.)
///
/// A unit opened by an uncompressed chunk (`0x01`) must set the LZMA properties in its first
/// LZMA chunk (`0xC0` and up), else it cannot be decoded on its own: the walk takes that for
/// a bad header. (7-Zip's and liblzma's encoders always set them; such a stream used to be
/// read by `BlockDecoder` from its start instead, when the whole walk was done at open.)
#[derive(Default, Debug)]
struct Scan {
    units: Vec<Unit>,
    /// Offset of the next chunk header, and the unpacked bytes before it.
    at: usize,
    total: u64,
    /// The last unit has had no LZMA chunk yet (only uncompressed ones).
    need_props: bool,
    /// The walk ended: cleanly (true) or on a bad header (false).
    end: Option<bool>,
}

impl Scan {
    /// Walks one chunk header.
    fn step(&mut self, packed: &[u8]) {
        if self.end.is_some() {
            return;
        }
        let at = self.at;
        let Some(&control) = packed.get(at) else {
            self.end = Some(!self.units.is_empty());
            return;
        };
        if control == 0x00 {
            self.end = Some(!self.units.is_empty());
            return;
        }
        let word = |i: usize| -> Option<u64> {
            let b = packed.get(at + i..at + i + 2)?;
            Some(u64::from(u16::from_be_bytes([b[0], b[1]])) + 1)
        };
        let chunk = match control {
            0x01 | 0x02 => word(1).map(|n| (n, n, 3)),
            0x80.. => word(1).zip(word(3)).map(|(u, d)| {
                let unpacked = (u64::from(control & 0x1F) << 16) + u;
                (unpacked, d, if control >= 0xC0 { 6 } else { 5 })
            }),
            _ => None,
        };
        let next = chunk.and_then(|(unpacked, data, head)| {
            let next = at
                .checked_add(head)?
                .checked_add(usize::try_from(data).ok()?)?;
            Some((next, self.total.checked_add(unpacked)?, unpacked))
        });
        let Some((next, total, unpacked)) = next.filter(|&(n, _, _)| n <= packed.len()) else {
            self.end = Some(false);
            return;
        };
        // A dictionary reset starts a unit that needs nothing decoded before it.
        if control == 0x01 || control >= 0xE0 || self.units.is_empty() {
            self.units.push(Unit {
                packed: at..at,
                start: self.total,
                len: 0,
            });
            self.need_props = control < 0xC0;
        }
        if control >= 0x80 {
            if control < 0xC0 && self.need_props {
                self.end = Some(false);
                return;
            }
            self.need_props = false;
        }
        let u = self.units.last_mut().expect("a unit");
        u.len += unpacked;
        u.packed.end = next;
        self.at = next;
        self.total = total;
    }

    /// Units whose extent is final: all once the walk ended cleanly, else all but the last
    /// (still growing, or cut by a bad header).
    fn settled(&self) -> usize {
        if self.end == Some(true) {
            self.units.len()
        } else {
            self.units.len().saturating_sub(1)
        }
    }
}

/// A filter applied to LZMA2's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Filter {
    X86,
    Arm,
    Arm64,
    ArmThumb,
    Ppc,
    Sparc,
    Ia64,
    RiscV,
    /// Delta with this distance.
    Delta(usize),
}

impl Filter {
    /// The filter a 7z coder id names (with its properties), if it is one.
    pub(crate) fn of(id: &[u8], props: &[u8]) -> Option<Filter> {
        use sevenz_rust2::EncoderMethod as M;
        Some(match id {
            _ if id == M::ID_BCJ_X86 => Filter::X86,
            _ if id == M::ID_BCJ_ARM => Filter::Arm,
            _ if id == M::ID_BCJ_ARM64 => Filter::Arm64,
            _ if id == M::ID_BCJ_ARM_THUMB => Filter::ArmThumb,
            _ if id == M::ID_BCJ_PPC => Filter::Ppc,
            _ if id == M::ID_BCJ_SPARC => Filter::Sparc,
            _ if id == M::ID_BCJ_IA64 => Filter::Ia64,
            _ if id == M::ID_BCJ_RISCV => Filter::RiscV,
            _ if id == M::ID_DELTA => Filter::Delta(usize::from(*props.first().unwrap_or(&0)) + 1),
            _ => return None,
        })
    }

    /// `r` (the stream from its first byte) seen through the filter.
    pub(crate) fn wrap<'a>(self, r: impl Read + 'a) -> Box<dyn Read + 'a> {
        match self {
            Filter::X86 => Box::new(BcjReader::new_x86(r, 0)),
            Filter::Arm => Box::new(BcjReader::new_arm(r, 0)),
            Filter::Arm64 => Box::new(BcjReader::new_arm64(r, 0)),
            Filter::ArmThumb => Box::new(BcjReader::new_arm_thumb(r, 0)),
            Filter::Ppc => Box::new(BcjReader::new_ppc(r, 0)),
            Filter::Sparc => Box::new(BcjReader::new_sparc(r, 0)),
            Filter::Ia64 => Box::new(BcjReader::new_ia64(r, 0)),
            Filter::RiscV => Box::new(BcjReader::new_riscv(r, 0)),
            Filter::Delta(d) => Box::new(DeltaReader::new(r, d)),
        }
    }
}

/// How a block's stream is decoded: its LZMA2 units (walked as far as needed), dictionary
/// and filter.
pub(crate) struct Plan {
    /// The packed stream in the archive's bytes.
    pub pack: Range<usize>,
    data: Arc<dyn ByteSource>,
    scan: Mutex<Scan>,
    /// Unpacked size the block declares.
    pub total: u64,
    pub dict: u32,
    pub filter: Option<Filter>,
    /// Threads the stream is decoded on (the reading one included), at least 1.
    pub threads: usize,
}

impl std::fmt::Debug for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plan")
            .field("pack", &self.pack)
            .field("scan", &*self.scan.lock())
            .field("threads", &self.threads)
            .finish_non_exhaustive()
    }
}

/// Helper threads for a stream of `total` bytes whose first unit has `first` (the others are
/// taken to be as long): one per further unit, as many as [`MT_MEMORY`] holds, `wanted - 1`
/// at most.
fn helpers(wanted: u32, total: u64, first: u64) -> usize {
    let first = first.max(1);
    let units = total.div_ceil(first);
    let fit = MT_MEMORY / first;
    usize::try_from(units.saturating_sub(1).min(fit))
        .unwrap_or(usize::MAX)
        .min(wanted.max(1) as usize - 1)
}

impl Plan {
    /// The plan for the LZMA2 stream at `pack` in `data`, declared to unpack to `total`
    /// bytes: `None` when its first unit does not walk cleanly. (The rest is walked when
    /// needed; a bad header there fails the reads that reach it.)
    pub(crate) fn new(
        data: &Arc<dyn ByteSource>,
        pack: Range<usize>,
        total: u64,
        dict: u32,
        filter: Option<Filter>,
        wanted: u32,
    ) -> Option<Plan> {
        data.bytes().get(pack.clone())?;
        let plan = Plan {
            pack,
            data: Arc::clone(data),
            scan: Mutex::new(Scan::default()),
            total,
            dict,
            filter,
            threads: 1,
        };
        let first = plan.unit(0).ok()??;
        Some(Plan {
            threads: helpers(wanted, total, first.len) + 1,
            ..plan
        })
    }

    /// Walks the headers until `done` gives an answer, or to their end.
    fn walk<T: Default>(&self, mut done: impl FnMut(&Scan) -> Option<T>) -> T {
        let bytes = self.data.bytes();
        let packed = bytes.get(self.pack.clone()).unwrap_or(&[]);
        let mut s = self.scan.lock();
        loop {
            if let Some(t) = done(&s) {
                return t;
            }
            if s.end.is_some() {
                return T::default();
            }
            s.step(packed);
            // Units that do not add up to the block are as bad as a bad header.
            if s.end == Some(true) && s.total != self.total {
                s.end = Some(false);
            }
        }
    }

    /// Unit `u`: `Ok(None)` past the last, an error when a header before its end is bad.
    pub(crate) fn unit(&self, u: usize) -> io::Result<Option<Unit>> {
        let r = self.walk(|s| {
            if u < s.settled() {
                Some(Some(Ok(s.units[u].clone())))
            } else if s.end == Some(true) {
                Some(None)
            } else if s.end == Some(false) {
                Some(Some(Err(())))
            } else {
                None
            }
        });
        match r {
            Some(Ok(unit)) => Ok(Some(unit)),
            Some(Err(())) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad LZMA2 chunk header",
            )),
            None => Ok(None),
        }
    }

    /// Whether the stream may be read from a unit other than the first (no filter).
    pub(crate) fn seekable(&self) -> bool {
        self.filter.is_none()
    }

    /// The unit holding unpacked offset `at` and where it starts (the last one past the
    /// end, or the last known before a bad header).
    pub(crate) fn unit_of(&self, at: u64) -> (usize, u64) {
        self.walk(|s| {
            let known = &s.units;
            (s.end.is_some() || known.last().is_some_and(|u| u.start > at)).then(|| {
                let u = known.partition_point(|u| u.start <= at).saturating_sub(1);
                (u, known.get(u).map_or(0, |u| u.start))
            })
        })
    }

    /// Every unit, walking the whole stream.
    #[cfg(test)]
    pub(crate) fn units(&self) -> Vec<Unit> {
        self.walk(|s| s.end.map(|_| s.units.clone()))
    }
}

/// Bytes `range` of the archive, read through its shared handle (so a thread may own it).
struct Packed {
    data: Arc<dyn ByteSource>,
    at: usize,
    end: usize,
}

impl Read for Packed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let b = self.data.bytes();
        let src = b.get(self.at..self.end.min(b.len())).unwrap_or(&[]);
        let n = src.len().min(buf.len());
        buf[..n].copy_from_slice(&src[..n]);
        self.at += n;
        Ok(n)
    }
}

/// An LZMA2 reader over unit `u` alone.
fn unit_reader(data: &Arc<dyn ByteSource>, plan: &Plan, unit: &Unit) -> Lzma2Reader<Packed> {
    let packed = Packed {
        data: Arc::clone(data),
        at: plan.pack.start + unit.packed.start,
        end: plan.pack.start + unit.packed.end,
    };
    // Nothing in a unit refers back past its own start: a smaller dictionary is enough.
    let dict = u64::from(plan.dict).min(unit.len.max(4096)) as u32;
    Lzma2Reader::new(packed, dict, None)
}

#[derive(Default)]
struct Slots {
    /// The unit the reader is in.
    cur: usize,
    /// The next unit a helper may take.
    next: usize,
    /// Helpers take only units that start before this unpacked offset ([`Limiter`]).
    limit: u64,
    working: HashSet<usize>,
    /// Units decoded for the reader (with their size, counted in `held` until read).
    done: HashMap<usize, (u64, io::Result<Vec<u8>>)>,
    /// Bytes of the units in `working` and `done`, at most [`MT_MEMORY`].
    held: u64,
}

/// Sees every unit a helper thread decoded (the unit and its bytes) before the reader gets
/// it: the solid 7z streamer keeps an entry it would otherwise only reach later
/// (`ComicInfo.xml`, read in passing).
pub(crate) type Tap = Box<dyn Fn(&Unit, &[u8]) + Send + Sync>;

struct Shared {
    slots: Mutex<Slots>,
    cv: Condvar,
    stop: AtomicBool,
    tap: Option<Tap>,
}

/// The unpacked stream from the start of unit `first` on (see the module docs).
pub(crate) struct UnitReader {
    data: Arc<dyn ByteSource>,
    plan: Arc<Plan>,
    shared: Arc<Shared>,
    /// The unit being read (`None` before the first).
    cur: Option<usize>,
    first: usize,
    /// Unit `cur` decoded here as it is read, and its bytes still to come.
    inline: Option<(Lzma2Reader<Packed>, u64)>,
    /// Unit `cur` decoded by a helper, and the read position in it.
    buf: Option<(Vec<u8>, usize)>,
}

impl UnitReader {
    /// Reads from unit `first` on, with `helpers` helper threads (at most `plan.threads - 1`),
    /// each running `thread_start` first and showing the units it decodes to `tap`.
    pub(crate) fn new(
        data: &Arc<dyn ByteSource>,
        plan: &Arc<Plan>,
        first: usize,
        helpers: usize,
        thread_start: Option<fn()>,
        tap: Option<Tap>,
    ) -> io::Result<UnitReader> {
        let shared = Arc::new(Shared {
            slots: Mutex::new(Slots {
                cur: first,
                next: first + 1,
                ..Slots::default()
            }),
            cv: Condvar::new(),
            stop: AtomicBool::new(false),
            tap,
        });
        let r = UnitReader {
            data: Arc::clone(data),
            plan: Arc::clone(plan),
            shared,
            cur: None,
            first,
            inline: None,
            buf: None,
        };
        for k in 1..=helpers.min(plan.threads - 1) {
            let (data, plan, shared) = (
                Arc::clone(&r.data),
                Arc::clone(&r.plan),
                Arc::clone(&r.shared),
            );
            specular_thread::builder(format!("arc-7z-unit-{k}")).spawn(move || {
                if let Some(f) = thread_start {
                    f();
                }
                helper(&data, &plan, &shared);
            })?;
        }
        Ok(r)
    }

    /// What lets the helpers decode further ahead.
    pub(crate) fn limiter(&self) -> Limiter {
        Limiter(Arc::clone(&self.shared))
    }

    /// Moves to the next unit; false at the end of the stream.
    fn advance(&mut self) -> io::Result<bool> {
        let u = self.cur.map_or(self.first, |c| c + 1);
        self.cur = Some(u);
        self.inline = None;
        self.buf = None;
        let Some(unit) = self.plan.unit(u)? else {
            return Ok(false);
        };
        let mut s = self.shared.slots.lock();
        s.cur = u;
        self.shared.cv.notify_all();
        loop {
            if let Some((len, r)) = s.done.remove(&u) {
                s.held -= len;
                self.buf = Some((r?, 0));
                return Ok(true);
            }
            if !s.working.contains(&u) {
                // Nobody took it: decode it here, as it is read.
                s.next = s.next.max(u + 1);
                self.inline = Some((unit_reader(&self.data, &self.plan, &unit), unit.len));
                return Ok(true);
            }
            self.shared.cv.wait(&mut s);
        }
    }
}

impl Read for UnitReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if let Some((r, left)) = self.inline.as_mut()
                && *left > 0
            {
                let n = (*left).min(out.len() as u64) as usize;
                let got = r.read(&mut out[..n])?;
                if got == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "LZMA2 unit ended early",
                    ));
                }
                *left -= got as u64;
                return Ok(got);
            }
            if let Some((b, at)) = self.buf.as_mut()
                && *at < b.len()
            {
                let n = (b.len() - *at).min(out.len());
                out[..n].copy_from_slice(&b[*at..*at + n]);
                *at += n;
                return Ok(n);
            }
            if !self.advance()? {
                return Ok(0);
            }
        }
    }
}

impl Drop for UnitReader {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        // Units decoded for nobody (the shared state may outlive the reader, a `Limiter`).
        let mut s = self.shared.slots.lock();
        for (_, (len, _)) in std::mem::take(&mut s.done) {
            s.held -= len;
        }
        drop(s);
        self.shared.cv.notify_all();
    }
}

/// How far ahead a [`UnitReader`]'s helpers may decode: only units that start before the
/// offset last set (nothing at first), so a reader that is not read stops costing memory
/// and CPU once the units before that offset are done.
#[derive(Clone)]
pub(crate) struct Limiter(Arc<Shared>);

impl Limiter {
    pub(crate) fn set(&self, limit: u64) {
        let mut s = self.0.slots.lock();
        if s.limit != limit {
            s.limit = limit;
            self.0.cv.notify_all();
        }
    }

    /// Whether unit `u` is being read already, or is decoded or being decoded by a helper,
    /// so reading on to it costs no more than copying. (The reader may have moved into it
    /// before its first entry is due.)
    pub(crate) fn taken(&self, u: usize) -> bool {
        let s = self.0.slots.lock();
        u <= s.cur || s.done.contains_key(&u) || s.working.contains(&u)
    }

    /// Whether a helper is decoding unit `u` right now (it shows it to the tap when done).
    pub(crate) fn working(&self, u: usize) -> bool {
        self.0.slots.lock().working.contains(&u)
    }
}

/// A helper thread: takes the next unit that starts before the reader's limit and fits in
/// [`MT_MEMORY`] with the others held, decodes it, hands it over; until the stream ends (or
/// a bad header: the reader meets it itself) or the reader is gone.
fn helper(data: &Arc<dyn ByteSource>, plan: &Plan, shared: &Shared) {
    let mut s = shared.slots.lock();
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        let u = s.next;
        // Walking the headers to its end (once per unit) is done without the lock.
        drop(s);
        let Ok(Some(unit)) = plan.unit(u) else {
            return;
        };
        s = shared.slots.lock();
        if s.next != u {
            continue;
        }
        if s.held + unit.len > MT_MEMORY || unit.start >= s.limit {
            shared.cv.wait(&mut s);
            continue;
        }
        s.next += 1;
        s.held += unit.len;
        s.working.insert(u);
        drop(s);
        let res = decode(data, plan, &unit, &shared.stop);
        if let (Some(tap), Ok(bytes)) = (&shared.tap, &res) {
            tap(&unit, bytes);
        }
        s = shared.slots.lock();
        s.working.remove(&u);
        // A unit the reader already passed (it restarted elsewhere) is dropped.
        if u >= s.cur && !shared.stop.load(Ordering::Relaxed) {
            s.done.insert(u, (unit.len, res));
        } else {
            s.held -= unit.len;
        }
        shared.cv.notify_all();
    }
}

/// `unit` decoded whole, checking `stop` every [`STEP`] bytes.
fn decode(
    data: &Arc<dyn ByteSource>,
    plan: &Plan,
    unit: &Unit,
    stop: &AtomicBool,
) -> io::Result<Vec<u8>> {
    let len = usize::try_from(unit.len).map_err(io::Error::other)?;
    let mut r = unit_reader(data, plan, unit);
    let mut out = vec![0u8; len];
    let mut at = 0;
    while at < len {
        if stop.load(Ordering::Relaxed) {
            return Err(io::Error::other("stopped"));
        }
        let end = (at + STEP).min(len);
        r.read_exact(&mut out[at..end])?;
        at = end;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_scan() {
        // Uncompressed chunks: 0x01 resets the dictionary, 0x02 does not.
        let mut s = vec![0x01, 0, 9];
        s.extend([7; 10]);
        s.extend([0x02, 0, 4]);
        s.extend([7; 5]);
        s.extend([0x01, 0, 1]);
        s.extend([7; 2]);
        s.push(0);
        let u = units(&s).unwrap();
        assert_eq!(
            u,
            [
                Unit {
                    packed: 0..21,
                    start: 0,
                    len: 15
                },
                Unit {
                    packed: 21..26,
                    start: 15,
                    len: 2
                }
            ]
        );
        // An LZMA chunk with new properties and a dictionary reset (sizes 0x1_0001 and 3).
        let mut l = vec![0xE1, 0x00, 0x00, 0x00, 0x02, 0x5D];
        l.extend([0; 3]);
        assert_eq!(units(&l).unwrap()[0].len, 0x1_0001);
        // After an uncompressed reset, the first LZMA chunk must set properties.
        let mut bad = vec![0x01, 0, 0, 7];
        bad.extend([0xA0, 0, 0, 0, 0, 1]);
        assert_eq!(units(&bad), None);
        let mut ok = vec![0x01, 0, 0, 7];
        ok.extend([0xC0, 0, 0, 0, 0, 0x5D, 1]);
        assert_eq!(units(&ok).map(|u| (u.len(), u[0].len)), Some((1, 2)));
        // Truncated or invalid.
        assert_eq!(units(&[0x01, 0, 9, 1]), None);
        assert_eq!(units(&[0x05]), None);
        assert_eq!(units(&[0xE0, 0]), None);
    }

    /// An LZMA2 stream of uncompressed units of `lens` bytes (chunks of at most 64 KiB).
    fn stored(lens: &[usize]) -> Vec<u8> {
        let mut s = Vec::new();
        for &len in lens {
            let mut left = len;
            let mut control = 0x01;
            while left > 0 {
                let n = left.min(1 << 16);
                s.push(control);
                s.extend_from_slice(&((n - 1) as u16).to_be_bytes());
                s.extend(std::iter::repeat_n(7, n));
                left -= n;
                control = 0x02;
            }
        }
        s.push(0);
        s
    }

    fn plan_of(stream: Vec<u8>, total: u64, wanted: u32) -> Option<Plan> {
        let pack = 0..stream.len();
        let data: Arc<dyn ByteSource> = Arc::new(stream);
        Plan::new(&data, pack, total, 1 << 20, None, wanted)
    }

    #[test]
    fn plan_threads_follow_units_and_memory() {
        assert_eq!(helpers(8, 5 << 20, 1 << 20), 4);
        // 128 MiB units: two may be held, so two helpers.
        assert_eq!(helpers(8, 3 << 27, 1 << 27), 2);
        assert_eq!(helpers(8, 1 << 30, 1 << 30), 0);
        assert_eq!(helpers(3, 100 << 20, 1 << 20), 2);
        assert_eq!(helpers(0, 100 << 20, 1 << 20), 0);
        let p = plan_of(stored(&[100_000; 5]), 500_000, 8).unwrap();
        assert_eq!(p.threads, 5);
        assert!(p.seekable());
        // Only the first unit was walked.
        assert_eq!(p.scan.lock().units.len(), 2);
        assert_eq!(p.unit_of(0), (0, 0));
        assert_eq!(p.unit_of(300_005), (3, 300_000));
        assert_eq!(p.unit_of(u64::MAX), (4, 400_000));
        assert_eq!(p.unit(4).unwrap().map(|u| u.len), Some(100_000));
        assert_eq!(p.unit(5).unwrap(), None);
        assert_eq!(p.units().len(), 5);
    }

    #[test]
    fn a_bad_header_past_the_first_unit_fails_the_units_after_it() {
        let mut s = stored(&[70_000, 70_000, 70_000]);
        // The second unit's second chunk header turns invalid.
        let at = 70_000 + 6 + 65_536 + 3;
        s[at] = 0x05;
        let p = plan_of(s.clone(), 210_000, 4).unwrap();
        assert_eq!(p.unit(0).unwrap().map(|u| u.len), Some(70_000));
        assert!(p.unit(1).is_err());
        assert!(p.unit(2).is_err());
        assert_eq!(p.unit_of(150_000), (1, 70_000));
        // Units that do not add up to the declared size fail at the last.
        let p = plan_of(stored(&[70_000, 70_000]), 150_000, 4).unwrap();
        assert!(p.unit(0).unwrap().is_some());
        assert!(p.unit(1).is_err());
        // A first unit that does not walk: no plan.
        s[3 + 65_536] = 0x05;
        assert!(plan_of(s, 210_000, 4).is_none());
    }
}
