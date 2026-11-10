//! Rhino reader tests: archives written here (version 5+ layout), and the corpus compared
//! with rhino3dm's facts (tools/corpus/fetch-models2.sh, gen/rhino_oracle.py).

use super::archive::*;
use super::objects::{INSTANCE_REF, MESH};
use super::*;

fn le32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

/// A long chunk (8-byte length), with a CRC slot when the type asks for one.
fn chunk(t: u32, body: &[u8]) -> Vec<u8> {
    let mut b = body.to_vec();
    if t & 0x8000 != 0 {
        b.extend_from_slice(&[0; 4]);
    }
    let mut out = le32(t).to_vec();
    out.extend_from_slice(&(b.len() as u64).to_le_bytes());
    out.extend(b);
    out
}

fn short(t: u32, v: i64) -> Vec<u8> {
    let mut out = le32(t).to_vec();
    out.extend_from_slice(&v.to_le_bytes());
    out
}

fn object(class: Uuid, data: &[u8]) -> Vec<u8> {
    let mut body = chunk(TCODE_OPENNURBS_CLASS_UUID, &class.0);
    body.extend(chunk(TCODE_OPENNURBS_CLASS_DATA, data));
    body.extend(short(0x8002_7FFF, 0));
    chunk(TCODE_OPENNURBS_CLASS, &body)
}

/// A zlib stream of stored blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut blocks = data.chunks(65_535).peekable();
    if data.is_empty() {
        out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(b) = blocks.next() {
        out.push(u8::from(blocks.peek().is_none()));
        out.extend_from_slice(&(b.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        out.extend_from_slice(b);
    }
    let (mut a, mut s) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65_521;
        s = (s + a) % 65_521;
    }
    out.extend_from_slice(&((s << 16) | a).to_be_bytes());
    out
}

fn buffer(raw: &[u8], deflate: bool) -> Vec<u8> {
    let mut out = le32(raw.len() as u32).to_vec();
    if raw.is_empty() {
        return out;
    }
    out.extend_from_slice(&[0; 4]);
    if deflate {
        out.push(1);
        out.extend(chunk(TCODE_ANONYMOUS_CHUNK, &zlib_stored(raw)));
    } else {
        out.push(0);
        out.extend_from_slice(raw);
    }
    out
}

fn mesh_data(
    v: &[[f32; 3]],
    f: &[[u32; 4]],
    colours: Option<&[[u8; 4]]>,
    deflate: bool,
) -> Vec<u8> {
    let mut d = vec![0x30];
    d.extend(le32(v.len() as u32));
    d.extend(le32(f.len() as u32));
    d.extend([0u8; 148]);
    d.extend([0u8; 5]);
    d.extend(le32(4));
    for q in f {
        for &i in q {
            d.extend(le32(i));
        }
    }
    let vraw: Vec<u8> = v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect();
    d.extend(buffer(&vraw, deflate));
    d.extend(buffer(&[], false));
    d.extend(buffer(&[], false));
    d.extend(buffer(&[], false));
    let craw: Vec<u8> = colours
        .map(|c| c.iter().flatten().copied().collect())
        .unwrap_or_default();
    d.extend(buffer(&craw, false));
    d
}

/// Version-5 attributes: id, layer, then items (colour by object when `colour` is set).
fn attributes(id: u8, layer: i32, colour: Option<[u8; 4]>, mode: u8) -> Vec<u8> {
    let mut d = vec![0x20];
    d.extend([id; 16]);
    d.extend(layer.to_le_bytes());
    if let Some(c) = colour {
        d.push(6);
        d.extend(c);
    }
    if mode != 0 {
        d.push(12);
        d.push(mode);
    }
    if colour.is_some() {
        d.push(13);
        d.push(1);
    }
    d.push(0);
    d
}

fn record(kind: u32, class: Uuid, data: &[u8], attrs: &[u8]) -> Vec<u8> {
    let mut body = short(TCODE_OBJECT_RECORD_TYPE, i64::from(kind));
    body.extend(object(class, data));
    body.extend(chunk(TCODE_OBJECT_RECORD_ATTRIBUTES, attrs));
    body.extend(short(0x8200_007F, 0));
    chunk(TCODE_OBJECT_RECORD, &body)
}

fn utf16(s: &str) -> Vec<u8> {
    let u: Vec<u16> = s.encode_utf16().chain([0]).collect();
    let mut out = le32(u.len() as u32).to_vec();
    out.extend(u.iter().flat_map(|x| x.to_le_bytes()));
    out
}

fn layer(name: &str, colour: [u8; 4], visible: bool) -> Vec<u8> {
    let mut d = vec![0x11];
    d.extend(0i32.to_le_bytes());
    d.extend(0i32.to_le_bytes());
    d.extend(0i32.to_le_bytes());
    d.extend((-1i32).to_le_bytes());
    d.extend(0i32.to_le_bytes());
    d.extend(colour);
    d.extend([0u8; 20]);
    d.extend(utf16(name));
    d.push(u8::from(visible));
    chunk(
        TCODE_LAYER_RECORD,
        &object(Uuid::parse("95809813-E985-11d3-BFE5-0010830122F0"), &d),
    )
}

fn table(t: u32, records: &[Vec<u8>]) -> Vec<u8> {
    let mut body: Vec<u8> = records.concat();
    body.extend(short(TCODE_ENDOFTABLE, 0));
    chunk(t, &body)
}

fn archive(tables: &[Vec<u8>]) -> Vec<u8> {
    let mut d = b"3D Geometry File Format       50".to_vec();
    d.extend(chunk(1, b"written by the specular tests"));
    let mut props = short(TCODE_PROPERTIES_OPENNURBS_VERSION, 201_001_015);
    let mut app = vec![0x10];
    app.extend(utf16("Specular tests"));
    app.extend(utf16(""));
    app.extend(utf16(""));
    props.extend(chunk(TCODE_PROPERTIES_APPLICATION, &app));
    d.extend(table(TCODE_PROPERTIES_TABLE, &[props]));
    let mut units = 100i32.to_le_bytes().to_vec();
    units.extend(2u32.to_le_bytes());
    units.extend([0u8; 24]);
    d.extend(table(
        TCODE_SETTINGS_TABLE,
        &[chunk(TCODE_SETTINGS_UNITSANDTOLS, &units)],
    ));
    for t in tables {
        d.extend_from_slice(t);
    }
    let size = d.len() as u64 + 16;
    d.extend(chunk(TCODE_ENDOFFILE, &size.to_le_bytes()));
    d
}

const SQUARE: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
];

#[test]
fn meshes_with_colours_layers_and_units() {
    let quad = mesh_data(&SQUARE, &[[0, 1, 2, 3]], None, false);
    let tri = mesh_data(&SQUARE, &[[0, 1, 2, 2]], None, true);
    let bytes = archive(&[
        table(
            TCODE_LAYER_TABLE,
            &[
                layer("Red", [255, 0, 0, 0], true),
                layer("Off", [0, 0, 255, 0], false),
            ],
        ),
        table(
            TCODE_OBJECT_TABLE,
            &[
                // By layer (red), by object (green), and one on a hidden layer.
                record(0x20, MESH, &quad, &attributes(1, 0, None, 0)),
                record(0x20, MESH, &tri, &attributes(2, 0, Some([0, 255, 0, 0]), 0)),
                record(0x20, MESH, &tri, &attributes(3, 1, None, 0)),
            ],
        ),
    ]);
    let m = read(&bytes, &Limits::default()).unwrap();
    assert_eq!(m.triangle_count(), 3);
    assert_eq!(m.unit, Unit::Millimetre);
    assert_eq!(m.up, Up::Z);
    assert_eq!(m.materials.len(), 2);
    assert_eq!(m.materials[0].name, "Red");
    assert_eq!(m.materials[0].base_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(m.materials[1].base_color, [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(m.generator.as_deref(), Some("Specular tests"));
    assert!(
        m.notes.iter().any(|n| n.contains("1 hidden")),
        "{:?}",
        m.notes
    );
    let f = facts(&bytes);
    let get = |k: &str| f.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    assert_eq!(get("Rhino version"), Some("5"));
    assert_eq!(get("Units"), Some("millimetres"));
    assert_eq!(get("Meshes"), Some("3"));
    assert_eq!(get("Layers"), Some("2"));
    assert_eq!(get("Saved by"), Some("Specular tests"));
}

#[test]
fn block_instances_place_their_definition() {
    let tri = mesh_data(&SQUARE, &[[0, 1, 2, 2]], Some(&[[9, 9, 9, 255]; 4]), false);
    let def_id = Uuid([7; 16]);
    let mut idef = vec![0x10];
    idef.extend(def_id.0);
    idef.extend(1i32.to_le_bytes());
    idef.extend([5u8; 16]);
    idef.extend(utf16("Block"));
    let mut iref = vec![0x10];
    iref.extend(def_id.0);
    let x: [f64; 16] = [
        1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    for v in x {
        iref.extend(v.to_le_bytes());
    }
    iref.extend([0u8; 48]);
    let bytes = archive(&[
        table(
            TCODE_INSTANCE_DEFINITION_TABLE,
            &[chunk(
                TCODE_INSTANCE_DEFINITION_RECORD,
                &object(Uuid::parse("26F8BFF6-2618-417f-A158-153D64A94989"), &idef),
            )],
        ),
        table(
            TCODE_OBJECT_TABLE,
            &[
                record(0x20, MESH, &tri, &attributes(5, 0, None, 3)),
                record(0x1000, INSTANCE_REF, &iref, &attributes(6, 0, None, 0)),
                record(0x1000, INSTANCE_REF, &iref, &attributes(8, 0, None, 0)),
            ],
        ),
    ]);
    let m = read(&bytes, &Limits::default()).unwrap();
    // The definition's triangle twice, moved by 10 in x; not drawn where it is defined.
    assert_eq!(m.triangle_count(), 2);
    assert_eq!(m.bounds.min, [10.0, 0.0, 0.0]);
    assert_eq!(m.bounds.max, [11.0, 1.0, 0.0]);
    assert_eq!(m.colors.as_ref().unwrap()[0], [9, 9, 9, 255]);
}

/// A definition that places itself ten times is cut where it repeats, not followed
/// 10^16 times.
#[test]
fn self_placing_blocks_end() {
    let tri = mesh_data(&SQUARE, &[[0, 1, 2, 2]], None, false);
    let def_id = Uuid([7; 16]);
    let mut idef = vec![0x10];
    idef.extend(def_id.0);
    idef.extend(11i32.to_le_bytes());
    idef.extend([5u8; 16]);
    for k in 0..10u8 {
        idef.extend([20 + k; 16]);
    }
    idef.extend(utf16("Loop"));
    let mut iref = vec![0x10];
    iref.extend(def_id.0);
    for v in [
        1.0f64, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ] {
        iref.extend(v.to_le_bytes());
    }
    iref.extend([0u8; 48]);
    let mut records = vec![record(0x20, MESH, &tri, &attributes(5, 0, None, 3))];
    for k in 0..10u8 {
        records.push(record(
            0x1000,
            INSTANCE_REF,
            &iref,
            &attributes(20 + k, 0, None, 3),
        ));
    }
    records.push(record(
        0x1000,
        INSTANCE_REF,
        &iref,
        &attributes(99, 0, None, 0),
    ));
    let bytes = archive(&[
        table(
            TCODE_INSTANCE_DEFINITION_TABLE,
            &[chunk(
                TCODE_INSTANCE_DEFINITION_RECORD,
                &object(Uuid::parse("26F8BFF6-2618-417f-A158-153D64A94989"), &idef),
            )],
        ),
        table(TCODE_OBJECT_TABLE, &records),
    ]);
    let m = read(&bytes, &Limits::default()).unwrap();
    assert_eq!(m.triangle_count(), 1);
    assert!(
        m.notes.iter().any(|n| n.contains("inside themselves")),
        "{:?}",
        m.notes
    );
}

#[test]
fn nurbs_only_models_are_refused_with_the_reason() {
    // A B-rep with no render meshes: eight empty arrays, a box, an empty mesh chunk.
    let mut brep = vec![0x31];
    for _ in 0..7 {
        brep.extend(chunk(TCODE_ANONYMOUS_CHUNK, &[0x10, 0, 0, 0, 0]));
    }
    brep.extend(chunk(TCODE_ANONYMOUS_CHUNK, &[0x10, 1, 0, 0, 0]));
    brep.extend([0u8; 48]);
    brep.extend(chunk(TCODE_ANONYMOUS_CHUNK, &[0]));
    brep.extend(chunk(TCODE_ANONYMOUS_CHUNK, &[0]));
    let bytes = archive(&[table(
        TCODE_OBJECT_TABLE,
        &[record(
            0x10,
            super::objects::BREPS[0],
            &brep,
            &attributes(1, 0, None, 0),
        )],
    )]);
    match read(&bytes, &Limits::default()) {
        Err(Error::Unsupported(m)) => assert_eq!(m, NO_RENDER_MESHES),
        other => panic!("{:?}", other.map(|m| m.triangle_count())),
    }
    let f = facts(&bytes);
    assert!(
        f.contains(&("Render meshes".into(), "not saved".into())),
        "{f:?}"
    );
    // Rhino 1 archives say so.
    match read(b"3D Geometry File Format        1", &Limits::default()) {
        Err(Error::Unsupported(m)) => assert_eq!(m, RHINO1),
        other => panic!("{:?}", other.map(|m| m.triangle_count())),
    }
}

#[test]
fn preview_becomes_a_bmp() {
    let mut img = Vec::new();
    for v in [40i32, 2, 2] {
        img.extend(v.to_le_bytes());
    }
    img.extend(1i16.to_le_bytes());
    img.extend(24i16.to_le_bytes());
    img.extend(0i32.to_le_bytes());
    img.extend(16i32.to_le_bytes());
    img.extend([0u8; 16]);
    let pixels = [10u8; 16];
    img.extend(buffer(&pixels, true));
    let mut d = b"3D Geometry File Format       60".to_vec();
    d.extend(chunk(1, b""));
    d.extend(table(
        TCODE_PROPERTIES_TABLE,
        &[chunk(TCODE_PROPERTIES_COMPRESSED_PREVIEWIMAGE, &img)],
    ));
    let bmp = preview_bmp(&d).expect("a preview");
    assert!(bmp.starts_with(b"BM"));
    assert_eq!(bmp.len(), 14 + 40 + 16);
    assert_eq!(&bmp[54..], &pixels);
    assert!(facts(&d).contains(&("Preview image".into(), "2 × 2".into())));
}

#[test]
fn junk_is_an_error_not_a_panic() {
    let good = archive(&[table(
        TCODE_OBJECT_TABLE,
        &[record(
            0x20,
            MESH,
            &mesh_data(&SQUARE, &[[0, 1, 2, 3]], None, true),
            &attributes(1, 0, None, 0),
        )],
    )]);
    assert!(read(&good, &Limits::default()).is_ok());
    for cut in (0..good.len()).step_by(7) {
        let _ = read(&good[..cut], &Limits::default());
        let _ = facts(&good[..cut]);
        let _ = preview_bmp(&good[..cut]);
    }
    let mut flipped = good.clone();
    for i in (32..flipped.len()).step_by(13) {
        flipped[i] ^= 0xa5;
        let _ = read(&flipped, &Limits::default());
        let _ = facts(&flipped);
    }
    // Counts over the caps are refused before allocating.
    let tight = Limits {
        max_vertices: 3,
        ..Limits::default()
    };
    assert!(read(&good, &tight).is_err());
}

// ------------------------------------------------------------------------------- corpus

fn corpus() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("SPECULAR_MODELS2_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../tools/corpus/models2"
            ))
        })
        .join("rhino");
    if dir.join("oracle.tsv").is_file() {
        Some(dir)
    } else {
        eprintln!("skipped: no Rhino corpus at {}", dir.display());
        None
    }
}

/// Every corpus file against rhino3dm: triangles (faces, quads split), vertices and bounds
/// of what is drawn, within 1e-4 of the model's size; files rhino3dm draws nothing from are
/// refused with a reason.
#[test]
fn corpus_matches_rhino3dm() {
    let Some(dir) = corpus() else { return };
    let tsv = std::fs::read_to_string(dir.join("oracle.tsv")).unwrap();
    let mut rows: std::collections::BTreeMap<String, HashMap<String, String>> = Default::default();
    for l in tsv.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() == 3 {
            rows.entry(c[0].to_owned())
                .or_default()
                .insert(c[1].to_owned(), c[2].to_owned());
        }
    }
    let (mut files, mut mismatches) = (0, Vec::new());
    let mut slowest: Option<(String, f64)> = None;
    let t0 = std::time::Instant::now();
    for (rel, facts_) in &rows {
        let bytes = std::fs::read(dir.join(rel)).unwrap();
        files += 1;
        let tris: usize = facts_.get("tris").and_then(|v| v.parse().ok()).unwrap_or(0);
        let t = std::time::Instant::now();
        let got = read(&bytes, &Limits::default());
        let ms = t.elapsed().as_secs_f64() * 1e3;
        if slowest.as_ref().is_none_or(|(_, s)| ms > *s) {
            slowest = Some((rel.clone(), ms));
        }
        // The reasons given for some refusals.
        let expect = match rel.as_str() {
            "gen/nurbs_only_v8.3dm" | "opennurbs/v5_teapot.3dm" => Some(NO_RENDER_MESHES),
            "rhino3dm/subd.3dm" | "opennurbs/v7_rhino_logo_subd.3dm" => Some(SUBD_ONLY),
            _ => None,
        };
        if let Some(want) = expect
            && !matches!(&got, Err(Error::Unsupported(m)) if m == want)
        {
            mismatches.push(format!("{rel}: not refused with “{want}”"));
        }
        if super::archive::header(&bytes).is_some_and(|h| h.raw == 1) {
            // Rhino 1 archives: refused (rhino3dm reads their legacy records).
            if !matches!(&got, Err(Error::Unsupported(m)) if m == RHINO1) {
                mismatches.push(format!("{rel}: a Rhino 1 file not refused"));
            }
            continue;
        }
        if tris == 0 {
            if let Ok(m) = &got {
                mismatches.push(format!(
                    "{rel}: rhino3dm draws nothing, we {}",
                    m.triangle_count()
                ));
            }
            continue;
        }
        let m = match got {
            Ok(m) => m,
            Err(e) => {
                mismatches.push(format!("{rel}: {e}"));
                continue;
            }
        };
        if m.triangle_count() != tris {
            mismatches.push(format!(
                "{rel}: {} triangles, rhino3dm {tris}",
                m.triangle_count()
            ));
        }
        let parse = |k: &str| -> Option<[f32; 3]> {
            let v: Vec<f32> = facts_
                .get(k)?
                .split(',')
                .filter_map(|x| x.parse().ok())
                .collect();
            (v.len() == 3).then(|| [v[0], v[1], v[2]])
        };
        if let (Some(lo), Some(hi)) = (parse("min"), parse("max")) {
            let size = (0..3).map(|c| hi[c] - lo[c]).fold(0f32, f32::max).max(1e-6);
            let tol = size * 1e-4;
            let off = (0..3)
                .map(|c| {
                    (m.bounds.min[c] - lo[c])
                        .abs()
                        .max((m.bounds.max[c] - hi[c]).abs())
                })
                .fold(0f32, f32::max);
            if off > tol {
                mismatches.push(format!(
                    "{rel}: bounds {:?}–{:?}, rhino3dm {lo:?}–{hi:?}",
                    m.bounds.min, m.bounds.max
                ));
            }
        }
    }
    eprintln!(
        "rhino corpus: {files} files, {} mismatches, {:.0} ms (slowest {slowest:?})",
        mismatches.len(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// Facts and previews of real files: every file whose properties carry a preview gives a
/// well-formed BMP; facts name the version, the application and the counts.
#[test]
fn corpus_facts_and_previews() {
    let Some(dir) = corpus() else { return };
    let (mut previews, mut files) = (0, 0);
    let mut paths = Vec::new();
    for sub in ["opennurbs", "rhino3dm", "gen"] {
        if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
            paths.extend(rd.flatten().map(|e| e.path()));
        }
    }
    paths.sort();
    for p in paths {
        let bytes = std::fs::read(&p).unwrap();
        files += 1;
        let f = facts(&bytes);
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(f.iter().any(|(k, _)| k == "Rhino version"), "{name}");
        if std::env::var_os("SPECULAR_RHINO_FACTS").is_some() {
            eprintln!("{name}: {f:?}");
            if let Ok(m) = read(&bytes, &Limits::default()) {
                let mats: Vec<_> = m
                    .materials
                    .iter()
                    .map(|x| (&x.name, x.base_color))
                    .collect();
                eprintln!("  materials {mats:?}\n  notes {:?}", m.notes);
            }
        }
        if let Some(bmp) = preview_bmp(&bytes) {
            previews += 1;
            let u32_at = |o: usize| u32::from_le_bytes(bmp[o..o + 4].try_into().unwrap());
            assert_eq!(u32_at(2) as usize, bmp.len(), "{name}");
            let (w, h) = (u32_at(18) as i32, u32_at(22) as i32);
            assert!(w > 0 && h != 0, "{name}");
            assert!(f.iter().any(|(k, _)| k == "Preview image"), "{name}");
        }
    }
    eprintln!("rhino corpus: {files} files, {previews} previews");
    assert!(previews > 10, "{previews}");
    let get = |file: &str, key: &str| {
        let f = facts(&std::fs::read(dir.join(file)).unwrap());
        f.into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
    };
    let ex = "opennurbs/v4_example_file.3dm";
    assert_eq!(get(ex, "Block instances").as_deref(), Some("3"));
    assert_eq!(get(ex, "Block definitions").as_deref(), Some("2"));
    let logo = "opennurbs/v7_rhino_logo_nurbs.3dm";
    assert_eq!(get(logo, "Render meshes").as_deref(), Some("saved"));
    let teapot = "opennurbs/v5_teapot.3dm";
    assert_eq!(get(teapot, "Render meshes").as_deref(), Some("not saved"));
    let mesh = "opennurbs/v6_my_mesh.3dm";
    assert_eq!(get(mesh, "Units").as_deref(), Some("millimetres"));
}
