//! Making one thumbnail: the embedded preview when it is large enough, else the coarsest
//! decoder scale that still covers the size class (JPEG DCT scaling, PNG/JXL/RAW reduced
//! decodes), then an area-average down to the class and the orientation.

use specular_core::{CancelToken, Raster};
use specular_decode::{DecodeError, DecodeRequest, Format, Session};

use crate::cache::Meta;
use crate::pixels::{self, Pixels};

#[derive(Clone, Copy, Debug)]
pub struct GenOptions {
    /// Accept an embedded preview down to 3/4 of the class (fast) instead of 9/10.
    pub prefer_embedded: bool,
    /// Largest decode, in bytes of working memory, a thumbnail may use; larger images fall
    /// back to their preview or fail.
    pub memory_limit: u64,
    /// Let the decode use several threads (the decoder's own threaded paths: PNG's inflate
    /// pipeline and wavefront unfiltering, ...). On where one thumbnail is all the process
    /// makes (the desktop thumbnailer), so a gigapixel file is not left to one core. Off for
    /// the gallery, whose workers already decode several files at once; its engine still
    /// turns it on for a very large image while the pool has room
    /// ([`crate::engine::decode_in_parallel`]).
    pub parallel: bool,
}

impl Default for GenOptions {
    fn default() -> GenOptions {
        GenOptions {
            prefer_embedded: true,
            memory_limit: 512 << 20,
            parallel: false,
        }
    }
}

/// The header facts of an open session, for sorting, badges and captions.
pub fn meta_of(s: &dyn Session) -> Meta {
    let info = s.info();
    let (w, h) = info.display_size();
    let cam = s.camera();
    let mut m = Meta {
        format: info.format.name().to_string(),
        width: w,
        height: h,
        animated: s.animation().is_some(),
        hdr: s.has_gain_map()
            || info.bit_depth > 8
                && matches!(
                    info.format,
                    Format::Exr
                        | Format::Hdr
                        | Format::Pfm
                        | Format::Avif
                        | Format::Jxl
                        | Format::Heic
                ),
        has_alpha: info.has_alpha,
        raw: matches!(
            info.format,
            Format::Raw | Format::CinemaRawLight | Format::ProResRaw
        ),
        pages: s.page_count().max(1),
        bit_depth: info.bit_depth,
        has_profile: info.icc_profile.is_some(),
        ..Meta::default()
    };
    if let Some(c) = cam {
        m.camera = Some(c.camera.clone()).filter(|c| !c.is_empty());
        m.lens = c.lens.clone();
        m.iso = c.iso;
        m.exposure = c.exposure_time.map(|v| v as f32);
        m.f_number = c.f_number.map(|v| v as f32);
        m.focal_length = c.focal_length.map(|v| v as f32);
        m.date_taken = c.date_time.as_deref().and_then(parse_exif_date);
    }
    // Camera facts, position and date from the Exif TIFF structure, found without decoding:
    // JPEG APP1, PNG eXIf (or its legacy hex text), WebP EXIF, the HEIF/AVIF `Exif` item, the
    // JPEG XL `Exif` box, the PSD Exif resource, a JPEG XR container's Exif IFD, the IFDs of a
    // TIFF (TIFF-based RAW, whose decoder gives no position) and a CR3's CMT blocks (CMT4 is its
    // GPS IFD).
    let bytes = s.source().bytes();
    let tiff_head = bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*");
    let exif = match info.format {
        Format::Jpeg
        | Format::Png
        | Format::WebP
        | Format::Heic
        | Format::Avif
        | Format::Jxl
        | Format::JpegXr
        | Format::Psd => Some(specular_decode::exif_tiff(bytes)),
        Format::Tiff | Format::GeoTiff | Format::Raw | Format::CinemaRawLight
            if tiff_head || specular_decode::is_cr3(bytes) =>
        {
            Some(specular_decode::exif_tiff(bytes))
        }
        // BPG and FLIF: from the extensions the decoder read.
        Format::Bpg | Format::Flif => Some(s.embedded_exif().map(std::borrow::Cow::Owned)),
        // Formats that carry no Exif.
        Format::Gif
        | Format::Bmp
        | Format::Ico
        | Format::Tga
        | Format::Qoi
        | Format::Pnm
        | Format::Dds
        | Format::Ktx2
        | Format::Ktx
        | Format::Pvr
        | Format::Vtf
        | Format::Astc
        | Format::Basis
        | Format::Svg
        | Format::Hdr
        | Format::Pfm
        | Format::Pcx
        | Format::Dcx
        | Format::Sgi
        | Format::SunRaster
        | Format::Xbm
        | Format::Xpm
        | Format::Wbmp
        | Format::CpcScreen
        | Format::MsxScreen
        | Format::AppleHgr
        | Format::AppleDhr
        | Format::BbcScreen
        | Format::AtariMic
        | Format::AtariGr8
        | Format::AtariGr9
        | Format::PhotoCd
        | Format::Farbfeld
        | Format::Fli
        | Format::Sixel
        | Format::Xcursor
        | Format::Qtif
        | Format::Avs
        | Format::MacPaint
        | Format::GemImg
        | Format::Degas
        | Format::Neochrome
        | Format::Koala
        | Format::ZxScreen
        | Format::DrHalo
        | Format::SoftimagePic
        | Format::AliasPix
        | Format::Rla
        | Format::UtahRle
        | Format::Wal
        | Format::Blp
        | Format::Dpx
        | Format::Cineon
        | Format::Xwd
        | Format::Miff
        | Format::Tim
        | Format::GimpPalette
        | Format::PhotoshopSwatches
        | Format::AdobeColorTable
        | Format::SwatchExchange
        | Format::Aseprite
        | Format::GimpGradient
        | Format::GimpBrush
        | Format::GimpBrushPipe
        | Format::GimpPattern
        | Format::MsPaint
        | Format::Gd
        | Format::Gd2
        | Format::PixarPicture
        | Format::LabEyeIm
        | Format::ImTools
        | Format::XvThumbnail
        | Format::McIdasArea
        | Format::Pgx
        | Format::Cals
        | Format::Pkm
        | Format::Ftex
        | Format::Spider
        | Format::Wsq
        | Format::Jbig
        | Format::Jbig2
        | Format::JpegXs
        | Format::Mng
        | Format::Jng
        | Format::Wmf
        | Format::Emf
        | Format::Procreate
        | Format::Affinity
        | Format::Pdn
        | Format::Psp
        | Format::Cdr
        | Format::Icns
        | Format::Ora
        | Format::Kra
        | Format::Iff
        | Format::Pict
        | Format::Dicom
        | Format::Fits
        | Format::Fpack
        | Format::Xisf
        | Format::Vicar
        | Format::Pds3
        | Format::Pds4
        | Format::Asdf
        | Format::Root
        | Format::Casa
        | Format::Hdf5
        | Format::NetCdf
        | Format::Emd
        | Format::Dm
        | Format::Mrc
        | Format::DeltaVision
        | Format::Gwyddion
        | Format::Igor
        | Format::Npy
        | Format::Npz
        | Format::Matlab
        | Format::Zarr
        | Format::Tdms
        | Format::Vtk
        | Format::Lif
        | Format::Nd2
        | Format::Oir
        | Format::Ics
        | Format::Fcs
        | Format::Spe
        | Format::Sif
        | Format::Czi
        | Format::Dxf
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
        | Format::Jcamp
        | Format::Spc
        | Format::Mzml
        | Format::Mzxml
        | Format::BrukerNmr
        | Format::GeoJson
        | Format::Shapefile
        | Format::GeoPackage
        | Format::Kml
        | Format::Kmz
        | Format::Gpx
        | Format::MbTiles
        | Format::PmTiles
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
        | Format::Nitf
        | Format::Stl
        | Format::Obj
        | Format::Ply
        | Format::Gltf
        | Format::Glb
        | Format::ThreeMf
        | Format::Splat
        | Format::Spz
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
        | Format::Alembic
        | Format::Ifc
        | Format::Blend
        | Format::Sketchup
        | Format::Step
        | Format::Iges
        | Format::Dwg
        | Format::Nifti
        | Format::Analyze
        | Format::Nrrd
        | Format::MetaImage
        | Format::Svs
        | Format::Ndpi
        | Format::Scn
        | Format::OmeTiff
        | Format::OmeZarr
        | Format::PyramidTiff
        | Format::DicomWsi
        | Format::Mrxs
        | Format::Bif
        | Format::Vsi
        | Format::Vms
        | Format::Vmu
        | Format::Las
        | Format::Laz
        | Format::Pcd
        | Format::Xyz
        | Format::Pts
        | Format::E57
        | Format::Ptx
        | Format::Mgh
        | Format::Minc
        | Format::Nexus
        | Format::Imaris
        | Format::Bag
        | Format::Srtm
        | Format::Cube
        | Format::Cbf
        | Format::Edf
        | Format::Smv
        | Format::Bruker
        | Format::MarCcd
        | Format::Raxis
        | Format::EsriAscii
        | Format::Dted
        | Format::Envi
        | Format::Sentinel2
        | Format::Stac
        | Format::Tiles3d
        | Format::EsriBil
        | Format::ErdasImagine
        | Format::BsbChart
        | Format::ErMapper
        | Format::SagaGrid
        | Format::ArcInfoGrid
        | Format::GdalVrt
        | Format::Grib
        | Format::Hdf4
        | Format::Tia
        | Format::Bcf
        | Format::Ang
        | Format::Ctf
        | Format::Nanoscope
        | Format::Nanonis
        | Format::Jpk
        | Format::NtMdt
        | Format::X3p
        | Format::MetroPro
        | Format::Datx
        | Format::Vk4
        | Format::Plu
        | Format::Al3d
        | Format::Sur
        | Format::Segy
        | Format::SeismicUnix
        | Format::Txm
        | Format::Amira
        | Format::SkyScan
        | Format::Xtek
        | Format::Vgi
        | Format::RawVolume
        | Format::Cirrus
        | Format::ParRec
        | Format::Ecat
        | Format::Ptm
        | Format::Rti
        | Format::Relight
        | Format::Omero
        | Format::Pdb
        | Format::Mmcif
        | Format::Cif
        | Format::Sdf
        | Format::MolXyz
        | Format::Gifti
        | Format::Cifti
        | Format::Trk
        | Format::Tck
        | Format::Mif
        | Format::JpegLs
        | Format::Pdf
        | Format::Illustrator
        | Format::Eps
        | Format::Dvi
        | Format::Djvu
        | Format::Xps
        | Format::Cgm
        | Format::Clip
        | Format::Visio
        | Format::Wpg
        | Format::Sketch
        | Format::Figma
        | Format::OpenDocument
        | Format::Ooxml
        | Format::MsOffice
        | Format::Iwork
        | Format::Xcf => Some(None),
        _ => None,
    };
    // The Exif fields search names (looked for in every file: none is a known "none").
    m.exif = Some(Box::new(
        exif.as_ref()
            .and_then(|e| e.as_deref())
            .and_then(crate::exif_meta)
            .unwrap_or_else(|| crate::ExifMeta {
                tags: Some(String::new()),
                ..Default::default()
            }),
    ));
    if let Some(exif) = &exif {
        let facts = exif.as_deref().and_then(specular_decode::exif_facts);
        m.gps = Some(facts.as_ref().is_some_and(|e| e.1));
        // The position itself, for event splits on location jumps.
        if m.gps == Some(true) {
            m.position = exif
                .as_deref()
                .and_then(specular_decode::exif_position)
                .map(|(lat, lon)| (lat as f32, lon as f32));
        }
        if let Some((c, _)) = facts
            && m.camera.is_none()
            && m.iso.is_none()
        {
            m.camera = Some(c.camera).filter(|c| !c.is_empty());
            m.lens = c.lens;
            m.iso = c.iso;
            m.exposure = c.exposure_time.map(|v| v as f32);
            m.f_number = c.f_number.map(|v| v as f32);
            m.focal_length = c.focal_length.map(|v| v as f32);
        }
    }
    // Motion Photo / Live Photo: the clip's marker in the XMP or a trailer, or a sibling movie.
    if matches!(info.format, Format::Jpeg | Format::Heic) {
        let xmp = (info.format == Format::Heic)
            .then(|| specular_decode::embedded_metadata(bytes).xmp)
            .flatten();
        if let Some(k) = specular_video::quick_kind(bytes, s.source().path(), xmp.as_deref()) {
            m.motion = true;
            m.live_photo = k == specular_video::Kind::LivePhoto;
        }
    }
    // A movie: its length for the tile's badge, and its recording date and camera for
    // sorting and captions.
    if let Some(v) = s.video() {
        m.video = true;
        m.sound = v.sound.is_some();
        m.animated = false;
        m.duration_us = v.duration_us;
        m.date_taken = m.date_taken.or(v.recorded);
        let camera = [v.make.as_deref(), v.model.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        if m.camera.is_none() && !camera.is_empty() {
            m.camera = Some(camera);
        }
        m.hdr = v.hdr.is_some();
    }
    m.space = Some(specular_decode::colour_space(
        s,
        exif.as_ref().and_then(|e| e.as_deref()),
    ));
    // An astronomy subframe: its header facts (the metrics need pixels: the engine).
    if let Some(h) = specular_decode::astro::subframe_header(s) {
        m.exposure = m.exposure.or(h.exposure.map(|v| v as f32));
        m.date_taken = m
            .date_taken
            .or_else(|| h.date_obs.as_deref().and_then(parse_exif_date));
        if m.camera.is_none() {
            m.camera = s.science().and_then(|sum| sum.instrument.clone());
        }
        m.focal_length = m.focal_length.or(h.focal_mm.map(|v| v as f32));
        m.astro = Some(Box::new(crate::cache::AstroMeta {
            frame_type: h.kind(),
            object: h.object,
            filter: h.filter,
            gain: h.gain.map(|v| v as f32),
            ccd_temp: h.ccd_temp.map(|v| v as f32),
            metrics: None,
        }));
    }
    if m.date_taken.is_none() {
        let date = exif
            .flatten()
            .and_then(|t| specular_decode::exif_date_taken(&t))
            .or_else(|| match info.format {
                // A text "Creation Time".
                Format::Png => specular_decode::png_date_taken(&mut std::io::Cursor::new(bytes)),
                _ => None,
            });
        m.date_taken = date.as_deref().and_then(parse_exif_date);
    }
    m
}

/// `YYYY:MM:DD HH:MM:SS` (also `-` / `T` separators) → seconds since 1970 (no zone).
pub fn parse_exif_date(s: &str) -> Option<i64> {
    let d: Vec<i64> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .take(6)
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    // Bounded fields keep the arithmetic below in range for any digits the file holds.
    if d.len() < 3
        || !(1800..=9999).contains(&d[0])
        || !(1..=12).contains(&d[1])
        || !(1..=31).contains(&d[2])
        || d.get(3).is_some_and(|h| *h > 24)
        || d.get(4).is_some_and(|m| *m > 59)
        || d.get(5).is_some_and(|s| *s > 60)
    {
        return None;
    }
    let get = |i: usize| d.get(i).copied().unwrap_or(0);
    Some(days_from_civil(d[0], d[1], d[2]) * 86_400 + get(3) * 3600 + get(4) * 60 + get(5))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Makes the thumbnail for `class`. `rough` receives an early low-resolution result (a small
/// embedded preview) while the real decode runs.
pub fn generate(
    s: &dyn Session,
    class: u32,
    opts: GenOptions,
    cancel: &CancelToken,
    rough: &mut dyn FnMut(Pixels),
) -> Result<Pixels, DecodeError> {
    generate_also(s, class, None, opts, cancel, rough).map(|(px, _)| px)
}

/// A smaller class made with a larger one by [`generate_also`].
#[derive(Clone, Debug)]
pub struct Also {
    pub pixels: Pixels,
    /// Made from the source [`generate`] takes for that class (the same embedded preview, or
    /// a decode at the same scale), so bit-identical to it; else area-averaged from the
    /// larger class's finer decode.
    pub exact: bool,
}

/// [`generate`] for `class`, and with it the thumbnail of the smaller class `also` (`None`
/// when that is not smaller, or when `class` fell back to a too-small preview). It comes
/// from the same source as [`generate`] would take for `also` whenever the larger decode
/// is that source, so it costs an area-average and no second decode.
pub fn generate_also(
    s: &dyn Session,
    class: u32,
    also: Option<u32>,
    opts: GenOptions,
    cancel: &CancelToken,
    rough: &mut dyn FnMut(Pixels),
) -> Result<(Pixels, Option<Also>), DecodeError> {
    let info = s.info().clone();
    let (dw, dh) = info.display_size();
    let long = dw.max(dh).max(1);
    let target = class.min(long);
    let small = also.filter(|&a| a < class).map(|a| a.min(long));
    let aspect = dw as f64 / dh.max(1) as f64;
    let need = if opts.prefer_embedded { 0.75 } else { 0.9 };
    let mut fallback = None;
    // The smaller class from the preview, when `generate` would take the preview for it.
    let mut small_px = None;
    // A raw's embedded JPEG decodes at 1/8, 1/4 or 1/2 scale: the coarsest with twice the
    // class, not the screen size the viewer wants. Within a level or two of the screen-size
    // decode on average; 3/4 of the class (the least taken) was faster still but up to four
    // levels off on average.
    let want = target.saturating_mul(2);
    if let Some(Ok(p)) = s.decode_preview_for(want) {
        let (pw, ph) = info.orientation.display_size(p.width, p.height);
        let pa = pw as f64 / ph.max(1) as f64;
        // EXIF thumbnails of 16:9 photos are often 4:3 with black bars: not usable.
        let same_shape = (pa / aspect - 1.0).abs() < 0.04;
        // An opaque preview of an image with alpha (PSD, HEIC `thmb`) is flattened against
        // some matte: good enough to show while decoding, never the final thumbnail.
        let final_ok = !info.has_alpha || p.layout.has_alpha();
        if same_shape {
            let px = pixels::thumbnail(&p, info.orientation, target);
            if let Some(t) = small
                && final_ok
                && pw.max(ph) as f64 >= t as f64 * need
            {
                small_px = Some(Also {
                    pixels: pixels::thumbnail(&p, info.orientation, t),
                    exact: true,
                });
            }
            if final_ok && pw.max(ph) as f64 >= target as f64 * need {
                return Ok((px, small_px));
            }
            rough(px.clone());
            fallback = Some(px);
        }
    }
    if cancel.is_cancelled() {
        return Err(DecodeError::Cancelled);
    }
    // Big science arrays: every n-th sample of the first plane, from only the chunks that
    // hold them. Failing here means a full decode would fare worse; no second try.
    if let Some(r) = s.decode_sampled(target, cancel) {
        return match r {
            Ok(r) => {
                let small_px = small_px.or_else(|| {
                    Some(Also {
                        pixels: pixels::thumbnail(&r, info.orientation, small?),
                        exact: false,
                    })
                });
                Ok((pixels::thumbnail(&r, info.orientation, target), small_px))
            }
            Err(DecodeError::Cancelled) => Err(DecodeError::Cancelled),
            Err(e) => fallback.map(|f| (f, small_px)).ok_or(e),
        };
    }
    let scales: Vec<u32> = if info.scales.is_empty() {
        vec![1]
    } else {
        info.scales.to_vec()
    };
    // The decode scale for a class: the coarsest that covers it, coarser still to stay
    // within the memory limit (a coarser scale than wanted beats no thumbnail at all).
    let scale_for = |t: u32| {
        let mut req =
            DecodeRequest::full(pixels::pick_scale(&scales, info.width.max(info.height), t));
        req.parallel = opts.parallel;
        let mut mem = s.decode_memory(&req);
        while mem > opts.memory_limit {
            match scales.iter().copied().filter(|&c| c > req.scale).min() {
                Some(c) => {
                    req.scale = c;
                    mem = s.decode_memory(&req);
                }
                None => break,
            }
        }
        (req, mem)
    };
    let (req, mem) = scale_for(target);
    if mem > opts.memory_limit {
        return fallback.map(|f| (f, small_px)).ok_or(DecodeError::TooLarge);
    }
    match s.decode(&req, cancel) {
        Ok(r) => {
            let r = sdr_rendition(s, r, cancel);
            let small_px = small_px.or_else(|| {
                let t = small?;
                Some(Also {
                    pixels: pixels::thumbnail(&r, info.orientation, t),
                    exact: scale_for(t).0.scale == req.scale,
                })
            });
            Ok((pixels::thumbnail(&r, info.orientation, target), small_px))
        }
        Err(e) => match (e, fallback) {
            (DecodeError::Cancelled, _) => Err(DecodeError::Cancelled),
            (_, Some(f)) => Ok((f, small_px)),
            (e, None) => Err(e),
        },
    }
}

/// A wide HDR base with a gain map towards SDR becomes its SDR alternate (the full map), as
/// the author made it, rather than a generic tone mapping; anything else is kept.
fn sdr_rendition(s: &dyn Session, r: Raster, cancel: &CancelToken) -> Raster {
    if !r.layout.is_wide() || !s.has_gain_map() {
        return r;
    }
    let peak = specular_core::hdr::peak_from_nits(None);
    s.gain_map(cancel)
        .filter(|gm| gm.params.base_is_hdr)
        .and_then(|gm| specular_core::hdr::sdr_rendition(&r, &gm, peak))
        .unwrap_or(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Raw thumbnails from the embedded preview: time per file at 512 + 256 (best of 3), and
    /// how far the pixels are from the previous run's (`$TMPDIR/rawthumb-*.rgba`). Skipped
    /// without the raw400 corpus.
    #[test]
    fn raw_preview_thumbnail_times() {
        let root = std::env::var("SPECULAR_RAW400").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/corpus/raw400").into()
        }) + "/pixls";
        let files = [
            "Canon/Canon EOS R6 Mark III/IMG_4241.CR3",
            "Nikon/Z f/DSC_0040.NEF",
            "SONY/ZV-E10M2/DSC00696.ARW",
            "FUJIFILM/X-T30 III/DSCF0131.RAF",
            "OM Digital Solutions/OM-3/P5048182.ORF",
        ];
        let reg = specular_decode::Registry::default();
        let c = CancelToken::new();
        let tmp = std::env::temp_dir();
        for (i, f) in files.iter().enumerate() {
            let p = std::path::Path::new(&root).join(f);
            let Ok(s) = reg.open_path(&p) else {
                eprintln!("skipped {f}");
                continue;
            };
            let mut best = std::time::Duration::MAX;
            let mut out = None;
            for _ in 0..3 {
                let t0 = std::time::Instant::now();
                let r = generate_also(&*s, 512, Some(256), GenOptions::default(), &c, &mut |_| {})
                    .unwrap();
                best = best.min(t0.elapsed());
                out = Some(r);
            }
            let (big, _) = out.unwrap();
            let keep = tmp.join(format!("rawthumb-{i}.rgba"));
            let diff = std::fs::read(&keep).ok().map(|old| {
                if old.len() != big.rgba.len() {
                    return format!("size changed ({} -> {})", old.len(), big.rgba.len());
                }
                let d: Vec<i32> = old
                    .iter()
                    .zip(&big.rgba)
                    .map(|(&a, &b)| (a as i32 - b as i32).abs())
                    .collect();
                let max = d.iter().max().copied().unwrap_or(0);
                let mean = d.iter().map(|&v| v as f64).sum::<f64>() / d.len() as f64;
                format!("max diff {max}, mean {mean:.3}")
            });
            if diff.is_none() {
                std::fs::write(&keep, &big.rgba).unwrap();
            }
            eprintln!(
                "{f}: {}x{} in {best:.1?} ({})",
                big.width,
                big.height,
                diff.unwrap_or_else(|| "kept".into())
            );
        }
    }

    #[test]
    fn dates() {
        assert_eq!(parse_exif_date("1970:01:01 00:00:00"), Some(0));
        assert_eq!(parse_exif_date("2024:03:12 10:30:00"), Some(1_710_239_400));
        assert_eq!(parse_exif_date("2024-03-12T10:30:00"), Some(1_710_239_400));
        assert_eq!(parse_exif_date("0000:00:00 00:00:00"), None);
        assert_eq!(parse_exif_date(""), None);
        // Header digits that would overflow the seconds.
        assert_eq!(parse_exif_date("999999999999999999:01:01 00:00:00"), None);
        assert_eq!(parse_exif_date("2024:03:12 999999999999999999:00:00"), None);
        assert_eq!(parse_exif_date("2024:03:12 00:00:999999999999999999"), None);
        assert_eq!(
            parse_exif_date("9999:12:31 24:59:60"),
            Some(253_402_304_400)
        );
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }

    /// A JPEG XL PQ base with a `jhgm` map towards SDR: the thumbnail is the SDR alternate
    /// (tests/gen_gainmap.py in specular-jxl derived the base from it).
    #[test]
    fn jxl_gain_map_sdr_alternate() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../formats/specular-jxl/tests/data/gainmap_hdr.jxl"
        );
        let s = specular_decode::Registry::new()
            .open_path(std::path::Path::new(path))
            .unwrap();
        assert!(meta_of(&*s).hdr);
        let c = CancelToken::new();
        let px = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
        assert_eq!((px.width, px.height), (32, 16));
        let mut worst = 0;
        for (n, p) in px.rgba.chunks_exact(4).enumerate() {
            let (x, y) = (n as u32 % 32, n as u32 / 32);
            let want = [40 + 6 * x, 30 + 12 * y, 200 - 4 * x, 255];
            for c in 0..4 {
                worst = worst.max(p[c].abs_diff(want[c] as u8));
            }
        }
        assert!(worst <= 2, "max |Δ| {worst}");
    }

    /// Arithmetic-coded JPEGs (specular-jpeg's fixtures from `make-arith.sh`, and the fuzz
    /// seed whose gallery tile showed the error icon) make thumbnails through the reduced-scale
    /// decode: close to the area-average of the full decode, at every size class.
    #[test]
    fn arithmetic_jpeg() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../formats/specular-jpeg");
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(format!("{dir}/tests/fixtures"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                let n = p.file_name().unwrap().to_string_lossy();
                n.starts_with("arith_") && n.ends_with(".jpg")
            })
            .collect();
        files.push(format!("{dir}/fuzz/seeds/arith.jpg").into());
        assert!(files.len() >= 6);
        let reg = specular_decode::Registry::new();
        let c = CancelToken::new();
        for p in files {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let s = reg.open_path(&p).unwrap();
            let m = meta_of(&*s);
            assert_eq!(m.format, "JPEG", "{name}");
            let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
            for class in [16, 48, 128, 256] {
                let px = generate(&*s, class, GenOptions::default(), &c, &mut |_| {})
                    .unwrap_or_else(|e| panic!("{name} {class}: {e}"));
                let want = pixels::thumbnail(&full, s.info().orientation, class);
                // A reduced-scale source may round the fitted size the other way.
                assert!(
                    px.width.abs_diff(want.width) <= 1 && px.height.abs_diff(want.height) <= 1,
                    "{name} {class}: {}×{} vs {}×{}",
                    px.width,
                    px.height,
                    want.width,
                    want.height
                );
                let mean = |p: &Pixels, ch: usize| {
                    let s: u64 = p.rgba.chunks_exact(4).map(|q| u64::from(q[ch])).sum();
                    s as f64 / (p.rgba.len() / 4) as f64
                };
                // The 1/8 (DC-only) decode of the smallest classes shifts the mean a little
                // (djpeg -scale 1/8 does the same, ~3 levels on the 4:4:4 fixture).
                for ch in 0..3 {
                    let (a, b) = (mean(&px, ch), mean(&want, ch));
                    assert!(
                        (a - b).abs() < 4.0,
                        "{name} {class} channel {ch}: {a:.1} vs {b:.1}"
                    );
                }
                // Not flat: the pictures have detail even at 16 pixels.
                let (lo, hi) = px
                    .rgba
                    .chunks_exact(4)
                    .fold((255u8, 0u8), |(lo, hi), q| (lo.min(q[1]), hi.max(q[1])));
                assert!(
                    hi - lo > 20 || px.width < 8,
                    "{name} {class}: flat {lo}..{hi}"
                );
            }
        }
    }

    /// A research volume's thumbnail is its middle slice through the default window: the
    /// shown page, not the first slice.
    #[test]
    fn research_volume_middle_slice() {
        let reg = specular_decode::Registry::new();
        let path = format!(
            "{}/../formats/specular-medvol/tests/data/gzipped.nii.gz",
            env!("CARGO_MANIFEST_DIR")
        );
        let s = reg.open_path(std::path::Path::new(&path)).unwrap();
        assert!(s.page_count() > 1);
        assert_eq!(s.page_index(), s.page_count() / 2);
        let c = CancelToken::new();
        let px = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        let want = pixels::thumbnail(&r, s.info().orientation, 256);
        assert_eq!(px.rgba, want.rgba);
        let m = meta_of(&*s);
        assert_eq!(m.format, "NIfTI");
    }

    /// A slide's thumbnail is its thumbnail image when that is large enough, else a decode
    /// from the pyramid level nearest the class, never the full-resolution level.
    #[test]
    fn slide_from_its_thumbnail_or_a_small_level() {
        let reg = specular_decode::Registry::new();
        let (file, _) = specular_wsi::fixtures::svs();
        let src = std::sync::Arc::new(specular_core::Source::from_vec(
            std::path::PathBuf::from("slide.svs"),
            file,
        ));
        let s = reg.open(src).unwrap();
        let c = CancelToken::new();
        // The 64x48 thumbnail image covers class 64.
        let px = generate(&*s, 64, GenOptions::default(), &c, &mut |_| {}).unwrap();
        let p = s.decode_preview().unwrap().unwrap();
        assert_eq!(
            px.rgba,
            pixels::thumbnail(&p, s.info().orientation, 64).rgba
        );
        // Class 128 comes from the 1/4 level (128x96).
        let px = generate(&*s, 128, GenOptions::default(), &c, &mut |_| {}).unwrap();
        let r = s.decode(&DecodeRequest::full(4), &c).unwrap();
        assert_eq!((r.width, r.height), (128, 96));
        assert_eq!(
            px.rgba,
            pixels::thumbnail(&r, s.info().orientation, 128).rgba
        );
        let m = meta_of(&*s);
        assert_eq!(
            (m.format.as_str(), m.width, m.height),
            ("Aperio SVS", 512, 384)
        );
        assert_eq!(m.pages, 4);
    }

    /// JPEG XL HDR stills without a map (tests/gen_hdr.py in specular-jxl) decode wide, and
    /// their thumbnails take the SDR path of AVIF HDR stills without a map: the wide raster
    /// through `wide_to_srgb8` from its own content peak.
    #[test]
    fn jxl_hdr_without_map() {
        let reg = specular_decode::Registry::new();
        for name in [
            "hdr_pq_p3.jxl",
            "hdr_hlg_2020_alpha.jxl",
            "hdr_pq_709_lossy.jxl",
            "linear_f32.jxl",
            "linear_f16_2020_lossy.jxl",
        ] {
            let path = format!(
                "{}/../formats/specular-jxl/tests/data/{name}",
                env!("CARGO_MANIFEST_DIR")
            );
            let s = reg.open_path(std::path::Path::new(&path)).unwrap();
            assert!(meta_of(&*s).hdr, "{name}");
            assert!(!s.has_gain_map(), "{name}");
            let c = CancelToken::new();
            let px = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
            let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
            assert!(r.layout.is_wide(), "{name}");
            let want = pixels::thumbnail(&r, s.info().orientation, 32);
            assert_eq!((px.width, px.height), (32, 16), "{name}");
            assert_eq!(px.rgba, want.rgba, "{name}");
            // The top right is the brightest red (four times reference white): near white
            // in red after tone mapping, and the image is not black.
            let tr = &px.rgba[31 * 4..32 * 4];
            assert!(tr[0] >= 200, "{name}: top right {tr:?}");
            let mean = px.rgba.iter().map(|&v| u32::from(v)).sum::<u32>() / px.rgba.len() as u32;
            assert!(mean > 60, "{name}: mean {mean}");
        }
    }

    /// The class-256 thumbnail made along with a class-512 one (HiDPI galleries, for the
    /// similar-shots signature) is the one a class-256 request makes whenever both come from
    /// the same source: a HEIC (no scaled decodes), an image under 1024 whose decode serves
    /// both. Where the decoder has a coarser scale for 256 (JPEG DCT scaling, PNG and WebP
    /// reduced decodes) it is said to be inexact, and stays within a few levels on average.
    #[test]
    fn signature_class_along_with_a_larger_one() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let reg = specular_decode::Registry::new();
        let c = CancelToken::new();
        for (name, exact) in [
            ("crates/specular-decode/data/jpeg-calibration.jpg", false),
            (
                "crates/specular-widgets/tests/shots/viewer-dark-2x.png",
                false,
            ),
            ("tools/bench/gate/webp-lossy.webp", false),
            ("crates/specular-decode/data/heic-calibration.heic", true),
            ("crates/formats/specular-jpeg/fuzz/seeds/arith.jpg", true),
        ] {
            let s = reg
                .open_path(std::path::Path::new(&format!("{root}/{name}")))
                .unwrap();
            let (big, also) =
                generate_also(&*s, 512, Some(256), GenOptions::default(), &c, &mut |_| {}).unwrap();
            let one = generate(&*s, 512, GenOptions::default(), &c, &mut |_| {}).unwrap();
            assert_eq!(
                (big.width, big.height, &big.rgba),
                (one.width, one.height, &one.rgba),
                "{name}"
            );
            let also = also.unwrap_or_else(|| panic!("{name}: no class 256"));
            let want = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
            assert_eq!(also.exact, exact, "{name}");
            assert_eq!(
                (also.pixels.width, also.pixels.height),
                (want.width, want.height),
                "{name}"
            );
            let worst = also
                .pixels
                .rgba
                .iter()
                .zip(&want.rgba)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0);
            let mean = also
                .pixels
                .rgba
                .iter()
                .zip(&want.rgba)
                .map(|(a, b)| u64::from(a.abs_diff(*b)))
                .sum::<u64>() as f64
                / want.rgba.len().max(1) as f64;
            eprintln!(
                "{name}: {}×{} exact {} max |Δ| {worst} mean |Δ| {mean:.3}",
                want.width, want.height, also.exact
            );
            if exact {
                assert_eq!(also.pixels.rgba, want.rgba, "{name}");
            } else {
                assert!(mean < 3.0, "{name}: max {worst} mean {mean:.3}");
            }
        }
        // Not smaller, or no second class asked: none.
        let s = reg
            .open_path(std::path::Path::new(&format!(
                "{root}/tools/bench/gate/webp-lossy.webp"
            )))
            .unwrap();
        for also in [None, Some(512), Some(1024)] {
            let (_, a) =
                generate_also(&*s, 512, also, GenOptions::default(), &c, &mut |_| {}).unwrap();
            assert!(a.is_none(), "{also:?}");
        }
    }

    /// A threaded decode (the desktop thumbnailer's) makes the same thumbnail as the gallery's
    /// single-threaded one: an opaque RGB PNG above the threading threshold, with the
    /// wavefront forced on so its parallel unfiltering is the path taken.
    #[test]
    fn parallel_decode_same_thumbnail() {
        let (w, h) = (1600u32, 900u32);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                rgb.extend_from_slice(&[(x * 5 + y) as u8, ((y * 3) ^ x) as u8, (x / 7) as u8]);
            }
        }
        let png = specular_png::encode(w, h, specular_png::ColorType::Rgb, &rgb).unwrap();
        specular_png::set_wavefront_min(0, 0);
        let src = std::sync::Arc::new(specular_core::Source::from_vec("t.png", png));
        let s = specular_decode::Registry::new().open(src).unwrap();
        let c = CancelToken::new();
        for class in [128, 256] {
            let make = |parallel| {
                let opts = GenOptions {
                    parallel,
                    ..GenOptions::default()
                };
                generate(&*s, class, opts, &c, &mut |_| {}).unwrap()
            };
            let (one, many) = (make(false), make(true));
            assert_eq!((one.width, one.height), (many.width, many.height));
            assert_eq!(one.rgba, many.rgba, "class {class}");
        }
    }
}
