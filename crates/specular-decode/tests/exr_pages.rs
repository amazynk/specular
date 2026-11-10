//! Multi-part OpenEXR: each part (deep parts flattened) is a page, in file order, labelled with
//! its name. Uses the OpenEXR sample images of the conformance corpus (skipped when absent).

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeError, DecodeRequest, PageLabel, Registry, Session};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance")
        })
        .join("openexr-images")
}

fn open(path: &Path) -> (Vec<u8>, Arc<dyn Session>) {
    let data = std::fs::read(path).unwrap();
    let s = Registry::new()
        .open(Arc::new(Source::from_vec(
            path.file_name().unwrap().to_str().unwrap(),
            data.clone(),
        )))
        .unwrap();
    (data, s)
}

#[test]
fn parts_are_pages() {
    let path = corpus().join("Beachball/multipart.0001.exr");
    if !path.exists() {
        eprintln!("EXR corpus absent; skipped");
        return;
    }
    let (data, s) = open(&path);
    let info = specular_exr::read_info(&data).unwrap();
    let parts: Vec<_> = info.parts.iter().collect();
    assert!(parts.len() > 1);
    assert_eq!(s.page_count(), parts.len() as u32);
    assert_eq!(s.page_index(), 0);
    let mut sessions = vec![s.clone()];
    for page in 1..s.page_count() {
        sessions.push(s.open_page(page).unwrap());
    }
    // Back to the first from the last.
    let back = sessions.last().unwrap().open_page(0).unwrap();
    assert_eq!(back.info().width, s.info().width);
    assert_eq!(back.page_label(), s.page_label());
    for (page, (p, part)) in sessions.iter().zip(&parts).enumerate() {
        assert_eq!(p.page_index(), page as u32);
        assert_eq!(p.page_count(), parts.len() as u32);
        let Some(PageLabel::DocumentLayer {
            name,
            index,
            hidden,
        }) = p.page_label()
        else {
            panic!("page {page}: {:?}", p.page_label());
        };
        assert_eq!(&*name, part.name.as_deref().unwrap_or(""));
        assert_eq!(index, page as u32 + 1);
        assert!(!hidden);
        assert_eq!(u64::from(p.info().width), part.width());
        assert_eq!(u64::from(p.info().height), part.height());
        let req = DecodeRequest::full(8);
        assert!(p.decode_memory(&req) > 0);
        let t = std::time::Instant::now();
        let r = p.decode(&req, &CancelToken::new()).unwrap();
        eprintln!(
            "page {page} {name:?}: {}x{} in {:?}",
            r.width,
            r.height,
            t.elapsed()
        );
        assert!(!r.truncated, "page {page}");
        assert_eq!(r.width, p.info().width.div_ceil(8));
    }
    assert!(s.open_page(s.page_count()).is_err());
    // A cancelled decode of a later page stops.
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        sessions[1].decode(&DecodeRequest::full(1), &cancel),
        Err(DecodeError::Cancelled)
    ));
}

#[test]
fn single_part_has_one_unlabelled_page() {
    let path = corpus().join("ScanLines/Desk.exr");
    if !path.exists() {
        eprintln!("EXR corpus absent; skipped");
        return;
    }
    let (_, s) = open(&path);
    assert_eq!(s.page_count(), 1);
    assert_eq!(s.page_label(), None);
    assert!(s.open_page(1).is_err());
    assert_eq!(s.open_page(0).unwrap().info().width, s.info().width);
}

#[test]
fn deep_parts_open_flattened() {
    let stereo = corpus().join("v2/Stereo/Balls.exr");
    let single = corpus().join("v2/LowResLeftView/Balls.exr");
    if !stereo.exists() || !single.exists() {
        eprintln!("EXR corpus absent; skipped");
        return;
    }
    let (_, s) = open(&single);
    assert_eq!(s.page_count(), 1);
    assert!(
        s.info().coding.contains("deep, flattened"),
        "{}",
        s.info().coding
    );
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(!r.truncated);
    assert_eq!(r.width, s.info().width);

    let (_, s) = open(&stereo);
    assert_eq!(s.page_count(), 2);
    for page in 0..2 {
        let p = s.open_page(page).unwrap();
        assert!(p.info().coding.contains("deep, flattened"));
        let r = p
            .decode(&DecodeRequest::full(2), &CancelToken::new())
            .unwrap();
        assert!(!r.truncated, "page {page}");
    }
}
