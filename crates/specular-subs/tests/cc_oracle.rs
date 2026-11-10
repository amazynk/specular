//! Closed captions against ffmpeg's caption decoder (cc_dec): movies made by
//! `tools/corpus/cc-oracle.py` (CC1 in H.264 SEI in TS, Matroska and MP4; in MPEG-2 user
//! data in TS and a programme stream; a MOV c608 track), each with ffmpeg's reading as
//! `<movie>.srt`. The same in movies with B-frames (ffmpeg re-encoding the first ones, each
//! picture's captions in its own SEI or user data, so in decode order in the stream).
//!
//! Every caption ffmpeg shows must show here with the same text while ffmpeg shows it.
//! Pop-on captions must also start within a frame of ffmpeg's start (ffmpeg starts roll-up
//! and paint-on rows when the screen was last cleared; here they show as they are written).
//! Not for the MOV track: ffmpeg's Scenarist import puts a whole line of pairs in one sample
//! and its decoder applies them at once; here they come one a frame as line 21 carries
//! them, so a pop-on caption shows a few frames later (counted, not failed).
//!
//! `SPECULAR_CC_CORPUS=<dir> cargo test -p specular-subs --test cc_oracle -- --ignored`

use std::path::PathBuf;

const FRAME_US: i64 = 33_367;

fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_CC_CORPUS").map(PathBuf::from)?;
    d.is_dir().then_some(d)
}

/// (start, end, text) of an SRT file, ffmpeg's markup taken out.
fn srt(text: &str) -> Vec<(i64, i64, String)> {
    let ts = |s: &str| -> i64 {
        let s = s.trim();
        let (hms, ms) = s.split_once(',').unwrap();
        let p: Vec<i64> = hms.split(':').map(|x| x.parse().unwrap()).collect();
        ((p[0] * 60 + p[1]) * 60 + p[2]) * 1_000_000 + ms.parse::<i64>().unwrap() * 1000
    };
    let mut out = Vec::new();
    for block in text.replace('\r', "").split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(i) = lines.iter().position(|l| l.contains("-->")) else {
            continue;
        };
        let (a, b) = lines[i].split_once("-->").unwrap();
        let mut body = lines[i + 1..].join("\n");
        for tag in ["</font>", "{\\an7}", "<i>", "</i>", "<u>", "</u>"] {
            body = body.replace(tag, "");
        }
        while let Some(s) = body.find("<font") {
            let e = body[s..].find('>').map_or(body.len(), |e| s + e + 1);
            body.replace_range(s..e, "");
        }
        let body = body.replace("\\h", " ");
        let body: Vec<&str> = body
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        out.push((ts(a), ts(b), body.join("\n")));
    }
    out
}

/// A screen being gathered: start, end, (row position, text) of each row.
type Rows = (i64, i64, Vec<(f32, String)>);

/// The screens shown: (start, end, rows top to bottom).
fn screens(t: &specular_subs::Track) -> Vec<(i64, i64, String)> {
    let mut out: Vec<Rows> = Vec::new();
    for c in &t.cues {
        let Some(x) = c.text() else { continue };
        let row = (x.pos.map_or(0.0, |p| p.1), x.plain_text().trim().to_owned());
        match out.last_mut() {
            Some(s) if s.0 == c.start_us && s.1 == c.end_us => s.2.push(row),
            _ => out.push((c.start_us, c.end_us, vec![row])),
        }
    }
    out.into_iter()
        .map(|(a, b, mut rows)| {
            rows.sort_by(|x, y| x.0.total_cmp(&y.0));
            let text: Vec<String> = rows.into_iter().map(|r| r.1).collect();
            (a, b, text.join("\n"))
        })
        .collect()
}

#[test]
#[ignore = "needs the corpus made by tools/corpus/cc-oracle.py"]
fn captions_match_ffmpeg() {
    let Some(dir) = corpus() else {
        eprintln!("SPECULAR_CC_CORPUS not set");
        return;
    };
    let mut report = Vec::new();
    let mut failed = 0;
    let popon: Vec<(i64, i64)> = std::fs::read_to_string(dir.join("popon.txt"))
        .unwrap()
        .lines()
        .filter_map(|l| {
            let (a, b) = l.split_once(' ')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect();
    assert_eq!(popon.len(), 3);
    for movie in [
        "cc_h264.ts",
        "cc_h264.mkv",
        "cc_h264.mp4",
        "cc_mpeg2.ts",
        "cc_mpeg2.mpg",
        "cc608.mov",
        "cc_h264_bf.ts",
        "cc_h264_bf.mkv",
        "cc_h264_bf.mp4",
        "cc_mpeg2_bf.ts",
    ] {
        let bytes = std::fs::read(dir.join(movie)).unwrap();
        let oracle = srt(&std::fs::read_to_string(dir.join(format!("{movie}.srt"))).unwrap());
        let x = specular_video::extras::read(&bytes);
        let Some(s) = x
            .subtitles
            .iter()
            .find(|s| s.info.codec == specular_subs::CEA_608 && s.private == [1])
        else {
            report.push(format!("{movie}: no CC1"));
            failed += 1;
            continue;
        };
        let t = specular_subs::decode_stream(
            &s.info.codec,
            &s.private,
            s.packets
                .iter()
                .map(|p| (p.start_us, p.dur_us, p.data.as_slice())),
        )
        .unwrap();
        let ours = screens(&t);
        let (mut text_miss, mut start_miss) = (0, 0);
        for (a, b, want) in &oracle {
            // The same text showing during ffmpeg's cue (times from the picture's start).
            let hit = ours.iter().find(|(s, e, txt)| {
                txt == want && *s - x.picture_start_us < *b && *e - x.picture_start_us > *a
            });
            match hit {
                None => {
                    text_miss += 1;
                    eprintln!("{movie}: {a}..{b} {want:?} not shown");
                }
                Some((s, _, _)) => {
                    let pop_on = !want.starts_with("ROLL") && want != "PAINTED";
                    if pop_on && (s - x.picture_start_us - a).abs() > FRAME_US {
                        start_miss += 1;
                        eprintln!("{movie}: {want:?} at {s}, ffmpeg {a}");
                    }
                }
            }
        }
        failed += text_miss;
        if movie != "cc608.mov" {
            failed += start_miss;
        }
        // Against the source: cues in order, each pop-on caption from the frame its End of
        // Caption is on to the frame its erase is on.
        let order_miss = t
            .cues
            .windows(2)
            .filter(|w| w[1].start_us < w[0].start_us)
            .count();
        let frame = |f: i64| f * 1_001_000 / 30;
        let mut source_miss = 0;
        for (a, b) in &popon {
            let near = |t: i64, f: i64| (t - x.picture_start_us - frame(f)).abs() <= FRAME_US / 2;
            if !ours.iter().any(|(s, e, _)| near(*s, *a) && near(*e, *b)) {
                source_miss += 1;
                eprintln!("{movie}: pop-on at frames {a}..{b} not found");
            }
        }
        if movie != "cc608.mov" {
            failed += order_miss + source_miss;
        }
        report.push(format!(
            "{movie}: {} ffmpeg captions, {} screens here; {text_miss} text mismatches, \
             {start_miss} pop-on start mismatches; against the source {order_miss} out of \
             order, {source_miss} pop-on times off",
            oracle.len(),
            ours.len()
        ));
    }
    eprintln!("{}", report.join("\n"));
    assert_eq!(failed, 0, "{}", report.join("\n"));
}
