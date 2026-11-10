use super::*;

/// The panel under Settings › Devices › Look again as the app builds it (`app/devices.rs`):
/// a gamepad being read and a SpaceMouse the account may not read, with the gamepad's
/// sticks and default buttons.
pub(crate) fn devices_panel() -> SettingPanel {
    let mut rows = vec![
        PanelRow {
            id: "problem".into(),
            label: t!(
                "devices-no-permission",
                name = isolate_ltr("3Dconnexion SpaceMouse Pro")
            ),
            lines: vec![
                t!("devices-no-permission-group", group = isolate_ltr("input")),
                t!("devices-no-permission-spnav"),
            ],
            detail: "/dev/input/event7".into(),
            attention: true,
            ..PanelRow::default()
        },
        PanelRow {
            id: "device:1".into(),
            label: isolate_ltr("Xbox Wireless Controller"),
            value: t!("devices-kind-gamepad"),
            detail: "/dev/input/js0".into(),
            ..PanelRow::default()
        },
        PanelRow {
            id: "Pad-sticks".into(),
            label: t!("devices-sticks-pad"),
            lines: vec![t!("devices-sticks-pad-value")],
            ..PanelRow::default()
        },
        PanelRow {
            id: "Pad-buttons".into(),
            label: t!("devices-buttons-pad"),
            ..PanelRow::default()
        },
    ];
    for (k, a) in specular_input::DEVICE_KEYS
        .iter()
        .filter(|(k, _)| k.starts_with("Pad-"))
    {
        let c = KeyCombo::parse(k).expect("device key");
        rows.push(PanelRow {
            id: format!("button:{k}"),
            label: isolate_ltr(&c.to_string()),
            value: a.name(),
            indent: 1,
            ..PanelRow::default()
        });
    }
    SettingPanel {
        intro: t!("devices-intro"),
        rows,
        keywords: t!("devices-keywords"),
        ..SettingPanel::default()
    }
}

/// This feature's goldens ([`super::cases`]): Settings › Devices, in English and Hebrew.
pub(crate) fn devices_shot_cases() -> Vec<Case> {
    let c = |name, dark, lang| Case {
        name,
        state: State::Devices,
        dark,
        lang,
    };
    vec![
        c("settings-devices-dark", true, langid!("en-US")),
        c("settings-devices-light", false, langid!("en-US")),
        c("he-settings-devices-dark", true, langid!("he")),
    ]
}
