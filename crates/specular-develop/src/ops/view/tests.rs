//! View transform properties: every method monotone and bounded, mid grey fixed
//! (D6) in SDR and HDR, neutrals neutral, and each method's hue behaviour.

use super::super::cpu_ref;
use super::{GREY, eval, for_headroom, headroom, uniforms};
use crate::model::params::{ModuleParams, ViewMethod, ViewTransform};
use crate::model::{Edit, Params, Registry, SourceKey};

const METHODS: [ViewMethod; 3] = [ViewMethod::Sigmoid, ViewMethod::Filmic, ViewMethod::Agx];

fn vt(method: ViewMethod) -> ViewTransform {
    ViewTransform {
        method,
        ..ViewTransform::default()
    }
}

fn apply(v: &ViewTransform, rgb: [f32; 3]) -> [f32; 3] {
    let u = uniforms(&Params::ViewTransform(v.clone()));
    let o = cpu_ref::apply("view_v1", &u, [rgb[0], rgb[1], rgb[2], 1.0]);
    [o[0], o[1], o[2]]
}

/// Scene values from 2^-16 to 2^10 times grey.
fn ramp() -> impl Iterator<Item = f64> {
    (0..=520).map(|i| GREY * (f64::from(i) / 20.0 - 16.0).exp2())
}

#[test]
fn every_method_is_monotone_bounded_and_keeps_mid_grey() {
    for m in METHODS {
        for (contrast, skew) in [(1.5, 0.0), (0.6, 0.0), (3.0, 0.0), (1.5, -1.0), (1.5, 1.5)] {
            for white in [1.0f32, 1.3, 4.0, 16.0] {
                for black in [0.0f32, 0.01] {
                    let v = ViewTransform {
                        contrast,
                        skew,
                        white_target: white,
                        black_target: black,
                        ..vt(m)
                    };
                    let tag = format!("{m:?} c{contrast} s{skew} w{white} b{black}");
                    let g = eval(&v, GREY);
                    assert!((g - GREY).abs() < 1e-9, "{tag}: grey → {g}");
                    let mut last = f64::from(black) - 1e-12;
                    for x in ramp() {
                        let y = eval(&v, x);
                        assert!(y >= last, "{tag}: {x} → {y} < {last}");
                        assert!(y <= f64::from(white) + 1e-9, "{tag}: {x} → {y}");
                        last = y;
                    }
                    assert_eq!(eval(&v, 0.0), f64::from(black), "{tag}");
                    assert_eq!(eval(&v, -1.0), f64::from(black), "{tag}");
                    // The f32 kernel agrees: grey within float rounding.
                    let o = apply(&v, [GREY as f32; 3]);
                    for c in o {
                        assert!((f64::from(c) - GREY).abs() < 2e-4, "{tag}: {o:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn neutrals_stay_neutral() {
    for m in METHODS {
        let v = vt(m);
        for x in ramp().step_by(13) {
            let o = apply(&v, [x as f32; 3]);
            let spread = o[0].max(o[1]).max(o[2]) - o[0].min(o[1]).min(o[2]);
            assert!(spread <= 2e-6 * o[1].max(1e-3), "{m:?} {x}: {o:?}");
            let want = eval(&v, x) as f32;
            assert!(
                (o[1] - want).abs() <= 2e-4 * want.max(0.01),
                "{m:?} {x}: {o:?} {want}"
            );
        }
    }
}

/// Filmic-style ends at scene white; the sigmoid and AgX-style approach white smoothly.
#[test]
fn ends_of_the_curves() {
    let f = vt(ViewMethod::Filmic);
    let white = GREY * super::curves::FILMIC_RANGE.1.exp2();
    assert!((eval(&f, white * 1.0001) - 1.0).abs() < 1e-9);
    assert!(eval(&f, white / 4.0) < 0.99);
    let a = vt(ViewMethod::Agx);
    assert!(eval(&a, GREY * 2f64.powi(7)) > 0.999);
    assert!(eval(&vt(ViewMethod::Sigmoid), 1e6) > 0.999);
}

/// With full hue preservation and no inset, the sigmoid and filmic-style keep each
/// pixel's hue: the middle channel keeps its place between the others.
#[test]
fn hue_preserving_methods_keep_hue() {
    for m in [ViewMethod::Sigmoid, ViewMethod::Filmic] {
        let v = ViewTransform {
            inset: 0.0,
            outset: 0.0,
            hue_preservation: 1.0,
            ..vt(m)
        };
        for px in [
            [2.0f32, 0.5, 0.1],
            [0.05, 0.2, 0.9],
            [8.0, 7.0, 0.3],
            [0.3, 0.02, 0.1],
        ] {
            let o = apply(&v, px);
            let order = |c: [f32; 3]| {
                let mut i = [0usize, 1, 2];
                i.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
                i
            };
            let [lo, mid, hi] = order(px);
            assert_eq!(order(o), [lo, mid, hi], "{m:?} {px:?} {o:?}");
            let r_in = (px[mid] - px[lo]) / (px[hi] - px[lo]);
            let r_out = (o[mid] - o[lo]) / (o[hi] - o[lo]);
            assert!((r_in - r_out).abs() < 1e-4, "{m:?} {px:?}: {r_in} {r_out}");
        }
    }
}

/// AgX-style: bright saturated colours go smoothly to white. Raising the exposure of a
/// saturated colour never raises its saturation once past grey, ends near white, and the
/// dominant channel stays dominant all the way.
#[test]
fn agx_runs_bright_colours_to_white() {
    let v = vt(ViewMethod::Agx);
    for base in [
        [1.0f32, 0.1, 0.05],
        [0.08, 1.0, 0.2],
        [0.1, 0.15, 1.0],
        [1.0, 0.8, 0.05],
    ] {
        let dom = (0..3).max_by(|&a, &b| base[a].total_cmp(&base[b])).unwrap();
        let mut last_sat = f32::MAX;
        for i in 0..=48 {
            let k = (i as f32 / 4.0 - 2.0).exp2() * GREY as f32;
            let o = apply(&v, base.map(|c| c * k));
            let mx = o[0].max(o[1]).max(o[2]);
            let mn = o[0].min(o[1]).min(o[2]);
            let sat = (mx - mn) / mx.max(1e-6);
            assert!(o[dom] >= mx - 1e-6, "{base:?} ×{k}: {o:?}");
            assert!(
                sat <= last_sat + 1e-3,
                "{base:?} ×{k}: {sat} after {last_sat}"
            );
            last_sat = sat;
        }
        assert!(
            last_sat < 0.05,
            "{base:?}: still {last_sat} saturated at the top"
        );
    }
}

/// The SDR rendition is the edit itself; HDR scales only the white target, keeps grey and
/// shadows, and gives highlights room up to the headroom.
#[test]
fn headroom_follows_the_peak_and_the_limit() {
    let reg = Registry::builtin();
    let e = Edit::default_for(reg, SourceKey::default(), false);
    let limit = ViewTransform::default().hdr_limit_ev.exp2();
    assert_eq!(headroom(&e, 1.0), 1.0);
    assert_eq!(headroom(&e, 0.5), 1.0);
    assert_eq!(headroom(&e, 2.5), 2.5);
    assert_eq!(headroom(&e, 100.0), limit);
    assert_eq!(headroom(&e, f32::NAN), 1.0);
    assert!(matches!(
        for_headroom(&e, 1.0),
        std::borrow::Cow::Borrowed(_)
    ));
    for m in METHODS {
        let sdr = vt(m);
        let hdr_e = {
            let mut e = e.clone();
            let mm = e
                .module_mut(crate::model::ModuleKind::VIEW_TRANSFORM, 0)
                .unwrap();
            mm.params = Params::ViewTransform(sdr.clone());
            for_headroom(&e, 4.0).into_owned()
        };
        let hdr = super::settings(&hdr_e);
        assert_eq!(hdr.white_target, 4.0);
        assert_eq!(hdr.method, m);
        assert!((eval(&hdr, GREY) - GREY).abs() < 1e-9);
        // Deep shadows barely move; highlights get brighter, up to the headroom.
        let d = GREY / 16.0;
        assert!((eval(&hdr, d) / eval(&sdr, d) - 1.0).abs() < 0.3, "{m:?}");
        let b = GREY * 32.0;
        assert!(eval(&hdr, b) > 1.5 * eval(&sdr, b), "{m:?}");
        assert!(eval(&hdr, 1e5) <= 4.0 + 1e-9);
    }
}

#[test]
fn the_method_round_trips_as_a_field() {
    for m in METHODS {
        let v = vt(m);
        assert_eq!(ViewTransform::from_fields(&v.to_fields()).method, m);
    }
}
