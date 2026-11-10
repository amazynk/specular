// Tiled GEMM with wider per-thread blocking and vec4 operand loads, C[m, n] =
// epilogue(sum_k A[m, k] B[k, n]), f32 accumulation in k order (as gemm.wgsl, which stays
// the fallback). A TM x 64 tile of C per (16, TM / RM) workgroup, RM x 4 outputs per thread
// (rows RM*ly.., columns 4lx..), K in steps of 16 through workgroup memory. The Rust side
// supplies TM, RM, the tile-load orders A_KMAJOR / B_NMAJOR, and the operand access:
// load_a / load_b (scalar) and, when A_VEC / B_VEC, load_a4 (A[m, k..k+4]) and load_b4
// (B[k, n..n+4] when B_NMAJOR, else B[k..k+4, n]); every vec4 load is either wholly inside
// the matrix or wholly outside (K, or N, is a multiple of 4). P: M, N, K, inner batch.
const TN: u32 = 64u;
const TK: u32 = 16u;
const NT: u32 = 16u * (TM / RM);

// [k][m / 4] and [k][n / 4].
var<workgroup> As: array<f32, TK * TM>;
var<workgroup> Bs: array<f32, TK * TN>;

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

@compute @workgroup_size(16, TM / RM, 1)
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
    var acc: array<vec4<f32>, RM>;
    for (var i = 0u; i < RM; i++) {
        acc[i] = vec4<f32>(0.0);
    }
    for (var k0 = 0u; k0 < K; k0 += TK) {
        if (A_VEC) {
            // TM x TK as TM x 4 vec4 along k.
            for (var r = 0u; r < TM * TK / 4u / NT; r++) {
                let e = li + r * NT;
                let mm = e / 4u;
                let kq = 4u * (e % 4u);
                var av = vec4<f32>(0.0);
                if (m0 + mm < M && k0 + kq < K) {
                    av = load_a4(z1, z2, m0 + mm, k0 + kq);
                }
                for (var j = 0u; j < 4u; j++) {
                    put_a((kq + j) * (TM / 4u) + mm / 4u, mm % 4u, av[j]);
                }
            }
        } else {
            for (var r = 0u; r < TM * TK / NT; r++) {
                let e = li + r * NT;
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
                put_a(ka * (TM / 4u) + mm / 4u, mm % 4u, av);
            }
        }
        if (B_VEC && B_NMAJOR) {
            // TK x TN as TK x 16 vec4 along n.
            for (var r = 0u; r < TK * TN / 4u / NT; r++) {
                let e = li + r * NT;
                let kb = e / 16u;
                let nq = e % 16u;
                var bv = vec4<f32>(0.0);
                if (n0 + 4u * nq < N && k0 + kb < K) {
                    bv = load_b4(z1, z2, k0 + kb, n0 + 4u * nq);
                }
                put_b4(kb * 16u + nq, bv);
            }
        } else if (B_VEC) {
            // TK x TN as 4 x TN vec4 along k.
            for (var r = 0u; r < TK * TN / 4u / NT; r++) {
                let e = li + r * NT;
                let nn = e / 4u;
                let kq = 4u * (e % 4u);
                var bv = vec4<f32>(0.0);
                if (n0 + nn < N && k0 + kq < K) {
                    bv = load_b4(z1, z2, k0 + kq, n0 + nn);
                }
                for (var j = 0u; j < 4u; j++) {
                    put_b((kq + j) * 16u + nn / 4u, nn % 4u, bv[j]);
                }
            }
        } else {
            for (var r = 0u; r < TK * TN / NT; r++) {
                let e = li + r * NT;
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
        }
        workgroupBarrier();
        for (var kk = 0u; kk < TK; kk++) {
            let b = get_b(kk * 16u + lid.x);
            for (var q = 0u; q < RM / 4u; q++) {
                let a = get_a(kk * (TM / 4u) + lid.y * (RM / 4u) + q);
                acc[4u * q] = fma(vec4<f32>(a.x), b, acc[4u * q]);
                acc[4u * q + 1u] = fma(vec4<f32>(a.y), b, acc[4u * q + 1u]);
                acc[4u * q + 2u] = fma(vec4<f32>(a.z), b, acc[4u * q + 2u]);
                acc[4u * q + 3u] = fma(vec4<f32>(a.w), b, acc[4u * q + 3u]);
            }
        }
        workgroupBarrier();
    }
    let m = m0 + RM * lid.y;
    let n = n0 + 4u * lid.x;
    for (var i = 0u; i < RM; i++) {
        store_row(z1, z2, m + i, n, acc[i], M, N);
    }
}
