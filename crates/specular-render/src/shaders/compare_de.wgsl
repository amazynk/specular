// Compare's CIEDE2000 (DESIGN.md §6.3) of two sample targets, after compare_reduce.wgsl (its
// bindings, `sh` and `reduce_shared`). specular-inspect's `quality` is the CPU reference: each
// pixel clipped to 0‥1, premultiplied (over black), BT.709 → XYZ → CIELAB with the D65 white,
// ΔE2000 as in Sharma, Wu and Dalal (2005). Each workgroup bins its 256 values into a shared
// histogram first (`DE_BINS` bins spaced by the square root up to `DE_RANGE`), then adds it
// to pair `slot / 3`'s histogram in `hist`; its sum and largest value go to a partial
// (x: sum, w: max).

const DE_BINS: u32 = 256u;
const DE_RANGE: f32 = 128.0;

var<workgroup> wh: array<atomic<u32>, 256>;

fn lab_d65(c: vec3<f32>) -> vec3<f32> {
    let r = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    let xyz = vec3<f32>(
        dot(vec3<f32>(0.4124564, 0.3575761, 0.1804375), r),
        dot(vec3<f32>(0.2126729, 0.7151522, 0.0721750), r),
        dot(vec3<f32>(0.0193339, 0.1191920, 0.9503041), r),
    ) / vec3<f32>(0.95047, 1.0, 1.08883);
    let e = 216.0 / 24389.0;
    let lin = (24389.0 / 27.0 * xyz + 16.0) / 116.0;
    let cube = pow(max(xyz, vec3<f32>(1e-12)), vec3<f32>(1.0 / 3.0));
    let f = select(lin, cube, xyz > vec3<f32>(e));
    return vec3<f32>(116.0 * f.y - 16.0, 500.0 * (f.x - f.y), 200.0 * (f.y - f.z));
}

fn hue_deg(b: f32, a: f32) -> f32 {
    if b == 0.0 && a == 0.0 {
        return 0.0;
    }
    let h = degrees(atan2(b, a));
    return select(h, h + 360.0, h < 0.0);
}

fn pow7(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x2 * x;
}

// specular-color's `delta_e2000`, in f32.
fn delta_e2000(p: vec3<f32>, q: vec3<f32>) -> f32 {
    let c1 = length(p.yz);
    let c2 = length(q.yz);
    let cm = (c1 + c2) * 0.5;
    let g = 0.5 * (1.0 - sqrt(pow7(cm) / (pow7(cm) + 6103515625.0)));
    let a1p = p.y * (1.0 + g);
    let a2p = q.y * (1.0 + g);
    let c1p = sqrt(a1p * a1p + p.z * p.z);
    let c2p = sqrt(a2p * a2p + q.z * q.z);
    let h1p = hue_deg(p.z, a1p);
    let h2p = hue_deg(q.z, a2p);
    let dl = q.x - p.x;
    let dc = c2p - c1p;
    var dhp = 0.0;
    if c1p * c2p != 0.0 {
        dhp = h2p - h1p;
        if dhp > 180.0 {
            dhp -= 360.0;
        } else if dhp < -180.0 {
            dhp += 360.0;
        }
    }
    let dh = 2.0 * sqrt(c1p * c2p) * sin(radians(dhp) * 0.5);
    let lm = (p.x + q.x) * 0.5;
    let cmp = (c1p + c2p) * 0.5;
    var hm = h1p + h2p;
    if c1p * c2p != 0.0 {
        if abs(h1p - h2p) <= 180.0 {
            hm = (h1p + h2p) * 0.5;
        } else if h1p + h2p < 360.0 {
            hm = (h1p + h2p + 360.0) * 0.5;
        } else {
            hm = (h1p + h2p - 360.0) * 0.5;
        }
    }
    let t = 1.0 - 0.17 * cos(radians(hm - 30.0)) + 0.24 * cos(radians(2.0 * hm))
        + 0.32 * cos(radians(3.0 * hm + 6.0)) - 0.20 * cos(radians(4.0 * hm - 63.0));
    let x = (hm - 275.0) / 25.0;
    let dtheta = 30.0 * exp(-x * x);
    let rc = 2.0 * sqrt(pow7(cmp) / (pow7(cmp) + 6103515625.0));
    let l50 = (lm - 50.0) * (lm - 50.0);
    let sl = 1.0 + 0.015 * l50 / sqrt(20.0 + l50);
    let sc = 1.0 + 0.045 * cmp;
    let shh = 1.0 + 0.015 * cmp * t;
    let rt = -sin(radians(2.0 * dtheta)) * rc;
    let v = (dl / sl) * (dl / sl) + (dc / sc) * (dc / sc) + (dh / shh) * (dh / shh)
        + rt * (dc / sc) * (dh / shh);
    return sqrt(max(v, 0.0));
}

fn de_bin(de: f32) -> u32 {
    let t = sqrt(max(de, 0.0) / DE_RANGE) * f32(DE_BINS);
    return min(u32(t), DE_BINS - 1u);
}

@compute @workgroup_size(16, 16)
fn cs_de(
    @builtin(global_invocation_id) g: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(num_workgroups) nw: vec3<u32>,
) {
    atomicStore(&wh[li], 0u);
    workgroupBarrier();
    var v = vec4<f32>(0.0);
    if g.x < rp.size.x && g.y < rp.size.y {
        let p = vec2<i32>(g.xy);
        let de = delta_e2000(lab_d65(textureLoad(sa, p, 0).rgb), lab_d65(textureLoad(sb, p, 0).rgb));
        atomicAdd(&wh[de_bin(de)], 1u);
        v = vec4<f32>(de, 0.0, 0.0, de);
    }
    workgroupBarrier();
    let n = atomicLoad(&wh[li]);
    if n > 0u {
        atomicAdd(&hist[(rp.slot / 3u) * DE_BINS + li], n);
    }
    sh[li] = v;
    reduce_shared(li);
    if li == 0u {
        partials[wg.y * nw.x + wg.x] = sh[0];
    }
}
