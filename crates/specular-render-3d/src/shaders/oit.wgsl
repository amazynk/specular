// Composites the translucent layers (weighted blended OIT; see molecule.wgsl) over the
// resolved view: the weighted average colour of the layers, covering 1 - revealage.

@group(0) @binding(0) var accum: texture_2d<f32>;
@group(0) @binding(1) var reveal: texture_2d<f32>;

@fragment
fn fs_oit(in: VOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(in.pos.xy));
    let r = textureLoad(reveal, p, 0).r;
    let cover = clamp(1.0 - r, 0.0, 1.0);
    if cover <= 1e-4 {
        discard;
    }
    let acc = textureLoad(accum, p, 0);
    let avg = acc.rgb / max(acc.a, 1e-5);
    return vec4<f32>(avg * cover, cover);
}
