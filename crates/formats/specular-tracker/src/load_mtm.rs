//! MultiTracker (MTM): a 66-byte header ("MTM" and version 1.x), 37-byte sample headers,
//! the order list, tracks of 3-byte cells shared between patterns, the patterns' track
//! table (32 a pattern, 0 an empty track), the comment, then unsigned 8-bit (or 16-bit)
//! samples. Notes and effects are ProTracker's.

use crate::load_mod::protracker_fx;
use crate::{Cell, Error, Kind, Module, NOTE_NONE, Pattern, bad, le16, le32, pcm8, text};

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let tracks = usize::from(le16(d, 24));
    let patterns = usize::from(d[26]) + 1;
    let orders = usize::from(d[27]) + 1;
    let comment = usize::from(le16(d, 28));
    let nos = usize::from(d[30]);
    let rows = match d[32] {
        0 => 64,
        r => usize::from(r),
    };
    let channels = usize::from(d[33]).clamp(1, 32);
    let mut m = crate::module(Kind::Mtm, text(&d[4..24]), channels);
    m.pans = d[34..34 + channels]
        .iter()
        .map(|&p| ((u16::from(p & 15) * 255 + 7) / 15) as u8)
        .collect();
    let ord_at = 66 + nos * 37;
    let trk_at = ord_at + 128;
    let seq_at = trk_at + tracks * rows * 3;
    let mut data_at = seq_at + patterns * 64 + comment;
    if d.len() < seq_at + patterns * 64 {
        return Err(bad("MTM tracks"));
    }
    m.orders = d[ord_at..ord_at + orders.min(128)].to_vec();
    for k in 0..nos {
        let h = &d[66 + 37 * k..66 + 37 * (k + 1)];
        let len = le32(h, 22) as usize;
        let (ls, le) = (le32(h, 26) as usize, le32(h, 30) as usize);
        let fine = (h[34] & 15) as i8;
        let fine = if fine > 7 { fine - 16 } else { fine };
        let wide = h[36] & 1 != 0;
        let avail = d.len().saturating_sub(data_at).min(len);
        let raw = &d[data_at..data_at + avail];
        data_at += len;
        let (data, div) = if wide {
            let v = raw
                .chunks_exact(2)
                .map(|w| f32::from((u16::from_le_bytes([w[0], w[1]]) ^ 0x8000) as i16) / 32768.0)
                .collect();
            (v, 2)
        } else {
            (pcm8(raw, true), 1)
        };
        let lp = (le > ls + 2).then_some((ls / div, le / div));
        m.samples.push(crate::sample(
            text(&h[..22]),
            data,
            lp,
            8363.0 * 2f64.powf(f64::from(fine) / 96.0),
            h[35],
        ));
    }
    for p in 0..patterns {
        let mut cells = vec![Cell::default(); rows * channels];
        for c in 0..channels {
            let t = usize::from(le16(d, seq_at + 64 * p + 2 * c));
            if t == 0 || t > tracks {
                continue;
            }
            let at = trk_at + (t - 1) * rows * 3;
            for r in 0..rows {
                let b = &d[at + 3 * r..at + 3 * r + 3];
                let pitch = b[0] >> 2;
                let (fx, param) = protracker_fx(b[1] & 15, b[2], false);
                cells[r * channels + c] = Cell {
                    // Pitch 25 plays a sample at its own rate (our note 60).
                    note: if pitch == 0 { NOTE_NONE } else { pitch + 35 },
                    instr: (b[0] & 3) << 4 | b[1] >> 4,
                    fx,
                    param,
                    ..Cell::default()
                };
            }
        }
        m.patterns.push(Pattern { rows, cells });
    }
    Ok(m)
}
