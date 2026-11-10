//! `specular-medvol`: the volume formats of medical imaging research, read into one
//! [`MedVolume`] with patient geometry, for the same 2D medical mode, MPR and 3D views
//! DICOM series use.
//!
//! * **NIfTI-1 and NIfTI-2** ([`nifti`]): single `.nii` files, gzip-compressed `.nii.gz`,
//!   and `.hdr` / `.img` pairs, either byte order. Voxels u8 to u64, i8 to i64, f32, f64 and
//!   RGB24 / RGBA32; complex and 128-bit types are refused with the reason. `scl_slope` /
//!   `scl_inter`, `cal_min` / `cal_max` as the default window, every dimension past the
//!   third as volumes (time points), extensions skipped. Orientation from the sform when its
//!   code is set, else the qform, else the voxel sizes alone (nibabel's precedence).
//! * **Analyze 7.5** through the same reader: no magic, orientation from the voxel sizes
//!   (and SPM's origin), flagged [`OrientationSource::Analyze`] as not reliable.
//! * **NRRD** ([`nrrd`]): attached `.nrrd` and detached `.nhdr` headers, `raw`, `gzip`,
//!   `ascii` and `hex` encodings (`bzip2` is refused: there is no bzip2 decoder in the tree),
//!   `endian`, `space` / `space directions` / `space origin` or `spacings`, `kinds`, and axes
//!   that are not spatial (components, time).
//! * **MetaImage** ([`meta`]): `.mha` and `.mhd` + raw, `ElementType`, `ElementSpacing`,
//!   `Offset`, `TransformMatrix`, `AnatomicalOrientation`, `ElementNumberOfChannels`,
//!   `CompressedData` (zlib).
//! * **FreeSurfer MGH** ([`mgh`]): `.mgh` and gzip-compressed `.mgz`, big-endian uchar,
//!   int, float and short voxels (and FreeSurfer's later ushort), frames as volumes, the
//!   vox2ras from the header's direction cosines, voxel sizes and centre, and the scan
//!   parameters and text tags after the data.
//! * **MINC2 and MINC1** ([`minc`]): `.mnc` files in HDF5 (`/minc-2.0/`) or NetCDF classic,
//!   any `dimorder`, `step` / `start` / `direction_cosines` per spatial dimension, `time`
//!   and other axes as volumes, and `image-min` / `image-max` slice scaling.
//! * **MRtrix images** ([`mif`]): `.mif`, `.mif.gz` and `.mih` with a separate data file,
//!   any `layout` (permuted and reversed axes), every integer and float type in either byte
//!   order, `transform`, `scaling`, and axes past the third as volumes.
//! * **Philips PAR/REC** ([`parrec`]): `.PAR` versions 4 to 4.2 with their `.REC` voxels,
//!   per-image scaling to displayed values, volumes from repeated slice numbers, the scanner
//!   frame from the angulation and off-centre.
//! * **ECAT 7** ([`ecat`]): PET images, frames as volumes, per-frame scale factors times the
//!   calibration factor; placement as nibabel reads it (not a stated patient orientation).
//!
//! Industrial CT and NDT volumes come through the same type, in the scanned object's frame
//! ([`OrientationSource::Object`]: columns and rows as the vendor shows them, slices in
//! stored order; no patient directions):
//!
//! * **Zeiss Xradia TXM / TXRM / XRM** ([`txm`]): compound files of images and facts.
//! * **Amira / Avizo** ([`amira`]): `.am` lattices, binary in either byte order or ASCII,
//!   raw, `HxByteRLE` or `HxZip`.
//! * **Bruker SkyScan** ([`skyscan`]): a reconstruction's slice files opened from its log.
//! * **Nikon X-Tek** ([`xtek`]): `.xtekct` / `.xtekVolume` headers with a raw `.vol`.
//! * **Volume Graphics** ([`vgi`]): `.vgi` headers with a raw volume.
//! * **Headerless `.raw` / `.vol`** ([`raw`]): with a layout given for the file.
//! * **Zeiss Cirrus OCT** ([`cirrus`]): raw `.img` exports, by name and size.
//!
//! CCP4 / MRC density maps and Gaussian cube files ([`grids`]) are read through specular-em
//! as volumes too (for MPR and the 3-D view), in their own frame, lengths in Ångström.
//!
//! Headers are parsed apart from the voxels ([`read_header`], then [`load`]), so a probe or
//! an info panel never inflates a volume. Geometry is in DICOM's patient frame (LPS:
//! x toward the patient's left, y toward the back, z toward the head); NIfTI's RAS and
//! NRRD's spaces are converted. [`MedVolume::canonical`] gives the volume re-ordered (axes
//! permuted and flipped, never resampled) to the nearest DICOM-convention stack of axial
//! slices, as 2D and MPR show it.

pub mod amira;
pub mod cirrus;
mod data;
pub mod ecat;
pub mod grids;
mod ini;
pub mod meta;
pub mod mgh;
pub mod mif;
pub mod minc;
pub mod nifti;
pub mod nrrd;
pub mod parrec;
mod planes;
pub mod raw;
pub mod skyscan;
mod slices;
pub mod txm;
pub mod vgi;
pub mod xtek;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use data::Bytes;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a medical volume")]
    NotRecognised,
    #[error("the file ends early ({0})")]
    Truncated(String),
    #[error("malformed {0}")]
    Malformed(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    Io(String),
    #[error("too large to read")]
    TooLarge,
    /// The memory budget cannot hold the voxels.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    /// A headerless `.raw` / `.vol` file with no layout given ([`raw::remember`]).
    #[error("a headerless volume: its size, sample type and byte order are needed")]
    NeedsLayout,
}

pub type Result<T> = std::result::Result<T, Error>;

/// The file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Nifti1,
    Nifti2,
    Analyze,
    Nrrd,
    MetaImage,
    /// FreeSurfer `.mgh` / `.mgz`.
    Mgh,
    /// MINC2 (`.mnc`, HDF5).
    Minc2,
    /// MINC1 (`.mnc`, NetCDF classic).
    Minc1,
    /// MRtrix `.mif` / `.mif.gz` / `.mih`.
    Mif,
    /// Zeiss Xradia `.txm` / `.txrm` / `.xrm`.
    Txm,
    /// Amira / Avizo `.am`.
    Amira,
    /// Bruker SkyScan reconstruction (slice files and their `.log`).
    SkyScan,
    /// Nikon X-Tek `.xtekct` / `.xtekVolume` with a `.vol`.
    Xtek,
    /// Volume Graphics `.vgi` with a raw volume.
    Vgi,
    /// A headerless `.raw` / `.vol` read with a given layout.
    Raw,
    /// A CCP4 / MRC density map ([`grids`]).
    Map,
    /// A Gaussian cube file ([`grids`]).
    Cube,
    /// A Zeiss Cirrus OCT raw export ([`cirrus`]).
    Cirrus,
    /// Philips PAR/REC ([`parrec`]).
    ParRec,
    /// ECAT 7 PET images ([`ecat`]).
    Ecat,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Nifti1 => "NIfTI-1",
            Kind::Nifti2 => "NIfTI-2",
            Kind::Analyze => "Analyze 7.5",
            Kind::Nrrd => "NRRD",
            Kind::MetaImage => "MetaImage",
            Kind::Mgh => "FreeSurfer MGH",
            Kind::Minc2 => "MINC2",
            Kind::Minc1 => "MINC1",
            Kind::Mif => "MRtrix image",
            Kind::Txm => "Zeiss Xradia TXM",
            Kind::Amira => "Amira",
            Kind::SkyScan => "Bruker SkyScan",
            Kind::Xtek => "Nikon X-Tek",
            Kind::Vgi => "Volume Graphics VGI",
            Kind::Raw => "Raw volume",
            Kind::Map => "CCP4/MRC map",
            Kind::Cube => "Gaussian cube",
            Kind::Cirrus => "Zeiss Cirrus OCT",
            Kind::ParRec => "Philips PAR/REC",
            Kind::Ecat => "ECAT 7",
        }
    }
}

/// How a stored sample is coded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl DataType {
    /// Bytes per sample.
    pub fn size(self) -> usize {
        match self {
            DataType::U8 | DataType::I8 => 1,
            DataType::U16 | DataType::I16 => 2,
            DataType::U32 | DataType::I32 | DataType::F32 => 4,
            DataType::U64 | DataType::I64 | DataType::F64 => 8,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, DataType::F32 | DataType::F64)
    }

    pub fn is_signed(self) -> bool {
        !matches!(
            self,
            DataType::U8 | DataType::U16 | DataType::U32 | DataType::U64
        )
    }

    /// Bits per sample.
    pub fn bits(self) -> u32 {
        self.size() as u32 * 8
    }

    /// The type's name for the info panel, e.g. "int16", "float32".
    pub fn name(self) -> &'static str {
        match self {
            DataType::U8 => "uint8",
            DataType::I8 => "int8",
            DataType::U16 => "uint16",
            DataType::I16 => "int16",
            DataType::U32 => "uint32",
            DataType::I32 => "int32",
            DataType::U64 => "uint64",
            DataType::I64 => "int64",
            DataType::F32 => "float32",
            DataType::F64 => "float64",
        }
    }
}

/// Where the voxel-to-patient mapping came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrientationSource {
    /// NIfTI sform (`sform_code` > 0).
    Sform,
    /// NIfTI qform (`qform_code` > 0, no sform).
    Qform,
    /// A NIfTI file with neither: voxel sizes only (nibabel's default, x flipped).
    None,
    /// Analyze 7.5: voxel sizes (and SPM's origin) only; the stored orientation is not
    /// reliable.
    Analyze,
    /// NRRD `space directions` in a named anatomical space.
    Space,
    /// NRRD `spacings` (or nothing): axis-aligned, orientation unknown.
    Spacings,
    /// MetaImage `TransformMatrix` (or its default, the identity).
    Transform,
    /// FreeSurfer MGH direction cosines, voxel sizes and centre (`goodRASflag` set).
    Vox2Ras,
    /// FreeSurfer MGH without `goodRASflag`: FreeSurfer's default coronal orientation and
    /// 1 mm voxels, not stated by the file.
    MghDefault,
    /// MINC dimension steps, starts and direction cosines.
    Minc,
    /// An industrial scan: the scanned object's own axes (columns and rows as the vendor
    /// shows them, slices in stored order), with no patient directions.
    Object,
    /// Philips PAR/REC midslice angulation and off-centre, in the scanner's frame.
    ParRec,
    /// ECAT 7 voxel sizes and offsets, flipped by the patient orientation code as nibabel
    /// reads them: not a stated patient orientation.
    Ecat,
}

impl OrientationSource {
    /// A stable key, for the info panel's message selectors.
    pub fn key(self) -> &'static str {
        match self {
            OrientationSource::Sform => "sform",
            OrientationSource::Qform => "qform",
            OrientationSource::None => "none",
            OrientationSource::Analyze => "analyze",
            OrientationSource::Space => "space",
            OrientationSource::Spacings => "spacings",
            OrientationSource::Transform => "transform",
            OrientationSource::Vox2Ras => "vox2ras",
            OrientationSource::MghDefault => "mgh-default",
            OrientationSource::Minc => "minc",
            OrientationSource::Object => "object",
            OrientationSource::ParRec => "parrec",
            OrientationSource::Ecat => "ecat",
        }
    }

    /// Whether the file states where its axes point in the patient.
    pub fn reliable(self) -> bool {
        matches!(
            self,
            OrientationSource::Sform
                | OrientationSource::Qform
                | OrientationSource::Space
                | OrientationSource::Transform
                | OrientationSource::Vox2Ras
                | OrientationSource::Minc
                | OrientationSource::ParRec
        )
    }
}

/// How the voxel data is stored in its file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Raw,
    Gzip,
    Zlib,
    Ascii,
    Hex,
    Bzip2,
    /// Amira's byte run-length coding (`HxByteRLE`).
    ByteRle,
    /// Inside a container format (MINC's HDF5 or NetCDF), read through its own reader.
    Container,
}

impl Encoding {
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Raw => "raw",
            Encoding::Gzip => "gzip",
            Encoding::Zlib => "zlib",
            Encoding::Ascii => "ASCII",
            Encoding::Hex => "hex",
            Encoding::Bzip2 => "bzip2",
            Encoding::ByteRle => "run-length",
            Encoding::Container => "container",
        }
    }
}

/// Where the voxel data starts in its (decoded) file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// `lines` text lines, then `bytes` bytes, from the start.
    Start { lines: usize, bytes: usize },
    /// The data ends the file (NRRD `byte skip: -1`, MetaImage `HeaderSize = -1`).
    FromEnd,
}

/// The file(s) holding the voxel data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataFile {
    /// In the header's own file.
    Attached,
    /// Other files, concatenated, relative to the header's folder (or absolute).
    Detached(Vec<PathBuf>),
}

/// Where and how the voxels are stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataRef {
    pub file: DataFile,
    pub skip: Skip,
    pub encoding: Encoding,
    pub big_endian: bool,
}

/// The sample layout: where voxel (i, j, k) of volume v (and component c) is in the
/// sample sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Voxels along the stored i, j, k axes.
    pub size: [usize; 3],
    /// Samples between neighbours along i, j, k.
    pub stride: [usize; 3],
    /// The axes that are neither spatial nor colour: (size, stride), fastest first.
    pub extra: Vec<(usize, usize)>,
    /// Colour components (3: RGB, 4: RGBA, u8 only): (count, stride).
    pub colour: Option<(usize, usize)>,
    /// Samples in the data.
    pub samples: usize,
}

impl Layout {
    /// Volumes (the product of the extra axes).
    pub fn volumes(&self) -> usize {
        self.extra.iter().map(|e| e.0).product::<usize>().max(1)
    }

    /// The first sample of volume `v`.
    fn volume_base(&self, mut v: usize) -> usize {
        let mut base = 0;
        for &(n, s) in &self.extra {
            base += (v % n) * s;
            v /= n;
        }
        base
    }

    /// A plain layout: i fastest, then j, k, then the extra axes, `comps` interleaved
    /// samples per voxel.
    pub fn dense(size: [usize; 3], extra: &[usize], comps: usize) -> Option<Layout> {
        let mut s = comps;
        let mut stride = [0; 3];
        for a in 0..3 {
            stride[a] = s;
            s = s.checked_mul(size[a])?;
        }
        let mut ex = Vec::new();
        for &n in extra {
            ex.push((n, s));
            s = s.checked_mul(n)?;
        }
        Some(Layout {
            size,
            stride,
            extra: ex,
            colour: None,
            samples: s,
        })
    }
}

/// A fact from a volume header, under a stable key the info panel names.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// "tr", "te", "ti", "flip", "fov" (MGH scan parameters), "field", "pedir", "cmdline",
    /// "xform" (MGH tags), …
    pub key: &'static str,
    pub value: ParamValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    /// A number and its unit symbol ("ms", "°", "mm", "T"; "" for none).
    Number(f64, &'static str),
    Text(String),
}

/// A volume's header: everything but the voxels.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub kind: Kind,
    pub dtype: DataType,
    pub layout: Layout,
    /// Stored voxel (i, j, k) to patient LPS millimetres: rows x, y, z; columns i, j, k, 1.
    pub affine: [[f64; 4]; 3],
    pub orientation: OrientationSource,
    /// Every axis's size, as the file lists them (spatial first for NIfTI and MetaImage).
    pub dims: Vec<usize>,
    /// Millimetres between voxel centres along i, j, k.
    pub spacing: [f64; 3],
    /// Value = stored × slope + intercept.
    pub scale: Option<(f64, f64)>,
    /// The file's display range (NIfTI `cal_min`, `cal_max`), in scaled values.
    pub cal: Option<(f64, f64)>,
    pub description: Option<String>,
    /// What the values mean (NIfTI intent code and name).
    pub intent: Option<String>,
    /// MetaImage `AnatomicalOrientation` as written (informational).
    pub anatomical: Option<String>,
    /// Other facts the file states (MGH scan parameters and tags), for the info panel.
    pub params: Vec<Param>,
    pub data: DataRef,
}

impl Header {
    /// Bytes of voxel data after decoding.
    pub fn data_bytes(&self) -> Option<usize> {
        self.layout.samples.checked_mul(self.dtype.size())
    }

    /// The volume's colour components, when it is RGB(A).
    pub fn is_colour(&self) -> bool {
        self.layout.colour.is_some()
    }

    /// Patient position of stored voxel `v` (i, j, k).
    pub fn to_patient(&self, v: [f64; 3]) -> [f64; 3] {
        let a = &self.affine;
        [0, 1, 2].map(|r| a[r][0] * v[0] + a[r][1] * v[1] + a[r][2] * v[2] + a[r][3])
    }

    /// The stored axes re-ordered to the nearest DICOM-convention stack (from the header
    /// alone: no voxels needed).
    pub fn canonical(&self) -> Canonical {
        Canonical::of(self)
    }

    /// The compression, for describing the file ("gzip", "zlib"…); `None` for raw.
    pub fn compression(&self) -> Option<&'static str> {
        (!matches!(self.data.encoding, Encoding::Raw | Encoding::Container))
            .then(|| self.data.encoding.name())
    }
}

/// Bytes of a header worth reading before parsing ([`read_header`] needs no more from an
/// uncompressed NIfTI file; NRRD and MetaImage headers are read up to their end).
pub const HEAD: usize = 544;

/// Which format `data` (a file's first bytes; more for a `.nii.gz`) is.
pub fn sniff(data: &[u8]) -> Option<Kind> {
    if data.starts_with(b"NRRD000") {
        return Some(Kind::Nrrd);
    }
    if meta::sniff(data) {
        return Some(Kind::MetaImage);
    }
    if mif::sniff(data) {
        return Some(Kind::Mif);
    }
    if let Some(k) = nifti::sniff(data) {
        return Some(k);
    }
    if mgh::sniff(data) {
        return Some(Kind::Mgh);
    }
    if let Some(k) = minc::sniff(data) {
        return Some(k);
    }
    if amira::sniff(data) {
        return Some(Kind::Amira);
    }
    if skyscan::sniff(data) {
        return Some(Kind::SkyScan);
    }
    if xtek::sniff(data) {
        return Some(Kind::Xtek);
    }
    if vgi::sniff(data) {
        return Some(Kind::Vgi);
    }
    if parrec::sniff(data) {
        return Some(Kind::ParRec);
    }
    if ecat::sniff(data) {
        return Some(Kind::Ecat);
    }
    // Needs the compound file's directory, not only its first bytes.
    txm::sniff(data).then_some(Kind::Txm)
}

/// Parses the header of the file at `path` whose bytes (all, or at least the header) are
/// `data`.
pub fn read_header(data: &[u8], path: &Path) -> Result<Header> {
    match sniff(data) {
        Some(Kind::Nrrd) => nrrd::parse(data),
        Some(Kind::MetaImage) => meta::parse(data),
        Some(Kind::Mgh) => mgh::parse(data),
        Some(Kind::Mif) => mif::parse(data),
        Some(k @ (Kind::Minc2 | Kind::Minc1)) => minc::parse(data, k),
        Some(Kind::Txm) => txm::parse(data, path),
        Some(Kind::Amira) => amira::parse(data),
        Some(Kind::SkyScan) => skyscan::parse(data, path),
        Some(Kind::Xtek) => xtek::parse(data, path),
        Some(Kind::Vgi) => vgi::parse(data, path),
        Some(Kind::ParRec) => parrec::parse(data, path),
        Some(Kind::Ecat) => ecat::parse(data),
        Some(_) => nifti::parse(data, path),
        None if parrec::is_rec_name(path) => parrec::parse(data, path),
        // Any other name with a layout given for it: data chosen as raw samples.
        None if raw::is_raw_name(path) || raw::layout_of(path).is_some() => raw_header(path),
        None => match cirrus::scan(path, data.len() as u64) {
            Some(s) => cirrus::header(&s),
            None => Err(Error::NotRecognised),
        },
    }
}

/// A headerless `.raw` / `.vol`: with the layout given for it, else through the header
/// beside it whose data it is.
fn raw_header(path: &Path) -> Result<Header> {
    if let Some(l) = raw::layout_of(path) {
        return raw::header(&l);
    }
    if let Some(h) = raw::sibling_header(path) {
        let data = std::fs::read(&h).map_err(|e| Error::Io(e.to_string()))?;
        if sniff(&data).is_some() {
            let mut header = read_header(&data, &h)?;
            // Its data file, named from the header's folder: absolute, so it reads from here.
            if let DataFile::Detached(files) = &mut header.data.file
                && files.len() == 1
            {
                files[0] = beside(&h, &files[0].to_string_lossy());
                return Ok(header);
            }
        }
    }
    Err(Error::NeedsLayout)
}

/// Reads the voxels `header` describes, from `file` (the header's own file, all of it) or
/// the files it names next to `path`.
pub fn load(mut header: Header, file: Bytes, path: &Path) -> Result<MedVolume> {
    match header.kind {
        Kind::Minc2 | Kind::Minc1 => return minc::load(header, (*file).as_ref()),
        Kind::Txm => return txm::load(header, (*file).as_ref()),
        Kind::SkyScan => return skyscan::load(header),
        Kind::ParRec => return parrec::load(header, (*file).as_ref(), path),
        Kind::Ecat => return ecat::load(header, (*file).as_ref()),
        _ => {}
    }
    let (bytes, offset, big_endian) = data::read(&header, file, path)?;
    // A compressed MGH's scan parameters and tags follow the voxels: read them now.
    if header.kind == Kind::Mgh && header.params.is_empty() {
        let end = offset.saturating_add(header.data_bytes().unwrap_or(0));
        header.params = mgh::footer((*bytes).as_ref().get(end..).unwrap_or(&[]));
    }
    Ok(MedVolume {
        header,
        bytes,
        offset,
        big_endian,
    })
}

/// Bytes of a file start [`grids::is_grid`] looks at.
const GRID_HEAD: usize = 64 << 10;

/// Reads the file at `path`: its header, then its voxels.
pub fn open_path(path: &Path) -> Result<MedVolume> {
    let data = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
    // Maps and cubes first: an MRC header can pass for Analyze's (both start with sizes).
    if grids::is_grid(&data[..data.len().min(GRID_HEAD)]) {
        return grids::open(data);
    }
    let header = read_header(&data, path)?;
    load(header, Arc::new(data), path)
}

/// A volume with its voxels.
#[derive(Clone)]
pub struct MedVolume {
    pub header: Header,
    bytes: Bytes,
    offset: usize,
    big_endian: bool,
}

impl MedVolume {
    /// A volume whose voxels are already decoded: `bytes` holds `header.layout`'s samples of
    /// `header.dtype` from `offset`, in the byte order `big_endian` says (a volume made by
    /// another reader, such as a CIFTI file's subcortical voxels).
    pub fn from_bytes(
        header: Header,
        bytes: Bytes,
        offset: usize,
        big_endian: bool,
    ) -> Result<MedVolume> {
        let need = header
            .data_bytes()
            .and_then(|n| n.checked_add(offset))
            .ok_or(Error::TooLarge)?;
        if (*bytes).as_ref().len() < need {
            return Err(Error::Truncated("voxel data".into()));
        }
        Ok(MedVolume {
            header,
            bytes,
            offset,
            big_endian,
        })
    }
}

impl std::fmt::Debug for MedVolume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MedVolume")
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

/// One slice in stored values.
#[derive(Clone, Debug, PartialEq)]
pub enum Plane {
    U8(Vec<u8>),
    I8(Vec<i8>),
    U16(Vec<u16>),
    I16(Vec<i16>),
    U32(Vec<u32>),
    I32(Vec<i32>),
    U64(Vec<u64>),
    I64(Vec<i64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    /// Interleaved RGB (an RGBA volume's alpha left out).
    Rgb8(Vec<u8>),
}

impl MedVolume {
    /// Bytes the voxels take in memory (0 when they are the file's own mapped bytes is not
    /// knowable here: the whole buffer is counted).
    pub fn memory_bytes(&self) -> usize {
        (*self.bytes).as_ref().len()
    }

    /// Volumes (time points).
    pub fn volumes(&self) -> usize {
        self.header.layout.volumes()
    }

    #[inline]
    fn sample(&self, s: usize) -> f64 {
        let b = (*self.bytes).as_ref();
        data::sample_f64(b, self.offset, s, self.header.dtype, self.big_endian)
    }

    /// The scaled value of stored voxel (i, j, k) in volume `v`, component `c` (0 for grey).
    pub fn value(&self, i: usize, j: usize, k: usize, v: usize, c: usize) -> f64 {
        let l = &self.header.layout;
        let mut s = i * l.stride[0] + j * l.stride[1] + k * l.stride[2] + l.volume_base(v);
        if let Some((_, cs)) = l.colour {
            s += c * cs;
        }
        let x = self.sample(s);
        match self.header.scale {
            Some((m, b)) if l.colour.is_none() => x * m + b,
            _ => x,
        }
    }

    /// The volume re-ordered to the nearest DICOM-convention stack (see [`Canonical`]).
    pub fn canonical(&self) -> Canonical {
        Canonical::of(&self.header)
    }

    /// Slice `k` of volume `v` in the canonical order, in stored values (rows top-down).
    pub fn plane(&self, c: &Canonical, k: usize, v: usize) -> Plane {
        data::plane(self, c, k, v)
    }

    /// Slice `k` of volume `v` in the canonical order as scaled values (grey volumes).
    pub fn plane_f32(&self, c: &Canonical, k: usize, v: usize) -> Vec<f32> {
        data::plane_f32(self, c, k, v)
    }

    /// The default window (centre, width) in scaled values: the file's display range when it
    /// has one, else the 0.5th to 99.5th percentile of the first volume (sampled), else its
    /// whole range.
    pub fn default_window(&self) -> (f64, f64) {
        if let Some((lo, hi)) = self.header.cal {
            return ((lo + hi) / 2.0, hi - lo);
        }
        let (lo, hi) = self.robust_range();
        ((lo + hi) / 2.0, (hi - lo).max(1e-6))
    }

    /// Up to about 2¹⁸ scaled values of the first volume, spread evenly, sorted (NaNs
    /// dropped).
    fn sampled(&self) -> Vec<f64> {
        let l = &self.header.layout;
        let n: usize = l.size.iter().product();
        let step = (n / (1 << 18)).max(1);
        let mut out = Vec::with_capacity(n / step + 1);
        let [w, h, _] = l.size;
        let mut idx = 0;
        while idx < n {
            let (i, j, k) = (idx % w, (idx / w) % h, idx / (w * h));
            let x = self.value(i, j, k, 0, 0);
            if x.is_finite() {
                out.push(x);
            }
            idx += step;
        }
        out.sort_by(f64::total_cmp);
        out
    }

    fn robust_range(&self) -> (f64, f64) {
        let s = self.sampled();
        if s.is_empty() {
            return (0.0, 1.0);
        }
        let at = |q: f64| s[((s.len() - 1) as f64 * q).round() as usize];
        let (lo, hi) = (at(0.005), at(0.995));
        if hi > lo {
            (lo, hi)
        } else if s[s.len() - 1] > s[0] {
            (s[0], s[s.len() - 1])
        } else {
            (s[0] - 0.5, s[0] + 0.5)
        }
    }

    /// The smallest and largest scaled value of the first volume (sampled as
    /// [`MedVolume::default_window`] is), for scaling window drags.
    pub fn value_range(&self) -> (f64, f64) {
        let s = self.sampled();
        match (s.first(), s.last()) {
            (Some(&a), Some(&b)) => (a, b),
            _ => (0.0, 1.0),
        }
    }
}

/// The volume's stored axes permuted and flipped (never resampled) to the stack DICOM
/// convention shows: columns toward the patient's left, rows toward the back, slices toward
/// the head (axial). A single coronal or sagittal slice keeps its plane (columns toward the
/// left or the back, rows toward the feet).
#[derive(Clone, Debug, PartialEq)]
pub struct Canonical {
    /// Columns, rows, slices.
    pub dims: [usize; 3],
    /// Unit directions (LPS) of increasing column and row.
    pub row: [f64; 3],
    pub col: [f64; 3],
    /// mm between columns, rows and slices.
    pub pixel: [f64; 3],
    /// The centre of voxel (0, 0, 0) and the step from one slice to the next (LPS mm).
    pub origin: [f64; 3],
    pub step: [f64; 3],
    /// The in-plane axes are not perpendicular (a skewed affine): row and col are then only
    /// the nearest perpendicular pair.
    pub skewed: bool,
    /// Stored axis of each canonical axis, and whether it runs backwards.
    pub perm: [usize; 3],
    pub flip: [bool; 3],
}

impl Canonical {
    fn of(h: &Header) -> Canonical {
        let a = &h.affine;
        let column = |c: usize| [a[0][c], a[1][c], a[2][c]];
        let cols = [column(0), column(1), column(2)];
        let size = h.layout.size;
        // A single slice keeps the plane its normal is nearest to.
        let flat: Vec<usize> = (0..3).filter(|&x| size[x] == 1).collect();
        let targets: [[f64; 3]; 3] = match flat.as_slice() {
            [s] => {
                let n = cols[*s];
                let m = (0..3)
                    .max_by(|&p, &q| n[p].abs().total_cmp(&n[q].abs()))
                    .unwrap_or(2);
                match m {
                    0 => [[0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]],
                    1 => [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
                    _ => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                }
            }
            _ => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        };
        let unit = |v: [f64; 3]| {
            let n = norm(v);
            if n > 0.0 { v.map(|x| x / n) } else { v }
        };
        const PERMS: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let score = |p: &[usize; 3]| -> f64 {
            (0..3)
                .map(|t| dot(unit(cols[p[t]]), targets[t]).abs())
                .sum()
        };
        // The best match; the stored order wins ties (an oblique volume stays as stored).
        let mut perm = PERMS[0];
        let mut best = score(&perm) + 1e-9;
        for p in &PERMS[1..] {
            let s = score(p);
            if s > best {
                best = s;
                perm = *p;
            }
        }
        let flip = [0, 1, 2].map(|t| size[perm[t]] > 1 && dot(cols[perm[t]], targets[t]) < 0.0);
        let dir = |t: usize| {
            let c = cols[perm[t]];
            if flip[t] { c.map(|x| -x) } else { c }
        };
        let (d0, d1, d2) = (dir(0), dir(1), dir(2));
        let mut first = [0.0; 3];
        for t in 0..3 {
            if flip[t] {
                first[perm[t]] = (size[perm[t]] - 1) as f64;
            }
        }
        let origin = h.to_patient(first);
        let row = unit(d0);
        // The nearest perpendicular column direction (exact for any affine without skew).
        let c1 = sub(d1, row.map(|x| x * dot(d1, row)));
        let skewed = dot(unit(d1), row).abs() > 1e-4;
        let col = if norm(c1) > 0.0 {
            unit(c1)
        } else {
            unit(cross(d2, row))
        };
        let row = if norm(row) > 0.0 {
            row
        } else {
            unit(cross(col, d2))
        };
        Canonical {
            dims: [size[perm[0]], size[perm[1]], size[perm[2]]],
            row,
            col,
            pixel: [norm(d0), norm(d1), norm(d2)].map(|x| if x > 0.0 { x } else { 1.0 }),
            origin,
            step: d2,
            skewed,
            perm,
            flip,
        }
    }

    /// Patient position (LPS mm) of the centre of canonical voxel (i, j, k).
    pub fn position(&self, i: f64, j: f64, k: f64) -> [f64; 3] {
        [0, 1, 2].map(|c| {
            self.origin[c]
                + self.row[c] * i * self.pixel[0]
                + self.col[c] * j * self.pixel[1]
                + self.step[c] * k
        })
    }

    /// The stored voxel of canonical voxel (i, j, k).
    pub fn stored(&self, v: [usize; 3]) -> [usize; 3] {
        let mut s = [0; 3];
        for t in 0..3 {
            let n = self.dims[t];
            s[self.perm[t]] = if self.flip[t] { n - 1 - v[t] } else { v[t] };
        }
        s
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Text of a fixed-size header field: up to the first NUL, trimmed; `None` when empty.
fn field_text(b: &[u8]) -> Option<String> {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let s: String = b[..end]
        .iter()
        .map(|&c| {
            if c.is_ascii() {
                c as char
            } else {
                char::from(c)
            }
        })
        .filter(|c| !c.is_control())
        .collect();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// Text after `data`'s first line as `key<sep>value` header lines, up to the first empty
/// line (or the end): (key, value, line end offset).
fn text_lines(data: &[u8]) -> impl Iterator<Item = (&str, usize)> {
    let mut at = 0;
    std::iter::from_fn(move || {
        if at >= data.len() {
            return None;
        }
        let end = data[at..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(data.len(), |p| at + p + 1);
        let line = &data[at..end];
        at = end;
        let s = std::str::from_utf8(line).unwrap_or("");
        Some((s.trim_end_matches(['\n', '\r']), end))
    })
}

/// Reads a file a header names: only a regular file (not a device such as `/dev/zero`, which
/// never ends, nor a FIFO, which blocks), and no more than its size when opened.
pub(crate) fn read_named(p: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let io = |e: std::io::Error| {
        Error::Io(format!(
            "{}: {e}",
            p.file_name().unwrap_or(p.as_os_str()).to_string_lossy()
        ))
    };
    let f = std::fs::File::open(p).map_err(io)?;
    let meta = f.metadata().map_err(io)?;
    if !meta.is_file() {
        return Err(io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        )));
    }
    let mut v = Vec::new();
    v.try_reserve_exact(meta.len() as usize)
        .map_err(|_| Error::TooLarge)?;
    f.take(meta.len()).read_to_end(&mut v).map_err(io)?;
    Ok(v)
}

/// A path named in a header, relative to the header's folder.
fn beside(path: &Path, name: &str) -> PathBuf {
    let p = Path::new(name);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        path.parent().unwrap_or(Path::new("")).join(p)
    }
}

/// `n` numbers from whitespace- or comma-separated text.
fn numbers(s: &str) -> Option<Vec<f64>> {
    s.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<f64>().ok())
        .collect()
}
