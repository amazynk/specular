//! NeXT 2-bit, PixarLog and JBIG against libtiff (the system library, through
//! `tools/corpus/tiff-gaps.py`): NeXT samples from GDAL's autotest data, PixarLog and JBIG
//! files written by libtiff, and libtiff's decode of each (`tools/corpus/fetch-tiff-gaps.sh`).
//! Grey output must equal libtiff's RGBA reading. PixarLog's linear floats (half output) must
//! equal libtiff's PIXARLOGDATAFMT_FLOAT reading where that reader is right (3 or 4 samples a
//! pixel, contiguous), and stay within the log coding's error of the source samples
//! (`NAME.src`) everywhere.
//! Skipped when the corpus (`$SPECULAR_TIFF_GAPS`, else `$CARGO_TARGET_DIR/corpus/tiff-gaps`)
//! is absent.

use std::path::{Path, PathBuf};

use specular_tiff::{DecodeOptions, PixelFormat, Tiff};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_TIFF_GAPS")
        .map(PathBuf::from)
        .or_else(|| {
            let t = std::env::var_os("CARGO_TARGET_DIR")?;
            Some(Path::new(&t).join("corpus/tiff-gaps"))
        })?;
    dir.is_dir().then_some(dir)
}

fn decode(data: &[u8], wide: bool) -> specular_tiff::Decoded {
    let t = Tiff::open(data).unwrap();
    t.pages()[0].check_supported().unwrap();
    t.decode(
        data,
        0,
        &DecodeOptions {
            wide,
            ..Default::default()
        },
    )
    .unwrap()
}

fn half_to_f32(h: u16) -> f32 {
    let (s, e, m) = ((h >> 15) as u32, ((h >> 10) & 31) as i32, (h & 1023) as f32);
    let v = if e == 0 {
        m * (-24f32).exp2()
    } else {
        (1.0 + m / 1024.0) * ((e - 15) as f32).exp2()
    };
    if s == 1 { -v } else { v }
}

fn f16_bits(v: f32) -> u16 {
    // Round to nearest even, as the converter does (finite, normal-range values here).
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xFF) as i32 - 127 + 15;
    if v == 0.0 {
        return sign;
    }
    if exp <= 0 {
        // Subnormal half.
        let m = (b & 0x7F_FFFF) | 0x80_0000;
        let shift = (14 - exp) as u32;
        let half = 1u32 << (shift - 1);
        let r = (m + half - 1 + ((m >> shift) & 1)) >> shift;
        return sign | r as u16;
    }
    let m = b & 0x7F_FFFF;
    let r = (m + 0xFFF + ((m >> 13) & 1)) >> 13;
    sign | ((((exp as u32) << 10) + r) as u16)
}

#[test]
fn matches_libtiff() {
    let Some(dir) = corpus() else {
        eprintln!("skipped: no corpus (tools/corpus/fetch-tiff-gaps.sh)");
        return;
    };
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "tif"))
        .collect();
    names.sort();
    let mut checked = 0;
    let mut fails = Vec::new();
    for p in names {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let data = std::fs::read(&p).unwrap();
        let (mut n, mut bad) = (0usize, 0usize);
        if let Ok(f) = std::fs::read(p.with_extension("u16")) {
            // 16-bit lossless JPEG: the source samples exactly.
            let want: Vec<u16> = f
                .chunks_exact(2)
                .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                .collect();
            let d = decode(&data, true);
            assert!(!d.truncated, "{name}");
            let got: Vec<u16> = d
                .data
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let spp = want.len() / (d.width as usize * d.height as usize);
            let per = match d.format {
                PixelFormat::Gray16 => 1,
                PixelFormat::Rgba16 => 4,
                f => panic!("{name}: {f:?}"),
            };
            for (i, px) in got.chunks_exact(per).enumerate() {
                for c in 0..spp.min(per) {
                    n += 1;
                    bad += (px[c] != want[i * spp + c]) as usize;
                }
            }
            eprintln!("{name}: {bad} of {n} samples differ from the source");
        } else if let Ok(f) = std::fs::read(p.with_extension("float")) {
            let floats = |b: &[u8]| -> Vec<f32> {
                b.chunks_exact(4)
                    .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
                    .collect()
            };
            let want = floats(&f);
            let src = floats(&std::fs::read(p.with_extension("src")).unwrap());
            let d = decode(&data, true);
            assert!(!d.truncated, "{name}");
            assert_eq!(d.format, PixelFormat::Rgba16F, "{name}");
            let (w, h) = (d.width as usize, d.height as usize);
            let spp = want.len() / (w * h);
            let planar = name.contains("planar");
            let at = |x: usize, c: usize| {
                if planar {
                    want[c * w * h + x]
                } else {
                    want[x * spp + c]
                }
            };
            // libtiff's reader accumulates the differences right only for 3 or 4 samples a
            // pixel: with one (grey, or a plane) it leaves the first pixel's value out of
            // every later one, so there the source samples are the reference.
            let libtiff_ok = !planar && matches!(spp, 3 | 4);
            let (mut off_src, mut worst) = (0usize, 0f32);
            for (i, px) in d.data.chunks_exact(8).enumerate() {
                for c in 0..spp.min(4) {
                    n += 1;
                    let v = u16::from_le_bytes([px[2 * c], px[2 * c + 1]]);
                    if libtiff_ok {
                        bad += (v.abs_diff(f16_bits(at(i, c))) > 1) as usize;
                    }
                    // The log coding keeps about 0.2 % (half rounding adds 0.05 %); the
                    // linear segment near 0 steps by 7.3e-5.
                    let s = src[i * spp + c];
                    let e = (half_to_f32(v) - s).abs();
                    worst = worst.max(e / s.abs().max(0.02));
                    off_src += (e > 0.003 * s.abs() + 6e-5) as usize;
                }
            }
            bad += off_src;
            eprintln!(
                "{name}: {} of {n} samples differ from libtiff{}; {off_src} off the source \
                 (worst relative error {worst:.1e})",
                if libtiff_ok { bad - off_src } else { 0 },
                if libtiff_ok {
                    ""
                } else {
                    " (not compared: libtiff misreads one sample a pixel)"
                }
            );
        } else if let Ok(rgba) = std::fs::read(p.with_extension("rgba")) {
            let d = decode(&data, false);
            assert!(!d.truncated, "{name}");
            assert_eq!(d.format, PixelFormat::Gray8, "{name}");
            for (o, px) in d.data.iter().zip(rgba.chunks_exact(4)) {
                n += 1;
                bad += (*o != px[0]) as usize;
            }
            eprintln!("{name}: {bad} of {n} grey samples differ from libtiff");
        } else {
            continue;
        }
        if bad != 0 || n == 0 {
            fails.push(name);
        }
        checked += 1;
    }
    eprintln!("{checked} files checked against libtiff");
    assert!(fails.is_empty(), "differ from libtiff: {fails:?}");
}

fn fixture(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/gaps")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The checked-in subset (`tests/fixtures/gaps`: GDAL's three NeXT samples, MIT; small
/// libtiff-written JBIG and PixarLog files) with libtiff's strips or the source samples.
#[test]
fn small_set() {
    // NeXT: libtiff gives one packed byte, four 2-bit pixels (MinIsBlack).
    for name in ["next_default_case", "next_literalrow", "next_literalspan"] {
        let want = fixture(&format!("{name}.raw"));
        let d = decode(&fixture(&format!("{name}.tif")), false);
        assert_eq!(d.format, PixelFormat::Gray8, "{name}");
        let grey: Vec<u8> = (0..4)
            .map(|x| ((want[0] >> (6 - 2 * x)) & 3) * 85)
            .collect();
        assert_eq!(d.data, grey, "{name}");
    }
    // JBIG, MinIsBlack: a set bit is white.
    let want = fixture("jbig_minisblack.raw");
    let d = decode(&fixture("jbig_minisblack.tif"), false);
    let (w, row) = (d.width as usize, (d.width as usize).div_ceil(8));
    for (i, &g) in d.data.iter().enumerate() {
        let (x, y) = (i % w, i / w);
        let bit = (want[y * row + x / 8] >> (7 - x % 8)) & 1;
        assert_eq!(g, bit * 255, "jbig at {x},{y}");
    }
    // PixarLog RGB: libtiff's floats, as half.
    let want: Vec<f32> = fixture("pixarlog_rgb16.float")
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
        .collect();
    let d = decode(&fixture("pixarlog_rgb16.tif"), true);
    assert_eq!(d.format, PixelFormat::Rgba16F);
    for (i, px) in d.data.chunks_exact(8).enumerate() {
        for c in 0..3 {
            let v = u16::from_le_bytes([px[2 * c], px[2 * c + 1]]);
            assert!(
                v.abs_diff(f16_bits(want[i * 3 + c])) <= 1,
                "pixarlog rgb {i}"
            );
        }
    }
    // Tiled PixarLog (16×16 tiles of a 45×37 image): libtiff codes a tile as image-width rows
    // and decodes only the whole ones, leaving the rest of each tile 0. Where libtiff decodes,
    // its floats as half; everywhere, within the coding's error of the source.
    let want: Vec<f32> = fixture("pixarlog_rgb16_tiled.float")
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
        .collect();
    let src: Vec<f32> = fixture("pixarlog_rgb16_tiled.src")
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
        .collect();
    let d = decode(&fixture("pixarlog_rgb16_tiled.tif"), true);
    assert_eq!((d.width, d.height), (45, 37));
    let (mut off_libtiff, mut off_src, mut compared) = (0, 0, 0);
    for (i, px) in d.data.chunks_exact(8).enumerate() {
        for c in 0..3 {
            let v = u16::from_le_bytes([px[2 * c], px[2 * c + 1]]);
            let w = want[i * 3 + c];
            if w != 0.0 {
                compared += 1;
                off_libtiff += usize::from(v.abs_diff(f16_bits(w)) > 1);
            }
            let s = src[i * 3 + c];
            off_src += usize::from((half_to_f32(v) - s).abs() > 0.003 * s + 1e-3);
        }
    }
    eprintln!(
        "tiled PixarLog: {off_libtiff} of {compared} differ from libtiff, {off_src} off the source"
    );
    assert_eq!((off_libtiff, off_src), (0, 0));
    // 16-bit lossless JPEG (tifffile + libjpeg-turbo 3): the source samples.
    let want: Vec<u16> = fixture("ljpeg16_gray.u16")
        .chunks_exact(2)
        .map(|c| u16::from_ne_bytes([c[0], c[1]]))
        .collect();
    let d = decode(&fixture("ljpeg16_gray.tif"), true);
    assert_eq!(d.format, PixelFormat::Gray16);
    let got: Vec<u16> = d
        .data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    assert_eq!(got, want);
    // PixarLog grey: within the coding's error of the source.
    let src: Vec<f32> = fixture("pixarlog_gray8.src")
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
        .collect();
    let d = decode(&fixture("pixarlog_gray8.tif"), true);
    for (px, &s) in d.data.chunks_exact(8).zip(&src) {
        let v = half_to_f32(u16::from_le_bytes([px[0], px[1]]));
        assert!(
            (v - s).abs() <= 0.003 * s + 6e-5,
            "pixarlog grey {v} vs {s}"
        );
    }
}

#[test]
fn damaged_streams_never_panic() {
    for name in [
        "jbig_minisblack",
        "pixarlog_rgb16",
        "next_literalspan",
        "ljpeg16_gray",
    ] {
        let f = fixture(&format!("{name}.tif"));
        for i in (8..f.len()).step_by(3) {
            let mut b = f.clone();
            b[i] ^= 0x5A;
            if let Ok(t) = Tiff::open(&b) {
                let _ = t.decode(&b, 0, &DecodeOptions::default());
            }
            b.truncate(i);
            if let Ok(t) = Tiff::open(&b) {
                let _ = t.decode(&b, 0, &DecodeOptions::default());
            }
        }
    }
}
