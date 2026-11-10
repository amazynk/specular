//! Raw video clips from `tools/corpus/fetch-rawvid1.sh` (never committed; skipped unless
//! `SPECULAR_RAWVID1_DIR` names the folder): Canon Cinema RAW Light and Apple ProRes RAW.
//!
//! No decoder outside Canon's own reads Cinema RAW Light, so its frames are checked against
//! what the CR3 path guarantees (the CRX code is shared and bit-exact with LibRaw on CR3
//! files, `tests/samples.rs`): the frames decode whole, inside black and white, differ from
//! each other (a moving clip) and develop to the picture of the clip's embedded thumbnail.
//! ProRes RAW frames are compared with FFmpeg in `specular-prores`'s `raw_oracle`; here the
//! clip's container, colour data and develop are checked.

use std::path::PathBuf;

use specular_raw::{DecodeOptions, OutputSize, RawFormat};

fn corpus(name: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SPECULAR_RAWVID1_DIR")?;
    std::fs::read(PathBuf::from(dir).join(name)).ok()
}

/// Luma (0..1) of an 8-bit RGBA picture, averaged into `w × h` cells, without the black
/// bars above and below the picture (a 16:9 clip's 4:3 thumbnail).
fn cells_rgba(rgba: &[u8], pw: u32, ph: u32, w: u32, h: u32) -> Vec<f64> {
    let luma = |x: u32, y: u32| {
        let i = ((y * pw + x) * 4) as usize;
        (0.2126 * f64::from(rgba[i])
            + 0.7152 * f64::from(rgba[i + 1])
            + 0.0722 * f64::from(rgba[i + 2]))
            / 255.0
    };
    let row = |y: u32| (0..pw).map(|x| luma(x, y)).sum::<f64>() / f64::from(pw);
    let top = (0..ph).find(|&y| row(y) > 0.03).unwrap_or(0);
    let bottom = (0..ph).rev().find(|&y| row(y) > 0.03).unwrap_or(ph - 1) + 1;
    let ch = bottom - top;
    let mut out = vec![0.0; (w * h) as usize];
    let mut n = vec![0u32; (w * h) as usize];
    for y in top..bottom {
        for x in 0..pw {
            let l = luma(x, y) * 255.0;
            let c = (((y - top) * h / ch) * w + x * w / pw) as usize;
            out[c] += l / 255.0;
            n[c] += 1;
        }
    }
    out.iter()
        .zip(n)
        .map(|(v, n)| v / f64::from(n.max(1)))
        .collect()
}

/// Luma of a linear 16-bit RGB develop, encoded with a plain 1/2.2 gamma, in cells.
fn cells_rgb16(rgb: &[u16], pw: u32, ph: u32, w: u32, h: u32) -> Vec<f64> {
    let mut out = vec![0.0; (w * h) as usize];
    let mut n = vec![0u32; (w * h) as usize];
    for y in 0..ph {
        for x in 0..pw {
            let i = ((y * pw + x) * 3) as usize;
            let l = (0.2126 * f64::from(rgb[i])
                + 0.7152 * f64::from(rgb[i + 1])
                + 0.0722 * f64::from(rgb[i + 2]))
                / 65535.0;
            let c = ((y * h / ph) * w + x * w / pw) as usize;
            out[c] += l.max(0.0).powf(1.0 / 2.2);
            n[c] += 1;
        }
    }
    out.iter()
        .zip(n)
        .map(|(v, n)| v / f64::from(n.max(1)))
        .collect()
}

/// Pearson correlation.
fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    sab / (saa * sbb).sqrt()
}

#[test]
fn cinema_raw_light_clip() {
    let Some(data) = corpus("c200_prefix.crm") else {
        eprintln!("SPECULAR_RAWVID1_DIR not set or c200_prefix.crm missing; skipping");
        return;
    };
    assert_eq!(specular_raw::detect(&data), Some(RawFormat::Crm));
    let f = specular_raw::parse(&data).unwrap();
    let info = f.info();
    let clip = info.clip.as_ref().expect("a clip");
    eprintln!(
        "{} {}: {} of {} frames, {:?} fps, sound {:?}, facts {:?}",
        info.make,
        info.model,
        clip.frames.len(),
        clip.listed,
        clip.frame_rate,
        clip.audio,
        clip.facts
    );
    assert_eq!(info.model, "Canon EOS C200");
    // The prefix holds the first chunk of 10 frames; the table lists the whole clip.
    assert_eq!(clip.frames.len(), 10);
    assert!(clip.listed > 10);
    assert!((clip.frame_rate.unwrap() - 59.94).abs() < 0.01);
    let s = info.sensor.as_ref().unwrap();
    assert!(s.decodable, "{:?}", s.unsupported_reason);
    assert_eq!((s.width, s.height), (4112, 2176));
    // The Exif picture size, centred.
    assert_eq!((s.crop.x, s.crop.y, s.crop.w, s.crop.h), (8, 8, 4096, 2160));
    let opts = DecodeOptions::default();
    let mut means = Vec::new();
    for i in [0usize, 1, 9] {
        let fr = f.frame(i).unwrap();
        let t = std::time::Instant::now();
        let r = fr.decode_raw(&data, &opts).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        assert!(!r.truncated, "frame {i}");
        assert_eq!(r.black, [64.0; 4]);
        let black = r.black.iter().copied().fold(f32::MAX, f32::min);
        let above = r
            .data
            .iter()
            .filter(|&&v| f32::from(v) > r.white + 1.0)
            .count();
        let mean = r.data.iter().map(|&v| f64::from(v)).sum::<f64>() / r.data.len() as f64;
        eprintln!(
            "frame {i}: black {:?} white {} wb {:?} mean {mean:.1}, {above} above white, {ms:.0} ms",
            r.black, r.white, r.wb
        );
        assert!(mean > f64::from(black) && mean < f64::from(r.white));
        assert_eq!(above, 0);
        means.push((mean, r.data));
    }
    // Different frames, close in level (one shot).
    assert!(means[0].1 != means[1].1);
    assert!((means[0].0 - means[2].0).abs() / means[0].0 < 0.05);
    assert!(f.frame(10).is_none());
    // The develop against the pictures the camera rendered into the clip: the thumbnail
    // and the larger preview (Canon's own processing, so only the picture is compared).
    let half = DecodeOptions {
        size: OutputSize::Half,
        ..Default::default()
    };
    let raw = f.decode_raw(&data, &half).unwrap();
    let img = specular_raw::develop(&raw, &half).unwrap();
    let (cw, ch) = (32, 18);
    let b = cells_rgb16(&img.data, img.width, img.height, cw, ch);
    let mut seen = 0;
    for p in &info.previews {
        let pic = specular_raw::decode_preview(&data, p, 0).unwrap();
        let a = cells_rgba(&pic.rgba, pic.width, pic.height, cw, ch);
        let c = correlation(&a, &b);
        eprintln!(
            "{} {}×{} vs develop {}×{}: luma correlation {c:.3} over {cw}×{ch} cells",
            p.source, pic.width, pic.height, img.width, img.height
        );
        assert!(c > 0.9, "{}: correlation {c}", p.source);
        seen += 1;
    }
    assert!(seen >= 1);
}

#[test]
fn prores_raw_clip() {
    let Some(data) = corpus("fs_aprh_4f.mov") else {
        eprintln!("SPECULAR_RAWVID1_DIR not set or fs_aprh_4f.mov missing; skipping");
        return;
    };
    assert_eq!(specular_raw::detect(&data), Some(RawFormat::ProResRaw));
    let f = specular_raw::parse(&data).unwrap();
    let info = f.info();
    let clip = info.clip.as_ref().expect("a clip");
    eprintln!(
        "{} {:?} ISO {:?}: {} frames, {:?} fps, facts {:?}",
        info.make,
        info.software,
        info.iso,
        clip.frames.len(),
        clip.frame_rate,
        clip.facts
    );
    assert_eq!(clip.codec, "Apple ProRes RAW HQ");
    assert_eq!((clip.frames.len(), clip.listed), (4, 4));
    assert_eq!(info.make, "Sony");
    assert_eq!(info.iso, Some(4000));
    assert!((clip.frame_rate.unwrap() - 25.0).abs() < 1e-6);
    let s = info.sensor.as_ref().unwrap();
    assert!(s.decodable);
    assert_eq!((s.crop.x, s.crop.y, s.crop.w, s.crop.h), (8, 8, 4096, 2160));
    let opts = DecodeOptions::default();
    let t = std::time::Instant::now();
    let r = f.frame(3).unwrap().decode_raw(&data, &opts).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let min = r.data.iter().copied().min().unwrap();
    let max = r.data.iter().copied().max().unwrap();
    eprintln!(
        "frame 3: black {:?} white {} wb {:?} matrix {:?}, samples {min}..{max}, {ms:.0} ms",
        r.black, r.white, r.wb, r.matrix_source
    );
    assert_eq!(r.black, [256.0; 4]);
    assert_eq!(r.white, 61568.0);
    assert!(f32::from(max) <= r.white && min > 0);
    assert_eq!(
        r.matrix_source,
        specular_raw::MatrixSource::CameraCalibration
    );
    // White-balanced camera white develops to neutral: the matrix keeps white.
    for row in r.rgb_cam {
        assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-4);
    }
    let half = DecodeOptions {
        size: OutputSize::Half,
        ..Default::default()
    };
    let img = specular_raw::develop(&f.decode_raw(&data, &half).unwrap(), &half).unwrap();
    assert_eq!((img.width, img.height), (2048, 1080));
}
