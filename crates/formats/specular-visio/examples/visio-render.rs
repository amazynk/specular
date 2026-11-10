//! Renders every page of a VSDX to `<out>/<stem>_p<N>.ppm` at a given dpi (for the oracle
//! comparison), or writes a VSD's embedded preview to `<out>/<stem>.preview.<ext>`.
//!
//! Usage: visio-render FILE OUT_DIR [DPI]

use std::sync::Arc;

use specular_drawpage::{ImageDecoder, RenderRequest, render};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: visio-render FILE OUT_DIR [DPI]");
        std::process::exit(2);
    }
    let path = std::path::Path::new(&args[1]);
    let out = std::path::Path::new(&args[2]);
    let dpi: f64 = args.get(3).and_then(|d| d.parse().ok()).unwrap_or(96.0);
    let data = std::fs::read(path).expect("reads the file");
    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
    std::fs::create_dir_all(out).expect("creates the output folder");
    if specular_visio::probe_vsd_file(&data) {
        match specular_visio::vsd_preview(&data) {
            Ok(p) => {
                let ext = match p.kind {
                    specular_visio::PreviewKind::Wmf => "wmf",
                    specular_visio::PreviewKind::Emf => "emf",
                    specular_visio::PreviewKind::Dib => "dib",
                    specular_visio::PreviewKind::Png => "png",
                    specular_visio::PreviewKind::Jpeg => "jpg",
                };
                std::fs::write(out.join(format!("{stem}.preview.{ext}")), &p.bytes).unwrap();
                println!("{stem}: preview {ext}, {} bytes", p.bytes.len());
            }
            Err(e) => println!("{stem}: {e}"),
        }
        return;
    }
    let decode: ImageDecoder = Arc::new(|d: &[u8]| {
        let img = specular_png::decode(d, &specular_png::DecodeOptions::default()).ok()?;
        Some((img.width, img.height, img.to_rgba8()))
    });
    let t0 = std::time::Instant::now();
    let doc = match specular_visio::open_vsdx(&data, Some(decode)) {
        Ok(d) => d,
        Err(e) => {
            println!("{stem}: {e}");
            return;
        }
    };
    let opened = t0.elapsed();
    for (i, p) in doc.pages.iter().enumerate() {
        if std::env::var_os("VISIO_DUMP").is_some() {
            for op in &p.ops {
                match op {
                    specular_drawpage::Op::Fill { path, paint, .. } => {
                        eprintln!(
                            "fill {:?} {:?}",
                            specular_drawpage::Shape::bounding_box(path),
                            paint
                        )
                    }
                    specular_drawpage::Op::Stroke {
                        path,
                        stroke,
                        paint,
                    } => eprintln!(
                        "stroke {:?} w {:.2} {:?}",
                        specular_drawpage::Shape::bounding_box(path),
                        stroke.width,
                        paint
                    ),
                    specular_drawpage::Op::Text(t) => eprintln!(
                        "text {:?} {}x{} {:?}",
                        t.transform,
                        t.width,
                        t.height,
                        t.paragraphs
                            .iter()
                            .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
                            .collect::<Vec<_>>()
                    ),
                    o => eprintln!("{o:?}"),
                }
            }
        }
        let s = dpi / 72.0;
        let (w, h) = (
            (p.size.0 * s).round().max(1.0) as u32,
            (p.size.1 * s).round().max(1.0) as u32,
        );
        let t = std::time::Instant::now();
        let rgba = render(
            p,
            RenderRequest {
                scale_x: s,
                scale_y: s,
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &|| false,
        )
        .expect("renders");
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for px in rgba.chunks_exact(4) {
            ppm.extend_from_slice(&px[..3]);
        }
        std::fs::write(out.join(format!("{stem}_p{}.ppm", i + 1)), ppm).unwrap();
        println!(
            "{stem} p{}: {w}x{h}, {} ops, open {:.1} ms, render {:.1} ms; {}",
            i + 1,
            p.ops.len(),
            opened.as_secs_f64() * 1e3,
            t.elapsed().as_secs_f64() * 1e3,
            doc.describe(i)
        );
    }
}
