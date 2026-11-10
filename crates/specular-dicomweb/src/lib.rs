//! DICOMweb client (PS3.18): QIDO-RS search, WADO-RS retrieval and progressive series loading.
//!
//! - [`DicomWeb`]: one server (base URL + credentials) over a shared
//!   [`specular_remote::Client`].
//! - [`qido`]: studies / series / instances search with paging.
//! - [`find`]: the same study and series searches against a DIMSE server (C-FIND).
//! - [`wado`]: instances (multipart/related application/dicom), frames, bulk data, metadata
//!   JSON and rendered previews; the `Accept` header lists the transfer syntaxes the in-tree
//!   decoders read.
//! - [`json`]: the DICOM JSON model, mapped into [`specular_dicom::Instance`] so series
//!   assembly is shared with local files.
//! - [`progressive`]: metadata first, then the middle slice, then the rest nearest the current
//!   slice first, with bounded parallel requests, into the disk cache.
//! - [`retrieve`]: a DIMSE server's series by C-GET or C-MOVE into the same cache, opened
//!   as a DICOMweb series is.
//! - [`servers`]: the user's server list (no secrets on disk).
//! - [`fake`]: a synthetic in-process server for tests and screenshots.
//!
//! Nothing here contacts a server by itself: every request follows a user action on a server
//! the user added.

pub mod accept;
#[doc(hidden)]
pub mod fake;
pub mod find;
pub mod json;
pub mod multipart;
pub mod progressive;
pub mod qido;
pub mod retrieve;
pub mod servers;
pub mod stow;
pub mod wado;

use specular_remote::{Auth, CancelToken, Client, Request, Response};

pub use json::{JsonDataSet, JsonError};
pub use multipart::{MultipartError, Part};
pub use progressive::{LoadEvent, LoadOptions, SeriesLoad};
pub use qido::{InstanceRow, Page, Patient, SeriesRow, StudyQuery, StudyRow};
pub use servers::{Kind as ServerKind, Server, ServerList};
pub use stow::{Outcome as StoreOutcome, StoreResponse};

/// What went wrong talking to a DICOMweb server.
#[derive(Debug)]
pub enum Error {
    Http(specular_remote::Error),
    Json(JsonError),
    Multipart(MultipartError),
    /// The response had no DICOM part.
    NoDicom,
    /// A UID with characters a UID cannot have.
    BadUid,
    Io(std::io::Error),
    /// A DIMSE server's C-GET or C-MOVE opened nothing.
    Retrieve(retrieve::Failure),
}

impl Error {
    pub fn is_offline(&self) -> bool {
        matches!(self, Error::Http(e) if e.is_offline())
    }
    /// The no-network switch refused the request.
    pub fn is_network_off(&self) -> bool {
        matches!(self, Error::Http(e) if e.is_network_off())
    }
    pub fn is_auth(&self) -> bool {
        matches!(self, Error::Http(e) if e.is_auth())
    }
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Error::Http(specular_remote::Error::Cancelled))
    }
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Http(specular_remote::Error::Status(s)) => Some(*s),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Http(e) => e.fmt(f),
            Error::Json(e) => e.fmt(f),
            Error::Multipart(e) => e.fmt(f),
            Error::NoDicom => f.write_str("the response holds no DICOM object"),
            Error::BadUid => f.write_str("invalid UID"),
            Error::Io(e) => e.fmt(f),
            Error::Retrieve(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<specular_remote::Error> for Error {
    fn from(e: specular_remote::Error) -> Self {
        Error::Http(e)
    }
}
impl From<JsonError> for Error {
    fn from(e: JsonError) -> Self {
        Error::Json(e)
    }
}
impl From<MultipartError> for Error {
    fn from(e: MultipartError) -> Self {
        Error::Multipart(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// One DICOMweb service: a base URL (the "service root", e.g.
/// `https://pacs.example/dicom-web`) and its credentials.
#[derive(Clone, Debug)]
pub struct DicomWeb {
    client: Client,
    base: String,
    auth: Auth,
}

impl DicomWeb {
    pub fn new(client: Client, base_url: &str, auth: Auth) -> DicomWeb {
        DicomWeb {
            client,
            base: base_url.trim().trim_end_matches('/').to_string(),
            auth,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The same service, its requests shown in the "Downloads" indicator as `what`.
    pub fn tracked(mut self, what: specular_core::net::What) -> DicomWeb {
        self.client = self.client.tracked(what);
        self
    }

    /// `base/seg/seg?k=v&…` with every segment and value percent-encoded.
    pub fn url(&self, segments: &[&str], query: &[(&str, String)]) -> String {
        let mut u = self.base.clone();
        for s in segments {
            u.push('/');
            u.push_str(&encode(s));
        }
        for (i, (k, v)) in query.iter().enumerate() {
            u.push(if i == 0 { '?' } else { '&' });
            u.push_str(&encode(k));
            u.push('=');
            u.push_str(&encode(v));
        }
        u
    }

    pub(crate) fn request(&self, url: String) -> Request {
        Request::get(url).auth(&self.auth)
    }

    pub(crate) fn get(&self, req: &Request, cancel: &CancelToken) -> Result<Response> {
        Ok(self.client.get(req, cancel)?)
    }

    /// A cheap request that proves the server answers and accepts the credentials.
    pub fn check(&self, cancel: &CancelToken) -> Result<()> {
        let q = StudyQuery {
            limit: 1,
            ..StudyQuery::default()
        };
        self.search_studies(&q, cancel).map(|_| ())
    }
}

/// A UID path segment (`1.2.3`); anything else is refused rather than sent.
pub(crate) fn uid(s: &str) -> Result<&str> {
    let s = s.trim_end_matches(['\0', ' ']);
    if !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        Ok(s)
    } else {
        Err(Error::BadUid)
    }
}

/// Percent-encodes everything but RFC 3986 unreserved characters (and `*`, `,`, which DICOMweb
/// uses for wildcards and lists).
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'*' | b',') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_encoded_and_uids_checked() {
        let c = specular_remote::Client::new(specular_remote::ClientConfig::for_tests()).unwrap();
        let d = DicomWeb::new(c, "http://h/dicom-web/ ", Auth::None);
        assert_eq!(
            d.url(&["studies"], &[("PatientName", "Müller^A*".into())]),
            "http://h/dicom-web/studies?PatientName=M%C3%BCller%5EA*"
        );
        assert!(uid("1.2.3").is_ok());
        assert!(uid("1.2/../x").is_err());
        assert!(uid("").is_err());
    }
}
