//! Suggested burned-in text regions: a simple detector of high-contrast, text-like areas near
//! the image edges (ultrasound header bars, secondary-capture captions, side panels of
//! acquisition parameters). It suggests rectangles for the user to check; it reads nothing.
//!
//! Method: the frame becomes 8-bit luma; 8 × 8 cells count strong horizontal and vertical
//! steps (glyph strokes have both, smooth anatomy and speckle have few strong ones); cells
//! with enough of both inside the outer band of the image are text-like; text-like cells are
//! joined across word gaps, grouped, and each group's padded bounding box is a suggestion.

use crate::mask::Rect;
use crate::pixel::{Frame, Pixels};

const CELL: usize = 8;
/// A step at least this large (of 255) is a stroke edge.
const STEP: i32 = 60;
/// Strong steps each way a text-like cell needs (of 56 per direction).
const MIN_STEPS: u32 = 5;
/// Outer band, as a fraction of the size, where text is looked for.
const BAND: f32 = 0.22;
/// Cells bridged between words.
const GAP: usize = 2;
const PAD: u32 = 3;
const MAX_RECTS: usize = 24;

/// 8-bit luma of a frame, stretched to its value range.
pub fn luma(f: &Frame) -> Vec<u8> {
    let n = (f.width * f.height) as usize;
    match &f.pixels {
        Pixels::Grey(g) => {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for i in 0..g.len() {
                let v = g.get_f64(i).unwrap_or(0.0);
                lo = lo.min(v);
                hi = hi.max(v);
            }
            let scale = if hi > lo { 255.0 / (hi - lo) } else { 0.0 };
            (0..n)
                .map(|i| ((g.get_f64(i).unwrap_or(lo) - lo) * scale) as u8)
                .collect()
        }
        Pixels::Rgb8(v) => v
            .chunks_exact(3)
            .map(|p| ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8)
            .collect(),
        Pixels::Rgb16(v) => {
            let shift = f.bits.saturating_sub(8) as u32;
            v.chunks_exact(3)
                .map(|p| {
                    let y = (p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8;
                    (y >> shift).min(255) as u8
                })
                .collect()
        }
    }
}

/// Suggested text rectangles of a `w` × `h` luma image.
pub fn suggest_luma(l: &[u8], w: usize, h: usize) -> Vec<Rect> {
    if w < CELL * 2 || h < CELL * 2 || l.len() < w * h {
        return Vec::new();
    }
    let (cw, ch) = (w / CELL, h / CELL);
    let mut text = vec![false; cw * ch];
    let band = |c: usize, n: usize| {
        let f = (c as f32 + 0.5) / n as f32;
        f < BAND || f > 1.0 - BAND
    };
    for cy in 0..ch {
        for cx in 0..cw {
            if !band(cx, cw) && !band(cy, ch) {
                continue;
            }
            let (mut hs, mut vs) = (0u32, 0u32);
            let (mut lo, mut hi) = (255u8, 0u8);
            for y in cy * CELL..cy * CELL + CELL {
                let row = &l[y * w..y * w + w];
                for x in cx * CELL..cx * CELL + CELL {
                    let p = row[x];
                    lo = lo.min(p);
                    hi = hi.max(p);
                    if x + 1 < w && (row[x + 1] as i32 - p as i32).abs() >= STEP {
                        hs += 1;
                    }
                    if y + 1 < h && (l[(y + 1) * w + x] as i32 - p as i32).abs() >= STEP {
                        vs += 1;
                    }
                }
            }
            // Strokes on both axes; not a field of noise (which steps nearly everywhere).
            text[cy * cw + cx] = hs >= MIN_STEPS && vs >= MIN_STEPS && hs + vs < 50 && hi > lo;
        }
    }
    // Bridge word gaps along rows.
    let mut joined = text.clone();
    for cy in 0..ch {
        let mut last: Option<usize> = None;
        for cx in 0..cw {
            if text[cy * cw + cx] {
                if let Some(p) = last
                    && cx - p > 1
                    && cx - p <= GAP + 1
                {
                    for x in p + 1..cx {
                        joined[cy * cw + x] = true;
                    }
                }
                last = Some(cx);
            }
        }
    }
    // Groups (4-connected).
    let mut seen = vec![false; cw * ch];
    let mut rects = Vec::new();
    let mut stack = Vec::new();
    for start in 0..cw * ch {
        if !joined[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        let mut cells = 0;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % cw, i / cw);
            cells += text[i] as usize;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            let mut push = |j: usize| {
                if joined[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < cw {
                push(i + 1);
            }
            if y > 0 {
                push(i - cw);
            }
            if y + 1 < ch {
                push(i + cw);
            }
        }
        if cells < 2 {
            continue;
        }
        let px0 = (x0 * CELL) as u32;
        let py0 = (y0 * CELL) as u32;
        let px1 = (((x1 + 1) * CELL) as u32 + PAD).min(w as u32);
        let py1 = (((y1 + 1) * CELL) as u32 + PAD).min(h as u32);
        let (px0, py0) = (px0.saturating_sub(PAD), py0.saturating_sub(PAD));
        rects.push(Rect::new(px0, py0, px1 - px0, py1 - py0));
    }
    merge(&mut rects);
    // Largest first; a whole-image "suggestion" is no suggestion.
    rects.retain(|r| (r.w as usize * r.h as usize) < w * h / 3);
    rects.sort_by_key(|r| std::cmp::Reverse(r.w as u64 * r.h as u64));
    rects.truncate(MAX_RECTS);
    rects
}

/// Suggested text rectangles of a frame.
pub fn suggest(f: &Frame) -> Vec<Rect> {
    suggest_luma(&luma(f), f.width as usize, f.height as usize)
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h
}

/// Merges overlapping or touching rectangles into their bounding boxes.
fn merge(rects: &mut Vec<Rect>) {
    let mut changed = true;
    while changed {
        changed = false;
        'outer: for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                if overlaps(&rects[i], &rects[j]) {
                    let (a, b) = (rects[i], rects[j]);
                    let x0 = a.x.min(b.x);
                    let y0 = a.y.min(b.y);
                    let x1 = (a.x + a.w).max(b.x + b.w);
                    let y1 = (a.y + a.h).max(b.y + b.h);
                    rects[i] = Rect::new(x0, y0, x1 - x0, y1 - y0);
                    rects.swap_remove(j);
                    changed = true;
                    break 'outer;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 5 × 7 glyph-like pattern (a box with a bar) drawn bright at (x, y).
    fn glyph(l: &mut [u8], w: usize, x: usize, y: usize, k: usize) {
        for dy in 0..7 {
            for dx in 0..5 {
                let on =
                    dy == 0 || dy == 6 || dx == 0 || (dx == 4 && k.is_multiple_of(2)) || dy == 3;
                if on {
                    l[(y + dy) * w + x + dx] = 230;
                }
            }
        }
    }

    fn smooth(w: usize, h: usize) -> Vec<u8> {
        (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                (90 + (x * 40 / w) + (y * 30 / h)) as u8
            })
            .collect()
    }

    #[test]
    fn finds_a_header_bar() {
        let (w, h) = (320, 240);
        let mut l = smooth(w, h);
        // A dark bar with a line of text at the top.
        for y in 0..20 {
            l[y * w..y * w + w].fill(10);
        }
        for k in 0..20 {
            glyph(&mut l, w, 12 + k * 7, 6, k);
        }
        let r = suggest_luma(&l, w, h);
        assert_eq!(r.len(), 1, "{r:?}");
        let r = r[0];
        assert!(r.x <= 12 && r.x + r.w >= 12 + 20 * 7 - 2, "{r:?}");
        assert!(r.y <= 6 && r.y + r.h >= 13 && r.y + r.h <= 32, "{r:?}");
    }

    #[test]
    fn ignores_smooth_and_central_content() {
        let (w, h) = (320, 240);
        let mut l = smooth(w, h);
        assert!(suggest_luma(&l, w, h).is_empty());
        // Text in the middle of the image is not suggested (it is the anatomy's region).
        for k in 0..10 {
            glyph(&mut l, w, 120 + k * 7, 116, k);
        }
        assert!(suggest_luma(&l, w, h).is_empty());
        // Neither is a field of noise along an edge.
        let mut s = 12345u32;
        for y in 0..24 {
            for x in 0..w {
                s = s.wrapping_mul(1103515245).wrapping_add(12345);
                l[y * w + x] = (s >> 16) as u8;
            }
        }
        assert!(suggest_luma(&l, w, h).is_empty());
    }

    #[test]
    fn merges_touching() {
        let mut r = vec![
            Rect::new(0, 0, 10, 10),
            Rect::new(10, 0, 5, 5),
            Rect::new(50, 50, 1, 1),
        ];
        merge(&mut r);
        assert_eq!(r.len(), 2);
        assert!(r.contains(&Rect::new(0, 0, 15, 10)));
    }
}
