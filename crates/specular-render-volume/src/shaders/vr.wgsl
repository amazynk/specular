// Volume rendering: orthographic rays through the volume in raw slice space (sampled with
// shaders/sample.wgsl, prepended), composited front to back through a transfer function, or
// projected as MIP / MinIP through the VOI window. One frame of a progressive accumulation:
// the ray start is jittered per frame and frames are blended (blend constant 1 / frame).
//
// The camera frame: x right, y down, z forward (into the screen); the viewer looks along +z.

struct Vr {
    // Raw position of accumulation pixel (0, 0) (its corner) at t = 0; w = ray length, mm.
    base: vec4<f32>,
    // Raw per accumulation pixel; du.w = step, mm; dv.w = frame (0: no jitter).
    du: vec4<f32>,
    dv: vec4<f32>,
    // Raw per mm along the view; w = most steps.
    dir: vec4<f32>,
    // Volume width, height, slices; w = brick edge in voxels (0: no skipping).
    dims: vec4<f32>,
    // Slice table: a of the first entry, 1 / mm between entries, entries, slices.
    table: vec4<f32>,
    // The raw box holding the volume; lo.w = steps an empty brick jumps; hi.w = gradient
    // step, mm.
    lo: vec4<f32>,
    hi: vec4<f32>,
    // Raw per mm of the camera's right and down; right.w = mode (0 shaded, 1 cinematic,
    // 2 MIP, 3 MinIP); down.w = keep planes.
    right: vec4<f32>,
    down: vec4<f32>,
    // Raw per mm towards the light; w = 1 when planes 12–14 cut a corner away.
    light: vec4<f32>,
    // Transfer table index = value · x + y; z = entries; w = ambient light.
    tf: vec4<f32>,
    // As shaders/medical.wgsl (specular-render): centre c', 1 / w', lower and upper edge.
    voi: vec4<f32>,
    // VOI function (0 linear, 1 sigmoid), inverted; gradient magnitudes (per mm) where
    // shading starts and is full.
    vmode: vec4<f32>,
    // Bricks along i, j, k; unused.
    grid: vec4<f32>,
    // Towards the light in the camera frame; w = shadow step, mm.
    lcam: vec4<f32>,
    // Raw planes (n, d): 0–11 keep n·q + d ≥ 0; 12–14 together cut away where all ≥ 0.
    planes: array<vec4<f32>, 15>,
};

@group(0) @binding(0) var<uniform> u: Vr;
@group(0) @binding(1) var vt: texture_3d<VTYPE>;
@group(0) @binding(2) var<storage, read> lut: array<vec4<f32>>;
// Linear RGB and extinction per mm.
@group(0) @binding(3) var tfx: texture_1d<f32>;
// Per brick: smallest and largest value of it and its neighbours, shown by the transfer
// function (1) or not (0), unused.
@group(0) @binding(4) var<storage, read> bricks: array<vec4<f32>>;

fn transfer(v: f32) -> vec4<f32> {
    let n = u.tf.z;
    let x = clamp(v * u.tf.x + u.tf.y, 0.0, n - 1.0);
    let i0 = i32(floor(x));
    let i1 = min(i0 + 1, i32(n) - 1);
    return mix(textureLoad(tfx, i0, 0), textureLoad(tfx, i1, 0), x - floor(x));
}

// The value at raw point `q`, or `fallback` outside the volume.
fn value_at(q: vec3<f32>, fallback: f32) -> f32 {
    let r = sample_raw(q, u.dims.xyz, u.table);
    if r.y < 0.5 {
        return fallback;
    }
    return r.x;
}

// The gradient at `q` (value `v`) in the camera frame, per mm, by central differences.
fn gradient(q: vec3<f32>, v: f32) -> vec3<f32> {
    let h = u.hi.w;
    let r = u.right.xyz * h;
    let d = u.down.xyz * h;
    let f = u.dir.xyz * h;
    return vec3<f32>(
        value_at(q + r, v) - value_at(q - r, v),
        value_at(q + d, v) - value_at(q - d, v),
        value_at(q + f, v) - value_at(q - f, v),
    ) / (2.0 * h);
}

fn hash(a: u32, b: u32, c: u32) -> u32 {
    var x = (a * 0x9e3779b9u) ^ ((b + 0x7f4a7c15u) * 0x85ebca6bu) ^ ((c + 0x165667b1u) * 0xc2b2ae35u);
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}

fn unit(x: u32) -> f32 {
    return f32(x >> 8u) / 16777216.0;
}

// Transmittance from raw `q` along raw direction `d` (per mm) over `n` steps of `s` mm,
// starting `s · offset` away.
fn transmittance(q: vec3<f32>, d: vec3<f32>, n: u32, s: f32, offset: f32) -> f32 {
    var tau = 0.0;
    for (var m = 0u; m < n; m++) {
        let p = q + d * (s * (f32(m) + offset));
        let r = sample_raw(p, u.dims.xyz, u.table);
        if r.y < 0.5 {
            continue;
        }
        tau += transfer(r.x).w * s;
        if tau > 6.0 {
            break;
        }
    }
    return exp(-tau);
}

// Lit colour of a sample: two-sided Blinn–Phong under a headlight, weighted by the gradient
// magnitude (homogeneous regions keep their unshaded colour); in cinematic mode the light
// is shadowed and the ambient term occluded (random rays, averaged over frames).
fn shade(q: vec3<f32>, v: f32, base: vec3<f32>, seed: u32) -> vec3<f32> {
    let g = gradient(q, v);
    let m = length(g);
    let k = smoothstep(u.vmode.z, u.vmode.w, m);
    if k <= 0.0 {
        return base;
    }
    var n = -g / m;
    if n.z > 0.0 {
        n = -n;
    }
    let l = u.lcam.xyz;
    let eye = vec3<f32>(0.0, 0.0, -1.0);
    let diff = max(dot(n, l), 0.0);
    let spec = pow(max(dot(n, normalize(l + eye)), 0.0), 32.0);
    var amb = u.tf.w;
    var lit = 1.0;
    if u.right.w > 0.5 {
        let s = u.lcam.w;
        lit = transmittance(q, u.light.xyz, 48u, s, 1.0 + unit(hash(seed, 1u, 7u)));
        // A cosine-weighted direction about the normal.
        let r1 = unit(hash(seed, 2u, 11u));
        let r2 = unit(hash(seed, 3u, 13u));
        let a = 6.2831853 * r1;
        let t = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.x) > 0.9);
        let b1 = normalize(cross(n, t));
        let b2 = cross(n, b1);
        let c = n * sqrt(1.0 - r2) + (b1 * cos(a) + b2 * sin(a)) * sqrt(r2);
        let d = u.right.xyz * c.x + u.down.xyz * c.y + u.dir.xyz * c.z;
        amb *= transmittance(q, d, 6u, 4.0 * u.hi.w, 0.5 + unit(hash(seed, 4u, 17u)));
    }
    let shaded = base * (amb + (1.0 - amb) * diff * lit) + vec3<f32>(0.25 * spec * lit);
    return mix(base, shaded, k);
}

fn voi(x: f32) -> f32 {
    if u.vmode.x > 0.5 {
        return 1.0 / (1.0 + exp(-4.0 * (x - u.voi.x) * u.voi.y));
    }
    if x <= u.voi.z { return 0.0; }
    if x > u.voi.w || u.voi.y == 0.0 { return 1.0; }
    return clamp((x - u.voi.x) * u.voi.y + 0.5, 0.0, 1.0);
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { return c / 12.92; }
    return pow((c + 0.055) / 1.055, 2.4);
}

// Narrows [t.x, t.y] to where plane `p` keeps (n·q + d ≥ 0) the ray q0 + dir · t.
fn keep(p: vec4<f32>, q0: vec3<f32>, t: vec2<f32>) -> vec2<f32> {
    let a = dot(p.xyz, q0) + p.w;
    let b = dot(p.xyz, u.dir.xyz);
    if abs(b) < 1e-12 {
        if a < 0.0 {
            return vec2<f32>(1.0, 0.0);
        }
        return t;
    }
    let r = -a / b;
    if b > 0.0 {
        return vec2<f32>(max(t.x, r), t.y);
    }
    return vec2<f32>(t.x, min(t.y, r));
}

fn brick(v: vec3<f32>) -> vec4<f32> {
    let e = u.dims.w;
    let c = clamp(v, vec3<f32>(0.0), u.dims.xyz - 1.0);
    let b = min(vec3<u32>(c / e), vec3<u32>(u.grid.xyz) - 1u);
    return bricks[(b.z * u32(u.grid.y) + b.y) * u32(u.grid.x) + b.x];
}

@fragment
fn fs_vr(in: VOut) -> @location(0) vec4<f32> {
    let q0 = u.base.xyz + u.du.xyz * in.pos.x + u.dv.xyz * in.pos.y;
    let dir = u.dir.xyz;
    var t = vec2<f32>(0.0, u.base.w);
    // The box.
    for (var c = 0; c < 3; c++) {
        if abs(dir[c]) < 1e-12 {
            if q0[c] < u.lo[c] || q0[c] > u.hi[c] {
                return vec4<f32>(0.0);
            }
        } else {
            let a = (u.lo[c] - q0[c]) / dir[c];
            let b = (u.hi[c] - q0[c]) / dir[c];
            t = vec2<f32>(max(t.x, min(a, b)), min(t.y, max(a, b)));
        }
    }
    for (var p = 0u; p < u32(u.down.w); p++) {
        t = keep(u.planes[p], q0, t);
    }
    var cut = vec2<f32>(1.0, 0.0);
    if u.light.w > 0.5 {
        cut = vec2<f32>(-1e30, 1e30);
        for (var p = 12u; p < 15u; p++) {
            cut = keep(u.planes[p], q0, cut);
        }
    }
    if t.y <= t.x {
        return vec4<f32>(0.0);
    }
    let step = u.du.w;
    let frame = u32(u.dv.w);
    let px = vec2<u32>(in.pos.xy);
    var jit = 0.5;
    if frame > 0u {
        jit = unit(hash(px.x, px.y, frame));
    }
    let mode = u.right.w;
    let skip = u.dims.w > 0.0;
    let jump = max(u32(u.lo.w), 1u);
    let most = u32(u.dir.w);
    var acc = vec4<f32>(0.0);
    var ext = 0.0;
    var hit = false;
    var k = 0u;
    for (var n = 0u; n < most; n++) {
        let tk = t.x + (f32(k) + jit) * step;
        if tk >= t.y {
            break;
        }
        if tk >= cut.x && tk < cut.y {
            k = max(k + 1u, u32(ceil((cut.y - t.x) / step - jit)));
            continue;
        }
        let q = q0 + dir * tk;
        let l = locate(q, u.dims.xyz, u.table);
        if l.w < 0.5 {
            k++;
            continue;
        }
        if skip {
            let b = brick(l.xyz);
            if (mode < 1.5 && b.z < 0.5) || (hit && mode > 1.5 && mode < 2.5 && b.y <= ext)
                || (hit && mode > 2.5 && b.x >= ext) {
                k += jump;
                continue;
            }
        }
        let r = fetch(l.xyz, u.dims.xyz, u.table.z);
        if r.y < 0.5 {
            k++;
            continue;
        }
        let v = r.x;
        if mode > 1.5 {
            if !hit {
                ext = v;
            } else if mode < 2.5 {
                ext = max(ext, v);
            } else {
                ext = min(ext, v);
            }
            hit = true;
            k++;
            continue;
        }
        let e = transfer(v);
        let a = 1.0 - exp(-e.w * step);
        if a > 1e-4 {
            let col = shade(q, v, e.rgb, hash(px.x * 7919u + k, px.y, frame));
            acc += vec4<f32>(col * a, a) * (1.0 - acc.a);
            if acc.a > 0.99 {
                break;
            }
        }
        k++;
    }
    if mode > 1.5 {
        if !hit {
            return vec4<f32>(0.0);
        }
        var g = voi(ext);
        if u.vmode.y > 0.5 {
            g = 1.0 - g;
        }
        let l = srgb_to_linear(g);
        return vec4<f32>(l, l, l, 1.0);
    }
    return acc;
}
