// Compare's metrics (DESIGN.md §6.3): the per-pixel error (compare_error.wgsl) of two sample
// targets in the metric space summed over their top-left `size`. WGSL has no float atomics, so it is two passes:
// cs_partial tree-reduces each 16 × 16 block in shared memory to one partial (sums of the
// absolute, squared and relative error, and the largest difference); cs_total, one workgroup,
// folds the partials into slot `slot` of the totals.

struct R {
    size: vec2<u32>,
    // alpha compared (0 / 1), number of partials
    alpha: u32,
    groups: u32,
    // the totals slot this pass folds into (pair × 3 + metric: error, SSIM, ΔE)
    slot: u32,
    // metric space (1: encoded sRGB)
    space: u32,
    _p1: u32,
    _p2: u32,
};

@group(0) @binding(0) var sa: texture_2d<f32>;
@group(0) @binding(1) var sb: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> partials: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> totals: array<vec4<f32>>;
@group(0) @binding(4) var<uniform> rp: R;
// ΔE histograms, 256 bins per pair (compare_de.wgsl)
@group(0) @binding(5) var<storage, read_write> hist: array<atomic<u32>>;

var<workgroup> sh: array<vec4<f32>, 256>;

fn fold(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(a.xyz + b.xyz, max(a.w, b.w));
}

fn reduce_shared(li: u32) {
    for (var s = 128u; s > 0u; s >>= 1u) {
        workgroupBarrier();
        if li < s {
            sh[li] = fold(sh[li], sh[li + s]);
        }
    }
    workgroupBarrier();
}

@compute @workgroup_size(16, 16)
fn cs_partial(
    @builtin(global_invocation_id) g: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(num_workgroups) nw: vec3<u32>,
) {
    var v = vec4<f32>(0.0);
    if g.x < rp.size.x && g.y < rp.size.y {
        let p = vec2<i32>(g.xy);
        let srgb = rp.space != 0u;
        v = pixel_error(
            to_space(textureLoad(sa, p, 0), srgb),
            to_space(textureLoad(sb, p, 0), srgb),
            rp.alpha != 0u,
        );
    }
    sh[li] = v;
    reduce_shared(li);
    if li == 0u {
        partials[wg.y * nw.x + wg.x] = sh[0];
    }
}

@compute @workgroup_size(256)
fn cs_total(@builtin(local_invocation_index) li: u32) {
    var v = vec4<f32>(0.0);
    for (var i = li; i < rp.groups; i += 256u) {
        v = fold(v, partials[i]);
    }
    sh[li] = v;
    reduce_shared(li);
    if li == 0u {
        totals[rp.slot] = sh[0];
    }
}
