//! FITS (`.fits`, `.fit`, `.fts`) and fpack (`.fz`, [`Format::Fpack`]: tile-compressed FITS)
//! through `specular-fits`. A whole file compressed by gzip (`.fits.gz`, as archives serve
//! them) is inflated into memory at open, its size reserved from the memory budget.
//!
//! The image HDUs of a file become the session's pages, except that several images of one
//! plane size (and no cube axes), such as the SCI / ERR / DQ of a calibrated exposure or the
//! filters of a colour set, form one page with an "extension" axis. Cube axes (`NAXIS3`…)
//! are the page's further axes. Planes come out as real values through
//! [`Session::nd_plane`]; [`Session::decode`] gives the default view: zscale limits and a
//! linear stretch, in grey, or in colour when three images or a three-plane cube are named
//! red, green and blue. A one-shot colour camera's raw frame (`BAYERPAT`) is debayered
//! with each colour auto-stretched on its own ([`fits_cfa`], [`science::cfa_rgba`]).
//!
//! Tables become pages too. A HEALPix map (`PIXTYPE = 'HEALPIX'`, a LIGO/Virgo multi-order
//! sky map) is shown as a Mollweide all-sky raster with a matching WCS, its fields (columns)
//! on a field axis, and for a probability map the 50 % and 90 % credible regions
//! ([`science::Credible`]). An X-ray event list (`EVENTS`) is binned into an image on the
//! WCS of its position columns, with a bin-size axis and an energy-band axis that re-bin
//! from the table.

use std::sync::Arc;

use specular_core::nd;
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_fits as fits;

use crate::science::{
    self, AxisKind, NdAxis, NdPlane, PixelSpectra, PlaneCache, ScienceSummary, Spectrum,
    SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// FITS, or (`fpack`) a FITS file as fpack writes it: an empty primary HDU, then the
/// tile-compressed image (`ZIMAGE = T`), reported as [`Format::Fpack`].
pub struct FitsDecoder {
    pub fpack: bool,
}

/// Header blocks looked at for fpack's compressed image.
const FPACK_BLOCKS: usize = 16;

/// A FITS header block.
const BLOCK: usize = 2880;

/// The largest `.fits.gz` inflated (the memory budget usually refuses well before).
const MAX_GZ: usize = 1 << 36;

/// Whether `data` starts with the gzip signature.
fn is_gzip(data: &[u8]) -> bool {
    data.starts_with(&[0x1f, 0x8b])
}

/// A `.fits.gz` inflated into memory (with the budget holding its size while the file is
/// open), or the file itself.
fn gunzip(source: Arc<Source>) -> Result<(Arc<Source>, Option<specular_core::Reservation>)> {
    if !is_gzip(source.bytes()) {
        return Ok((source, None));
    }
    // The output is reserved from the budget as it grows: a gzip bomb stops at what is free.
    let data =
        specular_png::inflate_gzip_members(source.bytes(), MAX_GZ, None).map_err(|e| match e {
            specular_png::InflateError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
            e => DecodeError::Malformed(format!("gzip stream: {e}")),
        })?;
    let hold = specular_core::budget::reserve(data.len() as u64)?;
    let inflated = Source::from_entry(source.path(), &source, data);
    Ok((Arc::new(inflated), Some(hold)))
}

/// Whether a FITS file's primary HDU has no data (`NAXIS = 0`) and the header after it is a
/// binary table with `ZIMAGE = T`: what fpack writes.
pub(crate) fn is_fpack(data: &[u8]) -> bool {
    let card = |c: &[u8], key: &str| -> Option<String> {
        let k = c.get(..8)?;
        (std::str::from_utf8(k).ok()?.trim_end() == key && c.get(8..10) == Some(b"= ")).then(|| {
            let v = String::from_utf8_lossy(&c[10..]);
            v.split('/')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('\'')
                .trim()
                .to_owned()
        })
    };
    // The cards of the header starting at block `b`, and the block after its END.
    let header = |b: usize| -> Option<(Vec<&[u8]>, usize)> {
        let mut cards = Vec::new();
        for blk in b..b + FPACK_BLOCKS {
            let block = data.get(blk * BLOCK..(blk + 1) * BLOCK)?;
            for c in block.chunks_exact(80) {
                if c.starts_with(b"END     ") || c == b"END" {
                    return Some((cards, blk + 1));
                }
                cards.push(c);
            }
        }
        None
    };
    let find = |cards: &[&[u8]], key: &str| cards.iter().find_map(|c| card(c, key));
    let Some((primary, next)) = header(0) else {
        return false;
    };
    if find(&primary, "NAXIS").as_deref() != Some("0") {
        return false;
    }
    let Some((ext, _)) = header(next) else {
        return false;
    };
    find(&ext, "XTENSION").as_deref() == Some("BINTABLE")
        && find(&ext, "ZIMAGE").as_deref() == Some("T")
}

/// The colour filter array of a one-shot colour camera's raw frame, in displayed
/// orientation, from `BAYERPAT` (or a four-letter `COLORTYP`), `XBAYROFF` / `YBAYROFF` and
/// `ROWORDER`, as capture software (N.I.N.A., SharpCap, ASIAIR, Siril) writes them: the
/// pattern starts at the first pixel of the first stored row (`ROWORDER = 'TOP-DOWN'` or
/// none: the bottom row as FITS is shown) or of the top row (`'BOTTOM-UP'`), shifted by the
/// offsets. `None` when the header names no Bayer pattern.
pub(crate) fn fits_cfa(h: &fits::Header, height: usize) -> Option<nd::Cfa> {
    let pat = ["BAYERPAT", "COLORTYP"]
        .iter()
        .find_map(|k| h.str(k).and_then(nd::Cfa::parse))?;
    let off = |k: &str| {
        h.f64(k)
            .filter(|v| v.is_finite())
            .map_or(0, |v| (v.round() as i64).rem_euclid(2) as usize)
    };
    let pat = pat.shifted(off("XBAYROFF"), off("YBAYROFF"));
    let bottom_up = h
        .str("ROWORDER")
        .is_some_and(|r| r.trim().eq_ignore_ascii_case("BOTTOM-UP"));
    Some(if bottom_up { pat } else { pat.flipped(height) })
}

fn map_err(e: fits::Error) -> DecodeError {
    match e {
        fits::Error::NotFits => DecodeError::UnsupportedFormat,
        fits::Error::Malformed(s) => DecodeError::Malformed(s),
        fits::Error::Unsupported(s) => DecodeError::Unsupported(s),
        fits::Error::TooLarge => DecodeError::TooLarge,
        fits::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        fits::Error::Cancelled => DecodeError::Cancelled,
    }
}

/// A page of a file.
#[derive(Clone, Debug, PartialEq)]
enum Page {
    /// Image HDUs (indices into `Fits::hdus`), more than one when they share a plane size and
    /// form an extension axis.
    Images(Vec<usize>),
    /// A HEALPix map's table HDU.
    Healpix(usize),
    /// An event list's table HDU.
    Events(usize),
}

impl Page {
    fn first(&self) -> usize {
        match self {
            Page::Images(m) => m.first().copied().unwrap_or(0),
            Page::Healpix(i) | Page::Events(i) => *i,
        }
    }
}

/// The pages of a file, in HDU order: its images, HEALPix maps and event lists.
fn pages(f: &fits::Fits) -> Vec<Page> {
    let mut pages: Vec<Page> = image_pages(f).into_iter().map(Page::Images).collect();
    for h in f.hdus.iter().filter(|h| h.kind == fits::Kind::BinTable) {
        if matches!(fits::healpix::Map::from_hdu(h), Some(Ok(_))) {
            pages.push(Page::Healpix(h.index));
        } else if fits::events::Events::is_events(h) {
            pages.push(Page::Events(h.index));
        }
    }
    pages.sort_by_key(Page::first);
    pages
}

/// Whether `bytes` are a FITS file with something to show (an image, a HEALPix map, an
/// event list), rather than only tables.
pub(crate) fn has_pages(bytes: &[u8]) -> bool {
    bytes.starts_with(b"SIMPLE  =") && fits::parse(bytes).is_ok_and(|f| !pages(&f).is_empty())
}

/// The image pages of a file: each a list of image HDUs, more than one when they share a
/// plane size and form an extension axis.
fn image_pages(f: &fits::Fits) -> Vec<Vec<usize>> {
    let imgs: Vec<&fits::Hdu> = f
        .images()
        .filter(|h| {
            h.image
                .as_ref()
                .is_some_and(|i| i.width() > 0 && i.height() > 0)
        })
        // WCS distortion lookup tables (HST) are part of another HDU's coordinates.
        .filter(|h| {
            !h.header.str("EXTNAME").is_some_and(|n| {
                let n = n.trim();
                n.eq_ignore_ascii_case("WCSDVARR") || n.eq_ignore_ascii_case("D2IMARR")
            })
        })
        .collect();
    let flat = |h: &fits::Hdu| h.image.as_ref().is_some_and(|i| i.planes() == 1);
    let size = |h: &fits::Hdu| h.image.as_ref().map(|i| (i.width(), i.height()));
    if imgs.len() >= 2 && imgs.iter().all(|h| flat(h) && size(h) == size(imgs[0])) {
        return vec![imgs.iter().map(|h| h.index).collect()];
    }
    imgs.iter().map(|h| vec![h.index]).collect()
}

impl Decoder for FitsDecoder {
    fn format(&self) -> Format {
        if self.fpack {
            Format::Fpack
        } else {
            Format::Fits
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        // The first bytes cannot tell fpack from plain FITS: `probe_file` looks further.
        !self.fpack && fits::sniff(head)
    }
    /// Also inside a gzip stream: the first header block inflated (and fpack's headers).
    fn probe_file(&self, data: &[u8]) -> bool {
        if is_gzip(data) {
            let sniffed =
                specular_png::inflate_gzip_prefix(data, BLOCK).is_ok_and(|h| fits::sniff(&h));
            return sniffed
                && (!self.fpack
                    || specular_png::inflate_gzip_prefix(data, 2 * FPACK_BLOCKS * BLOCK)
                        .is_ok_and(|h| is_fpack(&h)));
        }
        fits::sniff(&data[..data.len().min(crate::PROBE_HEAD)]) && (!self.fpack || is_fpack(data))
    }
    fn extensions(&self) -> &'static [&'static str] {
        if self.fpack {
            &["fz", "fits.fz"]
        } else {
            &["fits", "fit", "fts", "fits.gz", "fit.gz", "fts.gz"]
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let (source, inflated) = gunzip(source)?;
        let f = fits::parse(source.bytes()).map_err(map_err)?;
        let pages = pages(&f);
        if pages.is_empty() {
            return Err(DecodeError::Unsupported(
                "no image, HEALPix map or event list in the FITS file".into(),
            ));
        }
        let asdf = asdf_meta(&f, source.bytes()).map(Box::new);
        let file = Arc::new(FitsFile {
            fits: f,
            pages,
            asdf,
            format: self.format(),
            inflated,
        });
        Ok(Arc::new(FitsSession::new(source, file, 0)?))
    }
}

/// The spectral axis types of the FITS WCS standard (paper III), by their first four
/// letters.
const SPECTRAL: [&str; 11] = [
    "FREQ", "ENER", "WAVN", "VRAD", "WAVE", "VOPT", "ZOPT", "AWAV", "VELO", "BETA", "FELO",
];

/// Spectral channels a cube may declare past the planes its file holds (a truncated cube).
const MAX_SPECTRUM: usize = 1 << 16;

/// The spectrum at each pixel of a data cube whose third axis is spectral with world values
/// (an integral-field or radio cube), plotted under the cursor.
fn cube_spectra(
    source: &Arc<Source>,
    file: &Arc<FitsFile>,
    members: &[usize],
    width: u32,
    height: u32,
) -> Option<PixelSpectra> {
    let [hdu] = members else { return None };
    let h = &file.fits.hdus[*hdu];
    let img = h.image.as_ref()?;
    let n = *img.axes.get(2)?;
    if n < 2 || img.tiles.is_some() || img.axes.iter().skip(3).any(|&a| a != 1) {
        return None;
    }
    // NAXIS3 sizes the spectral axis below: a cut-short cube keeps it, but not one claiming
    // billions of planes the file has no room for.
    let plane = (img.width().saturating_mul(img.height())).saturating_mul(img.sample_bytes());
    let stored = source.bytes().len().saturating_sub(h.data_offset) / plane.max(1);
    if n > stored.saturating_add(1).max(MAX_SPECTRUM) {
        return None;
    }
    let ctype = h.header.str("CTYPE3")?.trim().to_ascii_uppercase();
    if !SPECTRAL.iter().any(|t| ctype.starts_with(t)) {
        return None;
    }
    let wcs = fits::wcs::Wcs::from_hdu(&file.fits, source.bytes(), h)?;
    let x: Arc<[f64]> = (0..n)
        .map(|i| wcs.axis_value(2, i))
        .collect::<Option<Vec<f64>>>()?
        .into();
    let name = ctype.get(..4).unwrap_or(&ctype);
    let name = match name {
        "FREQ" => "Frequency",
        "ENER" => "Energy",
        "WAVN" => "Wavenumber",
        "WAVE" | "AWAV" => "Wavelength",
        "ZOPT" => "Redshift",
        "BETA" => "Beta",
        _ => "Velocity",
    };
    let unit = h.header.str("CUNIT3").unwrap_or("").trim().to_owned();
    Some(PixelSpectra::new(
        Arc::new(CubeSpectra {
            source: source.clone(),
            file: file.clone(),
            hdu: *hdu,
            x,
            name: name.into(),
            unit,
        }),
        width,
        height,
    ))
}

/// A FITS cube's spectra (see [`cube_spectra`]).
struct CubeSpectra {
    source: Arc<Source>,
    file: Arc<FitsFile>,
    hdu: usize,
    x: Arc<[f64]>,
    name: String,
    unit: String,
}

impl std::fmt::Debug for CubeSpectra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CubeSpectra")
            .field("hdu", &self.hdu)
            .field("channels", &self.x.len())
            .finish()
    }
}

impl SpectrumSource for CubeSpectra {
    fn grid(&self) -> (usize, usize) {
        let img = self.file.fits.hdus[self.hdu].image.as_ref();
        img.map_or((0, 0), |i| (i.width(), i.height()))
    }
    fn channels(&self) -> usize {
        self.x.len()
    }
    fn x_axis(&self) -> (String, String) {
        (self.name.clone(), self.unit.clone())
    }
    fn at(&self, x: usize, y: usize, _index: &[u32]) -> Option<Spectrum> {
        let hdu = &self.file.fits.hdus[self.hdu];
        let v = fits::read_spectrum(self.source.bytes(), hdu, x, y).ok()?;
        Some(Spectrum {
            y: v,
            x: SpectrumX::Values(self.x.clone()),
            sticks: false,
        })
    }
}

struct FitsFile {
    /// FITS, or fpack for a tile-compressed file.
    format: Format,
    fits: fits::Fits,
    pages: Vec<Page>,
    /// The metadata tree of an `ASDF` extension (JWST).
    asdf: Option<Box<AsdfMeta>>,
    /// The budget held for a `.fits.gz` inflated into memory.
    inflated: Option<specular_core::Reservation>,
}

/// What a JWST file's `ASDF` extension (the data model's metadata tree) adds to the summary.
struct AsdfMeta {
    facts: science::AsdfFacts,
    meta: specular_asdf::Meta,
    tree: specular_asdf::Value,
    /// The tree's entries, listed after the header cards.
    keywords: Vec<(String, String, String)>,
}

/// Tree entries listed at most.
const MAX_ASDF_KEYWORDS: usize = 4000;

/// The metadata tree in the `ASDF` extension: a one-row binary table whose
/// `ASDF_METADATA` column holds an ASDF file's bytes. `None` when there is none or it cannot
/// be read (the image is shown regardless).
fn asdf_meta(f: &fits::Fits, data: &[u8]) -> Option<AsdfMeta> {
    let hdu = f
        .hdus
        .iter()
        .find(|h| h.kind == fits::Kind::BinTable && h.name().is_some_and(|n| n.trim() == "ASDF"))?;
    let t = fits::table::Table::from_hdu(hdu).ok()?;
    let c = &t.columns[t.column("ASDF_METADATA")?];
    let bytes = if c.is_var() {
        // Some writers store the bytes as a variable-length array.
        let n = t.len(data, 0, t.column("ASDF_METADATA")?);
        let v: Vec<u8> = t
            .cell(data, 0, t.column("ASDF_METADATA")?)
            .iter()
            .filter_map(|d| match d {
                fits::table::Datum::Int(i) => u8::try_from(*i).ok(),
                fits::table::Datum::UInt(u) => u8::try_from(*u).ok(),
                _ => None,
            })
            .collect();
        (v.len() == n).then_some(std::borrow::Cow::Owned(v))?
    } else {
        if c.ty != fits::table::Type::U8 || t.rows == 0 {
            return None;
        }
        let start = t.data_offset.checked_add(c.offset)?;
        std::borrow::Cow::Borrowed(data.get(start..start.checked_add(c.width)?)?)
    };
    let a = specular_asdf::Asdf::open(&bytes).ok()?;
    let keywords = specular_asdf::flatten(&a.tree, MAX_ASDF_KEYWORDS)
        .into_iter()
        .map(|(k, v)| (k, v, "ASDF".to_owned()))
        .collect();
    Some(AsdfMeta {
        facts: crate::asdf_dec::facts(&a),
        meta: specular_asdf::science_meta(&a.tree),
        keywords,
        tree: a.tree,
    })
}

/// What a table page's planes are made from.
enum TableData {
    Healpix {
        map: fits::healpix::Map,
        index: fits::healpix::Index,
    },
    Events(fits::events::Events),
}

struct FitsSession {
    source: Arc<Source>,
    file: Arc<FitsFile>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
    table: Option<Box<TableData>>,
}

/// The `ImageInfo` of a page of `width × height` with samples of `bits`.
fn page_info(width: u32, height: u32, bits: u8, coding: String) -> ImageInfo {
    ImageInfo {
        format: Format::Fits,
        width,
        height,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: bits,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    }
}

fn dims(w: usize, h: usize) -> Result<(u32, u32)> {
    Ok((
        u32::try_from(w).map_err(|_| DecodeError::TooLarge)?,
        u32::try_from(h).map_err(|_| DecodeError::TooLarge)?,
    ))
}

/// A number as short text: integers without a fraction.
fn short(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

impl FitsSession {
    fn new(source: Arc<Source>, file: Arc<FitsFile>, page: u32) -> Result<FitsSession> {
        let format = file.format;
        let mut s = match file.pages[page as usize] {
            Page::Images(_) => Self::new_image(source, file, page),
            Page::Healpix(hdu) => Self::new_healpix(source, file, page, hdu),
            Page::Events(hdu) => Self::new_events(source, file, page, hdu),
        }?;
        s.info.format = format;
        if s.file.inflated.is_some() {
            s.info.coding.push_str(", gzip");
        }
        Ok(s)
    }

    /// A HEALPix map: the Mollweide raster of each field.
    fn new_healpix(
        source: Arc<Source>,
        file: Arc<FitsFile>,
        page: u32,
        k: usize,
    ) -> Result<FitsSession> {
        let hdu = &file.fits.hdus[k];
        let data = source.bytes();
        let mut map = fits::healpix::Map::from_hdu(hdu)
            .ok_or_else(|| DecodeError::Unsupported("not a HEALPix map".into()))?
            .map_err(map_err)?;
        let index = map.index(data).map_err(map_err)?;
        let (w, h) = map.raster_size();
        let (width, height) = dims(w, h)?;
        let wcs = fits::wcs::Wcs::from_header(&map.mollweide_header(w), &[w, h]).map(Arc::new);
        let names = map.field_names();
        let units: Vec<String> = map
            .fields
            .iter()
            .map(|&c| map.table.columns[c].unit.trim().to_owned())
            .collect();
        let prob = map.probability_field();
        let mut axes = Vec::new();
        if names.len() > 1 {
            axes.push(NdAxis {
                name: String::new(),
                len: names.len() as u32,
                unit: String::new(),
                labels: names,
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Field,
            });
        }
        let at = |f: usize| {
            if axes.is_empty() {
                Vec::new()
            } else {
                vec![f as u32]
            }
        };
        let start = prob.map(at).unwrap_or_default();
        let credible = prob.and_then(|f| {
            let t = map.credible(data, f, &[0.5, 0.9])?;
            Some(science::Credible {
                index: at(f),
                levels: vec![(0.5, t[0]), (0.9, t[1])],
            })
        });
        let ordering = match map.ordering {
            fits::healpix::Ordering::Ring => "RING",
            fits::healpix::Ordering::Nested => "NESTED",
            fits::healpix::Ordering::Uniq => "NUNIQ",
        };
        let sample = match map.scheme {
            fits::healpix::Scheme::Uniq(_) => format!(
                "HEALPix {ordering}, orders to {} (Nside {})",
                index.order, map.nside
            ),
            fits::healpix::Scheme::Explicit(_) => {
                format!("HEALPix Nside {}, {ordering}, partial sky", map.nside)
            }
            fits::healpix::Scheme::Implicit => format!("HEALPix Nside {}, {ordering}", map.nside),
        };
        let mut summary = ScienceSummary {
            width,
            height,
            axes,
            wcs,
            sample,
            hdu: hdu.name(),
            start,
            credible,
            ..Default::default()
        };
        summary.describe(&hdu.header, Some(&file.fits.hdus[0].header));
        if units.len() > 1 {
            summary.units_by = Some((0, units));
        } else if let Some(u) = units.into_iter().next().filter(|u| !u.is_empty()) {
            summary.unit = Some(u);
        }
        let info = page_info(width, height, 32, summary.sample.clone());
        Ok(FitsSession {
            source,
            file,
            page,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
            table: Some(Box::new(TableData::Healpix { map, index })),
        })
    }

    /// An event list: counts in bins of a size and an energy band.
    fn new_events(
        source: Arc<Source>,
        file: Arc<FitsFile>,
        page: u32,
        k: usize,
    ) -> Result<FitsSession> {
        let hdu = &file.fits.hdus[k];
        let ev = fits::events::Events::from_hdu(source.bytes(), hdu)
            .ok_or_else(|| DecodeError::Unsupported("not an event list".into()))?
            .map_err(map_err)?;
        let (width, height) = dims(ev.width, ev.height)?;
        let wcs = ev
            .wcs_header(&hdu.header)
            .and_then(|h| fits::wcs::Wcs::from_header(&h, &[ev.width, ev.height]))
            .map(Arc::new);
        let mut axes = vec![NdAxis {
            name: String::new(),
            len: fits::events::FACTORS.len() as u32,
            unit: String::new(),
            labels: fits::events::FACTORS
                .iter()
                .map(|&f| short(ev.base * f64::from(f)))
                .collect(),
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Bin,
        }];
        let ename = ev
            .energy
            .map(|(c, _)| ev.table.columns[c].name.trim().to_owned())
            .unwrap_or_default();
        if ev.bands.len() > 1 {
            axes.push(NdAxis {
                name: String::new(),
                len: ev.bands.len() as u32,
                unit: String::new(),
                labels: ev
                    .bands
                    .iter()
                    .map(|b| match (b.kev, b.range) {
                        (Some((lo, hi)), _) => format!("{}–{} keV", short(lo), short(hi)),
                        (None, Some((lo, hi))) => {
                            format!("{ename} {}–{}", short(lo), short(hi))
                        }
                        // Every event: worded by the viewer.
                        (None, None) => String::new(),
                    })
                    .collect(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Band,
            });
        }
        let start = if axes.len() == 2 {
            vec![0, ev.bands.len() as u32 - 1]
        } else {
            vec![0]
        };
        let mut summary = ScienceSummary {
            width,
            height,
            axes,
            wcs,
            sample: format!("event list, {} events", ev.count),
            hdu: hdu.name(),
            start,
            ..Default::default()
        };
        summary.describe(&hdu.header, Some(&file.fits.hdus[0].header));
        summary.unit = None;
        let info = page_info(width, height, 32, summary.sample.clone());
        Ok(FitsSession {
            source,
            file,
            page,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
            table: Some(Box::new(TableData::Events(ev))),
        })
    }

    fn new_image(source: Arc<Source>, file: Arc<FitsFile>, page: u32) -> Result<FitsSession> {
        let Page::Images(members) = &file.pages[page as usize] else {
            return Err(DecodeError::Malformed("not an image page".into()));
        };
        let hdu = &file.fits.hdus[members[0]];
        let img = hdu.image.as_ref().expect("image HDU");
        let (w, h) = (img.width(), img.height());
        let (width, height) = (
            u32::try_from(w).map_err(|_| DecodeError::TooLarge)?,
            u32::try_from(h).map_err(|_| DecodeError::TooLarge)?,
        );
        let wcs = fits::wcs::Wcs::from_hdu(&file.fits, source.bytes(), hdu).map(Arc::new);
        let mut axes = Vec::new();
        let mut channel_axis = None;
        if members.len() > 1 {
            let labels: Vec<String> = members
                .iter()
                .map(|&i| {
                    file.fits.hdus[i]
                        .name()
                        .unwrap_or_else(|| format!("HDU {i}"))
                })
                .collect();
            if science::rgb_labels(&labels) {
                channel_axis = Some(0);
            }
            axes.push(NdAxis {
                name: String::new(),
                len: members.len() as u32,
                unit: String::new(),
                labels,
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Extension,
            });
        } else {
            for (k, &n) in img.axes.iter().enumerate().skip(2) {
                let ctype = hdu
                    .header
                    .str(&format!("CTYPE{}", k + 1))
                    .unwrap_or("")
                    .trim()
                    .to_owned();
                let unit = hdu
                    .header
                    .str(&format!("CUNIT{}", k + 1))
                    .unwrap_or("")
                    .trim()
                    .to_owned();
                let colour = n == 3
                    && img.axes.len() == 3
                    && (ctype.is_empty()
                        || ["RGB", "COLOR", "COLOUR", "BAND"]
                            .iter()
                            .any(|c| ctype.to_ascii_uppercase().contains(c)));
                if colour {
                    channel_axis = Some(0);
                }
                let has_wcs = wcs.as_ref().is_some_and(|w| w.axis_value(k, 0).is_some());
                axes.push(NdAxis {
                    name: ctype,
                    len: u32::try_from(n).map_err(|_| DecodeError::TooLarge)?,
                    unit,
                    labels: if colour {
                        ["R", "G", "B"].map(String::from).to_vec()
                    } else {
                        Vec::new()
                    },
                    wcs_axis: has_wcs.then_some(k),
                    linear: None,
                    kind: if colour {
                        AxisKind::Channel
                    } else {
                        AxisKind::Cube
                    },
                });
            }
        }
        let mut summary = ScienceSummary {
            width,
            height,
            axes,
            channel_axis,
            wcs,
            sample: img.coding(),
            hdu: hdu.name().filter(|_| members.len() == 1),
            ..Default::default()
        };
        summary.spectra = cube_spectra(&source, &file, members, width, height);
        let primary = (hdu.index != 0).then(|| &file.fits.hdus[0].header);
        summary.describe(&hdu.header, primary);
        // Data quality extensions (JWST, HST `DQ`) hold bit flags.
        let flags = |i: usize| {
            let h = &file.fits.hdus[i];
            h.image
                .as_ref()
                .is_some_and(|m| m.bitpix > 0 && m.bscale == 1.0)
                && h.name()
                    .is_some_and(|n| crate::asdf_dec::is_flags_name(n.trim()))
        };
        if members.len() > 1 {
            let f: Vec<bool> = members.iter().map(|&i| flags(i)).collect();
            if f.contains(&true) {
                summary.flags_by = Some((0, f));
            }
        } else {
            summary.flags = flags(members[0]);
        }
        if let Some(m) = &file.asdf {
            crate::asdf_dec::describe(&mut summary, &m.meta);
            let mut k = summary.keywords.as_ref().clone();
            k.extend(m.keywords.iter().cloned());
            summary.keywords = Arc::new(k);
            summary.asdf = Some(m.facts.clone());
            // No WCS in the header: approximate the tree's pointing.
            if summary.wcs.is_none()
                && let Some(cards) =
                    specular_asdf::approx_wcs_cards(&m.tree, u64::from(width), u64::from(height))
            {
                let header = fits::Header::from_pairs(
                    cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")),
                );
                summary.wcs = fits::wcs::Wcs::from_header(&header, &[w, h]).map(Arc::new);
                summary.wcs_approximate = summary.wcs.is_some();
            }
        }
        if members.len() == 1 && summary.planes() == 1 {
            summary.cfa = fits_cfa(&hdu.header, height as usize);
        }
        let bits = img.bitpix.unsigned_abs().min(64) as u8;
        let info = page_info(width, height, bits, summary.sample.clone());
        Ok(FitsSession {
            source,
            file,
            page,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
            table: None,
        })
    }

    fn members(&self) -> &[usize] {
        match &self.file.pages[self.page as usize] {
            Page::Images(m) => m,
            _ => &[],
        }
    }

    /// A table page's plane at `index`.
    fn read_table(&self, t: &TableData, index: &[u32], cancel: &CancelToken) -> Result<NdPlane> {
        let at = |k: usize| index.get(k).copied().unwrap_or(0) as usize;
        let data = self.source.bytes();
        let values = match t {
            TableData::Healpix { map, index: idx } => {
                let field = at(0);
                if field >= map.fields.len() {
                    return Err(DecodeError::Malformed("no such field".into()));
                }
                map.mollweide(data, idx, field, self.info.width as usize, &|| {
                    cancel.is_cancelled()
                })
            }
            TableData::Events(ev) => {
                if at(0) >= fits::events::FACTORS.len() || at(1) >= ev.bands.len() {
                    return Err(DecodeError::Malformed("no such plane".into()));
                }
                ev.bin(data, at(0), at(1), &|| cancel.is_cancelled())
            }
        }
        .map_err(map_err)?;
        Ok(NdPlane {
            width: self.info.width,
            height: self.info.height,
            values,
            truncated: false,
        })
    }

    fn read(&self, index: &[u32], cancel: &CancelToken) -> Result<NdPlane> {
        if let Some(t) = &self.table {
            return self.read_table(t, index, cancel);
        }
        let members = self.members();
        let (hdu, plane) = if members.len() > 1 {
            let k = index.first().copied().unwrap_or(0) as usize;
            let hdu = members
                .get(k)
                .ok_or_else(|| DecodeError::Malformed("no such plane".into()))?;
            (&self.file.fits.hdus[*hdu], 0)
        } else {
            let hdu = &self.file.fits.hdus[members[0]];
            let img = hdu.image.as_ref().expect("image HDU");
            for (k, (&i, &n)) in index.iter().zip(img.axes.iter().skip(2)).enumerate() {
                if i as usize >= n {
                    return Err(DecodeError::Malformed(format!(
                        "axis {} out of range",
                        k + 3
                    )));
                }
            }
            let pos: Vec<usize> = index.iter().map(|&i| i as usize).collect();
            (hdu, img.plane_index(&pos))
        };
        let p = fits::read_plane(self.source.bytes(), hdu, plane).map_err(map_err)?;
        if (p.width, p.height) != (self.info.width as usize, self.info.height as usize) {
            return Err(DecodeError::Malformed("plane size differs".into()));
        }
        Ok(NdPlane {
            width: self.info.width,
            height: self.info.height,
            values: p.values,
            truncated: p.truncated,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes
            .get_or(index, cancel, || self.read(index, cancel))
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let n = self.summary.axes.len();
        let first = if self.summary.start.len() == n {
            self.summary.start.clone()
        } else {
            vec![0; n]
        };
        let planes = match self.summary.channel_axis {
            Some(c) => (0..self.summary.axes[c].len.min(3))
                .map(|k| {
                    let mut idx = vec![0; n];
                    idx[c] = k;
                    self.plane(&idx, cancel)
                })
                .collect::<Result<Vec<_>>>()?,
            None => vec![self.plane(&first, cancel)?],
        };
        Ok(science::auto_raster(&planes, self.summary.cfa))
    }
}

impl Session for FitsSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        // The f32 plane(s) and the 8-bit view.
        let chans = if self.summary.channel_axis.is_some() {
            3
        } else {
            1
        };
        px * 4 * chans + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.file.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.file.pages.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(u32::from(self.info.bit_depth)),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.file.pages.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(FitsSession::new(
            self.source.clone(),
            self.file.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.summary.channel_axis.is_some() {
            "RGB"
        } else if self.summary.cfa.is_some() {
            "Bayer"
        } else if self.summary.axes.is_empty() {
            "Greyscale"
        } else {
            "Multichannel"
        })
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal FITS file: header cards then big-endian data, both padded to 2880 bytes.
    fn file(cards: &[String], data: &[u8]) -> Vec<u8> {
        let mut h: Vec<u8> = Vec::new();
        for c in cards.iter().chain(std::iter::once(&"END".to_string())) {
            h.extend(format!("{c:<80}").as_bytes());
        }
        h.resize(h.len().div_ceil(2880) * 2880, b' ');
        let mut d = data.to_vec();
        d.resize(d.len().div_ceil(2880) * 2880, 0);
        h.extend(d);
        h
    }

    fn card(k: &str, v: &str) -> String {
        format!("{k:<8}= {v:>20}")
    }

    fn open(bytes: Vec<u8>) -> Arc<dyn Session> {
        let reg = crate::Registry::new();
        reg.open(Arc::new(Source::from_vec("t.fits", bytes)))
            .unwrap()
    }

    fn gz(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    /// A `.fits.gz` opens as the FITS inside it, by name and by content.
    #[test]
    fn gzipped_fits_opens() {
        let data: Vec<u8> = (0..12i16).flat_map(i16::to_be_bytes).collect();
        let plain = file(
            &[
                card("SIMPLE", "T"),
                card("BITPIX", "16"),
                card("NAXIS", "2"),
                card("NAXIS1", "4"),
                card("NAXIS2", "3"),
            ],
            &data,
        );
        let reg = crate::Registry::new();
        let c = CancelToken::new();
        let want = open(plain.clone()).nd_plane(&[], &c).unwrap().unwrap();
        for name in ["t.fits.gz", "t.fts.gz", "no-extension"] {
            let s = reg
                .open(Arc::new(Source::from_vec(name, gz(&plain))))
                .unwrap();
            assert_eq!(s.info().format, Format::Fits, "{name}");
            assert!(s.info().coding.ends_with(", gzip"), "{name}");
            let p = s.nd_plane(&[], &c).unwrap().unwrap();
            assert_eq!(p.values, want.values, "{name}");
        }
        // Not FITS inside: not claimed.
        let other = Source::from_vec("x.fits.gz", gz(b"just text, not a FITS header"));
        assert!(!FitsDecoder { fpack: false }.probe_file(other.bytes()));
        // A broken stream after a good first block is malformed, not a panic.
        let mut cut = gz(&plain);
        cut.truncate(cut.len() - 12);
        let r = reg.open(Arc::new(Source::from_vec("t.fits.gz", cut)));
        assert!(matches!(r, Err(DecodeError::Malformed(_))), "{:?}", r.err());
    }

    /// Hostile: a small `.fits.gz` that inflates to far more than the memory budget has free
    /// is refused with what it needs, whether its gzip size field is honest or not.
    #[test]
    fn gzip_bomb_needs_memory() {
        let mut bomb = file(
            &[
                card("SIMPLE", "T"),
                card("BITPIX", "8"),
                card("NAXIS", "2"),
                card("NAXIS1", "8192"),
                card("NAXIS2", "6144"),
            ],
            &[],
        );
        bomb.resize(2880 + 8192 * 6144, 0);
        let honest = gz(&bomb);
        assert!(honest.len() < 1 << 20);
        let mut lying = honest.clone();
        let n = lying.len();
        lying[n - 4..].copy_from_slice(&4096u32.to_le_bytes());
        drop(bomb);
        let budget = Arc::new(specular_core::budget::MemoryBudget::fixed(8 << 20));
        for g in [honest, lying] {
            let r = specular_core::budget::with_budget(budget.clone(), || {
                crate::Registry::new().open(Arc::new(Source::from_vec("bomb.fits.gz", g)))
            });
            match r {
                Err(DecodeError::NeedsMemory(n)) => assert!(n.needed > n.free),
                r => panic!("{:?}", r.err()),
            }
            assert_eq!(budget.reserved(), 0);
        }
    }

    /// A raw frame whose `BAYERPAT` holds at the first stored pixel: the displayed
    /// pattern (FITS rows shown bottom-up) debayers every pixel to the stored colours.
    #[test]
    fn bayer_pattern_follows_row_order_and_offsets() {
        let colour = [1000.0f32, 500.0, 100.0];
        // (rows, cards, offsets, bottom-up, displayed pattern)
        for (h, extra, (dx, dy), bottom_up, want) in [
            (4usize, vec![], (0, 0), false, nd::Cfa::GBRG),
            (3, vec![], (0, 0), false, nd::Cfa::RGGB),
            (
                4,
                vec![card("ROWORDER", "'BOTTOM-UP'")],
                (0, 0),
                true,
                nd::Cfa::RGGB,
            ),
            (
                4,
                vec![card("ROWORDER", "'TOP-DOWN'"), card("XBAYROFF", "1")],
                (1, 0),
                false,
                nd::Cfa::BGGR,
            ),
            (4, vec![card("YBAYROFF", "1")], (0, 1), false, nd::Cfa::RGGB),
        ] {
            let w = 6usize;
            let mut cards = vec![
                card("SIMPLE", "T"),
                card("BITPIX", "-32"),
                card("NAXIS", "2"),
                card("NAXIS1", &w.to_string()),
                card("NAXIS2", &h.to_string()),
                card("BAYERPAT", "'RGGB'"),
            ];
            cards.extend(extra);
            let s = open(file(&cards, &[]));
            let cfa = s.science().unwrap().cfa;
            assert_eq!(cfa, Some(want), "{cards:?}");
            assert_eq!(s.colour_model(), Some("Bayer"));
            // The pattern starts at the first stored pixel (bottom-up: the top row's).
            let mut data = Vec::new();
            for r in 0..h {
                for x in 0..w {
                    let row = if bottom_up { h - 1 - r } else { r };
                    data.extend(colour[nd::Cfa::RGGB.at(x + dx, row + dy)].to_be_bytes());
                }
            }
            let s = open(file(&cards, &data));
            let p = s.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
            for y in 0..h {
                for x in 0..w {
                    assert_eq!(want.debayer_at(&p.values, w, h, x, y), colour, "{x},{y}");
                }
            }
        }
        // No pattern, or a cube: no debayering.
        let s = open(file(
            &[
                card("SIMPLE", "T"),
                card("BITPIX", "8"),
                card("NAXIS", "2"),
                card("NAXIS1", "2"),
                card("NAXIS2", "2"),
                card("COLORTYP", "'MONO'"),
            ],
            &[0; 4],
        ));
        assert_eq!(s.science().unwrap().cfa, None);
    }

    #[test]
    fn cube_planes_and_auto_view() {
        let (w, h, n) = (4usize, 3usize, 2usize);
        let mut data = Vec::new();
        for k in 0..n {
            for i in 0..w * h {
                data.extend(((k * 100 + i) as i16).to_be_bytes());
            }
        }
        let s = open(file(
            &[
                card("SIMPLE", "T"),
                card("BITPIX", "16"),
                card("NAXIS", "3"),
                card("NAXIS1", "4"),
                card("NAXIS2", "3"),
                card("NAXIS3", "2"),
                card("OBJECT", "'M42'"),
            ],
            &data,
        ));
        assert_eq!(s.info().format, Format::Fits);
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].len, 2);
        assert_eq!(sc.object.as_deref(), Some("M42"));
        let c = CancelToken::new();
        let p = s.nd_plane(&[1], &c).unwrap().unwrap();
        // Top row first: FITS row 3 (index 8..12) of plane 1.
        assert_eq!(p.value(0, 0), Some(108.0));
        assert_eq!(p.value(3, 2), Some(103.0));
        assert!(s.nd_plane(&[2], &c).unwrap().is_err());
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((r.width, r.height), (4, 3));
    }

    #[test]
    fn same_size_extensions_form_an_axis() {
        let img = |name: &str, v: f32| {
            let data: Vec<u8> = (0..6).flat_map(|_| v.to_be_bytes()).collect();
            file(
                &[
                    card("XTENSION", "'IMAGE   '"),
                    card("BITPIX", "-32"),
                    card("NAXIS", "2"),
                    card("NAXIS1", "3"),
                    card("NAXIS2", "2"),
                    card("PCOUNT", "0"),
                    card("GCOUNT", "1"),
                    card("EXTNAME", &format!("'{name}'")),
                ],
                &data,
            )
        };
        let mut bytes = file(
            &[card("SIMPLE", "T"), card("BITPIX", "8"), card("NAXIS", "0")],
            &[],
        );
        for (n, v) in [("R", 1.0), ("G", 2.0), ("B", 3.0)] {
            bytes.extend(img(n, v));
        }
        let s = open(bytes);
        assert_eq!(s.page_count(), 1);
        let sc = s.science().unwrap();
        assert_eq!(sc.axes[0].kind, AxisKind::Extension);
        assert_eq!(sc.axes[0].labels, ["R", "G", "B"]);
        assert_eq!(sc.channel_axis, Some(0));
        let c = CancelToken::new();
        assert_eq!(
            s.nd_plane(&[2], &c).unwrap().unwrap().value(1, 1),
            Some(3.0)
        );
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.layout, specular_core::PixelLayout::Rgba);
    }

    fn fixture(name: &str) -> Vec<u8> {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../formats/specular-fits/tests/data/tables/"
        );
        std::fs::read(format!("{dir}{name}")).unwrap()
    }

    #[test]
    fn healpix_map_is_a_mollweide_page() {
        let s = open(fixture("flatprob.fits"));
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].kind, AxisKind::Field);
        assert_eq!(sc.axes[0].labels, ["PROB", "DISTMU"]);
        assert_eq!(sc.start, [0]);
        assert_eq!(sc.object.as_deref(), Some("S0001"));
        assert!(sc.sample.starts_with("HEALPix Nside 32"), "{}", sc.sample);
        let (w, h) = (sc.width as usize, sc.height as usize);
        assert_eq!((w, h), (512, 256));
        let wcs = sc.wcs.as_ref().unwrap();
        // The centre of the raster is RA 0, Dec 0.
        let (a, d) = wcs.sky_at(w as f64 / 2.0, h as f64 / 2.0, &[]).unwrap();
        assert!(
            (a.rem_euclid(360.0).min(360.0 - a.rem_euclid(360.0))).abs() < 0.5 && d.abs() < 0.5
        );
        let cr = sc.credible.as_ref().unwrap();
        assert_eq!(cr.index, [0]);
        assert_eq!(cr.levels.len(), 2);
        assert!(cr.levels[0].1 > cr.levels[1].1 && cr.levels[1].1 > 0.0);
        let c = CancelToken::new();
        let p = s.nd_plane(&[1], &c).unwrap().unwrap();
        assert_eq!((p.width, p.height), (512, 256));
        // Outside the ellipse: no data.
        assert!(p.value(0, 0).is_none_or(f32::is_nan));
        assert!(p.value(256, 128).is_some_and(f32::is_finite));
        assert!(s.nd_plane(&[2], &c).unwrap().is_err());
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((r.width, r.height), (512, 256));
    }

    #[test]
    fn multi_order_map_opens() {
        let s = open(fixture("moc.fits"));
        let sc = s.science().unwrap();
        assert_eq!(sc.axes[0].labels, ["PROBDENSITY", "DISTMU"]);
        assert!(sc.sample.contains("NUNIQ"), "{}", sc.sample);
        assert!(sc.credible.is_some());
    }

    #[test]
    fn event_list_bins_by_size_and_band() {
        let s = open(fixture("chandra.fits"));
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.len(), 2);
        assert_eq!(sc.axes[0].kind, AxisKind::Bin);
        assert_eq!(sc.axes[0].len as usize, fits::events::FACTORS.len());
        assert_eq!(sc.axes[1].kind, AxisKind::Band);
        assert_eq!(
            sc.axes[1].labels.first().map(String::as_str),
            Some("0.5–1.2 keV")
        );
        assert_eq!(sc.axes[1].labels.last().map(String::as_str), Some(""));
        assert_eq!(sc.start, [0, sc.axes[1].len - 1]);
        assert!(sc.wcs.is_some());
        assert!(sc.unit.is_none());
        let c = CancelToken::new();
        let all = s.nd_plane(&sc.start, &c).unwrap().unwrap();
        let total: f64 = (0..all.height)
            .flat_map(|y| (0..all.width).map(move |x| (x, y)))
            .filter_map(|(x, y)| all.value(x, y).map(f64::from))
            .sum();
        assert_eq!(total, 6000.0);
        let soft = s.nd_plane(&[0, 0], &c).unwrap().unwrap();
        let some: f64 = (0..soft.height)
            .flat_map(|y| (0..soft.width).map(move |x| (x, y)))
            .filter_map(|(x, y)| soft.value(x, y).map(f64::from))
            .sum();
        assert!(some > 0.0 && some < 6000.0);
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((r.width, r.height), (sc.width, sc.height));
    }

    #[test]
    fn a_table_alone_is_not_an_image() {
        let reg = crate::Registry::new();
        let r = reg.open(Arc::new(Source::from_vec("t.fits", fixture("cat.fits"))));
        assert!(r.is_err());
    }
}

/// Synthetic hostile inputs: none may panic or blow up memory.
#[cfg(test)]
mod hostile {
    use super::*;

    /// A 1x1 byte cube of `planes` frequency channels with `stored` data bytes.
    fn cube(planes: &str, stored: usize) -> Vec<u8> {
        let card = |k: &str, v: &str| format!("{k:<8}= {v:>20}");
        let cards = [
            card("SIMPLE", "T"),
            card("BITPIX", "8"),
            card("NAXIS", "3"),
            card("NAXIS1", "1"),
            card("NAXIS2", "1"),
            card("NAXIS3", planes),
            card("CTYPE3", "'FREQ'"),
            card("CRVAL3", "1.0E9"),
            card("CDELT3", "1.0E3"),
            card("CRPIX3", "1.0"),
            "END".to_owned(),
        ];
        let mut h: Vec<u8> = cards
            .iter()
            .flat_map(|c| format!("{c:<80}").into_bytes())
            .collect();
        h.resize(h.len().div_ceil(2880) * 2880, b' ');
        h.resize(h.len() + stored, 7);
        h
    }

    fn spectra(bytes: Vec<u8>) -> Option<bool> {
        let s = crate::Registry::new()
            .open(Arc::new(Source::from_vec("t.fits", bytes)))
            .ok()?;
        Some(s.science()?.spectra.is_some())
    }

    /// NAXIS3 of four billion over 16 stored bytes: no 32 GB axis of world values.
    #[test]
    fn spectral_axis_past_the_file_is_not_tabulated() {
        assert_ne!(spectra(cube("4000000000", 16)), Some(true));
        // A whole small cube keeps its spectra.
        assert_eq!(spectra(cube("4", 2880)), Some(true));
    }
}
