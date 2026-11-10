// x / max(|x|, eps) over rows. P: rows, D, eps.
@compute @workgroup_size(RW)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let row = growid(wid, nwg);
    if (row >= pu(0u)) {
        return;
    }
    let d = pu(1u);
    let base = row * d;
    var s = 0.0;
    for (var j = li; j < d; j += RW) {
        let x = ld0(base + j);
        s += x * x;
    }
    let inv = 1.0 / max(sqrt(wg_sum(s, li)), pf(2u));
    for (var j = li; j < d; j += RW) {
        dst[base + j] = ld0(base + j) * inv;
    }
}
