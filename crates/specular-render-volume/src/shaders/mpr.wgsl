// Multiplanar reformatting: one plane (or a slab of planes) through a volume, sampled
// trilinearly from a 3D texture of modality values (shaders/sample.wgsl, prepended) and shown
// through a VOI window. The plane is given in raw slice space.

struct Mpr {
    // Raw slice-space position of output pixel (0, 0) (the target's corner, not a centre).
    base: vec4<f32>,
    // Per output pixel along x and y.
    du: vec4<f32>,
    dv: vec4<f32>,
    // Between slab samples; w = the number of samples (≥ 1). `base` is the first sample's.
    dw: vec4<f32>,
    // Volume width, height, slices; unused.
    dims: vec4<f32>,
    // Slice table: a of the first entry, 1 / mm between entries, entries, slices.
    table: vec4<f32>,
    // As shaders/medical.wgsl (specular-render): centre c', 1 / w', lower and upper edge.
    voi: vec4<f32>,
    // VOI function (0 linear, 1 sigmoid), inverted, slab mode (0 average, 1 MIP, 2 MinIP),
    // unused.
    mode: vec4<f32>,
    // Outside the volume.
    canvas: vec4<f32>,
    // The colour the grey value is drawn in (linear RGB); w = 1 when added over the
    // layers before (a channel of a composite: nothing outside the volume).
    tint: vec4<f32>,
    // The 2D view's stretch after the window (0 none, 1 square root, 2 log, 3 asinh,
    // 4 histogram equalisation through `curve`, 5 the automatic stretch's midtones
    // transfer), the log's a, the asinh's a, the midtones balance.
    tone: vec4<f32>,
    // Histogram equalisation: 256 knots, four a row.
    curve: array<vec4<f32>, 64>,
    // GSDF display (y > 0): JND indices of black and white, black in cd/m²,
    // 1 / (white − black), as shaders/medical.wgsl (specular-render).
    gsdf: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Mpr;
@group(0) @binding(1) var vt: texture_3d<VTYPE>;
// `table.z` entries [s, shift i, shift j, 0], then `table.w` entries [loaded slice at or
// below, at or above, 0, 0] (−1 / slices when none).
@group(0) @binding(2) var<storage, read> lut: array<vec4<f32>>;

fn voi(x: f32) -> f32 {
    if u.mode.x > 0.5 {
        return 1.0 / (1.0 + exp(-4.0 * (x - u.voi.x) * u.voi.y));
    }
    if x <= u.voi.z { return 0.0; }
    if x > u.voi.w || u.voi.y == 0.0 { return 1.0; }
    return clamp((x - u.voi.x) * u.voi.y + 0.5, 0.0, 1.0);
}

fn knot(i: u32) -> f32 {
    return u.curve[i / 4u][i % 4u];
}

// As specular_core::nd::apply (the science view's stretch).
fn stretch(x0: f32) -> f32 {
    let x = clamp(x0, 0.0, 1.0);
    let s = u32(u.tone.x + 0.5);
    if s == 1u {
        return sqrt(x);
    }
    if s == 2u {
        return log(1.0 + u.tone.y * x) / log(1.0 + u.tone.y);
    }
    if s == 3u {
        return asinh(x / u.tone.z) / asinh(1.0 / u.tone.z);
    }
    if s == 4u {
        let f = x * 255.0;
        let i = min(u32(floor(f)), 254u);
        return mix(knot(i), knot(i + 1u), f - f32(i));
    }
    if s == 5u {
        // As specular_core::nd::mtf: 0 and 1 stay, the balance goes to 0.5.
        let m = u.tone.w;
        if x <= 0.0 { return 0.0; }
        if x >= 1.0 { return 1.0; }
        let d = (2.0 * m - 1.0) * x - m;
        if d == 0.0 { return 0.5; }
        return clamp((m - 1.0) * x / d, 0.0, 1.0);
    }
    return x;
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { return c / 12.92; }
    return pow((c + 0.055) / 1.055, 2.4);
}

// P-value `p` (0..1) on a GSDF display (specular_dicom::gsdf::p_luminance): equal JND steps
// from the display's black to its white, as relative luminance over that range.
fn gsdf(p: f32) -> f32 {
    let j = mix(u.gsdf.x, u.gsdf.y, clamp(p, 0.0, 1.0));
    let x = log(clamp(j, 1.0, 1023.0));
    // PS3.14 B.1, equation (1).
    let num = -1.3011877 + x * (8.0242636e-2 + x * (1.3646699e-1 + x * (-2.5468404e-2 + x * 1.3635334e-3)));
    let den = 1.0 + x * (-2.5840191e-2 + x * (-1.0320229e-1 + x * (2.8745620e-2 + x * (-3.1978977e-3 + x * 1.2992634e-4))));
    let l = exp2(num / den * 3.32192809);
    return clamp((l - u.gsdf.z) * u.gsdf.w, 0.0, 1.0);
}

@fragment
fn fs_mpr(in: VOut) -> @location(0) vec4<f32> {
    let p = u.base.xyz + u.du.xyz * in.pos.x + u.dv.xyz * in.pos.y;
    let n = max(u32(u.dw.w), 1u);
    let slab = u.mode.z;
    var acc = 0.0;
    var hits = 0u;
    for (var m = 0u; m < n; m++) {
        let r = sample_raw(p + u.dw.xyz * f32(m), u.dims.xyz, u.table);
        if r.y > 0.5 {
            if hits == 0u {
                acc = r.x;
            } else if slab > 1.5 {
                acc = min(acc, r.x);
            } else if slab > 0.5 {
                acc = max(acc, r.x);
            } else {
                acc += r.x;
            }
            hits++;
        }
    }
    if hits == 0u {
        if u.tint.w > 0.5 {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
        return u.canvas;
    }
    if slab < 0.5 {
        acc /= f32(hits);
    }
    var g = stretch(voi(acc));
    if u.mode.y > 0.5 {
        g = 1.0 - g;
    }
    var l = srgb_to_linear(g);
    if u.gsdf.y > 0.0 {
        l = gsdf(g);
    }
    return vec4<f32>(u.tint.rgb * l, 1.0);
}
