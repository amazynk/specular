//! An in-memory least-recently-used cache bounded by total weight (bytes).

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

/// LRU map bounded by the sum of entry weights. Not thread-safe; wrap in a mutex.
#[derive(Debug)]
pub struct MemLru<K, V> {
    budget: u64,
    used: u64,
    tick: u64,
    map: HashMap<K, (V, u64, u64)>,
    order: BTreeMap<u64, K>,
}

impl<K: Hash + Eq + Clone, V: Clone> MemLru<K, V> {
    pub fn new(budget: u64) -> Self {
        MemLru {
            budget,
            used: 0,
            tick: 0,
            map: HashMap::new(),
            order: BTreeMap::new(),
        }
    }

    /// The value for `k`, marking it most recently used.
    pub fn get(&mut self, k: &K) -> Option<V> {
        let (v, _, t) = self.map.get_mut(k)?;
        self.order.remove(t);
        self.tick += 1;
        *t = self.tick;
        self.order.insert(self.tick, k.clone());
        Some(v.clone())
    }

    pub fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }

    /// Inserts (replacing any old value) and evicts the least recently used entries until
    /// the total fits. An entry heavier than the whole budget is not kept.
    pub fn insert(&mut self, k: K, v: V, weight: u64) {
        self.remove(&k);
        if weight > self.budget {
            return;
        }
        self.tick += 1;
        self.order.insert(self.tick, k.clone());
        self.map.insert(k, (v, weight, self.tick));
        self.used += weight;
        while self.used > self.budget {
            let Some((_, old)) = self.order.pop_first() else {
                break;
            };
            if let Some((_, w, _)) = self.map.remove(&old) {
                self.used -= w;
            }
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let (v, w, t) = self.map.remove(k)?;
        self.order.remove(&t);
        self.used -= w;
        Some(v)
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.used = 0;
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    /// Total weight held.
    pub fn used(&self) -> u64 {
        self.used
    }
}

#[cfg(test)]
mod tests {
    use super::MemLru;

    #[test]
    fn evicts_least_recently_used_by_weight() {
        let mut l = MemLru::new(10);
        l.insert("a", 1, 4);
        l.insert("b", 2, 4);
        assert_eq!(l.get(&"a"), Some(1)); // a is now newer than b
        l.insert("c", 3, 4); // 12 > 10: b goes
        assert!(!l.contains(&"b"));
        assert!(l.contains(&"a") && l.contains(&"c"));
        assert_eq!(l.used(), 8);
        l.insert("big", 9, 11); // heavier than the budget: not kept, nothing evicted
        assert!(!l.contains(&"big"));
        assert_eq!(l.len(), 2);
        l.insert("a", 5, 2); // replacing updates the weight
        assert_eq!(l.used(), 6);
        assert_eq!(l.remove(&"c"), Some(3));
        assert_eq!(l.used(), 2);
    }
}
