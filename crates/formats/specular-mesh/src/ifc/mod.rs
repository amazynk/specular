//! IFC building models (IFC2x3, IFC4, IFC4x3) in STEP physical file syntax, plain or as
//! `.ifczip`: an own reader ([`step`]) and own geometry for what building models use —
//! extruded, tapered and revolved area solids over every profile definition (arbitrary,
//! with voids, parameterised: rectangles, circles, I, L, T, U, C, Z, trapezium, composite,
//! derived), swept disks, fixed-reference and surface-curve sweeps, faceted B-reps, shell-
//! and face-based surface models, triangulated and polygonal face sets (with per-face
//! colours), CSG primitives, mapped items, local and grid placements, boolean clipping by
//! half-spaces (plain and polygonally bounded) and openings subtracted where they are made
//! of convex pieces. Advanced B-reps (NURBS) go to a [`BrepTessellator`] (OpenCASCADE in
//! the sandbox, through the decode layer) as a STEP file of their own; without one, their
//! plane faces are drawn.
//!
//! Elements are built in parallel. Colours come from styled items, else the element's
//! material, else a colour by the element's kind. Spaces, openings and other non-physical
//! elements are not drawn. Coordinates far from the origin (georeferenced models) are
//! shifted near it, and the shift is reported.

mod brep_step;
mod clip;
pub mod data;
mod geom;
pub mod georef;
mod profile;
mod solid;
pub(crate) mod step;
mod style;
mod tri2d;

pub(crate) use tri2d::triangulate;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::builder::Builder;
use crate::{Error, Limits, Material, Mesh, Result, Unit, Up};
use clip::{Plane, Region, subtract_region};
use geom::{Mat, P3, Shape, add, crease_normals, dot, sub};
use step::{Step, V};
use style::{Look, Styles};

/// Tessellates B-rep solids given as a STEP file (OpenCASCADE in the decode host).
pub trait BrepTessellator: Sync {
    /// The triangles of each product of `step` (an AP214 file whose products are named by
    /// the numbers they stand for), or why it could not.
    fn tessellate(&self, step: &[u8]) -> std::result::Result<Tessellation, String>;
}

/// What a [`BrepTessellator`] returns.
#[derive(Clone, Debug, Default)]
pub struct Tessellation {
    /// (product name, positions, three indices per triangle).
    pub parts: Vec<(String, Vec<[f32; 3]>, Vec<u32>)>,
    /// Metres per unit of the positions, the file's own length unit being declared as a
    /// metre.
    pub metres_per_unit: f64,
    /// Faces the tessellator could not mesh (left out).
    pub faces_unmeshed: u32,
}

/// One building element (or spatial element with geometry) as drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub id: u32,
    pub global_id: String,
    /// Entity type, upper case (`IFCWALLSTANDARDCASE`).
    pub kind: String,
    pub name: String,
    pub triangles: usize,
    /// Volume in the file's length unit cubed, when every piece is a closed shell.
    pub volume: Option<f64>,
    pub area: f64,
    /// Bounds in the file's length unit (before any shift).
    pub min: [f64; 3],
    pub max: [f64; 3],
    /// Its vertices in the mesh: `first_vertex .. first_vertex + vertices` (each element's
    /// vertices are its own and contiguous; the triangles are regrouped by material).
    pub first_vertex: u32,
    pub vertices: u32,
}

/// A row of the spatial tree: depth and label.
pub type TreeRow = (usize, String);

/// An IFC model read: the mesh, the facts for the info panel, the spatial tree and the
/// elements drawn.
#[derive(Debug, Default)]
pub struct IfcModel {
    pub mesh: Mesh,
    /// `IFC2X3`, `IFC4`, `IFC4X3_ADD2`…
    pub schema: String,
    /// The STEP header: (name, value) in file order.
    pub header: Vec<(String, String)>,
    /// Facts in display order.
    pub facts: Vec<(String, String)>,
    pub tree: Vec<TreeRow>,
    pub elements: Vec<Element>,
    /// Metres per length unit of the file.
    pub metres_per_unit: f64,
    /// Metres per unit of the mesh's positions, and where the mesh's origin is in the
    /// file's own frame, in metres (non-zero when far coordinates were shifted): a mesh
    /// point `p` is at `p * mesh_metres + origin_metres` in the project.
    pub mesh_metres: f64,
    pub origin_metres: [f64; 3],
    /// Where the model sits on the Earth (map conversion, CRS, site reference).
    pub georef: georef::Georef,
}

/// Entities never drawn: spaces, openings and other non-physical elements.
const HIDDEN: &[&str] = &[
    "IFCOPENINGELEMENT",
    "IFCOPENINGSTANDARDCASE",
    "IFCVOIDINGFEATURE",
    "IFCSPACE",
    "IFCSPATIALZONE",
    "IFCEXTERNALSPATIALELEMENT",
    "IFCANNOTATION",
    "IFCGRID",
    "IFCVIRTUALELEMENT",
    "IFCALIGNMENT",
    "IFCREFERENT",
    "IFCDISTRIBUTIONPORT",
];

fn hidden(ty: &str) -> bool {
    HIDDEN.contains(&ty) || ty.starts_with("IFCSTRUCTURAL") || ty.starts_with("IFCALIGNMENT")
}

/// Shared state while reading a file.
pub(crate) struct Ctx<'a> {
    pub s: Step<'a>,
    /// Radians per plane angle unit.
    pub angle: f64,
    /// Metres per length unit.
    pub length: f64,
    /// Length tolerance in file units.
    pub eps: f64,
    /// Points per full circle.
    pub segments: usize,
    pub styles: Styles,
    /// Face set → its `IfcIndexedColourMap`.
    pub colour_maps: HashMap<u32, u32>,
    /// Grid axis → the grid's placement and the axis curve (for grid placements).
    grid_axes: HashMap<u32, u32>,
    /// Representation → its pieces (mapped items share them).
    mapped: Mutex<HashMap<u32, Arc<Vec<Piece>>>>,
    /// Counted notes ("IFCSECTIONEDSPINE not drawn").
    notes: Mutex<BTreeMap<String, usize>>,
    /// Whether advanced B-reps are collected for the tessellator.
    pub defer_breps: bool,
    /// What reading the geometry may still cost ([`Work`]): shared references (a mapped item
    /// listing the same representation twice, a boolean of an item with itself) multiply it.
    pub work: Work,
}

/// A file's geometry budget. References can share targets, so a few hundred bytes can
/// describe exponentially many visits; every item, curve and profile visited costs one
/// `call`, and every point, vertex and triangle copied costs one `unit`. Both are tied to
/// the file (calls to its entity count, units to the mesh limits), so a valid model never
/// meets them.
pub(crate) struct Work {
    calls: std::sync::atomic::AtomicU64,
    units: std::sync::atomic::AtomicU64,
    max_calls: u64,
    max_units: u64,
}

impl Work {
    /// Visits per entity a model may make (mapped representations are visited once).
    const CALLS_PER_ENTITY: u64 = 16;

    pub fn new(entities: usize, limits: &Limits) -> Work {
        Work {
            calls: 0.into(),
            units: 0.into(),
            max_calls: (entities as u64).saturating_mul(Self::CALLS_PER_ENTITY) + 100_000,
            max_units: 2 * (limits.max_triangles as u64 + limits.max_vertices as u64),
        }
    }

    /// Spends one visit; false once the budget is gone.
    pub fn call(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.calls.fetch_add(1, Relaxed) < self.max_calls
    }

    /// Spends `n` copied points, vertices or triangles; false once the budget is gone.
    pub fn copy(&self, n: usize) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.units
            .fetch_add(n as u64, Relaxed)
            .saturating_add(n as u64)
            <= self.max_units
    }

    /// The budget ran out (part of the model was not drawn).
    pub fn spent(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.calls.load(Relaxed) >= self.max_calls || self.units.load(Relaxed) > self.max_units
    }
}

/// A shape in item coordinates with its look.
#[derive(Clone, Debug)]
pub(crate) struct Piece {
    pub shape: Shape,
    pub look: Option<u32>,
}

/// An advanced B-rep waiting for the tessellator: the item, where it goes and its look.
#[derive(Clone, Debug)]
struct Deferred {
    item: u32,
    xf: Mat,
    look: Option<u32>,
}

/// One element's output: pieces in world coordinates (file units).
struct Built {
    product: u32,
    pieces: Vec<Piece>,
    deferred: Vec<Deferred>,
    openings: (usize, usize),
    /// The product's placement.
    world: Mat,
}

impl<'a> Ctx<'a> {
    pub fn note(&self, what: impl Into<String>) {
        if let Ok(mut n) = self.notes.lock() {
            *n.entry(what.into()).or_default() += 1;
        }
    }

    /// The pieces of geometric item `id`, in its representation's coordinates.
    fn item(&self, id: u32, depth: usize, deferred: &mut Vec<Deferred>, xf: &Mat) -> Vec<Piece> {
        if depth > 24 || !self.work.call() {
            return Vec::new();
        }
        let Some(ty) = self.s.ty(id) else {
            return Vec::new();
        };
        let look = self.styles.item.get(&id).copied();
        let one = |s: Option<Shape>| -> Vec<Piece> {
            match s {
                Some(shape) if !shape.is_empty() && shape.is_finite() => {
                    vec![Piece { shape, look }]
                }
                _ => Vec::new(),
            }
        };
        match ty {
            "IFCMAPPEDITEM" => {
                let Some(a) = self.s.args(id) else {
                    return Vec::new();
                };
                let (Some(src), Some(target)) =
                    (a.first().and_then(V::as_ref), a.get(1).and_then(V::as_ref))
                else {
                    return Vec::new();
                };
                let Some(sa) = self.s.args(src) else {
                    return Vec::new();
                };
                // IfcRepresentationMap(MappingOrigin, MappedRepresentation)
                let origin = self.placement(sa.first().unwrap_or(&V::Null));
                let Some(rep) = sa.get(1).and_then(V::as_ref) else {
                    return Vec::new();
                };
                let m = self.operator(target).mul(&origin);
                let inner_xf = xf.mul(&m);
                let pieces = self.representation(rep, depth + 1, deferred, &inner_xf);
                let size: usize = pieces
                    .iter()
                    .map(|p| p.shape.pos.len() + p.shape.tri.len())
                    .sum();
                if !self.work.copy(size) {
                    return Vec::new();
                }
                pieces
                    .iter()
                    .map(|p| {
                        let mut s = p.shape.clone();
                        s.transform(&m);
                        Piece {
                            shape: s,
                            look: p.look.or(look),
                        }
                    })
                    .collect()
            }
            "IFCEXTRUDEDAREASOLID" | "IFCEXTRUDEDAREASOLIDTAPERED" => one(self.extruded(id, ty)),
            "IFCREVOLVEDAREASOLID" | "IFCREVOLVEDAREASOLIDTAPERED" => one(self.revolved(id)),
            "IFCSWEPTDISKSOLID" | "IFCSWEPTDISKSOLIDPOLYGONAL" => one(self.swept_disk(id)),
            "IFCFIXEDREFERENCESWEPTAREASOLID" | "IFCSURFACECURVESWEPTAREASOLID" => {
                one(self.swept_area(id, ty))
            }
            "IFCFACETEDBREP" | "IFCFACETEDBREPWITHVOIDS" => one(self.faceted_brep(id, ty)),
            "IFCADVANCEDBREP" | "IFCADVANCEDBREPWITHVOIDS" => {
                if self.defer_breps {
                    deferred.push(Deferred {
                        item: id,
                        xf: *xf,
                        look,
                    });
                    Vec::new()
                } else {
                    let s = self.planar_advanced(id);
                    if s.as_ref().is_none_or(Shape::is_empty) {
                        self.note(
                            "advanced B-reps (curved surfaces) not drawn: OpenCASCADE is needed",
                        );
                    }
                    one(s)
                }
            }
            "IFCSHELLBASEDSURFACEMODEL" | "IFCFACEBASEDSURFACEMODEL" => {
                let Some(a) = self.s.args(id) else {
                    return Vec::new();
                };
                let mut s = Shape::default();
                for sh in a.first().map(V::refs).unwrap_or_default() {
                    self.shell(sh, &mut s);
                }
                s.closed = solid::is_closed(&s);
                if s.closed {
                    s.orient_outward();
                }
                one(Some(s))
            }
            "IFCTRIANGULATEDFACESET"
            | "IFCPOLYGONALFACESET"
            | "IFCTRIANGULATEDIRREGULARNETWORK" => {
                let mut s = self.face_set(id, ty);
                if let Some(s) = &mut s
                    && s.closed
                {
                    s.orient_outward();
                }
                one(s)
            }
            "IFCBLOCK"
            | "IFCRIGHTCIRCULARCYLINDER"
            | "IFCRIGHTCIRCULARCONE"
            | "IFCSPHERE"
            | "IFCRECTANGULARPYRAMID" => one(self.primitive(id, ty)),
            "IFCCSGSOLID" => {
                let root = self.s.args(id).and_then(|a| a.first().and_then(V::as_ref));
                match root {
                    Some(r) => self
                        .item(r, depth + 1, deferred, xf)
                        .into_iter()
                        .map(|p| Piece {
                            look: p.look.or(look),
                            ..p
                        })
                        .collect(),
                    None => Vec::new(),
                }
            }
            "IFCBOOLEANRESULT" | "IFCBOOLEANCLIPPINGRESULT" => self
                .boolean(id, depth, deferred, xf)
                .into_iter()
                .map(|p| Piece {
                    look: p.look.or(look),
                    ..p
                })
                .collect(),
            "IFCPOLYLINE"
            | "IFCINDEXEDPOLYCURVE"
            | "IFCCOMPOSITECURVE"
            | "IFCTRIMMEDCURVE"
            | "IFCCIRCLE"
            | "IFCLINE"
            | "IFCGEOMETRICCURVESET"
            | "IFCGEOMETRICSET"
            | "IFCCARTESIANPOINT"
            | "IFCTEXTLITERAL"
            | "IFCTEXTLITERALWITHEXTENT"
            | "IFCANNOTATIONFILLAREA"
            | "IFCBOUNDINGBOX"
            | "IFCPOINTONCURVE"
            | "IFCGRADIENTCURVE"
            | "IFCSEGMENTEDREFERENCECURVE"
            | "IFCBSPLINECURVEWITHKNOTS" => Vec::new(),
            "IFCHALFSPACESOLID" | "IFCPOLYGONALBOUNDEDHALFSPACE" | "IFCBOXEDHALFSPACE" => {
                Vec::new()
            }
            other => {
                self.note(format!("{} not drawn", display_kind(other)));
                Vec::new()
            }
        }
    }

    /// Every item of a shape representation (mapped representations are cached).
    fn representation(
        &self,
        rep: u32,
        depth: usize,
        deferred: &mut Vec<Deferred>,
        xf: &Mat,
    ) -> Arc<Vec<Piece>> {
        if let Some(p) = self.mapped.lock().ok().and_then(|m| m.get(&rep).cloned()) {
            // Deferred items of a cached representation are found again (cheap).
            if self.defer_breps {
                let mut d = Vec::new();
                self.collect_deferred(rep, depth, &mut d, xf);
                deferred.extend(d);
            }
            return p;
        }
        let items = self
            .s
            .args(rep)
            .and_then(|a| a.get(3).map(V::refs))
            .unwrap_or_default();
        let mut out = Vec::new();
        for it in items {
            out.extend(self.item(it, depth + 1, deferred, xf));
        }
        let out = Arc::new(out);
        if let Ok(mut m) = self.mapped.lock() {
            m.insert(rep, out.clone());
        }
        out
    }

    /// The advanced B-reps below a representation, with their maps.
    fn collect_deferred(&self, rep: u32, depth: usize, d: &mut Vec<Deferred>, xf: &Mat) {
        if depth > 24 || !self.work.call() {
            return;
        }
        for it in self
            .s
            .args(rep)
            .and_then(|a| a.get(3).map(V::refs))
            .unwrap_or_default()
        {
            match self.s.ty(it) {
                Some("IFCADVANCEDBREP" | "IFCADVANCEDBREPWITHVOIDS") => d.push(Deferred {
                    item: it,
                    xf: *xf,
                    look: self.styles.item.get(&it).copied(),
                }),
                Some("IFCMAPPEDITEM") => {
                    let Some(a) = self.s.args(it) else { continue };
                    let (Some(src), Some(target)) =
                        (a.first().and_then(V::as_ref), a.get(1).and_then(V::as_ref))
                    else {
                        continue;
                    };
                    let Some(sa) = self.s.args(src) else { continue };
                    let origin = self.placement(sa.first().unwrap_or(&V::Null));
                    let m = self.operator(target).mul(&origin);
                    if let Some(r) = sa.get(1).and_then(V::as_ref) {
                        self.collect_deferred(r, depth + 1, d, &xf.mul(&m));
                    }
                }
                _ => {}
            }
        }
    }

    fn extruded(&self, id: u32, ty: &str) -> Option<Shape> {
        // (SweptArea, Position, ExtrudedDirection, Depth[, EndSweptArea])
        let a = self.s.args(id)?;
        let regions = self.profile(a.first()?.as_ref()?, 0)?;
        let m = self.placement(a.get(1).unwrap_or(&V::Null));
        let dir = a
            .get(2)
            .and_then(V::as_ref)
            .and_then(|d| self.direction(d))
            .unwrap_or([0.0, 0.0, 1.0]);
        let depth = a.get(3).and_then(V::as_f64)?;
        if !(depth.is_finite() && depth.abs() > 0.0) {
            return None;
        }
        let v = geom::scale(dir, depth);
        if ty == "IFCEXTRUDEDAREASOLIDTAPERED"
            && let Some(end) = a
                .get(4)
                .and_then(V::as_ref)
                .and_then(|p| self.profile(p, 0))
            && let Some(s) = solid::loft(&regions, &end, &m, v)
        {
            return Some(s);
        }
        Some(solid::extrude(&regions, &m, v))
    }

    fn revolved(&self, id: u32) -> Option<Shape> {
        // (SweptArea, Position, Axis, Angle[, EndSweptArea])
        let a = self.s.args(id)?;
        let regions = self.profile(a.first()?.as_ref()?, 0)?;
        let m = self.placement(a.get(1).unwrap_or(&V::Null));
        let axis = a.get(2)?.as_ref()?;
        let aa = self.s.args(axis)?;
        let o = aa
            .first()
            .and_then(V::as_ref)
            .and_then(|p| self.point(p))
            .unwrap_or([0.0; 3]);
        let d = aa
            .get(1)
            .and_then(V::as_ref)
            .and_then(|d| self.direction(d))
            .unwrap_or([0.0, 0.0, 1.0]);
        let angle = a.get(3).and_then(V::as_f64)? * self.angle;
        Some(solid::revolve(
            &regions,
            &m,
            o,
            d,
            angle,
            self.arc_steps(angle).max(3),
        ))
    }

    fn swept_disk(&self, id: u32) -> Option<Shape> {
        // (Directrix, Radius, InnerRadius, StartParam, EndParam[, FilletRadius])
        let a = self.s.args(id)?;
        let mut path = Vec::new();
        if !self.curve(a.first()?.as_ref()?, &mut path, 0) {
            return None;
        }
        let r = a.get(1).and_then(V::as_f64)?;
        let inner = a.get(2).and_then(V::as_f64).unwrap_or(0.0);
        let seg = (self.segments / 2).max(8);
        Some(solid::swept_disk(&path, r, inner, seg))
    }

    fn swept_area(&self, id: u32, ty: &str) -> Option<Shape> {
        // (SweptArea, Position, Directrix, StartParam, EndParam, FixedReference |
        //  ReferenceSurface)
        let a = self.s.args(id)?;
        let regions = self.profile(a.first()?.as_ref()?, 0)?;
        let m = self.placement(a.get(1).unwrap_or(&V::Null));
        let mut path = Vec::new();
        if !self.curve(a.get(2)?.as_ref()?, &mut path, 0) {
            return None;
        }
        let up = if ty == "IFCFIXEDREFERENCESWEPTAREASOLID" {
            // The profile's x follows the fixed reference: its y is the tangent × it, which
            // for a plane path is the plane's normal.
            let fixed = a.get(5).and_then(V::as_ref).and_then(|d| self.direction(d));
            fixed.and_then(|f| {
                let t = geom::unit(sub(*path.last()?, path[0]))?;
                geom::unit(geom::cross(t, f))
            })
        } else {
            // A plane reference surface: its normal.
            a.get(5).and_then(V::as_ref).and_then(|srf| {
                (self.s.ty(srf) == Some("IFCPLANE")).then(|| {
                    let pm = self.placement(self.s.args(srf)?.first()?);
                    geom::unit(pm.vector([0.0, 0.0, 1.0]))
                })?
            })
        };
        let mut s = solid::sweep_profile(&regions, &path, up);
        s.transform(&m);
        Some(s)
    }

    fn faceted_brep(&self, id: u32, ty: &str) -> Option<Shape> {
        let a = self.s.args(id)?;
        let mut s = Shape::default();
        self.shell(a.first()?.as_ref()?, &mut s);
        s.closed = solid::is_closed(&s);
        if s.closed {
            s.orient_outward();
        }
        if ty == "IFCFACETEDBREPWITHVOIDS" {
            for v in a.get(1).map(V::refs).unwrap_or_default() {
                let mut h = Shape::default();
                self.shell(v, &mut h);
                if solid::is_closed(&h) {
                    h.orient_outward();
                    h.flip();
                    h.closed = true;
                }
                s.append(&h);
            }
        }
        Some(s)
    }

    /// The faces of an advanced B-rep that lie on planes, bounded by their edges' curves
    /// (used without a tessellator).
    fn planar_advanced(&self, id: u32) -> Option<Shape> {
        let a = self.s.args(id)?;
        let shell = self.s.args(a.first()?.as_ref()?)?;
        let mut s = Shape::default();
        let mut skipped = false;
        for f in shell.first().map(V::refs).unwrap_or_default() {
            let Some(fa) = self.s.args(f) else { continue };
            let surface = fa.get(1).and_then(V::as_ref);
            if !surface.is_some_and(|x| self.flat_surface(x)) {
                skipped = true;
                continue;
            }
            let same = fa.get(2).and_then(V::as_bool).unwrap_or(true);
            let mut outer: Option<Vec<P3>> = None;
            let mut holes = Vec::new();
            for b in fa.first().map(V::refs).unwrap_or_default() {
                let Some(ba) = self.s.args(b) else { continue };
                let orient = ba.get(1).and_then(V::as_bool).unwrap_or(true);
                let Some(lp) = ba.first().and_then(V::as_ref) else {
                    continue;
                };
                let mut ring = self.edge_loop(lp)?;
                if !orient {
                    ring.reverse();
                }
                if self.s.ty(b) == Some("IFCFACEOUTERBOUND") && outer.is_none() {
                    outer = Some(ring);
                } else {
                    holes.push(ring);
                }
            }
            let outer = match outer {
                Some(o) => o,
                None if !holes.is_empty() => holes.remove(0),
                None => continue,
            };
            let mut t = Shape::default();
            solid::face(&mut t, &outer, &holes);
            if !same {
                t.flip();
            }
            s.append(&t);
        }
        if skipped {
            self.note("curved faces of advanced B-reps not drawn: OpenCASCADE is needed");
        }
        s.closed = solid::is_closed(&s);
        Some(s)
    }

    /// Whether a face's surface is flat: a plane, or a B-spline surface whose control points
    /// all lie on one plane (exporters write flat faces that way too; their bounds give the
    /// face, as for a plane).
    fn flat_surface(&self, id: u32) -> bool {
        match self.s.ty(id) {
            Some("IFCPLANE") => true,
            Some("IFCBSPLINESURFACEWITHKNOTS" | "IFCRATIONALBSPLINESURFACEWITHKNOTS") => {
                let Some(a) = self.s.args(id) else {
                    return false;
                };
                let pts: Vec<P3> = a
                    .get(2)
                    .and_then(V::as_list)
                    .unwrap_or_default()
                    .iter()
                    .flat_map(V::refs)
                    .filter_map(|p| self.point(p))
                    .collect();
                coplanar(&pts, self.eps)
            }
            _ => false,
        }
    }

    /// The points around an `IfcEdgeLoop` (or a poly loop).
    fn edge_loop(&self, id: u32) -> Option<Vec<P3>> {
        let a = self.s.args(id)?;
        if self.s.ty(id) == Some("IFCPOLYLOOP") {
            return Some(
                a.first()?
                    .refs()
                    .into_iter()
                    .filter_map(|p| self.point(p))
                    .collect(),
            );
        }
        let mut out: Vec<P3> = Vec::new();
        for oe in a.first()?.refs() {
            // IfcOrientedEdge(EdgeStart*, EdgeEnd*, EdgeElement, Orientation)
            let oa = self.s.args(oe)?;
            let (edge, forward) = if self.s.ty(oe) == Some("IFCORIENTEDEDGE") {
                (
                    oa.get(2)?.as_ref()?,
                    oa.get(3).and_then(V::as_bool).unwrap_or(true),
                )
            } else {
                (oe, true)
            };
            let ea = self.s.args(edge)?;
            // IfcEdgeCurve(EdgeStart, EdgeEnd, EdgeGeometry, SameSense)
            let vtx = |k: usize| -> Option<P3> {
                let v = ea.get(k)?.as_ref()?;
                self.point(self.s.args(v)?.first()?.as_ref()?)
            };
            let (p0, p1) = (vtx(0)?, vtx(1)?);
            let mut pts = Vec::new();
            let same = ea.get(3).and_then(V::as_bool).unwrap_or(true);
            let curve = ea.get(2).and_then(V::as_ref);
            let sampled = curve.is_some_and(|c| {
                !matches!(self.s.ty(c), Some("IFCLINE")) && self.curve(c, &mut pts, 0)
            });
            if !sampled || pts.len() < 2 {
                pts = vec![p0, p1];
            } else {
                if !same {
                    pts.reverse();
                }
                // Trim a closed or longer curve to the edge's vertices.
                pts = trim_to(&pts, p0, p1);
            }
            if !forward {
                pts.reverse();
            }
            for p in pts {
                if out.last().is_none_or(|l| geom::len(sub(*l, p)) > self.eps) {
                    out.push(p);
                }
            }
        }
        while out.len() > 1 && geom::len(sub(out[0], out[out.len() - 1])) <= self.eps {
            out.pop();
        }
        (out.len() >= 3).then_some(out)
    }

    fn boolean(&self, id: u32, depth: usize, deferred: &mut Vec<Deferred>, xf: &Mat) -> Vec<Piece> {
        let Some(a) = self.s.args(id) else {
            return Vec::new();
        };
        let op = a.first().and_then(V::as_enum).unwrap_or(b"DIFFERENCE");
        let (Some(first), Some(second)) =
            (a.get(1).and_then(V::as_ref), a.get(2).and_then(V::as_ref))
        else {
            return Vec::new();
        };
        let mut pieces = self.item(first, depth + 1, deferred, xf);
        if op == b"UNION" {
            pieces.extend(self.item(second, depth + 1, deferred, xf));
            return pieces;
        }
        let Some(regions) = self.convex_regions(second, depth + 1, pieces.iter().map(|p| &p.shape))
        else {
            self.note(format!(
                "{} operands not subtracted",
                display_kind(self.s.ty(second).unwrap_or("?"))
            ));
            return pieces;
        };
        let tol = self.eps * 1e-3;
        pieces
            .into_iter()
            .flat_map(|p| {
                if !p.shape.closed {
                    return vec![p];
                }
                let look = p.look;
                let shapes = if op == b"INTERSECTION" {
                    // Inside every region's planes (one region is all this handles well).
                    regions
                        .first()
                        .map(|r| {
                            let mut s = p.shape.clone();
                            for pl in &r.planes {
                                s = clip::clip(&s, pl, tol);
                            }
                            vec![s]
                        })
                        .unwrap_or_default()
                } else {
                    let mut cur = vec![p.shape];
                    for r in &regions {
                        cur = subtract_region(cur, r, tol);
                    }
                    cur
                };
                shapes
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .map(|shape| Piece { shape, look })
                    .collect()
            })
            .collect()
    }

    /// A boolean's second operand as convex regions (planes, bounds): half-spaces, half-
    /// spaces bounded by a polygon (one region per triangle of a concave one), and solids
    /// that are convex or extrusions of profiles (one region per profile triangle when
    /// concave). `first` (the shapes being cut) decides whether a bounded half-space can
    /// be taken whole.
    fn convex_regions<'s>(
        &self,
        id: u32,
        depth: usize,
        first: impl Iterator<Item = &'s Shape>,
    ) -> Option<Vec<Region>> {
        let ty = self.s.ty(id)?;
        let a = self.s.args(id)?;
        match ty {
            "IFCHALFSPACESOLID" | "IFCBOXEDHALFSPACE" | "IFCPOLYGONALBOUNDEDHALFSPACE" => {
                let srf = a.first()?.as_ref()?;
                if self.s.ty(srf) != Some("IFCPLANE") {
                    return None;
                }
                let pm = self.placement(self.s.args(srf)?.first()?);
                let n = geom::unit(pm.vector([0.0, 0.0, 1.0]))?;
                let o = pm.origin();
                // AgreementFlag true: the solid is on the side away from the normal.
                let agree = a.get(1).and_then(V::as_bool).unwrap_or(true);
                let base = if agree {
                    Plane { n, d: dot(n, o) }
                } else {
                    Plane {
                        n: geom::scale(n, -1.0),
                        d: -dot(n, o),
                    }
                };
                if ty != "IFCPOLYGONALBOUNDEDHALFSPACE" {
                    return Some(vec![Region::new(vec![base], None)]);
                }
                let bm = self.placement(a.get(2).unwrap_or(&V::Null));
                let ring = self.ring(a.get(3)?.as_ref()?)?;
                let pts: Vec<P3> = ring.iter().map(|p| bm.point([p[0], p[1], 0.0])).collect();
                let axis = geom::unit(bm.vector([0.0, 0.0, 1.0]))?;
                // Side planes grown a little: boundaries flush with the cut shape's faces
                // (the usual case) then do not cut along them.
                let grow = self.eps * 10.0;
                let prism = |tri: &[P3]| -> Vec<Plane> {
                    let mut planes = vec![base];
                    let ccw = geom::dot(geom::newell(tri), axis) > 0.0;
                    for k in 0..tri.len() {
                        let (p, q) = (tri[k], tri[(k + 1) % tri.len()]);
                        let e = sub(q, p);
                        let mut n = geom::cross(e, axis);
                        if !ccw {
                            n = geom::scale(n, -1.0);
                        }
                        if let Some(n) = geom::unit(n) {
                            planes.push(Plane {
                                n,
                                d: dot(n, p) + grow,
                            });
                        }
                    }
                    planes
                };
                let shapes: Vec<&Shape> = first.collect();
                if convex_ring(&ring) {
                    let planes = prism(&pts);
                    // All of the cut shapes inside the boundary: the plain half-space.
                    let inside = shapes.iter().all(|s| {
                        s.pos
                            .iter()
                            .all(|&p| planes[1..].iter().all(|pl| dot(pl.n, p) - pl.d <= 0.0))
                    });
                    if inside {
                        return Some(vec![Region::new(vec![base], None)]);
                    }
                    return Some(vec![Region::new(planes, None)]);
                }
                let inv = bm.inverse()?;
                let all_inside = shapes.iter().all(|s| {
                    s.pos.iter().all(|&p| {
                        let q = inv.point(p);
                        geom::in_ring([q[0], q[1]], &ring)
                    })
                });
                if all_inside {
                    return Some(vec![Region::new(vec![base], None)]);
                }
                let tris = tri2d::triangulate(&ring, &[]);
                Some(
                    tris.iter()
                        .map(|t| Region::new(prism(&t.map(|i| pts[i as usize])), None))
                        .collect(),
                )
            }
            "IFCEXTRUDEDAREASOLID" => {
                let regions = self.profile(a.first()?.as_ref()?, 0)?;
                let m = self.placement(a.get(1).unwrap_or(&V::Null));
                let dir = a
                    .get(2)
                    .and_then(V::as_ref)
                    .and_then(|d| self.direction(d))
                    .unwrap_or([0.0, 0.0, 1.0]);
                let v = geom::scale(dir, a.get(3).and_then(V::as_f64)?);
                self.extrusion_regions(&regions, &m, v, 0.0)
            }
            "IFCBOOLEANRESULT" | "IFCBOOLEANCLIPPINGRESULT" if depth < 8 => {
                // A union of operands subtracts both.
                if a.first().and_then(V::as_enum) == Some(b"UNION") {
                    let x =
                        self.convex_regions(a.get(1)?.as_ref()?, depth + 1, std::iter::empty())?;
                    let y =
                        self.convex_regions(a.get(2)?.as_ref()?, depth + 1, std::iter::empty())?;
                    Some(x.into_iter().chain(y).collect())
                } else {
                    None
                }
            }
            _ => {
                // Any other solid that comes out convex.
                let mut d = Vec::new();
                let pieces = self.item(id, depth + 1, &mut d, &Mat::IDENTITY);
                let mut out = Vec::new();
                for p in pieces {
                    out.push(Region::new(
                        convex_planes(&p.shape, self.eps)?,
                        p.shape.bounds(),
                    ));
                }
                (!out.is_empty()).then_some(out)
            }
        }
    }

    /// Convex regions of an extrusion, each region enlarged by `grow`.
    fn extrusion_regions(
        &self,
        regions: &[profile::Region],
        m: &Mat,
        v: P3,
        grow: f64,
    ) -> Option<Vec<Region>> {
        let mut out = Vec::new();
        for r in regions {
            let parts: Vec<Vec<geom::P2>> = if r.holes.is_empty() && convex_ring(&r.outer) {
                // Round openings: an inscribed polygon of at most MAX_SIDES sides (the
                // filling window or door covers the difference).
                let n = r.outer.len();
                if n > MAX_SIDES {
                    vec![(0..MAX_SIDES).map(|k| r.outer[k * n / MAX_SIDES]).collect()]
                } else {
                    vec![r.outer.clone()]
                }
            } else {
                let all: Vec<geom::P2> = r
                    .outer
                    .iter()
                    .chain(r.holes.iter().flatten())
                    .copied()
                    .collect();
                tri2d::triangulate(&r.outer, &r.holes)
                    .into_iter()
                    .map(|t| t.iter().map(|&i| all[i as usize]).collect())
                    .collect()
            };
            if parts.len() > MAX_PARTS {
                return None;
            }
            for ring in parts {
                let shape = solid::extrude(
                    &[profile::Region {
                        outer: ring.clone(),
                        holes: Vec::new(),
                    }],
                    m,
                    v,
                );
                if let Some(mut planes) = convex_planes(&shape, self.eps) {
                    for p in &mut planes {
                        p.d += grow;
                    }
                    let region = Region::new(planes, shape.bounds());
                    out.push(region);
                }
            }
        }
        (out.len() <= MAX_PARTS).then_some(out)
    }

    /// An element's placement in the world.
    pub fn object_placement(&self, id: u32, depth: usize) -> Mat {
        if depth > 64 {
            return Mat::IDENTITY;
        }
        let Some(a) = self.s.args(id) else {
            return Mat::IDENTITY;
        };
        match self.s.ty(id) {
            Some("IFCLOCALPLACEMENT") => {
                let parent = a
                    .first()
                    .and_then(V::as_ref)
                    .map_or(Mat::IDENTITY, |p| self.object_placement(p, depth + 1));
                parent.mul(&self.placement(a.get(1).unwrap_or(&V::Null)))
            }
            Some("IFCGRIDPLACEMENT") => self.grid_placement(&a, depth),
            Some("IFCLINEARPLACEMENT") => {
                // (PlacementRelTo, RelativePlacement, CartesianPosition)
                let parent = a
                    .first()
                    .and_then(V::as_ref)
                    .map_or(Mat::IDENTITY, |p| self.object_placement(p, depth + 1));
                match a.get(2).and_then(V::as_ref) {
                    Some(c) => parent.mul(&self.placement_id(c)),
                    None => {
                        self.note("linear placements along alignments not computed");
                        parent
                    }
                }
            }
            _ => Mat::IDENTITY,
        }
    }

    fn grid_placement(&self, a: &[V], depth: usize) -> Mat {
        // (PlacementRelTo (IFC4), PlacementLocation, PlacementRefDirection) or, in IFC2x3,
        // (PlacementLocation, PlacementRefDirection).
        let loc = a
            .iter()
            .filter_map(V::as_ref)
            .find(|&r| self.s.ty(r) == Some("IFCVIRTUALGRIDINTERSECTION"));
        let Some(loc) = loc else {
            return Mat::IDENTITY;
        };
        let Some(la) = self.s.args(loc) else {
            return Mat::IDENTITY;
        };
        let axes = la.first().map(V::refs).unwrap_or_default();
        let offsets = la.get(1).map(V::reals).unwrap_or_default();
        if axes.len() < 2 {
            return Mat::IDENTITY;
        }
        let line = |ax: u32| -> Option<(P3, P3)> {
            let aa = self.s.args(ax)?;
            let mut pts = Vec::new();
            self.curve(aa.get(1)?.as_ref()?, &mut pts, 0)
                .then_some(())?;
            let (p, q) = (*pts.first()?, *pts.last()?);
            let same = aa.get(2).and_then(V::as_bool).unwrap_or(true);
            Some(if same { (p, q) } else { (q, p) })
        };
        let (Some((p1, q1)), Some((p2, q2))) = (line(axes[0]), line(axes[1])) else {
            return Mat::IDENTITY;
        };
        // Intersection of the two lines in the plane.
        let (d1, d2) = (sub(q1, p1), sub(q2, p2));
        let den = d1[0] * d2[1] - d1[1] * d2[0];
        let x = if den.abs() > 1e-300 {
            let t = ((p2[0] - p1[0]) * d2[1] - (p2[1] - p1[1]) * d2[0]) / den;
            add(p1, geom::scale(d1, t))
        } else {
            p1
        };
        let mut origin = x;
        if let Some(&z) = offsets.get(2) {
            origin[2] += z;
        }
        let xdir = geom::unit([d1[0], d1[1], 0.0]).unwrap_or([1.0, 0.0, 0.0]);
        let local = Mat::from_axes(xdir, [-xdir[1], xdir[0], 0.0], [0.0, 0.0, 1.0], origin);
        let grid = self
            .grid_axes
            .get(&axes[0])
            .map_or(Mat::IDENTITY, |&g| self.object_placement(g, depth + 1));
        grid.mul(&local)
    }

    /// The shape representations of a product to draw: its body (or facetation, or any
    /// three-dimensional one).
    fn body_representations(&self, pds: u32) -> Vec<u32> {
        let Some(a) = self.s.args(pds) else {
            return Vec::new();
        };
        let reps = a.get(2).map(V::refs).unwrap_or_default();
        let mut best: Option<(u32, Vec<u32>)> = None;
        for r in reps {
            let Some(ra) = self.s.args(r) else { continue };
            let ident = ra.get(1).and_then(V::as_str).unwrap_or_default();
            let kind = ra.get(2).and_then(V::as_str).unwrap_or_default();
            let ctx_ty = ra
                .first()
                .and_then(V::as_ref)
                .and_then(|c| self.s.args(c))
                .and_then(|ca| ca.get(1).and_then(V::as_str))
                .unwrap_or_default();
            if ctx_ty.eq_ignore_ascii_case("Plan") {
                continue;
            }
            let score = match ident.as_str() {
                "Body" => 0,
                "Facetation" => 1,
                "Mesh" | "Reference" | "Surface" => 2,
                "Axis" | "FootPrint" | "Box" | "Annotation" | "Profile" | "CoG" | "Clearance"
                | "Lighting" | "Plan" => continue,
                _ => match kind.as_str() {
                    "Curve" | "Curve2D" | "Curve3D" | "GeometricCurveSet" | "GeometricSet"
                    | "Annotation2D" | "Point" | "PointCloud" | "BoundingBox" | "Text" => {
                        continue;
                    }
                    _ => 3,
                },
            };
            match &mut best {
                Some((s, v)) if *s == score => v.push(r),
                Some((s, _)) if *s < score => {}
                _ => best = Some((score, vec![r])),
            }
        }
        best.map(|(_, v)| v).unwrap_or_default()
    }
}

/// The part of a polyline between the points nearest `p0` and `p1`.
/// Whether `pts` (at least three, not all on a line) lie on one plane within `eps`.
fn coplanar(pts: &[P3], eps: f64) -> bool {
    let Some(&o) = pts.first() else {
        return false;
    };
    // The normal from the first point and the two that span the largest triangle with it.
    let far = pts
        .iter()
        .copied()
        .max_by(|a, b| geom::len(sub(*a, o)).total_cmp(&geom::len(sub(*b, o))))
        .unwrap_or(o);
    let u = sub(far, o);
    let Some(n) = pts
        .iter()
        .map(|p| geom::cross(u, sub(*p, o)))
        .max_by(|a, b| geom::len(*a).total_cmp(&geom::len(*b)))
    else {
        return false;
    };
    let nl = geom::len(n);
    if !(nl.is_finite() && nl > eps * geom::len(u).max(eps)) {
        return false;
    }
    let n = n.map(|v| v / nl);
    let tol = eps.max(geom::len(u) * 1e-6);
    pts.iter().all(|p| dot(sub(*p, o), n).abs() <= tol)
}

fn trim_to(pts: &[P3], p0: P3, p1: P3) -> Vec<P3> {
    let near = |q: P3| {
        (0..pts.len())
            .min_by(|&a, &b| geom::len(sub(pts[a], q)).total_cmp(&geom::len(sub(pts[b], q))))
            .unwrap_or(0)
    };
    let (i, j) = (near(p0), near(p1));
    let mut out = vec![p0];
    if i < j {
        out.extend_from_slice(&pts[i + 1..j]);
    } else if i > j {
        // A closed curve run through its end.
        out.extend_from_slice(&pts[i + 1..]);
        out.extend_from_slice(&pts[..j]);
    }
    out.push(p1);
    out
}

/// Whether a ring is convex (either turning).
fn convex_ring(r: &[geom::P2]) -> bool {
    let n = r.len();
    if n < 3 {
        return false;
    }
    let scale = r
        .iter()
        .map(|p| p[0].abs().max(p[1].abs()))
        .fold(0.0f64, f64::max)
        .max(1e-30);
    let tiny = scale * scale * 1e-12;
    let mut sign = 0.0f64;
    for k in 0..n {
        let (a, b, c) = (r[k], r[(k + 1) % n], r[(k + 2) % n]);
        let z = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        if z.abs() < tiny {
            continue;
        }
        if sign == 0.0 {
            sign = z.signum();
        } else if z.signum() != sign {
            return false;
        }
    }
    true
}

/// The planes of a closed convex shape (outward normals), `None` when it is not convex.
fn convex_planes(s: &Shape, eps: f64) -> Option<Vec<Plane>> {
    if !s.closed || s.is_empty() {
        return None;
    }
    let mut planes: Vec<Plane> = Vec::new();
    for t in &s.tri {
        let [a, b, c] = t.map(|i| s.pos[i as usize]);
        let Some(n) = geom::unit(geom::cross(sub(b, a), sub(c, a))) else {
            continue;
        };
        let d = dot(n, a);
        if !planes
            .iter()
            .any(|p| dot(p.n, n) > 1.0 - 1e-9 && (p.d - d).abs() <= eps)
        {
            planes.push(Plane { n, d });
        }
        if planes.len() > 256 {
            return None;
        }
    }
    let ok = planes
        .iter()
        .all(|p| s.pos.iter().all(|&q| dot(p.n, q) - p.d <= eps * 10.0));
    ok.then_some(planes)
}

/// A readable name of an entity type (`IFCWALLSTANDARDCASE` → `IfcWallStandardCase`).
pub(crate) fn display_kind(ty: &str) -> String {
    KINDS
        .iter()
        .find(|(k, _)| *k == ty)
        .map_or_else(|| ty.to_owned(), |(_, v)| (*v).to_owned())
}

/// "1 wall", "3 walls": a count of elements of a kind for the spatial tree.
fn count_of(n: usize, ty: &str) -> String {
    let t = ty.strip_prefix("IFC").unwrap_or(ty);
    let (one, many) = match t {
        "WALL" | "WALLSTANDARDCASE" | "WALLELEMENTEDCASE" => ("wall", "walls"),
        "CURTAINWALL" => ("curtain wall", "curtain walls"),
        "SLAB" | "SLABSTANDARDCASE" | "SLABELEMENTEDCASE" => ("slab", "slabs"),
        "ROOF" => ("roof", "roofs"),
        "WINDOW" | "WINDOWSTANDARDCASE" => ("window", "windows"),
        "DOOR" | "DOORSTANDARDCASE" => ("door", "doors"),
        "COLUMN" | "COLUMNSTANDARDCASE" => ("column", "columns"),
        "BEAM" | "BEAMSTANDARDCASE" => ("beam", "beams"),
        "MEMBER" | "MEMBERSTANDARDCASE" => ("member", "members"),
        "PLATE" | "PLATESTANDARDCASE" => ("plate", "plates"),
        "STAIR" => ("stair", "stairs"),
        "STAIRFLIGHT" => ("stair flight", "stair flights"),
        "RAMP" | "RAMPFLIGHT" => ("ramp", "ramps"),
        "RAILING" => ("railing", "railings"),
        "COVERING" => ("covering", "coverings"),
        "FOOTING" => ("footing", "footings"),
        "PILE" => ("pile", "piles"),
        "FURNISHINGELEMENT" | "FURNITURE" | "SYSTEMFURNITUREELEMENT" => {
            ("piece of furniture", "pieces of furniture")
        }
        "BUILDINGELEMENTPROXY" => ("other element", "other elements"),
        "SPACE" => ("space", "spaces"),
        "OPENINGELEMENT" | "OPENINGSTANDARDCASE" => ("opening", "openings"),
        "FLOWSEGMENT" | "PIPESEGMENT" | "DUCTSEGMENT" | "CABLESEGMENT" | "CABLECARRIERSEGMENT" => {
            ("segment", "segments")
        }
        "FLOWFITTING" | "PIPEFITTING" | "DUCTFITTING" | "CABLEFITTING" | "CABLECARRIERFITTING" => {
            ("fitting", "fittings")
        }
        "FLOWTERMINAL" | "SANITARYTERMINAL" | "AIRTERMINAL" | "LIGHTFIXTURE" | "OUTLET" => {
            ("terminal", "terminals")
        }
        "REINFORCINGBAR" | "REINFORCINGMESH" => ("reinforcing bar", "reinforcing bars"),
        "ELEMENTASSEMBLY" => ("assembly", "assemblies"),
        "GEOGRAPHICELEMENT" => ("geographic element", "geographic elements"),
        _ => return format!("{n} × {}", display_kind(ty)),
    };
    format!("{n} {}", if n == 1 { one } else { many })
}

/// IFC product entity names as written in the schema, for messages and the info panel.
const KINDS: &[(&str, &str)] = &[
    ("IFCPROJECT", "IfcProject"),
    ("IFCSITE", "IfcSite"),
    ("IFCBUILDING", "IfcBuilding"),
    ("IFCBUILDINGSTOREY", "IfcBuildingStorey"),
    ("IFCSPACE", "IfcSpace"),
    ("IFCFACILITY", "IfcFacility"),
    ("IFCFACILITYPART", "IfcFacilityPart"),
    ("IFCBRIDGE", "IfcBridge"),
    ("IFCBRIDGEPART", "IfcBridgePart"),
    ("IFCROAD", "IfcRoad"),
    ("IFCROADPART", "IfcRoadPart"),
    ("IFCRAILWAY", "IfcRailway"),
    ("IFCRAILWAYPART", "IfcRailwayPart"),
    ("IFCMARINEFACILITY", "IfcMarineFacility"),
    ("IFCMARINEPART", "IfcMarinePart"),
    ("IFCTUNNEL", "IfcTunnel"),
    ("IFCTUNNELPART", "IfcTunnelPart"),
    ("IFCWALL", "IfcWall"),
    ("IFCWALLSTANDARDCASE", "IfcWallStandardCase"),
    ("IFCSLAB", "IfcSlab"),
    ("IFCROOF", "IfcRoof"),
    ("IFCWINDOW", "IfcWindow"),
    ("IFCDOOR", "IfcDoor"),
    ("IFCCOLUMN", "IfcColumn"),
    ("IFCBEAM", "IfcBeam"),
    ("IFCMEMBER", "IfcMember"),
    ("IFCPLATE", "IfcPlate"),
    ("IFCSTAIR", "IfcStair"),
    ("IFCSTAIRFLIGHT", "IfcStairFlight"),
    ("IFCRAMP", "IfcRamp"),
    ("IFCRAILING", "IfcRailing"),
    ("IFCCOVERING", "IfcCovering"),
    ("IFCFOOTING", "IfcFooting"),
    ("IFCPILE", "IfcPile"),
    ("IFCCURTAINWALL", "IfcCurtainWall"),
    ("IFCBUILDINGELEMENTPROXY", "IfcBuildingElementProxy"),
    ("IFCFURNISHINGELEMENT", "IfcFurnishingElement"),
    ("IFCFURNITURE", "IfcFurniture"),
    ("IFCOPENINGELEMENT", "IfcOpeningElement"),
    ("IFCFLOWSEGMENT", "IfcFlowSegment"),
    ("IFCFLOWTERMINAL", "IfcFlowTerminal"),
    ("IFCFLOWFITTING", "IfcFlowFitting"),
    ("IFCPIPESEGMENT", "IfcPipeSegment"),
    ("IFCDUCTSEGMENT", "IfcDuctSegment"),
    ("IFCREINFORCINGBAR", "IfcReinforcingBar"),
    ("IFCELEMENTASSEMBLY", "IfcElementAssembly"),
    ("IFCGEOGRAPHICELEMENT", "IfcGeographicElement"),
    ("IFCSECTIONEDSPINE", "IfcSectionedSpine"),
    ("IFCSECTIONEDSOLID", "IfcSectionedSolid"),
    ("IFCSECTIONEDSOLIDHORIZONTAL", "IfcSectionedSolidHorizontal"),
    (
        "IFCDIRECTRIXCURVESWEPTAREASOLID",
        "IfcDirectrixCurveSweptAreaSolid",
    ),
    ("IFCEXTRUDEDAREASOLID", "IfcExtrudedAreaSolid"),
    ("IFCREVOLVEDAREASOLID", "IfcRevolvedAreaSolid"),
    ("IFCHALFSPACESOLID", "IfcHalfSpaceSolid"),
    ("IFCADVANCEDBREP", "IfcAdvancedBrep"),
    ("IFCFACETEDBREP", "IfcFacetedBrep"),
    ("IFCMAPPEDITEM", "IfcMappedItem"),
    ("IFCBOOLEANRESULT", "IfcBooleanResult"),
    ("IFCBOOLEANCLIPPINGRESULT", "IfcBooleanClippingResult"),
    ("IFCSWEPTDISKSOLID", "IfcSweptDiskSolid"),
    ("IFCTRIANGULATEDFACESET", "IfcTriangulatedFaceSet"),
    ("IFCPOLYGONALFACESET", "IfcPolygonalFaceSet"),
];

/// Units of the project: (metres per length unit, radians per plane angle unit, the length
/// unit's name).
fn units(s: &Step) -> (f64, f64, String) {
    let mut length = (1.0, "metre".to_owned());
    let mut angle = 1.0;
    let project = s.of_types(&["IFCPROJECT"]).into_iter().next();
    let assignment = project
        .and_then(|p| s.args(p))
        .and_then(|a| a.get(8).and_then(V::as_ref))
        .or_else(|| s.of_types(&["IFCUNITASSIGNMENT"]).into_iter().next());
    let Some(ua) = assignment.and_then(|u| s.args(u)) else {
        return (length.0, angle, length.1);
    };
    for u in ua.first().map(V::refs).unwrap_or_default() {
        let Some((kind, factor, name)) = unit_factor(s, u, 0) else {
            continue;
        };
        match kind.as_str() {
            "LENGTHUNIT" => length = (factor, name),
            "PLANEANGLEUNIT" => angle = factor,
            _ => {}
        }
    }
    (length.0, angle, length.1)
}

/// (unit type, SI factor, name) of an `IfcSIUnit` or `IfcConversionBasedUnit`.
fn unit_factor(s: &Step, u: u32, depth: usize) -> Option<(String, f64, String)> {
    if depth > 4 {
        return None;
    }
    let a = s.args(u)?;
    match s.ty(u)? {
        "IFCSIUNIT" => {
            // (Dimensions*, UnitType, Prefix, Name)
            let kind = String::from_utf8_lossy(a.get(1)?.as_enum()?).into_owned();
            let prefix = a.get(2).and_then(V::as_enum).unwrap_or(b"");
            let p = match prefix {
                b"EXA" => 1e18,
                b"PETA" => 1e15,
                b"TERA" => 1e12,
                b"GIGA" => 1e9,
                b"MEGA" => 1e6,
                b"KILO" => 1e3,
                b"HECTO" => 1e2,
                b"DECA" => 1e1,
                b"DECI" => 1e-1,
                b"CENTI" => 1e-2,
                b"MILLI" => 1e-3,
                b"MICRO" => 1e-6,
                b"NANO" => 1e-9,
                b"PICO" => 1e-12,
                _ => 1.0,
            };
            let base = String::from_utf8_lossy(a.get(3)?.as_enum()?).to_ascii_lowercase();
            let name = format!(
                "{}{}",
                String::from_utf8_lossy(prefix).to_ascii_lowercase(),
                base
            );
            let f = if base == "degree" {
                p * std::f64::consts::PI / 180.0
            } else {
                p
            };
            Some((kind, f, name))
        }
        "IFCCONVERSIONBASEDUNIT" | "IFCCONVERSIONBASEDUNITWITHOFFSET" => {
            // (Dimensions, UnitType, Name, ConversionFactor)
            let kind = String::from_utf8_lossy(a.get(1)?.as_enum()?).into_owned();
            let name = a
                .get(2)
                .and_then(V::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let mw = s.args(a.get(3)?.as_ref()?)?;
            let value = mw.first()?.as_f64()?;
            let base = mw
                .get(1)
                .and_then(V::as_ref)
                .and_then(|b| unit_factor(s, b, depth + 1))
                .map_or(1.0, |b| b.1);
            Some((kind, value * base, name))
        }
        _ => None,
    }
}

/// The unit for the mesh and the factor taking file units to it.
fn mesh_unit(metres: f64) -> (Unit, f64) {
    for (u, m) in [
        (Unit::Millimetre, 1e-3),
        (Unit::Metre, 1.0),
        (Unit::Centimetre, 1e-2),
        (Unit::Inch, 0.0254),
        (Unit::Foot, 0.3048),
    ] {
        if ((metres - m) / m).abs() < 1e-6 {
            return (u, 1.0);
        }
    }
    (Unit::Metre, metres)
}

/// The text of an `.ifc` file, from an `.ifczip` when it is one.
fn unpack<'a>(bytes: &'a [u8], limits: &Limits) -> Result<std::borrow::Cow<'a, [u8]>> {
    if !bytes.starts_with(b"PK") {
        return Ok(std::borrow::Cow::Borrowed(bytes));
    }
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| Error::Malformed(format!("IFC package: {e}")))?;
    let names: Vec<String> = zip.file_names().flatten().map(String::from).collect();
    let pick = names
        .iter()
        .find(|n| n.to_ascii_lowercase().ends_with(".ifc"))
        .or_else(|| {
            names
                .iter()
                .find(|n| n.to_ascii_lowercase().ends_with(".ifcxml"))
        })
        .cloned()
        .ok_or_else(|| Error::Malformed("the IFC package holds no .ifc file".into()))?;
    let f = zip
        .by_name(&pick)
        .map_err(|e| Error::Malformed(format!("{pick}: {e}")))?;
    let mut out = Vec::new();
    f.take(limits.max_unpacked as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| Error::Malformed(format!("{pick}: {e}")))?;
    if out.len() > limits.max_unpacked {
        return Err(Error::TooLarge("the IFC package unpacks too large".into()));
    }
    Ok(std::borrow::Cow::Owned(out))
}

/// Whether `bytes` look like ifcXML.
fn is_xml(bytes: &[u8]) -> bool {
    let t = bytes.strip_prefix(&[0xef, 0xbb, 0xbf][..]).unwrap_or(bytes);
    t.trim_ascii_start().starts_with(b"<")
}

/// Reads an IFC file (`.ifc` or `.ifczip`); advanced B-reps through `tess` when given.
pub fn read(bytes: &[u8], limits: &Limits, tess: Option<&dyn BrepTessellator>) -> Result<IfcModel> {
    let data = unpack(bytes, limits)?;
    if is_xml(&data) {
        return Err(Error::Unsupported(
            "ifcXML is not supported; export the model as an IFC (STEP) file".into(),
        ));
    }
    let t0 = std::time::Instant::now();
    let s = Step::index(&data)?;
    let entities = s.len();
    let schema = s
        .header_args("FILE_SCHEMA")
        .and_then(|a| a.first().and_then(|l| l.as_list()?.first()?.as_str()))
        .unwrap_or_default();
    if !schema.to_ascii_uppercase().starts_with("IFC") {
        return Err(Error::Unsupported(format!(
            "not an IFC model (its schema is {})",
            if schema.is_empty() {
                "not given"
            } else {
                &schema
            }
        )));
    }
    let index_ms = t0.elapsed().as_secs_f64() * 1e3;
    let header = header_fields(&s);
    let (length, angle, length_name) = units(&s);
    let styles = Styles::find(&s);
    let mut colour_maps = HashMap::new();
    for m in s.of_types(&["IFCINDEXEDCOLOURMAP"]) {
        if let Some(t) = s.args(m).and_then(|a| a.first().and_then(V::as_ref)) {
            colour_maps.insert(t, m);
        }
    }
    let mut grid_axes = HashMap::new();
    for g in s.of_types(&["IFCGRID"]) {
        let Some(a) = s.args(g) else { continue };
        let Some(place) = a.get(5).and_then(V::as_ref) else {
            continue;
        };
        for k in 7..10 {
            for ax in a.get(k).map(V::refs).unwrap_or_default() {
                grid_axes.insert(ax, place);
            }
        }
    }
    let ctx = Ctx {
        s,
        angle,
        length,
        eps: 1e-5 / length,
        segments: 32,
        styles,
        colour_maps,
        grid_axes,
        mapped: Mutex::new(HashMap::new()),
        notes: Mutex::new(BTreeMap::new()),
        defer_breps: tess.is_some(),
        work: Work::new(entities, limits),
    };
    let products = find_products(&ctx);
    let voids = voids(&ctx);
    let t1 = std::time::Instant::now();
    let built: Vec<Built> = products
        .par_iter()
        .filter(|p| !hidden(ctx.s.ty(p.id).unwrap_or("")))
        .map(|p| {
            let t = std::time::Instant::now();
            let b = build(&ctx, p, voids.get(&p.id).map(Vec::as_slice).unwrap_or(&[]));
            if cfg!(test) && t.elapsed().as_millis() > 300 {
                eprintln!(
                    "slow: #{} {} {} ms, {} pieces",
                    p.id,
                    ctx.s.ty(p.id).unwrap_or(""),
                    t.elapsed().as_millis(),
                    b.pieces.len()
                );
            }
            b
        })
        .collect();
    let mut built = built;
    // Advanced B-reps through the tessellator.
    let deferred: Vec<(usize, Deferred)> = built
        .iter()
        .enumerate()
        .flat_map(|(k, b)| b.deferred.iter().cloned().map(move |d| (k, d)))
        .collect();
    let mut brep_note = None;
    if let (Some(t), false) = (tess, deferred.is_empty()) {
        let mut unique: Vec<u32> = deferred.iter().map(|(_, d)| d.item).collect();
        unique.sort_unstable();
        unique.dedup();
        match brep_step::translate(&ctx, &unique) {
            Some((text, skipped)) => {
                if skipped > 0 {
                    ctx.note(format!("{skipped} advanced B-reps of kinds not translated"));
                }
                match t.tessellate(&text) {
                    Ok(tes) => {
                        if tes.faces_unmeshed > 0 {
                            ctx.note(format!(
                                "advanced B-rep faces OpenCASCADE could not mesh: {}",
                                tes.faces_unmeshed
                            ));
                        }
                        let mut by_item: HashMap<u32, Shape> = HashMap::new();
                        for (name, pos, idx) in tes.parts {
                            let Some(item) =
                                name.strip_prefix('B').and_then(|n| n.parse::<u32>().ok())
                            else {
                                continue;
                            };
                            let f = tes.metres_per_unit;
                            let s = by_item.entry(item).or_default();
                            let base = s.pos.len() as u32;
                            s.pos
                                .extend(pos.iter().map(|p| p.map(|v| f64::from(v) * f)));
                            s.tri.extend(
                                idx.chunks_exact(3)
                                    .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
                            );
                        }
                        for (k, d) in &deferred {
                            if let Some(src) = by_item.get(&d.item) {
                                let mut sh = src.clone();
                                sh.closed = solid::is_closed(&sh);
                                let world = built[*k].world;
                                sh.transform(&world.mul(&d.xf));
                                built[*k].pieces.push(Piece {
                                    shape: sh,
                                    look: d.look,
                                });
                            }
                        }
                        brep_note = Some(unique.len());
                    }
                    Err(e) => ctx.note(format!("advanced B-reps not drawn: {e}")),
                }
            }
            None => ctx.note("advanced B-reps not drawn: none could be translated"),
        }
    }
    let geometry_ms = t1.elapsed().as_secs_f64() * 1e3;
    assemble(
        ctx,
        products,
        built,
        limits,
        header,
        schema,
        length_name,
        brep_note,
        index_ms,
        geometry_ms,
    )
}

/// A product with a representation.
struct Product {
    id: u32,
    pds: u32,
    placement: Option<u32>,
}

/// Every product with a shape: instances whose first attribute is a GlobalId and whose
/// seventh is an `IfcProductDefinitionShape` (true across IFC2x3, IFC4 and IFC4x3).
fn find_products(ctx: &Ctx) -> Vec<Product> {
    let Some(pds_ty) = ctx.s.type_number("IFCPRODUCTDEFINITIONSHAPE") else {
        return Vec::new();
    };
    let candidates: Vec<(u32, u32)> = ctx.s.all().collect();
    candidates
        .par_iter()
        .filter_map(|&(id, _)| {
            let raw = ctx.s.raw(id)?;
            // "('" + 22 characters + "'"
            let r = raw.strip_prefix(b"(")?.trim_ascii_start();
            if !(r.len() > 24 && r[0] == b'\'' && r[23] == b'\'') {
                return None;
            }
            let a = ctx.s.args(id)?;
            let pds = a.get(6)?.as_ref()?;
            (ctx.s.ty_num(pds) == Some(pds_ty)).then(|| Product {
                id,
                pds,
                placement: a.get(5).and_then(V::as_ref),
            })
        })
        .collect()
}

/// Element → its openings (`IfcRelVoidsElement`).
fn voids(ctx: &Ctx) -> HashMap<u32, Vec<u32>> {
    let mut out: HashMap<u32, Vec<u32>> = HashMap::new();
    for r in ctx.s.of_types(&["IFCRELVOIDSELEMENT"]) {
        let Some(a) = ctx.s.args(r) else { continue };
        if let (Some(e), Some(o)) = (a.get(4).and_then(V::as_ref), a.get(5).and_then(V::as_ref)) {
            out.entry(e).or_default().push(o);
        }
    }
    out
}

/// Most sides of an opening's outline cut (more are approximated).
const MAX_SIDES: usize = 64;
/// Most convex parts an opening or boolean operand may be cut as.
const MAX_PARTS: usize = 16;

/// Most openings subtracted from one element.
const MAX_OPENINGS: usize = 256;
/// Most pieces an element may be cut into.
const MAX_PIECES: usize = 4096;
/// Mapped items inside an opening, nested.
const MAX_OPENING_DEPTH: usize = 24;

fn build(ctx: &Ctx, p: &Product, openings: &[u32]) -> Built {
    let world = p
        .placement
        .map_or(Mat::IDENTITY, |pl| ctx.object_placement(pl, 0));
    let mut deferred = Vec::new();
    let mut pieces = Vec::new();
    for rep in ctx.body_representations(p.pds) {
        let items = ctx
            .s
            .args(rep)
            .and_then(|a| a.get(3).map(V::refs))
            .unwrap_or_default();
        for it in items {
            pieces.extend(ctx.item(it, 0, &mut deferred, &Mat::IDENTITY));
        }
    }
    for piece in &mut pieces {
        piece.shape.transform(&world);
    }
    let mut subtracted = 0;
    let mut kept = 0;
    if !openings.is_empty() && !pieces.is_empty() {
        let grow = ctx.eps * 10.0;
        let tol = ctx.eps * 1e-3;
        for &o in openings.iter().take(MAX_OPENINGS) {
            let Some(oa) = ctx.s.args(o) else { continue };
            let ow = oa
                .get(5)
                .and_then(V::as_ref)
                .map_or(Mat::IDENTITY, |pl| ctx.object_placement(pl, 0));
            let Some(opds) = oa.get(6).and_then(V::as_ref) else {
                continue;
            };
            let mut regions = Vec::new();
            let mut ok = true;
            for rep in ctx.body_representations(opds) {
                for it in ctx
                    .s
                    .args(rep)
                    .and_then(|a| a.get(3).map(V::refs))
                    .unwrap_or_default()
                {
                    match opening_regions(ctx, it, &ow, grow, 0) {
                        Some(r) => regions.extend(r),
                        None => ok = false,
                    }
                }
            }
            if !ok || regions.is_empty() {
                kept += 1;
                continue;
            }
            let mut out = Vec::with_capacity(pieces.len());
            for piece in pieces.drain(..) {
                if !piece.shape.closed {
                    out.push(piece);
                    continue;
                }
                let look = piece.look;
                let mut cur = vec![piece.shape];
                for r in &regions {
                    cur = subtract_region(cur, r, tol);
                    if cur.len() > MAX_PIECES {
                        break;
                    }
                }
                out.extend(cur.into_iter().map(|shape| Piece { shape, look }));
            }
            pieces = out;
            subtracted += 1;
            if pieces.len() > MAX_PIECES {
                break;
            }
        }
        kept += openings.len().saturating_sub(MAX_OPENINGS);
    }
    Built {
        product: p.id,
        pieces,
        deferred,
        openings: (subtracted, kept),
        world,
    }
}

/// An opening's item as convex regions in world coordinates, grown by `grow`. Mapped items
/// nest at most [`MAX_OPENING_DEPTH`] deep (they may refer back to themselves).
fn opening_regions(
    ctx: &Ctx,
    item: u32,
    world: &Mat,
    grow: f64,
    depth: usize,
) -> Option<Vec<Region>> {
    if depth > MAX_OPENING_DEPTH || !ctx.work.call() {
        return None;
    }
    let a = ctx.s.args(item)?;
    match ctx.s.ty(item)? {
        "IFCEXTRUDEDAREASOLID" => {
            let regions = ctx.profile(a.first()?.as_ref()?, 0)?;
            let m = world.mul(&ctx.placement(a.get(1).unwrap_or(&V::Null)));
            let dir = a
                .get(2)
                .and_then(V::as_ref)
                .and_then(|d| ctx.direction(d))
                .unwrap_or([0.0, 0.0, 1.0]);
            let v = geom::scale(dir, a.get(3).and_then(V::as_f64)?);
            ctx.extrusion_regions(&regions, &m, v, grow)
        }
        "IFCMAPPEDITEM" => {
            let src = a.first()?.as_ref()?;
            let sa = ctx.s.args(src)?;
            let origin = ctx.placement(sa.first().unwrap_or(&V::Null));
            let m = world.mul(&ctx.operator(a.get(1)?.as_ref()?).mul(&origin));
            let rep = sa.get(1)?.as_ref()?;
            let mut out = Vec::new();
            for it in ctx.s.args(rep)?.get(3)?.refs() {
                out.extend(opening_regions(ctx, it, &m, grow, depth + 1)?);
            }
            Some(out)
        }
        _ => {
            let mut d = Vec::new();
            let pieces = ctx.item(item, 0, &mut d, &Mat::IDENTITY);
            let mut out = Vec::new();
            for p in pieces {
                let mut s = p.shape;
                s.transform(world);
                let mut planes = convex_planes(&s, ctx.eps)?;
                for pl in &mut planes {
                    pl.d += grow;
                }
                out.push(Region::new(planes, s.bounds()));
            }
            (!out.is_empty()).then_some(out)
        }
    }
}

/// Header fields for the info panel.
fn header_fields(s: &Step) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let text = |v: &V| -> String {
        match v {
            V::List(l) => l
                .iter()
                .filter_map(V::as_str)
                .filter(|x| !x.trim().is_empty())
                .collect::<Vec<_>>()
                .join(", "),
            other => other.as_str().unwrap_or_default(),
        }
    };
    if let Some(a) = s.header_args("FILE_DESCRIPTION") {
        let d = a.first().map(text).unwrap_or_default();
        // "ViewDefinition [CoordinationView]"
        if let Some(view) = d
            .split(',')
            .find_map(|p| p.trim().strip_prefix("ViewDefinition"))
        {
            out.push((
                "View definition".to_owned(),
                view.trim().trim_matches(['[', ']', ' ']).to_owned(),
            ));
        }
    }
    if let Some(a) = s.header_args("FILE_NAME") {
        let names = [
            "Name",
            "Date",
            "Author",
            "Organization",
            "Preprocessor",
            "Originating system",
            "Authorization",
        ];
        for (k, n) in names.iter().enumerate() {
            let v = a.get(k).map(text).unwrap_or_default();
            if !v.trim().is_empty() {
                out.push(((*n).to_owned(), v));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn assemble(
    ctx: Ctx,
    products: Vec<Product>,
    built: Vec<Built>,
    limits: &Limits,
    header: Vec<(String, String)>,
    schema: String,
    length_name: String,
    breps: Option<usize>,
    index_ms: f64,
    geometry_ms: f64,
) -> Result<IfcModel> {
    let (unit, to_unit) = mesh_unit(ctx.length);
    // The shift: the centre of everything, rounded, when far out.
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for b in &built {
        for p in &b.pieces {
            if let Some((a, c)) = p.shape.bounds() {
                for k in 0..3 {
                    lo[k] = lo[k].min(a[k]);
                    hi[k] = hi[k].max(c[k]);
                }
            }
        }
    }
    let far = 1e4 / ctx.length;
    let mut shift = [0.0; 3];
    if lo[0] <= hi[0] {
        let c = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
        if c.iter().any(|v| v.abs() > far) {
            shift = c.map(|v| (v * ctx.length).round() / ctx.length);
        }
    }
    let mut b = Builder::new(limits);
    b.unit = unit;
    b.up = Up::Z;
    // One material per look; kind colours for the rest.
    let mut mat_of_look: HashMap<u32, u32> = HashMap::new();
    let mut mat_of_kind: HashMap<String, u32> = HashMap::new();
    let mut elements = Vec::with_capacity(built.len());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let (mut subtracted, mut kept) = (0usize, 0usize);
    let tris_total: usize = built
        .iter()
        .flat_map(|b| b.pieces.iter().map(|p| p.shape.tri.len()))
        .sum();
    b.reserve_triangles(tris_total)?;
    // Normals, welding and element figures per product in parallel; the shapes are freed
    // as they are done.
    let prepared: Vec<ElementOut> = built
        .into_par_iter()
        .map(|out| {
            let kind = ctx.s.ty(out.product).unwrap_or("").to_owned();
            let pa = ctx.s.args(out.product).unwrap_or_default();
            let mut el = Element {
                id: out.product,
                global_id: pa.first().and_then(V::as_str).unwrap_or_default(),
                kind,
                name: pa.get(2).and_then(V::as_str).unwrap_or_default(),
                min: [f64::INFINITY; 3],
                max: [f64::NEG_INFINITY; 3],
                ..Element::default()
            };
            let mut volume = 0.0;
            let mut closed = !out.pieces.is_empty();
            let material_look = ctx.material_look(out.product);
            let mut parts = Vec::with_capacity(out.pieces.len());
            for p in out.pieces {
                let s = &p.shape;
                if s.is_empty() {
                    continue;
                }
                el.triangles += s.tri.len();
                el.area += s.area();
                if s.closed {
                    volume += s.volume();
                } else {
                    closed = false;
                }
                if let Some((a, c)) = s.bounds() {
                    for k in 0..3 {
                        el.min[k] = el.min[k].min(a[k]);
                        el.max[k] = el.max[k].max(c[k]);
                    }
                }
                parts.push((p.look.or(material_look), prepare(s, shift, to_unit)));
            }
            if closed && el.triangles > 0 {
                el.volume = Some(volume);
            }
            (el, out.openings, parts)
        })
        .collect();
    for (mut el, openings, parts) in prepared {
        subtracted += openings.0;
        kept += openings.1;
        if el.triangles == 0 {
            continue;
        }
        el.first_vertex = b.vertex_count();
        for (look, part) in parts {
            let kind = &el.kind;
            let mat = match look {
                Some(l) => *mat_of_look.entry(l).or_insert_with(|| {
                    let look: &Look = &ctx.styles.looks[l as usize];
                    b.add_material(material(look.rgba, &look.name))
                }),
                None => *mat_of_kind.entry(kind.clone()).or_insert_with(|| {
                    b.add_material(material(style::kind_colour(kind), &display_kind(kind)))
                }),
            };
            b.material(Some(mat));
            let base = b.vertex_count();
            for (k, (&q, &n)) in part.pos.iter().zip(&part.nrm).enumerate() {
                let c = part.col.as_ref().map(|c| c[k]);
                b.vertex(q, Some(n), c, None)?;
            }
            for t in &part.idx {
                b.triangle(base + t[0], base + t[1], base + t[2])?;
            }
        }
        el.vertices = b.vertex_count() - el.first_vertex;
        *counts.entry(el.kind.clone()).or_default() += 1;
        elements.push(el);
    }
    if ctx.work.spent() {
        ctx.note("shapes referenced too many times: part of the model not drawn");
    }
    let notes = ctx.notes.lock().map(|n| n.clone()).unwrap_or_default();
    for (n, c) in &notes {
        b.note(if *c > 1 {
            format!("{n} ({c})")
        } else {
            n.clone()
        });
    }
    if shift != [0.0; 3] {
        b.note(format!(
            "coordinates shifted by ({:.0}, {:.0}, {:.0}) {} to near the origin",
            shift[0], shift[1], shift[2], length_name
        ));
    }
    // Title: the project's name; generator: the authoring application.
    let project = ctx.s.of_types(&["IFCPROJECT"]).into_iter().next();
    let project_name = project
        .and_then(|p| ctx.s.args(p))
        .and_then(|a| {
            a.get(7)
                .and_then(V::as_str)
                .filter(|n| !n.trim().is_empty())
                .or_else(|| a.get(2).and_then(V::as_str))
        })
        .filter(|n| !n.trim().is_empty());
    b.title = project_name.clone();
    let app = application(&ctx);
    b.generator = app.clone().or_else(|| {
        header
            .iter()
            .find(|(k, _)| k == "Originating system")
            .map(|(_, v)| v.clone())
    });
    if b.triangle_count() == 0 {
        let why = if products.is_empty() {
            "this IFC model has no elements with geometry".to_owned()
        } else {
            let n: Vec<String> = notes.keys().take(3).cloned().collect();
            if n.is_empty() {
                "this IFC model has no geometry that can be shown".to_owned()
            } else {
                format!(
                    "this IFC model has no geometry that can be shown ({})",
                    n.join("; ")
                )
            }
        };
        return Err(Error::Unsupported(why));
    }
    let mesh = b.finish()?;
    // Facts.
    let mut facts: Vec<(String, String)> = Vec::new();
    facts.push(("Schema".to_owned(), schema.clone()));
    if let Some(a) = &app {
        facts.push(("Application".to_owned(), a.clone()));
    }
    if let Some(n) = &project_name {
        facts.push(("Project".to_owned(), n.clone()));
    }
    facts.push(("Length unit".to_owned(), length_name));
    facts.push(("Instances".to_owned(), ctx.s.len().to_string()));
    facts.push(("Elements drawn".to_owned(), elements.len().to_string()));
    let mut by_count: Vec<(&String, &usize)> = counts.iter().collect();
    by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    if !by_count.is_empty() {
        let list: Vec<String> = by_count
            .iter()
            .take(12)
            .map(|(k, n)| count_of(**n, k))
            .collect();
        let more = by_count.len().saturating_sub(12);
        let mut v = list.join(", ");
        if more > 0 {
            v.push_str(&format!(", {more} more kinds"));
        }
        facts.push(("Drawn".to_owned(), v));
    }
    let spaces = ctx.s.of_types(&["IFCSPACE"]).len();
    if spaces > 0 {
        facts.push(("Spaces (not drawn)".to_owned(), spaces.to_string()));
    }
    if subtracted + kept > 0 {
        facts.push(("Openings cut".to_owned(), subtracted.to_string()));
        if kept > 0 {
            facts.push(("Openings not cut".to_owned(), kept.to_string()));
        }
    }
    if let Some(n) = breps {
        facts.push((
            "Advanced B-reps".to_owned(),
            format!("{n}, through OpenCASCADE (sandboxed)"),
        ));
    }
    facts.push(("Read by".to_owned(), "Specular's IFC reader".to_owned()));
    facts.push((
        "Read in".to_owned(),
        format!("{index_ms:.0} ms index, {geometry_ms:.0} ms geometry"),
    ));
    let tree = spatial_tree(&ctx);
    let georef = georef::find(&ctx);
    facts.extend(georef::facts(&georef));
    Ok(IfcModel {
        mesh,
        schema,
        header,
        facts,
        tree,
        elements,
        metres_per_unit: ctx.length,
        mesh_metres: ctx.length / to_unit,
        origin_metres: shift.map(|v| v * ctx.length),
        georef,
    })
}

/// A corner by position, normal and colour (bits), for welding.
type CornerKey = ([u64; 3], [u32; 3], [u8; 4]);

/// An element's figures, its openings (cut, not cut) and its pieces with their looks.
type ElementOut = (Element, (usize, usize), Vec<(Option<u32>, Prepared)>);

/// A piece ready for the mesh: welded corners (shifted, in the mesh's unit) with normals
/// and colours, and its triangles.
struct Prepared {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Option<Vec<[u8; 4]>>,
    idx: Vec<[u32; 3]>,
}

/// Normals smoothed within 35°, and corners that agree in position, normal and colour
/// made one.
fn prepare(s: &Shape, shift: P3, to_unit: f64) -> Prepared {
    let normals = crease_normals(s, 35f64.to_radians());
    let mut ids: HashMap<CornerKey, u32> = HashMap::with_capacity(s.pos.len());
    let mut out = Prepared {
        pos: Vec::with_capacity(s.pos.len()),
        nrm: Vec::with_capacity(s.pos.len()),
        col: s.colors.as_ref().map(|_| Vec::new()),
        idx: Vec::with_capacity(s.tri.len()),
    };
    for (t, tri) in s.tri.iter().enumerate() {
        let colour = s.colors.as_ref().and_then(|c| c.get(t).copied());
        let mut v = [0u32; 3];
        for c in 0..3 {
            let p = s.pos[tri[c] as usize];
            let n = normals[t][c];
            let key = (
                p.map(f64::to_bits),
                n.map(f32::to_bits),
                colour.unwrap_or([255; 4]),
            );
            v[c] = *ids.entry(key).or_insert_with(|| {
                out.pos
                    .push([0, 1, 2].map(|k| ((p[k] - shift[k]) * to_unit) as f32));
                out.nrm.push(n);
                if let Some(cs) = &mut out.col {
                    cs.push(colour.unwrap_or([255; 4]));
                }
                (out.pos.len() - 1) as u32
            });
        }
        out.idx.push(v);
    }
    out
}

fn material(srgb: [f32; 4], name: &str) -> Material {
    Material {
        name: name.to_owned(),
        base_color: [
            crate::builder::srgb_to_linear(srgb[0]),
            crate::builder::srgb_to_linear(srgb[1]),
            crate::builder::srgb_to_linear(srgb[2]),
            srgb[3],
        ],
        texture: None,
    }
}

/// "Name Version" of the first `IfcApplication`.
fn application(ctx: &Ctx) -> Option<String> {
    let a = ctx
        .s
        .args(ctx.s.of_types(&["IFCAPPLICATION"]).into_iter().next()?)?;
    // (ApplicationDeveloper, Version, ApplicationFullName, ApplicationIdentifier)
    let name = a
        .get(2)
        .and_then(V::as_str)
        .filter(|n| !n.trim().is_empty())?;
    let ver = a.get(1).and_then(V::as_str).unwrap_or_default();
    Some(if ver.trim().is_empty() || name.contains(ver.trim()) {
        name
    } else {
        format!("{name} {ver}")
    })
}

/// Most rows of the spatial tree.
const MAX_TREE_ROWS: usize = 400;

/// Project → sites → buildings → storeys (and facilities, their parts), each with what it
/// contains counted by kind.
fn spatial_tree(ctx: &Ctx) -> Vec<TreeRow> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for r in ctx.s.of_types(&["IFCRELAGGREGATES"]) {
        let Some(a) = ctx.s.args(r) else { continue };
        if let Some(p) = a.get(4).and_then(V::as_ref) {
            children
                .entry(p)
                .or_default()
                .extend(a.get(5).map(V::refs).unwrap_or_default());
        }
    }
    let mut contained: HashMap<u32, Vec<u32>> = HashMap::new();
    for r in ctx.s.of_types(&["IFCRELCONTAINEDINSPATIALSTRUCTURE"]) {
        let Some(a) = ctx.s.args(r) else { continue };
        if let Some(st) = a.get(5).and_then(V::as_ref) {
            contained
                .entry(st)
                .or_default()
                .extend(a.get(4).map(V::refs).unwrap_or_default());
        }
    }
    let mut rows = Vec::new();
    let roots = ctx.s.of_types(&["IFCPROJECT"]);
    let mut seen = std::collections::HashSet::new();
    fn walk(
        ctx: &Ctx,
        id: u32,
        depth: usize,
        children: &HashMap<u32, Vec<u32>>,
        contained: &HashMap<u32, Vec<u32>>,
        rows: &mut Vec<TreeRow>,
        seen: &mut std::collections::HashSet<u32>,
    ) {
        if depth > 16 || rows.len() >= MAX_TREE_ROWS || !seen.insert(id) {
            return;
        }
        let ty = ctx.s.ty(id).unwrap_or("");
        let a = ctx.s.args(id).unwrap_or_default();
        let name = a.get(2).and_then(V::as_str).unwrap_or_default();
        let long = a.get(7).and_then(V::as_str).unwrap_or_default();
        let kind = match ty {
            "IFCPROJECT" => "project",
            "IFCSITE" => "site",
            "IFCBUILDING" => "building",
            "IFCBUILDINGSTOREY" => "storey",
            "IFCSPACE" => "space",
            "IFCBRIDGE" | "IFCROAD" | "IFCRAILWAY" | "IFCMARINEFACILITY" | "IFCTUNNEL"
            | "IFCFACILITY" => "facility",
            "IFCBRIDGEPART" | "IFCROADPART" | "IFCRAILWAYPART" | "IFCMARINEPART"
            | "IFCTUNNELPART" | "IFCFACILITYPART" => "part",
            _ => "",
        };
        let mut label = if name.trim().is_empty() {
            "(unnamed)".to_owned()
        } else {
            name.trim().to_owned()
        };
        if !long.trim().is_empty() && long.trim() != label {
            label = format!("{label} – {}", long.trim());
        }
        // What it holds, counted by kind (spaces too).
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        let mut spatial_kids = Vec::new();
        for &c in children.get(&id).into_iter().flatten() {
            let cty = ctx.s.ty(c).unwrap_or("");
            if matches!(
                cty,
                "IFCSITE"
                    | "IFCBUILDING"
                    | "IFCBUILDINGSTOREY"
                    | "IFCBRIDGE"
                    | "IFCROAD"
                    | "IFCRAILWAY"
                    | "IFCMARINEFACILITY"
                    | "IFCTUNNEL"
                    | "IFCFACILITY"
                    | "IFCBRIDGEPART"
                    | "IFCROADPART"
                    | "IFCRAILWAYPART"
                    | "IFCMARINEPART"
                    | "IFCTUNNELPART"
                    | "IFCFACILITYPART"
            ) {
                spatial_kids.push(c);
            } else {
                *counts.entry(cty).or_default() += 1;
            }
        }
        for &c in contained.get(&id).into_iter().flatten() {
            *counts.entry(ctx.s.ty(c).unwrap_or("")).or_default() += 1;
        }
        let mut c: Vec<(&str, usize)> = counts.into_iter().collect();
        c.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let summary = c
            .iter()
            .take(6)
            .map(|(k, n)| count_of(*n, k))
            .collect::<Vec<_>>()
            .join(", ");
        let more = c.len().saturating_sub(6);
        let mut text = if kind.is_empty() {
            format!("{label} ({})", display_kind(ty))
        } else {
            format!("{label} ({kind})")
        };
        if !summary.is_empty() {
            text.push_str(": ");
            text.push_str(&summary);
            if more > 0 {
                text.push_str(&format!(", {more} more kinds"));
            }
        }
        rows.push((depth, text));
        // Storeys in elevation order.
        if spatial_kids
            .iter()
            .all(|&k| ctx.s.ty(k) == Some("IFCBUILDINGSTOREY"))
        {
            spatial_kids.sort_by(|&x, &y| {
                let e = |k: u32| {
                    ctx.s
                        .args(k)
                        .and_then(|a| a.get(9).and_then(V::as_f64))
                        .unwrap_or(0.0)
                };
                e(x).total_cmp(&e(y))
            });
        }
        for k in spatial_kids {
            walk(ctx, k, depth + 1, children, contained, rows, seen);
        }
    }
    for r in roots {
        walk(ctx, r, 0, &children, &contained, &mut rows, &mut seen);
    }
    rows
}
