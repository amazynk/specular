//! FastTracker 2 (XM): a header ("Extended Module: "), packed patterns, then instruments
//! with their envelopes, sample headers and delta-coded 8- or 16-bit samples.

use crate::load_mod::protracker_fx;
use crate::{
    Cell, Envelope, Error, Instrument, Kind, LoopKind, MAX_SAMPLE, Module, NOTE_NONE, NOTE_OFF,
    Pattern, Sample, VolCmd, bad, le16, le32, text,
};

fn envelope(h: &[u8], at: usize, n: u8, sus: u8, ls: u8, le: u8, flags: u8) -> Envelope {
    let n = usize::from(n).min(12);
    let points = (0..n)
        .map(|k| (le16(h, at + 4 * k), le16(h, at + 4 * k + 2) as i16))
        .collect::<Vec<_>>();
    Envelope {
        enabled: flags & 1 != 0 && n > 0,
        sustain: (flags & 2 != 0 && usize::from(sus) < n)
            .then_some((usize::from(sus), usize::from(sus))),
        looped: (flags & 4 != 0 && usize::from(le) < n && ls <= le)
            .then_some((usize::from(ls), usize::from(le))),
        points,
    }
}

fn vol_column(v: u8) -> VolCmd {
    let (x, y) = (v >> 4, v & 15);
    match x {
        1..=4 => VolCmd::Set(v - 16),
        5 if v == 0x50 => VolCmd::Set(64),
        6 => VolCmd::SlideDown(y),
        7 => VolCmd::SlideUp(y),
        8 => VolCmd::FineDown(y),
        9 => VolCmd::FineUp(y),
        10 => VolCmd::VibratoSpeed(y),
        11 => VolCmd::Vibrato(y),
        12 => VolCmd::Pan(y * 17),
        13 => VolCmd::PanLeft(y),
        14 => VolCmd::PanRight(y),
        15 => VolCmd::TonePorta(y << 4),
        _ => VolCmd::None,
    }
}

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let title = text(&d[17..37]);
    let version = le16(d, 58);
    if version < 0x0104 {
        return Err(Error::Unsupported(format!(
            "XM version {}.{:02}",
            version >> 8,
            version & 0xFF
        )));
    }
    let hsize = le32(d, 60) as usize;
    let song_len = usize::from(le16(d, 64)).min(256);
    let restart = usize::from(le16(d, 66));
    let channels = usize::from(le16(d, 68));
    let patnum = usize::from(le16(d, 70));
    let insnum = usize::from(le16(d, 72));
    let linear = le16(d, 74) & 1 != 0;
    let speed = le16(d, 76);
    let tempo = le16(d, 78);
    if channels == 0 || channels > 64 {
        return Err(Error::Unsupported(format!("{channels} channels")));
    }
    let orders = d
        .get(80..80 + song_len)
        .ok_or_else(|| bad("XM orders"))?
        .to_vec();
    let mut at = 60 + hsize;
    let mut patterns = Vec::with_capacity(patnum);
    for _ in 0..patnum {
        let plen = le32(d, at) as usize;
        let rows = usize::from(le16(d, at + 5)).clamp(1, 256);
        let packed = usize::from(le16(d, at + 7));
        let mut cells = vec![Cell::default(); rows * channels];
        let mut i = at + plen;
        let end = (i + packed).min(d.len());
        let mut k = 0;
        let byte = |i: &mut usize| {
            let v = d.get(*i).copied().unwrap_or(0);
            *i += 1;
            v
        };
        while i < end && k < rows * channels {
            let first = byte(&mut i);
            let (mut note, mut ins, mut vol, mut fx, mut prm) = (0, 0, 0, 0, 0);
            if first & 0x80 != 0 {
                if first & 1 != 0 {
                    note = byte(&mut i);
                }
                if first & 2 != 0 {
                    ins = byte(&mut i);
                }
                if first & 4 != 0 {
                    vol = byte(&mut i);
                }
                if first & 8 != 0 {
                    fx = byte(&mut i);
                }
                if first & 16 != 0 {
                    prm = byte(&mut i);
                }
            } else {
                note = first;
                ins = byte(&mut i);
                vol = byte(&mut i);
                fx = byte(&mut i);
                prm = byte(&mut i);
            }
            let (f, p) = protracker_fx(fx, prm, true);
            cells[k] = Cell {
                note: match note {
                    0 => NOTE_NONE,
                    97 => NOTE_OFF,
                    n if n < 97 => n - 1 + 12,
                    _ => NOTE_NONE,
                },
                instr: ins,
                vol: vol_column(vol),
                fx: f,
                param: p,
            };
            k += 1;
        }
        patterns.push(Pattern { rows, cells });
        at += plen + packed;
    }
    let mut instruments = Vec::with_capacity(insnum);
    let mut samples: Vec<Sample> = Vec::new();
    for _ in 0..insnum {
        let isize = le32(d, at) as usize;
        let h = d.get(at..).unwrap_or_default();
        let name = text(h.get(4..26).unwrap_or_default());
        let nsamples = usize::from(le16(h, 27));
        let mut ins = Instrument {
            name,
            keymap: Vec::new(),
            global_volume: 128,
            ..Instrument::default()
        };
        if nsamples == 0 {
            at += isize.max(29);
            ins.keymap = vec![(0, 0); 120];
            instruments.push(ins);
            continue;
        }
        let shsize = le32(h, 29) as usize;
        let base = samples.len();
        let map = h.get(33..129).unwrap_or(&[0; 96]);
        ins.keymap = (0..120)
            .map(|n| {
                let k = if (12..108).contains(&n) {
                    map.get(n - 12).copied().unwrap_or(0)
                } else {
                    0
                };
                let s = if usize::from(k) < nsamples {
                    (base + usize::from(k) + 1).min(255) as u8
                } else {
                    0
                };
                (s, n as u8)
            })
            .collect();
        let b = |o: usize| h.get(o).copied().unwrap_or(0);
        ins.volume = envelope(h, 129, b(225), b(227), b(228), b(229), b(233));
        ins.panning = envelope(h, 177, b(226), b(230), b(231), b(232), b(234));
        let vib = (b(235), b(236), b(237), b(238));
        ins.fadeout = u32::from(le16(h, 239)) * 2;
        at += isize;
        // Sample headers, then their data.
        let mut heads = Vec::with_capacity(nsamples);
        for _ in 0..nsamples {
            heads.push(d.get(at..at + 40).unwrap_or(&[0; 40]).to_vec());
            at += shsize.max(40);
        }
        for sh in heads {
            let bytes = le32(&sh, 0) as usize;
            let mut ls = le32(&sh, 4) as usize;
            let mut ll = le32(&sh, 8) as usize;
            let volume = sh[12].min(64);
            let fine = sh[13] as i8;
            let kind = sh[14];
            let pan = sh[15];
            let rel = sh[16] as i8;
            let wide = kind & 16 != 0;
            let avail = bytes.min(d.len().saturating_sub(at));
            let raw = &d[at..at + avail];
            at += bytes;
            let mut data = Vec::new();
            if wide {
                let mut acc = 0i16;
                for c in raw.chunks_exact(2).take(MAX_SAMPLE) {
                    acc = acc.wrapping_add(i16::from_le_bytes([c[0], c[1]]));
                    data.push(f32::from(acc) / 32768.0);
                }
                ls /= 2;
                ll /= 2;
            } else {
                let mut acc = 0i8;
                for &c in raw.iter().take(MAX_SAMPLE) {
                    acc = acc.wrapping_add(c as i8);
                    data.push(f32::from(acc) / 128.0);
                }
            }
            let loop_kind = match kind & 3 {
                1 if ll > 0 => LoopKind::Forward,
                2 if ll > 0 => LoopKind::PingPong,
                _ => LoopKind::None,
            };
            samples.push(Sample {
                name: text(&sh[18..40]),
                loop_kind,
                loop_start: ls.min(data.len()),
                loop_end: (ls + ll).min(data.len()),
                data,
                c_speed: 8363.0 * 2f64.powf((f64::from(rel) + f64::from(fine) / 128.0) / 12.0),
                volume,
                global_volume: 64,
                pan: Some(pan),
                vibrato: vib,
                ..Sample::default()
            });
        }
        instruments.push(ins);
    }
    Ok(Module {
        kind: Kind::Xm,
        title,
        channels,
        orders,
        patterns,
        samples,
        instruments,
        speed: if speed == 0 { 6 } else { speed.min(255) as u8 },
        tempo: tempo.clamp(32, 255) as u8,
        global_volume: 128,
        mix_volume: 128,
        pans: vec![128; channels],
        channel_volumes: vec![64; channels],
        linear,
        use_instruments: true,
        restart: if restart < song_len { restart } else { 0 },
        fx_memory: true,
    })
}
