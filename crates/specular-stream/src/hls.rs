//! HLS playlists (RFC 8216): a master playlist's variants and renditions, a media playlist's
//! segments with their byte ranges, keys and initialisation sections.
//!
//! Only what playing needs is read; unknown tags are skipped. Keys are kept as written: the
//! fetcher decides what it can play ([`Media::refusal`]): clear segments and AES-128, never
//! SAMPLE-AES or a key system (FairPlay, Widevine, PlayReady).

use crate::addr::resolve;

/// A playlist.
#[derive(Clone, Debug, PartialEq)]
pub enum Playlist {
    Master(Master),
    Media(Media),
}

/// The variants of a stream at different qualities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Master {
    pub variants: Vec<Variant>,
    /// `EXT-X-MEDIA` renditions (alternative sound, subtitles).
    pub renditions: Vec<Rendition>,
    /// `EXT-X-SESSION-KEY`s: a master announcing a key system is refused up front.
    pub session_keys: Vec<Key>,
}

/// One quality (`EXT-X-STREAM-INF`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Variant {
    pub uri: String,
    /// Peak bits per second.
    pub bandwidth: u64,
    pub average_bandwidth: Option<u64>,
    pub resolution: Option<(u32, u32)>,
    pub codecs: Option<String>,
    pub frame_rate: Option<f64>,
    /// The `AUDIO` group whose rendition plays with it.
    pub audio: Option<String>,
}

/// An `EXT-X-MEDIA` rendition.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rendition {
    /// `AUDIO`, `VIDEO`, `SUBTITLES` or `CLOSED-CAPTIONS`.
    pub kind: String,
    pub group: String,
    pub name: String,
    pub language: Option<String>,
    /// `None`: the rendition is in the variant's own segments.
    pub uri: Option<String>,
    pub default: bool,
}

/// How a segment is encrypted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Method {
    #[default]
    None,
    Aes128,
    SampleAes,
    SampleAesCtr,
    Other(String),
}

/// An `EXT-X-KEY` (or `EXT-X-SESSION-KEY`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Key {
    pub method: Method,
    pub uri: Option<String>,
    pub iv: Option<[u8; 16]>,
    /// `KEYFORMAT`; absent means `identity` (a plain key file).
    pub keyformat: Option<String>,
}

impl Key {
    /// A key system (DRM) rather than a key file.
    pub fn is_drm(&self) -> bool {
        if self.method == Method::None {
            return false;
        }
        self.keyformat
            .as_deref()
            .is_some_and(|f| !f.eq_ignore_ascii_case("identity"))
            || self.uri.as_deref().is_some_and(|u| {
                let u = u.to_ascii_lowercase();
                u.starts_with("skd://") || u.starts_with("data:text/plain;base64")
            })
    }
}

/// An `EXT-X-MAP` (the initialisation section of fragmented MP4 segments).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    pub uri: String,
    /// `(offset, length)`.
    pub range: Option<(u64, u64)>,
}

/// One segment.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub uri: String,
    /// Seconds.
    pub duration: f64,
    pub title: Option<String>,
    /// `(offset, length)` in the resource (`EXT-X-BYTERANGE`).
    pub range: Option<(u64, u64)>,
    pub key: Key,
    pub map: Option<Map>,
    /// `EXT-X-DISCONTINUITY` before it: the timestamps may start over.
    pub discontinuity: bool,
    /// Its media sequence number.
    pub sequence: u64,
}

/// The segments of one variant.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Media {
    /// `EXT-X-TARGETDURATION`, seconds.
    pub target_duration: f64,
    pub media_sequence: u64,
    pub segments: Vec<Segment>,
    /// `EXT-X-ENDLIST`: no segments will be added.
    pub ended: bool,
    /// `EXT-X-PLAYLIST-TYPE`: `VOD` or `EVENT`.
    pub playlist_type: Option<String>,
}

impl Media {
    /// Whether segments are still being added (a live stream).
    pub fn live(&self) -> bool {
        !self.ended && self.playlist_type.as_deref() != Some("VOD")
    }

    /// The length of all segments, seconds.
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|s| s.duration).sum()
    }

    /// Why it can't be played, if it can't.
    pub fn refusal(&self) -> Option<Refusal> {
        for s in &self.segments {
            if s.key.is_drm() {
                return Some(Refusal::Drm);
            }
            match &s.key.method {
                Method::None | Method::Aes128 => {}
                Method::SampleAes | Method::SampleAesCtr => return Some(Refusal::SampleAes),
                Method::Other(_) => return Some(Refusal::UnknownKey),
            }
        }
        None
    }
}

/// Why a stream is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A key system (FairPlay, Widevine, PlayReady): protected, never played.
    Drm,
    /// SAMPLE-AES: each sample encrypted inside the stream (not read).
    SampleAes,
    /// An encryption method HLS doesn't define.
    UnknownKey,
}

/// What went wrong reading a playlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not an HLS playlist (no `#EXTM3U`).
    NotHls,
    /// A master playlist without variants, or a media playlist without segments.
    Empty,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::NotHls => "not an HLS playlist",
            Error::Empty => "the playlist lists nothing to play",
        })
    }
}

/// The `KEY=VALUE` pairs of an attribute list (values unquoted).
pub fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i] == b',' || b[i] == b' ') {
            i += 1;
        }
        let k0 = i;
        while i < b.len() && b[i] != b'=' && b[i] != b',' {
            i += 1;
        }
        let key = s[k0..i].trim().to_ascii_uppercase();
        if i >= b.len() || b[i] == b',' {
            if !key.is_empty() {
                out.push((key, String::new()));
            }
            continue;
        }
        i += 1;
        let value = if i < b.len() && b[i] == b'"' {
            i += 1;
            let v0 = i;
            while i < b.len() && b[i] != b'"' {
                i += 1;
            }
            let v = s[v0..i].to_owned();
            i = (i + 1).min(b.len());
            while i < b.len() && b[i] != b',' {
                i += 1;
            }
            v
        } else {
            let v0 = i;
            while i < b.len() && b[i] != b',' {
                i += 1;
            }
            s[v0..i].trim().to_owned()
        };
        out.push((key, value));
    }
    out
}

fn attr<'a>(a: &'a [(String, String)], k: &str) -> Option<&'a str> {
    a.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

fn hex16(s: &str) -> Option<[u8; 16]> {
    let h = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
    if h.is_empty() || h.len() > 32 {
        return None;
    }
    let padded = format!("{h:0>32}");
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&padded[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

fn key_of(a: &[(String, String)], base: &str) -> Key {
    let method = match attr(a, "METHOD").unwrap_or("NONE") {
        "NONE" => Method::None,
        "AES-128" => Method::Aes128,
        "SAMPLE-AES" => Method::SampleAes,
        "SAMPLE-AES-CTR" => Method::SampleAesCtr,
        m => Method::Other(m.to_owned()),
    };
    let raw_uri = attr(a, "URI");
    Key {
        uri: raw_uri.map(|u| {
            // Key-system addresses (`skd://…`, `data:`) are not relative references.
            if u.contains("://") || u.starts_with("data:") {
                u.to_owned()
            } else {
                resolve(base, u)
            }
        }),
        iv: attr(a, "IV").and_then(hex16),
        keyformat: attr(a, "KEYFORMAT").map(str::to_owned),
        method,
    }
}

/// `n[@o]`: `(length, offset)`.
fn byterange(s: &str) -> Option<(u64, Option<u64>)> {
    let (n, o) = match s.split_once('@') {
        Some((n, o)) => (n, Some(o.trim().parse().ok()?)),
        None => (s, None),
    };
    Some((n.trim().parse().ok()?, o))
}

/// Reads a playlist; relative addresses are resolved against `base` (the playlist's own
/// address, after redirects).
pub fn parse(text: &str, base: &str) -> Result<Playlist, Error> {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next().is_none_or(|l| !l.starts_with("#EXTM3U")) {
        return Err(Error::NotHls);
    }
    if text.contains("#EXT-X-STREAM-INF") {
        parse_master(text, base).map(Playlist::Master)
    } else {
        parse_media(text, base).map(Playlist::Media)
    }
}

fn parse_master(text: &str, base: &str) -> Result<Master, Error> {
    let mut m = Master::default();
    let mut pending: Option<Variant> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(rest) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let a = attributes(rest);
            pending = Some(Variant {
                uri: String::new(),
                bandwidth: attr(&a, "BANDWIDTH")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                average_bandwidth: attr(&a, "AVERAGE-BANDWIDTH").and_then(|v| v.parse().ok()),
                resolution: attr(&a, "RESOLUTION").and_then(|r| {
                    let (w, h) = r.split_once(['x', 'X'])?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                }),
                codecs: attr(&a, "CODECS").map(str::to_owned),
                frame_rate: attr(&a, "FRAME-RATE").and_then(|v| v.parse().ok()),
                audio: attr(&a, "AUDIO").map(str::to_owned),
            });
        } else if let Some(rest) = line.strip_prefix("#EXT-X-MEDIA:") {
            let a = attributes(rest);
            m.renditions.push(Rendition {
                kind: attr(&a, "TYPE").unwrap_or("").to_owned(),
                group: attr(&a, "GROUP-ID").unwrap_or("").to_owned(),
                name: attr(&a, "NAME").unwrap_or("").to_owned(),
                language: attr(&a, "LANGUAGE").map(str::to_owned),
                uri: attr(&a, "URI").map(|u| resolve(base, u)),
                default: attr(&a, "DEFAULT") == Some("YES"),
            });
        } else if let Some(rest) = line.strip_prefix("#EXT-X-SESSION-KEY:") {
            m.session_keys.push(key_of(&attributes(rest), base));
        } else if !line.starts_with('#')
            && let Some(mut v) = pending.take()
        {
            v.uri = resolve(base, line);
            m.variants.push(v);
        }
    }
    if m.variants.is_empty() {
        return Err(Error::Empty);
    }
    Ok(m)
}

fn parse_media(text: &str, base: &str) -> Result<Media, Error> {
    let mut m = Media::default();
    let mut key = Key::default();
    let mut map: Option<Map> = None;
    let mut duration: Option<(f64, Option<String>)> = None;
    let mut range: Option<(u64, Option<u64>)> = None;
    let mut disc = false;
    let mut seq: Option<u64> = None;
    // Where the last byte range of each resource ended (a range without an offset follows it).
    let mut last_end: Option<(String, u64)> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(v) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            m.target_duration = v.trim().parse().unwrap_or(0.0);
        } else if let Some(v) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            m.media_sequence = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("#EXT-X-PLAYLIST-TYPE:") {
            m.playlist_type = Some(v.trim().to_ascii_uppercase());
        } else if line.starts_with("#EXT-X-ENDLIST") {
            m.ended = true;
        } else if let Some(v) = line.strip_prefix("#EXTINF:") {
            let (d, t) = v.split_once(',').unwrap_or((v, ""));
            let t = t.trim();
            duration = Some((
                d.trim().parse().unwrap_or(0.0),
                (!t.is_empty()).then(|| t.to_owned()),
            ));
        } else if let Some(v) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            range = byterange(v);
        } else if line.starts_with("#EXT-X-DISCONTINUITY") && !line.contains("SEQUENCE") {
            disc = true;
        } else if let Some(v) = line.strip_prefix("#EXT-X-KEY:") {
            key = key_of(&attributes(v), base);
        } else if let Some(v) = line.strip_prefix("#EXT-X-MAP:") {
            let a = attributes(v);
            if let Some(u) = attr(&a, "URI") {
                map = Some(Map {
                    uri: resolve(base, u),
                    range: attr(&a, "BYTERANGE")
                        .and_then(byterange)
                        .map(|(n, o)| (o.unwrap_or(0), n)),
                });
            }
        } else if !line.starts_with('#') {
            let uri = resolve(base, line);
            let sequence = seq.map_or(m.media_sequence, |s| s + 1);
            seq = Some(sequence);
            let r = range.take().map(|(n, o)| {
                let off = o.unwrap_or_else(|| match &last_end {
                    Some((u, e)) if *u == uri => *e,
                    _ => 0,
                });
                (off, n)
            });
            if let Some((o, n)) = r {
                last_end = Some((uri.clone(), o + n));
            }
            let (d, title) = duration.take().unwrap_or((0.0, None));
            m.segments.push(Segment {
                uri,
                duration: d,
                title,
                range: r,
                key: key.clone(),
                map: map.clone(),
                discontinuity: std::mem::take(&mut disc),
                sequence,
            });
        }
    }
    if m.segments.is_empty() && m.ended {
        return Err(Error::Empty);
    }
    Ok(m)
}

/// The IV of a segment with no `IV` attribute: its media sequence number, big-endian.
pub fn default_iv(sequence: u64) -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv[8..].copy_from_slice(&sequence.to_be_bytes());
    iv
}

impl Master {
    /// Why it can't be played, if a session key already says so.
    pub fn refusal(&self) -> Option<Refusal> {
        self.session_keys
            .iter()
            .any(Key::is_drm)
            .then_some(Refusal::Drm)
    }

    /// Variants, lowest bandwidth first (the order the menu lists them).
    pub fn by_bandwidth(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.variants.len()).collect();
        v.sort_by_key(|&i| (self.variants[i].bandwidth, self.variants[i].resolution));
        v
    }

    /// The best variant for `bits_per_s` of measured throughput: the highest whose bandwidth
    /// fits in `margin` of it, else the lowest.
    pub fn pick(&self, bits_per_s: Option<f64>, margin: f64) -> usize {
        let order = self.by_bandwidth();
        let Some(bps) = bits_per_s else {
            return order[0];
        };
        order
            .iter()
            .copied()
            .rev()
            .find(|&i| (self.variants[i].bandwidth as f64) <= bps * margin)
            .unwrap_or(order[0])
    }

    /// The audio rendition with its own playlist that plays with variant `v`, if any.
    pub fn audio_for(&self, v: usize) -> Option<&Rendition> {
        let group = self.variants.get(v)?.audio.as_deref()?;
        let mut it = self
            .renditions
            .iter()
            .filter(|r| r.kind == "AUDIO" && r.group == group && r.uri.is_some());
        let first = it.clone().next();
        it.find(|r| r.default).or(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASTER: &str = "#EXTM3U
#EXT-X-VERSION:6
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"English\",LANGUAGE=\"en\",DEFAULT=YES,URI=\"audio/en.m3u8\"
#EXT-X-STREAM-INF:BANDWIDTH=2800000,AVERAGE-BANDWIDTH=2000000,RESOLUTION=1280x720,CODECS=\"avc1.64001f,mp4a.40.2\",FRAME-RATE=29.970,AUDIO=\"aud\"
v720/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,CODECS=\"avc1.4d401e,mp4a.40.2\"
v360/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080
https://cdn.example/v1080/index.m3u8
";

    #[test]
    fn a_master_lists_its_variants() {
        let Playlist::Master(m) = parse(MASTER, "https://h.example/show/master.m3u8").unwrap()
        else {
            panic!("a master");
        };
        assert_eq!(m.variants.len(), 3);
        let v = &m.variants[0];
        assert_eq!(v.uri, "https://h.example/show/v720/index.m3u8");
        assert_eq!(v.bandwidth, 2_800_000);
        assert_eq!(v.average_bandwidth, Some(2_000_000));
        assert_eq!(v.resolution, Some((1280, 720)));
        assert_eq!(v.codecs.as_deref(), Some("avc1.64001f,mp4a.40.2"));
        assert_eq!(v.frame_rate, Some(29.97));
        assert_eq!(m.variants[2].uri, "https://cdn.example/v1080/index.m3u8");
        assert_eq!(m.by_bandwidth(), vec![1, 0, 2]);
        assert_eq!(m.pick(None, 0.8), 1);
        assert_eq!(m.pick(Some(4e6), 0.8), 0);
        assert_eq!(m.pick(Some(10e6), 0.8), 2);
        assert_eq!(m.pick(Some(1e5), 0.8), 1);
        let a = m.audio_for(0).unwrap();
        assert_eq!(
            a.uri.as_deref(),
            Some("https://h.example/show/audio/en.m3u8")
        );
        assert_eq!(a.language.as_deref(), Some("en"));
        assert!(m.audio_for(1).is_none());
        assert!(m.refusal().is_none());
    }

    #[test]
    fn a_media_playlist_lists_segments_with_ranges_keys_and_maps() {
        let text = "#EXTM3U
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:100
#EXT-X-PLAYLIST-TYPE:VOD
#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"
#EXTINF:6.006,first
#EXT-X-BYTERANGE:1000@720
main.mp4
#EXTINF:5.5,
#EXT-X-BYTERANGE:2000
main.mp4
#EXT-X-KEY:METHOD=AES-128,URI=\"../keys/k1.bin\",IV=0x0000000000000000000000000000ABCD
#EXT-X-DISCONTINUITY
#EXTINF:4,
seg3.m4s
#EXT-X-KEY:METHOD=NONE
#EXTINF:4,
seg4.m4s
#EXT-X-ENDLIST
";
        let Playlist::Media(m) = parse(text, "http://h/a/b/media.m3u8").unwrap() else {
            panic!("a media playlist");
        };
        assert_eq!(m.target_duration, 6.0);
        assert!(m.ended && !m.live());
        assert_eq!(m.segments.len(), 4);
        let s = &m.segments;
        assert_eq!(s[0].sequence, 100);
        assert_eq!(s[3].sequence, 103);
        assert_eq!(s[0].title.as_deref(), Some("first"));
        assert_eq!(s[0].range, Some((720, 1000)));
        assert_eq!(
            s[1].range,
            Some((1720, 2000)),
            "a range without offset follows on"
        );
        assert_eq!(
            s[0].map,
            Some(Map {
                uri: "http://h/a/b/init.mp4".into(),
                range: Some((0, 720))
            })
        );
        assert_eq!(s[2].key.method, Method::Aes128);
        assert_eq!(s[2].key.uri.as_deref(), Some("http://h/a/keys/k1.bin"));
        assert_eq!(s[2].key.iv.unwrap()[14..], [0xAB, 0xCD]);
        assert!(s[2].discontinuity && !s[3].discontinuity);
        assert_eq!(s[3].key.method, Method::None);
        assert!((m.duration() - 19.506).abs() < 1e-9);
        assert!(m.refusal().is_none());
        assert_eq!(default_iv(0x0102)[14..], [1, 2]);
    }

    #[test]
    fn drm_and_sample_aes_are_refused() {
        let fairplay = "#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://key-id\",KEYFORMAT=\"com.apple.streamingkeydelivery\",KEYFORMATVERSIONS=\"1\"
#EXTINF:6,
a.ts
";
        let Playlist::Media(m) = parse(fairplay, "https://h/x.m3u8").unwrap() else {
            panic!()
        };
        assert!(m.live());
        assert_eq!(m.refusal(), Some(Refusal::Drm));
        let widevine = "#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-KEY:METHOD=SAMPLE-AES-CTR,URI=\"data:text/plain;base64,AAAA\",KEYFORMAT=\"urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed\"
#EXTINF:6,
a.m4s
";
        let Playlist::Media(m) = parse(widevine, "https://h/x.m3u8").unwrap() else {
            panic!()
        };
        assert_eq!(m.refusal(), Some(Refusal::Drm));
        let sample = "#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"k.bin\"
#EXTINF:6,
a.ts
";
        let Playlist::Media(m) = parse(sample, "https://h/x.m3u8").unwrap() else {
            panic!()
        };
        assert_eq!(m.refusal(), Some(Refusal::SampleAes));
        let session = "#EXTM3U
#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES,URI=\"skd://k\",KEYFORMAT=\"com.apple.streamingkeydelivery\"
#EXT-X-STREAM-INF:BANDWIDTH=1
a.m3u8
";
        let Playlist::Master(m) = parse(session, "https://h/x.m3u8").unwrap() else {
            panic!()
        };
        assert_eq!(m.refusal(), Some(Refusal::Drm));
    }

    #[test]
    fn attribute_lists_keep_quoted_commas() {
        let a = attributes("BANDWIDTH=1,CODECS=\"a,b\",NAME=\"x y\",FLAG");
        assert_eq!(
            a,
            vec![
                ("BANDWIDTH".into(), "1".into()),
                ("CODECS".into(), "a,b".into()),
                ("NAME".into(), "x y".into()),
                ("FLAG".into(), String::new()),
            ]
        );
        assert_eq!(parse("hello", "http://h/"), Err(Error::NotHls));
        assert_eq!(
            parse("#EXTM3U\n#EXT-X-ENDLIST\n", "http://h/"),
            Err(Error::Empty)
        );
    }
}
