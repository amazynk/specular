//! FLIF (Free Lossless Image Format, the FLIF16 bitstream) decoder for Specular.
//!
//! Written from the FLIF16 specification, with the reference encoder's behaviour where the
//! specification is incomplete (range coder, chance updates, the default channel order,
//! the colour bucket and palette encodings). Bit-exact with the reference decoder (`flif
//! -d`) on the corpus fetched by `tools/corpus/fetch-editors-flif.sh`.
//!
//! | | Supported |
//! |---|---|
//! | Channels | grey, RGB, RGBA; 1 to 16 bits per channel |
//! | Methods | non-interlaced and interlaced (progressive; downscaled decodes stop early) |
//! | Transforms | ChannelCompact, YCoCg, PermutePlanes (with subtract), Bounds, PaletteAlpha, Palette, ColorBuckets, DuplicateFrame, FrameShape, FrameLookback |
//! | Pixels | MANIAC trees, all three interlaced predictors (fixed or per zoomlevel), invisible pixels of `alpha_zero` images |
//! | Animation | frames with their delays and the loop count |
//! | Metadata | `iCCP`, `eXif`, `eXmp` chunks (DEFLATE) |
//!
//! Output: 8-bit grey or RGBA when every channel fits 8 bits, else 16-bit (native-endian
//! `u16`). Samples are scaled to the full range of the output depth, so a 10-bit image's
//! 1023 is 65535 (as a PNM viewer shows the reference decoder's output); 8- and 16-bit
//! files are unchanged. RGB images get an opaque alpha.
//!
//! A truncated interlaced file decodes to what is there, the rest interpolated exactly as
//! the reference does, with [`Image::truncated`] set; a truncated non-interlaced file keeps
//! the rows it has (later rows are garbage, as in the reference). A FLIF archive (a Unix
//! `ar` archive with an `__image.flif` member, which the reference decoder reads) is
//! unwrapped ([`unwrap_archive`]).
//!
//! Refused, with a reason, and kept so because nothing defines them or writes them (the
//! reference decoder refuses them too): files of later FLIF versions (none was published;
//! FLIF16 is the last), custom initial chances ("not yet implemented" in the reference and
//! undefined in the specification), and transforms FLIF16 does not define.
//!
//! Robustness: no input panics; allocations are bounded by [`MAX_PIXELS`], [`MAX_FRAMES`]
//! and the memory budget.

mod decode;
mod image;
mod maniac;
mod rac;
mod ranges;
mod transform;

use decode::{Dec, End};
use image::{Img, Plane, Store};
use rac::{ChanceTable, Rac};
use ranges::{Layer, Ranges};

/// Largest frame pixel count accepted (512 MP).
pub const MAX_PIXELS: u64 = 1 << 29;
/// Most frames accepted (the reference decoder's limit).
pub const MAX_FRAMES: u32 = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a FLIF file.
    NotFlif,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// The data needed to reach the pixels is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions or frame count over the caps.
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    NeedsMemory(specular_core::NeedsMemory),
    /// [`Options::cancel`] returned true.
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotFlif => write!(f, "not a FLIF file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

/// What the headers say, without decoding pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// 1 (grey), 3 (RGB) or 4 (RGBA).
    pub channels: u8,
    /// Output depth: 8 or 16.
    pub bit_depth: u8,
    /// Bits of the widest channel as stored (1 to 16).
    pub significant_bits: u8,
    pub frames: u32,
    pub interlaced: bool,
    /// Times to play; 0 plays forever.
    pub loop_count: u32,
    pub icc: Option<Vec<u8>>,
    /// The chunk as stored: "Exif\0\0" and the TIFF structure.
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    Rgba8,
    Gray16,
    Rgba16,
}

impl PixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Rgba8 => 4,
            PixelFormat::Gray16 => 2,
            PixelFormat::Rgba16 => 8,
        }
    }
}

/// One frame: the whole canvas, interleaved samples.
#[derive(Debug, Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub delay_ms: u32,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub frames: Vec<Frame>,
    /// The data ended early; the missing detail was interpolated.
    pub truncated: bool,
}

pub struct Options<'a> {
    /// Polled once per row; true stops the decode with [`Error::Cancelled`].
    pub cancel: Option<&'a (dyn Fn() -> bool + Sync)>,
    /// 1 for full size; 2, 4, 8… make an interlaced file stop once the image is complete at
    /// `ceil(w / scale)` × `ceil(h / scale)` and return that (the scale rounds down to a
    /// power of two). Non-interlaced files ignore it.
    pub scale: u32,
    /// Only the first frame of an animation is returned (all are decoded: FLIF interleaves
    /// frames row by row).
    pub first_frame_only: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            cancel: None,
            scale: 1,
            first_frame_only: false,
        }
    }
}

/// Whether `head` starts like a FLIF file.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"FLIF")
        && head.get(4).is_none_or(|&c| {
            matches!(
                c,
                b'1' | b'3' | b'4' | b'A' | b'C' | b'D' | b'Q' | b'S' | b'T' | b'a' | b'c' | b'd'
            )
        })
}

/// Whether `data` is a FLIF file, or a FLIF archive (see [`unwrap_archive`]) holding one.
pub fn probe_file(data: &[u8]) -> bool {
    match unwrap_archive(data) {
        Ok(d) => probe(d),
        Err(_) => false,
    }
}

/// The image of a FLIF archive: a Unix `ar` archive (`!<arch>\n`) whose member named
/// `__image.flif` is the picture, as the reference decoder reads it (members before it are
/// skipped, each padded to an even length). Anything else is returned as it is.
pub fn unwrap_archive(data: &[u8]) -> Result<&[u8], Error> {
    if !data.starts_with(b"!<ar") {
        return Ok(data);
    }
    if data.get(4..8) != Some(b"ch>\n") {
        return Err(Error::Malformed("FLIF archive header is not valid".into()));
    }
    let mut pos = 8usize;
    loop {
        let Some(h) = data.get(pos..pos + 60) else {
            return Err(Error::Malformed("FLIF archive holds no image".into()));
        };
        let size = std::str::from_utf8(&h[48..58])
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .ok_or_else(|| Error::Malformed("FLIF archive member size is not valid".into()))?;
        pos += 60;
        if h.starts_with(b"__image.flif/") {
            let end = (pos as u64).saturating_add(size).min(data.len() as u64) as usize;
            return Ok(&data[pos..end]);
        }
        pos = (pos as u64)
            .saturating_add(size + (size & 1))
            .min(data.len() as u64) as usize;
    }
}

struct Header {
    width: usize,
    height: usize,
    channels: usize,
    interlaced: bool,
    frames: usize,
    bpc: u8,
    icc: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
    xmp: Option<Vec<u8>>,
    /// Offset of the range-coded data.
    body: usize,
}

fn varint(data: &[u8], pos: &mut usize) -> Result<u64, Error> {
    let mut v: u64 = 0;
    for _ in 0..10 {
        let Some(&b) = data.get(*pos) else {
            return Err(Error::Truncated("FLIF header cut short".into()));
        };
        *pos += 1;
        if b < 128 {
            return Ok(v + u64::from(b));
        }
        v = (v + u64::from(b - 128)) << 7;
        if v > 1 << 40 {
            break;
        }
    }
    Err(Error::Malformed("FLIF header has an invalid number".into()))
}

const MAX_CHUNK: u64 = 256 << 20;

fn header(data: &[u8], metadata: bool) -> Result<Header, Error> {
    if data.starts_with(b"!<ar") {
        return Err(Error::Malformed(
            "FLIF archive inside a FLIF archive".into(),
        ));
    }
    if !data.starts_with(b"FLIF") {
        return Err(Error::NotFlif);
    }
    let Some(&c) = data.get(4) else {
        return Err(Error::Truncated("FLIF header cut short".into()));
    };
    if !(b' '..=b' ' + 32 + 15 + 32).contains(&c) {
        return Err(Error::Malformed("FLIF format byte is not valid".into()));
    }
    let mut c = c - b' ';
    let mut frames = 1;
    if c > 47 {
        c -= 32;
        frames = 2;
    }
    let interlaced = match c / 16 {
        1 => false,
        2 => true,
        _ => return Err(Error::Malformed("FLIF encoding method is not valid".into())),
    };
    let channels = (c % 16) as usize;
    if !matches!(channels, 1 | 3 | 4) {
        return Err(Error::Unsupported(format!("FLIF with {channels} channels")));
    }
    let Some(&bpc) = data.get(5) else {
        return Err(Error::Truncated("FLIF header cut short".into()));
    };
    if !(b'0'..=b'2').contains(&bpc) {
        return Err(Error::Malformed("FLIF colour depth is not valid".into()));
    }
    let mut pos = 6;
    let width = varint(data, &mut pos)? + 1;
    let height = varint(data, &mut pos)? + 1;
    if frames > 1 {
        frames = varint(data, &mut pos)? + 2;
    }
    if width > u64::from(u32::MAX) || height > u64::from(u32::MAX) || width * height > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    if frames > u64::from(MAX_FRAMES) {
        return Err(Error::TooLarge);
    }
    let (mut icc, mut exif, mut xmp) = (None, None, None);
    loop {
        let Some(&n0) = data.get(pos) else {
            return Err(Error::Truncated("FLIF header cut short".into()));
        };
        pos += 1;
        if n0 < 32 {
            if n0 > 0 {
                // No later version was ever defined (FLIF16 is the last).
                return Err(Error::Unsupported(
                    "a FLIF version newer than FLIF16".into(),
                ));
            }
            break;
        }
        let Some(name) = data.get(pos - 1..pos + 3) else {
            return Err(Error::Truncated("FLIF header cut short".into()));
        };
        let name: [u8; 4] = name.try_into().unwrap_or([0; 4]);
        pos += 3;
        let known = matches!(&name, b"iCCP" | b"eXif" | b"eXmp");
        if !known && name[0].is_ascii_uppercase() {
            return Err(Error::Unsupported(format!(
                "FLIF chunk {} that this decoder does not know",
                String::from_utf8_lossy(&name)
            )));
        }
        let len = varint(data, &mut pos)?;
        if len > MAX_CHUNK || pos as u64 + len > data.len() as u64 {
            return Err(Error::Truncated("FLIF metadata chunk cut short".into()));
        }
        let body = &data[pos..pos + len as usize];
        pos += len as usize;
        if metadata && known {
            // Unreadable metadata is left out, not fatal.
            if let Ok(v) = specular_png::inflate_raw(body, MAX_CHUNK as usize) {
                match &name {
                    b"iCCP" => icc = Some(v),
                    b"eXif" => exif = Some(v),
                    _ => xmp = Some(v),
                }
            }
        }
    }
    Ok(Header {
        width: width as usize,
        height: height as usize,
        channels,
        interlaced,
        frames: frames as usize,
        bpc,
        icc,
        exif,
        xmp,
        body: pos,
    })
}

/// The start of the second header: per-channel maxima, alpha_zero, loops, delays.
struct Second {
    maxval: i32,
    sig_bits: u8,
    alpha_zero: bool,
    loops: u32,
    delays: Vec<u32>,
}

fn second(rac: &mut Rac, h: &Header) -> Second {
    let mut maxmax = 0;
    for _ in 0..h.channels {
        let max = match h.bpc {
            b'2' => 65535,
            b'1' => 255,
            _ => (1 << rac.uniform(1, 15)) - 1,
        };
        maxmax = std::cmp::max(maxmax, max);
    }
    let alpha_zero = h.channels > 3 && rac.uniform(0, 1) != 0;
    let mut loops = 0;
    let mut delays = vec![0; h.frames];
    if h.frames > 1 {
        loops = rac.uniform(0, 100) as u32;
        for d in delays.iter_mut() {
            *d = rac.uniform(0, 60000) as u32;
        }
    }
    let sig_bits = (32 - (maxmax as u32).leading_zeros()) as u8;
    Second {
        maxval: maxmax,
        sig_bits,
        alpha_zero,
        loops,
        delays,
    }
}

/// The headers and metadata, without decoding pixels.
pub fn read_info(data: &[u8]) -> Result<Info, Error> {
    let data = unwrap_archive(data)?;
    let h = header(data, true)?;
    let mut rac = Rac::new(&data[h.body..]);
    let s = second(&mut rac, &h);
    if rac.eof() {
        return Err(Error::Truncated("FLIF header cut short".into()));
    }
    Ok(Info {
        width: h.width as u32,
        height: h.height as u32,
        channels: h.channels as u8,
        bit_depth: if s.maxval > 255 { 16 } else { 8 },
        significant_bits: s.sig_bits,
        frames: h.frames as u32,
        interlaced: h.interlaced,
        loop_count: s.loops,
        icc: h.icc,
        exif: h.exif,
        xmp: h.xmp,
    })
}

/// Decodes the image (every frame of an animation).
pub fn decode(data: &[u8], opts: &Options) -> Result<Image, Error> {
    let data = unwrap_archive(data)?;
    let h = header(data, false)?;
    let mut rac = Rac::new(&data[h.body..]);
    let s = second(&mut rac, &h);
    let mut cut = 2u32;
    let mut divisor = 19u32;
    if rac.uniform(0, 1) != 0 {
        cut = rac.uniform(1, 127) as u32;
        divisor = rac.uniform(2, 126) as u32;
        if rac.uniform(0, 1) != 0 {
            // FLIF16 leaves this undefined: the reference decoder refuses it too and no
            // encoder writes it.
            return Err(Error::Unsupported(
                "FLIF with custom initial chances (undefined in FLIF16)".into(),
            ));
        }
    }
    if rac.eof() {
        return Err(Error::Truncated("FLIF header cut short".into()));
    }
    let (w, ht, nf) = (h.width, h.height, h.frames);
    let n = w * ht;
    // Every frame's planes (up to 5, 4 bytes a sample), then the output.
    let planes_needed = h.channels as u64 + 2;
    let _reserve = specular_core::budget::reserve_n(
        n as u64 * nf as u64,
        planes_needed * 4 + if s.maxval > 255 { 8 } else { 4 },
    )?;
    let small = s.maxval < 256;
    let stores = if small {
        [Store::U8, Store::I16, Store::I16, Store::U8]
    } else {
        [Store::U16, Store::I32, Store::I32, Store::U16]
    };
    let mut frames: Vec<Img> = (0..nf)
        .map(|_| Img {
            w,
            h: ht,
            planes: (0..h.channels)
                .map(|p| Plane::constant(0, stores[p]))
                .collect(),
            col_begin: vec![0; ht],
            col_end: vec![w as u32; ht],
            seen_before: -1,
            palette: false,
        })
        .collect();
    let mut ranges = Ranges {
        layers: vec![Layer::Static(vec![(0, s.maxval); h.channels])],
    };
    let default_table = rac::default_table();
    let mut alpha_zero = s.alpha_zero;
    let mut undos = Vec::new();
    let mut last = -1;
    while rac.read_bit() {
        if rac.eof() {
            return Err(Error::Truncated("FLIF header cut short".into()));
        }
        let id = rac.uniform(0, 13);
        if id <= last {
            return Err(Error::Malformed(format!(
                "FLIF {} transform out of order",
                transform::name(id)
            )));
        }
        last = id;
        let mut ctx = transform::Ctx {
            frames: &mut frames,
            alpha_zero: &mut alpha_zero,
        };
        undos.push(transform::load(
            id,
            &mut rac,
            &default_table,
            &mut ranges,
            &mut ctx,
        )?);
    }
    let nump = ranges.num_planes();
    let mut inv_pred = 0;
    if s.alpha_zero && nump > 3 && ranges.min(3) <= 0 && h.interlaced {
        inv_pred = rac.uniform(0, 2) as usize;
    }
    let mut real = 0u64;
    for p in 0..nump {
        let trivial = ranges.min(p) >= ranges.max(p);
        if !trivial {
            real += 1;
        }
        let mbits = if trivial {
            0
        } else {
            let d = (ranges.max(p) as i64 - ranges.min(p) as i64) * 2 - 1;
            64 - (d as u64).leading_zeros()
        };
        if mbits > 18 {
            return Err(Error::Malformed("FLIF channel range is too wide".into()));
        }
        for f in frames.iter_mut() {
            if p >= f.planes.len() {
                continue;
            }
            let pl = &mut f.planes[p];
            if trivial && p < 4 {
                pl.constant = Some(ranges.min(p));
                pl.data = Vec::new();
            } else if pl.constant.is_some() {
                pl.constant = None;
                pl.data = vec![0; n];
            }
            if p == 0 {
                pl.make_real(n);
            }
        }
    }
    if nump > 3 && (ranges.min(3) > 0 || ranges.max(3) == 0) {
        alpha_zero = false;
    }
    let mut todo = n as u64 * real;
    let mut done0 = 0;
    if todo == 0 {
        todo = 1;
        done0 = 1;
    }
    let scale_shift = if h.interlaced && opts.scale > 1 {
        (31 - opts.scale.leading_zeros()) as usize
    } else {
        0
    };
    let zooms = frames[0].zooms();
    let stop_zl = (2 * scale_shift).min(zooms & !1);
    let mut dec = Dec {
        rac: &mut rac,
        table: ChanceTable::new(cut, 0xFFFF_FFFF / divisor),
        ranges: &ranges,
        frames: &mut frames,
        alpha_zero,
        inv_pred,
        undos: &mut undos,
        stop_zl,
        cancel: opts.cancel,
        pixels_done: done0,
        pixels_todo: todo,
        max_tree_nodes: (n as u64 * nf as u64 * 2 + 65536).min(1 << 24) as usize,
    };
    let end = if h.interlaced {
        dec.interlaced()?
    } else {
        let forest = dec.read_trees(false)?;
        dec.scanlines(&forest)?
    };
    let has_crc = end == End::Complete && rac.uniform(0, 1) != 0;
    let truncated = end == End::Truncated;
    // A downscaled decode keeps the pixels of its zoomlevel.
    if stop_zl > 0 {
        let rs = image::rowps(stop_zl);
        let cs = image::colps(stop_zl);
        for f in frames.iter_mut() {
            let (nw, nh) = (f.cols(stop_zl), f.rows(stop_zl));
            for pl in f.planes.iter_mut() {
                if pl.constant.is_none() {
                    let mut d = Vec::with_capacity(nw * nh);
                    for r in 0..nh {
                        for c in 0..nw {
                            d.push(pl.data[r * rs * f.w + c * cs]);
                        }
                    }
                    pl.data = d;
                }
            }
            f.w = nw;
            f.h = nh;
        }
    }
    while let Some(u) = undos.pop() {
        transform::undo(&u, &mut frames);
    }
    let black_invisible = s.alpha_zero && (has_crc || end == End::Scaled);
    Ok(output(
        frames,
        &h,
        &s,
        black_invisible,
        truncated,
        opts.first_frame_only,
    ))
}

fn output(
    frames: Vec<Img>,
    h: &Header,
    s: &Second,
    black_invisible: bool,
    truncated: bool,
    first_only: bool,
) -> Image {
    let (w, ht) = (frames[0].w, frames[0].h);
    let wide = s.maxval > 255;
    let full: u32 = if wide { 65535 } else { 255 };
    let maxval = s.maxval.max(1) as u32;
    let scale = |v: i32| -> u32 {
        let v = v.clamp(0, maxval as i32) as u32;
        if maxval == full {
            v
        } else {
            (v * full + maxval / 2) / maxval
        }
    };
    let grey = h.channels == 1;
    let format = match (grey, wide) {
        (true, false) => PixelFormat::Gray8,
        (false, false) => PixelFormat::Rgba8,
        (true, true) => PixelFormat::Gray16,
        (false, true) => PixelFormat::Rgba16,
    };
    let count = if first_only { 1 } else { frames.len() };
    let mut out = Vec::with_capacity(count);
    for (fi, f) in frames.into_iter().enumerate().take(count) {
        let n = w * ht;
        let mut data = Vec::with_capacity(n * format.bytes_per_pixel());
        let np = f.planes.len();
        for i in 0..n {
            let mut px = [0u32; 4];
            if grey {
                px[0] = scale(f.at(0, i));
            } else {
                for (c, v) in px.iter_mut().enumerate().take(3) {
                    *v = scale(if c < np { f.at(c, i) } else { 0 });
                }
                px[3] = if np > 3 { scale(f.at(3, i)) } else { full };
                if black_invisible && px[3] == 0 {
                    px[..3].fill(0);
                }
            }
            let k = if grey { 1 } else { 4 };
            for &v in &px[..k] {
                if wide {
                    data.extend_from_slice(&(v as u16).to_ne_bytes());
                } else {
                    data.push(v as u8);
                }
            }
        }
        out.push(Frame {
            data,
            delay_ms: s.delays.get(fi).copied().unwrap_or(0),
        });
    }
    Image {
        width: w as u32,
        height: ht as u32,
        format,
        frames: out,
        truncated,
    }
}
