//! BCF round trips: a made-up archive through both versions, and the official buildingSMART
//! test cases (fetched by `tools/corpus/fetch-bcf.sh`; skipped when absent) read, written
//! and read again. What is written is checked by the crate's own schema rules and, when the
//! XSDs were fetched and `xmllint` is installed, by the XSDs themselves.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use specular_bim::bcf::*;
use specular_bim::check::{Kind, check, check_members};
use specular_bim::write::{members, write};

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_BCF_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/bcf"),
        PathBuf::from,
    )
}

/// A one-pixel PNG (bytes only matter as bytes here).
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

fn made_up() -> Archive {
    let vis = VisInfo {
        guid: "a1b2c3d4-0000-4000-8000-000000000001".into(),
        components: Some(Components {
            selection: vec![Component::of("2O2Fr$t4X7Zf8NOew3FLOH")],
            default_visibility: true,
            exceptions: vec![Component {
                ifc_guid: Some("1hOSvn6df7F8_7GcBWlRGQ".into()),
                originating_system: Some("Specular".into()),
                authoring_tool_id: Some("42".into()),
            }],
            hints: Some(ViewSetupHints {
                spaces_visible: Some(false),
                space_boundaries_visible: None,
                openings_visible: Some(true),
            }),
            coloring: vec![Coloring {
                color: "FF00AA33".into(),
                components: vec![Component::of("3cUkl32yn9qRSPvBJVyWYp")],
            }],
        }),
        camera: Some(BcfCamera {
            kind: CameraKind::Perspective { fov: 55.0 },
            viewpoint: [12.5, -3.25, 1.6],
            direction: [0.0, 1.0, 0.0],
            up: [0.0, 0.0, 1.0],
            aspect: Some(1.6),
        }),
        lines: vec![([0.0; 3], [1.0, 2.0, 3.0])],
        clipping_planes: vec![ClippingPlane {
            location: [0.0, 0.0, 2.5],
            direction: [0.0, 0.0, 1.0],
        }],
        bitmaps: Vec::new(),
    };
    let ortho = VisInfo {
        guid: "a1b2c3d4-0000-4000-8000-000000000002".into(),
        camera: Some(BcfCamera {
            kind: CameraKind::Orthogonal { scale: 12.0 },
            viewpoint: [0.0, 0.0, 50.0],
            direction: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            aspect: Some(1.0),
        }),
        ..VisInfo::default()
    };
    let topic = Topic {
        guid: "a1b2c3d4-0000-4000-8000-0000000000aa".into(),
        topic_type: Some("Clash".into()),
        status: Some("Open".into()),
        title: "Duct through the beam & <slab>".into(),
        priority: Some("High".into()),
        index: Some(1),
        labels: vec!["Structural".into(), "MEP".into()],
        creation_date: "2026-10-25T09:30:00Z".into(),
        creation_author: "marc@example.invalid".into(),
        modified_date: Some("2026-10-25T10:00:00Z".into()),
        modified_author: Some("marc@example.invalid".into()),
        due_date: Some("2026-11-01T00:00:00Z".into()),
        assigned_to: Some("mep@example.invalid".into()),
        stage: Some("Design".into()),
        description: Some("Line one\nline two".into()),
        reference_links: vec!["https://example.invalid/issue/1".into()],
        related: vec!["a1b2c3d4-0000-4000-8000-0000000000bb".into()],
        files: vec![HeaderFile {
            ifc_project: Some("0M6o7Znnv7hxsbWgeu7oQq".into()),
            ifc_spatial: None,
            is_external: true,
            filename: Some("house.ifc".into()),
            date: Some("2026-10-24T08:00:00Z".into()),
            reference: Some("house.ifc".into()),
        }],
        comments: vec![
            Comment {
                guid: "a1b2c3d4-0000-4000-8000-0000000000c1".into(),
                date: "2026-10-25T09:31:00Z".into(),
                author: "marc@example.invalid".into(),
                text: "Move the duct 200 mm down.".into(),
                viewpoint: Some(vis.guid.clone()),
                modified_date: None,
                modified_author: None,
            },
            Comment {
                guid: "a1b2c3d4-0000-4000-8000-0000000000c2".into(),
                date: "2026-10-25T09:40:00Z".into(),
                author: "mep@example.invalid".into(),
                text: "שלום — done.".into(),
                viewpoint: None,
                modified_date: Some("2026-10-25T09:41:00Z".into()),
                modified_author: Some("mep@example.invalid".into()),
            },
        ],
        viewpoints: vec![
            Viewpoint {
                guid: vis.guid.clone(),
                file: Some("viewpoint.bcfv".into()),
                snapshot_name: Some("snapshot.png".into()),
                snapshot: Some(PNG.to_vec()),
                index: Some(0),
                vis: Some(vis),
            },
            Viewpoint {
                guid: ortho.guid.clone(),
                file: Some(format!("{}.bcfv", ortho.guid)),
                snapshot_name: None,
                snapshot: None,
                index: Some(1),
                vis: Some(ortho),
            },
        ],
        ..Topic::default()
    };
    let second = Topic {
        guid: "a1b2c3d4-0000-4000-8000-0000000000bb".into(),
        topic_type: Some("Issue".into()),
        status: Some("Closed".into()),
        title: "Minimum".into(),
        index: Some(2),
        creation_date: "2026-10-25T11:00:00Z".into(),
        creation_author: "marc@example.invalid".into(),
        ..Topic::default()
    };
    Archive {
        version: Version::V3_0,
        version_id: "3.0".into(),
        project: Some(Project {
            id: "a1b2c3d4-0000-4000-8000-0000000000ff".into(),
            name: Some("House".into()),
        }),
        topics: vec![topic, second],
        ..Archive::default()
    }
}

/// Only what a version can carry differs between the written and the re-read archive.
fn comparable(mut a: Archive, v: Version) -> Archive {
    if v == Version::V2_1 {
        for vi in a
            .topics
            .iter_mut()
            .flat_map(|t| t.viewpoints.iter_mut())
            .filter_map(|vp| vp.vis.as_mut())
        {
            if let Some(c) = vi.camera.as_mut() {
                c.aspect = None;
            }
        }
    }
    a.version = Version::V2_1;
    a.version_id.clear();
    a.extensions = Extensions::default();
    a.notes.clear();
    for t in &mut a.topics {
        t.server_id = None;
    }
    a
}

#[test]
fn a_made_up_archive_round_trips_through_both_versions() {
    let a = made_up();
    for v in [Version::V2_1, Version::V3_0] {
        let m = members(&a, v);
        let problems = check_members(&m, v);
        assert!(problems.is_empty(), "{v:?}: {problems:#?}");
        let back = read(&write(&a, v)).expect("reads back");
        assert_eq!(back.version, v);
        assert_eq!(comparable(back, v), comparable(a.clone(), v), "{v:?}");
    }
}

#[test]
fn version_rules_are_applied_when_writing() {
    let mut a = made_up();
    // 2.1 allows 45°–60° only; 3.0 wants a camera in every viewpoint.
    if let Some(CameraKind::Perspective { fov }) = a.topics[0].viewpoints[0]
        .vis
        .as_mut()
        .and_then(|v| v.camera.as_mut())
        .map(|c| &mut c.kind)
    {
        *fov = 35.0;
    }
    a.topics[1].viewpoints.push(Viewpoint {
        guid: "a1b2c3d4-0000-4000-8000-0000000000dd".into(),
        snapshot_name: Some("snapshot.png".into()),
        snapshot: Some(PNG.to_vec()),
        vis: Some(VisInfo {
            guid: "a1b2c3d4-0000-4000-8000-0000000000dd".into(),
            ..VisInfo::default()
        }),
        ..Viewpoint::default()
    });
    a.topics[1].topic_type = None;
    a.topics[1].status = None;
    for v in [Version::V2_1, Version::V3_0] {
        let m = members(&a, v);
        assert!(
            check_members(&m, v).is_empty(),
            "{v:?}: {:#?}",
            check_members(&m, v)
        );
    }
    let back = read(&write(&a, Version::V2_1)).unwrap();
    let cam = back.topics[0].viewpoints[0]
        .vis
        .as_ref()
        .unwrap()
        .camera
        .unwrap();
    assert_eq!(cam.kind, CameraKind::Perspective { fov: 45.0 });
    let back = read(&write(&a, Version::V3_0)).unwrap();
    assert_eq!(back.topics[1].topic_type.as_deref(), Some("Issue"));
    assert_eq!(back.topics[1].status.as_deref(), Some("Open"));
    let vp = &back.topics[1].viewpoints[0];
    assert!(
        vp.vis.is_none() && vp.snapshot.is_some(),
        "kept as its snapshot"
    );
}

#[test]
fn the_checker_catches_schema_breaks() {
    let bad = r#"<Markup><Topic Guid="NOT-A-GUID" TopicType="x" TopicStatus="y">
        <CreationDate>2026-10-25</CreationDate><Title>t</Title><CreationAuthor>a</CreationAuthor>
        <Bogus/></Topic></Markup>"#;
    let e = check(bad, Kind::Markup, Version::V3_0);
    assert!(e.iter().any(|m| m.contains("bad Guid")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("out of place")), "{e:?}");
    let bad_fov = r#"<VisualizationInfo Guid="a1b2c3d4-0000-4000-8000-000000000001"><PerspectiveCamera>
        <CameraViewPoint><X>0</X><Y>0</Y><Z>0</Z></CameraViewPoint>
        <CameraDirection><X>0</X><Y>1</Y><Z>0</Z></CameraDirection>
        <CameraUpVector><X>0</X><Y>0</Y><Z>1</Z></CameraUpVector>
        <FieldOfView>35</FieldOfView></PerspectiveCamera></VisualizationInfo>"#;
    assert!(!check(bad_fov, Kind::VisInfo, Version::V2_1).is_empty());
    let missing = r#"<VisualizationInfo Guid="a1b2c3d4-0000-4000-8000-000000000001"/>"#;
    assert!(
        !check(missing, Kind::VisInfo, Version::V3_0).is_empty(),
        "3.0 needs a camera"
    );
    assert!(check(missing, Kind::VisInfo, Version::V2_1).is_empty());
}

/// The official files, by version.
fn cases() -> Vec<(Version, PathBuf)> {
    let mut v = Vec::new();
    for (ver, dir) in [(Version::V2_1, "2.1"), (Version::V3_0, "3.0")] {
        let Ok(rd) = std::fs::read_dir(corpus().join("cases").join(dir)) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        v.extend(files.into_iter().map(|p| (ver, p)));
    }
    v
}

#[test]
fn official_test_cases_round_trip() {
    let cases = cases();
    if cases.is_empty() {
        eprintln!("no BCF corpus (tools/corpus/fetch-bcf.sh): skipped");
        return;
    }
    let mut topics = 0;
    let mut viewpoints = 0;
    let mut mismatches = 0;
    let mut problems_in_source = 0;
    for (ver, path) in &cases {
        let bytes = std::fs::read(path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let a = read(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(a.version, *ver, "{name}");
        assert!(!a.topics.is_empty(), "{name}: no topics");
        topics += a.topics.len();
        viewpoints += a.topics.iter().map(|t| t.viewpoints.len()).sum::<usize>();
        // The source as our rules see it (informational: some cases are older than their
        // schema).
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).unwrap();
            let Some(kind) = Kind::of(&f.name().unwrap()) else {
                continue;
            };
            let mut s = String::new();
            use std::io::Read;
            if f.read_to_string(&mut s).is_ok() {
                let e = check(&s, kind, *ver);
                if !e.is_empty() {
                    eprintln!("source {name}/{}: {e:?}", f.name().unwrap());
                }
                problems_in_source += e.len();
            }
        }
        for out in [Version::V2_1, Version::V3_0] {
            let m = members(&a, out);
            let p = check_members(&m, out);
            assert!(p.is_empty(), "{name} as {out:?}: {p:#?}");
            let back = read(&write(&a, out)).unwrap();
            if out == *ver && comparable(back.clone(), out) != comparable(a.clone(), out) {
                mismatches += 1;
                let (x, y) = (comparable(back.clone(), out), comparable(a.clone(), out));
                for (tx, ty) in x.topics.iter().zip(&y.topics) {
                    if tx != ty {
                        let (dx, dy) = (format!("{tx:?}"), format!("{ty:?}"));
                        let at = dx
                            .bytes()
                            .zip(dy.bytes())
                            .position(|(p, q)| p != q)
                            .unwrap_or(0);
                        eprintln!(
                            "{name}: round trip differs at …{}… vs …{}…",
                            &dx[at.saturating_sub(80)..(at + 80).min(dx.len())],
                            &dy[at.saturating_sub(80)..(at + 80).min(dy.len())]
                        );
                    }
                }
            }
            assert_eq!(back.topics.len(), a.topics.len(), "{name} as {out:?}");
        }
    }
    eprintln!(
        "BCF corpus: {} files, {topics} topics, {viewpoints} viewpoints, {mismatches} round-trip \
         mismatches, {problems_in_source} schema notes on the sources",
        cases.len()
    );
    assert_eq!(mismatches, 0);
}

/// What Specular writes, validated by xmllint against the official XSDs.
#[test]
fn written_archives_validate_against_the_xsds() {
    let schemas = corpus().join("schemas");
    if !schemas.join("2.1/markup.xsd").exists()
        || std::process::Command::new("xmllint")
            .arg("--version")
            .output()
            .is_err()
    {
        eprintln!("no XSDs or no xmllint: skipped");
        return;
    }
    let mut archives = vec![made_up()];
    for (_, p) in cases() {
        archives.push(read(&std::fs::read(p).unwrap()).unwrap());
    }
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bcf-xsd");
    let mut checked = 0;
    let mut failed = Vec::new();
    for (ai, a) in archives.iter().enumerate() {
        for (v, dir) in [(Version::V2_1, "2.1"), (Version::V3_0, "3.0")] {
            for (name, bytes) in members(a, v) {
                let xsd = match Kind::of(&name) {
                    Some(Kind::Markup) => "markup.xsd",
                    Some(Kind::VisInfo) => "visinfo.xsd",
                    Some(Kind::Version) => "version.xsd",
                    Some(Kind::Project) => "project.xsd",
                    Some(Kind::Extensions) if v == Version::V3_0 => "extensions.xsd",
                    _ => continue,
                };
                let f = out.join(format!("{ai}-{dir}")).join(&name);
                std::fs::create_dir_all(f.parent().unwrap()).unwrap();
                std::fs::write(&f, &bytes).unwrap();
                let r = std::process::Command::new("xmllint")
                    .args(["--noout", "--nonet", "--schema"])
                    .arg(schemas.join(dir).join(xsd))
                    .arg(&f)
                    .output()
                    .unwrap();
                checked += 1;
                if !r.status.success() {
                    failed.push(format!(
                        "{ai} {dir} {name}: {}",
                        String::from_utf8_lossy(&r.stderr)
                            .lines()
                            .next()
                            .unwrap_or("")
                    ));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&out);
    eprintln!("xmllint: {checked} files, {} invalid", failed.len());
    assert!(failed.is_empty(), "{failed:#?}");
}

/// The fetched files are the pinned ones.
#[test]
fn corpus_files_match_their_pins() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/fetch-bcf.sh");
    let text = std::fs::read_to_string(script).unwrap();
    let mut n = 0;
    for line in text.lines().filter(|l| l.matches('\t').count() == 3) {
        let f: Vec<&str> = line.split('\t').collect();
        let p = corpus().join(f[3]);
        let Ok(b) = std::fs::read(&p) else { continue };
        let h: String = Sha256::digest(&b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect();
        assert_eq!(h, f[0], "{}", f[3]);
        n += 1;
    }
    eprintln!("{n} pinned BCF files present");
}
