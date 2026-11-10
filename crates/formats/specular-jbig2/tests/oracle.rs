//! Pixel comparison with the hayro-jbig2 crate (oracle only) over the corpus of
//! `tools/corpus/fetch-raster-jbig2.sh`, plus the small committed fixtures.

use std::path::Path;

use specular_jbig2::{Pixels, decode, info, probe};

struct Collect {
    px: Vec<u8>,
}

impl hayro_jbig2::Decoder for Collect {
    fn push_pixel(&mut self, black: bool) {
        self.px.push(if black { 0 } else { 255 });
    }
    fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
        let v = if black { 0 } else { 255 };
        self.px
            .extend(std::iter::repeat_n(v, chunk_count as usize * 8));
    }
    fn next_line(&mut self) {}
}

/// hayro's reading of the first page: (width, height, grey pixels).
fn oracle(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = hayro_jbig2::Image::new(data).ok()?;
    let mut c = Collect { px: Vec::new() };
    img.decode(&mut c).ok()?;
    Some((img.width(), img.height(), c.px))
}

/// Ours: (width, height, grey pixels, truncated).
fn ours(data: &[u8]) -> Result<(u32, u32, Vec<u8>, bool), specular_jbig2::Error> {
    let img = decode(data, 0)?;
    let Pixels::Gray8(px) = img.pixels else {
        panic!("not grey")
    };
    Ok((img.width, img.height, px, img.truncated))
}

/// A binary PGM (P5, 8-bit): width, height, pixels.
fn pgm(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let d = std::fs::read(path).ok()?;
    let mut fields = Vec::new();
    let mut i = 2;
    while fields.len() < 3 {
        while d.get(i)?.is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !d.get(i)?.is_ascii_whitespace() {
            i += 1;
        }
        fields.push(std::str::from_utf8(&d[s..i]).ok()?.parse::<u32>().ok()?);
    }
    Some((fields[0], fields[1], d[i + 1..].to_vec()))
}

fn compare(name: &str, data: &[u8]) -> Result<bool, String> {
    compare_with(name, data, None)
}

fn compare_with(
    name: &str,
    data: &[u8],
    picture: Option<&(u32, u32, Vec<u8>)>,
) -> Result<bool, String> {
    let o = oracle(data).or_else(|| {
        let p = picture.filter(|_| name.starts_with("bitmap"))?.clone();
        eprintln!("  {name}: oracle refuses, compared with bitmap.pgm");
        Some(p)
    });
    let m = ours(data);
    match (o, m) {
        (Some((ow, oh, opx)), Ok((w, h, px, _))) => {
            if (ow, oh) != (w, h) {
                return Err(format!("{name}: size {w}×{h}, oracle {ow}×{oh}"));
            }
            let diff = px.iter().zip(&opx).filter(|(a, b)| a != b).count();
            if diff != 0 || px.len() != opx.len() {
                return Err(format!("{name}: {diff} pixels differ"));
            }
            let i = info(data, 0).map_err(|e| format!("{name}: info: {e}"))?;
            if (i.width, i.height) != (w, h) {
                return Err(format!(
                    "{name}: info says {}×{}, decode {w}×{h}",
                    i.width, i.height
                ));
            }
            Ok(true)
        }
        (None, Err(e)) => {
            eprintln!("  {name}: both refuse ({e})");
            Ok(false)
        }
        (None, Ok(_)) => {
            eprintln!("  {name}: oracle refuses, we decode");
            Ok(false)
        }
        (Some(_), Err(e)) => Err(format!("{name}: we fail: {e}")),
    }
}

#[test]
fn fixtures_match_oracle() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "jbig2") {
            let d = std::fs::read(&p).unwrap();
            assert!(probe(&d));
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            compare(&name, &d).unwrap();
            n += 1;
        }
    }
    assert!(n >= 5, "{n} fixtures");
}

#[test]
#[ignore = "needs the corpus: tools/corpus/fetch-raster-jbig2.sh"]
fn corpus_matches_oracle() {
    let Some(dir) = std::env::var_os("SPECULAR_RASTER_CORPUS_JBIG2") else {
        eprintln!("SPECULAR_RASTER_CORPUS_JBIG2 not set");
        return;
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(dir);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jbig2" || x == "jb2"))
        .collect();
    files.sort();
    let picture = pgm(&dir.join("bitmap.pgm"));
    let (mut pass, mut fail) = (0, Vec::new());
    let mut total = std::time::Duration::ZERO;
    for p in &files {
        let d = std::fs::read(p).unwrap();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let t0 = std::time::Instant::now();
        let _ = decode(&d, 0);
        let dt = t0.elapsed();
        total += dt;
        if dt.as_millis() > 20 {
            eprintln!("  {name}: {:.1} ms", dt.as_secs_f64() * 1e3);
        }
        // One line per file for tools/ci/oracle-pass-rates.sh (skip: no oracle reading).
        let status = match compare_with(&name, &d, picture.as_ref()) {
            Ok(compared) => {
                pass += 1;
                if compared { "pass" } else { "skip" }
            }
            Err(e) => {
                fail.push(e);
                "fail"
            }
        };
        println!("ORACLE\tJBIG2\t{name}\t{status}");
    }
    for f in &fail {
        eprintln!("MISMATCH {f}");
    }
    eprintln!(
        "jbig2 corpus: {pass}/{} match the oracle, {} mismatches; decode total {:.1} ms",
        files.len(),
        fail.len(),
        total.as_secs_f64() * 1e3
    );
    assert!(fail.is_empty());
}
