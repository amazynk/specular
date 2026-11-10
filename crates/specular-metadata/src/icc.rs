//! Minimal ICC profile reader: header fields plus the 'desc' and 'cprt' tags
//! (v2 `desc`/`text` and v4 `mluc` encodings).

use crate::i18n::Localizer;
use crate::{Field, strings};

#[derive(Debug, Clone, Default)]
pub struct IccInfo {
    pub size: u32,
    pub cmm: String,
    pub version: String,
    pub class: String,
    pub colour_space: String,
    pub pcs: String,
    pub created: Option<String>,
    pub platform: String,
    pub manufacturer: String,
    pub model: String,
    pub intent: u32,
    pub creator: String,
    pub description: Option<String>,
    pub copyright: Option<String>,
    pub tag_count: u32,
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}

fn sig(b: &[u8], at: usize) -> String {
    match b.get(at..at + 4) {
        Some(s) if s != [0, 0, 0, 0] => s
            .iter()
            .map(|&c| {
                if c.is_ascii_graphic() || c == b' ' {
                    c as char
                } else {
                    '?'
                }
            })
            .collect(),
        _ => String::new(),
    }
}

pub fn parse(b: &[u8]) -> Option<IccInfo> {
    if b.len() < 132 || &b[36..40] != b"acsp" {
        return None;
    }
    let mut info = IccInfo {
        size: be32(b, 0)?,
        cmm: sig(b, 4),
        version: format!("{}.{}", b[8], b[9] >> 4),
        class: sig(b, 12),
        colour_space: sig(b, 16).trim().to_owned(),
        pcs: sig(b, 20).trim().to_owned(),
        platform: sig(b, 40),
        manufacturer: sig(b, 48),
        model: sig(b, 52),
        intent: be32(b, 64)? & 0xffff,
        creator: sig(b, 80),
        ..Default::default()
    };
    let (y, mo, d, h, mi, s) = (
        be16(b, 24)?,
        be16(b, 26)?,
        be16(b, 28)?,
        be16(b, 30)?,
        be16(b, 32)?,
        be16(b, 34)?,
    );
    if y > 0 {
        info.created = Some(format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}"));
    }
    let count = be32(b, 128)?;
    info.tag_count = count;
    for i in 0..count.min(1024) as usize {
        let at = 132 + i * 12;
        let (Some(tag), Some(off), Some(len)) =
            (b.get(at..at + 4), be32(b, at + 4), be32(b, at + 8))
        else {
            break;
        };
        let Some(data) = b.get(off as usize..(off as usize).saturating_add(len as usize)) else {
            continue;
        };
        match tag {
            b"desc" => info.description = read_text(data),
            b"cprt" => info.copyright = read_text(data),
            _ => {}
        }
    }
    Some(info)
}

/// Reads a textual tag of type 'desc' (v2), 'text' or 'mluc' (v4).
fn read_text(data: &[u8]) -> Option<String> {
    let kind = data.get(0..4)?;
    let s = match kind {
        b"desc" => {
            let n = be32(data, 8)? as usize;
            let bytes = data.get(12..12 + n).or_else(|| data.get(12..))?;
            latin1(bytes)
        }
        b"text" => latin1(data.get(8..)?),
        b"mluc" => {
            let n = be32(data, 8)? as usize;
            let rec = be32(data, 12)? as usize;
            if rec < 12 {
                return None;
            }
            let mut best: Option<(u8, String)> = None;
            for i in 0..n.min(256) {
                let at = 16 + i * rec;
                let lang = data.get(at..at + 2)?;
                let country = data.get(at + 2..at + 4)?;
                let len = be32(data, at + 4)? as usize;
                let off = be32(data, at + 8)? as usize;
                let Some(raw) = data.get(off..off + len) else {
                    continue;
                };
                let units: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                let text = String::from_utf16_lossy(&units);
                let score = match (lang, country) {
                    (b"en", b"US") => 3,
                    (b"en", _) => 2,
                    _ if i == 0 => 1,
                    _ => 0,
                };
                if best.as_ref().is_none_or(|(s, _)| score > *s) {
                    best = Some((score, text));
                }
            }
            best?.1
        }
        _ => return None,
    };
    let s = s
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_owned();
    if s.is_empty() { None } else { Some(s) }
}

fn latin1(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|&c| c as char).collect()
}

impl IccInfo {
    pub fn fields(&self, l: &Localizer) -> Vec<Field> {
        let mut out = Vec::new();
        let mut push = |tag: &str, raw: String, display: String| {
            // Unset signatures are zero bytes (read as empty) or spaces.
            let empty = raw
                .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                .is_empty();
            out.push(Field {
                tag: tag.to_owned(),
                raw,
                display,
                empty,
                full: None,
            })
        };
        push(
            strings::ICC_PROFILE_SIZE,
            self.size.to_string(),
            strings::file_size_exact(l, self.size as u64),
        );
        push(strings::ICC_CMM, self.cmm.clone(), self.cmm.clone());
        push(
            strings::ICC_VERSION,
            self.version.clone(),
            self.version.clone(),
        );
        push(
            strings::ICC_CLASS,
            self.class.clone(),
            strings::icc_class(l, &self.class),
        );
        push(
            strings::ICC_COLOUR_SPACE,
            self.colour_space.clone(),
            self.colour_space.clone(),
        );
        push(strings::ICC_PCS, self.pcs.clone(), self.pcs.clone());
        if let Some(c) = &self.created {
            push(strings::ICC_DATE, c.clone(), c.clone());
        }
        push(
            strings::ICC_PLATFORM,
            self.platform.clone(),
            strings::icc_platform(l, &self.platform),
        );
        push(
            strings::ICC_MANUFACTURER,
            self.manufacturer.clone(),
            self.manufacturer.clone(),
        );
        push(strings::ICC_MODEL, self.model.clone(), self.model.clone());
        push(
            strings::ICC_INTENT,
            self.intent.to_string(),
            strings::icc_intent(l, self.intent),
        );
        push(
            strings::ICC_CREATOR,
            self.creator.clone(),
            self.creator.clone(),
        );
        if let Some(d) = &self.description {
            push(strings::ICC_DESCRIPTION, d.clone(), d.clone());
        }
        if let Some(c) = &self.copyright {
            push(strings::ICC_COPYRIGHT, c.clone(), c.clone());
        }
        push(
            strings::ICC_TAG_COUNT,
            self.tag_count.to_string(),
            self.tag_count.to_string(),
        );
        out
    }
}
