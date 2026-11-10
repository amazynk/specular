//! Deinterlacing of interlaced video shown on its own (AVCHD and broadcast 1080i/576i), at the
//! frame rate: motion-adaptive.
//!
//! A frame holds two fields woven together. The top field (even lines) is kept as it is. Each
//! line of the bottom field is woven where the picture stands still (full vertical detail)
//! and rebuilt from the top field where it moves (no combing):
//!
//! - Motion: how much the bottom field's pixel changed since the previous frame, and how much
//!   the top field's pixels above and below did (the larger of the two, the most of three
//!   neighbours across). Without a previous frame (the first after a seek, a still) every
//!   line is rebuilt: the comb alone missed the other field's picture where it lay within its
//!   neighbours' range, and left it as a ghost.
//! - The rebuilt pixel is an edge-directed average of the lines above and below (the best
//!   of three directions), kept within the range of the vertical neighbours so a wrong
//!   direction can't add detail that isn't there.
//! - Between "still" and "moving" the two blend smoothly, so the switch never flickers.
//!
//! Each plane is treated on its own (chroma lines of 4:2:0 interlaced video alternate fields
//! like luma lines).

use specular_hwdec::video::VideoFrame;

/// Below this motion (8-bit steps) a pixel is woven; above `HIGH` rebuilt; blended between.
const LOW: i32 = 6;
const HIGH: i32 = 22;

/// The planes of a frame as rows of samples: width, height, samples (8 or 16 bits).
fn plane_dims(f: &VideoFrame, i: usize) -> (usize, usize) {
    let (w, h) = (f.width as usize, f.height as usize);
    if i == 0 {
        return (w, h);
    }
    let (cw, ch) = f.chroma.plane_size(f.width, f.height);
    (cw as usize, ch as usize)
}

/// Deinterlaces `cur` in place; `prev` is the previous frame as decoded (woven, not
/// deinterlaced), when there is one of the same size.
pub fn deinterlace(cur: &mut VideoFrame, prev: Option<&VideoFrame>) {
    let wide = cur.bit_depth > 8;
    let shift = u32::from(cur.bit_depth.saturating_sub(8));
    let prev = prev.filter(|p| {
        p.width == cur.width
            && p.height == cur.height
            && p.chroma == cur.chroma
            && p.bit_depth == cur.bit_depth
    });
    for i in 0..3 {
        let (w, h) = plane_dims(cur, i);
        if h < 3 || w == 0 {
            continue;
        }
        let before = prev.map(|p| p.planes[i].as_slice());
        if wide {
            let read = |b: &[u8]| -> Vec<i32> {
                b.chunks_exact(2)
                    .map(|c| i32::from(u16::from_le_bytes([c[0], c[1]])))
                    .collect()
            };
            let mut px = read(&cur.planes[i]);
            if px.len() < w * h {
                continue;
            }
            let old = before.map(read).filter(|o| o.len() >= w * h);
            plane(&mut px, old.as_deref(), w, h, shift);
            for (d, v) in cur.planes[i].chunks_exact_mut(2).zip(&px) {
                d.copy_from_slice(&(*v as u16).to_le_bytes());
            }
        } else {
            let mut px: Vec<i32> = cur.planes[i].iter().map(|&v| i32::from(v)).collect();
            if px.len() < w * h {
                continue;
            }
            let old: Option<Vec<i32>> = before
                .filter(|o| o.len() >= w * h)
                .map(|o| o.iter().map(|&v| i32::from(v)).collect());
            plane(&mut px, old.as_deref(), w, h, shift);
            for (d, v) in cur.planes[i].iter_mut().zip(&px) {
                *d = *v as u8;
            }
        }
    }
}

/// One plane (`w`×`h` samples, row-major): the odd lines rebuilt where they move.
fn plane(px: &mut [i32], prev: Option<&[i32]>, w: usize, h: usize, shift: u32) {
    let (low, high) = (LOW << shift, HIGH << shift);
    let span = (high - low).max(1);
    let mut motion = vec![0i32; w];
    let mut out = vec![0i32; w];
    for y in (1..h).step_by(2) {
        let above = &px[(y - 1) * w..y * w];
        // The last line of an even-height plane has no line below: the one above stands in.
        let below_row = if y + 1 < h { y + 1 } else { y - 1 };
        let below = &px[below_row * w..(below_row + 1) * w];
        let line = &px[y * w..(y + 1) * w];
        // Motion of each pixel.
        for x in 0..w {
            let (a, b, c) = (above[x], below[x], line[x]);
            motion[x] = match prev {
                Some(p) => {
                    let pa = p[(y - 1) * w + x];
                    let pb = p[below_row * w + x];
                    let pc = p[y * w + x];
                    (c - pc).abs().max(((a - pa).abs() + (b - pb).abs()) / 2)
                }
                // No previous frame (the first after a seek, the still a thumbnail or export
                // shows): nothing tells a still detail from the other field's moment, so every
                // line is rebuilt. Weaving pixels inside their neighbours' range left the other
                // field's picture as a ghost (an interlaced MPEG-2 clip's time code showed two
                // readings; the visual review).
                None => high,
            };
        }
        for x in 0..w {
            let m = motion[x]
                .max(motion[x.saturating_sub(1)])
                .max(motion[(x + 1).min(w - 1)]);
            let c = line[x];
            if m <= low {
                out[x] = c;
                continue;
            }
            // Edge-directed average of the lines above and below.
            let at = |r: &[i32], dx: isize| r[(x as isize + dx).clamp(0, w as isize - 1) as usize];
            let mut best = (above[x] - below[x]).abs();
            let mut v = (above[x] + below[x] + 1) >> 1;
            for d in [-1isize, 1] {
                let (p, q) = (at(above, d), at(below, -d));
                let diff = (p - q).abs();
                if diff < best {
                    best = diff;
                    v = (p + q + 1) >> 1;
                }
            }
            let v = v.clamp(above[x].min(below[x]), above[x].max(below[x]));
            out[x] = if m >= high {
                v
            } else {
                c + ((v - c) * (m - low) + span / 2) / span
            };
        }
        px[y * w..(y + 1) * w].copy_from_slice(&out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_hwdec::video::Chroma;

    fn frame(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> VideoFrame {
        let y: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect();
        let c = vec![128u8; (w.div_ceil(2) * h.div_ceil(2)) as usize];
        VideoFrame {
            width: w,
            height: h,
            bit_depth: 8,
            chroma: Chroma::Yuv420,
            planes: [y, c.clone(), c],
            signal: None,
        }
    }

    /// A bar moving between the fields: the bottom field's copy is displaced, which combs.
    fn combed(w: u32, h: u32, at: u32) -> VideoFrame {
        frame(w, h, |x, y| {
            let pos = if y % 2 == 0 { at } else { at + 6 };
            if (pos..pos + 8).contains(&x) { 235 } else { 16 }
        })
    }

    fn comb_energy(f: &VideoFrame) -> u64 {
        let w = f.width as usize;
        let p = &f.planes[0];
        (1..f.height as usize - 1)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                let (a, b, c) = (p[(y - 1) * w + x], p[(y + 1) * w + x], p[y * w + x]);
                let d = i32::from(c) * 2 - i32::from(a) - i32::from(b);
                d.unsigned_abs() as u64
            })
            .sum()
    }

    #[test]
    fn moving_edges_lose_their_comb() {
        let mut f = combed(64, 16, 20);
        let before = comb_energy(&f);
        deinterlace(&mut f, None);
        assert!(
            comb_energy(&f) * 10 < before,
            "{} → {}",
            before,
            comb_energy(&f)
        );
        // The top field is untouched.
        let orig = combed(64, 16, 20);
        for y in (0..16).step_by(2) {
            assert_eq!(
                f.planes[0][y * 64..y * 64 + 64],
                orig.planes[0][y * 64..y * 64 + 64]
            );
        }
        // With the previous frame moving too, the same.
        let prev = combed(64, 16, 10);
        let mut g = combed(64, 16, 20);
        deinterlace(&mut g, Some(&prev));
        assert!(comb_energy(&g) * 10 < before);
    }

    #[test]
    fn still_detail_is_woven() {
        // Fine horizontal stripes that stand still: full vertical detail is kept.
        let stripes = |_x: u32, y: u32| if y.is_multiple_of(2) { 60 } else { 200 };
        let prev = frame(32, 8, stripes);
        let mut f = frame(32, 8, stripes);
        deinterlace(&mut f, Some(&prev));
        assert_eq!(f.planes[0], frame(32, 8, stripes).planes[0]);
        // A flat picture is unchanged with or without a previous frame.
        let mut flat = frame(32, 8, |_, _| 90);
        deinterlace(&mut flat, None);
        assert!(flat.planes[0].iter().all(|&v| v == 90));
    }

    /// The first frame: the bottom field's picture of another moment (a time code one
    /// reading on) lies inside its neighbours' range, so the comb test wove it in as a ghost.
    /// Every bottom line is rebuilt from the top field now.
    #[test]
    fn first_frame_keeps_no_ghost_of_the_other_field() {
        let (w, h) = (48u32, 16u32);
        // Top field: a vertical ramp. Bottom field: between its neighbours, but nearer the
        // lower one on a block the top field does not have.
        let top = |y: u32| 40 + y * 8;
        let mut f = frame(w, h, |x, y| {
            if y % 2 == 0 {
                top(y) as u8
            } else {
                let (a, b) = (top(y - 1), top((y + 1).min(h - 2)));
                if (16..32).contains(&x) {
                    ((a + 3 * b) / 4) as u8
                } else {
                    a.midpoint(b) as u8
                }
            }
        });
        deinterlace(&mut f, None);
        for y in (1..h as usize - 1).step_by(2) {
            let (a, b) = (top(y as u32 - 1), top(y as u32 + 1));
            for x in 0..w as usize {
                let v = u32::from(f.planes[0][y * w as usize + x]);
                assert!(v.abs_diff(a.midpoint(b)) <= 1, "({x}, {y}): {v}");
            }
        }
    }

    #[test]
    fn ten_bit_and_odd_sizes() {
        let mut f = combed(33, 9, 4);
        f.bit_depth = 10;
        for p in &mut f.planes {
            *p = p
                .iter()
                .flat_map(|&v| (u16::from(v) << 2).to_le_bytes())
                .collect();
        }
        deinterlace(&mut f, None);
        assert_eq!(f.planes[0].len(), 33 * 9 * 2);
    }
}
