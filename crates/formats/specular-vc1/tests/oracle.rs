//! Frame-exact comparison with FFmpeg's VC-1 decoder over the corpus fetched by
//! `tools/corpus/fetch-vc1.sh` (FATE's copies of the SMPTE conformance streams: raw
//! Advanced-profile `.vc1` and Simple/Main `.rcv`). FFmpeg is the oracle only: it decodes the
//! same file to raw 4:2:0 frames, which are compared sample by sample.
//!
//! Run: SPECULAR_VC1_CORPUS=<corpus>/vc1/fate cargo test -p specular-vc1 --test oracle -- --ignored --nocapture
//! Prints one `ORACLE<TAB>VC-1<TAB>file<TAB>pass|fail|skip` line per file (skip: refused in
//! words, e.g. interlaced pictures).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use specular_vc1::{Config, Decoder, OutputOrder, Picture};

/// An elementary stream split into coded frames.
pub struct Stream {
    pub struct_c: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Vec<u8>>,
}

/// Annex L: the RCV serialization of Simple/Main profile streams.
pub fn read_rcv(d: &[u8]) -> Option<Stream> {
    if d.len() < 36 || d[3] != 0xC5 {
        return None;
    }
    let u32le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    // STRUCT_C is 4 bytes; some encoders write a 5-byte extension (one padding byte).
    let n = u32le(4) as usize;
    if !(4..=8).contains(&n) || d.len() < 32 + n {
        return None;
    }
    let struct_c = d[8..12].to_vec();
    let height = u32le(8 + n);
    let width = u32le(12 + n);
    if u32le(16 + n) != 12 {
        return None;
    }
    let mut pos = 32 + n;
    let mut frames = Vec::new();
    while pos + 8 <= d.len() {
        let size = (u32le(pos) & 0xff_ffff) as usize;
        pos += 8;
        if pos + size > d.len() {
            break;
        }
        frames.push(d[pos..pos + size].to_vec());
        pos += size;
    }
    Some(Stream {
        struct_c: Some(struct_c),
        width,
        height,
        frames,
    })
}

/// An Advanced profile byte stream cut into access units at frame start codes; sequence and
/// entry-point headers go with the frame after them.
pub fn read_ebdu(d: &[u8]) -> Stream {
    let mut cuts = Vec::new();
    let mut pending: Option<usize> = None;
    let mut i = 0;
    while i + 3 < d.len() {
        if d[i] == 0 && d[i + 1] == 0 && d[i + 2] == 1 {
            let t = d[i + 3];
            match t {
                0x0F | 0x0E => {
                    if pending.is_none() {
                        pending = Some(i);
                    }
                }
                0x0D => {
                    cuts.push(pending.take().unwrap_or(i));
                }
                _ => {}
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut frames = Vec::new();
    for k in 0..cuts.len() {
        let e = cuts.get(k + 1).copied().unwrap_or(d.len());
        frames.push(d[cuts[k]..e].to_vec());
    }
    Stream {
        struct_c: None,
        width: 0,
        height: 0,
        frames,
    }
}

pub fn decode_all(s: &Stream) -> Result<Vec<Picture>, String> {
    let config = match &s.struct_c {
        Some(c) => Config::SimpleMain {
            struct_c: c,
            width: s.width,
            height: s.height,
        },
        None => Config::Advanced { headers: &[] },
    };
    let mut dec = Decoder::new(&config, OutputOrder::Decode).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (i, f) in s.frames.iter().enumerate() {
        if let Err(e) = dec.decode(f, i as i64) {
            match e {
                specular_vc1::Error::Malformed(m) => eprintln!("  frame {i}: {m}"),
                e => return Err(e.to_string()),
            }
        }
        while let Some(p) = dec.next_picture() {
            out.push(p);
        }
    }
    dec.flush();
    while let Some(p) = dec.next_picture() {
        out.push(p);
    }
    Ok(out)
}

fn ffmpeg_frames(path: &Path) -> Option<Vec<u8>> {
    let o = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-threads", "1", "-i"])
        .arg(path)
        .args([
            "-fps_mode",
            "passthrough",
            "-pix_fmt",
            "yuv420p",
            "-f",
            "rawvideo",
            "-",
        ])
        .output()
        .ok()?;
    o.status.success().then_some(o.stdout)
}

fn corpus() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var_os("SPECULAR_VC1_CORPUS")?);
    d.is_dir().then_some(d)
}

/// Compares one stream; returns (frames, mismatching frames, fps).
fn compare(path: &Path) -> Result<(usize, usize, usize, f64), String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let s = match path.extension().and_then(|e| e.to_str()) {
        Some("rcv") => read_rcv(&data).ok_or("not an RCV file")?,
        _ => read_ebdu(&data),
    };
    let t = Instant::now();
    let pics = decode_all(&s)?;
    let secs = t.elapsed().as_secs_f64();
    let reference = ffmpeg_frames(path).ok_or("ffmpeg failed")?;
    let Some(p0) = pics.first() else {
        return Err("no pictures".into());
    };
    let (w, h) = (p0.width as usize, p0.height as usize);
    let fsize = w * h + 2 * w.div_ceil(2) * h.div_ceil(2);
    let nref = reference.len() / fsize;
    let mut bad = 0;
    for (i, p) in pics.iter().enumerate().take(nref) {
        let r = &reference[i * fsize..(i + 1) * fsize];
        let ours: Vec<u8> = p.planes.concat();
        if ours != r {
            if bad < 3 {
                let k = ours.iter().zip(r).position(|(a, b)| a != b).unwrap_or(0);
                let (pl, off) = if k < w * h {
                    (0, k)
                } else {
                    let c = w.div_ceil(2) * h.div_ceil(2);
                    (1 + (k - w * h) / c, (k - w * h) % c)
                };
                let pw = if pl == 0 { w } else { w.div_ceil(2) };
                let n = ours.iter().zip(r).filter(|(a, b)| a != b).count();
                eprintln!(
                    "  frame {i}: {n} samples differ, first plane {pl} at ({}, {}): {} vs {}",
                    off % pw,
                    off / pw,
                    ours[k],
                    r[k]
                );
            }
            bad += 1;
        }
    }
    Ok((pics.len(), nref, bad, pics.len() as f64 / secs))
}

#[test]
#[ignore = "needs the VC-1 corpus and ffmpeg"]
fn corpus_matches_ffmpeg() {
    let Some(dir) = corpus() else {
        eprintln!("SPECULAR_VC1_CORPUS not set: skipped");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("rcv" | "vc1")))
        .collect();
    files.sort();
    if let Ok(only) = std::env::var("SPECULAR_VC1_ONLY") {
        files.retain(|p| p.to_string_lossy().contains(&only));
    }
    let mut failed = 0;
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        match compare(f) {
            Ok((ours, nref, bad, fps)) => {
                let ok = bad == 0 && ours == nref;
                println!("ORACLE\tVC-1\t{name}\t{}", if ok { "pass" } else { "fail" });
                eprintln!("{name}: {ours} pictures (ffmpeg {nref}), {bad} differ, {fps:.0} fps");
                if !ok {
                    failed += 1;
                }
            }
            Err(e) if e.starts_with("VC-1") || e.contains("unsupported") => {
                println!("ORACLE\tVC-1\t{name}\tskip");
                eprintln!("{name}: refused: {e}");
            }
            Err(e) => {
                println!("ORACLE\tVC-1\t{name}\tfail");
                eprintln!("{name}: {e}");
                failed += 1;
            }
        }
    }
    assert_eq!(failed, 0, "{failed} files differ from ffmpeg");
}
