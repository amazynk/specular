//! OpenEXR, Radiance RGBE and PFM through the registry: tiny hand-written files (uncompressed
//! scanline / tiled-mipmap EXR, flat and run-length RGBE, PFM) decoded to `Rgba16F` rasters;
//! the streamed levels and regions against the full decode box-filtered here.

use std::sync::Arc;

use specular_core::hdr::{self, Primaries};
use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeRequest, Format, Rect, Registry, Session};
use specular_exr as exr;
use specular_radiance as rad;

// ---- writers ----

fn attr(out: &mut Vec<u8>, name: &str, ty: &str, value: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out.extend_from_slice(ty.as_bytes());
    out.push(0);
    out.extend_from_slice(&(value.len() as i32).to_le_bytes());
    out.extend_from_slice(value);
}

fn le_f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn box2i(w: u32, h: u32) -> Vec<u8> {
    [0i32, 0, w as i32 - 1, h as i32 - 1]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

/// Channel names in file (alphabetical) order.
fn names(alpha: bool) -> Vec<&'static str> {
    if alpha {
        vec!["A", "B", "G", "R"]
    } else {
        vec!["B", "G", "R"]
    }
}

/// Header up to (and including) the end-of-header byte.
fn exr_header(
    w: u32,
    h: u32,
    alpha: bool,
    half: bool,
    chroma: Option<[f32; 8]>,
    tiles: Option<(u32, u32)>,
) -> Vec<u8> {
    let mut out = vec![0x76, 0x2f, 0x31, 0x01];
    let flags: u32 = 2 | if tiles.is_some() { 0x200 } else { 0 };
    out.extend_from_slice(&flags.to_le_bytes());
    let mut ch = Vec::new();
    for n in names(alpha) {
        ch.extend_from_slice(n.as_bytes());
        ch.push(0);
        ch.extend_from_slice(&(if half { 1i32 } else { 2 }).to_le_bytes());
        ch.extend_from_slice(&[0, 0, 0, 0]);
        ch.extend_from_slice(&1i32.to_le_bytes());
        ch.extend_from_slice(&1i32.to_le_bytes());
    }
    ch.push(0);
    attr(&mut out, "channels", "chlist", &ch);
    attr(&mut out, "compression", "compression", &[0]);
    attr(&mut out, "dataWindow", "box2i", &box2i(w, h));
    attr(&mut out, "displayWindow", "box2i", &box2i(w, h));
    attr(&mut out, "lineOrder", "lineOrder", &[0]);
    attr(&mut out, "pixelAspectRatio", "float", &1f32.to_le_bytes());
    attr(&mut out, "screenWindowCenter", "v2f", &le_f32s(&[0.0, 0.0]));
    attr(&mut out, "screenWindowWidth", "float", &1f32.to_le_bytes());
    if let Some(c) = chroma {
        attr(&mut out, "chromaticities", "chromaticities", &le_f32s(&c));
    }
    if let Some((tx, ty)) = tiles {
        let mut t = Vec::new();
        t.extend_from_slice(&tx.to_le_bytes());
        t.extend_from_slice(&ty.to_le_bytes());
        t.push(1); // MIPMAP_LEVELS, round down
        attr(&mut out, "tiles", "tiledesc", &t);
    }
    out.push(0);
    out
}

/// Samples of one channel row: `px(x, y)` gives RGBA.
fn row_bytes(
    out: &mut Vec<u8>,
    xs: std::ops::Range<u32>,
    y: u32,
    c: usize,
    half: bool,
    px: &dyn Fn(u32, u32) -> [f32; 4],
) {
    for x in xs {
        let v = px(x, y)[c];
        if half {
            out.extend_from_slice(&hdr::f32_to_f16(v).to_le_bytes());
        } else {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

fn slot(name: &str) -> usize {
    match name {
        "R" => 0,
        "G" => 1,
        "B" => 2,
        _ => 3,
    }
}

/// Uncompressed scanline EXR (one line per chunk).
fn exr_scanline(
    w: u32,
    h: u32,
    alpha: bool,
    half: bool,
    chroma: Option<[f32; 8]>,
    px: impl Fn(u32, u32) -> [f32; 4],
) -> Vec<u8> {
    let mut out = exr_header(w, h, alpha, half, chroma, None);
    let table = out.len();
    out.resize(table + 8 * h as usize, 0);
    for y in 0..h {
        let at = out.len() as u64;
        out[table + 8 * y as usize..][..8].copy_from_slice(&at.to_le_bytes());
        let mut data = Vec::new();
        for n in names(alpha) {
            row_bytes(&mut data, 0..w, y, slot(n), half, &px);
        }
        out.extend_from_slice(&(y as i32).to_le_bytes());
        out.extend_from_slice(&(data.len() as i32).to_le_bytes());
        out.extend_from_slice(&data);
    }
    out
}

/// Uncompressed tiled half-float RGB EXR with mip levels (rounded down); `px(level, x, y)`.
fn exr_mipmap(w: u32, h: u32, tile: u32, px: impl Fn(u32, u32, u32) -> [f32; 4]) -> Vec<u8> {
    let mut out = exr_header(w, h, false, true, None, Some((tile, tile)));
    let levels = 32 - w.max(h).leading_zeros();
    let mut chunks = Vec::new();
    for l in 0..levels {
        let (lw, lh) = ((w >> l).max(1), (h >> l).max(1));
        for ty in 0..lh.div_ceil(tile) {
            for tx in 0..lw.div_ceil(tile) {
                let mut data = Vec::new();
                let xs = tx * tile..((tx + 1) * tile).min(lw);
                for y in ty * tile..((ty + 1) * tile).min(lh) {
                    for n in names(false) {
                        row_bytes(&mut data, xs.clone(), y, slot(n), true, &|x, y| px(l, x, y));
                    }
                }
                let mut c = Vec::new();
                for v in [tx, ty, l, l] {
                    c.extend_from_slice(&(v as i32).to_le_bytes());
                }
                c.extend_from_slice(&(data.len() as i32).to_le_bytes());
                c.extend_from_slice(&data);
                chunks.push(c);
            }
        }
    }
    let mut at = (out.len() + 8 * chunks.len()) as u64;
    for c in &chunks {
        out.extend_from_slice(&at.to_le_bytes());
        at += c.len() as u64;
    }
    for c in chunks {
        out.extend_from_slice(&c);
    }
    out
}

fn rgbe([r, g, b]: [f32; 3]) -> [u8; 4] {
    let m = r.max(g).max(b);
    if m < 1e-32 {
        return [0; 4];
    }
    let e = m.log2().floor() as i32 + 1;
    let s = 256.0 / 2f32.powi(e);
    [(r * s) as u8, (g * s) as u8, (b * s) as u8, (e + 128) as u8]
}

/// Flat (non-RLE) Radiance file; `header` lines go before the blank line.
fn radiance(w: u32, h: u32, header: &str, px: impl Fn(u32, u32) -> [f32; 3]) -> Vec<u8> {
    let mut out = format!("#?RADIANCE\n{header}\n\n-Y {h} +X {w}\n").into_bytes();
    for y in 0..h {
        for x in 0..w {
            out.extend_from_slice(&rgbe(px(x, y)));
        }
    }
    out
}

/// Little-endian RGB PFM (stored bottom-up).
fn pfm(w: u32, h: u32, px: impl Fn(u32, u32) -> [f32; 3]) -> Vec<u8> {
    let mut out = format!("PF\n{w} {h}\n-1.0\n").into_bytes();
    for y in (0..h).rev() {
        for x in 0..w {
            out.extend_from_slice(&le_f32s(&px(x, y)));
        }
    }
    out
}

// ---- helpers ----

fn open(name: &str, data: Vec<u8>) -> Arc<dyn Session> {
    Registry::new()
        .open(Arc::new(Source::from_vec(name, data)))
        .unwrap()
}

fn decode(s: &dyn Session, scale: u32) -> Raster {
    s.decode(&DecodeRequest::full(scale), &CancelToken::new())
        .unwrap()
}

fn px(r: &Raster, x: u32, y: u32) -> [f32; 4] {
    let p = &r.planes[0];
    let o = y as usize * p.stride + x as usize * 8;
    std::array::from_fn(|i| {
        hdr::f16_to_f32(u16::from_ne_bytes([
            p.data[o + 2 * i],
            p.data[o + 2 * i + 1],
        ]))
    })
}

fn near(a: [f32; 4], b: [f32; 4], rel: f32) -> bool {
    a.iter()
        .zip(&b)
        .all(|(x, y)| (x - y).abs() <= rel * y.abs().max(0.01))
}

fn grad(x: u32, y: u32) -> [f32; 4] {
    [x as f32 * 0.25, y as f32 * 0.5, 0.125, 1.0]
}

// ---- tests ----

#[test]
fn exr_half_rgba_scanline() {
    let (w, h) = (21, 13);
    let s = open(
        "a.exr",
        exr_scanline(w, h, true, true, None, |x, y| {
            [x as f32 * 0.25, y as f32 * 0.5, 0.125, (x % 2) as f32]
        }),
    );
    let i = s.info();
    assert_eq!((i.format, i.width, i.height), (Format::Exr, w, h));
    assert!(i.has_alpha);
    assert_eq!(i.bit_depth, 16);
    let r = decode(&*s, 1);
    assert_eq!(r.layout, PixelLayout::Rgba16F(Primaries::Bt709));
    assert_eq!((r.width, r.height), (w, h));
    for (x, y) in [(0, 0), (7, 3), (20, 12)] {
        assert_eq!(
            px(&r, x, y),
            [x as f32 * 0.25, y as f32 * 0.5, 0.125, (x % 2) as f32]
        );
    }
    // 1/2 without mip levels: box-filtered, alpha-weighted, ceil-sized.
    let r2 = decode(&*s, 2);
    assert_eq!((r2.width, r2.height, r2.scale), (11, 7, 2));
    // Pixels (2,0) [α 0] and (3,0) [α 1], rows 0 and 1: colour is the opaque column's.
    assert!(
        near(px(&r2, 1, 0), [0.75, 0.25, 0.125, 0.5], 0.01),
        "{:?}",
        px(&r2, 1, 0)
    );
}

#[test]
fn exr_float_chromaticities() {
    let p3 = [0.68, 0.32, 0.265, 0.69, 0.15, 0.06, 0.3127, 0.3290];
    let s = open("p3.exr", exr_scanline(8, 4, false, false, Some(p3), grad));
    assert_eq!(s.info().bit_depth, 32);
    let r = decode(&*s, 1);
    assert_eq!(r.layout, PixelLayout::Rgba16F(Primaries::DisplayP3));
    assert!(near(px(&r, 5, 3), grad(5, 3), 1e-3));

    // Red and green swapped relative to BT.709: converted on the CPU, tagged BT.709.
    let swapped = [0.30, 0.60, 0.64, 0.33, 0.15, 0.06, 0.3127, 0.3290];
    let s = open(
        "odd.exr",
        exr_scanline(8, 4, false, false, Some(swapped), grad),
    );
    assert!(
        s.info().coding.contains("custom chromaticities"),
        "{}",
        s.info().coding
    );
    let r = decode(&*s, 1);
    assert_eq!(r.layout, PixelLayout::Rgba16F(Primaries::Bt709));
    let [a, b, c, _] = grad(5, 3);
    assert!(
        near(px(&r, 5, 3), [b, a, c, 1.0], 2e-3),
        "{:?}",
        px(&r, 5, 3)
    );
}

#[test]
fn exr_mip_levels_serve_scales() {
    let (w, h) = (37, 22);
    // Each level is a flat colour 1 + level, so it is visible which level was used.
    let s = open(
        "mip.exr",
        exr_mipmap(w, h, 16, |l, _, _| [1.0 + l as f32, 0.5, 0.25, 1.0]),
    );
    assert!(s.info().coding.contains("mipmap"), "{}", s.info().coding);
    assert!(s.info().coding.contains("tiled"));
    for (scale, level) in [(1u32, 0u32), (2, 1), (4, 2), (8, 3)] {
        let r = decode(&*s, scale);
        // Exact viewer level size even though mip levels round down.
        assert_eq!(
            (r.width, r.height, r.scale),
            (w.div_ceil(scale), h.div_ceil(scale), scale)
        );
        let last = px(&r, r.width - 1, r.height - 1);
        assert_eq!(last, [1.0 + level as f32, 0.5, 0.25, 1.0], "scale {scale}");
    }
    // Small file: no separate preview.
    assert!(s.decode_preview().is_none());
}

#[test]
fn radiance_rgb_and_xyz() {
    let s = open(
        "a.hdr",
        radiance(9, 5, "FORMAT=32-bit_rle_rgbe\nEXPOSURE=2", |x, y| {
            [x as f32 * 10.0, y as f32 + 0.5, 8.0]
        }),
    );
    let i = s.info();
    assert_eq!((i.format, i.width, i.height), (Format::Hdr, 9, 5));
    assert!(i.coding.contains("exposure 2"), "{}", i.coding);
    let r = decode(&*s, 1);
    assert_eq!(r.layout, PixelLayout::Rgba16F(Primaries::Bt709));
    assert!(
        near(px(&r, 8, 4), [80.0, 4.5, 8.0, 1.0], 0.02),
        "{:?}",
        px(&r, 8, 4)
    );

    // XYZE: D65 white (Y = 2) → RGB (2, 2, 2).
    let white = [
        0.3127 / 0.3290 * 2.0,
        2.0,
        (1.0 - 0.3127 - 0.3290) / 0.3290 * 2.0,
    ];
    let s = open(
        "x.hdr",
        radiance(4, 2, "FORMAT=32-bit_rle_xyze", |_, _| white),
    );
    let r = decode(&*s, 1);
    assert!(
        near(px(&r, 1, 1), [2.0, 2.0, 2.0, 1.0], 0.02),
        "{:?}",
        px(&r, 1, 1)
    );
}

#[test]
fn pfm_is_flipped_and_linear() {
    let s = open("a.pfm", pfm(6, 3, |x, y| [x as f32, y as f32 * 3.0, 100.0]));
    assert_eq!(s.info().format, Format::Pfm);
    assert!(s.info().coding.starts_with("PFM"));
    let r = decode(&*s, 1);
    assert_eq!(px(&r, 5, 0), [5.0, 0.0, 100.0, 1.0]);
    assert_eq!(px(&r, 2, 2), [2.0, 6.0, 100.0, 1.0]);
    let r4 = decode(&*s, 4);
    assert_eq!((r4.width, r4.height), (2, 1));
}

/// Grey (`Pf`) and colour (`PF`), big-endian (positive scale) and little-endian (negative):
/// rows are stored bottom-up, and the scale's magnitude does not change the values.
#[test]
fn pfm_grey_colour_both_byte_orders() {
    for (grey, big) in [(true, true), (true, false), (false, true), (false, false)] {
        let (w, h) = (5u32, 3u32);
        let v = |x: u32, y: u32, c: u32| x as f32 * 0.5 + y as f32 * 10.0 + c as f32 * 100.0;
        let ch = if grey { 1 } else { 3 };
        let mut f = format!(
            "{}\n{w} {h}\n{}\n",
            if grey { "Pf" } else { "PF" },
            if big { "4.0" } else { "-0.5" }
        )
        .into_bytes();
        for y in (0..h).rev() {
            for x in 0..w {
                for c in 0..ch {
                    let b = v(x, y, c);
                    f.extend_from_slice(&if big {
                        b.to_be_bytes()
                    } else {
                        b.to_le_bytes()
                    });
                }
            }
        }
        let s = open("g.pfm", f);
        assert_eq!(s.info().format, Format::Pfm);
        assert_eq!(
            s.colour_model(),
            Some(if grey { "Greyscale" } else { "RGB" })
        );
        assert!(
            s.info()
                .coding
                .contains(if big { "big-endian" } else { "little-endian" }),
            "{}",
            s.info().coding
        );
        let r = decode(&*s, 1);
        for (x, y) in [(0, 0), (4, 0), (2, 1), (4, 2)] {
            let want = if grey {
                [v(x, y, 0); 3]
            } else {
                [v(x, y, 0), v(x, y, 1), v(x, y, 2)]
            };
            assert!(
                near(px(&r, x, y), [want[0], want[1], want[2], 1.0], 0.01),
                "grey {grey} big {big} ({x},{y}): {:?} vs {want:?}",
                px(&r, x, y)
            );
        }
    }
    // `.pfm` is PFM's alone; Radiance keeps `.hdr`.
    assert!(Registry::new().supports_extension(std::path::Path::new("a.pfm")));
}

/// The corpus of `tools/corpus/fetch-raster-pfm.sh`: every PFM against its expected floats
/// (ImageMagick's reading, or the generator's values where ImageMagick would clamp), and
/// against ImageMagick's clamped reading where both exist. Half-float precision.
#[test]
#[ignore = "needs SPECULAR_RASTER_CORPUS_PFM (tools/corpus/fetch-raster-pfm.sh)"]
fn pfm_corpus_matches_imagemagick() {
    let Some(dir) = std::env::var_os("SPECULAR_RASTER_CORPUS_PFM") else {
        eprintln!("SPECULAR_RASTER_CORPUS_PFM not set");
        return;
    };
    let floats = |p: &std::path::Path| -> Option<Vec<f32>> {
        let b = std::fs::read(p).ok()?;
        Some(
            b.chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        )
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pfm"))
        .collect();
    files.sort();
    let (mut pass, mut fail) = (0, 0);
    for f in &files {
        let s = open(
            f.file_name().unwrap().to_str().unwrap(),
            std::fs::read(f).unwrap(),
        );
        let r = decode(&*s, 1);
        let mut bad = 0usize;
        for (ext, clamp) in [("rgbf", false), ("im.rgbf", true)] {
            let Some(want) = floats(&f.with_extension(ext)) else {
                continue;
            };
            assert_eq!(want.len(), (r.width * r.height * 3) as usize, "{f:?}");
            for y in 0..r.height {
                for x in 0..r.width {
                    let got = px(&r, x, y);
                    let i = ((y * r.width + x) * 3) as usize;
                    for c in 0..3 {
                        let g = if clamp {
                            got[c].clamp(0.0, 1.0)
                        } else {
                            got[c]
                        };
                        let w = want[i + c];
                        if (g - w).abs() > 1e-3 * w.abs().max(1.0) {
                            bad += 1;
                        }
                    }
                }
            }
        }
        if bad == 0 {
            pass += 1;
        } else {
            fail += 1;
            eprintln!("{}: {bad} samples differ", f.display());
        }
    }
    eprintln!(
        "PFM corpus: {pass} of {} files match, {fail} mismatch",
        files.len()
    );
    assert_eq!(fail, 0);
}

#[test]
fn damaged_files_do_not_panic() {
    let files = [
        exr_scanline(16, 8, true, true, None, grad),
        exr_mipmap(20, 12, 8, |l, x, _| [l as f32, x as f32, 0.0, 1.0]),
        radiance(10, 6, "FORMAT=32-bit_rle_rgbe", |x, _| [x as f32, 1.0, 2.0]),
        pfm(7, 5, |x, y| [x as f32, y as f32, 1.0]),
    ];
    let reg = Registry::new();
    for (k, f) in files.iter().enumerate() {
        let name = ["a.exr", "b.exr", "c.hdr", "d.pfm"][k];
        for n in (0..f.len()).step_by(7) {
            if let Ok(s) = reg.open(Arc::new(Source::from_vec(name, f[..n].to_vec()))) {
                for scale in [1, 2, 8] {
                    let _ = s.decode(&DecodeRequest::full(scale), &CancelToken::new());
                }
            }
        }
    }
}

#[path = "common/trim.rs"]
mod trim;

/// Trimming drops the Radiance row index (rebuilt by the next request). OpenEXR streams every
/// request from the file and keeps nothing.
#[test]
fn levels_trimmed() {
    let exr = open(
        "a.exr",
        exr_scanline(21, 13, true, true, None, |x, y| {
            [x as f32 * 0.25, y as f32 * 0.5, 0.125, 1.0]
        }),
    );
    decode(&*exr, 2);
    assert_eq!(exr.memory_bytes(), 0);
    assert_eq!(exr.trim_memory(), 0);
    let hdr = open(
        "a.hdr",
        radiance(9, 5, "FORMAT=32-bit_rle_rgbe", |x, y| {
            [x as f32, y as f32 + 0.5, 8.0]
        }),
    );
    trim::assert_trims(&*hdr, &DecodeRequest::full(2));
}

// ---- streamed levels and regions against a reference ----

/// One RLE component of a new-style Radiance scanline: runs of 3 or more, literals between.
fn rle(out: &mut Vec<u8>, v: &[u8]) {
    let mut i = 0;
    while i < v.len() {
        let run = v[i..].iter().take(127).take_while(|&&b| b == v[i]).count();
        if run >= 3 {
            out.extend_from_slice(&[128 + run as u8, v[i]]);
            i += run;
            continue;
        }
        let mut j = i;
        while j < v.len() && j - i < 128 {
            if j + 2 < v.len() && v[j] == v[j + 1] && v[j] == v[j + 2] {
                break;
            }
            j += 1;
        }
        out.push((j - i) as u8);
        out.extend_from_slice(&v[i..j]);
        i = j;
    }
}

/// New-style run-length Radiance with the given resolution line (`-Y h +X w` or any of the
/// eight orders); `px(i, line)` is the `i`th pixel of the `line`th stored scanline.
fn radiance_rle(
    resolution: &str,
    len: u32,
    lines: u32,
    px: impl Fn(u32, u32) -> [f32; 3],
) -> Vec<u8> {
    let mut out = format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n{resolution}\n").into_bytes();
    for l in 0..lines {
        let p: Vec<[u8; 4]> = (0..len).map(|i| rgbe(px(i, l))).collect();
        out.extend_from_slice(&[2, 2, (len >> 8) as u8, len as u8]);
        for c in 0..4 {
            let v: Vec<u8> = p.iter().map(|p| p[c]).collect();
            rle(&mut out, &v);
        }
    }
    out
}

/// What the viewer shows of the `w`×`h` RGBA `f32` image `data` (as its decoder gives it) at
/// 1/`f`: each value made finite and rounded to half (alpha clamped to 0..1 when
/// `clamp_alpha`, colour divided by partial alpha when `unpremultiply`), then boxes averaged
/// weighted by alpha. Pixel values (from half) of the level, row by row.
fn reference(
    data: &[f32],
    (w, h): (u32, u32),
    f: u32,
    matrix: Option<[[f32; 3]; 3]>,
    clamp_alpha: bool,
    unpremultiply: bool,
) -> Vec<[f32; 4]> {
    let r16 = |v: f32| hdr::f16_to_f32(hdr::f32_to_f16(v));
    let fin = |v: f32| {
        if v.is_nan() {
            0.0
        } else {
            v.clamp(-65504.0, 65504.0)
        }
    };
    let shown: Vec<[f32; 4]> = data
        .chunks_exact(4)
        .map(|p| {
            let mut c = [p[0], p[1], p[2]];
            if let Some(m) = matrix {
                c = hdr::mat_mul(m, c);
            }
            let a = if !clamp_alpha {
                fin(p[3])
            } else if p[3].is_nan() {
                1.0
            } else {
                p[3].clamp(0.0, 1.0)
            };
            let a = r16(a);
            let mut c = c.map(|v| r16(fin(v)));
            if unpremultiply && a > 0.0 && a < 1.0 {
                c = c.map(|v| r16(fin(v / a)));
            }
            [c[0], c[1], c[2], a]
        })
        .collect();
    let (lw, lh) = (w.div_ceil(f), h.div_ceil(f));
    let mut out = Vec::new();
    for oy in 0..lh {
        for ox in 0..lw {
            let (mut s, mut sa, mut n) = ([0f64; 3], 0f64, 0f64);
            for y in oy * f..(oy * f + f).min(h) {
                for x in ox * f..(ox * f + f).min(w) {
                    let p = shown[(y * w + x) as usize];
                    let a = f64::from(p[3]);
                    for c in 0..3 {
                        s[c] += f64::from(p[c]) * a;
                    }
                    sa += a;
                    n += 1.0;
                }
            }
            let c = |i: usize| {
                if sa > 0.0 {
                    r16((s[i] / sa) as f32)
                } else {
                    0.0
                }
            };
            if f == 1 {
                out.push(shown[(oy * w + ox) as usize]);
            } else {
                out.push([c(0), c(1), c(2), r16((sa / n) as f32)]);
            }
        }
    }
    out
}

/// Every level of `s` equals `want(f)`, and regions of it are the same pixels.
fn check_levels(name: &str, s: &dyn Session, want: impl Fn(u32) -> Vec<[f32; 4]>) {
    let (w, h) = (s.info().width, s.info().height);
    for f in [1, 2, 4, 8] {
        let r = decode(s, f);
        let (lw, lh) = (w.div_ceil(f), h.div_ceil(f));
        assert_eq!((r.width, r.height, r.scale), (lw, lh, f), "{name}");
        let want = want(f);
        for y in 0..lh {
            for x in 0..lw {
                let (g, e) = (px(&r, x, y), want[(y * lw + x) as usize]);
                assert!(g == e, "{name} 1/{f} ({x}, {y}): {g:?} want {e:?}");
            }
        }
        for (x, y, rw, rh) in [
            (0, 0, 1, 1),
            (1, 1, 2, 3),
            (lw - 1, lh - 1, 1, 1),
            (lw / 3, lh / 2, lw - lw / 3, lh - lh / 2),
            (0, 0, lw + 5, lh + 5),
        ] {
            let rect = Rect { x, y, w: rw, h: rh };
            let req = DecodeRequest {
                region: Some(rect),
                ..DecodeRequest::full(f)
            };
            let g = s.decode(&req, &CancelToken::new()).unwrap();
            let (ox, oy) = (g.origin.0, g.origin.1);
            assert!(
                ox <= x && oy <= y && ox + g.width >= (x + rw).min(lw),
                "{name}"
            );
            for yy in 0..g.height {
                for xx in 0..g.width {
                    let e = want[((oy + yy) * lw + ox + xx) as usize];
                    assert!(
                        px(&g, xx, yy) == e,
                        "{name} 1/{f} region {rect:?} ({xx}, {yy})"
                    );
                }
            }
        }
    }
}

fn radiance_reference(file: &[u8], f: u32, matrix: Option<[[f32; 3]; 3]>) -> Vec<[f32; 4]> {
    let opts = rad::DecodeOptions {
        channels: rad::OutputChannels::Rgba,
        ..Default::default()
    };
    let img = rad::decode(file, &opts).unwrap();
    reference(&img.data, (img.width, img.height), f, matrix, true, false)
}

#[test]
fn radiance_levels_and_regions_stream() {
    // Runs and literals, exponents that differ across the image, a few zero pixels.
    let px = |i: u32, l: u32| {
        if (i + l).is_multiple_of(17) {
            return [0.0; 3];
        }
        [
            (i / 4) as f32 * 0.37 + 0.01,
            l as f32 * 13.0 + 0.5,
            ((i * l) % 5) as f32 * 0.003,
        ]
    };
    let mut files = Vec::new();
    for res in ["-Y 13 +X 37", "+Y 13 +X 37", "-Y 13 -X 37", "+Y 13 -X 37"] {
        files.push((res.to_string(), radiance_rle(res, 37, 13, px)));
    }
    // x-major (decoded whole, not streamed) and flat.
    files.push(("x-major".into(), radiance_rle("+X 13 -Y 37", 37, 13, px)));
    files.push(("flat".into(), radiance(11, 6, "FORMAT=32-bit_rle_rgbe", px)));
    for (name, file) in &files {
        let s = open("a.hdr", file.clone());
        assert_eq!(s.info().fast_regions, name != "x-major", "{name}");
        if s.info().fast_regions {
            assert!(!s.regions_ready());
            s.prepare_regions(&CancelToken::new()).unwrap();
            assert!(s.regions_ready());
        }
        check_levels(name, &*s, |f| radiance_reference(file, f, None));
    }
    // XYZE: converted per pixel before the boxes are averaged.
    let file = radiance_rle("-Y 9 +X 20", 20, 9, |i, l| {
        [i as f32 * 0.1, l as f32 + 1.0, 0.5]
    });
    let xyz = String::from_utf8_lossy(&file).replace("rle_rgbe", "rle_xyze");
    let file = xyz.into_bytes();
    let s = open("x.hdr", file.clone());
    check_levels("xyze", &*s, |f| {
        radiance_reference(&file, f, Some(hdr::xyz_to_bt709()))
    });
}

#[test]
fn truncated_radiance_streams_what_is_there() {
    let file = radiance_rle("-Y 16 +X 40", 40, 16, |i, l| [i as f32, l as f32, 1.0]);
    for cut in [file.len() / 2, file.len() - 3] {
        let part = file[..cut].to_vec();
        let s = open("t.hdr", part.clone());
        for f in [1, 2, 4] {
            assert!(decode(&*s, f).truncated);
        }
        check_levels("truncated", &*s, |f| radiance_reference(&part, f, None));
    }
}

#[test]
fn pfm_levels_and_regions_stream() {
    let rgb = pfm(23, 10, |x, y| [x as f32 * 1e3, -(y as f32), f32::NAN]);
    let grey = {
        let mut f = b"Pf\n9 5\n1.0\n".to_vec();
        for i in 0..45u32 {
            f.extend_from_slice(&(i as f32 * 0.5).to_be_bytes());
        }
        f
    };
    let rgba = {
        let mut f = b"PF4\n13 7\n-1.0\n".to_vec();
        for i in 0..13 * 7u32 {
            let a = [0.0, 0.5, 1.0, 2.0][(i % 4) as usize];
            f.extend_from_slice(&le_f32s(&[i as f32, 1.0, 2.0, a]));
        }
        f
    };
    // A short file: the rest is black.
    let short = rgb[..rgb.len() - 100].to_vec();
    for (name, file) in [
        ("rgb", rgb),
        ("grey", grey),
        ("rgba", rgba),
        ("short", short),
    ] {
        let s = open("a.pfm", file.clone());
        assert!(s.info().fast_regions);
        check_levels(name, &*s, |f| radiance_reference(&file, f, None));
    }
}

fn exr_reference(file: &[u8], f: u32, matrix: Option<[[f32; 3]; 3]>) -> Vec<[f32; 4]> {
    let img = exr::decode(file, &exr::DecodeOptions::default()).unwrap();
    let exr::Pixels::F32(px) = img.pixels else {
        panic!()
    };
    let alpha = exr::read_info(file).unwrap().parts[0]
        .channels
        .iter()
        .any(|c| c.name == "A");
    reference(
        &px,
        (img.width, img.height),
        f,
        matrix,
        matrix.is_some(),
        alpha,
    )
}

#[test]
fn exr_levels_and_regions_stream() {
    let px = |x: u32, y: u32| {
        let a = [0.0, 0.25, 1.0, 0.5, 1.0][((x + y) % 5) as usize];
        [x as f32 * 0.3 * a, y as f32 * 7.0 * a, 1e5, a]
    };
    let s = open("a.exr", exr_scanline(29, 19, true, true, None, px));
    assert!(s.info().fast_regions);
    let file = exr_scanline(29, 19, true, true, None, px);
    check_levels("half rgba", &*s, |f| exr_reference(&file, f, None));
    let file = exr_scanline(29, 19, true, false, None, px);
    let s = open("b.exr", file.clone());
    check_levels("float rgba", &*s, |f| exr_reference(&file, f, None));
    // Custom chromaticities: the matrix applies per pixel, alpha clamped first.
    let swapped = [0.30, 0.60, 0.64, 0.33, 0.15, 0.06, 0.3127, 0.3290];
    let file = exr_scanline(29, 19, false, false, Some(swapped), grad);
    let s = open("c.exr", file.clone());
    let m =
        hdr::chromaticities_to_bt709([[0.30, 0.60], [0.64, 0.33], [0.15, 0.06], [0.3127, 0.3290]])
            .unwrap();
    check_levels("matrix", &*s, |f| exr_reference(&file, f, Some(m)));
}

#[test]
fn exr_tiled_regions_stream_from_mips() {
    let (w, h) = (75, 41);
    let file = exr_mipmap(w, h, 16, |l, x, y| {
        [(x + 3 * y) as f32 + l as f32 * 100.0, 0.5, 0.25, 1.0]
    });
    let s = open("mip.exr", file.clone());
    assert!(s.info().fast_regions);
    assert!(s.scaled_decode_cheap());
    // Full size: regions are the full decode's pixels.
    check_levels_at_1(&*s, &exr_reference(&file, 1, None));
}

fn check_levels_at_1(s: &dyn Session, want: &[[f32; 4]]) {
    let (w, h) = (s.info().width, s.info().height);
    for rect in [
        Rect {
            x: 17,
            y: 3,
            w: 20,
            h: 30,
        },
        Rect {
            x: 0,
            y: 32,
            w,
            h: 9,
        },
        Rect {
            x: w - 1,
            y: h - 1,
            w: 1,
            h: 1,
        },
    ] {
        let req = DecodeRequest {
            region: Some(rect),
            ..DecodeRequest::full(1)
        };
        let g = s.decode(&req, &CancelToken::new()).unwrap();
        assert_eq!(
            (g.origin, g.width, g.height),
            ((rect.x, rect.y), rect.w, rect.h)
        );
        for y in 0..g.height {
            for x in 0..g.width {
                let e = want[((rect.y + y) * w + rect.x + x) as usize];
                assert_eq!(px(&g, x, y), e, "{rect:?} ({x}, {y})");
            }
        }
    }
}

/// Peak memory estimates for an 8k panorama: every level now fits the thumbnailer's and the
/// gallery's 512 MB, and 1/2 is a small fraction of the old full `f32` decode.
#[test]
fn radiance_8k_estimates_fit_the_budget() {
    let (w, h) = (8192u32, 4096u32);
    // Header only: the estimate needs no pixels.
    let file = format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y {h} +X {w}\n").into_bytes();
    let s = open("big.hdr", file);
    for (scale, max) in [
        (1, 320u64 << 20),
        (2, 110 << 20),
        (4, 50 << 20),
        (8, 40 << 20),
    ] {
        let m = s.decode_memory(&DecodeRequest::full(scale));
        let out = u64::from(w.div_ceil(scale)) * u64::from(h.div_ceil(scale)) * 8;
        assert!(m >= out && m <= max, "1/{scale}: {m}");
    }
    let tile = DecodeRequest {
        region: Some(Rect {
            x: 4096,
            y: 2048,
            w: 512,
            h: 512,
        }),
        ..DecodeRequest::full(1)
    };
    assert!(s.decode_memory(&tile) < 16 << 20);
}

/// The 8k Radiance panorama of the large corpus opens at a level within the budget, its
/// levels match each other, and a full-size region matches the same pixels of level 1.
#[test]
fn corpus_8k_radiance_opens() {
    let Some(dir) = std::env::var_os("SPECULAR_LARGE_CORPUS") else {
        eprintln!("SPECULAR_LARGE_CORPUS not set; skipped");
        return;
    };
    let path = std::path::Path::new(&dir).join("hdr/kloofendal_48d_partly_cloudy_puresky_8k.hdr");
    let s = Registry::new()
        .open(Arc::new(Source::open(&path).unwrap()))
        .unwrap();
    let (w, h) = (s.info().width, s.info().height);
    assert_eq!((w, h), (8192, 4096));
    assert!(s.info().fast_regions);
    assert!(s.open_memory(&[2]) <= 512 << 20);
    let l4 = decode(&*s, 4);
    assert_eq!((l4.width, l4.height), (2048, 1024));
    assert!(!l4.truncated);
    let l8 = decode(&*s, 8);
    // 1/8 is 1/4 boxed by 2 (both from full size, in linear light): close, not equal
    // (half rounding of the intermediate).
    for (x, y) in [(0, 0), (500, 100), (1023, 511), (700, 300)] {
        let mut sum = [0f32; 4];
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let p = px(&l4, 2 * x + dx, 2 * y + dy);
            for c in 0..4 {
                sum[c] += p[c] / 4.0;
            }
        }
        assert!(
            near(px(&l8, x, y), sum, 2e-3),
            "({x}, {y}) {:?} {sum:?}",
            px(&l8, x, y)
        );
    }
    s.prepare_regions(&CancelToken::new()).unwrap();
    let rect = Rect {
        x: 4000,
        y: 1800,
        w: 64,
        h: 32,
    };
    let req = DecodeRequest {
        region: Some(rect),
        ..DecodeRequest::full(1)
    };
    let tile = s.decode(&req, &CancelToken::new()).unwrap();
    assert_eq!(
        (tile.origin, tile.width, tile.height),
        ((4000, 1800), 64, 32)
    );
    // Its 4×4 boxes are level 1/4's pixels.
    for (bx, by) in [(0, 0), (15, 7), (3, 5)] {
        let mut sum = [0f32; 4];
        for dy in 0..4 {
            for dx in 0..4 {
                let p = px(&tile, 4 * bx + dx, 4 * by + dy);
                for c in 0..4 {
                    sum[c] += p[c] / 16.0;
                }
            }
        }
        let l = px(&l4, 1000 + bx, 450 + by);
        assert!(near(l, sum, 2e-3), "{l:?} {sum:?}");
    }
}
