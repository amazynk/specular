//! Oktalyzer (OKT): "OKTASONG" then IFF-style chunks: CMOD (four channel modes, a split
//! channel plays two voices), SAMP (32-byte sample headers), SPEE (speed), SLEN (patterns),
//! PLEN and PATT (the orders), a PBOD a pattern (its row count, then 4-byte cells: note
//! 1–36, sample, effect, data) and an SBOD for each sample that has data (signed 8-bit).
//!
//! The clock is the Amiga's 50 Hz; notes are ProTracker's three octaves from C-1. Effects
//! are numbered: 1/2 portamento, 10–12 arpeggios, 13/17/21/30 note slides, 25 position
//! jump, 28 speed, 31 volume and its slides.

use crate::{
    Cell, Error, FX_FINE_VOL_DOWN, FX_FINE_VOL_UP, FX_MOD_VOLSLIDE, FX_SET_VOLUME, Kind, Module,
    NOTE_NONE, Pattern, bad, be16, be32, pcm8, text,
};

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let mut at = 8;
    let mut modes = [0u16; 4];
    let mut heads: Vec<&[u8]> = Vec::new();
    let mut orders_n = 0usize;
    let mut patt: &[u8] = &[];
    let mut speed = 6u16;
    let mut bodies: Vec<&[u8]> = Vec::new();
    let mut sbods: Vec<&[u8]> = Vec::new();
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let len = be32(d, at + 4) as usize;
        let body = &d[at + 8..(at + 8 + len).min(d.len())];
        match id {
            b"CMOD" => {
                for (k, m) in modes.iter_mut().enumerate() {
                    *m = be16(body, 2 * k) & 1;
                }
            }
            b"SAMP" => heads = body.chunks_exact(32).collect(),
            b"SPEE" => speed = be16(body, 0),
            b"PLEN" => orders_n = usize::from(be16(body, 0)),
            b"PATT" => patt = body,
            b"PBOD" => bodies.push(body),
            b"SBOD" => sbods.push(body),
            _ => {}
        }
        at += 8 + len;
    }
    let channels: usize = modes.iter().map(|&m| 1 + usize::from(m)).sum();
    let mut m = crate::module(Kind::Okt, String::new(), channels);
    // Each Amiga channel's voices on its side: left, right, right, left.
    m.pans = modes
        .iter()
        .enumerate()
        .flat_map(|(k, &md)| {
            let p = if k == 0 || k == 3 { 0 } else { 255 };
            vec![p; 1 + usize::from(md)]
        })
        .collect();
    m.speed = (speed as u8).max(1);
    m.orders = patt
        .iter()
        .take(orders_n.min(128))
        .copied()
        .filter(|&o| usize::from(o) < bodies.len())
        .collect();
    let mut sb = sbods.into_iter();
    for h in &heads {
        let len = be32(h, 20) as usize;
        let (rs, rl) = (usize::from(be16(h, 24)) * 2, usize::from(be16(h, 26)) * 2);
        let data = if len > 0 {
            sb.next()
                .map_or_else(Vec::new, |b| pcm8(&b[..len.min(b.len())], false))
        } else {
            Vec::new()
        };
        let lp = (rl > 2).then_some((rs, rs + rl));
        m.samples
            .push(crate::sample(text(&h[..20]), data, lp, 8363.0, h[29]));
    }
    for b in &bodies {
        let rows = usize::from(be16(b, 0)).clamp(1, 256);
        let mut cells = vec![Cell::default(); rows * channels];
        for (k, cell) in cells.iter_mut().enumerate() {
            let Some(c) = b.get(2 + 4 * k..6 + 4 * k) else {
                break;
            };
            if c[0] > 0 {
                cell.note = (c[0] + 47).min(119);
                cell.instr = c[1] + 1;
            } else {
                cell.note = NOTE_NONE;
            }
            let x = c[3];
            (cell.fx, cell.param) = match c[2] {
                1 => (b'F', x),
                2 => (b'E', x),
                10..=12 => (b'J', x),
                13 => (b'E', 0xE0 | (x & 15)),
                17 => (b'F', 0xE0 | (x & 15)),
                25 => (b'B', x),
                28 if x > 0 => (b'A', x),
                31 => match x {
                    0..=0x40 => (FX_SET_VOLUME, x),
                    0x41..=0x50 => (FX_MOD_VOLSLIDE, x - 0x40),
                    0x51..=0x60 => (FX_MOD_VOLSLIDE, (x - 0x50) << 4),
                    0x61..=0x70 => (FX_FINE_VOL_DOWN, x - 0x60),
                    0x71..=0x80 => (FX_FINE_VOL_UP, x - 0x70),
                    _ => (0, 0),
                },
                _ => (0, 0),
            };
        }
        m.patterns.push(Pattern { rows, cells });
    }
    if m.orders.is_empty() {
        return Err(bad("Oktalyzer orders"));
    }
    Ok(m)
}
