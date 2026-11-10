//! The JVT / ITU-T conformance bitstreams against libavcodec's per-frame MD5s, fetched by
//! `tools/corpus/h264-conformance.sh` into the gitignored corpus (skipped when absent).
//!
//! Every stream either decodes bit-exactly, frame for frame in output order, or is refused
//! as unsupported where that is expected (interlaced, other chroma formats or bit depths).
//! `SPECULAR_H264_CONF=<substring>` (or `=<name>`) limits the run; `SPECULAR_H264_THREADS=<n>` sets the
//! decoding threads (default 4).

use std::path::PathBuf;

use specular_h264::{Decoder, Error, Options, OutputOrder};

mod md5;

/// The JVT corpus (tools/corpus/h264-conformance.sh), or the one `SPECULAR_H264_CORPUS`
/// names (such as the professional profiles of tools/corpus/h264-professional.sh).
fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_H264_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance/h264"),
        PathBuf::from,
    );
    d.join("manifest.tsv").exists().then_some(d)
}

struct Entry {
    name: String,
    file: String,
    pix_fmt: String,
}

fn entries(dir: &std::path::Path) -> Vec<Entry> {
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

fn expected(dir: &std::path::Path, name: &str) -> Vec<String> {
    let t = std::fs::read_to_string(dir.join(format!("{name}.framemd5"))).unwrap_or_default();
    t.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.rsplit(',').next().map(|h| h.trim().to_string()))
        .filter(|h| h.len() == 32)
        .collect()
}

/// Decodes a stream: frame MD5s in output order, or the error that stopped it.
/// With `rgb`, a picture whose VUI says GBR (matrix_coefficients 0) is hashed in R, G, B plane
/// order, as the JM reference decoder writes it.
pub fn decode(data: &[u8], threads: usize, rgb: bool) -> (Vec<String>, Option<Error>) {
    let mut d = Decoder::new(Options {
        threads,
        output: OutputOrder::Display,
    });
    let mut out = Vec::new();
    let mut err = None;
    let collect = |d: &mut Decoder, out: &mut Vec<String>| {
        while let Some(p) = d.next_picture(true) {
            if let Ok(path) = std::env::var("SPECULAR_H264_DUMP") {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .unwrap();
                for pl in &p.planes {
                    f.write_all(pl).unwrap();
                }
            }
            let mut h = md5::Md5::new();
            let gbr = rgb && p.signal.and_then(|s| s.colour).is_some_and(|c| c.2 == 0);
            let order = if gbr { [2, 0, 1] } else { [0, 1, 2] };
            for i in order {
                h.update(&p.planes[i]);
            }
            out.push(h.hex());
        }
    };
    for nal in specular_h264::split_annexb(data) {
        match d.decode_nal(nal, 0) {
            Ok(()) => {}
            Err(e @ Error::Unsupported(_)) => {
                err = Some(e);
                break;
            }
            Err(_) => {}
        }
        collect(&mut d, &mut out);
    }
    d.flush();
    collect(&mut d, &mut out);
    (out, err)
}

#[test]
fn conformance() {
    let Some(dir) = corpus() else {
        eprintln!("conformance corpus absent: run tools/corpus/h264-conformance.sh");
        return;
    };
    let filter = std::env::var("SPECULAR_H264_CONF").unwrap_or_default();
    let threads = std::env::var("SPECULAR_H264_THREADS")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(4);
    let (mut pass, mut refused, mut fail) = (Vec::new(), Vec::new(), Vec::new());
    for e in entries(&dir) {
        let wanted = match filter.strip_prefix('=') {
            Some(exact) => e.name == exact,
            None => e.name.contains(filter.as_str()),
        };
        if !wanted {
            continue;
        }
        let data = std::fs::read(dir.join(&e.file)).unwrap();
        let want = expected(&dir, &e.name);
        let t = std::time::Instant::now();
        let (got, err) = decode(&data, threads, e.pix_fmt == "jm");
        let ms = t.elapsed().as_millis();
        // Samples deeper than 8 bits are compared as little-endian u16; streams ffmpeg
        // cannot describe may be refused.
        let may_refuse = e.pix_fmt.is_empty() || e.pix_fmt == "unknown";
        if let Some(err) = err {
            // The Extended profile's SP/SI slices and separate colour
            // planes are out of scope.
            let out_of_scope = ["SP/SI", "separate colour planes"]
                .iter()
                .any(|k| err.to_string().contains(k));
            if may_refuse || out_of_scope {
                println!("ORACLE\tH.264\t{}\tskip", e.name);
                refused.push(format!("{} ({err})", e.name));
            } else {
                println!("ORACLE\tH.264\t{}\tfail", e.name);
                fail.push(format!("{}: refused: {err}", e.name));
            }
            continue;
        }
        println!(
            "ORACLE\tH.264\t{}\t{}",
            e.name,
            if got == want { "pass" } else { "fail" }
        );
        if got == want {
            pass.push(e.name.clone());
            eprintln!("ok   {} ({} frames, {ms} ms)", e.name, got.len());
        } else {
            let first = got.iter().zip(&want).position(|(a, b)| a != b);
            let msg = format!(
                "{}: {} frames, expected {}, first mismatch {:?}",
                e.name,
                got.len(),
                want.len(),
                first
            );
            eprintln!("FAIL {msg}");
            fail.push(msg);
        }
    }
    eprintln!(
        "conformance: {} bit-exact, {} refused as expected, {} failing",
        pass.len(),
        refused.len(),
        fail.len()
    );
    for r in &refused {
        eprintln!("  refused {r}");
    }
    for f in &fail {
        eprintln!("  FAIL {f}");
    }
    assert!(fail.is_empty(), "{} conformance streams fail", fail.len());
}
