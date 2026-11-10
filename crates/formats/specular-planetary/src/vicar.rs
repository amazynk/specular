//! VICAR (JPL's image format): a text label of `KEY=value` items (`LBLSIZE` bytes, with
//! property and history-task sections), optional binary header records, then the image
//! records, each optionally led by binary prefix bytes; an end-of-file label (`EOL=1`) may
//! follow the image.
//!
//! Samples are `BYTE`, `HALF`, `FULL`, `REAL`, `DOUB` and `COMP`, in `BSQ`, `BIL` or `BIP`
//! order, with the integer and real formats the label states (`INTFMT` HIGH/LOW, `REALFMT`
//! IEEE/RIEEE/VAX) or its `HOST` implies.

use crate::odl::{Value, number};
use crate::raster::{Interleave, Layout};
use crate::sample::{Real, Sample};
use crate::{Error, Facts, Result, map_facts};

/// Largest label read, in bytes.
const MAX_LABEL: usize = 16 << 20;
/// Label items kept.
const MAX_ITEMS: usize = 1 << 16;

/// One label item with its section: `None` for the system items at the start, else
/// "PROPERTY name" or "TASK name".
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub section: Option<String>,
    pub key: String,
    pub value: Value,
}

/// A VICAR label (the leading one with the end-of-file label's items after it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Label {
    pub items: Vec<Item>,
    /// Bytes of the leading label.
    pub size: u64,
}

impl Label {
    /// A system item (before any property or task).
    pub fn system(&self, key: &str) -> Option<&Value> {
        self.items
            .iter()
            .find(|i| i.section.is_none() && i.key.eq_ignore_ascii_case(key))
            .map(|i| &i.value)
    }

    fn sys_int(&self, key: &str) -> Option<u64> {
        self.system(key)
            .and_then(Value::as_f64)
            .filter(|v| *v >= 0.0 && v.fract() == 0.0)
            .map(|v| v as u64)
    }

    fn sys_str(&self, key: &str) -> Option<String> {
        self.system(key)
            .and_then(Value::as_str)
            .map(|s| s.trim().to_ascii_uppercase())
    }

    /// The first item named `key` in a property section (`MAP`, `IDENTIFICATION`…), or among
    /// the system items.
    pub fn property(&self, key: &str) -> Option<&Value> {
        self.items
            .iter()
            .find(|i| {
                i.key.eq_ignore_ascii_case(key)
                    && i.section
                        .as_deref()
                        .is_none_or(|s| s.starts_with("PROPERTY"))
            })
            .map(|i| &i.value)
    }

    /// Whether a property section named `name` exists.
    pub fn has_property(&self, name: &str) -> bool {
        let want = format!("PROPERTY {name}");
        self.items.iter().any(|i| {
            i.section
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(&want))
        })
    }

    /// The items as (`SECTION.KEY`, value text).
    pub fn keywords(&self, max: usize) -> Vec<(String, String)> {
        self.items
            .iter()
            .take(max)
            .map(|i| {
                let k = match &i.section {
                    Some(s) => format!("{s}.{}", i.key),
                    None => i.key.clone(),
                };
                (k, i.value.display())
            })
            .collect()
    }
}

/// Whether `head` starts like a VICAR file.
pub fn sniff(head: &[u8]) -> bool {
    let Some(rest) = head.strip_prefix(b"LBLSIZE") else {
        return false;
    };
    let rest = rest.trim_ascii_start();
    let Some(rest) = rest.strip_prefix(b"=") else {
        return false;
    };
    rest.trim_ascii_start()
        .first()
        .is_some_and(u8::is_ascii_digit)
}

/// Parses the label at the start of `bytes` (`bytes` begins with `LBLSIZE=`).
pub fn parse_label(bytes: &[u8]) -> Result<Label> {
    let size = label_size(bytes).ok_or(Error::NotThisFormat)?;
    let end = size.min(bytes.len()).min(MAX_LABEL);
    let mut items = Vec::new();
    parse_items(&bytes[..end], &mut items);
    Ok(Label {
        items,
        size: size as u64,
    })
}

/// `LBLSIZE` of the label at the start of `bytes`.
fn label_size(bytes: &[u8]) -> Option<usize> {
    if !sniff(bytes) {
        return None;
    }
    let s = &bytes[..bytes.len().min(64)];
    let eq = s.iter().position(|&c| c == b'=')?;
    let digits: Vec<u8> = s[eq + 1..]
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take_while(u8::is_ascii_digit)
        .collect();
    std::str::from_utf8(&digits)
        .ok()?
        .parse()
        .ok()
        .filter(|&n| n > 0)
}

/// Appends the items of one label's text.
fn parse_items(b: &[u8], items: &mut Vec<Item>) {
    let mut section: Option<String> = None;
    let mut i = 0;
    while i < b.len() && items.len() < MAX_ITEMS {
        while i < b.len() && (b[i] == b' ' || b[i].is_ascii_whitespace()) {
            i += 1;
        }
        // The label ends at the first null (the rest of LBLSIZE is padding).
        if i >= b.len() || b[i] == 0 {
            break;
        }
        let start = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() && b[i] != 0 {
            i += 1;
        }
        let key = String::from_utf8_lossy(&b[start..i]).into_owned();
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            // Not an item: skip the word.
            continue;
        }
        i += 1;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        let value = value(b, &mut i, 0);
        let upper = key.to_ascii_uppercase();
        if upper == "PROPERTY" || upper == "TASK" {
            let name = value.as_str().unwrap_or("").trim().to_owned();
            section = Some(format!("{upper} {name}"));
        }
        items.push(Item {
            section: section.clone(),
            key,
            value,
        });
    }
}

fn value(b: &[u8], i: &mut usize, depth: usize) -> Value {
    match b.get(*i) {
        Some(b'\'') => {
            *i += 1;
            let mut s = Vec::new();
            while *i < b.len() {
                if b[*i] == b'\'' {
                    if b.get(*i + 1) == Some(&b'\'') {
                        s.push(b'\'');
                        *i += 2;
                        continue;
                    }
                    *i += 1;
                    break;
                }
                if b[*i] == 0 {
                    break;
                }
                s.push(b[*i]);
                *i += 1;
            }
            Value::Text(String::from_utf8_lossy(&s).into_owned())
        }
        Some(b'(') if depth < 4 => {
            *i += 1;
            let mut items = Vec::new();
            loop {
                while *i < b.len() && b[*i] == b' ' {
                    *i += 1;
                }
                match b.get(*i) {
                    None | Some(0) => break,
                    Some(b')') => {
                        *i += 1;
                        break;
                    }
                    Some(b',') => *i += 1,
                    Some(_) => {
                        let before = *i;
                        items.push(value(b, i, depth + 1));
                        if *i == before {
                            *i += 1;
                        }
                    }
                }
            }
            Value::Seq(items)
        }
        _ => {
            let start = *i;
            while *i < b.len()
                && !matches!(b[*i], b' ' | b',' | b')' | 0)
                && !b[*i].is_ascii_whitespace()
            {
                *i += 1;
            }
            let w = String::from_utf8_lossy(&b[start..*i]).into_owned();
            match number(&w) {
                Some(value) => Value::Number { value, unit: None },
                None => Value::Symbol(w),
            }
        }
    }
}

/// The sample type of a VICAR `FORMAT`.
fn sample(format: &str, intfmt: &str, realfmt: &str) -> Result<Sample> {
    let big = intfmt != "LOW";
    let real = match realfmt {
        "VAX" => Real::Vax,
        "RIEEE" => Real::IeeeLittle,
        _ => Real::IeeeBig,
    };
    Ok(match format {
        "BYTE" => Sample::U8,
        "HALF" | "WORD" => Sample::Int {
            bytes: 2,
            signed: true,
            big_endian: big,
        },
        "FULL" | "LONG" => Sample::Int {
            bytes: 4,
            signed: true,
            big_endian: big,
        },
        "REAL" => Sample::Float { bytes: 4, real },
        "DOUB" => Sample::Float { bytes: 8, real },
        "COMP" | "COMPLEX" => Sample::Complex { bytes: 4, real },
        f => return Err(Error::Unsupported(format!("VICAR sample format {f}"))),
    })
}

/// The observation facts of a label's properties (`IDENTIFICATION`, `MAP`, instrument
/// properties), or of its system items.
pub fn facts(l: &Label) -> Facts {
    let text = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| l.property(k).and_then(crate::pds3::display_text))
    };
    Facts {
        mission: text(&["MISSION_NAME"]),
        spacecraft: text(&[
            "INSTRUMENT_HOST_NAME",
            "SPACECRAFT_NAME",
            "INSTRUMENT_HOST_ID",
        ]),
        instrument: text(&["INSTRUMENT_NAME", "INSTRUMENT_ID"]),
        target: text(&["TARGET_NAME"]),
        start: text(&["START_TIME", "IMAGE_TIME", "IMAGE_MID_TIME"]),
        stop: text(&["STOP_TIME"]),
        product_id: text(&["PRODUCT_ID"]),
        title: None,
        map: map_facts(|k| l.property(k).cloned()),
    }
}

/// A VICAR image: its label (with the EOL items) and where its samples are.
#[derive(Clone, Debug)]
pub struct Image {
    pub label: Label,
    pub layout: Layout,
    /// `FORMAT` as written.
    pub format: String,
    /// The binary header records and per-line prefix bytes, skipped.
    pub header_records: u64,
    pub prefix_bytes: u64,
    /// The decompressed samples of a BASIC-compressed image (`layout` points into them).
    pub decoded: Option<Vec<u8>>,
}

/// Opens a VICAR image from a file's bytes, or from bytes of a larger file with the VICAR
/// label at `start` (a PDS3 file's `IMAGE_HEADER`).
pub fn open(bytes: &[u8], start: usize) -> Result<Image> {
    let b = bytes.get(start..).ok_or(Error::NotThisFormat)?;
    let mut label = parse_label(b)?;
    let int = |k: &str| label.sys_int(k);
    let kind = label.sys_str("TYPE").unwrap_or_else(|| "IMAGE".into());
    if kind != "IMAGE" {
        return Err(Error::Unsupported(format!(
            "this VICAR file holds {} data, not an image",
            kind.to_ascii_lowercase()
        )));
    }
    let compress = label
        .sys_str("COMPRESS")
        .filter(|c| c != "NONE" && !c.is_empty());
    if let Some(c) = compress
        .as_deref()
        .filter(|c| *c != "BASIC" && *c != "BASIC2")
    {
        return Err(Error::Unsupported(format!(
            "VICAR {c} compression is not supported"
        )));
    }
    let format = label.sys_str("FORMAT").unwrap_or_else(|| "BYTE".into());
    // VICAR's defaults for labels from before these items existed (VAX hosts).
    let intfmt = label.sys_str("INTFMT").unwrap_or_else(|| "LOW".into());
    let realfmt = label.sys_str("REALFMT").unwrap_or_else(|| "VAX".into());
    let sample = sample(&format, &intfmt, &realfmt)?;
    let org = label.sys_str("ORG").unwrap_or_else(|| "BSQ".into());
    let interleave = match org.as_str() {
        "BSQ" => Interleave::Bsq,
        "BIL" => Interleave::Bil,
        "BIP" => Interleave::Bip,
        o => return Err(Error::Unsupported(format!("VICAR organisation {o}"))),
    };
    let (nl, ns, nb) = (
        int("NL").unwrap_or(0),
        int("NS").unwrap_or(0),
        int("NB").unwrap_or(1).max(1),
    );
    let nbb = int("NBB").unwrap_or(0);
    let nlb = int("NLB").unwrap_or(0);
    if nl == 0 || ns == 0 {
        return Err(Error::Malformed(
            "the VICAR image has no lines or samples".into(),
        ));
    }
    // One record per line of each band for BSQ, of all bands for BIL and BIP (where GDAL
    // reads one NBB prefix per line), each led by NBB prefix bytes.
    let per_record = match interleave {
        Interleave::Bsq => ns,
        _ => ns.saturating_mul(nb),
    };
    let record = per_record
        .saturating_mul(sample.bytes() as u64)
        .saturating_add(nbb);
    let recsize = int("RECSIZE").filter(|&r| r > 0).unwrap_or(record);
    let (w, h, n) = (
        u32::try_from(ns).map_err(|_| Error::TooLarge)?,
        u32::try_from(nl).map_err(|_| Error::TooLarge)?,
        u32::try_from(nb).map_err(|_| Error::TooLarge)?,
    );
    let data = (start as u64)
        .saturating_add(label.size)
        .saturating_add(nlb.saturating_mul(recsize));
    if let Some(c) = &compress {
        if interleave != Interleave::Bsq || nbb != 0 {
            return Err(Error::Unsupported(format!(
                "VICAR {c} compression is supported for BSQ images without prefixes only"
            )));
        }
        let records = nl.saturating_mul(nb);
        let decoded = basic(
            bytes,
            data,
            records,
            ns,
            sample.bytes() as u64,
            c == "BASIC2",
        )?;
        // Decoded samples are little-endian.
        let sample = match sample {
            Sample::Int { bytes, signed, .. } => Sample::Int {
                bytes,
                signed,
                big_endian: false,
            },
            Sample::Float { bytes, .. } => Sample::Float {
                bytes,
                real: Real::IeeeLittle,
            },
            Sample::Complex { bytes, .. } => Sample::Complex {
                bytes,
                real: Real::IeeeLittle,
            },
        };
        return Ok(Image {
            label,
            layout: Layout::new(w, h, n, sample, interleave, 0, 0, 0),
            format,
            header_records: nlb,
            prefix_bytes: 0,
            decoded: Some(decoded),
        });
    }
    let layout = Layout::new(w, h, n, sample, interleave, data, nbb, 0);
    // The end-of-file label after the image records.
    if int("EOL") == Some(1) {
        let records = match interleave {
            Interleave::Bsq => nl.saturating_mul(nb),
            _ => nl,
        };
        let eol = data.saturating_add(records.saturating_mul(record));
        if let Some(e) = usize::try_from(eol).ok().and_then(|e| bytes.get(e..))
            && let Some(n) = label_size(e)
        {
            let mut more = Vec::new();
            parse_items(&e[..n.min(e.len()).min(MAX_LABEL)], &mut more);
            // The EOL label restates LBLSIZE; keep the leading label's system items first.
            label.items.extend(
                more.into_iter()
                    .filter(|i| i.section.is_some() || !i.key.eq_ignore_ascii_case("LBLSIZE")),
            );
        }
    }
    Ok(Image {
        label,
        layout,
        format,
        header_records: nlb,
        prefix_bytes: nbb,
        decoded: None,
    })
}

/// Largest image decompressed from BASIC records, in bytes.
const MAX_DECODED: u64 = 1 << 32;

/// Decompresses `records` BASIC records (each led by its size, a little-endian u32 counting
/// itself) or BASIC2 records (the sizes, one u32 per record, before the first record) of `ns`
/// samples of `width` bytes, starting at `start`.
fn basic(
    bytes: &[u8],
    start: u64,
    records: u64,
    ns: u64,
    width: u64,
    two: bool,
) -> Result<Vec<u8>> {
    let rec = ns.saturating_mul(width);
    let total = rec.saturating_mul(records);
    if total > MAX_DECODED {
        return Err(Error::TooLarge);
    }
    let u32_at = |at: u64| -> Option<u64> {
        let a = usize::try_from(at).ok()?;
        let b = bytes.get(a..a.checked_add(4)?)?;
        Some(u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
    };
    // Every record costs at least 4 bytes (its size), so a label claiming more records than
    // the file holds is refused before anything is allocated; the output grows per record.
    let left = (bytes.len() as u64).saturating_sub(start);
    if records.saturating_mul(4) > left {
        return Err(Error::Malformed("VICAR records cut short".into()));
    }
    let mut out = Vec::new();
    let mut at = if two {
        start.saturating_add(records.saturating_mul(4))
    } else {
        start
    };
    for r in 0..records as usize {
        let (from, size) = if two {
            let n = u32_at(start.saturating_add(4 * r as u64))
                .ok_or_else(|| Error::Malformed("VICAR record sizes cut short".into()))?;
            (at, n)
        } else {
            let n = u32_at(at)
                .filter(|&n| n >= 4)
                .ok_or_else(|| Error::Malformed("VICAR record size cut short".into()))?;
            (at.saturating_add(4), n - 4)
        };
        let code = usize::try_from(from)
            .ok()
            .and_then(|f| bytes.get(f..f.checked_add(size as usize)?))
            .ok_or_else(|| Error::Malformed("VICAR record cut short".into()))?;
        let end = out.len();
        out.resize(end + rec as usize, 0);
        basic_record(code, ns as usize, width as usize, &mut out[end..])?;
        at = from.saturating_add(size);
    }
    Ok(out)
}

/// MSB-first bit reader over a compressed record.
struct Bits<'a> {
    b: &'a [u8],
    bit: usize,
}

impl Bits<'_> {
    fn get(&mut self, n: usize) -> Result<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self
                .b
                .get(self.bit / 8)
                .ok_or_else(|| Error::Malformed("VICAR compressed record ends early".into()))?;
            v = (v << 1) | u32::from(byte >> (7 - self.bit % 8) & 1);
            self.bit += 1;
        }
        Ok(v)
    }
}

/// One BASIC record: each byte plane of the samples in turn (low byte first), coded as a
/// difference of -3..=3 from the previous byte (3 bits), or after the escape code 7 either a
/// literal byte or a run of one value (a 4-bit count, extended by 8 or 24 bits).
fn basic_record(code: &[u8], ns: usize, width: usize, out: &mut [u8]) -> Result<()> {
    let mut bits = Bits { b: code, bit: 0 };
    let mut old: u32 = 0;
    // Copies of `value` still to write.
    let mut run = 0u32;
    let mut value: u32 = 0;
    let delta = |old: u32, c: u32| (i64::from(old) + i64::from(c) - 3) as u32;
    for plane in 0..width {
        for i in 0..ns {
            let o = i * width + plane;
            if run > 0 {
                out[o] = value as u8;
                run -= 1;
                continue;
            }
            let c = bits.get(3)?;
            if c < 7 {
                value = delta(old, c);
                old = value;
                out[o] = value as u8;
                continue;
            }
            if bits.get(1)? == 1 {
                let mut n = bits.get(4)?;
                if n == 15 {
                    n = bits.get(8)?;
                    n = if n == 255 {
                        let a = bits.get(8)?;
                        let b = bits.get(8)?;
                        let c = bits.get(8)?;
                        a | b << 8 | c << 16
                    } else {
                        n + 15
                    };
                }
                let c = bits.get(3)?;
                value = if c < 7 { delta(old, c) } else { bits.get(8)? };
                old = value;
                out[o] = value as u8;
                // This value and n + 3 more.
                run = n + 3;
            } else {
                let v = bits.get(8)?;
                old = v;
                out[o] = v as u8;
            }
        }
    }
    Ok(())
}

/// A VICAR file for tests and fixtures: `values` per band (BSQ order: band, line, sample)
/// stored as `format` in `org`, with `nbb` prefix bytes (0x55) per record and `nlb` binary
/// header records, and `extra` label text.
#[allow(clippy::too_many_arguments)]
pub fn write(
    format: &str,
    org: &str,
    intfmt: &str,
    realfmt: &str,
    (ns, nl, nb): (usize, usize, usize),
    nbb: usize,
    nlb: usize,
    extra: &str,
    values: &[f64],
) -> Vec<u8> {
    let s = sample(format, intfmt, realfmt).unwrap_or(Sample::U8);
    let bytes = s.bytes();
    let (n1, n2, n3) = match org {
        "BIL" => (ns, nb, nl),
        "BIP" => (nb, ns, nl),
        _ => (ns, nl, nb),
    };
    let whole_line = org == "BIP" || org == "BIL";
    let recsize = nbb + if whole_line { nb * ns } else { ns } * bytes;
    let mut text = format!(
        "LBLSIZE=          FORMAT='{format}'  TYPE='IMAGE'  BUFSIZ=20480  DIM=3  EOL=0  \
         RECSIZE={recsize}  ORG='{org}'  NL={nl}  NS={ns}  NB={nb}  N1={n1}  N2={n2}  N3={n3}  \
         N4=0  NBB={nbb}  NLB={nlb}  HOST='TEST'  INTFMT='{intfmt}'  REALFMT='{realfmt}'  {extra}"
    );
    let size = (text.len() + 16).div_ceil(recsize.max(1)) * recsize.max(1);
    let n = format!("{size}");
    text.replace_range(8..8 + n.len(), &n);
    let mut out = text.into_bytes();
    out.resize(size, 0);
    out.extend(std::iter::repeat_n(0xaau8, nlb * recsize));
    let encode = |v: f64| -> Vec<u8> { encode(s, v) };
    let at = |b: usize, y: usize, x: usize| values[(b * nl + y) * ns + x];
    for r3 in 0..n3 {
        if whole_line {
            out.extend(std::iter::repeat_n(0x55u8, nbb));
        }
        for r2 in 0..n2 {
            if !whole_line {
                out.extend(std::iter::repeat_n(0x55u8, nbb));
            }
            for r1 in 0..n1 {
                let (b, y, x) = match org {
                    "BIL" => (r2, r3, r1),
                    "BIP" => (r1, r3, r2),
                    _ => (r3, r2, r1),
                };
                out.extend(encode(at(b, y, x)));
            }
        }
    }
    out
}

/// `v` stored as `s` (for fixtures).
pub fn encode(s: Sample, v: f64) -> Vec<u8> {
    use crate::sample::{to_vax_d, to_vax_f};
    match s {
        Sample::Int {
            bytes,
            signed: _,
            big_endian,
        } => {
            let x = v as i64 as u64;
            let all = if big_endian {
                x.to_be_bytes().to_vec()
            } else {
                x.to_le_bytes().to_vec()
            };
            if big_endian {
                all[8 - bytes as usize..].to_vec()
            } else {
                all[..bytes as usize].to_vec()
            }
        }
        Sample::Float { bytes: 4, real } => match real {
            Real::IeeeBig => (v as f32).to_be_bytes().to_vec(),
            Real::IeeeLittle => (v as f32).to_le_bytes().to_vec(),
            _ => to_vax_f(v as f32).to_vec(),
        },
        Sample::Float { real, .. } => match real {
            Real::IeeeBig => v.to_be_bytes().to_vec(),
            Real::IeeeLittle => v.to_le_bytes().to_vec(),
            Real::Vax => to_vax_d(v).to_vec(),
            Real::VaxG => crate::sample::to_vax_g(v).to_vec(),
        },
        Sample::Complex { bytes, real } => {
            let mut a = encode(Sample::Float { bytes, real }, v);
            a.extend(encode(Sample::Float { bytes, real }, 0.0));
            a
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BASIC label claiming 2^30 records of 4096 samples (4 GiB) over 16 bytes: refused
    /// before the output is allocated.
    #[test]
    fn basic_record_count_bomb() {
        let r = basic(&[0u8; 16], 0, 1 << 20, 4096, 1, false);
        assert!(matches!(r, Err(Error::Malformed(_))));
        let r = basic(&[0u8; 16], 0, 1 << 20, 4096, 1, true);
        assert!(matches!(r, Err(Error::Malformed(_))));
    }

    #[test]
    fn label_items() {
        let t = b"LBLSIZE=200  FORMAT='HALF'  NL=2  NS=3  LIST=(1,2.5,'A''B')  \
            PROPERTY='MAP'  MAP_PROJECTION_TYPE='SIMPLE_CYLINDRICAL'  TASK='GEN'  USER='me'\0\0\0";
        let mut b = t.to_vec();
        b.resize(200, b' ');
        let l = parse_label(&b).unwrap();
        assert_eq!(l.size, 200);
        assert_eq!(l.system("NL").and_then(Value::as_f64), Some(2.0));
        assert_eq!(l.system("LIST").unwrap().display(), "(1, 2.5, A'B)");
        assert_eq!(
            l.property("MAP_PROJECTION_TYPE").and_then(Value::as_str),
            Some("SIMPLE_CYLINDRICAL")
        );
        assert!(l.has_property("MAP"));
        assert!(l.property("USER").is_none());
        let k = l.keywords(100);
        assert!(k.contains(&("TASK GEN.USER".into(), "me".into())));
        assert!(!sniff(b"LBLSIZE=  x"));
        assert!(sniff(b"LBLSIZE=4144 "));
    }

    #[test]
    fn layouts_and_formats() {
        let values: Vec<f64> = (0..2 * 3 * 4).map(|i| f64::from(i) - 5.0).collect();
        for org in ["BSQ", "BIL", "BIP"] {
            for (format, intfmt, realfmt) in [
                ("BYTE", "LOW", "IEEE"),
                ("HALF", "HIGH", "IEEE"),
                ("HALF", "LOW", "IEEE"),
                ("FULL", "LOW", "VAX"),
                ("REAL", "LOW", "VAX"),
                ("REAL", "LOW", "RIEEE"),
                ("DOUB", "HIGH", "IEEE"),
                ("DOUB", "LOW", "VAX"),
                ("COMP", "HIGH", "IEEE"),
            ] {
                let f = write(
                    format,
                    org,
                    intfmt,
                    realfmt,
                    (4, 3, 2),
                    3,
                    2,
                    "PROPERTY='X'  A=1",
                    &values,
                );
                let img = open(&f, 0).unwrap();
                for band in 0..2 {
                    let (v, t) = img.layout.band(&f, band);
                    assert!(!t);
                    for (i, got) in v.iter().enumerate() {
                        let mut want = values[band as usize * 12 + i];
                        if format == "BYTE" {
                            want = (want as i64 as u64 & 0xff) as f64;
                        }
                        if format == "COMP" {
                            want = want.abs();
                        }
                        assert_eq!(f64::from(*got), want, "{format} {org} {intfmt} {realfmt}");
                    }
                }
            }
        }
    }

    #[test]
    fn eol_labels_and_refusals() {
        let mut f = write(
            "BYTE",
            "BSQ",
            "LOW",
            "IEEE",
            (2, 2, 1),
            0,
            0,
            "",
            &[1.0, 2.0, 3.0, 4.0],
        );
        let pos = f.windows(5).position(|w| w == b"EOL=0").unwrap();
        f[pos + 4] = b'1';
        let mut eol = b"LBLSIZE=64  PROPERTY='MAP'  TARGET_NAME='MARS'".to_vec();
        eol.resize(64, b' ');
        f.extend(eol);
        let img = open(&f, 0).unwrap();
        assert_eq!(
            img.label.property("TARGET_NAME").and_then(Value::as_str),
            Some("MARS")
        );
        let c = String::from_utf8_lossy(&f).replace("HOST='TEST'", "COMPRESS='HUFF'");
        assert!(matches!(open(c.as_bytes(), 0), Err(Error::Unsupported(_))));
        let t = String::from_utf8_lossy(&f).replace("TYPE='IMAGE'", "TYPE='PARMS'");
        assert!(matches!(open(t.as_bytes(), 0), Err(Error::Unsupported(m)) if m.contains("parms")));
        for n in [0usize, 10, 40, 80, f.len() - 1] {
            let _ = open(&f[..n], 0);
        }
    }
}
