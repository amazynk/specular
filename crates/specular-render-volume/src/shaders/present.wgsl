// Shows the volume rendering's accumulation (premultiplied, possibly at a lower resolution)
// in its rectangle of the HDR scene, over the canvas colour.

struct Present {
    // Target rectangle: x, y, width, height (px).
    rect: vec4<f32>,
    canvas: vec4<f32>,
};

@group(0) @binding(0) var<uniform> pu: Present;
@group(0) @binding(1) var acc: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

@fragment
fn fs_present(in: VOut) -> @location(0) vec4<f32> {
    let uv = (in.pos.xy - pu.rect.xy) / pu.rect.zw;
    let c = textureSampleLevel(acc, smp, uv, 0.0);
    return vec4<f32>(c.rgb + pu.canvas.rgb * (1.0 - c.a), 1.0);
}
