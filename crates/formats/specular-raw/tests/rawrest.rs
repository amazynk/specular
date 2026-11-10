//! Oracle comparison with rawpy (LibRaw) for the corpus of `tools/corpus/fetch-rawrest.sh`:
//! Phase One IIQ calibration (black tables, flat fields, defects, curves), Kodak DC50 RADC and
//! DC120 JPEG, Canon CMYG CRW, Sony SRF, Sigma X3F, and the colour of the bodies whose matrices
//! or white balance were missing.
//!
//!     SPECULAR_RAWPY=$CARGO_TARGET_DIR/venv/bin/python \
//!       cargo test -p specular-raw --test rawrest -- --ignored --nocapture
//!
//! Per file: the unpacked sensor values against LibRaw's `raw_image` (exact where LibRaw's are
//! the same stage), for IIQ the calibrated photosite values against a LibRaw bilinear develop
//! in camera colour (max / mean difference), and the develop's colour balance (R/G, B/G of ours
//! over LibRaw's and over the camera's own preview) and structure (correlation).
//! `SPECULAR_RAWREST_CORPUS` overrides the corpus directory, `SPECULAR_RAWREST_ONLY` keeps the
//! paths that contain it. Fails on decode errors other than plain refusals.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use specular_raw::{DecodeOptions, OutputSize, RawImage, RgbImage};

/// The upper-case extension of a corpus path ("?" without one).
fn ext_of(rel: &str) -> String {
    Path::new(rel)
        .extension()
        .map_or("?".into(), |e| e.to_string_lossy().to_ascii_uppercase())
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if !p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            out.push(p);
        }
    }
}

struct Plane {
    h: usize,
    w: usize,
    c: usize,
    data: Vec<u16>,
}

struct Oracle {
    facts: Vec<(String, String)>,
    plane: Plane,
}

impl Oracle {
    fn fact(&self, k: &str) -> Option<&str> {
        self.facts
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }
    fn nums(&self, k: &str) -> Vec<f64> {
        self.fact(k)
            .unwrap_or("")
            .split(',')
            .filter_map(|v| v.parse().ok())
            .collect()
    }
}

fn oracle(py: &str, file: &Path, tmp: &Path, mode: &str) -> Result<Oracle, String> {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/quality/rawrest_oracle.py");
    let prefix = tmp.join("oracle");
    let out = Command::new(py)
        .arg("-I")
        .arg(&script)
        .arg(file)
        .arg(&prefix)
        .arg(mode)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().last().unwrap_or("rawpy failed").to_string());
    }
    let facts = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let b = std::fs::read(prefix.with_extension(mode)).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(prefix.with_extension(mode));
    let u = |i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
    let plane = Plane {
        h: u(0),
        w: u(1),
        c: u(2),
        data: b[12..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect(),
    };
    Ok(Oracle { facts, plane })
}

/// (mismatches, compared, max |diff|) of our sensor samples against LibRaw's raw_image, at
/// the offset (within the size difference) with the fewest mismatches.
fn compare_raw(raw: &RawImage, o: &Plane) -> Option<(u64, u64, u32, i64, i64)> {
    let (w, h, c) = (raw.width as usize, raw.height as usize, raw.cpp as usize);
    if c != o.c {
        return None;
    }
    let cmp = |dx: i64, dy: i64, step: usize| {
        let (mut bad, mut n, mut maxd) = (0u64, 0u64, 0u32);
        for y in (0..h).step_by(step) {
            let oy = y as i64 + dy;
            if oy < 0 || oy as usize >= o.h {
                continue;
            }
            for x in 0..w {
                let ox = x as i64 + dx;
                if ox < 0 || ox as usize >= o.w {
                    continue;
                }
                for k in 0..c {
                    let a = raw.data[(y * w + x) * c + k];
                    let b = o.data[(oy as usize * o.w + ox as usize) * c + k];
                    n += 1;
                    if a != b {
                        bad += 1;
                        maxd = maxd.max((a as i32 - b as i32).unsigned_abs());
                    }
                }
            }
        }
        (bad, n, maxd)
    };
    let (rx, ry) = (
        (w as i64 - o.w as i64).abs().min(64),
        (h as i64 - o.h as i64).abs().min(64),
    );
    let mut best = (u64::MAX, 0i64, 0i64);
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let (b, n, _) = cmp(dx, dy, 31);
            if n > 0 && b < best.0 {
                best = (b, dx, dy);
            }
        }
    }
    let (b, n, m) = cmp(best.1, best.2, 1);
    if b > 0 && std::env::var_os("RAWREST_DIAG").is_some() && best == (b, 0, 0) || b > 0 && w == o.w
    {
        mismatch_layout(raw, o);
    }
    Some((b, n, m, best.1, best.2))
}

/// Where same-size raw mismatches lie (`RAWREST_DIAG`): extent, channels, and the most common
/// column and row residues for a few periods.
fn mismatch_layout(raw: &RawImage, o: &Plane) {
    if std::env::var_os("RAWREST_DIAG").is_none() {
        return;
    }
    let (w, h, c) = (raw.width as usize, raw.height as usize, raw.cpp as usize);
    let mut pts = Vec::new();
    let mut per_c = vec![0u64; c];
    let mut sum_c = vec![0f64; c];
    let cr = raw.crop;
    for y in (cr.y as usize..(cr.y + cr.h) as usize).take(h) {
        for x in (cr.x as usize..(cr.x + cr.w) as usize).take(w) {
            let mut any = false;
            for k in 0..c {
                let i = (y * w + x) * c + k;
                if raw.data[i] != o.data[i] {
                    per_c[k] += 1;
                    sum_c[k] += (raw.data[i] as f64 - o.data[i] as f64).abs();
                    any = true;
                }
            }
            if any {
                pts.push((x, y));
            }
        }
    }
    if pts.is_empty() {
        return;
    }
    let (x0, x1) = (
        pts.iter().map(|p| p.0).min().unwrap(),
        pts.iter().map(|p| p.0).max().unwrap(),
    );
    let (y0, y1) = (
        pts.iter().map(|p| p.1).min().unwrap(),
        pts.iter().map(|p| p.1).max().unwrap(),
    );
    let mean: Vec<String> = sum_c
        .iter()
        .zip(&per_c)
        .map(|(s, n)| format!("{:.1}", s / (*n).max(1) as f64))
        .collect();
    println!(
        "  mismatching pixels {} in x {x0}..={x1} y {y0}..={y1}; per channel {per_c:?}, mean |diff| {mean:?}",
        pts.len()
    );
    for period in [2usize, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64] {
        let mut hx = vec![0u64; period];
        let mut hy = vec![0u64; period];
        for &(x, y) in &pts {
            hx[x % period] += 1;
            hy[y % period] += 1;
        }
        let top = |v: &[u64]| {
            let mut i: Vec<usize> = (0..v.len()).collect();
            i.sort_by_key(|&k| std::cmp::Reverse(v[k]));
            i.iter().take(3).map(|&k| (k, v[k])).collect::<Vec<_>>()
        };
        println!("  period {period}: x {:?} y {:?}", top(&hx), top(&hy));
    }
    let first: Vec<_> = pts.iter().take(16).collect();
    println!("  first {first:?}");
    for &(px, py) in pts.iter().skip(pts.len() / 2).take(1) {
        for k in 0..c {
            println!("  channel {k} around {px},{py} (ours/theirs):");
            for y in py.saturating_sub(3)..(py + 5).min(h) {
                let mut l = String::new();
                for x in px.saturating_sub(3)..(px + 5).min(w) {
                    let i = (y * w + x) * c + k;
                    l += &format!(" {:>5}/{:<5}", raw.data[i], o.data[i]);
                }
                println!("   {y}:{l}");
            }
        }
    }
}

/// Calibrated photosites (cpp 1) against a LibRaw bilinear develop in camera colour: each
/// visible photosite's own channel. LibRaw's scale to 16 bits is fitted. Returns (photosites,
/// over 1, over 16, max, mean |diff|, scale) in our units.
fn compare_cfa(raw: &RawImage, o: &Plane) -> Option<(u64, u64, u64, f64, f64, f64)> {
    let cfa = raw.cfa.as_ref()?;
    let (w, cr) = (raw.width as usize, raw.crop);
    // LibRaw moves an odd visible origin by one photosite (and loses that row or column).
    let (dx, dy) = (cr.w as usize - o.w, cr.h as usize - o.h);
    if dx > 1 || dy > 1 || o.c < 3 {
        return None;
    }
    let site = |y: usize, x: usize| -> (f64, f64) {
        let (ry, rx) = (y + cr.y as usize + dy, x + cr.x as usize + dx);
        let ch = cfa.color(ry as u32, rx as u32).min(2) as usize;
        let ours = raw.data[ry * w + rx] as f64 - raw.black[(ry & 1) * 2 + (rx & 1)] as f64;
        (ours, o.data[(y * o.w + x) * o.c + ch] as f64)
    };
    let (mut st, mut so) = (0f64, 0f64);
    for y in (0..o.h).step_by(7) {
        for x in (0..o.w).step_by(5) {
            let (a, b) = site(y, x);
            if b < 60000.0 && a > 64.0 {
                st += a * b;
                so += a * a;
            }
        }
    }
    let k = st / so.max(1.0);
    let (mut n, mut over1, mut over16, mut maxd, mut sum) = (0u64, 0u64, 0u64, 0f64, 0f64);
    for y in 0..o.h {
        for x in 0..o.w {
            let (a, b) = site(y, x);
            if b >= 65535.0 || a * k >= 65000.0 {
                continue;
            }
            let d = (a - b / k).abs();
            n += 1;
            over1 += (d > 1.5) as u64;
            over16 += (d > 16.0) as u64;
            maxd = maxd.max(d);
            sum += d;
        }
    }
    if std::env::var_os("RAWREST_DIAG").is_some() {
        // Per quadrant of a 4x4 split and level band: fitted scale and mean signed diff.
        for qy in 0..4 {
            let mut s = String::new();
            for qx in 0..4 {
                // Least squares theirs/k = g * ours + c.
                let (mut sx, mut sy, mut sxx, mut sxy, mut n) = (0f64, 0f64, 0f64, 0f64, 0f64);
                for y in (qy * o.h / 4..(qy + 1) * o.h / 4).step_by(3) {
                    for x in (qx * o.w / 4..(qx + 1) * o.w / 4).step_by(3) {
                        let (a, b) = site(y, x);
                        if b < 60000.0 && a > 64.0 {
                            let b = b / k;
                            sx += a;
                            sy += b;
                            sxx += a * a;
                            sxy += a * b;
                            n += 1.0;
                        }
                    }
                }
                let g = (n * sxy - sx * sy) / (n * sxx - sx * sx).max(1e-9);
                let c = (sy - g * sx) / n.max(1.0);
                s += &format!(" {g:.4}{c:+.1}");
            }
            println!("  quad row {qy}:{s}");
        }
        let mut by_col = std::collections::HashMap::<usize, (u32, f64)>::new();
        let mut by_row = std::collections::HashMap::<usize, u32>::new();
        let mut samples = Vec::new();
        for y in 0..o.h {
            for x in 0..o.w {
                let (a, b) = site(y, x);
                if b >= 65535.0 || a * k >= 65000.0 {
                    continue;
                }
                let d = b / k - a;
                if d.abs() > 16.0 {
                    let e = by_col.entry(x + cr.x as usize + dx).or_default();
                    e.0 += 1;
                    e.1 += d;
                    *by_row.entry(y + cr.y as usize + dy).or_default() += 1;
                    if samples.len() < 12 {
                        samples.push((y + cr.y as usize + dy, x + cr.x as usize + dx, a, b / k));
                    }
                }
            }
        }
        let mut cols: Vec<_> = by_col.into_iter().collect();
        cols.sort_by_key(|c| std::cmp::Reverse(c.1.0));
        let mut rows: Vec<_> = by_row.into_iter().collect();
        rows.sort_by_key(|c| std::cmp::Reverse(c.1));
        println!(
            "  worst columns (raw x, count, mean diff): {:?}",
            &cols[..cols.len().min(12)]
        );
        println!(
            "  worst rows (raw y, count): {:?}",
            &rows[..rows.len().min(8)]
        );
        println!("  samples (y, x, ours, theirs): {samples:?}");
        for &(sy, sx, _, _) in samples.iter().take(3) {
            for yy in sy.saturating_sub(2)..sy + 3 {
                let mut l = String::new();
                for xx in sx.saturating_sub(2)..sx + 3 {
                    let ours = raw.data[yy * w + xx];
                    let (Some(oy), Some(ox)) = (
                        yy.checked_sub(cr.y as usize + dy).filter(|&v| v < o.h),
                        xx.checked_sub(cr.x as usize + dx).filter(|&v| v < o.w),
                    ) else {
                        continue;
                    };
                    let ch = cfa.color(yy as u32, xx as u32).min(2) as usize;
                    let th = o.data[(oy * o.w + ox) * o.c + ch] as f64 / k;
                    l += &format!(" {ours:>6}/{th:>7.1}");
                }
                println!("   {yy}:{l}");
            }
        }
        for band in [
            (0.0, 256.0),
            (256.0, 1024.0),
            (1024.0, 4096.0),
            (4096.0, 16384.0),
            (16384.0, 65536.0),
        ] {
            let (mut sd, mut n) = (0f64, 0f64);
            for y in (0..o.h).step_by(3) {
                for x in (0..o.w).step_by(3) {
                    let (a, b) = site(y, x);
                    if a >= band.0 && a < band.1 && b < 65535.0 {
                        sd += b / k - a;
                        n += 1.0;
                    }
                }
            }
            println!(
                "  level {band:?}: mean signed diff {:+.2} over {n}",
                sd / n.max(1.0)
            );
        }
    }
    Some((n, over1, over16, maxd, sum / n.max(1) as f64, k))
}

/// 24x24 grid of channel means of an interleaved RGB image (`c` channels: 1 is grey).
fn grid(w: usize, h: usize, c: usize, d: &[u16]) -> Vec<[f64; 3]> {
    const G: usize = 24;
    let mut out = vec![[0f64; 3]; G * G];
    let mut cnt = vec![0f64; G * G];
    for y in 0..h {
        for x in 0..w {
            let i = (y * G / h) * G + x * G / w;
            for k in 0..3 {
                out[i][k] += d[(y * w + x) * c + k.min(c - 1)] as f64;
            }
            cnt[i] += 1.0;
        }
    }
    for (o, n) in out.iter_mut().zip(&cnt) {
        for v in o.iter_mut() {
            *v /= n.max(1.0);
        }
    }
    out
}

/// (R/G and B/G of ours over theirs, luminance correlation of the grids).
fn compare_dev(ours: &RgbImage, o: &Plane) -> (f64, f64, f64) {
    let a = grid(ours.width as usize, ours.height as usize, 3, &ours.data);
    let b = grid(o.w, o.h, o.c.max(1), &o.data);
    let sum = |g: &[[f64; 3]], c: usize| g.iter().map(|p| p[c]).sum::<f64>();
    let ratio = |c: usize| (sum(&a, c) / sum(&a, 1)) / (sum(&b, c) / sum(&b, 1));
    let la: Vec<f64> = a.iter().map(|p| p[0] + p[1] + p[2]).collect();
    let lb: Vec<f64> = b.iter().map(|p| p[0] + p[1] + p[2]).collect();
    let n = la.len() as f64;
    let (ma, mb) = (la.iter().sum::<f64>() / n, lb.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in la.iter().zip(&lb) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    (ratio(0), ratio(2), sab / (saa * sbb).sqrt().max(1e-9))
}

/// Corner over centre brightness (green, linear) of an interleaved RGB image: the mean of the
/// four corner boxes (a tenth of each side) over the mean of the central box (a fifth).
fn falloff(w: usize, h: usize, d: &[u16]) -> f64 {
    let mean = |x0: usize, y0: usize, bw: usize, bh: usize| {
        let mut s = 0f64;
        for y in y0..y0 + bh {
            for x in x0..x0 + bw {
                s += d[(y * w + x) * 3 + 1] as f64;
            }
        }
        s / (bw * bh).max(1) as f64
    };
    let (cw, ch) = (w / 10, h / 10);
    let corners = (mean(0, 0, cw, ch)
        + mean(w - cw, 0, cw, ch)
        + mean(0, h - ch, cw, ch)
        + mean(w - cw, h - ch, cw, ch))
        / 4.0;
    corners / mean(w * 2 / 5, h * 2 / 5, w / 5, h / 5).max(1.0)
}

/// Our corner falloff against the largest preview's (sRGB decoded to linear).
fn falloff_vs_preview(ours: &RgbImage, data: &[u8], info: &specular_raw::RawInfo) -> String {
    let Some(p) = info.largest_preview() else {
        return "n/a".into();
    };
    let Ok(img) = specular_raw::decode_preview(data, p, 512) else {
        return "n/a".into();
    };
    let lin: Vec<u16> = img
        .rgba
        .chunks_exact(4)
        .flat_map(|p| {
            p[..3].iter().map(|&v| {
                let v = v as f64 / 255.0;
                let l = if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                };
                (l * 65535.0) as u16
            })
        })
        .collect();
    format!(
        "corner/centre {:.3} vs preview {:.3}",
        falloff(ours.width as usize, ours.height as usize, &ours.data),
        falloff(img.width as usize, img.height as usize, &lin)
    )
}

/// R/G and B/G of our develop's mean linear colour over the largest embedded preview's.
fn vs_preview(ours: &RgbImage, data: &[u8], info: &specular_raw::RawInfo) -> String {
    preview_ratios(ours, data, info).map_or("n/a".into(), |(r, b)| format!("R/G {r:.3} B/G {b:.3}"))
}

/// [`vs_preview`] as numbers.
fn preview_ratios(
    ours: &RgbImage,
    data: &[u8],
    info: &specular_raw::RawInfo,
) -> Option<(f64, f64)> {
    let p = info.largest_preview()?;
    let img = specular_raw::decode_preview(data, p, 512).ok()?;
    let lin = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let mut theirs = [0f64; 3];
    for px in img.rgba.chunks_exact(4) {
        for c in 0..3 {
            theirs[c] += lin(px[c]);
        }
    }
    let mut mine = [0f64; 3];
    for px in ours.data.chunks_exact(3) {
        for c in 0..3 {
            mine[c] += px[c] as f64;
        }
    }
    Some((
        (mine[0] / mine[1]) / (theirs[0] / theirs[1]),
        (mine[2] / mine[1]) / (theirs[2] / theirs[1]),
    ))
}

/// With `RAWREST_PNG=DIR`: a linear RGB image as an sRGB PNG, exposed so that the 99th
/// percentile of green sits at 90%, at most 1024 pixels wide.
fn dump_png(name: &str, w: usize, h: usize, d: &[u16]) {
    let Some(dir) = std::env::var_os("RAWREST_PNG") else {
        return;
    };
    let step = w.div_ceil(1024).max(1);
    let (ow, oh) = (w / step, h / step);
    let mut g: Vec<u16> = d.chunks_exact(3).map(|p| p[1]).collect();
    g.sort_unstable();
    let scale = 0.9 / (g[g.len() * 99 / 100].max(1) as f64);
    let enc = |v: u16| {
        let l = (v as f64 * scale).clamp(0.0, 1.0);
        let s = if l <= 0.0031308 {
            12.92 * l
        } else {
            1.055 * l.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0 + 0.5) as u8
    };
    let mut px = Vec::with_capacity(ow * oh * 3);
    for y in 0..oh {
        for x in 0..ow {
            let i = (y * step * w + x * step) * 3;
            px.extend([enc(d[i]), enc(d[i + 1]), enc(d[i + 2])]);
        }
    }
    let png =
        specular_png::encode(ow as u32, oh as u32, specular_png::ColorType::Rgb, &px).expect("png");
    let file: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    std::fs::write(Path::new(&dir).join(file + ".png"), png).expect("write png");
}

#[test]
#[ignore = "oracle runner: needs tools/corpus/fetch-rawrest.sh and SPECULAR_RAWPY"]
fn rawrest_vs_rawpy() {
    let Ok(py) = std::env::var("SPECULAR_RAWPY") else {
        eprintln!("SPECULAR_RAWPY unset; skipping");
        return;
    };
    let root = std::env::var_os("SPECULAR_RAWREST_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/rawrest")
        });
    let only = std::env::var("SPECULAR_RAWREST_ONLY").unwrap_or_default();
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rawrest");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut paths = Vec::new();
    files(&root, &mut paths);
    paths.sort();
    let mut failures = Vec::new();
    for p in paths {
        let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
        if !rel.contains(&only) {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let file = match specular_raw::parse(&data) {
            Ok(f) => f,
            Err(e) => {
                println!("{rel}: parse error {e}");
                println!("ORACLE\tRAW/{}\t{rel}\tfail", ext_of(&rel));
                failures.push(rel);
                continue;
            }
        };
        let info = file.info();
        let fmt = info.format.map_or("?", |f| f.name());
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let verdict = |status: &str| println!("ORACLE\tRAW/{fmt}\t{rel}\t{status}");
        if let Some(p) = info.largest_preview()
            && let Ok(img) = specular_raw::decode_preview(&data, p, 0)
            && std::env::var_os("RAWREST_PNG").is_some()
        {
            // As 16-bit "linear" RGB of the sRGB bytes, so dump_png's curve roughly undoes.
            let lin: Vec<u16> = img
                .rgba
                .chunks_exact(4)
                .flat_map(|p| {
                    p[..3].iter().map(|&v| {
                        let v = v as f64 / 255.0;
                        let l = if v <= 0.04045 {
                            v / 12.92
                        } else {
                            ((v + 0.055) / 1.055).powf(2.4)
                        };
                        (l * 65535.0) as u16
                    })
                })
                .collect();
            dump_png(
                &format!("{rel}.preview"),
                img.width as usize,
                img.height as usize,
                &lin,
            );
        }
        let t = Instant::now();
        let raw = match file.decode_raw(&data, &DecodeOptions::default()) {
            Ok(r) => r,
            Err(e) => {
                println!("{rel}: [{fmt}] decode error {e}");
                if matches!(e, specular_raw::RawError::Unsupported(_)) {
                    verdict("skip");
                } else {
                    verdict("fail");
                    failures.push(rel);
                }
                continue;
            }
        };
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let mut line = format!(
            "{rel}: [{fmt}] {:?} {}x{}x{} crop {},{} {}x{} in {ms:.0} ms",
            info.model,
            raw.width,
            raw.height,
            raw.cpp,
            raw.crop.x,
            raw.crop.y,
            raw.crop.w,
            raw.crop.h
        );
        // Sensor values: before LibRaw's corrections (IIQ calibrations, X3F AF pixels) or as
        // ours for the formats whose raw_image is already final.
        // GoPro GPR: LibRaw has no VC-5 decoder, so the GPR SDK's own (SPECULAR_GPR_TOOLS, its
        // gpr_tools built locally) converts the file to a plain DNG that LibRaw reads.
        let oracle_file = match std::env::var_os("SPECULAR_GPR_TOOLS") {
            Some(tool) if rel.to_ascii_lowercase().ends_with(".gpr") => {
                let dng = tmp.join("gpr.dng");
                let ok = Command::new(tool)
                    .arg("-i")
                    .arg(&p)
                    .arg("-o")
                    .arg(&dng)
                    .output()
                    .is_ok_and(|o| o.status.success());
                if ok { dng } else { p.clone() }
            }
            _ => p.clone(),
        };
        match oracle(&py, &oracle_file, &tmp, "raw") {
            Ok(o) => {
                // Before the vendor calibrations LibRaw's raw_image does not have.
                let mosaic = if matches!(
                    info.format,
                    Some(specular_raw::RawFormat::Iiq | specular_raw::RawFormat::X3f)
                ) {
                    file.decode_raw_as_stored(&data, &DecodeOptions::default())
                        .unwrap()
                } else {
                    file.decode_raw_mosaic(&data, &DecodeOptions::default())
                        .unwrap()
                };
                if std::env::var_os("RAWREST_DUMP").is_some() {
                    let w = mosaic.width as usize;
                    for y in 0..4 {
                        println!(
                            "  row {y} ours {:?}\n        theirs {:?}",
                            &mosaic.data[y * w..y * w + 12],
                            &o.plane.data[y * o.plane.w..y * o.plane.w + 12]
                        );
                    }
                }
                if let Some((b, n, m, dx, dy)) = compare_raw(&mosaic, &o.plane) {
                    line += &format!("; raw {b}/{n} mismatches (max {m}) at {dx},{dy}");
                } else {
                    line += &format!(
                        "; raw not comparable ({}x{}x{})",
                        o.plane.w, o.plane.h, o.plane.c
                    );
                }
                line += &format!(
                    "; black {:?} vs {:?}; white {} vs {}; wb {:?} vs {}",
                    raw.black,
                    o.nums("black"),
                    raw.white,
                    o.fact("white").unwrap_or("?"),
                    raw.wb,
                    o.fact("wb").unwrap_or("?")
                );
            }
            Err(e) => line += &format!("; no raw oracle ({e})"),
        }
        if info.format == Some(specular_raw::RawFormat::Iiq) {
            match oracle(&py, &p, &tmp, "cfa") {
                Ok(o) => match compare_cfa(&raw, &o.plane) {
                    Some((n, o1, o16, m, mean, k)) => {
                        line += &format!(
                            "; calibrated {n} sites: {o1} off by >1, {o16} by >16, max {m:.0}, \
                             mean {mean:.3} (LibRaw scale {k:.4})"
                        )
                    }
                    None => {
                        line += &format!(
                            "; calibrated: not comparable ({}x{}x{} vs crop {:?})",
                            o.plane.w, o.plane.h, o.plane.c, raw.crop
                        )
                    }
                },
                Err(e) => line += &format!("; no cfa oracle ({e})"),
            }
        }
        let half = DecodeOptions {
            size: OutputSize::Half,
            ..Default::default()
        };
        match specular_raw::develop(&raw, &half) {
            Ok(img) => {
                dump_png(
                    &format!("{rel}.ours"),
                    img.width as usize,
                    img.height as usize,
                    &img.data,
                );
                line += &format!(
                    "; matrix {:?} wb {:?}; vs preview {}; {}",
                    img.matrix_source,
                    img.wb_source,
                    vs_preview(&img, &data, info),
                    falloff_vs_preview(&img, &data, info)
                );
                // Quattro colour comes from the calibration alone (LibRaw has no matrix for
                // the sd bodies): within 15% of the camera's own preview (the sd Quattro H
                // was magenta, R/G 18.7 and B/G 38.8, before its digital layer gain).
                if info.model.contains("Quattro")
                    && let Some((r, b)) = preview_ratios(&img, &data, info)
                    && !((0.85..1.15).contains(&r) && (0.85..1.15).contains(&b))
                {
                    line += "; colour off the camera's preview";
                    failures.push(rel.clone());
                }
                match oracle(&py, &p, &tmp, "half") {
                    Ok(o) => {
                        dump_png(
                            &format!("{rel}.libraw"),
                            o.plane.w,
                            o.plane.h,
                            &o.plane.data,
                        );
                        let (r, b, c) = compare_dev(&img, &o.plane);
                        line += &format!("; vs LibRaw R/G {r:.3} B/G {b:.3} corr {c:.3}");
                    }
                    Err(e) => line += &format!("; no develop oracle ({e})"),
                }
            }
            Err(e) => {
                line += &format!("; develop error {e}");
                failures.push(rel.clone());
            }
        }
        println!("{line}");
        verdict(if failures.last() == Some(&rel) {
            "fail"
        } else {
            "pass"
        });
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(failures.is_empty(), "failures: {failures:?}");
}
