// Direct convolution, one thread per output (depthwise convolutions).
// P: Og, OH*OW, Cg*KH*KW, groups, C, H, W, KH, KW, OW, sy, sx, pt, pl, dy, dx, Cg, OH, O,
//    -, act @20, outputs @21.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(21u)) {
        return;
    }
    let ow = pu(9u);
    let oh = pu(17u);
    let o_all = pu(18u);
    let ox = i % ow;
    let t = i / ow;
    let oy = t % oh;
    let t2 = t / oh;
    let o = t2 % o_all;
    let img = t2 / o_all;
    let cg = pu(16u);
    let g = o / pu(0u);
    let kh = pu(7u);
    let kw = pu(8u);
    let h = i32(pu(5u));
    let w = i32(pu(6u));
    var acc = 0.0;
    for (var ci = 0u; ci < cg; ci++) {
        let xbase = (img * pu(4u) + g * cg + ci) * pu(5u);
        let wbase = (o * cg + ci) * kh;
        for (var ky = 0u; ky < kh; ky++) {
            let iy = i32(oy * pu(10u) + ky * pu(14u)) - i32(pu(12u));
            if (iy < 0 || iy >= h) {
                continue;
            }
            for (var kx = 0u; kx < kw; kx++) {
                let ix = i32(ox * pu(11u) + kx * pu(15u)) - i32(pu(13u));
                if (ix < 0 || ix >= w) {
                    continue;
                }
                acc += ld0((xbase + u32(iy)) * u32(w) + u32(ix)) * ld1((wbase + ky) * kw + kx);
            }
        }
    }
    dst[i] = act(acc + bias_at(o), pu(20u));
}
