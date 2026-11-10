//! AI denoise: the Denoise module's "AI" mode.
//!
//! A denoising network (the app's, `specular_ai::denoise`: NAFNet trained on SIDD) runs on
//! the picture the denoise module sees, and the module blends its result with the classic
//! result (`blend`: 0 classic only, 1 the network's only). The classic result is the draft:
//! it shows wherever the network's result is not there yet.
//!
//! - **Where it runs.** On the whole source frame at full size (no warp: no crop, lens
//!   correction or turns, not even the file's orientation), as the modules before denoise
//!   leave it: linear Rec.2020. So a result follows every geometry change without running
//!   again, as AI masks do; the kernel finds each pixel's frame point through a small grid
//!   ([`grid`], from the graph's placement).
//! - **The network's encoding** ([`net_input`], [`merge`]): Rec.2020 → sRGB primaries,
//!   clipped to 0..1, the sRGB curve (what SIDD's pictures are). The network's change is
//!   added back in linear light, so colours outside sRGB and the network's clipping cost
//!   nothing; pixels above white fade to the classic result (the network never saw them).
//! - **Keys** ([`key`]): a result is found by the photo, the model and every module before
//!   denoise (raw front end, corrections). The sidecar stores mode, model and blend, never
//!   keys: the app keys the edit it renders ([`super::ImageDetail::photo`]).
//! - **Store** ([`store`]): results by key, shared by the preview and exports, bounded in
//!   memory; the app's loader reads its disk cache on a miss. A result fills cell by cell
//!   (the 100 % view's cells first); the kernel uses the classic result in cells not done.
//! - **The table** the kernel reads ([`Store::table`]): header, grid, cell flags, then the
//!   pixels as packed f16 (two words each), built once per result version and grid.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::graph::OpData;
use crate::mask::Geometry;
use crate::model::{Edit, Fields, ModuleKind, Registry, Stage, Value};

/// Version of the key derivation and of the results a key names.
const KEY_CONTEXT: &str = "specular-develop 2026-11-01 ai denoise v1";
/// The table's first word.
pub const MAGIC: u32 = 0x4441_4931;
/// Words before the grid.
pub const HEADER: usize = 12;
/// Grid points per side.
pub const GRID: usize = 17;
/// Bytes of results the store keeps in memory (about two 24 MP photos).
const STORE_BYTES: u64 = 512 << 20;

/// Linear Rec.2020 → linear sRGB (Rec.709 primaries, D65).
pub const TO_SRGB: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641_1, -0.072_849_9],
    [-0.124_550_5, 1.132_899_9, -0.008_349_4],
    [-0.018_150_8, -0.100_578_9, 1.118_729_7],
];
/// Linear sRGB → linear Rec.2020.
pub const FROM_SRGB: [[f32; 3]; 3] = [
    [0.627_403_9, 0.329_283_04, 0.043_313_07],
    [0.069_097_29, 0.919_540_4, 0.011_362_32],
    [0.016_391_44, 0.088_013_31, 0.895_595_3],
];

/// Above this (in sRGB-linear, the brightest channel) the network's change fades out; at
/// white it is gone.
const FADE_FROM: f32 = 0.9;

/// The module's mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Classic,
    Ai,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Classic => "classic",
            Mode::Ai => "ai",
        }
    }

    pub fn of(f: &Fields) -> Mode {
        if f.text("mode") == Some("ai") {
            Mode::Ai
        } else {
            Mode::Classic
        }
    }
}

/// The AI mode's settings of a denoise module's fields.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AiParams {
    pub mode: Mode,
    /// The model's manifest id (empty: the default model).
    pub model: String,
    /// The network's share, 0 … 1.
    pub blend: f32,
    /// The result's key (set per render, never stored).
    pub key: Option<[u8; 32]>,
    /// The frame point of each grid point of the graph's picture (set per graph).
    pub grid: Option<Vec<f32>>,
}

/// Blend when the field is missing.
pub const DEFAULT_BLEND: f32 = 1.0;

impl AiParams {
    pub fn from_fields(f: &Fields) -> AiParams {
        let blend = f.f32_or("blend", DEFAULT_BLEND);
        AiParams {
            mode: Mode::of(f),
            model: f.text("model").unwrap_or("").to_owned(),
            blend: if blend.is_finite() {
                blend.clamp(0.0, 1.0)
            } else {
                DEFAULT_BLEND
            },
            key: f.text("ai_key").and_then(unhex),
            grid: match f.get("ai_grid") {
                Some(Value::Floats(v)) if v.len() == GRID * GRID * 2 => Some(v.clone()),
                _ => None,
            },
        }
    }

    /// Whether the network's result is mixed in.
    pub fn on(&self) -> bool {
        self.mode == Mode::Ai && self.blend > 0.0
    }
}

fn unhex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(k)
}

/// The key's text form (the hidden `ai_key` field, the cache file's name).
pub fn hex(k: &[u8; 32]) -> String {
    crate::model::hash::hex(k)
}

/// The key of the AI result for `edit`'s denoise input on the photo `photo` with `model`:
/// every enabled module before denoise (not the warp) counts.
pub fn key(photo: &[u8; 32], edit: &Edit, model: &str) -> [u8; 32] {
    let reg = Registry::builtin();
    let cut = reg.rank(ModuleKind::DENOISE);
    let mut h = blake3::Hasher::new_derive_key(KEY_CONTEXT);
    h.update(photo);
    h.update(model.as_bytes());
    h.update(&[0]);
    for m in edit.modules.iter().filter(|m| m.enabled) {
        let stage = reg.get(m.kind).map(|s| s.stage);
        if reg.rank(m.kind) >= cut || matches!(stage, Some(Stage::Warp | Stage::Output)) {
            continue;
        }
        h.update(&crate::model::hash::module_hash(edit, m));
    }
    *h.finalize().as_bytes()
}

/// The enabled denoise module in AI mode, if any.
pub fn ai_module(edit: &Edit) -> Option<AiParams> {
    edit.modules
        .iter()
        .find(|m| m.kind == ModuleKind::DENOISE && m.enabled && m.version == 1)
        .map(|m| AiParams::from_fields(&m.params.to_fields()))
        .filter(AiParams::on)
}

/// Whether the graph needs the picture's geometry (an AI denoise result is mixed in).
pub fn needs_geometry(edit: &Edit) -> bool {
    ai_module(edit).is_some()
}

/// The frame point (fractions) of each of the [`GRID`]² points spread over the graph's
/// picture, x then y per point, rows top to bottom.
pub fn grid(geometry: &Geometry<'_>) -> Vec<f32> {
    let mut g = Vec::with_capacity(GRID * GRID * 2);
    for j in 0..GRID {
        for i in 0..GRID {
            let (u, v) = (i as f64 / (GRID - 1) as f64, j as f64 / (GRID - 1) as f64);
            let (x, y) = geometry.frame_point(u, v);
            g.push(x as f32);
            g.push(y as f32);
        }
    }
    g
}

/// The grid of a picture that is the frame.
pub fn identity_grid() -> Vec<f32> {
    grid(&Geometry::Frame { w: 1.0, h: 1.0 })
}

// -- The network's encoding ----------------------------------------------------------------

fn mat(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

/// The sRGB curve.
pub fn oetf(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// The sRGB curve's inverse.
pub fn eotf(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// One pixel as the network takes it.
pub fn encode(x: [f32; 3]) -> [f32; 3] {
    mat(&TO_SRGB, x).map(oetf)
}

/// The denoised pixel from the input `x` (linear Rec.2020) and the network's output `y`
/// for it (encoded): the network's change added in linear light, faded out towards white.
pub fn merge(x: [f32; 3], y: [f32; 3]) -> [f32; 3] {
    let s = mat(&TO_SRGB, x);
    let e = s.map(oetf);
    let d: [f32; 3] = std::array::from_fn(|c| eotf(y[c]) - eotf(e[c]));
    let top = s[0].max(s[1]).max(s[2]);
    let keep = ((1.0 - top) / (1.0 - FADE_FROM)).clamp(0.0, 1.0);
    let d = mat(&FROM_SRGB, d);
    std::array::from_fn(|c| x[c] + keep * d[c])
}

/// How a network takes the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Encoding {
    /// sRGB primaries and curve, 0..1 (NAFNet: SIDD's pictures); the change is added back
    /// in linear light ([`merge`]).
    Srgb,
    /// The generalised Anscombe transform of the picture's noise model per channel, where
    /// the noise is unit and white, divided by `scale` (white's transform, the largest of
    /// the three) so white is about 1 and σ is `1/scale` (FFDNet). Back through the exact
    /// unbiased inverse (Mäkitalo and Foi).
    Gat {
        ia: [f32; 3],
        s2: [f32; 3],
        scale: f32,
        /// Per channel, the lowest value encoded (two read-noise σ below black): darker
        /// values (a demosaic's overshoot at dark edges) are encoded there and the rest is
        /// added back after the network. Below it the transform's linear tail turns them
        /// into outliers of hundreds of σ the network smears around.
        lo: [f32; 3],
    },
}

impl Encoding {
    /// The Anscombe encoding for `noise`.
    pub fn gat(noise: &super::noise::NoiseModel) -> Encoding {
        let n = noise.sane();
        let ia: [f32; 3] = n.a.map(|a| 1.0 / a);
        let s2: [f32; 3] = std::array::from_fn(|c| n.b[c] / (n.a[c] * n.a[c]));
        let scale = (0..3)
            .map(|c| super::noise::gat(1.0, ia[c], s2[c]))
            .fold(1.0f32, f32::max);
        let lo = n.b.map(|b| -2.0 * b.sqrt());
        Encoding::Gat { ia, s2, scale, lo }
    }

    /// The noise level the network is told (0 when it is not told one).
    pub fn sigma(&self) -> f32 {
        match self {
            Encoding::Srgb => 0.0,
            Encoding::Gat { scale, .. } => 1.0 / scale,
        }
    }

    /// One pixel (linear Rec.2020) as the network takes it.
    pub fn encode(&self, x: [f32; 3]) -> [f32; 3] {
        match self {
            Encoding::Srgb => encode(x),
            Encoding::Gat { ia, s2, scale, lo } => {
                std::array::from_fn(|c| super::noise::gat(x[c].max(lo[c]), ia[c], s2[c]) / scale)
            }
        }
    }

    /// The denoised pixel from the input `x` and the network's output `y` for it.
    pub fn merge(&self, x: [f32; 3], y: [f32; 3]) -> [f32; 3] {
        match self {
            Encoding::Srgb => merge(x, y),
            Encoding::Gat { ia, s2, scale, lo } => std::array::from_fn(|c| {
                super::noise::gat_inv(y[c] * scale, ia[c], s2[c], 1.0) + (x[c] - x[c].max(lo[c]))
            }),
        }
    }
}

/// The network's input for a `w × h` RGBA linear picture: planar, encoded by `enc`.
pub fn net_input(w: usize, h: usize, rgba: &[f32], enc: &Encoding) -> Vec<f32> {
    let n = w * h;
    let mut out = vec![0f32; 3 * n];
    for (i, p) in rgba.chunks_exact(4).take(n).enumerate() {
        let e = enc.encode([p[0], p[1], p[2]]);
        for c in 0..3 {
            out[c * n + i] = e[c];
        }
    }
    out
}

// -- Results ---------------------------------------------------------------------------------

/// The network's result over the source frame, filled cell by cell.
#[derive(Clone, Debug, PartialEq)]
pub struct AiResult {
    pub w: u32,
    pub h: u32,
    /// Cell side (pixels).
    pub cell: u32,
    pub cells_x: u32,
    pub cells_y: u32,
    /// Per cell, row-major: done.
    pub valid: Vec<bool>,
    /// Two words per pixel: red and green as f16, then blue as f16.
    pub px: Vec<u32>,
    /// Counts the changes (part of the table's hash).
    pub version: u64,
    /// This result's identity in the process (part of the table's hash, so a result made
    /// again under the same key never reuses another's table).
    pub id: u64,
}

fn next_id() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn pack(v: [f32; 3]) -> [u32; 2] {
    let h = |x: f32| u32::from(half::f16::from_f32(x).to_bits());
    [h(v[0]) | (h(v[1]) << 16), h(v[2])]
}

fn unpack(w: [u32; 2]) -> [f32; 3] {
    let f = |b: u32| half::f16::from_bits((b & 0xFFFF) as u16).to_f32();
    [f(w[0]), f(w[0] >> 16), f(w[1])]
}

impl AiResult {
    /// An empty result for a `w × h` frame in cells of `cell` pixels.
    pub fn new(w: u32, h: u32, cell: u32) -> AiResult {
        let cell = cell.max(1);
        let (cx, cy) = (w.div_ceil(cell), h.div_ceil(cell));
        AiResult {
            w,
            h,
            cell,
            cells_x: cx,
            cells_y: cy,
            valid: vec![false; (cx * cy) as usize],
            px: vec![0; 2 * w as usize * h as usize],
            version: 0,
            id: next_id(),
        }
    }

    pub fn bytes(&self) -> u64 {
        self.px.len() as u64 * 4 + self.valid.len() as u64
    }

    /// Writes the network's output `out` (planar, encoded by `enc`) over the rectangle
    /// `(x, y, rw, rh)`, merged with the input (`input`, the whole frame's linear RGBA), and
    /// marks the cells the rectangle covers whole.
    #[allow(clippy::too_many_arguments)]
    pub fn put(
        &mut self,
        (x, y, rw, rh): (u32, u32, u32, u32),
        out: &[f32],
        input: &[f32],
        enc: &Encoding,
    ) {
        let (w, n) = (self.w as usize, (rw * rh) as usize);
        for r in 0..rh as usize {
            let fy = y as usize + r;
            if fy >= self.h as usize {
                break;
            }
            for k in 0..rw as usize {
                let fx = x as usize + k;
                if fx >= w {
                    break;
                }
                let i = fy * w + fx;
                let o = r * rw as usize + k;
                let inp = [input[4 * i], input[4 * i + 1], input[4 * i + 2]];
                let v = enc.merge(inp, [out[o], out[n + o], out[2 * n + o]]);
                let p = pack(v);
                self.px[2 * i] = p[0];
                self.px[2 * i + 1] = p[1];
            }
        }
        let c = self.cell;
        let x1 = (x + rw).min(self.w);
        let y1 = (y + rh).min(self.h);
        for cy in y.div_ceil(c)..self.cells_y {
            if (cy * c + c).min(self.h) > y1 {
                break;
            }
            for cx in x.div_ceil(c)..self.cells_x {
                if (cx * c + c).min(self.w) > x1 {
                    break;
                }
                self.valid[(cy * self.cells_x + cx) as usize] = true;
            }
        }
        self.version += 1;
    }

    /// Whether every cell is done.
    pub fn complete(&self) -> bool {
        self.valid.iter().all(|&v| v)
    }

    /// Whether every cell of the rectangle `[x0, x1) × [y0, y1)` (cells) is done.
    pub fn covers(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> bool {
        (y0..y1.min(self.cells_y)).all(|cy| {
            (x0..x1.min(self.cells_x)).all(|cx| self.valid[(cy * self.cells_x + cx) as usize])
        })
    }

    /// The pixel at `(x, y)` (linear Rec.2020), None when its cell is not done.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[f32; 3]> {
        if x >= self.w || y >= self.h {
            return None;
        }
        let c = (y / self.cell) * self.cells_x + x / self.cell;
        if !self.valid[c as usize] {
            return None;
        }
        let i = 2 * (y as usize * self.w as usize + x as usize);
        Some(unpack([self.px[i], self.px[i + 1]]))
    }

    /// The kernel's table with `grid` ([`grid`]).
    fn table(&self, key: &[u8; 32], grid: &[f32]) -> OpData {
        let cells = self.valid.len();
        let g0 = HEADER;
        let v0 = g0 + grid.len();
        let p0 = v0 + cells;
        let mut words = Vec::with_capacity(p0 + self.px.len());
        let u = f32::from_bits;
        words.extend_from_slice(&[
            u(MAGIC),
            u(self.w),
            u(self.h),
            u(self.cell),
            u(self.cells_x),
            u(self.cells_y),
            u(GRID as u32),
            u(g0 as u32),
            u(v0 as u32),
            u(p0 as u32),
            0.0,
            0.0,
        ]);
        words.extend_from_slice(grid);
        words.extend(self.valid.iter().map(|&v| u(u32::from(v))));
        words.extend(self.px.iter().map(|&p| u(p)));
        let mut h =
            blake3::Hasher::new_derive_key("specular-develop 2026-11-01 ai denoise table v1");
        h.update(key);
        h.update(&self.id.to_le_bytes());
        h.update(&self.version.to_le_bytes());
        for g in grid {
            h.update(&g.to_bits().to_le_bytes());
        }
        OpData {
            hash: *h.finalize().as_bytes(),
            words,
        }
    }

    /// The disk cache's form: a header, the cell flags, the pixels.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(32 + self.valid.len() + self.px.len() * 4);
        b.extend_from_slice(b"SPDN0001");
        for v in [self.w, self.h, self.cell, self.cells_x, self.cells_y] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend(self.valid.iter().map(|&v| u8::from(v)));
        for p in &self.px {
            b.extend_from_slice(&p.to_le_bytes());
        }
        b
    }

    /// From [`AiResult::encode`]'s bytes; None when they are not one.
    pub fn decode(b: &[u8]) -> Option<AiResult> {
        let rest = b.strip_prefix(b"SPDN0001")?;
        let word = |i: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                rest.get(4 * i..4 * i + 4)?.try_into().ok()?,
            ))
        };
        let (w, h, cell, cx, cy) = (word(0)?, word(1)?, word(2)?, word(3)?, word(4)?);
        if w == 0 || h == 0 || cell == 0 || cx != w.div_ceil(cell) || cy != h.div_ceil(cell) {
            return None;
        }
        let cells = (cx as usize).checked_mul(cy as usize)?;
        let n = (w as usize).checked_mul(h as usize)?.checked_mul(2)?;
        let body = rest.get(20..)?;
        if body.len() != cells + 4 * n {
            return None;
        }
        let valid = body[..cells].iter().map(|&v| v != 0).collect();
        let px = body[cells..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Some(AiResult {
            w,
            h,
            cell,
            cells_x: cx,
            cells_y: cy,
            valid,
            px,
            version: 1,
            id: next_id(),
        })
    }
}

/// Reads a result from the app's disk cache on a store miss.
pub type Loader = Box<dyn Fn(&[u8; 32]) -> Option<AiResult> + Send>;

/// A table made: its key, the result's version, the grid, the table.
type TableMemo = ([u8; 32], u64, Vec<f32>, Arc<OpData>);

/// Results by key, bounded in memory, and the last tables made from them.
#[derive(Default)]
pub struct Store {
    entries: Vec<([u8; 32], AiResult)>,
    loader: Option<Loader>,
    /// Keys the loader found nothing for (not asked again until a put).
    misses: HashMap<[u8; 32], ()>,
    tables: Vec<TableMemo>,
}

impl Store {
    fn find(&mut self, key: &[u8; 32]) -> Option<usize> {
        if let Some(i) = self.entries.iter().position(|e| e.0 == *key) {
            // Most recently used last.
            let e = self.entries.remove(i);
            self.entries.push(e);
            return Some(self.entries.len() - 1);
        }
        if self.misses.contains_key(key) {
            return None;
        }
        match self.loader.as_ref().and_then(|l| l(key)) {
            Some(r) => {
                self.insert(*key, r);
                Some(self.entries.len() - 1)
            }
            None => {
                self.misses.insert(*key, ());
                None
            }
        }
    }

    fn insert(&mut self, key: [u8; 32], r: AiResult) {
        self.entries.retain(|e| e.0 != key);
        self.misses.remove(&key);
        self.entries.push((key, r));
        while self.entries.len() > 1 && self.bytes() > STORE_BYTES {
            let (k, _) = self.entries.remove(0);
            self.tables.retain(|t| t.0 != k);
        }
    }

    /// Bytes of results held.
    pub fn bytes(&self) -> u64 {
        self.entries.iter().map(|e| e.1.bytes()).sum()
    }

    /// Sets the disk cache's reader.
    pub fn set_loader(&mut self, l: Option<Loader>) {
        self.loader = l;
        self.misses.clear();
    }

    /// The result for `key` (from memory or the loader), copied.
    pub fn get(&mut self, key: &[u8; 32]) -> Option<AiResult> {
        let i = self.find(key)?;
        Some(self.entries[i].1.clone())
    }

    /// Runs `f` on the result for `key`, made empty for a `w × h` frame in `cell` cells
    /// when there is none (or it has another size).
    pub fn update<R>(
        &mut self,
        key: &[u8; 32],
        (w, h, cell): (u32, u32, u32),
        f: impl FnOnce(&mut AiResult) -> R,
    ) -> R {
        let fits = |r: &AiResult| r.w == w && r.h == h && r.cell == cell.max(1);
        match self.find(key) {
            Some(i) if fits(&self.entries[i].1) => {}
            _ => self.insert(*key, AiResult::new(w, h, cell)),
        }
        let i = self.entries.len() - 1;
        f(&mut self.entries[i].1)
    }

    /// Cells of `key`'s result done, and their count (None: no result).
    pub fn progress(&mut self, key: &[u8; 32]) -> Option<(usize, usize)> {
        let i = self.find(key)?;
        let v = &self.entries[i].1.valid;
        Some((v.iter().filter(|&&b| b).count(), v.len()))
    }

    /// Whether `key` has a result with every cell of `[x0, x1) × [y0, y1)` done.
    pub fn covers(&mut self, key: &[u8; 32], cells: (u32, u32, u32, u32)) -> bool {
        self.find(key)
            .is_some_and(|i| self.entries[i].1.covers(cells.0, cells.1, cells.2, cells.3))
    }

    /// Whether `key` has a result with every cell under the frame fractions `[x0, y0, x1,
    /// y1]` done.
    pub fn covers_fraction(&mut self, key: &[u8; 32], f: [f64; 4]) -> bool {
        let Some(i) = self.find(key) else {
            return false;
        };
        let r = &self.entries[i].1;
        let c = f64::from(r.cell);
        let (w, h) = (f64::from(r.w), f64::from(r.h));
        let cx = |v: f64, n: f64| ((v.clamp(0.0, 1.0) * n) / c) as u32;
        let x0 = cx(f[0], w);
        let y0 = cx(f[1], h);
        let x1 = ((f[2].clamp(0.0, 1.0) * w) / c).ceil() as u32;
        let y1 = ((f[3].clamp(0.0, 1.0) * h) / c).ceil() as u32;
        r.covers(x0, y0, x1.max(x0 + 1), y1.max(y0 + 1))
    }

    pub fn remove(&mut self, key: &[u8; 32]) {
        self.entries.retain(|e| e.0 != *key);
        self.tables.retain(|t| t.0 != *key);
    }

    /// The kernel's table for `key`'s result on a picture with `grid`: made once per
    /// result version and grid, the last few kept.
    pub fn table(&mut self, key: &[u8; 32], grid: &[f32]) -> Option<Arc<OpData>> {
        let i = self.find(key)?;
        let version = self.entries[i].1.version ^ (self.entries[i].1.id << 32);
        if let Some(t) = self
            .tables
            .iter()
            .find(|t| t.0 == *key && t.1 == version && t.2 == grid)
        {
            return Some(t.3.clone());
        }
        let d = Arc::new(self.entries[i].1.table(key, grid));
        self.tables.retain(|t| t.0 != *key || t.2 != grid);
        self.tables.push((*key, version, grid.to_vec(), d.clone()));
        if self.tables.len() > 3 {
            self.tables.remove(0);
        }
        Some(d)
    }
}

/// The process's results, shared by the preview and exports.
pub fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// What runs the network (the app's, with its installed model): fills `key`'s result in
/// the [`store`] from the whole frame's denoise input.
pub trait Runner: Send + Sync {
    /// Runs `model` over the cells covering the frame pixels `rect` (`[x0, y0, x1, y1]`;
    /// None: the whole frame) of `input` (the frame's linear Rec.2020 RGBA, `w × h`, whose
    /// noise is `noise`), putting each finished row of cells into `key`'s result.
    /// `progress(fraction)` returns false to stop: then `Ok(false)`.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        key: &[u8; 32],
        model: &str,
        size: (u32, u32),
        input: &[f32],
        noise: Option<super::noise::NoiseModel>,
        rect: Option<[u32; 4]>,
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<bool, String>;
}

/// The process's runner (None: AI denoise shows the classic result).
pub fn runner() -> &'static Mutex<Option<Arc<dyn Runner>>> {
    static R: OnceLock<Mutex<Option<Arc<dyn Runner>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(None))
}

/// The key of `edit`'s AI denoise result when its whole frame is not done yet (what an
/// export must run first), with the model.
pub fn pending(edit: &Edit) -> Option<([u8; 32], String)> {
    let a = ai_module(edit)?;
    let key = a.key?;
    let mut st = store().lock().ok()?;
    let done = st.progress(&key).is_some_and(|(d, n)| d == n);
    (!done).then_some((key, a.model))
}

/// A photo's identity from its path, size and modification time ([`crate::mask::ai::photo_id`]
/// of what the file system says), as the app keys results.
pub fn photo_of_path(path: &std::path::Path) -> [u8; 32] {
    let (mut len, mut ns) = (0, 0);
    if let Ok(m) = std::fs::metadata(path) {
        len = m.len();
        if let Ok(t) = m.modified()
            && let Ok(d) = t.duration_since(std::time::UNIX_EPOCH)
        {
            ns = d.as_nanos();
        }
    }
    crate::mask::ai::photo_id(path.to_string_lossy().as_bytes(), len, ns)
}

/// The table for these params (their key and grid), when a result is there.
pub fn table_for(p: &AiParams) -> Option<Arc<OpData>> {
    if !p.on() {
        return None;
    }
    let key = p.key?;
    let grid = p.grid.clone().unwrap_or_else(identity_grid);
    store().lock().ok()?.table(&key, &grid)
}

/// Sets the hidden fields: the key (or none), the grid (or none).
pub fn write_hidden(f: &mut Fields, key: Option<&[u8; 32]>, grid: Option<Vec<f32>>) {
    match key {
        Some(k) => f.set("ai_key", Value::Text(hex(k))),
        None => {
            f.remove("ai_key");
        }
    }
    if let Some(g) = grid {
        f.set("ai_grid", Value::Floats(g));
    }
}

/// The kernel's helpers: the AI result at image pixel `(x, y)` (rgb, and 1 where it is
/// there, else 0). The table is the node's `dt`.
pub const AI_WGSL: &str = "fn dv_ai_u(i: u32) -> u32 { return bitcast<u32>(dt(i)); }
fn dv_ai_frame(px: i32, py: i32) -> vec2<f32> {
    let n = dv_ai_u(6u);
    let go = dv_ai_u(7u);
    let fx = clamp((f32(px) + 0.5) / f32(pu(4u)), 0.0, 1.0) * f32(n - 1u);
    let fy = clamp((f32(py) + 0.5) / f32(pu(5u)), 0.0, 1.0) * f32(n - 1u);
    let i0 = min(u32(floor(fx)), n - 2u);
    let j0 = min(u32(floor(fy)), n - 2u);
    let tx = fx - f32(i0);
    let ty = fy - f32(j0);
    let a = go + 2u * (j0 * n + i0);
    let b = a + 2u;
    let c = a + 2u * n;
    let d = c + 2u;
    let top = mix(vec2<f32>(dt(a), dt(a + 1u)), vec2<f32>(dt(b), dt(b + 1u)), tx);
    let bot = mix(vec2<f32>(dt(c), dt(c + 1u)), vec2<f32>(dt(d), dt(d + 1u)), tx);
    return mix(top, bot, ty);
}
fn dv_ai_tex(x: i32, y: i32) -> vec4<f32> {
    let w = dv_ai_u(1u);
    let h = dv_ai_u(2u);
    let cx = u32(clamp(x, 0, i32(w) - 1));
    let cy = u32(clamp(y, 0, i32(h) - 1));
    let cell = dv_ai_u(3u);
    let ok = dv_ai_u(dv_ai_u(8u) + (cy / cell) * dv_ai_u(4u) + cx / cell);
    if (ok == 0u) { return vec4<f32>(0.0); }
    let i = dv_ai_u(9u) + 2u * (cy * w + cx);
    let rg = unpack2x16float(dv_ai_u(i));
    let b = unpack2x16float(dv_ai_u(i + 1u));
    return vec4<f32>(rg.x, rg.y, b.x, 1.0);
}
fn dv_ai_at(px: i32, py: i32) -> vec4<f32> {
    if (dv_ai_u(0u) != 0x44414931u) { return vec4<f32>(0.0); }
    let f = dv_ai_frame(px, py);
    let gx = f.x * f32(dv_ai_u(1u)) - 0.5;
    let gy = f.y * f32(dv_ai_u(2u)) - 0.5;
    let x0 = floor(gx);
    let y0 = floor(gy);
    let tx = gx - x0;
    let ty = gy - y0;
    let xi = i32(x0);
    let yi = i32(y0);
    let a = dv_ai_tex(xi, yi);
    let b = dv_ai_tex(xi + 1, yi);
    let c = dv_ai_tex(xi, yi + 1);
    let d = dv_ai_tex(xi + 1, yi + 1);
    if (a.w * b.w * c.w * d.w == 0.0) { return vec4<f32>(0.0); }
    return mix(mix(a, b, tx), mix(c, d, tx), ty);
}
";

/// The CPU mirror of [`AI_WGSL`]'s `dv_ai_at` (tests): the result at image pixel `(px, py)`
/// of a `iw × ih` picture, from a table's words.
pub fn sample(words: &[f32], (iw, ih): (u32, u32), px: i32, py: i32) -> Option<[f32; 3]> {
    let u = |i: usize| words[i].to_bits() as usize;
    if words.len() < HEADER || words[0].to_bits() != MAGIC {
        return None;
    }
    let (n, go) = (u(6), u(7));
    let fx = ((px as f32 + 0.5) / iw as f32).clamp(0.0, 1.0) * (n - 1) as f32;
    let fy = ((py as f32 + 0.5) / ih as f32).clamp(0.0, 1.0) * (n - 1) as f32;
    let i0 = (fx.floor() as usize).min(n - 2);
    let j0 = (fy.floor() as usize).min(n - 2);
    let (tx, ty) = (fx - i0 as f32, fy - j0 as f32);
    let g = |i: usize, j: usize, k: usize| words[go + 2 * (j * n + i) + k];
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let f: [f32; 2] = std::array::from_fn(|k| {
        lerp(
            lerp(g(i0, j0, k), g(i0 + 1, j0, k), tx),
            lerp(g(i0, j0 + 1, k), g(i0 + 1, j0 + 1, k), tx),
            ty,
        )
    });
    let (w, h, cell, cx) = (u(1), u(2), u(3), u(4));
    let tex = |x: i64, y: i64| -> Option<[f32; 3]> {
        let x = x.clamp(0, w as i64 - 1) as usize;
        let y = y.clamp(0, h as i64 - 1) as usize;
        if u(u(8) + (y / cell) * cx + x / cell) == 0 {
            return None;
        }
        let i = u(9) + 2 * (y * w + x);
        Some(unpack([words[i].to_bits(), words[i + 1].to_bits()]))
    };
    let gx = f[0] * w as f32 - 0.5;
    let gy = f[1] * h as f32 - 0.5;
    let (x0, y0) = (gx.floor(), gy.floor());
    let (tx, ty) = (gx - x0, gy - y0);
    let (xi, yi) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (
        tex(xi, yi)?,
        tex(xi + 1, yi)?,
        tex(xi, yi + 1)?,
        tex(xi + 1, yi + 1)?,
    );
    Some(std::array::from_fn(|k| {
        lerp(lerp(a[k], b[k], tx), lerp(c[k], d[k], tx), ty)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_anscombe_encoding_round_trips_and_whitens() {
        let n = crate::ops::detail::NoiseModel {
            a: [4.0e-4, 2.0e-4, 3.0e-4],
            b: [1.0e-6, 5.0e-7, 8.0e-7],
        };
        let e = Encoding::gat(&n);
        let Encoding::Gat { scale, .. } = e else {
            panic!("gat")
        };
        // White's transform of the noisiest-free channel is 1.
        let w = e.encode([1.0; 3]);
        assert!(
            (w.iter().fold(0f32, |a, &b| a.max(b)) - 1.0).abs() < 1e-6,
            "{w:?}"
        );
        assert!((e.sigma() - 1.0 / scale).abs() < 1e-9);
        // A clean value comes back (the exact unbiased inverse differs from the algebraic
        // one by little away from black).
        for v in [0.05f32, 0.2, 0.7, 1.3] {
            let back = e.merge([v; 3], e.encode([v; 3]));
            for c in 0..3 {
                assert!((back[c] - v).abs() < 5e-3 * v.max(0.05), "{v}: {back:?}");
            }
        }
    }

    #[test]
    fn primaries_round_trip() {
        for v in [[0.2f32, 0.5, 0.7], [1.0, 0.0, 0.0], [0.01, 0.02, 0.0]] {
            let back = mat(&FROM_SRGB, mat(&TO_SRGB, v));
            for c in 0..3 {
                assert!((back[c] - v[c]).abs() < 1e-5, "{v:?} → {back:?}");
            }
        }
        for v in [0.0f32, 0.001, 0.2, 0.5, 1.0] {
            assert!((eotf(oetf(v)) - v).abs() < 1e-6);
        }
    }

    #[test]
    fn merge_keeps_what_the_network_keeps() {
        // The network changing nothing changes nothing, even out of sRGB's gamut.
        for x in [[0.3f32, 0.2, 0.1], [0.05, 0.4, 0.02], [0.8, 0.85, 0.7]] {
            let y = encode(x);
            let m = merge(x, y);
            for c in 0..3 {
                assert!((m[c] - x[c]).abs() < 1e-5, "{x:?} → {m:?}");
            }
        }
        // A grey moved by the network moves in linear light.
        let x = [0.18f32; 3];
        let y = encode([0.2; 3]);
        let m = merge(x, y);
        assert!((m[1] - 0.2).abs() < 2e-3, "{m:?}");
        // Above white the change is gone.
        let x = [1.4f32, 1.2, 1.1];
        let m = merge(x, [0.0; 3]);
        assert_eq!(m, x);
    }

    #[test]
    fn cells_fill_and_codec_round_trips() {
        let (w, h) = (100u32, 70u32);
        let mut r = AiResult::new(w, h, 32);
        assert_eq!((r.cells_x, r.cells_y), (4, 3));
        let input: Vec<f32> = (0..w * h * 4).map(|i| (i % 97) as f32 / 200.0).collect();
        // Rows 32..64 of the whole width: the middle row of cells.
        let out = vec![0.5f32; 3 * 100 * 32];
        r.put((0, 32, 100, 32), &out, &input, &Encoding::Srgb);
        assert!(r.covers(0, 1, 4, 2) && !r.covers(0, 0, 4, 1) && !r.complete());
        assert!(r.pixel(10, 40).is_some() && r.pixel(10, 10).is_none());
        // The last row of cells is short (rows 64..70).
        let out = vec![0.5f32; 3 * 100 * 6];
        r.put((0, 64, 100, 6), &out, &input, &Encoding::Srgb);
        assert!(r.covers(0, 2, 4, 3));
        let back = AiResult::decode(&r.encode()).unwrap();
        assert_eq!(back.px, r.px);
        assert_eq!(back.valid, r.valid);
        assert!(AiResult::decode(b"SPDN0001xx").is_none());
        assert!(AiResult::decode(&r.encode()[..50]).is_none());
    }

    #[test]
    fn the_table_samples_the_result_through_the_grid() {
        let (w, h) = (64u32, 48u32);
        let mut r = AiResult::new(w, h, 16);
        let input = vec![0.25f32; (w * h * 4) as usize];
        // A horizontal ramp from the network (encoded).
        let mut out = vec![0f32; 3 * (w * h) as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                for c in 0..3 {
                    out[c * (w * h) as usize + y * w as usize + x] = oetf(x as f32 / 64.0);
                }
            }
        }
        r.put((0, 0, w, h), &out, &input, &Encoding::Srgb);
        let key = [7u8; 32];
        let t = r.table(&key, &identity_grid());
        // The picture is the frame at half size: pixel (8, 5) is texels (16.5, 10.5).
        let s = sample(&t.words, (32, 24), 8, 5).unwrap();
        let want = (r.pixel(16, 10).unwrap()[0] + r.pixel(17, 10).unwrap()[0]) / 2.0;
        assert!((s[0] - want).abs() < 1e-3, "{s:?} vs {want}");
        // Cells not done sample as nothing.
        let mut part = AiResult::new(w, h, 16);
        part.put((0, 0, 32, 16), &out, &input, &Encoding::Srgb);
        let t = part.table(&key, &identity_grid());
        assert!(sample(&t.words, (64, 48), 4, 4).is_some());
        assert!(sample(&t.words, (64, 48), 50, 40).is_none());
    }

    #[test]
    fn keys_follow_the_modules_before_denoise() {
        let reg = Registry::builtin();
        let mut e = Edit::default_for(reg, crate::model::SourceKey::default(), true);
        let photo = [1u8; 32];
        let k0 = key(&photo, &e, "ffdnet-color");
        assert_ne!(k0, key(&photo, &e, "other-model"));
        assert_ne!(k0, key(&[2u8; 32], &e, "ffdnet-color"));
        // Exposure (after denoise) changes nothing.
        let mut m = crate::model::ModuleInstance::new(reg, ModuleKind::EXPOSURE, 0);
        m.params = crate::model::Params::from_fields(
            ModuleKind::EXPOSURE,
            m.params.to_fields().with("exposure_ev", Value::Float(1.0)),
        );
        e.insert(reg, m);
        assert_eq!(k0, key(&photo, &e, "ffdnet-color"));
        // The raw front end does.
        let fe = e
            .modules
            .iter_mut()
            .find(|m| m.kind == ModuleKind::RAW_FRONT_END)
            .unwrap();
        fe.enabled = false;
        assert_ne!(k0, key(&photo, &e, "ffdnet-color"));
        let k = [0xabu8; 32];
        assert_eq!(unhex(&hex(&k)), Some(k));
    }
}
