use super::*;
use specular_vector::{Item, Shape};

const SAMPLE: &str = include_str!("../../tests/data/ipc2581/board.xml");

fn area(items: &[Item], layer: u32) -> f64 {
    items
        .iter()
        .filter(|i| i.layer == layer)
        .map(|i| match &i.shape {
            Shape::Fill { path, .. } => path.area(),
            _ => 0.0,
        })
        .sum()
}

#[test]
fn reads_a_board() {
    assert!(is_ipc2581(SAMPLE.as_bytes()));
    let s = scene(SAMPLE.as_bytes(), &Limits::default()).unwrap();
    assert_eq!(
        (s.format.as_str(), s.version.as_str()),
        ("IPC-2581", "rev C")
    );
    let p = s.pcb.as_ref().unwrap();
    assert_eq!(p.kind, PcbKind::Ipc2581);
    assert_eq!(p.copper_layers, 2, "IN1 has no features");
    assert_eq!((p.components, p.nets, p.holes), (2, 3, 2));
    assert_eq!(p.generator.as_deref(), Some("TestCAD 1.2"));
    assert_eq!(p.thickness_mm, Some(1.6));
    let (w, h) = p.size_mm.unwrap();
    assert!(
        (w - 50.0).abs() < 1e-6 && (h - 20.0).abs() < 1e-6,
        "{w} {h}"
    );
    let roles: Vec<&str> = p.layers.iter().map(|l| l.role.as_str()).collect();
    assert_eq!(roles, vec!["Bottom copper", "Top copper", "Plated drill"]);
    assert!(
        s.warnings
            .iter()
            .any(|w| w.contains("1 text features have no size"))
    );
    // Bottom: one donut flash.
    let bottom = s
        .layers
        .iter()
        .position(|l| l.name.contains("BOTTOM"))
        .unwrap() as u32;
    let a = area(&s.sheets[0].items, bottom);
    let want = std::f64::consts::PI * (0.25 - 0.0625);
    assert!((a - want).abs() < 1e-3, "{a} {want}");
    // The top board view has the substrate with its cut-out (even-odd) and R1's reference.
    let top = &s.sheets[1].items;
    assert!(
        top.iter()
            .any(|i| matches!(&i.shape, Shape::Text(t) if t.plain() == "R1"))
    );
    assert!(
        !top.iter()
            .any(|i| matches!(&i.shape, Shape::Text(t) if t.plain() == "U2"))
    );
}

/// A board of one layer whose features are `features`, with `content` in its Content.
fn board(content: &str, features: &str) -> String {
    format!(
        r#"<IPC-2581 revision="C"><Content roleRef="Owner">{content}</Content><Ecad name="t">
<CadHeader units="MILLIMETER"/><CadData><Layer name="TOP" layerFunction="SILKSCREEN" side="TOP"/>
<Step name="pcb"><LayerFeature layerRef="TOP"><Set>{features}</Set></LayerFeature></Step>
</CadData></Ecad></IPC-2581>"#
    )
}

/// Line ends of a scene's first sheet, rounded to 1 µm (stroke capsules: by their bounds).
fn stroke_boxes(s: &VectorScene) -> Vec<[i64; 4]> {
    let r = |v: f64| (v * 1000.0).round() as i64;
    let mut v: Vec<[i64; 4]> = s.sheets[0]
        .items
        .iter()
        .flat_map(|i| match &i.shape {
            Shape::Fill { path, .. } => {
                let mut out = Vec::new();
                let mut sub = BezPath::new();
                for el in path.elements() {
                    if matches!(el, specular_vector::kurbo::PathEl::MoveTo(_))
                        && !sub.elements().is_empty()
                    {
                        out.push(std::mem::take(&mut sub));
                    }
                    sub.push(*el);
                }
                out.push(sub);
                out.into_iter()
                    .map(|p| {
                        let b = p.bounding_box();
                        [r(b.x0), r(b.y0), r(b.x1), r(b.y1)]
                    })
                    .collect::<Vec<_>>()
            }
            _ => Vec::new(),
        })
        .collect();
    v.sort();
    v
}

#[test]
fn text_with_an_embedded_font() {
    // An "L" glyph 1 wide and 2 high, strokes 0.2 wide; "LL" in a box 6 wide and 4 high at
    // (10, 20): cells 2 × 4 stretched to 3 × 4.
    let content = r#"<DictionaryFont units="MILLIMETER"><EntryFont id="F1"><FontDefEmbedded name="lf">
<Glyph charCode="4C" lowerLeftX="0" lowerLeftY="0" upperRightX="1" upperRightY="2">
<Polyline><PolyBegin x="0" y="2"/><PolyStepSegment x="0" y="0"/><PolyStepSegment x="1" y="0"/>
<LineDesc lineWidth="0.2" lineEnd="ROUND"/></Polyline></Glyph></FontDefEmbedded></EntryFont>
<EntryFont id="F2"><FontDefExternal name="Arial"/></EntryFont></DictionaryFont>"#;
    let feats = r#"<Features><Location x="10" y="20"/><Text textString="LL" fontSize="4">
<BoundingBox lowerLeftX="0" lowerLeftY="0" upperRightX="6" upperRightY="4"/><FontRef id="F1"/></Text></Features>"#;
    let s = scene(board(content, feats).as_bytes(), &Limits::default()).unwrap();
    let b = stroke_boxes(&s);
    // Pen 0.2 × 2 = 0.4: stems x 10 and 13, from 20 to 24; feet to 13 and 16.
    assert!(b.contains(&[9800, 19800, 10200, 24200]), "{b:?}");
    assert!(b.contains(&[12800, 19800, 13200, 24200]), "{b:?}");
    assert!(b.contains(&[9800, 19800, 13200, 20200]), "{b:?}");
    assert!(b.contains(&[12800, 19800, 16200, 20200]), "{b:?}");
    assert!(s.counts.iter().any(|(k, n)| k == "Text" && *n == 1));
    // An external font: the built-in one, turned 90° counter-clockwise by its Xform.
    let feats = r#"<Features><Location x="0" y="0"/><Text textString="I" fontSize="6">
<Xform rotation="90"/><FontRef id="F2"/></Text></Features>"#;
    let s = scene(board(content, feats).as_bytes(), &Limits::default()).unwrap();
    assert!(
        s.warnings.iter().any(|w| w.contains("built-in font: F2")),
        "{:?}",
        s.warnings
    );
    // The built-in "I" stem at x 3 from 0 to 6, pen 6/8; turned: y 3, x from −6 to 0.
    let b = stroke_boxes(&s);
    assert!(b.contains(&[-6375, 2625, 375, 3375]), "{b:?}");
}

#[test]
fn refusals() {
    let no_ecad = r#"<IPC-2581 revision="B"><Content/><Bom name="b"/></IPC-2581>"#;
    match scene(no_ecad.as_bytes(), &Limits::default()) {
        Err(Error::Unsupported(m)) => assert!(m.contains("bill of materials")),
        r => panic!("{:?}", r.map(|s| s.format)),
    }
    assert_eq!(
        scene(b"<svg/>", &Limits::default()).err(),
        Some(Error::NotThisFormat)
    );
    assert!(matches!(
        scene(b"<IPC-2581><Ecad", &Limits::default()),
        Err(Error::Malformed(_))
    ));
    let dtd = "<!DOCTYPE x [<!ENTITY a \"aaaa\">]><IPC-2581/>";
    assert!(scene(dtd.as_bytes(), &Limits::default()).is_err());
    for n in (0..SAMPLE.len()).step_by(97) {
        let _ = scene(&SAMPLE.as_bytes()[..n], &Limits::default());
    }
}
