//! The volume slider beside the mute button of a movie's or sound file's transport bar.
//!
//! It looks like the seek bar: a track, the part up to the volume in the accent and a knob
//! that grows on hover or press. It goes to 100 %, for the sound played here and for a cast
//! device's own volume (a tick marks 100 % on a slider that goes higher).
//!
//! - **Press or drag** sets the volume under the pointer (it catches at 100 %).
//! - **Wheel** over it: 5 % a notch.
//! - **Keys** when focused (a Tab stop of its own): → and ↑ louder, ← and ↓ quieter (5 %),
//!   Page Up and Page Down 25 %, Home silent, End the top. Like all media controls it runs
//!   left to right in right-to-left languages too.
//!
//! Each change is a [`Command::Volume`]; the app saves it, applies it to the sound (or the
//! cast device) and unmutes when it is raised. While hovered, dragged or focused, the
//! percentage shows above it.

use accesskit::{Action as AkAction, ActionData, Node, Role};
use specular_gfx::Rect;
use specular_i18n::{I18n, t};
use specular_input::{KeyCombo, NamedKey};

use crate::ctx::{Ctx, Target};
use crate::{Chrome, Command, Response, VideoPlayback, ViewerState};

/// The slider's width in the bar (logical px).
pub(crate) const WIDTH: f32 = 120.0;
/// Between the slider's ends and its track.
const INSET: f32 = 8.0;
/// An arrow key's step, a wheel notch's, and Page Up's or Page Down's (percent).
const STEP: f32 = 5.0;
const COARSE: f32 = 25.0;
/// A drag lands on 100 % when this close to it (of normal).
const CATCH: f32 = 0.03;
/// How long the slider shows a value asked for before trusting the sound's again (s).
const HOLD: f64 = 0.6;

/// The track inside the slider's hit rectangle `r`.
pub(crate) fn track(r: Rect) -> Rect {
    Rect::new(
        r.x + INSET,
        r.y + r.h * 0.5 - 1.5,
        (r.w - 2.0 * INSET).max(1.0),
        3.0,
    )
}

/// The volume under `x` on the slider at `r` going up to `max` (1 is normal): in whole
/// percents, caught at 100 %.
pub(crate) fn volume_at(r: Rect, x: f32, max: f32) -> f32 {
    let t = track(r);
    let f = ((x - t.x) / t.w).clamp(0.0, 1.0);
    let v = whole(f * max);
    if max > 1.0 && (v - 1.0).abs() < CATCH {
        1.0
    } else {
        v
    }
}

/// `v` in whole percents.
fn whole(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// One step of `pct` percent from `v` up (or down), onto the step grid as the volume keys
/// are, within `0..=max`.
fn stepped(v: f32, pct: f32, up: bool, max: f32) -> f32 {
    let at = (v * 100.0).round() / pct;
    let n = if up {
        at.floor() + 1.0
    } else {
        at.ceil() - 1.0
    };
    (n * pct / 100.0).clamp(0.0, max)
}

/// "80%" in the UI language.
fn value_text(v: f32) -> String {
    I18n::global().localizer().percent(f64::from(whole(v)), 0)
}

/// "Volume 80%": the readout above the slider.
fn readout(v: f32) -> String {
    t!("toast-volume", percent = (v * 100.0).round() as i64)
}

/// Draws the slider in `r` (the bar's opacity is `alpha`).
pub(crate) fn draw(cx: &mut Ctx, r: Rect, volume: f32, max: f32, muted: bool, alpha: f32) {
    let t = Target::Volume;
    let track = track(r);
    let max = max.max(f32::EPSILON);
    let f = (volume / max).clamp(0.0, 1.0);
    let (hover, pressed, focused) = (cx.is_hover(&t), cx.is_pressed(&t), cx.is_focused(&t));
    let on = if muted {
        cx.pal.text_muted
    } else {
        cx.pal.accent
    };
    cx.dl.rect(track, 1.5, cx.pal.text_faint.with_alpha(0.5));
    cx.dl
        .rect(Rect::new(track.x, track.y, track.w * f, track.h), 1.5, on);
    if max > 1.0 {
        // Normal (100 %): a small tick across the track.
        let x = (track.x + track.w / max).round();
        cx.dl.rect(
            Rect::new(x - 0.75, track.y - 2.5, 1.5, track.h + 5.0),
            0.75,
            cx.pal.text,
        );
    }
    // The knob: the accent inside a rim of the text colour, which reads on the glass over
    // any picture (the accent alone can be too faint over a bright one).
    let knob = if hover || pressed { 6.0 } else { 4.5 };
    let c = (track.x + track.w * f, track.y + 1.5);
    cx.dl.circle(c, knob + 1.0, cx.pal.text);
    cx.dl.circle(c, knob - 0.5, on);
    if focused {
        cx.focus_ring(r, specular_theme::metrics::RADIUS_SM);
    }
    cx.hit(r, t);
    if (hover || pressed || focused) && alpha > 0.5 {
        cx.tooltip(&readout(volume), r);
    }
}

impl Chrome {
    /// The volume slider's value, top and mute: the value last asked for while the sound
    /// catches up. `None` without sound.
    pub(crate) fn volume_now(&self, st: &ViewerState) -> Option<(f32, f32, bool)> {
        let v = st.playback.as_ref()?.video?;
        let (vol, muted) = v.sound?;
        let vol = match self.volume_set {
            Some((s, at)) if self.volume_drag.is_some() || self.last_now - at <= HOLD => s,
            _ => vol,
        };
        Some((vol, v.volume_max.max(f32::EPSILON), muted))
    }

    /// `v` as the slider shows it: at the value asked for until the sound reports it (or
    /// [`HOLD`] passes).
    pub(crate) fn volume_shown(
        &mut self,
        v: &mut VideoPlayback,
        now: f64,
        want: &mut impl FnMut(f64),
    ) {
        let Some((set, at)) = self.volume_set else {
            return;
        };
        let Some((vol, _)) = v.sound.as_mut() else {
            self.volume_set = None;
            return;
        };
        let held = self.volume_drag.is_some();
        if !held && ((*vol - set).abs() < 0.005 || now - at > HOLD) {
            self.volume_set = None;
            return;
        }
        *vol = set;
        if !held {
            want((at + HOLD - now).max(0.0) + 0.01);
        }
    }

    /// Asks for `volume`.
    fn volume_cmd(&mut self, volume: f32, done: bool, now: f64) -> Response {
        self.volume_set = Some((volume, now));
        self.last_activity = now;
        Response::cmd(Command::Volume { volume, done })
    }

    /// The slider (hit rectangle `r`) pressed or dragged to `x`; `done` when let go.
    pub(crate) fn volume_drag_to(
        &mut self,
        r: Rect,
        x: f32,
        done: bool,
        st: &ViewerState,
        now: f64,
    ) -> Response {
        match self.volume_now(st) {
            Some((_, max, _)) => self.volume_cmd(volume_at(r, x, max), done, now),
            None => Response::used(),
        }
    }

    /// The wheel turned `dy` lines over the slider (away: louder).
    pub(crate) fn volume_wheel_by(&mut self, dy: f32, st: &ViewerState, now: f64) -> Response {
        let Some((v, max, _)) = self.volume_now(st) else {
            return Response::used();
        };
        // A touchpad's small turns add up to whole percents.
        if self.volume_wheel * dy < 0.0 {
            self.volume_wheel = 0.0;
        }
        self.volume_wheel += dy * STEP;
        let pct = self.volume_wheel.trunc();
        if pct == 0.0 {
            return Response::used();
        }
        self.volume_wheel -= pct;
        let to = whole(v + pct / 100.0).clamp(0.0, max);
        if to == v {
            return Response::used();
        }
        self.volume_cmd(to, true, now)
    }

    /// A key on the focused slider; `None`: not one of its keys.
    pub(crate) fn volume_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        st: &ViewerState,
        now: f64,
    ) -> Option<Response> {
        let m = combo.mods;
        if m.ctrl || m.alt || m.meta {
            return None;
        }
        let (v, max, _) = self.volume_now(st)?;
        let to = match named? {
            NamedKey::ArrowRight | NamedKey::ArrowUp => stepped(v, STEP, true, max),
            NamedKey::ArrowLeft | NamedKey::ArrowDown => stepped(v, STEP, false, max),
            NamedKey::PageUp => stepped(v, COARSE, true, max),
            NamedKey::PageDown => stepped(v, COARSE, false, max),
            NamedKey::Home => 0.0,
            NamedKey::End => max,
            _ => return None,
        };
        Some(if to == v {
            Response::used()
        } else {
            self.volume_cmd(to, true, now)
        })
    }

    /// The slider's screen-reader node: role, name, value as a percentage, range and step.
    /// `None` without sound.
    pub(crate) fn volume_node(&self, n: &mut Node, st: &ViewerState) -> Option<()> {
        let (v, max, muted) = self.volume_now(st)?;
        n.set_role(Role::Slider);
        n.set_label(t!("transport-volume"));
        let text = value_text(v);
        let text = if muted {
            specular_i18n::dot_join(&[text, t!("toast-muted")])
        } else {
            text
        };
        n.set_value(specular_i18n::strip_isolates(&text));
        n.set_numeric_value(f64::from((v * 100.0).round()));
        n.set_min_numeric_value(0.0);
        n.set_max_numeric_value(f64::from((max * 100.0).round()));
        n.set_numeric_value_step(f64::from(STEP));
        n.add_action(AkAction::Focus);
        n.add_action(AkAction::Increment);
        n.add_action(AkAction::Decrement);
        n.add_action(AkAction::SetValue);
        Some(())
    }

    /// A screen reader stepped the slider or set its value (a percentage).
    pub(crate) fn volume_a11y(
        &mut self,
        action: AkAction,
        data: Option<&ActionData>,
        st: &ViewerState,
    ) -> Response {
        let Some((v, max, _)) = self.volume_now(st) else {
            return Response::default();
        };
        let to = match (action, data) {
            (AkAction::Increment, _) => stepped(v, STEP, true, max),
            (AkAction::Decrement, _) => stepped(v, STEP, false, max),
            (AkAction::SetValue, Some(ActionData::NumericValue(p))) if p.is_finite() => {
                whole(*p as f32 / 100.0).clamp(0.0, max)
            }
            (AkAction::SetValue, Some(ActionData::Value(s))) => {
                let digits: String = s
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                match digits.parse::<f32>() {
                    Ok(p) => whole(p / 100.0).clamp(0.0, max),
                    Err(_) => return Response::default(),
                }
            }
            _ => return Response::default(),
        };
        let now = self.last_now;
        self.volume_cmd(to, true, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Rect {
        // A 100 px track from x = 18.
        Rect::new(10.0, 0.0, 100.0 + 2.0 * INSET, 32.0)
    }

    #[test]
    fn drag_maps_along_the_track() {
        let r = r();
        assert_eq!(volume_at(r, 18.0, 2.0), 0.0);
        assert_eq!(volume_at(r, 0.0, 2.0), 0.0, "before the start: silent");
        assert_eq!(volume_at(r, 118.0, 2.0), 2.0);
        assert_eq!(volume_at(r, 500.0, 2.0), 2.0, "past the end: the top");
        assert_eq!(volume_at(r, 58.0, 2.0), 0.8);
        assert_eq!(volume_at(r, 93.0, 2.0), 1.5);
        // The middle is normal, and a little either side catches there.
        assert_eq!(volume_at(r, 68.0, 2.0), 1.0);
        assert_eq!(volume_at(r, 69.0, 2.0), 1.0);
        assert_eq!(volume_at(r, 67.0, 2.0), 1.0);
        assert_eq!(volume_at(r, 70.0, 2.0), 1.04);
        // A cast device's own volume: the track is 0..=100 %, no catch.
        assert_eq!(volume_at(r, 68.0, 1.0), 0.5);
        assert_eq!(volume_at(r, 117.0, 1.0), 0.99);
        // Whole percents.
        let v = volume_at(r, 58.3, 2.0);
        assert_eq!(v, 0.81);
    }

    /// The knob's rim and the 100 % tick keep 3:1 on the bar's glass over a white and a
    /// black picture, in both themes (WCAG 1.4.11); the readout is the tooltip's text.
    #[test]
    fn parts_meet_non_text_contrast() {
        use specular_theme::{Color, Palette};
        let ratio = |a: Color, b: Color| {
            let (la, lb) = (a.luminance(), b.luminance());
            (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
        };
        let over =
            |top: Color, bottom: Color| bottom.lerp(top.with_alpha(1.0), top.a).with_alpha(1.0);
        for p in [Palette::dark(), Palette::light()] {
            for under in [Color::WHITE, Color::BLACK] {
                let glass = over(p.glass, under);
                // The knob's rim and the tick.
                let r = ratio(p.text, glass);
                assert!(r >= 3.0, "{:?} over {under:?}: {r:.2}", p.mode);
            }
            let r = ratio(p.tooltip_text, over(p.tooltip_bg, Color::WHITE));
            assert!(r >= 4.5, "{:?} readout: {r:.2}", p.mode);
        }
    }

    #[test]
    fn steps_land_on_the_grid() {
        assert_eq!(stepped(0.8, STEP, true, 2.0), 0.85);
        assert_eq!(stepped(0.82, STEP, true, 2.0), 0.85);
        assert_eq!(stepped(0.82, STEP, false, 2.0), 0.8);
        assert_eq!(stepped(0.8, STEP, false, 2.0), 0.75);
        assert_eq!(stepped(0.0, STEP, false, 2.0), 0.0);
        assert_eq!(stepped(2.0, STEP, true, 2.0), 2.0);
        assert_eq!(stepped(1.0, STEP, true, 1.0), 1.0);
        assert_eq!(stepped(0.9, COARSE, true, 2.0), 1.0);
    }
}
