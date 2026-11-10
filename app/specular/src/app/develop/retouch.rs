//! Retouch in Develop: the Retouch card and spots on the
//! picture.
//!
//! - **Card.** Tool (heal, clone, fill, blur), area (spot or brush), layer (whole picture,
//!   tones or texture of the frequency separation), its radius, and size, feather, opacity
//!   and blur. With a spot selected they change that spot; otherwise they set the next one.
//!   The spots are listed; a row selects its spot. "Remove spot" (or Delete on the picture)
//!   asks first.
//! - **Picture.** While the card is open and on: a click places a spot, its source found
//!   automatically (the best match on rings around it, [`specular_develop::ops::retouch::search`]);
//!   with the brush area a drag paints the area. Dragging a spot's centre moves it, its
//!   source circle moves the source, its edge handle sizes it. Outlines show for every spot
//!   ([`specular_input::Action::DevelopSpotOutlines`] hides them; the selected spot always
//!   shows).
//! - **Keyboard.** [`specular_input::Action::DevelopSpotCrosshair`] puts a crosshair on the
//!   picture (the viewer's keyboard crosshair, [`super::super::cursor`]): arrows move it one
//!   photo pixel when zoomed in, one screen pixel when not, Shift ten; Enter or Space places
//!   a spot of the chosen tool there (its source found as for a click, one history step);
//!   Esc puts it away. Each step is read out.
//! - **History.** A drag, a stroke, a placed spot (with its source) or a value is one step.
//! - **Remove (AI)**: a spot or brushed area the Remove feature's network fills
//!   ([`super::ai_remove`]); the selected one says what it is doing, with Stop, Compute
//!   again or a link to Settings › Local AI. Until its fill is ready it heals.
//!
//! Spots are kept in source coordinates; [`Map`] carries them to the screen and back.

use specular_develop::model::{Fields, ModuleInstance, ModuleKind, Params, Registry, Step};
use specular_develop::ops::retouch::{Layer, Retouch, Spot, SpotMode};
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control};
use specular_widgets::mask_handles::{Handle, Outline, Role};
use specular_widgets::slider::{Slider, SliderSpec, Unit};

use super::controls::{Request, Session};
use super::masks::Map;

/// Retouch control ids: `RT | sub`.
pub(super) const RT: u32 = 0x8000_0000;

mod sub {
    pub const TOOL: u32 = 1;
    pub const AREA: u32 = 2;
    pub const LAYER: u32 = 3;
    pub const SPLIT: u32 = 4;
    pub const SIZE: u32 = 5;
    pub const FEATHER: u32 = 6;
    pub const OPACITY: u32 = 7;
    pub const BLUR: u32 = 8;
    pub const OUTLINES: u32 = 9;
    pub const REMOVE: u32 = 10;
    pub const HINT: u32 = 11;
    /// A removal spot's state and its action.
    pub const AI_STATE: u32 = 12;
    pub const AI_ACTION: u32 = 13;
    /// Spot rows: `SPOT0 + i`.
    pub const SPOT0: u32 = 0x100;
}

/// Spot rows the card lists.
pub(super) const LISTED: usize = 200;

pub(super) fn is_retouch(id: u32) -> bool {
    id & 0xC000_0000 == RT
}

const MODES: [SpotMode; 5] = [
    SpotMode::Heal,
    SpotMode::Clone,
    SpotMode::Fill,
    SpotMode::Blur,
    SpotMode::Remove,
];
const LAYERS: [Layer; 3] = [Layer::All, Layer::Low, Layer::High];
const KIND: ModuleKind = ModuleKind::RETOUCH;

/// What a drag on the picture changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Drag {
    Target(usize),
    Source(usize),
    Size(usize),
    /// A brush area being painted.
    Paint,
}

/// Retouch state of one session.
#[derive(Clone, Debug)]
pub(super) struct State {
    /// For the next spot.
    pub tool: SpotMode,
    pub layer: Layer,
    pub brush: bool,
    pub size: f32,
    pub feather: f32,
    pub opacity: f32,
    pub blur: f32,
    pub selected: Option<usize>,
    /// Outlines of unselected spots hidden.
    pub hidden: bool,
    pub drag: Option<Drag>,
    /// The brush area being painted (frame fractions).
    pub path: Vec<[f32; 2]>,
    /// A spot waiting for its source to be found.
    pub search: Option<usize>,
    /// The params before the change in progress.
    before: Option<Fields>,
    /// The spot a removal asks about.
    confirm: Option<usize>,
    /// The keyboard crosshair that places spots (frame fractions), while it is on.
    pub cross: Option<[f32; 2]>,
}

impl Default for State {
    fn default() -> Self {
        let d = Spot::default();
        State {
            tool: SpotMode::Heal,
            layer: Layer::All,
            brush: false,
            size: d.size,
            feather: d.feather,
            opacity: d.opacity,
            blur: d.blur,
            selected: None,
            hidden: false,
            drag: None,
            path: Vec::new(),
            search: None,
            before: None,
            confirm: None,
            cross: None,
        }
    }
}

/// The model a spot of `mode` names: the Remove feature's for removal spots.
fn remove_model(mode: SpotMode) -> String {
    if mode == SpotMode::Remove {
        super::ai_remove::model_id(specular_ai::Feature::Remove)
    } else {
        String::new()
    }
}

pub(super) fn mode_name(m: SpotMode) -> String {
    match m {
        SpotMode::Heal => t!("develop-retouch-heal"),
        SpotMode::Clone => t!("develop-retouch-clone"),
        SpotMode::Fill => t!("develop-retouch-fill"),
        SpotMode::Blur => t!("develop-retouch-blur"),
        SpotMode::Remove => t!("develop-retouch-remove-ai"),
    }
}

fn layer_name(l: Layer) -> String {
    match l {
        Layer::All => t!("develop-retouch-layer-all"),
        Layer::Low => t!("develop-retouch-layer-low"),
        Layer::High => t!("develop-retouch-layer-high"),
    }
}

/// "3. Heal" or "3. Heal, tones".
pub(super) fn spot_name(i: usize, s: &Spot) -> String {
    let n = (i + 1) as i64;
    if s.layer == Layer::All {
        t!("develop-retouch-spot", n = n, mode = mode_name(s.mode))
    } else {
        t!(
            "develop-retouch-spot-layer",
            n = n,
            mode = mode_name(s.mode),
            layer = layer_name(s.layer)
        )
    }
}

fn choice(id: u32, label: String, options: Vec<String>, selected: usize) -> Control {
    let mut c = Choice::new(label, options);
    c.set(selected);
    Control {
        id: RT | id,
        body: Body::Choice(c),
    }
}

fn pct(
    id: u32,
    label: String,
    (min, max): (f32, f32),
    default: f32,
    v: f32,
    decimals: u8,
) -> Control {
    let step = 10f32.powi(-i32::from(decimals) - 2);
    let mut s = Slider::new(
        SliderSpec::new(label, min, max, default)
            .steps(step, step * 10.0)
            .unit(Unit::Percent, decimals),
    );
    s.set(v);
    Control {
        id: RT | id,
        body: Body::Slider(s),
    }
}

/// The card (controls filled by [`Session::sync_retouch`]); closed at first, so clicks on
/// the picture place nothing until it is opened.
pub(super) fn card() -> Card {
    Card {
        id: u32::from(KIND.0),
        title: super::controls::module_title(KIND),
        enabled: Some(true),
        expanded: false,
        controls: Vec::new(),
    }
}

impl Session {
    /// The retouch params of the edit.
    pub(super) fn retouch(&self) -> Retouch {
        self.edit
            .module(KIND, 0)
            .map(|m| Retouch::of(&m.params))
            .unwrap_or_default()
    }

    /// A removal spot's patch key on this photo.
    pub(super) fn remove_key(&self, s: &Spot) -> [u8; 32] {
        specular_develop::ops::retouch::patch::key(
            &specular_develop::mask::depth::photo(&self.edit.source),
            s,
        )
    }

    /// Whether spots are placed and shown on the picture: the card is open and on.
    pub(super) fn retouch_on(&self) -> bool {
        let open = self
            .panel
            .cards
            .iter()
            .any(|c| c.id == u32::from(KIND.0) && c.expanded && c.enabled != Some(false));
        open && self.edit.module(KIND, 0).is_none_or(|m| m.enabled)
    }

    /// Writes `r` into the edit as part of the change in progress.
    fn rt_set(&mut self, r: &Retouch) {
        let reg = Registry::builtin();
        if self.edit.module(KIND, 0).is_none() {
            self.edit.insert(reg, ModuleInstance::new(reg, KIND, 0));
        }
        let Some(m) = self.edit.module_mut(KIND, 0) else {
            return;
        };
        if self.retouch_state.before.is_none() {
            self.retouch_state.before = Some(m.params.to_fields());
        }
        m.params = Params::from_fields(KIND, r.to_fields());
    }

    /// Ends the change in progress: one history step.
    pub(super) fn rt_commit(&mut self, now_ms: i64) {
        let Some(before) = self.retouch_state.before.take() else {
            return;
        };
        let Some(m) = self.edit.module_mut(KIND, 0) else {
            return;
        };
        let after = m.params.to_fields();
        if after == before {
            return;
        }
        m.params = Params::from_fields(KIND, before.clone());
        let step = Step::params(KIND, 0, &before, &after, now_ms);
        let _ = self.history.push(&mut self.edit, step);
        self.after_step();
    }

    /// The card's controls from the edit and the state.
    fn retouch_controls(&self) -> Vec<Control> {
        let r = self.retouch();
        let st = &self.retouch_state;
        let sel = st.selected.and_then(|i| r.spots.get(i).map(|s| (i, s)));
        let (mode, layer, size, feather, opacity, blur) = match sel {
            Some((_, s)) => (s.mode, s.layer, s.size, s.feather, s.opacity, s.blur),
            None => (st.tool, st.layer, st.size, st.feather, st.opacity, st.blur),
        };
        let idx = |v: &[SpotMode]| v.iter().position(|m| *m == mode).unwrap_or(0);
        let mut v = vec![
            choice(
                sub::TOOL,
                t!("develop-retouch-tool"),
                MODES.iter().map(|m| mode_name(*m)).collect(),
                idx(&MODES),
            ),
            choice(
                sub::AREA,
                t!("develop-retouch-area"),
                vec![
                    t!("develop-retouch-area-spot"),
                    t!("develop-retouch-area-brush"),
                ],
                usize::from(st.brush),
            ),
            choice(
                sub::LAYER,
                t!("develop-retouch-layer"),
                LAYERS.iter().map(|l| layer_name(*l)).collect(),
                LAYERS.iter().position(|l| *l == layer).unwrap_or(0),
            ),
        ];
        let mut split = Slider::new(
            SliderSpec::new(t!("develop-retouch-split"), 0.5, 50.0, 4.0)
                .steps(0.1, 1.0)
                .unit(Unit::Plain, 1),
        );
        split.set(r.split_radius);
        v.push(Control {
            id: RT | sub::SPLIT,
            body: Body::Slider(split),
        });
        v.push(pct(
            sub::SIZE,
            t!("develop-retouch-size"),
            (0.001, 0.25),
            0.02,
            size,
            1,
        ));
        v.push(pct(
            sub::FEATHER,
            t!("develop-retouch-feather"),
            (0.0, 1.0),
            0.5,
            feather,
            0,
        ));
        v.push(pct(
            sub::OPACITY,
            t!("develop-retouch-opacity"),
            (0.0, 1.0),
            1.0,
            opacity,
            0,
        ));
        if mode == SpotMode::Blur {
            v.push(pct(
                sub::BLUR,
                t!("develop-retouch-blur-radius"),
                (0.0, 0.05),
                0.004,
                blur,
                2,
            ));
        }
        v.push(choice(
            sub::OUTLINES,
            t!("develop-retouch-outlines"),
            vec![t!("develop-mask-off"), t!("develop-mask-on")],
            usize::from(!st.hidden),
        ));
        if r.spots.is_empty() {
            v.push(Control {
                id: RT | sub::HINT,
                body: Body::Note(if mode == SpotMode::Remove {
                    t!("develop-retouch-remove-hint")
                } else {
                    t!("develop-retouch-hint")
                }),
            });
        }
        if let Some((_, s)) = sel
            && s.mode == SpotMode::Remove
        {
            let st = self.ai2_state.get(&self.remove_key(s));
            v.push(Control {
                id: RT | sub::AI_STATE,
                body: Body::Note(super::ai_remove::state_line(
                    st,
                    t!("develop-retouch-removing"),
                )),
            });
            let label = match st {
                Some(super::ai_remove::NetState::Computing) => Some(t!("develop-ai-net-stop")),
                Some(
                    super::ai_remove::NetState::Stopped | super::ai_remove::NetState::Failed(_),
                ) => Some(t!("develop-ai-net-retry")),
                Some(super::ai_remove::NetState::Missing(_)) => {
                    Some(t!("develop-ai-open-settings"))
                }
                _ => None,
            };
            if let Some(label) = label {
                v.push(Control {
                    id: RT | sub::AI_ACTION,
                    body: Body::Choice(Choice::button(label)),
                });
            }
        }
        for (i, s) in r.spots.iter().enumerate().take(LISTED) {
            let name = spot_name(i, s);
            let label = if Some(i) == st.selected {
                t!("develop-retouch-selected", name = name)
            } else {
                name
            };
            v.push(Control {
                id: RT | (sub::SPOT0 + i as u32),
                body: Body::Choice(Choice::button(label)),
            });
        }
        if sel.is_some() {
            v.push(Control {
                id: RT | sub::REMOVE,
                body: Body::Choice(Choice::button(t!("develop-retouch-remove"))),
            });
        }
        v
    }

    /// Puts the retouch controls on its card.
    pub(super) fn sync_retouch(&self, cards: &mut [Card]) {
        if let Some(c) = cards.iter_mut().find(|c| c.id == u32::from(KIND.0)) {
            c.controls = self.retouch_controls();
            if let (Some(on), Some(m)) = (c.enabled.as_mut(), self.edit.module(KIND, 0)) {
                *on = m.enabled;
            }
        }
    }

    fn refresh_retouch(&mut self) {
        let mut cards = std::mem::take(&mut self.panel.cards);
        self.sync_retouch(&mut cards);
        self.panel.cards = cards;
    }

    fn rt_value(&self, sub: u32) -> Option<f32> {
        match self.panel.control(RT | sub).map(|c| &c.body) {
            Some(Body::Slider(s)) => Some(s.value),
            Some(Body::Choice(c)) => Some(c.selected as f32),
            _ => None,
        }
    }

    /// A retouch control changed.
    pub(super) fn retouch_value(&mut self, control: u32, done: bool, now_ms: i64) -> Vec<Request> {
        let sub = control & !RT;
        let mut r = self.retouch();
        let sel = self.retouch_state.selected.filter(|&i| i < r.spots.len());
        let v = self.rt_value(sub).unwrap_or(0.0);
        let mut changed = false;
        let mut out = Vec::new();
        match sub {
            sub::TOOL => {
                let m = MODES[(v as usize).min(MODES.len() - 1)];
                self.retouch_state.tool = m;
                if let Some(i) = sel {
                    r.spots[i].mode = m;
                    r.spots[i].model = remove_model(m);
                    if m == SpotMode::Remove {
                        r.spots[i].layer = Layer::All;
                    }
                    changed = true;
                }
            }
            sub::AI_ACTION => {
                if let Some(i) = sel {
                    let key = self.remove_key(&r.spots[i]);
                    match self.ai2_state.get(&key) {
                        Some(super::ai_remove::NetState::Computing) => {
                            self.ai2_request = Some(super::ai_remove::Ask::Stop);
                        }
                        Some(super::ai_remove::NetState::Missing(_)) => {
                            return vec![Request::OpenLocalAi("ai.remove")];
                        }
                        Some(_) => {
                            self.ai2_request = Some(super::ai_remove::Ask::Retry(key));
                        }
                        None => {}
                    }
                }
            }
            sub::AREA => self.retouch_state.brush = v as usize == 1,
            sub::LAYER => {
                let l = LAYERS[(v as usize).min(2)];
                self.retouch_state.layer = l;
                if let Some(i) = sel {
                    r.spots[i].layer = l;
                    changed = true;
                }
            }
            sub::SPLIT => {
                r.split_radius = v;
                changed = true;
            }
            sub::SIZE | sub::FEATHER | sub::OPACITY | sub::BLUR => {
                let st = &mut self.retouch_state;
                let (field, spot): (&mut f32, Option<&mut f32>) = match sub {
                    sub::SIZE => (&mut st.size, sel.map(|i| &mut r.spots[i].size)),
                    sub::FEATHER => (&mut st.feather, sel.map(|i| &mut r.spots[i].feather)),
                    sub::OPACITY => (&mut st.opacity, sel.map(|i| &mut r.spots[i].opacity)),
                    _ => (&mut st.blur, sel.map(|i| &mut r.spots[i].blur)),
                };
                *field = v;
                if let Some(s) = spot {
                    *s = v;
                    changed = true;
                }
            }
            sub::OUTLINES => self.retouch_state.hidden = v as usize == 0,
            sub::REMOVE => {
                if let Some(i) = sel {
                    return self.retouch_ask_remove(i);
                }
            }
            s if s >= sub::SPOT0 => {
                let i = (s - sub::SPOT0) as usize;
                self.retouch_state.selected = if sel == Some(i) { None } else { Some(i) };
                if let Some(i) = self.retouch_state.selected
                    && let Some(sp) = r.spots.get(i)
                {
                    out.push(Request::Say(spot_name(i, sp)));
                }
            }
            _ => {}
        }
        if changed {
            self.rt_set(&r);
            out.push(Request::Render);
            if done {
                self.rt_commit(now_ms);
            }
        }
        // Rows and the blur slider come and go with the selection and the tool.
        let keep_focus = matches!(
            sub,
            sub::SIZE | sub::FEATHER | sub::OPACITY | sub::BLUR | sub::SPLIT
        );
        if !keep_focus || done {
            self.refresh_retouch();
        }
        out
    }

    /// `[` or `]` (the brush size keys) while spots are on the picture and no mask row is
    /// being edited: the selected spot's size (one history step) and the next spot's, by
    /// the brushes' steps. What to say; None when spots aren't on the picture.
    pub(super) fn retouch_size_key(&mut self, larger: bool, now_ms: i64) -> Option<String> {
        if self.masks.active.is_some() || !self.retouch_on() {
            return None;
        }
        let k = if larger { 1.25 } else { 0.8 };
        let step = |v: f32| (v * k).clamp(0.001, 0.25);
        let mut r = self.retouch();
        let size = match self.retouch_state.selected.filter(|&i| i < r.spots.len()) {
            Some(i) => {
                let v = step(r.spots[i].size);
                r.spots[i].size = v;
                self.rt_set(&r);
                self.rt_commit(now_ms);
                v
            }
            None => step(self.retouch_state.size),
        };
        self.retouch_state.size = size;
        self.refresh_retouch();
        Some(t!(
            "develop-retouch-size-now",
            percent = super::controls::percent_text(f64::from(size) * 100.0, 1)
        ))
    }

    /// Asks before removing spot `i`.
    pub(super) fn retouch_ask_remove(&mut self, i: usize) -> Vec<Request> {
        let r = self.retouch();
        let Some(s) = r.spots.get(i) else {
            return Vec::new();
        };
        self.retouch_state.confirm = Some(i);
        vec![Request::Confirm(
            t!("develop-retouch-remove-confirm", spot = spot_name(i, s)),
            t!("develop-retouch-remove-ok"),
        )]
    }

    /// The selected spot, asked about for removal (Delete on the picture).
    pub(super) fn retouch_delete(&mut self) -> Vec<Request> {
        match self.retouch_state.selected {
            Some(i) => self.retouch_ask_remove(i),
            None => Vec::new(),
        }
    }

    /// The user accepted a removal.
    pub(super) fn retouch_confirmed(&mut self, now_ms: i64) -> Vec<Request> {
        let Some(i) = self.retouch_state.confirm.take() else {
            return Vec::new();
        };
        let mut r = self.retouch();
        if i >= r.spots.len() {
            return Vec::new();
        }
        self.rt_commit(now_ms);
        r.spots.remove(i);
        self.rt_set(&r);
        self.rt_commit(now_ms);
        self.retouch_state.selected = None;
        self.refresh_retouch();
        vec![Request::Render, Request::Say(t!("develop-retouch-removed"))]
    }

    /// Outlines and handles on screen: every spot's outline (unless hidden), and the
    /// selected spot's centre, edge and source handles.
    pub(super) fn retouch_handles(&self, map: &Map) -> (Vec<Handle>, Vec<Outline>) {
        let r = self.retouch();
        let st = &self.retouch_state;
        let ppl = f64::from(map.px_per_long());
        let mut handles = Vec::new();
        let mut outlines = Vec::new();
        for (i, s) in r.spots.iter().enumerate() {
            let selected = Some(i) == st.selected;
            if st.hidden && !selected {
                continue;
            }
            let off = s.offset();
            let rad = (f64::from(s.size) * ppl) as f32;
            let at = |p: [f32; 2]| map.to_screen((f64::from(p[0]), f64::from(p[1])));
            let path: Vec<[f32; 2]> = if s.path.is_empty() {
                vec![s.target]
            } else {
                s.path.clone()
            };
            let shifted: Vec<[f32; 2]> = path
                .iter()
                .map(|p| [p[0] + off[0], p[1] + off[1]])
                .collect();
            for (pts, faint) in [(&path, false), (&shifted, true)] {
                if faint && !s.mode.has_source() {
                    continue;
                }
                let screen: Vec<(f32, f32)> = pts.iter().filter_map(|p| at(*p)).collect();
                outlines.extend(capsule(&screen, rad, faint || !selected));
            }
            if s.mode.has_source()
                && let (Some(a), Some(b)) = (at(s.target), at(s.source))
            {
                outlines.push(Outline {
                    points: vec![a, b],
                    closed: false,
                    faint: true,
                });
            }
            if selected {
                let name = spot_name(i, s);
                if let Some(c) = at(s.target) {
                    handles.push(Handle {
                        role: Role::Centre,
                        at: c,
                        name: t!("develop-retouch-handle-spot", spot = name.clone()),
                    });
                    handles.push(Handle {
                        role: Role::Size,
                        at: (c.0 + rad, c.1),
                        name: t!("develop-retouch-handle-size", spot = name.clone()),
                    });
                }
                if s.mode.has_source()
                    && let Some(c) = at(s.source)
                {
                    handles.push(Handle {
                        role: Role::Point,
                        at: c,
                        name: t!("develop-retouch-handle-source", spot = name),
                    });
                }
            }
        }
        (handles, outlines)
    }

    /// Which spot (and part) is under screen point `p`: the selected spot's handles first,
    /// then any spot's area, then its source.
    fn retouch_hit(&self, map: &Map, p: (f32, f32), handle: Option<usize>) -> Option<Drag> {
        let r = self.retouch();
        if let (Some(i), Some(h)) = (self.retouch_state.selected, handle) {
            let has_source = r.spots.get(i).is_some_and(|s| s.mode.has_source());
            return Some(match h {
                0 => Drag::Target(i),
                1 => Drag::Size(i),
                _ if has_source => Drag::Source(i),
                _ => Drag::Target(i),
            });
        }
        let (fx, fy) = map.to_frame(p);
        let f = [fx as f32, fy as f32];
        let aspect = (map.plan.frame[2] / map.plan.frame[3].max(1e-9)) as f32;
        let hidden = self.retouch_state.hidden;
        let visible = |i: usize| !hidden || Some(i) == self.retouch_state.selected;
        // Last drawn on top: search from the end.
        for (i, s) in r.spots.iter().enumerate().rev() {
            if visible(i) && s.covers(f, aspect) {
                return Some(Drag::Target(i));
            }
        }
        for (i, s) in r.spots.iter().enumerate().rev() {
            if visible(i) && s.covers_source(f, aspect) {
                return Some(Drag::Source(i));
            }
        }
        None
    }

    /// A press on the picture while retouching: grab a spot, place one, or start a brush
    /// area. `handle` is the retouch handle under the pointer.
    pub(super) fn retouch_press(
        &mut self,
        map: &Map,
        p: (f32, f32),
        handle: Option<usize>,
    ) -> bool {
        if let Some(d) = self.retouch_hit(map, p, handle) {
            let i = match d {
                Drag::Target(i) | Drag::Source(i) | Drag::Size(i) => i,
                Drag::Paint => return false,
            };
            self.retouch_state.selected = Some(i);
            self.retouch_state.drag = Some(d);
            self.refresh_retouch();
            return true;
        }
        let (fx, fy) = map.to_frame(p);
        if !(fx.is_finite() && fy.is_finite()) {
            return false;
        }
        let f = [fx as f32, fy as f32];
        if self.retouch_state.brush {
            self.retouch_state.path = vec![f];
            self.retouch_state.drag = Some(Drag::Paint);
            return true;
        }
        self.retouch_add(f, Vec::new(), map);
        // Placed: a drag goes on moving it.
        if let Some(i) = self.retouch_state.selected {
            self.retouch_state.drag = Some(Drag::Target(i));
        }
        true
    }

    /// Adds a spot at `target` (frame fractions) with a provisional source beside it, and
    /// asks for its source to be found.
    fn retouch_add(&mut self, target: [f32; 2], path: Vec<[f32; 2]>, map: &Map) {
        let st = self.retouch_state.clone();
        let mut r = self.retouch();
        if r.spots.len() >= specular_develop::ops::retouch::MAX_SPOTS {
            return;
        }
        // Beside it until the search answers: 2.5 sizes across, on the picture's side.
        let fw = map.plan.frame[2].max(1e-9);
        let long = map.plan.frame[2].max(map.plan.frame[3]);
        let dx = (2.5 * f64::from(st.size) * long / fw) as f32;
        let sx = if target[0] + dx < 0.98 {
            target[0] + dx
        } else {
            target[0] - dx
        };
        let spot = Spot {
            mode: st.tool,
            model: remove_model(st.tool),
            layer: if st.tool == SpotMode::Remove {
                Layer::All
            } else {
                st.layer
            },
            target,
            source: if st.tool.has_source() {
                [sx, target[1]]
            } else {
                target
            },
            size: st.size,
            feather: st.feather,
            opacity: st.opacity,
            blur: st.blur,
            path,
        };
        r.spots.push(spot);
        let i = r.spots.len() - 1;
        self.rt_set(&r);
        self.retouch_state.selected = Some(i);
        if st.tool.has_source() {
            self.retouch_state.search = Some(i);
        }
        self.refresh_retouch();
    }

    /// The pointer moved during a retouch drag; whether one is going on.
    pub(super) fn retouch_move(&mut self, map: &Map, p: (f32, f32)) -> bool {
        let Some(d) = self.retouch_state.drag else {
            return false;
        };
        let (fx, fy) = map.to_frame(p);
        if !(fx.is_finite() && fy.is_finite()) {
            return true;
        }
        let f = [fx as f32, fy as f32];
        let mut r = self.retouch();
        match d {
            Drag::Paint => {
                // Smoothed as mask brush strokes are (its Smoothing setting).
                let k = self.masks.brush.smoothing;
                let prev = self
                    .retouch_state
                    .path
                    .last()
                    .map(|l| (f64::from(l[0]), f64::from(l[1])));
                let (sx, sy) = super::masks::smooth(prev, (f64::from(f[0]), f64::from(f[1])), k);
                let f = [sx as f32, sy as f32];
                let path = &mut self.retouch_state.path;
                let gap = self.retouch_state.size * 0.1;
                let far = path
                    .last()
                    .is_none_or(|l| (l[0] - f[0]).hypot(l[1] - f[1]) > gap);
                if far {
                    path.push(f);
                }
                return true;
            }
            Drag::Target(i) => {
                let Some(s) = r.spots.get_mut(i) else {
                    return true;
                };
                let d = [f[0] - s.target[0], f[1] - s.target[1]];
                s.target = f;
                for q in &mut s.path {
                    q[0] += d[0];
                    q[1] += d[1];
                }
                if !s.mode.has_source() {
                    s.source = f;
                }
            }
            Drag::Source(i) => {
                let Some(s) = r.spots.get_mut(i) else {
                    return true;
                };
                s.source = f;
                if self.retouch_state.search == Some(i) {
                    self.retouch_state.search = None;
                }
            }
            Drag::Size(i) => {
                let Some(s) = r.spots.get_mut(i) else {
                    return true;
                };
                let Some(c) = map.to_screen((f64::from(s.target[0]), f64::from(s.target[1])))
                else {
                    return true;
                };
                let px = (p.0 - c.0).hypot(p.1 - c.1);
                s.size = (px / map.px_per_long().max(1e-6)).clamp(0.001, 0.25);
            }
        }
        self.rt_set(&r);
        true
    }

    /// The button went up: a brush area becomes a spot; the change becomes one step (a new
    /// spot's step waits for its source).
    pub(super) fn retouch_release(&mut self, map: &Map, now_ms: i64) -> bool {
        let Some(d) = self.retouch_state.drag.take() else {
            return false;
        };
        if d == Drag::Paint {
            let path = std::mem::take(&mut self.retouch_state.path);
            if let Some(&first) = path.first() {
                // The area's reference point: the middle of its path's box.
                let (mut lo, mut hi) = (first, first);
                for p in &path {
                    lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                    hi = [hi[0].max(p[0]), hi[1].max(p[1])];
                }
                let c = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
                self.retouch_add(c, path, map);
            }
        }
        if self.retouch_state.search.is_none() {
            self.rt_commit(now_ms);
        }
        self.refresh_retouch();
        true
    }

    /// The source found for spot `i` (frame fractions): set, and the spot's step ends.
    pub(super) fn retouch_found(
        &mut self,
        i: usize,
        source: Option<[f32; 2]>,
        now_ms: i64,
    ) -> bool {
        if self.retouch_state.search != Some(i) {
            return false;
        }
        self.retouch_state.search = None;
        let mut r = self.retouch();
        if let (Some(src), Some(s)) = (source, r.spots.get_mut(i)) {
            s.source = src;
            self.rt_set(&r);
        }
        if self.retouch_state.drag.is_none() {
            self.rt_commit(now_ms);
        }
        true
    }

    /// The crosshair key: on at screen point `at` (kept on the picture), or off. What to
    /// say; None when spots aren't on the picture.
    pub(super) fn retouch_cross_toggle(&mut self, map: &Map, at: (f32, f32)) -> Option<String> {
        if !self.retouch_on() {
            return None;
        }
        if self.retouch_state.cross.take().is_some() {
            return Some(t!("develop-retouch-cross-off"));
        }
        self.retouch_state.cross = Some(cross_frame(map, at));
        Some(t!(
            "develop-retouch-cross-on",
            tool = mode_name(self.retouch_state.tool)
        ))
    }

    /// The crosshair on screen (logical px), while it is on and spots are on the picture.
    pub(super) fn retouch_cross_screen(&self, map: &Map) -> Option<(f32, f32)> {
        let c = self.retouch_state.cross.filter(|_| self.retouch_on())?;
        map.to_screen((f64::from(c[0]), f64::from(c[1])))
    }

    /// Moves the crosshair by a screen delta (logical px), kept on the picture. Where it is
    /// now, to say.
    pub(super) fn retouch_cross_move(&mut self, map: &Map, dx: f32, dy: f32) -> Option<String> {
        let p = self.retouch_cross_screen(map)?;
        let to = (p.0 + dx, p.1 + dy);
        self.retouch_state.cross = Some(cross_frame(map, to));
        let q = self.retouch_cross_screen(map)?;
        let pct = |v: f32, o: f32, n: f32| ((v - o) / n.max(1.0) * 100.0).round().clamp(0.0, 100.0);
        Some(t!(
            "develop-retouch-cross-at",
            x = pct(q.0, map.shown.x, map.shown.w) as i64,
            y = pct(q.1, map.shown.y, map.shown.h) as i64
        ))
    }

    /// Enter: a spot of the chosen tool where the crosshair is, as a click places one (its
    /// source then searched for; the spot is one history step once the source is found, at
    /// once for tools without one). What to say.
    pub(super) fn retouch_cross_place(&mut self, map: &Map, now_ms: i64) -> Option<String> {
        let c = self.retouch_state.cross.filter(|_| self.retouch_on())?;
        let before = self.retouch().spots.len();
        self.retouch_add(c, Vec::new(), map);
        let r = self.retouch();
        if r.spots.len() == before {
            return Some(t!("develop-retouch-cross-full"));
        }
        if self.retouch_state.search.is_none() {
            self.rt_commit(now_ms);
        }
        let i = r.spots.len() - 1;
        Some(t!(
            "develop-retouch-cross-placed",
            spot = spot_name(i, &r.spots[i])
        ))
    }

    /// Esc while the crosshair is on: it goes; whether it was on.
    pub(super) fn retouch_cross_cancel(&mut self) -> bool {
        self.retouch_state.cross.take().is_some()
    }

    /// Esc on the picture: the selection goes; whether there was one.
    pub(super) fn retouch_deselect(&mut self) -> bool {
        if self.retouch_state.selected.take().is_some() {
            self.refresh_retouch();
            return true;
        }
        false
    }

    /// Shows or hides the outlines; whether they now show.
    pub(super) fn retouch_toggle_outlines(&mut self) -> bool {
        self.retouch_state.hidden = !self.retouch_state.hidden;
        self.refresh_retouch();
        !self.retouch_state.hidden
    }

    /// After the edit changed under the panel (undo, redo, a history jump): a selection or
    /// search past the spots goes.
    pub(super) fn retouch_after_history(&mut self) {
        let n = self.retouch().spots.len();
        let st = &mut self.retouch_state;
        if st.selected.is_some_and(|i| i >= n) {
            st.selected = None;
        }
        if st.search.is_some_and(|i| i >= n) {
            st.search = None;
        }
    }
}

/// The frame point (fractions) under screen point `p`, `p` kept on the picture.
fn cross_frame(map: &Map, p: (f32, f32)) -> [f32; 2] {
    let r = map.shown;
    let p = (
        p.0.clamp(r.x, r.x + (r.w - 0.01).max(0.0)),
        p.1.clamp(r.y, r.y + (r.h - 0.01).max(0.0)),
    );
    let (fx, fy) = map.to_frame(p);
    let ok = |v: f64| {
        if v.is_finite() {
            v.clamp(0.0, 1.0) as f32
        } else {
            0.5
        }
    };
    [ok(fx), ok(fy)]
}

/// One crosshair step on screen (logical px): a photo pixel when zoomed in, a screen pixel
/// when not (`k` physical px per logical px), as the viewer's crosshair steps; ten with
/// Shift.
pub(super) fn cross_step(map: &Map, k: f32, shift: bool) -> f32 {
    let c = map.plan.crop_rect();
    let photo_px = (f64::from(map.shown.w) / c[2].max(1e-9)) as f32;
    let one = photo_px.max(1.0 / k.max(0.1));
    if shift { one * 10.0 } else { one }
}

/// The outline of a round spot (`pts` one point) or a brushed area (circles at its ends,
/// sides along each segment), `r` screen px.
fn capsule(pts: &[(f32, f32)], r: f32, faint: bool) -> Vec<Outline> {
    let circle = |c: (f32, f32)| Outline {
        points: (0..32)
            .map(|k| {
                let a = std::f32::consts::TAU * k as f32 / 32.0;
                (c.0 + r * a.cos(), c.1 + r * a.sin())
            })
            .collect(),
        closed: true,
        faint,
    };
    let mut out = Vec::new();
    match pts {
        [] => {}
        [c] => out.push(circle(*c)),
        _ => {
            out.push(circle(pts[0]));
            out.push(circle(pts[pts.len() - 1]));
            let mut left = Vec::new();
            let mut right = Vec::new();
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let l = dx.hypot(dy).max(1e-6);
                let (nx, ny) = (-dy / l * r, dx / l * r);
                left.extend([(a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny)]);
                right.extend([(a.0 - nx, a.1 - ny), (b.0 - nx, b.1 - ny)]);
            }
            out.push(Outline {
                points: left,
                closed: false,
                faint,
            });
            out.push(Outline {
                points: right,
                closed: false,
                faint,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::model::{Edit, History};
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::preview::Orient;
    use specular_gfx::Rect;

    fn session() -> Session {
        let path = std::path::PathBuf::from("/photos/a.jpg");
        let edit = super::super::controls::new_edit(&path);
        let mut s = Session::new(path, edit, History::default());
        if let Some(c) = s.panel.card_mut(u32::from(KIND.0)) {
            c.expanded = true;
        }
        s
    }

    fn map(e: &Edit) -> Map {
        let ctx = Context::image(600, 400, Orient::default());
        Map {
            plan: Plan::new(e, &ctx, true),
            shown: Rect::new(0.0, 0.0, 600.0, 400.0),
        }
    }

    #[test]
    fn click_places_drag_moves_and_delete_asks() {
        let mut s = session();
        assert!(s.retouch_on());
        let m = map(&s.edit);
        assert!(s.retouch_press(&m, (150.0, 200.0), None));
        assert!(s.retouch_release(&m, 1));
        // Waiting for its source: no step yet; the source arrives and the step ends.
        assert_eq!(s.retouch_state.search, Some(0));
        assert!(s.retouch_found(0, Some([0.6, 0.5]), 2_000));
        let r = s.retouch();
        assert_eq!(r.spots.len(), 1);
        assert!((r.spots[0].target[0] - 0.25).abs() < 1e-3);
        assert_eq!(r.spots[0].source, [0.6, 0.5]);
        let steps = s.history.position;
        // Drag the source: one more step.
        let (h, _) = s.retouch_handles(&m);
        assert_eq!(h.len(), 3);
        assert!(s.retouch_press(&m, h[2].at, Some(2)));
        assert!(s.retouch_move(&m, (420.0, 100.0)));
        assert!(s.retouch_release(&m, 4_000));
        assert_eq!(s.history.position, steps + 1);
        assert!((s.retouch().spots[0].source[0] - 0.7).abs() < 1e-3);
        // The list names it; Delete asks first; yes removes it; undo brings it back.
        let rows: Vec<_> = s
            .panel
            .cards
            .iter()
            .find(|c| c.id == u32::from(KIND.0))
            .unwrap()
            .controls
            .iter()
            .filter(|c| c.id >= RT | sub::SPOT0)
            .collect();
        assert_eq!(rows.len(), 1);
        let ask = s.retouch_delete();
        assert!(matches!(ask.first(), Some(Request::Confirm(..))));
        assert_eq!(s.retouch().spots.len(), 1);
        s.retouch_confirmed(6_000);
        assert!(s.retouch().spots.is_empty());
        s.undo();
        assert_eq!(s.retouch().spots.len(), 1);
    }

    /// `[` `]` size the selected spot (one step each) and else the next spot, as
    /// they size brushes; they are the brush's while a mask row is edited.
    #[test]
    fn bracket_keys_size_spots() {
        let mut s = session();
        let m = map(&s.edit);
        let next = s.retouch_state.size;
        // No spot selected: the next spot's size.
        let said = s.retouch_size_key(true, 1).expect("spots on the picture");
        assert!((s.retouch_state.size - next * 1.25).abs() < 1e-6);
        assert!(said.contains("2.5"), "{said}");
        assert!(s.retouch_press(&m, (150.0, 200.0), None));
        assert!(s.retouch_release(&m, 2));
        assert!(s.retouch_found(0, Some([0.6, 0.5]), 3));
        s.retouch_state.selected = Some(0);
        let size = s.retouch().spots[0].size;
        let steps = s.history.position;
        s.retouch_size_key(false, 4_000).unwrap();
        assert!((s.retouch().spots[0].size - size * 0.8).abs() < 1e-6);
        assert_eq!(s.history.position, steps + 1);
        // Bounded.
        for i in 0..40 {
            s.retouch_size_key(false, 5_000 + i);
        }
        assert!((s.retouch().spots[0].size - 0.001).abs() < 1e-6);
        s.undo();
        assert!(s.retouch().spots[0].size > 0.001);
        // Card closed: not the spots' keys.
        if let Some(c) = s.panel.card_mut(u32::from(KIND.0)) {
            c.expanded = false;
        }
        assert!(s.retouch_size_key(true, 9_000).is_none());
    }

    /// The keyboard crosshair places spots: on where asked, arrows move it a photo pixel
    /// (Shift ten, kept on the picture), Enter places a spot of the chosen tool there (one
    /// history step, after its source for heal, at once for blur), Esc puts it away; each
    /// step has something to say. Off while the card is closed.
    #[test]
    fn keyboard_crosshair_places_spots() {
        let mut s = session();
        let m = map(&s.edit);
        // Without the direction isolates around the names and numbers.
        let plain = |s: String| -> String {
            s.chars()
                .filter(|c| !('\u{2066}'..='\u{2069}').contains(c))
                .collect()
        };
        // 600 × 400 photo shown at 600 × 400 logical px: a step is a photo pixel; at 2×
        // zoom it would be two.
        assert_eq!(cross_step(&m, 1.0, false), 1.0);
        assert_eq!(cross_step(&m, 1.0, true), 10.0);
        let said = plain(s.retouch_cross_toggle(&m, (300.0, 200.0)).expect("on"));
        assert!(said.contains("Heal"), "{said}");
        let c = s.retouch_cross_screen(&m).unwrap();
        assert!(
            (c.0 - 300.0).abs() < 0.01 && (c.1 - 200.0).abs() < 0.01,
            "{c:?}"
        );
        // Right 10 (Shift), right 1, down 1: 311, 201.
        let step = cross_step(&m, 1.0, true);
        s.retouch_cross_move(&m, step, 0.0).unwrap();
        s.retouch_cross_move(&m, cross_step(&m, 1.0, false), 0.0)
            .unwrap();
        let said = plain(s.retouch_cross_move(&m, 0.0, 1.0).unwrap());
        assert!(said.contains("52") && said.contains("50"), "{said}");
        let c = s.retouch_cross_screen(&m).unwrap();
        assert!(
            (c.0 - 311.0).abs() < 0.01 && (c.1 - 201.0).abs() < 0.01,
            "{c:?}"
        );
        // Kept on the picture.
        s.retouch_cross_move(&m, -5_000.0, 0.0);
        assert!(s.retouch_cross_screen(&m).unwrap().0.abs() < 0.01);
        s.retouch_cross_move(&m, 311.0, 0.0);

        // Enter: a heal spot there, waiting for its source; the source ends one step.
        let steps = s.history.position;
        let said = plain(s.retouch_cross_place(&m, 1_000).unwrap());
        assert!(said.contains("1. Heal"), "{said}");
        let r = s.retouch();
        assert_eq!(r.spots.len(), 1);
        assert_eq!(r.spots[0].mode, SpotMode::Heal);
        assert!((r.spots[0].target[0] - 311.0 / 600.0).abs() < 1e-3);
        assert!((r.spots[0].target[1] - 201.0 / 400.0).abs() < 1e-3);
        assert_eq!(s.retouch_state.search, Some(0));
        assert_eq!(s.history.position, steps);
        assert!(s.retouch_found(0, Some([0.8, 0.5]), 2_000));
        assert_eq!(s.history.position, steps + 1);
        // The crosshair stays for the next spot; blur has no source: a step at once.
        s.retouch_state.tool = SpotMode::Blur;
        s.retouch_cross_move(&m, -100.0, 0.0);
        let said = plain(s.retouch_cross_place(&m, 20_000).unwrap());
        assert!(said.contains("2. Blur"), "{said}");
        assert_eq!(s.retouch_state.search, None);
        assert_eq!(s.history.position, steps + 2);
        assert_eq!(s.retouch().spots[1].mode, SpotMode::Blur);
        // One undo takes back one spot.
        s.undo();
        assert_eq!(s.retouch().spots.len(), 1);
        // Esc: away, nothing placed.
        assert!(s.retouch_cross_cancel());
        assert!(s.retouch_cross_screen(&m).is_none());
        assert!(s.retouch_cross_place(&m, 4_000).is_none());
        assert_eq!(s.retouch().spots.len(), 1);
        // The key again: on, and again: off.
        assert!(s.retouch_cross_toggle(&m, (10.0, 10.0)).is_some());
        assert_eq!(
            s.retouch_cross_toggle(&m, (10.0, 10.0)).as_deref(),
            Some(t!("develop-retouch-cross-off").as_str())
        );
        // Card closed: not on the picture, nothing to say (the caller says why).
        if let Some(c) = s.panel.card_mut(u32::from(KIND.0)) {
            c.expanded = false;
        }
        assert!(s.retouch_cross_toggle(&m, (10.0, 10.0)).is_none());
    }

    #[test]
    fn brush_paints_an_area_and_values_change_the_selected_spot() {
        let mut s = session();
        let m = map(&s.edit);
        s.retouch_state.brush = true;
        s.retouch_state.tool = SpotMode::Blur;
        assert!(s.retouch_press(&m, (100.0, 100.0), None));
        for k in 1..20 {
            s.retouch_move(&m, (100.0 + 10.0 * k as f32, 100.0 + 3.0 * k as f32));
        }
        assert!(s.retouch_release(&m, 5));
        let r = s.retouch();
        assert_eq!(r.spots.len(), 1);
        assert_eq!(r.spots[0].mode, SpotMode::Blur);
        assert!(r.spots[0].path.len() > 5);
        // Blur needs no source: the step is done.
        assert!(s.retouch_state.search.is_none());
        let n = s.history.position;
        // The opacity slider changes the selected spot.
        if let Some(Body::Slider(sl)) = s
            .panel
            .cards
            .iter_mut()
            .flat_map(|c| c.controls.iter_mut())
            .find(|c| c.id == RT | sub::OPACITY)
            .map(|c| &mut c.body)
        {
            sl.set(0.4);
        }
        s.retouch_value(RT | sub::OPACITY, true, 9_000);
        assert!((s.retouch().spots[0].opacity - 0.4).abs() < 1e-6);
        assert_eq!(s.history.position, n + 1);
        // Outlines hide and show.
        assert!(!s.retouch_toggle_outlines());
        let (_, ol) = s.retouch_handles(&m);
        assert!(!ol.is_empty(), "the selected spot still shows");
        s.retouch_deselect();
        let (_, ol) = s.retouch_handles(&m);
        assert!(ol.is_empty());
    }
}
