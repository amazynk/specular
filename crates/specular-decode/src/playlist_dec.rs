//! Playlists (M3U/M3U8, PLS, XSPF; `specular-playlist`). A playlist has no picture: the app
//! opens it as the play queue. Read here it is a no-preview document whose facts (its title
//! and how many items it lists) the info panel and the gallery show.

use std::sync::Arc;

use specular_core::Source;
use specular_playlist::Kind;

use crate::office_dec::NoPreviewSession;
use crate::{DecodeError, Decoder, DocumentFacts, DocumentProperties, Format, Result, Session};

pub struct PlaylistDecoder;

/// The document kind key of a playlist ([`DocumentFacts::kind`]).
pub const PLAYLIST_KIND: &str = "playlist";

impl Decoder for PlaylistDecoder {
    fn format(&self) -> Format {
        Format::Playlist
    }
    fn probe(&self, head: &[u8]) -> bool {
        Kind::probe(head).is_some()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &specular_playlist::EXTENSIONS
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let kind = Kind::of_path(source.path())
            .or_else(|| Kind::probe(data))
            .ok_or_else(|| DecodeError::Malformed("not a playlist".into()))?;
        let base = source
            .path()
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let list = specular_playlist::read(data, kind, &base).map_err(|e| match e {
            specular_playlist::Error::TooLarge => DecodeError::Unsupported(format!("playlist {e}")),
            e => DecodeError::Malformed(format!("playlist {e}")),
        })?;
        let name = match kind {
            Kind::M3u => "M3U playlist",
            Kind::Pls => "PLS playlist",
            Kind::Xspf => "XSPF playlist",
        };
        let n = list.entries.len();
        let network = list.entries.iter().filter(|e| !e.playable()).count();
        let mut coding = format!("{name}, {n} item{}", if n == 1 { "" } else { "s" });
        if network > 0 {
            coding.push_str(&format!(", {network} on the network"));
        }
        let facts = DocumentFacts {
            kind: PLAYLIST_KIND,
            legacy: false,
            template: false,
            macros: false,
            preview: None,
            props: DocumentProperties {
                title: list.title,
                ..DocumentProperties::default()
            },
            items: Some(n as u64),
        };
        Ok(Arc::new(NoPreviewSession::with_coding(
            source,
            Format::Playlist,
            coding,
            facts,
        )))
    }
}
