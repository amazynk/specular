//! CPU twins of the colour kernels (tests only): the WGSL's f32 arithmetic, step by step,
//! from the same uniforms and tables.

use crate::colour::oklab::{from_rec2020_f32 as oklab, to_rec2020_f32 as oklab_inv};
use crate::ops::tone::LUMA;

use super::bands_f32;

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn luma(c: [f32; 3]) -> f32 {
    c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2]
}

fn hypot(a: f32, b: f32) -> f32 {
    (a * a + b * b).sqrt()
}

pub fn balance(u: &[f32], c: [f32; 4]) -> [f32; 4] {
    let rgb = [c[0], c[1], c[2]];
    let ev = (luma(rgb).max(1e-9) / 0.1845).log2();
    let ws = 1.0 / (1.0 + ((ev - u[1]) * u[0]).exp());
    let wh = 1.0 / (1.0 + (-(ev - u[2]) * u[0]).exp());
    let wm = (1.0 - ws - wh).max(0.0);
    let wt = [ws, wm, wh, 1.0];
    let (mut ta, mut tb, mut gl) = (0f32, 0f32, 0f32);
    for (i, w) in wt.iter().enumerate() {
        let q = &u[3 + i * 3..];
        ta += w * q[0];
        tb += w * q[1];
        gl += w * q[2];
    }
    let lab = oklab(rgb);
    let f = gl.exp2();
    let l = lab[0] * f;
    let ab = [lab[1] * f + ta * l, lab[2] * f + tb * l];
    let s = hypot(ab[0], ab[1]) / l.abs().max(1e-6);
    let fv = 1.0 + u[15] * (1.0 - smoothstep(0.0, 0.5, s));
    let sat = u[17];
    let k = (fv * (1.0 + u[16]) * (1.0 + sat)).max(0.0);
    let fl = (1.0 - 0.25 * sat * s.min(1.0)) * (u[18] * smoothstep(0.0, 0.3, s)).exp2();
    let o = oklab_inv([l * fl, ab[0] * (k * fl), ab[1] * (k * fl)]);
    [o[0], o[1], o[2], c[3]]
}

pub fn equalizer(u: &[f32], c: [f32; 4]) -> [f32; 4] {
    let lab = oklab([c[0], c[1], c[2]]);
    let ch = hypot(lab[1], lab[2]);
    if ch <= 0.0 {
        return c;
    }
    let s = ch / lab[0].abs().max(1e-6);
    let m = smoothstep(0.0, u[1], s);
    let d = bands_f32(&u[2..], u[0], lab[1] / ch, lab[2] / ch).map(|v| v * m);
    let (sr, cr) = d[0].sin_cos();
    let fl = d[2].exp2();
    let k = (1.0 + d[1]).max(0.0) * fl;
    let o = oklab_inv([
        lab[0] * fl,
        (lab[1] * cr - lab[2] * sr) * k,
        (lab[1] * sr + lab[2] * cr) * k,
    ]);
    [o[0], o[1], o[2], c[3]]
}

pub fn point(u: &[f32], c: [f32; 4]) -> [f32; 4] {
    let rgb = [c[0], c[1], c[2]];
    let lab = oklab(rgb);
    let ch = hypot(lab[1], lab[2]);
    let s = ch / lab[0].abs().max(1e-6);
    let mut w = 1.0 - smoothstep(0.5, 1.0, (s - u[3]).abs() * u[6]);
    w *= 1.0 - smoothstep(0.5, 1.0, (lab[0] - u[4]).abs() * u[7]);
    if u[0] > 0.0 {
        let mut dh = std::f32::consts::PI;
        if ch > 0.0 {
            dh = (lab[2] * u[1] - lab[1] * u[2])
                .atan2(lab[1] * u[1] + lab[2] * u[2])
                .abs();
        }
        w *= (1.0 - smoothstep(0.5, 1.0, dh * u[5])) * smoothstep(0.0, 0.04, s);
    }
    if u[11] > 0.0 {
        let g = luma(rgb);
        return [
            g + (rgb[0] - g) * w,
            g + (rgb[1] - g) * w,
            g + (rgb[2] - g) * w,
            c[3],
        ];
    }
    let (sr, cr) = (u[8] * w).sin_cos();
    let fl = (u[10] * w).exp2();
    let k = (1.0 + u[9] * w).max(0.0) * fl;
    let o = oklab_inv([
        lab[0] * fl,
        (lab[1] * cr - lab[2] * sr) * k,
        (lab[1] * sr + lab[2] * cr) * k,
    ]);
    [o[0], o[1], o[2], c[3]]
}

pub fn black_white(u: &[f32], c: [f32; 4]) -> [f32; 4] {
    let rgb = [c[0], c[1], c[2]];
    let y = luma(rgb);
    let lab = oklab(rgb);
    let ch = hypot(lab[1], lab[2]);
    let mut g = y;
    if ch > 0.0 {
        let s = ch / lab[0].abs().max(1e-6);
        let d = bands_f32(&u[2..], u[0], lab[1] / ch, lab[2] / ch)[0];
        g = y * (d * smoothstep(0.0, u[1], s)).exp2();
    }
    [g, g, g, c[3]]
}

/// The ACEScct linear segment (S-2016-001), as the kernel's f32 literals round.
const CCT_A: f32 = 10.540_237_741_654_5_f64 as f32;
const CCT_B: f32 = 0.072_905_534_195_835_5_f64 as f32;
const CCT_Y: f32 = 0.155_251_141_552_511_f64 as f32;

fn enc(sp: u32, x: f32) -> f32 {
    match sp {
        2 if x <= 0.0078125 => CCT_A * x + CCT_B,
        2 => (x.log2() + 9.72) / 17.52,
        3 if x <= 0.003_130_8 => 12.92 * x,
        3 => 1.055 * x.powf(1.0 / 2.4) - 0.055,
        4 if x == 0.0 => 0.0,
        4 => x.signum() * x.abs().powf(1.0 / 2.4),
        _ => x,
    }
}

fn dec(sp: u32, y: f32) -> f32 {
    match sp {
        2 if y <= CCT_Y => (y - CCT_B) / CCT_A,
        2 => (y * 17.52 - 9.72).exp2().min(65504.0),
        3 if y <= 0.04045 => y / 12.92,
        3 => ((y + 0.055) / 1.055).powf(2.4),
        4 if y == 0.0 => 0.0,
        4 => y.signum() * y.abs().powf(2.4),
        _ => y,
    }
}

fn l1(t: &[f32], n: usize, k: usize, x: f32) -> f32 {
    let f = x.clamp(0.0, (n - 1) as f32);
    let i = (f.floor() as usize).min(n - 2);
    let w = f - i as f32;
    t[i * 3 + k] * (1.0 - w) + t[(i + 1) * 3 + k] * w
}

fn lut3(t: &[f32], c: [f32; 3]) -> [f32; 3] {
    let n = t[2] as usize;
    let tet = t[3] > 0.0;
    let v = &t[10..];
    let nm = (n - 1) as f32;
    let mut i = [0usize; 3];
    let mut f = [0f32; 3];
    for k in 0..3 {
        let x = ((c[k] - t[4 + k]) / (t[7 + k] - t[4 + k]).max(1e-30) * nm).clamp(0.0, nm);
        i[k] = (x.floor() as usize).min(n - 2);
        f[k] = x - i[k] as f32;
    }
    let at = |r: usize, g: usize, b: usize| -> [f32; 3] {
        let o = (((i[0] + r) * n + i[1] + g) * n + i[2] + b) * 3;
        [v[o], v[o + 1], v[o + 2]]
    };
    let (fr, fg, fb) = (f[0], f[1], f[2]);
    let comb = |terms: [(f32, [f32; 3]); 4]| -> [f32; 3] {
        std::array::from_fn(|k| {
            terms[0].0 * terms[0].1[k]
                + terms[1].0 * terms[1].1[k]
                + terms[2].0 * terms[2].1[k]
                + terms[3].0 * terms[3].1[k]
        })
    };
    let (v000, v111) = (at(0, 0, 0), at(1, 1, 1));
    if tet {
        return if fr > fg {
            if fg > fb {
                comb([
                    (1.0 - fr, v000),
                    (fr - fg, at(1, 0, 0)),
                    (fg - fb, at(1, 1, 0)),
                    (fb, v111),
                ])
            } else if fr > fb {
                comb([
                    (1.0 - fr, v000),
                    (fr - fb, at(1, 0, 0)),
                    (fb - fg, at(1, 0, 1)),
                    (fg, v111),
                ])
            } else {
                comb([
                    (1.0 - fb, v000),
                    (fb - fr, at(0, 0, 1)),
                    (fr - fg, at(1, 0, 1)),
                    (fg, v111),
                ])
            }
        } else if fb > fg {
            comb([
                (1.0 - fb, v000),
                (fb - fg, at(0, 0, 1)),
                (fg - fr, at(0, 1, 1)),
                (fr, v111),
            ])
        } else if fb > fr {
            comb([
                (1.0 - fg, v000),
                (fg - fb, at(0, 1, 0)),
                (fb - fr, at(0, 1, 1)),
                (fr, v111),
            ])
        } else {
            comb([
                (1.0 - fg, v000),
                (fg - fr, at(0, 1, 0)),
                (fr - fb, at(1, 1, 0)),
                (fb, v111),
            ])
        };
    }
    let lerp = |a: [f32; 3], b: [f32; 3], t: f32| -> [f32; 3] {
        std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * t)
    };
    let c00 = lerp(v000, at(0, 0, 1), fb);
    let c01 = lerp(at(0, 1, 0), at(0, 1, 1), fb);
    let c10 = lerp(at(1, 0, 0), at(1, 0, 1), fb);
    let c11 = lerp(at(1, 1, 0), v111, fb);
    lerp(lerp(c00, c01, fg), lerp(c10, c11, fg), fr)
}

fn pw(x: f32, e: f32) -> f32 {
    if x <= 0.0 {
        return if e == 0.0 { 1.0 } else { 0.0 };
    }
    x.powf(e)
}

fn lut_exp(x: f32, e: f32, neg: f32) -> f32 {
    if x >= 0.0 {
        pw(x, e)
    } else if neg == 0.0 {
        0.0
    } else if neg == 1.0 {
        -pw(-x, e)
    } else {
        x
    }
}

fn explin(q: &[f32], inv: bool, mirror: bool, x: f32) -> f32 {
    let (g, o, s, b) = (q[0], q[1], q[2], q[3]);
    let (v, sg) = if mirror && x < 0.0 {
        (-x, -1.0)
    } else {
        (x, 1.0)
    };
    if !inv {
        if v < b {
            return sg * (v / s);
        }
        return sg * pw((v + o) / (1.0 + o), g);
    }
    if v < b / s {
        return sg * (v * s);
    }
    sg * ((1.0 + o) * pw(v, 1.0 / g) - o)
}

fn log(q: &[f32], inv: bool, lnb: f32, x: f32) -> f32 {
    let has = q[4] > 0.0;
    if !inv {
        if has && x <= q[5] {
            return q[6] * x + q[7];
        }
        let v = (q[2] * x + q[3]).max(f32::MIN_POSITIVE);
        return q[0] * v.ln() / lnb + q[1];
    }
    if has && x <= q[6] * q[5] + q[7] {
        return (x - q[7]) / q[6];
    }
    ((lnb * ((x - q[1]) / q[0])).exp() - q[3]) / q[2]
}

fn cdl(r: &[f32], c: [f32; 3]) -> [f32; 3] {
    let clp = r[2] > 0.0;
    let sat = r[4];
    let lw = [0.2126f32, 0.7152, 0.0722];
    let dot = |v: [f32; 3]| lw[0] * v[0] + lw[1] * v[1] + lw[2] * v[2];
    let mut v = c;
    if r[3] == 0.0 {
        for k in 0..3 {
            let mut x = v[k] * r[5 + k] + r[8 + k];
            if clp {
                x = x.clamp(0.0, 1.0);
            }
            if x > 0.0 || !clp {
                if x >= 0.0 {
                    x = pw(x, r[11 + k]);
                }
            } else {
                x = 0.0;
            }
            v[k] = x;
        }
        let l = dot(v);
        v = v.map(|x| l + sat * (x - l));
        if clp {
            v = v.map(|x| x.clamp(0.0, 1.0));
        }
        return v;
    }
    let l = dot(v);
    v = v.map(|x| l + (x - l) / sat);
    if clp {
        v = v.map(|x| x.clamp(0.0, 1.0));
    }
    for k in 0..3 {
        let mut x = v[k];
        if x > 0.0 {
            x = x.powf(1.0 / r[11 + k]);
        } else if clp {
            x = 0.0;
        }
        x = (x - r[8 + k]) / r[5 + k];
        if clp {
            x = x.clamp(0.0, 1.0);
        }
        v[k] = x;
    }
    v
}

/// The table interpreter (`dv_lut_run`).
pub fn run_table(t: &[f32], c: [f32; 3]) -> [f32; 3] {
    let mut v = c;
    let ns = t[0] as usize;
    let mut o = 1;
    for _ in 0..ns {
        let r = &t[o..];
        match r[0] as u32 {
            1 => {
                v = std::array::from_fn(|i| {
                    r[2 + i * 3] * v[0] + r[3 + i * 3] * v[1] + r[4 + i * 3] * v[2] + r[11 + i]
                });
            }
            2 => v = v.map(|x| (x * r[2] + r[3]).max(r[4]).min(r[5])),
            3 => {
                let n = r[2] as usize;
                for k in 0..3 {
                    let lo = r[3 + k];
                    let span = (r[6 + k] - lo).max(1e-30);
                    v[k] = l1(&r[9..], n, k, (v[k] - lo) / span * (n - 1) as f32);
                }
            }
            4 => v = lut3(r, v),
            5 => {
                for k in 0..3 {
                    v[k] = lut_exp(v[k], r[2 + k], r[5]);
                }
            }
            6 => {
                for k in 0..3 {
                    v[k] = explin(&r[4 + k * 4..], r[2] > 0.0, r[3] > 0.0, v[k]);
                }
            }
            7 => {
                for k in 0..3 {
                    v[k] = log(&r[4 + k * 8..], r[2] > 0.0, r[3], v[k]);
                }
            }
            8 => v = cdl(r, v),
            _ => {}
        }
        o += r[1] as usize;
    }
    v
}

fn mat3(m: &[f32], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i * 3] * v[0] + m[i * 3 + 1] * v[1] + m[i * 3 + 2] * v[2])
}

pub fn lut(u: &[f32], table: &[f32], c: [f32; 4]) -> [f32; 4] {
    let sp = u[0].to_bits();
    let rgb = [c[0], c[1], c[2]];
    let e = mat3(&u[2..11], rgb).map(|x| enc(sp, x));
    let d = run_table(table, e).map(|y| dec(sp, y));
    let o = mat3(&u[11..20], d);
    let s = u[1];
    [
        rgb[0] + (o[0] - rgb[0]) * s,
        rgb[1] + (o[1] - rgb[1]) * s,
        rgb[2] + (o[2] - rgb[2]) * s,
        c[3],
    ]
}
