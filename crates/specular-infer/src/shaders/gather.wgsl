// Rows of a [V, D] table (in0) at f32 indices (in1), clamped; NaN picks row 0.
// P: outputs, V, D.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let d = pu(2u);
    let x = ld1(i / d);
    let r = select(u32(clamp(x, 0.0, f32(pu(1u) - 1u))), 0u, x != x);
    dst[i] = ld0(r * d + i % d);
}
