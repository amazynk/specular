//! The scopes, soft-proof and clipping passes on the preview.

use super::*;
use crate::clip::ClipFlags;
use crate::model::{Registry, SourceKey};
use crate::proof::{ProofSettings, Proofer, synthetic};
use crate::testgpu::{dev, lock};

fn with_exposure(e: &Edit, ev: f32) -> Edit {
    let mut e = e.clone();
    for m in &mut e.modules {
        if let Params::Exposure(p) = &mut m.params {
            p.exposure_ev = ev;
            m.enabled = true;
        }
    }
    e
}

fn overlay_texture(d: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
    d.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

#[test]
fn edit_changes_build_no_pipelines_with_every_aid_on() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (w, h) = (480u32, 320u32);
    let px = crate::scopes::tests::picture(w, h, 4.0);
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba: px,
    })
    .unwrap();
    let base = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    dv.prepare(&base, 1000).unwrap();
    let lut = Proofer::new(&synthetic::icc(), ProofSettings::default())
        .unwrap()
        .bake();
    let tex = overlay_texture(&d.device, w, h);
    let all = ClipFlags {
        scene: true,
        display: true,
        gamut: true,
    };
    let out = OutputSpace::rec709();
    let round = |dv: &mut Developer, ev: f32, proof: bool| {
        let e = with_exposure(&base, ev);
        let r = dv.render(&e, None, &out).unwrap();
        if proof {
            dv.proof(&r, &lut);
        }
        dv.scopes(&r, 1.0);
        dv.clipping(&e, &r, all, &out, proof, &tex).unwrap();
        dv.recycle(r);
        dv.wait().unwrap();
        let b = dv.wait_scopes().expect("scopes read back");
        assert_eq!(b.samples, w * h);
        b
    };
    // The first round of each kind builds what it needs.
    round(&mut dv, 0.0, false);
    round(&mut dv, 0.0, true);
    let before = dv.pipelines();
    let mut hist = Vec::new();
    for (i, ev) in [0.5f32, -1.0, 1.5, 0.25].into_iter().enumerate() {
        let b = round(&mut dv, ev, i % 2 == 0);
        hist.push(b.hist(3).to_vec());
    }
    assert_eq!(dv.pipelines(), before, "an edit change built pipelines");
    // The scopes follow the edit: brighter exposure, more weight up high.
    let mean = |h: &[u32]| {
        let n: u64 = h.iter().map(|&v| u64::from(v)).sum();
        h.iter()
            .enumerate()
            .map(|(i, &v)| i as u64 * u64::from(v))
            .sum::<u64>() as f64
            / n as f64
    };
    assert!(mean(&hist[2]) > mean(&hist[1]) + 10.0);
}
