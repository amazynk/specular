//! Candidates: multi-index hashing over the 64-bit perceptual hashes.
//!
//! The hash is cut into four 16-bit chunks, each indexed in its own table. Two hashes within
//! Hamming distance `r` agree on at least one chunk to within `r / 4` bits (pigeonhole), so a
//! query only probes the chunk values within that small radius. Tables are flat (CSR) and keep
//! the hashes beside the ids, so a probe is a sequential scan; the index of 100k hashes is
//! about 5.8 MB and builds in a few milliseconds.

use std::collections::HashSet;

const CHUNKS: usize = 4;
const BUCKETS: usize = 1 << 16;

/// Exact Hamming-radius search over a fixed set of 64-bit hashes.
pub struct HashIndex {
    hashes: Vec<u64>,
    /// Per chunk: bucket start offsets (BUCKETS + 1) into `ids` and `vals`.
    offsets: Vec<Vec<u32>>,
    ids: Vec<Vec<u32>>,
    /// The hashes in bucket order, so a probe scans memory in sequence.
    vals: Vec<Vec<u64>>,
}

fn chunk(h: u64, c: usize) -> usize {
    ((h >> (16 * c)) & 0xFFFF) as usize
}

/// All 16-bit masks with at most `k` bits set.
fn masks(k: u32) -> Vec<u16> {
    let mut m = vec![0u16];
    if k >= 1 {
        m.extend((0..16).map(|i| 1u16 << i));
    }
    if k >= 2 {
        for i in 0..16 {
            for j in i + 1..16 {
                m.push((1 << i) | (1 << j));
            }
        }
    }
    if k >= 3 {
        for i in 0..16 {
            for j in i + 1..16 {
                for l in j + 1..16 {
                    m.push((1 << i) | (1 << j) | (1 << l));
                }
            }
        }
    }
    m
}

/// The first chunk in which `a` and `b` are within `k` bits: the one probe that reports
/// the pair (no "seen" set needed).
fn first_close_chunk(a: u64, b: u64, k: u32) -> usize {
    let x = a ^ b;
    (0..CHUNKS)
        .find(|&c| ((x >> (16 * c)) & 0xFFFF).count_ones() <= k)
        .unwrap_or(CHUNKS)
}

impl HashIndex {
    pub fn new(hashes: Vec<u64>) -> HashIndex {
        let mut offsets = Vec::with_capacity(CHUNKS);
        let mut ids = Vec::with_capacity(CHUNKS);
        let mut vals = Vec::with_capacity(CHUNKS);
        for c in 0..CHUNKS {
            let mut count = vec![0u32; BUCKETS + 1];
            for &h in &hashes {
                count[chunk(h, c) + 1] += 1;
            }
            for i in 0..BUCKETS {
                count[i + 1] += count[i];
            }
            let mut fill = count.clone();
            let mut v = vec![0u32; hashes.len()];
            let mut hv = vec![0u64; hashes.len()];
            for (i, &h) in hashes.iter().enumerate() {
                let b = chunk(h, c);
                v[fill[b] as usize] = i as u32;
                hv[fill[b] as usize] = h;
                fill[b] += 1;
            }
            offsets.push(count);
            ids.push(v);
            vals.push(hv);
        }
        HashIndex {
            hashes,
            offsets,
            ids,
            vals,
        }
    }

    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }

    /// Every pair `(i, j, distance)` with `i < j` and distance ≤ `r` (`r` ≤ 15), in no
    /// particular order.
    pub fn pairs(&self, r: u32) -> Vec<(u32, u32, u32)> {
        let r = r.min(15);
        let k = r / CHUNKS as u32;
        let ms = masks(k);
        let mut out = Vec::new();
        for (i, &q) in self.hashes.iter().enumerate() {
            for c in 0..CHUNKS {
                let qc = chunk(q, c) as u16;
                for &m in &ms {
                    let b = (qc ^ m) as usize;
                    let (s, e) = (self.offsets[c][b] as usize, self.offsets[c][b + 1] as usize);
                    for (t, &v) in self.vals[c][s..e].iter().enumerate() {
                        let d = (q ^ v).count_ones();
                        if d > r {
                            continue;
                        }
                        let j = self.ids[c][s + t];
                        if j as usize > i && first_close_chunk(q, v, k) == c {
                            out.push((i as u32, j, d));
                        }
                    }
                }
            }
        }
        out
    }

    /// Indices within distance `r` of `q` (for "similar: this" and new files).
    pub fn within(&self, q: u64, r: u32) -> Vec<(u32, u32)> {
        let r = r.min(15);
        let k = r / CHUNKS as u32;
        let ms = masks(k);
        let mut out: Vec<(u32, u32)> = Vec::new();
        for c in 0..CHUNKS {
            let qc = chunk(q, c) as u16;
            for &m in &ms {
                let b = (qc ^ m) as usize;
                let (s, e) = (self.offsets[c][b] as usize, self.offsets[c][b + 1] as usize);
                for (t, &v) in self.vals[c][s..e].iter().enumerate() {
                    let d = (q ^ v).count_ones();
                    if d <= r && first_close_chunk(q, v, k) == c {
                        out.push((self.ids[c][s + t], d));
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }
}

/// The (id, hash) entries of one chunk value.
type Bucket = Vec<(u32, u64)>;

/// The same search over a set that changes (the Library's catalogue): a hash can be added
/// or taken out at any time, under an id of the caller's choosing. Buckets are small vectors
/// of (id, hash), so a probe still scans memory in sequence; 100k hashes take about 8 MB.
pub struct DynIndex {
    /// Per chunk, per chunk value: the (id, hash) entries.
    tables: Vec<Box<[Bucket]>>,
    len: usize,
    /// Entries taken out but still in their buckets: one in a crowded bucket (thousands of
    /// copies of one image share every bucket) is marked rather than looked for, and the
    /// buckets are swept once these are many.
    dead: HashSet<(u32, u64)>,
}

/// A bucket longer than this has its entries marked dead on removal rather than looked for.
const CROWDED: usize = 64;

impl Default for DynIndex {
    fn default() -> DynIndex {
        DynIndex::new()
    }
}

impl DynIndex {
    pub fn new() -> DynIndex {
        DynIndex {
            tables: (0..CHUNKS)
                .map(|_| vec![Vec::new(); BUCKETS].into_boxed_slice())
                .collect(),
            len: 0,
            dead: HashSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// About how many bytes the index holds: the bucket tables, and each entry in every
    /// chunk's table with room to grow.
    pub fn approx_bytes(&self) -> u64 {
        let entry = std::mem::size_of::<(u32, u64)>();
        (CHUNKS * BUCKETS * std::mem::size_of::<Bucket>()
            + (self.len + self.dead.len()) * CHUNKS * entry * 3 / 2
            + self.dead.len() * 32) as u64
    }

    /// Adds `h` under `id` (an id already present is added again: remove it first).
    pub fn insert(&mut self, id: u32, h: u64) {
        self.len += 1;
        // Taken out and back again before the sweep: its entries are still there.
        if self.dead.remove(&(id, h)) {
            return;
        }
        for (c, t) in self.tables.iter_mut().enumerate() {
            t[chunk(h, c)].push((id, h));
        }
    }

    /// Takes out `id`, added with hash `h`. False when it was not there (from a crowded
    /// bucket it is taken to be there).
    pub fn remove(&mut self, id: u32, h: u64) -> bool {
        if self.dead.contains(&(id, h)) {
            return false;
        }
        let crowded = self
            .tables
            .iter()
            .enumerate()
            .any(|(c, t)| t[chunk(h, c)].len() > CROWDED);
        if crowded {
            // Not looked for (that is the cost avoided): the caller takes out only what it
            // put in.
            self.dead.insert((id, h));
            self.len -= 1;
            if self.dead.len() > 1024 && self.dead.len() > self.len / 2 {
                self.sweep();
            }
            return true;
        }
        let mut found = false;
        for (c, t) in self.tables.iter_mut().enumerate() {
            let b = &mut t[chunk(h, c)];
            if let Some(i) = b.iter().position(|e| *e == (id, h)) {
                b.swap_remove(i);
                found = true;
            }
        }
        if found {
            self.len -= 1;
        }
        found
    }

    /// Drops the entries marked dead from every bucket.
    fn sweep(&mut self) {
        let dead = std::mem::take(&mut self.dead);
        for t in &mut self.tables {
            for b in t.iter_mut() {
                if !b.is_empty() {
                    b.retain(|e| !dead.contains(e));
                }
            }
        }
    }

    /// Ids within distance `r` (≤ 15) of `q`, with their distances, sorted by id.
    pub fn within(&self, q: u64, r: u32) -> Vec<(u32, u32)> {
        let mut out = self.scan(q, r, usize::MAX);
        out.sort_unstable();
        out
    }

    /// At most `limit` of the ids within distance `r` (≤ 15) of `q`, nearest first (then by
    /// id). Once `limit` ids at distance 0 are found the rest is not looked at: thousands of
    /// copies of one image cost `limit` per look, not thousands.
    pub fn nearest(&self, q: u64, r: u32, limit: usize) -> Vec<(u32, u32)> {
        let mut out = self.scan(q, r, limit);
        out.sort_unstable_by_key(|&(id, d)| (d, id));
        out.truncate(limit);
        out
    }

    /// The ids within `r` of `q`, stopping once `exact` of them are at distance 0.
    fn scan(&self, q: u64, r: u32, exact: usize) -> Vec<(u32, u32)> {
        let r = r.min(15);
        let k = r / CHUNKS as u32;
        let ms = masks(k);
        let mut out: Vec<(u32, u32)> = Vec::new();
        let mut zeros = 0;
        for (c, t) in self.tables.iter().enumerate() {
            let qc = chunk(q, c) as u16;
            for &m in &ms {
                for &(id, v) in &t[(qc ^ m) as usize] {
                    let d = (q ^ v).count_ones();
                    if d <= r
                        && first_close_chunk(q, v, k) == c
                        && (self.dead.is_empty() || !self.dead.contains(&(id, v)))
                    {
                        out.push((id, d));
                        zeros += usize::from(d == 0);
                        if zeros >= exact {
                            return out;
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(s: &mut u64) -> u64 {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *s ^ (*s >> 29)
    }

    #[test]
    fn matches_brute_force() {
        let mut s = 7u64;
        let mut hs: Vec<u64> = (0..800).map(|_| lcg(&mut s)).collect();
        // Plant near neighbours.
        for k in 0..200 {
            let base = hs[k];
            let mut v = base;
            for b in 0..(k % 12) {
                v ^= 1 << ((b * 7 + k) % 64);
            }
            hs.push(v);
        }
        let idx = HashIndex::new(hs.clone());
        for r in [3u32, 7, 11] {
            let mut got = idx.pairs(r);
            got.sort_unstable();
            let mut want = Vec::new();
            for i in 0..hs.len() {
                for j in i + 1..hs.len() {
                    let d = (hs[i] ^ hs[j]).count_ones();
                    if d <= r {
                        want.push((i as u32, j as u32, d));
                    }
                }
            }
            assert_eq!(got, want, "radius {r}");
        }
    }

    #[test]
    fn dynamic_index_matches_brute_force() {
        let mut s = 11u64;
        let mut live: Vec<Option<u64>> = Vec::new();
        let mut idx = DynIndex::new();
        for step in 0..3000u32 {
            let roll = lcg(&mut s) % 10;
            if roll < 7 || live.iter().all(Option::is_none) {
                // New, often near an earlier one.
                let h = match live.iter().flatten().nth((lcg(&mut s) % 50) as usize) {
                    Some(&b) if roll < 4 => b ^ (1 << (lcg(&mut s) % 64)) ^ (1 << (step % 64)),
                    _ => lcg(&mut s),
                };
                idx.insert(live.len() as u32, h);
                live.push(Some(h));
            } else {
                let i = (lcg(&mut s) % live.len() as u64) as usize;
                if let Some(h) = live[i].take() {
                    assert!(idx.remove(i as u32, h));
                }
            }
        }
        assert_eq!(idx.len(), live.iter().flatten().count());
        for (qi, q) in live
            .iter()
            .enumerate()
            .filter_map(|(i, h)| h.map(|h| (i, h)))
            .take(300)
        {
            for r in [7u32, 11] {
                let want: Vec<(u32, u32)> = live
                    .iter()
                    .enumerate()
                    .filter_map(|(i, h)| h.map(|h| (i as u32, (h ^ q).count_ones())))
                    .filter(|&(_, d)| d <= r)
                    .collect();
                assert_eq!(idx.within(q, r), want, "query {qi}, radius {r}");
            }
        }
    }

    /// Thousands of equal hashes (copies of one image): `nearest` stops early, removals
    /// are marked and swept, and the answers stay those of a brute-force search.
    #[test]
    fn crowded_buckets_stay_exact() {
        let mut s = 5u64;
        let mut idx = DynIndex::new();
        let copy = lcg(&mut s);
        let mut live: Vec<Option<u64>> = Vec::new();
        for i in 0..6000u32 {
            let h = if i % 3 == 0 { lcg(&mut s) } else { copy };
            idx.insert(i, h);
            live.push(Some(h));
        }
        let near = idx.nearest(copy, 7, 8);
        assert_eq!(near.len(), 8);
        assert!(near.iter().all(|&(_, d)| d == 0));
        // Most copies go (marked, then swept), some come back.
        for (i, h) in live.iter_mut().enumerate() {
            if i % 3 != 0 && i % 5 != 0 {
                assert!(idx.remove(i as u32, h.take().unwrap()));
            }
        }
        for i in (1..6000u32).step_by(97) {
            if live[i as usize].is_none() {
                idx.insert(i, copy);
                live[i as usize] = Some(copy);
            }
        }
        assert_eq!(idx.len(), live.iter().flatten().count());
        for q in [copy, copy ^ 0b1011, live[0].unwrap()] {
            let want: Vec<(u32, u32)> = live
                .iter()
                .enumerate()
                .filter_map(|(i, h)| h.map(|h| (i as u32, (h ^ q).count_ones())))
                .filter(|&(_, d)| d <= 7)
                .collect();
            assert_eq!(idx.within(q, 7), want);
            let mut by_d = want.clone();
            by_d.sort_unstable_by_key(|&(id, d)| (d, id));
            let got = idx.nearest(q, 7, 5);
            assert_eq!(got.len(), by_d.len().min(5));
            // The nearest distances (which ids among equals is free).
            let d = |v: &[(u32, u32)]| v.iter().map(|x| x.1).collect::<Vec<_>>();
            assert_eq!(d(&got), d(&by_d[..got.len()]));
        }
    }
}
