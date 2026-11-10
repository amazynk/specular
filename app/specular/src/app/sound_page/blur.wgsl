// The sound page's backdrop: the cover blurred, in two passes (across, then down) of a
// 13-tap Gaussian two texels apart, into a small target. The first pass samples the cover
// (sRGB, read linear) at the target's texel steps, which also shrinks it.

struct U {
    // the step between taps (uv), unused
    step: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> VOut {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var o: VOut;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

@fragment
fn fs_blur(i: VOut) -> @location(0) vec4<f32> {
    // Weights of a Gaussian (sigma 3 taps) at 0..6 taps.
    var w = array<f32, 7>(0.1324, 0.1253, 0.1063, 0.0807, 0.0549, 0.0334, 0.0183);
    var acc = textureSampleLevel(src, smp, i.uv, 0.0) * w[0];
    var total = w[0];
    for (var k = 1; k < 7; k = k + 1) {
        let d = u.step.xy * f32(k);
        acc = acc + textureSampleLevel(src, smp, i.uv + d, 0.0) * w[k];
        acc = acc + textureSampleLevel(src, smp, i.uv - d, 0.0) * w[k];
        total = total + 2.0 * w[k];
    }
    let c = acc / total;
    return vec4<f32>(c.rgb, 1.0);
}
