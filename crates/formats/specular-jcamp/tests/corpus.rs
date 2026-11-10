//! Reads every file of the corpus (`tools/corpus/fetch-pcb-spectra.sh jcamp`) and prints what
//! it found, with the time taken:
//!
//! `SPECULAR_JCAMP_CORPUS=tools/corpus/pcb-spectra/jcamp cargo test -p specular-jcamp --test
//! corpus -- --ignored --nocapture`

use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
#[ignore]
fn corpus_reads() {
    let Some(dir) = std::env::var_os("SPECULAR_JCAMP_CORPUS") else {
        eprintln!("SPECULAR_JCAMP_CORPUS not set");
        return;
    };
    let mut all = Vec::new();
    files(Path::new(&dir), &mut all);
    let (mut ok, mut failed, mut unsniffed) = (0, 0, 0);
    let mut slowest = (0.0f64, PathBuf::new(), 0usize);
    for p in &all {
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let bytes = std::fs::read(p).expect("reads");
        let t = std::time::Instant::now();
        let r = match ext.as_str() {
            "spc" => specular_jcamp::spc::parse(&bytes),
            "jdx" | "dx" | "jcamp" => specular_jcamp::jcamp::parse(&bytes),
            _ => continue,
        };
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let head = &bytes[..bytes.len().min(4096)];
        let sniffed = match ext.as_str() {
            "spc" => specular_jcamp::spc::sniff(head),
            _ => specular_jcamp::jcamp::sniff(head),
        };
        if r.is_ok() && !sniffed {
            println!("NOT SNIFFED {}", p.display());
            unsniffed += 1;
        }
        if ms > slowest.0 {
            slowest = (ms, p.clone(), bytes.len());
        }
        let name = p.strip_prefix(&dir).unwrap_or(p).display();
        match r {
            Ok(s) => {
                ok += 1;
                let p0 = &s.pages[0];
                println!(
                    "ok   {name}: {} pages, {} points, x {} [{}], y {}; {}",
                    s.pages.len(),
                    s.facts.points,
                    p0.x.title,
                    p0.x.unit,
                    p0.y.title,
                    s.facts.warnings.join(" | ")
                );
            }
            Err(e) => {
                failed += 1;
                println!("FAIL {name}: {e}");
            }
        }
    }
    assert_eq!(unsniffed, 0, "files read but not recognised by content");
    println!(
        "{ok} read, {failed} refused; slowest {:.2} ms ({} bytes, {})",
        slowest.0,
        slowest.2,
        slowest.1.display()
    );
}
