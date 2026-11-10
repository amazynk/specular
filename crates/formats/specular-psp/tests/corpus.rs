//! The PaintShop Pro corpus (tools/corpus/fetch-editors-psp.sh) against GIMP 3's importer and
//! against each file's own composite image. Skips when the corpus is absent.

use specular_psp as sp;
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_PSP_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/editors/psp")
        });
    p.is_dir().then_some(p)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            files(&p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "psp" | "pspimage" | "tub"))
        {
            out.push(p);
        }
    }
}

struct GLayer {
    name: String,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    group: bool,
    rgba: Vec<u8>,
}

struct GRef {
    w: u32,
    h: u32,
    comp: Vec<u8>,
    layers: Vec<GLayer>,
}

fn read_gref(p: &Path) -> Option<GRef> {
    let d = std::fs::read(p).ok()?;
    let u32_at = |i: usize| u32::from_le_bytes(d[i..i + 4].try_into().unwrap());
    if &d[..4] != b"PGRF" {
        return None;
    }
    let (w, h, n) = (u32_at(4), u32_at(8), u32_at(12));
    let mut pos = 16 + (w * h * 4) as usize;
    let comp = d[16..pos].to_vec();
    let mut layers = Vec::new();
    for _ in 0..n {
        let ln = u32_at(pos) as usize;
        pos += 4;
        let name = String::from_utf8_lossy(&d[pos..pos + ln]).into_owned();
        pos += ln;
        let x = u32_at(pos) as i32;
        let y = u32_at(pos + 4) as i32;
        let lw = u32_at(pos + 8);
        let lh = u32_at(pos + 12);
        let group = d[pos + 17] != 0;
        pos += 20;
        let len = (lw * lh * 4) as usize;
        let rgba = d[pos..pos + len].to_vec();
        pos += len;
        layers.push(GLayer {
            name,
            x,
            y,
            w: lw,
            h: lh,
            group,
            rgba,
        });
    }
    Some(GRef { w, h, comp, layers })
}

fn rgba8(img: &sp::Image) -> Vec<u8> {
    match img.format {
        sp::PixelFormat::Rgba8 => img.data.clone(),
        sp::PixelFormat::Gray8 => img.data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        sp::PixelFormat::Rgba16 => img
            .data
            .chunks_exact(2)
            .map(|b| (u16::from_ne_bytes([b[0], b[1]]) >> 8) as u8)
            .collect(),
    }
}

/// Pixels differing by more than `tol` (premultiplied, so colours under zero alpha do not
/// count).
fn diff(a: &[u8], b: &[u8], tol: i32) -> (usize, i32) {
    let mut bad = 0;
    let mut max = 0;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let pm = |c: &[u8], k: usize| i32::from(c[k]) * i32::from(c[3]) / 255;
        let mut m = (i32::from(p[3]) - i32::from(q[3])).abs();
        for k in 0..3 {
            m = m.max((pm(p, k) - pm(q, k)).abs());
        }
        max = max.max(m);
        if m > tol {
            bad += 1;
        }
    }
    (bad, max)
}

#[test]
fn corpus_against_gimp_and_own_composite() {
    let Some(dir) = corpus() else {
        eprintln!("PSP corpus absent; run tools/corpus/fetch-editors-psp.sh");
        return;
    };
    let mut list = Vec::new();
    files(&dir, &mut list);
    let (mut parsed, mut layer_ok, mut layer_bad, mut comp_ok, mut comp_bad) = (0, 0, 0, 0, 0);
    let (mut self_ok, mut self_bad, mut failures) = (0, 0, Vec::new());
    for p in &list {
        let data = std::fs::read(p).unwrap();
        let name = p.strip_prefix(&dir).unwrap_or(p).display().to_string();
        let psp = match sp::parse(&data) {
            Ok(x) => x,
            Err(e) => {
                println!("{name}: parse failed: {e}");
                failures.push(name);
                continue;
            }
        };
        parsed += 1;
        let i = psp.info();
        let t = std::time::Instant::now();
        let shown = psp.render(&data, &sp::Options::default());
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let lay_opts = sp::Options {
            prefer_composite: false,
            ..Default::default()
        };
        let layered = psp.render(&data, &lay_opts);
        let thumb = psp.thumbnail(&data).map(|r| r.map(|t| (t.width, t.height)));
        let mut line = format!(
            "{name}: v{}.{} {}×{} {}bpp {:?} {} layers comp={} thumb={:?} {:.1} ms",
            i.major,
            i.minor,
            i.width,
            i.height,
            i.bit_depth,
            i.compression,
            i.layer_count,
            i.has_composite,
            thumb,
            ms
        );
        if let Some(r) = &i.not_drawn {
            line += &format!(" ({r})");
        }
        if let Err(e) = &shown {
            line += &format!(" RENDER ERROR {e}");
            failures.push(name.clone());
        }
        // Self-oracle: the stored composite against the layers composited.
        if i.has_composite
            && i.not_drawn.is_none()
            && let (Ok(a), Ok(b)) = (&shown, &layered)
        {
            let (bad, max) = diff(&rgba8(a), &rgba8(b), 2);
            line += &format!(" | self {bad} px >2 (max {max})");
            if bad * 1000 <= (a.width * a.height) as usize {
                self_ok += 1;
            } else {
                self_bad += 1;
            }
        }
        let gref = PathBuf::from(format!("{}.gref", p.display()));
        if let Some(g) = read_gref(&gref) {
            // Layers: GIMP lists them top-most first like we do.
            let ours = psp.layers();
            let mut lbad = 0;
            for (k, gl) in g.layers.iter().enumerate() {
                let Some(l) = ours.get(k) else {
                    lbad += 1;
                    continue;
                };
                if gl.group || !l.has_pixels || gl.w * gl.h <= 1 {
                    continue;
                }
                if gl.name != l.name {
                    lbad += 1;
                    line += &format!(" [name {k}: {:?} vs {:?}]", l.name, gl.name);
                    continue;
                }
                let Ok(img) = psp.render_layer(&data, k, &lay_opts) else {
                    lbad += 1;
                    continue;
                };
                let ours8 = rgba8(&img);
                let op = u32::from(l.opacity);
                let mut bad = 0;
                for ly in 0..gl.h as i32 {
                    for lx in 0..gl.w as i32 {
                        let (cx, cy) = (gl.x + lx, gl.y + ly);
                        if cx < 0 || cy < 0 || cx >= img.width as i32 || cy >= img.height as i32 {
                            continue;
                        }
                        let gi = ((ly as u32 * gl.w + lx as u32) * 4) as usize;
                        let oi = ((cy as u32 * img.width + cx as u32) * 4) as usize;
                        let ga = (u32::from(gl.rgba[gi + 3]) * op + 127) / 255;
                        let mut q = [gl.rgba[gi], gl.rgba[gi + 1], gl.rgba[gi + 2], ga as u8];
                        if ga == 0 {
                            q = [0; 4];
                        }
                        let (b, _) = diff(&ours8[oi..oi + 4], &q, 1);
                        bad += b;
                    }
                }
                if bad > 0 {
                    lbad += 1;
                    line += &format!(" [layer {k} {:?}: {bad} px]", l.name);
                }
            }
            if lbad == 0 {
                layer_ok += 1;
            } else {
                layer_bad += 1;
            }
            line += &format!(" | gimp layers {} bad", lbad);
            if let Ok(b) = &layered
                && (b.width, b.height) == (g.w, g.h)
            {
                let (bad, max) = diff(&rgba8(b), &g.comp, 2);
                line += &format!(" | gimp composite {bad} px >2 (max {max})");
                if bad * 1000 <= (g.w * g.h) as usize {
                    comp_ok += 1;
                } else {
                    comp_bad += 1;
                }
            }
        }
        println!("{line}");
    }
    println!(
        "PSP corpus: {} files, {parsed} parsed; GIMP layers {layer_ok} match / {layer_bad} differ; \
         GIMP composite {comp_ok} match / {comp_bad} differ; own composite {self_ok} match / {self_bad} differ",
        list.len()
    );
    assert!(failures.is_empty(), "failed: {failures:?}");
}
