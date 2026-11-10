//! No dead controls: a control that cannot change the picture in the current state
//! either switches on what it needs as it moves, in the same history step, or is shown
//! dimmed with why (its tooltip and screen-reader description).
//!
//! - **Switched on when moved** (the intent is plain): the controls of the basic cards
//!   (Light, white balance, Tone zones, local contrast, the curve, Colour, Colour grading, the
//!   colour mixer: no switch; an older edit may have their module off), chromatic aberration's
//!   Red and Blue (the mode becomes Manual), the hot pixel threshold (hot pixels on),
//!   defringe's hues, range and edge (an amount), the dual demosaic threshold (Dual), denoise
//!   detail (some luminance), sharpening's and capture sharpening's refinements (an amount),
//!   the graduated filter's angle and softness (an amount) and tint hue (a tint), the
//!   vignette's shape controls (an amount), glow's and grain's refinements (an amount), surface
//!   blur's radius and edges (an amount).
//! - **Dimmed with why** (the intent is open): everything in a card that is turned off;
//!   point colour before a pick; the LUT's settings before a file; tone smoothing before a
//!   tone change; diffuse's controls before a speed; using the flat before calibration is
//!   on; the Optics card's manual lens rows while Lens corrections is off (turning it on
//!   also brings back the file's lens profile, which may have been turned off on purpose).

use specular_develop::model::params::Demosaic;
use specular_develop::model::{Fields, ModuleKind, Value};
use specular_i18n::t;
use specular_widgets::develop_panel::Card;

use super::controls::{Session, id, module_title};
use super::diffuse::id as did;

/// What moving a control sets first when it would do nothing.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Fix {
    /// Turn the control's module on.
    Enable,
    /// Set a field (when it differs).
    Set(&'static str, Value),
    /// Give a float field this value while it and `unless` are zero.
    Raise {
        field: &'static str,
        to: f32,
        unless: Option<&'static str>,
    },
}

const fn raise(field: &'static str, to: f32) -> Fix {
    Fix::Raise {
        field,
        to,
        unless: None,
    }
}

/// The fixes of a control (empty: it acts by itself).
pub(super) fn fixes(control: u32) -> Vec<Fix> {
    match control {
        id::CA_RED | id::CA_BLUE => vec![Fix::Set("auto", Value::Bool(false)), Fix::Enable],
        id::HOT_THRESHOLD => vec![Fix::Enable],
        // No switch on the basic cards (Light, white balance, Tone zones, local contrast,
        // the curve, Colour, Colour grading, the colour mixer): moving one of theirs turns
        // its module back on.
        id::TEMPERATURE
        | id::TINT
        | id::TEXTURE
        | id::CLARITY
        | id::CURVE
        | id::LEVELS_BLACK
        | id::LEVELS_MID
        | id::LEVELS_WHITE
        | id::MIXER_HUE..=127
        | id::VIEW_METHOD
        | id::VIBRANCE
        | id::SATURATION
        | id::CHROMA
        | id::BRILLIANCE
        | id::WHEELS
        | id::AUTO_EXPOSURE
        | id::EXPOSURE
        | id::CONTRAST
        | id::HIGHLIGHTS
        | id::SHADOWS
        | id::WHITES
        | id::BLACKS
        | id::SMOOTHING
        | id::ZONE..=id::ZONE_LAST => vec![Fix::Enable],
        id::DEFRINGE_PURPLE | id::DEFRINGE_GREEN | id::DEFRINGE_RANGE | id::DEFRINGE_EDGE => {
            vec![raise("amount", 0.5)]
        }
        id::DUAL_THRESHOLD => vec![Fix::Set("demosaic", Demosaic::Dual.into())],
        id::NOISE_DETAIL => vec![raise("luminance", 0.5)],
        id::SHARPEN_RADIUS | id::SHARPEN_DETAIL | id::SHARPEN_MASKING => {
            vec![raise("amount", 0.5)]
        }
        id::CAPTURE_RADIUS => vec![raise("amount", 0.5)],
        id::ND_ANGLE | id::ND_SOFTNESS => vec![Fix::Raise {
            field: "amount",
            to: -1.0,
            unless: Some("tint_amount"),
        }],
        id::ND_TINT_HUE => vec![raise("tint_amount", 0.3)],
        id::VG_MIDPOINT
        | id::VG_ROUNDNESS
        | id::VG_SHAPE
        | id::VG_FEATHER
        | id::VG_PROTECT
        | id::VG_CX
        | id::VG_CY => vec![raise("amount", -1.0)],
        id::GLOW_RADIUS | id::GLOW_THRESHOLD | id::GLOW_MODE => vec![raise("amount", 0.3)],
        id::GRAIN_SIZE | id::GRAIN_BIAS | id::GRAIN_COLOUR => vec![raise("amount", 0.25)],
        did::SURFACE_RADIUS | did::SURFACE_EDGES => vec![raise("amount", 0.5)],
        _ => Vec::new(),
    }
}

/// Applies a control's fixes to its module's fields `f`; whether the module must be turned
/// on (it is off).
pub(super) fn apply(control: u32, f: &mut Fields, on: bool) -> bool {
    let mut enable = false;
    for fix in fixes(control) {
        match fix {
            Fix::Enable => enable = !on,
            Fix::Set(name, v) => {
                if f.get(name) != Some(&v) {
                    f.set(name, v);
                }
            }
            Fix::Raise { field, to, unless } => {
                let zero = |n: &str| f.f32_or(n, 0.0).abs() < 1e-6;
                if zero(field) && unless.is_none_or(zero) {
                    f.set(field, Value::Float(to));
                }
            }
        }
    }
    enable
}

impl Session {
    /// Why each control of `cards` can do nothing now (for the panel's dimming).
    pub(super) fn dims(&self, cards: &[Card]) -> Vec<(u32, String)> {
        let mut out = Vec::new();
        for c in cards {
            let off = c.enabled == Some(false);
            for ctl in &c.controls {
                let why = if off {
                    Some(t!(
                        "develop-dim-turn-on",
                        module = module_title(ModuleKind(c.id as u16))
                    ))
                } else {
                    self.dim_reason(ctl.id)
                };
                if let Some(w) = why {
                    out.push((ctl.id, w));
                }
            }
        }
        out
    }

    /// Why one control can do nothing now (its card on).
    fn dim_reason(&self, control: u32) -> Option<String> {
        let f = |k: ModuleKind| self.fields_of(k);
        match control {
            id::LENS_DISTORTION | id::LENS_VIGNETTE | id::LENS_TCA_RED | id::LENS_TCA_BLUE
                if !self.module_on(ModuleKind::LENS) =>
            {
                Some(t!(
                    "develop-dim-turn-on",
                    module = module_title(ModuleKind::LENS)
                ))
            }
            id::POINT_HUE..=id::POINT_LIGHTNESS | id::POINT_RANGE..=id::POINT_LIGHTNESS_RANGE
                if !f(ModuleKind::POINT_COLOUR).bool_or("picked", false) =>
            {
                Some(t!("develop-dim-pick"))
            }
            // The ranges pick what a shift changes.
            id::POINT_RANGE..=id::POINT_LIGHTNESS_RANGE
                if !point_shifted(&f(ModuleKind::POINT_COLOUR)) =>
            {
                Some(t!("develop-dim-point-shift"))
            }
            id::LUT_SPACE | id::LUT_INTERPOLATION | id::LUT_STRENGTH
                if f(ModuleKind::LUT_3D).text("path").unwrap_or("").is_empty() =>
            {
                Some(t!("develop-dim-lut"))
            }
            id::SMOOTHING if !tone_changed(&f(ModuleKind::TONE_EQUALIZER)) => {
                Some(t!("develop-dim-tone"))
            }
            c if super::diffuse::is_diffuse(c)
                && super::diffuse::module_of(c) == Some(ModuleKind::DIFFUSE)
                && !matches!(c, did::PRESET | did::NOTE)
                && !(did::SPEED..did::SPEED + 4).contains(&c)
                && !diffusing(&f(ModuleKind::DIFFUSE)) =>
            {
                Some(t!("develop-dim-diffuse"))
            }
            // An order's anisotropy shapes its speed.
            c if (did::ANISOTROPY..did::ANISOTROPY + 4).contains(&c)
                && speed(&f(ModuleKind::DIFFUSE), (c - did::ANISOTROPY) as usize) == 0.0 =>
            {
                let speed = super::diffuse::field_label(
                    ModuleKind::DIFFUSE,
                    specular_develop::ops::detail::diffuse::SPEED_FIELDS
                        [(c - did::ANISOTROPY) as usize],
                )
                .unwrap_or_default();
                Some(t!("develop-dim-speed", speed = speed))
            }
            // The edge threshold holds back the sharpening (negative) speeds.
            did::THRESHOLD if !(0..4).any(|k| speed(&f(ModuleKind::DIFFUSE), k) < 0.0) => {
                Some(t!("develop-dim-sharpen"))
            }
            id::FLAT_USE
                if !self
                    .edit
                    .module(ModuleKind::FLAT_FIELD, 0)
                    .is_some_and(|m| m.enabled) =>
            {
                Some(t!("develop-dim-calibration"))
            }
            _ => None,
        }
    }

    /// Puts the dimmed controls on the panel.
    pub(super) fn sync_dims(&mut self) {
        self.panel.dimmed = self.dims(&self.panel.cards);
    }
}

/// Whether the tone equalizer changes anything (highlights, shadows or a zone).
fn tone_changed(f: &Fields) -> bool {
    let nz = |v: f32| v.abs() > 1e-6;
    nz(f.f32_or("highlights_ev", 0.0))
        || nz(f.f32_or("shadows_ev", 0.0))
        || (id::ZONE..=id::ZONE_LAST).any(|c| nz(super::zones::read(f, c)))
}

/// Diffuse's speed of order `k` (0 … 3).
fn speed(f: &Fields, k: usize) -> f32 {
    f.f32_or(specular_develop::ops::detail::diffuse::SPEED_FIELDS[k], 0.0)
}

/// Whether point colour shifts the picked colour at all.
fn point_shifted(f: &Fields) -> bool {
    ["shift_hue", "shift_saturation", "shift_lightness"]
        .iter()
        .any(|n| f.f32_or(n, 0.0).abs() > 1e-6)
}

/// Whether diffuse does anything (a speed set).
fn diffusing(f: &Fields) -> bool {
    specular_develop::ops::detail::diffuse::SPEED_FIELDS
        .iter()
        .any(|n| f.f32_or(n, 0.0).abs() > 1e-6)
}
