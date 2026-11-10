struct BlurParams {
    // Half a texel of the source, in uv.
    half_texel: vec2<f32>,
    offset: f32,
    _pad: f32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> bp: BlurParams;

@fragment
fn fs_down(in: VOut) -> @location(0) vec4<f32> {
    let o = bp.half_texel * bp.offset;
    var s = textureSample(src, samp, in.uv) * 4.0;
    s = s + textureSample(src, samp, in.uv - o);
    s = s + textureSample(src, samp, in.uv + o);
    s = s + textureSample(src, samp, in.uv + vec2<f32>(o.x, -o.y));
    s = s + textureSample(src, samp, in.uv - vec2<f32>(o.x, -o.y));
    return s / 8.0;
}

@fragment
fn fs_up(in: VOut) -> @location(0) vec4<f32> {
    let o = bp.half_texel * bp.offset;
    var s = textureSample(src, samp, in.uv + vec2<f32>(-o.x * 2.0, 0.0));
    s = s + textureSample(src, samp, in.uv + vec2<f32>(-o.x, o.y)) * 2.0;
    s = s + textureSample(src, samp, in.uv + vec2<f32>(0.0, o.y * 2.0));
    s = s + textureSample(src, samp, in.uv + vec2<f32>(o.x, o.y)) * 2.0;
    s = s + textureSample(src, samp, in.uv + vec2<f32>(o.x * 2.0, 0.0));
    s = s + textureSample(src, samp, in.uv + vec2<f32>(o.x, -o.y)) * 2.0;
    s = s + textureSample(src, samp, in.uv + vec2<f32>(0.0, -o.y * 2.0));
    s = s + textureSample(src, samp, in.uv + vec2<f32>(-o.x, -o.y)) * 2.0;
    return s / 12.0;
}

