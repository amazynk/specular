//! The play queue: its items, the one playing, and what plays next.
//!
//! In order, the next item is the one after the current (addresses other than `http(s)` are
//! skipped; with repeat all the list starts over). Shuffled, each round plays every playable item once in
//! a random order before any plays again; a history of what played lets Previous go back.
//! Repeat one plays the current item again when it ends, while Next and Previous still move.
//!
//! [`Queue::next_item`] answers "what plays next" without changing anything, so a player
//! can open the next item before the current one ends (gapless, crossfade);
//! [`Queue::advance`] then moves to exactly that item.

use std::path::Path;

use crate::Entry;

/// What happens at the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Repeat {
    /// Play to the end of the list and stop.
    #[default]
    Off,
    /// Start over from the top (a new shuffled round).
    All,
    /// Play the current item again.
    One,
}

impl Repeat {
    /// Off → all → one → off (the transport button and the key).
    pub fn cycled(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// The queue.
#[derive(Clone, Debug)]
pub struct Queue {
    items: Vec<Entry>,
    current: Option<usize>,
    /// With no current item: where the next one is looked for (the item that took the place
    /// of a removed current one).
    resume: usize,
    repeat: Repeat,
    shuffle: bool,
    /// Shuffled: the items still to play this round, in play order (never the current one).
    upcoming: Vec<usize>,
    /// The items played before the current one, oldest first.
    history: Vec<usize>,
    rng: u64,
}

impl Default for Queue {
    fn default() -> Self {
        Queue::new(0x9E37_79B9_7F4A_7C15)
    }
}

impl Queue {
    /// An empty queue whose shuffles come from `seed`.
    pub fn new(seed: u64) -> Queue {
        Queue {
            items: Vec::new(),
            current: None,
            resume: 0,
            repeat: Repeat::Off,
            shuffle: false,
            upcoming: Vec::new(),
            history: Vec::new(),
            rng: seed | 1,
        }
    }

    pub fn items(&self) -> &[Entry] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, i: usize) -> Option<&Entry> {
        self.items.get(i)
    }

    /// Item `i`, to fill in its title and length once read.
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Entry> {
        self.items.get_mut(i)
    }

    /// The item playing (or last played and still current).
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    pub fn current_entry(&self) -> Option<&Entry> {
        self.current.and_then(|i| self.items.get(i))
    }

    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    pub fn set_repeat(&mut self, r: Repeat) {
        self.repeat = r;
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Shuffle on starts a new round (the current item has played); off plays on in list
    /// order from the current item.
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        self.history.clear();
        self.upcoming = if on {
            let mut rng = self.rng;
            let r = self.round(&mut rng, self.current, false);
            self.rng = rng;
            r
        } else {
            Vec::new()
        };
    }

    /// The first item whose file is `path`.
    pub fn find(&self, path: &Path) -> Option<usize> {
        self.items
            .iter()
            .position(|e| e.location.path() == Some(path))
    }

    /// What plays after the current item, without changing anything. `auto`: the current
    /// item came to its end (repeat one plays it again); otherwise Next was asked for.
    pub fn next_item(&self, auto: bool) -> Option<usize> {
        if auto
            && self.repeat == Repeat::One
            && let Some(c) = self.current.filter(|&c| self.playable(c))
        {
            return Some(c);
        }
        // Moving on by hand under repeat one wraps as repeat all does.
        let wrap = self.repeat == Repeat::All || (!auto && self.repeat == Repeat::One);
        if self.shuffle {
            if let Some(&i) = self.upcoming.first() {
                return Some(i);
            }
            if !wrap {
                return None;
            }
            let mut rng = self.rng;
            return self.round(&mut rng, self.current, true).first().copied();
        }
        let start = self.current.map_or(self.resume, |c| c + 1);
        if let Some(i) = (start..self.items.len()).find(|&i| self.playable(i)) {
            return Some(i);
        }
        if !wrap {
            return None;
        }
        (0..start.min(self.items.len())).find(|&i| self.playable(i))
    }

    /// What Previous goes to, without changing anything: the item played before (shuffled),
    /// else the one above the current (the last one, with repeat on, from the first).
    pub fn prev_item(&self) -> Option<usize> {
        if self.shuffle {
            return self.history.last().copied();
        }
        let end = self.current.unwrap_or(self.resume).min(self.items.len());
        if let Some(i) = (0..end).rev().find(|&i| self.playable(i)) {
            return Some(i);
        }
        if self.repeat == Repeat::Off {
            return None;
        }
        (end..self.items.len()).rev().find(|&i| self.playable(i))
    }

    /// Moves to [`Queue::next_item`] and returns it.
    pub fn advance(&mut self, auto: bool) -> Option<usize> {
        let n = self.next_item(auto)?;
        if Some(n) == self.current {
            // Repeat one, or a single item that starts over.
            if self.shuffle && self.upcoming.first() == Some(&n) {
                self.upcoming.remove(0);
            }
            return Some(n);
        }
        if self.shuffle {
            if self.upcoming.first() == Some(&n) {
                self.upcoming.remove(0);
            } else {
                // A new round: the same one `next_item` saw (the same draws).
                let mut rng = self.rng;
                let mut r = self.round(&mut rng, self.current, true);
                self.rng = rng;
                debug_assert_eq!(r.first(), Some(&n));
                r.retain(|&i| i != n);
                self.upcoming = r;
            }
        }
        if let Some(c) = self.current {
            self.history.push(c);
        }
        self.current = Some(n);
        Some(n)
    }

    /// Moves to [`Queue::prev_item`] and returns it. Shuffled, the item left plays next
    /// again.
    pub fn go_back(&mut self) -> Option<usize> {
        let p = self.prev_item()?;
        if self.shuffle {
            self.history.pop();
            if let Some(c) = self.current
                && self.playable(c)
            {
                self.upcoming.insert(0, c);
            }
            self.upcoming.retain(|&i| i != p);
        }
        self.current = Some(p);
        Some(p)
    }

    /// Item `i` plays now (chosen in the list): what played goes into the history.
    pub fn play(&mut self, i: usize) -> bool {
        if i >= self.items.len() || !self.playable(i) {
            return false;
        }
        if self.current == Some(i) {
            return true;
        }
        if let Some(c) = self.current {
            self.history.push(c);
        }
        self.upcoming.retain(|&u| u != i);
        self.current = Some(i);
        true
    }

    /// Something outside the queue plays: no item is current; the next one is the item after
    /// the one that was.
    pub fn detach(&mut self) {
        if let Some(c) = self.current.take() {
            self.resume = c + 1;
        }
    }

    /// Adds `entries` at the end. Shuffled, they take random places among the items still to
    /// play.
    pub fn append(&mut self, entries: Vec<Entry>) -> std::ops::Range<usize> {
        let start = self.items.len();
        self.items.extend(entries);
        let range = start..self.items.len();
        if self.shuffle {
            for i in range.clone() {
                if self.playable(i) {
                    let at = (next_u64(&mut self.rng) % (self.upcoming.len() as u64 + 1)) as usize;
                    self.upcoming.insert(at, i);
                }
            }
        }
        range
    }

    /// Adds `entries` to play right after the current item ("Play next"), in their order,
    /// shuffled or not.
    pub fn insert_next(&mut self, entries: Vec<Entry>) -> std::ops::Range<usize> {
        let at = self
            .current
            .map_or(self.resume.min(self.items.len()), |c| c + 1);
        let n = entries.len();
        self.remap(|i| Some(if i >= at { i + n } else { i }));
        // With no current item, new ones at `resume` are the next looked at.
        if self.resume > at {
            self.resume += n;
        }
        self.items.splice(at..at, entries);
        let range = at..at + n;
        if self.shuffle {
            let fresh: Vec<usize> = range.clone().filter(|&i| self.playable(i)).collect();
            self.upcoming.splice(0..0, fresh);
        }
        range
    }

    /// Takes item `i` out of the queue (the file stays where it is).
    pub fn remove(&mut self, i: usize) -> Option<Entry> {
        if i >= self.items.len() {
            return None;
        }
        if self.current == Some(i) {
            self.current = None;
            self.resume = i;
        } else if self.resume > i {
            self.resume -= 1;
        }
        self.remap(|j| match j.cmp(&i) {
            std::cmp::Ordering::Less => Some(j),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(j - 1),
        });
        Some(self.items.remove(i))
    }

    /// Moves item `from` to place `to` (dragged in the list).
    pub fn move_item(&mut self, from: usize, to: usize) {
        let len = self.items.len();
        if from >= len || to >= len || from == to {
            return;
        }
        let mut order: Vec<usize> = (0..len).collect();
        let x = order.remove(from);
        order.insert(to, x);
        let mut new_of = vec![0; len];
        for (new, &old) in order.iter().enumerate() {
            new_of[old] = new;
        }
        self.remap(|i| Some(new_of[i]));
        if self.current.is_none() && self.resume < len {
            self.resume = new_of[self.resume];
        }
        let e = self.items.remove(from);
        self.items.insert(to, e);
    }

    /// Empties the queue (repeat and shuffle stay as they are).
    pub fn clear(&mut self) {
        self.items.clear();
        self.current = None;
        self.resume = 0;
        self.upcoming.clear();
        self.history.clear();
    }

    fn playable(&self, i: usize) -> bool {
        self.items.get(i).is_some_and(Entry::playable)
    }

    /// A shuffled round of the playable items. `current` (the one playing) is left out of a
    /// round started while it plays (`again` false: it has played); in the next round
    /// (`again`) it is in, but not first, unless it is the only one.
    fn round(&self, rng: &mut u64, current: Option<usize>, again: bool) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.items.len())
            .filter(|&i| self.playable(i) && (again || Some(i) != current))
            .collect();
        for k in (1..v.len()).rev() {
            let j = (next_u64(rng) % (k as u64 + 1)) as usize;
            v.swap(k, j);
        }
        if v.len() > 1 && current.is_some() && v.first() == current.as_ref() {
            let j = 1 + (next_u64(rng) % (v.len() as u64 - 1)) as usize;
            v.swap(0, j);
        }
        v
    }

    fn remap(&mut self, f: impl Fn(usize) -> Option<usize>) {
        self.current = self.current.and_then(&f);
        self.upcoming = self.upcoming.iter().filter_map(|&i| f(i)).collect();
        self.history = self.history.iter().filter_map(|&i| f(i)).collect();
    }
}

/// xorshift64*: enough for shuffling.
fn next_u64(s: &mut u64) -> u64 {
    let mut x = *s;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *s = x;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}
