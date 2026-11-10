//! Every file of the fidelity corpus goes through the registry twice, as
//! decoded samples (full size, stored orientation, no colour management) and as the viewer
//! shows it (`convert_bytes`: ICC to sRGB, HDR tone-mapped, orientation applied, 8-bit PNG).
//! `tools/corpus/fidelity-oracle.py check` then compares both with two independent reference
//! decoders (and the generator's own array, for files it made losslessly), classifies each
//! difference (shift, channel swap, gamma/range, orientation, edge-only, systematic bias) and
//! prints one `ORACLE` line per file.
//!
//! The extra set adds further rows: the composited frames after the first of an animation
//! (`file#k`, up to 8) and the pages after the first (`file@k`, up to 6; a PSD layer page
//! carries its layer name), compared as samples only; and, for YCbCr rasters, the decoded
//! planes themselves (`<n>.planes`) for a bit-exact check against libwebp's and
//! libjpeg-turbo's. Lossy WebP is a studio-range `Yuv` raster (VP8's planes as
//! decoded), compared with `dwebp -yuv` as they are.
//!
//! ```text
//! tools/corpus/fetch-fidelity.sh                 # matrices and suites
//! tools/corpus/fetch-fidelity.sh --extra         # the extra set, into .../corpus/fidelity-extra
//! SPECULAR_FIDELITY_CORPUS=<dir> SPECULAR_RAWPY=<venv>/bin/python \
//!   cargo test -p specular-convert --test fidelity -- --ignored --nocapture
//! ```
//!
//! `SPECULAR_FIDELITY_ONLY=png,tiff` limits the groups (the corpus's top-level folders);
//! `SPECULAR_FIDELITY_KEEP=1` keeps the decoded samples and the manifest under `TMPDIR`
//! (`fidelity-<pid>/`) for `fidelity-oracle.py check` runs of their own.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use specular_convert::{ColorMode, Depth, Format, HdrMode, MetaMode, Options, Resize};
use specular_core::{CancelToken, PixelLayout, Raster};
use specular_decode::{DecodeRequest, Registry};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_FIDELITY_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            let t = std::env::var_os("CARGO_TARGET_DIR")?;
            Some(Path::new(&t).join("corpus/fidelity"))
        })?;
    dir.is_dir().then_some(dir)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Hidden files, and a suite's expected renderings (BMP Suite's html/).
        if name.starts_with('.') || (p.is_dir() && name == "html") {
            continue;
        }
        if p.is_dir() {
            files(&p, out);
        } else if !matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("npy" | "json" | "txt" | "md" | "sha256" | "tsv" | "rgba" | "conf")
        ) {
            out.push(p);
        }
    }
}

fn u16_at(d: &[u8], i: usize) -> f32 {
    f32::from(u16::from_ne_bytes([d[i], d[i + 1]]))
}

fn half(bits: u16) -> f32 {
    let s = if bits >> 15 == 1 { -1.0 } else { 1.0 };
    let e = i32::from((bits >> 10) & 0x1f);
    let m = f32::from(bits & 0x3ff);
    match e {
        0 => s * m * 2f32.powi(-24),
        31 if m == 0.0 => s * f32::INFINITY,
        31 => f32::NAN,
        _ => s * (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// The raster as straight-alpha RGBA `f32` (integer samples over their full scale; half
/// floats as stored), stored orientation.
fn to_rgba_f32(r: &Raster) -> Option<Vec<f32>> {
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = vec![0f32; w * h * 4];
    let p0 = r.planes.first()?;
    for y in 0..h {
        for x in 0..w {
            let o = &mut out[(y * w + x) * 4..(y * w + x) * 4 + 4];
            match r.layout {
                PixelLayout::Gray => {
                    let v = f32::from(p0.data[y * p0.stride + x]) / 255.0;
                    o.copy_from_slice(&[v, v, v, 1.0]);
                }
                PixelLayout::Gray16 => {
                    let v = u16_at(&p0.data, y * p0.stride + 2 * x) / 65535.0;
                    o.copy_from_slice(&[v, v, v, 1.0]);
                }
                PixelLayout::GrayAlpha16 => {
                    let i = y * p0.stride + 4 * x;
                    let v = u16_at(&p0.data, i) / 65535.0;
                    let a = u16_at(&p0.data, i + 2) / 65535.0;
                    o.copy_from_slice(&[v, v, v, a]);
                }
                PixelLayout::Rgba => {
                    let i = y * p0.stride + 4 * x;
                    for (c, v) in o.iter_mut().enumerate() {
                        *v = f32::from(p0.data[i + c]) / 255.0;
                    }
                }
                PixelLayout::Rgba16(_) => {
                    let i = y * p0.stride + 8 * x;
                    for (c, v) in o.iter_mut().enumerate() {
                        *v = u16_at(&p0.data, i + 2 * c) / 65535.0;
                    }
                }
                PixelLayout::Rgba16F(_) => {
                    let i = y * p0.stride + 8 * x;
                    for (c, v) in o.iter_mut().enumerate() {
                        *v = half(u16::from_ne_bytes([
                            p0.data[i + 2 * c],
                            p0.data[i + 2 * c + 1],
                        ]));
                    }
                }
                PixelLayout::Rgb | PixelLayout::YCbCr => {
                    let mut s = [0f32; 3];
                    for (c, v) in s.iter_mut().enumerate() {
                        *v = plane_at(r.planes.get(c)?, x, y);
                    }
                    let rgb = if r.layout == PixelLayout::YCbCr {
                        // JFIF: chroma centred on code 128, results clamped as decoders do.
                        let (l, cb, cr) = (s[0], s[1] - 128.0 / 255.0, s[2] - 128.0 / 255.0);
                        [
                            l + 1.402 * cr,
                            l - 0.344_136 * cb - 0.714_136 * cr,
                            l + 1.772 * cb,
                        ]
                        .map(|v| v.clamp(0.0, 1.0))
                    } else {
                        s
                    };
                    o[..3].copy_from_slice(&rgb);
                    o[3] = 1.0;
                }
                PixelLayout::Yuv(e) if e.bits <= 8 => {
                    // The planes' own coding (lossy WebP: studio-range BT.601), bilinear
                    // centred chroma as above; unclamped until the end, as libwebp.
                    let ([yo, ys], [co, cs]) = e.levels();
                    let m = e.matrix.to_rgb();
                    let mut s = [0f32; 3];
                    for (c, v) in s.iter_mut().enumerate() {
                        *v = plane_at(r.planes.get(c)?, x, y) * 255.0;
                    }
                    let (l, u, v) = ((s[0] - yo) * ys, (s[1] - co) * cs, (s[2] - co) * cs);
                    for (c, o) in o[..3].iter_mut().enumerate() {
                        *o = (m[c][0] * l + m[c][1] * u + m[c][2] * v).clamp(0.0, 1.0);
                    }
                    o[3] = 1.0;
                }
                PixelLayout::Yuv(_) => return None,
            }
        }
    }
    Some(out)
}

/// A subsampled plane's sample at full-size pixel (x, y): bilinear with centred siting for
/// 2× factors, as the viewer's conversion and libjpeg's fancy upsampling do; replicated for
/// larger factors (4:1:1, 4:1:0), as libjpeg does (the viewer filters those bilinearly too,
/// which the shown-picture check leaves aside).
fn plane_at(p: &specular_core::RasterPlane, x: usize, y: usize) -> f32 {
    // libjpeg filters only h2v1 and h2v2; any factor above 2 replicates in both directions.
    let replicate = p.h_div > 2 || p.v_div > 2;
    let at = |n: usize, div: usize, size: usize| -> (usize, usize, f32) {
        let last = size.max(1) - 1;
        if div <= 1 {
            return (n.min(last), n.min(last), 0.0);
        }
        if replicate {
            return ((n / div).min(last), (n / div).min(last), 0.0);
        }
        let c = (n as f32 + 0.5) / div as f32 - 0.5;
        if c <= 0.0 {
            return (0, 0, 0.0);
        }
        let i0 = (c as usize).min(last);
        (i0, (i0 + 1).min(last), c - c.floor())
    };
    let (x0, x1, fx) = at(x, usize::from(p.h_div.max(1)), p.width as usize);
    let (y0, y1, fy) = at(y, usize::from(p.v_div.max(1)), p.height as usize);
    let v = |xx: usize, yy: usize| f32::from(p.data[yy * p.stride + xx]);
    let top = v(x0, y0) * (1.0 - fx) + v(x1, y0) * fx;
    let bot = v(x0, y1) * (1.0 - fx) + v(x1, y1) * fx;
    (top * (1.0 - fy) + bot * fy) / 255.0
}

fn layout_name(l: PixelLayout) -> String {
    match l {
        PixelLayout::Gray => "Gray".into(),
        PixelLayout::YCbCr => "YCbCr".into(),
        PixelLayout::Rgb => "Rgb".into(),
        PixelLayout::Rgba => "Rgba".into(),
        PixelLayout::Rgba16(e) => format!("Rgba16:{e:?}").replace(['\t', ' '], ""),
        PixelLayout::Rgba16F(p) => format!("Rgba16F:{p:?}").replace(['\t', ' '], ""),
        PixelLayout::Gray16 => "Gray16".into(),
        PixelLayout::GrayAlpha16 => "GrayAlpha16".into(),
        PixelLayout::Yuv(_) => "Yuv".into(),
    }
}

/// Frames after the first (composited canvases) and pages after the first are written as
/// further manifest rows, `file#k` and `file@k`.
const MAX_FRAMES: u32 = 8;
const MAX_PAGES: u32 = 6;

/// One decoded raster's samples into `raw`; its manifest line (no shown picture).
fn raster_row(tag: &str, r: &Raster, info: &specular_decode::ImageInfo, raw: &Path) -> String {
    let Some(px) = to_rgba_f32(r) else {
        return format!("{tag}\terr:layout not compared");
    };
    let bytes: Vec<u8> = px.iter().flat_map(|v| v.to_le_bytes()).collect();
    if let Err(e) = std::fs::write(raw, bytes) {
        return format!("{tag}\terr:{e}");
    }
    format!(
        "{tag}\tok\t{}\t{}\t{}\t{}\t{}\t0\t{}\t{}\t{}\t-",
        r.width,
        r.height,
        layout_name(r.layout),
        info.bit_depth,
        info.orientation.0,
        u8::from(r.truncated),
        info.format.name(),
        raw.display(),
    )
}

fn err_row(tag: &str, e: &dyn std::fmt::Display) -> String {
    format!("{tag}\terr:{}", e.to_string().replace(['\t', '\n'], " "))
}

/// The frames after the first and the pages after the first, as extra manifest rows.
fn extra_rows(s: &dyn specular_decode::Session, rel: &str, work: &Path, n: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let info = s.info().clone();
    if s.animation().is_some()
        && let Ok(mut fr) = s.frames(1)
    {
        let cancel = CancelToken::new();
        for k in 0..MAX_FRAMES {
            let Some(f) = fr.next_frame(&cancel) else {
                break;
            };
            let tag = format!("{rel}#{k}");
            match f {
                Ok(f) if k > 0 => {
                    let raw = work.join(format!("{n}-f{k}.f32"));
                    rows.push(raster_row(&tag, &f.raster, &info, &raw));
                }
                Ok(_) => {}
                Err(e) => {
                    rows.push(err_row(&tag, &e));
                    break;
                }
            }
        }
    }
    for p in 1..s.page_count().min(MAX_PAGES) {
        let tag = format!("{rel}@{p}");
        let row = s.open_page(p).and_then(|ps| {
            let r = ps.decode(&DecodeRequest::full(1), &CancelToken::new())?;
            let raw = work.join(format!("{n}-p{p}.f32"));
            let mut row = raster_row(&tag, &r, ps.info(), &raw);
            if let Some(specular_decode::PageLabel::DocumentLayer { name, index, .. }) =
                ps.page_label()
            {
                // A document layer's name and place from the top, for the reference to find.
                row.push_str(&format!(
                    "\tlayer={index}:{}",
                    name.replace(['\t', '\n'], " ")
                ));
            }
            Ok(row)
        });
        rows.push(row.unwrap_or_else(|e| err_row(&tag, &e)));
    }
    rows
}

/// Decodes one file both ways into `work`; its manifest lines (the file, then its further
/// frames and pages).
fn dump(reg: &Registry, root: &Path, path: &Path, work: &Path, n: usize) -> String {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    let err = |e: &dyn std::fmt::Display| {
        format!("{rel}\terr:{}", e.to_string().replace(['\t', '\n'], " "))
    };
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let s = match reg.open_path(path) {
            Ok(s) => s,
            Err(e) => return err(&e),
        };
        let info = s.info().clone();
        let raster = match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
            Ok(r) => r,
            Err(e) => return err(&e),
        };
        let raw = work.join(format!("{n}.f32"));
        let Some(px) = to_rgba_f32(&raster) else {
            return err(&"layout not compared");
        };
        let bytes: Vec<u8> = px.iter().flat_map(|v| v.to_le_bytes()).collect();
        if let Err(e) = std::fs::write(&raw, bytes) {
            return err(&e);
        }
        if raster.layout == PixelLayout::YCbCr
            || matches!(raster.layout, PixelLayout::Yuv(e) if e.bits <= 8)
        {
            // The decoded planes themselves (Y, then Cb, then Cr, rows without padding), for
            // a bit-exact check against a reference's raw YUV where one gives it (dwebp).
            let planes: Vec<u8> = raster
                .planes
                .iter()
                .flat_map(|p| {
                    (0..p.height as usize).flat_map(move |y| {
                        p.data[y * p.stride..y * p.stride + p.width as usize]
                            .iter()
                            .copied()
                    })
                })
                .collect();
            if let Err(e) = std::fs::write(raw.with_extension("planes"), planes) {
                return err(&e);
            }
        }
        let opts = Options {
            format: Format::Png,
            png_effort: 1,
            depth: Depth::Eight,
            resize: Resize::None,
            upscale: false,
            color: ColorMode::Srgb,
            hdr: HdrMode::ToneMap,
            metadata: MetaMode::Strip,
            ..Options::default()
        };
        let png = work.join(format!("{n}.png"));
        let shown = match specular_convert::convert_bytes(s.as_ref(), &opts, &CancelToken::new()) {
            Ok(Some(b)) => {
                std::fs::write(&png, b).map_or("-".into(), |()| png.display().to_string())
            }
            _ => "-".into(),
        };
        let extra = extra_rows(s.as_ref(), &rel, work, n);
        let mut line = format!(
            "{rel}\tok\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{shown}",
            raster.width,
            raster.height,
            layout_name(raster.layout),
            info.bit_depth,
            info.orientation.0,
            u8::from(info.icc_profile.is_some() || s.embedded_icc().is_some()),
            u8::from(raster.truncated),
            info.format.name(),
            raw.display(),
        );
        for e in extra {
            line.push('\n');
            line.push_str(&e);
        }
        line
    }));
    r.unwrap_or_else(|_| err(&"panic"))
}

#[test]
#[ignore = "needs the fidelity corpus and the oracle venv"]
fn fidelity_matrix() {
    let Some(root) = corpus() else {
        eprintln!("fidelity: no corpus (tools/corpus/fetch-fidelity.sh); skipped");
        return;
    };
    let py = std::env::var("SPECULAR_RAWPY").unwrap_or_else(|_| "python3".into());
    let only: Vec<String> = std::env::var("SPECULAR_FIDELITY_ONLY")
        .map(|s| s.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let tmp = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let work = tmp.join(format!("fidelity-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let mut list = Vec::new();
    files(&root, &mut list);
    let reg = Registry::new();
    let manifest = work.join("manifest.tsv");
    let mut m = std::fs::File::create(&manifest).unwrap();
    for (n, p) in list.iter().enumerate() {
        let group = p
            .strip_prefix(&root)
            .ok()
            .and_then(|r| r.components().next())
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        if !only.is_empty() && !only.contains(&group) {
            continue;
        }
        writeln!(m, "{}", dump(&reg, &root, p, &work, n)).unwrap();
    }
    drop(m);
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/fidelity-oracle.py");
    let out = Command::new(&py)
        .arg("-I")
        .arg(&script)
        .arg("check")
        .arg(&root)
        .arg(&manifest)
        .output()
        .expect("run fidelity-oracle.py");
    let text = String::from_utf8_lossy(&out.stdout);
    print!("{text}");
    let errs = String::from_utf8_lossy(&out.stderr);
    for l in errs
        .lines()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        eprintln!("{l}");
    }
    if std::env::var_os("SPECULAR_FIDELITY_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&work);
    }
    assert!(out.status.success(), "fidelity-oracle.py failed");
    // Failures not listed as known in the script.
    let unexpected: usize = text
        .lines()
        .find_map(|l| l.strip_prefix("#\tunexpected\t"))
        .and_then(|n| n.trim().parse().ok())
        .expect("the script's summary");
    assert_eq!(unexpected, 0, "files differ from the references");
}
