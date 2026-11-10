//! Isolation through the registry: a decoder or sniffer that panics on one file costs that
//! file. Debug builds trip the test hooks of `specular_core::isolate` on marked names.

use std::path::PathBuf;
use std::sync::Arc;

use specular_core::isolate::{self, TEST_PANIC_MARK, TEST_SNIFF_MARK};
use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Registry};

fn pgm() -> Vec<u8> {
    let mut f = b"P5 2 2 255\n".to_vec();
    f.extend_from_slice(&[0, 64, 128, 255]);
    f
}

/// An OpenDocument text whose thumbnail is `thumb` (stored, uncompressed).
fn odt_with_thumbnail(thumb: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut out = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut out);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    z.start_file("mimetype", stored).unwrap();
    z.write_all(b"application/vnd.oasis.opendocument.text")
        .unwrap();
    z.start_file("Thumbnails/thumbnail.png", stored).unwrap();
    z.write_all(thumb).unwrap();
    z.finish().unwrap();
    out.into_inner()
}

/// A document whose thumbnail is a document whose thumbnail is... (2000 deep, a few hundred
/// bytes a level) is refused at `MAX_NESTING` instead of overflowing the stack; a few levels
/// still open.
#[test]
fn files_nested_in_files_stop() {
    let reg = Registry::default();
    let mut f = pgm();
    for _ in 0..3 {
        f = odt_with_thumbnail(&f);
    }
    let s = reg
        .open(Arc::new(Source::from_vec("a.odt", f)))
        .expect("three levels open");
    assert_eq!((s.info().width, s.info().height), (2, 2));

    let mut f = pgm();
    for _ in 0..2000 {
        f = odt_with_thumbnail(&f);
    }
    let r = specular_thread::on_stack(1 << 20, || {
        reg.open(Arc::new(Source::from_vec("a.odt", f)))
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
    let e = r.expect_err("refused");
    assert!(e.contains("nested too deeply"), "{e}");
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/decode-isolation"
    ))
    .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A panicking decode is an error at the job's boundary, and the registry opens the next
/// file as if nothing happened.
#[test]
fn a_panicking_decoder_is_an_error_and_the_next_file_opens() {
    let reg = Registry::new();
    let bad = Arc::new(Source::from_vec(format!("a-{TEST_PANIC_MARK}.pgm"), pgm()));
    let r = isolate::run("open the marked file", || reg.open(bad).map(|_| ()));
    let p = r.expect_err("the hook panics on the marked name");
    assert!(p.message.contains("test panic for"), "{p}");
    assert!(p.brief().starts_with("internal error"), "{}", p.brief());

    let good = Arc::new(Source::from_vec("b.pgm", pgm()));
    let s = isolate::run("open the next file", || reg.open(good))
        .expect("no panic")
        .expect("opens");
    let px = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .expect("decodes");
    assert_eq!((px.width, px.height), (2, 2));
}

/// A sniffer that panics on one file leaves that file out of folder listings; the other
/// files are still recognised and nothing propagates to the caller.
#[test]
fn a_panicking_sniffer_leaves_the_file_out() {
    let dir = scratch("sniff");
    let bad = dir.join(format!("c-{TEST_SNIFF_MARK}.pgm"));
    let good = dir.join("d.pgm");
    std::fs::write(&bad, pgm()).unwrap();
    std::fs::write(&good, pgm()).unwrap();
    let reg = Registry::new();
    let before = isolate::caught();
    assert!(!reg.supports_file(&bad));
    assert!(isolate::caught() > before, "the panic was caught");
    assert!(reg.supports_file(&good));
    let _ = std::fs::remove_dir_all(&dir);
}
