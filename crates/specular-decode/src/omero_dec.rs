//! OMERO images: a small `.omero` descriptor (server, image id, rendering; never a secret)
//! names an image on an OMERO.web server, whose rendered tiles the `specular-omero` crate
//! fetches. This crate does no networking: the application registers the opener
//! ([`set_omero_opener`]) and without one a descriptor is unsupported.

use std::sync::Arc;

use parking_lot::RwLock;
use specular_core::Source;

use crate::{DecodeError, Decoder, Format, Result, Session};

/// Opens an `.omero` descriptor as a session.
pub type OmeroOpener = Arc<dyn Fn(Arc<Source>) -> Result<Arc<dyn Session>> + Send + Sync>;

static OPENER: RwLock<Option<OmeroOpener>> = RwLock::new(None);

/// The first bytes of every descriptor (after optional white space).
pub const OMERO_MAGIC: &[u8] = b"{\"specular-omero\":";

/// Registers (or with `None` removes) the opener of `.omero` descriptors.
pub fn set_omero_opener(opener: Option<OmeroOpener>) {
    *OPENER.write() = opener;
}

pub struct OmeroDecoder;

impl Decoder for OmeroDecoder {
    fn format(&self) -> Format {
        Format::Omero
    }
    fn probe(&self, head: &[u8]) -> bool {
        let start = head
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(head.len());
        head[start..].starts_with(OMERO_MAGIC)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["omero"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let opener = OPENER.read().clone();
        match opener {
            Some(open) => open(source),
            None => Err(DecodeError::Unsupported("OMERO".into())),
        }
    }
}

/// An OMERO image for the info panel: where it lives on the server and what it holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OmeroFacts {
    /// The server's address ("https://idr.openmicroscopy.org").
    pub server: String,
    /// Signed in as (`None`: public browsing).
    pub user: Option<String>,
    pub image_id: u64,
    pub name: String,
    pub project: Option<String>,
    pub dataset: Option<String>,
    pub screen: Option<String>,
    pub plate: Option<String>,
    /// The well ("B4") and field.
    pub well: Option<String>,
    pub owner: Option<String>,
    pub group: Option<String>,
    /// Acquisition date as ISO 8601 ("2016-04-01 12:30:00").
    pub acquired: Option<String>,
    /// X, Y, Z, C, T.
    pub size: [u64; 5],
    /// Pixel type ("uint16").
    pub pixels_type: String,
    /// Physical pixel size along X, Y, Z: value and unit symbol.
    pub physical: [Option<(f64, String)>; 3],
    pub channels: Vec<OmeroChannel>,
    /// Tile width and height and pyramid levels of a tiled image.
    pub tiles: Option<(u32, u32, u32)>,
    /// The plane shown (Z, T), counted from 0.
    pub plane: (u32, u32),
    /// How pixels are fetched: rendered "JPEG" tiles or a lossless "PNG".
    pub encoding: &'static str,
}

/// A channel of an OMERO image as rendered.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OmeroChannel {
    pub name: String,
    pub colour: [u8; 3],
    /// Rendering window (start, end) in pixel values.
    pub window: (f64, f64),
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_the_magic_only() {
        assert!(OmeroDecoder.probe(b"{\"specular-omero\":1}"));
        assert!(OmeroDecoder.probe(b"\n  {\"specular-omero\":1}"));
        assert!(!OmeroDecoder.probe(b"{\"other\":1}"));
        assert!(!OmeroDecoder.probe(b""));
    }
}
