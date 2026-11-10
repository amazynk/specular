//! Host-side checks of the corrections: the chromaticity grid, rebuild areas, the CA fit
//! on synthetic shifted channels, calibration frames and matching, and defringe's CPU twin.

use super::calib::{self, FrameInfo, FrameKind};
use super::highlights::{self, BLOCK_WORDS, BlockStats, TILE_WORDS, TileStats};
use super::*;

#[test]
fn chroma_grid_fills_from_borders() {
    // 4x4 tiles; one tile has red border colour, the rest none.
    let mut words = vec![0f32; 16 * TILE_WORDS];
    let t = 5 * TILE_WORDS;
    words[t..t + TILE_WORDS].copy_from_slice(&[6.0, 3.0, 1.0, 10.0, 40.0, 3.0]);
    let g = highlights::chroma_grid(&TileStats { w: 4, h: 4, words });
    assert_eq!(g.len(), 16);
    for c in &g {
        let s: f32 = c.iter().sum();
        assert!((s - 1.0).abs() < 1e-4, "{c:?}");
        // Every tile takes the only border colour there is.
        assert!(
            (c[0] - 0.6).abs() < 1e-3 && (c[2] - 0.1).abs() < 1e-3,
            "{c:?}"
        );
    }
    // No border at all: neutral.
    let g = highlights::chroma_grid(&TileStats {
        w: 3,
        h: 2,
        words: vec![0.0; 6 * TILE_WORDS],
    });
    assert!(
        g.iter()
            .all(|c| c.iter().all(|v| (v - 1.0 / 3.0).abs() < 1e-6))
    );
}

#[test]
fn rebuild_finds_areas_and_domes() {
    // 20x20 blocks: a blown disc of radius 6 in the middle, brightening towards it.
    let (w, h) = (20u32, 20u32);
    let mut words = vec![0f32; (w * h) as usize * BLOCK_WORDS];
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 - 9.5).powi(2) + (y as f32 - 9.5).powi(2)).sqrt();
            let i = (y * w + x) as usize * BLOCK_WORDS;
            if d < 6.0 {
                words[i..i + 6].copy_from_slice(&[16.0, 0.0, 0.0, 0.0, 0.0, 16.0]);
            } else {
                // Unclipped, colour (0.5, 0.3, 0.2), brighter nearer the disc.
                let l = 3.0 / (1.0 + 0.3 * (d - 6.0));
                words[i..i + 6].copy_from_slice(&[
                    0.0,
                    16.0,
                    16.0 * 0.5 * l,
                    16.0 * 0.3 * l,
                    16.0 * 0.2 * l,
                    16.0,
                ]);
            }
        }
    }
    let grid = vec![[1.0 / 3.0; 3]; 4];
    let r = highlights::rebuild(&BlockStats { w, h, words }, &grid, 2, 2);
    assert_eq!(r.chroma.len(), 1);
    let c = r.chroma[0];
    assert!(
        (c[0] - 0.5).abs() < 1e-3 && (c[1] - 0.3).abs() < 1e-3,
        "{c:?}"
    );
    let centre = r.boost[(10 * w + 10) as usize];
    let edge = r.boost[(10 * w + 4) as usize];
    assert!(centre > edge && centre > 0.1, "dome {edge} → {centre}");
    assert!(r.boost.iter().all(|b| (0.0..=3.0).contains(b)));
    // Small areas are left to opposed.
    let mut words = vec![0f32; 100 * BLOCK_WORDS];
    words[0..6].copy_from_slice(&[16.0, 0.0, 0.0, 0.0, 0.0, 16.0]);
    let r = highlights::rebuild(
        &BlockStats {
            w: 10,
            h: 10,
            words,
        },
        &grid,
        2,
        2,
    );
    assert!(r.chroma.is_empty() && r.area.iter().all(|&a| a == 0));
}

#[test]
fn frames_match_by_camera_lens_aperture_iso() {
    let target = FrameInfo {
        camera: "Sony ILCE-7M3".into(),
        lens: "FE 24-70mm F2.8 GM".into(),
        f_number: Some(8.0),
        iso: Some(400.0),
        exposure: Some(1.0 / 60.0),
        width: 6048,
        height: 4024,
        ..FrameInfo::default()
    };
    let f = |path: &str, lens: &str, n: f32, iso: f32| FrameInfo {
        path: path.into(),
        lens: lens.into(),
        f_number: Some(n),
        iso: Some(iso),
        ..target.clone()
    };
    let cands = vec![
        f("/c/flats/a.arw", "FE 24-70mm F2.8 GM", 2.8, 100.0),
        f("/c/flats/b.arw", "FE 24-70mm F2.8 GM", 11.0, 100.0),
        f("/c/flats/c.arw", "FE 85mm F1.8", 8.0, 100.0),
        f("/c/darks/d.arw", "", 8.0, 3200.0),
        f("/c/darks/e.arw", "", 8.0, 400.0),
        FrameInfo {
            camera: "Nikon Z 6".into(),
            ..f("/c/darks/z.nef", "", 8.0, 400.0)
        },
    ];
    assert_eq!(calib::best_match(&target, FrameKind::Flat, &cands), Some(1));
    assert_eq!(calib::best_match(&target, FrameKind::Dark, &cands), Some(4));
    let other = FrameInfo {
        camera: "Canon EOS R5".into(),
        ..target
    };
    assert_eq!(calib::best_match(&other, FrameKind::Dark, &cands), None);
    assert_eq!(
        calib::kind_of(std::path::Path::new("/x/Flat_01.dng")),
        Some(FrameKind::Flat)
    );
    assert_eq!(calib::kind_of(std::path::Path::new("/x/img.dng")), None);
}

#[test]
fn defringe_cpu_twin_only_touches_fringes_at_edges() {
    let d = defringe::Defringe {
        amount: 1.0,
        ..defringe::Defringe::default()
    };
    // A purple pixel beside a strong edge loses its chroma, keeps its lightness.
    let purple = [0.30, 0.05, 0.45];
    let out = defringe::cpu_pixel(&d, purple, 0.02, 1.0);
    let lab0 = crate::colour::oklab::from_rec2020_f32(purple);
    let lab1 = crate::colour::oklab::from_rec2020_f32(out);
    assert!((lab0[0] - lab1[0]).abs() < 1e-4);
    let hue = lab0[2].atan2(lab0[1]).to_degrees();
    assert!(
        lab1[1].hypot(lab1[2]) < 0.05 * lab0[1].hypot(lab0[2]),
        "hue {hue}: {lab0:?} → {lab1:?}"
    );
    // The same pixel away from edges, and a red pixel at the edge, stay.
    assert_eq!(defringe::cpu_pixel(&d, purple, 0.3, 0.4), purple);
    let red = [0.6, 0.1, 0.05];
    assert_eq!(defringe::cpu_pixel(&d, red, 0.02, 1.0), red);
}

#[test]
fn corrections_from_an_edit() {
    use crate::model::{Edit, ModuleInstance, Registry, SourceKey};
    let reg = Registry::builtin();
    let mut e = Edit::default_for(reg, SourceKey::default(), true);
    let c = Corrections::of_edit(&e);
    assert_eq!(c.highlights, HighlightMode::Opposed);
    assert_eq!(c.hot, None);
    for k in [
        ModuleKind::HOT_PIXELS,
        ModuleKind::CHROMATIC_ABERRATION,
        ModuleKind::FLAT_FIELD,
    ] {
        e.insert(reg, ModuleInstance::new(reg, k, 0));
    }
    let c = Corrections::of_edit(&e);
    assert_eq!(c.hot, Some(HOT_DEFAULT));
    assert_eq!(c.ca, CaMode::Auto);
    assert!(c.calibrate && c.any());
}

// ---- GPU: defringe, RGB chromatic aberration ------------------------------------------------

mod gpu {
    use super::super::ca;
    use super::super::defringe::{self, Defringe};
    use crate::gpu::pool::Budget;
    use crate::gpu::{Engine, Request};
    use crate::graph::{Graph, OpRegistry, Precision, Roi};
    use crate::model::{Edit, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
    use crate::ops::geometry::{Context, Plan};
    use crate::preview::Orient;
    use crate::testgpu::{dev, lock};

    /// A dark branch against a bright sky with purple and green fringes beside it.
    fn fringed(w: u32, h: u32) -> Vec<f32> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let e = x as f32 - (w as f32 / 2.0 + 0.2 * y as f32);
                let mut c = if e < 0.0 {
                    [0.03, 0.03, 0.035]
                } else {
                    [0.9, 0.95, 1.0]
                };
                if (-3.0..0.0).contains(&e) {
                    c = [0.12, 0.02, 0.2];
                }
                if (0.0..2.0).contains(&e) && y > h / 2 {
                    c = [0.3, 0.7, 0.25];
                }
                // A purple flower far from any edge.
                if (x as f32 - 20.0).hypot(y as f32 - 20.0) < 6.0 {
                    c = [0.18, 0.08, 0.22];
                }
                v.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
            }
        }
        v
    }

    fn lum(c: &[f32]) -> f32 {
        (0.2627 * c[0] + 0.6780 * c[1] + 0.0593 * c[2]).max(1e-3)
    }

    #[test]
    fn defringe_gpu_matches_its_cpu_twin() {
        let Some(d) = dev() else { return };
        let _l = lock();
        let (w, h) = (96u32, 64u32);
        let img = fringed(w, h);
        let p = Defringe {
            amount: 1.0,
            ..Defringe::default()
        };
        let reg = Registry::builtin();
        let mut m = ModuleInstance::new(reg, ModuleKind::DEFRINGE, 0);
        m.params = Params::from_fields(
            ModuleKind::DEFRINGE,
            Defringe::default_fields().with("amount", Value::Float(1.0)),
        );
        let mut e = Edit::empty(SourceKey::default());
        e.insert(reg, m);
        let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(1 << 30));
        let src = en.upload(w, h, &img).unwrap();
        let g = Graph::build(&e, OpRegistry::builtin(), None).unwrap();
        let req = Request {
            precision: Precision::F32,
            checkpoints: false,
            scale: 1.0,
            ..Request::new(Roi::full(w, h))
        };
        let r = en.render(&g, &src, &req).unwrap();
        let gpu = en.read(&r).unwrap();
        let rad = defringe::radius(1.0) as i32;
        let (mut worst, mut purple_left, mut green_left, mut flower) = (0f32, 0f32, 0f32, 0f32);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for dy in -rad..=rad {
                    for dx in -rad..=rad {
                        let (qx, qy) = (
                            (x + dx).clamp(0, w as i32 - 1),
                            (y + dy).clamp(0, h as i32 - 1),
                        );
                        let i = ((qy as u32 * w + qx as u32) * 4) as usize;
                        let l = lum(&img[i..i + 3]);
                        lo = lo.min(l);
                        hi = hi.max(l);
                    }
                }
                let i = ((y as u32 * w + x as u32) * 4) as usize;
                let c = [img[i], img[i + 1], img[i + 2]];
                let cpu = defringe::cpu_pixel(&p, c, lo, hi);
                for k in 0..3 {
                    worst = worst.max((cpu[k] - gpu[i + k]).abs());
                }
                let ch = |c: [f32; 3]| {
                    let l = crate::colour::oklab::from_rec2020_f32(c);
                    l[1].hypot(l[2])
                };
                let out = [gpu[i], gpu[i + 1], gpu[i + 2]];
                if c == [0.12, 0.02, 0.2] {
                    purple_left = purple_left.max(ch(out) / ch(c));
                }
                if c == [0.3, 0.7, 0.25] {
                    green_left = green_left.max(ch(out) / ch(c));
                }
                if c == [0.18, 0.08, 0.22] {
                    flower = flower.max((ch(out) / ch(c) - 1.0).abs());
                }
            }
        }
        eprintln!(
            "defringe: GPU vs CPU {worst:.2e}; fringe chroma left: purple {purple_left:.3}, green {green_left:.3}; flower changed {flower:.2e}"
        );
        assert!(worst < 1e-4);
        assert!(purple_left < 0.05 && green_left < 0.05);
        assert!(flower < 1e-6);
    }

    /// R and B magnified about the centre; the estimate and the warp's fringe scales.
    #[test]
    fn rgb_ca_estimate_feeds_the_warp() {
        let (w, h) = (1800u32, 1200u32);
        let (kr, kb) = (0.0012f32, -0.0009f32);
        let (cx, cy) = ((w as f32 - 1.0) / 2.0, (h as f32 - 1.0) / 2.0);
        let chart = |x: f32, y: f32| {
            let (u, v) = (x / 19.0, y / 19.0);
            let (fu, fv) = (u - u.floor() - 0.5, v - v.floor() - 0.5);
            let r = (fu * fu + fv * fv).sqrt();
            0.1 + 0.6 / (1.0 + ((r - 0.3) * 19.0 / 0.7).exp())
        };
        let mut img = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                let at = |k: f32| chart(cx + dx / (1.0 + k), cy + dy / (1.0 + k));
                img.extend_from_slice(&[at(kr), at(0.0), at(kb), 1.0]);
            }
        }
        let k = ca::estimate_rgb(w, h, &img).expect("an estimate");
        let radius = ((w * w + h * h) as f32).sqrt() / 2.0;
        eprintln!(
            "RGB CA: estimate {k:?} (truth [{kr}, {kb}]); corner error {:.3} / {:.3} px",
            (k[0] - kr) * radius,
            (k[1] - kb) * radius
        );
        assert!(((k[0] - kr) * radius).abs() < 0.1 && ((k[1] - kb) * radius).abs() < 0.1);
        // The module's scales reach the warp as fringe percentages.
        let mut ctx = Context::image(w, h, Orient::default());
        ctx.rgb_ca = Some(k.map(|v| v * 100.0));
        let e = Edit::empty(SourceKey::default());
        let plan = Plan::new(&e, &ctx, true);
        // Red against green (the fill zoom scales both).
        let f = plan.radial.factor(0, 0.5) / plan.radial.factor(1, 0.5);
        assert!((f - (1.0 + f64::from(k[0]))).abs() < 1e-6, "{f}");
        assert!(plan.radial.per_plane());
    }
}

#[test]
fn dark_frames_scale_by_exposure_time_at_the_same_iso() {
    let at = |iso: f32, t: f32| FrameInfo {
        iso: Some(iso),
        exposure: Some(t),
        ..FrameInfo::default()
    };
    assert_eq!(calib::dark_scale(&at(400.0, 2.0), &at(400.0, 1.0)), 2.0);
    assert_eq!(calib::dark_scale(&at(400.0, 0.5), &at(400.0, 1.0)), 0.5);
    // A third of a stop apart is the same ISO; a stop is not.
    assert_eq!(calib::dark_scale(&at(500.0, 2.0), &at(400.0, 1.0)), 2.0);
    assert_eq!(calib::dark_scale(&at(800.0, 2.0), &at(400.0, 1.0)), 1.0);
    // Unknown times: as it is; far apart: kept within 64.
    assert_eq!(
        calib::dark_scale(&FrameInfo::default(), &at(400.0, 1.0)),
        1.0
    );
    assert_eq!(calib::dark_scale(&at(400.0, 600.0), &at(400.0, 1.0)), 64.0);
}
