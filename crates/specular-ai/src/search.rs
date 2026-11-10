//! Exact vector search over f16 embeddings: every stored vector is scored against the query
//! (dot product; embeddings are stored unit-length, so it is the cosine), on the GPU with a
//! wgpu compute shader or on the CPU, and the best `k` are kept.
//!
//! Both paths give the same bits, not merely close results. Every value is a normal f16 or
//! zero (subnormals are flushed and non-finite values zeroed on insert, [`sanitize`]), so
//! the f16 → f32 conversion is exact on both; the product of two such values (11-bit
//! significands) is exact in f32, so whether a compiler fuses multiply and add makes no
//! difference; no partial sum can be subnormal (every product is a multiple of 2⁻⁴⁸), so
//! flush-to-zero on a GPU makes none either; and both sum over the dimensions in the same
//! order with IEEE round-to-nearest additions. Ties in the ranking go to the lower index.
//!
//! Storage is blocks of [`BLOCK`] vectors, dimension-pair-major (two f16 in a `u32` word, see
//! `search.wgsl`): GPU threads of a workgroup read neighbouring words, and the CPU loop runs
//! across the vectors of a block, so it vectorises.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use half::f16;

/// Vectors per block.
pub const BLOCK: usize = 1024;

/// An f16 value made safe for exact scoring: subnormals and −0 become +0, infinities and
/// NaN become 0.
pub fn sanitize(h: u16) -> u16 {
    let e = (h >> 10) & 31;
    if e == 0 || e == 31 { 0 } else { h }
}

/// Exact f16 → f32 of a sanitised value: the same bit operations as the shader.
#[inline(always)]
pub fn half_to_f32(h: u32) -> f32 {
    let e = (h >> 10) & 31;
    let m = h & 0x3ff;
    let s = (h & 0x8000) << 16;
    let bits = if e == 0 {
        s
    } else {
        s | ((e + 112) << 23) | (m << 13)
    };
    f32::from_bits(bits)
}

/// f32 → sanitised f16 bits (round to nearest even).
pub fn to_f16(x: f32) -> u16 {
    sanitize(f16::from_f32(x).to_bits())
}

/// Scales `v` to unit length (a zero vector stays zero) and stores it as sanitised f16.
pub fn normalize_f16(v: &[f32]) -> Vec<u16> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let s = if n > 0.0 && n.is_finite() {
        1.0 / n
    } else {
        0.0
    };
    v.iter().map(|x| to_f16(x * s)).collect()
}

/// A result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Position in the [`Vectors`] (insert order).
    pub index: u32,
    /// The caller's id for it (a file id).
    pub id: i64,
    pub score: f32,
}

/// The vectors searched, in blocks. Rows are appended; a removed row is only marked (it
/// stays in place and is never a hit) until [`Vectors::compact_if_sparse`] drops the
/// removed rows, which renumbers the rest (and makes a [`GpuIndex`] upload anew).
#[derive(Clone, Debug)]
pub struct Vectors {
    dims: usize,
    len: usize,
    words: Vec<u32>,
    ids: Vec<i64>,
    /// Removed rows (marked, still stored).
    dead: Vec<bool>,
    removed: usize,
    /// Bumped by every compaction: row numbers before it mean nothing after.
    generation: u64,
    /// Bumped by every removal (a [`GpuIndex`] copies the marks when it changed).
    removals: u64,
}

impl Vectors {
    /// `dims` must be even.
    pub fn new(dims: usize) -> Vectors {
        assert!(dims >= 2 && dims.is_multiple_of(2), "dims must be even");
        Vectors {
            dims,
            len: 0,
            words: Vec::new(),
            ids: Vec::new(),
            dead: Vec::new(),
            removed: 0,
            generation: 0,
            removals: 0,
        }
    }

    pub fn dims(&self) -> usize {
        self.dims
    }

    /// Rows stored, removed ones included (row numbers run to this).
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Rows not removed.
    pub fn live(&self) -> usize {
        self.len - self.removed
    }

    pub fn ids(&self) -> &[i64] {
        &self.ids
    }

    fn pairs(&self) -> usize {
        self.dims / 2
    }

    fn block_words(&self) -> usize {
        self.pairs() * BLOCK
    }

    /// Adds a vector of f16 bits (sanitised here). Panics on a wrong length.
    pub fn push(&mut self, id: i64, v: &[u16]) {
        assert_eq!(v.len(), self.dims, "vector length");
        let (b, lane) = (self.len / BLOCK, self.len % BLOCK);
        if lane == 0 {
            self.words.resize((b + 1) * self.block_words(), 0);
        }
        let base = b * self.block_words() + lane;
        for p in 0..self.pairs() {
            let lo = sanitize(v[2 * p]) as u32;
            let hi = sanitize(v[2 * p + 1]) as u32;
            self.words[base + p * BLOCK] = lo | hi << 16;
        }
        self.ids.push(id);
        self.dead.push(false);
        self.len += 1;
    }

    /// Marks row `i` removed: never a hit again. False when it is out of range or removed
    /// already.
    pub fn remove(&mut self, i: usize) -> bool {
        match self.dead.get_mut(i) {
            Some(d) if !*d => {
                *d = true;
                self.removed += 1;
                self.removals += 1;
                true
            }
            _ => false,
        }
    }

    /// Whether row `i` was removed.
    pub fn is_removed(&self, i: usize) -> bool {
        self.dead.get(i).copied().unwrap_or(false)
    }

    /// Drops the removed rows once they are a quarter of all (lazily: a few removals cost
    /// nothing until then). The rows left keep their order and are renumbered from 0. True
    /// when it compacted.
    pub fn compact_if_sparse(&mut self) -> bool {
        if self.removed == 0 || self.removed * 4 < self.len {
            return false;
        }
        self.compact();
        true
    }

    /// Drops the removed rows now (see [`Vectors::compact_if_sparse`]).
    pub fn compact(&mut self) {
        let mut out = Vectors::new(self.dims);
        for i in (0..self.len).filter(|&i| !self.dead[i]) {
            out.push(self.ids[i], &self.get(i));
        }
        out.generation = self.generation + 1;
        *self = out;
    }

    /// Vector `i` back as f16 bits.
    pub fn get(&self, i: usize) -> Vec<u16> {
        let base = (i / BLOCK) * self.block_words() + i % BLOCK;
        (0..self.pairs())
            .flat_map(|p| {
                let w = self.words[base + p * BLOCK];
                [w as u16, (w >> 16) as u16]
            })
            .collect()
    }

    fn blocks(&self) -> usize {
        self.len.div_ceil(BLOCK)
    }
}

/// The query as exact f32 values of its sanitised f16 bits.
fn query_f32(q: &[u16]) -> Vec<f32> {
    q.iter().map(|&h| half_to_f32(sanitize(h) as u32)).collect()
}

/// Scores of one block (`out.len()` = vectors in it).
fn score_block(words: &[u32], q: &[f32], out: &mut [f32]) {
    let mut acc = [0f32; BLOCK];
    for (p, row) in words.chunks_exact(BLOCK).enumerate() {
        let (q0, q1) = (q[2 * p], q[2 * p + 1]);
        for (a, &w) in acc.iter_mut().zip(row) {
            *a += half_to_f32(w & 0xffff) * q0;
            *a += half_to_f32(w >> 16) * q1;
        }
    }
    out.copy_from_slice(&acc[..out.len()]);
}

/// Every row's score on the CPU (removed rows too), on up to `threads` threads.
pub fn scores_cpu(v: &Vectors, query: &[u16], threads: usize) -> Vec<f32> {
    assert_eq!(query.len(), v.dims, "query length");
    let q = query_f32(query);
    let mut out = vec![0f32; v.len];
    let bw = v.block_words();
    let blocks = v.blocks();
    let threads = threads.clamp(1, blocks.max(1));
    let per = blocks.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (t, chunk) in out.chunks_mut(per * BLOCK).enumerate() {
            let q = &q;
            let words = &v.words;
            specular_thread::spawn_scoped(s, format!("ai-score-{t}"), move || {
                for (j, o) in chunk.chunks_mut(BLOCK).enumerate() {
                    let b = t * per + j;
                    score_block(&words[b * bw..(b + 1) * bw], q, o);
                }
            });
        }
    });
    out
}

/// The best `k` of `scores` among the rows not removed: highest score first, ties to the
/// lower index.
pub fn top_k(v: &Vectors, scores: &[f32], k: usize) -> Vec<Hit> {
    rank(&v.ids, &v.dead, scores, k)
}

fn rank(ids: &[i64], dead: &[bool], scores: &[f32], k: usize) -> Vec<Hit> {
    let order = |a: &u32, b: &u32| {
        scores[*b as usize]
            .total_cmp(&scores[*a as usize])
            .then(a.cmp(b))
    };
    let mut idx: Vec<u32> = (0..scores.len() as u32)
        .filter(|&i| !dead.get(i as usize).copied().unwrap_or(false))
        .collect();
    let k = k.min(idx.len());
    if k == 0 {
        return Vec::new();
    }
    if k < idx.len() {
        idx.select_nth_unstable_by(k - 1, order);
        idx.truncate(k);
    }
    idx.sort_unstable_by(order);
    idx.into_iter()
        .map(|i| Hit {
            index: i,
            id: ids[i as usize],
            score: scores[i as usize],
        })
        .collect()
}

/// Exact search on the CPU.
pub fn search_cpu(v: &Vectors, query: &[u16], k: usize, threads: usize) -> Vec<Hit> {
    top_k(v, &scores_cpu(v, query, threads), k)
}

/// The vectors uploaded to a GPU, searched with a compute shader. Kept in step with its
/// [`Vectors`] by [`GpuIndex::update`]: added rows are appended (only the blocks they touch
/// are written; a chunk's buffer grows by doubling), removal marks are copied, and only a
/// compaction uploads everything again.
pub struct GpuIndex {
    dims: usize,
    len: usize,
    ids: Vec<i64>,
    dead: Vec<bool>,
    generation: u64,
    removals: u64,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// Blocks one chunk holds at most (one storage binding, one dispatch).
    chunk_blocks: usize,
    chunks: Vec<Chunk>,
    query: wgpu::Buffer,
    scores: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Rows the score buffers hold.
    score_cap: usize,
    /// Whole uploads made (the first included): for tests.
    uploads: usize,
}

/// Whole blocks in one storage binding.
struct Chunk {
    data: wgpu::Buffer,
    params: wgpu::Buffer,
    /// First vector.
    first: usize,
    count: usize,
    /// Blocks the buffer has room for.
    cap: usize,
}

const WORKGROUP: u32 = 256;

impl GpuIndex {
    /// Uploads `v`. Fails (no GPU path) when a single block does not fit a storage binding.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        v: &Vectors,
    ) -> Result<GpuIndex, String> {
        GpuIndex::with_chunk_blocks(device, queue, v, usize::MAX)
    }

    /// [`GpuIndex::new`] with at most `max_blocks` blocks per chunk (tests use a few to
    /// cross chunk boundaries with small sets).
    #[doc(hidden)]
    pub fn with_chunk_blocks(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        v: &Vectors,
        max_blocks: usize,
    ) -> Result<GpuIndex, String> {
        let limits = device.limits();
        let block_bytes = (v.block_words() * 4) as u64;
        let max_binding = limits
            .max_storage_buffer_binding_size
            .min(limits.max_buffer_size);
        let chunk_blocks = (max_binding / block_bytes) as usize;
        if chunk_blocks == 0 {
            return Err(format!(
                "a block of {} dimensions does not fit a storage binding",
                v.dims
            ));
        }
        let max_vectors_per_dispatch =
            limits.max_compute_workgroups_per_dimension as usize * WORKGROUP as usize;
        let chunk_blocks = chunk_blocks
            .min((max_vectors_per_dispatch / BLOCK).max(1))
            .min(max_blocks.max(1));
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ai search"),
            source: wgpu::ShaderSource::Wgsl(include_str!("search.wgsl").into()),
        });
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ai search"),
            entries: &[
                storage(0, true),
                storage(1, true),
                storage(2, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ai search"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ai search"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("cs_scores"),
            compilation_options: Default::default(),
            cache: None,
        });
        let query = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ai search query"),
            size: (v.dims * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (scores, readback) = score_buffers(device, 1);
        let mut index = GpuIndex {
            dims: v.dims,
            len: 0,
            ids: Vec::new(),
            dead: Vec::new(),
            generation: v.generation,
            removals: 0,
            pipeline,
            layout,
            chunk_blocks,
            chunks: Vec::new(),
            query,
            scores,
            readback,
            score_cap: 1,
            uploads: 1,
        };
        index.append(device, queue, v);
        Ok(index)
    }

    /// Rows uploaded (removed ones included).
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whole uploads made so far (1 after [`GpuIndex::new`]; appends do not count).
    #[doc(hidden)]
    pub fn uploads(&self) -> usize {
        self.uploads
    }

    /// Brings the index in step with `v`, which it was made from or last updated with:
    /// appends the rows added since and copies the removal marks; after a compaction (or for
    /// another set of vectors) uploads everything again.
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, v: &Vectors) {
        if v.dims != self.dims || v.generation != self.generation || v.len < self.len {
            self.chunks.clear();
            self.ids.clear();
            self.dead.clear();
            self.len = 0;
            self.generation = v.generation;
            self.removals = 0;
            self.uploads += 1;
        }
        self.append(device, queue, v);
    }

    /// Uploads the rows of `v` past `self.len`, and the marks if they changed.
    fn append(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, v: &Vectors) {
        if v.removals != self.removals || v.len != self.len {
            self.dead.clone_from(&v.dead);
            self.removals = v.removals;
        }
        if v.len == self.len {
            return;
        }
        let bw = v.block_words();
        let block_bytes = (bw * 4) as u64;
        let cb = self.chunk_blocks;
        let (from, to) = (self.len / BLOCK, v.blocks());
        let mut touched = Vec::new();
        for b in from..to {
            let (ci, local) = (b / cb, b % cb);
            // Blocks this chunk holds once `v` is in.
            let want = to.min((ci + 1) * cb) - ci * cb;
            if ci == self.chunks.len() {
                let cap = want.next_power_of_two().min(cb);
                let data = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ai search vectors"),
                    size: cap as u64 * block_bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let params = specular_infer::gpu::buffer_init(
                    device,
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("ai search params"),
                        contents: bytemuck::cast_slice(&[0u32, v.pairs() as u32, 0, 0]),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    },
                );
                self.chunks.push(Chunk {
                    data,
                    params,
                    first: ci * cb * BLOCK,
                    count: 0,
                    cap,
                });
            }
            let c = &mut self.chunks[ci];
            if local >= c.cap {
                // Grow by doubling (at least to what it needs now) and write back what the
                // chunk held: the host copy has it all.
                let cap = (2 * c.cap).max(want.next_power_of_two()).min(cb);
                c.data = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ai search vectors"),
                    size: cap as u64 * block_bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                c.cap = cap;
                let start = ci * cb;
                queue.write_buffer(
                    &c.data,
                    0,
                    bytemuck::cast_slice(&v.words[start * bw..b * bw]),
                );
            }
            queue.write_buffer(
                &c.data,
                local as u64 * block_bytes,
                bytemuck::cast_slice(&v.words[b * bw..(b + 1) * bw]),
            );
            if touched.last() != Some(&ci) {
                touched.push(ci);
            }
        }
        for ci in touched {
            let c = &mut self.chunks[ci];
            c.count = (v.len - c.first).min(cb * BLOCK);
            queue.write_buffer(
                &c.params,
                0,
                bytemuck::cast_slice(&[c.count as u32, v.pairs() as u32, 0, 0]),
            );
        }
        if v.len > self.score_cap {
            self.score_cap = v.len.next_power_of_two();
            (self.scores, self.readback) = score_buffers(device, self.score_cap);
        }
        self.ids.extend_from_slice(&v.ids[self.len..]);
        self.len = v.len;
    }

    /// Every row's score, removed rows too (blocks until the GPU is done).
    pub fn scores(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        query: &[u16],
    ) -> Result<Vec<f32>, String> {
        assert_eq!(query.len(), self.dims, "query length");
        if self.len == 0 {
            return Ok(Vec::new());
        }
        queue.write_buffer(&self.query, 0, bytemuck::cast_slice(&query_f32(query)));
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ai search"),
        });
        let binds: Vec<wgpu::BindGroup> = self
            .chunks
            .iter()
            .map(|c| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("ai search"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: c.data.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.query.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &self.scores,
                                offset: (c.first * 4) as u64,
                                size: std::num::NonZeroU64::new((c.count * 4) as u64),
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: c.params.as_entire_binding(),
                        },
                    ],
                })
            })
            .collect();
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ai search"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&self.pipeline);
            for (c, bind) in self.chunks.iter().zip(&binds) {
                cp.set_bind_group(0, bind, &[]);
                cp.dispatch_workgroups((c.count as u32).div_ceil(WORKGROUP), 1, 1);
            }
        }
        let bytes = (self.len * 4) as u64;
        enc.copy_buffer_to_buffer(&self.scores, 0, &self.readback, 0, bytes);
        queue.submit([enc.finish()]);
        let done = Arc::new(AtomicU8::new(0));
        let d = done.clone();
        self.readback
            .slice(..bytes)
            .map_async(wgpu::MapMode::Read, move |r| {
                d.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
        specular_infer::gpu::poll_device(device, wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        if done.load(Ordering::Acquire) != 1 {
            // A destroyed device takes the work and fails only the map.
            return Err("mapping the scores failed (the GPU device may be lost)".into());
        }
        let out = {
            let view = self
                .readback
                .slice(..bytes)
                .get_mapped_range()
                .map_err(|e| format!("reading the scores back failed: {e}"))?;
            bytemuck::cast_slice::<u8, f32>(&view).to_vec()
        };
        self.readback.unmap();
        Ok(out)
    }

    /// Exact search on the GPU: the same hits, in the same order, as [`search_cpu`].
    pub fn search(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        query: &[u16],
        k: usize,
    ) -> Result<Vec<Hit>, String> {
        let scores = self.scores(device, queue, query)?;
        Ok(rank(&self.ids, &self.dead, &scores, k))
    }
}

/// The score buffer and its readback, for `rows` rows.
fn score_buffers(device: &wgpu::Device, rows: usize) -> (wgpu::Buffer, wgpu::Buffer) {
    let bytes = (rows.max(1) * 4) as u64;
    let scores = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ai search scores"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ai search readback"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    (scores, readback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_conversion_is_exact_for_sanitised_values() {
        for h in 0..=u16::MAX {
            let s = sanitize(h);
            let want = f16::from_bits(s).to_f32();
            let got = half_to_f32(s as u32);
            assert_eq!(got.to_bits(), want.to_bits(), "{h:#06x}");
            assert!(got.is_finite());
            assert!(got == 0.0 || got.is_normal());
            assert!(!(got == 0.0 && got.is_sign_negative()));
        }
    }

    #[test]
    fn layout_round_trips() {
        let mut v = Vectors::new(6);
        for i in 0..2500u16 {
            v.push(i as i64, &[0x3c00, i | 0x400, 0, 0x4000, 0xbc00, 0x3800]);
        }
        assert_eq!(v.len(), 2500);
        assert_eq!(v.get(1700)[1], 1700 | 0x400);
        assert_eq!(v.get(3), vec![0x3c00, 3 | 0x400, 0, 0x4000, 0xbc00, 0x3800]);
    }

    #[test]
    fn ranks_by_score_then_index() {
        let mut v = Vectors::new(2);
        v.push(10, &[to_f16(0.5), 0]);
        v.push(11, &[to_f16(1.0), 0]);
        v.push(12, &[to_f16(0.5), 0]);
        v.push(13, &[to_f16(-1.0), 0]);
        let hits = search_cpu(&v, &[to_f16(1.0), 0], 3, 2);
        assert_eq!(
            hits.iter().map(|h| h.id).collect::<Vec<_>>(),
            vec![11, 10, 12]
        );
        assert_eq!(hits[0].score, 1.0);
        assert!(search_cpu(&Vectors::new(2), &[0, 0], 5, 1).is_empty());
    }

    #[test]
    fn threads_do_not_change_results() {
        let v = crate::testing::random_vectors(5000, 64, 7);
        let q = crate::testing::random_vectors(1, 64, 99).get(0);
        let one = scores_cpu(&v, &q, 1);
        let many = scores_cpu(&v, &q, 8);
        assert!(
            one.iter()
                .zip(&many)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        // Against a plain scalar loop in dimension order.
        for i in [0, 1, 1023, 1024, 4999] {
            let x = v.get(i);
            let mut acc = 0f32;
            for d in 0..64 {
                acc += half_to_f32(x[d] as u32) * half_to_f32(q[d] as u32);
            }
            assert_eq!(acc.to_bits(), one[i].to_bits(), "vector {i}");
        }
    }
}
