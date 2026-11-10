//! The GPU device local AI runs on, kept current across a GPU reset.
//!
//! The app publishes its device when it opens one and again after a reset
//! ([`publish`]); each AI worker keeps a [`Session`], which makes its inference context
//! lazily and makes it again on the newest device once the old one is gone. While the old
//! device is lost and no new one is published yet, a session hands out no context (the
//! CPU runs). Speech, OCR, people, search and Develop's AI masks, denoise, remove and depth
//! go through it.

use parking_lot::Mutex;
use specular_gfx::wgpu;
use specular_infer::gpu::Context;

type Device = (wgpu::Device, wgpu::Queue);

/// A device and how many times one was published.
pub(crate) struct Slot(Mutex<(Option<Device>, u64)>);

impl Slot {
    const fn new() -> Slot {
        Slot(Mutex::new((None, 0)))
    }

    fn publish(&self, gpu: Option<&specular_gfx::Gpu>) {
        let mut c = self.0.lock();
        c.0 = gpu.map(|g| (g.device.clone(), g.queue.clone()));
        c.1 += 1;
    }

    fn current(&self) -> (Option<Device>, u64) {
        let c = self.0.lock();
        (c.0.clone(), c.1)
    }

    fn generation(&self) -> u64 {
        self.0.lock().1
    }
}

/// The app's current device.
static CURRENT: Slot = Slot::new();

/// The app opened `gpu` (at start or after a reset): sessions move to it.
pub(crate) fn publish(gpu: Option<&specular_gfx::Gpu>) {
    CURRENT.publish(gpu);
}

/// One worker's inference context, made again after a GPU reset.
pub(crate) struct Session {
    slot: &'static Slot,
    /// The device the worker was started with, used until another is published.
    initial: Option<Device>,
    generation: u64,
    ctx: Option<Context>,
    made: bool,
}

impl Session {
    pub(crate) fn new(initial: Option<Device>) -> Session {
        Session::on(&CURRENT, initial)
    }

    fn on(slot: &'static Slot, initial: Option<Device>) -> Session {
        Session {
            slot,
            initial,
            generation: slot.generation(),
            ctx: None,
            made: false,
        }
    }

    /// The context to use: made on first use, made again on the newest device after a
    /// reset. None for the CPU (no device, or the device lost and no new one yet).
    pub(crate) fn context(&mut self) -> Option<&Context> {
        let (device, g) = if self.generation != self.slot.generation() {
            let (d, g) = self.slot.current();
            (d, Some(g))
        } else {
            (None, None)
        };
        if let Some(g) = g {
            if self.made {
                specular_log::info!(target: specular_log::GPU, "ai: inference moves to the new GPU device");
            }
            self.generation = g;
            self.ctx = device.map(|(d, q)| Context::new(&d, &q));
            self.made = true;
        } else if !self.made {
            self.ctx = self.initial.as_ref().map(|(d, q)| Context::new(d, q));
            self.made = true;
        } else if self.ctx.as_ref().is_some_and(Context::is_lost) {
            self.ctx = None;
        }
        self.ctx.as_ref()
    }

    /// The device of [`Session::context`]'s context, for GPU work outside inference.
    pub(crate) fn device_of(ctx: Option<&Context>) -> Option<Device> {
        ctx.map(|c| (c.device().clone(), c.queue().clone()))
    }

    /// Whether what was built on [`Session::context`] must be built again: its device was
    /// lost, or a new one was published.
    pub(crate) fn stale(&self) -> bool {
        self.made
            && (self.generation != self.slot.generation()
                || self.ctx.as_ref().is_some_and(Context::is_lost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_moves_to_the_new_device_after_a_loss() {
        let Ok(old) = specular_gfx::Gpu::new() else {
            assert!(
                std::env::var_os("SPECULAR_REQUIRE_GPU").is_none(),
                "SPECULAR_REQUIRE_GPU is set but there is no adapter"
            );
            return;
        };
        let slot: &'static Slot = Box::leak(Box::new(Slot::new()));
        slot.publish(Some(&old));
        let mut s = Session::on(slot, Some((old.device.clone(), old.queue.clone())));
        assert!(!s.stale());
        let first = s.context().cloned().expect("a context");
        assert!(!s.stale());
        // Built on it before the loss: a graph.
        let mut g = specular_infer::Graph::new();
        let a = g.input(&[2, 8], specular_infer::DType::F32).unwrap();
        let y = g.unary(a, specular_infer::Act::Relu, 1.0, 0.0).unwrap();
        g.output(y).unwrap();
        let x = specular_infer::Tensor::f32(&[2, 8], vec![-1.0; 16]);
        let mut gg = specular_infer::gpu::GpuGraph::new(&first, &g).unwrap();
        old.device.destroy();
        assert_eq!(
            gg.run(&[specular_infer::Feed::Tensor(&x)]),
            Err(specular_infer::Error::Lost)
        );
        assert!(s.stale(), "lost");
        // No new device yet: the CPU.
        assert!(s.context().is_none());
        assert!(!s.stale());
        // The app opens a new device.
        let new = specular_gfx::Gpu::new().expect("a new device");
        slot.publish(Some(&new));
        assert!(s.stale(), "a new device");
        let ctx = s.context().cloned().expect("a context on the new device");
        assert!(!s.stale());
        let mut gg = specular_infer::gpu::GpuGraph::new(&ctx, &g).unwrap();
        let out = gg.run(&[specular_infer::Feed::Tensor(&x)]).unwrap();
        assert_eq!(out[0].to_f32(), vec![0.0; 16]);
    }
}
