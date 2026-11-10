// Compare's SSIM (DESIGN.md §6.3; Wang et al. 2004) of two sample targets, after
// compare_reduce.wgsl (its bindings, `sh` and `reduce_shared`). specular-inspect's
// `quality::ssim_reference` is the CPU reference: luma in the metric space, an 11 × 11
// Gaussian window (σ 1.5), K1 0.01, K2 0.03, L 1, the population covariance, over every
// centre whose window lies inside `size`.
//
// A 16 × 16 workgroup covers 16 × 16 centres: the horizontal pass writes the five weighted
// moments of its 26 rows (16 + 2 × 5) to shared memory, the vertical pass gives each thread its
// centre's SSIM, and the tree reduction (`sh`) sums them with their count into one partial
// (x: sum, y: count); cs_total folds the partials into slot `slot`.

const SR: u32 = 5u;
const HCOUNT: u32 = 416u;
const C1: f32 = 0.0001;
const C2: f32 = 0.0009;

var<workgroup> hx: array<f32, 416>;
var<workgroup> hy: array<f32, 416>;
var<workgroup> hxx: array<f32, 416>;
var<workgroup> hyy: array<f32, 416>;
var<workgroup> hxy: array<f32, 416>;

fn ssim_w(i: u32) -> f32 {
    var k = array<f32, 11>(SSIM_WEIGHTS);
    return k[i];
}

fn luma_at(t: texture_2d<f32>, p: vec2<u32>) -> f32 {
    return luma709(to_space(textureLoad(t, vec2<i32>(p), 0), rp.space != 0u));
}

@compute @workgroup_size(16, 16)
fn cs_ssim(
    @builtin(local_invocation_id) l: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(num_workgroups) nw: vec3<u32>,
) {
    // The window of centre (x, y) starts at (x − 5, y − 5); this group's centres start at
    // base + 5, so its windows at base.
    let base = wg.xy * 16u;
    for (var i = li; i < HCOUNT; i += 256u) {
        let r = i / 16u;
        let c = i % 16u;
        var m = array<f32, 5>(0.0, 0.0, 0.0, 0.0, 0.0);
        let y = base.y + r;
        if y < rp.size.y {
            for (var k = 0u; k < 11u; k++) {
                let x = base.x + c + k;
                if x < rp.size.x {
                    let w = ssim_w(k);
                    let p = luma_at(sa, vec2<u32>(x, y));
                    let q = luma_at(sb, vec2<u32>(x, y));
                    m[0] += w * p;
                    m[1] += w * q;
                    m[2] += w * p * p;
                    m[3] += w * q * q;
                    m[4] += w * p * q;
                }
            }
        }
        hx[i] = m[0];
        hy[i] = m[1];
        hxx[i] = m[2];
        hyy[i] = m[3];
        hxy[i] = m[4];
    }
    workgroupBarrier();
    var mx = 0.0;
    var my = 0.0;
    var mxx = 0.0;
    var myy = 0.0;
    var mxy = 0.0;
    for (var k = 0u; k < 11u; k++) {
        let i = (l.y + k) * 16u + l.x;
        let w = ssim_w(k);
        mx += w * hx[i];
        my += w * hy[i];
        mxx += w * hxx[i];
        myy += w * hyy[i];
        mxy += w * hxy[i];
    }
    let c = base + l.xy + vec2<u32>(SR);
    var v = vec4<f32>(0.0);
    if c.x + SR < rp.size.x && c.y + SR < rp.size.y {
        let vx = mxx - mx * mx;
        let vy = myy - my * my;
        let cxy = mxy - mx * my;
        let s = ((2.0 * mx * my + C1) * (2.0 * cxy + C2))
            / ((mx * mx + my * my + C1) * (vx + vy + C2));
        v = vec4<f32>(s, 1.0, 0.0, 0.0);
    }
    sh[li] = v;
    reduce_shared(li);
    if li == 0u {
        partials[wg.y * nw.x + wg.x] = sh[0];
    }
}
