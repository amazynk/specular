// Segment overlays (SEG, RTSTRUCT, TMTV): a label volume (R16Uint: segment index + 1, or one
// fractional segment's fraction × 65535) drawn over a plane, filled with alpha and outlined
// where the segment ends. Appended to mpr.wgsl (its uniform block, texture and slice table).
//
// Uniform rows reused: `canvas` = [the fractional segment's index (−1: the layer holds
// indices), fill opacity, outline width (target px), the layer's first colour]. Nearest-voxel
// lookup, so segments keep their voxel edges.

// Per segment: linear rgb, opacity (0: hidden).
@group(0) @binding(3) var<storage, read> colours: array<vec4<f32>>;

// The label value nearest raw slice-space `q`; −1 outside the grid.
fn label(q: vec3<f32>) -> i32 {
    let nodes = u.table.z;
    let f = (q.z - u.table.x) * u.table.y;
    if f < 0.0 || f > nodes - 1.0 {
        return -1;
    }
    let n0 = min(u32(floor(f)), u32(nodes) - 2u);
    let e = mix(lut[n0], lut[n0 + 1u], f - f32(n0));
    let i = floor(q.x - e.y + 0.5);
    let j = floor(q.y - e.z + 0.5);
    let k = floor(e.x + 0.5);
    if i < 0.0 || j < 0.0 || k < 0.0 || i >= u.dims.x || j >= u.dims.y || k >= u.dims.z {
        return -1;
    }
    return i32(textureLoad(vt, vec3<i32>(i32(i), i32(j), i32(k)), 0).r);
}

// The segment at `q`: x = index (−1 none), y = fraction (1 for binary segments).
fn segment(q: vec3<f32>) -> vec2<f32> {
    let v = label(q);
    if v <= 0 {
        return vec2<f32>(-1.0, 0.0);
    }
    if u.canvas.x >= 0.0 {
        if v >= 32768 {
            return vec2<f32>(u.canvas.x, f32(v) / 65535.0);
        }
        return vec2<f32>(-1.0, 0.0);
    }
    return vec2<f32>(f32(v - 1), 1.0);
}

@fragment
fn fs_labels(in: VOut) -> @location(0) vec4<f32> {
    let p = u.base.xyz + u.du.xyz * in.pos.x + u.dv.xyz * in.pos.y;
    let s = segment(p);
    if s.x < 0.0 {
        discard;
    }
    let c = colours[u32(s.x) + u32(u.canvas.w)];
    if c.a <= 0.0 {
        discard;
    }
    let w = u.canvas.z;
    var edge = false;
    if w > 0.0 {
        let a = u.du.xyz * w;
        let b = u.dv.xyz * w;
        edge = segment(p + a).x != s.x || segment(p - a).x != s.x
            || segment(p + b).x != s.x || segment(p - b).x != s.x;
    }
    var alpha = c.a * u.canvas.y * s.y;
    if edge {
        alpha = c.a;
    }
    // Premultiplied.
    return vec4<f32>(c.rgb * alpha, alpha);
}
