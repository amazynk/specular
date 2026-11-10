// Compare's per-pixel error (DESIGN.md §6.3), shared by the difference view (compare.wgsl)
// and the metric passes (compare_reduce.wgsl, compare_ssim.wgsl, compare_de.wgsl).
// specular-inspect's `pixel_error` is the CPU reference: premultiplied linear light taken in
// the metric space (`to_space`), the mean over R, G, B (and alpha when `alpha`), relative to
// the first image `a`.

const REL_EPS: f32 = 0.01;

// (absolute, squared, relative, largest channel difference)
fn pixel_error(a: vec4<f32>, b: vec4<f32>, alpha: bool) -> vec4<f32> {
    let d = abs(a - b);
    let r = d / (abs(a) + REL_EPS);
    var s = vec3<f32>(d.r + d.g + d.b, dot(d.rgb, d.rgb), r.r + r.g + r.b);
    var m = max(d.r, max(d.g, d.b));
    var n = 3.0;
    if alpha {
        s += vec3<f32>(d.a, d.a * d.a, r.a);
        m = max(m, d.a);
        n = 4.0;
    }
    return vec4<f32>(s / n, m);
}

// specular-inspect's `MetricSpace::convert`: `srgb` encodes the colour channels (clipped to
// 0‥1; IEC 61966-2-1), alpha stays.
fn to_space(p: vec4<f32>, srgb: bool) -> vec4<f32> {
    if !srgb {
        return p;
    }
    let c = clamp(p.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let e = select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
    return vec4<f32>(e, p.a);
}

// specular-inspect's `signed_error`: the mean of a − b over the compared channels.
fn signed_error(a: vec4<f32>, b: vec4<f32>, alpha: bool) -> f32 {
    let d = a - b;
    if alpha {
        return (d.r + d.g + d.b + d.a) / 4.0;
    }
    return (d.r + d.g + d.b) / 3.0;
}

// BT.709 luma (relative luminance of linear values; Y' of encoded ones).
fn luma709(p: vec4<f32>) -> f32 {
    return dot(p.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}
