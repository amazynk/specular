//! 3D meshes (STL, OBJ with MTL, PLY, glTF and GLB, 3MF) via specular-mesh.
//!
//! A mesh has no pixel size, so the session picks one: an isometric preview whose long side
//! is [`PREVIEW_LONG_SIDE`] px, with the aspect of the model seen from the iso view. That
//! raster (rendered on the CPU, transparent around the model) is what thumbnails and the
//! plain image view show; the app draws the interactive GPU view from [`Session::mesh`].
//! Base-colour textures are decoded once at open through this registry's own decoders and
//! kept as sRGB RGBA8, the long side at most [`MAX_TEXTURE_SIDE`].

use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, MeshModel, Registry, Result, Session,
};
use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_mesh::preview::{self, Texture};
use specular_mesh::{DirResolver, ImageData, Limits, Mesh, MeshFormat};
use std::sync::Arc;

pub const PREVIEW_LONG_SIDE: u32 = 1024;
pub const MAX_TEXTURE_SIDE: u32 = 4096;
/// A texture is decoded only when its decode (at its smallest fitting scale) is at most this
/// many pixels.
const MAX_TEXTURE_DECODE_PIXELS: u64 = 64_000_000;

pub struct MeshDecoder(pub MeshFormat);

impl MeshDecoder {
    fn our_format(&self) -> Format {
        format_of(self.0)
    }
}

/// The registry format of a mesh format.
pub fn format_of(f: MeshFormat) -> Format {
    match f {
        MeshFormat::Stl => Format::Stl,
        MeshFormat::Obj => Format::Obj,
        MeshFormat::Ply => Format::Ply,
        MeshFormat::Gltf => Format::Gltf,
        MeshFormat::Glb => Format::Glb,
        MeshFormat::ThreeMf => Format::ThreeMf,
        MeshFormat::Usd => Format::Usd,
        MeshFormat::Usdz => Format::Usdz,
        MeshFormat::Dae => Format::Dae,
        MeshFormat::Zae => Format::Zae,
        MeshFormat::ThreeDs => Format::ThreeDs,
        MeshFormat::Lwo => Format::Lwo,
        MeshFormat::Fbx => Format::Fbx,
        MeshFormat::Ifc => Format::Ifc,
        MeshFormat::X3d => Format::X3d,
        MeshFormat::Vrml => Format::Vrml,
        MeshFormat::Rhino => Format::Rhino,
        MeshFormat::Amf => Format::Amf,
        MeshFormat::Off => Format::Off,
        MeshFormat::Alembic => Format::Alembic,
        MeshFormat::Md2 => Format::Md2,
        MeshFormat::Ms3d => Format::Ms3d,
        MeshFormat::Vox => Format::Vox,
        MeshFormat::Ac3d => Format::Ac3d,
        MeshFormat::DirectX => Format::DirectX,
        MeshFormat::Bvh => Format::Bvh,
    }
}

impl Decoder for MeshDecoder {
    fn format(&self) -> Format {
        self.our_format()
    }
    /// PLY, GLB, USD and binary FBX have signatures; STL (free-text header), OBJ, glTF JSON,
    /// 3MF and USDZ (ZIPs like many others), COLLADA, 3DS, LightWave and ASCII FBX are chosen
    /// by extension.
    fn probe(&self, head: &[u8]) -> bool {
        matches!(
            self.0,
            MeshFormat::Ply
                | MeshFormat::Glb
                | MeshFormat::Usd
                | MeshFormat::Fbx
                | MeshFormat::Vrml
                | MeshFormat::Rhino
                | MeshFormat::Off
                | MeshFormat::Alembic
                | MeshFormat::Md2
                | MeshFormat::Ms3d
                | MeshFormat::Vox
                | MeshFormat::Ac3d
                | MeshFormat::DirectX
                | MeshFormat::Bvh
        ) && MeshFormat::sniff(head) == Some(self.0)
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.0.extensions()
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        // A PLY of Gaussian splats (by its property names).
        if self.0 == MeshFormat::Ply && specular_splat::ply_is_splats(source.bytes()) {
            return crate::splat_dec::open_ply(source);
        }
        // A PLY of vertices alone (a scan) is a point cloud.
        if self.0 == MeshFormat::Ply && specular_points::ply_is_points(source.bytes()) {
            return crate::points_dec::open(specular_points::PointFormat::Ply, source);
        }
        let resolver = DirResolver::for_file(source.path()).climbing(self.0.climb());
        let read = specular_mesh::read(self.0, source.bytes(), &resolver, &Limits::default());
        // A Rhino model with nothing to draw (NURBS without saved render meshes) shows the
        // preview Rhino saved in it, when there is one.
        if self.0 == MeshFormat::Rhino
            && let Err(specular_mesh::Error::Unsupported(why)) = &read
            && let Some(bmp) = specular_mesh::rhino::preview_bmp(source.bytes())
            && let Ok(inner) =
                Registry::default().open(Arc::new(Source::from_vec("preview.bmp", bmp)))
        {
            let what = if why == specular_mesh::rhino::NO_RENDER_MESHES {
                "Rhino model without saved render meshes"
            } else {
                "Rhino model"
            };
            return Ok(crate::preview_doc::preview_session(
                source,
                inner,
                Format::Rhino,
                what,
            ));
        }
        let mesh = read.map_err(|e| match e {
            specular_mesh::Error::TooLarge(_) => DecodeError::TooLarge,
            specular_mesh::Error::Unsupported(m) => DecodeError::Unsupported(m),
            specular_mesh::Error::Malformed(m) => DecodeError::Malformed(m),
            specular_mesh::Error::Empty => DecodeError::Malformed("no triangles".into()),
        })?;
        let header = match self.0 {
            MeshFormat::Rhino => Some(crate::HeaderFacts {
                summary: Vec::new(),
                fields: specular_mesh::rhino::facts(source.bytes()),
            }),
            MeshFormat::Alembic => alembic_facts(source.bytes()),
            _ => None,
        };
        Ok(session_with_header(source, self.our_format(), mesh, header))
    }
}

/// An Alembic archive's metadata and objects (each with its schema) for the info panel.
fn alembic_facts(bytes: &[u8]) -> Option<crate::HeaderFacts> {
    /// Objects listed at most.
    const MAX_LISTED: usize = 200;
    let a = specular_mesh::alembic::archive(bytes).ok()?;
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut summary = Vec::new();
    for (k, v) in &a.metadata {
        let name = match k.as_str() {
            "_ai_Application" => "Application",
            "_ai_DateWritten" => "Written",
            "_ai_Description" => "Description",
            "_ai_AlembicVersion" => "Alembic version",
            _ => continue,
        };
        if k == "_ai_Application" {
            summary.push(("creator", v.clone()));
        }
        if k == "_ai_DateWritten" {
            summary.push(("created", v.clone()));
        }
        fields.push((name.to_owned(), v.clone()));
    }
    fields.push(("Objects".to_owned(), a.objects.len().to_string()));
    for o in a.objects.iter().take(MAX_LISTED) {
        let schema = o
            .schema
            .strip_prefix("AbcGeom_")
            .unwrap_or(&o.schema)
            .split("_v")
            .next()
            .unwrap_or("");
        fields.push((
            o.path.clone(),
            if schema.is_empty() {
                "—".into()
            } else {
                schema.to_owned()
            },
        ));
    }
    Some(crate::HeaderFacts { summary, fields })
}

/// A session showing `mesh` (read by another reader: VTK) as `format`.
pub(crate) fn session(source: Arc<Source>, format: Format, mesh: Mesh) -> Arc<dyn Session> {
    session_with_header(source, format, mesh, None)
}

/// [`session`] with the file's header facts for the info panel (STEP and IGES).
pub(crate) fn session_with_header(
    source: Arc<Source>,
    format: Format,
    mesh: Mesh,
    header: Option<crate::HeaderFacts>,
) -> Arc<dyn Session> {
    session_with_ifc(source, format, mesh, header, None)
}

/// [`session_with_header`] for a building model, with its elements.
pub(crate) fn session_with_ifc(
    source: Arc<Source>,
    format: Format,
    mesh: Mesh,
    header: Option<crate::HeaderFacts>,
    ifc: Option<Arc<crate::IfcIndex>>,
) -> Arc<dyn Session> {
    session_with_pmi(source, format, mesh, header, ifc, None)
}

/// [`session_with_ifc`] for a CAD model, with its PMI.
pub(crate) fn session_with_pmi(
    source: Arc<Source>,
    format: Format,
    mesh: Mesh,
    header: Option<crate::HeaderFacts>,
    ifc: Option<Arc<crate::IfcIndex>>,
    pmi: Option<Arc<crate::pmi::Pmi>>,
) -> Arc<dyn Session> {
    let textures = decode_textures(&mesh, &source);
    let (w, h) = preview_size(&mesh);
    let tris = mesh.triangle_count();
    let info = ImageInfo {
        format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding: format!("mesh, {tris} triangles, {} vertices", mesh.vertex_count()),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Arc::new(MeshSession {
        source,
        info,
        model: MeshModel {
            mesh: Arc::new(mesh),
            textures: Arc::new(textures),
            ifc,
        },
        cache: Mutex::new(None),
        header,
        pmi,
    })
}

/// The preview size: the long side [`PREVIEW_LONG_SIDE`], the aspect of the bounding box's
/// corners seen from the iso view (the preview renderer fits the model inside).
fn preview_size(mesh: &Mesh) -> (u32, u32) {
    preview_size_of(mesh.bounds, mesh.up)
}

/// [`preview_size`] of a box seen with `up` up.
pub(crate) fn preview_size_of(b: specular_mesh::Bounds, up: specular_mesh::Up) -> (u32, u32) {
    let [right, down, _] = preview::ISO;
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for i in 0..8 {
        let p = [
            if i & 1 == 0 { b.min[0] } else { b.max[0] },
            if i & 2 == 0 { b.min[1] } else { b.max[1] },
            if i & 4 == 0 { b.min[2] } else { b.max[2] },
        ];
        let q = preview::canonical(p, up);
        let d = |a: [f32; 3]| a[0] * q[0] + a[1] * q[1] + a[2] * q[2];
        for (k, v) in [d(right), d(down)].into_iter().enumerate() {
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
    }
    let (ew, eh) = (hi[0] - lo[0], hi[1] - lo[1]);
    if !(ew.is_finite() && eh.is_finite()) || ew <= 0.0 && eh <= 0.0 {
        return (PREVIEW_LONG_SIDE, PREVIEW_LONG_SIDE);
    }
    // Not thinner than 1:4, so a flat part still shows as a picture.
    let aspect = (ew / eh.max(1e-30)).clamp(0.25, 4.0);
    let l = PREVIEW_LONG_SIDE as f32;
    if aspect >= 1.0 {
        (PREVIEW_LONG_SIDE, (l / aspect).round().max(1.0) as u32)
    } else {
        ((l * aspect).round().max(1.0) as u32, PREVIEW_LONG_SIDE)
    }
}

/// Decodes the mesh's images in parallel; an image that fails to decode stays `None` (its
/// materials draw their colour alone).
pub(crate) fn decode_textures(mesh: &Mesh, parent: &Source) -> Vec<Option<Texture>> {
    use rayon::prelude::*;
    if mesh.images.is_empty() {
        return Vec::new();
    }
    let registry = Registry::default();
    mesh.images
        .par_iter()
        .map(|img| match img {
            ImageData::Bytes { bytes, mime } => {
                decode_texture(&registry, bytes, mime.as_deref(), parent)
            }
            ImageData::Missing { .. } => None,
        })
        .collect()
}

fn decode_texture(
    registry: &Registry,
    bytes: &[u8],
    mime: Option<&str>,
    parent: &Source,
) -> Option<Texture> {
    // A name for decoders chosen by extension (TGA); PNG, JPEG, WebP… are recognised by
    // content.
    let name = match mime {
        Some("image/jpeg") => "texture.jpg",
        Some("image/png") => "texture.png",
        Some("image/webp") => "texture.webp",
        Some("image/ktx2") => "texture.ktx2",
        Some("image/vnd-ms.dds") => "texture.dds",
        _ => "texture",
    };
    // Nested below the model (a texture that is itself a model is opened at most a few
    // levels deep), and on rayon threads, so not covered by the opening thread's nesting.
    let source = Arc::new(Source::from_vec(name, bytes.to_vec()).within(parent));
    let session = registry.open(source).ok()?;
    let info = session.info();
    let long = info.width.max(info.height);
    let scale = info
        .scales
        .iter()
        .copied()
        .find(|&s| long.div_ceil(s) <= MAX_TEXTURE_SIDE)
        .or_else(|| info.scales.last().copied())
        .unwrap_or(1);
    // A header claiming a huge texture with no reduced scale would be decoded whole.
    let (w, h) = (info.width.div_ceil(scale), info.height.div_ceil(scale));
    if u64::from(w) * u64::from(h) > MAX_TEXTURE_DECODE_PIXELS {
        return None;
    }
    let raster = session
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .ok()?;
    let mut tex = to_rgba8(&raster)?;
    while tex.width.max(tex.height) > MAX_TEXTURE_SIDE {
        tex = halve(&tex);
    }
    Some(tex)
}

/// sRGB RGBA8 of a decoded raster, alpha kept.
fn to_rgba8(r: &Raster) -> Option<Texture> {
    let (w, h) = (r.width as usize, r.height as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let pl = r.planes.first()?;
    let mut rgba = vec![0u8; w * h * 4];
    match r.layout {
        PixelLayout::Rgba => {
            for (y, row) in rgba.chunks_exact_mut(w * 4).enumerate() {
                let src = pl.data.get(y * pl.stride..y * pl.stride + w * 4)?;
                row.copy_from_slice(src);
            }
        }
        PixelLayout::Rgba16(_) | PixelLayout::Gray16 | PixelLayout::GrayAlpha16 => {
            let ch = match r.layout {
                PixelLayout::Rgba16(_) => 4,
                PixelLayout::Gray16 => 1,
                _ => 2,
            };
            for (y, row) in rgba.chunks_exact_mut(w * 4).enumerate() {
                let src = pl.data.get(y * pl.stride..y * pl.stride + w * ch * 2)?;
                for (x, out) in row.chunks_exact_mut(4).enumerate() {
                    let s = |c: usize| {
                        let i = (x * ch + c) * 2;
                        (u16::from_ne_bytes([src[i], src[i + 1]]) >> 8) as u8
                    };
                    match ch {
                        4 => out.copy_from_slice(&[s(0), s(1), s(2), s(3)]),
                        2 => out.copy_from_slice(&[s(0), s(0), s(0), s(1)]),
                        _ => out.copy_from_slice(&[s(0), s(0), s(0), 255]),
                    }
                }
            }
        }
        PixelLayout::Rgba16F(_) => {
            for (y, row) in rgba.chunks_exact_mut(w * 4).enumerate() {
                let src = pl.data.get(y * pl.stride..y * pl.stride + w * 8)?;
                for (x, out) in row.chunks_exact_mut(4).enumerate() {
                    for (c, o) in out.iter_mut().enumerate() {
                        let i = (x * 4 + c) * 2;
                        let v = half_to_f32(u16::from_ne_bytes([src[i], src[i + 1]]));
                        let v = if c == 3 { v } else { linear_to_srgb(v) };
                        *o = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
            }
        }
        _ => {
            let r8 = crate::gainmap::to_rgba8(r);
            let pl = r8.planes.first()?;
            for (y, row) in rgba.chunks_exact_mut(w * 4).enumerate() {
                row.copy_from_slice(pl.data.get(y * pl.stride..y * pl.stride + w * 4)?);
            }
        }
    }
    Some(Texture {
        width: r.width,
        height: r.height,
        rgba,
    })
}

fn half_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// A 2×2 box filter (odd edges repeat).
fn halve(t: &Texture) -> Texture {
    let (w, h) = (t.width as usize, t.height as usize);
    let (nw, nh) = (w.div_ceil(2), h.div_ceil(2));
    let mut rgba = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..4 {
                let mut sum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(w - 1), (2 * y + dy).min(h - 1));
                    sum += t.rgba[(sy * w + sx) * 4 + c] as u32;
                }
                rgba[(y * nw + x) * 4 + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    Texture {
        width: nw as u32,
        height: nh as u32,
        rgba,
    }
}

struct MeshSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: MeshModel,
    /// The last whole preview rendered, by scale: regions crop it.
    cache: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
    header: Option<crate::HeaderFacts>,
    pmi: Option<Arc<crate::pmi::Pmi>>,
}

impl MeshSession {
    fn rendered(&self, scale: u32) -> Arc<Vec<u8>> {
        if let Some((s, px)) = &*self.cache.lock()
            && *s == scale
        {
            return px.clone();
        }
        let (w, h) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let px = Arc::new(preview::render(
            &self.model.mesh,
            &self.model.textures,
            w,
            h,
        ));
        *self.cache.lock() = Some((scale, px.clone()));
        px
    }
}

impl Session for MeshSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// A rendering: transparency is not a property of the file.
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn scaled_decode_cheap(&self) -> bool {
        true
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (x, y, w, h) = match req.region {
            Some(r) => {
                let x = r.x.min(sw);
                let y = r.y.min(sh);
                (x, y, r.w.min(sw - x), r.h.min(sh - y))
            }
            None => (0, 0, sw, sh),
        };
        if w == 0 || h == 0 {
            return Err(DecodeError::Unsupported("empty region".into()));
        }
        let full = self.rendered(scale);
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let data = if (x, y, w, h) == (0, 0, sw, sh) {
            full.as_ref().clone()
        } else {
            let mut out = Vec::with_capacity(w as usize * h as usize * 4);
            for row in y..y + h {
                let o = (row as usize * sw as usize + x as usize) * 4;
                out.extend_from_slice(&full[o..o + w as usize * 4]);
            }
            out
        };
        Ok(Raster {
            width: w,
            height: h,
            origin: (x, y),
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.clamp(1, 8) as u64;
        let whole = self.info.width as u64 * self.info.height as u64 / (s * s);
        // The output, the cached whole preview, and the renderer's 2×2 supersampled colour
        // and depth.
        whole * 4 * 2 + whole * 4 * 8
    }
    fn memory_bytes(&self) -> usize {
        let cached = self.cache.lock().as_ref().map_or(0, |(_, px)| px.len());
        let tex: usize = self
            .model
            .textures
            .iter()
            .flatten()
            .map(|t| t.rgba.len())
            .sum();
        self.model.mesh.memory_bytes() + tex + cached
    }
    fn trim_memory(&self) -> usize {
        self.cache.lock().take().map_or(0, |(_, px)| px.len())
    }
    fn mesh(&self) -> Option<&MeshModel> {
        Some(&self.model)
    }
    fn pmi(&self) -> Option<&Arc<crate::pmi::Pmi>> {
        self.pmi.as_ref()
    }
    fn header(&self) -> Option<&crate::HeaderFacts> {
        self.header.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_mesh::fixtures;

    fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
        Registry::default()
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    #[test]
    fn every_format_opens_as_a_mesh() {
        let cases: Vec<(&str, Vec<u8>, Format)> = vec![
            ("cube.stl", fixtures::stl_binary(1.0), Format::Stl),
            ("cube.stl", fixtures::stl_ascii(), Format::Stl),
            ("cube.obj", fixtures::obj(), Format::Obj),
            (
                "cube.ply",
                fixtures::ply("binary_little_endian"),
                Format::Ply,
            ),
            // By signature whatever the name.
            (
                "cube.bin",
                fixtures::ply("binary_little_endian"),
                Format::Ply,
            ),
            ("cube.gltf", fixtures::gltf_embedded(), Format::Gltf),
            ("cube.glb", fixtures::glb(), Format::Glb),
            ("cube.dat", fixtures::glb(), Format::Glb),
            ("cube.3mf", fixtures::threemf(), Format::ThreeMf),
            ("cube.x3d", fixtures::x3d(), Format::X3d),
            ("cube.wrl", fixtures::vrml(), Format::Vrml),
            // By its "#VRML" line whatever the name.
            ("cube.txt", fixtures::vrml(), Format::Vrml),
            ("cube.amf", fixtures::amf(), Format::Amf),
            ("cube.ifc", fixtures::ifc(), Format::Ifc),
        ];
        for (name, bytes, format) in cases {
            let s = open(name, bytes);
            assert_eq!(s.info().format, format, "{name}");
            let m = s.mesh().expect("a mesh");
            // One cube; the glTF scenes place it twice.
            assert_eq!(m.mesh.triangle_count() % 12, 0, "{name}");
            assert!(m.mesh.triangle_count() > 0);
            let r = s
                .decode(&DecodeRequest::full(4), &CancelToken::new())
                .unwrap();
            assert_eq!(r.width, s.info().width.div_ceil(4));
            // Something drawn, something transparent.
            let a: Vec<u8> = r.planes[0].data.chunks_exact(4).map(|p| p[3]).collect();
            assert!(a.contains(&0) && a.contains(&255), "{name}");
        }
    }

    #[test]
    fn regions_crop_the_whole_preview() {
        let s = open("cube.stl", fixtures::stl_binary(1.0));
        let c = CancelToken::new();
        let whole = s.decode(&DecodeRequest::full(2), &c).unwrap();
        let region = crate::Rect {
            x: 37,
            y: 50,
            w: 101,
            h: 60,
        };
        let part = s
            .decode(
                &DecodeRequest {
                    region: Some(region),
                    ..DecodeRequest::full(2)
                },
                &c,
            )
            .unwrap();
        assert_eq!((part.width, part.height, part.origin), (101, 60, (37, 50)));
        let ws = whole.planes[0].stride;
        for y in 0..60usize {
            let a = &whole.planes[0].data[(50 + y) * ws + 37 * 4..][..101 * 4];
            let b = &part.planes[0].data[y * 101 * 4..][..101 * 4];
            assert_eq!(a, b);
        }
        assert!(s.memory_bytes() > 0);
        assert!(s.trim_memory() > 0);
    }

    /// The Rhino corpus: models with meshes open as meshes with their facts; NURBS-only
    /// ones show their saved preview; the rest are refused with a reason.
    #[test]
    fn rhino_models_meshes_previews_and_refusals() {
        let dir = std::env::var_os("SPECULAR_MODELS_EXTRA_CORPUS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tools/corpus/models-extra"
                ))
            })
            .join("rhino");
        let mut files = Vec::new();
        for sub in ["gen", "opennurbs", "rhino3dm"] {
            if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
                files.extend(rd.flatten().map(|e| e.path()));
            }
        }
        files.retain(|p| p.extension().is_some_and(|e| e == "3dm"));
        if files.is_empty() {
            eprintln!("skipped: no Rhino corpus at {}", dir.display());
            return;
        }
        let (mut meshes, mut previews, mut refused) = (0, 0, 0);
        for p in files {
            let bytes = std::fs::read(&p).unwrap();
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            match Registry::default().open(Arc::new(Source::from_vec(&name, bytes))) {
                Ok(s) => {
                    assert_eq!(s.info().format, Format::Rhino, "{name}");
                    if s.mesh().is_some() {
                        let h = s.header().expect("facts");
                        assert!(
                            h.fields.iter().any(|(k, _)| k == "Rhino version"),
                            "{name}: {:?}",
                            h.fields
                        );
                        meshes += 1;
                    } else {
                        assert!(s.info().coding.contains("embedded preview only"), "{name}");
                        previews += 1;
                    }
                }
                Err(e) => {
                    assert!(matches!(e, DecodeError::Unsupported(_)), "{name}: {e}");
                    refused += 1;
                }
            }
        }
        eprintln!("Rhino: {meshes} meshes, {previews} previews, {refused} refused");
        assert!(meshes > 0 && previews > 0, "{meshes} {previews}");
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        for name in ["x.stl", "x.obj", "x.gltf", "x.3mf"] {
            let r =
                Registry::default().open(Arc::new(Source::from_vec(name, b"nonsense".to_vec())));
            assert!(r.is_err(), "{name}");
        }
    }

    #[test]
    fn textures_decode_through_our_decoders() {
        let png = tiny_png();
        let reg = Registry::default();
        let t = decode_texture(
            &reg,
            &png,
            Some("image/png"),
            &Source::from_vec("m.glb", vec![]),
        )
        .expect("decoded");
        assert_eq!((t.width, t.height), (4, 2));
        assert_eq!(&t.rgba[..4], &[255, 0, 0, 255]);
        let big = Texture {
            width: 5,
            height: 3,
            rgba: vec![200; 5 * 3 * 4],
        };
        let h = halve(&big);
        assert_eq!((h.width, h.height), (3, 2));
        assert!(h.rgba.iter().all(|&v| v == 200));
        assert!((half_to_f32(0x3c00) - 1.0).abs() < 1e-6);
    }

    /// A 4×2 RGBA8 PNG with a stored (uncompressed) zlib stream; first pixel red.
    fn tiny_png() -> Vec<u8> {
        fn crc(data: &[u8]) -> u32 {
            let mut c = !0u32;
            for &b in data {
                c ^= b as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 {
                        0xedb8_8320 ^ (c >> 1)
                    } else {
                        c >> 1
                    };
                }
            }
            !c
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc(&body).to_be_bytes());
        }
        let mut raw = Vec::new();
        for y in 0..2u8 {
            raw.push(0);
            for x in 0..4u8 {
                if (x, y) == (0, 0) {
                    raw.extend_from_slice(&[255, 0, 0, 255]);
                } else {
                    raw.extend_from_slice(&[x * 60, y * 120, 90, 255]);
                }
            }
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &v in &raw {
            a = (a + v as u32) % 65521;
            b = (b + a) % 65521;
        }
        let mut z = vec![0x78, 0x01, 1];
        z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
        z.extend_from_slice(&raw);
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&4u32.to_be_bytes());
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}
