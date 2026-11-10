//! UltraTracker (ULT): "MAS_UTrack_V00" and a version digit (1 to 4), the title, a message of
//! 32-byte lines (from version 2), sample headers (64 bytes, 66 from version 4 with the
//! sample's rate), 256 orders, the channel and pattern counts, pan positions (from version
//! 3), then the cells channel by channel, pattern after pattern (5 bytes each, FCh and a
//! count repeating one), then signed samples.
//!
//! A cell has two effects with a parameter each (the second's in the low byte of the word):
//! a volume (C) goes to the volume column, the other plays.

use crate::load_mod::protracker_fx;
use crate::{Cell, Error, Kind, Module, NOTE_NONE, Pattern, VolCmd, bad, le16, le32, pcm8, text};

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let version = d[14] - b'0';
    let lines = if version >= 2 { usize::from(d[47]) } else { 0 };
    let mut at = 48 + lines * 32;
    let nos = usize::from(*d.get(at).ok_or_else(|| bad("ULT header"))?);
    at += 1;
    let size = if version >= 4 { 66 } else { 64 };
    let heads: Vec<&[u8]> = (0..nos)
        .map(|k| d.get(at + k * size..at + (k + 1) * size))
        .collect::<Option<_>>()
        .ok_or_else(|| bad("ULT samples"))?;
    at += nos * size;
    let orders = d.get(at..at + 256).ok_or_else(|| bad("ULT orders"))?;
    let channels = usize::from(d.get(at + 256).copied().unwrap_or(0)) + 1;
    let patterns = usize::from(d.get(at + 257).copied().unwrap_or(0)) + 1;
    at += 258;
    let mut m = crate::module(Kind::Ult, text(&d[15..47]), channels.min(32));
    if version >= 3 {
        m.pans = (0..channels.min(32))
            .map(|c| ((u16::from(d.get(at + c).copied().unwrap_or(8) & 15) * 255 + 7) / 15) as u8)
            .collect();
        at += channels;
    }
    m.orders = orders
        .iter()
        .copied()
        .take_while(|&o| o != 0xFF)
        .filter(|&o| usize::from(o) < patterns)
        .collect();
    let mut pats = vec![vec![Cell::default(); 64 * channels]; patterns];
    for c in 0..channels {
        for pat in pats.iter_mut() {
            let mut r = 0;
            while r < 64 {
                let mut count = 1;
                if d.get(at) == Some(&0xFC) {
                    count = usize::from(d.get(at + 1).copied().unwrap_or(1)).max(1);
                    at += 2;
                }
                let Some(b) = d.get(at..at + 5) else {
                    return Err(bad("ULT patterns"));
                };
                at += 5;
                let cell = cell(b);
                for _ in 0..count {
                    if r < 64 {
                        pat[r * channels + c] = cell;
                    }
                    r += 1;
                }
            }
        }
    }
    let channels = channels.min(32);
    for pat in pats {
        let width = pat.len() / 64;
        let cells = (0..64)
            .flat_map(|r| pat[r * width..r * width + channels].to_vec())
            .collect();
        m.patterns.push(Pattern { rows: 64, cells });
    }
    for h in heads {
        let (ls, le) = (le32(h, 44) as usize, le32(h, 48) as usize);
        let (ss, se) = (le32(h, 52) as usize, le32(h, 56) as usize);
        let flags = h[61];
        let wide = flags & 4 != 0;
        let frames = se.saturating_sub(ss);
        let bytes = if wide { 2 * frames } else { frames };
        let avail = d.len().saturating_sub(at).min(bytes);
        let raw = &d[at..at + avail];
        at += bytes;
        let data = if wide {
            crate::pcm16le(raw)
        } else {
            pcm8(raw, false)
        };
        let rate = if version >= 4 {
            f64::from(le16(h, 62).max(1))
        } else {
            8363.0
        };
        let lp = (flags & 8 != 0 && le > ls).then_some((ls, le));
        let mut s = crate::sample(
            text(&h[..32]),
            data,
            lp,
            rate,
            (h[60] / 4).max(u8::from(h[60] > 0)),
        );
        if flags & 16 != 0 && s.loop_kind == crate::LoopKind::Forward {
            s.loop_kind = crate::LoopKind::PingPong;
        }
        m.samples.push(s);
    }
    Ok(m)
}

/// One effect as a Scream Tracker letter.
fn effect(fx: u8, p: u8) -> (u8, u8) {
    match fx {
        0 if p != 0 => (b'J', p),
        1 => (b'F', p.min(0xDF)),
        2 => (b'E', p.min(0xDF)),
        3 => (b'G', p),
        4 => (b'H', p),
        9 => (b'O', p.saturating_mul(4)),
        0xA => (crate::FX_MOD_VOLSLIDE, p),
        0xB => (b'X', (p & 15) * 17),
        0xC => (crate::FX_SET_VOLUME, p / 4),
        0xD => (b'C', (p >> 4) * 10 + (p & 15)),
        0xE => protracker_fx(14, p, false),
        0xF if p == 0 => (0, 0),
        0xF if p < 32 => (b'A', p),
        0xF => (b'T', p),
        _ => (0, 0),
    }
}

fn cell(b: &[u8]) -> Cell {
    let mut c = Cell {
        note: if b[0] == 0 {
            NOTE_NONE
        } else {
            (b[0] + 35).min(119)
        },
        instr: b[1],
        ..Cell::default()
    };
    let one = effect(b[2] >> 4, b[4]);
    let two = effect(b[2] & 15, b[3]);
    for (fx, p) in [one, two] {
        if fx == crate::FX_SET_VOLUME && c.vol == VolCmd::None {
            c.vol = VolCmd::Set(p.min(64));
        } else if fx != 0 && c.fx == 0 {
            (c.fx, c.param) = (fx, p);
        }
    }
    c
}
