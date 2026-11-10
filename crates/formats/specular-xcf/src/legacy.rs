//! GIMP's legacy (≤ 2.8) layer modes, kept for old files: sRGB-coded maths with the old
//! "composite alpha is the smaller of the two" rule. Results never change the backdrop's alpha.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Legacy {
    Multiply,
    Screen,
    Difference,
    Addition,
    Subtract,
    Darken,
    Lighten,
    HsvHue,
    HsvSaturation,
    HslColor,
    HsvValue,
    Divide,
    Dodge,
    Burn,
    Hardlight,
    Softlight,
    GrainExtract,
    GrainMerge,
}

use specular_blend::float as fl;
use specular_blend::{Mode, blend1};

const HALF: f32 = 128.0 / 255.0;

#[inline]
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Clamp to 0..1 with NaN going to 0.
#[inline]
fn safe_clamp(v: f32) -> f32 {
    if v > 0.0 {
        if v < 1.0 { v } else { 1.0 }
    } else {
        0.0
    }
}

#[inline]
fn channel(k: Legacy, i: f32, l: f32) -> f32 {
    match k {
        // The plain ones through the formulas shared with the other readers; the rest keep
        // GIMP 2.8's own rounding and degenerate cases.
        Legacy::Multiply => blend1(Mode::Multiply, i, l),
        Legacy::Screen => fl::screen(i, l),
        Legacy::Difference => blend1(Mode::Difference, i, l),
        Legacy::Addition => clamp01(fl::add(i, l)),
        Legacy::Subtract => clamp01(fl::subtract(i, l)),
        Legacy::Darken => blend1(Mode::Darken, i, l),
        Legacy::Lighten => blend1(Mode::Lighten, i, l),
        Legacy::Divide => safe_clamp(i / l),
        Legacy::Dodge => safe_clamp(i / (1.0 - l)),
        Legacy::Burn => {
            let c = 1.0 - (1.0 - i) / l;
            // NaN (0/0) goes to 1.
            if c < 0.0 {
                0.0
            } else if c < 1.0 {
                c
            } else {
                1.0
            }
        }
        Legacy::Hardlight => {
            if l > HALF {
                let c = ((1.0 - f64::from(i)) * (1.0 - f64::from((l - HALF) * 2.0))) as f32;
                (1.0 - c).min(1.0)
            } else {
                (i * (l * 2.0)).min(1.0)
            }
        }
        Legacy::Softlight => fl::soft_light_pegtop(i, l),
        Legacy::GrainExtract => clamp01(i - l + HALF),
        Legacy::GrainMerge => clamp01(i + l - HALF),
        Legacy::HsvHue | Legacy::HsvSaturation | Legacy::HslColor | Legacy::HsvValue => i,
    }
}

fn rgb_to_hsv(c: [f64; 3]) -> [f64; 3] {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    let delta = max - min;
    if delta > 0.0001 {
        let s = delta / max;
        let mut h = if c[0] == max {
            let h = (c[1] - c[2]) / delta;
            if h < 0.0 { h + 6.0 } else { h }
        } else if c[1] == max {
            2.0 + (c[2] - c[0]) / delta
        } else {
            4.0 + (c[0] - c[1]) / delta
        };
        h /= 6.0;
        [h, s, max]
    } else {
        [0.0, 0.0, max]
    }
}

fn hsv_to_rgb(hsv: [f64; 3]) -> [f64; 3] {
    let [h, s, v] = hsv;
    if s == 0.0 {
        return [v; 3];
    }
    let h = if h == 1.0 { 0.0 } else { h } * 6.0;
    let i = h as i32;
    let f = h - f64::from(i);
    let w = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => [v, t, w],
        1 => [q, v, w],
        2 => [w, v, t],
        3 => [w, q, v],
        4 => [t, w, v],
        5 => [v, w, q],
        // Out-of-range hues (from out-of-gamut input) leave the colour unset in GIMP; black
        // is as good a guess.
        _ => [0.0; 3],
    }
}

fn rgb_to_hsl(c: [f64; 3]) -> [f64; 3] {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    let l = (max + min) / 2.0;
    if max == min {
        return [-1.0, 0.0, l];
    }
    let s = if l <= 0.5 {
        (max - min) / (max + min)
    } else {
        (max - min) / (2.0 - max - min)
    };
    let delta = max - min;
    let mut h = if c[0] == max {
        (c[1] - c[2]) / delta
    } else if c[1] == max {
        2.0 + (c[2] - c[0]) / delta
    } else {
        4.0 + (c[0] - c[1]) / delta
    };
    h /= 6.0;
    if h < 0.0 {
        h += 1.0;
    }
    [h, s, l]
}

fn hsl_value(n1: f64, n2: f64, mut hue: f64) -> f64 {
    if hue > 6.0 {
        hue -= 6.0;
    } else if hue < 0.0 {
        hue += 6.0;
    }
    if hue < 1.0 {
        n1 + (n2 - n1) * hue
    } else if hue < 3.0 {
        n2
    } else if hue < 4.0 {
        n1 + (n2 - n1) * (4.0 - hue)
    } else {
        n1
    }
}

fn hsl_to_rgb(hsl: [f64; 3]) -> [f64; 3] {
    let [h, s, l] = hsl;
    if s == 0.0 {
        return [l; 3];
    }
    let m2 = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let m1 = 2.0 * l - m2;
    [
        hsl_value(m1, m2, h * 6.0 + 2.0),
        hsl_value(m1, m2, h * 6.0),
        hsl_value(m1, m2, h * 6.0 - 2.0),
    ]
}

/// The blended colour of one pixel (`i` backdrop, `l` layer, both sRGB-coded).
#[inline]
fn blend(k: Legacy, i: [f32; 3], l: [f32; 3]) -> [f32; 3] {
    let wide = |c: [f32; 3]| c.map(f64::from);
    let narrow = |c: [f64; 3]| c.map(|v| v as f32);
    match k {
        Legacy::HsvHue => {
            let lh = rgb_to_hsv(wide(l));
            let mut o = rgb_to_hsv(wide(i));
            // No effect from a grey layer (GIMP bug #123296).
            if lh[1] != 0.0 {
                o[0] = lh[0];
            }
            narrow(hsv_to_rgb(o))
        }
        Legacy::HsvSaturation => {
            let lh = rgb_to_hsv(wide(l));
            let mut o = rgb_to_hsv(wide(i));
            o[1] = lh[1];
            narrow(hsv_to_rgb(o))
        }
        Legacy::HsvValue => {
            let lh = rgb_to_hsv(wide(l));
            let mut o = rgb_to_hsv(wide(i));
            o[2] = lh[2];
            narrow(hsv_to_rgb(o))
        }
        Legacy::HslColor => {
            let lh = rgb_to_hsl(wide(l));
            let mut o = rgb_to_hsl(wide(i));
            o[0] = lh[0];
            o[1] = lh[1];
            narrow(hsl_to_rgb(o))
        }
        _ => [
            channel(k, i[0], l[0]),
            channel(k, i[1], l[1]),
            channel(k, i[2], l[2]),
        ],
    }
}

/// Composites `layer` onto `inp` (RGBA, sRGB-coded, in place).
pub(crate) fn apply(k: Legacy, inp: &mut [f32], layer: &[f32], mask: Option<&[f32]>, opacity: f32) {
    for (n, (o, l)) in inp
        .chunks_exact_mut(4)
        .zip(layer.chunks_exact(4))
        .enumerate()
    {
        let in_a = o[3];
        let mut comp_a = in_a.min(l[3]) * opacity;
        if let Some(m) = mask {
            comp_a *= m[n];
        }
        let new_a = in_a + (1.0 - in_a) * comp_a;
        if comp_a != 0.0 && new_a != 0.0 {
            let ratio = comp_a / new_a;
            let c = blend(k, [o[0], o[1], o[2]], [l[0], l[1], l[2]]);
            for b in 0..3 {
                o[b] = c[b] * ratio + o[b] * (1.0 - ratio);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_hsl_round_trip() {
        for c in [
            [0.2, 0.5, 0.9],
            [0.9, 0.1, 0.3],
            [0.4, 0.4, 0.4],
            [1.0, 0.0, 0.5],
        ] {
            let back = hsv_to_rgb(rgb_to_hsv(c));
            let back2 = hsl_to_rgb(rgb_to_hsl(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 1e-9, "{c:?} hsv");
                assert!((back2[i] - c[i]).abs() < 1e-9, "{c:?} hsl");
            }
        }
    }

    #[test]
    fn keeps_backdrop_alpha() {
        let mut inp = [0.5, 0.5, 0.5, 0.6];
        apply(Legacy::Multiply, &mut inp, &[0.5, 0.5, 0.5, 1.0], None, 1.0);
        assert_eq!(inp[3], 0.6);
        // comp alpha 0.6, new alpha 0.84: ratio 0.714…; comp 0.25.
        let r = 0.6 / (0.6 + 0.4 * 0.6);
        assert!((inp[0] - (0.25 * r + 0.5 * (1.0 - r))).abs() < 1e-6);
    }
}
