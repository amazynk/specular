//! Frame-exact against ffmpeg (`-idct int -f framemd5`): the synthetic streams of
//! `tests/data` (see `gen_fixtures.sh`), and with `SPECULAR_MPEG2_CLIPS` set, every
//! `NAME.m2v` / `NAME.m1v` / `NAME.mpv` there with a `NAME.framemd5` beside it.

mod md5;

use std::path::{Path, PathBuf};

use specular_mpeg2::{Decoder, OutputOrder, Picture};

fn md5_of(p: &Picture) -> String {
    let mut m = md5::Md5::new();
    for plane in &p.planes {
        m.update(plane);
    }
    m.hex()
}

/// Decodes `data` fed in pieces of `piece` bytes (0: whole), in display order.
pub fn decode_all(data: &[u8], piece: usize, threads: usize) -> Vec<Picture> {
    let mut d = Decoder::new(threads, OutputOrder::Display);
    let mut out = Vec::new();
    let step = if piece == 0 { data.len().max(1) } else { piece };
    for (i, c) in data.chunks(step).enumerate() {
        d.decode(c, i as i64).expect("decodes");
        while let Some(p) = d.next_picture() {
            out.push(p);
        }
    }
    d.flush();
    while let Some(p) = d.next_picture() {
        out.push(p);
    }
    out
}

fn expected(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.rsplit(',').next().unwrap().trim().to_owned())
        .collect()
}

/// Mismatching frames of one stream (and the frame counts).
fn check(stream: &Path, piece: usize, threads: usize) -> (usize, usize, usize) {
    let data = std::fs::read(stream).unwrap();
    let want = expected(&stream.with_extension("framemd5"));
    let got = decode_all(&data, piece, threads);
    let bad = got
        .iter()
        .zip(&want)
        .filter(|(p, w)| md5_of(p) != **w)
        .count();
    (bad, got.len(), want.len())
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

#[test]
fn synthetic_streams_match_ffmpeg() {
    let mut n = 0;
    for name in [
        "ibp.m2v",
        "interlaced.m2v",
        "p422.m2v",
        "odd.m2v",
        "mpeg1.m1v",
    ] {
        let p = data_dir().join(name);
        for (piece, threads) in [(0, 4), (997, 1), (64, 3)] {
            let (bad, got, want) = check(&p, piece, threads);
            assert_eq!(
                (bad, got),
                (0, want),
                "{name} in pieces of {piece}: {bad} of {want} frames differ ({got} decoded)"
            );
        }
        n += 1;
    }
    assert_eq!(n, 5);
}

/// MPEG-1 D-pictures (DC coefficients only) against libmpeg2 (libavcodec refuses them;
/// `tests/data/gen_dpictures.py`).
#[test]
fn d_pictures_match_libmpeg2() {
    let p = data_dir().join("dpict.m1v");
    for (piece, threads) in [(0, 4), (37, 1), (5, 2)] {
        let (bad, got, want) = check(&p, piece, threads);
        assert_eq!((bad, got, want), (0, 3, 3), "in pieces of {piece}");
    }
}

#[test]
fn clips_match_ffmpeg() {
    let Some(dir) = std::env::var_os("SPECULAR_MPEG2_CLIPS") else {
        return;
    };
    let mut total = (0, 0);
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["m2v", "m1v", "mpv"].contains(&ext) || !p.with_extension("framemd5").exists() {
            continue;
        }
        let t = std::time::Instant::now();
        let (bad, got, want) = check(&p, 0, 0);
        let el = t.elapsed().as_secs_f64();
        eprintln!(
            "{}: {bad} of {want} frames differ, {got} decoded, {:.1} fps",
            p.display(),
            got as f64 / el
        );
        total.0 += bad + want.abs_diff(got);
        total.1 += want;
    }
    eprintln!("total: {} of {} frames differ", total.0, total.1);
    assert_eq!(total.0, 0);
}
