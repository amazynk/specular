//! Nikon High Efficiency raw through the registry: the Info coding names the codec, the
//! embedded preview (gallery thumbnails) and the developed sensor both show. Skipped without
//! the corpus of tools/corpus/fetch-nikon-he.sh.

use std::path::Path;
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Format, Registry};

#[test]
fn high_efficiency_develops_through_registry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/nikon-he");
    for (rel, coding) in [
        (
            "Z5_2/High_Efficiency_DX.NEF",
            "14-bit Nikon High Efficiency (HE, TicoRAW)",
        ),
        (
            "Z6_3/Nikon_Z6__3_High_Efficiency_Star_1_1.NEF",
            "14-bit Nikon High Efficiency* (HE*, TicoRAW)",
        ),
    ] {
        let Ok(bytes) = std::fs::read(root.join(rel)) else {
            eprintln!("skipped {rel}: no tools/corpus/nikon-he");
            continue;
        };
        let s = Registry::default()
            .open(Arc::new(Source::from_vec("x.nef", bytes)))
            .unwrap();
        let i = s.info();
        assert_eq!(i.format, Format::Raw);
        assert!(i.coding.contains(coding), "{rel}: {}", i.coding);
        assert!(!i.coding.contains("preview only"), "{rel}: {}", i.coding);
        let p = s.decode_preview().expect("preview").unwrap();
        assert!(p.width >= 160, "{rel}");
        let r = s
            .decode(&DecodeRequest::full(4), &CancelToken::new())
            .unwrap();
        assert!(r.width > 0 && r.height > 0, "{rel}");
        // Not flat: the scene shows.
        let d = &r.planes[0].data;
        let (lo, hi) = d
            .iter()
            .fold((u8::MAX, 0u8), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(hi - lo > 100, "{rel}: {lo}..{hi}");
    }
}
