//! Plain-language image metadata for Specular (DESIGN.md §6.7).
//!
//! [`read`] turns raw EXIF (TIFF payload), XMP, ICC and C2PA presence plus a few
//! file facts into a [`MetadataReport`]: a human-readable *summary* (ordered
//! groups of label/value items) and *all fields* (every raw tag, grouped).
//! It never touches the network or the place index.
//!
//! All text is localised: [`read`] takes a [`Localizer`] (Fluent catalogues in
//! `i18n/`, ICU4X for dates, numbers and country names). To switch language,
//! build a `Localizer` for the new language and call [`read`] again. Interpolated
//! values are wrapped in Unicode isolates (FSI … PDI) for right-to-left layouts;
//! see [`strip_isolates`].
//!
//! Place names come from [`geocode`] / [`attach_place`], which use the
//! `reverse_geocoder` crate and its bundled GeoNames data, offline. The index is
//! built lazily (a few hundred milliseconds); call [`warm_up_geocoder`] early on
//! a background thread to hide that. The About box must show
//! [`GEONAMES_ATTRIBUTION`] (or its translation, `Localizer::text("geonames-attribution")`).
//!
//! The API is format-neutral: any decoder that can hand over an EXIF TIFF block,
//! an XMP packet and an ICC profile can use it. [`jpeg::scan`] finds those in a
//! JPEG.

/// Builds `FluentArgs`: `args!("city" => name, "n" => 3)`.
macro_rules! args {
    ($($k:literal => $v:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut a = ::fluent_bundle::FluentArgs::new();
        $(a.set($k, $v);)*
        a
    }};
}

mod canon;
mod datetime;
pub mod edit;
mod exif_fields;
mod exif_tags;
mod fuji;
mod geo;
pub mod i18n;
mod icc;
mod iptc;
pub mod jpeg;
pub mod keywords;
mod lens;
mod panasonic;
mod samsung;
pub mod sidecars;
pub mod strings;
pub mod xmp;

use std::path::PathBuf;
use std::time::SystemTime;

use exif::{Context, In, Tag};

pub use fluent_bundle::FluentArgs;
pub use geo::{geocode, geocoder_ready, warm_up_geocoder};
pub use i18n::{Join, Localizer, langid, strip_isolates};
pub use strings::GEONAMES_ATTRIBUTION;
pub use unic_langid::LanguageIdentifier;

use datetime::Stamp;
use exif_fields::{ExifData, truncate};
use xmp::Xmp;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Facts about the file and its pixels that come from the container/decoder rather than metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileFacts {
    pub path: Option<PathBuf>,
    pub file_size: Option<u64>,
    pub modified: Option<SystemTime>,
    /// "JPEG", "PNG", "HEIC", …
    pub format_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Bits per sample.
    pub bit_depth: Option<u8>,
    /// "YCbCr", "RGB", "Greyscale", "CMYK", …
    pub colour_model: Option<String>,
    pub progressive: Option<bool>,
    /// A hierarchical JPEG (T.81 Annex J): named in place of baseline or progressive.
    pub hierarchical: bool,
    /// "4:2:0", "4:2:2", "4:4:4", …
    pub chroma_subsampling: Option<String>,
    /// How the samples carry transparency: "alpha" (straight or unstated), "premultiplied",
    /// "transparency" (palette entries or a colour key); `None` when opaque or unknown.
    pub alpha: Option<String>,
    /// The viewer draws the picture from what the file describes (a page, a drawing, a model,
    /// a font, a plot): no Camera group (its software is the File group's application) and no
    /// sample coding; the size is given only when the file states one (an SVG's).
    pub drawn: bool,
}

/// Raw metadata payloads, as found by the format decoder.
#[derive(Debug, Clone, Default)]
pub struct MetadataInput<'a> {
    /// TIFF-structured EXIF payload (in JPEG: after "Exif\0\0").
    pub exif_tiff: Option<&'a [u8]>,
    /// XMP packet bytes.
    pub xmp: Option<&'a [u8]>,
    /// ICC profile bytes.
    pub icc: Option<&'a [u8]>,
    pub has_c2pa: bool,
    pub file: FileFacts,
    /// Medical image facts (DICOM), shown as the first group with a not-for-diagnosis
    /// warning.
    pub medical: Option<MedicalFacts>,
    /// Scientific image facts (FITS, XISF), shown as the first group; the header keywords
    /// become an "all fields" group.
    pub science: Option<ScienceFacts>,
    /// Drawing facts (DXF), shown as the first group; the entity counts become an "all
    /// fields" group.
    pub drawing: Option<DrawingFacts>,
    /// Spectrum facts (JCAMP-DX, SPC, mzML, Bruker NMR), shown as the first group; the
    /// file's labelled fields come as [`MetadataInput::header`].
    pub spectrum: Option<SpectrumFacts>,
    /// 3D model facts (STL, OBJ, PLY, glTF, 3MF), shown as the first group.
    pub mesh: Option<MeshFacts>,
    /// Point cloud facts (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57), shown as the first group.
    pub points: Option<PointsFacts>,
    /// Gaussian splat facts (`.splat`, 3DGS PLY, `.spz`), shown as the first group.
    pub splats: Option<SplatFacts>,
    /// Research volume facts (NIfTI, Analyze, NRRD, MetaImage), shown after the medical group.
    pub volume: Option<VolumeFacts>,
    /// Geospatial facts (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX), shown as the
    /// first group; GeoKeys, GDAL metadata and the CRS definition become "all fields" groups.
    pub geo: Option<GeoFacts>,
    /// Reflectance transformation image facts (PTM, RTI, Relight), shown as the first group.
    pub rti: Option<RtiFacts>,
    /// Molecular structure facts (PDB, mmCIF, CIF, SDF, XYZ), shown as the first group.
    pub structure: Option<StructureFacts>,
    /// Font facts (TTF, OTF, TTC/OTC, WOFF, WOFF2), shown as the first group.
    pub font: Option<FontFacts>,
    /// Book facts (EPUB), shown as the first group.
    pub book: Option<BookFacts>,
    /// Office document facts (Word, Excel, PowerPoint, iWork, OpenDocument), shown as the
    /// first group.
    pub document: Option<DocumentFacts>,
    /// Neuroimaging surface, map and tractogram facts (GIFTI, CIFTI-2, TrackVis, MRtrix),
    /// each its own group, first.
    pub neuro: Option<NeuroFacts>,
    /// The header of a format without a metadata block of its own (DPX, Cineon, XWD, MIFF,
    /// TIM, WSQ, JBIG): its key facts as a group before the camera's, every field as an "all
    /// fields" group.
    pub header: Option<HeaderFacts>,
    /// IPTC-IIM datasets (JPEG APP13, TIFF tag 33723, PSD image resource 1028).
    pub iptc: Option<&'a [u8]>,
    /// Descriptive tags from outside Exif, XMP and IPTC (PNG text chunks, JPEG and GIF
    /// comments, QuickTime / MP4 and Matroska tags): the summary reads them where the XMP has
    /// none, and each becomes an "all fields" group.
    pub tags: Vec<TagFacts>,
}

/// Descriptive tags of a container, for [`MetadataInput::tags`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TagFacts {
    /// What they are ("PNG text", "QuickTime", "Matroska"…): names the all-fields group.
    pub kind: String,
    /// The tags as XMP properties, read where the XMP has none: `dc:title`, `dc:description`,
    /// `dc:creator`, `dc:rights`, `dc:subject` (one entry per keyword),
    /// `photoshop:DateCreated` and `xmp:CreateDate` (ISO 8601), `xmp:CreatorTool`,
    /// `tiff:Make`, `tiff:Model`, `exif:GPSLatitude` / `exif:GPSLongitude` ("48.8584N"),
    /// `exif:GPSAltitude` (metres), `xmp:Rating`, and `exif:UserComment` (a comment).
    pub props: Vec<(String, String)>,
    /// Every tag as (name, value), in file order.
    pub fields: Vec<(String, String)>,
}

/// What a file's own header says (DPX timecode and edge code, a MIFF comment, a WSQ
/// NISTCOM record).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeaderFacts {
    /// "DPX", "Cineon"…: names the all-fields group.
    pub format: String,
    /// Key facts by stable key, in display order. Known keys: "created", "creator",
    /// "copyright", "project", "comment", "device", "timecode", "edge-code", "frame-rate",
    /// "frame", "film", "transfer", "colorimetric", "window", "resolution", "compression",
    /// "profile", "level", "decomposition", "colour-transform", "artboards", "not-drawn",
    /// "approximated", and for text files "encoding", "lines", "line-ends", "words", "pages",
    /// "pictures", "pictures-not-shown", "tables", "rows", "columns", "delimiter",
    /// "column-names", "cues", "cue-span"; others are skipped.
    pub summary: Vec<(&'static str, String)>,
    /// Every header field as (name, value), in file order.
    pub fields: Vec<(String, String)>,
}

/// What the info panel shows about HCP-style neuroimaging files.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NeuroFacts {
    pub gifti: Option<GiftiFacts>,
    pub cifti: Option<CiftiFacts>,
    pub tracts: Option<TractFacts>,
}

/// A GIFTI file (`.surf.gii`, `.func.gii`, `.shape.gii`, `.label.gii`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GiftiFacts {
    /// Empty for the surface a CIFTI file is shown on.
    pub version: String,
    pub arrays: u64,
    /// "surface", "labels" or "data" (empty: not said).
    pub role: String,
    pub vertices: Option<u64>,
    pub triangles: Option<u64>,
    /// e.g. "CortexLeft".
    pub structure: Option<String>,
    /// A surface's geometric type, e.g. "Anatomical".
    pub geometry: Option<String>,
    /// The array shown: its name, intent ("NIFTI_INTENT_SHAPE") and data type.
    pub array: Option<String>,
    pub intent: Option<String>,
    pub data_type: Option<String>,
    /// e.g. "GZipBase64Binary".
    pub encoding: String,
    pub labels: u64,
    /// The vertices' coordinate transform: from and to space, and whether it was applied.
    pub transform: Option<(String, String, bool)>,
    /// The surface file a data file is shown on.
    pub surface: Option<String>,
}

/// A CIFTI-2 file (`.dscalar.nii`, `.dlabel.nii`, `.dtseries.nii`, `.pscalar.nii`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CiftiFacts {
    pub version: String,
    /// e.g. "ConnDenseScalar".
    pub intent: String,
    pub shape: [u64; 2],
    /// "scalars", "labels", "series", "parcels", "brain-models".
    pub map_kind: String,
    pub maps: u64,
    /// A series' start, step and unit ("SECOND").
    pub series: Option<(f64, f64, String)>,
    pub models: u64,
    pub parcels: Option<u64>,
    pub voxels: u64,
    /// e.g. "CORTEX_LEFT".
    pub structures: Vec<String>,
    /// Structure, vertices and the surface file found for it.
    pub surfaces: Vec<(String, u64, Option<String>)>,
    /// "surfaces" or "voxels".
    pub shown: String,
    pub maps_left_out: u64,
    /// Dense connectivity (`.dconn.nii`): the seeds (rows) and the bytes read per row.
    pub dense: Option<(u64, u64)>,
}

/// A tractogram (TrackVis `.trk`, MRtrix `.tck`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TractFacts {
    /// e.g. "TrackVis 2", "MRtrix".
    pub format: String,
    pub streamlines: u64,
    pub points: u64,
    pub shown_streamlines: u64,
    pub scalars: Vec<String>,
    pub properties: Vec<String>,
    pub dims: Option<[i64; 3]>,
    /// Millimetres.
    pub voxel_size: Option<[f64; 3]>,
    pub voxel_order: Option<String>,
    /// "vox_to_ras", "voxel order" or "scanner".
    pub space: String,
    /// As written, millimetres.
    pub step: Option<String>,
    /// Millimetres.
    pub mean_length: Option<f64>,
}

/// What the info panel shows about a reflectance transformation image (PTM, HSH RTI,
/// Relight).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RtiFacts {
    /// The container: "PTM_1.2", "RTI", "Relight".
    pub format: String,
    /// The reflectance model: "PTM", "HSH", "SH", "RBF", "Bilinear".
    pub basis: String,
    /// The basis order (HSH and SH), when it has one.
    pub order: Option<u32>,
    /// Coefficients per colour channel.
    pub terms: u32,
    /// How colour is coded: "LRGB", "RGB", "YCC", …
    pub colour: String,
    /// Coefficient planes (byte images) stored.
    pub planes: u32,
    /// Size of the coefficient planes.
    pub width: u32,
    pub height: u32,
    /// How the planes are stored ("JPEG, quality 95"), when compressed.
    pub compression: Option<String>,
    /// The byte-to-coefficient mapping, value = (byte − bias) × scale, per coefficient, as
    /// the file states it.
    pub scale: Vec<f32>,
    pub bias: Vec<f32>,
}

/// What the info panel shows about where an image or a map sits on the Earth.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeoFacts {
    /// The coordinate reference system's name, e.g. "WGS 84 / UTM zone 33N".
    pub crs: Option<String>,
    pub epsg: Option<u32>,
    /// How the CRS was found, or why it was not (".prj missing").
    pub crs_note: Option<String>,
    pub datum: Option<String>,
    /// The map projection's method, e.g. "Transverse Mercator".
    pub projection: Option<String>,
    /// The projection's method is not supported here (no longitude and latitude).
    pub projection_unsupported: bool,
    /// How the datum relates to WGS 84: "same", "helmert", "by-area" (one Helmert per area
    /// of use) or "unknown".
    pub to_wgs84: Option<&'static str>,
    /// The linear unit of a projected CRS ("metre", "US survey foot"); `None` for degrees.
    pub unit: Option<String>,
    /// Map bounds: min x, min y, max x, max y, in the CRS's units (degrees when geographic).
    pub bounds: Option<[f64; 4]>,
    /// The bounds' coordinates are longitude and latitude.
    pub geographic: bool,
    /// The bounds in longitude and latitude (west, south, east, north) of a projected CRS.
    pub lonlat: Option<[f64; 4]>,
    /// Pixel size in map units (rasters).
    pub pixel_size: Option<(f64, f64)>,
    /// Pixel size of each level, full resolution first (rasters with overviews).
    pub levels: Vec<(u32, u32)>,
    pub nodata: Option<f64>,
    /// The vertical CRS or datum.
    pub vertical: Option<String>,
    /// The GeoTIFF citation.
    pub citation: Option<String>,
    /// Pixel values are areas (false) or points.
    pub pixel_is_point: bool,
    /// Vector layers.
    pub layers: Vec<GeoLayer>,
    pub features: Option<u64>,
    pub vertices: Option<u64>,
    /// The attributes' text encoding.
    pub encoding: Option<String>,
    /// A tile pyramid: table, zoom level shown, tiles at that level.
    pub tiles: Option<(String, i64, u64)>,
    pub warnings: Vec<String>,
    /// The companion files the georeference came from ("scene.jgw", "scene.prj").
    pub sidecars: Vec<String>,
    /// The credit line a tile set asks to be shown with it.
    pub attribution: Option<String>,
    /// "All fields" groups: kind ("geokeys", "header", "gdal", "wkt", "projjson", "tileset") and
    /// (tag, value).
    pub fields: Vec<(&'static str, Vec<(String, String)>)>,
}

/// One vector layer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeoLayer {
    pub name: String,
    pub features: u64,
    /// Geometry types ("Polygon", "LineString", …) and their counts, most first.
    pub types: Vec<(String, u64)>,
    /// Attribute fields.
    pub fields: u64,
}

/// Survey data (LandXML): the coordinate system, TIN surfaces and alignments, in the file's
/// linear unit (`unit` is its symbol, "m" or "ft").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurveyFacts {
    pub crs: Option<String>,
    pub unit: String,
    pub application: Option<String>,
    pub project: Option<String>,
    pub surfaces: Vec<SurveySurface>,
    pub alignments: Vec<SurveyAlignment>,
}

/// One TIN surface's counts and measures.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurveySurface {
    pub name: String,
    pub points: u64,
    pub triangles: u64,
    pub area_2d: f64,
    pub area_3d: f64,
    pub volume: f64,
    pub z_min: f64,
    pub z_max: f64,
    pub contour_interval: f64,
}

/// One alignment: length and stations as written ("1+234.50").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurveyAlignment {
    pub name: String,
    pub length: f64,
    pub sta_start: String,
    pub sta_end: String,
}

/// A font a drawing names that was not found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MissingFontFact {
    /// The file (or TrueType family) asked for: "romans.shx".
    pub file: String,
    /// The text styles that name it.
    pub styles: Vec<String>,
    /// The family drawing the text instead; empty when only a Big Font is missing (its
    /// characters come from the fallback fonts).
    pub substitute: String,
}

/// What the info panel shows about a drawing (DXF).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawingFacts {
    /// "DXF".
    pub format: String,
    /// The version as stored and the release it stands for, e.g. "AC1032 (AutoCAD 2018)".
    pub version: String,
    /// The drawing-units code (DXF `$INSUNITS`: 0 unitless, 1 inches, 4 millimetres, …).
    pub units: i16,
    /// The unit's symbol for sizes ("mm"), when it has one.
    pub unit_symbol: Option<String>,
    /// Width and height of what is drawn in model space, in drawing units.
    pub extents: Option<(f64, f64)>,
    /// The sheet shown: `None` for model space, else the layout's name.
    pub layout: Option<String>,
    /// Paper-space layouts besides model space.
    pub layouts: u32,
    /// Layers, and how many of them the file has off or frozen.
    pub layers: u32,
    pub layers_off: u32,
    /// Entities by type as stored (blocks counted once), and in all.
    pub counts: Vec<(String, u64)>,
    /// Text code page for files before AutoCAD 2007 (`$DWGCODEPAGE`).
    pub codepage: Option<String>,
    /// Fonts the drawing names that are not what draws its text (SHX fonts not found,
    /// TrueType fonts not installed), each with its styles and what stands in for it.
    pub missing_fonts: Vec<MissingFontFact>,
    /// The SHX font files that draw the text, as found (the drawing's folder or the
    /// user's SHX font folders).
    pub shx_fonts: Vec<String>,
    /// Problems that did not stop reading (entities skipped, limits reached).
    pub warnings: Vec<String>,
    /// What a plot file (HP-GL) adds; `None` for CAD drawings.
    pub plot: Option<PlotFacts>,
    /// What an IC layout (GDSII) adds; `None` otherwise.
    pub ic: Option<IcLayoutFacts>,
    /// What a board or schematic (Gerber, Excellon, KiCad) adds; `None` otherwise.
    pub pcb: Option<PcbFacts>,
    /// What survey data (LandXML) adds; `None` otherwise.
    pub survey: Option<SurveyFacts>,
}

/// The facts of a circuit board or schematic the info panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PcbFacts {
    /// A schematic (else a board, a Gerber or drill file, or a board's Gerber set).
    pub schematic: bool,
    /// The board view or schematic sheet shown, already in the reader's language.
    pub view: Option<String>,
    pub generator: Option<String>,
    pub project: Option<String>,
    pub revision: Option<String>,
    pub date: Option<String>,
    /// Board width × height and thickness, millimetres.
    pub size_mm: Option<(f64, f64)>,
    pub thickness_mm: Option<f64>,
    pub copper_layers: u32,
    /// The layers or files with their roles in English ("Top copper"), in stack order.
    pub layers: Vec<(String, String)>,
    pub holes: u64,
    pub drill_sizes: u32,
    pub components: u64,
    pub nets: u64,
    pub tracks: u64,
    pub vias: u64,
    pub sheets: u32,
    pub apertures: u32,
    pub macros: u32,
}

/// The facts of a spectrum file (JCAMP-DX, SPC, mzML, Bruker NMR) and the page shown.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpectrumFacts {
    pub format: String,
    pub version: String,
    /// What was measured, in English ("Infrared", "NMR").
    pub technique: String,
    pub title: Option<String>,
    pub origin: Option<String>,
    pub instrument: Option<String>,
    pub date: Option<String>,
    pub spectra: u64,
    pub points: u64,
    pub nucleus: Option<String>,
    pub frequency_mhz: Option<f64>,
    pub solvent: Option<String>,
    pub pulse_program: Option<String>,
    /// The page shown (from 1), of how many, and its name.
    pub page: u32,
    pub pages: u32,
    pub page_name: String,
    /// The page's axes: title and unit.
    pub x_axis: (String, String),
    pub y_axis: (String, String),
    /// How the page was made (an automatic phase correction), English.
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
}

/// The facts of an IC layout (GDSII) the info panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IcLayoutFacts {
    /// The library name.
    pub library: String,
    /// The database unit and the user unit, metres.
    pub db_unit_m: f64,
    pub user_unit_m: f64,
    /// The top cell shown (the page).
    pub cell: String,
    /// Cells defined, and the top cells (not placed by any other).
    pub cells: u64,
    pub top_cells: Vec<String>,
    /// Layer/datatype pairs used.
    pub layers: u32,
    /// Boundaries, paths and boxes; texts; references (SREF and AREF) and the placements
    /// they make (an AREF places columns × rows).
    pub shapes: u64,
    pub texts: u64,
    pub references: u64,
    pub placements: u64,
    /// Shapes with every reference expanded, over all top cells.
    pub flat_shapes: u64,
    /// The bounds of the cell shown, micrometres: x0, y0, x1, y1.
    pub bounds_um: Option<[f64; 4]>,
}

/// The facts of a plot file (HP-GL, HP-GL/2) the info panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlotFacts {
    /// The paper the plot is laid out on, millimetres, when the file sets it.
    pub paper_mm: Option<(f64, f64)>,
    /// Pens that draw, by number.
    pub pens: Vec<u32>,
    /// The page shown (from 1) and how many there are.
    pub page: u32,
    pub pages: u32,
    /// The printer job it came in ("PCL 5"), if any.
    pub job: Option<String>,
}

/// What the info panel shows about a 3D model (STL, OBJ, PLY, glTF, 3MF).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshFacts {
    /// "STL", "glTF", …
    pub format: String,
    pub triangles: u64,
    pub vertices: u64,
    /// The bounding box's size along x, y and z, in the file's unit.
    pub size: [f64; 3],
    /// The unit's symbol ("mm", "m", "in"), when the file states one.
    pub unit: Option<String>,
    /// The up axis, "Z" or "Y".
    pub up: Option<String>,
    /// Material names, in file order (unnamed ones left out).
    pub materials: Vec<String>,
    /// Textures the file uses, and how many of those could not be read.
    pub textures: usize,
    pub textures_missing: usize,
    /// The model's name, when the file gives one.
    pub title: Option<String>,
    /// The program that wrote the file.
    pub generator: Option<String>,
}

/// What the info panel shows about a font face (TTF, OTF, TTC/OTC, WOFF, WOFF2).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontFacts {
    /// From the name table: family and style (typographic names where given), version,
    /// designer, foundry, licence, copyright, trademark.
    pub family: Option<String>,
    pub style: Option<String>,
    pub version: Option<String>,
    pub designer: Option<String>,
    pub manufacturer: Option<String>,
    pub licence: Option<String>,
    pub licence_url: Option<String>,
    pub copyright: Option<String>,
    pub trademark: Option<String>,
    /// The face's place in a collection (from 0) and the collection's size.
    pub face: Option<(u64, u64)>,
    pub glyphs: u64,
    /// "truetype", "cff", "cff2", "bitmap" or "none".
    pub outlines: &'static str,
    /// OS/2 fsType: "installable", "restricted", "preview-print" or "editable" (`None`
    /// without an OS/2 table), and its two extra bits.
    pub embedding: Option<&'static str>,
    pub no_subsetting: bool,
    pub bitmap_embedding_only: bool,
    /// Variation axes: tag, name, minimum, default, maximum.
    pub axes: Vec<(String, String, f64, f64, f64)>,
    /// Named instances of a variable font.
    pub instances: u64,
    /// Colour glyph tables: "colr0", "colr1", "sbix", "cbdt", "svg".
    pub colour: Vec<&'static str>,
    /// Table tags, in file order.
    pub tables: Vec<String>,
    pub units_per_em: u64,
    /// OS/2 weight class (100-900).
    pub weight: Option<u64>,
}

/// What the info panel shows about a book (EPUB).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookFacts {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub date: Option<String>,
    pub identifier: Option<String>,
    pub subjects: Vec<String>,
    pub rights: Option<String>,
    /// "2.0", "3.0"...
    pub version: Option<String>,
    /// Pre-paginated (fixed layout) rather than reflowable.
    pub fixed_layout: bool,
    /// Pages turn right to left.
    pub right_to_left: bool,
    /// Entries in the table of contents.
    pub contents: u64,
    /// Pages as laid out here.
    pub pages: u64,
    /// Fonts the book embeds, and how many of them were obfuscated.
    pub fonts: u64,
    pub obfuscated_fonts: u64,
}

/// What the info panel shows about an office document (Word, Excel, PowerPoint, Pages,
/// Numbers, Keynote, OpenDocument). Dates are ISO 8601 as the file gives them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocumentFacts {
    /// "word", "excel", "powerpoint", "pages", "numbers", "keynote", "odf-text",
    /// "odf-spreadsheet", "odf-presentation", "odf-graphics", "odf-other" (each has a
    /// `document-kind-<key>` message).
    pub kind: String,
    /// An Office 97–2003 file.
    pub legacy: bool,
    pub template: bool,
    pub macros: bool,
    /// The stored preview's image format ("JPEG"); `None`: there is none.
    pub preview: Option<String>,
    pub title: Option<String>,
    pub subject: Option<String>,
    pub author: Option<String>,
    pub keywords: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub last_modified_by: Option<String>,
    pub revision: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub printed: Option<String>,
    pub application: Option<String>,
    pub company: Option<String>,
    pub template_name: Option<String>,
    pub pages: Option<u64>,
    pub words: Option<u64>,
    pub characters: Option<u64>,
    pub paragraphs: Option<u64>,
    pub lines: Option<u64>,
    pub slides: Option<u64>,
    pub notes: Option<u64>,
    pub hidden_slides: Option<u64>,
    pub sheets: Option<u64>,
    /// Total editing time in minutes.
    pub edit_minutes: Option<u64>,
    /// How many items a playlist lists (`kind` "playlist", which has no preview row).
    pub items: Option<u64>,
}

/// What the info panel shows about a molecular structure (PDB, mmCIF, CIF, SDF, XYZ).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StructureFacts {
    /// PDB ID, CIF data block name or the record's title line.
    pub id: Option<String>,
    pub title: Option<String>,
    /// PDB HEADER classification or the compound's name.
    pub classification: Option<String>,
    /// Experimental method(s): "X-RAY DIFFRACTION", "ELECTRON MICROSCOPY"…
    pub method: Option<String>,
    /// Resolution in ångströms.
    pub resolution: Option<f64>,
    /// Deposition date, as the file gives it.
    pub date: Option<String>,
    /// Chemical formula (small-molecule CIF).
    pub formula: Option<String>,
    pub chains: u64,
    /// Polymer residues.
    pub residues: u64,
    /// Atoms shown (after filling a small-molecule unit cell), and those of the asymmetric
    /// unit (0 when the cell was not filled).
    pub atoms: u64,
    pub asymmetric_atoms: u64,
    pub bonds: u64,
    /// Models (NMR ensembles), and records (SDF) or data blocks (CIF): the first is shown.
    pub models: u64,
    pub records: u64,
    /// Ligand (non-polymer, non-water) residue names.
    pub ligands: Vec<String>,
    /// a, b, c (Å), α, β, γ (°).
    pub cell: Option<[f64; 6]>,
    /// Hermann–Mauguin symbol.
    pub space_group: Option<String>,
    /// Where the secondary structure came from, for polymers.
    pub secondary: Option<StructureSecondary>,
}

/// The source of a structure's secondary structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureSecondary {
    /// HELIX / SHEET records or their mmCIF categories.
    File,
    /// Assigned from the backbone's hydrogen bonds.
    Computed,
}

/// What the info panel shows about a point cloud (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PointsFacts {
    /// "LAS 1.4", "LAZ 1.4 (COPC)", "PCD 0.7", "E57 1.0", …
    pub format: String,
    /// LAS point data record format (0–10).
    pub point_format: Option<u8>,
    /// The compressor or encoding ("LASzip 3.4 r3, layered", PCD's "binary_compressed").
    pub coding: Option<String>,
    /// Points in the file, and those read (fewer when the cloud was evenly subsampled).
    pub total: u64,
    pub loaded: u64,
    /// The box's size along x, y and z, in the file's units.
    pub size: Option<[f64; 3]>,
    /// The coordinate reference system's name.
    pub crs: Option<String>,
    /// What the points carry besides positions: "RGB", "intensity", "classification",
    /// "returns".
    pub channels: Vec<&'static str>,
    /// ASPRS classes present (LAS), with their point counts, ascending.
    pub classes: Vec<(u8, u64)>,
    /// E57 scans: name (when given) and points.
    pub scans: Vec<(Option<String>, u64)>,
}

/// What the info panel shows about Gaussian splats (`.splat`, 3DGS PLY, `.spz`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SplatFacts {
    /// "SPLAT", "3DGS PLY", "Compressed PLY", "SPZ".
    pub format: String,
    /// Splats in the file, and those shown (fewer when the file was evenly thinned or some
    /// had non-finite values).
    pub total: u64,
    pub shown: u64,
    /// The degree of the view-dependent colour (0: none).
    pub sh_degree: u8,
    /// The size of the box holding all but the outermost splats, along x, y and z.
    pub size: Option<[f64; 3]>,
    /// The file's up axis ("−Y", "Y").
    pub up: Option<String>,
    /// Trained with anti-aliasing (`.spz`).
    pub antialiased: bool,
}

/// What the info panel shows about a research volume (NIfTI, Analyze 7.5, NRRD,
/// MetaImage).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VolumeFacts {
    /// "NIfTI-1", "NIfTI-2", "Analyze 7.5", "NRRD", "MetaImage".
    pub format: String,
    /// Every axis's size as the file lists them.
    pub dims: Vec<u64>,
    /// Millimetres between voxel centres along the stored axes.
    pub voxel_size: Option<[f64; 3]>,
    /// Stored data type, e.g. "int16", "float32", "RGB".
    pub data_type: String,
    /// Volumes (time points).
    pub volumes: u64,
    /// Where the orientation came from: "sform", "qform", "none", "analyze", "space",
    /// "spacings", "transform".
    pub orientation: String,
    /// The file states where its axes point in the patient.
    pub reliable: bool,
    pub description: Option<String>,
    /// What the values mean (NIfTI intent).
    pub intent: Option<String>,
    /// "gzip", "zlib", "ASCII", …; `None` for raw.
    pub compression: Option<String>,
    /// Other facts the header states, under stable keys: "tr", "te", "ti", "flip", "fov",
    /// "field", "pedir", "cmdline", "xform" (FreeSurfer MGH); "content" ("projections" or
    /// "reconstruction"), "projections", "angles", "angle-step", "kv", "current", "power",
    /// "exposure", "sod", "odd", "sdd", "magnification", "binning", "objective", "filter",
    /// "camera", "instrument", "software", "date", "ring", "smoothing", "coords" (industrial
    /// CT); unknown keys are not shown.
    pub params: Vec<(String, VolumeParam)>,
}

/// One of [`VolumeFacts::params`].
#[derive(Debug, Clone, PartialEq)]
pub enum VolumeParam {
    /// A number and its unit symbol ("ms", "°", "mm", "T"; empty for none).
    Number(f64, String),
    Text(String),
}

/// What the info panel shows about a scientific image (FITS, XISF). Coordinates come
/// pre-formatted (sexagesimal and degrees are not localised).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScienceFacts {
    /// "FITS" or "XISF".
    pub format: String,
    pub object: Option<String>,
    pub telescope: Option<String>,
    pub instrument: Option<String>,
    pub filter: Option<String>,
    /// As written, e.g. "2021-03-04T05:06:07".
    pub date_obs: Option<String>,
    /// Seconds.
    pub exposure: Option<f64>,
    /// Axis lengths, first (x) to last.
    pub dims: Vec<u64>,
    /// Sample format, e.g. "32-bit float".
    pub sample: Option<String>,
    /// Physical unit of the values (BUNIT).
    pub unit: Option<String>,
    /// The FITS extension (HDU) or XISF image shown, by name, when the file names it.
    pub hdu: Option<String>,
    /// Sky position of the image centre, e.g. ("RA 10h 00m 00.0s  Dec +02° 12′ 00″",
    /// "150.0000°, +2.2000°").
    pub centre: Option<(String, String)>,
    /// Coordinate frame, e.g. "ICRS", "FK5 J2000", "Galactic".
    pub frame: Option<String>,
    /// Projection code, e.g. "TAN".
    pub projection: Option<String>,
    /// Arcseconds per pixel.
    pub pixel_scale: Option<f64>,
    /// The distortions the world coordinates go through, e.g. "SIP, D2IM, CPDIS".
    pub distortion: Option<String>,
    /// Why the file's world coordinates cannot be followed (technical, e.g. "projection
    /// TNX"); the sky position, frame and scale are then left out.
    pub wcs_unsupported: Option<String>,
    /// The HDF5 dataset or NetCDF variable shown, e.g. "tas — Near-Surface Air Temperature".
    pub dataset: Option<String>,
    /// Physical size of a pixel (x, y, unit), e.g. (0.25, 0.25, "nm") for an EMD or
    /// DigitalMicrograph image.
    pub pixel_size: Option<(f64, f64, String)>,
    /// Header keywords (name, value, comment), in file order.
    pub keywords: Vec<(String, String, String)>,
    /// The field the image comes from: decides the group's title and labels.
    pub domain: ScienceDomain,
    /// The image's or channel's title (microscopy).
    pub title: Option<String>,
    /// Accelerating voltage, volts (electron microscopy).
    pub voltage: Option<f64>,
    /// Indicated magnification.
    pub magnification: Option<f64>,
    /// Operation or imaging mode ("TEM", "STEM", "AC Mode").
    pub mode: Option<String>,
    /// Camera or detector.
    pub detector: Option<String>,
    /// Physical size of the image, pre-formatted (e.g. "2 µm × 2 µm").
    pub field_of_view: Option<String>,
    /// Who made the slide format ("Aperio", "Hamamatsu"; whole-slide images).
    pub vendor: Option<String>,
    /// Pyramid levels, full resolution first (whole-slide images).
    pub levels: Vec<(u32, u32)>,
    /// Who captured it (SER).
    pub observer: Option<String>,
    /// The colour filter array of a one-shot colour camera's raw frames, e.g. "RGGB".
    pub bayer: Option<String>,
    /// Frames of a video (SER), and the count its header declares when the file is cut short.
    pub frames: Option<u64>,
    pub frames_declared: Option<u64>,
    /// First and last frame's capture time (UTC), pre-formatted.
    pub capture_start: Option<String>,
    pub capture_end: Option<String>,
    /// Mean frames per second from the frame timestamps.
    pub frame_rate: Option<f64>,
    /// The experiment geometry of an X-ray diffraction frame.
    pub diffraction: Option<DiffractionFacts>,
    /// Annotations from a sidecar beside the slide (NDPA, Aperio, ASAP).
    pub annotations: Option<AnnotationFacts>,
    /// The ASDF file the image or its metadata comes from (an `.asdf` file, a JWST FITS
    /// file's `ASDF` extension).
    pub asdf: Option<AsdfFacts>,
    /// The CASA image the planes come from (radio interferometry).
    pub casa: Option<CasaFacts>,
    /// The unstructured mesh the image paints (UGRID, ICON, MPAS).
    pub ugrid: Option<UgridFacts>,
    /// The sky position, frame and scale approximate the file's world coordinates (a TAN
    /// projection from an ASDF pointing; the GWCS distortion left out).
    pub wcs_approximate: bool,
    /// A thermal still's camera and object parameters.
    pub thermal: Option<ThermalFacts>,
    /// The CERN ROOT file and object the image comes from.
    pub root: Option<RootFacts>,
    /// What an HDF4 file holds and its HDF-EOS structures.
    pub hdf4: Option<Hdf4Facts>,
    /// The OMERO server and image an image was opened from.
    pub omero: Option<OmeroFacts>,
    /// A planetary image's observation and map projection (VICAR, PDS3, PDS4).
    pub planetary: Option<PlanetaryFacts>,
}

/// What a planetary image's label says about the observation and its map projection, for its
/// own group. Text as the label writes it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanetaryFacts {
    pub target: Option<String>,
    pub mission: Option<String>,
    pub spacecraft: Option<String>,
    pub instrument: Option<String>,
    pub start: Option<String>,
    pub stop: Option<String>,
    pub product_id: Option<String>,
    /// "SIMPLE CYLINDRICAL", "Equirectangular"…
    pub projection: Option<String>,
    pub metres_per_pixel: Option<f64>,
    pub pixels_per_degree: Option<f64>,
    /// Latitude and longitude of the projection centre, degrees.
    pub centre: Option<(f64, f64)>,
    /// (south, north) and (west, east) bounds, degrees.
    pub latitudes: Option<(f64, f64)>,
    pub longitudes: Option<(f64, f64)>,
    /// The body's equatorial radius, kilometres.
    pub radius_km: Option<f64>,
}

/// An image on an OMERO server (or the IDR), for its own group.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OmeroFacts {
    /// The server's address.
    pub server: String,
    /// Signed in as (`None`: browsed publicly).
    pub user: Option<String>,
    pub image_id: u64,
    pub name: String,
    pub project: Option<String>,
    pub dataset: Option<String>,
    pub screen: Option<String>,
    pub plate: Option<String>,
    pub well: Option<String>,
    pub owner: Option<String>,
    pub group: Option<String>,
    /// Acquisition date as the server gives it.
    pub acquired: Option<String>,
    /// X, Y, Z, C, T.
    pub size: [u64; 5],
    pub pixels_type: String,
    /// Physical pixel size along X, Y, Z: value and unit symbol.
    pub physical: [Option<(f64, String)>; 3],
    /// Channel names and whether each is shown.
    pub channels: Vec<(String, bool)>,
    /// Tile width and height and pyramid levels of a tiled image.
    pub tiles: Option<(u32, u32, u32)>,
    /// "JPEG" or "PNG".
    pub encoding: String,
}

/// A CERN ROOT file and the object shown, for their own group.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RootFacts {
    /// The ROOT version that wrote the file, e.g. "6.28/04".
    pub version: String,
    /// e.g. "zlib level 4".
    pub compression: String,
    pub keys: u64,
    /// The object's class ("TH2F") and path in the file.
    pub class: String,
    pub path: String,
    pub title: String,
    pub entries: Option<f64>,
    pub sum_weights: Option<f64>,
    /// x, y, z.
    pub axes: Vec<RootAxisFacts>,
    /// A graph's point count.
    pub points: Option<u64>,
    /// The file's trees by path, with their entry counts (-1 when unread).
    pub trees: Vec<(String, i64)>,
}

/// A histogram axis.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RootAxisFacts {
    pub title: String,
    pub unit: String,
    pub bins: u32,
    pub min: f64,
    pub max: f64,
    pub variable: bool,
    pub mean_rms: Option<(f64, f64)>,
}

/// An ASDF file's versions and data model, for its own group.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AsdfFacts {
    /// File format version, e.g. "1.0.0".
    pub version: String,
    /// ASDF Standard version, e.g. "1.5.0".
    pub standard: Option<String>,
    /// The library that wrote the file, e.g. "asdf 3.5.0".
    pub library: Option<String>,
    /// The data model, e.g. "wfi_image".
    pub model: Option<String>,
}

/// A thermal still's camera and the object parameters its temperatures were computed with.
/// Temperatures are in `unit` (the viewer's temperature unit), already converted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThermalFacts {
    /// "FLIR radiometric JPEG", "InfraTec IRB".
    pub format: String,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub emissivity: Option<f64>,
    /// Metres.
    pub distance: Option<f64>,
    pub reflected: Option<f64>,
    pub atmospheric: Option<f64>,
    /// A fraction.
    pub humidity: Option<f64>,
    /// The smallest and largest stored count.
    pub raw_range: Option<(u16, u16)>,
    /// The coldest and hottest pixel.
    pub temperature_range: Option<(f64, f64)>,
    /// "°C", "°F" or "K".
    pub unit: String,
}

/// A CASA image's type, beams, spectral frame and storage, for its own group.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CasaFacts {
    /// What the directory name says it is: "image", "residual", "model", "pb", "mask",
    /// "psf", "sumwt", "weight" or "other".
    pub image_type: String,
    /// The image's own type keyword, e.g. "Intensity".
    pub info_type: Option<String>,
    /// Restoring beam: major and minor axis (arcsec), position angle (degrees).
    pub beam: Option<[f64; 3]>,
    /// Per-plane beams: how many, and the range of their major axes (arcsec).
    pub plane_beams: Option<(usize, (f64, f64))>,
    /// Hz.
    pub rest_frequency: Option<f64>,
    /// Spectral reference frame, e.g. "LSRK".
    pub spectral_frame: Option<String>,
    /// Velocity definition of the frequency labels: "radio", "optical" or "relativistic".
    pub velocity: Option<String>,
    /// The default mask, e.g. "mask0".
    pub mask: Option<String>,
    /// Storage manager, e.g. "TiledCellStMan".
    pub storage: String,
    pub big_endian: bool,
}

/// An unstructured mesh (UGRID, ICON, MPAS), for its own group.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UgridFacts {
    /// The topology's name, with its long name when the file gives one.
    pub name: String,
    /// "UGRID", "ICON" or "MPAS".
    pub convention: String,
    pub nodes: u64,
    pub faces: u64,
    pub edges: u64,
    pub triangles: u64,
    pub quads: u64,
    /// Faces of five or more nodes, and the most nodes of a face.
    pub polygons: u64,
    pub max_face_nodes: u32,
    /// Longitude and latitude; else projected x and y in `unit`.
    pub geographic: bool,
    pub unit: Option<String>,
    /// Where the values sit: `Some(true)` the faces, `Some(false)` the nodes, `None` a
    /// mesh without data (the image counts each face's nodes).
    pub on_faces: Option<bool>,
    /// Faces split at the antimeridian or closed to a pole for drawing.
    pub split_faces: u64,
}

/// An X-ray diffraction frame's experiment geometry, as the header gives it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiffractionFacts {
    /// Å.
    pub wavelength: Option<f64>,
    /// Photon energy, keV.
    pub energy: Option<f64>,
    /// Sample to detector distance, mm.
    pub distance: Option<f64>,
    /// Beam centre (x, y), pixels from the image's top-left corner.
    pub beam: Option<(f64, f64)>,
    /// Detector swing, degrees.
    pub two_theta: Option<f64>,
    /// Goniometer angles, degrees.
    pub phi: Option<f64>,
    pub omega: Option<f64>,
    pub chi: Option<f64>,
    pub kappa: Option<f64>,
    /// Rotation start and width (degrees) and the axis' name.
    pub oscillation: Option<(f64, f64, Option<String>)>,
    /// The count at which a pixel saturates.
    pub overload: Option<f64>,
    /// d (Å) at the detector's edge nearest the beam and in its farthest corner.
    pub resolution: Option<(f64, f64)>,
    /// Whether q, d and rings follow (complete header, flat detector).
    pub complete: bool,
    /// A curved image plate (R-AXIS RAPID).
    pub curved: bool,
    /// Detector pixels summed into one (x, y).
    pub binning: Option<(u32, u32)>,
}

/// A slide's annotation sidecar, for the info panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnnotationFacts {
    /// The sidecar's file name.
    pub file: String,
    /// "Aperio ImageScope", "ASAP", "NDP.view (NDPA)".
    pub format: String,
    pub items: Vec<AnnotationItem>,
    /// Shapes or points left out (over the reader's limits, or unreadable).
    pub skipped: usize,
}

/// One annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationItem {
    pub name: String,
    pub group: String,
    /// "polygon", "polyline", "rectangle", "ellipse", "point", "points", "ruler", "arrow".
    pub shape: String,
    /// Its area or length, pre-formatted, when the pixel size is known.
    pub measure: Option<String>,
}

/// What an HDF4 file holds, for the info panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hdf4Facts {
    /// The library version the file was written with, as stored.
    pub library: Option<String>,
    pub datasets: u64,
    pub images: u64,
    pub vgroups: u64,
    pub vdatas: u64,
    pub eos: Option<HdfEosFacts>,
}

/// The HDF-EOS2 structures of a file and the one the image belongs to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HdfEosFacts {
    pub version: Option<String>,
    pub grids: Vec<String>,
    pub swaths: Vec<String>,
    pub points: u64,
    /// The grid or swath of the dataset shown.
    pub grid: Option<String>,
    pub swath: Option<String>,
    /// The GCTP projection as written ("GCTP_SNSOID").
    pub projection: Option<String>,
    /// Columns and rows.
    pub grid_size: Option<(u64, u64)>,
    /// Upper-left and lower-right corners (x0, y0, x1, y1), metres or degrees.
    pub corners: Option<[f64; 4]>,
    pub corners_degrees: bool,
    pub short_name: Option<String>,
    pub begin: Option<String>,
    pub end: Option<String>,
}

/// The field a scientific image comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScienceDomain {
    /// Astronomy (FITS, XISF).
    #[default]
    Astronomy,
    /// Electron microscopy (DigitalMicrograph, MRC).
    ElectronMicroscopy,
    /// Scanning probe microscopy (Gwyddion, Igor).
    ScanningProbe,
    /// Whole-slide images and microscopy pyramids (SVS, NDPI, SCN, OME-TIFF, OME-Zarr).
    Slide,
    /// Gridded data with physical axes (NeXus, Imaris, BAG, SRTM, Gaussian cube): the
    /// microscopy layout under the generic title.
    Gridded,
    /// Light microscopy (ImageJ hyperstacks, Zeiss LSM, MetaMorph STK) and other scientific
    /// TIFFs.
    Microscopy,
    /// X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
    Diffraction,
    /// Areal surface topography (X3P, Zygo, Keyence, Sensofar, Alicona, Digital Surf).
    Surface,
    /// Electron backscatter diffraction maps (EDAX .ang, Oxford .ctf).
    Ebsd,
    /// Seismic data (SEG-Y, Seismic Unix).
    Seismic,
    /// Radiometric thermal stills (FLIR, InfiRay, InfraTec).
    Thermal,
    /// Imaging mass spectrometry (imzML).
    MassSpec,
    /// Spectroscopy camera frames (SPE, SIF).
    Spectroscopy,
    /// Flow cytometry list-mode data (FCS).
    Cytometry,
}

/// What the info panel shows about a medical (DICOM) image. Deliberately no patient, physician
/// or institution attributes: those stay out of the default panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MedicalFacts {
    /// Modality code, e.g. "CT", "MR", "CR".
    pub modality: Option<String>,
    pub body_part: Option<String>,
    pub study_description: Option<String>,
    pub series_description: Option<String>,
    /// Millimetres.
    pub slice_thickness: Option<f64>,
    /// Pixel Spacing (0028,0030): millimetres in the patient, (row, column).
    pub pixel_spacing: Option<(f64, f64)>,
    /// Imager Pixel Spacing (0018,1164): millimetres at the detector, (row, column); not
    /// corrected for geometric magnification.
    pub imager_pixel_spacing: Option<(f64, f64)>,
    /// Number of frames, when more than one.
    pub frames: Option<u32>,
    /// Transfer syntax name, e.g. "JPEG Lossless, first-order prediction".
    pub transfer_syntax: Option<String>,
    /// The pixel data is (or was) lossy compressed.
    pub lossy: bool,
    /// An encapsulated document's title (Document Title, else its Concept Name).
    pub document_title: Option<String>,
    /// An encapsulated document's kind and MIME type, e.g. "PDF (application/pdf)".
    pub document_type: Option<String>,
    /// An industrial inspection (DICONDE) rather than a patient's image: shown in its own
    /// group, without the medical warning.
    pub nde: Option<NdeFacts>,
}

/// What a DICONDE file says of the inspected component and the inspection (no names).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NdeFacts {
    /// Component Shape, e.g. "PIPE", "PLATE".
    pub component_shape: Option<String>,
    pub component_manufacturer: Option<String>,
    pub material_grade: Option<String>,
    /// Millimetres.
    pub material_thickness: Option<f64>,
    /// Millimetres.
    pub pipe_diameter: Option<f64>,
    pub environment: Option<String>,
    /// Image quality indicator type (and material).
    pub iqi: Option<String>,
    /// Indications recorded in the file.
    pub indications: usize,
    /// Peak tube voltage or LINAC energy: the number and its unit ("kV", "MeV").
    pub source: Option<(f64, String)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetadataReport {
    /// Ordered groups; empty groups are omitted.
    pub summary: Vec<SummaryGroup>,
    /// Every field, grouped by IFD / XMP namespace / ICC header.
    pub all_fields: Vec<FieldGroup>,
    /// Plain-language warnings, most important first.
    pub warnings: Vec<String>,
    pub gps: Option<Gps>,
    /// EXIF orientation 1–8 (XMP tiff:Orientation if EXIF has none).
    pub orientation: Option<u8>,
    /// ISO 8601 capture time, with offset when known.
    pub capture_time: Option<String>,
    pub rating: Option<i8>,
    pub label: Option<String>,
    pub icc_description: Option<String>,
    /// "1/250 s · f/2.8 · ISO 400 · 35 mm (52 mm equiv.)"; see [`one_line_summary`].
    pub exposure_line: Option<String>,
    /// The language the report's text is in (BCP 47), e.g. "en-GB", "de".
    pub language: String,
}

impl MetadataReport {
    /// Removes what locates the image ("hide GPS"): the coordinates, the summary's location
    /// group, the whole GPS IFD group (so also GPS tags we cannot name) and GPS fields from
    /// other groups such as XMP's `exif:GPS…`. `l` is the localizer the report was read with
    /// (it names the GPS group).
    pub fn strip_location(&mut self, l: &Localizer) {
        self.gps = None;
        self.summary.retain(|g| g.key != "location");
        let gps_group = l.text("fields-gps");
        self.all_fields.retain(|g| g.title != gps_group);
        for g in &mut self.all_fields {
            g.fields.retain(|f| {
                let tag = f.tag.rsplit(':').next().unwrap_or(&f.tag);
                !tag.starts_with("GPS")
            });
        }
        self.all_fields.retain(|g| !g.fields.is_empty());
    }

    /// Adds the "motion" line to the image group (made when missing): the kind of moving
    /// photo (`kind` is its stable key: "motion-photo", "micro-video", "samsung",
    /// "live-photo"), the clip's length and its codec, as known.
    pub fn add_motion(
        &mut self,
        l: &Localizer,
        kind: &str,
        duration_us: Option<u64>,
        codec: Option<&str>,
    ) {
        let mut parts = vec![l.text(&format!("motion-kind-{kind}"))];
        if let Some(us) = duration_us.filter(|&us| us > 0) {
            let s = l.trimmed(us as f64 / 1e6, 1);
            parts.push(l.fmt("motion-duration", &args!("seconds" => s)));
        }
        parts.extend(codec.map(str::to_owned));
        let item = SummaryItem {
            key: "motion",
            label: l.text("label-motion"),
            value: parts.join(&l.text("one-line-separator")),
            detail: None,
        };
        if let Some(g) = self.summary.iter_mut().find(|g| g.key == "image") {
            g.items.push(item);
        } else {
            let at = self
                .summary
                .iter()
                .position(|g| g.key == "file")
                .unwrap_or(self.summary.len());
            self.summary.insert(
                at,
                SummaryGroup {
                    key: "image",
                    title: l.text("group-image"),
                    items: vec![item],
                },
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SummaryGroup {
    /// Stable identifier (not localised): "camera", "exposure", "lens", "date", "location",
    /// "image", "file", "rights", "credentials", "medical".
    pub key: &'static str,
    pub title: String,
    pub items: Vec<SummaryItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SummaryItem {
    /// Stable identifier (not localised), e.g. "shutter", "place"; usable for pinning.
    pub key: &'static str,
    pub label: String,
    pub value: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldGroup {
    pub title: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Tag name, e.g. "ExposureTime" or "dc:title".
    pub tag: String,
    /// Stored value, minimally formatted (rationals as n/d, blobs as "(N bytes)").
    pub raw: String,
    /// Human-readable value.
    pub display: String,
    /// The value carries no information (empty or blank text, an "unknown"/"not defined" code,
    /// a zero or unset number where zero means "not recorded", padding). The info panel hides
    /// these (DESIGN §6.7).
    pub empty: bool,
    /// The whole value when `display` was shortened to fit the panel (it then ends in "…").
    /// "Copy value" copies this, not the shortened text.
    pub full: Option<String>,
}

impl Field {
    /// The text "Copy value" puts on the clipboard: the whole value, never the shortened one.
    pub fn copy_text(&self) -> &str {
        self.full.as_deref().unwrap_or(&self.display)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NorthRef {
    True,
    Magnetic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gps {
    /// Signed decimal degrees (south negative).
    pub lat: f64,
    /// Signed decimal degrees (west negative).
    pub lon: f64,
    /// Metres; negative below sea level.
    pub altitude_m: Option<f64>,
    /// Direction the camera was facing, degrees.
    pub direction_deg: Option<f64>,
    pub direction_ref: Option<NorthRef>,
    /// Horizontal positioning error, metres.
    pub accuracy_m: Option<f64>,
    /// GPS (UTC) time, ISO 8601 with "Z".
    pub timestamp: Option<String>,
    pub place: Option<Place>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub city: String,
    pub admin1: String,
    pub admin2: String,
    pub country_code: String,
    /// Country name in the report's language (English from [`geocode`]).
    pub country: String,
    /// Great-circle distance from the photo to the matched place.
    pub distance_km: f64,
    /// Coordinates of the matched place.
    pub lat: f64,
    pub lon: f64,
}

impl Place {
    /// "Lisbon, Portugal", or "near Lisbon, Portugal" when more than 30 km away,
    /// with the country name in `l`'s language.
    pub fn display_name(&self, l: &Localizer) -> String {
        strings::place_line(
            l,
            &self.city,
            &l.country_name(&self.country_code),
            self.distance_km,
        )
    }

    /// "Lisbon · 2.1 km from Lisbon": administrative areas and distance to the matched town.
    pub fn detail(&self, l: &Localizer) -> String {
        strings::place_detail(l, &self.city, &self.admin1, &self.admin2, self.distance_km)
    }

    /// True when the photo is far enough from the matched place to say "near".
    pub fn is_approximate(&self) -> bool {
        self.distance_km > strings::NEAR_THRESHOLD_KM
    }
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Builds the report with all text in `l`'s language. No geocoding (`gps.place`
/// stays `None`); see [`attach_place`].
pub fn read(input: &MetadataInput, l: &Localizer) -> MetadataReport {
    read_at(input, l, SystemTime::now())
}

/// [`read`] with an explicit "now" for the date-in-the-future check (useful for tests).
pub fn read_at(input: &MetadataInput, l: &Localizer, now: SystemTime) -> MetadataReport {
    let (exif, exif_err) = match input.exif_tiff {
        Some(t) if !t.is_empty() => exif_fields::parse(t),
        _ => (None, None),
    };
    let xmp = input.xmp.map(xmp::parse).unwrap_or_default();
    let icc = input.icc.and_then(icc::parse);
    let report = MetadataReport {
        language: l.langid().to_string(),
        ..Default::default()
    };
    let canon = exif.as_ref().and_then(ExifData::canon_note);
    let panasonic = exif.as_ref().and_then(ExifData::panasonic_note);
    let samsung = exif.as_ref().and_then(ExifData::samsung_note);
    let fuji = exif.as_ref().and_then(ExifData::fuji_note);
    // IPTC, then the container's tags.
    let mut tags: Vec<TagFacts> = input.iptc.and_then(iptc::parse).into_iter().collect();
    tags.extend(input.tags.iter().cloned());
    let mut derived = Xmp::default();
    for (k, v) in tags.iter().flat_map(|t| &t.props) {
        derived.add(k, v);
    }
    Builder {
        input,
        l,
        exif: exif.as_ref(),
        canon: canon.as_ref(),
        panasonic: panasonic.as_ref(),
        samsung: samsung.as_ref(),
        fuji: fuji.as_ref(),
        xmp: &xmp,
        derived: &derived,
        tags: &tags,
        icc: icc.as_ref(),
        now,
        report,
    }
    .build(exif_err, input.icc.is_some() && icc.is_none())
}

/// Geocodes `report.gps` (blocking on first use while the index builds) and adds
/// the "Place" item at the top of the Location group, in `l`'s language (use the
/// same localizer as for [`read`]). No-op without GPS.
pub fn attach_place(report: &mut MetadataReport, l: &Localizer) {
    let Some(gps) = report.gps.as_mut() else {
        return;
    };
    if gps.place.is_none() {
        gps.place = geocode(gps.lat, gps.lon);
    }
    let Some(place) = gps.place.as_mut() else {
        return;
    };
    place.country = l.country_name(&place.country_code);
    let item = SummaryItem {
        key: "place",
        label: l.text("label-place"),
        value: place.display_name(l),
        detail: Some(place.detail(l)),
    };
    let group = match report.summary.iter_mut().position(|g| g.key == "location") {
        Some(i) => &mut report.summary[i],
        None => {
            report.summary.push(SummaryGroup {
                key: "location",
                title: l.text("group-location"),
                items: vec![],
            });
            report.summary.last_mut().expect("just pushed")
        }
    };
    group.items.retain(|i| i.key != "place");
    group.items.insert(0, item);
}

/// "1/250 s · f/2.8 · ISO 400 · 35 mm (52 mm equiv.)" from whatever exposure facts
/// exist, in the report's language.
pub fn one_line_summary(report: &MetadataReport) -> Option<String> {
    report.exposure_line.clone()
}

// ---------------------------------------------------------------------------
// Report builder
// ---------------------------------------------------------------------------

struct Builder<'a> {
    input: &'a MetadataInput<'a>,
    l: &'a Localizer,
    exif: Option<&'a ExifData>,
    /// The Canon maker note's facts, for what the standard tags leave out.
    canon: Option<&'a canon::CanonNote>,
    /// The Panasonic maker note's facts, likewise.
    panasonic: Option<&'a panasonic::PanasonicNote>,
    /// The Samsung maker note's lens name, likewise.
    samsung: Option<&'a samsung::SamsungNote>,
    /// The Fujifilm maker note's settings (film simulation, tone, white balance, focus).
    fuji: Option<&'a fuji::FujiNote>,
    xmp: &'a Xmp,
    /// The IPTC and container tags as XMP properties, read where the XMP has none.
    derived: &'a Xmp,
    /// The IPTC and container tags, for their "all fields" groups.
    tags: &'a [TagFacts],
    icc: Option<&'a icc::IccInfo>,
    now: SystemTime,
    report: MetadataReport,
}

struct GroupBuf {
    key: &'static str,
    title: String,
    items: Vec<SummaryItem>,
}

impl GroupBuf {
    fn new(key: &'static str, title: String) -> Self {
        Self {
            key,
            title,
            items: Vec::new(),
        }
    }
    fn add(
        &mut self,
        key: &'static str,
        label: String,
        value: impl Into<String>,
        detail: Option<String>,
    ) {
        let value = value.into();
        if !value.trim().is_empty() {
            self.items.push(SummaryItem {
                key,
                label,
                value,
                detail,
            });
        }
    }
    fn add_opt(&mut self, key: &'static str, label: String, value: Option<String>) {
        if let Some(v) = value {
            self.add(key, label, v, None);
        }
    }
}

/// The [`HeaderFacts::summary`] keys the header group shows (each has a `label-header-<key>`
/// message).
const HEADER_KEYS: &[&str] = &[
    "created",
    "creator",
    "copyright",
    "project",
    "comment",
    "device",
    "timecode",
    "edge-code",
    "frame-rate",
    "frame",
    "film",
    "transfer",
    "colorimetric",
    "window",
    "resolution",
    "compression",
    "profile",
    "level",
    "decomposition",
    "colour-transform",
    "artboards",
    "not-drawn",
    "approximated",
    "encoding",
    "lines",
    "line-ends",
    "words",
    "pages",
    "pictures",
    "pictures-not-shown",
    "tables",
    "rows",
    "columns",
    "delimiter",
    "column-names",
    "cues",
    "cue-span",
];

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// A length in metres per unit symbol (SI prefixes of the metre, ångström), `None` for
/// other units.
pub fn metres_per_unit(unit: &str) -> Option<f64> {
    Some(match unit.trim() {
        "m" => 1.0,
        "cm" => 1e-2,
        "mm" => 1e-3,
        "µm" | "μm" | "um" => 1e-6,
        "nm" => 1e-9,
        "pm" => 1e-12,
        "Å" | "\u{212b}" | "A" | "angstrom" => 1e-10,
        _ => return None,
    })
}

/// A pixel size in a metric length unit restated with the SI prefix that puts it between
/// 0.1 and 1000 (2.5e-10 m as 0.25 nm); other units as given.
fn metric_length(x: f64, y: f64, unit: &str) -> (f64, f64, String) {
    let Some(f) = metres_per_unit(unit) else {
        return (x, y, unit.trim().to_owned());
    };
    let m = x.max(y) * f;
    let (scale, name) = [(1.0, "m"), (1e-3, "mm"), (1e-6, "µm"), (1e-9, "nm")]
        .into_iter()
        .find(|(s, _)| m >= *s * 0.1)
        .unwrap_or((1e-12, "pm"));
    (x * f / scale, y * f / scale, name.to_owned())
}

impl<'a> Builder<'a> {
    fn ascii(&self, tag: Tag) -> Option<String> {
        non_empty(self.exif?.ascii(tag))
    }
    /// An XMP property, else its IPTC or container tag equivalent.
    fn xmp(&self, name: &str) -> Option<String> {
        non_empty(self.xmp.get(name).map(str::to_owned))
            .or_else(|| non_empty(self.derived.get(name).map(str::to_owned)))
    }
    fn xmp_list(&self, name: &str) -> Option<String> {
        let v = self
            .xmp
            .get_all(name)
            .or_else(|| self.derived.get_all(name))?;
        non_empty(Some(v.join(", ")))
    }

    fn build(mut self, exif_err: Option<String>, icc_damaged: bool) -> MetadataReport {
        let mut groups = vec![
            self.gifti(),
            self.cifti(),
            self.tracts(),
            self.geo(),
            self.medical(),
            self.inspection(),
            self.volume(),
            self.science(),
            self.planetary(),
            self.asdf(),
            self.root(),
            self.hdf4(),
            self.hdf_eos(),
            self.casa(),
            self.ugrid(),
            self.omero(),
            self.annotations(),
            self.spectrum(),
            self.drawing(),
            self.mesh(),
            self.points(),
            self.splats(),
            self.rti(),
            self.structure(),
            self.font(),
            self.book(),
            self.document(),
            self.header(),
            self.camera(),
            self.exposure(),
            self.lens(),
            self.picture(),
            self.date(),
            self.location(),
            self.drone(),
            self.image(),
            self.file(),
            self.rights(),
            self.credentials(),
        ];
        groups.retain(|g| !g.items.is_empty());
        self.report.exposure_line = groups.iter().find(|g| g.key == "exposure").and_then(|g| {
            let parts: Vec<String> = ["shutter", "aperture", "iso", "focal_length"]
                .iter()
                .filter_map(|k| {
                    g.items
                        .iter()
                        .find(|i| i.key == *k)
                        .map(|i| i.value.clone())
                })
                .collect();
            (!parts.is_empty()).then(|| strings::one_line(self.l, &parts))
        });
        self.report.summary = groups
            .into_iter()
            .map(|g| SummaryGroup {
                key: g.key,
                title: g.title,
                items: g.items,
            })
            .collect();

        // Warnings.
        let mut w = Vec::new();
        if self.input.medical.is_some() && !self.industrial() {
            w.push(self.l.text("warn-not-diagnostic"));
        }
        if let Some(g) = &self.report.gps {
            w.push(self.l.text("warn-gps").to_owned());
            if g.lat == 0.0 && g.lon == 0.0 {
                w.push(self.l.text("warn-null-island").to_owned());
            }
        }
        // Only real problems: a missing value is not a warning (DESIGN §6.7).
        if let Some((s, _)) = self.capture_stamp()
            && s.is_future(self.now)
        {
            w.push(self.l.text("warn-date-future").to_owned())
        }
        if icc_damaged {
            w.push(self.l.text("warn-icc-damaged").to_owned());
        }
        if let (Some(e), Some(x)) = (self.exif_orientation(), self.xmp_orientation())
            && e != x
        {
            w.push(strings::warn_orientation_conflict(self.l, e, x));
        }
        if let Some(e) = exif_err {
            w.push(strings::warn_exif_damaged(self.l, &e));
        }
        if let Some(e) = &self.xmp.error {
            w.push(strings::warn_xmp_damaged(self.l, e));
        }
        self.report.warnings = w;

        // All fields.
        let mut all = Vec::new();
        if let Some(e) = self.exif {
            all.extend(e.field_groups(self.l));
        }
        all.extend(self.xmp_fields());
        if let Some(icc) = self.icc {
            all.push(FieldGroup {
                title: self.l.text("fields-icc").to_owned(),
                fields: icc.fields(self.l),
            });
        }
        all.extend(self.header_fields());
        all.extend(self.science_fields());
        all.extend(self.annotation_fields());
        all.extend(self.drawing_fields());
        all.extend(self.geo_fields());
        all.extend(self.fuji_fields());
        for t in self.tags {
            all.push(FieldGroup {
                title: self.l.fmt("fields-tags", &args!("kind" => t.kind.clone())),
                fields: t
                    .fields
                    .iter()
                    .map(|(k, v)| {
                        let display = truncate(v.clone());
                        Field {
                            tag: k.clone(),
                            raw: display.clone(),
                            empty: v.trim().is_empty(),
                            full: (display != *v).then(|| v.clone()),
                            display,
                        }
                    })
                    .collect(),
            });
        }
        self.report.all_fields = all;
        self.report
    }

    // ----- Camera ---------------------------------------------------------

    fn camera(&self) -> GroupBuf {
        let mut g = GroupBuf::new("camera", self.l.text("group-camera"));
        if self.input.file.drawn {
            return g;
        }
        let make = self.ascii(Tag::Make).or_else(|| self.xmp("tiff:Make"));
        let model = self.ascii(Tag::Model).or_else(|| self.xmp("tiff:Model"));
        g.add_opt(
            "camera",
            self.l.text("label-camera"),
            camera_name(make.as_deref(), model.as_deref()),
        );
        g.add_opt(
            "software",
            self.l.text("label-software"),
            self.ascii(Tag::Software)
                .or_else(|| self.xmp("xmp:CreatorTool")),
        );
        g
    }

    // ----- Exposure -------------------------------------------------------

    fn exposure(&self) -> GroupBuf {
        let mut g = GroupBuf::new("exposure", self.l.text("group-exposure"));
        let Some(e) = self.exif else {
            return self.exposure_from_xmp(g);
        };
        g.add_opt(
            "shutter",
            self.l.text("label-shutter"),
            e.rational_parts(Tag::ExposureTime)
                .or_else(|| apex_exposure_time(e.rational(Tag::ShutterSpeedValue)?))
                .and_then(|(n, d)| strings::exposure_time(self.l, n, d)),
        );
        g.add_opt(
            "aperture",
            self.l.text("label-aperture"),
            e.rational(Tag::FNumber)
                .filter(|f| *f > 0.0)
                .or_else(|| apex_f_number(e.rational(Tag::ApertureValue)?))
                .map(|v| strings::f_number(self.l, v)),
        );
        let iso = e
            .uint(Tag::PhotographicSensitivity)
            .or_else(|| {
                self.xmp("exif:ISOSpeedRatings")
                    .and_then(|s| s.parse().ok())
            })
            .filter(|v| *v > 0);
        g.add_opt(
            "iso",
            self.l.text("label-iso"),
            iso.map(|v| strings::iso(self.l, v)),
        );
        let equiv = e.uint(Tag::FocalLengthIn35mmFilm).filter(|v| *v > 0);
        let focal = match e.rational(Tag::FocalLength).filter(|f| *f > 0.0) {
            Some(mm) => Some(strings::focal_length(self.l, mm, equiv)),
            None => equiv.map(|v| strings::focal_length_equiv_only(self.l, v)),
        };
        g.add_opt("focal_length", self.l.text("label-focal-length"), focal);
        g.add_opt(
            "exposure_bias",
            self.l.text("label-exposure-bias"),
            e.rational(Tag::ExposureBiasValue)
                .map(|v| strings::exposure_bias(self.l, v)),
        );
        g.add_opt(
            "program",
            self.l.text("label-program"),
            e.uint(Tag::ExposureProgram)
                .and_then(|v| strings::exposure_program(self.l, v)),
        );
        g.add_opt(
            "metering",
            self.l.text("label-metering"),
            e.uint(Tag::MeteringMode)
                .and_then(|v| strings::metering_mode(self.l, v)),
        );
        g.add_opt(
            "focus_mode",
            self.l.text("label-focus-mode"),
            self.canon
                .and_then(|c| c.focus_mode)
                .map(|v| strings::coded(self.l, "canon-focus-mode", u32::from(v)))
                .or_else(|| {
                    let v = self.panasonic?.focus_mode?;
                    Some(strings::coded(self.l, "panasonic-focus-mode", u32::from(v)))
                })
                .or_else(|| self.fuji_focus_mode()),
        );
        let light = e
            .uint(Tag::LightSource)
            .and_then(|v| strings::light_source(self.l, v));
        g.add_opt(
            "white_balance",
            self.l.text("label-white-balance"),
            self.fuji_white_balance().or_else(|| {
                e.uint(Tag::WhiteBalance)
                    .and_then(|v| strings::white_balance(self.l, v, light.clone()))
            }),
        );
        g.add_opt(
            "flash",
            self.l.text("label-flash"),
            e.uint(Tag::Flash).map(|v| strings::flash(self.l, v)),
        );
        if let Some(f) = self.fuji {
            g.add_opt(
                "shutter_type",
                self.l.text("label-shutter-type"),
                f.shutter_type
                    .filter(|&v| fuji::shutter_type_known(v))
                    .map(|v| strings::coded(self.l, "fuji-shutter-type", u32::from(v))),
            );
            g.add_opt(
                "drive_mode",
                self.l.text("label-drive-mode"),
                f.drive_mode
                    .filter(|&v| v <= 2)
                    .map(|v| strings::coded(self.l, "fuji-drive-mode", u32::from(v))),
            );
        }
        g
    }

    /// The Fujifilm note's focus mode: AF-S, AF-C or manual (FocusSettings), else auto or
    /// manual (FocusMode).
    fn fuji_focus_mode(&self) -> Option<String> {
        let f = self.fuji?;
        let v = match (f.focus_mode2, f.focus_mode) {
            (Some(m @ 0..=2), _) => u32::from(m),
            (_, Some(m @ 0..=1)) => 100 + u32::from(m),
            _ => return None,
        };
        Some(strings::coded(self.l, "fuji-focus-mode", v))
    }

    /// The Fujifilm note's white balance, with the colour temperature of a Kelvin setting.
    fn fuji_white_balance(&self) -> Option<String> {
        let f = self.fuji?;
        let v = f.white_balance.filter(|&v| fuji::white_balance_known(v))?;
        Some(match (v, f.colour_temperature) {
            (4080, Some(k)) => self.l.fmt(
                "fuji-white-balance-kelvin",
                &args!("kelvin" => self.l.int(u64::from(k))),
            ),
            _ => strings::coded(self.l, "fuji-white-balance", u32::from(v)),
        })
    }

    /// A setting's steps from normal: "+2", "−1", "0".
    fn step(&self, steps: f64) -> String {
        let sign = if steps > 0.0 {
            "plus"
        } else if steps < 0.0 {
            "minus"
        } else {
            "zero"
        };
        self.l.fmt(
            "fuji-step",
            &args!("sign" => sign, "n" => self.l.trimmed(steps.abs(), 1)),
        )
    }

    // ----- Picture settings (Fujifilm) ---------------------------------------

    /// The film simulation, dynamic range and tone settings the camera applied to its JPEG
    /// (and records in the RAF), from the Fujifilm maker note.
    fn picture(&self) -> GroupBuf {
        let mut g = GroupBuf::new("picture", self.l.text("group-picture"));
        let Some(f) = self.fuji else {
            return g;
        };
        let film = f
            .saturation
            .filter(|&v| fuji::monochrome_known(v))
            .map(|v| strings::coded(self.l, "fuji-monochrome", u32::from(v)))
            .or_else(|| {
                let v = f.film_mode.filter(|&v| fuji::film_mode_known(v))?;
                Some(strings::coded(self.l, "fuji-film-mode", u32::from(v)))
            });
        g.add_opt(
            "film_simulation",
            self.l.text("label-film-simulation"),
            film,
        );
        let dr = match (f.dynamic_range_setting, f.development_dynamic_range) {
            (Some(0), Some(p)) => Some(self.l.fmt(
                "fuji-dynamic-range-auto",
                &args!("percent" => self.l.int(u64::from(p))),
            )),
            (_, Some(p)) => Some(self.l.fmt(
                "fuji-dynamic-range",
                &args!("percent" => self.l.int(u64::from(p))),
            )),
            (Some(v @ (0 | 256 | 512 | 513)), None) => Some(strings::coded(
                self.l,
                "fuji-dynamic-range-setting",
                u32::from(v),
            )),
            _ => None,
        };
        g.add_opt("dynamic_range", self.l.text("label-dynamic-range"), dr);
        let tone = |v: Option<i32>| v.and_then(fuji::tone_steps).map(|s| self.step(s));
        g.add_opt(
            "highlight_tone",
            self.l.text("label-highlight-tone"),
            tone(f.highlight_tone),
        );
        g.add_opt(
            "shadow_tone",
            self.l.text("label-shadow-tone"),
            tone(f.shadow_tone),
        );
        let colour = f.saturation.and_then(|v| {
            Some(match v {
                0 => 0.0,
                128 => 1.0,
                256 => 2.0,
                192 => 3.0,
                224 => 4.0,
                384 => -1.0,
                1024 => -2.0,
                1216 => -3.0,
                1248 => -4.0,
                _ => return None,
            })
        });
        g.add_opt(
            "colour",
            self.l.text("label-colour-setting"),
            colour.map(|s| self.step(s)),
        );
        let sharpness = f.sharpness.and_then(|v| {
            Some(match v {
                0 => -4.0,
                1 => -3.0,
                2 => -2.0,
                130 => -1.0,
                3 => 0.0,
                132 => 1.0,
                4 => 2.0,
                5 => 3.0,
                6 => 4.0,
                _ => return None,
            })
        });
        g.add_opt(
            "sharpness",
            self.l.text("label-sharpness"),
            sharpness.map(|s| self.step(s)),
        );
        let nr = f.noise_reduction.and_then(|v| {
            Some(match v {
                0 => 0.0,
                384 => 1.0,
                256 => 2.0,
                448 => 3.0,
                480 => 4.0,
                640 => -1.0,
                512 => -2.0,
                704 => -3.0,
                736 => -4.0,
                _ => return None,
            })
        });
        g.add_opt(
            "noise_reduction",
            self.l.text("label-noise-reduction"),
            nr.map(|s| self.step(s)),
        );
        g.add_opt(
            "clarity",
            self.l.text("label-clarity"),
            f.clarity
                .filter(|c| c % 1000 == 0 && (-5000..=5000).contains(c))
                .map(|c| self.step(f64::from(c / 1000))),
        );
        let effect = |v: i32| strings::coded(self.l, "fuji-effect", v as u32);
        g.add_opt(
            "grain",
            self.l.text("label-grain-effect"),
            f.grain.filter(|&v| fuji::effect_known(v)).map(|v| {
                match f.grain_size.filter(|&s| v != 0 && matches!(s, 16 | 32)) {
                    Some(s) => self.l.fmt(
                        "fuji-grain",
                        &args!("strength" => effect(v), "size" => strings::coded(self.l, "fuji-grain-size", u32::from(s))),
                    ),
                    None => effect(v),
                }
            }),
        );
        g.add_opt(
            "colour_chrome",
            self.l.text("label-colour-chrome"),
            f.colour_chrome
                .filter(|&v| fuji::effect_known(v))
                .map(effect),
        );
        g.add_opt(
            "colour_chrome_blue",
            self.l.text("label-colour-chrome-blue"),
            f.colour_chrome_blue
                .filter(|&v| fuji::effect_known(v))
                .map(effect),
        );
        g
    }

    /// The Fujifilm note's settings among all fields: the entry's name, its code and words.
    fn fuji_fields(&self) -> Option<FieldGroup> {
        let pic = self.picture();
        let exp = self.exposure();
        let f = self.fuji?;
        let shown = |g: &GroupBuf, key: &str| {
            g.items
                .iter()
                .find(|i| i.key == key)
                .map(|i| i.value.clone())
        };
        let raw = |v: Option<i64>| v.map(|v| v.to_string());
        let rows: [(&str, Option<String>, Option<String>); 16] = [
            (
                "FilmMode",
                raw(f.film_mode.map(i64::from)),
                shown(&pic, "film_simulation").filter(|_| f.film_mode.is_some()),
            ),
            (
                "Saturation",
                raw(f.saturation.map(i64::from)),
                shown(&pic, "colour")
                    .or_else(|| shown(&pic, "film_simulation").filter(|_| f.film_mode.is_none())),
            ),
            (
                "DynamicRangeSetting",
                raw(f.dynamic_range_setting.map(i64::from)),
                None,
            ),
            (
                "DevelopmentDynamicRange",
                raw(f.development_dynamic_range.map(i64::from)),
                shown(&pic, "dynamic_range"),
            ),
            (
                "HighlightTone",
                raw(f.highlight_tone.map(i64::from)),
                shown(&pic, "highlight_tone"),
            ),
            (
                "ShadowTone",
                raw(f.shadow_tone.map(i64::from)),
                shown(&pic, "shadow_tone"),
            ),
            (
                "Sharpness",
                raw(f.sharpness.map(i64::from)),
                shown(&pic, "sharpness"),
            ),
            (
                "NoiseReduction",
                raw(f.noise_reduction.map(i64::from)),
                shown(&pic, "noise_reduction"),
            ),
            (
                "Clarity",
                raw(f.clarity.map(i64::from)),
                shown(&pic, "clarity"),
            ),
            (
                "GrainEffectRoughness",
                raw(f.grain.map(i64::from)),
                shown(&pic, "grain"),
            ),
            (
                "ColorChromeEffect",
                raw(f.colour_chrome.map(i64::from)),
                shown(&pic, "colour_chrome"),
            ),
            (
                "ColorChromeFXBlue",
                raw(f.colour_chrome_blue.map(i64::from)),
                shown(&pic, "colour_chrome_blue"),
            ),
            (
                "WhiteBalance",
                raw(f.white_balance.map(i64::from)),
                self.fuji_white_balance(),
            ),
            (
                "FocusMode",
                raw(f
                    .focus_mode2
                    .or(f.focus_mode.map(|v| v as u8))
                    .map(i64::from)),
                self.fuji_focus_mode(),
            ),
            (
                "ShutterType",
                raw(f.shutter_type.map(i64::from)),
                shown(&exp, "shutter_type"),
            ),
            (
                "DriveMode",
                raw(f.drive_mode.map(i64::from)),
                shown(&exp, "drive_mode"),
            ),
        ];
        let fields: Vec<Field> = rows
            .into_iter()
            .filter_map(|(tag, raw, display)| {
                let raw = raw?;
                Some(Field {
                    tag: tag.to_owned(),
                    display: display.unwrap_or_else(|| raw.clone()),
                    raw,
                    empty: false,
                    full: None,
                })
            })
            .collect();
        (!fields.is_empty()).then(|| FieldGroup {
            title: self.l.text("fields-fujifilm"),
            fields,
        })
    }

    fn exposure_from_xmp(&self, mut g: GroupBuf) -> GroupBuf {
        let ratio = |s: String| -> Option<(u32, u32)> {
            let (n, d) = s.split_once('/').unwrap_or((&s, "1"));
            Some((n.trim().parse().ok()?, d.trim().parse().ok()?))
        };
        let real = |s: String| -> Option<f64> {
            let (n, d) = s.split_once('/').unwrap_or((&s, "1"));
            let d: f64 = d.trim().parse().ok()?;
            (d != 0.0).then_some(n.trim().parse::<f64>().ok()? / d)
        };
        g.add_opt(
            "shutter",
            self.l.text("label-shutter"),
            self.xmp("exif:ExposureTime")
                .and_then(ratio)
                .and_then(|(n, d)| strings::exposure_time(self.l, n, d)),
        );
        g.add_opt(
            "aperture",
            self.l.text("label-aperture"),
            self.xmp("exif:FNumber")
                .and_then(real)
                .filter(|v| *v > 0.0)
                .map(|v| strings::f_number(self.l, v)),
        );
        g.add_opt(
            "iso",
            self.l.text("label-iso"),
            self.xmp("exif:ISOSpeedRatings")
                .and_then(|s| s.parse().ok())
                .filter(|v| *v > 0)
                .map(|v| strings::iso(self.l, v)),
        );
        let equiv = self
            .xmp("exif:FocalLengthIn35mmFilm")
            .and_then(|s| s.parse().ok())
            .filter(|v| *v > 0);
        let focal = match self
            .xmp("exif:FocalLength")
            .and_then(real)
            .filter(|f| *f > 0.0)
        {
            Some(mm) => Some(strings::focal_length(self.l, mm, equiv)),
            None => equiv.map(|v| strings::focal_length_equiv_only(self.l, v)),
        };
        g.add_opt("focal_length", self.l.text("label-focal-length"), focal);
        g
    }

    // ----- Lens -----------------------------------------------------------

    fn lens(&self) -> GroupBuf {
        let mut g = GroupBuf::new("lens", self.l.text("group-lens"));
        let model = self
            .ascii(Tag::LensModel)
            .or_else(|| self.xmp("exifEX:LensModel"))
            .or_else(|| self.xmp("aux:Lens"))
            .or_else(|| self.canon.and_then(|c| c.lens.clone()))
            .or_else(|| self.panasonic.and_then(|p| p.lens.clone()))
            .or_else(|| self.samsung.and_then(|s| s.lens.clone()));
        let make = self
            .ascii(Tag::LensMake)
            .or_else(|| self.xmp("exifEX:LensMake"));
        // The widest aperture at the focal length used: Canon's CameraSettings, else Exif's
        // MaxApertureValue (APEX).
        let now = self.canon.and_then(|c| c.aperture_now).or_else(|| {
            self.exif
                .and_then(|e| e.rational_at(Tag::MaxApertureValue, 0))
                .and_then(apex_f_number)
                .filter(|f| *f >= 0.7)
        });
        let named = model.as_deref().map(lens::name_spec).unwrap_or_default();
        // The lens's widest apertures when LensSpecification has none (Canon writes zeros):
        // those its name states, when the name's focal range is this one and the aperture
        // used lies within them; for a prime, the widest one at the focal length used.
        let apertures = |min: f64, max: f64| -> (Option<f64>, Option<f64>) {
            let near = |a: f64, b: f64| (a - b).abs() <= 0.5;
            if let Some((a, b)) = named.aperture
                && named
                    .focal
                    .is_none_or(|(x, y)| near(x, min) && near(y, max))
                && now.is_none_or(|f| f >= a * 0.95 && f <= b * 1.05)
            {
                return (Some(a), Some(b));
            }
            if near(min, max) {
                // Canon's own code for its prime, else the APEX value.
                let canon = self
                    .canon
                    .filter(|c| c.focal.is_some_and(|(a, b)| a == min && b == max))
                    .and_then(|c| c.prime_aperture);
                return (canon.or(now), None);
            }
            (None, None)
        };
        let range = self
            .exif
            .and_then(|e| {
                let min = e
                    .rational_at(Tag::LensSpecification, 0)
                    .filter(|v| *v > 0.0)?;
                let max = e.rational_at(Tag::LensSpecification, 1).unwrap_or(min);
                let fa = e
                    .rational_at(Tag::LensSpecification, 2)
                    .filter(|v| *v > 0.0);
                let fb = e
                    .rational_at(Tag::LensSpecification, 3)
                    .filter(|v| *v > 0.0);
                let (fa, fb) = match fa {
                    Some(fa) => (Some(fa), fb),
                    None => apertures(min, max),
                };
                Some(strings::lens_range(self.l, min, max, fa, fb))
            })
            .or_else(|| {
                let (min, max) = self.canon?.focal?;
                let (fa, fb) = apertures(min, max);
                Some(strings::lens_range(self.l, min, max, fa, fb))
            })
            .or_else(|| {
                // Else the range the name states ("LEICA DG 12-60/F2.8-4.0"), when the focal
                // length used lies within it (not through a converter or on another lens).
                let (min, max) = named.focal?;
                let used = self
                    .exif
                    .and_then(|e| e.rational(Tag::FocalLength))
                    .filter(|f| *f > 0.0);
                if used.is_some_and(|f| f < min * 0.95 || f > max * 1.05) {
                    return None;
                }
                let (fa, fb) = apertures(min, max);
                Some(strings::lens_range(self.l, min, max, fa, fb))
            });
        match model {
            Some(m) => g.add("lens", self.l.text("label-lens"), m.clone(), range),
            None => g.add_opt("lens_range", self.l.text("label-lens-range"), range),
        }
        if let Some(mk) = make {
            let model_l = g
                .items
                .first()
                .map(|i| i.value.to_lowercase())
                .unwrap_or_default();
            if !model_l.starts_with(&mk.to_lowercase()) {
                g.add("lens_make", self.l.text("label-lens-make"), mk, None);
            }
        }
        g.add_opt(
            "stabilisation",
            self.l.text("label-stabilisation"),
            self.canon
                .and_then(|c| c.stabilisation)
                .map(|v| strings::coded(self.l, "canon-stabilisation", u32::from(v)))
                .or_else(|| {
                    let v = self.panasonic?.stabilisation?;
                    Some(strings::coded(
                        self.l,
                        "panasonic-stabilisation",
                        u32::from(v),
                    ))
                })
                .or_else(|| {
                    // Fujifilm: the kind (lens, sensor shift…) and whether it was on.
                    let [kind, mode, _] = self.fuji?.stabilisation?;
                    if !fuji::stabilisation_known(kind) || mode > 2 {
                        return None;
                    }
                    if mode == 0 {
                        return Some(strings::coded(self.l, "fuji-stabilisation-mode", 0));
                    }
                    Some(self.l.fmt(
                        "fuji-stabilisation",
                        &args!(
                            "kind" => strings::coded(self.l, "fuji-stabilisation-kind", u32::from(kind)),
                            "mode" => strings::coded(self.l, "fuji-stabilisation-mode", u32::from(mode)),
                        ),
                    ))
                }),
        );
        g
    }

    // ----- Date and time --------------------------------------------------

    fn exif_stamp(&self, dt: Tag, off: Tag) -> Option<Stamp> {
        let local = datetime::parse_exif_datetime(&self.ascii(dt)?)?;
        Some(Stamp {
            local,
            offset_minutes: self.ascii(off).and_then(|s| datetime::parse_offset(&s)),
        })
    }

    fn xmp_stamp(&self, name: &str) -> Option<Stamp> {
        datetime::parse_xmp_datetime(&self.xmp(name)?)
    }

    fn gps_utc(&self) -> Option<chrono::NaiveDateTime> {
        let e = self.exif?;
        let date = e.ascii(Tag::GPSDateStamp)?;
        datetime::gps_datetime(
            &date,
            e.rational_at(Tag::GPSTimeStamp, 0)?,
            e.rational_at(Tag::GPSTimeStamp, 1).unwrap_or(0.0),
            e.rational_at(Tag::GPSTimeStamp, 2).unwrap_or(0.0),
        )
    }

    /// Capture time, and whether the offset was inferred from GPS.
    fn capture_stamp(&self) -> Option<(Stamp, bool)> {
        let mut s = self
            .exif_stamp(Tag::DateTimeOriginal, Tag::OffsetTimeOriginal)
            .or_else(|| self.xmp_stamp("exif:DateTimeOriginal"))
            .or_else(|| self.xmp_stamp("photoshop:DateCreated"))
            .or_else(|| self.exif_stamp(Tag::DateTimeDigitized, Tag::OffsetTimeDigitized))
            .or_else(|| self.xmp_stamp("xmp:CreateDate"))?;
        let mut inferred = false;
        if s.offset_minutes.is_none()
            && let Some(utc) = self.gps_utc()
            && let Some(off) = datetime::offset_from_gps(s.local, utc)
        {
            s.offset_minutes = Some(off);
            inferred = true;
        }
        Some((s, inferred))
    }

    fn date(&mut self) -> GroupBuf {
        let mut g = GroupBuf::new("date", self.l.text("group-date"));
        let taken = self.capture_stamp();
        if let Some((s, inferred)) = taken {
            let detail = inferred.then(|| self.l.text("detail-zone-from-gps").to_owned());
            g.add(
                "taken",
                self.l.text("label-taken"),
                s.display(self.l),
                detail,
            );
            self.report.capture_time = Some(s.iso());
        }
        let taken_local = taken.map(|(s, _)| s.local);
        if let Some(d) = self.exif_stamp(Tag::DateTimeDigitized, Tag::OffsetTimeDigitized)
            && Some(d.local) != taken_local
        {
            g.add(
                "digitised",
                self.l.text("label-digitised"),
                d.display(self.l),
                None,
            );
        }
        if let Some(m) = self
            .exif_stamp(Tag::DateTime, Tag::OffsetTime)
            .or_else(|| self.xmp_stamp("xmp:ModifyDate"))
            && Some(m.local) != taken_local
        {
            g.add(
                "edited",
                self.l.text("label-edited"),
                m.display(self.l),
                None,
            );
        }
        g
    }

    // ----- Location -------------------------------------------------------

    fn gps(&self) -> Option<Gps> {
        let (lat, lon) = self.gps_coords()?;
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return None;
        }
        let e = self.exif;
        let altitude_m = e
            .and_then(|e| {
                let a = e.rational(Tag::GPSAltitude)?;
                Some(if e.byte(Tag::GPSAltitudeRef) == Some(1) {
                    -a
                } else {
                    a
                })
            })
            .or_else(|| {
                // A container's position (a movie's ISO 6709 location) gives metres.
                self.derived
                    .get("exif:GPSAltitude")?
                    .parse::<f64>()
                    .ok()
                    .filter(|a| a.is_finite())
            });
        let direction_deg = e
            .and_then(|e| e.rational(Tag::GPSImgDirection))
            .filter(|d| (0.0..=360.0).contains(d));
        let direction_ref = e
            .and_then(|e| e.ascii(Tag::GPSImgDirectionRef))
            .and_then(|r| match r.trim() {
                "T" => Some(NorthRef::True),
                "M" => Some(NorthRef::Magnetic),
                _ => None,
            });
        let accuracy_m = e
            .and_then(|e| e.rational(Tag::GPSHPositioningError))
            .filter(|a| *a > 0.0);
        Some(Gps {
            lat,
            lon,
            altitude_m,
            direction_deg,
            direction_ref,
            accuracy_m,
            timestamp: self.gps_utc().map(datetime::utc_iso),
            place: None,
        })
    }

    fn gps_coords(&self) -> Option<(f64, f64)> {
        if let Some(e) = self.exif
            && let (Some(lat), Some(lon)) = (e.dms(Tag::GPSLatitude), e.dms(Tag::GPSLongitude))
        {
            let s = if e
                .ascii(Tag::GPSLatitudeRef)
                .is_some_and(|r| r.trim().eq_ignore_ascii_case("S"))
            {
                -1.0
            } else {
                1.0
            };
            let w = if e
                .ascii(Tag::GPSLongitudeRef)
                .is_some_and(|r| r.trim().eq_ignore_ascii_case("W"))
            {
                -1.0
            } else {
                1.0
            };
            return Some((s * lat, w * lon));
        }
        let lat = parse_xmp_coord(&self.xmp("exif:GPSLatitude")?)?;
        let lon = parse_xmp_coord(&self.xmp("exif:GPSLongitude")?)?;
        Some((lat, lon))
    }

    /// A DJI drone's photo: gimbal and flight attitude and the height above the take-off
    /// point (XMP `drone-dji`); for a thermal camera's radiometric JPEG, why its temperatures
    /// are not shown.
    fn drone(&self) -> GroupBuf {
        let mut g = GroupBuf::new("drone", self.l.text("group-drone"));
        let deg = |k: &str| {
            self.xmp(&format!("drone-dji:{k}"))
                .and_then(|v| v.trim().trim_start_matches('+').parse::<f64>().ok())
        };
        let attitude = |pre: &str| {
            let (y, p, r) = (
                deg(&format!("{pre}YawDegree")),
                deg(&format!("{pre}PitchDegree")),
                deg(&format!("{pre}RollDegree")),
            );
            (y.is_some() || p.is_some()).then(|| {
                let f = |v: Option<f64>| v.map_or_else(|| "–".to_owned(), |v| format!("{v:.1}°"));
                let mut a = FluentArgs::new();
                a.set("yaw", f(y));
                a.set("pitch", f(p));
                a.set("roll", f(r));
                self.l.fmt("drone-attitude", &a)
            })
        };
        if let Some(v) = attitude("Gimbal") {
            g.add("gimbal", self.l.text("label-gimbal"), v, None);
        }
        if let Some(v) = attitude("Flight") {
            g.add("flight", self.l.text("label-flight"), v, None);
        }
        if let Some(h) = deg("RelativeAltitude") {
            g.add(
                "relative_altitude",
                self.l.text("label-relative-altitude"),
                strings::altitude(self.l, h),
                None,
            );
        }
        let infrared = self
            .xmp("drone-dji:ImageSource")
            .is_some_and(|s| s.eq_ignore_ascii_case("InfraredCamera"));
        if infrared {
            g.add(
                "dji_thermal",
                self.l.text("label-thermal-data"),
                self.l.text("drone-thermal-refused"),
                Some(self.l.text("drone-thermal-refused-detail")),
            );
        }
        g
    }

    fn location(&mut self) -> GroupBuf {
        let mut g = GroupBuf::new("location", self.l.text("group-location"));
        let Some(gps) = self.gps() else { return g };
        g.add(
            "coordinates",
            self.l.text("label-coordinates"),
            strings::coordinates(self.l, gps.lat, gps.lon),
            None,
        );
        if let Some(a) = gps.altitude_m {
            g.add(
                "altitude",
                self.l.text("label-altitude"),
                strings::altitude(self.l, a),
                None,
            );
        }
        if let Some(d) = gps.direction_deg {
            let detail = gps.direction_ref.map(|r| {
                match r {
                    NorthRef::True => self.l.text("detail-true-north"),
                    NorthRef::Magnetic => self.l.text("detail-magnetic-north"),
                }
                .to_owned()
            });
            g.add(
                "direction",
                self.l.text("label-direction"),
                strings::facing(self.l, d),
                detail,
            );
        }
        if let Some(a) = gps.accuracy_m {
            g.add(
                "accuracy",
                self.l.text("label-accuracy"),
                strings::accuracy(self.l, a),
                None,
            );
        }
        if let Some(t) = self.gps_utc() {
            g.add(
                "gps_time",
                self.l.text("label-gps-time"),
                datetime::format_utc(t, self.l),
                None,
            );
        }
        self.report.gps = Some(gps);
        g
    }

    // ----- Image ----------------------------------------------------------

    fn exif_orientation(&self) -> Option<u8> {
        self.exif?
            .uint(Tag::Orientation)
            .filter(|v| (1..=8).contains(v))
            .map(|v| v as u8)
    }

    // ----- Science ----------------------------------------------------------

    fn science(&self) -> GroupBuf {
        let title = match self.input.science.as_ref().map(|s| s.domain) {
            Some(ScienceDomain::ElectronMicroscopy) => "group-em",
            Some(ScienceDomain::ScanningProbe) => "group-spm",
            Some(ScienceDomain::Slide) => "group-slide",
            Some(ScienceDomain::Microscopy) => "group-microscopy",
            Some(ScienceDomain::Diffraction) => "group-diffraction",
            Some(ScienceDomain::Surface) => "group-surface",
            Some(ScienceDomain::Ebsd) => "group-ebsd",
            Some(ScienceDomain::Seismic) => "group-seismic",
            Some(ScienceDomain::MassSpec) => "group-mass-spec",
            Some(ScienceDomain::Thermal) => "group-thermal",
            Some(ScienceDomain::Cytometry) => "group-cytometry",
            Some(ScienceDomain::Spectroscopy) => "group-spectroscopy",
            _ => "group-science",
        };
        let mut g = GroupBuf::new("science", self.l.text(title));
        let Some(s) = &self.input.science else {
            return g;
        };
        if s.domain == ScienceDomain::Diffraction {
            return self.diffraction(g, s);
        }
        if let Some(t) = s
            .thermal
            .as_ref()
            .filter(|_| s.domain == ScienceDomain::Thermal)
        {
            return self.thermal(g, s, t);
        }
        if s.domain != ScienceDomain::Astronomy {
            return self.microscopy(g, s);
        }
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add_opt("object", self.l.text("label-object"), text(&s.object));
        g.add_opt(
            "telescope",
            self.l.text("label-telescope"),
            text(&s.telescope),
        );
        g.add_opt(
            "instrument",
            self.l.text("label-instrument"),
            text(&s.instrument),
        );
        g.add_opt("filter", self.l.text("label-filter"), text(&s.filter));
        g.add_opt("date_obs", self.l.text("label-observed"), text(&s.date_obs));
        g.add_opt("observer", self.l.text("label-observer"), text(&s.observer));
        if let Some(n) = s.frames {
            let v = match s.frames_declared.filter(|&d| d > n) {
                Some(d) => self
                    .l
                    .fmt("science-frames-cut", &args!("frames" => n, "declared" => d)),
                None => self.l.int(n),
            };
            g.add("frames", self.l.text("label-frames"), v, None);
        }
        g.add_opt(
            "capture_start",
            self.l.text("label-capture-start"),
            text(&s.capture_start),
        );
        g.add_opt(
            "capture_end",
            self.l.text("label-capture-end"),
            text(&s.capture_end),
        );
        if let Some(r) = s.frame_rate.filter(|r| r.is_finite() && *r > 0.0) {
            let v = self
                .l
                .fmt("science-fps", &args!("fps" => self.l.trimmed(r, 2)));
            g.add("frame_rate", self.l.text("label-frame-rate"), v, None);
        }
        if let Some(t) = s.exposure.filter(|t| t.is_finite() && *t > 0.0) {
            let v = self
                .l
                .fmt("science-seconds", &args!("seconds" => self.l.trimmed(t, 3)));
            g.add("exposure", self.l.text("label-exposure-time"), v, None);
        }
        if !s.dims.is_empty() {
            let v = s
                .dims
                .iter()
                .map(|&d| self.l.int(d))
                .collect::<Vec<_>>()
                .join(" × ");
            g.add("dims", self.l.text("label-dimensions"), v, None);
        }
        g.add_opt("sample", self.l.text("label-sample"), text(&s.sample));
        g.add_opt("bayer", self.l.text("label-bayer"), text(&s.bayer));
        g.add_opt("unit", self.l.text("label-unit"), text(&s.unit));
        g.add_opt("hdu", self.l.text("label-hdu"), text(&s.hdu));
        g.add_opt("dataset", self.l.text("label-dataset"), text(&s.dataset));
        self.pixel_size_row(&mut g, s);
        if let Some((sexa, deg)) = &s.centre {
            g.add("centre", self.l.text("label-centre"), sexa.clone(), None);
            g.add(
                "centre_deg",
                self.l.text("label-centre-deg"),
                deg.clone(),
                None,
            );
        }
        // ICRS, FK5 J2000… are names; the galactic and ecliptic frames are words.
        let frame = s.frame.as_deref().map(|f| match f {
            "Galactic" => self.l.text("science-frame-galactic"),
            "Ecliptic" => self.l.text("science-frame-ecliptic"),
            f => f.to_owned(),
        });
        g.add_opt("frame", self.l.text("label-frame"), non_empty(frame));
        g.add_opt(
            "projection",
            self.l.text("label-projection"),
            text(&s.projection),
        );
        if let Some(a) = s.pixel_scale.filter(|a| a.is_finite() && *a > 0.0) {
            let v = self.l.fmt(
                "science-arcsec-px",
                &args!("arcsec" => self.l.trimmed(a, 3)),
            );
            g.add("pixel_scale", self.l.text("label-pixel-scale"), v, None);
        }
        g.add_opt(
            "distortion",
            self.l.text("label-distortion"),
            text(&s.distortion),
        );
        if s.wcs_approximate && s.centre.is_some() {
            g.add(
                "wcs_approximate",
                self.l.text("label-wcs"),
                self.l.text("science-wcs-approximate"),
                None,
            );
        }
        if let Some(why) = non_empty(s.wcs_unsupported.clone()) {
            let v = self
                .l
                .fmt("science-wcs-unsupported", &args!("reason" => why));
            g.add("wcs_unsupported", self.l.text("label-wcs"), v, None);
        }
        g
    }

    /// The thermal group: camera, object parameters, raw and temperature ranges.
    fn thermal(&self, mut g: GroupBuf, s: &ScienceFacts, t: &ThermalFacts) -> GroupBuf {
        let text = |v: &Option<String>| non_empty(v.clone());
        let finite = |v: Option<f64>| v.filter(|v| v.is_finite());
        let deg = |v: f64| {
            self.l.fmt(
                "thermal-temperature",
                &args!("value" => self.l.trimmed(v, 2), "unit" => t.unit.clone()),
            )
        };
        g.add_opt(
            "thermal_format",
            self.l.text("label-format"),
            non_empty(Some(t.format.clone())),
        );
        g.add_opt("camera", self.l.text("label-camera"), text(&t.camera));
        g.add_opt("lens", self.l.text("label-lens"), text(&t.lens));
        g.add_opt(
            "date_obs",
            self.l.text("label-acquired"),
            self.acquired(&s.date_obs),
        );
        g.add_opt(
            "emissivity",
            self.l.text("label-emissivity"),
            finite(t.emissivity).map(|e| self.l.trimmed(e, 3)),
        );
        g.add_opt(
            "object_distance",
            self.l.text("label-object-distance"),
            finite(t.distance).map(|d| {
                self.l
                    .fmt("thermal-metres", &args!("value" => self.l.trimmed(d, 2)))
            }),
        );
        g.add_opt(
            "reflected",
            self.l.text("label-reflected-temperature"),
            finite(t.reflected).map(deg),
        );
        g.add_opt(
            "atmospheric",
            self.l.text("label-atmospheric-temperature"),
            finite(t.atmospheric).map(deg),
        );
        g.add_opt(
            "humidity",
            self.l.text("label-humidity"),
            finite(t.humidity).map(|h| {
                self.l.fmt(
                    "thermal-percent",
                    &args!("value" => self.l.trimmed(h * 100.0, 1)),
                )
            }),
        );
        if let Some((lo, hi)) = t.raw_range {
            let v = self.l.fmt(
                "thermal-range",
                &args!("low" => self.l.int(u64::from(lo)), "high" => self.l.int(u64::from(hi))),
            );
            g.add("raw_range", self.l.text("label-raw-range"), v, None);
        }
        if let Some((lo, hi)) = t
            .temperature_range
            .filter(|(l, h)| l.is_finite() && h.is_finite())
        {
            let v = self
                .l
                .fmt("thermal-range", &args!("low" => deg(lo), "high" => deg(hi)));
            g.add(
                "temperature_range",
                self.l.text("label-temperature-range"),
                v,
                None,
            );
        }
        if !s.dims.is_empty() {
            let v = s
                .dims
                .iter()
                .map(|&d| self.l.int(d))
                .collect::<Vec<_>>()
                .join(" × ");
            g.add("dims", self.l.text("label-dimensions"), v, None);
        }
        g
    }

    /// The microscopy group (electron or scanning probe).
    /// An acquisition date as the rest of the panel shows dates ("Monday 12 October 2026,
    /// 09:30"): ISO 8601 with a `T` or a space, or EXIF's colons; anything else as written.
    fn acquired(&self, v: &Option<String>) -> Option<String> {
        let raw = non_empty(v.clone())?;
        let t = raw.trim();
        let digits = t.bytes().take_while(u8::is_ascii_digit).count();
        let iso = match t.as_bytes().get(10) {
            Some(b' ') if t.len() >= 16 => format!("{}T{}", &t[..10], &t[11..]),
            // DICOM DT: YYYYMMDDHHMM[SS[.F]][&ZZXX].
            _ if matches!(digits, 12 | 14) => {
                let (d, rest) = t.split_at(digits);
                let secs = if digits == 14 { &d[12..14] } else { "00" };
                let (frac, zone) = match rest.find(['+', '-']) {
                    Some(z) => rest.split_at(z),
                    None => (rest, ""),
                };
                let zone = match zone.len() {
                    5 => format!("{}:{}", &zone[..3], &zone[3..]),
                    _ => String::new(),
                };
                format!(
                    "{}-{}-{}T{}:{}:{secs}{frac}{zone}",
                    &d[..4],
                    &d[4..6],
                    &d[6..8],
                    &d[8..10],
                    &d[10..12]
                )
            }
            _ => t.to_owned(),
        };
        let stamp = datetime::parse_xmp_datetime(&iso)
            .filter(|_| iso.len() >= 16)
            .or_else(|| {
                datetime::parse_exif_datetime(t).map(|local| Stamp {
                    local,
                    offset_minutes: None,
                })
            });
        Some(stamp.map_or(raw, |s| s.display(self.l)))
    }

    fn microscopy(&self, mut g: GroupBuf, s: &ScienceFacts) -> GroupBuf {
        // An OMERO image's name, dataset, server, acquisition date, sizes and physical
        // pixel size are rows of its OMERO group: not told twice.
        let omero = s.omero.is_some();
        let text = |v: &Option<String>| non_empty(v.clone());
        let unless_omero = |v: &Option<String>| text(v).filter(|_| !omero);
        let acquired = self.acquired(&s.date_obs).filter(|_| !omero);
        let slide = s.domain == ScienceDomain::Slide;
        g.add_opt("title", self.l.text("label-title"), unless_omero(&s.title));
        g.add_opt(
            "dataset",
            self.l.text("label-dataset"),
            unless_omero(&s.dataset),
        );
        g.add_opt(
            "vendor",
            self.l.text("label-vendor"),
            unless_omero(&s.vendor),
        );
        let instrument = if slide {
            "label-scanner"
        } else if matches!(
            s.domain,
            ScienceDomain::Gridded
                | ScienceDomain::Surface
                | ScienceDomain::Seismic
                | ScienceDomain::MassSpec
                | ScienceDomain::Spectroscopy
                | ScienceDomain::Cytometry
        ) {
            "label-instrument"
        } else {
            "label-microscope"
        };
        g.add_opt("instrument", self.l.text(instrument), text(&s.instrument));
        if let Some(v) = s.voltage.filter(|v| v.is_finite() && *v > 0.0) {
            let v = self.l.fmt(
                "science-kilovolts",
                &args!("kv" => self.l.trimmed(v / 1000.0, 3)),
            );
            g.add("voltage", self.l.text("label-voltage"), v, None);
        }
        if let Some(m) = s.magnification.filter(|m| m.is_finite() && *m > 0.0) {
            let v = self.l.fmt(
                "science-magnification",
                &args!("value" => self.l.trimmed(m, 2)),
            );
            let label = if slide {
                "label-objective"
            } else {
                "label-magnification"
            };
            g.add("magnification", self.l.text(label), v, None);
        }
        g.add_opt("mode", self.l.text("label-imaging-mode"), text(&s.mode));
        g.add_opt("detector", self.l.text("label-detector"), text(&s.detector));
        g.add_opt("date_obs", self.l.text("label-acquired"), acquired);
        if let Some(t) = s.exposure.filter(|t| t.is_finite() && *t > 0.0) {
            let v = self
                .l
                .fmt("science-seconds", &args!("seconds" => self.l.trimmed(t, 3)));
            g.add("exposure", self.l.text("label-exposure-time"), v, None);
        }
        if !s.dims.is_empty() && !omero {
            let v = s
                .dims
                .iter()
                .map(|&d| self.l.int(d))
                .collect::<Vec<_>>()
                .join(" × ");
            g.add("dims", self.l.text("label-dimensions"), v, None);
        }
        if !omero {
            self.pixel_size_row(&mut g, s);
        }
        g.add_opt(
            "field_of_view",
            self.l.text("label-field-of-view"),
            text(&s.field_of_view),
        );
        if s.levels.len() > 1 {
            let sizes = s
                .levels
                .iter()
                .map(|&(w, h)| {
                    format!(
                        "{} × {}",
                        self.l.int(u64::from(w)),
                        self.l.int(u64::from(h))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let v = self.l.fmt(
                "slide-levels",
                &args!("count" => s.levels.len() as u64, "sizes" => sizes),
            );
            g.add("levels", self.l.text("label-pyramid-levels"), v, None);
        }
        g.add_opt("sample", self.l.text("label-sample"), text(&s.sample));
        g.add_opt("unit", self.l.text("label-unit"), text(&s.unit));
        g
    }

    /// The diffraction group: the detector, then the experiment geometry.
    fn diffraction(&self, mut g: GroupBuf, s: &ScienceFacts) -> GroupBuf {
        let text = |v: &Option<String>| non_empty(v.clone());
        let ok = |v: Option<f64>| v.filter(|v| v.is_finite());
        let pos = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0);
        g.add_opt("title", self.l.text("label-title"), text(&s.title));
        g.add_opt("detector", self.l.text("label-detector"), text(&s.detector));
        g.add_opt(
            "date_obs",
            self.l.text("label-acquired"),
            self.acquired(&s.date_obs),
        );
        if let Some(t) = pos(s.exposure) {
            let v = self
                .l
                .fmt("science-seconds", &args!("seconds" => self.l.trimmed(t, 3)));
            g.add("exposure", self.l.text("label-exposure-time"), v, None);
        }
        if !s.dims.is_empty() {
            let v = s
                .dims
                .iter()
                .map(|&d| self.l.int(d))
                .collect::<Vec<_>>()
                .join(" × ");
            g.add("dims", self.l.text("label-dimensions"), v, None);
        }
        self.pixel_size_row(&mut g, s);
        g.add_opt("sample", self.l.text("label-sample"), text(&s.sample));
        let Some(d) = &s.diffraction else {
            return g;
        };
        let value = |key: &str, v: f64, decimals: u8| {
            self.l
                .fmt(key, &args!("value" => self.l.trimmed(v, decimals)))
        };
        if let Some(l) = pos(d.wavelength) {
            let v = value("xtal-angstrom", l, 5);
            g.add("wavelength", self.l.text("label-wavelength"), v, None);
        }
        if let Some(e) = pos(d.energy) {
            let v = value("xtal-kev", e, 3);
            g.add("energy", self.l.text("label-photon-energy"), v, None);
        }
        if let Some(mm) = pos(d.distance) {
            let v = value("xtal-mm", mm, 2);
            g.add("distance", self.l.text("label-detector-distance"), v, None);
        }
        if let Some((x, y)) = d.beam.filter(|(x, y)| x.is_finite() && y.is_finite()) {
            let v = self.l.fmt(
                "xtal-beam-centre",
                &args!("x" => self.l.trimmed(x, 2), "y" => self.l.trimmed(y, 2)),
            );
            g.add("beam", self.l.text("label-beam-centre"), v, None);
        }
        for (key, label, angle) in [
            ("two_theta", "label-detector-swing", d.two_theta),
            ("phi", "label-phi", d.phi),
            ("omega", "label-omega", d.omega),
            ("chi", "label-chi", d.chi),
            ("kappa", "label-kappa", d.kappa),
        ] {
            if let Some(a) = ok(angle) {
                let v = value("xtal-degrees", a, 4);
                g.add(key, self.l.text(label), v, None);
            }
        }
        if let Some((start, width, axis)) = d
            .oscillation
            .as_ref()
            .filter(|(s, w, _)| s.is_finite() && w.is_finite() && *w != 0.0)
        {
            let (start, width) = (self.l.trimmed(*start, 4), self.l.trimmed(*width, 4));
            let v = match non_empty(axis.clone()) {
                Some(axis) => self.l.fmt(
                    "xtal-oscillation-axis",
                    &args!("start" => start, "width" => width, "axis" => axis),
                ),
                None => self.l.fmt(
                    "xtal-oscillation",
                    &args!("start" => start, "width" => width),
                ),
            };
            g.add("oscillation", self.l.text("label-oscillation"), v, None);
        }
        if let Some(o) = pos(d.overload) {
            g.add(
                "overload",
                self.l.text("label-overload"),
                self.l.trimmed(o, 0),
                None,
            );
        }
        if let Some((edge, corner)) = d.resolution {
            let v = self.l.fmt(
                "xtal-resolution",
                &args!("edge" => self.l.trimmed(edge, 2), "corner" => self.l.trimmed(corner, 2)),
            );
            g.add("resolution", self.l.text("label-resolution"), v, None);
        }
        if let Some((x, y)) = d.binning.filter(|&(x, y)| x > 1 || y > 1) {
            let v = self.l.fmt(
                "dimensions",
                &args!("width" => self.l.int(u64::from(x)), "height" => self.l.int(u64::from(y))),
            );
            g.add("binning", self.l.text("label-volume-binning"), v, None);
        }
        if d.curved {
            g.add(
                "geometry",
                self.l.text("label-geometry"),
                self.l.text("xtal-curved"),
                None,
            );
        } else if !d.complete {
            g.add(
                "geometry",
                self.l.text("label-geometry"),
                self.l.text("xtal-incomplete"),
                None,
            );
        }
        g
    }

    // ----- 3D models --------------------------------------------------------

    fn mesh(&self) -> GroupBuf {
        let mut g = GroupBuf::new("mesh", self.l.text("group-mesh"));
        let Some(m) = &self.input.mesh else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add_opt("title", self.l.text("label-title"), text(&m.title));
        g.add(
            "triangles",
            self.l.text("label-triangles"),
            self.l.int(m.triangles),
            None,
        );
        g.add(
            "vertices",
            self.l.text("label-vertices"),
            self.l.int(m.vertices),
            None,
        );
        if m.size.iter().all(|v| v.is_finite()) {
            let mut v = m
                .size
                .iter()
                .map(|&d| self.l.trimmed(d, 3))
                .collect::<Vec<_>>()
                .join(" × ");
            if let Some(u) = text(&m.unit) {
                v = format!("{v} {u}");
            }
            g.add("size", self.l.text("label-dimensions"), v, None);
        }
        g.add_opt("unit", self.l.text("label-unit"), text(&m.unit));
        g.add_opt("up", self.l.text("label-up-axis"), text(&m.up));
        if !m.materials.is_empty() {
            g.add(
                "materials",
                self.l.text("label-materials"),
                m.materials.join(", "),
                None,
            );
        }
        if m.textures > 0 {
            let v = if m.textures_missing > 0 {
                self.l.fmt(
                    "mesh-textures-missing",
                    &args!("count" => m.textures as u64, "missing" => m.textures_missing as u64),
                )
            } else {
                self.l.int(m.textures as u64)
            };
            g.add("textures", self.l.text("label-textures"), v, None);
        }
        g.add_opt(
            "generator",
            self.l.text("label-software"),
            text(&m.generator),
        );
        g
    }

    // ----- Molecular structures ----------------------------------------------

    /// The GIFTI group: the file (version, role, arrays, encoding), the surface (vertices,
    /// triangles, structure, geometric type, transform, the file it is shown on) and the
    /// array shown (name, intent, data type, label entries).
    fn gifti(&self) -> GroupBuf {
        let mut g = GroupBuf::new("gifti", self.l.text("group-gifti"));
        let Some(f) = self.input.neuro.as_ref().and_then(|n| n.gifti.as_ref()) else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add_opt(
            "gifti_version",
            self.l.text("label-gifti-version"),
            non_empty(Some(f.version.clone())),
        );
        if matches!(f.role.as_str(), "surface" | "labels" | "data") {
            g.add(
                "gifti_role",
                self.l.text("label-gifti-role"),
                self.l.text(&format!("gifti-role-{}", f.role)),
                None,
            );
        }
        if f.arrays > 0 {
            g.add(
                "gifti_arrays",
                self.l.text("label-gifti-arrays"),
                self.l.int(f.arrays),
                None,
            );
        }
        g.add_opt(
            "gifti_encoding",
            self.l.text("label-gifti-encoding"),
            non_empty(Some(f.encoding.clone())),
        );
        g.add_opt(
            "gifti_vertices",
            self.l.text("label-gifti-vertices"),
            f.vertices.map(|n| self.l.int(n)),
        );
        g.add_opt(
            "gifti_triangles",
            self.l.text("label-gifti-triangles"),
            f.triangles.map(|n| self.l.int(n)),
        );
        g.add_opt(
            "gifti_structure",
            self.l.text("label-gifti-structure"),
            text(&f.structure),
        );
        g.add_opt(
            "gifti_geometry",
            self.l.text("label-gifti-geometry"),
            text(&f.geometry),
        );
        if let Some((from, to, applied)) = &f.transform {
            let key = if *applied {
                "gifti-transform-applied"
            } else {
                "gifti-transform"
            };
            g.add(
                "gifti_transform",
                self.l.text("label-gifti-transform"),
                self.l
                    .fmt(key, &args!("from" => from.clone(), "to" => to.clone())),
                None,
            );
        }
        g.add_opt(
            "gifti_surface",
            self.l.text("label-gifti-surface"),
            text(&f.surface),
        );
        g.add_opt(
            "gifti_array",
            self.l.text("label-gifti-array"),
            text(&f.array),
        );
        g.add_opt(
            "gifti_intent",
            self.l.text("label-gifti-intent"),
            text(&f.intent),
        );
        g.add_opt(
            "gifti_data_type",
            self.l.text("label-gifti-data-type"),
            text(&f.data_type),
        );
        if f.labels > 0 {
            g.add(
                "gifti_labels",
                self.l.text("label-gifti-labels"),
                self.l.int(f.labels),
                None,
            );
        }
        g
    }

    /// The CIFTI group: version, intent, matrix size, the maps and what indexes them, the
    /// brain models (structures, voxels, parcels), the surfaces it needs and those found,
    /// and how it is shown.
    fn cifti(&self) -> GroupBuf {
        let mut g = GroupBuf::new("cifti", self.l.text("group-cifti"));
        let Some(c) = self.input.neuro.as_ref().and_then(|n| n.cifti.as_ref()) else {
            return g;
        };
        g.add_opt(
            "cifti_version",
            self.l.text("label-cifti-version"),
            non_empty(Some(c.version.clone())),
        );
        g.add_opt(
            "cifti_intent",
            self.l.text("label-cifti-intent"),
            non_empty(Some(c.intent.clone())),
        );
        g.add(
            "cifti_matrix",
            self.l.text("label-cifti-matrix"),
            self.l.fmt(
                "cifti-matrix",
                &args!("rows" => c.shape[0], "columns" => c.shape[1]),
            ),
            None,
        );
        let kind = match c.map_kind.as_str() {
            k @ ("scalars" | "labels" | "series" | "parcels") => k,
            _ => "other",
        };
        if let Some((seeds, row)) = c.dense {
            g.add(
                "cifti_dense",
                self.l.text("label-cifti-dense"),
                self.l.fmt(
                    "cifti-dense",
                    &args!(
                        "seeds" => seeds,
                        "size" => strings::file_size(self.l, row)
                    ),
                ),
                None,
            );
        } else {
            g.add(
                "cifti_maps",
                self.l.text("label-cifti-maps"),
                self.l
                    .fmt(&format!("cifti-maps-{kind}"), &args!("count" => c.maps)),
                None,
            );
        }
        if let Some((start, step, unit)) = &c.series {
            g.add(
                "cifti_series",
                self.l.text("label-cifti-series"),
                self.l.fmt(
                    "cifti-series",
                    &args!(
                        "start" => self.general(*start),
                        "step" => self.general(*step),
                        "unit" => unit.to_ascii_lowercase()
                    ),
                ),
                None,
            );
        }
        if c.models > 0 {
            g.add(
                "cifti_models",
                self.l.text("label-cifti-models"),
                self.l.int(c.models),
                None,
            );
        }
        g.add_opt(
            "cifti_parcels",
            self.l.text("label-cifti-parcels"),
            c.parcels.map(|n| self.l.int(n)),
        );
        g.add_opt(
            "cifti_structures",
            self.l.text("label-cifti-structures"),
            non_empty(Some(c.structures.join(", "))),
        );
        if c.voxels > 0 {
            g.add(
                "cifti_voxels",
                self.l.text("label-cifti-voxels"),
                self.l.int(c.voxels),
                None,
            );
        }
        for (structure, vertices, file) in &c.surfaces {
            let value = match file {
                Some(f) => self.l.fmt(
                    "cifti-surface-found",
                    &args!(
                        "structure" => structure.clone(),
                        "vertices" => *vertices,
                        "file" => f.clone()
                    ),
                ),
                None => self.l.fmt(
                    "cifti-surface-missing",
                    &args!(
                        "structure" => structure.clone(),
                        "vertices" => *vertices
                    ),
                ),
            };
            g.add(
                "cifti_surface",
                self.l.text("label-cifti-surface"),
                value,
                None,
            );
        }
        if !c.shown.is_empty() {
            let key = if c.dense.is_some() {
                "cifti-shown-connectivity"
            } else if c.shown == "voxels" {
                "cifti-shown-voxels"
            } else {
                "cifti-shown-surfaces"
            };
            g.add(
                "cifti_shown",
                self.l.text("label-cifti-shown"),
                self.l.text(key),
                None,
            );
        }
        if c.maps_left_out > 0 {
            g.add(
                "cifti_left_out",
                self.l.text("label-cifti-left-out"),
                self.l.int(c.maps_left_out),
                None,
            );
        }
        g
    }

    /// The tractography group: format, streamlines and points (and how many are drawn),
    /// mean length, per-point scalars and per-streamline properties, the TrackVis grid, the
    /// space the points were brought into, and the step size.
    fn tracts(&self) -> GroupBuf {
        let mut g = GroupBuf::new("tractography", self.l.text("group-tractography"));
        let Some(t) = self.input.neuro.as_ref().and_then(|n| n.tracts.as_ref()) else {
            return g;
        };
        let mm = |v: String| self.l.fmt("tract-mm", &args!("value" => v));
        g.add_opt(
            "tract_format",
            self.l.text("label-tract-format"),
            non_empty(Some(t.format.clone())),
        );
        let streamlines = if t.shown_streamlines > 0 && t.shown_streamlines < t.streamlines {
            self.l.fmt(
                "tract-streamlines-shown",
                &args!(
                    "count" => t.streamlines,
                    "shown" => t.shown_streamlines
                ),
            )
        } else {
            self.l.int(t.streamlines)
        };
        g.add(
            "tract_streamlines",
            self.l.text("label-tract-streamlines"),
            streamlines,
            None,
        );
        g.add(
            "tract_points",
            self.l.text("label-tract-points"),
            self.l.int(t.points),
            None,
        );
        g.add_opt(
            "tract_mean_length",
            self.l.text("label-tract-mean-length"),
            t.mean_length
                .filter(|l| l.is_finite())
                .map(|l| mm(self.l.trimmed(l, 1))),
        );
        g.add_opt(
            "tract_scalars",
            self.l.text("label-tract-scalars"),
            non_empty(Some(t.scalars.join(", "))),
        );
        g.add_opt(
            "tract_properties",
            self.l.text("label-tract-properties"),
            non_empty(Some(t.properties.join(", "))),
        );
        if let Some(d) = t.dims.filter(|d| d.iter().all(|&n| n > 0)) {
            g.add(
                "tract_grid",
                self.l.text("label-tract-grid"),
                format!("{} × {} × {}", d[0], d[1], d[2]),
                None,
            );
        }
        if let Some(v) = t
            .voxel_size
            .filter(|v| v.iter().all(|x| x.is_finite() && *x > 0.0))
        {
            let n = |x: f64| self.l.trimmed(x, 3);
            g.add(
                "tract_voxel_size",
                self.l.text("label-tract-voxel-size"),
                mm(format!("{} × {} × {}", n(v[0]), n(v[1]), n(v[2]))),
                None,
            );
        }
        g.add_opt(
            "tract_voxel_order",
            self.l.text("label-tract-voxel-order"),
            non_empty(t.voxel_order.clone()),
        );
        let space = match t.space.as_str() {
            "vox_to_ras" => Some("tract-space-vox-to-ras"),
            "voxel order" => Some("tract-space-voxel-order"),
            "scanner" => Some("tract-space-scanner"),
            _ => None,
        };
        g.add_opt(
            "tract_space",
            self.l.text("label-tract-space"),
            space.map(|k| self.l.text(k)),
        );
        g.add_opt(
            "tract_step",
            self.l.text("label-tract-step"),
            non_empty(t.step.clone()).map(mm),
        );
        g
    }

    fn font(&self) -> GroupBuf {
        let mut g = GroupBuf::new("font", self.l.text("group-font"));
        let Some(f) = &self.input.font else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add_opt("family", self.l.text("label-font-family"), text(&f.family));
        g.add_opt("style", self.l.text("label-font-style"), text(&f.style));
        if let Some((i, n)) = f.face.filter(|(_, n)| *n > 1) {
            g.add(
                "face",
                self.l.text("label-font-face"),
                self.l
                    .fmt("font-face-of", &args!("index" => i + 1, "count" => n)),
                None,
            );
        }
        g.add_opt(
            "version",
            self.l.text("label-font-version"),
            text(&f.version),
        );
        g.add_opt(
            "designer",
            self.l.text("label-font-designer"),
            text(&f.designer),
        );
        g.add_opt(
            "manufacturer",
            self.l.text("label-font-manufacturer"),
            text(&f.manufacturer),
        );
        g.add(
            "glyphs",
            self.l.text("label-font-glyphs"),
            self.l.int(f.glyphs),
            None,
        );
        g.add(
            "outlines",
            self.l.text("label-font-outlines"),
            self.l.text(&format!("font-outlines-{}", f.outlines)),
            None,
        );
        if let Some(e) = f.embedding {
            let mut v = self.l.text(&format!("font-embedding-{e}"));
            if f.no_subsetting {
                v = format!("{v} {}", self.l.text("font-embedding-no-subsetting"));
            }
            if f.bitmap_embedding_only {
                v = format!("{v} {}", self.l.text("font-embedding-bitmap-only"));
            }
            g.add("embedding", self.l.text("label-font-embedding"), v, None);
        }
        if !f.axes.is_empty() {
            let list = f
                .axes
                .iter()
                .map(|(tag, name, min, def, max)| {
                    let name = if name.trim().is_empty() { tag } else { name };
                    self.l.fmt(
                        "font-axis",
                        &args!(
                            "name" => name.clone(),
                            "min" => self.l.trimmed(*min, 2),
                            "default" => self.l.trimmed(*def, 2),
                            "max" => self.l.trimmed(*max, 2)
                        ),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            g.add("axes", self.l.text("label-font-axes"), list, None);
        }
        if f.instances > 0 {
            g.add(
                "instances",
                self.l.text("label-font-instances"),
                self.l.int(f.instances),
                None,
            );
        }
        if !f.colour.is_empty() {
            let list = f
                .colour
                .iter()
                .map(|c| self.l.text(&format!("font-colour-{c}")))
                .collect::<Vec<_>>()
                .join(", ");
            g.add("colour", self.l.text("label-font-colour"), list, None);
        }
        if let Some(w) = f.weight.filter(|w| (1..=1000).contains(w)) {
            g.add(
                "weight",
                self.l.text("label-font-weight"),
                self.l.int(w),
                None,
            );
        }
        if f.units_per_em > 0 {
            g.add(
                "units-per-em",
                self.l.text("label-font-units-per-em"),
                self.l.int(f.units_per_em),
                None,
            );
        }
        if !f.tables.is_empty() {
            g.add(
                "tables",
                self.l.text("label-font-tables"),
                f.tables.join(" "),
                None,
            );
        }
        g.add_opt(
            "licence",
            self.l.text("label-font-licence"),
            text(&f.licence),
        );
        g.add_opt(
            "licence-url",
            self.l.text("label-font-licence-url"),
            text(&f.licence_url),
        );
        g.add_opt(
            "copyright",
            self.l.text("label-font-copyright"),
            text(&f.copyright),
        );
        g.add_opt(
            "trademark",
            self.l.text("label-font-trademark"),
            text(&f.trademark),
        );
        g
    }

    fn document(&self) -> GroupBuf {
        let mut g = GroupBuf::new("document", self.l.text("group-document"));
        let Some(d) = &self.input.document else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        let mut kind = self.l.text(&format!("document-kind-{}", d.kind));
        for (on, id) in [
            (d.template, "document-template"),
            (d.macros, "document-macros"),
            (d.legacy, "document-legacy"),
        ] {
            if on {
                kind = format!("{kind}, {}", self.l.text(id));
            }
        }
        g.add("kind", self.l.text("label-document-kind"), kind, None);
        // A playlist has no picture to preview: its items are the fact.
        if let Some(n) = d.items {
            g.add(
                "items",
                self.l.text("label-playlist-items"),
                self.l.int(n),
                None,
            );
        } else {
            let preview = match &d.preview {
                Some(f) => self
                    .l
                    .fmt("document-preview-image", &args!("format" => f.as_str())),
                None => self.l.text("document-preview-none"),
            };
            g.add(
                "preview",
                self.l.text("label-document-preview"),
                preview,
                None,
            );
        }
        g.add_opt("title", self.l.text("label-title"), text(&d.title));
        for (key, label, v) in [
            ("subject", "label-document-subject", &d.subject),
            ("author", "label-document-author", &d.author),
            ("keywords", "label-keywords", &d.keywords),
            ("description", "label-document-description", &d.description),
            ("category", "label-document-category", &d.category),
            (
                "last-saved-by",
                "label-document-last-saved-by",
                &d.last_modified_by,
            ),
        ] {
            g.add_opt(key, self.l.text(label), text(v));
        }
        let when = |v: &Option<String>| {
            let s = text(v)?;
            Some(match datetime::parse_xmp_datetime(&s) {
                Some(t) => t.display(self.l),
                None => s,
            })
        };
        for (key, label, v) in [
            ("created", "label-document-created", &d.created),
            ("modified", "label-document-modified", &d.modified),
            ("printed", "label-document-printed", &d.printed),
        ] {
            g.add_opt(key, self.l.text(label), when(v));
        }
        for (key, label, v) in [
            ("pages", "label-document-pages", d.pages),
            ("slides", "label-document-slides", d.slides),
            (
                "hidden-slides",
                "label-document-hidden-slides",
                d.hidden_slides,
            ),
            ("notes", "label-document-notes", d.notes),
            ("sheets", "label-document-sheets", d.sheets),
            ("words", "label-document-words", d.words),
            ("characters", "label-document-characters", d.characters),
            ("paragraphs", "label-document-paragraphs", d.paragraphs),
            ("lines", "label-document-lines", d.lines),
        ] {
            if let Some(n) = v {
                g.add(key, self.l.text(label), self.l.int(n), None);
            }
        }
        if let Some(m) = d.edit_minutes {
            g.add(
                "editing-time",
                self.l.text("label-document-editing-time"),
                self.l
                    .fmt("document-minutes", &args!("minutes" => m as i64)),
                None,
            );
        }
        for (key, label, v) in [
            ("revision", "label-document-revision", &d.revision),
            ("application", "label-document-application", &d.application),
            ("company", "label-document-company", &d.company),
            ("template", "label-document-template", &d.template_name),
        ] {
            g.add_opt(key, self.l.text(label), text(v));
        }
        g
    }

    fn book(&self) -> GroupBuf {
        let mut g = GroupBuf::new("book", self.l.text("group-book"));
        let Some(b) = &self.input.book else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        let list = |v: &[String]| {
            non_empty(Some(
                v.iter()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        };
        g.add_opt("title", self.l.text("label-title"), text(&b.title));
        g.add_opt(
            "authors",
            self.l.text("label-book-authors"),
            list(&b.authors),
        );
        g.add_opt(
            "publisher",
            self.l.text("label-book-publisher"),
            text(&b.publisher),
        );
        g.add_opt("date", self.l.text("label-book-date"), text(&b.date));
        g.add_opt(
            "language",
            self.l.text("label-book-language"),
            text(&b.language),
        );
        if b.pages > 0 {
            g.add(
                "pages",
                self.l.text("label-book-pages"),
                self.l.int(b.pages),
                None,
            );
        }
        if b.contents > 0 {
            g.add(
                "contents",
                self.l.text("label-book-contents"),
                self.l.int(b.contents),
                None,
            );
        }
        let mut layout = if b.fixed_layout {
            self.l.text("book-layout-fixed")
        } else {
            self.l.text("book-layout-reflowable")
        };
        if b.right_to_left {
            layout = format!("{layout}, {}", self.l.text("book-right-to-left"));
        }
        g.add("layout", self.l.text("label-book-layout"), layout, None);
        if let Some(v) = text(&b.version) {
            g.add(
                "version",
                self.l.text("label-book-version"),
                self.l.fmt("book-version", &args!("version" => v)),
                None,
            );
        }
        if b.fonts > 0 {
            let v = if b.obfuscated_fonts > 0 {
                self.l.fmt(
                    "book-fonts-obfuscated",
                    &args!(
                        "count" => b.fonts,
                        "obfuscated" => b.obfuscated_fonts
                    ),
                )
            } else {
                self.l.int(b.fonts)
            };
            g.add("fonts", self.l.text("label-book-fonts"), v, None);
        }
        g.add_opt(
            "subjects",
            self.l.text("label-book-subjects"),
            list(&b.subjects),
        );
        g.add_opt(
            "identifier",
            self.l.text("label-book-identifier"),
            text(&b.identifier),
        );
        g.add_opt("rights", self.l.text("label-book-rights"), text(&b.rights));
        g
    }

    fn structure(&self) -> GroupBuf {
        let mut g = GroupBuf::new("structure", self.l.text("group-structure"));
        let Some(s) = &self.input.structure else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add_opt("id", self.l.text("label-structure-id"), text(&s.id));
        g.add_opt("title", self.l.text("label-title"), text(&s.title));
        g.add_opt(
            "classification",
            self.l.text("label-structure-classification"),
            text(&s.classification),
        );
        g.add_opt(
            "method",
            self.l.text("label-structure-method"),
            text(&s.method),
        );
        if let Some(r) = s.resolution.filter(|r| r.is_finite() && *r > 0.0) {
            g.add(
                "resolution",
                self.l.text("label-structure-resolution"),
                self.l.fmt(
                    "structure-resolution",
                    &args!("value" => self.l.trimmed(r, 2)),
                ),
                None,
            );
        }
        g.add_opt(
            "deposited",
            self.l.text("label-structure-deposited"),
            text(&s.date),
        );
        g.add_opt(
            "formula",
            self.l.text("label-structure-formula"),
            text(&s.formula),
        );
        if s.chains > 0 {
            g.add(
                "chains",
                self.l.text("label-structure-chains"),
                self.l.int(s.chains),
                None,
            );
        }
        if s.residues > 0 {
            g.add(
                "residues",
                self.l.text("label-structure-residues"),
                self.l.int(s.residues),
                None,
            );
        }
        let atoms = if s.asymmetric_atoms > 0 && s.asymmetric_atoms != s.atoms {
            self.l.fmt(
                "structure-atoms-cell",
                &args!("count" => s.atoms, "asymmetric" => s.asymmetric_atoms),
            )
        } else {
            self.l.int(s.atoms)
        };
        g.add("atoms", self.l.text("label-structure-atoms"), atoms, None);
        if s.bonds > 0 {
            g.add(
                "bonds",
                self.l.text("label-structure-bonds"),
                self.l.int(s.bonds),
                None,
            );
        }
        // Models and records beyond the first: "20 (the first shown)".
        let first_of =
            |n: u64| (n > 1).then(|| self.l.fmt("structure-first-of", &args!("count" => n)));
        g.add_opt(
            "models",
            self.l.text("label-structure-models"),
            first_of(s.models),
        );
        g.add_opt(
            "records",
            self.l.text("label-structure-records"),
            first_of(s.records),
        );
        if !s.ligands.is_empty() {
            const SHOWN: usize = 12;
            let list = s
                .ligands
                .iter()
                .take(SHOWN)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let v = if s.ligands.len() > SHOWN {
                self.l.fmt(
                    "structure-ligands-more",
                    &args!("list" => list, "more" => (s.ligands.len() - SHOWN) as u64),
                )
            } else {
                list
            };
            g.add("ligands", self.l.text("label-structure-ligands"), v, None);
        }
        if let Some(c) = s.cell {
            let n = |v: f64| self.l.trimmed(v, 3);
            g.add(
                "cell",
                self.l.text("label-structure-cell"),
                self.l.fmt(
                    "structure-cell",
                    &args!(
                        "a" => n(c[0]), "b" => n(c[1]), "c" => n(c[2]),
                        "alpha" => n(c[3]), "beta" => n(c[4]), "gamma" => n(c[5])
                    ),
                ),
                None,
            );
        }
        g.add_opt(
            "space_group",
            self.l.text("label-structure-space-group"),
            text(&s.space_group),
        );
        if let Some(src) = s.secondary {
            let key = match src {
                StructureSecondary::File => "structure-ss-file",
                StructureSecondary::Computed => "structure-ss-computed",
            };
            g.add(
                "secondary",
                self.l.text("label-structure-secondary"),
                self.l.text(key).to_owned(),
                None,
            );
        }
        g
    }
    fn planetary(&self) -> GroupBuf {
        let mut g = GroupBuf::new("planetary", self.l.text("group-planetary"));
        let Some(p) = self
            .input
            .science
            .as_ref()
            .and_then(|s| s.planetary.as_ref())
        else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        for (key, label, v) in [
            ("target", "label-planetary-target", &p.target),
            ("mission", "label-planetary-mission", &p.mission),
            ("spacecraft", "label-planetary-spacecraft", &p.spacecraft),
            ("instrument", "label-instrument", &p.instrument),
            ("start", "label-planetary-start", &p.start),
            ("stop", "label-planetary-stop", &p.stop),
            ("product", "label-planetary-product", &p.product_id),
            ("projection", "label-planetary-projection", &p.projection),
        ] {
            g.add_opt(key, self.l.text(label), text(v));
        }
        let finite = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0);
        let mut res = Vec::new();
        if let Some(m) = finite(p.metres_per_pixel) {
            res.push(self.l.fmt(
                "planetary-metres-per-pixel",
                &args!("metres" => self.l.trimmed(m, 3)),
            ));
        }
        if let Some(d) = finite(p.pixels_per_degree) {
            res.push(self.l.fmt(
                "planetary-pixels-per-degree",
                &args!("pixels" => self.l.trimmed(d, 3)),
            ));
        }
        if !res.is_empty() {
            g.add(
                "resolution",
                self.l.text("label-planetary-resolution"),
                strings::join(self.l, &res),
                None,
            );
        }
        let deg = |v: f64| self.l.trimmed(v, 4);
        if let Some((lat, lon)) = p.centre.filter(|(a, b)| a.is_finite() && b.is_finite()) {
            g.add(
                "centre",
                self.l.text("label-planetary-centre"),
                self.l.fmt(
                    "planetary-lat-lon",
                    &args!("lat" => deg(lat), "lon" => deg(lon)),
                ),
                None,
            );
        }
        if let (Some((s, n)), Some((w, e))) = (p.latitudes, p.longitudes) {
            g.add(
                "coverage",
                self.l.text("label-planetary-coverage"),
                self.l.fmt(
                    "planetary-coverage",
                    &args!(
                        "south" => deg(s),
                        "north" => deg(n),
                        "west" => deg(w),
                        "east" => deg(e)
                    ),
                ),
                None,
            );
        }
        if let Some(r) = finite(p.radius_km) {
            g.add(
                "radius",
                self.l.text("label-planetary-radius"),
                self.l
                    .fmt("planetary-km", &args!("km" => self.l.trimmed(r, 3))),
                None,
            );
        }
        g
    }

    fn hdf4(&self) -> GroupBuf {
        let mut g = GroupBuf::new("hdf4", self.l.text("group-hdf4"));
        let Some(h) = self.input.science.as_ref().and_then(|s| s.hdf4.as_ref()) else {
            return g;
        };
        g.add_opt(
            "library",
            self.l.text("label-hdf4-library"),
            h.library.clone(),
        );
        for (key, label, n) in [
            ("datasets", "label-hdf4-datasets", h.datasets),
            ("images", "label-hdf4-images", h.images),
            ("vgroups", "label-hdf4-vgroups", h.vgroups),
            ("vdatas", "label-hdf4-vdatas", h.vdatas),
        ] {
            g.add(key, self.l.text(label), self.l.int(n), None);
        }
        g
    }

    fn omero(&self) -> GroupBuf {
        let mut g = GroupBuf::new("omero", self.l.text("group-omero"));
        let Some(o) = self.input.science.as_ref().and_then(|s| s.omero.as_ref()) else {
            return g;
        };
        g.add(
            "server",
            self.l.text("label-omero-server"),
            match &o.user {
                Some(u) => self.l.fmt(
                    "omero-server-user",
                    &args!("server" => o.server.clone(), "user" => u.clone()),
                ),
                None => self
                    .l
                    .fmt("omero-server-public", &args!("server" => o.server.clone())),
            },
            None,
        );
        g.add(
            "image",
            self.l.text("label-omero-image"),
            self.l.fmt(
                "omero-image",
                &args!("name" => o.name.clone(), "id" => self.l.int_plain(o.image_id)),
            ),
            None,
        );
        let path = |parts: [&Option<String>; 3]| {
            let v: Vec<&str> = parts.iter().filter_map(|p| p.as_deref()).collect();
            (!v.is_empty()).then(|| v.join(" › "))
        };
        g.add_opt(
            "project",
            self.l.text("label-omero-project"),
            path([&o.project, &o.dataset, &None]),
        );
        g.add_opt(
            "screen",
            self.l.text("label-omero-screen"),
            path([&o.screen, &o.plate, &o.well]),
        );
        g.add_opt(
            "owner",
            self.l.text("label-omero-owner"),
            match (&o.owner, &o.group) {
                (Some(owner), Some(group)) => Some(self.l.fmt(
                    "omero-owner-group",
                    &args!("owner" => owner.clone(), "group" => group.clone()),
                )),
                (owner, group) => owner.clone().or_else(|| group.clone()),
            },
        );
        g.add_opt(
            "acquired",
            self.l.text("label-omero-acquired"),
            self.acquired(&o.acquired),
        );
        let [x, y, z, c, t] = o.size;
        let mut dims = vec![self.l.fmt(
            "omero-size",
            &args!("width" => self.l.int(x), "height" => self.l.int(y)),
        )];
        for (key, n) in [("omero-z", z), ("omero-c", c), ("omero-t", t)] {
            if n > 1 {
                dims.push(self.l.fmt(key, &args!("count" => n)));
            }
        }
        if !o.pixels_type.is_empty() {
            dims.push(o.pixels_type.clone());
        }
        g.add(
            "dimensions",
            self.l.text("label-omero-dimensions"),
            strings::join(self.l, &dims),
            None,
        );
        let physical: Vec<String> = o
            .physical
            .iter()
            .flatten()
            .map(|(v, unit)| format!("{} {}", self.l.trimmed(*v, 4), self.l.unit(unit)))
            .collect();
        if !physical.is_empty() {
            g.add(
                "physical",
                self.l.text("label-omero-physical"),
                physical.join(" × "),
                None,
            );
        }
        if !o.channels.is_empty() {
            let names: Vec<String> = o
                .channels
                .iter()
                .enumerate()
                .map(|(i, (name, on))| {
                    let name = if name.trim().is_empty() {
                        self.l.int_plain(i as u64 + 1)
                    } else {
                        name.trim().to_string()
                    };
                    if *on {
                        name
                    } else {
                        self.l
                            .fmt("omero-channel-hidden", &args!("channel" => name))
                    }
                })
                .collect();
            g.add(
                "channels",
                self.l.text("label-omero-channels"),
                strings::join(self.l, &names),
                None,
            );
        }
        g.add(
            "fetched",
            self.l.text("label-omero-fetched"),
            match o.tiles {
                Some((w, h, levels)) => self.l.fmt(
                    "omero-fetched-tiles",
                    &args!(
                        "encoding" => o.encoding.clone(),
                        "width" => self.l.int_plain(u64::from(w)),
                        "height" => self.l.int_plain(u64::from(h)),
                        "levels" => levels
                    ),
                ),
                None => self.l.fmt(
                    "omero-fetched-whole",
                    &args!("encoding" => o.encoding.clone()),
                ),
            },
            None,
        );
        g
    }

    fn hdf_eos(&self) -> GroupBuf {
        let mut g = GroupBuf::new("hdf_eos", self.l.text("group-hdf-eos"));
        let Some(e) = self
            .input
            .science
            .as_ref()
            .and_then(|s| s.hdf4.as_ref())
            .and_then(|h| h.eos.as_ref())
        else {
            return g;
        };
        g.add_opt(
            "version",
            self.l.text("label-eos-version"),
            e.version.clone(),
        );
        let mut parts = Vec::new();
        for (key, names) in [("eos-grids", &e.grids), ("eos-swaths", &e.swaths)] {
            if !names.is_empty() {
                parts.push(self.l.fmt(
                    key,
                    &args!("count" => names.len(), "names" => strings::join(self.l, names)),
                ));
            }
        }
        if e.points > 0 {
            parts.push(self.l.fmt("eos-points", &args!("count" => e.points)));
        }
        if !parts.is_empty() {
            g.add(
                "structures",
                self.l.text("label-eos-structures"),
                strings::join(self.l, &parts),
                None,
            );
        }
        g.add_opt(
            "structure",
            self.l.text("label-eos-structure"),
            e.grid.clone().or_else(|| e.swath.clone()),
        );
        g.add_opt(
            "projection",
            self.l.text("label-eos-projection"),
            e.projection.clone(),
        );
        if let Some((w, h)) = e.grid_size {
            g.add(
                "grid_size",
                self.l.text("label-eos-grid-size"),
                self.l.fmt(
                    "eos-grid-size",
                    &args!("width" => self.l.int(w), "height" => self.l.int(h)),
                ),
                None,
            );
        }
        if let Some(c) = e.corners.filter(|c| c.iter().all(|v| v.is_finite())) {
            let n = |v: f64| self.l.trimmed(v, if e.corners_degrees { 6 } else { 3 });
            let v = self.l.fmt(
                "geo-bounds",
                &args!("x0" => n(c[0]), "y0" => n(c[1]), "x1" => n(c[2]), "y1" => n(c[3])),
            );
            let v = if e.corners_degrees {
                format!("{v} °")
            } else {
                format!("{v} {}", self.l.unit("m"))
            };
            g.add("corners", self.l.text("label-eos-corners"), v, None);
        }
        g.add_opt(
            "product",
            self.l.text("label-eos-product"),
            e.short_name.clone(),
        );
        let time = match (&e.begin, &e.end) {
            (Some(b), Some(x)) if b != x => Some(
                self.l
                    .fmt("eos-time", &args!("begin" => b.clone(), "end" => x.clone())),
            ),
            (Some(b), _) => Some(b.clone()),
            (None, x) => x.clone(),
        };
        g.add_opt("time", self.l.text("label-eos-time"), time);
        g
    }

    fn rti(&self) -> GroupBuf {
        let mut g = GroupBuf::new("rti", self.l.text("group-rti"));
        let Some(r) = &self.input.rti else {
            return g;
        };
        g.add(
            "format",
            self.l.text("label-format"),
            r.format.clone(),
            None,
        );
        g.add(
            "basis",
            self.l.text("label-rti-basis"),
            r.basis.clone(),
            None,
        );
        if let Some(o) = r.order {
            g.add(
                "order",
                self.l.text("label-rti-order"),
                self.l.int(o.into()),
                None,
            );
        }
        g.add(
            "terms",
            self.l.text("label-rti-terms"),
            self.l.int(r.terms.into()),
            None,
        );
        g.add(
            "colour",
            self.l.text("label-rti-colour"),
            r.colour.clone(),
            None,
        );
        g.add(
            "planes",
            self.l.text("label-rti-planes"),
            self.l.int(r.planes.into()),
            None,
        );
        g.add(
            "size",
            self.l.text("label-dimensions"),
            format!(
                "{} × {}",
                self.l.int(r.width.into()),
                self.l.int(r.height.into())
            ),
            None,
        );
        g.add_opt(
            "compression",
            self.l.text("label-compression"),
            non_empty(r.compression.clone()),
        );
        let list = |v: &[f32]| {
            v.iter()
                .map(|&x| self.l.trimmed(f64::from(x), 4))
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !r.scale.is_empty() {
            g.add(
                "scale",
                self.l.text("label-rti-scale"),
                list(&r.scale),
                None,
            );
        }
        if !r.bias.is_empty() {
            g.add("bias", self.l.text("label-rti-bias"), list(&r.bias), None);
        }
        g
    }

    fn points(&self) -> GroupBuf {
        let mut g = GroupBuf::new("points", self.l.text("group-points"));
        let Some(p) = &self.input.points else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add(
            "format",
            self.l.text("label-format"),
            p.format.clone(),
            None,
        );
        let count = if p.loaded > 0 && p.loaded < p.total {
            self.l.fmt(
                "points-subsampled",
                &args!("loaded" => p.loaded, "total" => p.total),
            )
        } else {
            self.l.int(p.total)
        };
        g.add("count", self.l.text("label-points"), count, None);
        g.add_opt(
            "point-format",
            self.l.text("label-point-format"),
            p.point_format.map(|f| f.to_string()),
        );
        g.add_opt("coding", self.l.text("label-compression"), text(&p.coding));
        if let Some(size) = p.size.filter(|s| s.iter().all(|v| v.is_finite())) {
            let v = size
                .iter()
                .map(|&d| self.l.trimmed(d, 3))
                .collect::<Vec<_>>()
                .join(" × ");
            g.add("size", self.l.text("label-dimensions"), v, None);
        }
        g.add_opt("crs", self.l.text("label-crs"), text(&p.crs));
        if !p.channels.is_empty() {
            let v = p
                .channels
                .iter()
                .map(|c| self.l.text(&format!("points-channel-{c}")))
                .collect::<Vec<_>>()
                .join(", ");
            g.add("channels", self.l.text("label-attributes"), v, None);
        }
        if !p.classes.is_empty() {
            let v = p
                .classes
                .iter()
                .map(|&(c, n)| {
                    let name = if c <= 22 {
                        self.l.text(&format!("point-class-{c}"))
                    } else {
                        self.l
                            .fmt("point-class-other", &args!("class" => u64::from(c)))
                    };
                    format!("{name} ({})", self.l.int(n))
                })
                .collect::<Vec<_>>()
                .join(", ");
            g.add("classes", self.l.text("label-classes"), v, None);
        }
        if !p.scans.is_empty() {
            let v = p
                .scans
                .iter()
                .enumerate()
                .map(|(i, (name, n))| {
                    let name = non_empty(name.clone()).unwrap_or_else(|| (i + 1).to_string());
                    format!("{name} ({})", self.l.int(*n))
                })
                .collect::<Vec<_>>()
                .join(", ");
            g.add("scans", self.l.text("label-scans"), v, None);
        }
        g
    }

    fn splats(&self) -> GroupBuf {
        let mut g = GroupBuf::new("splats", self.l.text("group-splats"));
        let Some(p) = &self.input.splats else {
            return g;
        };
        let format = if p.antialiased {
            self.l.fmt(
                "splats-format-antialiased",
                &args!("format" => p.format.clone()),
            )
        } else {
            p.format.clone()
        };
        g.add("format", self.l.text("label-format"), format, None);
        let count = if p.shown > 0 && p.shown < p.total {
            self.l.fmt(
                "splats-shown-of",
                &args!("shown" => p.shown, "total" => p.total),
            )
        } else {
            self.l.int(p.total)
        };
        g.add("count", self.l.text("label-splats"), count, None);
        let degree = if p.sh_degree == 0 {
            self.l.text("splats-harmonics-none")
        } else {
            p.sh_degree.to_string()
        };
        g.add(
            "harmonics",
            self.l.text("label-splat-harmonics"),
            degree,
            None,
        );
        if let Some(size) = p.size.filter(|s| s.iter().all(|v| v.is_finite())) {
            let v = size
                .iter()
                .map(|&d| self.l.trimmed(d, 3))
                .collect::<Vec<_>>()
                .join(" × ");
            g.add("size", self.l.text("label-dimensions"), v, None);
        }
        g.add_opt("up", self.l.text("label-up-axis"), non_empty(p.up.clone()));
        g
    }

    /// "0.25 nm per pixel" (or "0.25 × 0.5 nm per pixel"), metric lengths with the SI prefix
    /// that suits them, other units as the file names them.
    fn pixel_size_row(&self, g: &mut GroupBuf, s: &ScienceFacts) {
        let Some((x, y, unit)) = s
            .pixel_size
            .as_ref()
            .filter(|(x, y, _)| x.is_finite() && y.is_finite() && *x > 0.0 && *y > 0.0)
        else {
            return;
        };
        let (x, y, unit) = metric_length(*x, *y, unit);
        // Four significant digits.
        let num = |v: f64| {
            let decimals = (3 - v.abs().log10().floor() as i32).clamp(0, 12);
            self.l.trimmed(v, decimals as u8)
        };
        let size = if x == y {
            num(x)
        } else {
            format!("{} × {}", num(x), num(y))
        };
        let v = self.l.fmt(
            "science-pixel-size",
            &args!("size" => size, "unit" => self.l.unit(&unit)),
        );
        g.add("pixel_size", self.l.text("label-pixel-size"), v, None);
    }

    /// The header keywords as an "all fields" group.
    /// The header group: the facts a film scan or legacy format's header gives.
    fn header(&self) -> GroupBuf {
        let mut g = GroupBuf::new("header", self.l.text("group-header"));
        let Some(h) = &self.input.header else {
            return g;
        };
        // A key given more than once is one row, its values joined in the UI's direction.
        let mut done: Vec<&str> = Vec::new();
        for (key, _) in &h.summary {
            let Some(id) = HEADER_KEYS.iter().find(|k| **k == *key) else {
                continue;
            };
            if done.contains(id) {
                continue;
            }
            done.push(id);
            let values: Vec<&str> = h
                .summary
                .iter()
                .filter(|(k, _)| k == id)
                .map(|(_, v)| v.trim())
                .collect();
            g.add(
                id,
                self.l.text(&format!("label-header-{id}")),
                match values[..] {
                    [one] => one.to_owned(),
                    _ => self.l.dot_join(&values),
                },
                None,
            );
        }
        g
    }

    /// Every header field, under "<format> header".
    fn header_fields(&self) -> Option<FieldGroup> {
        let h = self.input.header.as_ref()?;
        if h.fields.is_empty() {
            return None;
        }
        let fields = h
            .fields
            .iter()
            .map(|(k, v)| Field {
                tag: k.clone(),
                raw: v.clone(),
                display: v.clone(),
                empty: v.trim().is_empty(),
                full: None,
            })
            .collect();
        let title = self.l.fmt(
            "fields-science-header",
            &args!("format" => h.format.clone()),
        );
        Some(FieldGroup { title, fields })
    }

    fn science_fields(&self) -> Option<FieldGroup> {
        let s = self.input.science.as_ref()?;
        if s.keywords.is_empty() {
            return None;
        }
        let fields = s
            .keywords
            .iter()
            .map(|(k, v, c)| {
                let display = if c.trim().is_empty() {
                    v.clone()
                } else {
                    format!("{v}  / {c}")
                };
                Field {
                    tag: k.clone(),
                    raw: v.clone(),
                    empty: v.trim().is_empty() && c.trim().is_empty(),
                    display,
                    full: None,
                }
            })
            .collect();
        let title = if s.format == "ASDF" {
            self.l.text("fields-asdf-tree")
        } else if s.format == "ROOT" {
            self.l.text("fields-root-keys")
        } else {
            self.l.fmt(
                "fields-science-header",
                &args!("format" => s.format.clone()),
            )
        };
        Some(FieldGroup { title, fields })
    }

    /// The ASDF group: the data model and the versions of the file, its standard and the
    /// library that wrote it.
    fn asdf(&self) -> GroupBuf {
        let mut g = GroupBuf::new("asdf", self.l.text("group-asdf"));
        let Some(a) = self.input.science.as_ref().and_then(|s| s.asdf.as_ref()) else {
            return g;
        };
        g.add_opt(
            "asdf_model",
            self.l.text("label-asdf-model"),
            non_empty(a.model.clone()),
        );
        g.add_opt(
            "asdf_version",
            self.l.text("label-asdf-version"),
            non_empty(Some(a.version.clone())),
        );
        g.add_opt(
            "asdf_standard",
            self.l.text("label-asdf-standard"),
            non_empty(a.standard.clone()),
        );
        g.add_opt(
            "asdf_library",
            self.l.text("label-asdf-library"),
            non_empty(a.library.clone()),
        );
        g
    }

    /// A value of any size: up to six significant digits, in scientific notation beyond
    /// 10⁷ or below 10⁻³.
    fn general(&self, x: f64) -> String {
        let a = x.abs();
        if a == 0.0 || (1e-3..1e7).contains(&a) {
            // (log10 of 0 is −∞, which would overflow the subtraction.)
            let decimals = if a == 0.0 {
                0
            } else {
                (5 - a.log10().floor() as i32).clamp(0, 8) as u8
            };
            self.l.trimmed(x, decimals)
        } else {
            format!("{x:.4e}")
        }
    }

    /// The ROOT group: the file (version, compression, keys, trees) and the object shown
    /// (class, path, title, entries, sum of weights, and per axis its bins, range, mean and
    /// RMS).
    fn root(&self) -> GroupBuf {
        let mut g = GroupBuf::new("root", self.l.text("group-root"));
        let Some(r) = self.input.science.as_ref().and_then(|s| s.root.as_ref()) else {
            return g;
        };
        g.add_opt(
            "root_version",
            self.l.text("label-root-version"),
            non_empty(Some(r.version.clone())),
        );
        g.add_opt(
            "root_compression",
            self.l.text("label-root-compression"),
            non_empty(Some(r.compression.clone())),
        );
        g.add(
            "root_keys",
            self.l.text("label-root-keys"),
            self.l.int(r.keys),
            None,
        );
        g.add_opt(
            "root_class",
            self.l.text("label-root-class"),
            non_empty(Some(r.class.clone())),
        );
        g.add_opt(
            "root_name",
            self.l.text("label-root-name"),
            non_empty(Some(r.path.clone())),
        );
        g.add_opt(
            "root_title",
            self.l.text("label-root-title"),
            non_empty(Some(r.title.clone())),
        );
        g.add_opt(
            "root_entries",
            self.l.text("label-root-entries"),
            r.entries.map(|e| self.general(e)),
        );
        g.add_opt(
            "root_sum_weights",
            self.l.text("label-root-sum-weights"),
            r.sum_weights.map(|e| self.general(e)),
        );
        g.add_opt(
            "root_points",
            self.l.text("label-root-points"),
            r.points.map(|n| self.l.int(n)),
        );
        let names = [
            ("x", "root_axis_x", "root_mean_rms_x"),
            ("y", "root_axis_y", "root_mean_rms_y"),
            ("z", "root_axis_z", "root_mean_rms_z"),
        ];
        for (a, (name, axis_key, mean_key)) in r.axes.iter().zip(names) {
            let quantity = match (a.title.trim(), a.unit.trim()) {
                ("", _) => name.to_owned(),
                (t, "") => t.to_owned(),
                (t, u) => format!("{t} [{u}]"),
            };
            let key = if a.variable {
                "root-axis-variable"
            } else {
                "root-axis-fixed"
            };
            g.add(
                axis_key,
                self.l.fmt("label-root-axis", &args!("axis" => name)),
                self.l.fmt(
                    key,
                    &args!(
                        "quantity" => quantity,
                        "bins" => u64::from(a.bins),
                        "min" => self.general(a.min),
                        "max" => self.general(a.max)
                    ),
                ),
                None,
            );
            if let Some((mean, rms)) = a.mean_rms {
                g.add(
                    mean_key,
                    self.l.fmt("label-root-mean-rms", &args!("axis" => name)),
                    self.l.fmt(
                        "root-mean-rms",
                        // A signed mean takes a true minus, as other signed values.
                        &args!(
                            "mean" => self.general(mean).replacen('-', "\u{2212}", 1),
                            "rms" => self.general(rms)
                        ),
                    ),
                    None,
                );
            }
        }
        if !r.trees.is_empty() {
            let list: Vec<String> = r
                .trees
                .iter()
                .take(32)
                .map(|(p, n)| match u64::try_from(*n) {
                    Ok(n) => self
                        .l
                        .fmt("root-tree", &args!("name" => p.clone(), "entries" => n)),
                    Err(_) => p.clone(),
                })
                .collect();
            let mut v = list.join(", ");
            if r.trees.len() > 32 {
                v.push_str(", …");
            }
            g.add("root_trees", self.l.text("label-root-trees"), v, None);
        }
        g
    }

    /// The CASA image group: what the image is, its beam or per-plane beams, the spectral
    /// frame and velocity definition, the default mask and the storage.
    fn casa(&self) -> GroupBuf {
        let mut g = GroupBuf::new("casa", self.l.text("group-casa"));
        let Some(c) = self.input.science.as_ref().and_then(|s| s.casa.as_ref()) else {
            return g;
        };
        let kind = match c.image_type.as_str() {
            t @ ("image" | "residual" | "model" | "pb" | "mask" | "psf" | "sumwt" | "weight") => {
                Some(self.l.text(&format!("casa-type-{t}")))
            }
            _ => None,
        };
        let kind = match (kind, non_empty(c.info_type.clone())) {
            (Some(k), Some(i)) => Some(format!("{k} ({i})")),
            (k, i) => k.or(i),
        };
        g.add_opt("casa_type", self.l.text("label-casa-type"), kind);
        if let Some([major, minor, pa]) = c.beam.filter(|b| b.iter().all(|x| x.is_finite())) {
            let v = self.l.fmt(
                "casa-beam",
                &args!(
                    "major" => self.l.trimmed(major, 3),
                    "minor" => self.l.trimmed(minor, 3),
                    "pa" => self.l.trimmed(pa, 1)
                ),
            );
            g.add("casa_beam", self.l.text("label-casa-beam"), v, None);
        }
        if let Some((n, (lo, hi))) = c
            .plane_beams
            .filter(|(_, (lo, hi))| lo.is_finite() && hi.is_finite())
        {
            let v = self.l.fmt(
                "casa-plane-beams",
                &args!(
                    "count" => n as u64,
                    "min" => self.l.trimmed(lo, 3),
                    "max" => self.l.trimmed(hi, 3)
                ),
            );
            g.add(
                "casa_plane_beams",
                self.l.text("label-casa-plane-beams"),
                v,
                None,
            );
        }
        if let Some(f) = c.rest_frequency.filter(|f| f.is_finite() && *f > 0.0) {
            let v = self
                .l
                .fmt("casa-ghz", &args!("ghz" => self.l.trimmed(f / 1e9, 9)));
            g.add(
                "casa_rest",
                self.l.text("label-casa-rest-frequency"),
                v,
                None,
            );
        }
        g.add_opt(
            "casa_frame",
            self.l.text("label-casa-spectral-frame"),
            non_empty(c.spectral_frame.clone()),
        );
        if let Some(v) = c
            .velocity
            .as_deref()
            .filter(|v| matches!(*v, "radio" | "optical" | "relativistic"))
        {
            g.add(
                "casa_velocity",
                self.l.text("label-casa-velocity"),
                self.l.text(&format!("casa-velocity-{v}")),
                None,
            );
        }
        g.add_opt(
            "casa_mask",
            self.l.text("label-casa-mask"),
            non_empty(c.mask.clone()),
        );
        if !c.storage.is_empty() {
            let key = if c.big_endian {
                "casa-storage-big"
            } else {
                "casa-storage-little"
            };
            let v = self.l.fmt(key, &args!("manager" => c.storage.clone()));
            g.add("casa_storage", self.l.text("label-casa-storage"), v, None);
        }
        g
    }

    /// The unstructured mesh group: its name and convention, how many nodes, faces and
    /// edges, the face shapes, the coordinates and where the values sit.
    fn ugrid(&self) -> GroupBuf {
        let mut g = GroupBuf::new("ugrid", self.l.text("group-ugrid"));
        let Some(m) = self.input.science.as_ref().and_then(|s| s.ugrid.as_ref()) else {
            return g;
        };
        g.add_opt(
            "ugrid_name",
            self.l.text("label-ugrid-name"),
            non_empty(Some(m.name.clone())),
        );
        g.add_opt(
            "ugrid_convention",
            self.l.text("label-ugrid-convention"),
            non_empty(Some(m.convention.clone())),
        );
        g.add(
            "ugrid_nodes",
            self.l.text("label-ugrid-nodes"),
            self.l.int(m.nodes),
            None,
        );
        g.add(
            "ugrid_faces",
            self.l.text("label-ugrid-faces"),
            self.l.int(m.faces),
            None,
        );
        if m.edges > 0 {
            g.add(
                "ugrid_edges",
                self.l.text("label-ugrid-edges"),
                self.l.int(m.edges),
                None,
            );
        }
        let mut shapes = Vec::new();
        for (key, n) in [("ugrid-triangles", m.triangles), ("ugrid-quads", m.quads)] {
            if n > 0 {
                shapes.push(self.l.fmt(key, &args!("count" => n)));
            }
        }
        if m.polygons > 0 {
            shapes.push(self.l.fmt(
                "ugrid-polygons",
                &args!(
                    "count" => m.polygons,
                    "max" => u64::from(m.max_face_nodes)
                ),
            ));
        }
        g.add(
            "ugrid_shapes",
            self.l.text("label-ugrid-shapes"),
            shapes.join(", "),
            None,
        );
        let coords = match (m.geographic, m.unit.as_deref().map(str::trim)) {
            (true, _) => self.l.text("ugrid-lonlat"),
            (false, Some(u)) if !u.is_empty() => self
                .l
                .fmt("ugrid-projected-unit", &args!("unit" => u.to_owned())),
            (false, _) => self.l.text("ugrid-projected"),
        };
        g.add(
            "ugrid_coordinates",
            self.l.text("label-ugrid-coordinates"),
            coords,
            None,
        );
        let on = match m.on_faces {
            Some(true) => self.l.text("ugrid-on-faces"),
            Some(false) => self.l.text("ugrid-on-nodes"),
            None => self.l.text("ugrid-grid-only"),
        };
        g.add("ugrid_values", self.l.text("label-ugrid-values"), on, None);
        if m.split_faces > 0 {
            g.add(
                "ugrid_split",
                self.l.text("label-ugrid-split"),
                self.l.fmt("ugrid-split", &args!("count" => m.split_faces)),
                None,
            );
        }
        g
    }

    // ----- Annotations ------------------------------------------------------

    fn annotation_kind(&self, id: &str) -> String {
        let key = match id {
            "polygon" => "annotation-polygon",
            "polyline" => "annotation-polyline",
            "rectangle" => "annotation-rectangle",
            "ellipse" => "annotation-ellipse",
            "point" => "annotation-point",
            "points" => "annotation-points",
            "ruler" => "annotation-ruler",
            "arrow" => "annotation-arrow",
            other => return other.to_owned(),
        };
        self.l.text(key)
    }

    fn annotations(&self) -> GroupBuf {
        let mut g = GroupBuf::new("annotations", self.l.text("group-annotations"));
        let Some(a) = self
            .input
            .science
            .as_ref()
            .and_then(|s| s.annotations.as_ref())
        else {
            return g;
        };
        g.add(
            "annotation_file",
            self.l.text("label-annotation-file"),
            self.l.fmt(
                "annotation-source",
                &args!("file" => a.file.clone(), "format" => a.format.clone()),
            ),
            None,
        );
        // Counts by kind and by group, in order of first appearance.
        let tally = |key: fn(&AnnotationItem) -> &str| {
            let mut out: Vec<(&str, u64)> = Vec::new();
            for it in &a.items {
                let k = key(it);
                match out.iter_mut().find(|(n, _)| *n == k) {
                    Some((_, c)) => *c += 1,
                    None => out.push((k, 1)),
                }
            }
            out
        };
        let kinds = tally(|it| it.shape.as_str())
            .into_iter()
            .map(|(k, n)| format!("{} ({})", self.annotation_kind(k), self.l.int(n)))
            .collect::<Vec<_>>()
            .join(", ");
        g.add(
            "annotation_shapes",
            self.l.text("label-annotation-shapes"),
            kinds,
            None,
        );
        let groups = tally(|it| it.group.trim())
            .into_iter()
            .filter(|(k, _)| !k.is_empty())
            .map(|(k, n)| format!("{k} ({})", self.l.int(n)))
            .collect::<Vec<_>>()
            .join(", ");
        g.add(
            "annotation_groups",
            self.l.text("label-annotation-groups"),
            groups,
            None,
        );
        if a.skipped > 0 {
            g.add(
                "annotation_skipped",
                self.l.text("label-annotation-skipped"),
                self.l.int(a.skipped as u64),
                None,
            );
        }
        g
    }

    /// Every annotation as an "all fields" group: name, kind, group and size.
    fn annotation_fields(&self) -> Option<FieldGroup> {
        /// Most annotations listed.
        const MAX: usize = 1000;
        let a = self.input.science.as_ref()?.annotations.as_ref()?;
        if a.items.is_empty() {
            return None;
        }
        let mut fields: Vec<Field> = a
            .items
            .iter()
            .take(MAX)
            .enumerate()
            .map(|(i, it)| {
                let tag = non_empty(Some(it.name.trim().to_owned()))
                    .unwrap_or_else(|| format!("#{}", i + 1));
                let mut parts = vec![self.annotation_kind(&it.shape)];
                parts.extend(non_empty(Some(it.group.trim().to_owned())));
                parts.extend(it.measure.clone());
                let display = self.l.dot_join(&parts);
                Field {
                    tag,
                    raw: it.shape.clone(),
                    empty: false,
                    display,
                    full: None,
                }
            })
            .collect();
        if a.items.len() > MAX {
            let more = self.l.fmt(
                "annotation-more",
                &args!("count" => (a.items.len() - MAX) as u64),
            );
            fields.push(Field {
                tag: "…".to_owned(),
                raw: String::new(),
                empty: false,
                display: more,
                full: None,
            });
        }
        Some(FieldGroup {
            title: self.l.fmt(
                "fields-annotations-header",
                &args!("file" => a.file.clone()),
            ),
            fields,
        })
    }

    // ----- Drawing ----------------------------------------------------------

    fn drawing(&self) -> GroupBuf {
        let title = match self.input.drawing.as_ref().and_then(|d| d.pcb.as_ref()) {
            Some(p) if p.schematic => self.l.text("group-schematic"),
            Some(_) => self.l.text("group-board"),
            None => self.l.text("group-drawing"),
        };
        let mut g = GroupBuf::new("drawing", title);
        let Some(d) = &self.input.drawing else {
            return g;
        };
        if let Some(p) = &d.pcb {
            self.pcb(&mut g, d, p);
            return g;
        }
        if let Some(s) = &d.survey {
            self.survey(&mut g, d, s);
            return g;
        }
        g.add(
            "version",
            self.l.text("label-drawing-version"),
            d.version.clone(),
            None,
        );
        g.add(
            "units",
            self.l.text("label-drawing-units"),
            self.l
                .fmt("drawing-units", &args!("code" => i64::from(d.units))),
            None,
        );
        if let Some((w, h)) = d.extents.filter(|(w, h)| w.is_finite() && h.is_finite()) {
            let digits = |x: f64| -> u8 {
                // Four significant digits, at most 3 decimals.
                (3 - (x.abs().max(1e-9).log10().floor() as i32)).clamp(0, 3) as u8
            };
            let (width, height) = (self.l.trimmed(w, digits(w)), self.l.trimmed(h, digits(h)));
            let v = match &d.unit_symbol {
                Some(unit) => self.l.fmt(
                    "drawing-extents",
                    &args!("width" => width, "height" => height, "unit" => self.l.unit(unit)),
                ),
                None => self.l.fmt(
                    "drawing-extents-unitless",
                    &args!("width" => width, "height" => height),
                ),
            };
            g.add("extents", self.l.text("label-drawing-extents"), v, None);
        }
        if let Some(p) = &d.plot {
            self.plot(&mut g, d, p);
            return g;
        }
        if let Some(ic) = &d.ic {
            self.ic_layout(&mut g, d, ic);
            return g;
        }
        let sheet = match &d.layout {
            Some(name) => name.clone(),
            None => self.l.text("drawing-model-space"),
        };
        g.add("sheet", self.l.text("label-drawing-sheet"), sheet, None);
        if d.layouts > 0 {
            g.add(
                "layouts",
                self.l.text("label-drawing-layouts"),
                self.l.int(u64::from(d.layouts)),
                None,
            );
        }
        let layers = if d.layers_off > 0 {
            self.l.fmt(
                "drawing-layers-off",
                &args!(
                    "num" => u64::from(d.layers),
                    "off" => u64::from(d.layers_off),
                    "n" => i64::from(d.layers_off)
                ),
            )
        } else {
            self.l.int(u64::from(d.layers))
        };
        g.add("layers", self.l.text("label-drawing-layers"), layers, None);
        let total: u64 = d.counts.iter().map(|(_, n)| n).sum();
        g.add(
            "entities",
            self.l.text("label-drawing-entities"),
            self.l.int(total),
            None,
        );
        g.add_opt(
            "codepage",
            self.l.text("label-drawing-codepage"),
            non_empty(d.codepage.clone()),
        );
        self.font_facts(&mut g, d);
        if !d.warnings.is_empty() {
            g.add(
                "warnings",
                self.l.text("label-drawing-warnings"),
                d.warnings.join("; "),
                None,
            );
        }
        g
    }

    /// The board or schematic group: what made it, its size and layers, what is on it.
    /// Survey data: coordinate system, surfaces and alignments.
    fn survey(&self, g: &mut GroupBuf, d: &DrawingFacts, s: &SurveyFacts) {
        g.add(
            "version",
            self.l.text("label-drawing-version"),
            d.version.clone(),
            None,
        );
        g.add_opt(
            "crs",
            self.l.text("label-survey-crs"),
            s.crs
                .clone()
                .or_else(|| Some(self.l.text("survey-crs-none"))),
        );
        g.add_opt(
            "project",
            self.l.text("label-survey-project"),
            s.project.clone(),
        );
        g.add_opt(
            "application",
            self.l.text("label-survey-application"),
            s.application.clone(),
        );
        let num = |x: f64, dec: u8| self.l.trimmed(x, dec);
        for (i, f) in s.surfaces.iter().enumerate() {
            let v = self.l.fmt(
                "survey-surface",
                &args!(
                    "name" => f.name.clone(),
                    "points" => f.points,
                    "triangles" => f.triangles,
                    "area" => num(f.area_2d, 1),
                    "area3d" => num(f.area_3d, 1),
                    "volume" => num(f.volume, 1),
                    "zmin" => num(f.z_min, 3),
                    "zmax" => num(f.z_max, 3),
                    "interval" => num(f.contour_interval, 3),
                    "unit" => s.unit.clone()
                ),
            );
            g.add(
                if i == 0 { "surface" } else { "surface_more" },
                self.l.text("label-survey-surface"),
                v,
                None,
            );
        }
        for (i, a) in s.alignments.iter().enumerate() {
            let v = self.l.fmt(
                "survey-alignment",
                &args!(
                    "name" => a.name.clone(),
                    "length" => num(a.length, 3),
                    "unit" => s.unit.clone(),
                    "start" => a.sta_start.clone(),
                    "end" => a.sta_end.clone()
                ),
            );
            g.add(
                if i == 0 {
                    "alignment"
                } else {
                    "alignment_more"
                },
                self.l.text("label-survey-alignment"),
                v,
                None,
            );
        }
        if !d.warnings.is_empty() {
            g.add(
                "warnings",
                self.l.text("label-drawing-warnings"),
                d.warnings.join("; "),
                None,
            );
        }
    }

    /// SHX fonts used, and fonts not found with their styles and what draws instead.
    fn font_facts(&self, g: &mut GroupBuf, d: &DrawingFacts) {
        if !d.shx_fonts.is_empty() {
            g.add(
                "shx_fonts",
                self.l.text("label-drawing-shx-fonts"),
                d.shx_fonts.join(", "),
                None,
            );
        }
        if d.missing_fonts.is_empty() {
            return;
        }
        let v: Vec<String> = d
            .missing_fonts
            .iter()
            .map(|m| {
                let styles = m.styles.join(", ");
                if m.substitute.is_empty() {
                    self.l.fmt(
                        "drawing-missing-bigfont",
                        &args!("file" => m.file.clone(), "styles" => styles),
                    )
                } else {
                    self.l.fmt(
                        "drawing-missing-font",
                        &args!(
                            "file" => m.file.clone(),
                            "styles" => styles,
                            "substitute" => m.substitute.clone()
                        ),
                    )
                }
            })
            .collect();
        g.add(
            "missing_fonts",
            self.l.text("label-drawing-missing-fonts"),
            v.join("; "),
            None,
        );
    }

    fn pcb(&self, g: &mut GroupBuf, d: &DrawingFacts, p: &PcbFacts) {
        g.add(
            "version",
            self.l.text("label-drawing-version"),
            d.version.clone(),
            None,
        );
        g.add_opt("view", self.l.text("label-pcb-view"), p.view.clone());
        g.add_opt(
            "project",
            self.l.text("label-pcb-project"),
            p.project.clone(),
        );
        g.add_opt(
            "revision",
            self.l.text("label-pcb-revision"),
            p.revision.clone(),
        );
        g.add_opt("pcb_date", self.l.text("label-pcb-date"), p.date.clone());
        g.add_opt(
            "generator",
            self.l.text("label-pcb-generator"),
            p.generator.clone(),
        );
        if let Some((w, h)) = p.size_mm.filter(|(w, h)| w.is_finite() && h.is_finite()) {
            let v = self.l.fmt(
                "drawing-extents",
                &args!(
                    "width" => self.l.trimmed(w, 2),
                    "height" => self.l.trimmed(h, 2),
                    "unit" => self.l.unit("mm")
                ),
            );
            g.add("board_size", self.l.text("label-pcb-size"), v, None);
        }
        if let Some(t) = p.thickness_mm.filter(|t| t.is_finite() && *t > 0.0) {
            g.add(
                "thickness",
                self.l.text("label-pcb-thickness"),
                format!("{} {}", self.l.trimmed(t, 2), self.l.unit("mm")),
                None,
            );
        }
        let count = |g: &mut GroupBuf, key: &'static str, label: &str, n: u64| {
            if n > 0 {
                g.add(key, self.l.text(label), self.l.int(n), None);
            }
        };
        count(
            g,
            "copper",
            "label-pcb-copper-layers",
            u64::from(p.copper_layers),
        );
        count(g, "sheets", "label-pcb-sheets", u64::from(p.sheets));
        if !p.layers.is_empty() {
            const SHOWN: usize = 12;
            let parts: Vec<String> = p
                .layers
                .iter()
                .take(SHOWN)
                .map(|(name, role)| {
                    let role = self.pcb_role(role);
                    if role.is_empty() || role == *name {
                        name.clone()
                    } else {
                        self.l.fmt(
                            "pcb-layer-role",
                            &args!("name" => name.clone(), "role" => role),
                        )
                    }
                })
                .collect();
            let mut v = parts.join("; ");
            if p.layers.len() > SHOWN {
                v = self.l.fmt(
                    "layout-top-cells-more",
                    &args!(
                        "names" => v,
                        "more" => (p.layers.len() - SHOWN) as u64
                    ),
                );
            }
            g.add("pcb_layers", self.l.text("label-pcb-layers"), v, None);
        }
        let label = if p.schematic {
            "label-pcb-symbols"
        } else {
            "label-pcb-components"
        };
        count(g, "components", label, p.components);
        count(g, "nets", "label-pcb-nets", p.nets);
        let tracks = if p.schematic {
            "label-pcb-wires"
        } else {
            "label-pcb-tracks"
        };
        count(g, "tracks", tracks, p.tracks);
        count(g, "vias", "label-pcb-vias", p.vias);
        if p.holes > 0 {
            g.add(
                "holes",
                self.l.text("label-pcb-holes"),
                self.l.fmt(
                    "pcb-holes",
                    &args!(
                        "holes" => p.holes,
                        "sizes" => u64::from(p.drill_sizes)
                    ),
                ),
                None,
            );
        }
        count(
            g,
            "apertures",
            "label-pcb-apertures",
            u64::from(p.apertures),
        );
        count(g, "macros", "label-pcb-macros", u64::from(p.macros));
        self.font_facts(g, d);
        if !d.warnings.is_empty() {
            g.add(
                "warnings",
                self.l.text("label-drawing-warnings"),
                d.warnings.join("; "),
                None,
            );
        }
    }

    /// A layer role given in English ("Top copper"), in the reader's language when the
    /// catalogue has it (`pcb-role-top-copper`), else as given.
    fn pcb_role(&self, english: &str) -> String {
        if let Some(n) = english
            .strip_prefix("Inner copper ")
            .and_then(|n| n.parse::<i64>().ok())
        {
            return self.l.fmt("pcb-role-inner-copper", &args!("n" => n));
        }
        let slug: String = english
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let id = format!("pcb-role-{}", slug.trim_matches('-'));
        let t = self.l.text(&id);
        if t == id || t.is_empty() {
            english.to_owned()
        } else {
            t
        }
    }

    // ----- Spectrum ---------------------------------------------------------

    fn spectrum(&self) -> GroupBuf {
        let mut g = GroupBuf::new("spectrum", self.l.text("group-spectrum"));
        let Some(s) = &self.input.spectrum else {
            return g;
        };
        let version = if s.version.is_empty() {
            s.format.clone()
        } else {
            format!("{} {}", s.format, s.version)
        };
        g.add(
            "format",
            self.l.text("label-spectrum-format"),
            version,
            None,
        );
        g.add(
            "technique",
            self.l.text("label-spectrum-technique"),
            self.spectrum_technique(&s.technique),
            None,
        );
        g.add_opt(
            "title",
            self.l.text("label-spectrum-title"),
            s.title.clone(),
        );
        if s.pages > 1 {
            g.add(
                "page",
                self.l.text("label-spectrum-page"),
                self.l.fmt(
                    "spectrum-page-of",
                    &args!(
                        "page" => u64::from(s.page),
                        "pages" => u64::from(s.pages),
                        "name" => s.page_name.clone()
                    ),
                ),
                None,
            );
        }
        let axis = |(t, u): &(String, String)| {
            if u.is_empty() {
                t.clone()
            } else {
                self.l.fmt(
                    "spectrum-axis",
                    &args!("title" => t.clone(), "unit" => u.clone()),
                )
            }
        };
        if !s.x_axis.0.is_empty() {
            g.add(
                "x_axis",
                self.l.text("label-spectrum-x"),
                axis(&s.x_axis),
                None,
            );
        }
        if !s.y_axis.0.is_empty() {
            g.add(
                "y_axis",
                self.l.text("label-spectrum-y"),
                axis(&s.y_axis),
                None,
            );
        }
        g.add_opt(
            "nucleus",
            self.l.text("label-spectrum-nucleus"),
            s.nucleus.clone(),
        );
        if let Some(f) = s.frequency_mhz.filter(|f| f.is_finite() && *f > 0.0) {
            g.add(
                "frequency",
                self.l.text("label-spectrum-frequency"),
                format!("{} MHz", self.l.trimmed(f, 4)),
                None,
            );
        }
        g.add_opt(
            "solvent",
            self.l.text("label-spectrum-solvent"),
            s.solvent.clone(),
        );
        g.add_opt(
            "pulse_program",
            self.l.text("label-spectrum-pulse-program"),
            s.pulse_program.clone(),
        );
        if s.spectra > 1 {
            g.add(
                "spectra",
                self.l.text("label-spectrum-spectra"),
                self.l.int(s.spectra),
                None,
            );
        }
        if s.points > 0 {
            g.add(
                "points",
                self.l.text("label-spectrum-points"),
                self.l.int(s.points),
                None,
            );
        }
        g.add_opt(
            "instrument",
            self.l.text("label-spectrum-instrument"),
            s.instrument.clone(),
        );
        g.add_opt(
            "origin",
            self.l.text("label-spectrum-origin"),
            s.origin.clone(),
        );
        g.add_opt(
            "spectrum_date",
            self.l.text("label-spectrum-date"),
            s.date.clone(),
        );
        if !s.notes.is_empty() {
            g.add(
                "processing",
                self.l.text("label-spectrum-processing"),
                self.l.join(&s.notes, "; "),
                None,
            );
        }
        if !s.warnings.is_empty() {
            g.add(
                "warnings",
                self.l.text("label-drawing-warnings"),
                s.warnings.join("; "),
                None,
            );
        }
        g
    }

    /// A technique given in English, in the reader's language.
    fn spectrum_technique(&self, english: &str) -> String {
        let id = match english {
            "Infrared" => "spectrum-technique-ir",
            "Raman" => "spectrum-technique-raman",
            "UV-Vis" => "spectrum-technique-uv-vis",
            "Fluorescence" => "spectrum-technique-fluorescence",
            "NMR" => "spectrum-technique-nmr",
            "Mass spectrum" => "spectrum-technique-ms",
            "Chromatogram" => "spectrum-technique-chromatogram",
            _ => "spectrum-technique-other",
        };
        self.l.text(id)
    }

    /// The rest of the drawing group for a plot: paper, pages, pens, problems.
    fn plot(&self, g: &mut GroupBuf, d: &DrawingFacts, p: &PlotFacts) {
        if let Some((w, h)) = p.paper_mm.filter(|(w, h)| w.is_finite() && h.is_finite()) {
            let v = self.l.fmt(
                "drawing-extents",
                &args!(
                    "width" => self.l.trimmed(w, 1),
                    "height" => self.l.trimmed(h, 1),
                    "unit" => self.l.unit("mm")
                ),
            );
            g.add("paper", self.l.text("label-plot-paper"), v, None);
        }
        if p.pages > 1 {
            g.add(
                "sheet",
                self.l.text("label-plot-page"),
                self.l.fmt(
                    "plot-page-of",
                    &args!(
                        "page" => u64::from(p.page),
                        "pages" => u64::from(p.pages)
                    ),
                ),
                None,
            );
        }
        if !p.pens.is_empty() {
            let pens: Vec<String> = p.pens.iter().map(|&n| self.l.int(u64::from(n))).collect();
            g.add(
                "pens",
                self.l.text("label-plot-pens"),
                strings::join(self.l, &pens),
                None,
            );
        }
        g.add_opt(
            "job",
            self.l.text("label-plot-job"),
            non_empty(p.job.clone()),
        );
        let total: u64 = d.counts.iter().map(|(_, n)| n).sum();
        g.add(
            "entities",
            self.l.text("label-plot-items"),
            self.l.int(total),
            None,
        );
        if !d.warnings.is_empty() {
            g.add(
                "warnings",
                self.l.text("label-drawing-warnings"),
                d.warnings.join("; "),
                None,
            );
        }
    }

    /// A length in metres with the SI prefix that keeps it at least 1: "1 nm", "0.5 µm"
    /// becomes "500 nm".
    fn si_length(&self, m: f64) -> String {
        const UNITS: [(f64, &str); 5] = [
            (1.0, "m"),
            (1e-3, "mm"),
            (1e-6, "µm"),
            (1e-9, "nm"),
            (1e-12, "pm"),
        ];
        if !(m.is_finite() && m > 0.0) {
            return "?".into();
        }
        let (f, unit) = UNITS
            .iter()
            .copied()
            .find(|&(f, _)| m >= f * (1.0 - 1e-9))
            .unwrap_or(UNITS[4]);
        format!("{} {}", self.l.trimmed(m / f, 4), self.l.unit(unit))
    }

    /// The rest of the drawing group for an IC layout: library, units, cells, layers,
    /// shapes and references, bounds, problems.
    fn ic_layout(&self, g: &mut GroupBuf, d: &DrawingFacts, ic: &IcLayoutFacts) {
        g.add_opt(
            "library",
            self.l.text("label-layout-library"),
            non_empty(Some(ic.library.clone())),
        );
        g.add(
            "layout_units",
            self.l.text("label-layout-units"),
            self.l.fmt(
                "layout-units",
                &args!(
                    "db" => self.si_length(ic.db_unit_m),
                    "user" => self.si_length(ic.user_unit_m)
                ),
            ),
            None,
        );
        g.add(
            "cell",
            self.l.text("label-layout-cell"),
            ic.cell.clone(),
            None,
        );
        g.add(
            "cells",
            self.l.text("label-layout-cells"),
            self.l.int(ic.cells),
            None,
        );
        if !ic.top_cells.is_empty() {
            const SHOWN: usize = 8;
            let names = strings::join(self.l, &ic.top_cells[..ic.top_cells.len().min(SHOWN)]);
            let v = if ic.top_cells.len() > SHOWN {
                self.l.fmt(
                    "layout-top-cells-more",
                    &args!(
                        "names" => names,
                        "more" => (ic.top_cells.len() - SHOWN) as u64
                    ),
                )
            } else {
                names
            };
            g.add("top_cells", self.l.text("label-layout-top-cells"), v, None);
        }
        g.add(
            "layers",
            self.l.text("label-layout-layers"),
            self.l.int(u64::from(ic.layers)),
            None,
        );
        g.add(
            "shapes",
            self.l.text("label-layout-shapes"),
            self.l.int(ic.shapes),
            None,
        );
        if ic.texts > 0 {
            g.add(
                "texts",
                self.l.text("label-layout-texts"),
                self.l.int(ic.texts),
                None,
            );
        }
        if ic.references > 0 {
            g.add(
                "references",
                self.l.text("label-layout-references"),
                self.l.fmt(
                    "layout-references",
                    &args!(
                        "refs" => ic.references,
                        "placements" => ic.placements
                    ),
                ),
                None,
            );
        }
        g.add(
            "flat_shapes",
            self.l.text("label-layout-flat-shapes"),
            self.l.int(ic.flat_shapes),
            None,
        );
        if let Some([x0, y0, x1, y1]) = ic.bounds_um.filter(|b| b.iter().all(|v| v.is_finite())) {
            let c = |v: f64| self.l.trimmed(v, 3);
            g.add(
                "bounds",
                self.l.text("label-layout-bounds"),
                self.l.fmt(
                    "layout-bounds",
                    &args!("x0" => c(x0), "y0" => c(y0), "x1" => c(x1), "y1" => c(y1)),
                ),
                None,
            );
        }
        if !d.warnings.is_empty() {
            g.add(
                "warnings",
                self.l.text("label-drawing-warnings"),
                d.warnings.join("; "),
                None,
            );
        }
    }

    // ----- Geospatial ---------------------------------------------------------

    /// A coordinate in map units: whole metres (or feet) for projected CRSs, six decimals of a
    /// degree for geographic ones.
    fn geo_coord(&self, v: f64, geographic: bool) -> String {
        if geographic {
            self.l.trimmed(v, 6)
        } else {
            self.l.trimmed(v, if v.abs() >= 1000.0 { 1 } else { 3 })
        }
    }

    fn geo(&self) -> GroupBuf {
        let mut g = GroupBuf::new("geospatial", self.l.text("group-geospatial"));
        let Some(f) = &self.input.geo else {
            return g;
        };
        let crs = match (&f.crs, f.epsg) {
            (Some(n), Some(c)) => Some(self.l.fmt(
                "geo-crs-epsg",
                &args!("name" => n.clone(), "code" => c.to_string()),
            )),
            (Some(n), None) => Some(n.clone()),
            (None, Some(c)) => Some(format!("EPSG:{c}")),
            (None, None) => None,
        };
        match crs {
            Some(c) => g.add("crs", self.l.text("label-geo-crs"), c, f.crs_note.clone()),
            None => g.add(
                "crs",
                self.l.text("label-geo-crs"),
                self.l.text("geo-crs-unknown"),
                f.crs_note.clone(),
            ),
        }
        g.add_opt("datum", self.l.text("label-geo-datum"), f.datum.clone());
        g.add_opt(
            "projection",
            self.l.text("label-geo-projection"),
            f.projection.clone().map(|m| {
                if f.projection_unsupported {
                    self.l
                        .fmt("geo-projection-unsupported", &args!("method" => m))
                } else {
                    m
                }
            }),
        );
        if let Some(k) = f.to_wgs84 {
            let v = match k {
                "same" => self.l.text("geo-to-wgs84-same"),
                "helmert" => self.l.text("geo-to-wgs84-helmert"),
                "by-area" => self.l.text("geo-to-wgs84-by-area"),
                _ => self.l.text("geo-to-wgs84-unknown"),
            };
            g.add("to_wgs84", self.l.text("label-geo-to-wgs84"), v, None);
        }
        g.add_opt("unit", self.l.text("label-geo-unit"), f.unit.clone());
        g.add_opt(
            "attribution",
            self.l.text("label-geo-attribution"),
            f.attribution.clone(),
        );
        let unit = f.unit.as_deref().map(|u| match u {
            "metre" | "meter" | "m" => self.l.unit("m"),
            u => self.l.unit(u),
        });
        if let Some(b) = f.bounds.filter(|b| b.iter().all(|v| v.is_finite())) {
            let c = |v: f64| self.geo_coord(v, f.geographic);
            let v = self.l.fmt(
                "geo-bounds",
                &args!("x0" => c(b[0]), "y0" => c(b[1]), "x1" => c(b[2]), "y1" => c(b[3])),
            );
            let v = match (&unit, f.geographic) {
                (Some(u), false) => format!("{v} {u}"),
                _ => v,
            };
            g.add("bounds", self.l.text("label-geo-bounds"), v, None);
        }
        if let Some(b) = f.lonlat.filter(|b| b.iter().all(|v| v.is_finite())) {
            let c = |v: f64| self.l.trimmed(v, 5);
            let v = self.l.fmt(
                "geo-lonlat-bounds",
                &args!("west" => c(b[0]), "south" => c(b[1]), "east" => c(b[2]), "north" => c(b[3])),
            );
            g.add(
                "lonlat_bounds",
                self.l.text("label-geo-lonlat-bounds"),
                v,
                None,
            );
        }
        if let Some((x, y)) = f.pixel_size {
            let d = |v: f64| self.l.trimmed(v, if f.geographic { 9 } else { 4 });
            let v = self
                .l
                .fmt("geo-pixel-size", &args!("x" => d(x), "y" => d(y)));
            let v = match (&unit, f.geographic) {
                (Some(u), false) => format!("{v} {u}"),
                (_, true) => format!("{v}°"),
                _ => v,
            };
            g.add("pixel_size", self.l.text("label-geo-pixel-size"), v, None);
        }
        if f.levels.len() > 1 {
            let sizes = f
                .levels
                .iter()
                .map(|(w, h)| {
                    format!(
                        "{} × {}",
                        self.l.int(u64::from(*w)),
                        self.l.int(u64::from(*h))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            g.add(
                "overviews",
                self.l.text("label-geo-overviews"),
                self.l.fmt(
                    "geo-overviews",
                    &args!("n" => (f.levels.len() - 1) as i64, "sizes" => sizes),
                ),
                None,
            );
        }
        if let Some(n) = f.nodata {
            g.add(
                "nodata",
                self.l.text("label-geo-nodata"),
                self.l.trimmed(n, 6),
                None,
            );
        }
        if f.pixel_is_point {
            g.add(
                "raster_type",
                self.l.text("label-geo-raster-type"),
                self.l.text("geo-pixel-is-point"),
                None,
            );
        }
        g.add_opt(
            "vertical",
            self.l.text("label-geo-vertical"),
            f.vertical.clone(),
        );
        g.add_opt(
            "citation",
            self.l.text("label-geo-citation"),
            f.citation.clone(),
        );
        if !f.sidecars.is_empty() {
            g.add(
                "sidecars",
                self.l.text("label-geo-sidecars"),
                f.sidecars.join(", "),
                None,
            );
        }
        if let Some(n) = f.features {
            g.add(
                "features",
                self.l.text("label-geo-features"),
                self.l.int(n),
                None,
            );
        }
        if let Some(n) = f.vertices {
            g.add(
                "vertices",
                self.l.text("label-geo-vertices"),
                self.l.int(n),
                None,
            );
        }
        // Each layer on its own row (the first 20): its features by geometry type.
        for l in f.layers.iter().take(20) {
            let types = l
                .types
                .iter()
                .map(|(t, n)| format!("{} {t}", self.l.int(*n)))
                .collect::<Vec<_>>()
                .join(", ");
            let v = self.l.fmt(
                "geo-layer",
                &args!(
                    "features" => l.features,
                    "fields" => l.fields
                ),
            );
            let v = if types.is_empty() {
                v
            } else {
                format!("{v} ({types})")
            };
            g.items.push(SummaryItem {
                key: "layer",
                label: if l.name.trim().is_empty() {
                    self.l.text("label-geo-layer")
                } else {
                    l.name.clone()
                },
                value: v,
                detail: None,
            });
        }
        if f.layers.len() > 20 {
            g.add(
                "layers_more",
                self.l.text("label-geo-layers"),
                self.l.int(f.layers.len() as u64),
                None,
            );
        }
        if let Some((table, zoom, n)) = &f.tiles {
            g.add(
                "tiles",
                self.l.text("label-geo-tiles"),
                self.l.fmt(
                    "geo-tiles",
                    &args!(
                        "table" => table.clone(),
                        "zoom" => zoom.to_string(),
                        "tiles" => *n
                    ),
                ),
                None,
            );
        }
        g.add_opt(
            "encoding",
            self.l.text("label-geo-encoding"),
            f.encoding.clone(),
        );
        if !f.warnings.is_empty() {
            g.add(
                "warnings",
                self.l.text("label-geo-warnings"),
                f.warnings.join("; "),
                None,
            );
        }
        g
    }

    /// GeoKeys, GDAL metadata and the CRS definition as "all fields" groups.
    fn geo_fields(&self) -> Vec<FieldGroup> {
        let Some(f) = &self.input.geo else {
            return Vec::new();
        };
        f.fields
            .iter()
            .filter(|(_, fields)| !fields.is_empty())
            .map(|(kind, fields)| FieldGroup {
                title: self.l.text(&format!("fields-geo-{kind}")),
                fields: fields
                    .iter()
                    .map(|(k, v)| {
                        // Long values (a WKT definition) are cut for the panel; copying gives
                        // the whole text.
                        let (display, full) = if v.chars().count() > 400 {
                            (
                                format!("{}…", v.chars().take(400).collect::<String>()),
                                Some(v.clone()),
                            )
                        } else {
                            (v.clone(), None)
                        };
                        Field {
                            tag: k.clone(),
                            raw: v.clone(),
                            empty: v.trim().is_empty(),
                            display,
                            full,
                        }
                    })
                    .collect(),
            })
            .collect()
    }

    /// The entity counts as an "all fields" group.
    fn drawing_fields(&self) -> Option<FieldGroup> {
        let d = self.input.drawing.as_ref()?;
        if d.counts.is_empty() {
            return None;
        }
        let fields = d
            .counts
            .iter()
            .map(|(k, n)| Field {
                tag: k.clone(),
                raw: n.to_string(),
                empty: false,
                display: self.l.int(*n),
                full: None,
            })
            .collect();
        Some(FieldGroup {
            title: self.l.fmt(
                "fields-drawing-entities",
                &args!("format" => d.format.clone()),
            ),
            fields,
        })
    }

    // ----- Medical ----------------------------------------------------------

    /// A DICONDE file's component and inspection.
    fn inspection(&self) -> GroupBuf {
        let mut g = GroupBuf::new("inspection", self.l.text("group-inspection"));
        let Some(n) = self.input.medical.as_ref().and_then(|m| m.nde.as_ref()) else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        let mm = |v: Option<f64>| {
            v.filter(|t| t.is_finite() && *t > 0.0).map(|t| {
                self.l
                    .fmt("medical-mm", &args!("mm" => self.l.trimmed(t, 3)))
            })
        };
        g.add_opt(
            "component_shape",
            self.l.text("label-nde-component-shape"),
            text(&n.component_shape),
        );
        g.add_opt(
            "component_manufacturer",
            self.l.text("label-nde-component-manufacturer"),
            text(&n.component_manufacturer),
        );
        g.add_opt(
            "material",
            self.l.text("label-nde-material"),
            text(&n.material_grade),
        );
        g.add_opt(
            "thickness",
            self.l.text("label-nde-thickness"),
            mm(n.material_thickness),
        );
        g.add_opt(
            "pipe_diameter",
            self.l.text("label-nde-pipe-diameter"),
            mm(n.pipe_diameter),
        );
        if let Some((x, unit)) = n.source.as_ref().filter(|s| s.0.is_finite() && s.0 > 0.0) {
            g.add(
                "source",
                self.l.text("label-nde-source"),
                format!("{} {}", self.l.trimmed(*x, 3), self.l.unit(unit)),
                None,
            );
        }
        g.add_opt(
            "environment",
            self.l.text("label-nde-environment"),
            text(&n.environment),
        );
        g.add_opt("iqi", self.l.text("label-nde-iqi"), text(&n.iqi));
        if n.indications > 0 {
            g.add(
                "indications",
                self.l.text("label-nde-indications"),
                self.l.int(n.indications as u64),
                None,
            );
        }
        // Told apart from a medical image even when the file states nothing more.
        if g.items.is_empty() {
            g.add(
                "standard",
                self.l.text("label-nde-standard"),
                "DICONDE (ASTM E2339)".to_owned(),
                None,
            );
        }
        g
    }

    /// An industrial inspection rather than a patient's image: a DICONDE file, or a CT
    /// volume in the scanned object's own frame (Xradia, Amira, SkyScan, X-Tek, VGI, raw).
    fn industrial(&self) -> bool {
        self.input.medical.as_ref().is_some_and(|m| m.nde.is_some()) || self.object_volume()
    }

    fn object_volume(&self) -> bool {
        self.input
            .volume
            .as_ref()
            .is_some_and(|v| v.orientation == "object")
    }

    fn medical(&self) -> GroupBuf {
        let nde = self.input.medical.as_ref().is_some_and(|m| m.nde.is_some());
        let title = if nde {
            "group-nde-image"
        } else if self.object_volume() {
            "group-industrial-image"
        } else {
            "group-medical"
        };
        let mut g = GroupBuf::new("medical", self.l.text(title));
        let Some(m) = &self.input.medical else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add_opt("modality", self.l.text("label-modality"), text(&m.modality));
        g.add_opt(
            "body_part",
            self.l.text("label-body-part"),
            text(&m.body_part),
        );
        g.add_opt(
            "study",
            self.l.text("label-study"),
            text(&m.study_description),
        );
        g.add_opt(
            "series",
            self.l.text("label-series"),
            text(&m.series_description),
        );
        g.add_opt(
            "document",
            self.l.text("label-document"),
            text(&m.document_title),
        );
        g.add_opt(
            "document_type",
            self.l.text("label-document-type"),
            text(&m.document_type),
        );
        if let Some(t) = m.slice_thickness.filter(|t| t.is_finite() && *t > 0.0) {
            let v = self
                .l
                .fmt("medical-mm", &args!("mm" => self.l.trimmed(t, 3)));
            g.add(
                "slice_thickness",
                self.l.text("label-slice-thickness"),
                v,
                None,
            );
        }
        let spacing = |(r, c): (f64, f64)| {
            (r.is_finite() && c.is_finite() && r > 0.0 && c > 0.0).then(|| {
                self.l.fmt(
                    "medical-spacing",
                    &args!("row" => self.l.trimmed(r, 4), "column" => self.l.trimmed(c, 4)),
                )
            })
        };
        if let Some(v) = m.pixel_spacing.and_then(spacing) {
            g.add(
                "pixel_spacing",
                self.l.text("label-pixel-spacing"),
                v,
                Some(self.l.text(if self.industrial() {
                    "detail-pixel-spacing-object"
                } else {
                    "detail-pixel-spacing"
                })),
            );
        }
        if let Some(v) = m.imager_pixel_spacing.and_then(spacing) {
            g.add(
                "imager_pixel_spacing",
                self.l.text("label-imager-pixel-spacing"),
                v,
                Some(self.l.text("detail-imager-pixel-spacing")),
            );
        }
        if let Some(n) = m.frames.filter(|&n| n > 1) {
            g.add(
                "frames",
                self.l.text("label-frames"),
                self.l.int(n.into()),
                None,
            );
        }
        if let Some(ts) = text(&m.transfer_syntax) {
            let v = if m.lossy {
                self.l.fmt("medical-lossy", &args!("name" => ts))
            } else {
                ts
            };
            g.add(
                "transfer_syntax",
                self.l.text("label-transfer-syntax"),
                v,
                None,
            );
        }
        g
    }

    // ----- Research volumes -------------------------------------------------

    fn volume(&self) -> GroupBuf {
        let mut g = GroupBuf::new("volume", self.l.text("group-volume"));
        let Some(v) = &self.input.volume else {
            return g;
        };
        let text = |v: &Option<String>| non_empty(v.clone());
        g.add_opt(
            "format",
            self.l.text("label-format"),
            non_empty(Some(v.format.clone())),
        );
        if !v.dims.is_empty() {
            let d = v
                .dims
                .iter()
                .map(|&d| self.l.int(d))
                .collect::<Vec<_>>()
                .join(" × ");
            g.add("dims", self.l.text("label-dimensions"), d, None);
        }
        if let Some(s) = v
            .voxel_size
            .filter(|s| s.iter().all(|x| x.is_finite() && *x > 0.0))
        {
            let val = self.l.fmt(
                "volume-voxel-size",
                &args!(
                    "x" => self.l.trimmed(s[0], 4),
                    "y" => self.l.trimmed(s[1], 4),
                    "z" => self.l.trimmed(s[2], 4)
                ),
            );
            g.add("voxel_size", self.l.text("label-voxel-size"), val, None);
        }
        g.add_opt(
            "data_type",
            self.l.text("label-data-type"),
            non_empty(Some(v.data_type.clone())),
        );
        if v.volumes > 1 {
            g.add(
                "volumes",
                self.l.text("label-time-points"),
                self.l.int(v.volumes),
                None,
            );
        }
        if !v.orientation.is_empty() {
            let key = match v.orientation.as_str() {
                "sform" => "volume-orient-sform",
                "qform" => "volume-orient-qform",
                "analyze" => "volume-orient-analyze",
                "space" => "volume-orient-space",
                "transform" => "volume-orient-transform",
                "spacings" => "volume-orient-spacings",
                "vox2ras" => "volume-orient-vox2ras",
                "mgh-default" => "volume-orient-mgh-default",
                "minc" => "volume-orient-minc",
                "object" => "volume-orient-object",
                "parrec" => "volume-orient-parrec",
                "ecat" => "volume-orient-ecat",
                _ => "volume-orient-none",
            };
            let detail = (!v.reliable).then(|| {
                self.l.text(if v.orientation == "object" {
                    "detail-volume-object"
                } else {
                    "detail-volume-unreliable"
                })
            });
            g.add(
                "orientation",
                self.l.text("label-orientation-source"),
                self.l.text(key),
                detail,
            );
        }
        g.add_opt(
            "description",
            self.l.text("label-description"),
            text(&v.description),
        );
        g.add_opt("intent", self.l.text("label-intent"), text(&v.intent));
        g.add_opt(
            "compression",
            self.l.text("label-compression"),
            text(&v.compression),
        );
        for (key, value) in &v.params {
            let (key, label) = match key.as_str() {
                "tr" => ("tr", "label-volume-tr"),
                "te" => ("te", "label-volume-te"),
                "ti" => ("ti", "label-volume-ti"),
                "flip" => ("flip", "label-volume-flip"),
                "fov" => ("fov", "label-volume-fov"),
                "field" => ("field", "label-volume-field"),
                "pedir" => ("pedir", "label-volume-pedir"),
                "cmdline" => ("cmdline", "label-volume-cmdline"),
                "xform" => ("xform", "label-volume-xform"),
                "content" => ("content", "label-volume-content"),
                "projections" => ("projections", "label-volume-projections"),
                "angles" => ("angles", "label-volume-angles"),
                "angle-step" => ("angle-step", "label-volume-angle-step"),
                "kv" => ("kv", "label-volume-kv"),
                "current" => ("current", "label-volume-current"),
                "power" => ("power", "label-volume-power"),
                "exposure" => ("exposure", "label-volume-exposure"),
                "sod" => ("sod", "label-volume-sod"),
                "odd" => ("odd", "label-volume-odd"),
                "sdd" => ("sdd", "label-volume-sdd"),
                "magnification" => ("magnification", "label-volume-magnification"),
                "binning" => ("binning", "label-volume-binning"),
                "objective" => ("objective", "label-volume-objective"),
                "filter" => ("filter", "label-volume-filter"),
                "camera" => ("camera", "label-volume-camera"),
                "instrument" => ("instrument", "label-volume-instrument"),
                "software" => ("software", "label-volume-software"),
                "date" => ("date", "label-volume-date"),
                "ring" => ("ring", "label-volume-ring"),
                "smoothing" => ("smoothing", "label-volume-smoothing"),
                "coords" => ("coords", "label-volume-coords"),
                "eye" => ("eye", "label-volume-eye"),
                "technique" => ("technique", "label-volume-technique"),
                "version" => ("version", "label-volume-version"),
                "isotope" => ("isotope", "label-volume-isotope"),
                "tracer" => ("tracer", "label-volume-tracer"),
                "units" => ("units", "label-volume-units"),
                _ => continue,
            };
            let val = match value {
                VolumeParam::Number(x, unit) if unit.is_empty() => self.l.trimmed(*x, 4),
                VolumeParam::Number(x, unit) if unit == "°" => {
                    format!("{}°", self.l.trimmed(*x, 2))
                }
                VolumeParam::Number(x, unit) if unit == "×" => {
                    format!("{}×", self.l.trimmed(*x, 2))
                }
                // Industrial CT: what a scan holds and how its grid is spaced, in words.
                VolumeParam::Text(t) if key == "content" || key == "coords" => match t.as_str() {
                    "projections" => self.l.text("volume-content-projections"),
                    "reconstruction" => self.l.text("volume-content-reconstruction"),
                    "rectilinear" => self.l.text("volume-coords-rectilinear"),
                    _ => t.clone(),
                },
                // An eye scan (OCT): right (OD) or left (OS).
                VolumeParam::Text(t) if key == "eye" => match t.as_str() {
                    "OD" => self.l.text("volume-eye-right"),
                    "OS" => self.l.text("volume-eye-left"),
                    _ => t.clone(),
                },
                VolumeParam::Number(x, unit) => {
                    format!("{} {}", self.l.trimmed(*x, 4), self.l.unit(unit))
                }
                VolumeParam::Text(t) => t.clone(),
            };
            g.add(key, self.l.text(label), val, None);
        }
        g
    }

    fn xmp_orientation(&self) -> Option<u8> {
        self.xmp("tiff:Orientation")?
            .parse::<u8>()
            .ok()
            .filter(|v| (1..=8).contains(v))
    }

    fn image(&mut self) -> GroupBuf {
        let mut g = GroupBuf::new("image", self.l.text("group-image"));
        let f = &self.input.file;
        let exif = self.exif.filter(|_| !f.drawn);
        let w = f.width.or_else(|| exif?.uint(Tag::PixelXDimension));
        let h = f.height.or_else(|| exif?.uint(Tag::PixelYDimension));
        if let (Some(w), Some(h)) = (w, h) {
            g.add(
                "dimensions",
                self.l.text("label-dimensions"),
                strings::dimensions(self.l, w, h),
                None,
            );
        }
        g.add_opt(
            "encoding",
            self.l.text("label-encoding"),
            (!f.drawn)
                .then(|| {
                    strings::encoding(
                        self.l,
                        f.bit_depth,
                        f.colour_model.as_deref(),
                        f.chroma_subsampling.as_deref(),
                        f.alpha.as_deref(),
                        f.progressive.filter(|_| !f.hierarchical),
                    )
                    .map(|e| {
                        if f.hierarchical {
                            strings::hierarchical(self.l, e)
                        } else {
                            e
                        }
                    })
                })
                .flatten(),
        );
        let desc = self.icc.and_then(|i| i.description.clone());
        self.report.icc_description = desc.clone();
        // No embedded profile means sRGB, which needs no row.
        let profile = match (self.icc, desc) {
            (_, Some(d)) => Some(d),
            (Some(i), None) => Some(i.colour_space.clone()),
            (None, None) => {
                let interop = self
                    .exif
                    .and_then(|e| e.exif.get_field(Tag::InteroperabilityIndex, In::PRIMARY))
                    .and_then(|f| match &f.value {
                        exif::Value::Ascii(v) => {
                            v.first().map(|b| String::from_utf8_lossy(b).into_owned())
                        }
                        _ => None,
                    });
                (interop.as_deref() == Some("R03")).then(|| self.l.text("adobe-rgb-from-exif"))
            }
        };
        g.add_opt(
            "colour_profile",
            self.l.text("label-colour-profile"),
            profile,
        );
        let orientation = self.exif_orientation().or_else(|| self.xmp_orientation());
        self.report.orientation = orientation;
        if let Some(o) = orientation.and_then(|v| strings::orientation(self.l, v)) {
            g.add("orientation", self.l.text("label-orientation"), o, None);
        }
        g
    }

    // ----- File -----------------------------------------------------------

    fn file(&self) -> GroupBuf {
        let mut g = GroupBuf::new("file", self.l.text("group-file"));
        let f = &self.input.file;
        if let Some(p) = &f.path {
            g.add_opt(
                "name",
                self.l.text("label-file-name"),
                p.file_name().map(|n| n.to_string_lossy().into_owned()),
            );
            g.add_opt(
                "folder",
                self.l.text("label-folder"),
                p.parent().map(|d| d.display().to_string()),
            );
        }
        if let Some(sz) = f.file_size {
            g.add(
                "size",
                self.l.text("label-file-size"),
                strings::file_size(self.l, sz),
                (sz >= 1000).then(|| strings::file_size_exact(self.l, sz)),
            );
        }
        g.add_opt("format", self.l.text("label-format"), f.format_name.clone());
        if f.drawn {
            g.add_opt(
                "software",
                self.l.text("label-application"),
                self.ascii(Tag::Software)
                    .or_else(|| self.xmp("xmp:CreatorTool")),
            );
        }
        g.add_opt(
            "modified",
            self.l.text("label-modified"),
            f.modified.map(|t| datetime::format_system_time(t, self.l)),
        );
        g
    }

    // ----- Rights and description ------------------------------------------

    /// Text of one of our extra EXIF text tags (XP*, EXIF 3.0 ImageTitle, …).
    fn exif_extra(&self, num: u16) -> Option<String> {
        self.exif?.extra_text(num)
    }

    fn rights(&mut self) -> GroupBuf {
        let mut g = GroupBuf::new("rights", self.l.text("group-rights"));
        let title = self
            .xmp("dc:title")
            .or_else(|| self.xmp("photoshop:Headline"))
            .or_else(|| self.exif_extra(exif_tags::IMAGE_TITLE))
            .or_else(|| self.exif_extra(exif_tags::XP_TITLE));
        let description = self
            .xmp("dc:description")
            .or_else(|| self.ascii(Tag::ImageDescription))
            .or_else(|| self.exif_extra(exif_tags::XP_COMMENT));
        g.add_opt("title", self.l.text("label-title"), title.clone());
        if description != title {
            g.add_opt("description", self.l.text("label-description"), description);
        }
        g.add_opt(
            "creator",
            self.l.text("label-creator"),
            self.xmp_list("dc:creator")
                .or_else(|| self.ascii(Tag::Artist))
                .or_else(|| self.exif_extra(exif_tags::PHOTOGRAPHER))
                .or_else(|| self.exif_extra(exif_tags::XP_AUTHOR)),
        );
        g.add_opt(
            "copyright",
            self.l.text("label-copyright"),
            self.xmp("dc:rights").or_else(|| self.ascii(Tag::Copyright)),
        );
        g.add_opt(
            "keywords",
            self.l.text("label-keywords"),
            self.xmp_list("dc:subject").or_else(|| {
                // XPKeywords separates keywords with ';'.
                let k: Vec<String> = self
                    .exif_extra(exif_tags::XP_KEYWORDS)?
                    .split(';')
                    .map(|k| k.trim().to_owned())
                    .filter(|k| !k.is_empty())
                    .collect();
                non_empty(Some(k.join(", ")))
            }),
        );
        // A container's comment (JPEG, PNG, GIF, movie tags), when it says something new.
        let comment = non_empty(self.derived.get("exif:UserComment").map(str::to_owned))
            .filter(|c| g.items.iter().all(|i| i.value != *c));
        g.add_opt("comment", self.l.text("label-comment"), comment);
        let rating = self
            .xmp("xmp:Rating")
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| {
                self.exif?
                    .uint(Tag(Context::Tiff, exif_tags::RATING))
                    .map(|v| v as f64)
            })
            .or_else(|| self.fuji?.rating.filter(|r| *r <= 5).map(f64::from))
            .filter(|r| r.is_finite())
            .map(|r| r.round().clamp(-1.0, 5.0) as i8)
            // 0 means "not rated".
            .filter(|r| *r != 0);
        self.report.rating = rating;
        g.add_opt(
            "rating",
            self.l.text("label-rating"),
            rating.and_then(|v| strings::rating(self.l, v)),
        );
        let label = self.xmp("xmp:Label");
        self.report.label = label.clone();
        g.add_opt("label", self.l.text("label-colour-label"), label);
        g
    }

    fn credentials(&self) -> GroupBuf {
        let mut g = GroupBuf::new("credentials", self.l.text("group-credentials"));
        if self.input.has_c2pa {
            g.add(
                "c2pa",
                self.l.text("label-credentials"),
                self.l.text("c2pa-present"),
                Some(self.l.text("c2pa-detail").to_owned()),
            );
        }
        g
    }

    // ----- XMP all fields -------------------------------------------------

    fn xmp_fields(&self) -> Vec<FieldGroup> {
        let mut groups: Vec<(String, FieldGroup)> = Vec::new();
        for p in &self.xmp.properties {
            if exif_tags::XMP_IDENTIFYING.contains(&p.name.as_str()) {
                continue;
            }
            let idx = match groups.iter().position(|(k, _)| *k == p.prefix) {
                Some(i) => i,
                None => {
                    let title = strings::fields_xmp(self.l, &p.prefix);
                    groups.push((
                        p.prefix.clone(),
                        FieldGroup {
                            title,
                            fields: Vec::new(),
                        },
                    ));
                    groups.len() - 1
                }
            };
            let raw = p.values.join("; ");
            let rating = (p.name == "xmp:Rating")
                .then(|| raw.trim().parse::<f64>().ok())
                .flatten()
                .map(|r| r.round().clamp(-1.0, 5.0) as i8);
            let empty = p.values.iter().all(|v| v.trim().is_empty()) || rating == Some(0);
            let display = match p.name.as_str() {
                "xmp:Rating" => rating
                    .and_then(|r| strings::rating(self.l, r))
                    .unwrap_or_else(|| raw.clone()),
                "tiff:Orientation" => raw
                    .parse::<u8>()
                    .ok()
                    .and_then(|v| strings::orientation(self.l, v))
                    .unwrap_or_else(|| raw.clone()),
                n if n.ends_with("Date")
                    || n.ends_with("DateTimeOriginal")
                    || n.ends_with("DateCreated") =>
                {
                    datetime::parse_xmp_datetime(&raw)
                        .map_or_else(|| raw.clone(), |s| s.display(self.l))
                }
                _ => raw.clone(),
            };
            groups[idx].1.fields.push(Field {
                tag: p.name.clone(),
                raw,
                display,
                empty,
                full: None,
            });
        }
        groups.into_iter().map(|(_, g)| g).collect()
    }
}

/// "Canon" + "Canon EOS R5" → "Canon EOS R5"; "NIKON CORPORATION" + "NIKON D850" → "NIKON D850".
/// Exposure time from an APEX ShutterSpeedValue (t = 2^-Tv), as a fraction for
/// [`strings::exposure_time`].
fn apex_exposure_time(tv: f64) -> Option<(u32, u32)> {
    if !(-12.0..=30.0).contains(&tv) {
        return None;
    }
    let t = (-tv).exp2();
    Some(if t < 1.0 {
        (1, t.recip().round() as u32)
    } else {
        ((t * 10.0).round() as u32, 10)
    })
}

/// F-number from an APEX ApertureValue (N = 2^(Av/2)).
fn apex_f_number(av: f64) -> Option<f64> {
    (0.0..=32.0)
        .contains(&av)
        .then(|| (av / 2.0).exp2())
        .map(|f| (f * 10.0).round() / 10.0)
}

fn camera_name(make: Option<&str>, model: Option<&str>) -> Option<String> {
    match (make, model) {
        (Some(mk), Some(md)) => {
            let first = mk.split_whitespace().next().unwrap_or(mk).to_lowercase();
            if md.to_lowercase().starts_with(&first) {
                Some(md.to_owned())
            } else {
                Some(format!("{mk} {md}"))
            }
        }
        (Some(mk), None) => Some(mk.to_owned()),
        (None, Some(md)) => Some(md.to_owned()),
        (None, None) => None,
    }
}

/// XMP GPS coordinate: "38,43.38N" or "38,43,22.97N" → signed decimal.
fn parse_xmp_coord(s: &str) -> Option<f64> {
    let s = s.trim();
    let last = s.chars().last()?;
    let sign = match last.to_ascii_uppercase() {
        'N' | 'E' => 1.0,
        'S' | 'W' => -1.0,
        _ => return None,
    };
    let body = &s[..s.len() - 1];
    let parts: Vec<f64> = body
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let v = match parts.as_slice() {
        [d] => *d,
        [d, m] => d + m / 60.0,
        [d, m, sec] => d + m / 60.0 + sec / 3600.0,
        _ => return None,
    };
    Some(sign * v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_names() {
        assert_eq!(
            camera_name(Some("Canon"), Some("Canon EOS R5")).unwrap(),
            "Canon EOS R5"
        );
        assert_eq!(
            camera_name(Some("NIKON CORPORATION"), Some("NIKON D850")).unwrap(),
            "NIKON D850"
        );
        assert_eq!(
            camera_name(Some("FUJIFILM"), Some("X-T5")).unwrap(),
            "FUJIFILM X-T5"
        );
    }

    #[test]
    fn ic_layout_group() {
        let l = Localizer::new(unic_langid::langid!("en-GB"));
        let input = MetadataInput {
            drawing: Some(DrawingFacts {
                format: "GDSII".into(),
                version: "600".into(),
                units: 13,
                unit_symbol: Some("µm".into()),
                extents: Some((2920.0, 3520.0)),
                layers: 64,
                ic: Some(IcLayoutFacts {
                    library: "caravel".into(),
                    db_unit_m: 1e-9,
                    user_unit_m: 1e-6,
                    cell: "user_project_wrapper".into(),
                    cells: 1200,
                    top_cells: (0..10).map(|i| format!("T{i}")).collect(),
                    layers: 64,
                    shapes: 5_000_000,
                    texts: 12,
                    references: 4000,
                    placements: 1,
                    flat_shapes: 9_000_000,
                    bounds_um: Some([-10.0, 0.0, 2910.0, 3520.0]),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let r = read(&input, &l);
        let g = r
            .summary
            .iter()
            .find(|g| g.key == "drawing")
            .expect("a drawing group");
        let v = |k: &str| {
            g.items
                .iter()
                .find(|i| i.key == k)
                .map(|i| i.value.replace(['\u{2068}', '\u{2069}'], ""))
                .unwrap_or_else(|| panic!("{k} in {g:?}"))
        };
        assert_eq!(v("library"), "caravel");
        assert_eq!(v("layout_units"), "1 nm database unit, 1 µm user unit");
        assert_eq!(v("cell"), "user_project_wrapper");
        assert!(v("top_cells").ends_with("and 2 more"), "{}", v("top_cells"));
        assert_eq!(v("references"), "4,000 (1 placement)");
        assert_eq!(v("flat_shapes"), "9,000,000");
        assert_eq!(v("bounds"), "-10, 0 to 2,910, 3,520 µm");
    }

    #[test]
    fn planetary_group() {
        let l = Localizer::new(unic_langid::langid!("en-GB"));
        let input = MetadataInput {
            science: Some(ScienceFacts {
                format: "PDS3".into(),
                planetary: Some(PlanetaryFacts {
                    target: Some("MOON".into()),
                    spacecraft: Some("LUNAR RECONNAISSANCE ORBITER".into()),
                    instrument: Some("LUNAR ORBITER LASER ALTIMETER".into()),
                    start: Some("2009-07-13T17:33:17".into()),
                    projection: Some("SIMPLE CYLINDRICAL".into()),
                    metres_per_pixel: Some(1895.2102),
                    pixels_per_degree: Some(16.0),
                    centre: Some((0.0, 180.0)),
                    latitudes: Some((-90.0, 90.0)),
                    longitudes: Some((0.0, 360.0)),
                    radius_km: Some(1737.4),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let r = read(&input, &l);
        let g = r
            .summary
            .iter()
            .find(|g| g.key == "planetary")
            .expect("a planetary group");
        let v = |k: &str| {
            g.items
                .iter()
                .find(|i| i.key == k)
                .map(|i| i.value.replace(['\u{2068}', '\u{2069}'], ""))
                .unwrap_or_else(|| panic!("{k}"))
        };
        assert_eq!(v("target"), "MOON");
        assert_eq!(v("projection"), "SIMPLE CYLINDRICAL");
        assert_eq!(
            v("resolution"),
            "1,895.21 m per pixel, 16 pixels per degree"
        );
        assert_eq!(v("centre"), "0°, 180°");
        assert_eq!(v("coverage"), "latitude -90° to 90°, longitude 0° to 360°");
        assert_eq!(v("radius"), "1,737.4 km");
        assert!(g.items.iter().all(|i| i.key != "stop"));
    }

    #[test]
    fn hdf4_and_hdf_eos_groups() {
        let l = Localizer::new(unic_langid::langid!("en-GB"));
        let input = MetadataInput {
            science: Some(ScienceFacts {
                format: "HDF4".into(),
                domain: ScienceDomain::Gridded,
                hdf4: Some(Hdf4Facts {
                    library: Some("NCSA HDF Version 4.2 Release 15".into()),
                    datasets: 12,
                    images: 0,
                    vgroups: 7,
                    vdatas: 3,
                    eos: Some(HdfEosFacts {
                        version: Some("HDFEOS_V2.19".into()),
                        grids: vec!["MOD_Grid_500m".into()],
                        grid: Some("MOD_Grid_500m".into()),
                        projection: Some("GCTP_SNSOID".into()),
                        grid_size: Some((2400, 2400)),
                        corners: Some([-1111950.5197, 5559752.598, 0.0, 4447802.0788]),
                        short_name: Some("MOD09A1".into()),
                        begin: Some("2020-06-01T00:00:00".into()),
                        end: Some("2020-06-08T23:59:59".into()),
                        ..Default::default()
                    }),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let r = read(&input, &l);
        let group = |key: &str| {
            r.summary
                .iter()
                .find(|g| g.key == key)
                .unwrap_or_else(|| panic!("a {key} group"))
        };
        let v = |key: &str, k: &str| {
            group(key)
                .items
                .iter()
                .find(|i| i.key == k)
                .map(|i| i.value.replace(['\u{2068}', '\u{2069}'], ""))
                .unwrap_or_else(|| panic!("{k} in {key}"))
        };
        assert_eq!(v("hdf4", "library"), "NCSA HDF Version 4.2 Release 15");
        assert_eq!(v("hdf4", "datasets"), "12");
        assert_eq!(v("hdf4", "vgroups"), "7");
        assert_eq!(v("hdf_eos", "structures"), "1 grid (MOD_Grid_500m)");
        assert_eq!(v("hdf_eos", "projection"), "GCTP_SNSOID");
        assert_eq!(v("hdf_eos", "grid_size"), "2,400 × 2,400");
        assert_eq!(
            v("hdf_eos", "corners"),
            "-1,111,950.52, 5,559,752.598 to 0, 4,447,802.079 m"
        );
        assert_eq!(v("hdf_eos", "product"), "MOD09A1");
        assert_eq!(
            v("hdf_eos", "time"),
            "2020-06-01T00:00:00 to 2020-06-08T23:59:59"
        );
    }

    #[test]
    fn omero_group() {
        let l = Localizer::new(unic_langid::langid!("en-GB"));
        let input = MetadataInput {
            science: Some(ScienceFacts {
                format: "OMERO".into(),
                omero: Some(OmeroFacts {
                    server: "https://idr.openmicroscopy.org".into(),
                    image_id: 9836842,
                    name: "B4_1.tif".into(),
                    screen: Some("idr0001-graml-sysgro/screenA".into()),
                    plate: Some("JL_120731_S6A".into()),
                    well: Some("B4".into()),
                    owner: Some("Public User".into()),
                    group: Some("public".into()),
                    acquired: Some("2012-07-31 10:00:00".into()),
                    size: [1344, 1024, 1, 3, 1],
                    pixels_type: "uint16".into(),
                    physical: [
                        Some((0.3225, "µm".into())),
                        Some((0.3225, "µm".into())),
                        None,
                    ],
                    channels: vec![
                        ("DAPI".into(), true),
                        ("GFP".into(), false),
                        (String::new(), true),
                    ],
                    tiles: Some((512, 512, 3)),
                    encoding: "JPEG".into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let r = read(&input, &l);
        let g = r
            .summary
            .iter()
            .find(|g| g.key == "omero")
            .expect("an omero group");
        let v = |k: &str| {
            g.items
                .iter()
                .find(|i| i.key == k)
                .map(|i| i.value.replace(['\u{2068}', '\u{2069}'], ""))
                .unwrap_or_else(|| panic!("{k} in omero"))
        };
        assert_eq!(v("server"), "https://idr.openmicroscopy.org (public)");
        assert_eq!(v("image"), "B4_1.tif (ID 9836842)");
        assert_eq!(
            v("screen"),
            "idr0001-graml-sysgro/screenA › JL_120731_S6A › B4"
        );
        assert!(!g.items.iter().any(|i| i.key == "project"));
        assert_eq!(v("owner"), "Public User (public)");
        assert_eq!(v("dimensions"), "1,344 × 1,024, 3 channels, uint16");
        assert_eq!(v("physical"), "0.3225 µm × 0.3225 µm");
        assert_eq!(v("channels"), "DAPI, GFP (hidden), 3");
        assert_eq!(v("fetched"), "JPEG tiles of 512 × 512, 3 levels");
    }

    /// Acquisition dates read like the panel's other dates, whichever way the file wrote them;
    /// what is not a date stays as written.
    #[test]
    fn acquisition_dates_are_localised() {
        let l = Localizer::new(unic_langid::langid!("en-GB"));
        let acquired = |d: &str| {
            let r = read(
                &MetadataInput {
                    science: Some(ScienceFacts {
                        format: "OME-TIFF".into(),
                        domain: ScienceDomain::Microscopy,
                        date_obs: Some(d.into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                &l,
            );
            let g = r.summary.iter().find(|g| g.key == "science").unwrap();
            let v = &g.items.iter().find(|i| i.key == "date_obs").unwrap().value;
            v.replace(['\u{2068}', '\u{2069}'], "")
        };
        let want = "Monday 12 October 2026, 09:30";
        assert_eq!(acquired("2026-10-12T09:30:00"), want);
        assert_eq!(acquired("2026-10-12 09:30:00"), want);
        assert_eq!(acquired("2026:10:12 09:30:00"), want);
        assert_eq!(acquired("2026-10-12T09:30:00Z"), format!("{want} (UTC)"));
        // DICOM DT.
        assert_eq!(acquired("20261012093000"), want);
        assert_eq!(acquired("202610120930"), want);
        assert_eq!(acquired("20261012093000.25"), want);
        assert_eq!(acquired("20261012"), "20261012");
        assert_eq!(acquired("12/10/2026"), "12/10/2026");
        assert_eq!(acquired("2026-10-12"), "2026-10-12");
    }

    /// An OMERO image's name, dataset, acquisition date, sizes and pixel size are told once,
    /// in its OMERO group; the Microscopy group keeps what only it has.
    #[test]
    fn omero_rows_are_not_repeated_in_microscopy() {
        let l = Localizer::new(unic_langid::langid!("en-GB"));
        let science = ScienceFacts {
            format: "OMERO".into(),
            domain: ScienceDomain::Microscopy,
            title: Some("B4_1.tif".into()),
            dataset: Some("Plate 1".into()),
            vendor: Some("OMERO".into()),
            date_obs: Some("2012-07-31 10:00:00".into()),
            dims: vec![1344, 1024],
            pixel_size: Some((0.3225, 0.3225, "µm".into())),
            magnification: Some(40.0),
            omero: Some(OmeroFacts {
                server: "https://idr.openmicroscopy.org".into(),
                image_id: 1,
                name: "B4_1.tif".into(),
                acquired: Some("2012-07-31 10:00:00".into()),
                size: [1344, 1024, 1, 1, 1],
                ..Default::default()
            }),
            ..Default::default()
        };
        let keys = |science: ScienceFacts| {
            let r = read(
                &MetadataInput {
                    science: Some(science),
                    ..Default::default()
                },
                &l,
            );
            r.summary
                .iter()
                .find(|g| g.key == "science")
                .map(|g| g.items.iter().map(|i| i.key).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        assert_eq!(keys(science.clone()), ["magnification"]);
        // Without OMERO the same facts are all shown.
        let plain = keys(ScienceFacts {
            omero: None,
            ..science
        });
        for k in [
            "title",
            "dataset",
            "vendor",
            "date_obs",
            "dims",
            "pixel_size",
        ] {
            assert!(plain.contains(&k), "{k} in {plain:?}");
        }
    }

    #[test]
    fn xmp_coords() {
        assert!((parse_xmp_coord("38,43.338N").unwrap() - 38.7223).abs() < 1e-4);
        assert!((parse_xmp_coord("9,8.358W").unwrap() + 9.1393).abs() < 1e-4);
    }
}
