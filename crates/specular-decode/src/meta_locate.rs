//! Finding the XMP packet and the ICC profile of a file without decoding it, for the info
//! panel: PNG `iTXt` "XML:com.adobe.xmp" and `iCCP`; WebP `XMP ` and `ICCP` chunks; the HEIF/AVIF
//! `mime` item of type application/rdf+xml and the `colr` property (`prof` or `rICC`) of the
//! primary item; TIFF and BigTIFF (and TIFF-based RAW such as DNG) IFD0 tags 700 (XMP) and 34675 (ICC); the
//! JPEG XL `xml ` box; image resources 1060 (XMP) and 1039 (ICC) of a PSD/PSB; the XMP `uuid`
//! box of a Canon CR3; the IPTC-IIM block of a TIFF (tag 33723, else inside its Photoshop
//! resources, tag 34377) and of a PSD/PSB (image resource 1028). A HEIF XMP item stored with a `content_encoding` (deflate, zlib, gzip
//! or Brotli) is decompressed. Like [`crate::exif_tiff`], only chunk and box headers and the metadata
//! itself are read, every length is checked and capped, and walks are bounded.
//!
//! JPEG has its own scanner (`specular_metadata::jpeg::scan`). A JPEG XL ICC profile is coded in
//! the codestream: see [`crate::Session::embedded_icc`].

use std::borrow::Cow;

use crate::container_locate::{PSD_ICC, PSD_XMP, cr3_xmp, is_cr3, is_psd, psd_resource};

/// PSD image resource of the IPTC-IIM block.
const PSD_IPTC: u16 = 1028;
/// Largest IPTC-IIM block accepted (real ones are a few KiB).
const MAX_IPTC: u64 = 1 << 20;
use crate::exif_locate::{
    At, Encoding, HeifMeta, MAX_STEPS, MAX_XMP, heif_meta, is_big_tiff, is_jxr, is_raw_tiff,
    is_tiff, jxl_box,
};

/// Largest ICC profile accepted (JPEG's APP2 chunks allow about 16 MiB; real ones are far
/// smaller).
const MAX_ICC: u64 = 16 << 20;

/// The XMP packet, ICC profile and IPTC-IIM block of a file, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Embedded<'a> {
    pub xmp: Option<Cow<'a, [u8]>>,
    pub icc: Option<Cow<'a, [u8]>>,
    /// IPTC-IIM datasets (TIFF and PSD; a JPEG's are found by its own scanner).
    pub iptc: Option<Cow<'a, [u8]>>,
}

impl Embedded<'_> {
    pub fn into_owned(self) -> Embedded<'static> {
        Embedded {
            xmp: self.xmp.map(|c| Cow::Owned(c.into_owned())),
            icc: self.icc.map(|c| Cow::Owned(c.into_owned())),
            iptc: self.iptc.map(|c| Cow::Owned(c.into_owned())),
        }
    }
}

/// The XMP, ICC and IPTC of a whole file in memory (PNG, WebP, HEIC, AVIF, TIFF and TIFF-based
/// RAW, JPEG XL, PSD/PSB, CR3; a GIF's XMP). Empty for other formats (JPEG included). Borrows when a payload is stored
/// uncompressed in one piece.
pub fn embedded_metadata(bytes: &[u8]) -> Embedded<'_> {
    // A GIF's XMP application extension (found by walking its blocks).
    if let Some(x) = crate::text_tags::gif_xmp(bytes) {
        return Embedded {
            xmp: Some(Cow::Owned(x)),
            ..Default::default()
        };
    }
    locate(&bytes)
}

fn locate<'a>(s: &impl At<'a>) -> Embedded<'a> {
    let Some(head) = s.at(0, s.size().min(16)) else {
        return Embedded::default();
    };
    let h: &[u8] = &head;
    if h.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(s)
    } else if h.len() >= 12 && &h[..4] == b"RIFF" && &h[8..12] == b"WEBP" {
        webp(s)
    } else if h.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
        Embedded {
            xmp: jxl_box(s, *b"xml ", MAX_XMP),
            ..Default::default()
        }
    } else if is_psd(h) {
        Embedded {
            xmp: psd_resource(s, PSD_XMP, MAX_XMP),
            icc: psd_resource(s, PSD_ICC, MAX_ICC),
            iptc: psd_resource(s, PSD_IPTC, MAX_IPTC),
        }
    } else if is_cr3(h) {
        Embedded {
            xmp: cr3_xmp(s),
            ..Default::default()
        }
    } else if h.len() >= 8 && &h[4..8] == b"ftyp" {
        heif(s)
    } else if is_tiff(h) || is_big_tiff(h) || is_raw_tiff(h) || is_jxr(h) {
        tiff(s)
    } else {
        Embedded::default()
    }
}

const XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

fn png<'a>(s: &impl At<'a>) -> Embedded<'a> {
    let mut out = Embedded::default();
    let mut i = 8u64;
    for _ in 0..MAX_STEPS {
        let Some(h) = s.at(i, 8) else { break };
        let len = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
        let data = i + 8;
        match &h[4..8] {
            b"IEND" => break,
            b"iCCP" if out.icc.is_none() && len <= MAX_ICC => {
                out.icc = s.at(data, len).and_then(|b| png_iccp(&b));
            }
            b"iTXt" if out.xmp.is_none() && len <= MAX_XMP + 64 => {
                let is_xmp = s
                    .at(data, len.min(XMP_KEYWORD.len() as u64 + 1))
                    .is_some_and(|k| k.strip_suffix(b"\0") == Some(XMP_KEYWORD));
                if is_xmp {
                    out.xmp = s.at(data, len).and_then(png_itxt);
                }
            }
            _ => {}
        }
        if out.icc.is_some() && out.xmp.is_some() {
            break;
        }
        let Some(next) = data.checked_add(len).and_then(|n| n.checked_add(4)) else {
            break;
        };
        i = next;
    }
    out
}

/// An `iCCP` body: profile name, NUL, compression method 0, zlib stream.
fn png_iccp(b: &[u8]) -> Option<Cow<'static, [u8]>> {
    let nul = b.iter().take(80).position(|&c| c == 0)?;
    let rest = b.get(nul + 1..)?;
    if rest.first() != Some(&0) {
        return None;
    }
    let cap = usize::try_from(MAX_ICC).ok()?;
    specular_png::inflate_zlib(&rest[1..], cap)
        .ok()
        .filter(|p| !p.is_empty())
        .map(Cow::Owned)
}

/// The text of an `iTXt` body: keyword, NUL, compression flag and method, language tag, NUL,
/// translated keyword, NUL, text (zlib when the flag is 1).
fn png_itxt(c: Cow<'_, [u8]>) -> Option<Cow<'_, [u8]>> {
    let b: &[u8] = &c;
    let k = b.iter().position(|&x| x == 0)?;
    let (flag, method) = (*b.get(k + 1)?, *b.get(k + 2)?);
    let lang = k + 3 + b.get(k + 3..)?.iter().position(|&x| x == 0)?;
    let tkw = lang + 1 + b.get(lang + 1..)?.iter().position(|&x| x == 0)?;
    let start = tkw + 1;
    match (flag, method) {
        (0, _) => match c {
            Cow::Borrowed(b) => b.get(start..).map(Cow::Borrowed),
            Cow::Owned(mut v) => {
                v.drain(..start.min(v.len()));
                Some(Cow::Owned(v))
            }
        },
        (1, 0) => {
            let cap = usize::try_from(MAX_XMP).ok()?;
            specular_png::inflate_zlib(b.get(start..)?, cap)
                .ok()
                .map(Cow::Owned)
        }
        _ => None,
    }
    .filter(|t| !t.is_empty())
}

fn webp<'a>(s: &impl At<'a>) -> Embedded<'a> {
    let mut out = Embedded::default();
    let mut i = 12u64;
    for _ in 0..MAX_STEPS {
        let Some(h) = s.at(i, 8) else { break };
        let len = u64::from(u32::from_le_bytes([h[4], h[5], h[6], h[7]]));
        match &h[..4] {
            b"ICCP" if out.icc.is_none() && len <= MAX_ICC => out.icc = s.at(i + 8, len),
            b"XMP " if out.xmp.is_none() && len <= MAX_XMP => out.xmp = s.at(i + 8, len),
            _ => {}
        }
        if out.icc.is_some() && out.xmp.is_some() {
            break;
        }
        i += 8 + len + (len & 1);
    }
    out.icc = out.icc.filter(|p| !p.is_empty());
    out.xmp = out.xmp.filter(|p| !p.is_empty());
    out
}

fn heif<'a>(s: &impl At<'a>) -> Embedded<'a> {
    let Some(meta) = heif_meta(s) else {
        return Embedded::default();
    };
    let m = HeifMeta::parse(&meta);
    // The content encodings of the XMP items, in the order `items` lists them.
    let mut encodings = Vec::new();
    let xmp_items = m.items(|kind, rest| {
        // item_name, content_type, then an optional content_encoding.
        let mut f = rest.split(|&b| b == 0);
        let _name = f.next();
        let enc = match (kind == b"mime", f.next(), f.next()) {
            (true, Some(b"application/rdf+xml"), e) => Encoding::of(e.unwrap_or_default()),
            _ => None,
        };
        encodings.extend(enc);
        enc.is_some()
    });
    let xmp = m.prefer_primary(&xmp_items).and_then(|id| {
        let at = xmp_items.iter().position(|&i| i == id)?;
        let data = m.item_data(s, id, MAX_XMP)?;
        encodings.get(at)?.decode(data, MAX_XMP)
    });
    let icc = m.icc().map(|p| Cow::Owned(p.to_vec()));
    Embedded {
        xmp: xmp.filter(|x| !x.is_empty()),
        icc,
        iptc: None,
    }
}

/// TIFF tag of the XMP packet (XMLPacket).
const TAG_XMP: u16 = 700;
/// TIFF tag of the ICC profile (InterColorProfile).
const TAG_ICC: u16 = 34675;
/// TIFF tag of the IPTC-IIM block (IPTC-NAA).
const TAG_IPTC: u16 = 33723;
/// TIFF tag of the Photoshop image resources.
const TAG_PHOTOSHOP: u16 = 34377;

fn tiff<'a>(s: &impl At<'a>) -> Embedded<'a> {
    let mut out = Embedded::default();
    let Some(h) = s.at(0, 8) else { return out };
    let le = h[0] == b'I';
    let u16_at = |b: &[u8]| {
        let a = [b[0], b[1]];
        if le {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        }
    };
    let u32_at = |b: &[u8]| {
        let a = [b[0], b[1], b[2], b[3]];
        if le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        }
    };
    let u64_at = |b: &[u8]| {
        let a: [u8; 8] = b[..8].try_into().unwrap_or_default();
        if le {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        }
    };
    // BigTIFF: 8-byte IFD0 offset, entry count, counts and value fields; 20-byte entries.
    let big = u16_at(&h[2..4]) == 43;
    let (ifd, cs, es) = if big {
        if u16_at(&h[4..6]) != 8 {
            return out;
        }
        let Some(o) = s.at(8, 8) else { return out };
        (u64_at(&o), 8, 20)
    } else {
        (u64::from(u32_at(&h[4..8])), 2, 12)
    };
    let Some(n) = s.at(ifd, cs).map(|b| {
        if big {
            u64_at(&b).min(1024)
        } else {
            u64::from(u16_at(&b))
        }
    }) else {
        return out;
    };
    let Some(entries) = s.at(ifd + cs, n * es) else {
        return out;
    };
    let mut resources = None;
    for e in entries.chunks_exact(es as usize) {
        let tag = u16_at(&e[0..2]);
        let (max, slot) = match tag {
            TAG_XMP => (MAX_XMP, &mut out.xmp),
            TAG_ICC => (MAX_ICC, &mut out.icc),
            TAG_IPTC => (MAX_IPTC, &mut out.iptc),
            TAG_PHOTOSHOP => (MAX_IPTC * 4, &mut resources),
            _ => continue,
        };
        // BYTE, ASCII or UNDEFINED: one byte per value; the IPTC block and the Photoshop
        // resources are often written as LONGs (their bytes in file order all the same).
        let width = match u16_at(&e[2..4]) {
            1 | 2 | 7 => 1,
            4 if matches!(tag, TAG_IPTC | TAG_PHOTOSHOP) => 4,
            _ => continue,
        };
        if slot.is_some() {
            continue;
        }
        let (count, field) = if big {
            (u64_at(&e[4..12]), &e[12..20])
        } else {
            (u64::from(u32_at(&e[4..8])), &e[8..12])
        };
        let Some(count) = count.checked_mul(width) else {
            continue;
        };
        if count == 0 || count > max {
            continue;
        }
        *slot = if count <= field.len() as u64 {
            Some(Cow::Owned(field[..count as usize].to_vec()))
        } else if big {
            s.at(u64_at(field), count)
        } else {
            s.at(u64::from(u32_at(field)), count)
        };
    }
    if out.iptc.is_none()
        && let Some(r) = resources
    {
        out.iptc = photoshop_iptc(&r).map(|b| Cow::Owned(b.to_vec()));
    }
    out
}

/// The IPTC-IIM block (resource 1028) of a list of Photoshop image resources.
fn photoshop_iptc(b: &[u8]) -> Option<&[u8]> {
    let mut i = 0usize;
    for _ in 0..1024 {
        let h = b.get(i..i.checked_add(6)?)?;
        if &h[..4] != b"8BIM" {
            return None;
        }
        let id = u16::from_be_bytes([h[4], h[5]]);
        // A Pascal name padded to an even length.
        let mut j = i + 7 + usize::from(*b.get(i + 6)?);
        j += j % 2;
        let size = u32::from_be_bytes(b.get(j..j.checked_add(4)?)?.try_into().ok()?) as usize;
        j += 4;
        let data = b.get(j..j.checked_add(size)?)?;
        if id == PSD_IPTC {
            return Some(data);
        }
        i = j + size + size % 2;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const XMP: &[u8] = b"<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF \
        xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description \
        xmlns:xmp='http://ns.adobe.com/xap/1.0/' xmp:Rating='4'/></rdf:RDF></x:xmpmeta>";

    /// Not a real profile: the locator does not look inside.
    fn icc() -> Vec<u8> {
        (0..300u32).map(|i| (i * 7) as u8).collect()
    }

    /// A stored (uncompressed) zlib stream.
    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut z = vec![0x78, 0x01];
        for (i, part) in data.chunks(65535).enumerate() {
            let last = (i + 1) * 65535 >= data.len();
            z.push(last as u8);
            z.extend_from_slice(&(part.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(part.len() as u16)).to_le_bytes());
            z.extend_from_slice(part);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&(b << 16 | a).to_be_bytes());
        z
    }

    fn png_chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut c = (data.len() as u32).to_be_bytes().to_vec();
        c.extend_from_slice(kind);
        c.extend_from_slice(data);
        c.extend_from_slice(&[0; 4]);
        c
    }

    fn png(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut f = b"\x89PNG\r\n\x1a\n".to_vec();
        f.extend(png_chunk(b"IHDR", &[0; 13]));
        for c in chunks {
            f.extend_from_slice(c);
        }
        f.extend(png_chunk(b"IDAT", &[0; 16]));
        f.extend(png_chunk(b"IEND", &[]));
        f
    }

    fn itxt(compressed: bool, keyword: &[u8]) -> Vec<u8> {
        let mut b = keyword.to_vec();
        b.extend_from_slice(&[0, compressed as u8, 0]);
        b.extend_from_slice(b"en\0\0");
        if compressed {
            b.extend(zlib(XMP));
        } else {
            b.extend_from_slice(XMP);
        }
        png_chunk(b"iTXt", &b)
    }

    fn iccp() -> Vec<u8> {
        let mut b = b"ICC profile\0\0".to_vec();
        b.extend(zlib(&icc()));
        png_chunk(b"iCCP", &b)
    }

    /// Finds `xmp` and `icc` in `file`, and nothing panics on any truncation of it.
    fn check(file: &[u8], xmp: Option<&[u8]>, icc: Option<&[u8]>) {
        let e = embedded_metadata(file);
        assert_eq!(e.xmp.as_deref(), xmp);
        assert_eq!(e.icc.as_deref(), icc);
        for n in 0..file.len() {
            let _ = embedded_metadata(&file[..n]);
        }
    }

    #[test]
    fn png_itxt_and_iccp() {
        check(
            &png(&[iccp(), itxt(false, XMP_KEYWORD)]),
            Some(XMP),
            Some(&icc()),
        );
        check(&png(&[itxt(true, XMP_KEYWORD)]), Some(XMP), None);
        // Another keyword is not XMP.
        check(&png(&[itxt(false, b"Comment")]), None, None);
        check(&png(&[]), None, None);
    }

    fn webp(chunks: &[(&[u8], &[u8])]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        for (k, d) in chunks {
            body.extend_from_slice(k);
            body.extend_from_slice(&(d.len() as u32).to_le_bytes());
            body.extend_from_slice(d);
            if d.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut f = b"RIFF".to_vec();
        f.extend_from_slice(&(body.len() as u32).to_le_bytes());
        f.extend(body);
        f
    }

    #[test]
    fn webp_chunks() {
        let icc = icc();
        let f = webp(&[
            (b"VP8X", &[0x24, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (b"ICCP", &icc),
            (b"VP8L", &[0; 5]),
            (b"XMP ", XMP),
        ]);
        check(&f, Some(XMP), Some(&icc));
        check(&webp(&[(b"VP8L", &[0; 5])]), None, None);
    }

    fn bx(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(kind);
        b.extend_from_slice(body);
        b
    }

    fn full(v: u8, flags: u8, rest: &[u8]) -> Vec<u8> {
        let mut b = vec![v, 0, 0, flags];
        b.extend_from_slice(rest);
        b
    }

    /// A HEIF file: primary item 1 (hvc1) and a thumbnail 4, an XMP `mime` item 3 describing the
    /// primary (and a decoy XMP item 2 that does not), in the file or in `idat`. The thumbnail's
    /// `colr` comes first in `ipco`; the primary's is the second.
    fn heif(method: u8, wide_ipma: bool) -> Vec<u8> {
        heif_coded(method, wide_ipma, b"", XMP)
    }

    /// [`heif`] with the XMP item stored with content encoding `enc` as `xmp`.
    fn heif_coded(method: u8, wide_ipma: bool, enc: &[u8], xmp: &[u8]) -> Vec<u8> {
        let infe = |id: u16, kind: &[u8], extra: &[u8]| {
            let mut b = id.to_be_bytes().to_vec();
            b.extend_from_slice(&[0, 0]);
            b.extend_from_slice(kind);
            b.push(0);
            b.extend_from_slice(extra);
            bx(b"infe", &full(2, 0, &b))
        };
        let decoy = b"<x:xmpmeta/>";
        let mut iinf = 4u16.to_be_bytes().to_vec();
        iinf.extend(infe(1, b"hvc1", b""));
        iinf.extend(infe(2, b"mime", b"application/rdf+xml\0"));
        let mut ct = b"application/rdf+xml\0".to_vec();
        ct.extend_from_slice(enc);
        ct.push(0);
        iinf.extend(infe(3, b"mime", &ct));
        iinf.extend(infe(4, b"hvc1", b""));
        let iref = bx(b"cdsc", &[0, 3, 0, 1, 0, 1]);
        let mut other = b"prof".to_vec();
        other.extend_from_slice(&[9; 40]);
        let mut prof = b"prof".to_vec();
        prof.extend(icc());
        let mut ipco = bx(b"colr", &other);
        ipco.extend(bx(b"ispe", &full(0, 0, &[0; 8])));
        ipco.extend(bx(b"colr", &prof));
        let assoc = |idx: &[u16]| {
            let mut b = vec![idx.len() as u8];
            for &i in idx {
                if wide_ipma {
                    b.extend_from_slice(&(0x8000 | i).to_be_bytes());
                } else {
                    b.push(0x80 | i as u8);
                }
            }
            b
        };
        let mut ipma = 2u32.to_be_bytes().to_vec();
        ipma.extend_from_slice(&4u16.to_be_bytes());
        ipma.extend(assoc(&[1, 2]));
        ipma.extend_from_slice(&1u16.to_be_bytes());
        ipma.extend(assoc(&[2, 3]));
        let mut iprp = bx(b"ipco", &ipco);
        iprp.extend(bx(b"ipma", &full(0, wide_ipma as u8, &ipma)));
        let ftyp = bx(b"ftyp", b"heic\0\0\0\0mif1heic");
        let mut payload = decoy.to_vec();
        payload.extend_from_slice(xmp);
        let iloc = |data_at: u32| {
            let mut b = vec![0x44, 0x00, 0, 2];
            b.extend_from_slice(&[0, 2, 0, method, 0, 0, 0, 1]);
            b.extend_from_slice(&data_at.to_be_bytes());
            b.extend_from_slice(&(decoy.len() as u32).to_be_bytes());
            // Item 3 in two extents.
            let half = (xmp.len() / 2) as u32;
            let at = data_at + decoy.len() as u32;
            b.extend_from_slice(&[0, 3, 0, method, 0, 0, 0, 2]);
            b.extend_from_slice(&at.to_be_bytes());
            b.extend_from_slice(&half.to_be_bytes());
            b.extend_from_slice(&(at + half).to_be_bytes());
            b.extend_from_slice(&(xmp.len() as u32 - half).to_be_bytes());
            bx(b"iloc", &full(1, 0, &b))
        };
        let meta = |data_at: u32| {
            let mut m = bx(b"hdlr", &full(0, 0, &[0; 20]));
            m.extend(bx(b"pitm", &full(0, 0, &[0, 1])));
            m.extend(bx(b"iinf", &full(0, 0, &iinf)));
            m.extend(bx(b"iref", &full(0, 0, &iref)));
            m.extend(bx(b"iprp", &iprp));
            m.extend(iloc(data_at));
            if method == 1 {
                m.extend(bx(b"idat", &payload));
            }
            bx(b"meta", &full(0, 0, &m))
        };
        let at = (ftyp.len() + meta(0).len() + 8) as u32;
        let mut f = ftyp;
        f.extend(meta(if method == 0 { at } else { 0 }));
        f.extend(bx(b"mdat", if method == 0 { &payload } else { &[] }));
        f
    }

    #[test]
    fn heif_mime_item_and_colr() {
        for (method, wide) in [(0, false), (1, false), (0, true)] {
            check(&heif(method, wide), Some(XMP), Some(&icc()));
        }
    }

    /// A raw DEFLATE stream of fixed-Huffman literals (no matches), ending with end-of-block.
    fn deflate_fixed(data: &[u8]) -> Vec<u8> {
        let (mut out, mut acc, mut n) = (Vec::new(), 0u64, 0u32);
        let mut put = |v: u64, bits: u32, acc: &mut u64, n: &mut u32| {
            *acc |= v << *n;
            *n += bits;
            while *n >= 8 {
                out.push(*acc as u8);
                *acc >>= 8;
                *n -= 8;
            }
        };
        // Huffman codes are sent most significant bit first.
        let rev = |code: u32, len: u32| u64::from(code.reverse_bits() >> (32 - len));
        put(0b011, 3, &mut acc, &mut n); // BFINAL 1, BTYPE 01
        for &b in data {
            let (code, len) = if b < 144 {
                (0x30 + u32::from(b), 8)
            } else {
                (0x190 + u32::from(b) - 144, 9)
            };
            put(rev(code, len), len, &mut acc, &mut n);
        }
        put(0, 7, &mut acc, &mut n); // end of block: code 256 is 7 zero bits
        if n > 0 {
            out.push(acc as u8);
        }
        out
    }

    /// An XMP item stored compressed: zlib or raw DEFLATE under "deflate", Brotli under "br",
    /// gzip under "gzip";
    /// an item with an unknown encoding is skipped, and a broken stream gives no XMP.
    #[test]
    fn heif_compressed_xmp_item() {
        let raw = deflate_fixed(XMP);
        assert_eq!(specular_png::inflate_raw(&raw, 1 << 20).unwrap(), XMP);
        let brotli = {
            // One uncompressed meta-block (WBITS 16), then the empty last meta-block.
            let m = XMP.len() as u32 - 1;
            let bits = (m << 4) | (1 << 20);
            let mut b = bits.to_le_bytes()[..3].to_vec();
            b.extend_from_slice(XMP);
            b.push(0b11);
            b
        };
        for (enc, data) in [
            (&b"deflate"[..], zlib(XMP)),
            (b"deflate", raw.clone()),
            (b"zlib", zlib(XMP)),
            (b"br", brotli),
            (b"gzip", crate::exif_locate::tests::gzip(XMP)),
        ] {
            for method in [0, 1] {
                let f = heif_coded(method, false, enc, &data);
                check(&f, Some(XMP), Some(&icc()));
            }
        }
        let f = heif_coded(0, false, b"lzma", &zlib(XMP));
        assert_eq!(
            embedded_metadata(&f).xmp.as_deref(),
            Some(&b"<x:xmpmeta/>"[..])
        );
        let mut broken = raw;
        broken.truncate(broken.len() / 2);
        let f = heif_coded(0, false, b"deflate", &broken);
        assert_eq!(embedded_metadata(&f).xmp, None);
    }

    /// A little- or big-endian TIFF whose IFD0 has an XMP packet and an ICC profile.
    fn tiff(le: bool) -> Vec<u8> {
        let u16b = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let u32b = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let icc = icc();
        let mut f = if le {
            b"II*\0".to_vec()
        } else {
            b"MM\0*".to_vec()
        };
        f.extend_from_slice(&u32b(8));
        let entries: [(u16, u16, u32); 3] = [(256, 3, 1), (700, 1, 0), (34675, 7, 0)];
        let data_at = 8 + 2 + 12 * entries.len() as u32 + 4;
        f.extend_from_slice(&u16b(entries.len() as u16));
        for (tag, kind, _) in entries {
            f.extend_from_slice(&u16b(tag));
            f.extend_from_slice(&u16b(kind));
            let (count, off) = match tag {
                700 => (XMP.len() as u32, data_at),
                34675 => (icc.len() as u32, data_at + XMP.len() as u32),
                _ => (1, 0),
            };
            f.extend_from_slice(&u32b(count));
            f.extend_from_slice(&u32b(off));
        }
        f.extend_from_slice(&[0; 4]);
        f.extend_from_slice(XMP);
        f.extend(icc);
        f
    }

    /// [`tiff`] as a BigTIFF: 8-byte offsets and counts, 20-byte entries.
    fn big_tiff(le: bool) -> Vec<u8> {
        let u16b = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let u64b = |v: u64| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let icc = icc();
        let mut f = if le {
            b"II+\0".to_vec()
        } else {
            b"MM\0+".to_vec()
        };
        f.extend_from_slice(&u16b(8));
        f.extend_from_slice(&u16b(0));
        f.extend_from_slice(&u64b(16));
        let data_at = 16 + 8 + 20 * 3 + 8;
        f.extend_from_slice(&u64b(3));
        for (tag, kind, count, off) in [
            (256, 3, 1, 0),
            (700, 1, XMP.len() as u64, data_at),
            (34675, 7, icc.len() as u64, data_at + XMP.len() as u64),
        ] {
            f.extend_from_slice(&u16b(tag));
            f.extend_from_slice(&u16b(kind));
            f.extend_from_slice(&u64b(count));
            f.extend_from_slice(&u64b(off));
        }
        f.extend_from_slice(&[0; 8]);
        f.extend_from_slice(XMP);
        f.extend(icc);
        f
    }

    #[test]
    fn tiff_tags() {
        check(&tiff(true), Some(XMP), Some(&icc()));
        check(&tiff(false), Some(XMP), Some(&icc()));
        check(&big_tiff(true), Some(XMP), Some(&icc()));
        check(&big_tiff(false), Some(XMP), Some(&icc()));
        // Another offset size is not a BigTIFF.
        let mut f = big_tiff(true);
        f[4] = 4;
        check(&f, None, None);
    }

    #[test]
    fn jxl_xml_box() {
        let mut f = bx(b"JXL ", b"\r\n\x87\n");
        f.extend(bx(b"ftyp", b"jxl \0\0\0\0jxl "));
        f.extend(bx(b"xml ", XMP));
        f.extend(bx(b"jxlc", &[0xFF, 0x0A, 0, 0]));
        check(&f, Some(XMP), None);
    }

    #[test]
    fn other_formats_have_none() {
        check(&[0xFF, 0xD8, 0xFF, 0xD9], None, None);
        check(b"GIF89a", None, None);
        assert_eq!(embedded_metadata(&[]), Embedded::default());
    }
}
