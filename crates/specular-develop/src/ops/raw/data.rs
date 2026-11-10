//! The data buffer `D` of the raw front end: everything about one sensor and one set of
//! options that the kernels read (CFA, black levels, linearization, white balance, the DNG
//! opcode lists as records, the output matrix or the DNG profile with its tables).
//!
//! All words are u32; floats are stored as their bits. The header offsets are the `H_*`
//! constants of `common.wgsl`; [`HEADER`] words come first, then the variable blocks.

use specular_raw::sensor::{HsvTableData, Layout, OpcodeInfo, PointKind, SensorMeta};

use super::{Options, Output, WhiteBalance};

pub(crate) const H_LAYOUT: usize = 0;
pub(crate) const H_CFA_W: usize = 1;
pub(crate) const H_CFA_H: usize = 2;
pub(crate) const H_CFA_OFF: usize = 3;
pub(crate) const H_BL_ROWS: usize = 4;
pub(crate) const H_BL_COLS: usize = 5;
pub(crate) const H_BL_CPP: usize = 6;
pub(crate) const H_BL_OFF: usize = 7;
pub(crate) const H_BL_OX: usize = 8;
pub(crate) const H_BL_OY: usize = 9;
pub(crate) const H_BC_OFF: usize = 10;
pub(crate) const H_BC_LEN: usize = 11;
pub(crate) const H_BR_OFF: usize = 12;
pub(crate) const H_BR_LEN: usize = 13;
pub(crate) const H_LIN_OFF: usize = 14;
pub(crate) const H_LIN_LEN: usize = 15;
pub(crate) const H_WHITE: usize = 16;
pub(crate) const H_WB: usize = 17;
pub(crate) const H_L1: usize = 20;
pub(crate) const H_L2: usize = 22;
pub(crate) const H_L3: usize = 24;
pub(crate) const H_MAT: usize = 26;
pub(crate) const H_PROF: usize = 35;
pub(crate) const H_W: usize = 36;
pub(crate) const H_H: usize = 37;
pub(crate) const H_CPP: usize = 38;
pub(crate) const H_CX: usize = 39;
pub(crate) const H_CY: usize = 40;
pub(crate) const H_CW: usize = 41;
pub(crate) const H_CH: usize = 42;
pub(crate) const H_BAD: usize = 43;
pub(crate) const H_BAD_V: usize = 44;
pub(crate) const H_BAD_RECT: usize = 45;
pub(crate) const H_OUTPUT: usize = 49;
// Corrections (written by `correct`; see `common.wgsl`).
pub(crate) const H_CLIP: usize = 50;
pub(crate) const H_CAL: usize = 51;
pub(crate) const H_HOT: usize = 52;
pub(crate) const H_HL: usize = 53;
pub(crate) const H_HL_T: usize = 54;
pub(crate) const H_HL_GRID: usize = 55;
pub(crate) const H_HL_GW: usize = 56;
pub(crate) const H_HL_GH: usize = 57;
pub(crate) const H_HL_TS: usize = 58;
pub(crate) const H_RB_OFF: usize = 59;
pub(crate) const H_RB_BW: usize = 60;
pub(crate) const H_RB_BH: usize = 61;
pub(crate) const H_RB_BS: usize = 62;
pub(crate) const H_RB_SEG: usize = 63;
pub(crate) const H_CA: usize = 64;
pub(crate) const H_CA_CX: usize = 65;
pub(crate) const H_CA_CY: usize = 66;
pub(crate) const H_CA_R: usize = 67;
pub(crate) const H_CA_RA: usize = 68;
pub(crate) const H_CA_RB: usize = 69;
pub(crate) const H_CA_BA: usize = 70;
pub(crate) const H_CA_BB: usize = 71;
pub(crate) const H_CS: usize = 72;
pub(crate) const H_CELLS_W: usize = 73;
pub(crate) const H_CELLS_H: usize = 74;
/// 1: the samples are f32 words (a floating point DNG, an HDR merge), not 16-bit pairs.
pub(crate) const H_FLOAT: usize = 75;
/// 6×6 colour patterns (X-Trans): the colours relative to the crop, 2 bits each, two rows
/// per word (row `r`, column `k` at bit `(r & 1)·12 + 2k` of word `r / 2`), and 1 when set.
/// The raw CA pass looks colours up here without divisions.
pub(crate) const H_XT6: usize = 76;
pub(crate) const H_XT6_ON: usize = 79;
pub(crate) const HEADER: usize = 96;
/// Words per opcode record.
pub(crate) const REC: usize = 24;

/// Linear Rec.709 → linear Rec.2020 (ITU-R BT.2087), rows summing to 1.
pub const REC709_TO_REC2020: [[f32; 3]; 3] = [
    [0.627_404, 0.329_282, 0.043_313_6],
    [0.069_097, 0.919_540, 0.011_361_2],
    [0.016_391_6, 0.088_013_2, 0.895_595],
];

pub(crate) fn mul3(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

/// The sensor geometry the data describes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    pub width: u32,
    pub height: u32,
    pub cpp: u32,
    pub crop: specular_raw::Rect,
    /// The samples on the device are f32 words ([`SensorData::float`](specular_raw::SensorData)).
    pub float: bool,
}

/// What [`build`] could not put on the GPU (opcode names, once each).
pub(crate) type Skipped = Vec<&'static str>;

struct W(Vec<u32>);

impl W {
    fn at(&self) -> u32 {
        self.0.len() as u32
    }
    fn f(&mut self, v: f32) {
        self.0.push(v.to_bits());
    }
    fn fs(&mut self, v: &[f32]) -> u32 {
        let at = self.at();
        for &x in v {
            self.f(x);
        }
        at
    }
    /// u16 values two per word, low half first.
    fn u16s(&mut self, v: &[u16]) -> u32 {
        let at = self.at();
        for c in v.chunks(2) {
            let hi = c.get(1).copied().unwrap_or(0);
            self.0.push(u32::from(c[0]) | (u32::from(hi) << 16));
        }
        at
    }
    fn set(&mut self, i: usize, v: u32) {
        self.0[i] = v;
    }
    fn setf(&mut self, i: usize, v: f32) {
        self.0[i] = v.to_bits();
    }
}

fn op_name(o: &OpcodeInfo) -> &'static str {
    match o {
        OpcodeInfo::Point { kind, .. } => match kind {
            PointKind::MapTable(_) => "MapTable",
            PointKind::MapPolynomial(_) => "MapPolynomial",
            PointKind::GainMap { .. } => "GainMap",
            PointKind::Vignette { .. } => "FixVignetteRadial",
            PointKind::DeltaPerRow(_) => "DeltaPerRow",
            PointKind::DeltaPerColumn(_) => "DeltaPerColumn",
            PointKind::ScalePerRow(_) => "ScalePerRow",
            PointKind::ScalePerColumn(_) => "ScalePerColumn",
        },
        OpcodeInfo::FixBadPixelsConstant { .. } => "FixBadPixelsConstant",
        OpcodeInfo::FixBadPixelsList { .. } => "FixBadPixelsList",
        OpcodeInfo::Warp { fisheye: false, .. } => "WarpRectilinear",
        OpcodeInfo::Warp { fisheye: true, .. } => "WarpFisheye",
    }
}

/// Writes the point opcodes of `ops` as records; returns (offset, count) and adds the others
/// to `skipped`.
fn list(w: &mut W, ops: &[&OpcodeInfo], skipped: &mut Skipped) -> (u32, u32) {
    let points: Vec<&OpcodeInfo> = ops
        .iter()
        .copied()
        .filter(|o| {
            let ok = matches!(o, OpcodeInfo::Point { .. });
            if !ok && !skipped.contains(&op_name(o)) {
                skipped.push(op_name(o));
            }
            ok
        })
        .collect();
    let at = w.at();
    w.0.resize(w.0.len() + points.len() * REC, 0);
    for (k, o) in points.iter().enumerate() {
        let OpcodeInfo::Point {
            rect,
            plane,
            planes,
            row_pitch,
            col_pitch,
            bounds,
            kind,
        } = o
        else {
            continue;
        };
        let r = at as usize + k * REC;
        let (code, off, len) = match kind {
            PointKind::MapTable(t) => (1, w.u16s(t), t.len() as u32),
            PointKind::MapPolynomial(c) => {
                let c: Vec<f32> = c.iter().map(|&v| v as f32).collect();
                (2, w.fs(&c), c.len() as u32)
            }
            PointKind::GainMap { gains, .. } => (3, w.fs(gains), gains.len() as u32),
            PointKind::Vignette { .. } => (4, 0, 0),
            PointKind::DeltaPerRow(v) => (5, w.fs(v), v.len() as u32),
            PointKind::DeltaPerColumn(v) => (6, w.fs(v), v.len() as u32),
            PointKind::ScalePerRow(v) => (7, w.fs(v), v.len() as u32),
            PointKind::ScalePerColumn(v) => (8, w.fs(v), v.len() as u32),
        };
        let head = [
            code, rect.x, rect.y, rect.w, rect.h, *plane, *planes, *row_pitch, *col_pitch,
            bounds.x, bounds.y, bounds.w, bounds.h, off, len,
        ];
        for (i, v) in head.into_iter().enumerate() {
            w.set(r + i, v);
        }
        match kind {
            PointKind::GainMap {
                points_v,
                points_h,
                spacing_v,
                spacing_h,
                origin_v,
                origin_h,
                planes,
                ..
            } => {
                w.set(r + 15, *points_v);
                w.set(r + 16, *points_h);
                w.setf(r + 17, *spacing_v as f32);
                w.setf(r + 18, *spacing_h as f32);
                w.setf(r + 19, *origin_v as f32);
                w.setf(r + 20, *origin_h as f32);
                w.set(r + 21, *planes);
            }
            PointKind::Vignette { k, centre, inv_m2 } => {
                for (i, v) in k.iter().enumerate() {
                    w.setf(r + 15 + i, *v as f32);
                }
                w.setf(r + 20, centre.0 as f32);
                w.setf(r + 21, centre.1 as f32);
                w.setf(r + 22, *inv_m2 as f32);
            }
            _ => {}
        }
    }
    (at, points.len() as u32)
}

fn table(w: &mut W, t: &HsvTableData) -> u32 {
    let at = w.at();
    w.0.extend([t.hue, t.sat, t.val, u32::from(t.srgb), 0]);
    let flat: Vec<f32> = t.data.iter().flatten().copied().collect();
    let off = w.fs(&flat);
    w.set(at as usize + 4, off);
    at
}

/// The white balance multipliers `o` asks for.
pub(crate) fn wb(meta: &SensorMeta, o: &Options) -> [f32; 3] {
    match o.wb {
        WhiteBalance::AsShot => meta.wb,
        WhiteBalance::None => [1.0; 3],
        WhiteBalance::Multipliers(m) => m,
    }
}

/// The camera → output matrix (Rec.2020) when no profile is used.
pub(crate) fn output_matrix(meta: &SensorMeta) -> [[f32; 3]; 3] {
    scale3(mul3(&REC709_TO_REC2020, &meta.rgb_cam), baseline_gain(meta))
}

/// The DNG `BaselineExposure` as a gain on the front end's output (every sensor; a merge
/// read back from a float DNG has it folded into its samples already, see
/// [`crate::merge::apply_baseline`]). 1 for files without it.
pub(crate) fn baseline_gain(meta: &SensorMeta) -> f32 {
    let be = meta.dng.as_ref().map_or(0.0, |d| d.baseline_exposure);
    if be.is_finite() {
        2f64.powf(be.clamp(-10.0, 10.0)) as f32
    } else {
        1.0
    }
}

/// The DNG profile's matrix after its tables (linear ProPhoto → Rec.2020) with the
/// baseline exposure gain.
pub(crate) fn profile_post(meta: &SensorMeta, p: &specular_raw::ProfileTransform) -> [[f32; 3]; 3] {
    scale3(
        mul3(&REC709_TO_REC2020, &p.prophoto_to_rec709()),
        baseline_gain(meta),
    )
}

fn scale3(m: [[f32; 3]; 3], k: f32) -> [[f32; 3]; 3] {
    m.map(|r| r.map(|v| v * k))
}

/// White-balanced camera RGB → the front end's Rec.2020 as one matrix: the camera matrix, or
/// the DNG profile's matrices without its tables (the geometry warp's camera planes).
pub fn camera_matrix(meta: &SensorMeta) -> [[f32; 3]; 3] {
    match &meta.profile {
        Some(p) => mul3(&profile_post(meta, p), &p.camera_to_prophoto()),
        None => output_matrix(meta),
    }
}

/// Builds `D` for `meta` and `o`.
pub(crate) fn build(meta: &SensorMeta, g: Geometry, o: &Options) -> (Vec<u32>, Skipped) {
    let mut w = W(vec![0; HEADER]);
    let mut skipped = Skipped::new();
    // Layout.
    let (code, cw, ch, colors): (u32, u32, u32, Vec<u32>) = match &meta.layout {
        Layout::Bayer(p) => (0, 2, 2, p.iter().map(|&c| u32::from(c)).collect()),
        Layout::Pattern(c) => (
            1,
            c.width,
            c.height,
            c.colors.iter().map(|&c| u32::from(c)).collect(),
        ),
        Layout::Linear => (2, 1, 1, vec![1]),
        Layout::Mono => (3, 1, 1, vec![1]),
    };
    w.set(H_LAYOUT, code);
    w.set(H_FLOAT, u32::from(g.float));
    w.set(H_CFA_W, cw);
    w.set(H_CFA_H, ch);
    let at = w.at();
    w.0.extend(colors);
    w.set(H_CFA_OFF, at);
    // Black levels.
    let b = &meta.black;
    w.set(H_BL_ROWS, b.repeat.0.max(1));
    w.set(H_BL_COLS, b.repeat.1.max(1));
    w.set(H_BL_CPP, b.cpp.max(1));
    let need = (b.repeat.0.max(1) * b.repeat.1.max(1) * b.cpp.max(1)) as usize;
    let mut levels = b.levels.clone();
    levels.resize(need.max(1), 0.0);
    let at = w.fs(&levels);
    w.set(H_BL_OFF, at);
    w.set(H_BL_OX, b.origin.0);
    w.set(H_BL_OY, b.origin.1);
    let at = w.fs(&b.per_column);
    w.set(H_BC_OFF, at);
    w.set(H_BC_LEN, b.per_column.len() as u32);
    let at = w.fs(&b.per_row);
    w.set(H_BR_OFF, at);
    w.set(H_BR_LEN, b.per_row.len() as u32);
    if let Some(t) = &meta.linearization
        && !t.is_empty()
    {
        let at = w.u16s(t);
        w.set(H_LIN_OFF, at);
        w.set(H_LIN_LEN, t.len() as u32);
    }
    w.setf(H_WHITE, meta.white);
    for (i, m) in wb(meta, o).into_iter().enumerate() {
        w.setf(H_WB + i, m);
    }
    // Opcode lists.
    if o.opcodes {
        let l1: Vec<&OpcodeInfo> = meta.opcodes[0].iter().collect();
        let rest = match l1.first() {
            Some(OpcodeInfo::FixBadPixelsConstant { value, bounds }) => {
                w.set(H_BAD, 1);
                w.set(H_BAD_V, u32::from(*value));
                for (i, v) in [bounds.x, bounds.y, bounds.w, bounds.h]
                    .into_iter()
                    .enumerate()
                {
                    w.set(H_BAD_RECT + i, v);
                }
                &l1[1..]
            }
            _ => &l1[..],
        };
        let l3 = &meta.opcodes[2];
        let l3: Vec<&OpcodeInfo> = l3
            .iter()
            .enumerate()
            .filter(|&(i, _)| !(o.defer_warps && crate::ops::geometry::warp_takes(l3, i)))
            .map(|(_, op)| op)
            .collect();
        for (h, ops) in [
            (H_L1, rest.to_vec()),
            (H_L2, meta.opcodes[1].iter().collect()),
            (H_L3, l3),
        ] {
            let (at, n) = list(&mut w, &ops, &mut skipped);
            w.set(h, at);
            w.set(h + 1, n);
        }
    }
    // Output.
    let m = output_matrix(meta);
    for (i, v) in m.iter().flatten().enumerate() {
        w.setf(H_MAT + i, *v);
    }
    w.set(
        H_OUTPUT,
        match o.output {
            Output::Rec2020 => 0,
            Output::CameraRgb => 1,
            Output::Mosaic => 2,
        },
    );
    if o.profile
        && let Some(p) = &meta.profile
    {
        let at = w.at() as usize;
        w.0.resize(at + 21, 0);
        for (i, v) in p.camera_to_prophoto().iter().flatten().enumerate() {
            w.setf(at + i, *v);
        }
        let post = profile_post(meta, p);
        for (i, v) in post.iter().flatten().enumerate() {
            w.setf(at + 9 + i, *v);
        }
        w.set(at + 18, u32::from(p.is_hdr()));
        if let Some(t) = p.hue_sat_map() {
            let t = table(&mut w, &t);
            w.set(at + 19, t);
        }
        if let Some(t) = p.look_table() {
            let t = table(&mut w, &t);
            w.set(at + 20, t);
        }
        w.set(H_PROF, at as u32);
    }
    w.set(H_W, g.width);
    w.set(H_H, g.height);
    w.set(H_CPP, g.cpp);
    w.set(H_CX, g.crop.x);
    w.set(H_CY, g.crop.y);
    if let Layout::Pattern(c) = &meta.layout
        && c.width == 6
        && c.height == 6
        && c.colors.len() == 36
    {
        let mut words = [0u32; 3];
        for r in 0..6u32 {
            for k in 0..6u32 {
                let at = ((g.crop.y + r) % 6) * 6 + (g.crop.x + k) % 6;
                let col = u32::from(c.colors[at as usize]) & 3;
                words[(r / 2) as usize] |= col << ((r & 1) * 12 + 2 * k);
            }
        }
        for (i, v) in words.iter().enumerate() {
            w.set(H_XT6 + i, *v);
        }
        w.set(H_XT6_ON, 1);
    }
    w.set(H_CW, g.crop.w);
    w.set(H_CH, g.crop.h);
    // At least 16 bytes (a binding's minimum).
    if w.0.len() < 4 {
        w.0.resize(4, 0);
    }
    (w.0, skipped)
}
