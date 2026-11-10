//! XSPF: `<playlist xmlns="http://xspf.org/ns/0/">` with a `<trackList>` of `<track>`s, each
//! with `<location>` (a URI; relative ones from the playlist's folder), `<title>`,
//! `<creator>` and `<duration>` (milliseconds).

use std::path::Path;

use crate::uri::{file_uri, has_scheme, percent_decode, percent_encode};
use crate::{Entry, Error, Location, Playlist, join_path, path_from_file_uri, relative_to};

const NS: &str = "http://xspf.org/ns/0/";

pub(crate) fn read(text: &str, base: &Path) -> Result<Playlist, Error> {
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opts)
        .map_err(|e| Error::Malformed(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "playlist" {
        return Err(Error::NotPlaylist);
    }
    let mut out = Playlist {
        title: text_of(child(root, "title")),
        entries: Vec::new(),
    };
    let Some(list) = child(root, "trackList") else {
        return Ok(out);
    };
    for track in list
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "track")
    {
        let Some(loc) = text_of(child(track, "location")) else {
            continue;
        };
        let Some(location) = locate_uri(&loc, base) else {
            continue;
        };
        let title = text_of(child(track, "title"));
        let creator = text_of(child(track, "creator"));
        let title = match (creator, title) {
            (Some(c), Some(t)) => Some(format!("{c} – {t}")),
            (None, t) => t,
            (Some(_), None) => None,
        };
        let duration = text_of(child(track, "duration"))
            .and_then(|d| d.parse::<f64>().ok())
            .filter(|d| d.is_finite() && *d >= 0.0)
            .map(|ms| ms / 1000.0);
        out.entries.push(Entry {
            location,
            title,
            duration,
        });
    }
    Ok(out)
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name && in_ns(c))
}

fn text_of(n: Option<roxmltree::Node<'_, '_>>) -> Option<String> {
    n.and_then(|n| n.text())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// The XSPF namespace, or none (some writers leave it out).
fn in_ns(n: &roxmltree::Node<'_, '_>) -> bool {
    n.tag_name().namespace().is_none_or(|ns| ns == NS)
}

/// A `<location>`: a `file:` URI, another URI, or a relative reference from `base`.
fn locate_uri(s: &str, base: &Path) -> Option<Location> {
    if let Some(p) = path_from_file_uri(s) {
        return Some(Location::File(p));
    }
    if has_scheme(s) {
        return Some(Location::Url(s.to_owned()));
    }
    let raw = String::from_utf8_lossy(&percent_decode(s)).into_owned();
    Some(Location::File(join_path(&raw, base)))
}

pub(crate) fn write(title: Option<&str>, entries: &[Entry], dir: &Path) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str(&format!("<playlist version=\"1\" xmlns=\"{NS}\">\n"));
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        s.push_str(&format!("  <title>{}</title>\n", escape(t)));
    }
    s.push_str("  <trackList>\n");
    for e in entries {
        s.push_str("    <track>\n");
        let loc = match &e.location {
            Location::File(p) => match relative_to(p, dir) {
                Some(rel) => percent_encode(rel.as_bytes()),
                None => file_uri(p),
            },
            Location::Url(u) => u.clone(),
        };
        s.push_str(&format!("      <location>{}</location>\n", escape(&loc)));
        if let Some(t) = &e.title {
            s.push_str(&format!("      <title>{}</title>\n", escape(t)));
        }
        if let Some(d) = e.duration {
            s.push_str(&format!(
                "      <duration>{}</duration>\n",
                (d * 1000.0).round() as u64
            ));
        }
        s.push_str("    </track>\n");
    }
    s.push_str("  </trackList>\n</playlist>\n");
    s
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            // Characters XML 1.0 cannot hold.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}
