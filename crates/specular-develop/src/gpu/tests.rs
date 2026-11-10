//! GPU against the CPU reference, tiling, caching and pipeline reuse.
//!
//! The tests open their own device (with timestamp queries when the adapter has them).
//! Without an adapter they are skipped, unless SPECULAR_REQUIRE_GPU is set.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use super::pool::{Budget, size_class};
use super::*;
use crate::graph::testops::*;
use crate::graph::{Graph, OpRegistry};
use crate::model::{Edit, Value};

const GIB: u64 = 1 << 30;

struct Dev {
    device: wgpu::Device,
    queue: wgpu::Queue,
    name: String,
}

fn dev() -> Option<&'static Dev> {
    static D: OnceLock<Option<Dev>> = OnceLock::new();
    D.get_or_init(|| match open() {
        Ok(d) => Some(d),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU tests skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    })
    .as_ref()
}

/// Tests share one device; renders on it run one at a time (error scopes and timings).
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn open() -> Result<Dev, String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.flags = wgpu::InstanceFlags::empty().with_env();
    desc.backends = wgpu::Backends::PRIMARY.with_env();
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let info = adapter.get_info();
    let a = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(a.clone());
    limits.max_storage_buffer_binding_size = a.max_storage_buffer_binding_size;
    limits.max_buffer_size = a.max_buffer_size;
    let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("develop tests"),
        required_features: features,
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("no device: {e}"))?;
    let name = format!("{} ({:?})", info.name, info.backend);
    eprintln!("gpu: {name}, timestamps {}", !features.is_empty());
    Ok(Dev {
        device,
        queue,
        name,
    })
}

fn engine(d: &Dev, vram: u64) -> Engine {
    Engine::new(&d.device, &d.queue, Budget::fixed(vram))
}

fn f32_req(roi: Roi) -> Request {
    Request {
        precision: Precision::F32,
        ..Request::new(roi)
    }
}

fn run(
    en: &mut Engine,
    e: &Edit,
    focus: Option<(ModuleKind, u16)>,
    src: &Source,
    req: &Request,
) -> (Vec<f32>, RenderStats) {
    let g = Graph::build(e, &registry(), focus).expect("graph");
    let r = en.render(&g, src, req).expect("render");
    let px = en.read(&r).expect("read");
    let stats = r.stats.clone();
    en.recycle(r);
    (px, stats)
}

/// `roi` cut out of a `w`-wide RGBA image.
fn crop(img: &[f32], w: u32, roi: Roi) -> Vec<f32> {
    let mut v = Vec::with_capacity(roi.pixels() as usize * 4);
    for y in roi.y..roi.bottom() {
        let a = ((y * w + roi.x) * 4) as usize;
        v.extend_from_slice(&img[a..a + roi.w as usize * 4]);
    }
    v
}

#[test]
fn identity_graph_is_bit_exact() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let (w, h) = (37, 23);
    // Every bit pattern class: normals, ±0, ±inf, subnormals, NaN payloads.
    let mut img: Vec<f32> = (0..w * h * 4u32)
        .map(|i| f32::from_bits(i.wrapping_mul(0x9E37_79B9)))
        .collect();
    img[..8].copy_from_slice(&[
        0.0,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE / 4.0,
        -1e-40,
        f32::MAX,
        1.0,
    ]);
    let src = en.upload(w, h, &img).expect("upload");
    let bits = |v: &[f32]| v.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
    for modules in [vec![], vec![identity(0)], vec![identity(0), identity(1)]] {
        let e = edit(modules);
        let (out, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
        assert_eq!(st.dispatches, 1);
        assert!(bits(&out) == bits(&img), "identity changed bits");
        // A region too.
        let roi = Roi::new(5, 3, 20, 11);
        let (out, _) = run(&mut en, &e, None, &src, &f32_req(roi));
        assert!(bits(&out) == bits(&crop(&img, w, roi)));
    }
}

#[test]
fn fused_chain_matches_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let (w, h) = (301, 173);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let e = edit(vec![
        gain(0, 1.7),
        add(0, [0.25, -0.5, 1.0]),
        gain(1, 0.3),
        add(1, [-0.125, 0.0, 3.5]),
        gain(2, 2.9),
    ]);
    let (out, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    assert_eq!(st.dispatches, 1, "five point ops are one kernel");
    let want = reference(&e, w, h, &img);
    let rel = max_rel(&out, &want);
    eprintln!("5-op fused chain: max relative error {rel:e}");
    assert!(rel <= 1e-5, "max relative error {rel}");
    // With kernels in between, against the reference too.
    let e = edit(vec![
        gain(0, 1.7),
        box3(0),
        add(0, [0.25, -0.5, 1.0]),
        box3(1),
        gain(1, 0.3),
    ]);
    let (out, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    assert_eq!(st.dispatches, 5);
    let want = reference(&e, w, h, &img);
    let rel = max_rel(&out, &want);
    eprintln!("gain, blur, add, blur, gain: max relative error {rel:e}");
    assert!(rel <= 1e-5, "max relative error {rel}");
}

#[test]
fn f16_storage_stays_close_to_the_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let (w, h) = (128, 96);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let e = edit(vec![gain(0, 1.5), box3(0), add(0, [0.1, 0.2, 0.3])]);
    let (out, st) = run(&mut en, &e, None, &src, &Request::new(Roi::full(w, h)));
    assert_eq!(st.dispatches, 3);
    let want = reference(&e, w, h, &img);
    // Three f16 roundings (after the gain, after the blur, at the output), 2⁻¹¹ each.
    let rel = max_rel(&out, &want);
    eprintln!("f16 storage, 3 dispatches: max relative error {rel:e}");
    assert!(rel <= 4.0 * 2f32.powi(-11), "f16 relative error {rel}");
}

#[test]
fn tiled_equals_untiled_on_a_4096_square() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (4096, 4096);
    let img = synthetic(w, h);
    let e = edit(vec![
        gain(0, 1.3),
        box3(0),
        add(0, [0.5, -0.25, 0.125]),
        box3(1),
        gain(1, 0.7),
    ]);
    for prec in [Precision::F32, Precision::F16] {
        let mut en = engine(d, 4 * GIB);
        let src = en.upload(w, h, &img).expect("upload");
        let req = Request {
            precision: prec,
            tiling: Tiling::Off,
            checkpoints: false,
            ..Request::new(Roi::full(w, h))
        };
        let (whole, st) = run(&mut en, &e, None, &src, &req);
        assert_eq!(st.tiles, 1);
        // 1000 does not divide 4096: tiles of every shape, halos at every edge.
        let req = Request {
            tiling: Tiling::Side(1000),
            ..req
        };
        let (tiled, st) = run(&mut en, &e, None, &src, &req);
        assert_eq!(st.tiles, 25);
        let (diff, at) = max_abs(&whole, &tiled);
        eprintln!("{prec:?} 4096² tiled vs untiled: max |diff| {diff:e}");
        assert!(diff <= 1e-4, "{prec:?}: tiled differs by {diff} at {at}");
        if prec == Precision::F32 {
            let want = reference(&e, w, h, &img);
            let rel = max_rel(&whole, &want);
            assert!(rel <= 1e-5, "against the reference: {rel}");
        }
    }
}

#[test]
fn a_region_matches_the_same_pixels_of_the_whole() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let (w, h) = (200, 150);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let e = edit(vec![box3(0), gain(0, 2.0), box3(1), box3(2)]);
    let (whole, _) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    for roi in [
        Roi::new(0, 0, 17, 9),
        Roi::new(50, 40, 33, 61),
        Roi::new(183, 141, 17, 9),
    ] {
        let (part, _) = run(&mut en, &e, None, &src, &f32_req(roi));
        assert_eq!(part, crop(&whole, w, roi), "{roi:?}");
    }
}

#[test]
fn slider_update_creates_no_pipeline() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let (w, h) = (640, 400);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let mut e = edit(vec![gain(0, 1.0), box3(0), gain(1, 1.0), add(0, [0.0; 3])]);
    let focus = Some((GAIN, 1));
    let req = Request::new(Roi::full(w, h));
    run(&mut en, &e, focus, &src, &req);
    let warm = en.counters();
    for i in 1..=20 {
        let v = 1.0 + i as f32 * 0.05;
        set(&mut e, GAIN, 1, "gain", Value::Float(v));
        let (out, st) = run(&mut en, &e, focus, &src, &req);
        assert_eq!(st.pipelines_created, 0, "slider step {i} made a pipeline");
        assert_eq!(
            st.bind_groups_created, 0,
            "slider step {i} made a bind group"
        );
        assert_eq!(st.buffers_created, 0, "slider step {i} made a buffer");
        assert_eq!(st.cache_hits, 1);
        assert_eq!(st.nodes_run, vec![(GAIN, 1), (ADD, 0)]);
        assert_eq!(st.dispatches, 1);
        if i == 20 {
            let want = reference(&e, w, h, &img);
            // f16 storage: three roundings, as above.
            assert!(max_rel(&out, &want) <= 4.0 * 2f32.powi(-11));
        }
    }
    let c = en.counters();
    assert_eq!(c.pipelines, warm.pipelines);
    assert_eq!(c.renders, warm.renders + 20);
}

#[test]
fn downstream_change_reruns_only_downstream_nodes() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let (w, h) = (300, 200);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let mut e = edit(vec![
        gain(0, 1.5),
        box3(0),
        gain(1, 0.8),
        box3(1),
        add(0, [0.0; 3]),
    ]);
    let req = f32_req(Roi::full(w, h));
    let (_, st) = run(&mut en, &e, None, &src, &req);
    assert_eq!(st.cache_hits, 0);
    assert_eq!(st.nodes_run.len(), 5);
    // The add follows the second blur: only it runs.
    set(
        &mut e,
        ADD,
        0,
        "offset",
        Value::Floats(vec![0.5, 0.25, 0.0]),
    );
    let (out, st) = run(&mut en, &e, None, &src, &req);
    assert_eq!(st.cache_hits, 1);
    assert_eq!(st.nodes_run, vec![(ADD, 0)]);
    assert!(max_rel(&out, &reference(&e, w, h, &img)) <= 1e-5);
    // The gain between the blurs: from the first blur's checkpoint on.
    set(&mut e, GAIN, 1, "gain", Value::Float(0.6));
    let (out, st) = run(&mut en, &e, None, &src, &req);
    assert_eq!(st.nodes_run, vec![(GAIN, 1), (BOX3, 1), (ADD, 0)]);
    assert!(max_rel(&out, &reference(&e, w, h, &img)) <= 1e-5);
    // The first gain: everything.
    set(&mut e, GAIN, 0, "gain", Value::Float(1.1));
    let (_, st) = run(&mut en, &e, None, &src, &req);
    assert_eq!(st.nodes_run.len(), 5);
    assert_eq!(st.cache_hits, 0);
    // Another source with the same edit misses.
    let src2 = en.upload(w, h, &synthetic(w, h)).expect("upload");
    let (_, st) = run(&mut en, &e, None, &src2, &req);
    assert_eq!(st.cache_hits, 0);
    // Another scale or precision misses too.
    let (_, st) = run(&mut en, &e, None, &src2, &Request { scale: 0.5, ..req });
    assert_eq!(st.cache_hits, 0);
}

#[test]
fn cache_stays_within_its_budget() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (512, 512);
    // 4 MiB per f32 checkpoint; the cache half of 24 MiB holds three.
    let mut en = engine(d, 24 << 20);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let mut e = edit(vec![box3(0), gain(0, 1.0)]);
    for i in 0..6 {
        set(&mut e, GAIN, 0, "gain", Value::Float(i as f32));
        let (_, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
        assert_eq!(st.cache_hits, usize::from(i > 0));
        // A different region of the blur each time fills the cache.
        let roi = Roi::new(i * 10, 0, 300, 300);
        run(&mut en, &e, None, &src, &f32_req(roi));
        assert!(en.cached().1 <= en.budget().cache());
    }
    en.clear_cache();
    assert_eq!(en.cached(), (0, 0));
}

#[test]
fn bad_requests_are_refused() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let src = en.upload(8, 8, &synthetic(8, 8)).expect("upload");
    let g = Graph::build(&edit(vec![gain(0, 1.0)]), &registry(), None).expect("graph");
    for roi in [Roi::new(0, 0, 0, 4), Roi::new(4, 4, 5, 4)] {
        assert!(matches!(
            en.render(&g, &src, &Request::new(roi)),
            Err(Error::BadRequest(_))
        ));
    }
    assert!(en.upload(8, 8, &[0.0; 3]).is_err());
    // Too large for the budget even in the smallest tiles.
    let mut tiny = engine(d, 1 << 20);
    let src = tiny.upload(600, 600, &synthetic(600, 600)).expect("upload");
    let g = Graph::build(&edit(vec![box3(0), box3(1)]), &registry(), None).expect("graph");
    let r = tiny.render(&g, &src, &f32_req(Roi::full(600, 600)));
    assert!(matches!(r, Err(Error::TooLarge { .. })), "{r:?}");
    assert!(size_class(1) >= 16);
}

#[test]
fn global_statistics_are_taken_once_on_the_whole_image() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let (w, h) = (300, 200);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let mut e = edit(vec![gain(0, 1.5), mean(0), gain(1, 2.0), box3(0)]);
    let want = reference(&e, w, h, &img);
    let (whole, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    let rel = max_rel(&whole, &want);
    assert!(rel <= 1e-5, "whole: {rel}");
    // Grid reduce, the gain before the statistic on the grid, two statistics steps, then
    // the graph's three groups, the first two (point groups) in one kernel.
    assert_eq!(st.dispatches, 1 + 1 + 2 + 2, "{st:?}");
    assert_eq!(en.cached_stats(), 1);
    // A region and tiles use the same statistics: no seams, the same values.
    let roi = Roi::new(100, 50, 60, 70);
    let (part, st) = run(&mut en, &e, None, &src, &f32_req(roi));
    assert_eq!(st.dispatches, 2, "statistics come from the cache");
    assert!(max_rel(&part, &crop(&want, w, roi)) <= 1e-5);
    let req = Request {
        tiling: Tiling::Side(64),
        ..f32_req(Roi::full(w, h))
    };
    let (tiled, _) = run(&mut en, &e, None, &src, &req);
    assert!(max_abs(&tiled, &whole).0 <= 1e-5);
    // A change after the statistic keeps them; one before it takes them again, in place.
    let warm = en.counters();
    set(&mut e, GAIN, 1, "gain", Value::Float(3.0));
    let (_, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    assert_eq!(st.dispatches, 2);
    set(&mut e, GAIN, 0, "gain", Value::Float(0.5));
    let (out, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    assert_eq!(st.dispatches, 5, "the source's grid is kept");
    assert_eq!(en.counters().pipelines, warm.pipelines);
    assert!(max_rel(&out, &reference(&e, w, h, &img)) <= 1e-5);
    // An image larger than the grid: statistics on the reduced grid.
    let (w, h) = (2500, 1200);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let e = edit(vec![mean(0)]);
    let (out, _) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    let full = reference(&e, w, h, &img);
    // The grid's mean is the image's mean up to the reduction's rounding.
    assert!(max_abs(&out, &full).0 <= 2e-3, "{}", max_abs(&out, &full).0);
}

#[test]
fn empty_registry_passes_pixels_through() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = engine(d, GIB);
    let img = synthetic(16, 16);
    let src = en.upload(16, 16, &img).expect("upload");
    let e = Edit::default_for(crate::model::Registry::builtin(), Default::default(), true);
    let g = Graph::build(&e, &OpRegistry::empty(), None).expect("graph");
    // The raw front end and the warp run before the graph; every other module is skipped.
    assert_eq!(g.front_end.len(), 1);
    assert_eq!(g.skipped.len() + 1 + g.warp.len(), e.modules.len());
    let r = en
        .render(&g, &src, &f32_req(Roi::full(16, 16)))
        .expect("render");
    assert_eq!(en.read(&r).expect("read"), img);
}

/// Timings on this GPU for a 24 MP rgba32f image (the lane report). The machine may be
/// under load: no budget is asserted. `cargo test -p specular-develop -- --ignored timings`.
#[test]
#[ignore = "timing report; run by hand"]
fn timings_24mp() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (6000, 4000);
    let img = synthetic(w, h);
    eprintln!("gpu: {}", d.name);
    let chains: [(&str, Edit); 3] = [
        (
            "fused 5-op chain",
            edit(vec![
                gain(0, 1.1),
                add(0, [0.1; 3]),
                gain(1, 0.9),
                add(1, [-0.1; 3]),
                gain(2, 1.2),
            ]),
        ),
        ("3x3 box blur", edit(vec![box3(0)])),
        (
            "gain, blur, add (2 intermediates)",
            edit(vec![gain(0, 1.1), box3(0), add(0, [0.1; 3])]),
        ),
    ];
    for (budget, tiling, what) in [
        (8 * GIB, Tiling::Off, "untiled"),
        (8 * GIB, Tiling::Side(2048), "tiled 2048"),
        (
            Budget::for_device(false, None, None).vram,
            Tiling::Auto,
            "auto (default budget)",
        ),
    ] {
        let mut en = engine(d, budget);
        let src = en.upload(w, h, &img).expect("upload");
        for (name, e) in &chains {
            let g = Graph::build(e, &registry(), None).expect("graph");
            let req = Request {
                precision: Precision::F32,
                tiling,
                checkpoints: false,
                profile: true,
                ..Request::new(Roi::full(w, h))
            };
            let mut best_wall = f64::MAX;
            let mut best_gpu = f64::MAX;
            let mut tiles = 0;
            for _ in 0..5 {
                let t = Instant::now();
                let r = en.render(&g, &src, &req).expect("render");
                en.wait().expect("wait");
                best_wall = best_wall.min(t.elapsed().as_secs_f64() * 1e3);
                best_gpu = best_gpu.min(r.stats.gpu_total_ms().unwrap_or(f64::NAN));
                tiles = r.stats.tiles;
                en.recycle(r);
            }
            eprintln!(
                "24 MP f32 {name}, {what} ({tiles} tiles): GPU {best_gpu:.2} ms, wall {best_wall:.2} ms"
            );
        }
    }
}

/// A render split into one submission per dispatch gives the same
/// bits as one submission (same dispatches, same order).
#[test]
fn split_submissions_change_no_bit() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (300, 200);
    let mut en = engine(d, GIB);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let e = edit(vec![box3(0), gain(0, 1.5), mean(0), box3(1)]);
    let bits = |v: &[f32]| v.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
    let mut outs = Vec::new();
    for ms in [f64::INFINITY, 0.0] {
        en.set_submission_ms(ms);
        en.clear_cache();
        let (out, st) = run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
        if ms == 0.0 {
            assert_eq!(st.submissions.len(), st.dispatches);
        } else {
            assert_eq!(st.submissions.len(), 1);
        }
        assert!(st.dispatches > 2);
        outs.push(bits(&out));
    }
    assert!(outs[0] == outs[1], "split submissions changed the picture");
}

/// Once the device is lost (simulated by destroying it), renders,
/// reads and waits fail with [`Error::DeviceLost`] at once instead of hanging. Covered:
/// wgpu's own loss path (`Device::destroy`; the device-lost callback runs at the next
/// poll). Not covered: a real driver reset, which reports through the same callback.
#[test]
fn a_lost_device_is_reported_not_waited_on() {
    if dev().is_none() {
        return;
    }
    let _l = lock();
    let d = open().expect("a second device");
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let f = flag.clone();
    d.device.set_device_lost_callback(move |_, _| {
        f.store(true, std::sync::atomic::Ordering::Release);
    });
    let mut en = engine(&d, GIB);
    en.watch_device_loss(flag.clone());
    let (w, h) = (64, 48);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let e = edit(vec![box3(0), gain(0, 2.0)]);
    run(&mut en, &e, None, &src, &f32_req(Roi::full(w, h)));
    let g = Graph::build(&e, &registry(), None).expect("graph");
    let r = en
        .render(&g, &src, &f32_req(Roi::full(w, h)))
        .expect("render");
    d.device.destroy();
    let t = Instant::now();
    // wgpu runs the device-lost callback when the device is next polled.
    let _ = d.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(flag.load(std::sync::atomic::Ordering::Acquire));
    assert!(en.device_lost());
    assert_eq!(en.read(&r).err(), Some(Error::DeviceLost));
    assert_eq!(en.wait().err(), Some(Error::DeviceLost));
    let again = en.render(&g, &src, &f32_req(Roi::full(w, h)));
    assert_eq!(again.err(), Some(Error::DeviceLost));
    assert!(t.elapsed() < Duration::from_secs(5));
}

/// The statistics grid reduced per workgroup (`GRID_REDUCE_WG`) is the per-thread
/// grid bit for bit: grids smaller than, equal to and reduced from the source, landscape
/// and portrait, odd sizes, 24 MP; the time of both at 24 MP.
#[test]
fn grid_per_workgroup_equals_grid_per_thread() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut checked = 0usize;
    for (w, h) in [
        (777u32, 555u32),
        (1100, 333),
        (2500, 1200),
        (3001, 4999),
        (6000, 4000),
    ] {
        assert!(wgsl::grid_reduce_wg_fits(
            w,
            crate::graph::stats_grid(w, h).0
        ));
        let img = synthetic(w, h);
        let e = edit(vec![mean(0)]);
        let mut out = Vec::new();
        for plain in [true, false] {
            PLAIN_GRID.store(plain, std::sync::atomic::Ordering::Relaxed);
            let mut en = engine(d, 2 * GIB);
            let src = en.upload(w, h, &img).expect("upload");
            let g = Graph::build(&e, &registry(), None).expect("graph");
            let req = Request {
                profile: en.has_timestamps(),
                ..f32_req(Roi::new(0, 0, w.min(64), h.min(64)))
            };
            let r = en.render(&g, &src, &req).expect("render");
            let ms: f64 = r
                .stats
                .gpu_ms
                .iter()
                .flatten()
                .filter(|(l, _)| l.starts_with("grid_reduce"))
                .map(|(_, m)| m)
                .sum();
            en.recycle(r);
            let (gw, gh, v) = en.grid_values().expect("a grid");
            if w == 6000 {
                eprintln!(
                    "grid {gw}×{gh} of {w}×{h}: per {} {ms:.1} ms",
                    if plain { "thread" } else { "workgroup" }
                );
            }
            out.push(v.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
        }
        PLAIN_GRID.store(false, std::sync::atomic::Ordering::Relaxed);
        let same = out[0].iter().zip(&out[1]).filter(|(a, b)| a == b).count();
        assert_eq!(out[0].len(), out[1].len());
        assert_eq!(
            same,
            out[0].len(),
            "{w}×{h}: {} values differ",
            out[0].len() - same
        );
        checked += same;
    }
    eprintln!("{checked} values, 0 differ");
}

#[test]
fn grid_reduce_per_workgroup_fits() {
    // Up to 16 cells of a 1024-wide grid in 512 pixels: about 31 000 pixels wide.
    assert!(wgsl::grid_reduce_wg_fits(30_000, 1024));
    assert!(!wgsl::grid_reduce_wg_fits(40_000, 1024));
    assert!(wgsl::grid_reduce_wg_fits(500, 500));
    assert!(!wgsl::grid_reduce_wg_fits(500, 0));
}
