//! Sorenson Spark (FLV1) against ffmpeg (`-idct int -flags +bitexact -f framemd5`):
//! clips made with ffmpeg's own FLV1 encoder and FATE's FLV samples
//! (`tools/corpus/fetch-vp3-spark.sh` writes each file's video tags as NAME.pkts beside the
//! oracle's NAME.framemd5). Skips without the corpus (`SPECULAR_VP3_SPARK_CORPUS`, default
//! $CARGO_TARGET_DIR/corpus/vp3-spark).

mod md5;

use std::path::{Path, PathBuf};

use specular_mpeg4::{Decoder, OutputOrder, Picture};

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_VP3_SPARK_CORPUS").map_or_else(
        || {
            option_env!("CARGO_TARGET_DIR")
                .map_or_else(
                    || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target"),
                    PathBuf::from,
                )
                .join("corpus/vp3-spark")
        },
        PathBuf::from,
    )
}

fn md5_of(p: &Picture) -> String {
    let mut m = md5::Md5::new();
    for plane in &p.planes {
        m.update(plane);
    }
    m.hex()
}

fn packets(path: &Path) -> Vec<Vec<u8>> {
    let d = std::fs::read(path).unwrap();
    let mut out = Vec::new();
    let mut p = 0;
    while p + 4 <= d.len() {
        let n = u32::from_le_bytes(d[p..p + 4].try_into().unwrap()) as usize;
        out.push(d[p + 4..p + 4 + n].to_vec());
        p += 4 + n;
    }
    out
}

/// ffmpeg's frame MD5s in output order.
fn expected(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.rsplit(',').next().unwrap().trim().to_owned())
        .collect()
}

fn decode(pk: &[Vec<u8>], order: OutputOrder) -> Vec<Picture> {
    let mut d = Decoder::new(order);
    d.set_aligned(true);
    d.set_sorenson(true);
    let mut out = Vec::new();
    for (i, p) in pk.iter().enumerate() {
        if let Err(e) = d.decode(p, i as i64) {
            eprintln!("  packet {i}: {e}");
        }
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

/// The disposable frames ffmpeg's decoder drops: those before a second reference frame.
fn ffmpeg_skips(pk: &[Vec<u8>]) -> Vec<usize> {
    let mut refs = 0;
    let mut out = Vec::new();
    for (i, p) in pk.iter().enumerate() {
        let bits: String = p.iter().take(12).map(|b| format!("{b:08b}")).collect();
        let q = 33
            + match &bits[30..33] {
                "000" => 16,
                "001" => 32,
                _ => 0,
            };
        match &bits[q..q + 2] {
            "10" if refs < 2 => out.push(i),
            "00" | "01" => refs += 1,
            _ => {}
        }
    }
    out
}

#[test]
#[ignore = "needs tools/corpus/fetch-vp3-spark.sh"]
fn sorenson_spark_matches_ffmpeg() {
    let dir = corpus().join("flv1");
    let Ok(list) = std::fs::read_dir(&dir) else {
        eprintln!("skipped: run tools/corpus/fetch-vp3-spark.sh");
        return;
    };
    let mut names: Vec<PathBuf> = list
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "flv"))
        .collect();
    names.sort();
    let mut failed = 0;
    for path in &names {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let pk = path.with_file_name(format!("{name}.pkts"));
        let want = expected(&path.with_file_name(format!("{name}.framemd5")));
        if !pk.exists() || want.is_empty() {
            println!("ORACLE\tFLV1\t{name}\tskip");
            continue;
        }
        let pk = packets(&pk);
        let vol = specular_mpeg4::probe_sorenson(&pk[0]);
        let mut bad = 0;
        for order in [OutputOrder::Decode, OutputOrder::Display] {
            let t0 = std::time::Instant::now();
            let mut got = decode(&pk, order);
            let secs = t0.elapsed().as_secs_f64();
            // ffmpeg outputs nothing for disposable frames until it holds two references
            // (its B-frame rule): such frames refer to the key frame and decode here.
            let skipped = ffmpeg_skips(&pk);
            if got.len() == pk.len() {
                for &i in skipped.iter().rev() {
                    got.remove(i);
                }
            }
            let diff = got
                .iter()
                .zip(&want)
                .filter(|(g, w)| md5_of(g) != **w)
                .count();
            bad += diff + got.len().abs_diff(want.len());
            if order == OutputOrder::Decode {
                eprintln!(
                    "{name}: {} ({}): {diff} of {} frames differ, {} compared ({} disposable frames ffmpeg drops), {:.0} fps",
                    vol.as_ref()
                        .and_then(|v| v.profile_name())
                        .unwrap_or_default(),
                    vol.as_ref()
                        .map(|v| format!("{}x{}", v.width, v.height))
                        .unwrap_or_default(),
                    want.len(),
                    got.len(),
                    skipped.len(),
                    pk.len() as f64 / secs
                );
            }
        }
        println!(
            "ORACLE\tFLV1\t{name}\t{}",
            if bad == 0 { "pass" } else { "fail" }
        );
        failed += usize::from(bad > 0);
    }
    assert_eq!(failed, 0, "{failed} files differ");
}

/// Picture headers: sizes by code and custom, the version, refusals; garbage never panics.
#[test]
fn sorenson_headers() {
    // Start code, version 1, TR 0, size 101 (320x240), type 00, deblock 0, quant 4, no extra.
    let mut bits = String::from("00000000000000001");
    bits += "00001";
    bits += "00000000";
    bits += "101";
    bits += "00";
    bits += "0";
    bits += "00100";
    bits += "0";
    bits += &"0".repeat(40);
    let bytes: Vec<u8> = bits
        .as_bytes()
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| a << 1 | (b - b'0')) << (8 - c.len()))
        .collect();
    let v = specular_mpeg4::probe_sorenson(&bytes).unwrap();
    assert_eq!((v.width, v.height, v.sorenson), (320, 240, Some(1)));
    assert!(specular_mpeg4::is_sorenson_keyframe(&bytes));
    assert_eq!(
        v.profile_name().as_deref(),
        Some("Sorenson Spark, version 1")
    );
    let mut v2 = bytes.clone();
    v2[2] |= 0x20; // version 3
    assert!(specular_mpeg4::sorenson_refusal(&v2).is_some());
    let mut d = Decoder::new(OutputOrder::Decode);
    d.set_aligned(true);
    d.set_sorenson(true);
    let mut seed = 99u32;
    for n in 0..300usize {
        let mut f = bytes.clone();
        f.resize(8 + n % 200, 0);
        for b in f.iter_mut().skip(5) {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *b = seed as u8;
        }
        let _ = d.decode(&f, n as i64);
        while d.next_picture().is_some() {}
    }
}
