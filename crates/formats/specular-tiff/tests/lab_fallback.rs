//! L*a*b* TIFF with no hook installed: the decoder's built-in scalar formula, against lcms2
//! (test oracle only). A separate test binary, as the hook is process-wide.

mod common;
#[path = "common/lab.rs"]
mod lab;

use lab::{diff, make, matrix};
use specular_tiff::{DecodeOptions, Tiff, downscale};

#[test]
fn builtin_formula_matches_lcms() {
    for (i, case) in matrix().iter().enumerate() {
        let (f, want) = make(case, 37, 19, i as u32);
        let t = Tiff::open(&f).unwrap();
        let d = t.decode(&f, 0, &DecodeOptions::default()).unwrap();
        let (dmax, alpha) = diff(&d.data, &want);
        assert!(
            dmax <= 1 && alpha,
            "{case:?}: max diff {dmax}, alpha ok {alpha}"
        );
        if i % 17 == 0 {
            let s = t
                .decode(
                    &f,
                    0,
                    &DecodeOptions {
                        scale: 4,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(s.data == downscale(&d, 4).data, "{case:?} at 1/4");
        }
    }
}
