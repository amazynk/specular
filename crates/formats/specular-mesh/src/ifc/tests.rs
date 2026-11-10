//! IFC reader tests: synthetic files, and the corpus against IfcOpenShell's figures
//! (`gen/ifc_oracle.py`).

use std::collections::HashMap;
use std::path::PathBuf;

use super::*;

fn ifc(schema: &str, data: &str) -> Vec<u8> {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [DesignTransferView]'),'2;1');\n\
         FILE_NAME('t.ifc','2024-05-01T10:00:00',('Ann'),('Org'),'pre','Specular test','');\n\
         FILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n{data}\nENDSEC;\nEND-ISO-10303-21;\n"
    )
    .into_bytes()
}

/// A project in millimetres with a site, a building and one storey; elements go after.
const SPATIAL: &str = "\
#1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'Test project',$,$,$,$,(#20),#10);
#10=IFCUNITASSIGNMENT((#11,#12));
#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
#12=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);
#20=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#21,$);
#21=IFCAXIS2PLACEMENT3D(#22,$,$);
#22=IFCCARTESIANPOINT((0.,0.,0.));
#30=IFCSITE('1YvctVUKr0kugbFTf53O9L',$,'Site',$,$,#31,$,$,.ELEMENT.,$,$,$,$,$);
#31=IFCLOCALPLACEMENT($,#21);
#32=IFCBUILDING('2YvctVUKr0kugbFTf53O9L',$,'House',$,$,#33,$,$,.ELEMENT.,$,$,$);
#33=IFCLOCALPLACEMENT(#31,#21);
#34=IFCBUILDINGSTOREY('3YvctVUKr0kugbFTf53O9L',$,'Ground floor',$,$,#35,$,$,.ELEMENT.,0.);
#35=IFCLOCALPLACEMENT(#33,#21);
#36=IFCRELAGGREGATES('4YvctVUKr0kugbFTf53O9L',$,$,$,#1,(#30));
#37=IFCRELAGGREGATES('5YvctVUKr0kugbFTf53O9L',$,$,$,#30,(#32));
#38=IFCRELAGGREGATES('6YvctVUKr0kugbFTf53O9L',$,$,$,#32,(#34));
";

/// A 4000 × 200 × 3000 mm wall with a 1000 × 2000 door opening through it, coloured red.
const WALL: &str = "\
#100=IFCWALL('7YvctVUKr0kugbFTf53O9L',$,'Wall 1',$,$,#101,#110,$,$);
#101=IFCLOCALPLACEMENT(#35,#102);
#102=IFCAXIS2PLACEMENT3D(#103,$,$);
#103=IFCCARTESIANPOINT((1000.,0.,0.));
#110=IFCPRODUCTDEFINITIONSHAPE($,$,(#111));
#111=IFCSHAPEREPRESENTATION(#20,'Body','SweptSolid',(#112));
#112=IFCEXTRUDEDAREASOLID(#113,#21,#116,3000.);
#113=IFCRECTANGLEPROFILEDEF(.AREA.,$,#114,4000.,200.);
#114=IFCAXIS2PLACEMENT2D(#115,$);
#115=IFCCARTESIANPOINT((2000.,100.));
#116=IFCDIRECTION((0.,0.,1.));
#120=IFCSTYLEDITEM(#112,(#121),$);
#121=IFCSURFACESTYLE('Red paint',.BOTH.,(#122));
#122=IFCSURFACESTYLERENDERING(#123,0.,$,$,$,$,$,$,.NOTDEFINED.);
#123=IFCCOLOURRGB($,1.,0.,0.);
#130=IFCOPENINGELEMENT('8YvctVUKr0kugbFTf53O9L',$,'Opening',$,$,#131,#140,$,$);
#131=IFCLOCALPLACEMENT(#101,#132);
#132=IFCAXIS2PLACEMENT3D(#133,$,$);
#133=IFCCARTESIANPOINT((1000.,-100.,0.));
#140=IFCPRODUCTDEFINITIONSHAPE($,$,(#141));
#141=IFCSHAPEREPRESENTATION(#20,'Body','SweptSolid',(#142));
#142=IFCEXTRUDEDAREASOLID(#143,#21,#116,2000.);
#143=IFCRECTANGLEPROFILEDEF(.AREA.,$,#144,1000.,400.);
#144=IFCAXIS2PLACEMENT2D(#145,$);
#145=IFCCARTESIANPOINT((500.,200.));
#150=IFCRELVOIDSELEMENT('9YvctVUKr0kugbFTf53O9L',$,$,$,#100,#130);
#160=IFCRELCONTAINEDINSPATIALSTRUCTURE('AYvctVUKr0kugbFTf53O9L',$,$,$,(#100),#34);
";

#[test]
fn wall_with_an_opening() {
    let data = format!("{SPATIAL}{WALL}");
    let m = read(&ifc("IFC4", &data), &Limits::default(), None).unwrap();
    assert_eq!(m.schema, "IFC4");
    assert_eq!(m.mesh.unit, Unit::Millimetre);
    assert_eq!(m.elements.len(), 1, "{:?}", m.elements);
    let w = &m.elements[0];
    assert_eq!(w.kind, "IFCWALL");
    assert_eq!(w.name, "Wall 1");
    // 4 × 0.2 × 3 m³ minus the door's 1 × 0.2 × 2 m³ (the opening is grown by 0.1 mm).
    let v = w.volume.expect("closed") * 1e-9;
    assert!((v - 2.0).abs() < 2e-3, "{v}");
    assert_eq!(w.min, [1000.0, 0.0, 0.0]);
    assert_eq!(w.max, [5000.0, 200.0, 3000.0]);
    // Red, by its styled item, named after its style.
    assert_eq!(m.mesh.materials.len(), 1);
    assert_eq!(m.mesh.materials[0].name, "Red paint");
    assert_eq!(m.mesh.materials[0].base_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(m.mesh.title.as_deref(), Some("Test project"));
    let fact = |k: &str| {
        m.facts
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(fact("Openings cut"), Some("1"));
    assert_eq!(fact("Length unit"), Some("millimetre"));
    let tree: Vec<(usize, &str)> = m.tree.iter().map(|(d, t)| (*d, t.as_str())).collect();
    assert_eq!(
        tree,
        [
            (0, "Test project (project)"),
            (1, "Site (site)"),
            (2, "House (building)"),
            (3, "Ground floor (storey): 1 wall"),
        ]
    );
}

#[test]
fn ifc2x3_mapped_items_and_materials() {
    // A column type's representation mapped twice, coloured through its material.
    let data = format!(
        "{SPATIAL}\
#200=IFCREPRESENTATIONMAP(#21,#201);
#201=IFCSHAPEREPRESENTATION(#20,'Body','SweptSolid',(#202));
#202=IFCEXTRUDEDAREASOLID(#203,#21,#116,2500.);
#203=IFCCIRCLEPROFILEDEF(.AREA.,$,$,150.);
#116=IFCDIRECTION((0.,0.,1.));
#210=IFCCOLUMN('BYvctVUKr0kugbFTf53O9L',$,'C1',$,$,#211,#213,$);
#211=IFCLOCALPLACEMENT(#35,#212);
#212=IFCAXIS2PLACEMENT3D(#22,$,$);
#213=IFCPRODUCTDEFINITIONSHAPE($,$,(#214));
#214=IFCSHAPEREPRESENTATION(#20,'Body','MappedRepresentation',(#215,#217));
#215=IFCMAPPEDITEM(#200,#216);
#216=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#22,1.,$);
#217=IFCMAPPEDITEM(#200,#218);
#218=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#219,1.,$);
#219=IFCCARTESIANPOINT((1000.,0.,0.));
#220=IFCMATERIAL('Concrete');
#221=IFCRELASSOCIATESMATERIAL('CYvctVUKr0kugbFTf53O9L',$,$,$,(#210),#220);
#222=IFCMATERIALDEFINITIONREPRESENTATION($,$,(#223),#220);
#223=IFCSTYLEDREPRESENTATION(#20,$,$,(#224));
#224=IFCSTYLEDITEM($,(#225),$);
#225=IFCPRESENTATIONSTYLEASSIGNMENT((#226));
#226=IFCSURFACESTYLE($,.BOTH.,(#227));
#227=IFCSURFACESTYLESHADING(#228);
#228=IFCCOLOURRGB($,0.5,0.5,0.5);
"
    );
    let m = read(&ifc("IFC2X3", &data), &Limits::default(), None).unwrap();
    assert_eq!(m.elements.len(), 1);
    let c = &m.elements[0];
    let r = 150.0f64;
    let want = 2.0 * std::f64::consts::PI * r * r * 2500.0;
    let v = c.volume.unwrap();
    // 32 segments: the inscribed polygon's area is 0.9936 of the circle's.
    assert!((v / want - 0.9936).abs() < 1e-3, "{}", v / want);
    assert_eq!(c.min[0], -150.0);
    assert!((c.max[0] - 1150.0).abs() < 1e-9);
    assert_eq!(m.mesh.materials.len(), 1);
    assert_eq!(m.mesh.materials[0].name, "Concrete");
}

#[test]
fn face_sets_clipping_and_kind_colours() {
    let data = format!(
        "{SPATIAL}\
#300=IFCSLAB('DYvctVUKr0kugbFTf53O9L',$,'Slab',$,$,#211,#301,$,$);
#211=IFCLOCALPLACEMENT(#35,#21);
#301=IFCPRODUCTDEFINITIONSHAPE($,$,(#302));
#302=IFCSHAPEREPRESENTATION(#20,'Body','Tessellation',(#303));
#303=IFCTRIANGULATEDFACESET(#304,$,.T.,((1,3,2),(1,4,3),(5,6,7),(5,7,8),(1,2,6),(1,6,5),(2,3,7),(2,7,6),(3,4,8),(3,8,7),(4,1,5),(4,5,8)),$);
#304=IFCCARTESIANPOINTLIST3D(((0.,0.,0.),(1000.,0.,0.),(1000.,1000.,0.),(0.,1000.,0.),(0.,0.,100.),(1000.,0.,100.),(1000.,1000.,100.),(0.,1000.,100.)));
#310=IFCBEAM('EYvctVUKr0kugbFTf53O9L',$,'Beam',$,$,#211,#311,$,$);
#311=IFCPRODUCTDEFINITIONSHAPE($,$,(#312));
#312=IFCSHAPEREPRESENTATION(#20,'Body','Clipping',(#313));
#313=IFCBOOLEANCLIPPINGRESULT(.DIFFERENCE.,#314,#317);
#314=IFCEXTRUDEDAREASOLID(#315,#21,#116,1000.);
#315=IFCRECTANGLEPROFILEDEF(.AREA.,$,#316,200.,200.);
#316=IFCAXIS2PLACEMENT2D(#22,$);
#116=IFCDIRECTION((0.,0.,1.));
#317=IFCHALFSPACESOLID(#318,.F.);
#318=IFCPLANE(#319);
#319=IFCAXIS2PLACEMENT3D(#320,$,$);
#320=IFCCARTESIANPOINT((0.,0.,600.));
"
    );
    let m = read(&ifc("IFC4X3_ADD2", &data), &Limits::default(), None).unwrap();
    let by: HashMap<&str, &Element> = m.elements.iter().map(|e| (e.kind.as_str(), e)).collect();
    let slab = by["IFCSLAB"];
    assert_eq!(slab.triangles, 12);
    assert!((slab.volume.unwrap() - 1e8).abs() < 1e-3);
    // The beam clipped at z = 600 (the half-space above, its material on the normal's
    // side, removed).
    let beam = by["IFCBEAM"];
    assert!(
        (beam.volume.unwrap() - 200.0 * 200.0 * 600.0).abs() < 1.0,
        "{:?}",
        beam.volume
    );
    assert!((beam.max[2] - 600.0).abs() < 1e-9);
    // No styles: one material per kind.
    let names: Vec<&str> = m.mesh.materials.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
}

#[test]
fn refusals_are_plain() {
    let e = read(
        b"<?xml version=\"1.0\"?><ifcXML/>",
        &Limits::default(),
        None,
    )
    .err()
    .unwrap()
    .to_string();
    assert!(e.contains("ifcXML"), "{e}");
    let e = read(
        &ifc("AUTOMOTIVE_DESIGN", "#1=IFCPROJECT('x',$,$,$,$,$,$,$,$);"),
        &Limits::default(),
        None,
    )
    .err()
    .unwrap()
    .to_string();
    assert!(e.contains("not an IFC model"), "{e}");
    let e = read(&ifc("IFC4", SPATIAL), &Limits::default(), None)
        .err()
        .unwrap()
        .to_string();
    assert!(e.contains("no elements with geometry"), "{e}");
    // Junk and truncation never panic.
    let full = ifc("IFC4", &format!("{SPATIAL}{WALL}"));
    for n in (0..full.len()).step_by(97) {
        let _ = read(&full[..n], &Limits::default(), None);
    }
}

/// An advanced B-rep face on a flat B-spline surface is drawn without OpenCASCADE, like a
/// plane's; a curved one is still left out with the note.
#[test]
fn flat_bspline_faces_need_no_tessellator() {
    let face = |z11: f64| {
        format!(
            "\
#500=IFCADVANCEDBREP(#501);
#501=IFCCLOSEDSHELL((#502));
#502=IFCADVANCEDFACE((#503),#504,.T.);
#503=IFCFACEOUTERBOUND(#505,.T.);
#504=IFCBSPLINESURFACEWITHKNOTS(1,1,((#510,#511),(#513,#512)),.PLANE_SURF.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.);
#505=IFCEDGELOOP((#520,#521,#522,#523));
#510=IFCCARTESIANPOINT((0.,0.,0.));
#511=IFCCARTESIANPOINT((1000.,0.,0.));
#512=IFCCARTESIANPOINT((1000.,1000.,{z11}));
#513=IFCCARTESIANPOINT((0.,1000.,0.));
#514=IFCVERTEXPOINT(#510);
#515=IFCVERTEXPOINT(#511);
#516=IFCVERTEXPOINT(#512);
#517=IFCVERTEXPOINT(#513);
#520=IFCORIENTEDEDGE(*,*,#530,.T.);
#521=IFCORIENTEDEDGE(*,*,#531,.T.);
#522=IFCORIENTEDEDGE(*,*,#532,.T.);
#523=IFCORIENTEDEDGE(*,*,#533,.T.);
#530=IFCEDGECURVE(#514,#515,#540,.T.);
#531=IFCEDGECURVE(#515,#516,#541,.T.);
#532=IFCEDGECURVE(#516,#517,#542,.T.);
#533=IFCEDGECURVE(#517,#514,#543,.T.);
#540=IFCPOLYLINE((#510,#511));
#541=IFCPOLYLINE((#511,#512));
#542=IFCPOLYLINE((#512,#513));
#543=IFCPOLYLINE((#513,#510));
"
        )
    };
    let curved_note = |m: &IfcModel| m.mesh.notes.iter().any(|n| n.contains("OpenCASCADE"));
    let m = read(&wall_with("#500", &face(0.0)), &Limits::default(), None).unwrap();
    let wall = m.elements.iter().find(|e| e.kind == "IFCWALL").unwrap();
    assert_eq!(wall.triangles, 2);
    assert!(!curved_note(&m), "{:?}", m.mesh.notes);
    // One corner lifted: a twisted (curved) surface, still left to OpenCASCADE (with
    // nothing else to draw, the model is refused saying so).
    let e = read(&wall_with("#500", &face(300.0)), &Limits::default(), None).unwrap_err();
    assert!(e.to_string().contains("OpenCASCADE is needed"), "{e}");
}

// ------------------------------------------------------------------- hostile references

/// The wall of [`WALL`] with its body representation's items replaced by `items`, plus
/// `extra` entities.
fn wall_with(items: &str, extra: &str) -> Vec<u8> {
    let wall = WALL.replace(
        "#111=IFCSHAPEREPRESENTATION(#20,'Body','SweptSolid',(#112));",
        &format!("#111=IFCSHAPEREPRESENTATION(#20,'Body','SweptSolid',({items}));"),
    );
    ifc("IFC4", &format!("{SPATIAL}{wall}{extra}"))
}

/// Reads on a small stack, and must finish quickly (before, these looped or took
/// exponential time).
fn read_hostile(bytes: &[u8]) -> Result<IfcModel> {
    let t = std::time::Instant::now();
    let r = specular_thread::on_stack(256 * 1024, || read(bytes, &Limits::default(), None));
    assert!(t.elapsed().as_secs() < 60, "took {:?}", t.elapsed());
    r
}

#[test]
fn opening_mapping_itself_is_cut() {
    // The door opening's body is a mapped item whose representation holds that same item.
    let wall = WALL.replace(
        "#141=IFCSHAPEREPRESENTATION(#20,'Body','SweptSolid',(#142));",
        "#141=IFCSHAPEREPRESENTATION(#20,'Body','SweptSolid',(#400));\n\
         #400=IFCMAPPEDITEM(#401,#403);\n\
         #401=IFCREPRESENTATIONMAP(#21,#402);\n\
         #402=IFCSHAPEREPRESENTATION(#20,'Body','MappedRepresentation',(#400,#400));\n\
         #403=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#22,1.,$);",
    );
    let m = read_hostile(&ifc("IFC4", &format!("{SPATIAL}{wall}"))).unwrap();
    assert_eq!(m.elements.len(), 1);
}

#[test]
fn representation_mapping_itself_is_bounded() {
    // Eight items, each mapping the representation that holds them: 8^12 visits before.
    let items: Vec<String> = (0..8).map(|k| format!("#{}", 410 + k)).collect();
    let mut extra = String::from(
        "#400=IFCREPRESENTATIONMAP(#21,#401);\n\
         #401=IFCSHAPEREPRESENTATION(#20,'Body','MappedRepresentation',(#112,ITEMS));\n\
         #402=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#22,1.,$);\n",
    )
    .replace("ITEMS", &items.join(","));
    for k in 0..8 {
        extra += &format!("#{}=IFCMAPPEDITEM(#400,#402);\n", 410 + k);
    }
    let m = read_hostile(&wall_with("#112,#410", &extra)).unwrap();
    assert_eq!(m.elements.len(), 1);
}

#[test]
fn boolean_of_itself_is_bounded() {
    // Each union joins the next one with itself: 2^24 blocks before.
    let mut extra = String::new();
    for k in 0..24 {
        let (a, b) = (500 + k, 501 + k);
        extra += &format!("#{a}=IFCBOOLEANRESULT(.UNION.,#{b},#{b});\n");
    }
    extra += "#524=IFCBLOCK(#21,10.,10.,10.);\n";
    let m = read_hostile(&wall_with("#112,#500", &extra)).unwrap();
    assert_eq!(m.elements.len(), 1);
}

#[test]
fn curve_of_itself_is_bounded() {
    // A composite curve whose four segments are that curve again: 4^32 visits before.
    let extra = "#600=IFCCOMPOSITECURVE((#601,#601,#601,#601),.F.);\n\
                 #601=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#600);\n\
                 #602=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#600);\n\
                 #603=IFCEXTRUDEDAREASOLID(#602,#21,#116,100.);\n";
    let m = read_hostile(&wall_with("#112,#603", extra)).unwrap();
    assert_eq!(m.elements.len(), 1);
}

#[test]
fn styles_and_materials_of_themselves_are_bounded() {
    // A style assignment and a material list listing themselves 32 times: 32^9 and 32^7
    // visits before.
    let refs = |id: u32| vec![format!("#{id}"); 32].join(",");
    let extra = format!(
        "#700=IFCPRESENTATIONSTYLEASSIGNMENT(({}));\n\
         #701=IFCSTYLEDITEM(#112,(#700),$);\n\
         #710=IFCMATERIALLIST(({}));\n\
         #711=IFCRELASSOCIATESMATERIAL('DYvctVUKr0kugbFTf53O9L',$,$,$,(#100),#710);\n",
        refs(700),
        refs(710)
    );
    let m = read_hostile(&wall_with("#112", &extra)).unwrap();
    assert_eq!(m.elements.len(), 1);
}

// ------------------------------------------------------------------------------- corpus

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MODELS2_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../tools/corpus/models2"
            ))
        });
    if dir.join("ifc").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: no IFC corpus at {}", dir.display());
        None
    }
}

fn ifc_files(root: &std::path::Path) -> Vec<PathBuf> {
    fn walk(d: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        v.sort();
        for p in v {
            if p.is_dir() {
                walk(&p, out);
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("ifc") || e.eq_ignore_ascii_case("ifczip"))
            {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("ifc"), &mut out);
    out
}

/// Every corpus file opens or is refused with a reason; prints a line per file
/// (`SPECULAR_IFC_ONLY=part-of-path` narrows it).
#[test]
#[ignore = "prints timings; run by hand"]
fn corpus_survey() {
    let Some(root) = corpus() else { return };
    let only = std::env::var("SPECULAR_IFC_ONLY").unwrap_or_default();
    for p in ifc_files(&root) {
        let rel = p.strip_prefix(&root).unwrap().display().to_string();
        if !rel.contains(&only) {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let t = std::time::Instant::now();
        match read(&bytes, &Limits::default(), None) {
            Ok(m) => eprintln!(
                "{rel}: {} elements, {} tris, {:.0} ms ({}); notes {:?}",
                m.elements.len(),
                m.mesh.triangle_count(),
                t.elapsed().as_secs_f64() * 1e3,
                m.facts
                    .iter()
                    .find(|(k, _)| k == "Read in")
                    .map_or("", |(_, v)| v.as_str()),
                m.mesh.notes
            ),
            Err(e) => eprintln!("{rel}: refused: {e}"),
        }
    }
}

/// `SPECULAR_IFC_DUMP=relpath:id`: one element's pieces, before openings are cut.
#[test]
fn dump_element() {
    let Ok(spec) = std::env::var("SPECULAR_IFC_DUMP") else {
        return;
    };
    let Some(root) = corpus() else { return };
    let (rel, id) = spec.rsplit_once(':').unwrap();
    let id: u32 = id.parse().unwrap();
    let bytes = std::fs::read(root.join(rel)).unwrap();
    let data = unpack(&bytes, &Limits::default()).unwrap();
    let s = Step::index(&data).unwrap();
    let (length, angle, _) = units(&s);
    let ctx = Ctx {
        styles: Styles::find(&s),
        s,
        angle,
        length,
        eps: 1e-5 / length,
        segments: 32,
        colour_maps: HashMap::new(),
        grid_axes: HashMap::new(),
        mapped: Mutex::new(HashMap::new()),
        notes: Mutex::new(BTreeMap::new()),
        defer_breps: false,
        work: Work::new(1 << 20, &Limits::default()),
    };
    if let Ok(f) = std::env::var("SPECULAR_IFC_DUMP_SHELL") {
        let shell: u32 = f.parse().unwrap();
        for face in ctx.s.args(shell).unwrap()[0].refs() {
            let mut s = Shape::default();
            ctx.shell_face(face, &mut s);
            eprintln!("face #{face}: {} tris, area {:.1}", s.tri.len(), s.area());
        }
    }
    let p = find_products(&ctx)
        .into_iter()
        .find(|p| p.id == id)
        .unwrap();
    for rep in ctx.body_representations(p.pds) {
        for it in ctx
            .s
            .args(rep)
            .and_then(|a| a.get(3).map(V::refs))
            .unwrap_or_default()
        {
            for piece in ctx.item(it, 0, &mut Vec::new(), &Mat::IDENTITY) {
                let sh = &piece.shape;
                eprintln!(
                    "rep #{rep} item #{it} {}: {} tris, closed {}, volume {:.6}, area {:.6}, bounds {:?}",
                    ctx.s.ty(it).unwrap_or(""),
                    sh.tri.len(),
                    sh.closed,
                    sh.volume(),
                    sh.area(),
                    sh.bounds()
                );
            }
        }
    }
    let b = build(&ctx, &p, &voids(&ctx).get(&id).cloned().unwrap_or_default());
    for piece in &b.pieces {
        eprintln!(
            "built: {} tris, closed {}, volume {:.6}, area {:.6}",
            piece.shape.tri.len(),
            piece.shape.closed,
            piece.shape.volume(),
            piece.shape.area()
        );
    }
    eprintln!("openings {:?}", b.openings);
}

/// Files where IfcOpenShell is shown wrong, with the evidence: not compared.
const ORACLE_WRONG: &[(&str, &str)] = &[(
    "placements-and-gis-referencing/grid-placement.ifc",
    "IfcOpenShell 0.9.0 leaves out the 25 columns placed by IfcGridPlacement; ours stand on \
     the grid's intersections (e.g. #293 at x -14, y 4 m, under the beams that meet there)",
)];

/// Elements (file, type) whose IfcOpenShell volume is shown wrong: their volumes are not
/// compared (bounds and areas still are).
const VOLUME_WRONG: &[(&str, &str, &str)] = &[(
    "webifc/advanced_model.ifc",
    "IFCFLOWTERMINAL",
    "the 150 terminals are three closed shells each; IfcOpenShell's volume is the sum with \
     the largest shell inside out: |0.016747 + 0.001772 − 0.148816| = 0.130297 m³ (#183707), \
     ours 0.167334",
)];

/// One IfcOpenShell row: type, volume m³, area m², bounds in metres.
struct Oracle {
    kind: String,
    volume: Option<f64>,
    area: f64,
    min: [f64; 3],
    max: [f64; 3],
}

fn oracle(root: &std::path::Path) -> Option<HashMap<String, HashMap<u32, Oracle>>> {
    let text = std::fs::read_to_string(root.join("oracle/ifc.tsv")).ok()?;
    let mut out: HashMap<String, HashMap<u32, Oracle>> = HashMap::new();
    let v3 = |s: &str| -> Option<[f64; 3]> {
        let v: Vec<f64> = s.split(',').filter_map(|x| x.parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    };
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let entry = out.entry(f[0].to_owned()).or_default();
        if f.len() < 8 || f[1] == "-" {
            continue;
        }
        let Ok(id) = f[1].parse::<u32>() else {
            continue;
        };
        let (Some(min), Some(max)) = (v3(f[6]), v3(f[7])) else {
            continue;
        };
        entry.insert(
            id,
            Oracle {
                kind: f[2].to_ascii_uppercase(),
                volume: f[4].parse().ok(),
                area: f[5].parse().unwrap_or(0.0),
                min,
                max,
            },
        );
    }
    Some(out)
}

/// Mismatches against IfcOpenShell, in all.
#[derive(Debug, Default)]
struct Tally {
    elements: usize,
    missing: usize,
    extra: usize,
    bounds: usize,
    volumes: usize,
    volumes_off: usize,
    areas_off: usize,
    /// Elements drawn in files IfcOpenShell draws nothing of.
    oracle_none: usize,
}

fn compare(
    root: &std::path::Path,
    oracle: &HashMap<String, HashMap<u32, Oracle>>,
    verbose: bool,
) -> Tally {
    let mut files: Vec<&String> = oracle.keys().collect();
    files.sort();
    let mut t = Tally::default();
    // ORACLE lines start their own line (the harness's "test ... " has none).
    println!();
    for rel in files {
        // IfcOpenShell reads this file's millimetres as metres.
        // Per file, for tools/ci/oracle-pass-rates.sh: pass when every element the oracle
        // draws agrees (the test itself gates the totals).
        let line = |status: &str| println!("ORACLE\tIFC\t{rel}\t{status}");
        if rel.ends_with("refview/wall-with-opening-and-window.ifc") {
            line("skip");
            continue;
        }
        let want = &oracle[rel];
        let Ok(bytes) = std::fs::read(root.join(rel)) else {
            line("skip");
            continue;
        };
        if let Some((_, why)) = ORACLE_WRONG.iter().find(|(f, _)| rel.ends_with(f)) {
            eprintln!("{rel}: not compared: {why}");
            line("skip");
            continue;
        }
        // Curved advanced B-reps need the optional OpenCASCADE tessellator (specular-decode,
        // in the decode host), which this test runs without: not comparable here.
        let occt = |text: &str| text.contains("OpenCASCADE is needed");
        let m = match read(&bytes, &Limits::default(), None) {
            Ok(m) if m.mesh.notes.iter().any(|n| occt(n)) => {
                eprintln!("{rel}: not compared: curved faces need OpenCASCADE");
                line("skip");
                continue;
            }
            Ok(m) => m,
            Err(e) if occt(&e.to_string()) => {
                eprintln!("{rel}: not compared: refused without OpenCASCADE ({e})");
                line("skip");
                continue;
            }
            Err(e) => {
                if !want.is_empty() {
                    eprintln!("{rel}: refused ({e}) but IfcOpenShell draws {}", want.len());
                    t.missing += want.len();
                    t.elements += want.len();
                    line("fail");
                } else {
                    line("skip");
                }
                continue;
            }
        };
        let f = m.metres_per_unit;
        let got: HashMap<u32, &Element> = m.elements.iter().map(|e| (e.id, e)).collect();
        let (mut fm, mut fb, mut fv, mut fa) = (0, 0, 0, 0);
        let mut ids: Vec<&u32> = want.keys().collect();
        ids.sort();
        for id in ids {
            let o = &want[id];
            if hidden(&o.kind) {
                continue;
            }
            t.elements += 1;
            let Some(e) = got.get(id) else {
                fm += 1;
                if verbose && fm <= 3 {
                    eprintln!("  {rel}: #{id} {} missing", o.kind);
                }
                continue;
            };
            let diag = (0..3)
                .map(|k| (o.max[k] - o.min[k]).powi(2))
                .sum::<f64>()
                .sqrt();
            let tol = (diag * 0.02).max(0.005);
            let off = (0..3)
                .map(|k| {
                    (e.min[k] * f - o.min[k])
                        .abs()
                        .max((e.max[k] * f - o.max[k]).abs())
                })
                .fold(0.0f64, f64::max);
            if off > tol {
                fb += 1;
                if verbose && fb <= 3 {
                    eprintln!("  {rel}: #{id} {} bounds off by {off:.4} m", o.kind);
                }
            }
            let volume_wrong = VOLUME_WRONG
                .iter()
                .any(|(f, k, _)| rel.ends_with(f) && o.kind == *k);
            if let (Some(a), Some(b), false) = (e.volume, o.volume, volume_wrong) {
                t.volumes += 1;
                let a = a * f * f * f;
                if (a - b).abs() > (b.abs() * 0.03).max(1e-6) {
                    fv += 1;
                    if verbose && fv <= 3 {
                        eprintln!("  {rel}: #{id} {} volume {a:.6} vs {b:.6}", o.kind);
                    }
                }
            }
            // Areas of shapes IfcOpenShell leaves open are not comparable.
            let area = e.area * f * f;
            if o.volume.is_some() && (area - o.area).abs() > (o.area * 0.05).max(1e-4) {
                fa += 1;
                if verbose && fa <= 2 {
                    eprintln!("  {rel}: #{id} {} area {area:.4} vs {:.4}", o.kind, o.area);
                }
            }
        }
        let fx = got.keys().filter(|id| !want.contains_key(id)).count();
        if verbose {
            let mut extra: Vec<&&Element> = got
                .iter()
                .filter(|(id, _)| !want.contains_key(id))
                .map(|(_, e)| e)
                .collect();
            extra.sort_by_key(|e| e.id);
            for e in extra.iter().take(3) {
                eprintln!(
                    "  {rel}: #{} extra, {:.3?} to {:.3?} m",
                    e.id,
                    e.min.map(|v| v * f),
                    e.max.map(|v| v * f)
                );
            }
        }
        if want.is_empty() {
            // IfcOpenShell draws nothing here (its own failures): not compared.
            t.oracle_none += fx;
            line("skip");
            continue;
        }
        line(if fm + fx + fb + fv + fa > 0 {
            "fail"
        } else {
            "pass"
        });
        if verbose && fm + fx + fb + fv + fa > 0 {
            eprintln!(
                "{rel}: {} elements; {fm} missing, {fx} extra, {fb} bounds, {fv} volumes, {fa} areas off",
                want.len()
            );
        }
        t.missing += fm;
        t.extra += fx;
        t.bounds += fb;
        t.volumes_off += fv;
        t.areas_off += fa;
    }
    t
}

/// Elements, bounds, volumes and areas against IfcOpenShell's.
#[test]
fn corpus_against_ifcopenshell() {
    let Some(root) = corpus() else { return };
    let Some(oracle) = oracle(&root) else {
        eprintln!("skipped: no oracle/ifc.tsv (run gen/ifc_oracle.py)");
        return;
    };
    let t = compare(
        &root,
        &oracle,
        std::env::var_os("SPECULAR_IFC_VERBOSE").is_some(),
    );
    eprintln!("IFC vs IfcOpenShell: {t:?}");
    // What is left (see ORACLE_WRONG and VOLUME_WRONG for the oracle's own errors): the
    // two IFC4x3 sweeps along alignments not drawn (missing), the curved sweeps of
    // ISSUE_171 (3 bounds, 2 volumes, 1 area) and two signals placed along an alignment
    // from their cached positions (2 bounds).
    assert!(t.missing <= 2, "{t:?}");
    assert!(t.bounds <= 5, "{t:?}");
    assert!(t.volumes_off <= 2, "{t:?}");
    assert!(t.areas_off <= 1, "{t:?}");
}
