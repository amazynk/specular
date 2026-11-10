use super::*;

/// The insights panel's content as the app would localise it: HEIC decoded on the CPU.
pub(crate) fn insights() -> Insights {
    let stage = |name: &str, ms: f64, main| InsightStage {
        name: name.into(),
        ms,
        value: format!("{ms} ms"),
        main,
    };
    let item = |text: &str, gain: Option<&str>, action: Option<(&str, &str)>| InsightItem {
        text: text.into(),
        gain: gain.map(Into::into),
        action: action.map(|(a, k)| (a.into(), k.into())),
    };
    Insights {
        bottleneck: "HEIC images are decoded on the CPU".into(),
        detail: "6 of the last 6 slow HEIC images were decoded in software for this reason: \
                 hardware decoding is turned off in the settings"
            .into(),
        last: Some((
            "Last slow image: 930 ms until sharp".into(),
            vec![
                stage("Reading the file", 40.0, false),
                stage("Decoding", 820.0, true),
                stage("Uploading to the GPU", 70.0, false),
            ],
        )),
        now: vec![item(
            "Set Hardware decoding to Automatic in Settings → Performance.",
            Some("About 3 to 6 times faster"),
            Some(("Turn on", "set:hardware-decode")),
        )],
        hardware: vec![item(
            "A GPU with a HEVC hardware decoder",
            Some("About 3 to 6 times faster"),
            None,
        )],
        dismiss: Some((
            "Don’t show again for decoding".into(),
            "dismiss:decoder".into(),
        )),
        switch: Some(("Turn insights off".into(), "off".into())),
        restore: None,
    }
}

/// The panel about thumbnails as the app would localise it: reading from slow storage, with
/// hidden insights (so "Show hidden insights again" is offered).
pub(crate) fn thumb_insights() -> Insights {
    let stage = |name: &str, ms: f64, main| InsightStage {
        name: name.into(),
        ms,
        value: format!("{ms} ms"),
        main,
    };
    Insights {
        bottleneck: "Reading the files holds thumbnails up".into(),
        detail: "8 of the last 9 slow thumbnails were held up reading the file (about 12 MB/s)"
            .into(),
        last: Some((
            "Last slow thumbnail: 420 ms to make".into(),
            vec![
                stage("Reading the file", 380.0, true),
                stage("Reading the header", 10.0, false),
                stage("Decoding", 30.0, false),
            ],
        )),
        now: vec![
            InsightItem {
                text: "Copy the file to a local disk to open it faster.".into(),
                gain: Some("About 2.5 to 8 times faster".into()),
                action: None,
            },
            InsightItem {
                text: "Thumbnails cannot be kept on this computer (the thumbnail cache is \
                       unavailable), so they are made again each time the folder is shown."
                    .into(),
                gain: None,
                action: None,
            },
        ],
        hardware: vec![InsightItem {
            text: "A fast internal drive (an NVMe SSD) for these files".into(),
            gain: Some("About 2.5 to 8 times faster".into()),
            action: None,
        }],
        dismiss: Some((
            "Don’t show again for storage".into(),
            "dismiss:storage".into(),
        )),
        switch: Some(("Turn insights off".into(), "off".into())),
        restore: Some(("Show hidden insights again".into(), "unhide".into())),
    }
}

/// A crash report as the app shows it: redacted, with its context and log lines.
pub(crate) const CRASH_REPORT: &str = "Specular 0.1.0 crashed at 2026-10-12 10:00:00 UTC
This report stays on this computer; nothing was sent anywhere.

thread:   main
location: crates/specular-decode/src/jpeg/scan.rs:412:17
message:  index out of bounds: the len is 64 but the index is 64
os:       linux x86_64
gpu:      AMD Radeon RX 7800 XT (RADV NAVI32), Vulkan, driver Mesa 25.2.1
open:     ~/dir-1/file-1.jpg

backtrace:
   0: specular_decode::jpeg::scan::decode_block
             at ./crates/specular-decode/src/jpeg/scan.rs:412:17
   1: specular_decode::jpeg::scan::decode_mcu_row
             at ./crates/specular-decode/src/jpeg/scan.rs:388:9

last 3 log lines:
   1201.4 main          io       opened ~/dir-1/file-1.jpg
   1203.0 dec-1         decode   img=7 file-1.jpg header 6000×4000
   1203.9 dec-1         decode   img=7 scan 1/1
";

/// The diagnostics bundle's preview with file names hidden.
pub(crate) fn diag_sheet() -> DiagSheet {
    let e = |name: &str, description: &str, text: &str| DiagEntry {
        name: name.into(),
        description: description.into(),
        text: text.into(),
    };
    DiagSheet {
        entries: vec![
            e(
                "README.txt",
                "What this bundle is",
                "Specular diagnostics bundle.\n\nNothing was sent.\n",
            ),
            e(
                "summary.txt",
                "Version, system, GPU and driver",
                "Specular 0.1.0 (0123abcd, release)\nOS: linux x86_64\nGPU: AMD Radeon RX 7800 XT, Mesa 25.2.1\nOpen: ~/dir-1/file-1.jpg\n",
            ),
            e("crash-latest.txt", "Last crash report", CRASH_REPORT),
            e(
                "log.txt",
                "Last 200 log lines",
                "   1201.4 main io opened ~/dir-1/file-1.jpg\n",
            ),
            e(
                "settings.toml",
                "Settings, without passwords or addresses",
                "[viewing]\nzoom = \"fit\"\n[remote]\nserver_url = \"<redacted>\"\n",
            ),
        ],
        include_names: false,
        issues: "https://example.com/specular/issues".into(),
        email: "support@example.com".into(),
    }
}

pub(crate) fn hw_report(hebrew: bool) -> HwReport {
    let en_he = |en: &str, he: &str| if hebrew { he.to_owned() } else { en.to_owned() };
    let ltr = |s: &str| isolate_ltr(s);
    let ms = |v: &str| ltr(&format!("{v} ms"));
    let chips = |c: &[(&str, bool)]| c.iter().map(|&(n, on)| (n.to_owned(), on)).collect();
    let path = |engine: String, time: &str, cpu: &str, runs: &str, uses: Vec<String>| ReportPath {
        engine,
        time: ms(time),
        cpu: ms(cpu),
        runs: runs.into(),
        guess: false,
        uses,
        state: None,
    };
    let viewing = |why: (&str, &str)| {
        en_he(
            &format!("Viewing ({})", why.0),
            &format!("צפייה ({})", why.1),
        )
    };
    let background = |why: (&str, &str)| {
        en_he(
            &format!("Background ({})", why.0),
            &format!("רקע ({})", why.1),
        )
    };
    let fastest = ("fastest", "המהיר ביותר");
    let cheapest = ("least CPU time", "הכי מעט זמן CPU");
    let only = ("the only option", "האפשרות היחידה");
    let cpu = || en_he("CPU", "CPU");
    let gpu = || en_he("GPU", "GPU");
    let hw = || en_he("Hardware decoder", "מפענח חומרה");
    let up_to = |w: &str, h: &str| en_he(&format!("Up to {w} × {h}"), &format!("עד {w} × {h}"));
    HwReport {
        title: None,
        sections: vec![
            ReportSection {
                title: en_he("System", "מערכת"),
                rows: vec![
                    ReportRow::new(
                        en_he("Version", "גרסה"),
                        format!(
                            "{} · {}",
                            ltr("0.9.0 x86_64-linux"),
                            en_he("release build", "גרסת הפצה")
                        ),
                    ),
                    ReportRow::new(
                        en_he("Operating system", "מערכת הפעלה"),
                        ltr("Ubuntu 26.04 LTS (Linux 7.2.6)"),
                    ),
                    ReportRow::new(
                        en_he("CPU", "CPU"),
                        ltr("AMD Ryzen 9 7950X 16-Core Processor"),
                    )
                    .tone(Tone::Strong),
                    ReportRow::new(
                        en_he("Cores", "ליבות"),
                        en_he("32 logical, 16 physical", "32 לוגיות, 16 פיזיות"),
                    ),
                    ReportRow::new(en_he("CPU fast paths", "נתיבים מהירים ב־CPU"), "").chips(
                        chips(&[
                            ("SSE4.2", true),
                            ("AVX2", true),
                            ("FMA", true),
                            ("BMI2", true),
                        ]),
                    ),
                    ReportRow::new(
                        en_he("Memory", "זיכרון"),
                        // Sizes as Fluent arguments are: isolated.
                        en_he(
                            &format!("{} total, {} available", ltr("62.5 GiB"), ltr("41.2 GiB")),
                            &format!("{} בסך הכול, {} פנויים", ltr("62.5 GiB"), ltr("41.2 GiB")),
                        ),
                    ),
                    ReportRow::new(
                        en_he("Power", "חשמל"),
                        en_he(
                            "Normal temperature · Plugged in",
                            "טמפרטורה רגילה · מחובר לחשמל",
                        ),
                    )
                    .tone(Tone::Good),
                ],
                ..ReportSection::default()
            },
            ReportSection {
                title: en_he("GPU", "GPU"),
                rows: vec![
                    ReportRow::new(
                        en_he("Adapter", "מתאם"),
                        ltr("AMD Radeon RX 7800 XT (RADV NAVI32)"),
                    )
                    .tone(Tone::Strong),
                    ReportRow::new(en_he("Type", "סוג"), en_he("Dedicated", "ייעודי")),
                    ReportRow::new(en_he("Graphics API", "ממשק גרפי"), ltr("Vulkan")),
                    ReportRow::new(en_he("Driver", "מנהל התקן"), ltr("radv Mesa 25.2.3")),
                    ReportRow::new(en_he("Start-up time", "זמן הפעלה"), ms("142.6")),
                    ReportRow::new(
                        en_he("Largest texture", "המרקם הגדול ביותר"),
                        ltr("16,384 × 16,384 px"),
                    ),
                    ReportRow::new(en_he("Texture layers", "שכבות מרקם"), "2,048"),
                    ReportRow::new(en_he("Texture compression", "דחיסת מרקמים"), "")
                        .chips(chips(&[("BC", true), ("ASTC", false), ("ETC2", false)])),
                    ReportRow::new(
                        en_he("Memory shared with the CPU", "זיכרון משותף עם ה־CPU"),
                        en_he("No", "לא"),
                    )
                    .mark(false),
                    ReportRow::new(
                        en_he("Hardware decoding", "פענוח בחומרה"),
                        en_he(
                            "VA-API on AMD Radeon RX 7800 XT",
                            "VA-API ב-AMD Radeon RX 7800 XT",
                        ),
                    )
                    .detail(ltr("Mesa Gallium driver 25.2.3 for AMD Radeon RX 7800 XT")),
                    ReportRow::new("JPEG", up_to("16,384", "16,384"))
                        .mark(true)
                        .detail(ltr("4:2:0, 4:2:2, 4:4:4"))
                        .sub(),
                    ReportRow::new("HEVC", up_to("8,192", "4,352"))
                        .mark(true)
                        .detail(ltr("4:2:0"))
                        .sub(),
                    ReportRow::new("AV1", en_he("Not available", "לא זמין"))
                        .mark(false)
                        .tone(Tone::Muted)
                        .sub(),
                    ReportRow::new(en_he("Others", "אחרים"), ltr("H.264, VP9")).sub(),
                ],
                ..ReportSection::default()
            },
            ReportSection {
                title: en_he("Decoding", "פענוח"),
                rows: vec![
                    ReportRow::new(en_he("Decode threads", "תהליכוני פענוח"), "16"),
                    ReportRow::new(
                        en_he("Image memory cache", "מטמון תמונות בזיכרון"),
                        ltr("4 GiB"),
                    ),
                    ReportRow::new(
                        en_he("GPU memory cache", "מטמון בזיכרון ה־GPU"),
                        ltr("768 MiB"),
                    ),
                    ReportRow::new(en_he("Processing", "עיבוד"), en_he("Automatic", "אוטומטי")),
                    ReportRow::new(
                        en_he("Speed measurements", "מדידות מהירות"),
                        en_he("Measured on this computer", "נמדד במחשב זה"),
                    )
                    .tone(Tone::Good)
                    .mark(true),
                ],
                table: Some(ReportTable {
                    caption: en_he(
                        "Paths for a 24-megapixel image",
                        "נתיבים לתמונה של 24 מגה-פיקסל",
                    ),
                    columns: [
                        en_he("Task", "משימה"),
                        en_he("Path", "נתיב"),
                        en_he("Time", "זמן"),
                        en_he("CPU time (all threads)", "זמן CPU (כל התהליכונים)"),
                        en_he("Runs", "הרצות"),
                        en_he("Used for", "משמש ל"),
                    ],
                    tasks: vec![
                        ReportTask {
                            name: en_he("JPEG decoding", "פענוח JPEG"),
                            paths: vec![
                                path(cpu(), "182.4", "182.4", "14", Vec::new()),
                                path(
                                    en_he("CPU, all cores", "CPU, כל הליבות"),
                                    "41.0",
                                    "298.1",
                                    "14",
                                    Vec::new(),
                                ),
                                path(
                                    hw(),
                                    "28.7",
                                    "3.2",
                                    "9",
                                    vec![viewing(fastest), background(cheapest)],
                                ),
                            ],
                        },
                        ReportTask {
                            name: en_he("Colour conversion", "המרת צבע"),
                            paths: vec![
                                path(cpu(), "12.4", "12.4", "31", Vec::new()),
                                path(
                                    gpu(),
                                    "2.1",
                                    "0.4",
                                    "31",
                                    vec![viewing(fastest), background(cheapest)],
                                ),
                            ],
                        },
                        ReportTask {
                            name: en_he("HEIC decoding", "פענוח HEIC"),
                            paths: vec![
                                path(
                                    cpu(),
                                    "410.0",
                                    "410.0",
                                    "3",
                                    vec![viewing(only), background(only)],
                                ),
                                ReportPath {
                                    guess: true,
                                    runs: en_he("Estimate", "הערכה"),
                                    state: Some((en_he("Not checked yet", "טרם נבדק"), Tone::Warn)),
                                    ..path(hw(), "35.0", "4.0", "", Vec::new())
                                },
                            ],
                        },
                    ],
                }),
                notes: Vec::new(),
            },
            ReportSection {
                title: en_he("Performance insights", "תובנות ביצועים"),
                notes: vec![en_he(
                    "Images open quickly on this computer: nothing was slow in the past week.",
                    "תמונות נפתחות מהר במחשב זה: שום דבר לא היה איטי בשבוע האחרון.",
                )],
                ..ReportSection::default()
            },
        ],
    }
}

/// This feature's goldens ([`super::cases`]).
pub(crate) fn report_shot_cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    vec![
        c("crash-dark", State::Crash, true, langid!("en-US")),
        c(
            "diagnostics-light",
            State::Diagnostics,
            false,
            langid!("en-US"),
        ),
        c(
            "diagnostics-dark",
            State::Diagnostics,
            true,
            langid!("en-US"),
        ),
        c("hw-report-light", State::HwReport, false, langid!("en-US")),
        c("hw-report-dark", State::HwReport, true, langid!("en-US")),
        c(
            "hw-report-table-light",
            State::HwReportTable,
            false,
            langid!("en-US"),
        ),
        c(
            "hw-report-table-dark",
            State::HwReportTable,
            true,
            langid!("en-US"),
        ),
        c("hw-report-he-dark", State::HwReport, true, langid!("he")),
        c(
            "hw-report-table-he-dark",
            State::HwReportTable,
            true,
            langid!("he"),
        ),
        c(
            "hw-report-text-2-light",
            State::HwReportLargeText,
            false,
            langid!("en-US"),
        ),
        c("insights-dark", State::Insights, true, langid!("en-US")),
    ]
}
