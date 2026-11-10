//! TIFF-based slides: detection, and the level / associated-image structure of Aperio SVS,
//! Leica SCN, OME-TIFF and generic tiled pyramids (NDPI has its own reader, [`crate::ndpi`]).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use specular_tiff as tf;

use crate::composite::{self, Plane};
use crate::ome::{self, PlaneRef};
use crate::{
    AssociatedKind, Channel, Error, Files, Level, Props, ReadOptions, Region, Result, Rgba, Slide,
    Vendor, aperio, scn,
};

/// The longest side associated images are decoded at (a power-of-two downscale fits them).
const ASSOCIATED_MAX: u32 = 8192;
/// Generic pyramids smaller than this are left to the plain TIFF reader.
const GENERIC_MIN_PIXELS: u64 = 1 << 26;
/// Most channels composited.
const MAX_CHANNELS: usize = 16;

/// The first `n` bytes of `s`, cut at a character boundary.
pub(crate) fn head(s: &str, n: usize) -> &str {
    let mut n = n.min(s.len());
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    &s[..n]
}

/// Which convention a TIFF follows, from its first IFDs.
pub(crate) fn probe(data: &[u8]) -> Option<Vendor> {
    let d = tf::Directories::read_limited(data, 2).ok()?;
    let desc = d.ascii(data, 0, 270).unwrap_or_default();
    if d.has(0, crate::ndpi::TAG_FORMAT_FLAG) {
        return Some(Vendor::Hamamatsu);
    }
    if aperio::is_aperio(&desc) {
        return Some(Vendor::Aperio);
    }
    if scn::is_scn(&desc) {
        return Some(Vendor::Leica);
    }
    if crate::bif::is_bif(&d, data) {
        return Some(Vendor::Ventana);
    }
    if head(&desc, 4096).contains("<OME") || desc.contains("<OME ") {
        return Some(Vendor::OmeTiff);
    }
    let first = d.dirs.first()?.ifd.as_ref()?;
    let big = u64::from(first.width) * u64::from(first.height) >= GENERIC_MIN_PIXELS;
    if !first.tiled || !(big || d.big_tiff) {
        return None;
    }
    // Levels in SubIFDs, or a smaller tiled second IFD.
    let subs = d.has(0, 330);
    let second = d
        .main_chain()
        .get(1)
        .and_then(|&i| d.dirs[i].ifd.as_ref())
        .is_some_and(|s| s.tiled && s.width < first.width && s.height < first.height);
    (subs || second).then_some(Vendor::Generic)
}

/// A plane's pyramid as resolved from its file.
struct Resolved {
    page: tf::Page,
    le: bool,
}

pub(crate) struct TiffSlide {
    /// Planes by `c + C × (z + Z × t)`.
    planes: Vec<Option<PlaneRef>>,
    size_c: u32,
    size_z: u32,
    multichannel: bool,
    /// Associated images: (file, main-chain IFD).
    assoc: Vec<PlaneRef>,
    /// Parsed directories of the files of the set (the opened one at open, others on first
    /// use).
    dirs: Mutex<HashMap<usize, Option<Arc<tf::Directories>>>>,
    pages: Mutex<HashMap<PlaneRef, Option<Arc<Resolved>>>>,
    /// Display windows of the channels, from the file or the coarsest level.
    windows: Mutex<Option<Vec<(f64, f64)>>>,
    channels: Vec<Channel>,
}

/// The main-chain directories, with the SubIFDs of each.
pub(crate) fn page_of(d: &tf::Directories, ifd: u32) -> Option<tf::Page> {
    let mains = d.main_chain();
    let &di = mains.get(ifd as usize)?;
    let base = d.dirs[di].ifd.clone()?;
    let mut levels = vec![base];
    levels.extend(d.sub_ifds(di).cloned());
    tf::Page::from_levels(levels)
}

fn levels_of(p: &tf::Page) -> Vec<Level> {
    let w = f64::from(p.width());
    p.levels()
        .iter()
        .map(|l| Level {
            width: l.width,
            height: l.height,
            downsample: w / f64::from(l.width.max(1)),
        })
        .collect()
}

pub(crate) fn open(data: &[u8]) -> Result<Slide> {
    open_named(data, None)
}

/// Open a TIFF slide; `own_name` is the file's name, for OME-TIFF sets that list it.
pub(crate) fn open_named(data: &[u8], own_name: Option<&str>) -> Result<Slide> {
    let d = tf::Directories::read(data)?;
    let desc = d.ascii(data, 0, 270).unwrap_or_default();
    if d.has(0, crate::ndpi::TAG_FORMAT_FLAG) {
        return crate::ndpi::open(data, &d);
    }
    if aperio::is_aperio(&desc) {
        return open_aperio(data, d, &desc);
    }
    if scn::is_scn(&desc) {
        return open_scn(d, &desc);
    }
    if crate::bif::is_bif(&d, data) {
        return crate::bif::open(data, d);
    }
    if desc.contains("<OME") {
        if ome::binary_only(&desc).is_some() {
            return Err(Error::Unsupported(
                "OME-TIFF whose metadata is in a companion file".into(),
            ));
        }
        let uuid = ome::parse(&desc).ok().and_then(|o| o.uuid);
        return open_ome(d, &desc, own_name, uuid);
    }
    open_generic(d, data)
}

/// Open one file of an OME-TIFF set with the set's OME-XML read from elsewhere (the companion
/// file a "BinaryOnly" file names).
pub(crate) fn open_with_metadata(data: &[u8], own_name: Option<&str>, xml: &str) -> Result<Slide> {
    let d = tf::Directories::read(data)?;
    let desc = d.ascii(data, 0, 270).unwrap_or_default();
    let uuid = ome::binary_only(&desc).and_then(|b| b.1);
    open_ome(d, xml, own_name, uuid)
}

/// The companion metadata file a "BinaryOnly" OME-TIFF names.
pub(crate) fn metadata_file(data: &[u8]) -> Option<String> {
    let d = tf::Directories::read_limited(data, 1).ok()?;
    let desc = d.ascii(data, 0, 270)?;
    ome::binary_only(&desc).map(|b| b.0)
}

fn assoc_kind(desc: &str) -> Option<AssociatedKind> {
    let l = desc.to_ascii_lowercase();
    if l.contains("label") {
        Some(AssociatedKind::Label)
    } else if l.contains("macro") || l.contains("overview") {
        Some(AssociatedKind::Macro)
    } else if l.contains("thumbnail") {
        Some(AssociatedKind::Thumbnail)
    } else {
        None
    }
}

fn single_plane(
    d: tf::Directories,
    props: Props,
    page: tf::Page,
    base: u32,
    assoc: Vec<(AssociatedKind, PlaneRef)>,
) -> Result<Slide> {
    page.check_supported()?;
    let levels = levels_of(&page);
    let (w, h) = (page.width(), page.height());
    let fmt = page.output_format();
    let multichannel = !matches!(fmt, tf::PixelFormat::Rgba8 | tf::PixelFormat::Gray8);
    let assoc_sizes = assoc
        .iter()
        .filter_map(|(k, r)| {
            let p = page_of(&d, r.ifd)?;
            Some((k.clone(), p.width(), p.height()))
        })
        .collect::<Vec<_>>();
    let assoc_refs = assoc
        .iter()
        .filter(|(_, r)| page_of(&d, r.ifd).is_some())
        .map(|(_, r)| *r)
        .collect();
    let mut props = props;
    if props.sample.is_empty() {
        props.sample = page.ifd().description();
    }
    let le = d.little_endian;
    let mut pages = HashMap::new();
    let r = PlaneRef { file: 0, ifd: base };
    pages.insert(r, Some(Arc::new(Resolved { page, le })));
    let mut dirs = HashMap::new();
    dirs.insert(0, Some(Arc::new(d)));
    Ok(Slide {
        props,
        width: w,
        height: h,
        levels,
        channels: vec![Channel::default()],
        size_z: 1,
        size_t: 1,
        associated: assoc_sizes,
        files: Vec::new(),
        backend: crate::Backend::Tiff(TiffSlide {
            planes: vec![Some(r)],
            size_c: 1,
            size_z: 1,
            multichannel,
            assoc: assoc_refs,
            dirs: Mutex::new(dirs),
            pages: Mutex::new(pages),
            windows: Mutex::new(None),
            channels: vec![Channel::default()],
        }),
    })
}

fn open_aperio(data: &[u8], d: tf::Directories, desc: &str) -> Result<Slide> {
    let mains = d.main_chain();
    let mut level_ifds = Vec::new();
    let mut assoc = Vec::new();
    let mut base = None;
    for (k, &di) in mains.iter().enumerate() {
        let Some(ifd) = d.dirs[di].ifd.as_ref() else {
            continue;
        };
        let text = d.ascii(data, di, 270).unwrap_or_default();
        if ifd.tiled {
            base.get_or_insert(k as u32);
            level_ifds.push(ifd.clone());
            continue;
        }
        let kind = assoc_kind(&text).unwrap_or(if k == 1 {
            AssociatedKind::Thumbnail
        } else {
            AssociatedKind::Other(format!("{}", k + 1))
        });
        assoc.push((
            kind,
            PlaneRef {
                file: 0,
                ifd: k as u32,
            },
        ));
    }
    let base = base.ok_or_else(|| Error::Malformed("SVS without tiled levels".into()))?;
    let page = tf::Page::from_levels(level_ifds)
        .ok_or_else(|| Error::Malformed("SVS without levels".into()))?;
    single_plane(d, aperio::props(desc), page, base, assoc)
}

fn open_scn(d: tf::Directories, desc: &str) -> Result<Slide> {
    let s = scn::parse(desc)?;
    let main = s
        .images
        .iter()
        .filter(|i| !i.macro_image)
        .max_by_key(|i| {
            i.dims
                .first()
                .map_or(0, |d| u64::from(d.0) * u64::from(d.1))
        })
        .or(s.images.first())
        .ok_or_else(|| Error::Malformed("SCN without images".into()))?;
    let mains = d.main_chain();
    let ifd_at = |k: u32| mains.get(k as usize).and_then(|&di| d.dirs[di].ifd.clone());
    let levels: Vec<tf::Ifd> = main.dims.iter().filter_map(|x| ifd_at(x.2)).collect();
    let page = tf::Page::from_levels(levels)
        .ok_or_else(|| Error::Malformed("SCN image without readable levels".into()))?;
    let mut props = Props::new(Vendor::Leica);
    props.mpp = main.nm_per_px.map(|n| (n / 1000.0, n / 1000.0));
    props.objective = main.objective;
    props.scanner = main.device.clone();
    props.date = main.date.clone();
    props.name = Some(main.name.clone()).filter(|n| !n.is_empty());
    if let Some(b) = &s.barcode {
        props.properties.push(("Barcode".to_owned(), b.clone()));
    }
    if let Some(i) = &main.illumination {
        props
            .properties
            .push(("Illumination".to_owned(), i.clone()));
    }
    let base = main.dims[0].2;
    let assoc = s
        .images
        .iter()
        .filter(|i| !std::ptr::eq(*i, main))
        .filter_map(|i| {
            // The smallest resolution of the others suffices for a page.
            let &(_, _, ifd) = i.dims.first()?;
            let kind = if i.macro_image {
                AssociatedKind::Macro
            } else {
                AssociatedKind::Other(i.name.clone())
            };
            Some((kind, PlaneRef { file: 0, ifd }))
        })
        .collect();
    single_plane(d, props, page, base, assoc)
}

fn open_generic(d: tf::Directories, data: &[u8]) -> Result<Slide> {
    let mains = d.main_chain();
    let first = d
        .dirs
        .first()
        .and_then(|x| x.ifd.clone())
        .ok_or(Error::NotSlide)?;
    if !first.tiled {
        return Err(Error::NotSlide);
    }
    let mut levels = vec![first.clone()];
    levels.extend(d.sub_ifds(0).cloned());
    let mut assoc = Vec::new();
    let aspect = f64::from(first.width) / f64::from(first.height.max(1));
    for (k, &di) in mains.iter().enumerate().skip(1) {
        let Some(ifd) = d.dirs[di].ifd.as_ref() else {
            continue;
        };
        let a = f64::from(ifd.width) / f64::from(ifd.height.max(1));
        let text = d.ascii(data, di, 270).unwrap_or_default();
        let reduced = ifd.subfile_type() & 1 != 0;
        if ifd.tiled
            && ifd.width < first.width
            && (reduced || (a / aspect - 1.0).abs() < 0.02)
            && assoc_kind(&text).is_none()
        {
            levels.push(ifd.clone());
        } else if ifd.subfile_type() & 4 == 0 {
            let kind = assoc_kind(&text).unwrap_or(AssociatedKind::Other(format!("{}", k + 1)));
            assoc.push((
                kind,
                PlaneRef {
                    file: 0,
                    ifd: k as u32,
                },
            ));
        }
    }
    if levels.len() < 2 {
        return Err(Error::NotSlide);
    }
    let page = tf::Page::from_levels(levels).ok_or(Error::NotSlide)?;
    if page.levels().len() < 2 {
        return Err(Error::NotSlide);
    }
    let mut props = Props::new(Vendor::Generic);
    // Resolution tags (ResolutionUnit centimetre or inch) give microns per pixel.
    let res = |tag| d.reals(data, 0, tag).and_then(|v| v.first().copied());
    let unit = d
        .uints(data, 0, 296)
        .and_then(|v| v.first().copied())
        .unwrap_or(2);
    if let (Some(xr), Some(yr)) = (res(282), res(283))
        && xr > 0.0
        && yr > 0.0
        && unit == 3
    {
        props.mpp = Some((1e4 / xr, 1e4 / yr));
    }
    props.date = d.ascii(data, 0, 306).filter(|s| !s.trim().is_empty());
    props.scanner = d
        .ascii(data, 0, 272)
        .or_else(|| d.ascii(data, 0, 305))
        .filter(|s| !s.trim().is_empty());
    single_plane(d, props, page, 0, assoc)
}

/// `own_uuid`: the described file's UUID (its own OME-XML's, also when `desc` is a companion's).
fn open_ome(
    d: tf::Directories,
    desc: &str,
    own_name: Option<&str>,
    own_uuid: Option<String>,
) -> Result<Slide> {
    let o = ome::parse(desc)?;
    let own = |name: Option<&str>, uuid: Option<&str>| match (name, uuid) {
        (Some(n), _) if Some(n) == own_name => true,
        (_, Some(u)) if Some(u) == own_uuid.as_deref() => true,
        (None, _) => true,
        _ => false,
    };
    // Images without TiffData follow each other in IFD order.
    let mut first_ifd = 0u32;
    let mut maps = Vec::new();
    for im in &o.images {
        let (map, files) = im.plane_map(&own, first_ifd);
        if im.tiff_data.is_empty() {
            first_ifd = first_ifd.saturating_add(im.plane_count().min(u64::from(u32::MAX)) as u32);
        }
        maps.push((map, files));
    }
    // The main image: the first not named like an associated image.
    let main = o
        .images
        .iter()
        .position(|im| im.name.as_deref().and_then(assoc_kind).is_none())
        .unwrap_or(0);
    let im = &o.images[main];
    let (map, files) = maps[main].clone();
    let file0 = |r: &Option<PlaneRef>| r.filter(|r| r.file == 0);
    if map.first().copied().flatten().is_none() {
        return Err(Error::Malformed("OME-TIFF image without planes".into()));
    }
    // The geometry comes from a plane of this file (all planes of an image share it).
    let base_ref = map
        .iter()
        .flatten()
        .copied()
        .find(|r| r.file == 0)
        .ok_or_else(|| Error::Unsupported("OME-TIFF without planes in the opened file".into()))?;
    let (page, le) = (
        page_of(&d, base_ref.ifd)
            .ok_or_else(|| Error::Malformed("OME-TIFF plane IFD missing".into()))?,
        d.little_endian,
    );
    page.check_supported()?;
    let samples = im.samples();
    let size_c = im.effective_c();
    let fmt = page.output_format();
    let multichannel =
        size_c > 1 || !matches!(fmt, tf::PixelFormat::Rgba8 | tf::PixelFormat::Gray8);
    // SizeC is from the XML: no more channels than stored planes.
    let channels: Vec<Channel> = (0..(size_c as usize).min(map.len()))
        .map(|k| {
            let c = im.channels.get(k);
            Channel {
                name: c.map(|c| c.name.clone()).unwrap_or_default(),
                colour: c.and_then(|c| c.colour),
                window: None,
            }
        })
        .collect();
    let mut props = Props::new(Vendor::OmeTiff);
    props.mpp = match im.physical {
        [Some(x), Some(y), _] => Some((x, y)),
        [Some(x), None, _] => Some((x, x)),
        _ => None,
    };
    props.z_step = im.physical[2];
    props.t_step = im.time_increment;
    props.objective = im.objective;
    props.scanner = im.microscope.clone();
    props.date = im.acquired.clone();
    props.name = im.name.clone();
    props.sample = format!(
        "{}{}, {}",
        im.pixel_type,
        if samples >= 3 { " RGB" } else { "" },
        page.ifd().compression.name()
    );
    if let Some(c) = &o.creator {
        props.properties.push(("Creator".to_owned(), c.clone()));
    }
    let assoc: Vec<(AssociatedKind, PlaneRef)> = o
        .images
        .iter()
        .enumerate()
        .filter(|&(k, _)| k != main)
        .filter_map(|(k, other)| {
            let r = file0(maps[k].0.first()?)?;
            let kind = other
                .name
                .as_deref()
                .and_then(assoc_kind)
                .unwrap_or_else(|| {
                    AssociatedKind::Other(
                        other.name.clone().unwrap_or_else(|| format!("{}", k + 1)),
                    )
                });
            Some((kind, r))
        })
        .collect();
    let assoc_sizes = assoc
        .iter()
        .filter_map(|(k, r)| {
            let p = page_of(&d, r.ifd)?;
            Some((k.clone(), p.width(), p.height()))
        })
        .collect();
    let levels = levels_of(&page);
    let (w, h) = (page.width(), page.height());
    let mut pages = HashMap::new();
    pages.insert(base_ref, Some(Arc::new(Resolved { page, le })));
    let mut dirs = HashMap::new();
    dirs.insert(0, Some(Arc::new(d)));
    Ok(Slide {
        props,
        width: w,
        height: h,
        levels,
        channels: channels.clone(),
        size_z: im.size_z,
        size_t: im.size_t,
        associated: assoc_sizes,
        files,
        backend: crate::Backend::Tiff(TiffSlide {
            planes: map,
            size_c,
            size_z: im.size_z,
            multichannel,
            assoc: assoc.into_iter().map(|(_, r)| r).collect(),
            dirs: Mutex::new(dirs),
            pages: Mutex::new(pages),
            windows: Mutex::new(None),
            channels,
        }),
    })
}

impl TiffSlide {
    pub(crate) fn multichannel(&self) -> bool {
        self.multichannel
    }

    fn resolve(&self, files: Files, r: PlaneRef) -> Result<Arc<Resolved>> {
        if let Some(p) = self.pages.lock().get(&r) {
            return p
                .clone()
                .ok_or_else(|| Error::Malformed("plane IFD unreadable".into()));
        }
        let data = files.get(r.file)?;
        let dirs = {
            let mut m = self.dirs.lock();
            m.entry(r.file)
                .or_insert_with(|| tf::Directories::read(data).ok().map(Arc::new))
                .clone()
        };
        let res = dirs
            .and_then(|d| Some((page_of(&d, r.ifd)?, d.little_endian)))
            .filter(|(p, _)| p.check_supported().is_ok())
            .map(|(page, le)| Arc::new(Resolved { page, le }));
        self.pages.lock().insert(r, res.clone());
        res.ok_or_else(|| Error::Malformed("plane IFD unreadable".into()))
    }

    fn plane_ref(&self, c: u32, z: u32, t: u32) -> Result<PlaneRef> {
        let zt = u64::from(z).saturating_add(u64::from(self.size_z).saturating_mul(u64::from(t)));
        let i = u64::from(c).saturating_add(u64::from(self.size_c).saturating_mul(zt));
        self.planes
            .get(i as usize)
            .copied()
            .flatten()
            .ok_or_else(|| Error::Malformed("plane not stored".into()))
    }

    fn decode(
        &self,
        files: Files,
        r: PlaneRef,
        o: &ReadOptions,
        wide: bool,
    ) -> Result<tf::Decoded> {
        let res = self.resolve(files, r)?;
        let data = files.get(r.file)?;
        let reg = o.region;
        Ok(res.page.decode(
            data,
            res.le,
            &tf::DecodeOptions {
                scale: o.scale,
                region: reg.map(|r| tf::Region {
                    x: r.x,
                    y: r.y,
                    w: r.w,
                    h: r.h,
                }),
                cancel: o.cancel,
                parallel: o.parallel,
                wide,
            },
        )?)
    }

    pub(crate) fn read_channel(&self, files: Files, c: u32, o: &ReadOptions) -> Result<Plane> {
        let r = self.plane_ref(c, o.position.z, o.position.t)?;
        let res = self.resolve(files, r)?;
        let ifd = res.page.ifd();
        let signed = (ifd.sample_format == tf::SampleFormat::Int).then_some(ifd.bits);
        Ok(composite::tiff_plane(
            self.decode(files, r, o, true)?,
            signed,
        ))
    }

    /// Each channel's window: from the coarsest level of the first plane.
    fn windows(
        &self,
        slide: &Slide,
        files: Files,
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<(f64, f64)>> {
        if let Some(w) = self.windows.lock().clone() {
            return Ok(w);
        }
        let n = (self.size_c as usize).min(MAX_CHANNELS);
        let mut scale = 1u32;
        while scale < 1 << 16 && (slide.width / scale > 1024 || slide.height / scale > 1024) {
            scale *= 2;
        }
        let mut out = Vec::with_capacity(n);
        for c in 0..n {
            if let Some(w) = self.channels.get(c).and_then(|c| c.window) {
                out.push(w);
                continue;
            }
            let o = ReadOptions {
                scale,
                region: None,
                position: Default::default(),
                cancel,
                parallel: true,
            };
            let p = self.read_channel(files, c as u32, &o)?;
            out.push(composite::auto_window(&p.values));
        }
        *self.windows.lock() = Some(out.clone());
        Ok(out)
    }

    pub(crate) fn read(&self, slide: &Slide, files: Files, o: &ReadOptions) -> Result<Rgba> {
        if !self.multichannel {
            let r = self.plane_ref(0, o.position.z, o.position.t)?;
            return Ok(composite::tiff_rgba(self.decode(files, r, o, false)?));
        }
        let windows = self.windows(slide, files, o.cancel)?;
        let n = (self.size_c as usize).min(MAX_CHANNELS);
        let mut planes = Vec::with_capacity(n);
        for c in 0..n {
            crate::check_cancel(o.cancel)?;
            planes.push(self.read_channel(files, c as u32, o)?);
        }
        Ok(composite::composite(&planes, &self.channels, &windows))
    }

    pub(crate) fn read_associated(
        &self,
        files: Files,
        i: usize,
        cancel: Option<&AtomicBool>,
    ) -> Result<Rgba> {
        let r = *self
            .assoc
            .get(i)
            .ok_or_else(|| Error::Malformed("no such associated image".into()))?;
        let res = self.resolve(files, r)?;
        let (w, h) = (res.page.width(), res.page.height());
        let mut scale = 1;
        while w / scale > ASSOCIATED_MAX || h / scale > ASSOCIATED_MAX {
            scale *= 2;
        }
        let o = ReadOptions {
            scale,
            region: Some(Region {
                x: 0,
                y: 0,
                w: w.div_ceil(scale),
                h: h.div_ceil(scale),
            }),
            position: Default::default(),
            cancel,
            parallel: true,
        };
        Ok(composite::tiff_rgba(self.decode(files, r, &o, false)?))
    }
}
