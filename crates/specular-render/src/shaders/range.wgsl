// Auto-range (DESIGN.md §6.2): the smallest and largest code of a 16-bit grey plane (R16Uint,
// or Rg16Uint with alpha in g), reduced per workgroup and then into `out` with atomics. Fully
// transparent samples do not count. `out` starts at (0xffffffff, 0).

struct RangeParams {
    // 1 when the plane has alpha (Rg16Uint).
    alpha: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
};

@group(0) @binding(0) var src: texture_2d<u32>;
@group(0) @binding(1) var<storage, read_write> out: array<atomic<u32>, 2>;
@group(0) @binding(2) var<uniform> rp: RangeParams;

var<workgroup> wmin: atomic<u32>;
var<workgroup> wmax: atomic<u32>;

@compute @workgroup_size(16, 16)
fn cs_range(@builtin(global_invocation_id) g: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    if li == 0u {
        atomicStore(&wmin, 0xffffffffu);
        atomicStore(&wmax, 0u);
    }
    workgroupBarrier();
    let size = textureDimensions(src);
    if g.x < size.x && g.y < size.y {
        let t = textureLoad(src, vec2<i32>(g.xy), 0);
        if rp.alpha == 0u || t.g > 0u {
            atomicMin(&wmin, t.r);
            atomicMax(&wmax, t.r);
        }
    }
    workgroupBarrier();
    if li == 0u {
        let lo = atomicLoad(&wmin);
        if lo != 0xffffffffu {
            atomicMin(&out[0], lo);
            atomicMax(&out[1], atomicLoad(&wmax));
        }
    }
}
