//! Program kernels of glow and dehaze, run on the whole-image statistics grid (the input
//! of the node reduced to at most 1024 px a side, read with `src_at`). Planes are row-major
//! f32 arrays in the scratch `S` at word offsets given in each step's words; the box means,
//! block means, histograms and guided-filter coefficients are the tone kernels'
//! ([`crate::ops::tone::kernels`]). [`super::cpu`] runs the same steps in Rust for the tests.

use crate::graph::Kernel;

/// Words: destination (three planes), width, plane size, threshold (f32), knee (f32).
/// The bright pass of glow: each pixel times `max(soft, Y − T) / Y`, with a quadratic knee
/// of half-width `knee` around the threshold `T`.
pub static BRIGHT: Kernel = Kernel {
    name: "fx_bright_v1",
    atomic: false,
    helpers: &[],
    body: "    let c = src_at(i32(t.x), i32(t.y));
    let y = max(dot(c.rgb, vec3<f32>(0.2627, 0.6780, 0.0593)), 0.0);
    let th = pf(p + 3u);
    let k = pf(p + 4u);
    let q = clamp(y - th + k, 0.0, 2.0 * k);
    let soft = q * q / (4.0 * k + 1e-6);
    let f = max(soft, y - th) / max(y, 1e-6);
    let at = pu(p) + t.y * pu(p + 1u) + t.x;
    let ps = pu(p + 2u);
    let v = max(c.rgb, vec3<f32>(0.0)) * f;
    S[at] = v.x;
    S[at + ps] = v.y;
    S[at + 2u * ps] = v.z;",
};

/// Words: source, width, height, destination, destination width, factor s, planes,
/// source plane size, destination plane size. The mean of each s×s block (clipped at the
/// edges) of every plane.
pub static DOWN: Kernel = Kernel {
    name: "fx_down_v1",
    atomic: false,
    helpers: &[],
    body: "    let w = pu(p + 1u);
    let h = pu(p + 2u);
    let f = pu(p + 5u);
    let x1 = min(t.x * f + f, w);
    let y1 = min(t.y * f + f, h);
    let n = f32((x1 - t.x * f) * (y1 - t.y * f));
    for (var c = 0u; c < pu(p + 6u); c++) {
        var s = 0.0;
        for (var y = t.y * f; y < y1; y++) {
            for (var x = t.x * f; x < x1; x++) { s += S[pu(p) + c * pu(p + 7u) + y * w + x]; }
        }
        S[pu(p + 3u) + c * pu(p + 8u) + t.y * pu(p + 4u) + t.x] = s / n;
    }",
};

/// Words: mode, destination, width, A offset. Mode 0: the darkest channel of each pixel
/// (at least 0). Mode 1: the darkest of each channel divided by the atmospheric light A
/// (three words at the A offset).
pub static DARK: Kernel = Kernel {
    name: "fx_dark_v1",
    atomic: false,
    helpers: &[],
    body: "    var c = max(src_at(i32(t.x), i32(t.y)).rgb, vec3<f32>(0.0));
    if (pu(p) == 1u) {
        let a = pu(p + 3u);
        c = c / max(vec3<f32>(S[a], S[a + 1u], S[a + 2u]), vec3<f32>(1e-4));
    }
    S[pu(p + 1u) + t.y * pu(p + 2u) + t.x] = min(min(c.x, c.y), c.z);",
};

/// Words: source, destination, width, height, vertical (0/1), radius, a (f32), b (f32).
/// The minimum over `[x − r, x + r]` (or the column), clipped at the edges, written as
/// `a + b·min`.
pub static MIN: Kernel = Kernel {
    name: "fx_min_v1",
    atomic: false,
    helpers: &[],
    body: "    let w = pu(p + 2u);
    let h = pu(p + 3u);
    let r = i32(pu(p + 5u));
    var m = 3.0e38;
    if (pu(p + 4u) == 0u) {
        let lo = max(i32(t.x) - r, 0);
        let hi = min(i32(t.x) + r, i32(w) - 1);
        for (var i = lo; i <= hi; i++) { m = min(m, S[pu(p) + t.y * w + u32(i)]); }
    } else {
        let lo = max(i32(t.y) - r, 0);
        let hi = min(i32(t.y) + r, i32(h) - 1);
        for (var i = lo; i <= hi; i++) { m = min(m, S[pu(p) + u32(i) * w + t.x]); }
    }
    S[pu(p + 1u) + t.y * w + t.x] = pf(p + 6u) + pf(p + 7u) * m;",
};

/// Words: dark channel, width, threshold offset, destination (four words per row), slack
/// (f32). One thread per row: the sum of R, G, B of the pixels whose dark channel is at
/// least the threshold less the slack (a histogram bin: the percentile lies inside its
/// bin), and their count.
pub static ATMOS_ROWS: Kernel = Kernel {
    name: "fx_atmos_rows_v1",
    atomic: false,
    helpers: &[],
    body: "    let y = t.x;
    let w = pu(p + 1u);
    let th = S[pu(p + 2u)] - pf(p + 4u);
    var acc = vec4<f32>(0.0);
    for (var x = 0u; x < w; x++) {
        if (S[pu(p) + y * w + x] >= th) {
            acc += vec4<f32>(max(src_at(i32(x), i32(y)).rgb, vec3<f32>(0.0)), 1.0);
        }
    }
    let o = pu(p + 3u) + 4u * y;
    S[o] = acc.x;
    S[o + 1u] = acc.y;
    S[o + 2u] = acc.z;
    S[o + 3u] = acc.w;",
};

/// Words: rows, height, destination. One thread: the atmospheric light, the mean colour of
/// the counted pixels (at least 1e-3 a channel), then its luminance.
pub static ATMOS: Kernel = Kernel {
    name: "fx_atmos_v1",
    atomic: false,
    helpers: &[],
    body: "    var acc = vec4<f32>(0.0);
    for (var y = 0u; y < pu(p + 1u); y++) {
        let o = pu(p) + 4u * y;
        acc += vec4<f32>(S[o], S[o + 1u], S[o + 2u], S[o + 3u]);
    }
    let a = max(acc.xyz / max(acc.w, 1.0), vec3<f32>(1e-3));
    let d = pu(p + 2u);
    S[d] = a.x;
    S[d + 1u] = a.y;
    S[d + 2u] = a.z;
    S[d + 3u] = dot(a, vec3<f32>(0.2627, 0.6780, 0.0593));",
};

/// Words: destination, width, Y(A) offset. The dehaze guide: luminance over the
/// atmosphere's, in 0 … 2.
pub static GUIDE: Kernel = Kernel {
    name: "fx_guide_v1",
    atomic: false,
    helpers: &[],
    body: "    let c = src_at(i32(t.x), i32(t.y));
    let y = dot(c.rgb, vec3<f32>(0.2627, 0.6780, 0.0593));
    S[pu(p) + t.y * pu(p + 1u) + t.x] = clamp(y / max(S[pu(p + 2u)], 1e-4), 0.0, 2.0);",
};
