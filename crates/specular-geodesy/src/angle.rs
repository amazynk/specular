//! Angles in degrees, reduced exactly before the trigonometry so that 90°, 180° and their
//! multiples give exact zeros and ones.

/// `x` in (−180, 180].
pub(crate) fn normalize(x: f64) -> f64 {
    let y = x % 360.0;
    let y = if y <= -180.0 {
        y + 360.0
    } else if y > 180.0 {
        y - 360.0
    } else {
        y
    };
    if y == -180.0 { 180.0 } else { y }
}

/// `y − x` in (−180, 180].
pub(crate) fn diff(x: f64, y: f64) -> f64 {
    normalize(normalize(y) - normalize(x))
}

/// Sine and cosine of `x` degrees, exact at multiples of 90°.
pub(crate) fn sincosd(x: f64) -> (f64, f64) {
    let r = x % 360.0;
    let q = (r / 90.0).round();
    let r = (r - 90.0 * q).to_radians();
    let (s, c) = r.sin_cos();
    let (s, c) = match (q as i64).rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    };
    // No negative zeros.
    (s + 0.0, c + 0.0)
}

/// `atan2(y, x)` in degrees, in (−180, 180].
pub(crate) fn atan2d(y: f64, x: f64) -> f64 {
    let a = y.atan2(x).to_degrees();
    if a == -180.0 { 180.0 } else { a }
}

/// Scales `(s, c)` to unit length.
pub(crate) fn norm2(s: &mut f64, c: &mut f64) {
    let r = s.hypot(*c);
    *s /= r;
    *c /= r;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_quadrants() {
        assert_eq!(sincosd(90.0), (1.0, 0.0));
        assert_eq!(sincosd(-90.0), (-1.0, 0.0));
        assert_eq!(sincosd(180.0), (0.0, -1.0));
        assert_eq!(sincosd(540.0), (0.0, -1.0));
        assert_eq!(normalize(-180.0), 180.0);
        assert_eq!(normalize(190.0), -170.0);
        assert_eq!(diff(170.0, -170.0), 20.0);
        assert_eq!(atan2d(0.0, -1.0), 180.0);
    }
}
