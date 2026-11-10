//! Embedded ICC profiles → the convert shader's source transform (specular-color): per-channel
//! curves plus a 3×4 matrix for matrix/TRC profiles, a 33³ cube for LUT-based ones, nothing
//! for sRGB and near-sRGB profiles (the shader's exact sRGB decode is used then).

use std::time::Instant;

pub use specular_color::SourceLut;
use specular_color::gpu::{CUBE_SIZE, CURVE_SIZE};

/// Grid points per axis of cube LUTs.
pub const LUT_SIZE: u32 = CUBE_SIZE as u32;
/// Samples of curve LUTs.
pub const CURVE_LEN: u32 = CURVE_SIZE as u32;

/// The source transform for an embedded profile under the current colour-management policy
/// ([`specular_color::set_policy`]). `None` for profiles that are sRGB already, not RGB/gray
/// (CMYK and L*a*b* are converted while decoding), unusable, or when colour management is off.
/// Built on a worker thread.
pub fn build_lut(icc: &[u8]) -> Option<SourceLut> {
    let t0 = Instant::now();
    let lut = specular_color::policy::source_lut(icc);
    specular_log::debug!(
        target: specular_log::COLOUR,
        "icc source transform: {} in {:.2} ms",
        match &lut {
            None => "none (sRGB)",
            Some(SourceLut::Curves { .. }) => "curves + matrix",
            Some(SourceLut::Cube { .. }) => "cube",
        },
        t0.elapsed().as_secs_f64() * 1e3
    );
    lut
}

/// f32 → IEEE half bits (round to nearest even), for Rgba16Float upload.
pub fn f16_bits(v: f32) -> u16 {
    specular_color::math::f16_bits(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half() {
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(0.5), 0x3800);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(65504.0), 0x7bff);
    }

    #[test]
    fn srgb_profile_needs_no_lut() {
        let p = moxcms::ColorProfile::new_srgb().encode().unwrap();
        assert!(build_lut(&p).is_none());
        let p3 = moxcms::ColorProfile::new_display_p3().encode().unwrap();
        match build_lut(&p3).expect("P3 differs from sRGB") {
            SourceLut::Curves { data, .. } => assert_eq!(data.len(), CURVE_SIZE * 4),
            SourceLut::Cube { .. } => panic!("P3 is a matrix/TRC profile"),
        }
    }
}
