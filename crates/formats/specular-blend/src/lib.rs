//! Layer blend modes shared by Specular's layered-image readers: Photoshop (PSD/PSB),
//! PaintShop Pro, GIMP XCF, Krita and OpenRaster.
//!
//! [`Mode`] is the blend function `B(cb, cs)` of a backdrop colour `cb` and a source colour
//! `cs` (straight, normally 0..1): the W3C *Compositing and Blending Level 1* separable and
//! non-separable modes, the extras Photoshop's file format specification and the
//! OpenRaster specification name (linear, vivid, pin and hard mix lights, subtract,
//! divide, darker and lighter colour), grain merge and extract, the HSL modes of older
//! editors and a few of Krita's documented arithmetic modes. [`float`] holds the
//! unclamped variants float pipelines use, and [`PorterDuff`] the compositing operators of
//! W3C and OpenRaster, so each reader maps its own mode codes here and keeps only its
//! compositing loop.
//!
//! All formulas come from the public specifications (W3C, Adobe, OpenRaster) and the
//! modes' documented meaning; nothing here is taken from another program's code.

/// A blend mode: how a source colour combines with the backdrop where both are present.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The source colour.
    Normal,
    Multiply,
    Screen,
    /// Hard light with backdrop and source swapped.
    Overlay,
    Darken,
    Lighten,
    /// W3C `color-dodge` (Photoshop's).
    ColorDodge,
    /// W3C `color-burn` (Photoshop's).
    ColorBurn,
    HardLight,
    /// W3C `soft-light`: a cubic below a quarter of the backdrop, its square root above.
    SoftLight,
    /// Soft light with the square root throughout (Krita's `soft_light`).
    SoftLightSqrt,
    /// The "Pegtop" soft light: multiply and screen mixed by the backdrop (GIMP 2.10's).
    SoftLightPegtop,
    Difference,
    Exclusion,
    /// Add, clamped (Photoshop's linear dodge).
    LinearDodge,
    /// `cb + cs − 1`, clamped.
    LinearBurn,
    /// `cb − cs`, clamped.
    Subtract,
    /// `cb / cs`, clamped.
    Divide,
    VividLight,
    LinearLight,
    PinLight,
    /// 0 or 1 by whether `cb + cs` reaches 1.
    HardMix,
    /// `cb − cs + ½`, clamped.
    GrainExtract,
    /// `cb + cs − ½`, clamped.
    GrainMerge,
    /// Mean of the two.
    Allanon,
    /// Twice the harmonic mean.
    Parallel,
    GeometricMean,
    /// `1 − |1 − cb − cs|`.
    Negation,
    /// `|√cb − √cs|`.
    AdditiveSubtractive,
    /// `cb^(1/cs)`.
    GammaDark,
    /// `cb^cs`.
    GammaLight,
    /// `2·atan(cs / cb) / π`.
    ArcTangent,
    /// `cb² / (1 − cs)`.
    Reflect,
    /// Reflect with the two swapped.
    Glow,
    /// `1 − (1 − cb)² / cs`.
    Freeze,
    /// Freeze with the two swapped.
    Heat,
    // Non-separable (whole colours).
    /// The source's hue with the backdrop's saturation and luminosity (W3C).
    Hue,
    Saturation,
    Color,
    Luminosity,
    /// Whichever whole colour has the lower luminosity.
    DarkerColor,
    LighterColor,
    /// HSL hue of the source, saturation and lightness of the backdrop (older editors).
    HslHue,
    HslSaturation,
    HslColor,
    HslLightness,
}

impl Mode {
    /// Whether the mode blends whole colours (its result per channel depends on the
    /// others): use [`blend3`], not [`blend1`].
    pub fn non_separable(self) -> bool {
        matches!(
            self,
            Mode::Hue
                | Mode::Saturation
                | Mode::Color
                | Mode::Luminosity
                | Mode::DarkerColor
                | Mode::LighterColor
                | Mode::HslHue
                | Mode::HslSaturation
                | Mode::HslColor
                | Mode::HslLightness
        )
    }

    /// The OpenRaster `composite-op` of a blend, the separable and non-separable `svg:`
    /// names of the OpenRaster specification (with the W3C names it leaves out). Porter-Duff
    /// operators are [`PorterDuff::from_openraster`].
    pub fn from_openraster(op: &str) -> Option<Mode> {
        Some(match op.strip_prefix("svg:")? {
            "src-over" | "normal" => Mode::Normal,
            "multiply" => Mode::Multiply,
            "screen" => Mode::Screen,
            "overlay" => Mode::Overlay,
            "darken" => Mode::Darken,
            "lighten" => Mode::Lighten,
            "color-dodge" => Mode::ColorDodge,
            "color-burn" => Mode::ColorBurn,
            "hard-light" => Mode::HardLight,
            "soft-light" => Mode::SoftLight,
            "difference" => Mode::Difference,
            "exclusion" => Mode::Exclusion,
            "hue" => Mode::Hue,
            "saturation" => Mode::Saturation,
            "color" => Mode::Color,
            "luminosity" => Mode::Luminosity,
            _ => return None,
        })
    }

    /// Krita's composite op id (as `maindoc.xml` and Krita's OpenRaster files spell it).
    /// Krita's hue, saturation, colour and luminosity work in HSY; the W3C modes stand in.
    pub fn from_krita(id: &str) -> Option<Mode> {
        Some(match id {
            "normal" => Mode::Normal,
            "multiply" => Mode::Multiply,
            "screen" => Mode::Screen,
            "overlay" => Mode::Overlay,
            "darken" => Mode::Darken,
            "lighten" => Mode::Lighten,
            "add" | "linear_dodge" => Mode::LinearDodge,
            "subtract" => Mode::Subtract,
            "inverse_subtract" | "linear_burn" => Mode::LinearBurn,
            "diff" => Mode::Difference,
            "exclusion" => Mode::Exclusion,
            "dodge" => Mode::ColorDodge,
            "burn" => Mode::ColorBurn,
            "soft_light" => Mode::SoftLightSqrt,
            "soft_light_svg" => Mode::SoftLight,
            "soft_light_pegtop_delphi" => Mode::SoftLightPegtop,
            "hard_light" => Mode::HardLight,
            "vivid_light" => Mode::VividLight,
            "linear_light" => Mode::LinearLight,
            "pin_light" => Mode::PinLight,
            "hard_mix" | "hard_mix_photoshop" => Mode::HardMix,
            "divide" => Mode::Divide,
            "grain_extract" => Mode::GrainExtract,
            "grain_merge" => Mode::GrainMerge,
            "allanon" => Mode::Allanon,
            "parallel" => Mode::Parallel,
            "geometric_mean" => Mode::GeometricMean,
            "negation" => Mode::Negation,
            "additive_subtractive" => Mode::AdditiveSubtractive,
            "gamma_dark" => Mode::GammaDark,
            "gamma_light" => Mode::GammaLight,
            "arc_tangent" => Mode::ArcTangent,
            "reflect" => Mode::Reflect,
            "glow" => Mode::Glow,
            "freeze" => Mode::Freeze,
            "heat" => Mode::Heat,
            "darker color" => Mode::DarkerColor,
            "lighter color" => Mode::LighterColor,
            "hue" => Mode::Hue,
            "saturation" => Mode::Saturation,
            "color" => Mode::Color,
            "luminize" => Mode::Luminosity,
            "hue_hsl" => Mode::HslHue,
            "saturation_hsl" => Mode::HslSaturation,
            "color_hsl" => Mode::HslColor,
            "lightness" => Mode::HslLightness,
            _ => return None,
        })
    }

    /// A Photoshop blend mode key (layer records, section dividers, layer effects).
    /// Dissolve, pass-through and unknown keys are `None`.
    pub fn from_photoshop(key: &[u8; 4]) -> Option<Mode> {
        Some(match key {
            b"norm" => Mode::Normal,
            b"mul " => Mode::Multiply,
            b"scrn" => Mode::Screen,
            b"over" => Mode::Overlay,
            b"dark" => Mode::Darken,
            b"lite" => Mode::Lighten,
            b"diff" => Mode::Difference,
            b"smud" => Mode::Exclusion,
            b"div " => Mode::ColorDodge,
            b"idiv" => Mode::ColorBurn,
            b"lddg" => Mode::LinearDodge,
            b"lbrn" => Mode::LinearBurn,
            b"hLit" => Mode::HardLight,
            b"sLit" => Mode::SoftLight,
            b"vLit" => Mode::VividLight,
            b"lLit" => Mode::LinearLight,
            b"pLit" => Mode::PinLight,
            b"hMix" => Mode::HardMix,
            b"fsub" => Mode::Subtract,
            b"fdiv" => Mode::Divide,
            b"hue " => Mode::Hue,
            b"sat " => Mode::Saturation,
            b"colr" => Mode::Color,
            b"lum " => Mode::Luminosity,
            b"dkCl" => Mode::DarkerColor,
            b"lgCl" => Mode::LighterColor,
            _ => return None,
        })
    }
}

/// Screen of two values.
#[inline]
pub fn screen(cb: f32, cs: f32) -> f32 {
    cb + cs - cb * cs
}

/// Hard light of backdrop `cb` and source `cs` (overlay is hard light with them swapped).
#[inline]
pub fn hard_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb * 2.0 * cs
    } else {
        screen(cb, 2.0 * cs - 1.0)
    }
}

/// W3C colour dodge.
#[inline]
pub fn color_dodge(cb: f32, cs: f32) -> f32 {
    if cb <= 0.0 {
        0.0
    } else if cs >= 1.0 {
        1.0
    } else {
        (cb / (1.0 - cs)).min(1.0)
    }
}

/// W3C colour burn.
#[inline]
pub fn color_burn(cb: f32, cs: f32) -> f32 {
    if cb >= 1.0 {
        1.0
    } else if cs <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - cb) / cs).min(1.0)
    }
}

/// W3C soft light.
#[inline]
pub fn soft_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
    } else {
        let d = if cb <= 0.25 {
            ((16.0 * cb - 12.0) * cb + 4.0) * cb
        } else {
            cb.sqrt()
        };
        cb + (2.0 * cs - 1.0) * (d - cb)
    }
}

/// Separable blend `B(cb, cs)` of one channel. Non-separable modes give the source.
#[inline]
pub fn blend1(mode: Mode, cb: f32, cs: f32) -> f32 {
    match mode {
        Mode::Multiply => cb * cs,
        Mode::Screen => screen(cb, cs),
        Mode::Overlay => hard_light(cs, cb),
        Mode::Darken => cb.min(cs),
        Mode::Lighten => cb.max(cs),
        Mode::ColorDodge => color_dodge(cb, cs),
        Mode::ColorBurn => color_burn(cb, cs),
        Mode::HardLight => hard_light(cb, cs),
        Mode::SoftLight => soft_light(cb, cs),
        Mode::SoftLightSqrt => {
            let cb = cb.max(0.0);
            if cs > 0.5 {
                cb + (2.0 * cs - 1.0) * (cb.sqrt() - cb)
            } else {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            }
        }
        Mode::SoftLightPegtop => float::soft_light_pegtop(cb, cs),
        Mode::Difference => (cb - cs).abs(),
        Mode::Exclusion => cb + cs - 2.0 * cb * cs,
        Mode::LinearDodge => (cb + cs).min(1.0),
        Mode::LinearBurn => (cb + cs - 1.0).max(0.0),
        Mode::Subtract => (cb - cs).max(0.0),
        Mode::Divide => {
            if cs <= 0.0 {
                1.0
            } else {
                (cb / cs).min(1.0)
            }
        }
        Mode::VividLight => {
            if cs <= 0.5 {
                color_burn(cb, 2.0 * cs)
            } else {
                color_dodge(cb, 2.0 * cs - 1.0)
            }
        }
        Mode::LinearLight => (cb + 2.0 * cs - 1.0).clamp(0.0, 1.0),
        Mode::PinLight => {
            if cs <= 0.5 {
                cb.min(2.0 * cs)
            } else {
                cb.max(2.0 * cs - 1.0)
            }
        }
        Mode::HardMix => {
            if cb + cs < 1.0 {
                0.0
            } else {
                1.0
            }
        }
        Mode::GrainExtract => (cb - cs + 0.5).clamp(0.0, 1.0),
        Mode::GrainMerge => (cb + cs - 0.5).clamp(0.0, 1.0),
        Mode::Allanon => (cb + cs) * 0.5,
        Mode::Parallel => {
            if cb <= 0.0 || cs <= 0.0 {
                0.0
            } else {
                (2.0 / (1.0 / cb + 1.0 / cs)).min(1.0)
            }
        }
        Mode::GeometricMean => (cb.max(0.0) * cs.max(0.0)).sqrt(),
        Mode::Negation => 1.0 - (1.0 - cb - cs).abs(),
        Mode::AdditiveSubtractive => (cb.max(0.0).sqrt() - cs.max(0.0).sqrt()).abs(),
        Mode::GammaDark => {
            if cs <= 0.0 {
                0.0
            } else {
                cb.max(0.0).powf(1.0 / cs)
            }
        }
        Mode::GammaLight => cb.max(0.0).powf(cs),
        Mode::ArcTangent => {
            if cb <= 0.0 {
                if cs <= 0.0 { 0.0 } else { 1.0 }
            } else {
                (2.0 * (cs / cb).atan() / std::f32::consts::PI).clamp(0.0, 1.0)
            }
        }
        Mode::Reflect => reflect(cb, cs),
        Mode::Glow => reflect(cs, cb),
        Mode::Freeze => freeze(cb, cs),
        Mode::Heat => freeze(cs, cb),
        Mode::Normal
        | Mode::Hue
        | Mode::Saturation
        | Mode::Color
        | Mode::Luminosity
        | Mode::DarkerColor
        | Mode::LighterColor
        | Mode::HslHue
        | Mode::HslSaturation
        | Mode::HslColor
        | Mode::HslLightness => cs,
    }
}

fn reflect(cb: f32, cs: f32) -> f32 {
    if cs >= 1.0 {
        1.0
    } else {
        (cb * cb / (1.0 - cs)).min(1.0)
    }
}

fn freeze(cb: f32, cs: f32) -> f32 {
    if cs <= 0.0 {
        0.0
    } else {
        (1.0 - (1.0 - cb) * (1.0 - cb) / cs).max(0.0)
    }
}

/// Luminosity as W3C defines it (Rec. 601 weights).
#[inline]
pub fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut c = c;
    if n < 0.0 && l - n > 0.0 {
        c = c.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 && x - l > 0.0 {
        c = c.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    c
}

/// W3C `SetLum`.
pub fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

/// W3C `Sat`.
pub fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

/// W3C `SetSat`.
pub fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let x = c[0].max(c[1]).max(c[2]);
    let n = c[0].min(c[1]).min(c[2]);
    if x <= n {
        return [0.0; 3];
    }
    c.map(|v| (v - n) * s / (x - n))
}

/// RGB to hue, saturation, lightness (each 0..1).
pub fn to_hsl(c: [f32; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d <= 0.0 {
        return [0.0, 0.0, l];
    }
    let s = if l <= 0.5 {
        d / (mx + mn)
    } else {
        d / (2.0 - mx - mn)
    };
    let h = if mx == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h / 6.0, s, l]
}

/// Hue, saturation, lightness to RGB.
pub fn from_hsl(h: [f32; 3]) -> [f32; 3] {
    let (hh, s, l) = (h[0], h[1], h[2]);
    if s <= 0.0 {
        return [l, l, l];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [f(hh + 1.0 / 3.0), f(hh), f(hh - 1.0 / 3.0)]
}

#[inline]
/// Blend of whole colours: non-separable modes on the three channels, separable ones per
/// channel.
pub fn blend3(mode: Mode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    match mode {
        Mode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        Mode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        Mode::Color => set_lum(cs, lum(cb)),
        Mode::Luminosity => set_lum(cb, lum(cs)),
        Mode::DarkerColor => {
            if lum(cs) < lum(cb) {
                cs
            } else {
                cb
            }
        }
        Mode::LighterColor => {
            if lum(cs) > lum(cb) {
                cs
            } else {
                cb
            }
        }
        Mode::HslHue | Mode::HslSaturation | Mode::HslColor | Mode::HslLightness => {
            let (hb, hs) = (to_hsl(cb), to_hsl(cs));
            from_hsl(match mode {
                Mode::HslHue => [hs[0], hb[1], hb[2]],
                Mode::HslSaturation => [hb[0], hs[1], hb[2]],
                Mode::HslColor => [hs[0], hs[1], hb[2]],
                _ => [hb[0], hb[1], hs[2]],
            })
        }
        _ => [0, 1, 2].map(|c| blend1(mode, cb[c], cs[c])),
    }
}

/// Unclamped variants for float pipelines (values may leave 0..1), with divisions guarded
/// so that near-zero numerators give 0 and quotients stay finite.
pub mod float {
    const EPS: f32 = 1e-6;

    /// `a / b`, 0 for a near-zero `a`, kept within ±1/ε.
    #[inline]
    pub fn safe_div(a: f32, b: f32) -> f32 {
        if a.abs() > EPS {
            (a / b).clamp(-1.0 / EPS, 1.0 / EPS)
        } else {
            0.0
        }
    }

    #[inline]
    pub fn add(cb: f32, cs: f32) -> f32 {
        cb + cs
    }

    #[inline]
    pub fn subtract(cb: f32, cs: f32) -> f32 {
        cb - cs
    }

    #[inline]
    pub fn divide(cb: f32, cs: f32) -> f32 {
        safe_div(cb, cs)
    }

    #[inline]
    pub fn color_dodge(cb: f32, cs: f32) -> f32 {
        safe_div(cb, 1.0 - cs)
    }

    #[inline]
    pub fn color_burn(cb: f32, cs: f32) -> f32 {
        1.0 - safe_div(1.0 - cb, cs)
    }

    #[inline]
    pub fn linear_burn(cb: f32, cs: f32) -> f32 {
        cb + cs - 1.0
    }

    #[inline]
    pub fn linear_light(cb: f32, cs: f32) -> f32 {
        if cs <= 0.5 {
            cb + 2.0 * cs - 1.0
        } else {
            cb + 2.0 * (cs - 0.5)
        }
    }

    /// Vivid light clamped only on the side each half can overshoot.
    #[inline]
    pub fn vivid_light(cb: f32, cs: f32) -> f32 {
        if cs <= 0.5 {
            (1.0 - safe_div(1.0 - cb, 2.0 * cs)).max(0.0)
        } else {
            safe_div(cb, 2.0 * (1.0 - cs)).min(1.0)
        }
    }

    /// Hard light capped at 1 on both halves.
    #[inline]
    pub fn hard_light(cb: f32, cs: f32) -> f32 {
        if cs > 0.5 {
            (1.0 - (1.0 - cb) * (1.0 - (cs - 0.5) * 2.0)).min(1.0)
        } else {
            (cb * (cs * 2.0)).min(1.0)
        }
    }

    /// Overlay as `2·cb·cs` below a half backdrop, its screen counterpart above.
    #[inline]
    pub fn overlay(cb: f32, cs: f32) -> f32 {
        if cb < 0.5 {
            2.0 * cb * cs
        } else {
            1.0 - 2.0 * (1.0 - cs) * (1.0 - cb)
        }
    }

    /// Pegtop soft light: multiply and screen mixed by the backdrop.
    #[inline]
    pub fn soft_light_pegtop(cb: f32, cs: f32) -> f32 {
        let multiply = cb * cs;
        let screen = 1.0 - (1.0 - cb) * (1.0 - cs);
        (1.0 - cb) * multiply + cb * screen
    }

    #[inline]
    pub fn grain_extract(cb: f32, cs: f32) -> f32 {
        cb - cs + 0.5
    }

    #[inline]
    pub fn grain_merge(cb: f32, cs: f32) -> f32 {
        cb + cs - 0.5
    }

    /// Exclusion written around the midpoint (equal to `cb + cs − 2·cb·cs`).
    #[inline]
    pub fn exclusion(cb: f32, cs: f32) -> f32 {
        0.5 - 2.0 * (cb - 0.5) * (cs - 0.5)
    }

    #[inline]
    pub fn pin_light(cb: f32, cs: f32) -> f32 {
        if cs > 0.5 {
            cb.max(2.0 * (cs - 0.5))
        } else {
            cb.min(2.0 * cs)
        }
    }

    #[inline]
    pub fn screen(cb: f32, cs: f32) -> f32 {
        1.0 - (1.0 - cb) * (1.0 - cs)
    }
}

/// A Porter-Duff compositing operator (W3C, OpenRaster `svg:` names).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PorterDuff {
    Clear,
    /// The source only.
    Copy,
    /// The backdrop only.
    Destination,
    SourceOver,
    DestinationOver,
    SourceIn,
    DestinationIn,
    SourceOut,
    DestinationOut,
    SourceAtop,
    DestinationAtop,
    Xor,
    /// Sum of both (W3C `plus-lighter` / OpenRaster `svg:plus`), clamped.
    Plus,
}

impl PorterDuff {
    /// An OpenRaster `composite-op` that is a compositing operator, not a blend.
    pub fn from_openraster(op: &str) -> Option<PorterDuff> {
        Some(match op.strip_prefix("svg:")? {
            "clear" => PorterDuff::Clear,
            "src" => PorterDuff::Copy,
            "dst" => PorterDuff::Destination,
            "src-over" => PorterDuff::SourceOver,
            "dst-over" => PorterDuff::DestinationOver,
            "src-in" => PorterDuff::SourceIn,
            "dst-in" => PorterDuff::DestinationIn,
            "src-out" => PorterDuff::SourceOut,
            "dst-out" => PorterDuff::DestinationOut,
            "src-atop" => PorterDuff::SourceAtop,
            "dst-atop" => PorterDuff::DestinationAtop,
            "xor" => PorterDuff::Xor,
            "plus" => PorterDuff::Plus,
            _ => return None,
        })
    }

    /// The factors `(Fa, Fb)` the source and backdrop are weighted by, from the source and
    /// backdrop alphas.
    #[inline]
    pub fn factors(self, a_s: f32, a_b: f32) -> (f32, f32) {
        match self {
            PorterDuff::Clear => (0.0, 0.0),
            PorterDuff::Copy => (1.0, 0.0),
            PorterDuff::Destination => (0.0, 1.0),
            PorterDuff::SourceOver => (1.0, 1.0 - a_s),
            PorterDuff::DestinationOver => (1.0 - a_b, 1.0),
            PorterDuff::SourceIn => (a_b, 0.0),
            PorterDuff::DestinationIn => (0.0, a_s),
            PorterDuff::SourceOut => (1.0 - a_b, 0.0),
            PorterDuff::DestinationOut => (0.0, 1.0 - a_s),
            PorterDuff::SourceAtop => (a_b, 1.0 - a_s),
            PorterDuff::DestinationAtop => (1.0 - a_b, a_s),
            PorterDuff::Xor => (1.0 - a_b, 1.0 - a_s),
            PorterDuff::Plus => (1.0, 1.0),
        }
    }
}

/// One pixel composited the W3C way: the source colour `cs` with alpha `a_s` blended with
/// mode `mode` where the backdrop (`cb`, `a_b`) is, then combined by operator `op`. Colours
/// are straight; the result is straight colour and alpha (colour 0 where alpha is 0).
#[inline]
pub fn composite(
    mode: Mode,
    op: PorterDuff,
    cb: [f32; 3],
    a_b: f32,
    cs: [f32; 3],
    a_s: f32,
) -> ([f32; 3], f32) {
    let (fa, fb) = op.factors(a_s, a_b);
    let a_o = (a_s * fa + a_b * fb).min(1.0);
    if a_o <= 0.0 {
        return ([0.0; 3], 0.0);
    }
    let mixed = if mode == Mode::Normal || a_b <= 0.0 {
        cs
    } else {
        let b = blend3(mode, cb, cs);
        [0, 1, 2].map(|c| (1.0 - a_b) * cs[c] + a_b * b[c])
    };
    let c = [0, 1, 2].map(|c| (a_s * fa * mixed[c] + a_b * fb * cb[c]) / a_o);
    (c, a_o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn separable_modes_on_known_values() {
        let (b, s) = (0.25f32, 0.5f32);
        assert_eq!(blend1(Mode::Multiply, b, s), 0.125);
        assert_eq!(blend1(Mode::Screen, b, s), 0.625);
        assert_eq!(blend1(Mode::Darken, b, s), 0.25);
        assert_eq!(blend1(Mode::Lighten, b, s), 0.5);
        assert_eq!(blend1(Mode::Difference, b, s), 0.25);
        assert_eq!(blend1(Mode::Exclusion, b, s), 0.5);
        assert_eq!(blend1(Mode::LinearDodge, 0.75, 0.5), 1.0);
        assert_eq!(blend1(Mode::LinearBurn, 0.25, 0.5), 0.0);
        assert_eq!(blend1(Mode::Subtract, 0.75, 0.5), 0.25);
        assert_eq!(blend1(Mode::Divide, 0.25, 0.5), 0.5);
        assert_eq!(blend1(Mode::ColorDodge, 0.25, 0.5), 0.5);
        assert_eq!(blend1(Mode::ColorBurn, 0.75, 0.5), 0.5);
        assert_eq!(blend1(Mode::HardMix, 0.5, 0.5), 1.0);
        assert_eq!(blend1(Mode::HardMix, 0.4, 0.5), 0.0);
        assert_eq!(blend1(Mode::Allanon, 0.2, 0.6), 0.4);
        assert_eq!(blend1(Mode::GrainMerge, 0.5, 0.75), 0.75);
        assert_eq!(blend1(Mode::GrainExtract, 0.5, 0.75), 0.25);
        // Overlay is hard light swapped; soft light leaves 0 and 1 alone.
        assert_eq!(
            blend1(Mode::Overlay, 0.25, 0.8),
            blend1(Mode::HardLight, 0.8, 0.25)
        );
        for m in [Mode::SoftLight, Mode::SoftLightSqrt, Mode::SoftLightPegtop] {
            assert_eq!(blend1(m, 0.0, 0.7), 0.0);
            assert!((blend1(m, 1.0, 0.3) - 1.0).abs() < 1e-6);
            // A half source leaves the backdrop as it is.
            assert!((blend1(m, 0.3, 0.5) - 0.3).abs() < 1e-6, "{m:?}");
        }
        // Vivid light is burn below a half source, dodge above.
        assert_eq!(blend1(Mode::VividLight, 0.6, 0.25), color_burn(0.6, 0.5));
        assert_eq!(blend1(Mode::VividLight, 0.3, 0.75), color_dodge(0.3, 0.5));
        assert_eq!(blend1(Mode::Reflect, 0.5, 0.5), 0.5);
        assert_eq!(blend1(Mode::Glow, 0.5, 0.5), 0.5);
        assert_eq!(blend1(Mode::Freeze, 0.5, 0.5), 0.5);
        assert_eq!(blend1(Mode::GammaLight, 0.25, 0.5), 0.5);
        assert_eq!(blend1(Mode::ArcTangent, 0.5, 0.5), 0.5);
        assert_eq!(blend1(Mode::Negation, 0.75, 0.75), 0.5);
    }

    #[test]
    fn every_mode_stays_in_range_on_unit_inputs() {
        let all = [
            "normal",
            "multiply",
            "screen",
            "overlay",
            "darken",
            "lighten",
            "add",
            "subtract",
            "linear_burn",
            "diff",
            "exclusion",
            "dodge",
            "burn",
            "soft_light",
            "soft_light_svg",
            "soft_light_pegtop_delphi",
            "hard_light",
            "vivid_light",
            "linear_light",
            "pin_light",
            "hard_mix",
            "divide",
            "grain_extract",
            "grain_merge",
            "allanon",
            "parallel",
            "geometric_mean",
            "negation",
            "additive_subtractive",
            "gamma_dark",
            "gamma_light",
            "arc_tangent",
            "reflect",
            "glow",
            "freeze",
            "heat",
            "darker color",
            "lighter color",
            "hue",
            "saturation",
            "color",
            "luminize",
            "hue_hsl",
            "saturation_hsl",
            "color_hsl",
            "lightness",
        ];
        let steps: Vec<f32> = (0..=8).map(|i| i as f32 / 8.0).collect();
        for id in all {
            let m = Mode::from_krita(id).unwrap();
            for &b in &steps {
                for &s in &steps {
                    let cb = [b, 1.0 - b, b * 0.5];
                    let cs = [s, s * 0.3, 1.0 - s];
                    for v in blend3(m, cb, cs) {
                        assert!(v.is_finite() && (-1e-5..=1.0 + 1e-5).contains(&v), "{id}");
                    }
                }
            }
        }
    }

    #[test]
    fn non_separable_keep_luminosity() {
        let b = [0.25, 0.5, 1.0];
        let s = [0.5, 0.5, 0.0];
        assert!((lum(blend3(Mode::Color, b, s)) - lum(b)).abs() < 1e-4);
        assert!((lum(blend3(Mode::Luminosity, b, s)) - lum(s)).abs() < 1e-4);
        assert!(close(from_hsl(to_hsl([0.2, 0.6, 0.4])), [0.2, 0.6, 0.4]));
        assert_eq!(blend3(Mode::DarkerColor, b, s), s);
        assert_eq!(blend3(Mode::LighterColor, b, s), b);
        // HSL lightness from the source, hue and saturation from the backdrop.
        let r = to_hsl(blend3(Mode::HslLightness, b, s));
        let (hb, hs) = (to_hsl(b), to_hsl(s));
        assert!((r[2] - hs[2]).abs() < 1e-5 && (r[0] - hb[0]).abs() < 1e-5);
    }

    #[test]
    fn photoshop_and_openraster_names() {
        assert_eq!(Mode::from_photoshop(b"lbrn"), Some(Mode::LinearBurn));
        assert_eq!(Mode::from_photoshop(b"diss"), None);
        assert_eq!(
            Mode::from_openraster("svg:color-dodge"),
            Some(Mode::ColorDodge)
        );
        assert_eq!(Mode::from_openraster("svg:plus"), None);
        assert_eq!(
            PorterDuff::from_openraster("svg:dst-out"),
            Some(PorterDuff::DestinationOut)
        );
        assert!(Mode::Hue.non_separable() && !Mode::Multiply.non_separable());
    }

    #[test]
    fn compositing_operators() {
        let cb = [0.2, 0.4, 0.6];
        let cs = [1.0, 0.0, 0.5];
        // Source over an opaque backdrop with a half source: the mean.
        let (c, a) = composite(Mode::Normal, PorterDuff::SourceOver, cb, 1.0, cs, 0.5);
        assert!(close(c, [0.6, 0.2, 0.55]) && a == 1.0);
        // Over nothing: the source as it is.
        let (c, a) = composite(Mode::Multiply, PorterDuff::SourceOver, cb, 0.0, cs, 0.5);
        assert!(close(c, cs) && a == 0.5);
        // Multiply over an opaque backdrop, opaque source: the product.
        let (c, _) = composite(Mode::Multiply, PorterDuff::SourceOver, cb, 1.0, cs, 1.0);
        assert!(close(c, [0.2, 0.0, 0.3]));
        // Destination out removes the backdrop by the source alpha.
        let (c, a) = composite(Mode::Normal, PorterDuff::DestinationOut, cb, 1.0, cs, 0.25);
        assert!(close(c, cb) && (a - 0.75).abs() < 1e-6);
        // Source atop keeps the backdrop alpha.
        let (_, a) = composite(Mode::Normal, PorterDuff::SourceAtop, cb, 0.5, cs, 1.0);
        assert_eq!(a, 0.5);
        let (_, a) = composite(Mode::Normal, PorterDuff::Plus, cb, 0.75, cs, 0.75);
        assert_eq!(a, 1.0);
        let (_, a) = composite(Mode::Normal, PorterDuff::Xor, cb, 1.0, cs, 1.0);
        assert_eq!(a, 0.0);
    }

    #[test]
    fn float_variants() {
        assert_eq!(float::add(0.75, 0.5), 1.25);
        assert_eq!(float::subtract(0.25, 0.5), -0.25);
        assert_eq!(float::divide(0.0, 0.0), 0.0);
        assert!((float::exclusion(0.25, 0.5) - blend1(Mode::Exclusion, 0.25, 0.5)).abs() < 1e-6);
        assert!((float::overlay(0.3, 0.8) - blend1(Mode::Overlay, 0.3, 0.8)).abs() < 1e-6);
        assert!(
            (float::soft_light_pegtop(0.3, 0.8) - blend1(Mode::SoftLightPegtop, 0.3, 0.8)).abs()
                < 1e-6
        );
    }
}
