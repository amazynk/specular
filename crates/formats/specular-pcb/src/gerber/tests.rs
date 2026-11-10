use std::f64::consts::PI;

use super::*;

fn g(body: &str) -> Gerber {
    let src = format!("%FSLAX26Y26*%\n%MOMM*%\n{body}\nM02*\n");
    parse(src.as_bytes(), &Limits::default()).expect("parses")
}

fn area(o: &Object) -> f64 {
    o.path.area()
}

#[test]
fn sniffs_gerber_and_not_text() {
    assert!(is_gerber(b"%FSLAX26Y26*%\n%MOMM*%\n%ADD10C,0.1*%\n"));
    assert!(is_gerber(
        b"G04 KiCad*\n%TF.FileFunction,Copper,L1,Top*%\n%FSLAX46Y46*%\n"
    ));
    assert!(!is_gerber(
        b"Hello world, this is a README.\n* bullet\n* bullet\n"
    ));
    assert!(!is_gerber(b"M48\nMETRIC\nT1C0.8\n%\nX1Y1\nM30\n"));
    assert!(!is_gerber(b"%PDF-1.7\n"));
}

#[test]
fn flashes_standard_apertures_with_holes_and_transforms() {
    let gb = g(
        "%ADD10C,1*%\n%ADD11R,2X1*%\n%ADD12O,1X3*%\n%ADD13P,2X4X0*%\n%ADD14C,2X1*%\n\
         D10*\nX0Y0D03*\nD11*\nX5000000Y0D03*\nD12*\nX10000000Y0D03*\nD13*\nX15000000Y0D03*\n\
         D14*\nX20000000Y0D03*\n%LR90*%\nD11*\nX25000000Y0D03*\n%LS2*%\nD10*\nX30000000Y0D03*\n",
    );
    let a: Vec<f64> = gb.objects.iter().map(area).collect();
    let want = [
        PI * 0.25,
        2.0,
        2.0 + PI * 0.25,
        2.0,
        PI * (1.0 - 0.25),
        2.0,
        PI,
    ];
    for (i, (a, w)) in a.iter().zip(want).enumerate() {
        assert!((a - w).abs() < 1e-3, "{i}: {a} vs {w}");
    }
    assert!(gb.objects[4].holes);
    // Rotated 90°: 1 wide, 2 tall.
    let b = gb.objects[5].path.bounding_box();
    assert!((b.width() - 1.0).abs() < 1e-9 && (b.height() - 2.0).abs() < 1e-9);
    assert_eq!(gb.apertures, 5);
    assert!(gb.warnings.is_empty(), "{:?}", gb.warnings);
}

#[test]
fn draws_lines_and_arcs() {
    let gb = g(
        "%ADD10C,0.5*%\nD10*\nX0Y0D02*\nG01*\nX10000000Y0D01*\nG75*\nG03*\n\
         X0Y10000000I-10000000J0D01*\nG02X10000000Y0I0J-10000000D01*\n",
    );
    assert_eq!(gb.objects.len(), 3);
    assert!((area(&gb.objects[0]) - (5.0 + PI * 0.0625)).abs() < 1e-3);
    let quarter = PI / 4.0 * (10.25f64.powi(2) - 9.75f64.powi(2)) + PI * 0.0625;
    assert!(
        (area(&gb.objects[1]) - quarter).abs() < 1e-3,
        "{}",
        area(&gb.objects[1])
    );
    assert!((area(&gb.objects[2]) - quarter).abs() < 1e-3);
    let Some(Segment::Arc { center, sweep, .. }) = gb.objects[1].segment else {
        panic!("an arc");
    };
    assert!(center.distance(Point::new(0.0, 0.0)) < 1e-9);
    assert!((sweep - PI / 2.0).abs() < 1e-9);
}

#[test]
fn single_quadrant_arcs_pick_their_centre() {
    let gb = g("%ADD10C,0.1*%\nD10*\nG74*\nX10000000Y0D02*\nG03X0Y10000000I10000000J0D01*\n");
    let Some(Segment::Arc { center, sweep, .. }) = gb.objects[0].segment else {
        panic!("an arc");
    };
    assert!(center.distance(Point::ORIGIN) < 1e-9, "{center:?}");
    assert!((sweep - PI / 2.0).abs() < 1e-9);
}

#[test]
fn full_circle_in_multi_quadrant() {
    let gb = g("%ADD10C,1*%\nD10*\nG75*\nX5000000Y0D02*\nG02X5000000Y0I-5000000J0D01*\n");
    let a = area(&gb.objects[0]);
    assert!(
        (a - PI * (5.5f64.powi(2) - 4.5f64.powi(2))).abs() < 1e-3,
        "{a}"
    );
}

#[test]
fn regions_with_holes_by_cut_in_and_polarity() {
    // 10×10 square with a 2×2 cut-in hole.
    let gb = g(
        "G36*\nX0Y0D02*\nG01*\nX10000000Y0D01*\nX10000000Y10000000D01*\nX0Y10000000D01*\n\
         X0Y0D01*\nX4000000Y4000000D01*\nX4000000Y6000000D01*\nX6000000Y6000000D01*\n\
         X6000000Y4000000D01*\nX4000000Y4000000D01*\nX0Y0D01*\nG37*\n%LPC*%\n%ADD10C,1*%\n\
         D10*\nX8000000Y8000000D03*\n",
    );
    assert_eq!(gb.objects.len(), 2);
    assert_eq!(gb.objects[0].kind, ObjectKind::Region);
    assert!(
        (area(&gb.objects[0]) - 96.0).abs() < 1e-9,
        "{}",
        area(&gb.objects[0])
    );
    assert!(gb.objects[0].dark && !gb.objects[1].dark);
}

#[test]
fn macros_with_expressions() {
    let gb = g(
        "%AMDONUT*0 ring*$3=$1x0.5*1,1,$1,0,0*1,0,$3,0,0*%\n%AMBOX*21,1,$1,$2,0,0,$3*%\n\
         %ADD20DONUT,2*%\n%ADD21BOX,4X1X90*%\nD20*\nX0Y0D03*\nD21*\nX0Y0D03*\n",
    );
    assert_eq!(gb.macros, 2);
    assert!((area(&gb.objects[0]) - PI * 0.75).abs() < 1e-3);
    assert!(gb.objects[0].holes);
    let b = gb.objects[1].path.bounding_box();
    assert!((b.width() - 1.0).abs() < 1e-9 && (b.height() - 4.0).abs() < 1e-9);
}

#[test]
fn step_repeat_and_blocks() {
    let gb = g("%ADD10C,1*%\n%SRX3Y2I5J4*%\nD10*\nX0Y0D03*\n%SR*%\n\
         %ABD30*%\nD10*\nX1000000Y0D03*\n%LPC*%\nX1000000Y0D03*\n%LPD*%\n%AB*%\n\
         D30*\nX0Y100000000D03*\n%LPC*%\nX0Y200000000D03*\n");
    // 6 copies, then a block of two flashed twice (polarities toggled the second time).
    assert_eq!(gb.objects.len(), 10);
    let b = gb.objects[5].path.bounding_box().center();
    assert!(b.distance(Point::new(10.0, 4.0)) < 1e-9, "{b:?}");
    assert!(gb.objects[6].dark && !gb.objects[7].dark);
    assert!(!gb.objects[8].dark && gb.objects[9].dark);
    let c = gb.objects[6].path.bounding_box().center();
    assert!(c.distance(Point::new(1.0, 100.0)) < 1e-9);
}

#[test]
fn mirroring_and_attributes() {
    let src = "%TF.GenerationSoftware,KiCad,Pcbnew,8.0.1*%\n%TF.ProjectId,demo,1234,rev2*%\n\
               %TF.FileFunction,Copper,L1,Top*%\n%FSLAX46Y46*%\n%MOIN*%\n%TA.AperFunction,SMDPad*%\n\
               %ADD10R,0.1X0.05*%\n%LMX*%\nD10*\nX1000000Y0D03*\nM02*\n";
    let gb = parse(src.as_bytes(), &Limits::default()).unwrap();
    assert_eq!(gb.generator().as_deref(), Some("KiCad Pcbnew 8.0.1"));
    assert_eq!(gb.project().as_deref(), Some("demo"));
    assert_eq!(gb.revision().as_deref(), Some("rev2"));
    assert_eq!(gb.file_function().as_deref(), Some("Copper,L1,Top"));
    assert_eq!(gb.version(), "RS-274X X2");
    assert!(gb.inch);
    let b = gb.bounds.unwrap();
    assert!((b.center().x - 25.4).abs() < 1e-9);
    assert!((b.width() - 2.54).abs() < 1e-9);
}

#[test]
fn old_files_and_refusals() {
    // Trailing zeros omitted, G70 inches, deprecated codes, no M02.
    let src = "G04 old*\n%FSTAX23Y23*%\nG70*\n%ADD10C,0.010*%\nG54D10*\nX1Y1D03*\nX02Y0D03*\n";
    let gb = parse(src.as_bytes(), &Limits::default()).unwrap();
    assert_eq!(gb.objects.len(), 2);
    let c = gb.objects[0].path.bounding_box().center();
    // "1" is 1.0000 (10.000 in) with the trailing zeros put back; "02" is 2.000 in.
    assert!((c.x - 254.0).abs() < 1e-9, "{c:?}");
    let c = gb.objects[1].path.bounding_box().center();
    assert!((c.x - 50.8).abs() < 1e-9, "{c:?}");
    assert!(!gb.deprecated.is_empty());
    assert!(gb.warnings.iter().any(|w| w.contains("M02")));
    // RS-274-D: apertures used, none defined.
    let d = "G04 RS-274-D*\nG90*\nD10*\nX100Y100D03*\nX200Y100D01*\nM02*\n";
    match parse(d.as_bytes(), &Limits::default()) {
        Err(Error::Unsupported(m)) => assert!(m.contains("RS-274-D"), "{m}"),
        r => panic!("{:?}", r.map(|g| g.objects.len())),
    }
    assert!(parse(b"\0\0\0", &Limits::default()).is_err());
}

#[test]
fn negative_images_and_limits() {
    let gb = g("%IPNEG*%\n%ADD10C,1*%\nD10*\nX0Y0D03*\n");
    assert_eq!(gb.objects.len(), 2);
    assert!(gb.objects[0].dark && !gb.objects[1].dark);
    let lim = Limits {
        items: 3,
        copies: 10,
    };
    let src = "%FSLAX26Y26*%\n%MOMM*%\n%ADD10C,1*%\n%SRX10Y10I1J1*%\nD10*\nX0Y0D03*\n%SR*%\nM02*\n";
    let gb = parse(src.as_bytes(), &lim).unwrap();
    assert_eq!(gb.objects.len(), 3);
    assert!(!gb.warnings.is_empty());
}

#[test]
fn rectangular_holes() {
    let gb = g("%ADD10C,2X1X0.5*%\nD10*\nX0Y0D03*\n");
    assert!((area(&gb.objects[0]) - (PI - 0.5)).abs() < 1e-3);
    assert!(gb.deprecated.iter().any(|d| d.contains("rectangular")));
}
