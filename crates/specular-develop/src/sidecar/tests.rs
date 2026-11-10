use super::*;
use crate::model::mask::Combine;
use crate::model::params::{Exposure, ViewTransform};
use crate::model::{Blend, BlendMode, MaskExpr, MaskShape, Refine, Shape, Snapshot, Step};

fn source() -> SourceKey {
    SourceKey {
        size: 24_510_322,
        mtime_ns: 1_730_000_000_123_456_789,
        head_tail: std::array::from_fn(|i| i as u8 * 7),
    }
}

/// An edit with every part the sidecar writes.
fn edit() -> Edit {
    let r = Registry::builtin();
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
            shape: Shape::Linear {
                start: [0.0, 0.0],
                end: [0.0, 0.4],
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
    second.label = Some("sky & \"clouds\" <1>".into());
    second.blend = Some(Blend {
        mode: BlendMode::Luminance,
        opacity: 0.75,
        invert: false,
        mask: MaskExpr::Combine(
            Combine::Union,
            Box::new(MaskExpr::Shape(1)),
            Box::new(MaskExpr::Shape(2)),
        ),
        refine: Refine::default(),
    });
    e.insert(r, second);
    // A field-only module and a kind from a newer Specular.
    let mut te = ModuleInstance::new(r, ModuleKind::TONE_EQUALIZER, 0);
    te.params = Params::from_fields(
        ModuleKind::TONE_EQUALIZER,
        Fields::new()
            .with("highlights_ev", Value::Float(-1.25))
            .with("shadows_ev", Value::Float(0.5)),
    );
    e.insert(r, te);
    let mut curves = ModuleInstance::new(r, ModuleKind::CURVES, 0);
    curves.params = Params::from_fields(
        ModuleKind::CURVES,
        Fields::new().with("points", Value::Floats(vec![0.0, 0.0, 0.3, 0.25, 1.0, 1.0])),
    );
    curves.enabled = false;
    e.insert(r, curves);
    e.modules.push(ModuleInstance {
        kind: ModuleKind(900),
        version: 3,
        instance: 0,
        label: None,
        enabled: true,
        params: Params::Other(
            ModuleKind(900),
            Fields::new()
                .with("mode", Value::Text("glow".into()))
                .with("radius", Value::Int(12))
                .with("weights", Value::Floats(vec![0.25, 1e-7, -3.5e9])),
        ),
        blend: None,
    });
    let m = e.module_mut(ModuleKind::VIEW_TRANSFORM, 0).unwrap();
    m.params = Params::ViewTransform(ViewTransform {
        contrast: 1.7,
        white_target: 1.1,
        ..ViewTransform::default()
    });
    let snap = e.modules.clone();
    e.snapshots.push(Snapshot {
        name: "before sky".into(),
        modules: snap,
        masks: Vec::new(),
    });
    e
}

fn history(e: &mut Edit) -> History {
    let mut h = History::default();
    let before = e
        .module(ModuleKind::EXPOSURE, 0)
        .unwrap()
        .params
        .to_fields();
    let mut after = before.clone();
    after.set("exposure_ev", Value::Float(0.3));
    h.push(
        e,
        Step::params(ModuleKind::EXPOSURE, 0, &before, &after, 1_730_000_000_000),
    )
    .unwrap();
    h
}

#[test]
fn round_trip_keeps_the_edit_its_hash_and_its_history() {
    let mut e = edit();
    let h = history(&mut e);
    let text = to_xmp(&e, Some(&h));
    let back = from_xmp(&text, Registry::builtin()).expect("read");
    assert_eq!(back.edit, e);
    assert_eq!(edit_hash(&back.edit), edit_hash(&e));
    assert_eq!(back.history, h);
    assert!(back.report.migrated.is_empty());
    // Readable: named modules and fields.
    assert!(text.contains("spd:module=\"exposure\""));
    assert!(text.contains("spd:exposure_ev=\"0.3\""));
    assert!(text.contains("spd:module=\"kind_900\""));
    assert!(text.contains(&format!("spd:editHash=\"{}\"", hex(&edit_hash(&e)))));
}

#[test]
fn values_have_one_text_form() {
    for v in [
        Value::Bool(true),
        Value::Int(-42),
        Value::Float(1.0),
        Value::Float(0.1),
        Value::Float(-3.4e-12),
        Value::Text("cat16".into()),
        Value::Floats(vec![0.5, 2.0]),
    ] {
        assert_eq!(parse_value(&value_text(&v), None), v);
        assert_eq!(parse_value(&value_text(&v), Some(&v)), v);
    }
    // Defaults decide the type where the form alone cannot.
    assert_eq!(
        parse_value("0.5", Some(&Value::Floats(vec![]))),
        Value::Floats(vec![0.5])
    );
    assert_eq!(
        parse_value("12", Some(&Value::Text(String::new()))),
        Value::Text("12".into())
    );
}

#[test]
fn written_and_read_on_disk_next_to_the_image() {
    let dir = std::env::temp_dir().join(format!("specular-sidecar-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("IMG_1234.CR2");
    assert_eq!(
        path_for(&img).file_name().unwrap(),
        "IMG_1234.CR2.specular.xmp"
    );
    assert!(read(&img, Registry::builtin()).unwrap().is_none());
    let e = edit();
    let p = write(&img, &e, None).unwrap();
    assert_eq!(p, path_for(&img));
    let back = read(&img, Registry::builtin()).unwrap().unwrap();
    assert_eq!(back.edit, e);
    assert!(back.history.steps.is_empty());
    // No temporary file left behind.
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|d| d.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

fn bump(f: &mut Fields) {
    let v = f.f32_or("strength", 1.0);
    f.set("strength", Value::Float(v * 2.0));
}

static BUMP: [crate::model::kind::Migration; 1] =
    [crate::model::kind::Migration { from: 1, run: bump }];

#[test]
fn a_migrating_load_starts_a_fresh_history() {
    // A registry where a module's params moved to version 2.
    let kind = ModuleKind(0xFE10);
    let mut v1 = Registry::builtin().clone();
    let spec = |current: u16, live: &'static [u16]| crate::model::ModuleSpec {
        kind,
        name: "sample",
        stage: crate::model::Stage::Scene,
        rank: 450,
        current,
        live,
        migrations: &BUMP,
        geometry: false,
        defaults: Fields::new,
    };
    v1.register(spec(1, &[1])).unwrap();
    let mut v2 = Registry::builtin().clone();
    v2.register(spec(2, &[2])).unwrap();
    let mut e = Edit::default_for(Registry::builtin(), source(), false);
    e.modules.push(ModuleInstance {
        kind,
        version: 1,
        instance: 0,
        label: None,
        enabled: true,
        params: Params::Other(kind, Fields::new().with("strength", Value::Float(0.25))),
        blend: None,
    });
    let h = history(&mut e);
    let text = to_xmp(&e, Some(&h));
    let same = from_xmp(&text, &v1).expect("read");
    assert_eq!(same.history, h);
    let newer = from_xmp(&text, &v2).expect("read");
    assert_eq!(newer.report.migrated.len(), 1);
    assert!(newer.history.steps.is_empty());
    let m = newer.edit.module(kind, 0).unwrap();
    assert_eq!(m.version, 2);
    assert_eq!(m.params.to_fields().f32_or("strength", 0.0), 0.5);
}

#[test]
fn damaged_sidecars_say_why() {
    let r = Registry::builtin();
    assert!(matches!(from_xmp("not xml", r), Err(Error::Format(_))));
    assert!(matches!(
        from_xmp("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>", r),
        Err(Error::Format(_))
    ));
    let text = to_xmp(&edit(), None).replace("spd:centre=\"", "spd:centre=\"!!");
    // A shape that does not read is left out, and the file is marked as not to be overwritten.
    let l = from_xmp(&text, r).unwrap();
    assert!(l.partial);
    assert!(!keepable(&text));
    // A DTD is refused (no entity expansion).
    let dtd = format!("<!DOCTYPE x [<!ENTITY a \"b\">]>{}", to_xmp(&edit(), None));
    assert!(from_xmp(&dtd, r).is_err());
}
