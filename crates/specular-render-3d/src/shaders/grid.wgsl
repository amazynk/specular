// The ground grid: one quad under the model, lines every spacing and stronger every ten,
// fading towards its edge and when seen edge-on.

struct GOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) xy: vec2<f32>,
};

@vertex
fn vs_grid(@builtin(vertex_index) vi: u32) -> GOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let xy = fr.grid_centre.xy + corners[vi] * fr.grid.z;
    var o: GOut;
    o.clip = fr.vp * vec4<f32>(xy, fr.grid.x, 1.0);
    o.xy = xy;
    return o;
}

fn lines(coord: vec2<f32>) -> f32 {
    let g = abs(fract(coord - 0.5) - 0.5) / max(fwidth(coord), vec2<f32>(1e-6));
    return 1.0 - min(min(g.x, g.y), 1.0);
}

// The grid's premultiplied colour at a fragment (the point shader writes it with its depth).
fn grid_colour(in: GOut) -> vec4<f32> {
    let coord = in.xy / fr.grid.y;
    let minor = lines(coord);
    let major = lines(coord / 10.0);
    let dist = length(in.xy - fr.grid_centre.xy) / fr.grid.z;
    let fade = 1.0 - smoothstep(0.55, 1.0, dist);
    let grazing = smoothstep(0.03, 0.2, abs(fr.forward.z));
    // The x and y axes where the grid crosses them.
    let w = max(fwidth(in.xy), vec2<f32>(1e-9));
    let on_x = (1.0 - min(abs(in.xy.y) / (1.5 * w.y), 1.0)) * fr.grid_centre.z;
    let on_y = (1.0 - min(abs(in.xy.x) / (1.5 * w.x), 1.0)) * fr.grid_centre.z;
    var c = vec3<f32>(0.55, 0.58, 0.62);
    var a = max(minor * 0.22, major * 0.45);
    c = mix(c, vec3<f32>(0.85, 0.22, 0.2), on_x);
    a = max(a, on_x * 0.8);
    c = mix(c, vec3<f32>(0.3, 0.75, 0.25), on_y * (1.0 - on_x));
    a = max(a, on_y * 0.8);
    a = a * fade * grazing * fr.grid.w;
    return vec4<f32>(c * a, a);
}

@fragment
fn fs_grid(in: GOut) -> @location(0) vec4<f32> {
    return grid_colour(in);
}
