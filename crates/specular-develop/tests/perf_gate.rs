//! The Develop performance gate's rule (benches/develop): the committed baseline
//! passes against itself, within 10 % passes, and a synthetic +20 % regression fails; the
//! gate is skipped, not passed, across adapters and profiles. And the last bench run (when
//! there is one) is gated against the baseline, skipped when the machine is overloaded.
//!
//! Running the bench and the gate: see `benches/develop/main.rs`.

#[path = "../benches/develop/gate.rs"]
#[allow(dead_code)]
mod gate;

use gate::{Results, Verdict};

fn baseline() -> Results {
    Results::read(&gate::baseline_path(false)).expect("the committed baseline reads")
}

#[test]
fn the_baseline_passes_against_itself() {
    let b = baseline();
    assert!(!b.cases.is_empty());
    match gate::compare(&b, &b) {
        Verdict::Pass { compared, .. } => assert_eq!(compared, b.cases.len()),
        v => panic!("{v:?}"),
    }
}

#[test]
fn ten_percent_slower_passes_and_twenty_fails() {
    let b = baseline();
    assert!(matches!(
        gate::compare(&b, &b.scaled(1.09)),
        Verdict::Pass { .. }
    ));
    match gate::compare(&b, &b.scaled(1.2)) {
        // Every case fails at least its p50 (no slack there).
        Verdict::Fail(lines) => {
            let p50 = lines.iter().filter(|l| l.contains(": p50 ")).count();
            assert_eq!(p50, b.cases.len(), "{lines:#?}");
        }
        v => panic!("{v:?}"),
    }
    // One case alone regressing fails the gate and names it.
    let mut one = b.clone();
    let (k, c) = one.cases.iter_mut().next().unwrap();
    let k = k.clone();
    c.wall.p50 *= 1.2;
    c.wall.p95 *= 1.2;
    if let Some(g) = &mut c.gpu {
        g.p50 *= 1.2;
        g.p95 *= 1.2;
    }
    match gate::compare(&b, &one) {
        Verdict::Fail(lines) => assert!(lines.iter().all(|l| l.starts_with(&k)), "{lines:?}"),
        v => panic!("{v:?}"),
    }
    // Faster is never a failure.
    assert!(matches!(
        gate::compare(&b, &b.scaled(0.5)),
        Verdict::Pass { .. }
    ));
}

#[test]
fn other_adapters_and_profiles_are_skipped_not_passed() {
    let b = baseline();
    let mut other = b.clone();
    other.env.class = "software".into();
    assert!(matches!(gate::compare(&b, &other), Verdict::Skip(_)));
    let mut other = b.clone();
    other.env.profile = if b.env.profile == "dev" {
        "release"
    } else {
        "dev"
    }
    .into();
    assert!(matches!(gate::compare(&b, &other), Verdict::Skip(_)));
    let mut other = b.scaled(3.0);
    other.env.adapter = "Some Other GPU (Vulkan)".into();
    assert!(matches!(gate::compare(&b, &other), Verdict::Skip(_)));
}

#[test]
fn new_and_missing_cases_are_notes() {
    let b = baseline();
    let mut now = b.clone();
    let k = now.cases.keys().next().unwrap().clone();
    let c = now.cases.remove(&k).unwrap();
    now.cases.insert("new/case".into(), c);
    match gate::compare(&b, &now) {
        Verdict::Pass { notes, .. } => {
            assert!(notes.iter().any(|n| n.starts_with("new/case")));
            assert!(notes.iter().any(|n| n.starts_with(&k)));
        }
        v => panic!("{v:?}"),
    }
}

#[test]
fn a_provisional_bless_keeps_each_cases_best_run() {
    let b = baseline();
    let mut slower = b.scaled(1.5);
    let k = slower.cases.keys().next().unwrap().clone();
    // One case got faster in the new run: that one is taken, the others stay.
    let fast = b.cases[&k].clone();
    let mut f = Results {
        cases: [(k.clone(), fast)].into_iter().collect(),
        ..b.clone()
    }
    .scaled(0.5);
    slower.cases.insert(k.clone(), f.cases.remove(&k).unwrap());
    let blessed = slower.best_with(&b);
    for (name, c) in &blessed.cases {
        let want = if *name == k {
            b.cases[name].gated_stat().p50 * 0.5
        } else {
            b.cases[name].gated_stat().p50
        };
        assert!((c.gated_stat().p50 - want).abs() < 1e-9, "{name}");
    }
}

/// The last bench run (`$CARGO_TARGET_DIR/develop-bench/results-gpu.json`) against the
/// baseline. Skipped, and says so, without a run, under load or on another adapter.
#[test]
fn the_last_bench_run_is_within_the_baseline() {
    let Some(t) = std::env::var_os("CARGO_TARGET_DIR") else {
        eprintln!("SKIPPED: CARGO_TARGET_DIR is not set, no bench results to gate");
        return;
    };
    let path = std::path::Path::new(&t).join("develop-bench/results-gpu.json");
    let Ok(now) = Results::read(&path) else {
        eprintln!("SKIPPED: no bench results at {}", path.display());
        return;
    };
    if let Some(why) = gate::overloaded() {
        eprintln!("SKIPPED (not passed): {why}");
        return;
    }
    match gate::compare(&baseline(), &now) {
        Verdict::Fail(lines) => panic!("over the +10 % rule:\n{}", lines.join("\n")),
        Verdict::Skip(why) => eprintln!("SKIPPED (not passed): {why}"),
        Verdict::Pass { compared, .. } => eprintln!("passed: {compared} cases"),
    }
}
