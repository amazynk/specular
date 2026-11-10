//! NITF 2.1, NSIF 1.0 and NITF 2.0 images (container read by `specular-gis`): uncompressed
//! (NC, NM) blocks of 1 to 64 bits per sample in any interleave, JPEG (C3, M3) blocks and
//! JPEG 2000 (C8, M8) codestreams through the in-tree decoders. RGB band triples, look-up
//! table (LU) bands and grey; samples over 8 bits keep their depth for one band and are
//! scaled to 8 bits for colour. Georeferenced by IGEOLO corners, else by an affine fit of
//! the RPC00B model. Complex SAR images (SICD: I and Q bands of 16-bit integers or 32-bit
//! floats, uncompressed) open in the science view through [`crate::sar`]. Vector-quantised
//! (CADRG/CIB), bi-level and other complex layouts are refused by name.
//!
//! Also the refusals for ECW and MrSID, which need their vendors' SDKs.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_gis::nitf::{self, Image, Nitf};

use crate::geo::{GeoRef, RasterGeo};
use crate::science::{AxisKind, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary};
use crate::whole::Levels;
use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Registry, Result, Session, sar,
};

/// Pixels at most.
const MAX_PIXELS: u64 = 1 << 28;

pub struct NitfDecoder;

/// Why an image cannot be shown, when its compression or pixel type is not read here.
fn refusal(im: &Image) -> Option<String> {
    let ic = im.ic.as_str();
    Some(match ic {
        "NC" | "NM" if iq_pairs(im).is_some() => return None,
        "NC" | "NM" | "C3" | "M3" | "C8" | "M8" => {
            if im.complex() {
                if im.subcats.iter().any(|s| s == "M") {
                    "complex SAR stored as magnitude and phase is not supported".into()
                } else if im.ic != "NC" && im.ic != "NM" {
                    "compressed complex SAR samples are not supported".into()
                } else {
                    "this complex SAR sample type is not supported".into()
                }
            } else if im.pvtype == "B" && im.nbpp != 1 {
                "this bi-level NITF layout is not supported".into()
            } else {
                return None;
            }
        }
        "C4" | "M4" => "vector-quantised NITF (CADRG, CIB) is not supported".into(),
        "C1" | "M1" => "bi-level (fax) NITF compression is not supported".into(),
        "C5" | "M5" => "lossless JPEG NITF compression is not supported".into(),
        "C7" | "M7" => "complex SAR compression is not supported".into(),
        "I1" => "downsampled JPEG NITF compression is not supported".into(),
        other => format!("NITF compression {other} is not supported"),
    })
}

fn compression_name(ic: &str) -> &'static str {
    match ic {
        "NC" => "uncompressed",
        "NM" => "uncompressed, masked",
        "C3" => "JPEG",
        "M3" => "JPEG, masked",
        "C8" => "JPEG 2000",
        "M8" => "JPEG 2000, masked",
        _ => "other",
    }
}

/// The (I, Q) band pairs of a complex image read here: uncompressed 16-bit integers or
/// 32-bit floats (SICD's RE16I_IM16I and RE32F_IM32F).
fn iq_pairs(im: &Image) -> Option<Vec<(usize, usize)>> {
    let typed = matches!((im.pvtype.as_str(), im.nbpp), ("SI", 16) | ("R", 32));
    if !typed || !matches!(im.ic.as_str(), "NC" | "NM") {
        return None;
    }
    let of = |c: &str| {
        im.subcats
            .iter()
            .enumerate()
            .filter(|(_, s)| s.as_str() == c)
            .map(|(k, _)| k)
            .collect::<Vec<_>>()
    };
    let (i, q) = (of("I"), of("Q"));
    (!i.is_empty() && i.len() == q.len()).then(|| i.into_iter().zip(q).collect())
}

/// The image to show: the first displayable one.
fn pick(n: &Nitf) -> Option<usize> {
    n.images
        .iter()
        .position(|im| im.irep != "NODISPLY" && im.icat != "MASK")
        .or((!n.images.is_empty()).then_some(0))
}

impl Decoder for NitfDecoder {
    fn format(&self) -> Format {
        Format::Nitf
    }
    fn probe(&self, head: &[u8]) -> bool {
        nitf::sniff(head) || head.starts_with(b"NITF01.10")
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ntf", "nitf", "nsf"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let n = nitf::parse(data).map_err(|e| {
            if e.contains("not supported") {
                DecodeError::Unsupported(e)
            } else {
                DecodeError::Malformed(e)
            }
        })?;
        let k = pick(&n).ok_or(DecodeError::Unsupported("the NITF holds no image".into()))?;
        let im = n.images[k].clone();
        if let Some(why) = refusal(&im) {
            return Err(DecodeError::Unsupported(why));
        }
        if u64::from(im.cols) * u64::from(im.rows) > MAX_PIXELS {
            return Err(DecodeError::TooLarge);
        }
        if let Some(pairs) = iq_pairs(&im) {
            return Ok(Arc::new(SarSession::new(source, &n, k, im, pairs)));
        }
        let channels = channels(&im);
        let wide = channels.len() == 1 && im.abpp > 8 && lut(&im).is_none();
        let layout = match (channels.len(), wide) {
            (1, true) => PixelLayout::Gray16,
            (1, false) if lut(&im).is_none() => PixelLayout::Gray,
            _ => PixelLayout::Rgba,
        };
        let geo = georef(&n, &im);
        let mut coding = format!("{}, {}", n.version, compression_name(&im.ic));
        if n.images.len() > 1 {
            coding += &format!(", image {} of {}", k + 1, n.images.len());
        }
        let info = ImageInfo {
            format: Format::Nitf,
            width: im.cols,
            height: im.rows,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: if wide { 16 } else { 8 },
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(NitfSession {
            source,
            info,
            image: im,
            channels,
            layout,
            geo: geo.map(Arc::new),
            levels: Levels::new(),
        }))
    }
}

/// The bands shown, in output order: R, G, B when the image names them (or the first three
/// of an RGB image), else the first band.
fn channels(im: &Image) -> Vec<usize> {
    let find = |r: &str| im.bands.iter().position(|b| b == r);
    if let (Some(r), Some(g), Some(b)) = (find("R"), find("G"), find("B")) {
        return vec![r, g, b];
    }
    // Three bands without colour names (GDAL writes them as MONO) are taken as RGB.
    if (im.bands.len() >= 3 && im.irep == "RGB") || im.bands.len() == 3 {
        return vec![0, 1, 2];
    }
    vec![0]
}

/// The look-up tables of a single LU band (one to three outputs).
fn lut(im: &Image) -> Option<&Vec<Vec<u8>>> {
    (im.bands.len() == 1 && im.bands[0] == "LU")
        .then(|| &im.luts[0])
        .filter(|l| !l.is_empty())
}

fn georef(n: &Nitf, im: &Image) -> Option<GeoRef> {
    let mut keys: Vec<(String, String)> = vec![
        ("FVER".into(), n.version.clone()),
        ("FTITLE".into(), n.title.clone()),
        ("ONAME".into(), n.originator.clone()),
        ("FDT".into(), n.date.clone()),
        ("FSCLAS".into(), n.classification.clone()),
        ("IID1".into(), im.iid1.clone()),
        ("IID2".into(), im.iid2.clone()),
        ("IDATIM".into(), im.date.clone()),
        ("ISORCE".into(), im.source.clone()),
        ("ICAT".into(), im.icat.clone()),
        ("IREP".into(), im.irep.clone()),
        ("IC".into(), im.ic.clone()),
        ("PVTYPE".into(), im.pvtype.clone()),
        ("ABPP".into(), im.abpp.to_string()),
        ("NBANDS".into(), im.bands.len().to_string()),
        ("IMODE".into(), im.imode.to_string()),
        ("ICORDS".into(), im.icords.to_string()),
        ("IGEOLO".into(), im.igeolo.trim().to_string()),
    ];
    for c in &im.comments {
        keys.push(("ICOM".into(), c.clone()));
    }
    let tres: Vec<&str> = im.tres.iter().map(|(t, _)| t.as_str()).collect();
    if !tres.is_empty() {
        keys.push(("TRE".into(), tres.join(", ")));
    }
    keys.retain(|(_, v)| !v.trim().is_empty());
    let (to_map, crs, note) = match nitf::transform(im) {
        Some((t, c)) => (t, Some(c), None),
        None => {
            let rpc = im.rpc()?;
            let (t, worst) = rpc.fitted(im.cols, im.rows)?;
            (
                t,
                specular_gis::crs::from_epsg(4326),
                Some(format!("RPC model, fitted (within {worst:.1} pixels)")),
            )
        }
    };
    Some(crate::geo::raster(
        to_map,
        crs,
        note,
        (im.cols, im.rows),
        RasterGeo {
            levels: vec![(im.cols, im.rows)],
            keys,
            keys_kind: "header",
            ..Default::default()
        },
    ))
}

struct NitfSession {
    source: Arc<Source>,
    info: ImageInfo,
    image: Image,
    channels: Vec<usize>,
    layout: PixelLayout,
    geo: Option<Arc<GeoRef>>,
    levels: Levels,
}

/// Where decoded samples go: an output buffer with its layout and the value range.
struct Out {
    w: usize,
    h: usize,
    layout: PixelLayout,
    data: Vec<u8>,
}

impl Out {
    fn put(&mut self, x: usize, y: usize, ch: usize, v: u16) {
        if x >= self.w || y >= self.h {
            return;
        }
        match self.layout {
            PixelLayout::Gray => self.data[y * self.w + x] = v as u8,
            PixelLayout::Gray16 => {
                let i = (y * self.w + x) * 2;
                self.data[i..i + 2].copy_from_slice(&v.to_ne_bytes());
            }
            _ => {
                let i = (y * self.w + x) * 4;
                self.data[i + ch] = v as u8;
                self.data[i + 3] = 255;
            }
        }
    }
}

impl NitfSession {
    /// Samples to output values: 8-bit (or 16-bit for one wide band) by the actual bits per
    /// pixel; floats and signed values stretched over `range`.
    fn scale(&self, v: f64, range: (f64, f64)) -> u16 {
        let im = &self.image;
        let target = if self.layout == PixelLayout::Gray16 {
            65535.0
        } else {
            255.0
        };
        let (lo, hi) = range;
        let t = if im.pvtype == "R" || im.pvtype == "SI" {
            (v - lo) / (hi - lo).max(f64::MIN_POSITIVE)
        } else if im.left_justified && im.nbpp > im.abpp {
            // Left-justified: the actual bits are the high ones.
            (v / 2f64.powi((im.nbpp - im.abpp) as i32)).floor() / (2f64.powi(im.abpp as i32) - 1.0)
        } else {
            v / (2f64.powi(im.abpp.min(32) as i32) - 1.0)
        };
        (t.clamp(0.0, 1.0) * target).round() as u16
    }

    fn full(&self, cancel: &CancelToken) -> Result<Raster> {
        let im = &self.image;
        let (w, h) = (im.cols as usize, im.rows as usize);
        let bpp = self.layout.pixel_bytes();
        let mut out = Out {
            w,
            h,
            layout: self.layout,
            data: vec![0u8; w * h * bpp],
        };
        let file = self.source.bytes();
        let mut truncated = false;
        let (bw, bh) = (im.block_w as usize, im.block_h as usize);
        match im.ic.as_str() {
            "NC" | "NM" => {
                // Floats and signed samples are stretched over their range.
                let mut range = (f64::INFINITY, f64::NEG_INFINITY);
                let mut blocks = Vec::new();
                for by in 0..im.blocks_y {
                    for bx in 0..im.blocks_x {
                        if cancel.is_cancelled() {
                            return Err(DecodeError::Cancelled);
                        }
                        let b = im.block(file, bx, by).map_err(DecodeError::Malformed)?;
                        if let Some(b) = &b
                            && (im.pvtype == "R" || im.pvtype == "SI")
                        {
                            for &c in &self.channels {
                                for &v in &b[c] {
                                    if v.is_finite() {
                                        range = (range.0.min(v), range.1.max(v));
                                    }
                                }
                            }
                        }
                        blocks.push((bx, by, b));
                    }
                }
                for (bx, by, b) in blocks {
                    let Some(b) = b else {
                        truncated |= !im.masked();
                        continue;
                    };
                    let (x0, y0) = (bx as usize * bw, by as usize * bh);
                    for y in 0..bh {
                        for x in 0..bw {
                            let i = y * bw + x;
                            if let Some(lut) = lut(im) {
                                let k = b[0][i] as usize;
                                for ch in 0..3 {
                                    let t = &lut[ch.min(lut.len() - 1)];
                                    out.put(
                                        x0 + x,
                                        y0 + y,
                                        ch,
                                        u16::from(t.get(k).copied().unwrap_or(0)),
                                    );
                                }
                                continue;
                            }
                            for (ch, &c) in self.channels.iter().enumerate() {
                                let v = self.scale(b[c][i], range);
                                if self.channels.len() == 1 && self.layout == PixelLayout::Rgba {
                                    for k in 0..3 {
                                        out.put(x0 + x, y0 + y, k, v);
                                    }
                                } else {
                                    out.put(x0 + x, y0 + y, ch, v);
                                }
                            }
                        }
                    }
                }
            }
            "C3" | "M3" => {
                let streams = im.jpeg_blocks(file).map_err(DecodeError::Malformed)?;
                let reg = Registry::default();
                for (k, s) in streams.into_iter().enumerate() {
                    if cancel.is_cancelled() {
                        return Err(DecodeError::Cancelled);
                    }
                    let Some(s) = s else { continue };
                    let (bx, by) = (k % im.blocks_x as usize, k / im.blocks_x as usize);
                    match decode_embedded(&reg, s, &self.source, (bw, bh), cancel) {
                        Ok(r) => self.place(&r, bx * bw, by * bh, &mut out),
                        Err(DecodeError::Cancelled) => return Err(DecodeError::Cancelled),
                        Err(_) => truncated = true,
                    }
                }
            }
            _ => {
                let cs = im.j2k(file).map_err(DecodeError::Malformed)?;
                let r = decode_embedded(&Registry::default(), cs, &self.source, (w, h), cancel)?;
                self.place(&r, 0, 0, &mut out);
            }
        }
        Ok(crate::whole::interleaved(
            im.cols,
            im.rows,
            self.layout,
            out.data,
            truncated,
        ))
    }

    /// Copies a decoded block (any layout) into the output at (`x0`, `y0`).
    fn place(&self, r: &Raster, x0: usize, y0: usize, out: &mut Out) {
        if self.layout == PixelLayout::Gray16 && r.layout == PixelLayout::Gray16 {
            let p = &r.planes[0];
            let shift = 16u32.saturating_sub(self.image.abpp.min(16));
            for y in 0..r.height as usize {
                for x in 0..r.width as usize {
                    let i = y * p.stride + x * 2;
                    let v = u16::from_ne_bytes([p.data[i], p.data[i + 1]]);
                    out.put(x0 + x, y0 + y, 0, v << shift);
                }
            }
            return;
        }
        let rgba = if r.layout == PixelLayout::YCbCr {
            ycbcr_smooth(r)
        } else {
            crate::gainmap::to_rgba8(r)
        };
        let p = &rgba.planes[0];
        for y in 0..rgba.height as usize {
            for x in 0..rgba.width as usize {
                let i = y * p.stride + x * 4;
                match out.layout {
                    PixelLayout::Gray => out.put(x0 + x, y0 + y, 0, u16::from(p.data[i])),
                    PixelLayout::Gray16 => out.put(x0 + x, y0 + y, 0, u16::from(p.data[i]) * 257),
                    _ => {
                        for ch in 0..3 {
                            out.put(x0 + x, y0 + y, ch, u16::from(p.data[i + ch]));
                        }
                    }
                }
            }
        }
    }
}

/// YCbCr planes to RGBA8 with the chroma interpolated between its (centred) samples, as
/// libjpeg's smooth upsampling does, rather than repeated.
fn ycbcr_smooth(r: &Raster) -> Raster {
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = vec![255u8; w * h * 4];
    let chroma = |p: usize, x: usize, y: usize| -> f32 {
        let Some(pl) = r.planes.get(p) else {
            return 128.0;
        };
        let (hd, vd) = (pl.h_div.max(1) as f32, pl.v_div.max(1) as f32);
        let fx = ((x as f32 + 0.5) / hd - 0.5).max(0.0);
        let fy = ((y as f32 + 0.5) / vd - 0.5).max(0.0);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let lim = |v: usize, n: u32| v.min(n.saturating_sub(1) as usize);
        let at = |xx: usize, yy: usize| {
            f32::from(
                pl.data
                    .get(lim(yy, pl.height) * pl.stride + lim(xx, pl.width))
                    .copied()
                    .unwrap_or(128),
            )
        };
        let top = at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx;
        let bot = at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx;
        top * (1.0 - ty) + bot * ty
    };
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    let yp = &r.planes[0];
    for y in 0..h {
        for x in 0..w {
            let yy = f32::from(yp.data.get(y * yp.stride + x).copied().unwrap_or(0));
            let cb = chroma(1, x, y) - 128.0;
            let cr = chroma(2, x, y) - 128.0;
            let o = (y * w + x) * 4;
            out[o] = q(yy + 1.402 * cr);
            out[o + 1] = q(yy - 0.344_136 * cb - 0.714_136 * cr);
            out[o + 2] = q(yy + 1.772 * cb);
        }
    }
    crate::whole::interleaved(r.width, r.height, PixelLayout::Rgba, out, r.truncated)
}

/// Decodes an embedded JPEG or JPEG 2000 stream with the registry. Its own header must not
/// claim much more than the `expect`ed size (width, height): the NITF's size cap does not
/// hold for it.
fn decode_embedded(
    reg: &Registry,
    bytes: &[u8],
    parent: &Source,
    expect: (usize, usize),
    cancel: &CancelToken,
) -> Result<Raster> {
    let s = reg.open(Arc::new(
        Source::from_vec("block", bytes.to_vec()).within(parent),
    ))?;
    let info = s.info();
    let allowed = (expect.0 as u64 * expect.1 as u64).max(1 << 16) * 4;
    if u64::from(info.width) * u64::from(info.height) > allowed {
        return Err(DecodeError::TooLarge);
    }
    s.decode(&DecodeRequest::full(1), cancel)
}

/// A complex SAR image (SICD): I and Q bands through [`crate::sar`].
struct SarSession {
    source: Arc<Source>,
    info: ImageInfo,
    image: Image,
    pairs: Vec<(usize, usize)>,
    summary: ScienceSummary,
    geo: Option<Arc<GeoRef>>,
    planes: PlaneCache,
    levels: Levels,
}

impl SarSession {
    fn new(
        source: Arc<Source>,
        n: &Nitf,
        k: usize,
        im: Image,
        pairs: Vec<(usize, usize)>,
    ) -> SarSession {
        let geo = georef(n, &im).map(Arc::new);
        let sample = format!(
            "complex {}, {}",
            if im.pvtype == "R" {
                "32-bit float"
            } else {
                "16-bit signed"
            },
            compression_name(&im.ic)
        );
        let mut coding = format!("{}, {sample}", n.version);
        if n.images.len() > 1 {
            coding += &format!(", image {} of {}", k + 1, n.images.len());
        }
        let info = ImageInfo {
            format: Format::Nitf,
            width: im.cols,
            height: im.rows,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: if im.pvtype == "R" { 32 } else { 16 },
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let mut axes = Vec::new();
        if pairs.len() > 1 {
            axes.push(NdAxis {
                name: String::new(),
                len: pairs.len() as u32,
                unit: String::new(),
                labels: Vec::new(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
        }
        axes.extend(sar::axes());
        let keywords = geo
            .as_ref()
            .and_then(|g| match &g.facts {
                crate::geo::GeoFacts::Raster(r) => Some(r.keys.clone()),
                _ => None,
            })
            .unwrap_or_default()
            .into_iter()
            .map(|(k, v)| (k, v, String::new()))
            .collect();
        let summary = ScienceSummary {
            width: im.cols,
            height: im.rows,
            axes,
            sample,
            title: Some(im.iid2.trim().to_owned())
                .filter(|t| !t.is_empty())
                .or_else(|| Some(im.iid1.trim().to_owned()).filter(|t| !t.is_empty())),
            kind: ScienceKind::Gridded,
            keywords: Arc::new(keywords),
            geo: geo.clone(),
            start: sar::start(pairs.len() > 1),
            ..Default::default()
        };
        SarSession {
            source,
            info,
            image: im,
            pairs,
            summary,
            geo,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        }
    }
}

impl sar::ComplexSource for SarSession {
    fn band(&self, b: u32, cancel: &CancelToken) -> Result<(Vec<[f32; 2]>, bool)> {
        let im = &self.image;
        let &(bi, bq) = self
            .pairs
            .get(b as usize)
            .ok_or_else(|| DecodeError::Malformed("no such band".into()))?;
        let (w, h) = (im.cols as usize, im.rows as usize);
        let (bw, bh) = (im.block_w as usize, im.block_h as usize);
        let mut out = vec![[f32::NAN; 2]; w * h];
        let mut truncated = false;
        let file = self.source.bytes();
        for by in 0..im.blocks_y {
            for bx in 0..im.blocks_x {
                if cancel.is_cancelled() {
                    return Err(DecodeError::Cancelled);
                }
                let Some(blk) = im.block(file, bx, by).map_err(DecodeError::Malformed)? else {
                    truncated |= !im.masked();
                    continue;
                };
                let (x0, y0) = (bx as usize * bw, by as usize * bh);
                for y in 0..bh.min(h.saturating_sub(y0)) {
                    for x in 0..bw.min(w.saturating_sub(x0)) {
                        let i = y * bw + x;
                        out[(y0 + y) * w + x0 + x] = [blk[bi][i] as f32, blk[bq][i] as f32];
                    }
                }
            }
        }
        Ok((out, truncated))
    }
}

impl Session for SarSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let size = (self.info.width, self.info.height);
        let multi = self.pairs.len() > 1;
        self.levels.decode(
            req,
            size,
            |_| 1,
            |_| sar::picture(self, &self.planes, size, multi, cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        u64::from(w) * u64::from(h) * 16 + crate::whole::level_bytes(w, h, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.geo.clone()
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        let size = (self.info.width, self.info.height);
        let multi = self.pairs.len() > 1;
        Some(sar::plane(self, &self.planes, size, index, multi, cancel))
    }
}

impl Session for NitfSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.full(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, _req: &DecodeRequest) -> u64 {
        u64::from(self.info.width)
            * u64::from(self.info.height)
            * self.layout.pixel_bytes() as u64
            * 2
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.geo.clone()
    }
}
