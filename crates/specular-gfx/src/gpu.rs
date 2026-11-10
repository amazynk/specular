//! Instance, adapter and device creation.
//!
//! There is no wgpu pipeline cache: with Mesa's own shader disk cache warm, creating every
//! pipeline took 5.1-6.6 ms without it and 6.0-8.8 ms with it (UNSAFE.md), so it was dropped
//! along with the `unsafe` it needs.
//!
//! Creating a Vulkan device takes tens of milliseconds, so the app calls [`Gpu::spawn`] at
//! the very start of `main` and joins the handle once the window exists.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

/// Everything the renderers need from the GPU. Cheap to clone.
#[derive(Clone)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    pub limits: wgpu::Limits,
    /// Milliseconds the instance, adapter and device took to create.
    pub init_ms: f64,
    /// When (ms since process start) the init started and the instance, adapter and device
    /// were ready, for the start-up benchmark.
    pub stages: Vec<(&'static str, f64)>,
    /// Set by the device-lost callback (a driver reset): the app opens a new device.
    pub lost: Arc<AtomicBool>,
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gpu")
            .field("adapter", &self.info.name)
            .field("backend", &self.info.backend)
            .finish()
    }
}

impl Gpu {
    /// Starts GPU initialisation on a background thread.
    pub fn spawn() -> JoinHandle<Result<Gpu, String>> {
        specular_thread::builder("gpu-init")
            .spawn(Gpu::new)
            .expect("spawn gpu-init thread")
    }

    /// Creates an instance, picks the high-performance adapter and opens a device with the
    /// adapter's full limits (large textures and many array layers for the tile cache).
    pub fn new() -> Result<Gpu, String> {
        let t0 = Instant::now();
        let mut stages = vec![("gpu_init_start", specular_log::ms_since_start())];
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.flags = wgpu::InstanceFlags::empty().with_env();
        desc.backends = wgpu::Backends::PRIMARY.with_env();
        let instance = wgpu::Instance::new(desc);
        let t_instance = t0.elapsed().as_secs_f64() * 1000.0;
        stages.push(("gpu_instance", specular_log::ms_since_start()));
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let info = adapter.get_info();
        let t_adapter = t0.elapsed().as_secs_f64() * 1000.0;
        stages.push(("gpu_adapter", specular_log::ms_since_start()));
        let alimits = adapter.limits();
        let mut limits = wgpu::Limits::default().using_resolution(alimits.clone());
        limits.max_texture_dimension_2d = alimits.max_texture_dimension_2d;
        limits.max_texture_array_layers = alimits.max_texture_array_layers;
        limits.max_storage_buffer_binding_size = alimits.max_storage_buffer_binding_size;
        limits.max_buffer_size = alimits.max_buffer_size;
        let mut features = wgpu::Features::empty();
        // Block-compressed textures (BC, ASTC) for the display cache on Apple GPUs.
        #[cfg(target_os = "macos")]
        {
            features |= crate::macos::wanted_features(&adapter);
        }
        // BC on desktop GPUs elsewhere. Requested so a direct DDS/KTX2 block upload can use it
        // (measured in decode_stages: 3–9× faster than CPU decode + RGBA upload from 4096²);
        // not yet wired: textures are still decoded to RGBA on the CPU (specular-texture).
        #[cfg(not(target_os = "macos"))]
        {
            features |= adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC;
        }
        // f16 shader types and subgroup operations for specular-infer's kernels, which check
        // the device's features and fall back to plain kernels without them.
        features |= adapter.features() & (wgpu::Features::SHADER_F16 | wgpu::Features::SUBGROUP);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("specular"),
            required_features: features,
            required_limits: limits.clone(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            // A software adapter (WARP, llvmpipe) takes its "video memory" from system RAM:
            // wgpu's performance setting reserves it in blocks of 128 MiB and more, which a small
            // virtual machine could not give even for the first texture (every frame black);
            // the memory-usage setting reserves 8 to 64 MiB at a time.
            memory_hints: if info.device_type == wgpu::DeviceType::Cpu {
                wgpu::MemoryHints::MemoryUsage
            } else {
                // The performance setting's first blocks are 128 then 256 MiB: allocating the
                // second cost 8 to 15 ms of first pixels in about half the starts. Blocks of
                // 32 MiB growing to 256 MiB keep big images in few blocks without that cost
                // (time to first present, 2026-11-08 under load: median 156 → 138 ms, n = 20 each).
                wgpu::MemoryHints::Manual {
                    suballocated_device_memory_block_size: (32 << 20)..(256 << 20),
                }
            },
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("cannot open GPU device: {e}"))?;
        stages.push(("gpu_device", specular_log::ms_since_start()));
        specular_log::debug!(target: specular_log::GPU,
            "gpu init: instance {t_instance:.1} ms, adapter {:.1} ms, device {:.1} ms",
            t_adapter - t_instance,
            t0.elapsed().as_secs_f64() * 1000.0 - t_adapter
        );
        device.on_uncaptured_error(Arc::new(
            |e| specular_log::error!(target: specular_log::GPU, "wgpu: {e}"),
        ));
        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();
        device.set_device_lost_callback(move |reason, msg| {
            // `Destroyed` is our own drop of a device already replaced.
            if reason != wgpu::DeviceLostReason::Destroyed {
                specular_log::error!(target: specular_log::GPU, "GPU device lost: {msg}");
                flag.store(true, Ordering::Release);
            }
        });

        Ok(Gpu {
            instance,
            adapter,
            device,
            queue,
            info,
            limits,
            init_ms: t0.elapsed().as_secs_f64() * 1000.0,
            stages,
            lost,
        })
    }

    /// Whether the driver reset the device: nothing made on it works any more.
    pub fn is_lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    /// `Device::poll` that survives a lost device (wgpu panics in `poll` once the device is
    /// gone): that becomes [`PollError::Lost`] and the device is marked lost, for the app to
    /// open a new one. Every wait on the GPU outside tests goes through here.
    pub fn poll(&self, ty: wgpu::PollType) -> Result<(), PollError> {
        let r = poll_device(&self.device, ty);
        if r == Err(PollError::Lost) {
            self.lost.store(true, Ordering::Release);
        }
        r
    }

    /// Waits for everything submitted ([`Gpu::poll`]).
    pub fn wait(&self) -> Result<(), PollError> {
        self.poll(wgpu::PollType::wait_indefinitely())
    }

    /// Maps all of `buf` for reading and waits for it. `Err` when the device was lost or the
    /// map failed: the buffer is then not mapped.
    pub fn map_read(&self, buf: &wgpu::Buffer) -> Result<(), PollError> {
        let (tx, rx) = std::sync::mpsc::channel();
        buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.wait()?;
        match rx.try_recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) if !self.gone() => Err(PollError::Other(e.to_string())),
            Ok(Err(_)) => Err(PollError::Lost),
            Err(_) => Err(PollError::Other("the buffer was not mapped".into())),
        }
    }

    /// Whether the device no longer works: lost, or a poll now panics (marked lost then).
    fn gone(&self) -> bool {
        self.is_lost() || self.poll(wgpu::PollType::Poll) == Err(PollError::Lost)
    }

    /// Its dedicated video memory where the platform says ([`video_memory`]).
    pub fn video_memory(&self) -> Option<u64> {
        video_memory(&self.info)
    }

    /// No pipeline cache (see the module docs); kept so pipeline descriptors stay unchanged.
    pub fn cache(&self) -> Option<&wgpu::PipelineCache> {
        None
    }

    pub fn max_texture_size(&self) -> u32 {
        self.limits.max_texture_dimension_2d
    }

    /// A software adapter (WARP, llvmpipe, SwiftShader): no GPU of its own, its "video memory"
    /// is a share of system RAM (the hardware report's "software (CPU)").
    pub fn is_software(&self) -> bool {
        self.info.device_type == wgpu::DeviceType::Cpu
    }

    /// Runs `f`, which creates GPU resources, and returns its result with whether the GPU ran
    /// out of memory meanwhile. The error is caught here rather than reported as an uncaptured
    /// device error, so the caller can try something smaller; whatever `f` created when it ran
    /// out is invalid. Error scopes belong to the calling thread, which `f` must not leave.
    pub fn out_of_memory<T>(&self, f: impl FnOnce() -> T) -> (T, bool) {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let made = f();
        let failed = pollster::block_on(scope.pop()).is_some();
        (made, failed)
    }
}

/// Why a wait on the GPU did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PollError {
    /// The device is gone (a driver reset): nothing made on it works any more.
    Lost,
    /// wgpu reported an error (a timeout, a failed map).
    Other(String),
}

impl std::fmt::Display for PollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollError::Lost => f.write_str("the GPU device was lost"),
            PollError::Other(m) => f.write_str(m),
        }
    }
}

/// [`wgpu::util::DeviceExt::create_buffer_init`] that survives a lost device: wgpu's
/// expects the mapping a lost device refuses (a panic); this leaves the buffer unfilled.
pub trait BufferInit {
    fn buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor<'_>) -> wgpu::Buffer;
}

impl BufferInit for wgpu::Device {
    fn buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor<'_>) -> wgpu::Buffer {
        let len = desc.contents.len() as u64;
        let buffer = self.create_buffer(&wgpu::BufferDescriptor {
            label: desc.label,
            size: if len == 0 {
                0
            } else {
                len.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
            },
            usage: desc.usage,
            mapped_at_creation: len > 0,
        });
        if len > 0 {
            if let Ok(mut range) = buffer.get_mapped_range_mut(..) {
                range.slice(..len as usize).copy_from_slice(desc.contents);
            }
            buffer.unmap();
        }
        buffer
    }
}

/// `device.poll(ty)`, catching wgpu's panic on a lost device ([`Gpu::poll`] for a [`Gpu`]).
pub fn poll_device(device: &wgpu::Device, ty: wgpu::PollType) -> Result<(), PollError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| device.poll(ty))) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(PollError::Other(e.to_string())),
        Err(_) => Err(PollError::Lost),
    }
}

/// The dedicated video memory of the adapter `info` names, where the platform says:
/// Linux reads amdgpu's `mem_info_vram_total` of the DRM card with the adapter's PCI
/// vendor and device ids. `None` elsewhere (other kernel drivers don't publish it; wgpu
/// gives no heap sizes) and for integrated and software adapters, which use system RAM.
pub fn video_memory(info: &wgpu::AdapterInfo) -> Option<u64> {
    if info.device_type != wgpu::DeviceType::DiscreteGpu {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        let hex = |p: std::path::PathBuf| {
            let s = std::fs::read_to_string(p).ok()?;
            u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
        };
        for e in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            let dev = e.path().join("device");
            if hex(dev.join("vendor")) != Some(info.vendor)
                || hex(dev.join("device")) != Some(info.device)
            {
                continue;
            }
            let total = std::fs::read_to_string(dev.join("mem_info_vram_total")).ok()?;
            return total.trim().parse().ok().filter(|&b: &u64| b > 0);
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// Creates a shader module from WGSL source.
pub fn shader(device: &wgpu::Device, label: &str, src: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    })
}

/// Vulkan driver families (ICD globs) and the kernel drivers they run on. The test runner
/// `tools/ci/vk-runner.sh` keeps the same table (checked by a test).
#[cfg(any(target_os = "linux", test))]
const FAMILIES: [(&str, &[&str]); 11] = [
    ("*radeon*", &["amdgpu", "radeon"]),
    ("*intel*", &["i915", "xe"]),
    ("*nouveau*", &["nouveau"]),
    ("*asahi*", &["asahi"]),
    ("*broadcom*", &["v3d", "vc4"]),
    ("*freedreno*", &["msm"]),
    ("*panfrost*", &["panfrost", "panthor"]),
    ("*powervr*", &["powervr", "pvrsrvkm"]),
    ("*virtio*", &["virtio_gpu", "virtio-pci"]),
    ("*gfxstream*", &["virtio_gpu", "virtio-pci"]),
    ("*nvidia*", &["nvidia"]),
];

/// Vulkan drivers (ICD globs for `VK_LOADER_DRIVERS_DISABLE`) for hardware this machine does not
/// have. The Vulkan loader otherwise loads every installed driver at instance creation; on a
/// typical Linux install that is a dozen, and the software rasteriser (lavapipe) alone adds about
/// 15 ms to adapter enumeration. Returns `None` when the user chose drivers already, or when no
/// GPU is recognised (then nothing is disabled and lavapipe stays as the fallback).
pub fn vulkan_drivers_to_skip() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        const VARS: [&str; 4] = [
            "VK_LOADER_DRIVERS_SELECT",
            "VK_LOADER_DRIVERS_DISABLE",
            "VK_DRIVER_FILES",
            "VK_ICD_FILENAMES",
        ];
        if VARS.iter().any(|v| std::env::var_os(v).is_some()) {
            return None;
        }
        let kernel = render_node_drivers()?;
        let known = kernel
            .iter()
            .any(|k| FAMILIES.iter().any(|(_, ks)| ks.contains(&k.as_str())));
        if !known {
            return None;
        }
        let mut skip: Vec<&str> = FAMILIES
            .iter()
            .filter(|(_, ks)| !kernel.iter().any(|k| ks.contains(&k.as_str())))
            .map(|(g, _)| *g)
            .collect();
        skip.push("*lvp*");
        Some(skip.join(","))
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// Vulkan layers (for `VK_LOADER_LAYERS_DISABLE`) that do nothing on this machine but cost
/// start-up time: Mesa's implicit device-selection layer, which queries every DRM device at
/// instance creation and adapter enumeration (2-3 ms) only to reorder the GPUs. With a single
/// render node there is nothing to reorder. Returns `None` with more than one render node,
/// when the user picks layers or a device (`DRI_PRIME`, `MESA_VK_DEVICE_SELECT`), or off
/// Linux.
pub fn vulkan_layers_to_skip() -> Option<&'static str> {
    #[cfg(target_os = "linux")]
    {
        const VARS: [&str; 7] = [
            "VK_LOADER_LAYERS_ENABLE",
            "VK_LOADER_LAYERS_DISABLE",
            "VK_INSTANCE_LAYERS",
            "VK_LAYER_PATH",
            "DRI_PRIME",
            "MESA_VK_DEVICE_SELECT",
            "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE",
        ];
        if VARS.iter().any(|v| std::env::var_os(v).is_some()) {
            return None;
        }
        (render_node_drivers()?.len() == 1).then_some("VK_LAYER_MESA_device_select")
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// Kernel drivers of the DRM render nodes, one entry per node.
#[cfg(target_os = "linux")]
fn render_node_drivers() -> Option<Vec<String>> {
    let mut kernel = Vec::new();
    for e in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
        if !e.file_name().to_string_lossy().starts_with("renderD") {
            continue;
        }
        if let Ok(l) = std::fs::read_link(e.path().join("device/driver"))
            && let Some(n) = l.file_name()
        {
            kernel.push(n.to_string_lossy().into_owned());
        }
    }
    Some(kernel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu() -> Option<Gpu> {
        match Gpu::new() {
            Ok(g) => Some(g),
            Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_some() => panic!("{e}"),
            Err(_) => None,
        }
    }

    /// The cargo runner skips the same drivers for absent hardware as the app.
    #[test]
    fn the_test_runner_knows_every_family() {
        let script =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/ci/vk-runner.sh");
        let s = std::fs::read_to_string(&script).unwrap();
        let block = s
            .split("# families:begin")
            .nth(1)
            .and_then(|b| b.split("# families:end").next())
            .unwrap();
        let theirs: Vec<String> = block
            .lines()
            .filter_map(|l| {
                l.trim()
                    .strip_prefix('"')?
                    .strip_suffix('"')
                    .map(String::from)
            })
            .collect();
        let ours: Vec<String> = FAMILIES
            .iter()
            .map(|(g, ks)| format!("{g}:{}", ks.join(",")))
            .collect();
        assert_eq!(theirs, ours, "tools/ci/vk-runner.sh and FAMILIES differ");
    }

    /// Waits on a destroyed device (as a driver reset leaves it) never panic or hang, and a
    /// readback from it fails; a working device waits as before.
    #[test]
    fn waits_on_a_lost_device_return() {
        let Some(g) = gpu() else { return };
        assert_eq!(g.wait(), Ok(()));
        let buf = g.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: 256,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        assert_eq!(g.map_read(&buf), Ok(()));
        buf.unmap();
        assert!(!g.is_lost());
        // Work in flight when the device goes.
        let mut enc = g.device.create_command_encoder(&Default::default());
        enc.clear_buffer(&buf, 0, None);
        g.queue.submit([enc.finish()]);
        g.device.destroy();
        let t = Instant::now();
        // A destroyed device may wait as done or panic in the poll (then `Lost`).
        let r = g.wait();
        assert!(r.is_ok() || r == Err(PollError::Lost), "{r:?}");
        assert_eq!(r.is_err(), g.is_lost());
        assert!(g.map_read(&buf).is_err());
        assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
    }

    /// Texture ids are unique across UI renderers: one kept from before a reset names
    /// nothing in the new renderer.
    #[test]
    fn texture_ids_are_never_reused() {
        let Some(g) = gpu() else { return };
        let comp = crate::Compositor::new(&g, wgpu::TextureFormat::Rgba16Float, 16, 8, 8);
        let tex = g.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let mut a = crate::UiRenderer::new(&g, &comp);
        let old = a.register_texture(&g, &view);
        let mut b = crate::UiRenderer::new(&g, &comp);
        let new = b.register_texture(&g, &view);
        assert_ne!(old, new);
    }

    /// Video memory is only read for a discrete adapter.
    #[test]
    fn video_memory_only_for_discrete_adapters() {
        let Some(g) = gpu() else { return };
        let mut info = g.info.clone();
        info.device_type = wgpu::DeviceType::IntegratedGpu;
        assert_eq!(video_memory(&info), None);
        info.device_type = wgpu::DeviceType::Cpu;
        assert_eq!(video_memory(&info), None);
        eprintln!(
            "{} ({:?}): video memory {:?} MiB",
            g.info.name,
            g.info.device_type,
            g.video_memory().map(|b| b >> 20)
        );
    }
}
