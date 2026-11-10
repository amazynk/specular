// Sampling a volume in raw slice space, shared by the MPR and volume rendering shaders.
//
// The volume's slices need not be evenly spaced nor stacked straight (gantry tilt): a point
// is given in raw slice space (i, j before the slice's in-plane shift; a = mm along the
// slice normal from slice 0), and a lookup table maps a to the slice coordinate s and the
// shift. While the volume loads, missing slices are bridged by blending the nearest loaded
// ones (the neighbour table), as `specular_dicom::Volume::sample` does on the CPU.
//
// Integer textures and R32Float are not filterable: the eight taps are loaded by hand.
// `VTYPE` is replaced with f32, i32 or u32 for the three texture formats. The including
// shader binds `vt` (the volume) and `lut`: `table.z` entries [s, shift i, shift j, 0], then
// `table.w` entries [loaded slice at or below, at or above, 0, 0] (−1 / slices when none).

fn texel(c: vec3<i32>) -> f32 {
    return f32(textureLoad(vt, c, 0).r);
}

// Bilinear in slice `k` at (i, j), clamped to the edge pixels.
fn slice_value(i: f32, j: f32, k: i32, dims: vec3<f32>) -> f32 {
    let w = dims.x;
    let h = dims.y;
    let x = clamp(i, 0.0, w - 1.0);
    let y = clamp(j, 0.0, h - 1.0);
    let x0 = i32(floor(x));
    let y0 = i32(floor(y));
    let x1 = min(x0 + 1, i32(w) - 1);
    let y1 = min(y0 + 1, i32(h) - 1);
    let fx = x - floor(x);
    let fy = y - floor(y);
    let top = mix(texel(vec3<i32>(x0, y0, k)), texel(vec3<i32>(x1, y0, k)), fx);
    let bot = mix(texel(vec3<i32>(x0, y1, k)), texel(vec3<i32>(x1, y1, k)), fx);
    return mix(top, bot, fy);
}

// Voxel coordinates (i, j, s) of raw point `q`; w = 1 inside the volume, else all 0.
// `table`: a of the first entry, 1 / mm between entries, entries, slices.
fn locate(q: vec3<f32>, dims: vec3<f32>, table: vec4<f32>) -> vec4<f32> {
    let nodes = table.z;
    let f = (q.z - table.x) * table.y;
    if f < 0.0 || f > nodes - 1.0 {
        return vec4<f32>(0.0);
    }
    let n0 = min(u32(floor(f)), u32(nodes) - 2u);
    let t = f - f32(n0);
    let e = mix(lut[n0], lut[n0 + 1u], t);
    let i = q.x - e.y;
    let j = q.y - e.z;
    if i < -0.5 || i > dims.x - 0.5 || j < -0.5 || j > dims.y - 0.5 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(i, j, e.x, 1.0);
}

// The value at voxel coordinates `v` (inside the volume), linear between the nearest loaded
// slices; y = 1 when a slice is loaded (0 before any is).
fn fetch(v: vec3<f32>, dims: vec3<f32>, nodes: f32) -> vec2<f32> {
    let d = dims.z;
    let s = clamp(v.z, 0.0, d - 1.0);
    let k0 = min(u32(floor(s)), u32(d) - 1u);
    let k1 = min(k0 + 1u, u32(d) - 1u);
    let base = u32(nodes);
    var a = lut[base + k0].x;
    var b = lut[base + k1].y;
    if a < 0.0 && b >= d {
        return vec2<f32>(0.0);
    }
    if a < 0.0 {
        a = b;
    }
    if b >= d {
        b = a;
    }
    let pa = slice_value(v.x, v.y, i32(a), dims);
    if b <= a {
        return vec2<f32>(pa, 1.0);
    }
    let pb = slice_value(v.x, v.y, i32(b), dims);
    return vec2<f32>(mix(pa, pb, (s - a) / (b - a)), 1.0);
}

// The value at raw point `q`; x = value, y = 1 inside the volume (0 outside or before any
// slice loaded).
fn sample_raw(q: vec3<f32>, dims: vec3<f32>, table: vec4<f32>) -> vec2<f32> {
    let l = locate(q, dims, table);
    if l.w < 0.5 {
        return vec2<f32>(0.0);
    }
    return fetch(l.xyz, dims, table.z);
}
