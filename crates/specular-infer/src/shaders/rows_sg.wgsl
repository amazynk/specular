// One RW-thread workgroup per row: reductions within each subgroup, then across the
// subgroups' partial results in workgroup memory (as rows.wgsl, with two barriers rather
// than nine). Nothing is assumed about which invocations share a subgroup or how large
// subgroups are: each subgroup's partial goes to the slot of its `subgroup_id`, and the
// `num_subgroups` slots are combined in id order, so results don't depend on the layout.
// SGS (set by the host from the device's smallest subgroup size) bounds `num_subgroups`.
// `sg_id`, `sg_count` and `sg_lane` are set by the entry point.
var<workgroup> red: array<f32, RW>;
var<private> sg_id: u32;
var<private> sg_count: u32;
var<private> sg_lane: u32;

fn wg_sum(v: f32, li: u32) -> f32 {
    let s = subgroupAdd(v);
    if (sg_lane == 0u) {
        red[sg_id] = s;
    }
    workgroupBarrier();
    var r = red[0];
    for (var i = 1u; i < SGS; i++) {
        if (i < sg_count) {
            r += red[i];
        }
    }
    workgroupBarrier();
    return r;
}

fn wg_max(v: f32, li: u32) -> f32 {
    let s = subgroupMax(v);
    if (sg_lane == 0u) {
        red[sg_id] = s;
    }
    workgroupBarrier();
    var r = red[0];
    for (var i = 1u; i < SGS; i++) {
        if (i < sg_count) {
            r = max(r, red[i]);
        }
    }
    workgroupBarrier();
    return r;
}
