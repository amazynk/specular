//! A file's own XMP packet and IPTC-IIM block: found in a JPEG (APP1, APP13), a TIFF or DNG
//! (IFD0 tags 700 and 33723) or, in other files, the first packet near the start; and put back
//! into a JPEG or TIFF.
//!
//! Putting them back changes no other byte of the file's structure and never the image data:
//! - JPEG: the XMP APP1 segment and the Photoshop APP13 segment's IPTC resource (1028) are
//!   replaced, or added after the leading APP0/APP1 segments; the IPTC digest resource (1061)
//!   is brought up to date when there is one. Every other segment and everything from the
//!   first scan on are copied unchanged.
//! - TIFF and DNG (classic TIFF, not BigTIFF): the packet is appended to the file and IFD0's
//!   XMP entry pointed at it. Without one, IFD0 is copied to the end with the entry added and
//!   the header pointed at the copy (offsets in it are absolute, so they stay right). The old
//!   bytes stay where they were, unused.

use std::io::{self, Read, Seek, SeekFrom};

use md5::{Digest, Md5};

use super::Record;

const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PHOTOSHOP_SIG: &[u8] = b"Photoshop 3.0\0";
/// How far into other files a packet is looked for.
const HEAD: u64 = 256 << 10;
/// The largest XMP packet one APP1 segment holds.
pub const JPEG_XMP_MAX: usize = 65533 - 29;

/// A file's own XMP packet and IIM block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Embedded {
    pub xmp: Option<String>,
    pub iim: Option<Vec<u8>>,
}

impl Embedded {
    /// The editable fields: XMP's, then IIM's for those XMP does not have (the Metadata
    /// Working Group's rule).
    pub fn record(&self) -> Record {
        let x = self
            .xmp
            .as_deref()
            .map(|t| super::from_xmp(&crate::xmp::parse(t.as_bytes())))
            .unwrap_or_default();
        let i = self.iim.as_deref().map(super::read_iim).unwrap_or_default();
        x.over(&i)
    }
}

/// What kind of file the fields can be written into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Jpeg,
    /// Classic TIFF (TIFF, DNG).
    Tiff,
}

/// Why a file cannot take the fields.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EmbedError {
    #[error("only JPEG, TIFF and DNG files can hold the fields")]
    Unsupported,
    #[error("the file's structure could not be read")]
    Damaged,
    #[error("BigTIFF files are not written")]
    BigTiff,
    #[error("the XMP packet is too large for one JPEG segment")]
    TooLarge,
    #[error("the IPTC block spans several segments")]
    SplitIptc,
}

/// The container `head` (the file's first bytes) is, by content.
pub fn container(head: &[u8]) -> Option<Container> {
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Container::Jpeg)
    } else if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        Some(Container::Tiff)
    } else {
        None
    }
}

/// Whether a file named with `ext` may be written (by content it must also be JPEG or TIFF):
/// raw formats built on TIFF are never written.
pub fn writable_ext(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "jpe" | "jfif" | "tif" | "tiff" | "dng"
    )
}

/// The packet and IIM block of a file.
pub fn read_embedded<R: Read + Seek>(f: &mut R) -> io::Result<Embedded> {
    let mut sig = [0u8; 4];
    f.seek(SeekFrom::Start(0))?;
    let n = read_up_to(f, &mut sig)?;
    match container(&sig[..n]) {
        Some(Container::Jpeg) => read_jpeg(f),
        Some(Container::Tiff) => read_tiff(f),
        None => {
            f.seek(SeekFrom::Start(0))?;
            let mut head = Vec::new();
            f.take(HEAD).read_to_end(&mut head)?;
            let xmp =
                crate::keywords::packet(&head).map(|p| String::from_utf8_lossy(p).into_owned());
            Ok(Embedded { xmp, iim: None })
        }
    }
}

fn read_up_to<R: Read>(f: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// The XMP and IPTC of a JPEG's segments before the first scan.
fn read_jpeg<R: Read + Seek>(f: &mut R) -> io::Result<Embedded> {
    let mut out = Embedded::default();
    let mut pos = 2u64;
    for _ in 0..4096 {
        f.seek(SeekFrom::Start(pos))?;
        let mut h = [0u8; 4];
        if read_up_to(f, &mut h)? < 4 || h[0] != 0xFF {
            break;
        }
        let marker = h[1];
        if marker == 0xFF {
            pos += 1;
            continue;
        }
        if matches!(marker, 0xD9 | 0xDA) {
            break;
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            pos += 2;
            continue;
        }
        let len = u64::from(u16::from_be_bytes([h[2], h[3]]));
        if len < 2 {
            break;
        }
        if marker == 0xE1 || marker == 0xED {
            let mut seg = vec![0u8; len as usize - 2];
            if read_up_to(f, &mut seg)? < seg.len() {
                break;
            }
            if marker == 0xE1 && out.xmp.is_none() && seg.starts_with(XMP_SIG) {
                out.xmp = Some(String::from_utf8_lossy(&seg[XMP_SIG.len()..]).into_owned());
            } else if marker == 0xED && out.iim.is_none() && seg.starts_with(PHOTOSHOP_SIG) {
                out.iim =
                    crate::iptc::from_resources(&seg[PHOTOSHOP_SIG.len()..]).map(<[u8]>::to_vec);
            }
        }
        pos += 2 + len;
    }
    Ok(out)
}

/// Byte order of a TIFF.
#[derive(Clone, Copy)]
struct Order(bool);

impl Order {
    fn u16(self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        if self.0 {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        }
    }

    fn u32(self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        if self.0 {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        }
    }

    fn put_u16(self, v: u16) -> [u8; 2] {
        if self.0 {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }

    fn put_u32(self, v: u32) -> [u8; 4] {
        if self.0 {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }
}

/// Bytes per value of a TIFF field type.
fn type_size(t: u16) -> u64 {
    match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => 1,
    }
}

/// A TIFF's IFD0.
struct Ifd {
    order: Order,
    /// Where it starts.
    at: u32,
    /// Its entries as stored: tag, type, count, the value or offset.
    entries: Vec<[u8; 12]>,
    /// The next IFD's offset.
    next: u32,
}

/// IFD0 of a classic TIFF (None when it does not read).
fn ifd0<R: Read + Seek>(f: &mut R) -> io::Result<Option<Ifd>> {
    f.seek(SeekFrom::Start(0))?;
    let mut h = [0u8; 8];
    if read_up_to(f, &mut h)? < 8 {
        return Ok(None);
    }
    let o = Order(h[0] == b'I');
    if o.u16(&h[2..]) != 42 {
        return Ok(None);
    }
    let at = o.u32(&h[4..]);
    f.seek(SeekFrom::Start(u64::from(at)))?;
    let mut c = [0u8; 2];
    if read_up_to(f, &mut c)? < 2 {
        return Ok(None);
    }
    let n = usize::from(o.u16(&c));
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let mut e = [0u8; 12];
        if read_up_to(f, &mut e)? < 12 {
            return Ok(None);
        }
        entries.push(e);
    }
    let mut next = [0u8; 4];
    if read_up_to(f, &mut next)? < 4 {
        return Ok(None);
    }
    Ok(Some(Ifd {
        order: o,
        at,
        entries,
        next: o.u32(&next),
    }))
}

/// The value of IFD0 entry `e`, at most `max` bytes.
fn entry_value<R: Read + Seek>(
    f: &mut R,
    o: Order,
    e: &[u8; 12],
    max: u64,
) -> io::Result<Option<Vec<u8>>> {
    let len = type_size(o.u16(&e[2..])) * u64::from(o.u32(&e[4..]));
    if len > max {
        return Ok(None);
    }
    if len <= 4 {
        return Ok(Some(e[8..8 + len as usize].to_vec()));
    }
    f.seek(SeekFrom::Start(u64::from(o.u32(&e[8..]))))?;
    let mut v = vec![0u8; len as usize];
    if read_up_to(f, &mut v)? < v.len() {
        return Ok(None);
    }
    Ok(Some(v))
}

fn read_tiff<R: Read + Seek>(f: &mut R) -> io::Result<Embedded> {
    let mut out = Embedded::default();
    let Some(Ifd {
        order: o, entries, ..
    }) = ifd0(f)?
    else {
        return Ok(out);
    };
    for e in &entries {
        match o.u16(e) {
            700 => {
                if let Some(v) = entry_value(f, o, e, 16 << 20)? {
                    let v = crate::keywords::packet(&v).map_or(&v[..], |p| p).to_vec();
                    out.xmp = Some(String::from_utf8_lossy(&v).into_owned());
                }
            }
            33723 => out.iim = entry_value(f, o, e, 16 << 20)?,
            _ => {}
        }
    }
    Ok(out)
}

// ----- Writing -------------------------------------------------------------------------------

/// The XMP packet as written: in an `xpacket` wrapper, with room to grow in place.
fn wrapped(xmp: &str) -> String {
    let body = xmp.trim();
    if body.starts_with("<?xpacket") {
        return body.to_owned() + "\n";
    }
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n{body}\n<?xpacket end=\"w\"?>"
    )
}

/// `jpeg` with its XMP packet set to `xmp` and its IPTC block to `iim` (None: left as it
/// is; Some(None): taken out).
pub fn jpeg(jpeg: &[u8], xmp: &str, iim: Option<Option<&[u8]>>) -> Result<Vec<u8>, EmbedError> {
    if container(jpeg) != Some(Container::Jpeg) {
        return Err(EmbedError::Unsupported);
    }
    let packet = wrapped(xmp);
    if packet.len() > JPEG_XMP_MAX {
        return Err(EmbedError::TooLarge);
    }
    // The segments before the first scan: (marker, start, end).
    let mut segs: Vec<(u8, usize, usize)> = Vec::new();
    let mut i = 2usize;
    let scan_at;
    loop {
        let Some(h) = jpeg.get(i..i + 2) else {
            return Err(EmbedError::Damaged);
        };
        if h[0] != 0xFF {
            return Err(EmbedError::Damaged);
        }
        let marker = h[1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if marker == 0xDA || marker == 0xD9 {
            scan_at = i;
            break;
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            segs.push((marker, i, i + 2));
            i += 2;
            continue;
        }
        let Some(l) = jpeg.get(i + 2..i + 4) else {
            return Err(EmbedError::Damaged);
        };
        let len = usize::from(u16::from_be_bytes([l[0], l[1]]));
        if len < 2 || i + 2 + len > jpeg.len() {
            return Err(EmbedError::Damaged);
        }
        segs.push((marker, i, i + 2 + len));
        i += 2 + len;
    }
    let body = |s: &(u8, usize, usize)| &jpeg[s.1 + 4..s.2];
    let xmp_at = segs
        .iter()
        .position(|s| s.0 == 0xE1 && body(s).starts_with(XMP_SIG));
    let ps: Vec<usize> = segs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.0 == 0xED && body(s).starts_with(PHOTOSHOP_SIG))
        .map(|(k, _)| k)
        .collect();
    if iim.is_some() && ps.len() > 1 {
        return Err(EmbedError::SplitIptc);
    }
    let mut xmp_seg = XMP_SIG.to_vec();
    xmp_seg.extend(packet.as_bytes());
    let new_ps: Option<Option<Vec<u8>>> = match iim {
        None => None,
        Some(block) => {
            let old = ps.first().map(|&k| &body(&segs[k])[PHOTOSHOP_SIG.len()..]);
            let res = set_iptc_resource(old.unwrap_or(&[]), block)?;
            Some(res.map(|r| {
                let mut s = PHOTOSHOP_SIG.to_vec();
                s.extend(r);
                s
            }))
        }
    };
    if let Some(Some(s)) = &new_ps
        && s.len() > 65533
    {
        return Err(EmbedError::TooLarge);
    }
    // Where new segments go: after SOI and the leading APP0/APP1 run.
    let lead = segs
        .iter()
        .take_while(|s| s.0 == 0xE0 || s.0 == 0xE1)
        .count();
    let mut out = Vec::with_capacity(jpeg.len() + xmp_seg.len() + 64);
    out.extend(&jpeg[..2]);
    let push_seg = |out: &mut Vec<u8>, marker: u8, b: &[u8]| {
        out.extend([0xFF, marker]);
        out.extend(((b.len() + 2) as u16).to_be_bytes());
        out.extend(b);
    };
    let mut xmp_done = false;
    let mut ps_done = new_ps.is_none();
    for (k, s) in segs.iter().enumerate() {
        if k == lead {
            if xmp_at.is_none() && !xmp_done {
                push_seg(&mut out, 0xE1, &xmp_seg);
                xmp_done = true;
            }
            if ps.is_empty() && !ps_done {
                if let Some(Some(p)) = &new_ps {
                    push_seg(&mut out, 0xED, p);
                }
                ps_done = true;
            }
        }
        if Some(k) == xmp_at {
            push_seg(&mut out, 0xE1, &xmp_seg);
            xmp_done = true;
            continue;
        }
        if ps.first() == Some(&k) && !ps_done {
            if let Some(Some(p)) = &new_ps {
                push_seg(&mut out, 0xED, p);
            }
            ps_done = true;
            continue;
        }
        out.extend(&jpeg[s.1..s.2]);
    }
    if !xmp_done {
        push_seg(&mut out, 0xE1, &xmp_seg);
    }
    if !ps_done && let Some(Some(p)) = &new_ps {
        push_seg(&mut out, 0xED, p);
    }
    out.extend(&jpeg[scan_at..]);
    Ok(out)
}

/// A Photoshop resource list with its IPTC resource (1028) set to `iim` (None: taken out),
/// and the IPTC digest (1061) updated where there is one. None when no resource is left.
fn set_iptc_resource(old: &[u8], iim: Option<&[u8]>) -> Result<Option<Vec<u8>>, EmbedError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut placed = false;
    let digest = iim.map(|b| Md5::digest(b).to_vec());
    let resource = |out: &mut Vec<u8>, id: u16, data: &[u8]| {
        out.extend(b"8BIM");
        out.extend(id.to_be_bytes());
        out.extend([0, 0]);
        out.extend((data.len() as u32).to_be_bytes());
        out.extend(data);
        if data.len() % 2 == 1 {
            out.push(0);
        }
    };
    while i < old.len() {
        let Some(h) = old.get(i..i + 6) else {
            return Err(EmbedError::Damaged);
        };
        if &h[..4] != b"8BIM" {
            // Padding at the end of some lists.
            if old[i..].iter().all(|&b| b == 0) {
                break;
            }
            return Err(EmbedError::Damaged);
        }
        let id = u16::from_be_bytes([h[4], h[5]]);
        let name_len = usize::from(*old.get(i + 6).ok_or(EmbedError::Damaged)?);
        let mut j = i + 6 + 1 + name_len;
        j += j % 2;
        let size = old
            .get(j..j + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .ok_or(EmbedError::Damaged)?;
        let end = j + 4 + size;
        if end > old.len() {
            return Err(EmbedError::Damaged);
        }
        let next = (end + size % 2).min(old.len());
        match id {
            0x0404 => {
                if let Some(b) = iim
                    && !placed
                {
                    resource(&mut out, 0x0404, b);
                    placed = true;
                }
            }
            0x0425 => {
                if let Some(d) = &digest {
                    resource(&mut out, 0x0425, d);
                }
            }
            _ => out.extend(&old[i..next]),
        }
        i = next;
    }
    if let Some(b) = iim
        && !placed
    {
        resource(&mut out, 0x0404, b);
    }
    Ok((!out.is_empty()).then_some(out))
}

/// `tiff` with its XMP packet (IFD0 tag 700) set to `xmp`.
pub fn tiff(tiff: &[u8], xmp: &str) -> Result<Vec<u8>, EmbedError> {
    if tiff.starts_with(b"II+\0") || tiff.starts_with(b"MM\0+") {
        return Err(EmbedError::BigTiff);
    }
    if container(tiff) != Some(Container::Tiff) {
        return Err(EmbedError::Unsupported);
    }
    let mut cur = io::Cursor::new(tiff);
    let Ok(Some(Ifd {
        order: o,
        at,
        entries,
        next,
    })) = ifd0(&mut cur)
    else {
        return Err(EmbedError::Damaged);
    };
    let packet = wrapped(xmp);
    let mut out = tiff.to_vec();
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let data_at = out.len();
    out.extend(packet.as_bytes());
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let data_at = u32::try_from(data_at).map_err(|_| EmbedError::BigTiff)?;
    let entry = |tag: u16| -> [u8; 12] {
        let mut e = [0u8; 12];
        e[..2].copy_from_slice(&o.put_u16(tag));
        e[2..4].copy_from_slice(&o.put_u16(1));
        e[4..8].copy_from_slice(&o.put_u32(packet.len() as u32));
        e[8..].copy_from_slice(&o.put_u32(data_at));
        e
    };
    if let Some(k) = entries.iter().position(|e| o.u16(e) == 700) {
        // The entry in place points at the new packet.
        let at = at as usize + 2 + 12 * k;
        out[at..at + 12].copy_from_slice(&entry(700));
    } else {
        // IFD0 again at the end with the entry, and the header pointed at it.
        let mut es = entries.clone();
        es.push(entry(700));
        es.sort_by_key(|e| o.u16(e));
        let ifd_at = u32::try_from(out.len()).map_err(|_| EmbedError::BigTiff)?;
        out.extend(o.put_u16(es.len() as u16));
        for e in &es {
            out.extend(e);
        }
        out.extend(o.put_u32(next));
        out[4..8].copy_from_slice(&o.put_u32(ifd_at));
    }
    if out.len() > u32::MAX as usize {
        return Err(EmbedError::BigTiff);
    }
    Ok(out)
}
