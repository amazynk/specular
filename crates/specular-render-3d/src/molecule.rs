//! Molecules on the GPU: a [`specular_mol::Scene`]'s balls and sticks as impostors
//! (`shaders/molecule.wgsl`), in chunks of at most [`MOL_CHUNK`] of each, plus a small
//! overlay (picked atoms, the distance between them) replaced without touching the rest.
//! The cartoon goes up as an ordinary mesh ([`crate::MeshRenderer::upload`]).

use specular_gfx::{Gpu, wgpu};
use specular_mesh::Bounds;
use specular_mol::{Ball, Stick};

use crate::{MeshRenderer, layout_entry, storage};

/// Balls (and sticks) per chunk at most.
pub const MOL_CHUNK: usize = 1 << 20;

/// A molecule's impostors on the GPU.
pub struct MoleculeGpu {
    pub(crate) chunks: Vec<MolChunk>,
    pub(crate) overlay: Option<MolChunk>,
    /// Everything drawn, in the canonical frame (molecules are z up as they are).
    pub bounds: Bounds,
    pub balls: usize,
    pub sticks: usize,
    /// GPU memory held (bytes).
    pub bytes: u64,
    /// Below 1 the sticks draw translucent at this opacity, without balls or caps
    /// (streamlines); set it any frame, nothing is uploaded again.
    pub opacity: f32,
}

pub(crate) struct MolChunk {
    pub group: wgpu::BindGroup,
    pub balls: u32,
    pub sticks: u32,
    pub bytes: u64,
}

pub(crate) struct MolPipes {
    pub balls: wgpu::RenderPipeline,
    pub sticks: wgpu::RenderPipeline,
}

pub(crate) fn mol_layout(d: &wgpu::Device) -> wgpu::BindGroupLayout {
    let vs = wgpu::ShaderStages::VERTEX;
    d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("molecule"),
        entries: &[
            layout_entry(0, vs, storage()),
            layout_entry(1, vs, storage()),
            layout_entry(2, vs, storage()),
            layout_entry(3, vs, storage()),
        ],
    })
}

fn rgba(c: [u8; 4]) -> u32 {
    u32::from_le_bytes(c)
}

fn buffer(gpu: &Gpu, label: &str, bytes: &[u8]) -> wgpu::Buffer {
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

impl MeshRenderer {
    /// Puts a scene's balls and sticks on the GPU; `bounds` is the whole scene's box.
    pub fn upload_molecule(
        &self,
        gpu: &Gpu,
        balls: &[Ball],
        sticks: &[Stick],
        bounds: Bounds,
    ) -> MoleculeGpu {
        let n = balls.len().max(sticks.len()).div_ceil(MOL_CHUNK).max(1);
        let chunks: Vec<MolChunk> = (0..n)
            .map(|k| {
                let r = |len: usize| (k * MOL_CHUNK).min(len)..((k + 1) * MOL_CHUNK).min(len);
                self.mol_chunk(gpu, &balls[r(balls.len())], &sticks[r(sticks.len())])
            })
            .collect();
        MoleculeGpu {
            bytes: chunks.iter().map(|c| c.bytes).sum(),
            chunks,
            overlay: None,
            bounds,
            balls: balls.len(),
            sticks: sticks.len(),
            opacity: 1.0,
        }
    }

    /// Replaces the overlay (drawn with the molecule, over nothing: it is depth-tested).
    pub fn set_overlay(&self, gpu: &Gpu, mol: &mut MoleculeGpu, balls: &[Ball], sticks: &[Stick]) {
        mol.overlay =
            (!balls.is_empty() || !sticks.is_empty()).then(|| self.mol_chunk(gpu, balls, sticks));
    }

    fn mol_chunk(&self, gpu: &Gpu, balls: &[Ball], sticks: &[Stick]) -> MolChunk {
        let ball_pos: Vec<[f32; 4]> = balls
            .iter()
            .map(|b| [b.centre[0], b.centre[1], b.centre[2], b.radius])
            .collect();
        let ball_col: Vec<u32> = balls.iter().map(|b| rgba(b.colour)).collect();
        let mut stick_pos: Vec<[f32; 4]> = Vec::with_capacity(sticks.len() * 2);
        for s in sticks {
            stick_pos.push([s.a[0], s.a[1], s.a[2], s.radius]);
            stick_pos.push([s.b[0], s.b[1], s.b[2], 0.0]);
        }
        let stick_col: Vec<[u32; 2]> = sticks
            .iter()
            .map(|s| [rgba(s.colours[0]), rgba(s.colours[1])])
            .collect();
        let bufs = [
            buffer(gpu, "molecule balls", bytemuck::cast_slice(&ball_pos)),
            buffer(
                gpu,
                "molecule ball colours",
                bytemuck::cast_slice(&ball_col),
            ),
            buffer(gpu, "molecule sticks", bytemuck::cast_slice(&stick_pos)),
            buffer(
                gpu,
                "molecule stick colours",
                bytemuck::cast_slice(&stick_col),
            ),
        ];
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("molecule"),
            layout: &self.mol_layout,
            entries: &std::array::from_fn::<_, 4, _>(|i| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: bufs[i].as_entire_binding(),
            }),
        });
        MolChunk {
            group,
            balls: balls.len() as u32,
            sticks: sticks.len() as u32,
            bytes: bufs.iter().map(wgpu::Buffer::size).sum(),
        }
    }
}

/// The molecule shader.
pub fn molecule_source() -> String {
    format!(
        "{}\n{}",
        crate::FRAME_WGSL,
        include_str!("shaders/molecule.wgsl")
    )
}
