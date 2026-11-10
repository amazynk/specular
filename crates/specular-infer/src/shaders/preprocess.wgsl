// RGBA8 image -> [1, 3, H, W]: triangle-filtered resample of a source rectangle, then
// (v - mean) / std per channel. One thread per output pixel. The Rust side supplies
// px(x, y) -> vec4<f32> in [0, 1] (from a buffer of packed RGBA8 words or a texture).
// P: W, H, sw, sh, x0, y0, sx, sy, fx, fy, mean[3] @10, std[3] @13.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    let ow = pu(0u);
    let oh = pu(1u);
    if (i >= ow * oh) {
        return;
    }
    let ox = i % ow;
    let oy = i / ow;
    let sw = pu(2u);
    let sh = pu(3u);
    let fx = pf(8u);
    let fy = pf(9u);
    let cx = pf(4u) + (f32(ox) + 0.5) * pf(6u);
    let cy = pf(5u) + (f32(oy) + 0.5) * pf(7u);
    let xlo = u32(max(floor(cx - fx - 0.5), 0.0));
    let xhi = min(u32(max(ceil(cx + fx - 0.5), 0.0)), sw - 1u);
    let ylo = u32(max(floor(cy - fy - 0.5), 0.0));
    let yhi = min(u32(max(ceil(cy + fy - 0.5), 0.0)), sh - 1u);
    var acc = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var y = ylo; y <= yhi; y++) {
        let wy = max(1.0 - abs(f32(y) + 0.5 - cy) / fy, 0.0);
        for (var x = xlo; x <= xhi; x++) {
            let wx = max(1.0 - abs(f32(x) + 0.5 - cx) / fx, 0.0);
            let wgt = wx * wy;
            acc += wgt * px(x, y).rgb;
            wsum += wgt;
        }
    }
    let plane = ow * oh;
    for (var c = 0u; c < 3u; c++) {
        dst[c * plane + i] = (acc[c] / wsum - pf(10u + c)) / pf(13u + c);
    }
}
