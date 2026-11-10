// Point clouds: one instance per point, pulled from a page of 5-word records (x, y, z as f32
// bits relative to the cloud's origin; r | g << 8 | b << 16 | class << 24; intensity |
// returns << 16), drawn as a round splat of six vertices. The second target takes each
// splat's log depth for eye-dome lighting (0 where there is no point).

struct Points {
    // x: colour (0 RGB, 1 intensity, 2 elevation, 3 class); y: 1 when the size is in model
    // units; z: the size (logical px, or model units); w: physical px per logical px.
    mode: vec4<f32>,
    // xy: the elevation range (z low, high); zw: the intensity range.
    ranges: vec4<f32>,
    // xy: the view's size (px); z: px per model unit at depth 1 (at any depth in
    // orthographic).
    view: vec4<f32>,
    // x: 1 in orthographic; y: the virtual eye's distance and z: the depth span there
    // (orthographic depth is measured from a virtual eye so log depth still works).
    depth: vec4<f32>,
};

@group(1) @binding(0) var<uniform> pu: Points;
// Row 0: the elevation map (batlow); row 1: the ASPRS class colours. sRGB, read as linear.
@group(1) @binding(1) var lut: texture_2d<f32>;

@group(2) @binding(0) var<storage, read> pts: array<u32>;

struct POut {
    @builtin(position) clip: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) @interpolate(flat) depth: f32,
};

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn point_colour(p: vec3<f32>, a0: u32, a1: u32) -> vec3<f32> {
    let mode = u32(pu.mode.x);
    if mode == 1u {
        let i = f32(a1 & 0xffffu);
        let t = clamp((i - pu.ranges.z) / max(pu.ranges.w - pu.ranges.z, 1.0), 0.0, 1.0);
        return srgb_to_linear(vec3<f32>(t));
    }
    if mode == 2u {
        let t = clamp((p.z - pu.ranges.x) / max(pu.ranges.y - pu.ranges.x, 1e-6), 0.0, 1.0);
        return textureLoad(lut, vec2<i32>(i32(round(t * 255.0)), 0), 0).rgb;
    }
    if mode == 3u {
        return textureLoad(lut, vec2<i32>(i32(a0 >> 24u), 1), 0).rgb;
    }
    return srgb_to_linear(unpack4x8unorm(a0).rgb);
}

@vertex
fn vs_points(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let b = ii * 5u;
    let p = vec3<f32>(bitcast<f32>(pts[b]), bitcast<f32>(pts[b + 1u]), bitcast<f32>(pts[b + 2u]));
    var o: POut;
    var clip = fr.vp * vec4<f32>(p, 1.0);
    if clip.w <= 0.0 || clipped(p) {
        // Behind the eye: outside the depth range.
        o.clip = vec4<f32>(0.0, 0.0, -1.0, 1.0);
        return o;
    }
    let c = corners[vi];
    var r = 0.5 * pu.mode.z * pu.mode.w;
    if pu.mode.y > 0.5 {
        r = 0.5 * pu.mode.z * pu.view.z / clip.w;
    }
    r = clamp(r, 0.5, 64.0);
    clip = vec4<f32>(clip.xy + c * r * 2.0 / pu.view.xy * clip.w, clip.zw);
    o.clip = clip;
    o.corner = c;
    o.color = point_colour(p, pts[b + 3u], pts[b + 4u]);
    if pu.depth.x > 0.5 {
        o.depth = log2(pu.depth.y + (1.0 - clip.z) * pu.depth.z);
    } else {
        o.depth = log2(max(clip.w, 1e-6));
    }
    return o;
}

struct FOut {
    @location(0) color: vec4<f32>,
    @location(1) depth: f32,
};

@fragment
fn fs_points(in: POut) -> FOut {
    if dot(in.corner, in.corner) > 1.0 {
        discard;
    }
    var o: FOut;
    o.color = vec4<f32>(in.color, 1.0);
    // Log depth is kept above 0, which marks "no point".
    o.depth = in.depth + 64.0;
    return o;
}

@fragment
fn fs_grid_points(in: GOut) -> FOut {
    var o: FOut;
    o.color = grid_colour(in);
    o.depth = 0.0;
    return o;
}
