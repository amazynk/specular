//! The decoder against the MPEG-4 conformance streams (ISO/IEC 14496-26, fetched by
//! tools/corpus/fetch-aac.sh into `SPECULAR_AAC_CORPUS`): each `NAME.mp4` is demuxed here
//! (a minimal MP4 reader: the first sound track's AudioSpecificConfig and samples) and
//! decoded, then compared with the package's own 16-bit reference decode `NAME.s16` and with
//! ffmpeg's float decode `NAME.f32` when present. Prints the largest error, the RMS error
//! and the SNR of each; skips without the corpus.
//!
//! Checked to a bound: the AAC Main, AAC-LTP and 960-sample streams (am00_88, am05_44,
//! ap05_48, al04sf_48) against their conformance references.

use specular_aac::{Options, StreamDecoder};

const LSB: f64 = 1.0 / 32768.0;

/// Streams that must decode, with the largest error allowed against the conformance
/// reference (16-bit steps): a rounding difference of one step is expected, two where
/// coupling channels add up (am05_44, like al15_44).
const CHECKED: [(&str, f64); 6] = [
    ("al04_44", 2.0),
    ("am00_88", 1.0),
    ("am05_44", 2.0),
    ("ap05_48", 1.0),
    ("al04sf_48", 1.0),
    ("al05_44", 1.0),
];

/// RMS error allowed against the reference, in 16-bit steps. The references round
/// differently from our float output rounded to nearest: the AAC-LC streams already
/// checked against ffmpeg to 0.01 steps (al04_44) sit at 0.57 against theirs.
const MAX_RMS: f64 = 0.75;

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// The boxes directly inside `d`: (type, body).
fn boxes(d: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= d.len() {
        let Some(size) = be32(d, at) else { break };
        let kind: [u8; 4] = d[at + 4..at + 8].try_into().unwrap();
        let (head, size) = match size {
            1 => {
                let hi = u64::from(be32(d, at + 8).unwrap_or(0));
                let lo = u64::from(be32(d, at + 12).unwrap_or(0));
                (16, ((hi << 32) | lo) as usize)
            }
            0 => (8, d.len() - at),
            s => (8, s as usize),
        };
        if size < head || at + size > d.len() {
            break;
        }
        out.push((kind, &d[at + head..at + size]));
        at += size;
    }
    out
}

fn child<'a>(d: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(d)
        .into_iter()
        .find(|(k, _)| k == kind)
        .map(|(_, b)| b)
}

/// An MPEG-4 descriptor's (tag, body) at `at`, and where the next one starts.
fn descriptor(d: &[u8], at: usize) -> Option<(u8, &[u8], usize)> {
    let tag = *d.get(at)?;
    let mut len = 0usize;
    let mut i = at + 1;
    for _ in 0..4 {
        let b = *d.get(i)?;
        i += 1;
        len = (len << 7) | usize::from(b & 0x7f);
        if b & 0x80 == 0 {
            break;
        }
    }
    Some((tag, d.get(i..i + len)?, i + len))
}

/// The AudioSpecificConfig inside an `esds` body.
fn esds_asc(esds: &[u8]) -> Option<Vec<u8>> {
    let (tag, es, _) = descriptor(esds, 4)?;
    if tag != 3 {
        return None;
    }
    let flags = *es.get(2)?;
    let mut at = 3;
    if flags & 0x80 != 0 {
        at += 2;
    }
    if flags & 0x40 != 0 {
        at += 1 + usize::from(*es.get(at)?);
    }
    if flags & 0x20 != 0 {
        at += 2;
    }
    let (tag, dc, _) = descriptor(es, at)?;
    if tag != 4 {
        return None;
    }
    let (tag, dsi, _) = descriptor(dc, 13)?;
    (tag == 5).then(|| dsi.to_vec())
}

/// The first sound track's AudioSpecificConfig and samples.
fn demux(data: &[u8]) -> Option<(Vec<u8>, Vec<&[u8]>)> {
    let moov = child(data, b"moov")?;
    for (kind, trak) in boxes(moov) {
        if &kind != b"trak" {
            continue;
        }
        let mdia = child(trak, b"mdia")?;
        let hdlr = child(mdia, b"hdlr")?;
        if hdlr.get(8..12)? != b"soun" {
            continue;
        }
        let stbl = child(child(mdia, b"minf")?, b"stbl")?;
        let stsd = child(stbl, b"stsd")?;
        // stsd: version/flags, count, then the sample entry box
        let entry = boxes(stsd.get(8..)?).into_iter().next()?.1;
        // audio sample entry: 28 bytes of fields, then child boxes
        let esds = child(entry.get(28..)?, b"esds")?;
        let asc = esds_asc(esds)?;
        let stsz = child(stbl, b"stsz")?;
        let fixed = be32(stsz, 4)? as usize;
        let count = be32(stsz, 8)? as usize;
        let sizes: Vec<usize> = (0..count)
            .map(|i| {
                if fixed != 0 {
                    Some(fixed)
                } else {
                    be32(stsz, 12 + 4 * i).map(|v| v as usize)
                }
            })
            .collect::<Option<_>>()?;
        let offsets: Vec<usize> = if let Some(stco) = child(stbl, b"stco") {
            (0..be32(stco, 4)? as usize)
                .map(|i| be32(stco, 8 + 4 * i).map(|v| v as usize))
                .collect::<Option<_>>()?
        } else {
            let co64 = child(stbl, b"co64")?;
            (0..be32(co64, 4)? as usize)
                .map(|i| {
                    let hi = u64::from(be32(co64, 8 + 8 * i)?);
                    let lo = u64::from(be32(co64, 12 + 8 * i)?);
                    Some(((hi << 32) | lo) as usize)
                })
                .collect::<Option<_>>()?
        };
        let stsc = child(stbl, b"stsc")?;
        let runs: Vec<(usize, usize)> = (0..be32(stsc, 4)? as usize)
            .map(|i| {
                Some((
                    be32(stsc, 8 + 12 * i)? as usize,
                    be32(stsc, 12 + 12 * i)? as usize,
                ))
            })
            .collect::<Option<_>>()?;
        let mut samples = Vec::with_capacity(count);
        let mut s = 0;
        for (c, &off) in offsets.iter().enumerate() {
            let per = runs.iter().rev().find(|r| r.0 <= c + 1).map_or(0, |r| r.1);
            let mut at = off;
            for _ in 0..per {
                if s >= count {
                    break;
                }
                samples.push(data.get(at..at + sizes[s])?);
                at += sizes[s];
                s += 1;
            }
        }
        return Some((asc, samples));
    }
    None
}

struct Outcome {
    max_lsb: f64,
    rms_lsb: f64,
    snr_db: f64,
    offset: isize,
}

/// Compares `got` with `want` (interleaved, `ch` channels, both scaled to ±1) at the frame
/// offset that matches best.
fn compare(got: &[f32], want: &[f32], ch: usize, frame: usize) -> Outcome {
    let mut best: Option<Outcome> = None;
    for frames in [0usize, frame, 2 * frame] {
        for (g, w, sign) in [
            (got.get(frames * ch..).unwrap_or(&[]), want, 1),
            (got, want.get(frames * ch..).unwrap_or(&[]), -1),
        ] {
            let n = g.len().min(w.len());
            if n == 0 {
                continue;
            }
            let (mut sig, mut noise, mut max) = (0.0f64, 0.0f64, 0.0f64);
            for (a, b) in g[..n].iter().zip(&w[..n]) {
                let (a, b) = (f64::from(*a), f64::from(*b));
                sig += b * b;
                noise += (a - b) * (a - b);
                max = max.max((a - b).abs());
            }
            let o = Outcome {
                max_lsb: max / LSB,
                rms_lsb: (noise / n as f64).sqrt() / LSB,
                snr_db: 10.0 * (sig / noise.max(1e-30)).log10(),
                offset: sign * frames as isize,
            };
            if best.as_ref().is_none_or(|b| o.max_lsb < b.max_lsb) {
                best = Some(o);
            }
        }
    }
    best.unwrap_or(Outcome {
        max_lsb: f64::INFINITY,
        rms_lsb: f64::INFINITY,
        snr_db: 0.0,
        offset: 0,
    })
}

#[test]
fn conformance_streams() {
    let Some(dir) = std::env::var_os("SPECULAR_AAC_CORPUS") else {
        eprintln!("skipped: set SPECULAR_AAC_CORPUS (tools/corpus/fetch-aac.sh)");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let mut failed = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "mp4"))
        .collect();
    names.sort();
    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let bound = CHECKED.iter().find(|c| c.0 == name).map(|c| c.1);
        let data = std::fs::read(&path).unwrap();
        let Some((asc, samples)) = demux(&data) else {
            eprintln!("{name}: not demuxed");
            continue;
        };
        let mut dec = match StreamDecoder::new(&asc, &samples[..samples.len().min(48)], {
            Options {
                stereo_downmix: false,
            }
        }) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{name}: refused: {e}");
                if bound.is_some() {
                    failed.push(format!("{name}: refused: {e}"));
                }
                continue;
            }
        };
        dec.set_concealment(false);
        let mut got = Vec::new();
        let mut errors = 0;
        let t = std::time::Instant::now();
        for s in &samples {
            if dec.decode(s, &mut got).is_err() {
                errors += 1;
            }
        }
        let secs = t.elapsed().as_secs_f64();
        let ch = usize::from(dec.channels().max(1));
        let frame = dec.frame_len();
        let audio = got.len() as f64 / ch as f64 / f64::from(dec.sample_rate());
        let mut line = format!(
            "{name} ({}, {} ch, {} Hz, {frame}-sample frames, {errors} bad, {:.0}× real time):",
            dec.config().name(),
            ch,
            dec.sample_rate(),
            audio / secs.max(1e-9)
        );
        // the conformance reference, at 16 bits (multichannel references may be in another
        // channel order: only same-count references whose order matches are compared)
        let s16 = ["_reorder.s16", "s16"]
            .iter()
            .filter_map(|e| {
                let p = if e.starts_with('_') {
                    dir.join(format!("{name}{e}"))
                } else {
                    path.with_extension(e)
                };
                std::fs::read(p).ok()
            })
            .next();
        if let Some(r) = s16 {
            let want: Vec<f32> = r
                .chunks_exact(2)
                .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
                .collect();
            let rounded: Vec<f32> = got
                .iter()
                .map(|v| (v * 32768.0).round().clamp(-32768.0, 32767.0) / 32768.0)
                .collect();
            let o = compare(&rounded, &want, ch, frame);
            line += &format!(
                " reference max {:.0} LSB, RMS {:.3} LSB, SNR {:.1} dB (offset {});",
                o.max_lsb, o.rms_lsb, o.snr_db, o.offset
            );
            if let Some(b) = bound
                && (o.max_lsb > b || o.rms_lsb > MAX_RMS)
            {
                failed.push(format!(
                    "{name}: max {:.1} LSB, RMS {:.3} LSB against the reference",
                    o.max_lsb, o.rms_lsb
                ));
            }
        } else if bound.is_some() {
            failed.push(format!("{name}: no reference decode"));
        }
        if let Ok(f) = std::fs::read(path.with_extension("f32")) {
            let want: Vec<f32> = f
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            let o = compare(&got, &want, ch, frame);
            line += &format!(
                " ffmpeg max {:.3} LSB, SNR {:.1} dB (offset {})",
                o.max_lsb, o.snr_db, o.offset
            );
        }
        eprintln!("{line}");
    }
    assert!(failed.is_empty(), "{failed:#?}");
}
