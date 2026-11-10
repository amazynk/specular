//! Springs and easing. Springs are integrated analytically per frame so the result does not
//! depend on the frame rate.

/// Spring tuning: natural frequency (rad/s) and damping ratio (1 = critically damped).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringParams {
    pub omega: f32,
    pub zeta: f32,
}

impl SpringParams {
    /// Zoom and pan: quick, no overshoot.
    pub const VIEW: SpringParams = SpringParams {
        omega: 26.0,
        zeta: 1.0,
    };
    pub const VIEW_RELAXED: SpringParams = SpringParams {
        omega: 16.0,
        zeta: 1.0,
    };
    pub const VIEW_FAST: SpringParams = SpringParams {
        omega: 40.0,
        zeta: 1.0,
    };
    /// Chrome fades and slides: slightly soft.
    pub const UI: SpringParams = SpringParams {
        omega: 22.0,
        zeta: 0.95,
    };
    /// Snappy hover feedback.
    pub const HOVER: SpringParams = SpringParams {
        omega: 38.0,
        zeta: 1.0,
    };
}

/// A 1-D damped spring towards `target`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub params: SpringParams,
}

impl Spring {
    pub fn new(value: f32, params: SpringParams) -> Spring {
        Spring {
            value,
            velocity: 0.0,
            target: value,
            params,
        }
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    /// Jump to `v` with no motion.
    pub fn snap(&mut self, v: f32) {
        self.value = v;
        self.target = v;
        self.velocity = 0.0;
    }

    /// Advance by `dt` seconds. Returns true while still moving.
    pub fn step(&mut self, dt: f32) -> bool {
        let SpringParams { omega: w, zeta: z } = self.params;
        let x0 = self.value - self.target;
        let v0 = self.velocity;
        let dt = dt.clamp(0.0, 0.1);
        if z >= 0.999 {
            // Critically damped closed form.
            let e = (-w * dt).exp();
            let c = v0 + w * x0;
            let x = (x0 + c * dt) * e;
            // d/dt[(x0 + c t) e^{-wt}] = c e^{-wt} - w (x0 + c t) e^{-wt}
            self.value = self.target + x;
            self.velocity = c * e - w * x;
        } else {
            // Under-damped closed form.
            let wd = w * (1.0 - z * z).sqrt();
            let e = (-z * w * dt).exp();
            let a = x0;
            let b = (v0 + z * w * x0) / wd;
            let (s, c) = (wd * dt).sin_cos();
            let x = e * (a * c + b * s);
            let v = e * ((-z * w) * (a * c + b * s) + (-a * wd * s + b * wd * c));
            self.value = self.target + x;
            self.velocity = v;
        }
        let moving = !self.settled();
        if !moving {
            self.value = self.target;
            self.velocity = 0.0;
        }
        moving
    }

    pub fn settled(&self) -> bool {
        let scale = self.target.abs().max(1.0);
        (self.value - self.target).abs() < 1e-4 * scale && self.velocity.abs() < 1e-3 * scale
    }
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = 1.0 - t.clamp(0.0, 1.0);
    1.0 - t * t * t
}

pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Exponential approach factor for `dt` with a time constant `tau` (seconds).
pub fn approach(dt: f32, tau: f32) -> f32 {
    1.0 - (-dt / tau.max(1e-4)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_settles_without_overshoot() {
        let mut s = Spring::new(0.0, SpringParams::VIEW);
        s.set(1.0);
        let mut max = 0f32;
        for _ in 0..200 {
            s.step(1.0 / 60.0);
            max = max.max(s.value);
        }
        assert!(s.settled());
        assert!(max <= 1.0 + 1e-4);
    }
}
