//! Composer 669 ("if") and UNIS 669 ("JN"): a 0x1F1-byte header (message, counts, the
//! order, speed and break lists), 25-byte sample headers, patterns of 64 rows × 8 channels
//! of 3-byte cells, then unsigned 8-bit samples.
//!
//! Each order has its own speed and its own last row, so every order gets its own copy of
//! its pattern, cut after the break row, with the speed set on its first row. A 669 effect
//! goes on until another note or effect on its channel: it is repeated on the rows after it.
//! The clock is the PC timer divided by 32768 (36.4 ticks a second, tempo 91; the notes call
//! it 32 Hz); samples play at 8740 Hz on C-4.

use crate::{Cell, Error, Kind, Module, NOTE_NONE, Pattern, VolCmd, bad, le32, pcm8, text};

const HEADER: usize = 0x1F1;
const CHANNELS: usize = 8;
const PATTERN: usize = 64 * CHANNELS * 3;

pub(crate) fn sniff(d: &[u8]) -> bool {
    if d.len() < HEADER || !(d.starts_with(b"if") || d.starts_with(b"JN")) {
        return false;
    }
    let (nos, nop) = (usize::from(d[0x6E]), usize::from(d[0x6F]));
    nos <= 64
        && (1..=128).contains(&nop)
        && d[0x70] < 128
        && d[0x71..0xF1]
            .iter()
            .all(|&o| o == 0xFF || usize::from(o) < nop)
        && d[0x71] != 0xFF
        && d[0x171..0x1F1].iter().all(|&b| b < 64)
        && d.len() >= HEADER + nos * 25 + nop * PATTERN
}

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    if !sniff(d) {
        return Err(bad("669 header"));
    }
    let (nos, nop) = (usize::from(d[0x6E]), usize::from(d[0x6F]));
    let title = text(&d[2..38]);
    let mut m = crate::module(Kind::Six69, title, CHANNELS);
    m.tempo = 91;
    m.pans = (0..CHANNELS)
        .map(|c| if c % 2 == 0 { 48 } else { 208 })
        .collect();
    let pat_at = HEADER + nos * 25;
    let mut data_at = pat_at + nop * PATTERN;
    for k in 0..nos {
        let h = &d[HEADER + 25 * k..HEADER + 25 * (k + 1)];
        let len = le32(h, 13) as usize;
        let (ls, le) = (le32(h, 17) as usize, le32(h, 21) as usize);
        let avail = d.len().saturating_sub(data_at).min(len);
        let data = pcm8(&d[data_at..data_at + avail], true);
        data_at += len;
        let lp = (le < 0xFFFFF && le > ls && le <= len).then_some((ls, le));
        m.samples
            .push(crate::sample(text(&h[..13]), data, lp, 8740.0, 64));
    }
    // The effect each channel goes on with.
    let mut going = [(0u8, 0u8); CHANNELS];
    for o in 0..128 {
        let p = d[0x71 + o];
        if p == 0xFF {
            break;
        }
        let speed = d[0xF1 + o];
        let rows = usize::from(d[0x171 + o]) + 1;
        let at = pat_at + usize::from(p) * PATTERN;
        let mut cells = vec![Cell::default(); rows * CHANNELS];
        for r in 0..rows {
            for c in 0..CHANNELS {
                let b = &d[at + 3 * (r * CHANNELS + c)..at + 3 * (r * CHANNELS + c) + 3];
                let cell = &mut cells[r * CHANNELS + c];
                match b[0] {
                    0xFF => {}
                    0xFE => cell.vol = VolCmd::Set(volume(b[1])),
                    n => {
                        cell.note = (n >> 2) + 36;
                        cell.instr = ((n & 3) << 4 | b[1] >> 4) + 1;
                        cell.vol = VolCmd::Set(volume(b[1]));
                        going[c] = (0, 0);
                    }
                }
                if b[2] != 0xFF {
                    let (fx, x) = (b[2] >> 4, b[2] & 15);
                    let e = match fx {
                        0 => (b'F', x),
                        1 => (b'E', x),
                        2 => (b'G', x),
                        3 => (b'E', 0xF1),
                        4 => (b'H', x << 4 | 1),
                        5 if x > 0 => (b'A', x),
                        7 => (b'Q', x),
                        _ => (0, 0),
                    };
                    // A value of 0 (and a speed change) ends what went on.
                    going[c] = if x == 0 || matches!(fx, 3 | 5 | 7) {
                        (0, 0)
                    } else {
                        e
                    };
                    (cell.fx, cell.param) = e;
                } else if going[c].0 != 0 && cell.note == NOTE_NONE {
                    (cell.fx, cell.param) = going[c];
                }
            }
        }
        // The order's speed, unless its first row sets one.
        if speed > 0 && !cells[..CHANNELS].iter().any(|c| c.fx == b'A') {
            if let Some(free) = cells[..CHANNELS].iter_mut().find(|c| c.fx == 0) {
                (free.fx, free.param) = (b'A', speed);
            } else {
                cells[CHANNELS - 1].fx = b'A';
                cells[CHANNELS - 1].param = speed;
            }
        }
        m.orders.push(m.patterns.len() as u8);
        m.patterns.push(Pattern { rows, cells });
    }
    m.speed = d[0xF1].max(1);
    m.restart = usize::from(d[0x70]).min(m.orders.len().saturating_sub(1));
    Ok(m)
}

/// A 669 volume (0–15) on the 0–64 scale.
fn volume(b: u8) -> u8 {
    ((u16::from(b & 15) * 64 + 7) / 15) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tiny_669_loads() {
        let mut d = vec![0u8; HEADER + 25 + PATTERN + 4];
        d[..2].copy_from_slice(b"if");
        d[0x6E] = 1;
        d[0x6F] = 1;
        d[0x71] = 0;
        d[0x72..0xF1].fill(0xFF);
        d[0xF1] = 4;
        d[0x171] = 3;
        d[HEADER + 13] = 4;
        let p = HEADER + 25;
        for k in 0..64 * CHANNELS {
            d[p + 3 * k] = 0xFF;
            d[p + 3 * k + 2] = 0xFF;
        }
        d[p] = 24 << 2;
        d[p + 1] = 0x0F;
        d[p + 2] = 0x42;
        assert!(sniff(&d));
        let m = load(&d).unwrap();
        assert_eq!(m.orders, vec![0]);
        assert_eq!(m.patterns[0].rows, 4);
        assert_eq!(m.patterns[0].cells[0].note, 60);
        assert_eq!(m.patterns[0].cells[0].fx, b'H');
        // The vibrato goes on below.
        assert_eq!(m.patterns[0].cells[CHANNELS].fx, b'H');
        assert_eq!(m.samples[0].data.len(), 4);
    }
}
