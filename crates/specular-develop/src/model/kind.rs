//! Module kinds and the registry: stable ids and names, pipeline order, param versions and
//! migrations.
//!
//! Ids and names are stored (catalogue, sidecar) and hashed, so they never change once
//! released. A kind the registry does not know (from a newer Specular) is kept as is.

use std::sync::LazyLock;

use super::fields::Fields;
use super::params;

/// A module kind: a stable 16-bit id. Unknown ids are valid and round-trip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleKind(pub u16);

impl ModuleKind {
    // The alpha modules (typed params in `params`).
    pub const RAW_FRONT_END: Self = Self(1);
    pub const EXPOSURE: Self = Self(2);
    pub const COLOUR_CALIBRATION: Self = Self(3);
    pub const VIEW_TRANSFORM: Self = Self(4);
    pub const CROP_ORIENTATION: Self = Self(5);
    // Reserved for later lanes (params as plain fields until then).
    pub const LENS: Self = Self(6);
    pub const DENOISE: Self = Self(7);
    pub const TONE_EQUALIZER: Self = Self(8);
    pub const LOCAL_CONTRAST: Self = Self(9);
    pub const COLOUR_BALANCE: Self = Self(10);
    pub const COLOUR_EQUALIZER: Self = Self(11);
    pub const CURVES: Self = Self(12);
    pub const LUT_3D: Self = Self(13);
    pub const SHARPEN: Self = Self(14);
    /// Keystone and upright, in the fused warp (16–19 are kept free).
    pub const PERSPECTIVE: Self = Self(15);
    pub const POINT_COLOUR: Self = Self(20);
    pub const BLACK_WHITE: Self = Self(21);
    // Detail (22–25).
    pub const CAPTURE_SHARPENING: Self = Self(22);
    // Diffuse or sharpen, surface blur (in the detail range).
    pub const DIFFUSE: Self = Self(23);
    pub const SURFACE_BLUR: Self = Self(24);
    // Corrections.
    pub const HOT_PIXELS: Self = Self(26);
    pub const CHROMATIC_ABERRATION: Self = Self(27);
    pub const DEFRINGE: Self = Self(28);
    pub const FLAT_FIELD: Self = Self(29);
    pub const RETOUCH: Self = Self(30);
    // Effects (30 is retouch's).
    pub const NEGATIVE: Self = Self(40);
    pub const DEHAZE: Self = Self(41);
    pub const GRADUATED_ND: Self = Self(42);
    pub const VIGNETTE: Self = Self(43);
    pub const GLOW: Self = Self(44);
    pub const GRAIN: Self = Self(45);
    /// Borders and watermark: exports and prints only.
    pub const FRAME: Self = Self(46);
    // The depth tools (47–49 left for the effects lanes).
    pub const RELIGHT: Self = Self(50);
    pub const DEPTH_BLUR: Self = Self(51);
}

/// Where a module runs in the data flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// On the CFA mosaic up to the working space (checkpoint C1).
    Raw,
    /// Neighbourhood ops in sensor geometry (checkpoint C2).
    SensorGeometry,
    /// Part of the one fused warp (checkpoint C3).
    Warp,
    /// Scene-linear ops in output geometry.
    Scene,
    /// The view transform and output space.
    View,
    /// Added to exports and prints after rendering (borders, watermark): not a graph node.
    Output,
}

/// One param migration: fields of version `from` become version `from + 1`.
#[derive(Clone, Copy)]
pub struct Migration {
    pub from: u16,
    pub run: fn(&mut Fields),
}

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Migration({}→{})", self.from, self.from + 1)
    }
}

/// What the registry knows about one module kind.
#[derive(Clone, Debug)]
pub struct ModuleSpec {
    pub kind: ModuleKind,
    /// Stable `snake_case` name: the sidecar element and the style/preset key.
    pub name: &'static str,
    pub stage: Stage,
    /// Default pipeline position; lower runs first.
    pub rank: u16,
    /// The param version new instances get.
    pub current: u16,
    /// Versions with a live kernel, oldest first: one or two, `current` last.
    pub live: &'static [u16],
    /// `from → from + 1` for every version below `current`.
    pub migrations: &'static [Migration],
    /// Geometry is left out of styles and pastes by default.
    pub geometry: bool,
    /// Fields of a new instance.
    pub defaults: fn() -> Fields,
}

impl ModuleSpec {
    /// The oldest version that renders without migrating.
    pub fn oldest_live(&self) -> u16 {
        self.live.first().copied().unwrap_or(self.current)
    }

    pub fn migration_from(&self, v: u16) -> Option<&Migration> {
        self.migrations.iter().find(|m| m.from == v)
    }
}

/// Why a spec was not registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateKind(ModuleKind),
    DuplicateName(&'static str),
    /// `live` must hold one or two versions, ascending, ending at `current`.
    BadLive(ModuleKind),
    /// A version below `current` has no migration.
    MissingMigration(ModuleKind, u16),
}

/// The module kinds Specular knows. [`Registry::builtin`] has the shipped set; later lanes add
/// theirs there, tests build their own.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    specs: Vec<ModuleSpec>,
}

impl Registry {
    pub fn empty() -> Self {
        Self::default()
    }

    /// The shipped modules.
    pub fn builtin() -> &'static Registry {
        static BUILTIN: LazyLock<Registry> = LazyLock::new(|| {
            let mut r = Registry::empty();
            for s in builtin_specs() {
                r.register(s).expect("built-in module specs are valid");
            }
            r
        });
        &BUILTIN
    }

    pub fn register(&mut self, spec: ModuleSpec) -> Result<(), RegistryError> {
        if self.get(spec.kind).is_some() {
            return Err(RegistryError::DuplicateKind(spec.kind));
        }
        if self.by_name(spec.name).is_some() {
            return Err(RegistryError::DuplicateName(spec.name));
        }
        let live_ok = matches!(spec.live.len(), 1 | 2)
            && spec.live.windows(2).all(|w| w[0] < w[1])
            && spec.live.last() == Some(&spec.current);
        if !live_ok {
            return Err(RegistryError::BadLive(spec.kind));
        }
        if let Some(v) = (1..spec.current).find(|&v| spec.migration_from(v).is_none()) {
            return Err(RegistryError::MissingMigration(spec.kind, v));
        }
        self.specs.push(spec);
        Ok(())
    }

    pub fn get(&self, kind: ModuleKind) -> Option<&ModuleSpec> {
        self.specs.iter().find(|s| s.kind == kind)
    }

    pub fn by_name(&self, name: &str) -> Option<&ModuleSpec> {
        self.specs.iter().find(|s| s.name == name)
    }

    pub fn specs(&self) -> &[ModuleSpec] {
        &self.specs
    }

    /// Pipeline rank; unknown kinds sort last, by id.
    pub fn rank(&self, kind: ModuleKind) -> u32 {
        self.get(kind)
            .map_or(0x1_0000 + u32::from(kind.0), |s| u32::from(s.rank))
    }
}

const fn spec(
    kind: ModuleKind,
    name: &'static str,
    stage: Stage,
    rank: u16,
    geometry: bool,
    defaults: fn() -> Fields,
) -> ModuleSpec {
    ModuleSpec {
        kind,
        name,
        stage,
        rank,
        current: 1,
        live: &[1],
        migrations: &[],
        geometry,
        defaults,
    }
}

fn no_fields() -> Fields {
    Fields::new()
}

fn builtin_specs() -> Vec<ModuleSpec> {
    use ModuleKind as K;
    use Stage::*;
    vec![
        spec(
            K::FLAT_FIELD,
            "flat_field",
            Raw,
            90,
            false,
            crate::ops::correct::FlatField::default_fields,
        ),
        spec(
            K::HOT_PIXELS,
            "hot_pixels",
            Raw,
            92,
            false,
            crate::ops::correct::HotPixels::default_fields,
        ),
        spec(
            K::CHROMATIC_ABERRATION,
            "chromatic_aberration",
            Raw,
            94,
            false,
            crate::ops::correct::ChromaticAberration::default_fields,
        ),
        spec(
            K::RAW_FRONT_END,
            "raw_front_end",
            Raw,
            100,
            false,
            params::defaults::<params::RawFrontEnd>,
        ),
        spec(
            K::DENOISE,
            "denoise",
            SensorGeometry,
            200,
            false,
            crate::ops::detail::denoise::Denoise::default_fields,
        ),
        spec(
            K::DEFRINGE,
            "defringe",
            SensorGeometry,
            205,
            false,
            crate::ops::correct::defringe::Defringe::default_fields,
        ),
        spec(
            K::CAPTURE_SHARPENING,
            "capture_sharpening",
            SensorGeometry,
            210,
            false,
            crate::ops::detail::capture::Capture::default_fields,
        ),
        spec(
            K::LENS,
            "lens",
            Warp,
            300,
            true,
            params::defaults::<crate::ops::geometry::LensParams>,
        ),
        spec(
            K::PERSPECTIVE,
            "perspective",
            Warp,
            310,
            true,
            params::defaults::<crate::ops::geometry::persp::PerspParams>,
        ),
        spec(
            K::CROP_ORIENTATION,
            "crop_orientation",
            Warp,
            320,
            true,
            params::defaults::<params::CropOrientation>,
        ),
        spec(
            K::RETOUCH,
            "retouch",
            Scene,
            390,
            false,
            crate::ops::retouch::Retouch::default_fields,
        ),
        spec(
            K::EXPOSURE,
            "exposure",
            Scene,
            400,
            false,
            params::defaults::<params::Exposure>,
        ),
        spec(
            K::COLOUR_CALIBRATION,
            "colour_calibration",
            Scene,
            410,
            false,
            params::defaults::<params::ColourCalibration>,
        ),
        spec(
            K::SURFACE_BLUR,
            "surface_blur",
            Scene,
            415,
            false,
            crate::ops::detail::surface::SurfaceBlur::default_fields,
        ),
        spec(
            K::TONE_EQUALIZER,
            "tone_equalizer",
            Scene,
            420,
            false,
            no_fields,
        ),
        spec(
            K::DIFFUSE,
            "diffuse",
            Scene,
            425,
            false,
            crate::ops::detail::diffuse::Diffuse::default_fields,
        ),
        spec(
            K::LOCAL_CONTRAST,
            "local_contrast",
            Scene,
            430,
            false,
            no_fields,
        ),
        spec(
            K::COLOUR_BALANCE,
            "colour_balance",
            Scene,
            440,
            false,
            no_fields,
        ),
        spec(
            K::COLOUR_EQUALIZER,
            "colour_equalizer",
            Scene,
            450,
            false,
            no_fields,
        ),
        spec(
            K::POINT_COLOUR,
            "point_colour",
            Scene,
            452,
            false,
            no_fields,
        ),
        spec(K::BLACK_WHITE, "black_white", Scene, 455, false, no_fields),
        spec(K::CURVES, "curves", Scene, 460, false, no_fields),
        spec(K::LUT_3D, "lut_3d", Scene, 470, false, no_fields),
        spec(
            K::SHARPEN,
            "sharpen",
            Scene,
            480,
            false,
            crate::ops::detail::sharpen::Sharpen::default_fields,
        ),
        spec(
            K::NEGATIVE,
            "negative",
            Scene,
            395,
            false,
            crate::ops::effects::negative::Negative::default_fields,
        ),
        spec(
            K::DEHAZE,
            "dehaze",
            Scene,
            415,
            false,
            crate::ops::effects::dehaze::Dehaze::default_fields,
        ),
        spec(
            K::GRADUATED_ND,
            "graduated_nd",
            Scene,
            416,
            false,
            crate::ops::effects::graduated::Graduated::default_fields,
        ),
        spec(
            K::VIGNETTE,
            "vignette",
            Scene,
            484,
            false,
            crate::ops::effects::vignette::Vignette::default_fields,
        ),
        spec(
            K::GLOW,
            "glow",
            Scene,
            486,
            false,
            crate::ops::effects::glow::Glow::default_fields,
        ),
        spec(
            K::GRAIN,
            "grain",
            Scene,
            488,
            false,
            crate::ops::effects::grain::Grain::default_fields,
        ),
        spec(
            K::RELIGHT,
            "relight",
            Scene,
            405,
            false,
            crate::ops::depth::relight::Relight::default_fields,
        ),
        spec(
            K::DEPTH_BLUR,
            "depth_blur",
            Scene,
            418,
            false,
            crate::ops::depth::blur::DepthBlur::default_fields,
        ),
        spec(
            K::VIEW_TRANSFORM,
            "view_transform",
            View,
            900,
            false,
            params::defaults::<params::ViewTransform>,
        ),
        spec(
            K::FRAME,
            "frame",
            Output,
            950,
            false,
            crate::ops::effects::frame::Frame::default_fields,
        ),
    ]
}
