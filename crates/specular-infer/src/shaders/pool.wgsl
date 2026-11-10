// 2-D max / average pooling (average over in-bounds taps).
// P: outputs, H, W, OH, OW, kh, kw, sy, sx, pt, pl, kind (0 max, 1 avg).
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
    let is_max = pu(11u) == 0u;
    var acc = 0.0;
    var cnt = 0u;
    for (var ky = 0u; ky < pu(5u); ky++) {
        let iy = i32(oy * pu(7u) + ky) - i32(pu(9u));
        if (iy < 0 || iy >= h) {
            continue;
        }
        for (var kx = 0u; kx < pu(6u); kx++) {
            let ix = i32(ox * pu(8u) + kx) - i32(pu(10u));
            if (ix < 0 || ix >= w) {
                continue;
            }
            let v = ld0((plane * u32(h) + u32(iy)) * u32(w) + u32(ix));
            if (is_max) {
                acc = select(max(acc, v), v, cnt == 0u);
            } else {
                acc += v;
            }
            cnt += 1u;
        }
    }
    if (!is_max) {
        acc = acc / f32(max(cnt, 1u));
    }
    dst[i] = acc;
}
