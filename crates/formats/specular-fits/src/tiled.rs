//! Tile-compressed images (the FITS tiled image compression convention, FITS 4.0 §10): a
//! binary table with `ZIMAGE = T` whose rows are the image's tiles.
//!
//! Each row holds one tile, compressed in the `COMPRESSED_DATA` variable-length column
//! (`1PB` / `1QB`, or `1PI` for PLIO), or stored losslessly in `GZIP_COMPRESSED_DATA` (gzip
//! of the raw values) or `UNCOMPRESSED_DATA` when quantising a float tile failed. Floating
//! point images are usually quantised to integers per tile: `ZSCALE` / `ZZERO` (columns or
//! keywords) map them back, after the subtractive dither of `ZQUANTIZ` (the reference random
//! sequence of the convention, §10.2). `ZBLANK` marks undefined pixels.

use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::compress::{self, CodecError};
use crate::header::Header;
use crate::{Error, Hdu, ImageDesc, Plane, Result, axes_of, check_size};

/// The tile compression algorithm (`ZCMPTYPE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Rice,
    Gzip1,
    Gzip2,
    Hcompress,
    Plio,
    None,
}

impl Algorithm {
    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Rice => "Rice",
            Algorithm::Gzip1 => "GZIP",
            Algorithm::Gzip2 => "GZIP (shuffled)",
            Algorithm::Hcompress => "HCOMPRESS",
            Algorithm::Plio => "PLIO",
            Algorithm::None => "tiled, uncompressed",
        }
    }
}

/// How a floating-point image was quantised (`ZQUANTIZ`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantize {
    /// Integer image, or float tiles stored losslessly.
    Lossless,
    NoDither,
    SubtractiveDither1,
    SubtractiveDither2,
}

/// A column of the tile table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Column {
    /// Byte offset in the row.
    offset: usize,
    /// Element type (`B`, `I`, `J`, `K`, `E`, `D`).
    ty: u8,
    /// Variable-length: 4-byte (`P`) or 8-byte (`Q`) descriptors; fixed otherwise.
    var: Option<bool>,
}

/// The compression of a tile-compressed image.
#[derive(Clone, Debug, PartialEq)]
pub struct Tiles {
    pub algorithm: Algorithm,
    pub quantize: Quantize,
    /// `ZTILEn`.
    pub tile: Vec<usize>,
    pub blocksize: usize,
    pub bytepix: usize,
    pub smooth: bool,
    pub dither0: i64,
    row_len: usize,
    rows: usize,
    heap: usize,
    comp: Option<Column>,
    gzip: Option<Column>,
    raw: Option<Column>,
    zscale: Option<Column>,
    zzero: Option<Column>,
    zblank: Option<Column>,
    zscale_key: Option<f64>,
    zzero_key: Option<f64>,
    zblank_key: Option<i64>,
    slab: SlabCache,
}

/// Most samples of decoded tiles kept for the next plane (128 MiB).
const SLAB_BUDGET: usize = 1 << 25;

/// The decoded tiles (physical values, or why each failed) of one row of tiles across the
/// plane, by tile.
type Slab = Arc<Vec<Result<Vec<f32>>>>;

/// The last slab of tiles deeper than one plane: those tiles hold every plane they span in
/// one stream, and decoding them again for each plane was quadratic in their depth.
#[derive(Default)]
struct SlabCache(Mutex<Option<(usize, Slab)>>);

impl SlabCache {
    fn get(&self, first_row: usize) -> Option<Slab> {
        match &*self.0.lock().ok()? {
            Some((r, s)) if *r == first_row => Some(s.clone()),
            _ => None,
        }
    }

    fn put(&self, first_row: usize, slab: Slab) {
        if let Ok(mut g) = self.0.lock() {
            *g = Some((first_row, slab));
        }
    }
}

impl Clone for SlabCache {
    fn clone(&self) -> Self {
        SlabCache::default()
    }
}

impl PartialEq for SlabCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for SlabCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SlabCache")
    }
}

fn elem_size(ty: u8) -> Option<usize> {
    Some(match ty {
        b'L' | b'B' | b'A' => 1,
        b'I' => 2,
        b'J' | b'E' => 4,
        b'K' | b'D' | b'C' => 8,
        b'M' => 16,
        _ => return None,
    })
}

/// Parses `TFORMn`: (repeat, type, variable-length kind).
fn tform(s: &str) -> Option<(usize, u8, Option<bool>)> {
    let s = s.trim();
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    let repeat = if digits == 0 {
        1
    } else {
        s[..digits].parse().ok()?
    };
    let rest = &s.as_bytes()[digits..];
    let t = *rest.first()?;
    match t {
        b'P' | b'Q' => Some((repeat, *rest.get(1)?, Some(t == b'Q'))),
        _ => Some((repeat, t, None)),
    }
}

/// Reads the description of a compressed image HDU.
pub(crate) fn image_desc(h: &Header) -> Result<ImageDesc> {
    let bitpix = h
        .i64("ZBITPIX")
        .ok_or_else(|| Error::Malformed("no ZBITPIX".into()))?;
    if !matches!(bitpix, 8 | 16 | 32 | 64 | -32 | -64) {
        return Err(Error::Malformed(format!("ZBITPIX = {bitpix}")));
    }
    let axes = axes_of(h, "ZNAXIS", "ZNAXIS")?;
    if axes.is_empty() || axes.contains(&0) {
        return Err(Error::Malformed("empty compressed image".into()));
    }
    check_size(&axes)?;
    let algorithm = match h.str("ZCMPTYPE").map(str::trim) {
        Some("RICE_1") | Some("RICE_ONE") => Algorithm::Rice,
        Some("GZIP_1") => Algorithm::Gzip1,
        Some("GZIP_2") => Algorithm::Gzip2,
        Some("HCOMPRESS_1") => Algorithm::Hcompress,
        Some("PLIO_1") => Algorithm::Plio,
        Some("NOCOMPRESS") => Algorithm::None,
        other => {
            return Err(Error::Unsupported(format!(
                "tile compression {}",
                other.unwrap_or("?")
            )));
        }
    };
    let mut tile = Vec::with_capacity(axes.len());
    for (i, &a) in axes.iter().enumerate() {
        let t = h
            .i64(&format!("ZTILE{}", i + 1))
            .unwrap_or(if i == 0 { a as i64 } else { 1 });
        if t < 1 {
            return Err(Error::Malformed("ZTILE < 1".into()));
        }
        tile.push((t as usize).min(a));
    }
    let tile_px = tile.iter().try_fold(1u64, |a, &b| a.checked_mul(b as u64));
    if tile_px.is_none_or(|t| t > crate::MAX_PLANE) {
        return Err(Error::TooLarge);
    }
    // ZNAMEi / ZVALi parameters.
    let mut blocksize = 32;
    let mut bytepix = match bitpix {
        8 => 1,
        16 => 2,
        _ => 4,
    };
    let mut smooth = false;
    for i in 1..=999 {
        let Some(name) = h.str(&format!("ZNAME{i}")) else {
            break;
        };
        let val = h.i64(&format!("ZVAL{i}"));
        match (name.trim().to_ascii_uppercase().as_str(), val) {
            ("BLOCKSIZE", Some(v)) if (1..=1 << 16).contains(&v) => blocksize = v as usize,
            ("BYTEPIX", Some(v)) if matches!(v, 1 | 2 | 4 | 8) => bytepix = v as usize,
            ("SMOOTH", Some(v)) => smooth = v != 0,
            _ => {}
        }
    }
    let float = bitpix < 0;
    let quantize = match h.str("ZQUANTIZ").map(str::trim) {
        Some("NO_DITHER") => Quantize::NoDither,
        Some("SUBTRACTIVE_DITHER_1") => Quantize::SubtractiveDither1,
        Some("SUBTRACTIVE_DITHER_2") => Quantize::SubtractiveDither2,
        Some("NONE") => Quantize::Lossless,
        _ if float && (h.has("ZSCALE") || has_column(h, "ZSCALE")) => Quantize::NoDither,
        _ => Quantize::Lossless,
    };
    // Columns.
    let fields = h.i64("TFIELDS").unwrap_or(0).clamp(0, 999) as usize;
    let mut offset = 0usize;
    let (mut comp, mut gzip, mut raw, mut zscale, mut zzero, mut zblank) =
        (None, None, None, None, None, None);
    for i in 1..=fields {
        let f = h
            .str(&format!("TFORM{i}"))
            .and_then(tform)
            .ok_or_else(|| Error::Malformed(format!("TFORM{i}")))?;
        let (repeat, ty, var) = f;
        let width = match var {
            Some(false) => 8 * repeat.min(1),
            Some(true) => 16 * repeat.min(1),
            None if ty == b'X' => repeat.div_ceil(8),
            None => elem_size(ty)
                .and_then(|s| repeat.checked_mul(s))
                .ok_or_else(|| Error::Malformed("TFORM".into()))?,
        };
        let col = Column { offset, ty, var };
        match h
            .str(&format!("TTYPE{i}"))
            .map(|s| s.trim().to_ascii_uppercase())
            .as_deref()
        {
            Some("COMPRESSED_DATA") if var.is_some() => comp = Some(col),
            Some("GZIP_COMPRESSED_DATA") if var.is_some() => gzip = Some(col),
            Some("UNCOMPRESSED_DATA") if var.is_some() => raw = Some(col),
            Some("ZSCALE") if var.is_none() => zscale = Some(col),
            Some("ZZERO") if var.is_none() => zzero = Some(col),
            Some("ZBLANK") if var.is_none() => zblank = Some(col),
            _ => {}
        }
        offset = offset
            .checked_add(width)
            .ok_or_else(|| Error::Malformed("row too long".into()))?;
    }
    let row_len = h.i64("NAXIS1").unwrap_or(0).max(0) as usize;
    let rows = h.i64("NAXIS2").unwrap_or(0).max(0) as usize;
    if offset > row_len {
        return Err(Error::Malformed("columns wider than the row".into()));
    }
    let heap = h
        .i64("THEAP")
        .map(|t| t.max(0) as usize)
        .unwrap_or(row_len.saturating_mul(rows));
    let tiles = Tiles {
        algorithm,
        quantize,
        tile,
        blocksize,
        bytepix,
        smooth,
        dither0: h.i64("ZDITHER0").unwrap_or(1),
        row_len,
        rows,
        heap,
        comp,
        gzip,
        raw,
        zscale,
        zzero,
        zblank,
        zscale_key: h.f64("ZSCALE"),
        zzero_key: h.f64("ZZERO"),
        zblank_key: h.i64("ZBLANK"),
        slab: SlabCache::default(),
    };
    Ok(ImageDesc {
        bitpix: bitpix as i32,
        axes,
        bzero: if float {
            0.0
        } else {
            h.f64("BZERO").unwrap_or(0.0)
        },
        bscale: if float {
            1.0
        } else {
            h.f64("BSCALE").filter(|s| *s != 0.0).unwrap_or(1.0)
        },
        blank: if float { None } else { h.i64("BLANK") },
        tiles: Some(tiles),
    })
}

fn has_column(h: &Header, name: &str) -> bool {
    let fields = h.i64("TFIELDS").unwrap_or(0).clamp(0, 999);
    (1..=fields).any(|i| {
        h.str(&format!("TTYPE{i}"))
            .is_some_and(|t| t.trim().eq_ignore_ascii_case(name))
    })
}

/// The convention's random sequence: 10000 values of the Park & Miller minimal standard
/// generator (a = 16807, m = 2^31 − 1) from seed 1, divided by m.
fn randoms() -> &'static [f32; N_RANDOM] {
    static R: std::sync::OnceLock<Box<[f32; N_RANDOM]>> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        let (a, m) = (16807.0f64, 2147483647.0f64);
        let mut seed = 1.0f64;
        let mut r = Box::new([0.0f32; N_RANDOM]);
        for v in r.iter_mut() {
            let t = a * seed;
            seed = t - m * (t / m).trunc();
            *v = (seed / m) as f32;
        }
        r
    })
}
const N_RANDOM: usize = 10000;

/// Integer value of a quantised pixel that is undefined (`SUBTRACTIVE_DITHER_2` zero).
const ZERO_VALUE: i64 = -2147483646;

/// What one tile row holds, read from the table.
struct TileRow<'a> {
    comp: Option<&'a [u8]>,
    gzip: Option<&'a [u8]>,
    raw: Option<(&'a [u8], u8)>,
    zscale: Option<f64>,
    zzero: Option<f64>,
    zblank: Option<i64>,
}

fn be_f64(b: &[u8], ty: u8) -> Option<f64> {
    Some(match ty {
        b'D' => f64::from_be_bytes(b.get(..8)?.try_into().ok()?),
        b'E' => f64::from(f32::from_be_bytes(b.get(..4)?.try_into().ok()?)),
        b'J' => f64::from(i32::from_be_bytes(b.get(..4)?.try_into().ok()?)),
        b'K' => i64::from_be_bytes(b.get(..8)?.try_into().ok()?) as f64,
        b'I' => f64::from(i16::from_be_bytes(b.get(..2)?.try_into().ok()?)),
        b'B' => f64::from(*b.first()?),
        _ => return None,
    })
}

impl Tiles {
    fn row<'a>(&self, data: &'a [u8], hdu: &Hdu, r: usize) -> Option<TileRow<'a>> {
        let start = hdu.data_offset.checked_add(r.checked_mul(self.row_len)?)?;
        let row = data.get(start..start.checked_add(self.row_len)?)?;
        let heap = data
            .get(hdu.data_offset.checked_add(self.heap)?..)
            .unwrap_or(&[]);
        let var = |c: Option<Column>| -> Option<(&'a [u8], u8)> {
            let c = c?;
            let (n, off) = match c.var? {
                false => {
                    let d = row.get(c.offset..c.offset + 8)?;
                    (
                        i32::from_be_bytes(d[..4].try_into().ok()?) as i64,
                        i32::from_be_bytes(d[4..].try_into().ok()?) as i64,
                    )
                }
                true => {
                    let d = row.get(c.offset..c.offset + 16)?;
                    (
                        i64::from_be_bytes(d[..8].try_into().ok()?),
                        i64::from_be_bytes(d[8..].try_into().ok()?),
                    )
                }
            };
            if n <= 0 || off < 0 {
                return None;
            }
            let len = (n as usize).checked_mul(elem_size(c.ty)?)?;
            let s = off as usize;
            Some((heap.get(s..s.checked_add(len)?)?, c.ty))
        };
        let fixed = |c: Option<Column>| c.and_then(|c| be_f64(row.get(c.offset..)?, c.ty));
        Some(TileRow {
            comp: var(self.comp).map(|v| v.0),
            gzip: var(self.gzip).map(|v| v.0),
            raw: var(self.raw),
            zscale: fixed(self.zscale).or(self.zscale_key),
            zzero: fixed(self.zzero).or(self.zzero_key),
            zblank: fixed(self.zblank).map(|v| v as i64).or(self.zblank_key),
        })
    }
}

/// A decoded tile before scaling.
enum Decoded {
    Ints(Vec<i64>),
    Floats(Vec<f32>),
}

fn codec(e: CodecError) -> Error {
    Error::Malformed(e.to_string())
}

/// Big-endian samples of `size` bytes as integers (`float` false) or floats.
fn be_values(b: &[u8], size: usize, float: bool, n: usize) -> Result<Decoded> {
    if b.len() < n * size {
        return Err(Error::Malformed("short tile".into()));
    }
    let b = &b[..n * size];
    Ok(match (size, float) {
        (4, true) => Decoded::Floats(
            b.chunks_exact(4)
                .map(|c| f32::from_be_bytes(c.try_into().unwrap()))
                .collect(),
        ),
        (8, true) => Decoded::Floats(
            b.chunks_exact(8)
                .map(|c| f64::from_be_bytes(c.try_into().unwrap()) as f32)
                .collect(),
        ),
        (1, false) => Decoded::Ints(b.iter().map(|&v| i64::from(v)).collect()),
        (2, false) => Decoded::Ints(
            b.chunks_exact(2)
                .map(|c| i64::from(i16::from_be_bytes([c[0], c[1]])))
                .collect(),
        ),
        (4, false) => Decoded::Ints(
            b.chunks_exact(4)
                .map(|c| i64::from(i32::from_be_bytes(c.try_into().unwrap())))
                .collect(),
        ),
        (8, false) => Decoded::Ints(
            b.chunks_exact(8)
                .map(|c| i64::from_be_bytes(c.try_into().unwrap()))
                .collect(),
        ),
        _ => return Err(Error::Malformed("sample size".into())),
    })
}

fn gunzip(b: &[u8], n: usize) -> Result<Vec<u8>> {
    // The inflater wants headroom past the expected size (a final match may be checked
    // against it); the result must still hold at least `n` bytes.
    let cap = n + 1024;
    let mut out = specular_png::inflate_gzip(b, cap)
        .or_else(|_| specular_png::inflate_zlib(b, cap))
        .map_err(|e| Error::Malformed(format!("gzip tile: {e}")))?;
    if out.len() < n {
        return Err(Error::Malformed("short gzip tile".into()));
    }
    out.truncate(n);
    Ok(out)
}

/// GZIP_2: the bytes of each sample were grouped most significant first.
fn unshuffle(b: &[u8], size: usize) -> Vec<u8> {
    let n = b.len() / size;
    let mut out = vec![0u8; n * size];
    for k in 0..size {
        let plane = &b[k * n..(k + 1) * n];
        for (i, &v) in plane.iter().enumerate() {
            out[i * size + k] = v;
        }
    }
    out
}

impl Tiles {
    /// Decodes one tile of `n` pixels with dimensions `dims` (fastest first).
    fn decode(
        &self,
        img: &ImageDesc,
        t: &TileRow<'_>,
        dims: &[usize],
        n: usize,
    ) -> Result<Decoded> {
        let float = img.bitpix < 0;
        let quantised = float && self.quantize != Quantize::Lossless && t.zscale.is_some();
        if let Some(c) = t.comp {
            // Integers of a quantised float tile are 4 bytes; others the image's own size.
            let size = if quantised { 4 } else { img.sample_bytes() };
            let raw_float = float && !quantised;
            return match self.algorithm {
                Algorithm::Rice => {
                    let mut out = vec![0i32; n];
                    compress::rice(c, self.bytepix, self.blocksize, &mut out).map_err(codec)?;
                    Ok(Decoded::Ints(out.into_iter().map(i64::from).collect()))
                }
                Algorithm::Gzip1 | Algorithm::Gzip2 => {
                    let mut b = gunzip(c, n * size)?;
                    if self.algorithm == Algorithm::Gzip2 {
                        b = unshuffle(&b, size);
                    }
                    be_values(&b, size, raw_float, n)
                }
                Algorithm::None => be_values(c, size, raw_float, n),
                Algorithm::Hcompress => {
                    // Check the stream's dimensions against the tile before it allocates.
                    let dim = |o: usize| {
                        c.get(o..o + 4)
                            .map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]) as i64)
                    };
                    match (dim(2), dim(6)) {
                        (Some(a), Some(b)) if a > 0 && b > 0 && (a * b) as usize == n => {}
                        _ => return Err(Error::Malformed("HCOMPRESS tile size".into())),
                    }
                    let d = compress::hcompress(c, self.smooth).map_err(codec)?;
                    if d.nx * d.ny != n || d.data.len() != n {
                        return Err(Error::Malformed("HCOMPRESS tile size".into()));
                    }
                    let _ = dims;
                    Ok(Decoded::Ints(d.data))
                }
                Algorithm::Plio => {
                    let mut out = vec![0i32; n];
                    compress::plio(c, n, &mut out).map_err(codec)?;
                    Ok(Decoded::Ints(out.into_iter().map(i64::from).collect()))
                }
            };
        }
        if let Some(g) = t.gzip {
            let size = img.sample_bytes();
            let b = gunzip(g, n * size)?;
            return be_values(&b, size, float, n);
        }
        if let Some((r, ty)) = t.raw {
            let size = elem_size(ty).unwrap_or(1);
            let fl = matches!(ty, b'E' | b'D');
            return be_values(r, size, fl, n);
        }
        Err(Error::Malformed("tile without data".into()))
    }
}

/// Physical values of a decoded tile (tile row `row`, 0-based).
fn to_physical(
    img: &ImageDesc,
    tiles: &Tiles,
    t: &TileRow<'_>,
    row: usize,
    d: Decoded,
) -> Vec<f32> {
    match d {
        Decoded::Floats(v) => v,
        Decoded::Ints(v) => {
            let float = img.bitpix < 0;
            if float && let Some(scale) = t.zscale {
                let zero = t.zzero.unwrap_or(0.0);
                let blank = t.zblank;
                match tiles.quantize {
                    Quantize::SubtractiveDither1 | Quantize::SubtractiveDither2 => {
                        let r = randoms();
                        let mut iseed = (row as i64 + tiles.dither0.rem_euclid(N_RANDOM as i64) - 1)
                            .rem_euclid(N_RANDOM as i64)
                            as usize;
                        let mut next = (r[iseed] * 500.0) as usize;
                        let d2 = tiles.quantize == Quantize::SubtractiveDither2;
                        v.into_iter()
                            .map(|q| {
                                let out = if blank == Some(q) {
                                    f32::NAN
                                } else if d2 && q == ZERO_VALUE {
                                    0.0
                                } else {
                                    ((q as f64 - f64::from(r[next]) + 0.5) * scale + zero) as f32
                                };
                                next += 1;
                                if next == N_RANDOM {
                                    iseed = (iseed + 1) % N_RANDOM;
                                    next = (r[iseed] * 500.0) as usize;
                                }
                                out
                            })
                            .collect()
                    }
                    _ => v
                        .into_iter()
                        .map(|q| {
                            if blank == Some(q) {
                                f32::NAN
                            } else {
                                (q as f64 * scale + zero) as f32
                            }
                        })
                        .collect(),
                }
            } else {
                let blank = t.zblank.or(img.blank);
                let (z, s) = (img.bzero, img.bscale);
                v.into_iter()
                    .map(|q| {
                        if blank == Some(q) {
                            f32::NAN
                        } else {
                            (z + s * q as f64) as f32
                        }
                    })
                    .collect()
            }
        }
    }
}

/// Reads one plane of a tile-compressed image, top-down.
pub(crate) fn read_plane(
    data: &[u8],
    hdu: &Hdu,
    img: &ImageDesc,
    tiles: &Tiles,
    plane: usize,
) -> Result<Plane> {
    let axes = &img.axes;
    let nd = axes.len();
    let (w, h) = (img.width(), img.height());
    // The plane's position on the axes past the second.
    let mut pos = vec![0usize; nd];
    let mut rest = plane;
    for k in 2..nd {
        pos[k] = rest % axes[k];
        rest /= axes[k];
    }
    let ntiles: Vec<usize> = axes
        .iter()
        .zip(&tiles.tile)
        .map(|(&a, &t)| a.div_ceil(t))
        .collect();
    // Tiles meeting the plane: every tile index on axes 0 and 1, the one containing pos on
    // the others.
    let mut fixed_row = 0usize;
    let mut stride = ntiles[0] * ntiles.get(1).copied().unwrap_or(1);
    for k in 2..nd {
        fixed_row += (pos[k] / tiles.tile[k]) * stride;
        stride *= ntiles[k];
    }
    let (ntx, nty) = (ntiles[0], ntiles.get(1).copied().unwrap_or(1));
    let (tw, th) = (tiles.tile[0], tiles.tile.get(1).copied().unwrap_or(1));
    let jobs: Vec<(usize, usize)> = (0..nty)
        .flat_map(|ty| (0..ntx).map(move |tx| (tx, ty)))
        .collect();
    // The tile's dimensions (clipped at the image's edges).
    let dims_of = |tx: usize, ty: usize| -> Vec<usize> {
        (0..nd)
            .map(|k| {
                let start = match k {
                    0 => tx * tw,
                    1 => ty * th,
                    _ => (pos[k] / tiles.tile[k]) * tiles.tile[k],
                };
                tiles.tile[k].min(axes[k] - start)
            })
            .collect()
    };
    // Tiles spanning more than this plane are decoded whole: keep them for the next plane
    // when they are not too large.
    let depth: usize = dims_of(0, 0).iter().skip(2).product();
    let keep = depth > 1 && (w * h).checked_mul(depth).is_some_and(|n| n <= SLAB_BUDGET);
    // Every tile meeting the plane, decoded whole to f32 (`depth` planes deep), and the plane.
    let _res = specular_core::budget::reserve_n((w * h) as u64, 4 * (depth as u64 + 1))?;
    let slab = match keep.then(|| tiles.slab.get(fixed_row)).flatten() {
        Some(s) => s,
        None => {
            let decoded: Vec<Result<Vec<f32>>> = jobs
                .par_iter()
                .map(|&(tx, ty)| {
                    let row = fixed_row + ty * ntx + tx;
                    if row >= tiles.rows {
                        return Err(Error::Malformed("tile past the table".into()));
                    }
                    let dims = dims_of(tx, ty);
                    let n: usize = dims.iter().product();
                    let t = tiles
                        .row(data, hdu, row)
                        .ok_or_else(|| Error::Malformed("tile row past the end".into()))?;
                    let d = tiles.decode(img, &t, &dims, n)?;
                    let vals = to_physical(img, tiles, &t, row, d);
                    if vals.len() < n {
                        return Err(Error::Malformed("short tile".into()));
                    }
                    Ok(vals)
                })
                .collect();
            let s = Arc::new(decoded);
            if keep {
                tiles.slab.put(fixed_row, s.clone());
            }
            s
        }
    };
    let decoded = jobs.iter().zip(slab.iter()).map(|(&(tx, ty), r)| {
        let vals = r.as_ref().map_err(Clone::clone)?;
        // The tile's 2-D slice at the plane.
        let dims = dims_of(tx, ty);
        let (dw, dh) = (dims[0], dims.get(1).copied().unwrap_or(1));
        let mut off = 0;
        let mut st = dw * dh;
        for k in 2..nd {
            off += (pos[k] - (pos[k] / tiles.tile[k]) * tiles.tile[k]) * st;
            st *= dims[k];
        }
        Ok((tx, ty, &vals[off..off + dw * dh]))
    });
    let mut values = Vec::new();
    values
        .try_reserve_exact(w * h)
        .map_err(|_| Error::TooLarge)?;
    values.resize(w * h, f32::NAN);
    let mut truncated = false;
    for r in decoded {
        let (tx, ty, v) = match r {
            Ok(x) => x,
            Err(Error::Malformed(_)) => {
                truncated = true;
                continue;
            }
            Err(e) => return Err(e),
        };
        let (x0, y0) = (tx * tw, ty * th);
        let dw = tw.min(w - x0);
        let dh = th.min(h - y0);
        for j in 0..dh {
            // FITS row y0 + j goes to display row h - 1 - (y0 + j).
            let dst = (h - 1 - (y0 + j)) * w + x0;
            values[dst..dst + dw].copy_from_slice(&v[j * dw..(j + 1) * dw]);
        }
    }
    Ok(Plane {
        width: w,
        height: h,
        values,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_dither_seed() {
        for seed in ["9223372036854775807", "-9223372036854775808"] {
            let h = Header::from_pairs(
                [
                    ("ZBITPIX", "-32"),
                    ("ZNAXIS", "1"),
                    ("ZNAXIS1", "2"),
                    ("ZCMPTYPE", "'RICE_1'"),
                    ("ZQUANTIZ", "'SUBTRACTIVE_DITHER_1'"),
                    ("ZDITHER0", seed),
                ]
                .map(|(k, v)| (k, v, "")),
            );
            let img = image_desc(&h).unwrap();
            let row = TileRow {
                comp: None,
                gzip: None,
                raw: None,
                zscale: Some(1.0),
                zzero: None,
                zblank: None,
            };
            let v = to_physical(
                &img,
                img.tiles.as_ref().unwrap(),
                &row,
                3,
                Decoded::Ints(vec![1, 2]),
            );
            assert_eq!(v.len(), 2);
        }
    }

    #[test]
    fn random_sequence() {
        // The convention's check value: the 10000th number of the sequence (seed 1043618065).
        let r = randoms();
        let m = 2147483647.0f64;
        // (Stored as f32, as the reference implementation does.)
        assert!((f64::from(r[N_RANDOM - 1]) * m - 1043618065.0).abs() < 128.0);
    }

    #[test]
    fn tforms() {
        assert_eq!(tform("1PB(2048)"), Some((1, b'B', Some(false))));
        assert_eq!(tform("1QI"), Some((1, b'I', Some(true))));
        assert_eq!(tform("PB"), Some((1, b'B', Some(false))));
        assert_eq!(tform("1D"), Some((1, b'D', None)));
        assert_eq!(tform("16X"), Some((16, b'X', None)));
    }

    #[test]
    fn shuffle() {
        assert_eq!(unshuffle(&[1, 3, 2, 4], 2), vec![1, 2, 3, 4]);
    }
}
