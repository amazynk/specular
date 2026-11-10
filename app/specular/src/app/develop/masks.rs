//! Masks in Develop: a Masks section on each module
//! card, instances of a module, and the on-picture handles and brush.
//!
//! - **Section.** The card's menu (its ⋯ button, a right-click on its header or Shift+F10)
//!   adds masks ([`Session::mask_menu`]): "Add mask ›" (the kinds), "Use mask from ›"
//!   (another module's mask, as a live row), "Mask preset ›" (put one on, or delete one) and
//!   "Save mask as preset…". The blend (mode, amount, invert, refine edges, show mask) and each row (how it
//!   combines, opacity, invert, its own settings, "Edit on the picture", remove) follow on
//!   the card once there is a mask. Removing asks first.
//! - **Instances** (layers on screen). A card edits one instance of its module; "Layer" (on
//!   the card, with more than one) picks it. New, duplicate, delete (asks first) and move
//!   earlier or later, within the module's place in the stack, are in the card's menu.
//! - **Picture.** The row being edited shows its outline and handles ([`mask_handles`]);
//!   a brush row paints with the pointer. Shapes live in source coordinates; [`Map`]
//!   carries them through the warp to the screen and back.
//! - **History.** A drag or a stroke is one step when it ends.
//! - **Rows.** Detail rows; "Use mask from" rows (live references; deleting the source
//!   asks, names the users and copies the mask into them in one step); edge radius, grow,
//!   blur and keep-detail; polygon points added on an edge, chosen (own feather) and
//!   removed with Delete; pen pressure (flow, size or both) and smoothing; `[` `]` `{` `}`,
//!   Alt+E and `m`; mask presets (stored upright, applied by the app, see `batch.rs`; put on,
//!   saved and deleted from the card's menu).
//!
//! [`mask_handles`]: specular_widgets::mask_handles

use std::collections::HashMap;

use specular_develop::mask::{Geometry, Overlay, Space};
use specular_develop::model::mask::{
    AiKind, Blend, BlendMode, ColourRange, Combine, DepthRange, Details, Layer, MaskExpr,
    MaskShape, PathPoint, PressureUse, REFINE_RADIUS, Range, RangeChannel, Shape, Stroke,
};
use specular_develop::model::{Change, ModuleInstance, ModuleKind, Registry, Step};
use specular_develop::ops::geometry::Plan;
use specular_gfx::{Icon, Rect};
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control};
use specular_widgets::mask_handles::{Handle, Outline, Role};
use specular_widgets::slider::{Slider, SliderSpec, Unit};

use super::controls::{Request, Session};

/// Mask control ids: `MASK | kind << 8 | sub`.
pub(super) const MASK: u32 = 0x4000_0000;

/// Numbers no longer used (0 and 1 "New mask" and "Add mask", 18 "Use mask from", 19–21
/// mask presets: all in the card's menu now) are not given out again.
mod sub {
    pub const MODE: u32 = 2;
    pub const OPACITY: u32 = 3;
    pub const INVERT: u32 = 4;
    pub const REFINE: u32 = 5;
    pub const SHOW: u32 = 6;
    pub const REMOVE: u32 = 7;
    pub const INSTANCE: u32 = 8;
    pub const INST_NEW: u32 = 9;
    pub const INST_DUP: u32 = 10;
    pub const INST_DEL: u32 = 11;
    pub const INST_EARLIER: u32 = 12;
    pub const INST_LATER: u32 = 13;
    pub const RADIUS: u32 = 14;
    pub const FEATHER: u32 = 15;
    pub const BLUR: u32 = 16;
    pub const DETAILS: u32 = 17;
    /// Rows: `LAYER0 + PER·row + field`.
    pub const LAYER0: u32 = 32;
    pub const PER: u32 = 16;
}

/// Row fields.
mod field {
    pub const COMBINE: u32 = 0;
    pub const OPACITY: u32 = 1;
    pub const INVERT: u32 = 2;
    pub const A: u32 = 3;
    pub const B: u32 = 4;
    pub const C: u32 = 5;
    pub const D: u32 = 6;
    pub const E: u32 = 7;
    pub const EDIT: u32 = 8;
    pub const REMOVE: u32 = 9;
    /// Brush: what pen pressure changes, smoothing.
    pub const F: u32 = 10;
    pub const G: u32 = 11;
}

/// Rows a mask lists.
pub(super) const MAX_ROWS: usize = 14;

/// Modules a mask can go on (cards with one module each, applied after the warp).
/// The module whose masks card `card` holds: a module's own card, or the tone equalizer's
/// on Tone zones (its highlights and shadows are on Light, whose masks are exposure's).
pub(super) fn mask_module(card: u32) -> Option<ModuleKind> {
    let kind = if card == super::controls::ZONES_CARD {
        ModuleKind::TONE_EQUALIZER
    } else if card <= 0xFFFF {
        ModuleKind(card as u16)
    } else {
        return None;
    };
    MASKABLE.contains(&kind).then_some(kind)
}

pub(super) const MASKABLE: [ModuleKind; 17] = [
    ModuleKind::EXPOSURE,
    ModuleKind::TONE_EQUALIZER,
    ModuleKind::LOCAL_CONTRAST,
    ModuleKind::CURVES,
    ModuleKind::COLOUR_BALANCE,
    ModuleKind::COLOUR_EQUALIZER,
    ModuleKind::POINT_COLOUR,
    ModuleKind::BLACK_WHITE,
    ModuleKind::LUT_3D,
    ModuleKind::DENOISE,
    ModuleKind::SHARPEN,
    ModuleKind::NEGATIVE,
    ModuleKind::DEHAZE,
    ModuleKind::GRADUATED_ND,
    ModuleKind::VIGNETTE,
    ModuleKind::GLOW,
    ModuleKind::GRAIN,
];

pub(super) fn is_mask(id: u32) -> bool {
    id & 0xC000_0000 == MASK
}

fn mid(kind: ModuleKind, s: u32) -> u32 {
    MASK | (u32::from(kind.0) << 8) | s
}

fn split(id: u32) -> (ModuleKind, u32) {
    (ModuleKind(((id >> 8) & 0xFFFF) as u16), id & 0xFF)
}

/// What "New mask" offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NewKind {
    Ellipse,
    Rectangle,
    Polygon,
    Linear,
    Radial,
    Brush,
    Luminance,
    Hue,
    Chroma,
    Colour,
    /// Textured areas.
    Details,
    /// Another module's mask ("Use mask from"; added from its own list).
    Borrowed,
    /// A combination copied in (a borrowed mask whose module went).
    Group,
    /// AI masks.
    Subject,
    Sky,
    Object,
    /// Depth rows: the nearest part, the farthest, a band between.
    Near,
    Far,
    DepthBand,
}

const NEW_KINDS: [NewKind; 17] = [
    NewKind::Ellipse,
    NewKind::Rectangle,
    NewKind::Polygon,
    NewKind::Linear,
    NewKind::Radial,
    NewKind::Brush,
    NewKind::Luminance,
    NewKind::Hue,
    NewKind::Chroma,
    NewKind::Colour,
    NewKind::Details,
    NewKind::Subject,
    NewKind::Sky,
    NewKind::Object,
    NewKind::Near,
    NewKind::Far,
    NewKind::DepthBand,
];

/// What a card's menu does with its module's instances and masks ([`Session::mask_menu`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaskAct {
    InstNew,
    InstDup,
    InstDel,
    InstEarlier,
    InstLater,
    /// "Add mask ›": leads to the kinds.
    AddMenu,
    /// Adds a mask of kind `NEW_KINDS[i]`.
    Add(usize),
    /// "Use mask from ›": leads to the other modules' masks.
    FromMenu,
    /// Adds a live row of candidate `i` of [`Session::borrow_candidates`]'s mask.
    From(usize),
    /// "Mask preset ›": leads to the presets, to put on or delete.
    PresetMenu,
    /// Puts mask preset `i` on.
    Preset(usize),
    /// Deletes mask preset `i` (asks first).
    PresetDelete(usize),
    /// "Save mask as preset…" (the card has a mask).
    PresetSave,
}

impl MaskAct {
    /// Whether the row leads to more rows (a submenu) instead of doing something.
    pub(super) fn is_submenu(self) -> bool {
        matches!(
            self,
            MaskAct::AddMenu | MaskAct::FromMenu | MaskAct::PresetMenu
        )
    }
}

/// A menu row: what it does, its label and its icon.
pub(super) type MenuRow = (MaskAct, String, Icon);

/// A card menu's rows for its instances and masks ([`Session::mask_menu`]).
#[derive(Default)]
pub(super) struct MaskMenu {
    /// The menu's own rows, in groups.
    pub groups: Vec<Vec<MenuRow>>,
    /// The rows each submenu row (`AddMenu`, `FromMenu`, `PresetMenu`) leads to.
    pub subs: Vec<(MaskAct, Vec<MenuRow>)>,
}

fn kind_name(k: NewKind) -> String {
    match k {
        NewKind::Ellipse => t!("develop-mask-kind-ellipse"),
        NewKind::Rectangle => t!("develop-mask-kind-rectangle"),
        NewKind::Polygon => t!("develop-mask-kind-polygon"),
        NewKind::Linear => t!("develop-mask-kind-linear"),
        NewKind::Radial => t!("develop-mask-kind-radial"),
        NewKind::Brush => t!("develop-mask-kind-brush"),
        NewKind::Luminance => t!("develop-mask-kind-luminance"),
        NewKind::Hue => t!("develop-mask-kind-hue"),
        NewKind::Chroma => t!("develop-mask-kind-chroma"),
        NewKind::Colour => t!("develop-mask-kind-colour"),
        NewKind::Details => t!("develop-mask-kind-details"),
        NewKind::Borrowed => t!("develop-mask-kind-borrowed"),
        NewKind::Group => t!("develop-mask-kind-group"),
        NewKind::Subject => t!("develop-mask-kind-subject"),
        NewKind::Sky => t!("develop-mask-kind-sky"),
        NewKind::Object => t!("develop-mask-kind-object"),
        NewKind::Near => t!("develop-mask-kind-near"),
        NewKind::Far => t!("develop-mask-kind-far"),
        NewKind::DepthBand => t!("develop-mask-kind-depth"),
    }
}

/// The blend modes the panel offers (others read as the first).
const MODES: [BlendMode; 7] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::SoftLight,
    BlendMode::Luminance,
    BlendMode::Colour,
];

fn mode_name(m: BlendMode) -> String {
    match m {
        BlendMode::Multiply => t!("develop-blend-multiply"),
        BlendMode::Screen => t!("develop-blend-screen"),
        BlendMode::Overlay => t!("develop-blend-overlay"),
        BlendMode::SoftLight => t!("develop-blend-soft-light"),
        BlendMode::Luminance => t!("develop-blend-lightness"),
        BlendMode::Colour => t!("develop-blend-colour"),
        _ => t!("develop-blend-normal"),
    }
}

const COMBINES: [Combine; 3] = [Combine::Union, Combine::Subtract, Combine::Intersect];

/// Brush settings for new strokes (not part of the edit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BrushSettings {
    /// Radius, fraction of the long side.
    pub size: f32,
    pub hardness: f32,
    pub flow: f32,
    pub erase: bool,
    /// What a pen's pressure changes.
    pub pressure: PressureUse,
    /// Stroke smoothing 0..1: how far each point trails the pointer.
    pub smoothing: f32,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            size: 0.04,
            hardness: 0.5,
            flow: 0.8,
            erase: false,
            pressure: PressureUse::Size,
            smoothing: 0.3,
        }
    }
}

/// The row being edited on the picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Active {
    pub kind: ModuleKind,
    pub inst: u16,
    pub row: usize,
}

/// A change in progress (a drag, a stroke): one history step when it ends.
#[derive(Clone, Debug)]
struct Pending {
    kind: ModuleKind,
    inst: u16,
    blend: Option<Blend>,
    masks: Vec<MaskShape>,
}

/// What a confirmation asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Confirm {
    RemoveMask(ModuleKind, u16),
    RemoveRow(ModuleKind, u16, usize),
    DeleteInstance(ModuleKind, u16),
    /// A mask preset, by its index in the list.
    DeleteMaskPreset(usize),
}

/// The masks' state of a session.
#[derive(Clone, Debug, Default)]
pub(super) struct State {
    /// The instance each card edits (0 when absent).
    pub inst: HashMap<ModuleKind, u16>,
    pub active: Option<Active>,
    /// Show this module's mask.
    pub overlay: Option<((ModuleKind, u16), Overlay)>,
    pub brush: BrushSettings,
    pending: Option<Pending>,
    confirm: Option<Confirm>,
    /// The next pick adds a colour to this row's colour range.
    pub picking: Option<Active>,
    /// A brush stroke is being painted.
    pub painting: bool,
    /// The handle being dragged.
    pub dragging: Option<usize>,
    /// The polygon point chosen on the picture (its feather is the one the slider sets;
    /// Delete removes it).
    pub point: Option<usize>,
    /// The mask presets' names, as the app lists them.
    pub preset_names: Vec<String>,
    /// Pen pressure of the next brush point (none: a mouse, full pressure).
    pub pressure: Option<f32>,
    /// The smoothed brush position (frame fractions) of the stroke being painted.
    smoothed: Option<(f64, f64)>,
}

fn choice(id: u32, label: String, options: Vec<String>, selected: usize) -> Control {
    let mut c = Choice::new(label, options);
    c.set(selected);
    Control {
        id,
        body: Body::Choice(c),
    }
}

fn button(id: u32, label: String) -> Control {
    Control {
        id,
        body: Body::Choice(Choice::button(label)),
    }
}

fn onoff(id: u32, label: String, on: bool) -> Control {
    choice(
        id,
        label,
        vec![t!("develop-mask-off"), t!("develop-mask-on")],
        usize::from(on),
    )
}

fn slider(id: u32, spec: SliderSpec, v: f32) -> Control {
    let mut s = Slider::new(spec);
    s.set(v);
    Control {
        id,
        body: Body::Slider(s),
    }
}

fn pct(id: u32, label: String, min: f32, max: f32, default: f32, v: f32) -> Control {
    let mut spec = SliderSpec::new(label, min, max, default)
        .steps(0.01, 0.1)
        .unit(Unit::Percent, 0);
    if min < 0.0 {
        spec = spec.signed();
    }
    slider(id, spec, v)
}

fn plain(
    id: u32,
    label: String,
    (min, max): (f32, f32),
    step: f32,
    default: f32,
    v: f32,
) -> Control {
    slider(
        id,
        SliderSpec::new(label, min, max, default)
            .steps(step, step * 10.0)
            .unit(Unit::Plain, 2),
        v,
    )
}

/// What a row is, for its name and controls.
fn row_kind(item: &MaskExpr, masks: &[MaskShape]) -> Option<NewKind> {
    Some(match item {
        MaskExpr::Shape(id) => match &masks.iter().find(|m| m.id == *id)?.shape {
            Shape::Ellipse { .. } => NewKind::Ellipse,
            Shape::Rectangle { .. } => NewKind::Rectangle,
            Shape::Polygon { .. } => NewKind::Polygon,
            Shape::Linear { .. } => NewKind::Linear,
            Shape::Radial { .. } => NewKind::Radial,
            Shape::Brush { .. } => NewKind::Brush,
        },
        MaskExpr::Range(r) => match r.channel {
            RangeChannel::Hue => NewKind::Hue,
            RangeChannel::Chroma => NewKind::Chroma,
            _ => NewKind::Luminance,
        },
        MaskExpr::ColourRange(_) => NewKind::Colour,
        MaskExpr::Details(_) => NewKind::Details,
        MaskExpr::FromModule { .. } => NewKind::Borrowed,
        MaskExpr::Ai(m) => match m.kind {
            AiKind::Subject => NewKind::Subject,
            AiKind::Sky => NewKind::Sky,
            AiKind::Object => NewKind::Object,
        },
        MaskExpr::Depth(d) => {
            if d.near.min(d.far) <= 0.0 {
                NewKind::Near
            } else if d.near.max(d.far) >= 1.0 {
                NewKind::Far
            } else {
                NewKind::DepthBand
            }
        }
        MaskExpr::Combine(..) => NewKind::Group,
        _ => return None,
    })
}

/// A row's name: "Ellipse 2".
fn row_name(layers: &[Layer], i: usize, masks: &[MaskShape]) -> String {
    if let MaskExpr::FromModule { kind, instance } = &layers[i].item {
        return t!(
            "develop-mask-row-from",
            module = super::controls::module_title(*kind),
            n = i64::from(*instance) + 1
        );
    }
    let k = row_kind(&layers[i].item, masks);
    let n = layers[..=i]
        .iter()
        .filter(|l| row_kind(&l.item, masks) == k)
        .count();
    let kind = k.map_or_else(|| t!("develop-mask-kind-other"), kind_name);
    t!("develop-mask-row", kind = kind, n = n as i64)
}

/// Whether `from`'s mask borrows `target`'s, directly or through others.
fn reaches(
    edit: &specular_develop::model::Edit,
    from: (ModuleKind, u16),
    target: (ModuleKind, u16),
    depth: u32,
) -> bool {
    fn refs(e: &MaskExpr, out: &mut Vec<(ModuleKind, u16)>) {
        match e {
            MaskExpr::FromModule { kind, instance } => out.push((*kind, *instance)),
            MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => refs(a, out),
            MaskExpr::Combine(_, a, b) => {
                refs(a, out);
                refs(b, out);
            }
            _ => {}
        }
    }
    if depth > 8 {
        return true;
    }
    let Some(b) = edit.module(from.0, from.1).and_then(|m| m.blend.as_ref()) else {
        return false;
    };
    let mut v = Vec::new();
    refs(&b.mask, &mut v);
    v.into_iter()
        .any(|r| r == target || reaches(edit, r, target, depth + 1))
}

/// The shape of a row, if it is one.
fn row_shape<'a>(l: &Layer, masks: &'a [MaskShape]) -> Option<(u32, &'a Shape)> {
    match l.item {
        MaskExpr::Shape(id) => masks.iter().find(|m| m.id == id).map(|m| (id, &m.shape)),
        _ => None,
    }
}

impl Session {
    /// The instance a card edits.
    pub(super) fn inst(&self, kind: ModuleKind) -> u16 {
        let i = self.masks.inst.get(&kind).copied().unwrap_or(0);
        if self.edit.module(kind, i).is_some() {
            i
        } else {
            0
        }
    }

    fn blend_of(&self, kind: ModuleKind) -> Option<&Blend> {
        self.edit
            .module(kind, self.inst(kind))
            .and_then(|m| m.blend.as_ref())
    }

    pub(super) fn layers_of(&self, kind: ModuleKind) -> Vec<Layer> {
        self.blend_of(kind)
            .and_then(|b| b.mask.layers())
            .unwrap_or_default()
    }

    /// The Masks section of a card.
    fn mask_controls(&self, kind: ModuleKind) -> Vec<Control> {
        let id = |s| mid(kind, s);
        let mut v = Vec::new();
        // Instances.
        let insts: Vec<u16> = self
            .edit
            .modules
            .iter()
            .filter(|m| m.kind == kind)
            .map(|m| m.instance)
            .collect();
        let at = insts
            .iter()
            .position(|&i| i == self.inst(kind))
            .unwrap_or(0);
        if insts.len() > 1 {
            v.push(choice(
                id(sub::INSTANCE),
                t!("develop-instance"),
                (1..=insts.len())
                    .map(|n| t!("develop-instance-n", n = n as i64))
                    .collect(),
                at,
            ));
        }
        // Instances (new, duplicate, delete, move) and adding a mask (a kind, another module's
        // mask, a preset), and saving it as a preset: the card's menu ([`Self::mask_menu`]).
        let Some(b) = self.blend_of(kind) else {
            return v;
        };
        let mode = MODES.iter().position(|m| *m == b.mode).unwrap_or(0);
        v.push(choice(
            id(sub::MODE),
            t!("develop-mask-mode"),
            MODES.iter().map(|m| mode_name(*m)).collect(),
            mode,
        ));
        v.push(pct(
            id(sub::OPACITY),
            t!("develop-mask-amount"),
            0.0,
            1.0,
            1.0,
            b.opacity,
        ));
        v.push(onoff(id(sub::INVERT), t!("develop-mask-invert"), b.invert));
        v.push(pct(
            id(sub::REFINE),
            t!("develop-mask-refine"),
            0.0,
            1.0,
            0.0,
            b.refine.edge_aware,
        ));
        if b.refine.edge_aware > 0.0 {
            v.push(slider(
                id(sub::RADIUS),
                SliderSpec::new(t!("develop-mask-refine-radius"), 0.002, 0.1, REFINE_RADIUS)
                    .steps(0.001, 0.01)
                    .unit(Unit::Percent, 1),
                b.refine.radius,
            ));
        }
        for (s, label, value) in [
            (sub::FEATHER, t!("develop-mask-grow"), b.refine.feather),
            (sub::BLUR, t!("develop-mask-blur"), b.refine.blur),
        ] {
            v.push(slider(
                id(s),
                SliderSpec::new(label, 0.0, 0.1, 0.0)
                    .steps(0.001, 0.01)
                    .unit(Unit::Percent, 1),
                value,
            ));
        }
        v.push(slider(
            id(sub::DETAILS),
            SliderSpec::new(t!("develop-mask-details-keep"), -1.0, 1.0, 0.0)
                .steps(0.01, 0.1)
                .unit(Unit::Plain, 2)
                .signed(),
            b.refine.details,
        ));
        let me = (kind, self.inst(kind));
        let show = match self.masks.overlay {
            Some((m, Overlay::Tint)) if m == me => 1,
            Some((m, Overlay::Grey)) if m == me => 2,
            _ => 0,
        };
        v.push(choice(
            id(sub::SHOW),
            t!("develop-mask-show"),
            vec![
                t!("develop-mask-off"),
                t!("develop-mask-show-red"),
                t!("develop-mask-show-grey"),
            ],
            show,
        ));
        let layers = self.layers_of(kind);
        for (i, l) in layers.iter().enumerate().take(MAX_ROWS) {
            v.extend(self.row_controls(kind, &layers, i, l));
        }
        v.push(button(id(sub::REMOVE), t!("develop-mask-remove")));
        v
    }

    fn row_controls(
        &self,
        kind: ModuleKind,
        layers: &[Layer],
        i: usize,
        l: &Layer,
    ) -> Vec<Control> {
        let f = |fld: u32| mid(kind, sub::LAYER0 + sub::PER * i as u32 + fld);
        let masks = &self.edit.masks;
        let name = row_name(layers, i, masks);
        let mut v = Vec::new();
        let combine = COMBINES.iter().position(|c| *c == l.op).unwrap_or(0);
        let options = if i == 0 {
            vec![t!("develop-mask-combine-add")]
        } else {
            vec![
                t!("develop-mask-combine-add"),
                t!("develop-mask-combine-subtract"),
                t!("develop-mask-combine-intersect"),
            ]
        };
        v.push(choice(
            f(field::COMBINE),
            name.clone(),
            options,
            if i == 0 { 0 } else { combine },
        ));
        v.push(pct(
            f(field::OPACITY),
            t!("develop-mask-row-opacity"),
            0.0,
            1.0,
            1.0,
            l.opacity,
        ));
        v.push(onoff(
            f(field::INVERT),
            t!("develop-mask-row-invert"),
            l.invert,
        ));
        match (&l.item, row_shape(l, masks).map(|s| s.1)) {
            (_, Some(Shape::Ellipse { feather, .. })) => {
                v.push(pct(
                    f(field::A),
                    t!("develop-mask-feather"),
                    0.0,
                    1.0,
                    0.5,
                    *feather,
                ));
            }
            (_, Some(Shape::Rectangle { round, feather, .. })) => {
                v.push(pct(
                    f(field::A),
                    t!("develop-mask-feather"),
                    0.0,
                    1.0,
                    0.3,
                    *feather,
                ));
                v.push(pct(
                    f(field::B),
                    t!("develop-mask-roundness"),
                    0.0,
                    1.0,
                    0.2,
                    *round,
                ));
            }
            (_, Some(Shape::Polygon { points, .. })) => {
                // The chosen point's feather, or all points' (the first's shown).
                let point = self
                    .masks
                    .point
                    .filter(|&p| p < points.len() && self.is_active(kind, i));
                let fe = points.get(point.unwrap_or(0)).map_or(0.0, |p| p.feather);
                v.push(pct(
                    f(field::A),
                    match point {
                        Some(p) => t!("develop-mask-point-feather", n = p as i64 + 1),
                        None => t!("develop-mask-feather"),
                    },
                    0.0,
                    0.2,
                    0.02,
                    fe,
                ));
            }
            (_, Some(Shape::Brush { .. })) => {
                let b = self.masks.brush;
                v.push(pct(
                    f(field::A),
                    t!("develop-mask-brush-size"),
                    0.002,
                    0.25,
                    0.04,
                    b.size,
                ));
                v.push(pct(
                    f(field::B),
                    t!("develop-mask-brush-feather"),
                    0.0,
                    1.0,
                    0.5,
                    1.0 - b.hardness,
                ));
                v.push(pct(
                    f(field::C),
                    t!("develop-mask-brush-flow"),
                    0.05,
                    1.0,
                    0.8,
                    b.flow,
                ));
                v.push(choice(
                    f(field::D),
                    t!("develop-mask-brush-mode"),
                    vec![
                        t!("develop-mask-brush-paint"),
                        t!("develop-mask-brush-erase"),
                    ],
                    usize::from(b.erase),
                ));
                v.push(choice(
                    f(field::F),
                    t!("develop-mask-brush-pressure"),
                    vec![
                        t!("develop-mask-brush-pressure-flow"),
                        t!("develop-mask-brush-pressure-size"),
                        t!("develop-mask-brush-pressure-both"),
                    ],
                    match b.pressure {
                        PressureUse::Flow => 0,
                        PressureUse::Size => 1,
                        PressureUse::Both => 2,
                    },
                ));
                v.push(pct(
                    f(field::G),
                    t!("develop-mask-brush-smoothing"),
                    0.0,
                    0.9,
                    0.3,
                    b.smoothing,
                ));
            }
            (MaskExpr::Ai(m), _) => {
                v.push(Control {
                    id: f(field::A),
                    body: Body::Note(self.ai_state_line(m)),
                });
                if self.ai_missing(m) {
                    v.push(button(f(field::B), t!("develop-ai-open-settings")));
                }
                if m.kind == AiKind::Object && (!m.points.is_empty() || m.rect.is_some()) {
                    v.push(button(f(field::C), t!("develop-ai-clear")));
                }
            }
            (MaskExpr::Depth(d), _) => {
                let key = specular_develop::mask::depth::key(
                    &specular_develop::mask::depth::photo(&self.edit.source),
                    &d.model,
                );
                let st = self.ai2_state.get(&key);
                v.push(Control {
                    id: f(field::D),
                    body: Body::Note(super::ai_remove::state_line(
                        st,
                        t!("develop-depth-computing"),
                    )),
                });
                if matches!(st, Some(super::ai_remove::NetState::Missing(_))) {
                    v.push(button(f(field::E), t!("develop-ai-open-settings")));
                }
                v.push(plain(
                    f(field::A),
                    t!("develop-mask-depth-near"),
                    (0.0, 1.0),
                    0.01,
                    0.0,
                    d.near,
                ));
                v.push(plain(
                    f(field::B),
                    t!("develop-mask-depth-far"),
                    (0.0, 1.0),
                    0.01,
                    1.0,
                    d.far,
                ));
                v.push(plain(
                    f(field::C),
                    t!("develop-mask-depth-feather"),
                    (0.0, 0.5),
                    0.01,
                    0.1,
                    d.feather,
                ));
            }
            (MaskExpr::Details(d), _) => {
                v.push(plain(
                    f(field::A),
                    t!("develop-mask-details-threshold"),
                    (0.0, 1.5),
                    0.01,
                    0.25,
                    d.threshold,
                ));
                v.push(plain(
                    f(field::B),
                    t!("develop-mask-details-softness"),
                    (0.0, 1.0),
                    0.01,
                    0.2,
                    d.softness,
                ));
            }
            (MaskExpr::Range(r), _) => {
                let (lo, hi, step) = match r.channel {
                    RangeChannel::Hue => (0.0, 720.0, 1.0),
                    RangeChannel::Chroma => (0.0, 0.4, 0.005),
                    _ => (-10.0, 6.0, 0.1),
                };
                let labels = [
                    t!("develop-mask-range-from"),
                    t!("develop-mask-range-full-from"),
                    t!("develop-mask-range-full-to"),
                    t!("develop-mask-range-to"),
                ];
                for (k, label) in labels.into_iter().enumerate() {
                    v.push(plain(
                        f(field::A + k as u32),
                        label,
                        (lo, hi),
                        step,
                        r.limits[k],
                        r.limits[k],
                    ));
                }
                v.push(choice(
                    f(field::E),
                    t!("develop-mask-range-test"),
                    vec![
                        t!("develop-mask-range-input"),
                        t!("develop-mask-range-output"),
                    ],
                    usize::from(r.on_output),
                ));
            }
            (MaskExpr::ColourRange(c), _) => {
                v.push(button(
                    f(field::E),
                    if c.colours.is_empty() {
                        t!("develop-mask-colour-pick")
                    } else {
                        t!("develop-mask-colour-pick-more")
                    },
                ));
                v.push(plain(
                    f(field::A),
                    t!("develop-mask-colour-range"),
                    (0.0, 0.3),
                    0.005,
                    0.04,
                    c.distance,
                ));
                v.push(plain(
                    f(field::B),
                    t!("develop-mask-colour-falloff"),
                    (0.0, 0.3),
                    0.005,
                    0.06,
                    c.falloff,
                ));
            }
            _ => {}
        }
        let object = matches!(&l.item, MaskExpr::Ai(m) if m.kind == AiKind::Object);
        if row_shape(l, masks).is_some() || object {
            v.push(button(f(field::EDIT), t!("develop-mask-edit")));
        }
        v.push(button(
            f(field::REMOVE),
            t!("develop-mask-row-remove", name = name),
        ));
        v
    }

    /// The rows `kind`'s card menu has for its instances and masks (none for a module that
    /// takes no masks): adding a mask ("Add mask ›", "Use mask from ›" when another module
    /// has one, "Mask preset ›" when there are presets, "Save mask as preset…" when this one
    /// has a mask), then the instances.
    pub(super) fn mask_menu(&self, kind: ModuleKind) -> MaskMenu {
        if !MASKABLE.contains(&kind) {
            return MaskMenu::default();
        }
        let insts: Vec<u16> = self
            .edit
            .modules
            .iter()
            .filter(|m| m.kind == kind)
            .map(|m| m.instance)
            .collect();
        let at = insts
            .iter()
            .position(|&i| i == self.inst(kind))
            .unwrap_or(0);
        // The menu's mark of a row that leads to more choices (a submenu).
        let more = Icon::ChevronRight;
        let mut masks = vec![(MaskAct::AddMenu, t!("develop-mask-add"), more)];
        let mut subs = vec![(
            MaskAct::AddMenu,
            NEW_KINDS
                .iter()
                .enumerate()
                .map(|(i, k)| (MaskAct::Add(i), kind_name(*k), Icon::Plus))
                .collect(),
        )];
        let from = self.borrow_candidates(kind);
        if !from.is_empty() {
            masks.push((MaskAct::FromMenu, t!("develop-mask-from"), more));
            subs.push((
                MaskAct::FromMenu,
                from.iter()
                    .enumerate()
                    .map(|(i, &(k, n))| (MaskAct::From(i), self.instance_title(k, n), Icon::Plus))
                    .collect(),
            ));
        }
        let presets = &self.masks.preset_names;
        if !presets.is_empty() {
            masks.push((MaskAct::PresetMenu, t!("develop-mask-preset"), more));
            let put = presets
                .iter()
                .enumerate()
                .map(|(i, n)| (MaskAct::Preset(i), n.clone(), Icon::Check));
            let delete = presets.iter().enumerate().map(|(i, n)| {
                (
                    MaskAct::PresetDelete(i),
                    t!("develop-mask-preset-delete", name = n.clone()),
                    Icon::Trash,
                )
            });
            subs.push((MaskAct::PresetMenu, put.chain(delete).collect()));
        }
        if self.blend_of(kind).is_some() {
            masks.push((
                MaskAct::PresetSave,
                t!("develop-mask-preset-save"),
                Icon::Download,
            ));
        }
        let mut inst = vec![
            (MaskAct::InstNew, t!("develop-instance-new"), Icon::Plus),
            (
                MaskAct::InstDup,
                t!("develop-instance-duplicate"),
                Icon::Copy,
            ),
        ];
        if insts.len() > 1 {
            if at > 0 {
                inst.push((
                    MaskAct::InstEarlier,
                    t!("develop-instance-earlier"),
                    Icon::ChevronUp,
                ));
            }
            if at + 1 < insts.len() {
                inst.push((
                    MaskAct::InstLater,
                    t!("develop-instance-later"),
                    Icon::ChevronDown,
                ));
            }
            inst.push((MaskAct::InstDel, t!("develop-instance-delete"), Icon::Trash));
        }
        MaskMenu {
            groups: vec![masks, inst],
            subs,
        }
    }

    /// Does a card menu's `act` on `kind`'s instances or masks. Adding opens the card, so
    /// the new instance or mask row shows.
    pub(super) fn mask_act(&mut self, kind: ModuleKind, act: MaskAct, now_ms: i64) -> Vec<Request> {
        if matches!(
            act,
            MaskAct::Add(_)
                | MaskAct::From(_)
                | MaskAct::Preset(_)
                | MaskAct::InstNew
                | MaskAct::InstDup
        ) && let Some(c) = self.panel.card_mut(u32::from(kind.0))
        {
            c.expanded = true;
        }
        let inst = self.inst(kind);
        let s = match act {
            MaskAct::AddMenu | MaskAct::FromMenu | MaskAct::PresetMenu => return Vec::new(),
            MaskAct::From(i) => {
                let Some(&from) = self.borrow_candidates(kind).get(i) else {
                    return Vec::new();
                };
                let mut out = vec![Request::Render];
                out.extend(self.add_borrowed(kind, from, now_ms));
                return out;
            }
            MaskAct::Preset(i) => return vec![Request::ApplyMaskPreset(kind, inst, i)],
            MaskAct::PresetSave => return vec![Request::SaveMaskPreset(kind, inst)],
            MaskAct::PresetDelete(i) => {
                let Some(name) = self.masks.preset_names.get(i).cloned() else {
                    return Vec::new();
                };
                self.masks.confirm = Some(Confirm::DeleteMaskPreset(i));
                return vec![Request::Confirm(
                    t!("develop-mask-preset-delete-confirm", name = name),
                    t!("develop-mask-preset-delete-ok"),
                )];
            }
            MaskAct::Add(i) => {
                let k = NEW_KINDS.get(i).copied().unwrap_or(NewKind::Ellipse);
                let mut out = vec![Request::Render];
                out.extend(self.add_row(kind, k, now_ms));
                return out;
            }
            MaskAct::InstNew => sub::INST_NEW,
            MaskAct::InstDup => sub::INST_DUP,
            MaskAct::InstDel => sub::INST_DEL,
            MaskAct::InstEarlier => sub::INST_EARLIER,
            MaskAct::InstLater => sub::INST_LATER,
        };
        self.mask_value(mid(kind, s), true, now_ms)
    }

    /// Puts the Masks sections on the maskable cards (rebuilt from the edit).
    pub(super) fn sync_masks(&self, cards: &mut [Card]) {
        for c in cards {
            let Some(kind) = mask_module(c.id) else {
                continue;
            };
            c.controls.retain(|ctl| !is_mask(ctl.id));
            c.controls.extend(self.mask_controls(kind));
        }
    }

    /// Whether row `row` of `kind`'s card is the one edited on the picture.
    fn is_active(&self, kind: ModuleKind, row: usize) -> bool {
        self.masks
            .active
            .is_some_and(|a| a.kind == kind && a.row == row && a.inst == self.inst(kind))
    }

    /// Starts or continues a change of `kind`'s blend or the shapes.
    pub(super) fn begin(&mut self, kind: ModuleKind) {
        let inst = self.inst(kind);
        if self
            .masks
            .pending
            .as_ref()
            .is_some_and(|p| (p.kind, p.inst) == (kind, inst))
        {
            return;
        }
        self.commit(0);
        let blend = self.edit.module(kind, inst).and_then(|m| m.blend.clone());
        self.masks.pending = Some(Pending {
            kind,
            inst,
            blend,
            masks: self.edit.masks.clone(),
        });
    }

    /// Ends the change in progress: one step for the shapes, one for the blend.
    pub(super) fn commit(&mut self, now_ms: i64) {
        self.nd_commit(now_ms);
        let Some(p) = self.masks.pending.take() else {
            return;
        };
        let after_masks = self.edit.masks.clone();
        let after_blend = self
            .edit
            .module(p.kind, p.inst)
            .and_then(|m| m.blend.clone());
        // Back to the start, then forward through history.
        self.edit.masks = p.masks.clone();
        if let Some(m) = self.edit.module_mut(p.kind, p.inst) {
            m.blend = p.blend.clone();
        }
        if after_masks != p.masks {
            let _ = self.history.push(
                &mut self.edit,
                Step {
                    kind: p.kind,
                    instance: p.inst,
                    change: Change::Masks {
                        before: p.masks,
                        after: after_masks,
                    },
                    time_ms: now_ms,
                },
            );
        }
        if after_blend != p.blend {
            let _ = self.history.push(
                &mut self.edit,
                Step {
                    kind: p.kind,
                    instance: p.inst,
                    change: Change::Blend {
                        before: p.blend.map(Box::new),
                        after: after_blend.map(Box::new),
                    },
                    time_ms: now_ms,
                },
            );
        }
        self.after_mask_step();
    }

    fn after_mask_step(&mut self) {
        self.sync_panel();
    }

    fn blend_mut(&mut self, kind: ModuleKind) -> Option<&mut Blend> {
        let inst = self.inst(kind);
        self.edit
            .module_mut(kind, inst)
            .and_then(|m| m.blend.as_mut())
    }

    pub(super) fn set_layers(&mut self, kind: ModuleKind, layers: &[Layer]) {
        let inst = self.inst(kind);
        if let Some(m) = self.edit.module_mut(kind, inst) {
            let b = m.blend.get_or_insert_with(Blend::default);
            b.mask = MaskExpr::from_layers(layers);
        }
    }

    /// A Masks control changed; what the application should do.
    pub(super) fn mask_value(&mut self, control: u32, done: bool, now_ms: i64) -> Vec<Request> {
        let (kind, s) = split(control);
        // The card's menu sends these without a control on the card.
        let action = matches!(
            s,
            sub::INST_NEW | sub::INST_DUP | sub::INST_DEL | sub::INST_EARLIER | sub::INST_LATER
        );
        let ctl = self.panel.control(control).cloned();
        if ctl.is_none() && !action {
            return Vec::new();
        }
        let sel = match ctl.as_ref().map(|c| &c.body) {
            Some(Body::Choice(c)) => c.selected,
            _ => 0,
        };
        let val = match ctl.as_ref().map(|c| &c.body) {
            Some(Body::Slider(sl)) => sl.value,
            _ => 0.0,
        };
        let out = vec![Request::Render];
        match s {
            sub::MODE
            | sub::OPACITY
            | sub::INVERT
            | sub::REFINE
            | sub::RADIUS
            | sub::FEATHER
            | sub::BLUR
            | sub::DETAILS => {
                self.begin(kind);
                let refine_shown = self
                    .blend_of(kind)
                    .is_some_and(|b| b.refine.edge_aware > 0.0);
                if let Some(b) = self.blend_mut(kind) {
                    match s {
                        sub::MODE => b.mode = MODES.get(sel).copied().unwrap_or_default(),
                        sub::OPACITY => b.opacity = val,
                        sub::INVERT => b.invert = sel == 1,
                        sub::RADIUS => b.refine.radius = val,
                        sub::FEATHER => b.refine.feather = val,
                        sub::BLUR => b.refine.blur = val,
                        sub::DETAILS => b.refine.details = val,
                        _ => b.refine.edge_aware = val,
                    }
                }
                if done {
                    self.commit(now_ms);
                } else if s == sub::REFINE && refine_shown != (val > 0.0) {
                    // The radius slider comes and goes with edge refinement.
                    self.sync_panel();
                }
            }
            sub::SHOW => {
                let me = (kind, self.inst(kind));
                self.masks.overlay = match sel {
                    1 => Some((me, Overlay::Tint)),
                    2 => Some((me, Overlay::Grey)),
                    _ => None,
                };
            }
            sub::REMOVE => {
                let inst = self.inst(kind);
                self.masks.confirm = Some(Confirm::RemoveMask(kind, inst));
                let mut q = t!("develop-mask-remove-confirm", module = self.title_of(kind));
                if let Some(u) = self.users_line(kind, inst) {
                    q = format!("{q}\n{u}");
                }
                return vec![Request::Confirm(q, t!("develop-mask-remove-ok"))];
            }
            sub::INSTANCE => {
                let insts: Vec<u16> = self
                    .edit
                    .modules
                    .iter()
                    .filter(|m| m.kind == kind)
                    .map(|m| m.instance)
                    .collect();
                if let Some(&i) = insts.get(sel) {
                    self.masks.inst.insert(kind, i);
                    self.masks.active = None;
                }
                self.sync_panel();
                return vec![Request::Say(t!(
                    "develop-instance-chosen",
                    n = sel as i64 + 1
                ))];
            }
            sub::INST_NEW | sub::INST_DUP => {
                let reg = Registry::builtin();
                let n = self.edit.next_instance(kind);
                let m = if s == sub::INST_DUP {
                    let mut m = self
                        .edit
                        .module(kind, self.inst(kind))
                        .cloned()
                        .unwrap_or_else(|| ModuleInstance::new(reg, kind, n));
                    m.instance = n;
                    m
                } else {
                    ModuleInstance::new(reg, kind, n)
                };
                // Right after the card's instances (the module's place in the stack).
                let at = self
                    .edit
                    .modules
                    .iter()
                    .rposition(|x| x.kind == kind)
                    .map_or(self.edit.modules.len(), |p| p + 1) as u32;
                self.commit(now_ms);
                let step = Step {
                    kind,
                    instance: n,
                    change: Change::Add {
                        at,
                        module: Box::new(m),
                    },
                    time_ms: now_ms,
                };
                if self.history.push(&mut self.edit, step).is_ok() {
                    self.masks.inst.insert(kind, n);
                    self.sync_panel();
                }
            }
            sub::INST_DEL => {
                let inst = self.inst(kind);
                let n = self.instance_number(kind, inst);
                self.masks.confirm = Some(Confirm::DeleteInstance(kind, inst));
                let mut q = t!(
                    "develop-instance-delete-confirm",
                    n = n as i64,
                    module = self.title_of(kind)
                );
                if let Some(u) = self.users_line(kind, inst) {
                    q = format!("{q}\n{u}");
                }
                return vec![Request::Confirm(q, t!("develop-instance-delete-ok"))];
            }
            sub::INST_EARLIER | sub::INST_LATER => {
                let inst = self.inst(kind);
                let pos: Vec<usize> = self
                    .edit
                    .modules
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.kind == kind)
                    .map(|(i, _)| i)
                    .collect();
                let Some(k) = pos
                    .iter()
                    .position(|&p| self.edit.modules[p].instance == inst)
                else {
                    return Vec::new();
                };
                let to = if s == sub::INST_EARLIER {
                    k.checked_sub(1)
                } else {
                    (k + 1 < pos.len()).then_some(k + 1)
                };
                if let Some(t) = to {
                    self.commit(now_ms);
                    let step = Step {
                        kind,
                        instance: inst,
                        change: Change::Move {
                            from: pos[k] as u32,
                            to: pos[t] as u32,
                            custom_before: self.edit.custom_order,
                        },
                        time_ms: now_ms,
                    };
                    if self.history.push(&mut self.edit, step).is_ok() {
                        self.sync_panel();
                    }
                }
            }
            s if s >= sub::LAYER0 => {
                let row = ((s - sub::LAYER0) / sub::PER) as usize;
                let fld = (s - sub::LAYER0) % sub::PER;
                return self.row_value(kind, row, fld, sel, val, done, now_ms);
            }
            _ => return Vec::new(),
        }
        out
    }

    /// "Curves 1 and Exposure 2 use this mask. They keep a copy of it.", when any do.
    fn users_line(&self, kind: ModuleKind, inst: u16) -> Option<String> {
        let users = self.edit.mask_users(kind, inst);
        if users.is_empty() {
            return None;
        }
        let names: Vec<String> = users
            .iter()
            .map(|&(k, i)| self.instance_title(k, i))
            .collect();
        Some(t!(
            "develop-mask-used-by",
            modules = specular_i18n::dot_join(&names),
            count = names.len() as i64
        ))
    }

    /// Adds a row borrowing `from`'s mask to `kind`'s.
    fn add_borrowed(
        &mut self,
        kind: ModuleKind,
        from: (ModuleKind, u16),
        now_ms: i64,
    ) -> Vec<Request> {
        let mut layers = self.layers_of(kind);
        if layers.len() >= MAX_ROWS {
            return vec![Request::Say(t!("develop-mask-full"))];
        }
        self.begin(kind);
        layers.push(Layer::new(MaskExpr::FromModule {
            kind: from.0,
            instance: from.1,
        }));
        self.set_layers(kind, &layers);
        self.commit(now_ms);
        let name = row_name(&layers, layers.len() - 1, &self.edit.masks);
        vec![Request::Say(t!("develop-mask-added", name = name))]
    }

    /// A brush or mask key; what to say.
    pub(super) fn mask_action(&mut self, a: specular_input::Action) -> String {
        use specular_input::Action as A;
        let b = &mut self.masks.brush;
        let said = match a {
            A::DevelopBrushSmaller | A::DevelopBrushLarger => {
                let k = if a == A::DevelopBrushLarger {
                    1.25
                } else {
                    0.8
                };
                b.size = (b.size * k).clamp(0.002, 0.25);
                t!(
                    "develop-brush-size-now",
                    percent = super::controls::percent_text(f64::from(b.size) * 100.0, 1)
                )
            }
            A::DevelopBrushSofter | A::DevelopBrushHarder => {
                let d = if a == A::DevelopBrushHarder {
                    0.1
                } else {
                    -0.1
                };
                b.hardness = ((b.hardness + d) * 10.0).round().clamp(0.0, 10.0) / 10.0;
                t!(
                    "develop-brush-feather-now",
                    percent = super::controls::percent_text(f64::from(1.0 - b.hardness) * 100.0, 0)
                )
            }
            A::DevelopBrushErase => {
                b.erase = !b.erase;
                if b.erase {
                    t!("develop-brush-erasing")
                } else {
                    t!("develop-brush-painting")
                }
            }
            _ => {
                if self.masks.overlay.take().is_some() {
                    t!("develop-mask-overlay-off")
                } else {
                    let target = self
                        .masks
                        .active
                        .map(|a| (a.kind, a.inst))
                        .filter(|&(k, i)| self.edit.module(k, i).is_some_and(|m| m.blend.is_some()))
                        .or_else(|| {
                            self.edit
                                .modules
                                .iter()
                                .find(|m| m.enabled && m.blend.is_some())
                                .map(|m| m.id())
                        });
                    match target {
                        Some(t) => {
                            self.masks.overlay = Some((t, Overlay::Tint));
                            t!(
                                "develop-mask-overlay-on",
                                module = self.instance_title(t.0, t.1)
                            )
                        }
                        None => t!("develop-mask-overlay-none"),
                    }
                }
            }
        };
        self.sync_panel();
        said
    }

    /// Shots: a new instance of `kind` borrowing instance 0's mask.
    #[cfg(test)]
    pub(super) fn borrow_into_new_instance(&mut self, kind: ModuleKind, now_ms: i64) {
        self.mask_value(mid(kind, sub::INST_NEW), true, now_ms);
        self.add_borrowed(kind, (kind, 0), now_ms);
    }

    /// Shots: every refine value set on `kind`'s mask.
    #[cfg(test)]
    pub(super) fn set_refine_for_shots(&mut self, kind: ModuleKind) {
        if let Some(b) = self.blend_mut(kind) {
            b.refine.edge_aware = 0.5;
            b.refine.radius = 0.03;
            b.refine.feather = 0.01;
            b.refine.blur = 0.005;
            b.refine.details = 0.2;
        }
        self.sync_panel();
    }

    /// "Exposure 2": a module instance as the panel names it.
    fn instance_title(&self, kind: ModuleKind, inst: u16) -> String {
        t!(
            "develop-mask-from-option",
            module = super::controls::module_title(kind),
            n = self.instance_number(kind, inst) as i64
        )
    }

    /// The masks `kind`'s card may borrow: other instances with a mask that doesn't
    /// borrow this one (no loops).
    fn borrow_candidates(&self, kind: ModuleKind) -> Vec<(ModuleKind, u16)> {
        let me = (kind, self.inst(kind));
        self.edit
            .modules
            .iter()
            .filter(|m| m.id() != me && m.blend.is_some() && MASKABLE.contains(&m.kind))
            .filter(|m| !reaches(&self.edit, m.id(), me, 0))
            .map(|m| m.id())
            .collect()
    }

    fn instance_number(&self, kind: ModuleKind, inst: u16) -> usize {
        self.edit
            .modules
            .iter()
            .filter(|m| m.kind == kind)
            .position(|m| m.instance == inst)
            .map_or(1, |p| p + 1)
    }

    fn title_of(&self, kind: ModuleKind) -> String {
        self.panel
            .cards
            .iter()
            .find(|c| c.id == u32::from(kind.0))
            .map(|c| c.title.clone())
            .unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    fn row_value(
        &mut self,
        kind: ModuleKind,
        row: usize,
        fld: u32,
        sel: usize,
        val: f32,
        done: bool,
        now_ms: i64,
    ) -> Vec<Request> {
        let mut layers = self.layers_of(kind);
        let Some(l) = layers.get(row).cloned() else {
            return Vec::new();
        };
        let inst = self.inst(kind);
        let here = Active { kind, inst, row };
        match fld {
            field::EDIT => {
                self.masks.active = Some(here);
                let name = row_name(&layers, row, &self.edit.masks);
                let brush = matches!(
                    row_shape(&l, &self.edit.masks).map(|s| s.1),
                    Some(Shape::Brush { .. })
                );
                return vec![
                    Request::Render,
                    Request::Say(if brush {
                        t!("develop-mask-painting", name = name)
                    } else {
                        t!("develop-mask-editing", name = name)
                    }),
                ];
            }
            field::REMOVE => {
                self.masks.confirm = Some(Confirm::RemoveRow(kind, inst, row));
                return vec![Request::Confirm(
                    t!(
                        "develop-mask-row-remove-confirm",
                        name = row_name(&layers, row, &self.edit.masks),
                        module = self.title_of(kind)
                    ),
                    t!("develop-mask-remove-ok"),
                )];
            }
            _ => {}
        }
        // AI rows: the link to Local AI and "Clear clicks".
        if let MaskExpr::Ai(_) = &l.item {
            match fld {
                field::B => return vec![Request::OpenLocalAi("ai.masks")],
                field::C => {
                    self.masks.active = Some(here);
                    return match self.ai_clear(now_ms) {
                        Some(said) => vec![Request::Render, Request::Say(said)],
                        None => Vec::new(),
                    };
                }
                field::A => return Vec::new(),
                _ => {}
            }
        }
        // Depth rows: the link to Local AI; the state line is not a value.
        if let MaskExpr::Depth(_) = &l.item {
            match fld {
                field::E => return vec![Request::OpenLocalAi("ai.depth")],
                field::D => return Vec::new(),
                _ => {}
            }
        }
        // Brush settings are not part of the edit.
        if let Some((_, Shape::Brush { .. })) = row_shape(&l, &self.edit.masks) {
            let b = &mut self.masks.brush;
            match fld {
                field::A => b.size = val,
                field::B => b.hardness = 1.0 - val,
                field::C => b.flow = val,
                field::D => b.erase = sel == 1,
                field::F => {
                    b.pressure = [PressureUse::Flow, PressureUse::Size, PressureUse::Both]
                        .get(sel)
                        .copied()
                        .unwrap_or_default();
                }
                field::G => b.smoothing = val,
                _ => {}
            }
            if matches!(
                fld,
                field::A | field::B | field::C | field::D | field::F | field::G
            ) {
                self.masks.active = Some(here);
                return Vec::new();
            }
        }
        if fld == field::E
            && let MaskExpr::ColourRange(_) = l.item
        {
            self.masks.picking = Some(here);
            return vec![
                Request::PickFor(kind),
                Request::Say(t!("develop-mask-picking")),
            ];
        }
        self.begin(kind);
        let l2 = &mut layers[row];
        match fld {
            field::COMBINE => l2.op = COMBINES.get(sel).copied().unwrap_or(Combine::Union),
            field::OPACITY => l2.opacity = val,
            field::INVERT => l2.invert = sel == 1,
            _ => {}
        }
        match &mut l2.item {
            MaskExpr::Range(r) => match fld {
                field::A..=field::D => {
                    let k = (fld - field::A) as usize;
                    r.limits[k] = val;
                    // Keep them in order around the one moved.
                    for j in 0..4 {
                        if j < k {
                            r.limits[j] = r.limits[j].min(val);
                        } else if j > k {
                            r.limits[j] = r.limits[j].max(val);
                        }
                    }
                }
                field::E => r.on_output = sel == 1,
                _ => {}
            },
            MaskExpr::ColourRange(c) => match fld {
                field::A => c.distance = val,
                field::B => c.falloff = val,
                _ => {}
            },
            MaskExpr::Details(d) => match fld {
                field::A => d.threshold = val,
                field::B => d.softness = val,
                _ => {}
            },
            MaskExpr::Depth(d) => match fld {
                field::A => d.near = val,
                field::B => d.far = val,
                field::C => d.feather = val,
                _ => {}
            },
            _ => {}
        }
        self.set_layers(kind, &layers);
        if let Some((id, _)) = row_shape(&layers[row], &self.edit.masks.clone())
            && let Some(ms) = self.edit.masks.iter_mut().find(|m| m.id == id)
        {
            match (&mut ms.shape, fld) {
                (Shape::Ellipse { feather, .. }, field::A) => *feather = val,
                (Shape::Rectangle { feather, .. }, field::A) => *feather = val,
                (Shape::Rectangle { round, .. }, field::B) => *round = val,
                (Shape::Polygon { points, .. }, field::A) => {
                    let one = self.masks.point.filter(|&p| {
                        p < points.len()
                            && self
                                .masks
                                .active
                                .is_some_and(|a| a.kind == kind && a.inst == inst && a.row == row)
                    });
                    for (k, p) in points.iter_mut().enumerate() {
                        if one.is_none_or(|o| o == k) {
                            p.feather = val;
                        }
                    }
                }
                _ => {}
            }
        }
        if done {
            self.commit(now_ms);
        }
        vec![Request::Render]
    }

    /// Adds a row of kind `k` to `kind`'s mask (giving the module a blend if it has none).
    pub(super) fn add_row(&mut self, kind: ModuleKind, k: NewKind, now_ms: i64) -> Vec<Request> {
        let mut layers = self.layers_of(kind);
        if layers.len() >= MAX_ROWS {
            return vec![Request::Say(t!("develop-mask-full"))];
        }
        self.begin(kind);
        let id = self.edit.next_mask_id();
        let shape = match k {
            NewKind::Ellipse => Some(Shape::Ellipse {
                centre: [0.5, 0.5],
                radii: [0.2, 0.15],
                angle_deg: 0.0,
                feather: 0.5,
            }),
            NewKind::Rectangle => Some(Shape::Rectangle {
                centre: [0.5, 0.5],
                half: [0.2, 0.12],
                angle_deg: 0.0,
                round: 0.2,
                feather: 0.3,
            }),
            NewKind::Polygon => Some(Shape::Polygon {
                points: [[0.35, 0.35], [0.65, 0.35], [0.65, 0.65], [0.35, 0.65]]
                    .iter()
                    .map(|p| PathPoint {
                        x: p[0],
                        y: p[1],
                        feather: 0.02,
                    })
                    .collect(),
                closed: true,
            }),
            NewKind::Linear => Some(Shape::Linear {
                start: [0.5, 0.3],
                end: [0.5, 0.7],
            }),
            NewKind::Radial => Some(Shape::Radial {
                centre: [0.5, 0.5],
                inner: [0.08, 0.08],
                outer: [0.3, 0.3],
                angle_deg: 0.0,
            }),
            NewKind::Brush => Some(Shape::Brush {
                strokes: Vec::new(),
            }),
            _ => None,
        };
        let item = match (shape, k) {
            (Some(shape), _) => {
                self.edit.masks.push(MaskShape { id, shape });
                MaskExpr::Shape(id)
            }
            (None, NewKind::Details) => MaskExpr::Details(Details::default()),
            (None, NewKind::Subject) => MaskExpr::Ai(self.new_ai_row(AiKind::Subject)),
            (None, NewKind::Sky) => MaskExpr::Ai(self.new_ai_row(AiKind::Sky)),
            (None, NewKind::Object) => MaskExpr::Ai(self.new_ai_row(AiKind::Object)),
            (None, NewKind::Near | NewKind::Far | NewKind::DepthBand) => {
                let model = super::ai_remove::model_id(specular_ai::Feature::Depth);
                MaskExpr::Depth(match k {
                    NewKind::Near => DepthRange::near(&model),
                    NewKind::Far => DepthRange::far(&model),
                    _ => DepthRange::band(&model),
                })
            }
            (None, NewKind::Colour) => MaskExpr::ColourRange(ColourRange {
                colours: Vec::new(),
                distance: 0.04,
                falloff: 0.06,
            }),
            (None, k) => {
                let (channel, limits) = match k {
                    NewKind::Hue => (RangeChannel::Hue, [20.0, 40.0, 60.0, 80.0]),
                    NewKind::Chroma => (RangeChannel::Chroma, [0.03, 0.06, 0.4, 0.4]),
                    _ => (RangeChannel::Luminance, [-3.0, -2.0, 0.0, 1.0]),
                };
                MaskExpr::Range(Range {
                    channel,
                    on_output: false,
                    limits,
                })
            }
        };
        layers.push(Layer::new(item));
        self.set_layers(kind, &layers);
        self.commit(now_ms);
        let row = layers.len() - 1;
        let inst = self.inst(kind);
        let name = row_name(&layers, row, &self.edit.masks);
        let mut out = vec![Request::Say(t!("develop-mask-added", name = name))];
        if matches!(k, NewKind::Colour) {
            self.masks.picking = Some(Active { kind, inst, row });
            out.push(Request::PickFor(kind));
        } else if !matches!(
            k,
            NewKind::Luminance
                | NewKind::Hue
                | NewKind::Chroma
                | NewKind::Details
                | NewKind::Subject
                | NewKind::Sky
                | NewKind::Near
                | NewKind::Far
                | NewKind::DepthBand
        ) {
            self.masks.active = Some(Active { kind, inst, row });
        }
        out
    }

    /// The user accepted a removal.
    pub(super) fn mask_confirmed(&mut self, now_ms: i64) -> Vec<Request> {
        let Some(c) = self.masks.confirm.take() else {
            return Vec::new();
        };
        self.commit(now_ms);
        match c {
            Confirm::DeleteMaskPreset(i) => vec![Request::DeleteMaskPreset(i)],
            Confirm::RemoveMask(kind, inst) => {
                self.masks.inst.insert(kind, inst);
                if self.remove_keeping_users(kind, inst, now_ms, false) {
                    return vec![Request::Render, Request::Say(t!("develop-mask-removed"))];
                }
                self.begin(kind);
                let mut gone = Vec::new();
                if let Some(b) = self.blend_mut(kind) {
                    b.mask.shape_ids(&mut gone);
                }
                if let Some(m) = self.edit.module_mut(kind, inst) {
                    m.blend = None;
                }
                self.drop_unused(&gone);
                self.masks.active = None;
                if self.masks.overlay.is_some_and(|(m, _)| m == (kind, inst)) {
                    self.masks.overlay = None;
                }
                self.commit(now_ms);
                vec![Request::Render, Request::Say(t!("develop-mask-removed"))]
            }
            Confirm::RemoveRow(kind, inst, row) => {
                self.masks.inst.insert(kind, inst);
                let mut layers = self.layers_of(kind);
                if row >= layers.len() {
                    return Vec::new();
                }
                self.begin(kind);
                let l = layers.remove(row);
                if row == 0
                    && let Some(f) = layers.first_mut()
                {
                    f.op = Combine::Union;
                }
                self.set_layers(kind, &layers);
                let mut gone = Vec::new();
                l.item.shape_ids(&mut gone);
                self.drop_unused(&gone);
                self.masks.active = None;
                self.commit(now_ms);
                vec![Request::Render, Request::Say(t!("develop-mask-removed"))]
            }
            Confirm::DeleteInstance(kind, inst) => {
                if self.remove_keeping_users(kind, inst, now_ms, true) {
                    self.after_instance_gone(kind);
                    return vec![
                        Request::Render,
                        Request::Say(t!("develop-instance-deleted")),
                    ];
                }
                let Some(at) = self.edit.position(kind, inst) else {
                    return Vec::new();
                };
                let module = Box::new(self.edit.modules[at].clone());
                let step = Step {
                    kind,
                    instance: inst,
                    change: Change::Remove {
                        at: at as u32,
                        module,
                    },
                    time_ms: now_ms,
                };
                if self.history.push(&mut self.edit, step).is_err() {
                    return Vec::new();
                }
                let first = self
                    .edit
                    .modules
                    .iter()
                    .find(|m| m.kind == kind)
                    .map_or(0, |m| m.instance);
                self.masks.inst.insert(kind, first);
                self.masks.active = None;
                self.sync_panel();
                vec![
                    Request::Render,
                    Request::Say(t!("develop-instance-deleted")),
                ]
            }
        }
    }

    /// Removes `kind`/`inst`'s mask (or, with `instance`, the whole instance) when other
    /// modules borrow it: they get a copy first, all in one history step. False (nothing
    /// done) when no module borrows it.
    fn remove_keeping_users(
        &mut self,
        kind: ModuleKind,
        inst: u16,
        now_ms: i64,
        instance: bool,
    ) -> bool {
        if self.edit.mask_users(kind, inst).is_empty() {
            return false;
        }
        let before = self.edit.clone();
        let mut after = before.clone();
        after.inline_mask_users(kind, inst);
        let mut gone = Vec::new();
        if let Some(b) = after.module(kind, inst).and_then(|m| m.blend.as_ref()) {
            b.mask.shape_ids(&mut gone);
        }
        if instance {
            after.modules.retain(|m| m.id() != (kind, inst));
        } else if let Some(m) = after.module_mut(kind, inst) {
            m.blend = None;
        }
        let mut used = Vec::new();
        for m in &after.modules {
            if let Some(b) = &m.blend {
                b.mask.shape_ids(&mut used);
            }
        }
        after
            .masks
            .retain(|m| !gone.contains(&m.id) || used.contains(&m.id));
        let what = if instance {
            t!(
                "develop-history-instance-delete",
                module = self.title_of(kind)
            )
        } else {
            t!("develop-history-mask", module = self.title_of(kind))
        };
        let step = Step::stack(&what, &before, &after, now_ms);
        if self.history.push(&mut self.edit, step).is_err() {
            return false;
        }
        self.masks.active = None;
        if self.masks.overlay.is_some_and(|(m, _)| m == (kind, inst)) {
            self.masks.overlay = None;
        }
        self.sync_panel();
        true
    }

    /// The card goes to the first instance left after one went.
    fn after_instance_gone(&mut self, kind: ModuleKind) {
        let first = self
            .edit
            .modules
            .iter()
            .find(|m| m.kind == kind)
            .map_or(0, |m| m.instance);
        self.masks.inst.insert(kind, first);
        self.masks.active = None;
        self.sync_panel();
    }

    /// Puts a mask preset on `kind`/`inst` (one history step).
    pub(super) fn apply_mask_preset(
        &mut self,
        kind: ModuleKind,
        inst: u16,
        p: &specular_develop::model::MaskPreset,
        orient: specular_develop::preview::Orient,
        now_ms: i64,
    ) -> bool {
        self.commit(now_ms);
        let before = self.edit.clone();
        let mut after = before.clone();
        if !p.apply(&mut after, kind, inst, orient) {
            return false;
        }
        let step = Step::stack(&p.name, &before, &after, now_ms);
        if self.history.push(&mut self.edit, step).is_err() {
            return false;
        }
        self.masks.active = None;
        self.sync_panel();
        true
    }

    /// Drops shapes no blend refers to any more.
    fn drop_unused(&mut self, ids: &[u32]) {
        let mut used = Vec::new();
        for m in &self.edit.modules {
            if let Some(b) = &m.blend {
                b.mask.shape_ids(&mut used);
            }
        }
        self.edit
            .masks
            .retain(|m| !ids.contains(&m.id) || used.contains(&m.id));
    }

    /// A colour picked for a colour range (module input, linear Rec.2020).
    pub(super) fn mask_picked(&mut self, rgb: [f64; 3], now_ms: i64) -> bool {
        let Some(a) = self.masks.picking.take() else {
            return false;
        };
        self.masks.inst.insert(a.kind, a.inst);
        let mut layers = self.layers_of(a.kind);
        let Some(MaskExpr::ColourRange(c)) = layers.get_mut(a.row).map(|l| &mut l.item) else {
            return false;
        };
        let lab = specular_develop::colour::oklab::from_rec2020_rel(rgb);
        c.colours.push(lab.map(|v| v as f32));
        self.begin(a.kind);
        self.set_layers(a.kind, &layers);
        self.commit(now_ms);
        true
    }

    /// The overlay to render.
    pub(super) fn overlay(&self) -> Option<((ModuleKind, u16), Overlay)> {
        self.masks.overlay
    }

    /// The shape of the row being edited.
    fn active_shape(&self) -> Option<(Active, u32, Shape)> {
        let a = self.masks.active?;
        if self.inst(a.kind) != a.inst {
            return None;
        }
        let layers = self.layers_of(a.kind);
        let (id, s) = row_shape(layers.get(a.row)?, &self.edit.masks)?;
        Some((a, id, s.clone()))
    }

    /// The module whose mask a stroke is painting (its drafts render while it goes on).
    pub(super) fn painted_module(&self) -> Option<ModuleKind> {
        self.masks
            .painting
            .then(|| self.active_shape().map(|(a, ..)| a.kind))
            .flatten()
    }

    /// The active row is a brush.
    pub(super) fn brush_active(&self) -> bool {
        matches!(self.active_shape(), Some((_, _, Shape::Brush { .. })))
    }

    /// Handles and outlines of the row being edited, on screen.
    pub(super) fn handles(&self, map: &Map) -> (Vec<Handle>, Vec<Outline>) {
        if let Some(nd) = self.nd_handles(map) {
            return nd;
        }
        if let Some(ai) = self.ai_handles(map) {
            return ai;
        }
        let Some((_, _, shape)) = self.active_shape() else {
            return (Vec::new(), Vec::new());
        };
        let sp = map.space();
        let hs = shape_handles(&shape, sp);
        let handles = hs
            .into_iter()
            .filter_map(|(role, p, name)| {
                map.to_screen(sp.to_frame(p))
                    .map(|at| Handle { role, at, name })
            })
            .collect();
        let outlines = shape_outlines(&shape, sp)
            .into_iter()
            .map(|(pts, closed, faint)| Outline {
                points: pts
                    .into_iter()
                    .filter_map(|p| map.to_screen(sp.to_frame(p)))
                    .collect(),
                closed,
                faint,
            })
            .collect();
        (handles, outlines)
    }

    /// Moves handle `i` of the row being edited to screen point `p` (a drag in progress).
    pub(super) fn move_handle(&mut self, map: &Map, i: usize, p: (f32, f32)) -> bool {
        if self.nd_edit && self.masks.active.is_none() {
            return self.nd_move(map, i, p);
        }
        let Some((a, id, mut shape)) = self.active_shape() else {
            return false;
        };
        let sp = map.space();
        let f = map.to_frame(p);
        let s = sp.of_frame(f);
        if !move_shape(&mut shape, sp, i, s) {
            return false;
        }
        self.begin(a.kind);
        if let Some(m) = self.edit.masks.iter_mut().find(|m| m.id == id) {
            m.shape = shape;
        }
        true
    }

    /// The polygon row being edited: its shape id and points.
    fn active_polygon(&self) -> Option<(Active, u32, Vec<PathPoint>)> {
        match self.active_shape()? {
            (a, id, Shape::Polygon { points, .. }) => Some((a, id, points)),
            _ => None,
        }
    }

    /// Handle `h` of the row being edited was chosen (pointer or Tab): a polygon point
    /// becomes the one Delete and the feather slider act on.
    pub(super) fn choose_handle(&mut self, h: Option<usize>) {
        let point = match (self.active_polygon(), h) {
            (Some(_), Some(h)) => h.checked_sub(1),
            _ => None,
        };
        if point != self.masks.point {
            self.masks.point = point;
            self.sync_panel();
        }
    }

    /// Sets the polygon's points (one history step).
    fn set_points(&mut self, a: Active, id: u32, points: Vec<PathPoint>, now_ms: i64) {
        self.begin(a.kind);
        if let Some(Shape::Polygon { points: p, .. }) = self
            .edit
            .masks
            .iter_mut()
            .find(|m| m.id == id)
            .map(|m| &mut m.shape)
        {
            *p = points;
        }
        self.commit(now_ms);
    }

    /// A press at screen point `p` on an edge of the polygon being edited (within `tol`
    /// px) adds a point there, its feather between its neighbours'. What to say.
    pub(super) fn add_point_at(
        &mut self,
        map: &Map,
        p: (f32, f32),
        tol: f32,
        now_ms: i64,
    ) -> Option<String> {
        let (a, id, mut pts) = self.active_polygon()?;
        let screen: Vec<Option<(f32, f32)>> = pts
            .iter()
            .map(|q| map.to_screen((f64::from(q.x), f64::from(q.y))))
            .collect();
        let n = pts.len();
        let mut best: Option<(f32, usize, f32)> = None;
        for i in 0..n {
            let (Some(s), Some(e)) = (screen[i], screen[(i + 1) % n]) else {
                continue;
            };
            let d = (e.0 - s.0, e.1 - s.1);
            let l2 = (d.0 * d.0 + d.1 * d.1).max(1e-6);
            let t = (((p.0 - s.0) * d.0 + (p.1 - s.1) * d.1) / l2).clamp(0.0, 1.0);
            let q = (s.0 + t * d.0, s.1 + t * d.1);
            let dist = (p.0 - q.0).hypot(p.1 - q.1);
            if dist <= tol && best.is_none_or(|b| dist < b.0) {
                best = Some((dist, i, t));
            }
        }
        let (_, i, t) = best?;
        let (fx, fy) = map.to_frame(p);
        let (s, e) = (pts[i], pts[(i + 1) % n]);
        pts.insert(
            i + 1,
            PathPoint {
                x: fx as f32,
                y: fy as f32,
                feather: s.feather + (e.feather - s.feather) * t,
            },
        );
        self.set_points(a, id, pts, now_ms);
        self.masks.point = Some(i + 1);
        self.sync_panel();
        Some(t!("develop-mask-point-added", n = i as i64 + 2))
    }

    /// Adds a point halfway after the chosen one (the keyboard's way). What to say.
    pub(super) fn insert_point(&mut self, now_ms: i64) -> Option<String> {
        let (a, id, mut pts) = self.active_polygon()?;
        let Some(i) = self.masks.point.filter(|&i| i < pts.len()) else {
            return Some(t!("develop-mask-point-choose"));
        };
        let (s, e) = (pts[i], pts[(i + 1) % pts.len()]);
        pts.insert(
            i + 1,
            PathPoint {
                x: (s.x + e.x) / 2.0,
                y: (s.y + e.y) / 2.0,
                feather: (s.feather + e.feather) / 2.0,
            },
        );
        self.set_points(a, id, pts, now_ms);
        self.masks.point = Some(i + 1);
        self.sync_panel();
        Some(t!("develop-mask-point-added", n = i as i64 + 2))
    }

    /// Alt+click on handle `h`: when it is a point of the polygon being edited, that point
    /// goes (a polygon keeps three). What to say; None when `h` is no point.
    pub(super) fn remove_point_at(&mut self, h: usize, now_ms: i64) -> Option<String> {
        self.active_polygon()?;
        h.checked_sub(1)?;
        self.choose_handle(Some(h));
        self.remove_point(now_ms)
    }

    /// Removes the chosen point (a polygon keeps three). What to say.
    pub(super) fn remove_point(&mut self, now_ms: i64) -> Option<String> {
        let (a, id, mut pts) = self.active_polygon()?;
        let Some(i) = self.masks.point.filter(|&i| i < pts.len()) else {
            return Some(t!("develop-mask-point-choose"));
        };
        if pts.len() <= 3 {
            return Some(t!("develop-mask-point-last"));
        }
        pts.remove(i);
        self.set_points(a, id, pts, now_ms);
        self.masks.point = None;
        self.sync_panel();
        Some(t!("develop-mask-point-removed", n = i as i64 + 1))
    }

    /// Paints with the brush at screen point `p`: a new stroke when `start`.
    pub(super) fn paint(&mut self, map: &Map, p: (f32, f32), start: bool) -> bool {
        let Some((a, id, _)) = self.active_shape() else {
            return false;
        };
        let (fx, fy) = map.to_frame(p);
        if !(fx.is_finite() && fy.is_finite()) {
            return false;
        }
        self.begin(a.kind);
        let b = self.masks.brush;
        // Smoothing: the brush trails the pointer (in frame units, the same at any zoom).
        let prev = if start { None } else { self.masks.smoothed };
        let (fx, fy) = smooth(prev, (fx, fy), b.smoothing);
        self.masks.smoothed = Some((fx, fy));
        let pressure = self.masks.pressure.unwrap_or(1.0).clamp(0.0, 1.0);
        let Some(Shape::Brush { strokes }) = self
            .edit
            .masks
            .iter_mut()
            .find(|m| m.id == id)
            .map(|m| &mut m.shape)
        else {
            return false;
        };
        let pt = [fx as f32, fy as f32, pressure];
        if start || strokes.is_empty() {
            strokes.push(Stroke {
                radius: b.size,
                hardness: b.hardness,
                flow: b.flow,
                erase: b.erase,
                pressure: b.pressure,
                points: vec![pt],
            });
        } else if let Some(s) = strokes.last_mut() {
            // Points closer than a tenth of the radius add nothing.
            if let Some(l) = s.points.last() {
                let d = map.space().dist([l[0], l[1]], [pt[0], pt[1]]);
                if d < f64::from(s.radius) * 0.1 {
                    return false;
                }
            }
            s.points.push(pt);
        }
        true
    }
}

/// The next brush position: `p` itself at the start of a stroke, else `prev` moved towards
/// `p` by `1 − k` (`k` the smoothing, 0..0.9).
pub(super) fn smooth(prev: Option<(f64, f64)>, p: (f64, f64), k: f32) -> (f64, f64) {
    let k = if k.is_finite() {
        f64::from(k).clamp(0.0, 0.9)
    } else {
        0.0
    };
    match prev {
        Some(q) => (q.0 + (p.0 - q.0) * (1.0 - k), q.1 + (p.1 - q.1) * (1.0 - k)),
        None => p,
    }
}

/// The picture on screen and the warp: maps source-frame points and screen points.
pub(super) struct Map {
    pub plan: Plan,
    /// The developed picture on screen (logical px).
    pub shown: Rect,
}

impl Map {
    pub fn space(&self) -> Space {
        Space::of(self.plan.frame[2], self.plan.frame[3])
    }

    /// The source-frame point (fractions) under screen point `p`.
    pub fn to_frame(&self, p: (f32, f32)) -> (f64, f64) {
        let u = f64::from((p.0 - self.shown.x) / self.shown.w.max(1.0));
        let v = f64::from((p.1 - self.shown.y) / self.shown.h.max(1.0));
        Geometry::Warp(&self.plan).frame_point(u, v)
    }

    /// The screen point of source-frame point `f`.
    pub fn to_screen(&self, f: (f64, f64)) -> Option<(f32, f32)> {
        let [fx, fy, fw, fh] = self.plan.frame;
        let o = self.plan.to_output((fx + f.0 * fw, fy + f.1 * fh), 1.0)?;
        let c = self.plan.crop_rect();
        Some((
            self.shown.x + (o.0 / c[2].max(1e-9)) as f32 * self.shown.w,
            self.shown.y + (o.1 / c[3].max(1e-9)) as f32 * self.shown.h,
        ))
    }

    /// Screen px per unit of the frame's long side.
    pub fn px_per_long(&self) -> f32 {
        let c = self.plan.crop_rect();
        let l = self.plan.frame[2].max(self.plan.frame[3]);
        (f64::from(self.shown.w) / c[2].max(1e-9) * l) as f32
    }
}

/// Shape-space helpers.
trait SpaceExt {
    fn to_frame(&self, s: (f64, f64)) -> (f64, f64);
    fn of_frame(&self, f: (f64, f64)) -> (f64, f64);
    fn dist(&self, a: [f32; 2], b: [f32; 2]) -> f64;
}

impl SpaceExt for Space {
    fn to_frame(&self, s: (f64, f64)) -> (f64, f64) {
        (s.0 / self.ax, s.1 / self.ay)
    }
    fn of_frame(&self, f: (f64, f64)) -> (f64, f64) {
        (f.0 * self.ax, f.1 * self.ay)
    }
    fn dist(&self, a: [f32; 2], b: [f32; 2]) -> f64 {
        let (p, q) = (self.pos(a), self.pos(b));
        (p.0 - q.0).hypot(p.1 - q.1)
    }
}

/// Local (shape-axis) offset `q` of a shape turned by `deg` at `c`, in shape space.
fn world(c: (f64, f64), deg: f32, q: (f64, f64)) -> (f64, f64) {
    let (sn, cs) = f64::from(deg).to_radians().sin_cos();
    (c.0 + q.0 * cs - q.1 * sn, c.1 + q.0 * sn + q.1 * cs)
}

fn local(c: (f64, f64), deg: f32, p: (f64, f64)) -> (f64, f64) {
    let (sn, cs) = f64::from(deg).to_radians().sin_cos();
    let d = (p.0 - c.0, p.1 - c.1);
    (d.0 * cs + d.1 * sn, -d.0 * sn + d.1 * cs)
}

fn unpos(sp: Space, s: (f64, f64)) -> [f32; 2] {
    let f = sp.to_frame(s);
    [f.0 as f32, f.1 as f32]
}

/// Handles of a shape in shape space: role, point, name.
fn shape_handles(shape: &Shape, sp: Space) -> Vec<(Role, (f64, f64), String)> {
    let centre = || t!("develop-handle-centre");
    let width = || t!("develop-handle-width");
    let height = || t!("develop-handle-height");
    let rotate = || t!("develop-handle-rotate");
    let feather = || t!("develop-handle-feather");
    match shape {
        Shape::Ellipse {
            centre: c,
            radii,
            angle_deg: a,
            feather: f,
        } => {
            let c = sp.pos(*c);
            let (rx, ry) = (f64::from(radii[0]), f64::from(radii[1]));
            vec![
                (Role::Centre, c, centre()),
                (Role::Size, world(c, *a, (rx, 0.0)), width()),
                (Role::Size, world(c, *a, (0.0, ry)), height()),
                (Role::Rotate, world(c, *a, (0.0, -ry * 1.25)), rotate()),
                (
                    Role::Feather,
                    world(c, *a, (-rx * (1.0 - f64::from(*f)), 0.0)),
                    feather(),
                ),
            ]
        }
        Shape::Rectangle {
            centre: c,
            half,
            angle_deg: a,
            feather: f,
            ..
        } => {
            let c = sp.pos(*c);
            let (hx, hy) = (f64::from(half[0]), f64::from(half[1]));
            vec![
                (Role::Centre, c, centre()),
                (Role::Size, world(c, *a, (hx, 0.0)), width()),
                (Role::Size, world(c, *a, (0.0, hy)), height()),
                (Role::Rotate, world(c, *a, (0.0, -hy * 1.25)), rotate()),
                (
                    Role::Feather,
                    world(c, *a, (-hx * (1.0 - f64::from(*f)), 0.0)),
                    feather(),
                ),
            ]
        }
        Shape::Polygon { points, .. } => {
            let n = points.len().max(1) as f64;
            let (sx, sy) = points.iter().fold((0.0, 0.0), |a, p| {
                let q = sp.pos([p.x, p.y]);
                (a.0 + q.0, a.1 + q.1)
            });
            let mut v = vec![(Role::Centre, (sx / n, sy / n), centre())];
            for (i, p) in points.iter().enumerate() {
                v.push((
                    Role::Point,
                    sp.pos([p.x, p.y]),
                    t!("develop-handle-point", n = i as i64 + 1),
                ));
            }
            v
        }
        Shape::Linear { start, end } => {
            let (s, e) = (sp.pos(*start), sp.pos(*end));
            vec![
                (
                    Role::Centre,
                    ((s.0 + e.0) / 2.0, (s.1 + e.1) / 2.0),
                    centre(),
                ),
                (Role::Size, s, t!("develop-handle-start")),
                (Role::Size, e, t!("develop-handle-end")),
            ]
        }
        Shape::Radial {
            centre: c,
            inner,
            outer,
            angle_deg: a,
        } => {
            let c = sp.pos(*c);
            let (ox, oy) = (f64::from(outer[0]), f64::from(outer[1]));
            vec![
                (Role::Centre, c, centre()),
                (Role::Size, world(c, *a, (ox, 0.0)), width()),
                (Role::Size, world(c, *a, (0.0, oy)), height()),
                (Role::Rotate, world(c, *a, (0.0, -oy * 1.25)), rotate()),
                (
                    Role::Feather,
                    world(c, *a, (-f64::from(inner[0]), 0.0)),
                    t!("develop-handle-inner"),
                ),
            ]
        }
        Shape::Brush { .. } => Vec::new(),
    }
}

type Line = (Vec<(f64, f64)>, bool, bool);

fn ellipse_pts(c: (f64, f64), a: f32, rx: f64, ry: f64) -> Vec<(f64, f64)> {
    (0..64)
        .map(|i| {
            let t = i as f64 * std::f64::consts::TAU / 64.0;
            world(c, a, (rx * t.cos(), ry * t.sin()))
        })
        .collect()
}

fn rect_pts(c: (f64, f64), a: f32, hx: f64, hy: f64) -> Vec<(f64, f64)> {
    // Sides subdivided, so lens distortion bends them on screen.
    let corners = [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)];
    let mut v = Vec::new();
    for k in 0..4 {
        let (p, q) = (corners[k], corners[(k + 1) % 4]);
        for i in 0..16 {
            let t = i as f64 / 16.0;
            v.push(world(c, a, (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)));
        }
    }
    v
}

fn shape_outlines(shape: &Shape, sp: Space) -> Vec<Line> {
    match shape {
        Shape::Ellipse {
            centre,
            radii,
            angle_deg: a,
            feather,
        } => {
            let c = sp.pos(*centre);
            let (rx, ry) = (f64::from(radii[0]), f64::from(radii[1]));
            let k = 1.0 - f64::from(*feather);
            vec![
                (ellipse_pts(c, *a, rx, ry), true, false),
                (ellipse_pts(c, *a, rx * k, ry * k), true, true),
            ]
        }
        Shape::Rectangle {
            centre,
            half,
            angle_deg: a,
            feather,
            ..
        } => {
            let c = sp.pos(*centre);
            let (hx, hy) = (f64::from(half[0]), f64::from(half[1]));
            let w = f64::from(*feather) * hx.min(hy);
            vec![
                (rect_pts(c, *a, hx, hy), true, false),
                (
                    rect_pts(c, *a, (hx - w).max(0.0), (hy - w).max(0.0)),
                    true,
                    true,
                ),
            ]
        }
        Shape::Polygon { points, .. } => {
            vec![(
                points.iter().map(|p| sp.pos([p.x, p.y])).collect(),
                true,
                false,
            )]
        }
        Shape::Linear { start, end } => {
            let (s, e) = (sp.pos(*start), sp.pos(*end));
            let d = (e.0 - s.0, e.1 - s.1);
            let n = d.0.hypot(d.1).max(1e-9);
            let perp = (-d.1 / n * 0.5, d.0 / n * 0.5);
            let line = |p: (f64, f64)| -> Vec<(f64, f64)> {
                (0..=16)
                    .map(|i| {
                        let t = i as f64 / 8.0 - 1.0;
                        (p.0 + perp.0 * t, p.1 + perp.1 * t)
                    })
                    .collect()
            };
            vec![(line(s), false, false), (line(e), false, true)]
        }
        Shape::Radial {
            centre,
            inner,
            outer,
            angle_deg: a,
        } => {
            let c = sp.pos(*centre);
            vec![
                (
                    ellipse_pts(c, *a, f64::from(outer[0]), f64::from(outer[1])),
                    true,
                    false,
                ),
                (
                    ellipse_pts(c, *a, f64::from(inner[0]), f64::from(inner[1])),
                    true,
                    true,
                ),
            ]
        }
        Shape::Brush { .. } => Vec::new(),
    }
}

/// Moves handle `i` of `shape` to shape-space point `p`; whether it moved.
fn move_shape(shape: &mut Shape, sp: Space, i: usize, p: (f64, f64)) -> bool {
    const MIN: f64 = 0.003;
    let angle = |c: (f64, f64)| ((p.1 - c.1).atan2(p.0 - c.0).to_degrees() + 90.0) as f32;
    match shape {
        Shape::Ellipse {
            centre,
            radii,
            angle_deg,
            feather,
        }
        | Shape::Rectangle {
            centre,
            half: radii,
            angle_deg,
            feather,
            ..
        } => {
            let c = sp.pos(*centre);
            let q = local(c, *angle_deg, p);
            match i {
                0 => *centre = unpos(sp, p),
                1 => radii[0] = q.0.abs().max(MIN) as f32,
                2 => radii[1] = q.1.abs().max(MIN) as f32,
                3 => *angle_deg = norm_deg(angle(c)),
                4 => {
                    *feather =
                        (1.0 - q.0.abs() / f64::from(radii[0]).max(MIN)).clamp(0.0, 1.0) as f32
                }
                _ => return false,
            }
        }
        Shape::Polygon { points, .. } => {
            if i == 0 {
                let n = points.len().max(1) as f64;
                let (sx, sy) = points.iter().fold((0.0, 0.0), |a, q| {
                    let q = sp.pos([q.x, q.y]);
                    (a.0 + q.0, a.1 + q.1)
                });
                let d = (p.0 - sx / n, p.1 - sy / n);
                for q in points.iter_mut() {
                    let s = sp.pos([q.x, q.y]);
                    let f = unpos(sp, (s.0 + d.0, s.1 + d.1));
                    q.x = f[0];
                    q.y = f[1];
                }
            } else if let Some(q) = points.get_mut(i - 1) {
                let f = unpos(sp, p);
                q.x = f[0];
                q.y = f[1];
            } else {
                return false;
            }
        }
        Shape::Linear { start, end } => match i {
            0 => {
                let (s, e) = (sp.pos(*start), sp.pos(*end));
                let m = ((s.0 + e.0) / 2.0, (s.1 + e.1) / 2.0);
                let d = (p.0 - m.0, p.1 - m.1);
                *start = unpos(sp, (s.0 + d.0, s.1 + d.1));
                *end = unpos(sp, (e.0 + d.0, e.1 + d.1));
            }
            1 => *start = unpos(sp, p),
            2 => *end = unpos(sp, p),
            _ => return false,
        },
        Shape::Radial {
            centre,
            inner,
            outer,
            angle_deg,
        } => {
            let c = sp.pos(*centre);
            let q = local(c, *angle_deg, p);
            let ratio = |inner: &[f32; 2], outer: &[f32; 2]| {
                (f64::from(inner[0]) / f64::from(outer[0]).max(MIN)).clamp(0.0, 1.0)
            };
            match i {
                0 => *centre = unpos(sp, p),
                1 | 2 => {
                    let k = ratio(inner, outer);
                    let j = i - 1;
                    outer[j] = (if j == 0 { q.0 } else { q.1 }).abs().max(MIN) as f32;
                    inner[0] = (f64::from(outer[0]) * k) as f32;
                    inner[1] = (f64::from(outer[1]) * k) as f32;
                }
                3 => *angle_deg = norm_deg(angle(c)),
                4 => {
                    let k = (q.0.abs() / f64::from(outer[0]).max(MIN)).clamp(0.0, 0.99);
                    inner[0] = (f64::from(outer[0]) * k) as f32;
                    inner[1] = (f64::from(outer[1]) * k) as f32;
                }
                _ => return false,
            }
        }
        Shape::Brush { .. } => return false,
    }
    true
}

fn norm_deg(a: f32) -> f32 {
    let a = a.rem_euclid(360.0);
    if a > 180.0 { a - 360.0 } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::ops::geometry::Context;
    use specular_develop::preview::Orient;

    fn map() -> Map {
        let e = super::super::controls::new_edit(std::path::Path::new("a.jpg"));
        let ctx = Context::image(600, 400, Orient::default());
        Map {
            plan: Plan::new(&e, &ctx, true),
            shown: Rect::new(10.0, 20.0, 300.0, 200.0),
        }
    }

    #[test]
    fn screen_and_frame_round_trip() {
        let m = map();
        let f = m.to_frame((160.0, 70.0));
        assert!(
            (f.0 - 0.5).abs() < 1e-6 && (f.1 - 0.25).abs() < 1e-6,
            "{f:?}"
        );
        let s = m.to_screen(f).expect("inverse");
        assert!((s.0 - 160.0).abs() < 1e-3 && (s.1 - 70.0).abs() < 1e-3);
    }

    #[test]
    fn handles_move_their_shapes() {
        let sp = Space::of(600.0, 400.0);
        let mut s = Shape::Ellipse {
            centre: [0.5, 0.5],
            radii: [0.2, 0.1],
            angle_deg: 0.0,
            feather: 0.5,
        };
        let hs = shape_handles(&s, sp);
        assert_eq!(hs.len(), 5);
        // Dragging each handle onto itself changes nothing.
        for (i, h) in hs.iter().enumerate() {
            let before = s.clone();
            assert!(move_shape(&mut s, sp, i, h.1));
            match (&before, &s) {
                (
                    Shape::Ellipse {
                        centre: a,
                        radii: r,
                        angle_deg: g,
                        feather: f,
                    },
                    Shape::Ellipse {
                        centre: b,
                        radii: q,
                        angle_deg: h2,
                        feather: f2,
                    },
                ) => {
                    assert!((a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5);
                    assert!((r[0] - q[0]).abs() < 1e-5 && (r[1] - q[1]).abs() < 1e-5);
                    assert!((g - h2).abs() < 1e-3 && (f - f2).abs() < 1e-5, "{i}");
                }
                _ => unreachable!(),
            }
        }
        // The rotation handle turns it.
        let c = sp.pos([0.5, 0.5]);
        assert!(move_shape(&mut s, sp, 3, (c.0 + 0.1, c.1)));
        let Shape::Ellipse { angle_deg, .. } = s else {
            unreachable!()
        };
        assert!((angle_deg - 90.0).abs() < 1e-3, "{angle_deg}");
    }

    #[test]
    fn rows_add_combine_and_remove_with_history() {
        let mut s = Session::new(
            std::path::PathBuf::from("a.jpg"),
            super::super::controls::new_edit(std::path::Path::new("a.jpg")),
            Default::default(),
        );
        let k = ModuleKind::EXPOSURE;
        s.add_row(k, NewKind::Ellipse, 0);
        s.add_row(k, NewKind::Luminance, 10_000);
        let layers = s.layers_of(k);
        assert_eq!(layers.len(), 2);
        assert_eq!(s.edit.masks.len(), 1);
        // The section lists both rows.
        let ctl = s.mask_controls(k);
        assert!(
            ctl.iter()
                .any(|c| c.id == mid(k, sub::LAYER0 + sub::PER + field::COMBINE))
        );
        // Subtract the range.
        if let Some(Body::Choice(c)) = s
            .panel
            .control_mut(mid(k, sub::LAYER0 + sub::PER + field::COMBINE))
            .map(|c| &mut c.body)
        {
            c.set(1);
        }
        s.mask_value(
            mid(k, sub::LAYER0 + sub::PER + field::COMBINE),
            true,
            20_000,
        );
        assert_eq!(s.layers_of(k)[1].op, Combine::Subtract);
        // Removing asks first, then drops the shape too.
        let r = s.mask_value(mid(k, sub::LAYER0 + field::REMOVE), true, 30_000);
        assert!(matches!(r.first(), Some(Request::Confirm(..))));
        assert_eq!(s.layers_of(k).len(), 2);
        s.mask_confirmed(30_000);
        assert_eq!(s.layers_of(k).len(), 1);
        assert!(s.edit.masks.is_empty());
        // Undo brings it back.
        s.undo();
        s.undo();
        assert_eq!(s.layers_of(k).len(), 2);
    }

    #[test]
    fn instances_add_duplicate_move_and_delete() {
        let mut s = Session::new(
            std::path::PathBuf::from("a.jpg"),
            super::super::controls::new_edit(std::path::Path::new("a.jpg")),
            Default::default(),
        );
        let k = ModuleKind::EXPOSURE;
        s.mask_value(mid(k, sub::INST_DUP), true, 0);
        assert_eq!(s.inst(k), 1);
        assert_eq!(s.edit.modules.iter().filter(|m| m.kind == k).count(), 2);
        s.mask_value(mid(k, sub::INST_EARLIER), true, 0);
        let order: Vec<u16> = s
            .edit
            .modules
            .iter()
            .filter(|m| m.kind == k)
            .map(|m| m.instance)
            .collect();
        assert_eq!(order, vec![1, 0]);
        assert!(s.edit.in_recommended_order(Registry::builtin()));
        let r = s.mask_value(mid(k, sub::INST_DEL), true, 0);
        assert!(matches!(r.first(), Some(Request::Confirm(..))));
        s.mask_confirmed(0);
        assert_eq!(s.edit.modules.iter().filter(|m| m.kind == k).count(), 1);
    }

    /// The card menu's rows: "Add mask ›" leads to the kinds and adding one opens the card;
    /// "Mask preset ›" and "Save mask as preset…" come when they have something to offer;
    /// the instance rows follow the instances; the card body has none of these.
    #[test]
    fn card_menu_adds_masks_and_instances() {
        let mut s = session();
        let k = ModuleKind::EXPOSURE;
        let card = u32::from(k.0);
        let acts = |s: &Session, g: usize| {
            s.mask_menu(k).groups[g]
                .iter()
                .map(|r| r.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(acts(&s, 0), vec![MaskAct::AddMenu]);
        assert_eq!(acts(&s, 1), vec![MaskAct::InstNew, MaskAct::InstDup]);
        let menu = s.mask_menu(k);
        assert_eq!(menu.subs.len(), 1);
        assert_eq!(menu.subs[0].0, MaskAct::AddMenu);
        assert_eq!(menu.subs[0].1.len(), NEW_KINDS.len());
        s.panel.card_mut(card).expect("Exposure's card").expanded = false;
        s.mask_act(k, MaskAct::Add(0), 0);
        assert_eq!(s.layers_of(k).len(), 1);
        assert!(s.panel.cards.iter().any(|c| c.id == card && c.expanded));
        // With a mask it can be saved; with presets, their submenu (put on, delete).
        s.masks.preset_names = vec!["Sky".into()];
        assert_eq!(
            acts(&s, 0),
            vec![MaskAct::AddMenu, MaskAct::PresetMenu, MaskAct::PresetSave]
        );
        let presets = s
            .mask_menu(k)
            .subs
            .into_iter()
            .find(|x| x.0 == MaskAct::PresetMenu)
            .expect("the presets' submenu");
        let rows: Vec<MaskAct> = presets.1.iter().map(|r| r.0).collect();
        assert_eq!(rows, vec![MaskAct::Preset(0), MaskAct::PresetDelete(0)]);
        let r = s.mask_act(k, MaskAct::PresetDelete(0), 500);
        assert!(matches!(r.first(), Some(Request::Confirm(..))));
        s.mask_act(k, MaskAct::InstDup, 1_000);
        assert_eq!(s.inst(k), 1);
        assert_eq!(
            acts(&s, 1),
            vec![
                MaskAct::InstNew,
                MaskAct::InstDup,
                MaskAct::InstEarlier,
                MaskAct::InstDel
            ]
        );
        assert!(s.mask_menu(ModuleKind::CROP_ORIENTATION).groups.is_empty());
        let body = s.mask_controls(k);
        for gone in [sub::INST_NEW, sub::INST_DUP, sub::INST_DEL] {
            assert!(!body.iter().any(|c| c.id == mid(k, gone)), "{gone}");
        }
    }

    fn session() -> Session {
        Session::new(
            std::path::PathBuf::from("a.jpg"),
            super::super::controls::new_edit(std::path::Path::new("a.jpg")),
            Default::default(),
        )
    }

    /// A borrowed row, and deleting its source asks, names the user and copies the
    /// mask in (one history step).
    #[test]
    fn a_borrowed_mask_is_copied_in_when_its_module_goes() {
        let mut s = session();
        let k = ModuleKind::EXPOSURE;
        s.add_row(k, NewKind::Ellipse, 0);
        // A second instance borrows the first's mask.
        s.mask_value(mid(k, sub::INST_NEW), true, 1_000);
        let user = s.inst(k);
        assert_ne!(user, 0);
        assert_eq!(s.borrow_candidates(k), vec![(k, 0)]);
        let from = s.mask_menu(k);
        assert!(from.groups[0].iter().any(|r| r.0 == MaskAct::FromMenu));
        assert!(from.subs.iter().any(|(p, rows)| {
            *p == MaskAct::FromMenu && rows.iter().any(|r| r.0 == MaskAct::From(0))
        }));
        s.add_borrowed(k, (k, 0), 2_000);
        let layers = s.layers_of(k);
        assert_eq!(
            layers[0].item,
            MaskExpr::FromModule {
                kind: k,
                instance: 0
            }
        );
        assert!(row_name(&layers, 0, &s.edit.masks).contains('1'));
        // The source can't borrow back (no loop).
        s.masks.inst.insert(k, 0);
        assert!(s.borrow_candidates(k).is_empty());
        // Delete the source: asks, naming the user.
        let r = s.mask_value(mid(k, sub::INST_DEL), true, 3_000);
        let Some(Request::Confirm(q, _)) = r.first() else {
            panic!("asks first");
        };
        assert!(q.lines().count() >= 2, "{q}");
        let steps = s.history.steps.len();
        s.mask_confirmed(3_000);
        assert_eq!(s.history.steps.len(), steps + 1, "one step");
        let b = s
            .edit
            .module(k, user)
            .and_then(|m| m.blend.clone())
            .expect("b");
        assert!(matches!(b.mask, MaskExpr::Shape(_)), "{:?}", b.mask);
        assert_eq!(s.edit.masks.len(), 1, "the shape stays for the user");
        s.undo();
        assert!(s.edit.module(k, 0).is_some());
    }

    #[test]
    fn polygon_points_are_added_chosen_and_removed() {
        let mut s = session();
        let m = map();
        let k = ModuleKind::EXPOSURE;
        s.add_row(k, NewKind::Polygon, 0);
        let pts = |s: &Session| match s.active_shape() {
            Some((_, _, Shape::Polygon { points, .. })) => points,
            _ => Vec::new(),
        };
        assert_eq!(pts(&s).len(), 4);
        // The middle of the first edge, on screen.
        let p = pts(&s);
        let a = m
            .to_screen((f64::from(p[0].x), f64::from(p[0].y)))
            .expect("a");
        let b = m
            .to_screen((f64::from(p[1].x), f64::from(p[1].y)))
            .expect("b");
        let mid_pt = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0 + 2.0);
        // Inside, away from every edge: no point.
        let inside = ((a.0 + b.0) / 2.0, a.1 + 30.0);
        assert!(s.add_point_at(&m, inside, 6.0, 1_000).is_none());
        assert!(s.add_point_at(&m, mid_pt, 6.0, 1_000).is_some());
        assert_eq!(pts(&s).len(), 5);
        assert_eq!(s.masks.point, Some(1));
        // Its own feather.
        let id = mid(k, sub::LAYER0 + field::A);
        if let Some(Body::Slider(sl)) = s.panel.control_mut(id).map(|c| &mut c.body) {
            sl.set(0.1);
        }
        s.mask_value(id, true, 2_000);
        let p = pts(&s);
        assert!((p[1].feather - 0.1).abs() < 1e-6 && (p[0].feather - 0.02).abs() < 1e-6);
        // Removed again, down to three, not below.
        assert!(s.remove_point(3_000).is_some());
        assert_eq!(pts(&s).len(), 4);
        s.choose_handle(Some(1));
        s.remove_point(4_000);
        s.choose_handle(Some(1));
        let said = s.remove_point(5_000);
        assert_eq!(pts(&s).len(), 3);
        assert_eq!(said, Some(t!("develop-mask-point-last")));
        s.choose_handle(Some(2));
        assert!(s.insert_point(6_000).is_some());
        assert_eq!(pts(&s).len(), 4);
        // Alt+click on a point removes it; on the centre handle nothing goes.
        let before = pts(&s);
        assert!(s.remove_point_at(0, 7_000).is_none());
        assert_eq!(pts(&s), before);
        assert_eq!(
            s.remove_point_at(2, 8_000),
            Some(t!("develop-mask-point-removed", n = 2))
        );
        let after = pts(&s);
        assert_eq!(after.len(), 3);
        assert_eq!(after, [before[0], before[2], before[3]]);
        assert_eq!(
            s.remove_point_at(1, 9_000),
            Some(t!("develop-mask-point-last"))
        );
    }

    #[test]
    fn smoothing_trails_and_catches_up() {
        assert_eq!(smooth(None, (0.3, 0.4), 0.8), (0.3, 0.4));
        let mut p = Some((0.0, 0.0));
        for _ in 0..40 {
            p = Some(smooth(p, (1.0, 0.0), 0.5));
        }
        let q = p.expect("p");
        assert!((q.0 - 1.0).abs() < 1e-9);
        let one = smooth(Some((0.0, 0.0)), (1.0, 0.0), 0.5);
        assert!((one.0 - 0.5).abs() < 1e-12);
        assert_eq!(smooth(Some((0.0, 0.0)), (1.0, 0.0), 0.0), (1.0, 0.0));
    }

    #[test]
    fn brush_keys_and_the_overlay_key() {
        use specular_input::Action as A;
        let mut s = session();
        let k = ModuleKind::EXPOSURE;
        let size = s.masks.brush.size;
        s.mask_action(A::DevelopBrushLarger);
        assert!(s.masks.brush.size > size);
        s.mask_action(A::DevelopBrushSmaller);
        assert!((s.masks.brush.size - size).abs() < 1e-6);
        let h = s.masks.brush.hardness;
        s.mask_action(A::DevelopBrushSofter);
        assert!(s.masks.brush.hardness < h);
        s.mask_action(A::DevelopBrushErase);
        assert!(s.masks.brush.erase);
        // No mask: nothing to show; then the module with one.
        assert_eq!(
            s.mask_action(A::DevelopMaskOverlay),
            t!("develop-mask-overlay-none")
        );
        s.add_row(k, NewKind::Ellipse, 0);
        s.mask_action(A::DevelopMaskOverlay);
        assert_eq!(s.masks.overlay, Some(((k, 0), Overlay::Tint)));
        s.mask_action(A::DevelopMaskOverlay);
        assert_eq!(s.masks.overlay, None);
    }

    #[test]
    fn a_brush_stroke_carries_pressure_and_its_use() {
        let mut s = session();
        let m = map();
        let k = ModuleKind::EXPOSURE;
        s.add_row(k, NewKind::Brush, 0);
        s.masks.brush.pressure = PressureUse::Both;
        s.masks.brush.smoothing = 0.0;
        s.masks.pressure = Some(0.25);
        assert!(s.paint(&m, (100.0, 100.0), true));
        s.masks.pressure = Some(0.75);
        assert!(s.paint(&m, (200.0, 120.0), false));
        let Some((_, _, Shape::Brush { strokes })) = s.active_shape() else {
            panic!("brush");
        };
        assert_eq!(strokes[0].pressure, PressureUse::Both);
        assert_eq!(strokes[0].points[0][2], 0.25);
        assert_eq!(strokes[0].points[1][2], 0.75);
    }
}
