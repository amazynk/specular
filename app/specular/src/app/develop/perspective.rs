//! The Perspective card: upright (Off, Auto, Level, Vertical, Full, Guided), the keystone
//! sliders, scale and Reset; guides are drawn in the crop tool ([`super::crop`]).
//!
//! The automatic modes run on request: choosing one reads the lens-corrected picture once,
//! finds its line segments and solves ([`specular_develop::ops::geometry::upright::run`]);
//! the result is one history step, or a plain reason when there are too few lines.
//! It hooks into the Effects cards' plumbing (`effects.rs`, `depth_tools.rs`): titles,
//! float fields, labels.

use specular_develop::model::{Fields, ModuleKind, Step, Value};
use specular_develop::ops::geometry::persp::{M3, MAX_GUIDES, Mode, PerspParams};
use specular_develop::ops::geometry::upright;
use specular_develop::preview::UprightLines;
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control};
use specular_widgets::slider::{Slider, SliderSpec, Unit};

use super::controls::{Request, Session};
use specular_develop::model::ModuleParams;

/// Control ids (320–339).
pub(super) mod id {
    pub const MODE: u32 = 320;
    pub const VERTICAL: u32 = 321;
    pub const HORIZONTAL: u32 = 322;
    pub const ROTATE: u32 = 323;
    pub const ASPECT: u32 = 324;
    pub const SCALE: u32 = 325;
    pub const OFFSET_X: u32 = 326;
    pub const OFFSET_Y: u32 = 327;
    pub const AUTO_SCALE: u32 = 328;
    pub const GUIDES: u32 = 329;
    pub const RESET: u32 = 330;
}

/// The modes in the order the Upright row steps through them.
pub(super) const MODES: [Mode; 6] = [
    Mode::Off,
    Mode::Auto,
    Mode::Level,
    Mode::Vertical,
    Mode::Full,
    Mode::Guided,
];

/// What is waiting for the user's yes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Confirm {
    Reset,
    Guide(usize),
}

/// The session's perspective state.
#[derive(Clone, Debug, Default)]
pub(super) struct State {
    pub confirm: Option<Confirm>,
    /// The picture's keystone ([`super::crop::Layout::keystone`]): the Straighten slider
    /// keeps the crop inside the warped photo with it.
    pub keystone: Option<(M3, (f64, f64))>,
    /// An automatic run asked for and not back yet.
    pub waiting: Option<Mode>,
}

pub(super) fn is_ours(control: u32) -> bool {
    (id::MODE..=339).contains(&control)
}

pub(super) fn title(kind: ModuleKind) -> Option<String> {
    (kind == ModuleKind::PERSPECTIVE).then(|| t!("develop-module-perspective"))
}

pub(super) fn float_field(control: u32) -> Option<(ModuleKind, &'static str)> {
    let k = ModuleKind::PERSPECTIVE;
    Some(match control {
        id::VERTICAL => (k, "vertical"),
        id::HORIZONTAL => (k, "horizontal"),
        id::ROTATE => (k, "rotate"),
        id::ASPECT => (k, "aspect"),
        id::SCALE => (k, "scale"),
        id::OFFSET_X => (k, "offset_x"),
        id::OFFSET_Y => (k, "offset_y"),
        _ => return None,
    })
}

pub(super) fn module_of(control: u32) -> Option<ModuleKind> {
    is_ours(control).then_some(ModuleKind::PERSPECTIVE)
}

pub(super) fn field_label(kind: ModuleKind, field: &str) -> Option<String> {
    if kind != ModuleKind::PERSPECTIVE {
        return None;
    }
    Some(match field {
        "mode" | "solved" => t!("develop-persp-upright"),
        "vertical" => t!("develop-persp-vertical"),
        "horizontal" => t!("develop-persp-horizontal"),
        "rotate" => t!("develop-persp-rotate"),
        "aspect" => t!("develop-persp-aspect"),
        "scale" => t!("develop-persp-scale"),
        "offset_x" => t!("develop-persp-offset-x"),
        "offset_y" => t!("develop-persp-offset-y"),
        "auto_scale" => t!("develop-persp-auto-scale"),
        "guides" => t!("develop-persp-guides"),
        "focal_mm35" => t!("develop-persp-focal"),
        _ => t!("develop-module-perspective"),
    })
}

/// Buttons and the Upright row: handled as events ([`Session::persp_event`]).
pub(super) fn is_event(control: u32) -> bool {
    matches!(control, id::MODE | id::GUIDES | id::RESET)
}

fn mode_name(m: Mode) -> String {
    match m {
        Mode::Off => t!("develop-persp-mode-off"),
        Mode::Auto => t!("develop-persp-mode-auto"),
        Mode::Level => t!("develop-persp-mode-level"),
        Mode::Vertical => t!("develop-persp-mode-vertical"),
        Mode::Full => t!("develop-persp-mode-full"),
        Mode::Guided => t!("develop-persp-mode-guided"),
    }
}

fn slider(id: u32, spec: SliderSpec) -> Control {
    Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    }
}

fn amount(id: u32, label: String, (lo, hi): (f32, f32)) -> Control {
    slider(
        id,
        SliderSpec::new(label, lo, hi, 0.0)
            .steps(1.0, 10.0)
            .unit(Unit::Plain, 0)
            .signed(),
    )
}

/// The card: closed, with its own switch.
pub(super) fn card() -> Card {
    let d = PerspParams::default();
    Card {
        id: u32::from(ModuleKind::PERSPECTIVE.0),
        title: t!("develop-module-perspective"),
        enabled: Some(true),
        expanded: false,
        controls: vec![
            Control {
                id: id::MODE,
                body: Body::Choice(Choice::new(
                    t!("develop-persp-upright"),
                    MODES.iter().map(|m| mode_name(*m)).collect(),
                )),
            },
            Control {
                id: id::GUIDES,
                body: Body::Choice(Choice::button(t!("develop-persp-draw-guides"))),
            },
            amount(id::VERTICAL, t!("develop-persp-vertical"), (-100.0, 100.0)),
            amount(
                id::HORIZONTAL,
                t!("develop-persp-horizontal"),
                (-100.0, 100.0),
            ),
            slider(
                id::ROTATE,
                SliderSpec::new(t!("develop-persp-rotate"), -10.0, 10.0, 0.0)
                    .steps(0.1, 1.0)
                    .unit(Unit::Degrees, 1)
                    .signed(),
            ),
            amount(id::ASPECT, t!("develop-persp-aspect"), (-100.0, 100.0)),
            slider(
                id::SCALE,
                SliderSpec::new(t!("develop-persp-scale"), 50.0, 150.0, d.scale)
                    .steps(1.0, 10.0)
                    .unit(Unit::Plain, 0),
            ),
            amount(id::OFFSET_X, t!("develop-persp-offset-x"), (-100.0, 100.0)),
            amount(id::OFFSET_Y, t!("develop-persp-offset-y"), (-100.0, 100.0)),
            Control {
                id: id::AUTO_SCALE,
                body: Body::Choice(Choice::new(
                    t!("develop-persp-auto-scale"),
                    vec![t!("develop-mask-off"), t!("develop-mask-on")],
                )),
            },
            Control {
                id: id::RESET,
                body: Body::Choice(Choice::button(t!("develop-persp-reset"))),
            },
        ],
    }
}

/// Puts a value on a control that is not a plain float (true when it was ours).
pub(super) fn read(s: &Session, f: &Fields, ctl: &mut Control) -> bool {
    if !is_ours(ctl.id) || float_field(ctl.id).is_some() {
        return false;
    }
    let p = PerspParams::from_fields(f);
    if let Body::Choice(c) = &mut ctl.body {
        match ctl.id {
            id::MODE => {
                let m = s.persp.waiting.unwrap_or(p.mode);
                c.set(MODES.iter().position(|x| *x == m).unwrap_or(0));
            }
            id::AUTO_SCALE => {
                c.set(usize::from(p.auto_scale));
            }
            id::GUIDES => {
                c.label = t!(
                    "develop-persp-draw-guides-n",
                    n = p.guides.len(),
                    max = MAX_GUIDES
                );
            }
            _ => {}
        }
    }
    true
}

/// Writes a control that is not a plain float (`Some(false)`: an event).
pub(super) fn write(ctl: &Control, f: &mut Fields) -> Option<bool> {
    if !is_ours(ctl.id) || float_field(ctl.id).is_some() {
        return None;
    }
    match (&ctl.body, ctl.id) {
        (Body::Choice(c), id::AUTO_SCALE) => {
            f.set("auto_scale", Value::Bool(c.selected == 1));
            Some(true)
        }
        _ => Some(false),
    }
}

/// The plain reason an automatic run gave nothing.
pub(super) fn refusal(r: upright::Refusal, m: Mode) -> String {
    match r {
        upright::Refusal::FewLines => t!("develop-perspective-few-lines", mode = mode_name(m)),
    }
}

impl Session {
    /// The edit's perspective params (defaults without the module).
    pub(super) fn persp(&self) -> PerspParams {
        PerspParams::from_fields(&self.fields_of(ModuleKind::PERSPECTIVE))
    }

    /// Sets the perspective params as one history step; whether it changed.
    pub(super) fn set_persp(&mut self, p: &PerspParams, now_ms: i64) -> bool {
        let kind = ModuleKind::PERSPECTIVE;
        let before = self.fields_of(kind);
        let mut after = before.clone();
        // Optional fields go when unset.
        for k in ["guides", "solved", "focal_mm35"] {
            after.remove(k);
        }
        after.overlay(&p.to_fields());
        if before == after {
            return false;
        }
        let step = Step::params(kind, self.inst(kind), &before, &after, now_ms);
        if self.history.push(&mut self.edit, step).is_err() {
            return false;
        }
        self.after_step();
        true
    }

    /// The Upright row, Draw guides and Reset.
    pub(super) fn persp_event(&mut self, control: u32, now_ms: i64) -> Vec<Request> {
        match control {
            id::MODE => {
                let chosen = match self.panel.control(id::MODE).map(|c| &c.body) {
                    Some(Body::Choice(c)) => MODES.get(c.selected).copied().unwrap_or(Mode::Off),
                    _ => return Vec::new(),
                };
                let say = t!("develop-persp-mode-set", mode = mode_name(chosen));
                if chosen.is_automatic() {
                    self.persp.waiting = Some(chosen);
                    self.sync_panel();
                    return vec![
                        Request::Upright(chosen),
                        Request::Say(t!("develop-persp-analysing")),
                    ];
                }
                self.persp.waiting = None;
                let p = PerspParams {
                    mode: chosen,
                    ..self.persp()
                };
                let mut out = Vec::new();
                if self.set_persp(&p, now_ms) {
                    out.push(Request::Render);
                }
                if chosen == Mode::Guided && p.guides.is_empty() {
                    out.push(Request::CropGuides);
                }
                out.push(Request::Say(say));
                self.sync_panel();
                out
            }
            id::GUIDES => vec![Request::CropGuides],
            id::RESET => {
                let p = self.persp();
                if p.guides.is_empty() {
                    return self.persp_reset(now_ms);
                }
                self.persp.confirm = Some(Confirm::Reset);
                vec![Request::Confirm(
                    t!("develop-persp-reset-confirm", n = p.guides.len()),
                    t!("develop-persp-reset-ok"),
                )]
            }
            _ => Vec::new(),
        }
    }

    fn persp_reset(&mut self, now_ms: i64) -> Vec<Request> {
        let mut out = Vec::new();
        if self.set_persp(&PerspParams::default(), now_ms) {
            out.push(Request::Render);
        }
        self.sync_panel();
        out.push(Request::Say(t!("develop-persp-reset-done")));
        out
    }

    /// Asks before removing guide `i`.
    pub(super) fn persp_ask_remove_guide(&mut self, i: usize) -> Vec<Request> {
        if i >= self.persp().guides.len() {
            return Vec::new();
        }
        self.persp.confirm = Some(Confirm::Guide(i));
        vec![Request::Confirm(
            t!("develop-persp-guide-remove-confirm", n = i + 1),
            t!("develop-persp-guide-remove-ok"),
        )]
    }

    /// The user said yes.
    pub(super) fn persp_confirmed(&mut self, now_ms: i64) -> Vec<Request> {
        match self.persp.confirm.take() {
            Some(Confirm::Reset) => self.persp_reset(now_ms),
            Some(Confirm::Guide(i)) => {
                let mut p = self.persp();
                if i >= p.guides.len() {
                    return Vec::new();
                }
                p.guides.remove(i);
                let mut out = Vec::new();
                if self.set_persp(&p, now_ms) {
                    out.push(Request::Render);
                }
                self.sync_panel();
                out.push(Request::Say(t!("develop-persp-guide-removed")));
                out
            }
            None => Vec::new(),
        }
    }

    /// New guides from the crop tool: mode Guided, one step.
    pub(super) fn set_guides(&mut self, guides: Vec<[f32; 4]>, now_ms: i64) -> bool {
        let p = PerspParams {
            mode: Mode::Guided,
            guides: guides.into_iter().take(MAX_GUIDES).collect(),
            ..self.persp()
        };
        let ok = self.set_persp(&p, now_ms);
        self.sync_panel();
        ok
    }

    /// Alt+arrows in the crop tool: the keystone sliders by `dv` and `dh`.
    pub(super) fn nudge_keystone(&mut self, dv: f32, dh: f32, now_ms: i64) -> Option<String> {
        let mut p = self.persp();
        p.vertical = (p.vertical + dv).clamp(-100.0, 100.0);
        p.horizontal = (p.horizontal + dh).clamp(-100.0, 100.0);
        if !self.set_persp(&p, now_ms) {
            return None;
        }
        self.sync_panel();
        Some(t!(
            "develop-persp-keystone-now",
            vertical = specular_i18n::fixed(p.vertical, 0),
            horizontal = specular_i18n::fixed(p.horizontal, 0)
        ))
    }

    /// An automatic run came back: one step, or the reason it gave nothing.
    pub(super) fn upright_done(
        &mut self,
        mode: Mode,
        found: Result<UprightLines, String>,
        now_ms: i64,
    ) -> Vec<Request> {
        self.persp.waiting = None;
        let r = match found {
            Ok(l) => upright::run(mode, &self.persp(), &l).map_err(|r| refusal(r, mode)),
            Err(e) => Err(t!("develop-persp-failed", error = e)),
        };
        let out = match r {
            Ok(p) => {
                let mut out = Vec::new();
                if self.set_persp(&p, now_ms) {
                    out.push(Request::Render);
                }
                let s = p.solved.unwrap_or_default();
                out.push(Request::Say(t!(
                    "develop-persp-solved",
                    mode = mode_name(mode),
                    vertical = specular_i18n::fixed(s[0], 1),
                    horizontal = specular_i18n::fixed(s[1], 1),
                    rotate = specular_i18n::fixed(s[2], 1)
                )));
                out
            }
            Err(why) => vec![Request::Say(why)],
        };
        self.sync_panel();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::model::fields::Choice as _;

    #[test]
    fn the_modes_have_their_stored_names() {
        let names: Vec<&str> = MODES.iter().map(|m| m.name()).collect();
        assert_eq!(
            names,
            ["off", "auto", "level", "vertical", "full", "guided"]
        );
        assert!(is_event(id::MODE) && !is_event(id::VERTICAL));
        assert_eq!(
            float_field(id::SCALE),
            Some((ModuleKind::PERSPECTIVE, "scale"))
        );
        assert_eq!(module_of(id::RESET), Some(ModuleKind::PERSPECTIVE));
    }
}
