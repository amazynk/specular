//! Which tiles a view draws: screen-space error refinement from the root, as the 3D Tiles
//! standard describes it.
//!
//! A tile outside the view (its bounding sphere wholly beyond a side of the view frustum, or
//! the eye outside its viewer request volume) is skipped with everything under it. A visible
//! tile whose geometric error, projected to the screen, exceeds the allowed error in pixels
//! is refined: with `REPLACE` its children are drawn in its place, with `ADD` they are drawn
//! as well. A tile whose children are an external tileset or an implicit tree is always
//! passed through (as CesiumJS does). The projected error at distance `d` is
//! `error · height / (2 d tan(fov/2))` in perspective and `error · height / view height` in
//! orthographic; `d` is the distance from the eye to the tile's bounding volume.

use crate::fetch::Fetch;
use crate::math::{V3, cross, dot, sub, unit};
use crate::tileset::{Refine, Tileset};

/// The default allowed error, pixels (CesiumJS's default).
pub const MAX_SSE: f64 = 16.0;

/// A view, in the tileset's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: V3,
    /// Unit forward and up.
    pub dir: V3,
    pub up: V3,
    /// Vertical field of view (radians), in perspective.
    pub fov_y: f64,
    /// Orthographic: the height the view shows (tileset units); `None`: perspective.
    pub ortho_height: Option<f64>,
    pub width_px: f64,
    pub height_px: f64,
    pub max_sse: f64,
    /// Skip tiles outside the view.
    pub cull: bool,
}

/// What a traversal chose.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    /// Tiles whose contents are drawn, in traversal order.
    pub tiles: Vec<u32>,
    /// Tiles looked at, and skipped as outside the view.
    pub visited: usize,
    pub culled: usize,
}

impl View {
    /// The screen-space error of `error` at distance `d`.
    pub fn sse(&self, error: f64, d: f64) -> f64 {
        match self.ortho_height {
            Some(h) => error * self.height_px / h.max(1e-12),
            None => {
                let d = d.max(1e-7);
                error * self.height_px / (2.0 * d * (self.fov_y / 2.0).tan())
            }
        }
    }

    /// Whether a sphere is (at least partly) inside the view frustum's sides and in front
    /// of the eye.
    pub fn sees(&self, c: V3, r: f64) -> bool {
        let f = unit(self.dir);
        let up = unit(self.up);
        let right = unit(cross(f, up));
        let v = sub(c, self.eye);
        let z = dot(v, f);
        if z < -r {
            return false;
        }
        let aspect = self.width_px / self.height_px.max(1.0);
        let (x, y) = (dot(v, right), dot(v, up));
        match self.ortho_height {
            Some(h) => {
                let (hh, hw) = (h / 2.0, h / 2.0 * aspect);
                x.abs() <= hw + r && y.abs() <= hh + r
            }
            None => {
                let ty = (self.fov_y / 2.0).tan();
                let tx = ty * aspect;
                // Distance to each side plane through the eye: n · v, with n the plane's
                // outward unit normal.
                let side = |t: f64, a: f64| (a.abs() - t * z) / (1.0 + t * t).sqrt();
                side(tx, x) <= r && side(ty, y) <= r
            }
        }
    }
}

impl Tileset {
    /// The tiles `view` draws; pending children (external tilesets, implicit subtrees) are
    /// read through `fetch` as the traversal reaches them. At most `max_tiles` tiles are
    /// chosen (the traversal stops there).
    pub fn select(&mut self, view: &View, fetch: &dyn Fetch, max_tiles: usize) -> Selection {
        let mut sel = Selection::default();
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            sel.visited += 1;
            let t = &self.tiles[i as usize];
            let visible = !view.cull || {
                let v = &t.volume;
                view.sees(v.centre(), v.radius())
            };
            let requested = t.request.is_none_or(|r| r.contains(view.eye));
            if !visible || !requested {
                sel.culled += 1;
                continue;
            }
            let pass = t.through;
            let sse = view.sse(t.error, t.volume.distance(view.eye));
            let wants = pass || sse > view.max_sse;
            if wants && self.has_pending(i) {
                self.expand(i, fetch);
            }
            let t = &self.tiles[i as usize];
            let refine = wants && !t.children.is_empty();
            if (!refine || t.refine == Refine::Add) && !t.contents.is_empty() {
                if sel.tiles.len() >= max_tiles {
                    break;
                }
                sel.tiles.push(i);
            }
            if refine {
                for &c in t.children.iter().rev() {
                    stack.push(c);
                }
            }
        }
        sel
    }
}

/// What [`Tileset::select_near`] chose.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Near {
    /// The finest tiles within reach, in traversal order.
    pub tiles: Vec<u32>,
    /// Every tile within reach was refined as far as the tileset goes (false: the tile
    /// count ran out first).
    pub complete: bool,
}

impl Tileset {
    /// The tiles within `reach` of `at` (tileset coordinates) at the finest level the
    /// tileset has, whatever the view: what a walker there stands on and runs into. Tiles
    /// out of reach are skipped with everything under them; pending children are read
    /// through `fetch`. At most `max_tiles` tiles are chosen.
    pub fn select_near(&mut self, at: V3, reach: f64, fetch: &dyn Fetch, max_tiles: usize) -> Near {
        let mut out = Near {
            tiles: Vec::new(),
            complete: true,
        };
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            if self.tiles[i as usize].volume.distance(at) > reach {
                continue;
            }
            if self.has_pending(i) {
                self.expand(i, fetch);
            }
            let t = &self.tiles[i as usize];
            let refine = !t.children.is_empty();
            if (!refine || t.refine == Refine::Add) && !t.contents.is_empty() {
                if out.tiles.len() >= max_tiles {
                    out.complete = false;
                    break;
                }
                out.tiles.push(i);
            }
            for &c in t.children.iter().rev() {
                stack.push(c);
            }
        }
        out
    }
}
