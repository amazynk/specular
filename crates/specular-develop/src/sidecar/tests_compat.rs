//! The frozen format, forward compatibility and other tools' content.

use super::*;
use crate::model::mask::{PathPoint, Stroke};
use crate::model::{Blend, BlendMode, MaskExpr, MaskShape, Refine, Shape, Snapshot, Step};
use crate::ops::colour;

const FIXTURE: &str = "tests/fixtures/sidecar/every-module.specular.xmp";
const FOREIGN: [(&str, &str); 4] = [
    (
        "lightroom",
        include_str!("../../tests/fixtures/xmp/lightroom.xmp"),
    ),
    (
        "darktable",
        include_str!("../../tests/fixtures/xmp/darktable.xmp"),
    ),
    (
        "digikam",
        include_str!("../../tests/fixtures/xmp/digikam.xmp"),
    ),
    (
        "captureone",
        include_str!("../../tests/fixtures/xmp/captureone.xmp"),
    ),
];

fn source() -> SourceKey {
    SourceKey {
        size: 31_457_280,
        mtime_ns: 1_761_998_400_000_000_000,
        head_tail: std::array::from_fn(|i| (i as u8).wrapping_mul(29)),
    }
}

/// Fields a module the panel drives starts from, where the registry has none.
fn start_fields(kind: ModuleKind) -> Option<Fields> {
    let f = |n: &str, v: f32| Fields::new().with(n, Value::Float(v));
    Some(match kind {
        ModuleKind::TONE_EQUALIZER => {
            f("highlights_ev", -0.75).with("shadows_ev", Value::Float(0.5))
        }
        ModuleKind::LOCAL_CONTRAST => f("texture", 0.2).with("clarity", Value::Float(0.1)),
        ModuleKind::COLOUR_BALANCE => colour::balance::ColourBalance::defaults(),
        ModuleKind::COLOUR_EQUALIZER => colour::equalizer::ColourEqualizer::defaults(),
        ModuleKind::POINT_COLOUR => colour::point::PointColour::defaults(),
        ModuleKind::BLACK_WHITE => colour::bw::BlackWhite::defaults(),
        ModuleKind::LUT_3D => colour::lut::Lut3d::defaults(),
        ModuleKind::DIFFUSE => crate::ops::detail::diffuse::Preset::Deblur
            .params()
            .to_fields(),
        ModuleKind::SURFACE_BLUR => crate::ops::detail::surface::SurfaceBlur::default_fields()
            .with("amount", Value::Float(0.4))
            .with("radius", Value::Float(0.002)),
        ModuleKind::RETOUCH => {
            use crate::ops::retouch::{Layer, Retouch, Spot, SpotMode};
            Retouch {
                split_radius: 6.0,
                spots: vec![
                    Spot {
                        target: [0.3, 0.4],
                        source: [0.35, 0.5],
                        ..Spot::default()
                    },
                    Spot {
                        mode: SpotMode::Blur,
                        layer: Layer::Low,
                        target: [0.6, 0.3],
                        source: [0.6, 0.3],
                        size: 0.01,
                        feather: 0.8,
                        opacity: 0.6,
                        blur: 0.002,
                        path: vec![[0.55, 0.3], [0.6, 0.32], [0.65, 0.29]],
                        model: String::new(),
                    },
                    // A removal keeps its stroke and model, not the fill.
                    Spot {
                        mode: SpotMode::Remove,
                        target: [0.7, 0.6],
                        source: [0.75, 0.6],
                        size: 0.015,
                        path: vec![[0.68, 0.6], [0.72, 0.61]],
                        model: "big-lama".into(),
                        ..Spot::default()
                    },
                ],
            }
            .to_fields()
        }
        // The AI mode's fields (keys are never stored).
        ModuleKind::DENOISE => crate::ops::detail::denoise::Denoise::default_fields()
            .with("luminance", Value::Float(0.2))
            .with("mode", Value::Text("ai".into()))
            .with("model", Value::Text("ffdnet-color".into()))
            .with("blend", Value::Float(0.75)),
        ModuleKind::CURVES => Fields::new().with(
            "points",
            Value::Floats(vec![0.0, 0.0, 0.25, 0.2, 0.75, 0.8, 1.0, 1.0]),
        ),
        _ => return None,
    })
}

/// One shape of every kind.
fn every_shape() -> Vec<MaskShape> {
    vec![
        MaskShape {
            id: 1,
            shape: Shape::Radial {
                centre: [0.5, 0.4],
                inner: [0.1, 0.1],
                outer: [0.3, 0.25],
                angle_deg: 10.0,
            },
        },
        MaskShape {
            id: 2,
            shape: Shape::Ellipse {
                centre: [0.4, 0.5],
                radii: [0.2, 0.1],
                angle_deg: 15.0,
                feather: 0.05,
            },
        },
        MaskShape {
            id: 3,
            shape: Shape::Rectangle {
                centre: [0.5, 0.5],
                half: [0.25, 0.125],
                angle_deg: -5.0,
                round: 0.1,
                feather: 0.03,
            },
        },
        MaskShape {
            id: 4,
            shape: Shape::Polygon {
                points: vec![
                    PathPoint {
                        x: 0.1,
                        y: 0.1,
                        feather: 0.01,
                    },
                    PathPoint {
                        x: 0.9,
                        y: 0.15,
                        feather: 0.02,
                    },
                    PathPoint {
                        x: 0.5,
                        y: 0.8,
                        feather: 0.0,
                    },
                ],
                closed: true,
            },
        },
        MaskShape {
            id: 5,
            shape: Shape::Linear {
                start: [0.0, 0.0],
                end: [0.0, 0.4],
            },
        },
        MaskShape {
            id: 6,
            shape: Shape::Brush {
                strokes: vec![
                    Stroke {
                        radius: 0.03,
                        hardness: 0.7,
                        flow: 0.9,
                        erase: false,
                        pressure: crate::model::mask::PressureUse::Flow,
                        points: vec![[0.2, 0.2, 1.0], [0.25, 0.22, 0.8], [0.3, 0.25, 0.6]],
                    },
                    // Pressure on the size, an erasing stroke.
                    Stroke {
                        radius: 0.02,
                        hardness: 0.5,
                        flow: 1.0,
                        erase: true,
                        pressure: crate::model::mask::PressureUse::Size,
                        points: vec![[0.22, 0.2, 0.3], [0.26, 0.21, 0.9]],
                    },
                ],
            },
        },
    ]
}

/// A mask expression using every kind of item.
fn every_expr() -> MaskExpr {
    use crate::model::mask::{ColourRange, Combine, Range, RangeChannel};
    let b = Box::new;
    MaskExpr::Combine(
        Combine::Union,
        b(MaskExpr::Combine(
            Combine::Subtract,
            b(MaskExpr::Combine(
                Combine::Intersect,
                b(MaskExpr::Shape(1)),
                b(MaskExpr::Range(Range {
                    channel: RangeChannel::Luminance,
                    on_output: false,
                    limits: [0.0, 0.1, 0.6, 0.8],
                })),
            )),
            b(MaskExpr::Opacity(
                0.5,
                b(MaskExpr::Invert(b(MaskExpr::Shape(3)))),
            )),
        )),
        b(MaskExpr::Combine(
            Combine::Exclude,
            b(MaskExpr::ColourRange(ColourRange {
                colours: vec![[0.2, 0.4, 0.8], [0.1, 0.3, 0.7]],
                distance: 0.1,
                falloff: 0.05,
            })),
            b(MaskExpr::Combine(
                Combine::Union,
                b(MaskExpr::FromModule {
                    kind: ModuleKind::TONE_EQUALIZER,
                    instance: 0,
                }),
                b(MaskExpr::Combine(
                    Combine::Union,
                    b(MaskExpr::Ai(crate::model::mask::AiMask {
                        kind: crate::model::mask::AiKind::Object,
                        model: "efficient \"sam\" (ti)".into(),
                        points: vec![
                            crate::model::mask::AiPoint {
                                x: 0.25,
                                y: 0.5,
                                positive: true,
                            },
                            crate::model::mask::AiPoint {
                                x: 0.75,
                                y: 0.125,
                                positive: false,
                            },
                        ],
                        rect: Some([0.1, 0.2, 0.6, 0.9]),
                        // Keys are not in the sidecar.
                        key: [0; 32],
                    })),
                    b(MaskExpr::Combine(
                        Combine::Intersect,
                        b(MaskExpr::All),
                        b(MaskExpr::Combine(
                            Combine::Subtract,
                            b(MaskExpr::Details(crate::model::mask::Details {
                                threshold: 0.3,
                                softness: 0.15,
                            })),
                            // A depth range.
                            b(MaskExpr::Depth(crate::model::mask::DepthRange {
                                model: "depth-anything-v2-small".into(),
                                near: 0.6,
                                far: 1.0,
                                feather: 0.05,
                            })),
                        )),
                    )),
                )),
            )),
        )),
    )
}

#[test]
fn mask_expressions_read_back_and_refuse_nonsense() {
    let r = Registry::builtin();
    let e = every_expr();
    let t = masks::expr_text(&e, r);
    assert!(t.starts_with("(union (subtract (intersect (shape 1) (range luminance input"));
    assert!(t.contains("(module tone_equalizer 0)"));
    assert_eq!(masks::parse_expr(&t, r).unwrap(), e);
    for bad in [
        "",
        "(",
        "(all",
        "(shape x)",
        "(glow 1)",
        "(all) (all)",
        "(range red side 0 0 0 0)",
        "(ai sky 00)",
    ] {
        assert!(masks::parse_expr(bad, r).is_err(), "{bad}");
    }
}

/// An edit with a module of every kind this build knows, a second instance with a label, a
/// blend and masks, a kind from a newer Specular, and a snapshot.
fn every_module() -> Edit {
    let r = Registry::builtin();
    let mut e = Edit::empty(source());
    for spec in r.specs() {
        let mut m = ModuleInstance::new(r, spec.kind, 0);
        if let Some(f) = start_fields(spec.kind) {
            m.params = Params::from_fields(spec.kind, f);
        }
        m.enabled = spec.kind != ModuleKind::BLACK_WHITE;
        e.insert(r, m);
    }
    e.masks = every_shape();
    let mut sky = ModuleInstance::new(r, ModuleKind::EXPOSURE, 1);
    sky.params = Params::from_fields(
        ModuleKind::EXPOSURE,
        sky.params
            .to_fields()
            .with("exposure_ev", Value::Float(-0.4)),
    );
    sky.label = Some("Sky".into());
    sky.blend = Some(Blend {
        mode: BlendMode::SoftLight,
        opacity: 0.8,
        invert: true,
        mask: every_expr(),
        refine: Refine {
            feather: 0.02,
            blur: 1.5,
            contrast: 0.25,
            offset: -0.1,
            edge_aware: 0.5,
            details: 0.3,
            radius: 0.035,
        },
    });
    let mut plain = ModuleInstance::new(r, ModuleKind::LOCAL_CONTRAST, 1);
    plain.blend = Some(Blend {
        mask: MaskExpr::Shape(2),
        ..Blend::default()
    });
    e.insert(r, plain);
    e.insert(r, sky);
    e.modules.push(ModuleInstance {
        kind: ModuleKind(900),
        version: 2,
        instance: 0,
        label: None,
        enabled: true,
        params: Params::Other(
            ModuleKind(900),
            Fields::new().with("glow", Value::Float(0.25)),
        ),
        blend: None,
    });
    let snap = Snapshot::take("Before the sky", &e);
    e.snapshots.push(snap);
    e
}

/// `text` with its writer replaced, so the fixture holds over version bumps.
fn no_writer(text: &str) -> String {
    text.replace(
        &format!("spd:writer=\"{WRITER}\""),
        "spd:writer=\"Specular\"",
    )
}

fn manifest(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(p)
}

#[test]
fn the_fixture_of_every_module_kind_is_the_format() {
    let e = every_module();
    let text = no_writer(&to_xmp(&e, None));
    let path = manifest(FIXTURE);
    if std::env::var_os("SPECULAR_UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
    }
    let fixture =
        std::fs::read_to_string(&path).expect("fixture (SPECULAR_UPDATE_FIXTURES=1 writes it)");
    assert_eq!(
        text, fixture,
        "the sidecar format changed: see docs/sidecar-format.md"
    );
    // Every kind is in it, by name.
    for spec in Registry::builtin().specs() {
        assert!(
            fixture.contains(&format!("spd:module=\"{}\"", spec.name)),
            "{}",
            spec.name
        );
    }
    let back = from_xmp(&fixture, Registry::builtin()).unwrap();
    assert_eq!(back.edit, e);
    assert_eq!(back.writer.as_deref(), Some("Specular"));
    assert!(back.unknown_modules.is_empty() && !back.newer_process);
    // Read and written again: the same bytes.
    assert_eq!(
        no_writer(&to_xmp_over(Some(&fixture), &back.edit, None, None)),
        fixture
    );
}

#[test]
fn the_writer_is_recorded() {
    let l = from_xmp(&to_xmp(&every_module(), None), Registry::builtin()).unwrap();
    assert_eq!(l.writer.as_deref(), Some(WRITER));
    assert!(WRITER.starts_with("Specular "));
}

#[test]
fn a_newer_process_is_flagged_and_kept() {
    let mut e = every_module();
    e.process = crate::model::edit::PROCESS + 3;
    let text = to_xmp(&e, None);
    let l = from_xmp(&text, Registry::builtin()).unwrap();
    assert!(l.newer_process);
    assert_eq!(l.edit.process, e.process);
    let again = to_xmp_over(Some(&text), &l.edit, None, None);
    assert!(again.contains(&format!("spd:process=\"{}\"", e.process)));
    // This build's own edits are not flagged.
    let own = from_xmp(&to_xmp(&every_module(), None), Registry::builtin()).unwrap();
    assert!(!own.newer_process);
}

/// A sidecar as a newer Specular and another tool left it: an unknown module between two
/// known ones, an unknown field in a known module, unknown attributes and elements, and
/// other namespaces inside and outside Develop's description.
fn future_sidecar() -> (String, Vec<&'static str>) {
    let base = to_xmp(&every_module(), None);
    let extras = vec![
        r#"spd:future="kept as is""#,
        r#"xmp:Rating="4""#,
        r#"<spd:looks><rdf:Seq><rdf:li spd:look="warm"/></rdf:Seq></spd:looks>"#,
        r#"<dc:subject><rdf:Bag><rdf:li>harbour &amp; boats</rdf:li></rdf:Bag></dc:subject>"#,
        r#"<rdf:li spd:module="glow_v9" spd:version="4" spd:instance="0" spd:enabled="True"><spd:params spd:radius="12.0" spd:mode="soft"/></rdf:li>"#,
        r#"spd:exposure_curve="0.5 0.25""#,
        r#"digiKam:note='li "attr"'"#,
        r#"<spd:note>by hand</spd:note>"#,
    ];
    let t = base
        .replace(
            "xmlns:spd=\"http://ns.specular.app/develop/1/\"",
            "xmlns:spd=\"http://ns.specular.app/develop/1/\"\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    xmlns:digiKam=\"http://www.digikam.org/ns/1.0/\"",
        )
        .replacen("spd:schema=", &format!("{} {} spd:schema=", extras[0], extras[1]), 1)
        .replace(
            "   <spd:modules>",
            &format!("   {}\n   {}\n   <spd:modules>", extras[2], extras[3]),
        );
    // The unknown module goes right before colour_calibration's item.
    let cc = t.find("<rdf:li spd:module=\"colour_calibration\"").unwrap();
    let mut t = format!("{}{}\n     {}", &t[..cc], extras[4], &t[cc..]);
    // An unknown field in exposure's params and an attribute and element on its item.
    let ex = t
        .find("spd:module=\"exposure\" spd:version=\"1\" spd:instance=\"0\"")
        .unwrap();
    let close = ex + t[ex..].find('>').unwrap();
    t.insert_str(close, &format!(" {}", extras[6]));
    let params = ex + t[ex..].find("spd:exposure_ev=").unwrap();
    t.insert_str(params, &format!("{}\n        ", extras[5]));
    let li_end = ex + t[ex..].find("</rdf:li>").unwrap();
    t.insert_str(li_end, &format!("{}\n     ", extras[7]));
    // Another tool's description beside ours.
    let rdf_end = t.find("</rdf:RDF>").unwrap();
    t.insert_str(
        rdf_end,
        " <rdf:Description rdf:about=\"\" xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\" crs:Exposure2012=\"+0.30\"/>\n ",
    );
    (t, extras)
}

#[test]
fn unknown_modules_fields_and_other_tools_survive_a_rewrite() {
    let (old, extras) = future_sidecar();
    let reg = Registry::builtin();
    let l = from_xmp(&old, reg).expect("a newer sidecar reads");
    assert_eq!(l.unknown_modules, vec!["glow_v9".to_owned()]);
    // Unknown modules mean the history is not trusted.
    assert!(l.history.steps.is_empty());
    let mut e = l.edit.clone();
    let m = e.module_mut(ModuleKind::EXPOSURE, 0).unwrap();
    m.params = Params::from_fields(
        ModuleKind::EXPOSURE,
        m.params.to_fields().with("exposure_ev", Value::Float(1.25)),
    );
    let new = to_xmp_over(Some(&old), &e, None, None);
    for x in &extras {
        assert!(new.contains(x), "lost: {x}\n{new}");
    }
    assert!(new.contains("spd:exposure_ev=\"1.25\""));
    // The unknown module keeps its place: after exposure, before colour calibration.
    let at = |s: &str| new.find(s).unwrap_or_else(|| panic!("{s}"));
    assert!(at("spd:module=\"exposure\" spd:version=\"1\" spd:instance=\"0\"") < at("glow_v9"));
    assert!(at("glow_v9") < at("spd:module=\"colour_calibration\""));
    // Outside Develop's description, not a byte changed.
    let (o, n) = (desc_range(&old), desc_range(&new));
    assert_eq!(&old[..o.start], &new[..n.start]);
    assert_eq!(&old[o.end..], &new[n.end..]);
    let back = from_xmp(&new, reg).unwrap();
    assert_eq!(back.edit, e);
    // Written once more, nothing moves.
    assert_eq!(to_xmp_over(Some(&new), &e, None, None), new);
}

#[test]
fn an_older_field_left_behind_by_a_migration_is_dropped() {
    // A field this build does not know, in an item of an older version than it writes.
    let mut e = every_module();
    let text = to_xmp(&e, None).replacen(
        "spd:exposure_ev=",
        "spd:old_name=\"1.0\"\n        spd:exposure_ev=",
        1,
    );
    e.module_mut(ModuleKind::EXPOSURE, 0).unwrap().version = 2;
    let new = to_xmp_over(Some(&text), &e, None, None);
    assert!(!new.contains("spd:old_name"));
}

fn desc_range(text: &str) -> std::ops::Range<usize> {
    let doc = parse_doc(text).unwrap();
    our_description(&doc).unwrap().range()
}

#[test]
fn other_tools_files_are_kept_byte_for_byte() {
    let reg = Registry::builtin();
    let e = every_module();
    for (tool, text) in FOREIGN {
        let first = to_xmp_over(Some(text), &e, None, None);
        let r = desc_range(&first);
        // What was there before and after Develop's description is the file, unchanged.
        let cut = format!("{}{}", &first[..r.start], &first[r.end..]);
        let at = text.rfind("</rdf:RDF>").unwrap();
        assert_eq!(
            cut,
            format!("{}  \n {}", &text[..at], &text[at..]),
            "{tool}"
        );
        let l = from_xmp(&first, reg).unwrap_or_else(|e| panic!("{tool}: {e}"));
        assert_eq!(l.edit, e, "{tool}");
        // A second edit replaces only Develop's part.
        let mut e2 = e.clone();
        e2.snapshots.clear();
        let second = to_xmp_over(Some(&first), &e2, None, None);
        let r2 = desc_range(&second);
        assert_eq!(&second[..r2.start], &first[..r.start], "{tool}");
        assert_eq!(&second[r2.end..], &first[r.end..], "{tool}");
        assert_eq!(from_xmp(&second, reg).unwrap().edit, e2, "{tool}");
    }
}

#[test]
fn dv16_sidecars_still_read() {
    // Older sidecars wrote snapshots as base64 of the codec in an attribute.
    let mut e = every_module();
    let mut legacy = Edit::empty(e.source);
    legacy.snapshots = e.snapshots.clone();
    let attr = format!(
        "spd:snapshots=\"{}\"",
        b64::encode(&codec::encode_edit(&legacy))
    );
    e.snapshots.clear();
    let text = to_xmp(&e, None).replacen("spd:schema=", &format!("{attr} spd:schema="), 1);
    let l = from_xmp(&text, Registry::builtin()).unwrap();
    assert_eq!(l.edit.snapshots, legacy.snapshots);
    // Written again, in the readable form.
    let new = to_xmp_over(Some(&text), &l.edit, None, None);
    assert!(!new.contains("spd:snapshots=\""));
    assert!(new.contains("<spd:snapshots>"));
    assert!(new.contains("spd:name=\"Before the sky\""));
}

#[test]
fn the_history_rides_along_and_reads_back() {
    let mut e = every_module();
    let mut h = History::default();
    let before = e
        .module(ModuleKind::EXPOSURE, 0)
        .unwrap()
        .params
        .to_fields();
    let after = before.clone().with("exposure_ev", Value::Float(0.5));
    h.push(
        &mut e,
        Step::params(ModuleKind::EXPOSURE, 0, &before, &after, 1_761_998_400_000),
    )
    .unwrap();
    let text = to_xmp(&e, Some(&h));
    let l = from_xmp(&text, Registry::builtin()).unwrap();
    assert_eq!(l.history, h);
    assert_eq!(l.edit, e);
}

#[test]
fn write_merges_with_the_file_on_disk() {
    let dir = crate::interop::tests::scratch_dir("sidecar-merge");
    let img = dir.join("IMG_9.NEF");
    let (old, extras) = future_sidecar();
    std::fs::write(path_for(&img), &old).unwrap();
    let e = every_module();
    write(&img, &e, None).unwrap();
    let new = std::fs::read_to_string(path_for(&img)).unwrap();
    for x in &extras {
        assert!(new.contains(x), "lost on disk: {x}");
    }
    // Version sidecars too.
    std::fs::write(path_for_version(&img, 3), &old).unwrap();
    write_version(&img, 3, Some("Square"), &e, None).unwrap();
    let v3 = std::fs::read_to_string(path_for_version(&img, 3)).unwrap();
    assert!(v3.contains(extras[4]));
    assert!(v3.contains("spd:name=\"Square\""));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A sidecar this build cannot fully read is shown without that part and never written
/// over: its bytes stay exactly as they were.
#[test]
fn what_this_build_cannot_read_is_never_overwritten() {
    let reg = Registry::builtin();
    let base = to_xmp(&every_module(), None);
    let new_shape = base.replacen(
        "     <rdf:li spd:id=\"2\"",
        "     <rdf:li spd:id=\"9\" spd:shape=\"spline\" spd:knots=\"0.1 0.2 0.3\"/>\n     <rdf:li spd:id=\"2\"",
        1,
    );
    let new_mode = base.replacen("spd:mode=\"soft-light\"", "spd:mode=\"vivid-light\"", 1);
    let new_item = base.replacen("(shape 1)", "(depth 0.2 0.8)", 1);
    assert!(new_shape != base && new_mode != base && new_item != base);
    // What reads: everything but the unknown part.
    let l = from_xmp(&new_shape, reg).unwrap();
    assert!(l.partial);
    assert_eq!(l.edit.masks.len(), every_shape().len());
    assert!(l.history.steps.is_empty());
    for text in [&new_mode, &new_item] {
        let l = from_xmp(text, reg).unwrap();
        assert!(l.partial);
        // The module whose blend it was is shown off, not unmasked.
        let sky = l.edit.module(ModuleKind::EXPOSURE, 1).unwrap();
        assert!(!sky.enabled && sky.blend.is_none());
    }
    assert!(keepable(&base));
    let dir = crate::interop::tests::scratch_dir("sidecar-keep");
    let img = dir.join("IMG_7.CR3");
    let damaged = base.replacen("</rdf:RDF>", "", 1);
    for (i, text) in [&new_shape, &new_mode, &new_item, &damaged]
        .into_iter()
        .enumerate()
    {
        assert!(!keepable(text), "case {i}");
        let v = i as u32 + 2;
        let p = path_for_version(&img, v);
        std::fs::write(&p, text).unwrap();
        let e = write_version(&img, v, None, &every_module(), None).unwrap_err();
        assert!(is_kept(&e), "case {i}: {e}");
        assert_eq!(std::fs::read(&p).unwrap(), text.as_bytes(), "case {i}");
    }
    // Not even a temporary file is left.
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4);
    let _ = std::fs::remove_dir_all(&dir);
}

/// AI rows as text: every field but the key round-trips; the older placeholder
/// still reads; too many clicks or an unknown kind is refused.
#[test]
fn ai_rows_round_trip_without_their_key() {
    use crate::model::mask::{AiKind, AiMask, AiPoint};
    let reg = Registry::builtin();
    let mut m = AiMask::new(AiKind::Object, "efficientsam-ti");
    m.points.push(AiPoint {
        x: 0.3,
        y: 0.7,
        positive: true,
    });
    m.points.push(AiPoint {
        x: 0.9,
        y: 0.1,
        positive: false,
    });
    m.rect = Some([0.1, 0.2, 0.5, 0.6]);
    m.key = [7; 32];
    let text = masks::expr_text(&MaskExpr::Ai(m.clone()), reg);
    assert_eq!(
        text,
        "(ai object \"efficientsam-ti\" box 0.1 0.2 0.5 0.6 + 0.3 0.7 - 0.9 0.1)"
    );
    let back = masks::parse_expr(&text, reg).unwrap();
    m.key = [0; 32];
    assert_eq!(back, MaskExpr::Ai(m));
    let s = masks::parse_expr("(ai sky \"upernet-convnext-tiny-ade20k\")", reg).unwrap();
    assert_eq!(
        s,
        MaskExpr::Ai(AiMask::new(AiKind::Sky, "upernet-convnext-tiny-ade20k"))
    );
    let old = format!("(ai \"subject\" {})", "ab".repeat(32));
    assert_eq!(
        masks::parse_expr(&old, reg).unwrap(),
        MaskExpr::Ai(AiMask::new(AiKind::Subject, ""))
    );
    for bad in [
        "(ai moon \"x\")",
        "(ai object x)",
        "(ai object \"x\" + 0.1)",
        "(ai object \"x\" box 0 0 1 1 + 0 0 + 0 0 + 0 0 + 0 0 + 0 0)",
        "(ai object \"x\" * 0.1 0.2)",
    ] {
        assert!(masks::parse_expr(bad, reg).is_err(), "{bad}");
    }
}
