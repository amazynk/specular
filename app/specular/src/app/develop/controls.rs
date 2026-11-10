//! Develop's controls and the edit they set: the panel's cards, built from the edit
//! model, every value read from and written to a module's named fields, the history rows,
//! and undo and redo through [`History`].
//!
//! A [`Session`] is one image open in Develop. Changing a control pushes a step onto the
//! history (steps on one module within a second merge, as `History::push` does); undo,
//! redo and a click on a history row move along it and put the values back on the panel.

use std::path::{Path, PathBuf};

use specular_develop::model::params::{CropOrientation, ModuleParams};
use specular_develop::model::params::{
    Demosaic, Highlights, InputProfile, RawFrontEnd, ViewMethod,
};
use specular_develop::model::{
    Change, Edit, Fields, History, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Step,
    Value,
};
use specular_develop::ops::colour::{self, lut::LutSpace};
use specular_develop::ops::geometry::crop;
use specular_i18n::t;
use specular_theme::Color;
use specular_widgets::choice::Choice;
use specular_widgets::curve::Curve;
use specular_widgets::develop_panel::{
    Body, Card, Control, DevelopPanel, Event, Item, PanelButton,
};
use specular_widgets::slider::{Slider, SliderSpec, Unit};
use specular_widgets::wheels::Wheel;

/// The application's control ids.
pub(super) mod id {
    pub const EXPOSURE: u32 = 1;
    pub const BLACK_LEVEL: u32 = 2;
    /// Exposure's Auto, Off or On (its `auto` field).
    pub const AUTO_EXPOSURE: u32 = 3;
    pub const TEMPERATURE: u32 = 10;
    pub const TINT: u32 = 11;
    pub const HIGHLIGHTS: u32 = 20;
    pub const SHADOWS: u32 = 21;
    pub const SMOOTHING: u32 = 22;
    pub const CONTRAST: u32 = 30;
    pub const WHITES: u32 = 31;
    pub const BLACKS: u32 = 32;
    // View transform method, HDR limit, SDR preview.
    pub const VIEW_METHOD: u32 = 33;
    pub const HDR_LIMIT: u32 = 34;
    pub const SDR_PREVIEW: u32 = 35;
    pub const VIBRANCE: u32 = 40;
    pub const SATURATION: u32 = 41;
    pub const WHEELS: u32 = 42;
    pub const CURVE: u32 = 50;
    pub const LEVELS_BLACK: u32 = 51;
    pub const LEVELS_MID: u32 = 52;
    pub const LEVELS_WHITE: u32 = 53;
    pub const STRAIGHTEN: u32 = 60;
    // The Crop card's rows (`crop::card`): the tool's keys as rows.
    pub const CROP_ASPECT: u32 = 61;
    pub const CROP_TYPE: u32 = 62;
    pub const CROP_SWAP: u32 = 63;
    pub const CROP_TURN_LEFT: u32 = 64;
    pub const CROP_TURN_RIGHT: u32 = 65;
    pub const CROP_FLIP_H: u32 = 66;
    pub const CROP_FLIP_V: u32 = 67;
    pub const TEXTURE: u32 = 70;
    pub const CLARITY: u32 = 71;
    // Colour grading.
    pub const CHROMA: u32 = 43;
    pub const BRILLIANCE: u32 = 44;
    /// Colour mixer: hue, saturation, brightness of band `i` at `+ i`.
    pub const MIXER_HUE: u32 = 100;
    pub const MIXER_SATURATION: u32 = 110;
    pub const MIXER_BRIGHTNESS: u32 = 120;
    pub const POINT_PICK: u32 = 130;
    pub const POINT_RANGE: u32 = 131;
    pub const POINT_HUE_RANGE: u32 = 132;
    pub const POINT_SATURATION_RANGE: u32 = 133;
    pub const POINT_LIGHTNESS_RANGE: u32 = 134;
    pub const POINT_HUE: u32 = 135;
    pub const POINT_SATURATION: u32 = 136;
    pub const POINT_LIGHTNESS: u32 = 137;
    /// Black and white: band `i` at `+ i`.
    pub const BW_MIX: u32 = 140;
    pub const LUT_FILE: u32 = 150;
    /// The raw's camera profile (Auto, Matrix, a profile file) and its status line.
    pub const INPUT_PROFILE: u32 = 318;
    pub const INPUT_PROFILE_NOTE: u32 = 319;
    /// "Make profile from chart…".
    pub const CHART_PROFILE: u32 = 331;
    pub const LUT_SPACE: u32 = 151;
    pub const LUT_INTERPOLATION: u32 = 152;
    pub const LUT_STRENGTH: u32 = 153;
    pub const LENS_DISTORTION: u32 = 80;
    pub const LENS_VIGNETTE: u32 = 81;
    pub const LENS_TCA_RED: u32 = 82;
    pub const LENS_TCA_BLUE: u32 = 83;
    /// The Optics card's Lens corrections Off/On (the module's switch).
    pub const LENS_ON: u32 = 84;
    // The Detail card.
    pub const NOISE_LUMINANCE: u32 = 160;
    pub const NOISE_COLOUR: u32 = 161;
    pub const NOISE_DETAIL: u32 = 162;
    pub const SHARPEN_AMOUNT: u32 = 163;
    pub const SHARPEN_RADIUS: u32 = 164;
    pub const SHARPEN_DETAIL: u32 = 165;
    pub const SHARPEN_MASKING: u32 = 166;
    pub const CAPTURE_AMOUNT: u32 = 167;
    pub const CAPTURE_RADIUS: u32 = 168;
    pub const DEMOSAIC: u32 = 169;
    pub const DUAL_THRESHOLD: u32 = 186;
    // Denoise's AI mode.
    pub const NOISE_MODE: u32 = 187;
    pub const NOISE_BLEND: u32 = 188;
    pub const NOISE_AI_NOTE: u32 = 189;
    pub const NOISE_AI_ACTION: u32 = 190;
    // The Corrections card.
    pub const HL_METHOD: u32 = 170;
    pub const HL_THRESHOLD: u32 = 171;
    pub const HOT: u32 = 172;
    pub const HOT_THRESHOLD: u32 = 173;
    pub const CA_MODE: u32 = 174;
    pub const CA_RED: u32 = 175;
    pub const CA_BLUE: u32 = 176;
    pub const DEFRINGE: u32 = 177;
    pub const DEFRINGE_PURPLE: u32 = 178;
    pub const DEFRINGE_GREEN: u32 = 179;
    pub const DEFRINGE_RANGE: u32 = 180;
    pub const DEFRINGE_EDGE: u32 = 181;
    pub const FLAT_MODE: u32 = 182;
    pub const FLAT_FILE: u32 = 183;
    pub const DARK_FILE: u32 = 184;
    pub const FLAT_USE: u32 = 185;
    // The Effects cards.
    pub const NEG_PICK: u32 = 200;
    pub const NEG_DMAX: u32 = 201;
    pub const NEG_EXPOSURE: u32 = 202;
    pub const NEG_GRADE: u32 = 203;
    pub const NEG_GLOSS: u32 = 204;
    pub const NEG_FILM: u32 = 205;
    pub const DEHAZE: u32 = 206;
    pub const ND_AMOUNT: u32 = 207;
    pub const ND_ANGLE: u32 = 208;
    pub const ND_SOFTNESS: u32 = 209;
    pub const ND_TINT_HUE: u32 = 210;
    pub const ND_TINT: u32 = 211;
    pub const ND_EDIT: u32 = 212;
    pub const VG_AMOUNT: u32 = 213;
    pub const VG_MIDPOINT: u32 = 214;
    pub const VG_ROUNDNESS: u32 = 215;
    pub const VG_SHAPE: u32 = 216;
    pub const VG_FEATHER: u32 = 217;
    pub const VG_PROTECT: u32 = 218;
    pub const VG_CX: u32 = 219;
    pub const VG_CY: u32 = 220;
    pub const GLOW_AMOUNT: u32 = 221;
    pub const GLOW_RADIUS: u32 = 222;
    pub const GLOW_THRESHOLD: u32 = 223;
    pub const GLOW_MODE: u32 = 224;
    pub const GRAIN_AMOUNT: u32 = 225;
    pub const GRAIN_SIZE: u32 = 226;
    pub const GRAIN_BIAS: u32 = 227;
    pub const GRAIN_COLOUR: u32 = 228;
    pub const FRAME_BORDER: u32 = 229;
    pub const FRAME_COLOUR: u32 = 230;
    pub const FRAME_ASPECT: u32 = 231;
    pub const FRAME_MARK: u32 = 232;
    pub const FRAME_TEXT: u32 = 233;
    pub const FRAME_SVG: u32 = 234;
    pub const FRAME_PLACE: u32 = 235;
    pub const FRAME_SCALE: u32 = 236;
    pub const FRAME_OPACITY: u32 = 237;
    pub const FRAME_SHOW: u32 = 238;
    /// Tone equalizer zone `i` (−8 + i EV) at `+ i`, and the on-picture adjusting.
    pub const ZONE: u32 = 240;
    pub const ZONE_LAST: u32 = 248;
    pub const ZONE_DRAG: u32 = 249;
}

/// The Detail card: denoise, capture sharpening and sharpen (not one module, so no switch;
/// each module is left out while its strength is 0).
pub(super) const DETAIL_CARD: u32 = 0x2_0000;

/// The Corrections card: highlights, hot pixels, chromatic aberration, defringe and the
/// calibration frames (several modules, so no switch; choices turn each on or off).
pub(super) const CORRECTIONS_CARD: u32 = 0x3_0000;

/// The Tone zones card under Tone: the tone equalizer's nine zone sliders and its smoothing
/// (the fine control; Tone keeps highlights, shadows and adjusting on the picture). No
/// switch: Tone's is the module's.
pub(super) const ZONES_CARD: u32 = 0x7_0000;

/// The Colour grading card: the colour balance module's wheels, chroma and brilliance (Colour
/// keeps vibrance and saturation). No switch, like Colour: moving one turns the module on.
pub(super) const GRADING_CARD: u32 = 0x8_0000;

/// The tools most photos never need. Their cards stay out of the panel ([`DevelopPanel::hidden`])
/// until added from the Add a tool card or used by the edit, and then stay for the session.
pub(super) const TOOLS: [ModuleKind; 10] = [
    ModuleKind::POINT_COLOUR,
    ModuleKind::LUT_3D,
    ModuleKind::SURFACE_BLUR,
    ModuleKind::DIFFUSE,
    ModuleKind::NEGATIVE,
    ModuleKind::GRADUATED_ND,
    ModuleKind::GLOW,
    ModuleKind::FRAME,
    ModuleKind::RELIGHT,
    ModuleKind::DEPTH_BLUR,
];

/// The Add a tool card, last: a button per tool not shown yet (none: the card is hidden).
pub(super) const TOOLS_CARD: u32 = 0x9_0000;

/// Add a tool's buttons: `ADD_TOOL + i` shows [`TOOLS`]`[i]`.
pub(super) const ADD_TOOL: u32 = 0x60_0000;

/// The demosaic methods, in the order the choice shows them.
const DEMOSAICS: [Demosaic; 3] = [Demosaic::Directional, Demosaic::Dual, Demosaic::Fast];

/// The highlight methods, in the order the choice shows them.
const HL_METHODS: [Highlights; 3] = [Highlights::Opposed, Highlights::Rebuild, Highlights::Clip];

/// The HDR card (not a module: its slider writes the view transform's HDR limit, its
/// choice is the session's SDR preview).
pub(super) const HDR_CARD: u32 = 0x1_0000 | 0xFFF0;

/// The view transform methods, in the order the choice shows them.
const VIEW_METHODS: [ViewMethod; 3] = [ViewMethod::Sigmoid, ViewMethod::Filmic, ViewMethod::Agx];

/// The tonal ranges of the colour wheels, in order, as field-name prefixes.
pub(super) const RANGES: [&str; 4] = colour::balance::RANGES;

/// The colour bands of the mixer and black and white, as field-name stems.
const BANDS: [&str; 8] = colour::BANDS;

/// The LUT spaces, in the order the choice shows them.
const LUT_SPACES: [LutSpace; 5] = [
    LutSpace::LinearRec2020,
    LutSpace::Acescg,
    LutSpace::Acescct,
    LutSpace::Srgb,
    LutSpace::Rec709,
];

/// Point colour's range sliders: while one is dragged the picture shows what moves.
const POINT_RANGES: [u32; 4] = [
    id::POINT_RANGE,
    id::POINT_HUE_RANGE,
    id::POINT_SATURATION_RANGE,
    id::POINT_LIGHTNESS_RANGE,
];

/// Fields of the modules the panel drives that start from named fields.
fn module_defaults(kind: ModuleKind) -> Option<Fields> {
    let f0 = |f: Fields, n: &str| f.with(n, Value::Float(0.0));
    Some(match kind {
        ModuleKind::TONE_EQUALIZER => f0(f0(Fields::new(), "highlights_ev"), "shadows_ev"),
        ModuleKind::COLOUR_BALANCE => colour::balance::ColourBalance::defaults(),
        ModuleKind::COLOUR_EQUALIZER => colour::equalizer::ColourEqualizer::defaults(),
        ModuleKind::POINT_COLOUR => colour::point::PointColour::defaults(),
        ModuleKind::BLACK_WHITE => colour::bw::BlackWhite::defaults(),
        ModuleKind::LUT_3D => colour::lut::Lut3d::defaults(),
        ModuleKind::CURVES => Fields::new().with("points", Value::Floats(vec![0.0, 0.0, 1.0, 1.0])),
        ModuleKind::LOCAL_CONTRAST => f0(f0(Fields::new(), "texture"), "clarity"),
        _ => return None,
    })
}

/// A colour band's name.
fn band_name(i: usize) -> String {
    match BANDS.get(i).copied().unwrap_or("") {
        "red" => t!("develop-colour-red"),
        "orange" => t!("develop-colour-orange"),
        "yellow" => t!("develop-colour-yellow"),
        "green" => t!("develop-colour-green"),
        "aqua" => t!("develop-colour-aqua"),
        "blue" => t!("develop-colour-blue"),
        "purple" => t!("develop-colour-purple"),
        _ => t!("develop-colour-magenta"),
    }
}

/// A banded field (`hue_red`, `mix_blue`) as its band and kind ("hue", "mix", …).
fn band_field(field: &str) -> Option<(&str, usize)> {
    let (kind, band) = field.split_once('_')?;
    Some((kind, BANDS.iter().position(|b| *b == band)?))
}

/// A colour band slider's label.
fn band_label(kind: &str, i: usize) -> String {
    let colour = band_name(i);
    match kind {
        "hue" => t!("develop-mixer-hue", colour = colour),
        "saturation" => t!("develop-mixer-saturation", colour = colour),
        "brightness" => t!("develop-mixer-brightness", colour = colour),
        _ => t!("develop-bw-mix", colour = colour),
    }
}

/// Files Develop treats as raw (the raw front end joins the stack); a raw video clip
/// (Cinema RAW Light, ProRes RAW) develops its first frame.
pub(super) fn is_raw(path: &Path) -> bool {
    const RAW: &[&str] = &[
        "3fr", "ari", "arw", "cr2", "cr3", "crw", "dcr", "dng", "erf", "fff", "iiq", "k25", "kdc",
        "mef", "mos", "mrw", "nef", "nrw", "orf", "pef", "raf", "raw", "rw2", "rwl", "sr2", "srf",
        "srw", "x3f",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RAW.contains(&e.to_ascii_lowercase().as_str()))
        || specular_develop::thumb::is_raw_clip(path)
}

/// The RAW of a RAW+JPEG pair (the gallery's pairs): for a photo that is not a raw,
/// a raw file beside it with the same name; Develop develops that one.
pub(super) fn raw_partner(path: &Path) -> Option<PathBuf> {
    const RAW: &[&str] = &[
        "dng", "cr3", "cr2", "nef", "arw", "raf", "orf", "rw2", "pef", "srw", "nrw", "crw", "3fr",
        "ari", "dcr", "erf", "fff", "iiq", "k25", "kdc", "mef", "mos", "mrw", "raw", "rwl", "sr2",
        "srf", "x3f", "gpr",
    ];
    if is_raw(path) {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let dir = path.parent()?;
    let found = RAW
        .iter()
        .flat_map(|e| [e.to_string(), e.to_ascii_uppercase()])
        .map(|e| dir.join(format!("{stem}.{e}")))
        .find(|p| p.is_file())?;
    // A case-insensitive file system (macOS, Windows) finds `IMG_1.CR3` as `IMG_1.cr3`: give
    // the name as stored, which the Library and the sidecars know it by.
    let asked = found.file_name()?.to_str()?;
    let stored = std::fs::read_dir(dir).ok().and_then(|entries| {
        entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.eq_ignore_ascii_case(asked))
            .min_by_key(|n| n != asked)
    });
    Some(stored.map_or(found, |n| dir.join(n)))
}

/// What a raw photo starts with besides the identity, as Lightroom does: a raw arrives
/// softer and with more colour noise than the camera's own JPEG, which had both done in the
/// camera. Some colour noise reduction (luminance stays 0: it costs detail, and its
/// non-local means pass is the slow one) and capture sharpening at the lens's radius.
pub(super) const RAW_DEFAULTS: [(ModuleKind, &str, f32); 2] = [
    (ModuleKind::DENOISE, "colour", 0.25),
    (ModuleKind::CAPTURE_SHARPENING, "amount", 0.3),
];

/// The edit Develop starts from: the default stack, plus tone, colour and curve modules at
/// their identity, and on a raw its [`RAW_DEFAULTS`]. (A saved edit is kept as it was.)
pub(super) fn new_edit(path: &Path) -> Edit {
    let reg = Registry::builtin();
    let raw = is_raw(path);
    let mut e = Edit::default_for(reg, SourceKey::default(), raw);
    ensure_modules(&mut e, path);
    if raw {
        for (kind, field, v) in RAW_DEFAULTS {
            if let Some(m) = e.module_mut(kind, 0) {
                let mut f = m.params.to_fields();
                f.set(field, Value::Float(v));
                m.params.set_fields(f);
                m.enabled = true;
            }
        }
    }
    e
}

/// The raw default of control `control`'s field, if it has one (its slider resets there).
fn raw_default(control: u32) -> Option<f32> {
    let (kind, name) = float_field(control)?;
    RAW_DEFAULTS
        .iter()
        .find(|(k, f, _)| *k == kind && *f == name)
        .map(|d| d.2)
}

/// `edit` less what a raw starts with ([`RAW_DEFAULTS`]): a module still at its raw default
/// is left off, so the catalogue does not list it as used (`module:denoise` finds the photos
/// whose noise reduction someone set).
pub(super) fn without_raw_defaults(edit: &Edit) -> std::borrow::Cow<'_, Edit> {
    let reg = Registry::builtin();
    // A raw's modules as [`new_edit`] has them before its raw defaults (the registry's
    // defaults are not always where a new raw starts). Built once: the catalogue asks per photo.
    static FRESH: std::sync::OnceLock<Edit> = std::sync::OnceLock::new();
    let fresh = FRESH.get_or_init(|| {
        let mut e = Edit::default_for(reg, SourceKey::default(), true);
        ensure_modules(&mut e, Path::new("raw.dng"));
        e
    });
    let at_default = |m: &ModuleInstance| {
        RAW_DEFAULTS.iter().any(|(k, f, v)| {
            *k == m.kind
                && m.instance == 0
                && m.blend.is_none()
                && fresh.module(*k, 0).is_some_and(|base| {
                    m.params.to_fields() == base.params.to_fields().with(f, Value::Float(*v))
                })
        })
    };
    if !edit.modules.iter().any(|m| m.enabled && at_default(m)) {
        return std::borrow::Cow::Borrowed(edit);
    }
    let mut e = edit.clone();
    for m in &mut e.modules {
        if at_default(m) {
            m.enabled = false;
        }
    }
    std::borrow::Cow::Owned(e)
}

/// Adds the modules the panel drives that an edit lacks (at their identity, no history).
fn ensure_modules(e: &mut Edit, path: &Path) {
    let reg = Registry::builtin();
    for kind in [
        ModuleKind::EXPOSURE,
        ModuleKind::COLOUR_CALIBRATION,
        ModuleKind::TONE_EQUALIZER,
        ModuleKind::LOCAL_CONTRAST,
        ModuleKind::COLOUR_BALANCE,
        ModuleKind::COLOUR_EQUALIZER,
        ModuleKind::POINT_COLOUR,
        ModuleKind::BLACK_WHITE,
        ModuleKind::CURVES,
        ModuleKind::LUT_3D,
        ModuleKind::VIEW_TRANSFORM,
        ModuleKind::CROP_ORIENTATION,
        ModuleKind::PERSPECTIVE,
        ModuleKind::LENS,
        ModuleKind::DENOISE,
        ModuleKind::CAPTURE_SHARPENING,
        ModuleKind::SHARPEN,
        ModuleKind::SURFACE_BLUR,
        ModuleKind::DIFFUSE,
        ModuleKind::HOT_PIXELS,
        ModuleKind::CHROMATIC_ABERRATION,
        ModuleKind::DEFRINGE,
        ModuleKind::FLAT_FIELD,
    ]
    .into_iter()
    .chain(super::effects::EFFECTS)
    .chain(super::depth_tools::TOOLS)
    {
        if e.module(kind, 0).is_some() {
            continue;
        }
        let mut m = ModuleInstance::new(reg, kind, 0);
        if let Some(f) = module_defaults(kind) {
            m.params = Params::from_fields(kind, f);
        }
        // Each photo its own grain, the same every time.
        if kind == ModuleKind::GRAIN {
            let f = m
                .params
                .to_fields()
                .with("seed", Value::Int(super::effects::grain_seed(path)));
            m.params = Params::from_fields(kind, f);
        }
        // Black and white always turns the picture grey: off until asked for. Chromatic
        // aberration and calibration frames are off until asked for; hot pixels are on.
        m.enabled = !matches!(
            kind,
            ModuleKind::BLACK_WHITE
                | ModuleKind::CHROMATIC_ABERRATION
                | ModuleKind::FLAT_FIELD
                | ModuleKind::NEGATIVE
        );
        e.insert(reg, m);
    }
}

/// A plain float field a slider sets: its module and name.
fn float_field(control: u32) -> Option<(ModuleKind, String)> {
    let band = |base: u32| BANDS.get((control - base) as usize).copied();
    let banded = match control {
        id::MIXER_HUE..=127 | id::BW_MIX..=147 => {
            let (kind, prefix, base) = match control {
                id::MIXER_HUE..=107 => (ModuleKind::COLOUR_EQUALIZER, "hue", id::MIXER_HUE),
                id::MIXER_SATURATION..=117 => (
                    ModuleKind::COLOUR_EQUALIZER,
                    "saturation",
                    id::MIXER_SATURATION,
                ),
                id::MIXER_BRIGHTNESS..=127 => (
                    ModuleKind::COLOUR_EQUALIZER,
                    "brightness",
                    id::MIXER_BRIGHTNESS,
                ),
                _ => (ModuleKind::BLACK_WHITE, "mix", id::BW_MIX),
            };
            band(base).map(|b| (kind, format!("{prefix}_{b}")))
        }
        _ => None,
    };
    if banded.is_some() {
        return banded;
    }
    if let Some((k, n)) = super::diffuse::float_field(control) {
        return Some((k, n.to_owned()));
    }
    float_field_fixed(control).map(|(k, n)| (k, n.to_owned()))
}

fn float_field_fixed(control: u32) -> Option<(ModuleKind, &'static str)> {
    Some(match control {
        id::CHROMA => (ModuleKind::COLOUR_BALANCE, "chroma"),
        id::BRILLIANCE => (ModuleKind::COLOUR_BALANCE, "brilliance"),
        id::POINT_RANGE => (ModuleKind::POINT_COLOUR, "range"),
        id::POINT_HUE_RANGE => (ModuleKind::POINT_COLOUR, "hue_range"),
        id::POINT_SATURATION_RANGE => (ModuleKind::POINT_COLOUR, "saturation_range"),
        id::POINT_LIGHTNESS_RANGE => (ModuleKind::POINT_COLOUR, "lightness_range"),
        id::POINT_HUE => (ModuleKind::POINT_COLOUR, "shift_hue"),
        id::POINT_SATURATION => (ModuleKind::POINT_COLOUR, "shift_saturation"),
        id::POINT_LIGHTNESS => (ModuleKind::POINT_COLOUR, "shift_lightness"),
        id::LUT_STRENGTH => (ModuleKind::LUT_3D, "strength"),
        id::EXPOSURE => (ModuleKind::EXPOSURE, "exposure_ev"),
        id::BLACK_LEVEL => (ModuleKind::EXPOSURE, "black_offset"),
        id::HIGHLIGHTS => (ModuleKind::TONE_EQUALIZER, "highlights_ev"),
        id::SHADOWS => (ModuleKind::TONE_EQUALIZER, "shadows_ev"),
        id::SMOOTHING => (ModuleKind::TONE_EQUALIZER, "smoothing"),
        id::TEXTURE => (ModuleKind::LOCAL_CONTRAST, "texture"),
        id::CLARITY => (ModuleKind::LOCAL_CONTRAST, "clarity"),
        id::LEVELS_BLACK => (ModuleKind::CURVES, "levels_black"),
        id::LEVELS_MID => (ModuleKind::CURVES, "levels_grey"),
        id::LEVELS_WHITE => (ModuleKind::CURVES, "levels_white"),
        id::CONTRAST => (ModuleKind::VIEW_TRANSFORM, "contrast"),
        id::WHITES => (ModuleKind::VIEW_TRANSFORM, "white_target"),
        id::BLACKS => (ModuleKind::VIEW_TRANSFORM, "black_target"),
        id::HDR_LIMIT => (ModuleKind::VIEW_TRANSFORM, "hdr_limit_ev"),
        id::VIBRANCE => (ModuleKind::COLOUR_BALANCE, "vibrance"),
        id::SATURATION => (ModuleKind::COLOUR_BALANCE, "saturation"),
        id::STRAIGHTEN => (ModuleKind::CROP_ORIENTATION, "angle_deg"),
        id::LENS_DISTORTION => (ModuleKind::LENS, "distortion"),
        id::LENS_VIGNETTE => (ModuleKind::LENS, "vignette"),
        id::LENS_TCA_RED => (ModuleKind::LENS, "tca_red"),
        id::LENS_TCA_BLUE => (ModuleKind::LENS, "tca_blue"),
        id::NOISE_LUMINANCE => (ModuleKind::DENOISE, "luminance"),
        id::NOISE_COLOUR => (ModuleKind::DENOISE, "colour"),
        id::NOISE_DETAIL => (ModuleKind::DENOISE, "detail"),
        id::NOISE_BLEND => (ModuleKind::DENOISE, "blend"),
        id::SHARPEN_AMOUNT => (ModuleKind::SHARPEN, "amount"),
        id::SHARPEN_RADIUS => (ModuleKind::SHARPEN, "radius"),
        id::SHARPEN_DETAIL => (ModuleKind::SHARPEN, "detail"),
        id::SHARPEN_MASKING => (ModuleKind::SHARPEN, "masking"),
        id::CAPTURE_AMOUNT => (ModuleKind::CAPTURE_SHARPENING, "amount"),
        id::CAPTURE_RADIUS => (ModuleKind::CAPTURE_SHARPENING, "radius"),
        id::HL_THRESHOLD => (ModuleKind::RAW_FRONT_END, "highlight_threshold"),
        id::DUAL_THRESHOLD => (ModuleKind::RAW_FRONT_END, "dual_threshold"),
        id::HOT_THRESHOLD => (ModuleKind::HOT_PIXELS, "threshold"),
        id::CA_RED => (ModuleKind::CHROMATIC_ABERRATION, "red"),
        id::CA_BLUE => (ModuleKind::CHROMATIC_ABERRATION, "blue"),
        id::DEFRINGE => (ModuleKind::DEFRINGE, "amount"),
        id::DEFRINGE_PURPLE => (ModuleKind::DEFRINGE, "purple_hue"),
        id::DEFRINGE_GREEN => (ModuleKind::DEFRINGE, "green_hue"),
        id::DEFRINGE_RANGE => (ModuleKind::DEFRINGE, "hue_range"),
        id::DEFRINGE_EDGE => (ModuleKind::DEFRINGE, "edge"),
        c => return super::effects::float_field(c),
    })
}

/// The module a control belongs to.
pub(super) fn module_of(control: u32) -> ModuleKind {
    match control {
        id::TEMPERATURE | id::TINT => ModuleKind::COLOUR_CALIBRATION,
        id::WHEELS => ModuleKind::COLOUR_BALANCE,
        id::CURVE => ModuleKind::CURVES,
        id::POINT_PICK => ModuleKind::POINT_COLOUR,
        id::LUT_FILE | id::LUT_SPACE | id::LUT_INTERPOLATION => ModuleKind::LUT_3D,
        id::VIEW_METHOD | id::SDR_PREVIEW => ModuleKind::VIEW_TRANSFORM,
        id::HL_METHOD | id::DEMOSAIC | id::INPUT_PROFILE | id::INPUT_PROFILE_NOTE => {
            ModuleKind::RAW_FRONT_END
        }
        id::CHART_PROFILE => ModuleKind::COLOUR_CALIBRATION,
        id::CROP_ASPECT..=id::CROP_FLIP_V => ModuleKind::CROP_ORIENTATION,
        id::ZONE..=id::ZONE_DRAG => ModuleKind::TONE_EQUALIZER,
        id::HOT => ModuleKind::HOT_PIXELS,
        id::LENS_ON | super::lensfun::id::PROFILE | super::lensfun::id::NOTE => ModuleKind::LENS,
        id::AUTO_EXPOSURE => ModuleKind::EXPOSURE,
        id::NOISE_MODE | id::NOISE_AI_NOTE | id::NOISE_AI_ACTION => ModuleKind::DENOISE,
        id::CA_MODE => ModuleKind::CHROMATIC_ABERRATION,
        id::FLAT_MODE | id::FLAT_FILE | id::DARK_FILE | id::FLAT_USE => ModuleKind::FLAT_FIELD,
        c if super::diffuse::is_diffuse(c) => {
            super::diffuse::module_of(c).unwrap_or(ModuleKind::DIFFUSE)
        }
        c => super::effects::module_of(c)
            .or_else(|| float_field(c).map(|f| f.0))
            .unwrap_or(ModuleKind::EXPOSURE),
    }
}

/// A module's title.
pub(super) fn module_title(kind: ModuleKind) -> String {
    match kind {
        ModuleKind::EXPOSURE => t!("develop-module-exposure"),
        ModuleKind::COLOUR_CALIBRATION => t!("develop-module-white-balance"),
        ModuleKind::TONE_EQUALIZER => t!("develop-module-tone"),
        ModuleKind::LOCAL_CONTRAST => t!("develop-module-local-contrast"),
        ModuleKind::VIEW_TRANSFORM => t!("develop-module-contrast"),
        ModuleKind::COLOUR_BALANCE => t!("develop-module-colour"),
        ModuleKind::CURVES => t!("develop-module-curve"),
        ModuleKind::CROP_ORIENTATION => t!("develop-module-crop"),
        ModuleKind::COLOUR_EQUALIZER => t!("develop-module-colour-mixer"),
        ModuleKind::POINT_COLOUR => t!("develop-module-point-colour"),
        ModuleKind::BLACK_WHITE => t!("develop-module-black-white"),
        ModuleKind::LUT_3D => t!("develop-module-lut"),
        ModuleKind::LENS => t!("develop-module-lens"),
        ModuleKind::DENOISE => t!("develop-module-denoise"),
        ModuleKind::CAPTURE_SHARPENING => t!("develop-capture-amount"),
        ModuleKind::SHARPEN => t!("develop-module-sharpen"),
        ModuleKind::RAW_FRONT_END => t!("develop-module-raw"),
        ModuleKind::HOT_PIXELS => t!("develop-module-hot-pixels"),
        ModuleKind::CHROMATIC_ABERRATION => t!("develop-module-ca"),
        ModuleKind::DEFRINGE => t!("develop-module-defringe"),
        ModuleKind::FLAT_FIELD => t!("develop-module-flat-field"),
        ModuleKind::DIFFUSE => t!("develop-module-diffuse"),
        ModuleKind::SURFACE_BLUR => t!("develop-module-surface-blur"),
        ModuleKind::RETOUCH => t!("develop-module-retouch"),
        k => super::effects::title(k).unwrap_or_else(|| t!("develop-module-other")),
    }
}

/// The name of a field in a history row ("Exposure", "Temperature").
fn field_label(kind: ModuleKind, field: &str) -> Option<String> {
    if let Some(l) = super::diffuse::field_label(kind, field) {
        return Some(l);
    }
    Some(match (kind, field) {
        (ModuleKind::EXPOSURE, "exposure_ev") => t!("develop-exposure"),
        (ModuleKind::EXPOSURE, "black_offset") => t!("develop-black-level"),
        (ModuleKind::EXPOSURE, "auto") => t!("develop-auto-exposure"),
        (ModuleKind::COLOUR_CALIBRATION, "white_x" | "white_y" | "white_source") => {
            t!("develop-module-white-balance")
        }
        (ModuleKind::TONE_EQUALIZER, "highlights_ev") => t!("develop-highlights"),
        (ModuleKind::TONE_EQUALIZER, "shadows_ev") => t!("develop-shadows"),
        (ModuleKind::TONE_EQUALIZER, "smoothing") => t!("develop-smoothing"),
        (ModuleKind::TONE_EQUALIZER, "zones") => t!("develop-tone-zones"),
        (ModuleKind::LOCAL_CONTRAST, "texture") => t!("develop-texture"),
        (ModuleKind::LOCAL_CONTRAST, "clarity") => t!("develop-clarity"),
        (ModuleKind::CURVES, "levels_black") => t!("develop-levels-black"),
        (ModuleKind::CURVES, "levels_grey") => t!("develop-levels-mid"),
        (ModuleKind::CURVES, "levels_white") => t!("develop-levels-white"),
        (ModuleKind::VIEW_TRANSFORM, "contrast") => t!("develop-contrast"),
        (ModuleKind::VIEW_TRANSFORM, "white_target") => t!("develop-whites"),
        (ModuleKind::VIEW_TRANSFORM, "black_target") => t!("develop-blacks"),
        (ModuleKind::VIEW_TRANSFORM, "method") => t!("develop-look-profile"),
        (ModuleKind::VIEW_TRANSFORM, "hdr_limit_ev") => t!("develop-hdr-limit"),
        (ModuleKind::COLOUR_BALANCE, "vibrance") => t!("develop-vibrance"),
        (ModuleKind::COLOUR_BALANCE, "saturation") => t!("develop-saturation"),
        (ModuleKind::COLOUR_BALANCE, "chroma") => t!("develop-chroma"),
        (ModuleKind::COLOUR_BALANCE, "brilliance") => t!("develop-brilliance"),
        (ModuleKind::COLOUR_BALANCE, _) => t!("develop-colour-wheels"),
        (ModuleKind::COLOUR_EQUALIZER | ModuleKind::BLACK_WHITE, f) => {
            let (kind, i) = band_field(f)?;
            band_label(kind, i)
        }
        (ModuleKind::POINT_COLOUR, f) => match f {
            "range" => t!("develop-point-range"),
            "hue_range" => t!("develop-point-hue-range"),
            "saturation_range" => t!("develop-point-saturation-range"),
            "lightness_range" => t!("develop-point-lightness-range"),
            "shift_hue" => t!("develop-point-hue"),
            "shift_saturation" => t!("develop-point-saturation"),
            "shift_lightness" => t!("develop-point-lightness"),
            "picked" | "target_hue" | "target_saturation" | "target_lightness" => {
                t!("develop-point-picked")
            }
            _ => return None,
        },
        (ModuleKind::LUT_3D, f) => match f {
            "path" => t!("develop-module-lut"),
            "space" => t!("develop-lut-space"),
            "interpolation" => t!("develop-lut-interpolation"),
            "strength" => t!("develop-lut-strength"),
            _ => return None,
        },
        (ModuleKind::CURVES, _) => t!("develop-curve"),
        (ModuleKind::CROP_ORIENTATION, "angle_deg") => t!("develop-straighten"),
        (ModuleKind::CROP_ORIENTATION, "crop" | "aspect") => t!("develop-crop"),
        (ModuleKind::CROP_ORIENTATION, "quarter_turns") => t!("develop-crop-turn"),
        (ModuleKind::CROP_ORIENTATION, "flip_horizontal" | "flip_vertical") => {
            t!("develop-crop-flip")
        }
        (ModuleKind::LENS, "distortion") => t!("develop-lens-distortion"),
        (ModuleKind::LENS, "vignette") => t!("develop-lens-vignette"),
        (ModuleKind::LENS, "tca_red") => t!("develop-lens-tca-red"),
        (ModuleKind::LENS, "tca_blue") => t!("develop-lens-tca-blue"),
        (ModuleKind::LENS, "lensfun") => t!("develop-lensfun-profile"),
        (ModuleKind::DENOISE, "luminance") => t!("develop-noise-luminance"),
        (ModuleKind::DENOISE, "colour") => t!("develop-noise-colour"),
        (ModuleKind::DENOISE, "detail") => t!("develop-noise-detail"),
        (ModuleKind::DENOISE, "blend") => t!("develop-noise-ai-blend"),
        (ModuleKind::DENOISE, "mode") => t!("develop-noise-mode"),
        (ModuleKind::SHARPEN, "amount") => t!("develop-sharpen-amount"),
        (ModuleKind::SHARPEN, "radius") => t!("develop-sharpen-radius"),
        (ModuleKind::SHARPEN, "detail") => t!("develop-sharpen-detail"),
        (ModuleKind::SHARPEN, "masking") => t!("develop-sharpen-masking"),
        (ModuleKind::CAPTURE_SHARPENING, "amount") => t!("develop-capture-amount"),
        (ModuleKind::CAPTURE_SHARPENING, "radius") => t!("develop-capture-radius"),
        (ModuleKind::RAW_FRONT_END, "highlights") => t!("develop-highlight-method"),
        (ModuleKind::RAW_FRONT_END, "highlight_threshold") => t!("develop-highlight-threshold"),
        (ModuleKind::RAW_FRONT_END, "demosaic") => t!("develop-demosaic"),
        (ModuleKind::RAW_FRONT_END, "dual_threshold") => t!("develop-dual-threshold"),
        (ModuleKind::HOT_PIXELS, "threshold") => t!("develop-hot-threshold"),
        (ModuleKind::CHROMATIC_ABERRATION, "auto") => t!("develop-ca-mode"),
        (ModuleKind::CHROMATIC_ABERRATION, "red") => t!("develop-ca-red"),
        (ModuleKind::CHROMATIC_ABERRATION, "blue") => t!("develop-ca-blue"),
        (ModuleKind::DEFRINGE, "amount") => t!("develop-defringe"),
        (ModuleKind::DEFRINGE, "purple_hue") => t!("develop-defringe-purple"),
        (ModuleKind::DEFRINGE, "green_hue") => t!("develop-defringe-green"),
        (ModuleKind::DEFRINGE, "hue_range") => t!("develop-defringe-range"),
        (ModuleKind::DEFRINGE, "edge") => t!("develop-defringe-edge"),
        (ModuleKind::FLAT_FIELD, "flat") => t!("develop-flat-frame"),
        (ModuleKind::FLAT_FIELD, "dark") => t!("develop-dark-frame"),
        (ModuleKind::FLAT_FIELD, "auto") => t!("develop-calibration"),
        (ModuleKind::FLAT_FIELD, "blur") => t!("develop-flat-use"),
        (k, f) => return super::effects::field_label(k, f),
    })
}

fn slider(label: String, min: f32, max: f32, default: f32) -> SliderSpec {
    SliderSpec::new(label, min, max, default)
}

/// The panel's cards, in pipeline order, values at their defaults ([`cards_for`] puts them in
/// the panel's order).
pub(super) fn cards() -> Vec<Card> {
    let s = |id: u32, spec: SliderSpec| Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    };
    let card = |kind: ModuleKind, switch: bool, controls: Vec<Control>| Card {
        id: u32::from(kind.0),
        title: module_title(kind),
        enabled: switch.then_some(true),
        expanded: true,
        controls,
    };
    let wheels = RANGES
        .iter()
        .map(|r| {
            let (name, lum) = match *r {
                "shadows" => (t!("develop-wheel-shadows"), t!("develop-luminance-shadows")),
                "midtones" => (
                    t!("develop-wheel-midtones"),
                    t!("develop-luminance-midtones"),
                ),
                "highlights" => (
                    t!("develop-wheel-highlights"),
                    t!("develop-luminance-highlights"),
                ),
                _ => (t!("develop-wheel-global"), t!("develop-luminance-global")),
            };
            let spec = slider(lum, -1.0, 1.0, 0.0)
                .steps(0.01, 0.1)
                .unit(Unit::Percent, 0)
                .signed()
                .compact();
            (Wheel::new(name), Slider::new(spec))
        })
        .collect();
    // Crop starts closed, like the other repairs it sits with.
    let mut v = vec![
        Card {
            expanded: false,
            ..card(
                ModuleKind::CROP_ORIENTATION,
                false,
                std::iter::once(s(
                    id::STRAIGHTEN,
                    slider(t!("develop-straighten"), -45.0, 45.0, 0.0)
                        .steps(0.1, 1.0)
                        .unit(Unit::Degrees, 1)
                        .signed(),
                ))
                .chain(super::crop::card::controls())
                .collect(),
            )
        },
        super::perspective::card(),
        optics_card(),
        light_card(),
        card(
            ModuleKind::COLOUR_CALIBRATION,
            false,
            vec![
                s(
                    id::TEMPERATURE,
                    slider(t!("develop-temperature"), 2000.0, 12000.0, DEFAULT_KELVIN)
                        .steps(50.0, 500.0)
                        .unit(Unit::Kelvin, 0)
                        // Raising it warms the picture (it is the light's temperature).
                        .scale(Color::hex(0x5B8DEF), Color::hex(0xEDB03D)),
                ),
                s(
                    id::TINT,
                    slider(t!("develop-tint"), -100.0, 100.0, 0.0)
                        .steps(1.0, 10.0)
                        .unit(Unit::Plain, 0)
                        .signed()
                        // Positive is magenta (Lightroom-like, see `white_xy`).
                        .scale(Color::hex(0x4FAE6B), Color::hex(0xCF5FC6)),
                ),
                Control {
                    id: id::INPUT_PROFILE,
                    body: Body::Choice(Choice::new(
                        t!("develop-input-profile"),
                        vec![
                            t!("develop-input-profile-auto"),
                            t!("develop-input-profile-matrix"),
                            t!("develop-input-profile-file"),
                        ],
                    )),
                },
                Control {
                    id: id::INPUT_PROFILE_NOTE,
                    body: Body::Note(String::new()),
                },
                Control {
                    id: id::CHART_PROFILE,
                    body: Body::Choice(Choice::button(t!("develop-chart-start"))),
                },
            ],
        ),
        Card {
            id: ZONES_CARD,
            title: t!("develop-tone-zones"),
            enabled: None,
            expanded: false,
            controls: [super::zones::button()]
                .into_iter()
                .chain(super::zones::sliders())
                .chain([s(
                    id::SMOOTHING,
                    slider(t!("develop-smoothing"), 0.005, 0.1, 0.025)
                        .steps(0.001, 0.01)
                        .unit(Unit::Percent, 1),
                )])
                .collect(),
        },
        card(
            ModuleKind::LOCAL_CONTRAST,
            false,
            vec![
                s(
                    id::TEXTURE,
                    slider(t!("develop-texture"), -1.0, 1.0, 0.0)
                        .steps(0.01, 0.1)
                        .unit(Unit::Percent, 0)
                        .signed(),
                ),
                s(
                    id::CLARITY,
                    slider(t!("develop-clarity"), -1.0, 1.0, 0.0)
                        .steps(0.01, 0.1)
                        .unit(Unit::Percent, 0)
                        .signed(),
                ),
            ],
        ),
        card(
            ModuleKind::COLOUR_BALANCE,
            false,
            vec![
                s(
                    id::VIBRANCE,
                    slider(t!("develop-vibrance"), -1.0, 1.0, 0.0)
                        .steps(0.01, 0.1)
                        .unit(Unit::Percent, 0)
                        .signed(),
                ),
                s(
                    id::SATURATION,
                    slider(t!("develop-saturation"), -1.0, 1.0, 0.0)
                        .steps(0.01, 0.1)
                        .unit(Unit::Percent, 0)
                        .signed(),
                ),
            ],
        ),
        // The same module's wheels, with chroma and brilliance (the finer colour controls).
        Card {
            id: GRADING_CARD,
            title: t!("develop-module-grading"),
            enabled: None,
            expanded: false,
            controls: vec![
                Control {
                    id: id::WHEELS,
                    body: Body::Wheels(wheels),
                },
                s(
                    id::CHROMA,
                    slider(t!("develop-chroma"), -1.0, 1.0, 0.0)
                        .steps(0.01, 0.1)
                        .unit(Unit::Percent, 0)
                        .signed(),
                ),
                s(
                    id::BRILLIANCE,
                    slider(t!("develop-brilliance"), -1.0, 1.0, 0.0)
                        .steps(0.01, 0.1)
                        .unit(Unit::Percent, 0)
                        .signed(),
                ),
            ],
        },
        card(
            ModuleKind::CURVES,
            false,
            vec![
                Control {
                    id: id::CURVE,
                    body: Body::Curve(Curve::identity(t!("develop-curve"))),
                },
                s(
                    id::LEVELS_BLACK,
                    slider(t!("develop-levels-black"), -0.2, 0.5, 0.0)
                        .steps(0.005, 0.05)
                        .unit(Unit::Plain, 3)
                        .signed(),
                ),
                s(
                    id::LEVELS_MID,
                    slider(t!("develop-levels-mid"), 0.2, 0.8, 0.5)
                        .steps(0.005, 0.05)
                        .unit(Unit::Plain, 3),
                ),
                s(
                    id::LEVELS_WHITE,
                    slider(t!("develop-levels-white"), 0.6, 1.5, 1.0)
                        .steps(0.005, 0.05)
                        .unit(Unit::Plain, 3),
                ),
            ],
        ),
        Card {
            id: HDR_CARD,
            title: t!("develop-hdr"),
            enabled: None,
            expanded: true,
            controls: vec![
                s(
                    id::HDR_LIMIT,
                    slider(
                        t!("develop-hdr-limit"),
                        0.0,
                        specular_develop::ops::view::MAX_HDR_LIMIT_EV,
                        2.0,
                    )
                    .steps(0.1, 0.5)
                    .unit(Unit::Ev, 1),
                ),
                Control {
                    id: id::SDR_PREVIEW,
                    body: Body::Choice(Choice::new(
                        t!("develop-hdr-preview"),
                        vec![t!("develop-hdr-preview-hdr"), t!("develop-hdr-preview-sdr")],
                    )),
                },
            ],
        },
    ];
    // The colour cards after the colour wheels, the LUT before the view transform.
    let at = |v: &[Card], kind: ModuleKind| v.iter().position(|c| c.id == u32::from(kind.0));
    let i = at(&v, ModuleKind::COLOUR_BALANCE).map_or(v.len(), |i| i + 1);
    v.splice(i..i, colour_cards());
    let i = v.iter().position(|c| c.id == HDR_CARD).unwrap_or(v.len());
    v.insert(i, lut_card());
    // The Detail card after the LUT.
    v.insert(i + 1, detail_card());
    // The Effects cards after it.
    v.splice(i + 2..i + 2, super::effects::cards());
    // Depth blur and relight before the effects.
    v.splice(i + 2..i + 2, super::depth_tools::cards());
    // Surface blur, then diffuse or sharpen, after the Detail card.
    v.splice(i + 2..i + 2, super::diffuse::cards());
    // The Corrections card after the lens corrections.
    let i = at(&v, ModuleKind::LENS).map_or(0, |i| i + 1);
    v.insert(i, corrections_card());
    // Filled by `Session::sync_tools`.
    v.push(Card {
        id: TOOLS_CARD,
        title: t!("develop-add-tool"),
        enabled: None,
        expanded: false,
        controls: Vec::new(),
    });
    v
}

/// `value` (a percentage) as Develop's sliders write it in the UI language ("12.5%",
/// "12,5 %", "%12,5"): the locale's digits, separators and percent pattern
/// ([`specular_i18n::Localizer::percent_text`]).
pub(super) fn percent_text(value: f64, decimals: u8) -> String {
    let l = specular_i18n::I18n::global().localizer();
    l.percent_text(&l.number(value, decimals))
}

fn percent(id: u32, label: String, min: f32, max: f32, default: f32) -> Control {
    let mut spec = slider(label, min, max, default)
        .steps(0.01, 0.1)
        .unit(Unit::Percent, 0);
    if min < 0.0 {
        spec = spec.signed();
    }
    Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    }
}

fn degrees(id: u32, label: String, limit: f32) -> Control {
    Control {
        id,
        body: Body::Slider(Slider::new(
            slider(label, -limit, limit, 0.0)
                .steps(1.0, 5.0)
                .unit(Unit::Degrees, 0)
                .signed(),
        )),
    }
}

/// A module card, closed at first.
fn closed_card(kind: ModuleKind, controls: Vec<Control>) -> Card {
    Card {
        id: u32::from(kind.0),
        title: module_title(kind),
        enabled: Some(kind != ModuleKind::BLACK_WHITE),
        expanded: false,
        controls,
    }
}

/// The colour mixer, point colour, and black and white cards.
fn colour_cards() -> Vec<Card> {
    let mut mixer = Vec::new();
    for i in 0..BANDS.len() {
        mixer.push(degrees(
            id::MIXER_HUE + i as u32,
            band_label("hue", i),
            45.0,
        ));
    }
    for (kind, base) in [
        ("saturation", id::MIXER_SATURATION),
        ("brightness", id::MIXER_BRIGHTNESS),
    ] {
        for i in 0..BANDS.len() {
            mixer.push(percent(
                base + i as u32,
                band_label(kind, i),
                -1.0,
                1.0,
                0.0,
            ));
        }
    }
    let point = vec![
        Control {
            id: id::POINT_PICK,
            body: Body::Choice(Choice::button(t!("develop-point-pick"))),
        },
        degrees(id::POINT_HUE, t!("develop-point-hue"), 60.0),
        percent(
            id::POINT_SATURATION,
            t!("develop-point-saturation"),
            -1.0,
            1.0,
            0.0,
        ),
        percent(
            id::POINT_LIGHTNESS,
            t!("develop-point-lightness"),
            -1.0,
            1.0,
            0.0,
        ),
        percent(id::POINT_RANGE, t!("develop-point-range"), 0.0, 1.0, 0.5),
        percent(
            id::POINT_HUE_RANGE,
            t!("develop-point-hue-range"),
            0.0,
            1.0,
            0.5,
        ),
        percent(
            id::POINT_SATURATION_RANGE,
            t!("develop-point-saturation-range"),
            0.0,
            1.0,
            0.5,
        ),
        percent(
            id::POINT_LIGHTNESS_RANGE,
            t!("develop-point-lightness-range"),
            0.0,
            1.0,
            0.5,
        ),
    ];
    let bw = (0..BANDS.len())
        .map(|i| percent(id::BW_MIX + i as u32, band_label("mix", i), -1.0, 1.0, 0.0))
        .collect();
    vec![
        // No switch, like the other basic cards: moving a band turns it on.
        Card {
            enabled: None,
            ..closed_card(ModuleKind::COLOUR_EQUALIZER, mixer)
        },
        closed_card(ModuleKind::POINT_COLOUR, point),
        closed_card(ModuleKind::BLACK_WHITE, bw),
    ]
}

/// The LUT card.
fn lut_card() -> Card {
    let spaces = LUT_SPACES
        .iter()
        .map(|s| match s {
            LutSpace::LinearRec2020 => t!("develop-lut-space-linear"),
            LutSpace::Acescg => t!("develop-lut-space-acescg"),
            LutSpace::Acescct => t!("develop-lut-space-acescct"),
            LutSpace::Srgb => t!("develop-lut-space-srgb"),
            LutSpace::Rec709 => t!("develop-lut-space-rec709"),
        })
        .collect();
    closed_card(
        ModuleKind::LUT_3D,
        vec![
            Control {
                id: id::LUT_FILE,
                body: Body::Choice(Choice::button(t!("develop-lut-choose"))),
            },
            Control {
                id: id::LUT_SPACE,
                body: Body::Choice(Choice::new(t!("develop-lut-space"), spaces)),
            },
            Control {
                id: id::LUT_INTERPOLATION,
                body: Body::Choice(Choice::new(
                    t!("develop-lut-interpolation"),
                    vec![t!("develop-lut-tetrahedral"), t!("develop-lut-trilinear")],
                )),
            },
            percent(id::LUT_STRENGTH, t!("develop-lut-strength"), 0.0, 1.0, 1.0),
        ],
    )
}

/// The Detail card: noise reduction, sharpening and capture sharpening.
fn detail_card() -> Card {
    let s = |id: u32, spec: SliderSpec| Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    };
    let pct = |label: String, max: f32, default: f32| {
        slider(label, 0.0, max, default)
            .steps(0.01, 0.1)
            .unit(Unit::Percent, 0)
    };
    let px = |label: String, min: f32, default: f32| {
        slider(label, min, 3.0, default)
            .steps(0.05, 0.25)
            .unit(Unit::Plain, 2)
    };
    Card {
        id: DETAIL_CARD,
        title: t!("develop-module-detail"),
        enabled: None,
        expanded: false,
        controls: vec![
            Control {
                id: id::DEMOSAIC,
                body: Body::Choice(Choice::new(
                    t!("develop-demosaic"),
                    vec![
                        t!("develop-demosaic-directional"),
                        t!("develop-demosaic-dual"),
                        t!("develop-demosaic-fast"),
                    ],
                )),
            },
            percent(
                id::DUAL_THRESHOLD,
                t!("develop-dual-threshold"),
                0.0,
                1.0,
                RawFrontEnd::default().dual_threshold,
            ),
            s(
                id::NOISE_LUMINANCE,
                pct(t!("develop-noise-luminance"), 1.0, 0.0),
            ),
            s(id::NOISE_DETAIL, pct(t!("develop-noise-detail"), 1.0, 0.5)),
            s(id::NOISE_COLOUR, pct(t!("develop-noise-colour"), 1.0, 0.0)),
            // The AI mode (its share, status and button only while it is on).
            Control {
                id: id::NOISE_MODE,
                body: Body::Choice(Choice::new(
                    t!("develop-noise-mode"),
                    vec![
                        t!("develop-noise-mode-classic"),
                        t!("develop-noise-mode-ai"),
                    ],
                )),
            },
            s(
                id::SHARPEN_AMOUNT,
                pct(t!("develop-sharpen-amount"), 2.0, 0.0),
            ),
            s(
                id::SHARPEN_RADIUS,
                px(t!("develop-sharpen-radius"), 0.3, 1.0),
            ),
            s(
                id::SHARPEN_DETAIL,
                pct(t!("develop-sharpen-detail"), 1.0, 0.25),
            ),
            s(
                id::SHARPEN_MASKING,
                pct(t!("develop-sharpen-masking"), 1.0, 0.0),
            ),
            s(
                id::CAPTURE_AMOUNT,
                pct(t!("develop-capture-amount"), 1.0, 0.0),
            ),
            s(
                id::CAPTURE_RADIUS,
                px(t!("develop-capture-radius"), 0.0, 0.0),
            ),
        ],
    }
}

/// Denoise's AI rows: the AI share, the status line and the button; on the Detail
/// card after the mode only while the mode is AI ([`Session::sync_panel`]).
fn denoise_ai_controls() -> Vec<Control> {
    vec![
        Control {
            id: id::NOISE_BLEND,
            body: Body::Slider(Slider::new(
                slider(
                    t!("develop-noise-ai-blend"),
                    0.0,
                    1.0,
                    specular_develop::ops::detail::ai::DEFAULT_BLEND,
                )
                .steps(0.01, 0.1)
                .unit(Unit::Percent, 0),
            )),
        },
        Control {
            id: id::NOISE_AI_NOTE,
            body: Body::Note(String::new()),
        },
        Control {
            id: id::NOISE_AI_ACTION,
            body: Body::Choice(Choice::button(String::new())),
        },
    ]
}

/// The panel's cards for a photo, in [`task_group`] order: other images get no raw-only
/// rows (demosaic) and no Corrections card (highlights, hot pixels, calibration frames).
fn cards_for(path: &Path) -> Vec<Card> {
    let mut v = cards();
    // Retouch after Corrections (both repair the photo before its look).
    let at = v
        .iter()
        .position(|c| c.id == CORRECTIONS_CARD)
        .map_or(v.len(), |i| i + 1);
    v.insert(at, super::retouch::card());
    // Stable: a group's cards keep their pipeline order.
    v.sort_by_key(|c| task_group(c.id));
    if is_raw(path) {
        // Resetting a slider goes back to what a raw starts with.
        for ctl in v.iter_mut().flat_map(|c| c.controls.iter_mut()) {
            if let (Some(d), Body::Slider(s)) = (raw_default(ctl.id), &mut ctl.body) {
                s.spec.default = d;
            }
        }
    } else {
        for c in v.iter_mut().filter(|c| c.id == DETAIL_CARD) {
            c.controls
                .retain(|k| !matches!(k.id, id::DEMOSAIC | id::DUAL_THRESHOLD));
        }
        // The camera profile is the raw front end's.
        for c in v.iter_mut() {
            c.controls
                .retain(|k| !matches!(k.id, id::INPUT_PROFILE | id::INPUT_PROFILE_NOTE));
        }
        // Every Corrections row works on the raw sensor data.
        v.retain(|c| c.id != CORRECTIONS_CARD);
    }
    v
}

/// Where card `card` goes in the panel: by task rather than by where it runs in the pipeline.
/// What most photos need comes first, in Lightroom's order (Light with the profile, white
/// balance, then tone, colour and detail); the repairs (crop, optics, corrections, retouch)
/// and the effects follow, and the output (LUT, HDR) is last. A card not named here goes
/// with the effects.
fn task_group(card: u32) -> u8 {
    const FIRST: [&[u32]; 6] = [
        // Light (the profile, exposure, contrast …).
        &[ModuleKind::EXPOSURE.0 as u32],
        // White balance.
        &[ModuleKind::COLOUR_CALIBRATION.0 as u32],
        // Tone.
        &[
            ZONES_CARD,
            ModuleKind::LOCAL_CONTRAST.0 as u32,
            ModuleKind::CURVES.0 as u32,
        ],
        // Colour.
        &[
            ModuleKind::COLOUR_BALANCE.0 as u32,
            ModuleKind::COLOUR_EQUALIZER.0 as u32,
            ModuleKind::POINT_COLOUR.0 as u32,
            ModuleKind::BLACK_WHITE.0 as u32,
            GRADING_CARD,
        ],
        // Detail.
        &[
            DETAIL_CARD,
            ModuleKind::SURFACE_BLUR.0 as u32,
            ModuleKind::DIFFUSE.0 as u32,
        ],
        // Repairs.
        &[
            ModuleKind::CROP_ORIENTATION.0 as u32,
            ModuleKind::PERSPECTIVE.0 as u32,
            ModuleKind::LENS.0 as u32,
            CORRECTIONS_CARD,
            ModuleKind::RETOUCH.0 as u32,
        ],
    ];
    const OUTPUT: [u32; 2] = [ModuleKind::LUT_3D.0 as u32, HDR_CARD];
    if card == TOOLS_CARD {
        FIRST.len() as u8 + 2
    } else if let Some(i) = FIRST.iter().position(|g| g.contains(&card)) {
        i as u8
    } else if OUTPUT.contains(&card) {
        FIRST.len() as u8 + 1
    } else {
        FIRST.len() as u8
    }
}

/// What a card's changed dot watches.
enum Watch {
    /// A module: its settings, whether it is on, its instances and masks.
    All(ModuleKind),
    /// Only these fields of a module that other cards share.
    Fields(ModuleKind, &'static [&'static str]),
    /// A module but these fields (another card's).
    Except(ModuleKind, &'static [&'static str]),
}

impl Watch {
    fn kind(&self) -> ModuleKind {
        match *self {
            Watch::All(k) | Watch::Fields(k, _) | Watch::Except(k, _) => k,
        }
    }
}

/// What the changed dot of card `card` watches (nothing for Auto exposure: its switch says
/// all there is).
fn watched(card: u32) -> Vec<Watch> {
    use ModuleKind as K;
    use Watch::{All, Except, Fields};
    const DEMOSAIC: &[&str] = &["demosaic", "dual_threshold"];
    const HIGHLIGHTS: &[&str] = &["highlights", "highlight_threshold"];
    const HDR: &[&str] = &["hdr_limit_ev"];
    const ZONES: &[&str] = &["zones", "smoothing"];
    const BLACK_LEVEL: &[&str] = &["black_offset"];
    const LIGHT_TONE: &[&str] = &["highlights_ev", "shadows_ev"];
    const LIGHT_VIEW: &[&str] = &["method", "contrast", "white_target", "black_target"];
    const GRADING: &[&str] = &[
        "shadows_hue",
        "shadows_chroma",
        "shadows_luminance",
        "midtones_hue",
        "midtones_chroma",
        "midtones_luminance",
        "highlights_hue",
        "highlights_chroma",
        "highlights_luminance",
        "global_hue",
        "global_chroma",
        "global_luminance",
        "chroma",
        "brilliance",
    ];
    match card {
        DETAIL_CARD => vec![
            All(K::DENOISE),
            All(K::SHARPEN),
            All(K::CAPTURE_SHARPENING),
            Fields(K::RAW_FRONT_END, DEMOSAIC),
        ],
        CORRECTIONS_CARD => vec![
            All(K::HOT_PIXELS),
            All(K::FLAT_FIELD),
            Fields(K::RAW_FRONT_END, HIGHLIGHTS),
            Fields(K::EXPOSURE, BLACK_LEVEL),
        ],
        c if c == u32::from(K::LENS.0) => {
            vec![All(K::LENS), All(K::CHROMATIC_ABERRATION), All(K::DEFRINGE)]
        }
        HDR_CARD => vec![Fields(K::VIEW_TRANSFORM, HDR)],
        ZONES_CARD => vec![Fields(K::TONE_EQUALIZER, ZONES)],
        c if c == u32::from(K::EXPOSURE.0) => vec![
            Except(K::EXPOSURE, BLACK_LEVEL),
            Fields(K::TONE_EQUALIZER, LIGHT_TONE),
            Fields(K::VIEW_TRANSFORM, LIGHT_VIEW),
        ],
        // Colour holds the module's masks and instances; Colour grading only its fields.
        c if c == u32::from(K::COLOUR_BALANCE.0) => vec![Except(K::COLOUR_BALANCE, GRADING)],
        GRADING_CARD => vec![Fields(K::COLOUR_BALANCE, GRADING)],
        c if c < 0x1_0000 => vec![All(ModuleKind(c as u16))],
        _ => Vec::new(),
    }
}

/// `m`'s fields, less what reading the photo sets rather than the user: the camera's
/// as-shot white (and the white balance while it follows it).
fn comparable(m: &ModuleInstance) -> Fields {
    let mut f = m.params.to_fields();
    if m.kind == ModuleKind::COLOUR_CALIBRATION {
        f.remove("as_shot_x");
        f.remove("as_shot_y");
        if f.text("white_source") == Some("as_shot") {
            f.remove("white_x");
            f.remove("white_y");
        }
    }
    f
}

/// Light: the profile (the look's tone curve), then Lightroom's six, in its order (exposure,
/// contrast, highlights, shadows, whites, blacks), and Auto. Three modules drive them
/// (exposure, the tone equalizer, the view transform), so the card has no switch; moving a
/// slider turns its module on.
/// Everything but exposure is on Lightroom's scale ([`light_scale`]).
fn light_card() -> Card {
    let s = |id: u32, spec: SliderSpec| Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    };
    let lr = |id: u32, label: String| {
        s(
            id,
            slider(label, -100.0, 100.0, 0.0)
                .steps(1.0, 10.0)
                .unit(Unit::Plain, 0)
                .signed(),
        )
    };
    Card {
        id: u32::from(ModuleKind::EXPOSURE.0),
        title: t!("develop-module-light"),
        enabled: None,
        expanded: true,
        controls: vec![
            // The view transform's method, named for how it looks.
            Control {
                id: id::VIEW_METHOD,
                body: Body::Choice(Choice::new(
                    t!("develop-look-profile"),
                    vec![
                        t!("develop-look-standard"),
                        t!("develop-look-film"),
                        t!("develop-look-natural"),
                    ],
                )),
            },
            Control {
                id: id::AUTO_EXPOSURE,
                body: Body::Choice(Choice::new(
                    t!("develop-auto-exposure"),
                    vec![t!("develop-off"), t!("develop-on")],
                )),
            },
            s(
                id::EXPOSURE,
                slider(t!("develop-exposure"), -5.0, 5.0, 0.0)
                    .steps(0.05, 0.5)
                    .unit(Unit::Ev, 2)
                    .signed(),
            ),
            lr(id::CONTRAST, t!("develop-contrast")),
            lr(id::HIGHLIGHTS, t!("develop-highlights")),
            lr(id::SHADOWS, t!("develop-shadows")),
            lr(id::WHITES, t!("develop-whites")),
            lr(id::BLACKS, t!("develop-blacks")),
        ],
    }
}

/// Exposure's black level: a correction of the sensor's black, so in Corrections (raw files),
/// or under Light while another file's edit uses it ([`Session::sync_black_level`]).
fn black_level() -> Control {
    Control {
        id: id::BLACK_LEVEL,
        body: Body::Slider(Slider::new(
            slider(t!("develop-black-level"), -0.05, 0.05, 0.0)
                .steps(0.001, 0.01)
                .unit(Unit::Plain, 3)
                .signed(),
        )),
    }
}

/// A Light slider's field values at −100, 0 and +100: Lightroom's scale, 0 at the field's
/// neutral value and each side linear to its end (the fields keep the engine's units).
fn light_scale(control: u32) -> Option<(f32, f32, f32)> {
    Some(match control {
        id::CONTRAST => (0.5, 1.5, 3.0),
        id::HIGHLIGHTS | id::SHADOWS => (-4.0, 0.0, 4.0),
        id::WHITES => (0.5, 1.0, 2.0),
        id::BLACKS => (-0.1, 0.0, 0.1),
        _ => return None,
    })
}

/// A field value on its Light slider (−100 … +100).
fn to_light((lo, mid, hi): (f32, f32, f32), v: f32) -> f32 {
    let u = if v >= mid {
        (v - mid) / (hi - mid)
    } else {
        (v - mid) / (mid - lo)
    };
    (u * 100.0).clamp(-100.0, 100.0)
}

/// A Light slider's value (−100 … +100) as its field's.
fn from_light((lo, mid, hi): (f32, f32, f32), u: f32) -> f32 {
    let u = u.clamp(-100.0, 100.0) / 100.0;
    if u >= 0.0 {
        mid + u * (hi - mid)
    } else {
        mid + u * (mid - lo)
    }
}

/// The Optics card (the Lens corrections card's place and id): what the lens did, whatever
/// the sensor. The lens profile's switch (an Off/On row, as Hot pixels has: the card holds
/// more than one module, so it has no switch of its own), manual distortion and vignetting,
/// chromatic aberration and defringe. The lens module's own red and blue fringe sliders do
/// what Chromatic aberration's Red and Blue shift do, so they show only for an edit that
/// already uses them ([`Session::sync_optics`]).
fn optics_card() -> Card {
    let s = |id: u32, spec: SliderSpec| Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    };
    let choice = |id: u32, label: String, options: Vec<String>| Control {
        id,
        body: Body::Choice(Choice::new(label, options)),
    };
    let hue = |id: u32, label: String, default: f32| {
        s(
            id,
            slider(label, 0.0, 360.0, default)
                .steps(1.0, 10.0)
                .unit(Unit::Degrees, 0),
        )
    };
    let ca = |id: u32, label: String| {
        s(
            id,
            slider(label, -0.5, 0.5, 0.0)
                .steps(0.005, 0.05)
                .unit(Unit::Plain, 3)
                .signed(),
        )
    };
    Card {
        id: u32::from(ModuleKind::LENS.0),
        title: t!("develop-module-optics"),
        enabled: None,
        expanded: false,
        controls: vec![
            choice(
                id::LENS_ON,
                module_title(ModuleKind::LENS),
                vec![t!("develop-off"), t!("develop-on")],
            ),
            s(
                id::LENS_DISTORTION,
                slider(t!("develop-lens-distortion"), -100.0, 100.0, 0.0)
                    .steps(1.0, 10.0)
                    .unit(Unit::Plain, 0)
                    .signed(),
            ),
            s(
                id::LENS_VIGNETTE,
                slider(t!("develop-lens-vignette"), -3.0, 3.0, 0.0)
                    .steps(0.05, 0.5)
                    .unit(Unit::Ev, 2)
                    .signed(),
            ),
            choice(
                id::CA_MODE,
                t!("develop-ca-mode"),
                vec![
                    t!("develop-off"),
                    t!("develop-ca-auto"),
                    t!("develop-ca-manual"),
                ],
            ),
            ca(id::CA_RED, t!("develop-ca-red")),
            ca(id::CA_BLUE, t!("develop-ca-blue")),
            percent(id::DEFRINGE, t!("develop-defringe"), 0.0, 1.0, 0.0),
            hue(id::DEFRINGE_PURPLE, t!("develop-defringe-purple"), 320.0),
            hue(id::DEFRINGE_GREEN, t!("develop-defringe-green"), 140.0),
            s(
                id::DEFRINGE_RANGE,
                slider(t!("develop-defringe-range"), 10.0, 90.0, 40.0)
                    .steps(1.0, 5.0)
                    .unit(Unit::Degrees, 0),
            ),
            s(
                id::DEFRINGE_EDGE,
                slider(t!("develop-defringe-edge"), 0.5, 6.0, 2.0)
                    .steps(0.1, 0.5)
                    .unit(Unit::Ev, 1),
            ),
        ],
    }
}

/// The lens module's red and blue fringe sliders, which [`optics_card`] shows only while
/// the edit uses them.
fn lens_fringe_controls() -> Vec<Control> {
    [
        (id::LENS_TCA_RED, t!("develop-lens-tca-red")),
        (id::LENS_TCA_BLUE, t!("develop-lens-tca-blue")),
    ]
    .into_iter()
    .map(|(id, label)| Control {
        id,
        body: Body::Slider(Slider::new(
            slider(label, -0.5, 0.5, 0.0)
                .steps(0.005, 0.05)
                .unit(Unit::Plain, 3)
                .signed(),
        )),
    })
    .collect()
}

/// The Corrections card: what the sensor got wrong, on raws before demosaic:
/// highlights, hot pixels, flat field and dark frame. Other images have no Corrections card
/// ([`cards_for`]); chromatic aberration and defringe are on the Optics card.
fn corrections_card() -> Card {
    let choice = |id: u32, label: String, options: Vec<String>| Control {
        id,
        body: Body::Choice(Choice::new(label, options)),
    };
    let off_on = vec![t!("develop-off"), t!("develop-on")];
    Card {
        id: CORRECTIONS_CARD,
        title: t!("develop-module-corrections"),
        enabled: None,
        expanded: false,
        controls: vec![
            black_level(),
            choice(
                id::HL_METHOD,
                t!("develop-highlight-method"),
                vec![
                    t!("develop-highlight-opposed"),
                    t!("develop-highlight-rebuild"),
                    t!("develop-highlight-clip"),
                ],
            ),
            percent(
                id::HL_THRESHOLD,
                t!("develop-highlight-threshold"),
                0.5,
                1.0,
                1.0,
            ),
            choice(id::HOT, t!("develop-module-hot-pixels"), off_on),
            percent(
                id::HOT_THRESHOLD,
                t!("develop-hot-threshold"),
                0.25,
                4.0,
                specular_develop::ops::correct::HOT_DEFAULT,
            ),
            choice(
                id::FLAT_MODE,
                t!("develop-calibration"),
                vec![
                    t!("develop-off"),
                    t!("develop-calibration-auto"),
                    t!("develop-calibration-chosen"),
                ],
            ),
            Control {
                id: id::FLAT_FILE,
                body: Body::Choice(Choice::button(t!("develop-flat-choose"))),
            },
            Control {
                id: id::DARK_FILE,
                body: Body::Choice(Choice::button(t!("develop-dark-choose"))),
            },
            choice(
                id::FLAT_USE,
                t!("develop-flat-use"),
                vec![
                    t!("develop-flat-use-full"),
                    t!("develop-flat-use-vignetting"),
                ],
            ),
        ],
    }
}

/// The temperature slider's default: D50, the colour calibration module's default white.
pub(super) const DEFAULT_KELVIN: f32 = 5000.0;

/// What a session asks the application to do after events.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Request {
    /// The picture must be developed again.
    Render,
    Leave,
    BeforeAfter,
    /// Open Export for the photo.
    Export,
    /// Say this to screen readers.
    Say(String),
    /// The next click on the picture picks point colour's colour.
    Pick,
    /// Open the file dialog for a LUT file.
    ChooseLut,
    /// Open the file dialog for a camera profile (`.dcp`).
    ChooseInputProfile,
    /// Start "Make profile from chart…".
    ChartProfile,
    /// Open the file dialog for a flat frame (false) or a dark frame (true).
    ChooseFrame(bool),
    /// Ask before going on: the question and the button that goes on.
    Confirm(String, String),
    /// The next click on the picture picks a colour at this module's input (a colour-range
    /// mask).
    PickFor(ModuleKind),
    /// Ask for a name and save this module instance's mask as a preset.
    SaveMaskPreset(ModuleKind, u16),
    /// Put mask preset `n` (in the list) on this module instance.
    ApplyMaskPreset(ModuleKind, u16, usize),
    /// Delete mask preset `n` (asked first).
    DeleteMaskPreset(usize),
    /// Ask for the watermark's text, starting from this.
    WatermarkText(String),
    /// Open the file dialog for a watermark SVG.
    ChooseWatermark,
    /// Open Settings › Local AI at a feature's setting (an AI mask's or denoise's model is
    /// missing).
    OpenLocalAi(&'static str),
    /// Denoise's AI button: stop, open Local AI or try again.
    DenoiseAction,
    /// A card's ⋯ button: its menu (the one a right-click on `item` opens) at `at`.
    Menu(Item, (f32, f32)),
    /// Run automatic upright in this mode on the picture ([`super::perspective`]).
    Upright(specular_develop::ops::geometry::persp::Mode),
    /// Open the crop tool on the perspective guides.
    CropGuides,
    /// A row of the Crop card: aspect, a typed ratio, swap, turn or flip (`crop::card`).
    CropCard(u32),
}

/// One image open in Develop.
#[derive(Debug)]
pub(super) struct Session {
    pub path: PathBuf,
    pub edit: Edit,
    pub history: History,
    pub panel: DevelopPanel,
    /// The straightened frame's size (sensor pixels) once the picture is shown: the
    /// Straighten slider keeps the crop inside the photo with it.
    pub frame: Option<(f64, f64)>,
    /// Show the SDR rendition on an HDR display (the base of a gain-map export).
    pub sdr_preview: bool,
    /// Masks and instances ([`super::masks`]).
    pub masks: super::masks::State,
    /// Retouch spots ([`super::retouch`]).
    pub retouch_state: super::retouch::State,
    /// The next pick is the negative's film base.
    pub picking_base: bool,
    /// The graduated ND's handles are on the picture.
    pub nd_edit: bool,
    /// The graduated ND's fields when its drag began.
    pub nd_before: Option<Fields>,
    /// Borders and watermark drawn on the picture (a view, not the edit).
    pub frame_preview: bool,
    /// AI mask rows: the photo they are keyed for, the models, each row's state.
    pub ai: super::ai_masks::SessionAi,
    /// Denoise's AI mode.
    pub denoise: super::ai_denoise::SessionDenoise,
    /// Dragging on the picture raises or lowers the tone equalizer zone under the pointer.
    pub zone_edit: bool,
    /// The zone drag in progress ([`super::zones`]).
    pub zone_drag: Option<super::zones::Drag>,
    /// Removal patches' and the depth map's states (mirrored from the worker).
    pub ai2_state: std::collections::HashMap<[u8; 32], super::ai_remove::NetState>,
    /// Stop or Compute again, for the worker's next tick.
    pub ai2_request: Option<super::ai_remove::Ask>,
    /// The next click on the picture sets the depth blur's focus.
    pub focus_picking: bool,
    /// A fresh edit of this photo: a card whose settings differ from it gets the changed dot
    /// ([`DevelopPanel::changed`]).
    fresh: Edit,
    /// The [`TOOLS`] shown (by card id): added, or used by the edit at some point.
    tools: Vec<u32>,
    /// A raw's Lensfun lookup (Phase 3 item 11): the Optics card's profile rows.
    pub lensfun: Option<std::sync::Arc<specular_develop::ops::geometry::lensfun::Shot>>,
    /// Perspective: what waits for a yes, the picture's keystone, a run in progress.
    pub persp: super::perspective::State,
}

impl Session {
    pub fn new(path: PathBuf, edit: Edit, history: History) -> Session {
        let mut edit = edit;
        ensure_modules(&mut edit, &path);
        let cards = cards_for(&path);
        let fresh = new_edit(&path);
        let mut s = Session {
            path,
            edit,
            history,
            panel: DevelopPanel::new(t!("develop-title"), cards),
            frame: None,
            sdr_preview: false,
            masks: Default::default(),
            retouch_state: Default::default(),
            picking_base: false,
            nd_edit: false,
            nd_before: None,
            frame_preview: false,
            ai: Default::default(),
            denoise: Default::default(),
            zone_edit: false,
            zone_drag: None,
            ai2_state: Default::default(),
            ai2_request: None,
            focus_picking: false,
            fresh,
            tools: Vec::new(),
            lensfun: None,
            persp: Default::default(),
        };
        s.sync_panel();
        s
    }

    fn fields(&self, kind: ModuleKind) -> Fields {
        self.edit
            .module(kind, self.inst(kind))
            .map(|m| m.params.to_fields())
            .unwrap_or_default()
    }

    /// The panel's values, switches and history from the edit.
    pub fn sync_panel(&mut self) {
        let mut cards = std::mem::take(&mut self.panel.cards);
        for c in &mut cards {
            let kind = ModuleKind(c.id as u16);
            if let (Some(on), Some(m)) =
                (c.enabled.as_mut(), self.edit.module(kind, self.inst(kind)))
            {
                *on = m.enabled;
            }
            for ctl in &mut c.controls {
                self.read_control(ctl);
            }
        }
        self.sync_masks(&mut cards);
        self.sync_denoise(&mut cards);
        self.sync_optics(&mut cards);
        self.sync_lensfun(&mut cards);
        self.sync_black_level(&mut cards);
        for c in cards.iter_mut().filter(|c| c.id == DETAIL_CARD) {
            for ctl in c.controls.iter_mut().filter(|k| {
                matches!(
                    k.id,
                    id::NOISE_BLEND | id::NOISE_AI_NOTE | id::NOISE_AI_ACTION
                )
            }) {
                self.read_control(ctl);
            }
        }
        self.retouch_after_history();
        self.sync_retouch(&mut cards);
        self.sync_depth(&mut cards);
        self.panel.cards = cards;
        self.panel.menus = self
            .panel
            .cards
            .iter()
            .map(|c| c.id)
            .filter(|&id| super::batch::has_menu(id))
            .collect();
        let changed = (self.panel.cards.iter())
            .map(|c| c.id)
            .filter(|&id| watched(id).iter().any(|w| self.differs(w)))
            .collect();
        self.panel.changed = changed;
        self.sync_tools();
        self.panel.history = self.history_rows();
        self.panel.history_at = self.history.position;
        self.panel.can_undo = self.history.can_undo();
        self.panel.can_redo = self.history.can_redo();
        self.sync_dims();
    }

    /// Whether what `w` watches differs from a fresh edit of the photo.
    fn differs(&self, w: &Watch) -> bool {
        let kind = w.kind();
        let mut now = self.edit.modules.iter().filter(|m| m.kind == kind);
        let Some(m) = now.next() else {
            return false;
        };
        let whole = !matches!(w, Watch::Fields(..));
        // A second instance, or a mask: a change to the module as a whole.
        if whole && (now.next().is_some() || m.blend.is_some() || m.instance != 0) {
            return true;
        }
        let made;
        let base = match self.fresh.module(kind, 0) {
            Some(b) => b,
            None => {
                made = ModuleInstance::new(Registry::builtin(), kind, 0);
                &made
            }
        };
        let (mut a, mut b) = (comparable(m), comparable(base));
        match w {
            Watch::All(_) => m.enabled != base.enabled || a != b,
            Watch::Fields(_, names) => names.iter().any(|n| a.get(n) != b.get(n)),
            Watch::Except(_, names) => {
                for n in *names {
                    a.remove(n);
                    b.remove(n);
                }
                m.enabled != base.enabled || a != b
            }
        }
    }

    /// The lens module's red and blue fringe rows on the Optics card while the edit uses
    /// them (after Chromatic aberration's Red and Blue shift, which do the same); read by
    /// [`Session::sync_panel`] after.
    fn sync_optics(&self, cards: &mut [Card]) {
        let lens = self.fields(ModuleKind::LENS);
        let used = ["tca_red", "tca_blue"]
            .iter()
            .any(|n| lens.f32_or(n, 0.0) != 0.0);
        for c in cards
            .iter_mut()
            .filter(|c| c.id == u32::from(ModuleKind::LENS.0))
        {
            let shown = c.controls.iter().any(|k| k.id == id::LENS_TCA_RED);
            if used == shown {
                continue;
            }
            if !used {
                c.controls
                    .retain(|k| !matches!(k.id, id::LENS_TCA_RED | id::LENS_TCA_BLUE));
                continue;
            }
            let at = c
                .controls
                .iter()
                .position(|k| k.id == id::CA_BLUE)
                .map_or(c.controls.len(), |i| i + 1);
            let mut rows = lens_fringe_controls();
            // Inserted after the panel's values were read: read them here.
            for k in &mut rows {
                self.read_control(k);
            }
            c.controls.splice(at..at, rows);
        }
    }

    /// The Lensfun rows after the Lens corrections row while there is a lookup
    /// ([`super::lensfun`]), with their values.
    fn sync_lensfun(&self, cards: &mut [Card]) {
        use super::lensfun::{id as lf, rows_for};
        let setting = self
            .fields(ModuleKind::LENS)
            .text("lensfun")
            .unwrap_or("")
            .to_string();
        for c in cards
            .iter_mut()
            .filter(|c| c.id == u32::from(ModuleKind::LENS.0))
        {
            c.controls
                .retain(|k| !matches!(k.id, lf::PROFILE | lf::NOTE));
            let at = c
                .controls
                .iter()
                .position(|k| k.id == id::LENS_ON)
                .map_or(0, |i| i + 1);
            c.controls
                .splice(at..at, rows_for(self.lensfun.as_ref(), &setting));
        }
    }

    /// A raw's Lensfun lookup arrived (or none): the Optics card shows its rows.
    pub fn set_lensfun(
        &mut self,
        shot: Option<std::sync::Arc<specular_develop::ops::geometry::lensfun::Shot>>,
    ) {
        if self.lensfun != shot {
            self.lensfun = shot;
            self.sync_panel();
        }
    }

    /// Shows the [`TOOLS`] the edit uses (they stay shown), hides the rest, and puts a button
    /// for each hidden one on Add a tool (hidden too when there are none).
    fn sync_tools(&mut self) {
        let title = |s: &Self, c: u32| {
            s.panel
                .cards
                .iter()
                .find(|x| x.id == c)
                .map(|x| x.title.clone())
        };
        for k in TOOLS {
            let c = u32::from(k.0);
            if self.panel.changed.contains(&c) && !self.tools.contains(&c) {
                self.tools.push(c);
            }
        }
        let mut hidden = Vec::new();
        let mut buttons = Vec::new();
        for (i, k) in TOOLS.iter().enumerate() {
            let c = u32::from(k.0);
            let Some(name) = title(self, c).filter(|_| !self.tools.contains(&c)) else {
                continue;
            };
            hidden.push(c);
            buttons.push(Control {
                id: ADD_TOOL + i as u32,
                body: Body::Choice(Choice::button(name)),
            });
        }
        if buttons.is_empty() {
            hidden.push(TOOLS_CARD);
        }
        if let Some(c) = self.panel.card_mut(TOOLS_CARD) {
            c.controls = buttons;
        }
        self.panel.hidden = hidden;
    }

    /// Shows tool `TOOLS[i]` (Add a tool's button `i`): open, and the keyboard on its header,
    /// which scrolls it into view.
    pub(super) fn add_tool(&mut self, i: usize) {
        let Some(k) = TOOLS.get(i) else {
            return;
        };
        let c = u32::from(k.0);
        if !self.tools.contains(&c) {
            self.tools.push(c);
        }
        if let Some(card) = self.panel.card_mut(c) {
            card.expanded = true;
        }
        self.sync_tools();
        if let Some(at) = self.panel.cards.iter().position(|x| x.id == c) {
            self.panel.set_focus(Some(Item::Header(at)));
        }
    }

    /// Exposure's black level under Light while there is no Corrections card to hold it
    /// (not a raw file) and the edit uses it, so an older edit's setting stays in sight.
    fn sync_black_level(&self, cards: &mut [Card]) {
        if cards.iter().any(|c| c.id == CORRECTIONS_CARD) {
            return;
        }
        let used = self
            .fields(ModuleKind::EXPOSURE)
            .f32_or("black_offset", 0.0)
            != 0.0;
        let light = u32::from(ModuleKind::EXPOSURE.0);
        for c in cards.iter_mut().filter(|c| c.id == light) {
            let shown = c.controls.iter().any(|k| k.id == id::BLACK_LEVEL);
            if used && !shown {
                let mut row = black_level();
                self.read_control(&mut row);
                c.controls.push(row);
            } else if !used && shown {
                c.controls.retain(|k| k.id != id::BLACK_LEVEL);
            }
        }
    }

    /// Denoise's AI rows on the Detail card while the mode is AI (the button only when
    /// there is something to do); read by [`Session::sync_panel`] after.
    fn sync_denoise(&self, cards: &mut [Card]) {
        use specular_develop::ops::detail::ai::{AiParams, Mode};
        let on = AiParams::from_fields(&self.fields(ModuleKind::DENOISE)).mode == Mode::Ai;
        let button = self.denoise_button().is_some();
        for c in cards.iter_mut().filter(|c| c.id == DETAIL_CARD) {
            c.controls.retain(|k| {
                !matches!(
                    k.id,
                    id::NOISE_BLEND | id::NOISE_AI_NOTE | id::NOISE_AI_ACTION
                )
            });
            if !on {
                continue;
            }
            let at = c
                .controls
                .iter()
                .position(|k| k.id == id::NOISE_MODE)
                .map_or(c.controls.len(), |i| i + 1);
            let rows: Vec<Control> = denoise_ai_controls()
                .into_iter()
                .filter(|k| k.id != id::NOISE_AI_ACTION || button)
                .collect();
            c.controls.splice(at..at, rows);
        }
    }

    /// Puts the edit's value of a control on it.
    fn read_control(&self, ctl: &mut Control) {
        let kind = module_of(ctl.id);
        let f = self.fields(kind);
        if super::crop::card::read(&CropOrientation::from_fields(&f), ctl) {
            return;
        }
        if super::effects::read(self, &f, ctl) {
            return;
        }
        match &mut ctl.body {
            Body::Slider(s) => {
                let v = match ctl.id {
                    id::TEMPERATURE | id::TINT => {
                        let x = f.f32_or("white_x", D50[0]);
                        let y = f.f32_or("white_y", D50[1]);
                        let (k, tint) = kelvin_tint(x, y);
                        if ctl.id == id::TEMPERATURE {
                            (k / 10.0).round() * 10.0
                        } else {
                            tint.round()
                        }
                    }
                    c @ id::ZONE..=id::ZONE_LAST => super::zones::read(&f, c),
                    c => match (float_field(c), light_scale(c)) {
                        (Some((_, name)), Some(sc)) => to_light(sc, f.f32_or(&name, sc.1)),
                        (Some((_, name)), None) => f.f32_or(&name, s.spec.default),
                        (None, _) => s.spec.default,
                    },
                };
                s.set(v);
            }
            Body::Choice(c) => match ctl.id {
                super::diffuse::id::PRESET => super::diffuse::read_choice(c, &f),
                id::VIEW_METHOD => {
                    let m = f.choice_or("method", ViewMethod::Sigmoid);
                    c.set(VIEW_METHODS.iter().position(|v| *v == m).unwrap_or(0));
                }
                id::SDR_PREVIEW => {
                    c.set(usize::from(self.sdr_preview));
                }
                id::LUT_SPACE => {
                    let sp = f.choice_or("space", LutSpace::default());
                    c.set(LUT_SPACES.iter().position(|s| *s == sp).unwrap_or(0));
                }
                id::LUT_INTERPOLATION => {
                    c.set(usize::from(f.text("interpolation") == Some("trilinear")));
                }
                id::LUT_FILE => c.label = self.lut_label(),
                id::INPUT_PROFILE => {
                    let p = RawFrontEnd::from_fields(&f);
                    c.set(match p.input_profile {
                        InputProfile::Auto => 0,
                        InputProfile::Matrix => 1,
                        InputProfile::File => 2,
                    });
                    if let Some(o) = c.options.get_mut(2) {
                        *o = input_profile_file_label(&p);
                    }
                }
                id::HL_METHOD => {
                    let m = f.choice_or("highlights", Highlights::Opposed);
                    c.set(HL_METHODS.iter().position(|v| *v == m).unwrap_or(0));
                }
                id::NOISE_MODE => {
                    c.set(usize::from(
                        specular_develop::ops::detail::ai::Mode::of(&f)
                            == specular_develop::ops::detail::ai::Mode::Ai,
                    ));
                }
                id::NOISE_AI_ACTION => c.label = self.denoise_button().unwrap_or_default(),
                id::DEMOSAIC => {
                    let m = f.choice_or("demosaic", Demosaic::Directional);
                    c.set(DEMOSAICS.iter().position(|v| *v == m).unwrap_or(0));
                }
                id::HOT | id::LENS_ON => {
                    c.set(usize::from(self.module_on(module_of(ctl.id))));
                }
                id::AUTO_EXPOSURE => {
                    c.set(usize::from(f.bool_or("auto", false)));
                }
                id::CA_MODE | id::FLAT_MODE => {
                    let kind = module_of(ctl.id);
                    let auto_default = ctl.id == id::CA_MODE;
                    c.set(if !self.module_on(kind) {
                        0
                    } else if f.bool_or("auto", auto_default) {
                        1
                    } else {
                        2
                    });
                }
                id::FLAT_USE => {
                    c.set(usize::from(f.bool_or("blur", false)));
                }
                id::ZONE_DRAG => c.label = super::zones::button_label(self.zone_edit),
                id::FLAT_FILE => c.label = self.frame_label(false),
                id::DARK_FILE => c.label = self.frame_label(true),
                id::POINT_PICK => {
                    c.label = if f.bool_or("picked", false) {
                        t!("develop-point-pick-again")
                    } else {
                        t!("develop-point-pick")
                    };
                }
                _ => {}
            },
            Body::Curve(c) => {
                let pts: Vec<[f32; 2]> = match f.get("points") {
                    Some(Value::Floats(v)) => v.chunks_exact(2).map(|p| [p[0], p[1]]).collect(),
                    _ => vec![[0.0, 0.0], [1.0, 1.0]],
                };
                if !c.set_points(&pts) && pts.len() >= 2 {
                    c.set_points(&[[0.0, 0.0], [1.0, 1.0]]);
                }
            }
            Body::Wheels(ws) => {
                for (r, (w, lum)) in RANGES.iter().zip(ws.iter_mut()) {
                    w.set(
                        f.f32_or(&format!("{r}_hue"), 0.0),
                        f.f32_or(&format!("{r}_chroma"), 0.0),
                    );
                    lum.set(f.f32_or(&format!("{r}_luminance"), 0.0));
                }
            }
            Body::Note(t) => {
                if ctl.id == super::diffuse::id::NOTE {
                    *t = super::diffuse::note(&f, self.frame);
                }
                if ctl.id == id::NOISE_AI_NOTE {
                    *t = self.denoise_line();
                }
                if ctl.id == id::INPUT_PROFILE_NOTE {
                    *t = self.input_profile_note();
                }
            }
        }
    }

    /// The fields of a control's module with the control's value written in.
    fn written(&self, control: u32) -> Option<(ModuleKind, Fields)> {
        let ctl = self.panel.control(control)?;
        let kind = module_of(control);
        let mut f = self.fields(kind);
        if let Some(w) = super::effects::write(ctl, &mut f) {
            return w.then_some((kind, f));
        }
        match &ctl.body {
            Body::Slider(s) => match control {
                id::TEMPERATURE | id::TINT => {
                    let other = self.panel.control(if control == id::TEMPERATURE {
                        id::TINT
                    } else {
                        id::TEMPERATURE
                    });
                    let other = match other.map(|c| &c.body) {
                        Some(Body::Slider(o)) => o.value,
                        _ => 0.0,
                    };
                    let (k, tint) = if control == id::TEMPERATURE {
                        (s.value, other)
                    } else {
                        (other, s.value)
                    };
                    let (x, y) = white_xy(k, tint);
                    f.set("white_source", Value::Text("custom".into()));
                    f.set("white_x", Value::Float(x));
                    f.set("white_y", Value::Float(y));
                }
                c @ id::ZONE..=id::ZONE_LAST => super::zones::write(&mut f, c, s.value),
                c => {
                    let (_, name) = float_field(c)?;
                    let v = light_scale(c).map_or(s.value, |sc| from_light(sc, s.value));
                    f.set(&name, Value::Float(v));
                    super::diffuse::fix_written(c, &mut f);
                    // Straightening keeps the crop inside the photo.
                    if c == id::STRAIGHTEN
                        && let Some((w, h)) = self.frame
                    {
                        let p = CropOrientation::from_fields(&f);
                        let fr = super::crop::frame_for(
                            w,
                            h,
                            self.persp.keystone,
                            f64::from(p.angle_deg).to_radians(),
                        );
                        let fit = crop::fit_inside_in(p.crop.map(f64::from), &fr);
                        f.set(
                            "crop",
                            Value::Floats(fit.iter().map(|&v| v as f32).collect()),
                        );
                    }
                }
            },
            Body::Choice(c) => match control {
                super::diffuse::id::PRESET => super::diffuse::write_choice(c, &mut f)?,
                id::VIEW_METHOD => {
                    let m = VIEW_METHODS.get(c.selected).copied().unwrap_or_default();
                    f.set("method", m.into());
                }
                id::LUT_SPACE => {
                    let sp = LUT_SPACES.get(c.selected).copied().unwrap_or_default();
                    f.set("space", sp.into());
                }
                id::LUT_INTERPOLATION => {
                    let name = if c.selected == 1 {
                        "trilinear"
                    } else {
                        "tetrahedral"
                    };
                    f.set("interpolation", Value::Text(name.into()));
                }
                id::HL_METHOD => {
                    let m = HL_METHODS
                        .get(c.selected)
                        .copied()
                        .unwrap_or(Highlights::Opposed);
                    f.set("highlights", m.into());
                }
                id::NOISE_MODE => {
                    use specular_develop::ops::detail::ai::Mode;
                    let m = if c.selected == 1 {
                        Mode::Ai
                    } else {
                        Mode::Classic
                    };
                    super::ai_denoise::write_mode(&mut f, m);
                }
                id::DEMOSAIC => {
                    let m = DEMOSAICS
                        .get(c.selected)
                        .copied()
                        .unwrap_or(Demosaic::Directional);
                    f.set("demosaic", m.into());
                }
                id::INPUT_PROFILE => {
                    let has_file = !f.text("profile_path").unwrap_or("").is_empty();
                    let m = match c.selected {
                        0 => InputProfile::Auto,
                        1 => InputProfile::Matrix,
                        // A file is chosen in the dialog first ([`Request::ChooseInputProfile`]).
                        _ if has_file => InputProfile::File,
                        _ => return None,
                    };
                    f.set("input_profile", m.into());
                }
                id::FLAT_USE => f.set("blur", Value::Bool(c.selected == 1)),
                id::AUTO_EXPOSURE => f.set("auto", Value::Bool(c.selected == 1)),
                super::lensfun::id::PROFILE => {
                    super::lensfun::write(self.lensfun.as_deref()?, c.selected, &mut f);
                }
                // Buttons change nothing themselves.
                _ => return None,
            },
            Body::Curve(c) => {
                f.set(
                    "points",
                    Value::Floats(c.points.iter().flat_map(|p| [p[0], p[1]]).collect()),
                );
            }
            Body::Wheels(ws) => {
                for (r, (w, lum)) in RANGES.iter().zip(ws.iter()) {
                    f.set(&format!("{r}_hue"), Value::Float(w.hue));
                    f.set(&format!("{r}_chroma"), Value::Float(w.strength));
                    f.set(&format!("{r}_luminance"), Value::Float(lum.value));
                }
            }
            Body::Note(_) => return None,
        }
        Some((kind, f))
    }

    /// Writes a control's value into the edit as a history step; whether it changed.
    fn write(&mut self, control: u32, now_ms: i64) -> bool {
        let Some((kind, mut after)) = self.written(control) else {
            return false;
        };
        let inst = self.inst(kind);
        let before = self.fields(kind);
        // What the control needs to act is set in the same step.
        let on = self.edit.module(kind, inst).is_none_or(|m| m.enabled);
        let enable = super::prereq::apply(control, &mut after, on);
        // Turning the module on alone shows nowhere else when it was on already (Light's
        // drags would refresh every frame otherwise).
        let fixed = enable
            || super::prereq::fixes(control)
                .iter()
                .any(|f| *f != super::prereq::Fix::Enable);
        if before == after && !enable {
            return false;
        }
        let step = match self.edit.module(kind, inst) {
            Some(m) if enable => {
                let mut to = m.clone();
                to.enabled = true;
                to.params.set_fields(after);
                Step {
                    kind,
                    instance: inst,
                    change: Change::Replace {
                        before: Box::new(m.clone()),
                        after: Box::new(to),
                    },
                    time_ms: now_ms,
                }
            }
            _ => Step::params(kind, inst, &before, &after, now_ms),
        };
        if self.history.push(&mut self.edit, step).is_err() {
            return false;
        }
        if fixed {
            // The switch or the other control it set shows its new value (rows stay as
            // they are: the keyboard's place in the panel holds).
            self.refresh_values(kind);
        }
        self.after_step();
        true
    }

    /// Puts the edit's values of module `kind` on its controls, and its switch, as the
    /// rows are.
    fn refresh_values(&mut self, kind: ModuleKind) {
        let mut cards = std::mem::take(&mut self.panel.cards);
        for c in &mut cards {
            if c.id == u32::from(kind.0)
                && let (Some(on), Some(m)) =
                    (c.enabled.as_mut(), self.edit.module(kind, self.inst(kind)))
            {
                *on = m.enabled;
            }
            for ctl in c.controls.iter_mut().filter(|k| {
                module_of(k.id) == kind
                    && !super::masks::is_mask(k.id)
                    && !super::retouch::is_retouch(k.id)
            }) {
                self.read_control(ctl);
            }
        }
        self.panel.cards = cards;
    }

    /// Refreshes the panel's history and button states (values stay as set).
    pub(super) fn after_step(&mut self) {
        self.panel.history = self.history_rows();
        self.panel.history_at = self.history.position;
        self.panel.can_undo = self.history.can_undo();
        self.panel.can_redo = self.history.can_redo();
        self.sync_dims();
    }

    /// The panel's events, applied; what the application should do.
    pub fn handle(&mut self, events: &[Event], now_ms: i64) -> Vec<Request> {
        let mut out = Vec::new();
        for e in events {
            match *e {
                Event::Value {
                    control: id::POINT_PICK,
                    ..
                } => out.push(Request::Pick),
                Event::Value {
                    control: id::LUT_FILE,
                    ..
                } => out.push(Request::ChooseLut),
                Event::Value {
                    control: id::CHART_PROFILE,
                    ..
                } => out.push(Request::ChartProfile),
                Event::Value {
                    control: id::INPUT_PROFILE,
                    ..
                } if self.input_profile_wants_file() => {
                    // "Profile file…" without a file yet: choose one (the choice shows the
                    // profile in use until then).
                    self.sync_panel();
                    out.push(Request::ChooseInputProfile);
                }
                Event::Value {
                    control: id::NOISE_AI_ACTION,
                    ..
                } => out.push(Request::DenoiseAction),
                Event::Value {
                    control: id::ZONE_DRAG,
                    ..
                } => out.push(self.toggle_zone_edit()),
                Event::Value {
                    control: c @ (id::FLAT_FILE | id::DARK_FILE),
                    ..
                } => out.push(Request::ChooseFrame(c == id::DARK_FILE)),
                Event::Value {
                    control: c @ (id::HOT | id::LENS_ON | id::CA_MODE | id::FLAT_MODE),
                    ..
                } => {
                    if self.write_switch(c, now_ms) {
                        out.push(Request::Render);
                    }
                }
                Event::Value { control, done, .. } if super::masks::is_mask(control) => {
                    out.extend(self.mask_value(control, done, now_ms));
                }
                Event::Value { control, done, .. } if super::retouch::is_retouch(control) => {
                    out.extend(self.retouch_value(control, done, now_ms));
                }
                // A way of looking, not an edit: no history step.
                Event::Value {
                    control: id::SDR_PREVIEW,
                    ..
                } => {
                    if let Some(Body::Choice(c)) =
                        self.panel.control(id::SDR_PREVIEW).map(|c| &c.body)
                    {
                        self.sdr_preview = c.selected == 1;
                    }
                    out.push(Request::Render);
                }
                Event::Value { control, .. }
                    if (ADD_TOOL..ADD_TOOL + TOOLS.len() as u32).contains(&control) =>
                {
                    self.add_tool((control - ADD_TOOL) as usize);
                }
                Event::Value { control, .. } if super::crop::card::is_ours(control) => {
                    out.push(Request::CropCard(control));
                }
                Event::Value { control, .. } if super::perspective::is_event(control) => {
                    out.extend(self.persp_event(control, now_ms));
                }
                Event::Value { control, .. } if super::effects::is_event(control) => {
                    out.extend(self.effect_event(control).unwrap_or_default());
                }
                Event::Value { control, .. } => {
                    if self.write(control, now_ms) {
                        out.push(Request::Render);
                        // A preset sets every slider; any slider may change the preset
                        // shown and the note.
                        if super::diffuse::is_diffuse(control)
                            || control == super::lensfun::id::PROFILE
                        {
                            self.sync_panel();
                        }
                    }
                }
                Event::Enabled { card, on } => {
                    let kind = ModuleKind(card as u16);
                    let step = Step {
                        kind,
                        instance: self.inst(kind),
                        change: Change::Enabled(on),
                        time_ms: now_ms,
                    };
                    if self.history.push(&mut self.edit, step).is_ok() {
                        self.after_step();
                        out.push(Request::Render);
                    }
                }
                Event::GoTo(n) => {
                    if self.history.go_to(&mut self.edit, n).is_ok() {
                        self.sync_panel();
                        out.push(Request::Render);
                        out.push(Request::Say(self.position_text()));
                    }
                }
                Event::Button(PanelButton::Undo) => out.extend(self.undo()),
                Event::Button(PanelButton::Redo) => out.extend(self.redo()),
                Event::Button(PanelButton::BeforeAfter) => out.push(Request::BeforeAfter),
                Event::Button(PanelButton::Export) => out.push(Request::Export),
                Event::Button(PanelButton::Close) => out.push(Request::Leave),
                Event::Menu { item, at } => out.push(Request::Menu(item, at)),
                Event::Left => {}
            }
        }
        out
    }

    /// "Back to: Exposure" or "Back to the original".
    fn position_text(&self) -> String {
        match self.history.position {
            0 => t!("develop-at-original"),
            n => t!(
                "develop-at-step",
                step = self.history_rows().get(n).cloned().unwrap_or_default()
            ),
        }
    }

    pub fn undo(&mut self) -> Vec<Request> {
        self.commit(0);
        self.rt_commit(0);
        let row = self.history_rows().get(self.history.position).cloned();
        match self.history.undo(&mut self.edit) {
            Ok(true) => {
                self.sync_panel();
                vec![
                    Request::Render,
                    Request::Say(t!("develop-undone", step = row.unwrap_or_default())),
                ]
            }
            _ => Vec::new(),
        }
    }

    pub fn redo(&mut self) -> Vec<Request> {
        self.commit(0);
        self.rt_commit(0);
        match self.history.redo(&mut self.edit) {
            Ok(true) => {
                self.sync_panel();
                let row = self.history_rows().get(self.history.position).cloned();
                vec![
                    Request::Render,
                    Request::Say(t!("develop-redone", step = row.unwrap_or_default())),
                ]
            }
            _ => Vec::new(),
        }
    }

    /// The edit's crop and orientation.
    pub fn crop(&self) -> CropOrientation {
        CropOrientation::from_fields(&self.fields(ModuleKind::CROP_ORIENTATION))
    }

    /// Sets the crop and orientation as a history step (steps within a second merge);
    /// whether it changed.
    pub fn set_crop(&mut self, p: &CropOrientation, now_ms: i64) -> bool {
        let kind = ModuleKind::CROP_ORIENTATION;
        let before = self.fields(kind);
        let mut after = before.clone();
        after.overlay(&p.to_fields());
        if before == after {
            return false;
        }
        let step = Step::params(kind, self.inst(kind), &before, &after, now_ms);
        if self.history.push(&mut self.edit, step).is_err() {
            return false;
        }
        self.sync_panel();
        true
    }

    /// Goes back to history position `n` (the crop tool's Esc).
    pub fn go_to(&mut self, n: usize) -> bool {
        let ok = self.history.position != n && self.history.go_to(&mut self.edit, n).is_ok();
        if ok {
            self.sync_panel();
        }
        ok
    }

    /// Speed edit: `steps` arrow steps on slider `control`; whether it changed.
    pub fn speed(&mut self, control: u32, steps: f32, now_ms: i64) -> bool {
        let Some(s) = self.panel.slider_mut(control) else {
            return false;
        };
        s.nudge(steps) && self.write(control, now_ms)
    }

    /// The history rows: the original, then a name per step.
    pub fn history_rows(&self) -> Vec<String> {
        let mut v = vec![t!("develop-history-original")];
        v.extend(self.history.steps.iter().map(step_label));
        v
    }

    /// Writes `after` over a module's fields as one history step; whether it changed.
    pub(super) fn set_fields(&mut self, kind: ModuleKind, after: Fields, now_ms: i64) -> bool {
        let before = self.fields(kind);
        if before == after {
            return false;
        }
        let step = Step::params(kind, self.inst(kind), &before, &after, now_ms);
        if self.history.push(&mut self.edit, step).is_err() {
            return false;
        }
        self.sync_panel();
        true
    }

    /// Point colour's target from a picked colour (linear Rec.2020 at the module's input);
    /// whether the edit changed.
    pub fn set_point_target(&mut self, rgb: [f64; 3], now_ms: i64) -> bool {
        let t = colour::point::target_of(rgb);
        let f = self
            .fields(ModuleKind::POINT_COLOUR)
            .with("picked", Value::Bool(true))
            .with("target_hue", Value::Float(t[0]))
            .with("target_saturation", Value::Float(t[1]))
            .with("target_lightness", Value::Float(t[2]));
        self.set_fields(ModuleKind::POINT_COLOUR, f, now_ms)
    }

    /// The LUT file; whether the edit changed.
    pub fn set_lut_path(&mut self, path: &Path, now_ms: i64) -> bool {
        let f = self
            .fields(ModuleKind::LUT_3D)
            .with("path", Value::Text(path.to_string_lossy().into_owned()));
        self.set_fields(ModuleKind::LUT_3D, f, now_ms)
    }

    /// "Profile file…" is chosen and the edit has no profile file yet.
    fn input_profile_wants_file(&self) -> bool {
        let chose = matches!(
            self.panel.control(id::INPUT_PROFILE).map(|c| &c.body),
            Some(Body::Choice(c)) if c.selected == 2
        );
        chose
            && self
                .fields(ModuleKind::RAW_FRONT_END)
                .text("profile_path")
                .unwrap_or("")
                .is_empty()
    }

    /// Uses camera profile file `path` (content hash `hash`) for the raw; whether the edit
    /// changed.
    pub fn set_input_profile_file(&mut self, path: &Path, hash: &str, now_ms: i64) -> bool {
        let f = self
            .fields(ModuleKind::RAW_FRONT_END)
            .with("input_profile", InputProfile::File.into())
            .with(
                "profile_path",
                Value::Text(path.to_string_lossy().into_owned()),
            )
            .with("profile_hash", Value::Text(hash.to_owned()));
        self.set_fields(ModuleKind::RAW_FRONT_END, f, now_ms)
    }

    /// A chart profile for an image that is not raw: the colour calibration mixer, and the
    /// cube at `cube` in the LUT module (ACEScct, as written). Whether the edit changed.
    pub fn set_chart_mixer(&mut self, mixer: [f32; 9], cube: &Path, now_ms: i64) -> bool {
        let f = self
            .fields(ModuleKind::COLOUR_CALIBRATION)
            .with("mixer", Value::Floats(mixer.to_vec()));
        let a = self.set_fields(ModuleKind::COLOUR_CALIBRATION, f, now_ms);
        let f = self
            .fields(ModuleKind::LUT_3D)
            .with("path", Value::Text(cube.to_string_lossy().into_owned()))
            .with("space", LutSpace::Acescct.into())
            .with("strength", Value::Float(1.0));
        let b = self.set_fields(ModuleKind::LUT_3D, f, now_ms);
        a || b
    }

    /// The camera profile's status line (empty unless a profile file is asked for).
    pub(super) fn input_profile_note(&self) -> String {
        use specular_develop::ops::raw::profile_file::{self, Problem, Status};
        let p = RawFrontEnd::from_fields(&self.fields(ModuleKind::RAW_FRONT_END));
        match profile_file::status(&p) {
            Status::Builtin => String::new(),
            Status::InUse {
                name,
                changed: false,
            } => t!("develop-input-profile-in-use", name = name),
            Status::InUse {
                name,
                changed: true,
            } => {
                t!("develop-input-profile-changed", name = name)
            }
            Status::Fallback(Problem::Missing | Problem::NoFile) => {
                t!("develop-input-profile-missing")
            }
            Status::Fallback(e) => t!("develop-input-profile-unusable", reason = e.to_string()),
        }
    }

    /// Whether a module is in the edit and on.
    pub(super) fn module_on(&self, kind: ModuleKind) -> bool {
        self.edit.module(kind, 0).is_some_and(|m| m.enabled)
    }

    /// A choice that turns a module on or off (and sets its `auto`): one or two history
    /// steps; whether anything changed.
    fn write_switch(&mut self, control: u32, now_ms: i64) -> bool {
        let Some(Body::Choice(c)) = self.panel.control(control).map(|c| &c.body) else {
            return false;
        };
        let sel = c.selected;
        let kind = module_of(control);
        let on = sel != 0;
        let Some(before) = self.edit.module(kind, 0).cloned() else {
            return false;
        };
        let mut after = before.clone();
        after.enabled = on;
        if !matches!(control, id::HOT | id::LENS_ON) && on {
            let auto = sel == 1;
            let f = before.params.to_fields();
            if f.bool_or("auto", !auto) != auto {
                after.params.set_fields(f.with("auto", Value::Bool(auto)));
            }
        }
        if before == after {
            return false;
        }
        // One step: the switch, the mode, or both at once.
        let change = if before.params == after.params {
            Change::Enabled(on)
        } else {
            Change::Replace {
                before: Box::new(before),
                after: Box::new(after),
            }
        };
        let step = Step {
            kind,
            instance: 0,
            change,
            time_ms: now_ms,
        };
        let changed = self.history.push(&mut self.edit, step).is_ok();
        if changed {
            self.after_step();
        }
        changed
    }

    /// Sets the flat (or, `dark`, the dark) frame and turns calibration on with the chosen
    /// frames; whether it changed.
    pub fn set_frame_path(&mut self, dark: bool, path: &Path, now_ms: i64) -> bool {
        let key = if dark { "dark" } else { "flat" };
        let f = self
            .fields(ModuleKind::FLAT_FIELD)
            .with(key, Value::Text(path.to_string_lossy().into_owned()))
            .with("auto", Value::Bool(false));
        let mut changed = self.set_fields(ModuleKind::FLAT_FIELD, f, now_ms);
        if !self.module_on(ModuleKind::FLAT_FIELD) {
            let step = Step {
                kind: ModuleKind::FLAT_FIELD,
                instance: 0,
                change: Change::Enabled(true),
                time_ms: now_ms,
            };
            if self.history.push(&mut self.edit, step).is_ok() {
                self.after_step();
                changed = true;
            }
        }
        if changed {
            self.sync_panel();
        }
        changed
    }

    /// A frame button's label: the file, or the call to choose one.
    fn frame_label(&self, dark: bool) -> String {
        let f = self.fields(ModuleKind::FLAT_FIELD);
        let path = f.text(if dark { "dark" } else { "flat" }).unwrap_or("");
        if path.is_empty() {
            return if dark {
                t!("develop-dark-choose")
            } else {
                t!("develop-flat-choose")
            };
        }
        let name = Path::new(path)
            .file_name()
            .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned());
        if dark {
            t!("develop-dark-file", name = name)
        } else {
            t!("develop-flat-file", name = name)
        }
    }

    /// The folder of the chosen flat (or dark) frame, for the file dialog.
    pub fn frame_dir(&self, dark: bool) -> Option<PathBuf> {
        let f = self.fields(ModuleKind::FLAT_FIELD);
        let p = PathBuf::from(f.text(if dark { "dark" } else { "flat" })?);
        p.parent()
            .filter(|d| !d.as_os_str().is_empty())
            .map(Path::to_path_buf)
    }

    /// The LUT file button's label: the file, why it can't be used, or the call to choose.
    fn lut_label(&self) -> String {
        let Some(m) = self.edit.module(ModuleKind::LUT_3D, 0) else {
            return t!("develop-lut-choose");
        };
        let path = m.params.to_fields().text("path").unwrap_or("").to_owned();
        if path.is_empty() {
            return t!("develop-lut-choose");
        }
        let name = Path::new(&path)
            .file_name()
            .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
        match colour::lut::status(&m.params) {
            Ok(_) => t!("develop-lut-file", name = name),
            Err(_) => t!("develop-lut-unreadable", name = name),
        }
    }

    /// Why the LUT file can't be used (the reader's words), when it can't.
    pub fn lut_problem(&self) -> Option<String> {
        let m = self.edit.module(ModuleKind::LUT_3D, 0)?;
        colour::lut::status(&m.params)
            .err()
            .filter(|e| !e.is_empty())
    }

    /// The LUT file's folder, for the file dialog.
    pub fn lut_dir(&self) -> Option<PathBuf> {
        let m = self.edit.module(ModuleKind::LUT_3D, 0)?;
        let p = PathBuf::from(m.params.to_fields().text("path")?);
        p.parent().map(Path::to_path_buf)
    }

    /// The edit to show: while a point colour range slider is dragged, with the range
    /// overlay on (never saved).
    pub fn shown_edit(&self) -> std::borrow::Cow<'_, Edit> {
        let dragging_range = self.panel.dragging()
            && self.panel.focus().is_some_and(|it| {
                let specular_widgets::develop_panel::Item::Control { card, control, .. } = it
                else {
                    return false;
                };
                self.panel
                    .cards
                    .get(card)
                    .and_then(|c| c.controls.get(control))
                    .is_some_and(|c| POINT_RANGES.contains(&c.id))
            });
        let picked = self
            .fields(ModuleKind::POINT_COLOUR)
            .bool_or("picked", false);
        if !dragging_range || !picked {
            return std::borrow::Cow::Borrowed(&self.edit);
        }
        let mut e = self.edit.clone();
        if let Some(m) = e.module_mut(ModuleKind::POINT_COLOUR, 0) {
            let f = m.params.to_fields().with("show_range", Value::Bool(true));
            m.params.set_fields(f);
            m.enabled = true;
        }
        std::borrow::Cow::Owned(e)
    }

    /// The image's as-shot white (CIE xy), known once the raw is read: the white balance
    /// is relative to it, and "as shot" means it. Not a history step.
    pub fn set_as_shot(&mut self, xy: [f64; 2]) {
        let Some(m) = self.edit.module_mut(ModuleKind::COLOUR_CALIBRATION, 0) else {
            return;
        };
        let mut f = m.params.to_fields();
        let (x, y) = (Value::Float(xy[0] as f32), Value::Float(xy[1] as f32));
        if f.get("as_shot_x") == Some(&x) && f.get("as_shot_y") == Some(&y) {
            return;
        }
        f.set("as_shot_x", x.clone());
        f.set("as_shot_y", y.clone());
        if f.text("white_source") != Some("custom") {
            f.set("white_x", x);
            f.set("white_y", y);
        }
        m.params.set_fields(f);
        self.sync_panel();
    }
}

/// A history row's name: the field changed, else the module.
fn step_label(s: &Step) -> String {
    let title = module_title(s.kind);
    match &s.change {
        Change::Params(c) => {
            let mut names: Vec<String> = c
                .iter()
                .filter_map(|fc| field_label(s.kind, &fc.name))
                .collect();
            names.dedup();
            match names.as_slice() {
                [one] => one.clone(),
                _ => title,
            }
        }
        Change::Enabled(true) => t!("develop-history-on", module = title),
        Change::Enabled(false) => t!("develop-history-off", module = title),
        // A paste, a style, a snapshot or a match: what it was.
        Change::Stack { what, .. } => what.clone(),
        Change::Blend { .. } => t!("develop-history-mask", module = title),
        Change::Masks { .. } => t!("develop-history-mask-shapes", module = title),
        Change::Add { .. } => t!("develop-history-instance-new", module = title),
        Change::Remove { .. } => t!("develop-history-instance-delete", module = title),
        Change::Move { .. } => t!("develop-history-instance-move", module = title),
        _ => title,
    }
}

/// D50 white (CIE 1931 xy), colour calibration's default.
pub(super) const D50: [f32; 2] = specular_develop::model::params::D50_XY;

/// The CIE 1960 uv of a chromaticity.
fn uv(x: f64, y: f64) -> (f64, f64) {
    let d = -2.0 * x + 12.0 * y + 3.0;
    (4.0 * x / d, 6.0 * y / d)
}

fn xy_of_uv(u: f64, v: f64) -> (f64, f64) {
    let d = 2.0 * u - 8.0 * v + 4.0;
    (3.0 * u / d, 2.0 * v / d)
}

/// The Planckian locus at `k` kelvin (Kim et al. 2002 cubic fit, 1667–25000 K).
fn planck_xy(k: f64) -> (f64, f64) {
    let k = k.clamp(1667.0, 25000.0);
    let (k2, k3) = (k * k, k * k * k);
    let x = if k <= 4000.0 {
        -0.266_123_9e9 / k3 - 0.234_358_9e6 / k2 + 0.877_695_6e3 / k + 0.179_910
    } else {
        -3.025_846_9e9 / k3 + 2.107_037_9e6 / k2 + 0.222_634_7e3 / k + 0.240_390
    };
    let (x2, x3) = (x * x, x * x * x);
    let y = if k <= 2222.0 {
        -1.106_381_4 * x3 - 1.348_110_20 * x2 + 2.185_558_32 * x - 0.202_196_83
    } else if k <= 4000.0 {
        -0.954_947_6 * x3 - 1.374_185_93 * x2 + 2.091_370_15 * x - 0.167_488_67
    } else {
        3.081_758_0 * x3 - 5.873_386_70 * x2 + 3.751_129_97 * x - 0.370_014_83
    };
    (x, y)
}

/// The locus point at `k` in uv and its unit normal (towards green, +Duv).
fn locus(k: f64) -> ((f64, f64), (f64, f64)) {
    let p = planck_xy(k);
    let a = planck_xy(k - 10.0);
    let b = planck_xy(k + 10.0);
    let (pu, pv) = uv(p.0, p.1);
    let (au, av) = uv(a.0, a.1);
    let (bu, bv) = uv(b.0, b.1);
    let (tu, tv) = (bu - au, bv - av);
    let l = tu.hypot(tv).max(1e-12);
    // The tangent turned a quarter so the normal points to larger v (greener).
    let n = (-tv / l, tu / l);
    let n = if n.1 < 0.0 { (-n.0, -n.1) } else { n };
    ((pu, pv), n)
}

/// Duv of a full tint (±100): magenta below the locus, green above.
const TINT_DUV: f64 = 0.03;

/// The Duv of tint 0: D50's (just above the Planckian locus, as daylight is), so the
/// module's default white reads as 5000 K with no tint.
fn tint_zero() -> f64 {
    static Z: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *Z.get_or_init(|| raw_kelvin_duv(f64::from(D50[0]), f64::from(D50[1])).1)
}

/// The white of a temperature and tint (Lightroom-like: positive tint is magenta).
pub(super) fn white_xy(kelvin: f32, tint: f32) -> (f32, f32) {
    let ((u, v), n) = locus(f64::from(kelvin));
    let duv = tint_zero() - f64::from(tint) / 100.0 * TINT_DUV;
    let (x, y) = xy_of_uv(u + n.0 * duv, v + n.1 * duv);
    (x as f32, y as f32)
}

/// The temperature and tint of a white: the inverse of [`white_xy`] (McCamy for a first
/// guess, refined along the locus).
pub(super) fn kelvin_tint(x: f32, y: f32) -> (f32, f32) {
    let (k, duv) = raw_kelvin_duv(f64::from(x), f64::from(y));
    (k as f32, (-(duv - tint_zero()) / TINT_DUV * 100.0) as f32)
}

/// The temperature of a white and its distance from the Planckian locus (Duv).
fn raw_kelvin_duv(x: f64, y: f64) -> (f64, f64) {
    let n = (x - 0.3320) / (0.1858 - y);
    let mut k =
        (449.0 * n.powi(3) + 3525.0 * n.powi(2) + 6823.3 * n + 5520.33).clamp(1667.0, 25000.0);
    let (pu, pv) = uv(x, y);
    // Newton steps: the locus point whose tangent is perpendicular to the offset.
    for _ in 0..8 {
        let f = |k: f64| {
            let ((u, v), nrm) = locus(k);
            // Offset along the tangent (the normal turned back).
            let t = (nrm.1, -nrm.0);
            (pu - u) * t.0 + (pv - v) * t.1
        };
        let h = 5.0;
        let d = (f(k + h) - f(k - h)) / (2.0 * h);
        if d.abs() < 1e-15 {
            break;
        }
        k = (k - f(k) / d).clamp(1667.0, 25000.0);
    }
    let ((u, v), nrm) = locus(k);
    (k, (pu - u) * nrm.0 + (pv - v) * nrm.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let p = PathBuf::from("/photos/IMG_0001.JPG");
        Session::new(p.clone(), new_edit(&p), History::default())
    }

    fn slider_value(s: &Session, id: u32) -> f32 {
        match &s.panel.control(id).unwrap().body {
            Body::Slider(s) => s.value,
            _ => unreachable!(),
        }
    }

    #[test]
    fn temperature_and_tint_round_trip() {
        for k in [2500.0, 3200.0, 5000.0, 6500.0, 9000.0] {
            for tint in [-40.0, 0.0, 25.0] {
                let (x, y) = white_xy(k, tint);
                let (k2, t2) = kelvin_tint(x, y);
                assert!((k2 - k).abs() < 5.0, "{k} {tint}: {k2}");
                assert!((t2 - tint).abs() < 0.5, "{k} {tint}: {t2}");
            }
        }
        // D50 reads as about 5000 K with no tint.
        let (k, t) = kelvin_tint(D50[0], D50[1]);
        assert!((k - 5000.0).abs() < 30.0 && t.abs() < 0.01, "{k} {t}");
    }

    /// A raw starts with some colour noise reduction and capture sharpening; other files do
    /// not. Neither shows as a change (no dot), resetting goes back to them, and the
    /// catalogue does not list them as used until someone changes them.
    #[test]
    fn a_raw_starts_with_colour_noise_reduction_and_capture_sharpening() {
        use specular_develop::model::summary::used_modules;
        let reg = Registry::builtin();
        let raw = PathBuf::from("/photos/IMG_0042.CR3");
        let e = new_edit(&raw);
        let f = |e: &Edit, k: ModuleKind, n: &str| {
            let m = e.module(k, 0).unwrap();
            assert!(m.enabled, "{k:?}");
            m.params.to_fields().f32_or(n, f32::NAN)
        };
        assert_eq!(f(&e, ModuleKind::DENOISE, "colour"), 0.25);
        assert_eq!(f(&e, ModuleKind::DENOISE, "luminance"), 0.0);
        assert_eq!(f(&e, ModuleKind::CAPTURE_SHARPENING, "amount"), 0.3);
        // The raw lists what it would without its raw defaults.
        let mut plain = Edit::default_for(reg, SourceKey::default(), true);
        ensure_modules(&mut plain, &raw);
        assert_eq!(
            used_modules(&without_raw_defaults(&e), reg),
            used_modules(&plain, reg)
        );
        let jpeg = new_edit(Path::new("/photos/IMG_0042.JPG"));
        assert_eq!(f(&jpeg, ModuleKind::DENOISE, "colour"), 0.0);
        assert_eq!(f(&jpeg, ModuleKind::CAPTURE_SHARPENING, "amount"), 0.0);
        let mut s = Session::new(raw.clone(), e, History::default());
        assert!(s.panel.changed.is_empty(), "{:x?}", s.panel.changed);
        match s.panel.control(id::NOISE_COLOUR).map(|c| &c.body) {
            Some(Body::Slider(sl)) => {
                assert_eq!(sl.value, 0.25);
                assert_eq!(sl.spec.default, 0.25, "resets to the raw default");
            }
            _ => panic!("colour noise slider"),
        }
        // Changed: listed as used.
        let f = s
            .fields(ModuleKind::DENOISE)
            .with("colour", Value::Float(0.5));
        assert!(s.set_fields(ModuleKind::DENOISE, f, 1_000));
        let mut want = used_modules(&plain, reg);
        want.insert(0, "denoise");
        assert_eq!(used_modules(&without_raw_defaults(&s.edit), reg), want);
    }

    #[test]
    fn a_new_edit_has_every_module_the_panel_drives() {
        let s = session();
        for c in s.panel.cards.iter().flat_map(|c| c.controls.iter()) {
            assert!(s.edit.module(module_of(c.id), 0).is_some(), "{}", c.id);
        }
        assert!(s.edit.module(ModuleKind::RAW_FRONT_END, 0).is_none());
        assert!(is_raw(Path::new("a.NEF")) && !is_raw(Path::new("a.jpg")));
        assert_eq!(slider_value(&s, id::TEMPERATURE), DEFAULT_KELVIN);
        // Light's contrast at 0 on Lightroom's scale: the engine's 1.5.
        assert_eq!(slider_value(&s, id::CONTRAST), 0.0);
    }

    #[test]
    fn changes_are_steps_that_undo_and_redo() {
        let mut s = session();
        s.panel.slider_mut(id::EXPOSURE).unwrap().set(0.7);
        let r = s.handle(
            &[Event::Value {
                control: id::EXPOSURE,
                part: 0,
                done: true,
            }],
            1_000,
        );
        assert_eq!(r, vec![Request::Render]);
        // A second change to the same module within a second merges into one step.
        s.panel.slider_mut(id::EXPOSURE).unwrap().set(0.9);
        s.handle(
            &[Event::Value {
                control: id::EXPOSURE,
                part: 0,
                done: true,
            }],
            1_500,
        );
        assert_eq!(s.history.steps.len(), 1);
        s.panel.slider_mut(id::TEMPERATURE).unwrap().set(6500.0);
        s.handle(
            &[Event::Value {
                control: id::TEMPERATURE,
                part: 0,
                done: true,
            }],
            5_000,
        );
        assert_eq!(s.history.steps.len(), 2);
        assert_eq!(
            s.panel.history,
            vec![
                t!("develop-history-original"),
                t!("develop-exposure"),
                t!("develop-module-white-balance")
            ]
        );
        let cc = s.fields(ModuleKind::COLOUR_CALIBRATION);
        assert_eq!(cc.text("white_source"), Some("custom"));
        s.undo();
        assert_eq!(slider_value(&s, id::TEMPERATURE), DEFAULT_KELVIN);
        s.undo();
        assert_eq!(slider_value(&s, id::EXPOSURE), 0.0);
        assert!(!s.panel.can_undo && s.panel.can_redo);
        s.redo();
        assert!((slider_value(&s, id::EXPOSURE) - 0.9).abs() < 1e-6);
        // A history row.
        s.handle(&[Event::GoTo(2)], 9_000);
        assert_eq!(slider_value(&s, id::TEMPERATURE), 6500.0);
        // Switching a module off is a step too.
        s.handle(
            &[Event::Enabled {
                card: u32::from(ModuleKind::EXPOSURE.0),
                on: false,
            }],
            10_000,
        );
        assert!(!s.edit.module(ModuleKind::EXPOSURE, 0).unwrap().enabled);
        assert_eq!(
            s.panel.history.last().unwrap(),
            &t!(
                "develop-history-off",
                module = t!("develop-module-exposure")
            )
        );
        s.undo();
        assert!(s.edit.module(ModuleKind::EXPOSURE, 0).unwrap().enabled);
    }

    #[test]
    fn curve_and_wheels_write_named_fields() {
        let mut s = session();
        if let Some(Control {
            body: Body::Curve(c),
            ..
        }) = s.panel.control_mut(id::CURVE)
        {
            c.add_at(0.5);
            let i = c.selected;
            c.move_to(i, 0.5, 0.6);
        }
        s.handle(
            &[Event::Value {
                control: id::CURVE,
                part: 0,
                done: true,
            }],
            1_000,
        );
        let f = s.fields(ModuleKind::CURVES);
        assert_eq!(
            f.get("points"),
            Some(&Value::Floats(vec![0.0, 0.0, 0.5, 0.6, 1.0, 1.0]))
        );
        if let Some(Control {
            body: Body::Wheels(w),
            ..
        }) = s.panel.control_mut(id::WHEELS)
        {
            w[2].0.set(30.0, 0.25);
        }
        s.handle(
            &[Event::Value {
                control: id::WHEELS,
                part: 4,
                done: true,
            }],
            5_000,
        );
        let f = s.fields(ModuleKind::COLOUR_BALANCE);
        assert_eq!(f.f32_or("highlights_hue", 0.0), 30.0);
        assert_eq!(f.f32_or("highlights_chroma", 0.0), 0.25);
        // Undo puts the identity curve back on the panel.
        s.undo();
        s.undo();
        let Some(Body::Curve(c)) = s.panel.control(id::CURVE).map(|c| &c.body) else {
            panic!()
        };
        assert!(c.is_identity());
    }

    #[test]
    fn speed_edit_steps_a_slider() {
        let mut s = session();
        assert!(s.speed(id::EXPOSURE, 3.0, 1_000));
        assert!((slider_value(&s, id::EXPOSURE) - 0.15).abs() < 1e-6);
        assert!(!s.speed(id::CURVE, 1.0, 1_000));
        assert_eq!(s.history.steps.len(), 1);
    }

    /// A JPEG has no Corrections card (all of it is raw-only), and
    /// the Optics card's refining sliders switch on what they need, one history step each,
    /// undone whole.
    #[test]
    fn jpeg_corrections_switch_on_what_they_need() {
        let p = PathBuf::from("/photos/IMG_0042.JPG");
        let mut s = Session::new(p.clone(), new_edit(&p), History::default());
        assert!(s.panel.cards.iter().all(|c| c.id != CORRECTIONS_CARD));
        let optics = s
            .panel
            .cards
            .iter()
            .find(|c| c.id == u32::from(ModuleKind::LENS.0))
            .expect("optics");
        // Nothing on a JPEG's Optics card waits for something else.
        for k in &optics.controls {
            assert_eq!(s.panel.dimmed_reason(k.id), None, "{}", k.id);
        }
        let fresh = s.edit.clone();
        let choice = |s: &Session| match s.panel.control(id::CA_MODE).map(|c| &c.body) {
            Some(Body::Choice(c)) => c.selected,
            _ => usize::MAX,
        };
        assert_eq!(choice(&s), 0, "fringes start off");
        // Red shift: the module on, in Manual, in one step (the drag joins it).
        for (i, v) in [0.1f32, 0.2].into_iter().enumerate() {
            s.panel.slider_mut(id::CA_RED).unwrap().set(v);
            s.handle(
                &[Event::Value {
                    control: id::CA_RED,
                    part: 0,
                    done: false,
                }],
                10_000 + i as i64 * 100,
            );
        }
        assert_eq!(s.history.steps.len(), 1);
        assert!(s.module_on(ModuleKind::CHROMATIC_ABERRATION));
        assert_eq!(choice(&s), 2, "Manual");
        let ca = specular_develop::ops::correct::chromatic_aberration(&s.edit).unwrap();
        assert_eq!((ca.auto, ca.red), (false, 0.2));
        // A defringe hue with no amount gets one.
        s.panel.slider_mut(id::DEFRINGE_PURPLE).unwrap().set(300.0);
        s.handle(
            &[Event::Value {
                control: id::DEFRINGE_PURPLE,
                part: 0,
                done: true,
            }],
            50_000,
        );
        assert_eq!(s.history.steps.len(), 2);
        assert_eq!(slider_value(&s, id::DEFRINGE), 0.5);
        // Undone whole.
        s.undo();
        s.undo();
        assert_eq!(s.edit, fresh);
        assert_eq!(choice(&s), 0);
        assert_eq!(slider_value(&s, id::DEFRINGE), 0.0);
    }

    /// The Optics card: Lens corrections is a row (the card has no switch), the manual lens
    /// rows wait for it, and the lens module's own fringe rows show only while used.
    #[test]
    fn optics_card_holds_the_lens_and_its_fringes() {
        let p = PathBuf::from("/photos/IMG_0042.CR3");
        let mut s = Session::new(p.clone(), new_edit(&p), History::default());
        let lens = u32::from(ModuleKind::LENS.0);
        let ids = |s: &Session| -> Vec<u32> {
            let c = s.panel.cards.iter().find(|c| c.id == lens).expect("optics");
            assert_eq!(c.enabled, None, "no switch of its own");
            c.controls.iter().map(|k| k.id).collect()
        };
        let fresh = ids(&s);
        assert_eq!(fresh[0], id::LENS_ON);
        for k in [id::LENS_DISTORTION, id::CA_MODE, id::CA_RED, id::DEFRINGE] {
            assert!(fresh.contains(&k), "{k}");
        }
        assert!(
            !fresh.contains(&id::LENS_TCA_RED),
            "no lens fringe rows unused"
        );
        let corr = s
            .panel
            .cards
            .iter()
            .find(|c| c.id == CORRECTIONS_CARD)
            .unwrap();
        assert!(
            corr.controls
                .iter()
                .all(|k| !matches!(k.id, id::CA_MODE | id::DEFRINGE))
        );
        // An older edit's lens fringe: its rows show, after Blue shift.
        let f = s
            .fields(ModuleKind::LENS)
            .with("tca_red", Value::Float(0.1));
        assert!(s.set_fields(ModuleKind::LENS, f, 1_000));
        let used = ids(&s);
        let at = used.iter().position(|k| *k == id::CA_BLUE).unwrap();
        assert_eq!(used[at + 1..at + 3], [id::LENS_TCA_RED, id::LENS_TCA_BLUE]);
        assert!((slider_value(&s, id::LENS_TCA_RED) - 0.1).abs() < 1e-6);
        s.undo();
        assert_eq!(ids(&s), fresh);
        // Lens corrections off: one step, and the manual rows say why they wait.
        assert!(s.module_on(ModuleKind::LENS));
        match s.panel.control_mut(id::LENS_ON).map(|c| &mut c.body) {
            Some(Body::Choice(c)) => {
                c.set(0);
            }
            _ => panic!("lens switch row"),
        }
        // (By position: the undone step above is still there to redo, until this one.)
        let steps = s.history.position;
        s.handle(
            &[Event::Value {
                control: id::LENS_ON,
                part: 0,
                done: true,
            }],
            5_000,
        );
        assert!(!s.module_on(ModuleKind::LENS));
        assert_eq!(s.history.position, steps + 1);
        s.sync_panel();
        assert!(s.panel.dimmed_reason(id::LENS_DISTORTION).is_some());
        assert_eq!(s.panel.dimmed_reason(id::CA_RED), None);
    }

    /// Phase 3 item 11: a raw's Lensfun lookup puts the profile picker and its note after
    /// Lens corrections; picking a lens is one history step in the lens module.
    #[test]
    fn optics_card_lensfun_rows() {
        use specular_develop::ops::geometry::lensfun::{Candidate, NONE, Shot};
        use specular_lensfun::lookup::Pick;
        use std::sync::Arc;
        let lf = super::super::lensfun::id::PROFILE;
        let note = super::super::lensfun::id::NOTE;
        let p = PathBuf::from("/photos/IMG_0042.CR3");
        let mut s = Session::new(p.clone(), new_edit(&p), History::default());
        let lens = u32::from(ModuleKind::LENS.0);
        let ids = |s: &Session| -> Vec<u32> {
            let c = s.panel.cards.iter().find(|c| c.id == lens).expect("optics");
            c.controls.iter().map(|k| k.id).collect()
        };
        assert!(!ids(&s).contains(&lf), "no rows without a lookup");
        let profile = Arc::new(specular_lensfun::model::Profile::new(
            None, None, None, 1.0, 1.5, 1.0,
        ));
        let shot = Shot {
            lens_name: Some("EF50mm f/1.8".into()),
            candidates: ["II", "STM"]
                .iter()
                .map(|n| Candidate {
                    key: format!("k-{n}"),
                    name: format!("Canon EF 50mm f/1.8 {n}"),
                    profile: profile.clone(),
                })
                .collect(),
            pick: Pick::Ambiguous,
            tied: 2,
        };
        s.set_lensfun(Some(Arc::new(shot)));
        let v = ids(&s);
        assert_eq!(v[..3], [id::LENS_ON, lf, note]);
        let note_text = |s: &Session| match s.panel.control(note).map(|c| &c.body) {
            Some(Body::Note(t)) => t.clone(),
            _ => panic!("note"),
        };
        assert!(note_text(&s).contains('2'), "asks: {}", note_text(&s));
        // Pick the STM (option 2: Automatic, II, STM, None).
        let steps = s.history.position;
        match s.panel.control_mut(lf).map(|c| &mut c.body) {
            Some(Body::Choice(c)) => {
                assert_eq!(c.options.len(), 4);
                c.set(2);
            }
            _ => panic!("picker"),
        }
        s.handle(
            &[Event::Value {
                control: lf,
                part: 0,
                done: true,
            }],
            5_000,
        );
        assert_eq!(s.history.position, steps + 1);
        assert_eq!(s.fields(ModuleKind::LENS).text("lensfun"), Some("k-STM"));
        assert!(note_text(&s).contains("STM"));
        // None, then back to Automatic (the field goes).
        for (i, want) in [(3, Some(NONE)), (0, None)] {
            if let Some(Body::Choice(c)) = s.panel.control_mut(lf).map(|c| &mut c.body) {
                c.set(i);
            }
            s.handle(
                &[Event::Value {
                    control: lf,
                    part: 0,
                    done: true,
                }],
                9_000 + i as i64 * 2_000,
            );
            assert_eq!(s.fields(ModuleKind::LENS).text("lensfun"), want);
        }
        s.undo();
        assert_eq!(s.fields(ModuleKind::LENS).text("lensfun"), Some(NONE));
        s.set_lensfun(None);
        assert!(!ids(&s).contains(&lf));
    }

    /// Light, like Lightroom's: its six and Auto on one card with no switch; contrast,
    /// highlights, shadows, whites and blacks on −100 … +100 with 0 the engine's neutral;
    /// a slider turns its module back on in the same step; black level is a correction
    /// (Corrections on a raw; under Light on another file only while the edit uses it).
    #[test]
    fn light_holds_the_six_on_lightrooms_scale() {
        use ModuleKind as K;
        let p = PathBuf::from("/photos/IMG_0042.CR3");
        let mut s = Session::new(p.clone(), new_edit(&p), History::default());
        let light = u32::from(K::EXPOSURE.0);
        let card = s.panel.cards.iter().find(|c| c.id == light).unwrap();
        assert_eq!(card.enabled, None, "no switch");
        let ids: Vec<u32> = card.controls.iter().map(|k| k.id).collect();
        assert_eq!(
            ids,
            [
                id::VIEW_METHOD,
                id::AUTO_EXPOSURE,
                id::EXPOSURE,
                id::CONTRAST,
                id::HIGHLIGHTS,
                id::SHADOWS,
                id::WHITES,
                id::BLACKS
            ]
        );
        let corr = s.panel.cards.iter().find(|c| c.id == CORRECTIONS_CARD);
        assert!(
            corr.unwrap()
                .controls
                .iter()
                .any(|k| k.id == id::BLACK_LEVEL)
        );
        let send = |s: &mut Session, c: u32, v: f32, t: i64| {
            s.panel.slider_mut(c).unwrap().set(v);
            s.handle(
                &[Event::Value {
                    control: c,
                    part: 0,
                    done: true,
                }],
                t,
            );
        };
        let field = |s: &Session, k: ModuleKind, n: &str| s.fields(k).f32_or(n, f32::NAN);
        send(&mut s, id::CONTRAST, 100.0, 1_000);
        assert!((field(&s, K::VIEW_TRANSFORM, "contrast") - 3.0).abs() < 1e-5);
        send(&mut s, id::CONTRAST, -100.0, 2_000);
        assert!((field(&s, K::VIEW_TRANSFORM, "contrast") - 0.5).abs() < 1e-5);
        send(&mut s, id::CONTRAST, 0.0, 3_000);
        assert_eq!(field(&s, K::VIEW_TRANSFORM, "contrast"), 1.5);
        send(&mut s, id::SHADOWS, 50.0, 4_000);
        assert!((field(&s, K::TONE_EQUALIZER, "shadows_ev") - 2.0).abs() < 1e-5);
        assert!((slider_value(&s, id::SHADOWS) - 50.0).abs() < 1e-3);
        // Exposure off (an older edit): moving it turns it on, in the same step.
        s.handle(
            &[Event::Enabled {
                card: light,
                on: false,
            }],
            5_000,
        );
        assert!(!s.module_on(K::EXPOSURE));
        let steps = s.history.steps.len();
        send(&mut s, id::EXPOSURE, 0.5, 6_000);
        assert!(s.module_on(K::EXPOSURE));
        assert_eq!(s.history.steps.len(), steps + 1);
        // Another file: no Corrections card, so black level shows under Light while used.
        let p = PathBuf::from("/photos/IMG_0042.JPG");
        let mut s = Session::new(p.clone(), new_edit(&p), History::default());
        let has = |s: &Session| {
            let c = s.panel.cards.iter().find(|c| c.id == light).unwrap();
            c.controls.iter().any(|k| k.id == id::BLACK_LEVEL)
        };
        assert!(!has(&s));
        let f = s
            .fields(K::EXPOSURE)
            .with("black_offset", Value::Float(0.01));
        assert!(s.set_fields(K::EXPOSURE, f, 1_000));
        assert!(has(&s));
        assert!((slider_value(&s, id::BLACK_LEVEL) - 0.01).abs() < 1e-6);
        s.undo();
        assert!(!has(&s));
    }

    /// Auto exposure is a row of the Light card (no card of its own): it sets the
    /// module's `auto` in one step, the card shows it changed, and undo takes it back.
    #[test]
    fn auto_exposure_is_a_row_of_exposure() {
        let p = PathBuf::from("/photos/IMG_0042.CR3");
        let mut s = Session::new(p.clone(), new_edit(&p), History::default());
        let exposure = u32::from(ModuleKind::EXPOSURE.0);
        let card = s.panel.cards.iter().find(|c| c.id == exposure).unwrap();
        assert_eq!(card.controls[1].id, id::AUTO_EXPOSURE);
        let auto = t!("develop-auto-exposure");
        assert!(
            s.panel.cards.iter().all(|c| c.title != auto),
            "no card of its own"
        );
        let set = |s: &mut Session, on: usize| {
            match s.panel.control_mut(id::AUTO_EXPOSURE).map(|c| &mut c.body) {
                Some(Body::Choice(c)) => {
                    c.set(on);
                }
                _ => panic!("auto row"),
            }
            s.handle(
                &[Event::Value {
                    control: id::AUTO_EXPOSURE,
                    part: 0,
                    done: true,
                }],
                1_000,
            );
        };
        let steps = s.history.steps.len();
        set(&mut s, 1);
        assert!(s.fields(ModuleKind::EXPOSURE).bool_or("auto", false));
        assert_eq!(s.history.steps.len(), steps + 1);
        s.sync_panel();
        assert!(s.panel.changed.contains(&exposure));
        s.undo();
        assert!(!s.fields(ModuleKind::EXPOSURE).bool_or("auto", false));
        assert!(matches!(
            s.panel.control(id::AUTO_EXPOSURE).map(|c| &c.body),
            Some(Body::Choice(c)) if c.selected == 0
        ));
    }
}

#[cfg(test)]
mod pair_tests {
    use super::*;

    #[test]
    fn the_raw_of_a_pair_is_developed() {
        let dir =
            std::env::temp_dir().join(format!("specular-develop-pair-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "IMG_1.JPG",
            "IMG_1.CR3",
            "IMG_2.jpg",
            "IMG_3.jpg",
            "IMG_3.nef",
        ] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        assert_eq!(
            raw_partner(&dir.join("IMG_1.JPG")),
            Some(dir.join("IMG_1.CR3"))
        );
        assert_eq!(
            raw_partner(&dir.join("IMG_3.jpg")),
            Some(dir.join("IMG_3.nef"))
        );
        // No raw beside it, or a raw itself: nothing.
        assert_eq!(raw_partner(&dir.join("IMG_2.jpg")), None);
        assert_eq!(raw_partner(&dir.join("IMG_1.CR3")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// The camera profile choice's third option: the profile file's name, or the call to
/// choose one.
fn input_profile_file_label(p: &RawFrontEnd) -> String {
    if p.profile_path.is_empty() {
        return t!("develop-input-profile-file");
    }
    let name = Path::new(&p.profile_path).file_stem().map_or_else(
        || p.profile_path.clone(),
        |n| n.to_string_lossy().into_owned(),
    );
    t!("develop-input-profile-named", name = name)
}
