// Gamma-correct Lanczos-3 reduction of one mip level into the next (any ratio up to 2).
// Texels hold straight colour: it is averaged weighted by alpha (as premultiplied colour
// would be), or plainly where the result is transparent, so the colour under transparent
// areas is kept at every level.

@group(0) @binding(0) var src: texture_2d<f32>;

const PI: f32 = 3.14159265;

fn lz(x: f32) -> f32 {
    let ax = abs(x);
    if ax < 1e-4 { return 1.0; }
    if ax >= 3.0 { return 0.0; }
    let px = PI * x;
    return 3.0 * sin(px) * sin(px / 3.0) / (px * px);
}

@fragment
fn fs_mip(in: VOut) -> @location(0) vec4<f32> {
    let sd = vec2<f32>(textureDimensions(src));
    // Destination size is the render target's; derive it from the uv interpolation.
    let d = floor(in.pos.xy);
    let dd = round(in.pos.xy / max(in.uv, vec2<f32>(1e-6)));
    let ratio = sd / max(dd, vec2<f32>(1.0));
    let c = (d + 0.5) * ratio;
    let r = 3.0 * ratio;
    let x0 = i32(floor(c.x - r.x + 0.5));
    let x1 = i32(floor(c.x + r.x - 0.5));
    let y0 = i32(floor(c.y - r.y + 0.5));
    let y1 = i32(floor(c.y + r.y - 0.5));
    let maxp = vec2<i32>(sd) - 1;
    var acc = vec4<f32>(0.0);
    var plain = vec3<f32>(0.0);
    var hi = vec3<f32>(0.0);
    var lo = vec3<f32>(65504.0);
    var ws = 0.0;
    for (var y = y0; y <= y1; y++) {
        let wy = lz((f32(y) + 0.5 - c.y) / ratio.y);
        if wy == 0.0 { continue; }
        for (var x = x0; x <= x1; x++) {
            let wx = lz((f32(x) + 0.5 - c.x) / ratio.x);
            let w = wx * wy;
            let t = textureLoad(src, clamp(vec2<i32>(x, y), vec2<i32>(0), maxp), 0);
            acc += vec4<f32>(t.rgb * t.a, t.a) * w;
            plain += t.rgb * w;
            hi = max(hi, t.rgb);
            lo = min(lo, t.rgb);
            ws += w;
        }
    }
    let o = acc / max(ws, 1e-6);
    let a = clamp(o.a, 0.0, 1.0);
    // Below about a quarter of an 8-bit alpha step the division is noise. Where ringing
    // brings alpha near 0 the ratio can still overshoot: no brighter than any tap, and, so
    // that both lobes of an edge's ringing are treated alike, no darker than any tap. Clamping
    // only the bright side moved every edge in a reduced level towards the dark side, by up
    // to a sixth of a source pixel at 2:1.
    let rgb = select(plain / max(ws, 1e-6), o.rgb / o.a, o.a > 1e-3);
    // HDR_STORE (prepended by the renderer): half-float overviews keep values above 1.
    let top = min(hi, vec3<f32>(select(1.0, 65504.0, HDR_STORE)));
    return vec4<f32>(clamp(rgb, max(lo, vec3<f32>(0.0)), top), a);
}
