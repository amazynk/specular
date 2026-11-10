//! `cargo test --test dump -- --ignored --nocapture` prints every fixture's report.
use specular_metadata::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
#[ignore]
fn dump_all() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let lang = std::env::var("DUMP_LANG").unwrap_or_else(|_| "en".into());
    let l = Localizer::new(lang.parse().unwrap());
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "jpg").then_some(p)
        })
        .collect();
    names.sort();
    for p in names {
        let data = std::fs::read(&p).unwrap();
        let seg = jpeg::scan(&data).unwrap();
        let file = FileFacts {
            path: Some(p.clone()),
            file_size: Some(data.len() as u64),
            modified: Some(UNIX_EPOCH + Duration::from_secs(1_716_000_000)),
            ..Default::default()
        };
        let input = seg.input(file);
        let mut r = read_at(&input, &l, SystemTime::now());
        attach_place(&mut r, &l);
        println!("===== {}", p.file_name().unwrap().to_string_lossy());
        for g in &r.summary {
            println!("[{}]", strip_isolates(&g.title));
            for i in &g.items {
                println!(
                    "  {}: {}{}",
                    strip_isolates(&i.label),
                    strip_isolates(&i.value),
                    i.detail
                        .as_ref()
                        .map(|d| format!("   ({})", strip_isolates(d)))
                        .unwrap_or_default()
                );
            }
        }
        for w in &r.warnings {
            println!("  ! {}", strip_isolates(w));
        }
        println!(
            "  one-line: {:?}",
            one_line_summary(&r).map(|s| strip_isolates(&s))
        );
        if std::env::var("DUMP_FIELDS").is_ok() {
            for g in &r.all_fields {
                println!("  <{}>", g.title);
                for f in &g.fields {
                    println!("    {} = {}", f.tag, f.display);
                }
            }
        }
    }
}

/// `DUMP_FILES=a.raf:b.mp4 cargo test --test dump dump_files -- --ignored --nocapture` prints
/// each file's report from the Exif, XMP and ICC specular-decode finds by signature (the info
/// panel's path for formats other than JPEG).
#[test]
#[ignore]
fn dump_files() {
    let l = Localizer::new("en".parse().unwrap());
    let mut files = std::env::var("DUMP_FILES").unwrap_or_default();
    // `@list.txt`: the paths, separated by ':' or new lines, in a file.
    if let Some(list) = files.strip_prefix('@') {
        files = std::fs::read_to_string(list).unwrap().replace('\n', ":");
    }
    for p in files.split(':').filter(|s| !s.is_empty()) {
        let data = std::fs::read(p).unwrap();
        let exif = specular_decode::exif_tiff(&data);
        let emb = specular_decode::embedded_metadata(&data);
        println!(
            "===== {p}: exif {:?} xmp {:?} icc {:?}",
            exif.as_ref().map(|e| e.len()),
            emb.xmp.as_ref().map(|e| e.len()),
            emb.icc.as_ref().map(|e| e.len())
        );
        let input = MetadataInput {
            exif_tiff: exif.as_deref(),
            xmp: emb.xmp.as_deref(),
            icc: emb.icc.as_deref(),
            ..Default::default()
        };
        let r = read_at(&input, &l, SystemTime::now());
        for g in &r.summary {
            println!("[{}]", strip_isolates(&g.title));
            for i in &g.items {
                println!(
                    "  {}: {}",
                    strip_isolates(&i.label),
                    strip_isolates(&i.value)
                );
            }
        }
        for w in &r.warnings {
            println!("  ! {}", strip_isolates(w));
        }
        if std::env::var("DUMP_FIELDS").is_ok() {
            for g in &r.all_fields {
                println!("  <{}> {}", g.title, g.fields.len());
            }
        }
    }
}
