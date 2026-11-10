//! Design and office-drawing formats through the registry: Clip Studio Paint, Sketch, Figma,
//! Visio, OpenDocument, WordPerfect Graphics. The corpus parts (tools/corpus/fetch-design2*.sh)
//! skip when it is absent; rendering itself is tested against each format's oracle in its
//! own crate.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Format, Registry, Session};

fn corpus(sub: &str) -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DESIGN2_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/design2")
        })
        .join(sub);
    p.is_dir().then_some(p)
}

fn files(dir: &Path, ext: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| ext.iter().any(|e| x.eq_ignore_ascii_case(e)))
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn decode_small(s: &dyn Session) -> (u32, u32) {
    let r = s
        .decode(&DecodeRequest::full(8), &CancelToken::new())
        .unwrap_or_else(|e| panic!("{}: {e}", s.source().path().display()));
    (r.width, r.height)
}

/// Opens every corpus file of `ext`, checks the format, decodes every page at scale 8 and
/// returns (opened, refused).
fn sweep(sub: &str, ext: &[&str], format: Format) -> Option<(usize, usize)> {
    let dir = corpus(sub)?;
    let reg = Registry::new();
    let (mut ok, mut refused) = (0, 0);
    for f in files(&dir, ext) {
        let s = match reg.open_path(&f) {
            Ok(s) => s,
            Err(e) => {
                refused += 1;
                eprintln!("{}: refused: {e}", f.display());
                continue;
            }
        };
        assert_eq!(s.info().format, format, "{}", f.display());
        for p in 0..s.page_count().min(8) {
            let page = if p == 0 {
                s.clone()
            } else {
                s.open_page(p).unwrap()
            };
            let (w, h) = decode_small(page.as_ref());
            assert!(w > 0 && h > 0);
        }
        eprintln!("{}: {}", f.display(), s.info().coding);
        ok += 1;
    }
    Some((ok, refused))
}

#[test]
fn corpus_clip_studio() {
    let Some((ok, refused)) = sweep("clip", &["clip"], Format::Clip) else {
        eprintln!("Clip Studio corpus absent: skipped");
        return;
    };
    assert!(ok > 0 && refused == 0);
}

#[test]
fn corpus_visio() {
    let Some((ok, refused)) = sweep(
        "visio",
        &["vsdx", "vstx", "vssx", "vsdm", "vsd", "vss"],
        Format::Visio,
    ) else {
        eprintln!("Visio corpus absent: skipped");
        return;
    };
    // Two binary files of the corpus have no preview (one is not a compound file at all).
    assert!(ok >= 20 && refused <= 2, "{ok} opened, {refused} refused");
}

#[test]
fn corpus_wordperfect_graphics() {
    let Some((ok, refused)) = sweep("wpg", &["wpg"], Format::Wpg) else {
        eprintln!("WPG corpus absent: skipped");
        return;
    };
    // BADGE.wpg is an OLE file, kept as a refusal case.
    assert!(ok >= 18 && refused <= 1, "{ok} opened, {refused} refused");
}

#[test]
fn corpus_sketch() {
    let Some((ok, refused)) = sweep("sketch", &["sketch"], Format::Sketch) else {
        eprintln!("Sketch corpus absent: skipped");
        return;
    };
    // Refusal cases: a pre-43 database, and a reference file with nothing to draw.
    assert!(ok >= 15 && refused <= 2, "{ok} opened, {refused} refused");
}

#[test]
fn corpus_figma() {
    let Some((ok, refused)) = sweep("figma", &["fig"], Format::Figma) else {
        eprintln!("Figma corpus absent: skipped");
        return;
    };
    // The bare fig-kiwi file has no thumbnail.
    assert!(ok >= 2 && refused == 1, "{ok} opened, {refused} refused");
}

#[test]
fn corpus_opendocument() {
    let Some((ok, refused)) = sweep(
        "odf",
        &["odg", "otg", "fodg", "odp", "otp", "fodp", "ods", "odt"],
        Format::OpenDocument,
    ) else {
        eprintln!("OpenDocument corpus absent: skipped");
        return;
    };
    assert!(ok >= 40 && refused == 0, "{ok} opened, {refused} refused");
}

#[test]
fn clip_refusal_is_plain() {
    let mut f = b"CSFCHUNK".to_vec();
    f.extend_from_slice(&[0; 8]);
    f.extend_from_slice(&24u64.to_be_bytes());
    f.extend_from_slice(b"CHNKFoot");
    f.extend_from_slice(&[0; 8]);
    let e = Registry::new()
        .open(Arc::new(Source::from_vec("a.clip", f)))
        .err()
        .unwrap();
    assert!(e.to_string().contains("no document database"), "{e}");
}
