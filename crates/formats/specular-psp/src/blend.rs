//! Blend modes through the formulas shared by the layered-image readers
//! ([`specular_blend`]): the separable ones as W3C defines them, the legacy colour modes in
//! HSL and PSP 8's "true" ones as W3C's luminosity-preserving non-separable modes.

use specular_blend::Mode;

use crate::BlendMode;

type Rgb = [f32; 3];

/// The shared mode a PaintShop Pro blend mode is drawn with.
fn shared(mode: BlendMode) -> Mode {
    use BlendMode::*;
    match mode {
        Normal | Dissolve | Adjust | Other(_) => Mode::Normal,
        Darken => Mode::Darken,
        Lighten => Mode::Lighten,
        Multiply => Mode::Multiply,
        Screen => Mode::Screen,
        Overlay => Mode::Overlay,
        HardLight => Mode::HardLight,
        SoftLight => Mode::SoftLight,
        Difference => Mode::Difference,
        Dodge => Mode::ColorDodge,
        Burn => Mode::ColorBurn,
        Exclusion => Mode::Exclusion,
        LegacyHue => Mode::HslHue,
        LegacySaturation => Mode::HslSaturation,
        LegacyColor => Mode::HslColor,
        LegacyLuminosity => Mode::HslLightness,
        TrueHue => Mode::Hue,
        TrueSaturation => Mode::Saturation,
        TrueColor => Mode::Color,
        TrueLightness => Mode::Luminosity,
    }
}

/// The blended colour of source `s` over backdrop `b` (before alpha compositing).
#[inline]
pub(crate) fn blend(mode: BlendMode, b: Rgb, s: Rgb) -> Rgb {
    match shared(mode) {
        Mode::Normal => s,
        m => specular_blend::blend3(m, b, s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_on_known_values() {
        let b = [0.25, 0.5, 1.0];
        let s = [0.5, 0.5, 0.0];
        assert_eq!(blend(BlendMode::Multiply, b, s), [0.125, 0.25, 0.0]);
        assert_eq!(blend(BlendMode::Darken, b, s), [0.25, 0.5, 0.0]);
        assert_eq!(blend(BlendMode::Difference, b, s), [0.25, 0.0, 1.0]);
        assert_eq!(blend(BlendMode::Other(99), b, s), s);
        // Luminosity of the result is the backdrop's for true colour.
        let c = blend(BlendMode::TrueColor, b, s);
        assert!((specular_blend::lum(c) - specular_blend::lum(b)).abs() < 1e-4);
        // Legacy luminosity takes the source's HSL lightness.
        let l = specular_blend::to_hsl(blend(BlendMode::LegacyLuminosity, b, s));
        assert!((l[2] - specular_blend::to_hsl(s)[2]).abs() < 1e-5);
    }
}
