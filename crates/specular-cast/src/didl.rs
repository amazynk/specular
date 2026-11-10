//! The metadata sent with the URI (`CurrentURIMetaData`): one DIDL-Lite item with the title,
//! its class and the resource with its protocol info, size and length. Many renderers show
//! the title, and some refuse a URI without metadata.

use crate::xml::escape;

/// What kind of item (UPnP ContentDirectory `upnp:class`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Video,
    Audio,
    Image,
}

impl Class {
    /// From a MIME type's major part.
    pub fn of(mime: &str) -> Class {
        match mime.split('/').next().unwrap_or("") {
            "audio" => Class::Audio,
            "image" => Class::Image,
            _ => Class::Video,
        }
    }

    fn upnp(self) -> &'static str {
        match self {
            Class::Video => "object.item.videoItem",
            Class::Audio => "object.item.audioItem.musicTrack",
            Class::Image => "object.item.imageItem.photo",
        }
    }
}

/// `H:MM:SS.mmm`, as `res@duration` writes it.
pub fn duration_text(secs: f64) -> String {
    let ms = (secs.max(0.0) * 1000.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

/// The DIDL-Lite document for `url`.
pub fn item(
    title: &str,
    class: Class,
    url: &str,
    protocol_info: &str,
    size: Option<u64>,
    duration: Option<f64>,
) -> String {
    item_with_subtitles(title, class, url, protocol_info, size, duration, None)
}

/// [`item`] with subtitles at `srt` (SubRip): as Samsung's `sec:CaptionInfoEx` (and
/// `sec:CaptionInfo`) and as a second resource of type `text/srt`, the two ways renderers
/// look for them.
pub fn item_with_subtitles(
    title: &str,
    class: Class,
    url: &str,
    protocol_info: &str,
    size: Option<u64>,
    duration: Option<f64>,
    srt: Option<&str>,
) -> String {
    let mut res = format!("<res protocolInfo=\"{}\"", escape(protocol_info));
    if let Some(s) = size {
        res.push_str(&format!(" size=\"{s}\""));
    }
    if let Some(d) = duration.filter(|d| d.is_finite() && *d > 0.0) {
        res.push_str(&format!(" duration=\"{}\"", duration_text(d)));
    }
    res.push_str(&format!(">{}</res>", escape(url)));
    let mut sec_ns = "";
    if let Some(s) = srt {
        let s = escape(s);
        sec_ns = " xmlns:sec=\"http://www.sec.co.kr/\"";
        res.push_str(&format!(
            "<res protocolInfo=\"http-get:*:text/srt:*\">{s}</res>\
<sec:CaptionInfoEx sec:type=\"srt\">{s}</sec:CaptionInfoEx><sec:CaptionInfo sec:type=\"srt\">{s}</sec:CaptionInfo>"
        ));
    }
    format!(
        "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\"{sec_ns}>\
<item id=\"0\" parentID=\"-1\" restricted=\"1\"><dc:title>{}</dc:title><upnp:class>{}</upnp:class>{res}</item></DIDL-Lite>",
        escape(title),
        class.upnp()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_an_item() {
        let d = item(
            "Tom & Jerry <1>",
            Class::Video,
            "http://10.0.0.2:5000/abc/movie.mp4",
            "http-get:*:video/mp4:*",
            Some(1234),
            Some(3723.5),
        );
        assert!(d.contains("<dc:title>Tom &amp; Jerry &lt;1&gt;</dc:title>"));
        assert!(d.contains("<upnp:class>object.item.videoItem</upnp:class>"));
        assert!(d.contains("size=\"1234\" duration=\"1:02:03.500\""));
        let doc = crate::xml::parse(&d).unwrap();
        let res = doc
            .descendants()
            .find(|n| n.tag_name().name() == "res")
            .unwrap();
        assert_eq!(res.text(), Some("http://10.0.0.2:5000/abc/movie.mp4"));
        assert_eq!(Class::of("audio/flac"), Class::Audio);
        let s = item_with_subtitles(
            "Film",
            Class::Video,
            "http://10.0.0.2:5000/abc/film.mp4",
            "http-get:*:video/mp4:*",
            None,
            None,
            Some("http://10.0.0.2:5000/abc/film.srt"),
        );
        let doc = crate::xml::parse(&s).unwrap();
        let caps: Vec<_> = doc
            .descendants()
            .filter(|n| n.tag_name().name() == "CaptionInfoEx")
            .map(|n| n.text().unwrap_or(""))
            .collect();
        assert_eq!(caps, ["http://10.0.0.2:5000/abc/film.srt"]);
        assert_eq!(
            doc.descendants()
                .filter(|n| n.tag_name().name() == "res")
                .count(),
            2
        );
    }
}
