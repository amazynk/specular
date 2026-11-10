//! Scream Tracker 3 (S3M): a 96-byte header ("SCRM" at 44), the order list, paragraph
//! pointers to the instruments and packed patterns; samples unsigned (or signed, by the
//! header's format flag), 8- or 16-bit.

use crate::{
    Cell, Error, Kind, LoopKind, MAX_SAMPLE, Module, NOTE_CUT, NOTE_NONE, Pattern, Sample, VolCmd,
    bad, le16, le32, text,
};

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let title = text(&d[..28]);
    let ordnum = usize::from(le16(d, 32));
    let insnum = usize::from(le16(d, 34));
    let patnum = usize::from(le16(d, 36));
    let signed = le16(d, 42) == 1;
    let global = d[48];
    let speed = d[49];
    let tempo = d[50];
    let master = d[51];
    let stereo = master & 0x80 != 0;
    let default_pan = d[53] == 252;
    let settings = &d[64..96];
    let mut at = 96;
    let orders = d
        .get(at..at + ordnum)
        .ok_or_else(|| bad("S3M orders"))?
        .to_vec();
    at += ordnum;
    let para = |k: usize| usize::from(le16(d, at + 2 * k)) * 16;
    let ins_ptr: Vec<usize> = (0..insnum).map(para).collect();
    let pat_ptr: Vec<usize> = (insnum..insnum + patnum).map(para).collect();
    at += 2 * (insnum + patnum);
    // Channels in use: the enabled ones, numbered in order.
    let map: Vec<Option<usize>> = {
        let mut n = 0;
        settings
            .iter()
            .map(|&s| {
                (s < 16).then(|| {
                    n += 1;
                    n - 1
                })
            })
            .collect()
    };
    let channels = map.iter().flatten().count().max(1);
    let mut pans = vec![128u8; channels];
    for (k, &s) in settings.iter().enumerate() {
        if let Some(c) = map[k] {
            pans[c] = if !stereo {
                128
            } else if s < 8 {
                51
            } else {
                204
            };
            if default_pan
                && let Some(&p) = d.get(at + k)
                && p & 0x20 != 0
            {
                pans[c] = (p & 15) * 17;
            }
        }
    }
    let mut samples = Vec::with_capacity(insnum);
    for &p in &ins_ptr {
        let Some(h) = d.get(p..p + 80) else {
            samples.push(Sample::default());
            continue;
        };
        if h[0] != 1 {
            // Adlib instruments and empty slots.
            samples.push(Sample {
                name: text(&h[48..76]),
                ..Sample::default()
            });
            continue;
        }
        let ptr = ((usize::from(h[13]) << 16) | usize::from(le16(h, 14))) * 16;
        let length = (le32(h, 16) as usize).min(MAX_SAMPLE);
        let ls = le32(h, 20) as usize;
        let le = le32(h, 24) as usize;
        let flags = h[31];
        let wide = flags & 4 != 0;
        let two = flags & 2 != 0;
        let bytes = if wide { 2 } else { 1 };
        let mut data = Vec::with_capacity(length);
        let ch_len = length * bytes;
        for i in 0..length {
            let v = |o: usize| -> f32 {
                if wide {
                    let raw = le16(d, o);
                    let s = if signed {
                        raw as i16
                    } else {
                        (raw ^ 0x8000) as i16
                    };
                    f32::from(s) / 32768.0
                } else {
                    let raw = d.get(o).copied().unwrap_or(if signed { 0 } else { 128 });
                    let s = if signed {
                        raw as i8
                    } else {
                        (raw ^ 0x80) as i8
                    };
                    f32::from(s) / 128.0
                }
            };
            let o = ptr + i * bytes;
            if o + bytes > d.len() {
                break;
            }
            data.push(if two {
                (v(o) + v(o + ch_len)) / 2.0
            } else {
                v(o)
            });
        }
        let looped = flags & 1 != 0 && le > ls && ls < data.len();
        samples.push(Sample {
            name: text(&h[48..76]),
            loop_kind: if looped {
                LoopKind::Forward
            } else {
                LoopKind::None
            },
            loop_start: ls,
            loop_end: le.min(data.len()),
            data,
            c_speed: f64::from(le32(h, 32).max(1)),
            volume: h[28].min(64),
            global_volume: 64,
            ..Sample::default()
        });
    }
    let mut patterns = Vec::with_capacity(patnum);
    for &p in &pat_ptr {
        let mut cells = vec![Cell::default(); 64 * channels];
        if p > 0 && p + 2 <= d.len() {
            let end = (p + usize::from(le16(d, p))).min(d.len());
            let mut i = p + 2;
            let mut row = 0;
            while i < end && row < 64 {
                let what = d[i];
                i += 1;
                if what == 0 {
                    row += 1;
                    continue;
                }
                let ch = map.get(usize::from(what & 31)).copied().flatten();
                let mut cell = Cell::default();
                if what & 32 != 0 {
                    let (n, ins) = (
                        d.get(i).copied().unwrap_or(255),
                        d.get(i + 1).copied().unwrap_or(0),
                    );
                    i += 2;
                    cell.note = match n {
                        255 => NOTE_NONE,
                        254 => NOTE_CUT,
                        n => ((n >> 4) * 12 + (n & 15) + 12).min(119),
                    };
                    cell.instr = ins;
                }
                if what & 64 != 0 {
                    let v = d.get(i).copied().unwrap_or(255);
                    i += 1;
                    if v <= 64 {
                        cell.vol = VolCmd::Set(v);
                    }
                }
                if what & 128 != 0 {
                    let (f, prm) = (
                        d.get(i).copied().unwrap_or(0),
                        d.get(i + 1).copied().unwrap_or(0),
                    );
                    i += 2;
                    if (1..=26).contains(&f) {
                        cell.fx = b'A' + f - 1;
                        cell.param = prm;
                    }
                }
                if let Some(c) = ch {
                    cells[row * channels + c] = cell;
                }
            }
        }
        patterns.push(Pattern { rows: 64, cells });
    }
    Ok(Module {
        kind: Kind::S3m,
        title,
        channels,
        orders,
        patterns,
        samples,
        instruments: Vec::new(),
        speed: if speed == 0 || speed == 255 { 6 } else { speed },
        tempo: if tempo < 32 { 125 } else { tempo },
        global_volume: global.min(64) * 2,
        mix_volume: (master & 0x7F).max(16),
        pans,
        channel_volumes: vec![64; channels],
        linear: false,
        use_instruments: false,
        restart: 0,
        fx_memory: true,
    })
}
