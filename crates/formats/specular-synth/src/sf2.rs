//! SoundFont 2 (`.sf2`, RIFF `sfbk`), from the SoundFont 2.04 specification: the INFO list,
//! the 16-bit sample data (`smpl`; the low byte of 24-bit `sm24` is not used) and the
//! "hydra" of nine tables (`phdr` … `shdr`). Presets and instruments are flattened into
//! [`Zone`]s: instrument generators are absolute (a zone's own over the global zone's),
//! preset generators add to them, key and velocity ranges intersect. Modulators are not
//! read; the specification's default ones are the synthesiser's (velocity and the channel's
//! volume, expression and pan).
//!
//! SF3 files (Ogg Vorbis samples) are refused in words.

use crate::{
    Bank, BankKind, Envelope, LoopMode, Preset, Result, Sample, SampleFormat, Zone, bad, chunks,
    list, text, u16le, u32le,
};

pub fn is_sf2(d: &[u8]) -> bool {
    d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"sfbk"
}

/// Generator numbers used here (SoundFont 2.04 §8.1.2).
mod g {
    pub const START_OFF: usize = 0;
    pub const END_OFF: usize = 1;
    pub const LOOP_START_OFF: usize = 2;
    pub const LOOP_END_OFF: usize = 3;
    pub const START_COARSE: usize = 4;
    pub const FILTER_FC: usize = 8;
    pub const FILTER_Q: usize = 9;
    pub const END_COARSE: usize = 12;
    pub const PAN: usize = 17;
    pub const DELAY_VOL: usize = 33;
    pub const ATTACK_VOL: usize = 34;
    pub const HOLD_VOL: usize = 35;
    pub const DECAY_VOL: usize = 36;
    pub const SUSTAIN_VOL: usize = 37;
    pub const RELEASE_VOL: usize = 38;
    pub const INSTRUMENT: usize = 41;
    pub const KEY_RANGE: usize = 43;
    pub const VEL_RANGE: usize = 44;
    pub const LOOP_START_COARSE: usize = 45;
    pub const KEYNUM: usize = 46;
    pub const ATTENUATION: usize = 48;
    pub const LOOP_END_COARSE: usize = 50;
    pub const COARSE_TUNE: usize = 51;
    pub const FINE_TUNE: usize = 52;
    pub const SAMPLE_ID: usize = 53;
    pub const SAMPLE_MODES: usize = 54;
    pub const SCALE_TUNING: usize = 56;
    pub const EXCLUSIVE: usize = 57;
    pub const ROOT_KEY: usize = 58;
    pub const COUNT: usize = 61;
}

/// A zone's generator values (`None`: not given).
type Gens = [Option<i16>; g::COUNT];

fn defaults() -> [i16; g::COUNT] {
    let mut d = [0i16; g::COUNT];
    d[g::FILTER_FC] = 13500;
    for g in [21, 23, 25, 26, 27, 28, 30, 33, 34, 35, 36, 38] {
        d[g] = -12000;
    }
    d[g::KEY_RANGE] = 0x7F00u16 as i16;
    d[g::VEL_RANGE] = 0x7F00u16 as i16;
    d[g::KEYNUM] = -1;
    d[47] = -1;
    d[g::SCALE_TUNING] = 100;
    d[g::ROOT_KEY] = -1;
    d
}

/// A range generator's (low, high).
fn range(v: i16) -> (u8, u8) {
    let v = v as u16;
    ((v & 0xFF) as u8, (v >> 8) as u8)
}

/// Seconds of a timecents value.
fn tc(v: f32) -> f32 {
    if v <= -12000.0 {
        0.0
    } else {
        2f32.powf(v / 1200.0).min(100.0)
    }
}

/// The zones of a bag table range: each zone's generators, in order.
fn bags(
    d: &[u8],
    bag: &std::ops::Range<usize>,
    gens: &std::ops::Range<usize>,
    first: usize,
    last: usize,
) -> Vec<Gens> {
    let nbags = bag.len() / 4;
    let ngens = gens.len() / 4;
    let mut out = Vec::new();
    for b in first..last.min(nbags.saturating_sub(1)) {
        let g0 = usize::from(u16le(d, bag.start + 4 * b));
        let g1 = usize::from(u16le(d, bag.start + 4 * (b + 1)));
        let mut z: Gens = [None; g::COUNT];
        for g in g0..g1.min(ngens) {
            let at = gens.start + 4 * g;
            let op = usize::from(u16le(d, at));
            let amount = u16le(d, at + 2) as i16;
            if op < g::COUNT {
                z[op] = Some(amount);
            }
        }
        out.push(z);
    }
    out
}

/// Reads the bank's tables; its samples' bytes are `d`'s, attached by [`Bank::read`] (or
/// [`Bank::with_data`]).
pub fn read(d: &[u8]) -> Result<Bank> {
    if !is_sf2(d) {
        return Err(bad("not a SoundFont"));
    }
    let top = chunks(d, 12, 8 + u32le(d, 4) as usize);
    let info = list(&top, d, b"INFO")
        .map(|r| crate::info_fields(d, r.clone()))
        .unwrap_or_default();
    if let Some(r) = list(&top, d, b"INFO") {
        let ver = chunks(d, r.start, r.end)
            .into_iter()
            .find(|(id, _)| id == b"ifil")
            .map(|(_, b)| u16le(d, b.start));
        if ver == Some(3) {
            return Err(crate::Error::Unsupported(
                "SoundFont 3 (Ogg Vorbis samples)".into(),
            ));
        }
    }
    let sdta = list(&top, d, b"sdta").ok_or_else(|| bad("SoundFont without sample data"))?;
    let smpl = chunks(d, sdta.start, sdta.end)
        .into_iter()
        .find(|(id, _)| id == b"smpl")
        .map(|(_, r)| r)
        .ok_or_else(|| bad("SoundFont without samples"))?;
    let pdta = list(&top, d, b"pdta").ok_or_else(|| bad("SoundFont without presets"))?;
    let hydra = chunks(d, pdta.start, pdta.end);
    let table = |name: &[u8; 4]| {
        hydra
            .iter()
            .find(|(id, _)| id == name)
            .map(|(_, r)| r.clone())
            .ok_or_else(|| {
                bad(&format!(
                    "SoundFont without {}",
                    String::from_utf8_lossy(name)
                ))
            })
    };
    let (phdr, pbag, pgen) = (table(b"phdr")?, table(b"pbag")?, table(b"pgen")?);
    let (inst, ibag, igen, shdr) = (
        table(b"inst")?,
        table(b"ibag")?,
        table(b"igen")?,
        table(b"shdr")?,
    );

    // Samples (the last header is the terminal "EOS").
    let frames = smpl.len() / 2;
    let nsamples = (shdr.len() / 46).saturating_sub(1);
    let mut samples = Vec::with_capacity(nsamples);
    for k in 0..nsamples {
        let at = shdr.start + 46 * k;
        let start = (u32le(d, at + 20) as usize).min(frames);
        let end = (u32le(d, at + 24) as usize).clamp(start, frames);
        let ls = (u32le(d, at + 28) as usize).saturating_sub(start);
        let le = (u32le(d, at + 32) as usize).saturating_sub(start);
        let kind = u16le(d, at + 44);
        samples.push(Sample {
            name: text(&d[at..at + 20]),
            rate: u32le(d, at + 36).clamp(400, 384_000),
            format: SampleFormat::I16,
            offset: smpl.start + 2 * start,
            // ROM samples (bit 15) have no data here.
            len: if kind & 0x8000 != 0 { 0 } else { end - start },
            loop_start: ls,
            loop_end: le,
            root: d[at + 40].min(127),
            cents: i16::from(d[at + 41] as i8),
        });
    }

    // Instruments: their zones with absolute values.
    let ninst = (inst.len() / 22).saturating_sub(1);
    let mut instruments: Vec<Vec<Gens>> = Vec::with_capacity(ninst);
    for k in 0..ninst {
        let at = inst.start + 22 * k;
        let b0 = usize::from(u16le(d, at + 20));
        let b1 = usize::from(u16le(d, at + 22 + 20));
        let zones = bags(d, &ibag, &igen, b0, b1);
        let mut global: Gens = [None; g::COUNT];
        let mut out = Vec::new();
        for (i, z) in zones.into_iter().enumerate() {
            if z[g::SAMPLE_ID].is_none() {
                if i == 0 {
                    global = z;
                }
                continue;
            }
            let mut m = global;
            for g in 0..g::COUNT {
                if z[g].is_some() {
                    m[g] = z[g];
                }
            }
            out.push(m);
        }
        instruments.push(out);
    }

    // Presets: their zones added to their instruments'.
    let npresets = (phdr.len() / 38).saturating_sub(1);
    let mut presets = Vec::with_capacity(npresets);
    let def = defaults();
    for k in 0..npresets {
        let at = phdr.start + 38 * k;
        let b0 = usize::from(u16le(d, at + 24));
        let b1 = usize::from(u16le(d, at + 38 + 24));
        let zones = bags(d, &pbag, &pgen, b0, b1);
        let mut global: Gens = [None; g::COUNT];
        let mut out = Vec::new();
        for (i, pz) in zones.into_iter().enumerate() {
            let Some(inst_no) = pz[g::INSTRUMENT] else {
                if i == 0 {
                    global = pz;
                }
                continue;
            };
            let mut p = global;
            for g in 0..g::COUNT {
                if pz[g].is_some() {
                    p[g] = pz[g];
                }
            }
            let Some(izones) = instruments.get(inst_no as u16 as usize) else {
                continue;
            };
            for iz in izones {
                if let Some(z) = zone(&def, iz, &p, &samples) {
                    out.push(z);
                }
            }
        }
        presets.push(Preset {
            name: text(&d[at..at + 20]),
            program: u16le(d, at + 20).min(127) as u8,
            bank: u16le(d, at + 22),
            zones: out,
        });
    }
    Ok(Bank {
        kind: BankKind::SoundFont,
        info,
        presets,
        samples,
        data: std::sync::Arc::new(Vec::new()),
    })
}

/// An instrument zone `iz` under a preset zone `p`.
fn zone(def: &[i16; g::COUNT], iz: &Gens, p: &Gens, samples: &[Sample]) -> Option<Zone> {
    let sample = iz[g::SAMPLE_ID]? as u16 as usize;
    let s = samples.get(sample)?;
    if s.len == 0 {
        return None;
    }
    let i = |g: usize| f32::from(iz[g].unwrap_or(def[g]));
    // Preset values add (and ranges intersect); address and sample generators are the
    // instrument's only.
    let v = |g: usize| i(g) + f32::from(p[g].unwrap_or(0));
    let ir = |g: usize| range(iz[g].unwrap_or(def[g]));
    let pr = |g: usize| range(p[g].unwrap_or(def[g]));
    let (ik, pk) = (ir(g::KEY_RANGE), pr(g::KEY_RANGE));
    let (iv, pv) = (ir(g::VEL_RANGE), pr(g::VEL_RANGE));
    let keys = (ik.0.max(pk.0), ik.1.min(pk.1));
    let vels = (iv.0.max(pv.0), iv.1.min(pv.1));
    if keys.0 > keys.1 || vels.0 > vels.1 {
        return None;
    }
    let root = match iz[g::ROOT_KEY] {
        Some(r) if (0..=127).contains(&r) => r as u8,
        _ => s.root,
    };
    let off = |fine: usize, coarse: usize| {
        i64::from(i(fine) as i32) + 32768 * i64::from(i(coarse) as i32)
    };
    let loop_mode = match iz[g::SAMPLE_MODES].unwrap_or(0) & 3 {
        1 => LoopMode::Continuous,
        3 => LoopMode::UntilRelease,
        _ => LoopMode::None,
    };
    let fc = v(g::FILTER_FC);
    Some(Zone {
        keys,
        vels,
        sample,
        root,
        tune: v(g::COARSE_TUNE) * 100.0 + v(g::FINE_TUNE) + f32::from(s.cents),
        scale: v(g::SCALE_TUNING),
        loop_mode,
        start_off: off(g::START_OFF, g::START_COARSE),
        end_off: off(g::END_OFF, g::END_COARSE),
        loop_start_off: off(g::LOOP_START_OFF, g::LOOP_START_COARSE),
        loop_end_off: off(g::LOOP_END_OFF, g::LOOP_END_COARSE),
        // Centibels; SoundFont players scale it by 0.4, as the EMU hardware did.
        atten_db: (v(g::ATTENUATION).clamp(0.0, 1440.0)) * 0.04,
        pan: (v(g::PAN) / 500.0).clamp(-1.0, 1.0),
        env: Envelope {
            delay: tc(v(g::DELAY_VOL)),
            attack: tc(v(g::ATTACK_VOL)),
            hold: tc(v(g::HOLD_VOL)),
            decay: tc(v(g::DECAY_VOL)),
            sustain_db: (v(g::SUSTAIN_VOL) / 10.0).clamp(0.0, 144.0),
            release: tc(v(g::RELEASE_VOL)).max(0.005),
        },
        cutoff: (fc < 13500.0).then(|| 8.176 * 2f32.powf(fc.max(1500.0) / 1200.0)),
        q_db: (v(g::FILTER_Q) / 10.0).clamp(0.0, 96.0),
        exclusive: iz[g::EXCLUSIVE].unwrap_or(0) as u16,
        fixed_key: iz[g::KEYNUM]
            .filter(|k| (0..=127).contains(k))
            .map(|k| k as u8),
    })
}

/// Builds a small SoundFont: one preset of one instrument over the given samples (each its
/// own key range), for tests.
#[doc(hidden)]
pub fn build(
    name: &str,
    presets: &[(&str, u16, u8)],
    samples: &[(&str, u32, u8, &[i16])],
) -> Vec<u8> {
    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }
    fn name20(s: &str) -> [u8; 20] {
        let mut n = [0u8; 20];
        for (d, c) in n.iter_mut().zip(s.bytes().take(19)) {
            *d = c;
        }
        n
    }
    fn listc(kind: &[u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
        let mut body = kind.to_vec();
        for p in parts {
            body.extend_from_slice(p);
        }
        chunk(b"LIST", &body)
    }
    let info = listc(
        b"INFO",
        &[
            chunk(b"ifil", &[2, 0, 1, 0]),
            chunk(b"isng", b"EMU8000\0"),
            chunk(b"INAM", format!("{name}\0").as_bytes()),
        ],
    );
    let mut smpl = Vec::new();
    let mut shdr = Vec::new();
    for (sname, rate, root, data) in samples {
        let start = (smpl.len() / 2) as u32;
        for s in data.iter() {
            smpl.extend_from_slice(&s.to_le_bytes());
        }
        let end = (smpl.len() / 2) as u32;
        for _ in 0..46 {
            smpl.extend_from_slice(&0i16.to_le_bytes());
        }
        shdr.extend_from_slice(&name20(sname));
        for v in [start, end, start, end, *rate] {
            shdr.extend_from_slice(&v.to_le_bytes());
        }
        shdr.extend_from_slice(&[*root, 0, 0, 0, 1, 0]);
    }
    shdr.extend_from_slice(&name20("EOS"));
    shdr.extend_from_slice(&[0; 26]);
    let sdta = listc(b"sdta", &[chunk(b"smpl", &smpl)]);
    // One instrument: a zone per sample, keys split evenly.
    let n = samples.len().max(1);
    let mut ibag = Vec::new();
    let mut igen = Vec::new();
    for k in 0..samples.len() {
        ibag.extend_from_slice(&((igen.len() / 4) as u16).to_le_bytes());
        ibag.extend_from_slice(&0u16.to_le_bytes());
        let lo = (k * 128 / n) as u8;
        let hi = ((k + 1) * 128 / n - 1) as u8;
        igen.extend_from_slice(&(g::KEY_RANGE as u16).to_le_bytes());
        igen.extend_from_slice(&[lo, hi]);
        igen.extend_from_slice(&(g::SAMPLE_ID as u16).to_le_bytes());
        igen.extend_from_slice(&(k as u16).to_le_bytes());
    }
    ibag.extend_from_slice(&((igen.len() / 4) as u16).to_le_bytes());
    ibag.extend_from_slice(&0u16.to_le_bytes());
    igen.extend_from_slice(&[0; 4]);
    let mut inst = Vec::new();
    inst.extend_from_slice(&name20("Instrument"));
    inst.extend_from_slice(&0u16.to_le_bytes());
    inst.extend_from_slice(&name20("EOI"));
    inst.extend_from_slice(&(samples.len() as u16).to_le_bytes());
    let mut phdr = Vec::new();
    let mut pbag = Vec::new();
    let mut pgen = Vec::new();
    for (k, (pname, bank, prog)) in presets.iter().enumerate() {
        phdr.extend_from_slice(&name20(pname));
        phdr.extend_from_slice(&u16::from(*prog).to_le_bytes());
        phdr.extend_from_slice(&bank.to_le_bytes());
        phdr.extend_from_slice(&(k as u16).to_le_bytes());
        phdr.extend_from_slice(&[0; 12]);
        pbag.extend_from_slice(&((pgen.len() / 4) as u16).to_le_bytes());
        pbag.extend_from_slice(&0u16.to_le_bytes());
        pgen.extend_from_slice(&(g::INSTRUMENT as u16).to_le_bytes());
        pgen.extend_from_slice(&0u16.to_le_bytes());
    }
    phdr.extend_from_slice(&name20("EOP"));
    phdr.extend_from_slice(&[0; 4]);
    phdr.extend_from_slice(&(presets.len() as u16).to_le_bytes());
    phdr.extend_from_slice(&[0; 12]);
    pbag.extend_from_slice(&((pgen.len() / 4) as u16).to_le_bytes());
    pbag.extend_from_slice(&0u16.to_le_bytes());
    pgen.extend_from_slice(&[0; 4]);
    let pdta = listc(
        b"pdta",
        &[
            chunk(b"phdr", &phdr),
            chunk(b"pbag", &pbag),
            chunk(b"pmod", &[0; 10]),
            chunk(b"pgen", &pgen),
            chunk(b"inst", &inst),
            chunk(b"ibag", &ibag),
            chunk(b"imod", &[0; 10]),
            chunk(b"igen", &igen),
            chunk(b"shdr", &shdr),
        ],
    );
    let mut body = b"sfbk".to_vec();
    body.extend(info);
    body.extend(sdta);
    body.extend(pdta);
    chunk(b"RIFF", &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn reads_a_built_bank() {
        let a: Vec<i16> = (0..100).map(|i| (i * 300) as i16).collect();
        let b: Vec<i16> = (0..50).map(|i| -(i * 200) as i16).collect();
        let f = build(
            "Test",
            &[("Piano", 0, 0), ("Kit", 128, 0)],
            &[("A", 22050, 60, &a), ("B", 44100, 72, &b)],
        );
        let bank = read(&f).unwrap().with_data(Arc::new(f));
        assert_eq!(bank.info[0], ("title", "Test".into()));
        assert_eq!(bank.samples.len(), 2);
        assert_eq!(bank.samples[1].name, "B");
        assert_eq!(bank.samples[1].len, 50);
        assert_eq!(bank.samples[0].rate, 22050);
        assert_eq!(bank.presets.len(), 2);
        assert_eq!(bank.presets[1].bank, 128);
        let z = &bank.presets[0].zones;
        assert_eq!(z.len(), 2);
        assert_eq!(z[0].keys, (0, 63));
        assert_eq!(z[1].keys, (64, 127));
        assert_eq!(z[1].root, 72);
        assert!((bank.frame(&bank.samples[0], 2) - 600.0 / 32768.0).abs() < 1e-6);
        assert!(bank.find(5, 0).is_some());
    }
}
