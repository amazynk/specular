//! Compress, whole-stack steps, the memory budget, paste, snapshots, styles, presets
//! and auto-apply rules.

use std::time::Instant;

use super::history::{BUDGET, WHOLE_EDIT};
use super::style::{auto_style, shipped_presets, shipped_styles};
use super::*;
use crate::sidecar;

fn reg() -> &'static Registry {
    Registry::builtin()
}

fn base() -> Edit {
    Edit::default_for(reg(), SourceKey::default(), true)
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }
}

fn field_step(e: &Edit, kind: ModuleKind, inst: u16, name: &str, v: f32, t: i64) -> Step {
    let before = e.module(kind, inst).unwrap().params.to_fields();
    let after = before.clone().with(name, Value::Float(v));
    Step::params(kind, inst, &before, &after, t)
}

/// A random history of every kind of step on `e`; returns the steps pushed.
fn random_history(seed: u64, n: usize) -> (Edit, History, Edit) {
    let r = reg();
    let mut rng = Rng(seed);
    let start = base();
    let mut e = start.clone();
    let mut h = History::default();
    // Steps far apart in time, so push never merges them: compress must.
    let mut t = 1_000_000;
    for _ in 0..n {
        t += 5_000;
        let step = match rng.below(9) {
            0..=2 => {
                let (k, f) = [
                    (ModuleKind::EXPOSURE, "exposure_ev"),
                    (ModuleKind::VIEW_TRANSFORM, "contrast"),
                    (ModuleKind::EXPOSURE, "black_offset"),
                ][rng.below(3) as usize];
                let v = rng.below(5) as f32 * 0.25;
                field_step(&e, k, 0, f, v, t)
            }
            3 => {
                let k = [ModuleKind::EXPOSURE, ModuleKind::VIEW_TRANSFORM][rng.below(2) as usize];
                let on = !e.module(k, 0).unwrap().enabled;
                Step {
                    kind: k,
                    instance: 0,
                    change: Change::Enabled(on),
                    time_ms: t,
                }
            }
            4 => {
                // A second exposure instance in or out.
                match e.position(ModuleKind::EXPOSURE, 1) {
                    Some(at) => Step {
                        kind: ModuleKind::EXPOSURE,
                        instance: 1,
                        change: Change::Remove {
                            at: at as u32,
                            module: Box::new(e.modules[at].clone()),
                        },
                        time_ms: t,
                    },
                    None => {
                        let m = ModuleInstance::new(r, ModuleKind::EXPOSURE, 1);
                        Step {
                            kind: ModuleKind::EXPOSURE,
                            instance: 1,
                            change: Change::Add {
                                at: e.modules.len() as u32,
                                module: Box::new(m),
                            },
                            time_ms: t,
                        }
                    }
                }
            }
            5 if e.module(ModuleKind::EXPOSURE, 1).is_some() => {
                field_step(&e, ModuleKind::EXPOSURE, 1, "exposure_ev", -0.5, t)
            }
            6 => {
                let mut after = e.masks.clone();
                after.push(MaskShape {
                    id: e.next_mask_id(),
                    shape: Shape::Linear {
                        start: [0.0, 0.0],
                        end: [0.0, rng.below(9) as f32 / 10.0],
                    },
                });
                if rng.below(3) == 0 {
                    after.clear();
                }
                Step {
                    kind: ModuleKind::EXPOSURE,
                    instance: 0,
                    change: Change::Masks {
                        before: e.masks.clone(),
                        after,
                    },
                    time_ms: t,
                }
            }
            7 => {
                let s = &shipped_styles(r)[rng.below(4) as usize];
                let after = s.applied(&e, r, ApplyMode::Append);
                Step::stack(&s.name, &e, &after, t)
            }
            _ => field_step(&e, ModuleKind::COLOUR_CALIBRATION, 0, "white_x", 0.31, t),
        };
        h.push(&mut e, step).unwrap();
        if rng.below(10) == 0 {
            // Some undos, and new steps after them, along the way.
            let back = rng.below(3) as usize;
            for _ in 0..back {
                h.undo(&mut e).unwrap();
            }
        }
    }
    (start, h, e)
}

#[test]
fn compress_keeps_the_edit_the_start_and_the_redo_tail() {
    let mut total = (0, 0);
    for seed in 1..=60 {
        let (start, mut h, mut e) = random_history(seed * 7919, 80);
        // Leave a redo tail.
        h.undo(&mut e).unwrap();
        h.undo(&mut e).unwrap();
        let now = e.clone();
        let mut full = e.clone();
        let mut hh = h.clone();
        hh.go_to(&mut full, usize::MAX).unwrap();
        let tail = h.steps.len() - h.position;
        let before = h.steps.len();
        let gone = h.compress();
        assert_eq!(e, now, "seed {seed}: the edit changed");
        assert_eq!(h.steps.len(), before - gone);
        assert_eq!(
            h.steps.len() - h.position,
            tail,
            "seed {seed}: the redo tail"
        );
        let mut walk = e.clone();
        let mut hw = h.clone();
        hw.go_to(&mut walk, 0).unwrap();
        assert_eq!(walk, start, "seed {seed}: undo to the start");
        hw.go_to(&mut walk, usize::MAX).unwrap();
        assert_eq!(walk, full, "seed {seed}: redo to the end");
        // Compressing again finds nothing more.
        assert_eq!(
            h.clone().compress(),
            0,
            "seed {seed}: compress is idempotent"
        );
        total.0 += before;
        total.1 += gone;
    }
    println!("compress: {} of {} steps dropped", total.1, total.0);
    assert!(total.1 > total.0 / 4);
}

#[test]
fn compress_folds_drags_and_cancels_switches() {
    let mut e = base();
    let start = e.clone();
    let mut h = History::default();
    let k = ModuleKind::EXPOSURE;
    let mut t = 0;
    for v in [0.5, 1.0, -0.3] {
        t += 10_000;
        let s = field_step(&e, k, 0, "exposure_ev", v, t);
        h.push(&mut e, s).unwrap();
        // Something else between: a switch on another module turned off and on again.
        t += 10_000;
        for on in [false, true] {
            let s = Step {
                kind: ModuleKind::VIEW_TRANSFORM,
                instance: 0,
                change: Change::Enabled(on),
                time_ms: t,
            };
            h.push(&mut e, s).unwrap();
        }
    }
    assert_eq!(h.steps.len(), 9);
    h.compress();
    assert_eq!(h.steps.len(), 1, "{:?}", h.steps);
    let mut w = e.clone();
    h.clone().go_to(&mut w, 0).unwrap();
    assert_eq!(w, start);
    // A change back to the start goes entirely.
    let s = field_step(&e, k, 0, "exposure_ev", 0.0, t + 10_000);
    h.push(&mut e, s).unwrap();
    h.compress();
    assert!(h.steps.is_empty(), "{:?}", h.steps);
    assert_eq!(e, start);
}

#[test]
fn stack_steps_round_trip_and_undo() {
    let r = reg();
    let mut e = base();
    let start = e.clone();
    let mut h = History::default();
    let style = &shipped_styles(r)[0];
    assert!(style.paste(&mut e, &mut h, r, ApplyMode::Append, "Paste", 1));
    assert_eq!(h.steps.len(), 1);
    assert_eq!(h.steps[0].kind, WHOLE_EDIT);
    assert_ne!(e, start);
    // Pasting the same again changes nothing: no step.
    assert!(!style.paste(&mut e, &mut h, r, ApplyMode::Append, "Paste", 2));
    let blob = codec::encode_history(&h);
    assert_eq!(codec::decode_history(&blob, r).unwrap(), h);
    let after = e.clone();
    h.undo(&mut e).unwrap();
    assert_eq!(e, start);
    h.redo(&mut e).unwrap();
    assert_eq!(e, after);
}

/// The history stays within its memory budget however long the session (the undo
/// budget): slider drags merge, and whole-stack steps beyond the budget drop the oldest.
#[test]
fn history_stays_within_its_memory_budget() {
    let r = reg();
    let mut e = base();
    let mut h = History::default();
    // Ten thousand slider moves, 16 ms apart, in drags of a second.
    for i in 0..10_000i64 {
        let t = i * 16 + (i / 60) * 2_000;
        let s = field_step(
            &e,
            ModuleKind::EXPOSURE,
            0,
            "exposure_ev",
            (i % 97) as f32 * 0.01,
            t,
        );
        h.push(&mut e, s).unwrap();
    }
    println!(
        "10 000 slider moves: {} steps, {} bytes",
        h.steps.len(),
        h.bytes()
    );
    assert!(h.bytes() <= BUDGET);
    // Hundreds of pastes of a big style: trimmed to the budget, the newest kept.
    let styles = shipped_styles(r);
    let mut big = Style::from_edit("all", &e, |_| true);
    for i in 0..30 {
        let mut m = ModuleInstance::new(r, ModuleKind::EXPOSURE, 0);
        m.label = Some(format!("extra {i}"));
        big.items.push(m);
    }
    for i in 0..400 {
        let s = if i % 2 == 0 { &big } else { &styles[i % 4] };
        let mut v = s.clone();
        v.items[0].label = Some(format!("{i}"));
        v.paste(
            &mut e,
            &mut h,
            r,
            ApplyMode::Replace,
            "Paste",
            10_000_000 + i as i64,
        );
    }
    println!(
        "after 400 pastes: {} steps, {} bytes (budget {BUDGET})",
        h.steps.len(),
        h.bytes()
    );
    assert!(h.bytes() <= BUDGET);
    assert!(h.can_undo());
    let now = e.clone();
    h.undo(&mut e).unwrap();
    h.redo(&mut e).unwrap();
    assert_eq!(e, now);
    // Undo and redo of a whole-stack step are edits only (the picture is the renderer's):
    // the slowest of every step, back to the start and forward again.
    let mut worst = 0f64;
    for _ in 0..h.steps.len() {
        let t = Instant::now();
        h.undo(&mut e).unwrap();
        worst = worst.max(t.elapsed().as_secs_f64() * 1e3);
    }
    for _ in 0..h.steps.len() {
        let t = Instant::now();
        h.redo(&mut e).unwrap();
        worst = worst.max(t.elapsed().as_secs_f64() * 1e3);
    }
    assert_eq!(e, now);
    println!("undo/redo of a step (edit only): worst {worst:.3} ms");
    assert!(worst < 16.0);
}

/// Paste on 1,000 photos, edits only (acceptance: ≤ 1 s): apply, one history step each,
/// and the stored forms (edit blob, hash, steps).
#[test]
fn a_thousand_photo_paste_is_quick() {
    let r = reg();
    let mut src = base();
    for (k, f, v) in [
        (ModuleKind::EXPOSURE, "exposure_ev", 0.7),
        (ModuleKind::VIEW_TRANSFORM, "contrast", 1.9),
    ] {
        let m = src.module_mut(k, 0).unwrap();
        let fs = m.params.to_fields().with(f, Value::Float(v));
        m.params.set_fields(fs);
    }
    let copied = Style::from_edit_default("clip", &src, r);
    let mut photos: Vec<(Edit, History)> =
        (0..1000).map(|_| (base(), History::default())).collect();
    let t0 = Instant::now();
    let mut stored = 0usize;
    for (e, h) in &mut photos {
        assert!(copied.paste(e, h, r, ApplyMode::Append, "Paste", 1));
        let blob = codec::encode_edit(e);
        let hash = hash::edit_hash(e);
        let steps: Vec<Vec<u8>> = h.steps.iter().map(codec::encode_step).collect();
        stored += blob.len() + hash.len() + steps.iter().map(Vec::len).sum::<usize>();
    }
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    println!("1000-photo paste (edits only): {ms:.1} ms, {stored} bytes to store");
    assert!(ms <= 1000.0, "paste took {ms} ms");
    assert!(
        photos
            .iter()
            .all(|(e, _)| e.module(ModuleKind::EXPOSURE, 0) == src.module(ModuleKind::EXPOSURE, 0))
    );
}

#[test]
fn snapshots_take_and_restore() {
    let r = reg();
    let mut e = base();
    let mut h = History::default();
    e.snapshots.push(Snapshot::take("plain", &e));
    let s = field_step(&e, ModuleKind::EXPOSURE, 0, "exposure_ev", 1.5, 1);
    h.push(&mut e, s).unwrap();
    let edited = e.clone();
    let snap = e.snapshots[0].clone();
    let back = snap.edit_from(&e);
    assert_eq!(back.modules, base().modules);
    assert_eq!(back.snapshots, e.snapshots);
    // Restoring is one step; undo returns to the edit.
    let step = Step::stack(&snap.name, &e, &back, 2);
    h.push(&mut e, step).unwrap();
    assert_eq!(e.modules, base().modules);
    h.undo(&mut e).unwrap();
    assert_eq!(e, edited);
    // Snapshots survive storage.
    let (d, _) = codec::decode_edit(&codec::encode_edit(&e), r).unwrap();
    assert_eq!(d.snapshots, e.snapshots);
}

#[test]
fn selective_copy_takes_only_the_chosen_modules() {
    let r = reg();
    let mut src = base();
    for k in [ModuleKind::EXPOSURE, ModuleKind::VIEW_TRANSFORM] {
        let m = src.module_mut(k, 0).unwrap();
        m.enabled = false;
    }
    let s = Style::from_kinds("c", &src, &[ModuleKind::EXPOSURE]);
    assert_eq!(s.kinds(), vec![ModuleKind::EXPOSURE]);
    let out = s.applied(&base(), r, ApplyMode::Append);
    assert!(!out.module(ModuleKind::EXPOSURE, 0).unwrap().enabled);
    assert!(out.module(ModuleKind::VIEW_TRANSFORM, 0).unwrap().enabled);
}

#[test]
fn styles_presets_and_rules_round_trip_as_files() {
    let r = reg();
    for s in shipped_styles(r) {
        let text = sidecar::files::style_to_xmp(&s);
        let (back, rep) = sidecar::files::style_from_xmp(&text, r, "x").unwrap();
        assert!(rep.migrated.is_empty());
        assert_eq!(back, s);
    }
    assert_eq!(
        sidecar::files::style_file_name("Night / Rain?"),
        "Night _ Rain_.xmp"
    );
    let mut presets = shipped_presets();
    presets.push(Preset {
        kind: ModuleKind::COLOUR_CALIBRATION,
        name: "Tungsten = warm\nlight".into(),
        version: 1,
        fields: Fields::new()
            .with("white_x", Value::Float(0.4476))
            .with("white_source", Value::Text("custom".into())),
    });
    let text = sidecar::files::presets_to_text(&presets);
    let back = sidecar::files::presets_from_text(&text, r);
    assert_eq!(back.len(), presets.len());
    for (a, b) in back.iter().zip(&presets) {
        assert_eq!((a.kind, &a.fields), (b.kind, &b.fields));
    }
    // A line break in a name cannot start a new key.
    assert_eq!(back.last().unwrap().name, "Tungsten = warm light");
    let rules = vec![
        AutoRule {
            style: "Night".into(),
            maker: "Canon".into(),
            iso_min: Some(3200),
            file_type: FileType::Raw,
            ..AutoRule::default()
        },
        AutoRule {
            style: "Phone".into(),
            model: "iPhone".into(),
            iso_max: Some(400),
            file_type: FileType::Other,
            ..AutoRule::default()
        },
        AutoRule {
            style: "Everything".into(),
            ..AutoRule::default()
        },
    ];
    let text = sidecar::files::rules_to_text(&rules);
    assert_eq!(sidecar::files::rules_from_text(&text), rules);
    // Broken blocks are skipped, the rest load.
    let messy = format!("[rule]\nmaker = x\n\n[nonsense]\na = b\n{text}");
    assert_eq!(sidecar::files::rules_from_text(&messy), rules);

    let canon = PhotoFacts {
        maker: "Canon".into(),
        model: "EOS R5".into(),
        iso: Some(6400),
        ext: "cr3".into(),
        raw: true,
        ..PhotoFacts::default()
    };
    assert_eq!(auto_style(&rules, &canon), Some("Night"));
    let low = PhotoFacts {
        iso: Some(100),
        ..canon.clone()
    };
    assert_eq!(auto_style(&rules, &low), Some("Everything"));
    let phone = PhotoFacts {
        maker: "Apple".into(),
        model: "iPhone 15".into(),
        iso: Some(50),
        ext: "heic".into(),
        raw: false,
        ..PhotoFacts::default()
    };
    assert_eq!(auto_style(&rules, &phone), Some("Phone"));
    let unknown_iso = PhotoFacts { iso: None, ..phone };
    assert_eq!(auto_style(&rules, &unknown_iso), Some("Everything"));
}

#[test]
fn presets_apply_to_their_module() {
    let r = reg();
    let mut e = base();
    let p = shipped_presets()
        .into_iter()
        .find(|p| p.kind == ModuleKind::EXPOSURE)
        .unwrap();
    let m = e.module_mut(ModuleKind::EXPOSURE, 0).unwrap();
    assert!(p.apply_to(m));
    assert_eq!(m.params.to_fields().f32_or("exposure_ev", 0.0), 1.0);
    let own = Preset::of("mine", e.module(ModuleKind::EXPOSURE, 0).unwrap());
    assert_eq!(own.fields.f32_or("exposure_ev", 0.0), 1.0);
    let vt = e.module_mut(ModuleKind::VIEW_TRANSFORM, 0).unwrap();
    assert!(!p.apply_to(vt));
    let _ = r;
}

#[test]
fn versions_have_their_own_sidecars() {
    let dir = std::env::temp_dir().join(format!("specular-dv16-versions-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("IMG_0001.CR2");
    let mut e = base();
    let mut h = History::default();
    let s = field_step(&e, ModuleKind::EXPOSURE, 0, "exposure_ev", 0.4, 1);
    h.push(&mut e, s).unwrap();
    sidecar::write(&img, &base(), None).unwrap();
    let p2 = sidecar::write_version(&img, 2, Some("Black & white"), &e, Some(&h)).unwrap();
    assert_eq!(p2.file_name().unwrap(), "IMG_0001.CR2.v2.specular.xmp");
    sidecar::write_version(&img, 5, None, &e, None).unwrap();
    // Another image's versions are not this one's.
    sidecar::write_version(&dir.join("IMG_0001.CR2X"), 3, None, &e, None).unwrap();
    assert_eq!(sidecar::versions_on_disk(&img), vec![2, 5]);
    let mut counts = sidecar::version_counts_in(&dir);
    counts.sort();
    assert_eq!(
        counts,
        vec![(img.clone(), 3), (dir.join("IMG_0001.CR2X"), 2)]
    );
    let back = sidecar::read_version(&img, 2, reg()).unwrap().unwrap();
    assert_eq!(back.name.as_deref(), Some("Black & white"));
    assert_eq!(back.edit, e);
    assert_eq!(back.history, h);
    assert_eq!(sidecar::read(&img, reg()).unwrap().unwrap().edit, base());
    sidecar::remove_version(&img, 2).unwrap();
    sidecar::remove_version(&img, 2).unwrap();
    assert_eq!(sidecar::versions_on_disk(&img), vec![5]);
    assert!(img.with_extension("CR2.specular.xmp").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
