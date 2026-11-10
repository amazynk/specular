//! Screenshot tests of MPR on volumes outside medicine, as the default (General) profile
//! shows them: a light-microscopy z-stack (micrometres, its channel and time point named)
//! and an industrial CT of a drilled part with pores (millimetres, maximised at 1.5×), with
//! neutral XY / XZ / YZ views and no orientation letters, in English and Hebrew at 1× and
//! 1.5×. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`; `SPECULAR_BLESS=1 cargo test -p specular --bin specular
//! mpr::volume_shots` rewrites them.

use specular_dicom::volume::{Geometry, Volume, VoxelKind};
use specular_render::{VoiShape, Window};
use specular_render_volume::{Bricks, Mode, MprRenderer, VolumeTexture, VrRenderer};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::super::medical::number;
use super::draw::{self, Scene, layout, layout_maximised};
use super::load::Length;
use super::views::Views;
use super::volume3d::{self, ClipMode, Series, Vr3};
use super::{position_text, view_name};
use specular_i18n::t;

const SIZE: (f32, f32) = (960.0, 720.0);

/// A volume of `size` voxels `pixel` apart in plane and `dz` between slices, its values
/// from `f` at each voxel's position.
fn volume(size: [u32; 3], pixel: f64, dz: f64, f: impl Fn([f64; 3]) -> f32) -> Volume {
    let positions: Vec<[f64; 3]> = (0..size[2])
        .map(|k| [0.0, 0.0, f64::from(k) * dz])
        .collect();
    let g = Geometry::new(
        &positions,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [pixel, pixel],
        [size[0], size[1]],
    );
    let mut v = Volume::new(g, VoxelKind::F32, 1).expect("volume");
    for k in 0..size[2] {
        let plane: Vec<f32> = (0..size[1])
            .flat_map(|j| (0..size[0]).map(move |i| (i, j)))
            .map(|(i, j)| {
                f([
                    f64::from(i) * pixel,
                    f64::from(j) * pixel,
                    f64::from(k) * dz,
                ])
            })
            .collect();
        v.put(k as usize, &plane, size[0], size[1]).expect("slice");
    }
    v
}

/// Nuclei in a z-stack: bright ellipsoids over a dim, uneven background (µm).
fn nuclei() -> Volume {
    const CELLS: [([f64; 3], [f64; 3], f32); 5] = [
        ([5.0, 4.5, 8.0], [2.6, 2.1, 3.0], 1800.0),
        ([12.5, 6.0, 10.5], [2.2, 2.4, 2.6], 1400.0),
        ([9.0, 12.0, 6.5], [3.0, 2.0, 2.4], 1100.0),
        ([15.5, 12.5, 13.0], [1.8, 1.8, 2.2], 2000.0),
        ([3.5, 12.5, 14.0], [2.0, 2.6, 2.8], 900.0),
    ];
    volume([96, 80, 40], 0.2, 0.5, |p| {
        let mut v = 120.0 + 40.0 * (p[0] * 0.3).sin() as f32;
        for (c, r, b) in CELLS {
            let d: f64 = (0..3).map(|i| ((p[i] - c[i]) / r[i]).powi(2)).sum();
            if d < 1.0 {
                // Brighter inside, with a nucleolus.
                v += b * (1.0 - 0.4 * d as f32);
                if d < 0.08 {
                    v += 600.0;
                }
            }
        }
        v
    })
}

/// An aluminium part (mm): a cylinder along z with two cross bores and a few pores, in air.
fn part() -> Volume {
    volume([110, 110, 100], 0.5, 0.5, |p| {
        let (x, y, z) = (p[0] - 27.5, p[1] - 27.5, p[2]);
        let r = x.hypot(y);
        let mut v = 0.0f32;
        if r < 22.0 && (5.0..45.0).contains(&z) {
            v = 2200.0;
            // A central bore and a cross bore.
            if r < 6.0 || (y.hypot(z - 25.0) < 4.0 && x > -18.0) {
                v = 0.0;
            }
            for c in [[10.0, -8.0, 15.0], [-12.0, 6.0, 33.0], [5.0, 14.0, 40.0]] {
                let d = ((x - c[0]).powi(2) + (y - c[1]).powi(2) + (z - c[2]).powi(2)).sqrt();
                if d < 1.6 {
                    v = 0.0;
                }
            }
        }
        v
    })
}

struct Case {
    name: &'static str,
    scale: f32,
    rtl: bool,
    /// The view maximised, if any.
    maximised: Option<usize>,
}

fn cases(prefix: &'static str, maximise: bool) -> [Case; 4] {
    let max = maximise.then_some(1);
    let names: [&'static str; 4] = match prefix {
        "microscopy" => [
            "mpr-microscopy-1x",
            "mpr-microscopy-1_5x",
            "mpr-microscopy-he-1x",
            "mpr-microscopy-he-1_5x",
        ],
        _ => [
            "mpr-industrial-ct-1x",
            "mpr-industrial-ct-max-1_5x",
            "mpr-industrial-ct-he-1x",
            "mpr-industrial-ct-max-he-1_5x",
        ],
    };
    [
        Case {
            name: names[0],
            scale: 1.0,
            rtl: false,
            maximised: None,
        },
        Case {
            name: names[1],
            scale: 1.5,
            rtl: false,
            maximised: max,
        },
        Case {
            name: names[2],
            scale: 1.0,
            rtl: true,
            maximised: None,
        },
        Case {
            name: names[3],
            scale: 1.5,
            rtl: true,
            maximised: max,
        },
    ]
}

#[test]
fn volumes_outside_medicine_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr-volumes",
    );
    let mut renderer = MprRenderer::new();
    let mut vr = VrRenderer::new();
    let sets = [
        (
            "microscopy",
            nuclei(),
            Length::Micron,
            (1100.0, 2200.0),
            false,
        ),
        ("industrial", part(), Length::Mm, (1100.0, 2600.0), true),
    ];
    for (what, vol, length, (center, width), maximise) in sets {
        let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("texture");
        tex.sync(&h.gpu, &vol).expect("upload");
        let bricks = Bricks::compute(&vol);
        let w = Window {
            center,
            width,
            shape: VoiShape::LinearExact,
            invert: false,
        };
        let series = Series {
            hounsfield: false,
            window: (center, width),
        };
        for c in cases(what, maximise) {
            let lang = if c.rtl {
                specular_i18n::langid!("he")
            } else {
                specular_i18n::langid!("en-US")
            };
            let pal = Palette::dark();
            let mut views = Views::new(&vol.geometry);
            if what == "microscopy" {
                // Through the brightest nucleus.
                views.c = [15.5, 12.5, 13.0];
            } else {
                // Through the cross bore and a pore.
                views.c = [27.5 + 10.0, 27.5 - 8.0, 25.0];
            }
            let mut v3 = Vr3 {
                mode: Mode::Shaded,
                clip: ClipMode::Off,
                ..Vr3::default()
            };
            v3.orbit.orbit(0.6, -0.35);
            let scale = f64::from(c.scale);
            let vp = [0.0, 0.0, f64::from(SIZE.0), f64::from(SIZE.1)];
            let l = match c.maximised {
                Some(v) => layout_maximised(vp, scale, c.rtl, v),
                None => layout(vp, scale, c.rtl),
            };
            let shot = specular_i18n::I18n::with_thread_language(lang, || {
                let readouts = [0, 1, 2].map(|v| {
                    let pos = number((views.position(v) * 10.0).round() / 10.0);
                    vec![position_text(
                        &view_name(v, false, views.oblique(v)),
                        pos,
                        length,
                    )]
                });
                let d = vol.geometry.dims;
                let mut panel = vec![
                    t!(
                        "medical-window",
                        center = number(w.center),
                        width = number(w.width)
                    ),
                    draw::slab_text(&views, length),
                    t!(
                        "mpr-loaded",
                        count = i64::from(d[2]),
                        width = i64::from(d[0]),
                        height = i64::from(d[1])
                    ),
                ];
                if what == "microscopy" {
                    panel.push(t!(
                        "mpr-axis-label",
                        axis = t!("mpr-axis-channel"),
                        label = "DAPI".to_owned()
                    ));
                    panel.push(t!(
                        "mpr-axis-position",
                        axis = "T".to_owned(),
                        position = 3i64,
                        count = 12i64
                    ));
                }
                let scene = Scene {
                    views: &views,
                    layout: l,
                    readouts,
                    labels: false,
                    measures: [Vec::new(), Vec::new(), Vec::new()],
                    panel,
                    active: Some(c.maximised.unwrap_or(0)),
                    vr: draw::VrLabels {
                        preset: t!("vr-preset", preset = v3.preset(false).id()),
                        mode: t!("vr-mode", mode = v3.mode.id()),
                        clip: t!("vr-clip", clip = v3.clip.id()),
                        smooth: false,
                    },
                    segments: Vec::new(),
                    tracts: Default::default(),
                    buttons_first: 0,
                    maximised: c.maximised.is_some(),
                    curve: Default::default(),
                };
                let mut drawn = None;
                h.render_gpu(
                    SIZE,
                    c.scale,
                    |_, _, _, _| {},
                    |dl, text| drawn = Some(draw::draw(dl, text, &scene, &pal, c.rtl, 1.0)),
                );
                let drawn = drawn.expect("drawn");
                super::shots::audit(c.name, SIZE, c.rtl, &scene, &drawn);
                let view = volume3d::view(
                    &v3,
                    &views,
                    &l.views,
                    drawn.view3d.expect("the 3D view placed"),
                    scale,
                    &series,
                    w,
                    [0.0, 0.0, 0.0, 1.0],
                    std::time::Instant::now(),
                );
                h.render_gpu(
                    SIZE,
                    c.scale,
                    |gpu, enc, target, size| {
                        let planes: Vec<_> = (0..3)
                            .map(|v| views.plane(v, l.views[v], scale, w))
                            .collect();
                        renderer.draw(gpu, enc, target, size, &tex, &planes);
                        while vr.draw(gpu, enc, target, size, &tex, Some(&bricks), &view) {
                            let done = std::mem::replace(
                                enc,
                                gpu.device.create_command_encoder(&Default::default()),
                            );
                            gpu.queue.submit([done.finish()]);
                        }
                    },
                    |dl, text| {
                        draw::draw(dl, text, &scene, &pal, c.rtl, 1.0);
                    },
                )
            });
            goldens.check(c.name, &shot);
        }
    }
    goldens.finish();
}
