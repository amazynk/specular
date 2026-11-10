//! Frames and sound against FFmpeg's decode of the DV corpus (`tools/corpus/fetch-dv.sh`).
//!
//! Run: `SPECULAR_DV_CORPUS=<dir> cargo test -p specular-dv --test oracle -- --ignored
//! --nocapture`. Without a corpus the test passes and says so.
//!
//! Pictures: the inverse DCT is the floating-point transform of SMPTE 314M 5.2.1 and FFmpeg
//! uses an integer one, so samples may differ by a step or two; a file passes when no sample
//! is more than `TOL` away and fewer than `MAX_OFF` of them differ at all. Frames with damaged
//! blocks (tape dropouts) are counted, not compared: concealment is the decoder's choice.
//! Sound: bit exact.

use std::path::{Path, PathBuf};

const TOL: u8 = 2;
const MAX_OFF: f64 = 0.20;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_DV_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| Path::new(&t).join("corpus/dv"))
        })?;
    dir.is_dir().then_some(dir)
}

struct Stats {
    frames: usize,
    samples: u64,
    off: u64,
    max: u8,
    damaged: u32,
    pos: u64,
    neg: u64,
    skipped: usize,
    decode: std::time::Duration,
}

fn compare(dv: &[u8], yuv: &[u8]) -> Result<Stats, String> {
    let info = specular_dv::probe(dv).map_err(|e| e.to_string())?;
    let n = dv.len() / info.frame_bytes;
    let mut st = Stats {
        frames: 0,
        samples: 0,
        off: 0,
        max: 0,
        damaged: 0,
        pos: 0,
        neg: 0,
        skipped: 0,
        decode: std::time::Duration::ZERO,
    };
    let mut at = 0;
    for f in 0..n {
        let frame = &dv[f * info.frame_bytes..(f + 1) * info.frame_bytes];
        let t = std::time::Instant::now();
        let p = specular_dv::decode(frame).map_err(|e| format!("frame {f}: {e}"))?;
        st.decode += t.elapsed();
        let size: usize = p.planes.iter().map(Vec::len).sum();
        if at + size > yuv.len() {
            break;
        }
        if p.damaged_blocks > 0 {
            st.damaged += p.damaged_blocks;
            st.skipped += 1;
            at += size;
            continue;
        }
        for plane in &p.planes {
            let r = &yuv[at..at + plane.len()];
            for (a, b) in plane.iter().zip(r) {
                let d = a.abs_diff(*b);
                st.off += u64::from(d != 0);
                st.pos += u64::from(a > b);
                st.neg += u64::from(a < b);
                st.max = st.max.max(d);
            }
            st.samples += plane.len() as u64;
            at += plane.len();
        }
        st.frames += 1;
    }
    Ok(st)
}

/// Sound against FFmpeg's, frame by frame: (samples compared, samples that differ, frames
/// without sound packs, which are skipped: FFmpeg conceals them, we give no sound).
fn sound(dv: &[u8], pcm: &[u8]) -> Result<(usize, usize, usize), String> {
    let info = specular_dv::probe(dv).map_err(|e| e.to_string())?;
    let theirs: Vec<i16> = pcm
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    let (mut n, mut diff, mut skipped, mut off, mut last) = (0, 0, 0, 0, 0);
    for f in 0..dv.len() / info.frame_bytes {
        let frame = &dv[f * info.frame_bytes..(f + 1) * info.frame_bytes];
        match specular_dv::audio(frame) {
            Some(a) => {
                last = a.samples.len();
                let end = (off + last).min(theirs.len());
                let r = &theirs[off.min(end)..end];
                diff += a.samples.iter().zip(r).filter(|(x, y)| x != y).count();
                n += r.len();
            }
            None if f > 0 => skipped += 1,
            None => return Err("frame 0: no sound".into()),
        }
        off += last;
    }
    Ok((n, diff, skipped))
}

#[test]
#[ignore = "needs the DV corpus and FFmpeg's references (tools/corpus/fetch-dv.sh)"]
fn corpus_matches_ffmpeg() {
    let Some(dir) = corpus() else {
        println!("no DV corpus: skipped");
        return;
    };
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "dv"))
        .collect();
    names.sort();
    let mut failed = Vec::new();
    for path in names {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let dv = std::fs::read(&path).unwrap();
        let Ok(yuv) = std::fs::read(path.with_extension("yuv")) else {
            println!("ORACLE\tDV\t{name}\tskip");
            continue;
        };
        let ok = match compare(&dv, &yuv) {
            Ok(s) => {
                let frac = s.off as f64 / s.samples.max(1) as f64;
                println!(
                    "{name}: {} frames, {} of {} samples differ ({:.3}%), max {}, {} damaged blocks in {} frames not compared, {:.1} ms/frame decoding (+{} -{})",
                    s.frames,
                    s.off,
                    s.samples,
                    frac * 100.0,
                    s.max,
                    s.damaged,
                    s.skipped,
                    s.decode.as_secs_f64() * 1e3 / (s.frames + s.skipped).max(1) as f64,
                    s.pos,
                    s.neg
                );
                s.frames > 0 && s.max <= TOL && frac < MAX_OFF
            }
            Err(e) => {
                println!("{name}: {e}");
                false
            }
        };
        let sound_ok = match std::fs::read(path.with_extension("pcm")) {
            Ok(pcm) => match sound(&dv, &pcm) {
                Ok((n, d, sk)) => {
                    println!("{name}: sound {d} of {n} samples differ, {sk} frames without sound");
                    d == 0 && n > 0
                }
                Err(e) => {
                    println!("{name}: sound: {e}");
                    false
                }
            },
            Err(_) => true,
        };
        let ok = ok && sound_ok;
        println!("ORACLE\tDV\t{name}\t{}", if ok { "pass" } else { "fail" });
        if !ok {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "differ from FFmpeg: {failed:?}");
}

#[test]
#[ignore = "needs the DV corpus (tools/corpus/fetch-dv.sh)"]
fn corpus_facts() {
    let Some(dir) = corpus() else {
        println!("no DV corpus: skipped");
        return;
    };
    use specular_dv::Chroma::*;
    // name, size, chroma, profile, 16:9
    let want = [
        ("gen-ntsc411.dv", (720, 480), C411, "DVCPRO", false), // FFmpeg writes APT 1 here
        ("gen-pal420.dv", (720, 576), C420, "DV", false),
        ("gen-pal420-wide.dv", (720, 576), C420, "DV", true),
        ("gen-pal411.dv", (720, 576), C411, "DVCPRO", false),
        ("gen-ntsc422.dv", (720, 480), C422, "DVCPRO50", false),
        ("gen-pal422.dv", (720, 576), C422, "DVCPRO50", false),
        ("DVCPRO.dv", (720, 576), C411, "DVCPRO", false),
        ("dvcpro50.dv", (720, 576), C422, "DVCPRO50", false),
        ("pond.dv", (720, 480), C411, "DV", false),
        ("small_test2.dv", (720, 576), C420, "DV", false),
    ];
    for (name, size, chroma, profile, wide) in want {
        let Ok(dv) = std::fs::read(dir.join(name)) else {
            println!("{name}: missing");
            continue;
        };
        let i = specular_dv::probe(&dv).unwrap();
        println!("{name}: {i:?}");
        assert_eq!((i.width, i.height), size, "{name}");
        assert_eq!(i.chroma, chroma, "{name}");
        assert_eq!(i.profile, profile, "{name}");
        assert_eq!(i.wide, wide, "{name}");
        assert!(i.bottom_field_first, "{name}");
    }
}
