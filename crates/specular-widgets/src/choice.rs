//! A Develop panel row that is a choice among a few named options, or a button.
//!
//! - **Choice** (`options` not empty): the label at the start of the row, the chosen option
//!   at its end between ‹ ›. A click or Enter/Space takes the next option; Left and Right
//!   (mirrored in right-to-left) step back and forth; Home and End jump to the ends.
//! - **Button** (`options` empty): a full-width button showing the label; a click, Enter or
//!   Space presses it (the panel reports it as a value event, nothing changes here).

use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, Palette, metrics};

use crate::slider::KeyResult;

/// Row height (logical px at the standard text size).
pub const ROW: f32 = 36.0;

/// A choice or a button; see the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub label: String,
    /// The options' names (empty: a button).
    pub options: Vec<String>,
    pub selected: usize,
}

impl Choice {
    pub fn new(label: impl Into<String>, options: Vec<String>) -> Choice {
        Choice {
            label: label.into(),
            options,
            selected: 0,
        }
    }

    pub fn button(label: impl Into<String>) -> Choice {
        Choice::new(label, Vec::new())
    }

    pub fn is_button(&self) -> bool {
        self.options.is_empty()
    }

    /// Selects option `i` (clamped); whether it changed.
    pub fn set(&mut self, i: usize) -> bool {
        let i = i.min(self.options.len().saturating_sub(1));
        let changed = i != self.selected;
        self.selected = i;
        changed
    }

    /// Steps by `d` options, wrapping; whether it changed (a button: always, it was pressed).
    pub fn step(&mut self, d: isize) -> bool {
        let n = self.options.len() as isize;
        if n == 0 {
            return true;
        }
        self.set((self.selected as isize + d).rem_euclid(n) as usize)
    }

    /// The chosen option's name ("" for a button).
    pub fn value_text(&self) -> &str {
        self.options.get(self.selected).map_or("", String::as_str)
    }

    pub fn key(&mut self, combo: KeyCombo, rtl: bool) -> KeyResult {
        if combo.mods.ctrl || combo.mods.alt {
            return KeyResult::Ignored;
        }
        let back = if rtl { 1 } else { -1 };
        let r = match combo.key {
            Key::Named(NamedKey::Enter | NamedKey::Space) => self.step(1),
            _ if self.is_button() => return KeyResult::Ignored,
            Key::Named(NamedKey::ArrowLeft) => self.step(back),
            Key::Named(NamedKey::ArrowRight) => self.step(-back),
            Key::Named(NamedKey::Home) => self.set(0),
            Key::Named(NamedKey::End) => self.set(usize::MAX),
            _ => return KeyResult::Ignored,
        };
        if r {
            KeyResult::Changed
        } else {
            KeyResult::Handled
        }
    }

    /// A click; whether it changed (or pressed the button).
    pub fn press(&mut self) -> bool {
        self.step(1)
    }

    #[allow(clippy::too_many_arguments)] // the same signature as `Slider::draw`
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
    ) {
        let g = ts.max(1.0);
        let pad = (6.0 * g).round();
        let style = TextStyle::new(metrics::FONT_LABEL * ts);
        let inner = row.inset((3.0 * g).round());
        if self.is_button() {
            let fill = if hot { pal.surface_hover } else { pal.surface };
            dl.rect_border(inner, metrics::RADIUS_SM, fill, pal.text_faint, 1.0);
            // Names (styles, snapshots, versions) keep their own word order in either UI.
            let label = specular_i18n::isolate_label(&self.label, dir(rtl));
            let b = text.layout(&label, style, Some(inner.w - 2.0 * pad), Align::Center);
            dl.push_clip(inner);
            text.draw(
                dl,
                &b,
                inner.x + (inner.w - b.width) * 0.5,
                inner.y + (inner.h - b.height) * 0.5,
                pal.text,
            );
            dl.pop_clip();
        } else {
            if hot {
                dl.rect(inner, metrics::RADIUS_SM, pal.surface_hover);
            }
            let (label_w, value_w) = self.split(text, row.w, ts, rtl);
            let shown = self.shown_value(text, style, value_w, rtl);
            let vb = text.layout(&shown, style, None, Align::Start);
            let ty = inner.y + (inner.h - vb.height) * 0.5;
            let vx = if rtl {
                inner.x + pad
            } else {
                inner.right() - pad - vb.width
            };
            text.draw(dl, &vb, vx, ty, pal.accent);
            let ls = fitting(text, &self.label, style, label_w);
            let lb = text.layout(&self.label, ls, Some(label_w), Align::Start);
            let lx = if rtl {
                inner.right() - pad - lb.width.min(label_w)
            } else {
                inner.x + pad
            };
            dl.push_clip(Rect::new(
                if rtl {
                    inner.right() - pad - label_w
                } else {
                    inner.x + pad
                },
                inner.y,
                label_w,
                inner.h,
            ));
            // Centred on its own height: a label on two lines next to a one-line value.
            let ly = inner.y + (inner.h - lb.height) * 0.5;
            text.draw(dl, &lb, lx, ly, pal.text);
            dl.pop_clip();
        }
        if focused {
            dl.rect_border(
                row.inset(1.0),
                metrics::RADIUS_SM,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
    }

    /// The room for a choice's label in a row `w` wide: beside the widest option, so the
    /// label wraps the same way whichever is chosen.
    fn label_width(&self, text: &mut Text, w: f32, ts: f32, rtl: bool) -> f32 {
        self.split(text, w, ts, rtl).0
    }

    /// The label's room and the most the shown option may take in a row `w` wide. The
    /// label gets the room beside the widest option, and at least its widest word (a word
    /// can't wrap), as long as the option keeps room for "‹ X… ›"; the option is then cut
    /// to what is left.
    pub(crate) fn split(&self, text: &mut Text, w: f32, ts: f32, rtl: bool) -> (f32, f32) {
        let g = ts.max(1.0);
        let pad = (6.0 * g).round();
        let style = TextStyle::new(metrics::FONT_LABEL * ts);
        let inner_w = w - 2.0 * (3.0 * g).round();
        let most = inner_w * 0.6;
        let mut probe = self.clone();
        let vw = (0..self.options.len().max(1))
            .map(|i| {
                probe.selected = i;
                let s = probe.shown_value(text, style, most, rtl);
                text.measure(&s, style)
            })
            .fold(0.0, f32::max);
        let beside = (inner_w - 3.0 * pad - vw).max(0.0);
        let word = widest_word(text, &self.label, style);
        if word <= beside {
            return (beside, most);
        }
        let least = text.measure(&arrows("X…", rtl), style);
        let cap = (inner_w - 3.0 * pad - least).max(0.0);
        let label_w = word.min(cap).max(beside);
        (label_w, (inner_w - 3.0 * pad - label_w).clamp(0.0, most))
    }

    /// The height of the row `w` wide (logical px): [`ROW`] scaled, taller when the label
    /// wraps (a button grows to show its whole label; a choice's label never overlaps the
    /// rows around it).
    pub fn row_height(&self, text: &mut Text, w: f32, ts: f32, rtl: bool) -> f32 {
        let g = ts.max(1.0);
        let base = (ROW * g).round();
        let pad = (6.0 * g).round();
        let style = TextStyle::new(metrics::FONT_LABEL * ts);
        let inner_w = w - 2.0 * (3.0 * g).round();
        let label = if self.is_button() {
            specular_i18n::isolate_label(&self.label, dir(rtl))
        } else {
            self.label.clone()
        };
        let room = if self.is_button() {
            inner_w - 2.0 * pad
        } else {
            self.label_width(text, w, ts, rtl)
        };
        let ls = fitting(text, &label, style, room);
        let laid = text.layout(&label, ls, Some(room), Align::Start);
        if laid.lines <= 1 {
            return base;
        }
        // The label and its padding inside the row's inset.
        base.max((laid.height + 2.0 * pad + 2.0 * (3.0 * g).round()).ceil())
    }

    /// The chosen option between ‹ ›, on one line within `max_w`: a long name is cut with
    /// "…" so the arrows stay with it (the full name is the screen-reader value).
    ///
    /// The arrows sit in a left-to-right isolate in both directions, so they always read
    /// "‹ value ›" (never mirrored by the shaper into "› value ‹"): the left one points
    /// left, the right one right. Right to left, "next" (a click, the Left key) is the left
    /// one, as in the print and export steppers.
    fn shown_value(&self, text: &mut Text, style: TextStyle, max_w: f32, rtl: bool) -> String {
        let value = specular_i18n::isolate_label(self.value_text(), dir(false));
        let value = specular_i18n::ellipsize_to_fit(&value, false, |v| {
            text.measure(&arrows(v, rtl), style) <= max_w
        });
        arrows(&value, rtl)
    }

    /// The screen-reader node: a button, or a combo box whose value is the chosen option
    /// (a click takes the next). `bounds` in physical px.
    pub fn a11y_node(&self, bounds: accesskit::Rect) -> accesskit::Node {
        use accesskit::{Action, Node, Role};
        let mut n = Node::new(if self.is_button() {
            Role::Button
        } else {
            Role::ComboBox
        });
        n.set_label(self.label.as_str());
        if !self.is_button() {
            n.set_value(self.value_text());
            // It steps through its options in place; no list ever opens.
            n.set_expanded(false);
        }
        n.set_bounds(bounds);
        n.add_action(Action::Focus);
        n.add_action(Action::Click);
        n
    }
}

/// `style` for a label laid out `room` wide: breaking inside words when its widest word is
/// wider than the room (so it is never cut), else between words only.
pub(crate) fn fitting(text: &mut Text, label: &str, style: TextStyle, room: f32) -> TextStyle {
    if widest_word(text, label, style) > room + 0.01 {
        style.break_anywhere()
    } else {
        style
    }
}

/// The widest word of `label` (scripts written without spaces break between characters,
/// so there each character counts).
pub(crate) fn widest_word(text: &mut Text, label: &str, style: TextStyle) -> f32 {
    let mut most = 0.0f32;
    for word in label.split_whitespace() {
        let w = if word.chars().any(|c| c >= '\u{2E80}') {
            word.chars()
                .map(|c| text.measure(c.encode_utf8(&mut [0; 4]), style))
                .fold(0.0, f32::max)
        } else {
            text.measure(word, style)
        };
        most = most.max(w);
    }
    most
}

/// The short Develop labels (`develop-*` messages of up to 40 characters without
/// placeables) of the English files: the rows the panel shows.
#[cfg(test)]
pub(crate) fn develop_labels() -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../specular-i18n/i18n/en");
    let mut keys = Vec::new();
    for e in std::fs::read_dir(dir).expect("en files") {
        let p = e.expect("entry").path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("develop") {
            continue;
        }
        for line in std::fs::read_to_string(&p).unwrap_or_default().lines() {
            if let Some((k, v)) = line.split_once(" = ")
                && k.starts_with("develop-")
                && !k.contains(' ')
                && !v.contains('{')
                && v.chars().count() <= 40
            {
                keys.push(k.to_owned());
            }
        }
    }
    keys.sort();
    keys
}

/// `value` (isolated) between ‹ ›; right to left the whole in an LRI after an RLM, so the
/// arrows are laid out left to right and never mirrored.
pub fn arrows(value: &str, rtl: bool) -> String {
    if rtl {
        format!(
            "{}{}‹ {value} ›{}",
            specular_i18n::RLM,
            specular_i18n::LRI,
            specular_i18n::PDI
        )
    } else {
        format!("‹ {value} ›")
    }
}

fn dir(rtl: bool) -> specular_i18n::Direction {
    if rtl {
        specular_i18n::Direction::Rtl
    } else {
        specular_i18n::Direction::Ltr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_input::Mods;

    fn k(n: NamedKey) -> KeyCombo {
        KeyCombo::new(Key::Named(n), Mods::NONE)
    }

    /// In every Develop label (en, he, de, uk; 320–480 px, text 1× and 1.5×) the
    /// widest word gets its whole width beside the clipped-highlights options, unless the
    /// option would be left less than "‹ X… ›" (then the word breaks inside, never cut);
    /// the option shown fits what is left.
    #[test]
    fn a_label_word_is_never_cut() {
        use specular_i18n::I18n;
        let keys = develop_labels();
        assert!(keys.len() > 200, "{}", keys.len());
        let mut text = Text::new();
        let opts = [
            "develop-highlight-opposed",
            "develop-highlight-rebuild",
            "develop-highlight-clip",
        ];
        let (mut checked, mut capped) = (0usize, 0usize);
        for lang in ["en", "he", "de", "uk"] {
            I18n::with_thread_language(lang.parse().unwrap(), || {
                let rtl = lang == "he";
                let opts: Vec<String> = opts.iter().map(|k| I18n::global().tr(k)).collect();
                for w in [320.0, 360.0, 400.0, 440.0, 480.0] {
                    for ts in [1.0f32, 1.5] {
                        let g = ts.max(1.0);
                        let pad = (6.0 * g).round();
                        let inner_w = w - 2.0 * (3.0 * g).round();
                        let style = TextStyle::new(metrics::FONT_LABEL * ts);
                        let least = text.measure(&arrows("X…", rtl), style);
                        for k in &keys {
                            let label = I18n::global().tr(k);
                            let c = Choice::new(label.clone(), opts.clone());
                            let (lw, vw) = c.split(&mut text, w, ts, rtl);
                            let word = widest_word(&mut text, &label, style);
                            if word > lw + 0.01 {
                                // Cut only when the option is down to "‹ X… ›".
                                assert!(vw <= least + 0.5, "{lang} {w} {ts} {k}: {word} > {lw}");
                                capped += 1;
                            }
                            for i in 0..opts.len() {
                                let mut c = c.clone();
                                c.selected = i;
                                let shown = c.shown_value(&mut text, style, vw, rtl);
                                let sw = text.measure(&shown, style);
                                assert!(
                                    lw + 3.0 * pad + sw <= inner_w + 0.5,
                                    "{lang} {w} {ts} {k}: label {lw} + option {sw} > {inner_w}"
                                );
                            }
                            checked += 1;
                        }
                    }
                }
            });
        }
        eprintln!("{checked} label rows, {capped} with a word wider than the row allows");
        // The case the golden showed: "Clipped highlights" whole at the narrowest panel.
        let c = Choice::new(
            "Clipped highlights",
            vec!["Rebuild colour".into(), "Rebuild large areas".into()],
        );
        let style = TextStyle::new(metrics::FONT_LABEL);
        let (lw, _) = c.split(&mut text, 320.0, 1.0, false);
        assert!(text.measure("highlights", style) <= lw);
    }

    #[test]
    fn keys_step_and_wrap_and_a_button_presses() {
        let mut c = Choice::new("Space", vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(c.key(k(NamedKey::ArrowRight), false), KeyResult::Changed);
        assert_eq!(c.value_text(), "b");
        // Mirrored in right to left.
        assert_eq!(c.key(k(NamedKey::ArrowRight), true), KeyResult::Changed);
        assert_eq!(c.selected, 0);
        assert_eq!(c.key(k(NamedKey::ArrowLeft), false), KeyResult::Changed);
        assert_eq!(c.selected, 2);
        assert_eq!(c.key(k(NamedKey::End), false), KeyResult::Handled);
        assert_eq!(c.key(k(NamedKey::Home), false), KeyResult::Changed);
        assert!(c.press());
        assert_eq!(c.selected, 1);
        let mut b = Choice::button("Choose a file…");
        assert_eq!(b.key(k(NamedKey::Enter), false), KeyResult::Changed);
        assert_eq!(b.key(k(NamedKey::ArrowLeft), false), KeyResult::Ignored);
        assert!(b.press());
        assert_eq!(b.value_text(), "");
    }

    #[test]
    fn a_long_value_stays_on_one_line_with_its_arrows() {
        let mut text = Text::bundled_only();
        let style = TextStyle::new(metrics::FONT_LABEL);
        let c = Choice::new(
            "Profile",
            vec!["Specular test printer (synthetic)".into(), "Off".into()],
        );
        for rtl in [false, true] {
            let max_w = 120.0;
            let shown = c.shown_value(&mut text, style, max_w, rtl);
            let plain = specular_i18n::strip_isolates(&shown);
            assert!(
                plain.starts_with("‹ ") && plain.ends_with(" ›"),
                "{shown:?}"
            );
            assert!(shown.contains('…'), "{shown:?}");
            assert!(text.measure(&shown, style) <= max_w);
            assert!(specular_i18n::isolates_balanced(&shown));
            let one = text.layout("‹ Off ›", style, None, Align::Start).height;
            let b = text.layout(&shown, style, None, Align::Start);
            assert!(b.height <= one + 0.5, "one line");
            // Short values are whole.
            let short = Choice::new("Intent", vec!["Off".into()]);
            assert!(
                !short
                    .shown_value(&mut text, style, max_w, rtl)
                    .contains('…')
            );
        }
        // The screen reader hears the whole name.
        let n = c.a11y_node(accesskit::Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(n.value(), Some("Specular test printer (synthetic)"));
    }

    /// A numbered button row in Hebrew ("{ $n }. { $mode }", Retouch's spots; the markup
    /// list's items too): its label is isolated right to left, so the number is on the
    /// right, where a Hebrew list starts, and the word on its left.
    #[test]
    fn numbered_rows_start_on_the_right_in_rtl() {
        use specular_i18n::{Direction, I18n, isolate_label, t};
        let mut text = Text::bundled_only();
        I18n::with_thread_language(specular_i18n::langid!("he"), || {
            let mode = t!("develop-retouch-heal");
            for (id, n) in [("develop-retouch-spot", 3i64), ("markup-item", 12)] {
                let row = if id == "markup-item" {
                    t!("markup-item", n = n, kind = mode.clone())
                } else {
                    t!("develop-retouch-spot", n = n, mode = mode.clone())
                };
                let shown = isolate_label(&row, Direction::Rtl);
                let vis = text.visual_order(&shown);
                let vis = vis.trim();
                assert!(vis.ends_with(&format!(" .{n}")), "{id}: {vis:?}");
                assert!(
                    vis.chars().next().is_some_and(specular_i18n::is_rtl_letter),
                    "{id}: {vis:?}"
                );
            }
        });
    }

    /// Right to left the arrows are laid out left to right (an LRI, so the shaper never
    /// mirrors them): "‹" on the left, "›" on the right, the Hebrew or Latin value between
    /// in its own order, in both directions.
    #[test]
    fn choice_arrows_are_never_mirrored() {
        let mut text = Text::bundled_only();
        let style = TextStyle::new(metrics::FONT_LABEL);
        // "Ellipse", and a value made only of isolated arguments ("{ $n }. { $mode }").
        let row = format!(
            "{}{}3{}. {}ריפוי{}",
            specular_i18n::RLM,
            specular_i18n::LRI,
            specular_i18n::PDI,
            specular_i18n::RLI,
            specular_i18n::PDI
        );
        for (value, inner) in [
            ("אליפסה".to_owned(), "הספילא"),
            ("ACEScct".to_owned(), "ACEScct"),
            (row, "יופיר .3"),
        ] {
            let c = Choice::new("x", vec![value.clone()]);
            for rtl in [false, true] {
                let shown = c.shown_value(&mut text, style, 400.0, rtl);
                if rtl {
                    // No mirroring happens at an even (left-to-right) level.
                    assert!(shown.starts_with("\u{200F}\u{2066}‹"), "{shown:?}");
                }
                let vis = text.visual_order(&shown);
                assert_eq!(vis.trim(), format!("‹ {inner} ›"), "{value:?} rtl={rtl}");
            }
        }
    }
}
