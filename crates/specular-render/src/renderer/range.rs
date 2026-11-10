//! Auto-range for non-DICOM high-bit greyscale (DESIGN.md §6.2): a 16-bit TIFF, PNG or
//! FITS-like grey image is stretched from its smallest to its largest code when converted,
//! instead of showing a 12-bit scan as a nearly black picture. The range is found on the GPU
//! (`range.wgsl`) from the first wide grey raster uploaded (the overview, normally the whole
//! image) and read back asynchronously for the colour bar.
//!
//! With auto-range on, the convert pass also keeps HiLo exact through 8-bit storage: codes at
//! the real bit depth's maximum (minimum) store as full white (black) and every other code
//! within 1/255 … 254/255, so the image shader's thresholds mark exactly those pixels.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use specular_core::PixelLayout;
use specular_gfx::BufferInit;
use specular_gfx::{Gpu, wgpu};

use super::{GpuImage, ImageRenderer};

/// Per-image auto-range state.
#[derive(Debug)]
pub(super) struct AutoRange {
    /// Significant bits of the samples (HiLo's limits), 1‥16.
    pub(super) bits: u8,
    /// `[min, max]` codes (u32 each), bound by the convert pass.
    pub(super) buf: wgpu::Buffer,
    read: wgpu::Buffer,
    /// 0 idle, 1 mapping, 2 mapped, 3 failed.
    state: Arc<AtomicU8>,
    /// The range has been computed into `buf`.
    pub(super) computed: bool,
    value: Option<[u16; 2]>,
}

impl AutoRange {
    fn new(gpu: &Gpu, bits: u8) -> AutoRange {
        let buf = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("auto range"),
            contents: bytemuck::cast_slice(&[0u32, 65535]),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("auto range readback"),
            size: 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        AutoRange {
            bits: bits.clamp(1, 16),
            buf,
            read,
            state: Arc::new(AtomicU8::new(0)),
            computed: false,
            value: None,
        }
    }
}

/// The range pipeline, built on first use.
#[derive(Debug)]
pub(super) struct RangePipe {
    pipe: wgpu::ComputePipeline,
    bgl: wgpu::BindGroupLayout,
}

impl GpuImage {
    /// The auto-range in 16-bit codes (`[min, max]`), once the GPU has found it. Polls the
    /// readback: call once per frame while it is `None`.
    pub fn auto_range(&mut self) -> Option<[u16; 2]> {
        let r = self.range.as_mut()?;
        if r.value.is_none() && r.state.load(Ordering::Acquire) == 2 {
            let slice = r.read.slice(..);
            if let Ok(data) = slice.get_mapped_range() {
                let w: &[u32] = bytemuck::cast_slice(&data[..8]);
                if w[0] <= w[1] {
                    r.value = Some([w[0].min(65535) as u16, w[1].min(65535) as u16]);
                }
            }
            r.read.unmap();
            r.state.store(0, Ordering::Release);
        }
        r.value
    }

    /// The significant bits auto-range was enabled with.
    pub fn auto_range_bits(&self) -> Option<u8> {
        self.range.as_ref().map(|r| r.bits)
    }
}

impl ImageRenderer {
    /// Turns auto-range on (with the samples' significant bits, e.g. 12 for a 12-bit TIFF)
    /// or off for `img`. Applies to wide grey layouts ([`PixelLayout::Gray16`],
    /// [`PixelLayout::GrayAlpha16`]) uploaded from now on; the caller re-uploads pixels
    /// already converted.
    pub fn set_auto_range(&mut self, gpu: &Gpu, img: &mut GpuImage, bits: Option<u8>) {
        if img.range.as_ref().map(|r| r.bits) == bits {
            return;
        }
        img.range = bits.map(|b| AutoRange::new(gpu, b));
        self.pool.evict_image(img.id);
        img.revision += 1;
    }

    /// Records the range pass over `plane` (the raster's 16-bit plane, `size` texels) into `enc` when `img`
    /// wants auto-range and has none yet. Returns whether one was recorded (the caller then
    /// starts the readback after submitting).
    pub(super) fn record_range(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        img: &mut GpuImage,
        layout: PixelLayout,
        plane: Option<&wgpu::TextureView>,
        size: (u32, u32),
    ) -> bool {
        let (Some(r), Some(plane)) = (img.range.as_mut(), plane) else {
            return false;
        };
        let alpha = match layout {
            PixelLayout::Gray16 => 0u32,
            PixelLayout::GrayAlpha16 => 1,
            _ => return false,
        };
        if r.computed || r.state.load(Ordering::Acquire) != 0 {
            return false;
        }
        let d = &gpu.device;
        let pipes = self.range_pipe.get_or_insert_with(|| {
            let module = specular_gfx::shader(d, "range", include_str!("../shaders/range.wgsl"));
            let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("range"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("range"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            let pipe = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("range"),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("cs_range"),
                compilation_options: Default::default(),
                cache: gpu.cache(),
            });
            RangePipe { pipe, bgl }
        });
        gpu.queue
            .write_buffer(&r.buf, 0, bytemuck::cast_slice(&[u32::MAX, 0]));
        let params = d.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("range params"),
            contents: bytemuck::cast_slice(&[alpha, 0, 0, 0]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("range"),
            layout: &pipes.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(plane),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: r.buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("range"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&pipes.pipe);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(size.0.div_ceil(16), size.1.div_ceil(16), 1);
        }
        enc.copy_buffer_to_buffer(&r.buf, 0, &r.read, 0, 8);
        r.computed = true;
        true
    }

    /// Starts reading back the range recorded by [`ImageRenderer::record_range`] (after the
    /// submit).
    pub(super) fn read_range(img: &GpuImage) {
        let Some(r) = img.range.as_ref() else {
            return;
        };
        r.state.store(1, Ordering::Release);
        let s = r.state.clone();
        r.read.slice(..).map_async(wgpu::MapMode::Read, move |res| {
            s.store(if res.is_ok() { 2 } else { 3 }, Ordering::Release);
        });
    }

    /// The convert pass's auto-range fields (flags, bits) and range buffer for `img`.
    pub(super) fn range_binding<'a>(
        &'a self,
        img: &'a GpuImage,
        layout: PixelLayout,
    ) -> ([u32; 2], &'a wgpu::Buffer) {
        match (&img.range, layout) {
            (Some(r), PixelLayout::Gray16 | PixelLayout::GrayAlpha16) if r.computed => {
                ([1, r.bits as u32], &r.buf)
            }
            _ => ([0, 16], &self.dummy_range),
        }
    }
}
