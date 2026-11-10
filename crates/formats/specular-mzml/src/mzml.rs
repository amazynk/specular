//! mzML: the header (parameter groups, software, instrument, run), where the spectra and
//! chromatograms start (the index of an indexed file when it is right, else a fast scan for
//! their start tags), and one spectrum or chromatogram read when asked for.

use std::collections::HashMap;

use memchr::memmem;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use specular_spectra::{Error, Facts, Technique, warn};

use crate::codec::{self, Compression, Number};
use crate::xml::{attr, num, start_tag, tag_attr, unescape};
use crate::{Entry, MAX_ENTRIES, Polarity, Record, XKind};

/// One `cvParam` or `userParam` (no accession).
#[derive(Clone, Debug, Default)]
pub(crate) struct Param {
    pub acc: String,
    pub name: String,
    pub value: String,
    pub unit_acc: String,
    pub unit_name: String,
}

fn param(e: &BytesStart<'_>) -> Param {
    let mut p = Param::default();
    for a in e.attributes().with_checks(false).flatten() {
        let v = || unescape(&a.value);
        match a.key.local_name().as_ref() {
            b"accession" => p.acc = v(),
            b"name" => p.name = v(),
            b"value" => p.value = v(),
            b"unitAccession" => p.unit_acc = v(),
            b"unitName" => p.unit_name = v(),
            _ => {}
        }
    }
    p
}

pub(crate) type Groups = HashMap<String, Vec<Param>>;

fn xml_err(at: u64, e: impl std::fmt::Display) -> Error {
    Error::Malformed(format!("XML error at byte {at}: {e}"))
}

/// What comes before the spectra: facts and parameter groups.
pub(crate) struct Header {
    pub facts: Facts,
    pub groups: Groups,
}

#[derive(PartialEq)]
enum HCtx {
    Group,
    FileContent,
    Software,
    Instrument,
    Component,
    Other,
}

/// Reads the header, up to the first spectrum or chromatogram list.
pub(crate) fn header(data: &[u8]) -> Result<Header, Error> {
    let mut r = Reader::from_reader(data);
    r.config_mut().check_end_names = false;
    let mut facts = Facts {
        format: "mzML".into(),
        technique: Technique::Mass,
        ..Facts::default()
    };
    let mut groups: Groups = HashMap::new();
    let mut group: Option<String> = None;
    let mut stack: Vec<HCtx> = Vec::new();
    let mut software: Vec<(String, String, String)> = Vec::new();
    let mut instruments = 0usize;
    let mut model: Vec<String> = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut content: Vec<String> = Vec::new();
    let mut root = false;
    loop {
        let ev = r
            .read_event()
            .map_err(|e| xml_err(r.buffer_position(), e))?;
        let (e, empty) = match ev {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(_) => {
                if let Some(c) = stack.pop()
                    && c == HCtx::Group
                {
                    group = None;
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = e.local_name();
        let name = name.as_ref();
        if !root {
            if !matches!(name, b"mzML" | b"indexedmzML") {
                return Err(Error::NotThisFormat);
            }
            root = true;
        }
        let top = |stack: &[HCtx], c: HCtx| stack.contains(&c);
        let mut ctx = HCtx::Other;
        match name {
            b"mzML" => {
                if let Some(v) = attr(&e, b"version") {
                    facts.version = v;
                }
            }
            b"spectrumList" | b"chromatogramList" | b"spectrum" | b"chromatogram" => break,
            b"sourceFile" => {
                if let Some(n) = attr(&e, b"name") {
                    sources.push(n);
                }
            }
            b"fileContent" => ctx = HCtx::FileContent,
            b"referenceableParamGroup" => {
                group = attr(&e, b"id");
                ctx = HCtx::Group;
            }
            b"software" => {
                software.push((
                    attr(&e, b"id").unwrap_or_default(),
                    attr(&e, b"version").unwrap_or_default(),
                    String::new(),
                ));
                ctx = HCtx::Software;
            }
            b"instrumentConfiguration" => {
                instruments += 1;
                ctx = HCtx::Instrument;
            }
            b"source" | b"analyzer" | b"detector" if top(&stack, HCtx::Instrument) => {
                ctx = HCtx::Component;
            }
            b"run" => {
                facts.title = attr(&e, b"id");
                facts.date = attr(&e, b"startTimeStamp");
            }
            b"cvParam" | b"userParam" | b"referenceableParamGroupRef" => {
                let ps = if name == b"referenceableParamGroupRef" {
                    let id = attr(&e, b"ref").unwrap_or_default();
                    groups.get(&id).cloned().unwrap_or_default()
                } else {
                    let mut p = param(&e);
                    if name == b"userParam" {
                        p.acc.clear();
                    }
                    vec![p]
                };
                for p in ps {
                    if let Some(g) = &group
                        && top(&stack, HCtx::Group)
                    {
                        groups.entry(g.clone()).or_default().push(p);
                    } else if top(&stack, HCtx::FileContent) {
                        content.push(p.name);
                    } else if top(&stack, HCtx::Software) {
                        if let Some(s) = software.last_mut()
                            && s.2.is_empty()
                        {
                            s.2 = if p.value.is_empty() { p.name } else { p.value };
                        }
                    } else if top(&stack, HCtx::Instrument) && instruments == 1 {
                        // Serial numbers and user parameters are not the instrument's name.
                        if p.acc.is_empty() || p.acc == "MS:1000529" {
                            continue;
                        }
                        let n = if p.value.is_empty() { p.name } else { p.value };
                        let list = if top(&stack, HCtx::Component) {
                            &mut parts
                        } else {
                            &mut model
                        };
                        if !n.is_empty() && !list.contains(&n) {
                            list.push(n);
                        }
                    }
                }
            }
            _ => {}
        }
        if !empty {
            stack.push(ctx);
        }
    }
    if !root {
        return Err(Error::NotThisFormat);
    }
    if !model.is_empty() || !parts.is_empty() {
        let mut s = model.join(", ");
        if !parts.is_empty() {
            if s.is_empty() {
                s = parts.join(", ");
            } else {
                s = format!("{s} ({})", parts.join(", "));
            }
        }
        facts.instrument = Some(s);
    }
    let mut fields = Vec::new();
    for (id, version, name) in software {
        let n = if name.is_empty() { id } else { name };
        let s = if version.is_empty() {
            n
        } else {
            format!("{n} {version}")
        };
        fields.push(("Software".to_string(), s));
    }
    for s in sources {
        fields.push(("Source file".to_string(), s));
    }
    if !content.is_empty() {
        fields.push(("File content".to_string(), content.join(", ")));
    }
    let mut seen = Vec::new();
    for f in fields {
        if !seen.contains(&f) {
            seen.push(f);
        }
    }
    facts.fields = seen;
    Ok(Header { facts, groups })
}

/// The number in `<indexListOffset>` (indexed mzML) or `<indexOffset>` (mzXML) near the end.
pub(crate) fn index_offset(data: &[u8], tag: &[u8]) -> Option<usize> {
    let tail_start = data.len().saturating_sub(4096);
    let tail = &data[tail_start..];
    let at = memmem::rfind(tail, tag)? + tag.len();
    let rest = &tail[at..];
    let end = memchr::memchr(b'<', rest)?;
    let s = std::str::from_utf8(&rest[..end]).ok()?;
    s.trim().parse::<usize>().ok().filter(|&o| o < data.len())
}

/// The `<offset>` entries of the index named `name` (`<index name="...">`) after `from`:
/// id attribute `id_key` and offset.
pub(crate) fn index_entries(
    data: &[u8],
    from: usize,
    name: &str,
    id_key: &[u8],
) -> Option<Vec<Entry>> {
    let region = data.get(from..)?;
    let open = format!("<index name=\"{name}\"");
    let s = memmem::find(region, open.as_bytes())?;
    let body = &region[s..];
    let e = memmem::find(body, b"</index>")?;
    let body = &body[..e];
    let mut out = Vec::new();
    let finder = memmem::Finder::new(b"<offset");
    for at in finder.find_iter(body) {
        let tag = start_tag(body, at)?;
        let id = tag_attr(tag, id_key).unwrap_or_default();
        let rest = &body[at + tag.len()..];
        let end = memchr::memchr(b'<', rest)?;
        let off: usize = std::str::from_utf8(&rest[..end])
            .ok()?
            .trim()
            .parse()
            .ok()?;
        out.push(Entry { offset: off, id });
        if out.len() > MAX_ENTRIES {
            break;
        }
    }
    Some(out)
}

/// Whether every entry's offset points at a start tag `<{tag}` and its id matches.
pub(crate) fn entries_valid(data: &[u8], entries: &[Entry], tag: &[u8], id_key: &[u8]) -> bool {
    entries.iter().all(|e| {
        let Some(rest) = data.get(e.offset..) else {
            return false;
        };
        if !(rest.starts_with(tag) && rest.get(tag.len()).is_some_and(|c| c.is_ascii_whitespace()))
        {
            return false;
        }
        start_tag(data, e.offset).is_some_and(|t| tag_attr(t, id_key).unwrap_or_default() == e.id)
    })
}

/// Start tags `<{tag} ` from `from` on, with their `id_key` attribute; at most
/// [`MAX_ENTRIES`] (the flag says whether more were left out).
pub(crate) fn scan_tags(data: &[u8], from: usize, tag: &[u8], id_key: &[u8]) -> (Vec<Entry>, bool) {
    let mut out = Vec::new();
    let Some(region) = data.get(from..) else {
        return (out, false);
    };
    let finder = memmem::Finder::new(tag);
    for at in finder.find_iter(region) {
        if !region
            .get(at + tag.len())
            .is_some_and(|c| c.is_ascii_whitespace())
        {
            continue;
        }
        if out.len() >= MAX_ENTRIES {
            return (out, true);
        }
        let off = from + at;
        let id = start_tag(data, off)
            .and_then(|t| tag_attr(t, id_key))
            .unwrap_or_default();
        out.push(Entry { offset: off, id });
    }
    (out, false)
}

/// Where the spectra and chromatograms start: from the index when it is right, else found by
/// scanning. Warnings say which.
pub(crate) fn entries(data: &[u8], warnings: &mut Vec<String>) -> (Vec<Entry>, Vec<Entry>, bool) {
    let indexed = memmem::find(&data[..data.len().min(16 << 10)], b"<indexedmzML").is_some();
    if indexed {
        if let Some(off) = index_offset(data, b"<indexListOffset>")
            && data[off..].starts_with(b"<indexList")
        {
            let sp = index_entries(data, off, "spectrum", b"idRef");
            let ch = index_entries(data, off, "chromatogram", b"idRef");
            if sp.is_some() || ch.is_some() {
                let (sp, ch) = (sp.unwrap_or_default(), ch.unwrap_or_default());
                if sp.len() <= MAX_ENTRIES
                    && entries_valid(data, &sp, b"<spectrum", b"id")
                    && entries_valid(data, &ch, b"<chromatogram", b"id")
                {
                    return (sp, ch, false);
                }
            }
        }
        warn(
            warnings,
            "The file's index does not match its contents; spectra were found by reading the file.",
        );
    }
    let start = memmem::find(data, b"<spectrumList")
        .or_else(|| memmem::find(data, b"<chromatogramList"))
        .unwrap_or(0);
    let (sp, capped) = scan_tags(data, start, b"<spectrum", b"id");
    let cstart =
        memmem::find(&data[start..], b"<chromatogramList").map_or(data.len(), |c| c + start);
    let (ch, _) = scan_tags(data, cstart, b"<chromatogram", b"id");
    (sp, ch, capped)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ctx {
    Spectrum,
    Scan,
    Precursor,
    SelectedIon,
    Isolation,
    Array,
    Binary,
    Other,
}

#[derive(Default)]
struct ArrayDesc {
    params: Vec<Param>,
    text: Vec<u8>,
}

enum ArrayRole {
    X(XKind, f64),
    Y,
    Skip,
}

/// What an array holds and how it is stored.
fn describe(params: &[Param]) -> Result<(ArrayRole, Option<Number>, Compression), Error> {
    let mut number = None;
    let mut comp = None;
    let mut role = ArrayRole::Skip;
    for p in params {
        match p.acc.as_str() {
            "MS:1000521" => number = Some(Number::F32),
            "MS:1000523" => number = Some(Number::F64),
            "MS:1000519" => number = Some(Number::I32),
            "MS:1000522" => number = Some(Number::I64),
            "MS:1000520" => number = Some(Number::F16),
            "MS:1000576" => comp = Some(Compression::None),
            "MS:1000574" => comp = Some(Compression::Zlib),
            "MS:1002312" => comp = Some(Compression::Linear),
            "MS:1002313" => comp = Some(Compression::Pic),
            "MS:1002314" => comp = Some(Compression::Slof),
            "MS:1002746" => comp = Some(Compression::LinearZlib),
            "MS:1002747" => comp = Some(Compression::PicZlib),
            "MS:1002748" => comp = Some(Compression::SlofZlib),
            "MS:1000514" => role = ArrayRole::X(XKind::Mz, 1.0),
            "MS:1000617" => role = ArrayRole::X(XKind::Wavelength, 1.0),
            "MS:1000515" => role = ArrayRole::Y,
            "MS:1000595" => {
                let to_min = match (p.unit_acc.as_str(), p.unit_name.as_str()) {
                    ("UO:0000010", _) | (_, "second") => 1.0 / 60.0,
                    ("UO:0000032", _) | (_, "hour") => 60.0,
                    ("UO:0000028", _) | (_, "millisecond") => 1.0 / 60_000.0,
                    _ => 1.0,
                };
                role = ArrayRole::X(XKind::Time, to_min);
            }
            _ => {
                if p.name.contains("compression") && comp.is_none() {
                    return Err(Error::Unsupported(format!(
                        "This file's binary arrays use {} ({}), which Specular does not read.",
                        p.name, p.acc
                    )));
                }
            }
        }
    }
    Ok((role, number, comp.unwrap_or(Compression::None)))
}

/// Reads the spectrum or chromatogram starting at `entry`; with `header_only` its arrays are
/// not decoded.
pub(crate) fn record(
    data: &[u8],
    entry: &Entry,
    index: usize,
    groups: &Groups,
    header_only: bool,
) -> Result<Record, Error> {
    let slice = data
        .get(entry.offset..)
        .ok_or_else(|| Error::Truncated("a spectrum lies past the end of the file".into()))?;
    let mut r = Reader::from_reader(slice);
    r.config_mut().check_end_names = false;
    let mut rec = Record {
        id: entry.id.clone(),
        index,
        x_to_minutes: 1.0,
        ..Record::default()
    };
    let mut stack: Vec<Ctx> = Vec::new();
    let mut scans = 0usize;
    let mut precursors = 0usize;
    let mut ions = 0usize;
    let mut array: Option<ArrayDesc> = None;
    let mut iso_target: Option<f64> = None;
    let mut started = false;
    loop {
        let ev = match r.read_event() {
            Ok(ev) => ev,
            Err(e) => {
                if r.buffer_position() as usize >= slice.len() {
                    return Err(Error::Truncated("a spectrum ends early".into()));
                }
                return Err(xml_err(entry.offset as u64 + r.buffer_position(), e));
            }
        };
        let (e, empty) = match ev {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::Text(t) => {
                if stack.last() == Some(&Ctx::Binary)
                    && let Some(a) = array.as_mut()
                {
                    a.text.extend_from_slice(&t);
                }
                continue;
            }
            Event::End(_) => {
                let c = stack.pop();
                if c == Some(Ctx::Array)
                    && let Some(a) = array.take()
                    && !header_only
                {
                    let (role, number, comp) = describe(&a.params)?;
                    if !matches!(role, ArrayRole::Skip) {
                        let numpress = !matches!(comp, Compression::None | Compression::Zlib);
                        let number = match number {
                            Some(n) => n,
                            None if numpress => Number::F64,
                            None => {
                                return Err(Error::Malformed(
                                    "a binary array does not say its number type".into(),
                                ));
                            }
                        };
                        let v = codec::decode(&a.text, number, comp, false)?;
                        match role {
                            ArrayRole::X(k, m) if rec.x_kind == XKind::None => {
                                rec.x_kind = k;
                                rec.x_to_minutes = m;
                                rec.x = v;
                            }
                            ArrayRole::Y if rec.y.is_empty() => rec.y = v,
                            _ => {}
                        }
                    }
                }
                if stack.is_empty() {
                    break;
                }
                continue;
            }
            Event::Eof => {
                if !started || !stack.is_empty() {
                    return Err(Error::Truncated("a spectrum ends early".into()));
                }
                break;
            }
            _ => continue,
        };
        let name = e.local_name();
        let name = name.as_ref();
        if !started {
            if !matches!(name, b"spectrum" | b"chromatogram") {
                return Err(Error::Malformed(
                    "an index entry does not point at a spectrum".into(),
                ));
            }
            started = true;
            rec.default_length =
                attr(&e, b"defaultArrayLength").and_then(|v| v.trim().parse().ok());
            // mzML 0.99 states the level as an attribute.
            rec.ms_level = attr(&e, b"msLevel").and_then(|v| v.trim().parse().ok());
            if !empty {
                stack.push(Ctx::Spectrum);
                continue;
            }
            break;
        }
        let parent = stack.last().copied().unwrap_or(Ctx::Other);
        let ctx = match name {
            b"scan" => {
                scans += 1;
                Ctx::Scan
            }
            b"precursor" => {
                precursors += 1;
                Ctx::Precursor
            }
            b"selectedIon" => {
                ions += 1;
                Ctx::SelectedIon
            }
            b"isolationWindow" if parent == Ctx::Precursor => Ctx::Isolation,
            b"binaryDataArrayList" if header_only => break,
            b"binaryDataArray" => {
                array = Some(ArrayDesc::default());
                Ctx::Array
            }
            b"binary" if parent == Ctx::Array => Ctx::Binary,
            b"cvParam" | b"userParam" | b"referenceableParamGroupRef" => {
                let ps = if name == b"referenceableParamGroupRef" {
                    let id = attr(&e, b"ref").unwrap_or_default();
                    groups.get(&id).cloned().unwrap_or_default()
                } else {
                    let mut p = param(&e);
                    if name == b"userParam" {
                        p.acc.clear();
                    }
                    vec![p]
                };
                for p in ps {
                    apply(
                        &mut rec,
                        parent,
                        &p,
                        (scans, precursors, ions),
                        &mut array,
                        &mut iso_target,
                    );
                }
                Ctx::Other
            }
            _ => Ctx::Other,
        };
        if !empty {
            stack.push(ctx);
        } else if ctx == Ctx::Array {
            // `<binaryDataArray/>`: nothing in it.
            array = None;
        }
    }
    if rec.precursor_mz.is_none() {
        rec.precursor_mz = iso_target;
    }
    Ok(rec)
}

fn apply(
    rec: &mut Record,
    ctx: Ctx,
    p: &Param,
    (scans, precursors, ions): (usize, usize, usize),
    array: &mut Option<ArrayDesc>,
    iso: &mut Option<f64>,
) {
    match ctx {
        Ctx::Spectrum => match p.acc.as_str() {
            "MS:1000511" => rec.ms_level = p.value.trim().parse().ok(),
            "MS:1000127" => rec.centroid = Some(true),
            "MS:1000128" => rec.centroid = Some(false),
            "MS:1000130" => rec.polarity = Some(Polarity::Positive),
            "MS:1000129" => rec.polarity = Some(Polarity::Negative),
            "MS:1000285" => rec.tic = num(&p.value),
            "MS:1000504" => rec.base_mz = num(&p.value),
            "MS:1000505" => rec.base_intensity = num(&p.value),
            "MS:1000235" => {
                rec.is_tic = true;
                rec.kind_name = Some(p.name.clone());
            }
            _ if p.name.ends_with("chromatogram") && rec.kind_name.is_none() => {
                rec.kind_name = Some(p.name.clone());
            }
            _ => {}
        },
        Ctx::Scan if scans == 1 => {
            if p.acc == "MS:1000016"
                && let Some(v) = num(&p.value)
            {
                let m = match (p.unit_acc.as_str(), p.unit_name.as_str()) {
                    ("UO:0000010", _) | ("MS:1000039", _) | (_, "second") => 1.0 / 60.0,
                    ("UO:0000032", _) | (_, "hour") => 60.0,
                    _ => 1.0,
                };
                rec.rt_min = Some(v * m);
            }
        }
        Ctx::SelectedIon if precursors == 1 && ions == 1 => match p.acc.as_str() {
            "MS:1000744" | "MS:1000040" => rec.precursor_mz = rec.precursor_mz.or(num(&p.value)),
            "MS:1000041" => rec.charge = p.value.trim().parse().ok(),
            _ => {}
        },
        Ctx::Isolation if precursors == 1 => {
            if p.acc == "MS:1000827" {
                *iso = num(&p.value);
            }
        }
        Ctx::Array => {
            if let Some(a) = array.as_mut() {
                a.params.push(p.clone());
            }
        }
        _ => {}
    }
}
