//! Specular's front end over the forked decoder: whole-file parsing over any byte container,
//! planar output, scaled decoding and animation frames.

use crate::decoder::{DecodingError, LoopCount, WebPDecoder, WebPRiffChunk};
use crate::extended::{FilteringMethod, read_alpha_chunk};
use crate::lossless::LosslessDecoder;
use crate::vp8::{Sink, Vp8Decoder};
use crate::yuv;
use rayon::prelude::*;
use std::io::Cursor;
use std::ops::Range;

/// One 8-bit plane, `stride` bytes per row.
#[derive(Clone, Debug, Default)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub data: Vec<u8>,
}

/// Decoded pixels.
#[derive(Clone, Debug)]
pub enum Pixels {
    /// Y, Cb, Cr planes as VP8 decodes them: **studio-range BT.601** (luma 16–235, chroma
    /// 16–240 nominal, samples outside kept), chroma at half resolution in both directions
    /// (rounded up), sited centred. Converted as libwebp does ([`to_rgba`]) or by the
    /// viewer's limited-range YUV path; expanding them to full range in 8 bits clipped
    /// saturated colours (up to 25/255 off libwebp; fidelity round 6).
    YCbCr420 { y: Plane, cb: Plane, cr: Plane },
    /// Interleaved RGBA8, straight alpha.
    Rgba(Plane),
}

/// A decoded image (or frame) at some scale.
#[derive(Clone, Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// Downscale factor relative to the full image.
    pub scale: u32,
    pub pixels: Pixels,
}

impl Decoded {
    pub fn memory_bytes(&self) -> usize {
        match &self.pixels {
            Pixels::YCbCr420 { y, cb, cr } => y.data.len() + cb.data.len() + cr.data.len(),
            Pixels::Rgba(p) => p.data.len(),
        }
    }
}

/// How the image is coded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coding {
    /// VP8 (lossy), optionally with an ALPH chunk.
    Lossy,
    /// VP8L.
    Lossless,
}

/// Header facts.
#[derive(Clone, Debug)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    /// Coding of the (first) frame.
    pub coding: Coding,
    pub animated: bool,
    /// Number of frames (1 for stills).
    pub frame_count: u32,
    /// Animation loop count, 0 = forever.
    pub loop_count: u32,
    /// Sum of the frame durations in ms.
    pub loop_duration_ms: u64,
    pub icc_profile: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
}

/// An opened WebP file. `D` is the byte container (a `Vec<u8>`, a memory map...).
pub struct WebpImage<D: AsRef<[u8]>> {
    data: D,
    info: Info,
    vp8: Option<Range<u64>>,
    vp8l: Option<Range<u64>>,
    alph: Option<Range<u64>>,
}

/// Errors are the forked decoder's.
pub type Error = DecodingError;

/// Upper bound on the canvas (VP8X allows 2^24 × 2^24) so that absurd headers fail cleanly
/// before any allocation: 1 Gpx.
const MAX_PIXELS: u64 = 1 << 30;

/// Bytes per canvas pixel a still decode holds at its peak (the RGBA output, or the YUV frame
/// and its planes), reserved from the memory budget before decoding.
const STILL_BYTES: u64 = 4;
/// Bytes per canvas pixel an animation holds while it plays (the RGBA canvas and a frame).
const ANIMATION_BYTES: u64 = 8;

fn slice<'a>(data: &'a [u8], r: &Range<u64>) -> Result<&'a [u8], Error> {
    let s = usize::try_from(r.start).map_err(|_| Error::InvalidChunkSize)?;
    let e = usize::try_from(r.end).map_err(|_| Error::InvalidChunkSize)?;
    data.get(s..e.min(data.len()))
        .filter(|_| s <= e)
        .ok_or(Error::InvalidChunkSize)
}

impl<D: AsRef<[u8]>> WebpImage<D> {
    /// Parse the container (no pixel decoding).
    pub fn parse(data: D) -> Result<Self, Error> {
        let mut dec = WebPDecoder::new(Cursor::new(data.as_ref()))?;
        let (width, height) = dec.dimensions();
        if width as u64 * height as u64 > MAX_PIXELS {
            return Err(Error::ImageTooLarge);
        }
        let animated = dec.is_animated();
        let vp8 = dec.chunk_range(WebPRiffChunk::VP8);
        let vp8l = dec.chunk_range(WebPRiffChunk::VP8L);
        let alph = dec.chunk_range(WebPRiffChunk::ALPH);
        let coding = if vp8.is_some() && (vp8l.is_none() || !animated) {
            Coding::Lossy
        } else {
            Coding::Lossless
        };
        let info = Info {
            width,
            height,
            has_alpha: dec.has_alpha(),
            coding,
            animated,
            frame_count: if animated { dec.num_frames().max(1) } else { 1 },
            loop_count: match dec.loop_count() {
                LoopCount::Forever => 0,
                LoopCount::Times(n) => u32::from(n.get()),
            },
            loop_duration_ms: dec.loop_duration(),
            icc_profile: dec.icc_profile().ok().flatten(),
            exif: dec.exif_metadata().ok().flatten(),
            xmp: dec.xmp_metadata().ok().flatten(),
        };
        Ok(WebpImage {
            data,
            info,
            vp8,
            vp8l,
            alph,
        })
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    /// Estimated peak memory of [`WebpImage::decode`] at `scale`.
    pub fn decode_memory(&self, scale: u32) -> u64 {
        let px = self.info.width as u64 * self.info.height as u64;
        let s = scale.max(1) as u64;
        let out = |bpp: u64| bpp * px / (s * s);
        if self.info.animated {
            // Canvas, frame, output.
            return 4 * px * 2 + out(4);
        }
        match (self.info.coding, self.info.has_alpha) {
            // Padded YUV planes plus output.
            (Coding::Lossy, false) => px * 3 / 2 + out(3) / 2 + px / 8,
            (Coding::Lossy, true) => px * 3 / 2 + px * 4 + 2 * px + out(4),
            // Transformed ARGB, the output, and a copy when scaling.
            (Coding::Lossless, _) => 4 * px * 2 + out(4),
        }
    }

    /// Decode the image (the first frame of an animation) at `scale` (1, 2, 4, 8...).
    ///
    /// Opaque lossy images come out as planar [`Pixels::YCbCr420`]; everything else as RGBA.
    /// `parallel` allows several threads (the rayon pool) for the stages that split.
    pub fn decode(&self, scale: u32, parallel: bool) -> Result<Decoded, Error> {
        let scale = scale.max(1);
        let data = self.data.as_ref();
        if self.info.animated {
            let mut a = Animation::new(data)?;
            let f = a.next_frame().ok_or(Error::NoMoreFrames)??;
            return Ok(downscale_rgba(f.image, scale, parallel));
        }
        let _res = specular_core::budget::reserve_n(
            u64::from(self.info.width) * u64::from(self.info.height),
            STILL_BYTES,
        )?;
        match self.info.coding {
            Coding::Lossy => {
                let r = self.vp8.as_ref().ok_or(Error::ChunkMissing)?;
                if self.alph.is_none() && !self.info.has_alpha && scale == 1 {
                    // Straight into the output planes as rows finish.
                    let (frame, sink) =
                        Vp8Decoder::decode_pipelined(slice(data, r)?, parallel, |f| {
                            Ok(PlanarSink::new(f))
                        })?;
                    if u32::from(frame.width) != self.info.width
                        || u32::from(frame.height) != self.info.height
                    {
                        return Err(Error::InconsistentImageSizes);
                    }
                    return Ok(sink.finish());
                }
                let frame = Vp8Decoder::decode_frame_fast(slice(data, r)?, parallel)?;
                if u32::from(frame.width) != self.info.width
                    || u32::from(frame.height) != self.info.height
                {
                    return Err(Error::InconsistentImageSizes);
                }
                match &self.alph {
                    None if !self.info.has_alpha => Ok(yuv_planes(&frame, scale, parallel)),
                    alph => {
                        let mut rgba = frame_to_rgba(&frame, parallel);
                        if let Some(r) = alph {
                            apply_alpha(&mut rgba, slice(data, r)?)?;
                        }
                        Ok(downscale_rgba(rgba, scale, parallel))
                    }
                }
            }
            Coding::Lossless => {
                let r = self.vp8l.as_ref().ok_or(Error::ChunkMissing)?;
                let (w, h) = (self.info.width, self.info.height);
                let mut buf = vec![0u8; w as usize * h as usize * 4];
                let mut dec = LosslessDecoder::new(slice(data, r)?);
                dec.decode_frame(w, h, false, &mut buf)?;
                let img = rgba_image(w, h, buf);
                Ok(downscale_rgba(img, scale, parallel))
            }
        }
    }
}

fn rgba_image(w: u32, h: u32, data: Vec<u8>) -> Decoded {
    Decoded {
        width: w,
        height: h,
        scale: 1,
        pixels: Pixels::Rgba(Plane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
        }),
    }
}

/// VP8 frame → RGBA (alpha 255) with libwebp's "fancy" chroma upsampling, in row bands.
fn frame_to_rgba(frame: &crate::vp8::Frame, parallel: bool) -> Decoded {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut out = vec![255u8; w * h * 4];
    let bw = frame.buffer_width() as usize;
    // Bands of an even number of rows; each band converts independently given the chroma
    // rows around it.
    let band = if parallel { 64 } else { h.max(2) };
    let f = |(i, chunk): (usize, &mut [u8])| {
        let y0 = i * band;
        yuv::fill_rgba_band_fancy(chunk, frame, y0, chunk.len() / (w * 4), w, h, bw);
    };
    if parallel {
        out.par_chunks_mut(band * w * 4).enumerate().for_each(f);
    } else {
        out.chunks_mut(band * w * 4).enumerate().for_each(f);
    }
    rgba_image(w as u32, h as u32, out)
}

/// Decode the ALPH chunk into the alpha channel of `img` (RGBA).
fn apply_alpha(img: &mut Decoded, chunk: &[u8]) -> Result<(), Error> {
    let Pixels::Rgba(p) = &mut img.pixels else {
        return Ok(());
    };
    let (w, h) = (p.width as u16, p.height as u16);
    let mut reader = chunk;
    let alpha = read_alpha_chunk(&mut reader, w, h)?;
    let w = usize::from(w);
    if w == 0 || h == 0 {
        return Ok(());
    }
    let plane = unfilter_alpha(alpha.data, w, alpha.filtering_method);
    for (px, &a) in p.data.chunks_exact_mut(4).zip(&plane) {
        px[3] = a;
    }
    Ok(())
}

/// Undo the ALPH spatial filter of `data` (rows of `w` residuals), row by row: the predictors
/// of [`crate::extended::get_alpha_predictor`] with the per-pixel case analysis hoisted out.
pub(crate) fn unfilter_alpha(mut out: Vec<u8>, w: usize, method: FilteringMethod) -> Vec<u8> {
    let len = out.len();
    if matches!(method, FilteringMethod::None) {
        return out;
    }
    // Row 0 predicts from the left in every mode (0 for the first pixel).
    let (first, rest) = out.split_at_mut(w.min(len));
    let mut left = 0u8;
    for a in first.iter_mut() {
        *a = a.wrapping_add(left);
        left = *a;
    }
    let mut prev: &[u8] = first;
    for row in rest.chunks_exact_mut(w) {
        match method {
            FilteringMethod::None => {}
            FilteringMethod::Horizontal => {
                let mut left = prev[0];
                for a in row.iter_mut() {
                    *a = a.wrapping_add(left);
                    left = *a;
                }
            }
            FilteringMethod::Vertical => {
                for (a, &t) in row.iter_mut().zip(prev) {
                    *a = a.wrapping_add(t);
                }
            }
            FilteringMethod::Gradient => {
                row[0] = row[0].wrapping_add(prev[0]);
                let mut left = row[0];
                for (a, t) in row[1..].iter_mut().zip(prev.windows(2)) {
                    let g = i16::from(left) + i16::from(t[1]) - i16::from(t[0]);
                    left = a.wrapping_add(g.clamp(0, 255) as u8);
                    *a = left;
                }
            }
        }
        prev = row;
    }
    out
}

/// Pipeline sink for opaque images at scale 1: copies finished rows straight into the cropped
/// output planes (no intermediate full frame).
struct PlanarSink {
    w: usize,
    h: usize,
    cw: usize,
    ch: usize,
    luma_stride: usize,
    chroma_stride: usize,
    y: Vec<u8>,
    cb: Vec<u8>,
    cr: Vec<u8>,
}

impl PlanarSink {
    fn new(f: &crate::vp8::Frame) -> PlanarSink {
        let (w, h) = (usize::from(f.width), usize::from(f.height));
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        let bw = usize::from(f.buffer_width());
        PlanarSink {
            w,
            h,
            cw,
            ch,
            luma_stride: bw,
            chroma_stride: bw / 2,
            y: vec![0; w * h],
            cb: vec![0; cw * ch],
            cr: vec![0; cw * ch],
        }
    }

    fn finish(self) -> Decoded {
        let plane = |data, width: usize, height: usize| Plane {
            width: width as u32,
            height: height as u32,
            stride: width,
            data,
        };
        Decoded {
            width: self.w as u32,
            height: self.h as u32,
            scale: 1,
            pixels: Pixels::YCbCr420 {
                y: plane(self.y, self.w, self.h),
                cb: plane(self.cb, self.cw, self.ch),
                cr: plane(self.cr, self.cw, self.ch),
            },
        }
    }
}

impl Sink for PlanarSink {
    fn rows(&mut self, y0: usize, y: &[u8], u: &[u8], v: &[u8]) {
        if self.w == 0 {
            return;
        }
        let out = self.y.chunks_exact_mut(self.w).skip(y0);
        for (src, dst) in y.chunks_exact(self.luma_stride).zip(out) {
            dst.copy_from_slice(&src[..self.w]);
        }
        for (plane, out) in [(u, &mut self.cb), (v, &mut self.cr)] {
            let out = out.chunks_exact_mut(self.cw).skip(y0 / 2);
            for (src, dst) in plane.chunks_exact(self.chroma_stride).zip(out) {
                dst.copy_from_slice(&src[..self.cw]);
            }
        }
    }
}

/// Box-downscale (or copy, at factor 1) one plane, in parallel row bands.
fn plane_scaled(
    src: &[u8],
    src_stride: usize,
    sw: usize,
    sh: usize,
    f: usize,
    parallel: bool,
) -> Plane {
    let w = sw.div_ceil(f);
    let h = sh.div_ceil(f);
    let mut out = vec![0u8; w * h];
    let row = |(oy, o): (usize, &mut [u8])| {
        if f == 1 {
            o.copy_from_slice(&src[oy * src_stride..][..w]);
            return;
        }
        let y1 = ((oy + 1) * f).min(sh);
        let mut acc = vec![0u32; w];
        for y in oy * f..y1 {
            let r = &src[y * src_stride..][..sw];
            for (ox, a) in acc.iter_mut().enumerate() {
                let x0 = ox * f;
                let x1 = (x0 + f).min(sw);
                *a += r[x0..x1].iter().map(|&v| v as u32).sum::<u32>();
            }
        }
        let rows = (y1 - oy * f) as u32;
        for (ox, (d, a)) in o.iter_mut().zip(acc).enumerate() {
            let cols = ((ox * f + f).min(sw) - ox * f) as u32;
            let n = rows * cols;
            *d = ((a + n / 2) / n) as u8;
        }
    };
    if parallel && h > 16 {
        out.par_chunks_mut(w).enumerate().for_each(row);
    } else {
        out.chunks_mut(w).enumerate().for_each(row);
    }
    Plane {
        width: w as u32,
        height: h as u32,
        stride: w,
        data: out,
    }
}

/// Planar output: the VP8 planes (box-downscaled). At scale `s` luma is averaged over s×s and
/// chroma over (s/2)×(s/2), keeping 4:2:0.
fn yuv_planes(frame: &crate::vp8::Frame, scale: u32, parallel: bool) -> Decoded {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let bw = frame.buffer_width() as usize;
    let s = scale as usize;
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let cf = (s / 2).max(1);
    let y = plane_scaled(&frame.ybuf, bw, w, h, s, parallel);
    let (cb, cr) = rayon_join(
        parallel,
        || plane_scaled(&frame.ubuf, bw / 2, cw, ch, cf, parallel),
        || plane_scaled(&frame.vbuf, bw / 2, cw, ch, cf, parallel),
    );
    // At scale 1 the chroma planes are ceil(w/2); when scaled, make them ceil(Yw/2) exactly.
    let fix = |p: Plane, yw: u32, yh: u32| -> Plane {
        let (tw, th) = (yw.div_ceil(2), yh.div_ceil(2));
        if p.width == tw && p.height == th {
            return p;
        }
        let mut data = vec![0u8; (tw * th) as usize];
        for yy in 0..th as usize {
            let sy = yy.min(p.height as usize - 1);
            for xx in 0..tw as usize {
                let sx = xx.min(p.width as usize - 1);
                data[yy * tw as usize + xx] = p.data[sy * p.stride + sx];
            }
        }
        Plane {
            width: tw,
            height: th,
            stride: tw as usize,
            data,
        }
    };
    let cb = fix(cb, y.width, y.height);
    let cr = fix(cr, y.width, y.height);
    Decoded {
        width: y.width,
        height: y.height,
        scale,
        pixels: Pixels::YCbCr420 { y, cb, cr },
    }
}

fn rayon_join<A: Send, B: Send>(
    parallel: bool,
    a: impl FnOnce() -> A + Send,
    b: impl FnOnce() -> B + Send,
) -> (A, B) {
    if parallel {
        rayon::join(a, b)
    } else {
        (a(), b())
    }
}

fn srgb_to_linear_lut() -> [f32; 256] {
    let mut t = [0f32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *v = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    t
}

fn linear_to_srgb8(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0 + 0.5) as u8
}

/// Box-filter RGBA by an integer factor, averaging in linear light with premultiplied alpha.
pub fn downscale_rgba(img: Decoded, f: u32, parallel: bool) -> Decoded {
    if f <= 1 {
        return img;
    }
    let Pixels::Rgba(p) = &img.pixels else {
        return img;
    };
    let (sw, sh) = (p.width as usize, p.height as usize);
    let f = f as usize;
    let w = sw.div_ceil(f);
    let h = sh.div_ceil(f);
    let lut = srgb_to_linear_lut();
    let mut out = vec![0u8; w * h * 4];
    let row = |(oy, o): (usize, &mut [u8])| {
        let mut acc = vec![[0f32; 4]; w];
        let y1 = ((oy + 1) * f).min(sh);
        for y in oy * f..y1 {
            let r = &p.data[y * p.stride..][..sw * 4];
            for (ox, acc) in acc.iter_mut().enumerate() {
                // A local copy, so the sums stay in registers: a PGO build kept `acc` in memory,
                // a load-add-store round trip per pixel (the lossless WebP thumbnail +22 % cycles).
                let mut a = *acc;
                for px in r[ox * f * 4..((ox * f + f).min(sw)) * 4].chunks_exact(4) {
                    let al = px[3] as f32 * (1.0 / 255.0);
                    a[0] += lut[px[0] as usize] * al;
                    a[1] += lut[px[1] as usize] * al;
                    a[2] += lut[px[2] as usize] * al;
                    a[3] += al;
                }
                *acc = a;
            }
        }
        let rows = y1 - oy * f;
        for (ox, (d, a)) in o.chunks_exact_mut(4).zip(acc).enumerate() {
            let n = (rows * ((ox * f + f).min(sw) - ox * f)) as f32;
            for c in 0..3 {
                d[c] = linear_to_srgb8(if a[3] > 0.0 { a[c] / a[3] } else { 0.0 });
            }
            d[3] = (a[3] / n * 255.0 + 0.5) as u8;
        }
    };
    if parallel && h > 16 {
        out.par_chunks_mut(w * 4).enumerate().for_each(row);
    } else {
        out.chunks_mut(w * 4).enumerate().for_each(row);
    }
    Decoded {
        width: w as u32,
        height: h as u32,
        scale: img.scale * f as u32,
        pixels: Pixels::Rgba(Plane {
            width: w as u32,
            height: h as u32,
            stride: w * 4,
            data: out,
        }),
    }
}

/// One composited animation frame.
pub struct AnimFrame {
    /// The whole canvas, RGBA.
    pub image: Decoded,
    pub duration_ms: u32,
}

/// Sequential access to the frames of an animated WebP (a still image yields one frame).
/// `D` is the byte container, as for [`WebpImage`].
pub struct Animation<D: AsRef<[u8]>> {
    dec: WebPDecoder<Cursor<D>>,
    index: u32,
    /// The canvas in the memory budget while the animation lives.
    _res: specular_core::Reservation,
}

impl<D: AsRef<[u8]>> Animation<D> {
    pub fn new(data: D) -> Result<Self, Error> {
        let dec = WebPDecoder::new(Cursor::new(data))?;
        let (w, h) = dec.dimensions();
        if w as u64 * h as u64 > MAX_PIXELS {
            return Err(Error::ImageTooLarge);
        }
        let res = if dec.is_animated() {
            specular_core::budget::reserve_n(w as u64 * h as u64, ANIMATION_BYTES)?
        } else {
            // A still image reserves for itself as it decodes.
            specular_core::Reservation::none()
        };
        Ok(Animation {
            dec,
            index: 0,
            _res: res,
        })
    }

    /// Index of the next frame.
    pub fn position(&self) -> u32 {
        self.index
    }

    /// Restart from the first frame.
    pub fn rewind(&mut self) {
        if self.dec.is_animated() {
            self.dec.reset_animation();
        }
        self.index = 0;
    }

    /// Decode the next frame, or `None` after the last.
    pub fn next_frame(&mut self) -> Option<Result<AnimFrame, Error>> {
        if !self.dec.is_animated() {
            if self.index > 0 {
                return None;
            }
            self.index = 1;
            let data = self.dec.get_ref().get_ref().as_ref();
            return Some(
                WebpImage::parse(data)
                    .and_then(|s| s.decode(1, true))
                    .map(|d| AnimFrame {
                        image: to_rgba(d),
                        duration_ms: 0,
                    }),
            );
        }
        let (w, h) = self.dec.dimensions();
        let alpha = self.dec.has_alpha();
        let mut buf = vec![0u8; w as usize * h as usize * if alpha { 4 } else { 3 }];
        match self.dec.read_frame(&mut buf) {
            Ok(duration_ms) => {
                self.index += 1;
                let rgba = if alpha {
                    buf
                } else {
                    let mut o = vec![255u8; w as usize * h as usize * 4];
                    for (d, s) in o.chunks_exact_mut(4).zip(buf.chunks_exact(3)) {
                        d[..3].copy_from_slice(s);
                    }
                    o
                };
                Some(Ok(AnimFrame {
                    image: rgba_image(w, h, rgba),
                    duration_ms,
                }))
            }
            Err(Error::NoMoreFrames) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// Convert any decoded image to RGBA: planes as libwebp converts them (studio-range BT.601,
/// "fancy" bilinear chroma), so the values equal libwebp's RGB output.
pub fn to_rgba(d: Decoded) -> Decoded {
    let Pixels::YCbCr420 { y, cb, cr } = &d.pixels else {
        return d;
    };
    let (w, h) = (d.width as usize, d.height as usize);
    // Opaque: the converter writes colour only at the row edges.
    let mut out = vec![255u8; w * h * 4];
    if w > 0 && h > 0 {
        let planes = yuv::PlanesRef {
            y: &y.data,
            u: &cb.data,
            v: &cr.data,
            y_stride: y.stride,
            c_stride: cb.stride.min(cr.stride),
        };
        yuv::fill_rgba_band_fancy_planes(&mut out, &planes, 0, h, w, h);
    }
    Decoded {
        pixels: Pixels::Rgba(Plane {
            width: d.width,
            height: d.height,
            stride: w * 4,
            data: out,
        }),
        ..d
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    /// A lossless WebP whose header claims 16000 × 16000 (1 GB of RGBA) and holds nothing
    /// else.
    fn huge_vp8l() -> Vec<u8> {
        let mut vp8l = vec![0x2f];
        // 14 bits width − 1, 14 bits height − 1, alpha bit, 3 bits version.
        let bits: u32 = 15999 | (15999 << 14);
        vp8l.extend(bits.to_le_bytes());
        vp8l.extend([0; 3]);
        let mut f = b"RIFF".to_vec();
        f.extend((4 + 8 + vp8l.len() as u32).to_le_bytes());
        f.extend(b"WEBPVP8L");
        f.extend((vp8l.len() as u32).to_le_bytes());
        f.extend(&vp8l);
        f
    }

    #[test]
    fn canvas_within_memory_budget() {
        let f = huge_vp8l();
        let img = WebpImage::parse(&f[..]).unwrap();
        assert_eq!((img.info.width, img.info.height), (16000, 16000));
        let e = specular_core::budget::with_limit(64 << 20, || img.decode(1, false)).unwrap_err();
        let Error::NeedsMemory(n) = e else {
            panic!("{e:?}")
        };
        assert!(n.needed > n.free && n.needed >= 1_024_000_000, "{n:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_wise_alpha_unfilter_matches_per_pixel_predictor() {
        use crate::extended::get_alpha_predictor;
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for (w, h) in [(1, 1), (1, 5), (5, 1), (7, 3), (33, 17)] {
            for method in [
                FilteringMethod::None,
                FilteringMethod::Horizontal,
                FilteringMethod::Vertical,
                FilteringMethod::Gradient,
            ] {
                let data: Vec<u8> = (0..w * h)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        seed as u8
                    })
                    .collect();
                let mut want = vec![0u8; w * h * 4];
                for y in 0..h {
                    for x in 0..w {
                        let pred = get_alpha_predictor(x, y, w, method, &want);
                        want[(y * w + x) * 4 + 3] = pred.wrapping_add(data[y * w + x]);
                    }
                }
                let want: Vec<u8> = want.chunks_exact(4).map(|p| p[3]).collect();
                assert_eq!(unfilter_alpha(data, w, method), want, "{w}x{h} {method:?}");
            }
        }
    }
}
