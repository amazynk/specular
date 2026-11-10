//! Arc/Info binary grids: a folder of `.adf` files. `hdr.adf` (`GRID1.2`) gives the cell
//! type, cell size and the block layout; `dblbnd.adf` the bounds; `sta.adf` the statistics;
//! `prj.adf` the projection; `w001001x.adf` indexes the blocks of `w001001.adf`. Integer
//! blocks are compressed in one of a dozen ways (constant, packed 1/4/8/16/32-bit, run
//! lengths, literal and no-data runs) above a per-block minimum; float blocks are raw
//! big-endian `f32`. A `<name>.clr` colour file beside it colours the values.

use crate::crs::Crs;
use crate::geotiff::Affine2;

/// The integer no-data value.
pub const NODATA_INT: i32 = -2_147_483_647;
/// The float no-data value (`-FLT_MAX`).
pub const NODATA_FLOAT: f32 = -f32::MAX;
/// Most cells in a grid.
const MAX_CELLS: u64 = 1 << 30;

/// A grid's header.
#[derive(Clone, Debug)]
pub struct Grid {
    pub width: u32,
    pub height: u32,
    /// Integer cells (else 32-bit float).
    pub integer: bool,
    pub compressed: bool,
    pub cell: (f64, f64),
    /// Lower left and upper right corners.
    pub bounds: [f64; 4],
    pub blocks_per_row: u32,
    pub blocks_per_column: u32,
    pub block_w: u32,
    pub block_h: u32,
    /// Minimum, maximum, mean and standard deviation.
    pub stats: Option<[f64; 4]>,
    pub crs: Option<Crs>,
    /// The projection file's text.
    pub prj: Option<String>,
    /// Value to colour, from the `.clr` file.
    pub palette: Vec<(i64, [u8; 3])>,
}

/// Whether bytes start a grid's `hdr.adf`.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"GRID1.2\0")
}

fn be_i32(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be_f64(b: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// A `.clr` file: `value red green blue` lines (`#` comments).
pub fn parse_clr(text: &str) -> Vec<(i64, [u8; 3])> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let v: Vec<i64> = l
                .split_whitespace()
                .take(4)
                .map(|s| s.parse().ok())
                .collect::<Option<_>>()?;
            if v.len() != 4 {
                return None;
            }
            let c = |i: usize| u8::try_from(v[i]).ok();
            Some((v[0], [c(1)?, c(2)?, c(3)?]))
        })
        .take(1 << 20)
        .collect()
}

/// Reads a grid's header. `file` returns one of the folder's files by its lower-case name
/// (`hdr.adf`); `clr` is the colour file, when there is one.
pub fn open(file: &dyn Fn(&str) -> Option<Vec<u8>>, clr: Option<&str>) -> Result<Grid, String> {
    let hdr = file("hdr.adf").ok_or("no hdr.adf in the grid's folder")?;
    if !sniff(&hdr) || hdr.len() < 308 {
        return Err("hdr.adf is not a grid header".into());
    }
    let short = || "hdr.adf is too short".to_string();
    let cell_type = be_i32(&hdr, 16).ok_or_else(short)?;
    let integer = match cell_type {
        1 => true,
        2 => false,
        t => return Err(format!("cell type {t} is not supported")),
    };
    let compressed = be_i32(&hdr, 20).ok_or_else(short)? == 0;
    let cell = (
        be_f64(&hdr, 256).ok_or_else(short)?,
        be_f64(&hdr, 264).ok_or_else(short)?,
    );
    let pos = |at: usize| {
        be_i32(&hdr, at)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| (1..=1 << 20).contains(v))
            .ok_or_else(|| "hdr.adf: block layout".to_string())
    };
    let (blocks_per_row, blocks_per_column, block_w, block_h) =
        (pos(288)?, pos(292)?, pos(296)?, pos(304)?);
    let bnd = file("dblbnd.adf").ok_or("no dblbnd.adf in the grid's folder")?;
    let b = |i: usize| be_f64(&bnd, i * 8).ok_or_else(|| "dblbnd.adf is too short".to_string());
    let bounds = [b(0)?, b(1)?, b(2)?, b(3)?];
    if !(cell.0 > 0.0 && cell.1 > 0.0) || !cell.0.is_finite() || !cell.1.is_finite() {
        return Err("cell size".into());
    }
    let count = |span: f64, c: f64| {
        let n = ((span + 0.5 * c) / c).floor();
        (n.is_finite() && n >= 1.0 && n <= f64::from(u32::MAX)).then_some(n as u32)
    };
    let width = count(bounds[2] - bounds[0], cell.0).ok_or("grid bounds")?;
    let height = count(bounds[3] - bounds[1], cell.1).ok_or("grid bounds")?;
    if u64::from(width) * u64::from(height) > MAX_CELLS {
        return Err("the grid is too large".into());
    }
    let stats = file("sta.adf").and_then(|s| {
        let v = |i: usize| be_f64(&s, i * 8);
        Some([v(0)?, v(1)?, v(2)?, v(3).unwrap_or(f64::NAN)])
    });
    let prj = file("prj.adf").map(|p| String::from_utf8_lossy(&p).into_owned());
    let crs = prj.as_deref().and_then(crate::worldfile::parse_prj);
    Ok(Grid {
        width,
        height,
        integer,
        compressed,
        cell,
        bounds,
        blocks_per_row,
        blocks_per_column,
        block_w,
        block_h,
        stats,
        crs,
        prj,
        palette: clr.map(parse_clr).unwrap_or_default(),
    })
}

impl Grid {
    /// Pixel corner to map.
    pub fn transform(&self) -> Affine2 {
        Affine2 {
            a: self.cell.0,
            b: 0.0,
            c: self.bounds[0],
            d: 0.0,
            e: -self.cell.1,
            f: self.bounds[3],
        }
    }

    /// The header facts for the info panel.
    pub fn facts(&self) -> Vec<(String, String)> {
        let mut v = vec![
            (
                "Cell type".into(),
                if self.integer { "integer" } else { "float" }.into(),
            ),
            (
                "Cell size".into(),
                format!("{} × {}", self.cell.0, self.cell.1),
            ),
            (
                "Blocks".into(),
                format!(
                    "{} × {} of {} × {} cells",
                    self.blocks_per_row, self.blocks_per_column, self.block_w, self.block_h
                ),
            ),
            (
                "Bounds".into(),
                format!(
                    "{}, {} – {}, {}",
                    self.bounds[0], self.bounds[1], self.bounds[2], self.bounds[3]
                ),
            ),
        ];
        if let Some(s) = self.stats {
            v.push(("Minimum".into(), s[0].to_string()));
            v.push(("Maximum".into(), s[1].to_string()));
            v.push(("Mean".into(), s[2].to_string()));
            if s[3].is_finite() {
                v.push(("Standard deviation".into(), s[3].to_string()));
            }
        }
        if !self.palette.is_empty() {
            v.push(("Colours".into(), self.palette.len().to_string()));
        }
        if let Some(p) = &self.prj {
            for l in p.lines().map(str::trim).filter(|l| !l.is_empty()).take(64) {
                let (k, val) = l.split_once(char::is_whitespace).unwrap_or((l, ""));
                v.push((format!("prj {k}"), val.trim().to_owned()));
            }
        }
        v
    }

    /// The cells, row 0 the top, no data NaN. `file` as for [`open`]. True when blocks were
    /// missing, damaged or of an unknown kind.
    pub fn values(&self, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> (Vec<f32>, bool) {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![f32::NAN; w * h];
        let (bw, bh) = (self.block_w as usize, self.block_h as usize);
        let tile_w = self.blocks_per_row as usize * bw;
        let tile_h = self.blocks_per_column as usize * bh;
        let mut bad = false;
        for ty in 0..h.div_ceil(tile_h) {
            for tx in 0..w.div_ceil(tile_w) {
                let name = if tx == 0 && ty == 0 {
                    "w001001".to_string()
                } else {
                    format!("z{:03}{:03}", ty + 1, tx + 1)
                };
                let (Some(data), Some(index)) =
                    (file(&format!("{name}.adf")), file(&format!("{name}x.adf")))
                else {
                    bad = true;
                    continue;
                };
                let origin = (tx * tile_w, ty * tile_h);
                bad |= self.tile(&data, &index, origin, &mut out);
            }
        }
        (out, bad)
    }

    /// Decodes one tile file's blocks into `out`; true when any was bad.
    fn tile(&self, data: &[u8], index: &[u8], origin: (usize, usize), out: &mut [f32]) -> bool {
        let (w, h) = (self.width as usize, self.height as usize);
        let (bw, bh) = (self.block_w as usize, self.block_h as usize);
        let per_row = self.blocks_per_row as usize;
        let n = bw * bh;
        let mut bad = false;
        let mut cells = vec![f32::NAN; n];
        let entries = index.len().saturating_sub(100) / 8;
        for i in 0..entries.min(per_row * self.blocks_per_column as usize) {
            let at = 100 + i * 8;
            let (Some(off), Some(size)) = (be_i32(index, at), be_i32(index, at + 4)) else {
                break;
            };
            let (bx, by) = (i % per_row, i / per_row);
            let (x0, y0) = (origin.0 + bx * bw, origin.1 + by * bh);
            if x0 >= w || y0 >= h {
                continue;
            }
            cells.fill(f32::NAN);
            if size > 0 {
                let start = (off as usize).saturating_mul(2);
                let len = (size as usize).saturating_mul(2);
                let block = data
                    .get(start..start.saturating_add(2))
                    .filter(|p| u16::from_be_bytes([p[0], p[1]]) as usize * 2 == len)
                    .and_then(|_| data.get(start + 2..start + 2 + len));
                match block {
                    Some(b) => bad |= !self.block(b, &mut cells),
                    None => bad = true,
                }
            }
            for y in 0..bh.min(h - y0) {
                let row = &cells[y * bw..y * bw + bw.min(w - x0)];
                let o = (y0 + y) * w + x0;
                out[o..o + row.len()].copy_from_slice(row);
            }
        }
        if entries < per_row * self.blocks_per_column as usize {
            // Blocks past the index's end lie outside the grid only when the grid is small.
            let rows = entries.div_ceil(per_row.max(1)) * bh;
            bad |= origin.1 + rows < h.min(origin.1 + self.blocks_per_column as usize * bh);
        }
        bad
    }

    /// One block's cells; false when it is damaged or of an unknown kind (cells left NaN).
    fn block(&self, b: &[u8], cells: &mut [f32]) -> bool {
        if !self.integer {
            for (c, v) in cells.iter_mut().zip(b.chunks_exact(4)) {
                let f = f32::from_be_bytes([v[0], v[1], v[2], v[3]]);
                *c = if f == NODATA_FLOAT { f32::NAN } else { f };
            }
            return b.len() >= cells.len() * 4;
        }
        let Some((&kind, rest)) = b.split_first() else {
            return false;
        };
        let Some((&min_size, rest)) = rest.split_first() else {
            return false;
        };
        let min_size = usize::from(min_size);
        if min_size > 4 || rest.len() < min_size {
            return false;
        }
        let mut min: i64 = 0;
        for (i, &x) in rest[..min_size].iter().enumerate() {
            min = (min << 8) | i64::from(x);
            if i == 0 && x & 0x80 != 0 {
                min -= 256;
            }
        }
        let d = &rest[min_size..];
        let n = cells.len();
        let mut put = |i: usize, v: i64| {
            if let Some(c) = cells.get_mut(i) {
                *c = if v == i64::from(NODATA_INT) {
                    f32::NAN
                } else {
                    v as f32
                };
            }
        };
        let be16 = |p: &[u8]| i64::from(i16::from_be_bytes([p[0], p[1]]));
        let be32 = |p: &[u8]| i64::from(i32::from_be_bytes([p[0], p[1], p[2], p[3]]));
        match kind {
            // Constant.
            0x00 => (0..n).for_each(|i| put(i, min)),
            // Packed bits, 4-bit, 8-bit, 16-bit, 32-bit values above the minimum.
            0x01 => (0..n.min(d.len() * 8))
                .for_each(|i| put(i, min + i64::from((d[i / 8] >> (7 - i % 8)) & 1))),
            0x04 => (0..n.min(d.len() * 2))
                .for_each(|i| put(i, min + i64::from((d[i / 2] >> (4 * (1 - i % 2))) & 15))),
            0x08 => d
                .iter()
                .take(n)
                .enumerate()
                .for_each(|(i, &v)| put(i, min + i64::from(v))),
            0x10 => d
                .chunks_exact(2)
                .take(n)
                .enumerate()
                .for_each(|(i, p)| put(i, min + be16(p))),
            0x20 => d
                .chunks_exact(4)
                .take(n)
                .enumerate()
                .for_each(|(i, p)| put(i, min + be32(p))),
            // Literal runs (16-bit or 8-bit) and no-data runs; runs of the minimum and no-data.
            0xCF | 0xD7 | 0xDF => {
                let mut i = 0;
                let mut p = 0;
                while i < n && p < d.len() {
                    let m = usize::from(d[p]);
                    p += 1;
                    if m < 128 {
                        for _ in 0..m {
                            let v = match kind {
                                0xCF => {
                                    let Some(q) = d.get(p..p + 2) else {
                                        return false;
                                    };
                                    p += 2;
                                    min + be16(q)
                                }
                                0xD7 => {
                                    let Some(&q) = d.get(p) else {
                                        return false;
                                    };
                                    p += 1;
                                    min + i64::from(q)
                                }
                                _ => min,
                            };
                            put(i, v);
                            i += 1;
                        }
                    } else {
                        i += 256 - m;
                    }
                }
            }
            // Run lengths: a count, then a 32-, 16- or 8-bit value.
            0xE0 | 0xF0 | 0xFC | 0xF8 => {
                let size = match kind {
                    0xE0 => 4,
                    0xF0 => 2,
                    _ => 1,
                };
                let mut i = 0;
                for run in d.chunks_exact(1 + size) {
                    if i >= n {
                        break;
                    }
                    let v = min
                        + match size {
                            4 => be32(&run[1..]),
                            2 => be16(&run[1..]),
                            _ => i64::from(run[1]),
                        };
                    for _ in 0..usize::from(run[0]) {
                        put(i, v);
                        i += 1;
                    }
                }
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A 3 × 2 integer grid in one 4 × 2 block (written as the format lays it out).
    fn grid(block: &[u8], integer: bool) -> HashMap<&'static str, Vec<u8>> {
        // Blocks are whole 16-bit words.
        let mut block = block.to_vec();
        if block.len() % 2 == 1 {
            block.push(0);
        }
        let mut hdr = b"GRID1.2\0".to_vec();
        hdr.resize(308, 0);
        hdr[16..20].copy_from_slice(&(if integer { 1i32 } else { 2 }).to_be_bytes());
        hdr[256..264].copy_from_slice(&10f64.to_be_bytes());
        hdr[264..272].copy_from_slice(&10f64.to_be_bytes());
        for (at, v) in [(288, 1i32), (292, 1), (296, 4), (300, 1), (304, 2)] {
            hdr[at..at + 4].copy_from_slice(&v.to_be_bytes());
        }
        let bnd: Vec<u8> = [100f64, 200.0, 130.0, 220.0]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let mut data = vec![0u8; 100];
        data.extend((block.len() as u16 / 2).to_be_bytes());
        data.extend(&block);
        let mut index = vec![0u8; 100];
        index.extend(50i32.to_be_bytes());
        index.extend((block.len() as i32 / 2).to_be_bytes());
        HashMap::from([
            ("hdr.adf", hdr),
            ("dblbnd.adf", bnd),
            ("w001001.adf", data),
            ("w001001x.adf", index),
        ])
    }

    fn read(files: &HashMap<&'static str, Vec<u8>>) -> (Grid, Vec<f32>, bool) {
        let f = |n: &str| files.get(n).cloned();
        let g = open(&f, Some("# c\n1 255 0 0\n2 0 255 0\n")).unwrap();
        let (v, bad) = g.values(&f);
        (g, v, bad)
    }

    #[test]
    fn block_kinds() {
        let nan = f32::NAN;
        let cases: &[(&[u8], [f32; 6])] = &[
            (&[0x00, 1, 7], [7.0; 6]),
            (
                &[0x08, 1, 0xFE, 1, 2, 3, 9, 4, 5, 6, 0],
                [-1., 0., 1., 2., 3., 4.],
            ),
            (&[0x01, 0, 0b1010_1000], [1., 0., 1., 1., 0., 0.]),
            (&[0x04, 0, 0x12, 0x30, 0x45, 0x60], [1., 2., 3., 4., 5., 6.]),
            (
                &[0xD7, 1, 10, 0x02, 1, 2, 0xFE, 0x02, 3, 4],
                [11., 12., nan, 13., 14., nan],
            ),
            (&[0xDF, 1, 5, 0x01, 0xFD, 0x04], [5., nan, nan, 5., 5., 5.]),
            (&[0xFC, 0, 0x03, 9, 0x05, 4], [9., 9., 9., 4., 4., 4.]),
            (
                &[0xF0, 0, 0x02, 0xFF, 0xFF, 0x06, 0x00, 0x08],
                [-1., -1., 8., 8., 8., 8.],
            ),
            (
                &[0xCF, 0, 2, 1, 0, 0, 2, 0xFE, 2, 0, 2, 0, 2],
                [256., 2., nan, 2., 2., nan],
            ),
        ];
        for (block, want) in cases {
            let (g, v, bad) = read(&grid(block, true));
            assert_eq!((g.width, g.height), (3, 2));
            assert!(!bad, "{block:02x?}");
            for (got, w) in v.iter().zip(want) {
                assert!(
                    got == w || (got.is_nan() && w.is_nan()),
                    "{block:02x?}: {v:?}"
                );
            }
        }
        // An unknown kind leaves the block as no data and says so.
        let (_, v, bad) = read(&grid(&[0x77, 0, 1, 2], true));
        assert!(bad && v.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn float_grid_and_header() {
        let mut block = Vec::new();
        for v in [1.5f32, NODATA_FLOAT, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0] {
            block.extend(v.to_be_bytes());
        }
        let (g, v, bad) = read(&grid(&block, false));
        assert!(!bad && !g.integer);
        assert_eq!(v[0], 1.5);
        assert!(v[1].is_nan());
        assert_eq!(v[3], 4.0);
        assert_eq!(g.transform().apply(0.0, 0.0), (100.0, 220.0));
        assert_eq!(g.palette, [(1, [255, 0, 0]), (2, [0, 255, 0])]);
        assert!(!sniff(b"GRID1.1\0"));
    }
}
