//! A small tiled, single-channel TIFF writer (classic or BigTIFF, little-endian, Deflate):
//! the tiles of every level go out as they are made, then the directories, the reduced
//! levels as SubIFDs of the first (the OME-TIFF pyramid layout).

use std::io::{self, Seek, SeekFrom, Write};

/// One level's size, pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub width: u32,
    pub height: u32,
}

const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const LONG8: u16 = 16;
const IFD: u16 = 13;
const IFD8: u16 = 18;

struct Entry {
    tag: u16,
    ty: u16,
    count: u64,
    data: Vec<u8>,
}

fn shorts(tag: u16, v: &[u16]) -> Entry {
    Entry {
        tag,
        ty: SHORT,
        count: v.len() as u64,
        data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}

fn longs(tag: u16, v: &[u32]) -> Entry {
    Entry {
        tag,
        ty: LONG,
        count: v.len() as u64,
        data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}

/// Offsets: LONG8 (BigTIFF) or LONG.
fn offsets(tag: u16, v: &[u64], big: bool, ty_big: u16, ty_small: u16) -> io::Result<Entry> {
    if big {
        return Ok(Entry {
            tag,
            ty: ty_big,
            count: v.len() as u64,
            data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
        });
    }
    let mut data = Vec::with_capacity(v.len() * 4);
    for &x in v {
        let x = u32::try_from(x).map_err(|_| io::Error::other("over 4 GiB in a classic TIFF"))?;
        data.extend_from_slice(&x.to_le_bytes());
    }
    Ok(Entry {
        tag,
        ty: ty_small,
        count: v.len() as u64,
        data,
    })
}

/// The bytes of one directory written at `start`, its overflow data after it, with
/// `next` as the next directory's offset.
fn ifd(mut entries: Vec<Entry>, start: u64, next: u64, big: bool) -> io::Result<Vec<u8>> {
    entries.sort_by_key(|e| e.tag);
    let (head, entry, tail, inline) = if big {
        (8u64, 20u64, 8u64, 8usize)
    } else {
        (2, 12, 4, 4)
    };
    let mut extra_at = start + head + entry * entries.len() as u64 + tail;
    let mut out = Vec::new();
    let mut extra = Vec::new();
    if big {
        out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    } else {
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    }
    for e in &entries {
        out.extend_from_slice(&e.tag.to_le_bytes());
        out.extend_from_slice(&e.ty.to_le_bytes());
        if big {
            out.extend_from_slice(&e.count.to_le_bytes());
        } else {
            let c = u32::try_from(e.count).map_err(|_| io::Error::other("too many values"))?;
            out.extend_from_slice(&c.to_le_bytes());
        }
        if e.data.len() <= inline {
            let mut v = e.data.clone();
            v.resize(inline, 0);
            out.extend_from_slice(&v);
        } else {
            if big {
                out.extend_from_slice(&extra_at.to_le_bytes());
            } else {
                let o = u32::try_from(extra_at).map_err(|_| io::Error::other("over 4 GiB"))?;
                out.extend_from_slice(&o.to_le_bytes());
            }
            extra.extend_from_slice(&e.data);
            extra_at += e.data.len() as u64;
            if extra.len() % 2 == 1 {
                extra.push(0);
                extra_at += 1;
            }
        }
    }
    if big {
        out.extend_from_slice(&next.to_le_bytes());
    } else {
        let n = u32::try_from(next).map_err(|_| io::Error::other("over 4 GiB"))?;
        out.extend_from_slice(&n.to_le_bytes());
    }
    out.extend_from_slice(&extra);
    Ok(out)
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; zlib_rs::compress_bound(data.len())];
    let (c, rc) = zlib_rs::compress_slice(&mut out, data, zlib_rs::DeflateConfig::new(6));
    debug_assert!(rc == zlib_rs::ReturnCode::Ok);
    let n = c.len();
    out.truncate(n);
    out
}

/// Writes `levels` (the first full size) of `bits`-bit grey samples in `tile` × `tile`
/// tiles; `tile_data(level, tx, ty, level size)` gives each tile's little-endian samples,
/// row-major and padded to the full tile. `description` goes in the first directory.
pub fn write<W: Write + Seek>(
    out: &mut W,
    levels: &[Level],
    tile: u32,
    bits: u16,
    description: Option<&str>,
    big: bool,
    mut tile_data: impl FnMut(usize, u32, u32, &Level) -> Vec<u8>,
) -> io::Result<()> {
    let base = out.stream_position()?;
    if big {
        out.write_all(b"II\x2b\0\x08\0\0\0")?;
        out.write_all(&0u64.to_le_bytes())?;
    } else {
        out.write_all(b"II\x2a\0")?;
        out.write_all(&0u32.to_le_bytes())?;
    }
    let mut pos = out.stream_position()? - base;
    let mut tables: Vec<(Vec<u64>, Vec<u64>)> = Vec::new();
    for (l, lv) in levels.iter().enumerate() {
        let (nx, ny) = (lv.width.div_ceil(tile), lv.height.div_ceil(tile));
        let mut offs = Vec::with_capacity((nx * ny) as usize);
        let mut counts = Vec::with_capacity((nx * ny) as usize);
        for ty in 0..ny {
            for tx in 0..nx {
                let z = deflate(&tile_data(l, tx, ty, lv));
                offs.push(pos);
                counts.push(z.len() as u64);
                out.write_all(&z)?;
                pos += z.len() as u64;
                if pos % 2 == 1 {
                    out.write_all(&[0])?;
                    pos += 1;
                }
            }
        }
        tables.push((offs, counts));
    }
    // Directories: the reduced levels first (their offsets go in the full level's SubIFDs).
    let entries = |l: usize, lv: &Level, sub: &[u64]| -> io::Result<Vec<Entry>> {
        let (offs, counts) = &tables[l];
        let mut e = vec![
            longs(254, &[u32::from(l > 0)]),
            longs(256, &[lv.width]),
            longs(257, &[lv.height]),
            shorts(258, &[bits]),
            shorts(259, &[8]),
            shorts(262, &[1]),
            shorts(277, &[1]),
            shorts(284, &[1]),
            longs(322, &[tile]),
            longs(323, &[tile]),
            offsets(324, offs, big, LONG8, LONG)?,
            offsets(325, counts, big, LONG8, LONG)?,
            shorts(339, &[1]),
        ];
        if l == 0 {
            if let Some(d) = description {
                let mut data = d.as_bytes().to_vec();
                data.push(0);
                e.push(Entry {
                    tag: 270,
                    ty: ASCII,
                    count: data.len() as u64,
                    data,
                });
            }
            if !sub.is_empty() {
                e.push(offsets(330, sub, big, IFD8, IFD)?);
            }
        }
        Ok(e)
    };
    let mut sub = Vec::new();
    for (l, lv) in levels.iter().enumerate().skip(1) {
        let start = pos;
        let bytes = ifd(entries(l, lv, &[])?, start, 0, big)?;
        out.write_all(&bytes)?;
        pos += bytes.len() as u64;
        if pos % 2 == 1 {
            out.write_all(&[0])?;
            pos += 1;
        }
        sub.push(start);
    }
    let first = pos;
    let bytes = ifd(entries(0, &levels[0], &sub)?, first, 0, big)?;
    out.write_all(&bytes)?;
    out.seek(SeekFrom::Start(base + if big { 8 } else { 4 }))?;
    if big {
        out.write_all(&first.to_le_bytes())?;
    } else {
        let f =
            u32::try_from(first).map_err(|_| io::Error::other("over 4 GiB in a classic TIFF"))?;
        out.write_all(&f.to_le_bytes())?;
    }
    out.seek(SeekFrom::End(0))?;
    Ok(())
}
