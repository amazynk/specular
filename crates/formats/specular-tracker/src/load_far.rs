//! Farandole Composer (FAR): "FAR" FEh, the song name, 0D 0A 1A, the header's length, the
//! channel map, the default tempo, 16 pan positions, the song text, 256 orders, the order
//! length and loop point, 256 pattern lengths; then the patterns (the break row, a byte
//! unused, rows of 16 channels × 4 bytes: note, sample, volume with its nibbles swapped,
//! effect and parameter in nibbles), then a 64-bit map of the samples stored and each
//! stored sample's 48-byte header and signed data.
//!
//! The clock is 32 ticks a second (tempo 80): a tempo of `t` plays a row every `t` ticks.

use crate::{
    Cell, Error, FX_FINE_VOL_DOWN, FX_FINE_VOL_UP, Kind, Module, NOTE_NONE, Pattern, VolCmd, bad,
    le16, le32, pcm8, text,
};

const CHANNELS: usize = 16;

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let head = usize::from(le16(d, 47));
    let stlen = usize::from(le16(d, 96));
    let at = 98 + stlen;
    if d.len() < at + 771 {
        return Err(bad("FAR header"));
    }
    let mut m = crate::module(Kind::Far, text(&d[4..44]), CHANNELS);
    m.tempo = 80;
    m.speed = d[75].clamp(1, 15);
    m.pans = d[76..92]
        .iter()
        .map(|&p| ((u16::from(p & 15) * 255 + 7) / 15) as u8)
        .collect();
    let orders = &d[at..at + 256];
    let length = usize::from(d[at + 257]).clamp(1, 256);
    m.restart = usize::from(d[at + 258]).min(length - 1);
    let lens: Vec<usize> = (0..256)
        .map(|k| usize::from(le16(d, at + 259 + 2 * k)))
        .collect();
    let mut p = head.max(at + 771);
    let mut index = [None; 256];
    // Vibrato depth each channel was given.
    let mut depth = [4u8; CHANNELS];
    for (k, &len) in lens.iter().enumerate() {
        if len == 0 {
            continue;
        }
        let Some(b) = d.get(p..p + len) else {
            break;
        };
        p += len;
        let stored = (len.saturating_sub(2)) / (CHANNELS * 4);
        let rows = (usize::from(b[0]) + 1).clamp(1, stored.clamp(1, 256));
        let mut cells = vec![Cell::default(); rows * CHANNELS];
        for (i, cell) in cells.iter_mut().enumerate() {
            let Some(c) = b.get(2 + 4 * i..6 + 4 * i) else {
                break;
            };
            if c[0] > 0 {
                cell.note = (c[0] + 47).min(119);
                cell.instr = c[1] + 1;
            } else {
                cell.note = NOTE_NONE;
            }
            if c[2] != 0 {
                let v = u16::from(c[2] & 15) << 4 | u16::from(c[2] >> 4);
                cell.vol = VolCmd::Set((v / 4).min(64) as u8);
            }
            let (fx, x) = (c[3] >> 4, c[3] & 15);
            let ch = i % CHANNELS;
            (cell.fx, cell.param) = match fx {
                1 => (b'F', 0xE0 | x),
                2 => (b'E', 0xE0 | x),
                3 if x > 0 => (b'G', (0x40 / x).max(1)),
                4 if x > 0 => (b'Q', x),
                5 => {
                    depth[ch] = x;
                    (0, 0)
                }
                6 | 9 if x > 0 => (b'H', x << 4 | depth[ch]),
                7 => (FX_FINE_VOL_UP, 4),
                8 => (FX_FINE_VOL_DOWN, 4),
                0xF if x > 0 => (b'A', x),
                _ => (0, 0),
            };
        }
        index[k] = Some(m.patterns.len() as u8);
        m.patterns.push(Pattern { rows, cells });
    }
    m.orders = orders[..length]
        .iter()
        .filter_map(|&o| index[usize::from(o)])
        .collect();
    let map = d.get(p..p + 8).ok_or_else(|| bad("FAR sample map"))?;
    p += 8;
    for k in 0..64 {
        if map[k / 8] & (1 << (k % 8)) == 0 {
            m.samples
                .push(crate::sample(String::new(), Vec::new(), None, 8363.0, 64));
            continue;
        }
        let Some(h) = d.get(p..p + 48) else {
            break;
        };
        let len = le32(h, 32) as usize;
        let (ls, le) = (le32(h, 38) as usize, le32(h, 42) as usize);
        let wide = h[46] & 1 != 0;
        let raw = &d[(p + 48).min(d.len())..(p + 48 + len).min(d.len())];
        p += 48 + len;
        let (data, div) = if wide {
            (crate::pcm16le(raw), 2)
        } else {
            (pcm8(raw, false), 1)
        };
        let lp = (h[47] & 8 != 0 && le > ls).then_some((ls / div, le / div));
        m.samples
            .push(crate::sample(text(&h[..32]), data, lp, 8363.0, 64));
    }
    if m.orders.is_empty() {
        return Err(bad("FAR orders"));
    }
    Ok(m)
}
