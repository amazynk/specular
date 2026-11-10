//! The equaliser's panel: Develop's panel ([`crate::develop_panel`]) with one card, its
//! switch the equaliser's, holding the preset, the preamp, the ten bands (±12 dB) and the
//! buttons to save the bands as a preset and to delete the chosen one. Only the Close
//! button in the header, no history.

use specular_i18n::t;

use crate::choice::Choice;
use crate::develop_panel::{Body, Card, Control, DevelopPanel, PanelButton};
use crate::slider::{Slider, SliderSpec, Unit};

/// The card's id and the controls' ids (bands: [`BAND`] + 0..10).
pub const CARD: u32 = 1;
pub const PRESET: u32 = 1;
pub const PREAMP: u32 = 2;
pub const SAVE: u32 = 3;
pub const DELETE: u32 = 4;
pub const BAND: u32 = 10;

/// A band's and the preamp's range (dB, either way).
pub const MAX_DB: f32 = 12.0;

/// What the panel shows.
#[derive(Clone, Debug, PartialEq)]
pub struct EqView {
    pub on: bool,
    /// The presets' names, and which is chosen.
    pub presets: Vec<String>,
    pub preset: usize,
    pub preamp: f32,
    /// The bands' labels ([`band_label`]) and gains.
    pub labels: Vec<String>,
    pub bands: Vec<f32>,
    /// The chosen preset is the user's (it can be deleted).
    pub can_delete: bool,
}

fn db_slider(label: String, value: f32) -> Slider {
    let mut s = Slider::new(
        SliderSpec::new(label, -MAX_DB, MAX_DB, 0.0)
            .steps(0.5, 3.0)
            .unit(Unit::Decibels, 1)
            .signed(),
    );
    s.value = value.clamp(-MAX_DB, MAX_DB);
    s
}

/// A band's name: "31 Hz", "1 kHz" (whole numbers, in the UI language's digits).
pub fn band_label(hz: f64) -> String {
    let l = specular_i18n::I18n::global().localizer();
    if hz >= 1000.0 {
        t!("eq-band-khz", band = l.number((hz / 1000.0).round(), 0))
    } else {
        t!("eq-band-hz", band = l.number(hz.floor(), 0))
    }
}

/// The panel for `v`.
pub fn panel(v: &EqView) -> DevelopPanel {
    let mut preset = Choice::new(t!("eq-preset"), v.presets.clone());
    preset.selected = v.preset.min(v.presets.len().saturating_sub(1));
    let mut controls = vec![
        Control {
            id: PRESET,
            body: Body::Choice(preset),
        },
        Control {
            id: PREAMP,
            body: Body::Slider(db_slider(t!("eq-preamp"), v.preamp)),
        },
    ];
    for (i, (l, g)) in v.labels.iter().zip(&v.bands).enumerate() {
        controls.push(Control {
            id: BAND + i as u32,
            body: Body::Slider(db_slider(l.clone(), *g)),
        });
    }
    controls.push(Control {
        id: SAVE,
        body: Body::Choice(Choice::button(t!("eq-save-preset"))),
    });
    if v.can_delete {
        controls.push(Control {
            id: DELETE,
            body: Body::Choice(Choice::button(t!("eq-delete-preset"))),
        });
    }
    let mut p = DevelopPanel::new(
        t!("eq-title"),
        vec![Card {
            id: CARD,
            title: t!("eq-on"),
            enabled: Some(v.on),
            expanded: true,
            controls,
        }],
    );
    p.buttons = vec![PanelButton::Close];
    p.show_history = false;
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop_panel::Item;

    #[test]
    fn band_names() {
        let s = |hz| specular_i18n::strip_isolates(&band_label(hz));
        assert_eq!(s(31.25), "31 Hz");
        assert_eq!(s(16000.0), "16 kHz");
    }

    #[test]
    fn close_only_and_no_history() {
        let p = panel(&EqView {
            on: true,
            presets: vec!["Flat".into(), "Rock".into()],
            preset: 1,
            preamp: 0.0,
            labels: (0..10).map(|i| i.to_string()).collect(),
            bands: vec![1.0; 10],
            can_delete: false,
        });
        let order = p.order();
        assert_eq!(order[0], Item::Button(PanelButton::Close));
        assert!(!order.contains(&Item::Button(PanelButton::Undo)));
        // Close, the card's header and switch, preset, preamp, ten bands, save.
        assert_eq!(order.len(), 3 + 2 + 10 + 1);
        assert!(!order.contains(&Item::Header(1)));
    }
}
