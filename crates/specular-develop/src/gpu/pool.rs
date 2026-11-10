//! Buffer pool, checkpoint cache and the VRAM budget.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::graph::{Precision, Roi};
use crate::model::hash::Hash;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

/// The most kept scratch ([`Budget::kept`]).
pub const KEPT_MAX: u64 = 512 * MIB;
/// The most free buffers kept between renders ([`Budget::free`]).
pub const FREE_MAX: u64 = 512 * MIB;

/// The app's readings of its device's dedicated memory and of the machine's RAM (0: none),
/// for [`Budget::for_device`] callers that pass none.
static DEVICE_LOCAL: AtomicU64 = AtomicU64::new(0);
static HOST_RAM: AtomicU64 = AtomicU64::new(0);

/// Records the memory the app read of its GPU (dedicated memory, where the platform says)
/// and of the machine (total RAM): [`Budget::for_device`] uses them where its caller passes
/// none. Unset (tests, tools) it assumes 1 GiB and 4 GiB.
pub fn set_memory_readings(device_local: Option<u64>, host_ram: Option<u64>) {
    DEVICE_LOCAL.store(device_local.unwrap_or(0), Ordering::Relaxed);
    HOST_RAM.store(host_ram.unwrap_or(0), Ordering::Relaxed);
}

/// Video memory the engine may use: half for the working set of a render, half for the
/// checkpoint cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub vram: u64,
}

impl Budget {
    /// A fixed budget (tests, or a value read elsewhere).
    pub const fn fixed(bytes: u64) -> Budget {
        Budget { vram: bytes }
    }

    /// The plan's caps: on a discrete GPU min(1.5 GiB, 40 % of device-local memory); on an
    /// integrated one 25 % of RAM, since it shares it. wgpu does not report video memory:
    /// a reading not passed comes from [`set_memory_readings`], else 1 GiB of device memory
    /// (or 4 GiB of RAM) is assumed.
    pub fn for_device(
        integrated: bool,
        device_local: Option<u64>,
        host_ram: Option<u64>,
    ) -> Budget {
        let reading = |r: &AtomicU64| Some(r.load(Ordering::Relaxed)).filter(|&b| b > 0);
        let device_local = device_local.or_else(|| reading(&DEVICE_LOCAL));
        let host_ram = host_ram.or_else(|| reading(&HOST_RAM));
        let vram = if integrated {
            host_ram.unwrap_or(4 * GIB) / 4
        } else {
            (device_local.unwrap_or(GIB) / 5 * 2).min(3 * GIB / 2)
        };
        Budget { vram }
    }

    /// Bytes one render's intermediate buffers may use at once.
    pub fn working(&self) -> u64 {
        self.vram / 2
    }

    /// Bytes the checkpoint cache may hold.
    pub fn cache(&self) -> u64 {
        self.vram - self.working()
    }

    /// Bytes of persistent programs' scratch kept between renders (diffuse's draft and
    /// refined results at a preview size fit; a 24 MP one is let go): half the working set,
    /// at most [`KEPT_MAX`].
    pub fn kept(&self) -> u64 {
        (self.working() / 2).min(KEPT_MAX)
    }

    /// Bytes of free buffers kept for the next render (a preview's intermediates fit; an
    /// export's are let go): a quarter of the working set, at most [`FREE_MAX`].
    pub fn free(&self) -> u64 {
        (self.working() / 4).min(FREE_MAX)
    }
}

/// A GPU buffer with an identity (bind groups are cached by it).
#[derive(Clone, Debug)]
pub struct GpuBuf {
    pub buffer: wgpu::Buffer,
    pub id: u64,
    pub size: u64,
}

/// The size a request for `bytes` is rounded to: buffers of one class are interchangeable.
/// At most 1/8 is wasted, and small buffers come in 64 KiB steps.
pub fn size_class(bytes: u64) -> u64 {
    let b = bytes.max(16);
    let step = (b.next_power_of_two() / 8).max(64 * KIB);
    b.div_ceil(step) * step
}

/// Free buffers by size class, reused last-in first-out so a repeated render binds the
/// same buffers (and finds its bind groups cached).
#[derive(Default)]
pub struct Pool {
    free: Vec<GpuBuf>,
    /// Bytes of every buffer the pool made and has not destroyed (free or handed out).
    pub live: u64,
    pub created: u64,
    /// The device's largest buffer (0: no limit): a size class past it is made at the
    /// size asked for (a 1.9 GB scratch rounded to 2 GiB failed on a 2 GiB − 1 limit).
    pub max_buffer: u64,
}

impl Pool {
    /// A pool for a device whose largest buffer is `max` bytes.
    pub fn with_max_buffer(max: u64) -> Pool {
        Pool {
            max_buffer: max,
            ..Pool::default()
        }
    }

    pub fn acquire(&mut self, device: &wgpu::Device, next_id: &mut u64, bytes: u64) -> GpuBuf {
        let mut class = size_class(bytes);
        if self.max_buffer > 0 && class > self.max_buffer {
            class = bytes.max(16).next_multiple_of(16);
        }
        if let Some(i) = self.free.iter().rposition(|b| b.size == class) {
            return self.free.remove(i);
        }
        *next_id += 1;
        self.created += 1;
        self.live += class;
        GpuBuf {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop pixels"),
                size: class,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            id: *next_id,
            size: class,
        }
    }

    pub fn release(&mut self, b: GpuBuf) {
        self.free.push(b);
    }

    pub fn free_bytes(&self) -> u64 {
        self.free.iter().map(|b| b.size).sum()
    }

    /// Destroys the oldest free buffers until at most `keep` bytes are free; returns the
    /// ids destroyed.
    pub fn trim(&mut self, keep: u64) -> Vec<u64> {
        let mut gone = Vec::new();
        let mut free = self.free_bytes();
        while free > keep && !self.free.is_empty() {
            let b = self.free.remove(0);
            free -= b.size;
            self.live -= b.size;
            b.buffer.destroy();
            gone.push(b.id);
        }
        gone
    }
}

/// A checkpoint's identity: the edit up to it, the source, region, scale, precision and the
/// kernels' version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub prefix: Hash,
    pub source: u64,
    pub roi: Roi,
    pub scale_bits: u32,
    pub precision: Precision,
    pub version: u32,
}

struct Entry {
    buf: GpuBuf,
    used: u64,
}

/// The checkpoint cache: a VRAM LRU. Evicted outputs are recomputed, never swapped out.
#[derive(Default)]
pub struct Cache {
    entries: HashMap<Key, Entry>,
    tick: u64,
    pub bytes: u64,
}

impl Cache {
    pub fn get(&mut self, k: &Key) -> Option<GpuBuf> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(k).map(|e| {
            e.used = tick;
            e.buf.clone()
        })
    }

    pub fn contains(&self, k: &Key) -> bool {
        self.entries.contains_key(k)
    }

    /// Keeps `buf` under `k`; returns a buffer it replaced.
    pub fn insert(&mut self, k: Key, buf: GpuBuf) -> Option<GpuBuf> {
        self.tick += 1;
        self.bytes += buf.size;
        let old = self.entries.insert(
            k,
            Entry {
                buf,
                used: self.tick,
            },
        );
        old.map(|e| {
            self.bytes -= e.buf.size;
            e.buf
        })
    }

    /// Evicts least recently used entries until at most `budget` bytes remain.
    pub fn trim(&mut self, budget: u64) -> Vec<GpuBuf> {
        let mut out = Vec::new();
        while self.bytes > budget {
            let Some(k) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| *k)
            else {
                break;
            };
            let e = self.entries.remove(&k).expect("entry present");
            self.bytes -= e.buf.size;
            out.push(e.buf);
        }
        out
    }

    /// Takes the entry under `k` out.
    pub fn remove(&mut self, k: &Key) -> Option<GpuBuf> {
        let e = self.entries.remove(k)?;
        self.bytes -= e.buf.size;
        Some(e.buf)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
