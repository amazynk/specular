use super::*;

/// The card the app shows for it (`app/optional.rs`).
pub(crate) fn decoder_card() -> specular_widgets::DecoderCard {
    specular_widgets::DecoderCard {
        id: "braw".into(),
        title: "Blackmagic RAW needs Blackmagic Design’s free decoder".into(),
        lines: vec![
            "A001_C002.braw opens with the Blackmagic RAW SDK. It’s free.".into(),
            "Install it, then come back: Specular finds it by itself.".into(),
        ],
        buttons: vec![
            "Get it".into(),
            "I’ve installed it".into(),
            "Locate…".into(),
        ],
    }
}

/// The indicator as the app words it (`app/work.rs`), in the case's language.
pub(crate) fn work_chip(paused: bool) -> WorkChip {
    use specular_i18n::t;
    let chip = if paused {
        t!(
            "work-chip-paused",
            job = t!("work-kind-library"),
            reason = t!("work-paused-battery")
        )
    } else {
        t!(
            "work-chip",
            job = t!("work-kind-ai-search"),
            done = "1,240",
            total = "9,800"
        )
    };
    let label = t!("work-chip-more", chip = chip, more = 2);
    WorkChip {
        name: t!("work-chip-name", status = label.clone()),
        label,
        paused,
    }
}

/// The panel as the app fills it (`app/work.rs`): a job with its time left and what
/// follows, a paused one with an unknown total, a download still estimating; in the case's
/// language.
pub(crate) fn work_panel() -> WorkPanel {
    use specular_i18n::t;
    let settings = |page: &str| Some((t!("work-settings"), format!("settings:{page}")));
    let pause = |key: &str| Some((t!("work-pause"), format!("pause:{key}")));
    let step = |step: &str, item: &str| {
        t!(
            "work-step-item",
            step = specular_i18n::I18n::global().tr(step),
            item = item
        )
    };
    let photos = |done: &str, total: &str, n: i64, left: &str| {
        t!(
            "work-count-left",
            count = t!("work-count-photos", done = done, total = total, count = n),
            left = left
        )
    };
    WorkPanel {
        rows: vec![
            WorkRow {
                title: t!("work-kind-ai-search"),
                step: step("work-step-embedding", "IMG_2291.jpg"),
                count: photos("1,240", "9,800", 9800, "8,560"),
                fraction: Some(1240.0 / 9800.0),
                status: t!("work-eta-minutes", n = 25),
                paused: false,
                then: Some(t!("work-then", steps = t!("work-step-faces"))),
                pause: pause("ai-search"),
                settings: settings("ai.search"),
            },
            WorkRow {
                title: t!("work-kind-library"),
                step: step("work-step-reading", "2024-06 Lisbon"),
                count: t!("work-so-far-photos", done = "3,412", count = 3412),
                fraction: None,
                status: t!("work-paused-battery"),
                paused: true,
                then: Some(t!(
                    "work-then",
                    steps = specular_i18n::I18n::global().localizer().list(
                        &[t!("work-step-signatures"), t!("work-step-grouping")],
                        specular_i18n::ListKind::And
                    )
                )),
                pause: pause("library"),
                settings: settings("gallery"),
            },
            WorkRow {
                title: t!("work-kind-download"),
                step: t!("work-step-downloading"),
                count: t!(
                    "work-count-left",
                    count = t!("work-count", done = "120 MB", total = "340 MB"),
                    left = "220 MB"
                ),
                fraction: Some(120.0 / 340.0),
                status: t!("work-eta-estimating"),
                paused: false,
                then: None,
                pause: None,
                settings: settings("ai"),
            },
        ],
    }
}

/// The hardware report as the app would localise it: a desktop with a dedicated graphics card
/// and hardware decoders, HEIC not checked yet.
/// Placeholder offers for the Local AI page (sizes, licences and models are made up: which
/// models ship is DESIGN.md open decision 7).
/// "More file types" as the app builds it: open, editor files expanded with Krita ticked. Its
/// texts come from the catalogue, in the case's language.
pub(crate) fn more_types() -> SettingPanel {
    use specular_i18n::t;
    let row = |id: &str, label: &str, value: &str, check, expand, indent| PanelRow {
        id: id.into(),
        label: label.into(),
        value: value.into(),
        check,
        expand,
        indent,
        ..Default::default()
    };
    let mut rows = vec![PanelRow {
        lines: vec![t!("integration-extra-intro")],
        ..row(
            "more",
            &t!("integration-extra-title"),
            &t!("integration-extra-chosen", count = 1),
            None,
            Some(true),
            0,
        )
    }];
    rows.push(PanelRow {
        lines: vec![t!("integration-extra-group-editors-help")],
        ..row(
            "g:editors",
            &t!("integration-extra-group-editors"),
            &t!("integration-extra-count", on = 1, total = 9),
            Some(CheckState::Mixed),
            Some(true),
            1,
        )
    });
    for (m, ext, on) in [
        ("application/x-krita", ".kra", true),
        ("image/x-xcf", ".xcf", false),
    ] {
        let check = Some(if on { CheckState::On } else { CheckState::Off });
        rows.push(PanelRow {
            detail: m.into(),
            ..row(m, ext, "", check, None, 3)
        });
    }
    rows.push(row(
        "g:medical",
        &t!("integration-extra-group-medical"),
        &t!("integration-extra-count", on = 0, total = 11),
        Some(CheckState::Off),
        Some(false),
        1,
    ));
    rows.push(row(
        "g:models",
        &t!("integration-extra-group-models"),
        &t!("integration-extra-count", on = 15, total = 15),
        Some(CheckState::On),
        Some(false),
        1,
    ));
    rows.push(row(
        "g:videos",
        &t!("integration-extra-group-videos"),
        &t!("integration-extra-none-yet"),
        None,
        None,
        1,
    ));
    SettingPanel {
        rows,
        ..Default::default()
    }
}

/// This feature's goldens ([`super::cases`]).
pub(crate) fn work_shot_cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    vec![
        c("work-chip-dark", State::WorkChip, true, langid!("en-US")),
        c("work-chip-he-light", State::WorkChip, false, langid!("he")),
        c("work-light", State::Work, false, langid!("en-US")),
        c("work-dark", State::Work, true, langid!("en-US")),
        c("work-he-light", State::Work, false, langid!("he")),
        c("work-he-dark", State::Work, true, langid!("he")),
        c(
            "work-text-2-light",
            State::WorkLargeText,
            false,
            langid!("en-US"),
        ),
    ]
}
