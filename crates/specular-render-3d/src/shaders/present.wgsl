// Shows the resolved mesh view (premultiplied, the rectangle's size) in its rectangle of the
// HDR scene, over what is there.

struct Present {
    // Target rectangle: x, y, width, height (px).
    rect: vec4<f32>,
};

@group(0) @binding(0) var<uniform> pu: Present;
@group(0) @binding(1) var view: texture_2d<f32>;

@fragment
fn fs_present(in: VOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(in.pos.xy - pu.rect.xy));
    let c = textureLoad(view, p, 0);
    // X-ray adds up past opaque.
    return vec4<f32>(min(c.rgb, vec3<f32>(1.0)), min(c.a, 1.0));
}
