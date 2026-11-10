//! Image decoding front-end: format probing, a registry of decoders, and per-image sessions.
//!
//! A [`Session`] is created once per opened file and is shared between threads. It answers
//! cheap questions (size, orientation, ICC profile, embedded preview) without decoding, and
//! decodes any *scale* (1, 2, 4, 8) and *region* on request. Sessions keep whatever state
//! makes later requests faster (the JPEG row index, a decoded PNG).

mod array_dec;
mod asdf_dec;
mod avif_dec;
mod bitmap;
mod bpg_dec;
mod braw_dec;
mod cad_dec;
pub mod calibrate;
mod casa_dec;
mod cdr_dec;
mod colour;
mod colour_space;
mod container_locate;
mod czi_dec;
mod dicom_dec;
mod dicom_doc;
mod dicom_report;
pub use dicom_doc::EncapsulatedFacts;
pub use dicom_report::{SR_KIND, WAVEFORM_KIND};
mod bitmap_font_dec;
mod docpage_dec;
mod drawdoc_dec;
pub mod drawing;
mod dxf_dec;
mod em_dec;
mod embedded_dec;
mod epub_dec;
mod exif;
mod exif_locate;
mod exr_dec;
mod fcs_dec;
mod fits_dec;
mod flif_dec;
mod font_dec;
mod gainmap;
mod gds_dec;
pub mod geo;
mod geopdf;
mod georaster_dec;
mod gis_dec;
mod lottie_dec;
mod nitf_dec;
pub mod notebook;
pub mod office_dec;
pub mod playlist_dec;
mod sql_host;
mod tiles_dec;
pub use tiles_dec::{open_tile_file, render_vector_tile, tile_rgba};
pub mod astro;
mod fpx_dec;
mod grib_dec;
mod grid_dec;
mod hdf4_dec;
mod hdf5_dec;
mod heic;
mod hw_jpeg;
mod icns_dec;
pub mod ident;
pub use ident::{Ambiguity, Candidate, Confidence};
mod aseprite_dec;
mod dvi_dec;
mod ifc_dec;
mod iff_dec;
mod imzml_dec;
mod isyntax_dec;
mod jp2_dec;
mod jpeg;
mod jpegls_dec;
mod jpm_dec;
mod jxl_dec;
mod jxr_dec;
mod landxml_dec;
mod legacy_dec;
mod linear;
mod matsci_dec;
mod medvol_dec;
mod mesh_dec;
mod meta_locate;
mod metafile;
pub mod mime;
mod mng_dec;
mod mol_dec;
pub mod neuro_dec;
mod occt_dec;
mod omero_dec;
pub mod optional;
mod ora_dec;
mod pcb_dec;
mod pdf_dec;
mod pdn_dec;
mod pict_dec;
mod planetary_dec;
pub mod pmi;
mod png_dec;
mod points_dec;
mod preview_doc;
mod psd_dec;
mod psp_dec;
mod radiance_dec;
mod raw_dec;
mod raw_tiles;
mod rawclip_dec;
mod retro2_dec;
mod retro_dec;
mod root_dec;
mod rti_dec;
mod s2_dec;
mod sar;
pub mod science;
mod scitiff_dec;
mod segy_dec;
mod ser_dec;
pub mod sniff;
pub mod sound_ident;
mod spectra_dec;
mod spectro_dec;
mod splat_dec;
mod stac_dec;
mod svg;
mod svg_css;
mod svg_fonts;
mod tdms_dec;
pub mod text_dec;
pub mod text_tags;
mod texture_dec;
mod thermal_dec;
mod tiff_dec;
mod tiff_exif;
mod tiles3d_dec;
mod ugrid_dec;
mod vendor_dec;
mod vendor_gis_dec;
mod video_dec;
mod vrt_dec;
mod vtk_dec;
mod webp_dec;
mod whole;
mod whole_dec;
pub mod wsi_dec;
mod xcf_dec;
mod xisf_dec;
mod xtal_dec;
pub mod zarr_url_dec;

pub use cad_dec::{DwgConverter, find_oda_converter, set_dwg_converter};
pub use colour_space::{SPACES as COLOUR_SPACES, colour_space, icc_space};
pub use container_locate::is_cr3;
pub use czi_dec::{CziConfig, find_czi, set_czi};
pub use dicom_dec::{
    Calibration, DEFAULT_CINE_FRAME, Graphics, MedicalSummary, ModalityFrame, SpacingKind,
    Uncalibrated, Voi, VoiFunction, VoiSource, open_dicom_frame,
};
pub use exif::{
    exif_date_taken, exif_facts, exif_orientation, exif_position, jpeg_exif, png_date_taken,
};
pub use exif_locate::{exif_tiff, jxl_xmp, read_exif_tiff};
pub use gainmap::{
    apple_gain_map, apple_headroom, jhgm_params, jpeg_gain_map, jpeg_has_gain_map, mpo_pages,
    parse_hdrgm_xmp, parse_iso21496, tmap_params,
};
pub use hdf5_dec::allow_linked_file;
pub use heic::set_libheif_fallback;
pub use icns_dec::{icns_page_labels, open_icns_page};
pub use landxml_dec::{MODEL_PAGE as LANDXML_MODEL_PAGE, station_label};
pub use medvol_dec::{VolumeFacts, is_research_volume_name, medvol, research_frame};
pub use mesh_dec::format_of as mesh_format;
pub use meta_locate::{Embedded, embedded_metadata};
pub use occt_dec::{OcctConfig, occt_bridge, set_occt};
pub use omero_dec::{OMERO_MAGIC, OmeroChannel, OmeroFacts, OmeroOpener, set_omero_opener};
pub use optional::OptionalId;
pub use pcb_dec::{is_board_archive, is_odb_archive};
pub use points_dec::format_of as points_format;
pub use rti_dec::{read_relight_folder, relit_raster};
pub use s2_dec::is_sentinel2_archive;
pub use segy_dec::{SeismicCube, seismic_cube};
pub use spectra_dec::SpectrumFacts;
pub use specular_epub;
pub use specular_font;
/// Whether a JPEG is hierarchical (T.81 Annex J), for the Info panel.
pub use specular_jpeg::is_hierarchical as jpeg_is_hierarchical;
pub use specular_mesh;
pub use specular_mol;
pub use specular_points;
pub use specular_rti;
pub use specular_splat;
pub use specular_subs;
pub use specular_textdoc;
pub use specular_tiles3d;
pub use stac_dec::{StacRemote, set_stac_remote};
pub use text_dec::{SubtitleModel, TextModel};
pub use text_tags::{TextTags, text_tags};
pub use texture_dec::open_texture_page;
pub use tiff_dec::open_tiff_page;
pub use tiles3d_dec::{
    ADDRESS_FILE as TILES3D_ADDRESS_FILE, RemoteFetch as Tiles3dFetch, Tiles3dModel, Tiles3dScene,
    Tiles3dView, set_tiles3d_remote,
};
/// Decodes an embedded picture (a sound file's cover, an SVG's raster) with the in-tree
/// decoders: width, height and straight-alpha RGBA8 rows. `None` when it is not an image
/// this build reads, or larger than 64 megapixels.
pub fn decode_rgba8(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    svg::decode_rgba(data)
}

pub use video_dec::{SoundFacts, VideoFacts, VideoOpener, set_video_opener};
pub use zarr_url_dec::{ZARR_LINK_MAGIC, ZarrLink, ZarrStoreOpener, set_zarr_store_opener};

use specular_core::{CancelToken, Orientation, Raster, Source};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

/// File formats Specular can decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Jpeg,
    Png,
    Gif,
    WebP,
    Avif,
    Jxl,
    /// JPEG 2000 (JP2, J2K codestreams, JPX through its first codestream).
    Jpeg2000,
    Heic,
    Tiff,
    /// GeoTIFF: a TIFF with a GeoKey directory (`image/tiff; application=geotiff`).
    GeoTiff,
    Bmp,
    Ico,
    Tga,
    Qoi,
    Pnm,
    Dds,
    Ktx2,
    /// KTX version 1.
    Ktx,
    /// PowerVR PVR version 3.
    Pvr,
    /// Valve Texture Format.
    Vtf,
    /// The `.astc` file of astcenc (one ASTC image).
    Astc,
    /// The Basis Universal `.basis` file (ETC1S or UASTC).
    Basis,
    Svg,
    Exr,
    Hdr,
    /// Portable Float Map (`PF`, `Pf`, `PF4`).
    Pfm,
    /// Camera RAW (DNG, CR2, CR3, NEF, ARW, RAF, ORF, RW2, PEF, ...).
    Raw,
    /// Photoshop PSD / PSB.
    Psd,
    /// GIMP XCF (plain, or in gzip, bzip2 or xz).
    Xcf,
    Pcx,
    /// Multi-page PCX; one page per session.
    Dcx,
    Sgi,
    SunRaster,
    Xbm,
    Xpm,
    Wbmp,
    /// Amstrad CPC screen memory dumps.
    CpcScreen,
    /// MSX BSAVE screens (screens 2, 5–8, 10, 12).
    MsxScreen,
    /// Apple II hi-res screens.
    AppleHgr,
    /// Apple II double hi-res screens.
    AppleDhr,
    /// BBC Micro screen memory dumps.
    BbcScreen,
    /// Atari 8-bit Micro Painter pictures.
    AtariMic,
    /// Atari 8-bit Graphics 8 screens.
    AtariGr8,
    /// Atari 8-bit Graphics 9 screens.
    AtariGr9,
    /// Kodak Photo CD image packs.
    PhotoCd,
    Farbfeld,
    /// Autodesk FLI / FLC animation.
    Fli,
    /// DEC Sixel graphics (`.six`, `.sixel`).
    Sixel,
    /// X cursor files (cursor themes); one page per size.
    Xcursor,
    /// QuickTime still images (`.qtif`): JPEG, PNG, TIFF, GIF or raw RGB inside.
    Qtif,
    /// Stardent AVS X image.
    Avs,
    MacPaint,
    /// GEM bit images (`.img`, by content) and XIMG.
    GemImg,
    /// Atari ST DEGAS and DEGAS Elite (`.pi1`–`.pi3`, `.pc1`–`.pc3`).
    Degas,
    /// Atari ST NEOchrome (`.neo`).
    Neochrome,
    /// Commodore 64 Koala Painter.
    Koala,
    /// ZX Spectrum screen dumps (`.scr`, by size).
    ZxScreen,
    /// Dr. Halo CUT with its `.pal`.
    DrHalo,
    /// Softimage PIC (by its magic; `.pic` is shared with Radiance).
    SoftimagePic,
    /// Alias PIX.
    AliasPix,
    /// Wavefront RLA and 3ds Max RPF.
    Rla,
    /// Utah Raster Toolkit RLE.
    UtahRle,
    /// Quake 2 WAL textures.
    Wal,
    /// Blizzard BLP1 / BLP2 textures.
    Blp,
    /// SMPTE 268M Digital Picture Exchange (film scans).
    Dpx,
    /// Kodak Cineon film scans.
    Cineon,
    /// X Window dumps.
    Xwd,
    /// ImageMagick's own format.
    Miff,
    /// PlayStation TIM textures.
    Tim,
    /// GIMP palettes (`.gpl`), shown as swatch sheets.
    GimpPalette,
    /// Photoshop colour swatches (`.aco`).
    PhotoshopSwatches,
    /// Adobe colour tables (`.act`).
    AdobeColorTable,
    /// Adobe Swatch Exchange (`.ase` palettes).
    SwatchExchange,
    /// Aseprite sprites (`.aseprite`, `.ase`), frames animated.
    Aseprite,
    /// GIMP gradients (`.ggr`).
    GimpGradient,
    /// GIMP brushes (`.gbr`).
    GimpBrush,
    /// GIMP animated brushes (`.gih`), cells as pages.
    GimpBrushPipe,
    /// GIMP patterns (`.pat`).
    GimpPattern,
    /// Microsoft Paint pictures (`.msp`).
    MsPaint,
    /// libgd GD images (`.gd`).
    Gd,
    /// libgd GD2 images (`.gd2`).
    Gd2,
    /// Pixar Image Computer pictures (`.pxr`).
    PixarPicture,
    /// LabEye / IFUNC IM images (`.im`).
    LabEyeIm,
    /// IM Tools images (`.imt`).
    ImTools,
    /// XV thumbnails (`P7 332`), by content.
    XvThumbnail,
    /// McIDAS AREA satellite images (`.area`).
    McIdasArea,
    /// JPEG 2000 conformance raw components (`.pgx`).
    Pgx,
    /// CALS Type 1 rasters (`.cal`).
    Cals,
    /// Ericsson PKM textures (ETC1, ETC2, EAC).
    Pkm,
    /// Firaxis FTEX textures (`.ftc`, `.ftu`).
    Ftex,
    /// FBI WSQ fingerprints.
    Wsq,
    /// JBIG (ITU-T T.82) bi-level images.
    Jbig,
    /// Standalone JBIG2 (ITU-T T.88) files.
    Jbig2,
    /// JPEG XS (ISO/IEC 21122) codestreams and image files.
    JpegXs,
    /// DICOM (medical imaging), one frame per session.
    Dicom,
    /// JPEG-LS (ITU-T T.87) codestreams.
    JpegLs,
    /// FlashPix: tiled resolution pyramid in an OLE compound file.
    Fpx,
    /// Apple icon; one icon size per page, largest first.
    Icns,
    /// OpenRaster: its merged image (or its PNG layers flattened).
    Ora,
    /// Krita: its merged image (or its preview).
    Kra,
    /// Amiga IFF bitmap (ILBM, PBM, ACBM).
    Iff,
    /// Apple QuickDraw PICT: its bitmaps and QuickTime JPEGs.
    Pict,
    /// JPEG XR (ITU-T T.832; JXR, HD Photo, Windows Media Photo).
    JpegXr,
    /// FITS (astronomy): image HDUs, cubes, tile-compressed images.
    Fits,
    /// A FITS file tile-compressed by fpack (`.fz`): read as FITS.
    Fpack,
    /// VICAR (JPL planetary images).
    Vicar,
    /// PDS3 planetary products (an ODL label attached to the data or beside it).
    Pds3,
    /// PDS4 planetary products (an XML label beside the data).
    Pds4,
    /// XISF (PixInsight).
    Xisf,
    /// ASDF (Roman, JWST): n-dimensional arrays in a YAML tree.
    Asdf,
    /// CERN ROOT: histograms and graphs in a directory of keys.
    Root,
    /// CASA image (radio interferometry): a casacore table directory (`.image`,
    /// `.residual`, `.model`, `.pb`, `.mask`).
    Casa,
    /// SER planetary and lunar video ("LUCAM-RECORDER").
    Ser,
    /// PDF, one page per session page (rendered by hayro).
    Pdf,
    /// Adobe Illustrator: its PDF part, or the preview of a PostScript-only file.
    Illustrator,
    /// Encapsulated PostScript, through its TIFF or EPSI preview.
    Eps,
    /// DjVu page or document (bundled or indirect), one page per session page.
    Djvu,
    /// XPS or OpenXPS document, one fixed page per session page.
    Xps,
    /// CGM metafile (ISO/IEC 8632 binary or clear text), one picture per session page.
    Cgm,
    /// TrueType font (`.ttf`): the specimen, drawn by the viewer.
    TrueType,
    /// OpenType font with PostScript (CFF or CFF2) outlines (`.otf`).
    OpenType,
    /// TrueType/OpenType collection (`.ttc`, `.otc`), one face per session page.
    FontCollection,
    /// WOFF and WOFF2 web fonts.
    Woff,
    Woff2,
    /// TeX DVI pages.
    Dvi,
    /// PostScript Type 1 fonts (`.pfb`, `.pfa`).
    Type1Font,
    /// Bitmap fonts (`.pcf`, `.bdf`, `.fon`, `.fnt`, `.psf`), shown as a glyph sheet.
    BitmapFont,
    /// Jupyter notebooks (`.ipynb`): cells and their saved outputs as pages.
    Notebook,
    /// draw.io / diagrams.net diagrams (`.drawio`, `.dio`).
    Drawio,
    /// Excalidraw scenes (`.excalidraw`).
    Excalidraw,
    /// Lottie animations (Bodymovin `.json` by content, `.lottie` packages).
    Lottie,
    /// Flash movies (`.swf`), their first frame.
    Swf,
    /// Tripos MOL2 molecules (`.mol2`).
    Mol2,
    /// GROMACS GRO structures (`.gro`).
    Gro,
    /// Quake II model (`.md2`): its first frame.
    Md2,
    /// MilkShape 3D (`.ms3d`).
    Ms3d,
    /// MagicaVoxel (`.vox`): voxels as cubes.
    Vox,
    /// AC3D (`.ac`).
    Ac3d,
    /// DirectX text model (`.x`).
    DirectX,
    /// BioVision motion capture (`.bvh`): the skeleton in its rest pose.
    Bvh,
    /// EPUB 2 or 3 book, one laid-out page per session page.
    Epub,
    /// HDF5 datasets.
    Hdf5,
    /// NetCDF (classic and NetCDF-4) variables.
    NetCdf,
    /// EMD electron microscopy (Berkeley, Velox), an HDF5 convention.
    Emd,
    /// Gatan DigitalMicrograph DM3 / DM4 (electron microscopy).
    Dm,
    /// MRC / MRCS (electron microscopy and tomography).
    Mrc,
    /// SPIDER images, volumes and stacks (cryo-EM).
    Spider,
    /// DeltaVision (`.dv`, `.r3d`; widefield and structured-illumination light microscopy),
    /// recognised by content (`.dv` is also DV video's).
    DeltaVision,
    /// Gwyddion `.gwy` (scanning probe microscopy).
    Gwyddion,
    /// Igor Pro binary wave `.ibw` (scanning probe microscopy and others).
    Igor,
    /// NumPy array `.npy`.
    Npy,
    /// NumPy archive of arrays `.npz`.
    Npz,
    /// MATLAB `.mat` (levels 4 and 5, and v7.3 over HDF5).
    Matlab,
    /// A plain Zarr store (a folder, a `.zarr.zip`, or a metadata file in one).
    Zarr,
    /// NI TDMS measurement file.
    Tdms,
    /// VTK legacy `.vtk` or XML (`.vti`, `.vtr`, `.vts`, `.vtu`, `.vtp`) dataset.
    Vtk,
    /// Leica LIF project (or a single-image LOF): one page per image.
    Lif,
    /// Nikon NIS-Elements ND2 (chunk-map and legacy JPEG 2000 layouts).
    Nd2,
    /// Olympus/Evident FluoView OIR confocal stack.
    Oir,
    /// Image Cytometry Standard 1.0 / 2.0 (`.ics` header, `.ids` samples).
    Ics,
    /// Flow Cytometry Standard 2.0–3.2 list-mode data: a density plot of two parameters.
    Fcs,
    /// Hamamatsu VMS (JPEG tile files) whole-slide image.
    Vms,
    /// Hamamatsu VMU (uncompressed NGR) whole-slide image.
    Vmu,
    /// Philips iSyntax whole-slide image: recognised and refused (see `isyntax_dec`).
    Isyntax,
    /// Princeton Instruments SPE spectroscopy frames (WinSpec 2.x, LightField 3.0).
    Spe,
    /// Andor SIF spectroscopy frames.
    Sif,
    /// Zeiss CZI, read by libCZI in the decode host.
    Czi,
    /// DXF drawing (AutoCAD Drawing Exchange Format, ASCII and binary).
    Dxf,
    /// HP-GL or HP-GL/2 plot file (also inside a PCL 5 job).
    Hpgl,
    /// GDSII stream file (an IC layout): a page per top cell.
    Gdsii,
    /// One Gerber file (RS-274X, X2/X3): a board layer.
    Gerber,
    /// One Excellon drill file.
    Excellon,
    /// A board's Gerber and drill files together (a folder or a ZIP): its layers, then its
    /// top and bottom in board colours.
    GerberSet,
    /// An ODB++ job (a folder, `.tgz`, `.tar` or `.zip`): its layers, then its top and
    /// bottom in board colours.
    Odb,
    /// An IPC-2581 file (XML, revisions A to C), shown like a board set.
    Ipc2581,
    /// LandXML survey data: a plan of its surfaces and alignments, and profile charts.
    LandXml,
    /// KiCad 6–9 board (`.kicad_pcb`).
    KicadPcb,
    /// KiCad 6–9 schematic (`.kicad_sch`): a page per sheet.
    KicadSch,
    /// JCAMP-DX spectra (IR, UV-Vis, Raman, NMR, MS).
    Jcamp,
    /// Galactic/Thermo SPC spectra.
    Spc,
    /// mzML mass spectrometry run: the TIC, then a page per spectrum.
    Mzml,
    /// mzXML mass spectrometry run.
    Mzxml,
    /// Bruker NMR experiment folder (`fid`/`ser`, `acqus`, `pdata`).
    BrukerNmr,
    /// STL 3D mesh (ASCII or binary).
    Stl,
    /// Wavefront OBJ 3D mesh (with its MTL materials).
    Obj,
    /// PLY 3D mesh (Stanford polygon file).
    Ply,
    /// glTF 2.0 3D scene (JSON).
    Gltf,
    /// glTF 2.0 binary (GLB).
    Glb,
    /// 3D Manufacturing Format.
    ThreeMf,
    /// antimatter15 Gaussian splats (`.splat`).
    Splat,
    /// Niantic compressed Gaussian splats (`.spz`).
    Spz,
    /// mkkellogg's Gaussian splats (`.ksplat`): recognised to be refused.
    Ksplat,
    /// USD stage (`.usd`, `.usda`, `.usdc`).
    Usd,
    /// USDZ package.
    Usdz,
    /// COLLADA document (`.dae`).
    Dae,
    /// COLLADA package (`.zae`).
    Zae,
    /// Autodesk 3D Studio (`.3ds`).
    ThreeDs,
    /// LightWave object (`.lwo`).
    Lwo,
    /// Autodesk FBX (geometry of binary and ASCII FBX 7).
    Fbx,
    /// X3D scene, XML encoding (`.x3d`, `.x3dz`).
    X3d,
    /// VRML 97 or X3D classic encoding (`.wrl`, `.wrz`, `.x3dv`, `.x3dvz`).
    Vrml,
    /// Rhino 3D model (`.3dm`): its meshes and saved render meshes, else its preview.
    Rhino,
    /// Additive Manufacturing File Format (`.amf`).
    Amf,
    /// Geomview OFF mesh (`.off`).
    Off,
    /// USGS DEM elevation model (`.dem`).
    UsgsDem,
    /// Alembic archives (Ogawa).
    Alembic,
    /// Blender file (`.blend`), by its embedded thumbnail.
    Blend,
    /// SketchUp model (`.skp`), by its embedded preview.
    Sketchup,
    /// IFC building model, recognised to be refused; also the registry entry of the STEP and
    /// IGES files it hands to OpenCASCADE.
    CadExchange,
    /// STEP CAD model (AP203, AP214, AP242), read by OpenCASCADE in the decode host.
    Step,
    /// IGES CAD model, read by OpenCASCADE in the decode host.
    Iges,
    /// IFC building model (`.ifc`, `.ifczip`), read by Specular's own IFC reader.
    Ifc,
    /// AutoCAD drawing (`.dwg`), through the ODA File Converter when it is installed and
    /// not switched off (an optional decoder, [`optional`]).
    Dwg,
    /// Blackmagic RAW clip (`.braw`), its first frame through the Blackmagic RAW SDK the
    /// user installed (an optional decoder, [`optional`]).
    Braw,
    /// Canon Cinema RAW Light clip (`.crm`): CRX sensor frames, through Specular's own reader.
    CinemaRawLight,
    /// Apple ProRes RAW movie (QuickTime `aprn`, `aprh`): Bayer frames, own decoder.
    ProResRaw,
    /// MNG animation (embedded PNG and JNG layers).
    Mng,
    /// JNG: JPEG colour with PNG or JPEG alpha.
    Jng,
    /// Windows Metafile (WMF, placeable or not).
    Wmf,
    /// Enhanced Metafile (EMF, EMF+).
    Emf,
    /// Procreate document (its QuickLook thumbnail).
    Procreate,
    /// Paint.NET document (`.pdn`): layers composited, the header thumbnail as preview.
    Pdn,
    /// CorelDRAW drawing (`.cdr`), by its embedded previews.
    Cdr,
    /// Clip Studio Paint illustration (`.clip`): its merged canvas.
    Clip,
    /// Visio drawing: VSDX packages drawn as pages, binary VSD by its embedded preview.
    Visio,
    /// WordPerfect Graphics (`.wpg`, versions 1 and 2).
    Wpg,
    /// Sketch document (`.sketch`, version 43 and later): artboards drawn as pages.
    Sketch,
    /// Figma local copy (`.fig`): its thumbnail.
    Figma,
    /// OpenDocument: drawings (`.odg`) drawn as pages; presentations, spreadsheets and text
    /// documents by their thumbnail.
    OpenDocument,
    /// Office Open XML (Word, Excel, PowerPoint 2007 and later): the package thumbnail only.
    Ooxml,
    /// Word, Excel and PowerPoint 97–2003 compound files: the summary-information thumbnail.
    MsOffice,
    /// Apple Pages, Numbers and Keynote packages: their preview image.
    Iwork,
    /// Playlists (M3U/M3U8, PLS, XSPF): no picture; the app plays them as its queue.
    Playlist,
    /// BPG (HEVC stills and animations).
    Bpg,
    /// PaintShop Pro image (`.psp`, `.pspimage`, `.tub`…).
    Psp,
    /// FLIF (Free Lossless Image Format), stills and animations.
    Flif,
    /// Affinity Photo, Designer or Publisher document (its embedded preview).
    Affinity,
    /// NIfTI-1 and NIfTI-2 (`.nii`, `.nii.gz`, `.hdr` / `.img` pairs).
    Nifti,
    /// Analyze 7.5 (`.hdr` / `.img`).
    Analyze,
    /// NRRD (`.nrrd`, `.nhdr`).
    Nrrd,
    /// MetaImage (`.mha`, `.mhd`).
    MetaImage,
    /// Aperio SVS whole-slide image.
    Svs,
    /// Hamamatsu NDPI whole-slide image.
    Ndpi,
    /// Leica SCN whole-slide image.
    Scn,
    /// OME-TIFF microscopy image (`.ome.tif`, `.ome.tiff`).
    OmeTiff,
    /// OME-Zarr microscopy image (a `.ome.zarr` folder or a zipped one).
    OmeZarr,
    /// Tiled pyramidal (Big)TIFF of no particular slide convention.
    PyramidTiff,
    /// DICOM VL Whole Slide Microscopy: a series of instances as one pyramid.
    DicomWsi,
    /// 3DHISTECH MIRAX whole-slide image (`.mrxs` with its folder).
    Mrxs,
    /// Roche/Ventana BIF whole-slide image (BigTIFF with iScan metadata).
    Bif,
    /// Olympus/Evident cellSens VSI (with its `.ets` frame files).
    Vsi,
    /// ASPRS LAS point cloud (1.0–1.4).
    Las,
    /// LASzip-compressed LAS point cloud (and COPC).
    Laz,
    /// Point Cloud Library PCD point cloud.
    Pcd,
    /// XYZ point cloud (whitespace- or comma-separated columns).
    Xyz,
    /// Leica PTS point cloud.
    Pts,
    /// ASTM E57 3D imaging data (point cloud scans).
    E57,
    /// Leica PTX point cloud scans.
    Ptx,
    /// GeoJSON vector map.
    GeoJson,
    /// Esri Shapefile (`.shp` with its `.shx`, `.dbf`, `.prj`, `.cpg`).
    Shapefile,
    /// OGC GeoPackage: vector features, or a tile pyramid.
    GeoPackage,
    /// KML (Google Earth): placemarks and paths.
    Kml,
    /// KMZ: KML in a ZIP.
    Kmz,
    /// GPX: GPS waypoints, routes and tracks.
    Gpx,
    /// MBTiles: an SQLite database of web map tiles (raster or vector).
    MbTiles,
    /// PMTiles v3: a single-file web map tile pyramid (raster or vector).
    PmTiles,
    /// FlatGeobuf: vector features with a packed R-tree.
    FlatGeobuf,
    /// GeoParquet (and GeoArrow in Parquet): columnar vector features.
    GeoParquet,
    /// GeoArrow in Arrow IPC files and streams (`.arrow`, `.feather`, `.arrows`).
    GeoArrow,
    /// GML simple features (GML 2, 3.1.1, 3.2; WFS responses).
    Gml,
    /// OpenStreetMap XML (`.osm`).
    OsmXml,
    /// OpenStreetMap PBF (`.osm.pbf`).
    OsmPbf,
    /// Garmin FIT activity tracks.
    Fit,
    /// Garmin Training Center XML activities and courses.
    Tcx,
    /// Esri File Geodatabase: a `.gdb` folder of tables.
    FileGdb,
    /// MapInfo TAB (native `.tab` with `.map`, `.id`, `.dat`).
    MapInfoTab,
    /// MapInfo Interchange Format (`.mif` with `.mid`).
    MapInfoMif,
    /// S-57 electronic navigational chart cells (`.000`).
    S57,
    /// TopoJSON topologies.
    TopoJson,
    /// IGC glider and paraglider flight logs.
    Igc,
    /// NMEA 0183 GPS logs.
    Nmea,
    /// BSB/KAP raster nautical charts (`.kap`, and the `.bsb` chart description).
    BsbChart,
    /// ER Mapper rasters (`.ers` header beside the raw data).
    ErMapper,
    /// SAGA GIS grids (`.sgrd` header and `.sdat` data, or `.sg-grd-z`).
    SagaGrid,
    /// Arc/Info binary grids (a folder of `.adf` files).
    ArcInfoGrid,
    /// GDAL virtual rasters (`.vrt`) with simple and complex sources.
    GdalVrt,
    /// NITF 2.1 / NSIF 1.0 / NITF 2.0 imagery.
    Nitf,
    /// ECW (Hexagon): recognised and refused (see `vendor_gis_dec`).
    Ecw,
    /// MrSID (Extensis): recognised and refused (see `vendor_gis_dec`).
    MrSid,
    /// FreeSurfer MGH (`.mgh`, `.mgz`).
    Mgh,
    /// MINC1 and MINC2 (`.mnc`).
    Minc,
    /// NeXus (neutron, X-ray and muon data), an HDF5 convention.
    Nexus,
    /// Imaris (`.ims`) microscopy volumes, an HDF5 convention.
    Imaris,
    /// BAG bathymetry grids, an HDF5 convention.
    Bag,
    /// SRTM elevation tiles (`.hgt`).
    Srtm,
    /// Gaussian cube volumetric data (orbitals, densities).
    Cube,
    /// Crystallographic Binary File / miniCBF (Pilatus, Eiger) diffraction frames.
    Cbf,
    /// ESRF Data Format (`.edf`) detector frames.
    Edf,
    /// SMV (ADSC, Rigaku d*TREK) diffraction frames (`.img`).
    Smv,
    /// Bruker diffraction frames (`.sfrm`, numbered `.0001`).
    Bruker,
    /// MarCCD diffraction frames (`.mccd`, a TIFF with the Mar header).
    MarCcd,
    /// Rigaku R-AXIS image plate frames (`.img`, `.osc`).
    Raxis,
    /// Esri ASCII grids (`.asc`, recognised by content).
    EsriAscii,
    /// DTED elevation tiles (`.dt0`, `.dt1`, `.dt2`).
    Dted,
    /// ENVI rasters: raw data with an ENVI `.hdr` header.
    Envi,
    /// Sentinel-2 L1C and L2A products (a `.SAFE` folder, its metadata or a ZIP of it).
    Sentinel2,
    /// STAC items, item collections, collections and catalogs (JSON) by their GeoTIFF
    /// assets.
    Stac,
    /// Cesium 3D Tiles 1.0 and 1.1 tilesets (`tileset.json`, or its folder), shown in 3D with
    /// level of detail ([`Session::tiles3d`]).
    Tiles3d,
    /// ESRI `.bil`/`.bsq`/`.bip` rasters with an ESRI `.hdr` label.
    EsriBil,
    /// ERDAS IMAGINE (`.img`, recognised by content).
    ErdasImagine,
    /// GRIB editions 1 and 2 (meteorological and oceanographic fields).
    Grib,
    /// HDF4 and HDF-EOS2 (scientific datasets and raster images; MODIS and other EOS
    /// products).
    Hdf4,
    /// FEI / Thermo Fisher TIA series (`.ser` with its `.emi`).
    Tia,
    /// Bruker Esprit hypermaps (`.bcf`).
    Bcf,
    /// EDAX / TSL OIM EBSD maps (`.ang`).
    Ang,
    /// Oxford Instruments HKL / Channel 5 EBSD maps (`.ctf`).
    Ctf,
    /// Bruker (Digital Instruments) Nanoscope AFM files.
    Nanoscope,
    /// Nanonis scanning probe images (`.sxm`).
    Nanonis,
    /// JPK Instruments AFM images (`.jpk`, TIFF based).
    Jpk,
    /// NT-MDT scanning probe files (`.mdt`).
    NtMdt,
    /// ISO 25178-72 XML 3D surface profiles (`.x3p`).
    X3p,
    /// Zygo MetroPro interferometer data (`.dat`).
    MetroPro,
    /// Zygo Mx data (`.datx`, HDF5 based).
    Datx,
    /// Keyence VK laser scanning microscope data (`.vk4`).
    Vk4,
    /// Sensofar PLu surfaces (`.plu`).
    Plu,
    /// Alicona AL3D surfaces (`.al3d`).
    Al3d,
    /// Digital Surf studiables (`.sur`).
    Sur,
    /// SEG-Y seismic data (rev 0, 1, 2).
    Segy,
    /// Seismic Unix traces (`.su`).
    SeismicUnix,
    /// Zeiss Xradia reconstructions and projections (`.txm`, `.txrm`, `.xrm`).
    Txm,
    /// Amira / Avizo lattices (`.am`).
    Amira,
    /// A Bruker SkyScan reconstruction: its `.log` and the slice images beside it.
    SkyScan,
    /// Nikon X-Tek CT volumes (`.xtekct`, `.xtekVolume` beside a `.vol`).
    Xtek,
    /// Volume Graphics VGStudio volumes (`.vgi` beside a `.vol` / `.raw`).
    Vgi,
    /// A headerless volume (`.raw`, `.vol`) read with a layout the user gave.
    RawVolume,
    /// A Zeiss Cirrus OCT raw export (`.img`, recognised by its name and size).
    Cirrus,
    /// A Philips PAR/REC MR volume (`.PAR` header, `.REC` voxels).
    ParRec,
    /// An ECAT 7 PET volume (`.v`, recognised by content).
    Ecat,
    /// imzML imaging mass spectrometry (`.imzML` with its `.ibd`).
    Imzml,
    /// HP polynomial texture map (`.ptm`, PTM 1.1 / 1.2).
    Ptm,
    /// Cultural Heritage Imaging hemispherical-harmonics RTI (`.rti`, old `.hsh`).
    Rti,
    /// CNR-ISTI Relight image: a folder of JPEG coefficient planes with its `info.json`.
    Relight,
    /// Protein Data Bank coordinates (`.pdb`, `.ent`).
    Pdb,
    /// PDBx/mmCIF macromolecular structure.
    Mmcif,
    /// Small-molecule crystal structure (IUCr core CIF).
    Cif,
    /// MDL SDF / MOL file (V2000 or V3000).
    Sdf,
    /// XYZ file of a molecule (elements and coordinates); point-cloud XYZ is [`Format::Xyz`].
    MolXyz,
    /// A FLIR radiometric JPEG (R-JPEG): a JPEG with the camera's FFF in APP1 segments.
    FlirJpeg,
    /// A standalone FLIR FFF thermal image (`.fff`).
    FlirFff,
    /// A FLIR thermal sequence: `.seq` (FFF frames, raw counts) or `.csq` (JPEG-LS counts).
    FlirSeq,
    /// An InfiRay thermal image (`.irg`).
    InfirayIrg,
    /// An InfraTec thermal image (`.irb`).
    InfratecIrb,
    /// A Testo thermal image (`.bmt`, testo 865–883; the older layout is refused).
    TestoBmt,
    /// A Fluke thermal image (`.is2`, the ZIP layout; the older layout is refused).
    FlukeIs2,
    /// A GIFTI file (`.surf.gii`, `.func.gii`, `.shape.gii`, `.label.gii`): a cortical
    /// surface or values per vertex of one.
    Gifti,
    /// A CIFTI-2 file (`.dscalar.nii`, `.dlabel.nii`, `.dtseries.nii`, `.pscalar.nii`…): maps
    /// over surface vertices and subcortical voxels in a NIfTI-2 file.
    Cifti,
    /// A TrackVis tractogram (`.trk`).
    Trk,
    /// An MRtrix tractogram (`.tck`).
    Tck,
    /// An MRtrix image (`.mif`, `.mif.gz`, `.mih`).
    Mif,
    /// An image on an OMERO.web server, named by a `.omero` descriptor (rendered tiles).
    Omero,
    /// An MPEG-4 movie (`.mp4`, `.m4v`): opened by the application's registered video
    /// opener ([`set_video_opener`]).
    Mp4,
    /// A QuickTime movie (`.mov`, `.qt`).
    QuickTime,
    /// A 3GPP or 3GPP2 movie (`.3gp`, `.3g2`).
    ThreeGpp,
    /// A Matroska movie (`.mkv`, `.mk3d`; `.mka` sound only, which is refused).
    Matroska,
    /// A WebM movie (`.webm`).
    WebM,
    /// An MPEG transport stream: AVCHD and Blu-ray (`.mts`, `.m2ts`), broadcast and HDV
    /// (`.ts`, `.m2t`), JVC camcorders (`.tod`).
    MpegTs,
    /// An MPEG programme stream (MPEG-1 system or MPEG-2 programme stream): `.mpg`,
    /// `.mpeg`, DVD `.vob`, camcorder `.mod`, DVD-VR `.vro`, `.m2p`.
    MpegPs,
    /// A bare MPEG-1/2 video stream (`.m1v`, `.m2v`, `.mpv`).
    MpegVideo,
    /// An AVI (RIFF) movie, with OpenDML (`.avi`, `.divx`).
    Avi,
    /// An Ogg movie: Theora with Vorbis, Opus or FLAC (`.ogv`, `.ogg`).
    Ogg,
    /// An IVF file: raw VP8, VP9 or AV1 (`.ivf`).
    Ivf,
    /// A raw H.264 or HEVC (Annex B) or AV1 (OBU) elementary stream (`.h264`, `.hevc`,
    /// `.obu`…), without a container.
    VideoStream,
    /// A raw APV file (`.apv`): access units with their sizes.
    Apv,
    /// An MXF movie (`.mxf`): broadcast, post-production and camera files (OP1a, OP-Atom).
    Mxf,
    /// A Flash Video movie (`.flv`).
    Flv,
    /// An ASF movie: Windows Media (`.wmv`, `.asf`).
    Asf,
    /// A Windows Media Center TV recording (`.wtv`).
    Wtv,
    /// Raw DV (`.dv`, `.dif`): camcorder tapes captured over FireWire.
    Dv,
    /// MPEG audio: MP3, MP2, MP1 (`.mp3`, `.mp2`): a sound file, opened by the video opener.
    Mp3,
    /// AAC in ADTS frames (`.aac`).
    Aac,
    /// Native FLAC (`.flac`).
    Flac,
    /// WAVE and RF64 (`.wav`).
    Wave,
    /// AIFF and AIFF-C (`.aiff`, `.aif`, `.aifc`).
    Aiff,
    /// Raw DTS (`.dts`).
    Dts,
    /// Raw AC-3 and E-AC-3 (`.ac3`, `.eac3`).
    Ac3,
    /// AMR and AMR-WB speech in the storage format (`.amr`, `.awb`).
    Amr,
    /// WavPack (`.wv`; a hybrid file's `.wvc` correction file is read beside it).
    WavPack,
    /// Monkey's Audio (`.ape`).
    Ape,
    /// Musepack, stream versions 7 and 8 (`.mpc`, `.mp+`, `.mpp`).
    Mpc,
    /// Sun/NeXT AU (`.au`, `.snd`).
    Au,
    /// Apple Core Audio Format (`.caf`).
    Caf,
    /// Sony Wave64 (`.w64`).
    W64,
    /// Creative Voice (`.voc`).
    Voc,
    /// Amiga 8SVX and 16SV (`.8svx`, `.16sv`, `.svx`).
    Svx,
    /// Audio Visual Research (`.avr`).
    Avr,
    /// NIST SPHERE (`.sph`, `.nist`).
    Sphere,
    /// Berkeley/IRCAM/CARL sound (`.sf`, `.ircam`).
    Ircam,
    /// TTA, True Audio (`.tta`).
    Tta,
    /// Shorten (`.shn`).
    Shorten,
    /// DSD: Sony DSF (`.dsf`).
    Dsf,
    /// DSD: Philips DSDIFF (`.dff`).
    Dff,
    /// Tracker modules: MOD, S3M, XM, IT, 669, MTM, STM, OKT, MED, ULT, FAR (by content
    /// only).
    Tracker,
    /// YUV4MPEG2 uncompressed video (`.y4m`).
    Y4m,
    /// CD+G karaoke graphics (`.cdg`).
    Cdg,
    /// PlayStation movies (`.str`): MDEC pictures and XA sound in CD-ROM sectors.
    PsxStr,
    /// Ogg sound: Vorbis, Opus, FLAC (`.oga`, `.opus`, `.ogg`).
    OggAudio,
    /// MPEG-4 sound (`.m4a`, `.m4b` audiobooks).
    M4a,
    /// GSM 06.10 full-rate speech (`.gsm`; GSM in WAVE is [`Format::Wave`]).
    Gsm,
    /// QCP speech (`.qcp`, RFC 3625): QCELP-13 (PureVoice) or EVRC, from CDMA phones.
    Qcp,
    /// QOA, "Quite OK Audio" (`.qoa`).
    Qoa,
    /// Headerless signed 16-bit 8 kHz mono (`.sln`, Asterisk's "signed linear").
    Sln,
    /// HTK speech files (`.htk`): waveforms (features are refused in words).
    Htk,
    /// FastTracker 2 instruments (`.xi`).
    Xi,
    /// SoundFont 2 sample banks (`.sf2`).
    SoundFont,
    /// SFZ instruments (`.sfz`): text over sample files beside it.
    Sfz,
    /// Downloadable Sounds banks (`.dls`).
    Dls,
    /// Standard MIDI Files and RIFF MIDI (`.mid`, `.midi`, `.kar`, `.rmi`).
    Midi,
    /// Mobipocket and Kindle books (`.mobi`, `.prc`, `.azw`, `.azw3`) and PalmDOC: shown as
    /// EPUB pages.
    Mobi,
    /// FictionBook 2 (`.fb2`, `.fb2.zip`): shown as EPUB pages.
    Fb2,
    /// Plain text and logs (`.txt`, `.text`, `.log`), any common character set.
    PlainText,
    /// DOS art and release notes (`.nfo`, `.diz`), in code page 437 unless UTF-8.
    Nfo,
    /// Markdown (`.md`): its source in the viewer, rendered as pages.
    Markdown,
    /// Comma-separated values (also semicolon or bar), a table.
    Csv,
    /// Tab-separated values, a table.
    Tsv,
    /// Rich Text Format, rendered as pages.
    Rtf,
    /// SubRip subtitles (`.srt`).
    SubRip,
    /// WebVTT subtitles (`.vtt`).
    WebVtt,
    /// SubStation Alpha and Advanced SubStation subtitles (`.ssa`, `.ass`).
    Ssa,
    /// MicroDVD subtitles (`.sub`, timed in frames).
    MicroDvd,
    /// DVD VobSub picture subtitles (`.idx` with its `.sub`).
    VobSub,
    /// Blu-ray PGS picture subtitles (`.sup`).
    Pgs,
    /// LRC timed lyrics (`.lrc`).
    Lrc,
    /// TTML / DFXP timed text (`.ttml`, `.dfxp`).
    Ttml,
    /// Scenarist closed captions (`.scc`, CEA-608).
    Scc,
}

impl Format {
    /// Whether the format is one of the text-like files of `text_dec` (text, tables,
    /// Markdown, RTF, subtitles).
    pub fn is_text(self) -> bool {
        matches!(
            self,
            Format::PlainText
                | Format::Nfo
                | Format::Markdown
                | Format::Csv
                | Format::Tsv
                | Format::Rtf
                | Format::Notebook
                | Format::SubRip
                | Format::WebVtt
                | Format::Ssa
                | Format::MicroDvd
                | Format::VobSub
                | Format::Pgs
                | Format::Lrc
                | Format::Ttml
                | Format::Scc
        )
    }

    /// Whether the format is a 3D mesh ([`Session::mesh`]).
    pub fn is_mesh(self) -> bool {
        matches!(
            self,
            Format::Stl
                | Format::Obj
                | Format::Ply
                | Format::Gltf
                | Format::Glb
                | Format::ThreeMf
                | Format::Usd
                | Format::Usdz
                | Format::Dae
                | Format::Zae
                | Format::ThreeDs
                | Format::Lwo
                | Format::Fbx
                | Format::X3d
                | Format::Vrml
                | Format::Rhino
                | Format::Amf
                | Format::Off
                | Format::Md2
                | Format::Ms3d
                | Format::Vox
                | Format::Ac3d
                | Format::DirectX
                | Format::Bvh
                | Format::Alembic
                | Format::Step
                | Format::Iges
                | Format::Ifc
                | Format::Tiles3d
        )
    }

    /// Whether the format is Gaussian splats ([`Session::splats`]; 3DGS PLY files are too).
    pub fn is_splats(self) -> bool {
        matches!(self, Format::Splat | Format::Spz)
    }

    /// Whether the format is a point cloud ([`Session::points`]; PLY files without faces
    /// are too).
    pub fn is_points(self) -> bool {
        matches!(
            self,
            Format::Las
                | Format::Laz
                | Format::Pcd
                | Format::Xyz
                | Format::Pts
                | Format::E57
                | Format::Ptx
        )
    }

    /// Whether the format is a tractogram ([`Session::tracts`]).
    pub fn is_tracts(self) -> bool {
        matches!(self, Format::Trk | Format::Tck)
    }

    /// Whether the format is a molecular structure ([`Session::molecule`]).
    pub fn is_molecule(self) -> bool {
        matches!(
            self,
            Format::Pdb
                | Format::Mmcif
                | Format::Cif
                | Format::Sdf
                | Format::MolXyz
                | Format::Mol2
                | Format::Gro
        )
    }

    /// Whether the viewer draws the picture from what the file describes (a page, a vector
    /// drawing, a board, a model, a font, a plot of data, vector map features) rather than
    /// showing samples stored in it: such a file has no camera, and the drawn picture's pixel
    /// size and sample coding are not facts of the file. Includes the drawn formats shown
    /// through an embedded preview.
    pub fn is_drawn(self) -> bool {
        self.is_mesh()
            || self.is_splats()
            || self.is_points()
            || self.is_tracts()
            || self.is_molecule()
            || matches!(
                self,
                Format::Svg
                    | Format::Pdf
                    | Format::Illustrator
                    | Format::Eps
                    | Format::Xps
                    | Format::Cgm
                    | Format::Wmf
                    | Format::Emf
                    | Format::Wpg
                    | Format::Cdr
                    | Format::Visio
                    | Format::Sketch
                    | Format::Figma
                    | Format::Affinity
                    | Format::OpenDocument
                    | Format::Ooxml
                    | Format::MsOffice
                    | Format::Iwork
                    | Format::Playlist
                    | Format::TrueType
                    | Format::OpenType
                    | Format::FontCollection
                    | Format::Woff
                    | Format::Woff2
                    | Format::Type1Font
                    | Format::Drawio
                    | Format::Excalidraw
                    | Format::Swf
                    | Format::Dvi
                    | Format::Epub
                    | Format::Mobi
                    | Format::Fb2
                    | Format::Dxf
                    | Format::Dwg
                    | Format::CadExchange
                    | Format::Hpgl
                    | Format::Gdsii
                    | Format::Gerber
                    | Format::Excellon
                    | Format::GerberSet
                    | Format::Odb
                    | Format::Ipc2581
                    | Format::LandXml
                    | Format::KicadPcb
                    | Format::KicadSch
                    | Format::Blend
                    | Format::Sketchup
                    | Format::Jcamp
                    | Format::Spc
                    | Format::Mzml
                    | Format::Mzxml
                    | Format::BrukerNmr
                    | Format::Fcs
                    | Format::Root
                    | Format::Tdms
                    | Format::GeoJson
                    | Format::Shapefile
                    | Format::Kml
                    | Format::Kmz
                    | Format::Gpx
                    | Format::FlatGeobuf
                    | Format::GeoParquet
                    | Format::GeoArrow
                    | Format::Gml
                    | Format::OsmXml
                    | Format::OsmPbf
                    | Format::Fit
                    | Format::Tcx
                    | Format::FileGdb
                    | Format::MapInfoTab
                    | Format::MapInfoMif
                    | Format::S57
                    | Format::TopoJson
                    | Format::Igc
                    | Format::Nmea
            )
            || self.is_text()
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Jpeg => "JPEG",
            Format::Png => "PNG",
            Format::Gif => "GIF",
            Format::WebP => "WebP",
            Format::Avif => "AVIF",
            Format::Jxl => "JPEG XL",
            Format::Jpeg2000 => "JPEG 2000",
            Format::Heic => "HEIC",
            Format::Tiff => "TIFF",
            Format::GeoTiff => "GeoTIFF",
            Format::Bmp => "BMP",
            Format::Ico => "ICO",
            Format::Tga => "TGA",
            Format::Qoi => "QOI",
            Format::Pnm => "PNM",
            Format::Dds => "DDS",
            Format::Ktx2 => "KTX2",
            Format::Ktx => "KTX",
            Format::Pvr => "PVR",
            Format::Vtf => "VTF",
            Format::Astc => "ASTC",
            Format::Basis => "Basis Universal",
            Format::Svg => "SVG",
            Format::Exr => "OpenEXR",
            Format::Hdr => "Radiance HDR",
            Format::Pfm => "PFM",
            Format::Raw => "Camera RAW",
            Format::Psd => "Photoshop",
            Format::Xcf => "GIMP XCF",
            Format::Pcx => "PCX",
            Format::Dcx => "DCX",
            Format::Sgi => "SGI",
            Format::SunRaster => "Sun Raster",
            Format::Xbm => "XBM",
            Format::Xpm => "XPM",
            Format::Wbmp => "WBMP",
            Format::CpcScreen => "Amstrad CPC screen",
            Format::MsxScreen => "MSX screen",
            Format::AppleHgr => "Apple II hi-res",
            Format::AppleDhr => "Apple II double hi-res",
            Format::BbcScreen => "BBC Micro screen",
            Format::AtariMic => "Atari Micro Painter",
            Format::AtariGr8 => "Atari Graphics 8",
            Format::AtariGr9 => "Atari Graphics 9",
            Format::PhotoCd => "Kodak Photo CD",
            Format::Farbfeld => "farbfeld",
            Format::Fli => "FLI/FLC",
            Format::Sixel => "Sixel",
            Format::Xcursor => "X cursor",
            Format::Qtif => "QuickTime image",
            Format::Avs => "AVS X image",
            Format::MacPaint => "MacPaint",
            Format::GemImg => "GEM IMG",
            Format::Degas => "DEGAS",
            Format::Neochrome => "NEOchrome",
            Format::Koala => "Koala Painter",
            Format::ZxScreen => "ZX Spectrum screen",
            Format::DrHalo => "Dr. Halo CUT",
            Format::SoftimagePic => "Softimage PIC",
            Format::AliasPix => "Alias PIX",
            Format::Rla => "Wavefront RLA",
            Format::UtahRle => "Utah RLE",
            Format::Wal => "Quake 2 WAL",
            Format::Blp => "Blizzard BLP",
            Format::Dpx => "DPX",
            Format::Cineon => "Cineon",
            Format::Xwd => "XWD",
            Format::Miff => "MIFF",
            Format::Tim => "PlayStation TIM",
            Format::GimpPalette => "GIMP palette",
            Format::PhotoshopSwatches => "Photoshop swatches",
            Format::AdobeColorTable => "Adobe colour table",
            Format::SwatchExchange => "Adobe Swatch Exchange",
            Format::Aseprite => "Aseprite",
            Format::GimpGradient => "GIMP gradient",
            Format::GimpBrush => "GIMP brush",
            Format::GimpBrushPipe => "GIMP animated brush",
            Format::GimpPattern => "GIMP pattern",
            Format::MsPaint => "Microsoft Paint",
            Format::Gd => "GD",
            Format::Gd2 => "GD2",
            Format::PixarPicture => "Pixar picture",
            Format::LabEyeIm => "LabEye IM",
            Format::ImTools => "IM Tools",
            Format::XvThumbnail => "XV thumbnail",
            Format::McIdasArea => "McIDAS AREA",
            Format::Pgx => "PGX",
            Format::Cals => "CALS raster",
            Format::Pkm => "PKM",
            Format::Ftex => "Firaxis texture",
            Format::Wsq => "WSQ",
            Format::Jbig => "JBIG",
            Format::Jbig2 => "JBIG2",
            Format::JpegXs => "JPEG XS",
            Format::Dicom => "DICOM",
            Format::JpegLs => "JPEG-LS",
            Format::Fpx => "FlashPix",
            Format::Icns => "ICNS",
            Format::Ora => "OpenRaster",
            Format::Kra => "Krita",
            Format::Iff => "IFF ILBM",
            Format::Pict => "PICT",
            Format::JpegXr => "JPEG XR",
            Format::Fits => "FITS",
            Format::Fpack => "FITS (fpack)",
            Format::Vicar => "VICAR",
            Format::Pds3 => "PDS3",
            Format::Pds4 => "PDS4",
            Format::Xisf => "XISF",
            Format::Asdf => "ASDF",
            Format::Root => "ROOT",
            Format::Casa => "CASA image",
            Format::Ser => "SER",
            Format::Pdf => "PDF",
            Format::Illustrator => "Adobe Illustrator",
            Format::Eps => "EPS",
            Format::Djvu => "DjVu",
            Format::Xps => "XPS",
            Format::Cgm => "CGM",
            Format::TrueType => "TrueType font",
            Format::OpenType => "OpenType font",
            Format::FontCollection => "Font collection",
            Format::Woff => "WOFF font",
            Format::Woff2 => "WOFF2 font",
            Format::Type1Font => "Type 1 font",
            Format::BitmapFont => "bitmap font",
            Format::Notebook => "Jupyter notebook",
            Format::Drawio => "draw.io",
            Format::Excalidraw => "Excalidraw",
            Format::Lottie => "Lottie",
            Format::Swf => "Flash",
            Format::Mol2 => "MOL2",
            Format::Gro => "GROMACS GRO",
            Format::Dvi => "DVI",
            Format::Epub => "EPUB",
            Format::Hdf5 => "HDF5",
            Format::NetCdf => "NetCDF",
            Format::Emd => "EMD",
            Format::Dm => "DigitalMicrograph",
            Format::Mrc => "MRC",
            Format::Spider => "SPIDER",
            Format::DeltaVision => "DeltaVision",
            Format::Gwyddion => "Gwyddion",
            Format::Igor => "Igor binary wave",
            Format::Npy => "NumPy array",
            Format::Npz => "NumPy archive",
            Format::Matlab => "MATLAB",
            Format::Zarr => "Zarr",
            Format::Tdms => "NI TDMS",
            Format::Vtk => "VTK",
            Format::Lif => "Leica LIF",
            Format::Nd2 => "Nikon ND2",
            Format::Oir => "Olympus OIR",
            Format::Ics => "ICS",
            Format::Fcs => "FCS",
            Format::Vms => "Hamamatsu VMS",
            Format::Vmu => "Hamamatsu VMU",
            Format::Isyntax => "Philips iSyntax",
            Format::Spe => "Princeton Instruments SPE",
            Format::Sif => "Andor SIF",
            Format::Czi => "Zeiss CZI",
            Format::Dxf => "DXF",
            Format::Hpgl => "HP-GL",
            Format::Gdsii => "GDSII",
            Format::Gerber => "Gerber",
            Format::Excellon => "Excellon",
            Format::GerberSet => "Gerber board",
            Format::Odb => "ODB++",
            Format::Ipc2581 => "IPC-2581",
            Format::LandXml => "LandXML",
            Format::KicadPcb => "KiCad board",
            Format::KicadSch => "KiCad schematic",
            Format::Jcamp => "JCAMP-DX",
            Format::Spc => "SPC",
            Format::Mzml => "mzML",
            Format::Mzxml => "mzXML",
            Format::BrukerNmr => "Bruker NMR",
            Format::Stl => "STL",
            Format::Obj => "OBJ",
            Format::Ply => "PLY",
            Format::Gltf => "glTF",
            Format::Glb => "glTF binary",
            Format::ThreeMf => "3MF",
            Format::Splat => "Gaussian splats (SPLAT)",
            Format::Spz => "Gaussian splats (SPZ)",
            Format::Ksplat => "Gaussian splats (KSPLAT)",
            Format::Usd => "USD",
            Format::Usdz => "USDZ",
            Format::Dae => "COLLADA",
            Format::Zae => "COLLADA package",
            Format::ThreeDs => "3DS",
            Format::Lwo => "LightWave",
            Format::Fbx => "FBX",
            Format::X3d => "X3D",
            Format::Vrml => "VRML",
            Format::Rhino => "Rhino",
            Format::Amf => "AMF",
            Format::Off => "OFF",
            Format::Md2 => "MD2",
            Format::Ms3d => "MilkShape 3D",
            Format::Vox => "MagicaVoxel",
            Format::Ac3d => "AC3D",
            Format::DirectX => "DirectX",
            Format::Bvh => "BVH",
            Format::UsgsDem => "USGS DEM",
            Format::Alembic => "Alembic",
            Format::Blend => "Blender",
            Format::Sketchup => "SketchUp",
            Format::CadExchange => "CAD exchange (STEP, IGES, IFC)",
            Format::Step => "STEP",
            Format::Iges => "IGES",
            Format::Ifc => "IFC",
            Format::Dwg => "DWG",
            Format::Braw => "Blackmagic RAW",
            Format::CinemaRawLight => "Canon Cinema RAW Light",
            Format::ProResRaw => "Apple ProRes RAW",
            Format::Mng => "MNG",
            Format::Jng => "JNG",
            Format::Wmf => "WMF",
            Format::Emf => "EMF",
            Format::Procreate => "Procreate",
            Format::Pdn => "Paint.NET",
            Format::Cdr => "CorelDRAW",
            Format::Clip => "Clip Studio Paint",
            Format::Visio => "Visio",
            Format::Wpg => "WordPerfect Graphics",
            Format::Sketch => "Sketch",
            Format::Figma => "Figma",
            Format::OpenDocument => "OpenDocument",
            Format::Ooxml => "Office Open XML",
            Format::MsOffice => "Microsoft Office 97–2003",
            Format::Iwork => "Apple iWork",
            Format::Playlist => "Playlist",
            Format::Bpg => "BPG",
            Format::Psp => "PaintShop Pro",
            Format::Flif => "FLIF",
            Format::Affinity => "Affinity",
            Format::Nifti => "NIfTI",
            Format::Analyze => "Analyze 7.5",
            Format::Nrrd => "NRRD",
            Format::MetaImage => "MetaImage",
            Format::Svs => "Aperio SVS",
            Format::Ndpi => "Hamamatsu NDPI",
            Format::Scn => "Leica SCN",
            Format::OmeTiff => "OME-TIFF",
            Format::OmeZarr => "OME-Zarr",
            Format::PyramidTiff => "Pyramidal TIFF",
            Format::DicomWsi => "DICOM WSI",
            Format::Mrxs => "3DHISTECH MIRAX",
            Format::Bif => "Ventana BIF",
            Format::Vsi => "Olympus VSI",
            Format::Las => "LAS",
            Format::Laz => "LAZ",
            Format::Pcd => "PCD",
            Format::Xyz => "XYZ",
            Format::Pts => "PTS",
            Format::E57 => "E57",
            Format::Ptx => "PTX",
            Format::GeoJson => "GeoJSON",
            Format::Shapefile => "Shapefile",
            Format::GeoPackage => "GeoPackage",
            Format::Kml => "KML",
            Format::Kmz => "KMZ",
            Format::Gpx => "GPX",
            Format::MbTiles => "MBTiles",
            Format::PmTiles => "PMTiles",
            Format::FlatGeobuf => "FlatGeobuf",
            Format::GeoParquet => "GeoParquet",
            Format::GeoArrow => "GeoArrow",
            Format::Gml => "GML",
            Format::OsmXml => "OSM XML",
            Format::OsmPbf => "OSM PBF",
            Format::Fit => "FIT",
            Format::Tcx => "TCX",
            Format::FileGdb => "File Geodatabase",
            Format::MapInfoTab => "MapInfo TAB",
            Format::MapInfoMif => "MapInfo MIF",
            Format::S57 => "S-57 chart",
            Format::TopoJson => "TopoJSON",
            Format::Igc => "IGC flight log",
            Format::Nmea => "NMEA log",
            Format::BsbChart => "BSB/KAP chart",
            Format::ErMapper => "ER Mapper",
            Format::SagaGrid => "SAGA grid",
            Format::ArcInfoGrid => "Arc/Info grid",
            Format::GdalVrt => "GDAL VRT",
            Format::PlainText => "Plain text",
            Format::Nfo => "NFO text",
            Format::Markdown => "Markdown",
            Format::Csv => "CSV",
            Format::Tsv => "TSV",
            Format::Rtf => "RTF",
            Format::SubRip => "SubRip subtitles",
            Format::WebVtt => "WebVTT subtitles",
            Format::Ssa => "SubStation Alpha subtitles",
            Format::MicroDvd => "MicroDVD subtitles",
            Format::VobSub => "VobSub subtitles",
            Format::Pgs => "PGS subtitles",
            Format::Lrc => "LRC lyrics",
            Format::Ttml => "TTML subtitles",
            Format::Scc => "Scenarist captions",
            Format::Nitf => "NITF",
            Format::Ecw => "ECW",
            Format::MrSid => "MrSID",
            Format::Mgh => "FreeSurfer MGH",
            Format::Minc => "MINC",
            Format::Nexus => "NeXus",
            Format::Imaris => "Imaris",
            Format::Bag => "BAG",
            Format::Srtm => "SRTM",
            Format::Cube => "Gaussian cube",
            Format::Cbf => "CBF",
            Format::Edf => "ESRF EDF",
            Format::Smv => "SMV",
            Format::Bruker => "Bruker frame",
            Format::MarCcd => "MarCCD",
            Format::Raxis => "R-AXIS",
            Format::EsriAscii => "Esri ASCII grid",
            Format::Dted => "DTED",
            Format::Envi => "ENVI",
            Format::Sentinel2 => "Sentinel-2",
            Format::Stac => "STAC",
            Format::Tiles3d => "3D Tiles",
            Format::EsriBil => "ESRI BIL",
            Format::ErdasImagine => "ERDAS IMAGINE",
            Format::Grib => "GRIB",
            Format::Hdf4 => "HDF4",
            Format::Tia => "FEI TIA",
            Format::Bcf => "Bruker BCF",
            Format::Ang => "EDAX ANG",
            Format::Ctf => "Oxford CTF",
            Format::Nanoscope => "Bruker Nanoscope",
            Format::Nanonis => "Nanonis SXM",
            Format::Jpk => "JPK",
            Format::NtMdt => "NT-MDT",
            Format::X3p => "X3P",
            Format::MetroPro => "Zygo MetroPro",
            Format::Datx => "Zygo datx",
            Format::Vk4 => "Keyence VK4",
            Format::Plu => "Sensofar PLu",
            Format::Al3d => "Alicona AL3D",
            Format::Sur => "Digital Surf",
            Format::Segy => "SEG-Y",
            Format::SeismicUnix => "Seismic Unix",
            Format::Txm => "Zeiss Xradia TXM",
            Format::Amira => "Amira",
            Format::SkyScan => "Bruker SkyScan",
            Format::Xtek => "Nikon X-Tek",
            Format::Vgi => "Volume Graphics VGI",
            Format::RawVolume => "Raw volume",
            Format::Cirrus => "Zeiss Cirrus OCT",
            Format::ParRec => "Philips PAR/REC",
            Format::Ecat => "ECAT 7",
            Format::Imzml => "imzML",
            Format::Ptm => "PTM",
            Format::Rti => "RTI",
            Format::Relight => "Relight RTI",
            Format::Pdb => "PDB",
            Format::Mmcif => "PDBx/mmCIF",
            Format::Cif => "CIF",
            Format::Sdf => "MDL SDF",
            Format::MolXyz => "XYZ (molecule)",
            Format::FlirJpeg => "FLIR radiometric JPEG",
            Format::FlirFff => "FLIR FFF",
            Format::FlirSeq => "FLIR sequence",
            Format::InfirayIrg => "InfiRay IRG",
            Format::InfratecIrb => "InfraTec IRB",
            Format::TestoBmt => "Testo BMT",
            Format::FlukeIs2 => "Fluke IS2",
            Format::Gifti => "GIFTI",
            Format::Cifti => "CIFTI-2",
            Format::Trk => "TrackVis",
            Format::Tck => "MRtrix tracks",
            Format::Mif => "MRtrix image",
            Format::Omero => "OMERO",
            Format::Mp4 => "MPEG-4",
            Format::QuickTime => "QuickTime",
            Format::ThreeGpp => "3GPP",
            Format::Matroska => "Matroska",
            Format::WebM => "WebM",
            Format::MpegTs => "MPEG-TS",
            Format::MpegPs => "MPEG-PS",
            Format::MpegVideo => "MPEG video",
            Format::Avi => "AVI",
            Format::Ogg => "Ogg",
            Format::Ivf => "IVF",
            Format::VideoStream => "Raw video stream",
            Format::Apv => "APV",
            Format::Mxf => "MXF",
            Format::Flv => "FLV",
            Format::Asf => "ASF",
            Format::Wtv => "WTV",
            Format::Dv => "DV",
            Format::Mp3 => "MPEG audio",
            Format::Aac => "AAC",
            Format::Flac => "FLAC",
            Format::Wave => "WAVE",
            Format::Aiff => "AIFF",
            Format::Dts => "DTS",
            Format::Ac3 => "AC-3",
            Format::Amr => "AMR",
            Format::WavPack => "WavPack",
            Format::Ape => "Monkey's Audio",
            Format::Mpc => "Musepack",
            Format::Au => "AU",
            Format::Caf => "CAF",
            Format::W64 => "Wave64",
            Format::Voc => "Creative Voice",
            Format::Svx => "8SVX",
            Format::Avr => "AVR",
            Format::Sphere => "NIST SPHERE",
            Format::Ircam => "IRCAM",
            Format::Tta => "TTA",
            Format::Shorten => "Shorten",
            Format::Dsf => "DSF",
            Format::Dff => "DSDIFF",
            Format::Tracker => "Tracker module",
            Format::Y4m => "YUV4MPEG2",
            Format::Cdg => "CD+G",
            Format::PsxStr => "PlayStation movie",
            Format::OggAudio => "Ogg",
            Format::M4a => "M4A",
            Format::Gsm => "GSM 06.10",
            Format::Qcp => "QCP",
            Format::Qoa => "QOA",
            Format::Sln => "signed linear PCM",
            Format::Htk => "HTK",
            Format::Xi => "FastTracker 2 instrument",
            Format::SoundFont => "SoundFont 2",
            Format::Sfz => "SFZ",
            Format::Dls => "DLS",
            Format::Midi => "MIDI",
            Format::Mobi => "MOBI",
            Format::Fb2 => "FB2",
        }
    }
}

/// Header facts, available without decoding pixels.
#[derive(Clone, Debug)]
pub struct ImageInfo {
    pub format: Format,
    /// Stored size (before orientation).
    pub width: u32,
    pub height: u32,
    pub orientation: Orientation,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    pub has_alpha: bool,
    pub bit_depth: u8,
    /// Progressive JPEG / interlaced PNG.
    pub progressive: bool,
    /// Short technical description of the coding, e.g. "baseline 4:2:0".
    pub coding: String,
    /// Byte range of an embedded preview image (EXIF thumbnail) inside the file.
    pub embedded_preview: Option<Range<usize>>,
    /// The decoder can decode regions cheaply (row index or restart markers).
    pub fast_regions: bool,
    /// Downscale factors the decoder supports natively, finest first.
    pub scales: &'static [u32],
}

impl ImageInfo {
    /// Size as displayed (orientation applied).
    pub fn display_size(&self) -> (u32, u32) {
        self.orientation.display_size(self.width, self.height)
    }
    pub fn megapixels(&self) -> f64 {
        self.width as f64 * self.height as f64 / 1e6
    }
}

/// Region in *scaled* output coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// What to decode.
#[derive(Clone, Copy, Debug)]
pub struct DecodeRequest {
    /// Downscale factor (one of [`ImageInfo::scales`]).
    pub scale: u32,
    pub region: Option<Rect>,
    /// Use several threads.
    pub parallel: bool,
    /// Also prepare whatever makes later region decodes fast (JPEG row index).
    pub prepare_regions: bool,
}

impl DecodeRequest {
    pub fn full(scale: u32) -> DecodeRequest {
        DecodeRequest {
            scale,
            region: None,
            parallel: true,
            prepare_regions: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("unsupported format")]
    UnsupportedFormat,
    /// The file's extension is shared by several formats (`.img`) and the content is none
    /// of those read: the extension and the formats, for "it is none of the … files".
    #[error("none of the .{ext} formats read ({})", formats.join(", "))]
    NotOneOf {
        ext: &'static str,
        formats: &'static [&'static str],
    },
    /// The content does not settle which of several formats the file is (they share its
    /// extension and none has a signature that matched): the app asks ([`ident`]).
    #[error("{0}")]
    Ambiguous(Box<Ambiguity>),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("too large")]
    TooLarge,
    /// The decode needs more memory than is free right now ([`specular_core::budget`]):
    /// how much free RAM it needs and how much there is, for the "needs N GB of free RAM"
    /// message. Unlike [`DecodeError::TooLarge`] it may work later or on a larger computer.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    /// The format opens with an optional decoder from another vendor that is not on this
    /// computer ([`optional`]): the app offers to get it.
    #[error("{0} files need the free {d} from {v}, which is not installed", d = .0.decoder_name(), v = .0.vendor())]
    NeedsDecoder(OptionalId),
    #[error("cancelled")]
    Cancelled,
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    /// The decoder panicked on this file and the job's boundary caught it
    /// ([`specular_core::isolate`]): a bug, logged with where it happened.
    #[error("{0}")]
    Internal(String),
}

impl From<specular_core::isolate::Panicked> for DecodeError {
    fn from(p: specular_core::isolate::Panicked) -> Self {
        // "internal error in specular-jpeg": what a viewer or error tile shows. The panic's
        // message and location are in the log already.
        DecodeError::Internal(p.brief())
    }
}

pub type Result<T> = std::result::Result<T, DecodeError>;

/// The kinds of `.img` file read, for the refusal of any other.
pub const IMG_FORMATS: &[&str] = &[
    "ADSC or Rigaku SMV",
    "Rigaku R-AXIS",
    "ERDAS IMAGINE",
    "Analyze 7.5 or NIfTI beside its .hdr",
    "ENVI beside its .hdr",
    "Zeiss Cirrus OCT",
    "VICAR",
    "PDS3 with its label",
    "GEM bit image",
];

/// What a page of a multi-image file is ([`Session::page_label`]), for the page indicator
/// and the info panel; the application words it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageLabel {
    /// An icon size (ICNS): its size in points and the pixel density (2 for `@2x`).
    Icon {
        width: u32,
        height: u32,
        density: u32,
    },
    /// An image of a texture: its array layer, cube face (0‥5 for +X, −X, +Y, −Y, +Z, −Z, 6
    /// for a sphere map) and depth slice, each only when the texture has more than one.
    Texture {
        layer: Option<u32>,
        face: Option<u32>,
        slice: Option<u32>,
    },
    /// One image of a multi-image file by its pixel size and, where the format states it per
    /// image, its bits per pixel: an ICO/CUR entry, an image of a HEIF/AVIF collection.
    Image {
        width: u32,
        height: u32,
        bits: Option<u32>,
    },
    /// The merged image of a layered document (page 0 of a PSD/PSB).
    Merged,
    /// A layered document's composite built from its layers, beside the merged image the
    /// file stores (page 1 of a PSD/PSB with both).
    FromLayers,
    /// A layer of a layered document (PSD/PSB) on its canvas: its name as the file gives it
    /// (may be empty), its place in the layer list counted from 1 at the top, and whether it
    /// is hidden in the file.
    DocumentLayer {
        name: Arc<str>,
        index: u32,
        hidden: bool,
    },
    /// A sheet of a drawing (DXF): model space, or a paper-space layout by its name.
    Sheet { name: Arc<str>, model: bool },
    /// A page whose name Specular made up (a board's "Top" view), shown translated.
    Generated(drawing::specular_vector::Generated),
    /// A view of one step of a board with several (ODB++ "panel", "pcb"): the step's name
    /// as the file gives it and the view's made-up name, shown translated.
    BoardStep {
        step: Arc<str>,
        view: drawing::specular_vector::Generated,
    },
    /// An image of a whole-slide file: the slide itself or one of its associated images.
    Slide(SlideImage),
    /// A thermal still's temperatures, or (`visible`) the visible photo the camera took
    /// with them.
    Thermal { visible: bool },
    /// A view of a stereo pair (an MPO from a 3D camera): the left view, or (`right`) the
    /// right one.
    Stereo { right: bool },
    /// An object of a file of named objects (a ROOT histogram or graph): its path in the
    /// file and its class.
    Object { name: Arc<str>, class: Arc<str> },
    /// A map of a file of several (a GIFTI data array, a CIFTI map or time point): its
    /// name (may be empty).
    Map { name: Arc<str> },
    /// A plane of a remote multi-dimensional image (OMERO): its focal plane and time
    /// point counted from 0, each with its count, only for an axis longer than one.
    Plane {
        z: Option<(u32, u32)>,
        t: Option<(u32, u32)>,
    },
    /// A page of a book (EPUB): the title of the chapter it is in, from its contents.
    Chapter { title: Arc<str> },
    /// A face of a font collection (TTC, OTC): its full name.
    Face { name: Arc<str> },
    /// A view of a remote-sensing product or cube (an ENVI cube's default bands, a
    /// Sentinel-2 product's composites), at a pixel size in metres when the product offers
    /// several.
    Product {
        view: ProductView,
        metres: Option<u32>,
    },
}

/// What a page of a remote-sensing product shows ([`PageLabel::Product`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductView {
    /// The cube's default bands as red, green and blue (ENVI `default bands`, or the bands
    /// nearest 640, 550 and 460 nm).
    Composite,
    /// Every band, one at a time.
    Bands,
    /// Red, green, blue (Sentinel-2 B04, B03, B02).
    TrueColour,
    /// Near infrared, red, green (B08, B04, B03): vegetation shows red.
    FalseColour,
    /// Short-wave infrared, narrow NIR, red (B12, B8A, B04).
    Swir,
    /// SWIR, NIR, blue (B11, B08, B02).
    Agriculture,
    /// The scene classification (Sentinel-2 L2A SCL): clouds, shadows, water, vegetation…
    SceneClass,
    /// The cloud probability, percent.
    CloudProbability,
}

/// What an image of a whole-slide file is ([`PageLabel::Slide`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlideImage {
    /// The scanned slide (the pyramid).
    Main,
    /// The photographed slide label.
    Label,
    /// The low-magnification picture of the whole glass slide.
    Macro,
    /// The scanner's thumbnail of the slide.
    Thumbnail,
    /// Another image of the file (an OME series, a Leica SCN region), by its name.
    Other(Arc<str>),
}

/// Timing facts of an animated image, available without decoding frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationInfo {
    /// Number of frames, when the container states it (APNG, WebP, AVIF); `None` when it is
    /// only known after a scan (GIF).
    pub frame_count: Option<u32>,
    /// Times to play; 0 plays forever.
    pub loop_count: u32,
}

/// What an office document is and says (Word, Excel, PowerPoint, iWork, OpenDocument text
/// documents and spreadsheets), for the info panel. Its preview, if any, is the session's
/// image; without one the session is a [`office_dec::NoPreviewSession`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentFacts {
    /// What it is, a stable key: "word", "excel", "powerpoint", "pages", "numbers",
    /// "keynote", "odf-text", "odf-spreadsheet", "odf-presentation", "odf-graphics",
    /// "odf-other".
    pub kind: &'static str,
    /// An Office 97–2003 file.
    pub legacy: bool,
    pub template: bool,
    pub macros: bool,
    /// The embedded preview's image format ("JPEG", "WMF", …); `None`: the file has none.
    pub preview: Option<&'static str>,
    pub props: DocumentProperties,
    /// How many items a playlist lists.
    pub items: Option<u64>,
}

pub use specular_office::Properties as DocumentProperties;

/// Camera and exposure facts of a photo (camera RAW), for the info panel.
/// What the header of a format without a metadata block of its own says (DPX, Cineon, XWD,
/// MIFF, TIM, WSQ, JBIG): the info panel's header group and its "all fields" list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeaderFacts {
    /// Key facts by stable key ("timecode", "edge-code", "comment"…, see
    /// `specular_metadata::HeaderFacts`), in display order.
    pub summary: Vec<(&'static str, String)>,
    /// Every header field as (name, value), in file order.
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraInfo {
    /// "Make Model" without repeating the make, e.g. "Canon EOS 5D".
    pub camera: String,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    /// Seconds.
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    /// Millimetres.
    pub focal_length: Option<f64>,
    /// EV.
    pub exposure_bias: Option<f64>,
    /// `YYYY:MM:DD HH:MM:SS` as stored.
    pub date_time: Option<String>,
    pub software: Option<String>,
}

/// One composited animation frame.
#[derive(Clone, Debug)]
pub struct Frame {
    pub index: u32,
    /// The whole canvas at the requested scale, after disposal and blending of earlier frames:
    /// ready to show as is.
    pub raster: Raster,
    /// How long to show the frame, as stored (no browser-style clamping; the player decides).
    pub delay: Duration,
}

/// Sequential frame decoder. Owns its compositing canvas; frames come out in display order.
pub trait Frames: Send {
    /// The next frame, or `None` after the last.
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>>;
    /// Restart at frame 0 (for looping and seeking backwards).
    fn rewind(&mut self) -> Result<()>;
    /// Positions the decoder so the next [`Frames::next_frame`] returns frame `index`, the
    /// way a video seeks: from the nearest sync sample before it, decoding forward without
    /// handing out the frames in between. `Ok(false)` when the source cannot (the player
    /// then rewinds and decodes up to the frame).
    fn seek(&mut self, _index: u32) -> Result<bool> {
        Ok(false)
    }
    /// Every frame's delay, when the container states them all up front (a video's sample
    /// table): the player's timeline is then complete before any frame decodes.
    fn delays(&self) -> Option<Vec<Duration>> {
        None
    }
    /// The frames a seek can start decoding at (a video's sync samples), as ascending
    /// presentation indices, when the source knows them. Every frame for intra-only video.
    fn sync_frames(&self) -> Option<Vec<u32>> {
        None
    }
    /// Lets `flag` interrupt a [`Frames::next_frame`] between two samples: it then returns
    /// [`DecodeError::Cancelled`] with the source as it was before the call, so the next call
    /// (or a [`Frames::seek`]) goes on from there. False when the source cannot; the flag is
    /// then never looked at.
    fn set_interrupt(&mut self, _flag: Arc<std::sync::atomic::AtomicBool>) -> bool {
        false
    }
}

/// How an image stores transparency ([`Session::alpha`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alpha {
    /// An alpha channel, straight (unassociated) or of unstated association.
    Straight,
    /// An alpha channel premultiplied into the colour samples (associated alpha).
    Premultiplied,
    /// No alpha channel: transparent palette entries or a colour key (PNG tRNS, GIF).
    Transparency,
}

impl Alpha {
    /// The `alpha` selector key of the metadata catalogue's encoding strings.
    pub fn key(self) -> &'static str {
        match self {
            Alpha::Straight => "alpha",
            Alpha::Premultiplied => "premultiplied",
            Alpha::Transparency => "transparency",
        }
    }
}

/// A 3D model with its decoded base-colour textures (by [`specular_mesh::Mesh::images`]
/// index; `None` where an image is missing or failed to decode).
#[derive(Clone, Debug)]
pub struct MeshModel {
    pub mesh: Arc<specular_mesh::Mesh>,
    pub textures: Arc<Vec<Option<specular_mesh::preview::Texture>>>,
    /// A building model's elements (IFC): which vertices each one owns, for BCF viewpoints
    /// that pick, hide or colour elements by GUID.
    pub ifc: Option<Arc<IfcIndex>>,
}

/// An IFC model's elements as drawn, and where its mesh sits in the project's frame.
#[derive(Clone, Debug, Default)]
pub struct IfcIndex {
    /// By [`specular_mesh::ifc::Element::first_vertex`].
    pub elements: Vec<specular_mesh::ifc::Element>,
    /// A mesh point `p` is at `p * mesh_metres + origin_metres` in the project (metres).
    pub mesh_metres: f64,
    pub origin_metres: [f64; 3],
    /// Where the model sits on the Earth, for placing several models in one scene.
    pub georef: specular_mesh::ifc::georef::Georef,
}

/// Gaussian splats (`.splat`, 3DGS PLY, `.spz`) in the viewer's z-up frame.
#[derive(Clone, Debug)]
pub struct SplatModel {
    pub splats: Arc<specular_splat::Splats>,
}

/// A point cloud: what its header says at once, its points as they load (on a
/// `dec-points` thread) and the finished cloud.
#[derive(Clone, Debug)]
pub struct PointsModel {
    pub loading: Arc<specular_points::Loading>,
}

/// A molecular structure (PDB, mmCIF, CIF, SDF, molecular XYZ), its first model.
#[derive(Clone, Debug)]
pub struct MoleculeModel {
    pub molecule: Arc<specular_mol::Molecule>,
}

/// A tractogram (TrackVis, MRtrix): its streamlines in RAS millimetres, at most
/// [`neuro_dec::MAX_SHOWN_POINTS`] points (more are subsampled by streamline).
#[derive(Clone, Debug)]
pub struct TractsModel {
    pub tracts: Arc<specular_neuro::Tractogram>,
}

/// A font face for the viewer's specimen: the opened file and which of its faces.
#[derive(Clone, Debug)]
pub struct FontModel {
    pub file: Arc<specular_font::FontFile>,
    pub face: usize,
}

impl FontModel {
    pub fn face(&self) -> &specular_font::Face {
        &self.file.faces[self.face]
    }
}

/// What a book (EPUB) says about itself, for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookModel {
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
    pub fixed_layout: bool,
    pub right_to_left: bool,
    pub contents: usize,
    pub pages: usize,
    pub fonts: usize,
    pub obfuscated_fonts: usize,
}

/// An entry of a document's table of contents: its title, its depth (0 at the top) and the
/// page it leads to (`None` when its target is not among the pages laid out).
#[derive(Clone, Debug, PartialEq)]
pub struct ContentsEntry {
    pub title: Arc<str>,
    pub depth: u32,
    pub page: Option<u32>,
}

/// One opened image.
pub trait Session: Send + Sync {
    fn info(&self) -> &ImageInfo;
    fn source(&self) -> &Arc<Source>;
    /// Decode the embedded preview (EXIF thumbnail), if any. Its orientation is the image's.
    fn decode_preview(&self) -> Option<Result<Raster>>;
    /// [`Session::decode_preview`] for a thumbnail whose long side is about `long_side`: a
    /// preview that decodes at several sizes (a raw's embedded JPEG) is decoded no larger
    /// than that needs, with its long side at least `long_side` where it has that many
    /// pixels. By default the preview as [`Session::decode_preview`] gives it.
    fn decode_preview_for(&self, long_side: u32) -> Option<Result<Raster>> {
        let _ = long_side;
        self.decode_preview()
    }
    /// A quick rendition for a thumbnail whose long side is about `long_side`, sampled from
    /// a big array (every n-th value, only the chunks holding them) rather than decoded in
    /// full; `None` when the format has none or the image is small enough to decode. Its
    /// orientation is the image's; it may be larger than asked.
    fn decode_sampled(&self, _long_side: u32, _cancel: &CancelToken) -> Option<Result<Raster>> {
        None
    }
    /// Decode a scale and optional region.
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster>;
    /// Prepare fast region decoding (JPEG: build the row index). Idempotent.
    fn prepare_regions(&self, _cancel: &CancelToken) -> Result<()> {
        Ok(())
    }
    /// Whether [`Session::prepare_regions`] has completed (or is unnecessary).
    fn regions_ready(&self) -> bool {
        true
    }
    /// Estimated peak memory of `decode(req)` (output plus working buffers), for keeping
    /// concurrent decodes within a RAM budget. 0 when unknown.
    fn decode_memory(&self, _req: &DecodeRequest) -> u64 {
        0
    }
    /// Estimated peak memory of showing the image: the whole-image `levels` (1/scale)
    /// decoding at the same time, as an open does. 0 when unknown.
    fn open_memory(&self, levels: &[u32]) -> u64 {
        levels
            .iter()
            .map(|&scale| {
                self.decode_memory(&DecodeRequest {
                    scale,
                    region: None,
                    parallel: true,
                    prepare_regions: true,
                })
            })
            .sum()
    }
    /// Memory held by the session's caches.
    fn memory_bytes(&self) -> usize {
        0
    }
    /// Drops what the session keeps only to speed up later decodes, as the app does under
    /// memory pressure: camera RAW the held sensor data, decoded sensor and developed tiles and
    /// its developed levels (a preview-mode RAW its preview's levels); decoders that cache
    /// whole levels and crop regions from them (strip TIFF, JPEG 2000 box-filtered levels,
    /// JPEG XL, AVIF, WebP, PNG, PSD, OpenEXR, Radiance, DDS/KTX, the bitmap and legacy
    /// formats) those levels; GIF its first frame's palette indices; HEIC its decoded YCbCr
    /// image and libheif's levels. The JPEG row index stays: it is small (13-280 KiB for
    /// 24-108 MP Huffman files, ~1 MiB for 24 MP arithmetic progressive) and slow to rebuild
    /// (10-650 ms), and without it every region decodes the file from the start. Later decodes
    /// rebuild what they need, with the same results. Returns the bytes released from
    /// [`Session::memory_bytes`]. Neither this nor `memory_bytes` waits for a decode in
    /// progress.
    fn trim_memory(&self) -> usize {
        0
    }
    /// Whether a whole-image decode at a reduced scale costs much less than at full size
    /// (JPEG: DCT scaling). When it does not (PNG inflates and unfilters every row at any
    /// scale), a quick coarse level ahead of the sharp one only adds a decode.
    fn scaled_decode_cheap(&self) -> bool {
        true
    }
    /// Announces the whole-image levels (1/scale) about to be requested, with 1 when region
    /// requests at full size are likely. A decoder that can produce several levels in one
    /// pass decodes them together on the first such request and hands each to its own.
    fn want_levels(&self, _scales: &[u32]) {}
    /// Animation timing, when the file holds more than one frame to play. `decode` then
    /// returns the first frame.
    fn animation(&self) -> Option<&AnimationInfo> {
        None
    }
    /// Start decoding frames at 1/`scale` (one of [`ImageInfo::scales`]).
    fn frames(&self, _scale: u32) -> Result<Box<dyn Frames>> {
        Err(DecodeError::Unsupported("not animated".into()))
    }
    /// Number of pages (multi-page TIFF, multi-frame DICOM, ICNS icon sizes, ICO/CUR entries,
    /// texture layers, faces and slices, the top-level images of a HEIF/AVIF collection, the
    /// merged image and pixel layers of a PSD/PSB, XCF or ORA, the further images of an MPO,
    /// the parts of a multi-part EXR); the session shows the first. Other pages are opened
    /// with
    /// [`open_tiff_page`] / [`open_dicom_frame`] / [`open_icns_page`] / [`open_texture_page`].
    fn page_count(&self) -> u32 {
        1
    }
    /// The page this session shows (0-based, below [`Session::page_count`]).
    fn page_index(&self) -> u32 {
        0
    }
    /// What this session's page is, where the format says (an icon size; a texture's layer,
    /// face or slice; an ICO entry's, a collection image's or an MPO image's size; a PSD's,
    /// XCF's, ORA's or Krita's merged image or layer; an EXR part's name; a multi-image PNM's
    /// or a JPX codestream's size).
    fn page_label(&self) -> Option<PageLabel> {
        None
    }
    /// Opens page `page` of the same file from this session's bytes: no re-read and no
    /// format probe, only the page's own headers are parsed. Multi-page TIFF, ICNS sizes, ICO
    /// entries, texture images, HEIF/AVIF collection images, PSD/PSB, XCF, ORA and Krita layers
    /// (from the layer records parsed at open), MPO images, EXR parts, the images of a
    /// multi-image PNM/PAM file and the codestreams of a JPX file; DICOM frames step in medical mode or play in the animation player
    /// instead, so DICOM answers `Unsupported` like formats without pages.
    fn open_page(&self, _page: u32) -> Result<Arc<dyn Session>> {
        Err(DecodeError::Unsupported("no pages".into()))
    }
    /// A document's table of contents (EPUB), shared by all its pages.
    fn contents(&self) -> Option<Arc<[ContentsEntry]>> {
        None
    }
    /// The font face shown (TTF, OTF, a face of a TTC/OTC, WOFF, WOFF2): the specimen
    /// draws it with the text stack; the session's raster is a small CPU preview.
    fn font(&self) -> Option<&FontModel> {
        None
    }
    /// What a book (EPUB) says about itself, for the info panel.
    fn book(&self) -> Option<&BookModel> {
        None
    }
    /// A text-like file the viewer shows itself (every line of a log, every row of a table,
    /// every cue of a subtitle file, a Markdown source); the session's pages are its
    /// rendering or a preview of its start.
    fn text(&self) -> Option<&TextModel> {
        None
    }
    /// The medical summary for the info panel (DICOM): modality, body part, descriptions,
    /// spacing, transfer syntax. Never holds patient-identifying attributes.
    fn medical(&self) -> Option<&MedicalSummary> {
        None
    }
    /// The shown frame's stored and modality-rescaled values (HU for CT), with its windows
    /// and default VOI, for window / level and value readouts (DICOM). Decoded on first use
    /// and kept until [`Session::trim_memory`].
    fn modality_frame(&self, _cancel: &CancelToken) -> Option<Result<Arc<ModalityFrame>>> {
        None
    }
    /// Frame `index` of a multi-frame medical image with its modality values (DICOM),
    /// decoded from this session without re-reading the file; the shown frame comes from the
    /// [`Session::modality_frame`] cache, others are not kept.
    fn modality_frame_at(
        &self,
        _index: u32,
        _cancel: &CancelToken,
    ) -> Option<Result<Arc<ModalityFrame>>> {
        None
    }
    /// The drawing behind this page (DXF): its prepared vector scene, which the viewer draws
    /// itself at the exact zoom, and how the sheet maps onto the session's pixels.
    fn drawing(&self) -> Option<&drawing::DrawingPage> {
        None
    }
    /// A PDF page's size as displayed, in points (rotation and crop box applied): the
    /// session's raster is this size times its pixels per point. None for anything else.
    fn page_points(&self) -> Option<(f32, f32)> {
        None
    }
    /// The drawing's own scale (PDF viewport measures, CAD units and layout viewports):
    /// calibrations by region of the page and the paper size for printing to scale.
    fn drawing_scale(&self) -> Option<Arc<drawing::DrawingScale>> {
        self.drawing()
            .and_then(drawing::DrawingPage::drawing_scale)
            .map(Arc::new)
    }
    /// What a spectrum file (JCAMP-DX, SPC, mzML, Bruker NMR) says about itself and the page
    /// shown, for the info panel.
    fn spectrum(&self) -> Option<&SpectrumFacts> {
        None
    }
    /// Where the image or map sits on the Earth (GeoTIFF, GeoPackage tiles, vector maps): its
    /// coordinate system, the map coordinates of the stored pixels and, for vector maps,
    /// the features for hover and click.
    fn geo(&self) -> Option<Arc<geo::GeoRef>> {
        None
    }
    /// What a medical research volume (NIfTI, Analyze, NRRD, MetaImage) is: its dimensions,
    /// voxel size, data type and where its orientation came from, for the info panel.
    fn research_volume(&self) -> Option<&VolumeFacts> {
        None
    }
    /// The voxels of a volume another reader decoded (a CIFTI file's), which cannot be read
    /// again from the file: MPR shows these.
    fn prepared_volume(&self) -> Option<medvol::MedVolume> {
        None
    }
    /// The DICOM file around an encapsulated document (PDF, CDA, STL, OBJ) this session
    /// shows: the document's title and type and the study summary, for the info panel.
    fn encapsulated(&self) -> Option<&EncapsulatedFacts> {
        None
    }
    /// The axes, world coordinates and description of a scientific image (FITS, XISF).
    fn science(&self) -> Option<&science::ScienceSummary> {
        None
    }
    /// The plane at `index` (a position on each of [`science::ScienceSummary::axes`]) of a
    /// scientific image as real values, cached for a few planes until
    /// [`Session::trim_memory`].
    fn nd_plane(
        &self,
        _index: &[u32],
        _cancel: &CancelToken,
    ) -> Option<Result<Arc<science::NdPlane>>> {
        None
    }
    /// The Bayer pattern a session that debayers its own frames (SER video) uses now;
    /// `None` when they show raw.
    fn debayer(&self) -> Option<specular_core::nd::Cfa> {
        None
    }
    /// Whether [`Session::set_debayer`] applies: the session's frames are raw colour filter
    /// array samples it can debayer (SER).
    fn can_debayer(&self) -> bool {
        false
    }
    /// Debayers the frames with `cfa` from now on, or shows them raw (`None`): later
    /// decodes and frame decoders follow. Ignored unless [`Session::can_debayer`].
    fn set_debayer(&self, _cfa: Option<specular_core::nd::Cfa>) {}
    /// The model of a 3D mesh file (STL, OBJ, PLY, glTF, GLB, 3MF), for the interactive 3D
    /// view; [`Session::decode`] renders an isometric preview of it.
    fn mesh(&self) -> Option<&MeshModel> {
        None
    }
    /// A CAD model's PMI (dimensions, tolerances, datums, saved views), with
    /// [`Session::mesh`].
    fn pmi(&self) -> Option<&Arc<pmi::Pmi>> {
        None
    }
    /// The cloud of a point cloud file (LAS, LAZ, COPC, PCD, XYZ, PTS, E57, PLY without
    /// faces), for the interactive 3D view; [`Session::decode`] renders a preview of it.
    fn points(&self) -> Option<&PointsModel> {
        None
    }
    /// The splats of a Gaussian splat file (`.splat`, 3DGS PLY, `.spz`), for the interactive
    /// 3D view; [`Session::decode`] renders a preview of them.
    fn splats(&self) -> Option<&SplatModel> {
        None
    }
    /// A 3D Tiles tileset: the tiles a view needs and the scene they make, for the 3D view
    /// (its first scene is [`Session::mesh`] or [`Session::points`]).
    fn tiles3d(&self) -> Option<&Tiles3dModel> {
        None
    }
    /// The structure of a molecular file (PDB, mmCIF, CIF, SDF, molecular XYZ), for the
    /// interactive 3D view; [`Session::decode`] renders a preview of it.
    fn molecule(&self) -> Option<&MoleculeModel> {
        None
    }
    /// The streamlines of a tractogram (TrackVis `.trk`, MRtrix `.tck`), for the
    /// interactive 3D view; [`Session::decode`] renders a preview of them.
    fn tracts(&self) -> Option<&TractsModel> {
        None
    }
    /// What a neuroimaging file is (GIFTI, CIFTI-2, tractograms), for the info panel, and
    /// the values a coloured surface shows, for recolouring it.
    fn neuro(&self) -> Option<&neuro_dec::NeuroFacts> {
        None
    }
    /// How long each frame of a multi-frame medical image shows in cine, when the file says
    /// (DICOM Frame Time Vector, Frame Time, Cine Rate or Recommended Display Frame Rate).
    fn cine_timing(&self) -> Option<&[Duration]> {
        None
    }
    /// What a movie holds (MP4, QuickTime, 3GPP): duration, codecs, recording date.
    fn video(&self) -> Option<&VideoFacts> {
        None
    }
    /// The session itself, for its opener's own interface (a sound file's analysis for the
    /// sound page); `None` for most.
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
    /// Words the file holds beyond its name and metadata, for the Library's text search (a
    /// song's lyrics); `None` for most.
    fn search_text(&self) -> Option<String> {
        None
    }
    /// Camera and exposure facts, when the decoder reads them (camera RAW).
    fn camera(&self) -> Option<&CameraInfo> {
        None
    }
    /// The file's own header facts (formats without Exif or XMP: DPX, Cineon, XWD, MIFF, TIM,
    /// WSQ, JBIG).
    /// An office document's kind and properties ([`DocumentFacts`]).
    fn document(&self) -> Option<&DocumentFacts> {
        None
    }
    fn header(&self) -> Option<&HeaderFacts> {
        None
    }
    /// The ICC profile embedded in the file as stored (not the one the output is converted
    /// to), for the info panel, when the decoder has it and [`embedded_metadata`] cannot find
    /// it from headers alone: JPEG XL codes it in the codestream.
    fn embedded_icc(&self) -> Option<&[u8]> {
        None
    }
    /// The XMP packet of the file, for the info panel, when the decoder has it and
    /// [`embedded_metadata`] cannot find it from headers alone: a PDF's catalog metadata
    /// stream, or one made from its document information (title, author, subject, keywords,
    /// creator tool, producer).
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        None
    }
    /// The Exif TIFF structure of the file, for the info panel, when the decoder has it and
    /// the header scan of other formats does not reach it (BPG and FLIF extensions).
    fn embedded_exif(&self) -> Option<Vec<u8>> {
        None
    }
    /// The colour model of the stored samples for the info panel, from headers the session
    /// has parsed (no extra I/O). One of the `colour-model` selector keys of the metadata
    /// catalogue: "RGB", "Greyscale", "Palette", "YCbCr", "YCCK", "CMYK", "Lab", "YCgCo",
    /// "XYB", "YC", "Multichannel", "Duotone", "Bayer", "X-Trans", "CFA", "LinearRaw",
    /// "RGBE", "XYZE", "R", "RG".
    fn colour_model(&self) -> Option<&'static str> {
        None
    }
    /// How the stored samples carry transparency, for the info panel, from headers the
    /// session has parsed (no extra I/O). The default reads [`ImageInfo::has_alpha`] as an
    /// alpha channel of unknown or straight association; decoders that know better say
    /// premultiplied, or a palette / colour-key transparency.
    fn alpha(&self) -> Option<Alpha> {
        self.info().has_alpha.then_some(Alpha::Straight)
    }
    /// The colour space the file names other than through an ICC profile (one of
    /// [`COLOUR_SPACES`]), when that signalling wins over a profile (PNG `cICP`, HDR `nclx`)
    /// or there is no profile; `Some("")` when the file describes its colour in a way that
    /// has no name (PNG `gAMA`/`cHRM`). From headers the session has parsed; see
    /// [`colour_space`].
    fn colour_space(&self) -> Option<&'static str> {
        None
    }
    /// Chroma subsampling of stored YCbCr ("4:2:0", "4:2:2", "4:4:4", …) for the info panel,
    /// when the decoder knows it from headers. JPEG's comes from the metadata scan.
    fn chroma_subsampling(&self) -> Option<&'static str> {
        None
    }
    /// Whether the file announces a gain map we can apply (ISO 21496-1 in JPEG or a HEIF /
    /// AVIF `tmap` item, Ultra HDR `hdrgm` XMP, Apple's HDR gain map). Cheap: container and
    /// header metadata only.
    fn has_gain_map(&self) -> bool {
        false
    }
    /// Decodes the gain map (an RGBA8 raster covering the whole image) with its metadata.
    /// `None` without one; not cached, so callers keep the result.
    fn gain_map(&self, _cancel: &CancelToken) -> Option<specular_core::GainMap> {
        None
    }
    /// The coefficients of a reflectance transformation image (PTM, HSH, Relight), which
    /// the viewer relights as the light moves; [`Session::decode`] shows the image lit
    /// straight from the viewer.
    fn rti(&self) -> Option<Arc<specular_rti::Rti>> {
        None
    }
}

/// Bytes of the file start that [`Decoder::probe`] gets from [`Registry::open`].
pub const PROBE_HEAD: usize = 64;
/// Bytes a text format's probe reads (signs after comments or an XML prolog).
pub(crate) const PROBE_TEXT: usize = 64 << 10;

/// A format decoder.
pub trait Decoder: Send + Sync {
    fn format(&self) -> Format;
    /// Whether `head` (the first bytes of a file) looks like this format.
    fn probe(&self, head: &[u8]) -> bool;
    /// Content check with the whole file at hand, for formats the first bytes cannot settle
    /// (camera RAW inside TIFF, XBM/XPM, WBMP). Defaults to [`Decoder::probe`] on the first
    /// [`PROBE_HEAD`] bytes.
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(PROBE_HEAD)])
    }
    /// How sure the content check is that `data` (the whole file) is this format, `None`
    /// when it is not ([`ident`]). A signature is [`Confidence::Certain`] (the default:
    /// [`Decoder::probe_file`] matched); a decoder whose check is structural (header fields
    /// that add up, text that parses, no magic) says [`Confidence::Likely`] or
    /// [`Confidence::Possible`], so a file another format also fits is asked about instead
    /// of opened as whichever decoder comes first.
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        self.probe_file(data).then_some(Confidence::Certain)
    }
    /// File extensions (lower case) associated with the format.
    fn extensions(&self) -> &'static [&'static str];
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>>;
}

/// What [`Registry::scan`] found.
struct Scan<'a> {
    /// The first decoder whose signature matched.
    certain: Option<&'a dyn Decoder>,
    /// Weaker matches before it.
    weak: Vec<Candidate>,
    /// Formats claiming the extension that the content did not rule out.
    guesses: Vec<Candidate>,
    /// The file's extension.
    ext: Option<String>,
}

impl Scan<'_> {
    /// The weaker matches, then the guesses.
    fn candidates(&self) -> Vec<Candidate> {
        self.weak.iter().chain(&self.guesses).copied().collect()
    }
}

/// The set of available decoders.
pub struct Registry {
    decoders: Vec<Box<dyn Decoder>>,
    /// Every decoder's extensions, for [`Registry::supports_extension`] (asked of every file
    /// a folder listing meets).
    extensions: std::collections::HashSet<&'static str>,
}

impl Default for Registry {
    fn default() -> Self {
        colour::install();
        let mut r = Registry {
            // One line per decoder, in probe order. Content probes must not overlap.
            decoders: vec![
                // Thermal stills first: an R-JPEG is a JPEG with a FLIR APP1 segment before
                // the scan (a walk over the marker headers, which ordinary JPEGs pay); FFF
                // ("FFF\0"/"AFF\0"), IRB ("\xffIRB\0") and IRG (its magic and a header whose
                // sizes fit the file).
                Box::new(thermal_dec::ThermalDecoder(Format::FlirJpeg)),
                // A sequence is FFF frames back to back: ahead of the single FFF.
                Box::new(thermal_dec::ThermalDecoder(Format::FlirSeq)),
                Box::new(thermal_dec::ThermalDecoder(Format::FlirFff)),
                Box::new(thermal_dec::ThermalDecoder(Format::InfratecIrb)),
                Box::new(thermal_dec::ThermalDecoder(Format::InfirayIrg)),
                // Fluke IS2: a ZIP whose directory names `CameraInfo.gpbenc` (ahead of the
                // other ZIP-based formats). Testo BMT: a JPEG preview, then its `<ToFo>`
                // header; claimed with its extension (`thermal_by_name` in `open`).
                Box::new(thermal_dec::ThermalDecoder(Format::FlukeIs2)),
                Box::new(thermal_dec::ThermalDecoder(Format::TestoBmt)),
                // FCS list-mode data: "FCS" and a version, then four spaces.
                Box::new(fcs_dec::FcsDecoder),
                // Philips iSyntax: its XML header's `DPUfsImport` data object (refused by
                // name), ahead of every XML reader.
                Box::new(isyntax_dec::IsyntaxDecoder),
                // Before JPEG: JPEG-LS streams also start with SOI.
                Box::new(jpegls_dec::JpegLsDecoder),
                // FlashPix: an OLE compound file with an `Image Contents` property set
                // (ahead of the Office decoders, which take other compound files).
                Box::new(fpx_dec::FpxDecoder),
                Box::new(jpeg::JpegDecoder),
                Box::new(png_dec::PngDecoder),
                Box::new(webp_dec::WebpDecoder),
                Box::new(avif_dec::AvifDecoder),
                Box::new(jxl_dec::JxlDecoder),
                // ND2: the chunk magic 0xDA 0xCE 0xBE 0x0A, or a legacy (JPEG 2000) file:
                // a JP2 signature box and Nikon's box map at the end, so ahead of JP2.
                Box::new(vendor_dec::VendorDecoder(vendor_dec::Kind::Nd2)),
                Box::new(jp2_dec::Jp2Decoder),
                Box::new(heic::HeicDecoder),
                // "BPG\xfb".
                Box::new(bpg_dec::BpgDecoder),
                // "FLIF" and a format byte.
                Box::new(flif_dec::FlifDecoder),
                Box::new(bitmap::BitmapDecoder(bitmap::Kind::Gif)),
                Box::new(bitmap::BitmapDecoder(bitmap::Kind::Bmp)),
                Box::new(bitmap::BitmapDecoder(bitmap::Kind::Ico)),
                Box::new(bitmap::BitmapDecoder(bitmap::Kind::Ani)),
                Box::new(bitmap::BitmapDecoder(bitmap::Kind::Qoi)),
                Box::new(bitmap::BitmapDecoder(bitmap::Kind::Pnm)),
                // No signature: chosen by extension when no content probe matches.
                Box::new(bitmap::BitmapDecoder(bitmap::Kind::Tga)),
                Box::new(svg::SvgDecoder),
                // Before TIFF: a DICOM preamble may hold a TIFF header (dual-format files).
                Box::new(dicom_dec::DicomDecoder),
                // "II\xBC\x01": a TIFF-like header, but not TIFF's magic.
                Box::new(jxr_dec::JxrDecoder),
                // A TIFF with the MarCCD header ("MMX" at byte 1028): before the TIFF readers.
                Box::new(xtal_dec::XtalDecoder(specular_xtal::Kind::MarCcd)),
                // Before slides, RAW and TIFF: JPK AFM images are TIFFs with JPK's own tags.
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Jpk)),
                // Before RAW and TIFF: slides are TIFF files that say so (Aperio description,
                // NDPI tags, Leica XML, OME-XML) or tiled pyramids of at least 64 MP that
                // are not camera RAW.
                Box::new(wsi_dec::WsiDecoder(wsi_dec::Kind::Tiff)),
                // Before TIFF: DNG, CR2, NEF, ARW, PEF... start with a TIFF header.
                // Blackmagic RAW: QuickTime with a BRAW codec, so ahead of the movies.
                Box::new(braw_dec::BrawDecoder),
                // Raw video: Cinema RAW Light (CR3-like) and QuickTime with ProRes RAW, ahead of
                // the stills and the movies.
                Box::new(rawclip_dec::RawClipDecoder(
                    rawclip_dec::Kind::CinemaRawLight,
                )),
                Box::new(rawclip_dec::RawClipDecoder(rawclip_dec::Kind::ProResRaw)),
                Box::new(raw_dec::RawDecoder),
                Box::new(tiff_dec::TiffDecoder),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Dds)),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Ktx2)),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Ktx)),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Pvr)),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Vtf)),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Astc)),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Basis)),
                // "PKM 10"/"PKM 20"; Firaxis "FTEX".
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Pkm)),
                Box::new(texture_dec::TextureDecoder(texture_dec::Kind::Ftex)),
                Box::new(psd_dec::PsdDecoder),
                Box::new(mng_dec::MngDecoder(mng_dec::Kind::Mng)),
                Box::new(mng_dec::MngDecoder(mng_dec::Kind::Jng)),
                Box::new(metafile::MetafileDecoder(metafile::Kind::Wmf)),
                Box::new(metafile::MetafileDecoder(metafile::Kind::Emf)),
                Box::new(icns_dec::IcnsDecoder),
                // ZIP with a `main.xml` ISO 25178-72 document.
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::X3p)),
                Box::new(preview_doc::PreviewDocDecoder(preview_doc::Kind::Procreate)),
                Box::new(preview_doc::PreviewDocDecoder(preview_doc::Kind::Affinity)),
                // "BLENDER" (compressed files by extension) and SketchUp's UTF-16 header.
                Box::new(preview_doc::PreviewDocDecoder(preview_doc::Kind::Blend)),
                Box::new(preview_doc::PreviewDocDecoder(preview_doc::Kind::Sketchup)),
                // "PDN3" and an XML header (Paint.NET 2.x gzip streams by extension).
                Box::new(pdn_dec::PdnDecoder),
                // "Paint Shop Pro Image File\n\x1a".
                Box::new(psp_dec::PspDecoder),
                // RIFF "CDR?" and CorelDRAW 1/2's "WL"; X4 and later are ZIPs with a
                // `content/riffData.cdr` or `content/root.dat` member.
                Box::new(cdr_dec::CdrDecoder),
                // "CSFCHUNK".
                Box::new(embedded_dec::ClipDecoder),
                // A ZIP whose first member is a `.npy`.
                Box::new(array_dec::ArrayDecoder(array_dec::Kind::Npz)),
                // ZIP documents recognised by their stored `mimetype` first member.
                Box::new(ora_dec::OraDecoder(specular_ora::Kind::Ora)),
                Box::new(ora_dec::OraDecoder(specular_ora::Kind::Kra)),
                // OME-Zarr: folders (see `Registry::open`) and `.ome.zarr.zip` by extension.
                Box::new(wsi_dec::WsiDecoder(wsi_dec::Kind::Zarr)),
                // MIRAX: a `.mrxs` (a JPEG preview) beside its `<name>/Slidedat.ini`, by name
                // in `Registry::open`.
                Box::new(wsi_dec::WsiDecoder(wsi_dec::Kind::Mrxs)),
                // Hamamatsu VMS / VMU: the INI index's first section name.
                Box::new(wsi_dec::WsiDecoder(wsi_dec::Kind::Vms)),
                // Looks inside gzip/bzip2/xz files: after SVG (SVGZ is gzip, chosen by extension).
                Box::new(xcf_dec::XcfDecoder),
                // "%PDF-" (probe_file: within the first KiB); .ai without it opens as PostScript.
                Box::new(pdf_dec::PdfDecoder),
                // DOS EPS binary header, or a "%!PS-Adobe-… EPSF" first line.
                Box::new(pdf_dec::EpsDecoder),
                // Film scans: "SDPX"/"XPDS"; Cineon's magic number in either byte order.
                Box::new(whole_dec::WholeDecoder(Format::Dpx)),
                Box::new(whole_dec::WholeDecoder(Format::Cineon)),
                // "id=ImageMagick" in the first header line.
                Box::new(whole_dec::WholeDecoder(Format::Miff)),
                // The JBIG2 file header "\x97JB2\r\n\x1a\n".
                Box::new(whole_dec::WholeDecoder(Format::Jbig2)),
                // JPEG XS: the SOC and CAP markers of a codestream, or the "JXS " signature
                // box of an ISO/IEC 21122-3 file.
                Box::new(whole_dec::WholeDecoder(Format::JpegXs)),
                // WSQ's SOI marker followed by a table or comment marker.
                Box::new(whole_dec::WholeDecoder(Format::Wsq)),
                // "AT&TFORM" + DJVU/DJVM/DJVI/THUM.
                Box::new(docpage_dec::DjvuDecoder),
                // A ZIP whose directory names fixed pages (`.fpage`, `.fdseq`); else by extension.
                Box::new(docpage_dec::XpsDecoder),
                // BEGIN METAFILE (binary) or "BEGMF" (clear text).
                Box::new(docpage_dec::CgmDecoder),
                // A ZIP naming `visio/document.xml`, or a compound file with a
                // `VisioDocument` stream.
                Box::new(drawdoc_dec::VisioDecoder),
                // FF "WPC", product 1, file type 0x16.
                Box::new(drawdoc_dec::WpgDecoder),
                // ZIPs naming `document.json`, `meta.json` and `pages/`.
                Box::new(drawdoc_dec::SketchDecoder),
                // "fig-kiwi", or a ZIP holding `canvas.fig`.
                Box::new(drawdoc_dec::FigmaDecoder),
                // A ZIP whose `mimetype` member is an OpenDocument type, or flat XML.
                Box::new(drawdoc_dec::OdfDecoder),
                // Word, Excel and PowerPoint packages (`[Content_Types].xml` and a `word/`,
                // `xl/` or `ppt/` main part), 97–2003 compound files (`WordDocument`,
                // `Workbook`, `PowerPoint Document`; encrypted packages) and iWork ZIPs.
                Box::new(office_dec::OfficeDecoder(specular_office::Family::Ooxml)),
                Box::new(office_dec::OfficeDecoder(specular_office::Family::Legacy)),
                Box::new(office_dec::OfficeDecoder(specular_office::Family::Iwork)),
                // A ZIP whose first member is `mimetype` = application/epub+zip, or whose
                // container names an OPF package.
                Box::new(epub_dec::EpubDecoder),
                // Kindle and Mobipocket: `BOOKMOBI` / `TEXtREAd` at 60 (KFX by its container
                // magic, refused); FB2: a `FictionBook` root, or a ZIP whose first member is
                // an `.fb2`.
                Box::new(epub_dec::EbookDecoder(Format::Mobi)),
                Box::new(epub_dec::EbookDecoder(Format::Fb2)),
                // "OTTO", "ttcf", "wOFF", "wOF2"; 0x00010000 or "true" with a table count.
                Box::new(font_dec::FontDecoder(font_dec::Kind::OpenType)),
                Box::new(font_dec::FontDecoder(font_dec::Kind::Collection)),
                Box::new(font_dec::FontDecoder(font_dec::Kind::Woff)),
                Box::new(font_dec::FontDecoder(font_dec::Kind::Woff2)),
                // PFB's 0x80 0x01 segment header; PFA's "%!PS-AdobeFont-1" or "%!FontType1".
                Box::new(font_dec::FontDecoder(font_dec::Kind::Type1)),
                // Bitmap fonts: "\1fcp", STARTFONT, PSF's magic, an NE executable with
                // RT_FONT resources, or a plausible FNT header.
                Box::new(bitmap_font_dec::BitmapFontDecoder),
                // Jupyter notebooks: a JSON object with cells (or worksheets) and nbformat.
                Box::new(notebook::NotebookDecoder),
                // draw.io by its mxfile / mxGraphModel root; Excalidraw by "type": "excalidraw".
                Box::new(drawdoc_dec::DiagramDecoder(Format::Drawio)),
                Box::new(drawdoc_dec::DiagramDecoder(Format::Excalidraw)),
                // Lottie: a JSON object starting with "v", "fr", "ip", "op", "w", "h", or a
                // ZIP with manifest.json and animations/.
                Box::new(lottie_dec::LottieDecoder),
                // Flash: "FWS", "CWS" or "ZWS" and a version.
                Box::new(drawdoc_dec::SwfDecoder),
                // DVI: `pre` (247) and format 2.
                Box::new(dvi_dec::DviDecoder),
                Box::new(font_dec::FontDecoder(font_dec::Kind::TrueType)),
                // Weak or no signatures: last. WBMP is only recognised by its exact size.
                // FORM ILBM / PBM / ACBM.
                Box::new(iff_dec::IffDecoder),
                // Version 2 signature at offset 10 or 522; version 1 by extension only.
                Box::new(pict_dec::PictDecoder),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Farbfeld)),
                Box::new(legacy_dec::LegacyDecoder(
                    specular_legacy::Format::SunRaster,
                )),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Sgi)),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Fli)),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Dcx)),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Pcx)),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Xpm)),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Xbm)),
                Box::new(legacy_dec::LegacyDecoder(specular_legacy::Format::Wbmp)),
                // Retro and game rasters. Signatures: Sixel's device control string, "Xcur",
                // Softimage PIC's magic, Utah RLE's magic, "BLP1"/"BLP2", a QuickTime image's
                // `idsc`/`idat` atom. The rest have none and go by extension, checked when
                // opened; GEM IMG only for `.img` files (`Registry::open_img`).
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Sixel)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Xcursor)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Softimage)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::UtahRle)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Blp)),
                Box::new(retro_dec::QtifDecoder),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Avs)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::MacPaint)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Gem)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Degas)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Neochrome)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Koala)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::ZxScreen)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::DrHalo)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::AliasPix)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Rla)),
                Box::new(retro_dec::RetroDecoder(retro_dec::Kind::Wal)),
                // 8-bit home computer screens: AMSDOS (CPC) and BSAVE (MSX) headers, else by
                // extension and size; Photo CD's "PCD_IPI" in its second sector.
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::CpcScreen)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::MsxScreen)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::AppleHgr)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::AppleDhr)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::BbcScreen)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::AtariMic)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::AtariGr8)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::AtariGr9)),
                Box::new(retro2_dec::Retro2Decoder(specular_retro2::Kind::PhotoCd)),
                // XWD: the header size and file version (6 or 7) in either byte order, with
                // sizes that fit.
                Box::new(whole_dec::WholeDecoder(Format::Xwd)),
                // By signature: "GIMP Palette", "ASEF" 1.0, "GIMP Gradient", "GIMP" (brush)
                // or "GPAT" at byte 20, a brush pipe's two text lines and a brush header,
                // "DanM"/"LinS" with a header checksum, GD 2's 0xFFFE/0xFFFF id, "gd2\0",
                // Pixar's 80 E8 00 00, "Image type:", "P7 332", "PG ML"/"PG LM", a CALS
                // record and its rpelcnt.
                Box::new(whole_dec::WholeDecoder(Format::GimpPalette)),
                // Aseprite: 0xA5E0 at byte 4 and a known colour depth.
                Box::new(aseprite_dec::AsepriteDecoder),
                Box::new(whole_dec::WholeDecoder(Format::SwatchExchange)),
                Box::new(whole_dec::WholeDecoder(Format::GimpGradient)),
                Box::new(whole_dec::WholeDecoder(Format::GimpBrush)),
                Box::new(whole_dec::WholeDecoder(Format::GimpBrushPipe)),
                Box::new(whole_dec::WholeDecoder(Format::GimpPattern)),
                Box::new(whole_dec::WholeDecoder(Format::MsPaint)),
                Box::new(whole_dec::WholeDecoder(Format::Gd)),
                Box::new(whole_dec::WholeDecoder(Format::Gd2)),
                Box::new(whole_dec::WholeDecoder(Format::PixarPicture)),
                Box::new(whole_dec::WholeDecoder(Format::LabEyeIm)),
                Box::new(whole_dec::WholeDecoder(Format::XvThumbnail)),
                Box::new(whole_dec::WholeDecoder(Format::Pgx)),
                Box::new(whole_dec::WholeDecoder(Format::Cals)),
                Box::new(exr_dec::ExrDecoder),
                Box::new(radiance_dec::RadianceDecoder(Format::Hdr)),
                // "PF", "Pf" or "PF4" and whitespace.
                Box::new(radiance_dec::RadianceDecoder(Format::Pfm)),
                // "NRRD000", MetaImage key lines, NIfTI magic; Analyze by its header's
                // size and fields (a `.hdr` that is not Radiance's).
                // CIFTI-2 (NIfTI-2 with its XML extension) ahead of NIfTI; GIFTI's XML,
                // TrackVis's "TRACK", MRtrix's "mrtrix tracks".
                Box::new(neuro_dec::NeuroDecoder(Format::Cifti)),
                Box::new(neuro_dec::NeuroDecoder(Format::Gifti)),
                Box::new(neuro_dec::NeuroDecoder(Format::Trk)),
                Box::new(neuro_dec::NeuroDecoder(Format::Tck)),
                Box::new(medvol_dec::MedvolDecoder(Format::Nifti)),
                Box::new(medvol_dec::MedvolDecoder(Format::Analyze)),
                Box::new(medvol_dec::MedvolDecoder(Format::Nrrd)),
                Box::new(medvol_dec::MedvolDecoder(Format::MetaImage)),
                // FreeSurfer MGH: version 1, plausible sizes and type, unit direction cosines.
                Box::new(medvol_dec::MedvolDecoder(Format::Mgh)),
                // MINC2 (an HDF5 file with a /minc-2.0 group) and MINC1 (NetCDF classic with
                // an `image` variable and MINC dimensions), ahead of the HDF5 and NetCDF readers.
                Box::new(medvol_dec::MedvolDecoder(Format::Minc)),
                // MRtrix "mrtrix image".
                Box::new(medvol_dec::MedvolDecoder(Format::Mif)),
                // Philips PAR/REC: the PAR header's "DATA DESCRIPTION FILE" banner (a `.REC`
                // by name, through the PAR beside it); ECAT 7: "MATRIX7" at the start.
                Box::new(medvol_dec::MedvolDecoder(Format::ParRec)),
                Box::new(medvol_dec::MedvolDecoder(Format::Ecat)),
                // Industrial CT: Amira's "# AmiraMesh" / "# Avizo" line, a SkyScan log's
                // sections, X-Tek and VGI INI headers; Xradia compound files (the CFB
                // signature and an ImageInfo storage) ahead of other OLE readers.
                Box::new(medvol_dec::MedvolDecoder(Format::Amira)),
                Box::new(medvol_dec::MedvolDecoder(Format::SkyScan)),
                Box::new(medvol_dec::MedvolDecoder(Format::Xtek)),
                Box::new(medvol_dec::MedvolDecoder(Format::Vgi)),
                Box::new(medvol_dec::MedvolDecoder(Format::Txm)),
                // Headerless `.raw` / `.vol`: by name only, once a layout is known (or a
                // header beside it describes it).
                Box::new(medvol_dec::MedvolDecoder(Format::RawVolume)),
                // Zeiss Cirrus OCT raw `.img` exports: by name and size only (no header).
                Box::new(medvol_dec::MedvolDecoder(Format::Cirrus)),
                // "SIMPLE  =" and "XISF0100" signatures; fpack first: a FITS file whose first
                // extension is a tile-compressed image (`ZIMAGE = T`) after an empty primary.
                Box::new(fits_dec::FitsDecoder { fpack: true }),
                Box::new(fits_dec::FitsDecoder { fpack: false }),
                // Planetary images: "LBLSIZE=" (VICAR); "PDS_VERSION_ID", "ODL_VERSION_ID" or
                // a PDS SFDU label (PDS3, attached or detached); XML with a PDS4 `Product_`
                // root in the first 8 KiB.
                Box::new(planetary_dec::PlanetaryDecoder(planetary_dec::Kind::Vicar)),
                Box::new(planetary_dec::PlanetaryDecoder(planetary_dec::Kind::Pds3)),
                Box::new(planetary_dec::PlanetaryDecoder(planetary_dec::Kind::Pds4)),
                Box::new(xisf_dec::XisfDecoder),
                // "#ASDF " and a version digit.
                Box::new(asdf_dec::AsdfDecoder),
                // "root" and a version.
                Box::new(root_dec::RootDecoder),
                // A casacore `table.dat` with a `coords` keyword (or a tiled storage
                // manager's `table.f0`); the folder itself in `Registry::open`.
                Box::new(casa_dec::CasaDecoder),
                // TIA's byte order mark and series ID 0x0197 (SER videos say "LUCAM-RECORDER").
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Tia)),
                Box::new(ser_dec::SerDecoder),
                // An HDF5 file with Zygo's `MetaData` table and `Measurement` group.
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Datx)),
                // NumPy's "\x93NUMPY"; MATLAB's "MATLAB 5.0 MAT-file" / "MATLAB 7.3 MAT-file"
                // text header (v7.3 is HDF5 after a 512-byte user block, so ahead of HDF5;
                // level 4 has no signature: by extension); TDMS "TDSm"; VTK's legacy and XML
                // headers; a Zarr metadata document (a JSON object naming "zarr_format").
                Box::new(array_dec::ArrayDecoder(array_dec::Kind::Npy)),
                Box::new(array_dec::ArrayDecoder(array_dec::Kind::Mat)),
                Box::new(tdms_dec::TdmsDecoder),
                Box::new(vtk_dec::VtkDecoder),
                Box::new(array_dec::ArrayDecoder(array_dec::Kind::ZarrMeta)),
                // "\x89HDF\r\n\x1a\n" (also after a user block) and "CDF\x01/\x02/\x05".
                Box::new(hdf5_dec::Hdf5Decoder),
                Box::new(hdf5_dec::NetcdfDecoder),
                // "GWYP"; the DM version word and byte order (weak, so last); MRC's "MAP "
                // at byte 208; Igor waves by extension only.
                Box::new(em_dec::MicroDecoder(em_dec::Kind::Gwy)),
                Box::new(em_dec::MicroDecoder(em_dec::Kind::Mrc)),
                // DeltaVision: the Priism ID −16224 at byte 96, ahead of DV video (`.dv`).
                Box::new(em_dec::MicroDecoder(em_dec::Kind::Dv)),
                Box::new(em_dec::MicroDecoder(em_dec::Kind::Dm)),
                Box::new(em_dec::MicroDecoder(em_dec::Kind::Ibw)),
                // SPIDER: a header whose record sizes add up (likely, by extension).
                Box::new(em_dec::MicroDecoder(em_dec::Kind::Spider)),
                // Materials science by signature: Bruker's SFS container "AAMVHFSS", NT-MDT's
                // and MetroPro's magic numbers, Keyence "VK4_", Alicona "AliconaImaging",
                // Digital Surf "DIGITAL SURF"/"DSCOMPRESSED", Nanonis ":NANONIS_VERSION:",
                // Nanoscope's "\*File list"; EBSD maps by their header lines ("Channel Text
                // File", an .ang's "# TEM_PIXperUM"…). Sensofar .plu has no signature: last.
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Bcf)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::NtMdt)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::MetroPro)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Vk4)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Al3d)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Sur)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Nanonis)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Nanoscope)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Ang)),
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Ctf)),
                // Leica LIF / LOF: the 0x70 block magic with an XML header (or LOF's
                // "LMS_Object_File"). ND2 comes ahead of JPEG 2000 (see there).
                Box::new(vendor_dec::VendorDecoder(vendor_dec::Kind::Lif)),
                // Olympus OIR ("OLYMPUSRAWFORMAT"); Zeiss CZI ("ZISRAWFILE", libCZI in the
                // decode host); Andor SIF ("Andor Technology Multi-Channel File").
                Box::new(vendor_dec::VendorDecoder(vendor_dec::Kind::Oir)),
                // ICS: two separator bytes, then "ics_version" (an `.ids` by extension).
                Box::new(vendor_dec::VendorDecoder(vendor_dec::Kind::Ics)),
                Box::new(czi_dec::CziDecoder),
                Box::new(spectro_dec::SpectroDecoder(spectro_dec::Kind::Sif)),
                // Princeton Instruments SPE: no signature; the header's sample type, sizes and
                // frame count against the file's size, or a LightField XML footer. Weak, so
                // after every signed format.
                Box::new(spectro_dec::SpectroDecoder(spectro_dec::Kind::Spe)),
                // Gaussian cube files by their header lines (`.cube` is shared with colour
                // lookup tables, so only content counts); SRTM tiles by extension and size.
                Box::new(grid_dec::GridDecoder(Format::Cube)),
                Box::new(grid_dec::GridDecoder(Format::Srtm)),
                // Diffraction frames by their headers within the first 16 KiB, also inside a
                // gzip stream: "###CBF" or a CIF binary section; "{" and `Key = value ;`
                // (EDF) or `HEADER_BYTES=` (SMV); Bruker's "FORMAT :" record; "R-AXIS".
                Box::new(xtal_dec::XtalDecoder(specular_xtal::Kind::Cbf)),
                Box::new(xtal_dec::XtalDecoder(specular_xtal::Kind::Smv)),
                Box::new(xtal_dec::XtalDecoder(specular_xtal::Kind::Edf)),
                Box::new(xtal_dec::XtalDecoder(specular_xtal::Kind::Bruker)),
                Box::new(xtal_dec::XtalDecoder(specular_xtal::Kind::Raxis)),
                // Molecular structures by content within the first 4 KiB (inflated when
                // gzip): PDB records, a CIF data block with atom sites or a cell (after CBF,
                // whose imgCIF has neither), an SDF counts line or "M  END", an XYZ count
                // line over element lines (before point-cloud XYZ, chosen by extension).
                Box::new(mol_dec::MolDecoder(specular_mol::MolFormat::Pdb)),
                Box::new(mol_dec::MolDecoder(specular_mol::MolFormat::Mmcif)),
                Box::new(mol_dec::MolDecoder(specular_mol::MolFormat::Cif)),
                Box::new(mol_dec::MolDecoder(specular_mol::MolFormat::Sdf)),
                Box::new(mol_dec::MolDecoder(specular_mol::MolFormat::Xyz)),
                // MOL2 by its "@<TRIPOS>" section; GRO by its count line and fixed columns.
                Box::new(mol_dec::MolDecoder(specular_mol::MolFormat::Mol2)),
                Box::new(mol_dec::MolDecoder(specular_mol::MolFormat::Gro)),
                // Georeferenced grids: "EHFA_HEADER_TAG", DTED's "UHL1" label, an ASCII grid's
                // header keys (`ncols`…, then a number), an ENVI header ("ENVI") or an ESRI
                // label (NROWS and NCOLS keys) opened as the `.hdr`; raw data beside such a
                // header is found by `Registry::open`.
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Hfa)),
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Dted)),
                // USGS DEM: the A record's fixed-column codes (level, pattern, units).
                Box::new(georaster_dec::GeoRasterDecoder(
                    georaster_dec::Kind::UsgsDem,
                )),
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Asc)),
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Envi)),
                // ER Mapper headers (`DatasetHeader Begin`), SAGA grid headers (their keys,
                // or a ZIP whose first member is a `.sgrd`), Arc/Info grids (`GRID1.2` in
                // `hdr.adf`, or their folder), BSB/KAP charts (a `BSB/` or `NOS/` line before
                // Ctrl-Z) and GDAL virtual rasters (an XML root `VRTDataset`).
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Ers)),
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Saga)),
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Aig)),
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Bsb)),
                Box::new(georaster_dec::GeoRasterDecoder(georaster_dec::Kind::Vrt)),
                // "GRIB" and edition 1 or 2 at byte 7.
                Box::new(grib_dec::GribDecoder),
                // An OMERO image descriptor: `{"specular-omero":` (opened by the
                // application's registered opener, see `set_omero_opener`).
                Box::new(omero_dec::OmeroDecoder),
                // A remote Zarr link: `{"specular-zarr":` (its store opened by the
                // application's registered opener, see `set_zarr_store_opener`).
                Box::new(zarr_url_dec::ZarrLinkDecoder),
                // Movies: an `ftyp` with an MP4, QuickTime or 3GPP brand (HEIF and AVIF
                // brands were taken above), or a QuickTime file starting with a classic
                // atom (`moov`, `mdat`, `wide`, `free`, `skip`, `pnot`) by extension. Opened by
                // the application's registered opener (`set_video_opener`).
                // M4A and M4B (and FairPlay M4P, refused): the `ftyp` major brand. Ahead of
                // MP4, whose compatible brands they list.
                Box::new(video_dec::SoundFileDecoder(Format::M4a)),
                Box::new(video_dec::VideoDecoder(Format::Mp4)),
                Box::new(video_dec::VideoDecoder(Format::QuickTime)),
                Box::new(video_dec::VideoDecoder(Format::ThreeGpp)),
                // Matroska and WebM: an EBML header with that DocType. Transport streams: the
                // sync byte every 188 bytes (192 for AVCHD and Blu-ray), five in a row.
                Box::new(video_dec::VideoDecoder(Format::Matroska)),
                Box::new(video_dec::VideoDecoder(Format::WebM)),
                Box::new(video_dec::VideoDecoder(Format::MpegTs)),
                // MPEG programme streams: a pack header; bare MPEG video: a sequence header
                // with a picture size; AVI: RIFF with the AVI form type.
                Box::new(video_dec::VideoDecoder(Format::MpegPs)),
                Box::new(video_dec::VideoDecoder(Format::MpegVideo)),
                Box::new(video_dec::VideoDecoder(Format::Avi)),
                // Ogg: the capture pattern and a Theora (or other video) first packet; IVF:
                // `DKIF`.
                Box::new(video_dec::VideoDecoder(Format::Ogg)),
                Box::new(video_dec::VideoDecoder(Format::Ivf)),
                // MXF: a header partition pack (after a run-in of up to 64 KiB).
                Box::new(video_dec::VideoDecoder(Format::Mxf)),
                // FLV: `FLV` and version 1; ASF: the header object's GUID; raw DV: a header
                // DIF block (section type 0, sequence 0) every 80 bytes' block pattern.
                Box::new(video_dec::VideoDecoder(Format::Flv)),
                Box::new(video_dec::VideoDecoder(Format::Asf)),
                // WTV: the file system's GUID at 0.
                Box::new(video_dec::VideoDecoder(Format::Wtv)),
                Box::new(video_dec::VideoDecoder(Format::Dv)),
                // YUV4MPEG2 by its "YUV4MPEG2 " line; CD+G by its packets.
                Box::new(video_dec::VideoDecoder(Format::Y4m)),
                Box::new(video_dec::VideoDecoder(Format::Cdg)),
                // Raw H.264 / HEVC / AV1: a start code (or a temporal delimiter) at the
                // very start, a NAL type that opens a stream and a parameter set (or
                // sequence header) soon after. Likely, by extension.
                Box::new(video_dec::VideoDecoder(Format::VideoStream)),
                // APV: an access-unit size, then "aPv1".
                Box::new(video_dec::VideoDecoder(Format::Apv)),
                // PlayStation movies: an STR header (0160h, 8001h) in one of the first
                // sectors.
                Box::new(video_dec::VideoDecoder(Format::PsxStr)),
                // Sound files (shown as their cover and waveform; video_dec::sound_format):
                // WAVE/RF64 and AIFF by their RIFF/FORM types, Ogg by its first packet,
                // FLAC, DTS and AC-3 by their sync words, AMR by its "#!AMR" line, ADTS and
                // MPEG audio by two frame headers in a row (after any ID3v2 tag).
                Box::new(video_dec::SoundFileDecoder(Format::Wave)),
                Box::new(video_dec::SoundFileDecoder(Format::Aiff)),
                Box::new(video_dec::SoundFileDecoder(Format::OggAudio)),
                Box::new(video_dec::SoundFileDecoder(Format::Flac)),
                Box::new(video_dec::SoundFileDecoder(Format::Dts)),
                Box::new(video_dec::SoundFileDecoder(Format::Ac3)),
                Box::new(video_dec::SoundFileDecoder(Format::Amr)),
                // QCP by its RIFF type `QLCM`.
                Box::new(video_dec::SoundFileDecoder(Format::Qcp)),
                Box::new(video_dec::SoundFileDecoder(Format::WavPack)),
                Box::new(video_dec::SoundFileDecoder(Format::Ape)),
                // Musepack by its "MPCK" / "MP+" start.
                Box::new(video_dec::SoundFileDecoder(Format::Mpc)),
                // AU, CAF, Wave64, VOC, 8SVX, AVR, SPHERE, IRCAM, TTA, Shorten, DSF,
                // DSDIFF and tracker modules, each by its signature.
                Box::new(video_dec::SoundFileDecoder(Format::Au)),
                Box::new(video_dec::SoundFileDecoder(Format::Caf)),
                Box::new(video_dec::SoundFileDecoder(Format::W64)),
                Box::new(video_dec::SoundFileDecoder(Format::Voc)),
                Box::new(video_dec::SoundFileDecoder(Format::Svx)),
                Box::new(video_dec::SoundFileDecoder(Format::Avr)),
                Box::new(video_dec::SoundFileDecoder(Format::Sphere)),
                Box::new(video_dec::SoundFileDecoder(Format::Ircam)),
                Box::new(video_dec::SoundFileDecoder(Format::Tta)),
                Box::new(video_dec::SoundFileDecoder(Format::Shorten)),
                Box::new(video_dec::SoundFileDecoder(Format::Dsf)),
                Box::new(video_dec::SoundFileDecoder(Format::Dff)),
                Box::new(video_dec::SoundFileDecoder(Format::Tracker)),
                Box::new(video_dec::SoundFileDecoder(Format::Aac)),
                Box::new(video_dec::SoundFileDecoder(Format::Mp3)),
                // Instruments and MIDI by their RIFF types and magic (`sound_ident`); SFZ by
                // its headers in text; HTK and raw GSM by their structure over the whole
                // file; signed-linear `.sln` by extension only.
                Box::new(video_dec::SoundFileDecoder(Format::Midi)),
                Box::new(video_dec::SoundFileDecoder(Format::SoundFont)),
                Box::new(video_dec::SoundFileDecoder(Format::Dls)),
                Box::new(video_dec::SoundFileDecoder(Format::Xi)),
                Box::new(video_dec::SoundFileDecoder(Format::Sfz)),
                Box::new(video_dec::SoundFileDecoder(Format::Htk)),
                Box::new(video_dec::SoundFileDecoder(Format::Gsm)),
                // QOA: "qoaf" and a frame header.
                Box::new(video_dec::SoundFileDecoder(Format::Qoa)),
                Box::new(video_dec::SoundFileDecoder(Format::Sln)),
                // HDF4 (and HDF-EOS2): the magic 0e 03 13 01.
                Box::new(hdf4_dec::Hdf4Decoder),
                // SEG-Y by its textual and binary headers (a known sample format and a
                // sample count behind 3200 bytes of card images); Seismic Unix by extension.
                Box::new(segy_dec::SegyDecoder(Format::Segy)),
                Box::new(segy_dec::SegyDecoder(Format::SeismicUnix)),
                // imzML: an mzML document using the imaging (IMS) vocabulary.
                Box::new(imzml_dec::ImzmlDecoder),
                // Reflectance transformation images: "PTM_1." magic; an HSH `.rti` by its
                // text header ("#" comments, type 3, size, terms); a Relight `info.json` by
                // its keys (and its folder, see `Registry::open`).
                Box::new(rti_dec::RtiDecoder(Format::Ptm)),
                Box::new(rti_dec::RtiDecoder(Format::Rti)),
                Box::new(rti_dec::RtiDecoder(Format::Relight)),
                // "0\nSECTION" (ASCII, after optional 999 comments) or the binary sentinel.
                Box::new(dxf_dec::DxfDecoder),
                // Content within the first 8 KiB (PJL headers); `.plt` and kin otherwise,
                // if most of the file parses as HP-GL.
                Box::new(dxf_dec::HpglDecoder),
                // A HEADER record then BGNLIB; OASIS ("%SEMI-OASIS") is recognised to be
                // refused with a message.
                Box::new(gds_dec::GdsDecoder),
                // Electronics: KiCad boards and schematics by their leading `(kicad_pcb` /
                // `(kicad_sch` (KiCad 4/5 files recognised to be refused by name); Gerber by
                // its format statement, attributes or comments; Excellon by its M48 header
                // or drill dialect; a ZIP of a board's Gerbers by its file names (folders:
                // see `Registry::open`).
                Box::new(pcb_dec::KicadDecoder(Format::KicadPcb)),
                Box::new(pcb_dec::KicadDecoder(Format::KicadSch)),
                Box::new(pcb_dec::GerberDecoder(Format::Gerber)),
                Box::new(pcb_dec::GerberDecoder(Format::Excellon)),
                Box::new(pcb_dec::BoardDecoder),
                // ODB++ jobs (folders, see `Registry::open`; `.tgz`/`.tar`/`.zip` by the job's
                // paths) and IPC-2581 by its root element.
                Box::new(pcb_dec::OdbDecoder),
                Box::new(pcb_dec::Ipc2581Decoder),
                // LandXML by its root element.
                Box::new(landxml_dec::LandXmlDecoder),
                // Sentinel-2 products: the product metadata by its root element, a ZIP by the
                // SAFE folder's metadata inside (folders: see `Registry::open`).
                Box::new(s2_dec::Sentinel2Decoder),
                // Lab spectra: JCAMP-DX by `##TITLE=`; mzML and mzXML by their root element
                // (imzML, above, keeps the imaging ones); Bruker NMR folders (see
                // `Registry::open`) and their data files by name; SPC by its header, last.
                Box::new(spectra_dec::SpectraDecoder(Format::Jcamp)),
                Box::new(spectra_dec::SpectraDecoder(Format::Mzml)),
                Box::new(spectra_dec::SpectraDecoder(Format::Mzxml)),
                Box::new(spectra_dec::SpectraDecoder(Format::BrukerNmr)),
                Box::new(spectra_dec::SpectraDecoder(Format::Spc)),
                // 3D meshes: PLY ("ply\n") and GLB ("glTF") by signature; STL, OBJ, glTF JSON
                // and 3MF (a ZIP) by extension only.
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Ply)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Glb)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Stl)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Obj)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Gltf)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::ThreeMf)),
                // Scene formats: USD by "#usda" or "PXR-USDC", binary FBX by its header;
                // USDZ, COLLADA, 3DS, LightWave and ASCII FBX by extension.
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Usd)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Fbx)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Usdz)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Dae)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Zae)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::ThreeDs)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Lwo)),
                // X3D (XML) and AMF by extension; VRML ("#VRML", "#X3D") and Rhino ("3D
                // Geometry File Format") by signature too.
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Vrml)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Rhino)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::X3d)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Amf)),
                // "OFF" (with ST/C/N/4 prefixes) on the first line.
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Off)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Md2)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Ms3d)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Vox)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Ac3d)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::DirectX)),
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Bvh)),
                // Alembic: "Ogawa" (HDF5-based archives are refused by extension).
                Box::new(mesh_dec::MeshDecoder(specular_mesh::MeshFormat::Alembic)),
                // CAD exchange files ("ISO-10303-21" for STEP and IFC; IGES by extension):
                // STEP and IGES through OpenCASCADE in the decode host, IFC by its own
                // reader; DWG through the ODA File Converter when allowed.
                Box::new(cad_dec::CadExchangeDecoder),
                Box::new(cad_dec::DwgDecoder),
                // Point clouds: LAS/LAZ ("LASF"), E57 ("ASTM-E57") and PCD by signature;
                // XYZ, PTS and PTX by extension. PLY without faces comes through the mesh decoder.
                Box::new(points_dec::PointsDecoder(specular_points::PointFormat::Las)),
                Box::new(points_dec::PointsDecoder(specular_points::PointFormat::Laz)),
                Box::new(points_dec::PointsDecoder(specular_points::PointFormat::E57)),
                Box::new(points_dec::PointsDecoder(specular_points::PointFormat::Pcd)),
                Box::new(points_dec::PointsDecoder(specular_points::PointFormat::Xyz)),
                Box::new(points_dec::PointsDecoder(specular_points::PointFormat::Pts)),
                Box::new(points_dec::PointsDecoder(specular_points::PointFormat::Ptx)),
                // Gaussian splats: version 4 `.spz` by "NGSP"; older `.spz` (gzip), `.splat`
                // and `.ksplat` (refused) by extension. Splat PLY comes through the mesh
                // decoder.
                Box::new(splat_dec::SplatDecoder(splat_dec::Kind::Spz)),
                Box::new(splat_dec::SplatDecoder(splat_dec::Kind::Splat)),
                Box::new(splat_dec::SplatDecoder(splat_dec::Kind::Ksplat)),
                // Vector maps: the Shapefile header (file code 9994, version 1000), SQLite
                // with a GeoPackage application id, a ZIP whose first member is a .kml, the
                // KML and GPX namespaces, a JSON object with a GeoJSON type, all within the
                // first 4 KiB.
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Shapefile)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::GeoPackage)),
                // FlatGeobuf ("fgb\x03"), Parquet ("PAR1"), Arrow IPC files ("ARROW1"), FIT
                // (".FIT" at byte 8) and OSM PBF (an "OSMHeader" blob) by signature; a File
                // Geodatabase is a folder (or one of its tables, by extension).
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::FlatGeobuf)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::GeoParquet)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::GeoArrow)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Fit)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::OsmPbf)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::FileGdb)),
                // S-57 charts: an ISO 8211 descriptive record ("3LE1") naming DSID.
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::S57)),
                // NITF ("NITF02.10", "NSIF01.00", "NITF02.00"); ECW and MrSID are recognised to
                // be refused by their SDKs' licences (`vendor_gis_dec`).
                Box::new(nitf_dec::NitfDecoder),
                Box::new(vendor_gis_dec::EcwDecoder),
                Box::new(vendor_gis_dec::MrSidDecoder),
                Box::new(tiles_dec::TilesDecoder(tiles_dec::Kind::MbTiles)),
                Box::new(tiles_dec::TilesDecoder(tiles_dec::Kind::PmTiles)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Kmz)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Kml)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Gpx)),
                // MapInfo ("!table" / a Version line then Columns), TCX, OSM XML and GML by
                // their first lines and namespaces.
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::MapInfoTab)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::MapInfoMif)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Tcx)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Osm)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Gml)),
                // IGC flight logs (an A record, then H records) and NMEA logs (sentences
                // whose checksums are right); TopoJSON (`"type":"Topology"`) before GeoJSON.
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Igc)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::Nmea)),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::TopoJson)),
                // STAC items are GeoJSON features: by `stac_version`, before GeoJSON.
                Box::new(stac_dec::StacDecoder),
                // 3D Tiles tilesets: JSON with `asset`, `geometricError` and `root`.
                Box::new(tiles3d_dec::Tiles3dDecoder),
                Box::new(gis_dec::GisDecoder(specular_gis::VectorKind::GeoJson)),
                // PlayStation TIM: its tag word, a known pixel mode and blocks that fit; JBIG: a
                // plausible BIH (no magic). Both weak, so after every signature.
                Box::new(whole_dec::WholeDecoder(Format::Tim)),
                // No signatures, likely for their own extensions only: Photoshop swatches
                // (a version and records that fit), Adobe colour tables (768 or 772 bytes),
                // IM Tools (key lines before a form feed), McIDAS AREA (words 0 and 4).
                Box::new(whole_dec::WholeDecoder(Format::PhotoshopSwatches)),
                Box::new(whole_dec::WholeDecoder(Format::AdobeColorTable)),
                Box::new(whole_dec::WholeDecoder(Format::ImTools)),
                Box::new(whole_dec::WholeDecoder(Format::McIdasArea)),
                Box::new(whole_dec::WholeDecoder(Format::Jbig)),
                // Sensofar .plu: a plausible header (a date, sizes and pitch), the weakest.
                Box::new(matsci_dec::MatsciDecoder(matsci_dec::Kind::Plu)),
                // Playlists: `#EXTM3U`, `[playlist]`, or an XML `playlist` in the XSPF
                // namespace; plain M3U by name only.
                Box::new(playlist_dec::PlaylistDecoder),
                // Text-like files (`text_dec`). By signature: RTF ("{\rtf"), WebVTT
                // ("WEBVTT"), SCC ("Scenarist_SCC V1.0"), a VobSub index ("# VobSub index
                // file"), ASS/SSA ("[Script Info]"), PGS (two "PG" segment headers), TTML (a
                // `tt` root in the first 4 KiB). By content checks: SubRip (a number line,
                // then a time line), MicroDVD (`{n}{n}` lines). A VobSub `.sub` by name and
                // the `.idx` beside it (`Registry::open`). By extension only, after every
                // signature: LRC, CSV, TSV, Markdown, NFO and plain text (`.log` after the
                // SkyScan reconstruction logs).
                Box::new(text_dec::TextDecoder(text_dec::Kind::Rtf)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::WebVtt)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Scc)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::VobSub)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Ssa)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Pgs)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Ttml)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::SubRip)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::MicroDvd)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Lrc)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Csv)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Tsv)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Markdown)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Nfo)),
                Box::new(text_dec::TextDecoder(text_dec::Kind::Plain)),
            ],
            extensions: Default::default(),
        };
        r.extensions = r
            .decoders
            .iter()
            .flat_map(|d| d.extensions().iter().copied())
            .collect();
        r
    }
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Detect the format from the first bytes of a file ([`Registry::open`] also looks
    /// further into the file, see [`Decoder::probe_file`]).
    pub fn probe(&self, head: &[u8]) -> Option<&dyn Decoder> {
        self.decoders
            .iter()
            .find(|d| d.probe(head))
            .map(|d| d.as_ref())
    }

    /// Every extension a decoder claims, with the formats claiming it in probe order (the
    /// shared-extension audit, the "Open as…" menu).
    pub fn extension_claims(&self) -> std::collections::BTreeMap<&'static str, Vec<Format>> {
        let mut m = std::collections::BTreeMap::<&'static str, Vec<Format>>::new();
        for d in &self.decoders {
            for e in d.extensions() {
                let v = m.entry(*e).or_default();
                if !v.contains(&d.format()) {
                    v.push(d.format());
                }
            }
        }
        m
    }

    /// Whether a path has an extension any decoder handles (folder scans).
    pub fn supports_extension(&self, path: &std::path::Path) -> bool {
        path_extensions(path)
            .iter()
            .flatten()
            .any(|e| self.extensions.contains(e.as_str()))
    }

    /// Whether a file on disk belongs in folder listings: a supported extension, a DICOM
    /// file without one, a `.map`/`.map.gz` that is a CCP4/MRC map, a `.cube` that is a
    /// Gaussian cube, a `.bag` that is a BAG grid, a `.img`, `.edf` or numbered file that is a
    /// diffraction frame, a `.v` that is an ECAT 7 image, an `.img` named and sized like a
    /// Zeiss Cirrus OCT export or a `.log`
    /// that is a SkyScan reconstruction's (read and cached, see
    /// [`sniff`]); not a `.raw`, `.vol` or `.REC` that is a header's data.
    ///
    /// The sniffers read file content, so a panic in one (a parser bug on this file) is
    /// caught here and the file is left out: it costs that file, not the folder's listing.
    pub fn supports_file(&self, path: &std::path::Path) -> bool {
        self.supports_entry(path, None)
    }

    /// [`Registry::supports_file`] for a listed entry whose kind the caller knows: `is_dir`
    /// (symbolic links followed), else `None` to look. The probes for folder formats (a
    /// Relight or CASA image, a Bruker experiment, Gerber files, an ODB++ job) run only for
    /// a folder: each one failed for every file that was not an image (sidecars, notes),
    /// about eight system calls a file.
    pub fn supports_entry(&self, path: &std::path::Path, is_dir: Option<bool>) -> bool {
        let what = format_args!("sniffing {}", path.display());
        specular_core::isolate::run(what, || {
            // Test hook: a debug build panics here on a marked name (no-op in release).
            specular_core::isolate::trip_sniff(path);
            let folder = || is_dir.unwrap_or_else(|| path.is_dir());
            (self.supports_extension(path)
                && !sniff::is_volume_sidecar(path)
                && !sniff::is_foreign_label(path)
                && !sniff::is_vobsub_pictures(path))
                || sniff::is_skyscan_log(path)
                || sniff::is_extensionless_dicom(path)
                || sniff::is_mrc_map(path)
                || sniff::is_gaussian_cube(path)
                || sniff::is_bag(path)
                || sniff::is_detector_frame(path)
                || sniff::is_cirrus(path)
                || sniff::is_ecat(path)
                || sniff::is_georaster(path)
                || sniff::is_retro(path)
                || sniff::is_planetary(path)
                || sniff::is_relight_info(path)
                || sniff::is_casa_table(path)
                || spectra_dec::is_bruker_data_file(path)
                || array_dec::is_zarr_metadata_file(path)
                || (folder()
                    && (specular_core::is_relight_folder(path)
                        || specular_core::is_casa_folder(path)
                        || specular_core::is_bruker_folder(path)
                        || specular_core::is_gerber_folder(path)
                        || specular_core::is_odb_folder(path)
                        || specular_core::is_aig_folder(path)))
        })
        .unwrap_or(false)
    }

    /// An `.img` no probe recognised (SMV, R-AXIS and ERDAS IMAGINE are, by content; ENVI
    /// data by its `.hdr`): the data of an Analyze 7.5 or NIfTI pair, opened through the
    /// `.hdr` beside it, a Zeiss Cirrus OCT export (by name and size), or a refusal naming
    /// what it is not. `None` for other names.
    fn open_img(&self, source: &Arc<Source>) -> Option<Result<Arc<dyn Session>>> {
        let path = source.path();
        if path_extensions(path)[1].as_deref() != Some("img") {
            return None;
        }
        if let Some(hdr) = sniff::analyze_header_of(path) {
            return Some(
                Source::open(&hdr)
                    .map_err(DecodeError::Io)
                    .and_then(|s| self.open(Arc::new(s))),
            );
        }
        if specular_retro::gem::probe(source.bytes())
            && let Some(d) = self.decoders.iter().find(|d| d.format() == Format::GemImg)
        {
            return Some(d.open(source.clone()));
        }
        if medvol::cirrus::scan(path, source.len() as u64).is_some()
            && let Some(d) = self.decoders.iter().find(|d| d.format() == Format::Cirrus)
        {
            return Some(d.open(source.clone()));
        }
        specular_log::debug!(target: specular_log::DECODE, "{}: an .img of no known kind ({} bytes)", path.display(), source.len());
        Some(Err(DecodeError::NotOneOf {
            ext: "img",
            formats: IMG_FORMATS,
        }))
    }

    /// The decoder of `format`.
    fn decoder_of(&self, format: Format) -> Option<&dyn Decoder> {
        self.decoders
            .iter()
            .find(|d| d.format() == format)
            .map(|d| d.as_ref())
    }

    /// The format whose [`ident::key`] is `key` (remembered choices).
    pub fn format_by_key(&self, key: &str) -> Option<Format> {
        self.decoders
            .iter()
            .map(|d| d.format())
            .find(|f| ident::key(*f) == key)
    }

    /// The content checks in probe order, up to the first signature that matches; the
    /// weaker matches on the way, and the formats claiming the extension.
    fn scan(&self, path: &std::path::Path, data: &[u8]) -> Scan<'_> {
        let exts = path_extensions(path);
        let mut scan = Scan {
            certain: None,
            weak: Vec::new(),
            guesses: Vec::new(),
            ext: exts[1].clone(),
        };
        for d in &self.decoders {
            let claims = exts
                .iter()
                .flatten()
                .any(|e| d.extensions().contains(&e.as_str()));
            match d.confidence(data) {
                Some(Confidence::Certain) => {
                    scan.certain = Some(d.as_ref());
                    break;
                }
                // A check without a signature counts for the extensions its decoder claims
                // only: a size that fits says nothing of a file named for another format.
                Some(c) if claims => scan.weak.push(Candidate::new(d.format(), c)),
                Some(_) => {}
                None => {
                    // A decoder whose probe takes anything never stands for a guess; one
                    // with a signature that did not match is no guess either.
                    if claims && !d.probe(&[]) && ident::by_name(d.format(), exts[1].as_deref()) {
                        scan.guesses
                            .push(Candidate::new(d.format(), Confidence::Guess));
                    }
                }
            }
        }
        if scan.certain.is_none() {
            scan.weak
                .extend(self.named_candidate(exts[1].as_deref(), data));
        }
        scan
    }

    /// Content checks that need the file's name too: a headerless camera `.raw` (the exact
    /// size of a known layout and samples that fit it, see
    /// [`specular_raw::is_headerless_camera_raw`]) is likely camera raw, never certain.
    fn named_candidate(&self, ext: Option<&str>, data: &[u8]) -> Option<Candidate> {
        let ext = ext?;
        (self.decoder_of(Format::Raw).is_some()
            && specular_raw::is_headerless_camera_raw(data, ext))
        .then(|| Candidate::new(Format::Raw, Confidence::Likely))
    }

    /// Every format the file may be, most likely first: every content check (not only up
    /// to the first that matches) and every format claiming its extension. For "Open
    /// as…", which offers them all.
    pub fn identify(&self, source: &Source) -> Vec<Candidate> {
        let path = source.path();
        let data = source.bytes();
        let exts = path_extensions(path);
        let mut c: Vec<Candidate> = self
            .decoders
            .iter()
            .filter_map(|d| {
                let claims = exts
                    .iter()
                    .flatten()
                    .any(|e| d.extensions().contains(&e.as_str()));
                // As in `scan`: a check without a signature counts for claimed extensions.
                let confidence = match d.confidence(data) {
                    Some(Confidence::Certain) => Confidence::Certain,
                    Some(c) if claims => c,
                    _ => claims.then_some(Confidence::Guess)?,
                };
                Some(Candidate::new(d.format(), confidence))
            })
            .chain(self.named_candidate(exts[1].as_deref(), data))
            .collect();
        ident::rank_in(path, &mut c);
        c
    }

    /// Opens `source` as `format`, whatever its content says ("Open as…").
    pub fn open_as(&self, source: Arc<Source>, format: Format) -> Result<Arc<dyn Session>> {
        let d = self
            .decoder_of(format)
            .ok_or(DecodeError::UnsupportedFormat)?;
        d.open(source)
    }

    pub fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        // Test hook: a debug build panics here on a marked file name (no-op in release).
        specular_core::isolate::trip(source.path());
        // What a file embeds is opened here too: a file containing itself would recurse
        // until the stack overflows.
        if source.nesting() > specular_core::MAX_NESTING {
            return Err(DecodeError::Malformed("files nested too deeply".into()));
        }
        let _nested = source.enter();
        let data = source.bytes();
        // A format chosen for this file by hand ("Open as…", the prompt's answer).
        let chosen = || {
            ident::chosen(source.path())
                .and_then(|f| self.decoders.iter().find(|d| d.format() == f))
                .map(|d| d.as_ref())
        };
        // A folder (an OME-Zarr store, a Relight image or a CASA image, see `Source::open`)
        // has no bytes to probe.
        let folder = || {
            let want = if specular_core::is_filegdb_folder(source.path()) {
                Format::FileGdb
            } else if specular_core::is_bruker_folder(source.path()) {
                Format::BrukerNmr
            } else if specular_core::is_gerber_folder(source.path()) {
                Format::GerberSet
            } else if specular_core::is_odb_folder(source.path()) {
                Format::Odb
            } else if specular_core::is_sentinel2_folder(source.path()) {
                Format::Sentinel2
            } else if specular_core::is_tiles3d_folder(source.path()) {
                Format::Tiles3d
            } else if specular_core::is_relight_folder(source.path()) {
                Format::Relight
            } else if specular_core::is_casa_folder(source.path()) {
                Format::Casa
            } else if specular_core::is_aig_folder(source.path()) {
                Format::ArcInfoGrid
            } else {
                Format::OmeZarr
            };
            (source.is_folder())
                .then(|| self.decoders.iter().find(|d| d.format() == want))
                .flatten()
                .map(|d| d.as_ref())
        };
        // A Bruker `fid`, `ser`, `1r` or `acqus` opened by itself: its experiment.
        let bruker = || {
            (!source.is_folder() && spectra_dec::is_bruker_data_file(source.path()))
                .then(|| {
                    self.decoders
                        .iter()
                        .find(|d| d.format() == Format::BrukerNmr)
                })
                .flatten()
                .map(|d| d.as_ref())
        };
        // A Testo `.bmt` led by its JPEG preview, and the older `.bmt` and `.is2` layouts
        // (refused with the reason): by extension and content.
        let thermal_by_name = || {
            let want = (!source.is_folder())
                .then(|| thermal_dec::by_name(source.path(), data))
                .flatten()?;
            self.decoders
                .iter()
                .find(|d| d.format() == want)
                .map(|d| d.as_ref())
        };
        // A VobSub `.sub` is an MPEG program stream: its `.idx` beside it says what it is.
        let vobsub = || {
            (!source.is_folder() && text_dec::is_vobsub_sub(&source))
                .then(|| self.decoders.iter().find(|d| d.format() == Format::VobSub))
                .flatten()
                .map(|d| d.as_ref())
        };
        // A MIRAX `.mrxs` is a JPEG preview: its slide folder beside it says what it is.
        let mrxs = || {
            (!source.is_folder() && wsi_dec::is_mrxs(source.path()))
                .then(|| self.decoders.iter().find(|d| d.format() == Format::Mrxs))
                .flatten()
                .map(|d| d.as_ref())
        };
        // A Motion JPEG stream named so: JPEG pictures one after another, which the JPEG
        // decoder would take for one picture.
        let mjpeg = || {
            let ext = path_extensions(source.path())[1].clone()?;
            (!source.is_folder()
                && (ext == "mjpeg" || ext == "mjpg")
                && data.starts_with(&[0xFF, 0xD8, 0xFF]))
            .then(|| self.decoder_of(Format::VideoStream))
            .flatten()
        };
        let scan = self.scan(source.path(), data);
        let probed = scan.certain;
        if let Some(d) = probed {
            // Evidence for other files of a shared extension in this folder.
            ident::note_identified(source.path(), d.format());
        }
        // Raw data (`.img`, `.dat`, `.bil`, none…) described by an ENVI or ESRI `.hdr`.
        let labelled = || {
            (geo::on_disk(&source) && georaster_dec::envi_data(source.path()))
                .then(|| self.decoders.iter().find(|d| d.format() == Format::Envi))
                .flatten()
                .map(|d| d.as_ref())
        };
        // A header beside raw data says what the data is: a probe that matches by chance
        // (raw samples starting 00 00 01 00 look like an icon) yields to it, unless the
        // probed decoder also claims the file's extension (a TIFF with an ENVI `.hdr`).
        let claims_ext = |d: &dyn Decoder| {
            path_extensions(source.path())
                .iter()
                .flatten()
                .any(|e| d.extensions().contains(&e.as_str()))
        };
        let probed_or_labelled = || match probed {
            Some(d) if claims_ext(d) => Some(d),
            p => labelled().or(p),
        };
        // Raw planetary data with the PDS3 or PDS4 label beside it that names it.
        if probed.is_none()
            && scan.weak.is_empty()
            && geo::on_disk(&source)
            && let Some((kind, label)) = planetary_dec::label_beside(source.path())
        {
            return planetary_dec::open_through_label(kind, &label);
        }
        // No signature matched: what the content and the extension leave (formats without
        // one, such as TGA, SVGZ or SVG with a long prolog, by extension), asked about when
        // more than one is left and no rule settles it.
        let uncertain = || -> Result<Option<&dyn Decoder>> {
            let mut c = scan.candidates();
            let ext = scan.ext.clone().unwrap_or_default();
            // The folder is consulted only when there is something to choose between.
            if c.len() > 1 {
                ident::rank_in(source.path(), &mut c);
            }
            if !ident::needs_choice(&c) {
                return Ok(c.first().and_then(|c| self.decoder_of(c.format)));
            }
            if let Some(f) = ident::rule_for(source.path(), &c) {
                return Ok(self.decoder_of(f));
            }
            let a = Ambiguity { ext, candidates: c };
            specular_log::debug!(target: specular_log::DECODE, "{}: {a}", source.path().display());
            ident::note(source.path(), &a);
            Err(DecodeError::Ambiguous(Box::new(a)))
        };
        let found = chosen()
            .or_else(folder)
            .or_else(bruker)
            .or_else(thermal_by_name)
            .or_else(vobsub)
            .or_else(mrxs)
            .or_else(mjpeg)
            .or_else(probed_or_labelled);
        let found = match found {
            Some(d) => Some(d),
            None => uncertain()?,
        };
        let Some(dec) = found else {
            if let Some(r) = self.open_img(&source) {
                return r;
            }
            // Raw data whose header lies beside it (a SAGA `.sdat`, ER Mapper data).
            if geo::on_disk(&source)
                && let Some(h) = georaster_dec::header_beside(source.path())
            {
                return Source::open(&h)
                    .map_err(DecodeError::Io)
                    .and_then(|s| self.open(Arc::new(s.within(&source))));
            }
            specular_log::debug!(target: specular_log::DECODE, "{}: no decoder recognises the content ({} bytes)", source.path().display(), source.len());
            return Err(DecodeError::UnsupportedFormat);
        };
        let t0 = std::time::Instant::now();
        let r = dec.open(source);
        match &r {
            Ok(s) => {
                let i = s.info();
                specular_log::debug!(
                    target: specular_log::DECODE,
                    "{} decoder: {}×{} {}{}, orientation {:?}, icc {}, preview {}, fast regions {}, header {:.2} ms",
                    dec.format().name(),
                    i.width,
                    i.height,
                    i.coding,
                    if i.coding.to_string().contains("-bit") { String::new() } else { format!(" {}-bit", i.bit_depth) },
                    i.orientation,
                    i.icc_profile.as_ref().map_or(0, |p| p.len()),
                    i.embedded_preview.is_some(),
                    i.fast_regions,
                    t0.elapsed().as_secs_f64() * 1e3
                );
            }
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "{} decoder: header failed: {e}", dec.format().name())
            }
        }
        // A plain raster beside a world file, `.prj` or `.aux.xml` is georeferenced by them.
        match r {
            Ok(s) if s.geo().is_none() => match geo::sidecar_for(s.as_ref()) {
                Some(g) => Ok(Arc::new(georaster_dec::GeoWrap {
                    inner: s,
                    geo: Some(Arc::new(g)),
                    paged: None,
                })),
                None => Ok(s),
            },
            r => r,
        }
    }

    /// [`Registry::open`] for a thumbnail: a book (EPUB) opens as its cover picture, without
    /// laying out its pages.
    pub fn open_for_thumbnail(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let epub = path_extensions(source.path())
            .iter()
            .flatten()
            .any(|e| e == "epub")
            || specular_epub::is_epub(source.bytes());
        if epub && let Some(s) = epub_dec::open_cover(&source) {
            return Ok(s);
        }
        if let Some(s) = epub_dec::ebook_cover(&source) {
            return Ok(s);
        }
        // A thumbnail never asks: the likeliest format that opens, else the question for
        // when the file is opened.
        match self.open(source.clone()) {
            Err(DecodeError::Ambiguous(a)) => a
                .candidates
                .iter()
                .find_map(|c| self.open_as(source.clone(), c.format).ok())
                .ok_or(DecodeError::Ambiguous(a)),
            r => r,
        }
    }

    pub fn open_path(&self, path: &std::path::Path) -> Result<Arc<dyn Session>> {
        let src = Source::open(path)?;
        self.open(Arc::new(src))
    }
}

/// A path's extension and, for names like `a.xcf.gz`, its compound extension (`xcf.gz`),
/// lower case, compound first.
fn path_extensions(path: &std::path::Path) -> [Option<String>; 2] {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let inner = path
        .file_stem()
        .map(std::path::Path::new)
        .and_then(|s| s.extension())
        .and_then(|e| e.to_str());
    let compound = ext
        .as_ref()
        .zip(inner)
        .map(|(e, i)| format!("{}.{e}", i.to_ascii_lowercase()));
    [compound, ext]
}

/// Opens an image a file embeds (a map tile, a texture) through `registry`, nested below
/// `parent`, refusing one whose header claims more than `max_pixels`: the outer format's
/// size caps do not hold for it.
pub(crate) fn open_nested(
    registry: &Registry,
    name: &str,
    bytes: Vec<u8>,
    parent: &Source,
    max_pixels: u64,
) -> Result<Arc<dyn Session>> {
    let s = registry.open(Arc::new(Source::from_vec(name, bytes).within(parent)))?;
    let info = s.info();
    if u64::from(info.width) * u64::from(info.height) > max_pixels {
        return Err(DecodeError::TooLarge);
    }
    Ok(s)
}

/// The Exif TIFF structure in an Exif payload as formats store it: bare, or after JPEG's
/// `Exif\0\0` identifier.
pub(crate) fn exif_tiff_of(exif: &[u8]) -> Option<Vec<u8>> {
    let t = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
    (t.starts_with(b"II*\0") || t.starts_with(b"MM\0*")).then(|| t.to_vec())
}

/// The info panel's name for chroma subsampled `h`×`v` times (as the metadata scan names
/// JPEG's): 1×1 is "4:4:4", 2×1 "4:2:2", 2×2 "4:2:0".
pub(crate) fn subsampling_name(h: u32, v: u32) -> Option<&'static str> {
    Some(match (h, v) {
        (1, 1) => "4:4:4",
        (2, 1) => "4:2:2",
        (2, 2) => "4:2:0",
        (1, 2) => "4:4:0",
        (4, 1) => "4:1:1",
        (4, 2) => "4:1:0",
        _ => return None,
    })
}

/// The colour model of a coded HEIF/AVIF picture: monochrome, else by its CICP matrix
/// coefficients (ITU-T H.273: 0 = RGB (GBR) stored as is, 8/16/17 = YCgCo; unsignalled or
/// anything else is a YCbCr matrix).
pub(crate) fn cicp_colour_model(mono: bool, matrix: Option<u16>) -> &'static str {
    match (mono, matrix) {
        (true, _) => "Greyscale",
        (false, Some(0)) => "RGB",
        (false, Some(8 | 16 | 17)) => "YCgCo",
        _ => "YCbCr",
    }
}

/// The colour model an ICC profile's data colour space names (header bytes 16..20).
pub(crate) fn icc_colour_model(icc: &[u8]) -> Option<&'static str> {
    Some(match icc.get(16..20)? {
        b"GRAY" => "Greyscale",
        b"RGB " => "RGB",
        b"CMYK" => "CMYK",
        b"Lab " => "Lab",
        b"YCbr" => "YCbCr",
        _ => return None,
    })
}

/// [`Session::trim_memory`] checks for the decoders' own tests (`tests/common/trim.rs` is the
/// same for the integration tests).
#[cfg(test)]
pub(crate) mod trim_check {
    use crate::{DecodeRequest, Session};
    use specular_core::CancelToken;

    /// Decodes `req`, trims, decodes it again: `trim_memory` releases exactly what
    /// `memory_bytes` counted, leaves nothing counted, and the second decode, which rebuilds
    /// the cache, gives the same pixels.
    pub(crate) fn assert_trims(s: &dyn Session, req: &DecodeRequest) {
        let c = CancelToken::new();
        let a = s.decode(req, &c).unwrap();
        let held = s.memory_bytes();
        assert!(held > 0, "the decode cached something");
        assert_eq!(s.trim_memory(), held);
        assert_eq!(s.memory_bytes(), 0);
        assert_eq!(s.trim_memory(), 0, "nothing left");
        let b = s.decode(req, &c).unwrap();
        assert_eq!(
            (a.width, a.height, a.origin, a.scale, a.layout),
            (b.width, b.height, b.origin, b.scale, b.layout)
        );
        assert!(
            a.planes
                .iter()
                .zip(&b.planes)
                .all(|(p, q)| p.data == q.data)
        );
        assert_eq!(s.memory_bytes(), held, "the cache is rebuilt");
    }
}
