//! Medical research volumes (NIfTI-1/2, Analyze 7.5, NRRD, MetaImage, MGH, MINC) and
//! industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, Volume Graphics,
//! headerless `.raw` / `.vol`), Philips PAR/REC and ECAT 7 through `specular-medvol`.
//!
//! A session is one volume shown slice by slice the way DICOM frames are: its stored axes
//! re-ordered (permuted and flipped, never resampled) to the nearest DICOM-convention stack
//! ([`medvol::Canonical`]), every slice of every volume (time point) a page
//! ([`Session::page_count`] = slices × volumes), the middle slice of the first volume shown
//! ([`Session::page_index`]). Slices are [`ModalityFrame`]s: the file's stored values with
//! its scale (`scl_slope` / `scl_inter`) as the modality rescale and its display range
//! (`cal_min` / `cal_max`, else the 0.5th–99.5th percentile) as the window, so medical mode,
//! MPR and the 3D views treat them as they treat DICOM. RGB volumes show as colour. The
//! header is parsed at open; the voxels are read (and inflated) on the first decode and kept
//! until [`Session::trim_memory`].

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_dicom as dcm;
pub use specular_medvol as medvol;

use crate::dicom_dec::raster;
use crate::whole::{self, Levels};
use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, MedicalSummary, ModalityFrame, Result,
    Session,
};

/// What the info panel shows about a research volume.
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeFacts {
    /// "NIfTI-1", "NIfTI-2", "Analyze 7.5", "NRRD", "MetaImage".
    pub format: &'static str,
    /// Every axis's size as the file lists them.
    pub dims: Vec<usize>,
    /// Millimetres between voxel centres along the stored i, j, k axes.
    pub spacing: [f64; 3],
    /// "int16", "float32"…, with "RGB" volumes as "uint8".
    pub data_type: &'static str,
    pub colour: bool,
    /// Where the orientation came from: "sform", "qform", "none", "analyze", "space",
    /// "spacings", "transform" (see [`medvol::OrientationSource::key`]).
    pub orientation: &'static str,
    /// The file states where its axes point in the patient.
    pub reliable: bool,
    pub description: Option<String>,
    /// What the values mean (NIfTI intent).
    pub intent: Option<String>,
    /// "gzip", "zlib", "ASCII", "hex"; `None` for raw.
    pub compression: Option<&'static str>,
    /// Volumes (time points).
    pub volumes: usize,
    /// Other facts the header states (MGH scan parameters and tags).
    pub params: Vec<medvol::Param>,
}

/// What the info panel (and MPR) know about the volume `header` describes.
pub(crate) fn facts_of(header: &medvol::Header) -> VolumeFacts {
    VolumeFacts {
        format: header.kind.name(),
        dims: header.dims.clone(),
        spacing: header.spacing,
        data_type: header.dtype.name(),
        colour: header.is_colour(),
        orientation: header.orientation.key(),
        reliable: header.orientation.reliable(),
        description: header.description.clone(),
        intent: header.intent.clone(),
        compression: header.compression(),
        volumes: header.layout.volumes(),
        params: header.params.clone(),
    }
}

pub struct MedvolDecoder(pub Format);

fn format_of(kind: medvol::Kind) -> Format {
    match kind {
        medvol::Kind::Nifti1 | medvol::Kind::Nifti2 => Format::Nifti,
        medvol::Kind::Analyze => Format::Analyze,
        medvol::Kind::Nrrd => Format::Nrrd,
        medvol::Kind::MetaImage => Format::MetaImage,
        medvol::Kind::Mgh => Format::Mgh,
        medvol::Kind::Minc2 | medvol::Kind::Minc1 => Format::Minc,
        medvol::Kind::Txm => Format::Txm,
        medvol::Kind::Amira => Format::Amira,
        medvol::Kind::SkyScan => Format::SkyScan,
        medvol::Kind::Xtek => Format::Xtek,
        medvol::Kind::Vgi => Format::Vgi,
        medvol::Kind::Raw => Format::RawVolume,
        medvol::Kind::Mif => Format::Mif,
        medvol::Kind::Map => Format::Mrc,
        medvol::Kind::Cube => Format::Cube,
        medvol::Kind::Cirrus => Format::Cirrus,
        medvol::Kind::ParRec => Format::ParRec,
        medvol::Kind::Ecat => Format::Ecat,
    }
}

fn map_err(e: medvol::Error) -> DecodeError {
    match e {
        medvol::Error::NotRecognised => DecodeError::UnsupportedFormat,
        medvol::Error::Truncated(s) => DecodeError::Truncated(s),
        medvol::Error::Malformed(s) => DecodeError::Malformed(s),
        medvol::Error::Unsupported(s) => DecodeError::Unsupported(s),
        medvol::Error::Io(s) => DecodeError::Io(std::io::Error::other(s)),
        medvol::Error::TooLarge => DecodeError::TooLarge,
        medvol::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        // The viewer asks for the layout ([`medvol::raw::needs_layout`]) and opens it again.
        e @ medvol::Error::NeedsLayout => DecodeError::Unsupported(e.to_string()),
    }
}

impl Decoder for MedvolDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        // Headerless volumes have no signature: chosen by name.
        !matches!(self.0, Format::RawVolume | Format::Cirrus)
            && !head.is_empty()
            && medvol::sniff(head).map(format_of) == Some(self.0)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        !matches!(self.0, Format::RawVolume | Format::Cirrus)
            && !data.is_empty()
            && medvol::sniff(data).map(format_of) == Some(self.0)
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Nifti => &["nii", "nii.gz"],
            Format::Nrrd => &["nrrd", "nhdr"],
            Format::MetaImage => &["mha", "mhd"],
            Format::Mgh => &["mgh", "mgz"],
            Format::Minc => &["mnc"],
            Format::Txm => &["txm", "txrm", "xrm"],
            Format::Amira => &["am"],
            Format::Xtek => &["xtekct", "xtekvolume"],
            Format::Vgi => &["vgi"],
            Format::Mif => &["mif", "mif.gz", "mih"],
            // A `.REC` has no signature: chosen by name, read through the PAR beside it.
            // ECAT's `.v` is Verilog's too: recognised by content only.
            Format::ParRec => &["par", "rec"],
            // A SkyScan reconstruction's `.log` is recognised by content (see
            // [`crate::sniff::is_skyscan_log`]); `.raw` / `.vol` files that are another
            // header's data are left out of folders ([`crate::sniff::is_volume_sidecar`]).
            Format::RawVolume => &["raw", "vol"],
            // `.hdr` is Radiance's: Analyze and NIfTI pairs are recognised by content.
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_volume(source)
    }
}

/// The source's bytes as the voxel store (mapped files are read in place).
struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

fn open_volume(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let header = medvol::read_header(source.bytes(), source.path()).map_err(map_err)?;
    open_with(source, header, None, None)
}

/// A session over a volume another reader decoded (a CIFTI file's subcortical voxels):
/// `volume` is kept for the session's life (trimming memory does not drop it), `neuro` is
/// what the info panel says about the file.
pub(crate) fn open_prepared(
    source: Arc<Source>,
    volume: medvol::MedVolume,
    format: Format,
    neuro: crate::neuro_dec::NeuroFacts,
) -> Result<Arc<dyn Session>> {
    let header = volume.header.clone();
    open_with(source, header, Some((volume, format)), Some(neuro))
}

fn open_with(
    source: Arc<Source>,
    header: medvol::Header,
    prepared: Option<(medvol::MedVolume, Format)>,
    neuro: Option<crate::neuro_dec::NeuroFacts>,
) -> Result<Arc<dyn Session>> {
    let canon = header.canonical();
    let [w, h, nz] = canon.dims;
    if w == 0 || h == 0 || nz == 0 {
        return Err(DecodeError::Malformed("empty volume".into()));
    }
    let (w32, h32) = (
        u32::try_from(w).map_err(|_| DecodeError::TooLarge)?,
        u32::try_from(h).map_err(|_| DecodeError::TooLarge)?,
    );
    let volumes = header.layout.volumes();
    let pages = nz
        .checked_mul(volumes)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(DecodeError::TooLarge)?;
    let colour = header.is_colour();
    let format = prepared
        .as_ref()
        .map_or_else(|| format_of(header.kind), |p| p.1);
    let mut coding = header.dtype.name().to_owned();
    if colour {
        coding = "RGB".into();
    }
    if let Some(c) = header.compression() {
        coding += ", ";
        coding += c;
    }
    let info = ImageInfo {
        format,
        width: w32,
        height: h32,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: header.dtype.bits().min(64) as u8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    let summary = MedicalSummary {
        series_description: header.description.clone(),
        // Pixel Spacing's order: between rows, between columns.
        pixel_spacing: Some((canon.pixel[1], canon.pixel[0])),
        slice_thickness: (nz > 1).then_some(canon.pixel[2]),
        columns: w32,
        rows: h32,
        frames: pages,
        bits_stored: header.dtype.bits() as u16,
        photometric: if colour { "RGB" } else { "MONOCHROME2" }.into(),
        transfer_syntax: header.kind.name().into(),
        ..Default::default()
    };
    let facts = facts_of(&header);
    // Raw voxels in the file itself are read in place from the mapping.
    let in_place = prepared.is_none()
        && header.data.encoding == medvol::Encoding::Raw
        && header.data.file == medvol::DataFile::Attached;
    let prepared = prepared.map(|(volume, _)| {
        let window = if colour {
            (127.5, 255.0)
        } else {
            volume.default_window()
        };
        Arc::new(Loaded { volume, window })
    });
    Ok(Arc::new(MedvolSession {
        source,
        info,
        header,
        canon,
        summary,
        facts,
        pages,
        page: (nz / 2) as u32,
        colour,
        in_place,
        volume: Mutex::new(prepared.clone()),
        prepared,
        neuro,
        values: Mutex::new(None),
        levels: Levels::new(),
        loaded_facts: std::sync::OnceLock::new(),
    }))
}

/// The voxels with their default window, read once.
struct Loaded {
    volume: medvol::MedVolume,
    window: (f64, f64),
}

struct MedvolSession {
    source: Arc<Source>,
    info: ImageInfo,
    header: medvol::Header,
    canon: medvol::Canonical,
    summary: MedicalSummary,
    facts: VolumeFacts,
    pages: u32,
    /// The page shown: the middle slice of the first volume.
    page: u32,
    colour: bool,
    in_place: bool,
    volume: Mutex<Option<Arc<Loaded>>>,
    /// A volume another reader decoded: it cannot be read again from the source.
    prepared: Option<Arc<Loaded>>,
    /// What the info panel says about a CIFTI file shown as its voxels.
    neuro: Option<crate::neuro_dec::NeuroFacts>,
    /// The shown page's frame, kept after the first decode.
    values: Mutex<Option<Arc<ModalityFrame>>>,
    levels: Levels,
    /// The facts again once the voxels are read, when reading them told more (a `.mgz`'s
    /// scan parameters and tags follow its voxels).
    loaded_facts: std::sync::OnceLock<VolumeFacts>,
}

/// Slice `k` of volume `v` (canonical order) as a frame with the volume's scale and window
/// (`window`: centre, width), for medical mode and MPR.
pub fn research_frame(
    vol: &medvol::MedVolume,
    c: &medvol::Canonical,
    k: usize,
    v: usize,
    window: (f64, f64),
) -> ModalityFrame {
    use dcm::{Grey, Pixels};
    let h = &vol.header;
    let (pixels, bits, signed) = match vol.plane(c, k, v) {
        medvol::Plane::U8(p) => (
            Pixels::Grey(Grey::U16(p.into_iter().map(u16::from).collect())),
            8,
            false,
        ),
        medvol::Plane::I8(p) => (
            Pixels::Grey(Grey::I16(p.into_iter().map(i16::from).collect())),
            8,
            true,
        ),
        medvol::Plane::U16(p) => (Pixels::Grey(Grey::U16(p)), 16, false),
        medvol::Plane::I16(p) => (Pixels::Grey(Grey::I16(p)), 16, true),
        medvol::Plane::I32(p) => (Pixels::Grey(Grey::I32(p)), 32, true),
        medvol::Plane::U32(p) => (
            Pixels::Grey(Grey::F64(p.into_iter().map(f64::from).collect())),
            64,
            false,
        ),
        medvol::Plane::U64(p) => (
            Pixels::Grey(Grey::F64(p.into_iter().map(|x| x as f64).collect())),
            64,
            false,
        ),
        medvol::Plane::I64(p) => (
            Pixels::Grey(Grey::F64(p.into_iter().map(|x| x as f64).collect())),
            64,
            true,
        ),
        medvol::Plane::F32(p) => (Pixels::Grey(Grey::F32(p)), 32, true),
        medvol::Plane::F64(p) => (Pixels::Grey(Grey::F64(p)), 64, true),
        medvol::Plane::Rgb8(p) => (Pixels::Rgb8(p), 8, false),
    };
    let colour = matches!(pixels, Pixels::Rgb8(_));
    let frame = dcm::Frame {
        width: c.dims[0] as u32,
        height: c.dims[1] as u32,
        pixels,
        bits,
        signed,
        truncated: false,
    };
    let (slope, intercept) = h.scale.unwrap_or((1.0, 0.0));
    let windows = if colour { Vec::new() } else { vec![window] };
    let axes = h.orientation.reliable().then_some((c.row, c.col));
    ModalityFrame::from_values(
        frame,
        slope,
        intercept,
        windows,
        Some([c.pixel[0], c.pixel[1]]),
        axes,
    )
}

impl MedvolSession {
    fn loaded(&self) -> Result<Arc<Loaded>> {
        if let Some(l) = &self.prepared {
            return Ok(l.clone());
        }
        let mut g = self.volume.lock();
        if let Some(l) = &*g {
            return Ok(l.clone());
        }
        let t0 = std::time::Instant::now();
        let file: medvol::Bytes = Arc::new(SourceBytes(self.source.clone()));
        let volume =
            medvol::load(self.header.clone(), file, self.source.path()).map_err(map_err)?;
        let window = if self.colour {
            (127.5, 255.0)
        } else {
            volume.default_window()
        };
        specular_log::debug!(
            target: specular_log::DECODE,
            "{}: {} volume read in {:.1} ms",
            self.source.path().display(),
            self.header.kind.name(),
            t0.elapsed().as_secs_f64() * 1e3
        );
        if volume.header.params != self.facts.params && self.loaded_facts.get().is_none() {
            let mut facts = self.facts.clone();
            facts.params = volume.header.params.clone();
            let _ = self.loaded_facts.set(facts);
        }
        let l = Arc::new(Loaded { volume, window });
        *g = Some(l.clone());
        Ok(l)
    }

    /// Page `index` as a frame.
    fn frame_at(&self, index: u32, cancel: &CancelToken) -> Result<ModalityFrame> {
        if index >= self.pages {
            return Err(DecodeError::Malformed("no such slice".into()));
        }
        let l = self.loaded()?;
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let nz = self.canon.dims[2];
        let (k, v) = (index as usize % nz, index as usize / nz);
        Ok(research_frame(&l.volume, &self.canon, k, v, l.window))
    }

    fn values(&self, cancel: &CancelToken) -> Result<Arc<ModalityFrame>> {
        if let Some(v) = &*self.values.lock() {
            return Ok(v.clone());
        }
        let m = Arc::new(self.frame_at(self.page, cancel)?);
        *self.values.lock() = Some(m.clone());
        Ok(m)
    }

    fn volume_memory(&self) -> usize {
        if let Some(l) = &self.prepared {
            return l.volume.memory_bytes();
        }
        if self.in_place {
            return 0;
        }
        self.volume
            .lock()
            .as_ref()
            .map_or(0, |l| l.volume.memory_bytes())
    }
}

impl Session for MedvolSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.colour { "RGB" } else { "Greyscale" })
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let m = self.values(cancel)?;
                let img = m
                    .render_default()
                    .map_err(|e| DecodeError::Malformed(format!("rendering the slice: {e}")))?;
                Ok(raster(img))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let bpp = if self.colour { 4 } else { 2 };
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            m += whole::level_bytes(w, h, 1, bpp + 8);
            if self.volume.lock().is_none() && !self.in_place {
                m += self.header.data_bytes().unwrap_or(0) as u64;
            }
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
            + self.values.lock().as_ref().map_or(0, |v| v.memory_bytes())
            + self.volume_memory()
    }
    /// Drops the cached levels, the shown slice's values and the voxels read.
    fn trim_memory(&self) -> usize {
        let values = self.values.lock().take().map_or(0, |v| v.memory_bytes());
        let volume = if self.prepared.is_some() {
            0
        } else {
            let v = self.volume_memory();
            *self.volume.lock() = None;
            v
        };
        self.levels.clear() + values + volume
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn medical(&self) -> Option<&MedicalSummary> {
        Some(&self.summary)
    }
    fn research_volume(&self) -> Option<&VolumeFacts> {
        Some(self.loaded_facts.get().unwrap_or(&self.facts))
    }
    fn neuro(&self) -> Option<&crate::neuro_dec::NeuroFacts> {
        self.neuro.as_ref()
    }
    fn prepared_volume(&self) -> Option<medvol::MedVolume> {
        self.prepared.as_ref().map(|l| l.volume.clone())
    }
    fn modality_frame(&self, cancel: &CancelToken) -> Option<Result<Arc<ModalityFrame>>> {
        Some(self.values(cancel))
    }
    fn modality_frame_at(
        &self,
        index: u32,
        cancel: &CancelToken,
    ) -> Option<Result<Arc<ModalityFrame>>> {
        if index == self.page {
            return Some(self.values(cancel));
        }
        Some(self.frame_at(index, cancel).map(Arc::new))
    }
}

/// Whether `path` names a research volume by its extension (`.nii`, `.nii.gz`, `.nrrd`,
/// `.nhdr`, `.mha`, `.mhd`, `.mgh`, `.mgz`, `.mnc`, and the industrial CT `.txm`,
/// `.txrm`, `.xrm`, `.am`, `.xtekct`, `.xtekvolume`, `.vgi`, and MRtrix `.mif`, `.mif.gz`,
/// `.mih`); `.hdr` pairs and SkyScan
/// logs are recognised by content only, headerless `.raw` / `.vol` files by their layout.
pub fn is_research_volume_name(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    [
        ".nii",
        ".nii.gz",
        ".nrrd",
        ".nhdr",
        ".mha",
        ".mhd",
        ".mgh",
        ".mgz",
        ".mnc",
        ".txm",
        ".txrm",
        ".xrm",
        ".am",
        ".xtekct",
        ".xtekvolume",
        ".vgi",
        ".mif",
        ".mif.gz",
        ".mih",
    ]
    .iter()
    .any(|e| name.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    fn sample(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-medvol/tests/data")
            .join(name)
    }

    fn open(name: &str) -> Arc<dyn Session> {
        Registry::default()
            .open_path(&sample(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    #[test]
    fn every_kind_opens_by_content() {
        for (name, format) in [
            ("dt_int16.nii", Format::Nifti),
            ("gzipped.nii.gz", Format::Nifti),
            ("nifti2.nii", Format::Nifti),
            ("pair.hdr", Format::Nifti),
            ("analyze.hdr", Format::Analyze),
            ("enc_raw.nrrd", Format::Nrrd),
            ("detached.nhdr", Format::Nrrd),
            ("short.mha", Format::MetaImage),
            ("meta_detached.mhd", Format::MetaImage),
        ] {
            let s = open(name);
            assert_eq!(s.info().format, format, "{name}");
            let data = std::fs::read(sample(name)).unwrap();
            let src = Arc::new(Source::from_vec(sample(name), data));
            assert_eq!(Registry::default().open(src).unwrap().info().format, format);
        }
    }

    /// Philips PAR/REC opens from its header or its data; ECAT 7 by content. Both are pages
    /// of slices by volumes, with their scanner facts.
    #[test]
    fn parrec_and_ecat_open() {
        let c = CancelToken::new();
        for (name, format, pages) in [
            ("par_tra.PAR", Format::ParRec, 6),
            ("par_tra.REC", Format::ParRec, 6),
            // Sagittal: shown as axial slices, 6 of them.
            ("par_sag.PAR", Format::ParRec, 6),
            ("ecat_frames.v", Format::Ecat, 9),
        ] {
            let s = open(name);
            assert_eq!(s.info().format, format, "{name}");
            assert_eq!(s.page_count(), pages, "{name}");
            let m = s.modality_frame(&c).unwrap().unwrap();
            assert!(m.width() > 0, "{name}");
        }
        let f = open("par_tra.PAR").research_volume().unwrap().clone();
        assert_eq!(f.format, "Philips PAR/REC");
        assert_eq!(f.orientation, "parrec");
        assert!(f.reliable);
        let f = open("ecat_i16.v").research_volume().unwrap().clone();
        assert_eq!(f.orientation, "ecat");
        assert!(f.params.iter().any(|p| p.key == "isotope"));
        let r = Registry::default();
        assert!(r.supports_file(&sample("par_tra.PAR")));
        assert!(
            !r.supports_file(&sample("par_tra.REC")),
            "a REC beside its PAR is data"
        );
        assert!(r.supports_file(&sample("ecat_u8.v")));
        let e = r
            .open_path(&sample("ecat_sino.v"))
            .err()
            .unwrap()
            .to_string();
        assert!(e.contains("sinogram"), "{e}");
    }

    /// An `.img` opens by what it is: an Analyze or NIfTI pair's data through its `.hdr`, a
    /// Cirrus export by name and size; another `.img` is refused naming what it is not.
    #[test]
    fn img_files_open_by_content() {
        assert_eq!(open("analyze.img").info().format, Format::Analyze);
        assert_eq!(open("pair.img").info().format, Format::Nifti);
        let (a, b, depth) = (8usize, 3usize, 1024usize);
        let mut data = vec![0u8; a * b * depth];
        for (i, v) in data.iter_mut().enumerate() {
            *v = (i % 251) as u8;
        }
        let name = format!("P1_Macular Cube {a}x{b}_7-25-2012_14-20-23_OD_sn1_cube_raw.img");
        let s = Registry::default()
            .open(Arc::new(Source::from_vec(&name, data)))
            .unwrap();
        assert_eq!(s.info().format, Format::Cirrus);
        let f = s.research_volume().unwrap();
        assert_eq!(f.dims, [a, depth, b]);
        assert_eq!(f.format, "Zeiss Cirrus OCT");
        assert!(f.params.iter().any(|p| p.key == "eye"));
        // A B-scan is A-scans across, depth down: pixel (x, y) of B-scan k is byte
        // y + depth · (x + a · k).
        let c = CancelToken::new();
        let k = s.page_index() as usize;
        let m = s.modality_frame(&c).unwrap().unwrap();
        assert_eq!((m.width(), m.height()), (a as u32, depth as u32));
        let want = |x: usize, y: usize| ((y + depth * (x + a * k)) % 251) as f64;
        assert_eq!(m.value_at(5, 700).unwrap(), want(5, 700));
        // Not every .img is something.
        let disk = Arc::new(Source::from_vec("disk.img", vec![0u8; 4096]));
        match Registry::default().open(disk) {
            Err(DecodeError::NotOneOf {
                ext: "img",
                formats,
            }) => {
                assert!(formats.contains(&"Zeiss Cirrus OCT"))
            }
            Err(e) => panic!("{e}"),
            Ok(_) => panic!("a disk image opened"),
        }
    }

    #[test]
    fn slices_are_pages_and_the_middle_one_shows() {
        let s = open("time.nii");
        let f = s.research_volume().unwrap();
        let nz = f.dims[2] as u32;
        assert!(f.volumes > 1);
        assert_eq!(s.page_count(), nz * f.volumes as u32);
        assert_eq!(s.page_index(), nz / 2);
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((r.width, r.height), (s.info().width, s.info().height));
        let m = s.modality_frame(&c).unwrap().unwrap();
        assert!(m.is_grey());
        // The last page is the last slice of the last volume.
        let last = s
            .modality_frame_at(s.page_count() - 1, &c)
            .unwrap()
            .unwrap();
        assert_eq!(last.width(), m.width());
        assert!(s.modality_frame_at(s.page_count(), &c).unwrap().is_err());
    }

    #[test]
    fn values_window_and_facts() {
        let s = open("scaled.nii");
        let c = CancelToken::new();
        let m = s.modality_frame(&c).unwrap().unwrap();
        // cal_min / cal_max as the window.
        assert_eq!(m.windows(), &[(50.0, 200.0)]);
        let v = specular_medvol::open_path(&sample("scaled.nii")).unwrap();
        let canon = v.canonical();
        let st = canon.stored([1, 2, canon.dims[2] / 2]);
        let want = v.value(st[0], st[1], st[2], 0, 0);
        assert!((m.value_at(1, 2).unwrap() - want).abs() < 1e-9);
        let f = s.research_volume().unwrap();
        assert_eq!(f.format, "NIfTI-1");
        assert_eq!(f.orientation, "sform");
        assert_eq!(f.intent.as_deref(), Some("z-score (activation)"));
        assert!(s.medical().is_some());
        let a = open("analyze.hdr");
        assert!(!a.research_volume().unwrap().reliable);
        // No orientation labels from an Analyze file's unreliable axes.
        assert!(
            a.modality_frame(&c)
                .unwrap()
                .unwrap()
                .patient_axes()
                .is_none()
        );
    }

    #[test]
    fn colour_and_refusals() {
        let s = open("rgb.nii");
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.layout, specular_core::PixelLayout::Rgba);
        assert_eq!(s.colour_model(), Some("RGB"));
        let e = Registry::default().open_path(&sample("complex.nii"));
        assert!(
            matches!(&e, Err(DecodeError::Unsupported(m)) if m.contains("complex")),
            "{:?}",
            e.err()
        );
        // bzip2 voxel data decodes (through specular-xcf's bzip2); a stream cut after its
        // header is an error that names it.
        let s = open("enc_bzip2.nrrd");
        let g = open("enc_gzip.nrrd");
        let (rb, rg) = (
            s.decode(&DecodeRequest::full(1), &c).unwrap(),
            g.decode(&DecodeRequest::full(1), &c).unwrap(),
        );
        assert_eq!(rb.planes[0].data, rg.planes[0].data);
        let e = Registry::default()
            .open_path(&sample("bzip2.nrrd"))
            .and_then(|s| s.decode(&DecodeRequest::full(1), &c));
        assert!(
            matches!(&e, Err(e) if e.to_string().contains("bzip2")),
            "{e:?}"
        );
    }

    #[test]
    fn trims() {
        crate::trim_check::assert_trims(&*open("gzipped.nii.gz"), &DecodeRequest::full(1));
        crate::trim_check::assert_trims(&*open("enc_raw.nrrd"), &DecodeRequest::full(2));
    }

    #[test]
    fn names() {
        assert!(is_research_volume_name(Path::new("/a/T1.NII.GZ")));
        assert!(is_research_volume_name(Path::new("b.nhdr")));
        assert!(!is_research_volume_name(Path::new("c.hdr")));
    }

    /// Open timings on the generated speed volumes (`tools/corpus/fetch-medvol.sh
    /// src_speed`): the header, the first view (middle slice) and the last page.
    #[test]
    #[ignore = "timing: needs SPECULAR_MEDVOL_SPEED (release build)"]
    fn open_times() {
        let Some(dir) = std::env::var_os("SPECULAR_MEDVOL_SPEED") else {
            return;
        };
        let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
        eprintln!("load average: {}", load.trim());
        for name in ["head256_i16.nii.gz", "ct512x300_f32.nrrd"] {
            let path = Path::new(&dir).join(name);
            let c = CancelToken::new();
            for run in 0..3 {
                let t0 = std::time::Instant::now();
                let s = Registry::default().open_path(&path).unwrap();
                let t1 = t0.elapsed();
                s.decode(&DecodeRequest::full(1), &c).unwrap();
                let t2 = t0.elapsed();
                s.modality_frame_at(s.page_count() - 1, &c)
                    .unwrap()
                    .unwrap();
                let t3 = t0.elapsed();
                eprintln!(
                    "{name} run {run}: header {:.1} ms, first view {:.1} ms, last page {:.1} ms",
                    t1.as_secs_f64() * 1e3,
                    t2.as_secs_f64() * 1e3,
                    t3.as_secs_f64() * 1e3
                );
            }
        }
    }

    /// A 4×3×2 int16 FreeSurfer volume in its conformed LIA orientation, 1 × 1.5 × 2 mm,
    /// values `i * 10 + j * 100 + k * 1000`, with a TR and a command line after the voxels.
    fn mgh_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        for v in [1i32, 4, 3, 2, 1, 4, 0] {
            b.extend(v.to_be_bytes());
        }
        b.extend(1i16.to_be_bytes());
        let geom: [f32; 15] = [
            1.0, 1.5, 2.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 5.0, 6.0, 7.0,
        ];
        for v in geom {
            b.extend(v.to_be_bytes());
        }
        b.resize(specular_medvol::mgh::HEADER, 0);
        for k in 0..2i16 {
            for j in 0..3i16 {
                for i in 0..4i16 {
                    b.extend((i * 10 + j * 100 + k * 1000).to_be_bytes());
                }
            }
        }
        b.extend(2000.0f32.to_be_bytes());
        b.extend([0u8; 16]);
        b.extend(3i32.to_be_bytes());
        b.extend(9i64.to_be_bytes());
        b.extend(b"recon-all");
        b
    }

    #[test]
    fn freesurfer_mgh_and_mgz() {
        let dir = std::env::temp_dir().join(format!("specular-mgh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let raw = mgh_bytes();
        std::fs::write(dir.join("brain.mgh"), &raw).unwrap();
        std::fs::write(dir.join("brain.mgz"), crate::exif_locate::tests::gzip(&raw)).unwrap();
        let c = CancelToken::new();
        for name in ["brain.mgh", "brain.mgz"] {
            let p = dir.join(name);
            assert!(is_research_volume_name(&p));
            let s = Registry::default().open_path(&p).unwrap();
            assert_eq!(s.info().format, Format::Mgh, "{name}");
            let f = s.research_volume().unwrap().clone();
            assert_eq!(f.format, "FreeSurfer MGH");
            assert_eq!(f.orientation, "vox2ras");
            assert!(f.reliable);
            assert_eq!(f.dims, vec![4, 3, 2]);
            assert_eq!(f.data_type, "int16");
            // Coronal slices (k → anterior) shown as an axial stack: 4 columns (i, toward
            // the left), 2 rows (k, anterior → posterior), 3 slices (j, inferior first).
            assert_eq!((s.info().width, s.info().height), (4, 2));
            assert_eq!(s.page_count(), 3);
            let m = s.modality_frame_at(0, &c).unwrap().unwrap();
            assert!(m.patient_axes().is_some(), "{name}");
            // Row 0 is the most posterior row: k = 1 (k runs toward the front); slice 0 the
            // lowest: j = 2 (j runs toward the feet); column 0 at i = 0.
            assert_eq!(m.value_at(0, 0), Some(1200.0), "{name}");
            assert_eq!(m.value_at(3, 1), Some(230.0), "{name}");
            // The scan parameters and tags: at once for .mgh, after the voxels for .mgz.
            s.decode(&DecodeRequest::full(1), &c).unwrap();
            let f = s.research_volume().unwrap();
            let keys: Vec<&str> = f.params.iter().map(|p| p.key).collect();
            assert_eq!(keys, ["tr", "cmdline"], "{name}");
            // MPR reads the file the same way.
            let v = specular_medvol::open_path(&p).unwrap();
            assert_eq!(v.canonical().dims, [4, 2, 3]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn industrial_ct_opens_by_content() {
        for (name, format, pages) in [
            ("txm_u16.txm", Format::Txm, 6),
            ("txrm_f32.txrm", Format::Txm, 3),
            ("mosaic.xrm", Format::Txm, 1),
            ("am_le.am", Format::Amira, 4),
            ("am_rle.am", Format::Amira, 0),
            ("skyscan_png/sample_rec.log", Format::SkyScan, 5),
            ("xtek_f32.xtekct", Format::Xtek, 4),
            ("xtek_u16.xtekVolume", Format::Xtek, 3),
            ("vgi_u16.vgi", Format::Vgi, 3),
            // A volume's samples open through the header beside them.
            ("xtek_f32.vol", Format::Xtek, 4),
            ("vgi_u16.raw", Format::Vgi, 3),
        ] {
            let s = open(name);
            assert_eq!(s.info().format, format, "{name}");
            if pages > 0 {
                assert_eq!(s.page_count(), pages, "{name}");
            }
            let f = s.research_volume().unwrap();
            assert_eq!(f.orientation, "object", "{name}");
            assert!(!f.reliable, "{name}");
            // The thumbnail and first view: the middle slice.
            let c = CancelToken::new();
            let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
            assert_eq!(
                (r.width, r.height),
                (s.info().width, s.info().height),
                "{name}"
            );
            assert_eq!(
                s.page_index(),
                s.page_count() / 2 % s.page_count().max(1),
                "{name}"
            );
        }
        let f = open("txrm_f32.txrm").research_volume().unwrap().clone();
        assert_eq!(f.format, "Zeiss Xradia TXM");
        assert!(f.params.iter().any(
            |p| p.key == "content" && p.value == medvol::ParamValue::Text("projections".into())
        ));
    }

    #[test]
    fn headerless_volumes_wait_for_their_layout() {
        let reg = Registry::default();
        let p = sample("cube.vol");
        assert!(medvol::raw::needs_layout(&p));
        let e = reg.open_path(&p).err().expect("no layout yet");
        assert!(matches!(e, DecodeError::Unsupported(_)), "{e}");
        medvol::raw::remember(
            &p,
            medvol::raw::RawLayout {
                size: [4, 4, 4],
                dtype: medvol::DataType::F32,
                big_endian: true,
                offset: 0,
                spacing: [0.5, 0.5, 1.0],
            },
        );
        let s = reg.open_path(&p).unwrap();
        assert_eq!(s.info().format, Format::RawVolume);
        assert_eq!(s.page_count(), 4);
        assert_eq!(s.research_volume().unwrap().spacing, [0.5, 0.5, 1.0]);
        // Folder listings: a header's samples are left out, a SkyScan log is listed.
        assert!(reg.supports_file(&p));
        assert!(!reg.supports_file(&sample("xtek_f32.vol")));
        assert!(!reg.supports_file(&sample("vgi_u16.raw")));
        assert!(reg.supports_file(&sample("xtek_f32.xtekct")));
        assert!(reg.supports_file(&sample("skyscan_bmp/sample_rec.log")));
        assert!(reg.supports_file(&sample("am_le.am")));
        assert!(is_research_volume_name(&sample("txm_u16.txm")));
    }
}
