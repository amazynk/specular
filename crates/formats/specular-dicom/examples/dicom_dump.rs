//! Decodes DICOM files for oracle comparisons and timing.
//!
//! `dicom_dump OUT_DIR FILE...` writes, per file `NAME`:
//! - `NAME.txt`: `ok` / `err <message>`, then `key value` lines (size, frames, kind, transfer
//!   syntax, default VOI source, timings in microseconds);
//! - `NAME.f<k>.raw`: frame k's stored grey values as little-endian i32, or RGB samples as
//!   little-endian u16 (frames 0 and the last);
//! - `NAME.f<k>.disp`: the default rendering as little-endian u16 (grey, or RGBA).
//!
//! `dicom_dump --time FILE...` only prints parse / first-frame / render timings.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use specular_dicom::{Dicom, Pixels, Rendered};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((first, files)) = args.split_first() else {
        eprintln!("usage: dicom_dump OUT_DIR|--time FILE...");
        std::process::exit(2);
    };
    if first == "--time" {
        for f in files {
            time(Path::new(f));
        }
        return;
    }
    let out = Path::new(first);
    std::fs::create_dir_all(out).expect("output directory");
    for f in files {
        let p = Path::new(f);
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let report = dump(p, out, &name);
        std::fs::write(out.join(format!("{name}.txt")), report).expect("write report");
    }
}

fn time(p: &Path) {
    let data = std::fs::read(p).expect("read");
    let mut best = [u128::MAX; 4];
    for _ in 0..7 {
        let t0 = Instant::now();
        let Ok(d) = Dicom::parse(&data) else {
            println!("{}: not parsed", p.display());
            return;
        };
        let t1 = Instant::now();
        let mf = match d.modality_frame(&data, 0, None) {
            Ok(m) => m,
            Err(e) => {
                println!("{}: {e}", p.display());
                return;
            }
        };
        let t2 = Instant::now();
        let img = mf.render_default().expect("render");
        let t3 = Instant::now();
        std::hint::black_box(&img);
        let last = d.frame_count().saturating_sub(1);
        let t4 = Instant::now();
        let _ = std::hint::black_box(d.decode_frame(&data, last, None));
        let t5 = Instant::now();
        for (b, v) in best.iter_mut().zip([t1 - t0, t2 - t1, t3 - t2, t5 - t4]) {
            *b = (*b).min(v.as_micros());
        }
    }
    let d = Dicom::parse(&data).unwrap();
    let p_ = d.pixel.as_ref().unwrap();
    println!(
        "{}: {}x{} x{} {}-bit {} | parse {:.2} ms, frame 0 {:.2} ms, render {:.2} ms, open+first pixels {:.2} ms, last frame {:.2} ms",
        p.file_name().unwrap().to_string_lossy(),
        p_.columns,
        p_.rows,
        p_.frames,
        p_.bits_stored,
        d.transfer_syntax.name,
        best[0] as f64 / 1e3,
        best[1] as f64 / 1e3,
        best[2] as f64 / 1e3,
        (best[0] + best[1] + best[2]) as f64 / 1e3,
        best[3] as f64 / 1e3,
    );
}

fn dump(p: &Path, out: &Path, name: &str) -> String {
    let data = match std::fs::read(p) {
        Ok(d) => d,
        Err(e) => return format!("err read: {e}\n"),
    };
    let t0 = Instant::now();
    let d = match Dicom::parse(&data) {
        Ok(d) => d,
        Err(e) => return format!("err {e}\n"),
    };
    let parse_us = t0.elapsed().as_micros();
    let mut r = String::new();
    let mut body = String::new();
    body += &format!("ts {}\n", d.transfer_syntax.uid);
    body += &format!("parse_us {parse_us}\n");
    if let Some(Ok(icon)) = d.icon(&data) {
        body += &format!("icon {}x{}\n", icon.width, icon.height);
    }
    let frames = d.frame_count();
    body += &format!("frames {frames}\n");
    if frames == 0 {
        return format!("err no pixel data\n{body}");
    }
    let mut picks = vec![0];
    if frames > 1 {
        picks.push(frames - 1);
    }
    for k in picks {
        let t = Instant::now();
        let mf = match d.modality_frame(&data, k, None) {
            Ok(m) => m,
            Err(e) => return format!("err {e}\n{body}"),
        };
        body += &format!("frame{k}_us {}\n", t.elapsed().as_micros());
        let (voi, src) = mf.default_voi();
        body += &format!("voi{k} {src:?} {voi:?}\n");
        let f = &mf.frame;
        body += &format!("size {} {}\n", f.width, f.height);
        body += &format!("truncated{k} {}\n", f.truncated);
        let mut raw = Vec::new();
        match &f.pixels {
            Pixels::Grey(g) => {
                body += "kind grey\n";
                let n = g.len();
                for i in 0..n {
                    raw.extend((g.get(i).unwrap_or(0) as i32).to_le_bytes());
                }
                if let Some(u) = mf.unit() {
                    body += &format!("unit {u}\n");
                }
                body += &format!("inverted {}\n", mf.inverted());
            }
            Pixels::Rgb8(v) => {
                body += "kind rgb\n";
                for &x in v {
                    raw.extend((x as u16).to_le_bytes());
                }
            }
            Pixels::Rgb16(v) => {
                body += &format!("kind rgb{}\n", f.bits);
                for &x in v {
                    raw.extend(x.to_le_bytes());
                }
            }
        }
        std::fs::write(out.join(format!("{name}.f{k}.raw")), raw).expect("write raw");
        let img = mf.render(&voi).expect("render");
        let mut disp = Vec::new();
        match &img.pixels {
            Rendered::Gray16(v) | Rendered::Rgba16(v) => {
                for &x in v {
                    disp.extend(x.to_le_bytes());
                }
            }
            Rendered::Rgba8(v) => {
                for &x in v {
                    disp.extend(((x as u16) * 257).to_le_bytes());
                }
            }
        }
        std::fs::File::create(out.join(format!("{name}.f{k}.disp")))
            .and_then(|mut f| f.write_all(&disp))
            .expect("write disp");
    }
    r += "ok\n";
    r += &body;
    r
}
