// Bilinear demosaic of a one-shot colour camera's raw frame (a 2×2 Bayer colour filter
// array) into red, green and blue values: each pixel keeps its own colour and takes the
// others from the mean of the nearest pixels of that colour (across, or diagonally). The
// frame is mirrored at its edges without repeating the edge, so edge pixels keep the
// pattern. specular_core::nd::Cfa::debayer_at is the reference.

struct Debayer {
    // The colour (0 red, 1 green, 2 blue) at (x & 1) + 2 (y & 1).
    pat: vec4<u32>,
};

@group(0) @binding(0) var<uniform> d: Debayer;
@group(0) @binding(1) var raw: texture_2d<f32>;

fn mirror(i: i32, n: i32) -> i32 {
    var j = i;
    if j < 0 { j = -j; }
    if j >= n { j = 2 * n - 2 - j; }
    return clamp(j, 0, n - 1);
}

fn at(p: vec2<i32>, dx: i32, dy: i32, n: vec2<i32>) -> f32 {
    return textureLoad(raw, vec2<i32>(mirror(p.x + dx, n.x), mirror(p.y + dy, n.y)), 0).r;
}

fn colour(x: i32, y: i32) -> u32 {
    return d.pat[(x & 1) + 2 * (y & 1)];
}

@fragment
fn fs_debayer(in: VOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(in.pos.xy));
    let n = vec2<i32>(textureDimensions(raw, 0));
    let v = at(p, 0, 0, n);
    if n.x < 2 || n.y < 2 {
        return vec4<f32>(v, v, v, 0.0);
    }
    let own = colour(p.x, p.y);
    var o = array<f32, 3>(0.0, 0.0, 0.0);
    o[own] = v;
    if own == 1u {
        let row = colour(p.x + 1, p.y);
        o[row] = (at(p, -1, 0, n) + at(p, 1, 0, n)) * 0.5;
        o[2u - row] = (at(p, 0, -1, n) + at(p, 0, 1, n)) * 0.5;
    } else {
        o[1] = (at(p, -1, 0, n) + at(p, 1, 0, n) + at(p, 0, -1, n) + at(p, 0, 1, n)) * 0.25;
        o[2u - own] = (at(p, -1, -1, n) + at(p, 1, -1, n) + at(p, -1, 1, n) + at(p, 1, 1, n))
            * 0.25;
    }
    return vec4<f32>(o[0], o[1], o[2], 0.0);
}
