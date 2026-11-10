//! Playing a file on a TV or speaker on the local network: DLNA/UPnP media renderers.
//!
//! - [`find`]: SSDP discovery ([`ssdp`]) of MediaRenderers and their descriptions
//!   ([`device`]), on a worker, only while the app asks (its "Play on…" menu is open).
//! - [`session::start`]: one file on one renderer. Checks the renderer takes the file's type
//!   ([`accept`]), serves the file over a small HTTP server ([`serve`]) bound to the
//!   interface that reaches the renderer (only that file, under a random path, with ranges;
//!   it stops with the session), and drives AVTransport and RenderingControl ([`renderer`],
//!   [`soap`]): set the URI, play, pause, seek, stop, volume; and reports where it is.
//! - [`testing`]: an in-process fake renderer (SSDP responder, description and SOAP
//!   endpoints on loopback) for tests.
//!
//! Nothing here transcodes: a renderer that does not list the file's type is refused
//! ([`Error::NotAccepted`]). Every socket checks the no-network switch first
//! ([`specular_core::net::off`]); nothing connects without the user acting.
//!
//! Google Cast (CASTV2) is not here: it has no public specification, and its receivers'
//! TLS certificates are only trusted through a device-authentication chain we cannot verify
//! honestly.

pub mod accept;
pub mod device;
pub mod didl;
pub mod find;
pub mod http;
pub mod renderer;
pub mod serve;
pub mod session;
pub mod soap;
pub mod ssdp;
pub mod testing;
mod xml;

use std::fmt;

pub use device::Device;
pub use find::{Finder, Found};
pub use renderer::{Renderer, TransportState};
pub use session::{Cmd, Event, Media, Session, Status};

/// Why talking to a renderer failed (the app words it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The no-network switch is on: nothing was sent.
    NetworkOff,
    /// The device could not be reached (refused, unreachable, reset).
    Unreachable,
    /// The device did not answer in time.
    Timeout,
    /// The device answered with this HTTP status.
    Status(u16),
    /// The device refused the action (a UPnP fault: its code and description).
    Fault(u32, String),
    /// The answer made no sense (not XML, missing fields, too large).
    BadReply(&'static str),
    /// The description is not of a media renderer (no AVTransport service).
    NotARenderer,
    /// The renderer does not list this type (MIME) among those it plays.
    NotAccepted(String),
    /// The file could not be read or served.
    File(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NetworkOff => f.write_str(specular_core::net::off::MESSAGE),
            Error::Unreachable => f.write_str("the device could not be reached"),
            Error::Timeout => f.write_str("the device did not answer in time"),
            Error::Status(s) => write!(f, "the device answered HTTP {s}"),
            Error::Fault(c, d) => write!(f, "the device refused: {c} {d}"),
            Error::BadReply(w) => write!(f, "the device's answer is broken: {w}"),
            Error::NotARenderer => f.write_str("not a media renderer"),
            Error::NotAccepted(m) => write!(f, "the device does not play {m}"),
            Error::File(e) => write!(f, "the file cannot be served: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        use std::io::ErrorKind as K;
        if specular_core::net::off::is_refusal(&e) {
            return Error::NetworkOff;
        }
        match e.kind() {
            K::TimedOut | K::WouldBlock => Error::Timeout,
            _ => Error::Unreachable,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether `ip` is on the local network: private, link-local or loopback. Device and
/// control addresses must be (a reply on the LAN cannot send us to a server elsewhere).
pub fn is_local(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_link_local() || v.is_loopback(),
        IpAddr::V6(v) => {
            let s = v.segments()[0];
            v.is_loopback() || (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_addresses() {
        for a in [
            "192.168.1.20",
            "10.0.0.1",
            "172.20.3.4",
            "169.254.9.9",
            "127.0.0.1",
            "::1",
            "fe80::1",
            "fd00::5",
        ] {
            assert!(is_local(a.parse().unwrap()), "{a}");
        }
        for a in ["8.8.8.8", "172.32.0.1", "2001:db8::1", "1.1.1.1"] {
            assert!(!is_local(a.parse().unwrap()), "{a}");
        }
    }

    #[test]
    fn io_errors_map() {
        assert_eq!(
            Error::from(specular_core::net::off::io_error()),
            Error::NetworkOff
        );
        assert_eq!(
            Error::from(std::io::Error::from(std::io::ErrorKind::TimedOut)),
            Error::Timeout
        );
        assert_eq!(
            Error::from(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            Error::Unreachable
        );
    }
}
