//! Colour management in the renderer: per-image source transforms (the convert shader's
//! curves + matrix or 3D LUT, from [`crate::icc`]) and the storage space.
//!
//! The overview and tiles store linear light in an 8-bit sRGB-encoded format. With a
//! wide-gamut matrix display, the storage space is the display's own primaries (so 8-bit
//! textures keep the display's gamut instead of clipping to BT.709): the convert shader maps
//! linear BT.709 → storage, the image shader maps storage → linear BT.709 again, and the
//! compositor's final pass maps BT.709 → display. Each image keeps the storage space it was
//! converted with, so changing the display never mixes spaces within one image.

use specular_color::SourceLut;
use specular_color::gpu::{CUBE_SIZE, CURVE_SIZE};
use specular_gfx::wgpu::util::DeviceExt;
use specular_gfx::{Gpu, wgpu};

use super::{GpuImage, ImageRenderer};

/// 3×3 identity, row-major.
pub const IDENTITY: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

/// Linear BT.709 ↔ storage-space matrices (row-major).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Storage {
    pub to: [f32; 9],
    pub from: [f32; 9],
}

impl Default for Storage {
    fn default() -> Self {
        Storage {
            to: IDENTITY,
            from: IDENTITY,
        }
    }
}

/// A 3×3 row-major matrix as three vec4 rows (WGSL uniform layout).
pub(super) fn rows(m: &[f32; 9]) -> [f32; 12] {
    [
        m[0], m[1], m[2], 0.0, m[3], m[4], m[5], 0.0, m[6], m[7], m[8], 0.0,
    ]
}

/// Identity 3×4 matrix rows (no source matrix).
pub(super) const IDENTITY_ROWS: [f32; 12] =
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// A source transform on the GPU.
#[derive(Debug)]
pub struct GpuLut {
    _texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    /// Convert-shader mode: 1 cube, 2 curves + matrix.
    pub(super) mode: u32,
    /// Grid points of the cube / samples of the curves.
    pub(super) size: u32,
    /// Matrix rows with offsets in w (curves mode; identity for cubes).
    pub(super) matrix: [f32; 12],
}

impl GpuLut {
    /// Uploads a source transform: a cube as a `CUBE_SIZE`³ 3D texture, curves as a
    /// `CURVE_SIZE`×1×1 one (so both bind as the same `texture_3d`).
    pub(super) fn new(gpu: &Gpu, lut: &SourceLut) -> GpuLut {
        let (data, (w, h, d), mode, matrix) = match lut {
            SourceLut::Cube { data } => (
                data,
                (CUBE_SIZE as u32, CUBE_SIZE as u32, CUBE_SIZE as u32),
                1,
                IDENTITY_ROWS,
            ),
            SourceLut::Curves { data, matrix } => (data, (CURVE_SIZE as u32, 1, 1), 2, *matrix),
        };
        let texture = gpu.device.create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some(if mode == 1 { "icc cube" } else { "icc curves" }),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: d,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(data),
        );
        let view = texture.create_view(&Default::default());
        GpuLut {
            _texture: texture,
            view,
            mode,
            size: w,
            matrix,
        }
    }
}

impl ImageRenderer {
    /// Sets the storage space for images created from now on (from
    /// `specular_color::Display::{to_storage, from_storage}`). Images already on the GPU keep
    /// theirs; recreate them to move them into the new space.
    pub fn set_storage(&mut self, to: [f32; 9], from: [f32; 9]) {
        self.storage = Storage { to, from };
    }

    /// The storage space for new images.
    pub fn storage(&self) -> Storage {
        self.storage
    }
}

impl GpuImage {
    /// The storage space this image's textures hold.
    pub fn storage(&self) -> Storage {
        self.storage
    }
}
