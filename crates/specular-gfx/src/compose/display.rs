//! The display side of colour management in the final pass: linear BT.709 → display device
//! values through a [`specular_color::Display`] (matrix + inverse tone curves, or a 3D LUT).
//! SDR output only: an scRGB swapchain is colour-managed by the system compositor.

use specular_color::gpu::{CUBE_SIZE, CURVE_SIZE};
use specular_color::{Display, DisplayKind};
use wgpu::util::DeviceExt;

use super::Compositor;
use crate::gpu::Gpu;

/// `FinalParams` (final.wgsl) without a display transform or CVD simulation.
pub(super) fn params(lsb: f32) -> [f32; 28] {
    let mut p = [0f32; 28];
    p[0] = lsb;
    p[2] = 1.0;
    p
}

fn lut_texture(gpu: &Gpu, (w, h, d): (u32, u32, u32), data: &[u16]) -> wgpu::TextureView {
    gpu.device
        .create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("display lut"),
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
        )
        .create_view(&Default::default())
}

/// A 1×1×1 placeholder (mode 0 never samples it).
pub(super) fn identity_lut(gpu: &Gpu) -> wgpu::TextureView {
    lut_texture(gpu, (1, 1, 1), &[0, 0, 0, 0x3c00])
}

impl Compositor {
    /// Sets the display transform of the final pass (SDR output; ignored while the output is
    /// scRGB). [`Display::srgb`] restores plain sRGB encoding.
    pub fn set_display(&mut self, gpu: &Gpu, display: &Display) {
        let (mode, matrix, n, view) = match &display.kind {
            DisplayKind::Identity => (0.0, [0f32; 9], 1, None),
            DisplayKind::Curves { matrix, data } => (
                1.0,
                *matrix,
                CURVE_SIZE,
                Some(lut_texture(gpu, (CURVE_SIZE as u32, 1, 1), data)),
            ),
            DisplayKind::Cube { data } => {
                let n = CUBE_SIZE as u32;
                (
                    2.0,
                    [0f32; 9],
                    CUBE_SIZE,
                    Some(lut_texture(gpu, (n, n, n), data)),
                )
            }
        };
        self.display_lut = view.unwrap_or_else(|| identity_lut(gpu));
        self.rebind_final(&gpu.device);
        let m = &matrix;
        let tail: [f32; 13] = [
            mode, m[0], m[1], m[2], n as f32, m[3], m[4], m[5], 0.0, m[6], m[7], m[8], 0.0,
        ];
        gpu.queue
            .write_buffer(&self.final_buf, 12, bytemuck::cast_slice(&tail));
    }

    /// Simulates a colour-vision deficiency over the whole frame: `matrix` (rows, linear
    /// BT.709, e.g. `specular_inspect::Cvd::matrix`) is applied in the final pass before the
    /// display transform and encoding, in SDR and scRGB output alike. `None` turns it off.
    /// Display only: exports do not go through the compositor.
    pub fn set_cvd(&mut self, gpu: &Gpu, matrix: Option<[[f32; 3]; 3]>) {
        let mut rows = [0f32; 12];
        if let Some(m) = matrix {
            for (r, row) in m.iter().enumerate() {
                rows[r * 4..r * 4 + 3].copy_from_slice(row);
            }
            rows[3] = 1.0;
        }
        self.cvd = matrix;
        gpu.queue
            .write_buffer(&self.final_buf, 64, bytemuck::cast_slice(&rows));
    }

    /// The simulation [`Compositor::set_cvd`] set.
    pub fn cvd(&self) -> Option<[[f32; 3]; 3]> {
        self.cvd
    }
}

#[cfg(test)]
mod tests {
    use specular_color::math::{f16_bits, f16_to_f32, linear_to_srgb};
    use specular_color::{Display, DisplayKind, Intent, profiles};

    use super::*;

    /// Runs the final pass over `px` (linear BT.709) into a half-float target and reads back
    /// the encoded values (16-bit dither: negligible).
    fn final_pass(gpu: &Gpu, display: &Display, px: &[[f32; 3]]) -> Vec<[f32; 3]> {
        final_pass_cvd(gpu, display, None, px)
    }

    fn final_pass_cvd(
        gpu: &Gpu,
        display: &Display,
        cvd: Option<[[f32; 3]; 3]>,
        px: &[[f32; 3]],
    ) -> Vec<[f32; 3]> {
        let w = px.len() as u32;
        let fmt = wgpu::TextureFormat::Rgba16Float;
        let mut comp = Compositor::new(gpu, fmt, 16, w, 1);
        comp.set_display(gpu, display);
        comp.set_cvd(gpu, cvd);
        let texels: Vec<u16> = px
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 1.0].map(f16_bits))
            .collect();
        gpu.queue.write_texture(
            comp.comp.texture.as_image_copy(),
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 8),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: w,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("final test"),
            size: wgpu::Extent3d {
                width: w,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: fmt,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row = (w * 8).div_ceil(256) * 256;
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("final readback"),
            size: row as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        comp.finish(&mut enc, &target.create_view(&Default::default()));
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().expect("mapped");
        (0..w as usize)
            .map(|x| {
                let c = |i: usize| {
                    let o = x * 8 + 2 * i;
                    f16_to_f32(u16::from_le_bytes([data[o], data[o + 1]]))
                };
                [c(0), c(1), c(2)]
            })
            .collect()
    }

    fn colours() -> Vec<[f32; 3]> {
        let mut v = vec![
            [0.0; 3],
            [1.0; 3],
            [0.18; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        v.push([1.2, -0.05, -0.02]); // wide (Display P3 red)
        for r in [0.02f32, 0.3, 0.8] {
            for g in [0.05f32, 0.5, 0.95] {
                for b in [0.01f32, 0.4, 0.7] {
                    v.push([r, g, b]);
                }
            }
        }
        v
    }

    /// The final pass matches `Display::apply` for a matrix display (Display P3) and for a
    /// 3D table, and stays plain sRGB without a transform. Skips without a GPU.
    #[test]
    fn final_pass_matches_cpu_reference() {
        let Ok(gpu) = Gpu::new() else {
            eprintln!("no GPU, skipped");
            return;
        };
        let px = colours();
        // An identity cube: grid point i holds i/(n-1) on each axis.
        let n = CUBE_SIZE;
        let mut cube = Vec::with_capacity(n * n * n * 4);
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let v = |i: usize| f16_bits(i as f32 / (n - 1) as f32);
                    cube.extend_from_slice(&[v(r), v(g), v(b), f16_bits(1.0)]);
                }
            }
        }
        let cube = Display {
            kind: DisplayKind::Cube { data: cube },
            ..Display::srgb()
        };
        let p3 = Display::for_profile(&profiles::display_p3(), Intent::Relative, true);
        assert!(matches!(p3.kind, DisplayKind::Curves { .. }));
        for (name, d) in [("sRGB", Display::srgb()), ("P3", p3), ("cube", cube)] {
            let got = final_pass(&gpu, &d, &px);
            let mut max = 0f32;
            for (g, p) in got.iter().zip(&px) {
                let want = d.apply(*p);
                for c in 0..3 {
                    max = max.max((g[c] - want[c]).abs());
                }
            }
            eprintln!(
                "final pass {name}: max |GPU − CPU| {:.5} (8-bit LSB 0.0039)",
                max
            );
            assert!(max < 0.002, "{name}: {max}");
            if name == "sRGB" {
                assert!((got[2][0] - linear_to_srgb(0.18)).abs() < 0.002);
            }
        }
    }

    /// The CVD simulation applies its matrix in linear light before encoding, whatever the
    /// display transform, and off restores the plain pass. Skips without a GPU.
    #[test]
    fn final_pass_simulates_colour_vision() {
        let Ok(gpu) = Gpu::new() else {
            eprintln!("no GPU, skipped");
            return;
        };
        // Machado et al. 2009 deuteranopia (specular_inspect::cvd::DEUTERANOPIA).
        let m = [
            [0.367322, 0.860646, -0.227968],
            [0.280085, 0.672501, 0.047413],
            [-0.011820, 0.042940, 0.968881],
        ];
        let px = colours();
        let got = final_pass_cvd(&gpu, &Display::srgb(), Some(m), &px);
        let mut max = 0f32;
        for (g, p) in got.iter().zip(&px) {
            let sim = m.map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2]);
            let want = Display::srgb().apply(sim);
            for c in 0..3 {
                max = max.max((g[c] - want[c]).abs());
            }
        }
        eprintln!("final pass CVD: max |GPU − CPU| {max:.5}");
        assert!(max < 0.002, "{max}");
        // Pure red and pure green both come out as yellows (red ≈ green) for a deuteranope.
        let (r, g) = (got[3], got[4]);
        assert!(
            (r[0] - r[1]).abs() < 0.1 && (g[0] - g[1]).abs() < 0.1,
            "{r:?} {g:?}"
        );
        let off = final_pass_cvd(&gpu, &Display::srgb(), None, &px);
        assert!((off[3][0] - 1.0).abs() < 0.002 && off[3][1].abs() < 0.002);
    }
}
