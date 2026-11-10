//! The JCT-VC HEVC conformance bitstreams against libavcodec's per-frame MD5s, fetched by
//! `tools/corpus/hevc-conformance.sh` into the gitignored corpus (skipped when absent).
//!
//! Every stream either decodes bit-exactly, frame for frame in output order, or is refused
//! with [`Error::Unsupported`] where that is expected (range extensions beyond what the
//! decoder implements, other layers). `SPECULAR_HEVC_CONF=<substring>` (or `=<name>`) limits
//! the run; `SPECULAR_HEVC_THREADS=<n>` sets the decoding threads (default 4);
//! `SPECULAR_HEVC_VERBOSE` lists each output's access unit and `SPECULAR_HEVC_DUMP=<file>`
//! writes the decoded frames as raw video.

use std::path::{Path, PathBuf};

use hevc::{Error, Frame, VideoDecoder};

mod md5;

/// The corpus: `SPECULAR_HEVC_CORPUS` when set (a worktree without its own), else the
/// checkout's.
fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_HEVC_CORPUS").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../tools/corpus/conformance/hevc")
        },
        PathBuf::from,
    );
    d.join("manifest.tsv").exists().then_some(d)
}

struct Entry {
    name: String,
    file: String,
    pix_fmt: String,
}

fn entries(dir: &Path) -> Vec<Entry> {
    let m = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    m.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 6).then(|| Entry {
                name: f[0].into(),
                file: f[1].into(),
                pix_fmt: f[4].into(),
            })
        })
        .collect()
}

/// The oracle's frame MD5s, or `None` when libavcodec could not decode the stream.
fn expected(dir: &Path, name: &str) -> Option<Vec<String>> {
    let t = std::fs::read_to_string(dir.join(format!("{name}.framemd5"))).ok()?;
    Some(
        t.lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.rsplit(',').next().map(|h| h.trim().to_string()))
            .filter(|h| h.len() == 32)
            .collect(),
    )
}

/// The NAL units of an Annex B byte stream, without start codes (trailing zero bytes, which
/// belong to the next start code, are dropped).
fn split_annexb(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map_or(data.len(), |&n| n - 3);
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&data[s..e]);
        }
    }
    out
}

/// Groups NAL units into access units (7.4.2.4.4): a new one starts at the first slice
/// segment of a picture, or at a parameter set, access unit delimiter, prefix SEI or
/// reserved prefix type that follows a picture's slices.
fn access_units<'a>(nals: &[&'a [u8]]) -> Vec<Vec<&'a [u8]>> {
    let mut aus: Vec<Vec<&[u8]>> = Vec::new();
    let mut cur: Vec<&[u8]> = Vec::new();
    let mut seen_vcl = false;
    for &n in nals {
        if n.len() < 2 {
            continue;
        }
        let t = (n[0] >> 1) & 0x3f;
        let layer = ((n[0] & 1) << 5) | (n[1] >> 3);
        let starts = if t < 32 {
            layer == 0 && n.len() > 2 && n[2] & 0x80 != 0
        } else {
            matches!(t, 32..=35 | 39 | 41..=44 | 48..=55)
        };
        if starts && seen_vcl {
            aus.push(std::mem::take(&mut cur));
            seen_vcl = false;
        }
        if t < 32 && layer == 0 {
            seen_vcl = true;
        }
        cur.push(n);
    }
    if !cur.is_empty() {
        aus.push(cur);
    }
    aus
}

/// The MD5 of a frame as `ffmpeg -f framemd5` hashes it (native pix_fmt: 8-bit samples as
/// bytes, deeper ones as little-endian 16-bit words, planes in order); `all` hashes the same
/// bytes for the whole stream (what the JCT-VC packages' `.md5` files give for the `.yuv`).
fn frame_md5(f: &Frame, all: &mut md5::Md5, dump: &mut Option<std::fs::File>) -> String {
    let mut h = md5::Md5::new();
    let mut plane = |p: &[u16], stride: u32, w: u32, rows: u32, wide: bool| {
        let mut buf = Vec::with_capacity(w as usize * if wide { 2 } else { 1 });
        for r in 0..rows as usize {
            let row = &p[r * stride as usize..][..w as usize];
            buf.clear();
            if wide {
                buf.extend(row.iter().flat_map(|s| s.to_le_bytes()));
            } else {
                buf.extend(row.iter().map(|&s| s as u8));
            }
            h.update(&buf);
            all.update(&buf);
            if let Some(d) = dump {
                std::io::Write::write_all(d, &buf).unwrap();
            }
        }
    };
    plane(&f.y, f.y_stride, f.width, f.height, f.bit_depth > 8);
    let (cw, ch) = f.chroma.chroma_size(f.width, f.height);
    if cw > 0 {
        let wide = f.bit_depth_chroma > 8;
        plane(&f.cb, f.c_stride, cw, ch, wide);
        plane(&f.cr, f.c_stride, cw, ch, wide);
    }
    h.hex()
}

/// What decoding a stream gave.
struct Decode {
    /// Frame MD5s in output order.
    frames: Vec<String>,
    /// The access unit (its index in the stream) each output frame came from.
    tags: Vec<i64>,
    /// The access units that gave no output frame, as `next_dropped` reported them.
    dropped: Vec<i64>,
    /// The number of access units given to the decoder.
    fed: usize,
    /// The MD5 of all frames together.
    all: String,
    /// The refusal that stopped the decode.
    err: Option<Error>,
}

/// Decodes a stream from access unit `start` on (a seek when not 0, given the parameter
/// sets seen before it the way a container's would be), each access unit tagged with its
/// index.
fn decode(data: &[u8], threads: usize, start: usize) -> Decode {
    let aus = access_units(&split_annexb(data));
    let sets: Vec<&[u8]> = aus[..start.min(aus.len())]
        .iter()
        .flatten()
        .copied()
        .filter(|n| matches!((n[0] >> 1) & 0x3f, 32..=34))
        .collect();
    let mut d = VideoDecoder::new(&sets, threads).unwrap();
    let (mut frames, mut tags, mut dropped) = (Vec::new(), Vec::new(), Vec::new());
    let mut err = None;
    let mut all = md5::Md5::new();
    // SPECULAR_HEVC_DUMP=<file>: the decoded frames as raw video, to compare with ffmpeg's.
    let mut dump =
        std::env::var_os("SPECULAR_HEVC_DUMP").map(|p| std::fs::File::create(p).unwrap());
    let mut take = |d: &mut VideoDecoder, frames: &mut Vec<String>, tags: &mut Vec<i64>| {
        while let Some(p) = d.next_output() {
            frames.push(frame_md5(&p.frame, &mut all, &mut dump));
            tags.push(p.tag);
        }
        while let Some(t) = d.next_dropped() {
            dropped.push(t);
        }
    };
    let mut fed = 0;
    for (i, au) in aus.iter().enumerate().skip(start) {
        fed += 1;
        match d.decode_tagged(au, i as i64, None) {
            Ok(()) => {}
            Err(e @ Error::Unsupported(_)) => {
                err = Some(e);
                break;
            }
            Err(e) => eprintln!("  access unit {i}: {e}"),
        }
        take(&mut d, &mut frames, &mut tags);
    }
    d.flush();
    take(&mut d, &mut frames, &mut tags);
    Decode {
        frames,
        tags,
        dropped,
        fed,
        all: all.hex(),
        err,
    }
}

/// Whether every access unit fed came back exactly once, as an output frame's tag or as a
/// dropped one: what a player relies on to pace frames by composition time.
fn tags_accounted(r: &Decode, start: usize) -> bool {
    let mut seen: Vec<i64> = r.tags.iter().chain(&r.dropped).copied().collect();
    seen.sort_unstable();
    seen == (start as i64..(start + r.fed) as i64).collect::<Vec<_>>()
}

#[test]
fn conformance() {
    let Some(dir) = corpus() else {
        eprintln!("conformance corpus absent: run tools/corpus/hevc-conformance.sh");
        return;
    };
    let filter = std::env::var("SPECULAR_HEVC_CONF").unwrap_or_default();
    let threads = std::env::var("SPECULAR_HEVC_THREADS")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(4);
    let (mut pass, mut refused, mut no_oracle, mut fail) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut by_reference = Vec::new();
    // One line per stream for tools/ci/oracle-pass-rates.sh (refusals allowed by profile and
    // streams without an oracle are skips).
    let line = |name: &str, status: &str| println!("ORACLE\tHEVC\t{name}\t{status}");
    for e in entries(&dir) {
        let wanted = match filter.strip_prefix('=') {
            Some(exact) => e.name == exact,
            None => e.name.contains(filter.as_str()),
        };
        if !wanted {
            continue;
        }
        let data = std::fs::read(dir.join(&e.file)).unwrap();
        // The JCT-VC package's MD5 of the whole decoded .yuv: the oracle where libavcodec
        // differs from the reference decoder or cannot decode the stream.
        let reference = std::fs::read_to_string(dir.join(format!("{}.yuv.md5", e.name)))
            .ok()
            .and_then(|t| t.split_whitespace().next().map(str::to_ascii_lowercase))
            .filter(|h| h.len() == 32);
        let want = expected(&dir, &e.name);
        if want.is_none() && reference.is_none() {
            no_oracle.push(e.name.clone());
            line(&e.name, "skip");
            continue;
        }
        let want = want.unwrap_or_default();
        let t = std::time::Instant::now();
        let r = decode(&data, threads, 0);
        let ms = t.elapsed().as_millis();
        // Range extension and multi-layer streams may be refused; Main and Main 10 may not.
        let may_refuse = !matches!(e.pix_fmt.as_str(), "yuv420p" | "yuv420p10le")
            || e.name.contains("RExt")
            || e.name.starts_with("MVHEVC");
        if let Some(err) = &r.err {
            if may_refuse {
                refused.push(format!("{} ({err})", e.name));
                line(&e.name, "skip");
            } else {
                eprintln!("FAIL {}: refused: {err}", e.name);
                fail.push(format!("{}: refused: {err}", e.name));
                line(&e.name, "fail");
            }
            continue;
        }
        let (got, all) = (&r.frames, &r.all);
        if r.fed > 0 && !tags_accounted(&r, 0) {
            let msg = format!(
                "{}: access unit tags not accounted for ({} out, {} dropped, {} fed)",
                e.name,
                r.tags.len(),
                r.dropped.len(),
                r.fed
            );
            eprintln!("FAIL {msg}");
            fail.push(msg);
            line(&e.name, "fail");
        } else if *got == want {
            pass.push(e.name.clone());
            line(&e.name, "pass");
            eprintln!("ok   {} ({} frames, {ms} ms)", e.name, got.len());
        } else if reference.as_deref() == Some(all.as_str()) {
            pass.push(e.name.clone());
            by_reference.push(e.name.clone());
            line(&e.name, "pass");
            eprintln!(
                "ok   {} ({} frames, {ms} ms; the JCT-VC MD5, libavcodec differs or fails)",
                e.name,
                got.len()
            );
        } else {
            if std::env::var_os("SPECULAR_HEVC_VERBOSE").is_some() {
                eprintln!("  stream MD5 {all}, JCT-VC {reference:?}");
            }
            let first = got.iter().zip(&want).position(|(a, b)| a != b);
            let msg = format!(
                "{}: {} frames, expected {}, first mismatch {:?}",
                e.name,
                got.len(),
                want.len(),
                first
            );
            eprintln!("FAIL {msg}");
            if std::env::var_os("SPECULAR_HEVC_VERBOSE").is_some() {
                for (i, g) in got.iter().enumerate() {
                    let at = want.iter().position(|w| w == g);
                    eprintln!(
                        "  out {i}: {g} (access unit {}) expected at {at:?}",
                        r.tags[i]
                    );
                }
            }
            fail.push(msg);
            line(&e.name, "fail");
        }
    }
    eprintln!(
        "conformance: {} bit-exact ({} by the JCT-VC MD5), {} refused as expected, \
         {} without an oracle, {} failing",
        pass.len(),
        by_reference.len(),
        refused.len(),
        no_oracle.len(),
        fail.len()
    );
    for r in &refused {
        eprintln!("  refused {r}");
    }
    for n in &no_oracle {
        eprintln!("  no oracle {n}");
    }
    for f in &fail {
        eprintln!("  FAIL {f}");
    }
    assert!(fail.is_empty(), "{} conformance streams fail", fail.len());
}

/// The NAL unit type of an access unit's first slice segment.
fn vcl_type(au: &[&[u8]]) -> Option<u8> {
    au.iter().map(|n| (n[0] >> 1) & 0x3f).find(|&t| t < 32)
}

/// A seek to a CRA picture that is not the first in the stream: its RASL pictures refer to
/// pictures before it, which were never decoded, so they are skipped (8.1.3) and reported as
/// dropped, while everything else matches the oracle.
#[test]
fn seek_to_cra_skips_rasl() {
    let Some(dir) = corpus() else {
        eprintln!("conformance corpus absent: run tools/corpus/hevc-conformance.sh");
        return;
    };
    let mut checked = 0;
    for name in ["RAP_A_docomo_4", "RAP_B_Bossen_1"] {
        let Some(want) = expected(&dir, name) else {
            continue;
        };
        let Ok(data) = std::fs::read(dir.join(format!("{name}.bit"))) else {
            continue;
        };
        let aus = access_units(&split_annexb(&data));
        let types: Vec<Option<u8>> = aus.iter().map(|a| vcl_type(a)).collect();
        // The first CRA after the start that has RASL pictures.
        let Some(start) = (1..types.len()).find(|&i| {
            types[i] == Some(21)
                && types[i + 1..]
                    .iter()
                    .take_while(|t| !matches!(t, Some(16..=21)))
                    .any(|t| matches!(t, Some(8 | 9)))
        }) else {
            continue;
        };
        let rasl: Vec<i64> = (start..types.len())
            .filter(|&i| matches!(types[i], Some(8 | 9)))
            .take_while(|&i| {
                !types[start + 1..i]
                    .iter()
                    .any(|t| matches!(t, Some(16..=21)))
            })
            .map(|i| i as i64)
            .collect();
        let r = decode(&data, 4, start);
        assert!(r.err.is_none(), "{name}: {:?}", r.err);
        assert!(tags_accounted(&r, start), "{name}: tags not accounted for");
        assert!(
            !r.frames.is_empty(),
            "{name}: nothing decoded after the seek"
        );
        for t in &rasl {
            assert!(
                !r.tags.contains(t),
                "{name}: RASL access unit {t} output after a seek"
            );
            assert!(
                r.dropped.contains(t),
                "{name}: RASL access unit {t} not reported"
            );
        }
        for (f, t) in r.frames.iter().zip(&r.tags) {
            assert!(
                want.contains(f),
                "{name}: access unit {t} differs after the seek"
            );
        }
        eprintln!(
            "{name}: seek to access unit {start}: {} frames, {} RASL skipped",
            r.frames.len(),
            rasl.len()
        );
        checked += 1;
    }
    eprintln!("seek_to_cra_skips_rasl: {checked} streams");
}

/// A small xorshift generator for the stress test.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Frame threading under stress: a few streams (inter prediction, temporal motion vector
/// prediction, wavefronts, tiles, deblocking across slices, random access) decoded over and
/// over at random thread counts, intact and damaged (truncated, bits flipped), must give
/// exactly what one thread gives, every time, and the oracle where intact.
/// `SPECULAR_HEVC_STRESS=<rounds>` (default 3) and `SPECULAR_HEVC_STRESS_SEED=<n>` steer it.
#[test]
fn frame_threads_stress() {
    let Some(dir) = corpus() else {
        eprintln!("conformance corpus absent: run tools/corpus/hevc-conformance.sh");
        return;
    };
    let rounds: usize = std::env::var("SPECULAR_HEVC_STRESS")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(3);
    let seed: u64 = std::env::var("SPECULAR_HEVC_STRESS_SEED")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        })
        | 1;
    eprintln!("frame_threads_stress: seed {seed}, {rounds} rounds");
    let mut rng = Rng(seed);
    let names = [
        "TMVP_A_MS_3",
        "WPP_D_ericsson_MAIN_2",
        "MERGE_F_MTK_4",
        "DBLK_E_VIXS_1",
        "PMERGE_A_TI_3",
        "TILES_A_Cisco_2",
        "RAP_B_Bossen_1",
    ];
    let mut checked = 0;
    for name in names {
        let Ok(data) = std::fs::read(dir.join(format!("{name}.bit"))) else {
            continue;
        };
        let base = decode(&data, 1, 0);
        if let Some(want) = expected(&dir, name) {
            assert!(
                base.frames == want,
                "{name}: one thread differs from the oracle"
            );
        }
        for round in 0..rounds {
            let threads = 2 + rng.below(7) as usize;
            let r = decode(&data, threads, 0);
            assert!(
                r.frames == base.frames && r.tags == base.tags && r.dropped == base.dropped,
                "{name}: {threads} threads differ from one (round {round}, seed {seed})"
            );
            // Damaged: cut short somewhere, a few bits past the parameter sets flipped.
            let mut bad = data.clone();
            bad.truncate(data.len() / 4 + rng.below(data.len() as u64 * 3 / 4) as usize);
            for _ in 0..1 + rng.below(6) {
                let i = 200 + rng.below(bad.len().saturating_sub(200).max(1) as u64) as usize;
                if let Some(b) = bad.get_mut(i) {
                    *b ^= 1 << rng.below(8);
                }
            }
            let one = decode(&bad, 1, 0);
            let many = decode(&bad, threads, 0);
            assert!(
                many.frames == one.frames && many.tags == one.tags && many.dropped == one.dropped,
                "{name}: damaged, {threads} threads differ from one (round {round}, seed {seed})"
            );
            assert!(
                tags_accounted(&many, 0),
                "{name}: damaged, tags not accounted for"
            );
        }
        checked += 1;
    }
    eprintln!("frame_threads_stress: {checked} streams");
}

/// Still pictures decoded in parallel (tiles, wavefront rows) are exactly what one thread
/// gives, intact and damaged: the intra pictures of streams with tiles or wavefronts,
/// decoded on their own at several thread counts, with a few bits of their slice data
/// flipped or the last slice cut short.
#[test]
fn still_threads_match() {
    let Some(dir) = corpus() else {
        eprintln!("conformance corpus absent: run tools/corpus/hevc-conformance.sh");
        return;
    };
    let mut rng = Rng(0x5eed_1234_5678_9abd);
    let names = [
        "TILES_A_Cisco_2",
        "TILES_B_Cisco_1",
        "WPP_B_ericsson_MAIN_2",
        "WPP_D_ericsson_MAIN_2",
        "WPP_F_ericsson_MAIN10_2",
        "ENTP_A_Qualcomm_1",
        "ENTP_C_LG_3",
        "DSLICE_A_HHI_5",
    ];
    let still = |sets: &[&[u8]], slices: &[&[u8]], threads: usize| {
        let opts = hevc::Options {
            threads,
            cancel: None,
        };
        hevc::decode_still_with(sets, slices, &opts).map(|d| {
            let mut all = md5::Md5::new();
            (frame_md5(&d.frame, &mut all, &mut None), d.damaged)
        })
    };
    let mut checked = 0;
    for name in names {
        let Ok(data) = std::fs::read(dir.join(format!("{name}.bit"))) else {
            continue;
        };
        let aus = access_units(&split_annexb(&data));
        let mut sets: Vec<&[u8]> = Vec::new();
        for au in aus.iter().take(12) {
            sets.extend(
                au.iter()
                    .filter(|n| matches!((n[0] >> 1) & 0x3f, 32..=34))
                    .copied(),
            );
            let slices: Vec<&[u8]> = au
                .iter()
                .filter(|n| matches!((n[0] >> 1) & 0x3f, 16..=21))
                .copied()
                .collect();
            if slices.is_empty() {
                continue;
            }
            let base = still(&sets, &slices, 1);
            for threads in [2, 4, 8] {
                assert!(
                    still(&sets, &slices, threads) == base,
                    "{name}: {threads} threads differ from one"
                );
            }
            for round in 0..4 {
                let mut bad: Vec<Vec<u8>> = slices.iter().map(|s| s.to_vec()).collect();
                if round == 3 {
                    let last = bad.last_mut().unwrap();
                    last.truncate(last.len() / 2);
                }
                for _ in 0..1 + rng.below(4) {
                    let s = &mut bad[rng.below(slices.len() as u64) as usize];
                    let i = 12 + rng.below(s.len().saturating_sub(12).max(1) as u64) as usize;
                    if let Some(b) = s.get_mut(i) {
                        *b ^= 1 << rng.below(8);
                    }
                }
                let bad: Vec<&[u8]> = bad.iter().map(|s| &s[..]).collect();
                let one = still(&sets, &bad, 1);
                for threads in [2, 4, 8] {
                    assert!(
                        still(&sets, &bad, threads) == one,
                        "{name}: damaged, {threads} threads differ from one (round {round})"
                    );
                }
            }
            checked += 1;
        }
    }
    eprintln!("still_threads_match: {checked} pictures");
}

/// Cancelling in the middle of a frame-threaded decode stops it promptly; decoding then
/// goes on (with a fresh flag), every access unit is still accounted for once, and every
/// frame output is exactly what an uncancelled decode gives for it: pictures cancelled, and
/// pictures predicted from them, are reported dropped, never output half-made. A decoder
/// dropped with pictures in flight does not hang.
#[test]
fn cancel_mid_stream() {
    let Some(dir) = corpus() else {
        eprintln!("conformance corpus absent: run tools/corpus/hevc-conformance.sh");
        return;
    };
    for name in ["WPP_D_ericsson_MAIN_2", "RAP_B_Bossen_1", "TILES_A_Cisco_2"] {
        let Ok(data) = std::fs::read(dir.join(format!("{name}.bit"))) else {
            continue;
        };
        let base = decode(&data, 1, 0);
        let want: std::collections::HashMap<i64, &String> =
            base.tags.iter().copied().zip(&base.frames).collect();
        let aus = access_units(&split_annexb(&data));
        for threads in [1, 2, 4, 8] {
            let flag = std::sync::atomic::AtomicBool::new(false);
            let mut d = VideoDecoder::new(&[], threads).unwrap();
            let (mut tags, mut dropped) = (Vec::new(), Vec::new());
            let mut all = md5::Md5::new();
            let mut take = |d: &mut VideoDecoder, tags: &mut Vec<i64>, dropped: &mut Vec<i64>| {
                while let Some(p) = d.next_output() {
                    let f = frame_md5(&p.frame, &mut all, &mut None);
                    assert!(
                        want.get(&p.tag) == Some(&&f),
                        "{name}, {threads} threads: access unit {} differs after the cancel",
                        p.tag
                    );
                    tags.push(p.tag);
                }
                while let Some(t) = d.next_dropped() {
                    dropped.push(t);
                }
            };
            let mut cancelled = None;
            for (i, au) in aus.iter().enumerate() {
                if i == aus.len() / 2 {
                    flag.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                let f = if cancelled.is_some() {
                    None
                } else {
                    Some(&flag)
                };
                match d.decode_tagged(au, i as i64, f) {
                    Ok(()) => {}
                    Err(Error::Cancelled) if cancelled.is_none() => cancelled = Some(i),
                    Err(e) => panic!("{name}, {threads} threads: access unit {i}: {e}"),
                }
                take(&mut d, &mut tags, &mut dropped);
            }
            d.flush();
            take(&mut d, &mut tags, &mut dropped);
            let Some(at) = cancelled else {
                panic!("{name}, {threads} threads: not cancelled");
            };
            let mut seen: Vec<i64> = tags.iter().chain(&dropped).copied().collect();
            seen.sort_unstable();
            assert!(
                seen == (0..aus.len() as i64).collect::<Vec<_>>(),
                "{name}, {threads} threads: tags not accounted for"
            );
            assert!(
                dropped.contains(&(at as i64)),
                "{name}, {threads} threads: the cancelled access unit not reported"
            );
            eprintln!(
                "{name}, {threads} threads: cancelled at {at}, {} frames, {} dropped",
                tags.len(),
                dropped.len()
            );
        }
        // Dropping a decoder with pictures in flight returns.
        for threads in [2, 8] {
            let mut d = VideoDecoder::new(&[], threads).unwrap();
            for (i, au) in aus.iter().enumerate().take(aus.len() / 2) {
                let _ = d.decode_tagged(au, i as i64, None);
            }
            drop(d);
        }
    }
}

/// Decoding speed of an Annex-B stream, one thread against frame threads:
/// `SPECULAR_HEVC_BENCH=<file> cargo test --release --test conformance bench_fps -- --ignored
/// --nocapture`, `SPECULAR_HEVC_THREADS=<n>` for the threaded run (default: one per core).
/// Also the time to the first frame alone (what a seek waits for: the first access unit,
/// then a flush), where threads can only help inside the picture.
#[test]
#[ignore = "a benchmark: needs SPECULAR_HEVC_BENCH"]
fn bench_fps() {
    let Some(path) = std::env::var_os("SPECULAR_HEVC_BENCH") else {
        eprintln!("bench_fps: set SPECULAR_HEVC_BENCH to an Annex-B HEVC stream");
        return;
    };
    let data = std::fs::read(&path).unwrap();
    let aus = access_units(&split_annexb(&data));
    let threads: usize = std::env::var("SPECULAR_HEVC_THREADS")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(0);
    let run = |threads: usize| {
        let mut first: Vec<f64> = (0..7)
            .map(|_| {
                let t = std::time::Instant::now();
                let mut d = VideoDecoder::new(&[], threads).unwrap();
                d.decode(&aus[0], None).unwrap();
                d.flush();
                assert!(d.next_output().is_some());
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        first.sort_by(f64::total_cmp);
        let t = std::time::Instant::now();
        let mut d = VideoDecoder::new(&[], threads).unwrap();
        let mut n = 0usize;
        let mut size = (0, 0);
        for au in &aus {
            d.decode(au, None).unwrap();
            while let Some(p) = d.next_output() {
                size = (p.frame.width, p.frame.height);
                n += 1;
            }
        }
        d.flush();
        while let Some(p) = d.next_output() {
            size = (p.frame.width, p.frame.height);
            n += 1;
        }
        let s = t.elapsed().as_secs_f64();
        eprintln!(
            "bench_fps: {}x{}, {n} frames, threads {threads}: {:.2} s, {:.1} fps; first frame {:.1} ms",
            size.0,
            size.1,
            s,
            n as f64 / s,
            first[first.len() / 2]
        );
    };
    run(1);
    run(threads);
}
