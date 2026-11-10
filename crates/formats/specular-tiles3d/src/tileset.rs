//! The tile tree: explicit tiles from `tileset.json`, external tilesets grafted under the tile
//! that names them, and implicit trees grown from their subtree files, all read only when the
//! traversal first needs a tile's children.

use std::sync::Arc;

use serde_json::Value;

use crate::fetch::{self, Fetch};
use crate::implicit::{self, Scheme, Subtree};
use crate::math::{self, M4, V3};
use crate::volume::{Given, Volume};
use crate::{Error, MAX_JSON, MAX_TILES, Result};

/// How a tile's children improve on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refine {
    /// The children are drawn with the tile.
    Add,
    /// The children are drawn in place of the tile.
    Replace,
}

/// glTF content's up axis (3D Tiles 1.0 `asset.gltfUpAxis`; 1.1 is always y).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpAxis {
    X,
    Y,
    Z,
}

impl UpAxis {
    pub fn to_z_up(self) -> M4 {
        match self {
            UpAxis::X => math::X_UP_TO_Z_UP,
            UpAxis::Y => math::Y_UP_TO_Z_UP,
            UpAxis::Z => math::IDENTITY,
        }
    }
}

/// Children not read yet.
#[derive(Clone, Debug)]
pub enum Pending {
    None,
    /// An external tileset (its key).
    External(String),
    /// The implicit tree this tile declares (its root still to be read).
    ImplicitRoot(u32),
    /// An implicit tile's children.
    Implicit,
    /// Reading the children failed (why).
    Failed(String),
}

/// Where an implicit tile sits.
#[derive(Clone, Copy, Debug)]
pub struct ImplicitAt {
    pub scheme: u32,
    pub level: u32,
    pub xyz: [u64; 3],
    /// Its subtree (index into [`Tileset::subtrees`]) and its level in it.
    pub subtree: u32,
    pub local: u32,
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    /// In the tileset's coordinates (earth-fixed for geographic tilesets).
    pub volume: Volume,
    pub request: Option<Volume>,
    /// Content's own (tighter) volume, when given.
    pub content_volume: Option<Volume>,
    pub error: f64,
    pub refine: Refine,
    /// The tile's transform composed with its ancestors'.
    pub world: M4,
    /// Keys of drawable contents.
    pub contents: Vec<String>,
    pub pending: Pending,
    pub implicit: Option<ImplicitAt>,
    /// glTF up axis of the tileset file the tile comes from.
    pub up: UpAxis,
    pub depth: u32,
    /// Its children are an external tileset or an implicit tree: traversals always pass
    /// through it.
    pub through: bool,
}

/// A whole tileset as read so far.
pub struct Tileset {
    pub tiles: Vec<Tile>,
    /// The key of the root `tileset.json`.
    pub key: String,
    pub version: String,
    pub tileset_version: Option<String>,
    pub generator: Option<String>,
    pub copyright: Vec<String>,
    /// The root's geometric error.
    pub error: f64,
    pub schemes: Vec<Arc<Scheme>>,
    pub subtrees: Vec<Option<Arc<Subtree>>>,
    /// The subtree's root coordinates (level, xyz) by subtree index.
    pub subtree_roots: Vec<(u32, [u64; 3])>,
    /// Read problems, once each.
    pub notes: Vec<String>,
    /// External tilesets read.
    pub externals: usize,
    /// Local (drawing) frame → tileset frame.
    pub frame: M4,
    /// Tileset frame → local.
    pub frame_inv: M4,
    /// Whether the tileset is placed on the Earth (earth-fixed coordinates).
    pub geographic: bool,
}

/// The URIs of a tile's contents (`content`, `contents`, or the 1.0 multiple-contents
/// extension), as written.
pub(crate) fn content_uris(tile: &Value) -> Vec<String> {
    let one = |c: &Value| {
        c.get("uri")
            .or_else(|| c.get("url"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let mut out: Vec<String> = Vec::new();
    if let Some(c) = tile.get("content") {
        out.extend(one(c));
    }
    for k in [
        "/contents",
        "/extensions/3DTILES_multiple_contents/contents",
        "/extensions/3DTILES_multiple_contents/content",
    ] {
        if let Some(a) = tile.pointer(k).and_then(Value::as_array) {
            out.extend(a.iter().filter_map(one));
        }
    }
    out
}

fn is_json_key(k: &str) -> bool {
    let p = k.split(['?', '#']).next().unwrap_or(k);
    p.to_ascii_lowercase().ends_with(".json")
}

impl Tileset {
    /// Reads the tileset at `key` (its root only; children as needed).
    pub fn open(key: &str, fetch: &dyn Fetch) -> Result<Tileset> {
        let bytes = fetch::load(fetch, key, MAX_JSON).map_err(Error::Missing)?;
        Tileset::from_bytes(key, &bytes)
    }

    /// Reads the tileset whose root file `key` holds `bytes`.
    pub fn from_bytes(key: &str, bytes: &[u8]) -> Result<Tileset> {
        let doc: Value = serde_json::from_slice(bytes)
            .map_err(|e| Error::Malformed(format!("tileset JSON: {e}")))?;
        let asset = doc
            .get("asset")
            .ok_or_else(|| Error::Malformed("no asset (not a 3D Tiles tileset)".into()))?;
        let version = asset
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if !(version.starts_with("1.") || version == "0.0" || version == "1") {
            return Err(Error::Unsupported(format!("3D Tiles version {version}")));
        }
        for e in doc
            .get("extensionsRequired")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if e.as_str() == Some("3DTILES_bounding_volume_S2") {
                return Err(Error::Unsupported(
                    "S2 cell bounding volumes (3DTILES_bounding_volume_S2)".into(),
                ));
            }
        }
        let mut ts = Tileset {
            tiles: Vec::new(),
            key: key.to_owned(),
            version,
            tileset_version: asset
                .get("tilesetVersion")
                .and_then(Value::as_str)
                .map(str::to_owned),
            generator: asset
                .get("generator")
                .and_then(Value::as_str)
                .map(str::to_owned),
            copyright: Vec::new(),
            error: doc
                .get("geometricError")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            schemes: Vec::new(),
            subtrees: Vec::new(),
            subtree_roots: Vec::new(),
            notes: Vec::new(),
            externals: 0,
            frame: math::IDENTITY,
            frame_inv: math::IDENTITY,
            geographic: false,
        };
        if let Some(c) = asset.get("copyright").and_then(Value::as_str) {
            ts.copyright.push(c.to_owned());
        }
        let root = doc
            .get("root")
            .ok_or_else(|| Error::Malformed("the tileset has no root tile".into()))?;
        let up = up_axis(asset);
        ts.graft(root, key, None, &math::IDENTITY, up);
        if ts.tiles.is_empty() {
            return Err(Error::Malformed(
                "the root tile has no bounding volume".into(),
            ));
        }
        ts.set_frame();
        Ok(ts)
    }

    /// Picks the drawing frame: near the root's centre, turned east-north-up when the
    /// tileset sits on the Earth.
    fn set_frame(&mut self) {
        let c = self.tiles[0].volume.centre();
        if math::len(c) > 1.0e5 {
            self.frame = math::enu_at(c);
            self.geographic = true;
        } else {
            self.frame = math::translation(c);
        }
        self.frame_inv = math::rigid_inverse(&self.frame);
    }

    /// Adds the tile `v` (from the file `base`) and its explicit descendants; returns its
    /// index.
    fn graft(
        &mut self,
        v: &Value,
        base: &str,
        parent: Option<u32>,
        parent_world: &M4,
        up: UpAxis,
    ) -> Option<u32> {
        // Depth-first with an explicit stack (json, parent, parent's world, depth, inherited
        // refinement); children are pushed in reverse, so each parent lists them in order.
        let inherited = parent.map_or(Refine::Replace, |p| self.tiles[p as usize].refine);
        let depth0 = parent.map_or(0, |p| self.tiles[p as usize].depth + 1);
        let mut stack: Vec<(&Value, Option<u32>, M4, u32, Refine)> =
            vec![(v, parent, *parent_world, depth0, inherited)];
        let mut first = None;
        while let Some((t, par, pw, depth, inh)) = stack.pop() {
            if self.tiles.len() >= MAX_TILES {
                self.note("more tiles than can be shown; the rest are left out".into());
                break;
            }
            if depth > 256 {
                self.note("tiles nested too deeply; the deepest are left out".into());
                continue;
            }
            let world = match t.get("transform").and_then(Value::as_array) {
                Some(a) => {
                    let nums: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
                    match math::from_column_major(&nums) {
                        Some(m) => math::mul(&pw, &m),
                        None => {
                            self.note("a tile transform is not 16 numbers; ignored".into());
                            pw
                        }
                    }
                }
                None => pw,
            };
            let Some(given) = t.get("boundingVolume").and_then(Given::parse) else {
                self.note("a tile without a usable bounding volume is left out".into());
                continue;
            };
            let refine = match t.get("refine").and_then(Value::as_str) {
                Some(r) if r.eq_ignore_ascii_case("add") => Refine::Add,
                Some(r) if r.eq_ignore_ascii_case("replace") => Refine::Replace,
                _ => inh,
            };
            let error = t
                .get("geometricError")
                .and_then(Value::as_f64)
                .filter(|e| e.is_finite() && *e >= 0.0)
                .unwrap_or(0.0);
            let mut contents = Vec::new();
            let mut pending = Pending::None;
            let imp = t
                .get("implicitTiling")
                .or_else(|| t.pointer("/extensions/3DTILES_implicit_tiling"));
            if imp.is_none() {
                for u in content_uris(t) {
                    match fetch::resolve(base, &u) {
                        Some(k) if is_json_key(&k) => {
                            if matches!(pending, Pending::None) {
                                pending = Pending::External(k);
                            }
                        }
                        Some(k) => contents.push(k),
                        None => self.note(format!("{u}: outside the tileset, not read")),
                    }
                }
            }
            let id = self.tiles.len() as u32;
            if let Some(imp) = imp {
                match Scheme::parse(imp, t, base, given, error) {
                    Ok(s) => {
                        self.schemes.push(Arc::new(s));
                        pending = Pending::ImplicitRoot(self.schemes.len() as u32 - 1);
                    }
                    Err(e) => pending = Pending::Failed(e.to_string()),
                }
            }
            let volume = given.world(&world);
            let request = t
                .get("viewerRequestVolume")
                .and_then(Given::parse)
                .map(|g| g.world(&world));
            let content_volume = t
                .pointer("/content/boundingVolume")
                .and_then(Given::parse)
                .map(|g| g.world(&world));
            let through = matches!(pending, Pending::External(_) | Pending::ImplicitRoot(_));
            self.tiles.push(Tile {
                parent: par,
                children: Vec::new(),
                volume,
                request,
                content_volume,
                error,
                refine,
                world,
                contents,
                pending,
                implicit: None,
                up,
                depth,
                through,
            });
            if let Some(p) = par {
                self.tiles[p as usize].children.push(id);
            }
            first.get_or_insert(id);
            if let Some(kids) = t.get("children").and_then(Value::as_array) {
                for k in kids.iter().rev() {
                    stack.push((k, Some(id), world, depth + 1, refine));
                }
            }
        }
        first
    }

    /// Records a problem once.
    pub fn note(&mut self, n: String) {
        if !self.notes.contains(&n) && self.notes.len() < 64 {
            self.notes.push(n);
        }
    }

    /// Whether tile `i` has children still to read.
    pub fn has_pending(&self, i: u32) -> bool {
        matches!(
            self.tiles[i as usize].pending,
            Pending::External(_) | Pending::ImplicitRoot(_) | Pending::Implicit
        )
    }

    /// Reads tile `i`'s pending children (an external tileset, an implicit tree's root or
    /// an implicit tile's children). Problems end up in [`Tileset::notes`] and leave the tile
    /// without children.
    pub fn expand(&mut self, i: u32, fetch: &dyn Fetch) {
        let pending = std::mem::replace(&mut self.tiles[i as usize].pending, Pending::None);
        let r = match pending {
            Pending::External(key) => self.expand_external(i, &key, fetch),
            Pending::ImplicitRoot(s) => self.expand_implicit_root(i, s, fetch),
            Pending::Implicit => self.expand_implicit(i, fetch),
            p => {
                self.tiles[i as usize].pending = p;
                Ok(())
            }
        };
        if let Err(e) = r {
            let why = e.to_string();
            self.note(why.clone());
            self.tiles[i as usize].pending = Pending::Failed(why);
        }
    }

    fn expand_external(&mut self, i: u32, key: &str, fetch: &dyn Fetch) -> Result<()> {
        // A tileset that names itself (or an ancestor's file) would loop.
        let mut depth = 0;
        let mut a = Some(i);
        while let Some(p) = a {
            depth += 1;
            a = self.tiles[p as usize].parent;
        }
        if depth > 64 {
            return Err(Error::Malformed(
                "external tilesets nested too deeply".into(),
            ));
        }
        let bytes =
            fetch::load(fetch, key, MAX_JSON).map_err(|e| Error::Missing(format!("{key}: {e}")))?;
        let doc: Value =
            serde_json::from_slice(&bytes).map_err(|e| Error::Malformed(format!("{key}: {e}")))?;
        let root = doc
            .get("root")
            .ok_or_else(|| Error::Malformed(format!("{key}: no root tile")))?;
        let up = doc.get("asset").map_or(UpAxis::Y, up_axis);
        if let Some(c) = doc.pointer("/asset/copyright").and_then(Value::as_str)
            && !self.copyright.iter().any(|x| x == c)
        {
            self.copyright.push(c.to_owned());
        }
        let world = self.tiles[i as usize].world;
        self.externals += 1;
        self.graft(root, key, Some(i), &world, up);
        Ok(())
    }

    /// Loads the subtree at (`level`, `xyz`) of scheme `s`.
    fn subtree(&mut self, s: u32, level: u32, xyz: [u64; 3], fetch: &dyn Fetch) -> Result<u32> {
        let scheme = self.schemes[s as usize].clone();
        let uri = Scheme::fill(&scheme.subtree_uri, level, xyz);
        let key = fetch::resolve(&scheme.base, &uri)
            .ok_or_else(|| Error::Missing(format!("{uri}: outside the tileset")))?;
        let bytes = fetch::load(fetch, &key, MAX_JSON)
            .map_err(|e| Error::Missing(format!("{key}: {e}")))?;
        let sub = implicit::read_subtree(&bytes, &key, fetch)?;
        self.subtrees.push(Some(Arc::new(sub)));
        self.subtree_roots.push((level, xyz));
        Ok(self.subtrees.len() as u32 - 1)
    }

    fn expand_implicit_root(&mut self, i: u32, s: u32, fetch: &dyn Fetch) -> Result<()> {
        let st = self.subtree(s, 0, [0; 3], fetch)?;
        if self.subtrees[st as usize]
            .as_ref()
            .is_some_and(|t| t.tiles.get(0))
        {
            self.add_implicit(i, s, 0, [0; 3], st, 0);
        }
        Ok(())
    }

    fn expand_implicit(&mut self, i: u32, fetch: &dyn Fetch) -> Result<()> {
        let Some(at) = self.tiles[i as usize].implicit else {
            return Ok(());
        };
        let scheme = self.schemes[at.scheme as usize].clone();
        if at.level + 1 >= scheme.available_levels {
            return Ok(());
        }
        let n = scheme.branching();
        let sub = self.subtrees[at.subtree as usize].clone();
        let Some(sub) = sub else { return Ok(()) };
        let (root_level, root_xyz) = self.subtree_roots[at.subtree as usize];
        for c in 0..n {
            let xyz = [
                2 * at.xyz[0] + (c & 1),
                2 * at.xyz[1] + ((c >> 1) & 1),
                2 * at.xyz[2] + if scheme.octree { (c >> 2) & 1 } else { 0 },
            ];
            let local = at.local + 1;
            let rel_shift = at.level + 1 - root_level;
            let rel = [
                xyz[0] - (root_xyz[0] << rel_shift),
                xyz[1] - (root_xyz[1] << rel_shift),
                xyz[2] - (root_xyz[2] << rel_shift),
            ];
            let m = implicit::morton(rel, scheme.octree);
            if local < scheme.subtree_levels {
                let bit = implicit::level_offset(local, n) + m;
                if sub.tiles.get(bit) {
                    self.add_implicit(i, at.scheme, at.level + 1, xyz, at.subtree, local);
                }
            } else if sub.children.get(m) {
                match self.subtree(at.scheme, at.level + 1, xyz, fetch) {
                    Ok(st) => {
                        if self.subtrees[st as usize]
                            .as_ref()
                            .is_some_and(|t| t.tiles.get(0))
                        {
                            self.add_implicit(i, at.scheme, at.level + 1, xyz, st, 0);
                        }
                    }
                    Err(e) => self.note(e.to_string()),
                }
            }
        }
        Ok(())
    }

    fn add_implicit(
        &mut self,
        parent: u32,
        s: u32,
        level: u32,
        xyz: [u64; 3],
        st: u32,
        local: u32,
    ) {
        if self.tiles.len() >= MAX_TILES {
            self.note("more tiles than can be shown; the rest are left out".into());
            return;
        }
        let scheme = self.schemes[s as usize].clone();
        let p = &self.tiles[parent as usize];
        let (world, refine, up, depth) = (p.world, p.refine, p.up, p.depth + 1);
        let Some(given) = scheme.given.part(level, xyz, scheme.octree) else {
            return;
        };
        let sub = self.subtrees[st as usize].clone();
        let n = scheme.branching();
        let (root_level, root_xyz) = self.subtree_roots[st as usize];
        let shift = level - root_level;
        let rel = [
            xyz[0] - (root_xyz[0] << shift),
            xyz[1] - (root_xyz[1] << shift),
            xyz[2] - (root_xyz[2] << shift),
        ];
        let bit = implicit::level_offset(local, n) + implicit::morton(rel, scheme.octree);
        let mut contents = Vec::new();
        for (ci, t) in scheme.content_uris.iter().enumerate() {
            let avail = sub
                .as_ref()
                .and_then(|s| s.contents.get(ci))
                .is_some_and(|b| b.get(bit));
            if avail {
                let uri = Scheme::fill(t, level, xyz);
                match fetch::resolve(&scheme.base, &uri) {
                    Some(k) => contents.push(k),
                    None => self.note(format!("{uri}: outside the tileset, not read")),
                }
            }
        }
        let id = self.tiles.len() as u32;
        self.tiles.push(Tile {
            parent: Some(parent),
            children: Vec::new(),
            volume: given.world(&world),
            request: None,
            content_volume: None,
            error: scheme.error / 2f64.powi(level as i32),
            refine,
            world,
            contents,
            pending: Pending::Implicit,
            implicit: Some(ImplicitAt {
                scheme: s,
                level,
                xyz,
                subtree: st,
                local,
            }),
            up,
            depth,
            through: false,
        });
        self.tiles[parent as usize].children.push(id);
    }

    /// Reads every pending part of the tree (for counting and tests), up to [`MAX_TILES`].
    pub fn expand_all(&mut self, fetch: &dyn Fetch) {
        let mut i = 0;
        while i < self.tiles.len() {
            if self.has_pending(i as u32) {
                self.expand(i as u32, fetch);
            }
            i += 1;
        }
    }

    /// The local (drawing) position of a tileset point.
    pub fn to_local(&self, p: V3) -> V3 {
        math::point(&self.frame_inv, p)
    }

    /// The root's box in local coordinates: its corners' bounds.
    pub fn local_bounds(&self) -> ([f64; 3], [f64; 3]) {
        let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for c in self.tiles[0].volume.corners() {
            let p = self.to_local(c);
            for k in 0..3 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }
        (mn, mx)
    }
}

fn up_axis(asset: &Value) -> UpAxis {
    match asset.get("gltfUpAxis").and_then(Value::as_str) {
        Some(a) if a.eq_ignore_ascii_case("x") => UpAxis::X,
        Some(a) if a.eq_ignore_ascii_case("z") => UpAxis::Z,
        _ => UpAxis::Y,
    }
}
