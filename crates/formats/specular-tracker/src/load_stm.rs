//! Scream Tracker 2 (STM): a 48-byte header (the tracker's name, 1Ah, type 2 for a module,
//! version 2.x, speed, pattern count, global volume), 31 sample headers of 32 bytes, 128
//! orders, patterns of 64 rows × 4 channels of 4-byte cells, then signed 8-bit samples at
//! the paragraphs their headers give.
//!
//! The speed byte keeps the ticks a row in its high nibble (from version 2.21; before, the
//! whole byte over 10). Effects are Scream Tracker's letters A–J; a cell's volume over 64
//! means none.

use crate::{Cell, Error, Kind, Module, NOTE_CUT, Pattern, VolCmd, bad, le16, pcm8, text};

const SAMPLES: usize = 48;
const ORDERS: usize = SAMPLES + 31 * 32;
const PATTERNS: usize = ORDERS + 128;

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    if d.len() < PATTERNS {
        return Err(bad("STM header"));
    }
    let minor = d[31];
    let speed = |b: u8| if minor >= 21 { b >> 4 } else { b / 10 };
    let patterns = usize::from(d[33]).min(64);
    let mut m = crate::module(Kind::Stm, text(&d[..20]), 4);
    m.speed = speed(d[32]).max(1);
    m.global_volume = d[34].min(64) * 2;
    m.fx_memory = false;
    m.pans = vec![48, 208, 48, 208];
    m.orders = d[ORDERS..ORDERS + 128]
        .iter()
        .copied()
        .take_while(|&o| usize::from(o) < patterns)
        .collect();
    for k in 0..31 {
        let h = &d[SAMPLES + 32 * k..SAMPLES + 32 * (k + 1)];
        let at = usize::from(le16(h, 14)) * 16;
        let len = usize::from(le16(h, 16));
        let (ls, le) = (usize::from(le16(h, 18)), usize::from(le16(h, 20)));
        let data = d
            .get(at..(at + len).min(d.len()))
            .map_or_else(Vec::new, |b| pcm8(b, false));
        let lp = (le != 0xFFFF && le > ls).then_some((ls, le));
        m.samples.push(crate::sample(
            text(&h[..12]),
            data,
            lp,
            f64::from(le16(h, 24).max(1)),
            h[22],
        ));
    }
    for p in 0..patterns {
        let at = PATTERNS + p * 1024;
        let mut cells = vec![Cell::default(); 64 * 4];
        for (k, cell) in cells.iter_mut().enumerate() {
            let Some(b) = d.get(at + 4 * k..at + 4 * k + 4) else {
                break;
            };
            match b[0] {
                251..=255 => {
                    if b[0] == 252 || b[0] == 254 {
                        cell.note = NOTE_CUT;
                    }
                    continue;
                }
                n => cell.note = ((n >> 4) * 12 + (n & 15) + 36).min(119),
            }
            cell.instr = b[1] >> 3;
            let vol = (b[1] & 7) | (b[2] & 0xF0) >> 1;
            if vol <= 64 {
                cell.vol = VolCmd::Set(vol);
            }
            let (fx, x) = (b[2] & 15, b[3]);
            (cell.fx, cell.param) = match fx {
                1 if speed(x) > 0 => (b'A', speed(x)),
                2 => (b'B', x),
                3 => (b'C', 0),
                4 => (b'D', x),
                5 => (b'E', x),
                6 => (b'F', x),
                7 => (b'G', x),
                8 => (b'H', x),
                9 => (b'I', x),
                10 => (b'J', x),
                _ => (0, 0),
            };
        }
        m.patterns.push(Pattern { rows: 64, cells });
    }
    if m.orders.is_empty() {
        return Err(bad("STM orders"));
    }
    Ok(m)
}
