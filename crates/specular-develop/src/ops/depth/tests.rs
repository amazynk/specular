//! The depth tools: GPU = CPU reference for relight and the depth blur, both pass
//! through without a depth map, the render changes when the map arrives, tiles = whole,
//! and the blur's own properties (in-focus pixels stay, a sharp subject keeps its edge
//! against the blurred background, a light becomes a disc of the circle of confusion).

use super::blur::{self, DepthBlur};
use super::relight::Relight;
use super::*;
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request};
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::mask::brush::Raster;
use crate::mask::depth::{self as dm, DepthMap};
use crate::mask::{Geometry, MaskCache, MaskEnv};
use crate::model::{Edit, Fields, ModuleInstance, Registry, SourceKey, Value};
use crate::testgpu::{dev, lock};

const GIB: u64 = 1 << 30;

fn engine() -> Option<Engine> {
    let d = dev()?;
    Some(Engine::new(&d.device, &d.queue, Budget::fixed(2 * GIB)))
}

/// An edit with one module of `kind` with `f` over its defaults, for a photo of its own.
fn edit(kind: ModuleKind, f: Fields, photo: u8) -> Edit {
    let mut m = ModuleInstance::new(Registry::builtin(), kind, 0);
    let mut all = m.params.to_fields();
    all.overlay(&f);
    m.params = Params::from_fields(kind, all);
    let mut e = Edit::empty(SourceKey {
        size: 1000 + u64::from(photo),
        mtime_ns: 0,
        head_tail: [photo; 32],
    });
    e.insert(Registry::builtin(), m);
    e
}

fn fl(pairs: &[(&str, f32)]) -> Fields {
    let mut f = Fields::new();
    for (k, v) in pairs {
        f.set(k, Value::Float(*v));
    }
    f
}

/// Depth: a near disc (0.1) in the middle-left, the rest a far plane (0.9), a ramp along
/// the bottom fifth from near to far.
fn depth_of(u: f32, v: f32) -> f32 {
    if v > 0.8 {
        return u;
    }
    let (dx, dy) = (u - 0.35, v - 0.45);
    if dx * dx + dy * dy < 0.03 { 0.1 } else { 0.9 }
}

fn depth_raster(w: u32, h: u32) -> Raster {
    let tex: Vec<u16> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let d = depth_of((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            (d * 65535.0).round() as u16
        })
        .collect();
    crate::mask::ai::raster(w, h, &tex).unwrap()
}

/// A textured scene: a checker background, a smooth subject, two bright lights far away.
fn scene(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            let mut c = if ((x / 6) + (y / 6)) % 2 == 0 {
                [0.30, 0.25, 0.2]
            } else {
                [0.05, 0.08, 0.1]
            };
            let (dx, dy) = (u - 0.35, t - 0.45);
            if dx * dx + dy * dy < 0.03 {
                c = [0.5, 0.3 + 0.2 * u, 0.2];
            }
            for (lx, ly) in [(0.8, 0.25), (0.65, 0.7)] {
                if (u - lx).powi(2) + (t - ly).powi(2) < 0.00004 {
                    c = [8.0, 7.0, 5.0];
                }
            }
            v.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    }
    v
}

/// The graph for `e` on a `w × h` frame, its depth map stored or not.
fn graph(e: &Edit, w: u32, h: u32, focus: Option<(ModuleKind, u16)>) -> Graph {
    let mut cache = MaskCache::default();
    let mut env = MaskEnv {
        geometry: Geometry::Frame {
            w: f64::from(w),
            h: f64::from(h),
        },
        image: (w, h),
        scale: 1.0,
        cache: &mut cache,
        sensor_bin: 1.0,
        overlay: None,
    };
    Graph::build_masked(e, OpRegistry::builtin(), focus, None, Some(&mut env)).expect("graph")
}

fn store(e: &Edit, r: Option<Raster>) {
    let k = dm::key(&dm::photo(&e.source), &dm::models(e)[0]);
    let mut st = crate::mask::ai::store().lock().unwrap();
    match r {
        Some(r) => {
            st.put(k, r);
        }
        None => st.remove(&k),
    }
}

fn render(en: &mut Engine, g: &Graph, img: &[f32], w: u32, h: u32, tile: Option<Roi>) -> Vec<f32> {
    let src = en.upload(w, h, img).expect("upload");
    let roi = tile.unwrap_or(Roi::full(w, h));
    let req = Request {
        precision: Precision::F32,
        checkpoints: false,
        scale: 1.0,
        ..Request::new(roi)
    };
    let r = en.render(g, &src, &req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

/// The picture's depth table as the graph's node holds it.
fn map_of(g: &Graph, kind: ModuleKind) -> DepthMap {
    let n = g.nodes.iter().find(|n| n.id.0 == kind).expect("node");
    let d = n.data.as_ref().expect("data");
    let (w, h) = (d.words[0] as u32, d.words[1] as u32);
    DepthMap {
        w,
        h,
        data: d.words[2..2 + (w * h) as usize].to_vec(),
    }
}

fn params_of(g: &Graph, kind: ModuleKind) -> Fields {
    g.nodes
        .iter()
        .find(|n| n.id.0 == kind)
        .expect("node")
        .params
        .to_fields()
}

/// Worst |a − b| and the count over `tol`.
fn diff(a: &[f32], b: &[f32], tol: f32) -> (f32, usize) {
    let mut worst = 0f32;
    let mut over = 0;
    for (p, q) in a.iter().zip(b) {
        let d = (p - q).abs() / q.abs().max(0.05);
        worst = worst.max(d);
        over += usize::from(d > tol);
    }
    (worst, over)
}

#[test]
fn relight_gpu_matches_cpu_and_follows_depth() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (240u32, 160u32);
    let img = scene(w, h);
    let e = edit(
        ModuleKind::RELIGHT,
        fl(&[
            ("near", 1.0),
            ("far", -1.5),
            ("split", 0.5),
            ("transition", 0.2),
        ]),
        71,
    );
    store(&e, Some(depth_raster(w, h)));
    let g = graph(&e, w, h, None);
    let map = map_of(&g, ModuleKind::RELIGHT);
    assert_eq!((map.w, map.h), (w, h));
    let u = Relight::from_fields(&params_of(&g, ModuleKind::RELIGHT)).uniforms();
    let want = cpu::relight(&img, w, h, &map, u);
    let got = render(&mut en, &g, &img, w, h, None);
    let (worst, over) = diff(&got, &want, 1e-5);
    eprintln!("relight gpu vs cpu: worst {worst:.2e}, {over} over 1e-5");
    assert_eq!(over, 0);
    // The near subject is 1 EV brighter, the far plane 1.5 EV darker.
    let px = |x: u32, y: u32| 4 * (y * w + x) as usize;
    let (sx, sy) = ((0.35 * w as f32) as u32, (0.45 * h as f32) as u32);
    assert!((got[px(sx, sy)] / img[px(sx, sy)] - 2.0).abs() < 1e-3);
    assert!((got[px(5, 5)] / img[px(5, 5)] - 2f32.powf(-1.5)).abs() < 1e-3);
    store(&e, None);
}

#[test]
fn without_a_depth_map_nothing_changes_and_its_arrival_renders_anew() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (96u32, 64u32);
    let img = scene(w, h);
    for (kind, f) in [
        (ModuleKind::RELIGHT, fl(&[("near", 1.0), ("far", -1.0)])),
        (ModuleKind::DEPTH_BLUR, fl(&[("amount", 0.03)])),
    ] {
        let e = edit(kind, f, 72);
        store(&e, None);
        let g0 = graph(&e, w, h, None);
        assert_eq!(map_of(&g0, kind).w, 0);
        let out = render(&mut en, &g0, &img, w, h, None);
        assert_eq!(out, img, "{kind:?} without a map");
        store(&e, Some(depth_raster(w, h)));
        let g1 = graph(&e, w, h, None);
        // The table's hash salts the node's cache key.
        let n0 = g0.nodes.iter().find(|n| n.id.0 == kind).unwrap();
        let n1 = g1.nodes.iter().find(|n| n.id.0 == kind).unwrap();
        assert_ne!(n0.prefix, n1.prefix, "{kind:?}");
        assert_ne!(render(&mut en, &g1, &img, w, h, None), img);
        store(&e, None);
    }
}

#[test]
fn depth_blur_gpu_matches_cpu_and_tiles_are_whole() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (256u32, 160u32);
    let img = scene(w, h);
    let e = edit(
        ModuleKind::DEPTH_BLUR,
        fl(&[
            ("amount", 0.04),
            ("focus", 0.1),
            ("range", 0.1),
            ("highlights", 0.5),
        ]),
        73,
    );
    store(&e, Some(depth_raster(w, h)));
    let g = graph(&e, w, h, None);
    let f = params_of(&g, ModuleKind::DEPTH_BLUR);
    let u = blur::uniforms(&f);
    assert_eq!(u[5].to_bits(), blur::TAPS);
    assert!((u[0] - 0.04 * 256.0).abs() < 1e-4);
    let map = map_of(&g, ModuleKind::DEPTH_BLUR);
    let want = cpu::depth_blur(&img, w, h, &map, u);
    let got = render(&mut en, &g, &img, w, h, None);
    // The same taps (from the table); sums in f32 in the same order.
    let (worst, over) = diff(&got, &want, 1e-4);
    eprintln!(
        "depth blur gpu vs cpu: worst {worst:.2e}, {over} of {} over 1e-4",
        got.len()
    );
    assert_eq!(over, 0, "worst {worst}");
    // A tile renders the same as the whole picture.
    let t = Roi::new(100, 40, 90, 70);
    let tile = render(&mut en, &g, &img, w, h, Some(t));
    for y in 0..t.h {
        for x in 0..t.w {
            let a = 4 * (y * t.w + x) as usize;
            let b = 4 * ((t.y + y) * w + t.x + x) as usize;
            assert_eq!(&tile[a..a + 4], &got[b..b + 4], "tile at {x},{y}");
        }
    }
    store(&e, None);
}

#[test]
fn depth_blur_keeps_the_focus_sharp_and_blurs_the_rest() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (256u32, 160u32);
    let img = scene(w, h);
    let e = edit(
        ModuleKind::DEPTH_BLUR,
        fl(&[("amount", 0.03), ("focus", 0.1), ("range", 0.1)]),
        74,
    );
    store(&e, Some(depth_raster(w, h)));
    let g = graph(&e, w, h, None);
    let got = render(&mut en, &g, &img, w, h, None);
    let px = |x: u32, y: u32| 4 * (y * w + x) as usize;
    // Inside the in-focus subject, away from its edge: unchanged.
    let (sx, sy) = ((0.35 * w as f32) as u32, (0.45 * h as f32) as u32);
    for (dx, dy) in [(0i32, 0i32), (10, 0), (0, 8), (-12, -5)] {
        let i = px((sx as i32 + dx) as u32, (sy as i32 + dy) as u32);
        assert_eq!(&got[i..i + 3], &img[i..i + 3]);
    }
    // The far checker loses most of its contrast.
    let var = |v: &[f32]| {
        let xs: Vec<f32> = (150..240)
            .flat_map(|x| (5..30).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let (u, t) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                (u - 0.8).powi(2) + (t - 0.25).powi(2) > 0.01
            })
            .map(|(x, y)| v[px(x, y)])
            .collect();
        let m = xs.iter().sum::<f32>() / xs.len() as f32;
        xs.iter().map(|x| (x - m).powi(2)).sum::<f32>() / xs.len() as f32
    };
    let (v0, v1) = (var(&img), var(&got));
    eprintln!("far checker variance {v0:.4} -> {v1:.4}");
    assert!(v1 < 0.1 * v0);
    // The far light spreads into a disc about the circle of confusion: R·(0.8 − 0.05)/0.5
    // capped at R, R = 0.03·256 ≈ 7.7 px.
    let (lx, ly) = ((0.8 * w as f32) as u32, (0.25 * h as f32) as u32);
    let lit = |r: u32| got[px(lx + r, ly)];
    assert!(lit(5) > 0.6, "{}", lit(5));
    assert!(lit(11) < 0.35, "{}", lit(11));
    store(&e, None);
}

#[test]
fn drafts_use_fewer_taps() {
    let mut f = DepthBlur::default_fields();
    f.set("amount", Value::Float(0.01));
    f.set(DRAFT, Value::Bool(true));
    assert_eq!(blur::uniforms(&f)[5].to_bits(), blur::DRAFT_TAPS);
    let p = Params::Other(ModuleKind::DEPTH_BLUR, Fields::new());
    // Never placed: an empty table, never a panic.
    assert_eq!(data_of(&p).words, vec![0.0, 0.0]);
    // Every tap lies within the disc.
    let t = blur::tap_table(7.5, 40);
    assert_eq!(t.len(), 2 * 16 * 40);
    assert!(t.chunks_exact(2).all(|o| o[0].hypot(o[1]) <= 8.0));
}
