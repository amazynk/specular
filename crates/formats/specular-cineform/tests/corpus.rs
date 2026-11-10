//! Real CineForm files against FFmpeg's decode (`tools/corpus/fetch-cineform.sh`; ignored unless
//! `SPECULAR_CINEFORM_CORPUS` points at its output): SPECULAR_CINEFORM_CORPUS=… cargo test -p
//! specular-cineform --test corpus -- --ignored --nocapture

use specular_cineform::{Decoder, Layout, Transform, probe};

fn dir() -> Option<std::path::PathBuf> {
    let d = std::path::PathBuf::from(std::env::var_os("SPECULAR_CINEFORM_CORPUS")?).join("cfhd");
    d.is_dir().then_some(d)
}

fn read16(p: &std::path::Path) -> Vec<u16> {
    std::fs::read(p)
        .unwrap()
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect()
}

/// (differing samples, total, PSNR at 10 bits) of a 4:2:2 picture against a yuv422p10 frame.
fn compare_422(pic: &specular_cineform::Picture, want: &[u16]) -> (usize, usize, f64) {
    let shift = 10 - u32::from(pic.bits);
    let (mut bad, mut se, mut n) = (0, 0f64, 0);
    let mut at = 0;
    for p in &pic.planes {
        for (&a, &b) in p.iter().zip(&want[at..at + p.len()]) {
            let a = a << shift;
            let d = f64::from(a.abs_diff(b));
            bad += usize::from(a != b);
            se += d * d;
            n += 1;
        }
        at += p.len();
    }
    (
        bad,
        n,
        10.0 * (1023.0f64.powi(2) / (se / n as f64).max(1e-9)).log10(),
    )
}

fn frames(d: &std::path::Path, prefix: &str, n: usize) -> Vec<Vec<u8>> {
    (1..=n)
        .map(|i| std::fs::read(d.join(format!("{prefix}{i:02}.bin"))).unwrap())
        .collect()
}

#[test]
#[ignore = "needs tools/corpus/fetch-cineform.sh"]
fn spatial_8bit_codebook9() {
    let Some(d) = dir() else { return };
    let want = read16(&d.join("tl.ref"));
    let fs = frames(&d, "tl", 2);
    let info = probe(&fs[0]).unwrap();
    assert_eq!((info.width, info.height, info.precision), (1280, 720, 8));
    let mut dec = Decoder::new();
    let per = 1280 * 720 * 2;
    for (i, f) in fs.iter().enumerate() {
        let t = std::time::Instant::now();
        let pic = dec.decode(f).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let (bad, n, psnr) = compare_422(&pic, &want[i * per..(i + 1) * per]);
        eprintln!("timelapse {i}: {bad} of {n} differ, PSNR {psnr:.1} dB, {ms:.1} ms");
        assert!(psnr > 45.0);
    }
}

#[test]
#[ignore = "needs tools/corpus/fetch-cineform.sh"]
fn fieldplus_groups() {
    let Some(d) = dir() else { return };
    let want = read16(&d.join("bt.ref"));
    let fs = frames(&d, "bt", 4);
    let info = probe(&fs[0]).unwrap();
    assert_eq!(info.transform, Transform::FieldPlus);
    let mut dec = Decoder::new();
    let per = 1280 * 720 * 2;
    for (i, f) in fs.iter().enumerate() {
        let pic = dec.decode(f).unwrap_or_else(|e| panic!("sample {i}: {e}"));
        let (bad, n, psnr) = compare_422(&pic, &want[i * per..(i + 1) * per]);
        eprintln!("beartooth {i}: {bad} of {n} differ, PSNR {psnr:.1} dB");
        assert!(psnr > 40.0);
    }
}

#[test]
#[ignore = "needs tools/corpus/fetch-cineform.sh"]
fn bayer_raw() {
    let Some(d) = dir() else { return };
    let want = read16(&d.join("lr.ref"));
    // FFmpeg's first frame is the file's second sample (an edit list drops the first).
    let f = std::fs::read(d.join("lr02.bin")).unwrap();
    let info = probe(&f).unwrap();
    assert_eq!(info.layout, Layout::Bayer);
    let t = std::time::Instant::now();
    let pic = Decoder::new().decode(&f).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let (w, h) = (pic.width as usize, pic.height as usize);
    assert_eq!(want.len(), w * h);
    // At each site the demosaiced plane of the site's colour holds the mosaic value.
    let (mut bad, mut se) = (0usize, 0f64);
    for y in 0..h {
        for x in 0..w {
            let p = match (y & 1, x & 1) {
                (0, 0) => 0,
                (1, 1) => 2,
                _ => 1,
            };
            let a = pic.planes[p][y * w + x] << 4;
            let b = want[y * w + x];
            bad += usize::from(a != b);
            let dd = f64::from(a.abs_diff(b));
            se += dd * dd;
        }
    }
    let psnr = 10.0 * (65535.0f64.powi(2) / (se / (w * h) as f64).max(1e-9)).log10();
    eprintln!(
        "Bayer {w}x{h}: {bad} of {} differ, PSNR {psnr:.1} dB, {ms:.0} ms",
        w * h
    );
    assert!(psnr > 50.0);
}
