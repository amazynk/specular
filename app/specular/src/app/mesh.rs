//! 3D models (STL, OBJ, PLY, glTF, 3MF) in the viewer: the model the decoder read
//! ([`specular_decode::Session::mesh`]) is put on the GPU once and drawn by a
//! [`MeshRenderer`] over the image area in place of the preview raster, under an orbit
//! camera that works like MPR's 3D view: drag to turn it, Shift-drag or the middle button
//! to move it, the wheel to zoom.
//!
//! The keymap's mesh mode ([`Mode::Mesh`], `[mesh]` in `keymap.toml`, over the viewer's keys)
//! is on while one is shown: by default Shift+arrows turn the model by 15°, `1`–`4` look from
//! the front, top, side or the isometric view, `s` steps the shading (shaded, with edges,
//! wireframe, x-ray), `o` toggles perspective and orthographic, Alt+g the ground grid. The
//! viewer's zoom keys and fit (`+`, `-`, `f`) and its pan keys (↑ ↓, and h j k l with the
//! Vim preset) work on the model. Every mesh action is in [`Group::Mesh`].
//!
//! Shading, projection and grid stay across files; the camera is per file and starts at the
//! isometric view, fitted. A readout in the bottom corner gives the model's size in its
//! unit, its triangles and vertices, the grid's spacing and the look.
//!
//! Point clouds (LAS, LAZ and COPC, PCD, XYZ, PTS, E57, PLY without faces;
//! [`specular_decode::Session::points`]) open in the same view, under the same camera, keys
//! and grid, drawn by a [`PointRenderer`] as round splats with eye-dome lighting. Their
//! points are uploaded as they load, at most [`UPLOAD_PER_FRAME`] bytes a frame, so the
//! cloud fills in while the file is read, and each frame draws at most [`POINT_BUDGET`]
//! points, chosen by level of detail. `]` and `[` make the points larger and smaller, `c`
//! steps the colouring (true colour, intensity, elevation, class: those the cloud has) and
//! `e` turns eye-dome lighting on and off. Point size and eye-dome lighting stay across
//! files; the colouring starts at the cloud's own colours, else elevation. The readout
//! gives the box, the points (read of total), the CRS and the look.
//!
//! Gaussian splats (`.splat`, 3DGS PLY, `.spz`; [`specular_decode::Session::splats`]) open
//! in the same view, under the same camera, keys and grid, drawn by a [`SplatRenderer`]:
//! sorted back to front whenever the camera moves and blended as anisotropic Gaussians with
//! their view-dependent colour. The readout gives the box, the splats and the look.
//!
//! Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) open in the same view too: see
//! [`molecule`]. So do tractograms (TrackVis, MRtrix): see [`tracts`]. The surfaces of a
//! dense connectivity file (CIFTI `.dconn.nii`) are a model whose colours a click changes:
//! see [`dense`].
//!
//! [`Mode::Mesh`]: specular_input::Mode::Mesh
//! [`Group::Mesh`]: specular_input::Group::Mesh
//! [`UPLOAD_PER_FRAME`]: specular_render_3d::UPLOAD_PER_FRAME

pub(super) mod bim;
mod checks;
mod dense;
mod federation;
pub(super) use federation::{ORIGIN_ITEM as FED_ORIGIN_ITEM, REMOVE_ITEM as FED_REMOVE_ITEM};
mod molecule;
pub(super) mod pmi;
#[cfg(test)]
mod shots;
mod tiles3d;
mod tracts;

use std::time::Instant;

use specular_decode::specular_mesh::{Bounds, Mesh};
use specular_decode::specular_points::Loading;
use specular_decode::{MeshModel, PointsModel, SplatModel};
use specular_gfx::{Gpu, Rect, wgpu};
use specular_i18n::{I18n, t};
use specular_input::Action;
use specular_render::PxRect;
use specular_render_3d::{
    Camera, Grid, MeshGpu, MeshRenderer, ORBIT_KEY, ORBIT_PER_PX, PointColour, PointRenderer,
    PointView, PointsGpu, Projection, Shading, SplatRenderer, SplatView, SplatsGpu, StandardView,
    View, ZOOM_STEP, canonical_bounds,
};
use specular_widgets::accesskit;

use self::dense::{Dense, Picked};
use self::molecule::MolView;
use self::tracts::TractView;
use super::App;
use crate::loader::ImageId;

/// AccessKit id of the readout (anonymisation uses 0x5A00…, measuring 0x5C00…).
const A11Y_ID: u64 = 0x5E00_0000_0000_0000;

/// A press and release this close (logical px) is a click, not a drag.
const CLICK_SLOP: f64 = 4.0;

/// A pan key moves the model by this share of the view's shorter side.
const PAN_KEY: f64 = 0.1;

/// Points drawn a frame at most (level of detail picks which).
pub(super) const POINT_BUDGET: u64 = 10_000_000;

/// The point sizes `]` and `[` step through (diameter, logical px).
const POINT_SIZES: [f32; 9] = [1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// Turning, from the last logical point.
    Orbit((f64, f64)),
    /// Moving, from the last logical point.
    Pan((f64, f64)),
}

/// The model of the current file and how it is looked at.
pub(super) struct MeshView {
    /// The file the state below belongs to.
    id: Option<ImageId>,
    model: Option<MeshModel>,
    /// Its box in the canonical frame (z up).
    bounds: Bounds,
    camera: Camera,
    /// The camera has been fitted to the view.
    fitted: bool,
    /// On the GPU (made on the first frame that draws it).
    gpu: Option<MeshGpu>,
    renderer: Option<MeshRenderer>,
    /// Kept across files.
    pub(super) shading: Shading,
    pub(super) projection: Projection,
    pub(super) grid: bool,
    /// The image area (physical px) as last laid out.
    rect: Option<PxRect>,
    drag: Option<Drag>,
    /// Bumped whenever the view changes (the scene redraws).
    rev: u64,
    /// The point cloud of the current file, in place of a model.
    cloud: Option<PointsModel>,
    /// Its points on the GPU so far (made on the first frame that draws it).
    points: Option<PointsGpu>,
    point_renderer: Option<PointRenderer>,
    /// The cloud's colouring.
    pub(super) colour: PointColour,
    /// Kept across files: the points' diameter (logical px) and eye-dome lighting.
    pub(super) point_size: f32,
    pub(super) edl: bool,
    /// The Gaussian splats of the current file, in place of a model.
    splats: Option<SplatModel>,
    /// On the GPU (made on the first frame that draws them).
    splat_gpu: Option<SplatsGpu>,
    splat_renderer: Option<SplatRenderer>,
    /// The molecular structure of the current file, in place of a model.
    mol: Option<MolView>,
    /// The tractogram of the current file, in place of a model.
    tracts: Option<TractView>,
    /// Where the primary button went down (physical px), until it moves too far to
    /// be a click.
    click: Option<(f32, f32)>,
    /// The dense connectivity file the model's surfaces show a seed's row of.
    dense: Option<Dense>,
    /// The model is the map view's terrain (an elevation model in 3-D), not the file's:
    /// Back returns to the map.
    terrain: bool,
    /// Walking and flying, sections, saved views and BCF topics.
    pub(super) bim: bim::Bim,
    /// A 3D Tiles tileset: its tiles chosen and read as the camera moves.
    tiles: Option<tiles3d::TilesView>,
    /// A CAD model's PMI and how it is shown.
    pub(super) pmi: pmi::PmiState,
}

impl Default for MeshView {
    fn default() -> Self {
        MeshView {
            id: None,
            model: None,
            bounds: Bounds::EMPTY,
            camera: Camera::default(),
            fitted: false,
            gpu: None,
            renderer: None,
            shading: Shading::Shaded,
            projection: Projection::Perspective,
            grid: true,
            rect: None,
            drag: None,
            rev: 0,
            cloud: None,
            points: None,
            point_renderer: None,
            colour: PointColour::Rgb,
            point_size: 2.0,
            edl: true,
            splats: None,
            splat_gpu: None,
            splat_renderer: None,
            mol: None,
            tracts: None,
            click: None,
            dense: None,
            terrain: false,
            bim: bim::Bim::default(),
            tiles: None,
            pmi: pmi::PmiState::default(),
        }
    }
}

impl MeshView {
    /// The GPU device was replaced: renderers and uploads are made again when next drawn
    /// (the models stay in memory).
    pub(super) fn gpu_reset(&mut self) {
        self.gpu = None;
        self.renderer = None;
        self.points = None;
        self.point_renderer = None;
        self.splat_gpu = None;
        self.splat_renderer = None;
        self.bim.caps_gpu = None;
        if let Some(m) = self.mol.as_mut() {
            m.gpu_reset();
        }
        if let Some(t) = self.tracts.as_mut() {
            t.gpu_reset();
        }
        self.changed();
    }

    fn mesh(&self) -> Option<&Mesh> {
        self.model.as_ref().map(|m| &*m.mesh)
    }

    /// A model or a cloud is there to show.
    fn has_scene(&self) -> bool {
        self.model.is_some()
            || self.cloud.is_some()
            || self.splats.is_some()
            || self.mol.is_some()
            || self.tracts.is_some()
    }

    /// Takes `cloud` as the file's scene: fitted on its header's box (or, without one,
    /// the box of the points once some are read).
    fn set_cloud(&mut self, cloud: PointsModel) {
        let l = &cloud.loading;
        self.bounds = match l.result() {
            Some(Ok(c)) => c.bounds,
            _ => l.bounds,
        };
        self.colour = PointColour::default_for(&l.info.channels);
        self.camera.projection = self.projection;
        self.cloud = Some(cloud);
        self.changed();
    }

    fn point_look(&self) -> PointLook {
        PointLook {
            colour: self.colour,
            size: self.point_size,
            edl: self.edl,
            projection: self.projection,
            grid: self.grid,
        }
    }

    /// The next point size up (`up`) or down.
    fn step_size(&mut self, up: bool) {
        let s = self.point_size;
        self.point_size = if up {
            POINT_SIZES.into_iter().find(|&v| v > s + 1e-3)
        } else {
            POINT_SIZES.into_iter().rev().find(|&v| v < s - 1e-3)
        }
        .unwrap_or(s);
    }

    /// The view's size in physical px.
    fn size(&self) -> (f64, f64) {
        self.rect.map_or((1.0, 1.0), |r| {
            (f64::from(r.w.max(1.0)), f64::from(r.h.max(1.0)))
        })
    }

    fn fit(&mut self) {
        let (w, h) = self.size();
        self.camera.fit(&self.bounds, w, h);
        self.changed();
    }

    fn look(&mut self, v: StandardView) {
        let (w, h) = self.size();
        self.camera.look(v, &self.bounds, w, h);
        self.changed();
    }

    fn changed(&mut self) {
        self.rev += 1;
    }

    /// Model units per physical px.
    fn units_per_px(&self) -> f64 {
        let (w, h) = self.size();
        self.camera.units_per_px(&self.bounds, w, h)
    }
}

/// The readout lines for `mesh`.
fn readout(
    mesh: &Mesh,
    bounds: &Bounds,
    shading: Shading,
    projection: Projection,
    grid: bool,
) -> Vec<String> {
    let loc = I18n::global().localizer();
    let unit = mesh.unit.symbol().unwrap_or("");
    let size = mesh.bounds.size().map(f64::from);
    let mut lines = vec![
        size_line(size, unit),
        t!(
            "mesh-counts",
            triangles = loc.integer(mesh.triangle_count() as i64),
            vertices = loc.integer(mesh.vertex_count() as i64)
        ),
    ];
    lines.extend(grid_line(bounds, grid, unit));
    lines.push(specular_i18n::dot_join(&[
        shading_name(shading),
        projection_name(projection),
    ]));
    lines
}

/// "12.5 × 8 × 3 mm", to about four significant digits of the largest side.
fn size_line(size: [f64; 3], unit: &str) -> String {
    let loc = I18n::global().localizer();
    let big = size.iter().fold(0f64, |a, b| a.max(b.abs()));
    let digits = if big > 0.0 && big.is_finite() {
        (3.0 - big.log10().floor()).clamp(0.0, 4.0) as u8
    } else {
        0
    };
    t!(
        "mesh-size",
        x = loc.number(size[0], digits),
        y = loc.number(size[1], digits),
        z = loc.number(size[2], digits),
        unit = specular_i18n::unit_symbol(unit)
    )
    .trim_end()
    .to_owned()
}

/// The ground grid's spacing, when it is shown.
fn grid_line(bounds: &Bounds, grid: bool, unit: &str) -> Option<String> {
    if !grid || bounds.is_empty() {
        return None;
    }
    let loc = I18n::global().localizer();
    let step = Grid::of(bounds).step;
    let d = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as u8
    };
    // One isolate, so "0.5 mm" keeps its order inside RTL text.
    let length = specular_i18n::quantity(&loc.number(step, d.min(9)), unit);
    Some(t!("mesh-grid-step", length = length))
}

/// How a point cloud is drawn, for its readout.
#[derive(Clone, Copy, Debug)]
pub(super) struct PointLook {
    pub colour: PointColour,
    pub size: f32,
    pub edl: bool,
    pub projection: Projection,
    pub grid: bool,
}

/// The readout lines for a cloud: its box (in the file's units), the points read of the
/// total, the CRS and the look.
fn points_readout(loading: &Loading, bounds: &Bounds, look: PointLook) -> Vec<String> {
    let done = loading.result().and_then(Result::ok);
    let info = done.as_ref().map_or(&loading.info, |c| &c.info);
    let size = info
        .extent
        .map(|e| [e[3] - e[0], e[4] - e[1], e[5] - e[2]])
        .unwrap_or_else(|| bounds.size().map(f64::from));
    let mut lines = Vec::new();
    if !bounds.is_empty() {
        lines.push(size_line(size, ""));
    }
    let loaded = done.as_ref().map_or(loading.loaded(), |c| c.len() as u64);
    let total = info.total.max(loaded);
    lines.push(if loaded < total {
        t!(
            "points-loading",
            loaded = loaded as i64,
            total = total as i64
        )
    } else {
        t!("points-count", count = total as i64)
    });
    if let Some(crs) = info.crs.as_deref().filter(|c| !c.trim().is_empty()) {
        lines.push(specular_i18n::isolate_ltr(crs.trim()));
    }
    lines.extend(grid_line(bounds, look.grid, ""));
    let loc = I18n::global().localizer();
    let mut l = vec![
        colour_name(look.colour),
        t!(
            "points-size",
            size = loc.number(
                f64::from(look.size),
                if look.size.fract() == 0.0 { 0 } else { 1 }
            )
        ),
    ];
    if look.edl {
        l.push(t!("points-edl"));
    }
    l.push(projection_name(look.projection));
    lines.push(specular_i18n::dot_join(&l));
    lines
}

pub(super) fn colour_name(c: PointColour) -> String {
    match c {
        PointColour::Rgb => t!("points-colour-rgb"),
        PointColour::Intensity => t!("points-colour-intensity"),
        PointColour::Elevation => t!("points-colour-elevation"),
        PointColour::Classification => t!("points-colour-classification"),
    }
}

pub(super) fn shading_name(s: Shading) -> String {
    match s {
        Shading::Shaded => t!("mesh-shading-shaded"),
        Shading::Edges => t!("mesh-shading-edges"),
        Shading::Wireframe => t!("mesh-shading-wireframe"),
        Shading::XRay => t!("mesh-shading-xray"),
    }
}

pub(super) fn projection_name(p: Projection) -> String {
    match p {
        Projection::Perspective => t!("mesh-projection-perspective"),
        Projection::Orthographic => t!("mesh-projection-ortho"),
    }
}

/// The readout's node for assistive technology.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, lines: &[String], r: Rect, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let mut n = Node::new(Role::Status);
    n.set_label(format!("{}: {}", t!("keys-mode-mesh"), lines.join(", ")));
    n.set_bounds(accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    });
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), n));
}

impl App {
    /// Follows the current file: its model when it is one, and mesh mode's keys.
    pub(super) fn mesh_sync(&mut self) {
        let cur = self.current_id();
        if cur != self.mesh.id {
            let mut m = std::mem::take(&mut self.mesh);
            let bim = m.bim.carry();
            let pmi = m.pmi.carry();
            self.mesh = MeshView {
                id: cur,
                shading: m.shading,
                projection: m.projection,
                grid: m.grid,
                rect: m.rect,
                renderer: m.renderer,
                point_renderer: m.point_renderer,
                splat_renderer: m.splat_renderer,
                point_size: m.point_size,
                edl: m.edl,
                rev: m.rev + 1,
                bim,
                pmi,
                ..Default::default()
            };
            self.redraw = true;
        }
        if !self.mesh.has_scene()
            && let Some(id) = cur
            && let Some(s) = self.images.get(&id).and_then(|i| i.session.as_ref())
        {
            let tiles = s.tiles3d().cloned();
            if let Some(model) = s.mesh().cloned() {
                self.mesh.bounds = canonical_bounds(&model.mesh);
                // A tileset: fitted on its root box, so tiles coming and going never move
                // the camera.
                if let Some(t) = &tiles {
                    self.mesh.bounds = t.bounds();
                }
                self.mesh.tiles = tiles.clone().map(tiles3d::TilesView::new);
                self.mesh.camera.projection = self.mesh.projection;
                self.mesh.model = Some(model);
                // A tileset of models and points shows both.
                if let Some(c) = s.points().filter(|_| tiles.is_some()) {
                    self.mesh.colour = PointColour::default_for(&c.loading.info.channels);
                    self.mesh.cloud = Some(c.clone());
                }
                self.mesh.pmi.data = s.pmi().cloned();
                self.mesh.dense = s
                    .neuro()
                    .and_then(|n| n.connectivity.as_ref())
                    .map(|c| Dense::new(c.0.clone(), 0));
                self.mesh.changed();
                self.redraw = true;
            } else if let Some(cloud) = s.points().cloned() {
                self.mesh.set_cloud(cloud);
                if let Some(t) = &tiles {
                    self.mesh.bounds = t.bounds();
                    self.mesh.tiles = Some(tiles3d::TilesView::new(t.clone()));
                }
                self.redraw = true;
            } else if let Some(sp) = s.splats().cloned() {
                self.mesh.bounds = sp.splats.bounds;
                self.mesh.camera.projection = self.mesh.projection;
                self.mesh.splats = Some(sp);
                self.mesh.changed();
                self.redraw = true;
            } else if let Some(m) = s.molecule().cloned() {
                let mol = MolView::new(m);
                self.mesh.bounds = mol.scene.bounds;
                self.mesh.camera.projection = self.mesh.projection;
                self.mesh.mol = Some(mol);
                self.mesh.changed();
                self.redraw = true;
            } else if let Some(t) = s.tracts().cloned() {
                let tv = TractView::new(t);
                self.mesh.bounds = tv.bounds;
                self.mesh.camera.projection = self.mesh.projection;
                self.mesh.tracts = Some(tv);
                self.mesh.changed();
                self.redraw = true;
            }
        }
        // A cloud without a box in its header is fitted once points arrive.
        if self.mesh.cloud.is_some()
            && self.mesh.bounds.is_empty()
            && let Some(b) = self.mesh.points.as_ref().map(|g| g.bounds)
            && !b.is_empty()
        {
            self.mesh.bounds = b;
            self.mesh.fitted = false;
            self.mesh.changed();
            self.redraw = true;
        }
        let on = self.mesh.has_scene();
        if self.keys.mesh != on {
            self.keys.mesh = on;
            self.keys_sync_modes();
        }
        if on {
            self.bim_tick();
            self.tiles3d_tick();
            self.pmi_sync();
        }
    }

    /// A navigation device moved the 3D view (`app/devices.rs`): `pan` in px, `zoom` a factor,
    /// `orbit` in radians. False when no model is shown.
    pub(super) fn mesh_device_nav(
        &mut self,
        pan: (f64, f64),
        zoom: f64,
        orbit: (f64, f64),
    ) -> bool {
        if !self.mesh_active() {
            return false;
        }
        let m = &mut self.mesh;
        if orbit != (0.0, 0.0) {
            m.camera.orbit(orbit.0, orbit.1);
        }
        if pan != (0.0, 0.0) {
            let upp = m.units_per_px();
            m.camera.pan_px(pan.0, pan.1, upp);
        }
        if zoom != 1.0 {
            m.camera.zoom_by(zoom);
        }
        m.changed();
        self.redraw = true;
        true
    }

    /// A model is shown in the 3D view.
    pub(super) fn mesh_active(&self) -> bool {
        self.mesh.has_scene()
            && self.mesh.id.is_some()
            && self.mesh.id == self.current_id()
            && !self.gallery.active
            && !self.mpr_active()
    }

    /// The scene revision for the redraw key (0 when no model is shown).
    pub(super) fn mesh_rev(&self) -> u64 {
        if self.mesh_active() {
            self.mesh.rev.max(1)
        } else {
            0
        }
    }

    /// What the mesh actions depend on ([`specular_input::ActionState::mesh`]).
    pub(super) fn mesh_state(&self) -> bool {
        self.mesh_active()
    }

    /// What the point cloud actions depend on ([`specular_input::ActionState::points`]).
    pub(super) fn points_state(&self) -> bool {
        self.mesh_active() && self.mesh.cloud.is_some()
    }

    /// What the streamline thickness and colouring depend on
    /// ([`specular_input::ActionState::tracts`]).
    pub(super) fn tracts_state(&self) -> bool {
        self.mesh_active() && self.mesh.tracts.is_some()
    }

    /// What the molecule actions depend on ([`specular_input::ActionState::molecule`]).
    pub(super) fn molecule_state(&self) -> bool {
        self.mesh_active() && self.mesh.mol.is_some()
    }

    /// Lays the view out over the image area (physical px); fits the model the first time.
    pub(super) fn mesh_layout(&mut self, vp: PxRect) {
        if self.mesh.rect != Some(vp) {
            self.mesh.rect = Some(vp);
            self.mesh.changed();
        }
        if self.mesh.has_scene() && !self.mesh.bounds.is_empty() && !self.mesh.fitted {
            self.mesh.fitted = true;
            self.mesh.fit();
        }
    }

    /// Runs a mesh action, or the viewer's zoom and pan on the model; true when taken.
    /// Shows `model` (the map view's terrain) in the 3-D view for the current image, until
    /// Back.
    pub(super) fn mesh_show_terrain(&mut self, model: MeshModel) {
        self.mesh_sync();
        self.mesh.bounds = canonical_bounds(&model.mesh);
        self.mesh.camera.projection = self.mesh.projection;
        self.mesh.model = Some(model);
        self.mesh.gpu = None;
        self.mesh.terrain = true;
        self.mesh.fitted = false;
        self.mesh.changed();
        let on = self.mesh.has_scene();
        if self.keys.mesh != on {
            self.keys.mesh = on;
            self.keys_sync_modes();
        }
        self.redraw = true;
    }

    /// Leaves the map's terrain in 3-D: the map comes back.
    fn mesh_close_terrain(&mut self) {
        self.mesh.model = None;
        self.mesh.gpu = None;
        self.mesh.terrain = false;
        self.mesh.changed();
        if self.keys.mesh {
            self.keys.mesh = false;
            self.keys_sync_modes();
        }
        self.gis_terrain_closed();
        self.redraw = true;
    }

    pub(super) fn mesh_action(&mut self, a: Action) -> bool {
        if self.pmi_action(a) {
            return true;
        }
        if self.mesh.terrain
            && self.mesh_active()
            && matches!(a, Action::Back | Action::MapTerrain3d)
        {
            self.mesh_close_terrain();
            return true;
        }
        let mine = matches!(
            a,
            Action::MeshOrbitLeft
                | Action::MeshOrbitRight
                | Action::MeshOrbitUp
                | Action::MeshOrbitDown
                | Action::MeshViewFront
                | Action::MeshViewTop
                | Action::MeshViewSide
                | Action::MeshViewIso
                | Action::MeshShading
                | Action::MeshProjection
                | Action::MeshGrid
                | Action::MeshPointsBigger
                | Action::MeshPointsSmaller
                | Action::MeshPointColour
                | Action::MeshEdl
                | Action::MeshMolStyle
                | Action::MeshMolColour
                | Action::MeshTractOpacity
        );
        let viewer = matches!(
            a,
            Action::ZoomIn
                | Action::ZoomOut
                | Action::ZoomFit
                | Action::PanLeft
                | Action::PanRight
                | Action::PanUp
                | Action::PanDown
        );
        let review = matches!(
            a,
            Action::MeshWalk
                | Action::MeshFly
                | Action::MeshViewpoints
                | Action::MeshSaveViewpoint
                | Action::MeshSectionPlane
                | Action::MeshSectionBox
                | Action::MeshSectionFlip
                | Action::MeshSectionClear
                | Action::MeshCutMeasure
                | Action::MeshBcf
                | Action::MeshChecks
                | Action::MeshModels
                | Action::MeshAddModel
                | Action::MeshAddModelFolder
        );
        if review {
            if self.mesh_active() {
                self.bim_action(a);
            }
            return true;
        }
        if !mine && !viewer {
            return false;
        }
        if !self.mesh_active() {
            return mine;
        }
        // Walking or flying ends at a standard view, a fit, an orbit or a pan.
        if self.mesh.bim.nav.is_some()
            && !matches!(
                a,
                Action::MeshShading
                    | Action::MeshGrid
                    | Action::MeshPointsBigger
                    | Action::MeshPointsSmaller
                    | Action::MeshPointColour
                    | Action::MeshEdl
                    | Action::MeshMolStyle
                    | Action::MeshMolColour
                    | Action::MeshTractOpacity
                    | Action::ZoomIn
                    | Action::ZoomOut
                    | Action::MeshProjection
            )
        {
            self.bim_nav(specular_render_3d::NavMode::Orbit);
        }
        if self.mesh.bim.nav.is_some() && matches!(a, Action::MeshProjection) {
            return true;
        }
        let now = self.now();
        let m = &mut self.mesh;
        match a {
            Action::MeshOrbitLeft => m.camera.orbit(-ORBIT_KEY, 0.0),
            Action::MeshOrbitRight => m.camera.orbit(ORBIT_KEY, 0.0),
            Action::MeshOrbitUp => m.camera.orbit(0.0, -ORBIT_KEY),
            Action::MeshOrbitDown => m.camera.orbit(0.0, ORBIT_KEY),
            Action::MeshViewFront => m.look(StandardView::Front),
            Action::MeshViewTop => m.look(StandardView::Top),
            Action::MeshViewSide => m.look(StandardView::Side),
            Action::MeshViewIso => m.look(StandardView::Iso),
            // Splats have no surface to shade.
            Action::MeshShading if m.splats.is_some() => return true,
            Action::MeshShading => {
                m.shading = m.shading.next();
                let msg = shading_name(m.shading);
                self.chrome.toast(msg, None, now);
            }
            Action::MeshProjection => {
                m.projection = m.projection.toggled();
                m.camera.projection = m.projection;
                let msg = projection_name(m.projection);
                self.chrome.toast(msg, None, now);
            }
            Action::MeshGrid => match (m.mol.as_mut(), m.tracts.as_mut()) {
                // A molecule has its own (off at first: it would dwarf the molecule), and
                // so has a tractogram.
                (Some(mol), _) => mol.grid = !mol.grid,
                (None, Some(tv)) => tv.grid = !tv.grid,
                (None, None) => m.grid = !m.grid,
            },
            // A tractogram: `]` and `[` its thickness, `c` its colouring.
            Action::MeshPointsBigger | Action::MeshPointsSmaller if m.tracts.is_some() => {
                if let Some(tv) = m.tracts.as_mut() {
                    tv.step_width(a == Action::MeshPointsBigger);
                }
            }
            Action::MeshPointColour if m.tracts.is_some() => {
                if let Some(tv) = m.tracts.as_mut() {
                    tv.next_colour();
                    let msg = tv.colour_name();
                    self.chrome.toast(msg, None, now);
                }
            }
            // Tractograms only (disabled otherwise).
            Action::MeshTractOpacity => {
                if let Some(tv) = m.tracts.as_mut() {
                    tv.step_opacity();
                    let msg = tv.opacity_name();
                    self.chrome.toast(msg, None, now);
                }
            }
            // Point clouds only (disabled otherwise).
            Action::MeshPointsBigger
            | Action::MeshPointsSmaller
            | Action::MeshPointColour
            | Action::MeshEdl
                if m.cloud.is_none() =>
            {
                return true;
            }
            Action::MeshPointsBigger => m.step_size(true),
            Action::MeshPointsSmaller => m.step_size(false),
            Action::MeshPointColour => {
                if let Some(c) = &m.cloud {
                    m.colour = m.colour.next_for(&c.loading.info.channels);
                }
                let msg = colour_name(m.colour);
                self.chrome.toast(msg, None, now);
            }
            Action::MeshEdl => m.edl = !m.edl,
            Action::MeshMolStyle | Action::MeshMolColour => {
                // Molecules only (disabled otherwise).
                let Some(mol) = m.mol.as_mut() else {
                    return true;
                };
                let msg = if a == Action::MeshMolStyle {
                    mol.next_style();
                    molecule::style_name(mol.style)
                } else {
                    mol.next_colouring();
                    molecule::colouring_name(mol.colouring)
                };
                self.chrome.toast(msg, None, now);
            }
            Action::ZoomIn => m.camera.zoom_by(ZOOM_STEP),
            Action::ZoomOut => m.camera.zoom_by(1.0 / ZOOM_STEP),
            Action::ZoomFit => m.fit(),
            _ => {
                let (w, h) = m.size();
                let step = PAN_KEY * w.min(h);
                let (dx, dy) = match a {
                    Action::PanLeft => (-step, 0.0),
                    Action::PanRight => (step, 0.0),
                    Action::PanUp => (0.0, -step),
                    _ => (0.0, step),
                };
                let upp = m.units_per_px();
                m.camera.pan_px(dx, dy, upp);
            }
        }
        self.mesh.changed();
        self.redraw = true;
        true
    }

    /// A button press or release at physical point `p` over the image area; true when the
    /// 3D view took it.
    pub(super) fn mesh_button(
        &mut self,
        p: (f32, f32),
        down: bool,
        b: specular_widgets::Button,
    ) -> bool {
        use specular_widgets::Button;
        if b == Button::Primary && self.bim_button(p, down) {
            return true;
        }
        if !down {
            if let Some(at) = self.mesh.click.take() {
                self.mesh_pick(at);
            }
            return self.mesh.drag.take().is_some();
        }
        let scale = f64::from(self.dl.scale.max(0.1));
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        let inside = self
            .mesh
            .rect
            .is_some_and(|r| p.0 >= r.x && p.0 < r.x + r.w && p.1 >= r.y && p.1 < r.y + r.h);
        if !inside {
            return false;
        }
        self.mesh.drag = match b {
            Button::Middle => Some(Drag::Pan(lp)),
            Button::Primary if self.mods.shift_key() => Some(Drag::Pan(lp)),
            Button::Primary => Some(Drag::Orbit(lp)),
            Button::Secondary => None,
        };
        self.mesh.click = (matches!(self.mesh.drag, Some(Drag::Orbit(_)))
            && (self.mesh.mol.is_some() || self.mesh.dense.is_some()))
        .then_some(p);
        self.mesh.drag.is_some()
    }

    /// Pointer motion at physical `p`: turns or moves the model while dragging. True when
    /// it did.
    pub(super) fn mesh_motion(&mut self, p: (f32, f32)) -> bool {
        if self.bim_motion(p) {
            return true;
        }
        let Some(drag) = self.mesh.drag else {
            return false;
        };
        let scale = f64::from(self.dl.scale.max(0.1));
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        let m = &mut self.mesh;
        if let Some(c) = m.click {
            let d = f64::from(p.0 - c.0).hypot(f64::from(p.1 - c.1)) / scale;
            if d <= CLICK_SLOP {
                // Not a drag yet: the model stays put.
                return true;
            }
            m.click = None;
            // Turn from where the button went down.
            if let Drag::Orbit(_) = drag {
                let start = (f64::from(c.0) / scale, f64::from(c.1) / scale);
                m.drag = Some(Drag::Orbit(start));
            }
        }
        let drag = m.drag.unwrap_or(drag);
        if let Drag::Orbit(last) = drag
            && m.bim.nav.is_some()
        {
            m.drag = Some(Drag::Orbit(lp));
            return self.bim_look((lp.0 - last.0, lp.1 - last.1));
        }
        match drag {
            Drag::Orbit(last) => {
                m.camera.orbit(
                    (lp.0 - last.0) * ORBIT_PER_PX,
                    (lp.1 - last.1) * ORBIT_PER_PX,
                );
                m.drag = Some(Drag::Orbit(lp));
            }
            Drag::Pan(last) => {
                let upp = m.units_per_px() * scale;
                m.camera.pan_px(lp.0 - last.0, lp.1 - last.1, upp);
                m.drag = Some(Drag::Pan(lp));
            }
        }
        m.changed();
        self.redraw = true;
        true
    }

    /// A click at physical `p` on a molecule: picks the atom under it (or clears the
    /// picks on nothing); on connectivity surfaces, shows the row of the seed under it.
    fn mesh_pick(&mut self, p: (f32, f32)) {
        let now = self.now();
        let m = &mut self.mesh;
        let Some(r) = m.rect else {
            return;
        };
        let (w, h) = (f64::from(r.w.max(1.0)), f64::from(r.h.max(1.0)));
        let (x, y) = (f64::from(p.0 - r.x), f64::from(p.1 - r.y));
        let (o, d) = m.camera.ray(&m.bounds, x, y, w, h);
        if let (Some(dense), Some(model)) = (m.dense.as_mut(), m.model.as_ref()) {
            match dense.pick(model, o, d) {
                Ok(Picked::Missed) => {}
                Ok(Picked::NoSeed) => {
                    self.chrome.toast(t!("dconn-no-seed"), None, now);
                }
                Ok(Picked::Seed(model)) => {
                    m.model = Some(model);
                    m.gpu = None;
                    m.changed();
                }
                Err(e) => {
                    specular_log::warn!(target: specular_log::DECODE, "dconn: {e}");
                    self.chrome.toast(t!("dconn-row-failed"), None, now);
                }
            }
            self.redraw = true;
            return;
        }
        let Some(mol) = m.mol.as_mut() else {
            return;
        };
        let hit = mol.scene.pick(mol.molecule(), o, d).map(|(i, _)| i);
        mol.click(hit);
        m.changed();
        self.redraw = true;
    }

    /// A wheel turn: zooms the model. True when the 3D view took it.
    pub(super) fn mesh_wheel(&mut self, lines: (f32, f32)) -> bool {
        if !self.mesh_active() {
            return false;
        }
        self.mesh.camera.zoom_by(1.2f64.powf(f64::from(lines.1)));
        self.mesh.changed();
        self.redraw = true;
        true
    }

    /// The readout lines while a model is shown.
    pub(super) fn mesh_lines(&self) -> Option<Vec<String>> {
        if !self.mesh_active() || self.bim_capturing() {
            return None;
        }
        let mut lines = self.mesh_lines_of()?;
        lines.extend(self.bim_lines());
        Some(lines)
    }

    fn mesh_lines_of(&self) -> Option<Vec<String>> {
        let m = &self.mesh;
        let tiles = m
            .tiles
            .as_ref()
            .map(|t| t.lines(t.model.tiles_read()))
            .unwrap_or_default();
        if let Some(c) = &m.cloud
            && m.model.is_none()
        {
            let mut lines = points_readout(&c.loading, &m.bounds, m.point_look());
            lines.extend(tiles);
            return Some(lines);
        }
        if let Some(sp) = &m.splats {
            return Some(splats_readout(&sp.splats, &m.bounds, m.grid, m.projection));
        }
        if let Some(mol) = &m.mol {
            return Some(mol.readout(&m.bounds, m.projection));
        }
        if let Some(tv) = &m.tracts {
            return Some(tv.readout(m.projection));
        }
        let mut lines = readout(m.mesh()?, &m.bounds, m.shading, m.projection, m.grid);
        if let Some(d) = &m.dense {
            lines.extend(d.lines());
        }
        lines.extend(tiles);
        Some(lines)
    }

    /// Draws the model or the cloud into the scene over the image area: uploads a model on
    /// the first call, a cloud's points as they load. True while a cloud is still filling
    /// in (the scene needs another frame).
    pub(super) fn mesh_render(
        mv: &mut MeshView,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        scale: f32,
    ) -> bool {
        if mv.cloud.is_some() && mv.model.is_none() {
            let grid = mv.grid;
            return Self::points_render(mv, gpu, enc, target, size, scale, grid);
        }
        if mv.splats.is_some() {
            return Self::splats_render(mv, gpu, enc, target, size);
        }
        if let (Some(mol), Some(r)) = (mv.mol.as_mut(), mv.rect) {
            let renderer = mv.renderer.get_or_insert_with(|| MeshRenderer::new(gpu));
            let view = View {
                rect: px_rect(r),
                camera: mv.camera,
                shading: mv.shading,
                grid: mol.grid,
                scale,
            };
            mol.render(renderer, gpu, enc, target, size, &view, mv.bounds);
            return false;
        }
        if let (Some(tv), Some(r)) = (mv.tracts.as_mut(), mv.rect) {
            let renderer = mv.renderer.get_or_insert_with(|| MeshRenderer::new(gpu));
            let view = View {
                rect: px_rect(r),
                camera: mv.camera,
                shading: mv.shading,
                grid: tv.grid,
                scale,
            };
            tv.render(renderer, gpu, enc, target, size, &view);
            return false;
        }
        let (Some(model), Some(r)) = (mv.bim.restyled.as_ref().or(mv.model.as_ref()), mv.rect)
        else {
            return false;
        };
        let renderer = mv.renderer.get_or_insert_with(|| MeshRenderer::new(gpu));
        if mv.gpu.is_none() {
            let t0 = Instant::now();
            let g = renderer.upload(gpu, &model.mesh, &model.textures);
            specular_log::info!(
                target: specular_log::DECODE,
                "mesh: {} triangles on the GPU in {:.0} ms ({:.1} MiB)",
                g.triangles,
                t0.elapsed().as_secs_f64() * 1e3,
                g.bytes as f64 / f64::from(1u32 << 20)
            );
            mv.gpu = Some(g);
        }
        // Section caps, uploaded when the cut changes.
        if mv.bim.sections.is_empty() {
            mv.bim.caps_gpu = None;
        } else if mv.bim.caps_gpu.is_none() {
            let caps = mv.caps_mesh().cloned();
            let renderer = mv.renderer.get_or_insert_with(|| MeshRenderer::new(gpu));
            mv.bim.caps_gpu = caps.map(|c| renderer.upload_caps(gpu, &c));
        }
        let (Some(g), Some(renderer)) = (mv.gpu.as_ref(), mv.renderer.as_mut()) else {
            return false;
        };
        let view = View {
            rect: px_rect(r),
            camera: mv.camera,
            shading: mv.shading,
            grid: mv.grid,
            scale,
        };
        renderer.set_sections(&mv.bim.sections);
        // A tileset is framed on its root box: tiles coming and going don't move the view,
        // and its points line up with its models.
        let frame = mv.tiles.as_ref().map(|_| mv.bounds);
        renderer.set_frame(frame);
        renderer.draw_sectioned(gpu, enc, target, size, g, mv.bim.caps_gpu.as_ref(), &view);
        // A tileset's points over its models, hidden behind them (the grid drawn once).
        mv.cloud.is_some() && Self::points_render(mv, gpu, enc, target, size, scale, false)
    }

    /// Feeds the cloud's newly read points to the GPU and draws what is there.
    fn points_render(
        mv: &mut MeshView,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        scale: f32,
        grid: bool,
    ) -> bool {
        let (Some(cloud), Some(r)) = (mv.cloud.as_ref(), mv.rect) else {
            return false;
        };
        let loading = cloud.loading.clone();
        let renderer = mv
            .point_renderer
            .get_or_insert_with(|| PointRenderer::new(gpu));
        let g = mv.points.get_or_insert_with(|| PointsGpu::new(&loading));
        let was = g.complete();
        let t0 = Instant::now();
        g.feed(renderer, gpu, &loading);
        if g.complete() && !was {
            specular_log::info!(
                target: specular_log::DECODE,
                "points: {} on the GPU ({:.1} MiB), last upload {:.1} ms",
                g.len(),
                g.bytes() as f64 / f64::from(1u32 << 20),
                t0.elapsed().as_secs_f64() * 1e3
            );
        }
        let view = PointView {
            rect: px_rect(r),
            camera: mv.camera,
            grid,
            scale,
            colour: mv.colour,
            size: mv.point_size,
            world: false,
            edl: mv.edl,
            budget: POINT_BUDGET,
        };
        renderer.set_sections(&mv.bim.sections);
        renderer.set_frame(mv.tiles.as_ref().map(|_| mv.bounds));
        // Over a tileset's models: tested against their depth.
        let under = mv
            .model
            .as_ref()
            .and(mv.renderer.as_ref())
            .and_then(MeshRenderer::depth);
        renderer.draw_over(gpu, enc, target, size, g, &view, under);
        let refining = !g.complete();
        if refining {
            mv.changed();
        }
        refining
    }
}

impl App {
    /// Uploads the next splats and draws what is on the GPU (sorted for this camera). True
    /// while splats are still being uploaded (the scene needs another frame).
    fn splats_render(
        mv: &mut MeshView,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) -> bool {
        let (Some(sp), Some(r)) = (mv.splats.as_ref(), mv.rect) else {
            return false;
        };
        let renderer = mv
            .splat_renderer
            .get_or_insert_with(|| SplatRenderer::new(gpu));
        let g = mv
            .splat_gpu
            .get_or_insert_with(|| SplatsGpu::new(gpu, sp.splats.clone()));
        let was = g.complete();
        let t0 = Instant::now();
        g.feed(gpu);
        if g.complete() && !was {
            specular_log::info!(
                target: specular_log::DECODE,
                "splats: {} on the GPU ({:.1} MiB), last upload {:.1} ms{}",
                g.len(),
                g.bytes() as f64 / f64::from(1u32 << 20),
                t0.elapsed().as_secs_f64() * 1e3,
                if g.notes.is_empty() {
                    String::new()
                } else {
                    format!("; {}", g.notes.join("; "))
                }
            );
        }
        let view = SplatView {
            rect: px_rect(r),
            camera: mv.camera,
            grid: mv.grid,
        };
        renderer.draw(gpu, enc, target, size, g, &view);
        if g.sort_ms > 0.0 {
            specular_log::debug!(
                target: specular_log::DECODE,
                "splats: sorted {} in {:.1} ms",
                g.drawn,
                g.sort_ms
            );
        }
        // Still uploading, or a new order sorting on a worker: draw again.
        let filling = !g.complete() || g.sorting();
        if filling {
            mv.changed();
        }
        filling
    }
}

/// The readout lines for splats: their box, how many, the harmonics, the grid and the
/// projection.
fn splats_readout(
    s: &specular_decode::specular_splat::Splats,
    bounds: &Bounds,
    grid: bool,
    projection: Projection,
) -> Vec<String> {
    let mut lines = Vec::new();
    if !bounds.is_empty() {
        lines.push(size_line(bounds.size().map(f64::from), ""));
    }
    lines.push(t!("splats-count", count = s.len() as i64));
    if s.sh_degree > 0 {
        lines.push(t!("splats-harmonics", degree = i64::from(s.sh_degree)));
    }
    lines.extend(grid_line(bounds, grid, ""));
    lines.push(projection_name(projection));
    lines
}

/// A layout rectangle as the renderers' x, y, width, height.
fn px_rect(r: PxRect) -> [u32; 4] {
    [
        r.x.max(0.0) as u32,
        r.y.max(0.0) as u32,
        r.w.max(1.0) as u32,
        r.h.max(1.0) as u32,
    ]
}
