//! Renders every page of an OpenDocument drawing or presentation to PPM, for the oracle
//! comparison (tools/corpus/design2-odf-oracle.py).
//!
//! Usage: odf-render FILE OUT_DIR [DPI]  → OUT_DIR/<stem>_p<N>.ppm (N from 1). Embedded
//! images decode when they are PNG (the app decodes every format).

use std::io::Write;
use std::sync::Arc;

use specular_drawpage::{ImageDecoder, RenderRequest};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: odf-render FILE OUT_DIR [DPI]");
        std::process::exit(2);
    }
    let dpi: f64 = args.get(3).and_then(|d| d.parse().ok()).unwrap_or(48.0);
    let data = std::fs::read(&args[1]).expect("read");
    let decode: ImageDecoder = Arc::new(|b: &[u8]| {
        if b.starts_with(&[0xff, 0xd8]) {
            return jpeg_rgba(b);
        }
        let img = specular_png::decode(b, &Default::default()).ok()?;
        Some((img.width, img.height, img.to_rgba8()))
    });
    let t = std::time::Instant::now();
    let doc = match specular_odf::open_graphics(&data, Some(decode)) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: {e}", args[1]);
            std::process::exit(1);
        }
    };
    let opened = t.elapsed();
    if std::env::var_os("ODF_DUMP").is_some() {
        for (i, p) in doc.pages.iter().enumerate() {
            for op in &p.ops {
                if let specular_drawpage::Op::Text(t) = op {
                    let first: String = t
                        .paragraphs
                        .iter()
                        .flat_map(|p| p.runs.iter().map(|r| r.text.as_str()))
                        .collect::<String>()
                        .chars()
                        .take(30)
                        .collect();
                    eprintln!(
                        "p{} text {:?} {:.1}x{:.1} wrap {} {:?} size {:?} {:?}",
                        i + 1,
                        t.transform.translation(),
                        t.width,
                        t.height,
                        t.wrap,
                        t.valign,
                        t.paragraphs.first().map(|p| (
                            p.indent_left,
                            p.indent_first,
                            p.bullet.clone(),
                            p.runs.first().map(|r| (r.size, r.family.clone()))
                        )),
                        first
                    );
                }
            }
        }
        eprintln!("{:?}", doc.facts);
    }
    let stem = std::path::Path::new(&args[1])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("out")
        .to_string();
    let k = dpi / 72.0;
    let t = std::time::Instant::now();
    for (i, p) in doc.pages.iter().enumerate() {
        let (w, h) = (
            (p.size.0 * k).round().max(1.0) as u32,
            (p.size.1 * k).round().max(1.0) as u32,
        );
        let px = specular_drawpage::render(
            p,
            RenderRequest {
                scale_x: f64::from(w) / p.size.0,
                scale_y: f64::from(h) / p.size.1,
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &|| false,
        )
        .expect("render");
        let path = format!("{}/{stem}_p{}.ppm", args[2], i + 1);
        let mut f = std::fs::File::create(&path).expect("create");
        write!(f, "P6\n{w} {h}\n255\n").expect("write");
        let rgb: Vec<u8> = px
            .chunks_exact(4)
            .flat_map(|c| [c[0], c[1], c[2]])
            .collect();
        f.write_all(&rgb).expect("write");
    }
    eprintln!(
        "{}: {} pages, open {:.1} ms, render {:.1} ms; {}",
        args[1],
        doc.pages.len(),
        opened.as_secs_f64() * 1e3,
        t.elapsed().as_secs_f64() * 1e3,
        doc.describe(0)
    );
}

/// A JPEG as straight RGBA8 (chroma upsampled by repetition).
fn jpeg_rgba(b: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    use specular_jpeg::PlanesLayout as L;
    let d = specular_jpeg::decode(b, &Default::default()).ok()?;
    let (w, h) = (d.width as usize, d.height as usize);
    let maxh = d.planes.iter().map(|p| p.h_samp).max()?.max(1) as usize;
    let maxv = d.planes.iter().map(|p| p.v_samp).max()?.max(1) as usize;
    let at = |i: usize, x: usize, y: usize| -> f32 {
        let p = &d.planes[i];
        let (fx, fy) = (
            maxh / p.h_samp.max(1) as usize,
            maxv / p.v_samp.max(1) as usize,
        );
        let (px, py) = (
            (x / fx).min(p.width as usize - 1),
            (y / fy).min(p.height as usize - 1),
        );
        f32::from(p.data[py * p.stride + px])
    };
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let px = match d.layout {
                L::Gray => {
                    let v = at(0, x, y) as u8;
                    [v, v, v, 255]
                }
                L::Rgb => [at(0, x, y) as u8, at(1, x, y) as u8, at(2, x, y) as u8, 255],
                L::Rgba => {
                    let p = &d.planes[0];
                    let i = y * p.stride + x * 4;
                    [p.data[i], p.data[i + 1], p.data[i + 2], p.data[i + 3]]
                }
                L::YCbCr => {
                    let (yy, cb, cr) = (at(0, x, y), at(1, x, y) - 128.0, at(2, x, y) - 128.0);
                    let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                    [
                        c(yy + 1.402 * cr),
                        c(yy - 0.344_136 * cb - 0.714_136 * cr),
                        c(yy + 1.772 * cb),
                        255,
                    ]
                }
            };
            out.extend_from_slice(&px);
        }
    }
    Some((d.width, d.height, out))
}
