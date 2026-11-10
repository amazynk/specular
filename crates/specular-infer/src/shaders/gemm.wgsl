// Tiled GEMM, C[m, n] = epilogue(sum_k A[m, k] B[k, n]), f32 accumulation in k order.
// A 64x64 tile of C per 16x16 workgroup, 4x4 outputs per thread (rows 4ly..4ly+3, columns
// 4lx..4lx+3, so each k step is two vec4 reads of workgroup memory and four vec4 FMAs into
// register accumulators), K in steps of 16 through workgroup memory. The Rust side
// supplies load_a, load_b (strided matmul or implicit-im2col convolution), store_c (which
// adds the bias, applies the activation and adds a residual) and the tile-load orders
// A_KMAJOR / B_NMAJOR (which index is contiguous in memory, so that neighbouring threads
// load neighbouring words). P: M, N, K, inner batch count.
const TM: u32 = 64u;
const TN: u32 = 64u;
const TK: u32 = 16u;

// [k][m / 4] and [k][n / 4].
var<workgroup> As: array<f32, 1024>;
var<workgroup> Bs: array<f32, 1024>;

// The tiles are stored as scalars and read as vec4s. Storing single components of a
// workgroup vec4 (`As[i][c] = v`, or `As[i].z = v`) from different threads loses values on
// Apple GPUs through Metal, which writes the whole vector: neighbouring threads filling the
// other components of the same vec4 overwrite each other.
fn put_a(i: u32, c: u32, v: f32) {
    As[4u * i + c] = v;
}

fn put_b(i: u32, c: u32, v: f32) {
    Bs[4u * i + c] = v;
}

fn put_b4(i: u32, v: vec4<f32>) {
    Bs[4u * i] = v.x;
    Bs[4u * i + 1u] = v.y;
    Bs[4u * i + 2u] = v.z;
    Bs[4u * i + 3u] = v.w;
}

fn get_a(i: u32) -> vec4<f32> {
    return vec4<f32>(As[4u * i], As[4u * i + 1u], As[4u * i + 2u], As[4u * i + 3u]);
}

fn get_b(i: u32) -> vec4<f32> {
    return vec4<f32>(Bs[4u * i], Bs[4u * i + 1u], Bs[4u * i + 2u], Bs[4u * i + 3u]);
}


fn store_row(z1: u32, z2: u32, m: u32, n0: u32, v: vec4<f32>, M: u32, N: u32) {
    if (m >= M) {
        return;
    }
    for (var j = 0u; j < 4u; j++) {
        let n = n0 + j;
        if (n < N) {
            store_c(z1, z2, m, n, v[j]);
        }
    }
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let M = pu(0u);
    let N = pu(1u);
    let K = pu(2u);
    let nb2 = pu(3u);
    let z1 = wid.z / nb2;
    let z2 = wid.z % nb2;
    // Parameter 23: the first row of tiles of this dispatch (a GEMM split in several).
    let m0 = (wid.y + pu(23u)) * TM;
    let n0 = wid.x * TN;
    var acc0 = vec4<f32>(0.0);
    var acc1 = vec4<f32>(0.0);
    var acc2 = vec4<f32>(0.0);
    var acc3 = vec4<f32>(0.0);
    for (var k0 = 0u; k0 < K; k0 += TK) {
        for (var r = 0u; r < 4u; r++) {
            let e = li + r * 256u;
            var mm: u32;
            var ka: u32;
            if (A_KMAJOR) {
                ka = e % TK;
                mm = e / TK;
            } else {
                mm = e % TM;
                ka = e / TM;
            }
            var av = 0.0;
            if (m0 + mm < M && k0 + ka < K) {
                av = load_a(z1, z2, m0 + mm, k0 + ka);
            }
            put_a(ka * 16u + mm / 4u, mm % 4u, av);
            var nn: u32;
            var kb: u32;
            if (B_NMAJOR) {
                nn = e % TN;
                kb = e / TN;
            } else {
                kb = e % TK;
                nn = e / TK;
            }
            var bv = 0.0;
            if (n0 + nn < N && k0 + kb < K) {
                bv = load_b(z1, z2, k0 + kb, n0 + nn);
            }
            put_b(kb * 16u + nn / 4u, nn % 4u, bv);
        }
        workgroupBarrier();
        for (var kk = 0u; kk < TK; kk++) {
            let a = get_a(kk * 16u + lid.y);
            let b = get_b(kk * 16u + lid.x);
            acc0 += a.x * b;
            acc1 += a.y * b;
            acc2 += a.z * b;
            acc3 += a.w * b;
        }
        workgroupBarrier();
    }
    let m = m0 + 4u * lid.y;
    let n = n0 + 4u * lid.x;
    store_row(z1, z2, m, n, acc0, M, N);
    store_row(z1, z2, m + 1u, n, acc1, M, N);
    store_row(z1, z2, m + 2u, n, acc2, M, N);
    store_row(z1, z2, m + 3u, n, acc3, M, N);
}
