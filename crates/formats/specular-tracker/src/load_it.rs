//! Impulse Tracker (IT): the "IMPM" header, order list, offsets of the instruments
//! ("IMPI", format 2.0 and later), samples ("IMPS", also IT 2.14/2.15-compressed) and
//! packed patterns.

use crate::{
    Cell, Envelope, Error, Instrument, Kind, LoopKind, MAX_SAMPLE, Module, NOTE_CUT, NOTE_FADE,
    NOTE_OFF, Pattern, Sample, VolCmd, bad, le16, le32, text,
};

/// A packed pattern cell before channels are known: (row, channel, cell).
type RawCell = (usize, usize, Cell);

fn vol_column(v: u8) -> VolCmd {
    match v {
        0..=64 => VolCmd::Set(v),
        65..=74 => VolCmd::FineUp(v - 65),
        75..=84 => VolCmd::FineDown(v - 75),
        85..=94 => VolCmd::SlideUp(v - 85),
        95..=104 => VolCmd::SlideDown(v - 95),
        105..=114 => VolCmd::PortaDown((v - 105) * 4),
        115..=124 => VolCmd::PortaUp((v - 115) * 4),
        128..=192 => VolCmd::Pan(((u16::from(v - 128) * 255 + 32) / 64) as u8),
        193..=202 => {
            VolCmd::TonePorta([0, 1, 4, 8, 16, 32, 64, 96, 128, 255][usize::from(v - 193)])
        }
        203..=212 => VolCmd::Vibrato(v - 203),
        _ => VolCmd::None,
    }
}

fn envelope(h: &[u8], at: usize, pan: bool) -> Envelope {
    let flags = h.get(at).copied().unwrap_or(0);
    let n = usize::from(h.get(at + 1).copied().unwrap_or(0)).min(25);
    let b = |o: usize| usize::from(h.get(at + o).copied().unwrap_or(0));
    let points = (0..n)
        .map(|k| {
            let y = h.get(at + 6 + 3 * k).copied().unwrap_or(0);
            let y = if pan {
                i16::from(y as i8)
            } else {
                i16::from(y)
            };
            (le16(h, at + 7 + 3 * k), y)
        })
        .collect();
    Envelope {
        enabled: flags & 1 != 0 && n > 0,
        looped: (flags & 2 != 0 && b(2) <= b(3) && b(3) < n).then_some((b(2), b(3))),
        sustain: (flags & 4 != 0 && b(4) <= b(5) && b(5) < n).then_some((b(4), b(5))),
        points,
    }
}

/// LSB-first bits of a compressed block.
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn read(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            let b = *self.d.get(self.pos >> 3)?;
            v |= u32::from((b >> (self.pos & 7)) & 1) << i;
            self.pos += 1;
        }
        Some(v)
    }
}

/// Impulse Tracker 2.14 sample compression (2.15: twice integrated): blocks of 32 768
/// bytes of output, each a 16-bit length and a bit stream whose code width changes in-band.
fn decompress(d: &[u8], frames: usize, wide: bool, it215: bool) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames);
    let mut at = 0;
    let block_frames = if wide { 0x4000 } else { 0x8000 };
    let (top, fetch): (u32, u32) = if wide { (17, 4) } else { (9, 3) };
    while out.len() < frames {
        let len = usize::from(le16(d, at));
        at += 2;
        let Some(block) = d.get(at..at + len) else {
            break;
        };
        at += len;
        let mut b = Bits { d: block, pos: 0 };
        let mut width = top;
        let (mut d1, mut d2) = (0i32, 0i32);
        let n = block_frames.min(frames - out.len());
        let mut got = 0;
        while got < n {
            let Some(mut v) = b.read(width) else {
                break;
            };
            if width < 7 {
                if v == 1 << (width - 1) {
                    let Some(w) = b.read(fetch) else { break };
                    let w = w + 1;
                    width = if w < width { w } else { w + 1 };
                    continue;
                }
            } else if width < top {
                let border = if wide {
                    (0xFFFF >> (17 - width)) - 8
                } else {
                    (0xFF >> (9 - width)) - 4
                };
                let span = if wide { 16 } else { 8 };
                if v > border && v <= border + span {
                    v -= border;
                    width = if v < width { v } else { v + 1 };
                    continue;
                }
            } else if v & (1 << (top - 1)) != 0 {
                width = (v + 1) & 0xFF;
                if width == 0 || width > top {
                    break;
                }
                continue;
            }
            let bits = if wide { 16 } else { 8 };
            let s = if width < bits {
                let sh = 32 - width;
                ((v << sh) as i32) >> sh
            } else {
                let sh = 32 - bits;
                ((v << sh) as i32) >> sh
            };
            d1 = d1.wrapping_add(s);
            d2 = d2.wrapping_add(d1);
            let o = if it215 { d2 } else { d1 };
            out.push(if wide {
                f32::from(o as i16) / 32768.0
            } else {
                f32::from(o as i8) / 128.0
            });
            got += 1;
        }
        if got < n {
            out.resize(out.len() + (n - got), 0.0);
        }
    }
    out.truncate(frames);
    out
}

fn sample(d: &[u8], p: usize) -> Sample {
    let Some(h) = d.get(p..p + 80).filter(|h| h.starts_with(b"IMPS")) else {
        return Sample::default();
    };
    let flags = h[18];
    let cvt = h[46];
    let length = (le32(h, 48) as usize).min(MAX_SAMPLE);
    let ptr = le32(h, 72) as usize;
    let wide = flags & 2 != 0;
    let stereo = flags & 4 != 0;
    let compressed = flags & 8 != 0;
    let signed = cvt & 1 != 0;
    let mut data = Vec::new();
    if flags & 1 != 0 && length > 0 {
        let body = d.get(ptr..).unwrap_or_default();
        if compressed {
            data = decompress(body, length, wide, cvt & 4 != 0);
            if stereo {
                // The right channel's blocks follow; averaged.
                let skip = blocks_len(body, length, wide);
                let r = decompress(
                    body.get(skip..).unwrap_or_default(),
                    length,
                    wide,
                    cvt & 4 != 0,
                );
                for (l, r) in data.iter_mut().zip(r) {
                    *l = (*l + r) / 2.0;
                }
            }
        } else {
            let bytes = if wide { 2 } else { 1 };
            let at = |i: usize| -> f32 {
                if wide {
                    let raw = le16(body, i * 2);
                    let s = if signed {
                        raw as i16
                    } else {
                        (raw ^ 0x8000) as i16
                    };
                    f32::from(s) / 32768.0
                } else {
                    let raw = body.get(i).copied().unwrap_or(0);
                    let s = if signed {
                        raw as i8
                    } else {
                        (raw ^ 0x80) as i8
                    };
                    f32::from(s) / 128.0
                }
            };
            let have = (body.len() / bytes).min(length);
            data = (0..have)
                .map(|i| {
                    if stereo && i + length < body.len() / bytes {
                        (at(i) + at(i + length)) / 2.0
                    } else {
                        at(i)
                    }
                })
                .collect();
        }
    }
    let loop_kind = |on: u8, ping: u8| match (flags & on != 0, flags & ping != 0) {
        (true, true) => LoopKind::PingPong,
        (true, false) => LoopKind::Forward,
        _ => LoopKind::None,
    };
    let n = data.len();
    let (ls, le) = ((le32(h, 52) as usize).min(n), (le32(h, 56) as usize).min(n));
    let (ss, se) = ((le32(h, 64) as usize).min(n), (le32(h, 68) as usize).min(n));
    let lk = if le > ls {
        loop_kind(16, 64)
    } else {
        LoopKind::None
    };
    let sk = if se > ss {
        loop_kind(32, 128)
    } else {
        LoopKind::None
    };
    Sample {
        name: text(&h[20..46]),
        data,
        loop_kind: lk,
        loop_start: ls,
        loop_end: le,
        sustain: (sk != LoopKind::None).then_some((ss, se, sk)),
        c_speed: f64::from(le32(h, 60).max(1)),
        volume: h[19].min(64),
        global_volume: h[17].min(64),
        pan: (h[47] & 0x80 != 0).then(|| ((u16::from(h[47] & 0x7F).min(64) * 255 + 32) / 64) as u8),
        vibrato: (h[79], h[78], h[77], h[76]),
    }
}

/// Bytes of the compressed blocks holding `frames` frames.
fn blocks_len(d: &[u8], frames: usize, wide: bool) -> usize {
    let per = if wide { 0x4000 } else { 0x8000 };
    let mut at = 0;
    for _ in 0..frames.div_ceil(per) {
        at += 2 + usize::from(le16(d, at));
    }
    at
}

pub(crate) fn load(d: &[u8]) -> Result<Module, Error> {
    let title = text(&d[4..30]);
    let ordnum = usize::from(le16(d, 0x20));
    let insnum = usize::from(le16(d, 0x22));
    let smpnum = usize::from(le16(d, 0x24));
    let patnum = usize::from(le16(d, 0x26));
    let cmwt = le16(d, 0x2A);
    let flags = le16(d, 0x2C);
    let use_instruments = flags & 4 != 0;
    if use_instruments && cmwt < 0x200 {
        return Err(Error::Unsupported(
            "Impulse Tracker 1.x instruments (format before 2.0)".into(),
        ));
    }
    let global = d[0x30];
    let mix = d[0x31];
    let speed = d[0x32];
    let tempo = d[0x33];
    let stereo = flags & 1 != 0;
    let mut at = 0xC0;
    let orders = d
        .get(at..at + ordnum)
        .ok_or_else(|| bad("IT orders"))?
        .to_vec();
    at += ordnum;
    let ins_off: Vec<usize> = (0..insnum).map(|k| le32(d, at + 4 * k) as usize).collect();
    at += 4 * insnum;
    let smp_off: Vec<usize> = (0..smpnum).map(|k| le32(d, at + 4 * k) as usize).collect();
    at += 4 * smpnum;
    let pat_off: Vec<usize> = (0..patnum).map(|k| le32(d, at + 4 * k) as usize).collect();
    let samples: Vec<Sample> = smp_off.iter().map(|&p| sample(d, p)).collect();
    let mut instruments = Vec::with_capacity(insnum);
    for &p in &ins_off {
        let h = d.get(p..p + 554).unwrap_or_default();
        if h.len() < 554 || !h.starts_with(b"IMPI") {
            instruments.push(Instrument {
                keymap: vec![(0, 0); 120],
                ..Instrument::default()
            });
            continue;
        }
        let keymap = (0..120)
            .map(|n| (h[0x41 + 2 * n], h[0x40 + 2 * n].min(119)))
            .collect();
        instruments.push(Instrument {
            name: text(&h[32..58]),
            keymap,
            volume: envelope(h, 0x130, false),
            panning: envelope(h, 0x182, true),
            fadeout: u32::from(le16(h, 0x14)) * 64,
            global_volume: h[0x18].min(128),
            pan: (h[0x19] & 0x80 == 0)
                .then(|| ((u16::from(h[0x19]).min(64) * 255 + 32) / 64) as u8),
        });
    }
    // Channels: as many as the patterns use.
    let mut channels = 1;
    let mut patterns = Vec::with_capacity(patnum);
    let mut raw: Vec<(usize, Vec<RawCell>)> = Vec::with_capacity(patnum);
    for &p in &pat_off {
        if p == 0 || p + 8 > d.len() {
            raw.push((64, Vec::new()));
            continue;
        }
        let len = usize::from(le16(d, p));
        let rows = usize::from(le16(d, p + 2)).clamp(1, 200);
        let mut i = p + 8;
        let end = (i + len).min(d.len());
        let mut masks = [0u8; 64];
        let mut last = [Cell::default(); 64];
        let mut row = 0;
        let mut cells = Vec::new();
        let byte = |i: &mut usize| {
            let v = d.get(*i).copied().unwrap_or(0);
            *i += 1;
            v
        };
        while i < end && row < rows {
            let cv = byte(&mut i);
            if cv == 0 {
                row += 1;
                continue;
            }
            let ch = usize::from((cv - 1) & 63);
            if cv & 0x80 != 0 {
                masks[ch] = byte(&mut i);
            }
            let m = masks[ch];
            let mut c = Cell::default();
            if m & 1 != 0 {
                let n = byte(&mut i);
                last[ch].note = match n {
                    0..=119 => n,
                    255 => NOTE_OFF,
                    254 => NOTE_CUT,
                    _ => NOTE_FADE,
                };
            }
            if m & 2 != 0 {
                last[ch].instr = byte(&mut i);
            }
            if m & 4 != 0 {
                last[ch].vol = vol_column(byte(&mut i));
            }
            if m & 8 != 0 {
                let (f, prm) = (byte(&mut i), byte(&mut i));
                (last[ch].fx, last[ch].param) = if (1..=26).contains(&f) {
                    (b'A' + f - 1, prm)
                } else {
                    (0, 0)
                };
            }
            if m & 0x11 != 0 {
                c.note = last[ch].note;
            }
            if m & 0x22 != 0 {
                c.instr = last[ch].instr;
            }
            if m & 0x44 != 0 {
                c.vol = last[ch].vol;
            }
            if m & 0x88 != 0 {
                (c.fx, c.param) = (last[ch].fx, last[ch].param);
            }
            channels = channels.max(ch + 1);
            cells.push((row, ch, c));
        }
        raw.push((rows, cells));
    }
    for (rows, list) in raw {
        let mut cells = vec![Cell::default(); rows * channels];
        for (r, c, cell) in list {
            cells[r * channels + c] = cell;
        }
        patterns.push(Pattern { rows, cells });
    }
    let pans = (0..channels)
        .map(|c| {
            let p = d[0x40 + c];
            if !stereo || p & 0x7F == 100 {
                128
            } else {
                ((u16::from(p & 0x7F).min(64) * 255 + 32) / 64) as u8
            }
        })
        .collect();
    let channel_volumes = (0..channels).map(|c| d[0x80 + c].min(64)).collect();
    Ok(Module {
        kind: Kind::It,
        title,
        channels,
        orders,
        patterns,
        samples,
        instruments,
        speed: if speed == 0 { 6 } else { speed },
        tempo: tempo.max(32),
        global_volume: global.min(128),
        mix_volume: mix.min(128),
        pans,
        channel_volumes,
        linear: flags & 8 != 0,
        use_instruments,
        restart: 0,
        fx_memory: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_column() {
        assert_eq!(vol_column(64), VolCmd::Set(64));
        assert_eq!(vol_column(128 + 32), VolCmd::Pan(128));
        assert_eq!(vol_column(203), VolCmd::Vibrato(0));
    }
}
