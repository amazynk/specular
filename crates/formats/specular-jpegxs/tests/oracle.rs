//! The JPEG XS corpus against the ISO/IEC 21122-5 reference decoder (run
//! tools/corpus/fetch-jpegxs.sh, then
//! `SPECULAR_JPEGXS_CORPUS=tools/corpus/jpegxs cargo test -p specular-jpegxs --test oracle -- --nocapture`).
//!
//! `synth/` (the reference encoder across profiles and options) and `conformance/` (ISO/IEC
//! 21122-4 codestreams) decode file by file; `sequences/` (TDC video frames) through one
//! [`Sequence`] per sequence, in order. Each directory's `manifest.tsv` gives per file the
//! expected outcome and the sample count and SHA-256 of the reference decoder's 16-bit
//! little-endian samples; a `NAME.ref` beside a file (fetch with `--keep-ref`) holds those
//! samples, to count the ones that differ.

use sha2::{Digest, Sha256};
use specular_jpegxs::{Error, Samples, Sequence, codestream};
use std::path::Path;
use std::time::Instant;

struct Entry {
    file: String,
    ok: bool,
    samples: usize,
    sha: String,
}

fn manifest(dir: &Path) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.tsv")) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            Entry {
                file: c[0].to_owned(),
                ok: c[1] == "ok",
                samples: c[2].parse().unwrap_or(0),
                sha: c[3].to_owned(),
            }
        })
        .collect()
}

#[derive(Default)]
struct Tally {
    same: usize,
    differ: usize,
    refused: usize,
    errors: Vec<String>,
    samples: usize,
    ms: f64,
}

impl Tally {
    /// [`Self::verdict`] plus one `ORACLE<TAB>format<TAB>file<TAB>pass|fail|skip` line for
    /// tools/ci/oracle-pass-rates.sh (skip: declined like the reference decoder).
    fn check(&mut self, fmt: &str, dir: &Path, e: &Entry, got: Result<Samples, Error>, ms: f64) {
        let (same, refused) = (self.same, self.refused);
        self.verdict(dir, e, got, ms);
        let status = if self.same > same {
            "pass"
        } else if self.refused > refused {
            "skip"
        } else {
            "fail"
        };
        println!("ORACLE\t{fmt}\t{}\t{status}", e.file);
    }

    fn verdict(&mut self, dir: &Path, e: &Entry, got: Result<Samples, Error>, ms: f64) {
        if !e.ok {
            match got {
                Err(Error::Unsupported(why)) => {
                    eprintln!("{}: refused like the reference decoder: {why}", e.file);
                    self.refused += 1;
                }
                Ok(_) => {
                    eprintln!("{}: decoded (the reference decoder declines it)", e.file);
                    self.refused += 1;
                }
                Err(err) => self.errors.push(format!("{}: {err}", e.file)),
            }
            return;
        }
        let s = match got {
            Ok(s) => s,
            Err(err) => {
                self.errors.push(format!("{}: {err}", e.file));
                return;
            }
        };
        self.ms += ms;
        let ours: Vec<u8> = s
            .planes
            .iter()
            .flat_map(|p| p.data.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        self.samples += ours.len() / 2;
        let sha = format!("{:x}", Sha256::digest(&ours));
        if sha == e.sha && ours.len() / 2 == e.samples && !s.truncated {
            self.same += 1;
            return;
        }
        self.differ += 1;
        let stem = e.file.rsplit_once('.').map(|s| s.0).unwrap_or(&e.file);
        match std::fs::read(dir.join(format!("{stem}.ref"))) {
            Ok(want) => {
                let n = ours
                    .chunks_exact(2)
                    .zip(want.chunks_exact(2))
                    .filter(|(a, b)| a != b)
                    .count();
                eprintln!(
                    "{}: {n} of {} samples differ ({} decoded, truncated {})",
                    e.file,
                    want.len() / 2,
                    ours.len() / 2,
                    s.truncated
                );
            }
            Err(_) => eprintln!(
                "{}: differs ({} samples, {} expected; truncated {})",
                e.file,
                ours.len() / 2,
                e.samples,
                s.truncated
            ),
        }
    }

    fn report(&self, what: &str) {
        eprintln!(
            "JPEG XS {what}: {} identical to the reference decoder, {} differing, {} declined like it, {} errors; {:.1} M samples in {:.0} ms",
            self.same,
            self.differ,
            self.refused,
            self.errors.len(),
            self.samples as f64 / 1e6,
            self.ms
        );
        for e in &self.errors {
            eprintln!("  {e}");
        }
    }
}

#[test]
fn corpus_matches_reference_decoder() {
    let Some(root) = std::env::var_os("SPECULAR_JPEGXS_CORPUS") else {
        eprintln!("SPECULAR_JPEGXS_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    let mut failed = false;
    for (sub, fmt) in [
        ("synth", "JXS/codestream"),
        ("conformance", "JXS/conformance"),
        ("heif", "JXS/HEIF jxs1"),
    ] {
        let dir = root.join(sub);
        let mut t = Tally::default();
        for e in manifest(&dir) {
            let data = std::fs::read(dir.join(&e.file)).expect("codestream");
            let at = Instant::now();
            let got = codestream(&data).and_then(|(cs, _)| specular_jpegxs::decode_samples(&cs));
            t.check(fmt, &dir, &e, got, at.elapsed().as_secs_f64() * 1000.0);
        }
        t.report(sub);
        failed |= t.differ > 0 || !t.errors.is_empty();
    }
    // Sequences: frames in order through one decoder per sequence.
    let dir = root.join("sequences");
    let mut t = Tally::default();
    let mut seq = Sequence::new();
    let mut prefix = String::new();
    for e in manifest(&dir) {
        let p = e
            .file
            .split("_frame_")
            .next()
            .unwrap_or_default()
            .to_owned();
        if p != prefix {
            seq = Sequence::new();
            prefix = p;
        }
        let data = std::fs::read(dir.join(&e.file)).expect("frame");
        let at = Instant::now();
        let got = seq.decode(&data);
        t.check(
            "JXS/TDC sequence",
            &dir,
            &e,
            got,
            at.elapsed().as_secs_f64() * 1000.0,
        );
    }
    t.report("sequences");
    failed |= t.differ > 0 || !t.errors.is_empty();
    assert!(!failed);
}
