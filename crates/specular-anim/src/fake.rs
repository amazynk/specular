//! A synthetic [`Frames`] source with deterministic delays and a configurable decode cost, for
//! tests and benchmarks.

use specular_core::{CancelToken, PixelLayout, Raster, RasterPlane};
use specular_decode::{DecodeError, Frame, Frames, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// What a [`FakeFrames`] produces.
#[derive(Clone, Debug)]
pub struct FakeSpec {
    pub count: u32,
    /// Delay of frame `i` is `delays[i % delays.len()]`.
    pub delays: Vec<Duration>,
    pub width: u32,
    pub height: u32,
    /// Time each `next_frame` takes (on top of filling the canvas); honours cancellation.
    pub decode_cost: Duration,
    /// `next_frame` fails at this index.
    pub fail_at: Option<u32>,
    /// `next_frame` needs more free RAM than there is at this index.
    pub memory_at: Option<u32>,
    /// `next_frame` panics at this index (a decoder bug, for the worker's panic boundary).
    pub panic_at: Option<u32>,
    /// Behave like a movie: every delay is known up front ([`Frames::delays`]) and
    /// [`Frames::seek`] positions the source (counting as `seeks`, not as decodes).
    pub seekable: bool,
    /// Behave like long-GOP video (with `seekable`): a sync frame every `gop` frames
    /// ([`Frames::sync_frames`]), and a seek decodes (at `decode_cost` a frame) from the sync
    /// frame before its target unless the decoder is already on the way there.
    pub gop: Option<u32>,
    /// [`Frames::set_interrupt`] is honoured between two frames decoded.
    pub interruptible: bool,
}

impl FakeSpec {
    pub fn new(count: u32, delay: Duration, width: u32, height: u32) -> FakeSpec {
        FakeSpec {
            count,
            delays: vec![delay],
            width,
            height,
            decode_cost: Duration::ZERO,
            fail_at: None,
            memory_at: None,
            panic_at: None,
            seekable: false,
            gop: None,
            interruptible: false,
        }
    }

    /// Bytes of one frame.
    pub fn frame_bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }
}

/// Observes a [`FakeFrames`] from outside (it moves into the player's worker).
#[derive(Debug, Default)]
pub struct FakeProbe {
    pub decoded: AtomicU64,
    pub rewinds: AtomicU32,
    pub seeks: AtomicU32,
    /// Decodes running right now (must never exceed 1).
    pub busy: AtomicU32,
    pub dropped: AtomicBool,
    /// Decodes stopped by the interrupt flag.
    pub interrupted: AtomicU32,
}

/// Deterministic frame source: frame `i` is an RGBA canvas filled with byte `i % 256`.
pub struct FakeFrames {
    spec: FakeSpec,
    /// The frame `next_frame` hands out.
    next: u32,
    /// With a GOP: the next frame the decoder itself decodes (`<= next`).
    dec: u32,
    probe: Arc<FakeProbe>,
    interrupt: Option<Arc<AtomicBool>>,
}

impl FakeFrames {
    pub fn new(spec: FakeSpec) -> (FakeFrames, Arc<FakeProbe>) {
        let probe = Arc::new(FakeProbe::default());
        (
            FakeFrames {
                spec,
                next: 0,
                dec: 0,
                probe: Arc::clone(&probe),
                interrupt: None,
            },
            probe,
        )
    }
}

impl Frames for FakeFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.next >= self.spec.count {
            return None;
        }
        let i = self.next;
        if self.spec.memory_at == Some(i) {
            return Some(Err(DecodeError::NeedsMemory(specular_core::NeedsMemory {
                needed: 8 << 30,
                free: 1 << 30,
            })));
        }
        if self.spec.fail_at == Some(i) {
            return Some(Err(DecodeError::Malformed(format!("frame {i}"))));
        }
        if self.spec.panic_at == Some(i) {
            panic!("fake decoder panic at frame {i}");
        }
        self.probe.busy.fetch_add(1, Ordering::SeqCst);
        // With a GOP, the frames from where the decoder is up to this one; else this one.
        let from = if self.spec.gop.is_some() {
            self.dec.min(i)
        } else {
            i
        };
        for k in from..=i {
            if self
                .interrupt
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Acquire))
            {
                self.probe.interrupted.fetch_add(1, Ordering::SeqCst);
                self.probe.busy.fetch_sub(1, Ordering::SeqCst);
                return Some(Err(DecodeError::Cancelled));
            }
            let until = Instant::now() + self.spec.decode_cost;
            loop {
                if cancel.is_cancelled() {
                    self.probe.busy.fetch_sub(1, Ordering::SeqCst);
                    return Some(Err(DecodeError::Cancelled));
                }
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                std::thread::sleep(left.min(Duration::from_millis(1)));
            }
            self.dec = k + 1;
            if k < i {
                self.probe.decoded.fetch_add(1, Ordering::SeqCst);
            }
        }
        let (w, h) = (self.spec.width, self.spec.height);
        let stride = w as usize * 4;
        let raster = Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride,
                data: vec![(i % 256) as u8; stride * h as usize],
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        };
        let delay = self.spec.delays[i as usize % self.spec.delays.len().max(1)];
        self.next += 1;
        self.dec = self.next;
        self.probe.decoded.fetch_add(1, Ordering::SeqCst);
        self.probe.busy.fetch_sub(1, Ordering::SeqCst);
        Some(Ok(Frame {
            index: i,
            raster,
            delay,
        }))
    }

    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        self.dec = 0;
        self.probe.rewinds.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn seek(&mut self, index: u32) -> Result<bool> {
        if !self.spec.seekable {
            return Ok(false);
        }
        let index = index.min(self.spec.count);
        if let Some(g) = self.spec.gop.filter(|&g| g > 0) {
            let key = index / g * g;
            // Going on from where the decoder is, or starting over at the sync frame.
            if !(self.dec <= index && key <= self.dec) {
                self.dec = key;
            }
        }
        self.next = index;
        self.probe.seeks.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }

    fn sync_frames(&self) -> Option<Vec<u32>> {
        let g = self.spec.gop.filter(|&g| g > 0 && self.spec.seekable)?;
        Some((0..self.spec.count).step_by(g as usize).collect())
    }

    fn set_interrupt(&mut self, flag: Arc<AtomicBool>) -> bool {
        if self.spec.interruptible {
            self.interrupt = Some(flag);
        }
        self.spec.interruptible
    }

    fn delays(&self) -> Option<Vec<Duration>> {
        self.spec.seekable.then(|| {
            let d = &self.spec.delays;
            (0..self.spec.count as usize)
                .map(|i| d[i % d.len().max(1)])
                .collect()
        })
    }
}

impl Drop for FakeFrames {
    fn drop(&mut self) {
        self.probe.dropped.store(true, Ordering::SeqCst);
    }
}
