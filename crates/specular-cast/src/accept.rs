//! Whether a renderer plays a type: its ConnectionManager's sink ProtocolInfo list (UPnP
//! ConnectionManager:1 §2.5.2, `protocol:network:contentFormat:additionalInfo`).

/// Spellings devices use for the same type, each group's first the usual one.
const ALIASES: &[&[&str]] = &[
    &["audio/mpeg", "audio/mp3", "audio/x-mp3", "audio/mpeg3"],
    &["audio/flac", "audio/x-flac"],
    &["audio/wav", "audio/x-wav", "audio/wave", "audio/l16"],
    &[
        "audio/mp4",
        "audio/x-m4a",
        "audio/m4a",
        "audio/aac",
        "audio/x-aac",
    ],
    &["audio/ogg", "application/ogg", "audio/x-ogg"],
    &["video/x-matroska", "video/x-mkv", "video/mkv"],
    &["video/x-msvideo", "video/avi", "video/x-avi"],
    &["video/mp4", "video/x-m4v", "video/m4v"],
    &["video/quicktime", "video/mov"],
    &["video/mpeg", "video/mpeg2", "video/x-mpeg"],
    &["video/mp2t", "video/vnd.dlna.mpeg-tts", "video/x-mpegts"],
    &["image/jpeg", "image/jpg"],
];

/// `a` and `b` name the same type.
fn same(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    ALIASES.iter().any(|g| {
        g.iter().any(|x| x.eq_ignore_ascii_case(a)) && g.iter().any(|x| x.eq_ignore_ascii_case(b))
    })
}

/// The DLNA flags we serve with: byte ranges (OP=01), streaming transfer, background and
/// connection stalling allowed.
pub const DLNA_FEATURES: &str =
    "DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000";

/// The sink list from GetProtocolInfo's `Sink` (comma-separated).
pub fn split_sink(sink: &str) -> Vec<String> {
    sink.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a renderer with sink list `sink` plays `mime` over HTTP: the type as the device
/// spells it, or `None`. An empty list (the device does not say) accepts anything; the
/// device will refuse at SetAVTransportURI if it cannot.
pub fn accepts(sink: &[String], mime: &str) -> Option<String> {
    if sink.is_empty() {
        return Some(mime.to_string());
    }
    let (major, _) = mime.split_once('/')?;
    for p in sink {
        let mut f = p.splitn(4, ':');
        let (Some(proto), Some(_net), Some(format)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        if proto != "http-get" && proto != "*" {
            continue;
        }
        let format = format.trim();
        if format == "*" || format == "*/*" {
            return Some(mime.to_string());
        }
        if let Some(m) = format.strip_suffix("/*")
            && m.eq_ignore_ascii_case(major)
        {
            return Some(mime.to_string());
        }
        // `audio/L16;rate=44100;channels=2`: the type before its parameters.
        let bare = format.split(';').next().unwrap_or(format).trim();
        if same(bare, mime) {
            return Some(bare.to_string());
        }
    }
    None
}

/// Whether a sink list names a subtitle type (SubRip, SAMI, or Samsung's caption format).
pub fn takes_subtitles(sink: &[String]) -> bool {
    sink.iter().any(|p| {
        let format = p.split(':').nth(2).unwrap_or("").to_ascii_lowercase();
        matches!(
            format.split(';').next().unwrap_or("").trim(),
            "text/srt" | "application/x-subrip" | "text/x-srt" | "smi/caption" | "text/smi"
        )
    })
}

/// The protocol info we offer the file with (DIDL-Lite `res@protocolInfo`).
pub fn protocol_info(mime: &str) -> String {
    format!("http-get:*:{mime}:{DLNA_FEATURES}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink(s: &str) -> Vec<String> {
        split_sink(s)
    }

    #[test]
    fn matches_types() {
        let tv = sink(
            "http-get:*:video/mp4:DLNA.ORG_PN=AVC_MP4_HP_HD_AAC,http-get:*:audio/mpeg:*,\
             http-get:*:video/x-mkv:*, http-get:*:audio/L16;rate=44100;channels=2:DLNA.ORG_PN=LPCM",
        );
        assert_eq!(accepts(&tv, "video/mp4").as_deref(), Some("video/mp4"));
        assert_eq!(
            accepts(&tv, "video/x-matroska").as_deref(),
            Some("video/x-mkv")
        );
        assert_eq!(accepts(&tv, "audio/mp3").as_deref(), Some("audio/mpeg"));
        assert_eq!(accepts(&tv, "audio/wav").as_deref(), Some("audio/L16"));
        assert_eq!(accepts(&tv, "video/webm"), None);
        assert_eq!(accepts(&tv, "audio/flac"), None);
    }

    #[test]
    fn wildcards_and_unknown_lists() {
        assert!(accepts(&sink("http-get:*:*:*"), "video/webm").is_some());
        assert!(accepts(&sink("http-get:*:audio/*:*"), "audio/flac").is_some());
        assert!(accepts(&sink("http-get:*:audio/*:*"), "video/mp4").is_none());
        // Only RTSP: no.
        assert!(accepts(&sink("rtsp-rtp-udp:*:video/mp4:*"), "video/mp4").is_none());
        assert!(accepts(&[], "video/mp4").is_some());
    }

    #[test]
    fn offers_ranges() {
        assert_eq!(
            protocol_info("video/mp4"),
            "http-get:*:video/mp4:DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000"
        );
    }
}
