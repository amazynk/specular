// The UI uber-shader: one instanced quad per primitive, analytic anti-aliasing from signed
// distances, output in linear premultiplied light onto a half-float target.

struct Globals {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var atlas: texture_2d<f32>;
@group(0) @binding(3) var backdrop: texture_2d<f32>;
@group(1) @binding(0) var img: texture_2d<f32>;

struct Inst {
    @location(0) rect: vec4<f32>,
    @location(1) shape: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) color2: vec4<f32>,
    @location(5) params: vec4<f32>,
    @location(6) uv: vec4<f32>,
    @location(7) clip: vec4<f32>,
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) shape: vec4<f32>,
    @location(1) @interpolate(flat) radii: vec4<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) color2: vec4<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
    @location(5) @interpolate(flat) uv: vec4<f32>,
    @location(6) @interpolate(flat) clip: vec4<f32>,
    @location(7) @interpolate(flat) rect: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Inst) -> VOut {
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let p = inst.rect.xy + corner * inst.rect.zw;
    var o: VOut;
    o.pos = vec4<f32>(p.x / g.size.x * 2.0 - 1.0, 1.0 - p.y / g.size.y * 2.0, 0.0, 1.0);
    o.shape = inst.shape;
    o.radii = inst.radii;
    o.color = inst.color;
    o.color2 = inst.color2;
    o.params = inst.params;
    o.uv = inst.uv;
    o.clip = inst.clip;
    o.rect = inst.rect;
    return o;
}

// Signed distance to a rounded box centred at the origin; radii tl, tr, br, bl.
fn sd_rounded_box(p: vec2<f32>, half: vec2<f32>, radii: vec4<f32>) -> f32 {
    var r: f32;
    if (p.x < 0.0) {
        r = select(radii.w, radii.x, p.y < 0.0);
    } else {
        r = select(radii.z, radii.y, p.y < 0.0);
    }
    r = min(r, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

// Evan Wallace's closed-form rounded-rectangle shadow.
fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (2.5066283 * sigma);
}

fn erf2(x: vec2<f32>) -> vec2<f32> {
    let s = sign(x);
    let a = abs(x);
    var r = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    r = r * r;
    return s - s / (r * r);
}

fn shadow_x(x: f32, y: f32, sigma: f32, corner: f32, half: vec2<f32>) -> f32 {
    let delta = min(half.y - corner - abs(y), 0.0);
    let curved = half.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    let integral = 0.5 + 0.5 * erf2((vec2<f32>(x) + vec2<f32>(-curved, curved)) * (0.70710678 / sigma));
    return integral.y - integral.x;
}

fn rounded_shadow(p: vec2<f32>, half: vec2<f32>, sigma: f32, corner: f32) -> f32 {
    let low = p.y - half.y;
    let high = p.y + half.y;
    let start = clamp(-3.0 * sigma, low, high);
    let end = clamp(3.0 * sigma, low, high);
    let step = (end - start) / 4.0;
    var y = start + step * 0.5;
    var v = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + shadow_x(p.x, p.y - y, sigma, corner, half) * gaussian(y, sigma) * step;
        y = y + step;
    }
    return v;
}

fn box_fill(p: vec2<f32>, shape: vec4<f32>, radii: vec4<f32>, fill: vec4<f32>, border: vec4<f32>, bw: f32) -> vec4<f32> {
    let half = shape.zw * 0.5;
    let d = sd_rounded_box(p - (shape.xy + half), half, radii);
    let outer = clamp(0.5 - d, 0.0, 1.0);
    if (bw <= 0.0) {
        return fill * outer;
    }
    let inner = clamp(0.5 - (d + bw), 0.0, 1.0);
    // Fill under the border so translucent borders sit on the surface.
    return border * (outer - inner) + fill * inner + fill * (1.0 - border.a) * (outer - inner);
}

const TAU: f32 = 6.28318530718;

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    if (p.x < in.clip.x || p.y < in.clip.y || p.x >= in.clip.z || p.y >= in.clip.w) {
        discard;
    }
    let kind = u32(in.params.z + 0.5);
    switch kind {
        case 0u: {
            return box_fill(p, in.shape, in.radii, in.color, in.color2, in.params.x);
        }
        case 1u: {
            let half = in.shape.zw * 0.5;
            let a = rounded_shadow(p - (in.shape.xy + half), half, in.params.y, in.radii.x);
            return in.color * a;
        }
        case 2u: {
            let t = vec2<i32>(in.uv.xy + floor(p - in.rect.xy));
            let c = textureLoad(atlas, t, 0).r;
            return in.color * pow(c, in.params.w);
        }
        case 3u: {
            let d = sd_segment(p, in.shape.xy, in.shape.zw) - in.radii.x;
            return in.color * clamp(0.5 - d, 0.0, 1.0);
        }
        case 4u: {
            let q = p - in.shape.xy;
            let r = in.shape.z;
            let a0 = in.radii.x;
            let sweep = in.radii.y;
            let hw = in.radii.z;
            var d: f32;
            let ang = atan2(q.y, q.x);
            let rel = fract((ang - a0) / TAU) * TAU;
            if (sweep >= TAU - 1e-3 || rel <= sweep) {
                d = abs(length(q) - r) - hw;
            } else {
                let e0 = r * vec2<f32>(cos(a0), sin(a0));
                let e1 = r * vec2<f32>(cos(a0 + sweep), sin(a0 + sweep));
                d = min(length(q - e0), length(q - e1)) - hw;
            }
            return in.color * clamp(0.5 - d, 0.0, 1.0);
        }
        case 5u: {
            let bd = textureSampleLevel(backdrop, samp, p / g.size, 0.0);
            let tinted = vec4<f32>(bd.rgb * (1.0 - in.color.a) + in.color.rgb, 1.0);
            return box_fill(p, in.shape, in.radii, tinted, in.color2, in.params.x) * in.params.w;
        }
        case 6u: {
            let t = in.uv.xy + (p - in.shape.xy) / in.shape.zw * in.uv.zw;
            var c = textureSampleLevel(img, samp, t, 0.0);
            // A backdrop under transparency: params.y is the checker cell in physical px
            // (0: none), colour in even cells, color2 in odd ones; the viewer's parity.
            if (in.params.y > 0.0 && c.a < 0.999) {
                let cell = floor(p / in.params.y);
                let odd = ((i32(cell.x) + i32(cell.y)) & 1) == 1;
                c = c + select(in.color, in.color2, odd) * (1.0 - c.a);
            }
            let half = in.shape.zw * 0.5;
            let d = sd_rounded_box(p - (in.shape.xy + half), half, in.radii);
            return c * clamp(0.5 - d, 0.0, 1.0) * in.params.w;
        }
        default: {
            return vec4<f32>(0.0);
        }
    }
}
