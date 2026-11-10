//! The OCR runners on files, for the oracle script (tools/corpus/ocr-oracle.py) and timings.
//! Images are binary PPM (P6) or PGM (P5): this crate has no image decoders.
//!
//! ```text
//! ocr pipeline DET_DIR REC OUT.tsv [--cpu] [--side N] [--crops DIR] IMAGE.ppm...
//!     REC is a recogniser export folder, or tess:PATH.traineddata
//!     OUT.tsv: image, 8 corner coordinates, confidence, text (one line per box)
//!     --crops DIR: also writes each box's line image as DIR/<image>-<n>.pgm
//! ocr tess TRAINEDDATA OUT.tsv LINE.pgm...
//!     OUT.tsv: name, text, confidence
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_infer::gpu::Context;
use specular_infer::ocr::lines::{Img8, Rgba, crop_quad};
use specular_infer::ocr::tess::Tesseract;
use specular_infer::ocr::{Detector, PaddleNet, Recognizer, Runner};

fn read_pnm(p: &Path) -> Img8 {
    let d = std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while d[i].is_ascii_whitespace() {
            i += 1;
        }
        if d[i] == b'#' {
            while d[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let s = i;
        while !d[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&d[s..i]).into_owned());
    }
    i += 1;
    let channels = match fields[0].as_str() {
        "P6" => 3,
        "P5" => 1,
        m => panic!("{}: {m} is not P5/P6", p.display()),
    };
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    Img8 {
        width: w,
        height: h,
        channels,
        data: d[i..i + w * h * channels].to_vec(),
    }
}

fn write_pgm(p: &Path, img: &Img8) {
    let g = img.grey();
    let mut f = std::fs::File::create(p).unwrap();
    write!(f, "P5\n{} {}\n255\n", g.width, g.height).unwrap();
    f.write_all(&g.data).unwrap();
}

fn rgba(img: &Img8) -> Vec<u8> {
    let mut v = Vec::with_capacity(img.width * img.height * 4);
    for p in img.data.chunks_exact(img.channels) {
        let (r, g, b) = if img.channels >= 3 {
            (p[0], p[1], p[2])
        } else {
            (p[0], p[0], p[0])
        };
        v.extend_from_slice(&[r, g, b, 255]);
    }
    v
}

fn runner(cpu: bool) -> (Runner, Option<specular_gfx::Gpu>) {
    if cpu {
        return (Runner::Cpu(8), None);
    }
    let g = specular_gfx::Gpu::new().expect("no GPU");
    eprintln!("gpu: {}", g.info.name);
    let c = Context::new(&g.device, &g.queue);
    (Runner::Gpu(c), Some(g))
}

enum Rec {
    Paddle(Recognizer),
    Tess(Tesseract),
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("tess") => {
            let t = Tesseract::open(Path::new(&args[1])).unwrap();
            let mut out = std::fs::File::create(&args[2]).unwrap();
            let lines: Vec<(String, Img8)> = args[3..]
                .iter()
                .map(|p| {
                    let name = Path::new(p)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    (name, read_pnm(Path::new(p)))
                })
                .collect();
            let imgs: Vec<Img8> = lines.iter().map(|(_, i)| i.clone()).collect();
            let t0 = Instant::now();
            let res = t.read(&imgs, 8);
            eprintln!("{} lines in {:?}", imgs.len(), t0.elapsed());
            for ((name, _), d) in lines.iter().zip(res) {
                writeln!(out, "{name}\t{}\t{:.4}", d.text, d.confidence).unwrap();
            }
        }
        Some("pipeline") => {
            let det_dir = PathBuf::from(&args[1]);
            let rec_arg = &args[2];
            let out_path = &args[3];
            let mut cpu = false;
            let mut side = 1920;
            let mut crops: Option<PathBuf> = None;
            let mut images = Vec::new();
            let mut i = 4;
            while i < args.len() {
                match args[i].as_str() {
                    "--cpu" => cpu = true,
                    "--side" => {
                        i += 1;
                        side = args[i].parse().unwrap();
                    }
                    "--crops" => {
                        i += 1;
                        crops = Some(PathBuf::from(&args[i]));
                    }
                    p => images.push(PathBuf::from(p)),
                }
                i += 1;
            }
            let (run, _gpu) = runner(cpu);
            let t0 = Instant::now();
            let mut det = Detector::new(
                PaddleNet::open(
                    &det_dir.join("config.json"),
                    &det_dir.join("model.safetensors"),
                    true,
                )
                .unwrap(),
            )
            .unwrap();
            det.size.max_side = side;
            let mut rec = if let Some(p) = rec_arg.strip_prefix("tess:") {
                Rec::Tess(Tesseract::open(Path::new(p)).unwrap())
            } else {
                let d = PathBuf::from(rec_arg);
                Rec::Paddle(
                    Recognizer::open(
                        &d.join("config.json"),
                        &d.join("model.safetensors"),
                        &d.join("preprocessor_config.json"),
                        true,
                    )
                    .unwrap(),
                )
            };
            eprintln!("loaded in {:?}", t0.elapsed());
            let mut out = std::fs::File::create(out_path).unwrap();
            let (mut tdet, mut trec) = (0f64, 0f64);
            for p in &images {
                let img = read_pnm(p);
                let px = rgba(&img);
                let view = Rgba {
                    width: img.width,
                    height: img.height,
                    data: &px,
                };
                let t1 = Instant::now();
                let boxes = det.detect(&run, &view).unwrap();
                let t2 = Instant::now();
                let lines: Vec<Img8> = boxes
                    .iter()
                    .map(|b| {
                        crop_quad(&view, b.pts).unwrap_or(Img8 {
                            width: 1,
                            height: 1,
                            channels: 3,
                            data: vec![255; 3],
                        })
                    })
                    .collect();
                let texts = match &mut rec {
                    Rec::Paddle(r) => r.read(&run, &lines).unwrap(),
                    Rec::Tess(t) => t.read(&lines, 8),
                };
                let t3 = Instant::now();
                tdet += (t2 - t1).as_secs_f64();
                trec += (t3 - t2).as_secs_f64();
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                let label = std::env::var("OCR_LABEL_PREFIX").unwrap_or_default();
                for (n, (b, d)) in boxes.iter().zip(&texts).enumerate() {
                    let q: Vec<String> = b
                        .pts
                        .iter()
                        .flat_map(|p| [format!("{:.1}", p[0]), format!("{:.1}", p[1])])
                        .collect();
                    writeln!(
                        out,
                        "{label}{}\t{}\t{:.4}\t{}",
                        name.replace(".ppm", ".png"),
                        q.join("\t"),
                        d.confidence,
                        d.text
                    )
                    .unwrap();
                    if let Some(dir) = &crops {
                        write_pgm(
                            &dir.join(format!("{}-{n:03}.pgm", name.trim_end_matches(".ppm"))),
                            &lines[n],
                        );
                    }
                }
                eprintln!(
                    "{name}: {}×{}, {} boxes, detect {:.0} ms, read {:.0} ms",
                    img.width,
                    img.height,
                    boxes.len(),
                    (t2 - t1).as_secs_f64() * 1e3,
                    (t3 - t2).as_secs_f64() * 1e3
                );
            }
            eprintln!(
                "total detect {:.2} s, read {:.2} s over {} images",
                tdet,
                trec,
                images.len()
            );
        }
        _ => {
            eprintln!(
                "usage: ocr pipeline DET_DIR REC OUT.tsv [--cpu] [--side N] [--crops DIR] IMAGE.ppm... | ocr tess TRAINEDDATA OUT.tsv LINE.pgm..."
            );
            std::process::exit(2);
        }
    }
}
