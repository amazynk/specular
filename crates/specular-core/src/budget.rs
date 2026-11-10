//! One memory budget for every decoder.
//!
//! A decoder's fixed caps (16 GiB of decompressed XCF tiles, a gigapixel WebP canvas) keep a
//! hostile file from asking for absurd amounts, but a tiny file can still ask for more than
//! the machine has. Before a large allocation a decoder therefore [`reserve`]s it here: the
//! reservation succeeds while it fits 80 % of the memory available (`MemAvailable` on Linux,
//! not `MemFree`) minus what other decodes have reserved, and is given back when its
//! [`Reservation`] drops (on unwinding too). A refusal is a [`NeedsMemory`] with what the
//! decode needs and what is free, which the app words as "needs 24 GB of free RAM".
//!
//! * The available memory is sampled when a decode starts (no reservations outstanding, the
//!   sample older than [`FRESH`]) and at most every [`RESAMPLE`] while decodes overlap; the
//!   reservations outstanding then count as allocated already. Concurrent decodes share it.
//! * Gallery and thumbnail workers ([`Share::Thumbnails`], [`enter_share`]) together get a
//!   quarter of it, so many parallel thumbnails cannot exhaust it between them.
//! * Unknown available memory: everything fits (the decoders' fixed caps still apply).
//! * Tests and GUI checks: [`MemoryBudget::fixed`] with [`with_budget`] (this thread), or
//!   `SPECULAR_MEMORY_BUDGET` (bytes, or with a K, M or G suffix) for the whole process.
//!
//! A reservation costs one uncontended lock (and, now and then, a read of `/proc/meminfo`).

use std::cell::{Cell, RefCell};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// A sample of the available memory is reused for a new decode this long.
pub const FRESH: Duration = Duration::from_millis(250);
/// While decodes overlap, the available memory is sampled again after this long.
pub const RESAMPLE: Duration = Duration::from_secs(2);

/// The budget is this share of the available memory (as a fraction `NUM / DEN`).
const NUM: u64 = 4;
const DEN: u64 = 5;

/// A decode was refused: it needs `needed` bytes of free RAM and only `free` are free.
/// Both are in terms of the machine's free memory (the budget's headroom included), so
/// `needed > free` always holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NeedsMemory {
    pub needed: u64,
    pub free: u64,
}

impl std::fmt::Display for NeedsMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "needs {} MiB of free RAM, but only {} MiB is free",
            self.needed.div_ceil(1 << 20),
            self.free >> 20
        )
    }
}

impl std::error::Error for NeedsMemory {}

/// Who reserves: a decode the user opened, or one of many gallery and thumbnail workers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Share {
    #[default]
    Full,
    /// Thumbnail and gallery workers: together at most a quarter of the budget.
    Thumbnails,
}

#[derive(Debug, Default)]
struct State {
    /// The budget: 80 % of the available memory at the sample plus what was reserved then
    /// (None: unknown, everything fits).
    pool: Option<u64>,
    sampled: Option<Instant>,
    reserved: u64,
    /// Of `reserved`, by [`Share::Thumbnails`].
    reserved_thumbs: u64,
    count: usize,
}

/// A shared memory budget (the process has one, [`global`]; tests make their own).
#[derive(Debug)]
pub struct MemoryBudget {
    state: Mutex<State>,
    /// A fixed pool, never sampled (tests, `SPECULAR_MEMORY_BUDGET`).
    fixed: Option<u64>,
    /// Where the available memory comes from (None: [`crate::available_memory`]).
    probe: Option<fn() -> Option<u64>>,
}

impl MemoryBudget {
    /// The budget of a process: 80 % of the available memory, sampled.
    pub fn sampled() -> MemoryBudget {
        MemoryBudget {
            state: Mutex::new(State::default()),
            fixed: None,
            probe: None,
        }
    }

    /// A budget of exactly `bytes` (tests, GUI checks).
    pub fn fixed(bytes: u64) -> MemoryBudget {
        MemoryBudget {
            state: Mutex::new(State {
                pool: Some(bytes),
                ..State::default()
            }),
            fixed: Some(bytes),
            probe: None,
        }
    }

    /// A sampled budget whose available memory comes from `probe` (tests).
    pub fn probed(probe: fn() -> Option<u64>) -> MemoryBudget {
        MemoryBudget {
            probe: Some(probe),
            ..MemoryBudget::sampled()
        }
    }

    fn sample(&self, s: &mut State, now: Instant) {
        if self.fixed.is_some() {
            return;
        }
        let age = s.sampled.map(|t| now.saturating_duration_since(t));
        let stale = match age {
            None => true,
            Some(a) => a >= RESAMPLE || (s.count == 0 && a >= FRESH),
        };
        if !stale {
            return;
        }
        let avail = match self.probe {
            Some(p) => p(),
            None => crate::available_memory(),
        };
        s.pool = avail.map(|a| (a / DEN).saturating_mul(NUM).saturating_add(s.reserved));
        s.sampled = Some(now);
    }

    /// Reserves `bytes` for `share`, or says what is needed and free.
    pub fn reserve_for(
        self: &Arc<Self>,
        bytes: u64,
        share: Share,
    ) -> Result<Reservation, NeedsMemory> {
        let mut s = self.state.lock();
        self.sample(&mut s, Instant::now());
        if let Some(pool) = s.pool {
            let mut free = pool.saturating_sub(s.reserved);
            if share == Share::Thumbnails {
                free = free.min((pool / 4).saturating_sub(s.reserved_thumbs));
            }
            if bytes > free {
                return Err(NeedsMemory {
                    needed: to_free(bytes),
                    free: to_free(free),
                });
            }
        }
        s.reserved += bytes;
        s.count += 1;
        if share == Share::Thumbnails {
            s.reserved_thumbs += bytes;
        }
        drop(s);
        Ok(Reservation {
            budget: self.clone(),
            bytes,
            share,
        })
    }

    /// Bytes reserved now.
    pub fn reserved(&self) -> u64 {
        self.state.lock().reserved
    }

    /// The budget's free bytes now for `share`, in free-RAM terms (None: unknown).
    pub fn free(&self, share: Share) -> Option<u64> {
        self.unreserved(share).map(to_free)
    }

    /// The bytes `share` could reserve now, in budget terms (None: unknown).
    fn unreserved(&self, share: Share) -> Option<u64> {
        let mut s = self.state.lock();
        self.sample(&mut s, Instant::now());
        let pool = s.pool?;
        let mut free = pool.saturating_sub(s.reserved);
        if share == Share::Thumbnails {
            free = free.min((pool / 4).saturating_sub(s.reserved_thumbs));
        }
        Some(free)
    }

    fn release(&self, bytes: u64, share: Share, count: bool) {
        let mut s = self.state.lock();
        s.reserved = s.reserved.saturating_sub(bytes);
        if share == Share::Thumbnails {
            s.reserved_thumbs = s.reserved_thumbs.saturating_sub(bytes);
        }
        if count {
            s.count = s.count.saturating_sub(1);
        }
    }
}

/// Budget bytes as the free RAM they stand for (the budget is 80 % of it).
fn to_free(b: u64) -> u64 {
    b.saturating_mul(DEN).div_ceil(NUM)
}

/// Reserved memory, given back on drop.
#[derive(Debug)]
#[must_use = "the reservation is given back when dropped"]
pub struct Reservation {
    budget: Arc<MemoryBudget>,
    bytes: u64,
    share: Share,
}

impl Reservation {
    /// A reservation of nothing (a decoder that grows one as data arrives starts here).
    pub fn none() -> Reservation {
        let budget = current();
        budget.state.lock().count += 1;
        Reservation {
            budget,
            bytes: 0,
            share: share(),
        }
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Reserves `more` bytes besides (a buffer that grows as data arrives).
    pub fn grow(&mut self, more: u64) -> Result<(), NeedsMemory> {
        let r = self.budget.reserve_for(more, self.share)?;
        // Merged into this one: one release, one count.
        self.budget.release(0, self.share, true);
        self.bytes += r.bytes;
        std::mem::forget(r);
        Ok(())
    }

    /// Grows the reservation to `total` bytes, if it is less.
    pub fn grow_to(&mut self, total: u64) -> Result<(), NeedsMemory> {
        match total.checked_sub(self.bytes) {
            Some(more) if more > 0 => self.grow(more),
            _ => Ok(()),
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.budget.release(self.bytes, self.share, true);
    }
}

/// The process's budget (`SPECULAR_MEMORY_BUDGET` fixes it).
pub fn global() -> &'static Arc<MemoryBudget> {
    static G: LazyLock<Arc<MemoryBudget>> = LazyLock::new(|| {
        Arc::new(
            match std::env::var("SPECULAR_MEMORY_BUDGET")
                .ok()
                .and_then(|v| parse_bytes(&v))
            {
                Some(b) => MemoryBudget::fixed(b),
                None => MemoryBudget::sampled(),
            },
        )
    });
    &G
}

/// "512M", "2G", "4096" (bytes).
fn parse_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, shift) = match s.char_indices().last()? {
        (i, 'k' | 'K') => (&s[..i], 10),
        (i, 'm' | 'M') => (&s[..i], 20),
        (i, 'g' | 'G') => (&s[..i], 30),
        _ => (s, 0),
    };
    num.trim().parse::<u64>().ok()?.checked_mul(1 << shift)
}

thread_local! {
    static OVERRIDE: RefCell<Option<Arc<MemoryBudget>>> = const { RefCell::new(None) };
    static SHARE: Cell<Share> = const { Cell::new(Share::Full) };
}

/// The budget this thread reserves from.
fn current() -> Arc<MemoryBudget> {
    OVERRIDE
        .with(|o| o.borrow().clone())
        .unwrap_or_else(|| global().clone())
}

fn share() -> Share {
    SHARE.with(Cell::get)
}

/// This thread's share: a decode can pick a cheaper rendition for thumbnail workers.
pub fn current_share() -> Share {
    share()
}

/// Reserves `bytes` from this thread's budget, in this thread's share.
pub fn reserve(bytes: u64) -> Result<Reservation, NeedsMemory> {
    current().reserve_for(bytes, share())
}

/// This thread's budget's free bytes now, in its share and in free-RAM terms (None:
/// unknown). For caches sized to what is free rather than reserved.
pub fn free() -> Option<u64> {
    current().free(share())
}

/// Reserves `count × each` bytes (an overflow needs more than any machine has).
pub fn reserve_n(count: u64, each: u64) -> Result<Reservation, NeedsMemory> {
    reserve(count.saturating_mul(each))
}

/// Runs `f` with `budget` as this thread's budget (the test hook; also for a worker that
/// must not take from the process's budget).
pub fn with_budget<R>(budget: Arc<MemoryBudget>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Arc<MemoryBudget>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            OVERRIDE.with(|o| *o.borrow_mut() = prev);
        }
    }
    let prev = OVERRIDE.with(|o| o.borrow_mut().replace(budget));
    let _restore = Restore(prev);
    f()
}

/// Runs `f` with a fixed budget of `bytes` on this thread (tests).
pub fn with_limit<R>(bytes: u64, f: impl FnOnce() -> R) -> R {
    with_budget(Arc::new(MemoryBudget::fixed(bytes)), f)
}

/// This thread reserves in `share` until the guard drops (thumbnail workers).
pub fn enter_share(share: Share) -> ShareGuard {
    ShareGuard(SHARE.with(|s| s.replace(share)))
}

/// Restores the thread's share on drop.
#[must_use]
pub struct ShareGuard(Share);

impl Drop for ShareGuard {
    fn drop(&mut self) {
        SHARE.with(|s| s.set(self.0));
    }
}

/// A thread's budget and share, captured with [`current_budget`].
///
/// For crates with fixed caps, and for work handed to other threads (rayon pools, helper
/// threads), which would otherwise reserve from the process's budget in the full share: carry
/// the handle over and [`reserve`](BudgetHandle::reserve) from it, or [`run`](BudgetHandle::run)
/// the work under it.
#[derive(Clone, Debug)]
pub struct BudgetHandle {
    budget: Arc<MemoryBudget>,
    share: Share,
}

/// This thread's budget (a [`with_budget`] override, else the process's) and share.
pub fn current_budget() -> BudgetHandle {
    BudgetHandle {
        budget: current(),
        share: share(),
    }
}

impl BudgetHandle {
    /// Reserves `bytes` from the captured budget, in the captured share.
    pub fn reserve(&self, bytes: u64) -> Result<Reservation, NeedsMemory> {
        self.budget.reserve_for(bytes, self.share)
    }

    /// Reserves `count × each` bytes (an overflow needs more than any machine has).
    pub fn reserve_n(&self, count: u64, each: u64) -> Result<Reservation, NeedsMemory> {
        self.reserve(count.saturating_mul(each))
    }

    /// Free bytes now, in free-RAM terms (None: unknown).
    pub fn free(&self) -> Option<u64> {
        self.budget.free(self.share)
    }

    /// A fixed cap lowered to what could be reserved now: `min(fixed, unreserved)`. Unknown
    /// memory leaves the cap as it is. A size check, not a reservation.
    pub fn cap(&self, fixed: u64) -> u64 {
        self.budget
            .unreserved(self.share)
            .map_or(fixed, |f| f.min(fixed))
    }

    /// Runs `f` on this thread with the captured budget and share (a worker thread's job).
    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let _share = enter_share(self.share);
        with_budget(self.budget.clone(), f)
    }

    /// The budget itself.
    pub fn budget(&self) -> &Arc<MemoryBudget> {
        &self.budget
    }
}

/// `fixed` lowered to what this thread's budget could reserve now ([`BudgetHandle::cap`]).
pub fn cap(fixed: u64) -> u64 {
    current_budget().cap(fixed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn handle_carries_the_budget_to_another_thread() {
        let b = Arc::new(MemoryBudget::fixed(100 * MIB));
        let h = with_budget(b.clone(), || {
            let _t = enter_share(Share::Thumbnails);
            current_budget()
        });
        std::thread::scope(|s| {
            s.spawn(|| {
                // Not this thread's budget: the handle's, in the thumbnails' quarter.
                let r = h.reserve(20 * MIB).unwrap();
                assert_eq!(b.reserved(), 20 * MIB);
                assert!(h.reserve(10 * MIB).is_err());
                assert_eq!(h.cap(64 * MIB), 5 * MIB);
                h.run(|| {
                    assert_eq!(current_share(), Share::Thumbnails);
                    assert!(reserve(6 * MIB).is_err());
                    let _r = reserve_n(5, MIB).unwrap();
                    assert_eq!(b.reserved(), 25 * MIB);
                    assert_eq!(cap(u64::MAX), 0);
                });
                assert_eq!(current_share(), Share::Full);
                drop(r);
            });
        });
        assert_eq!(b.reserved(), 0);
        assert_eq!(h.cap(7), 7);
        assert_eq!(h.free(), Some(to_free(25 * MIB)));
    }

    const MIB: u64 = 1 << 20;

    #[test]
    fn reservations_release_on_drop() {
        let b = Arc::new(MemoryBudget::fixed(100 * MIB));
        let r = b.reserve_for(60 * MIB, Share::Full).unwrap();
        assert_eq!(b.reserved(), 60 * MIB);
        let e = b.reserve_for(50 * MIB, Share::Full).unwrap_err();
        assert_eq!(e.needed, to_free(50 * MIB));
        assert_eq!(e.free, to_free(40 * MIB));
        assert!(e.needed > e.free);
        drop(r);
        assert_eq!(b.reserved(), 0);
        let _r = b.reserve_for(50 * MIB, Share::Full).unwrap();
    }

    #[test]
    fn reservations_release_on_panic() {
        let b = Arc::new(MemoryBudget::fixed(100 * MIB));
        let b2 = b.clone();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _r = b2.reserve_for(80 * MIB, Share::Full).unwrap();
            panic!("decoder bug");
        }));
        assert!(r.is_err());
        assert_eq!(b.reserved(), 0);
        assert_eq!(b.state.lock().count, 0);
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn concurrent_reservations_add_up() {
        let b = Arc::new(MemoryBudget::fixed(100 * MIB));
        let ok = std::sync::atomic::AtomicUsize::new(0);
        let held = Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..16 {
                s.spawn(|| {
                    if let Ok(r) = b.reserve_for(10 * MIB, Share::Full) {
                        ok.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        held.lock().push(r);
                    }
                });
            }
        });
        assert_eq!(ok.into_inner(), 10, "exactly ten 10 MiB fit 100 MiB");
        assert_eq!(b.reserved(), 100 * MIB);
        held.lock().clear();
        assert_eq!(b.reserved(), 0);
    }

    #[test]
    fn thumbnails_share_a_quarter() {
        let b = Arc::new(MemoryBudget::fixed(100 * MIB));
        let t1 = b.reserve_for(20 * MIB, Share::Thumbnails).unwrap();
        let e = b.reserve_for(10 * MIB, Share::Thumbnails).unwrap_err();
        assert_eq!(e.free, to_free(5 * MIB));
        // The viewer still has the rest.
        let v = b.reserve_for(75 * MIB, Share::Full).unwrap();
        drop((t1, v));
        assert_eq!(b.reserved(), 0);
    }

    #[test]
    fn growing_reserves_as_it_goes() {
        let b = Arc::new(MemoryBudget::fixed(100 * MIB));
        with_budget(b.clone(), || {
            let mut r = Reservation::none();
            r.grow_to(40 * MIB).unwrap();
            r.grow_to(30 * MIB).unwrap();
            assert_eq!(r.bytes(), 40 * MIB);
            r.grow(60 * MIB).unwrap();
            assert!(r.grow(1).is_err());
            assert_eq!(b.reserved(), 100 * MIB);
            assert_eq!(b.state.lock().count, 1);
        });
        assert_eq!(b.reserved(), 0);
        assert_eq!(b.state.lock().count, 0);
    }

    #[test]
    fn the_thread_hook_overrides_and_restores() {
        let e = with_limit(MIB, || reserve(2 * MIB).unwrap_err());
        assert_eq!(e.needed, to_free(2 * MIB));
        // Nested and restored.
        with_limit(10 * MIB, || {
            with_limit(MIB, || assert!(reserve(2 * MIB).is_err()));
            let _r = reserve(2 * MIB).unwrap();
        });
        let _g = enter_share(Share::Thumbnails);
        assert_eq!(share(), Share::Thumbnails);
        drop(_g);
        assert_eq!(share(), Share::Full);
    }

    #[test]
    fn unknown_memory_fits_everything() {
        let b = Arc::new(MemoryBudget::probed(|| None));
        let _r = b.reserve_for(u64::MAX / 2, Share::Full).unwrap();
        assert_eq!(b.free(Share::Full), None);
    }

    #[test]
    fn sampled_budget_is_80_percent_of_available() {
        let b = Arc::new(MemoryBudget::probed(|| Some(1000 * MIB)));
        assert_eq!(b.free(Share::Full), Some(1000 * MIB));
        let _r = b.reserve_for(800 * MIB, Share::Full).unwrap();
        let e = b.reserve_for(MIB, Share::Full).unwrap_err();
        assert_eq!(e.free, 0);
        // A refusal's needed bytes are in free-RAM terms: 800 MiB of budget needs 1000 free.
        let b = Arc::new(MemoryBudget::probed(|| Some(999 * MIB)));
        let e = b.reserve_for(800 * MIB, Share::Full).unwrap_err();
        assert_eq!(e.needed, 1000 * MIB);
        assert!(e.free < e.needed);
    }

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_bytes("512M"), Some(512 * MIB));
        assert_eq!(parse_bytes("2g"), Some(2 << 30));
        assert_eq!(parse_bytes("4096"), Some(4096));
        assert_eq!(parse_bytes("x"), None);
    }

    #[test]
    #[ignore = "timing; run with --ignored"]
    fn reservation_cost() {
        let b = Arc::new(MemoryBudget::sampled());
        let n = 1_000_000;
        let t = Instant::now();
        for _ in 0..n {
            let r = b.reserve_for(MIB, Share::Full).unwrap();
            drop(r);
        }
        let per = t.elapsed() / n;
        eprintln!("reserve + release: {per:?}");
        let t = Instant::now();
        for _ in 0..1000 {
            let _ = crate::available_memory();
        }
        eprintln!("available_memory(): {:?}", t.elapsed() / 1000);
    }
}
