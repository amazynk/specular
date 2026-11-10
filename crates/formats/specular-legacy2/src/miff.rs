//! ImageMagick MIFF (Magick Image File Format).
//!
//! A file is one or more images, each a text header of `key=value` pairs (values may be
//! `{braced}` or quoted; a top-level `{…}` is a comment) ended by `:` and Ctrl-Z (0x1A), then
//! the binary part: the profiles named by `profile:*` keys (their sizes, in key order), the
//! colour map of a PseudoClass image (`colors` entries of red, green and blue at the image
//! depth, most significant byte first) and the pixels, row by row.
//!
//! A pixel is one quantum per channel at `depth` bits (8, 16, 32; 16/32/64-bit IEEE floats in
//! 0..1 with `quantum:format=floating-point`), in `endian` order (MSB unless the header says
//! LSB): the colour index (PseudoClass) or grey, red-green-blue or cyan-magenta-yellow-black,
//! then alpha. Compression:
//! - `None`;
//! - `RLE`: packets of one pixel and a byte giving the run length minus one;
//! - `Zip` and `BZip`: one zlib or bzip2 stream; since file version 1 it is cut into chunks
//!   each preceded by its 32-bit big-endian length (ImageMagick writes one per row and flushes
//!   the stream at row ends).
//!
//! LZMA, JPEG and the fax/LZW methods are refused, as are colour spaces other than sRGB, linear
//! RGB, grey and CMYK, and images with meta channels. Integer data is shown display-coded (linear
//! RGB and linear grey are encoded with the sRGB curve, CMYK converted without a profile as
//! `R = (1 − C)(1 − K)`); float data is returned as linear light. Before ImageMagick 7 the
//! colour space "RGB" meant sRGB; files without the version 7 keys are read that way.

use crate::colorspace::Cs;
use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload};
use rayon::prelude::*;

/// Longest header read.
const MAX_HEADER: usize = 1 << 20;
/// Most images walked in one file.
const MAX_PAGES: u32 = 100_000;

fn skip_space_and_comments(d: &[u8], mut p: usize) -> usize {
    loop {
        while p < d.len() && d[p].is_ascii_whitespace() {
            p += 1;
        }
        if d.get(p) == Some(&b'{') {
            match d[p..].iter().position(|&c| c == b'}') {
                Some(e) => p += e + 1,
                None => return d.len(),
            }
        } else {
            return p;
        }
    }
}

fn starts_image(d: &[u8], at: usize) -> bool {
    let p = skip_space_and_comments(d, at);
    d[p.min(d.len())..].starts_with(b"id=ImageMagick")
}

/// `id=ImageMagick` first, after any white space or `{comments}`.
pub fn probe(head: &[u8]) -> bool {
    starts_image(head, 0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compression {
    None,
    Rle,
    Zip,
    BZip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Space {
    Srgb,
    LinearRgb,
    Gray,
    LinearGray,
    Cmyk,
    /// Another colour space ImageMagick writes, converted to sRGB.
    Other(Cs),
}

struct Desc {
    keys: Vec<(String, String)>,
    comment: Option<String>,
    pseudo: bool,
    colors: usize,
    alpha: bool,
    width: u32,
    height: u32,
    depth: u32,
    float: bool,
    le: bool,
    space: Space,
    compression: Compression,
    /// Meta channels after the colour and alpha channels (not shown).
    meta: usize,
    /// Chunked compressed streams (`version` ≥ 1).
    chunked: bool,
    profiles: usize,
    /// Where the binary part starts.
    data: usize,
}

impl Desc {
    fn get(&self, k: &str) -> Option<&str> {
        self.keys
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_str())
    }
    fn channels(&self) -> usize {
        let base = if self.pseudo {
            1
        } else {
            match self.space {
                Space::Gray | Space::LinearGray => 1,
                Space::Cmyk => 4,
                _ => 3,
            }
        };
        base + self.alpha as usize + self.meta
    }
    fn bytes(&self) -> usize {
        self.depth as usize / 8
    }
    fn packet(&self) -> usize {
        self.channels() * self.bytes()
    }
    fn row_bytes(&self) -> usize {
        self.packet() * self.width as usize
    }
    fn colormap_bytes(&self) -> usize {
        if self.pseudo {
            self.colors * 3 * self.bytes()
        } else {
            0
        }
    }
}

/// A header: its (key, value) pairs, its comment and where the binary part starts.
type Header = (Vec<(String, String)>, Option<String>, usize);

/// Reads the header at `at`.
fn read_header(d: &[u8], at: usize) -> Result<Header> {
    let end = d.len().min(at.saturating_add(MAX_HEADER));
    let mut p = at;
    let mut keys = Vec::new();
    let mut comment: Option<String> = None;
    loop {
        while p < end && d[p].is_ascii_whitespace() {
            p += 1;
        }
        if p >= end {
            return Err(Error::Truncated("header without its end mark".into()));
        }
        match d[p] {
            b':' => {
                p += 1;
                if d.get(p) == Some(&0x1a) {
                    p += 1;
                }
                return Ok((keys, comment, p));
            }
            b'{' => {
                let e = d[p..end]
                    .iter()
                    .position(|&c| c == b'}')
                    .ok_or_else(|| Error::Truncated("comment".into()))?;
                let c = String::from_utf8_lossy(&d[p + 1..p + e]).trim().to_string();
                if !c.is_empty() {
                    comment = Some(match comment {
                        Some(o) => o + "\n" + &c,
                        None => c,
                    });
                }
                p += e + 1;
            }
            _ => {
                let k0 = p;
                while p < end && d[p] != b'=' && !d[p].is_ascii_whitespace() {
                    p += 1;
                }
                let key = String::from_utf8_lossy(&d[k0..p]).to_string();
                if d.get(p) != Some(&b'=') {
                    continue;
                }
                p += 1;
                let value = match d.get(p) {
                    Some(&b'{') => {
                        let mut depth = 0;
                        let v0 = p + 1;
                        while p < end {
                            match d[p] {
                                b'{' => depth += 1,
                                b'}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            p += 1;
                        }
                        let v = String::from_utf8_lossy(&d[v0..p.min(end)]).to_string();
                        p += 1;
                        v
                    }
                    Some(&q) if q == b'"' || q == b'\'' => {
                        let v0 = p + 1;
                        p += 1;
                        while p < end && d[p] != q {
                            p += 1;
                        }
                        let v = String::from_utf8_lossy(&d[v0..p.min(end)]).to_string();
                        p += 1;
                        v
                    }
                    _ => {
                        let v0 = p;
                        while p < end && !d[p].is_ascii_whitespace() {
                            p += 1;
                        }
                        String::from_utf8_lossy(&d[v0..p]).to_string()
                    }
                };
                keys.push((key, value));
            }
        }
    }
}

fn describe(d: &[u8], at: usize) -> Result<Desc> {
    let start = skip_space_and_comments(d, at);
    if !d[start.min(d.len())..].starts_with(b"id=ImageMagick") {
        return Err(Error::NotRecognised);
    }
    let (keys, comment, data) = read_header(d, at)?;
    let get = |k: &str| {
        keys.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.trim())
    };
    let num = |k: &str| get(k).and_then(|v| v.parse::<u64>().ok());
    let im7 = get("alpha-trait").is_some() || get("number-channels").is_some();
    let width = num("columns").ok_or_else(|| Error::Malformed("no columns".into()))?;
    let height = num("rows").ok_or_else(|| Error::Malformed("no rows".into()))?;
    check_dims(width, height)?;
    let float = get("quantum:format").is_some_and(|v| v.eq_ignore_ascii_case("floating-point"));
    let depth = num("depth").unwrap_or(8);
    let depth = match (depth, float) {
        (1..=8, false) => 8,
        (9..=16, _) => 16,
        (17..=32, _) => 32,
        (33..=64, _) => 64,
        (d, _) => {
            return Err(Error::Unsupported(format!(
                "MIFF images with {d}-bit {} are not supported",
                if float { "floats" } else { "integers" }
            )));
        }
    };
    let pseudo = get("class").is_some_and(|v| v.eq_ignore_ascii_case("PseudoClass"));
    let colors = num("colors").unwrap_or(0);
    if pseudo && (colors == 0 || colors > 1 << 24) {
        return Err(Error::Malformed(format!(
            "palette image with {colors} colours"
        )));
    }
    if pseudo && float {
        return Err(Error::Unsupported(
            "MIFF palette images with floating-point samples are not supported".into(),
        ));
    }
    let alpha = match get("alpha-trait") {
        Some(v) => !v.eq_ignore_ascii_case("Undefined"),
        None => get("matte").is_some_and(|v| v.eq_ignore_ascii_case("True")),
    };
    let cs = get("colorspace").unwrap_or("sRGB");
    let space = match cs.to_ascii_lowercase().as_str() {
        "srgb" | "undefined" => Space::Srgb,
        "rgb" if im7 => Space::LinearRgb,
        "rgb" => Space::Srgb,
        "gray" => Space::Gray,
        "lineargray" => Space::LinearGray,
        "cmyk" => Space::Cmyk,
        other => match Cs::parse(other) {
            Some(Cs::ScRgb) => Space::LinearRgb,
            Some(c) => Space::Other(c),
            None => {
                return Err(Error::Unsupported(format!(
                    "MIFF images in the {cs} colour space are not supported"
                )));
            }
        },
    };
    let meta = num("number-meta-channels").unwrap_or(0);
    if meta > 64 {
        return Err(Error::Malformed(format!("{meta} meta channels")));
    }
    let meta = meta as usize;
    let comp = get("compression").unwrap_or("None");
    let compression = match comp.to_ascii_lowercase().as_str() {
        "none" | "undefined" => Compression::None,
        "rle" | "runlengthencoded" => Compression::Rle,
        "zip" => Compression::Zip,
        "bzip" => Compression::BZip,
        _ => {
            return Err(Error::Unsupported(format!(
                "MIFF images compressed with {comp} are not supported"
            )));
        }
    };
    let le = get("endian").is_some_and(|v| v.eq_ignore_ascii_case("LSB"));
    let chunked = get("version")
        .and_then(|v| v.parse::<f64>().ok())
        .is_some_and(|v| v != 0.0);
    let mut profiles = 0usize;
    for (k, v) in &keys {
        let k = k.to_ascii_lowercase();
        if k.starts_with("profile:") || k.starts_with("profile-") {
            profiles = profiles.saturating_add(v.trim().parse::<usize>().unwrap_or(0));
        }
    }
    Ok(Desc {
        keys,
        comment,
        pseudo,
        colors: colors as usize,
        alpha,
        width: width as u32,
        height: height as u32,
        depth: depth as u32,
        float,
        le,
        space,
        compression,
        meta,
        chunked,
        profiles,
        data,
    })
}

/// The chunks of a chunked compressed stream from `at`: their slices and where they end. A
/// chunk is taken while its length fits the file and the bytes there do not start an image.
fn chunks(d: &[u8], mut at: usize) -> (Vec<&[u8]>, usize) {
    let mut out = Vec::new();
    while at + 4 <= d.len() && !starts_image(d, at) {
        let n = u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]) as usize;
        if n > d.len() - at - 4 {
            break;
        }
        out.push(&d[at + 4..at + 4 + n]);
        at += 4 + n;
    }
    (out, at)
}

/// The start of the pixel stream.
fn pixel_start(desc: &Desc) -> usize {
    desc.data
        .saturating_add(desc.profiles)
        .saturating_add(desc.colormap_bytes())
}

/// Where the image's data ends (the next image's header may follow).
fn skip(d: &[u8], desc: &Desc) -> Result<usize> {
    let s = pixel_start(desc);
    let need = desc.row_bytes() as u64 * desc.height as u64;
    Ok(match desc.compression {
        Compression::None => s.saturating_add(need as usize).min(d.len()),
        Compression::Rle => rle(d, s, desc, None)?.1,
        Compression::Zip | Compression::BZip if desc.chunked => chunks(d, s.min(d.len())).1,
        // An unchunked stream: only the last image can be found after it.
        Compression::Zip => inflate_skip(d.get(s..).unwrap_or(&[]), need) + s,
        Compression::BZip => d.len(),
    })
}

/// Expands RLE packets from `at` into `out` (when given) until `out` (or the image) is full.
/// Returns (bytes written, end).
fn rle(d: &[u8], at: usize, desc: &Desc, out: Option<&mut [u8]>) -> Result<(usize, usize)> {
    let ps = desc.packet();
    let total = desc.row_bytes() * desc.height as usize;
    let mut p = at.min(d.len());
    let mut n = 0usize;
    let mut out = out;
    while n < total {
        let Some(pkt) = d.get(p..p + ps + 1) else {
            break;
        };
        let run = (pkt[ps] as usize + 1) * ps;
        let take = run.min(total - n);
        if let Some(o) = out.as_deref_mut() {
            for c in o[n..n + take].chunks_mut(ps) {
                c.copy_from_slice(&pkt[..c.len()]);
            }
        }
        n += take;
        p += ps + 1;
    }
    Ok((n, p))
}

/// Inflates one zlib stream (no chunk lengths) into a small scratch buffer, only to find
/// where it ends: the input bytes used. (Inflating the whole image to skip it let a 200 KB
/// page header ask for gigabytes per page while listing the pages.)
fn inflate_skip(src: &[u8], need: u64) -> usize {
    let mut scratch = vec![0u8; 64 << 10];
    let mut inf = zlib_rs::Inflate::new(true, 15);
    let mut at = 0usize;
    while inf.total_out() < need && at < src.len() {
        let room = (need - inf.total_out()).min(scratch.len() as u64) as usize;
        let before = (inf.total_in(), inf.total_out());
        let r = inf.decompress(
            &src[at..],
            &mut scratch[..room],
            zlib_rs::InflateFlush::NoFlush,
        );
        at = inf.total_in() as usize;
        let stuck = (inf.total_in(), inf.total_out()) == before;
        if !matches!(r, Ok(zlib_rs::Status::Ok)) || stuck {
            break;
        }
    }
    inf.total_in() as usize
}

/// Inflates one zlib stream (no chunk lengths). Returns (output, input bytes used).
fn inflate_whole(src: &[u8], need: usize) -> Result<(Vec<u8>, usize)> {
    let mut out = vec![0u8; need];
    let mut inf = zlib_rs::Inflate::new(true, 15);
    let _ = inf.decompress(src, &mut out, zlib_rs::InflateFlush::Finish);
    let n = inf.total_out() as usize;
    out.truncate(n);
    Ok((out, inf.total_in() as usize))
}

/// The raw pixel stream of the image (rows × row bytes), and whether it ended early.
fn raw(d: &[u8], desc: &Desc) -> Result<(Vec<u8>, bool)> {
    let s = pixel_start(desc).min(d.len());
    let need = desc.row_bytes() * desc.height as usize;
    let have = d.len() - s;
    let ratio = match desc.compression {
        Compression::None => 1,
        Compression::Rle => 256,
        Compression::Zip => 1100,
        // bzip2 has no useful bound (runs of runs: a few KiB can claim gigabytes): judged
        // below by what the stream decodes to instead.
        Compression::BZip => 0,
    };
    // The image, from the memory budget before it is allocated.
    let mut canvas = None;
    if ratio > 0 {
        check_payload(have, need as u64, ratio)?;
        canvas = Some(specular_core::budget::reserve(need as u64).map_err(Error::NeedsMemory)?);
    }
    let (mut out, n) = match desc.compression {
        Compression::None => {
            let mut out = vec![0x80u8; need];
            let n = need.min(have);
            out[..n].copy_from_slice(&d[s..s + n]);
            (out, n)
        }
        Compression::Rle => {
            let mut out = vec![0x80u8; need];
            let n = rle(d, s, desc, Some(&mut out))?.0;
            (out, n)
        }
        Compression::Zip if desc.chunked => {
            let (parts, _) = chunks(d, s);
            let mut out = vec![0u8; need];
            let mut inf = zlib_rs::Inflate::new(true, 15);
            for part in parts {
                let mut used = 0usize;
                while used < part.len() && (inf.total_out() as usize) < need {
                    let in0 = inf.total_in();
                    let o = inf.total_out() as usize;
                    let r = inf.decompress(
                        &part[used..],
                        &mut out[o..],
                        zlib_rs::InflateFlush::SyncFlush,
                    );
                    used += (inf.total_in() - in0) as usize;
                    match r {
                        Ok(zlib_rs::Status::StreamEnd) => break,
                        Ok(_) if inf.total_in() == in0 && inf.total_out() as usize == o => break,
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                if inf.total_out() as usize >= need {
                    break;
                }
            }
            let n = inf.total_out() as usize;
            (out, n)
        }
        Compression::Zip => {
            let (mut out, _) = inflate_whole(&d[s..], need)?;
            let n = out.len();
            out.resize(need, 0x80);
            (out, n)
        }
        Compression::BZip => {
            let stream: Vec<u8> = if desc.chunked {
                chunks(d, s).0.concat()
            } else {
                d[s..].to_vec()
            };
            // The output reserves from the budget as it grows.
            let mut out = match specular_xcf::bunzip2(&stream, need as u64) {
                Ok(out) => out,
                Err(specular_xcf::Error::NeedsMemory(n)) => return Err(Error::NeedsMemory(n)),
                Err(_) => Vec::new(),
            };
            let n = out.len();
            // Data for under 1/64 of the image is a header claiming what the file lacks.
            check_payload(n, need as u64, 64)?;
            canvas = Some(specular_core::budget::reserve(need as u64).map_err(Error::NeedsMemory)?);
            out.resize(need, 0x80);
            (out, n)
        }
    };
    if n < need {
        out[n..].fill(0x80);
    }
    drop(canvas);
    Ok((out, n < need))
}

/// The images of the file: each one's description (errors for images that cannot be read
/// stop the walk after that image).
fn walk(d: &[u8], upto: Option<u32>) -> Result<Vec<Result<Desc>>> {
    let mut v = Vec::new();
    let mut at = 0usize;
    loop {
        let desc = describe(d, at);
        let end = match &desc {
            Ok(x) => skip(d, x).ok(),
            Err(_) => None,
        };
        let stop = desc.is_err();
        if v.is_empty() && matches!(desc, Err(Error::NotRecognised)) {
            return Err(Error::NotRecognised);
        }
        v.push(desc);
        if stop || upto.is_some_and(|u| v.len() as u32 > u) || v.len() as u32 >= MAX_PAGES {
            break;
        }
        let Some(e) = end else { break };
        if e <= at || !starts_image(d, e) {
            break;
        }
        at = e;
    }
    Ok(v)
}

fn page(d: &[u8], page: u32) -> Result<(Desc, u32)> {
    let all = walk(d, None)?;
    let pages = all.len() as u32;
    let mut all = all;
    if page as usize >= all.len() {
        return Err(Error::Malformed(format!("no image {page}")));
    }
    let desc = all.swap_remove(page as usize)?;
    Ok((desc, pages))
}

fn orientation(v: &str) -> u8 {
    match v.to_ascii_lowercase().as_str() {
        "topright" => 2,
        "bottomright" => 3,
        "bottomleft" => 4,
        "lefttop" => 5,
        "righttop" => 6,
        "rightbottom" => 7,
        "leftbottom" => 8,
        _ => 1,
    }
}

pub fn info(data: &[u8], page_no: u32) -> Result<Info> {
    let (desc, pages) = page(data, page_no)?;
    let model = if desc.pseudo {
        Model::Palette
    } else {
        match desc.space {
            Space::Gray | Space::LinearGray => Model::Grey,
            _ => Model::Rgb,
        }
    };
    let comp = match desc.compression {
        Compression::None => "uncompressed",
        Compression::Rle => "RLE",
        Compression::Zip => "Zip",
        Compression::BZip => "BZip2",
    };
    let kind = if desc.pseudo {
        format!("palette of {}", desc.colors)
    } else {
        match desc.space {
            Space::Srgb => "RGB",
            Space::LinearRgb => "linear RGB",
            Space::Gray => "grey",
            Space::LinearGray => "linear grey",
            Space::Cmyk => "CMYK",
            Space::Other(c) => c.name(),
        }
        .to_string()
    };
    let coding = format!(
        "{}-bit {}{}{}, {}",
        desc.depth,
        if desc.float { "float " } else { "" },
        kind,
        if desc.alpha { " + alpha" } else { "" },
        comp
    );
    let mut summary = Vec::new();
    let comment = desc
        .comment
        .clone()
        .or_else(|| desc.get("comment").map(str::to_string));
    if let Some(c) = comment.filter(|c| !c.trim().is_empty()) {
        summary.push(("comment", c));
    }
    if let Some(c) = desc.get("date:create") {
        summary.push(("created", c.to_string()));
    }
    for k in ["software", "creator", "Software"] {
        if let Some(c) = desc.get(k) {
            summary.push(("creator", c.to_string()));
            break;
        }
    }
    if let Some(c) = desc.get("copyright") {
        summary.push(("copyright", c.to_string()));
    }
    if let Some(r) = desc.get("resolution") {
        let unit = match desc.get("units").map(|u| u.to_ascii_lowercase()) {
            Some(u) if u.contains("inch") => " dpi",
            Some(u) if u.contains("centimeter") => " per cm",
            _ => "",
        };
        summary.push(("resolution", format!("{r}{unit}")));
    }
    summary.push(("compression", comp.to_string()));
    let fields = desc
        .keys
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| {
            let kl = k.to_ascii_lowercase();
            if kl.starts_with("profile:") || kl.starts_with("profile-") {
                (k.clone(), format!("{} bytes", v.trim()))
            } else {
                (k.clone(), v.trim().to_string())
            }
        })
        .collect();
    Ok(Info {
        width: desc.width,
        height: desc.height,
        pages,
        bit_depth: desc.depth.min(255) as u8,
        model,
        has_alpha: desc.alpha,
        orientation: desc.get("orientation").map_or(1, orientation),
        coding,
        summary,
        fields,
    })
}

/// sRGB-coded → linear (extended: sign kept, values over 1 follow the power curve).
fn to_linear(v: f32) -> f32 {
    let a = v.abs();
    let l = if a <= 0.04045 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    };
    l.copysign(v)
}

/// Linear → sRGB-coded, for 0..=1.
fn to_srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Reads sample `i` of `b`-byte samples as an integer.
#[inline]
fn int_at(raw: &[u8], i: usize, b: usize, le: bool) -> u32 {
    let s = &raw[i * b..i * b + b];
    match (b, le) {
        (1, _) => s[0] as u32,
        (2, false) => u16::from_be_bytes([s[0], s[1]]) as u32,
        (2, true) => u16::from_le_bytes([s[0], s[1]]) as u32,
        // 64-bit integers: the top 32 bits.
        (8, false) => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
        (8, true) => u32::from_le_bytes([s[4], s[5], s[6], s[7]]),
        (_, false) => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
        (_, true) => u32::from_le_bytes([s[0], s[1], s[2], s[3]]),
    }
}

/// Reads sample `i` normalised to 0..1 (floats as stored).
#[inline]
fn norm_at(raw: &[u8], i: usize, b: usize, le: bool, float: bool) -> f32 {
    if !float {
        let max = match b {
            1 => 255.0,
            2 => 65535.0,
            _ => 4_294_967_295.0,
        };
        return (int_at(raw, i, b, le) as f64 / max) as f32;
    }
    match b {
        2 => half_to_f32(int_at(raw, i, 2, le) as u16),
        4 => f32::from_bits(int_at(raw, i, 4, le)),
        _ => {
            let s: [u8; 8] = raw[i * 8..i * 8 + 8].try_into().unwrap_or([0; 8]);
            (if le {
                f64::from_le_bytes(s)
            } else {
                f64::from_be_bytes(s)
            }) as f32
        }
    }
}

fn half_to_f32(h: u16) -> f32 {
    let s = (h >> 15) as u32;
    let e = ((h >> 10) & 31) as i32;
    let m = (h & 1023) as u32;
    let v = if e == 0 {
        m as f32 / 1024.0 * 2f32.powi(-14)
    } else if e == 31 {
        if m == 0 { f32::INFINITY } else { f32::NAN }
    } else {
        (1.0 + m as f32 / 1024.0) * 2f32.powi(e - 15)
    };
    if s == 1 { -v } else { v }
}

pub fn decode(data: &[u8], page_no: u32) -> Result<Image> {
    let (desc, _) = page(data, page_no)?;
    let (raw, truncated) = raw(data, &desc)?;
    let (w, h) = (desc.width as usize, desc.height as usize);
    let b = desc.bytes();
    let le = desc.le;
    let ch = desc.channels();
    let row = desc.row_bytes();
    let alpha = desc.alpha;
    let pixels = if desc.pseudo {
        // The colour map, most significant byte first, at the image depth.
        let cm_at = desc.data.saturating_add(desc.profiles);
        let cm = data.get(cm_at..).unwrap_or(&[]);
        let pal: Vec<[u16; 3]> = (0..desc.colors)
            .map(|i| {
                let get = |c: usize| {
                    let k = i * 3 + c;
                    if (k + 1) * b > cm.len() {
                        return 0;
                    }
                    let v = int_at(cm, k, b, false);
                    match b {
                        1 => (v * 257) as u16,
                        2 => v as u16,
                        _ => (v >> 16) as u16,
                    }
                };
                [get(0), get(1), get(2)]
            })
            .collect();
        let wide = b > 1;
        let mut out = vec![0u16; w * h * 4];
        out.par_chunks_mut(w * 4).enumerate().for_each(|(y, o)| {
            let r = &raw[y * row..(y + 1) * row];
            for (x, px) in o.chunks_exact_mut(4).enumerate() {
                let i = int_at(r, x * ch, b, le) as usize;
                let c = pal.get(i).copied().unwrap_or([0; 3]);
                let a = if alpha {
                    let v = int_at(r, x * ch + 1, b, le);
                    match b {
                        1 => (v * 257) as u16,
                        2 => v as u16,
                        _ => (v >> 16) as u16,
                    }
                } else {
                    65535
                };
                px.copy_from_slice(&[c[0], c[1], c[2], a]);
            }
        });
        if wide {
            Pixels::Rgba16(out)
        } else {
            Pixels::Rgba8(out.iter().map(|&v| (v >> 8) as u8).collect())
        }
    } else if desc.float {
        let linear_in = matches!(desc.space, Space::LinearRgb | Space::LinearGray);
        let mut out = vec![0f32; w * h * 4];
        out.par_chunks_mut(w * 4).enumerate().for_each(|(y, o)| {
            let r = &raw[y * row..(y + 1) * row];
            for (x, px) in o.chunks_exact_mut(4).enumerate() {
                let s = |c: usize| norm_at(r, x * ch + c, b, le, true);
                let (mut rgb, a) = colour(desc.space, alpha, s);
                if !linear_in {
                    rgb = rgb.map(to_linear);
                }
                px.copy_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
            }
        });
        Pixels::RgbaF32(out)
    } else if b == 1 && !alpha && desc.meta == 0 && desc.space == Space::Gray {
        Pixels::Gray8(raw)
    } else if b == 2 && !alpha && desc.meta == 0 && desc.space == Space::Gray {
        Pixels::Gray16(
            raw.chunks_exact(2)
                .map(|s| {
                    if le {
                        u16::from_le_bytes([s[0], s[1]])
                    } else {
                        u16::from_be_bytes([s[0], s[1]])
                    }
                })
                .collect(),
        )
    } else if b == 1 && desc.meta == 0 && desc.space == Space::Srgb {
        let mut out = vec![255u8; w * h * 4];
        out.par_chunks_mut(w * 4).enumerate().for_each(|(y, o)| {
            let r = &raw[y * row..(y + 1) * row];
            for (px, s) in o.chunks_exact_mut(4).zip(r.chunks_exact(ch)) {
                px[..ch].copy_from_slice(s);
            }
        });
        Pixels::Rgba8(out)
    } else {
        // Everything else through normalised values, kept at the stored precision.
        let wide = b > 1;
        let enc = matches!(desc.space, Space::LinearRgb | Space::LinearGray);
        let mut out = vec![0u16; w * h * 4];
        out.par_chunks_mut(w * 4).enumerate().for_each(|(y, o)| {
            let r = &raw[y * row..(y + 1) * row];
            for (x, px) in o.chunks_exact_mut(4).enumerate() {
                let s = |c: usize| norm_at(r, x * ch + c, b, le, false);
                let (mut rgb, a) = colour(desc.space, alpha, s);
                if enc {
                    rgb = rgb.map(to_srgb);
                }
                let q = |v: f32| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
                px.copy_from_slice(&[q(rgb[0]), q(rgb[1]), q(rgb[2]), q(a)]);
            }
        });
        if wide {
            Pixels::Rgba16(out)
        } else {
            Pixels::Rgba8(out.iter().map(|&v| crate::u16_to_u8(v)).collect())
        }
    };
    Ok(Image {
        width: desc.width,
        height: desc.height,
        pixels,
        truncated,
    })
}

/// A pixel's colour and alpha from its normalised samples `s(channel)`.
fn colour(space: Space, alpha: bool, s: impl Fn(usize) -> f32) -> ([f32; 3], f32) {
    match space {
        Space::Gray | Space::LinearGray => {
            let g = s(0);
            ([g, g, g], if alpha { s(1) } else { 1.0 })
        }
        Space::Cmyk => {
            let k = 1.0 - s(3);
            (
                [(1.0 - s(0)) * k, (1.0 - s(1)) * k, (1.0 - s(2)) * k],
                if alpha { s(4) } else { 1.0 },
            )
        }
        Space::Other(c) => (
            c.to_srgb([s(0), s(1), s(2)]),
            if alpha { s(3) } else { 1.0 },
        ),
        _ => ([s(0), s(1), s(2)], if alpha { s(3) } else { 1.0 }),
    }
}
