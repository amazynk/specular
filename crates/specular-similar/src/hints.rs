//! Metadata hints: capture time with sub-seconds, camera and body serial, bracketing and
//! burst ids, XMP document ids, GPS and focal length, read from the head of the file (no
//! pixel decode) and from XMP sidecars.
//!
//! Only what raises or lowers the confidence of a match is read. The parsers are small and
//! bounded: a damaged or hostile header gives `None`s, never a panic or a long loop.

/// What the metadata says about one file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hints {
    /// Capture time as written (local wall clock), ms since 1970; sub-seconds when recorded.
    pub time_ms: Option<i64>,
    /// The time carries sub-seconds (bursts are told apart inside one second).
    pub subsec: bool,
    /// Hash of make + model.
    pub camera: Option<u64>,
    /// Hash of the body serial number.
    pub serial: Option<u64>,
    /// Hash of EXIF ImageUniqueID (same for a RAW and its camera JPEG on some bodies).
    pub unique_id: Option<u64>,
    /// Hashes of XMP xmpMM:DocumentID, OriginalDocumentID and DerivedFrom's documentID.
    pub document_id: Option<u64>,
    pub original_document_id: Option<u64>,
    pub derived_from: Option<u64>,
    /// Burst group (Apple BurstUUID, Google GCamera:BurstID), hashed.
    pub burst: Option<u64>,
    /// EXIF ExposureMode = auto bracket.
    pub bracket: bool,
    /// Latitude, longitude (degrees).
    pub gps: Option<(f64, f64)>,
    pub focal_mm: Option<f32>,
    /// The file names an editing application (Software), i.e. it is likely an export.
    pub edited_by_software: bool,
    /// Any EXIF at all (the "metadata present" tie-break of duplicates).
    pub has_exif: bool,
}

pub fn fnv_str(s: &str) -> u64 {
    s.trim().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "YYYY:MM:DD HH:MM:SS" (also `-`/`T` separators, optional fraction) → ms since 1970.
pub fn parse_time_ms(s: &str, subsec: Option<&str>) -> Option<(i64, bool)> {
    let mut parts = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty());
    let mut d = [0i64; 6];
    let mut got = 0;
    for v in d.iter_mut() {
        match parts.next() {
            Some(p) => {
                *v = p.parse().ok()?;
                got += 1;
            }
            None => break,
        }
    }
    if got < 3 || d[0] < 1800 || !(1..=12).contains(&d[1]) || !(1..=31).contains(&d[2]) {
        return None;
    }
    let secs = days_from_civil(d[0], d[1], d[2]) * 86_400 + d[3] * 3600 + d[4] * 60 + d[5];
    // A fraction in the time itself ("…:05.120"), else the SubSecTime tag.
    let frac = parts
        .next()
        .filter(|_| got == 6 && s.contains('.'))
        .map(str::to_owned)
        .or_else(|| subsec.map(|t| t.trim().to_owned()))
        .filter(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()));
    let (ms, has) = match frac {
        Some(f) => {
            let digits: String = f.chars().chain("000".chars()).take(3).collect();
            (digits.parse::<i64>().unwrap_or(0), true)
        }
        None => (0, false),
    };
    Some((secs * 1000 + ms, has))
}

/// A bounded TIFF reader.
struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl<'a> Tiff<'a> {
    fn new(b: &'a [u8]) -> Option<Tiff<'a>> {
        let le = match b.get(..2)? {
            b"II" => true,
            b"MM" => false,
            _ => return None,
        };
        let t = Tiff { b, le };
        (t.u16(2)? == 42).then_some(t)
    }
    fn u16(&self, o: usize) -> Option<u16> {
        let s = self.b.get(o..o + 2)?;
        Some(if self.le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        })
    }
    fn u32(&self, o: usize) -> Option<u32> {
        let s = self.b.get(o..o + 4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }
    fn ifd0(&self) -> Option<usize> {
        self.u32(4).map(|v| v as usize)
    }
    /// The entry offset of `tag` in the IFD at `ifd`.
    fn find(&self, ifd: usize, tag: u16) -> Option<usize> {
        let n = self.u16(ifd)? as usize;
        (0..n.min(400))
            .map(|i| ifd + 2 + i * 12)
            .find(|&e| self.u16(e) == Some(tag))
    }
    /// The data of an entry (inline or at its offset).
    fn data(&self, e: usize) -> Option<&'a [u8]> {
        let typ = self.u16(e + 2)?;
        let count = self.u32(e + 4)? as usize;
        let size = match typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => return None,
        };
        let len = size * count.min(1 << 16);
        let off = if len <= 4 {
            e + 8
        } else {
            self.u32(e + 8)? as usize
        };
        self.b.get(off..off + len)
    }
    fn ascii(&self, e: usize) -> Option<String> {
        let d = self.data(e)?;
        let s: String = d
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();
        let s = s.trim().to_owned();
        (!s.is_empty()).then_some(s)
    }
    fn short(&self, e: usize) -> Option<u32> {
        match self.u16(e + 2)? {
            3 => self.u16(e + 8).map(u32::from),
            4 => self.u32(e + 8),
            _ => None,
        }
    }
    fn rational(&self, e: usize, k: usize) -> Option<f64> {
        let d = self.data(e)?;
        let sub = Tiff {
            b: d.get(k * 8..k * 8 + 8)?,
            le: self.le,
        };
        let (n, dd) = (sub.u32(0)?, sub.u32(4)?);
        (dd != 0).then(|| f64::from(n) / f64::from(dd))
    }
    fn sub_ifd(&self, ifd: usize, tag: u16) -> Option<usize> {
        let e = self.find(ifd, tag)?;
        self.u32(e + 8).map(|v| v as usize)
    }
}

const EDITORS: [&str; 10] = [
    "photoshop",
    "lightroom",
    "gimp",
    "darktable",
    "rawtherapee",
    "capture one",
    "affinity",
    "snapseed",
    "luminar",
    "pixelmator",
];

/// Fills `h` from a TIFF-structured EXIF payload.
pub fn read_exif(tiff: &[u8], h: &mut Hints) {
    let Some(t) = Tiff::new(tiff) else { return };
    let Some(ifd0) = t.ifd0() else { return };
    h.has_exif = true;
    let make = t.find(ifd0, 0x010F).and_then(|e| t.ascii(e));
    let model = t.find(ifd0, 0x0110).and_then(|e| t.ascii(e));
    if make.is_some() || model.is_some() {
        h.camera = Some(fnv_str(&format!(
            "{} {}",
            make.as_deref().unwrap_or(""),
            model.as_deref().unwrap_or("")
        )));
    }
    if let Some(sw) = t.find(ifd0, 0x0131).and_then(|e| t.ascii(e)) {
        let sw = sw.to_lowercase();
        h.edited_by_software = EDITORS.iter().any(|e| sw.contains(e));
    }
    let dt0 = t.find(ifd0, 0x0132).and_then(|e| t.ascii(e));
    let mut time = None;
    if let Some(x) = t.sub_ifd(ifd0, 0x8769) {
        let original = t.find(x, 0x9003).and_then(|e| t.ascii(e));
        let sub = t.find(x, 0x9291).and_then(|e| t.ascii(e));
        time = original
            .as_deref()
            .and_then(|s| parse_time_ms(s, sub.as_deref()));
        if let Some(e) = t.find(x, 0x920A) {
            h.focal_mm = t.rational(e, 0).map(|v| v as f32).filter(|v| *v > 0.0);
        }
        if let Some(s) = t.find(x, 0xA431).and_then(|e| t.ascii(e)) {
            h.serial = Some(fnv_str(&s));
        }
        if let Some(s) = t.find(x, 0xA420).and_then(|e| t.ascii(e)) {
            h.unique_id = (s.trim_matches('0').len() > 2).then(|| fnv_str(&s));
        }
        if let Some(v) = t.find(x, 0xA402).and_then(|e| t.short(e)) {
            h.bracket = v == 2;
        }
        if let Some(e) = t.find(x, 0x927C)
            && let Some(mn) = t.data(e)
        {
            read_apple_makernote(mn, h);
        }
    }
    if time.is_none() {
        time = dt0.as_deref().and_then(|s| parse_time_ms(s, None));
    }
    if let Some((ms, sub)) = time {
        h.time_ms = Some(ms);
        h.subsec = sub;
    }
    if let Some(g) = t.sub_ifd(ifd0, 0x8825) {
        let coord = |tag: u16, rtag: u16| -> Option<f64> {
            let e = t.find(g, tag)?;
            let v = t.rational(e, 0)? + t.rational(e, 1)? / 60.0 + t.rational(e, 2)? / 3600.0;
            let r = t
                .find(g, rtag)
                .and_then(|e| t.data(e))
                .and_then(|d| d.first().copied());
            Some(if matches!(r, Some(b'S' | b'W')) {
                -v
            } else {
                v
            })
        };
        if let (Some(lat), Some(lon)) = (coord(2, 1), coord(4, 3))
            && (lat != 0.0 || lon != 0.0)
            && lat.abs() <= 90.0
            && lon.abs() <= 180.0
        {
            h.gps = Some((lat, lon));
        }
    }
}

/// Apple's maker note ("Apple iOS"): tag 0x000B is the BurstUUID.
fn read_apple_makernote(mn: &[u8], h: &mut Hints) {
    if !mn.starts_with(b"Apple iOS\0") || mn.len() < 16 {
        return;
    }
    // Big-endian IFD at 14; offsets relative to the maker note start.
    let t = Tiff { b: mn, le: false };
    if let Some(e) = t.find(14, 0x000B)
        && let Some(s) = t.ascii(e)
    {
        h.burst = Some(fnv_str(&s));
    }
}

/// The value of an XMP property, as an attribute (`name="v"`) or a simple element.
pub fn xmp_value<'a>(x: &'a str, name: &str) -> Option<&'a str> {
    let i = x.find(name)?;
    let rest = &x[i + name.len()..];
    let v = if let Some(r) = rest.strip_prefix("=\"") {
        &r[..r.find('"')?]
    } else if let Some(r) = rest.strip_prefix("='") {
        &r[..r.find('\'')?]
    } else if let Some(r) = rest.strip_prefix('>') {
        &r[..r.find('<')?]
    } else {
        // A longer name sharing the prefix: look further on.
        return xmp_value(rest, name);
    };
    let v = v.trim();
    (!v.is_empty()).then_some(v)
}

/// Fills `h` from an XMP packet.
pub fn read_xmp(x: &str, h: &mut Hints) {
    let id = |n: &str| xmp_value(x, n).map(fnv_str);
    h.document_id = h.document_id.or_else(|| id("xmpMM:DocumentID"));
    h.original_document_id = h
        .original_document_id
        .or_else(|| id("xmpMM:OriginalDocumentID"));
    if let Some(i) = x.find("xmpMM:DerivedFrom") {
        let rest = &x[i..];
        let end = rest
            .find("</xmpMM:DerivedFrom>")
            .unwrap_or(rest.len().min(2048));
        let part = &rest[..end];
        h.derived_from = xmp_value(part, "stRef:documentID")
            .or_else(|| xmp_value(part, "stRef:originalDocumentID"))
            .map(fnv_str);
        if h.derived_from.is_none() {
            // A DerivedFrom without ids still says "derivative".
            h.derived_from = Some(1);
        }
    }
    if h.burst.is_none() {
        h.burst = id("GCamera:BurstID");
    }
    if h.time_ms.is_none()
        && let Some((ms, sub)) = xmp_value(x, "exif:DateTimeOriginal")
            .or_else(|| xmp_value(x, "photoshop:DateCreated"))
            .and_then(|s| parse_time_ms(s, None))
    {
        h.time_ms = Some(ms);
        h.subsec = sub;
    }
    if let Some(tool) = xmp_value(x, "xmp:CreatorTool") {
        let tool = tool.to_lowercase();
        h.edited_by_software |= EDITORS.iter().any(|e| tool.contains(e));
    }
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Hints from the first bytes of a file (256 KiB is plenty): EXIF in JPEG APP1, a TIFF-based
/// RAW's own IFDs, an `Exif\0\0` block anywhere in the head (HEIC, PNG eXIf, WebP), and the
/// first XMP packet.
pub fn read_head(head: &[u8]) -> Hints {
    let mut h = Hints::default();
    if head.starts_with(&[0xFF, 0xD8]) {
        if let Some(t) = jpeg_app1(head, b"Exif\0\0") {
            read_exif(t, &mut h);
        }
    } else if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        read_exif(head, &mut h);
    } else if let Some(i) = find_bytes(&head[..head.len().min(1 << 18)], b"Exif\0\0") {
        read_exif(&head[i + 6..], &mut h);
    } else if let Some(i) = find_bytes(&head[..head.len().min(1 << 18)], b"eXIf") {
        // PNG eXIf: the TIFF payload follows the chunk type.
        read_exif(&head[i + 4..], &mut h);
    }
    if let Some(i) = find_bytes(head, b"<x:xmpmeta") {
        let rest = &head[i..];
        let end = find_bytes(rest, b"</x:xmpmeta>").map_or(rest.len(), |e| e + 12);
        if let Ok(x) = std::str::from_utf8(&rest[..end]) {
            read_xmp(x, &mut h);
        }
    }
    h
}

/// The payload of the first APP1 segment starting with `sig` (without it).
fn jpeg_app1<'a>(head: &'a [u8], sig: &[u8]) -> Option<&'a [u8]> {
    let mut i = 2;
    while i + 4 <= head.len() {
        if head[i] != 0xFF {
            return None;
        }
        let m = head[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        if m == 0xDA || m == 0xD9 {
            return None;
        }
        let len = u16::from_be_bytes([head[i + 2], head[i + 3]]) as usize;
        let seg = head.get(i + 4..(i + 2 + len).min(head.len()))?;
        if m == 0xE1 && seg.starts_with(sig) {
            return Some(&seg[sig.len()..]);
        }
        i += 2 + len;
    }
    None
}

/// What an XMP sidecar (or embedded packet) says the user did: such an image is always kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UserFacts {
    pub rated: bool,
    pub labelled: bool,
    pub flagged: bool,
    pub tagged: bool,
    pub edited: bool,
}

impl UserFacts {
    pub fn any(&self) -> bool {
        self.rated || self.labelled || self.flagged || self.tagged || self.edited
    }
}

/// Reads [`UserFacts`] from XMP text.
pub fn user_facts(x: &str) -> UserFacts {
    let rating = xmp_value(x, "xmp:Rating").and_then(|v| v.parse::<f32>().ok());
    let history_end = xmp_value(x, "darktable:history_end").and_then(|v| v.parse::<u32>().ok());
    UserFacts {
        // −1 is Lightroom's reject; it is a flag.
        rated: rating.is_some_and(|r| r > 0.0),
        flagged: rating.is_some_and(|r| r < 0.0) || xmp_value(x, "xmpDM:pick").is_some(),
        labelled: xmp_value(x, "xmp:Label").is_some(),
        tagged: x.contains("<dc:subject") || x.contains("lr:hierarchicalSubject"),
        edited: xmp_value(x, "crs:HasSettings").is_some_and(|v| v.eq_ignore_ascii_case("true"))
            || history_end.is_some_and(|n| n > 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        let (a, s) = parse_time_ms("2024:03:12 10:20:30", Some("45")).unwrap();
        assert!(s);
        let (b, _) = parse_time_ms("2024:03:12 10:20:30", None).unwrap();
        assert_eq!(a - b, 450);
        let (c, s) = parse_time_ms("2024-03-12T10:20:30.120", None).unwrap();
        assert!(s);
        assert_eq!(c - b, 120);
        assert!(parse_time_ms("0000:00:00 00:00:00", None).is_none());
    }

    #[test]
    fn xmp_ids_and_user_facts() {
        let x = r#"<x:xmpmeta><rdf:Description xmpMM:DocumentID="xmp.did:1"
            xmp:Rating="3" crs:HasSettings="True">
            <xmpMM:DerivedFrom stRef:documentID="xmp.did:0"/></rdf:Description></x:xmpmeta>"#;
        let mut h = Hints::default();
        read_xmp(x, &mut h);
        assert_eq!(h.document_id, Some(fnv_str("xmp.did:1")));
        assert_eq!(h.derived_from, Some(fnv_str("xmp.did:0")));
        let u = user_facts(x);
        assert!(u.rated && u.edited && !u.tagged);
        assert!(
            user_facts("<dc:subject><rdf:Bag><rdf:li>x</rdf:li></rdf:Bag></dc:subject>").tagged
        );
        assert!(!user_facts("<x:xmpmeta/>").any());
    }

    /// A tiny little-endian EXIF block with Make, DateTimeOriginal, SubSecTimeOriginal,
    /// BodySerialNumber and GPS.
    fn exif_block() -> Vec<u8> {
        let mut b = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
        let ent = |tag: u16, typ: u16, count: u32, val: u32| {
            let mut e = Vec::new();
            e.extend_from_slice(&tag.to_le_bytes());
            e.extend_from_slice(&typ.to_le_bytes());
            e.extend_from_slice(&count.to_le_bytes());
            e.extend_from_slice(&val.to_le_bytes());
            e
        };
        // IFD0 at 8: Make (inline "Ab\0"), ExifIFD → 50, GPS → 120.
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend(ent(0x010F, 2, 3, u32::from_le_bytes([b'A', b'b', 0, 0])));
        b.extend(ent(0x8769, 4, 1, 50));
        b.extend(ent(0x8825, 4, 1, 120));
        b.extend_from_slice(&0u32.to_le_bytes());
        b.resize(50, 0);
        // Exif IFD at 50: DateTimeOriginal → 200, SubSec inline "25", Serial inline "123".
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend(ent(0x9003, 2, 20, 200));
        b.extend(ent(0x9291, 2, 3, u32::from_le_bytes([b'2', b'5', 0, 0])));
        b.extend(ent(0xA431, 2, 4, u32::from_le_bytes([b'1', b'2', b'3', 0])));
        b.resize(120, 0);
        // GPS IFD at 120: N 38°30'0", W 9°0'0".
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend(ent(1, 2, 2, u32::from_le_bytes([b'N', 0, 0, 0])));
        b.extend(ent(2, 5, 3, 240));
        b.extend(ent(3, 2, 2, u32::from_le_bytes([b'W', 0, 0, 0])));
        b.extend(ent(4, 5, 3, 264));
        b.resize(200, 0);
        b.extend_from_slice(b"2023:07:01 12:00:00\0");
        b.resize(240, 0);
        for (n, d) in [(38u32, 1u32), (30, 1), (0, 1), (9, 1), (0, 1), (0, 1)] {
            b.extend_from_slice(&n.to_le_bytes());
            b.extend_from_slice(&d.to_le_bytes());
        }
        b
    }

    #[test]
    fn exif_fields() {
        let mut h = Hints::default();
        read_exif(&exif_block(), &mut h);
        assert!(h.has_exif && h.subsec);
        assert_eq!(h.serial, Some(fnv_str("123")));
        let (want, _) = parse_time_ms("2023:07:01 12:00:00", Some("25")).unwrap();
        assert_eq!(h.time_ms, Some(want));
        let (lat, lon) = h.gps.unwrap();
        assert!((lat - 38.5).abs() < 1e-9 && (lon + 9.0).abs() < 1e-9);
        // Garbage never panics.
        for n in 0..exif_block().len() {
            let mut g = exif_block();
            g.truncate(n);
            read_exif(&g, &mut Hints::default());
        }
    }
}
