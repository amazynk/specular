//! MED / OctaMED modules (MMD0 to MMD3), from Teijo Kinnunen's MMD documentation: a header
//! of big-endian pointers (offsets in the file) to the song, the block table, the
//! instrument table and the expansion data.
//!
//! - The song holds 63 sample entries (loop in words, volume, transpose), the block count,
//!   the play sequence (MMD0/MMD1; MMD2/MMD3 have sections of play sequences instead), the
//!   tempos, transpose, flags, track volumes and the instrument count.
//! - An MMD0 block is a track count, a line count and 3-byte notes; MMD1 and later have
//!   16-bit counts, a block info pointer and 4-byte notes.
//! - Instruments are a length and a type: samples (8- or 16-bit, mono or stereo; the
//!   multi-octave IFF ones play their longest octave) are read; synthetic and hybrid sounds
//!   are silent.
//!
//! Notes: 1 is C-1; the Amiga modes play a sample at its own rate on C-2, mixing mode on C-4.
//!
//! Timing: in BPM mode the tempo is beats a minute of a set number of lines, otherwise
//! tempos 1–10 are SoundTracker speeds and higher ones MED's timer (33 is 50 Hz); the
//! secondary tempo is the ticks a line.

use crate::{
    Cell, Error, FX_FINE_VOL_DOWN, FX_FINE_VOL_UP, FX_MOD_VOLSLIDE, FX_SET_VOLUME, Kind, Module,
    NOTE_NONE, Pattern, bad, be16, be32, text,
};

/// Ticks a minute (our tempo, in BPM of 24 ticks) for a MED main tempo.
fn bpm(tempo: u16, bpm_mode: bool, lines_per_beat: u32, ticks: u32) -> (u8, Option<u8>) {
    if bpm_mode {
        let t = u32::from(tempo) * lines_per_beat * ticks / 24;
        (t.clamp(32, 255) as u8, None)
    } else if tempo <= 10 {
        (125, Some(tempo.max(1) as u8))
    } else {
        ((u32::from(tempo) * 125 / 33).clamp(32, 255) as u8, None)
    }
}

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let version = d[3] - b'0';
    let ptr = |at: usize| be32(d, at) as usize;
    let song = ptr(8);
    let blockarr = ptr(16);
    let smplarr = ptr(24);
    let exp = ptr(32);
    let s = d.get(song..song + 788).ok_or_else(|| bad("MED song"))?;
    let numblocks = usize::from(be16(s, 504));
    let songlen = usize::from(be16(s, 506));
    let deftempo = be16(s, 764);
    let transpose = s[766] as i8;
    let flags = s[767];
    let flags2 = s[768];
    let tempo2 = s[769].max(1);
    let numsamples = usize::from(s[787]);
    let bpm_mode = flags2 & 0x20 != 0;
    let lpb = u32::from(flags2 & 0x1F) + 1;
    let volhex = flags & 0x10 != 0;
    // Mixing mode (Soundstudio) spans ten octaves: its C-4 plays a sample at its own rate,
    // where the Amiga modes' C-2 does.
    let base = if flags2 & 0x80 != 0 { 23 } else { 47 };

    // The play order as block numbers.
    let mut seq: Vec<usize> = Vec::new();
    if version >= 2 {
        let table = be32(s, 508) as usize;
        let sections = be32(s, 512) as usize;
        for k in 0..songlen.min(256) {
            let n = usize::from(be16(d, sections + 2 * k));
            let ps = be32(d, table + 4 * n) as usize;
            let len = usize::from(be16(d, ps + 40));
            for i in 0..len.min(1024) {
                let b = be16(d, ps + 42 + 2 * i);
                if b < 0x8000 {
                    seq.push(usize::from(b));
                }
            }
        }
    } else {
        seq.extend(
            s[508..508 + songlen.min(256)]
                .iter()
                .map(|&b| usize::from(b)),
        );
    }

    // Blocks.
    let mut blocks: Vec<(usize, usize, &[u8])> = Vec::new();
    for b in 0..numblocks.min(254) {
        let at = ptr(blockarr + 4 * b);
        let (tracks, lines, body, size) = if version == 0 {
            let h = d.get(at..at + 2).ok_or_else(|| bad("MED block"))?;
            (usize::from(h[0]), usize::from(h[1]) + 1, at + 2, 3)
        } else {
            (
                usize::from(be16(d, at)),
                usize::from(be16(d, at + 2)) + 1,
                at + 8,
                4,
            )
        };
        let tracks = tracks.clamp(1, 64);
        let lines = lines.min(256);
        let end = (body + tracks * lines * size).min(d.len());
        blocks.push((tracks, lines, d.get(body..end).unwrap_or_default()));
    }
    let channels = if version >= 2 {
        usize::from(be16(s, 520)).clamp(1, 64)
    } else {
        blocks.iter().map(|b| b.0).max().unwrap_or(4)
    }
    .max(blocks.iter().map(|b| b.0).max().unwrap_or(1))
    .min(64);

    // Song name and instrument names from the expansion data.
    let mut title = String::new();
    let mut names = Vec::new();
    let mut finetune = [0i8; 64];
    if exp > 0 && exp + 52 <= d.len() {
        let name = ptr(exp + 44);
        let len = ptr(exp + 48);
        if name > 0 && len > 0 {
            title = text(d.get(name..(name + len).min(d.len())).unwrap_or_default());
        }
        let (smp, entries, size) = (
            ptr(exp + 4),
            usize::from(be16(d, exp + 8)),
            usize::from(be16(d, exp + 10)),
        );
        if smp > 0 && size >= 4 {
            for (k, slot) in finetune.iter_mut().enumerate().take(entries.min(63)) {
                if let Some(&f) = d.get(smp + k * size + 3) {
                    *slot = f as i8;
                }
            }
        }
        let (info, entries, size) = (
            ptr(exp + 20),
            usize::from(be16(d, exp + 24)),
            usize::from(be16(d, exp + 26)),
        );
        if info > 0 && size >= 40 {
            for k in 0..entries.min(63) {
                names.push(text(
                    d.get(info + k * size..info + k * size + 40)
                        .unwrap_or_default(),
                ));
            }
        }
    }

    let mut m = crate::module(Kind::Med, title, channels);
    let (tempo, speed) = bpm(deftempo, bpm_mode, lpb, u32::from(tempo2));
    m.tempo = tempo;
    m.speed = speed.unwrap_or(tempo2);
    if version >= 2 {
        let vols = be32(s, 516) as usize;
        let pans = be32(s, 524) as usize;
        for c in 0..channels {
            if vols > 0
                && let Some(&v) = d.get(vols + c)
            {
                m.channel_volumes[c] = v.clamp(0, 64);
            }
            if pans > 0
                && let Some(&p) = d.get(pans + c)
            {
                m.pans[c] = ((i32::from(p as i8).clamp(-16, 16) + 16) * 255 / 32) as u8;
            }
        }
    } else {
        for c in 0..channels.min(16) {
            m.channel_volumes[c] = s[770 + c].clamp(0, 64);
        }
    }

    // Instruments.
    let mut transposes = [0i8; 64];
    for k in 0..numsamples.min(63) {
        let e = &s[8 * k..8 * k + 8];
        transposes[k] = e[7] as i8;
        let at = if smplarr > 0 { ptr(smplarr + 4 * k) } else { 0 };
        let name = names.get(k).cloned().unwrap_or_default();
        let rate = 8363.0 * 2f64.powf(f64::from(finetune[k]) / 96.0);
        let Some(h) = (at > 0).then(|| d.get(at..at + 6)).flatten() else {
            m.samples
                .push(crate::sample(name, Vec::new(), None, rate, 0));
            continue;
        };
        let len = be32(h, 0) as usize;
        let kind = be16(h, 4) as i16;
        if kind < 0 {
            // Synthetic and hybrid sounds.
            m.samples
                .push(crate::sample(name, Vec::new(), None, rate, 0));
            continue;
        }
        let wide = kind & 0x10 != 0;
        let stereo = kind & 0x20 != 0;
        let raw = d
            .get(at + 6..(at + 6 + len).min(d.len()))
            .unwrap_or_default();
        let mut data: Vec<f32> = if wide {
            raw.chunks_exact(2)
                .map(|w| f32::from(i16::from_be_bytes([w[0], w[1]])) / 32768.0)
                .collect()
        } else {
            crate::pcm8(raw, false)
        };
        if stereo && let Some(right) = d.get(at + 6 + len..(at + 6 + 2 * len).min(d.len())) {
            let r: Vec<f32> = if wide {
                right
                    .chunks_exact(2)
                    .map(|w| f32::from(i16::from_be_bytes([w[0], w[1]])) / 32768.0)
                    .collect()
            } else {
                crate::pcm8(right, false)
            };
            for (l, r) in data.iter_mut().zip(r) {
                *l = (*l + r) / 2.0;
            }
        }
        // The multi-octave IFF instruments: the longest octave is last.
        let octaves = match kind & 15 {
            1 => 5,
            2 => 3,
            3 => 2,
            4 => 4,
            5 => 6,
            6 => 7,
            _ => 1,
        };
        if octaves > 1 {
            let unit = data.len() / ((1 << octaves) - 1);
            let first = data.len() - unit * (1 << (octaves - 1));
            data.drain(..first);
        }
        let div = if wide { 2 } else { 1 };
        let (rep, replen) = (
            usize::from(be16(e, 0)) * 2 / div,
            usize::from(be16(e, 2)) * 2 / div,
        );
        let lp = (replen > 2).then_some((rep, rep + replen));
        m.samples.push(crate::sample(name, data, lp, rate, e[6]));
    }

    // Patterns: one a block, the play order through them.
    for &(tracks, lines, body) in &blocks {
        let size = if version == 0 { 3 } else { 4 };
        let mut cells = vec![Cell::default(); lines * channels];
        for r in 0..lines {
            for c in 0..tracks.min(channels) {
                let Some(b) = body.get((r * tracks + c) * size..(r * tracks + c + 1) * size) else {
                    continue;
                };
                let (note, instr, cmd, x) = if version == 0 {
                    (
                        b[0] & 0x3F,
                        b[1] >> 4 | (b[0] & 0x80) >> 3 | (b[0] & 0x40) >> 1,
                        b[1] & 15,
                        b[2],
                    )
                } else {
                    (b[0] & 0x7F, b[1] & 0x3F, b[2], b[3])
                };
                let cell = &mut cells[r * channels + c];
                if note > 0 {
                    let shift = i32::from(transpose)
                        + i32::from(
                            transposes
                                .get(usize::from(instr).wrapping_sub(1))
                                .copied()
                                .unwrap_or(0),
                        );
                    cell.note = (i32::from(note) + base + shift).clamp(0, 119) as u8;
                } else {
                    cell.note = NOTE_NONE;
                }
                cell.instr = instr;
                (cell.fx, cell.param) = effect(cmd, x, volhex, bpm_mode, lpb, tempo2);
            }
        }
        m.patterns.push(Pattern { rows: lines, cells });
    }
    m.orders = seq
        .into_iter()
        .filter(|&b| b < m.patterns.len())
        .map(|b| b as u8)
        .take(1 << 12)
        .collect();
    if m.orders.is_empty() {
        return Err(bad("MED play sequence"));
    }
    Ok(m)
}

/// A MED command as a Scream Tracker letter.
fn effect(cmd: u8, x: u8, volhex: bool, bpm_mode: bool, lpb: u32, ticks: u8) -> (u8, u8) {
    match cmd {
        0 if x != 0 => (b'J', x),
        1 => (b'F', x.min(0xDF)),
        2 => (b'E', x.min(0xDF)),
        3 => (b'G', x),
        4 | 0x14 => (b'H', x),
        5 => (b'L', x),
        6 => (b'K', x),
        7 => (b'R', x),
        9 if (1..=32).contains(&x) => (b'A', x),
        0xA | 0xD => (FX_MOD_VOLSLIDE, x),
        0xB => (b'B', x),
        0xC => {
            let v = if volhex { x } else { (x >> 4) * 10 + (x & 15) };
            (FX_SET_VOLUME, v.min(64))
        }
        0xF => match x {
            0 => (b'C', 0),
            1..=0xF0 => match bpm(u16::from(x), bpm_mode, lpb, u32::from(ticks)) {
                (_, Some(speed)) => (b'A', speed),
                (t, None) => (b'T', t),
            },
            0xF2 => (b'S', 0xD3),
            0xFE | 0xFF => (b'S', 0xC0),
            _ => (0, 0),
        },
        0x11 => (b'F', 0xF0 | (x & 15)),
        0x12 => (b'E', 0xF0 | (x & 15)),
        0x18 => (b'S', 0xC0 | (x & 15)),
        0x19 => (b'O', x),
        0x1A => (FX_FINE_VOL_UP, x),
        0x1B => (FX_FINE_VOL_DOWN, x),
        0x1D => (b'C', x),
        0x1E => (b'S', 0xE0 | (x & 15)),
        0x1F => (b'S', 0xD0 | (x >> 4)),
        _ => (0, 0),
    }
}
