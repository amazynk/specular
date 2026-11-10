// Broadcast binary op. P: n, op, rank, -, dims[6] @4, a strides[6] @10, b strides[6] @16.
fn bop(a: f32, b: f32, op: u32) -> f32 {
    switch op {
        case 0u: { return a + b; }
        case 1u: { return a - b; }
        case 2u: { return a * b; }
        case 3u: { return a / b; }
        case 4u: { return max(a, b); }
        case 5u: { return min(a, b); }
        case 6u: { return select(a * b, a, a >= 0.0); }
        default: { return (a - b) * (a - b); }
    }
}

@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let r = pu(2u);
    var rem = i;
    var oa = 0u;
    var ob = 0u;
    for (var d = 0u; d < r; d++) {
        let k = r - 1u - d;
        let dim = pu(4u + k);
        let c = rem % dim;
        rem = rem / dim;
        oa += c * pu(10u + k);
        ob += c * pu(16u + k);
    }
    dst[i] = bop(ld0(oa), ld1(ob), pu(1u));
}
