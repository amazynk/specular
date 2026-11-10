//! Lists and reads every entry of a generated 500-page CBZ (and CB7/CBT variants).
//!
//! `cargo run --release -p specular-archive --example archive_bench [pages] [page_kib]`
//! Files are written under the workspace `target/archive-bench/`.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use specular_archive::{Archive, Options};

fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

/// Mostly incompressible (like JPEG data) with a compressible tail, so Deflate does some work.
fn page(i: u64, len: usize) -> Vec<u8> {
    let mut v = noise(i + 1, len * 3 / 4);
    v.extend(format!("page {i} ").bytes().cycle().take(len - v.len()));
    v
}

fn name(i: usize) -> String {
    // Unpadded numbers so the natural sort has real work to do.
    format!("Vol 1/Chapter {}/page {}.jpg", i / 25 + 1, i)
}

fn make_zip(pages: usize, kib: usize, deflate: bool) -> Vec<u8> {
    use zip::write::SimpleFileOptions;
    let method = if deflate {
        zip::CompressionMethod::Deflated
    } else {
        zip::CompressionMethod::Stored
    };
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    // Written in reverse order: listing must sort.
    for i in (0..pages).rev() {
        w.start_file(
            name(i),
            SimpleFileOptions::default().compression_method(method),
        )
        .unwrap();
        w.write_all(&page(i as u64, kib << 10)).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn make_tar(pages: usize, kib: usize) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    for i in (0..pages).rev() {
        let data = page(i as u64, kib << 10);
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, name(i), &data[..]).unwrap();
    }
    b.into_inner().unwrap()
}

fn make_7z(pages: usize, kib: usize, solid: bool) -> Vec<u8> {
    use sevenz_rust2::{ArchiveEntry, ArchiveWriter, SourceReader};
    let mut w = ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
    if solid {
        let entries = (0..pages)
            .map(|i| ArchiveEntry::new_file(&name(i)))
            .collect();
        let readers = (0..pages)
            .map(|i| SourceReader::new(Cursor::new(page(i as u64, kib << 10))))
            .collect();
        w.push_archive_entries(entries, readers).unwrap();
    } else {
        for i in 0..pages {
            let data = page(i as u64, kib << 10);
            w.push_archive_entry(ArchiveEntry::new_file(&name(i)), Some(Cursor::new(data)))
                .unwrap();
        }
    }
    w.finish().unwrap().into_inner()
}

fn ms(d: Duration) -> String {
    format!("{:8.2} ms", d.as_secs_f64() * 1e3)
}

fn bench(label: &str, path: &Path) {
    let size = std::fs::metadata(path).unwrap().len();
    let t = Instant::now();
    let a = Archive::open_with(path, Options::default()).unwrap();
    let open = t.elapsed();
    let t = Instant::now();
    let pages = a.pages(|e| e.extension() == "jpg");
    let filter = t.elapsed();
    assert_eq!(a.entries()[pages[0]].name, name(0));
    assert_eq!(a.entries()[pages[1]].name, name(1));

    let t = Instant::now();
    let mut buf = Vec::new();
    let mut total = 0u64;
    for &i in &pages {
        a.read_into(i, &mut buf).unwrap();
        total += buf.len() as u64;
    }
    let seq = t.elapsed();

    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let t = Instant::now();
    std::thread::scope(|s| {
        for k in 0..threads {
            let (a, pages) = (&a, &pages);
            s.spawn(move || {
                let mut buf = Vec::new();
                for &i in pages.iter().skip(k).step_by(threads) {
                    a.read_into(i, &mut buf).unwrap();
                }
            });
        }
    });
    let par = t.elapsed();

    let t = Instant::now();
    let mut zero_copy = 0;
    for &i in &pages {
        if let Some(s) = a.stored_slice(i) {
            zero_copy += 1;
            std::hint::black_box(s[s.len() / 2]);
        }
    }
    let slices = t.elapsed();

    println!(
        "{label:<18} {:>6.1} MiB  open+list+sort {}  filter {}  read seq {} ({:6.0} MiB/s)  read x{threads} {}  stored_slice {} ({zero_copy} zero-copy)",
        size as f64 / 1048576.0,
        ms(open),
        ms(filter),
        ms(seq),
        total as f64 / 1048576.0 / seq.as_secs_f64(),
        ms(par),
        ms(slices),
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let pages: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(500);
    let kib: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(200);
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/archive-bench");
    std::fs::create_dir_all(&dir).unwrap();
    println!("{pages} pages of {kib} KiB, files in {}", dir.display());

    type Make = Box<dyn Fn() -> Vec<u8>>;
    let variants: [(&str, &str, Make); 5] = [
        (
            "CBZ stored",
            "stored.cbz",
            Box::new(move || make_zip(pages, kib, false)),
        ),
        (
            "CBZ deflate",
            "deflate.cbz",
            Box::new(move || make_zip(pages, kib, true)),
        ),
        ("CBT", "book.cbt", Box::new(move || make_tar(pages, kib))),
        (
            "CB7 non-solid",
            "nonsolid.cb7",
            Box::new(move || make_7z(pages, kib, false)),
        ),
        (
            "CB7 solid",
            "solid.cb7",
            Box::new(move || make_7z(pages, kib, true)),
        ),
    ];
    for (label, file, make) in &variants {
        let path = dir.join(file);
        if !path.exists() {
            std::fs::write(&path, make()).unwrap();
        }
        bench(label, &path);
    }
}
