//! Diffuse or sharpen as it ran before its passes were merged (tests only): an à-trous level and a
//! band update per dispatch, the change summed in an accumulator. The new program must
//! give the same pixels, bit for bit, when no band runs on a reduced grid.

use super::diffuse::{
    DF_DILATE, DF_HOT, DF_INIT, DIFFUSE_WGSL, Diffuse, HOT_TILE, f32_cascade, no_hot_mask, plan,
};
use super::kernels::fw;
use crate::graph::{Kernel, Program, Roi};

/// Scratch images (each `3n` words, of the region or, for a draft, of its half-size copy):
/// the iterate and the next (0, 1), the cascade's two buffers (2, 3) and, for a draft, the
/// half-size input (4). A pixel's channels are together (one cache line per stencil tap):
/// three f32, or in the cascade of an f16 render two words of packed f16 (the passes are
/// bound by memory traffic; the iterates stay f32).
pub(crate) fn program(d: &Diffuse, input: Roi) -> Program {
    let pl = plan(d);
    let f = if pl.half { 2 } else { 1 };
    let (w, h) = (input.w.div_ceil(f), input.h.div_ceil(f));
    let n = w * h;
    let mut prog = Program::default();
    let th = [w, h];
    prog.step(
        &DF_INIT,
        th,
        vec![
            0,
            w,
            input.x,
            input.y,
            n,
            f,
            input.right(),
            input.bottom(),
            12 * n,
        ],
    );
    let tail = [fw(pl.inv_k), fw(pl.threshold), fw(d.highlights)];
    // Highlights only: the tiles within one iteration's reach of a pixel that can change
    // (see `DF_HOT`). The bands are made only there; elsewhere no change reads them.
    let mb = if pl.half { 15 } else { 12 } * n;
    let (tw, tht) = (w.div_ceil(HOT_TILE), h.div_ceil(HOT_TILE));
    let nt = tw * tht;
    let masked = d.highlights > 0.0 && pl.iterations > 0 && !no_hot_mask();
    let mask = mb + 2 * nt;
    if masked {
        let reach = pl.reach().div_ceil(HOT_TILE) + 1;
        prog.step(&DF_HOT, [tw, tht], vec![w, h, n, tw, mb, fw(d.highlights)]);
        prog.step(
            &DF_DILATE,
            [tw, tht],
            vec![tw, tht, mb, mb + nt, reach, 1, 0],
        );
        prog.step(
            &DF_DILATE,
            [tw, tht],
            vec![tw, tht, mb + nt, mask, reach, 0, 1],
        );
    }
    // The cascade at f16 in f16 renders.
    let half = u32::from(!f32_cascade());
    for it in 0..pl.iterations {
        let (u, acc) = if it % 2 == 0 { (0, 3 * n) } else { (3 * n, 0) };
        let mut p = u;
        let mut first = true;
        let last = pl.weights.iter().rposition(|w| *w > 0.0).unwrap_or(0);
        for (j, &wt) in pl.weights.iter().enumerate() {
            let q = if j % 2 == 0 { 6 * n } else { 9 * n };
            // `p` is the iterate (f32) for the first band, then the cascade.
            let pc = u32::from(j > 0);
            prog.step(
                &DF_ATROUS,
                th,
                vec![p, q, w, h, n, 1 << j, u32::from(masked), mask, tw, pc, half],
            );
            if wt > 0.0 {
                let mut words = vec![
                    u,
                    p,
                    q,
                    acc,
                    w,
                    h,
                    n,
                    1 << j,
                    u32::from(first),
                    fw(wt * pl.dt),
                ];
                words.extend(d.speed.iter().map(|v| fw(*v)));
                words.extend(d.anisotropy.iter().map(|v| fw(*v)));
                words.extend_from_slice(&tail);
                words.extend_from_slice(&[
                    u32::from(j == last),
                    1 << pl.first,
                    1 << pl.last(),
                    fw(pl.margin),
                    pc,
                    half,
                ]);
                prog.step(&DF_UPDATE, th, words);
                first = false;
            }
            p = q;
        }
    }
    prog.words = u64::from(mb) + if masked { 3 * u64::from(nt) } else { 0 };
    prog
}

/// Words: source (an image), destination (cascade), width, height, plane size, spacing,
/// highlight mask on, mask, tiles per row, source in the cascade, cascade at f16 in f16
/// renders. One à-trous B3 level of
/// an RGB image.
pub static DF_ATROUS: Kernel = Kernel {
    name: "diffuse_atrous_v2",
    atomic: false,
    helpers: &[DIFFUSE_WGSL],
    body: "//u32
    let so = pu(p);
    let w = pu(p + 2u);
    let h = pu(p + 3u);
    let st = i32(pu(p + 5u));
    let hc = pu(p + 10u) != 0u;
    let sh = pu(p + 9u) != 0u && hc;
    // Highlights: no change reads the bands outside the marked tiles (`DF_HOT`).
    if (pu(p + 6u) != 0u
        && bitcast<f32>(S[pu(p + 7u) + (t.y / 16u) * pu(p + 8u) + t.x / 16u]) == 0.0) {
        return;
    }
    var acc = vec3<f32>(0.0);
    for (var j = -2; j <= 2; j++) {
        let hy = select(select(0.0625, 0.25, abs(j) == 1), 0.375, j == 0);
        var row = vec3<f32>(0.0);
        for (var k = -2; k <= 2; k++) {
            let hx = select(select(0.0625, 0.25, abs(k) == 1), 0.375, k == 0);
            row += hx * df_at(so, w, h, sh, i32(t.x) + k * st, i32(t.y) + j * st);
        }
        acc += hy * row;
    }
    df_set(pu(p + 1u), t.y * w + t.x, hc, acc);",
};

/// Words: iterate `u`, band's `L_j`, `L_{j+1}`, accumulator (each an image), width,
/// height, plane size, spacing, first band (starts the accumulator), weight·dt, speeds 1–4,
/// anisotropies 1–4, 1/K, edge threshold, highlights, last band, finest and coarsest
/// spacing, margin, `L_j` in the cascade, cascade at f16 in f16 renders. The last band writes `u + change` held within the
/// range of `u` over both 3×3 stencils, widened by the margin: the next iterate.
pub static DF_UPDATE: Kernel = Kernel {
    name: "diffuse_update_v2",
    atomic: false,
    helpers: &[DIFFUSE_WGSL],
    body: "//u32
    let uo = pu(p);
    let po = pu(p + 1u);
    let qo = pu(p + 2u);
    let ao = pu(p + 3u);
    let w = pu(p + 4u);
    let h = pu(p + 5u);
    let n = pu(p + 6u);
    let s = i32(pu(p + 7u));
    let x = i32(t.x);
    let y = i32(t.y);
    // Highlights only: a pixel below their smooth step does not change, so the stencils
    // are not read. Its accumulator stays 0 and the last band writes `u` itself (what the
    // range clamp gives, `u` being inside its neighbourhood's range).
    let hl0 = pf(p + 20u);
    if (hl0 > 0.0) {
        let i = t.y * w + t.x;
        let c = df_get(uo, i, false);
        if (max(c.x, max(c.y, c.z)) <= 0.8 * hl0) {
            if (pu(p + 21u) != 0u) {
                df_set(ao, i, false, c);
            } else if (pu(p + 8u) != 0u) {
                df_set(ao, i, false, vec3<f32>(0.0));
            }
            return;
        }
    }
    let hc = pu(p + 26u) != 0u;
    let ph = pu(p + 25u) != 0u && hc;
    // The 3×3 taps (named, not an array: an indexed array may live in scratch memory).
    let l0 = df_at(qo, w, h, hc, x - s, y - s);
    let l1 = df_at(qo, w, h, hc, x, y - s);
    let l2 = df_at(qo, w, h, hc, x + s, y - s);
    let l3 = df_at(qo, w, h, hc, x - s, y);
    let l4 = df_at(qo, w, h, hc, x, y);
    let l5 = df_at(qo, w, h, hc, x + s, y);
    let l6 = df_at(qo, w, h, hc, x - s, y + s);
    let l7 = df_at(qo, w, h, hc, x, y + s);
    let l8 = df_at(qo, w, h, hc, x + s, y + s);
    let h0 = df_at(po, w, h, ph, x - s, y - s) - l0;
    let h1 = df_at(po, w, h, ph, x, y - s) - l1;
    let h2 = df_at(po, w, h, ph, x + s, y - s) - l2;
    let h3 = df_at(po, w, h, ph, x - s, y) - l3;
    let h4 = df_at(po, w, h, ph, x, y) - l4;
    let h5 = df_at(po, w, h, ph, x + s, y) - l5;
    let h6 = df_at(po, w, h, ph, x - s, y + s) - l6;
    let h7 = df_at(po, w, h, ph, x, y + s) - l7;
    let h8 = df_at(po, w, h, ph, x + s, y + s) - l8;
    let lxx = l5 - 2.0 * l4 + l3;
    let lyy = l7 - 2.0 * l4 + l1;
    let lxy = 0.25 * (l8 - l6 - l2 + l0);
    let hxx = h5 - 2.0 * h4 + h3;
    let hyy = h7 - 2.0 * h4 + h1;
    let hxy = 0.25 * (h8 - h6 - h2 + h0);
    let gl = 0.5 * vec2<f32>(dot(l5 - l3, DF_Y), dot(l7 - l1, DF_Y));
    let gh = 0.5 * vec2<f32>(dot(h5 - h3, DF_Y), dot(h7 - h1, DF_Y));
    let lev = max(dot(l4, DF_Y), 0.0) + 1e-3;
    let ik = pf(p + 18u);
    let rl = length(gl) / lev;
    let rh = length(gh) / lev;
    let el = 1.0 / (1.0 + rl * ik * rl * ik);
    let eh = 1.0 / (1.0 + rh * ik * rh * ik);
    let d0 = df_term(lxx, lyy, lxy, gl, el, pf(p + 14u));
    let d1 = df_term(lxx, lyy, lxy, gh, eh, pf(p + 15u));
    let d2 = df_term(hxx, hyy, hxy, gl, el, pf(p + 16u));
    let d3 = df_term(hxx, hyy, hxy, gh, eh, pf(p + 17u));
    let thr = pf(p + 19u);
    var gate = 1.0;
    if (thr > 0.0) { gate = smoothstep(thr, 2.0 * thr, rl); }
    var up = vec3<f32>(0.0);
    up = df_add(up, pf(p + 10u), gate, d0);
    up = df_add(up, pf(p + 11u), gate, d1);
    up = df_add(up, pf(p + 12u), gate, d2);
    up = df_add(up, pf(p + 13u), gate, d3);
    let hl = pf(p + 20u);
    let i = t.y * w + t.x;
    if (hl > 0.0) {
        let c = df_get(uo, i, false);
        up *= smoothstep(0.8 * hl, hl, max(c.x, max(c.y, c.z)));
    }
    up *= pf(p + 9u);
    if (pu(p + 8u) == 0u) { up += df_get(ao, i, false); }
    if (pu(p + 21u) != 0u) {
        up = df_clamp(uo, w, h, n, x, y, pu(p + 22u), pu(p + 23u), pf(p + 24u), up);
    }
    df_set(ao, i, false, up);",
};
