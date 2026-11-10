//! 3DHISTECH MIRAX (`.mrxs`): the `.mrxs` file (a JPEG preview) beside a folder of the same
//! name holding `Slidedat.ini`, `Index.dat` and `Data*.dat`.
//!
//! - `Slidedat.ini` names the zoom levels (the `Slide zoom level` hierarchy, each a section
//!   with the camera image size, overlap, concatenation factor, image format and fill colour),
//!   the data files, and the non-hierarchical records (camera positions, slide images).
//! - `Index.dat` starts with the version `01.02` and the slide id, then the roots of the
//!   hierarchical records (one list of data pages per zoom level: image number, offset,
//!   length, data file) and the non-hierarchical ones (one data page each).
//! - The camera takes a photograph per position and splits it into
//!   `CameraImageDivisionsPerSide²` images; photographs overlap, their images do not. Each
//!   zoom level halves and concatenates 2×2 images of the one before, so once images of
//!   several photographs share one image it is cut into parts placed by their own
//!   photograph's position. Positions come from `VIMSLIDE_POSITION_BUFFER` (9-byte records:
//!   flag, x, y) or the zlib-compressed `StitchingIntensityLayer`, else from the nominal
//!   overlap. Where images overlap the lower-right one shows (as OpenSlide paints).
//! - The slide images are JPEGs: `ScanDataLayer_SlideThumbnail` (the macro photograph),
//!   `ScanDataLayer_SlideBarcode` (the label) and `ScanDataLayer_SlidePreview` (thumbnail).
//!
//! Fluorescence slides keep their filters (`Slide filter level`) as properties; the images are
//! shown as stored, as OpenSlide does.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;

use crate::mosaic::{Mosaic, MosaicLevel, Placed, TileSource};
use crate::stitched::{Assoc, Stitched};
use crate::{
    AssociatedKind, Channel, Error, Files, Level, Position, Props, Result, Rgba, Slide, Vendor,
};

/// Largest `Slidedat.ini` read.
pub const SLIDEDAT_MAX: usize = 1 << 20;
/// Most data files of a slide.
const MAX_FILES: usize = 1 << 16;
/// Most images of a zoom level.
const MAX_IMAGES: usize = 1 << 24;
/// Most camera positions.
const MAX_POSITIONS: u64 = 1 << 24;

/// The index of the first data file among a slide's [`Files`] (after the `.mrxs`,
/// `Slidedat.ini` and `Index.dat`).
const FIRST_DATA: usize = 3;

/// A parsed `Slidedat.ini`: groups of key/value pairs.
#[derive(Clone, Debug, Default)]
pub struct Ini {
    groups: HashMap<String, HashMap<String, String>>,
    order: Vec<(String, String, String)>,
}

impl Ini {
    pub fn parse(text: &str) -> Ini {
        let mut ini = Ini::default();
        let mut group = String::new();
        for line in text.trim_start_matches('\u{feff}').lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(g) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                group = g.trim().to_owned();
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let (k, v) = (k.trim().to_owned(), v.trim().to_owned());
                ini.order.push((group.clone(), k.clone(), v.clone()));
                ini.groups.entry(group.clone()).or_default().insert(k, v);
            }
        }
        ini
    }

    pub fn get(&self, group: &str, key: &str) -> Option<&str> {
        self.groups.get(group)?.get(key).map(String::as_str)
    }

    fn int(&self, group: &str, key: &str) -> Option<i64> {
        self.get(group, key)?.parse().ok()
    }

    fn num(&self, group: &str, key: &str) -> Option<f64> {
        self.get(group, key)?
            .replace(',', ".")
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
    }

    fn need_int(&self, group: &str, key: &str) -> Result<i64> {
        self.int(group, key)
            .ok_or_else(|| Error::Malformed(format!("Slidedat.ini without {group}/{key}")))
    }
}

/// The data files `Slidedat.ini` lists, in order (plain names in the slide's folder).
pub fn data_files(slidedat: &[u8]) -> Result<Vec<String>> {
    let ini = Ini::parse(&String::from_utf8_lossy(slidedat));
    let n = ini.need_int("DATAFILE", "FILE_COUNT")?;
    if n <= 0 || n as usize > MAX_FILES {
        return Err(Error::Malformed("MIRAX data file count".into()));
    }
    (0..n)
        .map(|i| {
            ini.get("DATAFILE", &format!("FILE_{i}"))
                .map(str::to_owned)
                .ok_or_else(|| Error::Malformed(format!("MIRAX without FILE_{i}")))
        })
        .collect()
}

/// The index file `Slidedat.ini` names.
pub fn index_file(slidedat: &[u8]) -> Option<String> {
    let ini = Ini::parse(&String::from_utf8_lossy(slidedat));
    ini.get("HIERARCHICAL", "INDEXFILE").map(str::to_owned)
}

/// A zoom level's section.
#[derive(Clone, Debug)]
struct ZoomSection {
    concat_exponent: u32,
    overlap: (f64, f64),
    mpp: (f64, f64),
    fill: [u8; 3],
    image_w: u32,
    image_h: u32,
}

/// How a zoom level cuts its images (OpenSlide's names).
#[derive(Clone, Copy, Debug)]
struct LevelParams {
    image_concat: u32,
    tile_count_divisor: u32,
    tiles_per_image: u32,
    positions_per_tile: u32,
}

fn i32_at(b: &[u8], o: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        b.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// A stored image: data file, offset, length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Image {
    file: u32,
    offset: u32,
    len: u32,
}

/// The data pages of a list starting at `ptr` (a zero, then the first page's pointer): every
/// entry (image number, offset, length, file).
fn pages(index: &[u8], ptr: i32) -> Result<Vec<[i32; 4]>> {
    let bad = |m: &str| Error::Malformed(format!("MIRAX Index.dat: {m}"));
    let p = usize::try_from(ptr).map_err(|_| bad("negative pointer"))?;
    if i32_at(index, p) != Some(0) {
        return Err(bad("data page list without its leading zero"));
    }
    let mut q = i32_at(index, p + 4).ok_or_else(|| bad("truncated"))?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    loop {
        let at = usize::try_from(q).map_err(|_| bad("negative page pointer"))?;
        if !seen.insert(at) {
            return Err(bad("data pages loop"));
        }
        let n = i32_at(index, at).ok_or_else(|| bad("truncated page"))?;
        let next = i32_at(index, at + 4).ok_or_else(|| bad("truncated page"))?;
        let n = usize::try_from(n).map_err(|_| bad("negative entry count"))?;
        if out.len() + n > MAX_IMAGES {
            return Err(Error::TooLarge);
        }
        for k in 0..n {
            let o = at + 8 + 16 * k;
            let e = [0, 1, 2, 3].map(|j| i32_at(index, o + 4 * j));
            let [Some(a), Some(b), Some(c), Some(d)] = e else {
                return Err(bad("truncated page entries"));
            };
            out.push([a, b, c, d]);
        }
        if next == 0 {
            break;
        }
        q = next;
    }
    Ok(out)
}

/// Non-hierarchical record `recordno`: (offset, length, file).
fn nonhier_record(
    index: &[u8],
    root: usize,
    recordno: usize,
    files: usize,
) -> Result<(usize, usize, usize)> {
    let bad = |m: &str| Error::Malformed(format!("MIRAX non-hierarchical record {recordno}: {m}"));
    let list = i32_at(index, root).ok_or_else(|| bad("no root"))?;
    let list = usize::try_from(list).map_err(|_| bad("negative pointer"))?;
    let ptr = i32_at(index, list + 4 * recordno).ok_or_else(|| bad("truncated"))?;
    let p = usize::try_from(ptr).map_err(|_| bad("negative pointer"))?;
    if i32_at(index, p) != Some(0) {
        return Err(bad("no leading zero"));
    }
    let page = i32_at(index, p + 4).ok_or_else(|| bad("truncated"))?;
    let page = usize::try_from(page).map_err(|_| bad("negative page"))?;
    if i32_at(index, page).is_none_or(|n| n < 1) {
        return Err(bad("empty"));
    }
    let v: Vec<Option<i32>> = (0..6).map(|k| i32_at(index, page + 4 + 4 * k)).collect();
    // Next page (ignored), two zeros, then offset, size, file.
    let (Some(offset), Some(size), Some(file)) = (v[3], v[4], v[5]) else {
        return Err(bad("truncated"));
    };
    let (Ok(offset), Ok(size), Ok(file)) = (
        usize::try_from(offset),
        usize::try_from(size),
        usize::try_from(file),
    ) else {
        return Err(bad("negative values"));
    };
    if file >= files {
        return Err(bad("file number out of range"));
    }
    Ok((offset, size, file))
}

/// The record number of the first value of non-hierarchical layer `name`, and the layer's
/// index (records count every value of the layers before).
fn nonhier_name(ini: &Ini, name: &str) -> Option<(usize, usize)> {
    let n = ini.int("HIERARCHICAL", "NONHIER_COUNT")?;
    let mut offset = 0usize;
    for i in 0..n.clamp(0, 4096) {
        let v = ini.get("HIERARCHICAL", &format!("NONHIER_{i}_NAME"))?;
        let count =
            usize::try_from(ini.int("HIERARCHICAL", &format!("NONHIER_{i}_COUNT"))?).ok()?;
        if v == name {
            return Some((offset, i as usize));
        }
        offset += count;
    }
    None
}

/// The record number of value `value` of non-hierarchical layer `name`, and its section.
fn nonhier_value(ini: &Ini, name: &str, value: &str) -> Option<(usize, Option<String>)> {
    let (offset, layer) = nonhier_name(ini, name)?;
    let count = ini.int("HIERARCHICAL", &format!("NONHIER_{layer}_COUNT"))?;
    for k in 0..count.clamp(0, 4096) {
        if ini.get("HIERARCHICAL", &format!("NONHIER_{layer}_VAL_{k}")) == Some(value) {
            let section = ini
                .get("HIERARCHICAL", &format!("NONHIER_{layer}_VAL_{k}_SECTION"))
                .map(str::to_owned);
            return Some((offset + k as usize, section));
        }
    }
    None
}

struct MiraxTiles {
    /// Images by zoom level and number.
    images: Vec<Vec<Image>>,
}

impl TileSource for MiraxTiles {
    fn rgba(
        &self,
        files: Files,
        level: usize,
        src: u32,
        _pos: Position,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Rgba>> {
        let Some(im) = self.images.get(level).and_then(|v| v.get(src as usize)) else {
            return Ok(None);
        };
        let data = files.get(FIRST_DATA + im.file as usize)?;
        let b = data
            .get(im.offset as usize..(im.offset as usize).saturating_add(im.len as usize))
            .ok_or_else(|| Error::Truncated("MIRAX image past the end of its data file".into()))?;
        crate::blob::rgba(b, cancel).map(Some)
    }
}

/// Open a MIRAX slide. `files` holds the `.mrxs` file, `Slidedat.ini`, `Index.dat` and the
/// data files [`data_files`] lists, in that order.
pub fn open(files: Files) -> Result<Slide> {
    let slidedat = files.get(1)?;
    if slidedat.len() > SLIDEDAT_MAX {
        return Err(Error::Malformed("Slidedat.ini too large".into()));
    }
    let ini = Ini::parse(&String::from_utf8_lossy(slidedat));
    let index = files.get(2)?;
    let bad = |m: &str| Error::Malformed(format!("MIRAX: {m}"));
    let slide_id = ini
        .get("GENERAL", "SLIDE_ID")
        .ok_or_else(|| bad("no SLIDE_ID"))?
        .to_owned();
    let images_x = ini.need_int("GENERAL", "IMAGENUMBER_X")?;
    let images_y = ini.need_int("GENERAL", "IMAGENUMBER_Y")?;
    let divisions = ini
        .int("GENERAL", "CameraImageDivisionsPerSide")
        .unwrap_or(1);
    if images_x <= 0
        || images_y <= 0
        || !(1..=64).contains(&divisions)
        || images_x * images_y > MAX_IMAGES as i64
    {
        return Err(bad("image counts"));
    }
    let (images_x, images_y, divisions) = (images_x as u32, images_y as u32, divisions as u32);
    let hier_count = ini.need_int("HIERARCHICAL", "HIER_COUNT")?;
    let zoom = (0..hier_count.clamp(0, 64))
        .position(|i| {
            ini.get("HIERARCHICAL", &format!("HIER_{i}_NAME")) == Some("Slide zoom level")
        })
        .ok_or_else(|| bad("no slide zoom level"))?;
    if zoom != 0 {
        return Err(Error::Unsupported(
            "MIRAX slide whose zoom levels are not the first hierarchy".into(),
        ));
    }
    let zoom_levels = ini.need_int("HIERARCHICAL", "HIER_0_COUNT")?;
    if !(1..=32).contains(&zoom_levels) {
        return Err(bad("zoom level count"));
    }
    let datafiles = data_files(slidedat)?;
    let mut sections = Vec::new();
    for i in 0..zoom_levels {
        let g = ini
            .get("HIERARCHICAL", &format!("HIER_0_VAL_{i}_SECTION"))
            .ok_or_else(|| bad("zoom level without its section"))?
            .to_owned();
        let concat = ini.need_int(&g, "IMAGE_CONCAT_FACTOR")?;
        let iw = ini.need_int(&g, "DIGITIZER_WIDTH")?;
        let ih = ini.need_int(&g, "DIGITIZER_HEIGHT")?;
        if !(0..=24).contains(&concat)
            || (i > 0 && concat == 0)
            || !(1..=1 << 16).contains(&iw)
            || !(1..=1 << 16).contains(&ih)
        {
            return Err(bad("zoom level section values"));
        }
        let bgr = ini.int(&g, "IMAGE_FILL_COLOR_BGR").unwrap_or(0xffffff) as u32;
        sections.push(ZoomSection {
            concat_exponent: concat as u32,
            overlap: (
                ini.num(&g, "OVERLAP_X").unwrap_or(0.0),
                ini.num(&g, "OVERLAP_Y").unwrap_or(0.0),
            ),
            mpp: (
                ini.num(&g, "MICROMETER_PER_PIXEL_X").unwrap_or(0.0),
                ini.num(&g, "MICROMETER_PER_PIXEL_Y").unwrap_or(0.0),
            ),
            fill: [bgr as u8, (bgr >> 8) as u8, (bgr >> 16) as u8],
            image_w: iw as u32,
            image_h: ih as u32,
        });
        if let Some(f) = ini.get(&g, "IMAGE_FORMAT")
            && !matches!(f, "JPEG" | "PNG" | "BMP24")
        {
            return Err(Error::Unsupported(format!("MIRAX images in {f}")));
        }
    }
    // Index.dat: version, slide id, roots.
    if !index.starts_with(b"01.02") {
        return Err(bad("Index.dat has another version"));
    }
    if index.get(5..5 + slide_id.len()) != Some(slide_id.as_bytes()) {
        return Err(bad("Index.dat belongs to another slide"));
    }
    let hier_root = 5 + slide_id.len();
    let nonhier_root = hier_root + 4;
    let vim = nonhier_name(&ini, "VIMSLIDE_POSITION_BUFFER").map(|v| (v.0, false));
    let stitch = nonhier_name(&ini, "StitchingIntensityLayer").map(|v| (v.0, true));
    let s0 = &sections[0];
    // Level parameters.
    let positions_x = images_x / divisions;
    let positions_y = images_y / divisions;
    let npositions = u64::from(positions_x) * u64::from(positions_y);
    if npositions == 0 || npositions > MAX_POSITIONS {
        return Err(bad("position count"));
    }
    let has_positions = vim.or(stitch).is_some() || s0.overlap != (0.0, 0.0);
    let mut params = Vec::new();
    let mut total = 0u32;
    for s in &sections {
        total += s.concat_exponent;
        if total > 24 {
            return Err(bad("concatenation factor"));
        }
        let image_concat = 1u32 << total;
        let positions_per_image = (image_concat / divisions).max(1);
        params.push(if has_positions {
            LevelParams {
                image_concat,
                tile_count_divisor: image_concat.min(divisions),
                tiles_per_image: positions_per_image,
                positions_per_tile: 1,
            }
        } else {
            LevelParams {
                image_concat,
                tile_count_divisor: image_concat,
                tiles_per_image: 1,
                positions_per_tile: positions_per_image,
            }
        });
    }
    // Camera positions (level-0 pixels of the uncut photographs).
    let concat0 = params[0].image_concat as i64;
    let positions: Vec<(i64, i64)> = match vim.or(stitch) {
        Some((rec, compressed)) => {
            let (off, size, file) = nonhier_record(index, nonhier_root, rec, datafiles.len())?;
            let data = files.get(FIRST_DATA + file)?;
            let raw = data
                .get(off..off.saturating_add(size))
                .ok_or_else(|| bad("position record past its data file"))?;
            let want = 9 * npositions as usize;
            let buf = if compressed {
                specular_png::inflate_zlib(raw, want)
                    .map_err(|e| bad(&format!("position buffer: {e}")))?
            } else {
                raw.to_vec()
            };
            if buf.len() < want {
                return Err(bad("position buffer too short"));
            }
            buf.chunks_exact(9)
                .take(npositions as usize)
                .map(|r| {
                    let x = i32::from_le_bytes([r[1], r[2], r[3], r[4]]);
                    let y = i32::from_le_bytes([r[5], r[6], r[7], r[8]]);
                    (i64::from(x) * concat0, i64::from(y) * concat0)
                })
                .collect()
        }
        None => (0..npositions)
            .map(|i| {
                let (px, py) = (
                    (i % u64::from(positions_x)) as i64,
                    (i / u64::from(positions_x)) as i64,
                );
                (
                    (px as f64 * (f64::from(s0.image_w) * f64::from(divisions) - s0.overlap.0))
                        as i64,
                    (py as f64 * (f64::from(s0.image_h) * f64::from(divisions) - s0.overlap.1))
                        as i64,
                )
            })
            .collect(),
    };
    // Full size: images edge to edge, minus the overlap at each photograph's edge (summed in
    // whole pixels, each step truncated, as the format's readers count it).
    let side = |n: u32, size: u32, overlap: f64| -> u64 {
        let mut acc = 0i64;
        for i in 0..n {
            let step = if i % divisions != divisions - 1 || i == n - 1 {
                f64::from(size)
            } else {
                f64::from(size) - overlap
            };
            acc = (acc as f64 + step) as i64;
        }
        acc.max(0) as u64
    };
    let base_w = side(images_x, s0.image_w, s0.overlap.0);
    let base_h = side(images_y, s0.image_h, s0.overlap.1);
    // Hierarchical records.
    let hier = i32_at(index, hier_root).ok_or_else(|| bad("Index.dat truncated"))?;
    let hier = usize::try_from(hier).map_err(|_| bad("negative hierarchy pointer"))?;
    let mut active: HashSet<u64> = HashSet::new();
    let mut levels = Vec::new();
    let mut slide_levels = Vec::new();
    let mut images = Vec::new();
    for (z, (lp, s)) in params.iter().zip(&sections).enumerate() {
        let list = i32_at(index, hier + 4 * z).ok_or_else(|| bad("zoom level pointer"))?;
        let entries = pages(index, list)?;
        // Parts of an image may be a fraction of a pixel wide: each part's edges are rounded.
        let tile_w = f64::from(s.image_w) / f64::from(lp.tiles_per_image);
        let tile_h = f64::from(s.image_h) / f64::from(lp.tiles_per_image);
        let edge = |t: f64, i: u32| (t * f64::from(i)).round() as u32;
        let mut level_images = Vec::with_capacity(entries.len());
        let mut grid: HashMap<(i64, i64), Placed> = HashMap::new();
        for [image_index, offset, length, fileno] in entries {
            if image_index < 0
                || offset < 0
                || length < 0
                || fileno < 0
                || fileno as usize >= datafiles.len()
            {
                return Err(bad("negative or out-of-range image entry"));
            }
            let (x, y) = (image_index as u32 % images_x, image_index as u32 / images_x);
            if y >= images_y || x % lp.image_concat != 0 || y % lp.image_concat != 0 {
                continue;
            }
            let src = level_images.len() as u32;
            level_images.push(Image {
                file: fileno as u32,
                offset: offset as u32,
                len: length as u32,
            });
            for yi in 0..lp.tiles_per_image {
                let yy = y + yi * divisions;
                if yy >= images_y {
                    break;
                }
                for xi in 0..lp.tiles_per_image {
                    let xx = x + xi * divisions;
                    if xx >= images_x {
                        break;
                    }
                    let (xp, yp) = (xx / divisions, yy / divisions);
                    let cp = u64::from(yp) * u64::from(positions_x) + u64::from(xp);
                    let Some(&(px, py)) = positions.get(cp as usize) else {
                        continue;
                    };
                    if z == 0 {
                        if px == 0 && py == 0 && (xp != 0 || yp != 0) {
                            continue;
                        }
                        active.insert(cp);
                    } else {
                        let mut any = false;
                        'p: for ypp in yp..yp + lp.positions_per_tile {
                            for xpp in xp..xp + lp.positions_per_tile {
                                if active.contains(
                                    &(u64::from(ypp) * u64::from(positions_x) + u64::from(xpp)),
                                ) {
                                    any = true;
                                    break 'p;
                                }
                            }
                        }
                        if !any {
                            continue;
                        }
                    }
                    let pos0_x =
                        px + i64::from(sections[0].image_w) * i64::from(xx - xp * divisions);
                    let pos0_y =
                        py + i64::from(sections[0].image_h) * i64::from(yy - yp * divisions);
                    let c = i64::from(lp.image_concat);
                    let key = (
                        i64::from(x / lp.tile_count_divisor + xi),
                        i64::from(y / lp.tile_count_divisor + yi),
                    );
                    grid.insert(
                        key,
                        Placed {
                            x: (pos0_x as f64 / c as f64).round() as i64,
                            y: (pos0_y as f64 / c as f64).round() as i64,
                            w: edge(tile_w, xi + 1) - edge(tile_w, xi),
                            h: edge(tile_h, yi + 1) - edge(tile_h, yi),
                            src,
                            sx: edge(tile_w, xi),
                            sy: edge(tile_h, yi),
                        },
                    );
                }
            }
        }
        // Lower-right tiles last: they show where images overlap (OpenSlide paints the grid
        // from the bottom right with a saturating operator, so the first painted stays).
        let mut placed: Vec<((i64, i64), Placed)> = grid.into_iter().collect();
        placed.sort_by_key(|a| (a.0.1, a.0.0));
        let factor = lp.image_concat / params[0].image_concat;
        let (w, h) = (
            u32::try_from(base_w / u64::from(lp.image_concat)).map_err(|_| Error::TooLarge)?,
            u32::try_from(base_h / u64::from(lp.image_concat)).map_err(|_| Error::TooLarge)?,
        );
        if w == 0 || h == 0 {
            break;
        }
        levels.push(MosaicLevel::new(
            w,
            h,
            factor,
            placed.into_iter().map(|p| p.1).collect(),
        ));
        slide_levels.push(Level {
            width: w,
            height: h,
            downsample: f64::from(factor),
        });
        images.push(level_images);
    }
    if levels.is_empty() {
        return Err(bad("no zoom levels"));
    }
    let (width, height) = (levels[0].width, levels[0].height);
    // Slide images.
    let mut assoc = Vec::new();
    let mut assoc_sizes = Vec::new();
    for (value, kind) in [
        ("ScanDataLayer_SlideThumbnail", AssociatedKind::Macro),
        ("ScanDataLayer_SlideBarcode", AssociatedKind::Label),
        ("ScanDataLayer_SlidePreview", AssociatedKind::Thumbnail),
    ] {
        let Some((rec, _)) = nonhier_value(&ini, "Scan data layer", value) else {
            continue;
        };
        let Ok((off, size, file)) = nonhier_record(index, nonhier_root, rec, datafiles.len())
        else {
            continue;
        };
        let Some(b) = files
            .get(FIRST_DATA + file)
            .ok()
            .and_then(|d| d.get(off..off.saturating_add(size)))
        else {
            continue;
        };
        let Some((w, h)) = specular_jpeg::read_info(b)
            .ok()
            .map(|i| (i.width, i.height))
        else {
            continue;
        };
        assoc.push(Assoc::Blob {
            file: FIRST_DATA + file,
            offset: off as u64,
            len: size as u64,
        });
        assoc_sizes.push((kind, w, h));
    }
    let mut props = Props::new(Vendor::Mirax);
    props.mpp = Some(s0.mpp).filter(|m| m.0 > 0.0 && m.1 > 0.0);
    props.objective = ini
        .get("GENERAL", "OBJECTIVE_MAGNIFICATION")
        .and_then(|m| m.trim_end_matches('x').trim().parse().ok())
        .filter(|m: &f64| *m > 0.0);
    props.scanner = ini
        .get("GENERAL", "SCANNER_TYPE")
        .or(ini.get("GENERAL", "CAMERA_TYPE"))
        .map(str::to_owned);
    props.date = ini
        .get("GENERAL", "SLIDE_CREATIONDATETIME")
        .map(str::to_owned);
    props.name = ini
        .get("GENERAL", "SLIDE_NAME")
        .map(str::to_owned)
        .filter(|n| !n.is_empty());
    props.sample = format!(
        "8-bit RGB, {}",
        ini.get(
            ini.get("HIERARCHICAL", "HIER_0_VAL_0_SECTION")
                .unwrap_or(""),
            "IMAGE_FORMAT"
        )
        .unwrap_or("JPEG")
    );
    for (k, label) in [
        ("SLIDE_TYPE", "Slide type"),
        ("CURRENT_SLIDE_VERSION", "Slide version"),
        ("OBJECTIVE_NAME", "Objective"),
        ("CAMERA_TYPE", "Camera"),
        ("SLIDE_ID", "Slide id"),
    ] {
        if let Some(v) = ini.get("GENERAL", k).filter(|v| !v.is_empty()) {
            props.properties.push((label.to_owned(), v.to_owned()));
        }
    }
    // Fluorescence filters.
    if let Some(f) = (0..hier_count.clamp(0, 64))
        .find(|i| ini.get("HIERARCHICAL", &format!("HIER_{i}_NAME")) == Some("Slide filter level"))
    {
        let n = ini
            .int("HIERARCHICAL", &format!("HIER_{f}_COUNT"))
            .unwrap_or(0);
        for k in 0..n.clamp(0, 64) {
            let Some(g) = ini.get("HIERARCHICAL", &format!("HIER_{f}_VAL_{k}_SECTION")) else {
                continue;
            };
            if let Some(name) = ini.get(g, "FILTER_NAME") {
                let mut v = name.to_owned();
                if let Some(e) = ini.get(g, "EXPOSURE_TIME") {
                    v += &format!(", exposure {e} µs");
                }
                props.properties.push((format!("Filter {}", k + 1), v));
            }
        }
    }
    let fill = s0.fill;
    let mut slide_files = vec![
        "Slidedat.ini".to_owned(),
        index_file(slidedat).unwrap_or_else(|| "Index.dat".into()),
    ];
    slide_files.extend(datafiles);
    let mosaic = Mosaic::new(
        levels,
        [fill[0], fill[1], fill[2], 255],
        Box::new(MiraxTiles { images }),
    );
    Ok(Slide {
        props,
        width,
        height,
        levels: slide_levels,
        channels: vec![Channel::default()],
        size_z: 1,
        size_t: 1,
        associated: assoc_sizes,
        files: slide_files,
        backend: crate::Backend::Stitched(Box::new(Stitched::new(
            mosaic,
            false,
            vec![Channel::default()],
            assoc,
        ))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_groups_and_values() {
        let ini = Ini::parse(
            "\u{feff}[GENERAL]\r\nSLIDE_ID = AB\r\n; note\r\nX=1\r\n[S]\r\nMPP = 0,25\r\n",
        );
        assert_eq!(ini.get("GENERAL", "SLIDE_ID"), Some("AB"));
        assert_eq!(ini.int("GENERAL", "X"), Some(1));
        assert_eq!(ini.num("S", "MPP"), Some(0.25));
        assert_eq!(ini.get("S", "X"), None);
    }

    #[test]
    fn data_pages_refuse_loops_and_bad_pointers() {
        // A list at 0: zero, first page at 8; the page at 8 points to itself.
        let mut idx = vec![0u8; 8];
        idx[4..8].copy_from_slice(&8i32.to_le_bytes());
        idx.extend_from_slice(&0i32.to_le_bytes());
        idx.extend_from_slice(&8i32.to_le_bytes());
        assert!(pages(&idx, 0).is_err());
        assert!(pages(&idx, -4).is_err());
        assert!(pages(&idx, 1000).is_err());
        // A page that ends the list.
        idx[12..16].copy_from_slice(&0i32.to_le_bytes());
        assert_eq!(pages(&idx, 0).unwrap().len(), 0);
    }
}
