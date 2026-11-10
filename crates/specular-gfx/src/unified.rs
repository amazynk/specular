//! Uploads for GPUs that share memory with the CPU (Apple Silicon, integrated GPUs).
//!
//! `Queue::write_texture` copies the pixels into a staging buffer on the CPU, then the GPU blits
//! that buffer into the texture. On unified memory the staging buffer *is* GPU-visible memory
//! (wgpu's Metal backend creates `MAP_WRITE` buffers with `MTLStorageModeShared`), so a decoder
//! that writes its output rows straight into a [`MappedUpload`] skips that CPU copy: for a
//! 50 MP RGBA image that is a 200 MB memcpy, mostly cache misses, taken off the critical path.
//! The GPU-side blit into the (private, tiled) texture stays.
//!
//! Not wired into the renderer yet: the decoders emit `Raster` planes in their own `Vec`s, and
//! copying a finished plane into a mapping costs the same as `write_texture`. The win needs the
//! decoder to write each finished band of rows here while it is still in cache
//! (docs/macos.md, "Unified-memory upload").

use crate::gpu::Gpu;

/// A row-pitched, persistently mapped upload buffer for one 2D texture region.
pub struct MappedUpload {
    buffer: wgpu::Buffer,
    view: Option<wgpu::BufferViewMut>,
    width: u32,
    height: u32,
    row_bytes: u32,
    /// Bytes per buffer row: `row_bytes` rounded up to wgpu's 256-byte copy alignment.
    pitch: u32,
}

impl std::fmt::Debug for MappedUpload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappedUpload")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pitch", &self.pitch)
            .finish()
    }
}

impl Gpu {
    /// Whether the adapter shares memory with the CPU, so [`MappedUpload`] can save a copy.
    pub fn unified_memory(&self) -> bool {
        self.info.device_type == wgpu::DeviceType::IntegratedGpu
    }
}

impl MappedUpload {
    /// A mapped buffer for `width × height` texels of `bytes_per_texel` bytes each.
    pub fn new(gpu: &Gpu, width: u32, height: u32, bytes_per_texel: u32) -> MappedUpload {
        let row_bytes = width * bytes_per_texel;
        let pitch = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mapped-upload"),
            size: u64::from(pitch) * u64::from(height.max(1)),
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        let view = buffer.get_mapped_range_mut(..).ok();
        MappedUpload {
            buffer,
            view,
            width,
            height,
            row_bytes,
            pitch,
        }
    }

    /// Bytes per buffer row (at least `width × bytes_per_texel`).
    pub fn pitch(&self) -> u32 {
        self.pitch
    }

    /// Writes rows `first..` from `src`, whose rows are `stride` bytes apart. Rows past the
    /// bottom are ignored. Call it from the decoder as each band of rows is finished.
    pub fn write_rows(&mut self, first: u32, src: &[u8], stride: usize) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        let row = self.row_bytes as usize;
        if stride < row || src.len() < row {
            return;
        }
        let rows = (src.len() - row) / stride + 1;
        for (i, line) in (first..self.height).zip(0..rows) {
            let at = i as usize * self.pitch as usize;
            view.slice(at..at + row)
                .copy_from_slice(&src[line * stride..line * stride + row]);
        }
    }

    /// Unmaps the buffer and records the copy into `dst` at `origin`. Submit `enc` afterwards.
    pub fn finish(
        mut self,
        enc: &mut wgpu::CommandEncoder,
        dst: &wgpu::Texture,
        origin: wgpu::Origin3d,
    ) {
        self.view = None;
        self.buffer.unmap();
        if self.width == 0 || self.height == 0 {
            return;
        }
        enc.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.pitch),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: dst,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
}
