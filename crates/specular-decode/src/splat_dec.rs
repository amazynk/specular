//! Gaussian splats (`.splat`, 3DGS and compressed PLY, `.spz`) via specular-splat.
//!
//! Like a mesh, splats have no pixel size: the session renders an isometric preview on the
//! CPU whose long side is [`PREVIEW_LONG_SIDE`] px, for thumbnails and the plain image view;
//! the app draws the interactive GPU view from [`Session::splats`]. A PLY is splats when its
//! properties say so (`specular_splat::ply_is_splats`); the mesh decoder hands it over.

use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session, SplatModel};
use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_splat::{Limits, preview};
use std::sync::Arc;

pub const PREVIEW_LONG_SIDE: u32 = 1024;

/// What a splat decoder reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// antimatter15 `.splat`.
    Splat,
    /// Niantic `.spz`.
    Spz,
    /// mkkellogg's `.ksplat`: recognised to be refused.
    Ksplat,
}

pub struct SplatDecoder(pub Kind);

impl Decoder for SplatDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Splat => Format::Splat,
            Kind::Spz => Format::Spz,
            Kind::Ksplat => Format::Ksplat,
        }
    }
    /// Only version 4 `.spz` has a signature of its own ("NGSP"); older ones are gzip and
    /// `.splat` has none, so they go by extension.
    fn probe(&self, head: &[u8]) -> bool {
        self.0 == Kind::Spz && head.starts_with(b"NGSP")
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Splat => &["splat"],
            Kind::Spz => &["spz"],
            Kind::Ksplat => &["ksplat"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let l = Limits::default();
        let s = match self.0 {
            Kind::Splat => specular_splat::read_splat(source.bytes(), &l),
            Kind::Spz => specular_splat::read_spz(source.bytes(), &l),
            Kind::Ksplat => {
                return Err(DecodeError::Unsupported(
                    "KSPLAT files are not supported: convert them to PLY, SPLAT or SPZ".into(),
                ));
            }
        }
        .map_err(map_error)?;
        Ok(session(self.format(), source, s))
    }
}

pub(crate) fn map_error(e: specular_splat::Error) -> DecodeError {
    match e {
        specular_splat::Error::TooLarge(_) => DecodeError::TooLarge,
        specular_splat::Error::Unsupported(m) => DecodeError::Unsupported(m),
        specular_splat::Error::Malformed(m) => DecodeError::Malformed(m),
        specular_splat::Error::Empty => DecodeError::Malformed("no splats".into()),
    }
}

/// Opens a splat PLY (the mesh decoder found its properties).
pub(crate) fn open_ply(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let s = specular_splat::read_ply(source.bytes(), &Limits::default()).map_err(map_error)?;
    Ok(session(Format::Ply, source, s))
}

fn session(format: Format, source: Arc<Source>, s: specular_splat::Splats) -> Arc<dyn Session> {
    let aspect = preview::aspect(&s);
    let l = PREVIEW_LONG_SIDE as f32;
    let (w, h) = if aspect >= 1.0 {
        (PREVIEW_LONG_SIDE, (l / aspect).round().max(1.0) as u32)
    } else {
        ((l * aspect).round().max(1.0) as u32, PREVIEW_LONG_SIDE)
    };
    let mut coding = format!("Gaussian splats ({}), {} splats", s.format.name(), s.len());
    if s.sh_degree > 0 {
        coding = format!("{coding}, harmonics of degree {}", s.sh_degree);
    }
    let info = ImageInfo {
        format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Arc::new(SplatSession {
        source,
        info,
        model: SplatModel {
            splats: Arc::new(s),
        },
        cache: Mutex::new(None),
    })
}

struct SplatSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: SplatModel,
    /// The last whole preview rendered, by scale: regions crop it.
    cache: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

impl SplatSession {
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
        let px = Arc::new(preview::render(&self.model.splats, w, h));
        *self.cache.lock() = Some((scale, px.clone()));
        px
    }
}

impl Session for SplatSession {
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
        // The output, the cached whole preview and the tile bins.
        whole * 4 * 2 + self.model.splats.len() as u64 * 16
    }
    fn memory_bytes(&self) -> usize {
        let cached = self.cache.lock().as_ref().map_or(0, |(_, px)| px.len());
        self.model.splats.memory_bytes() + cached
    }
    fn trim_memory(&self) -> usize {
        self.cache.lock().take().map_or(0, |(_, px)| px.len())
    }
    fn splats(&self) -> Option<&SplatModel> {
        Some(&self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;
    use specular_splat::fixtures;

    fn open(name: &str, bytes: Vec<u8>) -> Result<Arc<dyn Session>> {
        Registry::default().open(Arc::new(Source::from_vec(name, bytes)))
    }

    #[test]
    fn every_splat_format_opens() {
        let g = fixtures::scene(300);
        let cases = [
            ("a.splat", fixtures::splat_file(&g), Format::Splat),
            ("a.ply", fixtures::ply_file(&g, 3, false), Format::Ply),
            ("a.ply", fixtures::compressed_ply_file(&g), Format::Ply),
            ("a.spz", fixtures::spz_file(&g, 3, 1), Format::Spz),
            // Version 4 by its signature whatever the name.
            ("a.bin", fixtures::spz4_file(&g, 1), Format::Spz),
        ];
        for (name, bytes, format) in cases {
            let s = open(name, bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.info().format, format, "{name}");
            let m = s.splats().expect("splats");
            assert_eq!(m.splats.len(), 300);
            assert!(s.mesh().is_none() && s.points().is_none());
            let r = s
                .decode(&DecodeRequest::full(4), &CancelToken::new())
                .unwrap();
            let a: Vec<u8> = r.planes[0].data.chunks_exact(4).map(|p| p[3]).collect();
            assert!(a.contains(&0) && a.iter().any(|&v| v > 200), "{name}");
        }
    }

    #[test]
    fn other_plys_stay_meshes_and_clouds() {
        let mesh = specular_mesh::fixtures::ply("binary_little_endian");
        let s = open("m.ply", mesh).unwrap();
        assert!(s.mesh().is_some() && s.splats().is_none());
    }

    #[test]
    fn refusals_are_plain() {
        let e = open("x.ksplat", vec![0; 5000]).err().expect("refused");
        assert!(e.to_string().contains("KSPLAT"), "{e}");
        assert!(open("x.splat", vec![1; 31]).is_err());
        assert!(open("x.spz", b"not gzip".to_vec()).is_err());
    }
}
