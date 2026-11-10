//! Mask presets: a module's mask saved by name and put on other photos.
//!
//! A preset holds the combined rows (borrowed masks copied in), the invert, the refinement
//! and the shapes they use. Shapes are stored **upright**: as fractions of the picture as
//! shown (the file's orientation and the edit's turns applied), so a sky gradient saved on
//! a landscape lands on the sky of a portrait shot too. Lengths stay fractions of the long
//! side. Blend mode and amount are the module's, not the mask's.

use super::edit::Edit;
use super::kind::ModuleKind;
use super::mask::{Blend, MaskExpr, MaskShape, PathPoint, Refine, Shape};
use crate::preview::Orient;

#[derive(Clone, Debug, PartialEq)]
pub struct MaskPreset {
    pub name: String,
    pub mask: MaskExpr,
    pub invert: bool,
    pub refine: Refine,
    /// The shapes `mask` uses, numbered from 1, upright.
    pub masks: Vec<MaskShape>,
}

/// A frame point (fractions) of the stored image, as shown with `o`.
pub fn to_upright(o: Orient, p: [f32; 2]) -> [f32; 2] {
    let (dx, dy) = if o.transpose {
        (p[1], p[0])
    } else {
        (p[0], p[1])
    };
    [
        if o.flip_x { 1.0 - dx } else { dx },
        if o.flip_y { 1.0 - dy } else { dy },
    ]
}

/// The stored frame point shown at upright point `p` with `o`.
pub fn from_upright(o: Orient, p: [f32; 2]) -> [f32; 2] {
    let dx = if o.flip_x { 1.0 - p[0] } else { p[0] };
    let dy = if o.flip_y { 1.0 - p[1] } else { p[1] };
    if o.transpose { [dy, dx] } else { [dx, dy] }
}

/// A direction (degrees, clockwise, y down) of the stored image, as shown with `o`.
fn angle_upright(o: Orient, a: f32) -> f32 {
    let mut a = if o.transpose { 90.0 - a } else { a };
    if o.flip_x {
        a = 180.0 - a;
    }
    if o.flip_y {
        a = -a;
    }
    a
}

/// The inverse of [`angle_upright`].
fn angle_stored(o: Orient, a: f32) -> f32 {
    let mut a = a;
    if o.flip_y {
        a = -a;
    }
    if o.flip_x {
        a = 180.0 - a;
    }
    if o.transpose { 90.0 - a } else { a }
}

/// `s` with its points mapped by `p` and its angles by `a`. A reflection keeps every shape
/// the same set of points: ellipses, rectangles and radial gradients are symmetric about
/// their own axes.
fn map_shape(s: &Shape, p: &dyn Fn([f32; 2]) -> [f32; 2], a: &dyn Fn(f32) -> f32) -> Shape {
    let tidy = |d: f32| {
        let d = d.rem_euclid(360.0);
        if d > 180.0 { d - 360.0 } else { d }
    };
    match s {
        Shape::Brush { strokes } => Shape::Brush {
            strokes: strokes
                .iter()
                .map(|st| {
                    let mut st = st.clone();
                    for q in &mut st.points {
                        let m = p([q[0], q[1]]);
                        q[0] = m[0];
                        q[1] = m[1];
                    }
                    st
                })
                .collect(),
        },
        Shape::Ellipse {
            centre,
            radii,
            angle_deg,
            feather,
        } => Shape::Ellipse {
            centre: p(*centre),
            radii: *radii,
            angle_deg: tidy(a(*angle_deg)),
            feather: *feather,
        },
        Shape::Rectangle {
            centre,
            half,
            angle_deg,
            round,
            feather,
        } => Shape::Rectangle {
            centre: p(*centre),
            half: *half,
            angle_deg: tidy(a(*angle_deg)),
            round: *round,
            feather: *feather,
        },
        Shape::Polygon { points, closed } => Shape::Polygon {
            points: points
                .iter()
                .map(|q| {
                    let m = p([q.x, q.y]);
                    PathPoint {
                        x: m[0],
                        y: m[1],
                        feather: q.feather,
                    }
                })
                .collect(),
            closed: *closed,
        },
        Shape::Linear { start, end } => Shape::Linear {
            start: p(*start),
            end: p(*end),
        },
        Shape::Radial {
            centre,
            inner,
            outer,
            angle_deg,
        } => Shape::Radial {
            centre: p(*centre),
            inner: *inner,
            outer: *outer,
            angle_deg: tidy(a(*angle_deg)),
        },
    }
}

/// Renumbers the shapes `e` uses through `ids` (old → new).
fn renumber(e: &mut MaskExpr, ids: &[(u32, u32)]) {
    match e {
        MaskExpr::Shape(id) => {
            if let Some((_, n)) = ids.iter().find(|(o, _)| o == id) {
                *id = *n;
            }
        }
        MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => renumber(a, ids),
        MaskExpr::Combine(_, a, b) => {
            renumber(a, ids);
            renumber(b, ids);
        }
        _ => {}
    }
}

/// `e` with every "use mask from" replaced by the mask it borrows (a cycle or a missing
/// source masks nothing, as it renders).
fn resolve(edit: &Edit, e: &MaskExpr, depth: u32) -> MaskExpr {
    match e {
        MaskExpr::FromModule { kind, instance } => {
            match edit.module(*kind, *instance).and_then(|m| m.blend.as_ref()) {
                Some(b) if depth < 8 => resolve(edit, &b.combined(), depth + 1),
                _ => MaskExpr::Invert(Box::new(MaskExpr::All)),
            }
        }
        MaskExpr::Invert(a) => MaskExpr::Invert(Box::new(resolve(edit, a, depth))),
        MaskExpr::Opacity(k, a) => MaskExpr::Opacity(*k, Box::new(resolve(edit, a, depth))),
        MaskExpr::Combine(op, a, b) => MaskExpr::Combine(
            *op,
            Box::new(resolve(edit, a, depth)),
            Box::new(resolve(edit, b, depth)),
        ),
        e => e.clone(),
    }
}

impl MaskPreset {
    /// The mask of `kind`/`instance` in `edit` as a preset; `orient` is how the photo is
    /// shown. `None` when the module has no mask.
    pub fn from_module(
        name: &str,
        edit: &Edit,
        kind: ModuleKind,
        instance: u16,
        orient: Orient,
    ) -> Option<MaskPreset> {
        let b = edit.module(kind, instance)?.blend.as_ref()?;
        let mut mask = resolve(edit, &b.mask, 0);
        let mut used = Vec::new();
        mask.shape_ids(&mut used);
        let mut ids = Vec::new();
        let mut masks = Vec::new();
        for id in used {
            if ids.iter().any(|(o, _)| *o == id) {
                continue;
            }
            if let Some(s) = edit.mask(id) {
                let n = masks.len() as u32 + 1;
                ids.push((id, n));
                masks.push(MaskShape {
                    id: n,
                    shape: map_shape(&s.shape, &|p| to_upright(orient, p), &|a| {
                        angle_upright(orient, a)
                    }),
                });
            }
        }
        renumber(&mut mask, &ids);
        Some(MaskPreset {
            name: name.to_owned(),
            mask,
            invert: b.invert,
            refine: b.refine.clone(),
            masks,
        })
    }

    /// Puts the preset on `kind`/`instance` of `edit` (shown with `orient`): its shapes are
    /// added with new ids, its mask, invert and refinement replace the module's (mode and
    /// amount stay). Whether the module is there.
    pub fn apply(&self, edit: &mut Edit, kind: ModuleKind, instance: u16, orient: Orient) -> bool {
        if edit.module(kind, instance).is_none() {
            return false;
        }
        let mut ids = Vec::new();
        for (next, s) in (edit.next_mask_id()..).zip(self.masks.iter()) {
            ids.push((s.id, next));
            edit.masks.push(MaskShape {
                id: next,
                shape: map_shape(&s.shape, &|p| from_upright(orient, p), &|a| {
                    angle_stored(orient, a)
                }),
            });
        }
        let mut mask = self.mask.clone();
        renumber(&mut mask, &ids);
        let Some(m) = edit.module_mut(kind, instance) else {
            return false;
        };
        let old = m.blend.take();
        let mut gone = Vec::new();
        if let Some(b) = &old {
            b.mask.shape_ids(&mut gone);
        }
        let b = old.unwrap_or_default();
        m.blend = Some(Blend {
            mask,
            invert: self.invert,
            refine: self.refine.clone(),
            ..b
        });
        // The module's old shapes go unless another mask uses them.
        let mut used = Vec::new();
        for m in &edit.modules {
            if let Some(b) = &m.blend {
                b.mask.shape_ids(&mut used);
            }
        }
        edit.masks
            .retain(|s| !gone.contains(&s.id) || used.contains(&s.id));
        true
    }
}
