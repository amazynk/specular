//! WebP (lossy VP8, lossless VP8L, alpha, animation) through `specular-webp`, Specular's fork of
//! image-webp with a pipelined lossy decoder.
//!
//! Opaque lossy images come out as VP8's own planes, YCbCr 4:2:0 in studio-range BT.601
//! ([`PixelLayout::Yuv`], chroma sited centred), converted where they are shown; everything
//! else as RGBA. The codec cannot decode at reduced size, so scales are box-filtered from the full
//! decode. Regions crop a cached whole-image level.

use crate::{
    AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo, Result,
    Session,
};
use parking_lot::Mutex;
use specular_core::hdr::HdrEncoding;
use specular_core::yuv::{YuvEncoding, YuvMatrix};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_webp::{Animation, Coding, Decoded, DecodingError, Pixels, Plane, WebpImage};
use std::sync::Arc;
use std::time::Duration;

pub struct WebpDecoder;

fn map_err(e: DecodingError) -> DecodeError {
    match e {
        DecodingError::IoError(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            DecodeError::Truncated(e.to_string())
        }
        DecodingError::ImageTooLarge | DecodingError::MemoryLimitExceeded => DecodeError::TooLarge,
        DecodingError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        DecodingError::UnsupportedFeature(s) => DecodeError::Unsupported(s),
        other => DecodeError::Malformed(other.to_string()),
    }
}

/// `RIFF....WEBP`.
fn is_webp(head: &[u8]) -> bool {
    head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP"
}

struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl Decoder for WebpDecoder {
    fn format(&self) -> Format {
        Format::WebP
    }
    fn probe(&self, head: &[u8]) -> bool {
        is_webp(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["webp"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let img = WebpImage::parse(SourceBytes(source.clone())).map_err(map_err)?;
        let i = img.info();
        let orientation = i
            .exif
            .as_deref()
            .map(|e| e.strip_prefix(b"Exif\0\0").unwrap_or(e))
            .and_then(crate::exif::exif_orientation)
            .map(Orientation::from_exif)
            .unwrap_or(Orientation::NORMAL);
        let animation = (i.animated && i.frame_count > 1).then_some(AnimationInfo {
            frame_count: Some(i.frame_count),
            loop_count: i.loop_count,
        });
        let mut coding = match i.coding {
            Coding::Lossy if i.has_alpha => "lossy (VP8) + alpha".to_string(),
            Coding::Lossy => "lossy (VP8), YCbCr 4:2:0".to_string(),
            Coding::Lossless => "lossless (VP8L)".to_string(),
        };
        if i.animated {
            coding += &format!(", animated, {} frames", i.frame_count);
        }
        let info = ImageInfo {
            format: Format::WebP,
            width: i.width,
            height: i.height,
            orientation,
            icc_profile: i.icc_profile.clone().map(Arc::new),
            has_alpha: i.has_alpha,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(WebpSession {
            source,
            info,
            img,
            animation,
            levels: Mutex::new(Vec::new()),
        }))
    }
}

fn plane(p: Plane, div: u8) -> RasterPlane {
    RasterPlane {
        width: p.width,
        height: p.height,
        stride: p.stride,
        data: p.data,
        h_div: div,
        v_div: div,
    }
}

/// VP8's planes: BT.601, studio range, 8 bits, chroma between the luma columns (libwebp's
/// fancy upsampler is the bilinear filter of centred chroma). Kept in studio range: expanded
/// to full range in 8 bits they clipped saturated colours (up to 25/255 off libwebp).
pub(crate) const VP8_YUV: YuvEncoding = YuvEncoding {
    matrix: YuvMatrix::Bt601,
    full_range: false,
    bits: 8,
    cosited_x: false,
    cosited_y: false,
    bottom_y: false,
    right_x: false,
    alpha: false,
    colour: HdrEncoding::SRGB,
};

fn to_raster(d: Decoded) -> Raster {
    let (layout, planes) = match d.pixels {
        Pixels::YCbCr420 { y, cb, cr } => (
            PixelLayout::Yuv(VP8_YUV),
            vec![plane(y, 1), plane(cb, 2), plane(cr, 2)],
        ),
        Pixels::Rgba(p) => (PixelLayout::Rgba, vec![plane(p, 1)]),
    };
    Raster {
        width: d.width,
        height: d.height,
        origin: (0, 0),
        scale: d.scale,
        layout,
        planes,
        truncated: false,
    }
}

struct WebpSession {
    source: Arc<Source>,
    info: ImageInfo,
    img: WebpImage<SourceBytes>,
    animation: Option<AnimationInfo>,
    /// Decoded whole-image levels (for region requests and repeated decodes).
    levels: Mutex<Vec<Arc<Raster>>>,
}

impl WebpSession {
    fn level(&self, scale: u32, parallel: bool, cancel: &CancelToken) -> Result<Arc<Raster>> {
        if let Some(r) = self.levels.lock().iter().find(|r| r.scale == scale) {
            return Ok(r.clone());
        }
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let d = self.img.decode(scale, parallel).map_err(map_err)?;
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let r = Arc::new(to_raster(d));
        let mut g = self.levels.lock();
        if !g.iter().any(|l| l.scale == scale) {
            g.push(r.clone());
        }
        Ok(r)
    }

    fn out_bytes(&self, scale: u32) -> u64 {
        let s = scale.max(1);
        let px = u64::from(self.info.width.div_ceil(s)) * u64::from(self.info.height.div_ceil(s));
        if self.img.info().coding == Coding::Lossy
            && !self.info.has_alpha
            && !self.img.info().animated
        {
            px * 3 / 2
        } else {
            px * 4
        }
    }
}

impl Session for WebpSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// Lossy VP8 is always YCbCr 4:2:0; lossless VP8L stores ARGB.
    fn colour_model(&self) -> Option<&'static str> {
        Some(match self.img.info().coding {
            Coding::Lossy => "YCbCr",
            Coding::Lossless => "RGB",
        })
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        (self.img.info().coding == Coding::Lossy).then_some("4:2:0")
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.max(1);
        match req.region {
            None => {
                specular_log::debug!(target: specular_log::DECODE, "webp: decode 1/{scale} whole image")
            }
            Some(r) => {
                specular_log::trace!(target: specular_log::DECODE, "webp: 1/{scale} region {}×{}+{}+{}", r.w, r.h, r.x, r.y)
            }
        }
        let level = self.level(scale, req.parallel, cancel)?;
        match req.region {
            Some(r) => Ok(crate::jpeg::crop(&level, r)),
            None => Ok((*level).clone()),
        }
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.max(1);
        // Cached level + returned copy on top of the codec's working set.
        self.img.decode_memory(scale) + self.out_bytes(scale)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.lock().iter().map(|r| r.memory_bytes()).sum()
    }
    /// Drops the cached levels.
    fn trim_memory(&self) -> usize {
        crate::whole::drop_levels(&self.levels)
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.animation.is_none() {
            return Err(DecodeError::Unsupported("not animated".into()));
        }
        let a = Animation::new(SourceBytes(self.source.clone())).map_err(map_err)?;
        Ok(Box::new(WebpFrames {
            a,
            scale: scale.max(1),
        }))
    }
}

/// Sequential frames, composited on the canvas at full size, then box-filtered to the scale.
struct WebpFrames {
    a: Animation<SourceBytes>,
    scale: u32,
}

impl Frames for WebpFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let index = self.a.position();
        let f = self.a.next_frame()?;
        Some(f.map_err(map_err).map(|f| Frame {
            index,
            raster: to_raster(specular_webp::image::downscale_rgba(
                f.image, self.scale, true,
            )),
            delay: Duration::from_millis(u64::from(f.duration_ms)),
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.a.rewind();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-webp/tests/data/"
    );

    fn open(name: &str) -> Arc<dyn Session> {
        let bytes = std::fs::read(format!("{DATA}{name}")).unwrap();
        assert!(WebpDecoder.probe(&bytes));
        WebpDecoder
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    /// Saturated colours: the viewer's conversion of the studio-range planes (the CPU
    /// reference of the GPU shader) within a code of libwebp's RGB (`dwebp -pam`); the
    /// full-range planes clipped them, up to 25/255 off.
    #[test]
    fn saturated_colours_convert_as_libwebp() {
        let s = open("saturated-q90.webp");
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r.layout, PixelLayout::Yuv(VP8_YUV));
        let rgb = specular_core::yuv::to_rgb(&r).unwrap();
        let p = &rgb.planes[0];
        let pam = std::fs::read(format!("{DATA}saturated-q90.pam")).unwrap();
        let px = &pam[pam.windows(7).position(|w| w == b"ENDHDR\n").unwrap() + 7..];
        let w = r.width as usize;
        let mut max = 0;
        for y in 0..r.height as usize {
            for x in 0..w {
                for c in 0..3 {
                    let a = i32::from(p.data[y * p.stride + 4 * x + c]);
                    let b = i32::from(px[(y * w + x) * 4 + c]);
                    max = max.max((a - b).abs());
                }
            }
        }
        assert!(max <= 2, "max {max} off libwebp");
    }

    #[test]
    fn lossy_still_scales_and_regions() {
        let s = open("lossy-simple-160x96.webp");
        assert!(s.animation().is_none());
        let c = CancelToken::new();
        let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(
            (full.width, full.height, full.layout),
            (160, 96, PixelLayout::Yuv(VP8_YUV))
        );
        assert_eq!(full.planes[1].width, 80);
        let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!((half.width, half.height), (80, 48));
        assert!(s.memory_bytes() > 0);
        assert!(s.decode_memory(&DecodeRequest::full(1)) > 0);
    }

    #[test]
    fn animation_frames() {
        let s = open("anim-lossless.webp");
        let a = s.animation().unwrap().clone();
        assert_eq!((a.frame_count, a.loop_count), (Some(3), 3));
        let still = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(still.layout, PixelLayout::Rgba);
        let mut fr = s.frames(2).unwrap();
        let c = CancelToken::new();
        for pass in 0..2 {
            let mut delays = Vec::new();
            while let Some(f) = fr.next_frame(&c) {
                let f = f.unwrap();
                assert_eq!(f.index as usize, delays.len());
                assert_eq!(f.raster.width, still.width.div_ceil(2));
                delays.push(f.delay.as_millis());
            }
            assert_eq!(delays, [100, 50, 70], "pass {pass}");
            fr.rewind().unwrap();
        }
    }

    #[test]
    fn truncated_never_panics() {
        let bytes = std::fs::read(format!("{DATA}anim-mixed.webp")).unwrap();
        let c = CancelToken::new();
        for n in (0..bytes.len()).step_by(7) {
            let src = Arc::new(Source::from_vec("t.webp", bytes[..n].to_vec()));
            if let Ok(s) = WebpDecoder.open(src) {
                let _ = s.decode(&DecodeRequest::full(1), &c);
                if let Ok(mut f) = s.frames(1) {
                    while let Some(Ok(_)) = f.next_frame(&c) {}
                }
            }
        }
    }

    /// Trimming drops the cached levels; the next request decodes them again.
    #[test]
    fn trims_levels() {
        let req = DecodeRequest {
            region: Some(crate::Rect {
                x: 4,
                y: 4,
                w: 16,
                h: 16,
            }),
            ..DecodeRequest::full(1)
        };
        crate::trim_check::assert_trims(&*open("lossy-simple-160x96.webp"), &req);
    }
}
