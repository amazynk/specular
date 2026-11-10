//! SAMP (IVOA Simple Application Messaging Protocol 1.3, Standard Profile): how DS9,
//! TOPCAT, Aladin and astropy pass images, tables and sky positions to each other.
//!
//! - [`lockfile`]: finding the hub (`SAMP_HUB`, else `~/.samp`), on this computer only;
//! - [`xmlrpc`]: the XML-RPC subset SAMP uses (own code, quick-xml for reading);
//! - [`http`]: HTTP/1.1 between processes on this computer (loopback addresses only, never
//!   resolved through DNS);
//! - [`client`]: register, declare metadata and subscriptions, receive `image.load.fits`,
//!   `table.load.votable`, `coord.pointAt.sky`, `table.highlight.row` and
//!   `table.select.rowList`, send images, positions and highlighted rows;
//! - [`testing`]: a small hub for tests.
//!
//! Nothing here leaves the computer: every address is checked to be a loopback one before it
//! is contacted, and message URLs are only followed to local files or local servers.

pub mod client;
pub mod http;
pub mod lockfile;
pub mod testing;
pub mod xmlrpc;

pub use client::{Client, Incoming, Metadata, Source};
pub use lockfile::{HubInfo, NoHub};
pub use xmlrpc::Value;
