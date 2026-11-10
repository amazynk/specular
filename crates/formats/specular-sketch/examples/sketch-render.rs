//! Renders Sketch documents to PPM, for visual checks and the preview oracle
//! (`tools/corpus/design2-sketch-oracle.py`).
//!
//! Usage: `sketch-render DPI OUT_DIR FILE...` → `OUT_DIR/<stem>-<page>.ppm` (pages from 1, one
//! per artboard, at DPI with 72 Sketch points per inch), and `OUT_DIR/<stem>-overview.ppm`: the
//! page last viewed drawn whole at the size of the embedded `previews/preview.png`, which is
//! copied beside it as `<stem>-preview.png`. One line per output on stdout:
//! `file page width height open_ms render_ms` (or `file ERROR why`).
//!
//! Images are decoded here with the in-tree PNG and JPEG decoders only.

use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use specular_drawpage::{ImageDecoder, Page, RenderRequest};

fn decoder() -> ImageDecoder {
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

fn write_ppm(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).expect("output file"));
    write!(f, "P6\n{w} {h}\n255\n").expect("write");
    for p in rgba.chunks_exact(4) {
        f.write_all(&p[..3]).expect("write");
    }
}

fn render(page: &Page, pw: u32, ph: u32) -> (Vec<u8>, f64) {
    let t = Instant::now();
    let req = RenderRequest {
        scale_x: f64::from(pw) / page.size.0,
        scale_y: f64::from(ph) / page.size.1,
        x: 0,
        y: 0,
        width: pw,
        height: ph,
    };
    let px = specular_drawpage::render(page, req, &|| false).expect("render");
    (px, t.elapsed().as_secs_f64() * 1e3)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: sketch-render DPI OUT_DIR FILE...");
        std::process::exit(2);
    }
    let dpi: f64 = args[1].parse().expect("DPI");
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).expect("output directory");
    for file in &args[3..] {
        let stem = std::path::Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("doc")
            .to_owned();
        let Ok(bytes) = std::fs::read(file) else {
            println!("{stem} ERROR unreadable");
            continue;
        };
        let t0 = Instant::now();
        let doc = match specular_sketch::open_sketch(&bytes, Some(decoder())) {
            Ok(d) => d,
            Err(e) => {
                println!("{stem} ERROR {e}");
                continue;
            }
        };
        let open_ms = t0.elapsed().as_secs_f64() * 1e3;
        // SKETCH_PAGES: most pages written per file (design kits have thousands of symbols).
        let max: usize = std::env::var("SKETCH_PAGES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        println!("{stem} pages {}", doc.pages.len());
        for (i, p) in doc.pages.iter().enumerate().take(max) {
            let k = dpi / 72.0;
            let (pw, ph) = (
                ((p.size.0 * k).round() as u32).clamp(1, 16384),
                ((p.size.1 * k).round() as u32).clamp(1, 16384),
            );
            let (px, ms) = render(p, pw, ph);
            write_ppm(&out.join(format!("{stem}-{}.ppm", i + 1)), pw, ph, &px);
            println!("{stem} {} {pw} {ph} {open_ms:.1} {ms:.1}", i + 1);
        }
        if !doc.facts.not_drawn.is_empty() {
            println!("{stem} not drawn: {:?}", doc.facts.not_drawn);
        }
        // The overview against the embedded preview.
        let Ok(prev) = specular_sketch::sketch_preview(&bytes) else {
            continue;
        };
        let prev = prev.get(&bytes).to_vec();
        let Ok(info) = specular_png::read_info(&prev) else {
            continue;
        };
        std::fs::write(out.join(format!("{stem}-preview.png")), &prev).expect("write");
        let Ok((pages, cur)) = specular_sketch::open_sketch_overview(&bytes, Some(decoder()))
        else {
            continue;
        };
        if let Some(p) = pages.get(cur) {
            let (px, ms) = render(p, info.width, info.height);
            write_ppm(
                &out.join(format!("{stem}-overview.ppm")),
                info.width,
                info.height,
                &px,
            );
            println!(
                "{stem} overview {} {} {open_ms:.1} {ms:.1} page {:.0}x{:.0}",
                info.width, info.height, p.size.0, p.size.1
            );
        }
    }
}
