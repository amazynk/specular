// Strided copy (transpose, slice, one input of a concat).
// P: n, rank, dst base, src base, dims[6] @4, src strides[6] @10, dst strides[6] @16.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let r = pu(1u);
    var rem = i;
    var os = pu(3u);
    var od = pu(2u);
    for (var d = 0u; d < r; d++) {
        let k = r - 1u - d;
        let dim = pu(4u + k);
        let c = rem % dim;
        rem = rem / dim;
        os += c * pu(10u + k);
        od += c * pu(16u + k);
    }
    dst[od] = ld0(os);
}
