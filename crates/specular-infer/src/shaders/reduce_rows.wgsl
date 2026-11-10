// Reduce long contiguous rows, one workgroup per output.
// P: outputs, len, inner (1), op (0 sum, 1 mean, 2 max).
@compute @workgroup_size(RW)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let row = growid(wid, nwg);
    if (row >= pu(0u)) {
        return;
    }
    let len = pu(1u);
    let op = pu(3u);
    let base = row * len;
    var s = 0.0;
    var m = ld0(base);
    for (var j = li; j < len; j += RW) {
        let v = ld0(base + j);
        s += v;
        m = max(m, v);
    }
    let total = wg_sum(s, li);
    let most = wg_max(m, li);
    if (li == 0u) {
        var r = total;
        if (op == 1u) {
            r = total / f32(len);
        } else if (op == 2u) {
            r = most;
        }
        dst[row] = r;
    }
}
