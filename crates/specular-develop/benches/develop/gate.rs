//! The Develop performance gate's results, baseline and +10 % rule.
//!
//! Shared by the bench (`benches/develop/main.rs`) and its tests (`tests/perf_gate.rs`), so
//! the rule the gate applies is the rule the tests check. Standard library and serde only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The results file's format version.
pub const SCHEMA: u32 = 1;
/// A case fails when its p50 or p95 is more than this much above the baseline's.
pub const TOLERANCE: f64 = 0.10;
/// The p95 may also be this many milliseconds over (scheduling noise in the tail of
/// millisecond cases); the p50 has no slack.
pub const SLACK_MS: f64 = 0.5;

/// The committed baseline: the hardware adapter's, or the software adapter's.
pub fn baseline_path(software: bool) -> PathBuf {
    let name = if software {
        "develop-baseline-software.json"
    } else {
        "develop-baseline.json"
    };
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join(name)
}

/// 1, 5 and 15 minute load averages (zeros where the system has none).
pub fn load() -> [f64; 3] {
    let s = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let mut it = s.split_whitespace().map(|v| v.parse().unwrap_or(0.0));
    [0; 3].map(|_| it.next().unwrap_or(0.0))
}

/// Why timings taken now would not be comparable (`None`: the machine is quiet enough).
pub fn overloaded() -> Option<String> {
    let (load, cores) = specular_log::timing::overloaded()?;
    Some(format!(
        "the machine is overloaded (1-minute load {load:.1} above its {cores} cores)"
    ))
}

/// What a case's numbers measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Wall clock on the host, to the GPU being done (or the file written).
    Wall,
    /// GPU timestamp queries: the sum of the dispatches' times.
    Gpu,
}

/// Percentiles of one run of samples, in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stat {
    pub p50: f64,
    pub p95: f64,
    pub best: f64,
    pub n: usize,
}

impl Stat {
    /// `samples` (any order); `None` when empty.
    pub fn of(samples: &[f64]) -> Option<Stat> {
        let mut s: Vec<f64> = samples.iter().copied().filter(|v| v.is_finite()).collect();
        if s.is_empty() {
            return None;
        }
        s.sort_by(f64::total_cmp);
        Some(Stat {
            p50: percentile(&s, 0.50),
            p95: percentile(&s, 0.95),
            best: s[0],
            n: s.len(),
        })
    }

    fn scaled(self, f: f64) -> Stat {
        Stat {
            p50: self.p50 * f,
            p95: self.p95 * f,
            best: self.best * f,
            n: self.n,
        }
    }
}

/// Linear interpolation between closest ranks of sorted `s`.
pub fn percentile(s: &[f64], q: f64) -> f64 {
    let pos = q * (s.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    s[lo] + (s[hi] - s[lo]) * (pos - lo as f64)
}

/// A §3.9 ceiling for one case (milliseconds, of `on`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub p50: Option<f64>,
    pub p95: Option<f64>,
    /// A second p50 ceiling on the GPU time (the point-op row: 8 ms GPU, 16 ms to present).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_p50: Option<f64>,
}

/// One case's numbers: the round with the lowest p50 of the gated metric (best of N).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaseResult {
    /// The §3.9 row it belongs to.
    pub family: String,
    pub wall: Stat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<Stat>,
    /// What the +10 % rule compares.
    pub gated: Metric,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    /// Rounds run (the best is kept).
    pub rounds: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl CaseResult {
    /// The gated numbers (the wall clock when the GPU time is missing).
    pub fn gated_stat(&self) -> Stat {
        match (self.gated, self.gpu) {
            (Metric::Gpu, Some(g)) => g,
            _ => self.wall,
        }
    }

    /// Ceilings this result is over, as text (empty: within, or no budget).
    pub fn over_budget(&self) -> Vec<String> {
        let Some(b) = self.budget else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let w = self.wall;
        if let Some(l) = b.p50
            && w.p50 > l
        {
            out.push(format!("p50 {:.1} > {l:.0} ms", w.p50));
        }
        if let Some(l) = b.p95
            && w.p95 > l
        {
            out.push(format!("p95 {:.1} > {l:.0} ms", w.p95));
        }
        if let (Some(l), Some(g)) = (b.gpu_p50, self.gpu)
            && g.p50 > l
        {
            out.push(format!("GPU p50 {:.1} > {l:.0} ms", g.p50));
        }
        out
    }
}

/// The machine a run was made on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Env {
    /// Adapter name and backend, e.g. "AMD Radeon 860M Graphics (RADV GFX1152) (Vulkan)".
    pub adapter: String,
    /// "gpu" or "software" (lavapipe, WARP): compared only with the same class.
    pub class: String,
    /// Cargo profile of the bench: "dev" or "release".
    pub profile: String,
    pub timestamps: bool,
    /// 1, 5 and 15 minute load averages at the start and the end.
    pub load_start: [f64; 3],
    pub load_end: [f64; 3],
    pub cores: usize,
    /// Seconds since 1970.
    pub unix_time: u64,
}

/// A bench run, or the blessed baseline (same format).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Results {
    pub schema: u32,
    /// Blessed under load (not from an idle run): numbers to replace by an idle bless.
    #[serde(default)]
    pub provisional: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub env: Env,
    pub cases: BTreeMap<String, CaseResult>,
}

impl Results {
    pub fn read(path: &Path) -> Result<Results, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let r: Results =
            serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))?;
        if r.schema != SCHEMA {
            return Err(format!(
                "{}: schema {} (this gate reads {SCHEMA})",
                path.display(),
                r.schema
            ));
        }
        Ok(r)
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let mut s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        s.push('\n');
        std::fs::write(path, s).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Every time multiplied by `f` (tests: a synthetic regression).
    pub fn scaled(&self, f: f64) -> Results {
        let mut r = self.clone();
        for c in r.cases.values_mut() {
            c.wall = c.wall.scaled(f);
            c.gpu = c.gpu.map(|g| g.scaled(f));
        }
        r
    }

    /// The baseline to bless from this run and an older provisional `base`: each case's
    /// better (lower gated p50) result, so a baseline made under load keeps the best runs.
    pub fn best_with(&self, base: &Results) -> Results {
        let mut r = self.clone();
        if base.env.class != self.env.class || base.env.profile != self.env.profile {
            return r;
        }
        for (k, b) in &base.cases {
            match r.cases.get(k) {
                Some(c) if c.gated_stat().p50 <= b.gated_stat().p50 => {}
                // Keep the case's own budget and family from this run.
                Some(c) => {
                    let mut b = b.clone();
                    b.budget = c.budget;
                    b.family = c.family.clone();
                    r.cases.insert(k.clone(), b);
                }
                None => {
                    r.cases.insert(k.clone(), b.clone());
                }
            }
        }
        r
    }
}

/// What the gate decided.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Every compared case within the rule; the strings are notes (new cases, missing ones).
    Pass { compared: usize, notes: Vec<String> },
    /// The cases over the rule.
    Fail(Vec<String>),
    /// Not compared, and why. Not a pass.
    Skip(String),
}

/// The +10 % rule: each case of `now` that `base` also has must keep its gated p50 and p95
/// within `base × (1 + TOLERANCE)` (the p95 plus `SLACK_MS`).
pub fn compare(base: &Results, now: &Results) -> Verdict {
    if base.env.class != now.env.class {
        return Verdict::Skip(format!(
            "the baseline is for a {} adapter, this run used a {} one ({})",
            base.env.class, now.env.class, now.env.adapter
        ));
    }
    if base.env.profile != now.env.profile {
        return Verdict::Skip(format!(
            "the baseline was made with the {} profile, this run with {}",
            base.env.profile, now.env.profile
        ));
    }
    if base.env.adapter != now.env.adapter {
        return Verdict::Skip(format!(
            "the baseline was made on {}, this run on {}: bless on this machine to gate here",
            base.env.adapter, now.env.adapter
        ));
    }
    let mut fails = Vec::new();
    let mut notes = Vec::new();
    let mut compared = 0;
    for (k, c) in &now.cases {
        let Some(b) = base.cases.get(k) else {
            notes.push(format!("{k}: new, not in the baseline"));
            continue;
        };
        if b.gated != c.gated || (c.gated == Metric::Gpu && (b.gpu.is_none() || c.gpu.is_none())) {
            notes.push(format!(
                "{k}: measured differently from the baseline, not compared"
            ));
            continue;
        }
        compared += 1;
        let (bs, cs) = (b.gated_stat(), c.gated_stat());
        for (name, was, is) in [("p50", bs.p50, cs.p50), ("p95", bs.p95, cs.p95)] {
            let slack = if name == "p95" { SLACK_MS } else { 0.0 };
            let limit = was * (1.0 + TOLERANCE) + slack;
            if is > limit {
                fails.push(format!(
                    "{k}: {name} {is:.2} ms > {limit:.2} (baseline {was:.2} + {:.0} %, {:?})",
                    TOLERANCE * 100.0,
                    c.gated
                ));
            }
        }
    }
    for k in base.cases.keys() {
        if !now.cases.contains_key(k) {
            notes.push(format!("{k}: in the baseline, not run"));
        }
    }
    if fails.is_empty() {
        Verdict::Pass { compared, notes }
    } else {
        Verdict::Fail(fails)
    }
}

/// The results as a Markdown table: every case with its budget and whether it is over.
pub fn table(r: &Results) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "Develop bench on {} [{}], {} profile, timestamps {}, load {:.1} → {:.1} on {} cores\n",
        r.env.adapter,
        r.env.class,
        r.env.profile,
        r.env.timestamps,
        r.env.load_start[0],
        r.env.load_end[0],
        r.env.cores
    );
    let _ = writeln!(
        s,
        "| Case | Wall p50 | Wall p95 | GPU p50 | GPU p95 | Budget p50 / p95 | |\n|---|---:|---:|---:|---:|---|---|"
    );
    let f = |v: Option<f64>| v.map_or("—".to_owned(), |v| format!("{v:.1}"));
    for (k, c) in &r.cases {
        let budget = c.budget.map_or("—".to_owned(), |b| {
            let mut t = format!("{} / {}", f(b.p50), f(b.p95));
            if let Some(g) = b.gpu_p50 {
                let _ = write!(t, " (GPU {g})");
            }
            t
        });
        let over = c.over_budget();
        let _ = writeln!(
            s,
            "| {k} | {:.1} | {:.1} | {} | {} | {budget} | {} |",
            c.wall.p50,
            c.wall.p95,
            f(c.gpu.map(|g| g.p50)),
            f(c.gpu.map(|g| g.p95)),
            if over.is_empty() {
                String::new()
            } else {
                format!("**over** ({})", over.join(", "))
            }
        );
    }
    s
}
