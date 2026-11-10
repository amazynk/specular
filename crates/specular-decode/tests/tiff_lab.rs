//! L*a*b* TIFF through the registry: the colour model is reported, an attached profile is not
//! passed on (the samples are device-independent and already converted to sRGB), and the
//! picture matches its RGB original (ImageMagick fixtures from specular-tiff's tests).

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Registry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats/specular-tiff/tests/fixtures/lab")
        .join(name)
}

#[test]
fn lab_tiff_session() {
    let reg = Registry::new(); // installs the CMYK and L*a*b* hooks
    let open = |name: &str| {
        reg.open(Arc::new(Source::open(fixture(name)).unwrap()))
            .unwrap()
    };
    let rgba = |name: &str| {
        let s = open(name);
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r.layout, PixelLayout::Rgba, "{name}");
        let pl = &r.planes[0];
        let mut out = Vec::new();
        for y in 0..r.height as usize {
            out.extend_from_slice(&pl.data[y * pl.stride..y * pl.stride + r.width as usize * 4]);
        }
        out
    };
    assert_eq!(open("rose-rgb.tif").colour_model(), Some("RGB"));
    let want = rgba("rose-rgb.tif");
    for name in ["rose-lab8.tif", "rose-lab16.tif", "rose-lab8-icc.tif"] {
        let s = open(name);
        assert_eq!(s.colour_model(), Some("Lab"), "{name}");
        assert!(
            s.info().icc_profile.is_none(),
            "{name}: no profile for the GPU"
        );
        let got = rgba(name);
        // ImageMagick's L*a*b* is relative to D65 (plain XYZ scaling), the decoder reads it
        // as D50: a few codes apart on saturated colours.
        let max = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max();
        assert!(max <= Some(16), "{name}: {max:?}");
    }
}
