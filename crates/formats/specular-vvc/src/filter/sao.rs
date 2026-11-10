//! Sample adaptive offset (8.8.4): band and edge offsets per CTB, reading the deblocked
//! picture.

use super::{Band, SaoCtb, Shared, bands, run_jobs};
use crate::picture::Picture;

/// hPos / vPos of the two neighbours per SaoEoClass (Table 44).
const EO: [[(i32, i32); 2]; 4] = [
    [(-1, 0), (1, 0)],
    [(0, -1), (0, 1)],
    [(-1, -1), (1, 1)],
    [(1, -1), (-1, 1)],
];

/// Applies SAO to CTB (rx, ry) of plane `ci`, reading `src`, writing rows of `out`.
#[allow(clippy::too_many_arguments)]
fn ctb(
    sh: &Shared<'_>,
    src: &[u16],
    out: &mut Band<'_>,
    stride: usize,
    (pw, ph): (usize, usize),
    (sx, sy): (u32, u32),
    ci: usize,
    (rx, ry): (u32, u32),
    p: &SaoCtb,
    bd: u32,
) {
    let ty = p.type_idx[ci];
    if ty == 0 {
        return;
    }
    let cw = (1usize << sh.log2_ctb) >> sx;
    let chh = (1usize << sh.log2_ctb) >> sy;
    let x0 = rx as usize * cw;
    let y0 = ry as usize * chh;
    let x1 = (x0 + cw).min(pw);
    let y1 = (y0 + chh).min(ph);
    let max = (1i32 << bd) - 1;
    let off = |k: usize| -> i32 {
        if k == 0 {
            0
        } else {
            i32::from(p.offset[ci][k - 1])
        }
    };
    if ty == 1 {
        let shift = bd - 5;
        let mut table = [0usize; 32];
        for k in 0..4 {
            table[(k + usize::from(p.band_position[ci])) & 31] = k + 1;
        }
        for y in y0..y1 {
            if y < out.y0 || y >= out.y0 + out.rows {
                continue;
            }
            for x in x0..x1 {
                let v = i32::from(src[y * stride + x]);
                let b = table[(v >> shift) as usize & 31];
                out.data[(y - out.y0) * stride + x] = (v + off(b)).clamp(0, max) as u16;
            }
        }
        return;
    }
    let class = usize::from(p.eo_class[ci]) & 3;
    let me = sh.ctb_of((x0 as u32) << sx, (y0 as u32) << sy);
    // Whether each of the 3x3 neighbouring CTBs may be read (8.8.4.2 conditions).
    let mut ok = [[false; 3]; 3];
    for (dy, row) in ok.iter_mut().enumerate() {
        for (dx, o) in row.iter_mut().enumerate() {
            let nx = rx as i64 + dx as i64 - 1;
            let ny = ry as i64 + dy as i64 - 1;
            if nx < 0 || ny < 0 || nx >= i64::from(sh.w_ctbs) || ny >= i64::from(sh.h_ctbs) {
                continue;
            }
            *o = sh.across_ok(me, (ny as u32 * sh.w_ctbs + nx as u32) as usize);
        }
    }
    let vbx: Vec<usize> = sh.ctx.vb_x.iter().map(|&v| (v >> sx) as usize).collect();
    let vby: Vec<usize> = sh.ctx.vb_y.iter().map(|&v| (v >> sy) as usize).collect();
    let [(hx0, vy0), (hx1, vy1)] = EO[class];
    for y in y0..y1 {
        if y < out.y0 || y >= out.y0 + out.rows {
            continue;
        }
        let vb_row = class != 0 && vby.iter().any(|&b| y + 1 == b || y == b);
        for x in x0..x1 {
            if vb_row || (class != 1 && vbx.iter().any(|&b| x + 1 == b || x == b)) {
                continue;
            }
            let mut skip = false;
            let mut nb = [0i32; 2];
            for (k, &(hx, vy)) in [(hx0, vy0), (hx1, vy1)].iter().enumerate() {
                let nx = x as i64 + i64::from(hx);
                let ny = y as i64 + i64::from(vy);
                if nx < 0 || ny < 0 || nx >= pw as i64 || ny >= ph as i64 {
                    skip = true;
                    break;
                }
                let cxn = if (nx as usize) < x0 {
                    0
                } else if nx as usize >= x0 + cw {
                    2
                } else {
                    1
                };
                let cyn = if (ny as usize) < y0 {
                    0
                } else if ny as usize >= y0 + chh {
                    2
                } else {
                    1
                };
                if !ok[cyn][cxn] {
                    skip = true;
                    break;
                }
                nb[k] = i32::from(src[ny as usize * stride + nx as usize]);
            }
            if skip {
                continue;
            }
            let v = i32::from(src[y * stride + x]);
            let e = 2 + (v - nb[0]).signum() + (v - nb[1]).signum();
            let e = match e {
                0 => 1,
                1 => 2,
                2 => 0,
                e => e as usize,
            };
            if e == 0 {
                continue;
            }
            out.data[(y - out.y0) * stride + x] = (v + off(e)).clamp(0, max) as u16;
        }
    }
}

/// The sample adaptive offset process (8.8.4.1).
pub(super) fn run(pic: &mut Picture, sh: &Shared<'_>, threads: usize) {
    let ctx = sh.ctx;
    if !ctx.slices.iter().any(|s| s.sao_luma || s.sao_chroma) {
        return;
    }
    let (sx, sy) = match pic.chroma_format_idc {
        1 => (1, 1),
        2 => (1, 0),
        _ => (0, 0),
    };
    let bd = pic.bit_depth;
    for ci in 0..pic.planes.len() {
        let (csx, csy) = if ci == 0 { (0, 0) } else { (sx, sy) };
        let plane = &mut pic.planes[ci];
        let (pw, ph, stride) = (plane.w, plane.h, plane.stride);
        let src = plane.data.clone();
        let ctb_rows = (1usize << sh.log2_ctb) >> csy;
        let cuts: Vec<usize> = (0..=sh.h_ctbs as usize)
            .map(|r| (r * ctb_rows).min(ph))
            .collect();
        let jobs: Vec<(u32, Band<'_>)> = bands(&mut plane.data, stride, &cuts)
            .into_iter()
            .enumerate()
            .map(|(r, b)| (r as u32, b))
            .collect();
        let src = &src;
        run_jobs(jobs, threads, |(ry, mut band)| {
            for rx in 0..sh.w_ctbs {
                let c = (ry * sh.w_ctbs + rx) as usize;
                let Some(s) = sh.slice(c) else { continue };
                if (ci == 0 && !s.sao_luma) || (ci > 0 && !s.sao_chroma) {
                    continue;
                }
                let Some(p) = ctx.sao.get(c) else { continue };
                ctb(
                    sh,
                    src,
                    &mut band,
                    stride,
                    (pw, ph),
                    (csx, csy),
                    ci,
                    (rx, ry),
                    p,
                    bd,
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::EO;

    #[test]
    fn edge_classes_point_both_ways() {
        for c in EO {
            assert_eq!((c[0].0 + c[1].0, c[0].1 + c[1].1), (0, 0));
        }
    }
}
