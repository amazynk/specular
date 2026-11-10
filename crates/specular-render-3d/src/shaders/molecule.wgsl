// Molecules as impostors: each atom a camera-facing quad and each stick (bond, cell edge,
// distance) a box around its cylinder, the exact surface found per fragment by casting the
// pixel's ray at the sphere or the capped cylinder, with the depth of the hit written so
// impostors meet each other and the cartoon mesh correctly. Colours are sRGB in u32.
//
// Translucent sticks (`fs_stick_oit`, streamlines below full opacity) are blended
// order-independently (weighted blended OIT, McGuire and Bavoil 2013): each fragment adds
// its weighted premultiplied colour to an accumulation target and multiplies a revealage
// target by 1 - alpha; a full-screen pass (`oit.wgsl`) divides and composites. Only the
// cylinder's near surface counts, once (not the caps, not the far face of the box), so a
// stick is one layer.

// Balls: centre and radius; their colours.
@group(1) @binding(0) var<storage, read> balls: array<vec4<f32>>;
@group(1) @binding(1) var<storage, read> ball_col: array<u32>;
// Sticks: two per stick, (a, radius) and (b, 0); their two colours.
@group(1) @binding(2) var<storage, read> sticks: array<vec4<f32>>;
@group(1) @binding(3) var<storage, read> stick_col: array<vec2<u32>>;

struct IOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Ball: centre, radius. Stick: a, radius.
    @location(1) @interpolate(flat) p: vec4<f32>,
    // Stick: b.
    @location(2) @interpolate(flat) q: vec3<f32>,
    @location(3) @interpolate(flat) c0: vec4<f32>,
    @location(4) @interpolate(flat) c1: vec4<f32>,
};

struct FOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

fn mol_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn colour_of(u: u32) -> vec4<f32> {
    let c = unpack4x8unorm(u);
    return vec4<f32>(mol_linear(c.rgb), c.a);
}

// The viewing direction (from the eye) at a world point.
fn look_at(p: vec3<f32>) -> vec3<f32> {
    if fr.eye.w > 0.5 {
        return normalize(p - fr.eye.xyz);
    }
    return fr.forward.xyz;
}

// The pixel's ray through a world point: origin and unit direction; `back` is how far
// behind the point an orthographic ray starts.
struct Ray {
    o: vec3<f32>,
    d: vec3<f32>,
};

fn ray_through(world: vec3<f32>, back: f32) -> Ray {
    var r: Ray;
    if fr.eye.w > 0.5 {
        r.o = fr.eye.xyz;
        r.d = normalize(world - fr.eye.xyz);
    } else {
        r.d = fr.forward.xyz;
        r.o = world - r.d * back;
    }
    return r;
}

fn depth_of(p: vec3<f32>) -> f32 {
    let c = fr.vp * vec4<f32>(p, 1.0);
    return clamp(c.z / c.w, 0.0, 1.0);
}

// As the mesh's shaded surface: ambient, diffuse and a little specular.
fn lit(base: vec3<f32>, n: vec3<f32>, v: vec3<f32>) -> vec3<f32> {
    let l = fr.light.xyz;
    let diffuse = max(dot(n, l), 0.0);
    let h = normalize(l + v);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.18;
    return base * (fr.light.w + (1.0 - fr.light.w) * diffuse) + vec3<f32>(spec);
}

@vertex
fn vs_ball(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> IOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let b = balls[ii];
    let v = look_at(b.xyz);
    let helper = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(v.z) > 0.9);
    let s = normalize(cross(v, helper));
    let t = cross(s, v);
    // In perspective the silhouette is wider than the radius where the quad lies (through
    // the centre): r·d/√(d² − r²).
    var k = 1.0;
    if fr.eye.w > 0.5 {
        let d = length(b.xyz - fr.eye.xyz);
        k = min(d / sqrt(max(d * d - b.w * b.w, 1e-12)), 4.0);
    }
    let c = corners[vi] * (b.w * k * 1.02);
    let world = b.xyz + s * c.x + t * c.y;
    var o: IOut;
    o.clip = fr.vp * vec4<f32>(world, 1.0);
    o.world = world;
    o.p = b;
    o.q = vec3<f32>(0.0);
    o.c0 = colour_of(ball_col[ii]);
    o.c1 = o.c0;
    return o;
}

@fragment
fn fs_ball(in: IOut) -> FOut {
    let c = in.p.xyz;
    let r = in.p.w;
    let ray = ray_through(in.world, 4.0 * r);
    let oc = ray.o - c;
    let b = dot(oc, ray.d);
    let h = b * b - (dot(oc, oc) - r * r);
    if h < 0.0 {
        discard;
    }
    let t = -b - sqrt(h);
    if t < 0.0 {
        discard;
    }
    let hit = ray.o + ray.d * t;
    let n = normalize(hit - c);
    var o: FOut;
    o.color = vec4<f32>(lit(in.c0.rgb, n, -ray.d), 1.0);
    o.depth = depth_of(hit);
    return o;
}

@vertex
fn vs_stick(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> IOut {
    // A box around the cylinder: 12 triangles over the corners (bit 0 along, 1 and 2 across).
    var tri = array<u32, 36>(
        0u, 1u, 3u, 0u, 3u, 2u, 4u, 6u, 7u, 4u, 7u, 5u,
        0u, 4u, 5u, 0u, 5u, 1u, 2u, 3u, 7u, 2u, 7u, 6u,
        0u, 2u, 6u, 0u, 6u, 4u, 1u, 5u, 7u, 1u, 7u, 3u,
    );
    let a = sticks[2u * ii];
    let b = sticks[2u * ii + 1u].xyz;
    let r = a.w;
    let axis = b - a.xyz;
    let len = max(length(axis), 1e-6);
    let along = axis / len;
    let helper = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(along.z) > 0.9);
    let e1 = normalize(cross(along, helper));
    let e2 = cross(along, e1);
    let k = tri[vi];
    let x = select(-r, len + r, (k & 1u) != 0u);
    let y = select(-r, r, (k & 2u) != 0u) * 1.01;
    let z = select(-r, r, (k & 4u) != 0u) * 1.01;
    let world = a.xyz + along * x + e1 * y + e2 * z;
    var o: IOut;
    o.clip = fr.vp * vec4<f32>(world, 1.0);
    o.world = world;
    o.p = a;
    o.q = b;
    let cc = stick_col[ii];
    o.c0 = colour_of(cc.x);
    o.c1 = colour_of(cc.y);
    return o;
}

@fragment
fn fs_stick(in: IOut) -> FOut {
    let pa = in.p.xyz;
    let r = in.p.w;
    let axis = in.q - pa;
    let len = max(length(axis), 1e-6);
    let ca = axis / len;
    let ray = ray_through(in.world, 4.0 * (len + r));
    let oc = ray.o - pa;
    let card = dot(ca, ray.d);
    let caoc = dot(ca, oc);
    let qa = 1.0 - card * card;
    var t = -1.0;
    var n = vec3<f32>(0.0);
    var y = 0.0;
    if qa > 1e-7 {
        let qb = dot(oc, ray.d) - caoc * card;
        let qc = dot(oc, oc) - caoc * caoc - r * r;
        let h = qb * qb - qa * qc;
        if h >= 0.0 {
            let ts = (-qb - sqrt(h)) / qa;
            let ys = caoc + ts * card;
            if ys >= 0.0 && ys <= len && ts >= 0.0 {
                t = ts;
                y = ys;
                n = normalize(oc + ray.d * ts - ca * ys);
            }
        }
    }
    if t < 0.0 {
        // The flat end facing the ray.
        if abs(card) < 1e-7 {
            discard;
        }
        let end = select(len, 0.0, card > 0.0);
        let tc = (end - caoc) / card;
        let p = oc + ray.d * tc - ca * end;
        if tc < 0.0 || dot(p, p) > r * r {
            discard;
        }
        t = tc;
        y = end;
        n = select(ca, -ca, card > 0.0);
    }
    let hit = ray.o + ray.d * t;
    let base = select(in.c1, in.c0, y < 0.5 * len);
    var o: FOut;
    o.color = vec4<f32>(lit(base.rgb, n, -ray.d), 1.0);
    o.depth = depth_of(hit);
    return o;
}

struct OitOut {
    @location(0) accum: vec4<f32>,
    @location(1) reveal: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

// Where the ray through the pixel enters and leaves a stick's box (local: x along the
// stick from -r to len + r, y and z across from -r to r).
fn box_span(o: vec3<f32>, d: vec3<f32>, len: f32, r: f32) -> vec2<f32> {
    let lo = vec3<f32>(-r, -r * 1.01, -r * 1.01);
    let hi = vec3<f32>(len + r, r * 1.01, r * 1.01);
    let inv = 1.0 / select(d, vec3<f32>(1e-12), abs(d) < vec3<f32>(1e-12));
    let t0 = (lo - o) * inv;
    let t1 = (hi - o) * inv;
    let tn = min(t0, t1);
    let tf = max(t0, t1);
    return vec2<f32>(max(max(tn.x, tn.y), tn.z), min(min(tf.x, tf.y), tf.z));
}

@fragment
fn fs_stick_oit(in: IOut) -> OitOut {
    let pa = in.p.xyz;
    let r = in.p.w;
    let axis = in.q - pa;
    let len = max(length(axis), 1e-6);
    let ca = axis / len;
    let ray = ray_through(in.world, 4.0 * (len + r));
    let oc = ray.o - pa;
    // The box is rasterised front and back: keep the fragment on the face the ray enters.
    let helper = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(ca.z) > 0.9);
    let e1 = normalize(cross(ca, helper));
    let e2 = cross(ca, e1);
    let lo = vec3<f32>(dot(oc, ca), dot(oc, e1), dot(oc, e2));
    let ld = vec3<f32>(dot(ray.d, ca), dot(ray.d, e1), dot(ray.d, e2));
    let span = box_span(lo, ld, len, r);
    let tw = dot(in.world - ray.o, ray.d);
    if tw > 0.5 * (span.x + span.y) {
        discard;
    }
    let card = dot(ca, ray.d);
    let caoc = dot(ca, oc);
    let qa = 1.0 - card * card;
    if qa <= 1e-7 {
        discard;
    }
    let qb = dot(oc, ray.d) - caoc * card;
    let qc = dot(oc, oc) - caoc * caoc - r * r;
    let h = qb * qb - qa * qc;
    if h < 0.0 {
        discard;
    }
    let t = (-qb - sqrt(h)) / qa;
    let y = caoc + t * card;
    if y < 0.0 || y > len || t < 0.0 {
        discard;
    }
    let n = normalize(oc + ray.d * t - ca * y);
    let hit = ray.o + ray.d * t;
    let base = select(in.c1, in.c0, y < 0.5 * len);
    let c = lit(base.rgb, n, -ray.d);
    let a = clamp(fr.params.w, 0.0, 1.0);
    // Nearer layers weigh more: 0 at the near side of the drawn box, 1 at the far side.
    let rad = max(fr.bounds.w, 1e-6);
    let zn = clamp((dot(hit - fr.bounds.xyz, fr.forward.xyz) + rad) / (2.0 * rad), 0.0, 1.0);
    let k = 1.0 - zn;
    let w = a * clamp(1e2 * k * k * k, 1e-2, 1e2);
    var o: OitOut;
    o.accum = vec4<f32>(c * w, w);
    o.reveal = vec4<f32>(a);
    o.depth = depth_of(hit);
    return o;
}
