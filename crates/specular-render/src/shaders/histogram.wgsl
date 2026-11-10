// Bins the histogram target (straight linear RGB, alpha −1 outside the image) into 4 × 256
// sRGB-encoded bins (R, G, B, luma) plus total / clipped-high / clipped-low counts. Each
// workgroup bins into shared memory first, then adds its non-zero bins to the output.
// The binning rule is specular-inspect's `bin` / `luma`.

struct P {
    size: vec2<u32>,
    // clipping thresholds as 8-bit codes (inclusive)
    low: u32,
    high: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> bins: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> prm: P;

const WORDS: u32 = 1027u;
var<workgroup> local: array<atomic<u32>, 1027>;

fn enc(v: f32) -> f32 {
    let x = clamp(v, 0.0, 1.0);
    if x <= 0.0031308 { return x * 12.92; }
    return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}

fn bin_of(e: f32) -> u32 {
    return u32(min(clamp(e, 0.0, 1.0) * 255.0 + 0.5, 255.0));
}

@compute @workgroup_size(16, 16)
fn cs_histogram(@builtin(global_invocation_id) g: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    for (var i = li; i < WORDS; i += 256u) {
        atomicStore(&local[i], 0u);
    }
    workgroupBarrier();
    if g.x < prm.size.x && g.y < prm.size.y {
        let c = textureLoad(src, vec2<i32>(g.xy), 0);
        if c.a > 0.0 {
            let lin = clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
            let r = bin_of(enc(lin.r));
            let gg = bin_of(enc(lin.g));
            let b = bin_of(enc(lin.b));
            let l = bin_of(enc(dot(lin, vec3<f32>(0.2126, 0.7152, 0.0722))));
            atomicAdd(&local[r], 1u);
            atomicAdd(&local[256u + gg], 1u);
            atomicAdd(&local[512u + b], 1u);
            atomicAdd(&local[768u + l], 1u);
            atomicAdd(&local[1024u], 1u);
            if max(r, max(gg, b)) >= prm.high { atomicAdd(&local[1025u], 1u); }
            if min(r, min(gg, b)) <= prm.low { atomicAdd(&local[1026u], 1u); }
        }
    }
    workgroupBarrier();
    for (var i = li; i < WORDS; i += 256u) {
        let v = atomicLoad(&local[i]);
        if v > 0u { atomicAdd(&bins[i], v); }
    }
}
