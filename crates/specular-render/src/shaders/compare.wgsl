// Compare's difference view (DESIGN.md §6.3): two renders of the same frame (the reference
// `a` and `b`, premultiplied linear light over nothing, the channel view applied) to the
// error between them in false colour, as a signed error on the diverging map, or over the
// dimmed first image. Pixels outside the overlap stay transparent. specular-inspect's
// `diff_colour` is the CPU reference.

struct Diff {
    // overlap in target px (x0, y0, x1, y1)
    rect: vec4<f32>,
    // mode (1 absolute, 2 squared, 3 relative, 4 signed, 5 over the image), gain
    // (1 / full scale), alpha compared, metric space (1: encoded sRGB)
    m: vec4<f32>,
};

@group(0) @binding(0) var ta: texture_2d<f32>;
@group(0) @binding(1) var tb: texture_2d<f32>;
@group(0) @binding(2) var<uniform> dp: Diff;

// specular-inspect's RAMP (sRGB-encoded stops), interpolated, then decoded to linear.
fn ramp(v: f32) -> vec3<f32> {
    var stops = array<vec3<f32>, 5>(
        vec3<f32>(0.0, 0.0, 0.0),
        vec3<f32>(0.10, 0.12, 0.62),
        vec3<f32>(0.75, 0.15, 0.55),
        vec3<f32>(1.0, 0.55, 0.10),
        vec3<f32>(1.0, 1.0, 0.70),
    );
    let x = clamp(v, 0.0, 1.0) * 4.0;
    let i = min(u32(floor(x)), 3u);
    let e = mix(stops[i], stops[i + 1u], x - f32(i));
    return select(pow((e + 0.055) / 1.055, vec3<f32>(2.4)), e / 12.92, e <= vec3<f32>(0.04045));
}

@fragment
fn fs_diff(in: VOut) -> @location(0) vec4<f32> {
    let px = in.pos.xy;
    if px.x < dp.rect.x || px.y < dp.rect.y || px.x >= dp.rect.z || px.y >= dp.rect.w {
        return vec4<f32>(0.0);
    }
    let p = vec2<i32>(floor(px));
    let a = textureLoad(ta, p, 0);
    let srgb = dp.m.w > 0.5;
    let alpha = dp.m.z > 0.5;
    let sa = to_space(a, srgb);
    let sb = to_space(textureLoad(tb, p, 0), srgb);
    let e = pixel_error(sa, sb, alpha);
    let mode = u32(dp.m.x + 0.5);
    if mode == 4u {
        let t = 0.5 + 0.5 * clamp(signed_error(sa, sb, alpha) * dp.m.y, -1.0, 1.0);
        return vec4<f32>(vik(t), 1.0);
    }
    if mode == 5u {
        let v = clamp(e.x * dp.m.y, 0.0, 1.0);
        let base = max(a.rgb, vec3<f32>(0.0)) * ON_IMAGE_DIM * (1.0 - v);
        return vec4<f32>(base + ramp(v) * v, 1.0);
    }
    var v = e.x;
    if mode == 2u { v = e.y; } else if mode == 3u { v = e.z; }
    return vec4<f32>(ramp(v * dp.m.y), 1.0);
}
