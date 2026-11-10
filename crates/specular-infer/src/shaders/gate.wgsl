// NAFNet's simple gate: the first half of each image's channels times the second.
// P: outputs, half (C/2·H·W).
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let half = pu(1u);
    let a = (i / half) * 2u * half + i % half;
    dst[i] = ld0(a) * ld0(a + half);
}
