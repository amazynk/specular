// Reduce one axis. P: outputs, len, inner, op (0 sum, 1 mean, 2 max).
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let len = pu(1u);
    let inner = pu(2u);
    let op = pu(3u);
    let base = (i / inner) * len * inner + i % inner;
    var acc = 0.0;
    for (var l = 0u; l < len; l++) {
        let v = ld0(base + l * inner);
        if (op == 2u) {
            acc = select(max(acc, v), v, l == 0u);
        } else {
            acc += v;
        }
    }
    if (op == 1u) {
        acc = acc / f32(len);
    }
    dst[i] = acc;
}
