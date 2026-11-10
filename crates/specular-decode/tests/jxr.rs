//! JPEG XR through the registry: probing ahead of TIFF, the JXR extensions, metadata from the
//! container (Exif IFD, XMP, ICC) and, with SPECULAR_LARGE_CORPUS, the corpus files
//! (tools/corpus/fetch-jxr.sh) including other encoders' metadata and orientations.

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{
    DecodeRequest, Format, Registry, embedded_metadata, exif_date_taken, exif_tiff,
};

fn vector(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../formats/specular-jxr/tests/vectors/{name}.jxr",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn registry_opens_jxr() {
    let reg = Registry::new();
    let c = CancelToken::new();
    for ext in ["jxr", "wdp", "hdp", "tif"] {
        // Content wins over a misleading .tif name.
        let src = Arc::new(Source::from_vec(format!("a.{ext}"), vector("rgb24-q")));
        let s = reg.open(src).unwrap();
        assert_eq!(s.info().format, Format::JpegXr, "{ext}");
        assert_eq!(s.info().format.name(), "JPEG XR");
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((r.width, r.height), (s.info().width, s.info().height));
    }
    assert!(reg.supports_extension(std::path::Path::new("x.WDP")));
}

#[test]
fn corpus_metadata() {
    let Some(dir) = std::env::var_os("SPECULAR_LARGE_CORPUS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("jxr/kde");
    if !dir.join("metadata.jxr").exists() {
        return;
    }
    let data = std::fs::read(dir.join("metadata.jxr")).unwrap();
    let exif = exif_tiff(&data).expect("Exif");
    assert_eq!(
        exif_date_taken(&exif).as_deref().map(|d| &d[..10]),
        Some("2025:01:14")
    );
    let facts = specular_decode::exif_facts(&exif).unwrap();
    assert!(format!("{:?}", facts.0).contains("KFramework"));
    let meta = embedded_metadata(&data);
    assert!(
        meta.xmp
            .is_some_and(|x| x.windows(9).any(|w| w == b"x:xmpmeta"))
    );
    let reg = Registry::new();
    let c = CancelToken::new();
    for o in 1..=8u8 {
        let src = Arc::new(Source::from_vec(
            format!("o{o}.jxr"),
            std::fs::read(dir.join(format!("orientation{o}.jxr"))).unwrap(),
        ));
        let s = reg.open(src).unwrap();
        assert_eq!(s.info().display_size(), (480, 160), "orientation{o}");
        s.decode(&DecodeRequest::full(1), &c).unwrap();
    }
    for f in std::fs::read_dir(&dir).unwrap() {
        let p = f.unwrap().path();
        let s = reg
            .open_path(&p)
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        s.decode(&DecodeRequest::full(1), &c)
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    }
}
