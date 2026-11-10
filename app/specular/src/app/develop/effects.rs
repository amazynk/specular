//! The Effects cards: negative inversion, dehaze, graduated ND, vignette, glow,
//! grain, and borders with a watermark; the graduated ND's handles on the picture (the masks'
//! linear-gradient handles, in the cropped frame); the film base pick; the frame preview.
//!
//! One card per module, each with a switch and a Masks section. Borders and the watermark
//! only reach exports and prints; "Show on picture" draws them over the picture without
//! changing the edit.

use std::path::Path;

use specular_develop::model::{Fields, ModuleKind, Step, Value};
use specular_develop::ops::effects::frame::Frame;
use specular_develop::ops::effects::graduated::Graduated;
use specular_develop::ops::effects::grain;
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control};
use specular_widgets::mask_handles::{Handle, Outline, Role};
use specular_widgets::slider::{Slider, SliderSpec, Unit};

use super::controls::{Request, Session, id};
use super::masks::Map;

/// The effect modules, in the order their cards show.
pub(super) const EFFECTS: [ModuleKind; 7] = [
    ModuleKind::NEGATIVE,
    ModuleKind::DEHAZE,
    ModuleKind::GRADUATED_ND,
    ModuleKind::VIGNETTE,
    ModuleKind::GLOW,
    ModuleKind::GRAIN,
    ModuleKind::FRAME,
];

/// The canvas aspects the border can fit, width / height (0: the photo's own).
pub(super) const ASPECTS: [f32; 5] = [0.0, 1.0, 0.8, 1.5, 16.0 / 9.0];

/// The border colours offered (sRGB).
pub(super) const COLOURS: [[f32; 3]; 3] = [[1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [0.5, 0.5, 0.5]];

/// A module's title, for the effect modules.
pub(super) fn title(kind: ModuleKind) -> Option<String> {
    Some(match kind {
        ModuleKind::NEGATIVE => t!("develop-module-negative"),
        ModuleKind::DEHAZE => t!("develop-module-dehaze"),
        ModuleKind::GRADUATED_ND => t!("develop-module-graduated"),
        ModuleKind::VIGNETTE => t!("develop-module-vignette"),
        ModuleKind::GLOW => t!("develop-module-glow"),
        ModuleKind::GRAIN => t!("develop-module-grain"),
        ModuleKind::FRAME => t!("develop-module-frame"),
        k => return super::depth_tools::title(k),
    })
}

/// The float field a slider sets.
pub(super) fn float_field(control: u32) -> Option<(ModuleKind, &'static str)> {
    use ModuleKind as K;
    Some(match control {
        id::NEG_DMAX => (K::NEGATIVE, "d_max"),
        id::NEG_EXPOSURE => (K::NEGATIVE, "exposure"),
        id::NEG_GRADE => (K::NEGATIVE, "grade"),
        id::NEG_GLOSS => (K::NEGATIVE, "gloss"),
        id::DEHAZE => (K::DEHAZE, "strength"),
        id::ND_AMOUNT => (K::GRADUATED_ND, "amount"),
        id::ND_ANGLE => (K::GRADUATED_ND, "angle"),
        id::ND_SOFTNESS => (K::GRADUATED_ND, "softness"),
        id::ND_TINT_HUE => (K::GRADUATED_ND, "tint_hue"),
        id::ND_TINT => (K::GRADUATED_ND, "tint_amount"),
        id::VG_AMOUNT => (K::VIGNETTE, "amount"),
        id::VG_MIDPOINT => (K::VIGNETTE, "midpoint"),
        id::VG_ROUNDNESS => (K::VIGNETTE, "roundness"),
        id::VG_SHAPE => (K::VIGNETTE, "shape"),
        id::VG_FEATHER => (K::VIGNETTE, "feather"),
        id::VG_PROTECT => (K::VIGNETTE, "protect"),
        id::GLOW_AMOUNT => (K::GLOW, "amount"),
        id::GLOW_RADIUS => (K::GLOW, "radius"),
        id::GLOW_THRESHOLD => (K::GLOW, "threshold"),
        id::GRAIN_AMOUNT => (K::GRAIN, "amount"),
        id::GRAIN_SIZE => (K::GRAIN, "size"),
        id::GRAIN_BIAS => (K::GRAIN, "bias"),
        id::FRAME_BORDER => (K::FRAME, "border"),
        id::FRAME_SCALE => (K::FRAME, "scale"),
        id::FRAME_OPACITY => (K::FRAME, "opacity"),
        c => return super::depth_tools::float_field(c),
    })
}

/// The module of an effect control that is not a plain float.
pub(super) fn module_of(control: u32) -> Option<ModuleKind> {
    use ModuleKind as K;
    Some(match control {
        id::NEG_PICK | id::NEG_FILM => K::NEGATIVE,
        id::ND_EDIT => K::GRADUATED_ND,
        id::VG_CX | id::VG_CY => K::VIGNETTE,
        id::GLOW_MODE => K::GLOW,
        id::GRAIN_COLOUR => K::GRAIN,
        id::FRAME_COLOUR
        | id::FRAME_ASPECT
        | id::FRAME_MARK
        | id::FRAME_TEXT
        | id::FRAME_SVG
        | id::FRAME_PLACE
        | id::FRAME_SHOW => K::FRAME,
        c if super::depth_tools::is_ours(c) => return super::depth_tools::module_of(c),
        c if super::perspective::is_ours(c) => return super::perspective::module_of(c),
        c => return float_field(c).map(|f| f.0),
    })
}

/// A field's name in a history row.
pub(super) fn field_label(kind: ModuleKind, field: &str) -> Option<String> {
    use ModuleKind as K;
    Some(match (kind, field) {
        (K::NEGATIVE, "base") => t!("develop-negative-base"),
        (K::NEGATIVE, "d_max") => t!("develop-negative-range"),
        (K::NEGATIVE, "exposure") => t!("develop-negative-exposure"),
        (K::NEGATIVE, "grade") => t!("develop-negative-grade"),
        (K::NEGATIVE, "gloss") => t!("develop-negative-gloss"),
        (K::NEGATIVE, "black_white") => t!("develop-negative-film"),
        (K::DEHAZE, "strength") => t!("develop-dehaze"),
        (K::GRADUATED_ND, "amount") => t!("develop-graduated-amount"),
        (K::GRADUATED_ND, "angle" | "centre") => t!("develop-graduated-angle"),
        (K::GRADUATED_ND, "softness") => t!("develop-graduated-softness"),
        (K::GRADUATED_ND, "tint_hue") => t!("develop-graduated-tint-hue"),
        (K::GRADUATED_ND, "tint_amount") => t!("develop-graduated-tint"),
        (K::VIGNETTE, "amount") => t!("develop-vignette-amount"),
        (K::VIGNETTE, "midpoint") => t!("develop-vignette-midpoint"),
        (K::VIGNETTE, "roundness") => t!("develop-vignette-roundness"),
        (K::VIGNETTE, "shape") => t!("develop-vignette-shape"),
        (K::VIGNETTE, "feather") => t!("develop-vignette-feather"),
        (K::VIGNETTE, "protect") => t!("develop-vignette-protect"),
        (K::VIGNETTE, "centre") => t!("develop-vignette-centre"),
        (K::GLOW, "amount") => t!("develop-glow-amount"),
        (K::GLOW, "radius") => t!("develop-glow-radius"),
        (K::GLOW, "threshold") => t!("develop-glow-threshold"),
        (K::GLOW, "mode") => t!("develop-glow-mode"),
        (K::GRAIN, "amount") => t!("develop-grain-amount"),
        (K::GRAIN, "size") => t!("develop-grain-size"),
        (K::GRAIN, "bias") => t!("develop-grain-bias"),
        (K::GRAIN, "colour") => t!("develop-grain-colour"),
        (K::FRAME, "border" | "border_px") => t!("develop-frame-border"),
        (K::FRAME, "colour") => t!("develop-frame-colour"),
        (K::FRAME, "aspect") => t!("develop-frame-aspect"),
        (K::FRAME, "watermark") => t!("develop-frame-watermark"),
        (K::FRAME, "text" | "svg") => t!("develop-frame-watermark"),
        (K::FRAME, "anchor") => t!("develop-frame-place"),
        (K::FRAME, "scale") => t!("develop-frame-scale"),
        (K::FRAME, "opacity") => t!("develop-frame-opacity"),
        (k, f) => return super::depth_tools::field_label(k, f),
    })
}

fn slider(id: u32, spec: SliderSpec) -> Control {
    Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    }
}

fn pct(id: u32, label: String, min: f32, max: f32, default: f32) -> Control {
    let mut spec = SliderSpec::new(label, min, max, default)
        .steps(0.01, 0.1)
        .unit(Unit::Percent, 0);
    if min < 0.0 {
        spec = spec.signed();
    }
    slider(id, spec)
}

fn ev(id: u32, label: String, limit: f32) -> Control {
    slider(
        id,
        SliderSpec::new(label, -limit, limit, 0.0)
            .steps(0.05, 0.5)
            .unit(Unit::Ev, 2)
            .signed(),
    )
}

fn choice(id: u32, label: String, options: Vec<String>) -> Control {
    Control {
        id,
        body: Body::Choice(Choice::new(label, options)),
    }
}

fn button(id: u32, label: String) -> Control {
    Control {
        id,
        body: Body::Choice(Choice::button(label)),
    }
}

fn card(kind: ModuleKind, on: bool, controls: Vec<Control>) -> Card {
    Card {
        id: u32::from(kind.0),
        title: title(kind).unwrap_or_default(),
        enabled: Some(on),
        expanded: false,
        controls,
    }
}

/// The anchors' names, in [`Frame::anchor`] order.
fn places() -> Vec<String> {
    vec![
        t!("develop-frame-place-top-left"),
        t!("develop-frame-place-top"),
        t!("develop-frame-place-top-right"),
        t!("develop-frame-place-left"),
        t!("develop-frame-place-centre"),
        t!("develop-frame-place-right"),
        t!("develop-frame-place-bottom-left"),
        t!("develop-frame-place-bottom"),
        t!("develop-frame-place-bottom-right"),
    ]
}

/// The Effects cards, closed.
pub(super) fn cards() -> Vec<Card> {
    let off_on = || vec![t!("develop-off"), t!("develop-on")];
    vec![
        card(
            ModuleKind::NEGATIVE,
            false,
            vec![
                button(id::NEG_PICK, t!("develop-negative-pick")),
                choice(
                    id::NEG_FILM,
                    t!("develop-negative-film"),
                    vec![t!("develop-negative-colour"), t!("develop-negative-bw")],
                ),
                slider(
                    id::NEG_DMAX,
                    SliderSpec::new(t!("develop-negative-range"), 0.5, 4.0, 1.5)
                        .steps(0.01, 0.1)
                        .unit(Unit::Plain, 2),
                ),
                ev(id::NEG_EXPOSURE, t!("develop-negative-exposure"), 4.0),
                slider(
                    id::NEG_GRADE,
                    SliderSpec::new(t!("develop-negative-grade"), 0.0, 5.0, 2.0)
                        .steps(0.1, 0.5)
                        .unit(Unit::Plain, 1),
                ),
                pct(id::NEG_GLOSS, t!("develop-negative-gloss"), 0.0, 1.0, 0.5),
            ],
        ),
        card(
            ModuleKind::DEHAZE,
            true,
            vec![pct(id::DEHAZE, t!("develop-dehaze"), -1.0, 1.0, 0.0)],
        ),
        card(
            ModuleKind::GRADUATED_ND,
            true,
            vec![
                ev(id::ND_AMOUNT, t!("develop-graduated-amount"), 4.0),
                slider(
                    id::ND_ANGLE,
                    SliderSpec::new(t!("develop-graduated-angle"), -180.0, 180.0, 0.0)
                        .steps(1.0, 10.0)
                        .unit(Unit::Degrees, 0)
                        .signed(),
                ),
                pct(
                    id::ND_SOFTNESS,
                    t!("develop-graduated-softness"),
                    0.0,
                    2.0,
                    0.4,
                ),
                slider(
                    id::ND_TINT_HUE,
                    SliderSpec::new(t!("develop-graduated-tint-hue"), 0.0, 360.0, 250.0)
                        .steps(1.0, 10.0)
                        .unit(Unit::Degrees, 0),
                ),
                pct(id::ND_TINT, t!("develop-graduated-tint"), 0.0, 1.0, 0.0),
                button(id::ND_EDIT, t!("develop-graduated-edit")),
            ],
        ),
        card(
            ModuleKind::VIGNETTE,
            true,
            vec![
                ev(id::VG_AMOUNT, t!("develop-vignette-amount"), 4.0),
                pct(
                    id::VG_MIDPOINT,
                    t!("develop-vignette-midpoint"),
                    0.0,
                    1.0,
                    0.5,
                ),
                pct(
                    id::VG_FEATHER,
                    t!("develop-vignette-feather"),
                    0.0,
                    1.0,
                    0.5,
                ),
                pct(
                    id::VG_ROUNDNESS,
                    t!("develop-vignette-roundness"),
                    0.0,
                    1.0,
                    0.0,
                ),
                pct(id::VG_SHAPE, t!("develop-vignette-shape"), -1.0, 1.0, 0.0),
                pct(
                    id::VG_PROTECT,
                    t!("develop-vignette-protect"),
                    0.0,
                    1.0,
                    0.0,
                ),
                pct(id::VG_CX, t!("develop-vignette-centre-x"), -0.5, 0.5, 0.0),
                pct(id::VG_CY, t!("develop-vignette-centre-y"), -0.5, 0.5, 0.0),
            ],
        ),
        card(
            ModuleKind::GLOW,
            true,
            vec![
                pct(id::GLOW_AMOUNT, t!("develop-glow-amount"), 0.0, 1.0, 0.0),
                slider(
                    id::GLOW_RADIUS,
                    SliderSpec::new(t!("develop-glow-radius"), 0.002, 0.1, 0.02)
                        .steps(0.001, 0.01)
                        .unit(Unit::Percent, 1),
                ),
                slider(
                    id::GLOW_THRESHOLD,
                    SliderSpec::new(t!("develop-glow-threshold"), -4.0, 6.0, 2.0)
                        .steps(0.1, 0.5)
                        .unit(Unit::Ev, 1)
                        .signed(),
                ),
                choice(
                    id::GLOW_MODE,
                    t!("develop-glow-mode"),
                    vec![t!("develop-glow-bloom"), t!("develop-glow-soften")],
                ),
            ],
        ),
        card(
            ModuleKind::GRAIN,
            true,
            vec![
                pct(id::GRAIN_AMOUNT, t!("develop-grain-amount"), 0.0, 1.0, 0.0),
                pct(id::GRAIN_SIZE, t!("develop-grain-size"), 0.0, 1.0, 0.25),
                pct(id::GRAIN_BIAS, t!("develop-grain-bias"), -1.0, 1.0, 0.0),
                choice(
                    id::GRAIN_COLOUR,
                    t!("develop-grain-colour"),
                    vec![t!("develop-grain-mono"), t!("develop-grain-colour-on")],
                ),
            ],
        ),
        card(
            ModuleKind::FRAME,
            true,
            vec![
                choice(id::FRAME_SHOW, t!("develop-frame-show"), off_on()),
                slider(
                    id::FRAME_BORDER,
                    SliderSpec::new(t!("develop-frame-border"), 0.0, 0.2, 0.0)
                        .steps(0.001, 0.01)
                        .unit(Unit::Percent, 1),
                ),
                choice(
                    id::FRAME_COLOUR,
                    t!("develop-frame-colour"),
                    vec![
                        t!("develop-frame-white"),
                        t!("develop-frame-black"),
                        t!("develop-frame-grey"),
                    ],
                ),
                choice(
                    id::FRAME_ASPECT,
                    t!("develop-frame-aspect"),
                    vec![
                        t!("develop-frame-aspect-photo"),
                        "1:1".into(),
                        "4:5".into(),
                        "3:2".into(),
                        "16:9".into(),
                    ],
                ),
                choice(id::FRAME_MARK, t!("develop-frame-watermark"), off_on()),
                button(id::FRAME_TEXT, t!("develop-frame-text")),
                button(id::FRAME_SVG, t!("develop-frame-svg")),
                choice(id::FRAME_PLACE, t!("develop-frame-place"), places()),
                pct(id::FRAME_SCALE, t!("develop-frame-scale"), 0.02, 1.0, 0.25),
                pct(
                    id::FRAME_OPACITY,
                    t!("develop-frame-opacity"),
                    0.0,
                    1.0,
                    0.6,
                ),
            ],
        ),
    ]
}

/// Controls that are a button or a way of looking, not a value of the edit.
pub(super) fn is_event(control: u32) -> bool {
    matches!(
        control,
        id::NEG_PICK | id::ND_EDIT | id::FRAME_TEXT | id::FRAME_SVG | id::FRAME_SHOW
    ) || super::depth_tools::is_event(control)
}

/// The nearest entry of `list` to `v`.
fn nearest(list: &[f32], v: f32) -> usize {
    list.iter()
        .enumerate()
        .min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs()))
        .map_or(0, |e| e.0)
}

/// Puts a value the edit holds on an effect control that is not a plain float (true when
/// it was one).
pub(super) fn read(s: &Session, f: &Fields, ctl: &mut Control) -> bool {
    match (&mut ctl.body, ctl.id) {
        (Body::Slider(sl), id::VG_CX | id::VG_CY) => {
            let c = f.floats_or("centre", [0.0f32; 2]);
            sl.set(c[usize::from(ctl.id == id::VG_CY)]);
        }
        (Body::Choice(c), id::NEG_FILM) => {
            c.set(usize::from(f.bool_or("black_white", false)));
        }
        (Body::Choice(c), id::NEG_PICK) => {
            c.label = if s.picking_base {
                t!("develop-negative-picking")
            } else {
                t!("develop-negative-pick")
            };
        }
        (Body::Choice(c), id::ND_EDIT) => {
            c.label = if s.nd_edit {
                t!("develop-graduated-edit-done")
            } else {
                t!("develop-graduated-edit")
            };
        }
        (Body::Choice(c), id::GLOW_MODE) => {
            c.set(usize::from(f.text("mode") == Some("soften")));
        }
        (Body::Choice(c), id::GRAIN_COLOUR) => {
            c.set(usize::from(f.bool_or("colour", false)));
        }
        (Body::Choice(c), id::FRAME_SHOW) => {
            c.set(usize::from(s.frame_preview));
        }
        (Body::Choice(c), id::FRAME_COLOUR) => {
            let col = f.floats_or("colour", [1.0f32; 3]);
            let i = COLOURS
                .iter()
                .position(|k| k.iter().zip(&col).all(|(a, b)| (a - b).abs() < 1e-3))
                .unwrap_or(0);
            c.set(i);
        }
        (Body::Choice(c), id::FRAME_ASPECT) => {
            c.set(nearest(&ASPECTS, f.f32_or("aspect", 0.0)));
        }
        (Body::Choice(c), id::FRAME_MARK) => {
            c.set(usize::from(f.bool_or("watermark", false)));
        }
        (Body::Choice(c), id::FRAME_PLACE) => {
            c.set(f.i64_or("anchor", 8).clamp(0, 8) as usize);
        }
        (Body::Choice(c), id::FRAME_SVG) => {
            c.label = match f.text("svg").filter(|p| !p.is_empty()) {
                Some(p) => t!(
                    "develop-frame-svg-chosen",
                    name = Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                None => t!("develop-frame-svg"),
            };
        }
        (Body::Choice(_), id::FRAME_TEXT) => {}
        _ => return super::depth_tools::read(s, f, ctl),
    }
    true
}

/// Writes an effect control that is not a plain float into `f`. `None`: not one of
/// them; `Some(false)`: nothing to write (a button).
pub(super) fn write(ctl: &Control, f: &mut Fields) -> Option<bool> {
    match (&ctl.body, ctl.id) {
        (Body::Slider(sl), id::VG_CX | id::VG_CY) => {
            let mut c = f.floats_or("centre", [0.0f32; 2]);
            c[usize::from(ctl.id == id::VG_CY)] = sl.value;
            f.set("centre", Value::Floats(c.to_vec()));
        }
        (Body::Choice(c), id::NEG_FILM) => f.set("black_white", Value::Bool(c.selected == 1)),
        (Body::Choice(c), id::GLOW_MODE) => {
            let m = if c.selected == 1 { "soften" } else { "bloom" };
            f.set("mode", Value::Text(m.into()));
        }
        (Body::Choice(c), id::GRAIN_COLOUR) => f.set("colour", Value::Bool(c.selected == 1)),
        (Body::Choice(c), id::FRAME_COLOUR) => {
            let col = COLOURS.get(c.selected).copied().unwrap_or(COLOURS[0]);
            f.set("colour", Value::Floats(col.to_vec()));
        }
        (Body::Choice(c), id::FRAME_ASPECT) => {
            f.set(
                "aspect",
                Value::Float(ASPECTS.get(c.selected).copied().unwrap_or(0.0)),
            );
        }
        (Body::Choice(c), id::FRAME_MARK) => f.set("watermark", Value::Bool(c.selected == 1)),
        (Body::Choice(c), id::FRAME_PLACE) => f.set("anchor", Value::Int(c.selected as i64)),
        (
            Body::Choice(_),
            id::NEG_PICK | id::ND_EDIT | id::FRAME_TEXT | id::FRAME_SVG | id::FRAME_SHOW,
        ) => {
            return Some(false);
        }
        _ => return super::depth_tools::write(ctl, f),
    }
    Some(true)
}

impl Session {
    /// An effect button or view choice: its requests (`None`: not one).
    pub(super) fn effect_event(&mut self, control: u32) -> Option<Vec<Request>> {
        Some(match control {
            id::NEG_PICK => {
                self.picking_base = true;
                self.sync_panel();
                vec![Request::PickFor(ModuleKind::NEGATIVE)]
            }
            id::ND_EDIT => {
                self.nd_edit = !self.nd_edit;
                if self.nd_edit {
                    self.masks.active = None;
                }
                self.sync_panel();
                let say = if self.nd_edit {
                    t!("develop-graduated-editing")
                } else {
                    t!("develop-graduated-edit-done")
                };
                vec![Request::Say(say)]
            }
            id::FRAME_TEXT => vec![Request::WatermarkText(
                Frame::from_fields(&self.fields_of(ModuleKind::FRAME)).text,
            )],
            id::FRAME_SVG => vec![Request::ChooseWatermark],
            c if super::depth_tools::is_event(c) => return self.depth_event(c),
            id::FRAME_SHOW => {
                if let Some(Body::Choice(c)) = self.panel.control(id::FRAME_SHOW).map(|c| &c.body) {
                    self.frame_preview = c.selected == 1;
                }
                vec![Request::Render]
            }
            _ => return None,
        })
    }

    /// The film base was picked: the negative's `base` (one step).
    pub fn set_negative_base(&mut self, rgb: [f64; 3], now_ms: i64) -> bool {
        self.picking_base = false;
        let base: Vec<f32> = rgb.iter().map(|v| (*v as f32).max(1e-4)).collect();
        let before = self.fields_of(ModuleKind::NEGATIVE);
        let after = before.clone().with("base", Value::Floats(base));
        self.set_effect_fields(ModuleKind::NEGATIVE, before, after, now_ms)
    }

    /// The watermark's text template (one step).
    pub fn set_watermark_text(&mut self, text: &str, now_ms: i64) -> bool {
        let before = self.fields_of(ModuleKind::FRAME);
        let after = before
            .clone()
            .with("text", Value::Text(text.to_owned()))
            .with("svg", Value::Text(String::new()))
            .with("watermark", Value::Bool(true));
        self.set_effect_fields(ModuleKind::FRAME, before, after, now_ms)
    }

    /// The watermark's SVG file (one step).
    pub fn set_watermark_svg(&mut self, path: &Path, now_ms: i64) -> bool {
        let before = self.fields_of(ModuleKind::FRAME);
        let after = before
            .clone()
            .with("svg", Value::Text(path.to_string_lossy().into_owned()))
            .with("watermark", Value::Bool(true));
        self.set_effect_fields(ModuleKind::FRAME, before, after, now_ms)
    }

    fn set_effect_fields(
        &mut self,
        kind: ModuleKind,
        before: Fields,
        after: Fields,
        now_ms: i64,
    ) -> bool {
        if before == after {
            self.sync_panel();
            return false;
        }
        let step = Step::params(kind, self.inst(kind), &before, &after, now_ms);
        let ok = self.history.push(&mut self.edit, step).is_ok();
        self.sync_panel();
        ok
    }

    /// The fields of `kind`'s shown instance.
    pub(super) fn fields_of(&self, kind: ModuleKind) -> Fields {
        self.edit
            .module(kind, self.inst(kind))
            .map(|m| m.params.to_fields())
            .unwrap_or_default()
    }

    /// The borders and watermark to preview on the picture, when asked and on.
    pub fn frame_shown(&self) -> Option<Frame> {
        if !self.frame_preview {
            return None;
        }
        let m = self.edit.module(ModuleKind::FRAME, 0)?;
        let f = Frame::from_fields(&m.params.to_fields());
        (m.enabled && !f.is_identity()).then_some(f)
    }

    /// The graduated ND being edited on the picture.
    fn nd(&self) -> Option<Graduated> {
        // A mask row being edited has the picture's handles.
        if !self.nd_edit || self.masks.active.is_some() {
            return None;
        }
        Some(Graduated::from_fields(
            &self.fields_of(ModuleKind::GRADUATED_ND),
        ))
    }

    /// The graduated ND's handles and lines on screen (the masks' linear gradient: centre,
    /// start, end), when it is being edited on the picture.
    pub(super) fn nd_handles(&self, map: &Map) -> Option<(Vec<Handle>, Vec<Outline>)> {
        let g = self.nd()?;
        let sh = map.shown;
        let (w, h) = (f64::from(sh.w.max(1.0)), f64::from(sh.h.max(1.0)));
        let at = |p: [f64; 2]| (sh.x + (p[0] * w) as f32, sh.y + (p[1] * h) as f32);
        let (s, e) = g.ends(w, h);
        let c = [f64::from(g.centre[0]), f64::from(g.centre[1])];
        let handles = vec![
            Handle {
                role: Role::Centre,
                at: at(c),
                name: t!("develop-handle-centre"),
            },
            Handle {
                role: Role::Size,
                at: at(s),
                name: t!("develop-handle-start"),
            },
            Handle {
                role: Role::Size,
                at: at(e),
                name: t!("develop-handle-end"),
            },
        ];
        // Lines across the picture (clipped to it) through the start, the centre and the end.
        let d = g.direction();
        let line = |p: [f64; 2], faint: bool| {
            let q = [p[0] * w, p[1] * h];
            let v = [-d[1], d[0]];
            // Liang–Barsky: the part of q + t·v inside the picture.
            let (mut t0, mut t1) = (-1e9f64, 1e9f64);
            for (o, dv, hi) in [(q[0], v[0], w), (q[1], v[1], h)] {
                if dv.abs() < 1e-12 {
                    if !(0.0..=hi).contains(&o) {
                        return None;
                    }
                    continue;
                }
                let (a, b) = (-o / dv, (hi - o) / dv);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
            (t0 < t1).then(|| Outline {
                points: [t0, t1]
                    .iter()
                    .map(|t| {
                        (
                            sh.x + (q[0] + v[0] * t) as f32,
                            sh.y + (q[1] + v[1] * t) as f32,
                        )
                    })
                    .collect(),
                closed: false,
                faint,
            })
        };
        let lines = [line(s, false), line(c, true), line(e, false)];
        Some((handles, lines.into_iter().flatten().collect()))
    }

    /// Moves graduated ND handle `i` to screen point `p` (a drag in progress).
    pub(super) fn nd_move(&mut self, map: &Map, i: usize, p: (f32, f32)) -> bool {
        let Some(mut g) = self.nd() else {
            return false;
        };
        let sh = map.shown;
        let (w, h) = (f64::from(sh.w.max(1.0)), f64::from(sh.h.max(1.0)));
        let q = [f64::from(p.0 - sh.x) / w, f64::from(p.1 - sh.y) / h];
        let (s, e) = g.ends(w, h);
        match i {
            0 => g.centre = [q[0] as f32, q[1] as f32],
            1 => g.set_ends(q, e, w, h),
            2 => g.set_ends(s, q, w, h),
            _ => return false,
        }
        let inst = self.inst(ModuleKind::GRADUATED_ND);
        let Some(m) = self.edit.module_mut(ModuleKind::GRADUATED_ND, inst) else {
            return false;
        };
        let mut f = m.params.to_fields();
        if self.nd_before.is_none() {
            self.nd_before = Some(f.clone());
        }
        g.write(&mut f);
        m.params.set_fields(f);
        true
    }

    /// Ends a graduated ND drag: one history step.
    pub(super) fn nd_commit(&mut self, now_ms: i64) {
        let Some(before) = self.nd_before.take() else {
            return;
        };
        let kind = ModuleKind::GRADUATED_ND;
        let inst = self.inst(kind);
        let after = self.fields_of(kind);
        // Back to the start, then forward through history.
        if let Some(m) = self.edit.module_mut(kind, inst) {
            m.params.set_fields(before.clone());
        }
        if before != after {
            let step = Step::params(kind, inst, &before, &after, now_ms);
            let _ = self.history.push(&mut self.edit, step);
        }
        self.sync_panel();
    }
}

/// The grain seed of the photo at `path`: from its file name, so each photo has its own
/// grain and the same photo always the same.
pub(super) fn grain_seed(path: &Path) -> i64 {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    i64::from(grain::seed_for(&name))
}

/// The watermark texture for the frame preview: drawn again only when its source, text or
/// size (in 64 px steps) changes. Its texture and width/height.
pub(super) fn mark_texture(
    slot: &mut MarkSlot,
    gpu: &specular_gfx::Gpu,
    ui: &mut specular_gfx::UiRenderer,
    frame: &Frame,
    photo: &Path,
    photo_w: u32,
) -> Option<(specular_gfx::TexId, f32)> {
    use specular_develop::ops::effects::frame::{Vars, mark_svg, render_mark};
    if !frame.watermark {
        return None;
    }
    let want = ((frame.scale * photo_w as f32 / 64.0).ceil() as u32).max(1) * 64;
    let key = format!("{}\u{1}{}", frame.svg, frame.text);
    if !slot.as_ref().is_some_and(|s| s.0 == key && s.1 == want) {
        // The photo's variables: its Exif is near the start of the file.
        let head = std::fs::File::open(photo)
            .and_then(|f| {
                use std::io::Read;
                let mut b = Vec::new();
                f.take(4 << 20).read_to_end(&mut b)?;
                Ok(b)
            })
            .unwrap_or_default();
        let vars = Vars::of_file(photo, &head);
        let tex = mark_svg(frame, &vars)
            .and_then(|svg| render_mark(&svg, want))
            .ok()
            .map(|m| {
                // The UI blends premultiplied.
                let mut px = m.rgba.clone();
                for p in px.chunks_exact_mut(4) {
                    let a = u16::from(p[3]);
                    for c in &mut p[..3] {
                        *c = ((u16::from(*c) * a + 127) / 255) as u8;
                    }
                }
                let (t, id) = super::preview::upload(gpu, ui, m.w, m.h, &px);
                (t, id, m.aspect())
            });
        *slot = Some((key, want, tex));
    }
    slot.as_ref().and_then(|s| s.2.as_ref()).map(|t| (t.1, t.2))
}

/// The frame preview's watermark: what it was drawn from, its width, its texture.
pub(super) type MarkSlot = Option<(
    String,
    u32,
    Option<(specular_gfx::wgpu::Texture, specular_gfx::TexId, f32)>,
)>;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use specular_develop::model::History;
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::preview::Orient;
    use specular_gfx::Rect;

    use super::*;
    use crate::app::develop::controls;

    fn session(name: &str) -> Session {
        let p = PathBuf::from(format!("/photos/{name}"));
        Session::new(p.clone(), controls::new_edit(&p), History::default())
    }

    #[test]
    fn every_effect_has_a_card_and_a_module() {
        let s = session("IMG_1.CR3");
        for k in EFFECTS {
            assert!(s.edit.module(k, 0).is_some(), "{k:?}");
            assert!(
                s.panel.cards.iter().any(|c| c.id == u32::from(k.0)),
                "{k:?}"
            );
        }
        // A negative stays off until asked for; grain is seeded by the photo.
        assert!(!s.edit.module(ModuleKind::NEGATIVE, 0).unwrap().enabled);
        let seed = |s: &Session| s.fields_of(ModuleKind::GRAIN).i64_or("seed", 0);
        assert_ne!(seed(&s), 0);
        assert_eq!(seed(&s), seed(&session("IMG_1.CR3")));
        assert_ne!(seed(&s), seed(&session("IMG_2.CR3")));
    }

    #[test]
    fn dragging_the_graduated_filter_is_one_step() {
        let mut s = session("IMG_1.CR3");
        s.effect_event(id::ND_EDIT);
        assert!(s.nd_edit);
        let ctx = Context::image(6000, 4000, Orient::default());
        let map = Map {
            plan: Plan::new(&s.edit, &ctx, true),
            shown: Rect::new(100.0, 100.0, 600.0, 400.0),
        };
        let (hs, ol) = s.handles(&map);
        assert_eq!((hs.len(), ol.len()), (3, 3));
        let steps = s.history.steps.len();
        // The end handle down and to the right: the line turns and softens.
        let e = hs[2].at;
        assert!(s.move_handle(&map, 2, (e.0 + 80.0, e.1 + 40.0)));
        assert!(s.move_handle(&map, 2, (e.0 + 120.0, e.1 + 60.0)));
        s.commit(1000);
        assert_eq!(s.history.steps.len(), steps + 1);
        let g = Graduated::from_fields(&s.fields_of(ModuleKind::GRADUATED_ND));
        assert!(g.angle.abs() > 1.0, "{g:?}");
        // The handle sits where it was dropped.
        let (hs, _) = s.handles(&map);
        assert!((hs[2].at.0 - (e.0 + 120.0)).abs() < 0.5);
        assert!((hs[2].at.1 - (e.1 + 60.0)).abs() < 0.5);
        s.effect_event(id::ND_EDIT);
        assert!(s.handles(&map).0.is_empty());
    }

    #[test]
    fn base_pick_watermark_text_and_preview() {
        let mut s = session("IMG_1.CR3");
        let r = s.effect_event(id::NEG_PICK).unwrap();
        assert_eq!(r, vec![Request::PickFor(ModuleKind::NEGATIVE)]);
        assert!(s.set_negative_base([0.8, 0.5, 0.3], 10));
        assert!(!s.picking_base);
        assert_eq!(
            s.fields_of(ModuleKind::NEGATIVE)
                .floats_or("base", [1.0f32; 3]),
            [0.8, 0.5, 0.3]
        );
        assert!(s.set_watermark_text("© $(exif.artist)", 20));
        let f = Frame::from_fields(&s.fields_of(ModuleKind::FRAME));
        assert!(f.watermark && f.text == "© $(exif.artist)");
        // Shown only when asked.
        assert!(s.frame_shown().is_none());
        s.frame_preview = true;
        assert!(s.frame_shown().is_some());
    }
}
