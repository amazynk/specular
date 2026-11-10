//! The parser and the lookup on hand-written database samples, and the shipped database.

use crate::lookup::{Pick, Query, find, find_camera};
use crate::model::{Distortion, Tca};
use crate::{Db, LensType};

/// A small database in Lensfun's format (made up; the shapes the real files use).
const SAMPLE: &str = r#"<!DOCTYPE lensdatabase SYSTEM "lensfun-database.dtd">
<lensdatabase version="2">
    <mount>
        <name>Test EF</name>
        <compat>M42</compat>
    </mount>
    <mount>
        <name>Test EF-S</name>
        <compat>Test EF</compat>
    </mount>
    <camera>
        <maker>Testmaker</maker>
        <model>Testmaker Body 5</model>
        <model lang="en">Body 5</model>
        <mount>Test EF</mount>
        <cropfactor>1</cropfactor>
    </camera>
    <camera>
        <maker>Testmaker</maker>
        <model>Testmaker Body 70</model>
        <mount>Test EF-S</mount>
        <cropfactor>1.6</cropfactor>
    </camera>
    <camera>
        <maker>Pocket Corporation</maker>
        <maker lang="en">Pocket</maker>
        <model>Pocket P1</model>
        <mount>pocketP1</mount>
        <cropfactor>4.5</cropfactor>
    </camera>
    <lens>
        <maker>Testmaker</maker>
        <model>Testmaker EF 24-70mm f/2.8L II USM</model>
        <mount>Test EF</mount>
        <cropfactor>1</cropfactor>
        <calibration>
            <!-- comments are fine -->
            <distortion model="ptlens" focal="24" a="0.02" b="-0.08" c="0.03"/>
            <distortion model="ptlens" focal="70" a="0.01" b="-0.02" c="0.02"/>
            <tca model="poly3" focal="24" br="0.0001" vr="1.0002" bb="-0.0001" vb="0.9998"/>
            <vignetting model="pa" focal="24" aperture="2.8" distance="1000" k1="-0.5" k2="0.1" k3="-0.05"/>
            <vignetting model="pa" focal="24" aperture="8" distance="1000" k1="-0.1" k2="0" k3="0"/>
        </calibration>
    </lens>
    <lens>
        <maker>Testmaker</maker>
        <model>Testmaker EF 24-70mm f/2.8L USM</model>
        <mount>Test EF</mount>
        <cropfactor>1</cropfactor>
        <calibration>
            <distortion model="poly3" focal="24" k1="-0.02"/>
        </calibration>
    </lens>
    <lens>
        <maker>Testmaker</maker>
        <model>Testmaker EF 50mm f/1.8 II</model>
        <mount>Test EF</mount>
        <cropfactor>1</cropfactor>
        <calibration><distortion model="poly3" focal="50" k1="-0.01"/></calibration>
    </lens>
    <lens>
        <maker>Testmaker</maker>
        <model>Testmaker EF 50mm f/1.8 STM</model>
        <mount>Test EF</mount>
        <cropfactor>1</cropfactor>
        <calibration><distortion model="poly3" focal="50" k1="-0.012"/></calibration>
    </lens>
    <lens>
        <maker>Testmaker</maker>
        <model>Testmaker EF 50mm f/1.8 STM</model>
        <mount>Test EF</mount>
        <cropfactor>1.6</cropfactor>
        <calibration><distortion model="poly3" focal="50" k1="-0.013"/></calibration>
    </lens>
    <lens>
        <maker>Thirdparty</maker>
        <model>Thirdparty 18-35mm F1.8 DC HSM</model>
        <mount>Test EF-S</mount>
        <cropfactor>1.534</cropfactor>
        <aspect-ratio>3:2</aspect-ratio>
        <type>fisheye</type>
        <calibration>
            <tca model="linear" focal="18" kr="1.001" kb="0.999"/>
            <distortion model="acm" focal="18" k1="0.1"/>
        </calibration>
    </lens>
    <lens>
        <maker>Testmaker</maker>
        <model>Testmaker EF 100mm f/2.8 Macro (uncalibrated)</model>
        <mount>Test EF</mount>
        <cropfactor>1</cropfactor>
    </lens>
    <lens>
        <maker>Pocket</maker>
        <model>fixed lens</model>
        <mount>pocketP1</mount>
        <cropfactor>4.5</cropfactor>
        <aspect-ratio>4:3</aspect-ratio>
        <focal min="4.3" max="21.5"/>
        <calibration><distortion model="poly5" focal="4.3" k1="-0.05" k2="0.01"/></calibration>
    </lens>
</lensdatabase>
"#;

fn sample() -> Db {
    Db::from_xml(SAMPLE).expect("sample parses")
}

fn lens_named(db: &Db, i: usize) -> &str {
    &db.lenses[i].model
}

#[test]
fn parses_mounts_cameras_lenses() {
    let db = sample();
    assert_eq!(db.mounts.len(), 2);
    assert_eq!(db.mount("test ef-s").expect("mount").compat, ["Test EF"]);
    assert_eq!(db.cameras.len(), 3);
    let c = &db.cameras[0];
    assert_eq!(c.model, "Testmaker Body 5");
    assert_eq!(c.model_en, "Body 5");
    assert_eq!(c.crop, 1.0);
    assert_eq!(db.cameras[2].maker_en, "Pocket");
    assert_eq!(db.lenses.len(), 8);
    let l = &db.lenses[0];
    assert_eq!(l.focal, Some((24.0, 70.0)), "from the name");
    assert_eq!(l.distortion.len(), 2);
    assert_eq!(
        l.distortion[0].model,
        Distortion::PtLens {
            a: 0.02,
            b: -0.08,
            c: 0.03
        }
    );
    assert_eq!(
        l.tca[0].model,
        Tca {
            red: [0.0001, 0.0, 1.0002],
            blue: [-0.0001, 0.0, 0.9998]
        }
    );
    assert_eq!(l.vignetting.len(), 2);
    assert_eq!(l.vignetting[0].k, [-0.5, 0.1, -0.05]);
    assert_eq!(l.aspect, 1.5, "the default");
    let t = &db.lenses[5];
    assert_eq!(t.kind, LensType::Fisheye);
    assert_eq!(t.aspect, 1.5);
    assert_eq!(t.tca[0].model, Tca::linear(1.001, 0.999));
    assert!(t.distortion.is_empty(), "acm is not read");
    let p = &db.lenses[7];
    assert_eq!(p.focal, Some((4.3, 21.5)), "from <focal>");
    assert!((p.aspect - 4.0 / 3.0).abs() < 1e-12);
    assert_eq!(
        p.distortion[0].model,
        Distortion::Poly5 {
            k1: -0.05,
            k2: 0.01
        }
    );
    assert!(!db.lenses[6].has_calibration());
    assert!(db.errors.is_empty());
}

#[test]
fn bad_files_are_listed() {
    assert!(Db::from_xml("<notadb/>").is_err());
    assert!(Db::from_xml("<lensdatabase>").is_err());
    let db = Db::from_files([("a.xml", SAMPLE), ("b.xml", "garbage")]);
    assert_eq!(db.errors.len(), 1);
    assert!(db.errors[0].starts_with("b.xml"));
    assert_eq!(db.cameras.len(), 3);
}

#[test]
fn later_definitions_replace_earlier_ones() {
    let extra = r#"<lensdatabase version="2"><camera><maker>Testmaker</maker>
        <model>Testmaker Body 5</model><mount>Test EF</mount><cropfactor>1.3</cropfactor>
        </camera></lensdatabase>"#;
    let db = Db::from_files([("a.xml", SAMPLE), ("b.xml", extra)]);
    let i = find_camera(&db, "Testmaker", "Testmaker Body 5").expect("camera");
    assert_eq!(db.cameras[i].crop, 1.3);
    assert_eq!(db.cameras.len(), 3);
}

#[test]
fn camera_names_match_loosely() {
    let db = sample();
    let m = |a, b| find_camera(&db, a, b).map(|i| db.cameras[i].model.clone());
    assert_eq!(
        m("Testmaker", "Testmaker Body 5").as_deref(),
        Some("Testmaker Body 5")
    );
    assert_eq!(
        m("TESTMAKER ", "testmaker  body 5\0").as_deref(),
        Some("Testmaker Body 5")
    );
    // The model without the maker's name.
    assert_eq!(
        m("Testmaker", "Body 5").as_deref(),
        Some("Testmaker Body 5")
    );
    // The maker by its first word.
    assert_eq!(m("POCKET", "Pocket P1").as_deref(), Some("Pocket P1"));
    assert_eq!(m("Testmaker", "Body 6"), None);
    assert_eq!(m("Other", "Testmaker Body 5"), None);
}

#[test]
fn lens_names_pick_one() {
    let db = sample();
    let q = Query {
        maker: "Testmaker",
        model: "Testmaker Body 5",
        lens: Some("EF24-70mm f/2.8L II USM"),
        focal: Some(35.0),
        focal35: None,
    };
    let r = find(&db, &q);
    assert_eq!(r.pick, Pick::One);
    assert_eq!(
        lens_named(&db, r.candidates[0]),
        "Testmaker EF 24-70mm f/2.8L II USM"
    );
    assert_eq!(r.crop, Some(1.0));
    // Without "II": both match, the one without "II" shares more of its words.
    let r = find(
        &db,
        &Query {
            lens: Some("EF24-70mm f/2.8L USM"),
            ..q
        },
    );
    assert_eq!(r.pick, Pick::One);
    assert_eq!(
        lens_named(&db, r.candidates[0]),
        "Testmaker EF 24-70mm f/2.8L USM"
    );
    assert_eq!(
        lens_named(&db, r.candidates[1]),
        "Testmaker EF 24-70mm f/2.8L II USM"
    );
}

#[test]
fn ambiguous_names_pick_none() {
    let db = sample();
    let r = find(
        &db,
        &Query {
            maker: "Testmaker",
            model: "Testmaker Body 5",
            lens: Some("EF50mm f/1.8"),
            focal: Some(50.0),
            focal35: None,
        },
    );
    assert_eq!(r.pick, Pick::Ambiguous);
    assert_eq!(r.tied, 2);
    assert_eq!(r.picked(), None);
    let names: Vec<&str> = r.candidates.iter().map(|&i| lens_named(&db, i)).collect();
    assert_eq!(
        names[..2],
        ["Testmaker EF 50mm f/1.8 II", "Testmaker EF 50mm f/1.8 STM"]
    );
}

#[test]
fn one_calibration_per_lens_by_crop_factor() {
    let db = sample();
    let q = |model| Query {
        maker: "Testmaker",
        model,
        lens: Some("EF50mm f/1.8 STM"),
        focal: Some(50.0),
        focal35: None,
    };
    // Full frame: the full-frame calibration (1.6 is larger than 1).
    let r = find(&db, &q("Testmaker Body 5"));
    assert_eq!(r.pick, Pick::One);
    assert_eq!(db.lenses[r.candidates[0]].crop, 1.0);
    // On the 1.6 body (EF-S, EF lenses fit): the 1.6 calibration.
    let r = find(&db, &q("Testmaker Body 70"));
    assert_eq!(r.pick, Pick::One);
    assert_eq!(db.lenses[r.candidates[0]].crop, 1.6);
}

#[test]
fn no_match_and_no_name() {
    let db = sample();
    let base = Query {
        maker: "Testmaker",
        model: "Testmaker Body 5",
        lens: Some("EF24-105mm f/4L IS USM"),
        focal: Some(50.0),
        focal35: None,
    };
    let r = find(&db, &base);
    assert_eq!(r.pick, Pick::NoMatch);
    assert_eq!(r.picked(), None);
    // The picker still offers lenses that share words.
    assert!(!r.candidates.is_empty());
    // An uncalibrated lens is never offered.
    assert!(r.candidates.iter().all(|&i| db.lenses[i].has_calibration()));
    // No name: ask, never guess.
    for lens in [None, Some(""), Some("----"), Some("0mm f/0")] {
        let r = find(&db, &Query { lens, ..base });
        assert_eq!(r.pick, Pick::NoLensName, "{lens:?}");
        assert!(r.picked().is_none());
        // 50 mm: the 24-70s and the 50s, the maker's primes of that length first (a long
        // list is cut, and the lens should not be cut off by name order).
        assert_eq!(r.candidates.len(), 4, "{lens:?}");
        let spans: Vec<f64> = r
            .candidates
            .iter()
            .map(|&i| db.lenses[i].focal.map_or(f64::MAX, |(lo, hi)| hi - lo))
            .collect();
        let maker = |i: usize| db.lenses[i].maker_en == "Testmaker";
        let own: Vec<f64> = r
            .candidates
            .iter()
            .zip(&spans)
            .filter(|(i, _)| maker(**i))
            .map(|(_, s)| *s)
            .collect();
        assert!(own.windows(2).all(|w| w[0] <= w[1]), "{spans:?}");
    }
    // Focal length out of range: the 24-70 does not fit at 200 mm.
    let r = find(
        &db,
        &Query {
            lens: Some("EF24-70mm f/2.8L II USM"),
            focal: Some(200.0),
            ..base
        },
    );
    assert_eq!(r.pick, Pick::NoMatch);
    // A lens of another maker named in the Exif string does not match ours.
    let r = find(
        &db,
        &Query {
            model: "Testmaker Body 70",
            lens: Some("Otherbrand 18-35mm F1.8 DC HSM"),
            focal: Some(18.0),
            ..base
        },
    );
    assert_eq!(r.pick, Pick::NoMatch);
    let r = find(
        &db,
        &Query {
            model: "Testmaker Body 70",
            lens: Some("18-35mm F1.8 DC HSM"),
            focal: Some(18.0),
            ..base
        },
    );
    assert_eq!(r.pick, Pick::One);
    assert_eq!(db.lenses[r.candidates[0]].maker, "Thirdparty");
}

#[test]
fn fixed_lens_and_unknown_cameras() {
    let db = sample();
    let r = find(
        &db,
        &Query {
            maker: "Pocket Corporation",
            model: "Pocket P1",
            lens: None,
            focal: Some(8.0),
            focal35: None,
        },
    );
    assert_eq!(r.pick, Pick::One);
    assert_eq!(lens_named(&db, r.candidates[0]), "fixed lens");
    assert_eq!(r.crop, Some(4.5));
    // An unknown camera: every lens fits; the crop factor from the 35 mm equivalent.
    let r = find(
        &db,
        &Query {
            maker: "Nobody",
            model: "X",
            lens: Some("EF24-70mm f/2.8L II USM"),
            focal: Some(24.0),
            focal35: Some(38.4),
        },
    );
    assert_eq!(r.camera, None);
    assert_eq!(r.pick, Pick::One);
    assert!((r.crop.expect("crop") - 1.6).abs() < 1e-12);
}

#[test]
fn profile_of_a_lens_at_a_shot() {
    let db = sample();
    let l = &db.lenses[0];
    // 24 mm, f/2.8, far, full frame: the 24 mm calibrations as they are.
    let p = l.profile(24.0, 2.8, 1000.0, 1.0);
    assert_eq!(
        p.distortion,
        Some(Distortion::PtLens {
            a: 0.02,
            b: -0.08,
            c: 0.03
        })
    );
    assert_eq!(p.vignetting, Some([-0.5, 0.1, -0.05]));
    // At the corner (ρ = 1) the vignetting radius is 1: 1/(1 − 0.5 + 0.1 − 0.05).
    assert!((p.vignette_gain(1.0) - 1.0 / 0.55).abs() < 1e-12);
    // 47 mm: half way between the two distortion calibrations.
    match l.profile(47.0, 2.8, 1000.0, 1.0).distortion {
        Some(Distortion::PtLens { a, b, c }) => {
            assert!((a - 0.015).abs() < 1e-12 && (b + 0.05).abs() < 1e-12);
            assert!((c - 0.025).abs() < 1e-12);
        }
        o => panic!("{o:?}"),
    }
}

/// The shipped database: loads, and finds a well-known pair. No network.
#[test]
fn bundled_database() {
    let t = std::time::Instant::now();
    let db = Db::from_bundle(crate::BUNDLE).expect("bundle");
    let ms = t.elapsed().as_secs_f64() * 1e3;
    eprintln!(
        "bundled: {} mounts, {} cameras, {} lenses in {ms:.0} ms",
        db.mounts.len(),
        db.cameras.len(),
        db.lenses.len()
    );
    assert!(db.errors.is_empty(), "{:?}", db.errors);
    assert!(db.cameras.len() > 1000 && db.lenses.len() > 1400);
    let r = find(
        &db,
        &Query {
            maker: "Canon",
            model: "Canon EOS 5D Mark III",
            lens: Some("EF24-70mm f/2.8L II USM"),
            focal: Some(35.0),
            focal35: None,
        },
    );
    assert_eq!(r.pick, Pick::One);
    assert_eq!(
        db.lenses[r.candidates[0]].display_name(),
        "Canon EF 24-70mm f/2.8L II USM"
    );
    let t = std::time::Instant::now();
    for _ in 0..100 {
        std::hint::black_box(find(
            &db,
            &Query {
                maker: "NIKON CORPORATION",
                model: "NIKON D750",
                lens: Some("AF-S Nikkor 24-70mm f/2.8G ED"),
                focal: Some(50.0),
                focal35: None,
            },
        ));
    }
    eprintln!("lookup: {:.2} ms", t.elapsed().as_secs_f64() * 10.0);
}
