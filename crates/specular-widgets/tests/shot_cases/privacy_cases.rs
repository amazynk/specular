use super::*;

/// The panel under Settings › Privacy › No network as the app builds it (`app/privacy.rs`)
/// when the system policy holds the network off.
pub(crate) fn privacy_panel() -> SettingPanel {
    let row = |id: &str, label: String, attention: bool| PanelRow {
        id: id.into(),
        label,
        attention,
        ..PanelRow::default()
    };
    SettingPanel {
        title: t!("privacy-off-title"),
        rows: vec![
            row(
                "held",
                t!(
                    "privacy-held-policy",
                    file = isolate_ltr("/etc/specular/policy.toml")
                ),
                true,
            ),
            row("what", t!("privacy-off-what"), false),
        ],
        ..SettingPanel::default()
    }
}

/// This feature's goldens ([`super::cases`]): Settings › Privacy with the network off, and
/// the renderer information sheet with stable made-up adapter facts.
pub(crate) fn privacy_shot_cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    vec![
        c(
            "privacy-off-dark",
            State::PrivacyOff,
            true,
            langid!("en-US"),
        ),
        c(
            "privacy-off-light",
            State::PrivacyOff,
            false,
            langid!("en-US"),
        ),
        c(
            "privacy-off-he-dark",
            State::PrivacyOff,
            true,
            langid!("he"),
        ),
        c(
            "renderer-info-dark",
            State::RendererInfo,
            true,
            langid!("en-US"),
        ),
        c(
            "renderer-info-he-dark",
            State::RendererInfo,
            true,
            langid!("he"),
        ),
        c(
            "renderer-info-display-dark",
            State::RendererInfoDisplay,
            true,
            langid!("en-US"),
        ),
    ]
}
