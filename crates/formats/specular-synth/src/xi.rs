//! FastTracker 2 instruments (`.xi`): the "Extended Instrument: " header, the keyboard map
//! (which sample each of 96 notes plays), the volume envelope's fade-out, then up to 16
//! sample headers and their delta-coded 8- or 16-bit data. Read into a [`Bank`] of one
//! preset; the samples are decoded into 16-bit frames.
//!
//! Pitch is FastTracker's: a sample plays its "C-4" at 8363 Hz shifted by its relative note
//! and fine-tune (128ths of a semitone), which is MIDI key 60 here.

use std::sync::Arc;

use crate::{
    Bank, BankKind, Envelope, LoopMode, Preset, Result, Sample, SampleFormat, Zone, bad, text,
    u16le, u32le,
};

const MAGIC: &[u8] = b"Extended Instrument: ";

pub fn is_xi(d: &[u8]) -> bool {
    d.len() >= 298 && d.starts_with(MAGIC) && d[43] == 0x1A
}

/// Header bytes before the sample headers.
const HEAD: usize = 298;
const SAMPLE_HEAD: usize = 40;

pub fn read(d: &[u8]) -> Result<Bank> {
    if !is_xi(d) {
        return Err(bad("not a FastTracker 2 instrument"));
    }
    let name = text(&d[21..43]);
    let tracker = text(&d[44..64]);
    let map = &d[66..162];
    let fadeout = u16le(d, 272);
    let n = usize::from(u16le(d, 296)).min(16);
    if HEAD + n * SAMPLE_HEAD > d.len() {
        return Err(bad("FastTracker 2 instrument cut short"));
    }
    let mut pcm: Vec<u8> = Vec::new();
    let mut samples = Vec::new();
    let mut data_at = HEAD + n * SAMPLE_HEAD;
    let mut relnotes = Vec::new();
    for k in 0..n {
        let h = HEAD + k * SAMPLE_HEAD;
        let bytes = u32le(d, h) as usize;
        let loop_start = u32le(d, h + 4) as usize;
        let loop_len = u32le(d, h + 8) as usize;
        let volume = d[h + 12].min(64);
        let finetune = d[h + 13] as i8;
        let kind = d[h + 14];
        let relnote = d[h + 16] as i8;
        let sname = text(&d[h + 18..h + 40]);
        let wide = kind & 0x10 != 0;
        let body = d
            .get(data_at..data_at.saturating_add(bytes))
            .unwrap_or(&d[data_at.min(d.len())..]);
        data_at = data_at.saturating_add(bytes);
        let offset = pcm.len();
        // Delta-coded: each value is the previous plus the stored difference.
        let frames = if wide {
            let mut acc = 0i16;
            for c in body.chunks_exact(2) {
                acc = acc.wrapping_add(i16::from_le_bytes([c[0], c[1]]));
                pcm.extend_from_slice(&acc.to_le_bytes());
            }
            body.len() / 2
        } else {
            let mut acc = 0i8;
            for &b in body {
                acc = acc.wrapping_add(b as i8);
                pcm.extend_from_slice(&(i16::from(acc) << 8).to_le_bytes());
            }
            body.len()
        };
        let div = if wide { 2 } else { 1 };
        let (ls, le) = (loop_start / div, (loop_start + loop_len) / div);
        // C-4 at 8363 Hz, moved by the relative note and fine-tune.
        let semis = f64::from(relnote) + f64::from(finetune) / 128.0;
        let rate = (8363.0 * 2f64.powf(semis / 12.0))
            .round()
            .clamp(400.0, 384_000.0) as u32;
        samples.push(Sample {
            name: if sname.is_empty() {
                format!("Sample {}", k + 1)
            } else {
                sname
            },
            rate,
            format: SampleFormat::I16,
            offset,
            len: frames,
            loop_start: ls.min(frames),
            loop_end: le.min(frames),
            root: 60,
            cents: 0,
        });
        relnotes.push((kind & 3, volume));
    }
    // One zone per run of keys mapped to the same sample. XI note 0 is C-0 (MIDI 12).
    let mut zones = Vec::new();
    let mut key = 0usize;
    while key < 96 {
        let s = usize::from(map[key]);
        let mut end = key;
        while end + 1 < 96 && map[end + 1] == map[key] {
            end += 1;
        }
        if let Some(smp) = samples.get(s)
            && smp.len > 0
        {
            let (lp, vol) = relnotes[s];
            let mut z = Zone::whole(s, 60);
            z.keys = ((key + 12).min(127) as u8, (end + 12).min(127) as u8);
            z.loop_mode = if lp != 0 && smp.loop_end > smp.loop_start {
                LoopMode::Continuous
            } else {
                LoopMode::None
            };
            z.atten_db = if vol == 0 {
                96.0
            } else {
                -20.0 * (f32::from(vol) / 64.0).log10()
            };
            // FT2's fade-out: the volume falls by fadeout/32768 every tick (50 a second).
            let release = if fadeout == 0 {
                0.05
            } else {
                (32768.0 / f32::from(fadeout) / 50.0).clamp(0.01, 20.0)
            };
            z.env = Envelope {
                release,
                ..Envelope::default()
            };
            zones.push(z);
        }
        key = end + 1;
    }
    let mut info = vec![("title", name.clone())];
    if !tracker.is_empty() {
        info.push(("encoder", tracker));
    }
    info.retain(|(_, v)| !v.is_empty());
    Ok(Bank {
        kind: BankKind::Xi,
        info,
        presets: vec![Preset {
            name,
            bank: 0,
            program: 0,
            zones,
        }],
        samples,
        data: Arc::new(pcm),
    })
}

/// Builds an XI file with the given 8- or 16-bit samples (stored delta-coded), for tests.
#[doc(hidden)]
pub fn build(name: &str, samples: &[(&str, bool, &[i16])]) -> Vec<u8> {
    let mut d = vec![0u8; HEAD];
    d[..21].copy_from_slice(MAGIC);
    for (o, c) in d[21..43].iter_mut().zip(name.bytes()) {
        *o = c;
    }
    d[43] = 0x1A;
    for (o, c) in d[44..64].iter_mut().zip(b"FastTracker v2.00".iter()) {
        *o = *c;
    }
    d[64..66].copy_from_slice(&0x0102u16.to_le_bytes());
    for k in 0..96 {
        d[66 + k] = (k * samples.len() / 96) as u8;
    }
    d[272..274].copy_from_slice(&1024u16.to_le_bytes());
    d[296..298].copy_from_slice(&(samples.len() as u16).to_le_bytes());
    let mut bodies = Vec::new();
    for (sname, wide, data) in samples {
        let mut h = vec![0u8; SAMPLE_HEAD];
        let mut body = Vec::new();
        if *wide {
            let mut prev = 0i16;
            for &s in data.iter() {
                body.extend_from_slice(&s.wrapping_sub(prev).to_le_bytes());
                prev = s;
            }
            h[14] = 0x10;
        } else {
            let mut prev = 0i8;
            for &s in data.iter() {
                let v = (s >> 8) as i8;
                body.push(v.wrapping_sub(prev) as u8);
                prev = v;
            }
        }
        h[..4].copy_from_slice(&(body.len() as u32).to_le_bytes());
        h[12] = 64;
        for (o, c) in h[18..40].iter_mut().zip(sname.bytes()) {
            *o = c;
        }
        d.extend_from_slice(&h);
        bodies.push(body);
    }
    for b in bodies {
        d.extend(b);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_coding_round_trips() {
        let a: Vec<i16> = (0..64).map(|i| (i * 512 - 16000) as i16).collect();
        let b: Vec<i16> = (0..32).map(|i| ((i % 7) * 3000 - 9000) as i16).collect();
        let f = build("Bass", &[("eight", false, &a), ("sixteen", true, &b)]);
        assert!(is_xi(&f));
        let bank = read(&f).unwrap();
        assert_eq!(bank.info[0], ("title", "Bass".into()));
        assert_eq!(bank.samples.len(), 2);
        assert_eq!(bank.samples[0].rate, 8363);
        let s0 = &bank.samples[0];
        for i in 0..64 {
            let want = f32::from(((a[i] >> 8) as i8) as i16 * 256) / 32768.0;
            assert!((bank.frame(s0, i) - want).abs() < 1e-6);
        }
        let s1 = &bank.samples[1];
        for i in 0..32 {
            assert!((bank.frame(s1, i) - f32::from(b[i]) / 32768.0).abs() < 1e-6);
        }
        assert_eq!(bank.presets[0].zones.len(), 2);
        assert_eq!(bank.presets[0].zones[0].keys.0, 12);
    }
}
