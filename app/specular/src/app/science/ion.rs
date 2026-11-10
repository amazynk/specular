//! Ion images of mass spectrometry images (imzML): an m/z typed in a prompt (opened by
//! [`specular_input::Action::ScienceIonImage`] on the base peak of the pixel under the
//! pointer) shows that ion's image instead of the total ion current. The tolerance is typed
//! after the m/z ("760.585 5 ppm", "184.07 0.01 Da") or stepped with Tab, and kept across
//! files.

use specular_decode::science::{IonTolerance, IonWindow, MassSpec, ScienceSummary};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};

use super::super::App;
use super::trimmed;

/// The tolerances Tab steps through.
const TOLERANCES: [IonTolerance; 8] = [
    IonTolerance::Ppm(5.0),
    IonTolerance::Ppm(10.0),
    IonTolerance::Ppm(20.0),
    IonTolerance::Ppm(50.0),
    IonTolerance::Da(0.01),
    IonTolerance::Da(0.05),
    IonTolerance::Da(0.1),
    IonTolerance::Da(0.5),
];

/// The tolerance until one is chosen: pyimzML's default, wide enough for time-of-flight data.
pub(super) const DEFAULT_TOLERANCE: IonTolerance = IonTolerance::Da(0.1);

/// Longest prompt text.
const MAX_TEXT: usize = 40;

/// "10 ppm", "0.1 Da".
pub(super) fn tolerance_text(t: IonTolerance) -> String {
    match t {
        IonTolerance::Ppm(p) => format!("{} ppm", trimmed(p, 3)),
        IonTolerance::Da(d) => format!("{} Da", trimmed(d, 5)),
    }
}

/// "760.5851 ± 10 ppm".
pub(super) fn window_text(w: IonWindow) -> String {
    format!("{} ± {}", trimmed(w.mz, 5), tolerance_text(w.tolerance))
}

/// An m/z window from typed text: the m/z, then optionally the tolerance with its unit
/// ("ppm", "Da" or "mDa"; without one, the unit of `default`). `None` when it is not one.
pub(super) fn parse_window(text: &str, default: IonTolerance) -> Option<IonWindow> {
    // Numbers and units apart: "5ppm" → "5 ppm"; "±" and "+-" are only separators.
    let mut norm = String::new();
    let mut prev_digit = false;
    for c in text.trim().to_lowercase().chars() {
        let c = match c {
            ',' => '.',
            '±' | '+' | '/' => ' ',
            c => c,
        };
        if c.is_ascii_alphabetic() && prev_digit {
            norm.push(' ');
        }
        prev_digit = c.is_ascii_digit() || c == '.';
        norm.push(c);
    }
    let norm = norm.replace(" -", " ");
    let mut tokens = norm.split_whitespace();
    let positive = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0);
    let mz = positive(tokens.next()?)?;
    let tolerance = match tokens.next() {
        None => default,
        Some(v) => {
            let v = positive(v)?;
            match tokens.next() {
                Some("ppm") => IonTolerance::Ppm(v),
                Some("da" | "u" | "th") => IonTolerance::Da(v),
                Some("mda") => IonTolerance::Da(v / 1000.0),
                Some(_) => return None,
                None => match default {
                    IonTolerance::Ppm(_) => IonTolerance::Ppm(v),
                    IonTolerance::Da(_) => IonTolerance::Da(v),
                },
            }
        }
    };
    if tokens.next().is_some() {
        return None;
    }
    let w = IonWindow { mz, tolerance };
    (w.half_width().is_finite() && w.half_width() < mz).then_some(w)
}

/// The tolerance after `t` in [`TOLERANCES`] (before it with `back`).
fn next_tolerance(t: IonTolerance, back: bool) -> IonTolerance {
    let n = TOLERANCES.len();
    let i = TOLERANCES.iter().position(|&c| c == t);
    let j = match (i, back) {
        (Some(i), false) => (i + 1) % n,
        (Some(i), true) => (i + n - 1) % n,
        (None, _) => 0,
    };
    TOLERANCES[j]
}

/// The readout's lines for the spot under the pointer: its position as the file numbers it,
/// or that it has no spectrum.
pub(super) fn spot_lines(ms: &MassSpec, index: &[u32], x: u32, y: u32) -> Vec<String> {
    if !ms.has_spectrum(x, y, index) {
        return vec![t!("science-ms-no-spectrum")];
    }
    let (fx, fy, fz) = ms.position(x, y, index);
    let multi_z = index.len() == 1;
    vec![if multi_z {
        t!(
            "science-ms-spot-z",
            x = i64::from(fx),
            y = i64::from(fy),
            z = i64::from(fz)
        )
    } else {
        t!("science-ms-spot", x = i64::from(fx), y = i64::from(fy))
    }]
}

impl App {
    /// The m/z prompt (or a thermal image's object parameters prompt) is open: it takes the
    /// keys.
    pub(in crate::app) fn science_prompting(&self) -> bool {
        self.science.ion_prompt.is_some() || self.science.thermal_prompt.is_some()
    }

    fn mass_spec(&self) -> Option<(&ScienceSummary, &MassSpec)> {
        let s = self
            .science
            .summary
            .as_ref()
            .filter(|_| self.science.active())?;
        Some((s, s.mass_spec.as_deref()?))
    }

    /// Opens the m/z prompt, on the base peak of the pixel under the pointer.
    pub(super) fn science_ion_open(&mut self) {
        let Some((s, ms)) = self.mass_spec() else {
            return;
        };
        let at = self
            .pointer
            .filter(|p| super::super::rect_contains(self.view.viewport, *p))
            .map(|p| self.view.stored_from_screen().apply(p))
            .filter(|&(x, y)| x >= 0.0 && y >= 0.0 && x < s.width as f32 && y < s.height as f32);
        let text = at
            .and_then(|(x, y)| ms.base_peak(x as u32, y as u32, &self.science.index))
            .map(|m| trimmed(m, 4))
            .unwrap_or_default();
        self.science.ion_prompt = Some(text);
        self.science_readout();
        self.redraw = true;
    }

    /// A key while the m/z prompt is open: characters of the m/z and tolerance, Backspace,
    /// Tab (the next tolerance), Enter (show it; empty: the total ion current again), Esc.
    pub(in crate::app) fn science_ion_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) {
        if self.science.thermal_prompt.is_some() {
            self.science_thermal_key(combo, text);
            return;
        }
        let Some(combo) = combo else { return };
        let plain = !combo.mods.ctrl && !combo.mods.alt && !combo.mods.meta;
        match &combo.key {
            Key::Named(NamedKey::Escape) => self.science.ion_prompt = None,
            Key::Named(NamedKey::Enter) => self.science_ion_apply(),
            Key::Named(NamedKey::Backspace) => {
                if let Some(p) = self.science.ion_prompt.as_mut() {
                    p.pop();
                }
            }
            Key::Named(NamedKey::Tab) => {
                self.science.ion_tolerance =
                    next_tolerance(self.science.ion_tolerance, combo.mods.shift);
            }
            _ if plain => {
                if let (Some(p), Some(text)) = (self.science.ion_prompt.as_mut(), text) {
                    for c in text.chars() {
                        let ok = c.is_ascii_digit()
                            || matches!(c, '.' | ',' | ' ' | '±' | '+' | '-')
                            || "pmdaPMDA".contains(c);
                        if ok && p.chars().count() < MAX_TEXT {
                            p.push(c);
                        }
                    }
                }
            }
            _ => {}
        }
        self.science_readout();
        self.redraw = true;
    }

    /// Enter in the prompt: the typed window's ion image, or the total ion current again
    /// when nothing is typed.
    fn science_ion_apply(&mut self) {
        let text = self.science.ion_prompt.clone().unwrap_or_default();
        let now = self.now();
        if text.trim().is_empty() {
            self.science.ion_prompt = None;
            self.science.ion = None;
            self.chrome.toast(t!("science-ion-tic"), None, now);
            self.science_restyle();
            return;
        }
        match parse_window(&text, self.science.ion_tolerance) {
            Some(w) => {
                self.science.ion_prompt = None;
                self.science.ion_tolerance = w.tolerance;
                self.science.ion = Some(w);
                let msg = t!("science-ion-window", window = window_text(w));
                self.chrome.toast(msg, None, now);
                self.science_restyle();
            }
            None => {
                let msg = t!("science-ion-bad", text = text.trim().to_owned());
                self.chrome.toast(msg, None, now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_windows() {
        let ppm = IonTolerance::Ppm(10.0);
        let da = IonTolerance::Da(0.1);
        let w = |mz, tolerance| Some(IonWindow { mz, tolerance });
        assert_eq!(parse_window("760.585", ppm), w(760.585, ppm));
        assert_eq!(parse_window(" 760,585 ", da), w(760.585, da));
        assert_eq!(
            parse_window("760.585 5 ppm", da),
            w(760.585, IonTolerance::Ppm(5.0))
        );
        assert_eq!(
            parse_window("760.585±5ppm", da),
            w(760.585, IonTolerance::Ppm(5.0))
        );
        assert_eq!(
            parse_window("184.07 +- 0.01 Da", ppm),
            w(184.07, IonTolerance::Da(0.01))
        );
        assert_eq!(
            parse_window("184.07 20 mDa", ppm),
            w(184.07, IonTolerance::Da(0.02))
        );
        assert_eq!(
            parse_window("184.07 0.5", da),
            w(184.07, IonTolerance::Da(0.5))
        );
        assert_eq!(
            parse_window("184.07 3", ppm),
            w(184.07, IonTolerance::Ppm(3.0))
        );
        for bad in [
            "",
            "ppm",
            "-5",
            "0",
            "184.07 ppm",
            "184.07 5 kg",
            "1 2 3",
            "10 20 Da",
        ] {
            assert_eq!(parse_window(bad, ppm), None, "{bad}");
        }
    }

    #[test]
    fn tolerance_steps_and_text() {
        assert_eq!(
            next_tolerance(IonTolerance::Da(0.5), false),
            IonTolerance::Ppm(5.0)
        );
        assert_eq!(
            next_tolerance(IonTolerance::Ppm(5.0), true),
            IonTolerance::Da(0.5)
        );
        assert_eq!(
            next_tolerance(IonTolerance::Ppm(7.0), false),
            IonTolerance::Ppm(5.0)
        );
        assert_eq!(tolerance_text(IonTolerance::Ppm(10.0)), "10 ppm");
        assert_eq!(tolerance_text(IonTolerance::Da(0.01)), "0.01 Da");
        let w = IonWindow {
            mz: 760.5851,
            tolerance: IonTolerance::Ppm(5.0),
        };
        assert_eq!(window_text(w), "760.5851 ± 5 ppm");
    }
}
