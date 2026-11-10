// Relighting of reflectance transformation images (PTM, HSH, Relight RBF and bilinear): the
// coefficient planes, three per array layer, are mapped by each layer's scale and offset and
// summed through a 3×3 matrix per layer for the light (built on the CPU, specular_rti::shade,
// which is the reference this mirrors operation for operation). Writes the overview's mip 0:
// display-coded colour decoded to linear (the target is sRGB-encoded storage).

struct Layer {
    scale: vec4<f32>,
    offset: vec4<f32>,
    // Rows of the colour and the normal matrix.
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    n0: vec4<f32>,
    n1: vec4<f32>,
    n2: vec4<f32>,
};

struct Relight {
    colour: vec4<f32>,
    normal: vec4<f32>,
    light: vec4<f32>,
    // gain, kd, ks, exponent
    params: vec4<f32>,
    // mode (0 default, 1 specular, 2 normals, 3 diffuse gain), albedo (LRGB), layers
    flags: vec4<u32>,
    layers: array<Layer, 32>,
};

@group(0) @binding(0) var<uniform> u: Relight;
@group(0) @binding(1) var planes: texture_2d_array<f32>;

fn unit_normal(g: vec3<f32>) -> vec3<f32> {
    let l = length(g);
    if l > 1e-6 { return g / l; }
    return vec3<f32>(0.0, 0.0, 1.0);
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_relight(in: VOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(in.pos.xy));
    var c = u.colour.xyz;
    var g = u.normal.xyz;
    var tint = vec3<f32>(1.0);
    for (var j = 0u; j < u.flags.z; j++) {
        let l = u.layers[j];
        let v = round(textureLoad(planes, p, i32(j), 0).xyz * 255.0) * l.scale.xyz + l.offset.xyz;
        if j == 0u { tint = v; }
        c += vec3<f32>(dot(l.c0.xyz, v), dot(l.c1.xyz, v), dot(l.c2.xyz, v));
        g += vec3<f32>(dot(l.n0.xyz, v), dot(l.n1.xyz, v), dot(l.n2.xyz, v));
    }
    let albedo = u.flags.y == 1u;
    var lum = (c.x + c.y + c.z) / 3.0;
    if albedo {
        lum = c.x;
        c = c * tint;
    } else {
        tint = vec3<f32>(1.0);
    }
    var out = c;
    switch u.flags.x {
        case 1u: {
            let n = unit_normal(g);
            let h = normalize(u.light.xyz + vec3<f32>(0.0, 0.0, 1.0));
            let d = max(dot(n, h), 0.0);
            let s = select(u.params.z * pow(d, u.params.w) * max(lum, 0.0), 0.0, d <= 0.0);
            out = u.params.y * c + vec3<f32>(s);
        }
        case 2u: {
            out = unit_normal(g) * 0.5 + 0.5;
        }
        case 3u: {
            let d = (u.params.x - 1.0) * (lum - dot(g, u.light.xyz));
            out = c + d * tint;
        }
        default: {}
    }
    return vec4<f32>(srgb_decode(clamp(out, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
