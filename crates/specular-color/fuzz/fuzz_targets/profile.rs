//! Fuzz ICC profile parsing and everything built from a parsed profile: transforms to and from
//! sRGB and to itself in every intent (with and without black point compensation), evaluation
//! at edge values (out of range, NaN, infinity), the GPU source LUT and, for some inputs, the
//! baked display cube and CMYK table. Run from this directory with
//! `cargo +nightly fuzz run profile corpus/profile -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_color::{
    gpu, profiles, CmykConverter, ColorSpace, Display, Intent, Profile, Transform,
};

const EDGES: [f32; 8] = [0.0, 0.25, 0.5, 1.0, -1.0, 2.0, f32::NAN, f32::INFINITY];

fuzz_target!(|data: &[u8]| {
    let Ok(p) = Profile::parse(data) else { return };
    let _ = (p.is_matrix_shaper(), p.a2b_for(1), p.b2a_for(2));
    let n = p.color_space.channels();
    let srgb = profiles::srgb();
    let sel = data.len();
    let intent = [
        Intent::Perceptual,
        Intent::Relative,
        Intent::Saturation,
        Intent::Absolute,
    ][sel % 4];
    let bpc = sel % 3 != 0;
    let mut out = [0f32; 16];
    if let Ok(t) = Transform::new(&p, &srgb, intent, bpc) {
        assert_eq!(t.inputs(), n);
        let mut input = [0f32; 16];
        for (i, v) in EDGES.iter().enumerate() {
            input[..n].fill(*v);
            t.eval(&input[..n], &mut out);
            // A ramp across channels reaches different CLUT cells.
            for (c, x) in input[..n].iter_mut().enumerate() {
                *x = ((i + c) % 5) as f32 / 4.0;
            }
            t.eval(&input[..n], &mut out);
        }
    }
    if let Ok(t) = Transform::new(&srgb, &p, intent, bpc) {
        assert_eq!(t.outputs(), n);
        for v in EDGES {
            t.eval(&[v, 1.0 - v, 0.5], &mut out);
        }
    }
    // Profile to itself: both directions of the same tags.
    if let Ok(t) = Transform::new(&p, &p, intent, bpc) {
        t.eval(&[0.3f32; 16][..n], &mut out);
    }
    let _ = gpu::source_lut_for(&p, intent, bpc);
    // The baked tables evaluate tens of thousands of points: only for some inputs.
    if sel % 8 == 0 {
        let d = Display::for_profile(&p, intent, bpc);
        let _ = d.apply([0.2, 0.5, 0.8]);
        if p.color_space == ColorSpace::Cmyk {
            if let Ok(c) = CmykConverter::new(&p, intent, bpc) {
                let _ = c.convert([10, 20, 30, 40]);
                let mut rgba = [0u8; 8];
                c.convert_row(&[0, 64, 128, 255, 255, 0, 0, 0], &mut rgba);
            }
        }
    }
});
