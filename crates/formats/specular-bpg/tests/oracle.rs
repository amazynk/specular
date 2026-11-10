//! Every file of the BPG corpus (tools/corpus/fetch-editors-bpg.sh, or $SPECULAR_BPG_CORPUS)
//! against libbpg's frames (refs/<name>.fNNN.pam and refs/<name>.txt). Skips when the corpus
//! is absent. Prints, per file, the share of exactly equal samples and the largest
//! difference (in 8-bit or 16-bit codes), and fails on any difference over the tolerance.

use specular_bpg as bpg;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_BPG_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/editors/bpg")
        });
    dir.join("refs").is_dir().then_some(dir)
}

/// An RGBA PAM: width, height, maxval, samples.
fn read_pam(p: &Path) -> Option<(u32, u32, u32, Vec<u16>)> {
    let d = std::fs::read(p).ok()?;
    let end = d.windows(7).position(|w| w == b"ENDHDR\n")? + 7;
    let head = std::str::from_utf8(&d[..end]).ok()?;
    let field = |k: &str| -> Option<u32> {
        head.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|v| v.trim().parse().ok())
    };
    let (w, h, max) = (field("WIDTH ")?, field("HEIGHT ")?, field("MAXVAL ")?);
    let body = &d[end..];
    let s = if max > 255 {
        body.chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect()
    } else {
        body.iter().map(|&b| u16::from(b)).collect()
    };
    Some((w, h, max, s))
}

/// Our image as RGBA samples.
fn rgba(img: &bpg::Image) -> Vec<u16> {
    let d = &img.data;
    match img.format {
        bpg::PixelFormat::Rgba8 => d.iter().map(|&b| u16::from(b)).collect(),
        bpg::PixelFormat::Gray8 => d
            .iter()
            .flat_map(|&g| [g as u16, g as u16, g as u16, 255])
            .collect(),
        bpg::PixelFormat::Rgba16 => d
            .chunks_exact(2)
            .map(|c| u16::from_ne_bytes([c[0], c[1]]))
            .collect(),
        bpg::PixelFormat::Gray16 => d
            .chunks_exact(2)
            .map(|c| u16::from_ne_bytes([c[0], c[1]]))
            .flat_map(|g| [g, g, g, 65535])
            .collect(),
    }
}

#[test]
fn bpg_corpus_matches_libbpg() {
    let Some(dir) = corpus() else {
        eprintln!("BPG corpus absent; run tools/corpus/fetch-editors-bpg.sh");
        return;
    };
    let mut files: Vec<PathBuf> = ["src", "gen"]
        .iter()
        .filter_map(|s| std::fs::read_dir(dir.join(s)).ok())
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "bpg"))
        .collect();
    files.sort();
    let (mut exact, mut close, mut failed, mut refused) = (0, 0, Vec::new(), Vec::new());
    let mut frames_total = 0;
    for f in &files {
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        let durations = std::fs::read_to_string(dir.join("refs").join(format!("{name}.txt")))
            .unwrap_or_default();
        let durations: Vec<(u32, u32)> = durations
            .lines()
            .filter_map(|l| {
                let v: Vec<u32> = l.split(' ').filter_map(|x| x.parse().ok()).collect();
                (v.len() == 3).then(|| (v[1], v[2]))
            })
            .collect();
        if durations.is_empty() {
            eprintln!("{name}: libbpg has no reference, skipped");
            continue;
        }
        let data: Arc<[u8]> = std::fs::read(f).unwrap().into();
        let mut frames = match bpg::Frames::new(data.clone(), 1) {
            Ok(fr) => fr,
            Err(bpg::Error::Unsupported(m)) => {
                // A still may still decode where frames cannot (above 12 bits).
                match bpg::decode(&data, &bpg::Options::default()) {
                    Ok(_) if durations.len() > 1 => {
                        refused.push(format!("{name}: {m} (first frame decodes)"));
                        continue;
                    }
                    Ok(_) => {
                        refused.push(format!("{name}: {m}"));
                        continue;
                    }
                    Err(e) => {
                        refused.push(format!("{name}: {e}"));
                        continue;
                    }
                }
            }
            Err(e) => {
                failed.push(format!("{name}: {e}"));
                continue;
            }
        };
        if frames.frame_count() as usize != durations.len() {
            failed.push(format!(
                "{name}: {} frames, libbpg {}",
                frames.frame_count(),
                durations.len()
            ));
        }
        let (mut same, mut total, mut max_diff) = (0u64, 0u64, 0u32);
        let mut ok = true;
        for (i, &(num, den)) in durations.iter().enumerate() {
            let Some((rw, rh, rmax, want)) =
                read_pam(&dir.join("refs").join(format!("{name}.f{i:03}.pam")))
            else {
                break;
            };
            let Some(r) = frames.next_frame(None) else {
                ok = false;
                break;
            };
            let (img, dur) = match r {
                Ok(v) => v,
                Err(e) => {
                    failed.push(format!("{name} frame {i}: {e}"));
                    ok = false;
                    break;
                }
            };
            assert_eq!((img.width, img.height), (rw, rh), "{name}");
            let got = rgba(&img);
            assert_eq!(got.len(), want.len(), "{name}");
            let wide = matches!(
                img.format,
                bpg::PixelFormat::Rgba16 | bpg::PixelFormat::Gray16
            );
            assert_eq!(wide, rmax > 255, "{name}: output depth");
            for (a, b) in got.iter().zip(&want) {
                let d = u32::from(a.abs_diff(*b));
                same += u64::from(d == 0);
                max_diff = max_diff.max(d);
            }
            total += got.len() as u64;
            if num != 0 {
                let want_d = f64::from(num) / f64::from(den);
                assert!(
                    (dur.as_secs_f64() - want_d).abs() < 1e-6,
                    "{name} frame {i}: duration {dur:?}, libbpg {want_d}"
                );
            }
            frames_total += 1;
        }
        let pct = 100.0 * same as f64 / total.max(1) as f64;
        eprintln!(
            "{name}: {} frame(s), {pct:.3} % exact, max diff {max_diff}",
            durations.len()
        );
        if !ok {
            continue;
        }
        if max_diff == 0 {
            exact += 1;
        } else if max_diff <= 1 {
            close += 1;
        } else {
            failed.push(format!("{name}: max diff {max_diff} ({pct:.3} % exact)"));
        }
    }
    eprintln!(
        "BPG: {} files, {exact} exact, {close} within 1, {} failed, {} refused; {frames_total} frames compared",
        files.len(),
        failed.len(),
        refused.len()
    );
    for r in &refused {
        eprintln!("refused: {r}");
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Wall-clock timings of a 3 MP still (1 and 4 threads) and the longest corpus animation.
#[test]
fn bpg_timings() {
    let Some(dir) = corpus() else { return };
    let Ok(big) = std::fs::read(dir.join("gen/big.bpg")) else {
        return;
    };
    for threads in [1, 4] {
        let t = std::time::Instant::now();
        let img = bpg::decode(
            &big,
            &bpg::Options {
                threads,
                cancel: None,
            },
        )
        .unwrap();
        eprintln!(
            "timing: {}x{} still, {threads} thread(s): {:.1} ms",
            img.width,
            img.height,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
    let Ok(anim) = std::fs::read(dir.join("src/animated.bpg")) else {
        return;
    };
    let t = std::time::Instant::now();
    let mut f = bpg::Frames::new(anim.into(), 1).unwrap();
    let mut n = 0;
    while let Some(r) = f.next_frame(None) {
        r.unwrap();
        n += 1;
    }
    eprintln!(
        "timing: animated.bpg 494x371, {n} frames: {:.1} ms ({:.2} ms/frame)",
        t.elapsed().as_secs_f64() * 1e3,
        t.elapsed().as_secs_f64() * 1e3 / f64::from(n)
    );
}
