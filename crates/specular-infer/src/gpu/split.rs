//! Splitting a run into short queue submissions.
//!
//! Drivers reset a device whose submission runs for seconds (amdgpu and i915 after 2–10 s,
//! Windows TDR after 2 s), sooner when other work shares the GPU; Big-LaMa on a large crop
//! held the GPU ≈ 2 s in one submission. A run now goes in submissions of at most
//! [`SUBMIT_MS`] estimated GPU milliseconds, the dispatches in the same order (results bit
//! for bit the same).
//!
//! Estimates: per dispatch, learnt milliseconds by its label (op and shape, shared by every
//! graph on the context); a dispatch not seen yet is its node's work (multiply-adds, plus
//! [`ELEMENT_WORK`] per value read or written) times nanoseconds per unit of work, itself
//! learnt. Each submission's time is read back from when it finishes (the submissions go
//! up together, so finishing times one after the other are each one's own time); a
//! slower reading than estimated scales the estimates of its dispatches up at once, a
//! faster one down slowly. The app's device has no timestamp queries, so learning uses
//! finishing times only.

use std::collections::HashMap;

use crate::graph::{Dft, Graph, Node, Op};
use crate::tensor::numel;

/// Estimated GPU milliseconds one queue submission may hold (Develop's `SUBMIT_MS`).
pub const SUBMIT_MS: f64 = 150.0;

/// Work units per value a dispatch reads or writes, against 1 per multiply-add: memory
/// bound ops cost about this much more per value than a product's multiply-add (measured
/// on a Radeon 860M, 2026-11-02: LSTM steps 0.03 ns per multiply-add, a transpose 1.1 ns per
/// value).
pub const ELEMENT_WORK: f64 = 16.0;

/// Nanoseconds per work unit before any reading: about twice the slowest kernel measured
/// on an integrated GPU, so the first run of a new graph errs on short submissions.
pub const DEFAULT_NS_PER_WORK: f64 = 0.06;

/// Milliseconds every dispatch costs on top of its work (launch and barrier).
const DISPATCH_MS: f64 = 0.01;

/// Submissions shorter than this (measured and estimated) teach nothing: wake-up latency
/// would dominate.
const LEARN_MIN_MS: f64 = 2.0;

/// What a context has learnt about its kernels' speed.
#[derive(Debug)]
pub(crate) struct Speeds {
    /// Milliseconds per dispatch, by label.
    pub ms: HashMap<String, f64>,
    /// Nanoseconds per work unit for labels not learnt yet.
    pub ns_per_work: f64,
}

impl Default for Speeds {
    fn default() -> Speeds {
        Speeds {
            ms: HashMap::new(),
            ns_per_work: DEFAULT_NS_PER_WORK,
        }
    }
}

impl Speeds {
    /// Estimated milliseconds of a dispatch, and whether it was learnt.
    pub fn estimate(&self, label: &str, work: f64) -> (f64, bool) {
        match self.ms.get(label) {
            Some(&ms) => (ms, true),
            None => (work * self.ns_per_work * 1e-6 + DISPATCH_MS, false),
        }
    }

    /// Takes profiled dispatch times (label, ms) in: per label the slowest reading, up at
    /// once, down slowly.
    pub fn measured(&mut self, times: &[(String, f64)]) {
        let mut slowest: HashMap<&str, f64> = HashMap::new();
        for (label, ms) in times {
            let e = slowest.entry(label.as_str()).or_insert(0.0);
            *e = e.max(*ms);
        }
        for (label, ms) in slowest {
            if !ms.is_finite() {
                continue;
            }
            let e = self.ms.entry(label.to_owned()).or_insert(ms);
            *e = if ms > *e { ms } else { 0.875 * *e + 0.125 * ms };
        }
    }

    /// Takes one submission's measured time (`measured` ms) into the estimates of its
    /// dispatches (`(label, work)`).
    pub fn learn<'a>(&mut self, steps: impl Iterator<Item = (&'a str, f64)>, measured: f64) {
        let steps: Vec<(&str, f64)> = steps.collect();
        let est: Vec<(f64, bool)> = steps.iter().map(|(l, w)| self.estimate(l, *w)).collect();
        let total: f64 = est.iter().map(|e| e.0).sum();
        if !measured.is_finite() || total <= 0.0 || measured.max(total) < LEARN_MIN_MS {
            return;
        }
        let r = measured / total;
        let scale = |x: f64| {
            if r > 1.0 {
                x * r
            } else {
                x * (0.875 + 0.125 * r)
            }
        };
        // The unseen share of the estimate also moves the rate for unseen dispatches.
        let unseen: f64 = est.iter().filter(|e| !e.1).map(|e| e.0).sum();
        if unseen > 0.5 * total {
            self.ns_per_work = scale(self.ns_per_work);
        }
        for ((label, _), (ms, _)) in steps.iter().zip(est) {
            self.ms.insert((*label).to_owned(), scale(ms));
        }
    }
}

/// Splits dispatches of estimated milliseconds `ms` into runs of at most `limit` (a
/// dispatch over the limit alone), in order.
pub(crate) fn submissions(ms: &[f64], limit: f64) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let (mut start, mut sum) = (0, 0.0);
    for (i, &t) in ms.iter().enumerate() {
        if i > start && sum + t > limit {
            out.push(start..i);
            start = i;
            sum = 0.0;
        }
        sum += t;
    }
    if start < ms.len() || out.is_empty() {
        out.push(start..ms.len());
    }
    out
}

/// Work units of a node: its multiply-adds plus [`ELEMENT_WORK`] per value read and
/// written.
pub(crate) fn node_work(g: &Graph, node: &Node) -> f64 {
    let n = |v| numel(g.shape(v)).unwrap_or(0) as f64;
    let out = n(node.output);
    let ins: f64 = node.inputs.iter().map(|&v| n(v)).sum();
    let shape = |k: usize| g.shape(node.inputs[k]);
    let macs = match &node.op {
        Op::Conv2d(_) => {
            let w = shape(1);
            // w is [O, C / groups, KH, KW]: each output sums C / groups · KH · KW products.
            out * (w[1] * w[2] * w[3]) as f64
        }
        Op::ConvTranspose2d { stride, .. } => {
            let w = shape(1);
            out * (w[0] * w[2] * w[3]) as f64 / (stride[0] * stride[1]).max(1) as f64
        }
        Op::MatMul(m) => {
            let b = shape(1);
            let k = if m.trans_b {
                b[b.len() - 1]
            } else {
                b[b.len().saturating_sub(2)]
            };
            out * k as f64
        }
        Op::Attention { packed, .. } => {
            let q = shape(0);
            let (t, d) = (q[1], q[2] / if *packed { 3 } else { 1 });
            let tk = if *packed { t } else { shape(1)[1] };
            2.0 * (q[0] * t * tk * d) as f64
        }
        Op::Lstm { .. } => {
            let (p, wh) = (shape(0), shape(1));
            (p[0] * p[1]) as f64 * (wh[0] * wh[1]) as f64
        }
        Op::Dft(d) => {
            let len = match d {
                Dft::RealToComplex => *shape(0).last().unwrap_or(&1),
                Dft::Complex { .. } => shape(0)[shape(0).len().saturating_sub(3)],
                Dft::ComplexToReal { n } => *n,
            };
            out * len as f64
        }
        Op::Pool2d(p) => out * (p.kernel[0] * p.kernel[1]) as f64,
        _ => 0.0,
    };
    macs + ELEMENT_WORK * (out + ins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submissions_keep_order_and_limit() {
        let ms = [40.0, 60.0, 70.0, 200.0, 10.0, 10.0];
        assert_eq!(submissions(&ms, 150.0), vec![0..2, 2..3, 3..4, 4..6]);
        assert_eq!(submissions(&ms, f64::INFINITY), vec![0..6]);
        assert_eq!(submissions(&ms, 0.0).len(), 6);
        assert_eq!(submissions(&[], 150.0), vec![0..0]);
    }

    #[test]
    fn learning_goes_up_at_once_and_down_slowly() {
        let mut s = Speeds::default();
        let steps = || [("a", 1e9), ("b", 1e9)].into_iter();
        let (e, seen) = s.estimate("a", 1e9);
        assert!(!seen);
        s.learn(steps(), 4.0 * e);
        assert!((s.estimate("a", 0.0).0 - 2.0 * e).abs() < 1e-9);
        assert!(s.ns_per_work > DEFAULT_NS_PER_WORK);
        s.learn(steps(), 0.0);
        assert!((s.estimate("a", 0.0).0 - 2.0 * e * 0.875).abs() < 1e-9);
        // Too short to teach anything.
        let mut s = Speeds::default();
        s.learn([("c", 1.0)].into_iter(), 0.5 * LEARN_MIN_MS);
        assert!(s.ms.is_empty());
    }
}
