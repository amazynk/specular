//! ASTM E57: a file of checksummed pages (each page's last four bytes a CRC-32C of the rest)
//! holding an XML section that describes the scans and binary sections with their points.
//! Points are records of bit-packed fields (floats, integers in their range's bits, scaled
//! integers), each field a byte stream spread over data packets. Scans are read in
//! cartesian or spherical coordinates, with intensity and colour scaled by their limits, and
//! put in place by their poses. Images are not read.

use rayon::prelude::*;

use crate::build::Collector;
use crate::{Channels, CloudInfo, Error, Limits, PointCloud, PointFormat, Result, Scan, malformed};

/// XML elements at most.
const MAX_NODES: usize = 1 << 20;
/// XML nesting at most.
const MAX_DEPTH: usize = 128;

// ---------------------------------------------------------------------------------------
// CRC-32C (Castagnoli), slicing by eight.

const fn crc_tables() -> [[u32; 256]; 8] {
    let mut t = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0x82F6_3B78
            } else {
                c >> 1
            };
            k += 1;
        }
        t[0][i] = c;
        i += 1;
    }
    let mut s = 1;
    while s < 8 {
        let mut i = 0;
        while i < 256 {
            let p = t[s - 1][i];
            t[s][i] = (p >> 8) ^ t[0][(p & 0xFF) as usize];
            i += 1;
        }
        s += 1;
    }
    t
}

static CRC: [[u32; 256]; 8] = crc_tables();

pub(crate) fn crc32c(data: &[u8]) -> u32 {
    let mut c = !0u32;
    let mut chunks = data.chunks_exact(8);
    for b in &mut chunks {
        let v = c ^ u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        c = CRC[7][(v & 0xFF) as usize]
            ^ CRC[6][((v >> 8) & 0xFF) as usize]
            ^ CRC[5][((v >> 16) & 0xFF) as usize]
            ^ CRC[4][(v >> 24) as usize]
            ^ CRC[3][usize::from(b[4])]
            ^ CRC[2][usize::from(b[5])]
            ^ CRC[1][usize::from(b[6])]
            ^ CRC[0][usize::from(b[7])];
    }
    for &b in chunks.remainder() {
        c = (c >> 8) ^ CRC[0][((c ^ u32::from(b)) & 0xFF) as usize];
    }
    !c
}

// ---------------------------------------------------------------------------------------
// Pages.

/// The file as logical bytes (pages without their checksums).
struct Pages<'a> {
    b: &'a [u8],
    page: usize,
    data: usize,
}

impl Pages<'_> {
    fn len(&self) -> u64 {
        let full = self.b.len() / self.page;
        let rest = (self.b.len() % self.page).min(self.data);
        (full * self.data + rest) as u64
    }

    /// The logical offset of a physical one.
    fn logical(&self, phys: u64) -> Result<u64> {
        let (p, r) = (phys / self.page as u64, phys % self.page as u64);
        if r >= self.data as u64 || phys >= self.b.len() as u64 {
            return malformed(format!("E57 offset {phys} is not in a page's data"));
        }
        Ok(p * self.data as u64 + r)
    }

    /// `len` logical bytes from `at` into `out` (replacing its contents).
    fn copy(&self, at: u64, len: usize, out: &mut Vec<u8>) -> Result<()> {
        out.clear();
        if at.checked_add(len as u64).is_none_or(|e| e > self.len()) {
            return malformed("E57 section runs past the end of the file");
        }
        out.reserve(len);
        let mut at = at as usize;
        let mut left = len;
        while left > 0 {
            let (p, r) = (at / self.data, at % self.data);
            let n = (self.data - r).min(left);
            let s = p * self.page + r;
            out.extend_from_slice(&self.b[s..s + n]);
            at += n;
            left -= n;
        }
        Ok(())
    }

    /// Checks every page's checksum (stored in either byte order).
    fn verify(&self) -> Result<()> {
        let bad = self
            .b
            .par_chunks(self.page)
            .enumerate()
            .find_first(|(_, p)| {
                if p.len() < self.page {
                    return false;
                }
                let c = crc32c(&p[..self.data]);
                let s: [u8; 4] = p[self.data..].try_into().expect("4 bytes");
                c != u32::from_be_bytes(s) && c != u32::from_le_bytes(s)
            });
        match bad {
            Some((i, _)) => malformed(format!("E57 page {i} fails its checksum")),
            None => Ok(()),
        }
    }
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().expect("8 bytes"))
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

// ---------------------------------------------------------------------------------------
// XML.

#[derive(Default, Debug)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    kids: Vec<usize>,
}

struct Dom {
    nodes: Vec<Node>,
}

impl Dom {
    fn parse(xml: &[u8]) -> Result<Dom> {
        use quick_xml::events::Event;
        let mut r = quick_xml::Reader::from_reader(xml);
        let mut nodes = vec![Node::default()];
        let mut stack = vec![0usize];
        let err = |e: &dyn std::fmt::Display| Error::Malformed(format!("E57 XML: {e}"));
        loop {
            let ev = r.read_event().map_err(|e| err(&e))?;
            match ev {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    if nodes.len() >= MAX_NODES || stack.len() > MAX_DEPTH {
                        return Err(Error::TooLarge("E57 XML section".into()));
                    }
                    let attrs = e
                        .attributes()
                        .with_checks(false)
                        .flatten()
                        .map(|a| {
                            (
                                String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned(),
                                String::from_utf8_lossy(&a.value).into_owned(),
                            )
                        })
                        .collect();
                    let id = nodes.len();
                    nodes.push(Node {
                        name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                        attrs,
                        ..Node::default()
                    });
                    let parent = *stack.last().expect("root");
                    nodes[parent].kids.push(id);
                    if matches!(ev, Event::Start(_)) {
                        stack.push(id);
                    }
                }
                Event::End(_) => {
                    if stack.len() > 1 {
                        stack.pop();
                    }
                }
                Event::Text(ref t) => {
                    let top = *stack.last().expect("root");
                    nodes[top].text.push_str(&String::from_utf8_lossy(t));
                }
                Event::CData(ref t) => {
                    let top = *stack.last().expect("root");
                    nodes[top].text.push_str(&String::from_utf8_lossy(t));
                }
                Event::GeneralRef(ref g) => {
                    let name = String::from_utf8_lossy(g);
                    if let Ok(s) = quick_xml::escape::unescape(&format!("&{name};")) {
                        let top = *stack.last().expect("root");
                        nodes[top].text.push_str(&s);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(Dom { nodes })
    }

    fn kid(&self, n: usize, name: &str) -> Option<usize> {
        self.nodes[n]
            .kids
            .iter()
            .copied()
            .find(|&k| self.nodes[k].name == name)
    }

    fn path(&self, n: usize, names: &[&str]) -> Option<usize> {
        names.iter().try_fold(n, |n, name| self.kid(n, name))
    }

    fn attr(&self, n: usize, key: &str) -> Option<&str> {
        self.nodes[n]
            .attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.trim())
    }

    /// An element's number (an empty Float or Integer is 0).
    fn number(&self, n: usize) -> Option<f64> {
        let t = self.nodes[n].text.trim();
        if t.is_empty() {
            return Some(0.0);
        }
        t.parse().ok()
    }

    fn number_at(&self, n: usize, names: &[&str]) -> Option<f64> {
        self.path(n, names).and_then(|k| self.number(k))
    }

    fn text_at(&self, n: usize, name: &str) -> Option<String> {
        let t = self.nodes[self.kid(n, name)?].text.trim();
        (!t.is_empty()).then(|| t.to_string())
    }
}

// ---------------------------------------------------------------------------------------
// Fields.

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Float {
        double: bool,
    },
    Int {
        min: i64,
        bits: u32,
    },
    Scaled {
        min: i64,
        bits: u32,
        scale: f64,
        offset: f64,
    },
}

impl Kind {
    fn bits(self) -> u32 {
        match self {
            Kind::Float { double } => {
                if double {
                    64
                } else {
                    32
                }
            }
            Kind::Int { bits, .. } | Kind::Scaled { bits, .. } => bits,
        }
    }

    #[inline]
    fn value(self, raw: u64) -> f64 {
        match self {
            Kind::Float { double: true } => f64::from_bits(raw),
            Kind::Float { double: false } => f64::from(f32::from_bits(raw as u32)),
            Kind::Int { min, .. } => (i128::from(min) + i128::from(raw)) as f64,
            Kind::Scaled {
                min, scale, offset, ..
            } => (i128::from(min) + i128::from(raw)) as f64 * scale + offset,
        }
    }

    /// The field's range of values, where its type gives one.
    fn range(self, dom: &Dom, n: usize) -> Option<[f64; 2]> {
        let lo: f64 = dom.attr(n, "minimum")?.parse().ok()?;
        let hi: f64 = dom.attr(n, "maximum")?.parse().ok()?;
        match self {
            Kind::Scaled { scale, offset, .. } => Some([lo * scale + offset, hi * scale + offset]),
            _ => Some([lo, hi]),
        }
    }
}

fn bits_for(min: i64, max: i64) -> Result<u32> {
    if max < min {
        return malformed("E57 integer field with maximum below minimum");
    }
    let span = (i128::from(max) - i128::from(min)) as u128;
    Ok(128 - span.leading_zeros())
}

fn kind_of(dom: &Dom, n: usize) -> Result<Option<Kind>> {
    let int = |key: &str, default: i64| -> Result<i64> {
        match dom.attr(n, key) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|_| Error::Malformed(format!("E57 field {key} {v}"))),
        }
    };
    Ok(match dom.attr(n, "type") {
        Some("Float") => Some(Kind::Float {
            double: dom.attr(n, "precision") != Some("single"),
        }),
        Some("Integer") => {
            let (min, max) = (int("minimum", i64::MIN)?, int("maximum", i64::MAX)?);
            Some(Kind::Int {
                min,
                bits: bits_for(min, max)?,
            })
        }
        Some("ScaledInteger") => {
            let (min, max) = (int("minimum", i64::MIN)?, int("maximum", i64::MAX)?);
            let f = |k: &str, d: f64| dom.attr(n, k).and_then(|v| v.parse().ok()).unwrap_or(d);
            Some(Kind::Scaled {
                min,
                bits: bits_for(min, max)?,
                scale: f("scale", 1.0),
                offset: f("offset", 0.0),
            })
        }
        _ => None,
    })
}

/// What a field is read into.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Slot {
    X,
    Y,
    Z,
    Range,
    Azimuth,
    Elevation,
    Invalid,
    Intensity,
    Red,
    Green,
    Blue,
    NoIntensity,
    NoColour,
}

const SLOTS: usize = 13;

fn slot_of(name: &str) -> Option<Slot> {
    Some(match name {
        "cartesianX" => Slot::X,
        "cartesianY" => Slot::Y,
        "cartesianZ" => Slot::Z,
        "sphericalRange" => Slot::Range,
        "sphericalAzimuth" => Slot::Azimuth,
        "sphericalElevation" => Slot::Elevation,
        "cartesianInvalidState" | "sphericalInvalidState" => Slot::Invalid,
        "intensity" => Slot::Intensity,
        "colorRed" => Slot::Red,
        "colorGreen" => Slot::Green,
        "colorBlue" => Slot::Blue,
        "isIntensityInvalid" => Slot::NoIntensity,
        "isColorInvalid" => Slot::NoColour,
        _ => return None,
    })
}

#[derive(Debug)]
struct Field {
    kind: Kind,
    slot: Option<Slot>,
    range: Option<[f64; 2]>,
}

/// The prototype's fields in stream order (nested structures depth first).
fn fields(dom: &Dom, n: usize, top: bool, out: &mut Vec<Field>) -> Result<()> {
    for &k in &dom.nodes[n].kids {
        match dom.attr(k, "type") {
            Some("Structure") | Some("Vector") => fields(dom, k, false, out)?,
            _ => {
                let Some(kind) = kind_of(dom, k)? else {
                    return Err(Error::Unsupported(format!(
                        "E57 field {} of type {}",
                        dom.nodes[k].name,
                        dom.attr(k, "type").unwrap_or("?")
                    )));
                };
                let slot = if top {
                    slot_of(&dom.nodes[k].name)
                } else {
                    None
                };
                out.push(Field {
                    kind,
                    slot,
                    range: kind.range(dom, k),
                });
            }
        }
        if out.len() > 4096 {
            return malformed("E57 prototype with too many fields");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Bit streams.

#[derive(Default)]
struct Stream {
    buf: Vec<u8>,
    /// Bits read of `buf`.
    pos: u64,
}

impl Stream {
    fn available(&self, bits: u32) -> u64 {
        if bits == 0 {
            return u64::MAX;
        }
        (self.buf.len() as u64 * 8 - self.pos) / u64::from(bits)
    }

    #[inline]
    fn take(&mut self, bits: u32) -> u64 {
        if bits == 0 {
            return 0;
        }
        let byte = (self.pos / 8) as usize;
        let sh = (self.pos % 8) as u32;
        let mut w = [0u8; 9];
        let end = (byte + 9).min(self.buf.len());
        w[..end - byte].copy_from_slice(&self.buf[byte..end]);
        let lo = u64::from_le_bytes(w[..8].try_into().expect("8 bytes"));
        let v = if sh == 0 {
            lo
        } else {
            (lo >> sh) | (u64::from(w[8]) << (64 - sh))
        };
        self.pos += u64::from(bits);
        if bits >= 64 {
            v
        } else {
            v & ((1u64 << bits) - 1)
        }
    }

    fn compact(&mut self) {
        let n = (self.pos / 8) as usize;
        self.buf.drain(..n);
        self.pos -= n as u64 * 8;
    }
}

/// Reads `records` records of the binary section at physical offset `at`, calling `sink`
/// with each record's slots.
fn records(
    pages: &Pages,
    at: u64,
    fields: &[Field],
    records: u64,
    mut sink: impl FnMut(&[f64; SLOTS]),
) -> Result<()> {
    let mut buf = Vec::new();
    let start = pages.logical(at)?;
    pages.copy(start, 32, &mut buf)?;
    if buf[0] != 1 {
        return malformed("E57 points are not a binary section");
    }
    let section_len = u64_at(&buf, 8);
    let data_phys = u64_at(&buf, 16);
    let end = start
        .checked_add(section_len)
        .filter(|&e| e <= pages.len())
        .ok_or_else(|| Error::Malformed("E57 binary section runs past the file".into()))?;
    if records == 0 {
        return Ok(());
    }
    let mut at = pages.logical(data_phys)?;
    let mut streams: Vec<Stream> = fields.iter().map(|_| Stream::default()).collect();
    let used: Vec<usize> = (0..fields.len())
        .filter(|&i| fields[i].slot.is_some())
        .collect();
    // Records of zero-bit (constant) fields take no bytes, so nothing in the section bounds
    // their count: hold it to the section's size, not up to 2⁴⁰ identical points.
    let records = if used.iter().all(|&i| fields[i].kind.bits() == 0) {
        records.min(section_len)
    } else {
        records
    };
    let mut vals = [0f64; SLOTS];
    let mut done = 0u64;
    while done < records {
        if at + 4 > end {
            return malformed("E57 binary section ends before its points");
        }
        pages.copy(at, 4, &mut buf)?;
        let kind = buf[0];
        let len = usize::from(u16_at(&buf, 2)) + 1;
        if at + len as u64 > end {
            return malformed("E57 packet runs past its section");
        }
        pages.copy(at, len, &mut buf)?;
        at += len as u64;
        match kind {
            1 => {}
            0 | 2 => continue,
            k => return malformed(format!("E57 packet type {k}")),
        }
        if len < 6 {
            return malformed("E57 data packet too short");
        }
        let count = usize::from(u16_at(&buf, 4));
        if count != fields.len() {
            return malformed(format!(
                "E57 data packet with {count} streams for {} fields",
                fields.len()
            ));
        }
        let mut off = 6 + 2 * count;
        if off > len {
            return malformed("E57 data packet header runs past it");
        }
        for (i, s) in streams.iter_mut().enumerate() {
            let n = usize::from(u16_at(&buf, 6 + 2 * i));
            let Some(chunk) = buf.get(off..off + n).filter(|_| off + n <= len) else {
                return malformed("E57 stream buffer runs past its packet");
            };
            if fields[i].slot.is_some() {
                s.buf.extend_from_slice(chunk);
            }
            off += n;
        }
        let ready = used
            .iter()
            .map(|&i| streams[i].available(fields[i].kind.bits()))
            .min()
            .unwrap_or(records - done)
            .min(records - done);
        for _ in 0..ready {
            for &i in &used {
                let f = &fields[i];
                let raw = streams[i].take(f.kind.bits());
                vals[f.slot.expect("used") as usize] = f.kind.value(raw);
            }
            sink(&vals);
        }
        done += ready;
        for &i in &used {
            streams[i].compact();
        }
        if used.is_empty() {
            break;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Scans.

/// A rigid pose: rotation matrix rows and translation.
#[derive(Clone, Copy, Debug)]
struct Pose {
    r: [[f64; 3]; 3],
    t: [f64; 3],
    identity: bool,
}

fn pose(dom: &Dom, scan: usize) -> Pose {
    let get = |p: &[&str], d: f64| dom.number_at(scan, p).unwrap_or(d);
    let (mut w, mut x, mut y, mut z) = (
        get(&["pose", "rotation", "w"], 1.0),
        get(&["pose", "rotation", "x"], 0.0),
        get(&["pose", "rotation", "y"], 0.0),
        get(&["pose", "rotation", "z"], 0.0),
    );
    let n = (w * w + x * x + y * y + z * z).sqrt();
    if n > 1e-12 && n.is_finite() {
        (w, x, y, z) = (w / n, x / n, y / n, z / n);
    } else {
        (w, x, y, z) = (1.0, 0.0, 0.0, 0.0);
    }
    let t = [
        get(&["pose", "translation", "x"], 0.0),
        get(&["pose", "translation", "y"], 0.0),
        get(&["pose", "translation", "z"], 0.0),
    ];
    let r = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - w * z),
            2.0 * (x * z + w * y),
        ],
        [
            2.0 * (x * y + w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - w * x),
        ],
        [
            2.0 * (x * z - w * y),
            2.0 * (y * z + w * x),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    let identity = (w.abs() - 1.0).abs() < 1e-12 && t.iter().all(|v| *v == 0.0);
    Pose { r, t, identity }
}

impl Pose {
    #[inline]
    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        if self.identity {
            return p;
        }
        let r = &self.r;
        [0, 1, 2].map(|i| r[i][0] * p[0] + r[i][1] * p[1] + r[i][2] * p[2] + self.t[i])
    }
}

/// A linear map of `[lo, hi]` onto `[0, top]`.
#[derive(Clone, Copy)]
struct Norm {
    lo: f64,
    k: f64,
    top: f64,
}

impl Norm {
    fn new(range: Option<[f64; 2]>, top: f64) -> Norm {
        let [lo, hi] = range
            .filter(|r| r[1] > r[0] && r.iter().all(|v| v.is_finite()))
            .unwrap_or([0.0, 1.0]);
        Norm {
            lo,
            k: top / (hi - lo),
            top,
        }
    }

    #[inline]
    fn map(self, v: f64) -> f64 {
        ((v - self.lo) * self.k).clamp(0.0, self.top)
    }
}

/// A limit from the scan's limits structure, else the field's own range.
fn limits(
    dom: &Dom,
    scan: usize,
    group: &str,
    lo: &str,
    hi: &str,
    field: Option<&Field>,
) -> Option<[f64; 2]> {
    let a = dom.number_at(scan, &[group, lo]);
    let b = dom.number_at(scan, &[group, hi]);
    match (a, b) {
        (Some(a), Some(b)) if b > a => Some([a, b]),
        _ => field.and_then(|f| f.range),
    }
}

pub(crate) fn read(bytes: &[u8], limits_: &Limits) -> Result<PointCloud> {
    if bytes.len() < 48 || !bytes.starts_with(b"ASTM-E57") {
        return malformed("not an E57 file");
    }
    let major = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
    let minor = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes"));
    let xml_phys = u64_at(bytes, 24);
    let xml_len = u64_at(bytes, 32);
    let page = u64_at(bytes, 40);
    if !(64..=1 << 20).contains(&page) {
        return malformed(format!("E57 page size {page}"));
    }
    let page = page as usize;
    let pages = Pages {
        b: bytes,
        page,
        data: page - 4,
    };
    if xml_len > pages.len() {
        return malformed("E57 XML section runs past the file");
    }
    pages.verify()?;
    let mut xml = Vec::new();
    pages.copy(pages.logical(xml_phys)?, xml_len as usize, &mut xml)?;
    let dom = Dom::parse(&xml)?;
    let root = dom
        .kid(0, "e57Root")
        .ok_or_else(|| Error::Malformed("E57 XML without e57Root".into()))?;
    let scans: Vec<usize> = dom
        .kid(root, "data3D")
        .map(|d| dom.nodes[d].kids.clone())
        .unwrap_or_default();

    // Each scan's fields and record count first, for the total.
    struct Plan {
        node: usize,
        fields: Vec<Field>,
        records: u64,
        at: u64,
    }
    let mut plans = Vec::new();
    let mut total = 0u64;
    for &s in &scans {
        let Some(pts) = dom.kid(s, "points") else {
            continue;
        };
        let num = |k: &str| -> Result<u64> {
            dom.attr(pts, k)
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| Error::Malformed(format!("E57 points without {k}")))
        };
        let records = num("recordCount")?;
        let at = num("fileOffset")?;
        let mut fs = Vec::new();
        if let Some(proto) = dom.kid(pts, "prototype") {
            fields(&dom, proto, true, &mut fs)?;
        }
        total = total.saturating_add(records);
        plans.push(Plan {
            node: s,
            fields: fs,
            records,
            at,
        });
    }
    if total > limits_.max_declared {
        return Err(Error::TooLarge(format!("{total} points")));
    }
    let mut c = Collector::new(limits_.max_points, total);
    let mut channels = Channels::default();
    let mut info_scans = Vec::new();
    for p in &plans {
        let has = |s: Slot| p.fields.iter().find(|f| f.slot == Some(s));
        let cartesian = has(Slot::X).is_some() && has(Slot::Y).is_some() && has(Slot::Z).is_some();
        let spherical = !cartesian
            && has(Slot::Range).is_some()
            && has(Slot::Azimuth).is_some()
            && has(Slot::Elevation).is_some();
        let pose = pose(&dom, p.node);
        info_scans.push(Scan {
            name: dom.text_at(p.node, "name"),
            points: p.records,
            spherical,
            posed: !pose.identity,
        });
        if !cartesian && !spherical {
            continue;
        }
        let rgb =
            has(Slot::Red).is_some() && has(Slot::Green).is_some() && has(Slot::Blue).is_some();
        let intensity = has(Slot::Intensity).is_some();
        channels.rgb |= rgb;
        channels.intensity |= intensity;
        let ni = Norm::new(
            limits(
                &dom,
                p.node,
                "intensityLimits",
                "intensityMinimum",
                "intensityMaximum",
                has(Slot::Intensity),
            ),
            65_535.0,
        );
        let nc = [
            ("colorRedMinimum", "colorRedMaximum", Slot::Red),
            ("colorGreenMinimum", "colorGreenMaximum", Slot::Green),
            ("colorBlueMinimum", "colorBlueMaximum", Slot::Blue),
        ]
        .map(|(lo, hi, s)| Norm::new(limits(&dom, p.node, "colorLimits", lo, hi, has(s)), 255.0));
        records(&pages, p.at, &p.fields, p.records, |v| {
            if v[Slot::Invalid as usize] != 0.0 {
                return;
            }
            let q = if cartesian {
                [
                    v[Slot::X as usize],
                    v[Slot::Y as usize],
                    v[Slot::Z as usize],
                ]
            } else {
                let (r, az, el) = (
                    v[Slot::Range as usize],
                    v[Slot::Azimuth as usize],
                    v[Slot::Elevation as usize],
                );
                let ce = el.cos();
                [r * ce * az.cos(), r * ce * az.sin(), r * el.sin()]
            };
            let colour = if rgb && v[Slot::NoColour as usize] == 0.0 {
                [
                    nc[0].map(v[Slot::Red as usize]).round() as u8,
                    nc[1].map(v[Slot::Green as usize]).round() as u8,
                    nc[2].map(v[Slot::Blue as usize]).round() as u8,
                ]
            } else {
                [0; 3]
            };
            let i = if intensity && v[Slot::NoIntensity as usize] == 0.0 {
                ni.map(v[Slot::Intensity as usize]).round() as u16
            } else {
                0
            };
            c.push(pose.apply(q), colour, 0, i, 0);
        })?;
    }
    let info = CloudInfo {
        format: Some(PointFormat::E57),
        version: Some(format!("{major}.{minor}")),
        total,
        channels,
        scans: info_scans,
        ..CloudInfo::default()
    };
    c.finish(info)
}

/// Pages logical bytes with big-endian checksums (for tests).
#[cfg(test)]
pub(crate) fn paginate(logical: &[u8], page: usize) -> Vec<u8> {
    let data = page - 4;
    let mut out = Vec::new();
    for chunk in logical.chunks(data) {
        let mut p = chunk.to_vec();
        p.resize(data, 0);
        let c = crc32c(&p);
        p.extend_from_slice(&c.to_be_bytes());
        out.extend_from_slice(&p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32c_check_value() {
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
        assert_eq!(crc32c(b""), 0);
        let long: Vec<u8> = (0..1000u32).map(|i| (i * 7) as u8).collect();
        let mut c = !0u32;
        for &b in &long {
            c = (c >> 8) ^ CRC[0][((c ^ u32::from(b)) & 0xFF) as usize];
        }
        assert_eq!(crc32c(&long), !c);
    }

    #[test]
    fn bit_stream() {
        let mut s = Stream {
            buf: vec![0b1010_1101, 0xFF, 0x01],
            pos: 0,
        };
        assert_eq!(s.available(3), 8);
        assert_eq!(s.take(3), 0b101);
        assert_eq!(s.take(5), 0b10101);
        assert_eq!(s.take(9), 0x1FF);
        s.compact();
        assert_eq!(s.buf.len(), 1);
        assert_eq!(bits_for(0, 255).unwrap(), 8);
        assert_eq!(bits_for(5, 5).unwrap(), 0);
        assert_eq!(bits_for(i64::MIN, i64::MAX).unwrap(), 64);
    }
}
