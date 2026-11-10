//! Segmentations (DICOM SEG, [`crate::seg`]) and structure sets (RTSTRUCT, [`crate::rtstruct`])
//! as label volumes for display: each segment's voxels on a grid in patient coordinates, packed
//! into as few 16-bit [`Layer`]s as overlaps allow, with the segment's colour, label and volume.
//!
//! Read-only analysis (DESIGN §7): the volumes are what the objects state, measured; nothing
//! here detects or classifies anything.

use crate::volume::{Geometry, Volume, Voxels, cross, dot, normalise};

/// Where the labels came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelSource {
    Seg,
    RtStruct,
    /// A label volume file (NIfTI, MGH and the like), placed by its world affine.
    Volume,
}

/// One segment (SEG) or region of interest (RTSTRUCT).
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// Segment Number, or ROI Number.
    pub number: i64,
    pub label: String,
    /// sRGB.
    pub colour: [u8; 3],
    /// Segment Algorithm Type or ROI Generation Algorithm (MANUAL, AUTOMATIC, …).
    pub algorithm: String,
    /// The segment holds fractions (probability or occupancy), not a binary mask.
    pub fractional: bool,
    /// Volume, mL: voxels (or summed fractions) × voxel volume for SEG, contour areas × plane
    /// spacing for RTSTRUCT.
    pub ml: f64,
    /// Voxels set (fractional: above 0).
    pub voxels: u64,
}

/// Voxels of one or more segments: a U16 [`Volume`] whose values are segment index + 1 (0:
/// none), or, for a fractional segment, its fraction × 65535.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub volume: Volume,
    /// The segment index when the layer holds one fractional segment.
    pub fraction: Option<usize>,
}

/// The segments of a SEG or RTSTRUCT, ready for display.
#[derive(Clone, Debug, PartialEq)]
pub struct Labels {
    pub source: LabelSource,
    pub segments: Vec<Segment>,
    pub layers: Vec<Layer>,
    /// Frame of Reference UID: the images these labels can be laid over.
    pub frame_of_reference: String,
    /// Series Instance UIDs referenced.
    pub referenced_series: Vec<String>,
    /// Content label or Series Description.
    pub title: String,
}

impl Labels {
    /// The segments (indices) at patient position `p`: the nearest voxel of each layer.
    pub fn at(&self, p: [f64; 3]) -> Vec<usize> {
        let mut out = Vec::new();
        for l in &self.layers {
            let Some(v) = nearest(&l.volume, p) else {
                continue;
            };
            match l.fraction {
                Some(s) if v >= 32768 => out.push(s),
                Some(_) => {}
                None if v > 0 => out.push(v as usize - 1),
                None => {}
            }
        }
        out.sort_unstable();
        out
    }

    /// Bytes of the layers' voxels.
    pub fn bytes(&self) -> u64 {
        self.layers.iter().map(|l| l.volume.bytes()).sum()
    }
}

/// The voxel value nearest patient position `p` (0 outside the grid: more than half a voxel
/// past its edge centres).
pub fn nearest(vol: &Volume, p: [f64; 3]) -> Option<u16> {
    let Voxels::U16(data) = &vol.voxels else {
        return None;
    };
    let v = vol.geometry.to_voxel(p);
    let [w, h, d] = vol.geometry.dims;
    let i = v[0].round();
    let j = v[1].round();
    let k = v[2].round();
    if i < 0.0 || j < 0.0 || k < 0.0 || i >= w as f64 || j >= h as f64 || k >= d as f64 {
        return None;
    }
    let idx = (k as usize * h as usize + j as usize) * w as usize + i as usize;
    data.get(idx).copied()
}

/// CIELab (D50, as DICOM's Recommended Display CIELab Value: L 0..65535 for 0..100, a and b
/// 0..65535 for −128..127) to sRGB.
pub fn lab_to_srgb(lab: [u16; 3]) -> [u8; 3] {
    let l = f64::from(lab[0]) * 100.0 / 65535.0;
    let a = f64::from(lab[1]) * 255.0 / 65535.0 - 128.0;
    let b = f64::from(lab[2]) * 255.0 / 65535.0 - 128.0;
    // Lab → XYZ (D50 white).
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let e = 216.0 / 24389.0;
    let k = 24389.0 / 27.0;
    let finv = |f: f64| {
        if f * f * f > e {
            f * f * f
        } else {
            (116.0 * f - 16.0) / k
        }
    };
    let xr = finv(fx);
    let yr = if l > k * e { fy * fy * fy } else { l / k };
    let zr = finv(fz);
    let (x, y, z) = (xr * 0.964_22, yr, zr * 0.825_21);
    // Bradford D50 → D65, then XYZ → linear sRGB (IEC 61966-2-1).
    let x65 = 0.955_576_6 * x - 0.023_039_3 * y + 0.063_163_6 * z;
    let y65 = -0.028_289_5 * x + 1.009_941_6 * y + 0.021_007_7 * z;
    let z65 = 0.012_298_2 * x - 0.020_483 * y + 1.329_909_8 * z;
    let r = 3.240_454_2 * x65 - 1.537_138_5 * y65 - 0.498_531_4 * z65;
    let g = -0.969_266 * x65 + 1.876_010_8 * y65 + 0.041_556 * z65;
    let bl = 0.055_643_4 * x65 - 0.204_025_9 * y65 + 1.057_225_2 * z65;
    let enc = |c: f64| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    };
    [enc(r), enc(g), enc(bl)]
}

/// Colours for segments that state none: distinct, readable over grey images.
pub const DEFAULT_COLOURS: [[u8; 3]; 8] = [
    [230, 25, 75],
    [60, 180, 75],
    [255, 225, 25],
    [0, 130, 200],
    [245, 130, 48],
    [145, 30, 180],
    [70, 240, 240],
    [240, 50, 230],
];

/// A per-plane mask of one segment in the full grid: 0..=255 (binary masks hold 0 or 255).
#[derive(Clone, Debug)]
pub(crate) struct PlaneMask {
    pub plane: usize,
    pub data: Vec<u8>,
}

/// Collects segment masks on a grid, then packs them into cropped [`Layer`]s.
#[derive(Clone, Debug)]
pub(crate) struct Builder {
    pub grid: Geometry,
    /// Per segment: its masks.
    pub masks: Vec<Vec<PlaneMask>>,
}

impl Builder {
    pub fn new(grid: Geometry, segments: usize) -> Builder {
        Builder {
            grid,
            masks: vec![Vec::new(); segments],
        }
    }

    fn plane_len(&self) -> usize {
        self.grid.dims[0] as usize * self.grid.dims[1] as usize
    }

    /// The mask of `segment` on `plane`, made empty when new.
    pub fn mask(&mut self, segment: usize, plane: usize) -> &mut Vec<u8> {
        let n = self.plane_len();
        let list = &mut self.masks[segment];
        let at = match list.iter().position(|m| m.plane == plane) {
            Some(i) => i,
            None => {
                list.push(PlaneMask {
                    plane,
                    data: vec![0; n],
                });
                list.len() - 1
            }
        };
        &mut list[at].data
    }

    /// Voxels set per segment and summed fractions (1.0 per binary voxel).
    pub fn counts(&self) -> Vec<(u64, f64)> {
        self.masks
            .iter()
            .map(|ms| {
                let mut n = 0u64;
                let mut sum = 0u64;
                for m in ms {
                    for &v in &m.data {
                        if v > 0 {
                            n += 1;
                            sum += u64::from(v);
                        }
                    }
                }
                (n, sum as f64 / 255.0)
            })
            .collect()
    }

    /// Packs the masks into layers on the grid cropped to what is set (`fractional[s]`: one
    /// layer of its own). Binary segments share a layer while they do not overlap.
    pub fn finish(self, fractional: &[bool]) -> Vec<Layer> {
        let [w, h, _] = self.grid.dims;
        let (w, h) = (w as usize, h as usize);
        // Bounding box over every set voxel.
        let (mut i0, mut j0, mut k0) = (usize::MAX, usize::MAX, usize::MAX);
        let (mut i1, mut j1, mut k1) = (0, 0, 0);
        for ms in &self.masks {
            for m in ms {
                for (idx, &v) in m.data.iter().enumerate() {
                    if v == 0 {
                        continue;
                    }
                    let (i, j) = (idx % w, idx / w);
                    i0 = i0.min(i);
                    i1 = i1.max(i);
                    j0 = j0.min(j);
                    j1 = j1.max(j);
                    k0 = k0.min(m.plane);
                    k1 = k1.max(m.plane);
                }
            }
        }
        if i0 == usize::MAX {
            return Vec::new();
        }
        // One empty voxel of margin (outlines close at the crop's edge), within the grid.
        let slices = self.grid.slices();
        let (i0, j0, k0) = (
            i0.saturating_sub(1),
            j0.saturating_sub(1),
            k0.saturating_sub(1),
        );
        let (i1, j1, k1) = (
            (i1 + 1).min(w - 1),
            (j1 + 1).min(h - 1),
            (k1 + 1).min(slices - 1),
        );
        let (cw, ch, cd) = (i1 - i0 + 1, j1 - j0 + 1, k1 - k0 + 1);
        let crop = crop(&self.grid, [i0, j0, k0], [cw, ch, cd]);
        let n = cw * ch * cd;
        let index = |plane: usize, idx: usize| -> Option<usize> {
            let (i, j) = (idx % w, idx / w);
            if plane < k0 || plane > k1 || i < i0 || i > i1 || j < j0 || j > j1 {
                return None;
            }
            Some(((plane - k0) * ch + (j - j0)) * cw + (i - i0))
        };
        let mut layers: Vec<(Vec<u16>, Option<usize>)> = Vec::new();
        for (s, ms) in self.masks.iter().enumerate() {
            if ms.is_empty() {
                continue;
            }
            if fractional.get(s).copied().unwrap_or(false) {
                let mut buf = vec![0u16; n];
                for m in ms {
                    for (idx, &v) in m.data.iter().enumerate() {
                        if v > 0
                            && let Some(o) = index(m.plane, idx)
                        {
                            buf[o] = u16::from(v) * 257;
                        }
                    }
                }
                layers.push((buf, Some(s)));
                continue;
            }
            let free = |buf: &Vec<u16>| {
                ms.iter().all(|m| {
                    m.data
                        .iter()
                        .enumerate()
                        .all(|(idx, &v)| v == 0 || index(m.plane, idx).is_none_or(|o| buf[o] == 0))
                })
            };
            let at = match layers
                .iter()
                .position(|(buf, frac)| frac.is_none() && free(buf))
            {
                Some(l) => l,
                None => {
                    layers.push((vec![0u16; n], None));
                    layers.len() - 1
                }
            };
            let buf = &mut layers[at].0;
            let value = (s + 1).min(u16::MAX as usize) as u16;
            for m in ms {
                for (idx, &v) in m.data.iter().enumerate() {
                    if v > 0
                        && let Some(o) = index(m.plane, idx)
                    {
                        buf[o] = value;
                    }
                }
            }
        }
        layers
            .into_iter()
            .map(|(buf, fraction)| Layer {
                volume: Volume {
                    geometry: crop.clone(),
                    voxels: Voxels::U16(buf),
                    loaded: vec![true; cd],
                    step_xy: 1,
                },
                fraction,
            })
            .collect()
    }
}

/// The part of `g` from voxel `start`, `size` voxels.
pub(crate) fn crop(g: &Geometry, start: [usize; 3], size: [usize; 3]) -> Geometry {
    let positions: Vec<[f64; 3]> = (start[2]..start[2] + size[2])
        .map(|k| g.to_patient([start[0] as f64, start[1] as f64, k as f64]))
        .collect();
    Geometry::new(
        &positions,
        g.row,
        g.col,
        g.pixel,
        [size[0] as u32, size[1] as u32],
    )
}

/// A grid of planes at `along` (mm along `normal` from `origin`, increasing), evenly
/// re-spaced when they sit on multiples of a common step (missing planes filled in), with one
/// empty plane added at each end.
pub(crate) fn plane_grid(
    origin: [f64; 3],
    row: [f64; 3],
    col: [f64; 3],
    pixel: [f64; 2],
    size: [u32; 2],
    along: &[f64],
    step: Option<f64>,
) -> (Geometry, Vec<usize>) {
    let normal = normalise(cross(row, col)).unwrap_or([0.0, 0.0, 1.0]);
    let step = step
        .filter(|s| *s > 1e-3)
        .or_else(|| {
            along
                .windows(2)
                .map(|w| w[1] - w[0])
                .filter(|d| *d > 1e-3)
                .min_by(f64::total_cmp)
        })
        .unwrap_or(1.0);
    let a0 = along.first().copied().unwrap_or(0.0);
    let regular = along.iter().all(|a| {
        let f = (a - a0) / step;
        (f - f.round()).abs() < 0.1
    });
    let mut planes: Vec<f64> = Vec::new();
    let mut index = Vec::with_capacity(along.len());
    if regular {
        let last = along
            .last()
            .map_or(0, |a| ((a - a0) / step).round() as usize);
        planes.push(a0 - step);
        for m in 0..=last {
            planes.push(a0 + step * m as f64);
        }
        planes.push(a0 + step * (last + 1) as f64);
        for a in along {
            index.push(((a - a0) / step).round() as usize + 1);
        }
    } else {
        planes.push(a0 - step);
        planes.extend_from_slice(along);
        planes.push(along.last().copied().unwrap_or(0.0) + step);
        index.extend(1..=along.len());
    }
    let positions: Vec<[f64; 3]> = planes
        .iter()
        .map(|a| [0, 1, 2].map(|c| origin[c] + normal[c] * a))
        .collect();
    let g = Geometry::new(&positions, row, col, pixel, size);
    (g, index)
}

/// Distinct values of `a` (sorted), merged within `tol`.
pub(crate) fn distinct(mut a: Vec<f64>, tol: f64) -> Vec<f64> {
    a.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for x in a {
        if out.last().is_none_or(|l| x - l > tol) {
            out.push(x);
        }
    }
    out
}

pub(crate) fn along(p: [f64; 3], normal: [f64; 3]) -> f64 {
    dot(p, normal)
}

/// mL of one voxel of `g` (mean slice spacing).
pub fn voxel_ml(g: &Geometry) -> f64 {
    let v = g.voxel_mm();
    v[0] * v[1] * v[2] / 1000.0
}

/// All of a DS / IS / FL / FD multi-value as numbers (one pass over the text).
pub(crate) fn numbers(v: &crate::Values, e: &crate::Element) -> Vec<f64> {
    use crate::dict::Vr;
    match e.vr {
        Vr::DS | Vr::IS | Vr::UN | Vr::LO | Vr::SH => {
            let b = v.text_bytes(e);
            let Ok(s) = std::str::from_utf8(b) else {
                return Vec::new();
            };
            s.split('\\')
                .map(|x| x.trim().parse::<f64>().unwrap_or(f64::NAN))
                .collect()
        }
        _ => (0..).map_while(|i| v.number(e, i)).take(1 << 24).collect(),
    }
}
