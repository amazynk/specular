//! Cesium 3D Tiles tilesets (`tileset.json`, or the folder holding one) via
//! specular-tiles3d, shown in the 3D view.
//!
//! Opening reads the root tileset and draws the tiles a fitted iso view selects: that scene
//! is the session's model ([`Session::mesh`], or [`Session::points`] for a tileset of points)
//! and its CPU preview the thumbnail. The app then asks [`Tiles3dModel`] for the tiles its
//! camera needs ([`Tiles3dModel::select`]) and the scene they make ([`Tiles3dModel::scene`]),
//! as the view moves. Tiles read are kept (least recently used dropped first) up to
//! [`CACHE_BYTES`].
//!
//! A tileset on a server: the app fetches its `tileset.json` into the cache with the address
//! beside it ([`ADDRESS_FILE`]); this reader then reads every tile through the app's fetcher
//! ([`set_tiles3d_remote`]), which honours the no-network switch and shows the download.
//! This crate does no networking.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use specular_core::{CancelToken, Source};
use specular_mesh::preview::Texture;
use specular_mesh::{Bounds, Limits, Mesh};
use specular_tiles3d::math::{self, V3};
use specular_tiles3d::{DirFetch, Fetch, Points, Tileset, View};

use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, MeshModel,
    PointsModel, Raster, Result, Session,
};

/// The file beside a cached remote `tileset.json` holding its address.
pub const ADDRESS_FILE: &str = "tileset.address";
/// Bytes of tiles kept in memory at most.
pub const CACHE_BYTES: usize = 1536 << 20;
/// Tiles drawn at most; a view that wants more is drawn coarser.
pub const MAX_DRAWN_TILES: usize = 2000;
/// Triangles and points drawn at most (the rest of a selection is left out).
pub const MAX_TRIANGLES: usize = 30_000_000;
pub const MAX_POINTS: usize = 10_000_000;
/// Tiles near a walker read for collision at most.
pub const MAX_NEAR_TILES: usize = 256;
/// The default iso view's height (px) for the first scene.
const FIRST_VIEW_PX: f64 = 1024.0;

/// What reads a remote tileset's files (keys are full addresses).
pub type RemoteFetch = Arc<dyn Fetch + Send>;

static REMOTE: RwLock<Option<RemoteFetch>> = RwLock::new(None);

/// Registers (or with `None` removes) the fetcher of tilesets on servers.
pub fn set_tiles3d_remote(r: Option<RemoteFetch>) {
    *REMOTE.write() = r;
}

/// A fetcher shared with the model.
#[derive(Clone)]
enum Files {
    Local(Arc<DirFetch>),
    Remote(RemoteFetch),
}

impl Fetch for Files {
    fn get(&self, key: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        match self {
            Files::Local(d) => d.get(key, max),
            Files::Remote(r) => r.get(key, max),
        }
    }
}

/// One tile's contents, read and placed, its textures decoded.
struct Loaded {
    mesh: Option<Mesh>,
    textures: Vec<Option<Texture>>,
    points: Option<Points>,
    errors: Vec<String>,
    bytes: usize,
}

#[derive(Default)]
struct Cache {
    tiles: HashMap<u32, (Arc<Loaded>, u64)>,
    bytes: usize,
    tick: u64,
}

struct Live {
    tileset: Mutex<Tileset>,
    files: Files,
    cache: Mutex<Cache>,
    /// The root's box in the drawing frame.
    bounds: Bounds,
    /// A file the textures' decoders nest under.
    parent: Arc<Source>,
    /// Tiles read so far (kept outside the tileset's lock).
    read: std::sync::atomic::AtomicUsize,
}

/// A view in the drawing frame (z up, near the tileset).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tiles3dView {
    pub eye: V3,
    /// Unit forward and up.
    pub dir: V3,
    pub up: V3,
    /// Vertical field of view (radians), in perspective.
    pub fov_y: f64,
    /// Orthographic: the height shown (drawing units); `None`: perspective.
    pub ortho_height: Option<f64>,
    pub width_px: f64,
    pub height_px: f64,
}

/// The scene some tiles make.
#[derive(Clone, Debug)]
pub struct Tiles3dScene {
    pub tiles: Vec<u32>,
    pub mesh: Option<MeshModel>,
    pub points: Option<PointsModel>,
    /// Contents that could not be drawn, with the reason.
    pub errors: Vec<String>,
    /// Tiles left out to stay within [`MAX_TRIANGLES`] / [`MAX_POINTS`].
    pub left_out: usize,
}

/// A tileset being looked at: selection and scenes for the app's view.
#[derive(Clone)]
pub struct Tiles3dModel {
    live: Arc<Live>,
}

impl std::fmt::Debug for Tiles3dModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tiles3dModel").finish_non_exhaustive()
    }
}

impl Tiles3dModel {
    /// The root's box in the drawing frame.
    pub fn bounds(&self) -> Bounds {
        self.live.bounds
    }

    /// The tiles `view` draws (coarser when more than [`MAX_DRAWN_TILES`] would be).
    pub fn select(&self, view: &Tiles3dView) -> Vec<u32> {
        let mut ts = self.live.tileset.lock();
        let v = to_tileset(&ts, view);
        let mut sse = specular_tiles3d::MAX_SSE;
        loop {
            let s = ts.select(
                &View { max_sse: sse, ..v },
                &self.live.files,
                MAX_DRAWN_TILES,
            );
            self.live
                .read
                .store(ts.tiles.len(), std::sync::atomic::Ordering::Relaxed);
            if s.tiles.len() < MAX_DRAWN_TILES || sse > 4096.0 {
                return s.tiles;
            }
            sse *= 2.0;
        }
    }

    /// What a walker at `at` (drawing frame) collides with: the finest tiles the tileset
    /// has within `reach` of it, whichever way it looks, and of the tiles `drawn` those out
    /// of reach (all of them when more than [`MAX_NEAR_TILES`] are near).
    pub fn collision_tiles(&self, at: V3, reach: f64, drawn: &[u32]) -> Vec<u32> {
        let mut ts = self.live.tileset.lock();
        let p = math::point(&ts.frame, at);
        let near = ts.select_near(p, reach, &self.live.files, MAX_NEAR_TILES);
        self.live
            .read
            .store(ts.tiles.len(), std::sync::atomic::Ordering::Relaxed);
        let mut seen: std::collections::HashSet<u32> = near.tiles.iter().copied().collect();
        let mut out = near.tiles;
        for &t in drawn {
            let keep = !near.complete
                || ts
                    .tiles
                    .get(t as usize)
                    .is_some_and(|x| x.volume.distance(p) > reach);
            if keep && seen.insert(t) {
                out.push(t);
            }
        }
        out
    }

    /// Tiles of the tree read so far.
    pub fn tiles_read(&self) -> usize {
        self.live.read.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether every one of `tiles` has been read.
    pub fn ready(&self, tiles: &[u32]) -> bool {
        let c = self.live.cache.lock();
        tiles.iter().all(|t| c.tiles.contains_key(t))
    }

    /// The scene of `tiles`, reading those not read yet (in parallel).
    pub fn scene(&self, tiles: &[u32], cancel: &CancelToken) -> Result<Tiles3dScene> {
        use rayon::prelude::*;
        let missing: Vec<u32> = {
            let c = self.live.cache.lock();
            tiles
                .iter()
                .copied()
                .filter(|t| !c.tiles.contains_key(t))
                .collect()
        };
        let read: Vec<(u32, Arc<Loaded>)> = missing
            .par_iter()
            .filter(|_| !cancel.is_cancelled())
            .map(|&t| (t, Arc::new(self.read_tile(t))))
            .collect();
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let loaded: Vec<Arc<Loaded>> = {
            let mut c = self.live.cache.lock();
            for (t, l) in read {
                c.bytes += l.bytes;
                c.tick += 1;
                let tick = c.tick;
                if let Some((old, _)) = c.tiles.insert(t, (l, tick)) {
                    c.bytes -= old.bytes;
                }
            }
            c.tick += 1;
            let tick = c.tick;
            let out = tiles
                .iter()
                .filter_map(|t| {
                    let e = c.tiles.get_mut(t)?;
                    e.1 = tick;
                    Some(e.0.clone())
                })
                .collect();
            evict(&mut c, tick);
            out
        };
        Ok(build_scene(tiles, &loaded))
    }

    fn read_tile(&self, t: u32) -> Loaded {
        let (tile, frame_inv) = {
            let ts = self.live.tileset.lock();
            (ts.tiles[t as usize].clone(), ts.frame_inv)
        };
        let (contents, errors) =
            specular_tiles3d::load_tile(&tile, &frame_inv, &self.live.files, &Limits::default());
        let mut meshes = Vec::new();
        let mut points = Vec::new();
        for c in contents {
            meshes.extend(c.mesh);
            points.extend(c.points);
        }
        let mesh = (!meshes.is_empty()).then(|| specular_tiles3d::merge::meshes(meshes));
        let points = (!points.is_empty()).then(|| specular_tiles3d::merge::points(points));
        let textures = mesh
            .as_ref()
            .map(|m| crate::mesh_dec::decode_textures(m, &self.live.parent))
            .unwrap_or_default();
        let bytes = mesh
            .as_ref()
            .map_or(0, |m| m.memory_bytes() + m.normals.len() * 12)
            + textures
                .iter()
                .flatten()
                .map(|t| t.rgba.len())
                .sum::<usize>()
            + points.as_ref().map_or(0, |p| p.positions.len() * 16)
            + 256;
        Loaded {
            mesh,
            textures,
            points,
            errors,
            bytes,
        }
    }
}

/// Drops the least recently used tiles over [`CACHE_BYTES`] (never those used at `now`).
fn evict(c: &mut Cache, now: u64) {
    if c.bytes <= CACHE_BYTES {
        return;
    }
    let mut by_age: Vec<(u64, u32)> = c.tiles.iter().map(|(k, v)| (v.1, *k)).collect();
    by_age.sort_unstable();
    for (tick, k) in by_age {
        if c.bytes <= CACHE_BYTES || tick >= now {
            break;
        }
        if let Some((l, _)) = c.tiles.remove(&k) {
            c.bytes -= l.bytes;
        }
    }
}

fn build_scene(tiles: &[u32], loaded: &[Arc<Loaded>]) -> Tiles3dScene {
    let mut meshes = Vec::new();
    let mut textures = Vec::new();
    let mut points = Vec::new();
    let mut errors = Vec::new();
    let (mut tris, mut pts, mut left_out) = (0usize, 0usize, 0usize);
    for l in loaded {
        errors.extend(l.errors.iter().cloned());
        let t = l.mesh.as_ref().map_or(0, Mesh::triangle_count);
        let p = l.points.as_ref().map_or(0, |p| p.positions.len());
        if tris + t > MAX_TRIANGLES || pts + p > MAX_POINTS {
            left_out += 1;
            continue;
        }
        tris += t;
        pts += p;
        if let Some(m) = &l.mesh {
            meshes.push(m.clone());
            textures.extend(l.textures.iter().cloned());
        }
        if let Some(p) = &l.points {
            points.push(p.clone());
        }
    }
    errors.sort();
    errors.dedup();
    let mesh = (!meshes.is_empty()).then(|| {
        let mut m = specular_tiles3d::merge::meshes(meshes);
        m.format = Some(specular_mesh::MeshFormat::Glb);
        MeshModel {
            mesh: Arc::new(m),
            textures: Arc::new(textures),
            ifc: None,
        }
    });
    let points = if points.is_empty() {
        None
    } else {
        let p = specular_tiles3d::merge::points(points);
        let rgb = p
            .colors
            .map(|c| c.into_iter().map(|c| [c[0], c[1], c[2]]).collect());
        specular_points::from_points(p.positions, rgb, [0.0; 3], "3D Tiles points")
            .ok()
            .map(|loading| PointsModel { loading })
    };
    Tiles3dScene {
        tiles: tiles.to_vec(),
        mesh,
        points,
        errors,
        left_out,
    }
}

/// A drawing-frame view in the tileset's coordinates.
fn to_tileset(ts: &Tileset, v: &Tiles3dView) -> View {
    View {
        eye: math::point(&ts.frame, v.eye),
        dir: math::vector(&ts.frame, v.dir),
        up: math::vector(&ts.frame, v.up),
        fov_y: v.fov_y,
        ortho_height: v.ortho_height,
        width_px: v.width_px,
        height_px: v.height_px,
        max_sse: specular_tiles3d::MAX_SSE,
        cull: true,
    }
}

/// The fitted iso view of `b` (drawing frame) for a `px`-high square view.
pub fn iso_view(b: &Bounds, px: f64) -> Tiles3dView {
    let c: V3 = std::array::from_fn(|k| f64::from(b.min[k] + b.max[k]) / 2.0);
    let r = (f64::from(b.radius())).max(1e-3);
    let fov = 35f64.to_radians();
    let dir = math::unit([-1.0, 1.0, -0.8]);
    let eye = math::sub(c, math::scale(dir, r / (fov / 2.0).sin()));
    let right = math::unit(math::cross(dir, [0.0, 0.0, 1.0]));
    Tiles3dView {
        eye,
        dir,
        up: math::cross(right, dir),
        fov_y: fov,
        ortho_height: None,
        width_px: px,
        height_px: px,
    }
}

pub struct Tiles3dDecoder;

impl Decoder for Tiles3dDecoder {
    fn format(&self) -> Format {
        Format::Tiles3d
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_tiles3d::sniff(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_tiles3d::sniff(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let path = source.path().to_path_buf();
        let (dir, name) = if source.is_folder() {
            (path.clone(), "tileset.json".to_owned())
        } else {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "tileset.json".into());
            (path.parent().unwrap_or(Path::new(".")).to_path_buf(), name)
        };
        let address = std::fs::read_to_string(dir.join(ADDRESS_FILE))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| s.starts_with("https://") || s.starts_with("http://"));
        let (files, key) = match address {
            Some(url) => {
                let r = REMOTE.read().clone().ok_or_else(|| {
                    DecodeError::Unsupported(
                        "this tileset is on a server, and reading from servers is not set up"
                            .into(),
                    )
                })?;
                (Files::Remote(r), url)
            }
            None => (Files::Local(Arc::new(DirFetch::new(&dir))), name),
        };
        let ts = if source.is_folder() || matches!(files, Files::Remote(_)) {
            Tileset::open(&key, &files)
        } else {
            if source.len() > specular_tiles3d::MAX_JSON {
                return Err(DecodeError::TooLarge);
            }
            Tileset::from_bytes(&key, source.bytes())
        }
        .map_err(map_error)?;
        open_tileset(source, ts, files)
    }
}

fn map_error(e: specular_tiles3d::Error) -> DecodeError {
    match e {
        specular_tiles3d::Error::TooLarge(_) => DecodeError::TooLarge,
        specular_tiles3d::Error::Unsupported(m) => DecodeError::Unsupported(m),
        specular_tiles3d::Error::Malformed(m) => DecodeError::Malformed(m),
        specular_tiles3d::Error::Missing(m) => DecodeError::Malformed(m),
        specular_tiles3d::Error::Empty => DecodeError::Malformed("nothing to draw".into()),
    }
}

fn open_tileset(source: Arc<Source>, ts: Tileset, files: Files) -> Result<Arc<dyn Session>> {
    let (mn, mx) = ts.local_bounds();
    let mut bounds = Bounds::EMPTY;
    bounds.add(mn.map(|v| v as f32));
    bounds.add(mx.map(|v| v as f32));
    let facts = facts(&ts);
    let model = Tiles3dModel {
        live: Arc::new(Live {
            tileset: Mutex::new(ts),
            files,
            cache: Mutex::new(Cache::default()),
            bounds,
            parent: source.clone(),
            read: std::sync::atomic::AtomicUsize::new(0),
        }),
    };
    let tiles = model.select(&iso_view(&bounds, FIRST_VIEW_PX));
    let scene = model.scene(&tiles, &CancelToken::new())?;
    let mut header = HeaderFacts {
        summary: Vec::new(),
        fields: facts,
    };
    {
        let ts = model.live.tileset.lock();
        header
            .fields
            .push(("tiles read".into(), ts.tiles.len().to_string()));
        for n in ts.notes.iter().take(8) {
            header.fields.push(("not read".into(), n.clone()));
        }
    }
    for e in scene.errors.iter().take(8) {
        header.fields.push(("not drawn".into(), e.clone()));
    }
    let coding = {
        let ts = model.live.tileset.lock();
        format!(
            "3D Tiles {}, {} tiles drawn of {} read",
            ts.version,
            tiles.len(),
            ts.tiles.len()
        )
    };
    let inner: Arc<dyn Session> = if let Some(m) = &scene.mesh {
        crate::mesh_dec::session_with_header(
            source.clone(),
            Format::Tiles3d,
            (*m.mesh).clone(),
            None,
        )
    } else if let Some(p) = &scene.points {
        crate::points_dec::session_of(source.clone(), Format::Tiles3d, p.loading.clone())
    } else {
        let why = scene
            .errors
            .first()
            .cloned()
            .unwrap_or_else(|| "no tile has anything to draw".into());
        return Err(DecodeError::Unsupported(format!(
            "nothing in this tileset can be shown: {why}"
        )));
    };
    let mut info = inner.info().clone();
    info.coding = coding;
    Ok(Arc::new(Tiles3dSession {
        inner,
        info,
        model,
        header,
        points: scene.points,
    }))
}

/// The info panel's facts, named as the tileset names them.
fn facts(ts: &Tileset) -> Vec<(String, String)> {
    let mut f = vec![("asset.version".to_owned(), ts.version.clone())];
    if let Some(v) = &ts.tileset_version {
        f.push(("asset.tilesetVersion".into(), v.clone()));
    }
    if let Some(g) = &ts.generator {
        f.push(("asset.generator".into(), g.clone()));
    }
    for c in &ts.copyright {
        f.push(("asset.copyright".into(), c.clone()));
    }
    f.push(("geometricError".into(), format!("{}", ts.error)));
    let root = &ts.tiles[0];
    f.push((
        "root.refine".into(),
        match root.refine {
            specular_tiles3d::Refine::Add => "ADD".into(),
            specular_tiles3d::Refine::Replace => "REPLACE".into(),
        },
    ));
    if ts.geographic {
        let (lon, lat, h) = math::ecef_to_geodetic(root.volume.centre());
        f.push((
            "location".into(),
            format!(
                "{:.6}°, {:.6}°, {:.0} m",
                lat.to_degrees(),
                lon.to_degrees(),
                h
            ),
        ));
    }
    f
}

struct Tiles3dSession {
    inner: Arc<dyn Session>,
    info: ImageInfo,
    model: Tiles3dModel,
    header: HeaderFacts,
    /// The first scene's points (shown with its models, if any).
    points: Option<PointsModel>,
}

impl Session for Tiles3dSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        self.inner.source()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn scaled_decode_cheap(&self) -> bool {
        true
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes() + self.model.live.cache.lock().bytes
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn mesh(&self) -> Option<&MeshModel> {
        self.inner.mesh()
    }
    fn points(&self) -> Option<&PointsModel> {
        self.points.as_ref()
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.header)
    }
    fn tiles3d(&self) -> Option<&Tiles3dModel> {
        Some(&self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    /// A two-level tileset of one triangle each: opened as a file and as its folder, its
    /// first scene and a closer view's.
    #[test]
    fn opens_a_tileset_and_refines() {
        let dir = std::env::temp_dir().join(format!("specular-3dtiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let glb = specular_mesh::fixtures::glb();
        let mut b3dm = Vec::new();
        let ft = b"{\"BATCH_LENGTH\":0}      ";
        let total = 28 + ft.len() + glb.len();
        b3dm.extend_from_slice(b"b3dm");
        for v in [1u32, total as u32, ft.len() as u32, 0, 0, 0] {
            b3dm.extend_from_slice(&v.to_le_bytes());
        }
        b3dm.extend_from_slice(ft);
        b3dm.extend_from_slice(&glb);
        std::fs::write(dir.join("a.b3dm"), &b3dm).unwrap();
        std::fs::write(dir.join("b.b3dm"), &b3dm).unwrap();
        std::fs::write(
            dir.join("tileset.json"),
            br#"{"asset":{"version":"1.0","generator":"test"},"geometricError":500,"root":{
            "boundingVolume":{"sphere":[0,0,0,2]},"geometricError":0.5,"refine":"ADD","content":{"uri":"a.b3dm"},
            "children":[{"boundingVolume":{"sphere":[0,0,0,2]},"geometricError":0,"content":{"uri":"b.b3dm"},
              "transform":[1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,3,1]}]}}"#,
        )
        .unwrap();
        for path in [dir.join("tileset.json"), dir.clone()] {
            let s = Registry::default().open_path(&path).unwrap();
            assert_eq!(s.info().format, Format::Tiles3d, "{path:?}");
            let m = s.tiles3d().expect("a tileset");
            assert!(s.mesh().unwrap().mesh.triangle_count() > 0);
            let one = m.scene(&[0], &CancelToken::new()).unwrap();
            let tris = one.mesh.unwrap().mesh.triangle_count();
            assert!(tris > 0);
            // Far: the root alone; close: the child too (ADD).
            let far = iso_view(&m.bounds(), 100.0);
            let tiles = m.select(&far);
            assert_eq!(tiles, vec![0]);
            let near = Tiles3dView {
                eye: [0.0, -6.0, 1.0],
                dir: [0.0, 1.0, 0.0],
                up: [0.0, 0.0, 1.0],
                ..far
            };
            let tiles = m.select(&Tiles3dView {
                width_px: 4000.0,
                height_px: 4000.0,
                ..near
            });
            assert_eq!(tiles.len(), 2);
            let scene = m.scene(&tiles, &CancelToken::new()).unwrap();
            assert_eq!(scene.mesh.unwrap().mesh.triangle_count(), 2 * tris);
            assert!(m.ready(&tiles));
            let r = s
                .decode(&DecodeRequest::full(4), &CancelToken::new())
                .unwrap();
            assert!(r.width > 0);
            let h = s.header().unwrap();
            assert!(
                h.fields
                    .iter()
                    .any(|(k, v)| k == "asset.generator" && v == "test")
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Walking collides with the finest tiles near the walker, whichever way it looks,
    /// and with the drawn tiles out of its reach.
    #[test]
    fn walker_collides_with_the_finest_tiles_near_it() {
        let dir =
            std::env::temp_dir().join(format!("specular-3dtiles-near-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let glb = specular_mesh::fixtures::glb();
        let mut b3dm = Vec::new();
        let ft = b"{\"BATCH_LENGTH\":0}      ";
        let total = 28 + ft.len() + glb.len();
        b3dm.extend_from_slice(b"b3dm");
        for v in [1u32, total as u32, ft.len() as u32, 0, 0, 0] {
            b3dm.extend_from_slice(&v.to_le_bytes());
        }
        b3dm.extend_from_slice(ft);
        b3dm.extend_from_slice(&glb);
        std::fs::write(dir.join("t.b3dm"), &b3dm).unwrap();
        // Root over four quadrants (REPLACE), each refined once more.
        let quad = |x: i32, y: i32| {
            format!(
                r#"{{"boundingVolume":{{"sphere":[0,0,0,5]}},"geometricError":4,"content":{{"uri":"t.b3dm"}},
                "transform":[1,0,0,0, 0,1,0,0, 0,0,1,0, {x},{y},0,1],
                "children":[{{"boundingVolume":{{"sphere":[0,0,0,5]}},"geometricError":0,"content":{{"uri":"t.b3dm"}}}}]}}"#
            )
        };
        let kids: Vec<String> = [(10, 10), (-10, 10), (10, -10), (-10, -10)]
            .iter()
            .map(|&(x, y)| quad(x, y))
            .collect();
        std::fs::write(
            dir.join("tileset.json"),
            format!(
                r#"{{"asset":{{"version":"1.0"}},"geometricError":100,"root":{{
                "boundingVolume":{{"sphere":[0,0,0,20]}},"geometricError":50,"refine":"REPLACE",
                "content":{{"uri":"t.b3dm"}},"children":[{}]}}}}"#,
                kids.join(",")
            ),
        )
        .unwrap();
        let s = Registry::default()
            .open_path(&dir.join("tileset.json"))
            .unwrap();
        let m = s.tiles3d().expect("a tileset").clone();
        let centre = |t: u32| {
            let sc = m.scene(&[t], &CancelToken::new()).unwrap();
            let b = sc.mesh.unwrap().mesh.bounds;
            [
                f64::from(b.min[0] + b.max[0]) / 2.0,
                f64::from(b.min[1] + b.max[1]) / 2.0,
            ]
        };
        // Only the coarse root drawn (far away): near the walker it is replaced by the
        // finest tile there, read now.
        let tiles = m.collision_tiles([10.0, 10.0, 0.0], 2.0, &[0]);
        assert_eq!(tiles.len(), 1, "{tiles:?}");
        let c = centre(tiles[0]);
        assert!(
            (c[0] - 10.0).abs() < 3.0 && (c[1] - 10.0).abs() < 3.0,
            "{c:?}"
        );
        assert!(tiles[0] != 0);
        let fine = tiles[0];
        // A drawn tile out of reach stays; one in reach gives way.
        let far_quad = (1..9u32)
            .find(|&t| {
                let c = centre(t);
                c[0] < -5.0 && c[1] < -5.0
            })
            .unwrap();
        let tiles = m.collision_tiles([10.0, 10.0, 0.0], 2.0, &[far_quad, fine]);
        assert_eq!(tiles, vec![fine, far_quad]);
        let scene = m.scene(&tiles, &CancelToken::new()).unwrap();
        assert!(scene.mesh.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Previews of corpus tilesets as PNGs, to look at (tools/corpus/fetch-3dtiles.sh):
    /// `SPECULAR_TILES3D_CORPUS=… SPECULAR_TILES3D_OUT=… cargo test -p specular-decode
    /// --lib tiles3d_dec::tests::corpus_previews -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn corpus_previews() {
        let (Some(dir), Some(out)) = (
            std::env::var_os("SPECULAR_TILES3D_CORPUS"),
            std::env::var_os("SPECULAR_TILES3D_OUT"),
        ) else {
            return;
        };
        let (dir, out) = (std::path::PathBuf::from(dir), std::path::PathBuf::from(out));
        for rel in [
            "samples/1.0/TilesetWithDiscreteLOD/tileset.json",
            "samples/1.0/TilesetWithRequestVolume/tileset.json",
            "samples/1.0/TilesetWithTreeBillboards/tileset.json",
            "samples/1.1/SparseImplicitOctree/tileset.json",
            "cesium/Instanced/InstancedOrientation/tileset.json",
            "cesium/Tilesets/TilesetPoints/tileset.json",
            "cesium/Terrain/Test/tileset.json",
        ] {
            let t0 = std::time::Instant::now();
            let s = match Registry::default().open_path(&dir.join(rel)) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{rel}: {e}");
                    continue;
                }
            };
            let open_ms = t0.elapsed().as_secs_f64() * 1e3;
            let r = s
                .decode(&DecodeRequest::full(2), &CancelToken::new())
                .unwrap();
            let pl = &r.planes[0];
            let mut rgba = Vec::new();
            for y in 0..r.height as usize {
                rgba.extend_from_slice(
                    &pl.data[y * pl.stride..y * pl.stride + r.width as usize * 4],
                );
            }
            let png = specular_png::encode(r.width, r.height, specular_png::ColorType::Rgba, &rgba)
                .unwrap();
            let name = rel.replace('/', "_").replace(".json", ".png");
            std::fs::write(out.join(&name), png).unwrap();
            eprintln!(
                "{rel}: opened in {open_ms:.0} ms, {}, preview {}×{}",
                s.info().coding,
                r.width,
                r.height
            );
        }
    }

    #[test]
    fn refuses_in_words() {
        let dir = std::env::temp_dir().join(format!("specular-3dtiles-r-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tileset.json"),
            br#"{"asset":{"version":"1.0"},"geometricError":1,"root":{"boundingVolume":{"sphere":[0,0,0,2]},"geometricError":0,"content":{"uri":"x.vctr"}}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("x.vctr"), b"vctr\x01\0\0\0").unwrap();
        let e = Registry::default()
            .open_path(&dir.join("tileset.json"))
            .err()
            .unwrap();
        assert!(e.to_string().contains("vector tile"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
