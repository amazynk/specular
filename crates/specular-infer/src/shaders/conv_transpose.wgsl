// Transposed 2-D convolution, one thread per output element, gathering the input taps
// that land on it (input channel, then kernel row, then column: cpu.rs's order).
// P: outputs, C, IH, IW, O, KH, KW, OH, OW, sy, sx, pt, pl, act.
// in0 x [N, C, IH, IW], in1 w [C, O, KH, KW]; `bias(o)` from the defs.
@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let c = pu(1u);
    let ih = pu(2u);
    let iw = pu(3u);
    let o = pu(4u);
    let kh = pu(5u);
    let kw = pu(6u);
    let oh = pu(7u);
    let ow = pu(8u);
    let sy = pu(9u);
    let sx = pu(10u);
    let ox = i % ow;
    let oy = (i / ow) % oh;
    let oc = (i / (ow * oh)) % o;
    let n = i / (ow * oh * o);
    let fy = oy + pu(11u);
    let fx = ox + pu(12u);
    var acc = 0.0;
    for (var ic = 0u; ic < c; ic++) {
        let xb = (n * c + ic) * ih * iw;
        let wb = (ic * o + oc) * kh * kw;
        for (var ky = 0u; ky < kh; ky++) {
            if (fy < ky || (fy - ky) % sy != 0u || (fy - ky) / sy >= ih) {
                continue;
            }
            let iy = (fy - ky) / sy;
            for (var kx = 0u; kx < kw; kx++) {
                if (fx < kx || (fx - kx) % sx != 0u || (fx - kx) / sx >= iw) {
                    continue;
                }
                let ix = (fx - kx) / sx;
                acc += ld0(xb + iy * iw + ix) * ld1(wb + ky * kw + kx);
            }
        }
    }
    dst[i] = act(acc + bias(oc), pu(13u));
}
