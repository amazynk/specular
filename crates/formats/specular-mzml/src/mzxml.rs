//! mzXML 2.x–3.2: the header (instrument, software, parent files), the scan index (or a
//! fast scan for `<scan` tags) and one scan read when asked for.

use memchr::memmem;
use quick_xml::Reader;
use quick_xml::events::Event;
use specular_spectra::{Error, Facts, Technique, warn};

use crate::codec::{self, Compression, Number};
use crate::mzml::{entries_valid, index_entries, index_offset, scan_tags};
use crate::xml::{attr, duration_minutes, num};
use crate::{Entry, Polarity, Record, XKind};

/// Reads the header, up to the first scan.
pub(crate) fn header(data: &[u8]) -> Result<Facts, Error> {
    let mut r = Reader::from_reader(data);
    r.config_mut().check_end_names = false;
    let mut facts = Facts {
        format: "mzXML".into(),
        technique: Technique::Mass,
        ..Facts::default()
    };
    let mut root = false;
    let mut instrument: Vec<String> = Vec::new();
    let mut in_instrument = false;
    let mut instruments = 0usize;
    loop {
        let ev = r.read_event().map_err(|e| {
            Error::Malformed(format!("XML error at byte {}: {e}", r.buffer_position()))
        })?;
        let e = match ev {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.local_name().as_ref() == b"msInstrument" {
                    in_instrument = false;
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = e.local_name();
        let name = name.as_ref();
        if !root {
            if name != b"mzXML" {
                return Err(Error::NotThisFormat);
            }
            root = true;
            if let Some(ns) = attr(&e, b"xmlns")
                && let Some(i) = ns.find("mzXML_")
            {
                facts.version = ns[i + 6..].trim_end_matches('/').to_string();
            }
            continue;
        }
        match name {
            b"scan" => break,
            b"msRun" => {
                if let Some(s) = attr(&e, b"startTime").and_then(|s| duration_minutes(&s)) {
                    facts
                        .fields
                        .push(("Run start".into(), format!("{s:.3} min")));
                }
                if let Some(s) = attr(&e, b"endTime").and_then(|s| duration_minutes(&s)) {
                    facts.fields.push(("Run end".into(), format!("{s:.3} min")));
                }
            }
            b"parentFile" => {
                if let Some(f) = attr(&e, b"fileName") {
                    if facts.title.is_none() {
                        facts.title = Some(f.rsplit(['/', '\\']).next().unwrap_or(&f).to_string());
                    }
                    facts.fields.push(("Source file".into(), f));
                }
            }
            b"msInstrument" | b"instrument" => {
                instruments += 1;
                in_instrument = instruments == 1;
                // mzXML 2.0 `<instrument manufacturer=… model=…>`.
                for k in [&b"manufacturer"[..], b"model", b"ionisation", b"msType"] {
                    if let Some(v) = attr(&e, k) {
                        instrument.push(v);
                    }
                }
            }
            b"msManufacturer" | b"msModel" | b"msIonisation" | b"msMassAnalyzer"
            | b"msDetector" | b"msResolution"
                if in_instrument =>
            {
                if let Some(v) = attr(&e, b"value").filter(|v| !v.is_empty()) {
                    instrument.push(v);
                }
            }
            b"software" => {
                let n = attr(&e, b"name").unwrap_or_default();
                let v = attr(&e, b"version").unwrap_or_default();
                let s = format!("{n} {v}").trim().to_string();
                if !s.is_empty() {
                    facts.fields.push(("Software".into(), s));
                }
            }
            _ => {}
        }
    }
    if !root {
        return Err(Error::NotThisFormat);
    }
    instrument.dedup();
    if !instrument.is_empty() {
        let first = instrument.remove(0);
        facts.instrument = Some(if instrument.is_empty() {
            first
        } else {
            format!("{first} ({})", instrument.join(", "))
        });
    }
    let mut fields = Vec::new();
    for f in std::mem::take(&mut facts.fields) {
        if !fields.contains(&f) {
            fields.push(f);
        }
    }
    facts.fields = fields;
    Ok(facts)
}

/// Where the scans start: from the index when it is right, else found by scanning.
pub(crate) fn entries(data: &[u8], warnings: &mut Vec<String>) -> (Vec<Entry>, bool) {
    if let Some(off) = index_offset(data, b"<indexOffset>") {
        if let Some(sp) = index_entries(data, off, "scan", b"id")
            && entries_valid(data, &sp, b"<scan", b"num")
            && !sp.is_empty()
        {
            return (sp, false);
        }
        warn(
            warnings,
            "The file's index does not match its contents; scans were found by reading the file.",
        );
    }
    let start = memmem::find(data, b"<msRun").unwrap_or(0);
    scan_tags(data, start, b"<scan", b"num")
}

/// Reads the scan starting at `entry` (not the scans nested in it); with `header_only` its
/// peaks are not decoded.
pub(crate) fn record(
    data: &[u8],
    entry: &Entry,
    index: usize,
    header_only: bool,
) -> Result<Record, Error> {
    let slice = data
        .get(entry.offset..)
        .ok_or_else(|| Error::Truncated("a scan lies past the end of the file".into()))?;
    let mut r = Reader::from_reader(slice);
    r.config_mut().check_end_names = false;
    let mut rec = Record {
        id: entry.id.clone(),
        index,
        x_to_minutes: 1.0,
        ..Record::default()
    };
    let mut depth = 0usize;
    let mut started = false;
    // 0: elsewhere, 1: in precursorMz, 2: in peaks.
    let mut text_of = 0u8;
    let mut text = Vec::new();
    let mut peaks: Option<(Number, Compression, bool, bool)> = None;
    loop {
        let ev = match r.read_event() {
            Ok(ev) => ev,
            Err(_) if r.buffer_position() as usize >= slice.len() => {
                return Err(Error::Truncated("a scan ends early".into()));
            }
            Err(e) => {
                return Err(Error::Malformed(format!(
                    "XML error at byte {}: {e}",
                    entry.offset as u64 + r.buffer_position()
                )));
            }
        };
        let (e, empty) = match ev {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::Text(t) => {
                if text_of != 0 {
                    text.extend_from_slice(&t);
                }
                continue;
            }
            Event::End(_) => {
                match text_of {
                    1 => {
                        if rec.precursor_mz.is_none() {
                            rec.precursor_mz = std::str::from_utf8(&text).ok().and_then(num);
                        }
                    }
                    2 => {
                        if let Some(p) = peaks.take() {
                            decode_peaks(&mut rec, &text, p)?;
                        }
                        // Everything of the scan comes before its peaks end.
                        break;
                    }
                    _ => {}
                }
                text_of = 0;
                text.clear();
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                continue;
            }
            Event::Eof => {
                if !started {
                    return Err(Error::Truncated("a scan ends early".into()));
                }
                if depth > 0 && !header_only {
                    return Err(Error::Truncated("a scan ends early".into()));
                }
                break;
            }
            _ => continue,
        };
        let name = e.local_name();
        let name = name.as_ref();
        if !started {
            if name != b"scan" {
                return Err(Error::Malformed(
                    "an index entry does not point at a scan".into(),
                ));
            }
            started = true;
            let a = |k: &[u8]| attr(&e, k);
            rec.ms_level = a(b"msLevel").and_then(|v| v.trim().parse().ok());
            rec.centroid = a(b"centroided").map(|v| v.trim() == "1");
            rec.polarity = match a(b"polarity").as_deref() {
                Some("+") => Some(Polarity::Positive),
                Some("-") => Some(Polarity::Negative),
                _ => None,
            };
            rec.rt_min = a(b"retentionTime").and_then(|v| duration_minutes(&v));
            rec.tic = a(b"totIonCurrent").and_then(|v| num(&v));
            rec.base_mz = a(b"basePeakMz").and_then(|v| num(&v));
            rec.base_intensity = a(b"basePeakIntensity").and_then(|v| num(&v));
            rec.default_length = a(b"peaksCount").and_then(|v| v.trim().parse().ok());
            if empty || header_only {
                break;
            }
            depth = 1;
            continue;
        }
        match name {
            // A scan nested in this one: this one's peaks came before it.
            b"scan" => break,
            b"precursorMz" => {
                if rec.charge.is_none() {
                    rec.charge = attr(&e, b"precursorCharge").and_then(|v| v.trim().parse().ok());
                }
                if !empty {
                    text_of = 1;
                }
            }
            b"peaks" => {
                let precision = attr(&e, b"precision").unwrap_or_else(|| "32".into());
                let number = match precision.trim() {
                    "64" => Number::F64,
                    "32" => Number::F32,
                    p => {
                        return Err(Error::Unsupported(format!(
                            "mzXML peaks with precision {p} are not supported."
                        )));
                    }
                };
                let comp = match attr(&e, b"compressionType").as_deref().map(str::trim) {
                    None | Some("none") | Some("") => Compression::None,
                    Some("zlib") => Compression::Zlib,
                    Some(c) => {
                        return Err(Error::Unsupported(format!(
                            "mzXML peaks compressed with {c} are not supported."
                        )));
                    }
                };
                let order = attr(&e, b"pairOrder")
                    .or_else(|| attr(&e, b"contentType"))
                    .unwrap_or_else(|| "m/z-int".into());
                let pairs = match order.trim() {
                    "m/z-int" => true,
                    o => {
                        return Err(Error::Unsupported(format!(
                            "mzXML peaks stored as \"{o}\" are not supported."
                        )));
                    }
                };
                let be = attr(&e, b"byteOrder").is_none_or(|b| b.trim() != "little");
                if empty {
                    if !header_only {
                        decode_peaks(&mut rec, b"", (number, comp, be, pairs))?;
                    }
                    break;
                }
                peaks = Some((number, comp, be, pairs));
                text_of = 2;
            }
            _ => {}
        }
        if !empty {
            depth += 1;
        }
    }
    Ok(rec)
}

fn decode_peaks(
    rec: &mut Record,
    text: &[u8],
    (number, comp, be, _pairs): (Number, Compression, bool, bool),
) -> Result<(), Error> {
    let v = codec::decode(text, number, comp, be)?;
    rec.x_kind = XKind::Mz;
    rec.x = v.iter().step_by(2).copied().collect();
    rec.y = v.iter().skip(1).step_by(2).copied().collect();
    Ok(())
}
