// Draws the image: maps each output pixel to stored-image coordinates (orientation included),
// picks the sharpest resident data (detail tiles, then the overview) and resamples it in
// linear light: Lanczos-3 (kernel widened for downscaling) when settled, bilinear while
// moving, nearest-neighbour at high magnification with an optional pixel grid.
//
// Tiles and the overview hold straight (unpremultiplied) colour, so the colour under fully
// transparent pixels survives for the channel views. Every tap is premultiplied here, before
// it is weighted: filtering is done by hand (textureLoad), never by the sampler, which would
// blend straight colour and bleed hidden colour into visible edges.

struct Img {
    // stored = m0.xy · p + m0.z (x), m1 likewise (y); p = output pixel centre.
    m0: vec4<f32>,
    m1: vec4<f32>,
    // W, H, scale (output px per stored px), hq (0 bilinear, 1 Lanczos, 2 Lanczos without
    // the anti-ringing clamp)
    img: vec4<f32>,
    // overview: width, height, mips, present
    ov: vec4<f32>,
    canvas: vec4<f32>,
    grid: vec4<f32>,
    checker_a: vec4<f32>,
    checker_b: vec4<f32>,
    // checker cell size, nearest threshold, grid threshold, tile levels present (bits 0-16)
    // and the coarsest tile level (from bit 17)
    misc: vec4<f32>,
    // clip rect in output pixels (x0, y0, x1, y1)
    clip: vec4<f32>,
    // circular mask (centre x, centre y, radius, enabled): pixels outside are left alone
    mask: vec4<f32>,
    // value ceiling of the stored data (1 SDR, 65504 half-float), content peak, display
    // peak (units of SDR reference white), flags (1 gain map, 2 tone-map, 4 gain map in its
    // own application space)
    hdr: vec4<f32>,
    // gain map per channel: log2 min (w = weight), log2 max, gamma, base / alternate offset
    gm_min: vec4<f32>,
    gm_max: vec4<f32>,
    gm_gamma: vec4<f32>,
    gm_base: vec4<f32>,
    gm_alt: vec4<f32>,
    // Storage space → linear BT.709 rows (identity unless a wide-gamut display profile put
    // the stored data in its own primaries; see convert.wgsl).
    fs0: vec4<f32>,
    fs1: vec4<f32>,
    fs2: vec4<f32>,
    // channel isolation (specular_inspect::ChannelView::shader_mode): 0 off, 1‥3 R/G/B,
    // 4 alpha, 5 luminance; y > 0.5: the overview holds 8-bit sRGB codes (see `ld_ov`);
    // z > 0.5: the detail tiles are 16-bit (`tiles16`, see `ld_tile`)
    chan: vec4<f32>,
    // Gain-map application space (flag 4): linear BT.709 → application primaries rows, and
    // back (ISO 21496-1 Annex B; identity rows otherwise).
    ga0: vec4<f32>,
    ga1: vec4<f32>,
    ga2: vec4<f32>,
    gb0: vec4<f32>,
    gb1: vec4<f32>,
    gb2: vec4<f32>,
    // Channel colours (specular_inspect::ChannelColours): straight linear RGB → display rows;
    // cc0.w > 0.5 turns them on.
    cc0: vec4<f32>,
    cc1: vec4<f32>,
    cc2: vec4<f32>,
    // False colour and HiLo: false colour on (x), map index into `cmaps` (y), HiLo on (z).
    fc: vec4<f32>,
    // HiLo: the colours (linear, straight) of pixels at the maximum and the minimum; w the
    // sRGB-encoded thresholds (at or above hi.w, at or below lo.w).
    hi: vec4<f32>,
    lo: vec4<f32>,
    // Stain separation (specular_render::StainView): unmixing rows, reconstruction rows,
    // tints (sc0.w: OD at full brightness), shown stains (xyz) and mode (w: 0 off, 1 light,
    // 2 OD).
    st0: vec4<f32>,
    st1: vec4<f32>,
    st2: vec4<f32>,
    sr0: vec4<f32>,
    sr1: vec4<f32>,
    sr2: vec4<f32>,
    sc0: vec4<f32>,
    sc1: vec4<f32>,
    sc2: vec4<f32>,
    sv: vec4<f32>,
    // OCIO display/view (specular_render::OcioLut): on (x), log2 of the shaper's ends (y, z),
    // table size (w).
    oc: vec4<f32>,
    // Picture adjustments of a playing movie (specular_render::PictureAdjust): rows on the
    // sRGB-encoded colour (offset in w), then (1 / gamma, on, 0, 0).
    pa0: vec4<f32>,
    pa1: vec4<f32>,
    pa2: vec4<f32>,
    pg: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Img;
@group(0) @binding(1) var tiles: texture_2d_array<f32>;
@group(0) @binding(2) var ovt: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
@group(0) @binding(4) var<storage, read> table: array<u32>;
@group(0) @binding(5) var gmt: texture_2d<f32>;
// The false-colour maps (specular_inspect::ColourMap::gpu_table): 256 sRGB-encoded entries
// per map, maps back to back.
@group(0) @binding(6) var<storage, read> cmaps: array<vec4<f32>>;
// Detail tiles of images with wide samples (u.chan.z > 0.5): 16-bit sRGB codes.
@group(0) @binding(7) var tiles16: texture_2d_array<u32>;
// The sRGB EOTF of the 256 8-bit codes (`srgb_decode8`).
@group(0) @binding(8) var dec8: texture_2d<f32>;
// The OCIO table: display code values over the log2-shaped linear Rec.709 cube, red fastest.
@group(0) @binding(9) var ocio_lut: texture_3d<f32>;

const PI: f32 = 3.14159265;
const TILE: f32 = 512.0;
const BORDER: f32 = 8.0;
const SLOT: i32 = 528;

fn lz(x: f32) -> f32 {
    let ax = abs(x);
    if ax < 1e-4 { return 1.0; }
    if ax >= 3.0 { return 0.0; }
    let px = PI * x;
    return 3.0 * sin(px) * sin(px / 3.0) / (px * px);
}

// The sRGB EOTF of stored codes (16-bit tiles; 8-bit ones go through `srgb_decode8`).
// Tiles and 8-bit overviews are bound through a plain UNORM (or integer) view and decoded
// here, not by the texture unit: its sRGB decode keeps about nine significant bits on some
// GPUs (AMD), a third of a code off after the output encode, so a 1:1 image was not
// bit-exact on an 8-bit display.
fn srgb_decode(c: vec4<f32>) -> vec4<f32> {
    let e = c.rgb;
    let hi = pow((e + 0.055) / 1.055, vec3<f32>(2.4));
    return vec4<f32>(select(hi, e / 12.92, e <= vec3<f32>(0.04045)), c.a);
}

// `srgb_decode` of a stored 8-bit code, looked up in `dec8` (the exact EOTF of the 256
// codes): three texel loads cost less than three pows per tap (a downscaling Lanczos draw
// took 41 % longer than without any decode with pow, 19 % with the table; a degree-7
// polynomial was slower than the table).
fn srgb_decode8(c: vec4<f32>) -> vec4<f32> {
    let i = vec3<i32>(c.rgb * 255.0 + 0.5);
    return vec4<f32>(
        textureLoad(dec8, vec2<i32>(i.r, 0), 0).r,
        textureLoad(dec8, vec2<i32>(i.g, 0), 0).r,
        textureLoad(dec8, vec2<i32>(i.b, 0), 0).r,
        c.a,
    );
}

fn ld_tile(p: vec2<i32>, layer: i32) -> vec4<f32> {
    if u.chan.z > 0.5 {
        return srgb_decode(vec4<f32>(textureLoad(tiles16, p, layer, 0)) / 65535.0);
    }
    return srgb_decode8(textureLoad(tiles, p, layer, 0));
}

fn ld_ov(p: vec2<i32>, m: i32) -> vec4<f32> {
    let c = textureLoad(ovt, p, m);
    if u.chan.y > 0.5 {
        return srgb_decode8(c);
    }
    return c;
}

fn finish(o: vec4<f32>) -> vec4<f32> {
    let a = clamp(o.a, 0.0, 1.0);
    return vec4<f32>(clamp(o.rgb, vec3<f32>(0.0), vec3<f32>(a * u.hdr.x)), a);
}

// The straight colour of the last sample: the same taps and weights, alpha left out. What
// the colour channel views show (the straight channel, resampled), also under fully
// transparent pixels.
var<private> hidden: vec3<f32>;

// A straight texel, premultiplied; in the channel views its colour also goes into `hidden`
// (weight `w`).
fn tap(c: vec4<f32>, w: f32) -> vec4<f32> {
    if u.chan.x > 0.5 {
        hidden += c.rgb * w;
    }
    let aw = c.a * w;
    return vec4<f32>(c.rgb * aw, aw);
}

// Bilinear over a tile layer, `pos` in layer texels (centres at i + 0.5).
fn bilinear_tile(layer: i32, pos: vec2<f32>) -> vec4<f32> {
    let f = pos - 0.5;
    let i = vec2<i32>(floor(f));
    let t = f - floor(f);
    let i0 = clamp(i, vec2<i32>(0), vec2<i32>(SLOT - 1));
    let i1 = clamp(i + 1, vec2<i32>(0), vec2<i32>(SLOT - 1));
    return tap(ld_tile(i0, layer), (1.0 - t.x) * (1.0 - t.y))
        + tap(ld_tile(vec2<i32>(i1.x, i0.y), layer), t.x * (1.0 - t.y))
        + tap(ld_tile(vec2<i32>(i0.x, i1.y), layer), (1.0 - t.x) * t.y)
        + tap(ld_tile(i1, layer), t.x * t.y);
}

// Bilinear over overview level `m`, `uv` normalised.
fn bilinear_ov(m: i32, uv: vec2<f32>, w: f32) -> vec4<f32> {
    let dims = vec2<i32>(textureDimensions(ovt, m));
    let f = uv * vec2<f32>(dims) - 0.5;
    let i = vec2<i32>(floor(f));
    let t = f - floor(f);
    let i0 = clamp(i, vec2<i32>(0), dims - 1);
    let i1 = clamp(i + 1, vec2<i32>(0), dims - 1);
    return tap(ld_ov(i0, m), w * (1.0 - t.x) * (1.0 - t.y))
        + tap(ld_ov(vec2<i32>(i1.x, i0.y), m), w * t.x * (1.0 - t.y))
        + tap(ld_ov(vec2<i32>(i0.x, i1.y), m), w * (1.0 - t.x) * t.y)
        + tap(ld_ov(i1, m), w * t.x * t.y);
}

// The tone-mapping operator (specular_core::hdr::tone_map_scale): a C1 knee at 0.8 × display
// peak, then a Reinhard-style shoulder mapping the content peak onto the display peak.
fn tone_scale(m: f32, content: f32, display: f32) -> f32 {
    let k = 0.8 * display;
    if m <= k || content <= display { return 1.0; }
    let t = m - k;
    let y = min(k + t / (1.0 + t * (1.0 / (display - k) - 1.0 / (content - k))), display);
    return y / m;
}

// Gain map (ISO 21496-1) at the display's weight, then tone mapping. `c` premultiplied,
// `p` in stored pixels.
fn apply_hdr(c: vec4<f32>, p: vec2<f32>) -> vec4<f32> {
    let flags = u32(u.hdr.w);
    if c.a <= 0.0 { return c; }
    var rgb = c.rgb / c.a;
    // The weight is negative for an HDR base shown below its headroom (the map always encodes
    // the SDR → HDR gain).
    if (flags & 1u) != 0u && u.gm_min.w != 0.0 {
        let g = clamp(textureSampleLevel(gmt, samp, p / u.img.xy, 0.0).rgb, vec3<f32>(1e-6), vec3<f32>(1.0));
        let gg = pow(g, 1.0 / u.gm_gamma.xyz);
        let l = mix(u.gm_min.xyz, u.gm_max.xyz, gg);
        let app = (flags & 4u) != 0u;
        if app {
            rgb = vec3<f32>(dot(u.ga0.xyz, rgb), dot(u.ga1.xyz, rgb), dot(u.ga2.xyz, rgb));
        }
        rgb = (rgb + u.gm_base.xyz) * exp2(l * u.gm_min.w) - u.gm_alt.xyz;
        if app {
            rgb = vec3<f32>(dot(u.gb0.xyz, rgb), dot(u.gb1.xyz, rgb), dot(u.gb2.xyz, rgb));
        }
        rgb = max(rgb, vec3<f32>(0.0));
    }
    if (flags & 2u) != 0u {
        rgb *= tone_scale(max(rgb.r, max(rgb.g, rgb.b)), u.hdr.y, u.hdr.z);
    }
    return vec4<f32>(rgb * c.a, c.a);
}

// One channel as opaque grey: of `rgb`, the displayed straight colour (so colour under partial
// or full transparency shows at full strength), or the alpha `a`.
fn channel_grey(rgb: vec3<f32>, a: f32) -> vec4<f32> {
    let m = u32(u.chan.x + 0.5);
    var v = clamp(a, 0.0, 1.0);
    switch m {
        case 1u: { v = rgb.r; }
        case 2u: { v = rgb.g; }
        case 3u: { v = rgb.b; }
        case 5u: { v = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)); }
        default: {}
    }
    return vec4<f32>(vec3<f32>(max(v, 0.0)), 1.0);
}

// The OCIO display/view of a straight linear Rec.709 colour: each channel's log2 placed on the
// table's grid (texel centres at the ends), a trilinear lookup, and the display's code values
// decoded as sRGB so the output stage's encoding gives them back unchanged.
fn ocio(rgb: vec3<f32>) -> vec3<f32> {
    let lo = u.oc.y;
    let hi = u.oc.z;
    let n = u.oc.w;
    let l = log2(max(rgb, vec3<f32>(exp2(lo))));
    let t = clamp((l - lo) / (hi - lo), vec3<f32>(0.0), vec3<f32>(1.0));
    let p = (t * (n - 1.0) + 0.5) / n;
    let code = textureSampleLevel(ocio_lut, samp, p, 0.0).rgb;
    return decode(clamp(code, vec3<f32>(0.0), vec3<f32>(1.0)));
}

fn encode(v: f32) -> f32 {
    let c = clamp(v, 0.0, 1.0);
    if c <= 0.0031308 { return c * 12.92; }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

fn decode(e: vec3<f32>) -> vec3<f32> {
    let lo = e / 12.92;
    let hi = pow((max(e, vec3<f32>(0.0)) + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, e <= vec3<f32>(0.04045));
}

// Map `m` at `t` (0..1): neighbouring entries interpolated in sRGB, then decoded to linear
// (specular_inspect::ColourMap::sample_linear).
fn cmap(m: u32, t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0) * 255.0;
    let i = min(u32(floor(x)), 254u);
    let f = x - f32(i);
    let b = m * 256u + i;
    return decode(mix(cmaps[b].rgb, cmaps[b + 1u].rgb, f));
}

// Channel colours, false colour and HiLo on the straight displayed colour (linear BT.709).
// The grey position of false colour is the sRGB-encoded luminance (or the isolated channel),
// so 8-bit and auto-ranged 16-bit greys map code for code; HiLo marks pixels whose brightest
// channel reaches the maximum code (all channels at the minimum for the low end).
fn inspect_colour(rgb0: vec3<f32>) -> vec3<f32> {
    var rgb = rgb0;
    if u.cc0.w > 0.5 && u.chan.x < 0.5 {
        rgb = vec3<f32>(dot(u.cc0.xyz, rgb), dot(u.cc1.xyz, rgb), dot(u.cc2.xyz, rgb));
    }
    if u.fc.z > 0.5 {
        let top = encode(max(rgb.r, max(rgb.g, rgb.b)));
        if top >= u.hi.w { return u.hi.rgb; }
        if top <= u.lo.w { return u.lo.rgb; }
    }
    if u.fc.x > 0.5 {
        let g = encode(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)));
        rgb = cmap(u32(u.fc.y + 0.5), g);
    }
    return rgb;
}

// Colour deconvolution (Ruifrok and Johnston) of the straight displayed colour (linear
// BT.709): optical densities of its sRGB codes, unmixed into stain concentrations, shown as
// the light the visible stains pass or as their concentrations' brightness. The filter ran
// on linear light first, which is how transmitted light averages, so 1:1 is exact and
// zoomed-out views deconvolve the light a coarser camera pixel would see.
fn stain_colour(rgb: vec3<f32>) -> vec3<f32> {
    let code = vec3<f32>(encode(rgb.r), encode(rgb.g), encode(rgb.b));
    let od = -log2(max(code, vec3<f32>(1e-6))) * 0.30102999566;
    let c = max(vec3<f32>(dot(od, u.st0.xyz), dot(od, u.st1.xyz), dot(od, u.st2.xyz)), vec3<f32>(0.0)) * u.sv.xyz;
    if u.sv.w < 1.5 {
        let o = c.x * u.sr0.xyz + c.y * u.sr1.xyz + c.z * u.sr2.xyz;
        return decode(clamp(exp2(-o * 3.32192809489), vec3<f32>(0.0), vec3<f32>(1.0)));
    }
    let b = min(c / u.sc0.w, vec3<f32>(1.0));
    return decode(clamp(b.x * u.sc0.xyz + b.y * u.sc1.xyz + b.z * u.sc2.xyz, vec3<f32>(0.0), vec3<f32>(1.0)));
}

// Anti-ringing: Lanczos overshoots at hard edges (up to about 10 % of the step, in halos
// two texels wide), and in false colour, HiLo and difference views the overshoot reads as
// data. Each output is clamped to the range of the texels under the kernel's main lobe
// (centres closer than `f` texels on both axes: the nearest 2×2 when magnifying), in the
// space the filter works in: premultiplied colour and alpha, and the straight colour in
// `hidden`. Smooth content stays within that range, so only the overshoot is removed.
var<private> lobe_lo: vec4<f32>;
var<private> lobe_hi: vec4<f32>;
var<private> lobe_hlo: vec3<f32>;
var<private> lobe_hhi: vec3<f32>;

fn lobe_reset() {
    lobe_lo = vec4<f32>(3.0e38);
    lobe_hi = vec4<f32>(-3.0e38);
    lobe_hlo = vec3<f32>(3.0e38);
    lobe_hhi = vec3<f32>(-3.0e38);
}

fn lobe(c: vec4<f32>) {
    let pm = vec4<f32>(c.rgb * c.a, c.a);
    lobe_lo = min(lobe_lo, pm);
    lobe_hi = max(lobe_hi, pm);
    lobe_hlo = min(lobe_hlo, c.rgb);
    lobe_hhi = max(lobe_hhi, c.rgb);
}

// The normalised sum `acc · n` (and `hidden`), clamped to the main-lobe range.
fn antiring(acc: vec4<f32>, n: f32) -> vec4<f32> {
    if lobe_hi.a < lobe_lo.a || u.img.w > 1.5 {
        hidden *= n;
        return finish(acc * n);
    }
    hidden = clamp(hidden * n, lobe_hlo, lobe_hhi);
    return finish(clamp(acc * n, lobe_lo, lobe_hi));
}

// Lanczos-3 over a tile layer. `pos` in layer texels (centres at i + 0.5), `f` ≥ 1 widens the
// kernel for downscaling (radius 3f < 6 ≤ border).
fn lanczos_tile(layer: i32, pos: vec2<f32>, f: f32) -> vec4<f32> {
    let r = 3.0 * f;
    let x0 = i32(floor(pos.x - r + 0.5));
    let x1 = i32(floor(pos.x + r - 0.5));
    let y0 = i32(floor(pos.y - r + 0.5));
    let y1 = i32(floor(pos.y + r - 0.5));
    var wxs: array<f32, 14>;
    var sx = 0.0;
    let nx = min(x1 - x0 + 1, 14);
    for (var i = 0; i < nx; i++) {
        let w = lz((f32(x0 + i) + 0.5 - pos.x) / f);
        wxs[i] = w;
        sx += w;
    }
    var acc = vec4<f32>(0.0);
    var sy = 0.0;
    lobe_reset();
    for (var y = y0; y <= y1; y++) {
        let wy = lz((f32(y) + 0.5 - pos.y) / f);
        if wy == 0.0 { continue; }
        let yy = clamp(y, 0, SLOT - 1);
        var row = vec4<f32>(0.0);
        let ly = abs(f32(y) + 0.5 - pos.y) < f;
        for (var i = 0; i < nx; i++) {
            let c = ld_tile(vec2<i32>(clamp(x0 + i, 0, SLOT - 1), yy), layer);
            row += tap(c, wxs[i] * wy);
            if ly && abs(f32(x0 + i) + 0.5 - pos.x) < f {
                lobe(c);
            }
        }
        acc += row;
        sy += wy;
    }
    return antiring(acc, 1.0 / max(sx * sy, 1e-6));
}

fn lanczos_ov(m: i32, pos: vec2<f32>, f: f32) -> vec4<f32> {
    let dims = vec2<i32>(textureDimensions(ovt, m));
    let r = 3.0 * f;
    let x0 = i32(floor(pos.x - r + 0.5));
    let x1 = i32(floor(pos.x + r - 0.5));
    let y0 = i32(floor(pos.y - r + 0.5));
    let y1 = i32(floor(pos.y + r - 0.5));
    var wxs: array<f32, 14>;
    var sx = 0.0;
    let nx = min(x1 - x0 + 1, 14);
    for (var i = 0; i < nx; i++) {
        let w = lz((f32(x0 + i) + 0.5 - pos.x) / f);
        wxs[i] = w;
        sx += w;
    }
    var acc = vec4<f32>(0.0);
    var sy = 0.0;
    lobe_reset();
    for (var y = y0; y <= y1; y++) {
        let wy = lz((f32(y) + 0.5 - pos.y) / f);
        if wy == 0.0 { continue; }
        let yy = clamp(y, 0, dims.y - 1);
        var row = vec4<f32>(0.0);
        let ly = abs(f32(y) + 0.5 - pos.y) < f;
        for (var i = 0; i < nx; i++) {
            let c = ld_ov(vec2<i32>(clamp(x0 + i, 0, dims.x - 1), yy), m);
            row += tap(c, wxs[i] * wy);
            if ly && abs(f32(x0 + i) + 0.5 - pos.x) < f {
                lobe(c);
            }
        }
        acc += row;
        sy += wy;
    }
    return antiring(acc, 1.0 / max(sx * sy, 1e-6));
}

// Tile slot (layer + 1) or 0.
fn lookup(level: u32, t: vec2<i32>) -> u32 {
    let cols = i32(table[level * 4u]);
    let rows = i32(table[level * 4u + 1u]);
    if t.x < 0 || t.y < 0 || t.x >= cols || t.y >= rows { return 0u; }
    return table[table[level * 4u + 2u] + u32(t.y * cols + t.x)];
}

// Stored colour → linear BT.709. Linear, so it commutes with filtering (and premultiplying).
fn to_working(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(dot(u.fs0.xyz, c.rgb), dot(u.fs1.xyz, c.rgb), dot(u.fs2.xyz, c.rgb), c.a);
}

// The image at stored pixel `p` in linear BT.709 (premultiplied). In the channel views
// `hidden` is left holding the straight colour, in the storage space.
fn sample_image(p: vec2<f32>, s: f32, hq: bool) -> vec4<f32> {
    return to_working(sample_stored(p, s, hq));
}

fn sample_stored(p: vec2<f32>, s: f32, hq: bool) -> vec4<f32> {
    hidden = vec3<f32>(0.0);
    let nearest = s >= u.misc.y;
    // Detail tiles, finest useful level first.
    let ov_scale = select(0.0, u.ov.x / u.img.x, u.ov.w > 0.0);
    var want = 0u;
    let packed = u32(u.misc.w);
    let present = packed & 0x1FFFFu;
    let max_level = packed >> 17u;
    if s < 1.0 { want = u32(clamp(floor(log2(1.0 / s)), 0.0, f32(max_level))); }
    for (var l = want; l <= max_level; l++) {
        let lscale = 1.0 / f32(1u << l);
        if lscale <= ov_scale * 1.001 { break; }
        if (present & (1u << l)) == 0u { continue; }
        let lp = p * lscale;
        let t = vec2<i32>(floor(lp / TILE));
        let slot = lookup(l, t);
        if slot == 0u { continue; }
        let layer = i32(slot - 1u);
        let local = lp - vec2<f32>(t) * TILE + BORDER;
        let r = s / lscale; // output px per level px
        if nearest {
            return tap(ld_tile(vec2<i32>(floor(local)), layer), 1.0);
        }
        if !hq {
            return bilinear_tile(layer, local);
        }
        return lanczos_tile(layer, local, max(1.0, 1.0 / r));
    }
    if u.ov.w == 0.0 { return vec4<f32>(0.0); }
    let ovp = p * vec2<f32>(u.ov.x / u.img.x, u.ov.y / u.img.y);
    let ro = s * u.img.x / u.ov.x; // output px per overview texel
    // Compared without the division: GPU division is inexact (4 · 61 / 61 → 3.9999998), which
    // kept the threshold zoom itself off nearest neighbour.
    if nearest && s * u.img.x >= u.misc.y * u.ov.x {
        return tap(ld_ov(clamp(vec2<i32>(floor(ovp)), vec2<i32>(0), vec2<i32>(u.ov.xy) - 1), 0), 1.0);
    }
    let maxm = u.ov.z - 1.0;
    if !hq {
        // Trilinear while moving.
        let lod = clamp(log2(1.0 / ro), 0.0, maxm);
        let m0 = floor(lod);
        let t = lod - m0;
        let uv = ovp / u.ov.xy;
        var c = bilinear_ov(i32(m0), uv, 1.0 - t);
        if t > 0.0 {
            c += bilinear_ov(i32(m0) + 1, uv, t);
        }
        return c;
    }
    var m = 0;
    if ro < 1.0 { m = i32(clamp(floor(log2(1.0 / ro)), 0.0, maxm)); }
    var md = vec2<f32>(textureDimensions(ovt, m));
    // Mip sizes are floored, so a level can be slightly coarser than the output; resampling it
    // at more than one output pixel per texel moves near-Nyquist detail by up to a third of a
    // pixel. Take the finer level with a widened kernel instead.
    if m > 0 && ro * max(u.ov.x / md.x, u.ov.y / md.y) > 1.0 {
        m -= 1;
        md = vec2<f32>(textureDimensions(ovt, m));
    }
    let mp = ovp * md / u.ov.xy;
    let rm = ro * min(u.ov.x / md.x, u.ov.y / md.y);
    return lanczos_ov(m, mp, clamp(1.0 / rm, 1.0, 2.0));
}

fn srgb_enc_signed(x: f32) -> f32 {
    let a = abs(x);
    let e = select(1.055 * pow(a, 1.0 / 2.4) - 0.055, a * 12.92, a <= 0.0031308);
    return sign(x) * e;
}

fn srgb_dec_signed(x: f32) -> f32 {
    let a = abs(x);
    let d = select(pow((a + 0.055) / 1.055, 2.4), a / 12.92, a <= 0.04045);
    return sign(x) * d;
}

fn gamma_clamped(x: f32) -> f32 {
    return pow(max(x, 0.0), u.pg.x);
}

// Picture adjustments (PictureAdjust::apply) of straight linear colour, done on its sRGB
// encoding.
fn picture(lin: vec3<f32>) -> vec3<f32> {
    let e = vec4<f32>(srgb_enc_signed(lin.r), srgb_enc_signed(lin.g), srgb_enc_signed(lin.b), 1.0);
    let a = vec3<f32>(dot(u.pa0, e), dot(u.pa1, e), dot(u.pa2, e));
    return vec3<f32>(
        srgb_dec_signed(gamma_clamped(a.r)),
        srgb_dec_signed(gamma_clamped(a.g)),
        srgb_dec_signed(gamma_clamped(a.b)),
    );
}

@fragment
fn fs_image(in: VOut) -> @location(0) vec4<f32> {
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
    // Coverage of this output pixel by the image (axes stay axis-aligned under orientation).
    let cx = clamp(min(p.x, W - p.x) * s + 0.5, 0.0, 1.0);
    let cy = clamp(min(p.y, H - p.y) * s + 0.5, 0.0, 1.0);
    let cov = cx * cy;
    if cov <= 0.0 { return u.canvas; }
    let pc = clamp(p, vec2<f32>(0.0), vec2<f32>(W, H) - 0.001);
    var c = sample_image(pc, s, u.img.w > 0.5);
    if u.chan.x > 0.5 {
        var st = to_working(vec4<f32>(clamp(hidden, vec3<f32>(0.0), vec3<f32>(u.hdr.x)), 1.0));
        if u.hdr.w > 0.0 {
            st = apply_hdr(st, pc);
        }
        c = channel_grey(st.rgb, c.a);
    } else if u.oc.x > 0.5 {
        if c.a > 0.0 {
            c = vec4<f32>(ocio(c.rgb / c.a) * c.a, c.a);
        }
    } else if u.hdr.w > 0.0 {
        c = apply_hdr(c, pc);
    }
    if u.pg.y > 0.5 && u.chan.x < 0.5 && c.a > 0.0 {
        c = vec4<f32>(picture(c.rgb / c.a) * c.a, c.a);
    }
    if u.sv.w > 0.5 && u.chan.x < 0.5 && c.a > 0.0 {
        c = vec4<f32>(stain_colour(c.rgb / c.a) * c.a, c.a);
    }
    if (u.cc0.w > 0.5 || u.fc.x > 0.5 || u.fc.z > 0.5) && c.a > 0.0 {
        c = vec4<f32>(inspect_colour(c.rgb / c.a) * c.a, c.a);
    }
    // Transparency over a checkerboard (screen-space cells).
    if c.a < 0.999 {
        let cell = floor(px / u.misc.x);
        let odd = (i32(cell.x) + i32(cell.y)) & 1;
        let bg = select(u.checker_a, u.checker_b, odd == 1);
        c = c + bg * (1.0 - c.a);
    }
    if u.grid.a > 0.0 && s >= u.misc.z {
        let f = fract(p);
        let d = min(min(f, 1.0 - f).x, min(f, 1.0 - f).y) * s;
        let g = clamp(1.0 - d, 0.0, 1.0) * u.grid.a;
        c = vec4<f32>(mix(c.rgb, u.grid.rgb / max(u.grid.a, 1e-4), g), c.a);
    }
    return c * cov + u.canvas * (1.0 - cov);
}
