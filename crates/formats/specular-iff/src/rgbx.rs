//! Impulse Turbo Silver `RGB8` and `RGBN` bodies (compression 4).
//!
//! The body is a run of pixel groups in raster order. `RGB8`: four bytes, red, green, blue,
//! then the genlock bit (bit 7) and a 7-bit repeat count. `RGBN`: one big-endian word, 4-bit
//! red, green and blue (bits 15–4), the genlock bit (bit 3) and a 3-bit count. A count of 0
//! means the count is the next byte, and if that is 0 too, the next big-endian word.
//! The 4-bit values of `RGBN` widen by replication (`0xA` → `0xAA`); genlock pixels (where
//! the video signal was to show through) are transparent.

use crate::{GREY, Image, Info};

pub(crate) fn decode(info: &Info, body: &[u8]) -> Image {
    let n = info.width as usize * info.height as usize;
    let mut px = Vec::with_capacity(n * 4);
    let rgbn = info.form == crate::Form::Rgbn;
    let mut i = 0;
    while px.len() < n * 4 {
        let (rgba, count) = if rgbn {
            let Some(w) = body.get(i..i + 2) else { break };
            i += 2;
            let v = u16::from_be_bytes([w[0], w[1]]);
            let c = |s: u16| (((v >> s) & 15) * 17) as u8;
            let a = if v & 8 != 0 { 0 } else { 255 };
            ([c(12), c(8), c(4), a], usize::from(v & 7))
        } else {
            let Some(g) = body.get(i..i + 4) else { break };
            i += 4;
            let a = if g[3] & 0x80 != 0 { 0 } else { 255 };
            ([g[0], g[1], g[2], a], usize::from(g[3] & 0x7f))
        };
        let count = if count > 0 {
            count
        } else {
            match body.get(i) {
                Some(&c) if c > 0 => {
                    i += 1;
                    usize::from(c)
                }
                Some(_) => {
                    let Some(w) = body.get(i + 1..i + 3) else {
                        break;
                    };
                    i += 3;
                    usize::from(u16::from_be_bytes([w[0], w[1]]))
                }
                None => break,
            }
        };
        let left = n - px.len() / 4;
        for _ in 0..count.min(left) {
            px.extend_from_slice(&rgba);
        }
    }
    let truncated = px.len() < n * 4;
    while px.len() < n * 4 {
        px.extend_from_slice(&[GREY, GREY, GREY, 255]);
    }
    Image {
        width: info.width,
        height: info.height,
        pixels: px,
        truncated,
    }
}
