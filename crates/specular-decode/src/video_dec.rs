//! Movies (MP4, QuickTime, 3GPP, Matroska, WebM, MPEG-TS): recognised here, opened by the application's registered
//! opener ([`set_video_opener`], `specular_video::install`), which demuxes and decodes them
//! as a session whose frames play through the animation player. This crate stays free of the
//! video stack; without an opener a movie is unsupported.

use std::sync::Arc;

use parking_lot::RwLock;
use specular_core::Source;

use crate::ident::Confidence;
use crate::{DecodeError, Decoder, Format, Result, Session};

/// Opens a movie as a session; the format is the one its decoder recognised.
pub type VideoOpener = Arc<dyn Fn(Arc<Source>, Format) -> Result<Arc<dyn Session>> + Send + Sync>;

static OPENER: RwLock<Option<VideoOpener>> = RwLock::new(None);

/// Registers (or with `None` removes) the opener of movies.
pub fn set_video_opener(opener: Option<VideoOpener>) {
    *OPENER.write() = opener;
}

/// What a movie holds, for the info panel, the gallery and the library.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VideoFacts {
    /// Presented duration in microseconds.
    pub duration_us: Option<u64>,
    /// Video coding ("H.264", "HEVC", "AV1", "ProRes"...).
    pub codec: String,
    /// Frames per second (average over the track).
    pub frame_rate: Option<f64>,
    pub frames: u32,
    /// Audio coding with channels and rate ("AAC, 2 ch, 48 kHz"), when there is sound.
    pub audio: Option<String>,
    /// Recording time in seconds since 1970 (a local wall-clock reading: the date as written
    /// with its zone dropped, else the movie header's UTC time).
    pub recorded: Option<i64>,
    /// The recording date as written (`©day`, Apple's creation date, XMP), else the header's.
    pub date: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    /// Clockwise rotation applied for display (`tkhd`): 0, 90, 180 or 270.
    pub rotation: u16,
    /// Pixel aspect ratio (horizontal : vertical spacing) when the pixels are not square
    /// (DVD, DV, HDV and anamorphic video): the viewer stretches the frames by it.
    pub pixel_aspect: Option<(u32, u32)>,
    /// HDR transfer ("HLG", "PQ") when the video is HDR.
    pub hdr: Option<&'static str>,
    /// The video is interlaced (shown deinterlaced).
    pub interlaced: bool,
    /// The coding's profile and level when the stream names them ("Main Profile @ High
    /// Level", "Advanced Simple Profile @ Level 5").
    pub profile: Option<String>,
    /// Why the video does not play (an unsupported codec, interlacing): shown instead of
    /// frames, never an error.
    pub unplayable: Option<String>,
    /// The sound's coding when there is sound this build can't play ("AAC Main profile",
    /// "AC-3", "ALAC"...); the application says it in words.
    pub silent_reason: Option<String>,
    /// A sound file without a picture (MP3, FLAC, M4A...): its tags. The picture is its
    /// cover and waveform.
    pub sound: Option<SoundFacts>,
    /// A movie's text tags (QuickTime / MP4 items, Matroska tags): title, comment, location…
    pub tags: Option<crate::TextTags>,
    /// A movie's XMP packet (`udta/XMP_` or the XMP `uuid` box).
    pub xmp: Option<Vec<u8>>,
}

/// What a sound file's tags and stream say, for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SoundFacts {
    /// Tag fields by stable key ("title", "artist", "album", "album-artist", "composer",
    /// "genre", "date", "track", "disc", "comment", "copyright", "encoder"), in display
    /// order.
    pub fields: Vec<(&'static str, String)>,
    /// The kinds of tag read ("ID3v2.4", "Vorbis comments", "MP4 items"...).
    pub tag_kinds: Vec<&'static str>,
    /// The cover picture: its type and size ("JPEG 600 × 600").
    pub cover: Option<String>,
    /// The cover is drawn above the graphs (it decoded).
    pub cover_shown: bool,
    /// Average bit rate over the stream, in kbit/s.
    pub kbps: Option<u32>,
    /// Bits per sample of lossless and PCM sound.
    pub bits: Option<u16>,
}

/// The sound file formats: recognised here by their first bytes, opened by the video opener.
pub struct SoundFileDecoder(pub Format);

/// A sound file's kind by its first bytes: WAVE and RF64, AIFF and AIFF-C, AMR, native
/// FLAC, raw DTS and AC-3, ADTS AAC, MPEG audio (two frames in a row, after any ID3v2 tag), Ogg whose
/// first stream is Vorbis, Opus, FLAC or Speex.
fn sound_format(head: &[u8]) -> Option<Format> {
    if head.len() >= 12
        && matches!(&head[..4], b"RIFF" | b"RF64" | b"BW64")
        && &head[8..12] == b"WAVE"
    {
        return Some(Format::Wave);
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"QLCM" {
        return Some(Format::Qcp);
    }
    if let Some(f) = more_sound_format(head) {
        return Some(f);
    }
    if head.len() >= 12 && &head[..4] == b"FORM" && matches!(&head[8..12], b"AIFF" | b"AIFC") {
        return Some(Format::Aiff);
    }
    // AMR storage files: "#!AMR\n", "#!AMR-WB\n" and the multichannel "_MC1.0" forms.
    if head.starts_with(b"#!AMR") {
        return Some(Format::Amr);
    }
    if head.len() >= 28 + 8 && &head[..4] == b"OggS" && head[5] & 2 != 0 {
        let body = &head[27 + usize::from(head[26])..];
        let audio = [&b"\x01vorbis"[..], b"OpusHead", b"\x7fFLAC", b"Speex   "];
        return audio
            .iter()
            .any(|m| body.starts_with(m))
            .then_some(Format::OggAudio);
    }
    // After ID3v2 tags (MP3, also AAC, FLAC, DTS, AC-3 files written by taggers).
    let mut at = 0;
    while let Some(h) = head.get(at..at + 10) {
        if &h[..3] != b"ID3" || h[6..10].iter().any(|b| b & 0x80 != 0) {
            break;
        }
        let size = h[6..10]
            .iter()
            .fold(0usize, |a, &b| (a << 7) | usize::from(b));
        at += 10 + size + if h[5] & 0x10 != 0 { 10 } else { 0 };
    }
    let Some(s) = head.get(at..) else {
        // The tag runs past what was read: an ID3 tag starts nothing else.
        return Some(Format::Mp3);
    };
    if s.starts_with(b"TTA1") {
        return Some(Format::Tta);
    }
    if s.starts_with(b"fLaC") {
        return Some(Format::Flac);
    }
    // WavPack blocks ("wvpk" and a 32-byte header), Monkey's Audio descriptors.
    if s.len() >= 32 && s.starts_with(b"wvpk") {
        return Some(Format::WavPack);
    }
    if s.starts_with(b"MAC ") || s.starts_with(b"MACF") {
        return Some(Format::Ape);
    }
    // Musepack: SV8's "MPCK", SV7's "MP+" and its version byte.
    if s.starts_with(b"MPCK") || s.starts_with(b"MP+") {
        return Some(Format::Mpc);
    }
    if matches!(
        s,
        [0x7F, 0xFE, 0x80, 0x01, ..]
            | [0xFE, 0x7F, 0x01, 0x80, ..]
            | [0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0..=0xFF, ..]
            | [0xFF, 0x1F, 0x00, 0xE8, 0xF0..=0xFF, 0x07, ..]
    ) {
        return Some(Format::Dts);
    }
    if let [0x0B, 0x77, _, _, b4, b5, ..] = s {
        // AC-3: fscod 0–2 and a known frame size code; E-AC-3: bsid 11–16.
        let bsid = b5 >> 3;
        if (bsid <= 10 && b4 >> 6 != 3 && b4 & 0x3F < 38) || (11..=16).contains(&bsid) {
            return Some(Format::Ac3);
        }
    }
    // ADTS: the 12-bit sync, layer 0, a known rate index, and a second frame where the
    // first one's length says.
    if let [0xFF, b1, b2, b3, b4, b5, ..] = s
        && b1 & 0xF6 == 0xF0
        && (b2 >> 2) & 0xF < 12
    {
        let len = (usize::from(b3 & 3) << 11) | (usize::from(*b4) << 3) | usize::from(b5 >> 5);
        if len >= 7
            && s.get(len..len + 2)
                .is_none_or(|n| n[0] == 0xFF && n[1] & 0xF6 == 0xF0)
        {
            return Some(Format::Aac);
        }
    }
    // MPEG audio: a header (sync, version not reserved, layer not reserved, bit rate not
    // "bad", rate index not reserved) and, unless the file is that short, another header
    // where the frame ends.
    let mpa = |b: &[u8]| -> Option<usize> {
        let (b1, b2) = (*b.get(1)?, *b.get(2)?);
        if b[0] != 0xFF || b1 & 0xE0 != 0xE0 {
            return None;
        }
        let version = (b1 >> 3) & 3; // 0: 2.5, 2: 2, 3: 1
        let layer = (b1 >> 1) & 3; // 1: III, 2: II, 3: I
        let br = b2 >> 4;
        let sr = (b2 >> 2) & 3;
        if version == 1 || layer == 0 || br == 0 || br == 15 || sr == 3 {
            return None;
        }
        const RATES: [[u32; 3]; 4] = [
            [11_025, 12_000, 8_000],
            [0, 0, 0],
            [22_050, 24_000, 16_000],
            [44_100, 48_000, 32_000],
        ];
        const V1: [[u32; 15]; 3] = [
            [
                0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
            ],
            [
                0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
            ],
            [
                0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
            ],
        ];
        const V2: [[u32; 15]; 2] = [
            [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
            [
                0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
            ],
        ];
        let rate = RATES[usize::from(version)][usize::from(sr)];
        let l = usize::from(3 - layer); // 0: III, 1: II, 2: I
        let kbps = if version == 3 {
            V1[l][usize::from(br)]
        } else {
            V2[usize::from(l == 2)][usize::from(br)]
        };
        let pad = usize::from((b2 >> 1) & 1);
        Some(if l == 2 {
            (12 * kbps * 1000 / rate) as usize * 4 + pad * 4
        } else {
            let k = if l == 0 && version != 3 { 72 } else { 144 };
            (k * kbps * 1000 / rate) as usize + pad
        })
    };
    let tagged = at > 0;
    let reach = if tagged { s.len().min(4096) } else { 0 };
    (0..=reach)
        .take_while(|&i| i == 0 || s[i - 1] == 0)
        .find(|&i| {
            let Some(len) = s.get(i..).and_then(mpa) else {
                return false;
            };
            match s.get(i + len..) {
                Some(n) if n.len() >= 3 => mpa(n).is_some() && (n[1] & 0x1E) == (s[i + 1] & 0x1E),
                // A file of one frame, or a head cut before the next.
                _ => tagged || s.len() >= len,
            }
        })
        .map(|_| Format::Mp3)
}

/// Whether `t` is a ProTracker-style MOD tag (at byte 1080).
fn mod_tag(t: &[u8]) -> bool {
    let digit = |b: u8| b.is_ascii_digit();
    match t {
        b"M.K." | b"M!K!" | b"M&K!" | b"FLT4" | b"FLT8" | b"4CHN" | b"N.T." | b"OKTA" | b"OCTA"
        | b"CD81" => true,
        [c, b'C', b'H', b'N'] => digit(*c) && *c != b'0',
        [a, b, b'C', b'H'] | [a, b, b'C', b'N'] => digit(*a) && digit(*b),
        [b'T', b'D', b'Z', c] => digit(*c) && *c != b'0',
        _ => false,
    }
}

/// The older tracker modules by their signatures: MultiTracker ("MTM" 1.x), Scream Tracker 2
/// (1Ah and type 2 at byte 28, version 2), Oktalyzer ("OKTASONG" "CMOD"), MED ("MMD0" to
/// "MMD3"), UltraTracker ("MAS_UTrack_V00" 1–4), Farandole ("FAR" FEh, 0D 0A 1A at 44) and
/// Composer 669 ("if" or "JN" with order, speed and break lists in range).
fn old_module(h: &[u8]) -> bool {
    let six69 = || {
        h.len() >= 0x1F1
            && (h.starts_with(b"if") || h.starts_with(b"JN"))
            && h[0x6E] <= 64
            && (1..=128).contains(&h[0x6F])
            && h[0x70] < 128
            && h[0x71] != 0xFF
            && h[0x71..0xF1].iter().all(|&o| o == 0xFF || o < h[0x6F])
            && h[0x171..0x1F1].iter().all(|&b| b < 64)
    };
    (h.len() >= 66 && h.starts_with(b"MTM") && h[3] >> 4 == 1)
        || (h.len() >= 48
            && h[28] == 0x1A
            && h[29] == 2
            && h[30] == 2
            && h[20..28].iter().all(|c| c.is_ascii_graphic()))
        || h.starts_with(b"OKTASONG") && h.get(8..12) == Some(b"CMOD")
        || matches!(h.get(..4), Some(b"MMD0" | b"MMD1" | b"MMD2" | b"MMD3")) && h.len() >= 52
        || h.starts_with(b"MAS_UTrack_V00") && h.get(14).is_some_and(|v| (b'1'..=b'4').contains(v))
        || h.starts_with(b"FAR\xFE") && h.get(44..47) == Some(&[13, 10, 26][..])
        || six69()
}

/// AU, CAF, Wave64, VOC, 8SVX, AVR, NIST SPHERE, IRCAM, Shorten, DSF, DSDIFF and tracker
/// modules, by their signatures. A MOD is told by its tag at byte 1080 only, never by
/// name: `.mod` is also a camcorder's MPEG programme stream (left to the video path).
fn more_sound_format(head: &[u8]) -> Option<Format> {
    const W64: [u8; 16] = [
        0x72, 0x69, 0x66, 0x66, 0x2E, 0x91, 0xCF, 0x11, 0xA5, 0xD6, 0x28, 0xDB, 0x04, 0xC1, 0x00,
        0x00,
    ];
    let le32 = |at: usize| {
        head.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let be32 = |at: usize| {
        head.get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    if head.starts_with(&[0, 0, 1, 0xBA]) {
        return None;
    }
    if head.len() >= 24 && (head.starts_with(b".snd") || head.starts_with(b"dns.")) {
        return Some(Format::Au);
    }
    if head.len() >= 8 && head.starts_with(b"caff") && head[4..6] == [0, 1] {
        return Some(Format::Caf);
    }
    if head.len() >= 28 && head[..16] == W64 && &head[24..28] == b"wave" {
        return Some(Format::W64);
    }
    // QOA: "qoaf", then a first frame with channels and a sample rate.
    if head.len() >= 16 && head.starts_with(b"qoaf") && head[8] > 0 {
        return Some(Format::Qoa);
    }
    if head.starts_with(b"Creative Voice File\x1A") {
        return Some(Format::Voc);
    }
    if head.len() >= 12 && head.starts_with(b"FORM") && matches!(&head[8..12], b"8SVX" | b"16SV") {
        return Some(Format::Svx);
    }
    if head.len() >= 128 && head.starts_with(b"2BIT") {
        return Some(Format::Avr);
    }
    if head.starts_with(b"NIST_1A\n") {
        return Some(Format::Sphere);
    }
    if matches!(le32(0), Some(0x0001_A364 | 0x0003_A364))
        || matches!(be32(0), Some(0x0002_A364 | 0x0004_A364))
    {
        return Some(Format::Ircam);
    }
    if head.len() >= 5 && head.starts_with(b"ajkg") && (1..=3).contains(&head[4]) {
        return Some(Format::Shorten);
    }
    if head.starts_with(b"DSD ") && le32(4) == Some(28) && le32(8) == Some(0) {
        return Some(Format::Dsf);
    }
    if head.len() >= 16 && head.starts_with(b"FRM8") && &head[12..16] == b"DSD " {
        return Some(Format::Dff);
    }
    let module = head.starts_with(b"IMPM")
        || head.starts_with(b"Extended Module: ")
        || old_module(head)
        || (head.get(44..48) == Some(b"SCRM") && head.get(29) == Some(&16))
        || (head.get(1080..1084).is_some_and(mod_tag)
            && head.get(950).is_some_and(|&n| (1..=128).contains(&n)));
    module.then_some(Format::Tracker)
}

impl Decoder for SoundFileDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Format::M4a => {
                head.get(4..8) == Some(b"ftyp")
                    && matches!(head.get(8..12), Some(b"M4A " | b"M4B " | b"M4P "))
            }
            Format::Midi => crate::sound_ident::is_midi(head),
            Format::SoundFont => crate::sound_ident::is_soundfont(head),
            Format::Dls => crate::sound_ident::is_dls(head),
            Format::Xi => crate::sound_ident::is_xi(head),
            Format::Sfz => crate::sound_ident::is_sfz(head),
            // By the whole file's size (`probe_file`), or by extension.
            Format::Htk | Format::Gsm | Format::Sln => false,
            f => sound_format(head) == Some(f),
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Format::Htk => return crate::sound_ident::is_htk(data),
            Format::Gsm => return crate::sound_ident::is_gsm(data),
            _ => {}
        }
        // Past the ID3v2 tags (a cover can make them megabytes long), enough for two MPEG
        // audio or ADTS frames.
        let mut at = 0;
        while let Some(h) = data.get(at..at + 10) {
            if &h[..3] != b"ID3" || h[6..10].iter().any(|b| b & 0x80 != 0) {
                break;
            }
            let size = h[6..10]
                .iter()
                .fold(0usize, |a, &b| (a << 7) | usize::from(b));
            at += 10 + size + if h[5] & 0x10 != 0 { 10 } else { 0 };
        }
        self.probe(&data[..data.len().min(at.saturating_add(8192))])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Mp3 => &["mp3", "mp2", "mpga", "mp1", "mpa", "m1a", "m2a"],
            Format::Aac => &["aac", "adts"],
            Format::Flac => &["flac"],
            Format::Wave => &["wav", "rf64", "bw64"],
            Format::Au => &["au", "snd"],
            Format::Caf => &["caf"],
            Format::W64 => &["w64"],
            Format::Voc => &["voc"],
            Format::Svx => &["8svx", "16sv", "svx"],
            Format::Avr => &["avr"],
            Format::Sphere => &["sph", "nist"],
            Format::Ircam => &["sf", "ircam"],
            Format::Tta => &["tta"],
            Format::Shorten => &["shn"],
            Format::Dsf => &["dsf"],
            Format::Dff => &["dff"],
            Format::Tracker => &[
                "mod", "s3m", "xm", "it", "669", "mtm", "stm", "okt", "okta", "med", "mmd0",
                "mmd1", "mmd2", "mmd3", "ult", "far",
            ],
            Format::Aiff => &["aiff", "aif", "aifc"],
            Format::Dts => &["dts"],
            Format::Ac3 => &["ac3", "eac3"],
            Format::Amr => &["amr", "awb"],
            Format::WavPack => &["wv"],
            Format::Ape => &["ape"],
            Format::Mpc => &["mpc", "mp+", "mpp"],
            Format::OggAudio => &["oga", "opus", "spx"],
            Format::Midi => &["mid", "midi", "kar", "rmi"],
            Format::SoundFont => &["sf2"],
            Format::Dls => &["dls"],
            Format::Xi => &["xi"],
            Format::Sfz => &["sfz"],
            Format::Htk => &["htk"],
            Format::Gsm => &["gsm"],
            Format::Qcp => &["qcp"],
            Format::Qoa => &["qoa"],
            Format::Sln => &[
                "sln", "sln12", "sln16", "sln24", "sln32", "sln44", "sln48", "sln96", "sln192",
            ],
            _ => &["m4a", "m4b"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let opener = OPENER.read().clone();
        match opener {
            Some(open) => open(source, self.0),
            None => Err(DecodeError::Unsupported("sound".into())),
        }
    }
}

pub struct VideoDecoder(pub Format);

/// `ftyp` major brands of movies (compatible brands are checked too).
const MP4_BRANDS: &[&[u8; 4]] = &[
    b"isom", b"iso2", b"iso3", b"iso4", b"iso5", b"iso6", b"mp41", b"mp42", b"avc1", b"M4V ",
    b"M4VH", b"M4VP", b"mmp4", b"MSNV", b"XAVC", b"NDAS", b"f4v ", b"dby1", b"av01", b"hvc1",
];
const QT_BRANDS: &[&[u8; 4]] = &[b"qt  "];
/// 3GPP (`3gp4`..`3gp9`, `3ge*`, `3gg*`, `3gs*`, `3gr*`) and 3GPP2 (`3g2*`).
fn is_3gpp_brand(b: &[u8]) -> bool {
    b.len() == 4 && b[0] == b'3' && b[1] == b'g'
}

/// Brands of still-image HEIF and AVIF files, which share the box structure.
const IMAGE_BRANDS: &[&[u8; 4]] = &[
    b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"mif1", b"msf1", b"avif", b"avis",
    b"avci", b"jpeg", b"jxl ", b"crx ",
];

/// The movie format an `ftyp` box names (`None`: no `ftyp`, or an image brand).
fn ftyp_format(head: &[u8]) -> Option<Format> {
    if head.get(4..8)? != b"ftyp" {
        return None;
    }
    let size = u32::from_be_bytes(head.get(..4)?.try_into().ok()?) as usize;
    let end = size.clamp(8, head.len());
    let major = head.get(8..12)?;
    let compatible: Vec<&[u8]> = head
        .get(16..end)
        .unwrap_or_default()
        .chunks_exact(4)
        .collect();
    let brands = || std::iter::once(major).chain(compatible.iter().copied());
    if brands().any(|b| IMAGE_BRANDS.iter().any(|i| &i[..] == b)) && major != b"qt  " {
        return None;
    }
    if is_3gpp_brand(major) {
        return Some(Format::ThreeGpp);
    }
    if QT_BRANDS.iter().any(|b| &b[..] == major) {
        return Some(Format::QuickTime);
    }
    if MP4_BRANDS.iter().any(|b| &b[..] == major) {
        return Some(Format::Mp4);
    }
    // An unfamiliar major brand with familiar compatible ones.
    if compatible.iter().any(|b| is_3gpp_brand(b)) {
        return Some(Format::ThreeGpp);
    }
    if compatible
        .iter()
        .any(|b| MP4_BRANDS.iter().any(|m| &m[..] == *b))
    {
        return Some(Format::Mp4);
    }
    if compatible
        .iter()
        .any(|b| QT_BRANDS.iter().any(|m| &m[..] == *b))
    {
        return Some(Format::QuickTime);
    }
    None
}

/// The DocType of an EBML header at the start of `head` (`matroska`, `webm`).
fn ebml_doc_type(head: &[u8]) -> Option<&[u8]> {
    if !head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return None;
    }
    // A variable-size integer: (value, length).
    let vint = |p: usize| -> Option<(usize, usize)> {
        let first = *head.get(p)?;
        let len = first.leading_zeros() as usize + 1;
        if len > 8 {
            return None;
        }
        let mut v = u64::from(first) & (0xFF >> len);
        for &b in head.get(p + 1..p + len)? {
            v = (v << 8) | u64::from(b);
        }
        Some((usize::try_from(v).ok()?, len))
    };
    let (size, l) = vint(4)?;
    let end = (4 + l).checked_add(size)?.min(head.len());
    let mut p = 4 + l;
    while p + 2 < end {
        // Element IDs of the header are one or two bytes long.
        let id_len = head[p].leading_zeros() as usize + 1;
        let id = head.get(p..p + id_len)?;
        let (len, sl) = vint(p + id_len)?;
        let body = p + id_len + sl;
        if id == [0x42, 0x82] {
            let v = head.get(body..body.checked_add(len)?)?;
            return Some(v.split(|&c| c == 0).next().unwrap_or(v));
        }
        p = body.checked_add(len)?;
    }
    None
}

/// Whether `head` starts a transport stream: the sync byte 0x47 every 188 bytes, or every
/// 192 after a 4-byte timestamp (AVCHD, Blu-ray), five times in a row.
fn transport_stream(head: &[u8]) -> bool {
    [(188usize, 0usize), (192, 4)]
        .iter()
        .any(|&(s, f)| (0..5).all(|k| head.get(f + k * s) == Some(&0x47)))
}

/// An MPEG programme stream: a pack header of MPEG-1 (`0010`) or MPEG-2 (`01`) form.
fn program_stream(head: &[u8]) -> bool {
    head.starts_with(&[0, 0, 1, 0xBA]) && head.get(4).is_some_and(|&b| b >> 4 == 2 || b >> 6 == 1)
}

/// A bare MPEG-1/2 video stream: a sequence header whose picture size isn't zero.
fn mpeg_video(head: &[u8]) -> bool {
    head.len() >= 8
        && head.starts_with(&[0, 0, 1, 0xB3])
        && (u16::from(head[4]) << 4 | u16::from(head[5] >> 4)) > 0
        && (u16::from(head[5] & 15) << 8 | u16::from(head[6])) > 0
        && head[7] & 15 != 0
}

/// An Ogg stream whose first logical stream is video (Theora, Dirac, OGM video): sound-only
/// Ogg files (`.ogg`, `.oga`, `.opus`) are not movies.
fn ogg_video(head: &[u8]) -> bool {
    if head.len() < 28 + 7 || &head[..4] != b"OggS" || head[5] & 2 == 0 {
        return false;
    }
    let body = &head[27 + usize::from(head[26])..];
    body.starts_with(b"\x80theora") || body.starts_with(b"BBCD") || body.starts_with(b"\x01video")
}

/// An MXF file: a header partition pack (SMPTE ST 377-1 §7.1) at the start or after a run-in
/// in the bytes given.
fn mxf(head: &[u8]) -> bool {
    const KEY: [u8; 13] = [
        0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01,
    ];
    head.windows(14)
        .take(65_536)
        .any(|w| w[..13] == KEY && w[13] == 0x02)
}

/// The ASF header object's GUID as stored.
const ASF_HEADER: [u8; 16] = [
    0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C,
];

/// Raw DV (IEC 61834 / SMPTE 314M): 80-byte DIF blocks, the first a header block (section
/// type 0, DIF sequence 0, block 0), then two subcode blocks (section 1) and three VAUX
/// blocks (section 2).
fn raw_dv(head: &[u8]) -> bool {
    let section = |k: usize| head.get(80 * k).map(|b| b >> 5);
    head.len() >= 480
        && section(0) == Some(0)
        && head[1] >> 4 == 0
        && head[2] == 0
        && section(1) == Some(1)
        && section(2) == Some(1)
        && (3..6).all(|k| section(k) == Some(2))
}

/// A raw video elementary stream (see `specular_video::es`): an H.264 or HEVC start code
/// at the very start, a first NAL unit that opens a stream (delimiter, parameter set, SEI,
/// a slice for H.264) and a sequence parameter set within the first 64 KiB; or an AV1
/// temporal delimiter OBU followed by a sequence header OBU.
fn elementary(head: &[u8]) -> bool {
    let head = &head[..head.len().min(64 << 10)];
    if head.starts_with(&[0x12, 0x00]) && head.get(2).is_some_and(|&b| (b >> 3) & 15 == 1) {
        return true;
    }
    let first = if head.starts_with(&[0, 0, 0, 1]) {
        4
    } else if head.starts_with(&[0, 0, 1]) {
        3
    } else {
        return false;
    };
    let (Some(&b0), Some(&b1)) = (head.get(first), head.get(first + 1)) else {
        return false;
    };
    if b0 & 0x80 != 0 {
        return false;
    }
    let hevc = b0 & 1 == 0 && b1 & 7 >= 1 && matches!((b0 >> 1) & 0x3F, 32..=35 | 39);
    let avc = matches!(b0 & 0x1F, 1 | 5 | 6 | 7 | 9);
    // A sequence parameter set among the NAL headers after the start codes.
    head.windows(4)
        .filter_map(|w| (w[..3] == [0, 0, 1]).then_some(w[3]))
        .any(|h| (hevc && (h >> 1) & 0x3F == 33) || (avc && h & 0x1F == 7))
}

/// CD+G graphics: 24-byte packets, the first few hundred all CD+G (command 9, a known
/// instruction) or empty, at least one drawing.
fn cdg_packets(head: &[u8]) -> bool {
    if head.len() < 24 * 8 {
        return false;
    }
    let mut drawing = false;
    for p in head.chunks_exact(24).take(600) {
        match p[0] {
            0 => {}
            9 if matches!(p[1] & 0x3F, 1 | 2 | 6 | 20 | 24 | 28 | 30 | 31 | 38) => drawing = true,
            _ => return false,
        }
    }
    drawing
}

/// A PlayStation movie: raw CD-ROM sectors (sync pattern, or a RIFF `CDXA` wrapper),
/// 2336-byte sectors (subheader twice) or 2048-byte ones, with an STR video header (0160h,
/// 8001h) at the data of one of the first sectors.
fn psx_str(head: &[u8]) -> bool {
    const SYNC: [u8; 12] = [0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0];
    const MAGIC: [u8; 4] = [0x60, 0x01, 0x01, 0x80];
    let (base, size, data) = if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"CDXA") {
        (44, 2352, 24)
    } else if head.starts_with(&SYNC) {
        (0, 2352, 24)
    } else if head.len() >= 8 && head[..4] == head[4..8] {
        (0, 2336, 8)
    } else {
        (0, 2048, 0)
    };
    (0..32).any(|k| {
        let at = base + k * size + data;
        head.get(at..at + 4) == Some(&MAGIC[..])
    })
}

impl Decoder for VideoDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Format::Matroska => ebml_doc_type(head) == Some(b"matroska"),
            Format::WebM => ebml_doc_type(head) == Some(b"webm"),
            Format::MpegTs => transport_stream(head),
            Format::MpegPs => program_stream(head),
            Format::MpegVideo => mpeg_video(head),
            Format::Avi => head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"AVI ",
            Format::Ogg => ogg_video(head),
            Format::Ivf => head.len() >= 32 && &head[..4] == b"DKIF",
            Format::Mxf => mxf(head),
            Format::Flv => head.len() >= 9 && &head[..3] == b"FLV" && head[3] == 1,
            Format::Asf => head.starts_with(&ASF_HEADER),
            Format::Wtv => crate::sound_ident::is_wtv(head),
            Format::Dv => raw_dv(head),
            Format::Y4m => head.starts_with(b"YUV4MPEG2 "),
            Format::Cdg => cdg_packets(head),
            // Structural: see `confidence`.
            Format::VideoStream => false,
            Format::Apv => {
                head.len() >= 8
                    && &head[4..8] == b"aPv1"
                    && u32::from_be_bytes([head[0], head[1], head[2], head[3]]) >= 8
            }
            Format::PsxStr => psx_str(head),
            f => ftyp_format(head) == Some(f),
        }
    }
    /// CD+G has no signature: packets that are all CD+G commands, with some drawing, are
    /// likely.
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        match self.0 {
            Format::Cdg => {
                cdg_packets(&data[..data.len().min(24 * 600)]).then_some(Confidence::Likely)
            }
            Format::VideoStream => elementary(data).then_some(Confidence::Likely),
            // Movies may start with sound sectors: look past the first.
            Format::PsxStr => {
                psx_str(&data[..data.len().min(44 + 32 * 2352)]).then_some(Confidence::Certain)
            }
            _ => self.probe_file(data).then_some(Confidence::Certain),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            // `.f4v`: Adobe's Flash MP4 (brand `f4v `).
            // GoPro low-resolution proxies (`.lrv`) and 360° clips (`.360`, two HEVC
            // tracks), Insta360 clips (`.insv`): MP4.
            Format::Mp4 => &["mp4", "m4v", "f4v", "lrv", "360", "insv"],
            Format::QuickTime => &["mov", "qt"],
            // `.mka`: Matroska sound, shown as a sound file.
            Format::Matroska => &["mkv", "mk3d", "mka"],
            Format::WebM => &["webm"],
            // `.trp`/`.tp`: broadcast captures (TV cards, set-top boxes).
            Format::MpegTs => &["mts", "m2ts", "ts", "m2t", "tod", "trp", "tp"],
            // `.evo`/`.evob`: unencrypted HD DVD programme streams.
            Format::MpegPs => &[
                "mpg", "mpeg", "mpe", "vob", "mod", "vro", "m2p", "evo", "evob",
            ],
            Format::MpegVideo => &["m1v", "m2v", "mpv"],
            Format::Avi => &["avi", "divx"],
            Format::Ogg => &["ogv", "ogg", "ogm"],
            Format::Ivf => &["ivf"],
            Format::Mxf => &["mxf"],
            Format::Flv => &["flv"],
            // `.dvr-ms`: Windows XP/Vista Media Center recordings (ASF).
            Format::Asf => &["wmv", "asf", "dvr-ms"],
            // Windows Vista (TV Pack) to 8.1 Media Center recordings.
            Format::Wtv => &["wtv"],
            Format::Dv => &["dv", "dif"],
            Format::Y4m => &["y4m"],
            Format::Cdg => &["cdg"],
            Format::VideoStream => &[
                "h264", "264", "avc", "h265", "265", "hevc", "obu", "mjpeg", "mjpg",
            ],
            Format::Apv => &["apv"],
            Format::PsxStr => &["str"],
            _ => &["3gp", "3g2"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let opener = OPENER.read().clone();
        match opener {
            Some(open) => open(source, self.0),
            None => Err(DecodeError::Unsupported("video".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ftyp(major: &[u8; 4], compat: &[&[u8; 4]]) -> Vec<u8> {
        let mut v = (16 + 4 * compat.len() as u32).to_be_bytes().to_vec();
        v.extend_from_slice(b"ftyp");
        v.extend_from_slice(major);
        v.extend_from_slice(&[0, 0, 2, 0]);
        for c in compat {
            v.extend_from_slice(*c);
        }
        v.extend_from_slice(b"\0\0\0\x08free");
        v
    }

    #[test]
    fn brands_choose_the_format() {
        assert_eq!(
            ftyp_format(&ftyp(b"isom", &[b"isom", b"avc1"])),
            Some(Format::Mp4)
        );
        assert_eq!(ftyp_format(&ftyp(b"mp42", &[b"mp41"])), Some(Format::Mp4));
        assert_eq!(ftyp_format(&ftyp(b"M4V ", &[b"M4V "])), Some(Format::Mp4));
        assert_eq!(
            ftyp_format(&ftyp(b"qt  ", &[b"qt  "])),
            Some(Format::QuickTime)
        );
        assert_eq!(
            ftyp_format(&ftyp(b"3gp5", &[b"isom"])),
            Some(Format::ThreeGpp)
        );
        assert_eq!(ftyp_format(&ftyp(b"3g2a", &[])), Some(Format::ThreeGpp));
        assert_eq!(ftyp_format(&ftyp(b"abcd", &[b"mp42"])), Some(Format::Mp4));
        // Still images keep their decoders.
        assert_eq!(ftyp_format(&ftyp(b"heic", &[b"mif1", b"heic"])), None);
        assert_eq!(ftyp_format(&ftyp(b"avif", &[b"avif", b"mif1"])), None);
        assert_eq!(ftyp_format(&ftyp(b"isom", &[b"avis", b"msf1"])), None);
        assert_eq!(ftyp_format(b"\0\0\0\x08moov"), None);
    }

    #[test]
    fn matroska_webm_and_transport_streams() {
        // EBML header: DocTypeVersion, then DocType.
        let ebml = |doc: &[u8]| {
            let mut body = vec![0x42, 0x87, 0x81, 0x04, 0x42, 0x82, 0x80 | doc.len() as u8];
            body.extend_from_slice(doc);
            let mut v = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80 | body.len() as u8];
            v.extend(body);
            v.extend_from_slice(&[0x18, 0x53, 0x80, 0x67, 0xFF]);
            v
        };
        assert!(VideoDecoder(Format::WebM).probe(&ebml(b"webm")));
        assert!(!VideoDecoder(Format::Matroska).probe(&ebml(b"webm")));
        assert!(VideoDecoder(Format::Matroska).probe(&ebml(b"matroska")));
        assert!(!VideoDecoder(Format::Matroska).probe(&ebml(b"other")));
        assert!(!VideoDecoder(Format::Matroska).probe(&ebml(b"matroska")[..8]));
        let mut ts = vec![0u8; 188 * 6];
        for k in 0..6 {
            ts[k * 188] = 0x47;
        }
        assert!(VideoDecoder(Format::MpegTs).probe(&ts));
        let mut m2ts = vec![0u8; 192 * 6];
        for k in 0..6 {
            m2ts[4 + k * 192] = 0x47;
        }
        assert!(VideoDecoder(Format::MpegTs).probe(&m2ts));
        ts[188 * 3] = 0;
        assert!(!VideoDecoder(Format::MpegTs).probe(&ts));
        assert!(!VideoDecoder(Format::MpegTs).probe(b"G\0\0 TypeScript? no"));
        // Programme streams of both versions, bare MPEG video, AVI.
        assert!(VideoDecoder(Format::MpegPs).probe(&[0, 0, 1, 0xBA, 0x44, 0, 4]));
        assert!(VideoDecoder(Format::MpegPs).probe(&[0, 0, 1, 0xBA, 0x21, 0, 1]));
        assert!(!VideoDecoder(Format::MpegPs).probe(&[0, 0, 1, 0xBA, 0x00, 0, 1]));
        assert!(VideoDecoder(Format::MpegVideo).probe(&[0, 0, 1, 0xB3, 0x16, 0x01, 0x20, 0x13]));
        assert!(!VideoDecoder(Format::MpegVideo).probe(&[0, 0, 1, 0xB3, 0, 0, 0, 0x13]));
        assert!(VideoDecoder(Format::Avi).probe(b"RIFF\x10\0\0\0AVI LIST"));
        assert!(!VideoDecoder(Format::Avi).probe(b"RIFF\x10\0\0\0WAVEfmt "));
        // Ogg with Theora first, not sound-only Ogg; IVF.
        let mut ogg = b"OggS\0\x02".to_vec();
        ogg.extend_from_slice(&[0; 20]);
        ogg.push(1);
        ogg.push(42);
        ogg.extend_from_slice(b"\x80theora\x03\x02\x01");
        assert!(VideoDecoder(Format::Ogg).probe(&ogg));
        let at = ogg.len() - 10;
        ogg[at..at + 7].copy_from_slice(b"\x01vorbis");
        assert!(!VideoDecoder(Format::Ogg).probe(&ogg));
        assert!(
            VideoDecoder(Format::Ivf)
                .probe(b"DKIF\0\0\x20\0VP80\x40\0\x30\0\x1e\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0")
        );
        // MXF: the header partition pack, at the start or after a run-in; not a body one.
        let mut pp = vec![
            0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02,
        ];
        pp.extend([0x01, 0x01, 0x02, 0x04, 0x00, 0x83]);
        assert!(VideoDecoder(Format::Mxf).probe(&pp));
        let mut run_in = vec![0u8; 100];
        run_in.extend(&pp);
        assert!(VideoDecoder(Format::Mxf).probe(&run_in));
        pp[13] = 0x03;
        assert!(!VideoDecoder(Format::Mxf).probe(&pp));
    }

    #[test]
    fn without_an_opener_a_movie_is_unsupported() {
        let d = VideoDecoder(Format::Mp4);
        assert!(d.probe(&ftyp(b"isom", &[b"isom"])));
        assert!(!VideoDecoder(Format::QuickTime).probe(&ftyp(b"isom", &[b"isom"])));
    }
}
