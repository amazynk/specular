use wgpu::util::DeviceExt;

use super::*;
use crate::model::{Edit, ModuleKind, Registry, SourceKey};
use crate::testgpu;

fn params(proof: bool) -> ClipParams {
    let m = crate::ops::m3_f32(&crate::colour::rec2020_to_rec709());
    ClipParams {
        scene_white: 6.0,
        scene_black: 0.001,
        peak: 1.0,
        proof_gamut: proof,
        to_output: m,
    }
}

#[test]
fn scene_limits_follow_the_look() {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    let (w, b) = scene_limits(&e);
    let v = view::settings(&e);
    let c = view::Curve::new(&v);
    // The look reaches the top code at the white limit and not just below it.
    let top = c.black + f64::from(HIGH) * (c.white - c.black);
    assert!(view::eval(&v, f64::from(w)) >= top - 1e-9);
    assert!(view::eval(&v, f64::from(w) * 0.99) < top);
    assert!(view::eval(&v, f64::from(b)) >= c.black + f64::from(LOW) - 1e-9);
    assert!(w > 1.0 && b > 0.0 && b < 0.01, "{w} {b}");
    // More contrast clips sooner.
    if let Some(m) = e
        .modules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::VIEW_TRANSFORM)
    {
        let f = m
            .params
            .to_fields()
            .with("contrast", crate::model::Value::Float(3.0));
        m.params = crate::model::Params::from_fields(ModuleKind::VIEW_TRANSFORM, f);
        m.enabled = true;
        let (w2, _) = scene_limits(&e);
        assert!(w2 < w, "{w2} vs {w}");
    }
}

#[test]
fn kinds_and_priority() {
    let p = params(false);
    let all = ClipFlags {
        scene: true,
        display: true,
        gamut: true,
    };
    let mid = [0.3, 0.3, 0.3, 1.0];
    assert_eq!(
        classify([1.0, 0.5, 0.2, 1.0], None, None, &p, all),
        DISPLAY_HIGH
    );
    assert_eq!(
        classify([0.0, 0.0, 0.0, 1.0], None, None, &p, all),
        DISPLAY_LOW
    );
    assert_eq!(
        classify(mid, Some([7.0, 1.0, 1.0]), None, &p, all),
        SCENE_HIGH
    );
    assert_eq!(classify(mid, Some([0.0005; 3]), None, &p, all), SCENE_LOW);
    // A saturated Rec.2020 green is outside Rec.709.
    assert_eq!(
        classify(mid, Some([0.3; 3]), Some([0.0, 0.5, 0.0]), &p, all),
        GAMUT
    );
    assert_eq!(
        classify(mid, Some([0.3; 3]), Some([0.2, 0.2, 0.2]), &p, all),
        [0; 4]
    );
    // Display wins over scene.
    assert_eq!(
        classify([1.0; 4], Some([9.0; 3]), None, &p, all),
        DISPLAY_HIGH
    );
    // Off means off.
    assert_eq!(
        classify([1.0; 4], Some([9.0; 3]), None, &p, ClipFlags::default()),
        [0; 4]
    );
    // Proofing: alpha is the round-trip ΔE.
    let pp = params(true);
    assert_eq!(classify([0.3, 0.3, 0.3, 9.0], None, None, &pp, all), GAMUT);
    assert_eq!(classify([0.3, 0.3, 0.3, 0.5], None, None, &pp, all), [0; 4]);
}

fn up(d: &wgpu::Device, px: &[f32], prec: Precision) -> wgpu::Buffer {
    let bytes: Vec<u8> = match prec {
        Precision::F32 => bytemuck::cast_slice(px).to_vec(),
        Precision::F16 => px
            .iter()
            .flat_map(|v| half::f16::from_f32(*v).to_bits().to_le_bytes())
            .collect(),
    };
    d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("clip test"),
        contents: &bytes,
        usage: wgpu::BufferUsages::STORAGE,
    })
}

fn rounded(px: &[f32], prec: Precision) -> Vec<f32> {
    match prec {
        Precision::F32 => px.to_vec(),
        Precision::F16 => px
            .iter()
            .map(|v| half::f16::from_f32(*v).to_f32())
            .collect(),
    }
}

#[test]
fn gpu_matches_cpu() {
    let Some(d) = testgpu::dev() else { return };
    let _g = testgpu::lock();
    let (w, h) = (500, 300);
    let main = crate::scopes::tests::picture(w, h, 1.0);
    // The scene: the picture ×8 (bright parts over the white limit); display Rec.2020: a
    // picture with some negative channels.
    let scene: Vec<f32> = main.iter().map(|v| v * 8.0).collect();
    let disp: Vec<f32> = main
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1] * 1.1, p[2] - 0.2 * p[0], 1.0])
        .collect();
    let mut pass = ClipPass::new(&d.device, &d.queue);
    let all = ClipFlags {
        scene: true,
        display: true,
        gamut: true,
    };
    for prec in [Precision::F32, Precision::F16] {
        for (f, proof) in [
            (all, false),
            (all, true),
            (
                ClipFlags {
                    gamut: true,
                    ..Default::default()
                },
                false,
            ),
        ] {
            let p = params(proof);
            let (bm, bs, bd) = (
                up(&d.device, &main, prec),
                up(&d.device, &scene, prec),
                up(&d.device, &disp, prec),
            );
            let rows = pass.run(
                (&bm, prec),
                Some((&bs, prec)),
                Some((&bd, prec)),
                w,
                h,
                &p,
                f,
                None,
            );
            let got = read(&d.device, &d.queue, &rows, w, h);
            let want = cpu(
                &rounded(&main, prec),
                Some(&rounded(&scene, prec)),
                Some(&rounded(&disp, prec)),
                &p,
                f,
            );
            let lit = want.chunks_exact(4).filter(|p| p[3] != 0).count();
            let differ = got
                .chunks_exact(4)
                .zip(want.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            eprintln!("clipping {prec:?} {f:?} proof {proof}: {lit} lit, {differ} differ");
            assert!(lit > 1000);
            assert!(differ * 1000 <= (w * h) as usize, "{differ} differ");
        }
    }
}

fn read(d: &wgpu::Device, q: &wgpu::Queue, rows: &wgpu::Buffer, w: u32, h: u32) -> Vec<u8> {
    let stride = w.next_multiple_of(64) as usize * 4;
    let n = (stride * h as usize) as u64;
    let rb = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("clip read"),
        size: n,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = d.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(rows, 0, &rb, 0, n);
    q.submit([enc.finish()]);
    rb.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    d.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let v = rb.slice(..).get_mapped_range().expect("mapped").to_vec();
    rb.unmap();
    v.chunks_exact(stride)
        .flat_map(|r| r[..w as usize * 4].to_vec())
        .collect()
}
