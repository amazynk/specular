//! Grouping face embeddings into people: average-linkage agglomerative clustering on cosine
//! similarity, cut at a threshold.
//!
//! The dendrogram is built with the nearest-neighbour chain (average linkage is reducible,
//! so the chain gives the same tree as merging the closest pair each time) in O(n²) time and
//! n² memory; cutting it at `threshold` keeps the merges at or above it. Inputs larger than
//! [`CHUNK`] are clustered in chunks whose clusters are then clustered again by their mean
//! embedding (weighted by size).

/// The most embeddings clustered in one pass (the similarity matrix is n² f32).
pub const CHUNK: usize = 3000;

/// A cluster label per embedding (0, 1, …, in order of first appearance). Embeddings are
/// unit vectors of one length.
pub fn agglomerate(vecs: &[Vec<f32>], threshold: f32) -> Vec<usize> {
    let weights = vec![1.0f32; vecs.len()];
    relabel(&cluster(vecs, &weights, threshold))
}

fn cluster(vecs: &[Vec<f32>], weights: &[f32], threshold: f32) -> Vec<usize> {
    if vecs.len() <= CHUNK {
        return nn_chain(vecs, weights, threshold);
    }
    // Cluster each chunk, then the chunk clusters' means.
    let mut labels = vec![0usize; vecs.len()];
    let mut means: Vec<Vec<f32>> = Vec::new();
    let mut mass: Vec<f32> = Vec::new();
    for (ci, chunk) in vecs.chunks(CHUNK).enumerate() {
        let base = ci * CHUNK;
        let l = nn_chain(chunk, &weights[base..base + chunk.len()], threshold);
        let k = l.iter().max().map_or(0, |m| m + 1);
        let first = means.len();
        means.extend((0..k).map(|_| vec![0f32; chunk[0].len()]));
        mass.extend((0..k).map(|_| 0f32));
        for (i, &c) in l.iter().enumerate() {
            let w = weights[base + i];
            for (m, x) in means[first + c].iter_mut().zip(&chunk[i]) {
                *m += w * x;
            }
            mass[first + c] += w;
            labels[base + i] = first + c;
        }
    }
    for m in &mut means {
        super::normalize(m);
    }
    if means.len() >= vecs.len() {
        // Nothing merged: no progress possible.
        return labels;
    }
    let top = cluster(&means, &mass, threshold);
    labels.iter().map(|&c| top[c]).collect()
}

/// Labels 0.. in order of first appearance.
fn relabel(l: &[usize]) -> Vec<usize> {
    let mut map = std::collections::HashMap::new();
    l.iter()
        .map(|&c| {
            let n = map.len();
            *map.entry(c).or_insert(n)
        })
        .collect()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn nn_chain(vecs: &[Vec<f32>], weights: &[f32], threshold: f32) -> Vec<usize> {
    let n = vecs.len();
    if n < 2 {
        return vec![0; n];
    }
    // Similarity matrix (full, for simple indexing).
    let mut s = vec![0f32; n * n];
    for i in 0..n {
        for j in i + 1..n {
            let v = dot(&vecs[i], &vecs[j]);
            s[i * n + j] = v;
            s[j * n + i] = v;
        }
    }
    let mut size: Vec<f32> = weights.to_vec();
    let mut active = vec![true; n];
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut chain: Vec<usize> = Vec::new();
    let mut left = n;
    while left > 1 {
        if chain.is_empty() {
            let Some(first) = (0..n).find(|&i| active[i]) else {
                break;
            };
            chain.push(first);
        }
        let a = *chain.last().expect("non-empty");
        let prev = chain.len().checked_sub(2).map(|i| chain[i]);
        // Nearest active neighbour of a (ties prefer the previous in the chain).
        let mut best = None::<(usize, f32)>;
        for j in 0..n {
            if j == a || !active[j] {
                continue;
            }
            let v = s[a * n + j];
            let better = match best {
                None => true,
                Some((bj, bv)) => v > bv || (v == bv && Some(j) == prev && Some(bj) != prev),
            };
            if better {
                best = Some((j, v));
            }
        }
        let Some((b, sim)) = best else { break };
        if Some(b) != prev {
            chain.push(b);
            continue;
        }
        // a and b are mutual nearest neighbours: merge b into a.
        chain.pop();
        chain.pop();
        if sim >= threshold {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            parent[rb] = ra;
        }
        let (wa, wb) = (size[a], size[b]);
        for k in 0..n {
            if active[k] && k != a && k != b {
                let v = (wa * s[a * n + k] + wb * s[b * n + k]) / (wa + wb);
                s[a * n + k] = v;
                s[k * n + a] = v;
            }
        }
        size[a] = wa + wb;
        active[b] = false;
        left -= 1;
    }
    let roots: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    relabel(&roots)
}

/// Pairwise precision and recall of `got` against `truth` labels: of the pairs put together,
/// how many belong together; of the pairs that belong together, how many were put together.
pub fn pairwise(got: &[usize], truth: &[usize]) -> (f64, f64) {
    let (mut tp, mut fp, mut fneg) = (0u64, 0u64, 0u64);
    for i in 0..got.len() {
        for j in i + 1..got.len() {
            match (got[i] == got[j], truth[i] == truth[j]) {
                (true, true) => tp += 1,
                (true, false) => fp += 1,
                (false, true) => fneg += 1,
                _ => {}
            }
        }
    }
    let p = if tp + fp == 0 {
        1.0
    } else {
        tp as f64 / (tp + fp) as f64
    };
    let r = if tp + fneg == 0 {
        1.0
    } else {
        tp as f64 / (tp + fneg) as f64
    };
    (p, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tensor::Rng;

    fn unit(v: Vec<f32>) -> Vec<f32> {
        let mut v = v;
        super::super::normalize(&mut v);
        v
    }

    /// `people` random centres with `each` noisy copies.
    fn blobs(people: usize, each: usize, noise: f32, seed: u64) -> (Vec<Vec<f32>>, Vec<usize>) {
        let mut r = Rng::new(seed);
        let centres: Vec<Vec<f32>> = (0..people)
            .map(|_| unit((0..64).map(|_| r.sym(1.0)).collect()))
            .collect();
        let mut v = Vec::new();
        let mut t = Vec::new();
        for k in 0..each {
            for (p, c) in centres.iter().enumerate() {
                let _ = k;
                v.push(unit(c.iter().map(|x| x + r.sym(noise)).collect()));
                t.push(p);
            }
        }
        (v, t)
    }

    #[test]
    fn separated_people_are_found_exactly() {
        let (v, t) = blobs(6, 5, 0.05, 1);
        let l = agglomerate(&v, 0.5);
        assert_eq!(pairwise(&l, &t), (1.0, 1.0));
        assert_eq!(l.iter().max(), Some(&5));
    }

    #[test]
    fn a_high_threshold_keeps_everyone_apart() {
        let (v, _) = blobs(3, 3, 0.2, 2);
        let l = agglomerate(&v, 0.9999);
        assert_eq!(l, (0..9).collect::<Vec<_>>());
        assert_eq!(agglomerate(&[], 0.5), Vec::<usize>::new());
        assert_eq!(agglomerate(&v[..1], 0.5), vec![0]);
    }

    #[test]
    fn chunked_clustering_agrees_with_one_pass() {
        let (v, t) = blobs(40, 80, 0.05, 3);
        assert!(v.len() > CHUNK);
        let l = agglomerate(&v, 0.5);
        let (p, r) = pairwise(&l, &t);
        assert!(p > 0.999 && r > 0.999, "{p} {r}");
    }

    #[test]
    fn pairwise_counts() {
        assert_eq!(pairwise(&[0, 0, 1], &[0, 0, 1]), (1.0, 1.0));
        let (p, r) = pairwise(&[0, 0, 0], &[0, 0, 1]);
        assert!((p - 1.0 / 3.0).abs() < 1e-9 && r == 1.0);
    }
}
