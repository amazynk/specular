//! Screenshot tests of MPR on the synthetic CT phantom (`specular_dicom::volume::phantom`):
//! the three planes drawn by the GPU through the crosshair at the bone sphere, with the
//! crosshair lines, handles, orientation letters, readouts, a measurement, and the 3D view
//! in the fourth slot under its controls — thin at 1×, an oblique MIP slab at 1.5×,
//! right-to-left in Hebrew and on the light palette, each with a shaded bone rendering; then
//! the 3D view refined in cinematic mode with the crosshair's corner cut away, soft tissue
//! clipped to the zoomed 2D views' box, and a 3D MIP. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`; `SPECULAR_BLESS=1 cargo test -p specular --bin specular
//! mpr::shots` rewrites them.
//!
//! The analysis overlays on the same phantom: a synthetic two-segment segmentation (filled
//! with outlines, its segment list in the fourth slot, one segment hidden in the light case)
//! and a synthetic PET-like volume fused in hot iron and viridis with a manual nudge.

use specular_dicom::volume::phantom;
use specular_render::{VoiShape, Window};
use specular_render_volume::{Bricks, Mode, MprRenderer, Preset, Slab, VolumeTexture, VrRenderer};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use specular_dicom::labels::{LabelSource, Labels, Layer, Segment};
use specular_dicom::volume::{Geometry, Volume, VoxelKind, Voxels};
use specular_render_volume::{Colormap, FusionRenderer, Rigid};

use super::super::medical::{measure, number};
use super::analysis::{Analysis, Set};
use super::draw::{self, Scene, layout};
use super::views::{KINDS, Views};
use super::volume3d::{self, ClipMode, Series, Vr3};
use specular_i18n::t;

const SIZE: (f32, f32) = (640.0, 480.0);
/// The 3D view's own cases: a larger window, as the slot shares its space with the controls.
const SIZE_3D: (f32, f32) = (1024.0, 720.0);

/// The screen-reader audit of the views and controls `placed` drew in a window of `size`.
pub(super) fn audit(name: &str, size: (f32, f32), rtl: bool, scene: &Scene, placed: &draw::Placed) {
    super::super::a11y_audit::audit_screen(name, size, rtl, |t| {
        let crosshair = t!(
            "mpr-crosshair-a11y",
            x = "0".to_owned(),
            y = "0".to_owned(),
            z = "0".to_owned()
        );
        draw::a11y(t, scene, placed, crosshair, t!("mpr-3d"), 1.0);
    });
}

fn window() -> Window {
    Window {
        center: 40.0,
        width: 400.0,
        shape: VoiShape::Linear,
        invert: false,
    }
}

struct Case {
    name: &'static str,
    scale: f32,
    rtl: bool,
    light: bool,
    /// Axial rotation (radians) and slab.
    turn: f64,
    slab: Option<(Slab, f64)>,
    /// The 3D view: rendering, preset, clipping and the orbit (radians across, up).
    mode: Mode,
    preset: Preset,
    clip: ClipMode,
    orbit: (f64, f64),
}

const SHADED: (Mode, Preset, ClipMode, (f64, f64)) =
    (Mode::Shaded, Preset::CtBone, ClipMode::Off, (0.6, -0.35));

#[test]
fn mpr_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr",
    );
    let vol = phantom::volume([112, 112, 96], 3.0, 4.5, 0.0);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("texture");
    tex.sync(&h.gpu, &vol).expect("upload");
    let mut renderer = MprRenderer::new();
    let cases = [
        Case {
            name: "mpr-thin-1x",
            scale: 1.0,
            rtl: false,
            light: false,
            turn: 0.0,
            slab: None,
            mode: SHADED.0,
            preset: SHADED.1,
            clip: SHADED.2,
            orbit: SHADED.3,
        },
        Case {
            name: "mpr-oblique-mip-1_5x",
            scale: 1.5,
            rtl: false,
            light: false,
            turn: 0.45,
            slab: Some((Slab::Mip, 20.0)),
            mode: SHADED.0,
            preset: SHADED.1,
            clip: SHADED.2,
            orbit: SHADED.3,
        },
        Case {
            name: "mpr-thin-he-1x",
            scale: 1.0,
            rtl: true,
            light: false,
            turn: 0.0,
            slab: None,
            mode: SHADED.0,
            preset: SHADED.1,
            clip: SHADED.2,
            orbit: SHADED.3,
        },
        Case {
            name: "mpr-thin-light-1x",
            scale: 1.0,
            rtl: false,
            light: true,
            turn: 0.0,
            slab: None,
            mode: SHADED.0,
            preset: SHADED.1,
            clip: SHADED.2,
            orbit: SHADED.3,
        },
        Case {
            name: "mpr-3d-cinematic-cut-1x",
            scale: 1.0,
            rtl: false,
            light: false,
            turn: 0.0,
            slab: None,
            mode: Mode::Cinematic,
            preset: Preset::CtSoft,
            clip: ClipMode::Cut,
            orbit: (0.9, -0.5),
        },
        Case {
            name: "mpr-3d-soft-box-he-1_5x",
            scale: 1.5,
            rtl: true,
            light: false,
            turn: 0.0,
            slab: None,
            mode: Mode::Shaded,
            preset: Preset::CtSoft,
            clip: ClipMode::Box,
            orbit: (-0.5, -0.3),
        },
        Case {
            name: "mpr-3d-mip-light-1_5x",
            scale: 1.5,
            rtl: false,
            light: true,
            turn: 0.0,
            slab: None,
            mode: Mode::Mip,
            preset: Preset::CtBone,
            clip: ClipMode::Off,
            orbit: (0.3, 0.0),
        },
    ];
    let bricks = Bricks::compute(&vol);
    let mut vr = VrRenderer::new();
    for c in cases {
        let lang = if c.rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let pal = if c.light {
            Palette::light()
        } else {
            Palette::dark()
        };
        let mut views = Views::new(&vol.geometry);
        views.c = [phantom::SPHERES[0].0[0], 0.0, 20.0];
        views.rotate(0, c.turn);
        if let Some((s, t)) = c.slab {
            views.set_slab(Some(s));
            views.thickness = t;
        }
        if c.clip == ClipMode::Box {
            // The axial view zoomed in on the bone sphere: the box keeps what it shows.
            views.frames[0].zoom = 2.2;
            views.frames[0].pan = [35.0, -10.0];
        }
        let mut v3 = Vr3 {
            mode: c.mode,
            preset: Some(c.preset),
            clip: c.clip,
            ..Vr3::default()
        };
        v3.orbit.orbit(c.orbit.0, c.orbit.1);
        let series = Series {
            hounsfield: true,
            window: (40.0, 400.0),
        };
        let scale = f64::from(c.scale);
        let size = if c.name.starts_with("mpr-3d") {
            SIZE_3D
        } else {
            SIZE
        };
        let l = layout(
            [0.0, 0.0, f64::from(size.0), f64::from(size.1)],
            scale,
            c.rtl,
        );
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let readouts = [0, 1, 2].map(|v| {
                let name = if views.oblique(v) {
                    t!("mpr-oblique")
                } else {
                    specular_i18n::I18n::global().tr(KINDS[v].message_id())
                };
                let pos = number((views.position(v) * 10.0).round() / 10.0);
                vec![t!("mpr-position", view = name, position = pos)]
            });
            // A measurement across the bone sphere in the axial view.
            let a = [30.0, 0.0, views.c[2]];
            let b = [90.0, 0.0, views.c[2]];
            let to = |p| {
                let q = views.to_screen(0, l.views[0], p);
                (q.0 as f32, q.1 as f32)
            };
            let measures = [
                vec![measure::Drawn {
                    outline: vec![to(a), to(b)],
                    closed: false,
                    handles: vec![to(a), to(b)],
                    lines: vec![t!("measure-length-mm", value = "60.0")],
                    draft: false,
                }],
                Vec::new(),
                Vec::new(),
            ];
            let w = window();
            let panel = vec![
                t!(
                    "medical-window",
                    center = number(w.center),
                    width = number(w.width)
                ),
                draw::slab_text(&views, super::load::Length::Mm),
                t!("mpr-loaded", count = 96i64, width = 112i64, height = 112i64),
                t!("mpr-gpu-memory", size = "2 MiB"),
            ];
            let mut panel = panel;
            if c.mode.composite() {
                let x = number(v3.threshold(true, series.window).round());
                panel.push(t!("vr-threshold", value = format!("{x} HU")));
            }
            let scene = Scene {
                views: &views,
                layout: l,
                readouts,
                labels: true,
                measures,
                panel,
                active: Some(0),
                vr: draw::VrLabels {
                    preset: t!("vr-preset", preset = c.preset.id()),
                    mode: t!("vr-mode", mode = c.mode.id()),
                    clip: t!("vr-clip", clip = c.clip.id()),
                    smooth: false,
                },
                segments: Vec::new(),
                tracts: Default::default(),
                buttons_first: 0,
                maximised: false,
                curve: Default::default(),
            };
            // The controls decide where the 3D view starts: a first pass places them.
            let mut drawn = None;
            h.render_gpu(
                size,
                c.scale,
                |_, _, _, _| {},
                |dl, text| drawn = Some(draw::draw(dl, text, &scene, &pal, c.rtl, 1.0)),
            );
            let drawn = drawn.expect("drawn");
            audit(c.name, size, c.rtl, &scene, &drawn);
            let placed = drawn.view3d;
            let canvas = [0.0, 0.0, 0.0, 1.0]; // the harness clears the scene black
            let view = volume3d::view(
                &v3,
                &views,
                &l.views,
                placed.expect("the 3D view placed"),
                scale,
                &series,
                w,
                canvas,
                std::time::Instant::now(),
            );
            h.render_gpu(
                size,
                c.scale,
                |gpu, enc, target, size| {
                    let planes: Vec<_> = (0..3)
                        .map(|v| views.plane(v, l.views[v], scale, w))
                        .collect();
                    renderer.draw(gpu, enc, target, size, &tex, &planes);
                    // Every refinement frame in a submission of its own (each rewrites the
                    // uniforms), as the app draws them.
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
    goldens.finish();
}

/// A grid of `size` voxels, `pixel` mm in-plane and `spacing` mm apart, centred on the
/// phantom's origin.
fn grid(size: [u32; 3], pixel: f64, spacing: f64) -> Geometry {
    Geometry::new(
        &phantom::positions(size, pixel, spacing, 0.0),
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [pixel, pixel],
        [size[0], size[1]],
    )
}

/// Segment 1 over the bone sphere, segment 2 over the fat sphere (a coarser grid than
/// the CT's, as segmentations often are).
fn segmentation() -> Labels {
    let g = grid([60, 60, 50], 5.5, 8.0);
    let [w, h, d] = g.dims;
    let mut ids = vec![0u16; (w * h * d) as usize];
    for k in 0..d {
        for j in 0..h {
            for i in 0..w {
                let p = g.to_patient([f64::from(i), f64::from(j), f64::from(k)]);
                let inside = |(c, r, _): ([f64; 3], f64, f32)| {
                    (0..3).map(|a| (p[a] - c[a]).powi(2)).sum::<f64>() <= r * r
                };
                let idx = ((k * h + j) * w + i) as usize;
                if inside(phantom::SPHERES[0]) {
                    ids[idx] = 1;
                } else if inside(phantom::SPHERES[1]) {
                    ids[idx] = 2;
                }
            }
        }
    }
    let segment = |number, label: &str, colour, ml| Segment {
        number,
        label: label.to_owned(),
        colour,
        algorithm: "MANUAL".to_owned(),
        fractional: false,
        ml,
        voxels: 0,
    };
    Labels {
        source: LabelSource::Seg,
        segments: vec![
            segment(1, "Sphere A", [230, 80, 60], 113.1),
            segment(2, "Sphere B", [60, 160, 255], 65.4),
        ],
        layers: vec![Layer {
            volume: Volume {
                geometry: g,
                voxels: Voxels::U16(ids),
                loaded: vec![true; d as usize],
                step_xy: 1,
            },
            fraction: None,
        }],
        frame_of_reference: "1.2.3".to_owned(),
        referenced_series: Vec::new(),
        title: "Phantom".to_owned(),
    }
}

/// A PET-like volume: uptake 1 in the body, 12 in the bone sphere, 6 in the rod.
fn uptake() -> Volume {
    let g = grid([64, 64, 56], 6.0, 8.0);
    let mut v = Volume::new(g, VoxelKind::F32, 1).expect("fits");
    let [w, h, d] = v.geometry.dims;
    let mut buf = vec![0f32; (w * h) as usize];
    for k in 0..d as usize {
        for j in 0..h {
            for i in 0..w {
                let p = v
                    .geometry
                    .to_patient([f64::from(i), f64::from(j), k as f64]);
                buf[(j * w + i) as usize] = match phantom::hu(p) {
                    1000.0 => 12.0,
                    phantom::ROD => 6.0,
                    phantom::AIR => 0.0,
                    _ => 1.0,
                };
            }
        }
        v.put(k, &buf, w, h).expect("slice fits");
    }
    v
}

struct AnalysisCase {
    name: &'static str,
    scale: f32,
    rtl: bool,
    light: bool,
    /// Fused in this colour map (else the segmentation).
    fusion: Option<Colormap>,
}

#[test]
fn analysis_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr-analysis",
    );
    let vol = phantom::volume([112, 112, 96], 3.0, 4.5, 0.0);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("texture");
    tex.sync(&h.gpu, &vol).expect("upload");
    let labels = segmentation();
    let mut label_tex = VolumeTexture::new(&h.gpu, &labels.layers[0].volume).expect("texture");
    label_tex
        .sync(&h.gpu, &labels.layers[0].volume)
        .expect("upload");
    let pet = uptake();
    let mut pet_tex = VolumeTexture::new(&h.gpu, &pet).expect("texture");
    pet_tex.sync(&h.gpu, &pet).expect("upload");
    let mut renderer = MprRenderer::new();
    let mut fusion = FusionRenderer::new();
    let mut analysis = Analysis::default();
    analysis.sets.push(Set {
        labels,
        textures: vec![label_tex],
        hidden: vec![false, false],
    });
    analysis.searched = true;
    let cases = [
        AnalysisCase {
            name: "mpr-seg-1x",
            scale: 1.0,
            rtl: false,
            light: false,
            fusion: None,
        },
        AnalysisCase {
            name: "mpr-seg-light-1_5x",
            scale: 1.5,
            rtl: false,
            light: true,
            fusion: None,
        },
        AnalysisCase {
            name: "mpr-seg-he-1x",
            scale: 1.0,
            rtl: true,
            light: false,
            fusion: None,
        },
        AnalysisCase {
            name: "mpr-fusion-1x",
            scale: 1.0,
            rtl: false,
            light: false,
            fusion: Some(Colormap::HotIron),
        },
        AnalysisCase {
            name: "mpr-fusion-light-1_5x",
            scale: 1.5,
            rtl: false,
            light: true,
            fusion: Some(Colormap::Viridis),
        },
    ];
    for c in cases {
        let lang = if c.rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let pal = if c.light {
            Palette::light()
        } else {
            Palette::dark()
        };
        // The light case hides the second segment.
        analysis.sets[0].hidden[1] = c.light && c.fusion.is_none();
        let mut views = Views::new(&vol.geometry);
        views.c = [60.0, -15.0, 20.0];
        let scale = f64::from(c.scale);
        let l = layout(
            [0.0, 0.0, f64::from(SIZE.0), f64::from(SIZE.1)],
            scale,
            c.rtl,
        );
        let rigid = Rigid {
            translate: [4.0, -3.0, 0.0],
            rotate_deg: [0.0, 0.0, 3.0],
            centre: vol.geometry.centre(),
        };
        let fused_window = Window {
            center: 6.0,
            width: 12.0,
            shape: VoiShape::Linear,
            invert: false,
        };
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let readouts = [0, 1, 2].map(|v| {
                let name = specular_i18n::I18n::global().tr(KINDS[v].message_id());
                let pos = number((views.position(v) * 10.0).round() / 10.0);
                vec![t!("mpr-position", view = name, position = pos)]
            });
            let w = window();
            let mut panel = vec![
                t!(
                    "medical-window",
                    center = number(w.center),
                    width = number(w.width)
                ),
                draw::slab_text(&views, super::load::Length::Mm),
            ];
            let segments = if let Some(map) = c.fusion {
                panel.push(t!(
                    "fusion-on",
                    modality = "PT",
                    colormap = specular_i18n::I18n::global().tr(&format!("colormap-{}", map.id())),
                    blend = "50"
                ));
                panel.push(t!(
                    "fusion-moved",
                    x = "4",
                    y = "\u{2212}3",
                    z = "0",
                    degrees = "3"
                ));
                Vec::new()
            } else {
                panel.extend(analysis.panel());
                analysis.segment_list()
            };
            let scene = Scene {
                views: &views,
                layout: l,
                readouts,
                labels: true,
                measures: [Vec::new(), Vec::new(), Vec::new()],
                panel,
                active: Some(0),
                vr: {
                    let vr = Vr3::default();
                    draw::VrLabels {
                        preset: t!("vr-preset", preset = vr.preset(true).id()),
                        mode: t!("vr-mode", mode = vr.mode.id()),
                        clip: t!("vr-clip", clip = vr.clip.id()),
                        smooth: false,
                    }
                },
                segments,
                tracts: Default::default(),
                buttons_first: 0,
                maximised: false,
                curve: Default::default(),
            };
            let mut drawn = None;
            let shot = h.render_gpu(
                SIZE,
                c.scale,
                |gpu, enc, target, size| {
                    let planes: Vec<_> = (0..3)
                        .map(|v| views.plane(v, l.views[v], scale, w))
                        .collect();
                    renderer.draw(gpu, enc, target, size, &tex, &planes);
                    if let Some(map) = c.fusion {
                        let moved: Vec<_> = planes
                            .iter()
                            .map(|p| {
                                let mut q = rigid.plane(p);
                                q.window = fused_window;
                                q
                            })
                            .collect();
                        fusion.draw(gpu, enc, target, size, &pet_tex, map, 0.5, &moved);
                    } else {
                        analysis.draw(gpu, enc, target, size, &planes, scale);
                    }
                },
                |dl, text| {
                    drawn = Some(draw::draw(dl, text, &scene, &pal, c.rtl, 1.0));
                },
            );
            audit(c.name, SIZE, c.rtl, &scene, &drawn.expect("drawn"));
            shot
        });
        goldens.check(c.name, &shot);
    }
    goldens.finish();
}

/// A NIfTI label file over the phantom: its first two spheres as labels 1 and 7, written in
/// RAS on an oblique grid of its own (sform rotated 20° about x and 25° about z), read as
/// the app reads it, named and coloured by an ITK-SNAP table.
fn label_volume() -> Labels {
    use specular_labelmap::testing::{Form, nifti, spheres};
    let (dims, sp) = ([72usize, 72, 60], [2.5, 2.5, 3.0]);
    let (a, b) = (20f64.to_radians(), 25f64.to_radians());
    let rx = [
        [1.0, 0.0, 0.0],
        [0.0, a.cos(), -a.sin()],
        [0.0, a.sin(), a.cos()],
    ];
    let rz = [
        [b.cos(), -b.sin(), 0.0],
        [b.sin(), b.cos(), 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mut m = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = (0..3).map(|k| rz[i][k] * rx[k][j]).sum::<f64>() * sp[j];
        }
    }
    let c = [0, 1, 2].map(|r| {
        (0..3)
            .map(|j| m[r][j] * (dims[j] as f64 - 1.0) / 2.0)
            .sum::<f64>()
    });
    for r in 0..3 {
        m[r][3] = -c[r];
    }
    // The phantom's spheres are in LPS; the file's world is RAS.
    let ras = |p: [f64; 3]| [-p[0], -p[1], p[2]];
    let balls = [
        (ras(phantom::SPHERES[0].0), phantom::SPHERES[0].1, 1i16),
        (ras(phantom::SPHERES[1].0), phantom::SPHERES[1].1, 7i16),
    ];
    let data = spheres(dims, &m, &balls);
    let bytes = nifti(dims, sp, &Form::Sform(m), &data);
    let path = std::path::Path::new("labels.nii");
    let header = specular_decode::medvol::read_header(&bytes, path).expect("header");
    let vol =
        specular_decode::medvol::load(header, std::sync::Arc::new(bytes), path).expect("voxels");
    let lut = specular_labelmap::Lut::parse(
        "1 255 170 0 1 1 1 \"Bone insert\"\n7 0 200 120 1 1 1 \"Fat insert\"\n",
    )
    .expect("table");
    specular_labelmap::labels(&vol, Some(&lut), "labels").expect("labels")
}

#[test]
fn label_volume_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr-label-volume",
    );
    let vol = phantom::volume([112, 112, 96], 3.0, 4.5, 0.0);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("texture");
    tex.sync(&h.gpu, &vol).expect("upload");
    let labels = label_volume();
    let textures = super::analysis::upload(&h.gpu, &labels).expect("upload");
    let mut renderer = MprRenderer::new();
    let mut analysis = Analysis::default();
    let n = labels.segments.len();
    analysis.sets.push(Set {
        labels,
        textures,
        hidden: vec![false; n],
    });
    analysis.searched = true;
    // Filled at 1×, outlines only (the third fill) at 1.5×, in English and Hebrew.
    for (name, scale, rtl, fill) in [
        ("mpr-label-volume-1x", 1.0f32, false, 0usize),
        ("mpr-label-volume-outline-1.5x", 1.5, false, 2),
        ("mpr-label-volume-he-1x", 1.0, true, 0),
        ("mpr-label-volume-outline-he-1.5x", 1.5, true, 2),
    ] {
        analysis.fill = fill;
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let pal = Palette::dark();
        let mut views = Views::new(&vol.geometry);
        views.c = [20.0, -10.0, 15.0];
        let s = f64::from(scale);
        let l = layout([0.0, 0.0, f64::from(SIZE.0), f64::from(SIZE.1)], s, rtl);
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let readouts = [0, 1, 2].map(|v| {
                let name = specular_i18n::I18n::global().tr(KINDS[v].message_id());
                let pos = number((views.position(v) * 10.0).round() / 10.0);
                vec![t!("mpr-position", view = name, position = pos)]
            });
            let w = window();
            let mut panel = vec![
                t!(
                    "medical-window",
                    center = number(w.center),
                    width = number(w.width)
                ),
                draw::slab_text(&views, super::load::Length::Mm),
            ];
            panel.extend(analysis.panel());
            let scene = Scene {
                views: &views,
                layout: l,
                readouts,
                labels: true,
                measures: [Vec::new(), Vec::new(), Vec::new()],
                panel,
                active: Some(0),
                vr: {
                    let vr = Vr3::default();
                    draw::VrLabels {
                        preset: t!("vr-preset", preset = vr.preset(true).id()),
                        mode: t!("vr-mode", mode = vr.mode.id()),
                        clip: t!("vr-clip", clip = vr.clip.id()),
                        smooth: false,
                    }
                },
                segments: analysis.segment_list(),
                tracts: Default::default(),
                buttons_first: 0,
                maximised: false,
                curve: Default::default(),
            };
            let mut drawn = None;
            let shot = h.render_gpu(
                SIZE,
                scale,
                |gpu, enc, target, size| {
                    let planes: Vec<_> = (0..3).map(|v| views.plane(v, l.views[v], s, w)).collect();
                    renderer.draw(gpu, enc, target, size, &tex, &planes);
                    analysis.draw(gpu, enc, target, size, &planes, s);
                },
                |dl, text| {
                    drawn = Some(draw::draw(dl, text, &scene, &pal, rtl, 1.0));
                },
            );
            audit(name, SIZE, rtl, &scene, &drawn.expect("drawn"));
            shot
        });
        goldens.check(name, &shot);
    }
    goldens.finish();
}

/// A synthetic dose: a Gaussian of 66 Gy peak around the crosshair's point.
fn dose_cloud(at: [f64; 3]) -> Volume {
    let g = grid([56, 56, 48], 7.0, 9.0);
    let mut v = Volume::new(g, VoxelKind::F32, 1).expect("fits");
    let [w, h, d] = v.geometry.dims;
    let mut buf = vec![0f32; (w * h) as usize];
    for k in 0..d as usize {
        for j in 0..h {
            for i in 0..w {
                let p = v
                    .geometry
                    .to_patient([f64::from(i), f64::from(j), k as f64]);
                let r2 = (0..3).map(|a| (p[a] - at[a]).powi(2)).sum::<f64>();
                buf[(j * w + i) as usize] = (66.0 * (-r2 / (2.0 * 55.0f64.powi(2))).exp()) as f32;
            }
        }
        v.put(k, &buf, w, h).expect("slice fits");
    }
    v
}

/// RT dose over the phantom: isodose lines of a 60 Gy prescription with the light wash, the
/// dose legend in the fourth slot (one level hidden in the light case), and the dose as a
/// colour map.
#[test]
fn isodose_matches_goldens() {
    use super::dose::{Dose, DoseFound, DoseMode};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr-isodose",
    );
    let vol = phantom::volume([112, 112, 96], 3.0, 4.5, 0.0);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("texture");
    tex.sync(&h.gpu, &vol).expect("upload");
    let at = [60.0, -15.0, 20.0];
    let mut renderer = MprRenderer::new();
    let mut analysis = Analysis::default();
    let found = DoseFound {
        volume: dose_cloud(at),
        max: 66.0,
        units: "GY".to_owned(),
        prescription: Some(specular_dicom::rtdose::Prescription {
            sop_instance: "1.2.3".to_owned(),
            label: "PTV".to_owned(),
            dose: 60.0,
        }),
        dvhs: Vec::new(),
    };
    analysis.dose = Some(Dose::new(&h.gpu, found).expect("dose texture"));
    analysis.searched = true;
    // (name, scale, light, colour map)
    let cases = [
        ("mpr-isodose-1x", 1.0f32, false, false),
        ("mpr-isodose-light-1_5x", 1.5, true, false),
        ("mpr-dose-colour-1x", 1.0, false, true),
    ];
    for (name, ui, light, colour) in cases {
        let pal = if light {
            Palette::light()
        } else {
            Palette::dark()
        };
        if let Some(d) = analysis.dose.as_mut() {
            d.mode = if colour {
                DoseMode::Colour
            } else {
                DoseMode::Lines
            };
            d.hidden.iter_mut().for_each(|x| *x = false);
            if light {
                d.hidden[2] = true;
            }
        }
        let mut views = Views::new(&vol.geometry);
        views.c = at;
        let scale = f64::from(ui);
        let l = layout(
            [0.0, 0.0, f64::from(SIZE_3D.0), f64::from(SIZE_3D.1)],
            scale,
            false,
        );
        let readouts = [0, 1, 2].map(|v| {
            let name = specular_i18n::I18n::global().tr(KINDS[v].message_id());
            let pos = number((views.position(v) * 10.0).round() / 10.0);
            vec![t!("mpr-position", view = name, position = pos)]
        });
        let w = window();
        let mut panel = vec![
            t!(
                "medical-window",
                center = number(w.center),
                width = number(w.width)
            ),
            draw::slab_text(&views, super::load::Length::Mm),
        ];
        panel.extend(analysis.panel());
        let scene = Scene {
            views: &views,
            layout: l,
            readouts,
            labels: true,
            measures: [Vec::new(), Vec::new(), Vec::new()],
            panel,
            active: Some(0),
            vr: {
                let vr = Vr3::default();
                draw::VrLabels {
                    preset: t!("vr-preset", preset = vr.preset(true).id()),
                    mode: t!("vr-mode", mode = vr.mode.id()),
                    clip: t!("vr-clip", clip = vr.clip.id()),
                    smooth: false,
                }
            },
            segments: analysis.segment_list(),
            tracts: Default::default(),
            buttons_first: 0,
            maximised: false,
            curve: Default::default(),
        };
        let mut drawn = None;
        let shot = h.render_gpu(
            SIZE_3D,
            ui,
            |gpu, enc, target, size| {
                let planes: Vec<_> = (0..3)
                    .map(|v| views.plane(v, l.views[v], scale, w))
                    .collect();
                renderer.draw(gpu, enc, target, size, &tex, &planes);
                analysis.draw(gpu, enc, target, size, &planes, scale);
            },
            |dl, text| {
                drawn = Some(draw::draw(dl, text, &scene, &pal, false, 1.0));
            },
        );
        goldens.check(name, &shot);
        audit(name, SIZE_3D, false, &scene, &drawn.expect("drawn"));
    }
    goldens.finish();
}

/// MPR on a NIfTI volume: the phantom sampled into a RAS-stored NIfTI (i toward the patient's
/// right, j toward the front), loaded through [`Source::Research`] and drawn through the
/// crosshair at the bone sphere, which must sit on the patient's left as in the DICOM series.
#[test]
fn mpr_nifti_matches_goldens() {
    use super::load::{Loading, Source, lock};
    use std::time::Duration;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr-nifti",
    );
    let (dims, pixel, spacing) = ([96u16, 96, 64], 3.0f32, 4.5f32);
    let half = |n: u16, s: f32| (f32::from(n) - 1.0) * s / 2.0;
    let (hx, hy, hz) = (
        half(dims[0], pixel),
        half(dims[1], pixel),
        half(dims[2], spacing),
    );
    let mut values = Vec::new();
    for k in 0..dims[2] {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                // RAS to LPS: x and y change sign.
                let ras = [
                    f64::from(-hx + f32::from(i) * pixel),
                    f64::from(-hy + f32::from(j) * pixel),
                    f64::from(-hz + f32::from(k) * spacing),
                ];
                values.push(phantom::hu([-ras[0], -ras[1], ras[2]]) as i16);
            }
        }
    }
    let data = super::super::medical::tests::nifti_i16(
        dims,
        [
            [pixel, 0.0, 0.0, -hx],
            [0.0, pixel, 0.0, -hy],
            [0.0, 0.0, spacing, -hz],
        ],
        (-160.0, 240.0),
        "phantom",
        &values,
    );
    let path = std::env::temp_dir().join(format!("specular-mpr-nifti-{}.nii", std::process::id()));
    std::fs::write(&path, data).expect("write the NIfTI");
    let pool = std::sync::Arc::new(specular_core::JobPool::new(2, "mpr-shot"));
    let mut l = Loading::start(
        Source::Research(path.clone(), 0, None),
        pool,
        std::sync::Arc::new(|| {}),
        2048,
        1 << 30,
    );
    assert!(super::tests::wait(&mut l, Duration::from_secs(60)));
    let _ = std::fs::remove_file(&path);
    let info = l.info.clone().expect("opened");
    assert_eq!(info.full, [96, 96, 64]);
    let w = info.window.expect("the file's window");
    assert_eq!((w.center, w.width), (40.0, 400.0));
    let vol = lock(&l.volume);
    // The bone sphere on the patient's left.
    assert_eq!(vol.sample_patient(phantom::SPHERES[0].0), Some(1000.0));
    let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("texture");
    tex.sync(&h.gpu, &vol).expect("upload");
    let mut renderer = MprRenderer::new();
    let mut views = Views::new(&vol.geometry);
    views.c = [phantom::SPHERES[0].0[0], 0.0, 20.0];
    let scale = 1.0;
    let l = layout(
        [0.0, 0.0, f64::from(SIZE.0), f64::from(SIZE.1)],
        scale,
        false,
    );
    let pal = Palette::dark();
    // Streamlines beside it, in RAS: along each axis beside the crosshair, a circle in the
    // axial plane and a slanted line across the coronal plane.
    let mut tv = super::tracts::Tracts::default();
    tv.files.push((
        "arcs.tck".into(),
        std::sync::Arc::new(crosshair_tracts(views.c)),
    ));
    for (name, with) in [("mpr-nifti-1x", false), ("mpr-nifti-tracts", true)] {
        let shot =
            specular_i18n::I18n::with_thread_language(specular_i18n::langid!("en-US"), || {
                let readouts = [0, 1, 2].map(|v| {
                    let name = specular_i18n::I18n::global().tr(KINDS[v].message_id());
                    let pos = number((views.position(v) * 10.0).round() / 10.0);
                    vec![t!("mpr-position", view = name, position = pos)]
                });
                let mut scene = Scene {
                    views: &views,
                    layout: l,
                    readouts,
                    labels: true,
                    measures: [Vec::new(), Vec::new(), Vec::new()],
                    segments: Vec::new(),
                    buttons_first: 0,
                    maximised: false,
                    curve: Default::default(),
                    tracts: if with {
                        tv.pieces(&views, l)
                    } else {
                        Default::default()
                    },
                    panel: vec![
                        t!(
                            "medical-window",
                            center = number(w.center),
                            width = number(w.width)
                        ),
                        draw::slab_text(&views, super::load::Length::Mm),
                        t!("mpr-loaded", count = 64i64, width = 96i64, height = 96i64),
                    ],
                    active: Some(0),
                    vr: draw::VrLabels {
                        preset: t!("vr-preset", preset = SHADED.1.id()),
                        mode: t!("vr-mode", mode = SHADED.0.id()),
                        clip: t!("vr-clip", clip = SHADED.2.id()),
                        smooth: false,
                    },
                };
                if with {
                    scene.panel.extend(tv.text());
                }
                h.render_gpu(
                    SIZE,
                    1.0,
                    |gpu, enc, target, size| {
                        let planes: Vec<_> = (0..3)
                            .map(|v| views.plane(v, l.views[v], scale, w))
                            .collect();
                        renderer.draw(gpu, enc, target, size, &tex, &planes);
                    },
                    |dl, text| {
                        draw::draw(dl, text, &scene, &pal, false, 1.0);
                    },
                )
            });
        goldens.check(name, &shot);
    }
    goldens.finish();
}

/// Streamlines (RAS mm) about the patient point `c` (LPS): a line along each axis 12 mm
/// beside it, a circle of 40 mm in its axial plane and a line slanted 45° across its coronal plane.
fn crosshair_tracts(c: [f64; 3]) -> specular_decode::neuro_dec::Tractogram {
    let r = [-c[0] as f32, -c[1] as f32, c[2] as f32];
    let mut lines: Vec<Vec<[f32; 3]>> = Vec::new();
    for axis in 0..3 {
        lines.push(
            (-30..=30)
                .map(|k| {
                    // Beside the crosshair lines: 12 mm along the next axis.
                    let mut p = r;
                    p[axis] += 2.0 * k as f32;
                    p[(axis + 2) % 3] += 12.0;
                    p
                })
                .collect(),
        );
    }
    lines.push(
        (0..=48)
            .map(|k| {
                let a = k as f32 / 48.0 * std::f32::consts::TAU;
                [r[0] + 40.0 * a.cos(), r[1] + 40.0 * a.sin(), r[2]]
            })
            .collect(),
    );
    lines.push(
        (-30..=30)
            .map(|k| {
                let d = 2.0 * k as f32;
                [r[0] + d, r[1] + d, r[2] - 10.0]
            })
            .collect(),
    );
    let mut t = specular_decode::neuro_dec::Tractogram::default();
    t.offsets.push(0);
    for l in lines {
        t.points.extend(l);
        t.offsets.push(t.points.len() as u32);
    }
    t
}

/// A microscopy-like stack of three channels (the phantom's body and bone sphere, its fat
/// sphere, its rod and contrast sphere) drawn as a colour composite: red, green and blue
/// added, each through its own display range, in English and right to left in Hebrew at 1×
/// and 1.5×. Where only one channel has signal its colour shows alone; outside the volume
/// the canvas stays.
#[test]
fn composite_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mpr-composite",
    );
    let channel = |f: &dyn Fn([f64; 3]) -> f32| {
        let g = grid([64, 64, 56], 6.0, 8.0);
        let mut v = Volume::new(g, VoxelKind::F32, 1).expect("fits");
        let [w, hh, d] = v.geometry.dims;
        let mut buf = vec![0f32; (w * hh) as usize];
        for k in 0..d as usize {
            for j in 0..hh {
                for i in 0..w {
                    let p = v
                        .geometry
                        .to_patient([f64::from(i), f64::from(j), k as f64]);
                    buf[(j * w + i) as usize] = f(p);
                }
            }
            v.put(k, &buf, w, hh).expect("slice fits");
        }
        v
    };
    let vols = [
        channel(&|p| match phantom::hu(p) {
            1000.0 => 10.0,
            phantom::AIR => 0.0,
            _ => 2.0,
        }),
        channel(&|p| if phantom::hu(p) == -100.0 { 5.0 } else { 0.0 }),
        channel(&|p| match phantom::hu(p) {
            300.0 | phantom::ROD => 100.0,
            _ => 0.0,
        }),
    ];
    let windows = [(0.0, 10.0), (0.0, 5.0), (0.0, 200.0)].map(|(lo, hi): (f64, f64)| Window {
        center: (lo + hi) / 2.0,
        width: hi - lo,
        shape: VoiShape::LinearExact,
        invert: false,
    });
    let tints = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let texs: Vec<VolumeTexture> = vols
        .iter()
        .map(|v| {
            let mut t = VolumeTexture::new(&h.gpu, v).expect("texture");
            t.sync(&h.gpu, v).expect("upload");
            t
        })
        .collect();
    let mut views = Views::new(&vols[0].geometry);
    views.c = [0.0, -30.0, 40.0];
    let mut renderer = MprRenderer::new();
    let pal = Palette::dark();
    for (name, scale, rtl) in [
        ("mpr-composite-1x", 1.0f32, false),
        ("mpr-composite-1_5x", 1.5, false),
        ("mpr-composite-he-1x", 1.0, true),
        ("mpr-composite-he-1_5x", 1.5, true),
    ] {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let l = layout(
            [0.0, 0.0, f64::from(SIZE.0), f64::from(SIZE.1)],
            f64::from(scale),
            rtl,
        );
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let readouts = [0, 1, 2].map(|v| {
                let name = specular_i18n::I18n::global().tr(KINDS[v].message_id());
                let pos = number((views.position(v) * 10.0).round() / 10.0);
                vec![t!("mpr-position", view = name, position = pos)]
            });
            let scene = Scene {
                views: &views,
                layout: l,
                readouts,
                labels: true,
                measures: [Vec::new(), Vec::new(), Vec::new()],
                segments: Vec::new(),
                buttons_first: 0,
                maximised: false,
                curve: Default::default(),
                tracts: Default::default(),
                panel: vec![
                    t!("mpr-composite", count = 3i64),
                    draw::slab_text(&views, super::load::Length::Micron),
                ],
                active: Some(0),
                vr: draw::VrLabels {
                    preset: t!("vr-preset", preset = SHADED.1.id()),
                    mode: t!("vr-mode", mode = SHADED.0.id()),
                    clip: t!("vr-clip", clip = SHADED.2.id()),
                    smooth: false,
                },
            };
            h.render_gpu(
                SIZE,
                scale,
                |gpu, enc, target, size| {
                    let planes: Vec<Vec<_>> = windows
                        .iter()
                        .map(|w| {
                            (0..3)
                                .map(|v| views.plane(v, l.views[v], f64::from(scale), *w))
                                .collect()
                        })
                        .collect();
                    let layers: Vec<specular_render_volume::Layer> = (0..3)
                        .map(|k| specular_render_volume::Layer {
                            vol: &texs[k],
                            planes: &planes[k],
                            tint: tints[k],
                            stretch: specular_core::nd::Stretch::Linear,
                            curve: &[],
                            mid: 0.5,
                        })
                        .collect();
                    renderer.draw_layers(gpu, enc, target, size, &layers);
                },
                |dl, text| {
                    draw::draw(dl, text, &scene, &pal, rtl, 1.0);
                },
            )
        });
        goldens.check(name, &shot);
    }
    goldens.finish();
}

/// Channels fit the memory budget together or not at all (then what they need, in free-RAM
/// terms), and a channel's window follows the first one's as the user moves it.
#[test]
fn composite_budget_and_windows() {
    use super::{composite_fits, layer_window};
    assert!(composite_fits(100, 3, 300).is_ok());
    let need = composite_fits(100, 4, 300).expect_err("over the budget");
    assert_eq!((need.needed, need.free), (500, 375));
    let w = |c: f64, wd: f64| Window {
        center: c,
        width: wd,
        shape: VoiShape::LinearExact,
        invert: false,
    };
    let first = w(50.0, 100.0);
    let own = w(5.0, 10.0);
    assert_eq!(layer_window(first, None, own), own);
    // Half a width up and half as wide on the first: the same on the channel.
    let moved = layer_window(first, Some(w(100.0, 50.0)), own);
    assert!((moved.center - 10.0).abs() < 1e-9 && (moved.width - 5.0).abs() < 1e-9);
}
