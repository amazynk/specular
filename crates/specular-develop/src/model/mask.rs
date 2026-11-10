//! Masks and blending, rendered by [`crate::mask`].
//!
//! # Coordinates
//!
//! Shapes are in **source coordinates**, so they follow the fused warp (crop, straighten,
//! turns, lens correction) and stay on the subject:
//! - **Positions** are fractions 0..1 of the source frame (the raw's crop, or the image), in
//!   the sensor's own orientation, before the file's orientation is applied.
//! - **Lengths** (radii, half sizes, brush radius, path feather) are fractions of the
//!   frame's **long side**, so a circle is round on any frame shape.
//! - **Angles** are degrees, clockwise in the frame (y down).

use super::fields::choice;
use super::kind::ModuleKind;

/// A shape shared by the edit; instances refer to it by `id`.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskShape {
    /// Unique within the edit; never reused after deletion.
    pub id: u32,
    pub shape: Shape,
}

/// A point on a path with its own feather (source coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPoint {
    pub x: f32,
    pub y: f32,
    pub feather: f32,
}

choice! {
    /// What a pen's pressure changes in a brush stroke (a mouse gives full pressure).
    PressureUse {
        /// How much each dab adds (the original strokes).
        Flow = "flow",
        /// The dab's size: from a fifth of the radius at no pressure to all of it.
        Size = "size",
        Both = "both",
    }
}

impl PressureUse {
    /// The flow and radius factors at `pressure` (0..1).
    pub fn factors(self, pressure: f32) -> (f32, f32) {
        let size = 0.2 + 0.8 * pressure;
        match self {
            PressureUse::Flow => (pressure, 1.0),
            PressureUse::Size => (1.0, size),
            PressureUse::Both => (pressure, size),
        }
    }
}

/// One brush stroke: points as (x, y, pressure 0..1). `radius` is a fraction of the frame's
/// long side; `hardness` (0..1) the part of the radius at full strength; `flow` (0..1) how
/// much one dab adds; `pressure` what the pen's pressure scales; `erase` takes away instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub radius: f32,
    pub hardness: f32,
    pub flow: f32,
    pub erase: bool,
    pub pressure: PressureUse,
    pub points: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Stamped into a cached mask texture.
    Brush { strokes: Vec<Stroke> },
    /// Analytic (SDF) from here on. `feather` (0..1) is the soft band inside the edge, as a
    /// fraction of the smaller radius.
    Ellipse {
        centre: [f32; 2],
        radii: [f32; 2],
        angle_deg: f32,
        feather: f32,
    },
    /// A rectangle with `half` its half sizes, corners rounded by `round` (0..1 of the
    /// smaller half size); `feather` as the ellipse's.
    Rectangle {
        centre: [f32; 2],
        half: [f32; 2],
        angle_deg: f32,
        round: f32,
        feather: f32,
    },
    /// A closed path through `points` (an open one closes itself); each point's feather is
    /// the soft band inside the edge there, interpolated along the edges.
    Polygon {
        points: Vec<PathPoint>,
        closed: bool,
    },
    /// Fully on at `start`, fully off at `end`.
    Linear { start: [f32; 2], end: [f32; 2] },
    /// On inside the inner ellipse, off outside the outer one.
    Radial {
        centre: [f32; 2],
        inner: [f32; 2],
        outer: [f32; 2],
        angle_deg: f32,
    },
}

choice! {
    /// What a parametric range tests.
    RangeChannel {
        /// Luminance in EV.
        Luminance = "luminance",
        Chroma = "chroma",
        Hue = "hue",
        Red = "red",
        Green = "green",
        Blue = "blue",
    }
}

/// A feathered range: off below `limits[0]`, ramps to on at `limits[1]`, on to `limits[2]`,
/// ramps off by `limits[3]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    pub channel: RangeChannel,
    /// Test the module's output rather than its input.
    pub on_output: bool,
    pub limits: [f32; 4],
}

/// Colour range: ΔE distance in a UCS to one or more picked colours.
#[derive(Clone, Debug, PartialEq)]
pub struct ColourRange {
    /// Picked colours in the UCS (lightness, a, b).
    pub colours: Vec<[f32; 3]>,
    pub distance: f32,
    pub falloff: f32,
}

choice! {
    Combine {
        Union = "union",
        Intersect = "intersect",
        Subtract = "subtract",
        Exclude = "exclude",
    }
}

/// Detailed areas: the local contrast of the module's input (the standard deviation
/// of log₂ luminance over 0.6 % of the long side, measured on the picture blurred by a third
/// of that, so the preview and an export agree). On from `threshold` (EV) over `softness`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Details {
    pub threshold: f32,
    pub softness: f32,
}

impl Default for Details {
    fn default() -> Self {
        Self {
            threshold: 0.25,
            softness: 0.2,
        }
    }
}

choice! {
    /// What an AI mask selects.
    AiKind {
        /// The photo's main subject.
        Subject = "subject",
        Sky = "sky",
        /// The object at the clicks and box.
        Object = "object",
    }
}

/// A click on an AI object mask, in source coordinates (fractions of the frame).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiPoint {
    pub x: f32,
    pub y: f32,
    /// Part of the object (else: not part of it).
    pub positive: bool,
}

/// Clicks and a box an object mask takes at most (the network's prompt size).
pub const AI_MAX_POINTS: usize = 6;

/// An AI mask row. The sidecar keeps what selects it (kind, clicks, box, model), so
/// another computer recomputes the same mask; the computed mask itself lives in the app's
/// cache, found by `key`.
#[derive(Clone, Debug, PartialEq)]
pub struct AiMask {
    pub kind: AiKind,
    /// The model it is made with (the manifest id, "efficientsam-ti").
    pub model: String,
    /// Object masks: the clicks.
    pub points: Vec<AiPoint>,
    /// Object masks: a box (x0, y0, x1, y1), source coordinates.
    pub rect: Option<[f32; 4]>,
    /// The computed mask's cache key ([`crate::mask::ai::key`]: the photo, the fields above
    /// and the model); zero until the app keys it for its photo. Not in the sidecar.
    pub key: [u8; 32],
}

impl AiMask {
    pub fn new(kind: AiKind, model: &str) -> AiMask {
        AiMask {
            kind,
            model: model.to_owned(),
            points: Vec::new(),
            rect: None,
            key: [0; 32],
        }
    }

    /// Prompt points the network gets (a box counts two).
    pub fn prompt_len(&self) -> usize {
        self.points.len() + if self.rect.is_some() { 2 } else { 0 }
    }
}

/// A depth row: where the photo's depth map (Develop's depth network, cached per
/// photo like AI masks) is between `near` and `far`, 0 the nearest and 1 the farthest point
/// of the photo, with soft edges `feather` wide. The sidecar keeps the range and the model;
/// the depth map itself lives in the app's cache ([`crate::mask::depth`]).
#[derive(Clone, Debug, PartialEq)]
pub struct DepthRange {
    /// The model the depth map is made with (the manifest id).
    pub model: String,
    pub near: f32,
    pub far: f32,
    pub feather: f32,
}

/// The depth model new rows and modules name.
pub const DEPTH_MODEL: &str = "depth-anything-v2-small";

impl DepthRange {
    /// The nearest third.
    pub fn near(model: &str) -> DepthRange {
        DepthRange {
            model: model.to_owned(),
            near: 0.0,
            far: 0.35,
            feather: 0.1,
        }
    }

    /// The farthest third.
    pub fn far(model: &str) -> DepthRange {
        DepthRange {
            model: model.to_owned(),
            near: 0.65,
            far: 1.0,
            feather: 0.1,
        }
    }

    /// A band in the middle.
    pub fn band(model: &str) -> DepthRange {
        DepthRange {
            model: model.to_owned(),
            near: 0.3,
            far: 0.7,
            feather: 0.1,
        }
    }

    /// The weight of depth `t` (0..1): on inside the range, soft edges of `feather`; the
    /// range's open ends (0 or 1) have no edge.
    pub fn weight(&self, t: f32) -> f32 {
        let f = if self.feather.is_finite() {
            self.feather.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (lo, hi) = (self.near.min(self.far), self.near.max(self.far));
        let ss = |e0: f32, e1: f32, x: f32| {
            if e1 <= e0 {
                return if x >= e0 { 1.0 } else { 0.0 };
            }
            let k = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
            k * k * (3.0 - 2.0 * k)
        };
        let a = if lo <= 0.0 { 1.0 } else { ss(lo - f, lo, t) };
        let b = if hi >= 1.0 {
            1.0
        } else {
            1.0 - ss(hi, hi + f, t)
        };
        a * b
    }
}

/// The mask expression tree of one blend.
#[derive(Clone, Debug, PartialEq)]
pub enum MaskExpr {
    /// Everywhere (no mask).
    All,
    Shape(u32),
    Range(Range),
    ColourRange(ColourRange),
    Details(Details),
    /// "Use mask from": another instance's combined mask (its rows and invert, not its
    /// refinement), a live reference by kind and instance number.
    FromModule {
        kind: ModuleKind,
        instance: u16,
    },
    /// An AI mask: what to select, and with which model.
    Ai(AiMask),
    /// A depth range.
    Depth(DepthRange),
    Invert(Box<MaskExpr>),
    Combine(Combine, Box<MaskExpr>, Box<MaskExpr>),
    /// The inner mask times an opacity 0..1 (a node's own opacity in the combine tree).
    Opacity(f32, Box<MaskExpr>),
}

/// The original refine radius: 2 % of the long side.
pub const REFINE_RADIUS: f32 = 0.02;

/// Refinement of the combined mask, in this order: `details`, `feather`, `edge_aware`,
/// `blur`, then `contrast` (−1..1) and `offset` (−1..1). Lengths are fractions of the long
/// side.
#[derive(Clone, Debug, PartialEq)]
pub struct Refine {
    /// Grows the mask outwards by a soft edge this wide (its inside stays as it is).
    pub feather: f32,
    /// Blurs the mask by this radius.
    pub blur: f32,
    pub contrast: f32,
    pub offset: f32,
    /// Guided-filter edge refinement strength 0..1 (guide = the module's input); its
    /// radius is `radius × edge_aware`.
    pub edge_aware: f32,
    /// Keep the mask only on detail (> 0) or only on flat areas (< 0): the magnitude is the
    /// details mask's threshold in EV; 0 is off.
    pub details: f32,
    /// The edge refinement's radius at full strength (originally a fixed 2 %).
    pub radius: f32,
}

impl Default for Refine {
    fn default() -> Self {
        Self {
            feather: 0.0,
            blur: 0.0,
            contrast: 0.0,
            offset: 0.0,
            edge_aware: 0.0,
            details: 0.0,
            radius: REFINE_RADIUS,
        }
    }
}

impl Refine {
    /// Whether the mask needs the refine pass (anything but contrast and offset).
    pub fn needs_pass(&self) -> bool {
        let on = |v: f32| v.is_finite() && v > 0.0;
        on(self.feather) || on(self.blur) || on(self.edge_aware) || on(self.details.abs())
    }
}

choice! {
    /// Blend modes that make sense on scene-linear data.
    BlendMode {
        Normal = "normal",
        Multiply = "multiply",
        Divide = "divide",
        Add = "add",
        Subtract = "subtract",
        Lighten = "lighten",
        Darken = "darken",
        Luminance = "luminance",
        Colour = "colour",
        Hue = "hue",
        Chroma = "chroma",
        Screen = "screen",
        Overlay = "overlay",
        SoftLight = "soft-light",
    }
}

/// How a module's output mixes with its input.
#[derive(Clone, Debug, PartialEq)]
pub struct Blend {
    pub mode: BlendMode,
    pub opacity: f32,
    pub mask: MaskExpr,
    /// Use the complement of the final mask.
    pub invert: bool,
    pub refine: Refine,
}

impl Blend {
    /// The combined mask as another module borrows it: the rows, inverted if this blend
    /// inverts.
    pub fn combined(&self) -> MaskExpr {
        if self.invert {
            MaskExpr::Invert(Box::new(self.mask.clone()))
        } else {
            self.mask.clone()
        }
    }
}

impl Default for Blend {
    fn default() -> Self {
        Self {
            mode: BlendMode::Normal,
            opacity: 1.0,
            mask: MaskExpr::All,
            invert: false,
            refine: Refine::default(),
        }
    }
}

impl MaskExpr {
    /// Every shape id the expression refers to, in tree order.
    pub fn shape_ids(&self, out: &mut Vec<u32>) {
        match self {
            MaskExpr::Shape(id) => out.push(*id),
            MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => a.shape_ids(out),
            MaskExpr::Combine(_, a, b) => {
                a.shape_ids(out);
                b.shape_ids(out);
            }
            _ => {}
        }
    }

    /// Whether the expression uses `kind`/`instance`'s mask ("Use mask from").
    pub fn refers_to(&self, kind: ModuleKind, instance: u16) -> bool {
        match self {
            MaskExpr::FromModule {
                kind: k,
                instance: i,
            } => *k == kind && *i == instance,
            MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => a.refers_to(kind, instance),
            MaskExpr::Combine(_, a, b) => {
                a.refers_to(kind, instance) || b.refers_to(kind, instance)
            }
            _ => false,
        }
    }

    /// Replaces every reference to `kind`/`instance` by `with`.
    pub fn replace_reference(&mut self, kind: ModuleKind, instance: u16, with: &MaskExpr) {
        match self {
            MaskExpr::FromModule {
                kind: k,
                instance: i,
            } if *k == kind && *i == instance => {
                *self = with.clone();
            }
            MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => {
                a.replace_reference(kind, instance, with);
            }
            MaskExpr::Combine(_, a, b) => {
                a.replace_reference(kind, instance, with);
                b.replace_reference(kind, instance, with);
            }
            _ => {}
        }
    }

    /// Whether a details mask is in the expression itself (not through references).
    pub fn has_details(&self) -> bool {
        match self {
            MaskExpr::Details(_) => true,
            MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => a.has_details(),
            MaskExpr::Combine(_, a, b) => a.has_details() || b.has_details(),
            _ => false,
        }
    }

    /// A shape, range or reference (no combination).
    pub fn is_leaf(&self) -> bool {
        !matches!(
            self,
            MaskExpr::All | MaskExpr::Invert(_) | MaskExpr::Combine(..) | MaskExpr::Opacity(..)
        )
    }

    /// The rows of a mask built by [`MaskExpr::from_layers`] (`All` has none); `None` for a
    /// tree the panel cannot list.
    pub fn layers(&self) -> Option<Vec<Layer>> {
        match self {
            MaskExpr::All => Some(Vec::new()),
            MaskExpr::Combine(op, a, b) => {
                let mut v = a.layers()?;
                if v.is_empty() {
                    return None;
                }
                let mut l = Layer::of(b)?;
                l.op = *op;
                v.push(l);
                Some(v)
            }
            e => Layer::of(e).map(|l| vec![l]),
        }
    }

    /// The mask of rows, combined top to bottom (`All` for none).
    pub fn from_layers(layers: &[Layer]) -> MaskExpr {
        let mut it = layers.iter();
        let Some(first) = it.next() else {
            return MaskExpr::All;
        };
        let mut e = first.expr();
        for l in it {
            e = MaskExpr::Combine(l.op, Box::new(e), Box::new(l.expr()));
        }
        e
    }
}

/// One row of a mask as the panel lists it: an item, combined with the rows above it by
/// `op` (the first row's `op` is ignored), inverted and faded on its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub op: Combine,
    pub invert: bool,
    pub opacity: f32,
    /// A leaf: a shape, a range, a colour range, another module's mask, an AI mask.
    pub item: MaskExpr,
}

impl Layer {
    pub fn new(item: MaskExpr) -> Layer {
        Layer {
            op: Combine::Union,
            invert: false,
            opacity: 1.0,
            item,
        }
    }

    fn expr(&self) -> MaskExpr {
        let mut e = self.item.clone();
        if self.invert {
            e = MaskExpr::Invert(Box::new(e));
        }
        if self.opacity != 1.0 {
            e = MaskExpr::Opacity(self.opacity, Box::new(e));
        }
        e
    }

    fn of(e: &MaskExpr) -> Option<Layer> {
        let (opacity, e) = match e {
            MaskExpr::Opacity(k, a) => (*k, &**a),
            e => (1.0, e),
        };
        let (invert, e) = match e {
            MaskExpr::Invert(a) => (true, &**a),
            e => (false, e),
        };
        // A combination as a row's item is a group (a borrowed mask copied in when its
        // module went).
        (e.is_leaf() || matches!(e, MaskExpr::Combine(..))).then(|| Layer {
            op: Combine::Union,
            invert,
            opacity,
            item: e.clone(),
        })
    }
}
