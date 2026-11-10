//! Frame-exact against ffmpeg (`-idct int -flags +bitexact -f framemd5`): the synthetic
//! streams of `tests/data` (see `gen_fixtures.sh`), and with `SPECULAR_MPEG4_CLIPS` set, every
//! `NAME.m4v` / `NAME.h263` there with a `NAME.framemd5` beside it.

mod md5;

use std::path::{Path, PathBuf};

use specular_mpeg4::{Decoder, OutputOrder, Picture};

fn md5_of(p: &Picture) -> String {
    let mut m = md5::Md5::new();
    for plane in &p.planes {
        m.update(plane);
    }
    m.hex()
}

/// Decodes a bare stream: MPEG-4 in pieces of `piece` bytes, H.263 picture by picture.
fn decode_all(data: &[u8], piece: usize) -> Result<Vec<Picture>, specular_mpeg4::Error> {
    let mut d = Decoder::new(OutputOrder::Display);
    let mut out = Vec::new();
    let mut feed = |d: &mut Decoder, c: &[u8], i: usize| -> Result<(), specular_mpeg4::Error> {
        d.decode(c, i as i64)?;
        while let Some(p) = d.next_picture() {
            out.push(p);
        }
        Ok(())
    };
    if specular_mpeg4::is_short_header(data) {
        d.set_aligned(true);
        // Pictures start at the 22-bit start code, byte aligned in these streams.
        let mut starts: Vec<usize> = (0..data.len().saturating_sub(3))
            .filter(|&i| data[i] == 0 && data[i + 1] == 0 && data[i + 2] & 0xFC == 0x80)
            .collect();
        starts.push(data.len());
        for (i, w) in starts.windows(2).enumerate() {
            feed(&mut d, &data[w[0]..w[1]], i)?;
        }
    } else {
        let step = if piece == 0 { data.len().max(1) } else { piece };
        for (i, c) in data.chunks(step).enumerate() {
            feed(&mut d, c, i)?;
        }
    }
    d.flush();
    while let Some(p) = d.next_picture() {
        out.push(p);
    }
    Ok(out)
}

fn expected(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.rsplit(',').next().unwrap().trim().to_owned())
        .collect()
}

fn check(stream: &Path, piece: usize) -> (usize, usize, usize, Option<usize>) {
    let data = std::fs::read(stream).unwrap();
    let want = expected(&stream.with_extension("framemd5"));
    let got = decode_all(&data, piece).expect("decodes");
    let mut first = None;
    let mut bad = 0;
    for (i, (p, w)) in got.iter().zip(&want).enumerate() {
        if md5_of(p) != *w {
            bad += 1;
            first.get_or_insert(i);
        }
    }
    (bad, got.len(), want.len(), first)
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

#[test]
fn synthetic_streams_match_ffmpeg() {
    let mut failed = Vec::new();
    for name in [
        "sp.m4v",
        "mv4.m4v",
        "bvop.m4v",
        "bvop-skips.m4v",
        "qpel.m4v",
        "interlaced.m4v",
        "resync.m4v",
        "xvid-gmc.m4v",
        "xvid-gmc-zoom.m4v",
        "xvid-qpel.m4v",
        "partitioned.m4v",
        "partitioned-mv4.m4v",
        "baseline.h263",
    ] {
        let p = data_dir().join(name);
        for piece in [0, 333] {
            let (bad, got, want, first) = check(&p, piece);
            eprintln!(
                "{name} (pieces of {piece}): {bad} of {want} differ, {got} decoded, first {first:?}"
            );
            if bad > 0 || got != want {
                failed.push(name);
            }
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

#[test]
fn reversible_vlc_is_refused() {
    // reversible_vlc set in partitioned.m4v's VOL header: refused by name.
    let mut data = std::fs::read(data_dir().join("partitioned.m4v")).unwrap();
    let vol = specular_mpeg4::probe(&data).unwrap();
    assert_eq!(vol.refusal(), None);
    let at = data
        .windows(4)
        .position(|w| w[..3] == [0, 0, 1] && (0x20..0x30).contains(&w[3]))
        .unwrap();
    let mut flipped = false;
    for bit in 0..200 {
        let mut d = data.clone();
        d[at + 4 + bit / 8] ^= 0x80 >> (bit % 8);
        if specular_mpeg4::probe(&d)
            .and_then(|v| v.refusal().map(str::to_owned))
            .is_some_and(|r| r.contains("RVLC"))
        {
            data = d;
            flipped = true;
            break;
        }
    }
    assert!(flipped, "no bit sets reversible_vlc");
    let mut d = Decoder::new(OutputOrder::Display);
    // Decoding it must not panic.
    let _ = d.decode(&data, 0).map(|()| d.flush());
}

#[test]
fn clips_match_ffmpeg() {
    let Some(dir) = std::env::var_os("SPECULAR_MPEG4_CLIPS") else {
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
        if !["m4v", "h263"].contains(&ext) || !p.with_extension("framemd5").exists() {
            continue;
        }
        let t = std::time::Instant::now();
        if let Err(e) = decode_all(&std::fs::read(&p).unwrap(), 0) {
            eprintln!("{}: refused: {e}", p.display());
            continue;
        }
        let (bad, got, want, first) = check(&p, 0);
        eprintln!(
            "{}: {bad} of {want} frames differ (first {first:?}), {got} decoded, {:.1} fps",
            p.display(),
            got as f64 / t.elapsed().as_secs_f64()
        );
        total.0 += bad + want.abs_diff(got);
        total.1 += want;
    }
    eprintln!("total: {} of {} frames differ", total.0, total.1);
    assert_eq!(total.0, 0);
}
