//! Upper-layer PDUs (PS3.8 section 9.3): their encoding and decoding.
//!
//! Every PDU is a type byte, a reserved byte and a 32-bit big-endian length, then that many
//! bytes. A-ASSOCIATE-RQ/AC carry items (application context, presentation contexts, user
//! information) with 16-bit big-endian lengths. UIDs in items are sent without padding and
//! read with trailing NULs and spaces trimmed.

use std::io::Read;

use crate::{Error, Result};

/// The DICOM Application Context Name (PS3.7 Annex A.2.1).
pub const APPLICATION_CONTEXT: &str = "1.2.840.10008.3.1.1.1";

/// Longest PDU accepted that is not P-DATA (associations are small; this only bounds memory).
const MAX_CONTROL_PDU: u32 = 1 << 20;

/// A presentation context offered (A-ASSOCIATE-RQ).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRq {
    /// Odd, 1 to 255.
    pub id: u8,
    pub abstract_syntax: String,
    pub transfer_syntaxes: Vec<String>,
}

/// Results of a presentation context (PS3.8 table 9-18).
pub mod result {
    pub const ACCEPTANCE: u8 = 0;
    pub const USER_REJECTION: u8 = 1;
    pub const NO_REASON: u8 = 2;
    pub const ABSTRACT_SYNTAX_NOT_SUPPORTED: u8 = 3;
    pub const TRANSFER_SYNTAXES_NOT_SUPPORTED: u8 = 4;
}

/// A presentation context answered (A-ASSOCIATE-AC).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextAc {
    pub id: u8,
    /// [`result`]: 0 is acceptance.
    pub result: u8,
    /// The one transfer syntax chosen (meaningless unless accepted).
    pub transfer_syntax: String,
}

/// The A-ASSOCIATE-RQ and -AC fields used here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Associate {
    pub called_ae: String,
    pub calling_ae: String,
    pub application_context: String,
    pub requested: Vec<ContextRq>,
    pub answered: Vec<ContextAc>,
    /// Maximum Length Received (0: no limit).
    pub max_pdu: u32,
    pub implementation_class: String,
    pub implementation_version: String,
    /// SCP/SCU Role Selection (PS3.7 D.3.3.4): proposed in a request, answered in an
    /// acceptance.
    pub roles: Vec<Role>,
}

/// One SCP/SCU Role Selection sub-item: which roles the requestor takes for a SOP class.
/// C-GET needs the requestor as SCP of the storage classes it retrieves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub sop_class: String,
    pub scu: bool,
    pub scp: bool,
}

impl Default for Associate {
    fn default() -> Self {
        Associate {
            called_ae: String::new(),
            calling_ae: String::new(),
            application_context: APPLICATION_CONTEXT.to_owned(),
            requested: Vec::new(),
            answered: Vec::new(),
            max_pdu: 0,
            implementation_class: specular_dicom::write::IMPLEMENTATION_CLASS_UID.to_owned(),
            implementation_version: specular_dicom::write::IMPLEMENTATION_VERSION
                [..specular_dicom::write::IMPLEMENTATION_VERSION.len().min(16)]
                .to_owned(),
            roles: Vec::new(),
        }
    }
}

/// One presentation data value (PS3.8 9.3.5.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pdv {
    pub context: u8,
    /// A command fragment (else data set).
    pub command: bool,
    /// The last fragment of the command or data set.
    pub last: bool,
    pub data: Vec<u8>,
}

/// A PDU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pdu {
    AssociateRq(Associate),
    AssociateAc(Associate),
    AssociateRj { result: u8, source: u8, reason: u8 },
    PData(Vec<Pdv>),
    ReleaseRq,
    ReleaseRp,
    Abort { source: u8, reason: u8 },
}

/// An AE title as sent: 16 bytes, padded with spaces (PS3.8 9.3.2).
fn ae_bytes(ae: &str) -> [u8; 16] {
    let mut b = [b' '; 16];
    for (d, s) in b.iter_mut().zip(ae.bytes()) {
        *d = s;
    }
    b
}

/// Whether `ae` is a usable AE title: 1 to 16 characters of the default repertoire, no
/// backslash or control characters, not only spaces (PS3.5 table 6.2-1).
pub fn valid_ae(ae: &str) -> bool {
    let t = ae.trim();
    !t.is_empty() && ae.len() <= 16 && ae.bytes().all(|b| (0x20..0x7F).contains(&b) && b != b'\\')
}

fn item(out: &mut Vec<u8>, kind: u8, body: &[u8]) {
    out.push(kind);
    out.push(0);
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(body);
}

fn user_info(a: &Associate) -> Vec<u8> {
    let mut u = Vec::new();
    item(&mut u, 0x51, &a.max_pdu.to_be_bytes());
    item(&mut u, 0x52, a.implementation_class.as_bytes());
    if !a.implementation_version.is_empty() {
        item(&mut u, 0x55, a.implementation_version.as_bytes());
    }
    for r in &a.roles {
        let uid = r.sop_class.as_bytes();
        let mut b = Vec::with_capacity(uid.len() + 4);
        b.extend_from_slice(&(uid.len() as u16).to_be_bytes());
        b.extend_from_slice(uid);
        b.push(u8::from(r.scu));
        b.push(u8::from(r.scp));
        item(&mut u, 0x54, &b);
    }
    u
}

fn associate_body(a: &Associate, rq: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(256);
    b.extend_from_slice(&1u16.to_be_bytes());
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&ae_bytes(&a.called_ae));
    b.extend_from_slice(&ae_bytes(&a.calling_ae));
    b.extend_from_slice(&[0; 32]);
    item(&mut b, 0x10, a.application_context.as_bytes());
    if rq {
        for c in &a.requested {
            let mut p = vec![c.id, 0, 0, 0];
            item(&mut p, 0x30, c.abstract_syntax.as_bytes());
            for ts in &c.transfer_syntaxes {
                item(&mut p, 0x40, ts.as_bytes());
            }
            item(&mut b, 0x20, &p);
        }
    } else {
        for c in &a.answered {
            let mut p = vec![c.id, 0, c.result, 0];
            item(&mut p, 0x40, c.transfer_syntax.as_bytes());
            item(&mut b, 0x21, &p);
        }
    }
    item(&mut b, 0x50, &user_info(a));
    b
}

/// The bytes of `pdu`.
pub fn encode(pdu: &Pdu) -> Vec<u8> {
    let (kind, body) = match pdu {
        Pdu::AssociateRq(a) => (0x01, associate_body(a, true)),
        Pdu::AssociateAc(a) => (0x02, associate_body(a, false)),
        Pdu::AssociateRj {
            result,
            source,
            reason,
        } => (0x03, vec![0, *result, *source, *reason]),
        Pdu::PData(pdvs) => {
            let mut b = Vec::with_capacity(pdvs.iter().map(|p| p.data.len() + 6).sum());
            for p in pdvs {
                b.extend_from_slice(&((p.data.len() + 2) as u32).to_be_bytes());
                b.push(p.context);
                b.push(u8::from(p.command) | (u8::from(p.last) << 1));
                b.extend_from_slice(&p.data);
            }
            (0x04, b)
        }
        Pdu::ReleaseRq => (0x05, vec![0; 4]),
        Pdu::ReleaseRp => (0x06, vec![0; 4]),
        Pdu::Abort { source, reason } => (0x07, vec![0, 0, *source, *reason]),
    };
    let mut out = Vec::with_capacity(body.len() + 6);
    out.push(kind);
    out.push(0);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// The 6-byte header of a P-DATA-TF holding one PDV of `len` data bytes, followed by the PDV
/// header: the caller writes the data after it (no copy of large data sets).
pub fn pdata_header(context: u8, command: bool, last: bool, len: usize) -> [u8; 12] {
    let mut h = [0u8; 12];
    h[0] = 0x04;
    h[2..6].copy_from_slice(&((len + 6) as u32).to_be_bytes());
    h[6..10].copy_from_slice(&((len + 2) as u32).to_be_bytes());
    h[10] = context;
    h[11] = u8::from(command) | (u8::from(last) << 1);
    h
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches(['\0', ' '])
        .trim_start()
        .to_owned()
}

fn bad(what: &str) -> Error {
    Error::Protocol(what.to_owned())
}

/// Splits `b` into items: (type, body).
fn items(mut b: &[u8]) -> Result<Vec<(u8, &[u8])>> {
    let mut v = Vec::new();
    while !b.is_empty() {
        if b.len() < 4 {
            return Err(bad("truncated item"));
        }
        let len = u16::from_be_bytes([b[2], b[3]]) as usize;
        let body = b.get(4..4 + len).ok_or_else(|| bad("item past its PDU"))?;
        v.push((b[0], body));
        b = &b[4 + len..];
    }
    Ok(v)
}

fn parse_associate(b: &[u8], rq: bool) -> Result<Associate> {
    if b.len() < 68 {
        return Err(bad("short A-ASSOCIATE"));
    }
    let mut a = Associate {
        called_ae: text(&b[4..20]),
        calling_ae: text(&b[20..36]),
        application_context: String::new(),
        implementation_class: String::new(),
        implementation_version: String::new(),
        ..Associate::default()
    };
    for (kind, body) in items(&b[68..])? {
        match kind {
            0x10 => a.application_context = text(body),
            0x20 if rq => {
                if body.len() < 4 {
                    return Err(bad("short presentation context"));
                }
                let mut c = ContextRq {
                    id: body[0],
                    abstract_syntax: String::new(),
                    transfer_syntaxes: Vec::new(),
                };
                for (k, s) in items(&body[4..])? {
                    match k {
                        0x30 => c.abstract_syntax = text(s),
                        0x40 => c.transfer_syntaxes.push(text(s)),
                        _ => {}
                    }
                }
                a.requested.push(c);
            }
            0x21 if !rq => {
                if body.len() < 4 {
                    return Err(bad("short presentation context"));
                }
                let mut c = ContextAc {
                    id: body[0],
                    result: body[2],
                    transfer_syntax: String::new(),
                };
                for (k, s) in items(&body[4..])? {
                    if k == 0x40 {
                        c.transfer_syntax = text(s);
                    }
                }
                a.answered.push(c);
            }
            0x50 => {
                for (k, s) in items(body)? {
                    match k {
                        0x51 if s.len() == 4 => {
                            a.max_pdu = u32::from_be_bytes([s[0], s[1], s[2], s[3]]);
                        }
                        0x52 => a.implementation_class = text(s),
                        0x55 => a.implementation_version = text(s),
                        0x54 if s.len() >= 2 => {
                            let n = u16::from_be_bytes([s[0], s[1]]) as usize;
                            let (Some(uid), Some(&scu), Some(&scp)) =
                                (s.get(2..2 + n), s.get(2 + n), s.get(3 + n))
                            else {
                                return Err(bad("short role selection"));
                            };
                            a.roles.push(Role {
                                sop_class: text(uid),
                                scu: scu != 0,
                                scp: scp != 0,
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(a)
}

fn parse_pdvs(mut b: &[u8]) -> Result<Vec<Pdv>> {
    let mut v = Vec::new();
    while !b.is_empty() {
        if b.len() < 6 {
            return Err(bad("truncated PDV"));
        }
        let len = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
        if len < 2 {
            return Err(bad("empty PDV"));
        }
        let body = b.get(4..4 + len).ok_or_else(|| bad("PDV past its PDU"))?;
        v.push(Pdv {
            context: body[0],
            command: body[1] & 1 != 0,
            last: body[1] & 2 != 0,
            data: body[2..].to_vec(),
        });
        b = &b[4 + len..];
    }
    Ok(v)
}

/// Decodes one PDU from its type byte and body.
pub fn decode(kind: u8, body: &[u8]) -> Result<Pdu> {
    let four = |b: &[u8]| -> Result<[u8; 4]> {
        b.get(..4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| bad("short PDU"))
    };
    Ok(match kind {
        0x01 => Pdu::AssociateRq(parse_associate(body, true)?),
        0x02 => Pdu::AssociateAc(parse_associate(body, false)?),
        0x03 => {
            let b = four(body)?;
            Pdu::AssociateRj {
                result: b[1],
                source: b[2],
                reason: b[3],
            }
        }
        0x04 => Pdu::PData(parse_pdvs(body)?),
        0x05 => Pdu::ReleaseRq,
        0x06 => Pdu::ReleaseRp,
        0x07 => {
            let b = four(body)?;
            Pdu::Abort {
                source: b[2],
                reason: b[3],
            }
        }
        k => return Err(Error::Protocol(format!("unknown PDU type {k:#04x}"))),
    })
}

/// Reads one PDU; a P-DATA-TF longer than `max_pdata` (the length we announced, plus its
/// 6-byte header slack) or any other PDU over 1 MiB is a protocol error.
pub fn read(r: &mut impl Read, max_pdata: u32) -> Result<Pdu> {
    let mut h = [0u8; 6];
    r.read_exact(&mut h).map_err(Error::from_read)?;
    read_body(h, r, max_pdata)
}

/// The rest of a PDU whose 6-byte header `h` was read.
pub(crate) fn read_body(h: [u8; 6], r: &mut impl Read, max_pdata: u32) -> Result<Pdu> {
    let len = u32::from_be_bytes([h[2], h[3], h[4], h[5]]);
    let cap = if h[0] == 0x04 {
        if max_pdata == 0 { u32::MAX } else { max_pdata }
    } else {
        MAX_CONTROL_PDU
    };
    if len > cap {
        return Err(Error::Protocol(format!(
            "PDU of {len} bytes, more than the {cap} allowed"
        )));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body).map_err(Error::from_read)?;
    decode(h[0], &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associate_round_trips() {
        let a = Associate {
            called_ae: "PACS".into(),
            calling_ae: "SPECULAR".into(),
            requested: vec![ContextRq {
                id: 1,
                abstract_syntax: "1.2.840.10008.1.1".into(),
                transfer_syntaxes: vec!["1.2.840.10008.1.2".into(), "1.2.840.10008.1.2.1".into()],
            }],
            max_pdu: 16384,
            roles: vec![Role {
                sop_class: "1.2.840.10008.5.1.4.1.1.2".into(),
                scu: false,
                scp: true,
            }],
            ..Associate::default()
        };
        let b = encode(&Pdu::AssociateRq(a.clone()));
        // Type, reserved, length; protocol version 1; AE titles padded to 16 with spaces.
        assert_eq!(b[0], 1);
        assert_eq!(
            u32::from_be_bytes([b[2], b[3], b[4], b[5]]) as usize,
            b.len() - 6
        );
        assert_eq!(&b[6..8], &[0, 1]);
        assert_eq!(&b[10..26], b"PACS            ");
        let back = read(&mut &b[..], 0).unwrap();
        assert_eq!(back, Pdu::AssociateRq(a));
    }

    #[test]
    fn pdata_header_matches_encode() {
        let p = Pdu::PData(vec![Pdv {
            context: 3,
            command: true,
            last: true,
            data: vec![9; 10],
        }]);
        let b = encode(&p);
        assert_eq!(&b[..12], &pdata_header(3, true, true, 10));
        assert_eq!(read(&mut &b[..], 64).unwrap(), p);
        // Longer than announced: refused.
        assert!(matches!(read(&mut &b[..], 8), Err(Error::Protocol(_))));
    }

    #[test]
    fn ae_titles() {
        assert!(valid_ae("SPECULAR"));
        assert!(valid_ae("A B"));
        assert!(!valid_ae(""));
        assert!(!valid_ae("   "));
        assert!(!valid_ae("SEVENTEEN_CHARS_X"));
        assert!(!valid_ae("BACK\\SLASH"));
        assert!(!valid_ae("TAB\t"));
    }
}
