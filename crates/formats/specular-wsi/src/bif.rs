//! Roche/Ventana BIF: a BigTIFF whose first IFD's XMP (tag 700) holds an `iScan` element, and
//! whose pyramid levels are described `level=N mag=M quality=Q`.
//!
//! Level 0 of a scan is a grid of camera tiles that overlap. Its XMP (`EncodeInfo`, BIF
//! whitepaper) lists the scanned areas (`SlideStitchInfo/ImageInfo` with `AoiOrigin`) and,
//! per pair of neighbouring tiles, a `TileJointInfo`: the two tiles (numbered from 1 in
//! serpentine order from the bottom-left: rows bottom to top, every second row right to left)
//! and their overlap. A tile is placed from its neighbours: across a row by the horizontal
//! joints, down a column by the vertical ones, each joint's overlap where the scanner was
//! confident of it (else the confidence-weighted mean of its direction). The coarser levels
//! were made by downscaling the unstitched tiles, so each of their TIFF tiles holds
//! `ds × ds` level-0 tiles, which are placed the same way at `1/ds`. Where tiles overlap the
//! lower-right one shows (as OpenSlide paints).
//!
//! The `Label_Image` IFD is the slide's overview photograph, shown as the macro image (as
//! OpenSlide names it); `Thumbnail` is the thumbnail; `Probability_Image` (the tissue map)
//! is skipped. A Ventana TIFF without `EncodeInfo` is read as its plain tile grid.

use std::sync::atomic::AtomicBool;

use specular_tiff as tf;

use crate::mosaic::{Mosaic, MosaicLevel, Placed, TileSource};
use crate::stitched::{Assoc, Stitched};
use crate::xml::{self, Element};
use crate::{
    AssociatedKind, Channel, Error, Files, Level, Position, Props, Result, Rgba, Slide, Vendor,
};

/// The XMP packet tag.
pub(crate) const TAG_XMP: u16 = 700;
/// Most tiles per level accepted (a level of 1 Gpx in 1024² tiles is 1024).
const MAX_TILES: u64 = 1 << 22;

/// Whether IFD 0's XMP names an iScan scanner.
pub(crate) fn is_bif(d: &tf::Directories, data: &[u8]) -> bool {
    d.bytes(data, 0, TAG_XMP)
        .is_some_and(|x| memfind(x, b"<iScan").is_some())
}

fn memfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// The iScan element of the first IFD's XMP (`/iScan` or `/Metadata/iScan`).
fn iscan(root: &Element) -> Option<&Element> {
    if root.name == "iScan" {
        Some(root)
    } else {
        root.child("iScan").or_else(|| root.find("iScan"))
    }
}

/// `level=N mag=M` of a level's description.
fn level_info(desc: &str) -> Option<(u32, f64)> {
    let mut level = None;
    let mut mag = None;
    for kv in desc.split_whitespace() {
        match kv.split_once('=') {
            Some(("level", v)) => level = v.parse().ok(),
            Some(("mag", v)) => mag = v.parse::<f64>().ok().filter(|m| m.is_finite() && *m > 0.0),
            _ => {}
        }
    }
    Some((level?, mag?))
}

/// One scanned area of level 0.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Area {
    /// The area's first column and row in the TIFF tile grid.
    pub start_col: u32,
    pub start_row: u32,
    pub cols: u32,
    pub rows: u32,
    /// The area's top-left on the slide, level-0 pixels (from `Pos-X` and the bottom-up
    /// `Pos-Y`).
    pub x: f64,
    pub y: f64,
    /// Each tile's offset from the area's top-left, row-major.
    pub offsets: Vec<(f64, f64)>,
}

/// A joint's overlap and whether it counts (confident and joined).
#[derive(Clone, Copy, Debug, Default)]
struct Joint {
    overlap: f64,
    known: bool,
}

/// The tile (column, row from the top) of serpentine tile number `n` (from 1).
fn serpentine(n: i64, cols: u32, rows: u32) -> Option<(u32, u32)> {
    let total = i64::from(cols) * i64::from(rows);
    if n < 1 || n > total {
        return None;
    }
    let i = n - 1;
    let (mut c, r) = ((i % i64::from(cols)) as u32, (i / i64::from(cols)) as u32);
    if r % 2 == 1 {
        c = cols - 1 - c;
    }
    Some((c, rows - 1 - r))
}

/// The areas of level 0's `EncodeInfo`, for TIFF tiles of `tw`×`th`.
pub(crate) fn parse_encode_info(x: &str, tw: u32, th: u32) -> Result<Vec<Area>> {
    let root = xml::parse(x)?;
    let bad = |m: &str| Error::Malformed(format!("BIF EncodeInfo: {m}"));
    let stitch = root
        .find("SlideStitchInfo")
        .ok_or_else(|| bad("no SlideStitchInfo"))?;
    let infos: Vec<&Element> = stitch.all("ImageInfo").collect();
    let origins: Vec<&Element> = root
        .find("AoiOrigin")
        .map(|o| o.children.iter().collect())
        .unwrap_or_default();
    let mut areas = Vec::new();
    // Weighted sums of horizontal and vertical overlaps (for joints without a value of their own).
    let mut sum = [0f64; 2];
    let mut weight = [0f64; 2];
    let mut joints: Vec<(Vec<Joint>, Vec<Joint>)> = Vec::new();
    for (i, info) in infos.iter().enumerate() {
        if info.num("AOIScanned").unwrap_or(1.0) == 0.0 {
            continue;
        }
        let num = |e: &Element, k: &str| {
            e.num(k)
                .ok_or_else(|| bad(&format!("ImageInfo without {k}")))
        };
        if num(info, "Width")? as u32 != tw || num(info, "Height")? as u32 != th {
            return Err(bad("tile size differs from the TIFF's"));
        }
        let cols = num(info, "NumCols")?;
        let rows = num(info, "NumRows")?;
        if !(1.0..=65536.0).contains(&cols)
            || !(1.0..=65536.0).contains(&rows)
            || cols * rows > MAX_TILES as f64
        {
            return Err(bad("tile count"));
        }
        let (cols, rows) = (cols as u32, rows as u32);
        let origin = origins.get(i).copied();
        let ox = origin.and_then(|o| o.num("OriginX")).unwrap_or(0.0);
        let oy = origin.and_then(|o| o.num("OriginY")).unwrap_or(0.0);
        if ox < 0.0 || oy < 0.0 || ox % f64::from(tw) != 0.0 || oy % f64::from(th) != 0.0 {
            return Err(bad("area origin not on the tile grid"));
        }
        let n = cols as usize * rows as usize;
        // Joint on each tile's left (horizontal) and top (vertical) side.
        let mut left = vec![Joint::default(); n];
        let mut top = vec![Joint::default(); n];
        for j in info.all("TileJointInfo") {
            let (Some(t1), Some(t2)) = (j.num("Tile1"), j.num("Tile2")) else {
                continue;
            };
            let (Some(a), Some(b)) = (
                serpentine(t1 as i64, cols, rows),
                serpentine(t2 as i64, cols, rows),
            ) else {
                continue;
            };
            let conf = j.num("Confidence").unwrap_or(0.0).max(0.0);
            let joined = j.num("FlagJoined").unwrap_or(1.0) != 0.0;
            let (slot, axis, overlap) = if a.1 == b.1 && a.0.abs_diff(b.0) == 1 {
                let right = a.0.max(b.0);
                (
                    &mut left[(a.1 * cols + right) as usize],
                    0,
                    j.num("OverlapX"),
                )
            } else if a.0 == b.0 && a.1.abs_diff(b.1) == 1 {
                let lower = a.1.max(b.1);
                (
                    &mut top[(lower * cols + a.0) as usize],
                    1,
                    j.num("OverlapY"),
                )
            } else {
                continue;
            };
            let Some(overlap) = overlap else {
                continue;
            };
            *slot = Joint {
                overlap,
                known: joined && conf > 0.0,
            };
            sum[axis] += conf * overlap;
            weight[axis] += conf;
        }
        areas.push(Area {
            start_col: (ox / f64::from(tw)) as u32,
            start_row: (oy / f64::from(th)) as u32,
            cols,
            rows,
            x: info.num("Pos-X").unwrap_or(0.0),
            y: info.num("Pos-Y").unwrap_or(0.0),
            offsets: Vec::new(),
        });
        joints.push((left, top));
    }
    if areas.is_empty() {
        return Err(bad("no scanned area"));
    }
    let mean = [0, 1].map(|a| {
        if weight[a] > 0.0 {
            sum[a] / weight[a]
        } else {
            0.0
        }
    });
    for (area, (left, top)) in areas.iter_mut().zip(&joints) {
        let (cols, rows) = (area.cols as usize, area.rows as usize);
        let mut offs = vec![(0f64, 0f64); cols * rows];
        let ov = |j: &Joint, a: usize| if j.known { j.overlap } else { mean[a] };
        for r in 0..rows {
            for c in 0..cols {
                let i = r * cols + c;
                let x = if c == 0 {
                    0.0
                } else {
                    offs[i - 1].0 + f64::from(tw) - ov(&left[i], 0)
                };
                let y = if r == 0 {
                    0.0
                } else {
                    offs[i - cols].1 + f64::from(th) - ov(&top[i], 1)
                };
                offs[i] = (x, y);
            }
        }
        area.offsets = offs;
    }
    // Pos-Y runs up from below the slide: the area's top is `top − Pos-Y − height`.
    let height = |a: &Area| a.offsets.iter().fold(0f64, |m, o| m.max(o.1)) + f64::from(th);
    let top = areas
        .iter()
        .map(|a| a.y + height(a))
        .fold(f64::MIN, f64::max);
    for a in &mut areas {
        a.y = top - a.y - height(a);
    }
    let (mx, my) = areas
        .iter()
        .fold((f64::MAX, f64::MAX), |(x, y), a| (x.min(a.x), y.min(a.y)));
    for a in &mut areas {
        a.x -= mx;
        a.y -= my;
    }
    Ok(areas)
}

/// One TIFF level of the file.
struct TiffLevel {
    page: tf::Page,
    tw: u32,
    th: u32,
    across: u32,
}

struct BifTiles {
    levels: Vec<TiffLevel>,
    le: bool,
}

impl TileSource for BifTiles {
    fn rgba(
        &self,
        files: Files,
        level: usize,
        src: u32,
        _pos: Position,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Rgba>> {
        let l = self
            .levels
            .get(level)
            .ok_or_else(|| Error::Malformed("level".into()))?;
        let data = files.get(0)?;
        let (tc, tr) = (src % l.across, src / l.across);
        let (x, y) = (tc * l.tw, tr * l.th);
        let (w, h) = (l.page.width(), l.page.height());
        if x >= w || y >= h {
            return Ok(None);
        }
        let d = l.page.decode(
            data,
            self.le,
            &tf::DecodeOptions {
                scale: 1,
                region: Some(tf::Region {
                    x,
                    y,
                    w: l.tw.min(w - x),
                    h: l.th.min(h - y),
                }),
                cancel,
                parallel: false,
                wide: false,
            },
        )?;
        Ok(Some(crate::composite::tiff_rgba(d)))
    }
}

/// Level `l`'s placed tiles: every level-0 tile of every area at `1/ds`, cut from the TIFF
/// tile that holds it; or the plain TIFF grid when the tiles do not divide.
fn placements(areas: Option<&[Area]>, l: &TiffLevel, ds: u32, tw0: u32, th0: u32) -> Vec<Placed> {
    let mut out = Vec::new();
    match areas {
        Some(areas)
            if tw0.is_multiple_of(ds) && th0.is_multiple_of(ds) && l.tw == tw0 && l.th == th0 =>
        {
            let (sw, sh) = (tw0 / ds, th0 / ds);
            // Lower-right tiles last, so they show where tiles overlap (as OpenSlide paints).
            for a in areas {
                for r in 0..a.rows {
                    for c in 0..a.cols {
                        let (gc, gr) = (a.start_col + c, a.start_row + r);
                        let (ox, oy) = a.offsets[(r * a.cols + c) as usize];
                        out.push(Placed {
                            x: ((a.x + ox) / f64::from(ds)).round() as i64,
                            y: ((a.y + oy) / f64::from(ds)).round() as i64,
                            w: sw,
                            h: sh,
                            src: (gr / ds) * l.across + gc / ds,
                            sx: (gc % ds) * sw,
                            sy: (gr % ds) * sh,
                        });
                    }
                }
            }
        }
        _ => {
            let down = l.page.height().div_ceil(l.th);
            for r in 0..down {
                for c in 0..l.across {
                    out.push(Placed {
                        x: i64::from(c * l.tw),
                        y: i64::from(r * l.th),
                        w: l.tw,
                        h: l.th,
                        src: r * l.across + c,
                        sx: 0,
                        sy: 0,
                    });
                }
            }
        }
    }
    out
}

pub(crate) fn open(data: &[u8], d: tf::Directories) -> Result<Slide> {
    let xmp0 = d
        .bytes(data, 0, TAG_XMP)
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let root = xml::parse(xmp0.trim_end_matches('\0'))?;
    let scan =
        iscan(&root).ok_or_else(|| Error::Malformed("BIF without an iScan element".into()))?;
    let mains = d.main_chain();
    let mut tiff_levels: Vec<(u32, f64, usize, u32)> = Vec::new();
    let mut assoc = Vec::new();
    for (k, &di) in mains.iter().enumerate() {
        let desc = d.ascii(data, di, 270).unwrap_or_default();
        let Some(ifd) = d.dirs[di].ifd.as_ref() else {
            continue;
        };
        let desc = desc.trim();
        if let Some((level, mag)) = level_info(desc) {
            if ifd.tiled {
                tiff_levels.push((level, mag, di, k as u32));
            }
        } else if desc == "Label Image" || desc == "Label_Image" {
            assoc.push((AssociatedKind::Macro, k as u32, ifd.width, ifd.height));
        } else if desc == "Thumbnail" {
            assoc.push((AssociatedKind::Thumbnail, k as u32, ifd.width, ifd.height));
        }
    }
    tiff_levels.sort_by_key(|l| l.0);
    let Some(&(_, mag0, di0, _)) = tiff_levels.first() else {
        return Err(Error::Malformed("BIF without pyramid levels".into()));
    };
    let ifd0 = d.dirs[di0]
        .ifd
        .clone()
        .ok_or_else(|| Error::Malformed("BIF level 0".into()))?;
    let (tw0, th0) = (ifd0.unit_w, ifd0.unit_h);
    if tw0 == 0 || th0 == 0 {
        return Err(Error::Malformed("BIF tile size".into()));
    }
    let areas = match d.bytes(data, di0, TAG_XMP) {
        Some(x) => {
            let x = String::from_utf8_lossy(x);
            Some(parse_encode_info(x.trim_end_matches('\0'), tw0, th0)?)
        }
        None => None,
    };
    // Full size: the stitched areas' extent, else the TIFF's.
    let (width, height) = match &areas {
        Some(areas) => {
            let mut w = 0f64;
            let mut h = 0f64;
            for a in areas {
                for o in &a.offsets {
                    w = w.max(a.x + o.0 + f64::from(tw0));
                    h = h.max(a.y + o.1 + f64::from(th0));
                }
            }
            (w.ceil() as u32, h.ceil() as u32)
        }
        None => (ifd0.width, ifd0.height),
    };
    let mut levels = Vec::new();
    let mut tl = Vec::new();
    let mut slide_levels = Vec::new();
    for &(_, mag, di, _) in &tiff_levels {
        let Some(ifd) = d.dirs[di].ifd.clone() else {
            continue;
        };
        ifd.check_supported()?;
        let ds = (mag0 / mag).round().max(1.0) as u32;
        if !ds.is_power_of_two() || ifd.unit_w == 0 || ifd.unit_h == 0 {
            continue;
        }
        let page = tf::Page::from_levels(vec![ifd.clone()])
            .ok_or_else(|| Error::Malformed("BIF level".into()))?;
        let l = TiffLevel {
            across: ifd.width.div_ceil(ifd.unit_w),
            tw: ifd.unit_w,
            th: ifd.unit_h,
            page,
        };
        if u64::from(l.across) * u64::from(ifd.height.div_ceil(ifd.unit_h)) > MAX_TILES {
            return Err(Error::TooLarge);
        }
        let tiles = placements(areas.as_deref(), &l, ds, tw0, th0);
        let (lw, lh) = (width.div_ceil(ds), height.div_ceil(ds));
        levels.push(MosaicLevel::new(lw, lh, ds, tiles));
        slide_levels.push(Level {
            width: lw,
            height: lh,
            downsample: f64::from(ds),
        });
        tl.push(l);
    }
    if levels.is_empty() {
        return Err(Error::Malformed("BIF without readable levels".into()));
    }
    let mut props = Props::new(Vendor::Ventana);
    let attr = |k: &str| scan.attr(k).map(str::trim).filter(|v| !v.is_empty());
    props.mpp = scan.num("ScanRes").filter(|m| *m > 0.0).map(|m| (m, m));
    props.objective = scan.num("Magnification").filter(|m| *m > 0.0);
    props.scanner = attr("ScannerModel").map(str::to_owned);
    props.date = d.ascii(data, di0, 306).filter(|s| !s.trim().is_empty());
    props.name = attr("SlideAnnotation").map(str::to_owned);
    props.sample = format!("8-bit RGB, {}", ifd0.compression.name());
    for (k, v) in &scan.attrs {
        if !v.trim().is_empty() {
            props.properties.push((k.clone(), v.trim().to_owned()));
        }
    }
    if let Some(a) = &areas {
        props
            .properties
            .push(("Scanned areas".into(), a.len().to_string()));
    }
    let assoc_sizes = assoc.iter().map(|a| (a.0.clone(), a.2, a.3)).collect();
    let assoc = assoc.iter().map(|a| Assoc::Tiff { ifd: a.1 }).collect();
    let mosaic = Mosaic::new(
        levels,
        [255, 255, 255, 255],
        Box::new(BifTiles {
            levels: tl,
            le: d.little_endian,
        }),
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
        files: Vec::new(),
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
    fn serpentine_numbering_from_the_bottom_left() {
        // 3 columns, 2 rows: bottom row 1 2 3 left to right, top row 6 5 4.
        assert_eq!(serpentine(1, 3, 2), Some((0, 1)));
        assert_eq!(serpentine(3, 3, 2), Some((2, 1)));
        assert_eq!(serpentine(4, 3, 2), Some((2, 0)));
        assert_eq!(serpentine(6, 3, 2), Some((0, 0)));
        assert_eq!(serpentine(0, 3, 2), None);
        assert_eq!(serpentine(7, 3, 2), None);
    }

    #[test]
    fn levels_from_descriptions() {
        assert_eq!(level_info("level=2 mag=10 quality=95"), Some((2, 10.0)));
        assert_eq!(level_info("Label_Image"), None);
        assert_eq!(level_info("level=x mag=1"), None);
    }

    fn encode_info(joints: &str) -> String {
        format!(
            r#"<EncodeInfo Ver="2"><SlideStitchInfo><ImageInfo AOIScanned="1" AOIIndex="0" NumRows="2" NumCols="3" Width="8" Height="8" Pos-X="100" Pos-Y="50">{joints}</ImageInfo></SlideStitchInfo><AoiOrigin><AOI0 OriginX="8" OriginY="0"/></AoiOrigin></EncodeInfo>"#
        )
    }

    #[test]
    fn joints_place_tiles_and_unknown_joints_take_the_mean() {
        let j = r#"<TileJointInfo FlagJoined="1" Confidence="100" Direction="LEFT" Tile1="1" Tile2="2" OverlapX="2" OverlapY="0"/>
            <TileJointInfo FlagJoined="1" Confidence="100" Direction="LEFT" Tile1="2" Tile2="3" OverlapX="4" OverlapY="0"/>
            <TileJointInfo FlagJoined="1" Confidence="100" Direction="UP" Tile1="1" Tile2="6" OverlapX="0" OverlapY="1"/>
            <TileJointInfo FlagJoined="0" Confidence="0" Direction="UP" Tile1="2" Tile2="5" OverlapX="0" OverlapY="7"/>"#;
        let a = parse_encode_info(&encode_info(j), 8, 8).unwrap();
        assert_eq!(a.len(), 1);
        let a = &a[0];
        assert_eq!((a.start_col, a.start_row, a.cols, a.rows), (1, 0, 3, 2));
        // The bottom row (row 1 from the top) has the joints with overlaps 2 and 4; the top
        // row has none: the mean (3) for both.
        assert_eq!(a.offsets[3], (0.0, 7.0));
        assert_eq!(a.offsets[4], (6.0, 7.0 - 1.0 + 1.0));
        assert_eq!(a.offsets[5], (10.0, 7.0));
        assert_eq!(a.offsets[1], (5.0, 0.0));
        assert_eq!((a.x, a.y), (0.0, 0.0));
    }

    #[test]
    fn malformed_encode_info_is_refused() {
        assert!(parse_encode_info("<EncodeInfo/>", 8, 8).is_err());
        let wrong_size = encode_info("").replace(r#"Width="8""#, r#"Width="9""#);
        assert!(parse_encode_info(&wrong_size, 8, 8).is_err());
        let huge = encode_info("").replace(r#"NumRows="2""#, r#"NumRows="4000000000""#);
        assert!(parse_encode_info(&huge, 8, 8).is_err());
    }
}
