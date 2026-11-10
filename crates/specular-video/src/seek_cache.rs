//! Decoder snapshots near recent seek targets (software HEVC, H.264 and MPEG-1/2).
//!
//! A seek normally decodes from the sync sample before its target: up to a whole GOP (two
//! seconds of 4K in the clips cameras write, ten or more in broadcast and screen recordings).
//! While a software decoder works through a GOP, a snapshot of its state (the reference
//! pictures and what is due for output, shared, not copied) is kept every [`EVERY`] samples.
//! A later seek into the same stretch, typically scrubbing backwards or jumping back to a
//! spot just seen, resumes from the nearest snapshot before its target instead.
//!
//! The pictures a snapshot holds stay alive while it is kept, so the cache is capped: at most
//! [`CAP`] bytes of samples and a quarter of the free decode budget, least recently used
//! snapshots dropped first. Pictures shared between snapshots count once. HEVC pictures hold
//! their own reservation from the budget (shared by the decoder and every snapshot), so the
//! cache reserves nothing for them. H.264 and MPEG-1/2 reserve their decoded picture buffer
//! as one block: the cache reserves the bytes of the pictures its snapshots hold besides
//! (counting those the decoder still holds once more).

use std::collections::{BTreeMap, HashSet};

use specular_core::budget::{self, Reservation};
use specular_heic::hevc;

/// Samples decoded between snapshots.
pub(crate) const EVERY: usize = 4;
/// Most bytes of decoded samples the snapshots may keep alive (40 pictures of 4K at 10 bits,
/// 170 of 1080p), and at most a quarter of what the budget has free.
pub(crate) const CAP: u64 = 1 << 30;

fn cap() -> u64 {
    budget::free().map_or(CAP / 4, |f| (f / 4).min(CAP))
}

/// A decoder's state between two samples.
#[derive(Clone)]
pub(crate) enum State {
    Hevc(hevc::Snapshot),
    H264(specular_h264::Snapshot),
    Mpeg2(Box<specular_mpeg2::Snapshot>),
}

impl State {
    /// The pictures held, each as (identity, bytes).
    fn pictures(&self) -> Vec<(usize, u64)> {
        match self {
            State::Hevc(s) => s.pictures(),
            State::H264(s) => s.pictures(),
            State::Mpeg2(s) => s.pictures(),
        }
    }

    /// Its pictures hold their own reservations (software HEVC).
    fn reserved(&self) -> bool {
        matches!(self, State::Hevc(_))
    }
}

struct Snap {
    state: State,
    /// Pictures before this composition time may have been left out (skipped, or not made
    /// into frames) on the way: the snapshot only serves targets from here on.
    valid_from: i64,
    used: u64,
}

/// A snapshot to resume from.
pub(crate) struct Found {
    pub(crate) state: State,
    pub(crate) valid_from: i64,
}

pub(crate) struct SeekCache {
    /// By the decoding position of the next sample to decode.
    snaps: BTreeMap<usize, Snap>,
    clock: u64,
    /// Decoding position of the latest snapshot, or where decoding last (re)started.
    pub(crate) last_at: usize,
    /// The budget's share for pictures without a reservation of their own.
    held: Option<Reservation>,
    /// Takes no snapshots (to measure seeks without it).
    pub(crate) off: bool,
}

impl SeekCache {
    pub(crate) fn new() -> SeekCache {
        SeekCache {
            snaps: BTreeMap::new(),
            clock: 0,
            last_at: 0,
            held: None,
            off: false,
        }
    }

    /// Whether a snapshot is due with the next sample to decode at `at`.
    pub(crate) fn due(&self, at: usize) -> bool {
        !self.off && at >= self.last_at + EVERY && !self.snaps.contains_key(&at)
    }

    /// Keeps `state` (the decoder with sample `at` next), dropping the least recently used
    /// snapshots to stay within [`CAP`] and the budget.
    pub(crate) fn insert(&mut self, at: usize, state: State, valid_from: i64) {
        self.last_at = at;
        self.clock += 1;
        self.snaps.insert(
            at,
            Snap {
                state,
                valid_from,
                used: self.clock,
            },
        );
        self.fit();
    }

    /// Drops least recently used snapshots until the rest fit the cap, and reserves what
    /// they hold besides the pictures that are reserved already.
    fn fit(&mut self) {
        loop {
            // Released first: what is free then includes it.
            self.held = None;
            if self.bytes() <= cap() {
                let own = self.bytes_where(|s| !s.reserved());
                if own == 0 {
                    return;
                }
                if let Ok(r) = budget::reserve(own) {
                    self.held = Some(r);
                    return;
                }
            }
            let oldest = self
                .snaps
                .iter()
                .min_by_key(|(_, s)| s.used)
                .map(|(&k, _)| k);
            match oldest {
                Some(k) => {
                    self.snaps.remove(&k);
                }
                None => return,
            }
        }
    }

    /// Bytes of the distinct pictures the snapshots hold.
    fn bytes(&self) -> u64 {
        self.bytes_with(None)
    }

    fn bytes_where(&self, keep: impl Fn(&State) -> bool) -> u64 {
        let mut seen = HashSet::new();
        let mut total = 0;
        for s in self.snaps.values().map(|s| &s.state).filter(|s| keep(s)) {
            for (id, b) in s.pictures() {
                if seen.insert(id) {
                    total += b;
                }
            }
        }
        total
    }

    /// Bytes of the distinct pictures the snapshots and `also` hold.
    pub(crate) fn bytes_with(&self, also: Option<&State>) -> u64 {
        let mut seen = HashSet::new();
        let mut total = 0;
        for s in self.snaps.values().map(|s| &s.state).chain(also) {
            for (id, b) in s.pictures() {
                if seen.insert(id) {
                    total += b;
                }
            }
        }
        total
    }

    /// Bytes the cache has reserved for pictures without a reservation of their own.
    pub(crate) fn reserved(&self) -> u64 {
        self.held.as_ref().map_or(0, Reservation::bytes)
    }

    /// The latest snapshot after the sync sample `key` and no later than `target` (decoding
    /// positions) that serves a target presented at `cts`.
    pub(crate) fn best(&mut self, key: usize, target: usize, cts: i64) -> Option<(usize, Found)> {
        if target <= key {
            return None;
        }
        let (&at, s) = self
            .snaps
            .range_mut(key + 1..=target)
            .rev()
            .find(|(_, s)| s.valid_from <= cts)?;
        self.clock += 1;
        s.used = self.clock;
        Some((
            at,
            Found {
                state: s.state.clone(),
                valid_from: s.valid_from,
            },
        ))
    }

    /// The snapshot at `at` could not be restored (a picture of it was cancelled).
    pub(crate) fn forget(&mut self, at: usize) {
        self.snaps.remove(&at);
        self.fit();
    }
}
