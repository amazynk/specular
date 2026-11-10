//! DICOM over TLS (PS3.15 annex B.1, the Basic TLS Secure Transport Connection Profile):
//! the association runs inside a TLS session (rustls with ring, TLS 1.2 or 1.3).
//!
//! The server's certificate is checked as the DICOMweb path checks it: against the
//! operating system's trust store (hospital CAs) through `rustls-platform-verifier`, with
//! the host name or IP address as typed. There is no "trust anyway".

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Instant;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use specular_core::CancelToken;

use crate::{Error, Result};

/// Why a TLS session could not be set up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TlsError {
    /// The server's certificate is not trusted (unknown issuer, wrong name, expired,
    /// revoked); the detail is for the log.
    Certificate(String),
    /// Anything else (no common protocol version or cipher, a bad host name).
    Handshake(String),
}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsError::Certificate(s) => write!(f, "certificate not trusted: {s}"),
            TlsError::Handshake(s) => write!(f, "TLS handshake failed: {s}"),
        }
    }
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn tls_error(e: &rustls::Error) -> TlsError {
    match e {
        rustls::Error::InvalidCertificate(c) => TlsError::Certificate(format!("{c:?}")),
        e => TlsError::Handshake(e.to_string()),
    }
}

/// How the server is verified.
#[derive(Clone)]
pub struct Tls {
    config: Arc<rustls::ClientConfig>,
}

impl std::fmt::Debug for Tls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tls")
    }
}

impl Tls {
    /// The operating system's trust store.
    pub fn system() -> Result<Tls> {
        let p = provider();
        let verifier = rustls_platform_verifier::Verifier::new(p.clone())
            .map_err(|e| Error::Tls(tls_error(&e)))?;
        let config = rustls::ClientConfig::builder_with_provider(p)
            .with_safe_default_protocol_versions()
            .map_err(|e| Error::Tls(tls_error(&e)))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        Ok(Tls {
            config: Arc::new(config),
        })
    }

    /// Only these roots (DER), nothing from the system: for tests against a loopback
    /// server with its own CA.
    pub fn trusting(roots: &[&[u8]]) -> Result<Tls> {
        let mut store = rustls::RootCertStore::empty();
        for r in roots {
            store
                .add(CertificateDer::from(r.to_vec()))
                .map_err(|e| Error::Tls(tls_error(&e)))?;
        }
        let config = rustls::ClientConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(|e| Error::Tls(tls_error(&e)))?
            .with_root_certificates(store)
            .with_no_client_auth();
        Ok(Tls {
            config: Arc::new(config),
        })
    }
}

/// The connection under an association: plain TCP, or TLS as client or (test SCP) server.
pub(crate) enum Stream {
    Plain(TcpStream),
    Client(Box<StreamOwned<ClientConnection, TcpStream>>),
    Server(Box<StreamOwned<ServerConnection, TcpStream>>),
}

impl Stream {
    pub fn tcp(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Client(s) => &s.sock,
            Stream::Server(s) => &s.sock,
        }
    }

    /// Ends the session (close_notify for TLS) and the connection.
    pub fn shutdown(&mut self) {
        match self {
            Stream::Plain(_) => {}
            Stream::Client(s) => {
                s.conn.send_close_notify();
                let _ = s.conn.complete_io(&mut s.sock);
            }
            Stream::Server(s) => {
                s.conn.send_close_notify();
                let _ = s.conn.complete_io(&mut s.sock);
            }
        }
        let _ = self.tcp().shutdown(std::net::Shutdown::Both);
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Client(s) => s.read(buf),
            Stream::Server(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Client(s) => s.write(buf),
            Stream::Server(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Client(s) => s.flush(),
            Stream::Server(s) => s.flush(),
        }
    }
}

/// Runs a handshake to its end on a socket with a short read timeout, until `deadline`.
fn handshake<C, S>(
    conn: &mut C,
    sock: &mut TcpStream,
    deadline: Instant,
    cancel: &CancelToken,
) -> Result<()>
where
    C: std::ops::DerefMut<Target = rustls::ConnectionCommon<S>>,
    S: rustls::SideData,
{
    while conn.is_handshaking() {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        match conn.complete_io(sock) {
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => {
                if let Some(r) = e.get_ref().and_then(|r| r.downcast_ref::<rustls::Error>()) {
                    return Err(Error::Tls(tls_error(r)));
                }
                return Err(match e.kind() {
                    io::ErrorKind::UnexpectedEof => Error::Tls(TlsError::Handshake(
                        "the server closed the connection".into(),
                    )),
                    _ => Error::Io(e),
                });
            }
        }
    }
    Ok(())
}

/// TLS as the client to `host` over `sock` (whose read timeout is already short).
pub(crate) fn connect(
    tls: &Tls,
    host: &str,
    mut sock: TcpStream,
    deadline: Instant,
    cancel: &CancelToken,
) -> Result<Stream> {
    let name = ServerName::try_from(host.to_owned())
        .map_err(|_| Error::Tls(TlsError::Handshake(format!("{host:?} is not a host name"))))?;
    let mut conn =
        ClientConnection::new(tls.config.clone(), name).map_err(|e| Error::Tls(tls_error(&e)))?;
    handshake(&mut conn, &mut sock, deadline, cancel)?;
    Ok(Stream::Client(Box::new(StreamOwned::new(conn, sock))))
}

/// A server identity for the test SCP: certificate chain and PKCS #8 key (DER).
#[derive(Clone)]
pub struct ServerIdentity {
    config: Arc<rustls::ServerConfig>,
}

impl std::fmt::Debug for ServerIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServerIdentity")
    }
}

impl ServerIdentity {
    pub fn new(chain: &[&[u8]], key_pkcs8: &[u8]) -> Result<ServerIdentity> {
        let certs = chain
            .iter()
            .map(|c| CertificateDer::from(c.to_vec()))
            .collect();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pkcs8.to_vec()));
        let config = rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(|e| Error::Tls(tls_error(&e)))?
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| Error::Tls(tls_error(&e)))?;
        Ok(ServerIdentity {
            config: Arc::new(config),
        })
    }
}

/// TLS as the server (the test SCP).
pub(crate) fn accept(
    id: &ServerIdentity,
    mut sock: TcpStream,
    deadline: Instant,
    cancel: &CancelToken,
) -> Result<Stream> {
    let mut conn =
        ServerConnection::new(id.config.clone()).map_err(|e| Error::Tls(tls_error(&e)))?;
    handshake(&mut conn, &mut sock, deadline, cancel)?;
    Ok(Stream::Server(Box::new(StreamOwned::new(conn, sock))))
}
