// Inspection passes (DESIGN.md §6.2), appended to image.wgsl so they read the image exactly
// as it is drawn (same tile lookup and overview fallback):
//   fs_overlay  clipping (highlights / shadows) and focus peaking, blended over the scene;
//   fs_sample   the visible image, straight linear RGB, into the small histogram target
//               (alpha −1 outside the image);
//   fs_probe    the texel under the pointer and the level it came from (2×1 target).

struct Insp {
    // clipping on, peaking on, highlight code (0‥255, inclusive), shadow code (inclusive)
    a: vec4<f32>,
    // peaking colour (linear, premultiplied)
    peak: vec4<f32>,
    // highlight and shadow colours (linear, premultiplied)
    hi: vec4<f32>,
    lo: vec4<f32>,
    // peaking threshold (Sobel magnitude), peaking opacity, probe point (stored px)
    b: vec4<f32>,
};

@group(1) @binding(0) var<uniform> o: Insp;

fn enc3(c: vec3<f32>) -> vec3<f32> {
    let v = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(v, vec3<f32>(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3<f32>(0.0031308));
}

fn straight(c: vec4<f32>) -> vec3<f32> {
    if c.a <= 0.0 { return vec3<f32>(0.0); }
    return c.rgb / c.a;
}

fn stored_of(px: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(dot(u.m0.xy, px) + u.m0.z, dot(u.m1.xy, px) + u.m1.z);
}

fn inside(p: vec2<f32>) -> bool {
    return p.x >= 0.0 && p.y >= 0.0 && p.x < u.img.x && p.y < u.img.y;
}

// sRGB-encoded luma (Rec. 709 weights on encoded values: perceptual, cheap) at `p`.
fn luma_at(p: vec2<f32>, s: f32) -> f32 {
    let q = clamp(p, vec2<f32>(0.0), u.img.xy - 0.001);
    let c = sample_image(q, s, false);
    return dot(enc3(straight(c)) * c.a, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_overlay(in: VOut) -> @location(0) vec4<f32> {
    let px = in.pos.xy;
    if px.x < u.clip.x || px.y < u.clip.y || px.x >= u.clip.z || px.y >= u.clip.w {
        discard;
    }
    let p = stored_of(px);
    if !inside(p) { discard; }
    let s = u.img.z;
    var out = vec4<f32>(0.0);
    if o.a.y > 0.5 {
        // Sobel on luma, one stored pixel apart when magnified (the source's own sharpness),
        // one screen pixel apart when reduced (what is visible). Bilinear, never nearest.
        let d = max(1.0 / s, 1.0);
        let sl = min(s, 1.0) * 0.999;
        let mx = vec2<f32>(u.m0.x, u.m1.x) * d * s;
        let my = vec2<f32>(u.m0.y, u.m1.y) * d * s;
        // The centre tap has zero weight in both kernels: eight lookups, not nine.
        var l: array<f32, 9>;
        for (var j = 0; j < 3; j++) {
            for (var i = 0; i < 3; i++) {
                if i == 1 && j == 1 { continue; }
                l[j * 3 + i] = luma_at(p + mx * f32(i - 1) + my * f32(j - 1), sl);
            }
        }
        let gx = (l[2] + 2.0 * l[5] + l[8]) - (l[0] + 2.0 * l[3] + l[6]);
        let gy = (l[6] + 2.0 * l[7] + l[8]) - (l[0] + 2.0 * l[1] + l[2]);
        let g = length(vec2<f32>(gx, gy));
        let t = o.b.x;
        out = o.peak * (smoothstep(t, t * 1.3, g) * o.b.y);
    }
    if o.a.x > 0.5 {
        let c = sample_image(p, s, false);
        if c.a > 0.0 {
            let code = floor(enc3(straight(c)) * 255.0 + 0.5);
            if max(code.r, max(code.g, code.b)) >= o.a.z {
                out = o.hi;
            } else if min(code.r, min(code.g, code.b)) <= o.a.w {
                out = o.lo;
            }
        }
    }
    if out.a <= 0.0 { discard; }
    return out;
}

@fragment
fn fs_sample(in: VOut) -> @location(0) vec4<f32> {
    let p = stored_of(in.pos.xy);
    if !inside(p) { return vec4<f32>(0.0, 0.0, 0.0, -1.0); }
    // The pass runs with the nearest-neighbour threshold at 0: every target pixel is one
    // texel of the finest resident level that fits, not a blend of several.
    let c = sample_image(p, u.img.z, false);
    return vec4<f32>(straight(c), c.a);
}

// The texel at stored pixel `p` (premultiplied) from the finest resident data, and its level (0 = full
// resolution, larger = reduced; −1 when nothing is resident).
var<private> probe_level: f32 = -1.0;

fn probe(p: vec2<f32>) -> vec4<f32> {
    let ov_scale = select(0.0, u.ov.x / u.img.x, u.ov.w > 0.0);
    let packed = u32(u.misc.w);
    let present = packed & 0x1FFFFu;
    for (var l = 0u; l <= packed >> 17u; l++) {
        let lscale = 1.0 / f32(1u << l);
        if lscale <= ov_scale * 1.001 { break; }
        if (present & (1u << l)) == 0u { continue; }
        let lp = p * lscale;
        let t = vec2<i32>(floor(lp / TILE));
        let slot = lookup(l, t);
        if slot == 0u { continue; }
        let local = lp - vec2<f32>(t) * TILE + BORDER;
        probe_level = f32(l);
        return tap(ld_tile(vec2<i32>(floor(local)), i32(slot - 1u)), 1.0);
    }
    if u.ov.w == 0.0 { return vec4<f32>(0.0); }
    probe_level = select(1.0, 0.0, u.ov.x >= u.img.x);
    let ovp = p * vec2<f32>(u.ov.x / u.img.x, u.ov.y / u.img.y);
    return tap(ld_ov(clamp(vec2<i32>(floor(ovp)), vec2<i32>(0), vec2<i32>(u.ov.xy) - 1), 0), 1.0);
}

@fragment
fn fs_probe(in: VOut) -> @location(0) vec4<f32> {
    let p = clamp(o.b.zw + 0.5, vec2<f32>(0.0), u.img.xy - 0.001);
    let c = to_working(probe(p));
    if in.pos.x < 1.0 { return c; }
    return vec4<f32>(probe_level, 0.0, 0.0, 0.0);
}
