//! Retro, game and terminal rasters through the in-tree `specular-retro` decoders: Sixel, X
//! cursors (one page per size), AVS, MacPaint, GEM IMG, Atari ST DEGAS and NEOchrome,
//! Commodore 64 Koala, ZX Spectrum screens, Dr. Halo CUT (with the `.pal` beside it),
//! Softimage PIC, Alias PIX, Wavefront RLA/RPF, Utah RLE, Quake 2 WAL (with the game's
//! `colormap.pcx` when it is found nearby) and Blizzard BLP (DXT mips through
//! `specular-texture`, JPEG mips through `specular-jpeg`). Also QuickTime still images
//! (`.qtif`): the JPEG, PNG, TIFF, GIF or BMP inside opens through the registry; raw RGB is
//! read here.
//!
//! Images are decoded whole once (to 8-bit grey or RGBA); coarser levels are box-filtered
//! from it and regions are cropped.

use crate::ident::Confidence;
use crate::whole::{self, Levels};
use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_retro as rt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Which decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sixel,
    Xcursor,
    Avs,
    MacPaint,
    Gem,
    Degas,
    Neochrome,
    Koala,
    ZxScreen,
    DrHalo,
    Softimage,
    AliasPix,
    Rla,
    UtahRle,
    Wal,
    Blp,
    /// A raw QuickTime image (opened through [`QtifDecoder`]).
    QtRaw,
}

pub struct RetroDecoder(pub Kind);

fn map_err(e: rt::Error) -> DecodeError {
    match e {
        rt::Error::NotRecognised => DecodeError::UnsupportedFormat,
        rt::Error::Unsupported(s) => DecodeError::Unsupported(s),
        rt::Error::Truncated(s) => DecodeError::Truncated(s),
        rt::Error::Malformed(s) => DecodeError::Malformed(s),
        rt::Error::TooLarge => DecodeError::TooLarge,
    }
}

/// Side files read at most (palettes are small).
const SIDE_MAX: u64 = 1 << 20;

fn read_side(p: &Path) -> Option<Vec<u8>> {
    let m = std::fs::metadata(p).ok()?;
    if !m.is_file() || m.len() > SIDE_MAX {
        return None;
    }
    std::fs::read(p).ok()
}

/// The `.pal` beside a Dr. Halo `.cut` (same name, either case).
fn halo_palette(path: &Path) -> Option<Vec<u8>> {
    ["pal", "PAL", "Pal"]
        .into_iter()
        .find_map(|e| read_side(&path.with_extension(e)))
        .filter(|p| rt::halo::palette(p).is_some())
}

/// Quake 2's palette near a `.wal`: `colormap.pcx` in the texture's folder, or in a `pics`
/// folder up to three levels above (`baseq2/textures/e1u1/x.wal` → `baseq2/pics`).
fn wal_palette(path: &Path) -> Option<Vec<[u8; 4]>> {
    let mut cands: Vec<PathBuf> = Vec::new();
    let mut dir = path.parent();
    for _ in 0..4 {
        let Some(d) = dir else { break };
        cands.push(d.join("colormap.pcx"));
        cands.push(d.join("pics").join("colormap.pcx"));
        dir = d.parent();
    }
    cands
        .iter()
        .find_map(|p| read_side(p).as_deref().and_then(rt::wal::pcx_palette))
}

/// What a session decodes from besides the file.
#[derive(Clone, Default)]
struct Side {
    halo: Option<Arc<Vec<u8>>>,
    wal: Option<Arc<Vec<[u8; 4]>>>,
}

fn side(kind: Kind, source: &Source) -> Side {
    let on_disk = source.path().is_file();
    match kind {
        Kind::DrHalo if on_disk => Side {
            halo: halo_palette(source.path()).map(Arc::new),
            ..Side::default()
        },
        Kind::Wal if on_disk => Side {
            wal: wal_palette(source.path()).map(Arc::new),
            ..Side::default()
        },
        _ => Side::default(),
    }
}

fn info(kind: Kind, d: &[u8], page: u32, side: &Side) -> rt::Result<rt::Info> {
    use rt::atari::Kind as A;
    match kind {
        Kind::Sixel => rt::sixel::info(d),
        Kind::Xcursor => rt::xcursor::info(d, page),
        Kind::Avs => rt::avs::info(d),
        Kind::MacPaint => rt::macpaint::info(d),
        Kind::Gem => rt::gem::info(d),
        Kind::Degas => rt::atari::info(A::Degas, d),
        Kind::Neochrome => rt::atari::info(A::Neochrome, d),
        Kind::Koala => rt::koala::info(d),
        Kind::ZxScreen => rt::zxscr::info(d),
        Kind::DrHalo => rt::halo::info(d, side.halo.as_deref().map(|v| v.as_slice())),
        Kind::Softimage => rt::softimage::info(d),
        Kind::AliasPix => rt::aliaspix::info(d),
        Kind::Rla => rt::rla::info(d),
        Kind::UtahRle => rt::utahrle::info(d),
        Kind::Wal => rt::wal::info(d, side.wal.is_some()),
        Kind::Blp => rt::blp::info(d),
        Kind::QtRaw => qt_raw_info(d),
    }
}

fn decode(kind: Kind, d: &[u8], page: u32, side: &Side) -> Result<rt::Image> {
    use rt::atari::Kind as A;
    let r = match kind {
        Kind::Sixel => rt::sixel::decode(d),
        Kind::Xcursor => rt::xcursor::decode(d, page),
        Kind::Avs => rt::avs::decode(d),
        Kind::MacPaint => rt::macpaint::decode(d),
        Kind::Gem => rt::gem::decode(d),
        Kind::Degas => rt::atari::decode(A::Degas, d),
        Kind::Neochrome => rt::atari::decode(A::Neochrome, d),
        Kind::Koala => rt::koala::decode(d),
        Kind::ZxScreen => rt::zxscr::decode(d),
        Kind::DrHalo => rt::halo::decode(d, side.halo.as_deref().map(|v| v.as_slice())),
        Kind::Softimage => rt::softimage::decode(d),
        Kind::AliasPix => rt::aliaspix::decode(d),
        Kind::Rla => rt::rla::decode(d),
        Kind::UtahRle => rt::utahrle::decode(d),
        Kind::Wal => rt::wal::decode(d, side.wal.as_deref().map(|v| v.as_slice())),
        Kind::Blp => return blp(d),
        Kind::QtRaw => qt_raw(d),
    };
    r.map_err(map_err)
}

/// A BLP's first mip as pixels: DXT blocks through the texture decoder (wrapped in a DDS
/// header), JPEG data through the JPEG decoder's raw components (B, G, R, A).
fn blp(d: &[u8]) -> Result<rt::Image> {
    match rt::blp::decode(d).map_err(map_err)? {
        rt::blp::Mip::Pixels(img) => Ok(img),
        rt::blp::Mip::Dxt {
            kind,
            width,
            height,
            blocks,
        } => {
            let dds = dds_wrap(kind, width, height, &blocks);
            let t = specular_texture::Texture::open(&dds)
                .map_err(|e| DecodeError::Malformed(format!("BLP DXT: {e}")))?;
            let out = t
                .decode(&dds, &specular_texture::DecodeOptions::default())
                .map_err(|e| DecodeError::Malformed(format!("BLP DXT: {e}")))?;
            Ok(rt::Image {
                width,
                height,
                format: rt::PixelFormat::Rgba8,
                data: out.data,
                truncated: out.truncated,
            })
        }
        rt::blp::Mip::Jpeg { jpeg, alpha } => {
            let opts = specular_jpeg::DecodeOptions {
                scale: specular_jpeg::Scale::Full,
                region: None,
                cancel: None,
                parallelism: specular_jpeg::Parallelism::Auto,
            };
            let p = specular_jpeg::decode_components(&jpeg, &opts)
                .map_err(|e| DecodeError::Malformed(format!("BLP JPEG: {e}")))?;
            let (w, h) = (p.width, p.height);
            if p.planes.len() < 3 {
                return Err(DecodeError::Unsupported(format!(
                    "BLP JPEG with {} components",
                    p.planes.len()
                )));
            }
            let mut img = rt::Image::filled(w, h, rt::PixelFormat::Rgba8, 255);
            let max_h = p.planes.iter().map(|q| q.h_samp).max().unwrap_or(1).max(1) as usize;
            let max_v = p.planes.iter().map(|q| q.v_samp).max().unwrap_or(1).max(1) as usize;
            let sample = |c: usize, x: usize, y: usize| -> u8 {
                let q = &p.planes[c];
                let fx = max_h / (q.h_samp.max(1) as usize);
                let fy = max_v / (q.v_samp.max(1) as usize);
                let (sx, sy) = (
                    (x / fx).min(q.width as usize - 1),
                    (y / fy).min(q.height as usize - 1),
                );
                q.data.get(sy * q.stride + sx).copied().unwrap_or(0)
            };
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let o = (y * w as usize + x) * 4;
                    img.data[o] = sample(2, x, y);
                    img.data[o + 1] = sample(1, x, y);
                    img.data[o + 2] = sample(0, x, y);
                    if alpha && p.planes.len() > 3 {
                        img.data[o + 3] = sample(3, x, y);
                    }
                }
            }
            img.truncated = p.truncated;
            Ok(img)
        }
    }
}

/// A minimal DDS file holding one DXT1/3/5 level.
fn dds_wrap(kind: u8, w: u32, h: u32, blocks: &[u8]) -> Vec<u8> {
    let mut f = vec![0u8; 128];
    let put = |f: &mut [u8], at: usize, v: u32| f[at..at + 4].copy_from_slice(&v.to_le_bytes());
    f[..4].copy_from_slice(b"DDS ");
    put(&mut f, 4, 124);
    put(&mut f, 8, 0x1 | 0x2 | 0x4 | 0x1000 | 0x8_0000);
    put(&mut f, 12, h);
    put(&mut f, 16, w);
    put(&mut f, 20, blocks.len() as u32);
    put(&mut f, 76, 32);
    put(&mut f, 80, 0x4);
    f[84..88].copy_from_slice(match kind {
        1 => b"DXT1",
        2 => b"DXT3",
        _ => b"DXT5",
    });
    put(&mut f, 108, 0x1000);
    f.extend_from_slice(blocks);
    f
}

fn to_raster(img: rt::Image) -> Raster {
    let layout = match img.format {
        rt::PixelFormat::Gray8 => PixelLayout::Gray,
        rt::PixelFormat::Rgba8 => PixelLayout::Rgba,
    };
    whole::interleaved(img.width, img.height, layout, img.data, img.truncated)
}

impl RetroDecoder {
    /// Whether the whole file is this format (no-signature formats check sizes and
    /// structure).
    fn content(&self, d: &[u8]) -> bool {
        use rt::atari::Kind as A;
        match self.0 {
            Kind::Sixel => rt::sixel::probe(d),
            Kind::Xcursor => rt::xcursor::probe(d),
            Kind::Softimage => rt::softimage::probe(d),
            Kind::UtahRle => rt::utahrle::probe(d),
            Kind::Blp => rt::blp::probe(d),
            Kind::Gem => rt::gem::probe(d),
            Kind::Avs => rt::avs::probe_file(d),
            Kind::Degas => rt::atari::probe_file(A::Degas, d),
            Kind::Neochrome => rt::atari::probe_file(A::Neochrome, d),
            Kind::Koala => rt::koala::probe_file(d),
            Kind::ZxScreen => rt::zxscr::probe_file(d),
            Kind::DrHalo => rt::halo::probe_file(d),
            Kind::AliasPix => rt::aliaspix::probe_file(d),
            Kind::Rla => rt::rla::probe_file(d),
            Kind::Wal => rt::wal::probe_file(d),
            Kind::MacPaint => rt::macpaint::info(d).is_ok(),
            Kind::QtRaw => qt_raw_info(d).is_ok(),
        }
    }
}

impl Decoder for RetroDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Sixel => Format::Sixel,
            Kind::Xcursor => Format::Xcursor,
            Kind::Avs => Format::Avs,
            Kind::MacPaint => Format::MacPaint,
            Kind::Gem => Format::GemImg,
            Kind::Degas => Format::Degas,
            Kind::Neochrome => Format::Neochrome,
            Kind::Koala => Format::Koala,
            Kind::ZxScreen => Format::ZxScreen,
            Kind::DrHalo => Format::DrHalo,
            Kind::Softimage => Format::SoftimagePic,
            Kind::AliasPix => Format::AliasPix,
            Kind::Rla => Format::Rla,
            Kind::UtahRle => Format::UtahRle,
            Kind::Wal => Format::Wal,
            Kind::Blp => Format::Blp,
            Kind::QtRaw => Format::Qtif,
        }
    }
    /// Signatures: Sixel's device control string, "Xcur", Softimage's magic, Utah RLE's
    /// magic, "BLP1"/"BLP2". GEM IMG has a header strict enough to check but shares `.img`
    /// with disk images: only asked by the registry for `.img` files. The rest have no
    /// signature and are chosen by extension (checked when opened).
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Sixel => rt::sixel::probe(head),
            Kind::Xcursor => rt::xcursor::probe(head),
            Kind::Softimage => rt::softimage::probe(head),
            Kind::UtahRle => rt::utahrle::probe(head),
            Kind::Blp => rt::blp::probe(head),
            _ => false,
        }
    }
    /// The signatures are certain; a header whose fields fit the file is likely (GEM IMG,
    /// RLA, WAL, Alias PIX, Dr. Halo CUT); a size that fits is only possible (AVS, Atari,
    /// Koala, ZX Spectrum, MacPaint).
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        if !self.content(data) {
            return None;
        }
        Some(match self.0 {
            Kind::Sixel | Kind::Xcursor | Kind::Softimage | Kind::UtahRle | Kind::Blp => {
                Confidence::Certain
            }
            Kind::Gem | Kind::Rla | Kind::Wal | Kind::AliasPix | Kind::DrHalo | Kind::QtRaw => {
                Confidence::Likely
            }
            // Its exact size rules out the CPC screen sharing `.scr`.
            Kind::ZxScreen => Confidence::Likely,
            Kind::Avs | Kind::Degas | Kind::Neochrome | Kind::Koala | Kind::MacPaint => {
                Confidence::Possible
            }
        })
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Sixel => &["six", "sixel"],
            // Cursor files have no extension; by content.
            Kind::Xcursor => &[],
            Kind::Avs => &["avs"],
            Kind::MacPaint => &["pntg", "mac"],
            // `.img`: by content in `Registry::open_img`.
            Kind::Gem => &[],
            Kind::Degas => &["pi1", "pi2", "pi3", "pc1", "pc2", "pc3"],
            Kind::Neochrome => &["neo"],
            Kind::Koala => &["koa", "kla"],
            Kind::ZxScreen => &["scr"],
            Kind::DrHalo => &["cut"],
            // `.pic` is Radiance's too; both by content.
            Kind::Softimage => &["pic"],
            Kind::AliasPix => &["pix"],
            Kind::Rla => &["rla", "rpf"],
            Kind::UtahRle => &["rle"],
            Kind::Wal => &["wal"],
            Kind::Blp => &["blp"],
            Kind::QtRaw => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        if !self.content(source.bytes()) {
            // A file named for a format without a signature that is not one (a `.scr`
            // screen saver, an AviSynth `.avs` script).
            return Err(DecodeError::Unsupported(format!(
                "not a {} file",
                self.format().name()
            )));
        }
        let side = side(self.0, &source);
        open_page(self.0, source, 0, side)
    }
}

fn open_page(kind: Kind, source: Arc<Source>, page: u32, side: Side) -> Result<Arc<dyn Session>> {
    let i = info(kind, source.bytes(), page, &side).map_err(map_err)?;
    let info = ImageInfo {
        format: RetroDecoder(kind).format(),
        width: i.width,
        height: i.height,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: i.has_alpha,
        bit_depth: i.bit_depth,
        progressive: false,
        coding: i.coding.clone(),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    let model = match i.model {
        rt::Model::Grey => "Greyscale",
        rt::Model::Rgb => "RGB",
        rt::Model::Palette => "Palette",
    };
    let bpp = i.pixel_format.channels() as u64;
    Ok(Arc::new(RetroSession {
        kind,
        page,
        pages: i.pages,
        source,
        side,
        info,
        model,
        bpp,
        levels: Levels::new(),
    }))
}

struct RetroSession {
    kind: Kind,
    page: u32,
    pages: u32,
    source: Arc<Source>,
    side: Side,
    info: ImageInfo,
    model: &'static str,
    bpp: u64,
    levels: Levels,
}

impl Session for RetroSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(Alpha::Straight)
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
            |_| decode(self.kind, self.source.bytes(), self.page, &self.side).map(to_raster),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, self.bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The full image plus the decoder's working buffers (indices, planes).
            m += whole::level_bytes(w, h, 1, 8);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.pages > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.pages {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        open_page(self.kind, self.source.clone(), page, self.side.clone())
    }
}

// ----- QuickTime image --------------------------------------------------------------------

/// QuickTime still images (`.qtif`, `.qif`): an `idsc` image description atom (codec,
/// size, depth) and an `idat` atom with the coded image.
pub struct QtifDecoder;

/// The image description and data of a QTIF file.
struct Qtif<'a> {
    codec: [u8; 4],
    width: u32,
    height: u32,
    depth: u16,
    data: &'a [u8],
}

fn qtif(d: &[u8]) -> Option<Qtif<'_>> {
    let mut i = 0;
    let (mut idsc, mut idat) = (None, None);
    while i + 8 <= d.len() {
        let size = u32::from_be_bytes(d[i..i + 4].try_into().ok()?) as usize;
        let kind = &d[i + 4..i + 8];
        let end = if size == 0 {
            d.len()
        } else {
            i.checked_add(size)?
        };
        if size != 0 && size < 8 {
            return None;
        }
        let body = &d[i + 8..end.min(d.len())];
        match kind {
            b"idsc" => idsc = Some(body),
            b"idat" => idat = Some(body),
            _ => {}
        }
        i = end;
    }
    let desc = idsc?;
    // The description repeats its size first; then codec, reserved, ref index, version,
    // revision, vendor, temporal and spatial quality, width, height…
    if desc.len() < 82 {
        return None;
    }
    let codec: [u8; 4] = desc[4..8].try_into().ok()?;
    let u16be = |o: usize| u16::from_be_bytes([desc[o], desc[o + 1]]);
    Some(Qtif {
        codec,
        width: u32::from(u16be(32)),
        height: u32::from(u16be(34)),
        depth: u16be(82.min(desc.len() - 2)),
        data: idat?,
    })
}

impl Decoder for QtifDecoder {
    fn format(&self) -> Format {
        Format::Qtif
    }
    /// An `idsc` or `idat` atom first (shared-MIME-info checks "idat" at 4).
    fn probe(&self, head: &[u8]) -> bool {
        head.len() >= 8 && (&head[4..8] == b"idsc" || &head[4..8] == b"idat")
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["qtif", "qif"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let q = qtif(source.bytes())
            .ok_or_else(|| DecodeError::Malformed("no image description and data".into()))?;
        let name = String::from_utf8_lossy(&q.codec).trim().to_string();
        match &q.codec {
            b"jpeg" | b"png " | b"tiff" | b"gif " | b"WRLE" => {
                let image = Source::from_slice("qtif-image", source.clone(), q.data)
                    .ok_or_else(|| DecodeError::Malformed("QuickTime image data".into()))?;
                crate::embedded_dec::open_embedded(
                    source.clone(),
                    image,
                    Format::Qtif,
                    "QuickTime image",
                )
            }
            b"raw " => {
                qt_raw_info(source.bytes()).map_err(map_err)?;
                open_page(Kind::QtRaw, source, 0, Side::default())
            }
            _ => Err(DecodeError::Unsupported(format!(
                "QuickTime images coded with \"{name}\""
            ))),
        }
    }
}

/// A raw (`raw `) QuickTime image's facts: 24-bit RGB or 32-bit ARGB.
fn qt_raw_info(d: &[u8]) -> rt::Result<rt::Info> {
    let q = qtif(d).ok_or(rt::Error::NotRecognised)?;
    let (w, h) = (q.width, q.height);
    if w == 0 || h == 0 || w > rt::MAX_DIMENSION || h > rt::MAX_DIMENSION {
        return Err(rt::Error::Malformed("QuickTime image size".into()));
    }
    if !matches!(q.depth, 24 | 32) {
        return Err(rt::Error::Unsupported(format!(
            "QuickTime raw images of depth {}",
            q.depth
        )));
    }
    Ok(rt::Info {
        width: w,
        height: h,
        pixel_format: rt::PixelFormat::Rgba8,
        has_alpha: false,
        model: rt::Model::Rgb,
        bit_depth: 8,
        coding: format!("QuickTime image, raw {}-bit RGB", q.depth),
        pages: 1,
    })
}

fn qt_raw(d: &[u8]) -> rt::Result<rt::Image> {
    let i = qt_raw_info(d)?;
    let q = qtif(d).ok_or(rt::Error::NotRecognised)?;
    let (w, h) = (i.width, i.height);
    let bpp = if q.depth == 32 { 4 } else { 3 };
    // Rows padded to even lengths in some writers: use the stride that fits.
    let row = w as usize * bpp;
    let stride = if q.data.len() >= (row + row % 2) * h as usize {
        row + row % 2
    } else {
        row
    };
    if (q.data.len() as u64) < u64::from(h) * row as u64 / 2 {
        return Err(rt::Error::Truncated("QuickTime image data".into()));
    }
    let mut img = rt::Image::filled(w, h, rt::PixelFormat::Rgba8, 255);
    for y in 0..h as usize {
        let Some(src) = q.data.get(y * stride..y * stride + row) else {
            img.truncated = true;
            break;
        };
        for (o, p) in img.data[y * w as usize * 4..(y + 1) * w as usize * 4]
            .chunks_exact_mut(4)
            .zip(src.chunks_exact(bpp))
        {
            // 32-bit is ARGB with an unused alpha byte.
            o[..3].copy_from_slice(if bpp == 4 { &p[1..4] } else { &p[..3] });
        }
    }
    Ok(img)
}
