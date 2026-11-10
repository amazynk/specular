// Direct discrete Fourier transforms along one axis, orthonormal (cpu.rs's `dft`), one
// thread per output value pair (complex outputs) or value (real outputs). Twiddles are
// e^(±2πi·(k·t mod n)/n). P: threads, mode (0 real→complex, 1 complex, 2 complex→real),
// n, bins (n/2 + 1) or m (the inner complex count), sign (+1 inverse, −1 forward, as f32),
// norm (1/√n, f32).
const TAU: f32 = 6.283185307179586;

fn tw(k: u32, n: u32) -> vec2<f32> {
    let a = f32(k % n) * (TAU / f32(n));
    return vec2<f32>(cos(a), sin(a));
}

@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let i = gidx(wid, nwg, li);
    if (i >= pu(0u)) {
        return;
    }
    let mode = pu(1u);
    let n = pu(2u);
    let q = pu(3u);
    let sign = pf(4u);
    let norm = pf(5u);
    if (mode == 0u) {
        // i = row·bins + b.
        let b = i % q;
        let base = (i / q) * n;
        var acc = vec2<f32>(0.0);
        for (var t = 0u; t < n; t++) {
            let e = tw(b * t, n);
            acc += ld0(base + t) * vec2<f32>(e.x, -e.y);
        }
        dst[2u * i] = acc.x * norm;
        dst[2u * i + 1u] = acc.y * norm;
    } else if (mode == 1u) {
        // i = (block·n + b)·m + j.
        let j = i % q;
        let b = (i / q) % n;
        let base = (i / (q * n)) * n * q;
        var acc = vec2<f32>(0.0);
        for (var t = 0u; t < n; t++) {
            let e = tw(b * t, n);
            let s = e.y * sign;
            let at = 2u * (base + t * q + j);
            let xr = ld0(at);
            let xi = ld0(at + 1u);
            acc += vec2<f32>(xr * e.x - xi * s, xr * s + xi * e.x);
        }
        dst[2u * i] = acc.x * norm;
        dst[2u * i + 1u] = acc.y * norm;
    } else {
        // i = row·n + t; q bins per row.
        let t = i % n;
        let base = 2u * (i / n) * q;
        var acc = ld0(base);
        for (var b = 1u; b < q; b++) {
            let e = tw(b * t, n);
            let re = ld0(base + 2u * b);
            if (2u * b == n) {
                acc += re * e.x;
            } else {
                acc += 2.0 * (re * e.x - ld0(base + 2u * b + 1u) * e.y);
            }
        }
        dst[i] = acc * norm;
    }
}
