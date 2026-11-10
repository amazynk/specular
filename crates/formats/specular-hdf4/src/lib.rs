//! Read-only HDF4 (and HDF-EOS2) decoder.
//!
//! An HDF4 file is a table of data descriptors (DDs): every object is an element named by a
//! tag and a reference number. [`File::open`] walks the DD blocks, the Vgroups and Vdatas, and
//! builds the scientific datasets (SD interface: `Var0.0` Vgroups with their dimensions and
//! attributes, or old DFSD numeric data groups) and the raster images (GR interface `RI0.0`
//! Vgroups, RIG raster image groups and the old 8-bit RI8 tags). Element data is resolved
//! lazily and only when read: linked blocks, external files (through a caller-provided reader
//! that decides which files may be opened), compressed elements (deflate, run-length, NBIT,
//! skipping Huffman, SZIP) and chunked elements, whose chunks are decoded one by one and kept in
//! a byte-bounded cache. JPEG raster images are decoded by `specular-jpeg`.
//!
//! HDF-EOS2 structure metadata (`StructMetadata.N`) is parsed by an own ODL parser ([`odl`])
//! into grids and swaths ([`eos`]).
//!
//! Everything else (IMCOMP rasters, old JPEG tags, VAX floats, compressed elements of other
//! models) is refused with [`Error::Unsupported`].

pub mod codec;
mod dd;
mod elem;
pub mod eos;
mod nt;
pub mod odl;
pub mod raster;
pub mod sd;
mod vobj;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use dd::Dds;
pub use nt::Nt;
pub use raster::{Image, ImageData, Palette};
pub use sd::{Dim, Packing, Sds};
pub use specular_hdf5::{Attr, Num, Plane, PlaneSpec, Value};
pub use vobj::{Vdata, VdataField, Vgroup};

/// The HDF4 magic number (`DFMT_MAGIC`).
pub const MAGIC: [u8; 4] = [0x0e, 0x03, 0x13, 0x01];

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("not an HDF4 file")]
    NotHdf4,
    #[error("malformed HDF4 file: {0}")]
    Malformed(String),
    #[error("unsupported HDF4 feature: {0}")]
    Unsupported(String),
    #[error("HDF4 structure too large")]
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad(what: &str) -> Error {
    Error::Malformed(what.to_owned())
}

/// Caps that keep a hostile file from exhausting memory or time.
pub(crate) mod limits {
    /// DD blocks walked.
    pub const MAX_DD_BLOCKS: usize = 1 << 16;
    /// DDs in the whole file.
    pub const MAX_DDS: usize = 4 << 20;
    /// Vgroups and Vdatas parsed.
    pub const MAX_VOBJECTS: usize = 1 << 18;
    /// Members of one Vgroup.
    pub const MAX_VG_MEMBERS: usize = 1 << 16;
    /// Fields of one Vdata.
    pub const MAX_FIELDS: usize = 4096;
    /// Datasets and images collected.
    pub const MAX_ITEMS: usize = 1 << 16;
    /// Attributes of one object.
    pub const MAX_ATTRS: usize = 4096;
    /// Bytes of one decoded element or chunk.
    pub const MAX_ELEMENT: u64 = 2 << 30;
    /// Linked blocks of one element.
    pub const MAX_BLOCKS: usize = 1 << 20;
    /// Chunks of one dataset.
    pub const MAX_CHUNKS: u64 = 4 << 20;
    /// Rank of a dataset.
    pub const MAX_RANK: usize = 32;
    /// Nesting of special elements (a compressed element stored as linked blocks, …).
    pub const MAX_SPECIAL_DEPTH: usize = 4;
    /// Bytes of decoded elements kept in the cache.
    pub const CACHE_BYTES: usize = 512 << 20;
    /// Bytes of structure metadata parsed.
    pub const MAX_ODL: usize = 64 << 20;
}

/// Whether `head` starts with the HDF4 magic number.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(&MAGIC)
}

/// Reads `len` bytes at `offset` of an external file named (as stored) `name`, or `None` when
/// the file may not or cannot be read. The caller decides which names are acceptable.
pub type ExternalReader<'a> = &'a (dyn Fn(&str, u64, u64) -> Option<Vec<u8>> + Sync);

/// The file's bytes and how to reach external elements.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub data: &'a [u8],
    pub external: Option<ExternalReader<'a>>,
}

impl<'a> Ctx<'a> {
    pub fn new(data: &'a [u8]) -> Ctx<'a> {
        Ctx {
            data,
            external: None,
        }
    }
    pub fn with_external(data: &'a [u8], external: ExternalReader<'a>) -> Ctx<'a> {
        Ctx {
            data,
            external: Some(external),
        }
    }
}

/// The library version the file was written with (the VERSION tag).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibVersion {
    pub major: u32,
    pub minor: u32,
    pub release: u32,
    pub text: String,
}

/// A cache key: the element's (tag, ref) and, for chunks, the chunk's index.
pub(crate) type CacheKey = (u16, u16, u64);

/// Decoded elements, least recently used dropped first. Each holds its reservation from the
/// memory budget while it is cached (an element the budget cannot hold is not cached).
#[derive(Default)]
pub(crate) struct Cache {
    used: usize,
    tick: u64,
    map: HashMap<CacheKey, (Arc<Vec<u8>>, u64, specular_core::Reservation)>,
}

impl Cache {
    pub(crate) fn get(&mut self, key: CacheKey) -> Option<Arc<Vec<u8>>> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(&key).map(|e| {
            e.1 = t;
            e.0.clone()
        })
    }

    pub(crate) fn put(&mut self, key: CacheKey, v: Arc<Vec<u8>>) {
        if v.len() > limits::CACHE_BYTES / 2 {
            return;
        }
        let Ok(res) = specular_core::budget::reserve(v.len() as u64) else {
            return;
        };
        self.tick += 1;
        self.used += v.len();
        if let Some(old) = self.map.insert(key, (v, self.tick, res)) {
            self.used -= old.0.len();
        }
        while self.used > limits::CACHE_BYTES {
            let Some((&k, _)) = self.map.iter().min_by_key(|e| e.1.1) else {
                break;
            };
            if let Some(old) = self.map.remove(&k) {
                self.used -= old.0.len();
            }
        }
    }

    fn clear(&mut self) -> usize {
        let n = self.used;
        self.map.clear();
        self.used = 0;
        n
    }
}

/// An opened HDF4 file: its structure, without the element data.
pub struct File {
    pub dds: Dds,
    pub version: Option<LibVersion>,
    pub vgroups: Vec<Vgroup>,
    pub vdatas: Vec<Vdata>,
    /// Scientific datasets in file order (coordinate variables included, flagged).
    pub sds: Vec<Sds>,
    /// File attributes of the SD interface (the `CDF0.0` Vgroup's attributes).
    pub attrs: Vec<Attr>,
    /// Raster images: GR images first, then RIG and RI8 images not duplicating them.
    pub images: Vec<Image>,
    /// File attributes of the GR interface.
    pub image_attrs: Vec<Attr>,
    /// HDF-EOS2 grids and swaths, when the file carries structure metadata.
    pub eos: Option<eos::Eos>,
    /// Structure problems that did not stop the walk.
    pub warnings: Vec<String>,
    pub(crate) cache: Mutex<Cache>,
    pub(crate) vg_index: HashMap<u16, usize>,
    pub(crate) vd_index: HashMap<u16, usize>,
}

impl File {
    pub fn open(data: &[u8]) -> Result<File> {
        File::open_with(data, &|| false)
    }

    pub fn open_with(data: &[u8], cancel: &(dyn Fn() -> bool + Sync)) -> Result<File> {
        if !sniff(data) {
            return Err(Error::NotHdf4);
        }
        let dds = Dds::read(data)?;
        let ctx = Ctx::new(data);
        let mut f = File {
            version: None,
            vgroups: Vec::new(),
            vdatas: Vec::new(),
            sds: Vec::new(),
            attrs: Vec::new(),
            images: Vec::new(),
            image_attrs: Vec::new(),
            eos: None,
            warnings: Vec::new(),
            cache: Mutex::new(Cache::default()),
            vg_index: HashMap::new(),
            vd_index: HashMap::new(),
            dds,
        };
        f.version = f.read_version(data);
        if cancel() {
            return Err(Error::Cancelled);
        }
        vobj::scan(&mut f, ctx, cancel)?;
        if cancel() {
            return Err(Error::Cancelled);
        }
        sd::scan(&mut f, ctx)?;
        raster::scan(&mut f, ctx)?;
        if cancel() {
            return Err(Error::Cancelled);
        }
        f.eos = eos::Eos::from_file(&f);
        Ok(f)
    }

    fn read_version(&self, data: &[u8]) -> Option<LibVersion> {
        let b = self
            .dds
            .raw(data, dd::TAG_VERSION, self.dds.first_ref(dd::TAG_VERSION)?)?;
        if b.len() < 12 {
            return None;
        }
        let u = |k: usize| u32::from_be_bytes([b[k], b[k + 1], b[k + 2], b[k + 3]]);
        let text = String::from_utf8_lossy(&b[12..])
            .trim_end_matches(['\0', ' '])
            .to_owned();
        Some(LibVersion {
            major: u(0),
            minor: u(4),
            release: u(8),
            text,
        })
    }

    /// A file attribute of the SD interface by name.
    pub fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs.iter().find(|a| a.name == name).map(|a| &a.value)
    }

    /// The dataset named `name` (the first, when several share it).
    pub fn sds_by_name(&self, name: &str) -> Option<&Sds> {
        self.sds.iter().find(|s| s.name == name)
    }

    /// Bytes of decoded elements held in the cache.
    pub fn cache_bytes(&self) -> usize {
        self.cache.lock().map(|c| c.used).unwrap_or(0)
    }

    /// Drops the cached elements; returns the bytes freed.
    pub fn clear_cache(&self) -> usize {
        self.cache.lock().map(|mut c| c.clear()).unwrap_or(0)
    }

    /// Every stored value of a dataset, in row-major order, as stored numbers (no scaling;
    /// unwritten parts read as the fill value). At most `max` values.
    pub fn read_sds_all(&self, ctx: Ctx, sds: &Sds, max: u64) -> Result<Vec<f64>> {
        sd::read_all(self, ctx, sds, max)
    }

    /// One 2-D plane of a dataset as stored numbers.
    pub fn read_sds_plane(&self, ctx: Ctx, sds: &Sds, spec: &PlaneSpec) -> Result<Plane<f64>> {
        sd::read_plane(self, ctx, sds, spec)
    }

    /// A raster image's samples.
    pub fn read_image(&self, ctx: Ctx, image: &Image) -> Result<ImageData> {
        raster::read(self, ctx, image)
    }

    /// The records of a Vdata, as stored (packed by field, record after record).
    pub fn vdata_bytes(&self, ctx: Ctx, vd: &Vdata) -> Result<Vec<u8>> {
        vobj::vdata_bytes(self, ctx, vd)
    }
}

/// Text of a character value (Latin-1), trailing NULs dropped.
pub(crate) fn clean_text(b: &[u8]) -> String {
    let end = b.iter().rposition(|&c| c != 0).map_or(0, |k| k + 1);
    b[..end].iter().map(|&c| c as char).collect()
}

pub(crate) fn be16(b: &[u8], k: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(k..k + 2)?.try_into().ok()?))
}

pub(crate) fn be32(b: &[u8], k: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(k..k + 4)?.try_into().ok()?))
}

pub(crate) fn bei32(b: &[u8], k: usize) -> Option<i32> {
    Some(i32::from_be_bytes(b.get(k..k + 4)?.try_into().ok()?))
}

#[cfg(test)]
pub(crate) mod hostile {
    use super::*;

    #[test]
    fn cached_elements_hold_their_reservation() {
        use specular_core::budget::{MemoryBudget, with_budget};
        let b = Arc::new(MemoryBudget::fixed(1 << 20));
        let mut c = Cache::default();
        with_budget(b.clone(), || {
            c.put((1, 1, 0), Arc::new(vec![0; 1000]));
            // Over what the budget has free: not cached.
            c.put((1, 2, 0), Arc::new(vec![0; 2 << 20]));
        });
        assert_eq!(b.reserved(), 1000);
        assert_eq!(c.used, 1000);
        assert!(c.get((1, 2, 0)).is_none());
        // Replaced, then cleared: given back with the buffers.
        with_budget(b.clone(), || c.put((1, 1, 0), Arc::new(vec![0; 10])));
        assert_eq!(b.reserved(), 10);
        c.clear();
        assert_eq!(b.reserved(), 0);
    }

    /// A file of one descriptor: SD element 1, the first 4 bytes of the file.
    pub(crate) fn tiny() -> Vec<u8> {
        let mut d = MAGIC.to_vec();
        d.extend_from_slice(&[0, 1, 0, 0, 0, 0]);
        d.extend_from_slice(&dd::TAG_SD.to_be_bytes());
        d.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 4]);
        d
    }

    #[test]
    fn tiny_file_opens() {
        let d = tiny();
        let f = File::open(&d).unwrap();
        assert_eq!(f.dds.list.len(), 1);
    }

    #[test]
    fn overlapping_dd_blocks() {
        // 4096 DD blocks of 65535 descriptors each, 6 bytes apart: every block rescans the
        // same (mostly empty) descriptors.
        let mut d = MAGIC.to_vec();
        let n = 4096;
        for k in 0..n {
            let next = if k + 1 == n { 0 } else { 4 + 6 * (k + 1) };
            d.extend_from_slice(&[0xff, 0xff]);
            d.extend_from_slice(&(next as u32).to_be_bytes());
        }
        d.resize(d.len() + 65535 * 12, 0);
        let len = d.len();
        if let Ok(dds) = Dds::read(&d) {
            assert!(dds.list.len() <= len / 12);
        }
    }

    #[test]
    fn raster_image_size_overflow() {
        let d = tiny();
        let f = File::open(&d).unwrap();
        let im = Image {
            name: String::new(),
            width: i32::MAX as u32,
            height: 1 << 30,
            ncomp: 2,
            nt: Nt::from_code(21).unwrap(),
            interlace: 0,
            attrs: Vec::new(),
            palette: None,
            gr: true,
            comp: 0,
            data: Some((dd::TAG_SD, 1)),
        };
        assert!(matches!(
            f.read_image(Ctx::new(&d), &im),
            Err(Error::TooLarge)
        ));
    }

    #[test]
    fn vdata_with_huge_record_count() {
        let vd = Vdata {
            rf: 1,
            name: String::new(),
            class: String::new(),
            interlace: 0,
            nrecords: i32::MAX as u32,
            fields: vec![VdataField {
                name: String::new(),
                nt: Some(Nt::from_code(21).unwrap()),
                order: 65535,
                size: 65535,
            }],
            attrs: Vec::new(),
        };
        assert_eq!(vd.field_values(&[0; 4], 0), None);
    }

    #[test]
    fn dimension_map_with_minimal_increment() {
        let m = eos::DimMap {
            geo: String::new(),
            data: String::new(),
            offset: 0,
            increment: i64::MIN,
        };
        assert!(m.geo_index(1.0).is_finite());
    }
}
