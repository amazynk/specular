//! Read-only HDF5 decoder.
//!
//! [`File::open`] reads the superblock and walks the whole group graph once (bounded, with
//! cycle protection), collecting every group and dataset with its attributes. Nothing of the
//! raw data is touched until a read: the chunk index of a dataset is built on its first read,
//! and a read decodes only the chunks that intersect the requested box, in parallel, through
//! a byte-capped chunk cache.
//!
//! The file is passed in as a byte slice (a memory map) on every call; [`File`] owns only the
//! parsed metadata.
//!
//! Supported: superblocks v0-v3 (with a user block), object headers v1/v2 with continuations,
//! symbol-table groups (B-tree v1, local heaps), link-message groups, dense groups and dense
//! attributes (fractal heaps, B-tree v2 name indexes), committed datatypes, hard links,
//! compact / contiguous / chunked layouts v1-v4 with every chunk index (B-tree v1, single
//! chunk, implicit, fixed array, extensible array, B-tree v2), fill values, and the filters
//! deflate, shuffle, Fletcher-32, N-bit, scale-offset, SZIP, LZF, LZ4, Zstandard, bitshuffle,
//! Blosc and Blosc2 (frames, b2nd arrays and delta included). Integer and float
//! types (8-64 bit, both byte orders, half floats), enums, fixed and variable-length strings,
//! object references and arrays in attributes.
//!
//! Soft links are followed within the file (the link's path names its target); external links
//! are listed with their file and path, for the caller to open the other file
//! ([`Kind::External`]).
//!
//! Virtual datasets read their mappings (all, point and hyperslab selections) from datasets in
//! this file or, through a [`Resolve`] callback, in others. The numeric fields of compound
//! datasets read as datasets of their own ([`File::fields`]).
//!
//! Shared object-header messages are read from other headers and from the SOHM table's
//! fractal heaps (datatypes, dataspaces, fill values, filter pipelines, attributes).
//!
//! Not supported: unlimited virtual mappings, huge fractal-heap objects, filtered fractal
//! heaps, opaque data.

mod attr;
mod blosc2;
mod btree2;
mod chunks;
pub mod emd;
mod fheap;
mod filters;
mod gheap;
mod group;
mod io;
mod layout;
mod ohdr;
mod packed;
mod read;
mod types;
mod vds;
#[cfg(any(test, feature = "writer"))]
pub mod writer;

/// The filters' codecs on their own, for readers of other chunked formats (Zarr). Each takes
/// the stored bytes and the most the output may hold, and fails on corrupt input or more.
pub mod codec {
    /// A Blosc (version 1 format) frame: BloscLZ, LZ4, zlib or Zstandard, byte or bit
    /// shuffled.
    pub fn blosc(src: &[u8], max: usize) -> crate::Result<Vec<u8>> {
        crate::filters::blosc(src, max)
    }

    /// A Zstandard frame.
    pub fn zstd(src: &[u8], max: usize) -> crate::Result<Vec<u8>> {
        crate::filters::zstd(src, max)
    }

    /// One raw LZ4 block that decodes to exactly `n` bytes.
    pub fn lz4_block(src: &[u8], n: usize) -> crate::Result<Vec<u8>> {
        let mut out = Vec::new();
        crate::filters::lz4_block(src, n, &mut out)?;
        Ok(out)
    }
}

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

pub use attr::{Attr, Value};
pub use read::{Plane, PlaneSpec};
pub use types::{Dataspace, Dtype, Field, Num, Sample, convert, f16_to_f32};
pub use vds::Resolve;

use io::Ctx;
use layout::{Fill, Filter, Layout};

/// The HDF5 signature.
pub const SIGNATURE: [u8; 8] = *b"\x89HDF\r\n\x1a\n";

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("not an HDF5 file")]
    NotHdf5,
    #[error("malformed HDF5 file: {0}")]
    Malformed(String),
    #[error("unsupported HDF5 feature: {0}")]
    Unsupported(String),
    #[error("HDF5 structure too large")]
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A group still to walk: its path, header address, depth and symbol-table hint.
type WalkItem = (String, u64, usize, Option<(u64, u64)>);

/// Caps that keep a hostile file from exhausting memory or time.
pub(crate) mod limits {
    /// Objects (groups and datasets) collected by the walk.
    pub const MAX_OBJECTS: usize = 100_000;
    /// Depth of the group graph.
    pub const MAX_DEPTH: usize = 64;
    /// Links in one group.
    pub const MAX_LINKS: usize = 1_000_000;
    /// Messages in one object header (over all its continuation blocks).
    pub const MAX_MESSAGES: usize = 65_536;
    /// Continuation blocks of one object header.
    pub const MAX_CONTINUATIONS: usize = 4096;
    /// Attributes of one object.
    pub const MAX_ATTRS: usize = 4096;
    /// Elements decoded for one attribute.
    pub const MAX_ATTR_ELEMS: u64 = 1 << 16;
    /// Chunks in one dataset's index.
    pub const MAX_CHUNKS: u64 = 4 << 20;
    /// Bytes of one decoded chunk.
    pub const MAX_CHUNK_BYTES: u64 = 1 << 30;
    /// Nodes visited in one tree or heap walk.
    pub const MAX_NODES: usize = 1 << 20;
    /// Bytes of one read box.
    pub const MAX_READ_BYTES: u64 = 2 << 30;
    /// Bytes of decoded attributes over the whole file (later ones are left out).
    pub const MAX_ATTR_BYTES: usize = 256 << 20;
    /// Bytes of one link, attribute or member name (longer names are cut).
    pub const MAX_NAME_BYTES: usize = 1 << 16;
    /// Links followed over the whole walk.
    pub const MAX_WALK_LINKS: usize = 4 << 20;
    /// Bytes of the paths built over the whole walk.
    pub const MAX_PATH_BYTES: usize = 256 << 20;
}

/// Whether `head` starts with the HDF5 signature.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(&SIGNATURE)
}

/// The position of the superblock: the signature at 0, 512, 1024, 2048, ... (after a user
/// block).
pub fn find_superblock(data: &[u8]) -> Option<usize> {
    let mut p = 0usize;
    while p + 8 <= data.len() {
        if data[p..p + 8] == SIGNATURE {
            return Some(p);
        }
        p = if p == 0 { 512 } else { p.checked_mul(2)? };
    }
    None
}

/// What an object is.
#[derive(Debug)]
pub enum Kind {
    Group,
    Dataset(Box<Dataset>),
    /// A committed (named) datatype.
    Datatype,
    /// A soft link whose target is not in the file (dangling).
    Link(String),
    /// An external link: an object at `path` in the HDF5 file `file` (as the link names it,
    /// usually relative to this file's folder).
    External {
        file: String,
        path: String,
    },
}

/// One group, dataset or committed datatype, reached by its first path.
#[derive(Debug)]
pub struct Object {
    /// Absolute path, "/" for the root group.
    pub path: String,
    /// Address of the object header (unique per object).
    pub addr: u64,
    pub kind: Kind,
    pub attrs: Vec<Attr>,
}

impl Object {
    pub fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs.iter().find(|a| a.name == name).map(|a| &a.value)
    }

    pub fn dataset(&self) -> Option<&Dataset> {
        match &self.kind {
            Kind::Dataset(d) => Some(d),
            _ => None,
        }
    }

    /// The last path component.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or("")
    }
}

/// A dataset's type, shape and storage.
#[derive(Debug)]
pub struct Dataset {
    pub dtype: Dtype,
    pub space: Dataspace,
    pub(crate) layout: Layout,
    pub(crate) filters: Vec<Filter>,
    pub(crate) fill: Fill,
    pub(crate) index: OnceLock<std::result::Result<chunks::Index, Error>>,
    /// The mappings of a virtual dataset.
    pub(crate) vds: Option<std::result::Result<Vec<vds::Mapping>, Error>>,
    /// For the view of one compound field: (bytes per stored element, the field's offset).
    pub(crate) field: Option<(usize, usize)>,
    /// The numeric fields of a compound type.
    pub(crate) members: Vec<Field>,
}

impl Dataset {
    /// The shape, slowest dimension first.
    pub fn shape(&self) -> &[u64] {
        &self.space.dims
    }

    /// Chunk shape, when chunked.
    pub fn chunk_shape(&self) -> Option<&[u64]> {
        match &self.layout {
            Layout::Chunked { dims, .. } => Some(dims),
            _ => None,
        }
    }

    /// Short names of the filters, in pipeline order.
    pub fn filter_names(&self) -> Vec<&'static str> {
        self.filters.iter().map(|f| filters::name(f.id)).collect()
    }

    /// Storage kind for the info panel: "contiguous", "compact" or "chunked".
    pub fn storage(&self) -> &'static str {
        match self.layout {
            Layout::Compact { .. } => "compact",
            Layout::Contiguous { .. } => "contiguous",
            Layout::Chunked { .. } => "chunked",
            Layout::Virtual { .. } => "virtual",
        }
    }

    /// The numeric fields of a compound dataset, each readable as a dataset of its own
    /// ([`File::fields`]).
    pub fn fields(&self) -> &[Field] {
        &self.members
    }

    /// For the view of a compound field: the field's name is the last part of its path.
    pub fn is_field(&self) -> bool {
        self.field.is_some()
    }

    /// The source files a virtual dataset maps from, as named (this file's not included).
    pub fn virtual_files(&self) -> Vec<&str> {
        let mut v: Vec<&str> = match &self.vds {
            Some(Ok(m)) => m.iter().filter_map(|m| m.file.as_deref()).collect(),
            _ => Vec::new(),
        };
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Bytes per stored element (the whole compound element for a field view).
    pub(crate) fn elem_size(&self, c: &io::Ctx) -> usize {
        match self.field {
            Some((record, _)) => record,
            None => self.dtype.size(c),
        }
    }
}

/// An opened HDF5 file's metadata.
pub struct File {
    pub(crate) ctx: Ctx,
    /// The superblock version.
    pub superblock_version: u8,
    pub objects: Vec<Object>,
    /// The numeric fields of compound datasets, as datasets "path[field]"; their indices
    /// follow those of [`File::objects`].
    pub field_objects: Vec<Object>,
    by_path: HashMap<String, usize>,
    fields_by_path: HashMap<String, usize>,
    by_addr: HashMap<u64, usize>,
    /// Problems met during the walk that did not stop it.
    pub warnings: Vec<String>,
    pub(crate) cache: Mutex<read::ChunkCache>,
    /// The shared-message (SOHM) indexes.
    pub(crate) sohm: Vec<ohdr::SohmIndex>,
}

impl std::fmt::Debug for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("File")
            .field("superblock_version", &self.superblock_version)
            .field("objects", &self.objects.len())
            .finish()
    }
}

impl File {
    /// Reads the superblock and walks the group graph.
    pub fn open(data: &[u8]) -> Result<File> {
        Self::open_with(data, &|| false)
    }

    /// [`File::open`] that stops with [`Error::Cancelled`] when `cancel` returns true.
    pub fn open_with(data: &[u8], cancel: &(dyn Fn() -> bool + Sync)) -> Result<File> {
        let sb = ohdr::superblock(data)?;
        let mut file = File {
            ctx: sb.ctx,
            superblock_version: sb.version,
            objects: Vec::new(),
            field_objects: Vec::new(),
            by_path: HashMap::new(),
            fields_by_path: HashMap::new(),
            by_addr: HashMap::new(),
            warnings: Vec::new(),
            cache: Mutex::new(read::ChunkCache::new(256 << 20)),
            sohm: Vec::new(),
        };
        if let Some(ext) = sb.ext {
            match ohdr::sohm_table(data, &file.ctx, ext) {
                Ok(t) => file.sohm = t,
                Err(e) => file.warnings.push(format!("shared message table: {e}")),
            }
        }
        file.walk(data, sb.root, sb.root_symtab, cancel)?;
        file.add_fields();
        Ok(file)
    }

    fn walk(
        &mut self,
        data: &[u8],
        root: u64,
        root_symtab: Option<(u64, u64)>,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<()> {
        let c = self.ctx;
        let mut visited: HashSet<u64> = HashSet::new();
        let mut soft: Vec<(String, String)> = Vec::new();
        let mut shared = ohdr::SharedTargets::with_sohm(self.sohm.clone());
        // Links and path bytes over the whole walk: groups sharing one large link table
        // would otherwise build a path per link per group.
        let (mut links_seen, mut path_bytes) = (0usize, 0usize);
        let mut attr_budget = limits::MAX_ATTR_BYTES;
        // Depth-first, children in name order.
        let mut stack: Vec<WalkItem> = vec![("/".to_string(), root, 0, root_symtab)];
        while let Some((path, addr, depth, symtab_hint)) = stack.pop() {
            if cancel() {
                return Err(Error::Cancelled);
            }
            if self.objects.len() >= limits::MAX_OBJECTS {
                self.warnings
                    .push(format!("stopped after {} objects", limits::MAX_OBJECTS));
                break;
            }
            if !visited.insert(addr) {
                // A second link to an object already listed (or a cycle).
                continue;
            }
            let header = match ohdr::read_header(data, &c, addr, &mut shared) {
                Ok(h) => h,
                Err(e) => {
                    if addr == root {
                        return Err(e);
                    }
                    self.warnings.push(format!("{path}: {e}"));
                    continue;
                }
            };
            let attrs = match attr::read_all(data, &c, &header, &mut shared, &mut attr_budget) {
                Ok(a) => a,
                Err(e) => {
                    self.warnings.push(format!("{path}: attributes: {e}"));
                    Vec::new()
                }
            };
            let kind = if header.find(ohdr::MSG_LAYOUT).is_some() {
                match layout::dataset(&c, &header) {
                    Ok(mut d) => {
                        if let Layout::Virtual { heap } = d.layout {
                            d.vds = Some(match heap {
                                Some((a, i)) => vds::parse(data, &c, a, i),
                                None => Ok(Vec::new()),
                            });
                            if let Some(Err(e)) = &d.vds {
                                self.warnings.push(format!("{path}: {e}"));
                            }
                        }
                        Kind::Dataset(Box::new(d))
                    }
                    Err(e) => {
                        self.warnings.push(format!("{path}: {e}"));
                        continue;
                    }
                }
            } else if header.find(ohdr::MSG_DATATYPE).is_some()
                && header.find(ohdr::MSG_DATASPACE).is_none()
            {
                Kind::Datatype
            } else {
                Kind::Group
            };
            let is_group = matches!(kind, Kind::Group);
            let idx = self.objects.len();
            self.by_path.insert(path.clone(), idx);
            self.by_addr.insert(addr, idx);
            self.objects.push(Object {
                path: path.clone(),
                addr,
                kind,
                attrs,
            });
            if !is_group {
                continue;
            }
            if depth >= limits::MAX_DEPTH {
                self.warnings
                    .push(format!("{path}: groups nested too deeply"));
                continue;
            }
            let links = match group::links(data, &c, &header, symtab_hint) {
                Ok(l) => l,
                Err(e) => {
                    self.warnings.push(format!("{path}: links: {e}"));
                    continue;
                }
            };
            let base = if path == "/" { String::new() } else { path };
            for link in links.into_iter().rev() {
                if let group::Target::Hard(a, _) = link.target
                    && visited.contains(&a)
                {
                    continue;
                }
                links_seen += 1;
                path_bytes += base.len() + 1 + link.name.len();
                if links_seen > limits::MAX_WALK_LINKS || path_bytes > limits::MAX_PATH_BYTES {
                    self.warnings.push("stopped: too many links".into());
                    stack.clear();
                    break;
                }
                let child = format!("{base}/{}", link.name);
                match link.target {
                    group::Target::Hard(a, hint) => stack.push((child, a, depth + 1, hint)),
                    group::Target::Soft(t) => {
                        let t = if t.starts_with('/') {
                            t
                        } else {
                            format!("{base}/{t}")
                        };
                        soft.push((child, t));
                    }
                    group::Target::External { file, path } => {
                        self.push_link(child, Kind::External { file, path })
                    }
                }
            }
        }
        // Soft links name their target by path: an alias of the object there, once every
        // object is known (chains of soft links resolve in a few rounds).
        for _ in 0..8 {
            let before = soft.len();
            soft.retain(|(child, target)| match self.by_path.get(target.as_str()) {
                Some(&i) => {
                    self.by_path.entry(child.clone()).or_insert(i);
                    false
                }
                None => true,
            });
            if soft.is_empty() || soft.len() == before {
                break;
            }
        }
        for (child, target) in soft {
            self.push_link(child, Kind::Link(target));
        }
        Ok(())
    }

    /// A dataset per numeric field of every compound dataset.
    fn add_fields(&mut self) {
        let mut out = Vec::new();
        for o in &self.objects {
            let Some(ds) = o
                .dataset()
                .filter(|d| !matches!(d.layout, Layout::Virtual { .. }))
            else {
                continue;
            };
            let record = ds.dtype.size(&self.ctx);
            for f in &ds.members {
                if self.objects.len() + out.len() >= limits::MAX_OBJECTS {
                    break;
                }
                out.push(Object {
                    path: format!("{}[{}]", o.path, f.name),
                    addr: u64::MAX,
                    kind: Kind::Dataset(Box::new(Dataset {
                        dtype: Dtype::Num {
                            num: f.num,
                            be: f.be,
                        },
                        space: ds.space.clone(),
                        layout: ds.layout.clone(),
                        filters: ds.filters.clone(),
                        fill: ds.fill.clone(),
                        index: Default::default(),
                        vds: None,
                        field: Some((record, f.offset)),
                        members: Vec::new(),
                    })),
                    attrs: Vec::new(),
                });
            }
        }
        let base = self.objects.len();
        for (k, o) in out.iter().enumerate() {
            self.fields_by_path.insert(o.path.clone(), base + k);
        }
        self.field_objects = out;
    }

    /// The object at an index of [`File::objects`] or, past those, of
    /// [`File::field_objects`].
    pub fn object(&self, index: usize) -> Option<&Object> {
        self.objects.get(index).or_else(|| {
            self.field_objects
                .get(index.checked_sub(self.objects.len())?)
        })
    }

    /// The compound field datasets, with their index (see [`File::object`]).
    pub fn fields(&self) -> impl Iterator<Item = (usize, &Object, &Dataset)> {
        let base = self.objects.len();
        self.field_objects
            .iter()
            .enumerate()
            .filter_map(move |(i, o)| o.dataset().map(|d| (base + i, o, d)))
    }

    fn push_link(&mut self, path: String, kind: Kind) {
        if self.objects.len() < limits::MAX_OBJECTS && !self.by_path.contains_key(&path) {
            self.by_path.insert(path.clone(), self.objects.len());
            self.objects.push(Object {
                path,
                addr: u64::MAX,
                kind,
                attrs: Vec::new(),
            });
        }
    }

    /// The external links, as (path in this file, file, path in that file).
    pub fn external_links(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        self.objects.iter().filter_map(|o| match &o.kind {
            Kind::External { file, path } => Some((o.path.as_str(), file.as_str(), path.as_str())),
            _ => None,
        })
    }

    /// The object at an absolute path.
    pub fn get(&self, path: &str) -> Option<&Object> {
        self.by_path.get(path).map(|&i| &self.objects[i])
    }

    /// Index into [`File::objects`] of the object at an absolute path (or of a compound
    /// field's dataset "path[field]", see [`File::object`]).
    pub fn index_of(&self, path: &str) -> Option<usize> {
        self.by_path
            .get(path)
            .or_else(|| self.fields_by_path.get(path))
            .copied()
    }

    /// The object whose header lies at `addr` (the target of an object reference).
    pub fn at_addr(&self, addr: u64) -> Option<&Object> {
        self.by_addr.get(&addr).map(|&i| &self.objects[i])
    }

    /// Datasets, with their index into [`File::objects`].
    pub fn datasets(&self) -> impl Iterator<Item = (usize, &Object, &Dataset)> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.dataset().map(|d| (i, o, d)))
    }

    /// The direct children of a group path.
    pub fn children<'a>(&'a self, group: &'a str) -> impl Iterator<Item = &'a Object> + 'a {
        let prefix = if group == "/" {
            "/".to_string()
        } else {
            format!("{group}/")
        };
        self.objects.iter().filter(move |o| {
            o.path.len() > prefix.len()
                && o.path.starts_with(&prefix)
                && !o.path[prefix.len()..].contains('/')
        })
    }

    pub(crate) fn dataset_at(&self, index: usize) -> Result<&Dataset> {
        self.object(index)
            .and_then(Object::dataset)
            .ok_or_else(|| Error::Malformed("not a dataset".into()))
    }

    /// Reads one 2D plane of a numeric dataset as real values. Chunks that are missing hold
    /// the fill value; chunks that fail to decode or lie past the end of the file read as NaN
    /// and set [`Plane::truncated`].
    pub fn read_plane<T: Sample>(
        &self,
        data: &[u8],
        index: usize,
        spec: &PlaneSpec,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<Plane<T>> {
        read::plane(
            self,
            data,
            index,
            self.dataset_at(index)?,
            spec,
            cancel,
            &read::none,
        )
    }

    /// [`File::read_plane`] for a virtual dataset whose mappings read from other files:
    /// `resolve` gives each such file (by the name the mapping uses) opened, or `None` when it
    /// cannot be reached (its elements then hold the fill value).
    pub fn read_plane_in<'a, T: Sample>(
        &'a self,
        data: &'a [u8],
        index: usize,
        spec: &PlaneSpec,
        cancel: &(dyn Fn() -> bool + Sync),
        resolve: &Resolve<'a>,
    ) -> Result<Plane<T>> {
        read::plane(
            self,
            data,
            index,
            self.dataset_at(index)?,
            spec,
            cancel,
            resolve,
        )
    }

    /// [`File::read_plane_strided`] with other files for virtual mappings, as in
    /// [`File::read_plane_in`].
    #[allow(clippy::too_many_arguments)]
    pub fn read_plane_strided_in<'a, T: Sample>(
        &'a self,
        data: &'a [u8],
        index: usize,
        spec: &PlaneSpec,
        step: u64,
        budget: u64,
        cancel: &(dyn Fn() -> bool + Sync),
        resolve: &Resolve<'a>,
    ) -> Result<Plane<T>> {
        read::plane_strided(
            self,
            data,
            index,
            self.dataset_at(index)?,
            spec,
            step,
            budget,
            cancel,
            resolve,
        )
    }

    /// Reads every `step`-th sample of one 2D plane along x and y, for a quick look at a big
    /// dataset: `ceil(w / step)` by `ceil(h / step)` values, row-major. Only the chunks that
    /// hold samples are read; when those that need decompressing would decode to more than
    /// `budget` bytes, fewer chunks spread over the plane stand in for the rest, each
    /// squeezed over its neighbours. A single compressed chunk of four budgets or more is refused
    /// as [`Error::TooLarge`].
    pub fn read_plane_strided<T: Sample>(
        &self,
        data: &[u8],
        index: usize,
        spec: &PlaneSpec,
        step: u64,
        budget: u64,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<Plane<T>> {
        read::plane_strided(
            self,
            data,
            index,
            self.dataset_at(index)?,
            spec,
            step,
            budget,
            cancel,
            &read::none,
        )
    }

    /// Reads a whole numeric dataset (at most `max` elements) as real values, row-major.
    pub fn read_all<T: Sample>(&self, data: &[u8], index: usize, max: u64) -> Result<Vec<T>> {
        let ds = self.dataset_at(index)?;
        if ds.space.count() > max {
            return Err(Error::TooLarge);
        }
        read::all(self, data, index, ds, &read::none)
    }

    /// Reads a box of a numeric dataset as real values, row-major over `count`. Chunks that
    /// fail to decode or lie past the end of the file read as NaN and set the flag returned.
    pub fn read_box<T: Sample>(
        &self,
        data: &[u8],
        index: usize,
        start: &[u64],
        count: &[u64],
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<(Vec<T>, bool)> {
        read::typed_box(
            self,
            data,
            index,
            self.dataset_at(index)?,
            start,
            count,
            cancel,
            &read::none,
        )
    }

    /// Reads a box of a dataset as raw stored bytes (any element type), row-major over
    /// `count`. Fails when a chunk cannot be decoded.
    pub fn read_box_bytes(
        &self,
        data: &[u8],
        index: usize,
        start: &[u64],
        count: &[u64],
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u8>> {
        read::box_bytes_in(
            self,
            data,
            index,
            self.dataset_at(index)?,
            start,
            count,
            cancel,
            &read::none,
            0,
        )
    }

    /// Resolves the elements of a variable-length or reference attribute to object paths.
    pub fn ref_path(&self, addr: u64) -> Option<&str> {
        self.at_addr(addr).map(|o| o.path.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_superblock_after_user_block() {
        let mut d = vec![0u8; 2048];
        d[1024..1032].copy_from_slice(&SIGNATURE);
        assert_eq!(find_superblock(&d), Some(1024));
        assert_eq!(find_superblock(&d[..1000]), None);
        assert!(!sniff(&d));
    }

    /// Attributes stop at the file's attribute byte budget (shared and dense attributes can
    /// repeat one large value on every object of a file).
    #[test]
    fn attributes_within_budget() {
        use writer::{AttrValue, Writer};
        let mut w = Writer::new();
        let g = w.group(writer::ROOT, "g");
        for k in 0..10 {
            w.attr(g, &format!("a{k}"), AttrValue::F64(vec![1.0; 1000]));
        }
        let bytes = w.finish();
        let f = File::open(&bytes).unwrap();
        let addr = f.get("/g").unwrap().addr;
        let mut shared = ohdr::SharedTargets::default();
        let h = ohdr::read_header(&bytes, &f.ctx, addr, &mut shared).unwrap();
        let mut budget = 20_000;
        let a = attr::read_all(&bytes, &f.ctx, &h, &mut shared, &mut budget).unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!(budget, 0);
        let mut budget = usize::MAX;
        let a = attr::read_all(&bytes, &f.ctx, &h, &mut shared, &mut budget).unwrap();
        assert_eq!(a.len(), 10);
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(File::open(b"").is_err());
        assert!(File::open(&SIGNATURE).is_err());
        let mut d = SIGNATURE.to_vec();
        d.extend_from_slice(&[0xff; 200]);
        assert!(File::open(&d).is_err());
    }
}
