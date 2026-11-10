//! Masks and blending on the GPU.
//!
//! A module instance with a [`Blend`] mixes its output with its input through a mask. The
//! mask is evaluated **inside the module's own kernel** (fused with its neighbours), so a
//! mask costs one more function call per pixel and never a pass of its own, except for edge
//! refinement.
//!
//! # The mask table
//!
//! [`compile`] turns every blend of an edit into a small program in one storage buffer, the
//! **mask table** (binding 6 of every kernel, read with `mku(i)` / `mkf(i)`):
//!
//! | Words | Meaning |
//! |---|---|
//! | 0 | table version |
//! | 1–2 | coordinate grid side `n` and offset |
//! | 3–4 | shape-space scale of x and y (frame width and height over its long side) |
//! | 5 | one image pixel in shape-space units (the anti-aliasing band) |
//! | 6 | brush rasters in use |
//! | 8.. | per raster: offset, width, height, row stride (u16 texels) |
//! | then | the coordinate grid, then the programs |
//! | after the head | brush rasters, two u16 texels per word |
//!
//! - **Coordinates.** Shapes are in source coordinates ([`crate::model::mask`]). The grid
//!   holds, for `n`×`n` points spread over the graph's image, the source-frame point the
//!   fused warp ([`Plan::to_source`]) samples there; kernels interpolate it bilinearly. Crop,
//!   straighten and turns are affine, so the grid is exact for them; lens distortion is
//!   smooth, and a 65×65 grid keeps it within a small fraction of a pixel (tested). A
//!   perspective is not affine: its grid is 129×129 ([`GRID_PERSP`]). Masks therefore
//!   follow every geometry change.
//! - **Shapes** are signed-distance functions (ellipse, rounded rectangle, polygon) or
//!   gradients (linear, radial) evaluated analytically per pixel; feathering is a smoothstep
//!   over a band inside the edge, so the mask only falls moving outwards.
//! - **Brushes** are stroke lists, stamped into a u16 raster over the frame at the image's
//!   resolution ([`brush`]) and cached: a new dab stamps and uploads only the rows it
//!   touches.
//! - **Ranges** test luminance (EV from mid grey), OkLab chroma or hue, or a linear channel
//!   of the module's input or output; colour ranges take the OkLab distance to picked
//!   colours.
//! - **Combining** runs on a small stack: union (max), intersect (min), subtract
//!   (`a·(1−b)`), exclude (`|a−b|`), invert and opacity.
//!
//! Moving any mask value rewrites words of the table and the uniforms: the kernels never
//! change ([`crate::gpu`] counts pipelines). Giving a module its first blend builds its
//! kernel once, as switching a module on does.
//!
//! # Blending
//!
//! The blend's uniform words ([`BLEND_WORDS`]) follow the op's own. The kernel computes the
//! op's output `b` from its input `a`, the mask `m` (refined by contrast and offset, then
//! inverted if asked) and returns `mix(a, mode(a, b), m · opacity)`. Modes that need a 0..1
//! range (multiply, screen, overlay, soft light, divide, add, subtract) work on
//! `x / (x + 0.18)`, which maps scene-linear 0..∞ onto 0..1 with mid grey at ½, so mid grey
//! is their neutral point and highlights are never clipped. Lightness-only scales the input
//! by the output's luminance ratio; colour-only does the reverse.
//!
//! # Refinement
//!
//! With a details mask, feather, edge refinement, blur or `details` on, a node of its own
//! ([`refine`]) runs before the module: the details measure, the mask over its region, then
//! the refinements, written into the alpha channel; the module's kernel reads it there.
//! Output ranges see the module's own output when it is a plain point op, else its input.
//!
//! # Borrowed masks
//!
//! "Use mask from" compiles the source's rows and invert into the borrowing module's
//! program, by kind and instance number: it follows every edit of the source and any
//! reorder, and the two masks are the same words, so the same values where the two modules
//! see the same pixels (ranges and details test each module's own input).

pub mod ai;
pub mod brush;
pub mod depth;
pub mod refine;
pub mod shader;

#[cfg(test)]
pub(crate) mod cpu_ref;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use crate::model::hash::Hash;
use crate::model::mask::{Blend, BlendMode, ColourRange, Combine, MaskExpr, Range, Shape};
use crate::model::{Edit, ModuleKind};
use crate::ops::geometry::Plan;
use brush::{BrushEntry, Raster};

/// Uniform words of a blend, after the op's own.
pub const BLEND_WORDS: u32 = 8;
/// Points per side of the coordinate grid.
pub const GRID: u32 = 65;
/// Points per side with a perspective: a homography is not affine, and 129 keeps the
/// strongest keystone within 0.1 px at 6000 px (tested).
pub const GRID_PERSP: u32 = 129;

/// The grid's side for `g`.
pub fn grid_side(g: &Geometry<'_>) -> u32 {
    match g {
        Geometry::Warp(p) if p.persp.is_some() => GRID_PERSP,
        _ => GRID,
    }
}
/// Brush rasters one table holds.
pub const MAX_RASTERS: usize = 16;
/// Depth of the evaluation stack in the kernels.
pub const MAX_STACK: usize = 8;
/// Longest side of a brush raster.
pub const RASTER_CAP: u32 = 8192;
/// Texels all brush rasters of one table may take together (2 bytes each).
pub const RASTER_TEXELS: u64 = 60 << 20;
/// "Use mask from" chains longer than this mask nothing.
const MAX_REFS: u32 = 8;
/// Table words before the grid.
pub const HEAD_FIXED: u32 = 8 + 4 * MAX_RASTERS as u32;
/// Rasters start at a multiple of this many words, so small edits keep their offsets.
const HEAD_ALIGN: usize = 4096;
/// Table layout version (word 0).
const VERSION: u32 = 1;

/// Program opcodes (see [`shader::MASK_WGSL`]).
pub mod opc {
    pub const END: u32 = 0;
    pub const ALL: u32 = 1;
    pub const ELLIPSE: u32 = 2;
    pub const RECT: u32 = 3;
    pub const POLY: u32 = 4;
    pub const LINEAR: u32 = 5;
    pub const RADIAL: u32 = 6;
    pub const BRUSH: u32 = 7;
    pub const RANGE: u32 = 8;
    pub const COLOUR: u32 = 9;
    pub const INVERT: u32 = 10;
    pub const UNION: u32 = 11;
    pub const INTERSECT: u32 = 12;
    pub const SUBTRACT: u32 = 13;
    pub const EXCLUDE: u32 = 14;
    pub const OPACITY: u32 = 15;
    pub const ZERO: u32 = 16;
    pub const DETAILS: u32 = 17;
    /// Contrast and offset of the top value (a borrowed mask's source's).
    pub const LEVEL: u32 = 18;
    /// A depth range over the depth raster: slot, near, far, feather.
    pub const DEPTH: u32 = 19;
}

/// The blend a module's mask is drawn with. A mask borrowed whole ("Use mask from") by a
/// module that neither refines it nor sets its contrast or offset takes the source's
/// expression, contrast, offset and refinement (the inverts combine), so it is the source's
/// final mask; chains follow up to `MAX_REFS`. Any other borrow takes the source's
/// expression, contrast, offset and invert per pixel ([`opc::LEVEL`]), without its
/// refinement pass.
pub fn effective_blend<'a>(edit: &'a Edit, b: &'a Blend) -> std::borrow::Cow<'a, Blend> {
    let mut cur = std::borrow::Cow::Borrowed(b);
    for _ in 0..MAX_REFS {
        let MaskExpr::FromModule { kind, instance } = &cur.mask else {
            break;
        };
        let r = &cur.refine;
        if r.contrast != 0.0 || r.offset != 0.0 || r.needs_pass() || r.details != 0.0 {
            break;
        }
        let Some(src) = edit.module(*kind, *instance).and_then(|m| m.blend.as_ref()) else {
            break;
        };
        let mut n = cur.into_owned();
        n.mask = src.mask.clone();
        n.refine = src.refine.clone();
        n.invert ^= src.invert;
        cur = std::borrow::Cow::Owned(n);
    }
    cur
}

/// Points of a polygon, colours of a colour range: more are left out.
pub const MAX_POINTS: usize = 256;
pub const MAX_COLOURS: usize = 16;

/// What the overlay shows of the focused module's mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Overlay {
    /// The picture with the mask tinted red.
    Tint,
    /// The mask alone, white where it is on.
    Grey,
}

/// What the graph's image is, relative to the source frame the shapes live in.
#[derive(Clone, Copy, Debug)]
pub enum Geometry<'a> {
    /// The image is the frame (`w`×`h` sensor pixels), unwarped.
    Frame { w: f64, h: f64 },
    /// The image is this warp's output.
    Warp(&'a Plan),
}

impl Geometry<'_> {
    fn frame(&self) -> (f64, f64) {
        match self {
            Geometry::Frame { w, h } => (w.max(1.0), h.max(1.0)),
            Geometry::Warp(p) => (p.frame[2].max(1.0), p.frame[3].max(1.0)),
        }
    }

    /// Image pixels per sensor pixel for an `image`-sized graph image.
    fn pixels_per_sensor(&self, image: (u32, u32)) -> f64 {
        let w = f64::from(image.0.max(1));
        match self {
            Geometry::Frame { w: fw, .. } => w / fw.max(1.0),
            Geometry::Warp(p) => w / p.crop_rect()[2].max(1e-9),
        }
    }

    /// The source-frame point (fractions) shown at image point `(u, v)` (fractions).
    pub fn frame_point(&self, u: f64, v: f64) -> (f64, f64) {
        match self {
            Geometry::Frame { .. } => (u, v),
            Geometry::Warp(p) => {
                let c = p.crop_rect();
                let (s, _) = p.to_source(1, (u * c[2], v * c[3]), 1.0);
                let [fx, fy, fw, fh] = p.frame;
                ((s.0 - fx) / fw.max(1e-9), (s.1 - fy) / fh.max(1e-9))
            }
        }
    }

    /// Identifies the geometry (the warp's plan, or the plain frame).
    pub fn key(&self) -> Hash {
        let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 mask grid v1");
        match self {
            Geometry::Frame { .. } => {
                h.update(b"frame");
            }
            Geometry::Warp(p) => {
                h.update(format!("{p:?}").as_bytes());
            }
        }
        *h.finalize().as_bytes()
    }
}

/// What masks are evaluated against for one render.
pub struct MaskEnv<'a> {
    pub geometry: Geometry<'a>,
    /// The graph image's size (pixels).
    pub image: (u32, u32),
    /// The render's scale ([`crate::gpu::Request::scale`]): radii in pixels per unit of it.
    pub scale: f32,
    pub cache: &'a mut MaskCache,
    /// Sensor pixels per pixel of the source the geometry's frame is measured in (2 for a
    /// raw binned 2×2 for a thumbnail, else 1).
    pub sensor_bin: f64,
    /// Show this module's mask.
    pub overlay: Option<((ModuleKind, u16), Overlay)>,
}

/// Kept between renders of one image: the coordinate grid and the brush rasters.
#[derive(Default)]
pub struct MaskCache {
    grid: Option<(Hash, Arc<Vec<f32>>)>,
    brushes: HashMap<u32, BrushEntry>,
    next_id: u64,
}

impl std::fmt::Debug for MaskCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaskCache")
            .field("brushes", &self.brushes.len())
            .finish_non_exhaustive()
    }
}

impl MaskCache {
    pub fn clear(&mut self) {
        self.grid = None;
        self.brushes.clear();
    }

    /// The raster of brush shape `id`, as last compiled.
    pub fn raster(&self, id: u32) -> Option<&Raster> {
        self.brushes.get(&id).map(|b| &*b.raster)
    }

    /// Bytes the rasters take on the host.
    pub fn bytes(&self) -> u64 {
        self.brushes.values().map(|b| b.raster.bytes()).sum()
    }
}

/// The table one render binds.
#[derive(Debug)]
pub struct MaskTable {
    /// Header, grid and programs.
    pub head: Vec<u32>,
    pub head_hash: Hash,
    /// Each raster with its word offset.
    pub rasters: Vec<(u64, Arc<Raster>)>,
    /// Words the whole table takes.
    pub words: u64,
}

/// The refine pass of one masked module ([`refine::RefineOp`]). Lengths are image pixels
/// per unit of render scale; 0 turns a step off.
#[derive(Clone, Debug, PartialEq)]
pub struct RefineSpec {
    /// The mask program's word offset.
    pub program: u32,
    /// Edge refinement (guided filter) radius.
    pub radius_per_scale: f32,
    /// Guided filter ε (EV²).
    pub eps: f32,
    /// The details measure's window radius (needed by a details mask or `details`).
    pub detail_per_scale: f32,
    /// [`crate::model::mask::Refine::details`]: keep detail (> 0) or flat areas (< 0).
    pub details: f32,
    /// Outward feather.
    pub feather_per_scale: f32,
    pub blur_per_scale: f32,
    /// The program has ranges on the module's output.
    pub output: bool,
}

/// One masked module's part of a compiled table.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeBlend {
    pub words: [u32; BLEND_WORDS as usize],
    pub refine: Option<RefineSpec>,
}

/// Why a mask renders as nothing (for the panel to say).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    /// A shape the blend refers to is gone.
    MissingShape,
    /// An AI mask not computed yet (or its model is missing): it masks nothing until the
    /// app puts its raster in [`ai::store`].
    AiMask,
    /// A depth row whose depth map is not computed yet (or its model is missing): it masks
    /// nothing until the app puts the map in [`ai::store`] ([`depth`]).
    Depth,
    /// "Use mask from" loops or goes too deep.
    BadReference,
    /// Too many brush masks in one edit.
    TooManyBrushes,
    /// The combination nests too deeply.
    TooDeep,
}

/// Every blend of an edit, compiled.
#[derive(Debug)]
pub struct Compiled {
    pub table: Arc<MaskTable>,
    pub blends: HashMap<(ModuleKind, u16), NodeBlend>,
    pub issues: Vec<((ModuleKind, u16), Issue)>,
}

/// Blend flag: the mask is in the input's alpha (refined).
pub const FLAG_REFINED: u32 = 1;
/// Blend flag: write the mask into the alpha for the overlay.
pub const FLAG_OVERLAY: u32 = 2;

/// The kernels' number for a blend mode.
pub fn mode_index(m: BlendMode) -> u32 {
    match m {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Divide => 2,
        BlendMode::Add => 3,
        BlendMode::Subtract => 4,
        BlendMode::Lighten => 5,
        BlendMode::Darken => 6,
        BlendMode::Luminance => 7,
        BlendMode::Colour => 8,
        BlendMode::Hue => 9,
        BlendMode::Chroma => 10,
        BlendMode::Screen => 11,
        BlendMode::Overlay => 12,
        BlendMode::SoftLight => 13,
    }
}

/// Whether any enabled module of `edit` blends.
pub fn has_blends(edit: &Edit) -> bool {
    edit.modules.iter().any(|m| m.enabled && m.blend.is_some())
}

fn fbits(v: f64) -> u32 {
    let v = v as f32;
    if v.is_finite() { v.to_bits() } else { 0 }
}

fn clamp01(v: f32) -> f64 {
    if v.is_finite() {
        f64::from(v).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Shape-space conversions of one table.
#[derive(Clone, Copy, Debug)]
pub struct Space {
    /// Frame width and height over the long side.
    pub ax: f64,
    pub ay: f64,
}

impl Space {
    pub fn of(fw: f64, fh: f64) -> Space {
        let l = fw.max(fh).max(1e-9);
        Space {
            ax: fw / l,
            ay: fh / l,
        }
    }

    /// A frame point (fractions) in shape space.
    pub fn pos(&self, p: [f32; 2]) -> (f64, f64) {
        (f64::from(p[0]) * self.ax, f64::from(p[1]) * self.ay)
    }
}

/// A finite value in 0..1 (0 otherwise).
fn fin(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A finite non-negative length.
fn len(v: f32) -> f64 {
    if v.is_finite() {
        f64::from(v).max(0.0)
    } else {
        0.0
    }
}

struct Emit<'a> {
    edit: &'a Edit,
    space: Space,
    words: Vec<u32>,
    depth: usize,
    max_depth: usize,
    issue: Option<Issue>,
    /// A details mask is in the program (it needs the refine pass).
    details: bool,
    /// A range on the module's output is in the program.
    output: bool,
    raster_of: &'a mut dyn FnMut(u32) -> Option<u32>,
    /// The raster slot of a computed AI mask, by key.
    ai_of: &'a mut dyn FnMut(&[u8; 32]) -> Option<u32>,
    /// An AI raster is in the program (its edges get refined).
    ai: bool,
}

impl Emit<'_> {
    fn push(&mut self) {
        self.depth += 1;
        self.max_depth = self.max_depth.max(self.depth);
    }

    fn leaf(&mut self, words: &[u32]) {
        self.words.extend_from_slice(words);
        self.push();
    }

    fn zero(&mut self, why: Issue) {
        self.issue.get_or_insert(why);
        self.leaf(&[opc::ZERO]);
    }

    fn f(&mut self, v: f64) {
        self.words.push(fbits(v));
    }

    fn expr(&mut self, e: &MaskExpr, refs: u32) {
        match e {
            MaskExpr::All => self.leaf(&[opc::ALL]),
            MaskExpr::Shape(id) => match self.edit.mask(*id) {
                Some(s) => self.shape(*id, &s.shape),
                None => self.zero(Issue::MissingShape),
            },
            MaskExpr::Range(r) => self.range(r),
            MaskExpr::ColourRange(c) => self.colour(c),
            MaskExpr::FromModule { kind, instance } => {
                let target = self
                    .edit
                    .module(*kind, *instance)
                    .and_then(|m| m.blend.as_ref());
                match target {
                    Some(b) if refs < MAX_REFS => {
                        self.expr(&b.mask, refs + 1);
                        let (c, o) = (b.refine.contrast, b.refine.offset);
                        if (c.is_finite() && c != 0.0) || (o.is_finite() && o != 0.0) {
                            let num = |v: f32| {
                                if v.is_finite() {
                                    v.clamp(-1.0, 1.0)
                                } else {
                                    0.0
                                }
                            };
                            self.words.push(opc::LEVEL);
                            self.words.push(num(c).to_bits());
                            self.words.push(num(o).to_bits());
                        }
                        if b.invert {
                            self.words.push(opc::INVERT);
                        }
                    }
                    _ => self.zero(Issue::BadReference),
                }
            }
            MaskExpr::Ai(m) => match (self.ai_of)(&m.key) {
                Some(k) => {
                    self.ai = true;
                    self.leaf(&[opc::BRUSH, k]);
                }
                None => self.zero(Issue::AiMask),
            },
            MaskExpr::Depth(d) => {
                let photo = depth::photo(&self.edit.source);
                match (self.ai_of)(&depth::key(&photo, &d.model)) {
                    Some(k) => {
                        self.ai = true;
                        let f = if d.feather.is_finite() {
                            d.feather.clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let (n, r) = (fin(d.near), fin(d.far));
                        self.words.extend_from_slice(&[opc::DEPTH, k]);
                        self.f(f64::from(n.min(r)));
                        self.f(f64::from(n.max(r)));
                        self.f(f64::from(f));
                        self.push();
                    }
                    None => self.zero(Issue::Depth),
                }
            }
            MaskExpr::Details(d) => {
                self.details = true;
                self.words.push(opc::DETAILS);
                self.f(f64::from(if d.threshold.is_finite() {
                    d.threshold
                } else {
                    0.0
                }));
                self.f(len(d.softness));
                self.push();
            }
            MaskExpr::Invert(a) => {
                self.expr(a, refs);
                self.words.push(opc::INVERT);
            }
            MaskExpr::Opacity(k, a) => {
                self.expr(a, refs);
                self.words.push(opc::OPACITY);
                self.f(clamp01(*k));
            }
            MaskExpr::Combine(op, a, b) => {
                self.expr(a, refs);
                self.expr(b, refs);
                self.words.push(match op {
                    Combine::Union => opc::UNION,
                    Combine::Intersect => opc::INTERSECT,
                    Combine::Subtract => opc::SUBTRACT,
                    Combine::Exclude => opc::EXCLUDE,
                });
                self.depth -= 1;
            }
        }
    }

    fn shape(&mut self, id: u32, s: &Shape) {
        let sp = self.space;
        match s {
            Shape::Ellipse {
                centre,
                radii,
                angle_deg,
                feather,
            } => {
                let (cx, cy) = sp.pos(*centre);
                let (rx, ry) = (len(radii[0]).max(1e-6), len(radii[1]).max(1e-6));
                let (sn, cs) = f64::from(*angle_deg).to_radians().sin_cos();
                self.words.push(opc::ELLIPSE);
                for v in [cx, cy, rx, ry, cs, sn, clamp01(*feather) * rx.min(ry)] {
                    self.f(v);
                }
                self.push();
            }
            Shape::Rectangle {
                centre,
                half,
                angle_deg,
                round,
                feather,
            } => {
                let (cx, cy) = sp.pos(*centre);
                let (hx, hy) = (len(half[0]).max(1e-6), len(half[1]).max(1e-6));
                let (sn, cs) = f64::from(*angle_deg).to_radians().sin_cos();
                let m = hx.min(hy);
                self.words.push(opc::RECT);
                for v in [
                    cx,
                    cy,
                    hx,
                    hy,
                    cs,
                    sn,
                    clamp01(*round) * m,
                    clamp01(*feather) * m,
                ] {
                    self.f(v);
                }
                self.push();
            }
            Shape::Polygon { points, .. } => {
                let pts: Vec<_> = points
                    .iter()
                    .filter(|p| p.x.is_finite() && p.y.is_finite())
                    .take(MAX_POINTS)
                    .collect();
                if pts.len() < 3 {
                    self.leaf(&[opc::ZERO]);
                    return;
                }
                self.words.push(opc::POLY);
                self.words.push(pts.len() as u32);
                for p in pts {
                    let (x, y) = sp.pos([p.x, p.y]);
                    self.f(x);
                    self.f(y);
                    self.f(len(p.feather));
                }
                self.push();
            }
            Shape::Linear { start, end } => {
                let (sx, sy) = sp.pos(*start);
                let (ex, ey) = sp.pos(*end);
                self.words.push(opc::LINEAR);
                for v in [sx, sy, ex, ey] {
                    self.f(v);
                }
                self.push();
            }
            Shape::Radial {
                centre,
                inner,
                outer,
                angle_deg,
            } => {
                let (cx, cy) = sp.pos(*centre);
                let (sn, cs) = f64::from(*angle_deg).to_radians().sin_cos();
                self.words.push(opc::RADIAL);
                for v in [
                    cx,
                    cy,
                    cs,
                    sn,
                    len(inner[0]).max(1e-6),
                    len(inner[1]).max(1e-6),
                    len(outer[0]).max(1e-6),
                    len(outer[1]).max(1e-6),
                ] {
                    self.f(v);
                }
                self.push();
            }
            Shape::Brush { .. } => match (self.raster_of)(id) {
                Some(k) => self.leaf(&[opc::BRUSH, k]),
                None => self.zero(Issue::TooManyBrushes),
            },
        }
    }

    fn range(&mut self, r: &Range) {
        use crate::model::mask::RangeChannel as C;
        let ch = match r.channel {
            C::Luminance => 0,
            C::Chroma => 1,
            C::Hue => 2,
            C::Red => 3,
            C::Green => 4,
            C::Blue => 5,
        };
        self.words.push(opc::RANGE);
        self.words.push(ch);
        self.output |= r.on_output;
        self.words.push(u32::from(r.on_output));
        let mut l = r.limits.map(|v| if v.is_finite() { v } else { 0.0 });
        // Limits in order.
        for i in 1..4 {
            l[i] = l[i].max(l[i - 1]);
        }
        for v in l {
            self.words.push(v.to_bits());
        }
        self.push();
    }

    fn colour(&mut self, c: &ColourRange) {
        let cols: Vec<_> = c
            .colours
            .iter()
            .filter(|v| v.iter().all(|x| x.is_finite()))
            .take(MAX_COLOURS)
            .collect();
        if cols.is_empty() {
            self.leaf(&[opc::ZERO]);
            return;
        }
        self.words.push(opc::COLOUR);
        self.words.push(cols.len() as u32);
        self.f(len(c.distance));
        self.f(len(c.falloff));
        for v in cols {
            for x in v {
                self.words.push(x.to_bits());
            }
        }
        self.push();
    }
}

/// Compiles every blend of `edit` against `env`; `None` when no enabled module blends.
pub fn compile(edit: &Edit, env: &mut MaskEnv<'_>) -> Option<Compiled> {
    if !has_blends(edit) {
        return None;
    }
    let (fw, fh) = env.geometry.frame();
    let space = Space::of(fw, fh);
    let l = fw.max(fh);
    let px_per_l = (env.geometry.pixels_per_sensor(env.image) * l).max(1.0);
    let aa = 1.0 / px_per_l;
    // The grid, cached by the geometry.
    let gkey = env.geometry.key();
    let grid = match &env.cache.grid {
        Some((k, g)) if *k == gkey => g.clone(),
        _ => {
            let g = Arc::new(make_grid(&env.geometry));
            env.cache.grid = Some((gkey, g.clone()));
            g
        }
    };
    // AI rasters in use: computed ones only, at most half the slots.
    let mut ai_rasters: Vec<([u8; 32], Arc<Raster>)> = Vec::new();
    {
        let mut st = ai::store().lock().unwrap_or_else(|e| e.into_inner());
        for a in ai::rows(edit) {
            if ai_rasters.len() >= MAX_RASTERS / 2 {
                break;
            }
            if let Some(r) = st.get(&a.key) {
                ai_rasters.push((a.key, r));
            }
        }
        // The depth map, once per model its rows name.
        let photo = depth::photo(&edit.source);
        for m in depth::models(edit) {
            let k = depth::key(&photo, &m);
            if ai_rasters.len() < MAX_RASTERS / 2
                && !ai_rasters.iter().any(|(x, _)| *x == k)
                && let Some(r) = st.get(&k)
            {
                ai_rasters.push((k, r));
            }
        }
    }
    let ai_texels: u64 = ai_rasters
        .iter()
        .map(|(_, r)| u64::from(r.stride) * u64::from(r.h))
        .sum();
    // Brush shapes in use, in first-use order, and the raster size budget.
    let mut brush_ids: Vec<u32> = Vec::new();
    for m in edit.modules.iter().filter(|m| m.enabled) {
        if let Some(b) = &m.blend {
            let mut ids = Vec::new();
            collect_shapes(edit, &b.mask, &mut ids, 0);
            for id in ids {
                let brush = matches!(edit.mask(id).map(|s| &s.shape), Some(Shape::Brush { .. }));
                if brush
                    && !brush_ids.contains(&id)
                    && brush_ids.len() + ai_rasters.len() < MAX_RASTERS
                {
                    brush_ids.push(id);
                }
            }
        }
    }
    let mut long = (px_per_l.ceil() as u32).clamp(16, RASTER_CAP);
    if !brush_ids.is_empty() {
        let per = RASTER_TEXELS.saturating_sub(ai_texels).max(1 << 16) / brush_ids.len() as u64;
        let area = (space.ax * space.ay).max(1e-6);
        let fit = ((per as f64 / area).sqrt().floor() as u32).max(16);
        long = long.min(fit);
    }
    let (rw, rh) = (
        ((space.ax * f64::from(long)).round() as u32).max(1),
        ((space.ay * f64::from(long)).round() as u32).max(1),
    );
    // Rasterise (or update) each brush.
    env.cache.brushes.retain(|id, _| brush_ids.contains(id));
    let mut rasters: Vec<Arc<Raster>> = Vec::new();
    for &id in &brush_ids {
        let Some(Shape::Brush { strokes }) = edit.mask(id).map(|s| &s.shape) else {
            continue;
        };
        let next = &mut env.cache.next_id;
        let e = env
            .cache
            .brushes
            .entry(id)
            .or_insert_with(|| BrushEntry::new(next, rw, rh, long));
        e.update(strokes, rw, rh, long, next);
        rasters.push(e.raster.clone());
    }
    // AI rasters follow the brushes.
    let n_brush = rasters.len() as u32;
    rasters.extend(ai_rasters.iter().map(|(_, r)| r.clone()));
    // Programs.
    let mut head = vec![0u32; HEAD_FIXED as usize];
    head[0] = VERSION;
    head[1] = grid_side(&env.geometry);
    head[2] = HEAD_FIXED;
    head[3] = fbits(space.ax);
    head[4] = fbits(space.ay);
    head[5] = fbits(aa);
    head[6] = rasters.len() as u32;
    head.extend(grid.iter().map(|v| v.to_bits()));
    let mut blends = HashMap::new();
    let mut issues = Vec::new();
    for m in edit.modules.iter().filter(|m| m.enabled) {
        let Some(b) = &m.blend else { continue };
        let eb = effective_blend(edit, b);
        let b = &*eb;
        let at = head.len() as u32;
        let mut raster_of = |id: u32| brush_ids.iter().position(|&x| x == id).map(|k| k as u32);
        let mut ai_of = |key: &[u8; 32]| {
            ai_rasters
                .iter()
                .position(|(k, _)| k == key)
                .map(|k| n_brush + k as u32)
        };
        let mut e = Emit {
            ai_of: &mut ai_of,
            ai: false,
            edit,
            space,
            words: Vec::new(),
            depth: 0,
            max_depth: 0,
            issue: None,
            details: false,
            output: false,
            raster_of: &mut raster_of,
        };
        e.expr(&b.mask, 0);
        let mut prog = std::mem::take(&mut e.words);
        let (max_depth, issue, details, output) = (e.max_depth, e.issue, e.details, e.output);
        let has_ai = e.ai;
        if max_depth > MAX_STACK {
            prog = vec![opc::ZERO];
            issues.push((m.id(), Issue::TooDeep));
        } else if let Some(i) = issue {
            issues.push((m.id(), i));
        }
        prog.push(opc::END);
        head.extend_from_slice(&prog);
        blends.insert(
            m.id(),
            node_blend(b, at, env, m.id(), px_per_l, (details, output, has_ai)),
        );
    }
    // Raster table and offsets.
    let base = head.len().div_ceil(HEAD_ALIGN) * HEAD_ALIGN;
    let mut off = base as u64;
    let mut placed = Vec::with_capacity(rasters.len());
    for (k, r) in rasters.into_iter().enumerate() {
        let t = 8 + 4 * k;
        head[t] = off as u32;
        head[t + 1] = r.w;
        head[t + 2] = r.h;
        head[t + 3] = r.stride;
        let words = r.words();
        placed.push((off, r));
        off += words;
    }
    let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 mask head v1");
    h.update(bytemuck::cast_slice(&head));
    let table = MaskTable {
        head_hash: *h.finalize().as_bytes(),
        head,
        rasters: placed,
        words: off.max(base as u64),
    };
    Some(Compiled {
        table: Arc::new(table),
        blends,
        issues,
    })
}

/// The details measure's window: this fraction of the long side.
pub const DETAIL_WINDOW: f64 = 0.006;

/// The refine pass of a blend, if it needs one (refinement or a details mask in its
/// program). Lengths become image pixels per unit of render scale.
fn refine_spec(
    b: &Blend,
    program: u32,
    scale: f32,
    px_per_l: f64,
    details_in_program: bool,
    ai: bool,
) -> Option<RefineSpec> {
    let r = &b.refine;
    if !(r.needs_pass() || details_in_program || ai) {
        return None;
    }
    let scale = f64::from(scale.max(1e-6));
    let pos = |v: f32| {
        if v.is_finite() {
            f64::from(v).max(0.0)
        } else {
            0.0
        }
    };
    // A length (fraction of the long side) in pixels per unit of scale; 0 stays off.
    let px = |frac: f64| {
        if frac > 0.0 {
            ((frac * px_per_l).max(1.0) / scale) as f32
        } else {
            0.0
        }
    };
    let k = pos(r.edge_aware).min(1.0);
    let details = if r.details.is_finite() {
        r.details.clamp(-4.0, 4.0)
    } else {
        0.0
    };
    let detail = details_in_program || details != 0.0;
    // An AI row's edges are refined on the picture at the render's resolution even when
    // the row's own edge refinement is off (full-resolution guided upsampling).
    let radius = if k > 0.0 {
        k * pos(r.radius).min(0.25)
    } else if ai {
        f64::from(ai::EDGE_RADIUS)
    } else {
        0.0
    };
    Some(RefineSpec {
        program,
        radius_per_scale: px(radius),
        eps: 0.01,
        detail_per_scale: if detail { px(DETAIL_WINDOW) } else { 0.0 },
        details,
        feather_per_scale: px(pos(r.feather).min(0.25)),
        blur_per_scale: px(pos(r.blur).min(0.25)),
        output: false,
    })
}

fn node_blend(
    b: &Blend,
    program: u32,
    env: &MaskEnv<'_>,
    id: (ModuleKind, u16),
    px_per_l: f64,
    (details, output, ai): (bool, bool, bool),
) -> NodeBlend {
    let refine = refine_spec(b, program, env.scale, px_per_l, details, ai)
        .map(|s| RefineSpec { output, ..s });
    let mut flags = 0;
    if refine.is_some() {
        flags |= FLAG_REFINED;
    }
    if env.overlay.is_some_and(|(o, _)| o == id) {
        flags |= FLAG_OVERLAY;
    }
    let num = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { 0.0 };
    NodeBlend {
        words: [
            mode_index(b.mode),
            num(b.opacity, 0.0, 1.0).to_bits(),
            program,
            u32::from(b.invert),
            num(b.refine.contrast, -1.0, 1.0).to_bits(),
            num(b.refine.offset, -1.0, 1.0).to_bits(),
            flags,
            0,
        ],
        refine,
    }
}

/// The shapes an expression refers to (through "use mask from" too).
fn collect_shapes(edit: &Edit, e: &MaskExpr, out: &mut Vec<u32>, refs: u32) {
    match e {
        MaskExpr::Shape(id) => out.push(*id),
        MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => collect_shapes(edit, a, out, refs),
        MaskExpr::Combine(_, a, b) => {
            collect_shapes(edit, a, out, refs);
            collect_shapes(edit, b, out, refs);
        }
        MaskExpr::FromModule { kind, instance } if refs < MAX_REFS => {
            if let Some(b) = edit.module(*kind, *instance).and_then(|m| m.blend.as_ref()) {
                collect_shapes(edit, &b.mask, out, refs + 1);
            }
        }
        _ => {}
    }
}

/// The coordinate grid: `GRID`² source-frame points (x, y fractions), row by row.
pub fn make_grid(g: &Geometry<'_>) -> Vec<f32> {
    let n = grid_side(g) as usize;
    let mut v = vec![0f32; n * n * 2];
    for j in 0..n {
        for i in 0..n {
            let u = i as f64 / (n - 1) as f64;
            let w = j as f64 / (n - 1) as f64;
            let (x, y) = g.frame_point(u, w);
            v[2 * (j * n + i)] = x as f32;
            v[2 * (j * n + i) + 1] = y as f32;
        }
    }
    v
}
