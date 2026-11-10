//! Exact answers for every template, the same answers through non-square pixels and a
//! mirrored profile, and a dump for the numpy oracle (`tools/quality/clinical_oracle.py`).

use super::*;
use crate::calib::{Source, Unit};

fn cal() -> Calibration {
    Calibration::spacing(1.0, 1.0, Unit::Mm, Source::DicomPixelSpacing).unwrap()
}

fn get(o: &Outcome, key: &str) -> f64 {
    o.readouts
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no {key}"))
        .value
        .number()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn polar(c: P, r: f64, deg: f64) -> P {
    let t = deg.to_radians();
    P::new(c.x + r * t.cos(), c.y + r * t.sin())
}

/// Three rim points of a circle.
fn rim(c: P, r: f64) -> [P; 3] {
    [polar(c, r, 10.0), polar(c, r, 130.0), polar(c, r, 250.0)]
}

fn spine() -> Vec<P> {
    (0..7)
        .map(|i| P::new(200.0 + 10.0 * f64::from(i), 0.0))
        .collect()
}

fn vhs_points(l: f64, s: f64) -> Vec<P> {
    let mut v = vec![
        P::new(0.0, 50.0),
        P::new(l * 0.6, 50.0 + l * 0.8),
        P::new(10.0, 120.0),
        P::new(10.0 + s, 120.0),
    ];
    v.extend(spine());
    v
}

#[test]
fn every_template_asks_for_its_points() {
    for t in Template::ALL {
        assert_eq!(Template::from_key(t.key()), Some(t));
        assert!(compute(t, &[], &cal()).is_none());
        assert_eq!(t.next().prev(), t);
    }
    assert_eq!(Template::Vhs.landmarks().len(), 11);
    assert_eq!(Template::Steiner.landmarks().len(), 13);
    assert_eq!(Template::Downs.landmarks().len(), 16);
}

#[test]
fn vertebral_scale() {
    let m = spine();
    assert_eq!(vertebrae(25.0, &m), Some((2.5, false)));
    assert_eq!(vertebrae(60.0, &m), Some((6.0, false)));
    assert_eq!(vertebrae(75.0, &m), Some((7.5, true)));
    // Uneven vertebrae: each counts as one.
    let u = [P::new(0.0, 0.0), P::new(10.0, 0.0), P::new(30.0, 0.0)];
    assert_eq!(vertebrae(20.0, &u), Some((1.5, false)));
    assert!(vertebrae(1.0, &u[..1]).is_none());
}

#[test]
fn vhs_and_vlas() {
    let o = compute(Template::Vhs, &vhs_points(52.0, 43.0), &cal()).unwrap();
    close(get(&o, "long-axis"), 5.2);
    close(get(&o, "short-axis"), 4.3);
    close(get(&o, "vhs"), 9.5);
    close(get(&o, "long-axis-length"), 52.0);
    assert_eq!(o.readouts[0].key, "vhs");
    assert!(!o.readouts[0].extrapolated);
    let o = compute(Template::Vhs, &vhs_points(75.0, 43.0), &cal()).unwrap();
    close(get(&o, "vhs"), 11.8);
    assert!(o.readouts[0].extrapolated && o.readouts[1].extrapolated);
    assert!(!o.readouts[2].extrapolated);

    let mut v = vec![P::new(0.0, 0.0), P::new(0.0, 21.0)];
    v.extend(spine());
    let o = compute(Template::Vlas, &v, &cal()).unwrap();
    close(get(&o, "vlas"), 2.1);
    close(get(&o, "vlas-length"), 21.0);
}

fn norberg_points(angle: f64) -> Vec<P> {
    let (l, r) = (P::new(0.0, 0.0), P::new(100.0, 0.0));
    let mut v = rim(l, 12.0).to_vec();
    v.extend(rim(r, 12.0));
    // Angles measured from the line between the centres, the rims cranial (−y).
    v.push(polar(l, 15.0, -angle));
    v.push(polar(r, 15.0, -(180.0 - (angle - 2.0))));
    v
}

#[test]
fn norberg() {
    let o = compute(Template::Norberg, &norberg_points(105.0), &cal()).unwrap();
    close(get(&o, "norberg-left"), 105.0);
    close(get(&o, "norberg-right"), 103.0);
    assert_eq!(o.guides.len(), 3);
    // Rim points on a line: no circle.
    let mut bad = norberg_points(105.0);
    bad[1] = P::new((bad[0].x + bad[2].x) / 2.0, (bad[0].y + bad[2].y) / 2.0);
    assert!(compute(Template::Norberg, &bad, &cal()).is_none());
}

#[test]
fn tpa() {
    let talus = P::new(0.0, 100.0);
    let em = P::new(0.0, 0.0);
    let caudal = P::new(-10.0, 2.0);
    let cranial = caudal + polar(P::new(0.0, 0.0), 20.0, -25.0);
    let o = compute(Template::Tpa, &[em, talus, cranial, caudal], &cal()).unwrap();
    close(get(&o, "tpa"), 25.0);
}

#[test]
fn distraction_head_offset() {
    let mut v = rim(P::new(0.0, 0.0), 10.0).to_vec();
    v.extend(rim(P::new(3.0, 4.0), 12.0));
    let o = compute(Template::Distraction, &v, &cal()).unwrap();
    close(get(&o, "distraction-index"), 0.5);
    close(get(&o, "centre-distance"), 5.0);
    close(get(&o, "head-radius"), 10.0);

    let o = compute(Template::FemoralHead, &rim(P::new(7.0, 9.0), 23.5), &cal()).unwrap();
    close(get(&o, "head-diameter"), 47.0);

    let mut v = rim(P::new(0.0, 0.0), 20.0).to_vec();
    v.extend([P::new(40.0, 30.0), P::new(40.0, 200.0)]);
    let o = compute(Template::Offset, &v, &cal()).unwrap();
    close(get(&o, "offset"), 40.0);
    close(get(&o, "head-diameter"), 40.0);
}

#[test]
fn leg_length_and_acetabular_angle() {
    let v = [
        P::new(0.0, 0.0),
        P::new(100.0, 0.0),
        P::new(10.0, 30.0),
        P::new(90.0, 25.0),
    ];
    let o = compute(Template::LegLength, &v, &cal()).unwrap();
    close(get(&o, "leg-left"), 30.0);
    close(get(&o, "leg-right"), 25.0);
    close(get(&o, "leg-difference"), 5.0);
    assert_eq!(o.readouts[0].key, "leg-difference");

    let t = P::new(0.0, 0.0);
    let v = [t, P::new(-100.0, 0.0), polar(t, 50.0, -40.0)];
    let o = compute(Template::Acetabular, &v, &cal()).unwrap();
    close(get(&o, "acetabular-angle"), 40.0);
}

/// A lateral cephalogram facing +x (y down) with known angles: SN horizontal, A and B on
/// rays from N, the incisors and planes at set angles.
fn ceph() -> std::collections::HashMap<&'static str, P> {
    let n = P::new(0.0, 0.0);
    let s = P::new(-70.0, 0.0);
    // A ray making `deg` with N→S, going down.
    let ray = |deg: f64, r: f64| polar(n, r, 180.0 - deg);
    let a = ray(82.0, 55.0);
    let b = ray(79.0, 95.0);
    let pog = ray(80.0, 115.0);
    let go = P::new(-60.0, 80.0);
    let gn = go + polar(P::new(0.0, 0.0), 80.0, 32.0);
    let me = go + polar(P::new(0.0, 0.0), 75.0, 30.0);
    let u1t = P::new(8.0, 70.0);
    let u1a = u1t + polar(P::new(0.0, 0.0), 22.0, -110.0);
    let l1t = P::new(5.0, 68.0);
    let l1a = l1t + polar(P::new(0.0, 0.0), 20.0, 105.0);
    let ocf = P::new(6.0, 69.0);
    let ocb = ocf + polar(P::new(0.0, 0.0), 40.0, 180.0 + 14.0);
    let po = P::new(-90.0, 25.0);
    let or = P::new(-10.0, 25.0);
    [
        ("s", s),
        ("n", n),
        ("a", a),
        ("b", b),
        ("pog", pog),
        ("go", go),
        ("gn", gn),
        ("me", me),
        ("u1-tip", u1t),
        ("u1-apex", u1a),
        ("l1-tip", l1t),
        ("l1-apex", l1a),
        ("occlusal-front", ocf),
        ("occlusal-back", ocb),
        ("po", po),
        ("or", or),
    ]
    .into_iter()
    .collect()
}

fn ceph_points(t: Template, mut m: impl FnMut(P) -> P) -> Vec<P> {
    let c = ceph();
    t.landmarks().iter().map(|l| m(c[l.key])).collect()
}

#[test]
fn steiner() {
    let o = compute(
        Template::Steiner,
        &ceph_points(Template::Steiner, |p| p),
        &cal(),
    )
    .unwrap();
    close(get(&o, "sna"), 82.0);
    close(get(&o, "snb"), 79.0);
    close(get(&o, "anb"), 3.0);
    close(get(&o, "sn-gogn"), 32.0);
    close(get(&o, "occlusal-sn"), 14.0);
    // U1 at −110° from +x: its axis (apex→tip) points at 70°, NA at 98°: 28° between them.
    close(get(&o, "u1-na-angle"), 28.0);
    // L1 tip→apex at 105°, so apex→tip at −75°; NB (B→N) at −79°: 4°... as lines.
    close(get(&o, "l1-nb-angle"), 4.0);
    // Interincisal: tip→apex −110° and 105°: 145°.
    close(get(&o, "interincisal"), 145.0);
    // Pog on a ray between A's and B's: behind NB by 115·sin 1°.
    close(get(&o, "pog-nb-mm"), 115.0 * 1f64.to_radians().sin());
}

#[test]
fn downs() {
    let o = compute(
        Template::Downs,
        &ceph_points(Template::Downs, |p| p),
        &cal(),
    )
    .unwrap();
    // FH horizontal: N→Pog at 100° from +x, Po−Or along −x: 80°.
    close(get(&o, "facial-angle"), 80.0);
    close(get(&o, "mandibular-plane"), 30.0);
    close(get(&o, "occlusal-cant"), 14.0);
    close(get(&o, "interincisal"), 145.0);
    // L1 apex→tip at −75°, occlusal back→… front→back at 194°: 91° − 90.
    close(
        get(&o, "l1-occlusal"),
        (194.0 - 360.0 + 75.0f64).abs() - 90.0,
    );
    // Go−Me at 210°: from −75° that is 75 + 210 − 360… the angle between is 75°: −15.
    close(get(&o, "l1-mandibular"), 285.0 - 210.0 - 90.0);
    let c = ceph();
    let y = (c["gn"] - c["s"])
        .y
        .atan2((c["gn"] - c["s"]).x)
        .to_degrees();
    close(get(&o, "y-axis"), y);
    // A ahead of N–Pog (its ray is at 82° from N–S against Pog's 80°): convex, positive.
    assert!(get(&o, "convexity") > 0.0);
    // B (79°) behind A: negative.
    assert!(get(&o, "ab-plane") < 0.0);
}

#[test]
fn mirrored_and_non_square() {
    // Facing −x: the same numbers.
    let mirror = |p: P| P::new(300.0 - p.x, p.y);
    for t in [Template::Steiner, Template::Downs] {
        let a = compute(t, &ceph_points(t, |p| p), &cal()).unwrap();
        let b = compute(t, &ceph_points(t, mirror), &cal()).unwrap();
        for (x, y) in a.readouts.iter().zip(&b.readouts) {
            assert_eq!(x.key, y.key);
            close(x.value.number(), y.value.number());
        }
    }
    // Pixels 0.5 × 1 mm: the points in pixels give the millimetre answers.
    let wide = Calibration::spacing(0.5, 1.0, Unit::Mm, Source::DicomPixelSpacing).unwrap();
    let px = |p: P| P::new(p.x / 0.5, p.y);
    let cases: Vec<(Template, Vec<P>)> = vec![
        (Template::Vhs, vhs_points(52.0, 43.0)),
        (Template::Norberg, norberg_points(101.0)),
        (Template::Steiner, ceph_points(Template::Steiner, |p| p)),
        (Template::Downs, ceph_points(Template::Downs, |p| p)),
    ];
    for (t, v) in cases {
        let a = compute(t, &v, &cal()).unwrap();
        let q: Vec<P> = v.iter().map(|p| px(*p)).collect();
        let b = compute(t, &q, &wide).unwrap();
        for (x, y) in a.readouts.iter().zip(&b.readouts) {
            assert!(
                (x.value.number() - y.value.number()).abs() < 1e-9,
                "{t:?} {}",
                x.key
            );
        }
        // Guides come back in pixels.
        close(b.guides[0][0].x, a.guides[0][0].x / 0.5);
    }
}

#[test]
fn values_carry_calibration() {
    let un = Calibration::uncalibrated();
    let o = compute(Template::FemoralHead, &rim(P::new(0.0, 0.0), 5.0), &un).unwrap();
    assert_eq!(o.readouts[0].value.unit_key(), "px");
    let o = compute(
        Template::Tpa,
        &[
            P::new(0.0, 0.0),
            P::new(0.0, 9.0),
            P::new(5.0, 0.0),
            P::new(-5.0, 1.0),
        ],
        &cal(),
    )
    .unwrap();
    assert_eq!(o.readouts[0].value.unit_key(), "deg");
    let mag = Calibration::spacing(0.1, 0.1, Unit::Mm, Source::DicomMagnification).unwrap();
    let o = compute(Template::FemoralHead, &rim(P::new(0.0, 0.0), 5.0), &mag).unwrap();
    assert!(is_approximate(&o.readouts[0]));
}

/// Random cases for `tools/quality/clinical_oracle.py`:
/// `SPECULAR_ORACLE_DIR=<dir> cargo test -p specular-measure clinical_dump -- --ignored`.
#[test]
#[ignore]
fn clinical_dump() {
    let dir = std::path::PathBuf::from(std::env::var("SPECULAR_ORACLE_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut out = String::from("[\n");
    let mut first = true;
    for case in 0..400 {
        let t = Template::ALL[case % Template::ALL.len()];
        let (sx, sy) = (0.05 + rnd() * 0.3, 0.05 + rnd() * 0.3);
        let cal = Calibration::spacing(sx, sy, Unit::Mm, Source::DicomPixelSpacing).unwrap();
        let pts: Vec<P> = if matches!(t, Template::Steiner | Template::Downs) {
            // Jittered anatomy, sometimes mirrored, so signs and sides vary realistically.
            let flip = rnd() < 0.5;
            ceph_points(t, |p| {
                let p = P::new(p.x + (rnd() - 0.5) * 10.0, p.y + (rnd() - 0.5) * 10.0);
                let p = if flip { P::new(-p.x, p.y) } else { p };
                P::new((p.x + 400.0) / sx * 0.2, (p.y + 300.0) / sy * 0.2)
            })
        } else {
            (0..t.landmarks().len())
                .map(|_| P::new(rnd() * 2000.0, rnd() * 2000.0))
                .collect()
        };
        let Some(o) = compute(t, &pts, &cal) else {
            continue;
        };
        if !first {
            out.push_str(",\n");
        }
        first = false;
        let p: Vec<String> = pts.iter().map(|p| format!("[{},{}]", p.x, p.y)).collect();
        let r: Vec<String> = o
            .readouts
            .iter()
            .map(|r| format!("\"{}\":{}", r.key, r.value.number()))
            .collect();
        out.push_str(&format!(
            "{{\"template\":\"{}\",\"spacing\":[{sx},{sy}],\"points\":[{}],\"readouts\":{{{}}}}}",
            t.key(),
            p.join(","),
            r.join(",")
        ));
    }
    out.push_str("\n]\n");
    std::fs::write(dir.join("clinical.json"), out).unwrap();
}

#[test]
fn csv_and_geojson() {
    use crate::list::Measurement;
    use crate::provenance::Provenance;
    let mut v = rim(P::new(0.0, 0.0), 10.0).to_vec();
    v.extend(rim(P::new(3.0, 4.0), 12.0));
    let mut m = Measurement::clinical(Template::Distraction, v, cal());
    m.id = 3;
    assert!(m.length().is_none() && m.primary().is_none());
    let text = crate::csv::write(std::slice::from_ref(&m), &Provenance::default());
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with("3,")).collect();
    // Three readouts, six landmarks.
    assert_eq!(rows.len(), 9);
    let cols: Vec<&str> = rows[0].split(',').collect();
    assert_eq!(cols.len(), crate::csv::COLUMNS.len());
    assert_eq!(cols[2], "distraction-index");
    assert_eq!(cols[22], "distraction-index");
    close(cols[23].parse().unwrap(), 0.5);
    assert_eq!(cols[24], "ratio");
    assert_eq!(cols[18], "dicom-pixel-spacing");
    let last: Vec<&str> = rows[8].split(',').collect();
    assert_eq!(last[22], "acetabulum-rim-3");
    assert_eq!(last[24], "px");

    let json = crate::geojson::write(std::slice::from_ref(&m), &Provenance::default());
    assert!(json.contains("\"distraction-index ratio\":0.5"));
    let back = crate::geojson::read(&json).unwrap();
    assert_eq!(back.items[0].template, Some(Template::Distraction));
    close(
        back.items[0].outcome().unwrap().readouts[0].value.number(),
        0.5,
    );
}
