// Layer norm over rows. P: rows, D, eps. in0 x, in1 gamma, in2 beta.
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
        s += ld0(base + j);
    }
    let mean = wg_sum(s, li) / f32(d);
    var v = 0.0;
    for (var j = li; j < d; j += RW) {
        let c = ld0(base + j) - mean;
        v += c * c;
    }
    let inv = 1.0 / sqrt(wg_sum(v, li) / f32(d) + pf(2u));
    for (var j = li; j < d; j += RW) {
        dst[base + j] = (ld0(base + j) - mean) * inv * ld1(j) + ld2(j);
    }
}
