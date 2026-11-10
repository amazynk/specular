//! Federated IFC models on buildingSMART's Simple-Scene (`tools/corpus/fetch-ifc-federation.sh`;
//! skipped when absent): map coordinates and placements against IfcOpenShell, hard clashes
//! between every two models against the oracle's (`oracle.json`), and the BCF topics of
//! cross-model clashes written in 2.1 and 3.0 and read back with every field compared.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value as J;
use specular_bim::bcf::{self, Version};
use specular_bim::export::{self, Issue};
use specular_bim::federation::{self, Member, Placement};
use specular_bim::{clash, write};
use specular_mesh::Limits;
use specular_mesh::ifc::{self, IfcModel};

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_IFC_FED_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/ifc-federation"),
        PathBuf::from,
    )
}

fn sorted(mut m: IfcModel) -> IfcModel {
    m.elements.sort_by_key(|e| e.first_vertex);
    m
}

#[test]
fn federation_corpus() {
    let root = corpus();
    let Ok(oracle) = std::fs::read(root.join("oracle.json")) else {
        eprintln!(
            "federation corpus not found under {}: skipped",
            root.display()
        );
        return;
    };
    let oracle: J = serde_json::from_slice(&oracle).unwrap();
    let tol_m = oracle["tolerance"].as_f64().unwrap();
    let (mut map_checks, mut map_bad) = (0, 0);
    let (mut place_checks, mut place_bad) = (0, 0);
    let (mut ours_total, mut theirs_total, mut both_total) = (0, 0, 0);
    let mut unjudged_total = 0;
    for (schema, s) in oracle["schemas"].as_object().unwrap() {
        let names: Vec<String> = s["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap().to_owned())
            .collect();
        let t0 = std::time::Instant::now();
        let models: Vec<IfcModel> = names
            .iter()
            .map(|n| {
                let b = std::fs::read(root.join(schema).join(n)).unwrap();
                sorted(ifc::read(&b, &Limits::default(), None).unwrap())
            })
            .collect();
        let read_s = t0.elapsed().as_secs_f64();
        // Map coordinates of sample points.
        for (n, m) in names.iter().zip(&models) {
            let o = &s["maps"][n];
            assert_eq!(
                m.georef.crs_name(),
                o["crs"].as_str(),
                "{schema}/{n}: CRS name"
            );
            for p in o["points"].as_array().unwrap() {
                let v: Vec<Option<f64>> = p.as_array().unwrap().iter().map(J::as_f64).collect();
                let q = [v[0].unwrap(), v[1].unwrap(), v[2].unwrap()];
                map_checks += 1;
                let ours = m.georef.to_map(q);
                let ok = match (ours, v[3]) {
                    (None, None) => true,
                    (Some(e), Some(_)) => (0..3).all(|k| (e[k] - v[3 + k].unwrap()).abs() < 1e-6),
                    _ => false,
                };
                if !ok {
                    map_bad += 1;
                    eprintln!("{schema}/{n}: {q:?} → {ours:?}, IfcOpenShell {:?}", &v[3..]);
                }
            }
        }
        // Placements against the first model.
        let mut places = Vec::new();
        for (n, m) in names.iter().zip(&models) {
            let (p, basis) = federation::align(&models[0].georef, &m.georef)
                .unwrap_or_else(|e| panic!("{schema}/{n}: {e:?}"));
            let o = &s["placements"][n];
            let off: Vec<f64> = o["offset"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let ang = o["angle"].as_f64().unwrap();
            place_checks += 1;
            let ok = (0..3).all(|k| (p.offset()[k] - off[k]).abs() < 1e-6)
                && (p.angle_deg() - ang).abs() < 1e-7;
            if !ok {
                place_bad += 1;
                eprintln!(
                    "{schema}/{n}: placed at {:?} turned {}°, IfcOpenShell {off:?} {ang}°",
                    p.offset(),
                    p.angle_deg()
                );
            }
            eprintln!("{schema}/{n}: {basis:?}");
            places.push(p);
        }
        // Clashes between every two models.
        let members: Vec<Member> = models
            .iter()
            .zip(&places)
            .map(|(m, p)| Member {
                mesh: &m.mesh,
                elements: &m.elements,
                mesh_metres: m.mesh_metres,
                origin_metres: m.origin_metres,
                place: *p,
            })
            .collect();
        let t1 = std::time::Instant::now();
        let merged = federation::merge(&members);
        let merge_s = t1.elapsed().as_secs_f64();
        let mm = models[0].mesh_metres;
        let cm = clash::Model {
            positions: &merged.mesh.positions,
            indices: &merged.mesh.indices,
            elements: &merged.elements,
        };
        let t2 = std::time::Instant::now();
        let mut pairs_tested = 0;
        for (i, a) in names.iter().enumerate() {
            for (j, b) in names.iter().enumerate().skip(i + 1) {
                let sa: Vec<usize> = merged.ranges[i].clone().collect();
                let sb: Vec<usize> = merged.ranges[j].clone().collect();
                let found = clash::run(
                    &cm,
                    &sa,
                    &sb,
                    clash::Mode::Hard {
                        tolerance: tol_m / mm,
                    },
                    &|_, _| false,
                );
                pairs_tested += 1;
                let ours: BTreeSet<(String, String)> = found
                    .iter()
                    .map(|c| {
                        let (x, y) = if merged.member_of(c.a) == Some(i) {
                            (c.a, c.b)
                        } else {
                            (c.b, c.a)
                        };
                        (
                            merged.elements[x].global_id.clone(),
                            merged.elements[y].global_id.clone(),
                        )
                    })
                    .collect();
                let theirs: BTreeSet<(String, String)> = s["clashes"][format!("{a}|{b}")]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .map(|p| {
                                (
                                    p[0].as_str().unwrap().to_owned(),
                                    p[1].as_str().unwrap().to_owned(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let unjudged: BTreeSet<(String, String)> = s["not_solid"][format!("{a}|{b}")]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .map(|p| {
                                (
                                    p[0].as_str().unwrap().to_owned(),
                                    p[1].as_str().unwrap().to_owned(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let both = ours.intersection(&theirs).count();
                unjudged_total += ours.intersection(&unjudged).count();
                ours_total += ours.len();
                theirs_total += theirs.len();
                both_total += both;
                if ours != theirs {
                    eprintln!(
                        "{schema} {a} × {b}: ours {}, IfcOpenShell {}, both {both}; only ours {:?}; only theirs {:?}",
                        ours.len(),
                        theirs.len(),
                        ours.difference(&theirs).take(3).collect::<Vec<_>>(),
                        theirs.difference(&ours).take(3).collect::<Vec<_>>()
                    );
                }
            }
        }
        eprintln!(
            "{schema}: {} models, {} elements, {} triangles; read {read_s:.2} s, merge {merge_s:.3} s, {pairs_tested} model pairs clashed in {:.2} s",
            names.len(),
            merged.elements.len(),
            merged.mesh.triangle_count(),
            t2.elapsed().as_secs_f64()
        );
    }
    eprintln!(
        "map points {map_checks} ({map_bad} mismatches); placements {place_checks} ({place_bad} mismatches); clash pairs ours {ours_total}, IfcOpenShell {theirs_total}, both {both_total}; of ours only, {unjudged_total} where the oracle could not judge (an element not a closed solid)"
    );
    assert_eq!(map_bad, 0);
    assert_eq!(place_bad, 0);
    // The two judge depth differently (deepest vertex or overlap thickness, against twice
    // the overlap's volume over its area): most pairs agree.
    assert!(
        both_total * 100 >= theirs_total * 95,
        "{both_total} of {theirs_total}"
    );
}

/// A cross-model clash topic in both BCF versions, read back: every field the same.
#[test]
fn cross_model_topics_round_trip() {
    let files = vec![
        federation::header_file(
            "Building-Architecture.ifc",
            Some("0YvctVUKr0kugbFTf53O9L"),
            Some("2024-03-01T10:00:00"),
        ),
        federation::header_file("Building-Hvac.ifc", Some("1hqIFTRjfV6AWq_bMtnZwI"), None),
    ];
    let issue = Issue {
        title:
            "IfcWall Wall-001 (Building-Architecture.ifc) × IfcDuctSegment D-7 (Building-Hvac.ifc)"
                .into(),
        description: Some("Hard clash, 120 mm".into()),
        guids: vec![
            "2O2Fr$t4X7Zf8NOew3FLOH".into(),
            "3cUkl32yn9qRSPvBJVyWYp".into(),
        ],
        focus: Some(([1.0, 2.0, 0.0], [3.0, 4.0, 2.5])),
        labels: vec!["Clash".into()],
        status: Some("Open".into()),
        files: files.clone(),
        ..Issue::default()
    };
    for v in [Version::V2_1, Version::V3_0] {
        let a = export::topics(
            std::slice::from_ref(&issue),
            "tester",
            Some("ignored.ifc"),
            v,
        );
        let bytes = write(&a, v);
        let back = bcf::read(&bytes).unwrap();
        assert_eq!(back.topics.len(), 1);
        let (t, u) = (&a.topics[0], &back.topics[0]);
        assert_eq!(u.files, files, "{v:?}: header files");
        assert_eq!(u.guid, t.guid);
        assert_eq!(u.title, t.title);
        assert_eq!(u.description, t.description);
        assert_eq!(u.labels, t.labels);
        assert_eq!(u.status, t.status);
        assert_eq!(u.topic_type, t.topic_type);
        assert_eq!(u.creation_author, t.creation_author);
        assert_eq!(u.creation_date, t.creation_date);
        assert_eq!(u.index, t.index);
        assert_eq!(u.viewpoints.len(), 1);
        let (vp, vq) = (&t.viewpoints[0], &u.viewpoints[0]);
        assert_eq!(vq.guid, vp.guid);
        let (x, y) = (vp.vis.as_ref().unwrap(), vq.vis.as_ref().unwrap());
        assert_eq!(y.components, x.components, "{v:?}: components");
        let (c, d) = (x.camera.as_ref().unwrap(), y.camera.as_ref().unwrap());
        assert_eq!(d.kind, c.kind);
        for k in 0..3 {
            assert!((d.viewpoint[k] - c.viewpoint[k]).abs() < 1e-9);
            assert!((d.direction[k] - c.direction[k]).abs() < 1e-9);
            assert!((d.up[k] - c.up[k]).abs() < 1e-9);
        }
        // The whole topic, field by field (`Topic` is PartialEq); BCF 2.1 has no aspect
        // ratio.
        let mut t = t.clone();
        if v == Version::V2_1
            && let Some(c) = t.viewpoints[0].vis.as_mut().and_then(|x| x.camera.as_mut())
        {
            c.aspect = None;
        }
        assert_eq!(*u, t, "{v:?}: topic");
    }
    let _ = Placement::IDENTITY;
}
