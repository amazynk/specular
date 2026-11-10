//! The Local AI downloader's `https://` transport: [`specular_remote`]'s client (ureq +
//! rustls, the system trust store, proxies from the environment) behind
//! [`specular_ai::transport::Transport`].
//!
//! The installer keeps the partial file and asks for an offset; the request carries `Range`
//! and, when an earlier session saw the file's ETag or Last-Modified, `If-Range`, so a file
//! changed on the server comes back whole instead of being spliced (the installer's sha256
//! check stays the final word either way). Validators are remembered in the cache folder.
//! A dropped connection or a stalled body reconnects from where it stopped. Cancelling is
//! the installer's: it checks between reads, and a read waits at most the body timeout.

use std::io;
use std::path::PathBuf;

use specular_ai::transport::{Body, Transport, Transports};
use specular_core::net::What;
use specular_remote::{CancelToken, Client, ClientConfig, Request, Resumer};

pub(crate) struct HttpsTransport {
    resumer: Resumer,
    /// Also serve `http://` (loopback tests only; models come over HTTPS).
    plain_http: bool,
}

impl HttpsTransport {
    pub(crate) fn new(client: Client, validators: Option<PathBuf>) -> HttpsTransport {
        HttpsTransport {
            resumer: Resumer::new(client, validators),
            plain_http: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn allowing_http(mut self) -> HttpsTransport {
        self.plain_http = true;
        self
    }
}

fn scheme_is(url: &str, scheme: &str) -> bool {
    url.get(..scheme.len())
        .is_some_and(|s| s.eq_ignore_ascii_case(scheme))
}

impl Transport for HttpsTransport {
    fn serves(&self, url: &str) -> bool {
        scheme_is(url, "https://") || (self.plain_http && scheme_is(url, "http://"))
    }

    fn open(&self, url: &str, offset: u64) -> io::Result<Body> {
        self.open_named(url, offset, "")
    }

    /// Shown in "Downloads" as the model `name` (else the file's name in the URL).
    fn open_named(&self, url: &str, offset: u64, name: &str) -> io::Result<Body> {
        let name = if name.is_empty() {
            file_name(url)
        } else {
            name.to_owned()
        };
        let o = self
            .resumer
            .open(
                &Request::get(url).activity(What::Model(name)),
                offset,
                &CancelToken::new(),
            )
            .map_err(|e| match e {
                specular_remote::Error::Status(404 | 410) => {
                    io::Error::new(io::ErrorKind::NotFound, e.to_string())
                }
                specular_remote::Error::Cancelled => specular_ai::transport::stopped(),
                specular_remote::Error::Io(e) => e,
                e => io::Error::other(e.to_string()),
            })?;
        Ok(Body {
            resumed: o.resumed(offset),
            total: o.total,
            reader: Box::new(Stoppable(o.body)),
        })
    }
}

/// A body whose Stop in "Downloads" reads as the installer's cancel, not a failure.
struct Stoppable<R>(R);

impl<R: io::Read> io::Read for Stoppable<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf).map_err(|e| {
            let cancelled = e
                .get_ref()
                .and_then(|i| i.downcast_ref::<specular_remote::Error>())
                .is_some_and(|r| matches!(r, specular_remote::Error::Cancelled));
            if cancelled {
                specular_ai::transport::stopped()
            } else {
                e
            }
        })
    }
}

/// The last part of the URL's path (no query: a signed URL's signature stays out).
fn file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or("");
    path.rsplit('/').next().unwrap_or("").to_owned()
}

/// The transports the app's pack downloads with: `https://` through specular-remote, then
/// `file://`.
pub(super) fn transports() -> Transports {
    let cache = specular_settings::Locations::detect().cache_dir;
    match Client::new(ClientConfig::default()) {
        Ok(c) => Transports::default().with(Box::new(HttpsTransport::new(
            c,
            Some(cache.join("downloads").join("validators")),
        ))),
        Err(e) => {
            specular_log::warn!(target: specular_log::JOBS, "ai: no HTTPS client: {e}");
            Transports::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use specular_ai::transport::{Transport, Transports};
    use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
    use specular_remote::{Client, ClientConfig};

    use super::HttpsTransport;

    fn scratch() -> PathBuf {
        let d = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/ai-https"
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Loopback only: the test server speaks plain HTTP on 127.0.0.1.
    #[test]
    fn resumes_reconnects_and_restarts_a_changed_file() {
        let body: Vec<u8> = (0..900_000u32).map(|i| (i * 13 + i / 7) as u8).collect();
        let b = body.clone();
        let drop_once = Arc::new(AtomicBool::new(true));
        let d2 = drop_once.clone();
        let srv = TestServer::start(ServerOptions::default(), move |r| {
            let res = TestResponse::ranged(r, &b, "\"m1\"");
            if d2.swap(false, Ordering::SeqCst) {
                res.cut(250_000, Duration::ZERO)
            } else {
                res
            }
        })
        .unwrap();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let t = Transports::default().with(Box::new(
            HttpsTransport::new(client, Some(scratch())).allowing_http(),
        ));
        let url = srv.url("/clip.onnx");
        assert!(t.serves(&url) && t.serves("HTTPS://x/y") && t.serves("file:///m"));

        // Whole file, through one dropped connection.
        let mut o = t.open(&url, 0).unwrap();
        assert!(!o.resumed);
        assert_eq!(o.total, Some(900_000));
        let mut got = Vec::new();
        o.reader.read_to_end(&mut got).unwrap();
        assert_eq!(got, body);

        // From an offset: Range + If-Range with the ETag seen above.
        let mut o = t.open(&url, 600_000).unwrap();
        assert!(o.resumed);
        assert_eq!(o.total, Some(900_000));
        let mut rest = Vec::new();
        o.reader.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, body[600_000..]);
        let last = srv.log().pop().unwrap();
        assert_eq!(last.header("range"), Some("bytes=600000-"));
        assert_eq!(last.header("if-range"), Some("\"m1\""));

        // Errors come back as io errors the installer logs and moves past.
        let srv404 =
            TestServer::start(ServerOptions::default(), |_| TestResponse::status(404)).unwrap();
        let e = t.open(&srv404.url("/m"), 0).err().unwrap();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);

        // Stop in "Downloads" reads as the installer's cancel, not as a failed download.
        let mut o = t.open_named(&url, 0, "stop-me").unwrap();
        let mut buf = vec![0u8; 4096];
        assert!(o.reader.read(&mut buf).unwrap() > 0);
        let reg = specular_core::net::global();
        let id = reg
            .snapshot()
            .into_iter()
            .find(|a| a.label.what == specular_core::net::What::Model("stop-me".into()))
            .expect("the model download is listed")
            .id;
        assert!(reg.cancel(id));
        let e = loop {
            match o.reader.read(&mut buf) {
                Ok(0) => panic!("the body ended instead of stopping"),
                Ok(_) => {}
                Err(e) => break e,
            }
        };
        assert!(specular_ai::transport::is_stopped(&e), "{e}");

        // Without the test switch plain HTTP is not served.
        let strict = HttpsTransport::new(Client::new(ClientConfig::for_tests()).unwrap(), None);
        assert!(!strict.serves(&url));
    }
}
