//! Highlights and corrections: highlight recovery, hot pixels, raw
//! chromatic aberration, flat-field and dark-frame calibration, and defringe.
//!
//! # Order
//!
//! On the CFA mosaic, inside the raw front end ([`crate::ops::raw`]), before demosaic:
//!
//! 1. **Prepare**: DNG list 1, linearisation, black and white, then the **dark frame**
//!    (subtracted) and the **flat field** (divided), then DNG list 2 and white balance
//!    ([`calib`]).
//! 2. **Hot and dead pixels** ([`HotPixels`]): a sample far above (or below) every
//!    same-colour neighbour becomes their median.
//! 3. **Highlights** ([`highlights`]): clipped samples rebuilt from the unclipped channels
//!    and the colour around the clipped area (opposed, the default), plus a brightness dome
//!    for large blown areas (rebuild), or clipped to neutral (clip).
//! 4. **Raw chromatic aberration** ([`ca`], Bayer): red and blue resampled on their own
//!    lattices along a radial model fitted to per-tile shifts against green.
//!
//! After demosaic, among the graph's ops:
//!
//! 5. **Defringe** ([`defringe`]): purple and green fringes beside high-contrast edges lose
//!    their chroma.
//!
//! In the geometry warp ([`crate::ops::geometry`]):
//!
//! 6. **RGB chromatic aberration** for sources that are not raw: the fitted (or manual)
//!    red and blue radial scales join the lens card's red and blue fringe scales, so the
//!    picture is still sampled once. For raws the CFA step (4) does this job; the lens
//!    card's own fringe sliders stay as they are and apply in the warp for every
//!    source.

pub mod ca;
pub mod calib;
pub mod defringe;
pub mod highlights;

#[cfg(test)]
mod tests;

use crate::model::params::ModuleParams;
use crate::model::params::{Highlights, RawFrontEnd};
use crate::model::{Edit, Fields, ModuleKind, Params, Value};

/// The default hot-pixel threshold: a sample must be twice its brightest same-colour
/// neighbour (or half its darkest) to count.
pub const HOT_DEFAULT: f32 = 1.0;

fn clamped(f: &Fields, n: &str, d: f32, lo: f32, hi: f32) -> f32 {
    let v = f.f32_or(n, d);
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

/// Hot and dead pixel params (fields of [`ModuleKind::HOT_PIXELS`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HotPixels {
    /// How far above the brightest same-colour neighbour a sample must be, as a share of
    /// it (0.25 … 4); dead pixels: below the darkest over `1 + threshold`.
    pub threshold: f32,
}

impl Default for HotPixels {
    fn default() -> Self {
        Self {
            threshold: HOT_DEFAULT,
        }
    }
}

impl HotPixels {
    pub fn from_fields(f: &Fields) -> Self {
        Self {
            threshold: clamped(f, "threshold", HOT_DEFAULT, 0.25, 4.0),
        }
    }

    pub fn default_fields() -> Fields {
        Fields::new().with("threshold", Value::Float(HOT_DEFAULT))
    }
}

/// Chromatic aberration params (fields of [`ModuleKind::CHROMATIC_ABERRATION`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromaticAberration {
    /// Estimate the shifts from the picture.
    pub auto: bool,
    /// Manual red and blue radial scales, percent of the radius (−0.5 … 0.5): positive
    /// shrinks the plane towards the centre.
    pub red: f32,
    pub blue: f32,
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        Self {
            auto: true,
            red: 0.0,
            blue: 0.0,
        }
    }
}

impl ChromaticAberration {
    pub fn from_fields(f: &Fields) -> Self {
        Self {
            auto: f.bool_or("auto", true),
            red: clamped(f, "red", 0.0, -0.5, 0.5),
            blue: clamped(f, "blue", 0.0, -0.5, 0.5),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("auto", Value::Bool(d.auto))
            .with("red", Value::Float(d.red))
            .with("blue", Value::Float(d.blue))
    }
}

/// Flat-field and dark-frame params (fields of [`ModuleKind::FLAT_FIELD`], version 1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlatField {
    /// A flat frame (a raw of an even, unfocused light), or empty.
    pub flat: String,
    /// A dark frame (a raw with the cap on), or empty.
    pub dark: String,
    /// Pick both from the calibration folder by camera, lens, aperture and ISO
    /// ([`calib::best_match`]) instead of the paths.
    pub auto: bool,
    /// Use only the flat's smooth falloff (vignetting and lens cast), not its dust.
    pub blur: bool,
}

impl FlatField {
    pub fn from_fields(f: &Fields) -> Self {
        Self {
            flat: f.text("flat").unwrap_or_default().to_owned(),
            dark: f.text("dark").unwrap_or_default().to_owned(),
            auto: f.bool_or("auto", false),
            blur: f.bool_or("blur", false),
        }
    }

    pub fn default_fields() -> Fields {
        Fields::new()
            .with("flat", Value::Text(String::new()))
            .with("dark", Value::Text(String::new()))
            .with("auto", Value::Bool(false))
            .with("blur", Value::Bool(false))
    }
}

/// How clipped highlights are treated (the front end's form of [`Highlights`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HighlightMode {
    /// Left as the sensor gave them (values past white stay; checks only).
    Off,
    Clip,
    Opposed,
    Rebuild,
}

/// Raw chromatic aberration: off, estimated, or manual scales (percent of the radius).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaMode {
    Off,
    Auto,
    Manual { red: f32, blue: f32 },
}

/// The raw-stage corrections of one front-end run (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corrections {
    /// Hot pixel threshold; `None`: off.
    pub hot: Option<f32>,
    pub highlights: HighlightMode,
    /// Clip level as a share of the sensor's (0.5 … 1).
    pub highlight_threshold: f32,
    pub ca: CaMode,
    /// Apply the calibration frames set on the sensor ([`crate::ops::raw::GpuSensor`]).
    pub calibrate: bool,
}

impl Default for Corrections {
    /// None: the front end's output unchanged.
    fn default() -> Self {
        Self {
            hot: None,
            highlights: HighlightMode::Off,
            highlight_threshold: 1.0,
            ca: CaMode::Off,
            calibrate: false,
        }
    }
}

impl Corrections {
    /// Whether any correction runs.
    pub fn any(&self) -> bool {
        self.hot.is_some()
            || self.highlights != HighlightMode::Off
            || self.ca != CaMode::Off
            || self.calibrate
    }

    /// The corrections an edit asks for: the front end's highlights, and the hot pixel,
    /// chromatic aberration and flat-field modules when enabled. `calibrate` is set when
    /// frames are attached ([`calib::Calibration`]) and the module is on.
    pub fn of_edit(edit: &Edit) -> Corrections {
        let on = |k: ModuleKind| edit.modules.iter().find(|m| m.kind == k && m.enabled);
        let front = on(ModuleKind::RAW_FRONT_END).map(|m| match &m.params {
            Params::RawFrontEnd(p) => p.clone(),
            other => RawFrontEnd::from_fields(&other.to_fields()),
        });
        let mut c = Corrections::default();
        if let Some(p) = front {
            c.highlights = match p.highlights {
                Highlights::Opposed => HighlightMode::Opposed,
                Highlights::Rebuild => HighlightMode::Rebuild,
                Highlights::Clip => HighlightMode::Clip,
            };
            c.highlight_threshold = if p.highlight_threshold.is_finite() {
                p.highlight_threshold.clamp(0.5, 1.0)
            } else {
                1.0
            };
        }
        if let Some(m) = on(ModuleKind::HOT_PIXELS) {
            c.hot = Some(HotPixels::from_fields(&m.params.to_fields()).threshold);
        }
        if let Some(m) = on(ModuleKind::CHROMATIC_ABERRATION) {
            let p = ChromaticAberration::from_fields(&m.params.to_fields());
            c.ca = if p.auto {
                CaMode::Auto
            } else if p.red == 0.0 && p.blue == 0.0 {
                CaMode::Off
            } else {
                CaMode::Manual {
                    red: p.red,
                    blue: p.blue,
                }
            };
        }
        c.calibrate = on(ModuleKind::FLAT_FIELD).is_some();
        c
    }
}

/// The flat-field module's params when it is on.
pub fn flat_field(edit: &Edit) -> Option<FlatField> {
    edit.modules
        .iter()
        .find(|m| m.kind == ModuleKind::FLAT_FIELD && m.enabled)
        .map(|m| FlatField::from_fields(&m.params.to_fields()))
}

/// The chromatic aberration module's params when it is on (for the warp's RGB form).
pub fn chromatic_aberration(edit: &Edit) -> Option<ChromaticAberration> {
    edit.modules
        .iter()
        .find(|m| m.kind == ModuleKind::CHROMATIC_ABERRATION && m.enabled)
        .map(|m| ChromaticAberration::from_fields(&m.params.to_fields()))
}
