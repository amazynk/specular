//! The tiled storage managers that hold image pixels: `TiledCellStMan` (one hypercube per
//! row, what `PagedImage` writes) and `TiledShapeStMan` (rows of one shape stacked into a
//! hypercube with the row as its last axis).
//!
//! The header (`table.f<n>`) lists the data files (`table.f<n>_TSM<k>`) and the hypercubes:
//! shape, tile shape, data file and offset. A cube's tiles follow each other from its
//! offset, first axis fastest, each a full tile of `tile shape` elements (first axis
//! fastest) in the manager's byte order; Bool tiles are packed eight to a byte.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::aipsio::Reader;
use crate::record::{dtype, read_record};
use crate::{Error, Result};

/// The largest header read.
pub const MAX_HEADER: u64 = 64 << 20;
/// The most dimensions a cube may have.
const MAX_DIMS: usize = 16;
/// The largest tile, in bytes.
const MAX_TILE: u64 = 1 << 30;
/// The most cubes.
const MAX_CUBES: usize = 1 << 20;

/// One hypercube.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cube {
    pub shape: Vec<usize>,
    pub tile: Vec<usize>,
    /// Index of the data file; `None` when the cube has no data yet.
    pub file: Option<usize>,
    pub offset: u64,
}

impl Cube {
    /// Tiles along each axis.
    fn tiles(&self) -> Vec<usize> {
        self.shape
            .iter()
            .zip(&self.tile)
            .map(|(&s, &t)| s.div_ceil(t))
            .collect()
    }

    /// Elements per tile; `None` past u64 (tile shapes are the file's own).
    fn tile_elements(&self) -> Option<u64> {
        self.tile
            .iter()
            .try_fold(1u64, |a, &b| a.checked_mul(b as u64))
    }
}

/// A tiled storage manager.
#[derive(Clone, Debug)]
pub struct Tsm {
    /// `table.f<n>`, the stem of the data files.
    stem: PathBuf,
    pub kind: Kind,
    pub big_endian: bool,
    /// Element type (see [`dtype`]).
    pub dtype: i32,
    pub cubes: Vec<Cube>,
    /// Recorded length of each data file, `None` for files that do not exist.
    pub files: Vec<Option<u64>>,
    /// TiledShapeStMan: for each run of rows its last row, cube, and position of that last
    /// row along the cube's last axis.
    row_map: Vec<(u64, usize, u64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cell,
    Shape,
}

/// Bytes per element; 0 for Bool (bits).
pub fn element_size(dt: i32) -> Option<usize> {
    use dtype::*;
    Some(match dt {
        BOOL => 0,
        CHAR | UCHAR => 1,
        SHORT | USHORT => 2,
        INT | UINT | FLOAT => 4,
        DOUBLE | COMPLEX | INT64 => 8,
        DCOMPLEX => 16,
        _ => return None,
    })
}

fn usize_dims(v: &[i64], what: &str) -> Result<Vec<usize>> {
    if v.len() > MAX_DIMS {
        return Err(Error::Unsupported(format!(
            "{what} of {} dimensions",
            v.len()
        )));
    }
    v.iter()
        .map(|&x| usize::try_from(x).map_err(|_| Error::Malformed(format!("negative {what}"))))
        .collect()
}

impl Tsm {
    /// Reads the header `table.f<seqnr>` of the table in `dir`.
    pub fn open(dir: &Path, seqnr: u32) -> Result<Tsm> {
        let path = dir.join(format!("table.f{seqnr}"));
        let data = crate::read_capped(&path, MAX_HEADER)?;
        let mut t = Tsm::parse(&data)?;
        t.stem = path;
        Ok(t)
    }

    pub fn parse(data: &[u8]) -> Result<Tsm> {
        let mut r = Reader::new(data);
        let (name, _, outer) = {
            let top = r.u32()?;
            if top != 0xbebe_bebe {
                return Err(Error::Malformed(
                    "storage manager header without magic".into(),
                ));
            }
            r.open_any()?
        };
        let kind = match name.as_str() {
            "TiledCellStMan" => Kind::Cell,
            "TiledShapeStMan" => Kind::Shape,
            other => {
                return Err(Error::Unsupported(format!("storage manager {other}")));
            }
        };
        if kind == Kind::Cell {
            r.iposition()?; // default tile shape
        }
        let mut t = read_tiled(&mut r, kind)?;
        if kind == Kind::Shape {
            r.iposition()?; // default tile shape
            let used = r.u32()? as usize;
            let rows = r.block_u32()?;
            let cubes = r.block_u32()?;
            let pos = r.block_u32()?;
            if used > rows.len() || used > cubes.len() || used > pos.len() {
                return Err(Error::Malformed("row map shorter than used".into()));
            }
            t.row_map = (0..used)
                .map(|i| (u64::from(rows[i]), cubes[i] as usize, u64::from(pos[i])))
                .collect();
        }
        r.close(outer);
        Ok(t)
    }

    /// The cube holding `row` and, for a TiledShapeStMan, the row's position along the
    /// cube's last axis.
    pub fn locate(&self, row: u64) -> Result<(&Cube, Option<usize>)> {
        let missing = || Error::Malformed(format!("no hypercube for row {row}"));
        match self.kind {
            Kind::Cell => {
                let c = self
                    .cubes
                    .get(usize::try_from(row).map_err(|_| missing())?)
                    .ok_or_else(missing)?;
                Ok((c, None))
            }
            Kind::Shape => {
                let &(last, cube, pos) = self
                    .row_map
                    .iter()
                    .find(|(last, _, _)| *last >= row)
                    .ok_or_else(missing)?;
                let c = self.cubes.get(cube).ok_or_else(missing)?;
                let p = pos
                    .checked_add(row)
                    .and_then(|p| p.checked_sub(last))
                    .and_then(|p| usize::try_from(p).ok())
                    .filter(|&p| c.shape.last().is_some_and(|&n| p < n))
                    .ok_or_else(missing)?;
                Ok((c, Some(p)))
            }
        }
    }

    /// Bytes of one tile of `cube` in the data file.
    fn bucket(&self, cube: &Cube) -> Result<u64> {
        let n = cube.tile_elements().ok_or(Error::TooLarge)?;
        let size = element_size(self.dtype)
            .ok_or_else(|| Error::Unsupported(format!("{} pixels", dtype::name(self.dtype))))?;
        let b = if size == 0 {
            n.div_ceil(8)
        } else {
            n.checked_mul(size as u64).ok_or(Error::TooLarge)?
        };
        if b > MAX_TILE {
            return Err(Error::TooLarge);
        }
        Ok(b)
    }

    /// Checks that the data file of `cube` holds all of its tiles.
    pub fn check(&self, cube: &Cube) -> Result<()> {
        let Some(f) = cube.file else {
            return Err(Error::Malformed("the image has no data".into()));
        };
        let tiles = cube
            .tiles()
            .iter()
            .try_fold(1u64, |a, &b| a.checked_mul(b as u64))
            .ok_or(Error::TooLarge)?;
        let need = tiles
            .checked_mul(self.bucket(cube)?)
            .and_then(|n| n.checked_add(cube.offset))
            .ok_or(Error::TooLarge)?;
        let path = self.data_path(f);
        let len = std::fs::metadata(&path)
            .map_err(|e| Error::Io(format!("{}: {e}", path.display())))?
            .len();
        if len < need {
            return Err(Error::Truncated);
        }
        Ok(())
    }

    fn data_path(&self, file: usize) -> PathBuf {
        let mut name = self.stem.as_os_str().to_owned();
        name.push(format!("_TSM{file}"));
        PathBuf::from(name)
    }

    /// The elements of the box `start .. start + count` of `cube` (first axis fastest),
    /// each decoded by `dec(bytes, index)` from the bytes read and the element's index in
    /// them.
    pub fn region<T: Copy>(
        &self,
        cube: &Cube,
        start: &[usize],
        count: &[usize],
        fill: T,
        dec: impl Fn(&[u8], usize) -> T,
    ) -> Result<Vec<T>> {
        let nd = cube.shape.len();
        if start.len() != nd
            || count.len() != nd
            || (0..nd).any(|i| {
                start[i]
                    .checked_add(count[i])
                    .is_none_or(|e| e > cube.shape[i])
            })
        {
            return Err(Error::Malformed("region outside the hypercube".into()));
        }
        let total = count
            .iter()
            .try_fold(1usize, |a, &b| a.checked_mul(b))
            .ok_or(Error::TooLarge)?;
        if total > crate::MAX_ELEMENTS {
            return Err(Error::TooLarge);
        }
        // Up to 8 times the stored size (a mask: one bit stored, a byte out), so reserved
        // from the memory budget before it is allocated.
        let _held = specular_core::budget::reserve_n(total as u64, size_of::<T>() as u64)?;
        let mut out = vec![fill; total];
        if total == 0 {
            return Ok(out);
        }
        let Some(file_nr) = cube.file else {
            return Err(Error::Malformed("the image has no data".into()));
        };
        let path = self.data_path(file_nr);
        let mut file =
            File::open(&path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
        let bucket = self.bucket(cube)?;
        let bits = element_size(self.dtype) == Some(0);
        let size = element_size(self.dtype).unwrap_or(1).max(1);
        let tiles = cube.tiles();
        // Range of tiles along each axis.
        let first: Vec<usize> = (0..nd).map(|i| start[i] / cube.tile[i]).collect();
        let last: Vec<usize> = (0..nd)
            .map(|i| (start[i] + count[i] - 1) / cube.tile[i])
            .collect();
        let mut t = first.clone();
        let mut buf = Vec::new();
        loop {
            // This tile's number and the box's part in it (tile-local coordinates).
            let mut number = 0u64;
            for i in (0..nd).rev() {
                number = number
                    .checked_mul(tiles[i] as u64)
                    .and_then(|n| n.checked_add(t[i] as u64))
                    .ok_or(Error::TooLarge)?;
            }
            let lo: Vec<usize> = (0..nd)
                .map(|i| start[i].max(t[i] * cube.tile[i]) - t[i] * cube.tile[i])
                .collect();
            let hi: Vec<usize> = (0..nd)
                .map(|i| (start[i] + count[i]).min((t[i] + 1) * cube.tile[i]) - t[i] * cube.tile[i])
                .collect();
            // Elements of the tile spanned, in tile order.
            let local = |p: &[usize]| -> usize {
                let mut k = 0;
                for i in (0..nd).rev() {
                    k = k * cube.tile[i] + p[i];
                }
                k
            };
            let hi_in: Vec<usize> = hi.iter().map(|&h| h - 1).collect();
            let (e0, e1) = (local(&lo), local(&hi_in) + 1);
            let e0 = if bits { e0 / 8 * 8 } else { e0 };
            let (b0, b1) = if bits {
                ((e0 / 8) as u64, e1.div_ceil(8) as u64)
            } else {
                ((e0 * size) as u64, (e1 * size) as u64)
            };
            let at = cube
                .offset
                .checked_add(number.checked_mul(bucket).ok_or(Error::TooLarge)?)
                .and_then(|a| a.checked_add(b0))
                .ok_or(Error::TooLarge)?;
            let n = usize::try_from(b1 - b0).map_err(|_| Error::TooLarge)?;
            buf.resize(n, 0);
            file.seek(SeekFrom::Start(at))
                .and_then(|_| file.read_exact(&mut buf))
                .map_err(|e| match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => Error::Truncated,
                    _ => Error::Io(format!("{}: {e}", path.display())),
                })?;
            // Walk the box's part in this tile.
            let mut p = lo.clone();
            'walk: loop {
                let k = local(&p) - e0;
                let mut o = 0usize;
                for i in (0..nd).rev() {
                    o = o * count[i] + (t[i] * cube.tile[i] + p[i] - start[i]);
                }
                out[o] = dec(&buf, k);
                for i in 0..nd {
                    p[i] += 1;
                    if p[i] < hi[i] {
                        continue 'walk;
                    }
                    p[i] = lo[i];
                }
                break;
            }
            // Next tile.
            let mut i = 0;
            loop {
                if i == nd {
                    return Ok(out);
                }
                t[i] += 1;
                if t[i] <= last[i] {
                    break;
                }
                t[i] = first[i];
                i += 1;
            }
        }
    }
}

fn read_tiled(r: &mut Reader, kind: Kind) -> Result<Tsm> {
    let (version, outer) = r.open(false, &["TiledStMan"])?;
    // Version 1 headers predate the byte order flag: their data are big-endian.
    let big_endian = if version >= 2 { r.bool()? } else { true };
    let _seqnr = r.u32()?;
    let _rows = r.u32()?;
    let ncol = r.count(4)?;
    let mut types = Vec::with_capacity(ncol);
    for _ in 0..ncol {
        types.push(r.i32()?);
    }
    let _hypercolumn = r.string()?;
    let _cache = r.u32()?;
    let _ndim = r.u32()?;
    let nfile = r.count(1)?;
    let mut files = Vec::with_capacity(nfile);
    for _ in 0..nfile {
        if r.bool()? {
            let v = r.u32()?;
            let _seq = r.u32()?;
            let len = if v >= 2 {
                r.u64()?
            } else {
                u64::from(r.u32()?)
            };
            files.push(Some(len));
        } else {
            files.push(None);
        }
    }
    let ncube = r.count(8)?;
    if ncube > MAX_CUBES {
        return Err(Error::TooLarge);
    }
    let mut cubes = Vec::with_capacity(ncube);
    for _ in 0..ncube {
        let v = r.u32()?;
        let _values = read_record(r)?;
        let _extensible = r.bool()?;
        let _nd = r.u32()?;
        let shape = usize_dims(&r.iposition()?, "cube shape")?;
        let tile = usize_dims(&r.iposition()?, "tile shape")?;
        let file = r.i32()?;
        let offset = if v >= 2 {
            r.u64()?
        } else {
            u64::from(r.u32()?)
        };
        let file = usize::try_from(file).ok();
        if file.is_some() {
            if shape.len() != tile.len() || tile.contains(&0) {
                return Err(Error::Malformed("tile shape does not fit the cube".into()));
            }
            if file.is_none_or(|f| files.get(f).is_none_or(Option::is_none)) {
                return Err(Error::Malformed("hypercube in a missing data file".into()));
            }
        }
        cubes.push(Cube {
            shape,
            tile,
            file,
            offset,
        });
    }
    r.close(outer);
    let dtype = match types.as_slice() {
        [t] => *t,
        _ => {
            return Err(Error::Unsupported(
                "tiled storage with several data columns".into(),
            ));
        }
    };
    Ok(Tsm {
        stem: PathBuf::new(),
        kind,
        big_endian,
        dtype,
        cubes,
        files,
        row_map: Vec::new(),
    })
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn tile_shape_product_overflows() {
        // Tile shapes from the header whose product passes u64.
        let cube = Cube {
            shape: vec![1 << 40, 1 << 40],
            tile: vec![1 << 40, 1 << 40],
            file: Some(0),
            offset: 0,
        };
        let t = Tsm {
            stem: PathBuf::new(),
            kind: Kind::Cell,
            big_endian: false,
            dtype: dtype::FLOAT,
            cubes: vec![cube.clone()],
            files: vec![Some(0)],
            row_map: vec![(u64::MAX, 0, u64::MAX)],
        };
        assert!(matches!(t.bucket(&cube), Err(Error::TooLarge)));
        assert!(t.region(&cube, &[0, 0], &[1, 1], 0f32, |_, _| 0.0).is_err());
        let shape = Tsm {
            kind: Kind::Shape,
            ..t
        };
        assert!(shape.locate(5).is_err());
    }
}
