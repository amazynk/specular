//! OIR, SPE and SIF from the fetched corpus (tools/corpus/fetch-microscopy.sh) against the
//! oracle values written beside it (gen/spectro-oracle.py: Bio-Formats, spe2py / imageio,
//! sif_parser). Reads SPECULAR_MICROSCOPY_CORPUS, else tools/corpus/microscopy; skips when it
//! has no expected-micro.tsv. Values match exactly; wavelengths within 1e-6 nm.

use std::path::{Path, PathBuf};

use specular_micro::spectral::Spectral;
use specular_micro::{oir, sif, spe};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MICROSCOPY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/microscopy")
        });
    dir.join("expected-micro.tsv").is_file().then_some(dir)
}

fn f32s(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn spectral(rel: &str, data: &[u8]) -> Result<Spectral, String> {
    if rel.to_ascii_lowercase().ends_with(".sif") {
        sif::parse(data)
    } else {
        spe::parse(data)
    }
    .map_err(|e| e.to_string())
}

#[test]
fn corpus_matches_the_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("microscopy corpus absent: run tools/corpus/fetch-microscopy.sh");
        return;
    };
    let text = std::fs::read_to_string(dir.join("expected-micro.tsv")).unwrap();
    let mut bad = Vec::new();
    let mut checks = 0;
    let mut cache: Option<(String, Vec<u8>)> = None;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let rel = f[1];
        if cache.as_ref().is_none_or(|c| c.0 != rel) {
            cache = Some((rel.to_owned(), std::fs::read(dir.join(rel)).unwrap()));
        }
        let data = &cache.as_ref().unwrap().1;
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        checks += 1;
        match f[0] {
            "pages" | "page" | "wl" | "fact" => {
                let s = match spectral(rel, data) {
                    Ok(s) => s,
                    Err(e) => {
                        bad.push(format!("{rel}: {e}"));
                        continue;
                    }
                };
                match f[0] {
                    "pages" if s.pages.len() != n(2) => {
                        bad.push(format!("{rel}: {} pages, oracle {}", s.pages.len(), f[2]))
                    }
                    "page" => {
                        let p = &s.pages[n(2)];
                        if (p.width, p.height) != (n(3), n(4)) {
                            bad.push(format!(
                                "{rel} page {}: {}x{}, oracle {}x{}",
                                f[2], p.width, p.height, f[3], f[4]
                            ));
                            continue;
                        }
                        let want = f32s(&std::fs::read(dir.join(f[5])).unwrap());
                        let (got, _) = p.read(data).unwrap();
                        let diff = got.iter().zip(&want).filter(|(a, b)| a != b).count();
                        if diff > 0 {
                            bad.push(format!("{rel} page {}: {diff} values differ", f[2]));
                        }
                    }
                    "wl" if f[3] != "-" => {
                        let want: Vec<f64> = std::fs::read(dir.join(f[3]))
                            .unwrap()
                            .chunks_exact(8)
                            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                            .collect();
                        match &s.pages[n(2)].wavelengths {
                            // sif_parser evaluates SIF polynomials over the whole frame's length: its first
                            // columns are compared.
                            Some(w)
                                if w.len() <= want.len()
                                    && w.iter().zip(&want).all(|(a, b)| (a - b).abs() < 1e-6) => {}
                            w => bad.push(format!(
                                "{rel} page {}: wavelengths {:?}…, oracle {:?}…",
                                f[2],
                                w.as_ref().map(|w| w[0]),
                                want.first()
                            )),
                        }
                    }
                    "fact" => {
                        let want: f64 = f[3].parse().unwrap();
                        let got = match f[2] {
                            "exposure" => s.exposure,
                            _ => s.temperature,
                        };
                        if got.is_none_or(|g| (g - want).abs() > 1e-6 * want.abs().max(1.0)) {
                            bad.push(format!("{rel}: {} {got:?}, oracle {want}", f[2]));
                        }
                    }
                    _ => {}
                }
            }
            "refuse" => {
                if oir::parse(data).is_ok() {
                    bad.push(format!("{rel}: opens, the oracle refuses it"));
                }
            }
            "image" | "plane" => {
                let o = match oir::parse(data) {
                    Ok(o) => o,
                    Err(e) => {
                        bad.push(format!("{rel}: {e}"));
                        continue;
                    }
                };
                // Bio-Formats counts λ as channels (files hold one channel per λ scan).
                let c = o.channels.len() * o.size_l;
                if f[0] == "image" {
                    let got = (o.width, o.height, c, o.size_z, o.size_t);
                    let want = (n(2), n(3), n(4), n(5), n(6));
                    if got != want {
                        bad.push(format!("{rel}: WxHxCxZxT {got:?}, oracle {want:?}"));
                    }
                    continue;
                }
                let (bc, z, t) = (n(2), n(3), n(4));
                let (ch, l) = if o.size_l > 1 {
                    (bc / o.size_l, bc % o.size_l)
                } else {
                    (bc, 0)
                };
                let want = f32s(&std::fs::read(dir.join(f[5])).unwrap());
                let mut got = vec![0f32; o.width * o.height];
                o.read_plane(data, ch, l, z, t, &mut got).unwrap();
                let diff = got.iter().zip(&want).filter(|(a, b)| a != b).count();
                if diff > 0 {
                    bad.push(format!("{rel} c{bc} z{z} t{t}: {diff} values differ"));
                }
            }
            k => panic!("unknown row {k}"),
        }
    }
    eprintln!("micro corpus: {checks} checks, {} mismatches", bad.len());
    for b in &bad {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}
