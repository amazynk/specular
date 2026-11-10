//! Typed params of the alpha modules. Each is a plain struct with named fields and a version;
//! [`ModuleParams`] maps it to and from [`Fields`]. Missing fields take their defaults, so an
//! older or partial field set (a preset) always reads.

use super::fields::{Fields, Value, choice};
use super::kind::ModuleKind;

/// A module's typed params.
pub trait ModuleParams: Sized + Default + Clone + PartialEq {
    const KIND: ModuleKind;
    /// The param version this struct is shaped for (the registry's `current`).
    const VERSION: u16;
    fn to_fields(&self) -> Fields;
    /// Reads fields of any live version; absent or ill-typed fields take defaults.
    fn from_fields(f: &Fields) -> Self;
}

/// The default fields of `P` (for the registry).
pub fn defaults<P: ModuleParams>() -> Fields {
    P::default().to_fields()
}

/// One instance's params: typed for the modules this build knows, fields otherwise.
#[derive(Clone, Debug, PartialEq)]
pub enum Params {
    RawFrontEnd(RawFrontEnd),
    Exposure(Exposure),
    ColourCalibration(ColourCalibration),
    ViewTransform(ViewTransform),
    CropOrientation(CropOrientation),
    /// A kind without typed params yet, or unknown to this build: kept as is.
    Other(ModuleKind, Fields),
}

impl Params {
    pub fn kind(&self) -> ModuleKind {
        match self {
            Params::RawFrontEnd(_) => RawFrontEnd::KIND,
            Params::Exposure(_) => Exposure::KIND,
            Params::ColourCalibration(_) => ColourCalibration::KIND,
            Params::ViewTransform(_) => ViewTransform::KIND,
            Params::CropOrientation(_) => CropOrientation::KIND,
            Params::Other(k, _) => *k,
        }
    }

    pub fn to_fields(&self) -> Fields {
        match self {
            Params::RawFrontEnd(p) => p.to_fields(),
            Params::Exposure(p) => p.to_fields(),
            Params::ColourCalibration(p) => p.to_fields(),
            Params::ViewTransform(p) => p.to_fields(),
            Params::CropOrientation(p) => p.to_fields(),
            Params::Other(_, f) => f.clone(),
        }
    }

    /// Types `fields` of `kind`. Call after migration, so the version is a live one.
    pub fn from_fields(kind: ModuleKind, f: Fields) -> Self {
        match kind {
            ModuleKind::RAW_FRONT_END => Params::RawFrontEnd(RawFrontEnd::from_fields(&f)),
            ModuleKind::EXPOSURE => Params::Exposure(Exposure::from_fields(&f)),
            ModuleKind::COLOUR_CALIBRATION => {
                Params::ColourCalibration(ColourCalibration::from_fields(&f))
            }
            ModuleKind::VIEW_TRANSFORM => Params::ViewTransform(ViewTransform::from_fields(&f)),
            ModuleKind::CROP_ORIENTATION => {
                Params::CropOrientation(CropOrientation::from_fields(&f))
            }
            k => Params::Other(k, f),
        }
    }

    /// Copies named fields over these params (a preset or a pasted subset).
    pub fn overlay(&mut self, f: &Fields) {
        let mut all = self.to_fields();
        all.overlay(f);
        self.set_fields(all);
    }

    /// Replaces the fields. Params kept as fields (a version this build does not render)
    /// stay fields, so nothing of them is lost.
    pub fn set_fields(&mut self, f: Fields) {
        *self = match self {
            Params::Other(k, _) => Params::Other(*k, f),
            _ => Params::from_fields(self.kind(), f),
        };
    }
}

fn f(v: f32) -> Value {
    Value::Float(v)
}

choice! {
    /// Bayer demosaic method (X-Trans always uses its own).
    Demosaic {
        /// Ratio-corrected directional (the default).
        Directional = "directional",
        /// Directional in detail, a smooth method in flat areas.
        Dual = "dual",
        /// Malvar–He–Cutler.
        Fast = "fast",
    }
}

choice! {
    /// What happens to clipped highlights.
    Highlights {
        Opposed = "opposed",
        Rebuild = "rebuild",
        Clip = "clip",
    }
}

choice! {
    /// Camera to working space.
    InputProfile {
        /// The DNG profile (HueSatMap/LookTable) when present, else the matrix.
        Auto = "auto",
        Matrix = "matrix",
        /// A user camera profile (`.dcp`) at `profile_path`.
        File = "file",
    }
}

/// Raw front end: black/white/linearise, DNG opcodes, highlights, demosaic, input profile.
#[derive(Clone, Debug, PartialEq)]
pub struct RawFrontEnd {
    pub demosaic: Demosaic,
    /// Dual demosaic: contrast below which the smooth method is used (0..1).
    pub dual_threshold: f32,
    pub highlights: Highlights,
    /// Clip level as a fraction of the white level.
    pub highlight_threshold: f32,
    pub dng_opcodes: bool,
    pub input_profile: InputProfile,
    pub profile_path: String,
    /// BLAKE3 (hex) of the profile file when it was chosen; empty for none. Stored only when
    /// set, so edits without a profile file keep their hashes.
    pub profile_hash: String,
}

impl Default for RawFrontEnd {
    fn default() -> Self {
        Self {
            demosaic: Demosaic::Directional,
            dual_threshold: 0.2,
            highlights: Highlights::Opposed,
            highlight_threshold: 1.0,
            dng_opcodes: true,
            input_profile: InputProfile::Auto,
            profile_path: String::new(),
            profile_hash: String::new(),
        }
    }
}

impl ModuleParams for RawFrontEnd {
    const KIND: ModuleKind = ModuleKind::RAW_FRONT_END;
    const VERSION: u16 = 1;

    fn to_fields(&self) -> Fields {
        let f = Fields::new()
            .with("demosaic", self.demosaic.into())
            .with("dual_threshold", f(self.dual_threshold))
            .with("highlights", self.highlights.into())
            .with("highlight_threshold", f(self.highlight_threshold))
            .with("dng_opcodes", Value::Bool(self.dng_opcodes))
            .with("input_profile", self.input_profile.into())
            .with("profile_path", Value::Text(self.profile_path.clone()));
        if self.profile_hash.is_empty() {
            f
        } else {
            f.with("profile_hash", Value::Text(self.profile_hash.clone()))
        }
    }

    fn from_fields(x: &Fields) -> Self {
        let d = Self::default();
        Self {
            demosaic: x.choice_or("demosaic", d.demosaic),
            dual_threshold: x.f32_or("dual_threshold", d.dual_threshold),
            highlights: x.choice_or("highlights", d.highlights),
            highlight_threshold: x.f32_or("highlight_threshold", d.highlight_threshold),
            dng_opcodes: x.bool_or("dng_opcodes", d.dng_opcodes),
            input_profile: x.choice_or("input_profile", d.input_profile),
            profile_path: x.text("profile_path").unwrap_or_default().to_owned(),
            profile_hash: x.text("profile_hash").unwrap_or_default().to_owned(),
        }
    }
}

/// Exposure: linear gain and black offset, or auto by percentile.
#[derive(Clone, Debug, PartialEq)]
pub struct Exposure {
    pub exposure_ev: f32,
    /// Subtracted before the gain, in scene-linear units.
    pub black_offset: f32,
    pub auto: bool,
    /// Auto: the scene percentile (0..100) that lands on `auto_target_ev`.
    pub auto_percentile: f32,
    pub auto_target_ev: f32,
}

impl Default for Exposure {
    fn default() -> Self {
        Self {
            exposure_ev: 0.0,
            black_offset: 0.0,
            auto: false,
            auto_percentile: 50.0,
            auto_target_ev: -2.5,
        }
    }
}

impl ModuleParams for Exposure {
    const KIND: ModuleKind = ModuleKind::EXPOSURE;
    const VERSION: u16 = 1;

    fn to_fields(&self) -> Fields {
        Fields::new()
            .with("exposure_ev", f(self.exposure_ev))
            .with("black_offset", f(self.black_offset))
            .with("auto", Value::Bool(self.auto))
            .with("auto_percentile", f(self.auto_percentile))
            .with("auto_target_ev", f(self.auto_target_ev))
    }

    fn from_fields(x: &Fields) -> Self {
        let d = Self::default();
        Self {
            exposure_ev: x.f32_or("exposure_ev", d.exposure_ev),
            black_offset: x.f32_or("black_offset", d.black_offset),
            auto: x.bool_or("auto", d.auto),
            auto_percentile: x.f32_or("auto_percentile", d.auto_percentile),
            auto_target_ev: x.f32_or("auto_target_ev", d.auto_target_ev),
        }
    }
}

choice! {
    /// Chromatic adaptation transform.
    Adaptation {
        Cat16 = "cat16",
        Bradford = "bradford",
        /// No adaptation (the channel mixer only).
        None = "none",
    }
}

choice! {
    /// Where the white point comes from.
    WhiteSource {
        AsShot = "as_shot",
        Auto = "auto",
        /// Set by the user (sliders or the picker).
        Custom = "custom",
    }
}

/// Colour calibration: white balance as a CAT to D65, channel mixer, gamut compression.
#[derive(Clone, Debug, PartialEq)]
pub struct ColourCalibration {
    pub adaptation: Adaptation,
    pub white_source: WhiteSource,
    /// Scene illuminant chromaticity (CIE 1931 xy).
    pub white_x: f32,
    pub white_y: f32,
    /// The illuminant the image arrives balanced for (CIE 1931 xy): the raw front end's
    /// as-shot white, or the file's own white for other images. The CAT maps `white` to
    /// it, so white = as shot is no change. [`D50_XY`] until the camera's white is known.
    pub as_shot_x: f32,
    pub as_shot_y: f32,
    /// Row-major 3×3 in the working space; identity by default.
    pub mixer: [f32; 9],
    /// ACES-style gamut compression strength (0 = off, 1 = full).
    pub gamut_compression: f32,
    /// Distance from the achromatic axis where compression starts (0..1).
    pub gamut_threshold: f32,
}

/// D50, the as-shot placeholder before the camera's white is known.
pub const D50_XY: [f32; 2] = [0.3457, 0.3585];

impl Default for ColourCalibration {
    fn default() -> Self {
        Self {
            adaptation: Adaptation::Cat16,
            white_source: WhiteSource::AsShot,
            white_x: D50_XY[0],
            white_y: D50_XY[1],
            as_shot_x: D50_XY[0],
            as_shot_y: D50_XY[1],
            mixer: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            gamut_compression: 1.0,
            gamut_threshold: 0.8,
        }
    }
}

impl ModuleParams for ColourCalibration {
    const KIND: ModuleKind = ModuleKind::COLOUR_CALIBRATION;
    const VERSION: u16 = 1;

    fn to_fields(&self) -> Fields {
        Fields::new()
            .with("adaptation", self.adaptation.into())
            .with("white_source", self.white_source.into())
            .with("white_x", f(self.white_x))
            .with("white_y", f(self.white_y))
            .with("as_shot_x", f(self.as_shot_x))
            .with("as_shot_y", f(self.as_shot_y))
            .with("mixer", Value::Floats(self.mixer.to_vec()))
            .with("gamut_compression", f(self.gamut_compression))
            .with("gamut_threshold", f(self.gamut_threshold))
    }

    fn from_fields(x: &Fields) -> Self {
        let d = Self::default();
        Self {
            adaptation: x.choice_or("adaptation", d.adaptation),
            white_source: x.choice_or("white_source", d.white_source),
            white_x: x.f32_or("white_x", d.white_x),
            white_y: x.f32_or("white_y", d.white_y),
            as_shot_x: x.f32_or("as_shot_x", d.as_shot_x),
            as_shot_y: x.f32_or("as_shot_y", d.as_shot_y),
            mixer: x.floats_or("mixer", d.mixer),
            gamut_compression: x.f32_or("gamut_compression", d.gamut_compression),
            gamut_threshold: x.f32_or("gamut_threshold", d.gamut_threshold),
        }
    }
}

choice! {
    /// View transform family.
    ViewMethod {
        Sigmoid = "sigmoid",
        Filmic = "filmic",
        Agx = "agx",
    }
}

/// View transform: scene values to display values, as late as possible.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewTransform {
    pub method: ViewMethod,
    pub contrast: f32,
    /// Shifts the curve's midpoint towards shadows (<0) or highlights (>0).
    pub skew: f32,
    /// Display white in multiples of SDR white (1 for SDR; the peak for HDR).
    pub white_target: f32,
    pub black_target: f32,
    /// 0 = per-channel, 1 = full hue preservation.
    pub hue_preservation: f32,
    /// Primaries inset before the curve and outset after it (0..1), for smooth highlights.
    pub inset: f32,
    pub outset: f32,
    /// HDR headroom limit in EV above SDR white.
    pub hdr_limit_ev: f32,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            method: ViewMethod::Sigmoid,
            contrast: 1.5,
            skew: 0.0,
            white_target: 1.0,
            black_target: 0.0,
            hue_preservation: 1.0,
            inset: 0.1,
            outset: 0.1,
            hdr_limit_ev: 2.0,
        }
    }
}

impl ModuleParams for ViewTransform {
    const KIND: ModuleKind = ModuleKind::VIEW_TRANSFORM;
    const VERSION: u16 = 1;

    fn to_fields(&self) -> Fields {
        Fields::new()
            .with("method", self.method.into())
            .with("contrast", f(self.contrast))
            .with("skew", f(self.skew))
            .with("white_target", f(self.white_target))
            .with("black_target", f(self.black_target))
            .with("hue_preservation", f(self.hue_preservation))
            .with("inset", f(self.inset))
            .with("outset", f(self.outset))
            .with("hdr_limit_ev", f(self.hdr_limit_ev))
    }

    fn from_fields(x: &Fields) -> Self {
        let d = Self::default();
        Self {
            method: x.choice_or("method", d.method),
            contrast: x.f32_or("contrast", d.contrast),
            skew: x.f32_or("skew", d.skew),
            white_target: x.f32_or("white_target", d.white_target),
            black_target: x.f32_or("black_target", d.black_target),
            hue_preservation: x.f32_or("hue_preservation", d.hue_preservation),
            inset: x.f32_or("inset", d.inset),
            outset: x.f32_or("outset", d.outset),
            hdr_limit_ev: x.f32_or("hdr_limit_ev", d.hdr_limit_ev),
        }
    }
}

/// Crop, straighten and orientation, in source image coordinates (0..1 of the oriented
/// sensor image), part of the fused warp.
#[derive(Clone, Debug, PartialEq)]
pub struct CropOrientation {
    /// Quarter turns clockwise after the file's own orientation (0..3).
    pub quarter_turns: i64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    /// Straighten angle in degrees, positive clockwise.
    pub angle_deg: f32,
    /// Left, top, right, bottom as fractions of the rotated image.
    pub crop: [f32; 4],
    /// Aspect constraint: `free`, `original` or `W:H`.
    pub aspect: String,
}

impl Default for CropOrientation {
    fn default() -> Self {
        Self {
            quarter_turns: 0,
            flip_horizontal: false,
            flip_vertical: false,
            angle_deg: 0.0,
            crop: [0.0, 0.0, 1.0, 1.0],
            aspect: "original".to_owned(),
        }
    }
}

impl ModuleParams for CropOrientation {
    const KIND: ModuleKind = ModuleKind::CROP_ORIENTATION;
    const VERSION: u16 = 1;

    fn to_fields(&self) -> Fields {
        Fields::new()
            .with("quarter_turns", Value::Int(self.quarter_turns))
            .with("flip_horizontal", Value::Bool(self.flip_horizontal))
            .with("flip_vertical", Value::Bool(self.flip_vertical))
            .with("angle_deg", f(self.angle_deg))
            .with("crop", Value::Floats(self.crop.to_vec()))
            .with("aspect", Value::Text(self.aspect.clone()))
    }

    fn from_fields(x: &Fields) -> Self {
        let d = Self::default();
        Self {
            quarter_turns: x.i64_or("quarter_turns", d.quarter_turns).rem_euclid(4),
            flip_horizontal: x.bool_or("flip_horizontal", d.flip_horizontal),
            flip_vertical: x.bool_or("flip_vertical", d.flip_vertical),
            angle_deg: x.f32_or("angle_deg", d.angle_deg),
            crop: x.floats_or("crop", d.crop),
            aspect: x.text("aspect").unwrap_or(&d.aspect).to_owned(),
        }
    }
}
