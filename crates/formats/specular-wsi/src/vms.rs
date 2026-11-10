//! Hamamatsu VMS and VMU: NDPI's older siblings, an INI index and separate image files.
//!
//! * **VMS** (`[Virtual Microscope Specimen]`): `NoJpegColumns` × `NoJpegRows` JPEG files
//!   (`ImageFile` for (0, 0), `ImageFile(x,y)` for the others) side by side, each with
//!   restart markers, read like NDPI levels: a region splices the restart intervals that
//!   cover it into a small JPEG of its own, decoded at the DCT scale (1/2, 1/4, 1/8) the
//!   downscale needs. The `OptimisationFile` holds, per JPEG in file order, the offsets of
//!   the restart intervals that start its MCU rows (40-byte records, a little-endian u64
//!   first): the search for the other intervals runs between them in parallel. The
//!   `MapFile` is the whole slide at 1/8 (with restart markers): when its size says so it
//!   is the level coarser reads come from. `MacroImage` is the macro photograph.
//! * **VMU** (`[Uncompressed Virtual Microscope Specimen]`): `ImageFile` names NGR files
//!   (`GN`, then width, height and column width as little-endian u32 at 4, 8 and 12 and the
//!   pixels' offset at 24): 16-bit RGB samples (12 significant bits, `BitsPerPixel=36`)
//!   stored in columns of the column width, each column top to bottom.
//!
//! `SourceLens` is the objective, `PhysicalWidth` / `PhysicalHeight` the scanned area (nm),
//! which gives microns per pixel; `X/YOffsetFromSlideCentre` place NDP.view annotations.
//! Focal layers: `NoLayers` grids, layer 0's files named `ImageFile` / `ImageFile(x,y)`,
//! the others' `ImageFile(z,x,y)` (z first, from 1). Each layer is a Z plane of the slide
//! ([`Slide::size_z`], read by [`crate::Position::z`]); layers the index does not name
//! every file of are left out. The optimisation file's hints and the map are layer 0's,
//! so the other layers search their restart intervals themselves and read coarse
//! levels from their own files.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;

use crate::composite::box_reduce_rgba;
use crate::ndpi::{self, NdpiLevel};
use crate::{
    AssociatedKind, Channel, Error, Files, Level, Props, ReadOptions, Region, Result, Rgba, Slide,
    Vendor,
};

/// Largest index file read.
pub const INDEX_MAX: usize = 1 << 20;
/// Most image files of a grid (all focal layers together).
const MAX_TILES: usize = 4096;
/// Most focal layers.
const MAX_LAYERS: u32 = 64;
/// The longest side associated images are decoded at.
const ASSOCIATED_MAX: u32 = 8192;

/// Which index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Vms,
    Vmu,
}

fn first_section(data: &[u8]) -> Option<&str> {
    let head = &data[..data.len().min(4096)];
    let s = std::str::from_utf8(head).ok().or_else(|| {
        // A multi-byte character cut at 4 KiB.
        std::str::from_utf8(&head[..head.len().saturating_sub(3)]).ok()
    })?;
    let s = s.trim_start_matches('\u{feff}');
    let line = s.lines().map(str::trim).find(|l| !l.is_empty())?;
    line.strip_prefix('[')?.strip_suffix(']')
}

/// Whether `data` is a VMS or VMU index.
pub fn probe(data: &[u8]) -> Option<Kind> {
    match first_section(data)? {
        "Virtual Microscope Specimen" => Some(Kind::Vms),
        "Uncompressed Virtual Microscope Specimen" => Some(Kind::Vmu),
        _ => None,
    }
}

/// The index's keys and values (first section), in file order.
fn entries(data: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(&data[..data.len().min(INDEX_MAX)]);
    let mut out = Vec::new();
    let mut in_first = false;
    for line in text.lines() {
        let l = line.trim().trim_start_matches('\u{feff}');
        if l.starts_with('[') {
            if in_first {
                break;
            }
            in_first = true;
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            out.push((k.trim().to_owned(), v.trim().to_owned()));
        }
    }
    out
}

/// What the index lays out.
struct Layout {
    kind: Kind,
    cols: u32,
    rows: u32,
    /// (column, row, file name), row-major, of each focal layer (layer 0 first).
    images: Vec<(u32, u32, String)>,
    /// Focal layers whose files the index names (1 or more).
    layers: u32,
    opt: Option<String>,
    map: Option<String>,
    macro_image: Option<String>,
    entries: Vec<(String, String)>,
}

/// `ImageFile(x,y)`'s position (layer 0), or `ImageFile(z,x,y)`'s: (layer, column, row).
fn image_key(k: &str) -> Option<(u32, u32, u32)> {
    let inner = k.strip_prefix("ImageFile(")?.strip_suffix(')')?;
    let v: Vec<u32> = inner
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    match v[..] {
        [x, y] => Some((0, x, y)),
        [z, x, y] => Some((z, x, y)),
        _ => None,
    }
}

fn layout(data: &[u8]) -> Result<Layout> {
    let kind = probe(data).ok_or(Error::NotSlide)?;
    let e = entries(data);
    let get = |k: &str| {
        e.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    let num = |k: &str| get(k).and_then(|v| v.parse::<u32>().ok());
    let cols = num("NoJpegColumns").unwrap_or(1);
    let rows = num("NoJpegRows").unwrap_or(1);
    if cols == 0 || rows == 0 || u64::from(cols) * u64::from(rows) > MAX_TILES as u64 {
        return Err(Error::Malformed(format!("{cols} × {rows} image files")));
    }
    let find = |z: u32, c: u32, r: u32| {
        e.iter()
            .find(|(k, v)| image_key(k) == Some((z, c, r)) && !v.is_empty())
            .map(|(_, v)| v.clone())
    };
    let mut images = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let name = if (c, r) == (0, 0) {
                get("ImageFile").or_else(|| find(0, 0, 0))
            } else {
                find(0, c, r)
            };
            let name = name.ok_or_else(|| {
                Error::Malformed(format!("the index names no image file ({c},{r})"))
            })?;
            images.push((c, r, name));
        }
    }
    // Further focal layers (`NoLayers`), as long as the index names every file of each.
    let wanted = num("NoLayers").unwrap_or(1).clamp(1, MAX_LAYERS);
    let mut layers = 1;
    'layers: for z in 1..wanted {
        if u64::from(z + 1) * u64::from(cols) * u64::from(rows) > MAX_TILES as u64 {
            break;
        }
        let mut grid = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let Some(name) = find(z, c, r) else {
                    break 'layers;
                };
                grid.push((c, r, name));
            }
        }
        images.extend(grid);
        layers += 1;
    }
    Ok(Layout {
        kind,
        cols,
        rows,
        images,
        layers,
        opt: get("OptimisationFile"),
        map: get("MapFile"),
        macro_image: get("MacroImage"),
        entries: e,
    })
}

/// The files a VMS or VMU slide reads besides its index, in the order
/// [`Slide::open_vms`] takes them: the image files row by row (focal layer 0, then the
/// others in order), then the optimisation, map and macro files the index names. Names are as written (paths are the caller's to
/// refuse).
pub fn files(index: &[u8]) -> Result<Vec<String>> {
    let l = layout(index)?;
    let mut out: Vec<String> = l.images.into_iter().map(|(_, _, n)| n).collect();
    out.extend(l.opt);
    out.extend(l.map);
    out.extend(l.macro_image);
    Ok(out)
}

/// One NGR file's layout.
#[derive(Clone, Copy, Debug)]
struct Ngr {
    width: u32,
    height: u32,
    column: u32,
    offset: usize,
}

fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn ngr(d: &[u8]) -> Result<Ngr> {
    let bad = |s: &str| Error::Malformed(format!("NGR {s}"));
    if !d.starts_with(b"GN") {
        return Err(bad("signature"));
    }
    let (width, height, column) = (
        le32(d, 4).ok_or_else(|| bad("header"))?,
        le32(d, 8).ok_or_else(|| bad("header"))?,
        le32(d, 12).ok_or_else(|| bad("header"))?,
    );
    let offset = le32(d, 24).ok_or_else(|| bad("header"))? as usize;
    if width == 0 || height == 0 || column == 0 || column > width {
        return Err(bad("size"));
    }
    Ok(Ngr {
        width,
        height,
        column,
        offset,
    })
}

impl Ngr {
    /// The 8-bit RGB of pixel (`x`, `y`) (12-bit samples, the top 8 bits); `None` past the
    /// file's end.
    #[inline]
    fn pixel(&self, d: &[u8], x: u32, y: u32) -> Option<[u8; 3]> {
        let c = x / self.column;
        let cw = self.column.min(self.width - c * self.column);
        let px = u64::from(c) * u64::from(self.column) * u64::from(self.height)
            + u64::from(y) * u64::from(cw)
            + u64::from(x - c * self.column);
        let at = self.offset.checked_add(usize::try_from(px * 6).ok()?)?;
        let b = d.get(at..at + 6)?;
        let s = |i: usize| (u16::from_le_bytes([b[i], b[i + 1]]) >> 4).min(255) as u8;
        Some([s(0), s(2), s(4)])
    }

    /// Region `r` of the 1/`s` downscale: each output pixel the mean of its block (of up to
    /// 4 × 4 samples spread over it for larger blocks).
    fn read(&self, d: &[u8], s: u32, r: Region, cancel: Option<&AtomicBool>) -> Result<Rgba> {
        let mut out = vec![0u8; r.w as usize * r.h as usize * 4];
        let mut truncated = false;
        let k = s.min(4);
        let step = s / k;
        for oy in 0..r.h {
            if oy % 64 == 0 {
                crate::check_cancel(cancel)?;
            }
            for ox in 0..r.w {
                let (x0, y0) = ((r.x + ox) * s, (r.y + oy) * s);
                let (mut acc, mut n) = ([0u32; 3], 0u32);
                for j in 0..k {
                    for i in 0..k {
                        let (x, y) = (x0 + i * step, y0 + j * step);
                        if x >= self.width || y >= self.height {
                            continue;
                        }
                        match self.pixel(d, x, y) {
                            Some(p) => {
                                for c in 0..3 {
                                    acc[c] += u32::from(p[c]);
                                }
                                n += 1;
                            }
                            None => truncated = true,
                        }
                    }
                }
                let o = (oy as usize * r.w as usize + ox as usize) * 4;
                for c in 0..3 {
                    if let Some(v) = (acc[c] + n / 2).checked_div(n) {
                        out[o + c] = v as u8;
                    }
                }
                out[o + 3] = 255;
            }
        }
        Ok(Rgba {
            width: r.w,
            height: r.h,
            data: out,
            truncated,
        })
    }
}

enum Src {
    Jpeg(NdpiLevel),
    Ngr(Ngr),
    /// A file that could not be read: drawn as background.
    Missing(u32, u32),
}

struct Tile {
    /// Index into [`Files`].
    file: usize,
    /// Focal layer.
    layer: u32,
    x: u32,
    y: u32,
    src: Src,
}

impl Tile {
    fn size(&self) -> (u32, u32) {
        match &self.src {
            Src::Jpeg(l) => (l.width, l.height),
            Src::Ngr(n) => (n.width, n.height),
            Src::Missing(w, h) => (*w, *h),
        }
    }
}

pub(crate) struct Vms {
    tiles: Vec<Tile>,
    /// Focal layers.
    layers: u32,
    /// The map (the slide at 1/8) as a level: its file and level.
    map: Option<(usize, NdpiLevel)>,
    /// A VMU's map (an NGR at 1/8).
    map_ngr: Option<(usize, Ngr)>,
    /// Associated images: their files.
    assoc: Vec<usize>,
    /// JPEGs without restart markers decoded whole, by (file, DCT scale).
    whole: Mutex<HashMap<(usize, u32), Arc<Rgba>>>,
}

/// The optimisation file's offsets, split per JPEG (a new JPEG starts where they stop
/// increasing).
fn opt_runs(d: &[u8]) -> Vec<Vec<u64>> {
    let mut runs: Vec<Vec<u64>> = Vec::new();
    let mut last = None;
    for rec in d.chunks_exact(40).take(1 << 24) {
        let v = u64::from_le_bytes(rec[..8].try_into().unwrap_or([0; 8]));
        match last {
            Some(l) if v > l => {}
            _ => runs.push(Vec::new()),
        }
        if let Some(r) = runs.last_mut() {
            r.push(v);
        }
        last = Some(v);
    }
    runs
}

/// Open a VMS or VMU slide: `files[0]` is the index, then the files [`files`] lists.
pub(crate) fn open(f: Files) -> Result<Slide> {
    let index = f.get(0)?;
    let l = layout(index)?;
    let n = l.images.len();
    let mut at = 1 + n;
    let opt_file = l.opt.as_ref().map(|_| {
        at += 1;
        at - 1
    });
    let map_file = l.map.as_ref().map(|_| {
        at += 1;
        at - 1
    });
    let macro_file = l.macro_image.as_ref().map(|_| {
        at += 1;
        at - 1
    });
    let mut runs = opt_file
        .and_then(|i| f.get(i).ok())
        .map(opt_runs)
        .unwrap_or_default()
        .into_iter();
    let per_layer = (l.cols * l.rows) as usize;
    let mut srcs = Vec::with_capacity(n);
    for (k, (c, r, name)) in l.images.iter().enumerate() {
        let data = f.get(1 + k).ok();
        // Layer 0 must open; a broken file of another layer shows as background.
        let first = k < per_layer;
        let src = match (l.kind, data) {
            (_, None) => None,
            (Kind::Vms, Some(d)) => match ndpi::jpeg_level(d) {
                Ok(mut lv) => {
                    if first && let Some(h) = runs.next() {
                        lv.set_hints(h);
                    }
                    Some(Src::Jpeg(lv))
                }
                Err(e) if first => return Err(Error::Malformed(format!("{name}: {e}"))),
                Err(_) => None,
            },
            (Kind::Vmu, Some(d)) => match ngr(d) {
                Ok(g) => Some(Src::Ngr(g)),
                Err(e) if first => return Err(e),
                Err(_) => None,
            },
        };
        srcs.push((*c, *r, src));
    }
    // Column widths and row heights from the first row and column read.
    let size_of = |s: &Option<Src>| match s {
        Some(Src::Jpeg(l)) => Some((l.width, l.height)),
        Some(Src::Ngr(g)) => Some((g.width, g.height)),
        _ => None,
    };
    let mut widths = vec![None; l.cols as usize];
    let mut heights = vec![None; l.rows as usize];
    for (c, r, s) in &srcs {
        if let Some((w, h)) = size_of(s) {
            widths[*c as usize].get_or_insert(w);
            heights[*r as usize].get_or_insert(h);
        }
    }
    let typical = |v: &[Option<u32>]| v.iter().flatten().copied().next();
    let (tw, th) = (
        typical(&widths)
            .ok_or_else(|| Error::Io("no image file of the slide is readable".into()))?,
        typical(&heights).unwrap_or(1),
    );
    let widths: Vec<u32> = widths.iter().map(|w| w.unwrap_or(tw)).collect();
    let heights: Vec<u32> = heights.iter().map(|h| h.unwrap_or(th)).collect();
    let xs: Vec<u32> = widths
        .iter()
        .scan(0u32, |a, &w| {
            let x = *a;
            *a = a.saturating_add(w);
            Some(x)
        })
        .collect();
    let ys: Vec<u32> = heights
        .iter()
        .scan(0u32, |a, &h| {
            let y = *a;
            *a = a.saturating_add(h);
            Some(y)
        })
        .collect();
    let width: u32 = widths
        .iter()
        .try_fold(0u32, |a, &w| a.checked_add(w))
        .ok_or(Error::TooLarge)?;
    let height: u32 = heights
        .iter()
        .try_fold(0u32, |a, &h| a.checked_add(h))
        .ok_or(Error::TooLarge)?;
    let tiles: Vec<Tile> = srcs
        .into_iter()
        .enumerate()
        .map(|(k, (c, r, s))| Tile {
            file: 1 + k,
            layer: (k / per_layer) as u32,
            x: xs[c as usize],
            y: ys[r as usize],
            src: s.unwrap_or(Src::Missing(widths[c as usize], heights[r as usize])),
        })
        .collect();
    let get = |k: &str| {
        l.entries
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_str())
    };
    let real = |k: &str| {
        get(k)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    let mut props = Props::new(Vendor::Hamamatsu);
    props.objective = real("SourceLens").filter(|m| *m > 0.0);
    if let (Some(pw), Some(ph)) = (real("PhysicalWidth"), real("PhysicalHeight"))
        && pw > 0.0
        && ph > 0.0
    {
        props.mpp = Some((
            pw / 1000.0 / f64::from(width),
            ph / 1000.0 / f64::from(height),
        ));
    }
    props.centre_offset_nm = real("XOffsetFromSlideCentre").zip(real("YOffsetFromSlideCentre"));
    props.name = get("Reference").map(str::to_owned);
    props.sample = match l.kind {
        Kind::Vms => "8-bit RGB, JPEG".to_owned(),
        Kind::Vmu => "12-bit RGB, uncompressed".to_owned(),
    };
    props.properties = l.entries.clone();
    props
        .properties
        .push(("Image files".into(), format!("{} × {}", l.cols, l.rows)));
    if l.layers > 1 {
        props
            .properties
            .push(("Focal layers".into(), l.layers.to_string()));
    }
    // The map as the 1/8 level when its size says it is one.
    let map = match (l.kind, map_file.and_then(|i| f.get(i).ok().map(|d| (i, d)))) {
        (Kind::Vms, Some((i, d))) => ndpi::jpeg_level(d).ok().and_then(|lv| {
            let (vw, vh) = (width.div_ceil(8), height.div_ceil(8));
            (lv.width.abs_diff(vw) <= 1 && lv.height.abs_diff(vh) <= 1).then_some((i, lv))
        }),
        _ => None,
    };
    let map_ngr = match (l.kind, map_file.and_then(|i| f.get(i).ok().map(|d| (i, d)))) {
        (Kind::Vmu, Some((i, d))) => ngr(d).ok().and_then(|g| {
            let (vw, vh) = (width / 8, height / 8);
            (g.width.abs_diff(vw) <= 1 && g.height.abs_diff(vh) <= 1).then_some((i, g))
        }),
        _ => None,
    };
    let mut levels = vec![Level {
        width,
        height,
        downsample: 1.0,
    }];
    if let Some((_, g)) = &map_ngr {
        levels.push(Level {
            width: g.width,
            height: g.height,
            downsample: 8.0,
        });
    }
    if l.kind == Kind::Vms {
        for d in [2u32, 4, 8] {
            levels.push(Level {
                width: width.div_ceil(d),
                height: height.div_ceil(d),
                downsample: f64::from(d),
            });
        }
    }
    let mut associated = Vec::new();
    let mut assoc = Vec::new();
    if let Some(i) = macro_file
        && let Ok(d) = f.get(i)
        && let Some((w, h)) = jpeg_size(d)
    {
        associated.push((AssociatedKind::Macro, w, h));
        assoc.push(i);
    }
    if map.is_none()
        && map_ngr.is_none()
        && let Some(i) = map_file
        && let Ok(d) = f.get(i)
        && let Some((w, h)) = jpeg_size(d)
    {
        associated.push((AssociatedKind::Other("Map".into()), w, h));
        assoc.push(i);
    }
    Ok(Slide {
        props,
        width,
        height,
        levels,
        channels: vec![Channel::default()],
        size_z: l.layers,
        size_t: 1,
        associated,
        files: Vec::new(),
        backend: crate::Backend::Vms(Box::new(Vms {
            tiles,
            layers: l.layers,
            map,
            map_ngr,
            assoc,
            whole: Mutex::new(HashMap::new()),
        })),
    })
}

fn jpeg_size(d: &[u8]) -> Option<(u32, u32)> {
    ndpi::jpeg_level(d).ok().map(|l| (l.width, l.height))
}

impl Vms {
    fn whole(
        &self,
        file: usize,
        l: &NdpiLevel,
        data: &[u8],
        e: u32,
        cancel: Option<&AtomicBool>,
    ) -> Result<Arc<Rgba>> {
        if let Some(w) = self.whole.lock().get(&(file, e)) {
            return Ok(w.clone());
        }
        let px = Arc::new(l.decode_whole(data, e, cancel)?);
        self.whole.lock().insert((file, e), px.clone());
        Ok(px)
    }

    pub(crate) fn read(&self, files: Files, o: &ReadOptions) -> Result<Rgba> {
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let s = o.scale.max(1);
        let z = o.position.z;
        if z >= self.layers {
            return Err(Error::Malformed(format!("no focal layer {z}")));
        }
        // Coarse reads come from the map (layer 0's) when it is the 1/8 level.
        if s >= 8
            && o.position.z == 0
            && let Some((file, map)) = &self.map
            && let Ok(d) = files.get(*file)
        {
            let r8 = Region::clip(
                Some(r),
                map.width.div_ceil(s / 8),
                map.height.div_ceil(s / 8),
            );
            if let Some(r8) = r8 {
                let img =
                    map.read_scaled(d, s / 8, o, r8, |e| self.whole(*file, map, d, e, o.cancel))?;
                return Ok(place(&img, r8, r));
            }
        }
        if s >= 8
            && o.position.z == 0
            && let Some((file, g)) = &self.map_ngr
            && let Ok(d) = files.get(*file)
            && let Some(r8) =
                Region::clip(Some(r), g.width.div_ceil(s / 8), g.height.div_ceil(s / 8))
        {
            let img = g.read(d, s / 8, r8, o.cancel)?;
            return Ok(place(&img, r8, r));
        }
        let mut out = Rgba {
            width: r.w,
            height: r.h,
            data: vec![255u8; r.w as usize * r.h as usize * 4],
            truncated: false,
        };
        for t in self.tiles.iter().filter(|t| t.layer == z) {
            let (w, h) = t.size();
            let (ox, oy) = (t.x / s, t.y / s);
            let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
            // The tile's part of the output, in output pixels.
            let x0 = r.x.max(ox);
            let y0 = r.y.max(oy);
            let x1 = (r.x + r.w).min(ox + ow);
            let y1 = (r.y + r.h).min(oy + oh);
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            crate::check_cancel(o.cancel)?;
            let local = Region {
                x: x0 - ox,
                y: y0 - oy,
                w: x1 - x0,
                h: y1 - y0,
            };
            let img = match (&t.src, files.get(t.file)) {
                (Src::Jpeg(l), Ok(d)) => {
                    match l.read_scaled(d, s, o, local, |e| self.whole(t.file, l, d, e, o.cancel)) {
                        Ok(i) => Some(i),
                        Err(e @ (Error::Cancelled | Error::TooLarge)) => return Err(e),
                        Err(_) => None,
                    }
                }
                (Src::Ngr(g), Ok(d)) => Some(g.read(d, s, local, o.cancel)?),
                _ => None,
            };
            let Some(img) = img else {
                out.truncated = true;
                continue;
            };
            out.truncated |= img.truncated;
            for y in 0..local.h.min(img.height) {
                let src = &img.data[(y * img.width) as usize * 4..]
                    [..(local.w.min(img.width) * 4) as usize];
                let dy = y0 - r.y + y;
                let dst = (dy as usize * r.w as usize + (x0 - r.x) as usize) * 4;
                out.data[dst..dst + src.len()].copy_from_slice(src);
            }
        }
        Ok(out)
    }

    pub(crate) fn read_associated(
        &self,
        files: Files,
        i: usize,
        cancel: Option<&AtomicBool>,
    ) -> Result<Rgba> {
        let file = *self
            .assoc
            .get(i)
            .ok_or_else(|| Error::Malformed("no such associated image".into()))?;
        let d = files.get(file)?;
        let img = crate::blob::rgba(d, cancel)?;
        let mut f = 1;
        while img.width / f > ASSOCIATED_MAX || img.height / f > ASSOCIATED_MAX {
            f *= 2;
        }
        Ok(box_reduce_rgba(&img, f))
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        let starts: usize = self
            .tiles
            .iter()
            .map(|t| match &t.src {
                Src::Jpeg(l) => l.starts_bytes(),
                _ => 0,
            })
            .sum::<usize>()
            + self.map.as_ref().map_or(0, |(_, l)| l.starts_bytes());
        starts
            + self
                .whole
                .lock()
                .values()
                .map(|w| w.data.len())
                .sum::<usize>()
    }

    pub(crate) fn trim(&self) -> usize {
        let mut m = self.whole.lock();
        let n = m.values().map(|w| w.data.len()).sum();
        m.clear();
        n
    }
}

/// `img` (region `got` of the output's pixels) placed in output region `want` (white
/// where it does not reach).
fn place(img: &Rgba, got: Region, want: Region) -> Rgba {
    if got == want && img.width == want.w && img.height == want.h {
        return img.clone();
    }
    let mut out = vec![255u8; want.w as usize * want.h as usize * 4];
    for y in 0..got.h.min(img.height) {
        let dy = got.y + y;
        if dy < want.y || dy >= want.y + want.h {
            continue;
        }
        let n = got.w.min(img.width).min(want.w) as usize * 4;
        let src = &img.data[(y * img.width) as usize * 4..][..n];
        let dst = ((dy - want.y) as usize * want.w as usize) * 4;
        out[dst..dst + n].copy_from_slice(src);
    }
    Rgba {
        width: want.w,
        height: want.h,
        data: out,
        truncated: img.truncated,
    }
}
