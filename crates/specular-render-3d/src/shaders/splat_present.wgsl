// Shows the splat view (display-referred, premultiplied, the rectangle's size) in its
// rectangle of the linear HDR scene, over what is there.

struct Present {
    // Target rectangle: x, y, width, height (px).
    rect: vec4<f32>,
};

@group(0) @binding(0) var<uniform> pu: Present;
@group(0) @binding(1) var view: texture_2d<f32>;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_splat_present(in: VOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(in.pos.xy - pu.rect.xy));
    let c = textureLoad(view, p, 0);
    let a = clamp(c.a, 0.0, 1.0);
    if a <= 0.0 {
        return vec4<f32>(0.0);
    }
    let straight = clamp(c.rgb / c.a, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(srgb_to_linear(straight) * a, a);
}
