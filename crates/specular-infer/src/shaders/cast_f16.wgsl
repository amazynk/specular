// f32 -> f16, two per output word. P: elements.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    let n = pu(0u);
    if (2u * i >= n) {
        return;
    }
    var hi = 0.0;
    if (2u * i + 1u < n) {
        hi = ld0(2u * i + 1u);
    }
    dst[i] = pack2x16float(vec2<f32>(ld0(2u * i), hi));
}
