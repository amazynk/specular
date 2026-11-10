//! YCbCr → RGB on the GPU or the CPU: the path selector's [`Task::YcbcrToRgb`]
//! (DESIGN.md §12.3.1).
//!
//! The GPU path uploads the planes as they are (1.5 bytes per pixel for 4:2:0) and converts
//! in the shader; the CPU path converts first ([`crate::cpu_colour`]) and uploads 3 bytes per
//! pixel. On the machines measured the GPU path wins at every size; on a software rasteriser
//! (llvmpipe, WARP) the "GPU" is the CPU and the CPU path wins.
//!
//! * **Correctness gate.** Before the GPU path is used, a small test image is converted both
//!   ways, drawn and read back, and must match ([`ImageRenderer::verify_colour`]). This runs
//!   once per machine fingerprint, on the first YCbCr upload (a few milliseconds).
//! * **Calibration.** [`ImageRenderer::calibrate_colour_step`] times both paths end to end
//!   (upload and conversion until the GPU is done) at two sizes, one short step per call, from
//!   the app's idle handler.
//! * **Learning.** Real uploads don't wait for the GPU, so they record their CPU time only;
//!   the wall times come from calibration.

use specular_core::paths::{Choice, Engine, Request, Task, Trust, selector};
use specular_core::{Orientation, PixelLayout, Raster, RasterPlane, hdr};
use specular_gfx::{Gpu, HDR, wgpu};

use super::{GpuImage, ImageRenderer, RenderOptions};
use crate::view::{PxRect, View};

/// Image id of the renderer's own test images (never a real image's).
const TEST_ID: u64 = u64::MAX - 0x5eed;

/// Largest difference between the two paths' output, in linear light (1 = reference white).
/// Both round to 8 bits at different points: about one code value apart (≤ 0.01 linear).
const MAX_DIFF: f32 = 0.03;

/// Calibration sizes (bucket 1 for the setup cost, bucket 3 for the rate).
const SIZES: [(u32, u32); 2] = [(1024, 768), (3072, 2048)];
/// Runs per size and path; the first is a warm-up and not recorded.
const REPS: usize = 3;
/// Total steps of [`ImageRenderer::calibrate_colour_step`].
pub const COLOUR_STEPS: usize = SIZES.len() * REPS * 2;

/// [`Task::YcbcrToRgb`]'s variant of a YCbCr raster: 0 4:2:0, 1 4:2:2, 2 4:4:4, 3 other.
pub fn ycbcr_variant(r: &Raster) -> u8 {
    let (Some(y), Some(c)) = (r.planes.first(), r.planes.get(1)) else {
        return 3;
    };
    match (
        y.width.div_ceil(c.width.max(1)),
        y.height.div_ceil(c.height.max(1)),
    ) {
        (2, 2) => 0,
        (2, 1) => 1,
        (1, 1) => 2,
        _ => 3,
    }
}

/// A synthetic 4:2:0 image with smooth gradients, saturated patches and fine detail.
pub fn sample(w: u32, h: u32) -> Raster {
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let plane = |pw: u32, ph: u32, div: u8, f: &dyn Fn(u32, u32) -> u8| RasterPlane {
        width: pw,
        height: ph,
        stride: pw as usize,
        data: (0..ph)
            .flat_map(|y| (0..pw).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect(),
        h_div: div,
        v_div: div,
    };
    let y = plane(w, h, 1, &|x, y| {
        let g = (x * 220 / w.max(1) + 16) as u8;
        if (x / 7 + y / 5) % 9 == 0 { 255 - g } else { g }
    });
    let cb = plane(cw, ch, 2, &|x, y| {
        ((y * 255 / ch.max(1)) as u8) ^ ((x / 16) as u8 & 0x20)
    });
    let cr = plane(cw, ch, 2, &|x, _| (x * 255 / cw.max(1)) as u8);
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::YCbCr,
        planes: vec![y, cb, cr],
        truncated: false,
    }
}

impl ImageRenderer {
    /// The colour path for uploading `r`: `None` unless it is YCbCr. The first call on a
    /// machine runs the correctness gate.
    pub(super) fn colour_path(&mut self, gpu: &Gpu, r: &Raster) -> Option<Choice> {
        if r.layout != PixelLayout::YCbCr || r.planes.len() != 3 {
            return None;
        }
        let px = u64::from(r.width) * u64::from(r.height);
        let sel = selector();
        if let Some(path) = self.colour_force {
            return Some(sel.calibration(Task::YcbcrToRgb, path, px, ycbcr_variant(r)));
        }
        sel.set_available(Task::YcbcrToRgb, Engine::Gpu);
        if sel.trust(Task::YcbcrToRgb, 1) == Trust::Unverified && !self.colour_checked {
            self.colour_checked = true;
            self.verify_colour(gpu);
        }
        Some(sel.choose(Task::YcbcrToRgb, &Request::new(px, ycbcr_variant(r))))
    }

    /// Records an upload's CPU time (from `t0`) for its colour path. Calibration's own uploads
    /// are timed by [`Self::calibrate_colour_step`].
    pub(super) fn colour_done(&self, colour: Option<&Choice>, t0: std::time::Instant) {
        if let Some(c) = colour
            && self.colour_force.is_none()
        {
            let us = t0.elapsed().as_secs_f32() * 1e6;
            selector().record_cpu(c.task, c.path, c.key, c.mpx, us);
        }
    }

    /// The correctness gate: a small image converted on both paths must look the same.
    /// Records the result with the selector and returns it.
    pub fn verify_colour(&mut self, gpu: &Gpu) -> bool {
        let r = sample(96, 64);
        let mut out = [Vec::new(), Vec::new()];
        for (path, o) in out.iter_mut().enumerate() {
            self.colour_force = Some(path);
            let mut img = self.create_image(gpu, TEST_ID, r.width, r.height, None);
            self.upload_overview(gpu, &mut img, &r);
            *o = read_back(gpu, self, &img);
            self.release_image(TEST_ID);
        }
        self.colour_force = None;
        let diff = out[0]
            .iter()
            .zip(&out[1])
            .flat_map(|(a, b)| (0..3).map(move |i| (a[i] - b[i]).abs()))
            .fold(0.0f32, f32::max);
        let ok = out[0].len() == out[1].len() && !out[0].is_empty() && diff <= MAX_DIFF;
        specular_log::debug!(target: specular_log::RENDER,
            "colour paths: GPU and CPU conversion differ by at most {diff:.4}");
        selector().verified(Task::YcbcrToRgb, Engine::Gpu, ok);
        ok
    }

    /// One step of calibrating the colour paths (step `0..COLOUR_STEPS`): one upload on one
    /// path, timed until the GPU is done. Each takes a few to a few tens of milliseconds.
    pub fn calibrate_colour_step(&mut self, gpu: &Gpu, step: usize) {
        if step >= COLOUR_STEPS {
            return;
        }
        let (size, rest) = (step / (REPS * 2), step % (REPS * 2));
        let (rep, path) = (rest / 2, rest % 2);
        let (w, h) = SIZES[size];
        if self
            .colour_sample
            .as_ref()
            .is_none_or(|s| (s.width, s.height) != (w, h))
        {
            self.colour_sample = Some(sample(w, h));
        }
        let Some(r) = self.colour_sample.take() else {
            return;
        };
        if path == 1 && selector().trust(Task::YcbcrToRgb, 1) != Trust::Trusted {
            // Not checked yet, or failed: nothing to time.
            self.colour_sample = Some(r);
            return;
        }
        self.colour_force = Some(path);
        let mut img = self.create_image(gpu, TEST_ID, w, h, None);
        let t0 = std::time::Instant::now();
        self.upload_overview(gpu, &mut img, &r);
        let cpu = t0.elapsed().as_secs_f32() * 1e6;
        let _ = gpu.wait();
        let wall = t0.elapsed().as_secs_f32() * 1e6;
        self.colour_force = None;
        drop(img);
        self.release_image(TEST_ID);
        if rep > 0 {
            let sel = selector();
            let c = sel.calibration(
                Task::YcbcrToRgb,
                path,
                u64::from(w) * u64::from(h),
                ycbcr_variant(&r),
            );
            sel.start(&c).finish_with(wall, Some(cpu));
        }
        self.colour_sample = (step + 1 < COLOUR_STEPS).then_some(r);
    }
}

/// Draws `img` 1:1 and reads the half-float pixels back.
fn read_back(gpu: &Gpu, renderer: &mut ImageRenderer, img: &GpuImage) -> Vec<[f32; 4]> {
    let (w, h) = (img.width, img.height);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("colour check target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
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
    let view = View::fixed(
        (w, h),
        Orientation::NORMAL,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: w as f32,
            h: h as f32,
        },
        1.0,
        (w as f32 * 0.5, h as f32 * 0.5),
    );
    let opts = RenderOptions {
        hq: false,
        nearest_from: 0.5,
        ..RenderOptions::default()
    };
    let row = (w * 8).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("colour check readback"),
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    renderer.render(gpu, &mut enc, &tv, (w, h), Some(img), &view, &opts);
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.wait();
    let Ok(data) = buf.slice(..).get_mapped_range() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let o = (y * row + x * 8) as usize;
            let c = |i: usize| {
                hdr::f16_to_f32(u16::from_le_bytes([data[o + 2 * i], data[o + 2 * i + 1]]))
            };
            out.push([c(0), c(1), c(2), c(3)]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants() {
        assert_eq!(ycbcr_variant(&sample(33, 17)), 0);
        let mut r = sample(32, 16);
        r.planes[1].height = 16;
        assert_eq!(ycbcr_variant(&r), 1);
        r.planes[1].width = 32;
        assert_eq!(ycbcr_variant(&r), 2);
    }

    #[test]
    fn gpu_and_cpu_conversion_agree_and_calibrate() {
        let Some(gpu) = super::super::hdr_tests::gpu() else {
            return;
        };
        let mut renderer = ImageRenderer::new(&gpu, 64 << 20);
        assert!(renderer.verify_colour(&gpu));
        for step in 0..COLOUR_STEPS {
            renderer.calibrate_colour_step(&gpu, step);
        }
        let key = selector()
            .calibration(Task::YcbcrToRgb, 1, 3072 * 2048, 0)
            .key;
        assert!(selector().samples(Task::YcbcrToRgb, 1, key) >= 2);
        assert!(selector().samples(Task::YcbcrToRgb, 0, key) >= 2);
        assert!(renderer.colour_sample.is_none());
        // `--nocapture`: this machine's measurements.
        eprintln!("{}", selector().to_table());
    }
}
