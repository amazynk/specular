// One RW-thread workgroup per row (RW, and RP the power of two at or above it, are set by
// the host): reductions through workgroup memory.
var<workgroup> red: array<f32, RW>;

fn wg_sum(v: f32, li: u32) -> f32 {
    red[li] = v;
    workgroupBarrier();
    for (var s = RP / 2u; s > 0u; s >>= 1u) {
        if (li < s && li + s < RW) {
            red[li] += red[li + s];
        }
        workgroupBarrier();
    }
    let r = red[0];
    workgroupBarrier();
    return r;
}

fn wg_max(v: f32, li: u32) -> f32 {
    red[li] = v;
    workgroupBarrier();
    for (var s = RP / 2u; s > 0u; s >>= 1u) {
        if (li < s && li + s < RW) {
            red[li] = max(red[li], red[li + s]);
        }
        workgroupBarrier();
    }
    let r = red[0];
    workgroupBarrier();
    return r;
}
