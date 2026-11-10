//! The display path read back at 16 bits: the viewer's image renderer draws the image (with
//! its window, colour map, channel colours and stretch, exactly as on screen) into half-float
//! tiles of the scene format, which are read back and encoded to sRGB as the compositor's
//! final pass does, without its dither and without the display's own profile (a figure is
//! sRGB). An 8-bit image drawn at 1:1 comes back with its own codes (the half-float scene
//! keeps 8-bit codes within 0.06 of a code); 16-bit samples keep the scene's precision.

use specular_core::Orientation;
use specular_gfx::{Gpu, HDR, wgpu};
use specular_render::{GpuImage, ImageRenderer, PxRect, RenderOptions, View};

use crate::Rgb16;

/// What to draw: the stored image of `stored` size seen through `orient`, at `zoom` output px
/// per stored px, with the display point `centre` at the middle of an output of `size` px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub stored: (u32, u32),
    pub orient: Orientation,
    pub zoom: f32,
    pub centre: (f32, f32),
    pub size: (u32, u32),
}

impl Shot {
    /// The whole-pixel region `[x0, y0, x1, y1)` of the oriented (display) image at 1:1.
    pub fn region(stored: (u32, u32), orient: Orientation, r: [u32; 4]) -> Shot {
        let (w, h) = (
            r[2].saturating_sub(r[0]).max(1),
            r[3].saturating_sub(r[1]).max(1),
        );
        Shot {
            stored,
            orient,
            zoom: 1.0,
            centre: (r[0] as f32 + w as f32 * 0.5, r[1] as f32 + h as f32 * 0.5),
            size: (w, h),
        }
    }

    /// The view of the whole output.
    pub fn view(&self) -> View {
        self.tile_view(0, 0)
    }

    fn tile_view(&self, x0: u32, y0: u32) -> View {
        View::fixed(
            self.stored,
            self.orient,
            PxRect {
                x: -(x0 as f32),
                y: -(y0 as f32),
                w: self.size.0 as f32,
                h: self.size.1 as f32,
            },
            self.zoom,
            self.centre,
        )
    }
}

/// Largest tile side drawn at once.
pub const TILE: u32 = 2048;

/// IEEE half to single precision.
pub fn f16_to_f32(h: u16) -> f32 {
    let s = u32::from(h >> 15) << 31;
    let e = u32::from((h >> 10) & 0x1f);
    let m = u32::from(h & 0x3ff);
    let bits = if e == 0 {
        if m == 0 {
            s
        } else {
            // Subnormal: normalise.
            let mut e = 127 - 15 + 1;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            s | (e << 23) | ((m & 0x3ff) << 13)
        }
    } else if e == 31 {
        s | 0x7f80_0000 | (m << 13)
    } else {
        s | ((e + 127 - 15) << 23) | (m << 13)
    };
    f32::from_bits(bits)
}

/// Linear light to the sRGB encoding (the final pass's `to_srgb`), clamped to 0‥1.
pub fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Draws `shot` of `img` with `opts` and reads it back as 16-bit sRGB. The backdrop outside
/// the image is `opts.canvas`.
pub fn read(
    gpu: &Gpu,
    renderer: &mut ImageRenderer,
    img: &GpuImage,
    shot: &Shot,
    opts: &RenderOptions,
) -> Result<Rgb16, String> {
    let (w, h) = shot.size;
    if w == 0 || h == 0 {
        return Err("empty figure panel".into());
    }
    let max = gpu.max_texture_size().clamp(16, TILE);
    let tile = (w.min(max), h.min(max));
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("figure tile"),
        size: wgpu::Extent3d {
            width: tile.0,
            height: tile.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let pitch = (tile.0 * 8).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("figure readback"),
        size: u64::from(pitch) * u64::from(tile.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let c = opts.canvas;
    let mut out = Rgb16 {
        width: w,
        height: h,
        data: vec![0; w as usize * h as usize * 3],
    };
    // sRGB codes 0..=65535 of every half float would be a 64k table; the values are few.
    let enc = |v: u16| (to_srgb(f16_to_f32(v)) * 65535.0).round() as u16;
    let mut y0 = 0;
    while y0 < h {
        let rows = (h - y0).min(tile.1);
        let mut x0 = 0;
        while x0 < w {
            let cols = (w - x0).min(tile.0);
            let mut e = gpu.device.create_command_encoder(&Default::default());
            drop(e.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("figure clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &tv,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(c[0]),
                            g: f64::from(c[1]),
                            b: f64::from(c[2]),
                            a: f64::from(c[3]),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            }));
            let view = shot.tile_view(x0, y0);
            renderer.render_inset(gpu, &mut e, &tv, tile, img, &view, opts, None);
            e.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(pitch),
                        rows_per_image: Some(tile.1),
                    },
                },
                wgpu::Extent3d {
                    width: tile.0,
                    height: rows,
                    depth_or_array_layers: 1,
                },
            );
            gpu.queue.submit([e.finish()]);
            let slice = buf.slice(..u64::from(pitch) * u64::from(rows));
            slice.map_async(wgpu::MapMode::Read, |_| {});
            gpu.wait().map_err(|e| format!("GPU readback: {e}"))?;
            {
                let data = slice
                    .get_mapped_range()
                    .map_err(|e| format!("GPU readback: {e}"))?;
                for (y, row) in data
                    .chunks_exact(pitch as usize)
                    .take(rows as usize)
                    .enumerate()
                {
                    let at = ((y0 as usize + y) * w as usize + x0 as usize) * 3;
                    for x in 0..cols as usize {
                        let p = &row[x * 8..x * 8 + 8];
                        for k in 0..3 {
                            let hv = u16::from_le_bytes([p[k * 2], p[k * 2 + 1]]);
                            out.data[at + x * 3 + k] = enc(hv);
                        }
                    }
                }
            }
            buf.unmap();
            x0 += cols;
        }
        y0 += rows;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert_eq!(f16_to_f32(0x7bff), 65504.0);
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
        assert_eq!(f16_to_f32(0x3555), 0.333_251_95);
    }

    #[test]
    fn regions_cover_whole_pixels() {
        let s = Shot::region((100, 50), Orientation::NORMAL, [10, 5, 74, 37]);
        assert_eq!(s.size, (64, 32));
        assert_eq!(s.centre, (42.0, 21.0));
        assert_eq!(s.zoom, 1.0);
    }
}
