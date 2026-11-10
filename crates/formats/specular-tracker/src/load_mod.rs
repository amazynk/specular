//! ProTracker-style MOD: a 20-byte title, 31 sample headers, the order list, a 4-byte tag
//! at byte 1080 (the channel count), 64-row patterns of 4-byte cells, signed 8-bit samples.

use crate::{
    Cell, Error, FX_MOD_VOLSLIDE, FX_SET_VOLUME, Kind, LoopKind, MAX_SAMPLE, Module, NOTE_NONE,
    Pattern, Sample, bad, be16, mod_channels, text,
};

/// ProTracker's effect `fx` with `param` as a Scream Tracker letter (XM adds G–X).
pub(crate) fn protracker_fx(fx: u8, param: u8, xm: bool) -> (u8, u8) {
    let (x, y) = (param >> 4, param & 15);
    match fx {
        0 if param != 0 => (b'J', param),
        0 => (0, 0),
        1 => (b'F', param.min(0xDF)),
        2 => (b'E', param.min(0xDF)),
        3 => (b'G', param),
        4 => (b'H', param),
        5 => (b'L', param),
        6 => (b'K', param),
        7 => (b'R', param),
        8 => (b'X', param),
        9 => (b'O', param),
        10 => (FX_MOD_VOLSLIDE, param),
        11 => (b'B', param),
        12 => (FX_SET_VOLUME, param.min(64)),
        // The row in decimal digits.
        13 => (b'C', (x * 10 + y).min(63)),
        14 => match x {
            1 => (b'F', 0xF0 | y),
            2 => (b'E', 0xF0 | y),
            4 => (b'S', 0x30 | y),
            5 => (b'S', 0x20 | y),
            6 => (b'S', 0xB0 | y),
            7 => (b'S', 0x40 | y),
            8 => (b'S', 0x80 | y),
            9 => (b'Q', y),
            10 => (crate::FX_FINE_VOL_UP, y),
            11 => (crate::FX_FINE_VOL_DOWN, y),
            12 => (b'S', 0xC0 | y),
            13 => (b'S', 0xD0 | y),
            14 => (b'S', 0xE0 | y),
            _ => (0, 0),
        },
        15 if param == 0 => (0, 0),
        15 if param < 32 => (b'A', param),
        15 => (b'T', param),
        16 if xm => (b'V', param.min(64) * 2),
        17 if xm => (b'W', param),
        20 if xm => (crate::FX_KEY_OFF, param),
        21 if xm => (crate::FX_ENVELOPE_POS, param),
        25 if xm => (b'P', param),
        27 if xm => (b'Q', param),
        33 if xm => match x {
            1 => (b'F', 0xE0 | y),
            2 => (b'E', 0xE0 | y),
            _ => (0, 0),
        },
        _ => (0, 0),
    }
}

/// The note (60: the sample's own rate) of an Amiga period, 0 none.
fn note_of_period(period: u16) -> u8 {
    if period == 0 {
        return crate::NOTE_NONE;
    }
    let n = 60.0 + 12.0 * (428.0 / f64::from(period)).log2();
    n.round().clamp(0.0, 119.0) as u8
}

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let channels = mod_channels(&d[1080..1084]).ok_or_else(|| bad("MOD tag"))?;
    let title = text(&d[..20]);
    let len = usize::from(d[950]).clamp(1, 128);
    let restart = usize::from(d[951]);
    let all = &d[952..1080];
    let orders = all[..len].to_vec();
    let patterns_n = usize::from(all.iter().copied().filter(|&o| o < 128).max().unwrap_or(0)) + 1;
    let row_bytes = 4 * channels;
    let mut at = 1084;
    let mut patterns = Vec::with_capacity(patterns_n);
    for _ in 0..patterns_n {
        let mut cells = Vec::with_capacity(64 * channels);
        for r in 0..64 {
            for c in 0..channels {
                let o = at + r * row_bytes + 4 * c;
                let Some(b) = d.get(o..o + 4) else {
                    cells.push(Cell::default());
                    continue;
                };
                let instr = (b[0] & 0xF0) | (b[2] >> 4);
                let period = u16::from(b[0] & 0x0F) << 8 | u16::from(b[1]);
                let (fx, param) = protracker_fx(b[2] & 0x0F, b[3], false);
                cells.push(Cell {
                    note: if period == 0 {
                        NOTE_NONE
                    } else {
                        note_of_period(period)
                    },
                    instr,
                    vol: crate::VolCmd::None,
                    fx,
                    param,
                });
            }
        }
        patterns.push(Pattern { rows: 64, cells });
        at += 64 * row_bytes;
    }
    let mut samples = Vec::with_capacity(31);
    for k in 0..31 {
        let h = &d[20 + 30 * k..50 + 30 * k];
        let length = usize::from(be16(h, 22)) * 2;
        let fine = (h[24] & 15) as i8;
        let fine = if fine > 7 { fine - 16 } else { fine };
        let volume = h[25].min(64);
        let ls = usize::from(be16(h, 26)) * 2;
        let ll = usize::from(be16(h, 28)) * 2;
        let avail = d.len().saturating_sub(at).min(length).min(MAX_SAMPLE);
        let data: Vec<f32> = d[at..at + avail]
            .iter()
            .map(|&b| f32::from(b as i8) / 128.0)
            .collect();
        at += length;
        let looped = ll > 2 && ls < data.len();
        samples.push(Sample {
            name: text(&h[..22]),
            loop_kind: if looped {
                LoopKind::Forward
            } else {
                LoopKind::None
            },
            loop_start: ls,
            loop_end: (ls + ll).min(data.len()),
            data,
            c_speed: 8363.0 * 2f64.powf(f64::from(fine) / 96.0),
            volume,
            global_volume: 64,
            ..Sample::default()
        });
    }
    let pans = (0..channels)
        .map(|c| if c % 4 == 0 || c % 4 == 3 { 0 } else { 255 })
        .collect();
    Ok(Module {
        kind: Kind::Mod,
        title,
        channels,
        orders,
        patterns,
        samples,
        instruments: Vec::new(),
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 128,
        pans,
        channel_volumes: vec![64; channels],
        linear: false,
        use_instruments: false,
        restart: if restart < len { restart } else { 0 },
        fx_memory: false,
    })
}
