//! A value slider (Develop's controls, DESIGN.md §6.10): a label and the value on one line,
//! the track under them.
//!
//! - **Drag** the thumb to move the value relative to where it was; press on the track away
//!   from the thumb to jump there first. Shift while dragging moves ten times slower.
//! - **Keys:** arrows step (Up and the reading direction's forward arrow raise it), Shift
//!   steps coarsely, Alt or Ctrl finely; Page Up and Page Down step coarsely; Home and End go
//!   to the ends; Delete or Backspace resets.
//! - **Typing:** Enter, a digit or a sign starts typing a value; Enter sets it, Esc cancels.
//! - **Double-click** resets to the default.
//!
//! In right-to-left languages the track runs from right to left (its minimum on the right);
//! the number itself stays left to right.

use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::{I18n, isolate_ltr, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, Palette, metrics};

/// Height of a slider row (logical px at the standard text size).
pub const ROW: f32 = 46.0;

/// What a value is shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// A bare number.
    Plain,
    /// Exposure value: "+0.70 EV".
    Ev,
    /// Shown as a percentage (the value times 100).
    Percent,
    /// An angle: "2.5°".
    Degrees,
    /// A colour temperature: "5,000 K".
    Kelvin,
    /// A level: "+3.0 dB".
    Decibels,
}

/// What a slider sets and how.
#[derive(Clone, Debug, PartialEq)]
pub struct SliderSpec {
    /// The control's name, in the UI language.
    pub label: String,
    pub min: f32,
    pub max: f32,
    /// What a reset (double-click, Delete) goes back to; the fill starts here.
    pub default: f32,
    /// One arrow key's step.
    pub step: f32,
    /// Shift+arrow, Page Up and Page Down.
    pub coarse: f32,
    /// Digits after the decimal point shown (of the displayed number).
    pub decimals: u8,
    pub unit: Unit,
    /// A plus sign on positive values (offsets: exposure, contrast).
    pub signed: bool,
    /// Drawn without its label (a narrow column that names it otherwise); the label is
    /// still its accessible name.
    pub compact: bool,
    /// The track drawn as a colour scale from the minimum's colour to the maximum's (what
    /// the value does to the picture: white balance), with no fill.
    pub scale: Option<(Color, Color)>,
}

impl SliderSpec {
    /// A slider over `min..=max` resetting to `default`, stepping by a hundredth of its range.
    pub fn new(label: impl Into<String>, min: f32, max: f32, default: f32) -> SliderSpec {
        let range = (max - min).abs().max(f32::EPSILON);
        SliderSpec {
            label: label.into(),
            min,
            max,
            default,
            step: range / 100.0,
            coarse: range / 10.0,
            decimals: 2,
            unit: Unit::Plain,
            signed: false,
            compact: false,
            scale: None,
        }
    }

    pub fn steps(mut self, step: f32, coarse: f32) -> SliderSpec {
        self.step = step;
        self.coarse = coarse;
        self
    }

    pub fn unit(mut self, unit: Unit, decimals: u8) -> SliderSpec {
        self.unit = unit;
        self.decimals = decimals;
        self
    }

    pub fn signed(mut self) -> SliderSpec {
        self.signed = true;
        self
    }

    pub fn compact(mut self) -> SliderSpec {
        self.compact = true;
        self
    }

    /// The track as a colour scale from `min` (the minimum's end) to `max`.
    pub fn scale(mut self, min: Color, max: Color) -> SliderSpec {
        self.scale = Some((min, max));
        self
    }

    /// The number shown for a value (a percentage is the value times 100).
    fn shown(&self, v: f32) -> f64 {
        let v = f64::from(v);
        if self.unit == Unit::Percent {
            v * 100.0
        } else {
            v
        }
    }
}

/// What a key did to a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyResult {
    /// Not a key of this control.
    Ignored,
    /// Taken, nothing changed (typing, a step at the end of the range).
    Handled,
    /// The value changed.
    Changed,
}

/// A slider's value and its typing state.
#[derive(Clone, Debug, PartialEq)]
pub struct Slider {
    pub spec: SliderSpec,
    pub value: f32,
    /// The text being typed (`None`: not typing).
    typing: Option<String>,
}

/// Where a slider was drawn (logical px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderGeom {
    pub row: Rect,
    pub track: Rect,
    pub thumb: Rect,
}

impl Slider {
    pub fn new(spec: SliderSpec) -> Slider {
        let value = spec.default;
        Slider {
            spec,
            value,
            typing: None,
        }
    }

    /// Sets the value (clamped to the range); whether it changed.
    pub fn set(&mut self, v: f32) -> bool {
        if !v.is_finite() {
            return false;
        }
        let (lo, hi) = self.range();
        let v = v.clamp(lo, hi);
        let changed = v != self.value;
        self.value = v;
        changed
    }

    fn range(&self) -> (f32, f32) {
        let (a, b) = (self.spec.min, self.spec.max);
        if a <= b { (a, b) } else { (b, a) }
    }

    /// Where the value is along the track, 0 at the minimum.
    pub fn fraction(&self) -> f32 {
        self.fraction_of(self.value)
    }

    fn fraction_of(&self, v: f32) -> f32 {
        let (lo, hi) = self.range();
        if hi <= lo {
            return 0.0;
        }
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    }

    /// Back to the default; whether it changed.
    pub fn reset(&mut self) -> bool {
        self.typing = None;
        self.set(self.spec.default)
    }

    /// Moves by `steps` arrow steps (fractions and negatives allowed).
    pub fn nudge(&mut self, steps: f32) -> bool {
        // Land on the step grid so repeated steps read as round numbers.
        let s = self.spec.step.abs().max(f32::EPSILON);
        let v = self.value + steps * s;
        let snapped = if steps.fract() == 0.0 {
            (v / s).round() * s
        } else {
            v
        };
        self.set(snapped)
    }

    /// A drag of `dx` logical px along a track `track_w` long; `fine` (Shift) goes ten times
    /// slower. In right-to-left languages a drag to the left raises the value.
    pub fn drag(&mut self, dx: f32, track_w: f32, fine: bool, rtl: bool) -> bool {
        if track_w <= 0.0 {
            return false;
        }
        let (lo, hi) = self.range();
        let dx = if rtl { -dx } else { dx };
        let k = if fine { 0.1 } else { 1.0 };
        self.set(self.value + dx / track_w * (hi - lo) * k)
    }

    /// The value under `x` on a track drawn at `track`.
    pub fn value_at(&self, x: f32, track: Rect, rtl: bool) -> f32 {
        let (lo, hi) = self.range();
        let f = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        let f = if rtl { 1.0 - f } else { f };
        lo + f * (hi - lo)
    }

    /// Whether a value is being typed.
    pub fn typing(&self) -> Option<&str> {
        self.typing.as_deref()
    }

    /// Ends typing: sets what was typed when `commit` and it reads as a number.
    pub fn end_typing(&mut self, commit: bool) -> bool {
        let Some(s) = self.typing.take() else {
            return false;
        };
        if !commit {
            return false;
        }
        match parse_number(&s) {
            Some(n) => {
                let n = if self.spec.unit == Unit::Percent {
                    n / 100.0
                } else {
                    n
                };
                self.set(n as f32)
            }
            None => false,
        }
    }

    /// The number as typed into the field when typing starts with Enter.
    fn plain_number(&self) -> String {
        let n = self.spec.shown(self.value);
        let s = format!("{:.*}", usize::from(self.spec.decimals), n);
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        }
    }

    /// A key; `rtl` turns the left and right arrows round.
    pub fn key(&mut self, combo: KeyCombo, rtl: bool) -> KeyResult {
        let m = combo.mods;
        if let Some(buf) = self.typing.as_mut() {
            return match combo.key {
                Key::Named(NamedKey::Enter) => {
                    if self.end_typing(true) {
                        KeyResult::Changed
                    } else {
                        KeyResult::Handled
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    self.typing = None;
                    KeyResult::Handled
                }
                Key::Named(NamedKey::Backspace) => {
                    buf.pop();
                    KeyResult::Handled
                }
                Key::Char(c) if typable(c) && !m.ctrl && !m.alt && !m.meta => {
                    if buf.chars().count() < 12 {
                        buf.push(c);
                    }
                    KeyResult::Handled
                }
                // Tab and the like leave the field (the panel commits it).
                Key::Named(NamedKey::Tab) => KeyResult::Ignored,
                _ => KeyResult::Handled,
            };
        }
        let scale = if m.shift {
            self.spec.coarse / self.spec.step.max(f32::EPSILON)
        } else if m.alt || m.ctrl {
            0.1
        } else {
            1.0
        };
        let fwd = if rtl {
            NamedKey::ArrowLeft
        } else {
            NamedKey::ArrowRight
        };
        let back = if rtl {
            NamedKey::ArrowRight
        } else {
            NamedKey::ArrowLeft
        };
        let changed = match combo.key {
            Key::Named(k) if k == fwd || k == NamedKey::ArrowUp => self.nudge(scale),
            Key::Named(k) if k == back || k == NamedKey::ArrowDown => self.nudge(-scale),
            Key::Named(NamedKey::PageUp) => {
                self.nudge(self.spec.coarse / self.spec.step.max(f32::EPSILON))
            }
            Key::Named(NamedKey::PageDown) => {
                self.nudge(-self.spec.coarse / self.spec.step.max(f32::EPSILON))
            }
            Key::Named(NamedKey::Home) => self.set(self.range().0),
            Key::Named(NamedKey::End) => self.set(self.range().1),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) if !m.any() => self.reset(),
            Key::Named(NamedKey::Enter) if !m.any() => {
                self.typing = Some(self.plain_number());
                return KeyResult::Handled;
            }
            Key::Char(c) if typable(c) && !m.ctrl && !m.alt && !m.meta => {
                self.typing = Some(c.to_string());
                return KeyResult::Handled;
            }
            _ => return KeyResult::Ignored,
        };
        if changed {
            KeyResult::Changed
        } else {
            KeyResult::Handled
        }
    }

    /// The value as shown: "+0.70 EV", "25%", "5,000 K", in the UI language's digits.
    pub fn value_text(&self) -> String {
        value_text(&self.spec, self.value)
    }

    /// The room for the label in a row `w` wide (tests; see [`Slider::label_room`]).
    #[cfg(test)]
    fn label_width(&self, text: &mut Text, w: f32, ts: f32) -> f32 {
        self.label_room(text, w, ts).0
    }

    /// The label's room in a row `w` wide, and whether the value goes on its own line
    /// under the label: when the label's widest word is wider than the room beside the
    /// value, the label takes the whole line (a word is never cut).
    fn label_room(&self, text: &mut Text, w: f32, ts: f32) -> (f32, bool) {
        let g = ts.max(1.0);
        let pad = (6.0 * g).round();
        let beside = self.beside_value(text, w, ts);
        let word = crate::choice::widest_word(
            text,
            &self.spec.label,
            TextStyle::new(metrics::FONT_LABEL * ts),
        );
        if word > beside + 0.01 && !self.spec.compact {
            ((w - 2.0 * pad).max(0.0), true)
        } else {
            (beside, false)
        }
    }

    /// The room beside the widest value the slider can show.
    fn beside_value(&self, text: &mut Text, w: f32, ts: f32) -> f32 {
        let g = ts.max(1.0);
        let pad = (6.0 * g).round();
        let style = TextStyle::new(metrics::FONT_LABEL * ts).tabular();
        let vw = [self.spec.min, self.spec.max, self.spec.default, self.value]
            .iter()
            .map(|&v| text.measure(&value_text(&self.spec, v), style))
            .fold(0.0, f32::max);
        (w - 2.0 * pad - vw - 8.0).max(0.0)
    }

    /// The height of the slider's row `w` wide (logical px): [`ROW`] scaled, taller by the
    /// lines a label that wraps takes, so the track stays under the whole label.
    pub fn row_height(&self, text: &mut Text, w: f32, ts: f32) -> f32 {
        let g = ts.max(1.0);
        let base = (ROW * g).round();
        if self.spec.compact {
            return base;
        }
        let style = TextStyle::new(metrics::FONT_LABEL * ts);
        let line_h = (metrics::FONT_LABEL * ts * 1.3).ceil();
        let (label_w, below) = self.label_room(text, w, ts);
        let ls = crate::choice::fitting(text, &self.spec.label, style, label_w);
        let lb = text.layout(&self.spec.label, ls, Some(label_w), Align::Start);
        if lb.lines <= 1 && !below {
            return base;
        }
        let extra = (lb.height - line_h).max(0.0) + if below { line_h } else { 0.0 };
        base + extra.ceil()
    }

    /// Draws the slider into `row` (logical px). `focused` rings it; `hot` is the pointer
    /// over it or a drag in progress.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        row: Rect,
        rtl: bool,
        ts: f32,
        focused: bool,
        hot: bool,
    ) -> SliderGeom {
        let g = ts.max(1.0);
        let pad = (6.0 * g).round();
        let style = TextStyle::new(metrics::FONT_LABEL * ts);
        let line_h = (metrics::FONT_LABEL * ts * 1.3).ceil();
        let top = row.y + (6.0 * g).round();
        // The value at the end of the line, the label at its start; a label whose widest
        // word does not fit beside the value takes the line, the value goes under it.
        let (label_w, below) = self.label_room(text, row.w, ts);
        let ls = crate::choice::fitting(text, &self.spec.label, style, label_w);
        let lb = text.layout(&self.spec.label, ls, Some(label_w), Align::Start);
        let vy = if below { top + lb.height } else { top };
        let shown = match &self.typing {
            Some(s) => s.clone(),
            None => self.value_text(),
        };
        let shown = if rtl && !shown.starts_with(specular_i18n::LRI) {
            isolate_ltr(&shown)
        } else {
            shown
        };
        let vb = text.layout(&shown, style.tabular(), None, Align::Start);
        let vw = vb.width;
        let vx = if self.spec.compact {
            (row.center().0 - vw * 0.5).round()
        } else if rtl {
            row.x + pad
        } else {
            row.right() - pad - vw
        };
        if self.typing.is_some() {
            let field = Rect::new(vx - 4.0, vy - 2.0, vw + 10.0, line_h + 4.0);
            dl.rect_border(field, metrics::RADIUS_SM, pal.surface, pal.accent, 1.5);
            text.draw(dl, &vb, vx, vy, pal.text);
            // The caret after the text (left to right: numbers).
            let cx = vx + vw + 1.0;
            dl.line((cx, vy + 1.0), (cx, vy + line_h - 1.0), 1.0, pal.text);
        } else {
            // A value off its default reads stronger: what was changed stands out.
            let changed =
                value_text(&self.spec, self.value) != value_text(&self.spec, self.spec.default);
            let ink = if changed { pal.text } else { pal.text_muted };
            text.draw(dl, &vb, vx, vy, ink);
        }
        let lx = if rtl {
            row.right() - pad - lb.width.min(label_w)
        } else {
            row.x + pad
        };
        if !self.spec.compact {
            dl.push_clip(Rect::new(
                if rtl {
                    row.right() - pad - label_w
                } else {
                    row.x + pad
                },
                row.y,
                label_w,
                row.h,
            ));
            text.draw(dl, &lb, lx, top, pal.text);
            dl.pop_clip();
        }
        // The track, the fill from the default to the value, the thumb.
        let r = (6.0 * g).round();
        let track = track_of(row, ts);
        let x_of = |f: f32| {
            if rtl {
                track.right() - f * track.w
            } else {
                track.x + f * track.w
            }
        };
        let x0 = x_of(self.fraction_of(self.spec.default));
        let x1 = x_of(self.fraction());
        if let Some((lo, hi)) = self.spec.scale {
            // Rounded ends in the end colours, then thin steps between (mirrored with the
            // track: the minimum's colour is at the minimum's end).
            let (left, right) = if rtl { (hi, lo) } else { (lo, hi) };
            dl.rect(Rect::new(track.x, track.y, 4.0, track.h), 2.0, left);
            dl.rect(
                Rect::new(track.right() - 4.0, track.y, 4.0, track.h),
                2.0,
                right,
            );
            let inner = Rect::new(track.x + 2.0, track.y, (track.w - 4.0).max(0.0), track.h);
            let n = (inner.w / 3.0).ceil().max(1.0) as usize;
            let step = inner.w / n as f32;
            for k in 0..n {
                let t = (k as f32 + 0.5) / n as f32;
                let x = inner.x + k as f32 * step;
                // A hair wider than the step, so no seams show between them.
                dl.rect(
                    Rect::new(x, inner.y, step + 0.5, inner.h),
                    0.0,
                    left.lerp(right, t),
                );
            }
        } else {
            dl.rect(track, 2.0, pal.text_faint.with_alpha(0.45));
            let (a, b) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
            if b - a > 0.5 {
                dl.rect(Rect::new(a, track.y, b - a, track.h), 2.0, pal.accent);
            }
        }
        // A tick at the default when it is not an end.
        let fd = self.fraction_of(self.spec.default);
        if fd > 0.0 && fd < 1.0 {
            dl.rect(
                Rect::new(x0 - 0.75, track.y - 3.0, 1.5, track.h + 6.0),
                0.5,
                pal.text_faint,
            );
        }
        let rr = if hot { r + 1.0 } else { r };
        let thumb = Rect::new(x1 - rr, track.y + 2.0 - rr, 2.0 * rr, 2.0 * rr);
        dl.shadow(thumb, rr, pal.shadow, 3.0, 1.0);
        dl.rect_border(thumb, rr, Color::WHITE, pal.accent, 2.0);
        if focused {
            dl.rect_border(
                row.inset(1.0),
                metrics::RADIUS_SM,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        SliderGeom { row, track, thumb }
    }

    /// The slider's screen-reader node (role, name, value, range, step; it takes
    /// increment, decrement, set value and focus). `bounds` in physical px.
    pub fn a11y_node(&self, bounds: accesskit::Rect) -> accesskit::Node {
        use accesskit::{Action, Node, Role};
        let mut n = Node::new(Role::Slider);
        n.set_label(self.spec.label.as_str());
        n.set_value(specular_i18n::strip_isolates(&self.value_text()));
        n.set_numeric_value(f64::from(self.value));
        let (lo, hi) = self.range();
        n.set_min_numeric_value(f64::from(lo));
        n.set_max_numeric_value(f64::from(hi));
        n.set_numeric_value_step(f64::from(self.spec.step));
        n.set_bounds(bounds);
        n.add_action(Action::Focus);
        n.add_action(Action::Increment);
        n.add_action(Action::Decrement);
        n.add_action(Action::SetValue);
        n
    }
}

/// The track of a slider drawn in `row` at text scale `ts` (logical px, 4 px high).
pub fn track_of(row: Rect, ts: f32) -> Rect {
    let g = ts.max(1.0);
    let inset = (6.0 * g).round() * 2.0;
    let y = (row.bottom() - (12.0 * g).round()).round();
    Rect::new(row.x + inset, y - 2.0, (row.w - 2.0 * inset).max(1.0), 4.0)
}

/// A value of `spec` as shown.
pub fn value_text(spec: &SliderSpec, v: f32) -> String {
    let l = I18n::global().localizer();
    let n = spec.shown(v);
    let d = spec.decimals;
    // Rounded first, so a value that rounds to zero shows no sign.
    let k = 10f64.powi(i32::from(d));
    let n = (n * k).round() / k;
    let n = if n == 0.0 { 0.0 } else { n };
    // "+" or a true minus right before the digits; right to left the whole value (sign,
    // digits, unit) is one left-to-right isolate, so it reads "+0.060", "−1.50 EV", "+12°"
    // wherever it is drawn.
    let s =
        specular_i18n::signed_number(&l.number_fixed(n.abs(), d), n < 0.0, spec.signed && n > 0.0);
    let s = match spec.unit {
        Unit::Plain => s,
        Unit::Ev => t!("develop-value-ev", value = s),
        Unit::Percent => l.percent_text(&s),
        Unit::Degrees => t!("develop-value-degrees", value = s),
        Unit::Kelvin => t!("develop-value-kelvin", value = s),
        Unit::Decibels => t!("unit-decibels", value = s),
    };
    specular_i18n::ltr_value(&s, l.direction())
}

/// Characters that start or continue a typed value.
fn typable(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | ',' | '\u{2212}')
}

/// A typed number: a decimal point or comma, a leading sign (a minus sign too).
pub fn parse_number(s: &str) -> Option<f64> {
    let t: String = s
        .trim()
        .chars()
        .map(|c| match c {
            ',' => '.',
            '\u{2212}' => '-',
            c => c,
        })
        .collect();
    let t = t.strip_prefix('+').unwrap_or(&t);
    let v: f64 = t.parse().ok()?;
    v.is_finite().then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_input::Mods;

    fn key(k: NamedKey, mods: Mods) -> KeyCombo {
        KeyCombo::new(Key::Named(k), mods)
    }

    /// In slider rows a Develop label's widest word fits the label's room beside a
    /// wide value ("−5.00 EV"), en/he/de/uk, 320–480 px, text 1× and 1.5×; the rare word that
    /// can't (German compounds at 1.5× in the narrowest panel) breaks inside, never cut.
    #[test]
    fn slider_label_words_fit() {
        use specular_i18n::I18n;
        let keys = crate::choice::develop_labels();
        let mut text = Text::new();
        let mut checked = 0usize;
        let mut broken = 0usize;
        let mut moved = 0usize;
        let mut worst = f32::MAX;
        for lang in ["en", "he", "de", "uk"] {
            I18n::with_thread_language(lang.parse().unwrap(), || {
                for w in [320.0, 360.0, 400.0, 440.0, 480.0] {
                    for ts in [1.0f32, 1.5] {
                        let style = TextStyle::new(metrics::FONT_LABEL * ts);
                        for k in &keys {
                            let label = I18n::global().tr(k);
                            let s = Slider::new(
                                SliderSpec::new(label.clone(), -5.0, 5.0, 0.0)
                                    .unit(Unit::Ev, 2)
                                    .signed(),
                            );
                            let lw = s.label_width(&mut text, w, ts);
                            let word = crate::choice::widest_word(&mut text, &label, style);
                            worst = worst.min(lw - word);
                            let (_, below) = s.label_room(&mut text, w, ts);
                            if below {
                                moved += 1;
                            }
                            if word > lw + 0.01 {
                                // A word wider than the whole line breaks inside, never cut.
                                let lb = text.layout(
                                    &label,
                                    style.break_anywhere(),
                                    Some(lw),
                                    Align::Start,
                                );
                                assert!(lb.width <= lw + 0.5, "{lang} {w} {ts} {k}: cut");
                                broken += 1;
                            }
                            checked += 1;
                        }
                    }
                }
            });
        }
        eprintln!(
            "{checked} slider labels; {moved} with the value under the label, {broken} with a word wider than the line (broken inside); least room to spare {worst:.1} px"
        );
        assert!(broken * 100 < checked, "{broken} of {checked}");
    }

    fn ev() -> Slider {
        Slider::new(
            SliderSpec::new("Exposure", -5.0, 5.0, 0.0)
                .steps(0.1, 1.0)
                .unit(Unit::Ev, 2)
                .signed(),
        )
    }

    #[test]
    fn arrows_step_coarse_and_fine() {
        let mut s = ev();
        assert_eq!(
            s.key(key(NamedKey::ArrowRight, Mods::NONE), false),
            KeyResult::Changed
        );
        assert!((s.value - 0.1).abs() < 1e-6);
        s.key(key(NamedKey::ArrowUp, Mods::SHIFT), false);
        assert!((s.value - 1.1).abs() < 1e-5);
        let alt = Mods {
            alt: true,
            ..Mods::NONE
        };
        s.key(key(NamedKey::ArrowDown, alt), false);
        assert!((s.value - 1.09).abs() < 1e-5);
        // Whole steps land on the step grid.
        s.key(key(NamedKey::PageDown, Mods::NONE), false);
        assert!((s.value - 0.1).abs() < 1e-5);
        s.key(key(NamedKey::End, Mods::NONE), false);
        assert_eq!(s.value, 5.0);
        // At the end: taken, nothing changes.
        assert_eq!(
            s.key(key(NamedKey::ArrowUp, Mods::NONE), false),
            KeyResult::Handled
        );
        s.key(key(NamedKey::Home, Mods::NONE), false);
        assert_eq!(s.value, -5.0);
        s.key(key(NamedKey::Delete, Mods::NONE), false);
        assert_eq!(s.value, 0.0);
    }

    #[test]
    fn right_to_left_turns_the_side_arrows_round() {
        let mut s = ev();
        s.key(key(NamedKey::ArrowLeft, Mods::NONE), true);
        assert!(s.value > 0.0);
        s.key(key(NamedKey::ArrowRight, Mods::NONE), true);
        s.key(key(NamedKey::ArrowRight, Mods::NONE), true);
        assert!(s.value < 0.0);
        // Up raises in both directions.
        let mut s = ev();
        s.key(key(NamedKey::ArrowUp, Mods::NONE), true);
        assert!(s.value > 0.0);
    }

    #[test]
    fn typing_sets_a_value_and_escape_cancels() {
        let mut s = ev();
        let ch = |c| KeyCombo::new(Key::Char(c), Mods::NONE);
        assert_eq!(s.key(ch('1'), false), KeyResult::Handled);
        s.key(ch(','), false);
        s.key(ch('5'), false);
        assert_eq!(s.typing(), Some("1,5"));
        assert_eq!(
            s.key(key(NamedKey::Enter, Mods::NONE), false),
            KeyResult::Changed
        );
        assert_eq!(s.value, 1.5);
        assert_eq!(s.typing(), None);
        s.key(key(NamedKey::Enter, Mods::NONE), false);
        assert_eq!(s.typing(), Some("1.5"));
        s.key(key(NamedKey::Backspace, Mods::NONE), false);
        s.key(ch('9'), false);
        s.key(key(NamedKey::Escape, Mods::NONE), false);
        assert_eq!(s.value, 1.5);
        // Out of range is clamped.
        s.key(ch('9'), false);
        s.key(ch('9'), false);
        s.key(key(NamedKey::Enter, Mods::NONE), false);
        assert_eq!(s.value, 5.0);
        // Not a number: nothing set.
        s.key(ch('-'), false);
        s.key(key(NamedKey::Enter, Mods::NONE), false);
        assert_eq!(s.value, 5.0);
    }

    #[test]
    fn percent_is_typed_and_shown_in_percent() {
        let mut s =
            Slider::new(SliderSpec::new("Saturation", -1.0, 1.0, 0.0).unit(Unit::Percent, 0));
        for c in ['2', '5'] {
            s.key(KeyCombo::new(Key::Char(c), Mods::NONE), false);
        }
        s.key(key(NamedKey::Enter, Mods::NONE), false);
        assert!((s.value - 0.25).abs() < 1e-6);
    }

    #[test]
    fn drag_is_relative_and_shift_is_fine() {
        let mut s = ev();
        assert!(s.drag(20.0, 200.0, false, false));
        assert!((s.value - 1.0).abs() < 1e-5);
        s.drag(20.0, 200.0, true, false);
        assert!((s.value - 1.1).abs() < 1e-5);
        s.drag(20.0, 200.0, false, true);
        assert!((s.value - 0.1).abs() < 1e-5);
        let track = Rect::new(10.0, 0.0, 100.0, 4.0);
        assert_eq!(s.value_at(10.0, track, false), -5.0);
        assert_eq!(s.value_at(10.0, track, true), 5.0);
        assert_eq!(s.value_at(60.0, track, false), 0.0);
    }

    #[test]
    fn parses_signs_and_commas() {
        assert_eq!(parse_number("+0,5"), Some(0.5));
        assert_eq!(parse_number("\u{2212}2"), Some(-2.0));
        assert_eq!(parse_number("-"), None);
        assert_eq!(parse_number("inf"), None);
    }

    /// Signed values in a right-to-left UI, laid out by the real text engine and read back
    /// in visual order: the sign right before the digits, the unit after them ("+0.060",
    /// "−1.50 EV", "+12°"), never "0.060+" or "1.50 EV−". Sliders (drawn and as the speed
    /// readout), the player's sound and subtitle delay, sky and map positions.
    #[test]
    fn signed_values_read_sign_first_in_rtl() {
        let mut text = Text::new();
        for lang in ["he", "ar"] {
            I18n::with_thread_language(lang.parse().unwrap(), || {
                let cases = [
                    (Unit::Plain, 3, 0.06, "+0.060"),
                    (Unit::Plain, 3, 0.0, "0.000"),
                    (Unit::Plain, 3, -0.04, "\u{2212}0.040"),
                    (Unit::Ev, 2, -1.5, "\u{2212}1.50"),
                    (Unit::Ev, 2, 0.7, "+0.70"),
                    (Unit::Degrees, 0, 12.0, "+12"),
                    (Unit::Degrees, 0, -12.0, "\u{2212}12"),
                    (Unit::Percent, 0, -0.25, "\u{2212}25"),
                    (Unit::Decibels, 1, -3.0, "\u{2212}3.0"),
                ];
                for (unit, d, v, want) in cases {
                    let spec = SliderSpec::new("x", -100.0, 100.0, 0.0)
                        .unit(unit, d)
                        .signed();
                    let shown = value_text(&spec, v);
                    assert!(shown.starts_with(specular_i18n::LRI), "{lang}: {shown:?}");
                    let vis = text.visual_order(&shown);
                    assert!(vis.trim_start().starts_with(want), "{lang}: {vis:?}");
                    // As drawn by the panel and in the speed readout ("{control}: {value}").
                    let mut s = Slider::new(spec.clone());
                    s.set(v);
                    let hud = t!(
                        "develop-speed-hud",
                        control = "חשיפה".to_owned(),
                        value = s.value_text()
                    );
                    let vis = text.visual_order(&hud);
                    assert!(vis.contains(want), "{lang}: {vis:?}");
                }
                for (ms, want) in [(350, "+350"), (-120, "\u{2212}120")] {
                    let vis = text.visual_order(&crate::player::delay_text(ms));
                    assert!(vis.contains(want), "{lang}: {vis:?}");
                }
                let sky = t!(
                    "science-lonlat",
                    lon = "120.5000".to_owned(),
                    lat = "\u{2212}33.8688".to_owned()
                );
                let vis = text.visual_order(&sky);
                assert!(vis.contains("\u{2212}33.8688"), "{lang}: {vis:?}");
                let pos = t!(
                    "map-goto-toast",
                    position = "-33.868800, 151.209300".to_owned()
                );
                let vis = text.visual_order(&pos);
                assert!(vis.contains("-33.868800, 151.209300"), "{lang}: {vis:?}");
            });
        }
    }
}
