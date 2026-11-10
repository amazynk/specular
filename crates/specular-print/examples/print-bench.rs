//! Wall time and peak memory of writing a PDF from real files (DESIGN.md §6.5's budgets).
//!
//! `cargo run --release -p specular-print --example print-bench -- sheet|a3 OUT_DIR`
//!
//! - `sheet`: a 48-image A4 contact sheet at 300 dpi (4 × 6, names and dates, header: two
//!   pages) from the first 48 RAW files of `$SPECULAR_LARGE_CORPUS/raw`;
//! - `a3`: one A3 page at 300 dpi filled by a 24 MP image
//!   (`$SPECULAR_LARGE_CORPUS/raw/Adobe DNG Converter/Nikon D780/DSC_0336.dng`, or the file
//!   given as the third argument).
//!
//! Run each case in its own process: the peak (VmHWM) is the process's.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use specular_decode::Registry;
use specular_gfx::{Gpu, Text};
use specular_print::files::FileSource;
use specular_print::layout::{Caption, Fit, Item, Kind, PageSetup, Paper};
use specular_print::{PageRenderer, layout, pdf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn vm(key: &str) -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .map(|l| l[key.len()..].trim().to_owned())
        })
        .unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let case = args.get(1).map(String::as_str).unwrap_or("sheet");
    let out_dir = PathBuf::from(args.get(2).expect("OUT_DIR"));
    let corpus =
        PathBuf::from(std::env::var_os("SPECULAR_LARGE_CORPUS").expect("SPECULAR_LARGE_CORPUS"));
    std::fs::create_dir_all(&out_dir).unwrap();
    let (paths, setup, kind) = match case {
        "sheet" => {
            let mut v = Vec::new();
            files(&corpus.join("raw"), &mut v);
            v.truncate(48);
            assert_eq!(v.len(), 48, "48 RAW files");
            let setup = PageSetup {
                paper: Paper::A4,
                dpi: 300,
                ..PageSetup::default()
            };
            let kind = Kind::Sheet {
                cols: 4,
                rows: 6,
                caption: Caption::NameDate,
                header: true,
            };
            (v, setup, kind)
        }
        _ => {
            let p = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| corpus.join("raw/Adobe DNG Converter/Nikon D780/DSC_0336.dng"));
            let setup = PageSetup {
                paper: Paper::A3,
                dpi: 300,
                ..PageSetup::default()
            };
            (
                vec![p],
                setup,
                Kind::Single {
                    fit: Fit::Fill,
                    caption: Caption::Off,
                },
            )
        }
    };
    let rss0 = vm("VmRSS:");
    let gpu = Gpu::new().expect("a GPU");
    let t0 = Instant::now();
    let source = FileSource::new(Arc::new(Registry::default()), paths);
    let probes = source.probe();
    let items: Vec<Item> = probes
        .iter()
        .map(|p| Item {
            size: p.size.unwrap_or((3, 2)),
            name: p.name.clone(),
            date: p.modified.map(|s| s.to_string()),
            scale: None,
        })
        .collect();
    let probed = t0.elapsed();
    if case != "sheet" {
        println!("image: {:?} px", probes[0].size);
    }
    let doc = layout(setup, kind, &items, "bench", &|n, of| format!("{n} / {of}"));
    let mut r = PageRenderer::new(gpu, Text::new());
    let path = out_dir.join(format!("{case}.pdf"));
    let f = std::fs::File::create(&path).unwrap();
    let t1 = Instant::now();
    let meta = pdf::Meta {
        title: "bench".into(),
        date: pdf::date(0),
        ..Default::default()
    };
    let (w, stats, missing) = r
        .write_pdf(
            &doc,
            &meta,
            std::io::BufWriter::with_capacity(1 << 20, f),
            &source,
            &mut |_| {},
            &|| false,
        )
        .expect("pdf");
    w.into_inner().ok().unwrap().sync_all().ok();
    let render = t1.elapsed();
    let (pw, ph) = setup.size_px();
    println!(
        "{case}: {} pages of {pw}x{ph} px, {} images ({missing} unreadable), {:.1} MB PDF",
        stats.pages,
        items.len(),
        stats.bytes as f64 / 1e6
    );
    println!(
        "probe {:.2} s, render + write {:.2} s, total {:.2} s",
        probed.as_secs_f64(),
        render.as_secs_f64(),
        t0.elapsed().as_secs_f64()
    );
    println!("RSS before GPU {rss0}, peak (VmHWM) {}", vm("VmHWM:"));
}
