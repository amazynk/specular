//! macOS (Metal) specifics of the GPU layer. See docs/macos.md for what each choice needs measured.
//!
//! * Presentation: Metal offers only `Fifo` (vsync, `displaySyncEnabled = YES`) and `Immediate`
//!   (`displaySyncEnabled = NO`); there is no `Mailbox`. In a window, WindowServer composites
//!   the layer, so `Immediate` does not tear there and behaves like mailbox (newest frame
//!   wins); in native full screen the layer can go direct to the display and could tear.
//!   We keep `Fifo` by default; `SPECULAR_PRESENT=immediate` switches for A/B frame-pacing runs.
//! * Drawables: `CAMetalLayer.maximumDrawableCount` is wgpu's frame latency + 1 (2 or 3).
//!   `SPECULAR_FRAME_LATENCY=1` gives 2 drawables (one less frame of latency on ProMotion,
//!   risk of stalls in `nextDrawable`).
//! * ProMotion: with `Fifo` the layer presents at the display's current rate, up to 120 Hz;
//!   macOS raises the rate while frames keep coming. Nothing else is needed for 120 Hz.
//! * EDR: wgpu configures `wantsExtendedDynamicRangeContent` and the layer colour space from
//!   [`wgpu::SurfaceConfiguration::color_space`]; [`WindowSurface::edr_output`] picks the
//!   format and colour space the HDR work will switch to, [`WindowSurface::edr_headroom`]
//!   reads the live headroom.
//! * Texture compression: Apple GPUs (and Intel Macs) expose BC; Apple GPUs also ASTC. Both
//!   are requested when present, for the compressed display cache (DESIGN.md §12.2).

use crate::gpu::Gpu;
use crate::surface::WindowSurface;

/// `SPECULAR_PRESENT=immediate|fifo` (anything else: the default).
pub(crate) fn present_mode(
    caps: &wgpu::SurfaceCapabilities,
    default: wgpu::PresentMode,
) -> wgpu::PresentMode {
    let wanted = match std::env::var("SPECULAR_PRESENT").as_deref() {
        Ok("immediate") => wgpu::PresentMode::Immediate,
        Ok("fifo") => wgpu::PresentMode::Fifo,
        _ => return default,
    };
    if caps.present_modes.contains(&wanted) {
        wanted
    } else {
        default
    }
}

/// `SPECULAR_FRAME_LATENCY=1|2` (drawables = latency + 1); otherwise `default`.
pub(crate) fn frame_latency(default: u32) -> u32 {
    std::env::var("SPECULAR_FRAME_LATENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| (1..=2).contains(n))
        .unwrap_or(default)
}

/// Optional features worth having on a Mac: block-compressed textures for the display cache.
pub(crate) fn wanted_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features()
        & (wgpu::Features::TEXTURE_COMPRESSION_BC | wgpu::Features::TEXTURE_COMPRESSION_ASTC)
}

/// A swapchain format and colour space for EDR output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdrOutput {
    pub format: wgpu::TextureFormat,
    pub color_space: wgpu::SurfaceColorSpace,
}

/// The best EDR configuration `caps` allows: half-float extended linear sRGB (values above 1.0
/// are brighter than SDR white; the compositor maps the primaries), else half-float extended
/// Display P3, else 10-bit PQ. `None` when the surface has no HDR colour space.
pub fn pick_edr_output(caps: &wgpu::SurfaceCapabilities) -> Option<EdrOutput> {
    use wgpu::{SurfaceColorSpace as Cs, SurfaceColorSpaces as Set, TextureFormat as F};
    let order = [
        (
            F::Rgba16Float,
            Set::EXTENDED_SRGB_LINEAR,
            Cs::ExtendedSrgbLinear,
        ),
        (
            F::Rgba16Float,
            Set::EXTENDED_DISPLAY_P3,
            Cs::ExtendedDisplayP3,
        ),
        (F::Rgb10a2Unorm, Set::BT2100_PQ, Cs::Bt2100Pq),
    ];
    order
        .into_iter()
        .find(|(f, set, _)| caps.color_spaces(*f).contains(*set))
        .map(|(format, _, color_space)| EdrOutput {
            format,
            color_space,
        })
}

impl WindowSurface {
    /// The EDR output this window's surface supports (see [`pick_edr_output`]). Switching to
    /// it is a reconfigure with `format` and `color_space` set, plus a final pass that writes
    /// linear values with 1.0 = SDR white instead of encoding to sRGB.
    pub fn edr_output(&self, gpu: &Gpu) -> Option<EdrOutput> {
        pick_edr_output(&self.surface.get_capabilities(&gpu.adapter))
    }

    /// The display's current EDR headroom (1.0 = no HDR headroom right now), for tone mapping
    /// gain-map and HDR images. Main thread only; `None` when unknown.
    pub fn edr_headroom(&self, gpu: &Gpu) -> Option<f32> {
        self.surface
            .display_hdr_info(&gpu.adapter)
            .headroom
            .and_then(|h| h.current)
    }
}
