use super::crs::{self, fields_of};
use super::darktable::{self, Darktable};
use crate::model::{Edit, ModuleKind as K, Registry, SourceKey, Value};

const LIGHTROOM: &str = include_str!("../../tests/fixtures/xmp/lightroom.xmp");
const CAPTURE_ONE: &str = include_str!("../../tests/fixtures/xmp/captureone.xmp");
const DARKTABLE: &str = include_str!("../../tests/fixtures/xmp/darktable.xmp");
const DARKTABLE_OLD: &str = include_str!("../../tests/fixtures/xmp/darktable-old.xmp");
const DIGIKAM: &str = include_str!("../../tests/fixtures/xmp/digikam.xmp");

/// A fresh folder in the build's target folder (beside the test binary).
pub(crate) fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let base = exe
        .parent()
        .and_then(|d| d.parent())
        .unwrap_or(std::path::Path::new("."));
    let d = base
        .join("scratch")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A stand-in locus: the test checks the inputs, not colour science.
fn white(k: f32, tint: f32) -> (f32, f32) {
    (k / 10_000.0, tint / 100.0)
}

fn float(edit: &Edit, kind: K, name: &str) -> f32 {
    match fields_of(edit, kind).get(name) {
        Some(Value::Float(f)) => *f,
        other => panic!("{kind:?} {name}: {other:?}"),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[track_caller]
fn check(edit: &Edit, kind: K, name: &str, want: f32) {
    let got = float(edit, kind, name);
    assert!(close(got, want), "{kind:?} {name}: {got} ≠ {want}");
}

#[test]
fn lightroom_settings_map_to_their_modules() {
    let c = crs::read(LIGHTROOM).unwrap();
    assert!(c.has_edits());
    let base = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    let (e, used) = crs::import(&c, &base, true, &white);
    check(&e, K::EXPOSURE, "exposure_ev", 0.65);
    check(&e, K::VIEW_TRANSFORM, "contrast", 1.5 * 0.2f32.exp2());
    check(&e, K::TONE_EQUALIZER, "highlights_ev", -1.2);
    check(&e, K::TONE_EQUALIZER, "shadows_ev", 0.9);
    check(&e, K::VIEW_TRANSFORM, "white_target", 0.05f32.exp2());
    check(&e, K::VIEW_TRANSFORM, "black_target", -0.007);
    check(&e, K::LOCAL_CONTRAST, "texture", 0.15);
    check(&e, K::LOCAL_CONTRAST, "clarity", 0.25);
    check(&e, K::COLOUR_BALANCE, "vibrance", 0.22);
    check(&e, K::COLOUR_BALANCE, "saturation", -0.05);
    check(&e, K::SHARPEN, "amount", 0.55);
    check(&e, K::SHARPEN, "radius", 1.2);
    check(&e, K::SHARPEN, "detail", 0.3);
    check(&e, K::SHARPEN, "masking", 0.4);
    check(&e, K::DENOISE, "luminance", 0.2);
    check(&e, K::DENOISE, "detail", 0.5);
    check(&e, K::DENOISE, "colour", 0.25);
    check(&e, K::COLOUR_EQUALIZER, "hue_orange", -3.0);
    check(&e, K::COLOUR_EQUALIZER, "hue_blue", 6.0);
    check(&e, K::COLOUR_EQUALIZER, "saturation_orange", -0.15);
    check(&e, K::COLOUR_EQUALIZER, "saturation_blue", 0.3);
    check(&e, K::COLOUR_EQUALIZER, "brightness_blue", -0.25);
    check(&e, K::COLOUR_EQUALIZER, "brightness_green", 0.1);
    // Colour grading wins over the older split toning.
    check(&e, K::COLOUR_BALANCE, "shadows_hue", 210.0);
    check(&e, K::COLOUR_BALANCE, "shadows_chroma", 0.18);
    check(&e, K::COLOUR_BALANCE, "shadows_luminance", -0.05);
    check(&e, K::COLOUR_BALANCE, "highlights_hue", 45.0);
    check(&e, K::COLOUR_BALANCE, "highlights_chroma", 0.12);
    check(&e, K::COLOUR_CALIBRATION, "white_x", 0.56);
    check(&e, K::COLOUR_CALIBRATION, "white_y", 0.12);
    assert_eq!(
        fields_of(&e, K::COLOUR_CALIBRATION).get("white_source"),
        Some(&Value::Text("custom".into()))
    );
    check(&e, K::CROP_ORIENTATION, "angle_deg", -1.5);
    assert_eq!(
        fields_of(&e, K::CROP_ORIENTATION).get("crop"),
        Some(&Value::Floats(vec![0.05, 0.1, 0.95, 0.9]))
    );
    assert_eq!(
        fields_of(&e, K::CURVES).get("points"),
        Some(&Value::Floats(vec![
            0.0,
            0.0,
            64.0 / 255.0,
            56.0 / 255.0,
            192.0 / 255.0,
            200.0 / 255.0,
            1.0,
            1.0
        ]))
    );
    // An identity channel curve is left alone.
    assert!(fields_of(&e, K::CURVES).get("points_r").is_none());
    let lens = e.module(K::LENS, 0).unwrap();
    assert!(lens.enabled);
    assert_eq!(
        lens.params.to_fields().get("ca_on"),
        Some(&Value::Bool(true))
    );
    for n in [
        "Exposure2012",
        "Temperature",
        "Tint",
        "HasCrop",
        "ToneCurvePV2012",
    ] {
        assert!(used.contains(&n), "{n} not used: {used:?}");
    }
    // Not mapped: Dehaze; zero values do nothing.
    assert!(!used.contains(&"Dehaze"));
    assert!(!used.contains(&"HueAdjustmentRed"));
    assert!(!used.contains(&"SplitToningShadowSaturation"));
    // The base is untouched.
    assert_eq!(float(&base, K::EXPOSURE, "exposure_ev"), 0.0);
}

#[test]
fn white_balance_only_for_raws_and_not_as_shot() {
    let c = crs::read(LIGHTROOM).unwrap();
    let base = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    let (e, used) = crs::import(&c, &base, false, &white);
    assert!(!used.contains(&"Temperature"));
    assert_ne!(
        fields_of(&e, K::COLOUR_CALIBRATION).get("white_source"),
        Some(&Value::Text("custom".into()))
    );
    let shot = LIGHTROOM.replace(
        "crs:WhiteBalance=\"Custom\"",
        "crs:WhiteBalance=\"As Shot\"",
    );
    let c = crs::read(&shot).unwrap();
    let (_, used) = crs::import(&c, &base, true, &white);
    assert!(!used.contains(&"Temperature"));
}

#[test]
fn element_form_settings_and_lens_off() {
    // Capture One writes properties as elements.
    let c = crs::read(CAPTURE_ONE).unwrap();
    let base = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    let (e, used) = crs::import(&c, &base, true, &white);
    check(&e, K::EXPOSURE, "exposure_ev", -0.33);
    check(&e, K::VIEW_TRANSFORM, "contrast", 1.5 * (-0.1f32).exp2());
    check(&e, K::COLOUR_BALANCE, "saturation", 0.12);
    assert!(!e.module(K::LENS, 0).unwrap().enabled);
    assert!(!used.contains(&"HasCrop"));
    assert_eq!(
        used,
        vec![
            "Exposure2012",
            "Contrast2012",
            "Saturation",
            "LensProfileEnable"
        ]
    );
}

#[test]
fn split_toning_and_old_process() {
    let t = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
 crs:SplitToningShadowHue="200" crs:SplitToningShadowSaturation="30"
 crs:SplitToningHighlightHue="40" crs:SplitToningHighlightSaturation="20"/>
</rdf:RDF></x:xmpmeta>"#;
    let c = crs::read(t).unwrap();
    let base = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    let (e, _) = crs::import(&c, &base, false, &white);
    check(&e, K::COLOUR_BALANCE, "shadows_hue", 200.0);
    check(&e, K::COLOUR_BALANCE, "shadows_chroma", 0.3);
    check(&e, K::COLOUR_BALANCE, "highlights_hue", 40.0);
    check(&e, K::COLOUR_BALANCE, "highlights_chroma", 0.2);
    // Process 2010: Exposure and Contrast by their old names; Highlights2012 is ignored.
    let old = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
 crs:ProcessVersion="5.7" crs:Exposure="+1.00" crs:Contrast="+25" crs:Highlights2012="-50"/>
</rdf:RDF></x:xmpmeta>"#;
    let (e, used) = crs::import(&crs::read(old).unwrap(), &base, false, &white);
    check(&e, K::EXPOSURE, "exposure_ev", 1.0);
    assert_eq!(used, vec!["Exposure", "Contrast"]);
}

/// Transform is not carried over: its semantics are unpublished.
#[test]
fn lightroom_transform_is_not_carried_over() {
    let t = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
 crs:PerspectiveVertical="-20" crs:PerspectiveHorizontal="5" crs:PerspectiveRotate="1.5"
 crs:PerspectiveScale="95" crs:PerspectiveAspect="10" crs:PerspectiveUpright="2"
 crs:UprightVersion="151388160" crs:Exposure2012="+0.50"/>
</rdf:RDF></x:xmpmeta>"#;
    let c = crs::read(t).unwrap();
    let base = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    let (e, used) = crs::import(&c, &base, false, &white);
    assert_eq!(used, vec!["Exposure2012"]);
    assert!(e.module(K::PERSPECTIVE, 0).is_none());
}

#[test]
fn files_without_settings_offer_nothing() {
    assert!(crs::read(DIGIKAM).is_none());
    assert!(crs::read(DARKTABLE).is_none());
    assert!(crs::read("not xml").is_none());
    // Only bookkeeping: nothing to import.
    let t = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/" crs:Version="15.4" crs:HasSettings="True" crs:Exposure2012="0.00"/>
</rdf:RDF></x:xmpmeta>"#;
    assert!(!crs::read(t).unwrap().has_edits());
}

#[test]
fn darktable_modules_are_listed_by_name() {
    assert_eq!(
        darktable::detect(DARKTABLE),
        Some(Darktable {
            // history_end 7 leaves denoiseprofile out; the last vignette item turned it off.
            modules: vec![
                "rawprepare".into(),
                "exposure".into(),
                "sigmoid".into(),
                "colorbalancergb".into()
            ]
        })
    );
    assert_eq!(
        darktable::detect(DARKTABLE_OLD),
        Some(Darktable {
            modules: vec!["exposure".into(), "tonecurve".into()]
        })
    );
    for other in [LIGHTROOM, CAPTURE_ONE, DIGIKAM, "", "<x/>"] {
        assert_eq!(darktable::detect(other), None);
    }
}

#[test]
fn darktable_sidecar_beside_the_photo() {
    let dir = scratch_dir("dt-sidecar");
    let img = dir.join("IMG_2041.CR2");
    std::fs::write(&img, b"raw").unwrap();
    assert_eq!(darktable::read_for(&img), None);
    std::fs::write(dir.join("IMG_2041.CR2.xmp"), DARKTABLE).unwrap();
    assert_eq!(darktable::read_for(&img).unwrap().modules.len(), 4);
    assert_eq!(darktable::sidecar_path(&img), dir.join("IMG_2041.CR2.xmp"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn darktable_program_is_found_on_path_or_in_usual_places() {
    let dir = scratch_dir("dt-find");
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let name = if cfg!(windows) {
        "darktable.exe"
    } else {
        "darktable"
    };
    let path = std::env::join_paths([dir.join("none"), bin.clone()]).unwrap();
    assert_eq!(darktable::find_program_in(Some(&path), &[]), None);
    std::fs::write(bin.join(name), b"").unwrap();
    assert_eq!(
        darktable::find_program_in(Some(&path), &[]),
        Some(bin.join(name))
    );
    let usual = dir.join("opt-darktable");
    std::fs::write(&usual, b"").unwrap();
    assert_eq!(
        darktable::find_program_in(None, std::slice::from_ref(&usual)),
        Some(usual)
    );
    assert!(!darktable::usual_places(None).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Lightroom's Angle (stored as `crs:CropAngle`) turns the photo clockwise for positive
/// values (Adobe's Lightroom Classic help: dragging Angle right turns the photo
/// clockwise), as `angle_deg` does: the imported value keeps its sign, and a positive one
/// moves a point right of the centre downwards on screen.
#[test]
fn lightroom_crop_angle_turns_clockwise_when_positive() {
    use crate::ops::geometry::{Context, Plan};
    use crate::preview::Orient;
    let xmp = LIGHTROOM.replace("crs:CropAngle=\"-1.5\"", "crs:CropAngle=\"4\"");
    let c = crs::read(&xmp).unwrap();
    let base = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    let (e, _) = crs::import(&c, &base, true, &white);
    check(&e, K::CROP_ORIENTATION, "angle_deg", 4.0);
    let (w, h) = (600u32, 400u32);
    let plan = Plan::new(&e, &Context::image(w, h, Orient::default()), false);
    let at = |x: f64, y: f64| plan.to_output((x, y), 1.0).expect("inside");
    let centre = at(300.0, 200.0);
    let right = at(500.0, 200.0);
    let (dx, dy) = (right.0 - centre.0, right.1 - centre.1);
    let turned = dy.atan2(dx).to_degrees();
    assert!((turned - 4.0).abs() < 0.05, "turned {turned}");
}
