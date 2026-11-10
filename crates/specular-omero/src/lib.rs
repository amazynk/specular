//! Read-only OMERO.web client (OMERO servers and the Image Data Resource).
//!
//! - [`api`]: one server: API discovery, sign-in with a CSRF token and session cookies, and
//!   requests that sign in again once when the session ends (the password is kept in memory
//!   only).
//! - [`browse`]: Projects → Datasets → Images and Screens → Plates → Wells → Images, paged.
//! - [`image`]: `webgateway/imgData`: sizes, channels, windows, pyramid levels.
//! - [`render`]: channel strings and `render_image_region` / `render_image` /
//!   `render_thumbnail` URLs.
//! - [`tiles`]: fetching (through the shared disk cache), decoding with our own JPEG and PNG
//!   decoders and assembling tiles, several requests at a time, cancellable.
//! - [`descriptor`]: the `.omero` file naming an image (no secrets).
//! - [`session`]: an image as a decoder session (pyramid levels as scales, Z × T planes as
//!   pages); [`registry::install`] registers it with `specular-decode`.
//! - [`servers`]: the user's server list, with the IDR preset.
//! - [`fake`]: a synthetic in-process OMERO.web for tests and screenshots.
//!
//! Nothing here contacts a server by itself: every request follows a user action on a server
//! the user added (or an image the user opened).

pub mod api;
pub mod browse;
pub mod descriptor;
#[doc(hidden)]
pub mod fake;
pub mod image;
pub mod registry;
pub mod render;
pub mod servers;
pub mod session;
pub mod tiles;

pub use api::{EventContext, Omero, OmeroError, Result};
pub use browse::{ImageBrief, Kind, Listing, Node, Page, WellInfo};
pub use descriptor::{Context, Descriptor};
pub use image::{Channel, ImgData};
pub use render::{ChannelRender, Encoding, Render};
pub use servers::{IDR_URL, Server, ServerList};
pub use session::OmeroSession;
pub use tiles::Rgba;
