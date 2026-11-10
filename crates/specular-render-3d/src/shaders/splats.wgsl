// Gaussian splats: one instance per splat, in the back-to-front order the CPU sorted
// (`order`), each a screen-space quad around its centre covering three standard deviations
// of its projected covariance (EWA: the projection's Jacobian at the centre, plus a 0.3 px²
// low-pass). The colour is the base colour plus the harmonics seen along the view ray. The
// target takes display-referred premultiplied colour, blended as the reference renderer
// does; the present pass turns it linear.

struct Splat {
    // xyz: the view's right vector; w: 1 in orthographic.
    right: vec4<f32>,
    down: vec4<f32>,
    forward: vec4<f32>,
    // xyz: the eye (perspective) or the view's centre (orthographic); w: px per unit of
    // x/z (perspective) or per model unit (orthographic).
    origin: vec4<f32>,
    // xy: the view's size (px); z: harmonic coefficients per channel (0, 3, 8, 15); w: 1 when
    // trained with anti-aliasing.
    view: vec4<f32>,
    // Rows of the map from the viewer's frame back to the file's (for the harmonics).
    to_file0: vec4<f32>,
    to_file1: vec4<f32>,
    to_file2: vec4<f32>,
};

@group(1) @binding(0) var<uniform> su: Splat;
// 12 words a splat: x, y, z, the covariance (xx, xy, xz, yy, yz, zz) as f32 bits, then the
// colour and opacity as four halves, then a pad word.
@group(1) @binding(1) var<storage, read> core: array<u32>;
// The higher harmonics as halves, `su.view.z * 3` a splat, two to a word (rounded up).
@group(1) @binding(2) var<storage, read> sh: array<u32>;
// Splat indices, farthest first.
@group(1) @binding(3) var<storage, read> order: array<u32>;

struct SOut {
    @builtin(position) clip: vec4<f32>,
    // Offset from the centre (px).
    @location(0) d: vec2<f32>,
    @location(1) @interpolate(flat) conic: vec3<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
};

fn half_at(base: u32, i: u32) -> f32 {
    let w = sh[base + i / 2u];
    let pair = unpack2x16float(w);
    return select(pair.x, pair.y, (i & 1u) == 1u);
}

const C1: f32 = 0.48860251;

fn sh_colour(s: u32, base: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let k = u32(su.view.z);
    var c = base;
    if k == 0u {
        return max(c, vec3<f32>(0.0));
    }
    let x = d.x;
    let y = d.y;
    let z = d.z;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    var b = array<f32, 15>(
        -C1 * y, C1 * z, -C1 * x,
        1.0925484 * x * y, -1.0925484 * y * z, 0.31539157 * (2.0 * zz - xx - yy),
        -1.0925484 * x * z, 0.54627421 * (xx - yy),
        -0.59004359 * y * (3.0 * xx - yy), 2.8906114 * x * y * z,
        -0.45704580 * y * (4.0 * zz - xx - yy), 0.37317633 * z * (2.0 * zz - 3.0 * xx - 3.0 * yy),
        -0.45704580 * x * (4.0 * zz - xx - yy), 1.4453057 * z * (xx - yy),
        -0.59004359 * x * (xx - 3.0 * yy),
    );
    let per = k * 3u;
    let words = (per + 1u) / 2u;
    let at = s * words;
    for (var j = 0u; j < k; j++) {
        let h = vec3<f32>(half_at(at, j * 3u), half_at(at, j * 3u + 1u), half_at(at, j * 3u + 2u));
        c += b[j] * h;
    }
    return max(c, vec3<f32>(0.0));
}

@vertex
fn vs_splat(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> SOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    var o: SOut;
    // Outside the clip volume unless set below.
    o.clip = vec4<f32>(0.0, 0.0, 2.0, 1.0);
    let s = order[ii];
    let b = s * 12u;
    let p = vec3<f32>(bitcast<f32>(core[b]), bitcast<f32>(core[b + 1u]), bitcast<f32>(core[b + 2u]));
    let rel = p - su.origin.xyz;
    var t = vec3<f32>(dot(rel, su.right.xyz), dot(rel, su.down.xyz), dot(rel, su.forward.xyz));
    let f = su.origin.w;
    let half_size = 0.5 * su.view.xy;
    let ortho = su.right.w > 0.5;
    var centre: vec2<f32>;
    var j0: vec3<f32>;
    var j1: vec3<f32>;
    if ortho {
        centre = half_size + f * t.xy;
        j0 = vec3<f32>(f, 0.0, 0.0);
        j1 = vec3<f32>(0.0, f, 0.0);
    } else {
        if t.z < 1e-3 {
            return o;
        }
        centre = half_size + f * t.xy / t.z;
        let lim = 1.3 * half_size / f;
        t = vec3<f32>(clamp(t.xy, -lim * t.z, lim * t.z), t.z);
        j0 = vec3<f32>(f / t.z, 0.0, -f * t.x / (t.z * t.z));
        j1 = vec3<f32>(0.0, f / t.z, -f * t.y / (t.z * t.z));
    }
    let c = array<f32, 6>(
        bitcast<f32>(core[b + 3u]), bitcast<f32>(core[b + 4u]), bitcast<f32>(core[b + 5u]),
        bitcast<f32>(core[b + 6u]), bitcast<f32>(core[b + 7u]), bitcast<f32>(core[b + 8u]),
    );
    let sigma = mat3x3<f32>(
        vec3<f32>(c[0], c[1], c[2]),
        vec3<f32>(c[1], c[3], c[4]),
        vec3<f32>(c[2], c[4], c[5]),
    );
    // W Σ Wᵀ in view space (W's rows: right, down, forward).
    let w = transpose(mat3x3<f32>(su.right.xyz, su.down.xyz, su.forward.xyz));
    let sv = w * sigma * transpose(w);
    let a = sv * j0;
    let bb = sv * j1;
    let cov = vec3<f32>(dot(j0, a) + 0.3, dot(j0, bb), dot(j1, bb) + 0.3);
    let det = cov.x * cov.z - cov.y * cov.y;
    if !(det > 0.0) {
        return o;
    }
    let mid = 0.5 * (cov.x + cov.z);
    let l1 = mid + sqrt(max(mid * mid - det, 0.1));
    let radius = ceil(3.0 * sqrt(l1));
    if any(centre + radius < vec2<f32>(0.0)) || any(centre - radius > su.view.xy) {
        return o;
    }
    let rgba = vec4<f32>(unpack2x16float(core[b + 9u]), unpack2x16float(core[b + 10u]));
    var opacity = rgba.a;
    if su.view.w > 0.5 {
        let det0 = (cov.x - 0.3) * (cov.z - 0.3) - cov.y * cov.y;
        opacity *= sqrt(max(det0, 0.0) / det);
    }
    var dir = su.forward.xyz;
    if !ortho {
        dir = normalize(rel);
    }
    let fd = vec3<f32>(dot(su.to_file0.xyz, dir), dot(su.to_file1.xyz, dir), dot(su.to_file2.xyz, dir));
    let colour = sh_colour(s, rgba.rgb, fd);
    let off = corners[vi] * radius;
    let px = centre + off;
    o.clip = vec4<f32>(px.x / su.view.x * 2.0 - 1.0, 1.0 - px.y / su.view.y * 2.0, 0.5, 1.0);
    o.d = off;
    o.conic = vec3<f32>(cov.z / det, -cov.y / det, cov.x / det);
    o.color = vec4<f32>(colour, opacity);
    return o;
}

@fragment
fn fs_splat(in: SOut) -> @location(0) vec4<f32> {
    let d = in.d;
    let power = -0.5 * (in.conic.x * d.x * d.x + in.conic.z * d.y * d.y) - in.conic.y * d.x * d.y;
    if power > 0.0 {
        discard;
    }
    let a = min(0.99, in.color.a * exp(power));
    if a < 1.0 / 255.0 {
        discard;
    }
    return vec4<f32>(in.color.rgb * a, a);
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// The grid, in the splats' display-referred encoding.
@fragment
fn fs_grid_splats(in: GOut) -> @location(0) vec4<f32> {
    let g = grid_colour(in);
    if g.a <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(linear_to_srgb(g.rgb / g.a) * g.a, g.a);
}
