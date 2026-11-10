//! GPU textures (DDS, KTX, KTX2, PVR v3, VTF, `.astc`, `.basis`, PKM, Firaxis FTEX) through the in-tree `specular-texture` decoder
//! (CPU decode of BCn, ETC1/ETC2/EAC, ASTC, PVRTC1 and Basis Universal to RGBA8).
//!
//! Array layers, cube faces and depth slices are pages ([`Session::page_count`]); the session
//! shows the first, [`open_texture_page`] opens the others. Mip levels serve
//! the downscaled requests. Uncompressed and block-compressed data decode regions directly
//! (only the blocks touched); KTX2 supercompressed (zstd / zlib) levels are decoded whole once
//! per scale, cached and cropped. KTX2 `KTXorientation` "ru" (bottom-up rows) becomes a vertical
//! flip orientation.
//!
//! Float data (half, float, BC6H, R11G11B10F, RGB9E5) is taken as linear light, clamped to 0..1
//! and sRGB encoded; 16-bit UNORM is rounded to 8 bits (see `specular-texture`).

use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Rect, Result, Session,
};
use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_texture as tx;
use std::sync::Arc;

/// Whole levels above this size are not cached (decoded per request).
const LEVEL_CACHE_MAX: u64 = 768 << 20;

/// Which container a [`TextureDecoder`] handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dds,
    Ktx2,
    Ktx,
    Pvr,
    Vtf,
    /// The `.astc` file of astcenc.
    Astc,
    /// The Basis Universal `.basis` file.
    Basis,
    /// Ericsson's PKM (ETC1, ETC2, EAC).
    Pkm,
    /// Firaxis FTEX (`.ftc`, `.ftu`).
    Ftex,
}

pub struct TextureDecoder(pub Kind);

fn map_err(e: tx::Error) -> DecodeError {
    match e {
        tx::Error::Truncated(s) => DecodeError::Truncated(s.into()),
        tx::Error::Malformed(s) => DecodeError::Malformed(s.into()),
        tx::Error::Unsupported(s) => DecodeError::Unsupported(s),
        tx::Error::TooLarge => DecodeError::TooLarge,
        tx::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        tx::Error::Cancelled => DecodeError::Cancelled,
        tx::Error::BadRegion => DecodeError::Unsupported("region outside the image".into()),
    }
}

impl Decoder for TextureDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Dds => Format::Dds,
            Kind::Ktx2 => Format::Ktx2,
            Kind::Ktx => Format::Ktx,
            Kind::Pvr => Format::Pvr,
            Kind::Vtf => Format::Vtf,
            Kind::Astc => Format::Astc,
            Kind::Basis => Format::Basis,
            Kind::Pkm => Format::Pkm,
            Kind::Ftex => Format::Ftex,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Dds => tx::is_dds(head),
            Kind::Ktx2 => tx::is_ktx2(head),
            Kind::Ktx => tx::is_ktx1(head),
            Kind::Pvr => tx::is_pvr(head),
            Kind::Vtf => tx::is_vtf(head),
            Kind::Astc => tx::is_astc(head),
            Kind::Basis => tx::is_basis(head),
            Kind::Pkm => tx::is_pkm(head),
            Kind::Ftex => tx::is_ftex(head),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Dds => &["dds"],
            Kind::Ktx2 => &["ktx2"],
            Kind::Ktx => &["ktx"],
            Kind::Pvr => &["pvr"],
            Kind::Vtf => &["vtf"],
            Kind::Astc => &["astc"],
            Kind::Basis => &["basis"],
            Kind::Pkm => &["pkm"],
            Kind::Ftex => &["ftc", "ftu"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_page(self.format(), source, 0)
    }
}

/// Open image `page` (0..[`Session::page_count`]) of a texture: array layers, then cube faces,
/// then depth slices.
pub fn open_texture_page(source: Arc<Source>, page: u32) -> Result<Arc<dyn Session>> {
    let b = source.bytes();
    let format = if tx::is_dds(b) {
        Format::Dds
    } else if tx::is_ktx2(b) {
        Format::Ktx2
    } else if tx::is_ktx1(b) {
        Format::Ktx
    } else if tx::is_pvr(b) {
        Format::Pvr
    } else if tx::is_vtf(b) {
        Format::Vtf
    } else if tx::is_astc(b) {
        Format::Astc
    } else if tx::is_basis(b) {
        Format::Basis
    } else if tx::is_pkm(b) {
        Format::Pkm
    } else if tx::is_ftex(b) {
        Format::Ftex
    } else {
        return Err(DecodeError::Malformed("not a texture file".into()));
    };
    open_page(format, source, page)
}

fn open_page(format: Format, source: Arc<Source>, page: u32) -> Result<Arc<dyn Session>> {
    let mut t = tx::Texture::open(source.bytes()).map_err(map_err)?;
    if !t.select_image(page) {
        return Err(DecodeError::Malformed("no such image".into()));
    }
    let mut coding = t.description();
    if t.image_count() > 1 {
        let (layer, face, slice) = t.image_position();
        let mut at = Vec::new();
        if t.layers > 1 {
            at.push(format!("layer {}", layer + 1));
        }
        if t.faces > 1 {
            const FACES: [&str; 7] = ["+X", "−X", "+Y", "−Y", "+Z", "−Z", "sphere"];
            at.push(format!("face {}", FACES[face.min(6) as usize]));
        }
        if t.depth > 1 {
            at.push(format!("slice {}", slice + 1));
        }
        coding += &format!(", {}", at.join(" "));
    }
    // Compressed KTX2 levels must be inflated as a whole: no cheap regions.
    let direct = !t.is_supercompressed();
    let info = ImageInfo {
        format,
        width: t.width,
        height: t.height,
        orientation: if t.flip_y {
            Orientation::from_exif(4)
        } else {
            Orientation::NORMAL
        },
        icc_profile: None,
        has_alpha: t.has_alpha(source.bytes()),
        bit_depth: t.format.bit_depth(),
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: direct,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(TextureSession {
        source,
        format,
        tex: t,
        info,
        levels: Mutex::new(Vec::new()),
    }))
}

struct TextureSession {
    source: Arc<Source>,
    format: Format,
    tex: tx::Texture,
    info: ImageInfo,
    levels: Mutex<Vec<Arc<Raster>>>,
}

impl TextureSession {
    fn opts<'a>(
        scale: u32,
        region: Option<Rect>,
        parallel: bool,
        cancel: &'a CancelToken,
    ) -> tx::DecodeOptions<'a> {
        tx::DecodeOptions {
            scale,
            region: region.map(|r| tx::Region {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }),
            cancel: Some(cancel.flag()),
            parallel,
        }
    }

    fn direct(
        &self,
        scale: u32,
        region: Option<Rect>,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let d = self
            .tex
            .decode(
                self.source.bytes(),
                &Self::opts(scale, region, parallel, cancel),
            )
            .map_err(map_err)?;
        Ok(Raster {
            width: d.width,
            height: d.height,
            origin: d.origin,
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: d.width,
                height: d.height,
                stride: d.stride,
                data: d.data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: d.truncated,
        })
    }

    fn level_bytes(&self, scale: u32) -> u64 {
        u64::from(self.info.width.div_ceil(scale)) * u64::from(self.info.height.div_ceil(scale)) * 4
    }
}

impl Session for TextureSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// Which colour channels the texture stores: one- and two-channel textures (height maps,
    /// normal maps) are not pictures in colour, so the panel says so.
    /// DXT2/DXT4, DX10 alpha mode 2 and the KTX2 DFD flag say premultiplied.
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(if self.tex.premultiplied {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        })
    }
    fn colour_model(&self) -> Option<&'static str> {
        use tx::Format as F;
        let by_channels = |n: u8| match n {
            1 => Some("R"),
            2 => Some("RG"),
            _ => Some("RGB"),
        };
        match self.tex.format {
            F::Bc4 { .. } => Some("R"),
            F::Bc5 { .. } => Some("RG"),
            F::Bc1 { .. } | F::Bc2 | F::Bc3 | F::Bc6h { .. } | F::Bc7 | F::R11G11B10F => {
                Some("RGB")
            }
            F::Rgb9e5 => Some("RGBE"),
            F::EacR11 { .. } => Some("R"),
            F::EacRg11 { .. } | F::Etc1s { rg: true, .. } | F::Uastc { rg: true, .. } => Some("RG"),
            F::Etc1s { .. } | F::Uastc { .. } => Some("RGB"),
            F::Etc1
            | F::Etc2Rgb
            | F::Etc2Rgba1
            | F::Etc2Rgba
            | F::Astc { .. }
            | F::Pvrtc { .. } => Some("RGB"),
            F::Masked { grey: true, .. } => Some("Greyscale"),
            F::Masked { masks, .. } => match masks[..3].iter().filter(|&&m| m != 0).count() {
                0 => None,
                n => by_channels(n as u8),
            },
            F::Unorm16 { channels }
            | F::Snorm8 { channels }
            | F::Half { channels }
            | F::Float { channels } => by_channels(channels),
        }
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn page_count(&self) -> u32 {
        self.tex.image_count()
    }
    fn page_index(&self) -> u32 {
        self.tex.image()
    }
    fn page_label(&self) -> Option<PageLabel> {
        let t = &self.tex;
        if t.image_count() < 2 {
            return None;
        }
        let (layer, face, slice) = t.image_position();
        Some(PageLabel::Texture {
            layer: (t.layers > 1).then_some(layer),
            face: (t.faces > 1).then_some(face),
            slice: (t.depth > 1).then_some(slice),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(self.format, self.source.clone(), page)
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }

    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.max(1);
        if self.info.fast_regions
            || req.region.is_none()
            || self.level_bytes(scale) > LEVEL_CACHE_MAX
        {
            return self.direct(scale, req.region, req.parallel, cancel);
        }
        if let Some(l) = self.levels.lock().iter().find(|l| l.scale == scale) {
            return Ok(req
                .region
                .map_or_else(|| (**l).clone(), |r| crate::jpeg::crop(l, r)));
        }
        let level = Arc::new(self.direct(scale, None, req.parallel, cancel)?);
        {
            let mut ls = self.levels.lock();
            if !ls.iter().any(|l| l.scale == scale) {
                ls.push(level.clone());
            }
        }
        Ok(req
            .region
            .map_or_else(|| (*level).clone(), |r| crate::jpeg::crop(&level, r)))
    }

    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.max(1);
        let token = CancelToken::new();
        let mut m = self
            .tex
            .memory_estimate(&Self::opts(scale, req.region, req.parallel, &token));
        if !self.info.fast_regions && req.region.is_some() {
            m += self.level_bytes(scale);
        }
        m
    }

    fn memory_bytes(&self) -> usize {
        self.levels
            .lock()
            .iter()
            .map(|l| l.planes.iter().map(|p| p.data.len()).sum::<usize>())
            .sum()
    }
    /// Drops the cached levels.
    fn trim_memory(&self) -> usize {
        crate::whole::drop_levels(&self.levels)
    }
}
