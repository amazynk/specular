//! Focus of a camera RAW whose regions develop from the sensor tiles behind them (lossless ARW):
//! measured on region decodes, the same as on the whole developed image.

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod rw;

use specular_core::{CancelToken, Raster, Source};
use specular_decode::{DecodeRequest, ImageInfo, Registry, Result, Session};
use specular_similar::focus;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// `inner` as a session without fast regions: focus decodes the whole image.
struct Whole(Arc<dyn Session>, ImageInfo);

impl Session for Whole {
    fn info(&self) -> &ImageInfo {
        &self.1
    }
    fn source(&self) -> &Arc<Source> {
        self.0.source()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.0.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.0.decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.0.decode_memory(req)
    }
}

#[test]
fn raw_focus_same_through_regions() {
    let (w, h) = (640, 448);
    let mut spec = rw::ArwSpec::new(w, h, 64);
    spec.wb = Some([2400, 1024, 1024, 1700]);
    spec.crop = Some([2, 2, 632, 440]);
    let file = rw::write_arw(&spec, &rw::test_pattern(w as usize, h as usize, 1, 14, 5));
    let open = || {
        Registry::default()
            .open(Arc::new(Source::from_vec("x.arw", file.clone())))
            .unwrap()
    };
    let s = open();
    assert!(s.info().fast_regions);
    let stop = AtomicBool::new(false);
    let fast = focus::measure_session_at(&*s, None, &stop).unwrap();
    let inner = open();
    let mut info = inner.info().clone();
    info.fast_regions = false;
    let whole = focus::measure_session_at(&Whole(inner, info), None, &stop).unwrap();
    assert_eq!(fast.scale, 1);
    assert!(fast.regions && !whole.regions && (fast.decoded, whole.decoded) == (1, 1));
    assert_eq!(
        fast,
        focus::Focus {
            regions: true,
            ..whole
        }
    );
    // A record from before versions (one whole decode, as RAWs took then): kept where the
    // decode scale is the same today, measured again where it is not.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/similar-test")
        .join(format!("raw-focus-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.arw");
    std::fs::write(&path, &file).unwrap();
    let legacy = focus::Focus {
        version: 0,
        decoded: 0,
        regions: false,
        ..whole
    };
    let reg = Registry::default();
    let kept = focus::keep_or_measure(&reg, &path, None, Some(legacy), 1, &stop).unwrap();
    assert_eq!(kept, fast);
    let at2 = focus::Focus {
        scale: 2,
        detail: 0.5,
        ..legacy
    };
    assert!(at2.needs_check(true) && !legacy.needs_check(true));
    let again = focus::keep_or_measure(&reg, &path, None, Some(at2), 8, &stop).unwrap();
    assert_eq!(again, fast);
    std::fs::remove_dir_all(&dir).ok();
    // Reduced measures decode regions of the coarser levels (4 and 8 of 632 px: 158, 79).
    for (side, scale) in [(158, 4), (79, 8)] {
        let m = focus::measure_session_at(&*s, Some(side), &stop).unwrap();
        assert_eq!(m.scale, scale);
        assert!(m != focus::Focus::UNAVAILABLE, "{side}: {m:?}");
    }
}
