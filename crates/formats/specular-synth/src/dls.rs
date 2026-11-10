//! Downloadable Sounds (`.dls`, RIFF `DLS `), levels 1 and 2, from the MMA's DLS
//! specifications: the instrument list (`lins`: `ins ` lists of `insh`, regions `rgn `/`rgn2`
//! with `rgnh`, `wsmp`, `wlnk`, articulation `art1`/`art2`), the pool table (`ptbl`) and the
//! wave pool (`wvpl`: `wave` lists of `fmt `, `data`, `wsmp`). Read from articulation: the
//! volume envelope and pan; other connections (LFOs, filters) are not used.
//!
//! Waves other than 8- or 16-bit PCM are left out of play (listed, not sounded).

use crate::{
    Bank, BankKind, Envelope, LoopMode, Preset, Result, Sample, SampleFormat, Zone, bad, chunks,
    i32le, list, u16le, u32le,
};

pub fn is_dls(d: &[u8]) -> bool {
    d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"DLS "
}

/// A `wsmp` chunk: unity note, fine tune (cents), attenuation (dB), the first loop.
#[derive(Clone, Copy, Debug, Default)]
struct Wsmp {
    unity: u8,
    fine: i16,
    atten_db: f32,
    looped: Option<(usize, usize)>,
}

fn wsmp(d: &[u8], r: std::ops::Range<usize>) -> Wsmp {
    let b = &d[r];
    let size = u32le(b, 0) as usize;
    let unity = u16le(b, 4).min(127) as u8;
    let fine = u16le(b, 6) as i16;
    // Gain in 1/655360 dB.
    let gain = i32le(b, 8);
    let loops = u32le(b, 16);
    let looped = (loops > 0).then(|| {
        let at = size.max(20);
        let start = u32le(b, at + 8) as usize;
        let len = u32le(b, at + 12) as usize;
        (start, start + len)
    });
    Wsmp {
        unity,
        fine,
        atten_db: (-(gain as f32) / 655_360.0).clamp(0.0, 144.0),
        looped: looped.filter(|&(s, e)| e > s),
    }
}

/// Seconds of an articulation time value (absolute timecents × 65536).
fn tc(v: i32) -> f32 {
    if v == i32::MIN {
        return 0.0;
    }
    2f32.powf(v as f32 / 65536.0 / 1200.0).min(100.0)
}

/// The envelope and pan of articulation chunks (`art1`, `art2`) under a `lart`/`lar2` list.
fn articulation(d: &[u8], r: std::ops::Range<usize>, env: &mut Envelope, pan: &mut f32) {
    for (id, c) in chunks(d, r.start, r.end) {
        if id != *b"art1" && id != *b"art2" {
            continue;
        }
        let b = &d[c];
        let size = u32le(b, 0) as usize;
        let n = u32le(b, 4) as usize;
        for k in 0..n.min(1024) {
            let at = size + 12 * k;
            if at + 12 > b.len() {
                break;
            }
            let (src, ctl, dst) = (u16le(b, at), u16le(b, at + 2), u16le(b, at + 4));
            let scale = i32le(b, at + 8);
            if src != 0 || ctl != 0 {
                continue;
            }
            match dst {
                0x0004 => *pan = (scale as f32 / 65536.0 / 500.0).clamp(-1.0, 1.0),
                0x0206 => env.attack = tc(scale),
                0x0207 => env.decay = tc(scale),
                0x0209 => env.release = tc(scale).max(0.005),
                0x020A => {
                    // Sustain level in 0.1 % of full scale.
                    let level = (scale as f32 / 65536.0 / 1000.0).clamp(0.0, 1.0);
                    env.sustain_db = if level <= 0.0 {
                        144.0
                    } else {
                        (-20.0 * level.log10()).min(144.0)
                    };
                }
                0x020B => env.delay = tc(scale),
                0x020C => env.hold = tc(scale),
                _ => {}
            }
        }
    }
}

/// Reads the bank's tables; its samples' bytes are `d`'s, attached by [`Bank::read`] (or
/// [`Bank::with_data`]).
pub fn read(d: &[u8]) -> Result<Bank> {
    if !is_dls(d) {
        return Err(bad("not a DLS file"));
    }
    let top = chunks(d, 12, 8 + u32le(d, 4) as usize);
    let info = list(&top, d, b"INFO")
        .map(|r| crate::info_fields(d, r))
        .unwrap_or_default();
    let wvpl = list(&top, d, b"wvpl").ok_or_else(|| bad("DLS without a wave pool"))?;
    let ptbl = top
        .iter()
        .find(|(id, _)| id == b"ptbl")
        .map(|(_, r)| r.clone());
    // The waves, by their offsets from the pool's start.
    let mut samples = Vec::new();
    let mut offsets = Vec::new();
    let mut wsmps = Vec::new();
    let mut at = wvpl.start;
    for (id, r) in chunks(d, wvpl.start, wvpl.end) {
        let rel = at.saturating_sub(wvpl.start);
        at = r.end + ((r.end - r.start) & 1);
        if id != *b"LIST" || d.get(r.start..r.start + 4) != Some(b"wave") {
            continue;
        }
        let parts = chunks(d, r.start + 4, r.end);
        let fmt = parts
            .iter()
            .find(|(i, _)| i == b"fmt ")
            .map(|(_, r)| r.clone());
        let body = parts
            .iter()
            .find(|(i, _)| i == b"data")
            .map(|(_, r)| r.clone());
        let ws = parts
            .iter()
            .find(|(i, _)| i == b"wsmp")
            .map(|(_, r)| wsmp(d, r.clone()))
            .unwrap_or(Wsmp {
                unity: 60,
                ..Wsmp::default()
            });
        let name = list(&parts, d, b"INFO")
            .and_then(|r| {
                crate::info_fields(d, r)
                    .into_iter()
                    .find(|(k, _)| *k == "title")
                    .map(|(_, v)| v)
            })
            .unwrap_or_else(|| format!("Wave {}", samples.len() + 1));
        let (Some(fmt), Some(body)) = (fmt, body) else {
            continue;
        };
        let tag = u16le(d, fmt.start);
        let channels = u16le(d, fmt.start + 2).max(1);
        let rate = u32le(d, fmt.start + 4).clamp(400, 384_000);
        let bits = u16le(d, fmt.start + 14);
        let format = match (tag, bits) {
            (1, 16) => Some(SampleFormat::I16),
            (1, 8) => Some(SampleFormat::U8),
            _ => None,
        };
        // Only the first channel of a multichannel wave is played (DLS waves are mono).
        let fb = format.map_or(1, |f| f.bytes()) * usize::from(channels);
        let len = if format.is_some() && channels == 1 {
            body.len() / fb
        } else {
            0
        };
        let (ls, le) = ws.looped.unwrap_or((0, 0));
        offsets.push(rel);
        wsmps.push(ws);
        samples.push(Sample {
            name,
            rate,
            format: format.unwrap_or(SampleFormat::I16),
            offset: body.start,
            len,
            loop_start: ls.min(len),
            loop_end: le.min(len),
            root: ws.unity,
            cents: ws.fine,
        });
    }
    // Pool table cue k → wave index.
    let cue = |k: usize| -> Option<usize> {
        match &ptbl {
            Some(p) => {
                let size = u32le(d, p.start) as usize;
                let n = u32le(d, p.start + 4) as usize;
                if k >= n {
                    return None;
                }
                let off = u32le(d, p.start + size + 4 * k) as usize;
                offsets.iter().position(|&o| o == off)
            }
            None => (k < samples.len()).then_some(k),
        }
    };
    let mut presets = Vec::new();
    if let Some(lins) = list(&top, d, b"lins") {
        for (id, r) in chunks(d, lins.start, lins.end) {
            if id != *b"LIST" || d.get(r.start..r.start + 4) != Some(b"ins ") {
                continue;
            }
            let parts = chunks(d, r.start + 4, r.end);
            let Some(insh) = parts
                .iter()
                .find(|(i, _)| i == b"insh")
                .map(|(_, r)| r.start)
            else {
                continue;
            };
            let ulbank = u32le(d, insh + 4);
            let program = (u32le(d, insh + 8) & 0x7F) as u8;
            let drum = ulbank & 0x8000_0000 != 0;
            let bank = if drum {
                128
            } else {
                ((ulbank >> 8) & 0x7F) as u16
            };
            let name = list(&parts, d, b"INFO")
                .and_then(|r| {
                    crate::info_fields(d, r)
                        .into_iter()
                        .find(|(k, _)| *k == "title")
                        .map(|(_, v)| v)
                })
                .unwrap_or_else(|| format!("Instrument {program}"));
            let mut ienv = Envelope::default();
            let mut ipan = 0.0;
            if let Some(a) = list(&parts, d, b"lart").or_else(|| list(&parts, d, b"lar2")) {
                articulation(d, a, &mut ienv, &mut ipan);
            }
            let mut zones = Vec::new();
            if let Some(lrgn) = list(&parts, d, b"lrgn") {
                for (rid, rr) in chunks(d, lrgn.start, lrgn.end) {
                    let kind = d.get(rr.start..rr.start + 4);
                    if rid != *b"LIST" || !matches!(kind, Some(b"rgn " | b"rgn2")) {
                        continue;
                    }
                    let rp = chunks(d, rr.start + 4, rr.end);
                    let Some(h) = rp.iter().find(|(i, _)| i == b"rgnh").map(|(_, r)| r.start)
                    else {
                        continue;
                    };
                    let Some(w) = rp.iter().find(|(i, _)| i == b"wlnk").map(|(_, r)| r.start)
                    else {
                        continue;
                    };
                    let Some(sample) = cue(u32le(d, w + 8) as usize) else {
                        continue;
                    };
                    let rw = rp
                        .iter()
                        .find(|(i, _)| i == b"wsmp")
                        .map(|(_, r)| wsmp(d, r.clone()))
                        .unwrap_or(wsmps[sample]);
                    let mut env = ienv;
                    let mut pan = ipan;
                    if let Some(a) = list(&rp, d, b"lart").or_else(|| list(&rp, d, b"lar2")) {
                        articulation(d, a, &mut env, &mut pan);
                    }
                    let s = &samples[sample];
                    let mut z = Zone::whole(sample, rw.unity);
                    z.keys = (u16le(d, h).min(127) as u8, u16le(d, h + 2).min(127) as u8);
                    z.vels = (
                        u16le(d, h + 4).min(127) as u8,
                        u16le(d, h + 6).min(127) as u8,
                    );
                    if z.vels.1 == 0 {
                        z.vels = (0, 127);
                    }
                    z.tune = f32::from(rw.fine);
                    z.atten_db = rw.atten_db;
                    z.pan = pan;
                    z.env = env;
                    z.exclusive = u16le(d, h + 10);
                    if let Some((ls, le)) = rw.looped {
                        z.loop_mode = LoopMode::Continuous;
                        z.loop_start_off = ls as i64 - s.loop_start as i64;
                        z.loop_end_off = le as i64 - s.loop_end as i64;
                    }
                    zones.push(z);
                }
            }
            presets.push(Preset {
                name,
                bank,
                program,
                zones,
            });
        }
    }
    Ok(Bank {
        kind: BankKind::Dls,
        info,
        presets,
        samples,
        data: std::sync::Arc::new(Vec::new()),
    })
}

/// Builds a small DLS file of one instrument and one 16-bit wave, for tests.
#[doc(hidden)]
pub fn build(name: &str, wave: &[i16], rate: u32, unity: u8) -> Vec<u8> {
    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }
    fn listc(kind: &[u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
        let mut body = kind.to_vec();
        for p in parts {
            body.extend_from_slice(p);
        }
        chunk(b"LIST", &body)
    }
    let mut ws = Vec::new();
    ws.extend_from_slice(&20u32.to_le_bytes());
    ws.extend_from_slice(&u16::from(unity).to_le_bytes());
    ws.extend_from_slice(&0i16.to_le_bytes());
    ws.extend_from_slice(&0i32.to_le_bytes());
    ws.extend_from_slice(&0u32.to_le_bytes());
    ws.extend_from_slice(&0u32.to_le_bytes());
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * 2).to_le_bytes());
    fmt.extend_from_slice(&2u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let pcm: Vec<u8> = wave.iter().flat_map(|s| s.to_le_bytes()).collect();
    let wave_list = listc(
        b"wave",
        &[
            chunk(b"fmt ", &fmt),
            chunk(b"wsmp", &ws),
            chunk(b"data", &pcm),
            listc(b"INFO", &[chunk(b"INAM", b"Sine\0")]),
        ],
    );
    let wvpl = listc(b"wvpl", &[wave_list]);
    let mut ptbl = 8u32.to_le_bytes().to_vec();
    ptbl.extend_from_slice(&1u32.to_le_bytes());
    ptbl.extend_from_slice(&0u32.to_le_bytes());
    let mut rgnh = Vec::new();
    for v in [0u16, 127, 0, 127, 0, 0] {
        rgnh.extend_from_slice(&v.to_le_bytes());
    }
    let mut wlnk = Vec::new();
    wlnk.extend_from_slice(&0u16.to_le_bytes());
    wlnk.extend_from_slice(&0u16.to_le_bytes());
    wlnk.extend_from_slice(&1u32.to_le_bytes());
    wlnk.extend_from_slice(&0u32.to_le_bytes());
    let rgn = listc(b"rgn ", &[chunk(b"rgnh", &rgnh), chunk(b"wlnk", &wlnk)]);
    let mut insh = Vec::new();
    for v in [1u32, 0, 0] {
        insh.extend_from_slice(&v.to_le_bytes());
    }
    let ins = listc(
        b"ins ",
        &[
            chunk(b"insh", &insh),
            listc(b"lrgn", &[rgn]),
            listc(b"INFO", &[chunk(b"INAM", format!("{name}\0").as_bytes())]),
        ],
    );
    let mut body = b"DLS ".to_vec();
    body.extend(chunk(b"colh", &1u32.to_le_bytes()));
    body.extend(listc(b"lins", &[ins]));
    body.extend(chunk(b"ptbl", &ptbl));
    body.extend(wvpl);
    body.extend(listc(b"INFO", &[chunk(b"INAM", b"Test bank\0")]));
    chunk(b"RIFF", &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn reads_a_built_bank() {
        let w: Vec<i16> = (0..200)
            .map(|i| ((i as f32 * 0.1).sin() * 10000.0) as i16)
            .collect();
        let f = build("Organ", &w, 22050, 67);
        assert!(is_dls(&f));
        let b = read(&f).unwrap().with_data(Arc::new(f));
        assert_eq!(b.info[0].1, "Test bank");
        assert_eq!(b.samples.len(), 1);
        assert_eq!(b.samples[0].name, "Sine");
        assert_eq!(b.samples[0].len, 200);
        assert_eq!(b.samples[0].root, 67);
        assert_eq!(b.presets.len(), 1);
        assert_eq!(b.presets[0].name, "Organ");
        assert_eq!(b.presets[0].zones.len(), 1);
        assert_eq!(b.presets[0].zones[0].sample, 0);
    }
}
