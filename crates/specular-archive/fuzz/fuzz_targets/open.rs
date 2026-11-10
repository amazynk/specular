#![no_main]
//! Container parsing: detect, list, borrow and read entries of ZIP, TAR and 7z archives with
//! small limits. RAR is only detected here: listing it needs the sandboxed decode host.

use libfuzzer_sys::fuzz_target;
use specular_archive::{detect, Archive, Format, Limits, Options};

fuzz_target!(|data: &[u8]| {
    let Some(format) = detect(data) else { return };
    if format == Format::Rar {
        return;
    }
    let options = Options {
        limits: Limits {
            max_entries: 512,
            max_entry_size: 1 << 20,
            max_total_size: 1 << 30,
            max_ratio: 200,
            ratio_threshold: 1 << 16,
            max_decoder_memory: 1 << 24,
            max_comic_info: 1 << 16,
        },
        verify_crc: true,
        solid_cache_bytes: 1 << 20,
    };
    let Ok(archive) = Archive::from_bytes(data.to_vec(), options) else {
        return;
    };
    assert_eq!(archive.format(), format);
    let mut names = 0usize;
    for e in archive.entries() {
        names += e.file_name().len() + e.extension().len();
        let _ = (e.is_archive(), e.is_junk());
    }
    std::hint::black_box(names);
    if let Some(first) = archive.entries().first() {
        assert!(archive.find(&first.name).is_some());
        let _ = archive.find(&first.name.to_ascii_uppercase());
    }
    let pages = archive.pages(|e| e.extension() != "xml");
    let _ = archive.nested_archives();
    let mut buf = Vec::new();
    for &i in pages.iter().take(8) {
        let size = archive.entries()[i].size;
        if let Some(s) = archive.stored_slice(i) {
            assert_eq!(s.len() as u64, size);
        }
        if archive.read_into(i, &mut buf).is_ok() {
            assert_eq!(buf.len() as u64, size);
        }
    }
    let _ = archive.comic_info();
});
