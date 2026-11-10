//! The `.imzML` XML: a streaming pass over mzML elements that keeps only what imaging needs
//! (file content, parameter groups, instrument, scan settings and, per spectrum, its
//! position and the two binary arrays' places in the `.ibd`).
//!
//! Parameters are matched by accession. A `referenceableParamGroupRef` stands for the
//! group's parameters at the place of the reference, wherever it appears.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{Array, DataType, Entry, Error, Mode, Polarity, Result};

/// One `cvParam` (or `userParam`, with an empty accession).
#[derive(Clone, Debug, Default)]
struct Param {
    accession: Vec<u8>,
    name: Vec<u8>,
    value: Vec<u8>,
}

/// What the parser found, before the grid is laid out.
#[derive(Debug, Default)]
pub(crate) struct Parsed {
    pub uuid: Option<[u8; 16]>,
    pub uuid_text: Option<String>,
    pub mode: Option<Mode>,
    pub entries: Vec<Entry>,
    pub pixel_size: (Option<f64>, Option<f64>),
    pub max_count: (Option<u32>, Option<u32>),
    pub instrument: Option<String>,
    pub positive: usize,
    pub negative: usize,
    pub centroid: Option<bool>,
    pub file_centroid: Option<bool>,
    pub lowest: Option<f64>,
    pub highest: Option<f64>,
    pub software: Option<String>,
}

impl Parsed {
    pub fn polarity(&self) -> Option<Polarity> {
        match (self.positive > 0, self.negative > 0) {
            (true, false) => Some(Polarity::Positive),
            (false, true) => Some(Polarity::Negative),
            (true, true) => Some(Polarity::Mixed),
            (false, false) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ctx {
    Other,
    FileContent,
    Group,
    Instrument,
    Component,
    ScanSettings,
    Software,
    Spectrum,
    Scan,
    ScanWindow,
    Array,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unknown,
    Mz,
    Intensity,
    Other,
}

#[derive(Debug)]
struct ArrayBuilder {
    kind: Kind,
    dtype: Option<DataType>,
    unknown_type: Option<String>,
    compression: Option<String>,
    offset: Option<u64>,
    len: Option<u64>,
    encoded: Option<u64>,
}

impl ArrayBuilder {
    fn new() -> ArrayBuilder {
        ArrayBuilder {
            kind: Kind::Unknown,
            dtype: None,
            unknown_type: None,
            compression: None,
            offset: None,
            len: None,
            encoded: None,
        }
    }
}

#[derive(Debug, Default)]
struct SpectrumBuilder {
    index: usize,
    id: String,
    default_len: Option<u64>,
    x: Option<u32>,
    y: Option<u32>,
    z: Option<u32>,
    mz: Option<Array>,
    intensity: Option<Array>,
    positive: bool,
    negative: bool,
    centroid: Option<bool>,
}

fn num<T: std::str::FromStr>(v: &[u8]) -> Option<T> {
    std::str::from_utf8(v).ok()?.trim().parse().ok()
}

/// A position: an integer, or a float that is one (some writers say `1.0`).
fn position(v: &[u8]) -> Option<u32> {
    if let Some(n) = num::<u32>(v) {
        return Some(n);
    }
    let f: f64 = num(v)?;
    (f >= 0.0 && f.fract() == 0.0 && f <= f64::from(u32::MAX)).then_some(f as u32)
}

/// The five predefined XML entities and numeric character references.
pub(crate) fn unescape(v: &[u8]) -> String {
    let s = String::from_utf8_lossy(v);
    if !s.contains('&') {
        return s.into_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = &*s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else { break };
        let ent = &rest[1..end];
        let c = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The 16 bytes of a UUID written as hex with any braces and dashes.
pub(crate) fn parse_uuid(s: &str) -> Option<[u8; 16]> {
    let hex: Vec<u8> = s
        .bytes()
        .filter(|b| !matches!(b, b'{' | b'}' | b'-' | b' '))
        .collect();
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        let pair = std::str::from_utf8(&hex[2 * i..2 * i + 2]).ok()?;
        *o = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

fn attr(e: &BytesStart<'_>, key: &[u8]) -> Option<Vec<u8>> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.local_name().as_ref() == key)
        .map(|a| a.value.into_owned())
}

fn param(e: &BytesStart<'_>, user: bool) -> Param {
    let mut p = Param::default();
    for a in e.attributes().with_checks(false).flatten() {
        match a.key.local_name().as_ref() {
            b"accession" if !user => p.accession = a.value.into_owned(),
            b"name" => p.name = a.value.into_owned(),
            b"value" => p.value = a.value.into_owned(),
            _ => {}
        }
    }
    p
}

/// Compression accessions other than "no compression" (MS:1000576): zlib, the MS-Numpress
/// family (alone and with zlib) and the truncation / delta schemes.
fn is_compression(acc: &[u8], name: &[u8]) -> bool {
    if acc == b"MS:1000576" {
        return false;
    }
    matches!(
        acc,
        b"MS:1000574"
            | b"MS:1002312"
            | b"MS:1002313"
            | b"MS:1002314"
            | b"MS:1002746"
            | b"MS:1002747"
            | b"MS:1002748"
            | b"MS:1003088"
            | b"MS:1003089"
            | b"MS:1003090"
            | b"MS:1003091"
            | b"MS:1003092"
            | b"MS:1003093"
    ) || (acc.is_empty()
        && name.windows(11).any(|w| w == b"compression")
        && name != b"no compression")
}

/// Most parameters one referenceable group may hold (its own and those of the groups it
/// refers to): a group referring twice to the previous one, 30 deep, would otherwise hold
/// 2^30 copies.
const MAX_GROUP_PARAMS: usize = 4096;
/// Most parameters read through group references in one file.
const MAX_REFERENCED_PARAMS: usize = 1 << 22;

struct Parser {
    out: Parsed,
    /// Parameters read through group references so far ([`MAX_REFERENCED_PARAMS`]).
    referenced: usize,
    groups: HashMap<Vec<u8>, Vec<Param>>,
    group: Option<(Vec<u8>, Vec<Param>)>,
    ctx: Vec<Ctx>,
    spectrum: Option<SpectrumBuilder>,
    array: Option<ArrayBuilder>,
    have_spectrum_list: bool,
    xml_len: usize,
}

impl Parser {
    fn top(&self) -> Ctx {
        *self.ctx.last().unwrap_or(&Ctx::Other)
    }

    fn within(&self, c: Ctx) -> bool {
        self.ctx.contains(&c)
    }

    fn open(&mut self, e: &BytesStart<'_>, empty: bool) -> Result<()> {
        let name = e.local_name();
        let ctx = match name.as_ref() {
            b"cvParam" | b"userParam" => {
                let p = param(e, name.as_ref() == b"userParam");
                self.param(&p)?;
                if !empty {
                    self.ctx.push(Ctx::Other);
                }
                return Ok(());
            }
            b"referenceableParamGroupRef" => {
                if let Some(r) = attr(e, b"ref") {
                    let Some(ps) = self.groups.get(&r).cloned() else {
                        return Err(Error::Malformed(format!(
                            "reference to the undefined parameter group {:?}",
                            unescape(&r)
                        )));
                    };
                    self.referenced += ps.len();
                    if self.referenced > MAX_REFERENCED_PARAMS {
                        return Err(Error::Malformed(
                            "too many parameter group references".into(),
                        ));
                    }
                    for p in &ps {
                        self.param(p)?;
                    }
                }
                if !empty {
                    self.ctx.push(Ctx::Other);
                }
                return Ok(());
            }
            b"fileContent" => Ctx::FileContent,
            b"referenceableParamGroup" => {
                let id = attr(e, b"id").unwrap_or_default();
                self.group = Some((id, Vec::new()));
                Ctx::Group
            }
            b"instrumentConfiguration" => Ctx::Instrument,
            b"componentList" => Ctx::Component,
            b"scanSettings" => Ctx::ScanSettings,
            b"software" => Ctx::Software,
            b"spectrumList" => {
                self.have_spectrum_list = true;
                if let Some(n) = attr(e, b"count").and_then(|c| num::<usize>(&c)) {
                    // A spectrum takes far more than 64 bytes of XML.
                    self.out.entries.reserve(n.min(self.xml_len / 64));
                }
                Ctx::Other
            }
            b"spectrum" => {
                let mut s = SpectrumBuilder {
                    index: self.out.entries.len(),
                    ..Default::default()
                };
                for a in e.attributes().with_checks(false).flatten() {
                    match a.key.local_name().as_ref() {
                        b"id" => s.id = unescape(&a.value),
                        b"defaultArrayLength" => s.default_len = num(&a.value),
                        _ => {}
                    }
                }
                self.spectrum = Some(s);
                Ctx::Spectrum
            }
            b"scan" => Ctx::Scan,
            b"scanWindow" => Ctx::ScanWindow,
            b"binaryDataArray" => {
                let mut a = ArrayBuilder::new();
                a.encoded = attr(e, b"encodedLength")
                    .and_then(|v| num::<u64>(&v))
                    .filter(|&n| n > 0);
                self.array = Some(a);
                Ctx::Array
            }
            _ => Ctx::Other,
        };
        if empty {
            self.close(ctx)?;
        } else {
            self.ctx.push(ctx);
        }
        Ok(())
    }

    fn close(&mut self, ctx: Ctx) -> Result<()> {
        match ctx {
            Ctx::Group => {
                if let Some((id, ps)) = self.group.take() {
                    self.groups.insert(id, ps);
                }
            }
            Ctx::Array => {
                if let (Some(a), Some(s)) = (self.array.take(), self.spectrum.as_mut()) {
                    finish_array(a, s)?;
                }
            }
            Ctx::Spectrum => {
                if let Some(s) = self.spectrum.take() {
                    self.finish_spectrum(s)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn finish_spectrum(&mut self, s: SpectrumBuilder) -> Result<()> {
        let what = || {
            if s.id.is_empty() {
                format!("spectrum {}", s.index)
            } else {
                format!("spectrum {:?}", s.id)
            }
        };
        let (Some(x), Some(y)) = (s.x, s.y) else {
            return Err(Error::Malformed(format!(
                "{} has no pixel position",
                what()
            )));
        };
        let fix = |a: Option<Array>, which: &str| -> Result<Array> {
            let mut a = a.ok_or_else(|| {
                Error::Malformed(format!("{} has no external {which} array", what()))
            })?;
            if a.len == u64::MAX {
                a.len = s.default_len.ok_or_else(|| {
                    Error::Malformed(format!("{} gives no {which} array length", what()))
                })?;
            }
            Ok(a)
        };
        let mz = fix(s.mz, "m/z")?;
        let intensity = fix(s.intensity, "intensity")?;
        if mz.len != intensity.len {
            return Err(Error::Malformed(format!(
                "{}: m/z and intensity arrays differ in length ({} and {})",
                what(),
                mz.len,
                intensity.len
            )));
        }
        if s.positive {
            self.out.positive += 1;
        }
        if s.negative {
            self.out.negative += 1;
        }
        if let Some(c) = s.centroid {
            self.out.centroid = Some(self.out.centroid.unwrap_or(c) && c);
        }
        self.out.entries.push(Entry {
            x,
            y,
            z: s.z.unwrap_or(1),
            mz,
            intensity,
        });
        Ok(())
    }

    fn param(&mut self, p: &Param) -> Result<()> {
        let in_group = self.top() == Ctx::Group;
        if let Some((_, ps)) = self.group.as_mut()
            && in_group
        {
            if ps.len() >= MAX_GROUP_PARAMS {
                return Err(Error::Malformed(
                    "a parameter group with too many parameters".into(),
                ));
            }
            ps.push(p.clone());
            return Ok(());
        }
        let acc = p.accession.as_slice();
        match self.top() {
            Ctx::FileContent => match acc {
                b"IMS:1000080" => {
                    let t = unescape(&p.value);
                    self.out.uuid = parse_uuid(&t);
                    self.out.uuid_text = Some(t);
                }
                b"IMS:1000030" => self.out.mode = Some(Mode::Continuous),
                b"IMS:1000031" => self.out.mode = Some(Mode::Processed),
                b"MS:1000127" => self.out.file_centroid = Some(true),
                b"MS:1000128" => self.out.file_centroid = Some(false),
                _ => {}
            },
            Ctx::ScanSettings => match acc {
                b"IMS:1000046" => self.out.pixel_size.0 = num(&p.value),
                b"IMS:1000047" => self.out.pixel_size.1 = num(&p.value),
                b"IMS:1000042" => self.out.max_count.0 = num(&p.value),
                b"IMS:1000043" => self.out.max_count.1 = num(&p.value),
                _ => {}
            },
            Ctx::Instrument if self.out.instrument.is_none() && !self.within(Ctx::Component) => {
                // The model is the instrument configuration's own term (MS:1000031's
                // children); skip its serial number and generic attributes.
                if !matches!(acc, b"MS:1000529" | b"MS:1000032") && !p.accession.is_empty() {
                    let v = if acc == b"MS:1000031" && !p.value.is_empty() {
                        unescape(&p.value)
                    } else {
                        unescape(&p.name)
                    };
                    if !v.is_empty() {
                        self.out.instrument = Some(v);
                    }
                }
            }
            Ctx::Software => {
                if self.out.software.is_none() && !p.accession.is_empty() && !p.name.is_empty() {
                    self.out.software = Some(unescape(&p.name));
                }
            }
            Ctx::Spectrum | Ctx::Scan | Ctx::ScanWindow => {
                if let Some(s) = self.spectrum.as_mut() {
                    match acc {
                        b"IMS:1000050" => s.x = position(&p.value),
                        b"IMS:1000051" => s.y = position(&p.value),
                        b"IMS:1000052" => s.z = position(&p.value),
                        b"MS:1000130" => s.positive = true,
                        b"MS:1000129" => s.negative = true,
                        b"MS:1000127" => s.centroid = Some(true),
                        b"MS:1000128" => s.centroid = Some(false),
                        b"MS:1000528" => {
                            if let Some(v) = num::<f64>(&p.value) {
                                self.out.lowest = Some(self.out.lowest.map_or(v, |l| l.min(v)));
                            }
                        }
                        b"MS:1000527" => {
                            if let Some(v) = num::<f64>(&p.value) {
                                self.out.highest = Some(self.out.highest.map_or(v, |h| h.max(v)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ctx::Array => {
                if let Some(a) = self.array.as_mut() {
                    array_param(a, p);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn array_param(a: &mut ArrayBuilder, p: &Param) {
    let acc = p.accession.as_slice();
    match acc {
        b"MS:1000514" => a.kind = Kind::Mz,
        b"MS:1000515" => a.kind = Kind::Intensity,
        b"MS:1000521" => a.dtype = Some(DataType::F32),
        b"MS:1000523" => a.dtype = Some(DataType::F64),
        b"MS:1000519" | b"IMS:1000141" => a.dtype = Some(DataType::I32),
        b"MS:1000522" | b"IMS:1000142" => a.dtype = Some(DataType::I64),
        b"IMS:1000140" | b"MS:1000520" | b"IMS:1000143" | b"IMS:1000144" | b"IMS:1000145"
        | b"IMS:1000146" | b"IMS:1000147" => a.unknown_type = Some(unescape(&p.name)),
        // Some writers wrote offsets past 2 GiB as signed 32-bit numbers: kept as their
        // two's complement here and unwrapped once all spectra are read.
        b"IMS:1000102" => a.offset = num::<i64>(&p.value).map(|v| v as u64),
        b"IMS:1000103" => a.len = num(&p.value),
        b"IMS:1000104" => a.encoded = num(&p.value),
        _ => {
            if a.kind == Kind::Unknown && acc.starts_with(b"MS:") && p.name.ends_with(b" array") {
                a.kind = Kind::Other;
            }
            if is_compression(acc, &p.name) {
                a.compression = Some(if p.name.is_empty() {
                    String::from_utf8_lossy(acc).into_owned()
                } else {
                    unescape(&p.name)
                });
            }
        }
    }
}

fn finish_array(a: ArrayBuilder, s: &mut SpectrumBuilder) -> Result<()> {
    let which = match a.kind {
        Kind::Mz => "m/z",
        Kind::Intensity => "intensity",
        _ => return Ok(()),
    };
    if let Some(c) = a.compression {
        return Err(Error::Compressed {
            array: which.into(),
            compression: c,
        });
    }
    let dtype = match (a.dtype, a.unknown_type) {
        (Some(t), _) => t,
        (None, Some(n)) => return Err(Error::UnsupportedType(n)),
        (None, None) => {
            return Err(Error::Malformed(format!(
                "the {which} array of spectrum {} has no data type",
                s.index
            )));
        }
    };
    let offset = a.offset.ok_or_else(|| {
        Error::Malformed(format!(
            "the {which} array of spectrum {} has no external offset (inline arrays are not imzML)",
            s.index
        ))
    })?;
    let len = match (a.len, a.encoded) {
        (Some(n), _) => n,
        (None, Some(e)) => e / dtype.size() as u64,
        (None, None) => u64::MAX,
    };
    if let (Some(e), true) = (a.encoded, len != u64::MAX)
        && len.checked_mul(dtype.size() as u64) != Some(e)
    {
        return Err(Error::Malformed(format!(
            "the {which} array of spectrum {}: {len} values of {} take {} bytes, not the {e} encoded",
            s.index,
            dtype.name(),
            len.saturating_mul(dtype.size() as u64)
        )));
    }
    let arr = Array { offset, len, dtype };
    match a.kind {
        Kind::Mz => s.mz = Some(arr),
        _ => s.intensity = Some(arr),
    }
    Ok(())
}

/// Parses the `.imzML` document.
pub(crate) fn parse(xml: &[u8]) -> Result<Parsed> {
    let mut r = Reader::from_reader(xml);
    r.config_mut().check_end_names = false;
    let mut p = Parser {
        out: Parsed::default(),
        referenced: 0,
        groups: HashMap::new(),
        group: None,
        ctx: Vec::new(),
        spectrum: None,
        array: None,
        have_spectrum_list: false,
        xml_len: xml.len(),
    };
    let mut root = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                if !root {
                    if !matches!(e.local_name().as_ref(), b"mzML" | b"indexedmzML") {
                        return Err(Error::NotImzml);
                    }
                    root = true;
                }
                p.open(&e, false)?;
            }
            Ok(Event::Empty(e)) => {
                if !root {
                    return Err(Error::NotImzml);
                }
                p.open(&e, true)?;
            }
            Ok(Event::End(e)) => {
                if let Some(c) = p.ctx.pop() {
                    p.close(c)?;
                }
                if e.local_name().as_ref() == b"run" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Malformed(format!(
                    "XML error at byte {}: {e}",
                    r.buffer_position()
                )));
            }
        }
    }
    if !root {
        return Err(Error::NotImzml);
    }
    if !p.have_spectrum_list || p.out.entries.is_empty() {
        return Err(Error::NoSpectra);
    }
    Ok(p.out)
}
