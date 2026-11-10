//! `--hw-report`: the machine as Specular sees it, grouped into System, GPU, Decoding and
//! Performance insights. Coloured on a terminal unless `NO_COLOR` is set (`CLICOLOR_FORCE=1`
//! colours a pipe too). The facts are gathered by [`facts`], which the in-app report
//! ([`panel`]) shares.

use std::io::IsTerminal;

use specular_core::HardwareProfile;
use specular_core::paths::{Source, Trust, Unit};
use specular_gfx::wgpu::DeviceType;
use specular_platform::Thermal;

use crate::{app, open_report};

pub(crate) mod facts;
pub(crate) mod panel;

pub(crate) use facts::{Facts, REPORT_MPX};
/// Width of the label column.
const LABEL: usize = 22;

/// ANSI styling, or none.
struct Style {
    on: bool,
}

impl Style {
    fn detect() -> Self {
        let var = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
        let on = if var("NO_COLOR") {
            false
        } else {
            std::env::var("CLICOLOR_FORCE").is_ok_and(|v| v != "0")
                || std::io::stdout().is_terminal()
        };
        Self { on }
    }

    fn paint(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_owned()
        }
    }

    fn head(&self, s: &str) -> String {
        self.paint("1;36", s)
    }
    fn label(&self, s: &str) -> String {
        self.paint("90", s)
    }
    fn strong(&self, s: &str) -> String {
        self.paint("1", s)
    }
    fn good(&self, s: &str) -> String {
        self.paint("32", s)
    }
    fn warn(&self, s: &str) -> String {
        self.paint("33", s)
    }
    fn bad(&self, s: &str) -> String {
        self.paint("31", s)
    }
    fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }
}

/// Prints the report section by section.
struct Out {
    st: Style,
    first: bool,
}

impl Out {
    fn section(&mut self, title: &str) {
        if !self.first {
            println!();
        }
        self.first = false;
        println!("{}", self.st.head(title));
    }

    /// One labelled row; `value` may already be styled.
    fn row(&self, label: &str, value: &str) {
        let pad = LABEL.saturating_sub(label.chars().count() + 2);
        println!("  {}{} {value}", self.st.label(label), " ".repeat(pad));
    }

    /// A continuation line under the previous row.
    fn more(&self, value: &str) {
        println!("  {} {value}", " ".repeat(LABEL - 2));
    }
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1u64 << 30) as f64)
}

fn yes_no(st: &Style, v: bool) -> String {
    if v { st.good("yes") } else { "no".to_owned() }
}
pub(crate) fn run() {
    let mut hw = HardwareProfile::detect();
    let gpu = specular_gfx::Gpu::new();
    if let Ok(gpu) = &gpu {
        hw.gpu_name = gpu.info.name.clone();
        hw.gpu_backend = format!("{:?}", gpu.info.backend);
        hw.gpu_driver = format!("{} {}", gpu.info.driver, gpu.info.driver_info)
            .trim()
            .to_owned();
        hw.max_texture_size = gpu.max_texture_size();
    }
    // Sets the selector's machine and loads its calibration before it is reported.
    app::performance::describe_machine(&hw, gpu.as_ref().ok());
    // The GPU's video engine, which decodes JPEG, HEIC and AVIF stills. Probed before the
    // selector is read: finding a decoder makes its paths available.
    let hwdec = specular_hwdec::probe();
    let facts = Facts::gather(hw, gpu.as_ref().map_err(|e| e.to_string()), Some(hwdec));

    let mut o = Out {
        st: Style::detect(),
        first: true,
    };
    let title = format!("Specular {} hardware report", env!("CARGO_PKG_VERSION"));
    println!("{}", o.st.strong(&title));
    println!("{}", o.st.dim(&"─".repeat(title.chars().count())));
    println!();

    system(&mut o, &facts);
    gpu_section(&mut o, &facts);
    decoding(&mut o, &facts);
    insights(&mut o);
}

fn system(o: &mut Out, f: &Facts) {
    let hw = &f.hw;
    o.section("System");
    o.row("OS", &hw.os);
    o.row("CPU", &o.st.strong(&hw.cpu_model));
    o.row(
        "Cores",
        &format!(
            "{} logical, {} physical",
            hw.logical_cores, hw.physical_cores
        ),
    );
    if !hw.performance_cpus.is_empty() {
        let ids: Vec<String> = hw.performance_cpus.iter().map(|c| c.to_string()).collect();
        o.row(
            "Performance CPUs",
            &format!(
                "{} of {}  {}",
                hw.performance_cpus.len(),
                hw.logical_cores,
                o.st.dim(&ids.join(" "))
            ),
        );
    }
    for c in &f.clusters {
        o.row(
            "Core cluster",
            &format!("{} {} physical / {} logical", c.name, c.physical, c.logical),
        );
    }
    o.row(
        "CPU fast paths",
        &if f.cpu_fast_paths.is_empty() {
            o.st.dim("none")
        } else {
            f.cpu_fast_paths.join(", ")
        },
    );
    o.row(
        "RAM",
        &format!(
            "{} total, {} available",
            gib(hw.ram_total),
            o.st.strong(&gib(hw.ram_available))
        ),
    );
    let p = f.power;
    let thermal = match p.thermal {
        Thermal::Nominal => o.st.good("nominal"),
        Thermal::Fair => o.st.warn("fair"),
        Thermal::Serious => o.st.bad("serious"),
        Thermal::Critical => o.st.bad("critical"),
    };
    let source = match (p.on_battery, p.battery_percent) {
        (true, Some(b)) => o.st.warn(&format!("on battery ({b}%)")),
        (true, None) => o.st.warn("on battery"),
        (false, Some(b)) => format!("on mains ({b}%)"),
        (false, None) => "on mains".to_owned(),
    };
    let saving = if p.low_power {
        format!(", {}", o.st.warn("power saving"))
    } else {
        String::new()
    };
    o.row("Power", &format!("thermal {thermal}, {source}{saving}"));
}

fn gpu_section(o: &mut Out, f: &Facts) {
    o.section("GPU");
    match &f.gpu {
        Ok(g) => {
            o.row("Adapter", &o.st.strong(&g.name));
            let kind = match g.kind {
                DeviceType::DiscreteGpu => "discrete",
                DeviceType::IntegratedGpu => "integrated",
                DeviceType::VirtualGpu => "virtual",
                DeviceType::Cpu => "software (CPU)",
                DeviceType::Other => "other",
            };
            o.row("Type", kind);
            o.row("Backend", &g.backend);
            o.row("Driver", &g.driver);
            o.row("Init time", &format!("{:.1} ms", g.init_ms));
            o.row("Max texture size", &format!("{0} × {0}", g.max_texture));
            o.row("Max array layers", &g.max_layers.to_string());
            let compression: Vec<&str> =
                g.compression.iter().filter(|c| c.1).map(|c| c.0).collect();
            o.row(
                "Texture compression",
                &if compression.is_empty() {
                    "none".to_owned()
                } else {
                    compression.join(", ")
                },
            );
            let platform = g.platform_unified.map_or(String::new(), |u| {
                format!(", platform says {}", yes_no(&o.st, u))
            });
            o.row(
                "Unified memory",
                &format!("{}{platform}", yes_no(&o.st, g.unified)),
            );
        }
        Err(e) => o.row("Adapter", &o.st.bad(&format!("unavailable ({e})"))),
    }
    if let Some(s) = f.screen {
        let opt = |v: Option<f32>| v.map_or("?".to_owned(), |v| format!("{v:.2}"));
        o.row(
            "Main display",
            &format!(
                "{} Hz max, EDR headroom {} now / {} potential",
                s.max_fps.map_or("?".to_owned(), |f| f.to_string()),
                opt(s.edr_current),
                opt(s.edr_potential)
            ),
        );
    }
    match f.hwdec.flatten() {
        Some(c) => {
            let (device, driver) = split_device(&c.device);
            o.row(
                "Hardware decode",
                &format!("{} on {device}", o.st.strong(c.api)),
            );
            if !driver.is_empty() {
                o.more(&o.st.dim(driver));
            }
            for (name, caps) in [
                ("JPEG", &c.jpeg),
                ("H.264", &c.h264),
                ("HEVC", &c.hevc),
                ("AV1", &c.av1),
                ("H.264 encode", &c.h264_encode),
            ] {
                let v = match caps {
                    Some(k) => format!(
                        "{} up to {}×{}  {}",
                        o.st.good("✓"),
                        k.max_width,
                        k.max_height,
                        o.st.dim(&k.formats)
                    ),
                    None => o.st.dim("✗ no"),
                };
                o.more(&format!("{name:<12} {v}"));
            }
            if !c.others.is_empty() {
                o.more(&format!("{:<12} {}", "also", c.others.join(", ")));
            }
            if !c.software_only.is_empty() {
                o.more(&o.st.dim("in software:"));
                for s in &c.software_only {
                    o.more(&format!("  {}  {}", s.format, o.st.dim(s.reason)));
                }
            }
        }
        None => o.row("Hardware decode", &o.st.dim("none")),
    }
}

/// A hardware decoder's device description split into the device and its driver.
pub(crate) fn split_device(device: &str) -> (&str, &str) {
    device.split_once(", ").unwrap_or((device, ""))
}

fn decoding(o: &mut Out, f: &Facts) {
    o.section("Decoding");
    o.row("Decode threads", &f.hw.decode_threads().to_string());
    o.row(
        "RAM cache budget",
        &format!("{} MiB", f.hw.ram_cache_budget() >> 20),
    );
    let sel = &f.paths;
    let calibrated = if sel.calibrated {
        o.st.good("calibrated")
    } else {
        o.st.warn("not calibrated yet")
    };
    let saving = if sel.power_saving {
        format!(" · {}", o.st.warn("power saving"))
    } else {
        String::new()
    };
    o.row(
        "Path selection",
        &format!(
            "mode {:?} · {} GPU · {} threads · {calibrated}{saving}",
            sel.mode, sel.gpu_class, sel.threads
        ),
    );

    // Which path each task takes here, and why (DESIGN.md §12.3.1).
    let reports = &sel.tasks;
    if reports.is_empty() {
        return;
    }
    let tw = reports
        .iter()
        .map(|r| r.task.key().len())
        .max()
        .unwrap_or(0)
        .max(4);
    let pw = reports
        .iter()
        .flat_map(|r| r.paths.iter().map(|p| p.engine.key().len()))
        .max()
        .unwrap_or(0)
        .max(4);
    println!();
    println!(
        "  {}",
        o.st.label(&format!(
            "{:<tw$}  {:<pw$}  {:>9}  {:>9}  {:>9}   at {REPORT_MPX} MP",
            "task", "path", "wall", "CPU", "runs"
        ))
    );
    for r in reports {
        for (i, p) in r.paths.iter().enumerate() {
            let task = if i == 0 { r.task.key() } else { "" };
            // Only measured times are printed; untested or missing paths get no guess.
            let guess = p.estimate.source == Source::Prior || p.trust == Trust::Absent;
            let ms = |us: f32| {
                if guess {
                    "-".to_owned()
                } else {
                    format!("{:.1} ms", us / 1e3)
                }
            };
            let runs = if p.trust == Trust::Absent {
                "-".to_owned()
            } else if guess {
                "untested".to_owned()
            } else {
                p.samples.to_string()
            };
            let mut roles = Vec::new();
            if r.interactive.engine == p.engine {
                roles.push(format!("interactive ({})", r.interactive.reason.key()));
            }
            if r.background.engine == p.engine {
                roles.push(format!("background ({})", r.background.reason.key()));
            }
            let chosen = !roles.is_empty();
            let engine = format!("{:<pw$}", p.engine.key());
            let engine = if chosen { o.st.good(&engine) } else { engine };
            let trust = match p.trust {
                Trust::Trusted => String::new(),
                Trust::Unverified => format!("  {}", o.st.warn("unverified")),
                Trust::Blocked => format!("  {}", o.st.bad("blocked")),
                Trust::Absent => format!("  {}", o.st.dim("absent")),
            };
            let mut role = if chosen {
                format!("  {} {}", o.st.good("◀"), roles.join(", "))
            } else {
                String::new()
            };
            // Tasks timed at a size of their own say so.
            if i == 0 {
                match r.task.unit() {
                    Unit::Image => {}
                    Unit::Thumbnail { width, height } => {
                        role.push_str(&o.st.dim(&format!("  [per {width}×{height} thumbnail]")));
                    }
                    Unit::Frame { width, height } => {
                        role.push_str(&o.st.dim(&format!("  [per {width}×{height} frame]")));
                    }
                }
            }
            let runs = format!("{runs:>9}");
            println!(
                "  {}  {engine}  {:>9}  {:>9}  {}{trust}{role}",
                o.st.strong(&format!("{task:<tw$}")),
                ms(p.estimate.wall_us),
                ms(p.estimate.cpu_us),
                if guess { o.st.dim(&runs) } else { runs }
            );
        }
    }
}

/// Insights from the past week's slow opens (DESIGN.md §12.7), in the interface language.
fn insights(o: &mut Out) {
    let settings = specular_settings::SettingsStore::open_default()
        .map(|s| s.snapshot())
        .unwrap_or_default();
    let kept = open_report::read_kept(&open_report::kept_path());
    let text = open_report::insights::report(&kept, &settings);
    let mut lines = text.lines();
    o.section(lines.next().unwrap_or("Performance insights"));
    for l in lines {
        println!("{l}");
    }
}
