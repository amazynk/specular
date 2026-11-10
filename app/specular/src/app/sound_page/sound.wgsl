// The sound page's time-based visuals, one panel per draw (the viewport is the panel):
// waveform lanes, the spectrogram and the loudness history, from the whole-file analysis in
// textures, for the part of the sound in view. Colours come in linear; the spectrogram's
// ramp is fixed (data colours do not follow the theme).

struct U {
    // kind (0 waveform, 1 spectrogram, 2 history), columns, lanes, loudness steps
    a: vec4<f32>,
    // view start, view span, played to (fraction; < 0 none), texel rows of the spectrogram
    view: vec4<f32>,
    // panel width and height (px), steps for the whole sound, lane gap (px)
    px: vec4<f32>,
    bg: vec4<f32>,
    axis: vec4<f32>,
    wave: vec4<f32>,
    rms: vec4<f32>,
    played: vec4<f32>,
    played_rms: vec4<f32>,
    // history: lowest and highest LUFS shown, integrated loudness (< -150 none)
    hist: vec4<f32>,
    // the integrated loudness line (the theme's text colour)
    mark: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
// Per column and lane: lowest, highest, RMS; texel i = column * lanes + lane, 4096 a row.
@group(0) @binding(1) var wave_tex: texture_2d<f32>;
// Spectrogram levels 0…1: strips of 256 bins wide, `view.w` columns high.
@group(0) @binding(2) var spec_tex: texture_2d<f32>;
// Per 100 ms step: momentary and short-term LUFS; 4096 a row.
@group(0) @binding(3) var hist_tex: texture_2d<f32>;

const ROW: u32 = 4096u;
const BINS: u32 = 256u;
// Columns read at most for one pixel (more are strided).
const MAX_READS: u32 = 64u;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> VOut {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var o: VOut;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// Silence to full scale: black, violet, crimson, orange, pale yellow (as the picture).
fn ramp(v: f32) -> vec3<f32> {
    var stops = array<vec3<f32>, 5>(
        vec3<f32>(2.0, 2.0, 8.0),
        vec3<f32>(70.0, 20.0, 110.0),
        vec3<f32>(185.0, 45.0, 85.0),
        vec3<f32>(245.0, 135.0, 40.0),
        vec3<f32>(252.0, 248.0, 185.0),
    );
    let x = clamp(v, 0.0, 1.0) * 4.0;
    let i = min(u32(x), 3u);
    let t = x - f32(i);
    let c = mix(stops[i], stops[i + 1u], t) / 255.0;
    return srgb_to_linear(c);
}

// The columns (first, past the last) under pixel column `px` of `w`, of `cols`.
fn columns(px: f32, w: f32, cols: f32) -> vec2<u32> {
    let f0 = u.view.x + px / w * u.view.y;
    let f1 = u.view.x + (px + 1.0) / w * u.view.y;
    let c0 = clamp(floor(f0 * cols), 0.0, cols - 1.0);
    let c1 = clamp(max(ceil(f1 * cols), c0 + 1.0), c0 + 1.0, cols);
    return vec2<u32>(u32(c0), u32(c1));
}

fn row_load(i: u32) -> vec2<i32> {
    return vec2<i32>(i32(i % ROW), i32(i / ROW));
}

fn waveform(uv: vec2<f32>) -> vec4<f32> {
    let w = u.px.x;
    let h = u.px.y;
    let lanes = u.a.z;
    let gap = u.px.w;
    let lane_h = (h - gap * (lanes - 1.0)) / lanes;
    let y = uv.y * h;
    let lane = min(floor(y / (lane_h + gap)), lanes - 1.0);
    let within = y - lane * (lane_h + gap);
    if within > lane_h {
        return vec4<f32>(0.0);
    }
    let half_h = max(lane_h * 0.5, 1.0);
    // Level at this pixel's top and bottom edge (positive up).
    let v_top = (half_h - within) / half_h;
    let v_bot = (half_h - (within + 1.0)) / half_h;
    let px = floor(uv.x * w);
    let cr = columns(px, w, u.a.y);
    let step = max(1u, (cr.y - cr.x + MAX_READS - 1u) / MAX_READS);
    var lo = 0.0;
    var hi = 0.0;
    var r = 0.0;
    for (var c = cr.x; c < cr.y; c += step) {
        let s = textureLoad(wave_tex, row_load(c * u32(lanes) + u32(lane)), 0);
        lo = min(lo, s.x);
        hi = max(hi, s.y);
        r = max(r, s.z);
    }
    let centre = (u.view.x + (px + 0.5) / w * u.view.y);
    let played = u.view.z >= 0.0 && centre < u.view.z;
    var col = u.bg;
    // The zero line.
    if abs(within - half_h) < 0.5 {
        col = u.axis;
    }
    // At least a pixel high, so quiet stretches still show.
    let one = 1.0 / half_h;
    let hi_e = max(hi, lo + one);
    if v_bot <= hi_e && v_top >= lo {
        col = select(u.wave, u.played, played);
    }
    let rh = min(r, max(hi, 0.0));
    let rl = -min(r, max(-lo, 0.0));
    if r > 0.0 && v_bot <= rh && v_top >= rl {
        col = select(u.rms, u.played_rms, played);
    }
    return col;
}

fn spec_level(c: u32, b: u32) -> f32 {
    let rows = u32(u.view.w);
    let strip = c / rows;
    return textureLoad(spec_tex, vec2<i32>(i32(strip * BINS + b), i32(c % rows)), 0).r;
}

fn spectrogram(uv: vec2<f32>) -> vec4<f32> {
    let w = u.px.x;
    let px = floor(uv.x * w);
    let cr = columns(px, w, u.a.y);
    let step = max(1u, (cr.y - cr.x + MAX_READS - 1u) / MAX_READS);
    // High frequencies at the top.
    let b = (1.0 - uv.y) * f32(BINS) - 0.5;
    let b0 = u32(clamp(floor(b), 0.0, f32(BINS - 1u)));
    let b1 = min(b0 + 1u, BINS - 1u);
    let t = clamp(b - f32(b0), 0.0, 1.0);
    var v = 0.0;
    for (var c = cr.x; c < cr.y; c += step) {
        v = max(v, mix(spec_level(c, b0), spec_level(c, b1), t));
    }
    return vec4<f32>(ramp(v), 1.0);
}

// Momentary and short-term loudness at step position `x` (between step centres).
fn hist_at(x: f32, steps: f32) -> vec2<f32> {
    let xc = clamp(x, 0.0, steps - 1.0);
    let k = u32(floor(xc));
    let k1 = min(k + 1u, u32(steps) - 1u);
    let t = xc - floor(xc);
    let a = textureLoad(hist_tex, row_load(k), 0).xy;
    let b = textureLoad(hist_tex, row_load(k1), 0).xy;
    return mix(a, b, t);
}

fn history(uv: vec2<f32>) -> vec4<f32> {
    let w = u.px.x;
    let h = u.px.y;
    let steps = u.a.w;
    if steps < 1.0 {
        return u.bg;
    }
    let lo_l = u.hist.x;
    let hi_l = u.hist.y;
    let y = uv.y * h;
    let px = floor(uv.x * w);
    // Step positions (centres) at the pixel's edges.
    let per = u.px.z;
    let s0 = (u.view.x + px / w * u.view.y) * per - 0.5;
    let s1 = (u.view.x + (px + 1.0) / w * u.view.y) * per - 0.5;
    var m_lo = 1e9;
    var m_hi = -1e9;
    var s_hi = -1e9;
    if s1 - s0 <= 2.0 {
        // Few steps a pixel: the line between them, interpolated.
        let a = hist_at(s0, steps);
        let b = hist_at(s1, steps);
        let ya = (hi_l - clamp(a.x, lo_l, hi_l)) / (hi_l - lo_l) * h;
        let yb = (hi_l - clamp(b.x, lo_l, hi_l)) / (hi_l - lo_l) * h;
        m_lo = min(ya, yb);
        m_hi = max(ya, yb);
        s_hi = max(a.y, b.y);
    } else {
        // Many: their range.
        let k0 = u32(clamp(floor(s0), 0.0, steps - 1.0));
        let k1 = u32(clamp(ceil(s1) + 1.0, f32(k0) + 1.0, steps));
        let stride = max(1u, (k1 - k0 + MAX_READS - 1u) / MAX_READS);
        for (var k = k0; k < k1; k += stride) {
            let v = textureLoad(hist_tex, row_load(k), 0).xy;
            let ym = (hi_l - clamp(v.x, lo_l, hi_l)) / (hi_l - lo_l) * h;
            s_hi = max(s_hi, v.y);
            m_lo = min(m_lo, ym);
            m_hi = max(m_hi, ym);
        }
    }
    let centre = (u.view.x + (px + 0.5) / w * u.view.y);
    let played = u.view.z >= 0.0 && centre < u.view.z;
    var col = u.bg;
    // Grid every 10 LU.
    let l = hi_l - (y + 0.5) / h * (hi_l - lo_l);
    let per_px = (hi_l - lo_l) / h;
    if abs(l - round(l / 10.0) * 10.0) < 0.5 * per_px {
        col = u.axis;
    }
    // Short-term loudness as an area.
    let ys = (hi_l - clamp(s_hi, lo_l, hi_l)) / (hi_l - lo_l) * h;
    if s_hi > lo_l && y >= ys {
        col = mix(col, select(u.wave, u.played, played), 0.45);
    }
    // Momentary loudness as a line.
    if m_hi > -1e8 && y + 0.5 >= m_lo - 0.75 && y + 0.5 <= m_hi + 0.75 && m_lo < h - 0.5 {
        col = select(u.wave, u.played, played);
    }
    // The integrated loudness, dashed.
    if u.hist.z > -150.0 && abs(l - u.hist.z) < 0.6 * per_px && (u32(px) % 8u) < 5u {
        col = u.mark;
    }
    return col;
}

@fragment
fn fs_panel(in: VOut) -> @location(0) vec4<f32> {
    let kind = u32(u.a.x);
    var c: vec4<f32>;
    if kind == 0u {
        c = waveform(in.uv);
    } else if kind == 1u {
        c = spectrogram(in.uv);
    } else {
        c = history(in.uv);
    }
    // The colours come premultiplied.
    return c;
}
