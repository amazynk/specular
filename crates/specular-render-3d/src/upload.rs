//! A mesh on the GPU, uploaded once: per vertex 8 bytes of quantised position (u16 × 3 over
//! the model's box), 4 of octahedral normal (snorm16 × 2, none for flat models), 4 of
//! colour and 8 of UV where there are any; per triangle 12 bytes of u32 indices. Chunks of
//! at most [`CHUNK`] triangles, one material each, share the vertex buffers while they fit
//! in one storage binding and carry their own vertices (re-indexed) when not.

use std::collections::HashMap;

use rayon::prelude::*;
use specular_gfx::{Gpu, wgpu};
use specular_mesh::preview::{Texture, canonical};
use specular_mesh::{Bounds, Mesh};

use crate::MeshRenderer;

/// Triangles per chunk at most.
pub const CHUNK: usize = 1 << 20;

/// The colour of a model without colours or materials (linear).
const CLAY: [f32; 4] = [0.50, 0.53, 0.58, 1.0];

/// A mesh on the GPU.
pub struct MeshGpu {
    pub(crate) chunks: Vec<Chunk>,
    pub(crate) q_scale: [f32; 3],
    pub(crate) q_offset: [f32; 3],
    /// The model's box in the canonical frame (z up).
    pub bounds: Bounds,
    pub triangles: usize,
    /// GPU memory held (bytes).
    pub bytes: u64,
}

pub(crate) struct Chunk {
    pub group: wgpu::BindGroup,
    pub vertices: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialU {
    color: [f32; 4],
    flags: [u32; 4],
}

/// What the vertices carry.
#[derive(Clone, Copy)]
struct Needs {
    normals: bool,
    colors: bool,
    uvs: bool,
}

/// Vertex buffers (the dummy where an attribute is absent).
struct Vertices {
    pos: wgpu::Buffer,
    nrm: Option<wgpu::Buffer>,
    col: Option<wgpu::Buffer>,
    uv: Option<wgpu::Buffer>,
}

/// The quantisation of positions over the canonical box.
#[derive(Clone, Copy)]
struct Quant {
    offset: [f32; 3],
    inv: [f32; 3],
    up: specular_mesh::Up,
}

impl Quant {
    fn pack(&self, p: [f32; 3]) -> [u32; 2] {
        let c = canonical(p, self.up);
        let q: [u32; 3] = std::array::from_fn(|k| {
            let v = ((c[k] - self.offset[k]) * self.inv[k]).round();
            if v.is_nan() {
                0
            } else {
                v.clamp(0.0, 65535.0) as u32
            }
        });
        [q[0] | (q[1] << 16), q[2]]
    }
}

/// The box `b` (in the file's frame) in the canonical frame.
pub fn canonical_bounds(mesh: &Mesh) -> Bounds {
    let b = &mesh.bounds;
    if b.is_empty() {
        return *b;
    }
    let mut out = Bounds::EMPTY;
    for c in crate::camera::corners(b) {
        out.add(canonical(c.map(|v| v as f32), mesh.up));
    }
    out
}

/// Octahedral encoding as snorm16 × 2.
pub(crate) fn oct(n: [f32; 3]) -> u32 {
    let l1 = n[0].abs() + n[1].abs() + n[2].abs();
    if !l1.is_finite() || l1 <= 0.0 {
        return pack_snorm(0.0, 0.0);
    }
    let (mut x, mut y) = (n[0] / l1, n[1] / l1);
    if n[2] < 0.0 {
        let sign = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
        (x, y) = ((1.0 - y.abs()) * sign(x), (1.0 - x.abs()) * sign(y));
    }
    pack_snorm(x, y)
}

fn pack_snorm(x: f32, y: f32) -> u32 {
    let s = |v: f32| ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
    s(x) | (s(y) << 16)
}

/// A storage buffer of `n` elements, `f(i)` each, computed in parallel and copied into the
/// buffer's memory mapped at creation (write-only: it may be write-combined).
fn filled<T: bytemuck::Pod + Send>(
    gpu: &Gpu,
    label: &str,
    n: usize,
    f: impl Fn(usize) -> T + Sync,
) -> wgpu::Buffer {
    let data: Vec<T> = (0..n).into_par_iter().with_min_len(4096).map(&f).collect();
    let bytes: &[u8] = bytemuck::cast_slice(&data);
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (bytes.len() as u64)
            .max(16)
            .next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: true,
    });
    // Mapped at creation, so the range is there; a failure leaves zeros.
    if let Ok(mut range) = buf.slice(..).get_mapped_range_mut() {
        range.slice(..bytes.len()).copy_from_slice(bytes);
    }
    buf.unmap();
    buf
}

fn vertices(gpu: &Gpu, mesh: &Mesh, q: Quant, need: Needs, pick: Option<&[u32]>) -> Vertices {
    let n = pick.map_or(mesh.positions.len(), <[u32]>::len);
    let at = |i: usize| pick.map_or(i, |p| p[i] as usize);
    let pos = filled(gpu, "mesh positions", n, |i| q.pack(mesh.positions[at(i)]));
    let nrm = need.normals.then(|| {
        filled(gpu, "mesh normals", n, |i| {
            oct(canonical(mesh.normals[at(i)], mesh.up))
        })
    });
    let col = match (&mesh.colors, need.colors) {
        (Some(c), true) => Some(filled(gpu, "mesh colours", n, |i| {
            u32::from_le_bytes(c[at(i)])
        })),
        _ => None,
    };
    let uv = match (&mesh.uvs, need.uvs) {
        (Some(u), true) => Some(filled(gpu, "mesh uvs", n, |i| u[at(i)])),
        _ => None,
    };
    Vertices { pos, nrm, col, uv }
}

fn size(b: &Option<wgpu::Buffer>) -> u64 {
    b.as_ref().map_or(0, wgpu::Buffer::size)
}

impl Vertices {
    fn bytes(&self) -> u64 {
        self.pos.size() + size(&self.nrm) + size(&self.col) + size(&self.uv)
    }
}

/// Splits the parts into chunks: (material, first triangle, triangles).
fn plan(mesh: &Mesh, chunk: usize) -> Vec<(Option<u32>, usize, usize)> {
    let tris = mesh.triangle_count();
    let parts: Vec<(Option<u32>, usize, usize)> = if mesh.parts.is_empty() {
        vec![(None, 0, tris)]
    } else {
        mesh.parts
            .iter()
            .map(|p| (p.material, p.first as usize, p.count as usize))
            .collect()
    };
    let mut out = Vec::new();
    for (m, first, count) in parts {
        let end = (first + count).min(tris);
        let mut at = first;
        while at < end {
            let n = (end - at).min(chunk);
            out.push((m, at, n));
            at += n;
        }
    }
    out
}

/// A texture with its mip chain (box-filtered on the CPU), capped at the GPU's size.
fn texture(gpu: &Gpu, t: &Texture) -> Option<(wgpu::TextureView, u64)> {
    let (w, h) = (t.width as usize, t.height as usize);
    if w == 0 || h == 0 || t.rgba.len() < w * h * 4 {
        return None;
    }
    let max = gpu.max_texture_size().max(1) as usize;
    let mut levels: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    let (mut lw, mut lh) = (w, h);
    let mut cur: std::borrow::Cow<[u8]> = std::borrow::Cow::Borrowed(&t.rgba[..w * h * 4]);
    loop {
        let next = (lw > 1 || lh > 1).then(|| half(lw, lh, &cur));
        if lw <= max && lh <= max {
            levels.push((lw, lh, cur.into_owned()));
        }
        match next {
            Some((nw, nh, px)) => {
                (lw, lh) = (nw, nh);
                cur = std::borrow::Cow::Owned(px);
            }
            None => break,
        }
    }
    let (w0, h0) = (levels[0].0 as u32, levels[0].1 as u32);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mesh texture"),
        size: wgpu::Extent3d {
            width: w0,
            height: h0,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut bytes = 0;
    for (i, (lw, lh, px)) in levels.iter().enumerate() {
        bytes += px.len() as u64;
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            px,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(*lw as u32 * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: *lw as u32,
                height: *lh as u32,
                depth_or_array_layers: 1,
            },
        );
    }
    Some((texture.create_view(&Default::default()), bytes))
}

/// The next mip level: each pixel the mean of up to 2 × 2.
fn half(w: usize, h: usize, px: &[u8]) -> (usize, usize, Vec<u8>) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = vec![0u8; nw * nh * 4];
    out.par_chunks_mut(nw * 4).enumerate().for_each(|(y, row)| {
        let ys = [(2 * y).min(h - 1), (2 * y + 1).min(h - 1)];
        for x in 0..nw {
            let xs = [(2 * x).min(w - 1), (2 * x + 1).min(w - 1)];
            for c in 0..4 {
                let mut s = 0u32;
                for yy in ys {
                    for xx in xs {
                        s += u32::from(px[(yy * w + xx) * 4 + c]);
                    }
                }
                row[x * 4 + c] = ((s + 2) / 4) as u8;
            }
        }
    });
    (nw, nh, out)
}

impl MeshRenderer {
    /// Uploads `mesh` (with `textures` decoded for its images, by image index).
    pub fn upload(&self, gpu: &Gpu, mesh: &Mesh, textures: &[Option<Texture>]) -> MeshGpu {
        self.upload_with(gpu, mesh, textures, CHUNK, true, false)
    }

    /// Uploads section caps ([`crate::section::caps`]): drawn hatched.
    pub fn upload_caps(&self, gpu: &Gpu, mesh: &Mesh) -> MeshGpu {
        self.upload_with(gpu, mesh, &[], CHUNK, true, true)
    }

    /// [`MeshRenderer::upload`] in chunks of `chunk` triangles, sharing the vertices across
    /// chunks when `share` and they fit.
    pub(crate) fn upload_with(
        &self,
        gpu: &Gpu,
        mesh: &Mesh,
        textures: &[Option<Texture>],
        chunk: usize,
        share: bool,
        hatch: bool,
    ) -> MeshGpu {
        let bounds = canonical_bounds(mesh);
        let (offset, ext) = if bounds.is_empty() {
            ([0.0; 3], [0.0; 3])
        } else {
            (bounds.min, bounds.size())
        };
        let q = Quant {
            offset,
            inv: ext.map(|e| if e > 0.0 { 65535.0 / e } else { 0.0 }),
            up: mesh.up,
        };
        let q_scale = ext.map(|e| e / 65535.0);
        let plan = plan(mesh, chunk.max(1));
        let any_texture = plan
            .iter()
            .any(|(m, ..)| self.texture_of(mesh, *m, textures));
        let need = Needs {
            normals: !mesh.flat && mesh.normals.len() == mesh.positions.len(),
            colors: mesh.colors.is_some(),
            uvs: any_texture && mesh.uvs.is_some(),
        };
        // Positions and UVs, 8 bytes a vertex, are the largest.
        let largest = mesh.positions.len() as u64 * 8;
        let shared_fits = largest <= gpu.limits.max_storage_buffer_binding_size
            && largest <= gpu.limits.max_buffer_size;
        let shared = (shared_fits && share).then(|| vertices(gpu, mesh, q, need, None));
        let mut bytes = shared.as_ref().map_or(0, Vertices::bytes);
        let mut views: HashMap<u32, Option<wgpu::TextureView>> = HashMap::new();
        let mut chunks = Vec::with_capacity(plan.len());
        for (m, first, count) in plan {
            let tri = &mesh.indices[first * 3..(first + count) * 3];
            let local;
            let (verts, idx) = match &shared {
                Some(v) => (v, filled(gpu, "mesh indices", tri.len(), |i| tri[i])),
                None => {
                    let mut pick = tri.to_vec();
                    pick.par_sort_unstable();
                    pick.dedup();
                    local = vertices(gpu, mesh, q, need, Some(&pick));
                    bytes += local.bytes();
                    let idx = filled(gpu, "mesh indices", tri.len(), |i| {
                        pick.binary_search(&tri[i]).unwrap_or(0) as u32
                    });
                    (&local, idx)
                }
            };
            bytes += idx.size();
            let material = m.and_then(|i| mesh.materials.get(i as usize));
            let image = material.and_then(|m| m.texture);
            let view = image.and_then(|i| {
                views
                    .entry(i)
                    .or_insert_with(|| {
                        let t = textures.get(i as usize)?.as_ref()?;
                        let (v, b) = texture(gpu, t)?;
                        bytes += b;
                        Some(v)
                    })
                    .clone()
            });
            let textured = view.is_some() && verts.uv.is_some();
            let color = match material {
                Some(m) => m.base_color,
                None if mesh.colors.is_some() => [1.0; 4],
                None => CLAY,
            };
            let mu = MaterialU {
                color,
                flags: [
                    u32::from(verts.col.is_some()),
                    u32::from(textured),
                    u32::from(verts.nrm.is_none()),
                    u32::from(hatch),
                ],
            };
            let ubuf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mesh material"),
                size: size_of::<MaterialU>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            gpu.queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&mu));
            let or = |b: &'_ Option<wgpu::Buffer>| -> wgpu::Buffer {
                b.as_ref().unwrap_or(&self.dummy).clone()
            };
            let (nrm, col, uv) = (or(&verts.nrm), or(&verts.col), or(&verts.uv));
            let tex_view = if textured {
                view.as_ref().unwrap_or(&self.white)
            } else {
                &self.white
            };
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mesh chunk"),
                layout: &self.chunk_layout,
                entries: &[
                    bind(0, ubuf.as_entire_binding()),
                    bind(1, verts.pos.as_entire_binding()),
                    bind(2, nrm.as_entire_binding()),
                    bind(3, col.as_entire_binding()),
                    bind(4, uv.as_entire_binding()),
                    bind(5, idx.as_entire_binding()),
                    bind(6, wgpu::BindingResource::TextureView(tex_view)),
                    bind(7, wgpu::BindingResource::Sampler(&self.sampler)),
                ],
            });
            chunks.push(Chunk {
                group,
                vertices: (count * 3) as u32,
            });
        }
        MeshGpu {
            chunks,
            q_scale,
            q_offset: offset,
            bounds,
            triangles: mesh.triangle_count(),
            bytes,
        }
    }

    /// Whether material `m` has a texture among `textures`.
    fn texture_of(&self, mesh: &Mesh, m: Option<u32>, textures: &[Option<Texture>]) -> bool {
        m.and_then(|i| mesh.materials.get(i as usize))
            .and_then(|m| m.texture)
            .and_then(|i| textures.get(i as usize))
            .is_some_and(Option::is_some)
    }
}

fn bind(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}
