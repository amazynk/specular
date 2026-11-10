// dst[i] = act(x[i] * mul + add). P: n, act, mul, add.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    dst[i] = act(ld0(i) * pf(2u) + pf(3u), pu(1u));
}
