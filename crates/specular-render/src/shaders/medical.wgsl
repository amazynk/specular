// Medical images: draws grey modality values (an R32Float texture with box-filtered mips)
// through a VOI window (DICOM PS3.3 C.11.2.1.2) on the GPU, so window/level changes cost one
// redraw and never a re-decode. Interpolation happens on the values, before the window, as
// medical viewers do: a narrow window then shows a smooth ramp instead of interpolated
// display greys. Values are filtered by hand (textureLoad): R32Float is not filterable.
//
// Same geometry as image.wgsl (orientation matrix, coverage at the image edges, clip, the
// loupe's circular mask, the pixel grid).

struct Med {
    // stored = m0.xy · p + m0.z (x), m1 likewise (y); p = output pixel centre.
    m0: vec4<f32>,
    m1: vec4<f32>,
    // W, H (stored size), scale (output px per stored px), unused
    img: vec4<f32>,
    // values texture: width, height, mips, unused
    tex: vec4<f32>,
    canvas: vec4<f32>,
    grid: vec4<f32>,
    // has a shutter / overlay mask, nearest-neighbour threshold, grid threshold, unused
    misc: vec4<f32>,
    // clip rect in output pixels (x0, y0, x1, y1)
    clip: vec4<f32>,
    // circular mask (centre x, centre y, radius, enabled)
    mask: vec4<f32>,
    // LINEAR / LINEAR_EXACT: centre c', 1 / w' (0 when w' = 0), lower edge c' − w'/2,
    // upper edge c' + w'/2. SIGMOID: centre, 1 / width.
    voi: vec4<f32>,
    // function (0 linear, 1 sigmoid), inverted (MONOCHROME1 or the user's invert), overlays
    // shown, unused
    mode: vec4<f32>,
    // The shutter's colour (Shutter Presentation Value or Color), display-coded rgb.
    shutter: vec4<f32>,
    // Overlay inks 1..=15 (the layers' colours), display-coded rgb.
    inks: array<vec4<f32>, 15>,
    // Science images (mode.w = 1): stretch (0 linear, 1 sqrt, 2 log, 3 asinh, 4 histogram
    // equalisation, 5 midtones transfer), channel count, unused, unused.
    sci: vec4<f32>,
    // Per channel (four per vector, as the values' layers): the lower limit and
    // 1 / (upper − lower) (0 when the limits meet).
    lo: array<vec4<f32>, 4>,
    inv: array<vec4<f32>, 4>,
    // Per channel: its display colour (rgb) and whether it is shown (a).
    cols: array<vec4<f32>, 16>,
    // False colour on (x), map index (y), HiLo on (z).
    fc: vec4<f32>,
    // HiLo: values at or above x are at the bit depth's maximum, at or below y its minimum.
    hl: vec4<f32>,
    // HiLo colours, display-coded rgb.
    hilo_hi: vec4<f32>,
    hilo_lo: vec4<f32>,
    // Science, automatic stretch (5): per channel (four per vector), the midtones balance.
    mid: array<vec4<f32>, 4>,
    // Relief (one channel): off (0), hillshade (1), slope (2) or aspect (3) in x; the
    // direction towards the sun (east, north, up: yzw).
    hs: vec4<f32>,
    // Relief: the slope factors per stored pixel, 1 / (8 × ground step) × exaggeration,
    // along a row (x) and down a column (y, the step northward); the slope's (z) and the
    // aspect's (w) colour maps.
    hsr: vec4<f32>,
    // Isotherm (one channel): on (x), above (y, else below), the level (z, in values).
    iso: vec4<f32>,
    // The isotherm's colour, display-coded.
    iso_col: vec4<f32>,
    // DICOM GSDF display (PS3.14): JND index of the display's black (x) and white (y), its
    // black in cd/m² (z) and 1 / (white − black) (w); y = 0 shows greys as sRGB.
    gsdf: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Med;
// The values: one layer (R32Float), or four channels per layer (Rgba32Float) for science
// images of 2 to 16 channels.
@group(0) @binding(1) var vt: texture_2d_array<f32>;
// Display shutters (bit 0) and the ink of the overlay on top (bits 1..=4, 0 none) at the
// values' size, R8Unorm.
@group(0) @binding(2) var mt: texture_2d<f32>;
// Science: the histogram-equalisation tables, one row of HISTEQ_BINS per channel, R32Float.
@group(0) @binding(3) var lut: texture_2d<f32>;
// The false-colour maps (specular_inspect::ColourMap::gpu_table), sRGB-encoded.
@group(0) @binding(4) var<storage, read> cmaps: array<vec4<f32>>;

// Map `m` at `t` (0..1), display-coded: neighbouring entries interpolated in sRGB, as
// specular_inspect::ColourMap::sample.
fn cmap(m: u32, t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0) * 255.0;
    let i = min(u32(floor(x)), 254u);
    let f = x - f32(i);
    let b = m * 256u + i;
    return mix(cmaps[b].rgb, cmaps[b + 1u].rgb, f);
}

// Values are R32Float (one channel) or Rgba32Float (science images of 2–16 channels, four
// per layer `l`).
fn texel(m: i32, i: vec2<i32>, dims: vec2<i32>, l: i32) -> vec4<f32> {
    return textureLoad(vt, clamp(i, vec2<i32>(0), dims - 1), l, m);
}

// Per component: 1 where a value is a number, 0 where it is NaN or infinite (no data: a
// GeoTIFF's no-data value, a FITS blank).
fn valid(v: vec4<f32>) -> vec4<f32> {
    let bits = bitcast<vec4<u32>>(v) & vec4<u32>(0x7fffffffu);
    return select(vec4<f32>(0.0), vec4<f32>(1.0), bits < vec4<u32>(0x7f800000u));
}

// A filtered value: the weighted sum of the valid texels under the filter (`v`) and their
// weight (`w`), per component. No-data never mixes into its neighbours.
struct Smp {
    v: vec4<f32>,
    w: vec4<f32>,
};

fn smp(x: vec4<f32>, k: f32) -> Smp {
    let ok = valid(x);
    return Smp(select(vec4<f32>(0.0), x, ok > vec4<f32>(0.5)) * k, ok * k);
}

fn add(a: Smp, b: Smp) -> Smp {
    return Smp(a.v + b.v, a.w + b.w);
}

// Bilinear over level `m` of layer `l`, `uv` normalised.
fn bilinear(m: i32, uv: vec2<f32>, l: i32) -> Smp {
    let dims = vec2<i32>(textureDimensions(vt, m));
    let f = uv * vec2<f32>(dims) - 0.5;
    let i = vec2<i32>(floor(f));
    let t = f - floor(f);
    let a = add(
        smp(texel(m, i, dims, l), (1.0 - t.x) * (1.0 - t.y)),
        smp(texel(m, i + vec2<i32>(1, 0), dims, l), t.x * (1.0 - t.y)),
    );
    let b = add(
        smp(texel(m, i + vec2<i32>(0, 1), dims, l), (1.0 - t.x) * t.y),
        smp(texel(m, i + vec2<i32>(1, 1), dims, l), t.x * t.y),
    );
    return add(a, b);
}

// The modality value at stored pixel `p` for scale `s`: nearest-neighbour at high zoom,
// bilinear down to 1:1, trilinear over the box-filtered mips below. `w` is the share of the
// filter on valid values (0 at no-data). Layer `l` of the values.
fn value(p: vec2<f32>, s: f32, l: i32) -> Smp {
    let tp = p * u.tex.xy / u.img.xy;
    let r = s * u.img.x / u.tex.x; // output px per level-0 texel
    let dims = vec2<i32>(u.tex.xy);
    if s * u.img.x >= u.misc.y * u.tex.x { // not r: GPU division is inexact at the threshold
        return smp(texel(0, vec2<i32>(floor(tp)), dims, l), 1.0);
    }
    let uv = tp / u.tex.xy;
    var a: Smp;
    if r >= 1.0 {
        a = bilinear(0, uv, l);
    } else {
        let lod = clamp(log2(1.0 / r), 0.0, u.tex.z - 1.0);
        let m0 = floor(lod);
        let t = lod - m0;
        a = bilinear(i32(m0), uv, l);
        if t > 0.0 {
            let b = bilinear(i32(m0) + 1, uv, l);
            a = Smp(a.v * (1.0 - t) + b.v * t, a.w * (1.0 - t) + b.w * t);
        }
    }
    return Smp(a.v / max(a.w, vec4<f32>(1e-6)), a.w);
}

// The VOI output in 0..1 (PS3.3 C.11.2.1.2.1 and .2; LINEAR_EXACT arrives with c' = c and
// w' = w).
fn voi(x: f32) -> f32 {
    if u.mode.x > 0.5 {
        return 1.0 / (1.0 + exp(-4.0 * (x - u.voi.x) * u.voi.y));
    }
    if x <= u.voi.z { return 0.0; }
    if x > u.voi.w || u.voi.y == 0.0 { return 1.0; }
    return clamp((x - u.voi.x) * u.voi.y + 0.5, 0.0, 1.0);
}

// The window's output is a display grey coded like an sRGB value (as the CPU path's Gray16
// raster is), decoded here to the scene's linear light.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { return c / 12.92; }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn linear_to_srgb(l: f32) -> f32 {
    let c = clamp(l, 0.0, 1.0);
    if c <= 0.0031308 { return c * 12.92; }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

// P-value `p` (0..1) on a display calibrated to the Grayscale Standard Display Function
// (specular_dicom::gsdf::p_luminance): equal JND steps from the display's black to its white,
// as the display-coded grey that shows that luminance (relative luminance, sRGB-coded).
fn gsdf(p: f32) -> f32 {
    let j = mix(u.gsdf.x, u.gsdf.y, clamp(p, 0.0, 1.0));
    let x = log(clamp(j, 1.0, 1023.0));
    // PS3.14 B.1, equation (1).
    let num = -1.3011877 + x * (8.0242636e-2 + x * (1.3646699e-1 + x * (-2.5468404e-2 + x * 1.3635334e-3)));
    let den = 1.0 + x * (-2.5840191e-2 + x * (-1.0320229e-1 + x * (2.8745620e-2 + x * (-3.1978977e-3 + x * 1.2992634e-4))));
    let l = exp2(num / den * 3.32192809);
    return linear_to_srgb((l - u.gsdf.z) * u.gsdf.w);
}

@fragment
fn fs_medical(in: VOut) -> @location(0) vec4<f32> {
    let px = in.pos.xy;
    if u.mask.w > 0.5 && distance(px, u.mask.xy) > u.mask.z {
        discard;
    }
    if px.x < u.clip.x || px.y < u.clip.y || px.x >= u.clip.z || px.y >= u.clip.w {
        return u.canvas;
    }
    let p = vec2<f32>(dot(u.m0.xy, px) + u.m0.z, dot(u.m1.xy, px) + u.m1.z);
    let s = u.img.z;
    let W = u.img.x;
    let H = u.img.y;
    let cx = clamp(min(p.x, W - p.x) * s + 0.5, 0.0, 1.0);
    let cy = clamp(min(p.y, H - p.y) * s + 0.5, 0.0, 1.0);
    let cov = cx * cy;
    if cov <= 0.0 { return u.canvas; }
    let pc = clamp(p, vec2<f32>(0.0), vec2<f32>(W, H) - 0.001);
    let sv = value(pc, s, 0);
    let v = sv.v;
    if u.mode.w > 0.5 {
        // A channel with under half its filter on values has no data there and adds nothing;
        // where no shown channel has data, the canvas shows.
        let sc = science(pc, s, sv);
        if sc.a < 0.5 {
            return u.canvas;
        }
        var rgb = sc.rgb;
        if u.hs.x > 0.5 {
            // The shade alone, or darkening the false colours of a terrain map; the slope
            // or aspect in their own colours.
            let rc = relief(pc, s);
            if rc.a > 0.5 {
                if u.hs.x < 1.5 {
                    rgb = select(rc.rgb, rgb * rc.r, u.fc.x > 0.5);
                } else {
                    rgb = rc.rgb;
                }
            }
        }
        if u.iso.x > 0.5 && u.cols[0].a > 0.5 && sv.w.x >= 0.5 {
            let x = sv.v.x;
            if (u.iso.y > 0.5 && x >= u.iso.z) || (u.iso.y < 0.5 && x <= u.iso.z) {
                rgb = u.iso_col.rgb;
            }
        }
        return finish(rgb, p, s, cov);
    }
    let x = v.x;
    var g = voi(x);
    if u.mode.y > 0.5 {
        g = 1.0 - g;
    }
    // False colour maps the displayed grey (after the window and inversion); HiLo marks
    // the stored range's ends on top of it. Greys are P-values: on a GSDF display they go
    // through its JND steps.
    var rgb = vec3<f32>(g);
    if u.fc.x > 0.5 {
        rgb = cmap(u32(u.fc.y + 0.5), g);
    } else if u.gsdf.y > 0.0 {
        rgb = vec3<f32>(gsdf(g));
    }
    if u.fc.z > 0.5 {
        if x >= u.hl.x {
            rgb = u.hilo_hi.rgb;
        } else if x <= u.hl.y {
            rgb = u.hilo_lo.rgb;
        }
    }
    // Shutters and overlays are presentation, drawn after the window and the inversion
    // (PS3.4 N.2): the shutter in its stated grey or colour, overlays in their layer's ink
    // on top.
    if u.misc.x > 0.5 {
        let md = vec2<i32>(textureDimensions(mt));
        let mi = clamp(vec2<i32>(floor(pc * vec2<f32>(md) / u.img.xy)), vec2<i32>(0), md - 1);
        let bits = u32(round(textureLoad(mt, mi, 0).r * 255.0));
        let ink = (bits >> 1u) & 15u;
        if ink != 0u && u.mode.z > 0.5 {
            rgb = u.inks[ink - 1u].rgb;
        } else if (bits & 1u) != 0u {
            rgb = u.shutter.rgb;
        }
    }
    return finish(rgb, p, s, cov);
}

// The stretch of a normalised value (specular_core::nd::apply).
fn stretch(x: f32, k: i32) -> f32 {
    let f = i32(u.sci.x + 0.5);
    if f == 1 { return sqrt(x); }
    if f == 2 { return log(1.0 + 1000.0 * x) / log(1001.0); }
    if f == 3 {
        let a = x / 0.1;
        return log(a + sqrt(a * a + 1.0)) / log(10.0 + sqrt(101.0));
    }
    if f == 4 {
        let n = i32(textureDimensions(lut).x);
        return textureLoad(lut, vec2<i32>(min(i32(x * f32(n)), n - 1), k), 0).r;
    }
    if f == 5 {
        // The midtones transfer function (specular_core::nd::mtf).
        let m = u.mid[k / 4][k % 4];
        if x <= 0.0 { return 0.0; }
        if x >= 1.0 { return 1.0; }
        let d = (2.0 * m - 1.0) * x - m;
        if d == 0.0 { return 0.5; }
        return clamp((m - 1.0) * x / d, 0.0, 1.0);
    }
    return x;
}

// Science images: each shown channel between its limits, stretched, in its colour, added
// (rgb), and whether any shown channel has data here (a). One channel with false colour on
// goes through the map instead (an elevation model's terrain). `first` is layer 0's value at
// `pc`; further layers (channels 5 to 16) are filtered here.
fn science(pc: vec2<f32>, s: f32, first: Smp) -> vec4<f32> {
    let n = i32(u.sci.y + 0.5);
    if n == 1 && u.fc.x > 0.5 && u.cols[0].a > 0.5 {
        if first.w.x < 0.5 {
            return vec4<f32>(0.0);
        }
        let x = clamp((first.v.x - u.lo[0].x) * u.inv[0].x, 0.0, 1.0);
        return vec4<f32>(cmap(u32(u.fc.y + 0.5), stretch(x, 0)), 1.0);
    }
    var rgb = vec3<f32>(0.0);
    var any = 0.0;
    let layers = (n + 3) / 4;
    for (var l = 0; l < 4; l++) {
        if l >= layers {
            break;
        }
        var sv = first;
        if l > 0 {
            sv = value(pc, s, l);
        }
        for (var j = 0; j < 4; j++) {
            let k = l * 4 + j;
            if k < n && u.cols[k].a > 0.5 && sv.w[j] >= 0.5 {
                any = 1.0;
                let x = clamp((sv.v[j] - u.lo[l][j]) * u.inv[l][j], 0.0, 1.0);
                rgb += u.cols[k].rgb * stretch(x, k);
            }
        }
    }
    return vec4<f32>(min(rgb, vec3<f32>(1.0)), any);
}

// A neighbour's elevation for the slope: the centre's where it has no data.
fn elev(m: i32, i: vec2<i32>, dims: vec2<i32>, c: f32) -> f32 {
    let x = texel(m, i, dims, 0).x;
    return select(c, x, valid(vec4<f32>(x)).x > 0.5);
}

// The gradient of texel `i` of level `m` (Hillshade::gradient): height per ground unit
// east and north from the 3×3 neighbours with Horn's weights; `ok` false where the texel
// has no data.
struct Grad {
    g: vec2<f32>,
    ok: bool,
};

fn grad_at(m: i32, i: vec2<i32>) -> Grad {
    let dims = vec2<i32>(textureDimensions(vt, m));
    let c = texel(m, i, dims, 0).x;
    if valid(vec4<f32>(c)).x < 0.5 {
        return Grad(vec2<f32>(0.0), false);
    }
    let a = elev(m, i + vec2<i32>(-1, -1), dims, c);
    let b = elev(m, i + vec2<i32>(0, -1), dims, c);
    let e = elev(m, i + vec2<i32>(1, -1), dims, c);
    let d = elev(m, i + vec2<i32>(-1, 0), dims, c);
    let f = elev(m, i + vec2<i32>(1, 0), dims, c);
    let g = elev(m, i + vec2<i32>(-1, 1), dims, c);
    let h = elev(m, i + vec2<i32>(0, 1), dims, c);
    let k = elev(m, i + vec2<i32>(1, 1), dims, c);
    // Stored pixels per texel of this level: the slope is per stored pixel's ground step.
    let per = u.img.xy / vec2<f32>(dims);
    let gx = ((e + 2.0 * f + k) - (a + 2.0 * d + g)) * u.hsr.x / per.x;
    let gy = ((g + 2.0 * h + k) - (a + 2.0 * b + e)) * u.hsr.y / per.y;
    return Grad(vec2<f32>(gx, gy), true);
}

// The relief colour of texel `i` of level `m` (Hillshade::colour): the shade's grey (1),
// the slope through its map (2), or the aspect through its cyclic map (3); a = 0 where the
// texel has no data.
fn relief_at(m: i32, i: vec2<i32>) -> vec4<f32> {
    let gr = grad_at(m, i);
    if !gr.ok {
        return vec4<f32>(0.0);
    }
    let kind = i32(u.hs.x + 0.5);
    if kind == 2 {
        let deg = degrees(atan(length(gr.g)));
        // Gentle ground light, steep ground dark (the map runs dark to light).
        return vec4<f32>(cmap(u32(u.hsr.z + 0.5), 1.0 - deg / 60.0), 1.0);
    }
    if kind == 3 {
        if gr.g.x == 0.0 && gr.g.y == 0.0 {
            return vec4<f32>(vec3<f32>(0.6), 1.0);
        }
        var a = degrees(atan2(-gr.g.x, -gr.g.y));
        a = a - 360.0 * floor(a / 360.0);
        return vec4<f32>(cmap(u32(u.hsr.w + 0.5), a / 360.0), 1.0);
    }
    let n = vec3<f32>(-gr.g.x, -gr.g.y, 1.0);
    return vec4<f32>(vec3<f32>(max(dot(n, u.hs.yzw) / length(n), 0.0)), 1.0);
}

// The relief at stored position `p` for scale `s`, on the mip level the values are drawn
// from: the nearest texel's at high zoom, else the colours of the four around blended
// bilinearly (no-data texels left out); a = 0 where none has data.
fn relief(p: vec2<f32>, s: f32) -> vec4<f32> {
    let r = s * u.img.x / u.tex.x;
    var m = 0;
    if r < 1.0 {
        m = i32(clamp(round(log2(1.0 / r)), 0.0, u.tex.z - 1.0));
    }
    let dims = vec2<f32>(textureDimensions(vt, m));
    let tp = p / u.img.xy * dims;
    if s * u.img.x >= u.misc.y * u.tex.x {
        return relief_at(m, vec2<i32>(floor(tp)));
    }
    let fp = tp - 0.5;
    let i = vec2<i32>(floor(fp));
    let t = fp - floor(fp);
    let last = vec2<i32>(dims) - 1;
    var sum = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var j = 0; j < 4; j++) {
        let o = vec2<i32>(j & 1, j >> 1u);
        let wx = select(1.0 - t.x, t.x, o.x == 1);
        let wy = select(1.0 - t.y, t.y, o.y == 1);
        let c = relief_at(m, clamp(i + o, vec2<i32>(0), last));
        if c.a > 0.5 {
            sum += c.rgb * wx * wy;
            wsum += wx * wy;
        }
    }
    if wsum <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(sum / wsum, 1.0);
}

// Display-coded rgb to the scene, with the pixel grid and edge coverage.
fn finish(rgb: vec3<f32>, p: vec2<f32>, s: f32, cov: f32) -> vec4<f32> {
    var c = vec4<f32>(
        srgb_to_linear(rgb.r), srgb_to_linear(rgb.g), srgb_to_linear(rgb.b), 1.0
    );
    if u.grid.a > 0.0 && s >= u.misc.z {
        let f = fract(p);
        let d = min(min(f, 1.0 - f).x, min(f, 1.0 - f).y) * s;
        let k = clamp(1.0 - d, 0.0, 1.0) * u.grid.a;
        c = vec4<f32>(mix(c.rgb, u.grid.rgb / max(u.grid.a, 1e-4), k), 1.0);
    }
    return c * cov + u.canvas * (1.0 - cov);
}

// One mip level of the values: the mean of the valid ones of the 2×2 texels below, NaN where
// none is. Levels round up, so the last column / row of an odd level is averaged with itself
// (by the clamp).
@fragment
fn fs_values_mip(in: VOut) -> @location(0) vec4<f32> {
    let o = vec2<i32>(floor(in.pos.xy)) * 2;
    // The view is the level below, one layer.
    let dims = vec2<i32>(textureDimensions(vt, 0));
    let a = add(
        add(smp(texel(0, o, dims, 0), 1.0), smp(texel(0, o + vec2<i32>(1, 0), dims, 0), 1.0)),
        add(
            smp(texel(0, o + vec2<i32>(0, 1), dims, 0), 1.0),
            smp(texel(0, o + vec2<i32>(1, 1), dims, 0), 1.0),
        ),
    );
    // A quiet NaN from a runtime value (a constant NaN is refused), for no-data.
    let nan = vec4<f32>(bitcast<f32>(0x7fc00000u | (u32(in.pos.x) & 0u)));
    return select(nan, a.v / max(a.w, vec4<f32>(1e-6)), a.w > vec4<f32>(0.0));
}
