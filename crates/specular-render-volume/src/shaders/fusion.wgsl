// Fusion: a second series (PET, SPECT) over the first, through its VOI window and a colour
// map, blended. Appended to mpr.wgsl (its uniforms, sampling and window). Values at or below
// the window's lower edge stay transparent, so the anatomy shows through.
//
// Uniform rows reused: `canvas` = [blend (the overlay's opacity), unused ×3].

// 256 entries: linear rgb, 1.
@group(0) @binding(3) var<storage, read> colours: array<vec4<f32>>;

@fragment
fn fs_fusion(in: VOut) -> @location(0) vec4<f32> {
    let p = u.base.xyz + u.du.xyz * in.pos.x + u.dv.xyz * in.pos.y;
    let n = max(u32(u.dw.w), 1u);
    let slab = u.mode.z;
    var acc = 0.0;
    var hits = 0u;
    for (var m = 0u; m < n; m++) {
        let r = sample_raw(p + u.dw.xyz * f32(m), u.dims.xyz, u.table);
        if r.y > 0.5 {
            if hits == 0u {
                acc = r.x;
            } else if slab > 1.5 {
                acc = min(acc, r.x);
            } else if slab > 0.5 {
                acc = max(acc, r.x);
            } else {
                acc += r.x;
            }
            hits++;
        }
    }
    if hits == 0u {
        discard;
    }
    if slab < 0.5 {
        acc /= f32(hits);
    }
    var g = voi(acc);
    if g <= 0.0 {
        discard;
    }
    if u.mode.y > 0.5 {
        g = 1.0 - g;
    }
    let c = colours[min(u32(g * 255.0 + 0.5), 255u)];
    let a = u.canvas.x;
    return vec4<f32>(c.rgb * a, a);
}
