//! The in-app hardware report (Settings → Advanced): the [`Facts`] of `--hw-report` in the
//! interface language, in the same sections.

use specular_core::paths::{Choice, Mode, Reason, Source, TaskReport, Trust, Unit};
use specular_gfx::wgpu::DeviceType;
use specular_hwdec::CodecCaps;
use specular_i18n::{ByteUnits, I18n, isolate_ltr, t};
use specular_platform::Thermal;
use specular_widgets::{
    HwReport, ReportPath, ReportRow, ReportSection, ReportTable, ReportTask, Tone,
};

use super::{Facts, REPORT_MPX, split_device};

/// What the app adds to the facts: its own settings and state.
pub(crate) struct AppState {
    pub(crate) decode_threads: usize,
    pub(crate) ram_cache: u64,
    pub(crate) vram_cache: u64,
    /// Available memory now (the profile's figure is from start-up).
    pub(crate) ram_available: u64,
    /// The performance insights as plain text, title first ([`crate::open_report::insights::text`]).
    pub(crate) insights: String,
}

pub(crate) fn build(f: &Facts, app: &AppState) -> HwReport {
    HwReport {
        title: None,
        sections: vec![
            system(f, app),
            graphics(f),
            decoding(f, app),
            insights(&app.insights),
        ],
    }
}

fn size(b: u64) -> String {
    I18n::global().localizer().bytes(b, ByteUnits::Binary)
}

fn integer(n: impl TryInto<i64>) -> String {
    I18n::global()
        .localizer()
        .integer(n.try_into().unwrap_or(i64::MAX))
}

fn ms(ms: f64) -> String {
    t!(
        "hw-report-ms",
        ms = I18n::global().localizer().number_fixed(ms, 1)
    )
}

fn system(f: &Facts, app: &AppState) -> ReportSection {
    let hw = &f.hw;
    let mut rows = vec![
        ReportRow::new(
            t!("hw-report-version"),
            format!(
                "{} · {}",
                isolate_ltr(&format!(
                    "{} {}-{}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::ARCH,
                    std::env::consts::OS
                )),
                if cfg!(debug_assertions) {
                    t!("hw-report-build-debug")
                } else {
                    t!("hw-report-build-release")
                }
            ),
        ),
        ReportRow::new(t!("hw-report-operating-system"), isolate_ltr(&hw.os)),
        ReportRow::new(t!("hw-report-cpu"), isolate_ltr(&hw.cpu_model)).tone(Tone::Strong),
        ReportRow::new(
            t!("hw-report-cores"),
            t!(
                "hw-report-cores-value",
                logical = integer(hw.logical_cores),
                physical = integer(hw.physical_cores)
            ),
        ),
    ];
    if !hw.performance_cpus.is_empty() {
        let ids: Vec<String> = hw.performance_cpus.iter().map(|c| c.to_string()).collect();
        rows.push(
            ReportRow::new(
                t!("hw-report-performance-cores"),
                t!(
                    "hw-report-performance-cores-value",
                    count = integer(hw.performance_cpus.len()),
                    total = integer(hw.logical_cores)
                ),
            )
            .detail(isolate_ltr(&ids.join(" "))),
        );
    }
    for c in &f.clusters {
        rows.push(ReportRow::new(
            t!("hw-report-core-cluster"),
            t!(
                "hw-report-cluster-value",
                name = c.name.clone(),
                physical = integer(c.physical),
                logical = integer(c.logical)
            ),
        ));
    }
    let fast = ReportRow::new(t!("hw-report-cpu-features"), "");
    rows.push(if f.cpu_fast_paths.is_empty() {
        ReportRow::new(t!("hw-report-cpu-features"), t!("hw-report-none")).tone(Tone::Muted)
    } else {
        fast.chips(
            f.cpu_fast_paths
                .iter()
                .map(|s| ((*s).to_owned(), true))
                .collect(),
        )
    });
    rows.push(ReportRow::new(
        t!("hw-report-memory"),
        t!(
            "hw-report-memory-value",
            total = size(hw.ram_total),
            available = size(app.ram_available)
        ),
    ));
    rows.push(power(f));
    ReportSection {
        title: t!("hw-report-section-system"),
        rows,
        ..ReportSection::default()
    }
}

/// Temperature, power source and power saving, toned by the worst of them.
fn power(f: &Facts) -> ReportRow {
    let p = f.power;
    let (thermal, mut tone) = match p.thermal {
        Thermal::Nominal => (t!("hw-report-thermal-nominal"), Tone::Good),
        Thermal::Fair => (t!("hw-report-thermal-fair"), Tone::Warn),
        Thermal::Serious => (t!("hw-report-thermal-serious"), Tone::Bad),
        Thermal::Critical => (t!("hw-report-thermal-critical"), Tone::Bad),
    };
    let percent = |b: u8| I18n::global().localizer().percent(f64::from(b) / 100.0, 0);
    let source = match (p.on_battery, p.battery_percent) {
        (true, Some(b)) => t!("hw-report-power-battery", percent = percent(b)),
        (true, None) => t!("hw-report-power-battery-unknown"),
        (false, Some(b)) => t!("hw-report-power-mains-percent", percent = percent(b)),
        (false, None) => t!("hw-report-power-mains"),
    };
    let mut parts = vec![thermal, source];
    if p.low_power {
        parts.push(t!("hw-report-power-saving"));
    }
    if (p.on_battery || p.low_power) && tone == Tone::Good {
        tone = Tone::Warn;
    }
    ReportRow::new(t!("hw-report-power"), specular_i18n::dot_join(&parts)).tone(tone)
}

fn graphics(f: &Facts) -> ReportSection {
    let mut rows = Vec::new();
    match &f.gpu {
        Ok(g) => {
            rows.push(
                ReportRow::new(t!("hw-report-adapter"), isolate_ltr(&g.name)).tone(Tone::Strong),
            );
            let kind = match g.kind {
                DeviceType::DiscreteGpu => t!("hw-report-gpu-discrete"),
                DeviceType::IntegratedGpu => t!("hw-report-gpu-integrated"),
                DeviceType::VirtualGpu => t!("hw-report-gpu-virtual"),
                DeviceType::Cpu => t!("hw-report-gpu-software"),
                DeviceType::Other => t!("hw-report-gpu-other"),
            };
            rows.push(ReportRow::new(t!("hw-report-gpu-type"), kind));
            rows.push(ReportRow::new(
                t!("hw-report-graphics-api"),
                isolate_ltr(&g.backend),
            ));
            if !g.driver.is_empty() {
                rows.push(ReportRow::new(
                    t!("hw-report-driver"),
                    isolate_ltr(&g.driver),
                ));
            }
            rows.push(ReportRow::new(t!("hw-report-gpu-init"), ms(g.init_ms)));
            rows.push(ReportRow::new(
                t!("hw-report-max-texture"),
                t!(
                    "hw-report-pixels",
                    width = integer(g.max_texture),
                    height = integer(g.max_texture)
                ),
            ));
            rows.push(ReportRow::new(
                t!("hw-report-max-layers"),
                integer(g.max_layers),
            ));
            rows.push(
                ReportRow::new(t!("hw-report-compression"), "").chips(
                    g.compression
                        .iter()
                        .map(|(n, on)| ((*n).to_owned(), *on))
                        .collect(),
                ),
            );
            let mut unified = ReportRow::new(
                t!("hw-report-unified-memory"),
                if g.unified {
                    t!("hw-report-yes")
                } else {
                    t!("hw-report-no")
                },
            )
            .mark(g.unified);
            if g.platform_unified.is_some_and(|u| u != g.unified) {
                unified = unified.detail(t!("hw-report-unified-platform-differs"));
            }
            rows.push(unified);
        }
        Err(e) => {
            let row = ReportRow::new(t!("hw-report-adapter"), t!("hw-report-gpu-unavailable"))
                .tone(Tone::Bad)
                .mark(false);
            rows.push(if e.is_empty() {
                row
            } else {
                row.detail(e.clone())
            });
        }
    }
    if let Some(s) = f.screen {
        let l = I18n::global().localizer();
        let opt = |v: Option<f32>| v.map_or("?".to_owned(), |v| l.number(f64::from(v), 2));
        rows.push(ReportRow::new(
            t!("hw-report-display"),
            t!(
                "hw-report-display-value",
                fps = s.max_fps.map_or("?".to_owned(), integer),
                now = opt(s.edr_current),
                max = opt(s.edr_potential)
            ),
        ));
    }
    hw_decoders(f, &mut rows);
    ReportSection {
        title: t!("hw-report-section-gpu"),
        rows,
        ..ReportSection::default()
    }
}

/// The GPU's video engine, which decodes JPEG, HEIC and AVIF stills: a row, then one per codec.
fn hw_decoders(f: &Facts, rows: &mut Vec<ReportRow>) {
    let label = t!("hw-report-hw-decode");
    let c = match f.hwdec {
        None => {
            rows.push(ReportRow::new(label, t!("hw-report-hw-decode-checking")).tone(Tone::Muted));
            return;
        }
        Some(None) => {
            rows.push(
                ReportRow::new(label, t!("hw-report-none"))
                    .mark(false)
                    .tone(Tone::Muted),
            );
            return;
        }
        Some(Some(c)) => c,
    };
    let (device, driver) = split_device(&c.device);
    let mut row = ReportRow::new(
        label,
        t!(
            "hw-report-hw-decode-value",
            api = c.api,
            device = isolate_ltr(device)
        ),
    );
    if !driver.is_empty() {
        row = row.detail(isolate_ltr(driver));
    }
    rows.push(row);
    let codec = |name: &str, caps: &Option<CodecCaps>| match caps {
        Some(k) => ReportRow::new(
            name,
            t!(
                "hw-report-codec-up-to",
                width = integer(k.max_width),
                height = integer(k.max_height)
            ),
        )
        .mark(true)
        .detail(isolate_ltr(&k.formats))
        .sub(),
        None => ReportRow::new(name, t!("hw-report-codec-none"))
            .mark(false)
            .tone(Tone::Muted)
            .sub(),
    };
    rows.push(codec("JPEG", &c.jpeg));
    rows.push(codec("H.264", &c.h264));
    rows.push(codec("HEVC", &c.hevc));
    rows.push(codec("AV1", &c.av1));
    rows.push(codec(&t!("hw-report-codec-h264-encode"), &c.h264_encode));
    if !c.others.is_empty() {
        rows.push(
            ReportRow::new(
                t!("hw-report-codec-others"),
                isolate_ltr(&c.others.join(", ")),
            )
            .sub(),
        );
    }
    if !c.software_only.is_empty() {
        let formats: Vec<&str> = c.software_only.iter().map(|s| s.format).collect();
        rows.push(
            ReportRow::new(
                t!("hw-report-codec-software"),
                isolate_ltr(&formats.join(", ")),
            )
            .tone(Tone::Muted)
            .sub(),
        );
    }
}

fn decoding(f: &Facts, app: &AppState) -> ReportSection {
    let sel = &f.paths;
    let mode = match sel.mode {
        Mode::Automatic => t!("settings-performance-processing-automatic"),
        Mode::PreferGpu => t!("settings-performance-processing-prefer-gpu"),
        Mode::CpuOnly => t!("settings-performance-processing-cpu-only"),
    };
    let mut processing = ReportRow::new(t!("hw-report-processing"), mode);
    if sel.power_saving {
        processing = processing.detail(t!("hw-report-paths-power-saving"));
    }
    let rows = vec![
        ReportRow::new(t!("hw-report-decode-threads"), integer(app.decode_threads)),
        ReportRow::new(t!("hw-report-ram-cache"), size(app.ram_cache)),
        ReportRow::new(t!("hw-report-vram-cache"), size(app.vram_cache)),
        processing,
        if sel.calibrated {
            ReportRow::new(
                t!("hw-report-calibration"),
                t!("hw-report-calibration-done"),
            )
            .tone(Tone::Good)
            .mark(true)
        } else {
            ReportRow::new(
                t!("hw-report-calibration"),
                t!("hw-report-calibration-pending"),
            )
            .tone(Tone::Warn)
        },
    ];
    let table = (!sel.tasks.is_empty()).then(|| ReportTable {
        caption: t!(
            "hw-report-paths-caption",
            mpx = I18n::global().localizer().number(f64::from(REPORT_MPX), 0)
        ),
        columns: [
            t!("hw-report-col-task"),
            t!("hw-report-col-path"),
            t!("hw-report-col-time"),
            t!("hw-report-col-cpu"),
            t!("hw-report-col-runs"),
            t!("hw-report-col-use"),
        ],
        tasks: sel.tasks.iter().map(task).collect(),
    });
    ReportSection {
        title: t!("hw-report-section-decoding"),
        rows,
        table,
        notes: Vec::new(),
    }
}

/// The task's name, with the size of one request when it has its own (the table's caption
/// is for images).
fn task_name(r: &TaskReport) -> String {
    let name = I18n::global().tr(r.task.message_id());
    let size = |width: u32, height: u32| {
        t!(
            "hw-report-pixels",
            width = integer(width),
            height = integer(height)
        )
    };
    match r.task.unit() {
        Unit::Image => name,
        Unit::Thumbnail { width, height } => t!(
            "hw-report-task-thumbnail",
            task = name,
            size = size(width, height)
        ),
        Unit::Frame { width, height } => t!(
            "hw-report-task-frame",
            task = name,
            size = size(width, height)
        ),
    }
}

/// Why a path was chosen, in a few words.
fn reason(r: Reason) -> String {
    match r {
        Reason::Only => t!("hw-report-reason-only"),
        Reason::Faster => t!("hw-report-reason-faster"),
        Reason::Cheaper => t!("hw-report-reason-cheaper"),
        Reason::LowerPower => t!("hw-report-reason-lower-power"),
        Reason::Explore => t!("hw-report-reason-explore"),
        Reason::Forced => t!("hw-report-reason-forced"),
        Reason::PreferGpu => t!("hw-report-reason-prefer-gpu"),
        Reason::CpuOnly => t!("hw-report-reason-cpu-only"),
        Reason::Calibrate => t!("hw-report-reason-calibrate"),
        Reason::Scaled => t!("hw-report-reason-scaled"),
    }
}

fn task(r: &TaskReport) -> ReportTask {
    let uses = |p: &specular_core::paths::PathReport| {
        let on = |c: &Choice| c.engine == p.engine;
        let mut v = Vec::new();
        if on(&r.interactive) {
            v.push(t!(
                "hw-report-use-viewing",
                reason = reason(r.interactive.reason)
            ));
        }
        if on(&r.background) {
            v.push(t!(
                "hw-report-use-background",
                reason = reason(r.background.reason)
            ));
        }
        v
    };
    ReportTask {
        name: task_name(r),
        paths: r
            .paths
            .iter()
            .map(|p| {
                // Only measured times are shown: a path that is missing or not yet run
                // (an NPU, a hardware decoder) gets no guessed figure.
                let guess = p.estimate.source == Source::Prior || p.trust == Trust::Absent;
                let timed = |us: f32| {
                    if guess {
                        "—".to_owned()
                    } else {
                        ms(f64::from(us) / 1e3)
                    }
                };
                ReportPath {
                    engine: I18n::global().tr(p.engine.message_id()),
                    time: timed(p.estimate.wall_us),
                    cpu: timed(p.estimate.cpu_us),
                    runs: if p.trust == Trust::Absent {
                        "—".to_owned()
                    } else if guess {
                        t!("hw-report-runs-untested")
                    } else {
                        integer(p.samples)
                    },
                    guess,
                    uses: uses(p),
                    state: match p.trust {
                        Trust::Trusted => None,
                        Trust::Unverified => Some((t!("hw-report-trust-unverified"), Tone::Warn)),
                        Trust::Blocked => Some((t!("hw-report-trust-blocked"), Tone::Bad)),
                        Trust::Absent => Some((t!("hw-report-trust-absent"), Tone::Muted)),
                    },
                }
            })
            .collect(),
    }
}

/// The performance insights: the panel's text as paragraphs under its title.
fn insights(text: &str) -> ReportSection {
    let mut lines = text.lines();
    ReportSection {
        title: lines
            .next()
            .map_or_else(|| t!("open-report-insights-title"), str::to_owned),
        notes: lines
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect(),
        ..ReportSection::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without a graphics adapter and while the decoders are still probed: the four sections
    /// in `--hw-report`'s order, saying so.
    #[test]
    fn sections_in_cli_order() {
        let f = Facts::gather(
            specular_core::HardwareProfile::default(),
            Err(String::new()),
            None,
        );
        let app = AppState {
            decode_threads: 4,
            ram_cache: 1 << 30,
            vram_cache: 768 << 20,
            ram_available: 2 << 30,
            insights: "Performance insights\n  Nothing was slow.\n".into(),
        };
        let r = build(&f, &app);
        let titles: Vec<&str> = r.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            ["System", "GPU", "Decoding", "Performance insights"]
        );
        let text = r.text();
        assert!(text.contains("Not available"), "{text}");
        assert!(text.contains("Still checking…"), "{text}");
        assert!(text.contains("  Nothing was slow."), "{text}");
    }
}
