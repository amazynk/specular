//! Lightroom and Camera Raw settings (`crs:`), imported approximately.
//!
//! The settings come from the photo's `.xmp` sidecar or its embedded XMP. Each maps to the
//! module that does the same job, from the public description of what the slider does,
//! never from Adobe's code; the result looks alike, not the same. Nothing is written back
//! in `crs:`. The mapping (Lightroom value `v`):
//!
//! | Lightroom | Specular |
//! |---|---|
//! | Exposure2012 (EV) | exposure `exposure_ev` = v |
//! | Contrast2012 (±100) | view transform `contrast` = 1.5 · 2^(v/100) |
//! | Highlights2012, Shadows2012 (±100) | tone equalizer `highlights_ev`, `shadows_ev` = 2 · v/100 |
//! | Whites2012 (±100) | view transform `white_target` = 2^(v/200) |
//! | Blacks2012 (±100) | view transform `black_target` = 0.05 · v/100 |
//! | Temperature (K), Tint (raws only) | colour calibration white, through the caller's locus |
//! | Vibrance, Saturation (±100) | colour balance `vibrance`, `saturation` = v/100 |
//! | Clarity2012, Texture (±100) | local contrast `clarity`, `texture` = v/100 |
//! | ToneCurvePV2012 (+ Red, Green, Blue), 0–255 points | curves `points` (`_r`, `_g`, `_b`) = x/255, y/255 |
//! | HueAdjustment*, SaturationAdjustment*, LuminanceAdjustment* (±100) | colour equalizer `hue_*` = 0.3 · v degrees, `saturation_*`, `brightness_*` = v/100 |
//! | ColorGrade* (hue 0–360, saturation 0–100, luminance ±100); else SplitToning* | colour balance wheels `*_hue`, `*_chroma` = s/100, `*_luminance` = l/100 |
//! | Sharpness (0–150), SharpenRadius, SharpenDetail, SharpenEdgeMasking | sharpen `amount` = v/100, `radius`, `detail` and `masking` = v/100 |
//! | LuminanceSmoothing, LuminanceNoiseReductionDetail, ColorNoiseReduction | denoise `luminance`, `detail`, `colour` = v/100 |
//! | HasCrop, CropLeft/Top/Right/Bottom, CropAngle | crop `crop`, `angle_deg` |
//! | LensProfileEnable, AutoLateralCA | lens on/off, `ca_on` |
//!
//! Before process 2012 (ProcessVersion < 10) only Exposure, Contrast and Clarity carry over.
//!
//! Not carried over: Transform (`PerspectiveVertical`, `PerspectiveHorizontal`,
//! `PerspectiveRotate`, `PerspectiveScale`, `PerspectiveAspect`, `PerspectiveX`,
//! `PerspectiveY`, `PerspectiveUpright`, `UprightVersion` and the `Upright*` fit values).
//! Adobe's semantics for them are unpublished, so they are left unmapped rather than
//! guessed; the perspective module ([`crate::ops::geometry::persp`]) is set by hand.

use std::collections::BTreeMap;

use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, Value};

/// The Camera Raw settings namespace.
pub const NS: &str = "http://ns.adobe.com/camera-raw-settings/1.0/";

/// Lightroom's eight HSL bands, as Specular's colour equalizer names them.
const BANDS: [(&str, &str); 8] = [
    ("Red", "red"),
    ("Orange", "orange"),
    ("Yellow", "yellow"),
    ("Green", "green"),
    ("Aqua", "aqua"),
    ("Blue", "blue"),
    ("Purple", "purple"),
    ("Magenta", "magenta"),
];

/// The `crs:` settings of one photo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crs {
    /// Simple settings by name (`Exposure2012` → `+0.50`).
    pub values: BTreeMap<String, String>,
    /// Point curves by name (`ToneCurvePV2012`), 0–255 on both axes.
    pub curves: BTreeMap<String, Vec<[f32; 2]>>,
}

impl Crs {
    fn num(&self, name: &str) -> Option<f32> {
        let v = self.values.get(name)?;
        v.trim()
            .trim_start_matches('+')
            .parse::<f32>()
            .ok()
            .filter(|f| f.is_finite())
    }

    fn flag(&self, name: &str) -> Option<bool> {
        Some(matches!(
            self.values.get(name)?.trim(),
            "True" | "true" | "1"
        ))
    }

    /// Whether any setting would change the picture (not only `Version` or the like).
    pub fn has_edits(&self) -> bool {
        let mut e = Edit::empty(Default::default());
        !apply(self, &mut e, true, &|k, t| (k, t)).is_empty()
    }
}

/// The `crs:` settings in XMP text, if it has any.
pub fn read(text: &str) -> Option<Crs> {
    let doc = super::parse(text)?;
    let mut c = Crs::default();
    for (n, v) in super::simple_props(&doc, NS) {
        c.values.insert(n.to_owned(), v.to_owned());
    }
    for name in [
        "ToneCurvePV2012",
        "ToneCurvePV2012Red",
        "ToneCurvePV2012Green",
        "ToneCurvePV2012Blue",
    ] {
        let pts: Vec<[f32; 2]> = super::seq_items(&doc, NS, name)
            .into_iter()
            .filter_map(|s| {
                let (x, y) = s.split_once(',')?;
                Some([x.trim().parse().ok()?, y.trim().parse().ok()?])
            })
            .collect();
        if pts.len() >= 2 {
            c.curves.insert(name.to_owned(), pts);
        }
    }
    (!c.values.is_empty() || !c.curves.is_empty()).then_some(c)
}

/// Sets `name` of module `kind` (instance 0), adding the module at its defaults first.
fn set(edit: &mut Edit, kind: ModuleKind, name: &str, v: Value) {
    let reg = Registry::builtin();
    if edit.module(kind, 0).is_none() {
        edit.insert(reg, ModuleInstance::new(reg, kind, 0));
    }
    if let Some(m) = edit.module_mut(kind, 0) {
        let mut f = m.params.to_fields();
        f.set(name, v);
        m.params = Params::from_fields(kind, f);
        m.enabled = true;
    }
}

/// Applies `crs` to `edit` (raw: whether the photo is a raw, for its white balance).
/// `white_xy` turns a temperature (K) and a Lightroom-like tint into the white's CIE xy.
/// Returns the `crs:` settings used, by name.
pub fn apply(
    crs: &Crs,
    edit: &mut Edit,
    raw: bool,
    white_xy: &dyn Fn(f32, f32) -> (f32, f32),
) -> Vec<&'static str> {
    use ModuleKind as K;
    let mut used = Vec::new();
    let pv2012 = crs
        .values
        .get("ProcessVersion")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .is_none_or(|v| v >= 10.0);
    let f = Value::Float;
    let mut one =
        |edit: &mut Edit, name: &'static str, kind, field: &str, map: &dyn Fn(f32) -> f32| {
            one_setting(crs, edit, &mut used, name, kind, field, map);
        };
    let (exposure, contrast, clarity) = if pv2012 {
        ("Exposure2012", "Contrast2012", "Clarity2012")
    } else {
        ("Exposure", "Contrast", "Clarity")
    };
    one(edit, exposure, K::EXPOSURE, "exposure_ev", &|v| {
        v.clamp(-5.0, 5.0)
    });
    one(edit, contrast, K::VIEW_TRANSFORM, "contrast", &|v| {
        (1.5 * (v / 100.0).exp2()).clamp(0.5, 3.0)
    });
    one(edit, clarity, K::LOCAL_CONTRAST, "clarity", &|v| {
        (v / 100.0).clamp(-1.0, 1.0)
    });
    if pv2012 {
        one(
            edit,
            "Highlights2012",
            K::TONE_EQUALIZER,
            "highlights_ev",
            &|v| (2.0 * v / 100.0).clamp(-4.0, 4.0),
        );
        one(edit, "Shadows2012", K::TONE_EQUALIZER, "shadows_ev", &|v| {
            (2.0 * v / 100.0).clamp(-4.0, 4.0)
        });
        one(
            edit,
            "Whites2012",
            K::VIEW_TRANSFORM,
            "white_target",
            &|v| (v / 200.0).exp2().clamp(0.5, 2.0),
        );
        one(
            edit,
            "Blacks2012",
            K::VIEW_TRANSFORM,
            "black_target",
            &|v| (0.05 * v / 100.0).clamp(-0.1, 0.1),
        );
        one(edit, "Texture", K::LOCAL_CONTRAST, "texture", &|v| {
            (v / 100.0).clamp(-1.0, 1.0)
        });
        one(edit, "Vibrance", K::COLOUR_BALANCE, "vibrance", &|v| {
            (v / 100.0).clamp(-1.0, 1.0)
        });
        one(edit, "Saturation", K::COLOUR_BALANCE, "saturation", &|v| {
            (v / 100.0).clamp(-1.0, 1.0)
        });
        let pct = |v: f32| (v / 100.0).clamp(0.0, 1.0);
        one(edit, "LuminanceSmoothing", K::DENOISE, "luminance", &pct);
        one(
            edit,
            "LuminanceNoiseReductionDetail",
            K::DENOISE,
            "detail",
            &pct,
        );
        one(edit, "ColorNoiseReduction", K::DENOISE, "colour", &pct);
        one(edit, "Sharpness", K::SHARPEN, "amount", &|v| {
            (v / 100.0).clamp(0.0, 2.0)
        });
        if crs.num("Sharpness").is_some_and(|v| v != 0.0) {
            one(edit, "SharpenRadius", K::SHARPEN, "radius", &|v| {
                v.clamp(0.3, 3.0)
            });
            one(edit, "SharpenDetail", K::SHARPEN, "detail", &pct);
            one(edit, "SharpenEdgeMasking", K::SHARPEN, "masking", &pct);
        }
        for (lr, ours) in BANDS {
            let hue: &'static str = band_name("HueAdjustment", lr);
            let sat: &'static str = band_name("SaturationAdjustment", lr);
            let lum: &'static str = band_name("LuminanceAdjustment", lr);
            one(
                edit,
                hue,
                K::COLOUR_EQUALIZER,
                &format!("hue_{ours}"),
                &|v| (0.3 * v).clamp(-45.0, 45.0),
            );
            let unit = |v: f32| (v / 100.0).clamp(-1.0, 1.0);
            one(
                edit,
                sat,
                K::COLOUR_EQUALIZER,
                &format!("saturation_{ours}"),
                &unit,
            );
            one(
                edit,
                lum,
                K::COLOUR_EQUALIZER,
                &format!("brightness_{ours}"),
                &unit,
            );
        }
        grading(crs, edit, &mut used);
        for (name, field) in [
            ("ToneCurvePV2012", "points"),
            ("ToneCurvePV2012Red", "points_r"),
            ("ToneCurvePV2012Green", "points_g"),
            ("ToneCurvePV2012Blue", "points_b"),
        ] {
            let Some(pts) = crs.curves.get(name) else {
                continue;
            };
            let identity = pts.len() == 2 && pts[0] == [0.0, 0.0] && pts[1] == [255.0, 255.0];
            if identity {
                continue;
            }
            let mut flat = Vec::with_capacity(pts.len() * 2);
            for p in pts {
                flat.push((p[0] / 255.0).clamp(0.0, 1.0));
                flat.push((p[1] / 255.0).clamp(0.0, 1.0));
            }
            set(edit, K::CURVES, field, Value::Floats(flat));
            used.push(match field {
                "points" => "ToneCurvePV2012",
                "points_r" => "ToneCurvePV2012Red",
                "points_g" => "ToneCurvePV2012Green",
                _ => "ToneCurvePV2012Blue",
            });
        }
    }
    // White balance: a raw's absolute temperature; a rendered file's is relative, left out.
    if raw
        && crs
            .values
            .get("WhiteBalance")
            .is_none_or(|w| w != "As Shot")
        && let Some(k) = crs.num("Temperature")
    {
        let tint = crs.num("Tint").unwrap_or(0.0);
        let (x, y) = white_xy(k.clamp(2000.0, 50000.0), tint.clamp(-150.0, 150.0));
        set(
            edit,
            K::COLOUR_CALIBRATION,
            "white_source",
            Value::Text("custom".into()),
        );
        set(edit, K::COLOUR_CALIBRATION, "white_x", f(x));
        set(edit, K::COLOUR_CALIBRATION, "white_y", f(y));
        used.push("Temperature");
        if crs.values.contains_key("Tint") {
            used.push("Tint");
        }
    }
    if crs.flag("HasCrop") == Some(true) {
        let side = |n: &str, d: f32| crs.num(n).unwrap_or(d).clamp(0.0, 1.0);
        let c = [
            side("CropLeft", 0.0),
            side("CropTop", 0.0),
            side("CropRight", 1.0),
            side("CropBottom", 1.0),
        ];
        if c[0] < c[2] && c[1] < c[3] {
            set(edit, K::CROP_ORIENTATION, "crop", Value::Floats(c.to_vec()));
            used.push("HasCrop");
        }
        if let Some(a) = crs.num("CropAngle")
            && a != 0.0
        {
            set(
                edit,
                K::CROP_ORIENTATION,
                "angle_deg",
                f(a.clamp(-45.0, 45.0)),
            );
            used.push("CropAngle");
        }
    }
    if let Some(on) = crs.flag("LensProfileEnable") {
        set(edit, K::LENS, "embedded", Value::Bool(on));
        if let Some(m) = edit.module_mut(K::LENS, 0) {
            m.enabled = on;
        }
        used.push("LensProfileEnable");
    }
    if let Some(ca) = crs.flag("AutoLateralCA")
        && edit.module(K::LENS, 0).is_some_and(|m| m.enabled)
    {
        set(edit, K::LENS, "ca_on", Value::Bool(ca));
        used.push("AutoLateralCA");
    }
    used
}

/// Sets `field` from setting `name` through `map` when the setting is there and not 0.
fn one_setting(
    crs: &Crs,
    edit: &mut Edit,
    used: &mut Vec<&'static str>,
    name: &'static str,
    kind: ModuleKind,
    field: &str,
    map: &dyn Fn(f32) -> f32,
) {
    if let Some(v) = crs.num(name)
        && v != 0.0
    {
        set(edit, kind, field, Value::Float(map(v)));
        used.push(name);
    }
}

/// Colour grading (Lightroom 10 and later), else split toning, onto the colour balance wheels.
fn grading(crs: &Crs, edit: &mut Edit, used: &mut Vec<&'static str>) {
    use ModuleKind as K;
    const GRADE: [(&str, &str, &str, &str); 4] = [
        (
            "shadows",
            "ColorGradeShadowHue",
            "ColorGradeShadowSat",
            "ColorGradeShadowLum",
        ),
        (
            "midtones",
            "ColorGradeMidtoneHue",
            "ColorGradeMidtoneSat",
            "ColorGradeMidtoneLum",
        ),
        (
            "highlights",
            "ColorGradeHighlightHue",
            "ColorGradeHighlightSat",
            "ColorGradeHighlightLum",
        ),
        (
            "global",
            "ColorGradeGlobalHue",
            "ColorGradeGlobalSat",
            "ColorGradeGlobalLum",
        ),
    ];
    let graded = GRADE.iter().any(|(_, _, s, l)| {
        crs.num(s).is_some_and(|v| v != 0.0) || crs.num(l).is_some_and(|v| v != 0.0)
    });
    let wheel = |edit: &mut Edit, range: &str, hue: f32, sat: f32, lum: f32| {
        set(
            edit,
            K::COLOUR_BALANCE,
            &format!("{range}_hue"),
            Value::Float(hue.rem_euclid(360.0)),
        );
        set(
            edit,
            K::COLOUR_BALANCE,
            &format!("{range}_chroma"),
            Value::Float((sat / 100.0).clamp(0.0, 1.0)),
        );
        set(
            edit,
            K::COLOUR_BALANCE,
            &format!("{range}_luminance"),
            Value::Float((lum / 100.0).clamp(-1.0, 1.0)),
        );
    };
    if graded {
        for (range, h, s, l) in GRADE {
            let (sat, lum) = (crs.num(s).unwrap_or(0.0), crs.num(l).unwrap_or(0.0));
            if sat != 0.0 || lum != 0.0 {
                wheel(edit, range, crs.num(h).unwrap_or(0.0), sat, lum);
                used.push(s);
            }
        }
        return;
    }
    for (range, h, s) in [
        (
            "shadows",
            "SplitToningShadowHue",
            "SplitToningShadowSaturation",
        ),
        (
            "highlights",
            "SplitToningHighlightHue",
            "SplitToningHighlightSaturation",
        ),
    ] {
        if let Some(sat) = crs.num(s).filter(|v| *v != 0.0) {
            wheel(edit, range, crs.num(h).unwrap_or(0.0), sat, 0.0);
            used.push(s);
        }
    }
}

fn band_name(prefix: &str, band: &str) -> &'static str {
    // The 24 names, so `used` can hold `&'static str`.
    const NAMES: [&str; 24] = [
        "HueAdjustmentRed",
        "HueAdjustmentOrange",
        "HueAdjustmentYellow",
        "HueAdjustmentGreen",
        "HueAdjustmentAqua",
        "HueAdjustmentBlue",
        "HueAdjustmentPurple",
        "HueAdjustmentMagenta",
        "SaturationAdjustmentRed",
        "SaturationAdjustmentOrange",
        "SaturationAdjustmentYellow",
        "SaturationAdjustmentGreen",
        "SaturationAdjustmentAqua",
        "SaturationAdjustmentBlue",
        "SaturationAdjustmentPurple",
        "SaturationAdjustmentMagenta",
        "LuminanceAdjustmentRed",
        "LuminanceAdjustmentOrange",
        "LuminanceAdjustmentYellow",
        "LuminanceAdjustmentGreen",
        "LuminanceAdjustmentAqua",
        "LuminanceAdjustmentBlue",
        "LuminanceAdjustmentPurple",
        "LuminanceAdjustmentMagenta",
    ];
    NAMES
        .iter()
        .find(|n| n.strip_prefix(prefix) == Some(band))
        .copied()
        .unwrap_or("")
}

/// A fresh edit with `crs` applied: the photo's default stack, then the settings.
pub fn import(
    crs: &Crs,
    base: &Edit,
    raw: bool,
    white_xy: &dyn Fn(f32, f32) -> (f32, f32),
) -> (Edit, Vec<&'static str>) {
    let mut e = base.clone();
    let used = apply(crs, &mut e, raw, white_xy);
    (e, used)
}

/// Fields of a module after import, for tests and the Info panel.
pub fn fields_of(edit: &Edit, kind: ModuleKind) -> Fields {
    edit.module(kind, 0)
        .map(|m| m.params.to_fields())
        .unwrap_or_default()
}
