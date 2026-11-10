//! JPEG XL through `specular-jxl`, Specular's in-tree fork of jxl-oxide (safe Rust + `std::simd`).
//!
//! Output is 8-bit sRGB (or the file's colour space with its ICC profile in `ImageInfo`),
//! except HDR stills, which decode wide like AVIF's: PQ and HLG to the 16-bit signal in the
//! file's primaries ([`PixelLayout::Rgba16`]), with or without a gain map (`jhgm` box), and
//! linear float (or brighter than 255 cd/m²) images to linear half floats
//! ([`PixelLayout::Rgba16F`], 1.0 = SDR reference white, see
//! [`JxlFile::decode_linear`]). The renderer shows those as HDR and makes the SDR fallback;
//! a gain map of an HDR (PQ / HLG) base lets it apply the map either way, and an SDR base's map
//! is handed over as for JPEG and AVIF. Animations and HDR without known primaries are tone
//! mapped to 8 bits by the crate. CMYK (a black extra channel) is converted to sRGB through
//! its embedded CMYK profile by specular-color, as JPEG, TIFF and PSD CMYK are. Scales are box-filtered from the full decode (in
//! linear light for linear output); regions crop a cached whole-image level. Large 8-bit
//! VarDCT images offer a 1/8 preview rendered from the LF (DC) image alone.

use crate::gainmap;
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    Result, Session,
};
use parking_lot::Mutex;
use specular_core::{
    CancelToken, GainMap, GainMapParams, HdrEncoding, Orientation, PixelLayout, Primaries, Raster,
    RasterPlane, Source, Transfer,
};
use specular_jxl::{Decoded, Error, Hdr, JxlFile, Layout};
use std::sync::Arc;
use std::time::Duration;

pub struct JxlDecoder;

/// Images at least this large offer the DC preview.
const PREVIEW_MIN_PIXELS: u64 = 2_000_000;

fn map_err(e: Error) -> DecodeError {
    match e {
        Error::NotJxl => DecodeError::UnsupportedFormat,
        Error::Truncated(s) => DecodeError::Truncated(s),
        Error::Malformed(s) => DecodeError::Malformed(s),
        Error::Unsupported(s) => DecodeError::Unsupported(s),
        Error::TooLarge => DecodeError::TooLarge,
    }
}

fn plane(width: u32, height: u32, stride: usize, data: Vec<u8>) -> RasterPlane {
    RasterPlane {
        width,
        height,
        stride,
        data,
        h_div: 1,
        v_div: 1,
    }
}

/// How a still is decoded wide.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wide {
    /// The PQ or HLG signal ([`JxlFile::decode_hdr`]).
    Signal(HdrEncoding),
    /// Linear light ([`JxlFile::decode_linear`]).
    Linear(Primaries),
}

/// `wide`: what a [`Layout::Rgba16`] or [`Layout::Rgba16F`] picture holds.
fn to_raster(d: Decoded, wide: Option<Wide>) -> Raster {
    let (w, h) = (d.width, d.height);
    let n = w as usize * h as usize;
    let (layout, planes) = match d.layout {
        Layout::Gray => (PixelLayout::Gray, vec![plane(w, h, w as usize, d.data)]),
        Layout::Rgba => (PixelLayout::Rgba, vec![plane(w, h, w as usize * 4, d.data)]),
        Layout::Rgba16 => (
            PixelLayout::Rgba16(match wide {
                Some(Wide::Signal(e)) => e,
                _ => HdrEncoding::SRGB,
            }),
            vec![plane(w, h, w as usize * 8, d.data)],
        ),
        Layout::Rgba16F => (
            PixelLayout::Rgba16F(match wide {
                Some(Wide::Linear(p)) => p,
                _ => Primaries::Bt709,
            }),
            vec![plane(w, h, w as usize * 8, d.data)],
        ),
        Layout::Rgb => {
            let mut p = [vec![0u8; n], vec![0u8; n], vec![0u8; n]];
            for (i, px) in d.data.chunks_exact(3).enumerate().take(n) {
                p[0][i] = px[0];
                p[1][i] = px[1];
                p[2][i] = px[2];
            }
            let [r, g, b] = p;
            (
                PixelLayout::Rgb,
                vec![
                    plane(w, h, w as usize, r),
                    plane(w, h, w as usize, g),
                    plane(w, h, w as usize, b),
                ],
            )
        }
    };
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: d.scale,
        layout,
        planes,
        truncated: !d.complete,
    }
}

struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl Decoder for JxlDecoder {
    fn format(&self) -> Format {
        Format::Jxl
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_jxl::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["jxl"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let file = JxlFile::open(SourceBytes(source.clone())).map_err(map_err)?;
        let i = file.info();
        let animation = i.animated.then_some(AnimationInfo {
            frame_count: None,
            loop_count: i.loop_count,
        });
        let mut coding = if i.xyb {
            "VarDCT (lossy)".to_string()
        } else {
            "modular".to_string()
        };
        coding += &format!(
            ", {}{}-bit",
            if i.float_samples { "float " } else { "" },
            i.bits_per_sample
        );
        if i.jpeg_reconstruction {
            coding += ", recompressed JPEG";
        }
        // HDR stills decode wide: PQ / HLG as the signal, linear float (or brighter than SDR)
        // as linear light. A gain map applies to an SDR base as decoded, and to a PQ / HLG base
        // decoded wide; maps on a linear base are not read.
        let params = gain_map_params(&file);
        let still = !i.animated && !i.cmyk;
        let base_primaries = i.primaries.map(primaries).filter(|_| still);
        let wide = match (i.hdr, base_primaries) {
            (Some(h), Some(primaries)) => Some(Wide::Signal(HdrEncoding {
                transfer: match h {
                    Hdr::Pq => Transfer::Pq,
                    Hdr::Hlg => Transfer::Hlg,
                },
                primaries,
            })),
            (None, Some(p)) if i.linear && (i.float_samples || i.intensity_target > 255.0) => {
                Some(Wide::Linear(p))
            }
            _ => None,
        };
        let gain_map = params.filter(|p| match wide {
            None => i.hdr.is_none() && still && !p.base_is_hdr,
            Some(Wide::Signal(_)) => p.base_is_hdr,
            Some(Wide::Linear(_)) => false,
        });
        match (i.hdr, wide) {
            (Some(Hdr::Pq), Some(_)) => coding += ", PQ HDR",
            (Some(Hdr::Hlg), Some(_)) => coding += ", HLG HDR",
            (Some(Hdr::Pq), None) => coding += ", PQ HDR (tone mapped)",
            (Some(Hdr::Hlg), None) => coding += ", HLG HDR (tone mapped)",
            (None, Some(_)) => coding += ", linear HDR",
            (None, None) => {}
        }
        match gain_map {
            Some(p) if p.base_is_hdr => coding += ", gain map (ISO 21496-1, HDR base)",
            Some(_) => coding += ", gain map (ISO 21496-1)",
            None if i.gain_map => coding += ", gain map (not applied)",
            None => {}
        }
        if i.cmyk {
            coding += ", CMYK";
        }
        if i.animated {
            coding += ", animated";
        }
        let info = ImageInfo {
            format: Format::Jxl,
            width: i.width,
            height: i.height,
            orientation: Orientation::from_exif(u8::try_from(i.orientation).unwrap_or(1)),
            // CMYK is converted to sRGB while decoding (its profile is the source's).
            icc_profile: i.output_icc.clone().filter(|_| !i.cmyk).map(Arc::new),
            has_alpha: i.has_alpha,
            bit_depth: u8::try_from(i.bits_per_sample).unwrap_or(16),
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(JxlSession {
            source,
            info,
            file,
            animation,
            gain_map,
            wide,
            levels: Mutex::new(Vec::new()),
        }))
    }
}

fn primaries(p: specular_jxl::Primaries) -> Primaries {
    match p {
        specular_jxl::Primaries::Bt709 => Primaries::Bt709,
        specular_jxl::Primaries::DisplayP3 => Primaries::DisplayP3,
        specular_jxl::Primaries::Bt2020 => Primaries::Bt2020,
    }
}

/// The metadata of a readable `jhgm` gain map of a sane size, in either direction (the
/// caller checks the base matches). The alternate rendition's primaries default to the
/// base's when the bundle has no colour encoding (or an ICC-described one).
fn gain_map_params(file: &JxlFile<SourceBytes>) -> Option<GainMapParams> {
    let g = match file.gain_map()? {
        Ok(g) => g,
        Err(e) => {
            specular_log::debug!(target: specular_log::DECODE, "jxl: gain map not read: {e}");
            return None;
        }
    };
    // An ICC-described base: the profile's primaries.
    let base = file.info().primaries.map(primaries).or_else(|| {
        file.info()
            .original_icc
            .as_deref()
            .and_then(gainmap::icc_primaries)
    });
    let alternate = g.alt_primaries().map(primaries).or(base);
    let p = gainmap::jhgm_params(g.metadata, base, alternate)?;
    let (w, h) = file.gain_map_size().ok()?;
    (w <= gainmap::MAX_MAP_SIDE && h <= gainmap::MAX_MAP_SIDE).then_some(p)
}

struct JxlSession {
    source: Arc<Source>,
    info: ImageInfo,
    file: JxlFile<SourceBytes>,
    animation: Option<AnimationInfo>,
    /// The ISO 21496-1 parameters of a usable gain map (of an SDR base, or of a wide PQ / HLG
    /// base).
    gain_map: Option<GainMapParams>,
    /// How a still decodes wide ([`PixelLayout::Rgba16`] or [`PixelLayout::Rgba16F`]); `None`
    /// for 8-bit.
    wide: Option<Wide>,
    /// Decoded whole-image levels (for region requests and repeated decodes).
    levels: Mutex<Vec<Arc<Raster>>>,
}

impl JxlSession {
    fn level(&self, scale: u32, parallel: bool, cancel: &CancelToken) -> Result<Arc<Raster>> {
        if let Some(r) = self.levels.lock().iter().find(|r| r.scale == scale) {
            return Ok(r.clone());
        }
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        if self.file.info().cmyk {
            let r = Arc::new(self.cmyk_level(scale, parallel)?);
            self.levels.lock().push(r.clone());
            return Ok(r);
        }
        let d = match self.wide {
            Some(Wide::Signal(_)) => self.file.decode_hdr(scale, parallel),
            Some(Wide::Linear(_)) => self.file.decode_linear(scale, parallel),
            None => self.file.decode(scale, parallel),
        }
        .map_err(map_err)?;
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let r = Arc::new(to_raster(d, self.wide));
        let mut g = self.levels.lock();
        if !g.iter().any(|l| l.scale == scale) {
            g.push(r.clone());
        }
        Ok(r)
    }

    /// A CMYK image (black extra channel): its inks through the embedded CMYK profile to
    /// sRGB (specular-color, as JPEG, TIFF and PSD CMYK), then box-averaged to `1/scale`.
    fn cmyk_level(&self, scale: u32, parallel: bool) -> Result<Raster> {
        let c = self.file.decode_cmyk(parallel).map_err(map_err)?;
        let (w, h) = (c.width as usize, c.height as usize);
        let icc = self.file.info().original_icc.as_deref();
        let mut rgba = crate::colour::cmyk_rgba(&c.ink, w, icc, c.alpha.as_deref());
        let s = scale.max(1) as usize;
        let (nw, nh) = (w.div_ceil(s), h.div_ceil(s));
        if s > 1 {
            let mut out = vec![0u8; nw * nh * 4];
            for (y, row) in out.chunks_exact_mut(nw * 4).enumerate() {
                for (x, px) in row.chunks_exact_mut(4).enumerate() {
                    let mut acc = [0u32; 4];
                    let mut n = 0;
                    for sy in y * s..((y + 1) * s).min(h) {
                        for sx in x * s..((x + 1) * s).min(w) {
                            let o = (sy * w + sx) * 4;
                            for (a, &v) in acc.iter_mut().zip(&rgba[o..o + 4]) {
                                *a += u32::from(v);
                            }
                            n += 1;
                        }
                    }
                    for (p, a) in px.iter_mut().zip(acc) {
                        *p = ((a + n / 2) / n.max(1)) as u8;
                    }
                }
            }
            rgba = out;
        }
        Ok(Raster {
            width: nw as u32,
            height: nh as u32,
            origin: (0, 0),
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![plane(nw as u32, nh as u32, nw * 4, rgba)],
            truncated: !c.complete,
        })
    }

    fn out_bytes(&self, scale: u32) -> u64 {
        let s = scale.max(1);
        let px = u64::from(self.info.width.div_ceil(s)) * u64::from(self.info.height.div_ceil(s));
        px * if self.wide.is_some() {
            8
        } else if self.info.has_alpha {
            4
        } else {
            3
        }
    }

    fn decode_gain_map(&self, params: GainMapParams, cancel: &CancelToken) -> Option<GainMap> {
        if cancel.is_cancelled() {
            return None;
        }
        let t = std::time::Instant::now();
        let d = match self.file.decode_gain_map(true) {
            Ok(d) => d,
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "jxl: gain map: {e}");
                return None;
            }
        };
        if cancel.is_cancelled() {
            return None;
        }
        let map = gainmap::to_rgba8(&to_raster(d, None));
        specular_log::debug!(
            target: specular_log::DECODE,
            "jxl: gain map {}×{} decoded in {:.1} ms",
            map.width,
            map.height,
            t.elapsed().as_secs_f64() * 1e3
        );
        Some(GainMap {
            map,
            params,
            source: gainmap::ISO_SOURCE,
        })
    }
}

impl Session for JxlSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// An enum colour encoding (no embedded profile): PQ / HLG, grey, CMYK, or its primaries
    /// (not with the linear transfer).
    fn colour_space(&self) -> Option<&'static str> {
        let i = self.file.info();
        if let Some(h) = i.hdr {
            return Some(match h {
                Hdr::Pq => "pq",
                Hdr::Hlg => "hlg",
            });
        }
        if i.original_icc.is_some() {
            return None;
        }
        if i.cmyk {
            return Some("cmyk");
        }
        if i.gray {
            return Some("grey");
        }
        Some(match (i.primaries?, i.linear) {
            (specular_jxl::Primaries::Bt709, true) => "Linear sRGB",
            (specular_jxl::Primaries::DisplayP3, true) => "Linear Display P3",
            (specular_jxl::Primaries::Bt2020, true) => "Linear Rec. 2020",
            (specular_jxl::Primaries::Bt709, false) => "srgb",
            (specular_jxl::Primaries::DisplayP3, false) => "p3",
            (specular_jxl::Primaries::Bt2020, false) => "rec2020",
        })
    }
    fn colour_model(&self) -> Option<&'static str> {
        let i = self.file.info();
        Some(if i.cmyk {
            "CMYK"
        } else if i.gray {
            "Greyscale"
        } else if i.jpeg_reconstruction {
            // A recompressed JPEG keeps its YCbCr DCT coefficients.
            "YCbCr"
        } else if i.xyb {
            "XYB"
        } else {
            "RGB"
        })
    }
    /// The alpha extra channel; premultiplied when it is marked associated.
    fn alpha(&self) -> Option<Alpha> {
        let i = self.file.info();
        i.has_alpha.then_some(if i.premultiplied_alpha {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        })
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.file.info().original_icc.as_deref()
    }
    /// For large opaque VarDCT stills: a 1/8 image from the LF (DC) data alone (tens of ms
    /// for 24 MP, where a full decode takes hundreds).
    fn decode_preview(&self) -> Option<Result<Raster>> {
        let i = self.file.info();
        let px = u64::from(i.width) * u64::from(i.height);
        if px < PREVIEW_MIN_PIXELS
            || self.wide.is_some()
            || !i.xyb
            || i.has_alpha
            || i.animated
            || i.jpeg_reconstruction
            || i.cmyk
        {
            return None;
        }
        Some(
            self.file
                .decode_dc(true)
                .map(|d| to_raster(d, None))
                .map_err(map_err),
        )
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.max(1);
        match req.region {
            None => {
                specular_log::debug!(target: specular_log::DECODE, "jxl: decode 1/{scale} whole image")
            }
            Some(r) => {
                specular_log::trace!(target: specular_log::DECODE, "jxl: 1/{scale} region {}×{}+{}+{}", r.w, r.h, r.x, r.y)
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
        // Codec working set (f32 planes and the output it converts) plus the cached level and
        // the returned copy.
        let codec = match self.wide {
            Some(_) => self.file.decode_wide_memory(scale),
            None => self.file.decode_memory(scale),
        };
        codec + 2 * self.out_bytes(scale)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.lock().iter().map(|r| r.memory_bytes()).sum()
    }
    /// Drops the cached levels (regions are cropped from them).
    fn trim_memory(&self) -> usize {
        crate::whole::drop_levels(&self.levels)
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn has_gain_map(&self) -> bool {
        self.gain_map.is_some()
    }
    fn gain_map(&self, cancel: &CancelToken) -> Option<GainMap> {
        self.decode_gain_map(self.gain_map?, cancel)
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.animation.is_none() {
            return Err(DecodeError::Unsupported("not animated".into()));
        }
        let scale = scale.max(1);
        let f = self.file.frames(scale, true).map_err(map_err)?;
        Ok(Box::new(JxlFrames {
            source: self.source.clone(),
            f,
            scale,
            index: 0,
        }))
    }
}

/// Sequential keyframes, each composited on the canvas.
struct JxlFrames {
    source: Arc<Source>,
    f: specular_jxl::Frames,
    scale: u32,
    index: u32,
}

impl Frames for JxlFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let r = self.f.next()?;
        let index = self.index;
        self.index += 1;
        Some(r.map_err(map_err).map(|d| Frame {
            index,
            delay: Duration::from_millis(u64::from(d.duration_ms)),
            raster: to_raster(d, None),
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        let file = JxlFile::open(SourceBytes(self.source.clone())).map_err(map_err)?;
        self.f = file.frames(self.scale, true).map_err(map_err)?;
        self.index = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-jxl/tests/data/"
    );

    fn open(name: &str) -> Arc<dyn Session> {
        let bytes = std::fs::read(format!("{DATA}{name}")).unwrap();
        assert!(JxlDecoder.probe(&bytes));
        JxlDecoder
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    #[test]
    fn still_scales_and_regions() {
        let c = CancelToken::new();
        for name in ["vardct.jxl", "modular.jxl", "gray.jxl", "alpha.jxl"] {
            let s = open(name);
            assert!(s.animation().is_none(), "{name}");
            let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
            assert_eq!((full.width, full.height), (s.info().width, s.info().height));
            let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
            assert_eq!(half.width, full.width.div_ceil(2), "{name}");
            if name == "alpha.jxl" {
                assert_eq!(full.layout, PixelLayout::Rgba);
            }
        }
    }

    #[test]
    fn animation_frames() {
        let s = open("anim3.jxl");
        assert!(s.animation().is_some());
        let mut fr = s.frames(2).unwrap();
        let c = CancelToken::new();
        for _ in 0..2 {
            let mut n = 0;
            while let Some(f) = fr.next_frame(&c) {
                let f = f.unwrap();
                assert_eq!(f.index, n);
                assert_eq!(f.raster.width, s.info().width.div_ceil(2));
                n += 1;
            }
            assert_eq!(n, 3);
            fr.rewind().unwrap();
        }
    }

    /// gainmap_sdr.jxl (tests/gen_gainmap.py): an sRGB base with a 16×8 grey ramp map, two
    /// stops at full headroom.
    #[test]
    fn jhgm_sdr_base() {
        let s = open("gainmap_sdr.jxl");
        assert!(s.has_gain_map());
        assert!(s.info().coding.ends_with(", gain map (ISO 21496-1)"));
        let c = CancelToken::new();
        let base = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(base.layout, PixelLayout::Rgb);
        let gm = s.gain_map(&c).unwrap();
        assert_eq!(gm.source, gainmap::ISO_SOURCE);
        let p = gm.params;
        assert!(!p.base_is_hdr);
        assert_eq!((p.base_headroom, p.alt_headroom), (0.0, 2.0));
        assert_eq!(
            (p.gain_min, p.gain_max, p.gamma),
            ([0.0; 3], [2.0; 3], [1.0; 3])
        );
        // 1/64 over the common denominator 10000.
        assert_eq!((p.base_offset, p.alt_offset), ([0.0156; 3], [0.0156; 3]));
        assert_eq!(p.space, Primaries::Bt709);
        assert_eq!(
            (gm.map.width, gm.map.height, gm.map.layout),
            (16, 8, PixelLayout::Rgba)
        );
        let m = &gm.map.planes[0].data;
        for x in 0..16 {
            let v = ((x * 255 + 7) / 15) as u8;
            assert_eq!(&m[x * 4..x * 4 + 4], &[v, v, v, 255]);
        }
        // At full headroom the right edge gets two stops, the left none.
        let w = p.weight(4.0);
        assert_eq!(w, 1.0);
        let lin = |v: u8| specular_core::hdr::srgb_to_linear(f32::from(v) / 255.0);
        let b = lin(base.planes[0].data[31]);
        let g = f32::from(m[15 * 4]) / 255.0;
        let out = p.apply_rgb([b; 3], [g; 3], w)[0];
        assert!((out - ((b + 0.0156) * 4.0 - 0.0156)).abs() < 1e-5);
        let out = p.apply_rgb([b; 3], [0.0; 3], w)[0];
        assert!((out - b).abs() < 1e-6);
    }

    /// gainmap_hdr.jxl: a PQ BT.2100 base with an RGB map down to an sRGB alternate. The
    /// SDR rendition the core makes from it is compared with the sRGB image the script
    /// derived the base from.
    #[test]
    fn jhgm_hdr_base() {
        let s = open("gainmap_hdr.jxl");
        assert!(s.has_gain_map());
        assert!(
            s.info()
                .coding
                .contains(", PQ HDR, gain map (ISO 21496-1, HDR base)")
        );
        assert!(s.decode_preview().is_none());
        let c = CancelToken::new();
        let base = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(
            base.layout,
            PixelLayout::Rgba16(HdrEncoding {
                transfer: Transfer::Pq,
                primaries: Primaries::Bt2020
            })
        );
        assert_eq!(base.planes[0].data.len(), 32 * 16 * 8);
        let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!((half.width, half.height), (16, 8));
        assert!(half.layout.is_wide());
        assert!(s.decode_memory(&DecodeRequest::full(1)) >= 2 * 32 * 16 * 8);

        let gm = s.gain_map(&c).unwrap();
        let p = gm.params;
        assert!(p.base_is_hdr);
        assert_eq!((p.base_headroom, p.alt_headroom), (2.0, 0.0));
        assert_eq!(p.space, Primaries::Bt2020);
        assert_eq!(p.weight(1.0), -1.0);
        assert_eq!((gm.map.width, gm.map.height), (32, 16));

        let peak = specular_core::hdr::peak_from_nits(None);
        let r = specular_core::hdr::sdr_rendition(&base, &gm, peak).unwrap();
        assert_eq!((r.width, r.height, r.layout), (32, 16, PixelLayout::Rgba));
        let out = &r.planes[0].data;
        let (mut sum, mut worst) = (0u32, 0u8);
        for y in 0..16u32 {
            for x in 0..32u32 {
                let want = [40 + 6 * x, 30 + 12 * y, 200 - 4 * x];
                let i = (y * 32 + x) as usize * 4;
                for c in 0..3 {
                    let d = out[i + c].abs_diff(want[c] as u8);
                    sum += u32::from(d);
                    worst = worst.max(d);
                }
            }
        }
        let mean = f64::from(sum) / (32.0 * 16.0 * 3.0);
        eprintln!("jhgm HDR base → SDR: mean |Δ| {mean:.3}, max {worst} (8-bit levels)");
        assert!(mean <= 1.0 && worst <= 3, "mean {mean}, max {worst}");
    }

    /// gainmap_hdr_brob.jxl: gainmap_hdr.jxl with the `jhgm` box Brotli-compressed (`brob`):
    /// the same map and the same SDR rendition.
    #[test]
    fn jhgm_brotli_compressed() {
        let (plain, brob) = (open("gainmap_hdr.jxl"), open("gainmap_hdr_brob.jxl"));
        assert!(brob.has_gain_map());
        assert_eq!(brob.info().coding, plain.info().coding);
        let c = CancelToken::new();
        let (a, b) = (plain.gain_map(&c).unwrap(), brob.gain_map(&c).unwrap());
        assert_eq!(a.map.planes[0].data, b.map.planes[0].data);
        let base = brob.decode(&DecodeRequest::full(1), &c).unwrap();
        let peak = specular_core::hdr::peak_from_nits(None);
        let ra = specular_core::hdr::sdr_rendition(&base, &a, peak).unwrap();
        let rb = specular_core::hdr::sdr_rendition(&base, &b, peak).unwrap();
        assert_eq!(ra.planes[0].data, rb.planes[0].data);
    }

    /// gps_brob.jxl (specular-thumbs' gpsmeta fixtures): a `brob` Exif box reaches the info.
    #[test]
    fn brotli_compressed_exif() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../specular-thumbs/tests/data/gpsmeta/"
        );
        let exif = |name: &str| {
            let f = JxlFile::open(std::fs::read(format!("{dir}{name}")).unwrap()).unwrap();
            f.info().exif.clone()
        };
        let plain = exif("gps.jxl");
        assert!(plain.is_some());
        assert_eq!(exif("gps_brob.jxl"), plain);
    }

    #[test]
    fn jhgm_unusable() {
        // No gain map: 8-bit as before, no map.
        let s = open("vardct.jxl");
        assert!(!s.has_gain_map());
        assert!(!s.info().coding.contains("gain map"));
        // Another bundle version: present, not applied.
        let mut bytes = std::fs::read(format!("{DATA}gainmap_sdr.jxl")).unwrap();
        assert_eq!(&bytes[36..40], b"jhgm");
        bytes[40] = 1;
        let s = JxlDecoder
            .open(Arc::new(Source::from_vec("v1.jxl", bytes)))
            .unwrap();
        assert!(!s.has_gain_map());
        assert!(s.info().coding.ends_with(", gain map (not applied)"));
        assert!(
            s.decode(&DecodeRequest::full(1), &CancelToken::new())
                .is_ok()
        );
        // An HDR base whose map is not applied still decodes wide, without it.
        let mut bytes = std::fs::read(format!("{DATA}gainmap_hdr.jxl")).unwrap();
        let at = (12..bytes.len() - 4)
            .find(|&i| &bytes[i..i + 4] == b"jhgm")
            .unwrap();
        bytes[at + 4] = 7;
        let s = JxlDecoder
            .open(Arc::new(Source::from_vec("v7.jxl", bytes)))
            .unwrap();
        assert!(!s.has_gain_map());
        assert!(
            s.info()
                .coding
                .ends_with(", PQ HDR, gain map (not applied)")
        );
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(
            r.layout,
            PixelLayout::Rgba16(HdrEncoding {
                transfer: Transfer::Pq,
                primaries: Primaries::Bt2020
            })
        );
    }

    /// Wide stills without a map (tests/gen_hdr.py): layouts, the scales, a region of a
    /// cached level, and memory estimates that count the 8-byte output.
    #[test]
    fn hdr_without_map() {
        let c = CancelToken::new();
        let cases = [
            (
                "hdr_pq_p3.jxl",
                ", PQ HDR",
                PixelLayout::Rgba16(HdrEncoding {
                    transfer: Transfer::Pq,
                    primaries: Primaries::DisplayP3,
                }),
            ),
            (
                "hdr_hlg_2020_alpha.jxl",
                ", HLG HDR",
                PixelLayout::Rgba16(HdrEncoding {
                    transfer: Transfer::Hlg,
                    primaries: Primaries::Bt2020,
                }),
            ),
            (
                "hdr_pq_709_lossy.jxl",
                ", PQ HDR",
                PixelLayout::Rgba16(HdrEncoding {
                    transfer: Transfer::Pq,
                    primaries: Primaries::Bt709,
                }),
            ),
            (
                "linear_f32.jxl",
                ", linear HDR",
                PixelLayout::Rgba16F(Primaries::Bt709),
            ),
            (
                "linear_f16_2020_lossy.jxl",
                ", linear HDR",
                PixelLayout::Rgba16F(Primaries::Bt2020),
            ),
        ];
        for (name, tag, layout) in cases {
            let s = open(name);
            assert!(!s.has_gain_map(), "{name}");
            assert!(
                s.info().coding.ends_with(tag),
                "{name}: {}",
                s.info().coding
            );
            assert!(s.info().bit_depth > 8, "{name}");
            assert!(s.decode_preview().is_none(), "{name}");
            let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
            assert_eq!(
                (full.width, full.height, full.layout),
                (32, 16, layout),
                "{name}"
            );
            assert_eq!(full.planes[0].data.len(), 32 * 16 * 8, "{name}");
            assert!(
                s.decode_memory(&DecodeRequest::full(1)) >= 2 * 32 * 16 * 8,
                "{name}"
            );
            for scale in [2, 8] {
                let r = s.decode(&DecodeRequest::full(scale), &c).unwrap();
                assert_eq!(
                    (r.width, r.height, r.layout),
                    (32u32.div_ceil(scale), 16u32.div_ceil(scale), layout),
                    "{name} 1/{scale}"
                );
                let n = u64::from(r.width) * u64::from(r.height);
                assert!(
                    s.decode_memory(&DecodeRequest::full(scale)) >= 2 * n * 8,
                    "{name}"
                );
            }
            // Regions crop the cached level (on a 16-pixel grid).
            let req = DecodeRequest {
                region: Some(crate::Rect {
                    x: 16,
                    y: 0,
                    w: 16,
                    h: 8,
                }),
                ..DecodeRequest::full(1)
            };
            let r = s.decode(&req, &c).unwrap();
            assert_eq!((r.width, r.height, r.layout), (16, 8, layout), "{name}");
            let p = &full.planes[0];
            for y in 0..8 {
                assert_eq!(
                    &r.planes[0].data[y * 16 * 8..][..16 * 8],
                    &p.data[y * p.stride + 16 * 8..][..16 * 8],
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn truncated_never_panics() {
        let bytes = std::fs::read(format!("{DATA}vardct_multigroup.jxl")).unwrap();
        let c = CancelToken::new();
        for n in (0..bytes.len()).step_by(bytes.len() / 40 + 1) {
            let src = Arc::new(Source::from_vec("t.jxl", bytes[..n].to_vec()));
            if let Ok(s) = JxlDecoder.open(src) {
                let _ = s.decode(&DecodeRequest::full(4), &c);
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
        crate::trim_check::assert_trims(&*open("vardct.jxl"), &req);
    }
}
