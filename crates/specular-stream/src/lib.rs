//! Network streams for the player (DESIGN: "Open network stream"): what an address is, and a
//! worker that fetches it into files in the cache the existing demuxers read.
//!
//! - [`addr`]: joining relative references, telling an address and an answer apart.
//! - [`hls`]: HLS master and media playlists; [`aes128`] decrypts `AES-128` segments.
//! - [`dash`]: DASH manifests (templates, timelines, lists, `sidx` indexes; static and live).
//! - [`fmp4`]: fragmented MP4 segments rewritten as a plain MP4.
//! - [`tsmux`]: an HLS sound rendition added to MPEG-TS pieces.
//! - [`icy`]: internet radio's in-band titles; [`frames`] cuts its sound into pieces.
//! - [`ytdlp`]: a user-installed yt-dlp, asked for a page's media address.
//! - [`stream`]: the worker: a file fetched whole, or pieces of a few seconds each (radio,
//!   HLS, DASH) which the player plays one after another without a gap.
//!
//! Nothing here runs unless the user opens an address. Every request goes through the one
//! HTTP client (`specular-remote`), which the "No network" switch stops and the download
//! indicator shows ("Stream from <host>"). Streams protected by a DRM system are refused.

pub mod addr;
pub mod aes128;
pub mod dash;
pub mod fmp4;
pub mod frames;
pub mod hls;
pub mod icy;
pub mod stream;
pub mod tsmux;
pub mod ytdlp;

pub use stream::{Chunk, Failure, Options, Status, Stream, StreamKind};
