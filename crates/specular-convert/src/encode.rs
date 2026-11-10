//! Output encoders with metadata, the fastest we measured for each format and mode (see
//! ): JPEG and PNG by the tree's own encoders and
//! TIFF by the writer here (all three parallel on the current rayon pool), lossless WebP by
//! `image-webp`, lossy WebP by libwebp (the `webp` crate) with the extended-format chunks
//! written here, JPEG XL by libjxl (`gamut-jxl`) and AVIF by rav1e (`ravif`) with the
//! profile and XMP added by [`crate::avif_mux`].

use crate::exif::{Entry, Exif};
use crate::pixels::{Pixels, Samples};
use crate::{Format, Options};
use rayon::prelude::*;

/// What goes into the file besides pixels.
#[derive(Clone, Copy, Default)]
pub struct Meta<'a> {
    pub exif: Option<&'a Exif>,
    pub xmp: Option<&'a [u8]>,
    pub icc: Option<&'a [u8]>,
    /// `cICP` code points (HDR kept as PQ in PNG).
    pub cicp: Option<[u8; 4]>,
}

/// Largest payload of a JPEG marker segment.
const SEGMENT_MAX: usize = 65_533;

pub fn encode(opts: &Options, px: &Pixels, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    match opts.format {
        Format::Jpeg => jpeg(opts, px, meta),
        Format::Png => png(opts, px, meta),
        Format::Tiff => Ok(tiff(px, meta, opts.tiff_deflate)),
        Format::Webp if opts.webp_quality.is_none() => webp(px, meta),
        Format::Webp => webp_lossy(opts, px, meta),
        Format::Jxl => jxl(opts, px, meta),
        Format::Avif => avif(opts, px, meta),
    }
}

/// Interleaved 8-bit RGB or RGBA for encoders that take nothing else (grey expanded).
fn rgb_or_rgba(px: &Pixels) -> Result<std::borrow::Cow<'_, [u8]>, String> {
    let data = eight(px)?;
    Ok(match px.channels {
        1 => data.iter().flat_map(|&v| [v, v, v]).collect(),
        2 => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        _ => data.into(),
    })
}

fn eight(px: &Pixels) -> Result<&[u8], String> {
    match &px.samples {
        Samples::Eight(v) => Ok(v),
        Samples::Sixteen(_) => Err("16-bit samples for an 8-bit format".into()),
    }
}

fn jpeg(opts: &Options, px: &Pixels, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    use specular_jpeg::encode as je;
    let data = eight(px)?;
    let samples = match px.channels {
        1 => je::Samples::Gray(data),
        3 => je::Samples::Rgb(data),
        c => return Err(format!("{c} channels for JPEG")),
    };
    // Exif must fit one APP1 segment: drop the maker note first, then give up on Exif.
    let exif = meta.exif.and_then(|e| {
        let t = e.to_tiff();
        if t.len() + 6 <= SEGMENT_MAX {
            return Some(t);
        }
        let t = e.without_maker_note().to_tiff();
        (t.len() + 6 <= SEGMENT_MAX).then_some(t)
    });
    let xmp = meta.xmp.filter(|x| x.len() + 29 <= SEGMENT_MAX);
    let o = je::EncodeOptions {
        quality: opts.jpeg_quality.clamp(1, 100),
        subsampling: if opts.jpeg_full_chroma {
            je::Subsampling::S444
        } else {
            je::Subsampling::S420
        },
        exif: exif.as_deref(),
        xmp,
        icc: meta.icc,
    };
    je::encode(px.width, px.height, samples, &o).map_err(|e| e.to_string())
}

fn png(opts: &Options, px: &Pixels, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    use specular_png::ColorType;
    let color = match px.channels {
        1 => ColorType::Gray,
        2 => ColorType::GrayAlpha,
        3 => ColorType::Rgb,
        _ => ColorType::Rgba,
    };
    let be;
    let (data, sixteen): (&[u8], bool) = match &px.samples {
        Samples::Eight(v) => (v, false),
        Samples::Sixteen(v) => {
            be = big_endian(v);
            (&be, true)
        }
    };
    let exif = meta.exif.map(Exif::to_tiff);
    let o = specular_png::EncodeOptions {
        sixteen_bit: sixteen,
        // specular-png's own single-block encoder (level None) is several times slower
        // than zlib level 1 on photographs, and makes larger files.
        level: Some(opts.png_effort.clamp(1, 9)),
        icc: meta.icc,
        srgb: meta.icc.is_none(),
        cicp: meta.cicp,
        exif: exif.as_deref(),
        xmp: meta.xmp,
        text: &[],
    };
    specular_png::encode_with(px.width, px.height, color, data, &o).map_err(|e| e.to_string())
}

fn webp(px: &Pixels, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    use image_webp::{ColorType, WebPEncoder};
    let data = eight(px)?;
    let color = match px.channels {
        1 => ColorType::L8,
        2 => ColorType::La8,
        3 => ColorType::Rgb8,
        _ => ColorType::Rgba8,
    };
    let mut out = Vec::new();
    let mut e = WebPEncoder::new(&mut out);
    if let Some(icc) = meta.icc {
        e.set_icc_profile(icc.to_vec());
    }
    if let Some(x) = meta.exif {
        e.set_exif_metadata(x.to_tiff());
    }
    if let Some(x) = meta.xmp {
        e.set_xmp_metadata(x.to_vec());
    }
    e.encode(data, px.width, px.height, color)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// The largest WebP canvas side.
const WEBP_MAX: u32 = 16_383;

/// Lossy WebP: libwebp makes the `VP8 ` (and `ALPH`) chunks, and the extended-format file
/// around them is written here when there is a profile or metadata to carry.
fn webp_lossy(opts: &Options, px: &Pixels, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    if px.width > WEBP_MAX || px.height > WEBP_MAX {
        return Err(format!("WebP holds at most {WEBP_MAX} × {WEBP_MAX} pixels"));
    }
    let data = rgb_or_rgba(px)?;
    let alpha = px.channels.is_multiple_of(2);
    let mut c = webp::WebPConfig::new().map_err(|()| "libwebp: no configuration".to_owned())?;
    c.lossless = 0;
    c.quality = f32::from(opts.webp_quality.unwrap_or(90).clamp(1, 100));
    // Effort 1–9 onto libwebp's methods 0–6.
    c.method = i32::from(webp_method(opts.webp_effort));
    c.alpha_compression = 1;
    c.thread_level = 1;
    let layout = if alpha {
        webp::PixelLayout::Rgba
    } else {
        webp::PixelLayout::Rgb
    };
    let mem = webp::Encoder::new(&data, layout, px.width, px.height)
        .encode_advanced(&c)
        .map_err(|e| format!("libwebp: {e:?}"))?;
    webp_mux(&mem, px.width, px.height, meta)
}

/// libwebp's method for effort 1–9.
pub fn webp_method(effort: u8) -> u8 {
    ((effort.clamp(1, 9) - 1) * 6 + 4) / 8
}

/// `simple` (a RIFF WebP from libwebp) with an `ICCP`, `EXIF` and `XMP ` chunk from `meta`
/// in the extended format; unchanged when there are none.
fn webp_mux(simple: &[u8], w: u32, h: u32, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    let exif = meta.exif.map(Exif::to_tiff);
    if meta.icc.is_none() && exif.is_none() && meta.xmp.is_none() {
        return Ok(simple.to_vec());
    }
    if simple.len() < 12 || &simple[..4] != b"RIFF" || &simple[8..12] != b"WEBP" {
        return Err("libwebp: not a WebP file".into());
    }
    // The image chunks, minus libwebp's own VP8X if it wrote one (for alpha).
    let mut image = Vec::with_capacity(simple.len());
    let mut has_alpha = false;
    let mut at = 12;
    while at + 8 <= simple.len() {
        let fourcc = &simple[at..at + 4];
        let len =
            u32::from_le_bytes(simple[at + 4..at + 8].try_into().unwrap_or_default()) as usize;
        let end = (at + 8 + len + (len & 1)).min(simple.len());
        match fourcc {
            b"VP8X" => {}
            b"ALPH" => {
                has_alpha = true;
                image.extend_from_slice(&simple[at..end]);
            }
            _ => image.extend_from_slice(&simple[at..end]),
        }
        at = end;
    }
    let mut flags = 0u8;
    if meta.icc.is_some() {
        flags |= 0x20;
    }
    if has_alpha {
        flags |= 0x10;
    }
    if exif.is_some() {
        flags |= 0x08;
    }
    if meta.xmp.is_some() {
        flags |= 0x04;
    }
    let mut vp8x = [0u8; 10];
    vp8x[0] = flags;
    vp8x[4..7].copy_from_slice(&(w - 1).to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&(h - 1).to_le_bytes()[..3]);
    let mut out = Vec::with_capacity(image.len() + 4096);
    out.extend_from_slice(b"RIFF\0\0\0\0WEBP");
    riff_chunk(&mut out, b"VP8X", &vp8x);
    if let Some(icc) = meta.icc {
        riff_chunk(&mut out, b"ICCP", icc);
    }
    out.extend_from_slice(&image);
    if let Some(x) = &exif {
        riff_chunk(&mut out, b"EXIF", x);
    }
    if let Some(x) = meta.xmp {
        riff_chunk(&mut out, b"XMP ", x);
    }
    let size = u32::try_from(out.len() - 8).map_err(|_| "WebP over 4 GiB".to_owned())?;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(out)
}

fn riff_chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

/// The Butteraugli distance for a JPEG XL quality 1–100, as `cjxl --quality` maps it
/// (90 → 1.0, "visually lossless").
pub fn jxl_distance(quality: u8) -> f32 {
    let q = f32::from(quality.clamp(1, 100));
    if q >= 30.0 {
        0.1 + (100.0 - q) * 0.09
    } else {
        6.4 + 2.5_f32.powf((30.0 - q) / 5.0) / 6.25
    }
}

/// JPEG XL by libjxl, lossless or lossy, with the profile in the codestream and Exif and
/// XMP in container boxes.
fn jxl(opts: &Options, px: &Pixels, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    jxl_framed(opts, px, meta, false)
}

/// [`jxl`], always in the ISOBMFF container when `container` (a gain-map base).
fn jxl_framed(
    opts: &Options,
    px: &Pixels,
    meta: &Meta<'_>,
    container: bool,
) -> Result<Vec<u8>, String> {
    use gamut_core::{Dimensions, EncodeImage, ImageRef};
    use gamut_jxl::{ColorSpec, Container, Distance, Effort, JxlEncoder};
    let mut e = match opts.jxl_quality {
        None => JxlEncoder::lossless(),
        Some(q) => JxlEncoder::lossy(Distance::new(jxl_distance(q)).map_err(|e| e.to_string())?),
    };
    e = e.with_effort(Effort::from_level(opts.jxl_effort.clamp(1, 9)).unwrap_or_default());
    if let Some(c) = meta.cicp {
        e = e.with_color(if c[1] == 18 {
            ColorSpec::Hlg
        } else {
            ColorSpec::Pq
        });
    } else if let Some(icc) = meta.icc {
        e = e.with_color(ColorSpec::Icc(icc.to_vec()));
    }
    let xmp = meta.xmp.and_then(|x| std::str::from_utf8(x).ok());
    if container || meta.exif.is_some() || xmp.is_some() {
        e = e.with_container(Container::IsoBmff);
    }
    if let Some(x) = meta.exif {
        e = e.with_exif(&x.to_tiff());
    }
    if let Some(x) = xmp {
        e = e.with_xmp(x);
    }
    let dims = Dimensions::new(px.width, px.height).map_err(|e| e.to_string())?;
    let err = |e: gamut_core::Error| format!("libjxl: {e}");
    macro_rules! go {
        ($t:ty, $v:expr) => {
            e.encode_to_vec(ImageRef::<$t>::new($v, dims).map_err(err)?)
                .map_err(err)
        };
    }
    use gamut_core::{Gray8, Gray16, GrayAlpha8, GrayAlpha16, Rgb8, Rgb16, Rgba8, Rgba16};
    match (&px.samples, px.channels) {
        (Samples::Eight(v), 1) => go!(Gray8, v),
        (Samples::Eight(v), 2) => go!(GrayAlpha8, v),
        (Samples::Eight(v), 3) => go!(Rgb8, v),
        (Samples::Eight(v), _) => go!(Rgba8, v),
        (Samples::Sixteen(v), 1) => go!(Gray16, v),
        (Samples::Sixteen(v), 2) => go!(GrayAlpha16, v),
        (Samples::Sixteen(v), 3) => go!(Rgb16, v),
        (Samples::Sixteen(v), _) => go!(Rgba16, v),
    }
}

/// rav1e's speed for effort 1–9 (10 is the fastest preset).
pub fn avif_speed(effort: u8) -> u8 {
    10 - effort.clamp(1, 9) / 2
}

/// AVIF by rav1e (`ravif`): 8-bit 4:4:4 YCbCr, tiles in parallel on the current rayon pool,
/// Exif in the container; the profile and XMP, which ravif can't write, are added by
/// [`crate::avif_mux`].
fn avif(opts: &Options, px: &Pixels, meta: &Meta<'_>) -> Result<Vec<u8>, String> {
    if let (Some(c), Samples::Sixteen(_)) = (meta.cicp, &px.samples) {
        return avif_hdr(opts, px, meta, c);
    }
    let data = rgb_or_rgba(px)?;
    let (w, h) = (px.width as usize, px.height as usize);
    let mut e = ravif::Encoder::new()
        .with_quality(f32::from(opts.avif_quality.clamp(1, 100)))
        .with_alpha_quality(f32::from(opts.avif_quality.clamp(1, 100)))
        .with_speed(avif_speed(opts.avif_effort))
        .with_num_threads(Some(rayon::current_num_threads()));
    if let Some(x) = meta.exif {
        e = e.with_exif(x.to_tiff());
    }
    use rgb::FromSlice;
    let r = if px.channels.is_multiple_of(2) {
        e.encode_rgba(imgref::Img::new(data.as_rgba(), w, h))
    } else {
        e.encode_rgb(imgref::Img::new(data.as_rgb(), w, h))
    };
    let file = r.map(|r| r.avif_file).map_err(|e| format!("rav1e: {e}"))?;
    crate::avif_mux::add_icc_xmp(&file, meta.icc, meta.xmp)
}

/// HDR AVIF: 16-bit RGB holding a PQ or HLG signal (`cicp`: primaries, transfer, matrix,
/// range) as 10-bit full-range BT.2020 YCbCr 4:4:4; the container's `nclx` says the
/// primaries and transfer. Exif in the container, XMP added; no ICC profile (the `nclx`
/// describes the signal).
fn avif_hdr(
    opts: &Options,
    px: &Pixels,
    meta: &Meta<'_>,
    cicp: [u8; 4],
) -> Result<Vec<u8>, String> {
    let Samples::Sixteen(v) = &px.samples else {
        return Err("HDR AVIF needs 16-bit samples".into());
    };
    if px.channels != 3 {
        return Err(format!("{} channels for HDR AVIF", px.channels));
    }
    let (w, h) = (px.width as usize, px.height as usize);
    let mut e = ravif::Encoder::new()
        .with_quality(f32::from(opts.avif_quality.clamp(1, 100)))
        .with_speed(avif_speed(opts.avif_effort))
        .with_num_threads(Some(rayon::current_num_threads()));
    if let Some(x) = meta.exif {
        e = e.with_exif(x.to_tiff());
    }
    // BT.2020 non-constant luminance, full range.
    let (kr, kb) = (0.2627f32, 0.0593f32);
    let planes = v.chunks_exact(3).map(move |p| {
        let [r, g, b] = [p[0], p[1], p[2]].map(|s| f32::from(s) / 65535.0);
        let y = kr * r + (1.0 - kr - kb) * g + kb * b;
        let cb = (b - y) / (2.0 * (1.0 - kb)) + 0.5;
        let cr = (r - y) / (2.0 * (1.0 - kr)) + 0.5;
        [y, cb, cr].map(|c| (c * 1023.0 + 0.5).clamp(0.0, 1023.0) as u16)
    });
    let r = e
        .encode_raw_planes_10_bit(
            w,
            h,
            planes,
            None::<std::iter::Empty<u16>>,
            ravif::PixelRange::Full,
            ravif::MatrixCoefficients::BT2020NCL,
        )
        .map_err(|e| format!("rav1e: {e}"))?;
    let mut file = r.avif_file;
    crate::avif_mux::set_nclx(&mut file, u16::from(cicp[0]), u16::from(cicp[1]))?;
    // The bitstream's sequence header says the same (ravif writes BT.709 / sRGB there).
    crate::avif_mux::set_av1_colour(&mut file, cicp[0], cicp[1])?;
    crate::avif_mux::add_icc_xmp(&file, None, meta.xmp)
}

/// [`encode`] with a gain map (ISO 21496-1) for JPEG, AVIF or JPEG XL: `px` is the SDR base
/// (8-bit, encoded with its own profile in `meta`), `map` the gains towards the HDR
/// rendition. `alt_primaries` is the CICP code of the space the gains apply in (the
/// base's).
pub fn encode_with_gain_map(
    opts: &Options,
    px: &Pixels,
    meta: &Meta<'_>,
    map: &crate::gain_map::GainMapImage,
    alt_primaries: u16,
) -> Result<Vec<u8>, String> {
    let multichannel = map.channels == 3;
    let map_px = Pixels {
        width: map.width,
        height: map.height,
        channels: map.channels,
        samples: Samples::Eight(map.samples.clone()),
    };
    let none = Meta::default();
    match opts.format {
        Format::Jpeg => {
            let base = jpeg(opts, px, meta)?;
            // Gains are smooth: 4:4:4 at a high quality keeps them exact enough.
            let mo = Options {
                jpeg_quality: opts.jpeg_quality.max(90),
                jpeg_full_chroma: true,
                ..opts.clone()
            };
            let m = jpeg(&mo, &map_px, &none)?;
            crate::gain_map::jpeg(&base, &m, &map.params, multichannel)
        }
        Format::Jxl => {
            let base = jxl_framed(opts, px, meta, true)?;
            let m = jxl_framed(opts, &map_px, &none, false)?;
            crate::gain_map::jxl(&base, &m, &map.params, multichannel)
        }
        Format::Avif => {
            let base = avif(
                opts,
                px,
                &Meta {
                    icc: None,
                    xmp: None,
                    ..*meta
                },
            )?;
            let mo = Options {
                avif_quality: opts.avif_quality.max(90),
                ..opts.clone()
            };
            let m = avif(&mo, &map_px, &none)?;
            let mut body = vec![0u8];
            body.extend_from_slice(&crate::gain_map::iso21496(&map.params, multichannel, true));
            let f = crate::avif_mux::with_gain_map(&base, &m, &body, alt_primaries)?;
            crate::avif_mux::add_icc_xmp(&f, meta.icc, meta.xmp)
        }
        f => Err(format!("{f:?} cannot hold a gain map")),
    }
}

/// A little-endian baseline TIFF: chunky samples in strips of about 256 KiB, uncompressed
/// or Deflate with the horizontal predictor, the source's descriptive tags in IFD0 and its
/// Exif / GPS directories behind the usual pointers.
pub fn tiff(px: &Pixels, meta: &Meta<'_>, deflate: bool) -> Vec<u8> {
    let (bps, raw) = match &px.samples {
        Samples::Eight(v) => (8, Chunky::U8(v)),
        Samples::Sixteen(v) => (16, Chunky::U16(v)),
    };
    let layout = TiffLayout {
        width: px.width,
        height: px.height,
        channels: px.channels,
        bits: bps,
        float: false,
    };
    tiff_from(&layout, raw, meta, deflate)
}

/// As [`tiff`] for 32-bit floating-point samples (SampleFormat 3): `data` holds `channels`
/// interleaved values per pixel, rows top to bottom. Deflate uses the floating-point
/// predictor (3), which keeps float data compressible. For linear-light exports.
pub fn tiff_float(
    width: u32,
    height: u32,
    channels: usize,
    data: &[f32],
    meta: &Meta<'_>,
    deflate: bool,
) -> Vec<u8> {
    let layout = TiffLayout {
        width,
        height,
        channels,
        bits: 32,
        float: true,
    };
    tiff_from(&layout, Chunky::F32(data), meta, deflate)
}

/// 16-bit samples as big-endian bytes (PNG), converted in parallel.
fn big_endian(v: &[u16]) -> Vec<u8> {
    let mut out = vec![0u8; v.len() * 2];
    out.par_chunks_mut(1 << 17)
        .zip(v.par_chunks(1 << 16))
        .for_each(|(o, v)| {
            for (o, s) in o.chunks_exact_mut(2).zip(v) {
                o.copy_from_slice(&s.to_be_bytes());
            }
        });
    out
}

/// The samples of a TIFF, borrowed: each strip is turned into little-endian bytes by the
/// worker that writes it, never the whole picture at once.
#[derive(Clone, Copy)]
enum Chunky<'a> {
    U8(&'a [u8]),
    U16(&'a [u16]),
    F32(&'a [f32]),
}

impl Chunky<'_> {
    fn len(self) -> usize {
        match self {
            Chunky::U8(v) => v.len(),
            Chunky::U16(v) => v.len(),
            Chunky::F32(v) => v.len(),
        }
    }

    /// Samples `at` as little-endian bytes into `out` (exactly their size).
    fn write_le(self, at: std::ops::Range<usize>, out: &mut [u8]) {
        match self {
            Chunky::U8(v) => out.copy_from_slice(&v[at]),
            Chunky::U16(v) => {
                for (o, s) in out.chunks_exact_mut(2).zip(&v[at]) {
                    o.copy_from_slice(&s.to_le_bytes());
                }
            }
            Chunky::F32(v) => {
                for (o, s) in out.chunks_exact_mut(4).zip(&v[at]) {
                    o.copy_from_slice(&s.to_le_bytes());
                }
            }
        }
    }
}

struct TiffLayout {
    width: u32,
    height: u32,
    channels: usize,
    bits: u16,
    float: bool,
}

/// The file around chunky samples `raw`, written little-endian.
fn tiff_from(l: &TiffLayout, raw: Chunky<'_>, meta: &Meta<'_>, deflate: bool) -> Vec<u8> {
    let (w, h, c) = (l.width as usize, l.height as usize, l.channels);
    let bps = l.bits;
    let bytes = usize::from(bps / 8);
    let row = w * c * bytes;
    let rows_per_strip = (256 * 1024 / row.max(1)).clamp(1, h.max(1));
    let mut out = b"II\x2a\0\x08\0\0\0".to_vec();
    let mut offsets = Vec::new();
    let mut counts = Vec::new();
    let predictor: u16 = if l.float { 3 } else { 2 };
    // Strips of whole rows, the last one shorter; samples per strip.
    let per_strip = (w * c * rows_per_strip).max(1);
    let n = raw.len();
    let strips: Vec<std::ops::Range<usize>> = (0..n.div_ceil(per_strip))
        .map(|i| i * per_strip..((i + 1) * per_strip).min(n))
        .collect();
    if deflate {
        // Strips are independent: converted, predicted and deflated in parallel on the
        // current rayon pool.
        let coded: Vec<Vec<u8>> = strips
            .into_par_iter()
            .map(|at| {
                let mut d = vec![0u8; at.len() * bytes];
                raw.write_le(at, &mut d);
                if l.float {
                    predict_float(&mut d, row, c, bytes);
                } else {
                    predict(&mut d, row, c, bytes);
                }
                deflate_zlib(&d)
            })
            .collect();
        for data in coded {
            if out.len() % 2 == 1 {
                out.push(0);
            }
            offsets.push(out.len() as u32);
            counts.push(data.len() as u32);
            out.extend_from_slice(&data);
        }
    } else {
        // Uncompressed: each strip written in place, in parallel (word-aligned starts as
        // above, the pad bytes zero).
        let mut at = out.len();
        for s in &strips {
            at += at % 2;
            offsets.push(at as u32);
            counts.push((s.len() * bytes) as u32);
            at += s.len() * bytes;
        }
        let base = out.len();
        out.resize(at, 0);
        let mut rest = &mut out[base..];
        let mut pos = base;
        let mut slots = Vec::with_capacity(strips.len());
        for (s, &off) in strips.iter().zip(&offsets) {
            let skip = off as usize - pos;
            let (slot, tail) = std::mem::take(&mut rest)[skip..].split_at_mut(s.len() * bytes);
            slots.push(slot);
            rest = tail;
            pos = off as usize + s.len() * bytes;
        }
        slots
            .into_par_iter()
            .zip(strips)
            .for_each(|(slot, at)| raw.write_le(at, slot));
    }
    let alpha = c % 2 == 0;
    let mut ifd = vec![
        Entry::long(256, &[w as u32]),
        Entry::long(257, &[h as u32]),
        Entry::short(258, &vec![bps; c]),
        Entry::short(259, &[if deflate { 8 } else { 1 }]),
        Entry::short(262, &[if c <= 2 { 1 } else { 2 }]),
        Entry::long(273, &offsets),
        Entry::short(274, &[1]),
        Entry::short(277, &[c as u16]),
        Entry::long(278, &[rows_per_strip as u32]),
        Entry::long(279, &counts),
        Entry::short(284, &[1]),
        Entry::short(339, &vec![if l.float { 3 } else { 1 }; c]),
    ];
    if deflate {
        ifd.push(Entry::short(317, &[predictor]));
    }
    if alpha {
        // Unassociated alpha.
        ifd.push(Entry::short(338, &[2]));
    }
    let (mut exif, mut gps) = (Vec::new(), Vec::new());
    if let Some(x) = meta.exif {
        // Descriptive tags (resolution included) join the image's own IFD.
        ifd.extend(
            x.ifd0
                .iter()
                .filter(|e| e.tag != crate::exif::TAG_ORIENTATION)
                .cloned(),
        );
        exif = x.exif.clone();
        gps = x.gps.clone();
    }
    if !ifd.iter().any(|e| e.tag == crate::exif::TAG_X_RESOLUTION) {
        ifd.push(Entry::rational(crate::exif::TAG_X_RESOLUTION, 72, 1));
        ifd.push(Entry::rational(crate::exif::TAG_Y_RESOLUTION, 72, 1));
        ifd.push(Entry::short(crate::exif::TAG_RESOLUTION_UNIT, &[2]));
    }
    if let Some(icc) = meta.icc {
        ifd.push(Entry::undefined(34675, icc));
    }
    if let Some(x) = meta.xmp {
        ifd.push(Entry {
            tag: 700,
            ty: 1,
            count: x.len() as u32,
            data: x.to_vec(),
        });
    }
    crate::exif::write_ifds(&mut out, ifd, &exif, &gps);
    out
}

/// TIFF predictor 3 (floating point), per row: the bytes of every little-endian sample are
/// spread into planes, most significant first, then each byte becomes its difference from
/// the byte `c` places before it.
fn predict_float(d: &mut [u8], row: usize, c: usize, bytes: usize) {
    let wc = row / bytes;
    let mut tmp = vec![0u8; row];
    for r in d.chunks_exact_mut(row) {
        for i in 0..wc {
            for b in 0..bytes {
                tmp[b * wc + i] = r[i * bytes + (bytes - 1 - b)];
            }
        }
        for i in (c..row).rev() {
            tmp[i] = tmp[i].wrapping_sub(tmp[i - c]);
        }
        r.copy_from_slice(&tmp);
    }
}

/// TIFF predictor 2: each sample minus the same channel's sample to its left, per row.
fn predict(d: &mut [u8], row: usize, c: usize, bytes: usize) {
    for r in d.chunks_exact_mut(row) {
        if bytes == 1 {
            for i in (c..r.len()).rev() {
                r[i] = r[i].wrapping_sub(r[i - c]);
            }
        } else {
            let n = r.len() / 2;
            for i in (c..n).rev() {
                let cur = u16::from_le_bytes([r[2 * i], r[2 * i + 1]]);
                let prev = u16::from_le_bytes([r[2 * (i - c)], r[2 * (i - c) + 1]]);
                r[2 * i..2 * i + 2].copy_from_slice(&cur.wrapping_sub(prev).to_le_bytes());
            }
        }
    }
}

pub(crate) fn deflate_zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; zlib_rs::compress_bound(data.len())];
    let (c, rc) = zlib_rs::compress_slice(&mut out, data, zlib_rs::DeflateConfig::new(6));
    assert!(
        rc == zlib_rs::ReturnCode::Ok,
        "zlib compress_bound too small"
    );
    let n = c.len();
    out.truncate(n);
    out
}
