//! A lost device (a driver reset, as `Device::destroy` leaves it): building or running a
//! graph on it neither panics nor hangs, it says the device was lost, and the same graph
//! runs on a new device.

use std::time::{Duration, Instant};

use specular_infer::gpu::{Context, GpuGraph};
use specular_infer::tensor::Rng;
use specular_infer::{Act, DType, Error, Feed, Graph, cpu};

fn graph() -> Graph {
    let mut g = Graph::new();
    let a = g.input(&[4, 33], DType::F32).unwrap();
    let y = g.unary(a, Act::Gelu, 1.0, 0.0).unwrap();
    g.output(y).unwrap();
    g
}

fn device() -> Option<specular_gfx::Gpu> {
    match specular_gfx::Gpu::new() {
        Ok(g) => Some(g),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    }
}

#[test]
fn a_lost_device_errors_then_a_new_one_works() {
    let Some(gpu) = device() else { return };
    let g = graph();
    let x = Rng::new(3).tensor(&[4, 33], 2.0);
    let want = cpu::run(&g, &[Feed::Tensor(&x)], 1).unwrap();
    let ctx = Context::new(&gpu.device, &gpu.queue);
    let mut gg = GpuGraph::new(&ctx, &g).unwrap();
    assert!(gg.run(&[Feed::Tensor(&x)]).is_ok());
    assert!(!ctx.is_lost());

    gpu.device.destroy();
    let t = Instant::now();
    let run = gg.run(&[Feed::Tensor(&x)]);
    assert_eq!(run, Err(Error::Lost), "a graph built before the loss");
    assert!(ctx.is_lost());
    // Built after the loss, on the same context.
    let built = GpuGraph::new(&ctx, &g).map(|_| ());
    assert_eq!(built, Err(Error::Lost));
    // A new context on the dead device finds out at its first use.
    let late = Context::new(&gpu.device, &gpu.queue);
    let r = GpuGraph::new(&late, &g).and_then(|mut gg| gg.run(&[Feed::Tensor(&x)]));
    assert_eq!(r, Err(Error::Lost));
    assert!(late.is_lost());
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());

    // The app opens a new device; the graph runs there as before.
    let fresh = specular_gfx::Gpu::new().expect("a new device");
    let ctx = Context::new(&fresh.device, &fresh.queue);
    let mut gg = GpuGraph::new(&ctx, &g).unwrap();
    let got = gg.run(&[Feed::Tensor(&x)]).unwrap();
    let d = specular_infer::tensor::compare(&got[0].to_f32(), &want[0].to_f32(), 2e-6, 2e-5);
    assert!(d.worst <= 1.0, "{d:?}");
}
