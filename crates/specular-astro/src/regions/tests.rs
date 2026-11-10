use super::*;
use specular_fits::Header;

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/regions");

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{DATA}/{name}")).unwrap()
}

/// The oracle's WCS (`sample.hdr`, 200 × 150).
fn sample_wcs() -> Wcs {
    let text = read("sample.hdr");
    let pairs: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let h = Header::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
    Wcs::from_header(&h, &[200, 150]).unwrap()
}

/// A placed shape in the oracle's layout: kind, include, numbers.
fn row(r: &Region, p: &Placed) -> (String, bool, Vec<f64>) {
    let kind = match (&r.shape, p) {
        (Shape::Circle { .. }, _) => "circle",
        (Shape::Annulus { .. }, _) => "annulus",
        (_, Placed::Ellipse { .. }) => "ellipse",
        (_, Placed::Rect { .. }) => "box",
        (_, Placed::Polygon(_)) => "polygon",
        (_, Placed::Point { .. }) => "point",
        (_, Placed::Line { .. }) => "line",
        (_, Placed::Text { .. }) => "text",
    };
    let v = match p {
        Placed::Ellipse { c, r, angle } => vec![c.0, c.1, r.0, r.1, *angle],
        Placed::Rect { c, size, angle } => vec![c.0, c.1, size.0, size.1, *angle],
        Placed::Polygon(v) => v.iter().flat_map(|q| [q.0, q.1]).collect(),
        Placed::Line { a, b, .. } => vec![a.0, a.1, b.0, b.1],
        Placed::Point { c, .. } | Placed::Text { c } => vec![c.0, c.1],
    };
    (kind.to_owned(), r.props.include, v)
}

/// Ours, one row per region (an annulus as inner and outer radius).
fn ours(parsed: &Parsed, t: &Target) -> Vec<(String, bool, Vec<f64>)> {
    parsed
        .regions
        .iter()
        .map(|r| {
            let placed = r.place(t);
            assert!(!placed.is_empty(), "{r:?} not placed");
            let (k, inc, mut v) = row(r, &placed[0]);
            if k == "annulus" {
                let outer = row(r, &placed[1]).2;
                v = vec![v[0], v[1], v[2], outer[2], 0.0];
            }
            if k == "circle" {
                v.truncate(4);
                v.push(0.0);
            }
            (k, inc, v)
        })
        .collect()
}

fn oracle(name: &str) -> Vec<(String, bool, Vec<f64>)> {
    read(name)
        .lines()
        .map(|l| {
            let mut f = l.split('\t');
            let kind = f.next().unwrap().to_owned();
            let inc = f.next().unwrap().parse::<f64>().unwrap() != 0.0;
            (kind, inc, f.map(|v| v.parse().unwrap()).collect())
        })
        .collect()
}

/// Angles equal modulo 360 (and 180 for symmetric shapes).
fn same_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(180.0);
    d.min(180.0 - d) < 1e-3
}

/// Mismatches of `ours` against an oracle's rows: positions and sizes within 1e-3 px,
/// angles within 1e-3° (when `angles`).
fn mismatches(
    ours: &[(String, bool, Vec<f64>)],
    theirs: &[(String, bool, Vec<f64>)],
    angles: bool,
) -> Vec<String> {
    assert_eq!(ours.len(), theirs.len());
    let mut bad = Vec::new();
    for (k, (o, t)) in ours.iter().zip(theirs).enumerate() {
        if o.0 != t.0 || o.1 != t.1 {
            bad.push(format!("#{k}: {o:?} vs {t:?}"));
            continue;
        }
        let n = if matches!(o.0.as_str(), "ellipse" | "box") {
            4
        } else {
            o.2.len().min(t.2.len())
        };
        let n = if o.0 == "circle" || o.0 == "annulus" {
            4
        } else {
            n
        };
        let mut ok = (0..n.min(t.2.len())).all(|i| (o.2[i] - t.2[i]).abs() < 1e-3);
        if angles && matches!(o.0.as_str(), "ellipse" | "box") {
            ok &= same_angle(o.2[4], t.2[4]);
        }
        if !ok {
            bad.push(format!("#{k}: {o:?} vs {t:?}"));
        }
    }
    bad
}

#[test]
fn sample_matches_astropy_regions() {
    let wcs = sample_wcs();
    let rest: [usize; 0] = [];
    let t = Target {
        height: 150.0,
        wcs: Some((&wcs, &rest)),
        physical: None,
    };
    let parsed = parse(&read("sample.reg"));
    assert!(parsed.refused.is_empty(), "{:?}", parsed.refused);
    let bad = mismatches(&ours(&parsed, &t), &oracle("sample.oracle.tsv"), true);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn sample_positions_match_pyregion() {
    // pyregion turns sky angles the other way from astropy-regions (and from the
    // "width along longitude" rule), so only positions and sizes are compared.
    let wcs = sample_wcs();
    let rest: [usize; 0] = [];
    let t = Target {
        height: 150.0,
        wcs: Some((&wcs, &rest)),
        physical: None,
    };
    let parsed = parse(&read("sample.reg"));
    let theirs: Vec<_> = oracle("sample.pyregion.tsv");
    let ours: Vec<_> = ours(&parsed, &t)
        .into_iter()
        .zip(&theirs)
        // pyregion gives text regions no position.
        .filter(|(_, t)| t.0 != "text")
        .map(|(o, _)| o)
        .collect();
    let theirs: Vec<_> = theirs.into_iter().filter(|t| t.0 != "text").collect();
    let bad = mismatches(&ours, &theirs, false);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn properties_and_old_syntax() {
    let p = parse(concat!(
        "# Region file format: DS9 version 4.1\n",
        "global color=cyan width=2 font=\"times 14 bold roman\"\n",
        "image; circle 10 20 5 # color=#ff8000 dash=1 text={Hello, world} tag={a} background\n",
        "-box(1,2,3,4,0) # width = 3\n",
        "image;circle point 5 6\n",
        "x point(7,8)\n",
        "text 3 4 {Note}\n",
        "line(1,1,5,5) # line=0 1\n",
        "# vector(10,10,20,45) vector=1\n",
        "annulus(50,50,10,20,30)\n",
        "annulus 50 50 10 20 n=2\n",
        "panda(50,50,0,360,4,10,20,2)\n",
        "ruler(1,1,2,2) ruler=image image\n",
        "ecliptic\n",
        "circle(1,2,3)\n",
        "image\n",
        "circle(1,2)\n",
        "circle(1,2,3); box(4,5,6,7,0)\n",
    ));
    let r = &p.regions;
    assert_eq!(r[0].props.color, [255, 128, 0]);
    assert_eq!(r[0].props.width, 2.0);
    assert!(r[0].props.dash);
    assert_eq!(r[0].props.text, "Hello, world");
    assert_eq!(r[0].props.font_size, 14.0);
    assert_eq!(r[0].props.tags, ["a"]);
    assert!(r[0].props.background);
    assert_eq!(r[0].system, System::Image);
    assert!(!r[1].props.include);
    assert_eq!(r[1].props.width, 3.0);
    assert_eq!(r[1].props.color, [0, 255, 255]);
    assert!(matches!(
        r[2].shape,
        Shape::Point {
            marker: Marker::Circle,
            ..
        }
    ));
    assert!(matches!(
        r[3].shape,
        Shape::Point {
            marker: Marker::X,
            c: (7.0, 8.0),
            ..
        }
    ));
    assert_eq!(r[4].props.text, "Note");
    assert!(matches!(
        r[5].shape,
        Shape::Line {
            arrows: (false, true),
            ..
        }
    ));
    assert!(matches!(r[6].shape, Shape::Vector { arrow: true, .. }));
    match &r[7].shape {
        Shape::Annulus { radii, .. } => assert_eq!(radii.len(), 3),
        s => panic!("{s:?}"),
    }
    match &r[8].shape {
        Shape::Annulus { radii, .. } => {
            assert_eq!(radii, &[Len::Px(10.0), Len::Px(15.0), Len::Px(20.0)]);
        }
        s => panic!("{s:?}"),
    }
    assert_eq!(r[9].approximate, Some(Approx::PandaEdges));
    assert_eq!(r.len(), 12, "{r:?}");
    let refused: Vec<_> = p.refused.iter().map(|(l, r)| (*l, r.to_string())).collect();
    assert_eq!(
        refused,
        [
            (13, "ruler regions are not supported".to_owned()),
            (
                15,
                "the ecliptic coordinate system is not supported".to_owned()
            ),
            (17, "a circle region has unreadable values".to_owned()),
        ]
    );
}

/// DS9 `physical` regions on a cut-out, binned image (`LTM`/`LTV` in its header), against
/// pyregion (which reads the same transform from the `L` alternate WCS).
#[test]
fn physical_matches_pyregion() {
    let text = read("physical.hdr");
    let pairs: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let h = Header::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
    let physical = physical_of(&h);
    assert_eq!(physical, Some(([0.5, 0.5], [-10.25, -20.75])));
    let t = Target {
        height: 150.0,
        wcs: None,
        physical,
    };
    let parsed = parse(&read("physical.reg"));
    assert!(parsed.refused.is_empty(), "{:?}", parsed.refused);
    let bad = mismatches(&ours(&parsed, &t), &oracle("physical.pyregion.tsv"), true);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
    // Without LTM/LTV, physical is image.
    assert_eq!(physical_of(&Header::from_pairs([("NAXIS", "2", "")])), None);
}

#[test]
fn physical_through_ltm_ltv() {
    let p = parse("physical\ncircle(21,41,10)\n");
    let t = Target {
        height: 100.0,
        wcs: None,
        physical: Some(([2.0, 2.0], [1.0, 1.0])),
    };
    // image = (physical − LTV) / LTM = (10, 20); radius 10 / 2.
    assert_eq!(
        p.regions[0].place(&t),
        [Placed::Ellipse {
            c: (9.5, 80.5),
            r: (5.0, 5.0),
            angle: 0.0
        }]
    );
    // Sky regions need a WCS.
    let p = parse("fk5\ncircle(1,2,3\")\n");
    assert!(p.regions[0].place(&t).is_empty());
}

#[test]
fn sexagesimal_and_units() {
    let p = parse("fk5\ncircle(10:00:00,+02:12:00,1')\ngalactic\npoint(10:30:00,-01:00:00)\n");
    match &p.regions[0].shape {
        Shape::Circle { c, r } => {
            assert!((c.0 - 150.0).abs() < 1e-9 && (c.1 - 2.2).abs() < 1e-9);
            assert_eq!(*r, Len::Deg(1.0 / 60.0));
        }
        s => panic!("{s:?}"),
    }
    // Galactic sexagesimal values are degrees.
    match &p.regions[1].shape {
        Shape::Point { c, .. } => {
            assert!((c.0 - 10.5).abs() < 1e-9 && (c.1 + 1.0).abs() < 1e-9);
        }
        s => panic!("{s:?}"),
    }
}

/// Shapes drawn on the sample image, written as fk5.
fn drawn() -> Vec<Drawn> {
    vec![
        Drawn {
            shape: Placed::Ellipse {
                c: (100.0, 75.0),
                r: (10.0, 10.0),
                angle: 0.0,
            },
            text: "M51 {core}".into(),
            color: Some([255, 0, 0]),
        },
        Drawn {
            shape: Placed::Ellipse {
                c: (60.0, 40.0),
                r: (12.0, 6.0),
                angle: 0.0,
            },
            text: String::new(),
            color: None,
        },
        Drawn {
            shape: Placed::Rect {
                c: (140.0, 100.0),
                size: (20.0, 10.0),
                angle: 0.0,
            },
            text: String::new(),
            color: Some([10, 20, 30]),
        },
        Drawn {
            shape: Placed::Polygon(vec![(10.0, 10.0), (40.0, 12.0), (25.0, 30.0)]),
            text: String::new(),
            color: None,
        },
        Drawn {
            shape: Placed::Line {
                a: (5.0, 140.0),
                b: (50.0, 120.0),
                arrows: (false, false),
            },
            text: String::new(),
            color: None,
        },
        Drawn {
            shape: Placed::Point {
                c: (170.0, 20.0),
                marker: Marker::Cross,
                size: 11.0,
            },
            text: "star".into(),
            color: None,
        },
    ]
}

#[test]
fn written_file_reads_back() {
    let wcs = sample_wcs();
    let rest: [usize; 0] = [];
    let t = Target {
        height: 150.0,
        wcs: Some((&wcs, &rest)),
        physical: None,
    };
    let shapes = drawn();
    let (text, skipped) = write(&shapes, &t);
    assert_eq!(skipped, 0);
    // The committed file is what astropy-regions read back (`sample.written.tsv`).
    if std::env::var_os("SPECULAR_UPDATE_REGIONS").is_some() {
        std::fs::write(format!("{DATA}/sample.written.reg"), &text).unwrap();
    }
    assert_eq!(text, read("sample.written.reg"), "{text}");
    let p = parse(&text);
    assert!(p.refused.is_empty());
    assert_eq!(p.regions.len(), shapes.len());
    assert_eq!(p.regions[0].props.text, "M51 core");
    assert_eq!(p.regions[0].props.color, [255, 0, 0]);
    assert_eq!(p.regions[2].props.color, [10, 20, 30]);
    for (r, d) in p.regions.iter().zip(&shapes) {
        let back = r.place(&t);
        let (a, b) = (back[0].outline(16), d.shape.outline(16));
        for (p, q) in a.iter().zip(&b) {
            assert!(
                (p.0 - q.0).abs() < 1e-3 && (p.1 - q.1).abs() < 1e-3,
                "{r:?}: {a:?} vs {b:?}"
            );
        }
    }
    // astropy-regions reads our file to the same shapes.
    let theirs = oracle("sample.written.tsv");
    let mine = ours(&p, &t);
    let bad = mismatches(&mine, &theirs, true);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
    // Without a WCS: image coordinates, exactly.
    let t2 = Target {
        height: 150.0,
        wcs: None,
        physical: None,
    };
    let (text, _) = write(&shapes, &t2);
    assert!(text.contains("\nimage\ncircle(100.5,75.5,10) # color=red text={M51 core}\n"));
    let p = parse(&text);
    for (r, d) in p.regions.iter().zip(&shapes) {
        assert_eq!(r.place(&t2)[0], d.shape, "{text}");
    }
}

#[test]
fn hostile_input_never_panics() {
    for s in [
        "circle(",
        "circle()",
        "polygon(1,2,3)",
        "ellipse(1,2,3,4,5,6)",
        "# text(",
        "global color",
        "annulus 1 2 3 4 n=99999999",
        "box 1 2 3 4 5 6 7 8 9",
        "-",
        ";;;",
        "circle(1e400,2,3)",
        "text 1 2 {unterminated",
        "fk5;point(10:00:00:00,1)",
        "\u{feff}circle(1,2,3) # text={é}",
    ] {
        let p = parse(s);
        let t = Target {
            height: 10.0,
            wcs: None,
            physical: None,
        };
        for r in &p.regions {
            let _ = r.place(&t);
        }
    }
    assert!(sniff(b"# Region file format: DS9 version 4.1\n"));
    assert!(sniff(b"image\ncircle(1,2,3)\n"));
    assert!(!sniff(b"SIMPLE  =                    T"));
}

/// Timing (run by hand, `--ignored --nocapture`): 20 000 fk5 circles read and placed.
#[test]
#[ignore]
fn parse_and_place_timing() {
    let mut text = String::from("fk5\n");
    for k in 0..20_000 {
        let (a, b) = (
            149.99 + (k % 200) as f64 * 1e-4,
            2.19 + (k / 200) as f64 * 1e-4,
        );
        text.push_str(&format!(
            "circle({a:.6},{b:.6},2\") # color=red text={{s{k}}}\n"
        ));
    }
    let wcs = sample_wcs();
    let rest: [usize; 0] = [];
    let t = Target {
        height: 150.0,
        wcs: Some((&wcs, &rest)),
        physical: None,
    };
    let t0 = std::time::Instant::now();
    let p = parse(&text);
    let t1 = t0.elapsed();
    let n: usize = p.regions.iter().map(|r| r.place(&t).len()).sum();
    eprintln!(
        "regions: read {} in {:.1} ms, placed {n} in {:.1} ms",
        p.regions.len(),
        t1.as_secs_f64() * 1000.0,
        (t0.elapsed() - t1).as_secs_f64() * 1000.0
    );
}

/// A B1950 image (`RADESYS = 'FK4'`, or only `EQUINOX = 1950`) is FK4: `fk4` regions place
/// on it directly and `fk5` ones through the FK4 → FK5 conversion (astropy's position for
/// the image centre, 150°, 2.2° B1950, is 150.645 610 5°, 1.958 123 5° J2000).
#[test]
fn fk4_image_and_regions() {
    for radesys in [Some("'FK4'"), None] {
        let mut pairs: Vec<(String, String)> = read("sample.hdr")
            .lines()
            .filter_map(|l| l.split_once(" = "))
            .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            .filter(|(k, _)| k != "RADESYS")
            .collect();
        pairs.push(("EQUINOX".into(), "1950.0".into()));
        if let Some(r) = radesys {
            pairs.push(("RADESYS".into(), r.into()));
        }
        let h = Header::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
        let w = Wcs::from_header(&h, &[200, 150]).unwrap();
        assert_eq!(frames::of_wcs(&w), Some(Sky::Fk4), "{radesys:?}");
        let t = Target {
            height: 150.0,
            wcs: Some((&w, &[])),
            physical: None,
        };
        let (ra, dec) = frames::convert(Sky::Fk4, Sky::Fk5, 150.0, 2.2);
        assert!(
            separation(ra, dec, 150.645_610_5, 1.958_123_5) < 1e-6,
            "{ra} {dec}"
        );
        let p = parse(&format!(
            "fk4\ncircle(150,2.2,3\")\nb1950;point(150,2.2)\nfk5\ncircle({ra},{dec},3\")\n"
        ));
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let at = |k: usize| match p.regions[k].place(&t)[..] {
            [Placed::Ellipse { c, .. }] | [Placed::Point { c, .. }] => c,
            ref o => panic!("{o:?}"),
        };
        for k in 0..3 {
            let c = at(k);
            assert!(
                (c.0 - 100.0).abs() < 1e-4 && (c.1 - 75.0).abs() < 1e-4,
                "{k}: {c:?}"
            );
        }
    }
}
