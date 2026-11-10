// Layer norm across the channels of an NCHW map, one thread per pixel.
// P: pixels (N*H*W), C, H*W, eps. in0 x, in1 gamma, in2 beta.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let c = pu(1u);
    let hw = pu(2u);
    let base = (i / hw) * c * hw + i % hw;
    var s = 0.0;
    for (var k = 0u; k < c; k++) {
        s += ld0(base + k * hw);
    }
    let mean = s / f32(c);
    var v = 0.0;
    for (var k = 0u; k < c; k++) {
        let d = ld0(base + k * hw) - mean;
        v += d * d;
    }
    let inv = 1.0 / sqrt(v / f32(c) + pf(3u));
    for (var k = 0u; k < c; k++) {
        dst[base + k * hw] = (ld0(base + k * hw) - mean) * inv * ld1(k) + ld2(k);
    }
}
