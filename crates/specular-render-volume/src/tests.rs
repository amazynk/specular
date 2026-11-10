//! The shader against [`reference`] (the CPU sampling through `Volume::sample`, itself tested
//! against the analytic phantom in specular-dicom). GPU tests run on lavapipe and skip without
//! it.

use specular_dicom::volume::phantom;
use specular_render::VoiShape;
use specular_shots::{Harness, Shot};

use super::*;

#[test]
fn shader_variants_validate() {
    for t in ["f32", "i32", "u32"] {
        let src = shader_source(t);
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("mpr {t}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("mpr {t}: {e:?}"));
    }
}

/// Every phantom value lands inside the window, none clipped.
const WIDE: Window = Window {
    center: 0.0,
    width: 2400.0,
    shape: VoiShape::Linear,
    invert: false,
};

const SIZE: (u32, u32) = (96, 80);

fn srgb8(linear: f64) -> f64 {
    let c = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    c * 255.0
}

/// A phantom with slices `spacing` mm apart (varying when `uneven`), sheared by `tilt`°.
fn volume(size: [u32; 3], pixel: f64, spacing: f64, tilt: f64, uneven: bool) -> Volume {
    let mut pos = phantom::positions(size, pixel, spacing, tilt);
    if uneven {
        let shear = tilt.to_radians().tan();
        let mut a = pos[0][2];
        for (k, p) in pos.iter_mut().enumerate() {
            p[1] += (a - p[2]) * shear;
            p[2] = a;
            a += spacing * if k % 3 == 2 { 1.5 } else { 1.0 };
        }
    }
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [pixel, pixel],
        [size[0], size[1]],
    );
    let mut v = Volume::new(g, VoxelKind::I16, 1).unwrap();
    let mut buf = vec![0f32; (size[0] * size[1]) as usize];
    for k in 0..size[2] as usize {
        for j in 0..size[1] {
            for i in 0..size[0] {
                let p = v.geometry.to_patient([i as f64, j as f64, k as f64]);
                buf[(j * size[0] + i) as usize] = phantom::hu(p);
            }
        }
        v.put(k, &buf, size[0], size[1]).unwrap();
    }
    v
}

fn render(h: &mut Harness, vol: &Volume, planes: &[Plane]) -> Shot {
    let mut tex = VolumeTexture::new(&h.gpu, vol).expect("volume fits");
    tex.sync(&h.gpu, vol).unwrap();
    let mut mpr = MprRenderer::new();
    h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, view, size| mpr.draw(gpu, enc, view, size, &tex, planes),
        |_, _| {},
    )
}

/// Compares the shot with [`reference`] at every pixel centre, skipping pixels next to the
/// volume's edge (inside on one side of a rounding, outside on the other).
fn compare(shot: &Shot, vol: &Volume, plane: &Plane, tolerance: f64) {
    compare_toned(shot, vol, plane, tolerance, |g| g);
}

/// [`compare`] with `tone` applied to the windowed grey value (a stretch).
fn compare_toned(
    shot: &Shot,
    vol: &Volume,
    plane: &Plane,
    tolerance: f64,
    tone: impl Fn(f64) -> f64,
) {
    let at = |x: i64, y: i64| reference(vol, plane, x as f64 + 0.5, y as f64 + 0.5);
    let mut worst = 0f64;
    let mut checked = 0;
    for y in 0..SIZE.1 as i64 {
        for x in 0..SIZE.0 as i64 {
            let r = at(x, y);
            let edge = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|(dx, dy)| at(x + dx, y + dy).is_some() != r.is_some());
            if edge {
                continue;
            }
            let want = match r {
                Some(v) => srgb8(srgb_to_linear(tone(plane.window.grey(f64::from(v))))),
                None => 0.0,
            };
            let got = f64::from(shot.rgb[((y as usize) * SIZE.0 as usize + x as usize) * 3]);
            let d = (got - want).abs();
            assert!(
                d <= tolerance,
                "pixel ({x}, {y}): GPU {got}, CPU {want:.1} (value {r:?})"
            );
            worst = worst.max(d);
            checked += 1;
        }
    }
    assert!(
        checked > (SIZE.0 * SIZE.1 / 2) as usize,
        "too few pixels inside"
    );
    eprintln!("worst difference {worst:.2} over {checked} pixels");
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// An oblique plane through `centre`: the axial plane turned 30° about x, then 25° about z.
fn oblique(centre: [f64; 3], mm: f64) -> Plane {
    let (a, b) = (30f64.to_radians(), 25f64.to_radians());
    let right = [b.cos(), b.sin(), 0.0];
    let down0 = [-b.sin(), b.cos(), 0.0];
    let down = [down0[0] * a.cos(), down0[1] * a.cos(), a.sin()];
    Plane::centred([0, 0, SIZE.0, SIZE.1], centre, right, down, mm, WIDE)
}

#[test]
fn axial_and_sagittal_planes_match_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = volume([48, 40, 30], 7.0, 9.0, 0.0, false);
    let axial = Plane::centred(
        [0, 0, SIZE.0, SIZE.1],
        [10.0, 5.0, 2.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        3.7,
        WIDE,
    );
    let shot = render(&mut h, &vol, &[axial]);
    compare(&shot, &vol, &axial, 1.5);
    let sagittal = Plane::centred(
        [0, 0, SIZE.0, SIZE.1],
        [55.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, -1.0],
        3.3,
        WIDE,
    );
    let shot = render(&mut h, &vol, &[sagittal]);
    compare(&shot, &vol, &sagittal, 1.5);
}

/// A composite channel through each of the 2D view's stretches (square root, log, asinh,
/// histogram equalisation of the volume's values in the window, the automatic stretch's
/// midtones transfer) matches the CPU's
/// [`tone`]; the curve the shader interpolates is the science view's table resampled.
#[test]
fn stretched_layers_match_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = volume([48, 40, 30], 7.0, 9.0, 0.0, false);
    let values: Vec<f32> = (0..vol.voxels.len()).map(|i| vol.voxels.get(i)).collect();
    let (lo, hi) = (
        WIDE.center - WIDE.width / 2.0,
        WIDE.center + WIDE.width / 2.0,
    );
    let lut = specular_core::nd::histeq_lut(&values, lo as f32, hi as f32);
    let curve = tone_curve(&lut);
    assert_eq!(curve.len(), TONE_KNOTS);
    // The resampled curve follows the table at its knots.
    for k in [0, 64, 128, 255] {
        let x = k as f32 / 255.0;
        assert_eq!(curve[k], specular_core::nd::apply(Stretch::HistEq, x, &lut));
    }
    let plane = oblique(vol.geometry.centre(), 4.1);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("volume fits");
    tex.sync(&h.gpu, &vol).unwrap();
    let mut mpr = MprRenderer::new();
    for stretch in [
        Stretch::Linear,
        Stretch::Sqrt,
        Stretch::Log,
        Stretch::Asinh,
        Stretch::HistEq,
        Stretch::Auto,
    ] {
        let planes = [plane];
        // Auto: a strong midtones balance, as a faint astronomical background takes.
        let mid = if stretch == Stretch::Auto { 0.04 } else { 0.5 };
        let layer = Layer {
            vol: &tex,
            planes: &planes,
            tint: [1.0; 3],
            stretch,
            curve: &curve,
            mid,
        };
        let shot = h.render_gpu(
            (SIZE.0 as f32, SIZE.1 as f32),
            1.0,
            |gpu, enc, view, size| mpr.draw_layers(gpu, enc, view, size, &[layer]),
            |_, _| {},
        );
        eprintln!("{stretch:?}");
        let tol = 1.5;
        compare_toned(&shot, &vol, &plane, tol, |g| tone(stretch, g, &curve, mid));
    }
}

/// On a GSDF display a plane's greys go through the display's JND steps as the 2D view's
/// do (`specular_dicom::gsdf::p_luminance`, relative to its black and white); a colour
/// composite channel keeps its sRGB greys.
#[test]
fn gsdf_display_matches_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = volume([48, 40, 30], 7.0, 9.0, 0.0, false);
    let plane = oblique(vol.geometry.centre(), 4.1);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).expect("volume fits");
    tex.sync(&h.gpu, &vol).unwrap();
    let (l_min, l_max) = (0.5, 350.0);
    let mut mpr = MprRenderer::new();
    mpr.set_gsdf(Some(Gsdf {
        j_min: specular_dicom::gsdf::jnd_index(l_min),
        j_max: specular_dicom::gsdf::jnd_index(l_max),
        l_min,
        l_max,
    }));
    let planes = [plane];
    let shot = h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, view, size| mpr.draw(gpu, enc, view, size, &tex, &planes),
        |_, _| {},
    );
    let to_srgb = |l: f64| srgb8(l) / 255.0;
    compare_toned(&shot, &vol, &plane, 1.5, |g| {
        let l = specular_dicom::gsdf::p_luminance(g, l_min, l_max);
        to_srgb((l - l_min) / (l_max - l_min))
    });
    // A tinted channel: plain greys in its colour.
    let layer = Layer {
        vol: &tex,
        planes: &planes,
        tint: [1.0, 0.0, 0.0],
        stretch: Stretch::Linear,
        curve: &[],
        mid: 0.5,
    };
    let shot = h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, view, size| mpr.draw_layers(gpu, enc, view, size, &[layer]),
        |_, _| {},
    );
    compare(&shot, &vol, &plane, 1.5);
}

#[test]
fn oblique_plane_through_tilted_uneven_slices_matches_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = volume([48, 40, 30], 7.0, 8.0, 15.0, true);
    let plane = oblique(vol.geometry.centre(), 4.1);
    let shot = render(&mut h, &vol, &[plane]);
    // The slice table is linear between its entries: up to ~0.03 slice off the exact
    // position between uneven slices.
    compare(&shot, &vol, &plane, 2.5);
}

#[test]
fn slabs_match_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = volume([48, 40, 30], 7.0, 9.0, 0.0, false);
    for slab in [Slab::Mip, Slab::MinIp, Slab::Average] {
        let mut plane = oblique([20.0, 0.0, 10.0], 3.9);
        plane.thickness = 30.0;
        plane.slab = slab;
        let shot = render(&mut h, &vol, &[plane]);
        compare(&shot, &vol, &plane, 1.5);
    }
}

#[test]
fn partial_volume_blends_the_loaded_slices() {
    let Some(mut h) = Harness::new() else { return };
    let mut vol = volume([48, 40, 30], 7.0, 9.0, 0.0, false);
    for (k, l) in vol.loaded.iter_mut().enumerate() {
        *l = k % 4 == 1 || k == 18;
    }
    let plane = oblique([0.0, 0.0, 0.0], 3.9);
    let shot = render(&mut h, &vol, &[plane]);
    compare(&shot, &vol, &plane, 1.5);
}

#[test]
fn planes_stay_in_their_rectangles() {
    let Some(mut h) = Harness::new() else { return };
    let vol = volume([32, 32, 20], 10.0, 10.0, 0.0, false);
    let half = SIZE.0 / 2;
    let mut left = Plane::centred(
        [0, 0, half, SIZE.1],
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        5.0,
        WIDE,
    );
    left.canvas = [1.0, 1.0, 1.0, 1.0];
    let right = Plane::centred(
        [half, 0, half, SIZE.1],
        [0.0; 3],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, -1.0],
        5.0,
        WIDE,
    );
    let shot = render(&mut h, &vol, &[left, right]);
    // The left plane's white canvas shows at its corner (outside the body, inside the
    // rectangle is air at −1000, so far corners of a 32×32×10 mm volume are canvas) and
    // never in the right half.
    let px = |x: u32, y: u32| shot.rgb[((y * SIZE.0 + x) * 3) as usize];
    assert_eq!(px(0, 0), 255, "left canvas");
    assert!(
        px(half, 0) < 255,
        "right plane drew over its own rectangle only"
    );
    // Each centre shows its plane's centre value.
    let want = srgb8(srgb_to_linear(WIDE.grey(f64::from(phantom::TISSUE))));
    for x in [half / 2, half + half / 2] {
        let got = f64::from(px(x, SIZE.1 / 2));
        assert!(
            (got - want).abs() <= 3.0,
            "centre at x {x}: {got} vs {want}"
        );
    }
}

#[test]
fn texture_follows_promotion_and_reports_memory() {
    let Some(h) = Harness::new() else { return };
    let mut vol = volume([16, 16, 8], 10.0, 10.0, 0.0, false);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).unwrap();
    assert!(tex.sync(&h.gpu, &vol).unwrap());
    assert!(!tex.sync(&h.gpu, &vol).unwrap(), "nothing new");
    assert_eq!(tex.uploaded(), 8);
    assert_eq!(tex.kind(), VoxelKind::I16);
    assert!(tex.bytes() >= 16 * 16 * 8 * 2);
    vol.promote();
    assert!(tex.sync(&h.gpu, &vol).unwrap());
    assert_eq!(tex.kind(), VoxelKind::F32);
    assert_eq!(tex.uploaded(), 8);
    assert!(tex.bytes() >= 16 * 16 * 8 * 4);
}

#[test]
fn oversized_volumes_are_refused() {
    let Some(h) = Harness::new() else { return };
    let max = max_dimension(&h.gpu);
    let pos = phantom::positions([4, 4, 3], 1.0, 1.0, 0.0);
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0],
        [max + 1, 4],
    );
    let vol = Volume::new(g, VoxelKind::I16, 1).unwrap();
    assert!(VolumeTexture::new(&h.gpu, &vol).is_err());
}

#[test]
fn overlay_shaders_validate() {
    let mut sources = vec![("labels", overlay::labels_shader_source())];
    for t in ["f32", "i32", "u32"] {
        sources.push(("fusion", overlay::fusion_shader_source(t)));
    }
    sources.push(("isodose", overlay::isodose_shader_source()));
    for (name, src) in sources {
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}

#[test]
fn colormaps_and_rigid_moves() {
    assert_eq!(Colormap::HotIron.srgb(0), [0, 0, 0]);
    assert_eq!(Colormap::HotIron.srgb(255), [255, 254, 252]);
    assert_eq!(Colormap::HotIron.srgb(100), [200, 0, 0]);
    assert_eq!(Colormap::Viridis.srgb(0), [0x44, 0x01, 0x54]);
    assert_eq!(Colormap::Viridis.srgb(255), [0xfd, 0xe7, 0x25]);
    assert_eq!(Colormap::Rainbow.srgb(0)[2], 128);
    assert_eq!(Colormap::Rainbow.srgb(255)[0], 128);
    assert_eq!(Colormap::HotIron.next().next().next(), Colormap::HotIron);
    let r = Rigid {
        translate: [3.0, -2.0, 5.0],
        rotate_deg: [10.0, -20.0, 35.0],
        centre: [1.0, 2.0, 3.0],
    };
    let p = [12.0, -7.0, 40.0];
    let back = r.to_fixed(r.to_moving(p));
    assert!((0..3).all(|c| (back[c] - p[c]).abs() < 1e-9), "{back:?}");
    // The moved plane samples the moving series where the fixed pixel shows it.
    let plane = oblique([5.0, 5.0, 5.0], 2.0);
    let moved = r.plane(&plane);
    let (want, got) = (r.to_moving(plane.at(17.5, 9.5)), moved.at(17.5, 9.5));
    assert!((0..3).all(|c| (want[c] - got[c]).abs() < 1e-9));
    // A z rotation of 90° carries +x to +y.
    let q = Rigid {
        rotate_deg: [0.0, 0.0, 90.0],
        ..Rigid::default()
    }
    .to_fixed([1.0, 0.0, 0.0]);
    assert!(q[0].abs() < 1e-12 && (q[1] - 1.0).abs() < 1e-12);
}

/// Clears the scene to black, then runs `draw`.
fn on_black(
    h: &mut Harness,
    draw: impl FnOnce(&Gpu, &mut wgpu::CommandEncoder, &wgpu::TextureView, (u32, u32)),
) -> Shot {
    h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, view, size| {
            drop(enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            }));
            draw(gpu, enc, view, size);
        },
        |_, _| {},
    )
}

/// Compares an overlay shot (over black) with a CPU reference of premultiplied colour,
/// allowing `ties` of the pixels to differ (nearest-voxel and colour-index rounding ties).
fn compare_rgba(shot: &Shot, want: impl Fn(f64, f64) -> Option<[f32; 4]>, ties: f64) {
    let mut off = 0usize;
    let mut drawn = 0usize;
    for y in 0..SIZE.1 {
        for x in 0..SIZE.0 {
            let w = want(f64::from(x) + 0.5, f64::from(y) + 0.5).unwrap_or([0.0; 4]);
            if w[3] > 0.0 {
                drawn += 1;
            }
            let i = ((y * SIZE.0 + x) * 3) as usize;
            let bad = (0..3).any(|c| {
                let e = srgb8(f64::from(w[c]));
                (f64::from(shot.rgb[i + c]) - e).abs() > 2.0
            });
            off += usize::from(bad);
        }
    }
    let total = (SIZE.0 * SIZE.1) as usize;
    eprintln!("{off} of {total} pixels differ, {drawn} drawn");
    assert!(drawn > total / 20, "too little drawn: {drawn}");
    assert!(
        (off as f64) <= ties * total as f64,
        "{off} of {total} pixels differ"
    );
}

/// A label volume of two separate balls (segments 0 and 1) and a fractional layer.
fn labels() -> (Volume, Volume) {
    let size = [40u32, 36, 24];
    let pos = phantom::positions(size, 2.5, 3.0, 0.0);
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.5, 2.5],
        [size[0], size[1]],
    );
    let n = (size[0] * size[1] * size[2]) as usize;
    let (mut ids, mut frac) = (vec![0u16; n], vec![0u16; n]);
    let c = g.centre();
    for k in 0..size[2] {
        for j in 0..size[1] {
            for i in 0..size[0] {
                let p = g.to_patient([f64::from(i), f64::from(j), f64::from(k)]);
                let d = |o: [f64; 3]| {
                    (0..3)
                        .map(|a| (p[a] - c[a] - o[a]).powi(2))
                        .sum::<f64>()
                        .sqrt()
                };
                let idx = ((k * size[1] + j) * size[0] + i) as usize;
                if d([-18.0, 0.0, 0.0]) < 16.0 {
                    ids[idx] = 1;
                } else if d([20.0, 5.0, 0.0]) < 12.0 {
                    ids[idx] = 2;
                }
                let r = d([0.0, -10.0, 0.0]);
                frac[idx] = ((1.0 - r / 30.0).clamp(0.0, 1.0) * 65535.0) as u16;
            }
        }
    }
    let vol = |v| Volume {
        geometry: g.clone(),
        voxels: Voxels::U16(v),
        loaded: vec![true; size[2] as usize],
        step_xy: 1,
    };
    (vol(ids), vol(frac))
}

#[test]
fn labels_match_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let (ids, frac) = labels();
    let style = LabelStyle {
        colours: vec![
            LabelStyle::colour([230, 60, 40], 1.0),
            LabelStyle::colour([40, 200, 90], 0.8),
            LabelStyle::colour([60, 120, 255], 1.0),
        ],
        fill: 0.4,
        outline: 1.5,
    };
    for (vol, fraction) in [(&ids, None), (&frac, Some(2))] {
        let mut tex = VolumeTexture::new(&h.gpu, vol).unwrap();
        tex.sync(&h.gpu, vol).unwrap();
        for plane in [
            Plane::centred(
                [0, 0, SIZE.0, SIZE.1],
                vol.geometry.centre(),
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                1.3,
                WIDE,
            ),
            oblique(vol.geometry.centre(), 1.2),
        ] {
            let mut r = LabelRenderer::new();
            let layer = LabelLayer {
                texture: &tex,
                fraction,
                base: 0,
            };
            let shot = on_black(&mut h, |gpu, enc, view, size| {
                r.draw(gpu, enc, view, size, &[layer], &style, &[plane])
            });
            compare_rgba(
                &shot,
                |x, y| labels_reference(vol, fraction, 0, &style, &plane, x, y),
                0.01,
            );
        }
    }
    // Hidden segments draw nothing.
    let mut hidden = style.clone();
    hidden.colours[0][3] = 0.0;
    hidden.colours[1][3] = 0.0;
    let mut tex = VolumeTexture::new(&h.gpu, &ids).unwrap();
    tex.sync(&h.gpu, &ids).unwrap();
    let plane = oblique(ids.geometry.centre(), 1.2);
    let mut r = LabelRenderer::new();
    let layer = LabelLayer {
        texture: &tex,
        fraction: None,
        base: 0,
    };
    let shot = on_black(&mut h, |gpu, enc, view, size| {
        r.draw(gpu, enc, view, size, &[layer], &hidden, &[plane])
    });
    assert!(shot.rgb.iter().all(|&v| v == 0));
}

#[test]
fn fusion_matches_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = volume([48, 40, 30], 7.0, 9.0, 0.0, false);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).unwrap();
    tex.sync(&h.gpu, &vol).unwrap();
    let window = Window {
        center: 40.0,
        width: 400.0,
        shape: VoiShape::Linear,
        invert: false,
    };
    let rigid = Rigid {
        translate: [6.0, -4.0, 3.0],
        rotate_deg: [4.0, -3.0, 8.0],
        centre: vol.geometry.centre(),
    };
    for (map, slab) in [
        (Colormap::HotIron, Slab::Average),
        (Colormap::Rainbow, Slab::Average),
        (Colormap::Viridis, Slab::Mip),
    ] {
        let mut fixed = oblique([10.0, 0.0, 5.0], 3.6);
        fixed.window = window;
        if slab == Slab::Mip {
            fixed.thickness = 20.0;
            fixed.slab = slab;
        }
        let plane = rigid.plane(&fixed);
        let mut f = FusionRenderer::new();
        let shot = on_black(&mut h, |gpu, enc, view, size| {
            f.draw(gpu, enc, view, size, &tex, map, 0.6, &[plane])
        });
        compare_rgba(
            &shot,
            |x, y| fusion_reference(&vol, map, 0.6, &plane, x, y),
            0.02,
        );
    }
}

/// A dose volume (f32): a smooth peak of 60 Gy falling off with distance from a point off
/// the centre, on a coarser grid than the image.
fn dose() -> Volume {
    let size = [30u32, 26, 20];
    let pos = phantom::positions(size, 4.0, 3.0, 0.0);
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [4.0, 4.0],
        [size[0], size[1]],
    );
    let c = g.centre();
    let mut v = Volume::new(g, VoxelKind::F32, 1).unwrap();
    let mut buf = vec![0f32; (size[0] * size[1]) as usize];
    for k in 0..size[2] as usize {
        for j in 0..size[1] {
            for i in 0..size[0] {
                let p = v
                    .geometry
                    .to_patient([f64::from(i), f64::from(j), k as f64]);
                let r2: f64 = [6.0, -4.0, 2.0]
                    .iter()
                    .enumerate()
                    .map(|(a, o)| (p[a] - c[a] - o).powi(2))
                    .sum();
                buf[(j * size[0] + i) as usize] =
                    (60.0 * (-r2 / (2.0 * 30.0f64.powi(2))).exp()) as f32;
            }
        }
        v.put(k, &buf, size[0], size[1]).unwrap();
    }
    v
}

#[test]
fn isodose_lines_match_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = dose();
    let mut tex = VolumeTexture::new(&h.gpu, &vol).unwrap();
    tex.sync(&h.gpu, &vol).unwrap();
    let levels: Vec<IsodoseLevel> = [
        (57.0, [255, 0, 0]),
        (48.0, [255, 255, 0]),
        (30.0, [0, 255, 0]),
        (18.0, [0, 0, 255]),
    ]
    .into_iter()
    .map(|(dose, srgb)| IsodoseLevel { dose, srgb })
    .collect();
    for wash in [0.0, 0.25] {
        let style = IsodoseStyle {
            levels: levels.clone(),
            width: 1.5,
            wash,
        };
        for plane in [
            Plane::centred(
                [0, 0, SIZE.0, SIZE.1],
                vol.geometry.centre(),
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                1.4,
                WIDE,
            ),
            oblique(vol.geometry.centre(), 1.3),
        ] {
            let mut r = IsodoseRenderer::new();
            let shot = on_black(&mut h, |gpu, enc, view, size| {
                r.draw(gpu, enc, view, size, &tex, &style, &[plane])
            });
            compare_rgba(
                &shot,
                |x, y| isodose_reference(&vol, &style, &plane, x, y),
                0.01,
            );
        }
    }
}

/// The overlay's cost on a 1920×1080 target: three thin planes of labels (two layers, one
/// fractional) and of a fused volume, each draw submitted and waited for. Printed; lavapipe
/// is a CPU rasteriser, so this is an upper bound for real GPUs.
#[test]
fn overlay_draw_cost() {
    let Some(h) = Harness::new() else { return };
    let gpu = &h.gpu;
    let size = (1920u32, 1080u32);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cost"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: specular_gfx::HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let (ids, frac) = labels();
    let mut ids_tex = VolumeTexture::new(gpu, &ids).unwrap();
    ids_tex.sync(gpu, &ids).unwrap();
    let mut frac_tex = VolumeTexture::new(gpu, &frac).unwrap();
    frac_tex.sync(gpu, &frac).unwrap();
    let vol = volume([128, 128, 96], 2.0, 2.5, 0.0, false);
    let mut vol_tex = VolumeTexture::new(gpu, &vol).unwrap();
    vol_tex.sync(gpu, &vol).unwrap();
    let style = LabelStyle {
        colours: vec![
            LabelStyle::colour([230, 60, 40], 1.0),
            LabelStyle::colour([40, 200, 90], 1.0),
            LabelStyle::colour([60, 120, 255], 1.0),
        ],
        fill: 0.35,
        outline: 1.5,
    };
    let w = size.0 / 3;
    let planes: Vec<Plane> = (0..3)
        .map(|i| {
            let (du, dv) = match i {
                0 => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
                1 => ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
                _ => ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
            };
            Plane::centred(
                [i * w, 0, w, size.1],
                ids.geometry.centre(),
                du,
                dv,
                0.15,
                WIDE,
            )
        })
        .collect();
    let mut labels_r = LabelRenderer::new();
    let mut fusion_r = FusionRenderer::new();
    let layers = [
        LabelLayer {
            texture: &ids_tex,
            fraction: None,
            base: 0,
        },
        LabelLayer {
            texture: &frac_tex,
            fraction: Some(2),
            base: 0,
        },
    ];
    let time = |f: &mut dyn FnMut(&mut wgpu::CommandEncoder)| {
        let mut best = f64::MAX;
        for _ in 0..6 {
            let t = std::time::Instant::now();
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            f(&mut enc);
            gpu.queue.submit([enc.finish()]);
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        }
        best
    };
    let empty = time(&mut |_| {});
    let label_ms = time(&mut |enc| labels_r.draw(gpu, enc, &view, size, &layers, &style, &planes));
    let fusion_ms = time(&mut |enc| {
        fusion_r.draw(
            gpu,
            enc,
            &view,
            size,
            &vol_tex,
            Colormap::HotIron,
            0.5,
            &planes,
        )
    });
    eprintln!(
        "overlay cost at 1920x1080, 3 planes (best of 6, submit to idle, empty {empty:.2} ms): \
         labels (2 layers) {label_ms:.2} ms, fusion {fusion_ms:.2} ms"
    );
}
