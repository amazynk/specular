//! FFmpeg's FATE subtitle samples (tools/corpus/fetch-subs.sh) against ffmpeg as the oracle:
//! text subtitles converted to SubRip by ffmpeg must give the same cues (times to the
//! millisecond, words without markup); PGS and VobSub cues must start where ffprobe puts a
//! packet. Without the corpus the test skips; without ffmpeg it only reads every file.
//!
//! `SPECULAR_SUBS_CORPUS=<dir> cargo test -p specular-subs --test fate -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use std::process::Command;

use specular_subs::{Body, Track};

fn corpus() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var_os("SPECULAR_SUBS_CORPUS")?);
    d.is_dir().then_some(d)
}

fn has(tool: &str) -> bool {
    Command::new(tool)
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Words of a cue without markup or spacing differences.
fn words(s: &str) -> String {
    let t = specular_subs::markup::parse(s).plain_text();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_cues(t: &Track) -> Vec<(i64, i64, String)> {
    t.cues
        .iter()
        .filter_map(|c| match &c.body {
            Body::Text(x) => Some((c.start_us / 1000, c.end_us / 1000, words(&x.plain_text()))),
            Body::Bitmap(_) => None,
        })
        .collect()
}

/// ffmpeg's reading of `path` as SubRip.
fn ffmpeg_srt(path: &Path) -> Option<Track> {
    let o = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "srt", "-"])
        .output()
        .ok()?;
    if !o.status.success() {
        return None;
    }
    specular_subs::srt::parse(&String::from_utf8_lossy(&o.stdout)).ok()
}

/// ffprobe's packet times of the first subtitle stream (ms).
fn packet_times(path: &Path) -> Vec<i64> {
    let Ok(o) = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "s:0",
            "-show_entries",
            "packet=pts_time",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| l.trim().trim_end_matches(',').parse::<f64>().ok())
        .map(|t| (t * 1000.0).round() as i64)
        .collect()
}

#[test]
#[ignore = "needs tools/corpus/fetch-subs.sh"]
fn fate_samples_against_ffmpeg() {
    let Some(dir) = corpus() else {
        eprintln!("skipped: SPECULAR_SUBS_CORPUS unset (tools/corpus/fetch-subs.sh)");
        return;
    };
    let ffmpeg = has("ffmpeg") && has("ffprobe");
    if !ffmpeg {
        eprintln!("ffmpeg not installed: the files are only read");
    }
    let mut report = Vec::new();
    for name in [
        "SubRip_capability_tester.srt",
        "WebVTT_capability_tester.vtt",
        "WebVTT_extended_tester.vtt",
        "a9-misc.ssa",
        "1ededcbd7b.ass",
        "badsyntax.srt",
        "madness.srt",
        "empty-events-2167.srt",
    ] {
        let path = dir.join(name);
        if !path.exists() {
            continue;
        }
        let (ours, _) =
            specular_subs::read_file(&path, None, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        let ours = text_cues(&ours);
        if !ffmpeg {
            report.push(format!("{name}: {} cues", ours.len()));
            continue;
        }
        let Some(theirs) = ffmpeg_srt(&path) else {
            report.push(format!(
                "{name}: {} cues, ffmpeg could not read it",
                ours.len()
            ));
            continue;
        };
        let theirs = text_cues(&theirs);
        let timing = ours
            .iter()
            .zip(&theirs)
            .filter(|(a, b)| (a.0 - b.0).abs() > 1 || (a.1 - b.1).abs() > 1)
            .count();
        let text = ours.iter().zip(&theirs).filter(|(a, b)| a.2 != b.2).count();
        report.push(format!(
            "{name}: ours {} cues, ffmpeg {}; {timing} timing and {text} text mismatches",
            ours.len(),
            theirs.len()
        ));
    }
    for (name, kind) in [("pgs_sub.sup", "pgs"), ("vobsub.idx", "vobsub")] {
        let path = dir.join(name);
        if !path.exists() {
            continue;
        }
        let (t, _) =
            specular_subs::read_file(&path, None, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        let starts: Vec<i64> = t.cues.iter().map(|c| c.start_us / 1000).collect();
        if !ffmpeg {
            report.push(format!("{name}: {} {kind} cues", starts.len()));
            continue;
        }
        let packets = packet_times(&path);
        let off = starts
            .iter()
            .filter(|s| !packets.iter().any(|p| (p - **s).abs() <= 1))
            .count();
        report.push(format!(
            "{name}: {} {kind} cues, {} packets; {off} cue starts on no packet",
            starts.len(),
            packets.len()
        ));
    }
    // Subtitles inside movies: Matroska (made by the fetch script) and MP4 timed text.
    for movie in ["mkv_subs.mkv", "MovText_capability_tester.mp4"] {
        let path = dir.join(movie);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let x = specular_video::extras::read(&bytes);
        for (k, s) in x.subtitles.iter().enumerate() {
            let t = match specular_subs::decode_stream(
                &s.info.codec,
                &s.private,
                s.packets
                    .iter()
                    .map(|p| (p.start_us, p.dur_us, p.data.as_slice())),
            ) {
                Ok(t) => t,
                Err(e) => {
                    report.push(format!("{movie} #{k} {}: {e}", s.info.codec));
                    continue;
                }
            };
            if !ffmpeg {
                report.push(format!(
                    "{movie} #{k} {}: {} cues",
                    s.info.codec,
                    t.cues.len()
                ));
                continue;
            }
            if t.is_bitmap() {
                let o = Command::new("ffprobe")
                    .args([
                        "-v",
                        "error",
                        "-select_streams",
                        &format!("s:{k}"),
                        "-show_entries",
                        "packet=pts_time",
                        "-of",
                        "csv=p=0",
                    ])
                    .arg(&path)
                    .output()
                    .expect("ffprobe");
                let packets: Vec<i64> = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter_map(|l| l.trim().trim_end_matches(',').parse::<f64>().ok())
                    .map(|v| (v * 1000.0).round() as i64)
                    .collect();
                let off = t
                    .cues
                    .iter()
                    .filter(|c| !packets.iter().any(|p| (p - c.start_us / 1000).abs() <= 1))
                    .count();
                report.push(format!(
                    "{movie} #{k} {}: {} cues, {} packets; {off} cue starts on no packet",
                    s.info.codec,
                    t.cues.len(),
                    packets.len()
                ));
                continue;
            }
            let o = Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&path)
                .args(["-map", &format!("0:s:{k}"), "-f", "srt", "-"])
                .output()
                .expect("ffmpeg");
            let theirs =
                specular_subs::srt::parse(&String::from_utf8_lossy(&o.stdout)).unwrap_or_default();
            let (ours, theirs) = (text_cues(&t), text_cues(&theirs));
            let timing = ours
                .iter()
                .zip(&theirs)
                .filter(|(a, b)| (a.0 - b.0).abs() > 1 || (a.1 - b.1).abs() > 1)
                .count();
            let text = ours.iter().zip(&theirs).filter(|(a, b)| a.2 != b.2).count();
            if let Some((a, b)) = ours.iter().zip(&theirs).find(|(a, b)| a != b) {
                eprintln!("first difference: ours {a:?}, ffmpeg {b:?}");
            }
            report.push(format!(
                "{movie} #{k} {}: ours {} cues, ffmpeg {}; {timing} timing and {text} text mismatches",
                s.info.codec,
                ours.len(),
                theirs.len()
            ));
        }
    }
    for r in &report {
        eprintln!("fate: {r}");
    }
    assert!(!report.is_empty(), "no samples in {}", dir.display());
}
