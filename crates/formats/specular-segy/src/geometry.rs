//! Survey geometry: whether the traces are a 3-D post-stack volume (one trace per inline ×
//! crossline cell) or a 2-D line, and where each trace is.
//!
//! The inline and crossline numbers are read from the standard bytes (189, 193) and, when
//! those do not form a grid, from the common older places: field record and CDP (9, 21),
//! energy source point and trace number in field record (17, 13). A pair forms a grid when
//! both have at least two values on regular steps, at most 1 % of the traces repeat a
//! cell (pre-stack gathers repeat every cell), and the traces fill at least a fifth of the
//! cells. Cells without a trace are gaps.

use crate::{Result, Segy};

/// Inline / crossline byte pairs tried, in order.
pub const KEY_CANDIDATES: [(usize, usize); 3] = [(189, 193), (9, 21), (17, 13)];

/// No trace in a cell.
const NONE: u32 = u32::MAX;

/// The per-trace header values the geometry is found from.
#[derive(Clone, Debug, Default)]
pub struct Keys {
    /// Values at 189, 193, 9, 21, 17, 13 for each trace.
    pub keys: Vec<[i32; 6]>,
    /// CDP X / Y (or source X / Y when every CDP coordinate is 0), scaled.
    pub coords: Vec<[f64; 2]>,
    /// The coordinates are the source's.
    pub source: bool,
}

const KEY_BYTES: [usize; 6] = [189, 193, 9, 21, 17, 13];

/// Where the traces are.
#[derive(Clone, Debug)]
pub struct Geometry {
    /// CDP X / Y of each trace (source X / Y when no trace has a CDP coordinate), scaled by
    /// the coordinate scalar; all zero when the file has none.
    pub coords: Vec<[f64; 2]>,
    /// The coordinates are the source's, not the CDP's.
    pub source_coords: bool,
    /// CDP ensemble number (bytes 21–24) of each trace, for 2-D lines.
    pub cdp: Vec<i32>,
    /// The 3-D grid, when the traces form one.
    pub volume: Option<Volume>,
}

/// A 3-D post-stack volume: traces on an inline × crossline grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    /// The header bytes the inline and crossline numbers came from.
    pub key: (usize, usize),
    /// Inline numbers, ascending on a regular step.
    pub inlines: Vec<i32>,
    /// Crossline numbers, ascending on a regular step.
    pub crosslines: Vec<i32>,
    /// Trace in each cell, inline-major (`il * crosslines + xl`); `u32::MAX` for gaps.
    index: Vec<u32>,
    /// Cells without a trace.
    pub gaps: usize,
    /// Traces that repeated a cell (the first one is kept).
    pub duplicates: usize,
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// The sorted distinct values and the regular grid through them: (first, step, count).
fn axis(values: impl Iterator<Item = i32>) -> Option<(i32, i32, usize)> {
    let mut v: Vec<i32> = values.collect();
    v.sort_unstable();
    v.dedup();
    if v.len() < 2 {
        return None;
    }
    let step = v
        .windows(2)
        .fold(0i64, |g, w| gcd(g, i64::from(w[1]) - i64::from(w[0])));
    let span = i64::from(v[v.len() - 1]) - i64::from(v[0]);
    let n = usize::try_from(span / step + 1).ok()?;
    Some((v[0], i32::try_from(step).ok()?, n))
}

impl Volume {
    /// Builds the grid for one byte pair, or `None` when the values do not form one.
    fn try_build(keys: &[[i32; 6]], a: usize, b: usize, key: (usize, usize)) -> Option<Volume> {
        let traces = keys.len();
        if traces < 4 || traces > NONE as usize {
            return None;
        }
        let (il0, ils, nil) = axis(keys.iter().map(|k| k[a]))?;
        let (xl0, xls, nxl) = axis(keys.iter().map(|k| k[b]))?;
        let cells = nil.checked_mul(nxl)?;
        // At least a fifth of the cells filled.
        if cells > traces.saturating_mul(5) || cells > 1 << 31 {
            return None;
        }
        let mut index = vec![NONE; cells];
        let mut duplicates = 0;
        for (t, k) in keys.iter().enumerate() {
            let i = ((i64::from(k[a]) - i64::from(il0)) / i64::from(ils)) as usize;
            let x = ((i64::from(k[b]) - i64::from(xl0)) / i64::from(xls)) as usize;
            let cell = &mut index[i * nxl + x];
            if *cell == NONE {
                *cell = t as u32;
            } else {
                duplicates += 1;
            }
        }
        if duplicates * 100 > traces {
            return None;
        }
        let gaps = index.iter().filter(|&&c| c == NONE).count();
        Some(Volume {
            key,
            inlines: (0..nil).map(|i| il0 + ils * i as i32).collect(),
            crosslines: (0..nxl).map(|i| xl0 + xls * i as i32).collect(),
            index,
            gaps,
            duplicates,
        })
    }

    /// The trace in each cell, inline-major (`il * crosslines + xl`); `u32::MAX` for gaps.
    pub fn cells(&self) -> &[u32] {
        &self.index
    }

    /// The trace at inline index `il` and crossline index `xl`, if any.
    pub fn trace_at(&self, il: usize, xl: usize) -> Option<usize> {
        if il >= self.inlines.len() || xl >= self.crosslines.len() {
            return None;
        }
        match self.index[il * self.crosslines.len() + xl] {
            NONE => None,
            t => Some(t as usize),
        }
    }

    /// Inline section `il`: crosslines across, samples down (`values[k * crosslines + xl]`),
    /// NaN in gaps.
    pub fn inline_section(&self, segy: &Segy, il: usize) -> Result<Vec<f32>> {
        let (nx, ns) = (self.crosslines.len(), segy.samples);
        let _held = specular_core::budget::reserve_n((nx * ns) as u64, 4)?;
        let mut out = vec![f32::NAN; nx * ns];
        let mut tr = vec![0.0; ns];
        let mut scratch = Vec::new();
        for x in 0..nx {
            if let Some(t) = self.trace_at(il, x) {
                segy.trace_into(t, &mut tr, &mut scratch)?;
                for (k, v) in tr.iter().enumerate() {
                    out[k * nx + x] = *v;
                }
            }
        }
        Ok(out)
    }

    /// Crossline section `xl`: inlines across, samples down, NaN in gaps.
    pub fn crossline_section(&self, segy: &Segy, xl: usize) -> Result<Vec<f32>> {
        let (ni, ns) = (self.inlines.len(), segy.samples);
        let _held = specular_core::budget::reserve_n((ni * ns) as u64, 4)?;
        let mut out = vec![f32::NAN; ni * ns];
        let mut tr = vec![0.0; ns];
        let mut scratch = Vec::new();
        for i in 0..ni {
            if let Some(t) = self.trace_at(i, xl) {
                segy.trace_into(t, &mut tr, &mut scratch)?;
                for (k, v) in tr.iter().enumerate() {
                    out[k * ni + i] = *v;
                }
            }
        }
        Ok(out)
    }

    /// The whole volume, inline-major then crossline then sample
    /// (`values[(il * crosslines + xl) * samples + k]`), NaN in gaps. `progress` is called
    /// with the inlines done and may stop the read by returning false.
    pub fn read(
        &self,
        segy: &Segy,
        mut progress: impl FnMut(usize) -> bool,
    ) -> Result<Option<Vec<f32>>> {
        let (nx, ns) = (self.crosslines.len(), segy.samples);
        // A grid a fifth full of one-byte samples comes out 20 times the traces' size:
        // reserved from the memory budget before it is allocated.
        let cells = self.inlines.len() * nx;
        let _held = specular_core::budget::reserve_n((cells * ns) as u64, 4)?;
        let mut out = vec![f32::NAN; cells * ns];
        let mut scratch = Vec::new();
        for i in 0..self.inlines.len() {
            // Runs of consecutive traces read in one go.
            let mut x = 0;
            while x < nx {
                let Some(t0) = self.trace_at(i, x) else {
                    x += 1;
                    continue;
                };
                let mut n = 1;
                while x + n < nx && self.trace_at(i, x + n) == Some(t0 + n) {
                    n += 1;
                }
                let o = (i * nx + x) * ns;
                if n == 1 {
                    segy.trace_into(t0, &mut out[o..o + ns], &mut scratch)?;
                } else {
                    segy.traces_into(t0..t0 + n, &mut out[o..o + n * ns])?;
                }
                x += n;
            }
            if !progress(i + 1) {
                return Ok(None);
            }
        }
        Ok(Some(out))
    }
}

/// Reads the keys of every trace.
pub fn keys(segy: &Segy) -> Result<Keys> {
    let mut k = Keys {
        keys: Vec::with_capacity(segy.traces),
        coords: Vec::with_capacity(segy.traces),
        source: false,
    };
    let mut src = Vec::with_capacity(segy.traces);
    segy.headers(0..segy.traces, |_, h| {
        k.keys.push(KEY_BYTES.map(|b| h.i32(b)));
        let (x, y) = h.cdp_xy();
        k.coords.push([x, y]);
        let (sx, sy) = h.source_xy();
        src.push([sx, sy]);
    })?;
    let zero = |c: &[f64; 2]| c[0] == 0.0 && c[1] == 0.0;
    if k.coords.iter().all(zero) && !src.iter().all(zero) {
        k.coords = src;
        k.source = true;
    }
    Ok(k)
}

pub(crate) fn detect(segy: &Segy) -> Result<Geometry> {
    let k = keys(segy)?;
    let volume = from_key_values(&k.keys);
    Ok(Geometry {
        cdp: k.keys.iter().map(|v| v[3]).collect(),
        coords: k.coords,
        source_coords: k.source,
        volume,
    })
}

/// The geometry of traces whose keys are known (tests, and callers that read headers
/// themselves).
pub fn from_key_values(keys: &[[i32; 6]]) -> Option<Volume> {
    KEY_CANDIDATES
        .iter()
        .enumerate()
        .find_map(|(c, &key)| Volume::try_build(keys, 2 * c, 2 * c + 1, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(ils: &[i32], xls: &[i32], skip: &[(i32, i32)]) -> Vec<[i32; 6]> {
        let mut v = Vec::new();
        for &i in ils {
            for &x in xls {
                if !skip.contains(&(i, x)) {
                    v.push([i, x, 0, 0, 0, 0]);
                }
            }
        }
        v
    }

    #[test]
    fn regular_grid_with_gaps() {
        let k = grid(&[100, 102, 104], &[10, 11, 12, 13], &[(102, 11), (104, 13)]);
        let v = from_key_values(&k).unwrap();
        assert_eq!(v.key, (189, 193));
        assert_eq!(v.inlines, [100, 102, 104]);
        assert_eq!(v.crosslines, [10, 11, 12, 13]);
        assert_eq!(v.gaps, 2);
        assert_eq!(v.trace_at(1, 1), None);
        assert_eq!(v.trace_at(1, 2), Some(5));
    }

    #[test]
    fn missing_inline_is_a_gap_plane() {
        let k = grid(&[1, 2, 4], &[1, 2, 3], &[]);
        let v = from_key_values(&k).unwrap();
        assert_eq!(v.inlines, [1, 2, 3, 4]);
        assert_eq!(v.gaps, 3);
    }

    #[test]
    fn fallback_bytes() {
        // Nothing at 189 / 193; field record and CDP at 9 / 21.
        let k: Vec<[i32; 6]> = grid(&[5, 6, 7], &[1, 2], &[])
            .into_iter()
            .map(|g| [0, 0, g[0], g[1], 0, 0])
            .collect();
        assert_eq!(from_key_values(&k).unwrap().key, (9, 21));
    }

    #[test]
    fn gathers_and_lines_are_not_volumes() {
        // A 2-D line with inline = crossline = CDP: a diagonal.
        let line: Vec<[i32; 6]> = (0..50).map(|i| [i, i, 0, i, 0, 0]).collect();
        assert!(from_key_values(&line).is_none());
        // Pre-stack: every cell three times.
        let mut pre = grid(&[1, 2, 3], &[1, 2, 3], &[]);
        pre.extend(pre.clone());
        pre.extend(grid(&[1, 2, 3], &[1, 2, 3], &[]));
        assert!(from_key_values(&pre).is_none());
        // One inline only.
        assert!(from_key_values(&grid(&[1], &[1, 2, 3, 4, 5], &[])).is_none());
    }
}
