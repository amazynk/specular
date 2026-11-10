//! CD+G (karaoke graphics, the Red Book subcode channels R–W): 24-byte packets, 300 a
//! second, the ones whose command is 9 drawing on a 300×216 screen of 16 colours from a
//! 12-bit palette: memory and border presets, 6×12 tiles (normal and XOR), scrolls (preset
//! and copy, with the fine offsets of the 288×192 window), palette loads. Each "frame" given
//! to [`decode`] is a run of packets; the screen keeps what they drew.

use crate::{Pixels, Result};

pub const WIDTH: usize = 300;
pub const HEIGHT: usize = 216;
const TILE_W: usize = 6;
const TILE_H: usize = 12;

/// The screen between packets.
#[derive(Clone)]
pub struct Screen {
    px: Vec<u8>,
    palette: [[u8; 3]; 16],
    h_offset: usize,
    v_offset: usize,
}

impl Default for Screen {
    fn default() -> Screen {
        Screen {
            px: vec![0; WIDTH * HEIGHT],
            palette: [[0; 3]; 16],
            h_offset: 0,
            v_offset: 0,
        }
    }
}

impl Screen {
    fn fill(&mut self, c: u8, border_only: bool) {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let border = !(TILE_W..WIDTH - TILE_W).contains(&x)
                    || !(TILE_H..HEIGHT - TILE_H).contains(&y);
                if !border_only || border {
                    self.px[y * WIDTH + x] = c;
                }
            }
        }
    }

    fn tile(&mut self, d: &[u8], xor: bool) {
        let (c0, c1) = (d[0] & 15, d[1] & 15);
        let row = usize::from(d[2] & 0x1F);
        let col = usize::from(d[3] & 0x3F);
        if row >= HEIGHT / TILE_H || col >= WIDTH / TILE_W {
            return;
        }
        // Tiles land where the scrolled picture has moved them.
        let (x0, y0) = (col * TILE_W + self.h_offset, row * TILE_H + self.v_offset);
        for (r, &bits) in d[4..16].iter().enumerate() {
            let y = y0 + r;
            if y >= HEIGHT {
                break;
            }
            for k in 0..TILE_W {
                let x = x0 + k;
                if x >= WIDTH {
                    break;
                }
                let on = bits >> (5 - k) & 1 != 0;
                let c = if on { c1 } else { c0 };
                let i = y * WIDTH + x;
                self.px[i] = if xor { self.px[i] ^ c } else { c };
            }
        }
    }

    /// Moves the picture: a whole tile left, right, up or down, and the fine offsets'
    /// change; what comes in is the colour given (preset) or what went out (copy).
    fn scroll(&mut self, d: &[u8], copy: bool) {
        let colour = d[0] & 15;
        let (h, v) = (d[1] & 0x3F, d[2] & 0x3F);
        let h_off = usize::from(h & 7).min(TILE_W - 1);
        let v_off = usize::from(v & 15).min(TILE_H - 1);
        let mut dx = h_off as isize - self.h_offset as isize;
        // (A larger vertical offset moves the picture up, as ffmpeg's player does.)
        let mut dy = self.v_offset as isize - v_off as isize;
        self.h_offset = h_off;
        self.v_offset = v_off;
        match (h >> 4) & 3 {
            1 => dx += TILE_W as isize,
            2 => dx -= TILE_W as isize,
            _ => {}
        }
        match (v >> 4) & 3 {
            1 => dy += TILE_H as isize,
            2 => dy -= TILE_H as isize,
            _ => {}
        }
        if dx == 0 && dy == 0 {
            return;
        }
        let old = self.px.clone();
        for y in 0..HEIGHT as isize {
            for x in 0..WIDTH as isize {
                let (sx, sy) = (x - dx, y - dy);
                let inside =
                    (0..WIDTH as isize).contains(&sx) && (0..HEIGHT as isize).contains(&sy);
                let v = if inside {
                    old[sy as usize * WIDTH + sx as usize]
                } else if copy {
                    let wx = sx.rem_euclid(WIDTH as isize) as usize;
                    let wy = sy.rem_euclid(HEIGHT as isize) as usize;
                    old[wy * WIDTH + wx]
                } else {
                    colour
                };
                self.px[y as usize * WIDTH + x as usize] = v;
            }
        }
    }

    fn palette(&mut self, d: &[u8], high: bool) {
        for i in 0..8 {
            let (a, b) = (u16::from(d[2 * i] & 0x3F), u16::from(d[2 * i + 1] & 0x3F));
            let v = a << 6 | b;
            let (r, g, bl) = ((v >> 8) & 15, (v >> 4) & 15, v & 15);
            self.palette[i + if high { 8 } else { 0 }] =
                [r as u8 * 17, g as u8 * 17, bl as u8 * 17];
        }
    }

    /// Runs one packet.
    pub fn packet(&mut self, p: &[u8]) {
        if p.len() < 24 || p[0] & 0x3F != 9 {
            return;
        }
        let d: Vec<u8> = p[4..20].iter().map(|b| b & 0x3F).collect();
        match p[1] & 0x3F {
            1 => self.fill(d[0] & 15, false),
            2 => self.fill(d[0] & 15, true),
            6 => self.tile(&d, false),
            38 => self.tile(&d, true),
            20 => self.scroll(&d, false),
            24 => self.scroll(&d, true),
            30 => self.palette(&d, false),
            31 => self.palette(&d, true),
            _ => {}
        }
    }

    /// The picture as RGB.
    pub fn draw(&self, out: &mut [u8]) {
        for (i, &c) in self.px.iter().enumerate() {
            out[3 * i..3 * i + 3].copy_from_slice(&self.palette[usize::from(c & 15)]);
        }
    }
}

/// Runs a frame's packets and draws the screen on `canvas` (RGB, 300×216).
pub fn decode(data: &[u8], screen: &mut Screen, canvas: &mut Pixels) -> Result<()> {
    for p in data.chunks_exact(24) {
        screen.packet(p);
    }
    if let Pixels::Rgb24(px) = canvas
        && px.len() == WIDTH * HEIGHT * 3
    {
        screen.draw(px);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_tile_and_palette() {
        let mut s = Screen::default();
        let mut p = [0u8; 24];
        p[0] = 9;
        // Colour 1 = pure red (R=15).
        p[1] = 30;
        p[4 + 2] = 0x3C;
        p[4 + 3] = 0;
        s.packet(&p);
        assert_eq!(s.palette[1], [255, 0, 0]);
        let mut t = [0u8; 24];
        t[0] = 9;
        t[1] = 6;
        t[4] = 0;
        t[5] = 1;
        t[6] = 1;
        t[7] = 1;
        t[8..20].fill(0x3F);
        s.packet(&t);
        let mut rgb = vec![0u8; WIDTH * HEIGHT * 3];
        s.draw(&mut rgb);
        let i = 3 * (12 * WIDTH + 6);
        assert_eq!(&rgb[i..i + 3], &[255, 0, 0]);
        assert_eq!(&rgb[0..3], &[0, 0, 0]);
    }
}
