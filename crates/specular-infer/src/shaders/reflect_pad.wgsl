// Mirror padding of NCHW planes. P: outputs, H, W, OH, OW, top, left.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let h = i32(pu(1u));
    let w = i32(pu(2u));
    let oh = pu(3u);
    let ow = pu(4u);
    let ox = i % ow;
    let oy = (i / ow) % oh;
    let plane = i / (ow * oh);
    var y = abs(i32(oy) - i32(pu(5u)));
    var x = abs(i32(ox) - i32(pu(6u)));
    if (y >= h) { y = 2 * (h - 1) - y; }
    if (x >= w) { x = 2 * (w - 1) - x; }
    dst[i] = ld0(plane * u32(h * w) + u32(y) * u32(w) + u32(x));
}
