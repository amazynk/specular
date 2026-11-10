//! The window's swapchain. We render in linear light into a half-float target and do the
//! sRGB encoding (with blue-noise dither) ourselves, so the swapchain is a plain UNORM format:
//! 10-bit when the platform offers it, 8-bit otherwise.

use std::sync::Arc;

use crate::gpu::Gpu;

pub struct WindowSurface {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    /// Bits per colour channel of the swapchain (8 or 10, 16 for scRGB); sets the dither
    /// amplitude.
    pub bits: u32,
    /// The SDR format and bits, restored when HDR output is switched off.
    sdr: (wgpu::TextureFormat, u32),
    /// The swapchain is HDR: half-float extended linear sRGB (scRGB, 1.0 = SDR white,
    /// values above it brighter, written linear by the compositor's final pass), or 10-bit
    /// BT.2100 PQ where the platform offers no scRGB (encoded by the final pass).
    pub hdr: bool,
}

/// The HDR swapchain configuration a surface offers, best first: scRGB (Windows, macOS
/// EDR, Vulkan drivers with `VK_EXT_swapchain_colorspace`), else 10-bit BT.2100 PQ (HDR10,
/// what HDR Wayland compositors and some drivers offer instead). `None` without either.
pub fn hdr_config(
    caps: &wgpu::SurfaceCapabilities,
) -> Option<(wgpu::TextureFormat, wgpu::SurfaceColorSpace, u32)> {
    use wgpu::{SurfaceColorSpace as Cs, SurfaceColorSpaces as Set, TextureFormat as F};
    [
        (
            F::Rgba16Float,
            Set::EXTENDED_SRGB_LINEAR,
            Cs::ExtendedSrgbLinear,
            16,
        ),
        (F::Rgb10a2Unorm, Set::BT2100_PQ, Cs::Bt2100Pq, 10),
    ]
    .into_iter()
    .find(|(f, set, ..)| caps.color_spaces(*f).contains(*set))
    .map(|(f, _, cs, bits)| (f, cs, bits))
}

/// What the display behind a surface reports about HDR (see [`WindowSurface::hdr_display`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HdrDisplay {
    /// The surface accepts an HDR swapchain ([`hdr_config`]: scRGB or BT.2100 PQ).
    pub supported: bool,
    /// The display says it can show HDR (EDR potential headroom above 1, an HDR mode, or a
    /// peak above SDR white). Steady, unlike `headroom`: decides "auto" HDR output.
    pub capable: bool,
    /// Brightest value it can show now, in units of SDR white; `None` when unknown (Linux,
    /// most Vulkan drivers).
    pub headroom: Option<f32>,
}

impl std::fmt::Debug for WindowSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowSurface")
            .field("format", &self.config.format)
            .finish()
    }
}

/// Outcome of [`WindowSurface::acquire`].
pub enum Frame {
    Ready(wgpu::SurfaceTexture),
    /// Nothing to draw into right now (minimised, timeout); try again on the next event.
    Skip,
}

impl WindowSurface {
    pub fn new(gpu: &Gpu, window: Arc<winit::window::Window>) -> Result<WindowSurface, String> {
        let size = window.inner_size();
        let surface = gpu
            .instance
            .create_surface(window)
            .map_err(|e| format!("cannot create surface: {e}"))?;
        let caps = surface.get_capabilities(&gpu.adapter);
        if caps.formats.is_empty() {
            return Err("the GPU cannot present to this window".into());
        }
        let preferred = [
            wgpu::TextureFormat::Rgb10a2Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ];
        let format = preferred
            .iter()
            .copied()
            .find(|f| caps.formats.contains(f))
            .unwrap_or_else(|| caps.formats[0].remove_srgb_suffix());
        let bits = if format == wgpu::TextureFormat::Rgb10a2Unorm {
            10
        } else {
            8
        };
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        // Metal has no Mailbox; macOS can A/B Fifo and Immediate (crate::macos).
        #[cfg(target_os = "macos")]
        let present_mode = crate::macos::present_mode(&caps, present_mode);
        let frame_latency = 2;
        #[cfg(target_os = "macos")]
        let frame_latency = crate::macos::frame_latency(frame_latency);
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: Default::default(),
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            desired_maximum_frame_latency: frame_latency,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&gpu.device, &config);
        Ok(WindowSurface {
            surface,
            config,
            bits,
            sdr: (format, bits),
            hdr: false,
        })
    }

    /// HDR facts for the display the window is on. Cheap enough to ask every frame (it
    /// changes with brightness and the display the window is on).
    pub fn hdr_display(&self, gpu: &Gpu) -> HdrDisplay {
        let caps = self.surface.get_capabilities(&gpu.adapter);
        let supported = hdr_config(&caps).is_some();
        let info = self.surface.display_hdr_info(&gpu.adapter);
        let headroom = info
            .tone_map_headroom()
            .filter(|h| h.is_finite() && *h >= 1.0);
        let capable = info
            .headroom
            .and_then(|h| h.potential)
            .is_some_and(|p| p > 1.01)
            || info.coarse.and_then(|c| c.high_dynamic_range) == Some(true)
            || headroom.is_some_and(|h| h > 1.01);
        HdrDisplay {
            supported,
            capable,
            headroom,
        }
    }

    /// Switches the swapchain between SDR (UNORM, sRGB-encoded by the compositor) and HDR
    /// ([`hdr_config`]: scRGB `Rgba16Float` + `ExtendedSrgbLinear`, else `Rgb10a2Unorm` +
    /// `Bt2100Pq`). Returns whether it is HDR afterwards: asking for HDR on a surface
    /// without either keeps SDR. The compositor must follow with
    /// [`crate::Compositor::set_output`] (which picks the PQ encoding from the format).
    pub fn set_hdr(&mut self, gpu: &Gpu, on: bool) -> bool {
        let config = if on {
            hdr_config(&self.surface.get_capabilities(&gpu.adapter))
        } else {
            None
        };
        let on = config.is_some();
        if on == self.hdr {
            return on;
        }
        let (format, color_space, bits) =
            config.unwrap_or((self.sdr.0, Default::default(), self.sdr.1));
        self.config.format = format;
        self.config.color_space = color_space;
        self.bits = bits;
        self.hdr = on;
        self.surface.configure(&gpu.device, &self.config);
        on
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        let (w, h) = (width.max(1), height.max(1));
        if (w, h) != (self.config.width, self.config.height) {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&gpu.device, &self.config);
        }
    }

    pub fn acquire(&mut self, gpu: &Gpu) -> Frame {
        for _ in 0..2 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) => return Frame::Ready(t),
                // Present this one; the resize event that follows reconfigures.
                wgpu::CurrentSurfaceTexture::Suboptimal(t) => return Frame::Ready(t),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&gpu.device, &self.config);
                }
                _ => return Frame::Skip,
            }
        }
        Frame::Skip
    }
}
