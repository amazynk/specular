struct FinalParams {
    // 1 / (2^bits - 1): one code value of the swapchain.
    lsb: f32,
    // 1: the swapchain is scRGB (extended linear sRGB): write linear light, no encoding.
    // 2: the swapchain is BT.2100 PQ (10-bit): Rec. 2020, SDR white at 203 cd/m².
    linear_out: f32,
    // Largest value written in scRGB mode (the display's headroom).
    peak: f32,
    // Display transform (specular_color::Display; SDR output only): 0 none (sRGB display),
    // 1 matrix + inverse tone curves (`dlut` is N×1×1), 2 3D LUT indexed by sRGB-encoded
    // BT.709.
    cm_mode: f32,
    // Linear BT.709 → linear display RGB rows (mode 1); m0.w = samples / grid points of dlut.
    m0: vec4<f32>,
    m1: vec4<f32>,
    m2: vec4<f32>,
    // Colour-vision-deficiency simulation (specular_inspect::Cvd): linear BT.709 rows applied
    // to the whole frame before encoding; cvd0.w > 0.5 turns it on. Display only: exports
    // never pass through here.
    cvd0: vec4<f32>,
    cvd1: vec4<f32>,
    cvd2: vec4<f32>,
};

// Fraction of an output code around each code left undithered (see fs_final).
const DEAD_ZONE: f32 = 0.125;

@group(0) @binding(0) var comp: texture_2d<f32>;
@group(0) @binding(1) var noise: texture_2d<f32>;
@group(0) @binding(2) var<uniform> fp: FinalParams;
@group(0) @binding(3) var dlut: texture_3d<f32>;
@group(0) @binding(4) var dsamp: sampler;

// Linear BT.709 (1.0 = SDR white) → BT.2100 PQ signal.
fn to_pq(c: vec3<f32>) -> vec3<f32> {
    let r = vec3<f32>(
        dot(vec3<f32>(0.627404, 0.329283, 0.043313), c),
        dot(vec3<f32>(0.069097, 0.919541, 0.011362), c),
        dot(vec3<f32>(0.016391, 0.088013, 0.895595), c));
    let y = clamp(r * (203.0 / 10000.0), vec3<f32>(0.0), vec3<f32>(1.0));
    let p = pow(y, vec3<f32>(0.1593017578125));
    return pow((0.8359375 + 18.8515625 * p) / (1.0 + 18.6875 * p), vec3<f32>(78.84375));
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// Linear BT.709 → display device values (0..1), mirroring specular_color::Display::apply.
fn to_display(c: vec3<f32>) -> vec3<f32> {
    let n = fp.m0.w;
    let k = (n - 1.0) / n;
    let h = 0.5 / n;
    if fp.cm_mode < 1.5 {
        let d = vec3<f32>(dot(fp.m0.xyz, c), dot(fp.m1.xyz, c), dot(fp.m2.xyz, c));
        let e = to_srgb(clamp(d, vec3<f32>(0.0), vec3<f32>(1.0))) * k + h;
        return vec3<f32>(
            textureSampleLevel(dlut, dsamp, vec3<f32>(e.r, 0.5, 0.5), 0.0).r,
            textureSampleLevel(dlut, dsamp, vec3<f32>(e.g, 0.5, 0.5), 0.0).g,
            textureSampleLevel(dlut, dsamp, vec3<f32>(e.b, 0.5, 0.5), 0.0).b);
    }
    let e = to_srgb(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
    return textureSampleLevel(dlut, dsamp, e * k + h, 0.0).rgb;
}

@fragment
fn fs_final(in: VOut) -> @location(0) vec4<f32> {
    let ip = vec2<i32>(in.pos.xy);
    var c = textureLoad(comp, ip, 0);
    if fp.cvd0.w > 0.5 {
        c = vec4<f32>(dot(fp.cvd0.xyz, c.rgb), dot(fp.cvd1.xyz, c.rgb), dot(fp.cvd2.xyz, c.rgb), c.a);
    }
    if fp.linear_out > 1.5 {
        return vec4<f32>(to_pq(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(fp.peak))), 1.0);
    }
    if fp.linear_out > 0.5 {
        // Half floats need no dither; negative values would leave the BT.709 gamut.
        return vec4<f32>(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(fp.peak)), 1.0);
    }
    var e: vec3<f32>;
    if fp.cm_mode > 0.5 {
        e = to_display(c.rgb);
    } else {
        e = to_srgb(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0)));
    }
    // Triangular-ish blue noise: one LSB peak to peak, different phase per channel.
    let n0 = textureLoad(noise, ip & vec2<i32>(63), 0).r;
    let n1 = textureLoad(noise, (ip + vec2<i32>(17, 41)) & vec2<i32>(63), 0).r;
    let n2 = textureLoad(noise, (ip + vec2<i32>(43, 23)) & vec2<i32>(63), 0).r;
    var d = (vec3<f32>(n0, n1, n2) - 0.5) * fp.lsb;
    // No dither within DEAD_ZONE of an output code: such a value is that code up to the
    // half-float composite's rounding (at most 0.06 of a code), so an 8-bit image at 1:1
    // reaches an 8-bit display bit-exact. Dithering it would flip pixels to the next code.
    let q = e / fp.lsb;
    d = select(d, vec3<f32>(0.0), abs(q - round(q)) < vec3<f32>(DEAD_ZONE));
    return vec4<f32>(e + d, 1.0);
}
