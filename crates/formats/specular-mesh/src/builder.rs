//! Collects vertices, triangles and materials from a reader and finishes them into a
//! [`Mesh`]: caps, invalid triangles dropped, triangles grouped by material, normals where
//! missing, bounds.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};

use rayon::prelude::*;

use crate::{Bounds, Error, ImageData, Limits, Material, Mesh, Part, Result, Unit, Up};

pub(crate) struct Builder {
    pub limits: Limits,
    pub positions: Vec<[f32; 3]>,
    /// One per position; `[0, 0, 0]` where the file gave none.
    pub normals: Vec<[f32; 3]>,
    pub colors: Option<Vec<[u8; 4]>>,
    pub uvs: Option<Vec<[f32; 2]>>,
    pub indices: Vec<u32>,
    /// (first triangle, material) at each change of material.
    runs: Vec<(u32, Option<u32>)>,
    pub materials: Vec<Material>,
    pub images: Vec<ImageData>,
    pub notes: Vec<String>,
    pub unit: Unit,
    pub up: Up,
    pub flat: bool,
    /// Vertices sharing a key share a computed normal: an OBJ's split vertices (one position,
    /// several texture coordinates) stay smooth across texture seams.
    pub normal_key: Option<Vec<u32>>,
    pub title: Option<String>,
    pub generator: Option<String>,
    /// Set when any vertex was given a normal.
    any_normal: bool,
    /// Set when any vertex was given none.
    missing_normal: bool,
    /// The memory budget's share of the arrays above, grown as they grow.
    res: specular_core::Reservation,
}

/// Bytes per vertex across positions, normals, colours, texture coordinates and the normal key.
const VERTEX_BYTES: usize = 12 + 12 + 4 + 8 + 4;

impl Builder {
    pub fn new(limits: &Limits) -> Builder {
        Builder {
            limits: *limits,
            positions: Vec::new(),
            normals: Vec::new(),
            colors: None,
            uvs: None,
            indices: Vec::new(),
            runs: Vec::new(),
            materials: Vec::new(),
            images: Vec::new(),
            notes: Vec::new(),
            unit: Unit::Unknown,
            up: Up::Z,
            flat: false,
            normal_key: None,
            title: None,
            generator: None,
            any_normal: false,
            missing_normal: false,
            res: specular_core::Reservation::none(),
        }
    }

    /// Bytes the vertex and index arrays hold now.
    fn bytes(&self) -> usize {
        self.positions.capacity() * 12
            + self.normals.capacity() * 12
            + self.colors.as_ref().map_or(0, |c| c.capacity() * 4)
            + self.uvs.as_ref().map_or(0, |c| c.capacity() * 8)
            + self.normal_key.as_ref().map_or(0, |c| c.capacity() * 4)
            + self.indices.capacity() * 4
    }

    /// Grows the reservation to the arrays' size plus `more` bytes about to be allocated.
    fn account(&mut self, more: usize) -> Result<()> {
        let want = self.bytes().saturating_add(more) as u64;
        Ok(self.res.grow_to(want)?)
    }

    /// Room for `n` more vertices, refused over the cap.
    pub fn reserve_vertices(&mut self, n: usize) -> Result<()> {
        if self.positions.len().saturating_add(n) > self.limits.max_vertices {
            return Err(too_many_vertices(&self.limits));
        }
        self.account(n.saturating_mul(VERTEX_BYTES))?;
        self.positions.reserve(n);
        self.normals.reserve(n);
        Ok(())
    }

    pub fn reserve_triangles(&mut self, n: usize) -> Result<()> {
        if self.indices.len() / 3 + n > self.limits.max_triangles {
            return Err(too_many_triangles(&self.limits));
        }
        self.account(n.saturating_mul(12))?;
        self.indices.reserve(n.saturating_mul(3));
        Ok(())
    }

    pub fn vertex_count(&self) -> u32 {
        self.positions.len() as u32
    }

    pub fn vertex(
        &mut self,
        p: [f32; 3],
        n: Option<[f32; 3]>,
        c: Option<[u8; 4]>,
        uv: Option<[f32; 2]>,
    ) -> Result<u32> {
        let i = self.positions.len();
        if i >= self.limits.max_vertices {
            return Err(too_many_vertices(&self.limits));
        }
        if i == self.positions.capacity() {
            // The arrays are about to double.
            self.account(self.bytes())?;
        }
        self.positions.push(p);
        match n {
            Some(n) => {
                self.any_normal = true;
                self.normals.push(n);
            }
            None => {
                self.missing_normal = true;
                self.normals.push([0.0; 3]);
            }
        }
        match (c, &mut self.colors) {
            (Some(c), Some(cs)) => cs.push(c),
            (Some(c), None) => {
                let mut cs = vec![[255u8; 4]; i];
                cs.push(c);
                self.colors = Some(cs);
            }
            (None, Some(cs)) => cs.push([255; 4]),
            (None, None) => {}
        }
        match (uv, &mut self.uvs) {
            (Some(t), Some(ts)) => ts.push(t),
            (Some(t), None) => {
                let mut ts = vec![[0f32; 2]; i];
                ts.push(t);
                self.uvs = Some(ts);
            }
            (None, Some(ts)) => ts.push([0.0; 2]),
            (None, None) => {}
        }
        Ok(i as u32)
    }

    /// Marks every vertex added so far as having a file normal (bulk readers that fill
    /// [`Builder::normals`] themselves).
    pub fn normals_given(&mut self, all: bool) {
        if all {
            self.any_normal = true;
        } else {
            self.missing_normal = true;
        }
    }

    /// Material for the triangles that follow.
    pub fn material(&mut self, m: Option<u32>) {
        let first = (self.indices.len() / 3) as u32;
        match self.runs.last_mut() {
            Some((_, cur)) if *cur == m => {}
            Some((f, cur)) if *f == first => *cur = m,
            _ => self.runs.push((first, m)),
        }
    }

    pub fn add_material(&mut self, m: Material) -> u32 {
        self.materials.push(m);
        (self.materials.len() - 1) as u32
    }

    /// Adds an image, `None` over the image cap (noted once).
    pub fn add_image(&mut self, image: ImageData) -> Option<u32> {
        if self.images.len() >= self.limits.max_images {
            self.note_once("too many textures; the rest are not shown");
            return None;
        }
        self.images.push(image);
        Some((self.images.len() - 1) as u32)
    }

    pub fn triangle(&mut self, a: u32, b: u32, c: u32) -> Result<()> {
        if self.indices.len() / 3 >= self.limits.max_triangles {
            return Err(too_many_triangles(&self.limits));
        }
        if self.indices.len() + 3 > self.indices.capacity() {
            self.account(self.indices.capacity().max(4) * 4)?;
        }
        self.indices.extend_from_slice(&[a, b, c]);
        Ok(())
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn note(&mut self, s: impl Into<String>) {
        self.notes.push(s.into());
    }

    pub fn note_once(&mut self, s: &str) {
        if !self.notes.iter().any(|n| n == s) {
            self.notes.push(s.to_owned());
        }
    }

    pub fn finish(mut self) -> Result<Mesh> {
        if self.normals.len() != self.positions.len() {
            // Bulk readers that set positions but no normals.
            self.normals.resize(self.positions.len(), [0.0; 3]);
            self.missing_normal = true;
        }
        self.drop_invalid();
        if self.indices.is_empty() {
            return Err(Error::Empty);
        }
        // Regrouping copies the indices; normals take a pass of their own.
        self.account(self.indices.len() * 4 + self.positions.len() * 12)?;
        let parts = self.group_parts();
        let file_normals = self.any_normal && !self.missing_normal;
        self.fix_normals();
        let bounds = bounds_of(&self.positions, &self.indices);
        Ok(Mesh {
            format: None,
            positions: self.positions,
            normals: self.normals,
            colors: self.colors,
            uvs: self.uvs,
            indices: self.indices,
            parts,
            materials: self.materials,
            images: self.images,
            unit: self.unit,
            up: self.up,
            bounds,
            flat: self.flat,
            file_normals,
            notes: self.notes,
            title: self.title,
            generator: self.generator,
        })
    }

    /// Drops triangles with an index out of range or a corner that is not finite.
    fn drop_invalid(&mut self) {
        let n = self.positions.len();
        let pos = &self.positions;
        let ok = |t: &[u32]| {
            t.iter()
                .all(|&i| (i as usize) < n && pos[i as usize].iter().all(|v| v.is_finite()))
        };
        if self.indices.par_chunks_exact(3).all(ok) {
            self.indices.truncate(self.indices.len() / 3 * 3);
            return;
        }
        let mut out = Vec::with_capacity(self.indices.len());
        let mut runs = Vec::with_capacity(self.runs.len());
        let mut r = 0;
        let mut dropped = 0usize;
        for (t, tri) in self.indices.chunks_exact(3).enumerate() {
            while r < self.runs.len() && self.runs[r].0 as usize <= t {
                let first = (out.len() / 3) as u32;
                match runs.last_mut() {
                    Some((f, m)) if *f == first => *m = self.runs[r].1,
                    _ => runs.push((first, self.runs[r].1)),
                }
                r += 1;
            }
            if ok(tri) {
                out.extend_from_slice(tri);
            } else {
                dropped += 1;
            }
        }
        self.indices = out;
        self.runs = runs;
        self.notes
            .push(format!("{dropped} invalid triangles dropped"));
    }

    /// Orders triangles so each material's are contiguous, keeping their order otherwise.
    fn group_parts(&mut self) -> Vec<Part> {
        let tris = (self.indices.len() / 3) as u32;
        if self.runs.is_empty() || self.runs[0].0 != 0 {
            self.runs.insert(0, (0, None));
        }
        // Runs as (first, count, material), empty ones gone.
        let mut runs: Vec<(u32, u32, Option<u32>)> = Vec::with_capacity(self.runs.len());
        for (i, &(first, m)) in self.runs.iter().enumerate() {
            let end = self.runs.get(i + 1).map_or(tris, |r| r.0).min(tris);
            if end > first {
                runs.push((first, end - first, m));
            }
        }
        // Material order: first appearance.
        let mut order: Vec<Option<u32>> = Vec::new();
        for r in &runs {
            if !order.contains(&r.2) {
                order.push(r.2);
            }
        }
        if order.len() == runs.len() {
            return runs
                .into_iter()
                .map(|(first, count, material)| Part {
                    first,
                    count,
                    material,
                })
                .collect();
        }
        let mut indices = Vec::with_capacity(self.indices.len());
        let mut parts = Vec::with_capacity(order.len());
        for m in order {
            let first = (indices.len() / 3) as u32;
            for r in runs.iter().filter(|r| r.2 == m) {
                let s = r.0 as usize * 3;
                indices.extend_from_slice(&self.indices[s..s + r.1 as usize * 3]);
            }
            parts.push(Part {
                first,
                count: (indices.len() / 3) as u32 - first,
                material: m,
            });
        }
        self.indices = indices;
        parts
    }

    /// Normalises file normals and computes the missing ones (area-weighted).
    fn fix_normals(&mut self) {
        let needs = self.missing_normal || self.normals.par_iter().any(|&n| degenerate(n));
        if needs {
            let key = self.normal_key.as_deref();
            let slots = key.map_or(self.positions.len(), |k| {
                k.iter().map(|&k| k as usize + 1).max().unwrap_or(0)
            });
            let mut acc = vec![[0f32; 3]; slots];
            let slot = |i: u32| key.map_or(i as usize, |k| k[i as usize] as usize);
            for t in self.indices.chunks_exact(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| self.positions[i as usize]);
                let n = cross(sub(b, a), sub(c, a));
                for &i in t {
                    let s = &mut acc[slot(i)];
                    s[0] += n[0];
                    s[1] += n[1];
                    s[2] += n[2];
                }
            }
            for (i, n) in self.normals.iter_mut().enumerate() {
                if degenerate(*n) {
                    *n = acc[slot(i as u32)];
                }
            }
        }
        self.normals.par_iter_mut().for_each(|n| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            *n = if l > 1e-12 && l.is_finite() {
                [n[0] / l, n[1] / l, n[2] / l]
            } else {
                [0.0, 0.0, 1.0]
            };
        });
    }
}

fn too_many_vertices(l: &Limits) -> Error {
    Error::TooLarge(format!("more than {} vertices", l.max_vertices))
}

fn too_many_triangles(l: &Limits) -> Error {
    Error::TooLarge(format!("more than {} triangles", l.max_triangles))
}

pub(crate) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Bounds of the positions the triangles use.
pub(crate) fn bounds_of(positions: &[[f32; 3]], indices: &[u32]) -> Bounds {
    // Every position used is the common case; check by counting instead of marking.
    let used = indices.len() >= positions.len() && {
        let mut seen = vec![false; positions.len()];
        let mut n = 0usize;
        for &i in indices {
            let s = &mut seen[i as usize];
            if !*s {
                *s = true;
                n += 1;
            }
        }
        n == positions.len()
    };
    let fold = |mut b: Bounds, p: &[f32; 3]| {
        b.add(*p);
        b
    };
    if used {
        positions
            .par_iter()
            .fold(|| Bounds::EMPTY, fold)
            .reduce(|| Bounds::EMPTY, |a, b| a.union(&b))
    } else {
        indices
            .par_iter()
            .fold(|| Bounds::EMPTY, |b, &i| fold(b, &positions[i as usize]))
            .reduce(|| Bounds::EMPTY, |a, b| a.union(&b))
    }
}

/// An sRGB-encoded byte from a linear value in 0..=1.
pub(crate) fn linear_to_srgb8(v: f32) -> u8 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

/// A linear value from an sRGB-encoded one in 0..=1.
pub(crate) fn srgb_to_linear(s: f32) -> f32 {
    let s = if s.is_nan() { 0.0 } else { s.clamp(0.0, 1.0) };
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// An affine transform: `out[r] = Σ m[r][c]·in[c] + m[r][3]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Affine(pub [[f64; 4]; 3]);

impl Affine {
    pub const IDENTITY: Affine = Affine([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    /// `self ∘ o`: `o` first.
    pub fn then_after(&self, o: &Affine) -> Affine {
        let a = &self.0;
        let b = &o.0;
        let mut m = [[0.0; 4]; 3];
        for (r, row) in m.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                *v = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c];
            }
            row[3] += a[r][3];
        }
        Affine(m)
    }

    pub fn point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        let p = p.map(f64::from);
        [0, 1, 2].map(|r| (m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3]) as f32)
    }

    pub fn det(&self) -> f64 {
        let m = &self.0;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// The matrix that carries normals: the inverse transpose of the linear part, up to
    /// scale (the cofactor matrix), so it exists for singular transforms too.
    pub fn normal_matrix(&self) -> [[f64; 3]; 3] {
        let m = &self.0;
        let c = |r0: usize, r1: usize, c0: usize, c1: usize| {
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        [
            [c(1, 2, 1, 2), -c(1, 2, 0, 2), c(1, 2, 0, 1)],
            [-c(0, 2, 1, 2), c(0, 2, 0, 2), -c(0, 2, 0, 1)],
            [c(0, 1, 1, 2), -c(0, 1, 0, 2), c(0, 1, 0, 1)],
        ]
    }

    pub fn is_identity(&self) -> bool {
        *self == Affine::IDENTITY
    }
}

pub(crate) fn apply3(m: &[[f64; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    let v = v.map(f64::from);
    let o = [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2]);
    let l = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
    if l > 0.0 && l.is_finite() {
        o.map(|x| (x / l) as f32)
    } else {
        [0.0; 3]
    }
}

/// A position's exact bits, with −0 read as +0.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Key([u32; 3]);

impl Key {
    fn of(p: &[f32; 3]) -> Key {
        Key(p.map(|v| if v == 0.0 { 0 } else { v.to_bits() }))
    }

    fn mix(&self) -> u64 {
        let [a, b, c] = self.0.map(u64::from);
        let h = (a | (b << 32)).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ c.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        h ^ (h >> 29)
    }
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.mix());
    }
}

#[derive(Default)]
struct PassHasher(u64);

impl Hasher for PassHasher {
    fn finish(&self) -> u64 {
        self.0.wrapping_mul(0xFF51_AFD7_ED55_8CCD).rotate_left(31)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100_0000_01B3);
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = v;
    }
}

const SHARD_BITS: u32 = 8;

/// Merges corners at identical positions: the unique positions, in order of first use, and
/// each corner's index into them. Parallel throughout: corners are sharded by a hash of
/// their position, each shard deduplicated on its own, and the results renumbered by first
/// use so neighbouring triangles keep nearby vertices.
pub(crate) fn weld(corners: &[[f32; 3]]) -> (Vec<[f32; 3]>, Vec<u32>) {
    const SHARDS: usize = 1 << SHARD_BITS;
    let n = corners.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let shard: Vec<u8> = corners
        .par_iter()
        .map(|p| (Key::of(p).mix() >> (64 - SHARD_BITS)) as u8)
        .collect();
    let chunk = n.div_ceil(rayon::current_num_threads() * 4).max(4096);
    // Per chunk, corners per shard.
    let hist: Vec<[u32; SHARDS]> = shard
        .par_chunks(chunk)
        .map(|c| {
            let mut h = [0u32; SHARDS];
            for &s in c {
                h[s as usize] += 1;
            }
            h
        })
        .collect();
    let mut starts = [0usize; SHARDS + 1];
    for s in 0..SHARDS {
        starts[s + 1] = starts[s] + hist.iter().map(|h| h[s] as usize).sum::<usize>();
    }
    // `order`: corner indices grouped by shard, ascending within each; split into the
    // pieces each chunk writes, so the chunks fill it in parallel.
    let mut order = vec![0u32; n];
    {
        let mut pieces: Vec<Vec<&mut [u32]>> = (0..hist.len())
            .map(|_| Vec::with_capacity(SHARDS))
            .collect();
        let mut rest: &mut [u32] = &mut order;
        for s in 0..SHARDS {
            for (k, h) in hist.iter().enumerate() {
                let (piece, tail) = std::mem::take(&mut rest).split_at_mut(h[s] as usize);
                pieces[k].push(piece);
                rest = tail;
            }
        }
        pieces
            .into_par_iter()
            .zip(shard.par_chunks(chunk))
            .enumerate()
            .for_each(|(k, (mut out, sh))| {
                let mut at = [0usize; SHARDS];
                let base = (k * chunk) as u32;
                for (i, &s) in sh.iter().enumerate() {
                    let s = s as usize;
                    out[s][at[s]] = base + i as u32;
                    at[s] += 1;
                }
            });
    }
    // Each shard: its unique positions, the first corner of each, and each of its corners'
    // index among them.
    struct Done {
        unique: Vec<[f32; 3]>,
        first: Vec<u32>,
        local: Vec<u32>,
    }
    let done: Vec<Done> = (0..SHARDS)
        .into_par_iter()
        .map(|s| {
            let mine = &order[starts[s]..starts[s + 1]];
            let mut map: HashMap<Key, u32, BuildHasherDefault<PassHasher>> =
                HashMap::with_capacity_and_hasher(mine.len() / 4 + 16, Default::default());
            let mut unique = Vec::new();
            let mut first = Vec::new();
            let mut local = Vec::with_capacity(mine.len());
            for &c in mine {
                let p = corners[c as usize];
                let id = *map.entry(Key::of(&p)).or_insert_with(|| {
                    unique.push(p);
                    first.push(c);
                    (unique.len() - 1) as u32
                });
                local.push(id);
            }
            Done {
                unique,
                first,
                local,
            }
        })
        .collect();
    drop(order);
    let mut base = [0u32; SHARDS + 1];
    for s in 0..SHARDS {
        base[s + 1] = base[s] + done[s].unique.len() as u32;
    }
    let total = base[SHARDS] as usize;
    // Renumber by first use.
    let firsts: Vec<u32> = done.iter().flat_map(|d| d.first.iter().copied()).collect();
    let mut by_first: Vec<u32> = (0..total as u32).collect();
    by_first.par_sort_unstable_by_key(|&g| firsts[g as usize]);
    drop(firsts);
    let mut new_of = vec![0u32; total];
    for (r, &g) in by_first.iter().enumerate() {
        new_of[g as usize] = r as u32;
    }
    let all: Vec<&[f32; 3]> = done.iter().flat_map(|d| d.unique.iter()).collect();
    let positions: Vec<[f32; 3]> = by_first.par_iter().map(|&g| *all[g as usize]).collect();
    drop(all);
    // Each corner's new index, chunk by chunk, walking the shards in the same order as
    // `order` was filled.
    let offsets: Vec<[u32; SHARDS]> = {
        let mut run = [0u32; SHARDS];
        hist.iter()
            .map(|h| {
                let o = run;
                for s in 0..SHARDS {
                    run[s] += h[s];
                }
                o
            })
            .collect()
    };
    let mut remap = vec![0u32; n];
    remap
        .par_chunks_mut(chunk)
        .zip(shard.par_chunks(chunk))
        .zip(offsets.par_iter())
        .for_each(|((out, sh), off)| {
            let mut at = *off;
            for (o, &s) in out.iter_mut().zip(sh) {
                let s = s as usize;
                let g = base[s] + done[s].local[at[s] as usize];
                at[s] += 1;
                *o = new_of[g as usize];
            }
        });
    (positions, remap)
}

/// Too short to be a direction (or NaN).
fn degenerate(n: [f32; 3]) -> bool {
    let l = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
    l.is_nan() || l <= 1e-24
}
