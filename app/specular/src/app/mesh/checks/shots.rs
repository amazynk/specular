//! Screenshot tests of the model checks panel: IDS results and a failing element shown,
//! a clash with the pair selected and the rest ghosted, the changes against an older
//! version in colour, quantities by class; at 1× and 1.5×, in English and right to left in
//! Hebrew. A small IFC building made here (walls, a slab, a duct through a wall, a beam)
//! read by Specular's own IFC reader, drawn by the app's mesh path on lavapipe and compared
//! with the PNGs in `app/specular/tests/shots/`; the panel is audited for screen readers.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular checks::shots`, then review and
//! commit the rewritten PNGs.

use std::sync::Arc;

use specular_decode::specular_mesh::Unit;
use specular_render::PxRect;
use specular_render_3d::{Shading, StandardView, canonical_bounds};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::*;
use crate::app::mesh::bim::{a11y, draw_panel};

const SIZE: (f32, f32) = (560.0, 380.0);

/// An element: class, GlobalId, name, box corners (m), extra properties.
pub(in crate::app::mesh) type Part<'a> = (&'a str, &'a str, &'a str, [f64; 3], [f64; 3], &'a str);

/// An IFC4 file of boxes in one storey; `props` adds a `Pset_WallCommon` FireRating.
pub(in crate::app::mesh) fn ifc(parts: &[Part]) -> Vec<u8> {
    let mut s = String::from(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [ReferenceView]'),'2;1');\n\
         FILE_NAME('checks.ifc','2026-10-25T00:00:00',('A'),('B'),'','shots','');\n\
         FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
         #1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'Checks',$,$,$,$,(#2),#3);\n\
         #2=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#4,$);\n\
         #3=IFCUNITASSIGNMENT((#5));\n#4=IFCAXIS2PLACEMENT3D(#6,$,$);\n\
         #5=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n#6=IFCCARTESIANPOINT((0.,0.,0.));\n\
         #7=IFCLOCALPLACEMENT($,#4);\n\
         #8=IFCBUILDINGSTOREY('2YvctVUKr0kugbFTf53O9L',$,'Ground floor',$,$,#7,$,$,.ELEMENT.,0.);\n\
         #9=IFCRELAGGREGATES('3YvctVUKr0kugbFTf53O9L',$,$,$,#1,(#8));\n",
    );
    let mut contained = Vec::new();
    let mut n = 100;
    for (class, guid, name, lo, hi, props) in parts {
        let id = n;
        n += 10;
        let c = |k: usize, hi_side: bool| if hi_side { hi[k] } else { lo[k] };
        let pts: Vec<String> = (0..8)
            .map(|k| {
                format!(
                    "({:.3},{:.3},{:.3})",
                    c(0, k & 1 == 1),
                    c(1, k & 2 == 2),
                    c(2, k & 4 == 4)
                )
            })
            .collect();
        let faces = "(1,3,4),(1,4,2),(5,6,8),(5,8,7),(1,2,6),(1,6,5),(3,7,8),(3,8,4),(1,5,7),(1,7,3),(2,4,8),(2,8,6)";
        s.push_str(&format!(
            "#{id}={class}('{guid}',$,'{name}',$,$,#7,#{},$,$);\n\
             #{}=IFCPRODUCTDEFINITIONSHAPE($,$,(#{}));\n\
             #{}=IFCSHAPEREPRESENTATION(#2,'Body','Tessellation',(#{}));\n\
             #{}=IFCTRIANGULATEDFACESET(#{},$,.T.,({faces}),$);\n\
             #{}=IFCCARTESIANPOINTLIST3D(({}));\n",
            id + 1,
            id + 1,
            id + 2,
            id + 2,
            id + 3,
            id + 3,
            id + 4,
            id + 4,
            pts.join(",")
        ));
        if !props.is_empty() {
            s.push_str(&format!(
                "#{}=IFCPROPERTYSET('{guid}PS',$,'Pset_WallCommon',$,(#{}));\n\
                 #{}=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('{props}'),$);\n\
                 #{}=IFCRELDEFINESBYPROPERTIES('{guid}RL',$,$,$,(#{id}),#{});\n",
                id + 5,
                id + 6,
                id + 6,
                id + 7,
                id + 5
            ));
        }
        contained.push(format!("#{id}"));
    }
    s.push_str(&format!(
        "#90=IFCRELCONTAINEDINSPATIALSTRUCTURE('4YvctVUKr0kugbFTf53O9L',$,$,$,({}),#8);\nENDSEC;\nEND-ISO-10303-21;\n",
        contained.join(",")
    ));
    s.into_bytes()
}

const NEW: &[Part] = &[
    (
        "IFCSLAB",
        "1SlabAAAAAAAAAAAAAAAAA",
        "Slab",
        [-0.5, -0.5, -0.25],
        [12.5, 8.5, 0.0],
        "",
    ),
    (
        "IFCWALL",
        "1WallSouthAAAAAAAAAAAA",
        "South",
        [0.0, 0.0, 0.0],
        [12.0, 0.25, 2.8],
        "2HR",
    ),
    (
        "IFCWALL",
        "1WallNorthAAAAAAAAAAAA",
        "North",
        [0.0, 7.75, 0.0],
        [12.0, 8.0, 2.8],
        "1HR",
    ),
    (
        "IFCWALL",
        "1WallWestAAAAAAAAAAAAA",
        "West",
        [0.0, 0.25, 0.0],
        [0.25, 7.75, 2.8],
        "",
    ),
    (
        "IFCWALL",
        "1WallEastAAAAAAAAAAAAA",
        "East",
        [11.75, 0.25, 0.0],
        [12.0, 7.75, 2.8],
        "2HR",
    ),
    (
        "IFCWALL",
        "1WallMiddleAAAAAAAAAAA",
        "Middle",
        [6.0, 0.25, 0.0],
        [6.2, 7.75, 2.8],
        "",
    ),
    (
        "IFCDUCTSEGMENT",
        "1DuctAAAAAAAAAAAAAAAAA",
        "Supply duct",
        [2.0, 3.8, 2.2],
        [10.0, 4.2, 2.5],
        "",
    ),
    (
        "IFCBEAM",
        "1BeamAAAAAAAAAAAAAAAAA",
        "Beam",
        [0.0, 5.9, 2.5],
        [12.0, 6.1, 2.8],
        "",
    ),
];

/// The older version: no beam, the middle wall elsewhere, a column since removed and the
/// north wall rated differently.
const OLD: &[Part] = &[
    (
        "IFCSLAB",
        "1SlabAAAAAAAAAAAAAAAAA",
        "Slab",
        [-0.5, -0.5, -0.25],
        [12.5, 8.5, 0.0],
        "",
    ),
    (
        "IFCWALL",
        "1WallSouthAAAAAAAAAAAA",
        "South",
        [0.0, 0.0, 0.0],
        [12.0, 0.25, 2.8],
        "2HR",
    ),
    (
        "IFCWALL",
        "1WallNorthAAAAAAAAAAAA",
        "North",
        [0.0, 7.75, 0.0],
        [12.0, 8.0, 2.8],
        "2HR",
    ),
    (
        "IFCWALL",
        "1WallWestAAAAAAAAAAAAA",
        "West",
        [0.0, 0.25, 0.0],
        [0.25, 7.75, 2.8],
        "",
    ),
    (
        "IFCWALL",
        "1WallEastAAAAAAAAAAAAA",
        "East",
        [11.75, 0.25, 0.0],
        [12.0, 7.75, 2.8],
        "2HR",
    ),
    (
        "IFCWALL",
        "1WallMiddleAAAAAAAAAAA",
        "Middle",
        [5.0, 0.25, 0.0],
        [5.2, 7.75, 2.8],
        "",
    ),
    (
        "IFCDUCTSEGMENT",
        "1DuctAAAAAAAAAAAAAAAAA",
        "Supply duct",
        [2.0, 3.8, 2.2],
        [10.0, 4.2, 2.5],
        "",
    ),
    (
        "IFCCOLUMN",
        "1ColumnAAAAAAAAAAAAAAA",
        "Column",
        [9.0, 2.0, 0.0],
        [9.3, 2.3, 2.8],
        "",
    ),
];

const IDS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ids xmlns="http://standards.buildingsmart.org/IDS" xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <info><title>Fire safety</title></info>
  <specifications>
    <specification name="Walls have a fire rating" ifcVersion="IFC4">
      <applicability minOccurs="1" maxOccurs="unbounded">
        <entity><name><simpleValue>IFCWALL</simpleValue></name></entity>
      </applicability>
      <requirements>
        <property dataType="IFCLABEL">
          <propertySet><simpleValue>Pset_WallCommon</simpleValue></propertySet>
          <baseName><simpleValue>FireRating</simpleValue></baseName>
          <value><xs:restriction base="xs:string"><xs:enumeration value="1HR"/><xs:enumeration value="2HR"/></xs:restriction></value>
        </property>
      </requirements>
    </specification>
    <specification name="Every element has a name" ifcVersion="IFC4">
      <applicability minOccurs="1" maxOccurs="unbounded">
        <entity><name><simpleValue>IFCBEAM</simpleValue></name></entity>
      </applicability>
      <requirements>
        <attribute><name><simpleValue>Name</simpleValue></name></attribute>
      </requirements>
    </specification>
    <specification name="No proxies" ifcVersion="IFC4">
      <applicability minOccurs="0" maxOccurs="0">
        <entity><name><simpleValue>IFCBUILDINGELEMENTPROXY</simpleValue></name></entity>
      </applicability>
    </specification>
  </specifications>
</ids>"#;

/// A model as the app holds it, and its information.
fn model(parts: &[Part]) -> (MeshModel, Arc<IfcData>) {
    let bytes = ifc(parts);
    let m = specular_decode::specular_mesh::ifc::read(&bytes, &Limits::default(), None).unwrap();
    let d = ifcdata::read(&bytes, &Limits::default()).unwrap();
    let mut elements = m.elements.clone();
    elements.sort_by_key(|e| e.first_vertex);
    let mut mesh = m.mesh;
    mesh.unit = Unit::Metre;
    let index = specular_decode::IfcIndex {
        elements,
        mesh_metres: m.mesh_metres,
        origin_metres: m.origin_metres,
        georef: m.georef,
    };
    (
        MeshModel {
            mesh: Arc::new(mesh),
            textures: Arc::new(Vec::new()),
            ifc: Some(Arc::new(index)),
        },
        Arc::new(d),
    )
}

fn other(parts: &[Part]) -> Arc<Other> {
    other_bytes(&ifc(parts))
}

/// Another version of a model, from its file's bytes.
pub(in crate::app::mesh) fn other_bytes(bytes: &[u8]) -> Arc<Other> {
    let m = specular_decode::specular_mesh::ifc::read(bytes, &Limits::default(), None).unwrap();
    let mut elements = m.elements;
    elements.sort_by_key(|e| e.first_vertex);
    Arc::new(Other {
        data: ifcdata::read(bytes, &Limits::default()).unwrap(),
        mesh: m.mesh,
        elements,
        mesh_metres: m.mesh_metres,
        origin_metres: m.origin_metres,
    })
}

#[derive(Clone, Copy, PartialEq)]
enum What {
    Ids,
    Failure,
    Clash,
    Changes,
    Quantities,
}

/// Sets the view up as a user would get there, through the panel's own code.
fn setup(mv: &mut MeshView, what: What) {
    let (model, data) = model(NEW);
    mv.model = Some(model.clone());
    mv.bounds = canonical_bounds(&model.mesh);
    mv.shading = Shading::Edges;
    mv.grid = true;
    mv.look(StandardView::Iso);
    mv.bim.panel = Some(super::super::bim::PanelKind::Checks);
    let c = &mut mv.bim.checks;
    c.data = Some(data.clone());
    c.ids = Some((
        PathBuf::from("fire-safety.ids"),
        Arc::new(idscheck::parse(IDS).unwrap()),
    ));
    c.report = Some(idscheck::check(c.ids.as_ref().unwrap().1.as_ref(), &data));
    let ifc = model.ifc.clone().unwrap();
    c.quantities = Some(qto::rows(
        &data,
        &ifc.elements,
        &model.mesh,
        ifc.mesh_metres,
    ));
    match what {
        What::Ids => {}
        What::Failure => {
            mv.bim.checks.open_spec = Some((0, 0));
            mv.bim.row = 0;
            assert!(mv.checks_go_failure());
        }
        What::Clash => {
            c.tab = Tab::Clashes;
            let all: Vec<usize> = (0..ifc.elements.len()).collect();
            let m = clash::Model {
                positions: &model.mesh.positions,
                indices: &model.mesh.indices,
                elements: &ifc.elements,
            };
            let found = clash::run(&m, &all, &all, Mode::Hard { tolerance: 0.01 }, &|_, _| {
                false
            });
            mv.checks_clashes_found(found, 12.0, None);
            let row = mv
                .bim
                .checks
                .clashes
                .as_ref()
                .unwrap()
                .iter()
                .position(|f| {
                    ifc.elements[f.clash.a].kind == "IFCDUCTSEGMENT"
                        || ifc.elements[f.clash.b].kind == "IFCDUCTSEGMENT"
                })
                .unwrap();
            mv.bim.row = row;
            assert!(mv.checks_enter());
        }
        What::Changes => {
            c.tab = Tab::Changes;
            c.other = Some((PathBuf::from("checks-rev-a.ifc"), other(OLD)));
            mv.checks_compare();
            mv.bim.row = 0;
        }
        What::Quantities => {
            c.tab = Tab::Quantities;
            c.by = Some(By::Class);
        }
    }
    // Animations end at once.
    mv.finish_anim();
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, name: &str, what: What, rtl: bool, scale: f32) {
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let mut mv = MeshView {
        rect: Some(PxRect {
            x: 0.0,
            y: 0.0,
            w: size_px.0 as f32,
            h: size_px.1 as f32,
        }),
        ..Default::default()
    };
    setup(&mut mv, what);
    let panel = mv.panel().expect("the panel");
    let pal = Palette::dark();
    let mut drawn = None;
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            App::mesh_render(&mut mv, gpu, enc, target, size, scale);
        },
        |dl, text| {
            drawn = Some(draw_panel(
                dl,
                text,
                &pal,
                &panel,
                (0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            ));
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
    crate::app::a11y_audit::audit_screen(name, SIZE, rtl, |t| {
        if let Some((pr, rows, _, _)) = &drawn {
            a11y(t, &panel, *pr, rows, 1.0);
        }
    });
}

#[test]
fn checks_panel_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-checks",
    );
    // Names, values and file names from the model and the IDS are data in any language.
    for d in [
        "Walls have a fire rating",
        "Every element has a name",
        "No proxies",
        "fire-safety.ids",
        "checks-rev-a.ifc",
        "Pset_WallCommon.FireRating",
        "IfcWall",
        "IfcSlab",
        "IfcBeam",
        "IfcDuctSegment",
        "IfcColumn",
        "South",
        "North",
        "West",
        "East",
        "Middle",
        "Supply duct",
        "Beam",
        "Column",
        "Slab",
        "Ground floor",
        "1HR",
        "2HR",
    ] {
        h.data_text(d);
    }
    let cases = [
        ("checks-ids", What::Ids),
        ("checks-ids-failure", What::Failure),
        ("checks-clash", What::Clash),
        ("checks-changes", What::Changes),
        ("checks-quantities", What::Quantities),
    ];
    for (name, what) in cases {
        for scale in [1.0, 1.5] {
            shoot(&mut h, &mut goldens, name, what, false, scale);
        }
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        for (name, what) in [
            ("checks-ids-he", What::Failure),
            ("checks-clash-he", What::Clash),
            ("checks-changes-he", What::Changes),
        ] {
            for scale in [1.0, 1.5] {
                shoot(&mut h, &mut goldens, name, what, true, scale);
            }
        }
    });
    goldens.finish();
}

#[test]
fn checks_find_what_the_model_has() {
    // IDS: the west and middle walls have no fire rating; the beam is named; no proxies.
    let mut app = MeshView::default();
    setup(&mut app, What::Ids);
    let c = &app.bim.checks;
    let r = c.report.as_ref().unwrap();
    assert_eq!(r.specs.len(), 3);
    assert_eq!((r.specs[0].applicable, r.specs[0].failed.len()), (5, 2));
    assert!(r.specs[1].ok && r.specs[2].ok);
    // Exports: CSV rows for the two failures, a BCF topic each.
    let csv = String::from_utf8(app.checks_bytes(Out::Csv, None).unwrap()).unwrap();
    assert_eq!(csv.lines().count(), 3, "{csv}");
    let bcf = specular_bim::read(&app.checks_bytes(Out::Bcf, None).unwrap()).unwrap();
    assert_eq!(bcf.topics.len(), 2);
    // Clashes: the duct runs through the middle wall (beams sit on walls, slabs under them).
    let mut app = MeshView::default();
    setup(&mut app, What::Clash);
    let found = app.bim.checks.clashes.clone().unwrap();
    let ifc = app.model.as_ref().unwrap().ifc.clone().unwrap();
    let names: Vec<(String, String)> = found
        .iter()
        .map(|f| {
            (
                ifc.elements[f.clash.a].name.clone(),
                ifc.elements[f.clash.b].name.clone(),
            )
        })
        .collect();
    assert!(
        names
            .iter()
            .any(|(a, b)| (a == "Middle" && b == "Supply duct")
                || (b == "Middle" && a == "Supply duct")),
        "{names:?}"
    );
    assert!(app.bim.restyled.is_some(), "the pair is shown");
    assert!(found.iter().all(|f| f.status == Status::New));
    let bcf = specular_bim::read(&app.checks_bytes(Out::Bcf, None).unwrap()).unwrap();
    assert_eq!(bcf.topics.len(), found.len());
    assert_eq!(
        bcf.topics[0].viewpoints[0]
            .vis
            .as_ref()
            .unwrap()
            .components
            .as_ref()
            .unwrap()
            .selection
            .len(),
        2
    );
    // Changes: the beam added, the column removed, the middle wall moved, the north wall's
    // rating changed.
    let mut app = MeshView::default();
    setup(&mut app, What::Changes);
    let ch = app.bim.checks.changes.clone().unwrap();
    assert_eq!(diff::counts(&ch), [1, 1, 2], "{ch:#?}");
    let csv = String::from_utf8(app.checks_bytes(Out::Csv, None).unwrap()).unwrap();
    assert!(
        csv.contains("FireRating") && csv.contains("2HR,1HR"),
        "{csv}"
    );
    // Quantities: the walls' volumes computed from the mesh.
    let mut app = MeshView::default();
    setup(&mut app, What::Quantities);
    let rows = app.bim.checks.quantities.clone().unwrap();
    let south = rows.iter().find(|r| r.name == "South").unwrap();
    assert!((south.volume.unwrap().value - 12.0 * 0.25 * 2.8).abs() < 1e-6);
    assert!(!south.volume.unwrap().from_file);
    assert!((south.area.unwrap().value - 12.0 * 2.8).abs() < 1e-6);
    let xlsx = app.checks_bytes(Out::Xlsx, None).unwrap();
    assert!(xlsx.starts_with(b"PK"));
}
