//! Remote sources: files on HTTP(S) servers read like local ones.
//!
//! - [`http`]: blocking client (ureq + rustls, keep-alive pool) with ranges, timeouts,
//!   cancellation through [`CancelToken`](specular_core::CancelToken), retries with backoff,
//!   and resumable downloads.
//! - [`cache`]: bounded on-disk cache under the app's cache dir, LRU eviction, pinning.
//! - [`lru`]: in-memory LRU bounded by bytes.
//! - [`resume`]: downloads as a `Read` from an offset that reconnect with `Range` + `If-Range`
//!   (the Local AI installer's HTTPS transport).
//! - [`remote_file`]: a remote file as a [`Source`](specular_core::Source) or by ranges.
//! - [`auth`]: Basic/Bearer credentials, kept in memory (no plaintext on disk).
//! - [`s3`]: S3 addresses, SigV4 signing and configured AWS credentials.
//! - [`keystore`]: a Zarr hierarchy over HTTP(S) or S3 as a
//!   [`KeyStore`](specular_core::keys::KeyStore), fetched lazily, chunks several at a time.
//! - [`testing`]: an in-process HTTP server for tests (loopback only).
//!
//! Nothing here contacts a server on its own: every request is made by a caller acting on a
//! server the user added. DICOMweb (`specular-dicomweb`) is the first user; S3/COG/Zarr
//! sources (Stage 5) are meant to reuse [`RemoteFile`] and [`DiskCache`].

pub mod auth;
pub mod cache;
pub mod http;
pub mod keystore;
pub mod lru;
pub mod remote_file;
pub mod resume;
pub mod s3;
pub mod testing;
pub mod zarr_fixture;

pub use auth::{Auth, AuthKind, CredentialStore, Secret, SessionCredentials};
pub use cache::{CachePin, CacheStats, DiskCache};
pub use http::{Client, ClientConfig, Error, Head, Headers, Progress, Request, Response};
pub use keystore::{RemoteStore, StoreOptions};
pub use lru::MemLru;
pub use remote_file::RemoteFile;
pub use resume::{BodyStream, Opened, Resumer};
pub use specular_core::CancelToken;
