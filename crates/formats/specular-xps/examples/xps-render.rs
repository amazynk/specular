//! Renders every page of XPS/OpenXPS files to PNG, for comparison with an oracle
//! (`tools/corpus/xps-oracle.py` runs MuPDF and compares).
//!
//! Usage: `xps-render DPI OUT_DIR FILE...` → `OUT_DIR/<stem>-<page>.png` (pages from 1) and
//! one line per page on stdout: `file page width height open_ms render_ms` (or `file ERROR why`).
//! With `XPS_TILE` set, also the time of a 4096² region of page 1 at 16× the DPI.
//!
//! Images are decoded here with the in-tree PNG and JPEG decoders only; TIFF and JPEG XR images are
//! left undrawn (the app decodes them through its registry). Pages with such images are listed
//! by the oracle script as "images not compared".

use std::sync::Arc;
use std::time::Instant;

use specular_xps::{Document, ImageDecoder, RenderRequest};
use vello_cpu::Pixmap;
use vello_cpu::peniko::ImageAlphaType;

fn png_decoder() -> ImageDecoder {
    Arc::new(|d: &[u8]| {
        if d.starts_with(b"\xff\xd8") {
            let p = specular_jpeg::decode(d, &specular_jpeg::DecodeOptions::default()).ok()?;
            return Some((p.width, p.height, specular_jpeg::planes_to_rgba(&p)));
        }
        if !d.starts_with(b"\x89PNG") {
            return None;
        }
        let img = specular_png::decode(d, &specular_png::DecodeOptions::default()).ok()?;
        Some((img.width, img.height, img.to_rgba8()))
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: xps-render DPI OUT_DIR FILE...");
        std::process::exit(2);
    }
    let dpi: f64 = args[1].parse().expect("DPI");
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).expect("output directory");
    for file in &args[3..] {
        let stem = std::path::Path::new(file)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("doc")
            .to_owned();
        let Ok(bytes) = std::fs::read(file) else {
            println!("{stem} ERROR unreadable");
            continue;
        };
        let t0 = Instant::now();
        let doc = match Document::open(Arc::new(bytes), Some(png_decoder())) {
            Ok(d) => d,
            Err(e) => {
                println!("{stem} ERROR {e}");
                continue;
            }
        };
        let open_ms = t0.elapsed().as_secs_f64() * 1e3;
        for p in 0..doc.page_count() {
            let Some((w, h)) = doc.page_size(p) else {
                println!("{stem} {} ERROR no size", p + 1);
                continue;
            };
            let k = dpi / 72.0;
            let (pw, ph) = ((w * k).round() as u32, (h * k).round() as u32);
            let t1 = Instant::now();
            let req = RenderRequest {
                page: p,
                scale_x: f64::from(pw) / w,
                scale_y: f64::from(ph) / h,
                x: 0,
                y: 0,
                width: pw.max(1),
                height: ph.max(1),
            };
            match doc.render(req, &|| false) {
                Ok(px) => {
                    let ms = t1.elapsed().as_secs_f64() * 1e3;
                    let pix = Pixmap::from_parts(
                        px,
                        pw as u16,
                        ph as u16,
                        vello_cpu::PixelMetadata::new(ImageAlphaType::Alpha, false),
                    );
                    let png = pix.into_png().expect("PNG encode");
                    std::fs::write(out.join(format!("{stem}-{}.png", p + 1)), png)
                        .expect("write PNG");
                    println!("{stem} {} {pw} {ph} {open_ms:.1} {ms:.1}", p + 1);
                }
                Err(e) => println!("{stem} {} ERROR {e}", p + 1),
            }
        }
        // With XPS_TILE set: a 4096² region from the middle of the first page at 16× the
        // DPI (deep zoom), timed.
        if std::env::var_os("XPS_TILE").is_some()
            && let Some((w, h)) = doc.page_size(0)
        {
            let k = dpi * 16.0 / 72.0;
            let (pw, ph) = ((w * k).round() as u32, (h * k).round() as u32);
            let (tw, th) = (pw.min(4096), ph.min(4096));
            let t = Instant::now();
            let r = doc.render(
                RenderRequest {
                    page: 0,
                    scale_x: f64::from(pw) / w,
                    scale_y: f64::from(ph) / h,
                    x: (pw - tw) / 2,
                    y: (ph - th) / 2,
                    width: tw,
                    height: th,
                },
                &|| false,
            );
            println!(
                "{stem} tile {tw}x{th} of {pw}x{ph} {:.1} ms {}",
                t.elapsed().as_secs_f64() * 1e3,
                if r.is_ok() { "ok" } else { "ERROR" }
            );
        }
    }
}
