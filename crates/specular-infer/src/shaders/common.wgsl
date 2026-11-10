// Shared by every specular-infer kernel. The Rust side prepends the bindings: the uniform
// parameter words `P` (read with pu/pf), the output `dst`, and per input `inK` with a loader
// `ldK(i) -> f32` that reads f32 storage directly or unpacks f16 storage (two per u32 word).

// Linear thread index of a 1-D kernel dispatched as a 2-D grid of 256-thread workgroups.
fn gidx(wid: vec3<u32>, nwg: vec3<u32>, li: u32) -> u32 {
    return (wid.y * nwg.x + wid.x) * 256u + li;
}

// Workgroup index of a one-workgroup-per-row kernel.
fn growid(wid: vec3<u32>, nwg: vec3<u32>) -> u32 {
    return wid.y * nwg.x + wid.x;
}

// erf, Abramowitz-Stegun 7.1.26 (|error| <= 1.5e-7), as graph::erf.
fn erf_(x: f32) -> f32 {
    let a = abs(x);
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0 - (((((1.0614054 * t - 1.4531521) * t) + 1.4214138) * t - 0.28449674) * t
        + 0.2548296) * t * exp(-a * a);
    return select(-y, y, x >= 0.0);
}

fn sigm(x: f32) -> f32 {
    return 1.0 / (1.0 + exp(-x));
}

// tanh without overflow (some drivers' tanh gives NaN for large |x|).
fn tanh_(x: f32) -> f32 {
    let e = exp(-2.0 * abs(x));
    let t = (1.0 - e) / (1.0 + e);
    return select(-t, t, x >= 0.0);
}

// graph::Act codes.
fn act(x: f32, c: u32) -> f32 {
    switch c {
        case 1u: { return max(x, 0.0); }
        case 2u: { return clamp(x, 0.0, 6.0); }
        case 3u: { return 0.5 * x * (1.0 + erf_(x * 0.70710678118654752)); }
        case 4u: { return 0.5 * x * (1.0 + tanh_(0.7978845608028654 * (x + 0.044715 * x * x * x))); }
        case 5u: { return x * sigm(x); }
        case 6u: { return sigm(x); }
        case 7u: { return tanh_(x); }
        case 8u: { return clamp(x / 6.0 + 0.5, 0.0, 1.0); }
        case 9u: { return x * clamp(x / 6.0 + 0.5, 0.0, 1.0); }
        case 10u: { return sqrt(x); }
        case 11u: { return 1.0 / sqrt(x); }
        default: { return x; }
    }
}
