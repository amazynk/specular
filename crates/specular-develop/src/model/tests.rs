use super::codec::{self, DecodeError};
use super::fields::Choice;
use super::hash::{self, hex};
use super::kind::{Migration, RegistryError};
use super::mask::{ColourRange, Combine, PathPoint, Range, RangeChannel, Stroke};
use super::migrate::{self, NOTICE_MIGRATED, NOTICE_NEWER};
use super::params::{
    self, ColourCalibration, CropOrientation, Exposure, ViewMethod, ViewTransform,
};
use super::*;

fn source() -> SourceKey {
    SourceKey {
        size: 24_510_322,
        mtime_ns: 1_730_000_000_123_456_789,
        head_tail: std::array::from_fn(|i| i as u8 * 7),
    }
}

fn reg() -> &'static Registry {
    Registry::builtin()
}

/// An edit that uses every model feature.
fn rich_edit() -> Edit {
    let r = reg();
    let mut e = Edit::default_for(r, source(), true);
    e.masks = vec![
        MaskShape {
            id: 1,
            shape: Shape::Ellipse {
                centre: [0.4, 0.5],
                radii: [0.2, 0.1],
                angle_deg: 15.0,
                feather: 0.05,
            },
        },
        MaskShape {
            id: 2,
            shape: Shape::Brush {
                strokes: vec![Stroke {
                    radius: 0.01,
                    hardness: 0.5,
                    flow: 0.8,
                    erase: false,
                    pressure: crate::model::mask::PressureUse::Flow,
                    points: vec![[0.1, 0.1, 1.0], [0.12, 0.11, 0.7]],
                }],
            },
        },
        MaskShape {
            id: 3,
            shape: Shape::Polygon {
                points: vec![
                    PathPoint {
                        x: 0.1,
                        y: 0.1,
                        feather: 0.0,
                    },
                    PathPoint {
                        x: 0.9,
                        y: 0.1,
                        feather: 0.02,
                    },
                    PathPoint {
                        x: 0.5,
                        y: 0.9,
                        feather: 0.01,
                    },
                ],
                closed: true,
            },
        },
        MaskShape {
            id: 4,
            shape: Shape::Linear {
                start: [0.0, 0.0],
                end: [0.0, 0.4],
            },
        },
        MaskShape {
            id: 5,
            shape: Shape::Radial {
                centre: [0.5, 0.5],
                inner: [0.1, 0.1],
                outer: [0.3, 0.2],
                angle_deg: -5.0,
            },
        },
    ];
    let mut second = ModuleInstance::with_params(
        Exposure {
            exposure_ev: -0.5,
            ..Exposure::default()
        },
        1,
    );
    second.label = Some("sky".into());
    second.blend = Some(Blend {
        mode: BlendMode::Luminance,
        opacity: 0.75,
        invert: false,
        mask: MaskExpr::Combine(
            Combine::Subtract,
            Box::new(MaskExpr::Combine(
                Combine::Union,
                Box::new(MaskExpr::Shape(1)),
                Box::new(MaskExpr::Shape(2)),
            )),
            Box::new(MaskExpr::Invert(Box::new(MaskExpr::Range(Range {
                channel: RangeChannel::Luminance,
                on_output: false,
                limits: [-6.0, -4.0, 0.0, 2.0],
            })))),
        ),
        refine: Refine {
            feather: 0.1,
            edge_aware: 0.5,
            ..Refine::default()
        },
    });
    e.insert(r, second);
    let cb = e.module_mut(ModuleKind::COLOUR_CALIBRATION, 0).unwrap();
    cb.blend = Some(Blend {
        mask: MaskExpr::Combine(
            Combine::Intersect,
            Box::new(MaskExpr::FromModule {
                kind: ModuleKind::EXPOSURE,
                instance: 1,
            }),
            Box::new(MaskExpr::Combine(
                Combine::Exclude,
                Box::new(MaskExpr::ColourRange(ColourRange {
                    colours: vec![[0.6, 0.1, -0.2]],
                    distance: 0.05,
                    falloff: 0.02,
                })),
                Box::new(MaskExpr::Combine(
                    Combine::Union,
                    Box::new(MaskExpr::Ai(crate::model::mask::AiMask {
                        kind: crate::model::mask::AiKind::Sky,
                        model: "upernet-convnext-tiny-ade20k".into(),
                        points: vec![crate::model::mask::AiPoint {
                            x: 0.5,
                            y: 0.25,
                            positive: false,
                        }],
                        rect: Some([0.0, 0.1, 0.2, 0.3]),
                        key: [9; 32],
                    })),
                    Box::new(MaskExpr::Shape(3)),
                )),
            )),
        ),
        ..Blend::default()
    });
    // A reserved kind with plain fields, and an unknown one from a newer build.
    let mut lut = ModuleInstance::new(r, ModuleKind::LUT_3D, 0);
    lut.params = Params::Other(
        ModuleKind::LUT_3D,
        Fields::new()
            .with("path", Value::Text("film.cube".into()))
            .with("size", Value::Int(-33)),
    );
    e.insert(r, lut);
    e.modules.push(ModuleInstance {
        kind: ModuleKind(4000),
        version: 3,
        instance: 0,
        label: None,
        enabled: false,
        params: Params::Other(
            ModuleKind(4000),
            Fields::new().with("weights", Value::Floats(vec![1.0, f32::NAN, -0.0])),
        ),
        blend: Some(Blend {
            mask: MaskExpr::Shape(4),
            ..Blend::default()
        }),
    });
    e.custom_order = true;
    e.snapshots.push(Snapshot {
        name: "before sky".into(),
        modules: e.modules[..3].to_vec(),
        masks: e.masks[4..].to_vec(),
    });
    e
}

#[test]
fn registry_builtin_is_valid_and_ordered() {
    let r = reg();
    assert_eq!(r.specs().len(), 34);
    let e = Edit::default_for(r, source(), true);
    let kinds: Vec<_> = e
        .modules
        .iter()
        .map(|m| r.get(m.kind).unwrap().name)
        .collect();
    assert_eq!(
        kinds,
        [
            "raw_front_end",
            "crop_orientation",
            "exposure",
            "colour_calibration",
            "view_transform"
        ]
    );
    assert!(e.in_recommended_order(r));
    let jpeg = Edit::default_for(r, source(), false);
    assert_eq!(jpeg.modules.len(), 4);
    // Typed params read back their own defaults.
    for m in &e.modules {
        assert_eq!(Params::from_fields(m.kind, m.params.to_fields()), m.params);
        assert!(!matches!(m.params, Params::Other(..)));
        assert!(migrate::renders(r, m));
    }
    assert!(r.rank(ModuleKind(4000)) > r.rank(ModuleKind::VIEW_TRANSFORM));
}

#[test]
fn registry_refuses_bad_specs() {
    let mut r = sample_registry();
    let dup = r.get(SAMPLE).unwrap().clone();
    assert_eq!(
        r.register(dup.clone()),
        Err(RegistryError::DuplicateKind(SAMPLE))
    );
    let three_live = ModuleSpec {
        kind: ModuleKind(901),
        name: "x",
        live: &[1, 2, 3],
        ..dup.clone()
    };
    assert_eq!(
        r.register(three_live),
        Err(RegistryError::BadLive(ModuleKind(901)))
    );
    let gap = ModuleSpec {
        kind: ModuleKind(902),
        name: "y",
        migrations: &MIGRATIONS[..1],
        ..dup
    };
    assert_eq!(
        r.register(gap),
        Err(RegistryError::MissingMigration(ModuleKind(902), 2))
    );
}

#[test]
fn edit_round_trips() {
    let r = reg();
    let e = rich_edit();
    let bytes = codec::encode_edit(&e);
    let (back, report) = codec::decode_edit(&bytes, r).unwrap();
    // The unknown kind is reported, kept and written back byte for byte.
    assert_eq!(report.migrated, []);
    assert_eq!(report.not_rendered, [(ModuleKind(4000), 0, 3)]);
    assert_eq!(codec::encode_edit(&back), bytes);
    // NaN never compares equal; compare everything else.
    let strip = |mut e: Edit| {
        e.modules.retain(|m| m.kind != ModuleKind(4000));
        e
    };
    assert_eq!(strip(back.clone()), strip(e.clone()));
    assert_eq!(back.snapshots, e.snapshots);
    assert_eq!(back.masks, e.masks);
}

#[test]
fn every_choice_name_round_trips() {
    fn check<C: Choice + PartialEq + std::fmt::Debug>() {
        for (c, n) in C::ALL {
            assert_eq!(C::from_name(n), Some(*c));
            assert_eq!(c.name(), *n);
        }
    }
    check::<params::Demosaic>();
    check::<params::Highlights>();
    check::<params::InputProfile>();
    check::<params::Adaptation>();
    check::<params::WhiteSource>();
    check::<ViewMethod>();
    check::<BlendMode>();
    check::<RangeChannel>();
    check::<Combine>();
}

#[test]
fn damaged_blobs_are_refused_not_panicking() {
    let r = reg();
    let bytes = codec::encode_edit(&rich_edit());
    for n in 0..bytes.len() {
        assert!(codec::decode_edit(&bytes[..n], r).is_err(), "prefix {n}");
    }
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert_eq!(
        codec::decode_edit(&bad, r).err(),
        Some(DecodeError::BadMagic)
    );
    let mut newer = bytes.clone();
    newer[4] = 2;
    assert_eq!(
        codec::decode_edit(&newer, r).err(),
        Some(DecodeError::NewerFormat(2))
    );
    // A huge count must not allocate.
    let mut huge = bytes[..6].to_vec();
    huge.extend_from_slice(&[0; 50]);
    huge.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert!(codec::decode_edit(&huge, r).is_err());
    // Flipped bytes give an edit or an error, never a panic.
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0x5A;
        let _ = codec::decode_edit(&b, r);
    }
}

#[test]
fn frames_skip_fields_a_newer_writer_appends() {
    // A module frame with two extra trailing bytes still reads.
    let r = reg();
    let mut e = Edit::empty(source());
    let mut m = ModuleInstance::new(r, ModuleKind::LUT_3D, 0);
    m.params = Params::Other(m.kind, Fields::new().with("size", Value::Int(33)));
    e.modules.push(m);
    let bytes = codec::encode_edit(&e);
    // Header 6 + process 2 + source 48 + order 1 + count 1, then the frame length.
    let at = 6 + 2 + 48 + 1 + 1;
    let len = bytes[at] as usize;
    let mut longer = bytes[..at].to_vec();
    longer.push((len + 2) as u8);
    longer.extend_from_slice(&bytes[at + 1..at + 1 + len]);
    longer.extend_from_slice(&[0xAB, 0xCD]);
    longer.extend_from_slice(&bytes[at + 1 + len..]);
    let (back, _) = codec::decode_edit(&longer, r).unwrap();
    assert_eq!(back, e);
}

fn now() -> i64 {
    1_730_000_000_000
}

#[test]
fn a_move_that_turns_its_module_on_is_one_step() {
    let r = reg();
    let mut e = Edit::default_for(r, source(), true);
    let k = ModuleKind::EXPOSURE;
    e.module_mut(k, 0).unwrap().enabled = false;
    let start = e.clone();
    let mut h = History::default();
    // The first change turns the module on with its value: a whole-instance step.
    let before = e.module(k, 0).unwrap().clone();
    let mut after = before.clone();
    after.enabled = true;
    let f = after
        .params
        .to_fields()
        .with("exposure_ev", Value::Float(0.5));
    after.params.set_fields(f);
    let replace = Change::Replace {
        before: Box::new(before),
        after: Box::new(after),
    };
    h.push(
        &mut e,
        Step {
            kind: k,
            instance: 0,
            change: replace,
            time_ms: now(),
        },
    )
    .unwrap();
    // The rest of the drag joins it.
    let b = e.module(k, 0).unwrap().params.to_fields();
    let a = b.clone().with("exposure_ev", Value::Float(1.5));
    h.push(&mut e, Step::params(k, 0, &b, &a, now() + 300))
        .unwrap();
    assert_eq!(h.steps.len(), 1);
    let moved = e.clone();
    h.undo(&mut e).unwrap();
    assert_eq!(e, start);
    h.redo(&mut e).unwrap();
    assert_eq!(e, moved);
    assert!(e.module(k, 0).unwrap().enabled);
    // Later changes are steps of their own.
    let b = e.module(k, 0).unwrap().params.to_fields();
    let a = b.clone().with("exposure_ev", Value::Float(2.0));
    h.push(&mut e, Step::params(k, 0, &b, &a, now() + 5_000))
        .unwrap();
    assert_eq!(h.steps.len(), 2);
}

#[test]
fn history_undo_redo_and_merge() {
    let r = reg();
    let mut e = Edit::default_for(r, source(), true);
    let start = e.clone();
    let mut h = History::default();
    let k = ModuleKind::EXPOSURE;
    let set_ev = |e: &Edit, ev: f32| {
        let before = e.module(k, 0).unwrap().params.to_fields();
        let mut after = before.clone();
        after.set("exposure_ev", Value::Float(ev));
        (before, after)
    };
    // A drag: three changes within a second merge into one step.
    for (i, ev) in [0.3, 0.6, 0.9].into_iter().enumerate() {
        let (b, a) = set_ev(&e, ev);
        h.push(&mut e, Step::params(k, 0, &b, &a, now() + i as i64 * 300))
            .unwrap();
    }
    assert_eq!(h.steps.len(), 1);
    let after_drag = e.clone();
    // Later, a separate change.
    let (b, a) = set_ev(&e, 2.0);
    h.push(&mut e, Step::params(k, 0, &b, &a, now() + 5_000))
        .unwrap();
    h.push(
        &mut e,
        Step {
            kind: ModuleKind::VIEW_TRANSFORM,
            instance: 0,
            change: Change::Enabled(false),
            time_ms: now() + 6_000,
        },
    )
    .unwrap();
    let added = ModuleInstance::with_params(Exposure::default(), 1);
    let at = e.position(k, 0).unwrap() as u32 + 1;
    h.push(
        &mut e,
        Step {
            kind: k,
            instance: 1,
            change: Change::Add {
                at,
                module: Box::new(added),
            },
            time_ms: now() + 7_000,
        },
    )
    .unwrap();
    h.push(
        &mut e,
        Step {
            kind: k,
            instance: 1,
            change: Change::Move {
                from: at,
                to: 0,
                custom_before: false,
            },
            time_ms: now() + 8_000,
        },
    )
    .unwrap();
    assert!(e.custom_order && !e.in_recommended_order(r));
    let end = e.clone();
    assert_eq!(h.steps.len(), 5);

    h.go_to(&mut e, 1).unwrap();
    assert_eq!(e, after_drag);
    h.go_to(&mut e, 0).unwrap();
    assert_eq!(e, start);
    assert!(!h.undo(&mut e).unwrap());
    h.go_to(&mut e, 99).unwrap();
    assert_eq!(e, end);
    assert!(!h.redo(&mut e).unwrap());

    // The history survives storage, as one blob and as per-step rows.
    let blob = codec::encode_history(&h);
    assert_eq!(codec::decode_history(&blob, r).unwrap(), h);
    for s in &h.steps {
        assert_eq!(&codec::decode_step(&codec::encode_step(s), r).unwrap(), s);
    }

    // A new step after undo drops the redo tail; compress keeps the edit.
    h.go_to(&mut e, 2).unwrap();
    let (b, a) = set_ev(&e, -1.0);
    h.push(&mut e, Step::params(k, 0, &b, &a, now() + 20_000))
        .unwrap();
    assert_eq!((h.steps.len(), h.position), (3, 3));
    let kept = e.clone();
    h.clear();
    assert!(!h.can_undo() && !h.can_redo());
    assert_eq!(e, kept);
    // A step on a missing module is refused and changes nothing.
    let bad = Step {
        kind: ModuleKind::CURVES,
        instance: 0,
        change: Change::Enabled(true),
        time_ms: now(),
    };
    assert!(h.push(&mut e, bad).is_err());
    assert_eq!(e, kept);
}

#[test]
fn history_other_changes_invert() {
    let r = reg();
    let mut e = rich_edit();
    let start = e.clone();
    let mut h = History::default();
    let k = ModuleKind::EXPOSURE;
    let m = e.module(k, 1).unwrap().clone();
    let at = e.position(k, 1).unwrap() as u32;
    let mut masks = e.masks.clone();
    masks.remove(0);
    let steps = vec![
        Change::Label {
            before: m.label.clone(),
            after: Some("sky 2".into()),
        },
        Change::Blend {
            before: m.blend.clone().map(Box::new),
            after: None,
        },
        Change::Replace {
            before: Box::new(e.module(k, 0).unwrap().clone()),
            after: Box::new(ModuleInstance::with_params(
                Exposure {
                    auto: true,
                    ..Exposure::default()
                },
                0,
            )),
        },
        Change::Masks {
            before: e.masks.clone(),
            after: masks,
        },
    ];
    for (i, c) in steps.into_iter().enumerate() {
        let instance = if i == 2 { 0 } else { 1 };
        h.push(
            &mut e,
            Step {
                kind: k,
                instance,
                change: c,
                time_ms: now() + i as i64 * 10_000,
            },
        )
        .unwrap();
    }
    let m = e.module(k, 1).unwrap().clone();
    h.push(
        &mut e,
        Step {
            kind: k,
            instance: 1,
            change: Change::Remove {
                at,
                module: Box::new(m),
            },
            time_ms: now() + 90_000,
        },
    )
    .unwrap();
    assert!(e.module(k, 1).is_none());
    let end = e.clone();
    let strip = |mut e: Edit| {
        e.modules.retain(|m| m.kind != ModuleKind(4000));
        e
    };
    h.go_to(&mut e, 0).unwrap();
    assert_eq!(strip(e.clone()), strip(start));
    h.go_to(&mut e, 5).unwrap();
    assert_eq!(strip(e.clone()), strip(end));
    let blob = codec::encode_history(&h);
    assert_eq!(
        codec::encode_history(&codec::decode_history(&blob, r).unwrap()),
        blob
    );
}

#[test]
fn styles_and_presets() {
    let r = reg();
    let src = rich_edit();
    let style = Style::from_edit_default("Moody", &src, r);
    assert!(
        style
            .items
            .iter()
            .all(|m| m.kind != ModuleKind::CROP_ORIENTATION)
    );
    // Shapes used by the picked blends travel with the style; unused ones do not.
    let mut ids: Vec<_> = style.masks.iter().map(|m| m.id).collect();
    ids.sort();
    assert_eq!(ids, [1, 2, 3, 4]);
    let (back, _) = codec::decode_style(&codec::encode_style(&style), r).unwrap();
    assert_eq!(codec::encode_style(&back), codec::encode_style(&style));

    // Append onto an edit that already has a shape with id 1 and a labelled sky exposure.
    let mut target = Edit::default_for(r, source(), true);
    target.masks.push(MaskShape {
        id: 1,
        shape: Shape::Linear {
            start: [0.0; 2],
            end: [1.0; 2],
        },
    });
    let mut sky = ModuleInstance::with_params(Exposure::default(), 3);
    sky.label = Some("sky".into());
    target.insert(r, sky);
    let crop_before = target.module(ModuleKind::CROP_ORIENTATION, 0).cloned();
    let only_exposures = Style::from_edit("e", &src, |m| m.kind == ModuleKind::EXPOSURE);
    only_exposures.apply(&mut target, r, ApplyMode::Append);
    // The labelled one replaced instance 3; the unlabelled one replaced instance 0.
    let ex: Vec<_> = target
        .modules
        .iter()
        .filter(|m| m.kind == ModuleKind::EXPOSURE)
        .collect();
    assert_eq!(ex.len(), 2);
    let sky = target.module(ModuleKind::EXPOSURE, 3).unwrap();
    assert_eq!(
        sky.params,
        src.module(ModuleKind::EXPOSURE, 1).unwrap().params
    );
    // Its shapes got fresh ids; the target's own shape 1 is untouched.
    let mut used = Vec::new();
    sky.blend.as_ref().unwrap().mask.shape_ids(&mut used);
    assert!(!used.contains(&1));
    assert_eq!(
        target.mask(1).unwrap().shape,
        Shape::Linear {
            start: [0.0; 2],
            end: [1.0; 2]
        }
    );
    for id in used {
        assert!(
            src.masks
                .iter()
                .any(|s| s.shape == target.mask(id).unwrap().shape)
        );
    }
    assert_eq!(
        target.module(ModuleKind::CROP_ORIENTATION, 0).cloned(),
        crop_before
    );

    // Replace: every colour calibration goes, the style's comes in with its reference
    // following the exposure instance it was pasted to.
    let style = Style::from_edit("cc", &src, |m| {
        matches!(
            m.kind,
            ModuleKind::COLOUR_CALIBRATION | ModuleKind::EXPOSURE
        )
    });
    let mut t2 = Edit::default_for(r, source(), true);
    t2.insert(
        r,
        ModuleInstance::with_params(ColourCalibration::default(), 1),
    );
    style.apply(&mut t2, r, ApplyMode::Replace);
    let cc: Vec<_> = t2
        .modules
        .iter()
        .filter(|m| m.kind == ModuleKind::COLOUR_CALIBRATION)
        .collect();
    assert_eq!(cc.len(), 1);
    let sky = t2
        .modules
        .iter()
        .find(|m| m.label.as_deref() == Some("sky"))
        .unwrap();
    let mut found = false;
    fn refs(e: &MaskExpr, out: &mut Vec<(ModuleKind, u16)>) {
        match e {
            MaskExpr::FromModule { kind, instance } => out.push((*kind, *instance)),
            MaskExpr::Invert(a) => refs(a, out),
            MaskExpr::Combine(_, a, b) => {
                refs(a, out);
                refs(b, out);
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    refs(&cc[0].blend.as_ref().unwrap().mask, &mut out);
    for (k, i) in out {
        found |= k == ModuleKind::EXPOSURE && i == sky.instance;
    }
    assert!(found);
    assert!(t2.in_recommended_order(r));

    // Presets overlay their fields only, and survive storage.
    let preset = Preset {
        kind: ModuleKind::VIEW_TRANSFORM,
        name: "Punchy".into(),
        version: 1,
        fields: Fields::new().with("contrast", Value::Float(2.0)),
    };
    let preset = codec::decode_preset(&codec::encode_preset(&preset), r).unwrap();
    let mut m = ModuleInstance::with_params(
        ViewTransform {
            skew: 0.3,
            ..ViewTransform::default()
        },
        0,
    );
    assert!(preset.apply_to(&mut m));
    let Params::ViewTransform(v) = &m.params else {
        panic!()
    };
    assert_eq!((v.contrast, v.skew), (2.0, 0.3));
    let mut other = ModuleInstance::with_params(CropOrientation::default(), 0);
    assert!(!preset.apply_to(&mut other));
}

// ---------------------------------------------------------------------------------------------
// Migrations: a sample module whose v1 had `ev` in stops and `clip` as a flag; v2 renamed
// `ev` to `exposure_ev` and made the flag a mode; v3 halved a `strength` scale.

const SAMPLE: ModuleKind = ModuleKind(900);

fn v1_to_v2(f: &mut Fields) {
    f.rename("ev", "exposure_ev");
    let clip = matches!(f.remove("clip"), Some(Value::Bool(true)));
    f.set(
        "highlights",
        Value::Text(if clip { "clip" } else { "opposed" }.into()),
    );
}

fn v2_to_v3(f: &mut Fields) {
    let s = f.f32_or("strength", 1.0);
    f.set("strength", Value::Float(s / 2.0));
}

static MIGRATIONS: [Migration; 2] = [
    Migration {
        from: 1,
        run: v1_to_v2,
    },
    Migration {
        from: 2,
        run: v2_to_v3,
    },
];

fn sample_registry() -> Registry {
    let mut r = reg().clone();
    r.register(ModuleSpec {
        kind: SAMPLE,
        name: "sample",
        stage: Stage::Scene,
        rank: 500,
        current: 3,
        live: &[2, 3],
        migrations: &MIGRATIONS,
        geometry: false,
        defaults: || Fields::new().with("exposure_ev", Value::Float(0.0)),
    })
    .unwrap();
    r
}

/// An edit holding one sample module at version 1, as an older Specular wrote it. Frozen:
/// this is also the format-stability fixture.
const FIXTURE_V1: &str = "535044450100010072ff75010000000015cdc88cd531021800070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d900012b84030100000001036f6c640103026576020000c03f04636c6970000108737472656e67746802cdcc4c3f000000";

fn v1_edit() -> Edit {
    let mut e = Edit::empty(source());
    e.modules.push(ModuleInstance {
        kind: SAMPLE,
        version: 1,
        instance: 0,
        label: Some("old".into()),
        enabled: true,
        params: Params::Other(
            SAMPLE,
            Fields::new()
                .with("ev", Value::Float(1.5))
                .with("clip", Value::Bool(true))
                .with("strength", Value::Float(0.8)),
        ),
        blend: None,
    });
    e
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn migration_fixture_v1_to_v2() {
    let bytes = codec::encode_edit(&v1_edit());
    assert_eq!(
        hex_bytes(&bytes),
        FIXTURE_V1,
        "the v1 fixture's bytes changed"
    );
    let r = sample_registry();
    let (e, report) = codec::decode_edit(&unhex(FIXTURE_V1), &r).unwrap();
    // v1 has no live kernel: it moves to v2, the nearest live version, not to v3.
    assert_eq!(
        report.migrated,
        [migrate::Migrated {
            kind: SAMPLE,
            instance: 0,
            from: 1,
            to: 2
        }]
    );
    assert_eq!(
        report.notices(),
        [migrate::Notice {
            id: NOTICE_MIGRATED,
            count: 1
        }]
    );
    let m = &e.modules[0];
    assert_eq!(m.version, 2);
    assert_eq!(m.label.as_deref(), Some("old"));
    let f = m.params.to_fields();
    assert_eq!(f.get("exposure_ev"), Some(&Value::Float(1.5)));
    assert_eq!(f.text("highlights"), Some("clip"));
    assert_eq!(f.get("strength"), Some(&Value::Float(0.8)));
    assert!(f.get("ev").is_none() && f.get("clip").is_none());
    assert!(migrate::renders(&r, m));

    // v2 is live: it loads untouched and offers "Update to current" as an undoable step.
    let (again, report) = codec::decode_edit(&codec::encode_edit(&e), &r).unwrap();
    assert!(report.is_empty());
    assert!(migrate::has_updates(&r, &again));
    let mut cur = again.clone();
    let mut h = History::default();
    let step = migrate::update_step(&r, &cur, SAMPLE, 0, now()).unwrap();
    h.push(&mut cur, step).unwrap();
    assert_eq!(cur.modules[0].version, 3);
    assert_eq!(
        cur.modules[0].params.to_fields().get("strength"),
        Some(&Value::Float(0.4))
    );
    assert!(!migrate::has_updates(&r, &cur));
    assert!(migrate::update_step(&r, &cur, SAMPLE, 0, now()).is_none());
    h.undo(&mut cur).unwrap();
    assert_eq!(cur, again);

    // Without the sample's spec the module is kept unrendered and written back unchanged.
    let (plain, report) = codec::decode_edit(&unhex(FIXTURE_V1), reg()).unwrap();
    assert_eq!(
        report.notices(),
        [migrate::Notice {
            id: NOTICE_NEWER,
            count: 1
        }]
    );
    assert_eq!(codec::encode_edit(&plain), unhex(FIXTURE_V1));
}

#[test]
fn newer_and_broken_versions_are_kept() {
    let r = sample_registry();
    for v in [0, 4] {
        let mut e = v1_edit();
        e.modules[0].version = v;
        let bytes = codec::encode_edit(&e);
        let (back, report) = codec::decode_edit(&bytes, &r).unwrap();
        assert_eq!(report.not_rendered, [(SAMPLE, 0, v)]);
        assert!(!migrate::renders(&r, &back.modules[0]));
        assert_eq!(codec::encode_edit(&back), bytes);
    }
    // A typed kind at a newer version stays fields through presets and undo.
    let mut e = Edit::empty(source());
    e.modules.push(ModuleInstance {
        version: 9,
        params: Params::Other(
            ModuleKind::EXPOSURE,
            Fields::new().with("future", Value::Int(1)),
        ),
        ..ModuleInstance::with_params(Exposure::default(), 0)
    });
    let (mut e, _) = codec::decode_edit(&codec::encode_edit(&e), &r).unwrap();
    let preset = Preset {
        kind: ModuleKind::EXPOSURE,
        name: "p".into(),
        version: 1,
        fields: Fields::new().with("exposure_ev", Value::Float(1.0)),
    };
    preset.apply_to(&mut e.modules[0]);
    let mut h = History::default();
    let f = e.modules[0].params.to_fields();
    let g = f.clone().with("exposure_ev", Value::Float(2.0));
    h.push(&mut e, Step::params(ModuleKind::EXPOSURE, 0, &f, &g, now()))
        .unwrap();
    h.undo(&mut e).unwrap();
    let Params::Other(_, f) = &e.modules[0].params else {
        panic!("typed a version this build does not render")
    };
    assert_eq!(f.get("future"), Some(&Value::Int(1)));
    assert_eq!(f.get("exposure_ev"), Some(&Value::Float(1.0)));

    // A preset written for v1 migrates to the current version on load.
    let p = Preset {
        kind: SAMPLE,
        name: "old".into(),
        version: 1,
        fields: Fields::new().with("ev", Value::Float(1.0)),
    };
    let p = codec::decode_preset(&codec::encode_preset(&p), &r).unwrap();
    assert_eq!(p.version, 3);
    assert_eq!(p.fields.get("exposure_ev"), Some(&Value::Float(1.0)));
    assert_eq!(p.fields.get("strength"), Some(&Value::Float(0.5)));
}

// ---------------------------------------------------------------------------------------------
// Hashes

fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn hashes_are_stable() {
    let e = Edit::default_for(reg(), source(), true);
    let got: Vec<_> = e
        .modules
        .iter()
        .map(|m| hex(&hash::module_hash(&e, m)))
        .collect();
    let prefix: Vec<_> = hash::prefix_hashes(&e).iter().map(hex).collect();
    let expected_modules = [
        "05670b490c9535181cb79130368b3f6ed6f8a0a647b3fca7c8f0b4213bc8bbf9",
        "a3e754dbec573de1966880b93db552ee309416916f612a1af4bdc6c94d13697a",
        "7f1e1ea504e129a833ec43bfb3412064c4947ed5377ec8527cf81d282d62f2e6",
        "744cd3a08a45239118c9ccd938eeae0eb35809f785471e31f752bc2f5e2c9f82",
        "fb2bbd95568404c5c6b357ac6155f3aea53509f2e77d035c39de3437a9973fa7",
    ];
    let expected_prefix = [
        "9ad8dfef96208d167d818596e386e26b8c381dc0e944fcd6e426cfac686eb567",
        "e1e67f96a3db07b5903e74687b8da4ba5dbe67ac59a89c3ec2cd203f8f79a445",
        "081b6a550d6a500ceadeee8027ddb46f8fd2e590b508662392aaa261bd4d8690",
        "9895ee9b1c9e16e503b1a61dbc9975b1af9541aa4297a981586426eda3431d7f",
        "71d11b359b1107155fe888c42d4d3aea81e698a9a8107c02acc0341c9f9459e3",
    ];
    assert_eq!(got, expected_modules);
    assert_eq!(prefix, expected_prefix);
    assert_eq!(hex(&hash::edit_hash(&e)), expected_prefix[4]);
    // A borrowed mask's hash includes its source's refinement (the rich edit
    // borrows a refined mask).
    assert_eq!(
        hex(&hash::edit_hash(&rich_edit())),
        "a7ade09d2831ba295cf02ee5b1ddd65abfe07f9307902b3b20efc02644c80967"
    );
}

#[test]
fn hashes_follow_pixels_only() {
    let r = reg();
    let e = rich_edit();
    let base = hash::prefix_hashes(&e);
    let at = |e: &Edit, k, i| e.position(k, i).unwrap();
    let sky = at(&e, ModuleKind::EXPOSURE, 1);

    // Labels, instance numbers, field order and the sign of zero do not count.
    let mut same = e.clone();
    same.modules[sky].label = Some("other".into());
    let p = same.position(ModuleKind::VIEW_TRANSFORM, 0).unwrap();
    same.modules[p].params = Params::Other(ModuleKind::VIEW_TRANSFORM, {
        let mut f = e.modules[p].params.to_fields();
        f.0.reverse();
        f
    });
    let c = same.position(ModuleKind::COLOUR_CALIBRATION, 0).unwrap();
    if let Params::ColourCalibration(cc) = &mut same.modules[c].params {
        cc.mixer[1] = -0.0;
    }
    assert_eq!(hash::prefix_hashes(&same), base);

    // Renumbering shapes (as a paste does) does not count; changing one does, from the first
    // module that uses it on, and so does a referenced module's mask.
    let mut renum = e.clone();
    for s in &mut renum.masks {
        s.id += 100;
    }
    fn bump(x: &mut MaskExpr) {
        match x {
            MaskExpr::Shape(id) => *id += 100,
            MaskExpr::Invert(a) => bump(a),
            MaskExpr::Combine(_, a, b) => {
                bump(a);
                bump(b);
            }
            _ => {}
        }
    }
    for m in &mut renum.modules {
        if let Some(b) = &mut m.blend {
            bump(&mut b.mask);
        }
    }
    assert_eq!(hash::prefix_hashes(&renum), base);
    let mut moved = e.clone();
    moved.masks[0].shape = Shape::Ellipse {
        centre: [0.41, 0.5],
        radii: [0.2, 0.1],
        angle_deg: 15.0,
        feather: 0.05,
    };
    let cc = at(&e, ModuleKind::COLOUR_CALIBRATION, 0);
    let mh = |e: &Edit, i: usize| hash::module_hash(e, &e.modules[i]);
    assert_ne!(mh(&moved, sky), mh(&e, sky));
    // Colour calibration uses the sky exposure's mask, which uses shape 1.
    assert_ne!(mh(&moved, cc), mh(&e, cc));
    let p = hash::prefix_hashes(&moved);
    let first = sky.min(cc);
    assert_eq!(p[..first], base[..first]);
    assert!(p[first..].iter().zip(&base[first..]).all(|(a, b)| a != b));

    // A change late in the stack leaves earlier prefixes alone; disabling repeats the hash.
    let mut late = e.clone();
    let v = at(&e, ModuleKind::VIEW_TRANSFORM, 0);
    late.modules[v].enabled = false;
    let p = hash::prefix_hashes(&late);
    assert_eq!(p[..v], base[..v]);
    assert_eq!(p[v], p[v - 1]);
    // The source and process version count.
    let mut other = e.clone();
    other.source.size += 1;
    assert_ne!(hash::edit_hash(&other), hash::edit_hash(&e));
    let mut proc = e.clone();
    proc.process += 1;
    assert_ne!(hash::edit_hash(&proc), hash::edit_hash(&e));
    // A self-referencing mask terminates.
    let mut cyc = e.clone();
    cyc.modules[sky].blend.as_mut().unwrap().mask = MaskExpr::FromModule {
        kind: ModuleKind::EXPOSURE,
        instance: 1,
    };
    let _ = hash::edit_hash(&cyc);
    let _ = r;
}

mod mask_fields;

#[test]
fn a_pasted_grain_takes_the_target_photos_seed() {
    let r = reg();
    let mut src = Edit::default_for(r, source(), true);
    let mut g = ModuleInstance::new(r, ModuleKind::GRAIN, 0);
    g.params = Params::from_fields(
        ModuleKind::GRAIN,
        g.params
            .to_fields()
            .with("amount", Value::Float(0.4))
            .with("seed", Value::Int(111)),
    );
    src.insert(r, g);
    let style = Style::from_kinds("Grainy", &src, &[ModuleKind::GRAIN]);
    for mode in [ApplyMode::Append, ApplyMode::Replace] {
        let mut target = Edit::default_for(r, source(), true);
        style.with_grain_seed(222).apply(&mut target, r, mode);
        let f = target
            .module(ModuleKind::GRAIN, 0)
            .unwrap()
            .params
            .to_fields();
        assert_eq!(f.i64_or("seed", 0), 222, "{mode:?}");
        assert_eq!(f.f32_or("amount", 0.0), 0.4, "{mode:?}");
    }
    // The style itself keeps its seed.
    let f = style.items[0].params.to_fields();
    assert_eq!(f.i64_or("seed", 0), 111);
}
