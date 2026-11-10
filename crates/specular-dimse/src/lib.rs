//! DICOM networking for Specular: the upper layer (PS3.8) and DIMSE (PS3.7), enough to send
//! studies to a PACS.
//!
//! - [`pdu`]: A-ASSOCIATE-RQ/AC/RJ, P-DATA-TF, A-RELEASE, A-ABORT.
//! - [`command`]: DIMSE command sets (Implicit VR Little Endian, group 0000).
//! - [`assoc`]: an association as the requestor (SCU): connect with timeouts, negotiate
//!   presentation contexts, send and receive messages fragmented to the peer's maximum PDU
//!   length, C-ECHO, C-STORE, release and abort. Every wait can be cancelled.
//! - [`store`]: what to propose for a set of files, and each file sent as stored when the
//!   server accepted its transfer syntax, else as Explicit or Implicit VR Little Endian
//!   (compressed pixel data decoded by the in-tree decoders).
//! - [`tls`]: DICOM over TLS, the server verified against the system trust store.
//! - [`find`]: C-FIND SCU, Study Root at the study and series levels.
//! - [`retrieve`]: C-GET SCU (instances on the same association) and the C-MOVE SCU half.
//! - [`listen`]: C-MOVE with a temporary storage SCP, listening only while the move runs.
//! - [`scp`]: a minimal C-ECHO/C-STORE/C-FIND SCP on loopback, for tests and screenshots.
//!
//! Nothing here connects by itself: an association follows a user action (Verify, Send) on
//! a server the user added. The no-network switch refuses it before any name lookup.

pub mod assoc;
pub mod command;
pub mod find;
pub mod listen;
pub mod pdu;
pub mod retrieve;
#[doc(hidden)]
pub mod scp;
pub mod sop;
pub mod store;
pub mod tls;

pub use assoc::{Accepted, Association, Options, Peer};
pub use command::Status;
pub use store::{Head, Sent};
pub use tls::{Tls, TlsError};

/// Why an association or a message failed.
#[derive(Debug)]
pub enum Error {
    /// The no-network switch is on.
    NetworkOff,
    /// The host name could not be resolved.
    UnknownHost,
    /// No connection (refused, unreachable).
    Connect(std::io::Error),
    /// No answer in time.
    Timeout,
    Cancelled,
    /// The connection closed or failed mid-way.
    Io(std::io::Error),
    /// The server refused the association (PS3.8 table 9-21).
    Rejected {
        result: u8,
        source: u8,
        reason: u8,
    },
    /// The association was aborted (by the server when `source` is 2, else by its side).
    Aborted {
        source: u8,
        reason: u8,
    },
    /// The server broke the protocol.
    Protocol(String),
    /// None of the presentation contexts offered was accepted.
    NothingAccepted,
    /// The server accepted no presentation context for this SOP class (its UID).
    NoContext(String),
    /// The file could not be read or converted.
    Dicom(specular_dicom::Error),
    /// The Verification answered with a failure status.
    Status(u16),
    /// DICOM over TLS: the secure connection could not be made.
    Tls(tls::TlsError),
    /// C-MOVE: the storage SCP could not listen on the address and port given.
    Listen(std::io::Error),
}

/// Why the server refused an association, in the terms a user can act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// The server's AE title is not the one it answers to.
    CalledAe,
    /// The server does not know our AE title.
    CallingAe,
    /// Too busy or out of resources for now.
    Busy,
    Other,
}

impl Error {
    /// Whether the association cannot go on (else only this file failed).
    pub fn is_fatal(&self) -> bool {
        !matches!(self, Error::NoContext(_) | Error::Dicom(_))
    }

    pub fn rejection(&self) -> Option<Rejection> {
        let Error::Rejected { source, reason, .. } = *self else {
            return None;
        };
        Some(match (source, reason) {
            (1, 7) => Rejection::CalledAe,
            (1, 3) => Rejection::CallingAe,
            (3, 1 | 2) => Rejection::Busy,
            _ => Rejection::Other,
        })
    }

    /// From a read: end of stream and the cancel/time-out markers of [`assoc`].
    pub(crate) fn from_read(e: std::io::Error) -> Error {
        if let Some(m) = e.get_ref().and_then(|r| r.downcast_ref::<assoc::Stop>()) {
            return match m {
                assoc::Stop::Cancelled => Error::Cancelled,
                assoc::Stop::Timeout => Error::Timeout,
            };
        }
        match e.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => Error::Timeout,
            _ => Error::Io(e),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NetworkOff => f.write_str("the network is switched off"),
            Error::UnknownHost => f.write_str("unknown host"),
            Error::Connect(e) => write!(f, "could not connect: {e}"),
            Error::Timeout => f.write_str("no answer in time"),
            Error::Cancelled => f.write_str("cancelled"),
            Error::Io(e) => write!(f, "connection failed: {e}"),
            Error::Rejected {
                result,
                source,
                reason,
            } => write!(
                f,
                "association rejected (result {result}, source {source}, reason {reason})"
            ),
            Error::Aborted { source, reason } => {
                write!(f, "association aborted (source {source}, reason {reason})")
            }
            Error::Protocol(s) => write!(f, "protocol error: {s}"),
            Error::NothingAccepted => f.write_str("no presentation context accepted"),
            Error::NoContext(c) => write!(f, "{} not accepted by the server", sop::name(c)),
            Error::Dicom(e) => e.fmt(f),
            Error::Status(s) => write!(f, "status {s:#06x}"),
            Error::Tls(e) => e.fmt(f),
            Error::Listen(e) => write!(f, "could not listen: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<specular_dicom::Error> for Error {
    fn from(e: specular_dicom::Error) -> Self {
        Error::Dicom(e)
    }
}

pub(crate) use find::vr_of as dict_vr;

pub type Result<T> = std::result::Result<T, Error>;
