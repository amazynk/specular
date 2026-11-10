// softmax(scale * x) over rows; with `causal`, row r (index r % T in its matrix) keeps its
// first (r % T) + 1 entries and zeroes the rest. P: rows, D, scale, causal, T.
@compute @workgroup_size(RW)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let row = growid(wid, nwg);
    if (row >= pu(0u)) {
        return;
    }
    let d = pu(1u);
    let scale = pf(2u);
    var valid = d;
    if (pu(3u) != 0u) {
        valid = min(row % pu(4u) + 1u, d);
    }
    let base = row * d;
    var m = -3.4028235e38;
    for (var j = li; j < valid; j += RW) {
        m = max(m, ld0(base + j) * scale);
    }
    let mx = wg_max(m, li);
    var s = 0.0;
    for (var j = li; j < valid; j += RW) {
        s += exp(ld0(base + j) * scale - mx);
    }
    let inv = 1.0 / wg_sum(s, li);
    for (var j = li; j < d; j += RW) {
        var y = 0.0;
        if (j < valid) {
            y = exp(ld0(base + j) * scale - mx) * inv;
        }
        dst[base + j] = y;
    }
}
