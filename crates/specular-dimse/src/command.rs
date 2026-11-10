//! DIMSE command sets (PS3.7 section 9.3 and annex E): group 0000 elements, always Implicit
//! VR Little Endian, led by their group length (0000,0000).

use crate::{Error, Result};

/// Command Field values (PS3.7 table E.1-1).
pub mod field {
    pub const C_STORE_RQ: u16 = 0x0001;
    pub const C_STORE_RSP: u16 = 0x8001;
    pub const C_ECHO_RQ: u16 = 0x0030;
    pub const C_ECHO_RSP: u16 = 0x8030;
    pub const C_FIND_RQ: u16 = 0x0020;
    pub const C_FIND_RSP: u16 = 0x8020;
    pub const C_GET_RQ: u16 = 0x0010;
    pub const C_GET_RSP: u16 = 0x8010;
    pub const C_MOVE_RQ: u16 = 0x0021;
    pub const C_MOVE_RSP: u16 = 0x8021;
    pub const C_CANCEL_RQ: u16 = 0x0FFF;
}

/// Command Data Set Type: no data set follows.
pub const NO_DATA_SET: u16 = 0x0101;
/// Any other value: a data set follows (0x0000 is what is sent).
pub const DATA_SET: u16 = 0x0000;

const AFFECTED_SOP_CLASS: u16 = 0x0002;
const COMMAND_FIELD: u16 = 0x0100;
const MESSAGE_ID: u16 = 0x0110;
const RESPONDED_TO: u16 = 0x0120;
const MOVE_DESTINATION: u16 = 0x0600;
const PRIORITY: u16 = 0x0700;
const DATA_SET_TYPE: u16 = 0x0800;
const STATUS: u16 = 0x0900;
const ERROR_COMMENT: u16 = 0x0902;
const AFFECTED_SOP_INSTANCE: u16 = 0x1000;
const REMAINING: u16 = 0x1020;
const COMPLETED: u16 = 0x1021;
const FAILED: u16 = 0x1022;
const WARNING: u16 = 0x1023;
const MOVE_ORIGINATOR_AE: u16 = 0x1030;
const MOVE_ORIGINATOR_ID: u16 = 0x1031;

/// Sub-operation counts of a C-GET or C-MOVE response (PS3.7 9.1.3, 9.1.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub remaining: Option<u16>,
    pub completed: u16,
    pub failed: u16,
    pub warning: u16,
}

/// The command elements this crate reads and writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Command {
    pub field: u16,
    pub message_id: Option<u16>,
    pub responded_to: Option<u16>,
    pub sop_class: String,
    pub sop_instance: String,
    /// 0 medium, 1 high, 2 low.
    pub priority: Option<u16>,
    pub data_set: bool,
    pub status: Option<u16>,
    pub error_comment: String,
    /// C-MOVE: the AE title the instances go to.
    pub move_destination: String,
    /// C-GET and C-MOVE responses.
    pub counts: Counts,
    /// A C-STORE sub-operation of a C-MOVE: who asked, and its message id.
    pub move_originator: Option<(String, u16)>,
}

impl Command {
    pub fn echo_rq(id: u16) -> Command {
        Command {
            field: field::C_ECHO_RQ,
            message_id: Some(id),
            sop_class: crate::sop::VERIFICATION.to_owned(),
            ..Command::default()
        }
    }

    pub fn store_rq(id: u16, sop_class: &str, sop_instance: &str) -> Command {
        Command {
            field: field::C_STORE_RQ,
            message_id: Some(id),
            sop_class: sop_class.to_owned(),
            sop_instance: sop_instance.to_owned(),
            priority: Some(0),
            data_set: true,
            ..Command::default()
        }
    }

    /// C-FIND-RQ; the identifier follows as its data set.
    pub fn find_rq(id: u16, sop_class: &str) -> Command {
        Command {
            field: field::C_FIND_RQ,
            message_id: Some(id),
            sop_class: sop_class.to_owned(),
            priority: Some(0),
            data_set: true,
            ..Command::default()
        }
    }

    /// C-GET-RQ; the identifier follows as its data set.
    pub fn get_rq(id: u16, sop_class: &str) -> Command {
        Command {
            field: field::C_GET_RQ,
            ..Command::find_rq(id, sop_class)
        }
    }

    /// C-MOVE-RQ to `destination`; the identifier follows as its data set.
    pub fn move_rq(id: u16, sop_class: &str, destination: &str) -> Command {
        Command {
            field: field::C_MOVE_RQ,
            move_destination: destination.trim().to_owned(),
            ..Command::find_rq(id, sop_class)
        }
    }

    /// C-CANCEL-RQ for the request with message id `id`.
    pub fn cancel_rq(id: u16) -> Command {
        Command {
            field: field::C_CANCEL_RQ,
            responded_to: Some(id),
            ..Command::default()
        }
    }

    /// The response to `rq` with `status`.
    pub fn response(rq: &Command, status: u16) -> Command {
        Command {
            field: rq.field | 0x8000,
            responded_to: rq.message_id,
            sop_class: rq.sop_class.clone(),
            sop_instance: rq.sop_instance.clone(),
            status: Some(status),
            ..Command::default()
        }
    }

    /// Implicit VR Little Endian bytes, with the group length first.
    pub fn encode(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(160);
        let uid = |b: &mut Vec<u8>, e: u16, v: &str| {
            if !v.is_empty() {
                let mut s = v.as_bytes().to_vec();
                if s.len() % 2 == 1 {
                    s.push(0);
                }
                put(b, e, &s);
            }
        };
        uid(&mut body, AFFECTED_SOP_CLASS, &self.sop_class);
        put(&mut body, COMMAND_FIELD, &self.field.to_le_bytes());
        if let Some(id) = self.message_id {
            put(&mut body, MESSAGE_ID, &id.to_le_bytes());
        }
        if let Some(id) = self.responded_to {
            put(&mut body, RESPONDED_TO, &id.to_le_bytes());
        }
        if !self.move_destination.is_empty() {
            let mut s = self.move_destination.as_bytes().to_vec();
            if s.len() % 2 == 1 {
                s.push(b' ');
            }
            put(&mut body, MOVE_DESTINATION, &s);
        }
        if let Some(p) = self.priority {
            put(&mut body, PRIORITY, &p.to_le_bytes());
        }
        let ds = if self.data_set { DATA_SET } else { NO_DATA_SET };
        put(&mut body, DATA_SET_TYPE, &ds.to_le_bytes());
        if let Some(s) = self.status {
            put(&mut body, STATUS, &s.to_le_bytes());
        }
        if !self.error_comment.is_empty() {
            let mut s: Vec<u8> = self.error_comment.bytes().take(64).collect();
            if s.len() % 2 == 1 {
                s.push(b' ');
            }
            put(&mut body, ERROR_COMMENT, &s);
        }
        uid(&mut body, AFFECTED_SOP_INSTANCE, &self.sop_instance);
        let resp = self.field & 0x8000 != 0
            && matches!(self.field & 0x7FFF, field::C_GET_RQ | field::C_MOVE_RQ);
        if resp {
            if let Some(r) = self.counts.remaining {
                put(&mut body, REMAINING, &r.to_le_bytes());
            }
            put(&mut body, COMPLETED, &self.counts.completed.to_le_bytes());
            put(&mut body, FAILED, &self.counts.failed.to_le_bytes());
            put(&mut body, WARNING, &self.counts.warning.to_le_bytes());
        }
        if let Some((ae, id)) = &self.move_originator {
            let mut s = ae.as_bytes().to_vec();
            if s.len() % 2 == 1 {
                s.push(b' ');
            }
            put(&mut body, MOVE_ORIGINATOR_AE, &s);
            put(&mut body, MOVE_ORIGINATOR_ID, &id.to_le_bytes());
        }
        let mut out = Vec::with_capacity(body.len() + 12);
        put(&mut out, 0x0000, &(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Reads a command set (elements of other groups or unknown ones are skipped).
    pub fn decode(mut b: &[u8]) -> Result<Command> {
        let mut c = Command::default();
        let mut has_field = false;
        while !b.is_empty() {
            if b.len() < 8 {
                return Err(Error::Protocol("truncated command".into()));
            }
            let group = u16::from_le_bytes([b[0], b[1]]);
            let elem = u16::from_le_bytes([b[2], b[3]]);
            let len = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
            let v = b
                .get(8..8 + len)
                .ok_or_else(|| Error::Protocol("command element past its end".into()))?;
            b = &b[8 + len..];
            if group != 0 {
                continue;
            }
            let us = || (v.len() >= 2).then(|| u16::from_le_bytes([v[0], v[1]]));
            let text = || {
                String::from_utf8_lossy(v)
                    .trim_end_matches(['\0', ' '])
                    .to_owned()
            };
            match elem {
                AFFECTED_SOP_CLASS => c.sop_class = text(),
                COMMAND_FIELD => {
                    c.field = us().unwrap_or(0);
                    has_field = true;
                }
                MESSAGE_ID => c.message_id = us(),
                RESPONDED_TO => c.responded_to = us(),
                PRIORITY => c.priority = us(),
                DATA_SET_TYPE => c.data_set = us().is_some_and(|t| t != NO_DATA_SET),
                STATUS => c.status = us(),
                ERROR_COMMENT => c.error_comment = text(),
                AFFECTED_SOP_INSTANCE => c.sop_instance = text(),
                MOVE_DESTINATION => c.move_destination = text().trim().to_owned(),
                REMAINING => c.counts.remaining = us(),
                COMPLETED => c.counts.completed = us().unwrap_or(0),
                FAILED => c.counts.failed = us().unwrap_or(0),
                WARNING => c.counts.warning = us().unwrap_or(0),
                MOVE_ORIGINATOR_AE => {
                    let id = c.move_originator.as_ref().map_or(0, |o| o.1);
                    c.move_originator = Some((text().trim().to_owned(), id));
                }
                MOVE_ORIGINATOR_ID => {
                    let id = us().unwrap_or(0);
                    match c.move_originator.as_mut() {
                        Some(o) => o.1 = id,
                        None => c.move_originator = Some((String::new(), id)),
                    }
                }
                _ => {}
            }
        }
        if !has_field {
            return Err(Error::Protocol("command without a Command Field".into()));
        }
        Ok(c)
    }
}

fn put(out: &mut Vec<u8>, elem: u16, v: &[u8]) {
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&elem.to_le_bytes());
    out.extend_from_slice(&(v.len() as u32).to_le_bytes());
    out.extend_from_slice(v);
}

/// What a C-STORE or C-ECHO status means (PS3.7 annex C and PS3.4 table B.2-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    /// Stored, with a warning (coercion, elements discarded, data set does not match).
    Warning,
    /// The server is out of resources (0xA7xx).
    OutOfResources,
    /// The data set does not match the SOP class (0xA9xx).
    DataSetMismatch,
    /// The server cannot understand it (0xCxxx).
    CannotUnderstand,
    /// The SOP class is not supported (0x0122).
    SopClassNotSupported,
    /// Not authorised (0x0124).
    NotAuthorised,
    /// Any other failure.
    Failed,
}

impl Status {
    pub fn of(code: u16) -> Status {
        match code {
            0x0000 => Status::Success,
            0x0001 | 0x0107 | 0x0116 | 0xB000..=0xBFFF => Status::Warning,
            0xA700..=0xA7FF => Status::OutOfResources,
            0xA900..=0xA9FF => Status::DataSetMismatch,
            0xC000..=0xCFFF => Status::CannotUnderstand,
            0x0122 => Status::SopClassNotSupported,
            0x0124 => Status::NotAuthorised,
            _ => Status::Failed,
        }
    }

    pub fn stored(self) -> bool {
        matches!(self, Status::Success | Status::Warning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_rq_round_trips_with_group_length() {
        let c = Command::store_rq(7, "1.2.840.10008.5.1.4.1.1.2", "1.2.3.4.5");
        let b = c.encode();
        // (0000,0000) UL, 4 bytes, the rest's length.
        assert_eq!(&b[..8], &[0, 0, 0, 0, 4, 0, 0, 0]);
        assert_eq!(
            u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize,
            b.len() - 12
        );
        // UIDs padded with NUL to even length.
        assert_eq!(b.len() % 2, 0);
        assert_eq!(Command::decode(&b).unwrap(), c);
        let r = Command::response(&c, 0xB000);
        let back = Command::decode(&r.encode()).unwrap();
        assert_eq!(back.field, field::C_STORE_RSP);
        assert_eq!(back.responded_to, Some(7));
        assert!(!back.data_set);
        assert_eq!(Status::of(back.status.unwrap()), Status::Warning);
    }

    #[test]
    fn retrieve_commands_round_trip() {
        let g = Command::get_rq(3, "1.2.840.10008.5.1.4.1.2.2.3");
        assert_eq!(Command::decode(&g.encode()).unwrap(), g);
        let m = Command::move_rq(4, "1.2.840.10008.5.1.4.1.2.2.2", "SPECULAR");
        let back = Command::decode(&m.encode()).unwrap();
        assert_eq!(back.move_destination, "SPECULAR");
        assert_eq!(back, m);
        let mut r = Command::response(&g, 0xFF00);
        r.counts = Counts {
            remaining: Some(7),
            completed: 2,
            failed: 1,
            warning: 0,
        };
        let back = Command::decode(&r.encode()).unwrap();
        assert_eq!(back.counts, r.counts);
        let mut s = Command::store_rq(9, "1.2", "1.2.3");
        s.move_originator = Some(("SCU".into(), 4));
        assert_eq!(Command::decode(&s.encode()).unwrap(), s);
    }

    #[test]
    fn statuses() {
        assert!(Status::of(0).stored());
        assert_eq!(Status::of(0xA701), Status::OutOfResources);
        assert_eq!(Status::of(0xC123), Status::CannotUnderstand);
        assert!(!Status::of(0x0211).stored());
    }
}
