//! A minimal client for Specular's remote-control socket: the IPC protocol of `docs/ipc.md`
//! (version 3, whose viewers also answer versions 1 and 2), events included.
//!
//! The viewer and this crate share the request types ([`protocol::Request`]) and their
//! encoding ([`protocol::request_line`]); the viewer parses requests itself. Standard library
//! and `serde_json` only.
//!
//! ```no_run
//! let mut c = specular_client::Client::connect_default()?;
//! c.call(&specular_client::Request::Navigate(specular_client::Nav::Next))?;
//! let png = c.screenshot()?;
//! # Ok::<(), specular_client::Error>(())
//! ```
#![cfg_attr(windows, feature(windows_unix_domain_sockets))]

pub mod base64;
mod client;
pub mod protocol;

pub use client::{Client, Error, Events, default_socket};
pub use protocol::{
    ArraySource, Channel, Flag, Lang, Levels, MarkSet, Nav, Request, RoiFormat, Shape, ShapeKind,
    ViewSet, Window, Zoom,
};
