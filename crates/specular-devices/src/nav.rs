//! From a device's state to motion: dead zones, a speed curve and rates.
//!
//! [`Nav`] is a velocity the app integrates over each frame's time while it is not
//! [`Nav::is_still`]: the view moves over the image (2D) or the camera turns about the model
//! (3D).
//!
//! - **Gamepad, 2D**: left stick pans; right stick up/down and the triggers zoom (right in,
//!   left out).
//! - **Gamepad, 3D**: left stick orbits, right stick pans, triggers zoom.
//! - **SpaceMouse, 2D**: push sideways and forward/back to pan, push down to zoom in, pull up
//!   to zoom out.
//! - **SpaceMouse, 3D**: tilt and twist orbit (the model turns as the cap does), push sideways
//!   and up/down to pan, push forward to zoom in.

use crate::{Pad, Puck};

/// The user's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// Deflection ignored around the centre, 0…0.5 of full.
    pub dead_zone: f32,
    /// Overall speed, 1 = normal.
    pub speed: f32,
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            dead_zone: 0.15,
            speed: 1.0,
        }
    }
}

/// Viewport sizes per second at full deflection.
pub const PAN_RATE: f32 = 1.5;
/// Natural log of the zoom factor per second at full deflection (×5 per second).
pub const ZOOM_RATE: f32 = 1.609;
/// Radians per second at full deflection.
pub const ORBIT_RATE: f32 = 3.0;

/// A velocity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Nav {
    /// The view's movement over the image in viewport sizes per second, x right, y down
    /// (2D); or the model's on screen (3D).
    pub pan: [f32; 2],
    /// ln(zoom factor) per second, + zooms in.
    pub zoom: f32,
    /// Turn in radians per second, as a drag of the same direction would (x: about the
    /// up axis, y: about the screen's horizontal).
    pub orbit: [f32; 2],
}

impl Nav {
    pub fn is_still(&self) -> bool {
        *self == Nav::default()
    }

    /// The motion over `dt` seconds: (pan in viewport sizes, zoom factor, orbit radians).
    pub fn over(&self, dt: f32) -> ([f32; 2], f32, [f32; 2]) {
        (
            [self.pan[0] * dt, self.pan[1] * dt],
            (self.zoom * dt).exp(),
            [self.orbit[0] * dt, self.orbit[1] * dt],
        )
    }
}

/// The speed curve past the dead zone: slow and precise near the centre, full speed at the
/// edge (0.2·u + 0.8·u³ of the deflection beyond the dead zone, rescaled to 0…1).
pub fn curve(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    0.2 * u + 0.8 * u * u * u
}

/// One axis with its dead zone and curve.
pub fn shape(v: f32, dead_zone: f32) -> f32 {
    let dz = dead_zone.clamp(0.0, 0.9);
    let a = v.abs();
    if !a.is_finite() || a <= dz {
        return 0.0;
    }
    curve((a - dz) / (1.0 - dz)).copysign(v)
}

/// A stick with a round dead zone (diagonals as fast as the axes).
pub fn stick(v: [f32; 2], dead_zone: f32) -> [f32; 2] {
    let m = v[0].hypot(v[1]);
    if !m.is_finite() || m == 0.0 {
        return [0.0, 0.0];
    }
    let s = shape(m.min(1.0), dead_zone) / m;
    [v[0] * s, v[1] * s]
}

/// A gamepad's motion.
pub fn pad(p: &Pad, t: &Tuning, three_d: bool) -> Nav {
    let dz = t.dead_zone;
    let k = t.speed.max(0.0);
    let left = stick(p.left, dz);
    let right = stick(p.right, dz);
    let triggers = shape(p.rt, dz) - shape(p.lt, dz);
    if three_d {
        Nav {
            pan: [right[0] * PAN_RATE * k, right[1] * PAN_RATE * k],
            zoom: triggers * ZOOM_RATE * k,
            orbit: [left[0] * ORBIT_RATE * k, left[1] * ORBIT_RATE * k],
        }
    } else {
        Nav {
            pan: [left[0] * PAN_RATE * k, left[1] * PAN_RATE * k],
            zoom: ((-right[1] + triggers).clamp(-1.0, 1.0)) * ZOOM_RATE * k,
            orbit: [0.0, 0.0],
        }
    }
}

/// A SpaceMouse's motion.
pub fn puck(p: &Puck, t: &Tuning, three_d: bool) -> Nav {
    let s = |v: f32| shape(v, t.dead_zone);
    let k = t.speed.max(0.0);
    if three_d {
        Nav {
            pan: [s(p.t[0]) * PAN_RATE * k, -s(p.t[1]) * PAN_RATE * k],
            zoom: -s(p.t[2]) * ZOOM_RATE * k,
            // Twisting about the up axis turns the model like a sideways drag; tilting
            // forward (about x) like a downward one.
            orbit: [-s(p.r[1]) * ORBIT_RATE * k, -s(p.r[0]) * ORBIT_RATE * k],
        }
    } else {
        Nav {
            pan: [s(p.t[0]) * PAN_RATE * k, s(p.t[2]) * PAN_RATE * k],
            zoom: -s(p.t[1]) * ZOOM_RATE * k,
            orbit: [0.0, 0.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_zone_and_curve() {
        assert_eq!(shape(0.1, 0.15), 0.0);
        assert_eq!(shape(-0.15, 0.15), 0.0);
        assert_eq!(shape(1.0, 0.15), 1.0);
        assert_eq!(shape(-1.0, 0.15), -1.0);
        // Half way past the dead zone is well under half speed.
        let h = shape(0.575, 0.15);
        assert!(h > 0.1 && h < 0.3, "{h}");
        // Monotonic.
        let mut last = 0.0;
        for i in 0..=100 {
            let v = shape(i as f32 / 100.0, 0.15);
            assert!(v >= last);
            last = v;
        }
        assert_eq!(shape(f32::NAN, 0.1), 0.0);
    }

    #[test]
    fn round_stick_dead_zone() {
        assert_eq!(stick([0.1, 0.1], 0.15), [0.0, 0.0]);
        let d = stick([std::f32::consts::FRAC_1_SQRT_2; 2], 0.15);
        let a = stick([1.0, 0.0], 0.15);
        assert!((d[0].hypot(d[1]) - a[0]).abs() < 1e-3);
    }

    #[test]
    fn pad_2d_and_3d() {
        let t = Tuning::default();
        let rest = Pad::default();
        assert!(pad(&rest, &t, false).is_still());
        let p = Pad {
            left: [1.0, 0.0],
            right: [0.0, -1.0],
            ..Pad::default()
        };
        let n = pad(&p, &t, false);
        assert_eq!(n.pan, [PAN_RATE, 0.0]);
        assert!((n.zoom - ZOOM_RATE).abs() < 1e-6);
        let n3 = pad(&p, &t, true);
        assert_eq!(n3.orbit, [ORBIT_RATE, 0.0]);
        assert_eq!(n3.pan, [0.0, -PAN_RATE]);
        let trig = Pad {
            lt: 1.0,
            ..Pad::default()
        };
        assert!(pad(&trig, &t, false).zoom < 0.0);
        let fast = Tuning { speed: 2.0, ..t };
        assert_eq!(pad(&p, &fast, false).pan, [2.0 * PAN_RATE, 0.0]);
        let (pan, f, _) = n.over(0.5);
        assert_eq!(pan, [0.75, 0.0]);
        assert!((f - 5f32.sqrt()).abs() < 1e-2);
    }

    #[test]
    fn puck_2d_and_3d() {
        let t = Tuning::default();
        // Pushed down: zooms in; forward (away, −z): the view moves up.
        let down = Puck {
            t: [0.0, -1.0, -1.0],
            r: [0.0; 3],
        };
        let n = puck(&down, &t, false);
        assert!(n.zoom > 0.0);
        assert!(n.pan[1] < 0.0);
        let twist = Puck {
            t: [0.0; 3],
            r: [0.0, -1.0, 0.0],
        };
        let n = puck(&twist, &t, true);
        assert!(n.orbit[0] > 0.0 && n.orbit[1] == 0.0);
        assert!(puck(&Puck::default(), &t, true).is_still());
    }
}
