// Shows the point view in its rectangle of the HDR scene, with eye-dome lighting when it is
// on: each point pixel is darkened by how much nearer than it its neighbours are not, in
// log depth, so edges and steps in the cloud read as shading without normals. Neighbours
// with no point count as far behind, outlining the cloud against the background.

struct Edl {
    // Target rectangle: x, y, width, height (px).
    rect: vec4<f32>,
    // x: strength (0 off); y: neighbour radius (px).
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> eu: Edl;
@group(0) @binding(1) var view: texture_2d<f32>;
@group(0) @binding(2) var depth: texture_2d<f32>;

fn depth_at(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(depth));
    let q = clamp(p, vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(depth, q, 0).r;
}

@fragment
fn fs_edl(in: VOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(in.pos.xy - eu.rect.xy));
    let c = textureLoad(view, p, 0);
    let d = depth_at(p);
    if eu.params.x <= 0.0 || d <= 0.0 {
        return vec4<f32>(min(c.rgb, vec3<f32>(1.0)), min(c.a, 1.0));
    }
    var dirs = array<vec2<f32>, 8>(
        vec2<f32>(1.0, 0.0), vec2<f32>(0.7071, 0.7071), vec2<f32>(0.0, 1.0),
        vec2<f32>(-0.7071, 0.7071), vec2<f32>(-1.0, 0.0), vec2<f32>(-0.7071, -0.7071),
        vec2<f32>(0.0, -1.0), vec2<f32>(0.7071, -0.7071),
    );
    var sum = 0.0;
    for (var i = 0; i < 8; i++) {
        let n = depth_at(p + vec2<i32>(round(dirs[i] * eu.params.y)));
        if n <= 0.0 {
            sum += 0.05;
        } else {
            sum += max(0.0, d - n);
        }
    }
    let shade = exp(-sum * 40.0 * eu.params.x);
    return vec4<f32>(min(c.rgb * shade, vec3<f32>(1.0)), min(c.a, 1.0));
}
