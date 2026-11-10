//! Sending files with C-STORE: what to propose, and how each file goes.
//!
//! For every SOP class among the files, [`contexts`] offers one presentation context with
//! Explicit and Implicit VR Little Endian (any file can be sent that way), and one per
//! transfer syntax the files are stored in. A file is sent
//! as stored, byte for byte from its data set, when the server accepted its syntax; else as
//! the little-endian syntax accepted for its class, converted here: big endian swapped,
//! deflated inflated, compressed pixel data decoded by the in-tree decoders (a lossy source
//! keeps or gains Lossy Image Compression "01").

use std::borrow::Cow;
use std::io::Read;
use std::path::Path;

use specular_dicom::dict::Vr;
use specular_dicom::write::{self, Object, Value};
use specular_dicom::{Dicom, TransferSyntax};

use crate::assoc::Association;
use crate::pdu::ContextRq;
use crate::sop::{EXPLICIT_LE, IMPLICIT_LE};
use crate::{Error, Result};

/// The most presentation contexts one association can have (odd ids 1–255).
pub const MAX_CONTEXTS: usize = 128;

/// What negotiation needs to know about a file, from its File Meta Information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub sop_class: String,
    pub sop_instance: String,
    pub transfer_syntax: String,
    /// Where the data set starts in the file.
    pub data_set: usize,
}

fn uid_text(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches(['\0', ' '])
        .trim()
        .to_owned()
}

/// Reads the File Meta Information (group 0002, Explicit VR Little Endian after the
/// preamble and "DICM"). `None` for a file without one or with an incomplete one.
pub fn head(file: &[u8]) -> Option<Head> {
    if file.get(128..132)? != b"DICM" {
        return None;
    }
    let mut at = 132;
    let (mut class, mut inst, mut ts) = (None, None, None);
    while let Some(h) = file.get(at..at + 8) {
        let group = u16::from_le_bytes([h[0], h[1]]);
        if group != 2 {
            break;
        }
        let elem = u16::from_le_bytes([h[2], h[3]]);
        let vr = [h[4], h[5]];
        let long = matches!(
            &vr,
            b"OB" | b"OW" | b"OF" | b"SQ" | b"UT" | b"UN" | b"UC" | b"UR" | b"OD" | b"OL" | b"OV"
        );
        let (len, start) = if long {
            let l = file.get(at + 8..at + 12)?;
            (
                u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize,
                at + 12,
            )
        } else {
            (u16::from_le_bytes([h[6], h[7]]) as usize, at + 8)
        };
        let v = file.get(start..start.checked_add(len)?)?;
        match elem {
            0x0002 => class = Some(uid_text(v)),
            0x0003 => inst = Some(uid_text(v)),
            0x0010 => ts = Some(uid_text(v)),
            _ => {}
        }
        at = start + len;
    }
    Some(Head {
        sop_class: class.filter(|s| !s.is_empty())?,
        sop_instance: inst.filter(|s| !s.is_empty())?,
        transfer_syntax: ts.filter(|s| !s.is_empty())?,
        data_set: at,
    })
}

/// [`head`] of a file on disk, reading only its start (the whole file when the meta
/// information is longer, or for a file without one: then from its data set).
pub fn read_head(path: &Path) -> std::io::Result<Option<Head>> {
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(16 * 1024);
    (&mut f).take(16 * 1024).read_to_end(&mut buf)?;
    if let Some(h) = head(&buf) {
        return Ok(Some(h));
    }
    f.read_to_end(&mut buf)?;
    Ok(head(&buf).or_else(|| head_of_parsed(&buf)))
}

/// A file without File Meta Information (a bare data set, Implicit VR Little Endian).
fn head_of_parsed(file: &[u8]) -> Option<Head> {
    let d = Dicom::parse(file).ok()?;
    let v = d.values(file);
    let t = |tag: u32| d.dataset.get(tag).map(|e| uid_text(v.bytes(e)));
    Some(Head {
        sop_class: t(0x0008_0016).filter(|s| !s.is_empty())?,
        sop_instance: t(0x0008_0018).filter(|s| !s.is_empty())?,
        transfer_syntax: d.transfer_syntax.uid.clone(),
        data_set: 0,
    })
}

fn uncompressed_le(ts: &str) -> bool {
    ts == EXPLICIT_LE || ts == IMPLICIT_LE
}

/// The presentation contexts to offer for files of these (SOP class, transfer syntax).
/// Each class gets its little-endian context first (the server picks one of the two); then
/// each (class, stored syntax) gets a context of its own, so that a file goes as stored
/// whenever the server takes its syntax, as far as the 128 contexts go.
pub fn contexts<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<ContextRq> {
    let mut classes: Vec<&str> = Vec::new();
    let mut stored: Vec<(&str, &str)> = Vec::new();
    for (c, ts) in files {
        if !classes.contains(&c) {
            classes.push(c);
        }
        if !stored.contains(&(c, ts)) {
            stored.push((c, ts));
        }
    }
    let mut out: Vec<ContextRq> = Vec::new();
    let mut next = 1u16;
    let mut push = |out: &mut Vec<ContextRq>, a: &str, ts: Vec<String>| {
        if out.len() < MAX_CONTEXTS {
            out.push(ContextRq {
                id: next as u8,
                abstract_syntax: a.to_owned(),
                transfer_syntaxes: ts,
            });
            next += 2;
        }
    };
    for c in &classes {
        push(&mut out, c, vec![EXPLICIT_LE.into(), IMPLICIT_LE.into()]);
    }
    for (c, ts) in stored {
        push(&mut out, c, vec![ts.to_owned()]);
    }
    out
}

/// One file sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
    pub sop_class: String,
    pub sop_instance: String,
    /// The C-STORE-RSP status ([`crate::Status::of`]).
    pub status: u16,
    pub comment: String,
    /// The transfer syntax it went in.
    pub transfer_syntax: String,
    /// Converted (not sent as stored).
    pub converted: bool,
    /// Data set bytes sent.
    pub bytes: u64,
}

impl Association {
    /// Sends one Part 10 file (or bare data set) with C-STORE. `Err` with a non-fatal
    /// error ([`Error::is_fatal`]) when only this file could not go.
    pub fn send_file(
        &mut self,
        file: &[u8],
        cancel: Option<&std::sync::atomic::AtomicBool>,
        sent: &mut dyn FnMut(u64),
    ) -> Result<Sent> {
        let h = head(file)
            .or_else(|| head_of_parsed(file))
            .ok_or(Error::Dicom(specular_dicom::Error::Malformed(
                "no SOP class or instance UID",
            )))?;
        let (ctx, data, converted) = if let Some(c) =
            self.find(&h.sop_class, Some(&h.transfer_syntax)).cloned()
        {
            (c, Cow::Borrowed(&file[h.data_set..]), false)
        } else {
            let c = self
                .accepted()
                .iter()
                .find(|c| c.abstract_syntax == h.sop_class && uncompressed_le(&c.transfer_syntax))
                .cloned()
                .ok_or_else(|| Error::NoContext(h.sop_class.clone()))?;
            let d = transcode(file, &c.transfer_syntax, cancel)?;
            (c, Cow::Owned(d), true)
        };
        let (status, comment) = self.store(&ctx, &h.sop_class, &h.sop_instance, &data, sent)?;
        Ok(Sent {
            sop_class: h.sop_class,
            sop_instance: h.sop_instance,
            status,
            comment,
            transfer_syntax: ctx.transfer_syntax,
            converted,
            bytes: data.len() as u64,
        })
    }
}

/// The data set of `file` in `to` (Explicit or Implicit VR Little Endian), compressed pixel
/// data decoded.
pub fn transcode(
    file: &[u8],
    to: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<u8>> {
    let d = Dicom::parse(file)?;
    let mut obj = Object::from_dataset(&d.dataset, d.buf(file));
    let ts: &TransferSyntax = &d.transfer_syntax;
    if ts.encapsulated() {
        specular_dicom::mask::decompress(&d, file, &mut obj, cancel)?;
        if ts.lossy {
            // PS3.3 C.7.6.1.1.5: once lossy, always marked so.
            obj.put_text(0x0028_2110, Vr::CS, "01");
        }
    }
    let mut out = Vec::with_capacity(file.len());
    if to == IMPLICIT_LE {
        implicit(&obj, &mut out)?;
    } else {
        write::encode(&obj, &mut out)?;
    }
    Ok(out)
}

/// Whether `tag` is left out (group lengths, Length to End, command and meta elements).
fn dropped(tag: u32) -> bool {
    tag & 0xFFFF == 0 || tag == 0x0008_0001 || tag >> 16 == 0 || tag >> 16 == 2
}

fn put_tag(out: &mut Vec<u8>, tag: u32) {
    out.extend_from_slice(&((tag >> 16) as u16).to_le_bytes());
    out.extend_from_slice(&(tag as u16).to_le_bytes());
}

fn patch(out: &mut [u8], at: usize) -> Result<()> {
    let len = u32::try_from(out.len() - at - 4)
        .ok()
        .filter(|&l| l != u32::MAX)
        .ok_or(Error::Dicom(specular_dicom::Error::TooLarge(
            out.len() as u64
        )))?;
    out[at..at + 4].copy_from_slice(&len.to_le_bytes());
    Ok(())
}

/// `obj` as Implicit VR Little Endian (PS3.5 7.1.3), sequences and items with defined
/// lengths.
pub fn implicit(obj: &Object, out: &mut Vec<u8>) -> Result<()> {
    for e in &obj.elements {
        if dropped(e.tag) {
            continue;
        }
        put_tag(out, e.tag);
        match &e.value {
            Value::Bytes(b) => {
                let len = u32::try_from(b.len() + (b.len() & 1))
                    .ok()
                    .filter(|&l| l != u32::MAX)
                    .ok_or(Error::Dicom(
                        specular_dicom::Error::TooLarge(b.len() as u64),
                    ))?;
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(b);
                if b.len() & 1 == 1 {
                    out.push(if e.vr.is_text() && e.vr != Vr::UI {
                        b' '
                    } else {
                        0
                    });
                }
            }
            Value::Items(items) => {
                let at = out.len();
                out.extend_from_slice(&[0; 4]);
                for item in items {
                    put_tag(out, 0xFFFE_E000);
                    let iat = out.len();
                    out.extend_from_slice(&[0; 4]);
                    implicit(item, out)?;
                    patch(out, iat)?;
                }
                patch(out, at)?;
            }
            Value::Fragments { .. } => {
                return Err(Error::Dicom(specular_dicom::Error::Unsupported(
                    "encapsulated pixel data in Implicit VR Little Endian".into(),
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contexts_put_little_endian_first() {
        let ct = "1.2.840.10008.5.1.4.1.1.2";
        let mr = "1.2.840.10008.5.1.4.1.1.4";
        let jpeg = "1.2.840.10008.1.2.4.70";
        let c = contexts([(ct, IMPLICIT_LE), (ct, jpeg), (mr, EXPLICIT_LE), (ct, jpeg)]);
        assert_eq!(c.len(), 5);
        assert_eq!((c[0].id, c[0].abstract_syntax.as_str()), (1, ct));
        assert_eq!(c[0].transfer_syntaxes, [EXPLICIT_LE, IMPLICIT_LE]);
        assert_eq!((c[1].id, c[1].abstract_syntax.as_str()), (3, mr));
        assert_eq!(
            (c[2].id, c[2].transfer_syntaxes.as_slice()),
            (5, &[IMPLICIT_LE.to_string()][..])
        );
        assert_eq!((c[3].id, c[3].transfer_syntaxes[0].as_str()), (7, jpeg));
        assert_eq!(
            (
                c[4].abstract_syntax.as_str(),
                c[4].transfer_syntaxes[0].as_str()
            ),
            (mr, EXPLICIT_LE)
        );
        // Never more than 128.
        let many: Vec<String> = (0..300).map(|i| format!("1.2.3.{i}")).collect();
        let c = contexts(many.iter().map(|s| (s.as_str(), IMPLICIT_LE)));
        assert_eq!(c.len(), MAX_CONTEXTS);
        assert_eq!(c.last().unwrap().id, 255);
    }
}
