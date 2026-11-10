//! C-FIND SCU (PS3.4 annex C, PS3.7 section 9.1.2): Study Root Query/Retrieve at the study
//! and series levels.
//!
//! The identifier goes as Implicit VR Little Endian (every server takes it): the matching
//! keys with values, the return keys empty. Each pending response carries one match; once
//! `limit` matches arrived, a C-CANCEL-RQ asks the server to stop and the rest is read and
//! dropped until the final response.

use specular_core::CancelToken;
use specular_dicom::Charset;
use specular_dicom::dict::{self, Vr};

use crate::assoc::{Association, Options, Peer};
use crate::command::{Command, field};
use crate::pdu::ContextRq;
use crate::sop::{IMPLICIT_LE, STUDY_ROOT_FIND};
use crate::{Error, Result};

pub const QUERY_LEVEL: u32 = 0x0008_0052;
pub const SPECIFIC_CHARACTER_SET: u32 = 0x0008_0005;

/// The level of a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Study,
    Series,
}

impl Level {
    fn code(self) -> &'static str {
        match self {
            Level::Study => "STUDY",
            Level::Series => "SERIES",
        }
    }
}

/// A query: matching keys (with a value) and return keys (empty). Values use the DICOM
/// matching rules: `*`/`?` wildcards, `a-b` date ranges, `\`-separated UID lists.
#[derive(Clone, PartialEq, Eq)]
pub struct Query {
    pub level: Level,
    pub keys: Vec<(u32, String)>,
}

impl std::fmt::Debug for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Values can identify a patient.
        f.debug_struct("Query")
            .field("level", &self.level)
            .field("keys", &self.keys.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn vr_of(tag: u32) -> Vr {
    dict::lookup(tag).map_or(Vr::LO, |(v, _)| v)
}

fn put(out: &mut Vec<u8>, tag: u32, value: &[u8], pad: u8) {
    out.extend_from_slice(&((tag >> 16) as u16).to_le_bytes());
    out.extend_from_slice(&(tag as u16).to_le_bytes());
    let len = value.len() + (value.len() & 1);
    out.extend_from_slice(&(len as u32).to_le_bytes());
    out.extend_from_slice(value);
    if value.len() & 1 == 1 {
        out.push(pad);
    }
}

impl Query {
    /// The identifier, Implicit VR Little Endian, elements in tag order; text in UTF-8
    /// (Specific Character Set ISO_IR 192) when any value is not ASCII.
    pub fn identifier(&self) -> Vec<u8> {
        let mut keys: Vec<(u32, &str)> = self
            .keys
            .iter()
            .filter(|(t, _)| *t != QUERY_LEVEL && *t != SPECIFIC_CHARACTER_SET)
            .map(|(t, v)| (*t, v.as_str()))
            .collect();
        keys.push((QUERY_LEVEL, self.level.code()));
        if keys.iter().any(|(_, v)| !v.is_ascii()) {
            keys.push((SPECIFIC_CHARACTER_SET, "ISO_IR 192"));
        }
        keys.sort_by_key(|(t, _)| *t);
        keys.dedup_by_key(|(t, _)| *t);
        let mut out = Vec::with_capacity(keys.len() * 24);
        for (t, v) in keys {
            let pad = if vr_of(t) == Vr::UI { 0 } else { b' ' };
            put(&mut out, t, v.trim().as_bytes(), pad);
        }
        out
    }
}

/// One match: its elements as received.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Match {
    elements: Vec<(u32, Vec<u8>)>,
    charset: Charset,
}

impl std::fmt::Debug for Match {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Match")
            .field("elements", &self.elements.len())
            .finish_non_exhaustive()
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|v| u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
}

fn tag_at(b: &[u8], at: usize) -> Option<u32> {
    let g = b.get(at..at + 2)?;
    let e = b.get(at + 2..at + 4)?;
    Some(
        (u32::from(u16::from_le_bytes([g[0], g[1]])) << 16)
            | u32::from(u16::from_le_bytes([e[0], e[1]])),
    )
}

/// Where the undefined-length value starting at `at` ends (after its delimiter).
fn skip_undefined(b: &[u8], mut at: usize, depth: u32) -> Option<usize> {
    if depth > 16 {
        return None;
    }
    loop {
        let tag = tag_at(b, at)?;
        let len = u32_at(b, at + 4)?;
        at += 8;
        match tag {
            0xFFFE_E0DD | 0xFFFE_E00D => return Some(at),
            _ if len == u32::MAX => at = skip_undefined(b, at, depth + 1)?,
            _ => at = at.checked_add(len as usize)?,
        }
    }
}

impl Match {
    /// Reads an Implicit VR Little Endian identifier (top-level elements; sequences
    /// skipped).
    pub fn parse(b: &[u8]) -> Result<Match> {
        let mut m = Match::default();
        let mut at = 0;
        while at + 8 <= b.len() {
            let tag = tag_at(b, at).unwrap_or(0);
            let len = u32_at(b, at + 4).unwrap_or(0);
            at += 8;
            if len == u32::MAX {
                at = skip_undefined(b, at, 0)
                    .ok_or_else(|| Error::Protocol("identifier sequence without its end".into()))?;
                continue;
            }
            let v = b
                .get(at..at + len as usize)
                .ok_or_else(|| Error::Protocol("identifier element past its end".into()))?;
            at += len as usize;
            if vr_of(tag) != Vr::SQ {
                m.elements.push((tag, v.to_vec()));
            }
        }
        if at != b.len() {
            return Err(Error::Protocol("identifier ends mid-element".into()));
        }
        if let Some((_, v)) = m
            .elements
            .iter()
            .find(|(t, _)| *t == SPECIFIC_CHARACTER_SET)
        {
            m.charset = Charset::parse(v);
        }
        Ok(m)
    }

    /// The tags present, in order.
    pub fn tags(&self) -> impl Iterator<Item = u32> + '_ {
        self.elements.iter().map(|(t, _)| *t)
    }

    pub fn has(&self, tag: u32) -> bool {
        self.elements.iter().any(|(t, _)| *t == tag)
    }

    fn raw(&self, tag: u32) -> Option<&[u8]> {
        self.elements
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, v)| v.as_slice())
    }

    /// The value as text (all values, `\`-separated), trimmed; empty when absent.
    pub fn text(&self, tag: u32) -> String {
        self.raw(tag).map_or_else(String::new, |v| {
            self.charset
                .decode(v, vr_of(tag))
                .trim_matches(['\0', ' '])
                .to_owned()
        })
    }

    /// Every value of a multi-valued element.
    pub fn texts(&self, tag: u32) -> Vec<String> {
        self.text(tag)
            .split('\\')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// An integer (IS, or US/UL in binary).
    pub fn int(&self, tag: u32) -> Option<i64> {
        let v = self.raw(tag)?;
        match vr_of(tag) {
            Vr::US => v
                .get(..2)
                .map(|b| i64::from(u16::from_le_bytes([b[0], b[1]]))),
            Vr::UL => v
                .get(..4)
                .map(|b| i64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))),
            _ => self.text(tag).split('\\').next()?.trim().parse().ok(),
        }
    }
}

/// What a search found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub matches: Vec<Match>,
    /// The server had more than `limit` (it was asked to stop).
    pub more: bool,
}

/// The presentation context a C-FIND association offers.
pub fn contexts() -> Vec<ContextRq> {
    vec![ContextRq {
        id: 1,
        abstract_syntax: STUDY_ROOT_FIND.into(),
        transfer_syntaxes: vec![IMPLICIT_LE.into()],
    }]
}

/// Pending statuses (PS3.4 C.4.1.1.4): a match follows.
fn pending(s: u16) -> bool {
    matches!(s, 0xFF00 | 0xFF01)
}

impl Association {
    /// C-FIND of `q`; matches after the first `skip` are kept until `limit` (0: all).
    pub fn c_find(&mut self, q: &Query, skip: usize, limit: usize) -> Result<Found> {
        let ctx = self
            .find_context(STUDY_ROOT_FIND)
            .ok_or_else(|| Error::NoContext(STUDY_ROOT_FIND.into()))?;
        let r = self.find_inner(ctx, q, skip, limit);
        if let Err(e) = &r
            && e.is_fatal()
        {
            self.abort();
        }
        r
    }

    fn find_inner(&mut self, ctx: u8, q: &Query, skip: usize, limit: usize) -> Result<Found> {
        let id = self.next_message_id();
        let rq = Command::find_rq(id, STUDY_ROOT_FIND);
        self.send(ctx, &rq, Some(&q.identifier()))?;
        let mut found = Found::default();
        let mut seen = 0usize;
        let mut cancelled = false;
        loop {
            let m = self.receive(1 << 20)?;
            let c = m.command;
            if c.field != field::C_FIND_RSP || c.responded_to != Some(id) {
                return Err(Error::Protocol(format!(
                    "response {:#06x} to message {:?} during a C-FIND",
                    c.field, c.responded_to
                )));
            }
            let status = c
                .status
                .ok_or_else(|| Error::Protocol("C-FIND-RSP without a status".into()))?;
            if !pending(status) {
                return match status {
                    // Success, or Matching terminated due to Cancel.
                    0x0000 | 0xFE00 => Ok(found),
                    s => Err(Error::Status(s)),
                };
            }
            if cancelled {
                continue;
            }
            let Some(data) = m.data else {
                return Err(Error::Protocol("pending C-FIND-RSP without a match".into()));
            };
            seen += 1;
            if seen <= skip {
                continue;
            }
            if limit > 0 && found.matches.len() >= limit {
                found.more = true;
                cancelled = true;
                self.send(ctx, &Command::cancel_rq(id), None)?;
                continue;
            }
            found.matches.push(Match::parse(&data)?);
        }
    }
}

/// An association for C-FIND, one query, then release.
pub fn find(
    peer: &Peer,
    opts: &Options,
    q: &Query,
    skip: usize,
    limit: usize,
    cancel: &CancelToken,
) -> Result<Found> {
    let mut a = Association::request(peer, opts, &contexts(), cancel)?;
    let r = a.c_find(q, skip, limit)?;
    if let Err(e) = a.release() {
        specular_log::debug!(target: specular_log::IO, "dimse release after C-FIND: {e}");
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_round_trips() {
        let q = Query {
            level: Level::Study,
            keys: vec![
                (0x0010_0010, "Müller*".into()),
                (0x0020_000D, String::new()),
                (0x0008_0020, "20240101-".into()),
            ],
        };
        let b = q.identifier();
        assert_eq!(b.len() % 2, 0);
        let m = Match::parse(&b).unwrap();
        assert_eq!(m.text(QUERY_LEVEL), "STUDY");
        assert_eq!(m.text(SPECIFIC_CHARACTER_SET), "ISO_IR 192");
        assert_eq!(m.text(0x0010_0010), "Müller*");
        assert_eq!(m.text(0x0008_0020), "20240101-");
        assert!(m.has(0x0020_000D));
        assert_eq!(m.text(0x0020_000D), "");
        // Tag order.
        let tags: Vec<u32> = m.elements.iter().map(|(t, _)| *t).collect();
        let mut sorted = tags.clone();
        sorted.sort_unstable();
        assert_eq!(tags, sorted);
    }

    #[test]
    fn sequences_are_skipped() {
        let mut b = Vec::new();
        put(&mut b, 0x0008_0020, b"20240101", b' ');
        // (0008,1110) SQ of undefined length with one undefined-length item.
        b.extend_from_slice(&[0x08, 0, 0x10, 0x11, 0xFF, 0xFF, 0xFF, 0xFF]);
        b.extend_from_slice(&[0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
        put(&mut b, 0x0008_1150, b"1.2", 0);
        b.extend_from_slice(&[0xFE, 0xFF, 0x0D, 0xE0, 0, 0, 0, 0]);
        b.extend_from_slice(&[0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]);
        put(&mut b, 0x0020_1206, b"3", b' ');
        let m = Match::parse(&b).unwrap();
        assert_eq!(m.int(0x0020_1206), Some(3));
        assert_eq!(m.text(0x0008_0020), "20240101");
        assert!(!m.has(0x0008_1150));
        // Truncated: an error, not a panic.
        assert!(Match::parse(&b[..b.len() - 3]).is_err());
    }
}
