// Isodose lines: a dose volume (f32, Gy) drawn over a plane as the outlines of the regions at
// or above each level, optionally washed with the band's colour. Appended to mpr.wgsl (its
// uniforms and trilinear sampling); outside the grid the dose is 0.
//
// Uniform rows reused: `canvas` = [levels, line width (target px), wash opacity, unused].

// Per level, lowest dose first: linear rgb, the level's dose.
@group(0) @binding(3) var<storage, read> levels: array<vec4<f32>>;

// How many levels the dose at raw slice-space `q` reaches.
fn band(q: vec3<f32>) -> u32 {
    let r = sample_raw(q, u.dims.xyz, u.table);
    var b = 0u;
    if r.y > 0.5 {
        let n = u32(u.canvas.x);
        for (var i = 0u; i < n; i++) {
            if r.x >= levels[i].w {
                b = i + 1u;
            }
        }
    }
    return b;
}

@fragment
fn fs_isodose(in: VOut) -> @location(0) vec4<f32> {
    let p = u.base.xyz + u.du.xyz * in.pos.x + u.dv.xyz * in.pos.y;
    let b = band(p);
    if b == 0u {
        discard;
    }
    let w = u.canvas.y;
    var edge = false;
    if w > 0.0 {
        let a = u.du.xyz * w;
        let c = u.dv.xyz * w;
        edge = band(p + a) < b || band(p - a) < b || band(p + c) < b || band(p - c) < b;
    }
    var alpha = u.canvas.z;
    if edge {
        alpha = 1.0;
    }
    if alpha <= 0.0 {
        discard;
    }
    let col = levels[b - 1u].rgb;
    return vec4<f32>(col * alpha, alpha);
}
