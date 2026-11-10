// The frame both the mesh and the point shaders draw under (FrameU in lib.rs).

struct Frame {
    // World to clip (reversed depth: 1 nearest).
    vp: mat4x4<f32>,
    // Quantised position to world: offset + q * scale.
    q_scale: vec4<f32>,
    q_offset: vec4<f32>,
    // xyz: the eye; w: 1 in perspective, 0 in orthographic.
    eye: vec4<f32>,
    forward: vec4<f32>,
    // xyz: towards the light; w: the ambient share.
    light: vec4<f32>,
    // x: edge width (px); y: 1 to draw edges over the shaded surface; z: x-ray gain;
    // w: the opacity of translucent impostors.
    params: vec4<f32>,
    // x: the grid's height; y: its spacing; z: its half extent; w: its opacity.
    grid: vec4<f32>,
    // xy: the grid's centre; z: 1 to draw the x and y axes.
    grid_centre: vec4<f32>,
    // xyz: the centre of the drawn box; w: its radius (translucent impostors' depth weight).
    bounds: vec4<f32>,
    // Section planes (xyz: normal, w: offset; what is beyond is cut away).
    clip: array<vec4<f32>, 12>,
    // x: planes in use; y: the tolerance (model units).
    clip_n: vec4<f32>,
};

@group(0) @binding(0) var<uniform> fr: Frame;

// Whether `p` is cut away by a section plane or the section box.
fn clipped(p: vec3<f32>) -> bool {
    let n = u32(fr.clip_n.x);
    for (var i = 0u; i < n; i = i + 1u) {
        let c = fr.clip[i];
        if dot(c.xyz, p) - c.w > fr.clip_n.y {
            return true;
        }
    }
    return false;
}
