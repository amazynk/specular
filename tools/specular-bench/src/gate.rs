//! Instruction-count gate (feature `gate`): a fixed set of single-threaded decodes, one per
//! process, counted under valgrind's callgrind by `tools/bench/perf-gate.py`.
//!
//! ```text
//! specular-bench gate list               case names, one per line, with their input file
//! specular-bench gate gen DIR            write the generated inputs into DIR
//! specular-bench gate train DIR          write PGO training inputs (not the gate's) under DIR
//! specular-bench gate run CASE FILE      decode FILE as CASE once
//! specular-bench gate time CASE FILE [N] the counted work's native wall time, median of N
//! ```
//!
//! Only [`perfgate_measure`] is counted (`callgrind --toggle-collect`), so reading the file,
//! process start-up and exit are not. Every decoder runs with its single-threaded option.
//! Inputs are either generated here by pure-Rust encoders pinned in Cargo.lock (JPEG, PNG,
//! GIF, TIFF, DNG) from integer-only procedural images, so they are byte-identical on every
//! machine, or committed files (WebP, AVIF, HEIC, JPEG XL; see `tools/bench/gate/`).

use std::hint::black_box;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};

#[allow(dead_code, clippy::all)]
#[path = "../../../crates/formats/specular-raw/tests/common/mod.rs"]
mod raw_common;

/// Where a case's input comes from.
enum Input {
    /// Written by `gate gen` into the inputs directory.
    Generated(&'static str),
    /// A committed file, relative to the workspace root.
    Committed(&'static str),
}

struct Case {
    name: &'static str,
    input: Input,
}

const CASES: &[Case] = &[
    Case {
        name: "jpeg-420-3mp",
        input: Input::Generated("jpeg-420.jpg"),
    },
    Case {
        name: "jpeg-420-3mp-quarter",
        input: Input::Generated("jpeg-420.jpg"),
    },
    Case {
        name: "jpeg-444-progressive",
        input: Input::Generated("jpeg-444-prog.jpg"),
    },
    Case {
        name: "jpeg-arith-420",
        input: Input::Committed("tools/bench/gate/jpeg-arith-420.jpg"),
    },
    Case {
        name: "png-rgb8-photo",
        input: Input::Generated("png-rgb8.png"),
    },
    Case {
        name: "png-rgba8-graphic",
        input: Input::Generated("png-rgba8.png"),
    },
    Case {
        name: "png-rgb16-photo",
        input: Input::Generated("png-rgb16.png"),
    },
    Case {
        name: "webp-lossy",
        input: Input::Committed("tools/bench/gate/webp-lossy.webp"),
    },
    Case {
        name: "webp-lossless",
        input: Input::Committed("tools/bench/gate/webp-lossless.webp"),
    },
    Case {
        name: "avif-420",
        input: Input::Committed("tools/bench/gate/avif-420.avif"),
    },
    // The same image as the display gets it: its YCbCr planes, for the GPU to convert.
    Case {
        name: "avif-420-display",
        input: Input::Committed("tools/bench/gate/avif-420.avif"),
    },
    // The planes and their CPU conversion (thumbnails, export), chroma interpolated like
    // the GPU's.
    Case {
        name: "avif-420-rgb",
        input: Input::Committed("tools/bench/gate/avif-420.avif"),
    },
    Case {
        name: "heic-grid",
        input: Input::Committed("crates/specular-decode/data/heic-calibration.heic"),
    },
    // The same decode and what the display does after it: the YCbCr planes handed over for
    // the GPU to convert.
    Case {
        name: "heic-grid-display",
        input: Input::Committed("crates/specular-decode/data/heic-calibration.heic"),
    },
    // The same decode and the CPU conversion of its planes (thumbnails, export), chroma
    // interpolated like the GPU's.
    Case {
        name: "heic-grid-rgb",
        input: Input::Committed("crates/specular-decode/data/heic-calibration.heic"),
    },
    Case {
        name: "jxl-vardct",
        input: Input::Committed("crates/formats/specular-jxl/tests/data/vardct_multigroup.jxl"),
    },
    Case {
        name: "jxl-modular",
        input: Input::Committed("crates/formats/specular-jxl/tests/data/modular.jxl"),
    },
    Case {
        name: "gif-still",
        input: Input::Generated("gif-still.gif"),
    },
    Case {
        name: "gif-anim",
        input: Input::Generated("gif-anim.gif"),
    },
    Case {
        name: "tiff-lzw",
        input: Input::Generated("tiff-lzw.tif"),
    },
    Case {
        name: "tiff-deflate",
        input: Input::Generated("tiff-deflate.tif"),
    },
    Case {
        name: "raw-dng-lj92",
        input: Input::Generated("raw-lj92.dng"),
    },
    Case {
        name: "raw-arw-lj92",
        input: Input::Generated("raw-lj92.arw"),
    },
];

pub fn main(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => {
            for c in CASES {
                match c.input {
                    Input::Generated(f) => println!("{}\tgenerated\t{f}", c.name),
                    Input::Committed(f) => println!("{}\tcommitted\t{f}", c.name),
                }
            }
            for c in crate::headline::CASES {
                if c.budget_ms > 0.0 {
                    println!(
                        "{}\tgenerated\t{}\t{}\t{}\t{}",
                        c.name, c.input, c.budget_ms, c.cores, c.what
                    );
                } else {
                    println!("{}\tgenerated\t{}", c.name, c.input);
                }
            }
            Ok(())
        }
        Some("gen") => {
            let dir = Path::new(args.get(1).context("gate gen DIR")?);
            generate(dir)
        }
        Some("gen-app") => {
            let dir = Path::new(args.get(1).context("gate gen-app DIR")?);
            generate_app(dir)
        }
        Some("train") => {
            let dir = Path::new(args.get(1).context("gate train DIR")?);
            generate_training(dir)
        }
        Some("run") => {
            let (case, file) = match args {
                [_, c, f] => (c, f),
                _ => bail!("gate run CASE FILE"),
            };
            let (out, _) = run_case(case, file)?;
            println!("{case}: {out}");
            Ok(())
        }
        // Native wall time of the counted work, for information.
        Some("time") => {
            let (case, file, n) = match args {
                [_, c, f] => (c, f, 5),
                [_, c, f, n] => (c, f, n.parse()?),
                _ => bail!("gate time CASE FILE [RUNS]"),
            };
            let mut ms: Vec<f64> = (0..n.max(1))
                .map(|_| run_case(case, file).map(|r| r.1))
                .collect::<Result<_>>()?;
            ms.sort_by(f64::total_cmp);
            let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
            println!(
                "{{\"case\": \"{case}\", \"runs\": {}, \"median_ms\": {:.2}, \"min_ms\": {:.2}, \"load\": \"{}\"}}",
                ms.len(),
                ms[ms.len() / 2],
                ms[0],
                load.split_whitespace().next().unwrap_or("?")
            );
            Ok(())
        }
        _ => bail!(
            "usage: specular-bench gate list | gen DIR | gen-app DIR | train DIR | run CASE FILE"
        ),
    }
}

/// Runs one case once: what it produced and the counted work's wall time (ms).
fn run_case(case: &str, file: &str) -> Result<(String, f64)> {
    if crate::headline::is_case(case) {
        return crate::headline::run(case, Path::new(file));
    }
    if !CASES.iter().any(|c| c.name == case) {
        bail!("unknown case {case}");
    }
    let data = std::fs::read(file).with_context(|| format!("reading {file}"))?;
    let t = std::time::Instant::now();
    let (w, h) = perfgate_measure(case, &data)?;
    Ok((format!("{w}x{h}"), t.elapsed().as_secs_f64() * 1e3))
}

fn err<E: std::fmt::Debug>(e: E) -> anyhow::Error {
    anyhow!("{e:?}")
}

/// The counted region: open/parse plus one full single-threaded decode, including freeing the
/// output. Returns the decoded size. The name is the callgrind toggle pattern; keep it unique.
#[inline(never)]
fn perfgate_measure(case: &str, data: &[u8]) -> Result<(u32, u32)> {
    let data = black_box(data);
    let size = match case {
        "jpeg-420-3mp" | "jpeg-444-progressive" | "jpeg-420-3mp-quarter" | "jpeg-arith-420" => {
            use specular_jpeg::{DecodeOptions, Parallelism, Scale};
            let scale = if case.ends_with("quarter") {
                Scale::Quarter
            } else {
                Scale::Full
            };
            let opts = DecodeOptions {
                scale,
                parallelism: Parallelism::Single,
                ..DecodeOptions::default()
            };
            let d = black_box(specular_jpeg::decode(data, &opts).map_err(err)?);
            (d.width, d.height)
        }
        "png-rgb8-photo" | "png-rgba8-graphic" | "png-rgb16-photo" => {
            use specular_png::{DecodeOptions, Parallelism};
            let opts = DecodeOptions {
                parallelism: Parallelism::Single,
                ..DecodeOptions::default()
            };
            let d = black_box(specular_png::decode(data, &opts).map_err(err)?);
            (d.width, d.height)
        }
        "webp-lossy" | "webp-lossless" => {
            let img = specular_webp::WebpImage::parse(data).map_err(err)?;
            let d = black_box(img.decode(1, false).map_err(err)?);
            (d.width, d.height)
        }
        "avif-420" => {
            let f = specular_avif::AvifFile::open(data).map_err(err)?;
            let d = black_box(f.decode(1, false).map_err(err)?);
            (d.width, d.height)
        }
        "avif-420-display" | "avif-420-rgb" => {
            let f = specular_avif::AvifFile::open(data).map_err(err)?;
            let y = f
                .decode_planes(1, false, &|| false)
                .map_err(err)?
                .ok_or_else(|| anyhow!("no planes"))?;
            if case == "avif-420-display" {
                black_box(&y.planes);
                return Ok((y.width, y.height));
            }
            use specular_core::{PixelLayout, Raster, RasterPlane, YuvEncoding, YuvMatrix};
            // As specular-decode labels them (the gate image is BT.601, limited range).
            let e = YuvEncoding {
                matrix: YuvMatrix::from_h273(Some(y.matrix), 0),
                full_range: y.full_range,
                bits: y.bits,
                cosited_x: y.cosited_x,
                cosited_y: y.cosited_y,
                bottom_y: false,
                right_x: false,
                alpha: y.planes.len() > 3,
                colour: specular_core::HdrEncoding::SRGB,
            };
            let planes = Raster {
                width: y.width,
                height: y.height,
                origin: (0, 0),
                scale: 1,
                layout: PixelLayout::Yuv(e),
                planes: y
                    .planes
                    .into_iter()
                    .map(|p| RasterPlane {
                        width: p.width,
                        height: p.height,
                        stride: p.stride,
                        data: p.data,
                        h_div: p.h_div,
                        v_div: p.v_div,
                    })
                    .collect(),
                truncated: false,
            };
            let r =
                specular_core::yuv::to_rgb_with(&planes, false).ok_or_else(|| anyhow!("planes"))?;
            black_box(&r.planes);
            (r.width, r.height)
        }
        "heic-grid" => {
            let f = specular_heic::HeicFile::open(data).map_err(err)?;
            let opts = specular_heic::DecodeOptions::default();
            let d = black_box(f.decode(&opts).map_err(err)?);
            (d.width(), d.height())
        }
        "heic-grid-display" | "heic-grid-rgb" => {
            let f = specular_heic::HeicFile::open(data).map_err(err)?;
            let opts = specular_heic::DecodeOptions::default();
            let d = f.decode(&opts).map_err(err)?;
            let planes = d
                .to_yuv(
                    &specular_heic::ConvertOptions::default(),
                    specular_core::HdrEncoding::SRGB,
                )
                .map_err(err)?
                .ok_or_else(|| anyhow!("monochrome calibration image"))?;
            let r = if case == "heic-grid-rgb" {
                specular_core::yuv::to_rgb_with(&planes, false).ok_or_else(|| anyhow!("planes"))?
            } else {
                planes
            };
            black_box(&r.planes);
            (r.width, r.height)
        }
        "jxl-vardct" | "jxl-modular" => {
            let f = specular_jxl::JxlFile::open(data).map_err(err)?;
            let d = black_box(f.decode(1, false).map_err(err)?);
            (d.width, d.height)
        }
        "gif-still" => {
            use specular_bitmap::Decode as _;
            let g = specular_bitmap::Gif::parse(data).map_err(err)?;
            let opts = specular_bitmap::Options {
                parallel: false,
                ..Default::default()
            };
            let d = black_box(g.decode(data, &opts, &|| false).map_err(err)?);
            (d.width, d.height)
        }
        "gif-anim" => {
            use specular_bitmap::Decode as _;
            let g = Arc::new(specular_bitmap::Gif::parse(data).map_err(err)?);
            let size = (g.info().width, g.info().height);
            let mut anim = specular_bitmap::gif::Animator::new(g);
            while let Some(f) = anim.next_frame(data, &|| false) {
                black_box(f.map_err(err)?);
            }
            size
        }
        "tiff-lzw" | "tiff-deflate" => {
            let t = specular_tiff::Tiff::open(data).map_err(err)?;
            let opts = specular_tiff::DecodeOptions {
                parallel: false,
                ..Default::default()
            };
            let d = black_box(t.decode(data, 0, &opts).map_err(err)?);
            (d.width, d.height)
        }
        "raw-dng-lj92" | "raw-arw-lj92" => {
            let f = specular_raw::parse(data).map_err(err)?;
            let opts = specular_raw::DecodeOptions {
                parallel: false,
                ..Default::default()
            };
            let d = black_box(f.decode(data, &opts).map_err(err)?);
            (d.width, d.height)
        }
        other => bail!("unknown case {other}"),
    };
    Ok(size)
}

// ---------------------------------------------------------------------------------------------
// Generated inputs. Integer arithmetic only (no libm), so every machine produces the same pixels,
// and the encoders are pure Rust at the versions pinned in Cargo.lock.

pub(crate) fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h =
        x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}

/// Smooth value noise in 0..=65535, cells of `scale` pixels.
fn value_noise(x: u32, y: u32, scale: u32, seed: u32) -> i64 {
    let (cx, cy) = (x / scale, y / scale);
    let s = |f: u32| -> i64 {
        // smoothstep in 16.16 fixed point
        let t = (f as i64 * 65536) / scale as i64;
        t * t / 65536 * (3 * 65536 - 2 * t) / 65536
    };
    let (fx, fy) = (s(x % scale), s(y % scale));
    let v = |dx: u32, dy: u32| (hash(cx + dx, cy + dy, seed) & 0xffff) as i64;
    let top = v(0, 0) + (v(1, 0) - v(0, 0)) * fx / 65536;
    let bot = v(0, 1) + (v(1, 1) - v(0, 1)) * fx / 65536;
    top + (bot - top) * fy / 65536
}

/// Photo-like RGB8: fractal noise, gradients, soft discs and sensor noise.
fn photo(w: u32, h: u32, seed: u32) -> Vec<u8> {
    const OCTAVES: [(u32, i64); 7] = [
        (256, 1000),
        (128, 620),
        (64, 384),
        (32, 238),
        (16, 148),
        (8, 92),
        (4, 57),
    ];
    let discs: Vec<(i64, i64, i64, [i64; 3])> = (0..24)
        .map(|i| {
            let r = |k: u32| hash(i, k, seed ^ 0x5eed) as i64;
            let c = |k: u32| r(k) % 256;
            (
                r(0) % w as i64,
                r(1) % h as i64,
                16 + r(2) % 120,
                [c(3), c(4), c(5)],
            )
        })
        .collect();
    let mut out = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            for c in 0..3u32 {
                let mut v: i64 = 0;
                for (s, amp) in OCTAVES {
                    v += (value_noise(x, y, s, seed * 7 + c) - 32768) * amp / 32768;
                }
                // v is roughly -1500..1500; map to 0..255 with gradients.
                let grad = match c {
                    0 => x as i64 * 90 / w as i64,
                    1 => y as i64 * 90 / h as i64,
                    _ => 90 - x as i64 * 90 / w as i64,
                };
                let mut p = 70 + v * 100 / 1500 + grad;
                for &(dx, dy, r, col) in &discs {
                    let (ex, ey) = (x as i64 - dx, y as i64 - dy);
                    if ex * ex + ey * ey < r * r {
                        p = (p * 6 + col[c as usize] * 4) / 10;
                    }
                }
                p += (hash(x, y, seed ^ (c + 99)) % 7) as i64 - 3;
                out.push(p.clamp(0, 255) as u8);
            }
        }
    }
    out
}

/// Graphics-like RGBA8: flat panels, hard edges, thin lines, a gradient and an alpha ramp.
fn graphic(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let panel = hash(x / 96, y / 64, 7);
            let mut px = [
                (panel & 0xff) as u8 | 0x40,
                (panel >> 8 & 0xff) as u8 | 0x40,
                (panel >> 16 & 0xff) as u8 | 0x40,
                255u8,
            ];
            if x % 96 < 2 || y % 64 < 2 {
                px = [30, 30, 30, 255];
            }
            // "Text": short dark dashes on some rows of each panel.
            if (y % 64) / 6 % 2 == 1
                && (y % 64) > 10
                && !hash(x / 5, y / 6, 3).is_multiple_of(3)
                && x % 96 > 8
            {
                px = [20, 20, 20, 255];
            }
            if y > h * 3 / 4 {
                px = [
                    (x * 255 / w) as u8,
                    (y * 255 / h) as u8,
                    128,
                    (x * 255 / w) as u8,
                ];
            }
            out.extend_from_slice(&px);
        }
    }
    out
}

pub(crate) fn write(dir: &Path, name: &str, data: &[u8]) -> Result<()> {
    let p = dir.join(name);
    std::fs::write(&p, data).with_context(|| format!("writing {}", p.display()))?;
    println!("{}\t{} bytes", p.display(), data.len());
    Ok(())
}

fn jpeg(
    rgb: &[u8],
    w: u32,
    h: u32,
    quality: u8,
    sampling: jpeg_encoder::SamplingFactor,
    progressive: bool,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut out, quality);
    e.set_sampling_factor(sampling);
    e.set_progressive(progressive);
    e.encode(rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(err)?;
    Ok(out)
}

fn png(px: &[u8], w: u32, h: u32, color: png::ColorType) -> Result<Vec<u8>> {
    png_at(px, w, h, color, png::BitDepth::Eight)
}

/// `px` big-endian samples of `depth`.
fn png_at(
    px: &[u8],
    w: u32,
    h: u32,
    color: png::ColorType,
    depth: png::BitDepth,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(color);
        e.set_depth(depth);
        let mut wr = e.write_header().map_err(err)?;
        wr.write_image_data(px).map_err(err)?;
        wr.finish().map_err(err)?;
    }
    Ok(out)
}

fn tiff(rgb: &[u8], w: u32, h: u32, compression: tiff::encoder::Compression) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut out)
        .map_err(err)?
        .with_compression(compression)
        .with_predictor(tiff::tags::Predictor::Horizontal)
        .write_image::<tiff::encoder::colortype::RGB8>(w, h, rgb)
        .map_err(err)?;
    Ok(out.into_inner())
}

/// A 14-bit RGGB mosaic of the RGB8 image `rgb`, black 256.
fn mosaic(rgb: &[u8], w: u32, h: u32) -> Vec<u16> {
    let mut cfa = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = match (y % 2, x % 2) {
                (0, 0) => 0,
                (1, 1) => 2,
                _ => 1,
            };
            let v = rgb[((y * w + x) * 3 + c) as usize] as u16;
            cfa.push(v * 60 + (hash(x, y, 77) % 48) as u16 + 256);
        }
    }
    cfa
}

/// A 14-bit RGGB DNG with LJ92 tiles, as DNG converters write them, of the RGB8 image `rgb`.
fn dng_lj92(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
    let cfa = mosaic(rgb, w, h);
    let mut spec = raw_common::DngSpec::new(w, h, raw_common::DngCodec::Lj92Tiles(256));
    spec.bits = 14;
    spec.black = 256;
    spec.white = 16383;
    spec.neutral = [0.5, 1.0, 0.7];
    raw_common::write_dng(&spec, &cfa)
}

/// A Sony lossless ARW (512x512 LJ92 tiles, one 4-component MCU per 2x2 block) of `rgb`.
fn arw_lj92(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut spec = raw_common::ArwSpec::new(w, h, 512);
    spec.black = 256;
    raw_common::write_arw(&spec, &mosaic(rgb, w, h))
}

/// A small picture for gallery tiles: a gradient between two hashed colours, a few hashed
/// rectangles and light noise (cheap to make, distinct per `seed`).
fn tile_picture(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let c = |k: u32| {
        let v = hash(seed, k, 11);
        [
            (v & 0xff) as i64,
            (v >> 8 & 0xff) as i64,
            (v >> 16 & 0xff) as i64,
        ]
    };
    let (a, b) = (c(0), c(1));
    let rects: Vec<(u32, u32, u32, u32, [i64; 3])> = (0..4)
        .map(|r| {
            let v = hash(seed, r + 10, 12);
            (
                v % w,
                (v >> 8) % h,
                20 + (v >> 16) % 80,
                20 + (v >> 24) % 60,
                c(r + 2),
            )
        })
        .collect();
    let mut out = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = i64::from(x + y) * 256 / i64::from(w + h);
            let mut px = [0i64; 3];
            for k in 0..3 {
                px[k] = (a[k] * (256 - t) + b[k] * t) / 256;
            }
            for &(rx, ry, rw, rh, col) in &rects {
                if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                    px = col;
                }
            }
            for v in px {
                out.push((v + (hash(x, y, seed) % 9) as i64 - 4).clamp(0, 255) as u8);
            }
        }
    }
    out
}

/// The app suite's own inputs (`tools/bench/perfgate_app.py`): `gallery/`, 2,000 distinct
/// 320 x 240 JPEGs (the first 100 are also the Library's 100k-file folder, linked).
fn generate_app(dir: &Path) -> Result<()> {
    use jpeg_encoder::SamplingFactor;
    let gallery = dir.join("gallery");
    std::fs::create_dir_all(&gallery)?;
    for i in 0..2000u32 {
        let p = gallery.join(format!("g{i:05}.jpg"));
        if p.exists() {
            continue;
        }
        let px = tile_picture(320, 240, i);
        std::fs::write(&p, jpeg(&px, 320, 240, 85, SamplingFactor::R_4_2_0, false)?)?;
    }
    Ok(())
}

/// Inputs for `tools/release/pgo.sh` training, into its corpus folder DIR: `raw/lj92.dng` (no
/// common tool writes LJ92 DNG tiles) and `png/rgb8-gen.png` (the png crate's filters and deflate
/// stream, whose match statistics differ from ImageMagick's zlib ones; trained on the latter
/// only, the inflate loop came out +6.5 % in a PGO build). Other sizes and images than the
/// gate's, so the gate does not measure its own training.
fn generate_training(dir: &Path) -> Result<()> {
    let raw = dir.join("raw");
    std::fs::create_dir_all(&raw)?;
    let (w, h) = (1536, 1024);
    write(&raw, "lj92.dng", &dng_lj92(&photo(w, h, 9), w, h))?;
    let png_dir = dir.join("png");
    std::fs::create_dir_all(&png_dir)?;
    let (w, h) = (1280, 960);
    let px = photo(w, h, 10);
    write(
        &png_dir,
        "rgb8-gen.png",
        &png(&px, w, h, png::ColorType::Rgb)?,
    )
}

fn generate(dir: &Path) -> Result<()> {
    use jpeg_encoder::SamplingFactor;
    std::fs::create_dir_all(dir)?;

    let (w, h) = (2048, 1536);
    let big = photo(w, h, 1);
    write(
        dir,
        "jpeg-420.jpg",
        &jpeg(&big, w, h, 90, SamplingFactor::R_4_2_0, false)?,
    )?;

    // RGGB Bayer mosaic of the same photo, 14 bits, LJ92 tiles as DNG converters write them.
    write(dir, "raw-lj92.dng", &dng_lj92(&big, w, h))?;
    // The same mosaic as a Sony lossless ARW.
    write(dir, "raw-lj92.arw", &arw_lj92(&big, w, h))?;
    // The headline cases' inputs (headline.rs), some made from the same photo.
    crate::headline::generate(dir, &big, w, h)?;
    drop(big);

    let (w, h) = (1024, 768);
    let mid = photo(w, h, 2);
    write(
        dir,
        "jpeg-444-prog.jpg",
        &jpeg(&mid, w, h, 95, SamplingFactor::R_4_4_4, true)?,
    )?;
    write(dir, "png-rgb8.png", &png(&mid, w, h, png::ColorType::Rgb)?)?;
    // 16-bit: the same photo, low bytes from the hash (a scanner's or a raw developer's noise).
    let deep: Vec<u8> = mid
        .iter()
        .enumerate()
        .flat_map(|(i, &v)| [v, hash(i as u32, 0, 16) as u8])
        .collect();
    write(
        dir,
        "png-rgb16.png",
        &png_at(&deep, w, h, png::ColorType::Rgb, png::BitDepth::Sixteen)?,
    )?;
    write(
        dir,
        "png-rgba8.png",
        &png(&graphic(w, h), w, h, png::ColorType::Rgba)?,
    )?;
    use tiff::encoder::{Compression, compression::DeflateLevel};
    write(dir, "tiff-lzw.tif", &tiff(&mid, w, h, Compression::Lzw)?)?;
    write(
        dir,
        "tiff-deflate.tif",
        &tiff(&mid, w, h, Compression::Deflate(DeflateLevel::Balanced))?,
    )?;
    drop(mid);

    // GIF: a quantised photo, and a 16-frame animation of moving panels on a photo background.
    let (w, h) = (512u16, 384u16);
    let mut out = Vec::new();
    {
        let mut rgba: Vec<u8> = photo(w as u32, h as u32, 3)
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        let frame = gif::Frame::from_rgba_speed(w, h, &mut rgba, 10);
        let mut e = gif::Encoder::new(&mut out, w, h, &[]).map_err(err)?;
        e.write_frame(&frame).map_err(err)?;
    }
    write(dir, "gif-still.gif", &out)?;

    let (w, h) = (256u16, 192u16);
    let bg = photo(w as u32, h as u32, 4);
    let mut out = Vec::new();
    {
        let palette: Vec<u8> = (0..=255u32)
            .flat_map(|i| [(i * 37) as u8, (i * 91) as u8, i as u8])
            .collect();
        let mut e = gif::Encoder::new(&mut out, w, h, &palette).map_err(err)?;
        e.set_repeat(gif::Repeat::Infinite).map_err(err)?;
        let mut first = gif::Frame::from_rgb_speed(w, h, &bg, 10);
        first.delay = 4;
        e.write_frame(&first).map_err(err)?;
        for i in 0..15u16 {
            // A 64x48 sub-rectangle frame that moves, drawn from the global palette.
            let (fw, fh) = (64u16, 48u16);
            let buffer: Vec<u8> = (0..fw as u32 * fh as u32)
                .map(|k| ((k % fw as u32 + i as u32 * 3) ^ (k / fw as u32)) as u8)
                .collect();
            let frame = gif::Frame {
                left: i * 12,
                top: (i * 9) % (h - fh),
                width: fw,
                height: fh,
                delay: 4,
                dispose: gif::DisposalMethod::Keep,
                buffer: buffer.into(),
                ..gif::Frame::default()
            };
            e.write_frame(&frame).map_err(err)?;
        }
    }
    write(dir, "gif-anim.gif", &out)?;
    Ok(())
}
