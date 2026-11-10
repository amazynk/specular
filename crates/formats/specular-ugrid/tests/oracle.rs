//! The face under a point and the value there, against xugrid (its cell tree and
//! barycentric weights) and netCDF4, through the rows `gen/oracle.py` wrote to
//! tests/data/expected.tsv; and the corpus's expected.tsv when SPECULAR_UGRID_DIR names a
//! directory with one (tools/corpus/fetch-ugrid.sh).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use specular_netcdf::Netcdf;
use specular_ugrid::{
    DataVar, Display, Location, Locator, Mesh, Topology, detect, read_values, variables,
};

struct Opened {
    nc: Netcdf,
    data: Vec<u8>,
    display: Display,
    locator: Locator,
    vars: Vec<DataVar>,
    values: HashMap<(String, String), Vec<f32>>,
    topo: Topology,
}

fn open(path: &Path) -> Opened {
    let data = std::fs::read(path).unwrap();
    let nc = Netcdf::open(&data, &|| false).unwrap();
    let topo = detect(&nc).unwrap_or_else(|| panic!("{}: no mesh", path.display()));
    let mesh = Mesh::load(&nc, &data, &topo, &|| false).unwrap();
    let display = Display::build(&mesh);
    let locator = Locator::build(&display);
    let vars = variables(&nc, &topo);
    Opened {
        nc,
        data,
        display,
        locator,
        vars,
        values: HashMap::new(),
        topo,
    }
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-5 * (1.0 + b.abs())
}

fn locate(o: &Opened, x: f64, y: f64) -> Option<specular_ugrid::Hit> {
    let (dx, dy) = if o.display.geographic {
        (o.display.west + (x - o.display.west).rem_euclid(360.0), y)
    } else {
        (x, y)
    };
    o.locator.locate(&o.display, dx, dy)
}

fn run(dir: &Path, strict: bool) -> (BTreeMap<String, (usize, usize)>, Vec<String>) {
    let text = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut files: HashMap<String, Opened> = HashMap::new();
    let mut tally: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut notes = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let (file, name, index) = (f[0], f[1], f[2]);
        let x: f64 = f[3].parse().unwrap();
        let y: f64 = f[4].parse().unwrap();
        let face: i64 = f[5].parse().unwrap();
        let o = files
            .entry(file.to_owned())
            .or_insert_with(|| open(&dir.join(file)));
        if name == "-" {
            // The face alone.
            let hit = locate(o, x, y);
            let got = hit.map_or(-1, |h| i64::from(h.face));
            let t = tally.entry(format!("{file} faces")).or_default();
            t.0 += 1;
            if got != face {
                t.1 += 1;
                notes.push(format!("{file} at {x} {y}: face {got}, xugrid {face}"));
            }
            continue;
        }
        let Some(dv) = o
            .vars
            .iter()
            .find(|v| o.nc.vars[v.var].name == name)
            .cloned()
        else {
            // The oracle samples every numeric variable on the face or node dimension;
            // coordinates and structural arrays are not pages here. Not a mismatch, only
            // counted when the fixtures (whose pages are known) disagree.
            if strict {
                let t = tally.entry(format!("{file} variables")).or_default();
                t.0 += 1;
                t.1 += 1;
                notes.push(format!("{file}: {name} is not a page"));
            }
            continue;
        };
        let idx: Vec<u32> = if index == "-" {
            Vec::new()
        } else {
            index.split(',').map(|s| s.parse().unwrap()).collect()
        };
        let key = (name.to_owned(), index.to_owned());
        if !o.values.contains_key(&key) {
            let (v, _) = read_values(&o.nc, &o.data, &dv, &idx, &|| false).unwrap();
            o.values.insert(key.clone(), v);
        }
        let values = &o.values[&key];
        let hit = locate(o, x, y);
        let got_face = hit.map_or(-1, |h| i64::from(h.face));
        let t = tally.entry(format!("{file} faces")).or_default();
        t.0 += 1;
        if got_face != face {
            t.1 += 1;
            notes.push(format!(
                "{file} {name} at {x} {y}: face {got_face}, xugrid {face}"
            ));
            continue;
        }
        if f[6] == "-" {
            continue;
        }
        let want: f64 = if f[6] == "nan" {
            f64::NAN
        } else {
            f[6].parse().unwrap()
        };
        let got = match (hit, dv.location) {
            (None, _) => f64::NAN,
            (Some(h), Location::Face) => f64::from(values[h.face as usize]),
            (Some(h), Location::Node) => (0..3)
                .map(|k| h.weights[k] * f64::from(values[h.nodes[k] as usize]))
                .sum(),
        };
        let t = tally.entry(format!("{file} values")).or_default();
        t.0 += 1;
        if !same(got, want) {
            t.1 += 1;
            notes.push(format!(
                "{file} {name}[{index}] at {x} {y}: {got}, expected {want}"
            ));
        }
    }
    for (name, o) in &files {
        eprintln!(
            "{name}: {} ({:?}), {} faces drawn as {} triangles, {} split, {} pages",
            o.topo.name,
            o.topo.convention,
            o.topo.faces,
            o.display.tris.len(),
            o.display.split_faces,
            o.vars.len()
        );
    }
    (tally, notes)
}

fn report(tally: &BTreeMap<String, (usize, usize)>, notes: &[String]) -> usize {
    let mut bad = 0;
    for (k, (n, b)) in tally {
        eprintln!("{k}: {n} checked, {b} mismatches");
        bad += b;
    }
    for n in notes.iter().take(30) {
        eprintln!("  {n}");
    }
    bad
}

#[test]
fn fixtures_against_xugrid() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let (tally, notes) = run(&dir, true);
    let checked: usize = tally.values().map(|t| t.0).sum();
    assert!(checked > 1000, "{checked}");
    assert_eq!(report(&tally, &notes), 0);
}

#[test]
fn corpus_against_xugrid() {
    let Some(dir) = std::env::var_os("SPECULAR_UGRID_DIR") else {
        return;
    };
    let dir = Path::new(&dir);
    if !dir.join("expected.tsv").exists() {
        return;
    }
    let (tally, notes) = run(dir, false);
    assert_eq!(report(&tally, &notes), 0);
}

#[test]
fn fixture_structure() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let o = open(&dir.join("mixed_fill_start1.nc"));
    let mesh = Mesh::load(&o.nc, &o.data, &o.topo, &|| false).unwrap();
    assert_eq!(mesh.start_index, 1);
    assert_eq!(mesh.kinds.polygons, 1);
    assert!(mesh.kinds.triangles > 0 && mesh.kinds.quads > 0);
    assert_eq!(mesh.kinds.max_nodes, 5);
    let names: Vec<&str> = o
        .vars
        .iter()
        .map(|v| o.nc.vars[v.var].name.as_str())
        .collect();
    assert_eq!(names, ["mesh2d_s1", "mesh2d_sa1"]);
    // FVCOM: nv is nodes-by-faces and 1-based without start_index.
    let o = open(&dir.join("fvcom_like.nc"));
    let mesh = Mesh::load(&o.nc, &o.data, &o.topo, &|| false).unwrap();
    assert_eq!(mesh.start_index, 1);
    assert_eq!(mesh.kinds.triangles, 40);
    let names: Vec<&str> = o
        .vars
        .iter()
        .map(|v| o.nc.vars[v.var].name.as_str())
        .collect();
    assert_eq!(names, ["zeta", "u"]);
    // ICON from bounds: the shared vertices found again.
    let a = open(&dir.join("icon_grid.nc"));
    let b = open(&dir.join("icon_data.nc"));
    let ma = Mesh::load(&a.nc, &a.data, &a.topo, &|| false).unwrap();
    let mb = Mesh::load(&b.nc, &b.data, &b.topo, &|| false).unwrap();
    assert_eq!(ma.nodes.len(), mb.nodes.len());
    assert_eq!(ma.faces(), 320);
    assert!(a.display.split_faces > 0);
}

#[test]
fn hostile_connectivity_is_refused() {
    // Point a face past the last node: the load fails cleanly.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut data = std::fs::read(dir.join("tri_start0.nc")).unwrap();
    let nc = Netcdf::open(&data, &|| false).unwrap();
    let topo = detect(&nc).unwrap();
    let conn = nc.var("Mesh2_face_nodes").unwrap();
    // Classic layout: the variable's data begins after the header; find the first
    // triangle (0, 1, 7) as big-endian i32s and replace a node with 9999.
    let pat: Vec<u8> = [0i32, 1, 7].iter().flat_map(|v| v.to_be_bytes()).collect();
    let at = data
        .windows(12)
        .position(|w| w == pat.as_slice())
        .expect("first triangle");
    data[at + 8..at + 12].copy_from_slice(&9999i32.to_be_bytes());
    assert!(conn.shape[1] == 3);
    let nc = Netcdf::open(&data, &|| false).unwrap();
    assert!(Mesh::load(&nc, &data, &topo, &|| false).is_err());
    // Truncated files: no panic.
    for cut in [100, 400, 1000, data.len() - 50] {
        let d = &data[..cut];
        if let Ok(nc) = Netcdf::open(d, &|| false)
            && let Some(t) = detect(&nc)
        {
            let _ = Mesh::load(&nc, d, &t, &|| false);
        }
    }
}
