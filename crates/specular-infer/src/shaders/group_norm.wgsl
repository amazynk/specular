// Group norm, one workgroup per (image, group): the group's channels are one contiguous
// run of D = channels-per-group * H * W values. P: rows, D, eps, H*W, channels per group,
// groups. in0 x, in1 gamma [C], in2 beta [C].
@compute @workgroup_size(RW)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let row = growid(wid, nwg);
    if (row >= pu(0u)) {
        return;
    }
    let d = pu(1u);
    let hw = pu(3u);
    let c0 = (row % pu(5u)) * pu(4u);
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
        let ch = c0 + j / hw;
        dst[base + j] = (ld0(base + j) - mean) * inv * ld1(ch) + ld2(ch);
    }
}
