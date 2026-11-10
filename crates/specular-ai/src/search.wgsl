// Exact dot-product scores of every stored f16 vector against one query (specular-ai search).
//
// Vectors are stored in blocks of BLOCK, dimension-pair-major: word (b * pairs + p) * BLOCK + i
// holds dimensions 2p and 2p+1 of vector i of block b (low half first), so neighbouring
// threads read neighbouring words. Every value is a normal f16 or zero (sanitised on insert),
// so the bit conversion below is exact and the products are exact in f32; the sum runs over
// d in order, exactly as the CPU path does, so both give the same bits.

const BLOCK: u32 = 1024u;

struct Params {
    count: u32,   // vectors in this chunk
    pairs: u32,   // dims / 2
    _a: u32,
    _b: u32,
}

@group(0) @binding(0) var<storage, read> data: array<u32>;
@group(0) @binding(1) var<storage, read> query: array<f32>;
@group(0) @binding(2) var<storage, read_write> scores: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

fn half_to_f32(h: u32) -> f32 {
    let e = (h >> 10u) & 31u;
    let m = h & 0x3ffu;
    let s = (h & 0x8000u) << 16u;
    let bits = select(s | ((e + 112u) << 23u) | (m << 13u), s, e == 0u);
    return bitcast<f32>(bits);
}

@compute @workgroup_size(256)
fn cs_scores(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.count) {
        return;
    }
    let b = i / BLOCK;
    let lane = i % BLOCK;
    var base = b * params.pairs * BLOCK + lane;
    var acc = 0.0;
    for (var p = 0u; p < params.pairs; p = p + 1u) {
        let w = data[base];
        acc = acc + half_to_f32(w & 0xffffu) * query[2u * p];
        acc = acc + half_to_f32(w >> 16u) * query[2u * p + 1u];
        base = base + BLOCK;
    }
    scores[i] = acc;
}
