//! Walking and flying through a model, beside the orbit camera.
//!
//! Walking keeps the feet on the floor: a ray down from just above the step height finds the
//! floor under the feet, a drop up to [`Body::step`] is followed at once (stairs, kerbs) and a
//! longer one is fallen under gravity. The body is an upright capsule from the step height to
//! the eye, pushed out of the walls (horizontally only, so walls never lift it) after every
//! sub-step of at most half its radius, so it never tunnels through a thin wall. The ground
//! grid's plane (the model box's floor) holds it when nothing else does.
//!
//! Flying moves the eye freely along the view, the side and up, at a speed scaled to the
//! model, without collision.
//!
//! Lengths are in model units; [`Body::for_model`] sizes a person from the model's unit.

use std::f64::consts::FRAC_PI_2;

use specular_mesh::Bounds;

use crate::bvh::NavScene;
use crate::camera::{Camera, V3, radius};

/// Gravity (m/s²).
const GRAVITY_M: f64 = 9.81;
/// Walking speed (m/s), and how much faster Shift goes.
const WALK_M: f64 = 1.6;
pub const FAST: f64 = 3.0;
/// Fall speed at most (m/s).
const TERMINAL_M: f64 = 30.0;
/// The view's field in walk and fly (radians, vertical).
pub const NAV_FOV: f64 = 55.0 * std::f64::consts::PI / 180.0;
/// Pitch is kept this far from straight up or down.
const PITCH_LIMIT: f64 = FRAC_PI_2 - 0.02;

/// How the view moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NavMode {
    /// Turn the model about its centre (the default).
    #[default]
    Orbit,
    Walk,
    Fly,
}

impl NavMode {
    pub fn id(self) -> &'static str {
        match self {
            NavMode::Orbit => "orbit",
            NavMode::Walk => "walk",
            NavMode::Fly => "fly",
        }
    }
}

/// The walker's size and pace, in model units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Feet to eye.
    pub eye: f64,
    /// The capsule's radius.
    pub radius: f64,
    /// The highest step taken without falling or stopping.
    pub step: f64,
    /// Model units per metre (gravity and speed follow it).
    pub per_metre: f64,
    /// Fly speed (units/s), a share of the model's size.
    pub fly: f64,
}

impl Body {
    /// A person 1.7 m tall in a model of `metres` per unit (`None`: unknown, taken as metres
    /// when the model's height is that of a building, else scaled to it).
    pub fn for_model(b: &Bounds, metres: Option<f64>) -> Body {
        let size = b.size().map(f64::from);
        let height = size[2].max(1e-9);
        let per_metre = match metres {
            Some(m) if m > 0.0 => 1.0 / m,
            _ if (2.0..=1000.0).contains(&height) => 1.0,
            // A small object or a huge one: a walker a tenth of its height.
            _ => (height / 10.0) / 1.6,
        };
        Body {
            eye: 1.6 * per_metre,
            radius: 0.25 * per_metre,
            step: 0.4 * per_metre,
            per_metre,
            fly: (radius(b) * 0.25).max(1e-6),
        }
    }
}

/// Keys held and the mouse's look, for one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavInput {
    /// Forward (+) and back, −1 … 1.
    pub forward: f64,
    /// Right (+) and left.
    pub strafe: f64,
    /// Up (+) and down (flying only).
    pub rise: f64,
    /// Turning by keys (radians/s): left (+) and right; up (+) and down.
    pub turn: f64,
    pub tilt: f64,
    /// Turned by the mouse since the last step (radians): yaw left (+), pitch up (+).
    pub look: (f64, f64),
    /// Shift: faster.
    pub fast: bool,
}

/// Where the walker or flyer is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nav {
    pub mode: NavMode,
    /// Walking: the feet; flying: the eye.
    pub pos: V3,
    pub yaw: f64,
    pub pitch: f64,
    /// Walking: vertical speed (units/s) and whether the feet are on a floor.
    pub vz: f64,
    pub grounded: bool,
    pub body: Body,
    /// The floor's lowest level (the box's floor).
    floor: f64,
}

impl Nav {
    /// Starts `mode` from where `cam` looks at `b` from (`w` × `h` px).
    pub fn start(mode: NavMode, cam: &Camera, b: &Bounds, w: f64, h: f64, body: Body) -> Nav {
        let pose = cam.pose(b, w, h);
        let (yaw, pitch) = cam.yaw_pitch();
        let floor = f64::from(b.min[2]);
        let mut nav = Nav {
            mode,
            pos: pose.eye,
            yaw,
            pitch: pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT),
            vz: 0.0,
            grounded: false,
            body,
            floor,
        };
        if mode == NavMode::Walk {
            // Level the view a little and stand where the view looks down on: on the floor
            // under the orbit target, a body's length back from it.
            nav.pitch = nav.pitch.clamp(-0.35, 0.35);
            let target = if pose.projection == crate::Projection::Perspective && cam.eye.is_none() {
                std::array::from_fn(|k| pose.eye[k] + pose.direction[k] * pose.distance)
            } else {
                pose.eye
            };
            let back = 4.0 * body.radius;
            let (sy, cy) = nav.yaw.sin_cos();
            let x = target[0] - cy * back;
            let y = target[1] - sy * back;
            let top = f64::from(b.max[2]).max(target[2]) + body.eye;
            nav.pos = [x, y, top.min(target[2].max(floor) + body.eye)];
        }
        nav
    }

    /// Places the walker on the floor under its feet (at the start, or after a jump in a
    /// viewpoint).
    pub fn settle(&mut self, scene: Option<&NavScene>) {
        if self.mode != NavMode::Walk {
            return;
        }
        if let Some(g) = self.ground(scene, self.pos, self.body.eye) {
            self.pos[2] = g;
            self.grounded = true;
            self.vz = 0.0;
        }
    }

    /// The eye.
    pub fn eye(&self) -> V3 {
        match self.mode {
            NavMode::Walk => [self.pos[0], self.pos[1], self.pos[2] + self.body.eye],
            _ => self.pos,
        }
    }

    pub fn camera(&self, fov: f64) -> Camera {
        Camera::first_person(self.eye(), self.yaw, self.pitch, fov)
    }

    /// The floor under `p` (feet) found by a ray from `above` over it: the highest surface
    /// below that, or the box's floor.
    fn ground(&self, scene: Option<&NavScene>, p: V3, above: f64) -> Option<f64> {
        let from = [p[0], p[1], p[2] + above];
        let reach = above + (p[2] - self.floor).max(0.0) + self.body.step;
        let hit = scene.and_then(|s| s.ray(from, [0.0, 0.0, -1.0], reach));
        match hit {
            Some(t) => Some((from[2] - t).max(self.floor)),
            None if p[2] - self.floor <= self.body.step => Some(self.floor),
            None => None,
        }
    }

    /// Moves by `dt` seconds of `input`; true when the view changed.
    pub fn step(&mut self, scene: Option<&NavScene>, input: &NavInput, dt: f64) -> bool {
        let dt = dt.clamp(0.0, 0.1);
        let before = (self.pos, self.yaw, self.pitch);
        self.yaw += input.look.0 + input.turn * dt;
        self.pitch = (self.pitch + input.look.1 + input.tilt * dt).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        let fast = if input.fast { FAST } else { 1.0 };
        let (sy, cy) = self.yaw.sin_cos();
        let right = [sy, -cy, 0.0];
        match self.mode {
            NavMode::Orbit => {}
            NavMode::Fly => {
                let (sp, cp) = self.pitch.sin_cos();
                let f = [cp * cy, cp * sy, sp];
                let v = self.body.fly * fast * dt;
                for k in 0..3 {
                    self.pos[k] += (f[k] * input.forward
                        + right[k] * input.strafe
                        + [0.0, 0.0, 1.0][k] * input.rise)
                        * v;
                }
            }
            NavMode::Walk => {
                let speed = WALK_M * self.body.per_metre * fast;
                let mv = [
                    (cy * input.forward + right[0] * input.strafe) * speed * dt,
                    (sy * input.forward + right[1] * input.strafe) * speed * dt,
                ];
                self.walk(scene, mv, dt);
            }
        }
        (self.pos, self.yaw, self.pitch) != before || (self.mode == NavMode::Walk && !self.grounded)
    }

    /// Walks the feet by `mv` (horizontal), then follows the floor or falls.
    fn walk(&mut self, scene: Option<&NavScene>, mv: [f64; 2], dt: f64) {
        let b = self.body;
        let len = mv[0].hypot(mv[1]);
        let steps = ((len / (b.radius * 0.5)).ceil() as usize).clamp(1, 64);
        for _ in 0..steps {
            let z0 = self.pos[2];
            self.pos[0] += mv[0] / steps as f64;
            self.pos[1] += mv[1] / steps as f64;
            self.collide(scene);
            if !self.grounded {
                continue;
            }
            // The floor here: up or down a step is taken at once; a longer drop is fallen.
            match self.ground(scene, self.pos, b.step) {
                Some(g) if z0 - g <= b.step => self.pos[2] = g,
                _ => self.grounded = false,
            }
        }
        if self.grounded {
            self.vz = 0.0;
            return;
        }
        let gz = GRAVITY_M * b.per_metre;
        self.vz = (self.vz - gz * dt).max(-TERMINAL_M * b.per_metre);
        let z = self.pos[2] + self.vz * dt;
        let land = self.ground(scene, self.pos, b.step).unwrap_or(self.floor);
        if z <= land {
            self.pos[2] = land;
            self.vz = 0.0;
            self.grounded = true;
        } else {
            self.pos[2] = z;
        }
    }

    /// Pushes the capsule (from the step height to the eye) out of walls, horizontally.
    fn collide(&mut self, scene: Option<&NavScene>) {
        let Some(scene) = scene else { return };
        let b = self.body;
        for _ in 0..4 {
            let lo = [self.pos[0], self.pos[1], self.pos[2] + b.step + b.radius];
            let hi = [self.pos[0], self.pos[1], (self.pos[2] + b.eye).max(lo[2])];
            let Some(p) = scene.push_out(lo, hi, b.radius) else {
                return;
            };
            let l = p[0].hypot(p[1]);
            if l < 1e-12 {
                return;
            }
            // At most a radius a pass (summed contacts can overshoot in corners).
            let k = (b.radius / l).min(1.0);
            self.pos[0] += p[0] * k;
            self.pos[1] += p[1] * k;
        }
    }
}
