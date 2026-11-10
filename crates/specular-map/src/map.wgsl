// The map view's base map (crates/specular-map/src/render.rs). Positions are quantised world
// units (u32, the world 2^20 wide); they are made relative to the view's centre in integers
// first, so the deepest zoom loses nothing to f32.

struct View {
    // The centre's integer part (minus the world copy's offset) and fraction, world units.
    c: vec2<i32>,
    f: vec2<f32>,
    // NDC per world unit (y negative: world y runs south).
    k: vec2<f32>,
    // NDC per physical px (y negative).
    px: vec2<f32>,
};

struct Style {
    land: vec4<f32>,
    coast: vec4<f32>,
    border: vec4<f32>,
    // x: coast width, y: border width, physical px.
    widths: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> style: Style;

fn ndc(p: vec2<u32>) -> vec2<f32> {
    let d = vec2<f32>(vec2<i32>(p) - view.c) - view.f;
    return d * view.k;
}

// Land and lake rings as triangle fans into the stencil (even-odd by inverting).
@vertex
fn vs_fill(@location(0) p: vec2<u32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(ndc(p), 0.0, 1.0);
}

@fragment
fn fs_none() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

// The land colour wherever the stencil is odd.
@vertex
fn vs_cover(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_land() -> @location(0) vec4<f32> {
    return style.land;
}

struct LineOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// A segment as a quad `w` px wide with square caps (they hide the joins), one instance a
// segment. Below one pixel the line stays one pixel wide and fades instead.
fn segment(i: u32, s: vec4<u32>, w: f32, color: vec4<f32>) -> LineOut {
    let a = ndc(s.xy) / view.px;
    let b = ndc(s.zw) / view.px;
    let d = b - a;
    let len = length(d);
    var dir = vec2<f32>(1.0, 0.0);
    if len > 1e-4 {
        dir = d / len;
    }
    let n = vec2<f32>(-dir.y, dir.x);
    let hw = max(w, 1.0) * 0.5;
    // Corners: (end, side) for two triangles.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[i];
    let base = select(a - dir * hw, b + dir * hw, c.x > 0.5);
    let p = base + n * (c.y * hw);
    var out: LineOut;
    out.pos = vec4<f32>(p * view.px, 0.0, 1.0);
    out.color = color * min(w, 1.0);
    return out;
}

@vertex
fn vs_coast(@builtin(vertex_index) i: u32, @location(0) s: vec4<u32>) -> LineOut {
    return segment(i, s, style.widths.x, style.coast);
}

@vertex
fn vs_border(@builtin(vertex_index) i: u32, @location(0) s: vec4<u32>) -> LineOut {
    return segment(i, s, style.widths.y, style.border);
}

@fragment
fn fs_line(v: LineOut) -> @location(0) vec4<f32> {
    return v.color;
}
