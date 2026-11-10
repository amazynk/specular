//! Screenshot tests of curved MPR and the dental layout on synthetic phantoms (no patient
//! data): a winding contrast-filled tube in soft tissue, straightened and stretched, with
//! its cross-section; and the synthetic jaw ([`specular_render_volume::arch::phantom`]) with
//! the suggested arch, the panoramic, five numbered cross-sections with rulers and a traced
//! nerve canal. English and Hebrew, at 1× and 1.5×; the screen-reader audit runs on each.
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular mpr::curve_shots` rewrites them.

use specular_dicom::volume::{Geometry, Volume, VoxelKind};
use specular_i18n::t;
use specular_render::{VoiShape, Window};
use specular_render_volume::arch::{self, phantom as jaw};
use specular_render_volume::{Bricks, CprKind, MprRenderer, VolumeTexture, VrRenderer};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::super::medical::number;
use super::curved::{self, Curved, Dental};
use super::draw::{self, CurveButtons, Scene};
use super::views::{KINDS, V3, Views};
use super::volume3d::{self, Series, Vr3};

const SIZE: (f32, f32) = (1024.0, 720.0);

/// The tube's centreline: winding across x, rising and falling in z.
fn tube_centre(x: f64) -> V3 {
    [x, 28.0 * (x / 32.0).sin(), 18.0 * (x / 45.0).cos()]
}

/// Soft tissue with a winding contrast tube of radius 5 mm and a bright calcification.
fn tube_volume() -> Volume {
    let dims = [96u32, 80, 56];
    let (px, sp) = (1.6, 1.6);
    let half = |n: u32, s: f64| (f64::from(n) - 1.0) * s / 2.0;
    let pos: Vec<V3> = (0..dims[2])
        .map(|k| {
            [
                -half(dims[0], px),
                -half(dims[1], px),
                f64::from(k) * sp - half(dims[2], sp),
            ]
        })
        .collect();
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [px, px],
        [dims[0], dims[1]],
    );
    let mut v = Volume::new(g, VoxelKind::I16, 1).unwrap();
    let mut buf = vec![0f32; (dims[0] * dims[1]) as usize];
    for k in 0..dims[2] as usize {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                let p = v
                    .geometry
                    .to_patient([f64::from(i), f64::from(j), k as f64]);
                // Distance to the centreline, by a coarse search along x.
                let mut d = f64::INFINITY;
                let mut x = p[0] - 12.0;
                while x <= p[0] + 12.0 {
                    let c = tube_centre(x);
                    d = d.min(
                        ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2))
                            .sqrt(),
                    );
                    x += 0.5;
                }
                let calc = tube_centre(10.0);
                let dc = ((p[0] - calc[0]).powi(2)
                    + (p[1] - calc[1] - 4.0).powi(2)
                    + (p[2] - calc[2]).powi(2))
                .sqrt();
                buf[(j * dims[0] + i) as usize] = if dc <= 2.5 {
                    1200.0
                } else {
                    // A soft wall (partial volume), so the reformat shows no voxel steps.
                    40.0 + 310.0 * ((5.8 - d) / 1.6).clamp(0.0, 1.0) as f32
                };
            }
        }
        v.put(k, &buf, dims[0], dims[1]).unwrap();
    }
    v
}

struct Case {
    name: &'static str,
    scale: f32,
    rtl: bool,
    dental: bool,
    kind: CprKind,
}

fn window(dental: bool) -> Window {
    Window {
        center: if dental { 1000.0 } else { 150.0 },
        width: if dental { 3200.0 } else { 900.0 },
        shape: VoiShape::Linear,
        invert: false,
    }
}

#[test]
fn curved_mpr_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr-curved",
    );
    let tube = tube_volume();
    let teeth = jaw::volume([112, 112, 56], 0.7, 0.9);
    let cases = [
        Case {
            name: "mpr-curved-1x",
            scale: 1.0,
            rtl: false,
            dental: false,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-curved-1_5x",
            scale: 1.5,
            rtl: false,
            dental: false,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-curved-stretched-he-1x",
            scale: 1.0,
            rtl: true,
            dental: false,
            kind: CprKind::Stretched,
        },
        Case {
            name: "mpr-curved-he-1_5x",
            scale: 1.5,
            rtl: true,
            dental: false,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-curved-evar-1x",
            scale: 1.0,
            rtl: false,
            dental: false,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-curved-evar-he-1x",
            scale: 1.0,
            rtl: true,
            dental: false,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-dental-1x",
            scale: 1.0,
            rtl: false,
            dental: true,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-dental-1_5x",
            scale: 1.5,
            rtl: false,
            dental: true,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-dental-he-1x",
            scale: 1.0,
            rtl: true,
            dental: true,
            kind: CprKind::Straightened,
        },
        Case {
            name: "mpr-dental-he-1_5x",
            scale: 1.5,
            rtl: true,
            dental: true,
            kind: CprKind::Straightened,
        },
    ];
    let mut renderer = MprRenderer::new();
    let mut vr = VrRenderer::new();
    let mut textures = Vec::new();
    for vol in [&tube, &teeth] {
        let mut t = VolumeTexture::new(&h.gpu, vol).expect("texture");
        t.sync(&h.gpu, vol).expect("upload");
        textures.push((t, Bricks::compute(vol)));
    }
    let arch = arch::suggest(&teeth).expect("the phantom's arch");
    for c in cases {
        let vol = if c.dental { &teeth } else { &tube };
        let (tex, bricks) = &textures[usize::from(c.dental)];
        let lang = if c.rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let pal = Palette::dark();
        let scale = f64::from(c.scale);
        let w = window(c.dental);
        let mut views = Views::new(&vol.geometry);
        let mut curve = if c.dental {
            let mut cv = Curved::new(0, views.normal(0));
            let mut d = Dental::new(Some("Planmeca"));
            d.set_nerve(jaw::canal(1.0));
            cv.dental = Some(d);
            cv.set_points(arch.points.clone());
            cv.placing = false;
            views.c[2] = arch.points[0][2];
            // The cursor on the left premolars, where the canal ends.
            let s = cv.spline().unwrap();
            let (at, _) = s.closest([
                jaw::CANAL_FRONT + 6.0,
                jaw::arch(jaw::CANAL_FRONT + 6.0),
                arch.points[0][2],
            ]);
            cv.cursor = at;
            cv
        } else {
            let mut cv = Curved::new(0, views.normal(0));
            let pts: Vec<V3> = [-62.0, -40.0, -18.0, 4.0, 26.0, 48.0, 66.0]
                .iter()
                .map(|&x| tube_centre(x))
                .collect();
            cv.set_points(pts);
            cv.kind = c.kind;
            // Two centrelines of the volume, the first chosen; the Hebrew 1.5× case is
            // renaming it.
            cv.name = "Aorta".into();
            cv.list = vec!["Aorta".into(), "Centreline 2".into()];
            // At 1×, the second chip has the keyboard focus (Tab, then →; ← in Hebrew).
            if c.scale == 1.0 {
                cv.focus = Some(1);
            }
            if c.rtl && c.scale > 1.0 {
                cv.rename = Some((
                    "Aorta \u{5e9}\u{5de}\u{5d0}\u{5dc}".into(),
                    5,
                    String::new(),
                ));
            }
            views.c[2] = tube_centre(4.0)[2];
            let s = cv.spline().unwrap();
            let (at, len) = (s.closest(tube_centre(10.0)).0, s.length());
            cv.cursor = at;
            if c.name.contains("evar") {
                cv.marks = vec![(at - 30.0).max(0.0), at, (at + 25.0).min(len)];
            }
            cv
        };
        let (l, rects) = curved::layout(
            [0.0, 0.0, f64::from(SIZE.0), f64::from(SIZE.1)],
            scale,
            c.rtl,
            curve.view,
            c.dental,
        );
        curve.rects = Some(rects);
        curve.update(scale);
        // EVAR: two diameters drawn by hand on the cross-section at the cursor (mark 2).
        let evar_measures: Vec<[V3; 2]> = match curve.spline() {
            Some(s) if c.name.contains("evar") => {
                let f = s.frame_at(curve.up, curve.angle, curve.cursor);
                let at = |v: V3, k: f64| [0, 1, 2].map(|i| f.point[i] + v[i] * k);
                vec![
                    [at(f.side, -11.0), at(f.side, 11.0)],
                    [at(f.down, -9.5), at(f.down, 9.5)],
                ]
            }
            _ => Vec::new(),
        };
        let series = Series {
            hounsfield: true,
            window: (w.center, w.width),
        };
        let v3 = Vr3::default();
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let readouts = [0, 1, 2].map(|v| {
                let name = specular_i18n::I18n::global().tr(KINDS[v].message_id());
                let pos = number((views.position(v) * 10.0).round() / 10.0);
                vec![t!("mpr-position", view = name, position = pos)]
            });
            let mut panel = vec![
                t!(
                    "medical-window",
                    center = number(w.center),
                    width = number(w.width)
                ),
                draw::slab_text(&views, super::load::Length::Mm),
            ];
            if c.dental {
                panel.push(t!("dental-vendor", vendor = "Planmeca"));
            }
            let scene = Scene {
                views: &views,
                layout: l,
                readouts,
                labels: true,
                measures: [Vec::new(), Vec::new(), Vec::new()],
                panel,
                active: Some(0),
                vr: draw::VrLabels {
                    preset: t!("vr-preset", preset = v3.preset(true).id()),
                    mode: t!("vr-mode", mode = v3.mode.id()),
                    clip: t!("vr-clip", clip = v3.clip.id()),
                    smooth: false,
                },
                segments: Vec::new(),
                tracts: Default::default(),
                buttons_first: 0,
                maximised: false,
                curve: CurveButtons {
                    on: !c.dental,
                    dental: Some(c.dental),
                    segments: None,
                },
            };
            let cx = curved::Ctx {
                views: &views,
                layout: l,
                scale,
                window: w,
                length: super::load::Length::Mm,
                pointer: None,
                measures: &evar_measures,
                draft: None,
            };
            // A first pass places the controls (and so the 3D view).
            let mut drawn = None;
            let mut placed_curve = Vec::new();
            h.render_gpu(
                SIZE,
                c.scale,
                |_, _, _, _| {},
                |dl, text| {
                    drawn = Some(draw::draw(dl, text, &scene, &pal, c.rtl, 1.0));
                    placed_curve = curved::draw(dl, text, &curve, &cx, &pal, c.rtl, 1.0).0;
                },
            );
            let drawn = drawn.expect("drawn");
            let mut audited = curve.clone();
            audited.placed = placed_curve;
            // The focused chip is the screen reader's focus, a button that takes Focus.
            if let Some(f) = audited.focus {
                let mut t = super::super::a11y_audit::window(SIZE, 1.0);
                curved::a11y(&mut t, &audited, super::load::Length::Mm, 1.0);
                let i = audited
                    .placed
                    .iter()
                    .position(|(ctl, ..)| *ctl == draw::Control::CurveName(f))
                    .expect("the focused chip placed");
                let id = specular_widgets::accesskit::NodeId(curved::A11Y_BASE + 0x20 + i as u64);
                assert_eq!(t.focus, id, "{}", c.name);
                let node = &t.nodes.iter().find(|(n, _)| *n == id).unwrap().1;
                assert!(node.supports_action(specular_widgets::accesskit::Action::Focus));
                assert!(node.supports_action(specular_widgets::accesskit::Action::Click));
                assert_eq!(
                    curved::a11y_control(&audited.placed, id.0),
                    Some(draw::Control::CurveName(f))
                );
            }
            super::super::a11y_audit::audit_screen(c.name, SIZE, c.rtl, |t| {
                let crosshair = t!(
                    "mpr-crosshair-a11y",
                    x = "0".to_owned(),
                    y = "0".to_owned(),
                    z = "0".to_owned()
                );
                draw::a11y(t, &scene, &drawn, crosshair, t!("mpr-3d"), 1.0);
                curved::a11y(t, &audited, super::load::Length::Mm, 1.0);
            });
            let canvas = [0.0, 0.0, 0.0, 1.0];
            let view = volume3d::view(
                &v3,
                &views,
                &l.views,
                drawn.view3d.expect("the 3D view placed"),
                scale,
                &series,
                w,
                canvas,
                std::time::Instant::now(),
            );
            h.render_gpu(
                SIZE,
                c.scale,
                |gpu, enc, target, size| {
                    let mut planes: Vec<_> = (0..3)
                        .map(|v| views.plane(v, l.views[v], scale, w))
                        .collect();
                    planes.extend(curve.planes(scale, w, &views, canvas));
                    renderer.draw(gpu, enc, target, size, tex, &planes);
                    while vr.draw(gpu, enc, target, size, tex, Some(bricks), &view) {
                        let done = std::mem::replace(
                            enc,
                            gpu.device.create_command_encoder(&Default::default()),
                        );
                        gpu.queue.submit([done.finish()]);
                    }
                },
                |dl, text| {
                    draw::draw(dl, text, &scene, &pal, c.rtl, 1.0);
                    curved::draw(dl, text, &curve, &cx, &pal, c.rtl, 1.0);
                },
            )
        });
        goldens.check(c.name, &shot);
    }
    goldens.finish();
}
