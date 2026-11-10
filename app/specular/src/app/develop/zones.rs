//! The tone equalizer's zones: one slider per EV zone of the mask
//! (−8 … 0 EV) in the Tone zones card, and "Adjust on the picture" in the Tone card (a way in
//! that needs no zone names): a press on the picture picks the
//! zone under the pointer (its luminance at the equalizer's input, through mask exposure
//! and contrast), and dragging up or down raises or lowers it. The drag's steps merge into
//! one history step, as a slider's do.

use specular_develop::model::{Fields, ModuleKind, Value};
use specular_develop::ops::tone::equalizer::{ToneEq, ZONES};
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Control};
use specular_widgets::slider::{Slider, SliderSpec, Unit};

use super::controls::{Request, Session, id};
use crate::app::App;

/// A zone's slider range (EV).
const LIMIT: f32 = 4.0;
/// EV per logical pixel dragged.
const EV_PER_PX: f32 = 0.01;

/// The zone drag in progress.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Drag {
    /// Where the press was (logical px, y).
    pub start_y: f32,
    /// The zone under the press, once the pick is back, and its value then.
    pub zone: Option<(usize, f32)>,
    /// The button went up before the pick was back (the pick still names the zone).
    pub released: bool,
}

/// The zone of control `c`.
fn zone(c: u32) -> usize {
    (c - id::ZONE) as usize
}

/// A zone's name: its EV, with a true minus ("−8 EV").
pub(super) fn zone_label(i: usize) -> String {
    let ev = ZONES[i.min(8)];
    let digits = format!("{}", ev.abs() as i32);
    t!(
        "develop-tone-zone",
        ev = specular_i18n::signed_number(&digits, ev < 0.0, false)
    )
}

/// "Adjust on the picture".
pub(super) fn button() -> Control {
    Control {
        id: id::ZONE_DRAG,
        body: Body::Choice(Choice::button(button_label(false))),
    }
}

/// The nine zone sliders.
pub(super) fn sliders() -> Vec<Control> {
    (0..ZONES.len())
        .map(|i| Control {
            id: id::ZONE + i as u32,
            body: Body::Slider(Slider::new(
                SliderSpec::new(zone_label(i), -LIMIT, LIMIT, 0.0)
                    .steps(0.05, 0.5)
                    .unit(Unit::Ev, 2)
                    .signed(),
            )),
        })
        .collect()
}

pub(super) fn button_label(on: bool) -> String {
    if on {
        t!("develop-tone-zones-done")
    } else {
        t!("develop-tone-zones-adjust")
    }
}

/// The zone slider `c`'s value in the module's fields.
pub(super) fn read(f: &Fields, c: u32) -> f32 {
    f.floats_or("zones", [0f32; 9])[zone(c).min(8)]
}

/// Writes zone slider `c`'s value `v` into the fields.
pub(super) fn write(f: &mut Fields, c: u32, v: f32) {
    let mut z = f.floats_or("zones", [0f32; 9]);
    z[zone(c).min(8)] = v;
    f.set("zones", Value::Floats(z.to_vec()));
}

impl Session {
    /// "Adjust on the picture" on or off.
    pub(super) fn toggle_zone_edit(&mut self) -> Request {
        self.zone_edit = !self.zone_edit;
        self.zone_drag = None;
        if self.zone_edit {
            self.masks.active = None;
            self.nd_edit = false;
        }
        self.sync_panel();
        Request::Say(if self.zone_edit {
            t!("develop-tone-zones-adjusting")
        } else {
            t!("develop-tone-zones-done")
        })
    }

    /// A press on the picture while adjusting zones: the drag starts (the zone follows
    /// from the pick). False when zones are not being adjusted.
    pub(super) fn zone_press(&mut self, y: f32) -> bool {
        if !self.zone_edit {
            return false;
        }
        self.zone_drag = Some(Drag {
            start_y: y,
            zone: None,
            released: false,
        });
        true
    }

    /// Waiting for the colour under a zone press.
    pub(super) fn zone_picking(&self) -> bool {
        self.zone_drag.as_ref().is_some_and(|d| d.zone.is_none())
    }

    /// The colour under the press (the equalizer's input) and, for an automatic mask, the
    /// mask's percentiles: its zone. What to say.
    pub(super) fn zone_picked(
        &mut self,
        rgb: [f64; 3],
        percentiles: Option<[f32; 2]>,
    ) -> Option<String> {
        let f = self.fields_of(ModuleKind::TONE_EQUALIZER);
        let z = ToneEq::from_fields(&f).zone_of(rgb.map(|v| v as f32), percentiles);
        let d = self.zone_drag.as_mut()?;
        d.zone = Some((z, f.floats_or("zones", [0f32; 9])[z]));
        if d.released {
            self.zone_drag = None;
        }
        Some(t!("develop-tone-zone-picked", zone = zone_label(z)))
    }

    /// The pointer at `y` during a zone drag: the zone's new value. Whether it changed.
    pub(super) fn zone_move(&mut self, y: f32, now_ms: i64) -> bool {
        let Some(Drag {
            start_y,
            zone: Some((z, v0)),
            released: false,
        }) = self.zone_drag.clone()
        else {
            return false;
        };
        let v = (v0 + (start_y - y) * EV_PER_PX).clamp(-LIMIT, LIMIT);
        let v = (v * 100.0).round() / 100.0;
        let mut f = self.fields_of(ModuleKind::TONE_EQUALIZER);
        write(&mut f, id::ZONE + z as u32, v);
        self.set_fields(ModuleKind::TONE_EQUALIZER, f, now_ms)
    }

    /// The button went up: the drag ends. Whether one was going on.
    pub(super) fn zone_release(&mut self) -> bool {
        match &mut self.zone_drag {
            None => false,
            // The pick is still to come: it ends the drag.
            Some(d) if d.zone.is_none() => {
                let was = !d.released;
                d.released = true;
                was
            }
            Some(_) => {
                self.zone_drag = None;
                true
            }
        }
    }
}

impl App {
    /// A press at `lp` on the picture while adjusting zones: picks the colour there.
    pub(super) fn develop_zone_press(&mut self, lp: (f32, f32)) -> bool {
        let pic = self.develop.pic;
        let Some(s) = self.develop.session.as_mut() else {
            return false;
        };
        if pic.w <= 0.0 || pic.h <= 0.0 || !s.zone_press(lp.1) {
            return false;
        }
        self.develop.preview.pick_before(ModuleKind::TONE_EQUALIZER);
        self.develop
            .preview
            .pick_at((lp.0 - pic.x) / pic.w, (lp.1 - pic.y) / pic.h);
        true
    }

    /// The pointer moved: a zone drag goes on. Whether one did.
    pub(super) fn develop_zone_move(&mut self, lp: Option<(f32, f32)>) -> bool {
        let ms = self.develop_ms();
        let (Some(lp), Some(s)) = (lp, self.develop.session.as_mut()) else {
            return false;
        };
        if s.zone_drag.as_ref().is_none_or(|d| d.released) {
            return false;
        }
        if s.zone_move(lp.1, ms) {
            self.develop.unsaved = Some(self.now());
        }
        self.redraw = true;
        true
    }

    /// The button went up: whether a zone drag ended.
    pub(super) fn develop_zone_release(&mut self) -> bool {
        self.develop
            .session
            .as_mut()
            .is_some_and(Session::zone_release)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_widgets::develop_panel::Event;

    fn session() -> Session {
        let p = std::path::PathBuf::from("zones.jpg");
        Session::new(
            p.clone(),
            super::super::controls::new_edit(&p),
            Default::default(),
        )
    }

    fn zones(s: &Session) -> [f32; 9] {
        s.fields_of(ModuleKind::TONE_EQUALIZER)
            .floats_or("zones", [0f32; 9])
    }

    #[test]
    fn zone_sliders_write_their_zone() {
        let mut s = session();
        let c = id::ZONE + 3;
        if let Some(Body::Slider(sl)) = s.panel.control_mut(c).map(|c| &mut c.body) {
            sl.set(1.25);
        }
        s.handle(
            &[Event::Value {
                control: c,
                part: 0,
                done: true,
            }],
            1_000,
        );
        let mut want = [0f32; 9];
        want[3] = 1.25;
        assert_eq!(zones(&s), want);
        // Undo puts the slider back.
        s.undo();
        assert_eq!(zones(&s), [0f32; 9]);
        match s.panel.control(c).map(|c| &c.body) {
            Some(Body::Slider(sl)) => assert_eq!(sl.value, 0.0),
            _ => panic!("no zone slider"),
        }
    }

    #[test]
    fn dragging_on_the_picture_moves_the_zone_under_the_press() {
        let mut s = session();
        // Not adjusting: a press is not ours.
        assert!(!s.zone_press(100.0));
        s.handle(
            &[Event::Value {
                control: id::ZONE_DRAG,
                part: 0,
                done: true,
            }],
            0,
        );
        assert!(s.zone_edit);
        assert!(s.zone_press(100.0));
        assert!(s.zone_picking());
        // Moves before the pick is back change nothing.
        assert!(!s.zone_move(50.0, 10));
        // A pixel at −3 EV: the −3 EV zone (index 5).
        let y = f64::from((-3f32).exp2());
        assert!(s.zone_picked([y, y, y], None).is_some());
        assert!(!s.zone_picking());
        // 50 px up: +0.5 EV.
        assert!(s.zone_move(50.0, 20));
        assert!((zones(&s)[5] - 0.5).abs() < 1e-6);
        // Further down past the limit: clamped.
        assert!(s.zone_move(100.0 + 1000.0, 30));
        assert_eq!(zones(&s)[5], -LIMIT);
        assert!(s.zone_release());
        assert!(!s.zone_release());
        // Released before the pick: the pick still ends it (not a point-colour pick).
        assert!(s.zone_press(10.0));
        assert!(s.zone_release());
        assert!(s.zone_picking());
        assert!(s.zone_picked([y, y, y], None).is_some());
        assert!(s.zone_drag.is_none());
        // One step for the whole drag.
        let steps = s.history.steps.len();
        assert_eq!(steps, 1, "{:?}", s.history_rows());
        // The slider shows it.
        match s.panel.control(id::ZONE + 5).map(|c| &c.body) {
            Some(Body::Slider(sl)) => assert_eq!(sl.value, -LIMIT),
            _ => panic!("no zone slider"),
        }
    }
}
