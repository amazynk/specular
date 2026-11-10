//! Mask model additions: new fields through the codec, old blobs and hashes unchanged, mask presets
//! upright and back, preset files.

use crate::model::codec::{decode_edit, encode_edit};
use crate::model::hash::module_hash;
use crate::model::mask::{
    Blend, Details, MaskExpr, MaskShape, PathPoint, PressureUse, REFINE_RADIUS, Refine, Shape,
    Stroke,
};
use crate::model::mask_preset::{MaskPreset, from_upright, to_upright};
use crate::model::params::Exposure;
use crate::model::{Edit, ModuleInstance, ModuleKind, Registry};
use crate::preview::Orient;

fn edit(blend: Blend, masks: Vec<MaskShape>) -> Edit {
    let mut e = Edit::empty(Default::default());
    let mut m = ModuleInstance::with_params(Exposure::default(), 0);
    m.blend = Some(blend);
    e.modules.push(m);
    e.masks = masks;
    e
}

fn brush(p: PressureUse) -> MaskShape {
    MaskShape {
        id: 3,
        shape: Shape::Brush {
            strokes: vec![
                Stroke {
                    radius: 0.02,
                    hardness: 0.5,
                    flow: 0.7,
                    erase: false,
                    pressure: PressureUse::Flow,
                    points: vec![[0.1, 0.2, 0.5]],
                },
                Stroke {
                    radius: 0.03,
                    hardness: 0.2,
                    flow: 1.0,
                    erase: true,
                    pressure: p,
                    points: vec![[0.3, 0.2, 0.9], [0.4, 0.25, 0.4]],
                },
            ],
        },
    }
}

#[test]
fn new_fields_round_trip_and_old_bytes_stay() {
    let reg = Registry::builtin();
    // An older blend: default radius, flow pressure: the bytes have no new field.
    let old = edit(
        Blend {
            mask: MaskExpr::Shape(3),
            refine: Refine {
                edge_aware: 0.4,
                ..Refine::default()
            },
            ..Blend::default()
        },
        vec![brush(PressureUse::Flow)],
    );
    let old_bytes = encode_edit(&old);
    let mut new = old.clone();
    new.masks = vec![brush(PressureUse::Size)];
    let b = new.modules[0].blend.as_mut().expect("b");
    b.refine.radius = 0.05;
    b.mask = MaskExpr::Combine(
        crate::model::mask::Combine::Intersect,
        Box::new(MaskExpr::Shape(3)),
        Box::new(MaskExpr::Details(Details {
            threshold: 0.4,
            softness: 0.1,
        })),
    );
    let bytes = encode_edit(&new);
    assert!(bytes.len() > old_bytes.len());
    let (back, _) = decode_edit(&bytes, reg).expect("decode");
    assert_eq!(back, new);
    let (back_old, _) = decode_edit(&old_bytes, reg).expect("decode");
    assert_eq!(back_old, old);
    assert_eq!(
        back_old.modules[0].blend.as_ref().expect("b").refine.radius,
        REFINE_RADIUS
    );
    // Hashes: unchanged for the old blend, changed by each new value.
    let h0 = module_hash(&old, &old.modules[0]);
    let mut same = old.clone();
    same.modules[0].blend.as_mut().expect("b").refine.radius = REFINE_RADIUS;
    assert_eq!(module_hash(&same, &same.modules[0]), h0);
    let mut r = old.clone();
    r.modules[0].blend.as_mut().expect("b").refine.radius = 0.03;
    assert_ne!(module_hash(&r, &r.modules[0]), h0);
    let mut p = old.clone();
    p.masks = vec![brush(PressureUse::Both)];
    assert_ne!(module_hash(&p, &p.modules[0]), h0);
}

#[test]
fn the_new_fields_go_through_the_sidecar() {
    let reg = Registry::builtin();
    let mut e = edit(
        Blend {
            mask: MaskExpr::Combine(
                crate::model::mask::Combine::Union,
                Box::new(MaskExpr::Shape(3)),
                Box::new(MaskExpr::Details(Details {
                    threshold: 0.35,
                    softness: 0.125,
                })),
            ),
            refine: Refine {
                radius: 0.04,
                feather: 0.01,
                blur: 0.005,
                details: -0.3,
                ..Refine::default()
            },
            ..Blend::default()
        },
        vec![brush(PressureUse::Size)],
    );
    e.source = Default::default();
    let text = crate::sidecar::to_xmp(&e, None);
    assert!(text.contains("(details 0.35 0.125)"), "{text}");
    assert!(text.contains("spd:radius=\"0.04\""));
    let back = crate::sidecar::from_xmp(&text, reg).expect("read");
    assert_eq!(back.edit.modules[0].blend, e.modules[0].blend);
    assert_eq!(back.edit.masks, e.masks);
    // An older blend writes no radius.
    let mut plain = e.clone();
    plain.modules[0].blend.as_mut().expect("b").refine.radius = REFINE_RADIUS;
    assert!(!crate::sidecar::to_xmp(&plain, None).contains("spd:radius"));
}

fn every_orient() -> Vec<Orient> {
    (1..=8).map(Orient::from_exif).collect()
}

#[test]
fn upright_points_round_trip_and_match_the_preview() {
    for o in every_orient() {
        for p in [[0.1f32, 0.2], [0.9, 0.35], [0.5, 0.5]] {
            let u = to_upright(o, p);
            let b = from_upright(o, u);
            assert!((b[0] - p[0]).abs() < 1e-6 && (b[1] - p[1]).abs() < 1e-6);
            // As the preview shows a stored pixel (`Orient::source_of`, on a 100×60 image).
            let (w, h) = (100u32, 60u32);
            let (ow, oh) = o.size(w, h);
            let (ux, uy) = ((u[0] * ow as f32) as u32, (u[1] * oh as f32) as u32);
            let (sx, sy) = o.source_of(ux.min(ow - 1), uy.min(oh - 1), w, h);
            assert!(
                (sx as f32 - p[0] * w as f32).abs() <= 1.0
                    && (sy as f32 - p[1] * h as f32).abs() <= 1.0,
                "{o:?} {p:?}"
            );
        }
    }
}

#[test]
fn a_mask_preset_lands_on_the_same_part_of_the_picture() {
    let sky = MaskShape {
        id: 7,
        shape: Shape::Linear {
            start: [0.5, 0.0],
            end: [0.5, 0.4],
        },
    };
    let blob = MaskShape {
        id: 9,
        shape: Shape::Ellipse {
            centre: [0.3, 0.6],
            radii: [0.2, 0.05],
            angle_deg: 30.0,
            feather: 0.4,
        },
    };
    let poly = MaskShape {
        id: 11,
        shape: Shape::Polygon {
            points: vec![
                PathPoint {
                    x: 0.1,
                    y: 0.1,
                    feather: 0.01,
                },
                PathPoint {
                    x: 0.4,
                    y: 0.1,
                    feather: 0.02,
                },
                PathPoint {
                    x: 0.2,
                    y: 0.3,
                    feather: 0.0,
                },
            ],
            closed: true,
        },
    };
    let mut src = edit(
        Blend {
            mask: MaskExpr::Combine(
                crate::model::mask::Combine::Subtract,
                Box::new(MaskExpr::Combine(
                    crate::model::mask::Combine::Union,
                    Box::new(MaskExpr::Shape(7)),
                    Box::new(MaskExpr::Shape(11)),
                )),
                Box::new(MaskExpr::Shape(9)),
            ),
            invert: true,
            refine: Refine {
                feather: 0.02,
                ..Refine::default()
            },
            ..Blend::default()
        },
        vec![sky, blob, poly],
    );
    // A second module borrowing it: the preset copies the borrowed mask in.
    let mut user = ModuleInstance::with_params(Exposure::default(), 1);
    user.blend = Some(Blend {
        mask: MaskExpr::FromModule {
            kind: ModuleKind::EXPOSURE,
            instance: 0,
        },
        ..Blend::default()
    });
    src.modules.push(user);
    for so in every_orient() {
        let p = MaskPreset::from_module("Sky", &src, ModuleKind::EXPOSURE, 1, so).expect("p");
        assert_eq!(p.masks.len(), 3);
        assert!(!p.invert);
        assert!(matches!(p.mask, MaskExpr::Invert(_)));
        // Through the preset file.
        let text = crate::sidecar::files::mask_preset_to_xmp(&p);
        let p2 = crate::sidecar::files::mask_preset_from_xmp(&text, Registry::builtin(), "x")
            .expect("read");
        assert_eq!(p2, p);
        for to in every_orient() {
            let mut dst = edit(
                Blend::default(),
                vec![MaskShape {
                    id: 4,
                    shape: Shape::Linear {
                        start: [0.0, 0.0],
                        end: [1.0, 1.0],
                    },
                }],
            );
            dst.modules[0].blend.as_mut().expect("b").mask = MaskExpr::Shape(4);
            dst.modules[0].blend.as_mut().expect("b").opacity = 0.6;
            assert!(p.apply(&mut dst, ModuleKind::EXPOSURE, 0, to));
            let b = dst.modules[0].blend.clone().expect("b");
            assert_eq!(b.opacity, 0.6, "amount stays");
            assert!(dst.mask(4).is_none(), "the old shape went");
            // Each shape shows at the same upright place as on the source.
            let mut ids = Vec::new();
            b.mask.shape_ids(&mut ids);
            assert_eq!(ids.len(), 3);
            let place = |s: &Shape, o: Orient| match s {
                Shape::Linear { start, end } => [to_upright(o, *start), to_upright(o, *end)],
                Shape::Ellipse { centre, .. } => [to_upright(o, *centre); 2],
                Shape::Polygon { points, .. } => [to_upright(o, [points[2].x, points[2].y]); 2],
                _ => [[0.0; 2]; 2],
            };
            for (k, old) in [7u32, 11, 9].iter().enumerate() {
                let a = place(&src.mask(*old).expect("s").shape, so);
                let c = place(&dst.mask(ids[k]).expect("s").shape, to);
                for i in 0..2 {
                    for j in 0..2 {
                        assert!(
                            (a[i][j] - c[i][j]).abs() < 1e-5,
                            "{so:?} → {to:?} shape {old}"
                        );
                    }
                }
            }
            // The ellipse's long axis turns with the picture.
            let ang = |s: &Shape| match s {
                Shape::Ellipse { angle_deg, .. } => *angle_deg,
                _ => 0.0,
            };
            if so == to {
                let d = (ang(&src.mask(9).expect("s").shape)
                    - ang(&dst.mask(ids[2]).expect("s").shape))
                .rem_euclid(180.0);
                assert!(d < 1e-3 || d > 180.0 - 1e-3);
            }
        }
    }
}
