//! The ray marcher against [`reference`] (the same rays through `Volume::sample` on the
//! CPU), skipping against no skipping, and refinement. GPU tests run on lavapipe and skip
//! without it.

use specular_dicom::volume::phantom;
use specular_render::VoiShape;
use specular_shots::{Harness, Shot};

use super::*;
use crate::tf::Preset;

#[test]
fn shader_variants_validate() {
    let mut sources: Vec<(String, String)> = ["f32", "i32", "u32"]
        .iter()
        .map(|t| (format!("vr {t}"), shader_source(t)))
        .collect();
    sources.push(("present".into(), present_source()));
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

const SIZE: (u32, u32) = (96, 80);

const WIDE: Window = Window {
    center: 0.0,
    width: 2400.0,
    shape: VoiShape::Linear,
    invert: false,
};

/// Looking down at the phantom from the front-left, 25° above.
fn view(vol: &Volume, mode: Mode, preset: Preset) -> View3d {
    let (a, e) = (35f64.to_radians(), 25f64.to_radians());
    let right = [a.cos(), a.sin(), 0.0];
    let down = [a.sin() * e.sin(), -a.cos() * e.sin(), -e.cos()];
    let transfer = preset.transfer((40.0, 400.0));
    View3d {
        rect: [0, 0, SIZE.0, SIZE.1],
        centre: vol.geometry.centre(),
        right,
        down,
        mm: 5.2,
        mode,
        preset,
        series: (40.0, 400.0),
        tf_window: transfer.window(),
        window: WIDE,
        clip: Clip::default(),
        canvas: [0.02, 0.03, 0.05, 1.0],
        interactive: false,
    }
}

fn render(
    h: &mut Harness,
    vol: &Volume,
    bricks: Option<&Bricks>,
    views: &[View3d],
) -> (Shot, bool) {
    let mut tex = VolumeTexture::new(&h.gpu, vol).expect("volume fits");
    tex.sync(&h.gpu, vol).unwrap();
    let mut vr = VrRenderer::new();
    let mut more = false;
    let shot = h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, target, size| {
            for v in views {
                more = vr.draw(gpu, enc, target, size, &tex, bricks, v);
            }
        },
        |_, _| {},
    );
    (shot, more)
}

fn srgb8(linear: f64) -> f64 {
    let c = linear.clamp(0.0, 1.0);
    let c = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    c * 255.0
}

/// Compares every pixel with [`reference`] composited over the canvas: at most `outliers`
/// of them (rays grazing a surface, where f32 and f64 round a sample to opposite sides of
/// the transfer function's ramp) may differ by more than `tolerance` levels.
fn compare(shot: &Shot, vol: &Volume, v: &View3d, tolerance: f64, outliers: f64) {
    let mut bad = 0;
    let mut worst = 0f64;
    let mut lit = 0;
    for y in 0..SIZE.1 as usize {
        for x in 0..SIZE.0 as usize {
            let r = reference(vol, v, x as f64 + 0.5, y as f64 + 0.5);
            if r[3] > 0.5 {
                lit += 1;
            }
            for c in 0..3 {
                let want = srgb8(f64::from(r[c] + v.canvas[c] * (1.0 - r[3])));
                let got = f64::from(shot.rgb[(y * SIZE.0 as usize + x) * 3 + c]);
                let d = (got - want).abs();
                if d > tolerance {
                    bad += 1;
                }
                worst = worst.max(d);
            }
        }
    }
    let n = (SIZE.0 * SIZE.1 * 3) as f64;
    eprintln!(
        "{:?}: {bad} of {n} channels over {tolerance}, worst {worst:.1}, {lit} opaque pixels",
        v.mode
    );
    assert!(lit > 200, "too little drawn");
    assert!(
        f64::from(bad) <= n * outliers,
        "{bad} channels differ by more than {tolerance}"
    );
}

#[test]
fn rays_match_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    // A gantry tilt: the slice lookup is exact for evenly spaced slices.
    let vol = phantom::volume([48, 48, 40], 7.0, 10.0, 12.0);
    for (mode, preset) in [
        (Mode::Shaded, Preset::CtBone),
        (Mode::Shaded, Preset::CtSoft),
        (Mode::Mip, Preset::CtBone),
        (Mode::MinIp, Preset::CtBone),
    ] {
        let v = view(&vol, mode, preset);
        let (shot, more) = render(&mut h, &vol, None, &[v]);
        assert_eq!(more, mode == Mode::Shaded, "{mode:?}");
        compare(&shot, &vol, &v, 3.0, 0.01);
    }
}

#[test]
fn clipped_rays_match_the_cpu() {
    let Some(mut h) = Harness::new() else { return };
    let vol = phantom::volume([48, 48, 40], 7.0, 10.0, 0.0);
    let mut v = view(&vol, Mode::Shaded, Preset::CtSoft);
    // Keep x ≤ 100 and z ≥ -150; cut away the corner x > 0, y < 0, z > 0 (facing the view).
    v.clip.keep[0] = [-1.0, 0.0, 0.0, 100.0];
    v.clip.keep[1] = [0.0, 0.0, 1.0, 150.0];
    v.clip.keeps = 2;
    v.clip.cut = Some([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);
    let (shot, _) = render(&mut h, &vol, None, &[v]);
    compare(&shot, &vol, &v, 3.0, 0.01);
    // The cut shows the inside: the bone sphere at x = 60 is visible through it.
    let (unclipped, _) = render(
        &mut h,
        &vol,
        None,
        &[view(&vol, Mode::Shaded, Preset::CtSoft)],
    );
    assert_ne!(shot.rgb, unclipped.rgb);
}

#[test]
fn empty_space_skipping_changes_nothing() {
    let Some(mut h) = Harness::new() else { return };
    let vol = phantom::volume([64, 64, 48], 5.5, 8.5, 8.0);
    let bricks = Bricks::compute(&vol);
    for (mode, preset) in [
        (Mode::Shaded, Preset::CtBone),
        (Mode::Shaded, Preset::CtLung),
        (Mode::Mip, Preset::CtBone),
        (Mode::MinIp, Preset::CtBone),
    ] {
        let v = view(&vol, mode, preset);
        let (plain, _) = render(&mut h, &vol, None, &[v]);
        let (skipped, _) = render(&mut h, &vol, Some(&bricks), &[v]);
        let diff = plain
            .rgb
            .iter()
            .zip(&skipped.rgb)
            .filter(|(a, b)| a.abs_diff(**b) > 1)
            .count();
        assert!(diff <= 3, "{mode:?} {preset:?}: {diff} channels differ");
    }
}

#[test]
fn frames_refine_then_stop() {
    let Some(h) = Harness::new() else { return };
    let vol = phantom::volume([32, 32, 24], 10.0, 16.0, 0.0);
    let mut tex = VolumeTexture::new(&h.gpu, &vol).unwrap();
    tex.sync(&h.gpu, &vol).unwrap();
    let target = h.gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 40,
            height: 30,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let mut vr = VrRenderer::new();
    let mut v = view(&vol, Mode::Cinematic, Preset::CtBone);
    v.rect = [0, 0, 40, 30];
    v.mm = 14.0;
    let mut frames = 0;
    loop {
        let mut enc = h.gpu.device.create_command_encoder(&Default::default());
        let more = vr.draw(&h.gpu, &mut enc, &tv, (40, 30), &tex, None, &v);
        h.gpu.queue.submit([enc.finish()]);
        frames += 1;
        if !more {
            break;
        }
        assert!(frames < 100);
    }
    assert_eq!(frames, v.frames());
    // Unchanged, nothing more is drawn; a change starts over.
    let mut enc = h.gpu.device.create_command_encoder(&Default::default());
    assert!(!vr.draw(&h.gpu, &mut enc, &tv, (40, 30), &tex, None, &v));
    v.interactive = true;
    assert!(!vr.draw(&h.gpu, &mut enc, &tv, (40, 30), &tex, None, &v));
    assert_eq!(vr.frame(), 1);
    v.interactive = false;
    assert!(vr.draw(&h.gpu, &mut enc, &tv, (40, 30), &tex, None, &v));
    h.gpu.queue.submit([enc.finish()]);
}

/// Frame times for a 512 × 512 × 500 phantom on the real GPU, in the 3D slot of a
/// 1280 × 800 window at 1× and at 1.5×, and of a 1920 × 1080 window at 1.5×: `cargo test
/// -p specular-render-volume --release bench_512 -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark: needs a GPU and 1 GB of memory"]
fn bench_512_cube() {
    use std::time::{Duration, Instant};
    let gpu = Gpu::new().expect("a GPU");
    let t = Instant::now();
    let vol = phantom::volume([512, 512, 500], 0.7, 0.8, 0.0);
    eprintln!("phantom built in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let bricks = Bricks::compute(&vol);
    eprintln!("bricks in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let mesh = crate::iso::isosurface(&vol, 300.0);
    eprintln!(
        "isosurface at 300 HU: {} triangles in {:.0} ms",
        mesh.triangles.len(),
        t.elapsed().as_secs_f64() * 1e3
    );
    let mut tex = VolumeTexture::new(&gpu, &vol).expect("volume fits");
    tex.sync(&gpu, &vol).unwrap();
    let wait = || {
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).ok();
    };
    wait();
    // The radius of the bounding sphere, as the app fits it.
    let radius = (2.0 * (256.0f64 * 0.7).powi(2) + (250.0f64 * 0.8).powi(2)).sqrt();
    for (label, w, h) in [
        ("1280x800 at 1x", 630u32, 330u32),
        ("1280x800 at 1.5x", 945, 500),
        ("1920x1080 at 1.5x", 1430, 700),
    ] {
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let tv = target.create_view(&Default::default());
        for (mode, preset) in [
            (Mode::Shaded, Preset::CtBone),
            (Mode::Shaded, Preset::CtSoft),
            (Mode::Cinematic, Preset::CtBone),
            (Mode::Mip, Preset::CtBone),
        ] {
            let mut vr = VrRenderer::new();
            let mut v = view(&vol, mode, preset);
            v.rect = [0, 0, w, h];
            v.mm = 2.0 * radius * 0.85 / f64::from(w.min(h));
            // Orbiting: a new angle every frame, coarse.
            v.interactive = true;
            let mut times = Vec::new();
            for i in 0..40 {
                let a = 0.3 + f64::from(i) * 0.05;
                v.right = [a.cos(), a.sin(), 0.0];
                v.down = [0.0, 0.0, -1.0];
                let t = Instant::now();
                let mut enc = gpu.device.create_command_encoder(&Default::default());
                vr.draw(&gpu, &mut enc, &tv, (w, h), &tex, Some(&bricks), &v);
                gpu.queue.submit([enc.finish()]);
                wait();
                if i >= 5 {
                    times.push(t.elapsed());
                }
            }
            times.sort();
            let orbit = times[times.len() / 2];
            let worst = *times.last().unwrap();
            // At rest: every refinement frame.
            v.interactive = false;
            let mut frames = Vec::new();
            loop {
                let t = Instant::now();
                let mut enc = gpu.device.create_command_encoder(&Default::default());
                let more = vr.draw(&gpu, &mut enc, &tv, (w, h), &tex, Some(&bricks), &v);
                gpu.queue.submit([enc.finish()]);
                wait();
                frames.push(t.elapsed());
                if !more {
                    break;
                }
            }
            let total: Duration = frames.iter().sum();
            let mut sorted = frames.clone();
            sorted.sort();
            eprintln!(
                "{label} {mode:?} {preset:?}: orbiting median {:.1} ms (worst {:.1}); at rest {} frames, median {:.1} ms, worst {:.1}, {:.0} ms in all",
                orbit.as_secs_f64() * 1e3,
                worst.as_secs_f64() * 1e3,
                frames.len(),
                sorted[sorted.len() / 2].as_secs_f64() * 1e3,
                sorted.last().unwrap().as_secs_f64() * 1e3,
                total.as_secs_f64() * 1e3,
            );
        }
    }
    eprintln!("on {}", gpu.info.name);
}
