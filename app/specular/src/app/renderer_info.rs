//! Renderer information (About › Renderer info, Settings › Advanced): the facts gathered
//! from wgpu and the app ([`specular_widgets::renderer_info`] words them). Copy report puts
//! the text on the clipboard only when pressed.

use specular_decode::optional::{self, State};
use specular_gfx::wgpu;
use specular_widgets::renderer_info::{self, AdapterKind, DecoderFact, HdrFacts, RendererFacts};

use crate::app::App;

/// The limits the renderer depends on (tile cache, compute passes), by wgpu's name.
pub(crate) fn limits(l: &wgpu::Limits) -> Vec<(&'static str, u64)> {
    vec![
        (
            "max_texture_dimension_2d",
            u64::from(l.max_texture_dimension_2d),
        ),
        (
            "max_texture_dimension_3d",
            u64::from(l.max_texture_dimension_3d),
        ),
        (
            "max_texture_array_layers",
            u64::from(l.max_texture_array_layers),
        ),
        ("max_buffer_size", l.max_buffer_size),
        (
            "max_storage_buffer_binding_size",
            l.max_storage_buffer_binding_size,
        ),
        ("max_bind_groups", u64::from(l.max_bind_groups)),
        (
            "max_compute_workgroup_size_x",
            u64::from(l.max_compute_workgroup_size_x),
        ),
        (
            "max_compute_invocations_per_workgroup",
            u64::from(l.max_compute_invocations_per_workgroup),
        ),
    ]
}

pub(crate) fn kind(t: wgpu::DeviceType) -> AdapterKind {
    match t {
        wgpu::DeviceType::DiscreteGpu => AdapterKind::Discrete,
        wgpu::DeviceType::IntegratedGpu => AdapterKind::Integrated,
        wgpu::DeviceType::VirtualGpu => AdapterKind::Virtual,
        wgpu::DeviceType::Cpu => AdapterKind::Software,
        wgpu::DeviceType::Other => AdapterKind::Other,
    }
}

/// Wayland, X11, Windows or macOS.
pub(crate) fn display_server() -> String {
    if cfg!(windows) {
        "Windows".into()
    } else if cfg!(target_os = "macos") {
        "macOS".into()
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty()) {
        "Wayland".into()
    } else {
        "X11".into()
    }
}

/// The optional decoders, as Settings › Optional decoders lists them.
pub(crate) fn decoders() -> Vec<(String, DecoderFact)> {
    optional::entries()
        .into_iter()
        .map(|e| {
            let name = specular_i18n::dot_join(&[e.id.format_name(), e.id.decoder_name()]);
            let s = match e.state {
                State::Ready { version, .. } => DecoderFact::Ready(version),
                State::Off { .. } => DecoderFact::Off,
                State::Broken { why, .. } => DecoderFact::Broken(why),
                State::NotFound => DecoderFact::NotFound,
                State::Refused => DecoderFact::Refused,
                State::Planned => DecoderFact::Planned,
                State::Checking => DecoderFact::Checking,
            };
            (name, s)
        })
        .collect()
}

fn names<T: std::fmt::Debug>(v: &[T]) -> Vec<String> {
    v.iter().map(|x| format!("{x:?}")).collect()
}

/// Device features as their names ("TEXTURE_COMPRESSION_BC").
pub(crate) fn features(f: wgpu::Features) -> Vec<String> {
    f.iter_names().map(|(n, _)| n.to_owned()).collect()
}

impl App {
    /// The facts now (no GPU yet: the adapter rows say Unknown).
    fn renderer_facts(&self) -> RendererFacts {
        let ram_cache = match self.settings.performance.ram_cache {
            0 => self.hw.ram_cache_budget(),
            mib => (mib as u64) << 20,
        };
        let vram_cache = match self.settings.performance.vram_cache {
            0 => 768u64 << 20,
            mb => (mb as u64) << 20,
        };
        let p = &self.settings.performance;
        let mut f = RendererFacts {
            display_server: display_server(),
            ram_total: self.hw.ram_total,
            ram_available: specular_core::available_memory().unwrap_or(self.hw.ram_available),
            decode_budget: specular_core::budget::free(),
            ram_cache,
            vram_cache,
            display_cache: p
                .display_cache
                .then_some((p.display_cache_size.max(0) as u64) << 20),
            decoders: decoders(),
            ..RendererFacts::default()
        };
        if let Some(g) = &self.gfx {
            let i = &g.gpu.info;
            f.adapter = i.name.clone();
            f.kind = kind(i.device_type);
            f.backend = format!("{:?}", i.backend);
            f.driver = i.driver.clone();
            f.driver_info = i.driver_info.clone();
            f.vendor_id = i.vendor;
            f.device_id = i.device;
            f.limits = limits(&g.gpu.limits);
            f.features = features(g.gpu.device.features());
            let caps = g.surface.surface.get_capabilities(&g.gpu.adapter);
            f.surface_format = Some(format!("{:?}", g.surface.config.format));
            f.surface_formats = names(&caps.formats);
            f.present_mode = Some(format!("{:?}", g.surface.config.present_mode));
            f.present_modes = names(&caps.present_modes);
            let d = g.surface.hdr_display(&g.gpu);
            f.hdr = HdrFacts {
                supported: d.supported,
                capable: d.capable,
                on: g.surface.hdr,
                headroom: d.headroom,
            };
        }
        f
    }

    /// Opens the renderer information sheet.
    pub(crate) fn show_renderer_info(&mut self) {
        let report = renderer_info::report(&self.renderer_facts());
        self.chrome.show_report(report);
        self.redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_from_wgpu_types() {
        let l = limits(&wgpu::Limits::downlevel_defaults());
        assert!(
            l.iter()
                .any(|(n, v)| *n == "max_texture_dimension_2d" && *v >= 2048)
        );
        assert_eq!(kind(wgpu::DeviceType::Cpu), AdapterKind::Software);
        let f = features(wgpu::Features::TEXTURE_COMPRESSION_BC);
        assert_eq!(f, vec!["TEXTURE_COMPRESSION_BC".to_owned()]);
        assert!(!display_server().is_empty());
        // Every optional decoder is listed, whatever its state.
        assert_eq!(decoders().len(), optional::OptionalId::ALL.len());
    }
}
