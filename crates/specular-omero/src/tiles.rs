//! Rendered pixels: fetching (through the shared disk cache) and decoding with our own JPEG
//! and PNG decoders, and assembling tiles into one RGBA region, several requests at a time,
//! stopping as soon as the request is cancelled.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;
use specular_remote::{CancelToken, DiskCache};

use crate::api::{Omero, OmeroError, Result};

/// Concurrent tile requests per region (OMERO.web renders each on its own).
pub const TILE_REQUESTS: usize = 6;

/// An RGBA image.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The most pixels a server's tile or region image may have (the largest region asked for).
const MAX_PIXELS: u64 = 1 << 27;

/// Decodes a rendered JPEG or PNG to RGBA.
pub fn decode(bytes: &[u8], cancel: &CancelToken) -> Result<Rgba> {
    let bad = |m: String| OmeroError::Malformed(m);
    // The server's header sizes the decode: refuse a size no request asks for.
    let too_large = |w: u32, h: u32| u64::from(w) * u64::from(h) > MAX_PIXELS;
    if bytes.starts_with(&[0xff, 0xd8]) {
        let info = specular_jpeg::read_info(bytes).map_err(|e| bad(format!("tile: {e}")))?;
        if too_large(info.width, info.height) {
            return Err(bad(format!("tile of {}×{} px", info.width, info.height)));
        }
        let opts = specular_jpeg::DecodeOptions {
            cancel: Some(cancel.flag()),
            parallelism: specular_jpeg::Parallelism::Single,
            ..Default::default()
        };
        let p = specular_jpeg::decode(bytes, &opts).map_err(|e| bad(format!("tile: {e}")))?;
        let data = specular_jpeg::planes_to_rgba(&p);
        Ok(Rgba {
            width: p.width,
            height: p.height,
            data,
        })
    } else if bytes.starts_with(b"\x89PNG") {
        if let (Some(w), Some(h)) = (be32(bytes, 16), be32(bytes, 20))
            && too_large(w, h)
        {
            return Err(bad(format!("tile of {w}×{h} px")));
        }
        let opts = specular_png::DecodeOptions {
            output: specular_png::Output::Rgba8,
            parallelism: specular_png::Parallelism::Single,
            cancel: Some(cancel.flag()),
            ..Default::default()
        };
        let img = specular_png::decode(bytes, &opts).map_err(|e| bad(format!("tile: {e}")))?;
        Ok(Rgba {
            width: img.width,
            height: img.height,
            data: img.to_rgba8(),
        })
    } else if bytes.starts_with(b"<") {
        // An HTML page (a sign-in form) where pixels were asked for.
        Err(OmeroError::SignInRequired)
    } else {
        Err(bad("tile is neither JPEG nor PNG".into()))
    }
}

/// Bytes of `url`, from the cache under `key` when there, else fetched and kept.
pub fn fetch(
    omero: &Omero,
    cache: Option<&Arc<DiskCache>>,
    key: &str,
    url: &str,
    cancel: &CancelToken,
) -> Result<Vec<u8>> {
    if let Some(c) = cache
        && let Some(p) = c.get(key)
        && let Ok(b) = std::fs::read(p)
        && !b.is_empty()
    {
        return Ok(b);
    }
    let bytes = omero.get_bytes(url, cancel)?;
    if let Some(c) = cache
        && let Err(e) = c.insert_bytes(key, &bytes)
    {
        specular_log::debug!(target: specular_log::CACHE, "omero: cache write failed: {e}");
    }
    Ok(bytes)
}

/// An image's thumbnail (the server's saved rendering, longest side `size`), through the
/// cache.
pub fn thumbnail(
    omero: &Omero,
    cache: Option<&Arc<DiskCache>>,
    image: u64,
    size: u32,
    cancel: &CancelToken,
) -> Result<Rgba> {
    let key = DiskCache::key(&["omero", omero.base(), "thumb", &format!("{image}-{size}")]);
    let url = crate::render::thumbnail_url(image, size);
    let bytes = fetch(omero, cache, &key, &url, cancel)?;
    decode(&bytes, cancel)
}

/// One piece to fetch and place: its URL, cache key, and where its pixels go in the output
/// (offset of the piece's top-left, and the size expected).
#[derive(Clone, Debug)]
pub struct Piece {
    pub url: String,
    pub key: String,
    pub x: i64,
    pub y: i64,
    pub w: u32,
    pub h: u32,
}

/// Fetches `pieces` (several at a time) and pastes them into a `width × height` RGBA image;
/// a piece that comes back larger than expected is cropped, a smaller one leaves its edge
/// filled from its last row and column. Returns the image and whether a piece was short.
pub fn assemble(
    omero: &Omero,
    cache: Option<&Arc<DiskCache>>,
    pieces: &[Piece],
    width: u32,
    height: u32,
    cancel: &CancelToken,
) -> Result<(Rgba, bool)> {
    let out = Mutex::new(Rgba {
        width,
        height,
        data: vec![0; width as usize * height as usize * 4],
    });
    let short = Mutex::new(false);
    let err: Mutex<Option<OmeroError>> = Mutex::new(None);
    let next = AtomicUsize::new(0);
    let work = || {
        loop {
            if cancel.is_cancelled() || err.lock().is_some() {
                return;
            }
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(p) = pieces.get(i) else { return };
            let r = fetch(omero, cache, &p.key, &p.url, cancel).and_then(|b| decode(&b, cancel));
            match r {
                Ok(tile) => {
                    let cut = paste(&mut out.lock(), &tile, p);
                    if cut {
                        *short.lock() = true;
                    }
                }
                Err(e) => {
                    err.lock().get_or_insert(e);
                    return;
                }
            }
        }
    };
    let threads = TILE_REQUESTS.min(pieces.len());
    if threads <= 1 {
        work();
    } else {
        std::thread::scope(|s| {
            for i in 0..threads {
                specular_thread::spawn_scoped(s, format!("omero-tile-{i}"), work);
            }
        });
    }
    if cancel.is_cancelled() {
        return Err(OmeroError::Cancelled);
    }
    if let Some(e) = err.into_inner() {
        return Err(e);
    }
    Ok((out.into_inner(), short.into_inner()))
}

/// Pastes `tile` at the piece's place; true when it was smaller than expected.
fn paste(out: &mut Rgba, tile: &Rgba, p: &Piece) -> bool {
    if tile.width == 0 || tile.height == 0 {
        return true;
    }
    let short = tile.width < p.w || tile.height < p.h;
    for ty in 0..p.h {
        let oy = p.y + i64::from(ty);
        if oy < 0 || oy >= i64::from(out.height) {
            continue;
        }
        let sy = ty.min(tile.height - 1) as usize;
        for tx in 0..p.w {
            let ox = p.x + i64::from(tx);
            if ox < 0 || ox >= i64::from(out.width) {
                continue;
            }
            let sx = tx.min(tile.width - 1) as usize;
            let s = (sy * tile.width as usize + sx) * 4;
            let d = (oy as usize * out.width as usize + ox as usize) * 4;
            out.data[d..d + 4].copy_from_slice(&tile.data[s..s + 4]);
        }
    }
    short
}

/// Box-filters `src` down by `factor` (a power of two) into `w × h` (at most the reduced
/// size; pixels past the source's edge average what is there).
pub fn reduce(src: &Rgba, factor: u32, w: u32, h: u32) -> Rgba {
    if factor <= 1 && src.width == w && src.height == h {
        return src.clone();
    }
    let f = factor.max(1) as usize;
    let mut data = vec![0u8; w as usize * h as usize * 4];
    let (sw, sh) = (src.width as usize, src.height as usize);
    for y in 0..h as usize {
        let y0 = (y * f).min(sh.saturating_sub(1));
        let y1 = ((y + 1) * f).min(sh).max(y0 + 1);
        for x in 0..w as usize {
            let x0 = (x * f).min(sw.saturating_sub(1));
            let x1 = ((x + 1) * f).min(sw).max(x0 + 1);
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                let row = &src.data[(sy * sw + x0) * 4..(sy * sw + x1) * 4];
                for px in row.chunks_exact(4) {
                    for c in 0..4 {
                        acc[c] += u32::from(px[c]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let d = (y * w as usize + x) * 4;
            for c in 0..4 {
                data[d + c] = ((acc[c] + n / 2) / n) as u8;
            }
        }
    }
    Rgba {
        width: w,
        height: h,
        data,
    }
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server JPEG whose frame header says 65535 × 65535 (4 GiB of RGBA) and a PNG saying
    /// the same: refused from the header.
    #[test]
    fn huge_server_images_are_refused_from_the_header() {
        let mut j = vec![0xff, 0xd8];
        j.extend_from_slice(&[0xff, 0xdb, 0, 67, 0]);
        j.extend_from_slice(&[1; 64]);
        j.extend_from_slice(&[0xff, 0xc0, 0, 11, 8, 0xff, 0xff, 0xff, 0xff, 1, 1, 0x11, 0]);
        let mut one = vec![1u8];
        one.extend_from_slice(&[0; 15]);
        for class in [0x00, 0x10] {
            j.extend_from_slice(&[0xff, 0xc4, 0, 20, class]);
            j.extend_from_slice(&one);
            j.push(0);
        }
        j.extend_from_slice(&[0xff, 0xda, 0, 8, 1, 1, 0x00, 0, 63, 0]);
        j.resize(j.len() + (1 << 20), 0);
        j.extend_from_slice(&[0xff, 0xd9]);
        let cancel = CancelToken::new();
        assert!(matches!(decode(&j, &cancel), Err(OmeroError::Malformed(_))));
        let mut p = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        p.extend_from_slice(&[0xff; 8]);
        p.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        assert!(matches!(decode(&p, &cancel), Err(OmeroError::Malformed(_))));
    }

    #[test]
    fn reduce_averages_boxes() {
        let src = Rgba {
            width: 3,
            height: 2,
            data: [10, 20, 30, 255, 30, 40, 50, 255, 100, 100, 100, 255]
                .repeat(2)
                .to_vec(),
        };
        let r = reduce(&src, 2, 2, 1);
        assert_eq!(&r.data[..4], &[20, 30, 40, 255]);
        assert_eq!(&r.data[4..], &[100, 100, 100, 255]);
    }

    #[test]
    fn paste_crops_and_extends() {
        let mut out = Rgba {
            width: 4,
            height: 2,
            data: vec![0; 32],
        };
        let tile = Rgba {
            width: 1,
            height: 1,
            data: vec![9, 9, 9, 255],
        };
        let p = Piece {
            url: String::new(),
            key: String::new(),
            x: 2,
            y: 0,
            w: 3,
            h: 2,
        };
        assert!(paste(&mut out, &tile, &p));
        assert_eq!(&out.data[8..16], &[9, 9, 9, 255, 9, 9, 9, 255]);
        assert_eq!(&out.data[..8], &[0; 8]);
    }
}
