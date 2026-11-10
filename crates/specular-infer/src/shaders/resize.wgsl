// NCHW resize. P: outputs, H, W, OH, OW, mode (0 nearest, 1 bilinear, 2 bilinear without half-pixel centres), sy, sx.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let h = pu(1u);
    let w = pu(2u);
    let oh = pu(3u);
    let ow = pu(4u);
    let ox = i % ow;
    let oy = (i / ow) % oh;
    let base = (i / (ow * oh)) * h * w;
    let sy = pf(6u);
    let sx = pf(7u);
    if (pu(5u) == 0u) {
        let iy = min(u32((f32(oy) + 0.5) * sy), h - 1u);
        let ix = min(u32((f32(ox) + 0.5) * sx), w - 1u);
        dst[i] = ld0(base + iy * w + ix);
        return;
    }
    let half = select(0.0, 0.5, pu(5u) == 1u);
    let cy = clamp((f32(oy) + half) * sy - half, 0.0, f32(h - 1u));
    let cx = clamp((f32(ox) + half) * sx - half, 0.0, f32(w - 1u));
    let y0 = u32(floor(cy));
    let x0 = u32(floor(cx));
    let y1 = min(y0 + 1u, h - 1u);
    let x1 = min(x0 + 1u, w - 1u);
    let ty = cy - f32(y0);
    let tx = cx - f32(x0);
    let top = ld0(base + y0 * w + x0) * (1.0 - tx) + ld0(base + y0 * w + x1) * tx;
    let bot = ld0(base + y1 * w + x0) * (1.0 - tx) + ld0(base + y1 * w + x1) * tx;
    dst[i] = top * (1.0 - ty) + bot * ty;
}
