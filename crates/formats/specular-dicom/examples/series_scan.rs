//! Times series scans: for every folder under the given roots that holds a DICOM file, scans
//! the series around its first one (as opening it in the viewer does) and prints per-folder
//! and total header counts and times.
//!
//! `series_scan [-v] [--any] ROOT...`; `--any`: scan around the first file of every folder,
//! DICOM or not (the cost of a folder of other images that holds one DICOM file).

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use specular_dicom::series::{self, ScanLimits};

fn folders(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut has_file = false;
    let mut subs = Vec::new();
    for e in rd.flatten() {
        match e.file_type() {
            Ok(t) if t.is_dir() => subs.push(e.path()),
            Ok(t) if t.is_file() => has_file = true,
            _ => {}
        }
    }
    if has_file {
        out.push(dir.to_path_buf());
    }
    subs.sort();
    for s in subs {
        folders(&s, out);
    }
}

fn first_dicom(dir: &Path, any: bool) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| !series::is_dicomdir_name(p.file_name().unwrap_or_default()))
        .collect();
    files.sort();
    files.into_iter().find(|p| {
        if any {
            return true;
        }
        let mut head = vec![0u8; 512];
        std::fs::File::open(p)
            .and_then(|mut f| {
                use std::io::Read;
                let n = f.read(&mut head)?;
                head.truncate(n);
                Ok(())
            })
            .is_ok()
            && specular_dicom::sniff(&head)
    })
}

fn main() {
    let mut verbose = false;
    let mut any = false;
    let mut roots = Vec::new();
    for a in std::env::args().skip(1) {
        if a == "-v" {
            verbose = true;
        } else if a == "--any" {
            any = true;
        } else {
            roots.push(PathBuf::from(a));
        }
    }
    let mut dirs = Vec::new();
    for r in &roots {
        folders(r, &mut dirs);
    }
    let (mut n_dirs, mut headers, mut skipped, mut series_n, mut slowest) = (0, 0, 0, 0, 0f64);
    let t_all = Instant::now();
    let mut total_ms = 0.0;
    for d in dirs {
        let Some(f) = first_dicom(&d, any) else {
            continue;
        };
        let t = Instant::now();
        let s = series::scan(&f, ScanLimits::default(), &AtomicBool::new(false));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        total_ms += ms;
        slowest = slowest.max(ms);
        n_dirs += 1;
        match s {
            Ok(s) => {
                headers += s.headers;
                skipped += s.skipped;
                series_n += s.series.len();
                if verbose {
                    println!(
                        "{:8.2} ms  {:5} headers {:4} skipped {:3} series{}  {}",
                        ms,
                        s.headers,
                        s.skipped,
                        s.series.len(),
                        if s.dicomdir.is_some() {
                            " (DICOMDIR)"
                        } else {
                            ""
                        },
                        d.display()
                    );
                }
            }
            Err(e) => println!("{ms:8.2} ms  error {e}  {}", d.display()),
        }
    }
    println!(
        "{n_dirs} folders, {headers} headers ({skipped} other files), {series_n} series: \
         {total_ms:.1} ms scanning ({:.1} µs per header, slowest folder {slowest:.1} ms), \
         {:.1} ms wall",
        if headers > 0 {
            total_ms * 1e3 / headers as f64
        } else {
            0.0
        },
        t_all.elapsed().as_secs_f64() * 1e3
    );
}
