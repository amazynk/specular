//! 3D Tiles in the 3D view: whenever the camera has moved, the tiles it needs are chosen
//! and read in the background ([`Tiles3dModel::select`], [`Tiles3dModel::scene`]); the scene
//! drawn so far stays until the new one is complete, then it is swapped in. The camera is
//! fitted on the tileset's root box, so swapping tiles never moves it.

use crossbeam_channel::Receiver;
use specular_decode::{Tiles3dModel, Tiles3dScene, Tiles3dView};
use specular_i18n::{I18n, t};
use specular_render_3d::Projection;

use specular_render_3d::Body;
use specular_render_3d::bvh::NavScene;

use super::{MeshView, PointColour};
use crate::app::App;

/// One background pass: the scene of the tiles a view chose (`None`: the same tiles as
/// shown).
type Pass = Result<Option<Tiles3dScene>, String>;

pub(in crate::app) struct TilesView {
    pub(super) model: Tiles3dModel,
    /// The tiles drawn now.
    shown: Vec<u32>,
    /// The camera revision the last pass started at.
    asked: Option<u64>,
    rx: Option<Receiver<Pass>>,
    errors: usize,
    left_out: bool,
    failed: Option<String>,
    /// Where the walker was, and the tiles drawn, when its collision scene was last
    /// started.
    walk_at: Option<[f64; 3]>,
    walk_shown: Vec<u32>,
}

impl TilesView {
    pub(super) fn new(model: Tiles3dModel) -> TilesView {
        TilesView {
            model,
            shown: Vec::new(),
            asked: None,
            rx: None,
            errors: 0,
            left_out: false,
            failed: None,
            walk_at: None,
            walk_shown: Vec::new(),
        }
    }

    /// Readout lines.
    pub(super) fn lines(&self, read: usize) -> Vec<String> {
        let loc = I18n::global().localizer();
        let mut out = vec![t!(
            "mesh-tiles3d",
            drawn = loc.integer(self.shown.len() as i64),
            read = loc.integer(read as i64)
        )];
        if self.rx.is_some() {
            out.push(t!("mesh-tiles3d-loading"));
        }
        if self.left_out {
            out.push(t!("mesh-tiles3d-left-out"));
        }
        if self.errors > 0 {
            out.push(t!("mesh-tiles3d-not-drawn", count = self.errors as i64));
        }
        if let Some(f) = &self.failed {
            out.push(t!("mesh-tiles3d-failed", reason = f.clone()));
        }
        out
    }
}

/// The camera of `m` as a tile view (drawing frame), `None` before layout.
fn view_of(m: &MeshView) -> Option<Tiles3dView> {
    let r = m.rect?;
    let (w, h) = (f64::from(r.w.max(1.0)), f64::from(r.h.max(1.0)));
    let p = m.camera.pose(&m.bounds, w, h);
    Some(Tiles3dView {
        eye: p.eye,
        dir: p.direction,
        up: p.up,
        fov_y: p.fov,
        ortho_height: (p.projection == Projection::Orthographic).then_some(p.height),
        width_px: w,
        height_px: h,
    })
}

/// How far from the walker tiles are read at their finest for it to collide with: 25 m
/// (model units as [`Body::for_model`] takes them).
fn reach(b: &specular_decode::specular_mesh::Bounds) -> f64 {
    25.0 * Body::for_model(b, None).per_metre
}

impl MeshView {
    /// Starts building what walking in a tileset collides with: the finest tiles within
    /// reach of the walker (whichever way it looks) and the drawn tiles beyond
    /// ([`Tiles3dModel::collision_tiles`]).
    pub(super) fn tiles_collision(&mut self) {
        let at = match self.bim.nav {
            Some(n) => n.eye(),
            None => match view_of(self) {
                Some(v) => v.eye,
                None => return,
            },
        };
        let b = self.bounds;
        let Some(tv) = self.tiles.as_mut() else {
            return;
        };
        tv.walk_at = Some(at);
        tv.walk_shown = tv.shown.clone();
        let model = tv.model.clone();
        let shown = tv.shown.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        specular_thread::spawn("ren-nav-bvh", move || {
            let t0 = std::time::Instant::now();
            let tiles = model.collision_tiles(at, reach(&b), &shown);
            let Ok(scene) = model.scene(&tiles, &specular_core::CancelToken::new()) else {
                return;
            };
            let cloud = scene
                .points
                .as_ref()
                .and_then(|p| p.loading.result())
                .and_then(Result::ok);
            let pts = cloud.as_ref().map(|c| &c.positions[..]);
            let s = match (&scene.mesh, pts) {
                (Some(m), Some(p)) => NavScene::from_mesh_and_points(
                    &m.mesh,
                    p,
                    super::bim::point_radius(&b, p.len()),
                ),
                (Some(m), None) => NavScene::from_mesh(&m.mesh),
                (None, Some(p)) => NavScene::from_points(p, super::bim::point_radius(&b, p.len())),
                (None, None) => NavScene::default(),
            };
            specular_log::info!(
                target: specular_log::RENDER,
                "walk: {} tiles near the walker, {} triangles and points in {:.0} ms",
                tiles.len(),
                s.len(),
                t0.elapsed().as_secs_f64() * 1e3
            );
            let _ = tx.send(s);
        });
        self.bim.building = Some(rx);
    }

    /// While walking or flying in a tileset: the collision scene is built again when the
    /// walker has moved a third of the reach, or the tiles drawn changed.
    fn tiles_collision_tick(&mut self) {
        let (Some(n), Some(tv)) = (self.bim.nav, self.tiles.as_ref()) else {
            return;
        };
        if self.bim.building.is_some() {
            return;
        }
        let eye = n.eye();
        let r = reach(&self.bounds);
        let moved = tv.walk_at.is_none_or(|a| {
            let d: f64 = (0..3).map(|k| (eye[k] - a[k]).powi(2)).sum();
            d.sqrt() > r / 3.0
        });
        if moved || tv.walk_shown != tv.shown {
            self.tiles_collision();
        }
    }
}

/// Shows what a pass found.
fn take(m: &mut MeshView, pass: Pass) {
    let Some(tv) = m.tiles.as_mut() else {
        return;
    };
    match pass {
        Ok(None) => {}
        Ok(Some(scene)) => {
            tv.shown = scene.tiles.clone();
            tv.errors = scene.errors.len();
            tv.left_out = scene.left_out > 0;
            tv.failed = None;
            // Models and points both shown; a scene with neither keeps what is drawn.
            if scene.mesh.is_some() || scene.points.is_some() {
                m.model = scene.mesh;
                m.gpu = None;
                if m.cloud.is_none()
                    && let Some(p) = &scene.points
                {
                    m.colour = PointColour::default_for(&p.loading.info.channels);
                }
                m.cloud = scene.points;
                m.points = None;
                m.geometry_changed();
            }
            m.changed();
        }
        Err(e) => {
            tv.failed = Some(e);
            m.changed();
        }
    }
}

/// One pass on this thread (screenshot tests).
#[cfg(test)]
pub(super) fn pass_now(m: &mut MeshView) {
    let (Some(view), Some(tv)) = (view_of(m), m.tiles.as_ref()) else {
        return;
    };
    let model = tv.model.clone();
    let tiles = model.select(&view);
    let pass = model
        .scene(&tiles, &specular_core::CancelToken::new())
        .map(Some)
        .map_err(|e| e.to_string());
    take(m, pass);
}

impl App {
    /// Takes a finished pass and starts the next when the camera moved since the last one.
    pub(in crate::app) fn tiles3d_tick(&mut self) {
        let m = &mut self.mesh;
        if m.tiles.is_none() {
            return;
        }
        m.tiles_collision_tick();
        let Some(tv) = m.tiles.as_mut() else {
            return;
        };
        if let Some(rx) = &tv.rx
            && let Ok(pass) = rx.try_recv()
        {
            tv.rx = None;
            take(m, pass);
            self.redraw = true;
        }
        let m = &mut self.mesh;
        let Some(tv) = m.tiles.as_mut() else {
            return;
        };
        if tv.rx.is_some() || tv.asked == Some(m.rev) || !m.fitted {
            return;
        }
        let Some(view) = view_of(m) else {
            return;
        };
        let Some(tv) = m.tiles.as_mut() else {
            return;
        };
        tv.asked = Some(m.rev);
        let (tx, rx) = crossbeam_channel::bounded(1);
        tv.rx = Some(rx);
        let model = tv.model.clone();
        let shown = tv.shown.clone();
        self.loader.background("dec-3dtiles", move |l| {
            let tiles = model.select(&view);
            let pass = if tiles == shown {
                Ok(None)
            } else {
                model
                    .scene(&tiles, &specular_core::CancelToken::new())
                    .map(Some)
                    .map_err(|e| e.to_string())
            };
            let _ = tx.send(pass);
            l.waker()();
        });
    }
}
