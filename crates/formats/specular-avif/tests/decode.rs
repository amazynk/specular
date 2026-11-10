//! Decoding tests against ffmpeg (libdav1d + swscale) reference output stored in tests/data.
//!
//! Fixtures are tiny synthetic images (ffmpeg testsrc2 / ImageMagick gradients) encoded with
//! libaom; references were produced with
//! `ffmpeg -c:v libdav1d -i X.avif -sws_flags accurate_rnd+full_chroma_int+bilinear ...`.
//! swscale's chroma upsampling differs from ours at the edges, so comparisons use a
//! mean-error bound. The chroma-siting tests at the end compare tightly against libavif
//! instead.

use specular_avif::{AvifFile, Decoded, Error, HdrTransfer, Layout, probe};

fn data(name: &str) -> Vec<u8> {
    let p = format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// Minimal PNM reader (P5, P6, P7 with 8-bit samples): (width, height, channels, data).
fn pnm(name: &str) -> (usize, usize, usize, Vec<u8>) {
    let d = data(name);
    let mut pos = 0;
    let mut token = || -> String {
        while d[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let s = pos;
        while !d[pos].is_ascii_whitespace() {
            pos += 1;
        }
        String::from_utf8_lossy(&d[s..pos]).into_owned()
    };
    let magic = token();
    let (w, h, c) = if magic == "P7" {
        let (mut w, mut h, mut c) = (0, 0, 0);
        loop {
            match token().as_str() {
                "WIDTH" => w = token().parse().unwrap(),
                "HEIGHT" => h = token().parse().unwrap(),
                "DEPTH" => c = token().parse().unwrap(),
                "MAXVAL" | "TUPLTYPE" => {
                    token();
                }
                "ENDHDR" => break,
                t => panic!("pam token {t}"),
            }
        }
        (w, h, c)
    } else {
        let w = token().parse().unwrap();
        let h = token().parse().unwrap();
        let _max = token();
        (w, h, if magic == "P5" { 1 } else { 3 })
    };
    pos += 1;
    (w, h, c, d[pos..pos + w * h * c].to_vec())
}

/// Mean and max absolute difference.
fn diff(a: &[u8], b: &[u8]) -> (f64, u8) {
    assert_eq!(a.len(), b.len());
    let mut sum = 0u64;
    let mut max = 0u8;
    for (x, y) in a.iter().zip(b) {
        let d = x.abs_diff(*y);
        sum += u64::from(d);
        max = max.max(d);
    }
    (sum as f64 / a.len() as f64, max)
}

/// Box-downscales an interleaved reference image (partial edge boxes averaged).
fn box_down(w: usize, h: usize, c: usize, d: &[u8], s: usize) -> Vec<u8> {
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let mut out = vec![0u8; ow * oh * c];
    for oy in 0..oh {
        for ox in 0..ow {
            for k in 0..c {
                let (mut sum, mut n) = (0u32, 0u32);
                for y in oy * s..((oy + 1) * s).min(h) {
                    for x in ox * s..((ox + 1) * s).min(w) {
                        sum += u32::from(d[(y * w + x) * c + k]);
                        n += 1;
                    }
                }
                out[(oy * ow + ox) * c + k] = ((sum + n / 2) / n) as u8;
            }
        }
    }
    out
}

fn check(img: &Decoded, reference: &str, scale: usize, max_mean: f64) {
    let (w, h, c, r) = pnm(reference);
    let r = if scale > 1 {
        box_down(w, h, c, &r, scale)
    } else {
        r
    };
    assert_eq!(
        (img.width as usize, img.height as usize),
        (w.div_ceil(scale), h.div_ceil(scale)),
        "{reference} size at 1/{scale}"
    );
    assert_eq!(img.layout.channels(), c, "{reference} channels");
    let (mean, max) = diff(&img.data, &r);
    assert!(
        mean <= max_mean,
        "{reference} 1/{scale}: mean abs diff {mean:.2} (max {max}) exceeds {max_mean}"
    );
}

fn open(name: &str) -> AvifFile {
    AvifFile::open(data(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn probe_detects_avif() {
    assert!(probe(&data("rgb8_420.avif")[..32]));
    assert!(probe(&data("anim3.avif")[..32]));
    assert!(!probe(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"));
    assert!(!probe(b"short"));
}

#[test]
fn still_8bit_420() {
    let f = open("rgb8_420.avif");
    let i = f.info();
    assert_eq!(
        (i.width, i.height, i.bit_depth, i.chroma),
        (64, 48, 8, "4:2:0")
    );
    assert!(!i.has_alpha && !i.animated && i.hdr.is_none() && i.grid.is_none());
    assert_eq!((i.orientation, i.frame_count), (1, 1));
    for s in [1, 2, 4, 8] {
        for par in [false, true] {
            let img = f.decode(s as u32, par).unwrap();
            assert_eq!(img.layout, Layout::Rgb);
            assert_eq!(img.scale, s as u32);
            check(&img, "rgb8_420.ppm", s, if s == 1 { 3.0 } else { 2.5 });
        }
    }
}

/// The `colr` box's range flag wins over the AV1 header's, as libavif and libheif decode:
/// the same limited-range stream labelled full range in its `nclx` comes out with less
/// contrast (Microsoft's test files; fidelity round 4).
#[test]
fn nclx_range_wins_over_the_bitstream() {
    let plain = data("rgb8_420.avif");
    let at = plain
        .windows(8)
        .position(|w| w == b"colrnclx")
        .expect("an nclx box");
    assert_eq!(plain[at + 14] >> 7, 0, "limited range in the fixture");
    let mut full = plain.clone();
    full[at + 14] |= 0x80;
    let spread = |bytes: Vec<u8>| {
        let img = AvifFile::open(bytes).unwrap().decode(1, false).unwrap();
        img.data
            .iter()
            .map(|&v| (f64::from(v) - 128.0).abs())
            .sum::<f64>()
            / img.data.len() as f64
    };
    let (lim, fr) = (spread(plain), spread(full));
    // Limited range expands by 255/219 around mid-grey; read as full it does not.
    assert!(fr < lim * 0.92, "full {fr:.2} vs limited {lim:.2}");
}

/// A spatially layered item decodes to its top layer over the whole canvas, not the base layer
/// in a corner of black (Xiph's 2-layer file from the AV1 AVIF test files; needs the fidelity
/// corpus, `tools/corpus/fetch-fidelity.sh avif`, and passes without it).
#[test]
fn layered_items_show_the_top_layer() {
    let dir = std::env::var_os("SPECULAR_FIDELITY_CORPUS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| std::path::Path::new(&t).join("corpus/fidelity4"))
        });
    let Some(path) = dir
        .map(|d| d.join("avif/av1-avif/testFiles/Xiph/fruits_2layer_thumbsize.avif"))
        .filter(|p| p.is_file())
    else {
        eprintln!("layered AVIF sample not fetched; skipped");
        return;
    };
    let img = AvifFile::open(std::fs::read(path).unwrap())
        .unwrap()
        .decode(1, false)
        .unwrap();
    let (w, h) = (img.width as usize, img.height as usize);
    // The bottom-right quarter holds picture, not the black around a base layer.
    let mut sum = 0u64;
    for y in h / 2..h {
        for x in w / 2..w {
            sum += u64::from(img.data[(y * w + x) * 3 + 1]);
        }
    }
    let mean = sum as f64 / ((h - h / 2) * (w - w / 2)) as f64;
    assert!(mean > 30.0, "bottom-right mean {mean:.1}");
}

#[test]
fn still_10bit_and_444() {
    let f = open("rgb10_420.avif");
    assert_eq!(f.info().bit_depth, 10);
    check(&f.decode(1, true).unwrap(), "rgb10_420.ppm", 1, 3.0);
    check(&f.decode(4, true).unwrap(), "rgb10_420.ppm", 4, 2.5);
    let f = open("rgb8_444.avif");
    assert_eq!(f.info().chroma, "4:4:4");
    check(&f.decode(1, false).unwrap(), "rgb8_444.ppm", 1, 1.5);
    check(&f.decode(2, true).unwrap(), "rgb8_444.ppm", 2, 2.0);
}

#[test]
fn still_gray() {
    let f = open("gray.avif");
    assert_eq!(f.info().chroma, "4:0:0");
    let img = f.decode(1, true).unwrap();
    assert_eq!(img.layout, Layout::Gray);
    check(&img, "gray.pgm", 1, 1.0);
    check(&f.decode(8, true).unwrap(), "gray.pgm", 8, 1.0);
}

#[test]
fn still_alpha_12bit() {
    let f = open("alpha.avif");
    let i = f.info();
    assert!(i.has_alpha);
    assert_eq!(i.bit_depth, 12);
    let img = f.decode(1, true).unwrap();
    assert_eq!(img.layout, Layout::Rgba);
    check(&img, "alpha.pam", 1, 2.0);
    check(&f.decode(2, false).unwrap(), "alpha.pam", 2, 2.0);
}

#[test]
fn orientation_reported_not_applied() {
    let f = open("rot90_flip.avif");
    let i = f.info();
    // Stored pixels are untouched: size is the coded 64x48.
    assert_eq!((i.width, i.height), (64, 48));
    // Made by libheif from a JPEG with EXIF orientation 5 (transpose) -> irot + imir.
    assert_eq!(i.orientation, 5);
    check(&f.decode(1, true).unwrap(), "rot90_flip.ppm", 1, 3.0);
}

#[test]
fn pq_hdr_is_tone_mapped() {
    let f = open("pq10.avif");
    let hdr = f.info().hdr.expect("hdr");
    assert_eq!(hdr.transfer, HdrTransfer::Pq);
    let img = f.decode(1, true).unwrap();
    assert_eq!(img.layout, Layout::Rgb);
    let mean = img.data.iter().map(|&v| u64::from(v)).sum::<u64>() / img.data.len() as u64;
    assert!((20..240).contains(&mean), "mean {mean}");
}

#[test]
fn animation_frames() {
    let f = open("anim3.avif");
    let i = f.info();
    assert!(i.animated);
    assert_eq!(i.frame_count, 3);
    let mut frames = f.frames(1, true).unwrap();
    for round in 0..2 {
        for k in 1..=3 {
            let fr = frames.next().expect("frame").unwrap();
            assert_eq!(fr.duration_ms, 100, "round {round} frame {k}");
            check(&fr, &format!("anim3_{k}.ppm"), 1, 3.5);
        }
        assert!(frames.next().is_none());
        frames.rewind();
    }
    let mut frames = f.frames(2, false).unwrap();
    let fr = frames.next().unwrap().unwrap();
    check(&fr, "anim3_1.ppm", 2, 3.0);
}

#[test]
fn animation_with_alpha() {
    let f = open("anim_alpha.avif");
    assert!(f.info().has_alpha && f.info().animated);
    for (k, fr) in f.frames(1, true).unwrap().enumerate() {
        let fr = fr.unwrap();
        assert_eq!(fr.layout, Layout::Rgba);
        check(&fr, &format!("anim_alpha_{}.pam", k + 1), 1, 3.5);
    }
}

#[test]
fn grid_images() {
    // heif-enc --cut-tiles 32; oracle is heif-dec (libheif) output, which uses its own chroma
    // upsampling, hence the looser bound.
    let f = open("grid.avif");
    assert_eq!(f.info().grid, Some((3, 2)));
    assert_eq!((f.info().width, f.info().height), (96, 64));
    for s in [1, 2, 4, 8] {
        check(&f.decode(s as u32, s == 2).unwrap(), "grid.ppm", s, 3.0);
    }
    // 90x60 canvas over 3x2 tiles of 32x32: right/bottom tiles are cropped.
    let f = open("grid_odd.avif");
    assert_eq!((f.info().width, f.info().height), (90, 60));
    for s in [1, 2, 4] {
        check(&f.decode(s as u32, true).unwrap(), "grid_odd.ppm", s, 3.0);
    }
}

#[test]
fn cancel_is_honoured() {
    let f = open("rgb8_420.avif");
    assert_eq!(f.decode_with(1, true, &|| true), Err(Error::Cancelled));
}

#[test]
fn not_avif() {
    assert_eq!(
        AvifFile::open(b"GIF89a....".to_vec()).unwrap_err(),
        Error::NotAvif
    );
    assert!(AvifFile::open(Vec::new()).is_err());
}

const ALL: [&str; 11] = [
    "grid.avif",
    "grid_odd.avif",
    "rgb8_420.avif",
    "rgb10_420.avif",
    "rgb8_444.avif",
    "gray.avif",
    "alpha.avif",
    "rot90_flip.avif",
    "pq10.avif",
    "anim3.avif",
    "anim_alpha.avif",
];

fn exercise(bytes: Vec<u8>) {
    if let Ok(f) = AvifFile::open(bytes) {
        let _ = f.decode(1, false);
        let _ = f.decode(4, false);
        if let Ok(fr) = f.frames(2, false) {
            for r in fr.take(4) {
                let _ = r;
            }
        }
    }
}

#[test]
fn every_prefix_errors_or_decodes() {
    for name in ALL {
        let d = data(name);
        for n in 0..d.len() {
            exercise(d[..n].to_vec());
        }
    }
}

#[test]
fn corrupted_bytes_never_panic() {
    // Deterministic xorshift; flip bytes all over each file.
    let mut x = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for name in ALL {
        let d = data(name);
        for _ in 0..150 {
            let mut c = d.clone();
            for _ in 0..1 + rnd() % 4 {
                let i = (rnd() % c.len() as u64) as usize;
                c[i] ^= 1 << (rnd() % 8);
            }
            exercise(c);
        }
    }
}

// Chroma siting. The `siting*.avif` fixtures come from `tests/data/make_siting.py`: flat luma
// and a sharp chroma pattern, lossless, with `chroma_sample_position` patched in. The
// `*.libavif.ppm` references were made with libavif 1.3.0 (`avifRGBImage` with
// `chromaUpsampling = AVIF_CHROMA_UPSAMPLING_BILINEAR`, `avoidLibYUV = 1`). libavif ignores
// `chroma_sample_position`, so for the co-sited fixtures the test computes the expected
// output from the known planes instead.

/// The chroma planes `make_siting.py` encodes, at 8 bits: (Cb, Cr, width, height).
fn siting_planes(w: usize, h: usize) -> (Vec<i32>, Vec<i32>, usize, usize) {
    const COLOURS: [(i32, i32); 4] = [(68, 188), (188, 68), (68, 68), (128, 128)];
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut state: u64 = 12345;
    let (mut u, mut v) = (Vec::new(), Vec::new());
    for cy in 0..ch {
        for cx in 0..cw {
            let c = if cy < ch / 2 {
                COLOURS[(cx + cy) & 1]
            } else {
                state = (state * 1_103_515_245 + 12345) & 0x7FFF_FFFF;
                COLOURS[((state >> 16) & 3) as usize]
            };
            u.push(c.0);
            v.push(c.1);
        }
    }
    (u, v, cw, ch)
}

/// Bilinear 2× upsampling taps for output `x` from `n` samples: (sample, weight in quarters)
/// pairs. Centred chroma sits between two luma samples; co-sited chroma sits on the even one.
fn up_taps(x: usize, n: usize, cosited: bool) -> [(usize, i32); 2] {
    let c = x / 2;
    match (x.is_multiple_of(2), cosited) {
        (true, false) => [(c, 3), (c.saturating_sub(1), 1)],
        (false, false) => [(c, 3), ((c + 1).min(n - 1), 1)],
        (true, true) => [(c, 4), (c, 0)],
        (false, true) => [(c, 2), ((c + 1).min(n - 1), 2)],
    }
}

/// The expected RGB of a 4:2:0 siting fixture: bilinear upsampling at the given siting, then
/// BT.601 limited range with Y' = 126.
fn siting_model(w: usize, h: usize, cosited_x: bool, cosited_y: bool) -> Vec<u8> {
    let (u, v, cw, ch) = siting_planes(w, h);
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (mut cb, mut cr) = (0, 0);
            for (sy, wy) in up_taps(y, ch, cosited_y) {
                for (sx, wx) in up_taps(x, cw, cosited_x) {
                    cb += wy * wx * u[sy * cw + sx];
                    cr += wy * wx * v[sy * cw + sx];
                }
            }
            let yy = (126.0 - 16.0) / 219.0;
            let pb = (f64::from(cb) / 16.0 - 128.0) / 224.0;
            let pr = (f64::from(cr) / 16.0 - 128.0) / 224.0;
            let r = yy + 1.402 * pr;
            let b = yy + 1.772 * pb;
            let g = (yy - 0.299 * r - 0.114 * b) / 0.587;
            for c in [r, g, b] {
                out.push((c * 255.0).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}

#[test]
fn chroma_unknown_siting_matches_libavif() {
    for (name, w, h, chroma) in [
        ("siting420-unknown", 32, 24, "4:2:0"),
        ("siting420-odd-unknown", 33, 25, "4:2:0"),
        ("siting422", 32, 24, "4:2:2"),
        ("siting422-odd", 33, 25, "4:2:2"),
    ] {
        let f = open(&format!("{name}.avif"));
        let i = f.info();
        assert_eq!((i.width, i.height, i.chroma), (w, h, chroma));
        let (rw, rh, _, r) = pnm(&format!("{name}.libavif.ppm"));
        assert_eq!((rw, rh), (w as usize, h as usize));
        for par in [false, true] {
            let img = f.decode(1, par).unwrap();
            let (mean, max) = diff(&img.data, &r);
            assert!(max <= 1, "{name}: mean {mean:.3} max {max} against libavif");
        }
        if chroma == "4:2:0" {
            let img = f.decode(1, true).unwrap();
            let (mean, max) = diff(
                &img.data,
                &siting_model(w as usize, h as usize, false, false),
            );
            assert!(
                max <= 1,
                "{name}: mean {mean:.3} max {max} against the centred model"
            );
        }
    }
}

#[test]
fn chroma_sample_position_is_honoured() {
    let unknown = open("siting420-unknown.avif").decode(1, true).unwrap();
    for (name, w, h, cx, cy) in [
        ("siting420-vertical", 32, 24, true, false),
        ("siting420-colocated", 32, 24, true, true),
        ("siting420-odd-colocated", 33, 25, true, true),
        ("siting420-10bit-colocated", 32, 24, true, true),
    ] {
        let f = open(&format!("{name}.avif"));
        let model = siting_model(w, h, cx, cy);
        for par in [false, true] {
            let img = f.decode(1, par).unwrap();
            let (mean, max) = diff(&img.data, &model);
            assert!(
                max <= 1,
                "{name}: mean {mean:.3} max {max} against the co-sited model"
            );
            if w == 32 {
                // The siting visibly matters on this content.
                assert!(
                    diff(&img.data, &unknown.data).1 >= 30,
                    "{name}: same as unknown"
                );
            }
        }
        // Reduced decodes average the same upsampled chroma.
        for s in [2, 4, 8] {
            let img = f.decode(s as u32, true).unwrap();
            let want = box_down(w, h, 3, &model, s);
            let (mean, max) = diff(&img.data, &want);
            assert!(
                mean <= 0.6 && max <= 3,
                "{name} 1/{s}: mean {mean:.3} max {max}"
            );
        }
    }
}

// Grid seams. `grid420-*.avif` and `grid422.avif` come from `tests/data/make_grid.py`: 2x2
// grids of 64x64 lossless tiles with steep chroma ramps across the seams, each next to
// `<name>.single.avif`, the same samples as one item. Chroma upsampling must read across tile
// seams (libavif stitches the YUV planes first), so both decode identically. libavif 1.3.0
// (bilinear, no libyuv) also gives each grid and its single item the same pixels, equal to
// ours for the centred fixtures.
#[test]
fn grid_chroma_is_upsampled_across_seams() {
    for (name, w, h, chroma) in [
        ("grid420-unknown", 128, 128, "4:2:0"),
        ("grid420-colocated", 128, 128, "4:2:0"),
        ("grid420-cropped", 126, 118, "4:2:0"),
        ("grid422", 128, 128, "4:2:2"),
    ] {
        let grid = open(&format!("{name}.avif"));
        let single = open(&format!("{name}.single.avif"));
        let i = grid.info();
        assert_eq!(i.grid, Some((2, 2)), "{name}");
        assert_eq!((i.width, i.height, i.chroma), (w, h, chroma), "{name}");
        for s in [1, 2, 4, 8] {
            let want = single.decode(s, true).unwrap();
            for par in [false, true] {
                let got = grid.decode(s, par).unwrap();
                assert_eq!((got.width, got.height), (want.width, want.height));
                let (mean, max) = diff(&got.data, &want.data);
                assert!(
                    max == 0,
                    "{name} 1/{s}: mean {mean:.3} max {max} against the single item"
                );
            }
        }
    }
}

/// A collection of three images with 8-px thumbnails (`heif-enc -A -q 90 -t 8 -o
/// collection.avif p1.png p2.png p3.png`: 32×24 red, 16×16 green, 40×20 translucent blue):
/// the images are pages, their thumbnails and the third's alpha are not.
#[test]
fn collection_images_are_pages() {
    let d = data("collection.avif");
    let f = AvifFile::open(d.clone()).unwrap();
    assert_eq!((f.page_count(), f.page()), (3, 0));
    let mid = |img: &Decoded| {
        let c = match img.layout {
            Layout::Rgb => 3,
            Layout::Rgba => 4,
            l => panic!("{l:?}"),
        };
        let i = (img.height as usize / 2 * img.width as usize + img.width as usize / 2) * c;
        img.data[i..i + c].to_vec()
    };
    let near = |a: &[u8], b: &[u8]| a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= 12);
    let expect: [(u32, u32, bool, &[u8]); 3] = [
        (32, 24, false, &[220, 40, 40]),
        (16, 16, false, &[40, 200, 60]),
        (40, 20, true, &[30, 60, 220, 160]),
    ];
    for (n, (w, h, alpha, px)) in expect.into_iter().enumerate() {
        let p = AvifFile::open_page(d.clone(), n as u32).unwrap();
        assert_eq!((p.page(), p.page_count()), (n as u32, 3));
        let i = p.info();
        assert_eq!((i.width, i.height, i.has_alpha), (w, h, alpha), "page {n}");
        let img = p.decode(1, false).unwrap();
        assert_eq!((img.width, img.height), (w, h));
        assert!(near(&mid(&img), px), "page {n}: {:?}", mid(&img));
    }
    assert!(AvifFile::open_page(d, 3).is_err());
    // Single images and sequences have one page.
    assert_eq!(open("alpha.avif").page_count(), 1);
    assert_eq!(open("grid.avif").page_count(), 1);
    assert_eq!(open("anim3.avif").page_count(), 1);
    assert!(AvifFile::open_page(data("anim3.avif"), 1).is_err());
}

/// Layered items (AVIF 1.1 `a1op` and HEIF `lsel`), on the AV1 AVIF test files when the
/// fidelity corpus is there (`tools/corpus/fetch-fidelity.sh avif`; skipped otherwise):
/// Apple's `animals_00_multilayer_a1op_lsel.avif` holds the 2048x1536 primary (top layer) and
/// two items that select layer 0 (1024x768). Each lower-layer page is the primary picture at
/// half size; before fidelity round 6 the decoder showed the top layer whatever `lsel` said.
#[test]
fn lower_layers_are_selected() {
    let Some(t) = std::env::var_os("CARGO_TARGET_DIR") else {
        return;
    };
    let p = std::path::Path::new(&t).join(
        "corpus/fidelity4/avif/av1-avif/testFiles/Apple/multilayer_examples/\
         animals_00_multilayer_a1op_lsel.avif",
    );
    let Ok(bytes) = std::fs::read(&p) else {
        eprintln!("no AV1 AVIF test files; skipped");
        return;
    };
    let f = AvifFile::open(bytes.clone()).unwrap();
    assert_eq!(f.page_count(), 3);
    let top = f.decode(2, false).unwrap();
    assert_eq!((top.width, top.height), (1024, 768));
    for page in [1, 2] {
        let g = AvifFile::open_page(bytes.clone(), page).unwrap();
        let low = g.decode(1, false).unwrap();
        assert_eq!((low.width, low.height, low.layout), (1024, 768, top.layout));
        let d: f64 = low
            .data
            .iter()
            .zip(&top.data)
            .map(|(&a, &b)| f64::from(a.abs_diff(b)))
            .sum::<f64>()
            / low.data.len() as f64;
        assert!(
            d < 10.0,
            "page {page}: mean {d:.2} off the top layer at half size"
        );
    }
}
