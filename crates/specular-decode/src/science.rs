//! Scientific N-D images (FITS, XISF, HDF5, NetCDF, EMD): what a session tells the viewer about its axes and
//! world coordinates, the real-valued planes it hands out ([`crate::Session::nd_plane`]),
//! and the auto-stretched raster it decodes for thumbnails and the CPU path.
//!
//! An image is a stack of `width × height` planes over further axes: the cube axes of a
//! FITS HDU (spectral, Stokes, time…), an "extension" axis when a file holds several images of
//! one shape, an XISF image's channels. Every plane is real-valued (`f32`, physical values,
//! NaN where undefined), rows top-down as displayed.

use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::nd::{self, Limits, Stretch};
use specular_core::{CancelToken, PixelLayout, Raster};

pub use crate::fcs_dec::CytometryFacts;
pub use crate::hdf4_dec::{EosFacts, Hdf4Facts, LonLatGrid};
pub use crate::imzml_dec::{IonTolerance, IonWindow, MassSpec};
pub use crate::thermal_dec::{TemperatureUnit, ThermalCamera, ThermalConditions, ThermalImage};
pub use specular_em::dataset::{Orientations, Phase as CrystalPhase};
pub use specular_fits::catalogue;
pub use specular_fits::wcs;
/// Temperature grids as a float32 TIFF and as CSV, for export.
pub use specular_fmt_thermal::{csv as temperature_csv, tiff_f32 as temperature_tiff};
pub use specular_planetary::{Facts as PlanetaryFacts, MapFacts as PlanetaryMap};

/// The source catalogue in `bytes` (a VOTable, a FITS table, delimited text with sky
/// positions); `None` for anything else, a FITS file with an image to show among them.
pub fn catalogue_of(bytes: &[u8]) -> Option<catalogue::Catalogue> {
    if crate::fits_dec::has_pages(bytes) {
        return None;
    }
    catalogue::parse(bytes).ok()
}
pub use specular_wsi::annot;
/// An X-ray diffraction frame's geometry: 2θ, q and d under a pixel, resolution rings.
pub use specular_xtal::Geometry as DiffractionGeometry;

/// One axis past the image plane.
#[derive(Clone, Debug, PartialEq)]
pub struct NdAxis {
    /// The file's name for the axis (`CTYPEn`, "Channel"), or empty when it gives none.
    pub name: String,
    pub len: u32,
    /// Unit of the world values (`CUNITn`), may be empty.
    pub unit: String,
    /// A label per position (extension names, channel names); empty when positions are
    /// only numbered.
    pub labels: Vec<String>,
    /// The WCS axis giving this axis' world values ([`wcs::Wcs::axis_value`]).
    pub wcs_axis: Option<usize>,
    /// World values as a linear function of the position (a calibrated energy-loss axis, a
    /// defocus series), in `unit`, when there is no WCS for it.
    pub linear: Option<Linear>,
    /// What the axis is, for wording.
    pub kind: AxisKind,
}

/// A linear calibration: the world value at position `i` is `offset + step × i`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Linear {
    pub offset: f64,
    pub step: f64,
}

impl Linear {
    pub fn at(&self, i: f64) -> f64 {
        self.offset + self.step * i
    }
}

/// Physical position of the image's pixels (microscopy, scanning probe): the value at a
/// displayed column and row (pixel indices, rows top-down).
#[derive(Clone, Debug, PartialEq)]
pub struct PixelCalibration {
    pub x: Linear,
    pub y: Linear,
    /// Units of x and y ("m", "nm", "1/nm"…); lengths in metres come from SI units.
    pub unit_x: String,
    pub unit_y: String,
}

/// The field a scientific image comes from, for wording and the info panel's group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScienceKind {
    /// Astronomy (FITS, XISF).
    #[default]
    Astronomy,
    /// Electron microscopy (DigitalMicrograph, MRC).
    ElectronMicroscopy,
    /// Scanning probe microscopy (Gwyddion, Igor waves): height maps and the like.
    ScanningProbe,
    /// Whole-slide images and light-microscopy pyramids (SVS, NDPI, SCN, OME-TIFF,
    /// OME-Zarr).
    Slide,
    /// Gridded data with physical axes (NeXus, Imaris, BAG, SRTM, Gaussian cube).
    Gridded,
    /// Light microscopy stacks (ImageJ hyperstacks, Zeiss LSM, MetaMorph STK) and other
    /// scientific TIFFs.
    Microscopy,
    /// X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
    Diffraction,
    /// Areal surface topography (X3P, Zygo, Keyence, Sensofar, Alicona, Digital Surf).
    Surface,
    /// Electron backscatter diffraction maps (EDAX .ang, Oxford .ctf).
    Ebsd,
    /// Seismic data (SEG-Y, Seismic Unix): sections of traces against time or depth.
    Seismic,
    /// Imaging mass spectrometry (imzML): a mass spectrum per pixel, shown as the total
    /// ion current or an ion image.
    MassSpec,
    /// Radiometric thermal stills (FLIR, InfiRay, InfraTec): a temperature per pixel.
    Thermal,
    /// Spectroscopy camera frames (Princeton Instruments SPE, Andor SIF): columns are
    /// wavelengths, each row a spectrum.
    Spectroscopy,
    /// Flow cytometry list-mode data (FCS): a density plot of two parameters.
    Cytometry,
}

/// Where a [`PixelCalibration`] comes from, for the measuring tool's wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelSource {
    /// TIFF XResolution / YResolution (with ImageJ's unit).
    Resolution,
    /// A vendor's own tag or sidecar (Zeiss LSM, MetaMorph, SEM vendors).
    Vendor,
}

/// A 256-entry colour table, display-coded sRGB.
pub type ColourTable = [[u8; 3]; 256];

/// Reads an ImageJ colour table file (`.lut`: raw, NIH `ICOL` or text) for single planes to
/// show through.
pub fn read_lut(bytes: &[u8]) -> Option<Arc<ColourTable>> {
    specular_tiff::science::parse_lut(bytes).map(Arc::from)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisKind {
    /// A cube axis of the data (FITS `NAXIS3`…).
    Cube,
    /// The images (HDUs) of a multi-extension file.
    Extension,
    /// Colour or filter channels.
    Channel,
    /// The fields (columns) of a HEALPix map.
    Field,
    /// The bin size of an event list's image, in sky pixels (labels).
    Bin,
    /// An event list's energy bands; a position without a label is every event.
    Band,
    /// The parts of complex values: magnitude, real, imaginary and phase, in
    /// [`specular_arrays::Part::ALL`] order.
    Part,
    /// The frames of a recording in time (a FLIR sequence), labelled with their times.
    Frame,
}

/// The credible regions of a probability sky map (a gravitational-wave localisation): the
/// plane they are drawn on and, per enclosed probability, the plane value its contour
/// follows.
#[derive(Clone, Debug, PartialEq)]
pub struct Credible {
    pub index: Vec<u32>,
    /// `(probability, threshold)`, such as `(0.9, 2.1e-5)`.
    pub levels: Vec<(f64, f64)>,
}

/// A categorical plane whose values are named classes; the app holds the names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassScheme {
    /// Sentinel-2 Level-2A scene classification (SCL), classes 0–11 (ESA's L2A product
    /// specification); 0 (no data) is stored as NaN.
    Sentinel2Scl,
    /// Sentinel-2 Level-1C cloud and snow masks (`MSK_CLASSI`) as one plane: 0 clear,
    /// 1 opaque cloud, 2 cirrus, 3 snow or ice.
    Sentinel2L1cMask,
}

impl ClassScheme {
    /// The class number of value `v`, `None` when `v` is no class of the scheme.
    pub fn class_of(self, v: f32) -> Option<u8> {
        let last = match self {
            ClassScheme::Sentinel2Scl => {
                if v.is_nan() {
                    return Some(0);
                }
                11.0
            }
            ClassScheme::Sentinel2L1cMask => 3.0,
        };
        (v >= 0.0 && v <= last && v.fract() == 0.0).then_some(v as u8)
    }
}

/// What the info panel and the science view know about a scientific image.
#[derive(Clone, Debug, Default)]
pub struct ScienceSummary {
    pub width: u32,
    pub height: u32,
    /// Axes past the image plane, in index order ([`crate::Session::nd_plane`]).
    pub axes: Vec<NdAxis>,
    /// The axis to composite as colour channels by default (an RGB XISF, a FITS cube or
    /// extension set of three named R, G, B).
    pub channel_axis: Option<usize>,
    pub wcs: Option<Arc<wcs::Wcs>>,
    /// Sample type, e.g. "32-bit float", "16-bit unsigned, Rice".
    pub sample: String,
    /// The HDU (FITS extension) shown, by name, when the file names it.
    pub hdu: Option<String>,
    pub object: Option<String>,
    pub telescope: Option<String>,
    pub instrument: Option<String>,
    pub filter: Option<String>,
    /// `DATE-OBS` as stored.
    pub date_obs: Option<String>,
    /// Seconds.
    pub exposure: Option<f64>,
    /// Unit of the values (`BUNIT`).
    pub unit: Option<String>,
    /// Header cards (keyword, value as text, comment), in file order.
    pub keywords: Arc<Vec<(String, String, String)>>,
    /// The dataset or variable shown (HDF5, NetCDF, EMD), by path or name, with its long
    /// name when the file gives one.
    pub dataset: Option<String>,
    /// The field the image comes from.
    pub kind: ScienceKind,
    /// Physical pixel positions (microscopy, EMD), for the readout and a scale bar when there
    /// are no sky coordinates.
    pub pixel: Option<PixelCalibration>,
    /// The value unit per position of an axis whose planes differ in unit (Gwyddion
    /// channels: height in m, phase in deg): axis and units, overriding `unit`.
    pub units_by: Option<(usize, Vec<String>)>,
    /// The image's or channel's title.
    pub title: Option<String>,
    /// Accelerating voltage, volts (electron microscopy).
    pub voltage: Option<f64>,
    /// Indicated magnification.
    pub magnification: Option<f64>,
    /// Operation or imaging mode ("TEM", "STEM", "AC Mode").
    pub mode: Option<String>,
    /// Camera or detector.
    pub detector: Option<String>,
    /// The planes are height maps that plane levelling suits (scanning probe).
    pub levelling: bool,
    /// The display colour of each position of the channel axis as the file gives it (OME
    /// channel colours), display-coded 0..=1; empty for the viewer's defaults.
    pub channel_colours: Vec<[f32; 3]>,
    /// The image is shown from its decoded raster (a slide pyramid too large for value
    /// planes): the summary gives the readout's position, the scale bar and the info panel,
    /// and [`crate::Session::nd_plane`] has nothing.
    pub raster_only: bool,
    /// Who made the slide (scanner vendor or convention: "Aperio", "OME-Zarr").
    pub vendor: Option<String>,
    /// Stored resolutions, full size first.
    pub levels: Vec<(u32, u32)>,
    /// Where the image sits on the Earth (a GeoTIFF elevation model or measurement grid):
    /// map coordinates and a scale bar in metres instead of pixels.
    pub geo: Option<Arc<crate::geo::GeoRef>>,
    /// The colour filter array of a one-shot colour camera's raw frame (FITS `BAYERPAT`, a
    /// SER colour ID), in displayed orientation: the image is shown debayered.
    pub cfa: Option<nd::Cfa>,
    /// Who captured it (SER).
    pub observer: Option<String>,
    /// A video's frames and capture times (SER).
    pub video: Option<VideoFacts>,
    /// The plane shown first (a position on each axis); empty for the first plane.
    pub start: Vec<u32>,
    /// Where `pixel` comes from.
    pub pixel_source: Option<PixelSource>,
    /// The display range the file stores (ImageJ's min/max), per position of the channel
    /// axis (or one for every plane), in the planes' values.
    pub display_ranges: Vec<(f64, f64)>,
    /// The colour table a single plane shows through (ImageJ LUTs), per position of the
    /// channel axis (or one for every plane); empty for grey.
    pub luts: Vec<Arc<ColourTable>>,
    /// The detector's geometry (diffraction frames): scattering angle, q and d per pixel and
    /// resolution rings; pixels in file coordinates, which the planes keep.
    pub diffraction: Option<Arc<DiffractionGeometry>>,
    /// A spectrum per pixel (an EDS hypermap, a TIA spectrum image), plotted at the cursor.
    pub spectra: Option<PixelSpectra>,
    /// Crystal orientations per pixel (an EBSD map), for the readout.
    pub orientations: Option<Arc<Orientations>>,
    /// Seismic sections: what each column and row is (inline, crossline, trace, time).
    pub seismic: Option<Arc<Seismic>>,
    /// Shapes drawn over the image: a slide's annotation sidecar (NDPA, Aperio, ASAP).
    pub annotations: Option<Arc<AnnotationSet>>,
    /// Credible regions to outline (HEALPix probability maps).
    pub credible: Option<Credible>,
    /// Imaging mass spectrometry: ion images of m/z windows, pixel positions.
    pub mass_spec: Option<Arc<MassSpec>>,
    /// The values are bit flags (a data quality array): the readout lists the set bits.
    pub flags: bool,
    /// Which positions of an axis hold bit flags when only some planes do (a FITS `DQ`
    /// extension beside `SCI` and `ERR`): axis and one entry per position.
    pub flags_by: Option<(usize, Vec<bool>)>,
    /// The values are classes with names (a scene classification): the readout names them.
    pub classes: Option<ClassScheme>,
    /// The ASDF file the image or its metadata comes from (an `.asdf` file, the `ASDF`
    /// extension of a JWST FITS file).
    pub asdf: Option<AsdfFacts>,
    /// The CASA image the planes come from (type, beams, spectral frame, mask).
    pub casa: Option<CasaFacts>,
    /// `wcs` approximates the file's world coordinates (a TAN projection from an ASDF
    /// pointing, the GWCS distortion left out).
    pub wcs_approximate: bool,
    /// Thermal stills: temperatures under chosen object parameters, the camera's records.
    pub thermal: Option<Arc<ThermalImage>>,
    /// A CERN ROOT histogram or graph: the file, the object and what its pixels are.
    pub root: Option<Arc<RootFacts>>,
    /// Longitude and latitude per pixel from geolocation arrays (an HDF-EOS swath), for the
    /// readout when there is no map georeference.
    pub lonlat: Option<Arc<LonLatGrid>>,
    /// What an HDF4 file holds, and its HDF-EOS structures (the info panel).
    pub hdf4: Option<Arc<Hdf4Facts>>,
    /// An unstructured mesh (UGRID, ICON, MPAS) painted onto the planes: the face and
    /// value under a point, the wireframe.
    pub mesh: Option<Arc<dyn MeshProbe>>,
    /// An image on an OMERO server: where it lives and how it is rendered.
    pub omero: Option<Arc<crate::OmeroFacts>>,
    /// A planetary image's observation and map projection (VICAR, PDS3, PDS4).
    pub planetary: Option<Arc<PlanetaryFacts>>,
    /// A flow cytometry plot (FCS): its axes for the readout and the ticks.
    pub cytometry: Option<Arc<CytometryFacts>>,
}

/// A CERN ROOT file and the object shown from it, for the info panel and the readout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RootFacts {
    /// The ROOT release that wrote the file ("6.28/04").
    pub version: String,
    /// "zlib level 4", "none".
    pub compression: String,
    /// Keys in every directory.
    pub keys: u64,
    /// The object: class, path in the file, title.
    pub class: String,
    pub path: String,
    pub title: String,
    /// Fills (fEntries) of a histogram.
    pub entries: Option<f64>,
    /// Sum of weights (fTsumw) of a histogram.
    pub sum_weights: Option<f64>,
    /// A graph's points (TGraph), sorted by x; empty for a histogram.
    pub points: Arc<[(f64, f64)]>,
    /// The histogram's axes, x first; for a graph, its x axis only (titles).
    pub axes: Vec<RootAxis>,
    /// Under- and overflow bins holding something, and their sum: not shown on the image.
    pub flow: Option<(u64, f64)>,
    /// Variable-width bins were resampled onto an even grid of pixels.
    pub resampled: bool,
    /// A profile: the values are the mean per bin.
    pub profile: bool,
    /// Trees in the file (path, entries), listed only.
    pub trees: Vec<(String, i64)>,
}

/// One axis of a ROOT histogram.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RootAxis {
    /// The axis title without its unit ("p_{T}"), and the unit from its trailing brackets.
    pub title: String,
    pub unit: String,
    pub bins: u32,
    pub min: f64,
    pub max: f64,
    pub variable: bool,
    /// Mean and RMS from the stored sums.
    pub mean_rms: Option<(f64, f64)>,
    /// The bins' edges (bins + 1).
    pub edges: Arc<[f64]>,
    /// Alphanumeric bin labels by bin (from 1).
    pub labels: Arc<[(u32, String)]>,
    /// The bin (from 1) under each pixel along this image axis, in displayed order (columns
    /// left to right, rows top to bottom); empty for an axis on a slider. For a graph, the
    /// point (from 1) under each column.
    pub pixel_bins: Arc<[u32]>,
}

impl RootAxis {
    /// The label of bin `b`, if any.
    pub fn label(&self, b: u32) -> Option<&str> {
        self.labels
            .iter()
            .find(|(i, _)| *i == b)
            .map(|(_, s)| s.as_str())
    }

    /// The low and high edge of bin `b` (from 1).
    pub fn bin_range(&self, b: u32) -> Option<(f64, f64)> {
        let i = (b as usize).checked_sub(1)?;
        Some((*self.edges.get(i)?, *self.edges.get(i + 1)?))
    }

    /// The width of a pixel along the axis when pixels are evenly spaced in value.
    pub fn pixel_step(&self) -> Option<f64> {
        let n = self.pixel_bins.len();
        (n > 0 && self.edges.len() >= 2).then(|| (self.max - self.min) / n as f64)
    }
}

impl RootFacts {
    /// The bins (from 1) under displayed pixel (`x`, `y`) of the plane at `index`, per axis
    /// in axis order; for a graph, the point.
    pub fn bins_at(&self, x: u32, y: u32, index: &[u32]) -> Vec<u32> {
        let mut out = Vec::new();
        for (k, a) in self.axes.iter().enumerate() {
            let b = match k {
                0 => a.pixel_bins.get(x as usize).copied(),
                1 if !a.pixel_bins.is_empty() => a.pixel_bins.get(y as usize).copied(),
                _ => index.first().map(|&i| i + 1),
            };
            match b {
                Some(b) => out.push(b),
                None => return Vec::new(),
            }
        }
        out
    }
}

/// Facts about an unstructured mesh for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshFacts {
    /// The topology's name (`Mesh2`, `mesh2d`, "ICON grid"), with its long name.
    pub name: String,
    pub long_name: Option<String>,
    /// "UGRID", "ICON" or "MPAS".
    pub convention: &'static str,
    pub nodes: u64,
    pub faces: u64,
    /// Edges as the file stores them, else the distinct sides of the faces.
    pub edges: u64,
    pub triangles: u64,
    pub quads: u64,
    /// Faces of five or more nodes.
    pub polygons: u64,
    pub max_face_nodes: u32,
    /// Longitude and latitude (else projected x and y in `unit`).
    pub geographic: bool,
    pub unit: Option<String>,
    /// Where the values shown sit; `None` for a grid without data (the planes count each
    /// face's nodes).
    pub location: Option<MeshLocation>,
    /// Faces split at the antimeridian or closed to a pole for drawing.
    pub split_faces: u64,
}

/// Where a mesh variable's values sit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshLocation {
    /// One value per face, drawn flat.
    Face,
    /// One value per node, interpolated across the faces.
    Node,
}

/// The mesh under a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshHit {
    pub face: u32,
    /// The nearest node of the face, when the values sit on the nodes.
    pub node: Option<u32>,
    /// The value at the point (interpolated between the nodes), NaN where masked.
    pub value: f64,
    /// The point in mesh coordinates: longitude (−180‥180) and latitude, or x and y.
    pub x: f64,
    pub y: f64,
}

/// An unstructured mesh behind a science image: the planes are the mesh painted at the
/// image's resolution; this answers for the mesh itself.
pub trait MeshProbe: Send + Sync + std::fmt::Debug {
    fn facts(&self) -> &MeshFacts;
    /// The face and value at stored pixel position (`x`, `y`) (continuous, (0, 0) the top
    /// left corner) of the plane at `index`; `None` outside the mesh.
    fn at(&self, x: f64, y: f64, index: &[u32]) -> Option<MeshHit>;
    /// The mesh's edges reaching the stored pixel rectangle `visible` (min x, min y, max
    /// x, max y) as segments in stored pixels (x0, y0, x1, y1); `None` when more than `max`
    /// do.
    fn edges(&self, visible: [f64; 4], max: usize) -> Option<Vec<[f32; 4]>>;
}

/// One pixel's spectrum: values against an x axis (energy, wavelength, m/z…).
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    /// The values (counts, intensities, radiances), one per x position.
    pub y: Vec<f32>,
    pub x: SpectrumX,
    /// The values are separate peaks (centroided mass spectra), drawn as sticks; else a
    /// curve.
    pub sticks: bool,
}

/// Where a spectrum's values lie on its x axis.
#[derive(Clone, Debug, PartialEq)]
pub enum SpectrumX {
    /// Evenly spaced: value `k` at `offset + step × k` (EDS channels, a FITS cube's WCS).
    Linear(Linear),
    /// One x value per value, ascending or descending (m/z of a processed imzML spectrum,
    /// ENVI band wavelengths).
    Values(Arc<[f64]>),
}

impl Spectrum {
    /// The x value of value `k`.
    pub fn x_at(&self, k: usize) -> f64 {
        match &self.x {
            SpectrumX::Linear(l) => l.at(k as f64),
            SpectrumX::Values(v) => v.get(k).copied().unwrap_or(f64::NAN),
        }
    }

    /// The first and last x values; `None` when empty.
    pub fn x_range(&self) -> Option<(f64, f64)> {
        let n = self.y.len();
        (n > 0).then(|| (self.x_at(0), self.x_at(n - 1)))
    }
}

/// Where the spectra of an image's pixels come from: a hypermap, a mass spectrometry
/// image, a hyperspectral cube.
pub trait SpectrumSource: Send + Sync + std::fmt::Debug {
    /// The grid the spectra are on: columns, rows. The image's pixels map onto it by
    /// proportion when the sizes differ.
    fn grid(&self) -> (usize, usize);
    /// Values per spectrum when every spectrum has as many (0 when they differ).
    fn channels(&self) -> usize;
    /// What the x axis is ("Energy", "m/z", "Wavelength") and its unit ("keV", "", "nm").
    fn x_axis(&self) -> (String, String);
    /// The spectrum at grid point (`x`, `y`) of the plane at `index` on the further axes
    /// (most sources ignore it).
    fn at(&self, x: usize, y: usize, index: &[u32]) -> Option<Spectrum>;
    /// The file's sum spectrum, when it stores one.
    fn sum(&self) -> Option<Spectrum> {
        None
    }
}

/// Facts about an ASDF file for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AsdfFacts {
    /// File format version (`1.0.0`).
    pub version: String,
    /// ASDF Standard version (`1.5.0`).
    pub standard: Option<String>,
    /// The library that wrote the file, "name version".
    pub library: Option<String>,
    /// The data model's tag name (`wfi_image`, `ImageModel`).
    pub model: Option<String>,
}

/// Facts about a CASA image for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CasaFacts {
    /// What the directory name says the image is: "image", "residual", "model", "pb",
    /// "mask", "psf", "sumwt", "weight" or "other".
    pub image_type: &'static str,
    /// `imageinfo.imagetype` ("Intensity", "Beam").
    pub info_type: Option<String>,
    /// Restoring beam: major and minor axis (arcsec), position angle (degrees).
    pub beam: Option<[f64; 3]>,
    /// Per-plane beams.
    pub plane_beams: Option<PlaneBeams>,
    /// Hz.
    pub rest_frequency: Option<f64>,
    /// The spectral reference frame ("LSRK", "BARY", "TOPO").
    pub spectral_frame: Option<String>,
    /// The velocity definition the frequency labels use: "radio", "optical" or
    /// "relativistic".
    pub velocity: Option<&'static str>,
    /// The default mask's name ("mask0").
    pub mask: Option<String>,
    /// The storage manager ("TiledCellStMan", "TiledShapeStMan").
    pub storage: &'static str,
    pub big_endian: bool,
}

/// A CASA image's per-plane beams: how many, and the range of their major and minor axes
/// (arcsec).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneBeams {
    pub count: usize,
    pub major: (f64, f64),
    pub minor: (f64, f64),
}

/// The spectra of an image's pixels, read from the file on demand.
#[derive(Clone, Debug)]
pub struct PixelSpectra {
    pub(crate) spectra: Arc<dyn SpectrumSource>,
    /// The image's size, which the spectrum grid maps onto by proportion.
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl PixelSpectra {
    pub(crate) fn new(spectra: Arc<dyn SpectrumSource>, width: u32, height: u32) -> PixelSpectra {
        PixelSpectra {
            spectra,
            width,
            height,
        }
    }

    /// Values per spectrum (0 when spectra differ in length).
    pub fn channels(&self) -> usize {
        self.spectra.channels()
    }

    /// What the x axis is and its unit.
    pub fn x_axis(&self) -> (String, String) {
        self.spectra.x_axis()
    }

    /// The spectrum under displayed pixel (`x`, `y`) of the plane at `index`.
    pub fn at(&self, x: u32, y: u32, index: &[u32]) -> Option<Spectrum> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let (gw, gh) = self.spectra.grid();
        let gx = (u64::from(x) * gw as u64 / u64::from(self.width)) as usize;
        let gy = (u64::from(y) * gh as u64 / u64::from(self.height)) as usize;
        if gx >= gw || gy >= gh {
            return None;
        }
        self.spectra.at(gx, gy, index)
    }

    /// The file's sum spectrum, when it stores one.
    pub fn sum(&self) -> Option<Spectrum> {
        self.spectra.sum()
    }
}

/// What the columns and rows of seismic sections are, for the readout. Planes are
/// sections with time (or depth) down: a 2-D line's traces across, or a 3-D volume's
/// inline sections (one per position of the "Inline" axis) with the crosslines across.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seismic {
    /// Rows are depths (in `unit`) rather than two-way times (ms).
    pub depth: bool,
    /// "ms", "m" or "ft" (or empty when a depth's unit is not stated).
    pub unit: String,
    /// The first sample's time or depth and the step per row.
    pub t0: f64,
    pub dt: f64,
    /// A volume's inline and crossline numbers (empty for a line) and the header bytes
    /// they come from.
    pub inlines: Vec<i32>,
    pub crosslines: Vec<i32>,
    pub key: (usize, usize),
    /// A volume's trace per cell, inline-major (`u32::MAX` for a gap); a line's trace per
    /// column (every n-th trace when the line is wider than shown).
    pub traces: Vec<u32>,
    /// Per trace in file order: the CDP number (bytes 21-24) and the CDP (or source) X/Y
    /// with the coordinate scalar applied; `coords` is empty when every trace has 0, 0.
    pub cdp: Vec<i32>,
    pub coords: Vec<[f64; 2]>,
}

/// One position in a seismic section.
#[derive(Clone, Debug, PartialEq)]
pub struct SeismicPoint {
    pub inline: Option<i32>,
    pub crossline: Option<i32>,
    /// The trace (from 1, as in the file) and its CDP number; `None` in a gap.
    pub trace: Option<usize>,
    pub cdp: Option<i32>,
    pub xy: Option<[f64; 2]>,
    /// Time (ms) or depth of the row.
    pub time: f64,
}

impl Seismic {
    /// A volume (inline sections) rather than a line.
    pub fn is_volume(&self) -> bool {
        !self.inlines.is_empty()
    }

    /// What is at column `x`, row `y` of the plane at `index` (the inline's position for a
    /// volume).
    pub fn at(&self, index: &[u32], x: u32, y: u32) -> SeismicPoint {
        let (inline, crossline, trace) = if self.is_volume() {
            let il = index.first().copied().unwrap_or(0) as usize;
            let xl = x as usize;
            let t = self
                .traces
                .get(il * self.crosslines.len() + xl)
                .copied()
                .filter(|&t| t != u32::MAX);
            (
                self.inlines.get(il).copied(),
                self.crosslines.get(xl).copied(),
                t,
            )
        } else {
            (None, None, self.traces.get(x as usize).copied())
        };
        let trace = trace.map(|t| t as usize);
        SeismicPoint {
            inline,
            crossline,
            trace: trace.map(|t| t + 1),
            cdp: trace.and_then(|t| self.cdp.get(t).copied()),
            xy: trace.and_then(|t| self.coords.get(t).copied()),
            time: self.t0 + self.dt * f64::from(y),
        }
    }
}

/// The seismic colour table: dark blue through blue to white at the middle, then red to
/// dark red (negative amplitudes blue, positive red, zero white with limits symmetric
/// about 0).
pub fn seismic_lut() -> Arc<ColourTable> {
    let mut t = [[0u8; 3]; 256];
    for (i, c) in t.iter_mut().enumerate() {
        let x = i as f64 / 255.0;
        let rgb = if x < 0.25 {
            [0.0, 0.0, 0.3 + 0.7 * x / 0.25]
        } else if x < 0.5 {
            let s = (x - 0.25) / 0.25;
            [s, s, 1.0]
        } else if x < 0.75 {
            let s = 1.0 - (x - 0.5) / 0.25;
            [1.0, s, s]
        } else {
            [1.0 - 0.5 * (x - 0.75) / 0.25, 0.0, 0.0]
        };
        *c = rgb.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    Arc::new(t)
}

/// A slide's annotations and the sidecar they came from.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationSet {
    /// The sidecar's file name.
    pub file: String,
    pub set: annot::Annotations,
    /// Where each shape lies along a plane axis, when they belong to some planes only
    /// (a cube's atoms); `None`: every shape shows on every plane.
    pub depth: Option<Depth>,
}

/// The position of each annotation along one plane axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Depth {
    /// The summary axis ([`ScienceSummary::axes`]).
    pub axis: usize,
    /// Each item's fractional plane index along it (plane `k` covers `k ± 0.5`).
    pub at: Vec<f64>,
}

impl AnnotationSet {
    /// How strongly item `k` shows on the plane with index `index`: fully within half a
    /// plane, faded (0.35) on the planes either side, else not at all.
    pub fn opacity(&self, k: usize, index: &[u32]) -> Option<f32> {
        let Some(d) = &self.depth else {
            return Some(1.0);
        };
        let (Some(&z), Some(&at)) = (index.get(d.axis), d.at.get(k)) else {
            return Some(1.0);
        };
        let off = (at - f64::from(z)).abs();
        if off <= 0.5 {
            Some(1.0)
        } else if off <= 1.5 {
            Some(0.35)
        } else {
            None
        }
    }
}

/// The frames of a captured video (SER) for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VideoFacts {
    /// Frames present.
    pub frames: u32,
    /// Frames the header declares (more than `frames` when the file is cut short).
    pub declared: u32,
    /// First and last frame's capture time (UTC), "YYYY-MM-DD HH:MM:SS.fff".
    pub start: Option<String>,
    pub end: Option<String>,
    /// Mean frames per second from the frame timestamps.
    pub frame_rate: Option<f64>,
}

impl ScienceSummary {
    /// Number of planes (product of the axis lengths).
    pub fn planes(&self) -> u64 {
        self.axes.iter().map(|a| u64::from(a.len)).product()
    }

    /// Whether the plane at `index` holds bit flags ([`ScienceSummary::flags`]).
    pub fn flags_at(&self, index: &[u32]) -> bool {
        if let Some((axis, f)) = &self.flags_by {
            return index
                .get(*axis)
                .and_then(|&i| f.get(i as usize))
                .copied()
                .unwrap_or(false);
        }
        self.flags
    }

    /// The value unit of the plane at `index`.
    pub fn unit_at(&self, index: &[u32]) -> Option<&str> {
        if let Some((axis, units)) = &self.units_by
            && let Some(u) = index.get(*axis).and_then(|&i| units.get(i as usize))
        {
            return Some(u.as_str()).filter(|u| !u.trim().is_empty());
        }
        self.unit.as_deref().filter(|u| !u.trim().is_empty())
    }

    /// Fills the descriptive fields from FITS-style keywords: the image's header `h`, then
    /// `outer` (a FITS primary header, whose cards extensions inherit by convention).
    pub(crate) fn describe(
        &mut self,
        h: &specular_fits::Header,
        outer: Option<&specular_fits::Header>,
    ) {
        let s = |k: &str| {
            [Some(h), outer].into_iter().flatten().find_map(|h| {
                h.str(k)
                    .map(|v| v.trim().to_owned())
                    .filter(|v| !v.is_empty())
            })
        };
        let f = |k: &str| {
            [Some(h), outer]
                .into_iter()
                .flatten()
                .find_map(|h| h.f64(k))
        };
        self.object = s("OBJECT");
        self.telescope = s("TELESCOP");
        self.instrument = s("INSTRUME");
        self.filter = s("FILTER")
            .or_else(|| s("FILTNAM1"))
            .or_else(|| s("FILTER1"));
        self.date_obs = s("DATE-OBS").or_else(|| s("DATE"));
        self.exposure = f("EXPTIME").or_else(|| f("EXPOSURE"));
        self.unit = s("BUNIT");
        self.keywords = Arc::new(
            h.cards
                .iter()
                .filter(|c| !c.key.is_empty())
                .map(|c| {
                    (
                        c.key.clone(),
                        c.value.as_ref().map(|v| v.display()).unwrap_or_default(),
                        c.comment.clone(),
                    )
                })
                .collect(),
        );
    }
}

/// One plane of real values.
#[derive(Clone, Debug, PartialEq)]
pub struct NdPlane {
    pub width: u32,
    pub height: u32,
    /// `width × height` values, rows top-down; NaN where undefined.
    pub values: Vec<f32>,
    /// Part of the plane lay past the end of the file.
    pub truncated: bool,
}

impl NdPlane {
    pub fn value(&self, x: u32, y: u32) -> Option<f32> {
        (x < self.width && y < self.height)
            .then(|| self.values[y as usize * self.width as usize + x as usize])
    }
}

/// A small cache of decoded planes (most recent last), bounded in count and bytes.
///
/// Each cached plane holds a reservation of its bytes from the memory budget, given back
/// when it is evicted or cleared. A plane the budget cannot hold, even with the others
/// evicted, is returned without being cached.
pub(crate) struct PlaneCache {
    planes: Mutex<Vec<Cached>>,
}

struct Cached {
    index: Vec<u32>,
    plane: Arc<NdPlane>,
    _held: specular_core::Reservation,
}

/// Enough for a 16-channel composite.
const CACHE_PLANES: usize = 16;
const CACHE_BYTES: usize = 512 << 20;

impl PlaneCache {
    pub(crate) fn new() -> PlaneCache {
        PlaneCache {
            planes: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn get_or(
        &self,
        index: &[u32],
        cancel: &CancelToken,
        read: impl FnOnce() -> crate::Result<NdPlane>,
    ) -> crate::Result<Arc<NdPlane>> {
        {
            let mut g = self.planes.lock();
            if let Some(i) = g.iter().position(|c| c.index == index) {
                let e = g.remove(i);
                let p = e.plane.clone();
                g.push(e);
                return Ok(p);
            }
        }
        if cancel.is_cancelled() {
            return Err(crate::DecodeError::Cancelled);
        }
        let p = Arc::new(read()?);
        let bytes = size_of_val(p.values.as_slice()) as u64;
        let mut g = self.planes.lock();
        let held = match specular_core::budget::reserve(bytes) {
            Ok(r) => r,
            Err(_) => {
                // Older planes make room; without room even then, not cached.
                g.clear();
                match specular_core::budget::reserve(bytes) {
                    Ok(r) => r,
                    Err(_) => return Ok(p),
                }
            }
        };
        g.push(Cached {
            index: index.to_vec(),
            plane: p.clone(),
            _held: held,
        });
        while g.len() > CACHE_PLANES
            || (g.len() > 1
                && g.iter().map(|c| c.plane.values.len() * 4).sum::<usize>() > CACHE_BYTES)
        {
            g.remove(0);
        }
        Ok(p)
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        self.planes
            .lock()
            .iter()
            .map(|c| c.plane.values.len() * 4)
            .sum()
    }

    pub(crate) fn clear(&self) -> usize {
        let n = self.memory_bytes();
        self.planes.lock().clear();
        n
    }
}

/// The default view of an image as an 8-bit raster: the first plane in grey, or the first
/// three positions of the channel axis as red, green and blue, each at zscale limits with a
/// linear stretch (what the science view opens with).
pub(crate) fn auto_raster(planes: &[Arc<NdPlane>], cfa: Option<nd::Cfa>) -> Raster {
    let (w, h) = (planes[0].width, planes[0].height);
    if let Some(cfa) = cfa {
        let p = &planes[0];
        let stf = cfa.stf(&p.values, w as usize);
        let rgba = cfa_rgba(&p.values, w as usize, h as usize, cfa, stf);
        return crate::whole::interleaved(w, h, PixelLayout::Rgba, rgba, p.truncated);
    }
    let grey =
        |p: &NdPlane| nd::to_grey(&p.values, p.width as usize, Limits::ZScale, Stretch::Linear);
    if planes.len() >= 2 {
        let chans: Vec<Vec<u8>> = planes.iter().take(3).map(|p| grey(p)).collect();
        let n = w as usize * h as usize;
        let mut rgba = vec![255u8; n * 4];
        for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
            for (c, out) in px[..3].iter_mut().enumerate() {
                *out = chans.get(c).map_or(0, |ch| ch[i]);
            }
        }
        crate::whole::interleaved(
            w,
            h,
            PixelLayout::Rgba,
            rgba,
            planes.iter().any(|p| p.truncated),
        )
    } else {
        crate::whole::interleaved(
            w,
            h,
            PixelLayout::Gray,
            grey(&planes[0]),
            planes[0].truncated,
        )
    }
}

/// A raw colour filter array frame (`width × height`, rows top-down) demosaiced
/// ([`nd::Cfa::debayer_row`]) into display RGBA8 under each colour's automatic stretch
/// (`stf`, unlinked; a colour without one shows black), rows in parallel. NaN shows black.
pub fn cfa_rgba(
    values: &[f32],
    width: usize,
    height: usize,
    cfa: nd::Cfa,
    stf: [Option<(f32, f32, f32)>; 3],
) -> Vec<u8> {
    use rayon::prelude::*;
    // Display levels through a table per colour would need the value range; the curve is
    // cheap enough per sample.
    let curve = stf.map(|s| {
        s.map(|(lo, hi, mid)| {
            let inv = if hi > lo { 1.0 / (hi - lo) } else { 0.0 };
            (lo, inv, mid)
        })
    });
    let mut out = vec![255u8; width * height * 4];
    out.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, row)| {
            cfa.debayer_row(values, width, height, y, |x, rgb| {
                for c in 0..3 {
                    row[x * 4 + c] = match curve[c] {
                        Some((lo, inv, mid)) if rgb[c].is_finite() => {
                            (nd::mtf(mid, ((rgb[c] - lo) * inv).clamp(0.0, 1.0)) * 255.0 + 0.5)
                                as u8
                        }
                        _ => 0,
                    };
                }
            });
        });
    out
}

/// `values` (`width` per row) less their least-squares plane `a + b·x + c·y` fitted over the
/// finite values (what scanning-probe software calls mean plane subtraction). NaN stays NaN;
/// fewer than three finite values, or a degenerate fit, leave the values as they are.
pub fn level_plane(values: &[f32], width: usize) -> Vec<f32> {
    if width == 0 {
        return values.to_vec();
    }
    // Centred coordinates keep the normal equations well conditioned.
    let h = values.len() / width;
    let (cx, cy) = ((width as f64 - 1.0) / 2.0, (h as f64 - 1.0) / 2.0);
    let (mut n, mut sx, mut sy, mut sz) = (0f64, 0f64, 0f64, 0f64);
    let (mut sxx, mut syy, mut sxy, mut sxz, mut syz) = (0f64, 0f64, 0f64, 0f64, 0f64);
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        let x = (i % width) as f64 - cx;
        let y = (i / width) as f64 - cy;
        let z = f64::from(v);
        n += 1.0;
        sx += x;
        sy += y;
        sz += z;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
        sxz += x * z;
        syz += y * z;
    }
    if n < 3.0 {
        return values.to_vec();
    }
    // Solve [n sx sy; sx sxx sxy; sy sxy syy] · [a b c] = [sz sxz syz] by Cramer's rule.
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let m = [[n, sx, sy], [sx, sxx, sxy], [sy, sxy, syy]];
    let r = [sz, sxz, syz];
    let d = det3(m);
    let (a, b, c) = if d.abs() > 1e-12 * n.powi(3).max(1.0) {
        let col = |k: usize| {
            let mut mm = m;
            for (row, v) in mm.iter_mut().zip(r) {
                row[k] = v;
            }
            det3(mm) / d
        };
        (col(0), col(1), col(2))
    } else {
        // A single row or column: a line along it (else the mean).
        let line = |s: f64, ss: f64, sv: f64| {
            let var = ss - s * s / n;
            (var > 1e-12).then(|| {
                let slope = (sv - s * sz / n) / var;
                ((sz - slope * s) / n, slope)
            })
        };
        if let Some((a, b)) = line(sx, sxx, sxz) {
            (a, b, 0.0)
        } else if let Some((a, c)) = line(sy, syy, syz) {
            (a, 0.0, c)
        } else {
            (sz / n, 0.0, 0.0)
        }
    };
    values
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x = (i % width) as f64 - cx;
            let y = (i / width) as f64 - cy;
            (f64::from(v) - (a + b * x + c * y)) as f32
        })
        .collect()
}

/// Whether labels name red, green and blue (in that order).
pub(crate) fn rgb_labels(labels: &[String]) -> bool {
    labels.len() == 3
        && labels
            .iter()
            .zip([("R", "RED"), ("G", "GREEN"), ("B", "BLUE")])
            .all(|(l, (short, long))| {
                let l = l.trim().to_ascii_uppercase();
                l == short || l == long || l.starts_with(&format!("{short}_"))
            })
}

/// Contour lines of a `w × h` plane (rows top-down) at `level`, by marching squares over
/// the pixel centres: polylines in stored pixels, each closed where it meets itself. Values
/// that are not finite count as below the level, so a region at the edge of the data is
/// closed along it.
pub fn contour_lines(values: &[f32], w: usize, h: usize, level: f32) -> Vec<Vec<(f32, f32)>> {
    use std::collections::HashMap;
    if w < 2 || h < 2 || values.len() < w * h {
        return Vec::new();
    }
    let v = |x: usize, y: usize| {
        let a = values[y * w + x];
        if a.is_finite() { a } else { f32::NEG_INFINITY }
    };
    // Edges: 2·(y·w + x) is the one from (x, y) to (x + 1, y), + 1 the one to (x, y + 1).
    let point = |e: usize| -> (f32, f32) {
        let (c, down) = (e / 2, e % 2 == 1);
        let (x, y) = (c % w, c / w);
        let (x2, y2) = if down { (x, y + 1) } else { (x + 1, y) };
        let (a, b) = (v(x, y), v(x2, y2));
        let t = if a.is_finite() && b.is_finite() && a != b {
            ((level - a) / (b - a)).clamp(0.0, 1.0)
        } else if a.is_finite() {
            // Into no data: the crossing at the data's edge.
            0.0
        } else {
            1.0
        };
        let lerp = |p: usize, q: usize| p as f32 + 0.5 + t * (q as f32 - p as f32);
        (lerp(x, x2), lerp(y, y2))
    };
    let mut segs: Vec<[usize; 2]> = Vec::new();
    for y in 0..h - 1 {
        for x in 0..w - 1 {
            let (a, b, c, d) = (v(x, y), v(x + 1, y), v(x + 1, y + 1), v(x, y + 1));
            let k = usize::from(a >= level)
                | usize::from(b >= level) << 1
                | usize::from(c >= level) << 2
                | usize::from(d >= level) << 3;
            if k == 0 || k == 15 {
                continue;
            }
            let top = 2 * (y * w + x);
            let left = top + 1;
            let right = 2 * (y * w + x + 1) + 1;
            let bottom = 2 * ((y + 1) * w + x);
            let centre = (a + b + c + d) / 4.0 >= level;
            match k {
                1 | 14 => segs.push([left, top]),
                2 | 13 => segs.push([top, right]),
                3 | 12 => segs.push([left, right]),
                4 | 11 => segs.push([right, bottom]),
                6 | 9 => segs.push([top, bottom]),
                7 | 8 => segs.push([left, bottom]),
                5 if centre => segs.extend([[left, bottom], [top, right]]),
                5 => segs.extend([[left, top], [right, bottom]]),
                10 if centre => segs.extend([[left, top], [right, bottom]]),
                _ => segs.extend([[left, bottom], [top, right]]),
            }
        }
    }
    // Chain the segments through their shared edges.
    let mut at: HashMap<usize, Vec<usize>> = HashMap::with_capacity(segs.len() * 2);
    for (i, s) in segs.iter().enumerate() {
        at.entry(s[0]).or_default().push(i);
        at.entry(s[1]).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut lines = Vec::new();
    let walk = |start: usize, from: usize, used: &mut Vec<bool>, out: &mut Vec<usize>| {
        let mut e = from;
        let mut s = start;
        loop {
            used[s] = true;
            e = if segs[s][0] == e {
                segs[s][1]
            } else {
                segs[s][0]
            };
            out.push(e);
            match at[&e].iter().find(|&&n| !used[n]) {
                Some(&n) => s = n,
                None => break,
            }
        }
    };
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        let mut fwd = vec![segs[i][0]];
        walk(i, segs[i][0], &mut used, &mut fwd);
        // Back from the start, for a line that did not close.
        let mut back = Vec::new();
        if let Some(&n) = at[&segs[i][0]].iter().find(|&&n| !used[n]) {
            walk(n, segs[i][0], &mut used, &mut back);
        }
        back.reverse();
        back.extend(fwd);
        lines.push(back.into_iter().map(point).collect());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_numbers() {
        let scl = ClassScheme::Sentinel2Scl;
        assert_eq!(scl.class_of(f32::NAN), Some(0));
        assert_eq!(scl.class_of(4.0), Some(4));
        assert_eq!(scl.class_of(11.0), Some(11));
        assert_eq!(scl.class_of(12.0), None);
        assert_eq!(scl.class_of(4.5), None);
        let mask = ClassScheme::Sentinel2L1cMask;
        assert_eq!(mask.class_of(0.0), Some(0));
        assert_eq!(mask.class_of(3.0), Some(3));
        assert_eq!(mask.class_of(4.0), None);
        assert_eq!(mask.class_of(f32::NAN), None);
    }

    /// Cached planes hold their bytes in the memory budget until evicted or cleared; older
    /// planes make room for a new one, and one that cannot fit is returned uncached.
    #[test]
    fn plane_cache_holds_reservations() {
        use specular_core::budget::{MemoryBudget, with_budget};
        let budget = Arc::new(MemoryBudget::fixed(10_000));
        let plane = |n: usize| NdPlane {
            width: n as u32,
            height: 1,
            values: vec![1.0; n],
            truncated: false,
        };
        with_budget(budget.clone(), || {
            let c = PlaneCache::new();
            let t = CancelToken::new();
            c.get_or(&[0], &t, || Ok(plane(1000))).unwrap();
            c.get_or(&[1], &t, || Ok(plane(1000))).unwrap();
            assert_eq!(budget.reserved(), 8000);
            // A hit reserves nothing more.
            c.get_or(&[0], &t, || unreachable!()).unwrap();
            assert_eq!(budget.reserved(), 8000);
            // No room for a third: the others are evicted for it.
            c.get_or(&[2], &t, || Ok(plane(1000))).unwrap();
            assert_eq!(budget.reserved(), 4000);
            assert_eq!(c.memory_bytes(), 4000);
            // Too large for the budget at all: returned, not cached.
            let p = c.get_or(&[3], &t, || Ok(plane(3000))).unwrap();
            assert_eq!(p.values.len(), 3000);
            assert_eq!((budget.reserved(), c.memory_bytes()), (0, 0));
            c.get_or(&[4], &t, || Ok(plane(1000))).unwrap();
            assert_eq!(c.clear(), 4000);
            assert_eq!(budget.reserved(), 0);
        });
    }

    #[test]
    fn contours_close_around_a_peak() {
        let (w, h) = (40usize, 30usize);
        let v: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32 - 20.0, (i / w) as f32 - 15.0);
                (-(x * x + y * y) / 50.0).exp()
            })
            .collect();
        let lines = contour_lines(&v, w, h, 0.5);
        assert_eq!(lines.len(), 1);
        let l = &lines[0];
        assert_eq!(l.first(), l.last(), "closed");
        // On the circle where exp(-r²/50) = 0.5, around the peak's pixel centre.
        let r = (50.0f32 * 2f32.ln()).sqrt();
        for &(x, y) in l {
            let d = ((x - 20.5).powi(2) + (y - 15.5).powi(2)).sqrt();
            assert!((d - r).abs() < 0.15, "{x},{y}: {d} vs {r}");
        }
        // A region reaching no data is closed along it.
        let mut cut = v.clone();
        for y in 0..h {
            cut[y * w + 20] = f32::NAN;
        }
        let lines = contour_lines(&cut, w, h, 0.5);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.first() == l.last()));
        assert!(contour_lines(&v, w, h, 2.0).is_empty());
    }

    #[test]
    fn rgb_names() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(rgb_labels(&l(&["R", "G", "B"])));
        assert!(rgb_labels(&l(&["red", "Green", "BLUE"])));
        assert!(!rgb_labels(&l(&["SCI", "ERR", "DQ"])));
    }

    #[test]
    fn levelling() {
        // A tilted plane plus a bump, with a hole.
        let (w, h) = (6usize, 4usize);
        let mut v: Vec<f32> = (0..w * h)
            .map(|i| 3.0 + 0.5 * (i % w) as f32 - 0.25 * (i / w) as f32)
            .collect();
        v[7] = f32::NAN;
        let l = level_plane(&v, w);
        assert!(l[7].is_nan());
        assert!(
            l.iter().filter(|x| x.is_finite()).all(|x| x.abs() < 1e-5),
            "{l:?}"
        );
        let mut b = v.clone();
        b[14] += 2.0;
        let l = level_plane(&b, w);
        assert!(l[14] > 1.5);
        // One row: its mean.
        let l = level_plane(&[1.0, 2.0, 3.0], 3);
        assert!(l.iter().all(|x| x.abs() < 1e-6), "{l:?}");
        assert_eq!(level_plane(&[1.0, f32::NAN], 2)[0], 1.0);
    }
}
