use std::collections::HashMap;

use specular_mesh::Limits;

use super::*;
use crate::math::V3;

/// Files from memory.
#[derive(Default)]
pub(crate) struct Mem(pub HashMap<String, Vec<u8>>);

impl Fetch for Mem {
    fn get(&self, key: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        let b = self.0.get(key).ok_or_else(|| format!("{key} is missing"))?;
        if b.len() > max {
            return Err("too large".into());
        }
        Ok(b.clone())
    }
}

impl Mem {
    fn put(&mut self, k: &str, v: impl Into<Vec<u8>>) {
        self.0.insert(k.to_owned(), v.into());
    }
}

/// A GLB of one triangle (y up: it stands in the x-y plane of glTF).
pub(crate) fn glb_triangle(size: f32) -> Vec<u8> {
    let pos: [f32; 9] = [0.0, 0.0, 0.0, size, 0.0, 0.0, 0.0, size, 0.0];
    let mut bin: Vec<u8> = pos.iter().flat_map(|v| v.to_le_bytes()).collect();
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
        "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}}}}]}}],
        "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[{size},{size},0]}}],
        "bufferViews":[{{"buffer":0,"byteLength":36}}],"buffers":[{{"byteLength":36}}]}}"#
    );
    let mut j = json.into_bytes();
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + j.len() + 8 + bin.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&j);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

fn pad(mut v: Vec<u8>, to: usize) -> Vec<u8> {
    while !v.len().is_multiple_of(to) {
        v.push(b' ');
    }
    v
}

/// A tile file with `magic`, a feature table and a body (`extra`: i3dm's gltfFormat).
fn tile(magic: &[u8; 4], ft_json: &str, ft_bin: &[u8], body: &[u8], extra: Option<u32>) -> Vec<u8> {
    let header = if extra.is_some() { 32 } else { 28 };
    let j = pad(ft_json.as_bytes().to_vec(), 8);
    let mut b = ft_bin.to_vec();
    while !b.len().is_multiple_of(8) {
        b.push(0);
    }
    let total = header + j.len() + b.len() + body.len();
    let mut out = Vec::new();
    out.extend_from_slice(magic);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    for v in [j.len() as u32, b.len() as u32, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    if let Some(e) = extra {
        out.extend_from_slice(&e.to_le_bytes());
    }
    out.extend_from_slice(&j);
    out.extend_from_slice(&b);
    out.extend_from_slice(body);
    out
}

fn b3dm(rtc: Option<V3>) -> Vec<u8> {
    let ft = match rtc {
        Some(c) => format!(
            r#"{{"BATCH_LENGTH":0,"RTC_CENTER":[{},{},{}]}}"#,
            c[0], c[1], c[2]
        ),
        None => r#"{"BATCH_LENGTH":0}"#.into(),
    };
    tile(b"b3dm", &ft, &[], &glb_triangle(1.0), None)
}

fn view_from(eye: V3, target: V3, h: f64) -> View {
    let dir = math::unit(math::sub(target, eye));
    let up = if dir[2].abs() > 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let right = math::cross(dir, up);
    View {
        eye,
        dir,
        up: math::unit(math::cross(right, dir)),
        fov_y: 60f64.to_radians(),
        ortho_height: None,
        width_px: h,
        height_px: h,
        max_sse: MAX_SSE,
        cull: true,
    }
}

fn quad_tileset(refine: &str) -> String {
    let child = |x: f64, y: f64, n: u32| {
        format!(
            r#"{{"boundingVolume":{{"box":[{x},{y},0,5,0,0,0,5,0,0,0,1]}},"geometricError":0,"content":{{"uri":"c{n}.b3dm"}}}}"#
        )
    };
    format!(
        r#"{{"asset":{{"version":"1.0"}},"geometricError":200,"root":{{"boundingVolume":{{"box":[0,0,0,10,0,0,0,10,0,0,0,1]}},"geometricError":100,"refine":"{refine}","content":{{"uri":"root.b3dm"}},"children":[{},{},{},{}]}}}}"#,
        child(-5.0, -5.0, 0),
        child(5.0, -5.0, 1),
        child(-5.0, 5.0, 2),
        child(5.0, 5.0, 3)
    )
}

#[test]
fn sniffs_tilesets() {
    assert!(sniff(quad_tileset("REPLACE").as_bytes()));
    assert!(!sniff(br#"{"type":"FeatureCollection"}"#));
    assert!(!sniff(
        br#"{"stac_version":"1.0","asset":1,"geometricError":1,"root":1}"#
    ));
}

#[test]
fn replace_and_add_refinement() {
    for (refine, near) in [("REPLACE", vec![1, 2, 3, 4]), ("ADD", vec![0, 1, 2, 3, 4])] {
        let mut m = Mem::default();
        m.put("tileset.json", quad_tileset(refine));
        let mut ts = Tileset::open("tileset.json", &m).unwrap();
        assert_eq!(ts.tiles.len(), 5);
        // Far away: the root's error is under 16 px.
        let far = view_from([0.0, -1.0e5, 1.0e5], [0.0; 3], 1000.0);
        assert_eq!(ts.select(&far, &m, 100).tiles, vec![0]);
        // Close: refined.
        let close = view_from([0.0, -30.0, 30.0], [0.0; 3], 1000.0);
        assert_eq!(ts.select(&close, &m, 100).tiles, near, "{refine}");
        // Looking away from two children culls them.
        let side = view_from([-5.0, -5.0, 3.0], [-5.0, -50.0, 3.0], 1000.0);
        let s = ts.select(&side, &m, 100);
        assert!(s.culled > 0, "{s:?}");
    }
}

#[test]
fn sse_formula() {
    let v = view_from([0.0, 0.0, 10.0], [0.0; 3], 1000.0);
    let tan = (30f64).to_radians().tan();
    assert!((v.sse(1.0, 10.0) - 1000.0 / (20.0 * tan)).abs() < 1e-9);
    let o = View {
        ortho_height: Some(50.0),
        ..v
    };
    assert!((o.sse(1.0, 123.0) - 20.0).abs() < 1e-12);
}

#[test]
fn b3dm_with_rtc_in_a_geographic_tileset() {
    // A tile on the equator at longitude 0: x points up there.
    let r = 6_378_137.0;
    let mut m = Mem::default();
    m.put(
        "tileset.json",
        format!(
            r#"{{"asset":{{"version":"1.0"}},"geometricError":10,"root":{{"boundingVolume":{{"sphere":[{r},0,0,10]}},"geometricError":0,"content":{{"uri":"a.b3dm"}}}}}}"#
        ),
    );
    m.put("a.b3dm", b3dm(Some([r, 0.0, 0.0])));
    let ts = Tileset::open("tileset.json", &m).unwrap();
    assert!(ts.geographic);
    let (c, errors) = load(&ts, 0, &m, &Limits::default());
    assert!(errors.is_empty(), "{errors:?}");
    let mesh = c[0].mesh.as_ref().unwrap();
    // glTF (0,0,0),(1,0,0),(0,1,0) y-up → z-up → earth-fixed (r,0,0)+(x,−z,y) → local ENU at
    // (r,0,0): east = +y, north = +z, up = +x: (1,0,0) → up 1 m; (0,1,0) → north 1 m.
    let p = &mesh.positions;
    let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
    assert!(near(p[0], [0.0, 0.0, 0.0]), "{p:?}");
    assert!(near(p[1], [0.0, 0.0, 1.0]), "{p:?}");
    assert!(near(p[2], [0.0, 1.0, 0.0]), "{p:?}");
}

#[test]
fn points_quantized_and_coloured() {
    let n = 3u32;
    let mut bin = Vec::new();
    for q in [[0u16, 0, 0], [65535, 0, 0], [0, 65535, 65535]] {
        for v in q {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let rgb_at = bin.len();
    bin.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255]);
    let rgb565_note = rgb_at;
    let ft = format!(
        r#"{{"POINTS_LENGTH":{n},"POSITION_QUANTIZED":{{"byteOffset":0}},"QUANTIZED_VOLUME_OFFSET":[-1,-2,-3],"QUANTIZED_VOLUME_SCALE":[2,4,6],"RGB":{{"byteOffset":{rgb565_note}}}}}"#
    );
    let mut m = Mem::default();
    m.put("p.pnts", tile(b"pnts", &ft, &bin, &[], None));
    m.put(
        "tileset.json",
        r#"{"asset":{"version":"1.0"},"geometricError":1,"root":{"boundingVolume":{"box":[0,0,0,1,0,0,0,2,0,0,0,3]},"geometricError":0,"content":{"uri":"p.pnts"}}}"#,
    );
    let ts = Tileset::open("tileset.json", &m).unwrap();
    let (c, e) = load(&ts, 0, &m, &Limits::default());
    assert!(e.is_empty(), "{e:?}");
    let p = c[0].points.as_ref().unwrap();
    assert_eq!(
        p.positions,
        vec![[-1.0, -2.0, -3.0], [1.0, -2.0, -3.0], [-1.0, 2.0, 3.0]]
    );
    assert_eq!(p.colors.as_ref().unwrap()[1], [0, 255, 0, 255]);
}

#[test]
fn instances_and_composites() {
    let mut bin = Vec::new();
    for v in [0f32, 0.0, 0.0, 10.0, 0.0, 0.0] {
        bin.extend_from_slice(&v.to_le_bytes());
    }
    let scale_at = bin.len();
    for v in [1f32, 2.0] {
        bin.extend_from_slice(&v.to_le_bytes());
    }
    let ft = format!(
        r#"{{"INSTANCES_LENGTH":2,"POSITION":{{"byteOffset":0}},"SCALE":{{"byteOffset":{scale_at}}}}}"#
    );
    let i3dm = tile(b"i3dm", &ft, &bin, &glb_triangle(1.0), Some(1));
    let b = b3dm(None);
    let mut cmpt = Vec::new();
    cmpt.extend_from_slice(b"cmpt");
    cmpt.extend_from_slice(&1u32.to_le_bytes());
    cmpt.extend_from_slice(&((16 + i3dm.len() + b.len()) as u32).to_le_bytes());
    cmpt.extend_from_slice(&2u32.to_le_bytes());
    cmpt.extend_from_slice(&i3dm);
    cmpt.extend_from_slice(&b);
    let mut m = Mem::default();
    m.put("c.cmpt", cmpt);
    m.put(
        "tileset.json",
        r#"{"asset":{"version":"1.0"},"geometricError":1,"root":{"boundingVolume":{"sphere":[0,0,0,20]},"geometricError":0,"content":{"uri":"c.cmpt"}}}"#,
    );
    let ts = Tileset::open("tileset.json", &m).unwrap();
    let (c, e) = load(&ts, 0, &m, &Limits::default());
    assert!(e.is_empty(), "{e:?}");
    let mesh = c[0].mesh.as_ref().unwrap();
    assert_eq!(mesh.triangle_count(), 3);
    assert_eq!(c[0].kinds, vec!["cmpt", "i3dm", "b3dm"]);
    // Second instance at x 10, scale 2: its triangle reaches x 12.
    assert!(
        (mesh.bounds.max[0] - 12.0).abs() < 1e-4,
        "{:?}",
        mesh.bounds
    );
    assert!((mesh.bounds.max[2] - 2.0).abs() < 1e-4, "{:?}", mesh.bounds);
}

#[test]
fn implicit_quadtree_and_external() {
    let mut m = Mem::default();
    m.put(
        "tileset.json",
        r#"{"asset":{"version":"1.1"},"geometricError":100,"root":{"boundingVolume":{"box":[0,0,0,10,0,0,0,10,0,0,0,1]},"geometricError":100,"refine":"REPLACE","children":[
          {"boundingVolume":{"box":[0,0,0,10,0,0,0,10,0,0,0,1]},"geometricError":50,"content":{"uri":"imp.json"}}]}}"#,
    );
    m.put(
        "imp.json",
        r#"{"asset":{"version":"1.1"},"geometricError":50,"root":{"boundingVolume":{"box":[0,0,0,10,0,0,0,10,0,0,0,1]},"geometricError":50,"refine":"REPLACE",
          "content":{"uri":"t/{level}/{x}/{y}.glb"},
          "implicitTiling":{"subdivisionScheme":"QUADTREE","subtreeLevels":2,"availableLevels":3,"subtrees":{"uri":"s/{level}.{x}.{y}.json"}}}}"#,
    );
    // Subtree 0: root and 4 children; only child (1,1) has a child subtree.
    m.put(
        "s/0.0.0.json",
        r#"{"tileAvailability":{"constant":1},"contentAvailability":[{"constant":1}],"childSubtreeAvailability":{"bitstream":0},
           "buffers":[{"uri":"bits.bin","byteLength":2}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":2}]}"#,
    );
    // Morton of (3,3) at level 2 relative to subtree root: 15 → byte 1, bit 7.
    m.put("s/bits.bin", vec![0u8, 0x80]);
    m.put(
        "s/2.3.3.json",
        r#"{"tileAvailability":{"constant":1},"contentAvailability":[{"constant":0}],"childSubtreeAvailability":{"constant":0}}"#,
    );
    let mut ts = Tileset::open("tileset.json", &m).unwrap();
    ts.expand_all(&m);
    assert!(ts.notes.is_empty(), "{:?}", ts.notes);
    // 2 explicit + external root + 5 implicit + 1 deeper.
    assert_eq!(ts.tiles.len(), 9);
    let deep = ts
        .tiles
        .iter()
        .find(|t| t.implicit.is_some_and(|a| a.level == 2))
        .unwrap();
    assert!(deep.contents.is_empty());
    match deep.volume {
        volume::Volume::Box { c, axes } => {
            assert!(
                (c[0] - 7.5).abs() < 1e-9 && (c[1] - 7.5).abs() < 1e-9,
                "{c:?}"
            );
            assert!((axes[0][0] - 2.5).abs() < 1e-9);
        }
        v => panic!("{v:?}"),
    }
    let lvl1: Vec<&String> = ts
        .tiles
        .iter()
        .filter(|t| t.implicit.is_some_and(|a| a.level == 1))
        .flat_map(|t| &t.contents)
        .collect();
    assert_eq!(lvl1.len(), 4);
    assert!(lvl1.iter().any(|k| *k == "t/1/1/0.glb"), "{lvl1:?}");
    assert!((deep.error - 12.5).abs() < 1e-12);
}

#[test]
fn region_boxes() {
    let r = [0.0, 0.0, 0.01, 0.01, 0.0, 100.0];
    let v = volume::region_box(r);
    for (lon, lat, h) in [(0.0, 0.0, 0.0), (0.01, 0.01, 100.0), (0.005, 0.005, 50.0)] {
        let p = math::geodetic_to_ecef(lon, lat, h);
        assert!(v.distance(p) < 1e-6, "{lon} {lat} {h}");
    }
    let far = math::geodetic_to_ecef(0.005, 0.005, 1000.0);
    assert!((v.distance(far) - 900.0).abs() < 1.0);
    let (lon, lat, h) = math::ecef_to_geodetic(math::geodetic_to_ecef(0.3, -0.7, 1234.5));
    assert!((lon - 0.3).abs() < 1e-12 && (lat + 0.7).abs() < 1e-12 && (h - 1234.5).abs() < 1e-6);
}

#[test]
fn refusals_in_words() {
    let mut m = Mem::default();
    m.put(
        "tileset.json",
        r#"{"asset":{"version":"1.0"},"extensionsRequired":["3DTILES_bounding_volume_S2"],"geometricError":1,"root":{"boundingVolume":{"box":[0,0,0,1,0,0,0,1,0,0,0,1]},"geometricError":0}}"#,
    );
    let e = Tileset::open("tileset.json", &m).err().unwrap();
    assert!(e.to_string().contains("S2"), "{e}");
    let draco = tile(
        b"pnts",
        r#"{"POINTS_LENGTH":1,"extensions":{"3DTILES_draco_point_compression":{}}}"#,
        &[],
        &[],
        None,
    );
    let place = Place {
        to_local: math::IDENTITY,
        up: tileset::UpAxis::Y,
    };
    let e = content::read(&draco, "a.pnts", &place, &m, &Limits::default()).unwrap_err();
    assert!(e.to_string().contains("Draco"), "{e}");
    let e = content::read(b"vctr0000", "a.vctr", &place, &m, &Limits::default()).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)));
}
