// One time step of a 1-D LSTM (graph::Op::Lstm) shaped as a matrix product: the step's
// recurrent sums are hprev [S, ns] · wh [ns, 4ns], tiled. A workgroup of UN·SQ threads takes
// UN states (all four gates) of SQ sequences; thread (u, q) sums the four gate rows of state
// u for sequence q, so it updates that cell itself, no partial sums to combine. K is walked
// in chunks of KC: the chunk's weights of the workgroup's 4·UN columns and its sequences'
// previous hidden values are loaded to workgroup memory once and each weight serves SQ
// sequences, each hidden value 4·UN columns. After the output (`out_len` floats) dst holds
// the cells, then the hidden state twice (read one, write the other, alternating by step).
// SL threads split each chunk's sum for one (state, sequence) when there are few of them;
// their partial sums meet in workgroup memory. The host sets UN, SQ, SL, SLP (SL − 1, at
// least 1), KC and WGS (= UN·SQ·SL).
// P: S, T, ns, flags (1 reverse, 2 summary, 4 lengths), per, step, out_len.
// in0 pre [S, T, 4ns], in1 wh [ns, 4ns], in2 lens [S / per] (with flag 4).
var<workgroup> wt: array<f32, KC * 4u * UN>;
var<workgroup> ht: array<f32, KC * SQ>;
var<workgroup> part: array<vec4<f32>, SLP * UN * SQ>;

fn seq_len(s: u32) -> u32 {
    if ((pu(3u) & 4u) == 0u) {
        return pu(1u);
    }
    return min(u32(max(LEN_LOAD, 0.0)), pu(1u));
}

@compute @workgroup_size(WGS)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let n_seq = pu(0u);
    let t_all = pu(1u);
    let ns = pu(2u);
    let flags = pu(3u);
    let k = pu(5u);
    let out_len = pu(6u);
    let g4 = 4u * ns;
    let blocks = (ns + UN - 1u) / UN;
    let wg = growid(wid, nwg);
    let s0 = (wg / blocks) * SQ;
    let u0 = (wg % blocks) * UN;
    let n_c = out_len;
    let h_in = out_len + n_seq * ns * (1u + (k & 1u));
    let h_out = out_len + n_seq * ns * (2u - (k & 1u));
    let u = li % UN;
    let q = (li / UN) % SQ;
    let sl = li / (UN * SQ);
    let iu = u0 + u;
    let s = s0 + q;
    var acc = vec4<f32>(0.0);
    if (k > 0u) {
        for (var j0 = 0u; j0 < ns; j0 += KC) {
            // The chunk's weights: row j, gate g, state u0 + uu at wt[(j·4 + g)·UN + uu].
            for (var x = li; x < KC * 4u * UN; x += WGS) {
                let uu = x % UN;
                let gj = x / UN;
                let j = j0 + gj / 4u;
                let col = u0 + uu;
                var w = 0.0;
                if (j < ns && col < ns) {
                    w = ld1(j * g4 + (gj % 4u) * ns + col);
                }
                wt[x] = w;
            }
            // The chunk's previous hidden values of the workgroup's sequences.
            for (var x = li; x < KC * SQ; x += WGS) {
                let j = j0 + x % KC;
                let sq = min(s0 + x / KC, n_seq - 1u);
                var h = 0.0;
                if (j < ns) {
                    h = dst[h_in + sq * ns + j];
                }
                ht[x] = h;
            }
            workgroupBarrier();
            // This thread's slice of the chunk.
            let n = min(KC, ns - j0);
            let per = (KC + SL - 1u) / SL;
            let j1 = min(n, (sl + 1u) * per);
            for (var j = sl * per; j < j1; j++) {
                let h = ht[q * KC + j];
                let b = j * 4u * UN + u;
                acc += h * vec4<f32>(wt[b], wt[b + UN], wt[b + 2u * UN], wt[b + 3u * UN]);
            }
            workgroupBarrier();
        }
    }
    // The slices' sums added up by the first slice's threads.
    if (SL > 1u) {
        if (sl > 0u) {
            part[((sl - 1u) * SQ + q) * UN + u] = acc;
        }
        workgroupBarrier();
        if (sl > 0u) {
            return;
        }
        for (var x = 1u; x < SL; x++) {
            acc += part[((x - 1u) * SQ + q) * UN + u];
        }
    }
    if (s >= n_seq || iu >= ns) {
        return;
    }
    let len = seq_len(s);
    if (k >= len) {
        return;
    }
    var t = k;
    if ((flags & 1u) != 0u) {
        t = len - 1u - k;
    }
    let p = (s * t_all + t) * g4;
    let g = acc + vec4<f32>(ld0(p + iu), ld0(p + ns + iu), ld0(p + 2u * ns + iu),
                            ld0(p + 3u * ns + iu));
    var c = 0.0;
    if (k > 0u) {
        c = dst[n_c + s * ns + iu];
    }
    c = clamp(c * sigm(g[2]) + tanh_(g[0]) * sigm(g[1]), -100.0, 100.0);
    let h = tanh_(c) * sigm(g[3]);
    dst[n_c + s * ns + iu] = c;
    dst[h_out + s * ns + iu] = h;
    if ((flags & 2u) != 0u) {
        dst[s * ns + iu] = h;
    } else {
        dst[(s * t_all + t) * ns + iu] = h;
    }
}
