// Triangle meshes, pulled from storage buffers: the draw's vertex index walks the chunk's
// index list, so the corner within its triangle (vertex_index % 3) gives barycentrics for
// the edges. Positions are 16-bit quantised over the model's box, normals octahedral.
// Everything is in the canonical frame (z up); colours are linear, premultiplied.

struct Material {
    // Linear base colour.
    color: vec4<f32>,
    // x: vertex colours; y: a texture; z: flat (normals from the screen derivatives).
    flags: vec4<u32>,
};

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var<storage, read> pos: array<u32>;
@group(1) @binding(2) var<storage, read> nrm: array<u32>;
@group(1) @binding(3) var<storage, read> col: array<u32>;
@group(1) @binding(4) var<storage, read> uvs: array<vec2<f32>>;
@group(1) @binding(5) var<storage, read> idx: array<u32>;
@group(1) @binding(6) var tex: texture_2d<f32>;
@group(1) @binding(7) var smp: sampler;

struct MOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) bary: vec3<f32>,
};

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    let t = max(-n.z, 0.0);
    n.x = n.x + select(t, -t, n.x >= 0.0);
    n.y = n.y + select(t, -t, n.y >= 0.0);
    return normalize(n);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@vertex
fn vs_mesh(@builtin(vertex_index) vi: u32) -> MOut {
    let v = idx[vi];
    let a = pos[2u * v];
    let b = pos[2u * v + 1u];
    let q = vec3<f32>(f32(a & 0xffffu), f32(a >> 16u), f32(b & 0xffffu));
    let world = fr.q_offset.xyz + q * fr.q_scale.xyz;
    var o: MOut;
    o.clip = fr.vp * vec4<f32>(world, 1.0);
    o.world = world;
    o.normal = vec3<f32>(0.0, 0.0, 1.0);
    if mat.flags.z == 0u {
        o.normal = oct_decode(unpack2x16snorm(nrm[v]));
    }
    var c = mat.color;
    if mat.flags.x != 0u {
        let vc = unpack4x8unorm(col[v]);
        c = c * vec4<f32>(srgb_to_linear(vc.rgb), vc.a);
    }
    o.color = c;
    o.uv = vec2<f32>(0.0);
    if mat.flags.y != 0u {
        o.uv = uvs[v];
    }
    let k = vi % 3u;
    o.bary = vec3<f32>(f32(k == 0u), f32(k == 1u), f32(k == 2u));
    return o;
}

fn towards_eye(world: vec3<f32>) -> vec3<f32> {
    if fr.eye.w > 0.5 {
        return normalize(fr.eye.xyz - world);
    }
    return -fr.forward.xyz;
}

// The surface normal, facing the viewer (both sides of a surface are lit alike).
fn facing_normal(in: MOut, v: vec3<f32>) -> vec3<f32> {
    let flat_n = cross(dpdx(in.world), dpdy(in.world));
    var n = normalize(in.normal);
    if mat.flags.z != 0u {
        n = normalize(flat_n);
    }
    if dot(n, v) < 0.0 {
        n = -n;
    }
    return n;
}

fn base_color(in: MOut) -> vec4<f32> {
    let t = textureSample(tex, smp, in.uv);
    if mat.flags.y != 0u {
        return in.color * t;
    }
    return in.color;
}

// 1 on a triangle's edge, falling to 0 over the edge width.
fn edge(bary: vec3<f32>) -> f32 {
    let d = fwidth(bary);
    let a = smoothstep(vec3<f32>(0.0), d * fr.params.x, bary);
    return 1.0 - min(min(a.x, a.y), a.z);
}

// Section caps: diagonal hatching over a light tint of the material.
fn hatched(in: MOut, base: vec3<f32>) -> vec3<f32> {
    let spacing = 7.0 * max(fr.params.x / 1.1, 0.5);
    let s = fract((in.clip.x + in.clip.y) / spacing);
    let line = 1.0 - smoothstep(0.12, 0.24, min(s, 1.0 - s));
    let fill = mix(base, vec3<f32>(1.0), 0.35);
    return mix(fill, base * 0.25, line);
}

@fragment
fn fs_shaded(in: MOut) -> @location(0) vec4<f32> {
    if clipped(in.world) {
        discard;
    }
    if mat.flags.w != 0u {
        return vec4<f32>(hatched(in, in.color.rgb), 1.0);
    }
    let v = towards_eye(in.world);
    let n = facing_normal(in, v);
    let base = base_color(in);
    let e = edge(in.bary) * fr.params.y;
    let l = fr.light.xyz;
    let diffuse = max(dot(n, l), 0.0);
    let h = normalize(l + v);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.15;
    let lit = base.rgb * (fr.light.w + (1.0 - fr.light.w) * diffuse) + vec3<f32>(spec);
    return vec4<f32>(mix(lit, vec3<f32>(0.015, 0.017, 0.02), e * 0.85), 1.0);
}

@fragment
fn fs_wire(in: MOut) -> @location(0) vec4<f32> {
    if clipped(in.world) {
        discard;
    }
    let e = edge(in.bary);
    if e < 0.004 {
        discard;
    }
    let c = mix(vec3<f32>(0.62, 0.70, 0.82), in.color.rgb, 0.35);
    return vec4<f32>(c * e, e);
}

@fragment
fn fs_xray(in: MOut) -> @location(0) vec4<f32> {
    if clipped(in.world) {
        discard;
    }
    let v = towards_eye(in.world);
    let n = facing_normal(in, v);
    let rim = 1.0 - abs(dot(n, v));
    let a = clamp(fr.params.z * (0.08 + 0.92 * rim * rim * rim), 0.0, 1.0);
    let tint = mix(vec3<f32>(0.45, 0.68, 1.0), in.color.rgb, 0.3);
    return vec4<f32>(tint * a, a);
}
