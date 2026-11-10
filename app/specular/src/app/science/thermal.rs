//! Thermal stills (FLIR, InfiRay, InfraTec): temperatures under the pointer in °C, °F or K
//! (from the settings' units or the locale until one is chosen with
//! [`Action::ScienceTemperatureUnit`]), an isotherm painting every pixel above or below the
//! temperature under the pointer ([`Action::ScienceIsotherm`]), the object parameters
//! (emissivity, reflected temperature) typed in a prompt and the temperatures recomputed
//! ([`Action::ScienceThermalParameters`]), the temperature grid saved as float32 TIFF or CSV
//! ([`Action::ScienceExportTemperatures`]) and the camera's visible photo
//! ([`Action::ToggleThermalVisible`], the file's second page).
//!
//! [`Action::ScienceTemperatureUnit`]: specular_input::Action::ScienceTemperatureUnit
//! [`Action::ScienceIsotherm`]: specular_input::Action::ScienceIsotherm
//! [`Action::ScienceThermalParameters`]: specular_input::Action::ScienceThermalParameters
//! [`Action::ScienceExportTemperatures`]: specular_input::Action::ScienceExportTemperatures
//! [`Action::ToggleThermalVisible`]: specular_input::Action::ToggleThermalVisible

use std::sync::Arc;

use specular_decode::PageLabel;
use specular_decode::science::{
    self as dsci, NdPlane, TemperatureUnit, ThermalConditions, ThermalImage,
};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_settings::Units;

use super::super::App;
use super::{Science, trimmed};

/// The isotherm's colour when the camera gives none.
const ISOTHERM_GREEN: [u8; 3] = [0, 230, 64];

/// Longest prompt text.
const MAX_TEXT: usize = 12;

/// Regions that measure temperatures in degrees Fahrenheit.
const FAHRENHEIT_REGIONS: [&str; 7] = ["US", "LR", "MM", "BS", "BZ", "KY", "PW"];

/// The unit temperatures show in until one is chosen: the settings' units, else the
/// region's custom.
pub(super) fn default_unit(units: Units, region: Option<&str>) -> TemperatureUnit {
    match units {
        Units::Imperial => TemperatureUnit::Fahrenheit,
        Units::Metric => TemperatureUnit::Celsius,
        Units::Auto => {
            let r = region.unwrap_or("").to_ascii_uppercase();
            if FAHRENHEIT_REGIONS.contains(&r.as_str()) {
                TemperatureUnit::Fahrenheit
            } else {
                TemperatureUnit::Celsius
            }
        }
    }
}

/// The parameter the prompt edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Emissivity,
    Reflected,
}

/// The object parameters prompt: both values as typed, and the one being edited.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Prompt {
    pub field: Field,
    pub emissivity: String,
    pub reflected: String,
}

impl Prompt {
    fn text_mut(&mut self) -> &mut String {
        match self.field {
            Field::Emissivity => &mut self.emissivity,
            Field::Reflected => &mut self.reflected,
        }
    }
}

/// Emissivity from typed text: (0, 1].
pub(super) fn parse_emissivity(text: &str) -> Option<f64> {
    let v: f64 = text.trim().replace(',', ".").parse().ok()?;
    (v.is_finite() && v > 0.0 && v <= 1.0).then_some(v)
}

/// A temperature typed in `unit`, in degrees Celsius; above absolute zero.
pub(super) fn parse_temperature(text: &str, unit: TemperatureUnit) -> Option<f64> {
    let v: f64 = text.trim().replace(',', ".").parse().ok()?;
    let c = unit.to_celsius(v);
    (c.is_finite() && c > -273.15).then_some(c)
}

/// "21.5 °C".
pub(super) fn temperature_text(celsius: f64, unit: TemperatureUnit) -> String {
    format!(
        "{} {}",
        trimmed(unit.from_celsius(celsius), 2),
        unit.symbol()
    )
}

/// The isotherm after `cur`: above, below, off.
pub(super) fn next_isotherm(cur: Option<bool>) -> Option<bool> {
    match cur {
        None => Some(true),
        Some(true) => Some(false),
        Some(false) => None,
    }
}

impl Science {
    /// The thermal image shown, when it is one.
    pub(super) fn thermal(&self) -> Option<&Arc<ThermalImage>> {
        self.summary.as_ref().and_then(|s| s.thermal.as_ref())
    }

    /// The unit temperatures show in now: the one chosen, else the default.
    pub(super) fn unit_now(&self) -> TemperatureUnit {
        self.unit_chosen.unwrap_or(self.unit_default)
    }

    /// The object parameters temperatures are computed under: the edited ones, else the
    /// file's.
    pub(super) fn conditions_now(&self) -> Option<ThermalConditions> {
        let t = self.thermal()?;
        Some(self.conditions.unwrap_or_else(|| t.file_conditions()))
    }

    /// The readout's thermal lines: the prompt, the object parameters and the isotherm.
    pub(super) fn thermal_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let Some(t) = self.thermal() else {
            return lines;
        };
        let unit = self.unit_now();
        if let Some(p) = &self.thermal_prompt {
            let cursor = |f: Field, s: &str| {
                if p.field == f {
                    format!("{s}▏")
                } else {
                    s.to_owned()
                }
            };
            lines.push(t!(
                "science-thermal-emissivity-prompt",
                text = cursor(Field::Emissivity, &p.emissivity)
            ));
            lines.push(t!(
                "science-thermal-reflected-prompt",
                text = cursor(Field::Reflected, &p.reflected),
                unit = unit.symbol()
            ));
            lines.push(t!("science-thermal-prompt-hint"));
        }
        if t.adjustable()
            && let Some(c) = self.conditions_now()
        {
            // A sequence frame's own parameters unless edited.
            let c = t.effective_conditions(self.index.first().copied().unwrap_or(0), &c);
            lines.push(t!(
                "science-thermal-conditions",
                emissivity = trimmed(c.emissivity.unwrap_or(1.0), 3),
                reflected = c
                    .reflected
                    .map_or_else(|| "–".to_owned(), |r| temperature_text(r, unit))
            ));
        }
        if let Some((above, level)) = self.isotherm {
            let level = temperature_text(level, unit);
            lines.push(if above {
                t!("science-isotherm-above", level = level)
            } else {
                t!("science-isotherm-below", level = level)
            });
        }
        lines
    }

    /// The isotherm drawn, in the shown plane's unit.
    pub(super) fn isotherm_look(&self) -> Option<specular_render::Isotherm> {
        let (above, level) = self.isotherm?;
        let (_, unit) = self.shown.as_ref()?.thermal?;
        let c = self
            .thermal()
            .and_then(|t| t.isotherm_colour())
            .unwrap_or(ISOTHERM_GREEN);
        Some(specular_render::Isotherm {
            above,
            level: unit.from_celsius(level) as f32,
            colour: c.map(|v| f32::from(v) / 255.0),
        })
    }
}

impl App {
    /// The unit temperatures show in when none was chosen: the settings', else the region's.
    pub(super) fn temperature_unit_default(&self) -> TemperatureUnit {
        let lang = I18n::global().localizer().format_language().clone();
        let region = lang.region.as_ref().map(|r| r.as_str().to_owned());
        default_unit(self.settings.metadata.units, region.as_deref())
    }

    /// The pointer's position in stored pixels, on the image.
    fn science_pointer_px(&self) -> Option<(u32, u32)> {
        let s = self.science.summary.as_ref()?;
        self.pointer
            .filter(|p| super::super::rect_contains(self.view.viewport, *p))
            .map(|p| self.view.stored_from_screen().apply(p))
            .filter(|&(x, y)| x >= 0.0 && y >= 0.0 && x < s.width as f32 && y < s.height as f32)
            .map(|(x, y)| (x as u32, y as u32))
    }

    /// The temperature (°C) shown at the pixel under the pointer.
    fn temperature_under_pointer(&self) -> Option<f64> {
        let (x, y) = self.science_pointer_px()?;
        let (_, unit) = self.science.shown.as_ref()?.thermal?;
        let v = self.science.planes.first()?.plane.value(x, y)?;
        v.is_finite().then(|| unit.to_celsius(f64::from(v)))
    }

    /// The unit temperatures show in now, for any image (the chosen one is kept across
    /// images): the readout's and the info panel's.
    pub(in crate::app) fn science_temperature_unit_now(&self) -> TemperatureUnit {
        self.science
            .unit_chosen
            .unwrap_or_else(|| self.temperature_unit_default())
    }

    /// The next unit (°C, °F, K).
    pub(super) fn science_temperature_unit(&mut self) {
        if self.science.thermal().is_none() {
            return;
        }
        let u = self.science.unit_now().next();
        self.science.unit_chosen = Some(u);
        let now = self.now();
        self.chrome.toast(
            t!("science-temperature-unit-toast", unit = u.symbol()),
            None,
            now,
        );
        self.science_restyle();
        // The info panel's temperatures follow.
        self.request_metadata();
    }

    /// The next isotherm (above, below, off), at the temperature under the pointer, else
    /// the middle of the limits.
    pub(super) fn science_isotherm(&mut self) {
        if self.science.thermal().is_none() {
            return;
        }
        let next = next_isotherm(self.science.isotherm.map(|(a, _)| a));
        let level = self
            .temperature_under_pointer()
            .or_else(|| {
                let (_, unit) = self.science.shown.as_ref()?.thermal?;
                let (lo, hi) = self.science.planes.first()?.limits?;
                Some(unit.to_celsius(f64::from(lo + hi) / 2.0))
            })
            .or(self.science.isotherm.map(|(_, l)| l));
        self.science.isotherm = next.zip(level);
        let unit = self.science.unit_now();
        let msg = match self.science.isotherm {
            Some((true, l)) => t!("science-isotherm-above", level = temperature_text(l, unit)),
            Some((false, l)) => t!("science-isotherm-below", level = temperature_text(l, unit)),
            None => t!("science-isotherm-off"),
        };
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.science_restyle();
    }

    /// Opens the object parameters prompt on the parameters in effect.
    pub(super) fn science_thermal_open(&mut self) {
        let Some(c) = self
            .science
            .thermal()
            .filter(|t| t.adjustable())
            .and(self.science.conditions_now())
        else {
            return;
        };
        let unit = self.science.unit_now();
        self.science.thermal_prompt = Some(Prompt {
            field: Field::Emissivity,
            emissivity: trimmed(c.emissivity.unwrap_or(1.0), 3),
            reflected: c
                .reflected
                .map(|r| trimmed(unit.from_celsius(r), 2))
                .unwrap_or_default(),
        });
        self.science_readout();
        self.redraw = true;
    }

    /// A key while the object parameters prompt is open: digits, Backspace, Tab (the other
    /// field), Enter (recompute), Esc.
    pub(in crate::app) fn science_thermal_key(
        &mut self,
        combo: Option<&KeyCombo>,
        text: Option<&str>,
    ) {
        let Some(combo) = combo else { return };
        let plain = !combo.mods.ctrl && !combo.mods.alt && !combo.mods.meta;
        match &combo.key {
            Key::Named(NamedKey::Escape) => self.science.thermal_prompt = None,
            Key::Named(NamedKey::Enter) => self.science_thermal_apply(),
            Key::Named(NamedKey::Backspace) => {
                if let Some(p) = self.science.thermal_prompt.as_mut() {
                    p.text_mut().pop();
                }
            }
            Key::Named(NamedKey::Tab) => {
                if let Some(p) = self.science.thermal_prompt.as_mut() {
                    p.field = match p.field {
                        Field::Emissivity => Field::Reflected,
                        Field::Reflected => Field::Emissivity,
                    };
                }
            }
            _ if plain => {
                if let (Some(p), Some(text)) = (self.science.thermal_prompt.as_mut(), text) {
                    let s = p.text_mut();
                    for c in text.chars() {
                        let ok = c.is_ascii_digit() || matches!(c, '.' | ',' | '-');
                        if ok && s.chars().count() < MAX_TEXT {
                            s.push(c);
                        }
                    }
                }
            }
            _ => {}
        }
        self.science_readout();
        self.redraw = true;
    }

    /// Enter in the prompt: the temperatures recomputed under the typed parameters.
    fn science_thermal_apply(&mut self) {
        let Some(p) = self.science.thermal_prompt.clone() else {
            return;
        };
        let Some(base) = self.science.conditions_now() else {
            self.science.thermal_prompt = None;
            return;
        };
        let unit = self.science.unit_now();
        let now = self.now();
        let e = parse_emissivity(&p.emissivity);
        let r = if p.reflected.trim().is_empty() {
            Some(base.reflected)
        } else {
            parse_temperature(&p.reflected, unit).map(Some)
        };
        match (e, r) {
            (Some(e), Some(r)) => {
                self.science.thermal_prompt = None;
                self.science.conditions = Some(ThermalConditions {
                    emissivity: Some(e),
                    reflected: r,
                    ..base
                });
                let msg = t!(
                    "science-thermal-applied",
                    emissivity = trimmed(e, 3),
                    reflected = r.map_or_else(|| "–".to_owned(), |r| temperature_text(r, unit))
                );
                self.chrome.toast(msg, None, now);
                self.science_restyle();
            }
            _ => {
                let bad = if e.is_none() {
                    p.emissivity.trim().to_owned()
                } else {
                    p.reflected.trim().to_owned()
                };
                self.chrome
                    .toast(t!("science-thermal-bad", text = bad), None, now);
            }
        }
    }

    /// Asks where to save the temperature grid shown (float32 TIFF, or CSV by the name's
    /// extension) and writes it on the dialog's thread.
    pub(super) fn science_export_temperatures(&mut self) {
        let Some(pl) = self.science.planes.first() else {
            return;
        };
        let Some((cond, unit)) = self.science.shown.as_ref().and_then(|r| r.thermal) else {
            return;
        };
        let plane: Arc<NdPlane> = pl.plane.clone();
        let mut stem = self
            .science
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map_or_else(
                || "temperatures".to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
        // A sequence: the frame shown, under its own parameters unless edited.
        let cond = match self.science.thermal() {
            Some(t) if t.frame_count() > 1 => {
                let frame = self.science.index.first().copied().unwrap_or(0);
                stem = format!("{stem}-frame{}", frame + 1);
                t.effective_conditions(frame, &cond)
            }
            _ => cond,
        };
        let description = format!(
            "Temperatures in {} (emissivity {}, reflected {})",
            unit.symbol(),
            trimmed(cond.emissivity.unwrap_or(1.0), 3),
            cond.reflected
                .map_or_else(|| "-".to_owned(), |r| temperature_text(r, unit))
        );
        let (tx, rx) = crossbeam_channel::bounded(1);
        let tiff = t!("science-thermal-export-tiff");
        let csv = t!("science-thermal-export-csv");
        self.loader.background("dlg-therm-save", move |l| {
            let Some(path) = rfd::FileDialog::new()
                .add_filter(tiff, &["tif", "tiff"])
                .add_filter(csv, &["csv"])
                .set_file_name(format!("{stem}.tif"))
                .save_file()
            else {
                return;
            };
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let is_csv = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
            let (w, h) = (plane.width as usize, plane.height as usize);
            let bytes = if is_csv {
                dsci::temperature_csv(w, &plane.values, 3).into_bytes()
            } else {
                dsci::temperature_tiff(w, h, &plane.values, &description)
            };
            let r = std::fs::write(&path, bytes)
                .map(|()| name)
                .map_err(|e| crate::error_text::io_reason(&e));
            let _ = tx.send(r);
            l.waker()();
        });
        self.science.export_rx = Some(rx);
    }

    /// Reports a finished export.
    pub(super) fn science_export_poll(&mut self) {
        let Some(Ok(r)) = self.science.export_rx.as_ref().map(|r| r.try_recv()) else {
            return;
        };
        self.science.export_rx = None;
        let msg = match r {
            Ok(name) => t!("science-thermal-export-done", name = name),
            Err(e) => t!("science-thermal-export-failed", reason = e),
        };
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The temperatures shown and their unit, for the measurement tools' statistics: only
    /// for a thermal image in science mode.
    pub(in crate::app) fn science_measure_values(&self) -> Option<(Arc<NdPlane>, String)> {
        let sc = &self.science;
        if !sc.active() || sc.id != self.current_id() || sc.planes.len() != 1 {
            return None;
        }
        let (_, unit) = sc.shown.as_ref()?.thermal?;
        Some((sc.planes[0].plane.clone(), unit.symbol().to_owned()))
    }

    /// The current image is a thermal still with a visible photo, or that photo.
    pub(in crate::app) fn thermal_visible_capable(&self) -> bool {
        self.current_id()
            .and_then(|id| self.images.get(&id))
            .is_some_and(|i| matches!(i.page_label, Some(PageLabel::Thermal { .. })) && i.pages > 1)
    }

    /// Shows the visible photo, or the temperatures again.
    pub(super) fn thermal_visible_toggle(&mut self) {
        if !self.thermal_visible_capable() {
            return;
        }
        let Some(id) = self.current_id() else { return };
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let to_visible = img.page_label == Some(PageLabel::Thermal { visible: false });
        self.page_switch(id, u32::from(to_visible));
        let now = self.now();
        let msg = if to_visible {
            t!("page-label-visible")
        } else {
            t!("page-label-thermal")
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_from_settings_and_region() {
        let c = TemperatureUnit::Celsius;
        let f = TemperatureUnit::Fahrenheit;
        assert_eq!(default_unit(Units::Imperial, Some("DE")), f);
        assert_eq!(default_unit(Units::Metric, Some("US")), c);
        assert_eq!(default_unit(Units::Auto, Some("US")), f);
        assert_eq!(default_unit(Units::Auto, Some("us")), f);
        assert_eq!(default_unit(Units::Auto, Some("PW")), f);
        assert_eq!(default_unit(Units::Auto, Some("GB")), c);
        assert_eq!(default_unit(Units::Auto, None), c);
    }

    #[test]
    fn typed_parameters() {
        assert_eq!(parse_emissivity("0.95"), Some(0.95));
        assert_eq!(parse_emissivity(" 0,8 "), Some(0.8));
        assert_eq!(parse_emissivity("1"), Some(1.0));
        for bad in ["", "0", "-0.5", "1.01", "x"] {
            assert_eq!(parse_emissivity(bad), None, "{bad}");
        }
        let f = TemperatureUnit::Fahrenheit;
        let k = TemperatureUnit::Kelvin;
        assert!((parse_temperature("68", f).unwrap() - 20.0).abs() < 1e-9);
        assert!((parse_temperature("293.15", k).unwrap() - 20.0).abs() < 1e-9);
        assert_eq!(parse_temperature("-300", TemperatureUnit::Celsius), None);
        assert_eq!(parse_temperature("0", k), None);
        assert_eq!(temperature_text(20.0, f), "68 °F");
        assert_eq!(temperature_text(21.5, TemperatureUnit::Celsius), "21.5 °C");
    }

    #[test]
    fn isotherm_cycles_above_below_off() {
        assert_eq!(next_isotherm(None), Some(true));
        assert_eq!(next_isotherm(Some(true)), Some(false));
        assert_eq!(next_isotherm(Some(false)), None);
    }
}
