//! Renderer information (About and Settings › Advanced): the GPU adapter, backend and
//! driver, the limits and features the renderer asked for, the window's formats and present
//! mode, HDR, the memory budgets and the optional decoders present. Shown in the hardware
//! report's sheet with its Copy button; nothing here touches the network or the clipboard.
//!
//! The app gathers [`RendererFacts`] from wgpu and its own state; [`report`] words them.

use specular_i18n::{ByteUnits, I18n, isolate_ltr, t};

use crate::{HwReport, ReportRow, ReportSection, Tone};

/// The setting action that opens it (About's button sends it too).
pub const RENDERER_INFO: &str = "advanced.renderer_info";

/// What kind of adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AdapterKind {
    Discrete,
    Integrated,
    Virtual,
    Software,
    #[default]
    Other,
}

/// Where HDR output stands.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HdrFacts {
    /// The surface offers an HDR format.
    pub supported: bool,
    /// The display reports HDR headroom.
    pub capable: bool,
    /// HDR output is on.
    pub on: bool,
    pub headroom: Option<f32>,
}

/// An optional decoder's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecoderFact {
    Ready(Option<String>),
    Off,
    NotFound,
    Broken(String),
    Refused,
    Planned,
    Checking,
}

/// The renderer as the app found it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RendererFacts {
    pub adapter: String,
    pub kind: AdapterKind,
    /// Vulkan, Metal, DX12, GL.
    pub backend: String,
    pub driver: String,
    pub driver_info: String,
    pub vendor_id: u32,
    pub device_id: u32,
    /// Wayland, X11, Windows, macOS.
    pub display_server: String,
    /// The device limits the renderer runs with, by wgpu's name.
    pub limits: Vec<(&'static str, u64)>,
    /// The device features enabled.
    pub features: Vec<String>,
    /// The window's format in use, and those the surface offers.
    pub surface_format: Option<String>,
    pub surface_formats: Vec<String>,
    pub present_mode: Option<String>,
    pub present_modes: Vec<String>,
    pub hdr: HdrFacts,
    pub ram_total: u64,
    pub ram_available: u64,
    /// What decoding may reserve now (80 % of the free memory); `None`: not known.
    pub decode_budget: Option<u64>,
    pub ram_cache: u64,
    pub vram_cache: u64,
    /// The display cache's size on disk (`None`: off).
    pub display_cache: Option<u64>,
    /// Optional decoders: (format and decoder name, state).
    pub decoders: Vec<(String, DecoderFact)>,
}

fn size(b: u64) -> String {
    I18n::global().localizer().bytes(b, ByteUnits::Binary)
}

fn integer(n: u64) -> String {
    I18n::global()
        .localizer()
        .integer(i64::try_from(n).unwrap_or(i64::MAX))
}

fn chips(v: &[String]) -> Vec<(String, bool)> {
    v.iter().map(|s| (s.clone(), true)).collect()
}

/// A row of technical names (formats, modes, features): chips, or "None".
fn list_row(label: String, v: &[String]) -> ReportRow {
    if v.is_empty() {
        ReportRow::new(label, t!("hw-report-none")).tone(Tone::Muted)
    } else {
        ReportRow::new(label, "").chips(chips(v))
    }
}

/// The report: adapter, limits and features, display, memory, optional decoders.
pub fn report(f: &RendererFacts) -> HwReport {
    let unknown = || t!("renderer-unknown");
    let or_unknown = |s: &str| {
        if s.is_empty() {
            unknown()
        } else {
            isolate_ltr(s)
        }
    };
    let kind = match f.kind {
        AdapterKind::Discrete => t!("hw-report-gpu-discrete"),
        AdapterKind::Integrated => t!("hw-report-gpu-integrated"),
        AdapterKind::Virtual => t!("hw-report-gpu-virtual"),
        AdapterKind::Software => t!("hw-report-gpu-software"),
        AdapterKind::Other => t!("hw-report-gpu-other"),
    };
    let mut adapter = vec![
        ReportRow::new(t!("hw-report-adapter"), or_unknown(&f.adapter)).tone(Tone::Strong),
        ReportRow::new(t!("hw-report-gpu-type"), kind),
        ReportRow::new(t!("hw-report-graphics-api"), or_unknown(&f.backend)),
        ReportRow::new(t!("hw-report-driver"), or_unknown(&f.driver))
            .detail(isolate_ltr(&f.driver_info)),
        ReportRow::new(
            t!("renderer-ids"),
            isolate_ltr(&format!("{:04x}:{:04x}", f.vendor_id, f.device_id)),
        ),
        ReportRow::new(t!("renderer-display-server"), or_unknown(&f.display_server)),
    ];
    if f.kind == AdapterKind::Software {
        adapter[1] = adapter[1].clone().tone(Tone::Warn);
    }
    let mut limits: Vec<ReportRow> = f
        .limits
        .iter()
        .map(|(name, v)| ReportRow::new(isolate_ltr(name), integer(*v)))
        .collect();
    limits.push(list_row(t!("renderer-features"), &f.features));

    let hdr = &f.hdr;
    let hdr_row = if hdr.on {
        let head = hdr.headroom.map_or_else(unknown, |h| {
            I18n::global().localizer().number_fixed(f64::from(h), 1)
        });
        ReportRow::new(t!("renderer-hdr"), t!("renderer-hdr-on", headroom = head)).tone(Tone::Good)
    } else if hdr.capable && hdr.supported {
        ReportRow::new(t!("renderer-hdr"), t!("renderer-hdr-off"))
    } else {
        ReportRow::new(t!("renderer-hdr"), t!("renderer-hdr-unavailable")).tone(Tone::Muted)
    };
    let display = vec![
        ReportRow::new(
            t!("renderer-surface-format"),
            f.surface_format
                .as_deref()
                .map_or_else(unknown, isolate_ltr),
        ),
        list_row(t!("renderer-surface-formats"), &f.surface_formats),
        ReportRow::new(
            t!("renderer-present-mode"),
            f.present_mode.as_deref().map_or_else(unknown, isolate_ltr),
        ),
        list_row(t!("renderer-present-modes"), &f.present_modes),
        hdr_row,
    ];

    let memory = vec![
        ReportRow::new(
            t!("hw-report-memory"),
            t!(
                "hw-report-memory-value",
                total = size(f.ram_total),
                available = size(f.ram_available)
            ),
        ),
        ReportRow::new(
            t!("renderer-decode-budget"),
            f.decode_budget.map_or_else(unknown, size),
        )
        .detail(t!("renderer-decode-budget-detail")),
        ReportRow::new(t!("hw-report-ram-cache"), size(f.ram_cache)),
        ReportRow::new(t!("hw-report-vram-cache"), size(f.vram_cache)),
        ReportRow::new(
            t!("renderer-display-cache"),
            f.display_cache
                .map_or_else(|| t!("decoders-state-off"), size),
        ),
    ];

    let decoders = if f.decoders.is_empty() {
        vec![
            ReportRow::new(t!("renderer-section-decoders"), t!("hw-report-none")).tone(Tone::Muted),
        ]
    } else {
        f.decoders
            .iter()
            .map(|(name, s)| {
                let (v, tone) = match s {
                    DecoderFact::Ready(_) => (t!("decoders-state-found"), Tone::Good),
                    DecoderFact::Off => (t!("decoders-state-off"), Tone::Muted),
                    DecoderFact::NotFound => (t!("decoders-state-not-found"), Tone::Muted),
                    DecoderFact::Broken(_) => (t!("decoders-state-broken"), Tone::Warn),
                    DecoderFact::Refused => (t!("decoders-state-refused"), Tone::Muted),
                    DecoderFact::Planned => (t!("decoders-state-planned"), Tone::Muted),
                    DecoderFact::Checking => (t!("decoders-state-checking"), Tone::Muted),
                };
                let row = ReportRow::new(isolate_ltr(name), v).tone(tone);
                match s {
                    DecoderFact::Broken(why) => row.detail(why.clone()),
                    DecoderFact::Ready(Some(ver)) => row.detail(isolate_ltr(ver)),
                    _ => row,
                }
            })
            .collect()
    };

    let section = |title: String, rows: Vec<ReportRow>| ReportSection {
        title,
        rows,
        ..ReportSection::default()
    };
    HwReport {
        title: Some(t!("renderer-title")),
        sections: vec![
            section(t!("renderer-section-adapter"), adapter),
            section(t!("renderer-section-limits"), limits),
            section(t!("renderer-section-display"), display),
            section(t!("renderer-section-memory"), memory),
            section(t!("renderer-section-decoders"), decoders),
        ],
    }
}

/// Stable made-up facts (screenshot tests, and the app's tests).
pub fn sample() -> RendererFacts {
    RendererFacts {
        adapter: "Example GPU 7800".into(),
        kind: AdapterKind::Discrete,
        backend: "Vulkan".into(),
        driver: "Example driver".into(),
        driver_info: "25.2.1".into(),
        vendor_id: 0x1002,
        device_id: 0x747e,
        display_server: "Wayland".into(),
        limits: vec![
            ("max_texture_dimension_2d", 16384),
            ("max_texture_array_layers", 2048),
            ("max_buffer_size", 1 << 31),
            ("max_storage_buffer_binding_size", 1 << 30),
            ("max_compute_workgroup_size_x", 1024),
        ],
        features: vec!["TEXTURE_COMPRESSION_BC".into(), "FLOAT32_FILTERABLE".into()],
        surface_format: Some("Rgb10a2Unorm".into()),
        surface_formats: vec![
            "Bgra8Unorm".into(),
            "Rgb10a2Unorm".into(),
            "Rgba16Float".into(),
        ],
        present_mode: Some("Mailbox".into()),
        present_modes: vec!["Fifo".into(), "Mailbox".into(), "Immediate".into()],
        hdr: HdrFacts {
            supported: true,
            capable: true,
            on: true,
            headroom: Some(4.0),
        },
        ram_total: 32 << 30,
        ram_available: 20 << 30,
        decode_budget: Some(16 << 30),
        ram_cache: 4 << 30,
        vram_cache: 768 << 20,
        display_cache: Some(4 << 30),
        decoders: vec![
            (
                "Blackmagic RAW · Blackmagic RAW SDK".into(),
                DecoderFact::Ready(Some("4.6".into())),
            ),
            ("DWG · ODA File Converter".into(), DecoderFact::NotFound),
            ("STEP, IGES · OpenCASCADE".into(), DecoderFact::Off),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_has_every_part_and_copies_as_text() {
        let r = report(&sample());
        assert_eq!(r.title_text(), t!("renderer-title"));
        assert_eq!(r.sections.len(), 5);
        let text = r.text().replace(['\u{2068}', '\u{2069}'], "");
        for s in [
            "Example GPU 7800",
            "Vulkan",
            "25.2.1",
            "1002:747e",
            "Wayland",
            "max_texture_dimension_2d",
            "TEXTURE_COMPRESSION_BC",
            "Rgb10a2Unorm",
            "Mailbox",
            "Blackmagic RAW",
        ] {
            assert!(text.contains(s), "{s} missing:\n{text}");
        }
        assert!(text.starts_with(&t!("renderer-title")));
        // Nothing known: no panic, "Unknown" and "None" instead.
        let empty = report(&RendererFacts::default()).text();
        assert!(empty.contains(&t!("renderer-unknown")));
    }
}
