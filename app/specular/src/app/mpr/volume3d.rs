//! The fourth slot's live 3D view: the volume rendered by [`VrRenderer`], orbited like a
//! turntable about the patient's head–foot axis.
//!
//! Left drag orbits, Shift+left or middle drag pans, the wheel zooms, right drag shifts the
//! transfer function's window (in MIP and MinIP the shared window, as in the 2D views).
//! `Shift+arrows` (and `↑` `↓`) orbit from the keyboard, `+` `-` zoom and `f` fits once the
//! 3D view is the active one. `Shift+V` cycles the presets, `Shift+C` the renderings,
//! `Shift+X` the clipping: none, the box the three 2D views show (zooming or panning them
//! crops the volume), or the crosshair's corner facing the viewer cut away. While the view
//! moves it renders at half resolution; at rest it refines over a few frames (shadows and
//! occlusion over more in cinematic mode). The surface at the transfer function's threshold
//! exports as binary STL in patient millimetres.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use specular_render_volume::{Bricks, Clip, Mode, Preset, View3d};

use super::views::{R, V3, Views, add, cross, dot, sub, turn, unit};

/// Radians per logical pixel dragged.
pub(crate) const ORBIT_PER_PX: f64 = 0.01;
/// Radians per orbit key press.
pub(crate) const ORBIT_KEY: f64 = std::f64::consts::PI / 12.0;
/// The view renders coarsely for this long after a wheel or key move (more may follow).
const SETTLE: Duration = Duration::from_millis(180);

/// What the 3D view clips away.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ClipMode {
    #[default]
    Off,
    /// Keeps what the three 2D views show.
    Box,
    /// Cuts away the crosshair's corner that faces the viewer.
    Cut,
}

impl ClipMode {
    pub(crate) fn id(self) -> &'static str {
        match self {
            ClipMode::Off => "off",
            ClipMode::Box => "box",
            ClipMode::Cut => "cut",
        }
    }

    pub(crate) fn next(self) -> ClipMode {
        match self {
            ClipMode::Off => ClipMode::Box,
            ClipMode::Box => ClipMode::Cut,
            ClipMode::Cut => ClipMode::Off,
        }
    }
}

/// The camera: unit right and down vectors (it looks along `right × down`), a zoom over
/// the fitted size and a pan (mm along right and down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Orbit {
    pub right: V3,
    pub down: V3,
    pub zoom: f64,
    pub pan: [f64; 2],
}

impl Default for Orbit {
    /// From the front (anterior), head up.
    fn default() -> Orbit {
        Orbit {
            right: [1.0, 0.0, 0.0],
            down: [0.0, 0.0, -1.0],
            zoom: 1.0,
            pan: [0.0; 2],
        }
    }
}

impl Orbit {
    /// Turns the object by `dx` radians about the head–foot axis (dragging right brings
    /// the patient's right side forward) and tips it by `dy` about the view's horizontal.
    pub(crate) fn orbit(&mut self, dx: f64, dy: f64) {
        let z = [0.0, 0.0, 1.0];
        let right = turn(self.right, z, -dx);
        let down = turn(self.down, z, -dx);
        let down = turn(down, right, -dy);
        self.right = unit(right);
        self.down = unit(sub(down, right.map(|c| c * dot(down, right))));
    }

    pub(crate) fn zoom_by(&mut self, f: f64) {
        self.zoom = (self.zoom * f).clamp(0.25, 40.0);
    }

    pub(crate) fn forward(&self) -> V3 {
        unit(cross(self.right, self.down))
    }
}

/// The 3D view's state in a session.
#[derive(Default)]
pub(crate) struct Vr3 {
    pub orbit: Orbit,
    pub mode: Mode,
    /// `None` until chosen: the series' kind decides ([`Preset::initial`]).
    pub preset: Option<Preset>,
    /// The user's transfer window (`None`: the preset's own).
    pub tf_window: Option<(f64, f64)>,
    pub clip: ClipMode,
    /// The exported surface is smoothed.
    pub smooth: bool,
    pub bricks: Option<Bricks>,
    pub bricks_rx: Option<Receiver<Bricks>>,
    /// A drag is moving the view.
    pub dragging: bool,
    /// A wheel or key move: coarse until then.
    pub settle: Option<Instant>,
    /// The save dialog's answer, then the export's.
    pub save_rx: Option<Receiver<PathBuf>>,
    pub export_rx: Option<Receiver<Result<(PathBuf, usize), String>>>,
}

impl Vr3 {
    pub(crate) fn preset(&self, hounsfield: bool) -> Preset {
        self.preset.unwrap_or_else(|| Preset::initial(hounsfield))
    }

    /// The transfer window in use (centre, width).
    pub(crate) fn tf_window(&self, hounsfield: bool, series: (f64, f64)) -> (f64, f64) {
        self.tf_window
            .unwrap_or_else(|| self.preset(hounsfield).transfer(series).window())
    }

    /// The surface threshold of the transfer function at its window.
    pub(crate) fn threshold(&self, hounsfield: bool, series: (f64, f64)) -> f64 {
        let t = self.preset(hounsfield).transfer(series);
        t.iso_at(self.tf_window(hounsfield, series))
    }

    /// A wheel or key move: coarse for a moment.
    pub(crate) fn nudge(&mut self) {
        self.settle = Some(Instant::now() + SETTLE);
    }

    pub(crate) fn interactive(&self, now: Instant) -> bool {
        self.dragging || self.settle.is_some_and(|t| now < t)
    }
}

/// The bounding sphere's diameter over the view's shorter side: a little under 1, as the
/// corners are mostly air (no turn shrinks or grows the volume).
const FIT: f64 = 0.85;

/// Patient mm per logical pixel in the 3D view `r`: the volume's bounding sphere nearly
/// fits.
pub(crate) fn mm_per_px(views: &Views, orbit: &Orbit, r: R) -> f64 {
    let radius = views
        .corners
        .iter()
        .map(|k| dot(sub(*k, views.vc), sub(*k, views.vc)).sqrt())
        .fold(1.0f64, f64::max);
    2.0 * radius * FIT / r[2].min(r[3]).max(1.0) / orbit.zoom
}

/// The clipping planes for `mode`: the 2D views' visible rectangles as a box, or the
/// crosshair's corner facing the camera (looking along `forward`).
pub(crate) fn clip(mode: ClipMode, views: &Views, rects: &[R; 3], forward: V3) -> Clip {
    let mut c = Clip::default();
    let plane = |n: V3, p: V3| [n[0], n[1], n[2], -dot(n, p)];
    match mode {
        ClipMode::Off => {}
        ClipMode::Box => {
            for (v, r) in rects.iter().enumerate() {
                let f = &views.frames[v];
                let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                let neg = |d: V3| d.map(|x| -x);
                for (n, at) in [
                    (f.right, (r[0], cy)),
                    (neg(f.right), (r[0] + r[2], cy)),
                    (f.down, (cx, r[1])),
                    (neg(f.down), (cx, r[1] + r[3])),
                ] {
                    c.keep[c.keeps] = plane(n, views.to_patient(v, *r, at));
                    c.keeps += 1;
                }
            }
        }
        ClipMode::Cut => {
            c.cut = Some([0, 1, 2].map(|v| {
                let n = views.normal(v);
                let n = if dot(n, forward) > 0.0 {
                    n.map(|x| -x)
                } else {
                    n
                };
                plane(n, views.c)
            }));
        }
    }
    c
}

/// Everything about the series a [`View3d`] needs.
pub(crate) struct Series {
    pub hounsfield: bool,
    /// The default window (centre, width).
    pub window: (f64, f64),
}

/// The view drawn into the slot `slot` (logical) at `scale`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn view(
    vr: &Vr3,
    views: &Views,
    rects: &[R; 3],
    slot: R,
    scale: f64,
    series: &Series,
    window: specular_render::Window,
    canvas: [f32; 4],
    now: Instant,
) -> View3d {
    let o = &vr.orbit;
    let px = slot.map(|c| (c * scale).round().max(0.0) as u32);
    let centre = add(add(views.vc, o.right, o.pan[0]), o.down, o.pan[1]);
    View3d {
        rect: px,
        centre,
        right: o.right,
        down: o.down,
        mm: mm_per_px(views, o, slot) / scale,
        mode: vr.mode,
        preset: vr.preset(series.hounsfield),
        series: series.window,
        tf_window: vr.tf_window(series.hounsfield, series.window),
        window,
        clip: clip(vr.clip, views, rects, o.forward()),
        canvas,
        interactive: vr.interactive(now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbits_keep_an_orthonormal_frame() {
        let mut o = Orbit::default();
        assert_eq!(o.forward(), [0.0, 1.0, 0.0]);
        // A quarter turn right: looking from the patient's right side (−x) towards +x.
        o.orbit(std::f64::consts::FRAC_PI_2, 0.0);
        let f = o.forward();
        assert!((f[0] - 1.0).abs() < 1e-9, "{f:?}");
        // Tipping forward looks down from the head.
        let mut o = Orbit::default();
        o.orbit(0.0, std::f64::consts::FRAC_PI_2);
        let f = o.forward();
        assert!((f[2] + 1.0).abs() < 1e-9, "{f:?}");
        for _ in 0..50 {
            o.orbit(0.37, -0.21);
        }
        assert!((dot(o.right, o.right) - 1.0).abs() < 1e-9);
        assert!((dot(o.down, o.down) - 1.0).abs() < 1e-9);
        assert!(dot(o.right, o.down).abs() < 1e-9);
    }
}
