// Decoded planes → linear-light RGBA (straight alpha), written into an sRGB-encoded tile layer
// (any SDR layout, 16-bit sRGB-coded included) or overview texture (8-bit layouts) or a
// half-float overview (16-bit and float layouts: linear, 1.0 = SDR reference white = 203
// cd/m², BT.709 primaries). One fragment per destination texel. The HDR transfer functions
// mirror specular_core::hdr.
//
// Colour management (specular-color): an embedded profile's source transform maps encoded
// RGB to linear BT.709 (has_lut 1: a 3D LUT; 2: per-channel curves in a N×1×1 texture, then
// a 3×4 matrix). The result is then mapped into the storage space (`store`: the display's
// primaries for wide-gamut matrix displays, else identity); image.wgsl maps it back.

struct Cvt {
    // Level pixel of destination texel (0, 0).
    dst_origin: vec2<f32>,
    // Level pixel of raster texel (0, 0).
    raster_origin: vec2<f32>,
    raster_size: vec2<f32>,
    level_size: vec2<f32>,
    // Subsampling divisors of planes 1 and 2 (relative to the raster).
    div1: vec2<f32>,
    div2: vec2<f32>,
    // 0 gray, 1 ycbcr, 2 rgb, 3 rgba, 4 rgba16 (uint, p16), 5 rgba16f (half float, p0),
    // 6 gray16 (R16Uint, p16), 7 gray+alpha16 (Rg16Uint, p16); 6 and 7 are sRGB-coded;
    // 8 YCbCr planes (R8Unorm p0‥p2 and alpha pa, or above 8 bits one R16Uint p16 stacking
    // the Y, Cb, Cr and alpha planes top to bottom).
    plane_layout: u32,
    // 0 none (sRGB), 1 cube, 2 curves + matrix
    has_lut: u32,
    lut_size: f32,
    // Destination texel → level pixel scale (1 for tiles, level/overview ratio otherwise).
    step: f32,
    // Of layouts 4 and 8: 0 sRGB, 1 linear, 2 PQ, 3 HLG.
    transfer: u32,
    // Of layouts 4, 5 and 8: 0 BT.709, 1 Display P3, 2 BT.2020.
    primaries: u32,
    // Auto-range of layouts 6 and 7 (range.rs): 1 on; the samples' significant bits.
    // Layout 8 has its sample bits in range_bits and flags in range_on: bit 0 full range,
    // bit 1 horizontally co-sited chroma, bits 2‥4 the matrix (YuvMatrix::code: 0 BT.601,
    // 1 BT.709, 2 BT.2020, 3 SMPTE 240M, 4 FCC, 5 identity (G, B, R planes), 6 YCgCo), bit 5
    // vertically co-sited chroma, bit 6 chroma on the last luma row it covers (bottom), bit 7
    // chroma on the last luma column it covers (right), bit 8 a straight alpha plane.
    range_on: u32,
    range_bits: u32,
    // Source matrix rows (curves mode), offset in w.
    src0: vec4<f32>,
    src1: vec4<f32>,
    src2: vec4<f32>,
    // Linear BT.709 → storage space rows.
    st0: vec4<f32>,
    st1: vec4<f32>,
    st2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Cvt;
@group(0) @binding(1) var p0: texture_2d<f32>;
@group(0) @binding(2) var p1: texture_2d<f32>;
@group(0) @binding(3) var p2: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;
@group(0) @binding(5) var lut: texture_3d<f32>;
@group(0) @binding(6) var p16: texture_2d<u32>;
// [min, max] code of the image (auto-range), or [0, 65535].
@group(0) @binding(7) var<storage, read> range: array<u32>;
// Alpha plane of 8-bit layout 8.
@group(0) @binding(8) var pa: texture_2d<f32>;

const REF_WHITE: f32 = 203.0;

fn pq_to_linear(e: vec3<f32>) -> vec3<f32> {
    let m1 = 2610.0 / 16384.0;
    let m2 = 2523.0 / 4096.0 * 128.0;
    let c1 = 3424.0 / 4096.0;
    let c2 = 2413.0 / 4096.0 * 32.0;
    let c3 = 2392.0 / 4096.0 * 32.0;
    let p = pow(clamp(e, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / m2));
    let y = pow(max(p - c1, vec3<f32>(0.0)) / (c2 - c3 * p), vec3<f32>(1.0 / m1));
    return y * (10000.0 / REF_WHITE);
}

fn hlg_to_linear(e: vec3<f32>) -> vec3<f32> {
    let a = 0.17883277;
    let b = 0.28466892;
    let c = 0.5599107;
    let x = clamp(e, vec3<f32>(0.0), vec3<f32>(1.0));
    let lo = x * x / 3.0;
    let hi = (exp((x - c) / a) + b) / 12.0;
    let s = select(hi, lo, x <= vec3<f32>(0.5));
    // BT.2100 OOTF for a 1000 cd/m² display (system gamma 1.2).
    let ys = max(dot(s, vec3<f32>(0.2627, 0.6780, 0.0593)), 0.0);
    return s * (1000.0 / REF_WHITE) * pow(max(ys, 1e-6), 0.2);
}

fn to_bt709(c: vec3<f32>, p: u32) -> vec3<f32> {
    switch p {
        case 1u: {
            return vec3<f32>(
                dot(vec3<f32>(1.224940, -0.224940, 0.0), c),
                dot(vec3<f32>(-0.042057, 1.042057, 0.0), c),
                dot(vec3<f32>(-0.019638, -0.078636, 1.098274), c));
        }
        case 2u: {
            return vec3<f32>(
                dot(vec3<f32>(1.660491, -0.587641, -0.072850), c),
                dot(vec3<f32>(-0.124551, 1.132900, -0.008349), c),
                dot(vec3<f32>(-0.018151, -0.100579, 1.118730), c));
        }
        default: { return c; }
    }
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// Encoded RGB (0..1) → linear BT.709 through the source transform, or the sRGB decode.
fn source(rgb: vec3<f32>) -> vec3<f32> {
    if u.has_lut == 1u {
        let n = u.lut_size;
        let l = textureSampleLevel(lut, samp, rgb * ((n - 1.0) / n) + 0.5 / n, 0.0).rgb;
        return max(l, vec3<f32>(0.0));
    }
    if u.has_lut == 2u {
        let n = u.lut_size;
        let x = rgb * ((n - 1.0) / n) + 0.5 / n;
        let c = vec3<f32>(
            textureSampleLevel(lut, samp, vec3<f32>(x.r, 0.5, 0.5), 0.0).r,
            textureSampleLevel(lut, samp, vec3<f32>(x.g, 0.5, 0.5), 0.0).g,
            textureSampleLevel(lut, samp, vec3<f32>(x.b, 0.5, 0.5), 0.0).b);
        // Wide-gamut sources leave BT.709: negative components are kept here and clipped in
        // the storage space.
        return vec3<f32>(
            dot(u.src0.xyz, c) + u.src0.w,
            dot(u.src1.xyz, c) + u.src1.w,
            dot(u.src2.xyz, c) + u.src2.w);
    }
    return srgb_to_linear(rgb);
}

// Linear BT.709 → storage space, clipped to its gamut (and to `hi`).
fn to_store(c: vec3<f32>, hi: f32) -> vec3<f32> {
    let s = vec3<f32>(dot(u.st0.xyz, c), dot(u.st1.xyz, c), dot(u.st2.xyz, c));
    return clamp(s, vec3<f32>(0.0), vec3<f32>(hi));
}

fn chroma(t: texture_2d<f32>, rp: vec2<f32>, div: vec2<f32>) -> f32 {
    // Centred chroma siting: raster pixel centre (x + 0.5) maps to chroma (x + 0.5) / div.
    // Bilinear by hand (edges replicated) rather than through the sampler: Vulkan lets
    // implementations filter 8-bit textures at 8-bit precision (llvmpipe does), and the
    // Cb/Cr gains (up to 1.772) turn the rounded chroma into ~1.7 codes of error.
    let hi = vec2<i32>(textureDimensions(t)) - 1;
    let f = (rp + 0.5) / div - 0.5;
    let f0 = floor(f);
    let w = f - f0;
    let i = vec2<i32>(f0);
    let a = textureLoad(t, clamp(i, vec2<i32>(0), hi), 0).r;
    let b = textureLoad(t, clamp(i + vec2<i32>(1, 0), vec2<i32>(0), hi), 0).r;
    let c = textureLoad(t, clamp(i + vec2<i32>(0, 1), vec2<i32>(0), hi), 0).r;
    let d = textureLoad(t, clamp(i + vec2<i32>(1, 1), vec2<i32>(0), hi), 0).r;
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

// A grey code (0‥65535) as a 0‥1 encoded value. Under auto-range it is stretched from the
// image's smallest to its largest code; codes at the real bit depth's maximum (all top
// `range_bits` bits set) become exactly 1 and those at its minimum exactly 0, and every other
// code stays within 1/255‥254/255, so HiLo (image.wgsl) marks exactly the former through
// 8-bit storage (legend.rs BitRange::is_high16 is the CPU twin).
fn grey(code: u32) -> f32 {
    if u.range_on == 0u {
        return f32(code) / 65535.0;
    }
    let bits = clamp(u.range_bits, 1u, 16u);
    let top = code >> (16u - bits);
    if top == (1u << bits) - 1u {
        return 1.0;
    }
    if top == 0u {
        return 0.0;
    }
    let lo = f32(range[0]);
    let hi = f32(range[1]);
    var t = 0.5;
    if hi > lo {
        t = (f32(code) - lo) / (hi - lo);
    }
    return clamp(t, 1.0 / 255.0, 254.0 / 255.0);
}

// A sample code of plane `k` (0 Y, 1 Cb, 2 Cr, 3 alpha) at `i`, clamped to the plane.
fn yuv_code(k: u32, i: vec2<i32>) -> f32 {
    let size = u.raster_size;
    var dims = vec2<i32>(size);
    if k == 1u {
        dims = vec2<i32>(ceil(size / u.div1));
    } else if k == 2u {
        dims = vec2<i32>(ceil(size / u.div2));
    }
    let c = clamp(i, vec2<i32>(0), dims - 1);
    if u.range_bits > 8u {
        var y0 = 0;
        if k >= 1u {
            y0 = i32(size.y);
        }
        if k >= 2u {
            y0 += i32(ceil(size.y / u.div1.y));
        }
        if k == 3u {
            y0 += i32(ceil(size.y / u.div2.y));
        }
        return f32(textureLoad(p16, c + vec2<i32>(0, y0), 0).r);
    }
    switch k {
        case 1u: { return textureLoad(p1, c, 0).r * 255.0; }
        case 2u: { return textureLoad(p2, c, 0).r * 255.0; }
        case 3u: { return textureLoad(pa, c, 0).r * 255.0; }
        default: { return textureLoad(p0, c, 0).r * 255.0; }
    }
}

// Straight alpha of layout 8 at `ip`: its plane at the sample bits, full range.
fn yuv_alpha(ip: vec2<i32>) -> f32 {
    if (u.range_on & 256u) == 0u {
        return 1.0;
    }
    let bits = clamp(u.range_bits, 8u, 16u);
    return clamp(yuv_code(3u, ip) / f32((1u << bits) - 1u), 0.0, 1.0);
}

// A video chroma code at raster pixel `rp`, bilinear with the coded siting (co-sited or
// centred, horizontally and vertically); specular_core::yuv::chroma is the CPU twin.
fn yuv_chroma(k: u32, rp: vec2<f32>, div: vec2<f32>) -> f32 {
    var f = (rp + 0.5) / div - 0.5;
    if (u.range_on & 2u) != 0u {
        f.x = rp.x / div.x;
    }
    if (u.range_on & 128u) != 0u {
        // Right siting: chroma column j sits on luma column (j + 1) × div − 1.
        f.x = (rp.x + 1.0) / div.x - 1.0;
    }
    if (u.range_on & 32u) != 0u {
        f.y = rp.y / div.y;
    }
    if (u.range_on & 64u) != 0u {
        // Bottom siting: chroma row j sits on luma row (j + 1) × div − 1.
        f.y = (rp.y + 1.0) / div.y - 1.0;
    }
    if div.x <= 1.0 {
        f.x = rp.x;
    }
    if div.y <= 1.0 {
        f.y = rp.y;
    }
    f = max(f, vec2<f32>(0.0));
    let f0 = floor(f);
    let w = f - f0;
    let i = vec2<i32>(f0);
    let a = yuv_code(k, i);
    let b = yuv_code(k, i + vec2<i32>(1, 0));
    let c = yuv_code(k, i + vec2<i32>(0, 1));
    let d = yuv_code(k, i + vec2<i32>(1, 1));
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

// Video Y'CbCr at `rp` → R'G'B' (unclamped), mirroring specular_core::yuv::to_rgb.
fn yuv_rgb(rp: vec2<f32>) -> vec3<f32> {
    let bits = clamp(u.range_bits, 8u, 16u);
    let unit = f32(1u << (bits - 8u));
    var yo = 16.0 * unit;
    var ys = 1.0 / (219.0 * unit);
    var cs = 1.0 / (224.0 * unit);
    if (u.range_on & 1u) != 0u {
        let mx = f32((1u << bits) - 1u);
        yo = 0.0;
        ys = 1.0 / mx;
        cs = 1.0 / mx;
    }
    let matrix = (u.range_on >> 2u) & 7u;
    var co = 128.0 * unit;
    if matrix == 5u {
        // Identity: G, B, R planes, each coded like luma.
        co = yo;
        cs = ys;
    }
    let l = (yuv_code(0u, vec2<i32>(rp)) - yo) * ys;
    let cb = (yuv_chroma(1u, rp, u.div1) - co) * cs;
    let cr = (yuv_chroma(2u, rp, u.div2) - co) * cs;
    var k = vec2<f32>(0.299, 0.114);
    switch matrix {
        case 1u: { k = vec2<f32>(0.2126, 0.0722); }
        case 2u: { k = vec2<f32>(0.2627, 0.0593); }
        case 3u: { k = vec2<f32>(0.212, 0.087); }
        case 4u: { k = vec2<f32>(0.30, 0.11); }
        case 5u: { return vec3<f32>(cr, l, cb); }
        case 6u: {
            // YCgCo: Cb carries Cg, Cr carries Co.
            let t = l - cb;
            return vec3<f32>(t + cr, l + cb, t - cr);
        }
        default: {}
    }
    let kg = 1.0 - k.x - k.y;
    let rv = 2.0 * (1.0 - k.x);
    let bu = 2.0 * (1.0 - k.y);
    return vec3<f32>(l + rv * cr, l - bu * k.y / kg * cb - rv * k.x / kg * cr, l + bu * cb);
}

@fragment
fn fs_convert(in: VOut) -> @location(0) vec4<f32> {
    return convert(in.pos.xy);
}

// Detail tiles of images with wide samples: the stored linear value as a 16-bit sRGB code
// (exactly the source code for sRGB-coded 16-bit samples without a colour transform),
// decoded in the image shader like an 8-bit code. Within reference white, like 8-bit tiles.
@fragment
fn fs_convert_codes16(in: VOut) -> @location(0) vec4<u32> {
    let c = clamp(convert(in.pos.xy), vec4<f32>(0.0), vec4<f32>(1.0));
    let l = c.rgb;
    let e = select(1.055 * pow(l, vec3<f32>(1.0 / 2.4)) - 0.055, l * 12.92, l <= vec3<f32>(0.0031308));
    return vec4<u32>(round(vec4<f32>(e, c.a) * 65535.0));
}

fn convert(pos: vec2<f32>) -> vec4<f32> {
    let d = floor(pos);
    let q = clamp(floor(u.dst_origin + d * u.step), vec2<f32>(0.0), u.level_size - 1.0);
    let rp = clamp(q - u.raster_origin, vec2<f32>(0.0), u.raster_size - 1.0);
    let ip = vec2<i32>(rp);
    var rgb: vec3<f32>;
    var a = 1.0;
    switch u.plane_layout {
        case 4u: {
            let c = vec4<f32>(textureLoad(p16, ip, 0)) / 65535.0;
            var l: vec3<f32>;
            switch u.transfer {
                case 1u: { l = c.rgb; }
                case 2u: { l = pq_to_linear(c.rgb); }
                case 3u: { l = hlg_to_linear(c.rgb); }
                default: { l = source(c.rgb); }
            }
            l = to_store(to_bt709(l, u.primaries), 65504.0);
            return vec4<f32>(l, c.a);
        }
        case 8u: {
            let e = clamp(yuv_rgb(rp), vec3<f32>(0.0), vec3<f32>(1.0));
            a = yuv_alpha(ip);
            if u.transfer == 0u && u.primaries == 0u {
                rgb = e;
            } else {
                var l: vec3<f32>;
                switch u.transfer {
                    case 1u: { l = e; }
                    case 2u: { l = pq_to_linear(e); }
                    case 3u: { l = hlg_to_linear(e); }
                    default: { l = srgb_to_linear(e); }
                }
                return vec4<f32>(to_store(to_bt709(l, u.primaries), 65504.0), a);
            }
        }
        case 6u, 7u: {
            // Grey splats to RGB before the source transform (a grey profile's LUT sees
            // equal channels, as for 8-bit grey). Absent channels load as 0, so alpha is
            // explicit.
            let t = textureLoad(p16, ip, 0).rg;
            let l = to_store(source(vec3<f32>(grey(t.r))), 65504.0);
            return vec4<f32>(l, select(1.0, f32(t.g) / 65535.0, u.plane_layout == 7u));
        }
        case 5u: {
            let c = textureLoad(p0, ip, 0);
            let ca = clamp(c.a, 0.0, 1.0);
            let l = to_store(to_bt709(c.rgb, u.primaries), 65504.0);
            return vec4<f32>(l, ca);
        }
        case 0u: {
            rgb = vec3<f32>(textureLoad(p0, ip, 0).r);
        }
        case 1u: {
            let y = textureLoad(p0, ip, 0).r;
            let cb = chroma(p1, rp, u.div1) - 128.0 / 255.0;
            let cr = chroma(p2, rp, u.div2) - 128.0 / 255.0;
            rgb = vec3<f32>(y + 1.402 * cr, y - 0.344136 * cb - 0.714136 * cr, y + 1.772 * cb);
        }
        case 2u: {
            rgb = vec3<f32>(textureLoad(p0, ip, 0).r, chroma(p1, rp, u.div1), chroma(p2, rp, u.div2));
        }
        default: {
            let c = textureLoad(p0, ip, 0);
            rgb = c.rgb;
            a = c.a;
        }
    }
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    // 8-bit storage holds 0..1 (reference white); BT.709 sources stay in gamut unless the
    // storage space is wider.
    let lin = to_store(source(rgb), 1.0);
    return vec4<f32>(lin, a);
}
