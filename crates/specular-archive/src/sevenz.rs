//! 7z / CB7 through `sevenz-rust2` (Apache-2.0, pure Rust: LZMA, LZMA2, PPMd, Deflate, BCJ and
//! Delta filters). Writing a 7z header parser plus LZMA/LZMA2/PPMd decoders is a project of its
//! own; this crate is maintained, fuzzed upstream and already depends only on permissive crates.
//! We use it without its AES, bzip2 (licence) and writer features.
//!
//! Random access: a non-solid block holds one file and is decoded directly on the calling thread,
//! so several such entries decode in parallel. A solid block must be decoded from its start (or,
//! for LZMA2, from the start of an independent unit, below). Each solid block being read gets a
//! *streamer* thread that decodes its entries in order into a byte-budgeted cache
//! ([`crate::Options::solid_cache_bytes`]) shared by every reader of the block: a reader takes
//! its entry from the cache, or waits (without holding the lock) until the streamer puts it
//! there. The streamer runs [`crate::Options::solid_read_ahead`] bytes past the furthest entry
//! asked for and then pauses, so the next pages are ready when the viewer turns to them and
//! sequential reading stays linear. An entry behind the streamer and no longer cached restarts
//! it (when no other reader still waits for something ahead of it). The thread exits when the
//! archive is dropped.
//!
//! **LZMA2 units.** A multi-threaded LZMA2 encoder (7-Zip's default) cuts a block's stream into
//! units that each begin with a dictionary reset; their chunk headers are walked as far as
//! needed (no decoding, [`Plan`]): the first unit when the archive is opened, the rest
//! as the stream is read or an entry further on is asked for. A block whose coders are LZMA2
//! alone (or followed by one BCJ or Delta filter) and whose first unit walks cleanly is then
//! decoded by [`UnitReader`] instead of `BlockDecoder`:
//!
//! * The streamer starts at the unit holding the entry asked for, not at the block's start,
//!   and an entry more than a unit ahead of it starts a new one there. Jumping to the middle
//!   of a book decodes at most one unit before the page. (A filtered block always starts at
//!   its beginning: the filter is position-dependent.)
//! * With [`crate::Options::decode_threads`] above 1, helper threads decode the next units
//!   while the streamer decodes the current one (the first page is decoded on the streamer
//!   itself, as soon as it is reached). They hold at most
//!   [`MT_MEMORY`](crate::lzma2mt::MT_MEMORY) bytes of decoded units (the headers declare
//!   every unpacked size, so a hostile unit cannot grow past it),
//!   and take a unit only when it starts within the read-ahead of the furthest entry asked
//!   for, or within as many bytes as the readers have come since the stream started (a
//!   growing window: a page read on its own costs a unit or two, a book read through keeps
//!   every helper busy), or within three times as far again past the last entry of a batch read
//!   (`Archive::read_many`, the gallery's thumbnails of an archive's pages, batch after
//!   batch).
//! * Entries already read are evicted from the cache first, so the read-ahead of a reader
//!   that pauses (a thumbnail worker making one between its reads) is still there for it.
//!
//! The entries' CRC-32 are checked here when [`crate::Options::verify_crc`] is set.
//! Other blocks (LZMA, PPMd, BCJ2, …) go through `BlockDecoder` on one thread.
//! The streamer and the helpers first run [`crate::Options::thread_start`].
//!
//! Encrypted blocks (AES-256 + SHA-256) and encrypted headers are decrypted by `sevenz-rust2`
//! with [`crate::Options::password`]; such blocks always go through `BlockDecoder`. 7z has no
//! password check: a wrong password shows up as a decoding or CRC failure, reported as
//! [`Error::WrongPassword`] for encrypted entries.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{Cursor, Read};
use std::sync::Arc;

use parking_lot::{Condvar, Mutex};
use sevenz_rust2::{Archive as SzArchive, ArchiveEntry, BlockDecoder, EncoderMethod, Password};

use crate::lzma2mt::{Filter, Limiter, Plan, UnitReader};
use crate::names::normalize;
use crate::{ByteSource, Entry, Error, Loc, Method, Options, Result};

pub(crate) const SIGNATURE: &[u8] = b"7z\xBC\xAF\x27\x1C";

fn malformed(e: impl std::fmt::Display) -> Error {
    Error::Malformed(format!("7z: {e}"))
}

fn map_err(e: sevenz_rust2::Error) -> Error {
    match e {
        sevenz_rust2::Error::PasswordRequired => Error::Encrypted,
        sevenz_rust2::Error::MaybeBadPassword(_) => Error::WrongPassword,
        sevenz_rust2::Error::UnsupportedCompressionMethod(m) => {
            Error::Unsupported(format!("7z method {m}"))
        }
        sevenz_rust2::Error::ChecksumVerificationFailed => Error::Checksum,
        sevenz_rust2::Error::MaxMemLimited { .. } => Error::Limit("7z decoder memory".into()),
        e => malformed(e),
    }
}

/// The same error again, for every reader that waited on a stream that failed.
fn dup(e: &Error) -> Error {
    match e {
        Error::Io(e) => Error::Io(std::io::Error::new(e.kind(), e.to_string())),
        Error::NotAnArchive => Error::NotAnArchive,
        Error::Malformed(m) => Error::Malformed(m.clone()),
        Error::Unsupported(m) => Error::Unsupported(m.clone()),
        Error::NeedsSandbox(f) => Error::NeedsSandbox(*f),
        Error::Encrypted => Error::Encrypted,
        Error::WrongPassword => Error::WrongPassword,
        Error::Limit(m) => Error::Limit(m.clone()),
        Error::Checksum => Error::Checksum,
        Error::NoEntry(i) => Error::NoEntry(*i),
    }
}

/// A file of a solid block: where it lies in the block's unpacked stream.
#[derive(Clone, Copy, Debug)]
struct Span {
    file: usize,
    start: u64,
    size: u64,
    crc: Option<u32>,
}

impl Span {
    fn end(&self) -> u64 {
        self.start + self.size
    }
}

/// The streamer currently decoding a solid block.
struct Ctl {
    /// Which streamer this is: one that sees another number in the state stops.
    epoch: u64,
    /// Place (in the block) of the next entry it delivers.
    next: usize,
    /// The LZMA2 unit it is in (0 without a plan).
    unit: usize,
    /// Unpacked offset of the entry it decodes or waits to decode.
    at: u64,
    /// Its LZMA2 helpers, once it has them.
    helpers: Option<Limiter>,
    /// It stopped at entry `next` on this error.
    failed: Option<Error>,
    /// It reached the end of the block.
    ended: bool,
}

#[derive(Default)]
struct SolidState {
    /// Decoded entries by place in the block: those not read yet in `order` (as decoded),
    /// those read in `read` (least recently first), evicted first (a reader seldom asks for
    /// a page twice, and evicting the read-ahead before it is read restarts the stream).
    cache: HashMap<usize, Vec<u8>>,
    order: VecDeque<usize>,
    read: VecDeque<usize>,
    cached: usize,
    /// Entries passed over because a limit refused them.
    limited: HashMap<usize, String>,
    /// Places readers wait for (with how many readers): never evicted, and a reason not to
    /// restart the streamer elsewhere.
    waiting: BTreeMap<usize, usize>,
    /// End (unpacked offset) of the furthest entry asked for since the streamer started.
    horizon: u64,
    /// Batch reads under way ([`SevenZ::will_read`]), and the end of the furthest entry they
    /// are going to ask for (0 when none is).
    batches: usize,
    wanted: u64,
    stream: Option<Ctl>,
    epoch: u64,
    /// Place of the entry kept whenever a decode passes over it ([`SevenZ::capture`]), its
    /// bytes once one has, and what runs then ([`SevenZ::on_kept`]).
    keep: Option<usize>,
    kept: Option<Arc<[u8]>>,
    hooks: Vec<Hook>,
}

/// Runs once the entry a block keeps in passing has been decoded, with its bytes.
pub(crate) type Hook = Box<dyn FnOnce(Arc<[u8]>) + Send>;

impl SolidState {
    fn insert(&mut self, p: usize, data: Vec<u8>, budget: usize) {
        if data.len() > budget && !self.waiting.contains_key(&p) {
            return;
        }
        self.cached += data.len();
        if let Some(v) = self.cache.insert(p, data) {
            self.cached -= v.len();
            self.order.retain(|&q| q != p);
            self.read.retain(|&q| q != p);
        }
        self.order.push_back(p);
        self.trim(budget);
    }

    /// Evicts entries nobody waits for until `budget` is met: those read (least recently
    /// first), then those not read yet (the earliest decoded first).
    fn trim(&mut self, budget: usize) {
        for read in [true, false] {
            let mut kept = Vec::new();
            while self.cached > budget {
                let q = if read {
                    &mut self.read
                } else {
                    &mut self.order
                };
                let Some(old) = q.pop_front() else {
                    break;
                };
                if self.waiting.contains_key(&old) {
                    kept.push(old);
                } else if let Some(v) = self.cache.remove(&old) {
                    self.cached -= v.len();
                }
            }
            let q = if read {
                &mut self.read
            } else {
                &mut self.order
            };
            for p in kept.into_iter().rev() {
                q.push_front(p);
            }
        }
    }

    fn get(&mut self, p: usize) -> Option<Vec<u8>> {
        let v = self.cache.get(&p)?.clone();
        if self.read.back() != Some(&p) {
            if self.order.front() == Some(&p) {
                self.order.pop_front();
            } else {
                self.order.retain(|&q| q != p);
            }
            self.read.retain(|&q| q != p);
            self.read.push_back(p);
        }
        Some(v)
    }

    /// The bytes of entry `p` when it is the one kept in passing and has been decoded.
    fn kept(&self, p: usize) -> Option<Arc<[u8]>> {
        self.kept.clone().filter(|_| self.keep == Some(p))
    }

    /// Keeps `bytes` as entry `p`'s when it is the one to keep and nothing has yet; the
    /// hooks to run (outside the lock) with them.
    #[must_use]
    fn keep(&mut self, p: usize, bytes: &[u8]) -> Option<(Arc<[u8]>, Vec<Hook>)> {
        if self.keep != Some(p) || self.kept.is_some() {
            return None;
        }
        let b: Arc<[u8]> = bytes.into();
        self.kept = Some(Arc::clone(&b));
        Some((b, std::mem::take(&mut self.hooks)))
    }

    /// Whether a reader waits for a place in `range`.
    fn waited(&self, range: std::ops::Range<usize>) -> bool {
        !range.is_empty() && self.waiting.range(range).next().is_some()
    }
}

struct Solid {
    state: Mutex<SolidState>,
    cv: Condvar,
}

impl Solid {
    /// [`SolidState::keep`], waking whoever waits for it and running the hooks.
    fn keep(&self, p: usize, bytes: &[u8]) {
        let got = self.state.lock().keep(p, bytes);
        if let Some((b, hooks)) = got {
            self.cv.notify_all();
            run_hooks(&b, hooks);
        }
    }
}

fn run_hooks(b: &Arc<[u8]>, hooks: Vec<Hook>) {
    for h in hooks {
        h(Arc::clone(b));
    }
}

struct Block {
    /// More than one file: must be decoded from the start.
    solid: bool,
    /// Why reading from this block is refused (memory or ratio limit), checked once at open.
    refused: Option<String>,
    /// Its files with a stream, in stream order.
    spans: Arc<[Span]>,
    /// How [`lzma2mt`] decodes it, when it can.
    plan: Option<Arc<Plan>>,
    shared: Arc<Solid>,
}

impl Block {
    /// Place of `file` in the block.
    fn place(&self, file: usize) -> Option<usize> {
        self.spans.binary_search_by_key(&file, |s| s.file).ok()
    }

    /// Where a streamer for place `p` starts: its LZMA2 unit, the first place in it and the
    /// unit's offset (walking the LZMA2 headers that far, [`Plan::unit_of`]).
    fn start_for(&self, p: usize) -> (usize, usize, u64) {
        match &self.plan {
            Some(plan) if plan.seekable() => {
                let (u, at) = plan.unit_of(self.spans[p].start);
                (u, self.spans.partition_point(|s| s.start < at), at)
            }
            _ => (0, 0, 0),
        }
    }
}

pub(crate) struct SevenZ {
    archive: Arc<SzArchive>,
    blocks: Vec<Block>,
    password: Option<crate::Password>,
}

/// `sevenz-rust2`'s form of `p` (UTF-16LE). It keeps its copy unwiped, and only for the call
/// it is made for.
fn sz_password(p: Option<&crate::Password>) -> Password {
    p.map_or_else(Password::empty, |p| Password::new(p.as_str()))
}

impl Drop for SevenZ {
    fn drop(&mut self) {
        // Streamers see no stream of theirs and stop (a paused one is woken for it).
        for b in &self.blocks {
            b.shared.state.lock().stream = None;
            b.shared.cv.notify_all();
        }
    }
}

fn u32_le(b: &[u8], at: usize) -> Option<u64> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as u64)
}

/// Dictionary or model memory a coder asks for.
fn coder_memory(id: &[u8], props: &[u8]) -> Option<u64> {
    if id == EncoderMethod::ID_LZMA || id == EncoderMethod::ID_PPMD {
        u32_le(props, 1)
    } else if id == EncoderMethod::ID_LZMA2 {
        let b = *props.first()? as u64;
        if b > 40 {
            None
        } else if b == 40 {
            Some(u32::MAX as u64)
        } else {
            Some((2 | (b & 1)) << (b / 2 + 11))
        }
    } else {
        Some(0)
    }
}

/// Memory sevenz-rust2's `BlockDecoder` allocates for block `blk`'s coders: LZMA2
/// dictionaries and PPMd models as declared (it does not clamp them to the data), LZMA ones
/// clamped to the coder's unpacked size (lzma-rust2 does that).
fn decoder_memory(blk: &sevenz_rust2::Block) -> u64 {
    blk.coders
        .iter()
        .map(|c| {
            let m = coder_memory(c.encoder_method_id(), c.properties()).unwrap_or(0);
            if c.encoder_method_id() == EncoderMethod::ID_LZMA {
                m.min(blk.get_unpack_size_for_coder(c)).max(4096)
            } else {
                m
            }
        })
        .fold(0u64, u64::saturating_add)
}

/// Where block `b`'s (first) packed stream lies in the archive.
fn pack_range(a: &SzArchive, b: usize) -> Option<std::ops::Range<usize>> {
    let map = &a.stream_map;
    let first = *map.block_first_pack_stream_index().get(b)?;
    let offset = *map.pack_stream_offsets().get(first)?;
    let len = *a.pack_sizes().get(first)?;
    let start = usize::try_from(32u64.checked_add(a.pack_pos())?.checked_add(offset)?).ok()?;
    Some(start..start.checked_add(usize::try_from(len).ok()?)?)
}

/// Block `blk`'s coders when they are LZMA2, optionally followed by one BCJ or Delta filter:
/// the LZMA2 coder's index, the coder and the filter.
fn lzma2_chain(blk: &sevenz_rust2::Block) -> Option<(usize, &sevenz_rust2::Coder, Option<Filter>)> {
    let order: Vec<(usize, &sevenz_rust2::Coder)> = blk.ordered_coder_iter().collect();
    if order.len() != blk.coders.len() || order.is_empty() || order.len() > 2 {
        return None;
    }
    let (lz, lzma2) = order[0];
    if lzma2.encoder_method_id() != EncoderMethod::ID_LZMA2 {
        return None;
    }
    let filter = match order.get(1) {
        Some((_, c)) => Some(Filter::of(c.encoder_method_id(), c.properties())?),
        None => None,
    };
    Some((lz, lzma2, filter))
}

fn block_method(block: &sevenz_rust2::Block) -> (Method, bool) {
    let mut encrypted = false;
    let mut method = None;
    for c in &block.coders {
        let id = c.encoder_method_id();
        if id == EncoderMethod::ID_AES256_SHA256 {
            encrypted = true;
            continue;
        }
        let m = if id == EncoderMethod::ID_LZMA2 {
            Method::Lzma2
        } else if id == EncoderMethod::ID_LZMA {
            Method::Lzma
        } else if id == EncoderMethod::ID_PPMD {
            Method::Ppmd
        } else if id == EncoderMethod::ID_DEFLATE {
            Method::Deflate
        } else if id == EncoderMethod::ID_COPY {
            Method::Stored
        } else {
            match EncoderMethod::by_id(id) {
                // Filters (BCJ, Delta) are not the compression method.
                Some(m) if m.name().starts_with("BCJ") || m.name() == "DELTA" => continue,
                Some(m) => Method::Other(m.name().to_owned()),
                None => Method::Other(format!("7z method {id:02X?}")),
            }
        };
        if method.is_none() || method == Some(Method::Stored) {
            method = Some(m);
        }
    }
    (method.unwrap_or(Method::Stored), encrypted)
}

/// How block `b` is decoded by [`lzma2mt`]: when its coders are LZMA2, optionally followed by
/// one BCJ or Delta filter, its LZMA2 stream walks into units cleanly, and its sizes and CRCs
/// agree with its files. `None` sends it to `BlockDecoder`.
fn plan(
    a: &SzArchive,
    b: usize,
    data: &Arc<dyn ByteSource>,
    spans: &[Span],
    threads: u32,
) -> Option<Plan> {
    let blk = a.blocks.get(b)?;
    let (lz, lzma2, filter) = lzma2_chain(blk)?;
    // Every entry is checked by its own CRC, or the block has none either.
    if blk.has_crc && spans.iter().any(|s| s.crc.is_none()) {
        return None;
    }
    let dict = u32::try_from(coder_memory(lzma2.encoder_method_id(), lzma2.properties())?).ok()?;
    let pack = pack_range(a, b)?;
    let total = blk.get_unpack_size();
    let files = spans.last().map_or(0, Span::end);
    if total != blk.get_unpack_size_at_index(lz) || files > total {
        return None;
    }
    Plan::new(data, pack, total, dict, filter, threads)
}

/// The most decoder memory a packed 7z header may ask for. sevenz-rust2 decodes the header
/// before our block limits run, with no memory limit: a 100-byte file whose header coder
/// declares a 4 GiB LZMA2 dictionary had it allocated and zeroed. Real headers are small.
const MAX_HEADER_DECODER_MEMORY: u64 = 64 << 20;

/// A 7z NUMBER: the first byte's leading one bits count the extra bytes.
fn sz_number(d: &[u8], p: &mut usize) -> Option<u64> {
    let first = *d.get(*p)?;
    *p += 1;
    let extra = first.leading_ones() as usize;
    let mut v = 0u64;
    for i in 0..extra.min(8) {
        v |= u64::from(*d.get(*p + i)?) << (8 * i);
    }
    *p += extra.min(8);
    if extra < 8 {
        let high = u64::from(first) & ((1u64 << (7 - extra)) - 1);
        v |= high << (8 * extra);
    }
    Some(v)
}

/// The decoder memory the coders of a packed ("encoded") header ask for, read from the raw
/// bytes; `None` when the header is not packed or not understood (sevenz-rust2 then judges).
fn header_coder_memory(d: &[u8]) -> Option<u64> {
    if !d.starts_with(b"7z\xbc\xaf\x27\x1c") {
        return None;
    }
    let off = usize::try_from(u64::from_le_bytes(d.get(12..20)?.try_into().ok()?)).ok()?;
    let mut p = 32usize.checked_add(off)?;
    // kEncodedHeader, then StreamsInfo: kPackInfo (skipped to its kEnd), kUnPackInfo.
    if *d.get(p)? != 0x17 {
        return None;
    }
    p += 1;
    if *d.get(p)? == 0x06 {
        p += 1;
        sz_number(d, &mut p)?;
        let n = sz_number(d, &mut p)?;
        loop {
            match *d.get(p)? {
                0x00 => {
                    p += 1;
                    break;
                }
                0x09 => {
                    p += 1;
                    for _ in 0..n.min(1 << 16) {
                        sz_number(d, &mut p)?;
                    }
                }
                _ => return None,
            }
        }
    }
    if d.get(p..p + 2)? != [0x07, 0x0b] {
        return None;
    }
    p += 2;
    let folders = sz_number(d, &mut p)?;
    if *d.get(p)? != 0 {
        return None;
    }
    p += 1;
    let mut most = 0u64;
    for _ in 0..folders.min(64) {
        let coders = sz_number(d, &mut p)?;
        let (mut ins, mut outs) = (0u64, 0u64);
        for _ in 0..coders.min(64) {
            let flag = *d.get(p)?;
            p += 1;
            let id = d.get(p..p + usize::from(flag & 0x0f))?;
            p += usize::from(flag & 0x0f);
            if flag & 0x10 != 0 {
                // Stream counts are the file's word: a forged one must not overflow.
                ins = ins.checked_add(sz_number(d, &mut p)?)?;
                outs = outs.checked_add(sz_number(d, &mut p)?)?;
            } else {
                ins = ins.checked_add(1)?;
                outs = outs.checked_add(1)?;
            }
            let props: &[u8] = if flag & 0x20 != 0 {
                let n = usize::try_from(sz_number(d, &mut p)?).ok()?;
                let v = d.get(p..p.checked_add(n)?)?;
                p += n;
                v
            } else {
                &[]
            };
            most = most.max(coder_memory(id, props).unwrap_or(u64::MAX));
        }
        let pairs = outs.saturating_sub(1);
        for _ in 0..pairs.min(64) {
            sz_number(d, &mut p)?;
            sz_number(d, &mut p)?;
        }
        if ins.saturating_sub(pairs) > 1 {
            for _ in 0..ins.saturating_sub(pairs).min(64) {
                sz_number(d, &mut p)?;
            }
        }
    }
    Some(most)
}

pub(crate) fn open(data: &Arc<dyn ByteSource>, options: &Options) -> Result<(Vec<Entry>, SevenZ)> {
    let bytes = data.bytes();
    let limits = &options.limits;
    let password = options.password.as_ref();
    if let Some(m) = header_coder_memory(bytes)
        && m > MAX_HEADER_DECODER_MEMORY
    {
        return Err(Error::Limit(format!(
            "7z header needs {m} bytes of decoder memory (limit {MAX_HEADER_DECODER_MEMORY})"
        )));
    }
    let archive = match SzArchive::read(&mut Cursor::new(bytes), &sz_password(password)) {
        Ok(a) => a,
        // With a password, a header that does not open is a wrong password exactly when it
        // does not open without one for want of a password (7z stores no password check).
        Err(e) if password.is_some() => {
            return Err(
                match SzArchive::read(&mut Cursor::new(bytes), &Password::empty()) {
                    Err(sevenz_rust2::Error::PasswordRequired) => Error::WrongPassword,
                    _ => map_err(e),
                },
            );
        }
        Err(e) => return Err(map_err(e)),
    };
    if archive.files.len() > limits.max_entries {
        return Err(Error::Limit(format!(
            "{} entries exceed the limit of {}",
            archive.files.len(),
            limits.max_entries
        )));
    }
    let map = &archive.stream_map;
    let mut spans: Vec<Vec<Span>> = vec![Vec::new(); archive.blocks.len()];
    for (i, f) in archive.files.iter().enumerate() {
        if let Some(Some(b)) = map.file_block_index.get(i)
            && f.has_stream()
            && let Some(v) = spans.get_mut(*b)
        {
            let start = v.last().map_or(0, Span::end);
            v.push(Span {
                file: i,
                start,
                size: f.size(),
                crc: f.has_crc.then_some(f.crc as u32),
            });
        }
    }
    let mut blocks = Vec::with_capacity(archive.blocks.len());
    let mut block_info = Vec::with_capacity(archive.blocks.len());
    for ((i, block), spans) in archive.blocks.iter().enumerate().zip(spans) {
        let pack = map
            .block_first_pack_stream_index()
            .get(i)
            .and_then(|&p| archive.pack_sizes().get(p))
            .copied()
            .unwrap_or(0);
        let unpack = block.get_unpack_size();
        let mut refused = None;
        for c in &block.coders {
            match coder_memory(c.encoder_method_id(), c.properties()) {
                Some(m) if m <= limits.max_decoder_memory => {}
                Some(m) => {
                    refused = Some(format!(
                        "7z block needs {m} bytes of decoder memory (limit {})",
                        limits.max_decoder_memory
                    ))
                }
                None => refused = Some("7z coder properties invalid".into()),
            }
        }
        if let Err(Error::Limit(m)) = limits.check_ratio(unpack, pack) {
            refused = Some(m);
        }
        let files_in_block = map
            .file_block_index
            .iter()
            .filter(|&&b| b == Some(i))
            .count();
        let solid = files_in_block > 1;
        let plan = if solid && refused.is_none() {
            plan(&archive, i, data, &spans, options.decode_threads).map(Arc::new)
        } else {
            None
        };
        blocks.push(Block {
            solid,
            refused,
            spans: spans.into(),
            plan,
            shared: Arc::new(Solid {
                state: Mutex::new(SolidState::default()),
                cv: Condvar::new(),
            }),
        });
        block_info.push((block_method(block), pack, files_in_block));
    }

    let mut entries = Vec::new();
    for (i, f) in archive.files.iter().enumerate() {
        if f.is_directory() || f.is_anti_item() {
            continue;
        }
        let block = map.file_block_index.get(i).copied().flatten();
        let ((method, encrypted), compressed) = match block.and_then(|b| block_info.get(b)) {
            Some(((m, enc), pack, n)) => ((m.clone(), *enc), if *n == 1 { *pack } else { 0 }),
            None => ((Method::Stored, false), 0),
        };
        entries.push(Entry {
            name: normalize(f.name()),
            size: f.size(),
            compressed_size: compressed,
            method,
            encrypted,
            loc: Loc::SevenZ { file: i },
        });
    }
    Ok((
        entries,
        SevenZ {
            archive: Arc::new(archive),
            blocks,
            password: options.password.clone(),
        },
    ))
}

/// Index of `entry` in `files` (it is always an element of that slice).
fn file_index(files: &[ArchiveEntry], entry: &ArchiveEntry) -> Option<usize> {
    let base = files.as_ptr() as usize;
    let at = entry as *const ArchiveEntry as usize;
    let size = size_of::<ArchiveEntry>().max(1);
    let i = at.checked_sub(base)? / size;
    (i < files.len() && std::ptr::eq(&files[i], entry)).then_some(i)
}

/// Reads one entry's stream fully, never past `limit` bytes.
fn read_entry(r: &mut dyn Read, size: u64, out: &mut Vec<u8>) -> Result<()> {
    out.reserve(size.min(64 << 20) as usize);
    r.take(size.saturating_add(1))
        .read_to_end(out)
        .map_err(|e| {
            match e
                .get_ref()
                .and_then(|x| x.downcast_ref::<sevenz_rust2::Error>())
            {
                Some(sevenz_rust2::Error::ChecksumVerificationFailed) => Error::Checksum,
                _ => malformed(e),
            }
        })?;
    if out.len() as u64 != size {
        return Err(malformed("entry size differs from the header"));
    }
    Ok(())
}

/// Bytes a new streamer at an entry's LZMA2 unit must save over the running one (decoding
/// on to it) to be started: a little less than restarting is not worth what the running
/// one's helpers may have decoded ahead.
const RESTART_MARGIN: u64 = if cfg!(test) { 64 << 10 } else { 8 << 20 };

/// How many times the stretch from where the streamer started to the end of a batch read
/// ([`SevenZ::will_read`]) its LZMA2 helpers may decode past that end. Batches come one after
/// another (the gallery's thumbnails of a book's pages), and decoding is the whole cost of a
/// cold book: with 1 (as much again) the helpers ramped up over the first batches and 500
/// cold pages took ~15 % longer than decoding every unit at once; a tile's few covers still
/// reach no further than a unit or two.
const BATCH_RUN_ON: u64 = 3;

/// What [`SevenZ::aside`] found.
enum Aside {
    /// A decode elsewhere kept the entry first.
    Kept(Arc<[u8]>),
    Read(Vec<u8>),
}

/// Passes over `n` bytes of `r` a step at a time; true when `kept` has the entry first.
fn skip(r: &mut dyn Read, n: u64, kept: &dyn Fn() -> Option<Arc<[u8]>>) -> Result<bool> {
    const STEP: u64 = 1 << 20;
    let mut left = n;
    while left > 0 {
        if kept().is_some() {
            return Ok(true);
        }
        let k = left.min(STEP);
        let got =
            std::io::copy(&mut Read::take(&mut *r, k), &mut std::io::sink()).map_err(malformed)?;
        if got != k {
            return Err(malformed("solid block ended early"));
        }
        left -= k;
    }
    Ok(false)
}

/// What a reader does next about the entry it waits for.
enum Next {
    Wait,
    Restart,
    Fail(Error),
}

impl SevenZ {
    pub(crate) fn read(
        &self,
        data: &Arc<dyn ByteSource>,
        file: usize,
        e: &Entry,
        options: &Options,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let r = self.read_file(data, file, e, options, out);
        match r {
            // Garbage from a wrong key: 7z has no other way of telling.
            Err(Error::Malformed(_) | Error::Checksum | Error::Encrypted)
                if e.encrypted && options.password.is_some() =>
            {
                out.clear();
                Err(Error::WrongPassword)
            }
            r => r,
        }
    }

    fn read_file(
        &self,
        data: &Arc<dyn ByteSource>,
        file: usize,
        e: &Entry,
        options: &Options,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let limits = &options.limits;
        if e.size > limits.max_entry_size {
            return Err(Error::Limit(format!(
                "entry of {} bytes exceeds the {} byte limit",
                e.size, limits.max_entry_size
            )));
        }
        let f = self.archive.files.get(file).ok_or(Error::NoEntry(file))?;
        let Some(b) = self
            .archive
            .stream_map
            .file_block_index
            .get(file)
            .copied()
            .flatten()
        else {
            return Ok(()); // empty file
        };
        if !f.has_stream() || f.size() == 0 {
            return Ok(());
        }
        let block = self.blocks.get(b).ok_or_else(|| malformed("block index"))?;
        if let Some(why) = &block.refused {
            return Err(Error::Limit(why.clone()));
        }
        if !block.solid {
            return self.read_direct(data.bytes(), b, file, out);
        }
        let p = block
            .place(file)
            .ok_or_else(|| malformed("entry not found in its block"))?;
        *out = self.read_solid(data, block, b, p, options)?;
        Ok(())
    }

    pub(crate) fn has_solid_blocks(&self) -> bool {
        self.blocks.iter().any(|b| b.solid)
    }

    /// For a file in a solid block: the block and the file's place in it (files are decoded in
    /// index order).
    pub(crate) fn stream_position(&self, file: usize) -> Option<(u64, u64)> {
        let b = self
            .archive
            .stream_map
            .file_block_index
            .get(file)
            .copied()
            .flatten()?;
        self.blocks
            .get(b)
            .filter(|bl| bl.solid)
            .map(|_| (b as u64, file as u64))
    }

    /// A batch read is about to ask for `files` in stream order (`ahead`), or is done (not
    /// `ahead`, with the same `files`). Meanwhile the LZMA2 helpers of their solid blocks
    /// decode past the last of them by three times as much as lies between it and where the
    /// streamer started ([`BATCH_RUN_ON`]), beyond the read-ahead: the gallery's thumbnails of an archive's
    /// pages come in batches one after another, read by one thread that also makes
    /// thumbnails between its reads, and the helpers would otherwise idle while it works
    /// and wait at the start of each batch; a tile's few covers still decode no further
    /// than a unit or two.
    pub(crate) fn will_read(&self, files: &[usize], ahead: bool) {
        let mut ends: HashMap<usize, u64> = HashMap::new();
        for &file in files {
            let Some(b) = self.stream_position(file).map(|(b, _)| b as usize) else {
                continue;
            };
            let block = &self.blocks[b];
            if block.plan.is_none() {
                continue;
            }
            if let Some(p) = block.place(file) {
                let end = ends.entry(b).or_default();
                *end = (*end).max(block.spans[p].end());
            }
        }
        for (b, end) in ends {
            let shared = &self.blocks[b].shared;
            let mut st = shared.state.lock();
            if ahead {
                st.batches += 1;
                st.wanted = st.wanted.max(end);
            } else {
                st.batches = st.batches.saturating_sub(1);
                if st.batches == 0 {
                    st.wanted = 0;
                }
            }
            shared.cv.notify_all();
        }
    }

    /// Bytes decoded before `file` when its block's streamer starts for it: from the start
    /// of its LZMA2 unit, or of its block. `None` when it is not in a solid block.
    pub(crate) fn reach_cost(&self, file: usize) -> Option<u64> {
        let b = self
            .archive
            .stream_map
            .file_block_index
            .get(file)
            .copied()??;
        let block = self.blocks.get(b).filter(|bl| bl.solid)?;
        let p = block.place(file)?;
        let (_, _, from) = block.start_for(p);
        Some(block.spans[p].start - from)
    }

    /// `file`'s solid block and its place there; `None` when it is not in a solid block.
    fn solid_place(&self, file: usize) -> Option<(usize, &Block, usize)> {
        let b = self
            .archive
            .stream_map
            .file_block_index
            .get(file)
            .copied()??;
        let block = self.blocks.get(b).filter(|bl| bl.solid)?;
        Some((b, block, block.place(file)?))
    }

    /// Keeps `file` (a solid block's `ComicInfo.xml`, small) whenever a decode passes over
    /// it: a streamer reading on, one of its LZMA2 helpers decoding the unit holding it
    /// ahead of the streamer, or [`SevenZ::read_aside`]. False when it is not in a solid
    /// block.
    pub(crate) fn capture(&self, file: usize) -> bool {
        let Some((_, block, p)) = self.solid_place(file) else {
            return false;
        };
        block.shared.state.lock().keep = Some(p);
        true
    }

    /// The bytes of `file` ([`SevenZ::capture`]) once a decode has passed over it.
    pub(crate) fn kept(&self, file: usize) -> Option<Arc<[u8]>> {
        let (_, block, p) = self.solid_place(file)?;
        block.shared.state.lock().kept(p)
    }

    /// Runs `hook` with the bytes of `file` ([`SevenZ::capture`]) once a decode has passed
    /// over it (now, if one has), on the thread that decoded it. False (and `hook` is
    /// dropped) when it is not kept.
    pub(crate) fn on_kept(&self, file: usize, hook: Hook) -> bool {
        let Some((_, block, p)) = self.solid_place(file) else {
            return false;
        };
        let mut st = block.shared.state.lock();
        if st.keep != Some(p) {
            return false;
        }
        match st.kept(p) {
            Some(b) => {
                drop(st);
                hook(b);
            }
            None => st.hooks.push(hook),
        }
        true
    }

    /// `file` of a solid block read without its streamer, so readers elsewhere in the
    /// block keep theirs (`None` when it is not in a solid block): decoded on the calling
    /// thread from the start of its LZMA2 unit (or of its block), the entries before it
    /// passed over without being kept. Returns early with the bytes a streamer or its helpers
    /// decoded meanwhile when `file` is kept in passing ([`SevenZ::capture`]), and waits for
    /// a helper already decoding its unit instead of decoding that again.
    pub(crate) fn read_aside(
        &self,
        data: &Arc<dyn ByteSource>,
        file: usize,
        options: &Options,
    ) -> Option<Result<Vec<u8>>> {
        let (b, block, p) = self.solid_place(file)?;
        Some(self.aside(data, b, block, p, options))
    }

    fn aside(
        &self,
        data: &Arc<dyn ByteSource>,
        b: usize,
        block: &Block,
        p: usize,
        options: &Options,
    ) -> Result<Vec<u8>> {
        if let Some(why) = &block.refused {
            return Err(Error::Limit(why.clone()));
        }
        let span = block.spans[p];
        if span.size > options.limits.max_entry_size {
            return Err(Error::Limit(format!(
                "entry of {} bytes exceeds the {} byte limit",
                span.size, options.limits.max_entry_size
            )));
        }
        let shared = &block.shared;
        let kept = || shared.state.lock().kept(p);
        let v = match &block.plan {
            Some(plan) => self.aside_units(data, block, plan, p, &kept)?,
            None => self.aside_block(data, b, p, &kept)?,
        };
        let v = match v {
            Aside::Kept(b) => return Ok(b.to_vec()),
            Aside::Read(v) => v,
        };
        if options.verify_crc
            && let Some(crc) = span.crc
            && crc32fast::hash(&v) != crc
        {
            return Err(Error::Checksum);
        }
        shared.keep(p, &v);
        Ok(v)
    }

    /// [`SevenZ::aside`] through [`lzma2mt`].
    fn aside_units(
        &self,
        data: &Arc<dyn ByteSource>,
        block: &Block,
        plan: &Arc<Plan>,
        p: usize,
        kept: &dyn Fn() -> Option<Arc<[u8]>>,
    ) -> Result<Aside> {
        let span = block.spans[p];
        let shared = &block.shared;
        let (u, from) = if plan.seekable() {
            plan.unit_of(span.start)
        } else {
            (0, 0)
        };
        // A helper decoding that unit shows it to the streamer's tap when done.
        if plan.filter.is_none() {
            let mut st = shared.state.lock();
            loop {
                if let Some(b) = st.kept(p) {
                    return Ok(Aside::Kept(b));
                }
                let busy = (st.stream.as_ref())
                    .and_then(|c| c.helpers.as_ref())
                    .is_some_and(|h| h.working(u));
                if !busy {
                    break;
                }
                shared
                    .cv
                    .wait_for(&mut st, std::time::Duration::from_millis(20));
            }
        }
        // Read from the first unit on, a filtered stream gets the plan's helpers too (with
        // the calling thread's priority where threads inherit it).
        let helpers = if plan.seekable() { 0 } else { plan.threads - 1 };
        let r = UnitReader::new(data, plan, u, helpers, None, None).map_err(Error::Io)?;
        r.limiter().set(span.end());
        let mut r: Box<dyn Read> = match plan.filter {
            Some(f) => f.wrap(r),
            None => Box::new(r),
        };
        if skip(&mut r, span.start - from, kept)? {
            return kept().map(Aside::Kept).ok_or_else(|| malformed("kept"));
        }
        let mut v = vec![0u8; span.size as usize];
        r.read_exact(&mut v).map_err(malformed)?;
        Ok(Aside::Read(v))
    }

    /// [`SevenZ::aside`] through `BlockDecoder`, from the start of block `b`.
    fn aside_block(
        &self,
        data: &Arc<dyn ByteSource>,
        b: usize,
        p: usize,
        kept: &dyn Fn() -> Option<Arc<[u8]>>,
    ) -> Result<Aside> {
        let archive = &*self.archive;
        let _budget = crate::reserve(decoder_memory(&archive.blocks[b]))?;
        let target = &archive.files[self.blocks[b].spans[p].file];
        let mut src = Cursor::new(data.bytes());
        let pw = sz_password(self.password.as_ref());
        let mut result = Err(malformed("entry not found in its block"));
        BlockDecoder::new(1, b, archive, &pw, &mut src)
            .for_each_entries(&mut |entry, r| {
                if std::ptr::eq(entry, target) {
                    let mut v = Vec::new();
                    result = read_entry(r, entry.size(), &mut v).map(|()| Aside::Read(v));
                    return Ok(false);
                }
                match skip(r, entry.size(), kept) {
                    Ok(false) => Ok(true),
                    Ok(true) => {
                        result = kept().ok_or_else(|| malformed("kept")).map(Aside::Kept);
                        Ok(false)
                    }
                    Err(e) => {
                        result = Err(e);
                        Ok(false)
                    }
                }
            })
            .map_err(map_err)?;
        result
    }

    /// Threads block `b`'s stream is decoded with (1 without a plan).
    #[cfg(test)]
    fn threads(&self, b: usize) -> usize {
        self.blocks[b].plan.as_ref().map_or(1, |p| p.threads)
    }

    /// File `file`, alone in block `b` of LZMA2 (and at most one BCJ or Delta filter), decoded
    /// here with a dictionary no larger than the file: sevenz-rust2 allocates the dictionary
    /// the header declares (up to `max_decoder_memory`) for a file however small. `None`
    /// sends the block to `BlockDecoder`.
    fn read_lzma2(
        &self,
        bytes: &[u8],
        b: usize,
        file: usize,
        out: &mut Vec<u8>,
    ) -> Option<Result<()>> {
        let a = &*self.archive;
        let blk = a.blocks.get(b)?;
        let (lz, lzma2, filter) = lzma2_chain(blk)?;
        let f = a.files.get(file)?;
        let size = blk.get_unpack_size();
        if size != f.size() || size != blk.get_unpack_size_at_index(lz) {
            return None;
        }
        let packed = bytes.get(pack_range(a, b)?)?;
        let declared = coder_memory(lzma2.encoder_method_id(), lzma2.properties())?;
        // Nothing in the stream refers back past its own start.
        let dict = u32::try_from(declared.min(size).max(4096)).ok()?;
        let size = usize::try_from(size).ok()?;
        let crc = if f.has_crc {
            Some(f.crc as u32)
        } else {
            blk.has_crc.then_some(blk.crc as u32)
        };
        Some((|| {
            let _budget = crate::reserve(u64::from(dict))?;
            let r = lzma_rust2::Lzma2Reader::new(packed, dict, None);
            let r: Box<dyn Read + '_> = match filter {
                Some(f) => f.wrap(r),
                None => Box::new(r),
            };
            let start = out.len();
            crate::codec::lzma::read_exact_into(r, size, out)
                .map_err(|e| e.into_error("7z LZMA2", size, true))?;
            if crc.is_some_and(|c| crc32fast::hash(&out[start..]) != c) {
                out.truncate(start);
                return Err(Error::Checksum);
            }
            Ok(())
        })())
    }

    fn read_direct(&self, bytes: &[u8], b: usize, file: usize, out: &mut Vec<u8>) -> Result<()> {
        if let Some(r) = self.read_lzma2(bytes, b, file, out) {
            return r;
        }
        let archive = &*self.archive;
        let _budget = crate::reserve(decoder_memory(&archive.blocks[b]))?;
        let target = &archive.files[file];
        let mut src = Cursor::new(bytes);
        let mut result = Err(malformed("entry not found in its block"));
        let pw = sz_password(self.password.as_ref());
        BlockDecoder::new(1, b, archive, &pw, &mut src)
            .for_each_entries(&mut |entry, r| {
                if std::ptr::eq(entry, target) {
                    result = read_entry(r, entry.size(), out);
                    Ok(false)
                } else {
                    Ok(true)
                }
            })
            .map_err(map_err)?;
        result
    }

    /// Entry `p` of solid `block`: from the cache, or once the streamer has delivered it.
    fn read_solid(
        &self,
        data: &Arc<dyn ByteSource>,
        block: &Block,
        b: usize,
        p: usize,
        options: &Options,
    ) -> Result<Vec<u8>> {
        let shared = &block.shared;
        let mut st = shared.state.lock();
        *st.waiting.entry(p).or_default() += 1;
        let res = loop {
            if let Some(v) = st.get(p) {
                // Read ahead from here on too: a reader served by the read-ahead would
                // otherwise move the streamer on only once it caught up with it.
                let end = block.spans[p].end();
                if end > st.horizon {
                    st.horizon = end;
                    shared.cv.notify_all();
                }
                break Ok(v);
            }
            if let Some(m) = st.limited.get(&p) {
                break Err(Error::Limit(m.clone()));
            }
            let next = match &st.stream {
                None => Next::Restart,
                Some(c) if c.next <= p && c.failed.is_some() => {
                    Next::Fail(c.failed.as_ref().map_or_else(|| malformed("stream"), dup))
                }
                Some(c) if c.next <= p && c.ended => {
                    Next::Fail(malformed("solid block ended early"))
                }
                // Passed over and evicted (or never kept): start again, unless another
                // reader waits for an entry the streamer still has ahead of it.
                Some(c) if c.next > p => {
                    if st.waited(c.next..usize::MAX) {
                        Next::Wait
                    } else {
                        Next::Restart
                    }
                }
                // In a later unit of a seekable block: start at that unit when it decodes
                // less (by a margin), unless another reader waits for an entry before it or
                // the helpers have that unit decoded or under way already (reading on to it
                // then costs only copies; a new streamer would decode it again).
                Some(c) => {
                    let (u, first, from) = block.start_for(p);
                    let target = block.spans[p].start;
                    let from_unit = target - from;
                    if u > c.unit
                        && target.saturating_sub(c.at) > from_unit.saturating_add(RESTART_MARGIN)
                        && !st.waited(c.next..first)
                        && !c.helpers.as_ref().is_some_and(|h| h.taken(u))
                    {
                        Next::Restart
                    } else {
                        Next::Wait
                    }
                }
            };
            match next {
                Next::Wait => {
                    let end = block.spans[p].end();
                    if end > st.horizon {
                        st.horizon = end;
                        shared.cv.notify_all();
                    }
                    shared.cv.wait(&mut st);
                }
                Next::Fail(e) => {
                    st.stream = None;
                    break Err(e);
                }
                Next::Restart => {
                    let (unit, first, _) = block.start_for(p);
                    st.epoch += 1;
                    let epoch = st.epoch;
                    st.horizon = block.spans[p].end();
                    st.stream = Some(Ctl {
                        epoch,
                        next: first,
                        unit,
                        at: block.spans.get(first).map_or(0, |s| s.start),
                        helpers: None,
                        failed: None,
                        ended: false,
                    });
                    // An earlier streamer (paused, maybe) sees it is replaced and stops.
                    shared.cv.notify_all();
                    let streamer = Streamer {
                        shared: Arc::clone(shared),
                        spans: Arc::clone(&block.spans),
                        plan: block.plan.clone(),
                        epoch,
                        budget: options.solid_cache_bytes,
                        ahead: options.solid_read_ahead.min(options.solid_cache_bytes / 2) as u64,
                        origin: block.spans.get(first).map_or(0, |s| s.start),
                        max_entry: options.limits.max_entry_size,
                        verify_crc: options.verify_crc,
                        password: options.password.clone(),
                    };
                    if let Err(e) = self.spawn(data, b, unit, first, streamer, options.thread_start)
                    {
                        st.stream = None;
                        break Err(e);
                    }
                }
            }
        };
        if let Some(n) = st.waiting.get_mut(&p) {
            *n -= 1;
            if *n == 0 {
                st.waiting.remove(&p);
            }
        }
        st.trim(options.solid_cache_bytes);
        res
    }

    fn spawn(
        &self,
        data: &Arc<dyn ByteSource>,
        b: usize,
        unit: usize,
        first: usize,
        s: Streamer,
        thread_start: Option<fn()>,
    ) -> Result<()> {
        let archive = Arc::clone(&self.archive);
        let data = Arc::clone(data);
        specular_thread::builder("arc-7z").spawn(move || {
            if let Some(f) = thread_start {
                f();
            }
            match s.plan.clone() {
                Some(plan) => s.run_units(&data, &plan, unit, first, thread_start),
                None => s.run_block(&data, &archive, b),
            }
        })?;
        Ok(())
    }
}

/// A solid block's streamer thread: decodes entries in order into the block's cache.
struct Streamer {
    shared: Arc<Solid>,
    spans: Arc<[Span]>,
    plan: Option<Arc<Plan>>,
    epoch: u64,
    budget: usize,
    ahead: u64,
    /// Where it started (the first entry's offset).
    origin: u64,
    max_entry: u64,
    verify_crc: bool,
    password: Option<crate::Password>,
}

impl Streamer {
    /// Waits until entry `p` is within the read-ahead; false when this streamer is no longer
    /// wanted. Meanwhile keeps `limiter` (its helpers) past the furthest entry asked for by
    /// the read-ahead or by as far as the readers have come since it started, whichever is
    /// more (the window of a kernel's file read-ahead: one page read costs a unit or two, a
    /// book read through keeps every helper busy, within
    /// [`MT_MEMORY`](crate::lzma2mt::MT_MEMORY)); and past
    /// the furthest entry batch reads will ask for by [`BATCH_RUN_ON`] times as far
    /// ([`SevenZ::will_read`]).
    fn before(&self, p: usize, limiter: Option<&Limiter>) -> bool {
        let mut st = self.shared.state.lock();
        loop {
            let (horizon, wanted) = (st.horizon, st.wanted);
            let Some(c) = st.stream.as_mut().filter(|c| c.epoch == self.epoch) else {
                return false;
            };
            if let Some(l) = limiter {
                let grow = |at: u64| at.saturating_add(at.saturating_sub(self.origin));
                l.set(
                    horizon.saturating_add(self.ahead).max(grow(horizon)).max(
                        wanted.saturating_add(
                            wanted
                                .saturating_sub(self.origin)
                                .saturating_mul(BATCH_RUN_ON),
                        ),
                    ),
                );
            }
            let start = self.spans[p].start;
            if start < horizon.saturating_add(self.ahead) || horizon == 0 && p == c.next {
                if let Some(plan) = &self.plan {
                    c.unit = c.unit.max(plan.unit_of(start).0);
                }
                c.at = start;
                return true;
            }
            self.shared.cv.wait(&mut st);
        }
    }

    /// Hands entry `p` (or why it could not be read) to the readers; false to stop.
    fn deliver(&self, p: usize, item: Result<Vec<u8>>) -> bool {
        let mut st = self.shared.state.lock();
        if st.stream.as_ref().is_none_or(|c| c.epoch != self.epoch) {
            return false;
        }
        let mut kept = None;
        let go = match item {
            Ok(v) => {
                kept = st.keep(p, &v);
                st.insert(p, v, self.budget);
                true
            }
            // Too large to keep: decoded through to reach later entries.
            Err(Error::Limit(m)) => {
                st.limited.insert(p, m);
                true
            }
            // A corrupt entry breaks the solid stream for everything after it.
            Err(e) => {
                if let Some(c) = st.stream.as_mut() {
                    c.failed = Some(e);
                }
                false
            }
        };
        if let Some(c) = st.stream.as_mut()
            && go
        {
            c.next = p + 1;
        }
        self.shared.cv.notify_all();
        drop(st);
        if let Some((b, hooks)) = kept {
            run_hooks(&b, hooks);
        }
        go
    }

    fn finish(&self, err: Option<Error>) {
        let mut st = self.shared.state.lock();
        if let Some(c) = st.stream.as_mut().filter(|c| c.epoch == self.epoch) {
            match err {
                Some(e) if c.failed.is_none() => c.failed = Some(e),
                _ => c.ended = true,
            }
        }
        self.shared.cv.notify_all();
    }

    /// Entry `p` read from `r` (its whole span), or why not.
    fn entry(&self, r: &mut dyn Read, p: usize) -> Result<Vec<u8>> {
        let s = self.spans[p];
        if s.size > self.max_entry {
            std::io::copy(&mut r.take(s.size), &mut std::io::sink()).map_err(malformed)?;
            return Err(Error::Limit(format!(
                "entry of {} bytes exceeds the {} byte limit",
                s.size, self.max_entry
            )));
        }
        let mut v = vec![0u8; s.size as usize];
        r.read_exact(&mut v).map_err(malformed)?;
        if self.verify_crc
            && let Some(crc) = s.crc
            && crc32fast::hash(&v) != crc
        {
            return Err(Error::Checksum);
        }
        Ok(v)
    }

    /// What keeps the block's entry to keep ([`SevenZ::capture`]) from a unit a helper
    /// decoded, when it lies wholly in it: units decode on their own, so it is there before
    /// the streamer reaches it. Not with a filter, which runs on the streamer only.
    fn tap(&self, plan: &Plan) -> Option<crate::lzma2mt::Tap> {
        if plan.filter.is_some() {
            return None;
        }
        let p = self.shared.state.lock().keep?;
        let span = self.spans[p];
        let (shared, verify) = (Arc::clone(&self.shared), self.verify_crc);
        Some(Box::new(
            move |unit: &crate::lzma2mt::Unit, bytes: &[u8]| {
                if span.start < unit.start || span.end() > unit.start + unit.len {
                    return;
                }
                let at = (span.start - unit.start) as usize;
                let Some(v) = bytes.get(at..at + span.size as usize) else {
                    return;
                };
                if verify && span.crc.is_some_and(|c| crc32fast::hash(v) != c) {
                    return;
                }
                shared.keep(p, v);
            },
        ))
    }

    /// Through [`lzma2mt`], from unit `unit` (whose first entry is `first`).
    fn run_units(
        &self,
        data: &Arc<dyn ByteSource>,
        plan: &Arc<Plan>,
        unit: usize,
        first: usize,
        thread_start: Option<fn()>,
    ) {
        let r = UnitReader::new(
            data,
            plan,
            unit,
            plan.threads - 1,
            thread_start,
            self.tap(plan),
        );
        let r = match r {
            Ok(r) => r,
            Err(e) => return self.finish(Some(Error::Io(e))),
        };
        let limiter = r.limiter();
        if let Some(c) =
            (self.shared.state.lock().stream.as_mut()).filter(|c| c.epoch == self.epoch)
        {
            c.helpers = Some(limiter.clone());
        }
        let mut r: Box<dyn Read> = match plan.filter {
            Some(f) => f.wrap(r),
            None => Box::new(r),
        };
        // The tail of an entry that began in the unit before.
        let base = match plan.unit(unit) {
            Ok(u) => u.map_or(plan.total, |u| u.start),
            Err(e) => return self.finish(Some(malformed(e))),
        };
        let skip = self.spans.get(first).map_or(0, |s| s.start - base);
        for p in first..self.spans.len() {
            if !self.before(p, Some(&limiter)) {
                return;
            }
            // (Skipped once the helpers have their limit.)
            if p == first
                && skip > 0
                && let Err(e) = std::io::copy(&mut (&mut r).take(skip), &mut std::io::sink())
            {
                return self.finish(Some(malformed(e)));
            }
            let item = self.entry(&mut r, p);
            if !self.deliver(p, item) {
                return;
            }
        }
        self.finish(None);
    }

    /// Through `BlockDecoder`, from the start of block `b`.
    fn run_block(&self, data: &Arc<dyn ByteSource>, archive: &SzArchive, b: usize) {
        let _budget = match crate::reserve(decoder_memory(&archive.blocks[b])) {
            Ok(r) => r,
            Err(e) => return self.finish(Some(e)),
        };
        let mut src = Cursor::new(data.bytes());
        let pw = sz_password(self.password.as_ref());
        let files = &archive.files;
        let res =
            BlockDecoder::new(1, b, archive, &pw, &mut src).for_each_entries(&mut |entry, r| {
                let Some(p) = file_index(files, entry)
                    .and_then(|i| self.spans.binary_search_by_key(&i, |s| s.file).ok())
                else {
                    // An entry without a stream: nothing to hand over.
                    return Ok(true);
                };
                if !self.before(p, None) {
                    return Ok(false);
                }
                let item = if entry.size() > self.max_entry {
                    self.entry(r, p)
                } else {
                    let mut v = Vec::new();
                    read_entry(r, entry.size(), &mut v).map(|()| v)
                };
                Ok(self.deliver(p, item))
            });
        self.finish(res.err().map(map_err));
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;
    use crate::Archive;

    /// Pseudo-random bytes with some repetition (LZMA finds matches, not too many).
    fn pages(n: usize, len: usize) -> Vec<Vec<u8>> {
        (0..n)
            .map(|i| {
                let mut x = (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
                (0..len)
                    .map(|k| {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        if k % 4 == 0 { x as u8 } else { (k / 64) as u8 }
                    })
                    .collect()
            })
            .collect()
    }

    /// A solid 7z of `data`, LZMA2 with independent units of `unit` bytes when `threads` > 1,
    /// after `filter` when given.
    fn solid_7z_with(
        data: &[Vec<u8>],
        threads: u32,
        unit: u64,
        filter: Option<sevenz_rust2::EncoderMethod>,
    ) -> Vec<u8> {
        use sevenz_rust2::encoder_options::Lzma2Options;
        use sevenz_rust2::{ArchiveWriter, EncoderConfiguration, SourceReader};
        let mut w = ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
        let mut o = if threads > 1 {
            Lzma2Options::from_level_mt(1, threads, unit)
        } else {
            Lzma2Options::from_level(1)
        };
        o.set_dictionary_size(1 << 16);
        // The writer lists methods in decode order: LZMA2 first, the filter after it.
        let mut methods: Vec<EncoderConfiguration> = vec![o.into()];
        if let Some(f) = filter {
            methods.push(f.into());
        }
        w.set_content_methods(methods);
        let entries = (0..data.len())
            .map(|i| ArchiveEntry::new_file(&format!("{i:03}.jpg")))
            .collect();
        let readers = data
            .iter()
            .map(|d| SourceReader::new(Cursor::new(d.clone())))
            .collect();
        w.push_archive_entries(entries, readers).unwrap();
        w.finish().unwrap().into_inner()
    }

    /// A 7z of 61 bytes whose packed header's LZMA2 coder declares a 4 GiB dictionary:
    /// sevenz-rust2 allocated (and zeroed) it decoding the header, before any of our limits.
    /// Refused from the raw header now; a written archive's header coder still passes.
    #[test]
    fn packed_header_with_a_huge_dictionary() {
        let mut f = b"7z\xbc\xaf\x27\x1c\x00\x04".to_vec();
        f.extend([0; 4]);
        f.extend(0u64.to_le_bytes());
        let header = [
            0x17, 0x06, 0x00, 0x01, 0x09, 0x10, 0x00, 0x07, 0x0b, 0x01, 0x00, 0x01, 0x21, 0x21,
            0x01, 40, 0x0c, 0x10, 0x00,
        ];
        f.extend((header.len() as u64).to_le_bytes());
        f.extend([0; 4]);
        f.extend(header);
        assert_eq!(header_coder_memory(&f), Some(u64::from(u32::MAX)));
        let r = Archive::from_bytes(f, opts(1));
        assert!(matches!(r, Err(Error::Limit(_))), "{:?}", r.err());
        let ok = solid_7z(&pages(2, 1000), 1, 1 << 20);
        assert!(header_coder_memory(&ok).is_none_or(|m| m <= MAX_HEADER_DECODER_MEMORY));
        assert!(Archive::from_bytes(ok, opts(1)).is_ok());
    }

    /// A one-file 7z whose LZMA2 coder declares a 1 GiB dictionary for 3000 bytes (the
    /// largest `max_decoder_memory` lets through): sevenz-rust2 allocated all of it. The file
    /// is now decoded with a dictionary of its own size, so it reads within a 16 MiB budget.
    #[test]
    fn small_file_with_a_huge_dictionary() {
        let data = pages(1, 3000);
        let mut f = solid_7z(&data, 1, 1 << 20);
        // The raw (unencoded) header's LZMA2 coder: flags 0x21, id 0x21, one property byte.
        let off = u64::from_le_bytes(f[12..20].try_into().unwrap()) as usize;
        let len = u64::from_le_bytes(f[20..28].try_into().unwrap()) as usize;
        let header = 32 + off..32 + off + len;
        let at = f[header.clone()]
            .windows(3)
            .position(|w| w == [0x21, 0x21, 0x01])
            .expect("raw header with an LZMA2 coder")
            + header.start
            + 3;
        f[at] = 36; // 1 GiB
        let crc = crc32fast::hash(&f[header]);
        f[28..32].copy_from_slice(&crc.to_le_bytes());
        let crc = crc32fast::hash(&f[12..32]);
        f[8..12].copy_from_slice(&crc.to_le_bytes());
        let a = Archive::from_bytes(f, opts(1)).unwrap();
        assert!(!sz(&a).blocks[0].solid);
        let got = specular_core::budget::with_limit(16 << 20, || a.read(0));
        assert_eq!(got.unwrap(), data[0]);
    }

    fn solid_7z(data: &[Vec<u8>], threads: u32, unit: u64) -> Vec<u8> {
        solid_7z_with(data, threads, unit, None)
    }

    fn sz(a: &Archive) -> &SevenZ {
        match &a.backend {
            crate::Backend::SevenZ(s) => s,
            _ => unreachable!(),
        }
    }

    fn opts(threads: u32) -> Options {
        Options {
            decode_threads: threads,
            ..Options::default()
        }
    }

    /// Every entry of `a` reads back as `data[its number]`: all at once, then in an order
    /// that jumps forwards and back.
    fn check_all(a: &Archive, data: &[Vec<u8>], what: &str) {
        let all: Vec<usize> = (0..a.len()).collect();
        let mut got = vec![None; a.len()];
        a.read_many(&all, &mut |k, r| {
            got[k] = Some(r.unwrap());
            true
        });
        let want = |i: usize| &data[a.entries()[i].name[..3].parse::<usize>().unwrap()];
        for (i, g) in got.into_iter().enumerate() {
            assert_eq!(g.as_ref(), Some(want(i)), "{}, {what}", a.entries()[i].name);
        }
        let n = a.len();
        for i in [3, n - 1, n / 2, 1, n / 2 + 1, n - 2, 0] {
            assert_eq!(&a.read(i).unwrap(), want(i), "entry {i}, {what}");
        }
    }

    #[test]
    fn independent_lzma2_units_decode_in_parallel() {
        // Level 1 keeps its 1 MiB dictionary as the least unit size: about 5 units.
        let data = pages(24, 200_000);
        for (enc, parallel) in [(4u32, true), (1, false)] {
            let bytes = solid_7z(&data, enc, 1 << 16);
            let a = Archive::from_bytes(bytes.clone(), opts(4)).unwrap();
            let threads = sz(&a).threads(0);
            assert_eq!(threads > 1, parallel, "encoder threads {enc}: {threads}");
            // One thread or several, every entry reads back, in order and out of it.
            let one = Archive::from_bytes(bytes.clone(), opts(1)).unwrap();
            assert_eq!(sz(&one).threads(0), 1);
            for (t, a) in [(1, one), (4, a)] {
                check_all(&a, &data, &format!("{t} threads, encoder {enc}"));
            }
        }
    }

    #[test]
    fn a_batch_read_lets_the_helpers_reach_its_last_entry() {
        let data = pages(24, 200_000);
        let bytes = solid_7z(&data, 4, 1 << 16);
        let o = Options {
            solid_read_ahead: 100_000,
            ..opts(4)
        };
        let a = Archive::from_bytes(bytes, o).unwrap();
        let s = sz(&a);
        let want: Vec<usize> = (0..a.len()).step_by(3).collect();
        let last_end = s.blocks[0].spans[*want.last().unwrap()].end();
        let mut got = 0;
        a.read_many(&want, &mut |k, r| {
            assert_eq!(r.unwrap(), data[want[k]]);
            assert_eq!(s.blocks[0].shared.state.lock().wanted, last_end);
            got += 1;
            true
        });
        assert_eq!(got, want.len());
        // Done: back to the read-ahead. Stopped early: the same.
        assert_eq!(s.blocks[0].shared.state.lock().wanted, 0);
        a.read_many(&want, &mut |_, _| false);
        assert_eq!(s.blocks[0].shared.state.lock().wanted, 0);
        // Overlapping batches: the furthest holds until the last one ends.
        s.will_read(&[file_of(&a, 2)], true);
        s.will_read(&[file_of(&a, 20)], true);
        let end = |i: usize| s.blocks[0].spans[i].end();
        assert_eq!(s.blocks[0].shared.state.lock().wanted, end(20));
        s.will_read(&[file_of(&a, 20)], false);
        assert_eq!(s.blocks[0].shared.state.lock().wanted, end(20));
        s.will_read(&[file_of(&a, 2)], false);
        assert_eq!(s.blocks[0].shared.state.lock().wanted, 0);
    }

    fn file_of(a: &Archive, i: usize) -> usize {
        let Loc::SevenZ { file } = a.entries()[i].loc else {
            unreachable!()
        };
        file
    }

    #[test]
    fn read_entries_are_evicted_before_the_read_ahead() {
        let mut st = SolidState::default();
        for p in 0..3 {
            st.insert(p, vec![0; 10], 30);
        }
        st.get(0);
        st.get(1);
        // Read entries go first, the least recently read first...
        st.insert(3, vec![0; 10], 30);
        assert!(!st.cache.contains_key(&0) && st.cache.contains_key(&2));
        st.insert(4, vec![0; 10], 30);
        assert!(!st.cache.contains_key(&1) && st.cache.contains_key(&2));
        // ...then the earliest decoded, unless a reader waits for it.
        st.waiting.insert(2, 1);
        st.insert(5, vec![0; 10], 30);
        assert!(st.cache.contains_key(&2) && !st.cache.contains_key(&3));
        assert_eq!(st.cached, 30);
    }

    #[test]
    fn far_entries_start_at_their_unit() {
        let data = pages(24, 200_000);
        let bytes = solid_7z(&data, 4, 1 << 16);
        let a = Archive::from_bytes(bytes, opts(1)).unwrap();
        let s = sz(&a);
        let plan = s.blocks[0].plan.clone().expect("planned");
        assert!(plan.units().len() >= 3, "{:?}", plan.units());
        // The last entry is reached from the start of the last unit, not of the block.
        let last = a.len() - 1;
        let Loc::SevenZ { file } = a.entries()[last].loc else {
            unreachable!()
        };
        let cost = s.reach_cost(file).unwrap();
        assert!(cost < plan.units().last().unwrap().len, "{cost}");
        assert_eq!(a.read(last).unwrap(), data[last]);
        {
            let st = s.blocks[0].shared.state.lock();
            let c = st.stream.as_ref().unwrap();
            assert_eq!(c.unit, plan.units().len() - 1);
            // Nothing before that unit was decoded.
            assert!(st.cache.keys().all(|&p| p >= s.blocks[0].start_for(last).1));
        }
        // Back to the start: a new streamer at unit 0.
        assert_eq!(a.read(0).unwrap(), data[0]);
        assert_eq!(
            s.blocks[0]
                .shared
                .state
                .lock()
                .stream
                .as_ref()
                .unwrap()
                .unit,
            0
        );
        // From there, an entry two units on starts a streamer at its unit too (the running
        // one, held back by its read-ahead, would decode the unit between).
        let o = Options {
            solid_read_ahead: 100_000,
            ..opts(1)
        };
        let a = Archive::from_bytes(a.data.bytes().to_vec(), o).unwrap();
        let s = sz(&a);
        assert_eq!(a.read(0).unwrap(), data[0]);
        let far = s.blocks[0].start_for(last).1;
        assert_eq!(a.read(far).unwrap(), data[far]);
        let st = s.blocks[0].shared.state.lock();
        assert_eq!(st.stream.as_ref().unwrap().unit, plan.units().len() - 1);
        assert!(!st.cache.contains_key(&(far / 2)), "{:?}", st.cache.keys());
    }

    #[test]
    fn a_unit_the_helpers_have_is_read_on_to_not_started_again() {
        let data = pages(24, 200_000);
        let o = Options {
            solid_read_ahead: 100_000,
            ..opts(4)
        };
        let a = Archive::from_bytes(solid_7z(&data, 4, 1 << 16), o).unwrap();
        let s = sz(&a);
        let last = a.len() - 1;
        let (far_unit, far, _) = s.blocks[0].start_for(last);
        // A batch up to the last entry lets the helpers decode every unit.
        s.will_read(&[file_of(&a, last)], true);
        assert_eq!(a.read(0).unwrap(), data[0]);
        let shared = &s.blocks[0].shared;
        let epoch = shared.state.lock().epoch;
        let t = std::time::Instant::now();
        while !(shared.state.lock().stream.as_ref())
            .and_then(|c| c.helpers.as_ref())
            .is_some_and(|h| h.taken(far_unit))
        {
            assert!(
                t.elapsed().as_secs() < 30,
                "the helpers never took unit {far_unit}"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(a.read(far).unwrap(), data[far]);
        assert_eq!(shared.state.lock().epoch, epoch, "started again");
        s.will_read(&[file_of(&a, last)], false);
    }

    #[test]
    fn filtered_lzma2_decodes_across_unit_boundaries() {
        // BCJ x86 changes E8/E9 call targets by position: make sure they occur, and that
        // the units' seams fall inside such data.
        let mut data = pages(16, 150_000);
        for (i, d) in data.iter_mut().enumerate() {
            for k in (i * 7..d.len().saturating_sub(5)).step_by(97) {
                d[k] = 0xE8;
                d[k + 4] = if k % 2 == 0 { 0x00 } else { 0xFF };
            }
        }
        for (filter, name) in [
            (sevenz_rust2::EncoderMethod::BCJ_X86_FILTER, "BCJ x86"),
            (sevenz_rust2::EncoderMethod::BCJ_ARM_FILTER, "BCJ ARM"),
        ] {
            let bytes = solid_7z_with(&data, 4, 1 << 16, Some(filter));
            for t in [1, 4] {
                let a = Archive::from_bytes(bytes.clone(), opts(t)).unwrap();
                let plan = sz(&a).blocks[0].plan.clone().expect("planned");
                assert!(plan.filter.is_some() && plan.units().len() >= 3);
                assert!(!plan.seekable());
                check_all(&a, &data, &format!("{name}, {t} threads"));
            }
        }
    }

    #[test]
    fn read_ahead_stops_and_resumes() {
        let data = pages(24, 200_000);
        let bytes = solid_7z(&data, 4, 1 << 16);
        let o = Options {
            decode_threads: 4,
            solid_read_ahead: 450_000,
            ..Options::default()
        };
        let a = Archive::from_bytes(bytes, o).unwrap();
        let s = sz(&a);
        assert_eq!(a.read(0).unwrap(), data[0]);
        // The streamer pauses before the first entry starting 450 kB past the end of entry 0:
        // entries 1 to 3 (starting at 200, 400 and 600 kB) are decoded ahead, 4 is not.
        let t = std::time::Instant::now();
        loop {
            let n = s.blocks[0].shared.state.lock().cache.len();
            if n >= 4 || t.elapsed().as_secs() > 10 {
                break;
            }
            std::thread::yield_now();
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        {
            let st = s.blocks[0].shared.state.lock();
            let mut have: Vec<usize> = st.cache.keys().copied().collect();
            have.sort();
            assert_eq!(have, [0, 1, 2, 3]);
        }
        // Reading on moves it along; a cached entry reads again.
        for (i, d) in data.iter().enumerate().take(10).skip(1) {
            assert_eq!(a.read(i).unwrap(), *d);
        }
        assert_eq!(a.read(9).unwrap(), data[9]);
        assert!(
            s.blocks[0]
                .shared
                .state
                .lock()
                .cache
                .keys()
                .all(|&p| p <= 12)
        );
    }

    #[test]
    fn readers_on_several_threads_share_one_stream() {
        let data = pages(24, 100_000);
        let bytes = solid_7z(&data, 4, 1 << 16);
        let a = Archive::from_bytes(bytes, opts(3)).unwrap();
        std::thread::scope(|s| {
            for k in 0..4 {
                let (a, data) = (&a, &data);
                s.spawn(move || {
                    for i in (0..24).map(|i| (i * 7 + k * 5) % 24) {
                        assert_eq!(a.read(i).unwrap(), data[i], "thread {k}, entry {i}");
                    }
                });
            }
        });
    }

    #[test]
    fn corrupt_stream_fails_cleanly() {
        let data = pages(12, 100_000);
        let mut bytes = solid_7z(&data, 4, 1 << 16);
        // Flip bytes in the middle of the packed stream (well past the headers at its start).
        let mid = 32 + 300_000;
        for b in &mut bytes[mid..mid + 64] {
            *b ^= 0x5A;
        }
        let a = Archive::from_bytes(bytes, opts(4)).unwrap();
        let mut errors = 0;
        for (i, d) in data.iter().enumerate().take(a.len()) {
            match a.read(i) {
                Ok(v) => assert_eq!(v, *d),
                Err(_) => errors += 1,
            }
        }
        assert!(errors > 0);
    }

    /// A ComicInfo.xml of a few kB.
    fn comic_xml() -> Vec<u8> {
        let mut x = br#"<?xml version="1.0"?><ComicInfo><Manga>YesAndRightToLeft</Manga>
            <Pages><Page Image="0" Type="Deleted"/></Pages>"#
            .to_vec();
        x.extend(std::iter::repeat_n(b' ', 3000));
        x.extend_from_slice(b"</ComicInfo>");
        x
    }

    /// A solid 7z of `data` (named `000.jpg` on) and then `ComicInfo.xml` (last in the
    /// stream), compressed with `methods`.
    fn comic_7z(data: &[Vec<u8>], methods: Vec<sevenz_rust2::EncoderConfiguration>) -> Vec<u8> {
        use sevenz_rust2::{ArchiveWriter, SourceReader};
        let mut w = ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
        w.set_content_methods(methods);
        let mut entries: Vec<ArchiveEntry> = (0..data.len())
            .map(|i| ArchiveEntry::new_file(&format!("{i:03}.jpg")))
            .collect();
        entries.push(ArchiveEntry::new_file("ComicInfo.xml"));
        let mut readers: Vec<SourceReader<Cursor<Vec<u8>>>> = data
            .iter()
            .map(|d| SourceReader::new(Cursor::new(d.clone())))
            .collect();
        readers.push(SourceReader::new(Cursor::new(comic_xml())));
        w.push_archive_entries(entries, readers).unwrap();
        w.finish().unwrap().into_inner()
    }

    /// LZMA2 in independent units of about 1 MiB (level 1's dictionary), after `filter`.
    fn mt_methods(
        filter: Option<sevenz_rust2::EncoderMethod>,
    ) -> Vec<sevenz_rust2::EncoderConfiguration> {
        let mut o = sevenz_rust2::encoder_options::Lzma2Options::from_level_mt(1, 4, 1 << 16);
        o.set_dictionary_size(1 << 16);
        let mut m: Vec<sevenz_rust2::EncoderConfiguration> = vec![o.into()];
        m.extend(filter.map(Into::into));
        m
    }

    /// The archive's ComicInfo entry (its 7z file number).
    fn comic_file(a: &Archive) -> usize {
        let i = a.entries().iter().position(|e| e.name == "ComicInfo.xml");
        file_of(a, i.unwrap())
    }

    /// Waits up to 30 s for `f`.
    fn wait_for(what: &str, f: impl Fn() -> bool) {
        let t = std::time::Instant::now();
        while !f() {
            assert!(t.elapsed().as_secs() < 30, "{what}: never");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn comic_info_is_kept_when_the_streamer_reads_past_it() {
        let data = pages(24, 200_000);
        let a = Archive::from_bytes(comic_7z(&data, mt_methods(None)), opts(4)).unwrap();
        let (s, ci) = (sz(&a), comic_file(&a));
        assert!(s.kept(ci).is_none());
        // Once the pages are read the streamer reads on (its read-ahead) into ComicInfo.
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(a.on_comic_info(move |c| tx.send(c).unwrap()));
        let pages: Vec<usize> = (0..a.len()).filter(|&i| file_of(&a, i) != ci).collect();
        let num = |i: usize| a.entries()[i].name[..3].parse::<usize>().unwrap();
        a.read_many(&pages, &mut |k, r| {
            assert_eq!(r.unwrap(), data[num(pages[k])]);
            true
        });
        wait_for("kept", || s.kept(ci).is_some());
        assert_eq!(&*s.kept(ci).unwrap(), &comic_xml()[..]);
        let found = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(found.found().is_some_and(|c| c.right_to_left()));
        // Known now without a read, however small the budget; a later hook runs at once.
        assert!(a.comic_info_lookup(Some(0)).found().is_some());
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(a.on_comic_info(move |c| tx.send(c).unwrap()));
        assert!(rx.try_recv().unwrap().found().is_some());
    }

    #[test]
    fn comic_info_is_kept_when_a_helper_decodes_its_unit_ahead() {
        let data = pages(24, 200_000);
        let o = Options {
            solid_read_ahead: 100_000,
            ..opts(4)
        };
        let a = Archive::from_bytes(comic_7z(&data, mt_methods(None)), o).unwrap();
        let (s, ci) = (sz(&a), comic_file(&a));
        let plan = s.blocks[0].plan.clone().expect("planned");
        assert!(plan.units().len() >= 3 && plan.seekable());
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(a.on_comic_info(move |c| tx.send(c).unwrap()));
        // A batch up to the last page lets the helpers decode the last unit, ComicInfo's,
        // while the streamer stays near the start.
        let last = (a.entries().iter()).position(|e| e.name == "023.jpg");
        let last = file_of(&a, last.unwrap());
        s.will_read(&[last], true);
        assert_eq!(a.read(0).unwrap(), data[0]);
        wait_for("kept", || s.kept(ci).is_some());
        let (_, block, p) = s.solid_place(ci).unwrap();
        let next = (block.shared.state.lock().stream.as_ref()).map_or(0, |c| c.next);
        assert!(next < p, "the streamer reached it: {next}");
        assert_eq!(&*s.kept(ci).unwrap(), &comic_xml()[..]);
        let found = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(found.found().is_some());
        s.will_read(&[last], false);
    }

    #[test]
    fn comic_info_read_aside_leaves_the_streamer_where_it_is() {
        let data = pages(24, 200_000);
        let bcj = Some(sevenz_rust2::EncoderMethod::BCJ_X86_FILTER);
        let lzma = vec![sevenz_rust2::EncoderMethod::LZMA.into()];
        for (methods, what) in [
            (mt_methods(None), "LZMA2 units"),
            (mt_methods(bcj), "BCJ + LZMA2 units"),
            (lzma, "LZMA"),
        ] {
            let o = Options {
                solid_read_ahead: 100_000,
                ..opts(4)
            };
            let a = Archive::from_bytes(comic_7z(&data, methods), o).unwrap();
            let (s, ci) = (sz(&a), comic_file(&a));
            let (tx, rx) = std::sync::mpsc::channel();
            assert!(a.on_comic_info(move |c| tx.send(c).unwrap()));
            assert_eq!(a.read(0).unwrap(), data[0], "{what}");
            let shared = &s.blocks[0].shared;
            let epoch = shared.state.lock().epoch;
            let c = a.comic_info_aside();
            assert!(c.found().is_some_and(|c| c.right_to_left()), "{what}");
            assert!(rx.try_recv().unwrap().found().is_some(), "{what}");
            {
                let st = shared.state.lock();
                assert_eq!(st.epoch, epoch, "{what}: started again");
                // Nothing it passed over was kept.
                assert!(
                    st.cache.keys().all(|&p| p < 8),
                    "{what}: {:?}",
                    st.cache.keys()
                );
            }
            assert_eq!(a.read(1).unwrap(), data[1], "{what}");
            // Kept: read again without decoding.
            assert!(a.comic_info_lookup(Some(0)).found().is_some(), "{what}");
            let again = s.read_aside(&a.data, ci, &a.options).unwrap().unwrap();
            assert_eq!(again, comic_xml(), "{what}");
        }
    }

    #[test]
    fn comic_info_read_aside_takes_what_the_streamer_kept() {
        // The streamer reads every page meanwhile: whichever gets there first, both agree.
        let data = pages(24, 200_000);
        let a = Archive::from_bytes(comic_7z(&data, mt_methods(None)), opts(4)).unwrap();
        std::thread::scope(|sc| {
            let aside = sc.spawn(|| a.comic_info_aside());
            for i in 0..a.len() {
                a.read(i).unwrap();
            }
            assert!(aside.join().unwrap().found().is_some());
        });
        assert_eq!(&*sz(&a).kept(comic_file(&a)).unwrap(), &comic_xml()[..]);
    }

    #[test]
    fn plan_needs_lzma2_alone_or_with_one_filter() {
        let data = pages(6, 50_000);
        let lzma = {
            use sevenz_rust2::{ArchiveWriter, SourceReader};
            let mut w = ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
            w.set_content_methods(vec![sevenz_rust2::EncoderMethod::LZMA.into()]);
            let entries = (0..data.len())
                .map(|i| ArchiveEntry::new_file(&format!("{i:03}.jpg")))
                .collect();
            let readers = data
                .iter()
                .map(|d| SourceReader::new(Cursor::new(d.clone())))
                .collect();
            w.push_archive_entries(entries, readers).unwrap();
            w.finish().unwrap().into_inner()
        };
        let a = Archive::from_bytes(lzma, opts(4)).unwrap();
        assert!(sz(&a).blocks[0].plan.is_none());
        check_all(&a, &data, "LZMA");
    }
}
