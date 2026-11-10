//! Recipe tests: the text form and starters, naming, the resize arithmetic, metadata read
//! back from written files, colour spaces against their profiles, sharpening and watermark
//! placement. Files go to a private folder under `TMPDIR`.

use super::finish::{Encoded, Linear};
use super::size::{Size, Target, needed_scale, target};
use super::space::Primaries;
use super::watermark::{Mark, place};
use super::*;
use crate::exif::Entry;
use crate::pixels::Pixels;
use crate::{Outcome, Pattern};
use specular_core::CancelToken;
use specular_decode::Registry;
use std::fs;
use std::path::Path;
use std::sync::Arc;

fn tmpdir(tag: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let d = base.join(format!("specular-recipe-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn text_form_round_trips_and_starters_load() {
    let mut r = Recipe {
        name: "Client [proof]\nv2".into(),
        format: Format::Webp,
        quality: 77,
        lossless: false,
        space: Space::DisplayP3,
        embed_icc: false,
        size: Size::Exact {
            width: 1080,
            height: 1350,
        },
        enlarge: true,
        sharpen: Sharpen {
            medium: Medium::Matte,
            strength: Strength::High,
        },
        metadata: Metadata {
            keep: Keep::CopyrightContact,
            strip_gps: true,
            strip_people: true,
        },
        watermark: Watermark {
            kind: MarkKind::Text("© Ana Silva".into()),
            position: Position::Top,
            opacity: 45,
            scale: 30,
        },
        pattern: "{date:%Y%m%d}-{seq:3}-{text}".into(),
        text: "client".into(),
        destination: Destination::Fixed("/srv/out".into()),
        collision: Collision::Overwrite,
        after: After::OpenFolder,
        ..Recipe::default()
    };
    let back = Recipe::parse_all(&Recipe::write_all(std::slice::from_ref(&r)));
    r.name = "Client  proof  v2".into();
    assert_eq!(back, vec![r.clone()]);
    for size in [
        Size::Full,
        Size::LongEdge(2048),
        Size::ShortEdge(1080),
        Size::Megapixels(24),
        Size::Percent(50),
    ] {
        assert_eq!(Size::parse(&size.to_text()), Some(size));
    }
    // Unknown keys and bad values keep the defaults; an image watermark and Ask survive.
    let t = "[A]\nformat = bmp\nquality = 0\nnew_key = 1\nwatermark = image /x/logo.png\nfolder = ask\n";
    let a = &Recipe::parse_all(t)[0];
    assert_eq!(a.format, Format::Jpeg);
    assert_eq!(a.quality, 90);
    assert_eq!(a.watermark.kind, MarkKind::Image("/x/logo.png".into()));
    assert_eq!(a.destination, Destination::Ask);
    // The starters: web, print, email, each a different file.
    let s = Recipe::starters();
    let ids: Vec<_> = s.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["web", "print", "email"]);
    assert_eq!(s[1].format, Format::Tiff);
    assert_eq!(s[1].bits, Bits::Sixteen);
    assert_eq!(Recipe::parse_all(&Recipe::write_all(&s)), s);
    // 16 bits only where the format holds it.
    let j = Recipe {
        bits: Bits::Sixteen,
        ..Recipe::default()
    }
    .normalised();
    assert_eq!(j.bits, Bits::Eight);
}

#[test]
fn resize_math() {
    let t = |w, h, s, e| {
        let Target { width, height, .. } = target(w, h, s, e);
        (width, height)
    };
    // Long and short edges, either orientation; never larger unless asked.
    assert_eq!(t(6000, 4000, Size::LongEdge(2048), false), (2048, 1365));
    assert_eq!(t(4000, 6000, Size::LongEdge(2048), false), (1365, 2048));
    assert_eq!(t(6000, 4000, Size::ShortEdge(1080), false), (1620, 1080));
    assert_eq!(t(4000, 6000, Size::ShortEdge(1080), false), (1080, 1620));
    assert_eq!(t(1000, 800, Size::LongEdge(2048), false), (1000, 800));
    assert_eq!(t(1000, 800, Size::LongEdge(2048), true), (2048, 1638));
    assert_eq!(t(1000, 800, Size::ShortEdge(1600), true), (2000, 1600));
    // Megapixels: within a rounding of the asked count, aspect kept.
    let (w, h) = t(6000, 4000, Size::Megapixels(24), false);
    assert!(((w * h) as f64 - 2.4e6).abs() < 3000.0, "{w}×{h}");
    assert_eq!((w as f64 / h as f64 * 100.0).round(), 150.0);
    assert_eq!(t(1000, 1000, Size::Megapixels(24), false), (1000, 1000));
    // Percent.
    assert_eq!(t(6000, 4000, Size::Percent(25), false), (1500, 1000));
    assert_eq!(t(600, 400, Size::Percent(200), false), (600, 400));
    assert_eq!(t(600, 400, Size::Percent(200), true), (1200, 800));
    // Exact: the box exactly, the source cropped to its aspect around the centre.
    let e = target(
        6000,
        4000,
        Size::Exact {
            width: 1080,
            height: 1080,
        },
        false,
    );
    assert_eq!((e.width, e.height), (1080, 1080));
    let c = e.crop;
    assert!((c[0] - 1000.0 / 6000.0).abs() < 1e-9 && c[1] == 0.0);
    assert!((c[2] - 4000.0 / 6000.0).abs() < 1e-9 && c[3] == 1.0);
    // … smaller than the box and not enlarged: the crop at its own size.
    let e = target(
        800,
        600,
        Size::Exact {
            width: 1080,
            height: 1350,
        },
        false,
    );
    assert_eq!((e.width, e.height), (480, 600));
    // Full.
    assert_eq!(t(123, 45, Size::Full, false), (123, 45));
    // The decode a set of recipes needs: the largest of them.
    let s = needed_scale(
        6000,
        4000,
        &[(Size::LongEdge(1280), false), (Size::LongEdge(2048), false)],
    );
    assert!((s - 2048.0 / 6000.0).abs() < 1e-3, "{s}");
    assert_eq!(
        needed_scale(
            6000,
            4000,
            &[(Size::LongEdge(2048), false), (Size::Full, false)]
        ),
        1.0
    );
}

/// Exif with a camera, a copyright, an artist, a date and a GPS position.
fn camera_exif() -> crate::exif::Exif {
    crate::exif::Exif {
        ifd0: vec![
            Entry::ascii(0x010F, "Canon"),
            Entry::ascii(0x0110, "Canon EOS R5"),
            Entry::ascii(0x013B, "Ana Silva"),
            Entry::ascii(0x8298, "(c) 2026 Ana Silva"),
            Entry::short(0x4746, &[4]),
        ],
        exif: vec![Entry::ascii(0x9003, "2026:07:14 18:03:09")],
        gps: vec![
            Entry::ascii(0x0001, "N"),
            Entry::rational(0x0002, 48, 1),
            Entry::ascii(0x0003, "E"),
            Entry::rational(0x0004, 2, 1),
        ],
    }
}

const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:exif="http://ns.adobe.com/exif/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/" xmlns:Iptc4xmpExt="http://iptc.org/std/Iptc4xmpExt/2008-02-29/" xmlns:Iptc4xmpCore="http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/" exif:GPSLatitude="48,51.5N" photoshop:City="Paris" xmp:Rating="4"><dc:rights><rdf:Alt><rdf:li xml:lang="x-default">(c) 2026 Ana Silva</rdf:li></rdf:Alt></dc:rights><dc:creator><rdf:Seq><rdf:li>Ana Silva</rdf:li></rdf:Seq></dc:creator><Iptc4xmpCore:CreatorContactInfo rdf:parseType="Resource"><Iptc4xmpCore:CiEmailWork>ana@example.org</Iptc4xmpCore:CiEmailWork></Iptc4xmpCore:CreatorContactInfo><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Tower</rdf:li></rdf:Alt></dc:title><Iptc4xmpExt:PersonInImage><rdf:Bag><rdf:li>Rui Costa</rdf:li></rdf:Bag></Iptc4xmpExt:PersonInImage><exif:GPSAltitude>35/1</exif:GPSAltitude></rdf:Description></rdf:RDF></x:xmpmeta>"#;

/// A `w × h` sRGB JPEG gradient with [`camera_exif`] and [`XMP`].
fn photo(path: &Path, w: u32, h: u32) {
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            v.extend([(x * 255 / w) as u8, (y * 255 / h) as u8, 120]);
        }
    }
    let px = Pixels {
        width: w,
        height: h,
        channels: 3,
        samples: crate::pixels::Samples::Eight(v),
    };
    let exif = camera_exif();
    let bytes = crate::encode::encode(
        &crate::Options {
            jpeg_quality: 95,
            ..crate::Options::default()
        },
        &px,
        &crate::encode::Meta {
            exif: Some(&exif),
            xmp: Some(XMP.as_bytes()),
            ..Default::default()
        },
    )
    .unwrap();
    fs::write(path, bytes).unwrap();
}

fn files() -> Files {
    Files {
        registry: Arc::new(Registry::new()),
    }
}

fn run_all(items: &[PathBuf], recipes: Vec<Recipe>) -> (Plan, RecipeReport) {
    let r = files();
    let c = CancelToken::new();
    let p = plan(items.iter().map(Item::new).collect(), recipes, &r, &c).unwrap();
    let rep = execute(&p, &r, Some(&r.registry), 2, &c, &|_| {});
    (p, rep)
}

fn out_paths(rep: &RecipeReport) -> Vec<PathBuf> {
    rep.results
        .iter()
        .map(|(_, _, o)| match o {
            Outcome::Converted { out, .. } => out.clone(),
            o => panic!("not written: {o:?}"),
        })
        .collect()
}

#[test]
fn names_follow_the_template_and_collision_rules() {
    let d = tmpdir("names");
    let a = d.join("IMG_0001.jpg");
    let b = d.join("IMG_0002.jpg");
    photo(&a, 64, 48);
    photo(&b, 48, 64);
    let r = Recipe {
        pattern: "{date:%Y%m%d}_{seq:03}_{camera}_{rating}_{text}".into(),
        text: "Lisbon".into(),
        destination: Destination::Subfolder("out".into()),
        ..Recipe::default()
    };
    let p = plan(
        vec![Item::new(&a), Item::new(&b)],
        vec![r.clone()],
        &files(),
        &CancelToken::new(),
    )
    .unwrap();
    let names: Vec<_> = p
        .outputs
        .iter()
        .map(|o| o.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "20260714_001_Canon EOS R5_4_Lisbon.jpg",
            "20260714_002_Canon EOS R5_4_Lisbon.jpg"
        ]
    );
    assert!(
        p.outputs
            .iter()
            .all(|o| o.path.parent() == Some(&*d.join("out")))
    );
    // The same name twice in one run: the second gets a suffix.
    let same = Recipe {
        pattern: "{camera}".into(),
        ..r.clone()
    };
    let (_, rep) = run_all(&[a.clone(), b.clone()], vec![same.clone()]);
    let outs = out_paths(&rep);
    assert_eq!(outs[0].file_name().unwrap(), "Canon EOS R5.jpg");
    assert_eq!(outs[1].file_name().unwrap(), "Canon EOS R5-2.jpg");
    // Again: unique names go on counting; skip leaves both; overwrite replaces both (and
    // the plan lists them first).
    let (_, rep) = run_all(std::slice::from_ref(&a), vec![same.clone()]);
    assert_eq!(
        out_paths(&rep)[0].file_name().unwrap(),
        "Canon EOS R5-3.jpg"
    );
    let skip = Recipe {
        collision: Collision::Skip,
        ..same.clone()
    };
    let (_, rep) = run_all(std::slice::from_ref(&a), vec![skip]);
    assert!(matches!(rep.results[0].2, Outcome::Skipped { .. }));
    let over = Recipe {
        collision: Collision::Overwrite,
        quality: 50,
        ..same.clone()
    };
    let before = fs::metadata(d.join("out/Canon EOS R5.jpg")).unwrap().len();
    let (p, rep) = run_all(&[a.clone(), b.clone()], vec![over]);
    assert_eq!(p.replacing().len(), 2);
    let outs = out_paths(&rep);
    assert_eq!(outs[0].file_name().unwrap(), "Canon EOS R5.jpg");
    assert_eq!(outs[1].file_name().unwrap(), "Canon EOS R5-2.jpg");
    assert_ne!(fs::metadata(&outs[0]).unwrap().len(), before);
    // An original is never the output, whatever the rule.
    let into_source = Recipe {
        destination: Destination::Same,
        format: Format::Jpeg,
        collision: Collision::Overwrite,
        ..Recipe::default()
    };
    let src_len = fs::metadata(&a).unwrap().len();
    let (_, rep) = run_all(std::slice::from_ref(&a), vec![into_source]);
    assert!(matches!(
        rep.results[0].2,
        Outcome::Failed(crate::Failure::SameAsSource(_))
    ));
    assert_eq!(fs::metadata(&a).unwrap().len(), src_len);
    // A recipe that asks for its folder can't be planned without one.
    let ask = Recipe {
        destination: Destination::Ask,
        ..Recipe::default()
    };
    assert!(matches!(
        plan(
            vec![Item::new(&a)],
            vec![ask],
            &files(),
            &CancelToken::new()
        ),
        Err(PlanError::NoFolder { recipe: 0 })
    ));
    // The pattern preview for the dialog.
    let f = Facts {
        size: (6000, 4000),
        ..Facts::default()
    };
    let web = &Recipe::starters()[0];
    assert_eq!(
        name_for(&Pattern::parse("{name}_{w}").unwrap(), web, &a, 1, &f),
        "IMG_0001_2048.jpg"
    );
    let _ = fs::remove_dir_all(&d);
}

/// The Exif and XMP of a written file.
fn read_meta(p: &Path) -> (Option<crate::exif::Exif>, String) {
    let bytes = fs::read(p).unwrap();
    let sm = crate::source_meta(&bytes);
    let xmp = sm
        .xmp
        .map(|x| String::from_utf8(x).unwrap())
        .unwrap_or_default();
    (sm.exif, xmp)
}

#[test]
fn metadata_choices_read_back() {
    let d = tmpdir("meta");
    let a = d.join("IMG_0001.jpg");
    photo(&a, 64, 48);
    let mk = |keep, strip_gps, strip_people, fmt: Format, name: &str| Recipe {
        format: fmt,
        metadata: Metadata {
            keep,
            strip_gps,
            strip_people,
        },
        pattern: name.into(),
        ..Recipe::default()
    };
    let mut recipes = Vec::new();
    for fmt in [
        Format::Jpeg,
        Format::Png,
        Format::Tiff,
        Format::Webp,
        Format::Avif,
        Format::Jxl,
    ] {
        let f = fmt.id();
        recipes.push(mk(Keep::All, false, false, fmt, &format!("all-{f}")));
        recipes.push(mk(Keep::All, true, true, fmt, &format!("nogps-{f}")));
        recipes.push(mk(Keep::Copyright, false, false, fmt, &format!("copy-{f}")));
        recipes.push(mk(
            Keep::CopyrightContact,
            false,
            false,
            fmt,
            &format!("contact-{f}"),
        ));
        recipes.push(mk(Keep::None, false, false, fmt, &format!("none-{f}")));
    }
    let (_, rep) = run_all(std::slice::from_ref(&a), recipes);
    let outs = out_paths(&rep);
    let mut checked = 0;
    for p in &outs {
        let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
        let (kind, fmt) = stem.split_once('-').unwrap();
        let (exif, xmp) = read_meta(p);
        let gps = exif.as_ref().is_some_and(|e| !e.gps.is_empty());
        let has = |tag: u16| {
            exif.as_ref()
                .is_some_and(|e| e.ifd0.iter().any(|x| x.tag == tag))
        };
        match kind {
            "all" => {
                assert!(gps, "{stem}: GPS kept");
                assert!(
                    xmp.contains("GPSLatitude") && xmp.contains("Paris"),
                    "{stem}"
                );
                assert!(xmp.contains("Rui Costa") && xmp.contains("Tower"), "{stem}");
                assert!(has(0x0110) && has(0x8298), "{stem}");
            }
            "nogps" => {
                assert!(!gps, "{stem}: GPS read back");
                assert!(exif.as_ref().is_some_and(|e| !e.exif.is_empty()), "{stem}");
                assert!(
                    !xmp.contains("GPS") && !xmp.contains("Paris"),
                    "{stem}: {xmp}"
                );
                assert!(
                    !xmp.contains("Rui Costa") && !xmp.contains("PersonInImage"),
                    "{stem}"
                );
                assert!(xmp.contains("Tower"), "{stem}");
                // No GPS bytes anywhere in the file.
                let bytes = fs::read(p).unwrap();
                assert!(!bytes.windows(9).any(|w| w == b"GPSLatitu"), "{stem}");
            }
            "copy" => {
                assert!(
                    !gps && has(0x8298) && !has(0x013B) && !has(0x0110),
                    "{stem}"
                );
                assert!(xmp.contains("(c) 2026 Ana Silva"), "{stem}");
                for gone in [
                    "Tower",
                    "Ana Silva</rdf:li></rdf:Seq>",
                    "ana@example.org",
                    "GPS",
                    "Rui",
                ] {
                    assert!(!xmp.contains(gone), "{stem}: {gone} in {xmp}");
                }
            }
            "contact" => {
                assert!(!gps && has(0x8298) && has(0x013B) && !has(0x0110), "{stem}");
                assert!(
                    xmp.contains("ana@example.org") && xmp.contains("dc:creator"),
                    "{stem}"
                );
                for gone in ["Tower", "GPS", "Rui", "Paris"] {
                    assert!(!xmp.contains(gone), "{stem}: {gone} in {xmp}");
                }
            }
            "none" => {
                // A TIFF's own directory is TIFF-structured: no Exif tags in it, though.
                assert!(
                    !gps && !has(0x8298) && !has(0x013B) && !has(0x0110),
                    "{stem}"
                );
                assert!(exif.as_ref().is_none_or(|e| e.exif.is_empty()), "{stem}");
                assert!(xmp.is_empty(), "{stem}: {xmp}");
            }
            o => panic!("{o}"),
        }
        let _ = fmt;
        checked += 1;
    }
    assert_eq!(checked, 30);
    let _ = fs::remove_dir_all(&d);
}

/// Each space's curve inverts, its matrix keeps white white, and our profile says what the
/// pixels are: the profile's own colorants give back the linear BT.709 we started from.
#[test]
fn spaces_round_trip_through_their_profiles() {
    let colours = [
        [0.2f32, 0.5, 0.8],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.18, 0.18, 0.18],
        [1.0, 1.0, 1.0],
    ];
    for s in Space::ALL {
        for i in 0..=1000 {
            let v = i as f32 / 1000.0;
            assert!((s.decode(s.encode(v)) - v).abs() < 2e-5, "{s:?} {v}");
        }
        let m = s.from_linear(Primaries::Bt709);
        let w = specular_color::math::mul3(&m, [1.0, 1.0, 1.0]);
        assert!(
            w.iter().all(|c| (c - 1.0).abs() < 2e-3),
            "{s:?} white {w:?}"
        );
        let p = specular_color::Profile::parse(s.icc()).expect("profile parses");
        assert_eq!(p.color_space, specular_color::ColorSpace::Rgb);
        // Through the profile back to linear BT.709, as a colour-managed reader does.
        let lin = specular_color::profiles::linear_bt709();
        let tr = specular_color::Transform::new(&p, &lin, specular_color::Intent::Relative, false)
            .expect("transform");
        for c in colours {
            let t = specular_color::math::mul3(&m, c);
            if t.iter().any(|v| *v < -1e-4 || *v > 1.0 + 1e-4) {
                continue; // out of this space's gamut (sRGB's own primaries are in all).
            }
            let enc = t.map(|v| s.encode(v));
            let mut back = [0f32; 3];
            tr.eval(&enc, &mut back);
            for k in 0..3 {
                assert!((back[k] - c[k]).abs() < 0.01, "{s:?} {c:?} → {back:?}");
            }
        }
    }
}

fn flat(w: u32, h: u32, v: f32) -> Encoded {
    Encoded {
        width: w,
        height: h,
        rgb: vec![v; (w * h * 3) as usize],
        alpha: None,
    }
}

#[test]
fn sharpening_lifts_edges_and_leaves_flat_areas() {
    let (w, h) = (40u32, 8u32);
    let mut img = flat(w, h, 0.3);
    for y in 0..h {
        for x in 20..w {
            let i = ((y * w + x) * 3) as usize;
            img.rgb[i..i + 3].fill(0.7);
        }
    }
    let before = img.clone();
    let (sigma, amount) = finish::sharpen_params(
        Sharpen {
            medium: Medium::Screen,
            strength: Strength::High,
        },
        2048,
    )
    .unwrap();
    finish::sharpen(&mut img, sigma, amount);
    let at = |e: &Encoded, x: u32| e.rgb[((4 * w + x) * 3) as usize];
    // Overshoot on both sides of the edge; far from it nothing moves.
    assert!(at(&img, 19) < at(&before, 19) - 0.02);
    assert!(at(&img, 20) > at(&before, 20) + 0.02);
    assert_eq!(at(&img, 3), at(&before, 3));
    assert_eq!(at(&img, 36), at(&before, 36));
    // Prints sharpen wider than screens, matte more than glossy, high more than low.
    let p = |m, s| {
        finish::sharpen_params(
            Sharpen {
                medium: m,
                strength: s,
            },
            6000,
        )
        .unwrap()
    };
    assert!(p(Medium::Matte, Strength::Standard).0 > p(Medium::Glossy, Strength::Standard).0);
    assert!(p(Medium::Glossy, Strength::Standard).0 > p(Medium::Screen, Strength::Standard).0);
    assert!(p(Medium::Screen, Strength::High).1 > p(Medium::Screen, Strength::Low).1);
    assert!(finish::sharpen_params(Sharpen::OFF, 6000).is_none());
}

#[test]
fn watermark_goes_where_asked() {
    let (w, h) = (1000u32, 600u32);
    let mark = Mark {
        w: 100,
        h: 50,
        rgba: [255u8, 255, 255, 255].repeat(100 * 50),
    };
    let margin = 18; // 3 % of 600
    for pos in Position::ALL {
        let wm = Watermark {
            kind: MarkKind::Text("x".into()),
            position: pos,
            opacity: 50,
            scale: 20,
        };
        let (x, y, mw, mh) = place(w, h, (mark.w, mark.h), &wm);
        assert_eq!((mw, mh), (200, 100), "{pos:?}");
        let (col, row) = pos.cell();
        let ex = [margin, (w - mw) / 2, w - mw - margin][col as usize];
        let ey = [margin, (h - mh) / 2, h - mh - margin][row as usize];
        assert_eq!((x, y), (ex, ey), "{pos:?}");
        // Drawn there and nowhere else, at the opacity asked.
        let r = Recipe {
            watermark: wm,
            ..Recipe::default()
        };
        let mut img = flat(w, h, 0.0);
        let got = finish::draw_mark(&mut img, &mark, &r);
        assert_eq!(got, (x, y, mw, mh));
        for py in 0..h {
            for px in 0..w {
                let v = img.rgb[((py * w + px) * 3) as usize];
                let inside = px >= x && px < x + mw && py >= y && py < y + mh;
                if inside {
                    assert!((v - 0.5).abs() < 0.01, "{pos:?} {px},{py} {v}");
                } else {
                    assert_eq!(v, 0.0, "{pos:?} {px},{py}");
                }
            }
        }
    }
    // A mark taller than the room shrinks to fit within the margins.
    let tall = Watermark {
        scale: 100,
        ..Watermark::default()
    };
    let (_, _, mw, mh) = place(1000, 300, (100, 100), &tall);
    assert_eq!((mw, mh), (282, 282));
    // Text draws when a font is there (the SVG renderer's system fonts).
    if let Ok(m) = watermark::text_mark("© Ana Silva", 400) {
        assert!(m.w > 100 && m.h > 10);
        assert!(m.rgba.chunks_exact(4).any(|p| p[3] > 200));
    }
}

/// End to end: a recipe with a watermark, sharpening and a wide space on a real file, with
/// an image watermark from a PNG; the file decodes at the asked size.
#[test]
fn runs_several_recipes_on_a_selection() {
    let d = tmpdir("run");
    let srcs: Vec<PathBuf> = (0..3).map(|i| d.join(format!("IMG_{i}.jpg"))).collect();
    for s in &srcs {
        photo(s, 300, 200);
    }
    let logo = d.join("logo.png");
    let px: Vec<u8> = [255u8, 0, 0, 200].repeat(40 * 20);
    fs::write(
        &logo,
        specular_png::encode_with(
            40,
            20,
            specular_png::ColorType::Rgba,
            &px,
            &Default::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let web = Recipe {
        size: Size::LongEdge(150),
        watermark: Watermark {
            kind: MarkKind::Image(logo.clone()),
            position: Position::BottomRight,
            opacity: 100,
            scale: 20,
        },
        sharpen: Sharpen {
            medium: Medium::Screen,
            strength: Strength::Standard,
        },
        destination: Destination::Subfolder("web".into()),
        ..Recipe::default()
    };
    let tiff = Recipe {
        format: Format::Tiff,
        bits: Bits::Sixteen,
        space: Space::ProPhoto,
        destination: Destination::Subfolder("tiff".into()),
        ..Recipe::default()
    };
    let progress = std::sync::Mutex::new(Vec::new());
    let r = files();
    let c = CancelToken::new();
    let p = plan(
        srcs.iter().map(Item::new).collect(),
        vec![web, tiff],
        &r,
        &c,
    )
    .unwrap();
    let rep = execute(&p, &r, Some(&r.registry), 2, &c, &|pr| {
        progress.lock().unwrap().push(pr.done)
    });
    assert_eq!(rep.written(), 6, "{:?}", rep.results);
    assert_eq!(rep.folders(), [d.join("web"), d.join("tiff")]);
    assert_eq!(progress.lock().unwrap().last(), Some(&6));
    for (_, k, o) in &rep.results {
        let Outcome::Converted { out, .. } = o else {
            unreachable!()
        };
        let s = Registry::new().open_path(out).unwrap();
        let size = s.info().display_size();
        if *k == 0 {
            assert_eq!(size, (150, 100));
            // The logo's red in the bottom right corner (scaled to 30 px wide).
            let raster = s
                .decode(
                    &specular_decode::DecodeRequest::full(1),
                    &CancelToken::new(),
                )
                .unwrap();
            let px = crate::pixels::render(
                &raster,
                &crate::pixels::ToLinear::Srgb,
                &crate::pixels::Target {
                    width: raster.width,
                    height: raster.height,
                    gray: false,
                    alpha: false,
                    sixteen: false,
                    transfer: crate::pixels::OutTransfer::Srgb,
                    tone_map: true,
                },
                s.info().orientation,
                &CancelToken::new(),
            )
            .ok()
            .unwrap();
            let crate::pixels::Samples::Eight(v) = px.samples else {
                unreachable!()
            };
            let at = |x: usize, y: usize| &v[(y * 150 + x) * 3..][..3];
            let red = at(150 - 3 - 10, 100 - 3 - 3);
            assert!(red[0] > 200 && red[1] < 80, "{red:?}");
            let plain = at(10, 10);
            assert!(plain[0] < 120, "{plain:?}");
        } else {
            assert_eq!(size, (300, 200));
            assert!(s.embedded_icc().is_some() || s.info().icc_profile.is_some());
        }
    }
    // Cancelled before it starts: nothing written, all cancelled.
    let c = CancelToken::new();
    c.cancel();
    let rep = execute(&p, &r, None, 2, &c, &|_| {});
    assert!(rep.cancelled && rep.written() == 0);
    let _ = fs::remove_dir_all(&d);
}

/// The linear input of [`finish::finish`]: alpha flattened onto white for JPEG, kept for
/// PNG; the profile embedded only when asked.
#[test]
fn transparency_and_profile_choices() {
    let img = Linear {
        width: 2,
        height: 1,
        rgba: vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0],
        primaries: Primaries::Bt709,
    };
    let jpeg = Recipe::default();
    let e = finish::encode_space(&img, &jpeg);
    assert!(e.alpha.is_none());
    assert!((e.rgb[0] - 1.0).abs() < 1e-5); // transparent → white
    let png = Recipe {
        format: Format::Png,
        ..Recipe::default()
    };
    let e = finish::encode_space(&img, &png);
    assert_eq!(e.alpha.as_deref(), Some(&[0.0, 1.0][..]));
    let no_icc = Recipe {
        format: Format::Png,
        embed_icc: false,
        ..Recipe::default()
    };
    let with = finish::finish(&img, (2, 1), &png, None, None, None).unwrap();
    let without = finish::finish(&img, (2, 1), &no_icc, None, None, None).unwrap();
    assert!(with.bytes.windows(4).any(|w| w == b"iCCP"));
    assert!(!without.bytes.windows(4).any(|w| w == b"iCCP"));
}
