//! Checks every plane listed in `expected.tsv` of the generated corpus (`gen/make_samples.py`):
//! `$SPECULAR_SCI_CORPUS/xisf`, else `tools/corpus/sci/xisf`. Skips when absent.

use std::path::PathBuf;
use std::time::Instant;

fn corpus_dir() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_SCI_CORPUS") {
        Some(d) => PathBuf::from(d).join("xisf"),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/sci/xisf"),
    };
    dir.join("expected.tsv").is_file().then_some(dir)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn expected_planes() {
    let Some(dir) = corpus_dir() else {
        eprintln!("XISF corpus not found; skipping");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut checked = 0;
    let mut cache: Option<(String, Vec<u8>, specular_xisf::Xisf)> = None;
    for line in tsv.lines().skip(1).filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let [name, image, c, z, w, h, sum, min, max, samples] = f[..] else {
            panic!("bad line {line:?}");
        };
        if cache.as_ref().is_none_or(|(n, ..)| n != name) {
            let data = std::fs::read(dir.join(name)).unwrap();
            let x = specular_xisf::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
            cache = Some((name.to_owned(), data, x));
        }
        let (_, data, x) = cache.as_ref().unwrap();
        let img = &x.images[image.parse::<usize>().unwrap()];
        let (w, h): (u32, u32) = (w.parse().unwrap(), h.parse().unwrap());
        assert_eq!((img.width, img.height), (w, h), "{name}");
        let (c, z) = (c.parse().unwrap(), z.parse().unwrap());
        let p = img
            .plane_f32(data, c, z)
            .unwrap_or_else(|e| panic!("{name} c{c} z{z}: {e}"));
        let s: f64 = p.iter().map(|&v| f64::from(v)).sum();
        let lo = p.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = p.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let what = format!("{name} image {image} c{c} z{z}");
        assert!(
            close(s, sum.parse().unwrap(), 1e-9),
            "{what}: sum {s} vs {sum}"
        );
        assert_eq!(f64::from(lo), min.parse::<f64>().unwrap(), "{what}: min");
        assert_eq!(f64::from(hi), max.parse::<f64>().unwrap(), "{what}: max");
        for sample in samples.split(' ') {
            let (xy, v) = sample.split_once('=').unwrap();
            let (sx, sy) = xy.split_once(',').unwrap();
            let (sx, sy): (usize, usize) = (sx.parse().unwrap(), sy.parse().unwrap());
            let got = p[sy * w as usize + sx];
            assert_eq!(f64::from(got), v.parse::<f64>().unwrap(), "{what} at {xy}");
        }
        checked += 1;
    }
    eprintln!("checked {checked} planes");
    assert!(checked > 0);
}

#[test]
fn keywords_in_corpus() {
    let Some(dir) = corpus_dir() else {
        return;
    };
    let data = std::fs::read(dir.join("py_wcs_tan.xisf")).unwrap();
    let x = specular_xisf::parse(&data).unwrap();
    let img = &x.images[0];
    assert_eq!(img.keyword("CTYPE1").unwrap().text(), "RA---TAN");
    assert_eq!(img.keyword("OBJECT").unwrap().text(), "M31");
    assert_eq!(img.keyword("CRVAL2").unwrap().value, "41.26875");
    assert_eq!(
        img.property("Observation:Object:Name").unwrap().value,
        specular_xisf::PropertyValue::String("M31".into())
    );
}

/// Decode speed of the 4096×4096 Float32 files (`make_samples.py --big`); run with
/// `--ignored --nocapture` in release.
#[test]
#[ignore]
fn speed_big_f32() {
    let Some(dir) = corpus_dir() else {
        return;
    };
    for name in ["big_f32.xisf", "big_f32_zstd_sh.xisf"] {
        let Ok(data) = std::fs::read(dir.join(name)) else {
            eprintln!("{name} missing");
            continue;
        };
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = Instant::now();
            let x = specular_xisf::parse(&data).unwrap();
            let p = x.images[0].plane_f32(&data, 0, 0).unwrap();
            std::hint::black_box(&p);
            best = best.min(t.elapsed().as_secs_f64());
        }
        eprintln!("{name}: {:.1} ms (best of 5)", best * 1e3);
    }
}
