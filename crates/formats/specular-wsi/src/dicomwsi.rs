//! DICOM VL Whole Slide Microscopy images (PS3.3 A.32.8): a series of instances, one per
//! level (or per part of a level: concatenations, focal planes, optical paths), read as one
//! pyramid.
//!
//! - Each instance is a Total Pixel Matrix cut into Rows × Columns tiles, one frame each:
//!   TILED_FULL (frames in x, then y, then focal plane, then optical path order) or
//!   TILED_SPARSE (each frame's Plane Position (Slide) in the per-frame functional groups;
//!   tiles missing from the matrix read as white).
//! - Levels are the VOLUME instances by matrix size, the largest at full resolution; a level
//!   serves a power-of-two downscale when its size is the base size divided by it.
//! - One focal plane and optical path show: the first frame's of the base level, and the
//!   nearest focal plane of the same path at the other levels.
//! - LABEL, OVERVIEW and THUMBNAIL instances (of the series, or of the same slide in another
//!   series of the study) are the label, macro and thumbnail images.
//! - Microns per pixel from the base level's Pixel Spacing; frames decode one at a time
//!   through `specular-dicom` (any transfer syntax it decodes).
//!
//! No patient-identifying attributes are read into the slide's facts.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use rayon::prelude::*;
use specular_dicom as dcm;
use specular_dicom::{DataSet, Dicom, Pixels, Values};

use crate::{
    AssociatedKind, Channel, Error, Files, Level, Props, ReadOptions, Region, Result, Rgba, Slide,
    Vendor, box_reduce_rgba,
};

/// VL Whole Slide Microscopy Image Storage.
pub const WSI_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";

/// Associated images are decoded at most this large (then downscaled by powers of two).
const ASSOCIATED_MAX: u32 = 8192;
/// Tiles one grid may have (a sanity cap far above real slides).
const MAX_TILES: u64 = 1 << 24;

/// Tiles of the coarsest level sampled for a grey display window.
const WINDOW_TILES: usize = 64;

const SOP_CLASS: u32 = 0x0008_0016;
const WINDOW_CENTER: u32 = 0x0028_1050;
const WINDOW_WIDTH: u32 = 0x0028_1051;
const IMAGE_TYPE: u32 = 0x0008_0008;
const STUDY_UID: u32 = 0x0020_000D;
const SERIES_UID: u32 = 0x0020_000E;
const FRAME_OF_REFERENCE: u32 = 0x0020_0052;
const CONTAINER_ID: u32 = 0x0040_0512;
const INSTANCE_NUMBER: u32 = 0x0020_0013;
const TPM_COLUMNS: u32 = 0x0048_0006;
const TPM_ROWS: u32 = 0x0048_0007;
const TPM_FOCAL_PLANES: u32 = 0x0048_0303;
const OPTICAL_PATHS: u32 = 0x0048_0302;
const DIMENSION_ORGANIZATION: u32 = 0x0020_9311;
const CONCATENATION_OFFSET: u32 = 0x0020_9228;
const SHARED_GROUPS: u32 = 0x5200_9229;
const PER_FRAME_GROUPS: u32 = 0x5200_9230;
const PLANE_POSITION_SLIDE: u32 = 0x0048_021A;
const COLUMN_POSITION: u32 = 0x0048_021E;
const ROW_POSITION: u32 = 0x0048_021F;
const Z_OFFSET: u32 = 0x0040_074A;
const OPTICAL_PATH_SEQUENCE: u32 = 0x0048_0105;
const OPTICAL_PATH_ID_SEQUENCE: u32 = 0x0048_0207;
const OPTICAL_PATH_ID: u32 = 0x0048_0106;
const OBJECTIVE_POWER: u32 = 0x0048_0112;
const PIXEL_MEASURES: u32 = 0x0028_9110;
const PIXEL_SPACING: u32 = 0x0028_0030;
const SLICE_SPACING: u32 = 0x0018_0088;

/// Whether a file's first bytes are a Part 10 header naming the WSI SOP class (the Media
/// Storage SOP Class UID is in the file meta information, near the start).
pub fn probe(head: &[u8]) -> bool {
    let head = &head[..head.len().min(4096)];
    head.get(128..132) == Some(b"DICM")
        && head
            .windows(WSI_SOP_CLASS.len())
            .any(|w| w == WSI_SOP_CLASS.as_bytes())
}

fn dcm_err(e: dcm::Error) -> Error {
    match e {
        dcm::Error::Cancelled => Error::Cancelled,
        dcm::Error::Truncated => Error::Truncated("DICOM data".into()),
        dcm::Error::TooLarge(_) => Error::TooLarge,
        dcm::Error::Unsupported(s) => Error::Unsupported(s),
        dcm::Error::NotDicom => Error::NotSlide,
        e => Error::Malformed(e.to_string()),
    }
}

fn text(ds: &DataSet, v: &Values, tag: u32) -> Option<String> {
    ds.get(tag)
        .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
        .map(|s| s.trim_end_matches('\0').trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The default VOI window of `ds` as (low, high) stored values: the image's first Window
/// Center/Width, else that of the first frame's Frame VOI LUT Sequence (Per-frame, then
/// Shared Functional Groups).
fn voi_window(ds: &DataSet, v: &Values) -> Option<(f64, f64)> {
    const FRAME_VOI_LUT: u32 = 0x0028_9132;
    const PER_FRAME: u32 = 0x5200_9230;
    const SHARED: u32 = 0x5200_9229;
    let first = |d: &DataSet, tag| {
        text(d, v, tag).and_then(|t| {
            t.split('\\')
                .next()
                .and_then(|x| x.trim().parse::<f64>().ok())
        })
    };
    let per_frame = ds
        .get(PER_FRAME)
        .and_then(|e| e.items.first())
        .and_then(|i| i.item(FRAME_VOI_LUT));
    let shared = ds.item(SHARED).and_then(|i| i.item(FRAME_VOI_LUT));
    [Some(ds), per_frame, shared]
        .into_iter()
        .flatten()
        .find_map(
            |d| match (first(d, WINDOW_CENTER), first(d, WINDOW_WIDTH)) {
                (Some(c), Some(w)) if w >= 1.0 => {
                    Some((c - 0.5 - (w - 1.0) / 2.0, c - 0.5 + (w - 1.0) / 2.0))
                }
                _ => None,
            },
        )
}

fn int(ds: &DataSet, v: &Values, tag: u32) -> Option<i64> {
    ds.get(tag).and_then(|e| v.int(e))
}

fn num(ds: &DataSet, v: &Values, tag: u32, i: usize) -> Option<f64> {
    ds.get(tag).and_then(|e| v.number(e, i))
}

/// The first item of `seq` in the per-frame groups of `frame`, else in the shared ones.
fn functional(ds: &DataSet, frame: u32, seq: u32) -> Option<&DataSet> {
    ds.get(PER_FRAME_GROUPS)
        .and_then(|e| e.items.get(frame as usize))
        .and_then(|i| i.item(seq))
        .or_else(|| ds.item(SHARED_GROUPS).and_then(|i| i.item(seq)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Volume,
    Label,
    Overview,
    Thumbnail,
}

/// One frame's place: tile column and row, focal plane (µm, or the plane's index when the
/// file gives no offset) and optical path.
#[derive(Clone, Debug)]
struct Place {
    col: u32,
    row: u32,
    z: f64,
    path: String,
}

/// A parsed instance of the series.
struct Inst {
    /// Index into [`Files`].
    file: usize,
    dicom: Dicom,
    kind: Kind,
    series: String,
    study: String,
    /// Frame of Reference or Container Identifier: what ties a label in another series to
    /// the slide.
    slide_key: Option<String>,
    instance_number: i64,
    width: u32,
    height: u32,
    tile_w: u32,
    tile_h: u32,
    places: Vec<Place>,
    tiled_full: bool,
    /// Optical path identifiers in the Optical Path Sequence's order.
    paths: Vec<String>,
    /// Pixel Spacing (mm), between rows then columns.
    spacing: Option<(f64, f64)>,
}

/// A tile grid: one level (or associated image) of one focal plane and optical path.
#[derive(Debug)]
struct Grid {
    width: u32,
    height: u32,
    tile_w: u32,
    tile_h: u32,
    cols: u32,
    /// `(instance, frame)` of each tile, row-major; `None` for tiles not stored.
    tiles: Vec<Option<(u32, u32)>>,
}

pub(crate) struct DicomSlide {
    insts: Vec<Inst>,
    /// Matching [`Slide::levels`].
    levels: Vec<Grid>,
    /// Matching [`Slide::associated`].
    assoc: Vec<Grid>,
    /// The pyramid's display window for grey samples over 8 bits (fluorescence is often 16-bit
    /// and dim): the base's VOI window, else the coarsest level's histogram. Found on the
    /// first read; `Some(None)` for 8-bit and colour pyramids.
    window: Mutex<Option<Option<(f64, f64)>>>,
}

impl std::fmt::Debug for DicomSlide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DicomSlide")
            .field("instances", &self.insts.len())
            .field("levels", &self.levels)
            .finish_non_exhaustive()
    }
}

fn parse_inst(file: usize, data: &[u8]) -> Option<Inst> {
    let dicom = Dicom::parse(data).ok()?;
    let v = dicom.values(data);
    let ds = &dicom.dataset;
    if text(ds, &v, SOP_CLASS).as_deref() != Some(WSI_SOP_CLASS) {
        return None;
    }
    let info = dicom.pixel.as_ref()?;
    let kind = match ds
        .get(IMAGE_TYPE)
        .and_then(|e| v.text_n(e, 2))
        .map(str::to_ascii_uppercase)
        .as_deref()
    {
        Some("LABEL") => Kind::Label,
        Some("OVERVIEW") => Kind::Overview,
        Some("THUMBNAIL") => Kind::Thumbnail,
        _ => Kind::Volume,
    };
    let (tile_w, tile_h) = (info.columns, info.rows);
    if tile_w == 0 || tile_h == 0 {
        return None;
    }
    let width = int(ds, &v, TPM_COLUMNS).map_or(tile_w, |x| x.clamp(1, u32::MAX as i64) as u32);
    let height = int(ds, &v, TPM_ROWS).map_or(tile_h, |x| x.clamp(1, u32::MAX as i64) as u32);
    let (tx, ty) = (width.div_ceil(tile_w), height.div_ceil(tile_h));
    if u64::from(tx) * u64::from(ty) > MAX_TILES {
        return None;
    }
    // Every frame takes a fragment item (8 bytes) or a tile of samples in the file: no more
    // frames than the file can hold (Number of Frames is unchecked).
    let frame_min = (u64::from(tile_w) * u64::from(tile_h))
        .saturating_mul(u64::from(info.samples.max(1)) * u64::from(info.bits_allocated.max(1)))
        .div_ceil(8)
        .clamp(1, 8);
    let frames = info
        .frames
        .min(u32::try_from(data.len() as u64 / frame_min).unwrap_or(u32::MAX))
        .max(1);
    // Optical path identifiers in the Optical Path Sequence's order.
    let paths: Vec<String> = ds
        .get(OPTICAL_PATH_SEQUENCE)
        .map(|e| {
            e.items
                .iter()
                .enumerate()
                .map(|(i, it)| text(it, &v, OPTICAL_PATH_ID).unwrap_or_else(|| i.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let path_of = |i: usize| paths.get(i).cloned().unwrap_or_else(|| i.to_string());
    let organisation = text(ds, &v, DIMENSION_ORGANIZATION);
    let sparse_positions = ds
        .get(PER_FRAME_GROUPS)
        .and_then(|e| e.items.first())
        .is_some_and(|i| i.get(PLANE_POSITION_SLIDE).is_some());
    let tiled_full = organisation.as_deref() == Some("TILED_FULL") || !sparse_positions;
    let mut places = Vec::with_capacity(frames as usize);
    if tiled_full {
        let planes = int(ds, &v, TPM_FOCAL_PLANES).unwrap_or(1).max(1) as u64;
        let npaths = int(ds, &v, OPTICAL_PATHS).unwrap_or(1).max(1) as u64;
        let offset = int(ds, &v, CONCATENATION_OFFSET).unwrap_or(0).max(0) as u64;
        // The focal planes' Z: the shared plane position's offset, Spacing Between Slices
        // apart (µm), else the planes' indices.
        let z0 = ds
            .item(SHARED_GROUPS)
            .and_then(|g| g.item(PLANE_POSITION_SLIDE))
            .and_then(|p| num(p, &v, Z_OFFSET, 0));
        let dz = ds
            .item(SHARED_GROUPS)
            .and_then(|g| g.item(PIXEL_MEASURES))
            .and_then(|p| num(p, &v, SLICE_SPACING, 0))
            .map(|mm| mm * 1000.0);
        let per_plane = u64::from(tx) * u64::from(ty);
        for k in 0..frames as u64 {
            let g = offset.saturating_add(k);
            let zi = (g / per_plane) % planes;
            let pi = (g / per_plane.saturating_mul(planes)).min(npaths - 1);
            let z = match (z0, dz) {
                (Some(z0), Some(dz)) => z0 + zi as f64 * dz,
                _ => zi as f64,
            };
            places.push(Place {
                col: (g % u64::from(tx)) as u32,
                row: ((g / u64::from(tx)) % u64::from(ty)) as u32,
                z,
                path: path_of(pi as usize),
            });
        }
    } else {
        let shared_path = ds
            .item(SHARED_GROUPS)
            .and_then(|g| g.item(OPTICAL_PATH_ID_SEQUENCE))
            .and_then(|p| text(p, &v, OPTICAL_PATH_ID));
        let per = ds.get(PER_FRAME_GROUPS).map_or(&[][..], |e| &e.items[..]);
        for k in 0..frames as usize {
            let item = per.get(k);
            let pos = item.and_then(|i| i.item(PLANE_POSITION_SLIDE));
            let (Some(c), Some(r)) = (
                pos.and_then(|p| int(p, &v, COLUMN_POSITION)),
                pos.and_then(|p| int(p, &v, ROW_POSITION)),
            ) else {
                // Without a position the frame cannot be placed; keep the frame index aligned.
                places.push(Place {
                    col: u32::MAX,
                    row: u32::MAX,
                    z: 0.0,
                    path: String::new(),
                });
                continue;
            };
            let z = pos.and_then(|p| num(p, &v, Z_OFFSET, 0)).unwrap_or(0.0);
            let path = item
                .and_then(|i| i.item(OPTICAL_PATH_ID_SEQUENCE))
                .and_then(|p| text(p, &v, OPTICAL_PATH_ID))
                .or_else(|| shared_path.clone())
                .unwrap_or_else(|| path_of(0));
            // 1-based positions of the tile's top left pixel.
            let (c, r) = (
                c.saturating_sub(1).max(0) as u64,
                r.saturating_sub(1).max(0) as u64,
            );
            places.push(Place {
                col: (c / u64::from(tile_w)).min(u64::from(u32::MAX)) as u32,
                row: (r / u64::from(tile_h)).min(u64::from(u32::MAX)) as u32,
                z,
                path,
            });
        }
    }
    let spacing = functional(ds, 0, PIXEL_MEASURES)
        .or(Some(ds))
        .and_then(|p| Some((num(p, &v, PIXEL_SPACING, 0)?, num(p, &v, PIXEL_SPACING, 1)?)))
        .filter(|&(r, c)| r > 0.0 && c > 0.0);
    let series = text(ds, &v, SERIES_UID).unwrap_or_default();
    let study = text(ds, &v, STUDY_UID).unwrap_or_default();
    let slide_key = text(ds, &v, CONTAINER_ID).or_else(|| text(ds, &v, FRAME_OF_REFERENCE));
    let instance_number = int(ds, &v, INSTANCE_NUMBER).unwrap_or(0);
    Some(Inst {
        file,
        dicom,
        kind,
        series,
        study,
        slide_key,
        instance_number,
        width,
        height,
        tile_w,
        tile_h,
        places,
        tiled_full,
        paths,
        spacing,
    })
}

/// Build the grid of instances `members` (all of one matrix size) for the optical path
/// `path` (else the first frame's) and the focal plane nearest `z` (else the first frame's).
fn grid(insts: &[Inst], members: &[usize], path: Option<&str>, z: Option<f64>) -> Option<Grid> {
    let first = insts.get(*members.first()?)?;
    let (w, h, tw, th) = (first.width, first.height, first.tile_w, first.tile_h);
    let has_path = |p: &str| {
        members
            .iter()
            .any(|&m| insts[m].places.iter().any(|pl| pl.path == p))
    };
    let path = match path {
        Some(p) if has_path(p) => p.to_owned(),
        _ => first.places.first()?.path.clone(),
    };
    let want_z = z.unwrap_or(first.places.first()?.z);
    let z = members
        .iter()
        .flat_map(|&m| insts[m].places.iter())
        .filter(|p| p.path == path && p.col != u32::MAX)
        .map(|p| p.z)
        .min_by(|a, b| (a - want_z).abs().total_cmp(&(b - want_z).abs()))?;
    let (cols, rows) = (w.div_ceil(tw), h.div_ceil(th));
    let mut tiles = vec![None; cols as usize * rows as usize];
    for &m in members {
        let inst = &insts[m];
        if (inst.tile_w, inst.tile_h) != (tw, th) {
            continue;
        }
        for (f, p) in inst.places.iter().enumerate() {
            if p.path != path || p.z != z || p.col >= cols || p.row >= rows {
                continue;
            }
            let slot = &mut tiles[p.row as usize * cols as usize + p.col as usize];
            if slot.is_none() {
                *slot = Some((m as u32, f as u32));
            }
        }
    }
    Some(Grid {
        width: w,
        height: h,
        tile_w: tw,
        tile_h: th,
        cols,
        tiles,
    })
}

/// Open the slide that the file `members[0]` belongs to, from it and the other candidate
/// files `members[1..]` (name, bytes). [`Slide::files`] lists the members used besides the
/// first, so [`Files`] index 0 is `members[0]` and the rest follow in that order.
pub(crate) fn open(members: &[(&str, &[u8])]) -> Result<Slide> {
    let (_, own) = members.first().ok_or(Error::NotSlide)?;
    let first = parse_inst(0, own).ok_or(Error::NotSlide)?;
    let parsed: Vec<Inst> = members[1..]
        .par_iter()
        .enumerate()
        .filter(|(_, (_, d))| probe(d))
        .filter_map(|(i, (_, d))| parse_inst(i + 1, d))
        .collect();
    let mut all = vec![first];
    all.extend(parsed);
    let (own_study, own_key, own_series) = (
        all[0].study.clone(),
        all[0].slide_key.clone(),
        all[0].series.clone(),
    );
    // The pyramid: VOLUME instances of the opened file's series, else (a label opened from a
    // series of its own) of the same slide in the study.
    let same_slide =
        |i: &Inst| i.study == own_study && i.slide_key.is_some() && i.slide_key == own_key;
    let mut series = own_series;
    if !all
        .iter()
        .any(|i| i.kind == Kind::Volume && i.series == series)
        && let Some(i) = all.iter().find(|i| i.kind == Kind::Volume && same_slide(i))
    {
        series = i.series.clone();
    }
    let in_series = |i: &Inst| i.series == series;
    let mut keep: Vec<Inst> = Vec::new();
    for i in all {
        let wanted = match i.kind {
            Kind::Volume | Kind::Thumbnail => in_series(&i),
            Kind::Label | Kind::Overview => in_series(&i) || same_slide(&i),
        };
        if wanted {
            keep.push(i);
        }
    }
    // Instance order: instance number, then file order (concatenations follow their offsets
    // within a level anyway).
    keep.sort_by_key(|i| (i.instance_number, i.file));
    let volumes: Vec<usize> = (0..keep.len())
        .filter(|&i| keep[i].kind == Kind::Volume)
        .collect();
    if volumes.is_empty() {
        return Err(Error::Malformed(
            "no VOLUME image in the whole slide series".into(),
        ));
    }
    // Levels: by matrix size, largest first.
    let mut sizes: Vec<(u32, u32)> = volumes
        .iter()
        .map(|&i| (keep[i].width, keep[i].height))
        .collect();
    sizes.sort_by_key(|&(w, h)| std::cmp::Reverse(u64::from(w) * u64::from(h)));
    sizes.dedup();
    let base = sizes[0];
    // The default plane: the first frame of the base level's first instance.
    let base_first = volumes
        .iter()
        .copied()
        .find(|&i| (keep[i].width, keep[i].height) == base)
        .ok_or(Error::NotSlide)?;
    // The default plane: the base's first optical path (in the Optical Path Sequence's
    // order) that has frames, and its first (lowest) focal plane.
    let b = &keep[base_first];
    let dpath = b
        .paths
        .iter()
        .find(|p| b.places.iter().any(|pl| &pl.path == *p))
        .cloned()
        .or_else(|| b.places.first().map(|p| p.path.clone()));
    let dz = b
        .places
        .iter()
        .filter(|pl| Some(&pl.path) == dpath.as_ref() && pl.col != u32::MAX)
        .map(|pl| pl.z)
        .min_by(f64::total_cmp);
    let base_spacing = b.spacing;
    let mut levels = Vec::new();
    let mut grids = Vec::new();
    for &(w, h) in &sizes {
        let members: Vec<usize> = volumes
            .iter()
            .copied()
            .filter(|&i| (keep[i].width, keep[i].height) == (w, h))
            .collect();
        let Some(g) = grid(&keep, &members, dpath.as_deref(), dz) else {
            continue;
        };
        // The level's downsample from its Pixel Spacing (Imaged Volume Width over its
        // matrix width), else from the matrix sizes.
        let by_size = f64::from(base.0) / f64::from(w.max(1));
        let downsample = match (base_spacing, keep[members[0]].spacing) {
            (Some((_, b)), Some((_, l))) if (l / b / by_size - 1.0).abs() < 0.05 => l / b,
            _ => by_size,
        };
        levels.push(Level {
            width: w,
            height: h,
            downsample,
        });
        grids.push(g);
    }
    if grids.is_empty() {
        return Err(Error::Malformed("no placeable tiles".into()));
    }
    // Associated images: one of each kind (the largest).
    let mut associated = Vec::new();
    let mut assoc = Vec::new();
    for (kind, ak) in [
        (Kind::Label, AssociatedKind::Label),
        (Kind::Overview, AssociatedKind::Macro),
        (Kind::Thumbnail, AssociatedKind::Thumbnail),
    ] {
        let best = (0..keep.len())
            .filter(|&i| keep[i].kind == kind)
            .max_by_key(|&i| u64::from(keep[i].width) * u64::from(keep[i].height));
        if let Some(i) = best {
            let members: Vec<usize> = (0..keep.len())
                .filter(|&j| {
                    keep[j].kind == kind
                        && (keep[j].width, keep[j].height) == (keep[i].width, keep[i].height)
                        && keep[j].series == keep[i].series
                })
                .collect();
            if let Some(g) = grid(&keep, &members, dpath.as_deref(), None) {
                associated.push((ak, g.width, g.height));
                assoc.push(g);
            }
        }
    }
    let props = props(&keep, base_first, members, &levels, grids.len());
    // Files: the members used besides the opened one, in member order.
    let mut used: Vec<usize> = keep.iter().map(|i| i.file).filter(|&f| f != 0).collect();
    used.sort_unstable();
    used.dedup();
    let remap: HashMap<usize, usize> = used.iter().enumerate().map(|(k, &f)| (f, k + 1)).collect();
    for i in &mut keep {
        i.file = if i.file == 0 { 0 } else { remap[&i.file] };
    }
    let files = used.iter().map(|&f| members[f].0.to_owned()).collect();
    Ok(Slide {
        props,
        width: base.0,
        height: base.1,
        levels,
        channels: vec![Channel::default()],
        size_z: 1,
        size_t: 1,
        associated,
        files,
        backend: crate::Backend::Dicom(DicomSlide {
            insts: keep,
            levels: grids,
            assoc,
            window: Mutex::new(None),
        }),
    })
}

fn props(
    insts: &[Inst],
    base: usize,
    members: &[(&str, &[u8])],
    levels: &[Level],
    _n: usize,
) -> Props {
    let inst = &insts[base];
    let file = members.get(inst.file).map_or(&[][..], |m| m.1);
    let v = inst.dicom.values(file);
    let ds = &inst.dicom.dataset;
    let mut p = Props::new(Vendor::Dicom);
    p.mpp = inst.spacing.map(|(r, c)| (c * 1000.0, r * 1000.0));
    p.objective = ds
        .get(OPTICAL_PATH_SEQUENCE)
        .and_then(|e| e.items.iter().find_map(|i| num(i, &v, OBJECTIVE_POWER, 0)))
        .filter(|&x| x > 0.0);
    let maker = text(ds, &v, 0x0008_0070);
    let model = text(ds, &v, 0x0008_1090);
    p.scanner = match (maker, model) {
        (Some(a), Some(b)) => Some(format!("{a} {b}")),
        (a, b) => a.or(b),
    };
    p.date = text(ds, &v, 0x0008_002A).or_else(|| {
        let d = text(ds, &v, 0x0008_0023).or_else(|| text(ds, &v, 0x0008_0022))?;
        Some(
            match text(ds, &v, 0x0008_0033).or_else(|| text(ds, &v, 0x0008_0032)) {
                Some(t) => format!("{d} {t}"),
                None => d,
            },
        )
    });
    p.name = text(ds, &v, 0x0008_103E).or_else(|| text(ds, &v, CONTAINER_ID));
    let ts = &inst.dicom.transfer_syntax;
    let colour = inst.dicom.pixel.as_ref().is_some_and(|i| i.samples >= 3);
    let bits = inst.dicom.pixel.as_ref().map_or(8, |i| i.bits_stored);
    p.sample = format!(
        "{bits}-bit {}, {}",
        if colour { "RGB" } else { "grey" },
        ts.name
    );
    let mut props = vec![
        ("Transfer Syntax".to_owned(), ts.name.clone()),
        (
            "Dimension Organization Type".to_owned(),
            if inst.tiled_full {
                "TILED_FULL"
            } else {
                "TILED_SPARSE"
            }
            .to_owned(),
        ),
        (
            "Tile Size".to_owned(),
            format!("{} × {}", inst.tile_w, inst.tile_h),
        ),
        ("Levels".to_owned(), levels.len().to_string()),
        ("Instances".to_owned(), insts.len().to_string()),
    ];
    // Optical paths in the Optical Path Sequence's order, then any others the frames name.
    let mut paths: Vec<&str> = inst
        .paths
        .iter()
        .filter(|p| inst.places.iter().any(|pl| &pl.path == *p))
        .map(String::as_str)
        .collect();
    let mut planes: Vec<f64> = Vec::new();
    for pl in &inst.places {
        if !paths.contains(&pl.path.as_str()) && !pl.path.is_empty() {
            paths.push(&pl.path);
        }
        if !planes.contains(&pl.z) {
            planes.push(pl.z);
        }
    }
    if planes.len() > 1 {
        props.push(("Focal Planes".to_owned(), planes.len().to_string()));
    }
    if paths.len() > 1 {
        props.push(("Optical Paths".to_owned(), paths.join(", ")));
    }
    if let Some(c) = text(ds, &v, CONTAINER_ID) {
        props.push(("Container Identifier".to_owned(), c));
    }
    if let Some(m) = p.objective {
        props.push(("Objective Lens Power".to_owned(), format!("{m}")));
    }
    p.properties = props;
    p
}

/// A decoded frame as RGBA8 (grey shown as grey, to 8 bits: through `window` when given, else
/// its full range).
fn frame_rgba(f: dcm::Frame, window: Option<(f64, f64)>) -> (u32, u32, Vec<u8>, bool) {
    let n = f.width as usize * f.height as usize;
    let mut out = vec![255u8; n * 4];
    match &f.pixels {
        Pixels::Rgb8(d) => {
            for (o, p) in out.chunks_exact_mut(4).zip(d.chunks_exact(3)) {
                o[..3].copy_from_slice(p);
            }
        }
        Pixels::Rgb16(d) => {
            let shift = f.bits.saturating_sub(8).min(8);
            for (o, p) in out.chunks_exact_mut(4).zip(d.chunks_exact(3)) {
                for c in 0..3 {
                    o[c] = (p[c] >> shift).min(255) as u8;
                }
            }
        }
        Pixels::Grey(g) => {
            let bits = f.bits.clamp(1, 32);
            let (lo, hi) = if f.signed {
                (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1)
            } else {
                (0, (1i64 << bits) - 1)
            };
            let (lo, hi) = window.unwrap_or((lo as f64, hi as f64));
            let span = (hi - lo).max(f64::MIN_POSITIVE);
            for (i, o) in out.chunks_exact_mut(4).enumerate() {
                let v = g.get(i).map_or(lo, |v| v as f64);
                let q = ((v - lo) / span * 255.0).round().clamp(0.0, 255.0) as u8;
                o[..3].fill(q);
            }
        }
    }
    (f.width, f.height, out, f.truncated)
}

impl DicomSlide {
    fn decode_tile(
        &self,
        files: Files,
        (inst, frame): (u32, u32),
        window: Option<(f64, f64)>,
        cancel: Option<&AtomicBool>,
    ) -> Result<(u32, u32, Vec<u8>, bool)> {
        let inst = &self.insts[inst as usize];
        let data = files.get(inst.file)?;
        let f = inst
            .dicom
            .decode_frame(data, frame, cancel)
            .map_err(dcm_err)?;
        Ok(frame_rgba(f, window))
    }

    /// The pyramid's grey display window (see [`DicomSlide::window`]).
    fn window(&self, files: Files, cancel: Option<&AtomicBool>) -> Result<Option<(f64, f64)>> {
        if let Some(w) = *self.window.lock() {
            return Ok(w);
        }
        let w = self.find_window(files, cancel)?;
        *self.window.lock() = Some(w);
        Ok(w)
    }

    fn find_window(&self, files: Files, cancel: Option<&AtomicBool>) -> Result<Option<(f64, f64)>> {
        let Some(&(bi, _)) = self.levels[0].tiles.iter().flatten().next() else {
            return Ok(None);
        };
        let base = &self.insts[bi as usize];
        match &base.dicom.pixel {
            Some(p) if p.samples == 1 && p.bits_stored > 8 && !p.float => {}
            _ => return Ok(None),
        }
        // The file's VOI window (its first, the default; PS3.3 C.11.2.1.2): the image's, else
        // the first frame's Frame VOI LUT (per-frame, else shared functional groups), as
        // multi-frame whole-slide files store it.
        let data = files.get(base.file)?;
        let v = base.dicom.values(data);
        if let Some(w) = voi_window(&base.dicom.dataset, &v) {
            return Ok(Some(w));
        }
        // The coarsest level's histogram, from up to WINDOW_TILES tiles spread over it.
        let g = self.levels.last().unwrap_or(&self.levels[0]);
        let stored: Vec<(u32, u32)> = g.tiles.iter().flatten().copied().collect();
        let step = stored.len().div_ceil(WINDOW_TILES).max(1);
        let mut values = Vec::new();
        for &(inst, frame) in stored.iter().step_by(step) {
            crate::check_cancel(cancel)?;
            let inst = &self.insts[inst as usize];
            let Ok(f) = inst
                .dicom
                .decode_frame(files.get(inst.file)?, frame, cancel)
            else {
                continue;
            };
            if let Pixels::Grey(px) = &f.pixels {
                let n = f.width as usize * f.height as usize;
                values.extend((0..n).filter_map(|i| px.get(i)).map(|v| v as f32));
            }
        }
        if values.is_empty() {
            return Ok(None);
        }
        Ok(Some(crate::composite::auto_window(&values)))
    }

    /// The level to read a 1/`scale` downscale from, with the remaining factor.
    fn choose(&self, scale: u32) -> (usize, u32) {
        let (w, h) = (self.levels[0].width, self.levels[0].height);
        let mut best = (0, scale);
        for (i, g) in self.levels.iter().enumerate().skip(1) {
            let mut f = 2u64;
            while f <= u64::from(scale) {
                let (vw, vh) = (u64::from(w).div_ceil(f), u64::from(h).div_ceil(f));
                let (lw, lh) = (u64::from(g.width), u64::from(g.height));
                // Levels are sized floor or ceiling of the division.
                if lw.abs_diff(vw) <= 1.max(vw / 1000)
                    && lh.abs_diff(vh) <= 1.max(vh / 1000)
                    && scale / f as u32 <= best.1
                {
                    best = (i, scale / f as u32);
                }
                f *= 2;
            }
        }
        best
    }

    /// Region `lr` of grid `g` (in its pixels), reduced by `f`, snapped to `f`'s grid: the
    /// reduced raster and its origin in reduced pixels.
    fn read_grid(
        &self,
        g: &Grid,
        files: Files,
        lr: Region,
        f: u32,
        window: Option<(f64, f64)>,
        o: &ReadOptions,
    ) -> Result<(Rgba, u32, u32)> {
        let x0 = lr.x / f * f;
        let y0 = lr.y / f * f;
        let x1 = (lr.x + lr.w).div_ceil(f).saturating_mul(f).min(g.width);
        let y1 = (lr.y + lr.h).div_ceil(f).saturating_mul(f).min(g.height);
        let (pw, ph) = ((x1 - x0) as usize, (y1 - y0) as usize);
        if pw as u64 * ph as u64 > crate::MAX_READ_PIXELS {
            return Err(Error::TooLarge);
        }
        let jobs: Vec<(u32, u32, (u32, u32))> = (y0 / g.tile_h..y1.div_ceil(g.tile_h))
            .flat_map(|ty| (x0 / g.tile_w..x1.div_ceil(g.tile_w)).map(move |tx| (tx, ty)))
            .filter_map(|(tx, ty)| {
                g.tiles
                    .get(ty as usize * g.cols as usize + tx as usize)
                    .copied()
                    .flatten()
                    .map(|t| (tx, ty, t))
            })
            .collect();
        let load = |&(tx, ty, t): &(u32, u32, (u32, u32))| -> Result<_> {
            crate::check_cancel(o.cancel)?;
            match self.decode_tile(files, t, window, o.cancel) {
                Ok(d) => Ok((tx, ty, Some(d))),
                Err(e @ (Error::Cancelled | Error::TooLarge | Error::Io(_))) => Err(e),
                Err(_) => Ok((tx, ty, None)),
            }
        };
        let tiles: Vec<_> = if o.parallel {
            jobs.par_iter().map(load).collect::<Result<_>>()?
        } else {
            jobs.iter().map(load).collect::<Result<_>>()?
        };
        // White where no tile is stored (sparse tiling leaves out empty glass).
        let mut buf = vec![255u8; pw * ph * 4];
        let mut truncated = false;
        for (tx, ty, d) in tiles {
            let Some((w, h, px, trunc)) = d else {
                truncated = true;
                continue;
            };
            truncated |= trunc;
            let (gx0, gy0) = (tx * g.tile_w, ty * g.tile_h);
            let xs = gx0.max(x0)..(gx0 + w.min(g.tile_w)).min(x1);
            if xs.is_empty() {
                continue;
            }
            for gy in gy0.max(y0)..(gy0 + h.min(g.tile_h)).min(y1) {
                let src = ((gy - gy0) as usize * w as usize + (xs.start - gx0) as usize) * 4;
                let dst = ((gy - y0) as usize * pw + (xs.start - x0) as usize) * 4;
                let n = xs.len() * 4;
                buf[dst..dst + n].copy_from_slice(&px[src..src + n]);
            }
        }
        let img = Rgba {
            width: pw as u32,
            height: ph as u32,
            data: buf,
            truncated,
        };
        Ok((box_reduce_rgba(&img, f), x0 / f, y0 / f))
    }

    pub(crate) fn read(&self, files: Files, o: &ReadOptions) -> Result<Rgba> {
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let (li, f) = self.choose(o.scale.max(1));
        let g = &self.levels[li];
        let lr = Region::clip(
            Some(Region {
                x: r.x.saturating_mul(f),
                y: r.y.saturating_mul(f),
                w: r.w.saturating_mul(f),
                h: r.h.saturating_mul(f),
            }),
            g.width,
            g.height,
        );
        let mut out = vec![255u8; r.w as usize * r.h as usize * 4];
        let Some(lr) = lr else {
            // A level a pixel smaller than the base: the region's edge lies past it.
            return Ok(Rgba {
                width: r.w,
                height: r.h,
                data: out,
                truncated: false,
            });
        };
        let window = self.window(files, o.cancel)?;
        let (img, ox, oy) = self.read_grid(g, files, lr, f, window, o)?;
        // Crop to the region (the snap extends up and left by under one output pixel; a level
        // a pixel short leaves white at the far edges).
        for y in 0..r.h {
            let Some(sy) = (y + r.y).checked_sub(oy).filter(|&s| s < img.height) else {
                continue;
            };
            let Some(sx0) = r.x.checked_sub(ox) else {
                continue;
            };
            let n = img.width.saturating_sub(sx0).min(r.w) as usize;
            let s = (sy as usize * img.width as usize + sx0 as usize) * 4;
            out[y as usize * r.w as usize * 4..][..n * 4].copy_from_slice(&img.data[s..s + n * 4]);
        }
        Ok(Rgba {
            width: r.w,
            height: r.h,
            data: out,
            truncated: img.truncated,
        })
    }

    pub(crate) fn read_associated(
        &self,
        files: Files,
        i: usize,
        cancel: Option<&AtomicBool>,
    ) -> Result<Rgba> {
        let g = self
            .assoc
            .get(i)
            .ok_or_else(|| Error::Malformed("no such associated image".into()))?;
        let mut f = 1;
        while g.width / f > ASSOCIATED_MAX || g.height / f > ASSOCIATED_MAX {
            f *= 2;
        }
        let o = ReadOptions {
            scale: f,
            region: None,
            position: Default::default(),
            cancel,
            parallel: true,
        };
        let all = Region {
            x: 0,
            y: 0,
            w: g.width,
            h: g.height,
        };
        Ok(self.read_grid(g, files, all, f, None, &o)?.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey16(v: Vec<u16>) -> dcm::Frame {
        dcm::Frame {
            width: v.len() as u32,
            height: 1,
            pixels: Pixels::Grey(dcm::Grey::U16(v)),
            bits: 16,
            signed: false,
            truncated: false,
        }
    }

    #[test]
    fn grey_over_eight_bits_through_the_window() {
        // Dim 16-bit fluorescence: black over its full range, visible through its window.
        let (_, _, full, _) = frame_rgba(grey16(vec![0, 5, 10, 20]), None);
        assert_eq!([full[0], full[4], full[8], full[12]], [0, 0, 0, 0]);
        let (_, _, win, _) = frame_rgba(grey16(vec![0, 5, 10, 20]), Some((0.0, 10.0)));
        assert_eq!([win[0], win[4], win[8], win[12]], [0, 128, 255, 255]);
        assert_eq!(&win[4..8], &[128, 128, 128, 255]);
    }

    mod hostile {
        use super::*;

        /// An explicit VR little endian element.
        fn el(out: &mut Vec<u8>, tag: u32, vr: &[u8; 2], val: &[u8]) {
            let mut val = val.to_vec();
            if val.len() % 2 == 1 {
                val.push(if vr == b"UI" || vr == b"OB" { 0 } else { b' ' });
            }
            out.extend_from_slice(&((tag >> 16) as u16).to_le_bytes());
            out.extend_from_slice(&(tag as u16).to_le_bytes());
            out.extend_from_slice(vr);
            if matches!(vr, b"OB" | b"SQ") {
                out.extend_from_slice(&[0, 0]);
                out.extend_from_slice(&(val.len() as u32).to_le_bytes());
            } else {
                out.extend_from_slice(&(val.len() as u16).to_le_bytes());
            }
            out.extend_from_slice(&val);
        }

        /// A sequence of one item holding `body`.
        fn seq(out: &mut Vec<u8>, tag: u32, body: &[u8]) {
            let mut item = vec![0xFE, 0xFF, 0x00, 0xE0];
            item.extend_from_slice(&(body.len() as u32).to_le_bytes());
            item.extend_from_slice(body);
            el(out, tag, b"SQ", &item);
        }

        /// A 16 x 16 grey WSI instance; `extra` are (tag, IS value) pairs, `per_frame` the
        /// first per-frame functional group.
        fn wsi(extra: &[(u32, &str)], per_frame: Option<&[u8]>) -> Vec<u8> {
            let mut d = vec![0u8; 128];
            d.extend_from_slice(b"DICM");
            el(&mut d, 0x0002_0010, b"UI", b"1.2.840.10008.1.2.1");
            el(&mut d, SOP_CLASS, b"UI", WSI_SOP_CLASS.as_bytes());
            let mut els: Vec<(u32, Vec<u8>)> = extra
                .iter()
                .map(|&(t, s)| {
                    let mut e = Vec::new();
                    el(&mut e, t, b"IS", s.as_bytes());
                    (t, e)
                })
                .collect();
            for (t, vr, v) in [
                (0x0028_0002, b"US", 1u16),
                (0x0028_0010, b"US", 16),
                (0x0028_0011, b"US", 16),
                (0x0028_0100, b"US", 8),
                (0x0028_0101, b"US", 8),
                (0x0028_0102, b"US", 7),
                (0x0028_0103, b"US", 0),
            ] {
                let mut e = Vec::new();
                el(&mut e, t, vr, &v.to_le_bytes());
                els.push((t, e));
            }
            let mut e = Vec::new();
            el(&mut e, 0x0028_0004, b"CS", b"MONOCHROME2");
            els.push((0x0028_0004, e));
            if let Some(p) = per_frame {
                let mut e = Vec::new();
                seq(&mut e, PER_FRAME_GROUPS, p);
                els.push((PER_FRAME_GROUPS, e));
            }
            let mut e = Vec::new();
            el(&mut e, 0x7FE0_0010, b"OB", &[128; 256]);
            els.push((0x7FE0_0010, e));
            els.sort_by_key(|(t, _)| *t);
            for (_, e) in els {
                d.extend_from_slice(&e);
            }
            d
        }

        /// A grey pyramid's window from the first frame's Frame VOI LUT when the image has
        /// none of its own (multi-frame files keep it in the functional groups).
        #[test]
        fn window_from_the_frame_voi_lut() {
            let mut voi = Vec::new();
            el(&mut voi, WINDOW_CENTER, b"DS", b"1000\\2000");
            el(&mut voi, WINDOW_WIDTH, b"DS", b"401\\10");
            let mut group = Vec::new();
            seq(&mut group, 0x0028_9132, &voi);
            let d = wsi(&[(0x0028_0008, "1")], Some(&group));
            let inst = parse_inst(0, &d).unwrap();
            let v = inst.dicom.values(&d);
            assert_eq!(voi_window(&inst.dicom.dataset, &v), Some((799.5, 1199.5)));
            // The image's own window wins; none at all: none.
            let d = wsi(&[(WINDOW_CENTER, "50"), (WINDOW_WIDTH, "21")], Some(&group));
            let inst = parse_inst(0, &d).unwrap();
            let v = inst.dicom.values(&d);
            assert_eq!(voi_window(&inst.dicom.dataset, &v), Some((39.5, 59.5)));
            let d = wsi(&[(0x0028_0008, "1")], None);
            let inst = parse_inst(0, &d).unwrap();
            assert_eq!(
                voi_window(&inst.dicom.dataset, &inst.dicom.values(&d)),
                None
            );
        }

        #[test]
        fn frame_count_beyond_the_file() {
            // A billion frames in a 700-byte file once reserved and filled a billion places.
            let d = wsi(&[(0x0028_0008, "999999999")], None);
            let inst = parse_inst(0, &d).unwrap();
            assert!(inst.places.len() <= d.len());
        }

        #[test]
        fn focal_planes_overflow() {
            // 2^20 tiles a plane times 10^18 planes once overflowed the frame arithmetic.
            let d = wsi(
                &[
                    (0x0028_0008, "3"),
                    (TPM_COLUMNS, "16777216"),
                    (TPM_FOCAL_PLANES, "1000000000000000000"),
                    (CONCATENATION_OFFSET, "1e30"),
                ],
                None,
            );
            let inst = parse_inst(0, &d).unwrap();
            assert_eq!(inst.places.len(), 3);
        }

        #[test]
        fn sparse_position_at_i64_min() {
            // A DS-style position of -1e300 saturates to i64::MIN; one less once overflowed.
            let mut pos = Vec::new();
            el(&mut pos, COLUMN_POSITION, b"IS", b"-1e300");
            el(&mut pos, ROW_POSITION, b"IS", b"-1e300");
            let mut group = Vec::new();
            seq(&mut group, PLANE_POSITION_SLIDE, &pos);
            let d = wsi(&[(0x0028_0008, "1")], Some(&group));
            let inst = parse_inst(0, &d).unwrap();
            assert!(!inst.tiled_full);
            assert_eq!((inst.places[0].col, inst.places[0].row), (0, 0));
        }
    }
}
