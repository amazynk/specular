use super::*;
use crate::model::{Registry, SourceKey};
use crate::testgpu::{dev, lock};

#[test]
fn exif_orientations_turn_like_the_viewer() {
    // A 3×2 image; for each orientation, where the stored top-left pixel goes.
    let (w, h) = (3, 2);
    for (o, corner) in [
        (1, (0, 0)),
        (2, (2, 0)),
        (3, (2, 1)),
        (4, (0, 1)),
        (5, (0, 0)),
        (6, (1, 0)),
        (7, (1, 2)),
        (8, (0, 2)),
    ] {
        let t = Orient::from_exif(o);
        let (ow, oh) = t.size(w, h);
        let mut found = None;
        for y in 0..oh {
            for x in 0..ow {
                if t.source_of(x, y, w, h) == (0, 0) {
                    found = Some((x, y));
                }
            }
        }
        assert_eq!(found, Some(corner), "orientation {o}");
    }
}

#[test]
fn rgba8_input_is_linear_rec2020_and_reduced() {
    let rgba = [128u8, 128, 128, 255].repeat(400 * 200);
    let Input::Rgb {
        width,
        height,
        rgba: v,
    } = rgba8_to_input(400, 200, &rgba, 100)
    else {
        panic!("rgb")
    };
    assert_eq!((width, height), (100, 50));
    let lin = ((128.0 / 255.0 + 0.055) / 1.055f32).powf(2.4);
    for c in &v[..3] {
        assert!((c - lin).abs() < 1e-5, "{v:?}");
    }
    assert!((v[3] - 1.0).abs() < 1e-6);
    assert_eq!(fit(4000, 3000, 1000), (1000, 750));
    assert_eq!(fit(400, 300, 1000), (400, 300));
}

#[test]
fn grey_develops_to_grey_and_turns() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    // Mid grey everywhere but one bright red pixel at the stored top-left.
    let (w, h) = (6u32, 4u32);
    let mut px = [0.1845f32, 0.1845, 0.1845, 1.0].repeat((w * h) as usize);
    px[..4].copy_from_slice(&[4.0, 0.05, 0.05, 1.0]);
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba: px,
    })
    .unwrap();
    let e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    assert_eq!(dv.prepare(&e, 1000).unwrap(), (6, 4));
    for o in [1u16, 6] {
        let t = Orient::from_exif(o);
        let r = dv.render(&e, None, &OutputSpace::rec709()).unwrap();
        let (ow, oh, bytes) = dv.read_rgba8(r, t).unwrap();
        assert_eq!((ow, oh), t.size(w, h));
        // sRGB of 0.1845 is about 0.471 → 120.
        let at = |x: u32, y: u32| &bytes[((y * ow + x) * 4) as usize..][..4];
        let (gx, gy) = (ow - 1, oh - 1);
        let g = at(gx, gy);
        assert!(g[..3].iter().all(|&c| (119..=121).contains(&c)), "{g:?}");
        let (rx, ry) = (0..oh)
            .flat_map(|y| (0..ow).map(move |x| (x, y)))
            .find(|&(x, y)| t.source_of(x, y, w, h) == (0, 0))
            .unwrap();
        let red = at(rx, ry);
        assert!(red[0] > 200 && red[0] > red[1] + 60, "{red:?}");
        assert!(dv.idle() || dv.wait().is_ok());
    }
}

#[test]
fn present_writes_the_texture() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (w, h) = (70u32, 33u32);
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba: [0.1845f32, 0.1845, 0.1845, 1.0].repeat((w * h) as usize),
    })
    .unwrap();
    let e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    dv.prepare(&e, 2000).unwrap();
    let tex = d.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: h,
            height: w,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let r = dv.render(&e, None, &OutputSpace::rec709()).unwrap();
    dv.present(r, Orient::from_exif(6), &tex).unwrap();
    dv.wait().unwrap();
    assert!(dv.idle());
    // A wrongly sized texture is refused, not written.
    let r = dv.render(&e, None, &OutputSpace::rec709()).unwrap();
    assert!(dv.present(r, Orient::default(), &tex).is_err());
}

/// The overlay of a refined first masked module is its mask, whatever comes
/// after it (a later refine pass used to overwrite the alpha it travels in: all white).
#[test]
fn overlay_of_a_refined_first_module_is_its_mask() {
    use crate::model::ModuleInstance;
    use crate::model::mask::{Blend, MaskExpr, MaskShape, Refine, Shape};
    use crate::model::params::Exposure;
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (w, h) = (96u32, 64u32);
    // A dark left half and a bright right half, so edge refinement has an edge to follow.
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for _y in 0..h {
        for x in 0..w {
            let v = if x < w / 2 { 0.03 } else { 0.5 };
            px.extend_from_slice(&[v, v * 0.9, v * 0.8, 1.0]);
        }
    }
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba: px,
    })
    .unwrap();
    let refined = |shape: u32, invert: bool| Blend {
        mask: MaskExpr::Shape(shape),
        invert,
        refine: Refine {
            feather: 0.02,
            edge_aware: 0.8,
            ..Refine::default()
        },
        ..Blend::default()
    };
    let exposure = |ev: f32, instance: u16, blend: Blend| {
        let mut m = ModuleInstance::with_params(
            Exposure {
                exposure_ev: ev,
                ..Exposure::default()
            },
            instance,
        );
        m.blend = Some(blend);
        m
    };
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    e.masks = vec![
        MaskShape {
            id: 1,
            shape: Shape::Linear {
                start: [0.3, 0.5],
                end: [0.7, 0.5],
            },
        },
        MaskShape {
            id: 2,
            shape: Shape::Linear {
                start: [0.5, 0.2],
                end: [0.5, 0.8],
            },
        },
    ];
    let at = e
        .modules
        .iter()
        .position(|m| m.kind == ModuleKind::EXPOSURE)
        .map_or(e.modules.len(), |i| i + 1);
    let first = (ModuleKind::EXPOSURE, 7);
    e.modules.insert(at, exposure(0.7, 7, refined(1, false)));
    e.modules
        .insert(at + 1, exposure(-0.5, 8, refined(2, true)));
    // The same edit without the later masked module: the reference mask.
    let mut alone = e.clone();
    alone.modules.remove(at + 1);
    assert_eq!(dv.prepare(&e, 1000).unwrap(), (w, h));
    let out = OutputSpace::rec709();
    let render = |dv: &mut Developer, e: &Edit, o: Option<Overlay>| {
        dv.mask_overlay = o.map(|o| (first, o));
        let r = dv.render(e, None, &out).unwrap();
        let v = dv.read(&r).unwrap();
        dv.engine.recycle(r);
        v
    };
    let grey = render(&mut dv, &e, Some(Overlay::Grey));
    let reference = render(&mut dv, &alone, Some(Overlay::Grey));
    let n = (w * h) as usize;
    let mean = |v: &[f32]| (0..n).map(|i| f64::from(v[4 * i])).sum::<f64>() / n as f64;
    eprintln!(
        "grey overlay mean {:.4}, reference {:.4}",
        mean(&grey),
        mean(&reference)
    );
    assert!(grey.iter().zip(&reference).all(|(a, b)| a == b));
    // Not all white: the linear mask runs from 1 to 0 across the middle.
    assert!(mean(&grey) > 0.2 && mean(&grey) < 0.8);
    assert!(grey[0] > 0.99 && grey[4 * (w as usize - 1)] < 0.01);
    // The tint is the picture without the overlay, tinted by that mask.
    let plain = render(&mut dv, &e, None);
    let tint = render(&mut dv, &e, Some(Overlay::Tint));
    let mut worst = 0f32;
    for i in 0..n {
        let k = 0.5 * grey[4 * i].clamp(0.0, 1.0);
        for (c, red) in [1.0f32, 0.0, 0.0].iter().enumerate() {
            let want = plain[4 * i + c] + (red - plain[4 * i + c]) * k;
            worst = worst.max((tint[4 * i + c] - want).abs() / want.abs().max(1.0));
        }
    }
    eprintln!("tint vs picture tinted by the mask: worst {worst:.2e}");
    assert!(worst < 2e-3, "{worst}");
}

/// A raw developed ahead ([`Developer::prefetch`], [`Developer::take_prefetched`])
/// renders as the same raw opened with [`Developer::set_input`], bit for bit, without a
/// second front-end run; the photo shown meanwhile keeps its picture.
#[test]
fn a_raw_developed_ahead_renders_as_one_opened() {
    use crate::model::ModuleInstance;
    use crate::ops::detail::tests::bayer_sensor;
    let Some(d) = dev() else { return };
    let _l = lock();
    let shown_raw = bayer_sensor(640, 426, [8e-4, 4e-4, 6e-4], [4e-6, 2e-6, 3e-6], 3);
    let next_raw = bayer_sensor(640, 426, [3e-3, 1.6e-3, 2.4e-3], [4e-5, 2e-5, 3e-5], 4);
    let reg = Registry::builtin();
    let mut edit = Edit::default_for(reg, SourceKey::default(), true);
    edit.modules
        .push(ModuleInstance::new(reg, ModuleKind::DENOISE, 0));
    edit.sort(reg);
    let out = OutputSpace::rec709();
    let picture = |dv: &mut Developer| -> Vec<u32> {
        dv.prepare(&edit, 400).expect("prepare");
        let r = dv.render(&edit, None, &out).expect("render");
        let v = dv.read(&r).expect("read");
        dv.recycle(r);
        v.iter().map(|x| x.to_bits()).collect()
    };
    let new = || {
        let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
        dv.wait_highlight_stats = true;
        dv
    };
    let mut plain = new();
    plain
        .set_input(Input::Raw(Box::new(next_raw.clone())))
        .expect("input");
    let want = picture(&mut plain);

    let mut dv = new();
    dv.set_input(Input::Raw(Box::new(shown_raw)))
        .expect("input");
    let shown = picture(&mut dv);
    let noise = Some(RawNoise::of_sensor(&next_raw));
    let kept = dv
        .prefetch(
            7,
            Input::Raw(Box::new(next_raw)),
            noise,
            &edit,
            400,
            Orient::default(),
        )
        .expect("prefetch");
    assert!(kept);
    assert_eq!(dv.prefetched(), Some(7));
    assert!(picture(&mut dv) == shown, "the photo shown changed");
    assert!(!dv.take_prefetched(8), "another tag");
    assert!(dv.take_prefetched(7));
    assert_eq!(dv.prefetched(), None);
    let id = dv.source.as_ref().map(Source::id);
    let got = picture(&mut dv);
    assert_eq!(dv.source.as_ref().map(Source::id), id, "developed again");
    let same = got.iter().zip(&want).filter(|(a, b)| a == b).count();
    assert_eq!(
        same,
        want.len(),
        "{} of {} values differ",
        want.len() - same,
        want.len()
    );

    // Over the memory budget: nothing kept.
    let mut small = Developer::new(&d.device, &d.queue, Budget::fixed(8 << 20));
    let raw = bayer_sensor(640, 426, [8e-4, 4e-4, 6e-4], [4e-6, 2e-6, 3e-6], 5);
    let kept = small
        .prefetch(
            1,
            Input::Raw(Box::new(raw)),
            None,
            &edit,
            400,
            Orient::default(),
        )
        .expect("prefetch");
    assert!(!kept && small.prefetched().is_none());
}

/// A refined render keeps every group's output ([`Request::checkpoint_every`]), so a
/// change to a late module resumes right before it: the earlier modules don't run again,
/// and the picture is the one a developer without those checkpoints makes, bit for bit.
#[test]
fn refined_renders_resume_before_the_changed_module() {
    use crate::model::{Fields, ModuleInstance, Value};
    use crate::ops::detail::tests::bayer_sensor;
    let Some(d) = dev() else { return };
    let _l = lock();
    let raw = bayer_sensor(640, 426, [8e-4, 4e-4, 6e-4], [4e-6, 2e-6, 3e-6], 3);
    let reg = Registry::builtin();
    let mut edit = Edit::default_for(reg, SourceKey::default(), true);
    for k in [
        ModuleKind::DENOISE,
        ModuleKind::LOCAL_CONTRAST,
        ModuleKind::COLOUR_BALANCE,
    ] {
        edit.modules.push(ModuleInstance::new(reg, k, 0));
    }
    edit.sort(reg);
    let set = |e: &mut Edit, v: f32| {
        let m = e.module_mut(ModuleKind::COLOUR_BALANCE, 0).expect("module");
        let mut f = m.params.to_fields();
        f.overlay(&Fields::new().with("vibrance", Value::Float(v)));
        m.params.set_fields(f);
    };
    let out = OutputSpace::rec709();
    let new = |every: bool| {
        let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
        dv.wait_highlight_stats = true;
        dv.checkpoint_every = every;
        dv.keep_results = false;
        dv.set_input(Input::Raw(Box::new(raw.clone())))
            .expect("input");
        dv
    };
    let mut pictures = Vec::new();
    for every in [true, false] {
        let mut dv = new(every);
        let mut last = None;
        for v in [0.1, 0.3, 0.5] {
            set(&mut edit, v);
            dv.prepare(&edit, 400).expect("prepare");
            let r = dv.render(&edit, None, &out).expect("render");
            let ran = r.stats.nodes_run.clone();
            last = Some(dv.read(&r).expect("read"));
            dv.recycle(r);
            if every && v > 0.1 {
                assert!(
                    !ran.iter().any(|n| n.0 == ModuleKind::LOCAL_CONTRAST),
                    "ran again: {ran:?}"
                );
            }
        }
        pictures.push(last.expect("a picture"));
    }
    let same = pictures[0]
        .iter()
        .zip(&pictures[1])
        .filter(|(a, b)| a.to_bits() == b.to_bits())
        .count();
    assert_eq!(same, pictures[0].len(), "values differ");
}
