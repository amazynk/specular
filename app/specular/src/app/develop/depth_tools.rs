//! The Relight and Depth blur cards: their sliders, the depth map's state (with
//! Stop, Compute again, or a link to Settings › Local AI when the model is missing) and
//! "Pick focus": the next click on the picture sets the focus to the depth there.
//!
//! Both cards are off until used (each is the identity at its defaults). They hook into
//! the Effects cards' plumbing (`effects.rs`): titles, float fields, events.

use specular_develop::mask::depth as dm;
use specular_develop::model::{Fields, ModuleKind, Value};
use specular_develop::ops::depth::blur::DepthBlur;
use specular_develop::ops::depth::relight::Relight;
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control};
use specular_widgets::slider::{Slider, SliderSpec, Unit};

use super::ai_remove::{NetState, state_line};
use super::controls::{Request, Session};
use crate::app::App;

/// The depth modules, in the order their cards show.
pub(super) const TOOLS: [ModuleKind; 2] = [ModuleKind::RELIGHT, ModuleKind::DEPTH_BLUR];

/// Control ids (300–319).
pub(super) mod id {
    pub const RL_NEAR: u32 = 300;
    pub const RL_FAR: u32 = 301;
    pub const RL_SPLIT: u32 = 302;
    pub const RL_TRANSITION: u32 = 303;
    pub const RL_STATE: u32 = 304;
    pub const RL_ACTION: u32 = 305;
    pub const DB_AMOUNT: u32 = 310;
    pub const DB_FOCUS: u32 = 311;
    pub const DB_RANGE: u32 = 312;
    pub const DB_HIGHLIGHTS: u32 = 313;
    pub const DB_NEAR: u32 = 314;
    pub const DB_PICK: u32 = 315;
    pub const DB_STATE: u32 = 316;
    pub const DB_ACTION: u32 = 317;
}

/// Whether a control belongs to these cards.
pub(super) fn is_ours(control: u32) -> bool {
    (id::RL_NEAR..=id::DB_ACTION).contains(&control)
}

pub(super) fn title(kind: ModuleKind) -> Option<String> {
    Some(match kind {
        ModuleKind::RELIGHT => t!("develop-module-relight"),
        ModuleKind::DEPTH_BLUR => t!("develop-module-depth-blur"),
        k => return super::perspective::title(k),
    })
}

pub(super) fn float_field(control: u32) -> Option<(ModuleKind, &'static str)> {
    use ModuleKind as K;
    Some(match control {
        id::RL_NEAR => (K::RELIGHT, "near"),
        id::RL_FAR => (K::RELIGHT, "far"),
        id::RL_SPLIT => (K::RELIGHT, "split"),
        id::RL_TRANSITION => (K::RELIGHT, "transition"),
        id::DB_AMOUNT => (K::DEPTH_BLUR, "amount"),
        id::DB_FOCUS => (K::DEPTH_BLUR, "focus"),
        id::DB_RANGE => (K::DEPTH_BLUR, "range"),
        id::DB_HIGHLIGHTS => (K::DEPTH_BLUR, "highlights"),
        c => return super::perspective::float_field(c),
    })
}

pub(super) fn module_of(control: u32) -> Option<ModuleKind> {
    Some(match control {
        id::RL_STATE | id::RL_ACTION => ModuleKind::RELIGHT,
        id::DB_NEAR | id::DB_PICK | id::DB_STATE | id::DB_ACTION => ModuleKind::DEPTH_BLUR,
        c => return float_field(c).map(|f| f.0),
    })
}

pub(super) fn field_label(kind: ModuleKind, field: &str) -> Option<String> {
    use ModuleKind as K;
    Some(match (kind, field) {
        (K::RELIGHT, "near") => t!("develop-relight-near"),
        (K::RELIGHT, "far") => t!("develop-relight-far"),
        (K::RELIGHT, "split") => t!("develop-relight-split"),
        (K::RELIGHT, "transition") => t!("develop-relight-transition"),
        (K::DEPTH_BLUR, "amount") => t!("develop-depth-blur-amount"),
        (K::DEPTH_BLUR, "focus") => t!("develop-depth-blur-focus"),
        (K::DEPTH_BLUR, "range") => t!("develop-depth-blur-range"),
        (K::DEPTH_BLUR, "highlights") => t!("develop-depth-blur-highlights"),
        (K::DEPTH_BLUR, "near") => t!("develop-depth-blur-near"),
        (k, f) => return super::perspective::field_label(k, f),
    })
}

/// Buttons: not values of the edit.
pub(super) fn is_event(control: u32) -> bool {
    matches!(control, id::RL_ACTION | id::DB_ACTION | id::DB_PICK)
}

fn slider(id: u32, spec: SliderSpec) -> Control {
    Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    }
}

fn ev(id: u32, label: String) -> Control {
    slider(
        id,
        SliderSpec::new(label, -4.0, 4.0, 0.0)
            .steps(0.05, 0.5)
            .unit(Unit::Ev, 2)
            .signed(),
    )
}

fn pct(id: u32, label: String, (lo, hi): (f32, f32), default: f32, decimals: u8) -> Control {
    slider(
        id,
        SliderSpec::new(label, lo, hi, default)
            .steps(0.005, 0.05)
            .unit(Unit::Percent, decimals),
    )
}

fn note(id: u32) -> Control {
    Control {
        id,
        body: Body::Note(String::new()),
    }
}

/// The two cards, closed and off-until-used.
pub(super) fn cards() -> Vec<Card> {
    let r = Relight::default();
    let d = DepthBlur::default();
    let card = |kind: ModuleKind, controls: Vec<Control>| Card {
        id: u32::from(kind.0),
        title: title(kind).unwrap_or_default(),
        enabled: Some(true),
        expanded: false,
        controls,
    };
    vec![
        card(
            ModuleKind::RELIGHT,
            vec![
                ev(id::RL_NEAR, t!("develop-relight-near")),
                ev(id::RL_FAR, t!("develop-relight-far")),
                pct(
                    id::RL_SPLIT,
                    t!("develop-relight-split"),
                    (0.0, 1.0),
                    r.split,
                    0,
                ),
                pct(
                    id::RL_TRANSITION,
                    t!("develop-relight-transition"),
                    (0.01, 1.0),
                    r.transition,
                    0,
                ),
                note(id::RL_STATE),
            ],
        ),
        card(
            ModuleKind::DEPTH_BLUR,
            vec![
                pct(
                    id::DB_AMOUNT,
                    t!("develop-depth-blur-amount"),
                    (0.0, 0.05),
                    d.amount,
                    1,
                ),
                pct(
                    id::DB_FOCUS,
                    t!("develop-depth-blur-focus"),
                    (0.0, 1.0),
                    d.focus,
                    0,
                ),
                Control {
                    id: id::DB_PICK,
                    body: Body::Choice(Choice::button(t!("develop-depth-blur-pick"))),
                },
                pct(
                    id::DB_RANGE,
                    t!("develop-depth-blur-range"),
                    (0.0, 1.0),
                    d.range,
                    0,
                ),
                pct(
                    id::DB_HIGHLIGHTS,
                    t!("develop-depth-blur-highlights"),
                    (0.0, 1.0),
                    d.highlights,
                    0,
                ),
                Control {
                    id: id::DB_NEAR,
                    body: Body::Choice(Choice::new(
                        t!("develop-depth-blur-near"),
                        vec![t!("develop-mask-off"), t!("develop-mask-on")],
                    )),
                },
                note(id::DB_STATE),
            ],
        ),
    ]
}

/// Puts a value on a control that is not a plain float (true when it was ours).
pub(super) fn read(s: &Session, f: &Fields, ctl: &mut Control) -> bool {
    match (&mut ctl.body, ctl.id) {
        (Body::Choice(c), id::DB_NEAR) => {
            c.set(usize::from(f.bool_or("near", true)));
        }
        (Body::Choice(c), id::DB_PICK) => {
            c.label = if s.focus_picking {
                t!("develop-depth-blur-picking")
            } else {
                t!("develop-depth-blur-pick")
            };
        }
        (Body::Note(n), id::RL_STATE | id::DB_STATE) => *n = s.depth_line(),
        (Body::Choice(c), id::RL_ACTION | id::DB_ACTION) => c.label = s.depth_action_label(),
        _ => return super::perspective::read(s, f, ctl),
    }
    true
}

/// Writes a control that is not a plain float (`Some(false)`: a button).
pub(super) fn write(ctl: &Control, f: &mut Fields) -> Option<bool> {
    match (&ctl.body, ctl.id) {
        (Body::Choice(c), id::DB_NEAR) => f.set("near", Value::Bool(c.selected == 1)),
        (_, id::DB_PICK | id::RL_ACTION | id::DB_ACTION | id::RL_STATE | id::DB_STATE) => {
            return Some(false);
        }
        _ => return super::perspective::write(ctl, f),
    }
    Some(true)
}

/// What the action button does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Stop,
    Retry,
    Settings,
}

impl Session {
    /// The depth map's key for the depth modules of this photo.
    fn depth_key(&self) -> [u8; 32] {
        let model = specular_develop::ops::depth::model_of(&self.fields_of(ModuleKind::DEPTH_BLUR));
        dm::key(&dm::photo(&self.edit.source), &model)
    }

    fn depth_state(&self) -> Option<&NetState> {
        self.ai2_state.get(&self.depth_key())
    }

    /// The depth map's state line.
    pub(super) fn depth_line(&self) -> String {
        if !TOOLS.iter().any(|k| self.module_on(*k)) && self.depth_state().is_none() {
            return t!("develop-depth-off");
        }
        state_line(self.depth_state(), t!("develop-depth-computing"))
    }

    fn depth_action(&self) -> Option<Action> {
        match self.depth_state()? {
            NetState::Computing => Some(Action::Stop),
            NetState::Stopped | NetState::Failed(_) => Some(Action::Retry),
            NetState::Missing(_) => Some(Action::Settings),
            NetState::Ready => None,
        }
    }

    fn depth_action_label(&self) -> String {
        match self.depth_action() {
            Some(Action::Stop) => t!("develop-ai-net-stop"),
            Some(Action::Retry) => t!("develop-ai-net-retry"),
            _ => t!("develop-ai-open-settings"),
        }
    }

    /// Adds or drops the action buttons after the state lines (rebuilt with the panel).
    pub(super) fn sync_depth(&self, cards: &mut [Card]) {
        let want = self.depth_action().is_some();
        for (card, state, action) in [
            (ModuleKind::RELIGHT, id::RL_STATE, id::RL_ACTION),
            (ModuleKind::DEPTH_BLUR, id::DB_STATE, id::DB_ACTION),
        ] {
            let Some(c) = cards.iter_mut().find(|c| c.id == u32::from(card.0)) else {
                continue;
            };
            let has = c.controls.iter().any(|x| x.id == action);
            if want && !has {
                let at = c
                    .controls
                    .iter()
                    .position(|x| x.id == state)
                    .map_or(c.controls.len(), |i| i + 1);
                let mut b = Control {
                    id: action,
                    body: Body::Choice(Choice::button(String::new())),
                };
                read(self, &Fields::new(), &mut b);
                c.controls.insert(at, b);
            } else if !want && has {
                c.controls.retain(|x| x.id != action);
            }
        }
    }

    /// A depth card's button.
    pub(super) fn depth_event(&mut self, control: u32) -> Option<Vec<Request>> {
        Some(match control {
            id::DB_PICK => {
                self.focus_picking = !self.focus_picking;
                self.sync_panel();
                if self.focus_picking {
                    vec![Request::Say(t!("develop-depth-blur-picking"))]
                } else {
                    Vec::new()
                }
            }
            id::RL_ACTION | id::DB_ACTION => match self.depth_action() {
                Some(Action::Stop) => {
                    self.ai2_request = Some(super::ai_remove::Ask::Stop);
                    Vec::new()
                }
                Some(Action::Retry) => {
                    self.ai2_request = Some(super::ai_remove::Ask::Retry(self.depth_key()));
                    Vec::new()
                }
                _ => vec![Request::OpenLocalAi("ai.depth")],
            },
            _ => return None,
        })
    }

    /// The focus picked at frame point `f`: the depth there, one step.
    pub(super) fn set_focus_at(&mut self, f: (f64, f64), now_ms: i64) -> Option<String> {
        self.focus_picking = false;
        let r = specular_develop::mask::ai::store()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&self.depth_key());
        let Some(r) = r else {
            self.sync_panel();
            return Some(t!("develop-depth-blur-no-map"));
        };
        let d = r.sample(f.0 as f32, f.1 as f32).clamp(0.0, 1.0);
        let kind = ModuleKind::DEPTH_BLUR;
        let before = self.fields_of(kind);
        let after = before.clone().with("focus", Value::Float(d));
        let step =
            specular_develop::model::Step::params(kind, self.inst(kind), &before, &after, now_ms);
        let _ = self.history.push(&mut self.edit, step);
        self.sync_panel();
        Some(t!(
            "develop-depth-blur-focus-set",
            depth = specular_i18n::fixed(d * 100.0, 0)
        ))
    }
}

impl App {
    /// A click on the picture while picking the focus.
    pub(super) fn develop_focus_press(&mut self, lp: (f32, f32)) -> bool {
        let ms = self.develop_ms();
        let (Some(s), Some(m)) = (self.develop.session.as_mut(), self.develop.map.as_ref()) else {
            return false;
        };
        if !s.focus_picking {
            return false;
        }
        let (fx, fy) = m.to_frame(lp);
        if !(fx.is_finite() && fy.is_finite()) {
            return false;
        }
        if let Some(say) = s.set_focus_at((fx, fy), ms) {
            self.chrome.announce_status(say);
        }
        self.develop.unsaved = Some(self.now());
        self.redraw = true;
        true
    }
}
