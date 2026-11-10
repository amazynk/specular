//! ISO-BMFF / QuickTime (MP4, MOV, M4V, 3GP, 3G2) demux.
//!
//! Zero-copy over a byte slice that holds the whole movie (an embedded motion-photo video is a
//! complete MP4 appended to the still; its chunk offsets count from its own start). Parses
//! `ftyp`, `moov/mvhd` (timescale, duration, creation time), every `trak` (`tkhd` size and
//! rotation, `edts/elst` edit list, `mdia/mdhd` timescale and duration, `hdlr`, `stsd` with
//! `avcC` / `hvcC` / `av1C` / `colr` / `pasp` / `fiel` for video and `esds` / `alac` for
//! audio, and the sample table `stts`, `ctts`, `stss`, `stsz` / `stz2`, `stsc`, `stco` /
//! `co64`), and the metadata: QuickTime `meta/keys` + `ilst` (Apple's content identifier,
//! creation date, make and model), iTunes-style `ilst` and classic `udta` `©day` / `©mak` /
//! `©mod` items, and XMP (`udta/XMP_` or the XMP `uuid` box). Fragments (`moof`) are not
//! read: a fragmented file has no samples in its `moov` and is reported so.
//!
//! Every count and offset is checked against the bytes present before it is used, and the
//! sample table is capped, so a malformed file is an error, never a panic or a huge allocation.

use std::borrow::Cow;
use std::fmt;

/// Most samples a track may have (a minute at 240 fps is 14 400; six hours of 48 kHz AAC is
/// about a million).
pub const MAX_SAMPLES: usize = 1 << 20;
/// Deepest box nesting followed.
const MAX_DEPTH: usize = 12;
/// Seconds from 1904-01-01 (the QuickTime epoch) to 1970-01-01.
const EPOCH_1904: i64 = 2_082_844_800;
/// The XMP `uuid` box's identifier.
const XMP_UUID: [u8; 16] = [
    0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a movie, or a structure that contradicts itself.
    Malformed(String),
    /// A valid movie this demuxer does not read (fragmented, no video track).
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "malformed movie: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported movie: {s}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The video coding of a track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    /// `avc1` / `avc3` with an `avcC` record.
    H264,
    /// `hvc1` / `hev1` with an `hvcC` record.
    Hevc,
    /// `av01` with an `av1C` record.
    Av1,
    /// VP8 (Matroska `V_VP8`, MP4 `vp08`).
    Vp8,
    /// VP9 (Matroska `V_VP9`, MP4 `vp09` with a `vpcC` record).
    Vp9,
    /// JPEG XS (MP4 `jxsm`, ISO/IEC 21122-3): every sample a codestream.
    JpegXs,
    /// MPEG-2 video (H.262): transport and programme streams, MP4 `mp4v` with object type
    /// 0x60–0x65, QuickTime XDCAM / HDV / IMX entries, Matroska `V_MPEG2`. The configuration
    /// is the sequence header, when the container gives it.
    Mpeg2,
    /// MPEG-1 video (decoded by the MPEG-2 decoder).
    Mpeg1,
    /// MPEG-4 Part 2 (Simple and Advanced Simple Profile: DivX, Xvid): MP4 `mp4v` with object
    /// type 0x20, AVI, Matroska. The configuration is the VOL header, when given.
    Mpeg4,
    /// H.263 baseline (3GPP `s263`), decoded by the MPEG-4 Part 2 decoder (short header).
    H263,
    /// Theora (Ogg, Matroska `V_THEORA`): the configuration is its three headers,
    /// Xiph-laced.
    Theora,
    /// Apple ProRes (QuickTime `apco`, `apcs`, `apcn`, `apch`, `ap4h`, `ap4x`; Matroska
    /// `V_PRORES`; MXF). The configuration is the four-character code (the profile).
    ProRes,
    /// Motion JPEG (QuickTime `jpeg`/`mjpa`/`mjpb`/`AVDJ`, AVI `MJPG`, Matroska `V_MJPEG`):
    /// every sample a JPEG picture or a pair of field pictures.
    Mjpeg,
    /// JPEG 2000 (MP4 `mjp2`, MXF): every sample a codestream (or a `jp2c` box).
    Jpeg2000,
    /// DV (IEC 61834, SMPTE 314M: DV, DVCPRO, DVCPRO50; raw `.dv`, AVI, QuickTime `dvc `...).
    Dv,
    /// VC-1 / WMV9 (ASF, AVI, Matroska `WMV3`/`WVC1`, MP4 `vc-1`). The configuration is the
    /// sequence header: STRUCT_C for Simple/Main (`WMV3`), start-code units for Advanced.
    Vc1,
    /// VP3 (AVI `VP31`): Theora's ancestor, without headers.
    Vp3,
    /// Sorenson Spark (FLV codec 2, `FLV1`), an H.263 variant.
    Flv1,
    /// FFV1 (RFC 9043; Matroska `V_FFV1`, AVI and QuickTime `FFV1`). The configuration is
    /// the configuration record (version 3), empty for versions 0 and 1.
    Ffv1,
    /// Uncompressed video. The configuration is a [`crate::rawvid::PixDesc`].
    Raw,
    /// An old palette, RGB or vector codec. The configuration is a
    /// [`crate::rawvid::PixDesc`] (depth and colour table).
    Classic(Classic),
    /// GoPro CineForm (`CFHD`): every sample a picture, or a group's first picture.
    CineForm,
    /// Vidvox HAP (`Hap1`, `Hap5`, `HapY`, `HapM`, `HapA`, `Hap7`, `HapH`): every sample a
    /// texture frame.
    Hap(specular_hap::Kind),
    /// SMPTE VC-2 / Dirac (MP4 `drac`, Matroska `V_DIRAC`, Ogg): every sample a sequence of
    /// data units.
    Vc2,
    /// APV (MP4 `apv1`, the raw `.apv` file): every sample an access unit.
    Apv,
    /// Any other sample entry (its four-character code).
    Other([u8; 4]),
}

/// The old codecs [`Codec::Classic`] names (decoded by `specular-oldvideo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classic {
    /// QuickTime Animation (`rle `).
    QtRle,
    /// Cinepak (`cvid`).
    Cinepak,
    /// Microsoft Video 1 (`CRAM`, `MSVC`).
    MsVideo1,
    /// Microsoft RLE (BI_RLE8, BI_RLE4).
    MsRle,
    /// QuickTime Graphics (`smc `).
    Smc,
    /// Apple Video (`rpza`).
    Rpza,
    /// Flash Screen Video (FLV codec 3).
    FlashSv1,
    /// Flash Screen Video 2 (FLV codec 6).
    FlashSv2,
    /// CD+G karaoke graphics (`.cdg`).
    Cdg,
    /// PlayStation MDEC pictures (`.str`).
    Mdec,
}

impl Classic {
    /// The in-tree decoder's codec.
    pub fn codec(self) -> specular_oldvideo::Codec {
        match self {
            Classic::QtRle => specular_oldvideo::Codec::QtRle,
            Classic::Cinepak => specular_oldvideo::Codec::Cinepak,
            Classic::MsVideo1 => specular_oldvideo::Codec::MsVideo1,
            Classic::MsRle => specular_oldvideo::Codec::MsRle,
            Classic::Smc => specular_oldvideo::Codec::Smc,
            Classic::Rpza => specular_oldvideo::Codec::Rpza,
            Classic::FlashSv1 => specular_oldvideo::Codec::FlashSv1,
            Classic::FlashSv2 => specular_oldvideo::Codec::FlashSv2,
            Classic::Cdg => specular_oldvideo::Codec::Cdg,
            Classic::Mdec => specular_oldvideo::Codec::Mdec,
        }
    }
}

impl Codec {
    /// Short display name ("H.264", "HEVC", "AV1", a known family such as "ProRes", or the
    /// sample entry code).
    pub fn name(self) -> String {
        match self {
            Codec::H264 => "H.264".into(),
            Codec::Hevc => "HEVC".into(),
            Codec::Av1 => "AV1".into(),
            Codec::Vp8 => "VP8".into(),
            Codec::Vp9 => "VP9".into(),
            Codec::JpegXs => "JPEG XS".into(),
            Codec::Mpeg2 => "MPEG-2".into(),
            Codec::Mpeg1 => "MPEG-1".into(),
            Codec::Mpeg4 => "MPEG-4 Part 2".into(),
            Codec::H263 => "H.263".into(),
            Codec::Theora => "Theora".into(),
            Codec::ProRes => "ProRes".into(),
            Codec::Mjpeg => "Motion JPEG".into(),
            Codec::Jpeg2000 => "JPEG 2000".into(),
            Codec::Dv => "DV".into(),
            Codec::Vc1 => "VC-1".into(),
            Codec::Vp3 => "VP3".into(),
            Codec::Flv1 => "Sorenson Spark".into(),
            Codec::Ffv1 => "FFV1".into(),
            Codec::Raw => "uncompressed".into(),
            Codec::Classic(c) => c.codec().name().into(),
            Codec::CineForm => "CineForm".into(),
            Codec::Hap(k) => k.name().into(),
            Codec::Vc2 => "VC-2 (Dirac)".into(),
            Codec::Apv => "APV".into(),
            Codec::Other(c) => match &c {
                b"apcn" | b"apch" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => "ProRes".into(),
                b"aprn" | b"aprh" => "ProRes RAW".into(),
                b"AVdn" | b"AVdh" => "Avid DNxHD".into(),
                b"mjp2" => "JPEG 2000".into(),
                b"AVCI" => "AVC-Intra".into(),
                b"mxfv" => "unknown MXF picture coding".into(),
                b"mp4v" | b"xvid" | b"XVID" | b"DIVX" | b"divx" | b"FMP4" => "MPEG-4 Part 2".into(),
                b"s263" | b"h263" | b"H263" => "H.263".into(),
                b"MP43" => "Microsoft MPEG-4 (DivX 3)".into(),
                b"dvc " => "DV".into(),
                b"vp08" => "VP8".into(),
                b"vp09" => "VP9".into(),
                b"jpeg" | b"mjpa" | b"mjpb" | b"AVDJ" => "Motion JPEG".into(),
                b"dvh1" | b"dvhe" => "Dolby Vision".into(),
                b"mp2v" | b"xdvc" | b"xd5c" => "MPEG-2".into(),
                b"mp1v" => "MPEG-1".into(),
                b"theo" => "Theora".into(),
                b"RV40" => "RealVideo".into(),
                b"VP6F" | b"VP6A" | b"VP60" | b"VP61" | b"VP62" => "VP6".into(),
                b"WMV2" => "Windows Media Video 7/8".into(),
                b"raw " => "uncompressed".into(),
                b"encr" => "encrypted".into(),
                b"cmpr" => "compressed Matroska".into(),
                b"cLZO" => "LZO-compressed Matroska".into(),
                b"????" => "unknown".into(),
                _ => String::from_utf8_lossy(&c).trim().to_owned(),
            },
        }
    }
}

/// The ADPCM layouts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdpcmKind {
    /// Microsoft IMA ADPCM blocks (AVI, WAV, ASF; WAVE format tag 0x11).
    ImaWav,
    /// CD-ROM XA ADPCM (PlayStation `.str`, `.xa`): 128-byte sound groups of 4-bit (or
    /// 8-bit) samples.
    Xa { eight: bool },
    /// QuickTime `ima4`: 34-byte packets of 64 samples per channel.
    ImaQt,
    /// Microsoft ADPCM blocks (WAVE format tag 2) with the predictor coefficient pairs.
    Ms(Vec<[i16; 2]>),
}

/// The seven coefficient pairs every Microsoft ADPCM stream carries (×256).
pub const MS_COEFFS: [[i16; 2]; 7] = [
    [256, 0],
    [512, -256],
    [0, 0],
    [192, 64],
    [240, 0],
    [460, -208],
    [392, -232],
];

/// The audio coding of a track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioCodec {
    /// MPEG-4 AAC (`mp4a`, object type 0x40 or MPEG-2 AAC 0x66‥0x68) with its
    /// AudioSpecificConfig.
    Aac(Vec<u8>),
    /// Apple Lossless with its 24-byte `ALACSpecificConfig`.
    Alac(Vec<u8>),
    /// Opus with its identification header (`OpusHead`, RFC 7845 §5.1).
    Opus(Vec<u8>),
    /// Vorbis with its three headers, Xiph-laced (Matroska's `CodecPrivate`).
    Vorbis(Vec<u8>),
    /// AC-3 (ATSC A/52; MP4 `ac-3`, Matroska `A_AC3`, DVD and transport streams).
    Ac3,
    /// E-AC-3 (A/52 Annex E; MP4 `ec-3`, Matroska `A_EAC3`).
    Eac3,
    /// FLAC with what the container stores ahead of the frames (Matroska's `fLaC` and
    /// metadata blocks, MP4's `dfLa` body, Ogg FLAC's first packet), or nothing.
    Flac(Vec<u8>),
    /// MPEG audio (MPEG-1/2/2.5 Layer I, II or III: the layer when known, else 0); Layer
    /// III is MP3.
    Mpa(u8),
    /// Uncompressed PCM (`sowt`, `twos`, `lpcm`, `in24`, `in32`, `fl32`, `fl64`, `raw `,
    /// ISO `ipcm` / `fpcm`): bits per sample, integer or float, byte order. Integer samples
    /// are signed except 8-bit ones marked little-endian, which are unsigned (`raw `).
    Pcm {
        bits: u16,
        float: bool,
        little_endian: bool,
    },
    /// G.711 telephone sound, one byte a sample (`ulaw` / `alaw`, WAVE tags 7 and 6).
    G711 { alaw: bool },
    /// IMA or Microsoft ADPCM in blocks of `block` bytes (QuickTime `ima4` packets are 34
    /// bytes a channel whatever `block` says).
    Adpcm { kind: AdpcmKind, block: u16 },
    /// DV sound, inside the DV frames themselves (raw DV, type-1 AVI): each sample is a
    /// whole frame.
    Dv,
    /// WavPack (`.wv`): the stream's channels, stored bits, hybrid and float flags; for DSD
    /// (1-bit) streams, played as PCM, the 1-bit rate and samples a channel (when known).
    WavPack {
        channels: u16,
        bits: u16,
        hybrid: bool,
        float: bool,
        dsd: Option<(u32, Option<u64>)>,
    },
    /// Monkey's Audio (`.ape`): bits per sample and compression level (1000…5000).
    Ape { bits: u16, level: u16 },
    /// Speex with its Ogg header (80 bytes); empty in FLV, where it is wideband mono.
    Speex(Vec<u8>),
    /// Anything else: the sample entry and, for `mp4a`, the object type.
    Other([u8; 4], u8),
}

impl AudioCodec {
    /// Short display name.
    pub fn name(&self) -> String {
        match self {
            AudioCodec::Aac(_) => "AAC".into(),
            AudioCodec::Alac(_) => "ALAC".into(),
            AudioCodec::Opus(_) => "Opus".into(),
            AudioCodec::Vorbis(_) => "Vorbis".into(),
            AudioCodec::Ac3 => "AC-3".into(),
            AudioCodec::Eac3 => "E-AC-3".into(),
            AudioCodec::Flac(_) => "FLAC".into(),
            AudioCodec::WavPack {
                dsd: Some((rate, samples)),
                ..
            } => {
                let mut s = format!(
                    "WavPack {} ({} MHz",
                    specular_wavpack::dsd::name(*rate),
                    f64::from(*rate) / 1e6
                );
                if let Some(n) = samples {
                    s += &format!(", {n} samples");
                }
                s + ")"
            }
            AudioCodec::WavPack {
                hybrid: true,
                float,
                ..
            } => format!("WavPack hybrid{}", if *float { " float" } else { "" }),
            AudioCodec::WavPack { float: true, .. } => "WavPack float".into(),
            AudioCodec::WavPack { .. } => "WavPack".into(),
            AudioCodec::Ape { level, .. } => match level {
                1000 => "Monkey's Audio (fast)".into(),
                2000 => "Monkey's Audio (normal)".into(),
                3000 => "Monkey's Audio (high)".into(),
                4000 => "Monkey's Audio (extra high)".into(),
                _ => "Monkey's Audio (insane)".into(),
            },
            AudioCodec::Speex(h) => match specular_speex::Header::parse(h).map(|h| h.mode) {
                Ok(0) => "Speex narrowband".into(),
                Ok(2) => "Speex ultra-wideband".into(),
                _ => "Speex wideband".into(),
            },
            AudioCodec::Mpa(1) => "MP1".into(),
            AudioCodec::Mpa(2) => "MP2".into(),
            AudioCodec::Mpa(3) => "MP3".into(),
            AudioCodec::Mpa(_) => "MPEG audio".into(),
            AudioCodec::Pcm { .. } => "PCM".into(),
            AudioCodec::Dv => "DV PCM".into(),
            AudioCodec::G711 { alaw: false } => "G.711 µ-law".into(),
            AudioCodec::G711 { alaw: true } => "G.711 A-law".into(),
            AudioCodec::Adpcm {
                kind: AdpcmKind::Ms(_),
                ..
            } => "Microsoft ADPCM".into(),
            AudioCodec::Adpcm {
                kind: AdpcmKind::Xa { .. },
                ..
            } => "CD-ROM XA ADPCM".into(),
            AudioCodec::Adpcm { .. } => "IMA ADPCM".into(),
            AudioCodec::Other(c, ot) => match (c, ot) {
                (b"WMA ", 3) => "Windows Media Audio Professional".into(),
                (b"WMA ", 4) => "Windows Media Audio Lossless".into(),
                (b"WMA ", _) => "Windows Media Audio".into(),
                (b"WMAV", _) => "Windows Media Audio Voice".into(),
                (b"nmos", _) => "Nellymoser".into(),
                (b"spex", _) => "Speex".into(),
                (b"swfa", _) => "Flash ADPCM".into(),
                (b"mp4a", 0x69 | 0x6B) | (b".mp3", _) => "MP3".into(),
                (b"Opus", _) => "Opus".into(),
                (b"fLaC", _) => "FLAC".into(),
                (b"ac-3", _) => "AC-3".into(),
                (b"ec-3", _) => "E-AC-3".into(),
                (b"ac-4", _) => "AC-4".into(),
                (b"samr" | b"sawb", m) => crate::amr_sound::name(c, *m),
                (b"MPC7" | b"MPC8", p) => crate::mpc_sound::name(c, *p),
                (b"TTA1", bits) => format!("TTA ({bits}-bit)"),
                (b"ajkg", v) => format!("Shorten (version {v})"),
                (b"DSF " | b"DFF ", s) => crate::dsd_files::name(*s),
                (b"8SVX", f) => crate::pcm_files::svx_name(*f),
                (b"trkr", k) => crate::tracker_sound::name(*k),
                (b"gsm ", _) => "GSM 06.10".into(),
                (b"qclp", _) => "QCELP (PureVoice)".into(),
                (b"sevc", _) => "EVRC".into(),
                (b"ssmv", _) => "SMV".into(),
                (b"qoaf", _) => "QOA".into(),
                (b"midi" | b"bank", k) => crate::synth_sound::name(c, *k),
                (
                    b"lpcm" | b"sowt" | b"twos" | b"in24" | b"in32" | b"fl32" | b"fl64" | b"raw ",
                    _,
                ) => "PCM".into(),
                (b"ulaw" | b"alaw", _) => "G.711".into(),
                (b"dtsc", _) => "DTS".into(),
                (b"dtsh", _) => "DTS-HD High Resolution".into(),
                (b"dtsl", _) => "DTS-HD Master Audio".into(),
                (b"dtse", _) => "DTS Express".into(),
                (b"dtsx" | b"dtsy", _) => "DTS:X".into(),
                (b"mlpa", _) => "Dolby TrueHD".into(),
                (b"mlp ", _) => "MLP".into(),
                (b".mp2", _) => "MP2".into(),
                (b"DVDL", b) => format!("{b}-bit DVD LPCM"),
                (b"cook", _) => "RealAudio".into(),
                (b"encr", _) => "encrypted".into(),
                (b"cmpr", _) => "compressed Matroska".into(),
                (b"cLZO", _) => "LZO-compressed Matroska".into(),
                (b"mxfa", _) => "compressed MXF".into(),
                (b"????", _) => "unknown".into(),
                _ => String::from_utf8_lossy(c).trim().to_owned(),
            },
        }
    }
}

/// `colr` `nclx` / `nclc` colour signalling of a sample entry (H.273 code points).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour {
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}

/// One coded frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Byte offset of the sample from the start of the movie.
    pub offset: u64,
    pub size: u32,
    /// Decode time (media timescale).
    pub dts: u64,
    /// Composition (presentation) time: `dts` plus the `ctts` offset.
    pub cts: i64,
    /// Duration from `stts` (media timescale).
    pub duration: u32,
    /// A sync sample (`stss`; every sample when the table is absent).
    pub sync: bool,
}

/// What a track's edit list says about its presentation: shown from media time `start`
/// (its timescale), after an empty `delay_us` at the start of the movie.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    pub start: i64,
    pub delay_us: u64,
    /// The presented duration of the first non-empty edit, in microseconds.
    pub length_us: Option<u64>,
}

/// The first video track of a movie.
#[derive(Clone, Debug, Default)]
pub struct VideoTrack {
    pub codec: Option<Codec>,
    /// The decoder configuration record payload (`avcC` / `hvcC` / `av1C` without its box
    /// header).
    pub config: Vec<u8>,
    /// Coded size from the sample entry.
    pub width: u32,
    pub height: u32,
    /// Presentation size from `tkhd` (before rotation).
    pub display: (u32, u32),
    /// Clockwise rotation to apply for display, from the `tkhd` matrix: 0, 90, 180 or 270.
    pub rotation: u16,
    /// The `tkhd` matrix mirrors the picture (left and right swapped before the rotation),
    /// as some front cameras and editors write it.
    pub mirror: bool,
    pub timescale: u32,
    /// Media duration in `timescale` units.
    pub duration: u64,
    pub sample_count: u32,
    /// The sample table, in decode order (empty unless parsed with samples).
    pub samples: Vec<Sample>,
    pub colour: Option<Colour>,
    /// Pixel aspect ratio (`pasp`: horizontal and vertical spacing), when not square.
    pub pixel_aspect: Option<(u32, u32)>,
    /// QuickTime `fiel` says the frames are interlaced (two fields).
    pub interlaced: bool,
    /// The container says the bottom field comes first (QuickTime `fiel`, DV).
    pub bottom_first: bool,
    /// The samples' times are slots, not presentation times (AVI, Matroska VFW): a codec
    /// with B-frames shows its pictures in display order in them.
    pub slot_timing: bool,
    pub edit: Edit,
    /// Where the samples' bytes are.
    pub layout: Layout,
}

impl VideoTrack {
    /// The EXIF orientation that shows the picture upright: the mirror, then the rotation.
    pub fn orientation(&self) -> u8 {
        specular_core::Orientation::from_rotation_flip((self.rotation / 90 % 4) as u8, self.mirror)
            .0
    }

    /// The coded bytes of sample `s` of this track in the movie's bytes `data`.
    pub fn sample_data<'a>(&self, data: &'a [u8], s: &Sample) -> Option<Cow<'a, [u8]>> {
        self.layout.read(data, s)
    }

    /// Media duration in microseconds.
    pub fn duration_us(&self) -> Option<u64> {
        (self.timescale > 0)
            .then(|| (self.duration as u128 * 1_000_000 / self.timescale as u128) as u64)
    }

    /// Microseconds of a media time.
    pub fn us(&self, t: i64) -> i64 {
        ticks_to_us(t, self.timescale)
    }

    /// Displayed width and height before rotation: the coded size stretched horizontally by
    /// the pixel aspect ratio.
    pub fn shown_size(&self) -> (u32, u32) {
        match self.pixel_aspect {
            Some((h, v)) if h > 0 && v > 0 && h != v => {
                let w = (u64::from(self.width) * u64::from(h) / u64::from(v)).clamp(1, 1 << 16);
                (w as u32, self.height)
            }
            _ => (self.width, self.height),
        }
    }
}

/// The first audio track of a movie.
#[derive(Clone, Debug)]
pub struct AudioTrack {
    pub codec: AudioCodec,
    pub channels: u16,
    pub sample_rate: u32,
    pub timescale: u32,
    pub duration: u64,
    pub sample_count: u32,
    /// The sample table (empty unless parsed with samples).
    pub samples: Vec<Sample>,
    pub edit: Edit,
    /// Where the samples' bytes are.
    pub layout: Layout,
    /// Sound kept apart from the movie's own bytes (MXF).
    pub extra: SoundExtra,
}

/// Another file's bytes: an OP-Atom MXF sound file beside the picture's.
#[derive(Clone)]
pub struct External(pub std::sync::Arc<specular_core::Source>);

impl fmt::Debug for External {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "External({})", self.0.path().display())
    }
}

/// What a sound track has besides its samples in the movie's bytes.
#[derive(Clone, Debug, Default)]
pub struct SoundExtra {
    /// The file the samples are in, when not the movie's.
    pub source: Option<External>,
    /// Mono tracks played with this one as its further channels (MXF's one track per
    /// channel), sample `k` beside sample `k`.
    pub companions: Vec<AudioTrack>,
    /// How many sound tracks the movie has (0 when not counted: one).
    pub tracks: usize,
    /// A WavPack hybrid file's correction file (`.wvc` beside it): played lossless with it.
    pub correction: Option<External>,
    /// The codec's header kept apart from the samples, for codecs whose type carries none
    /// (Musepack SV8 in Matroska: the stream header packets).
    pub head: Vec<u8>,
    /// The SoundFont a MIDI song plays through (found beside it or installed).
    pub bank: Option<External>,
}

impl AudioTrack {
    /// Microseconds of a media time.
    pub fn us(&self, t: i64) -> i64 {
        ticks_to_us(t, self.timescale)
    }

    /// The coded bytes of sample `s` of this track in the movie's bytes `data`.
    pub fn sample_data<'a>(&self, data: &'a [u8], s: &Sample) -> Option<Cow<'a, [u8]>> {
        self.layout.read(data, s)
    }
}

/// Where a track's samples' bytes are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// Each sample is `size` bytes at `offset` (MP4, Matroska).
    #[default]
    Contiguous,
    /// Spread over MPEG transport stream packets (see [`crate::ts`]).
    Ts(crate::ts::TsLayout),
    /// Spread over MPEG programme stream packets (see [`crate::ps`]).
    Ps(crate::ps::PsLayout),
    /// Ogg packets, possibly across pages (see [`crate::ogg`]).
    Ogg(crate::ogg::OggLayout),
    /// Each sample is one H.264 / HEVC access unit in Annex B (MXF): start codes become
    /// 4-byte lengths.
    AnnexB,
    /// Pieces of media objects in ASF packets (see [`crate::asf`]).
    Asf(crate::asf::AsfLayout),
    /// Each sample is a D-10 sound element (MXF), read as 32-bit PCM of this many channels
    /// (see [`crate::mxf::d10_sound`]).
    D10Sound(u8),
    /// Each sample is `size` bytes at `offset`, stored compressed (Matroska
    /// `ContentEncodings`; see [`crate::mkv::MkvLayout`]).
    Mkv(crate::mkv::MkvLayout),
    /// Data packets in a WTV recording's timeline (see [`crate::wtv`]).
    Wtv(crate::wtv::WtvLayout),
    /// A PlayStation movie frame over its CD-ROM sectors (see [`crate::psxstr`]).
    Str(crate::psxstr::StrLayout),
}

impl Layout {
    /// The coded bytes of sample `s` in the movie's bytes `data`: borrowed when they lie in
    /// one piece, gathered (and for Annex B video, length-prefixed) otherwise.
    pub fn read<'a>(&self, data: &'a [u8], s: &Sample) -> Option<Cow<'a, [u8]>> {
        match self {
            Layout::Contiguous => sample_bytes(data, s).map(Cow::Borrowed),
            Layout::Ts(t) => t.read(data, s).map(Cow::Owned),
            Layout::Ps(p) => p.read(data, s).map(Cow::Owned),
            Layout::Ogg(o) => o.read(data, s),
            Layout::AnnexB => {
                sample_bytes(data, s).map(|b| Cow::Owned(crate::ts::length_prefixed(b)))
            }
            Layout::D10Sound(ch) => crate::mxf::d10_sound(data, s, *ch).map(Cow::Owned),
            Layout::Asf(a) => a.read(data, s),
            Layout::Mkv(m) => m.read(sample_bytes(data, s)?),
            Layout::Wtv(w) => w.read(data, s),
            Layout::Str(l) => l.read(data, s).map(Cow::Owned),
        }
    }
}

fn ticks_to_us(t: i64, timescale: u32) -> i64 {
    if timescale == 0 {
        return 0;
    }
    (t as i128 * 1_000_000 / timescale as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

/// What a movie holds.
#[derive(Clone, Debug, Default)]
pub struct Movie {
    /// `ftyp` major brand (`qt  ` for QuickTime; zero when there is no `ftyp`).
    pub brand: [u8; 4],
    /// Movie timescale (`mvhd`), the unit of edit list durations.
    pub timescale: u32,
    /// Movie duration (`mvhd`) in microseconds.
    pub duration_us: Option<u64>,
    /// `mvhd` creation time in seconds since 1970 (UTC by the standard, local time from some
    /// cameras); `None` when unset.
    pub created: Option<i64>,
    pub video: Option<VideoTrack>,
    pub has_audio: bool,
    /// The first audio track (also when its codec is not one we decode).
    pub audio: Option<AudioTrack>,
    /// Apple's `com.apple.quicktime.content.identifier` (pairs a Live Photo's movie and still).
    pub content_id: Option<String>,
    /// The recording date as written: Apple's `com.apple.quicktime.creationdate` or `©day`
    /// (ISO 8601, usually with a zone offset).
    pub date: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    /// The XMP packet (`udta/XMP_` or the XMP `uuid` box).
    pub xmp: Option<Vec<u8>>,
    /// Every text tag as (name, value), in file order: QuickTime `keys` names
    /// ("com.apple.quicktime.location.ISO6709"), item codes ("©nam", "©xyz"), Matroska
    /// simple tag names ("TITLE").
    pub tags: Vec<(String, String)>,
}

impl Movie {
    /// Duration: the video track's (its edit when trimmed), else the movie header's.
    pub fn duration_us(&self) -> Option<u64> {
        self.video
            .as_ref()
            .and_then(|v| {
                let media = v.duration_us();
                match (v.edit.length_us, media) {
                    (Some(e), Some(m)) => Some(e.min(m)),
                    (e, m) => m.or(e),
                }
            })
            .filter(|&d| d > 0)
            .or(self.duration_us)
    }
}

/// A box: its type, payload and the offset of the payload in the parent slice.
#[derive(Clone, Copy)]
pub(crate) struct Bx<'a> {
    pub(crate) kind: [u8; 4],
    pub(crate) body: &'a [u8],
}

/// Iterates the boxes of a payload. A box whose size runs past the end is an error (the last
/// box may say size 0: "to the end").
pub(crate) struct Boxes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for Boxes<'a> {
    type Item = Result<Bx<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let rest = self.data.get(self.pos..)?;
        if rest.is_empty() {
            return None;
        }
        // Fewer than 8 trailing bytes: padding some writers leave (QuickTime's 4 zero bytes).
        if rest.len() < 8 {
            self.pos = self.data.len();
            return None;
        }
        let size32 = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as u64;
        let kind = [rest[4], rest[5], rest[6], rest[7]];
        let (size, head) = match size32 {
            0 => (rest.len() as u64, 8),
            1 => {
                let Some(b) = rest.get(8..16) else {
                    self.pos = self.data.len();
                    return Some(Err(bad("truncated 64-bit box size")));
                };
                (u64::from_be_bytes(b.try_into().unwrap_or_default()), 16)
            }
            s => (s, 8),
        };
        if size < head as u64 || size > rest.len() as u64 {
            self.pos = self.data.len();
            return Some(Err(bad(format!(
                "box '{}' of {size} bytes in {} remaining",
                String::from_utf8_lossy(&kind),
                rest.len()
            ))));
        }
        self.pos += size as usize;
        Some(Ok(Bx {
            kind,
            body: &rest[head..size as usize],
        }))
    }
}

pub(crate) fn boxes(data: &[u8]) -> Boxes<'_> {
    Boxes { data, pos: 0 }
}

/// The first child box of type `kind`.
pub(crate) fn child<'a>(data: &'a [u8], kind: &[u8; 4]) -> Result<Option<&'a [u8]>> {
    for b in boxes(data) {
        let b = b?;
        if &b.kind == kind {
            return Ok(Some(b.body));
        }
    }
    Ok(None)
}

/// Big-endian reads that fail cleanly past the end.
#[derive(Clone, Copy)]
pub(crate) struct Rd<'a>(pub(crate) &'a [u8]);

impl Rd<'_> {
    pub(crate) fn u8(&self, o: usize) -> Result<u8> {
        self.0.get(o).copied().ok_or_else(|| bad("truncated box"))
    }
    pub(crate) fn u16(&self, o: usize) -> Result<u16> {
        let b = self.0.get(o..o + 2).ok_or_else(|| bad("truncated box"))?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub(crate) fn u32(&self, o: usize) -> Result<u32> {
        let b = self.0.get(o..o + 4).ok_or_else(|| bad("truncated box"))?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub(crate) fn u64(&self, o: usize) -> Result<u64> {
        let b = self.0.get(o..o + 8).ok_or_else(|| bad("truncated box"))?;
        Ok(u64::from_be_bytes(b.try_into().unwrap_or_default()))
    }
    /// A full box's entry count at `o` with entries of `size` bytes after it: checked against
    /// the bytes present.
    fn count(&self, o: usize, size: usize) -> Result<usize> {
        let n = self.u32(o)? as usize;
        let room = self.0.len().saturating_sub(o + 4) / size.max(1);
        if n > room {
            return Err(bad(format!("{n} entries in room for {room}")));
        }
        Ok(n)
    }
}

/// Whether the four bytes look like a box type (printable ASCII or QuickTime's `©`).
fn is_fourcc(b: &[u8]) -> bool {
    b.len() == 4 && b.iter().all(|&c| (0x20..0x7f).contains(&c) || c == 0xA9)
}

/// Parses a movie. `samples`: also build the sample tables (otherwise only the headers are
/// read, which is what the info panel needs: microseconds even for long clips).
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    parse_with(data, samples, None)
}

/// A track's `track_ID` (`tkhd`).
pub(crate) fn track_id(trak: &[u8]) -> Option<u32> {
    let tkhd = child(trak, b"tkhd").ok()??;
    let r = Rd(tkhd);
    r.u32(if r.u8(0).ok()? == 1 { 20 } else { 12 }).ok()
}

/// [`parse`] taking the sound track with `track_ID` `audio` when there is one, else the
/// first.
pub fn parse_with(data: &[u8], samples: bool, audio: Option<u32>) -> Result<Movie> {
    let mut m = Movie::default();
    let mut moov = None;
    for b in boxes(data) {
        let b = b?;
        match &b.kind {
            b"ftyp" => {
                if b.body.len() >= 4 {
                    m.brand.copy_from_slice(&b.body[..4]);
                }
            }
            b"moov" => {
                moov = Some(b.body);
                break;
            }
            b"moof" => return Err(Error::Unsupported("fragmented movie".into())),
            b"uuid" if m.xmp.is_none() && b.body.get(..16) == Some(&XMP_UUID[..]) => {
                m.xmp = Some(b.body[16..].to_vec());
            }
            _ => {}
        }
    }
    let moov = moov.ok_or_else(|| bad("no moov box"))?;
    if let Some(mvhd) = child(moov, b"mvhd")? {
        let r = Rd(mvhd);
        let (created, ts, dur) = if r.u8(0)? == 1 {
            (r.u64(4)?, r.u32(20)?, r.u64(24)?)
        } else {
            (r.u32(4)? as u64, r.u32(12)?, r.u32(16)? as u64)
        };
        m.timescale = ts;
        if ts > 0 && dur != u64::MAX && dur != u32::MAX as u64 {
            m.duration_us = Some((dur as u128 * 1_000_000 / ts as u128) as u64);
        }
        // Zero is "unset"; anything before 1970 or absurdly far ahead is a broken clock.
        let unix = (created.min(1 << 40) as i64) - EPOCH_1904;
        if created > 0 && created < 1 << 40 && unix > 0 {
            m.created = Some(unix);
        }
    }
    let mut items = Vec::new();
    for b in boxes(moov) {
        let b = b?;
        match &b.kind {
            b"trak" => {
                let want_video = m.video.is_none();
                let want_audio =
                    m.audio.is_none() && audio.is_none_or(|a| track_id(b.body) == Some(a));
                let t = trak(
                    b.body,
                    samples,
                    data.len() as u64,
                    m.timescale,
                    want_video,
                    want_audio,
                )?;
                match t {
                    Track::Video(v) => m.video = Some(*v),
                    Track::Audio(a) => {
                        m.has_audio = true;
                        if want_audio {
                            m.audio = a.map(|a| *a);
                        }
                    }
                    Track::Other => {}
                }
            }
            b"meta" => meta_items(b.body, 0, &mut items)?,
            b"udta" => udta(b.body, &mut items, &mut m.xmp)?,
            _ => {}
        }
    }
    let item = |keys: &[&str]| {
        keys.iter().find_map(|k| {
            items
                .iter()
                .find(|(key, v)| key == k && !v.is_empty())
                .map(|(_, v)| v.clone())
        })
    };
    if samples && let Some(v) = &mut m.video {
        poc_order(data, v);
    }
    m.content_id = item(&["com.apple.quicktime.content.identifier"]);
    m.date = item(&["com.apple.quicktime.creationdate", "©day"]);
    m.make = item(&["com.apple.quicktime.make", "©mak"]);
    m.model = item(&["com.apple.quicktime.model", "©mod"]);
    m.tags = items
        .into_iter()
        .filter(|(k, v)| k != "com.apple.quicktime.content.identifier" && !v.is_empty())
        .collect();
    if audio.is_some() && m.audio.is_none() {
        return parse_with(data, samples, None);
    }
    Ok(m)
}

/// H.264 without composition offsets (no `ctts`; some remuxers drop it even with B-frames):
/// the pictures' order counts give the presentation order, as other players show them.
fn poc_order(data: &[u8], v: &mut VideoTrack) {
    if v.codec != Some(Codec::H264)
        || v.samples.len() < 2
        || v.samples
            .iter()
            .any(|s| s.cts != s.dts.min(i64::MAX as u64) as i64)
    {
        return;
    }
    // A stream's web file still being fetched: reading every picture would wait for the
    // whole file. Its times are taken as slots instead: the decoder's pictures, which come
    // out in presentation order, take the samples' times in turn.
    // An edit starting a few frames in is the reorder delay the lost offsets carried
    // (muxers start the first presentation there): the presentation times keep it.
    let first = v.samples[0].duration.max(1) as i64;
    let delay = v.edit.start;
    let shift = if delay > 0 && delay <= 4 * first && delay % first == 0 {
        delay
    } else {
        0
    };
    if specular_core::fill::pending(data) {
        v.slot_timing = true;
        for s in &mut v.samples {
            s.cts += shift;
        }
        return;
    }
    let Some(units) = v
        .samples
        .iter()
        .map(|s| sample_bytes(data, s))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let Some(rank) = crate::flv::avc_poc_rank(&v.config, &units) else {
        return;
    };
    let dts: Vec<i64> = v.samples.iter().map(|s| s.cts).collect();
    let reordered = rank.iter().enumerate().any(|(i, &r)| r as usize != i);
    let shift = if reordered { shift } else { 0 };
    for (s, &r) in v.samples.iter_mut().zip(&rank) {
        s.cts = dts[r as usize] + shift;
    }
}

enum Track {
    Video(Box<VideoTrack>),
    /// A sound track; its details when they were wanted and readable.
    Audio(Option<Box<AudioTrack>>),
    Other,
}

/// The edit list of a track: leading empty edits become a delay, the first real edit's media
/// time the start.
pub(crate) fn edit_list(trak: &[u8], movie_ts: u32) -> Result<Edit> {
    let mut e = Edit::default();
    let Some(elst) = child(trak, b"edts")?
        .map(|d| child(d, b"elst"))
        .transpose()?
        .flatten()
    else {
        return Ok(e);
    };
    let r = Rd(elst);
    let v1 = r.u8(0)? == 1;
    let size = if v1 { 20 } else { 12 };
    let n = r.count(4, size)?;
    let mut delay = 0u64;
    for i in 0..n.min(64) {
        let o = 8 + i * size;
        let (dur, time) = if v1 {
            (r.u64(o)?, r.u64(o + 8)? as i64)
        } else {
            (r.u32(o)? as u64, r.u32(o + 4)? as i32 as i64)
        };
        if time == -1 {
            delay = delay.saturating_add(dur);
            continue;
        }
        e.start = time.max(0);
        e.delay_us = ticks_to_us(delay.min(i64::MAX as u64) as i64, movie_ts).max(0) as u64;
        if dur > 0 && movie_ts > 0 {
            e.length_us = Some(ticks_to_us(dur.min(i64::MAX as u64) as i64, movie_ts) as u64);
        }
        break;
    }
    Ok(e)
}

fn trak(
    body: &[u8],
    samples: bool,
    movie_len: u64,
    movie_ts: u32,
    want_video: bool,
    want_audio: bool,
) -> Result<Track> {
    let Some(mdia) = child(body, b"mdia")? else {
        return Ok(Track::Other);
    };
    let handler = match child(mdia, b"hdlr")? {
        Some(h) if h.len() >= 12 => [h[8], h[9], h[10], h[11]],
        _ => return Ok(Track::Other),
    };
    match &handler {
        b"vide" if want_video => {}
        b"soun" if !want_audio => return Ok(Track::Audio(None)),
        b"soun" => {}
        _ => return Ok(Track::Other),
    }
    let audio = &handler == b"soun";
    let (timescale, duration) = match child(mdia, b"mdhd")? {
        Some(mdhd) => {
            let r = Rd(mdhd);
            if r.u8(0)? == 1 {
                (r.u32(20)?, r.u64(24)?)
            } else {
                (r.u32(12)?, r.u32(16)? as u64)
            }
        }
        None => (0, 0),
    };
    let edit = edit_list(body, movie_ts).unwrap_or_default();
    let Some(stbl) = child(mdia, b"minf")?
        .map(|m| child(m, b"stbl"))
        .transpose()?
        .flatten()
    else {
        if audio {
            return Ok(Track::Audio(None));
        }
        return Err(bad("video track without a sample table"));
    };
    let mut tables = Tables::default();
    let mut stsd = None;
    for b in boxes(stbl) {
        let b = b?;
        match &b.kind {
            b"stsd" => stsd = Some(b.body),
            b"stts" => tables.stts = Some(b.body),
            b"ctts" => tables.ctts = Some(b.body),
            b"stss" => tables.stss = Some(b.body),
            b"stsz" => tables.stsz = Some((b.body, false)),
            b"stz2" => tables.stsz = Some((b.body, true)),
            b"stsc" => tables.stsc = Some(b.body),
            b"stco" => tables.stco = Some((b.body, false)),
            b"co64" => tables.stco = Some((b.body, true)),
            _ => {}
        }
    }
    if audio {
        // A sound track that cannot be read still says the movie has sound.
        let n = tables
            .stsz
            .and_then(|(z, _)| Rd(z).u32(8).ok())
            .map_or(0, |n| n as usize);
        let Some(mut a) = stsd.and_then(|s| audio_entry(s).ok().flatten()) else {
            return Ok(Track::Audio(None));
        };
        a.timescale = timescale;
        a.duration = duration;
        a.sample_count = n.min(MAX_SAMPLES) as u32;
        a.edit = edit;
        let ch = usize::from(a.channels.max(1));
        let packets = match a.codec {
            AudioCodec::Pcm { bits, .. } => Some((1, usize::from(bits / 8) * ch)),
            AudioCodec::G711 { .. } => Some((1, ch)),
            // QuickTime counts `ima4` frames (sample size 1); ffmpeg counts packets, as
            // ordinary samples.
            AudioCodec::Adpcm {
                kind: AdpcmKind::ImaQt,
                ..
            } if tables.stsz.and_then(|(z, _)| Rd(z).u32(4).ok()) != Some(34 * ch as u32) => {
                Some((64, 34 * ch))
            }
            // QuickTime `Qclp` (sound description 1) counts PCM frames too: 160 a 35-byte
            // packet slot, at the entry's rate (Apple coded 22 050 Hz sound this way).
            AudioCodec::Other(ref t, _)
                if t == b"qclp" && tables.stsz.and_then(|(z, _)| Rd(z).u32(4).ok()) == Some(1) =>
            {
                Some((160, 35))
            }
            _ => None,
        };
        if let Some((frames, bytes)) = packets {
            // One "sample" per audio frame: read whole chunks instead.
            if samples {
                match tables.pcm_chunks(frames, bytes, movie_len) {
                    Ok(s) => a.samples = s,
                    Err(_) => return Ok(Track::Audio(None)),
                }
            }
            return Ok(Track::Audio(Some(Box::new(a))));
        }
        if samples {
            if n > MAX_SAMPLES {
                return Ok(Track::Audio(None));
            }
            match tables.expand(n, movie_len) {
                Ok(s) => a.samples = s,
                Err(_) => return Ok(Track::Audio(None)),
            }
        }
        return Ok(Track::Audio(Some(Box::new(a))));
    }
    let (stsz, _) = tables.stsz.ok_or_else(|| bad("no stsz"))?;
    let n = Rd(stsz).u32(8)? as usize;
    if n > MAX_SAMPLES {
        return Err(Error::Unsupported(format!("{n} samples")));
    }
    let mut v = VideoTrack {
        timescale,
        duration,
        edit,
        sample_count: n as u32,
        ..VideoTrack::default()
    };
    if let Some(tkhd) = child(body, b"tkhd")? {
        let r = Rd(tkhd);
        let base = if r.u8(0)? == 1 { 52 } else { 40 };
        let fix = |o: usize| -> Result<i32> { Ok(r.u32(o)? as i32) };
        let (a, b, c, d) = (fix(base)?, fix(base + 4)?, fix(base + 12)?, fix(base + 16)?);
        (v.rotation, v.mirror) = matrix_turn(a, b, c, d);
        v.display = (r.u32(base + 36)? >> 16, r.u32(base + 40)? >> 16);
    }
    if let Some(stsd) = stsd {
        sample_entry(stsd, &mut v)?;
    }
    if samples {
        v.samples = tables.expand(n, movie_len)?;
        if v.samples.len() < n {
            // Cut short: the movie is as long as what is there.
            v.sample_count = v.samples.len() as u32;
            v.duration = v
                .samples
                .last()
                .map_or(0, |s| s.dts.saturating_add(u64::from(s.duration)));
        }
    }
    Ok(Track::Video(Box::new(v)))
}

/// Reads the first sample entry of `stsd`: codec, coded size, configuration record, colour,
/// pixel aspect and field order.
fn sample_entry(stsd: &[u8], v: &mut VideoTrack) -> Result<()> {
    let Some(entry) = stsd.get(8..).and_then(|e| boxes(e).next()) else {
        return Err(bad("empty stsd"));
    };
    let entry = entry?;
    let r = Rd(entry.body);
    v.width = r.u16(24)? as u32;
    v.height = r.u16(26)? as u32;
    let (codec, cfg) = match &entry.kind {
        b"avc1" | b"avc3" => (Codec::H264, b"avcC"),
        // QuickTime AVC-Intra entries (`ai5p`, `ai1q`, `aivx`...): H.264 when they carry an
        // `avcC`; Panasonic's leave the parameter sets out.
        k if k.starts_with(b"ai") => (Codec::H264, b"avcC"),
        b"hvc1" | b"hev1" => (Codec::Hevc, b"hvcC"),
        b"av01" => (Codec::Av1, b"av1C"),
        b"vp08" => (Codec::Vp8, b"vpcC"),
        b"vp09" => (Codec::Vp9, b"vpcC"),
        b"jxsm" => (Codec::JpegXs, b"jxpl"),
        b"mp4v" => (Codec::Mpeg4, b"esds"),
        b"s263" | b"h263" | b"H263" => (Codec::H263, b"\0\0\0\0"),
        // QuickTime MPEG-2: XDCAM, XDCAM HD/EX, HDV, IMX, and plain MPEG-2 entries.
        k if k.starts_with(b"xdv")
            || k.starts_with(b"xd5")
            || k.starts_with(b"hdv")
            || k.starts_with(b"mx")
            || matches!(
                k,
                b"xdhd" | b"xdh2" | b"mp2v" | b"m2v1" | b"mpg2" | b"MPG2" | b"AVmp"
            ) =>
        {
            (Codec::Mpeg2, b"\0\0\0\0")
        }
        b"mp1v" | b"m1v " | b"mpg1" => (Codec::Mpeg1, b"\0\0\0\0"),
        b"jpeg" | b"mjpa" | b"mjpb" | b"AVDJ" | b"dmb1" => (Codec::Mjpeg, b"\0\0\0\0"),
        b"mjp2" => (Codec::Jpeg2000, b"\0\0\0\0"),
        b"dvc " | b"dvcp" | b"dvpp" | b"dv5n" | b"dv5p" | b"dvh5" | b"dvh6" | b"dvhq" | b"dvhp"
        | b"dvh2" | b"dvh3" => (Codec::Dv, b"\0\0\0\0"),
        b"vc-1" => (Codec::Vc1, b"dvc1"),
        k if crate::prores::is_prores(k) => {
            // The four-character code names the profile.
            v.config = k.to_vec();
            (Codec::ProRes, b"\0\0\0\0")
        }
        b"FFV1" => (Codec::Ffv1, b"glbl"),
        b"CFHD" => (Codec::CineForm, b"\0\0\0\0"),
        b"drac" => (Codec::Vc2, b"\0\0\0\0"),
        b"apv1" => (Codec::Apv, b"\0\0\0\0"),
        k if specular_hap::kind_of_fourcc(k).is_some() => (
            Codec::Hap(specular_hap::kind_of_fourcc(k).unwrap_or(specular_hap::Kind::Hap)),
            b"\0\0\0\0",
        ),
        k if crate::rawvid::codec_of(*k).is_some() => {
            // Raw video and the old codecs: the depth and colour table describe the pictures.
            v.config = crate::rawvid::PixDesc {
                fourcc: *k,
                depth: r.u16(74).unwrap_or(0),
                bottom_up: false,
                palette: crate::rawvid::qt_palette(entry.body),
            }
            .to_config();
            (
                crate::rawvid::codec_of(*k).unwrap_or(Codec::Raw),
                b"\0\0\0\0",
            )
        }
        k => (Codec::Other(*k), b"\0\0\0\0"),
    };
    v.codec = Some(codec);
    // An inline colour table sits between the entry's fields and its boxes.
    let children = entry
        .body
        .get(78 + crate::rawvid::qt_inline_table_len(entry.body)..)
        .unwrap_or_default();
    for b in boxes(children) {
        let Ok(b) = b else { break };
        if &b.kind == cfg && v.config.is_empty() {
            v.config = b.body.to_vec();
        } else if &b.kind == b"colr" && b.body.len() >= 10 {
            let r = Rd(b.body);
            let kind = &b.body[..4];
            if kind == b"nclx" || kind == b"nclc" {
                v.colour = Some(Colour {
                    primaries: r.u16(4)?,
                    transfer: r.u16(6)?,
                    matrix: r.u16(8)?,
                    full_range: kind == b"nclx" && r.u8(10).is_ok_and(|f| f & 0x80 != 0),
                });
            }
        } else if &b.kind == b"pasp" && b.body.len() >= 8 {
            let r = Rd(b.body);
            let (h, vs) = (r.u32(0)?, r.u32(4)?);
            if h > 0 && vs > 0 && h != vs {
                v.pixel_aspect = Some((h, vs));
            }
        } else if &b.kind == b"fiel" && !b.body.is_empty() {
            v.interlaced = b.body[0] == 2;
            // Detail 6 and 14: the bottom field first (separated or interleaved).
            v.bottom_first = v.interlaced && matches!(b.body.get(1), Some(6 | 14));
        }
    }
    if codec == Codec::H264 && v.config.is_empty() && entry.kind.starts_with(b"ai") {
        v.codec = Some(Codec::Other(*b"AVCI"));
        return Ok(());
    }
    if codec == Codec::Mpeg4 {
        // `mp4v`: the object type says which MPEG video; the decoder-specific info is the
        // MPEG-4 VOL header or the MPEG-1/2 sequence header (often absent: in band).
        let esds = std::mem::take(&mut v.config);
        let (ot, dsi) = esds_config_full(&esds).unwrap_or((0x20, Vec::new()));
        v.codec = Some(match ot {
            0x60..=0x65 => Codec::Mpeg2,
            0x6A => Codec::Mpeg1,
            0x20 => Codec::Mpeg4,
            0x6C => Codec::Mjpeg,
            _ => Codec::Other(*b"mp4v"),
        });
        v.config = dsi;
        return Ok(());
    }
    if matches!(
        codec,
        Codec::Other(_)
            | Codec::Vp8
            | Codec::Vp9
            | Codec::Mpeg2
            | Codec::Mpeg1
            | Codec::H263
            | Codec::JpegXs
            | Codec::ProRes
            | Codec::Mjpeg
            | Codec::Jpeg2000
            | Codec::Dv
            | Codec::Vp3
            | Codec::Flv1
            | Codec::Raw
            | Codec::Classic(_)
            | Codec::CineForm
            | Codec::Hap(_)
            | Codec::Vc2
            | Codec::Apv
    ) {
        // These describe themselves in band; their records are optional for playing them.
        return Ok(());
    }
    if v.config.is_empty() {
        return Err(bad(format!(
            "{} sample entry without its configuration record",
            codec.name()
        )));
    }
    Ok(())
}

/// The first sample entry of a sound track's `stsd`.
pub(crate) fn audio_entry(stsd: &[u8]) -> Result<Option<AudioTrack>> {
    let Some(entry) = stsd.get(8..).and_then(|e| boxes(e).next()) else {
        return Ok(None);
    };
    let entry = entry?;
    let r = Rd(entry.body);
    // SoundSampleEntry: 6 reserved + data reference index, then version (QuickTime 0, 1, 2),
    // revision, vendor, channels, sample size, compression id, packet size, rate 16.16.
    let version = r.u16(8)?;
    let mut channels = r.u16(16)?;
    let mut bits = r.u16(18)?;
    let mut rate = r.u32(24)? >> 16;
    // Version 2's LPCM format flags (Core Audio): bit 0 float, bit 1 big-endian.
    let mut lpcm_flags = None;
    let children_at = match version {
        1 => 28 + 16,
        2 => {
            // Version 2: the rate as a 64-bit float, the channel count, bits per channel and
            // the format flags follow.
            let f = f64::from_bits(r.u64(32)?);
            if f.is_finite() && f > 0.0 && f < 1e7 {
                rate = f as u32;
            }
            channels = r.u32(40)?.min(64) as u16;
            bits = r.u32(48)?.min(64) as u16;
            lpcm_flags = Some(r.u32(52)?);
            28 + 36
        }
        _ => 28,
    };
    let children = entry.body.get(children_at..).unwrap_or_default();
    let codec = match &entry.kind {
        b"mp4a" => match find_deep(children, b"esds", 0) {
            Some(esds) => match esds_config(esds) {
                Some((ot, asc)) if matches!(ot, 0x40 | 0x66..=0x68) && !asc.is_empty() => {
                    AudioCodec::Aac(asc)
                }
                // MPEG-1 and MPEG-2 (backward compatible) audio: the layer from the frames.
                Some((0x69 | 0x6B, _)) => AudioCodec::Mpa(0),
                // DTS (MP4RA object types): core, DTS-HD High Resolution, Master Audio,
                // Express.
                Some((0xA9, _)) => AudioCodec::Other(*b"dtsc", 0),
                Some((0xAA, _)) => AudioCodec::Other(*b"dtsh", 0),
                Some((0xAB, _)) => AudioCodec::Other(*b"dtsl", 0),
                Some((0xAC, _)) => AudioCodec::Other(*b"dtse", 0),
                // 3GPP2 speech (C.S0050): QCELP-13 (13K voice), EVRC, SMV.
                Some((0xE1, _)) => AudioCodec::Other(*b"qclp", 0),
                Some((0xA0, _)) => AudioCodec::Other(*b"sevc", 0),
                Some((0xA1, _)) => AudioCodec::Other(*b"ssmv", 0),
                Some((ot, _)) => AudioCodec::Other(*b"mp4a", ot),
                None => AudioCodec::Other(*b"mp4a", 0),
            },
            None => AudioCodec::Other(*b"mp4a", 0),
        },
        b"alac" => match find_deep(children, b"alac", 0) {
            // A full box: version and flags, then the 24-byte config.
            Some(c) if c.len() >= 28 => AudioCodec::Alac(c[4..28].to_vec()),
            _ => AudioCodec::Other(*b"alac", 0),
        },
        b"Opus" => match find_deep(children, b"dOps", 0).and_then(ops_to_opus_head) {
            Some(head) => AudioCodec::Opus(head),
            None => AudioCodec::Other(*b"Opus", 0),
        },
        b"ac-3" => AudioCodec::Ac3,
        b"ec-3" => AudioCodec::Eac3,
        b"fLaC" => AudioCodec::Flac(find_deep(children, b"dfLa", 0).unwrap_or_default().to_vec()),
        // QuickTime MPEG audio: `.mp3`, and the WAVE format tags 0x55 (MP3) and 0x50.
        b".mp3" | b"ms\0\x55" => AudioCodec::Mpa(3),
        b".mp2" | b".mp1" | b"ms\0\x50" => AudioCodec::Mpa(0),
        b"ulaw" => AudioCodec::G711 { alaw: false },
        b"alaw" => AudioCodec::G711 { alaw: true },
        // 3GPP2 (`sqcp`, `sevc`, `ssmv`) and QuickTime (`Qclp`) speech.
        b"sqcp" => AudioCodec::Other(*b"qclp", 0),
        // QuickTime keeps every packet in a 35-byte slot, the shorter ones padded.
        b"Qclp" => AudioCodec::Other(*b"qclp", 1),
        b"sevc" => AudioCodec::Other(*b"sevc", 0),
        b"ssmv" => AudioCodec::Other(*b"ssmv", 0),
        b"ima4" => AudioCodec::Adpcm {
            kind: AdpcmKind::ImaQt,
            block: 0,
        },
        k => pcm(k, bits, lpcm_flags, children).unwrap_or(AudioCodec::Other(*k, 0)),
    };
    if matches!(codec, AudioCodec::Opus(_)) {
        // Opus always decodes at 48 kHz, whatever the entry says.
        rate = 48_000;
    }
    if crate::qcelp_sound::is_speech(&codec) {
        // 3GPP2 speech is 8 kHz mono whatever the entry says; QuickTime's `Qclp` plays its
        // frames at the entry's rate (Apple's encoder took 22 050 Hz sound as is).
        channels = 1;
        if !matches!(codec, AudioCodec::Other(_, 1)) || rate == 0 {
            rate = 8000;
        }
    }
    Ok(Some(AudioTrack {
        codec,
        channels,
        sample_rate: rate,
        timescale: 0,
        duration: 0,
        sample_count: 0,
        samples: Vec::new(),
        edit: Edit::default(),
        layout: Layout::Contiguous,
        extra: Default::default(),
    }))
}

/// An `OpusSpecificBox` (`dOps`, big-endian) as the Ogg/Matroska `OpusHead` (little-endian).
fn ops_to_opus_head(d: &[u8]) -> Option<Vec<u8>> {
    if d.len() < 11 || d[0] != 0 {
        return None;
    }
    let mut h = b"OpusHead".to_vec();
    h.push(1);
    h.push(d[1]);
    h.extend_from_slice(&[d[3], d[2]]);
    h.extend_from_slice(&[d[7], d[6], d[5], d[4]]);
    h.extend_from_slice(&[d[9], d[8]]);
    h.push(d[10]);
    if d[10] != 0 {
        let n = 2 + usize::from(d[1]);
        h.extend_from_slice(d.get(11..11 + n)?);
    }
    Some(h)
}

/// The PCM coding of sample entry `kind` with the entry's sample size `bits`, version 2
/// format flags and child boxes (`enda` byte order in a `wave`, ISO `pcmC`).
fn pcm(kind: &[u8; 4], bits: u16, flags: Option<u32>, children: &[u8]) -> Option<AudioCodec> {
    // QuickTime `enda`: 1 for little-endian (in24, in32, fl32, fl64).
    let enda = || {
        find_deep(children, b"enda", 0)
            .and_then(|e| e.get(..2))
            .is_some_and(|e| e[1] == 1 || e[0] == 1)
    };
    let (bits, float, little_endian) = match kind {
        // Byte order means nothing at 8 bits; "little-endian" marks the unsigned samples.
        b"sowt" | b"twos" if bits == 8 => (8, false, false),
        b"raw " => (8, false, true),
        b"sowt" => (bits, false, true),
        b"twos" => (bits, false, false),
        b"in24" => (24, false, enda()),
        b"in32" => (32, false, enda()),
        b"fl32" => (32, true, enda()),
        b"fl64" => (64, true, enda()),
        b"lpcm" => {
            let f = flags?;
            (bits, f & 1 != 0, f & 2 == 0)
        }
        b"ipcm" | b"fpcm" => {
            // pcmC: version and flags, format flags (bit 0: little-endian), sample size.
            let c = find_deep(children, b"pcmC", 0)?;
            (u16::from(*c.get(5)?), kind == b"fpcm", c.get(4)? & 1 != 0)
        }
        _ => return None,
    };
    let ok = if float {
        matches!(bits, 32 | 64)
    } else {
        matches!(bits, 8 | 16 | 24 | 32)
    };
    ok.then_some(AudioCodec::Pcm {
        bits,
        float,
        little_endian,
    })
}

/// The first box of `kind` among `data`'s boxes or inside a QuickTime `wave` box.
fn find_deep<'a>(data: &'a [u8], kind: &[u8; 4], depth: usize) -> Option<&'a [u8]> {
    if depth > 2 {
        return None;
    }
    for b in boxes(data) {
        let Ok(b) = b else { return None };
        if &b.kind == kind {
            return Some(b.body);
        }
        if &b.kind == b"wave"
            && let Some(f) = find_deep(b.body, kind, depth + 1)
        {
            return Some(f);
        }
    }
    None
}

/// An MPEG-4 descriptor: tag, then a length of up to four 7-bit groups. Returns the tag, the
/// body and what follows.
fn descriptor(d: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *d.first()?;
    let mut len = 0usize;
    let mut i = 1;
    loop {
        let b = *d.get(i)?;
        len = (len << 7) | (b & 0x7f) as usize;
        i += 1;
        if b & 0x80 == 0 || i > 4 {
            break;
        }
    }
    let body = d.get(i..i.checked_add(len)?)?;
    Some((tag, body, &d[i + len..]))
}

/// [`esds_config`] of an `esds` box's body as kept by `sample_entry` (the box payload).
fn esds_config_full(esds: &[u8]) -> Option<(u8, Vec<u8>)> {
    esds_config(esds)
}

/// The object type indication and decoder-specific info (AudioSpecificConfig) of an `esds`.
pub(crate) fn esds_config(esds: &[u8]) -> Option<(u8, Vec<u8>)> {
    let (tag, es, _) = descriptor(esds.get(4..)?)?;
    if tag != 3 {
        return None;
    }
    let flags = *es.get(2)?;
    let mut o = 3;
    if flags & 0x80 != 0 {
        o += 2;
    }
    if flags & 0x40 != 0 {
        o += 1 + *es.get(o)? as usize;
    }
    if flags & 0x20 != 0 {
        o += 2;
    }
    let mut rest = es.get(o..)?;
    while !rest.is_empty() {
        let (tag, body, next) = descriptor(rest)?;
        if tag == 4 {
            let ot = *body.first()?;
            let mut inner = body.get(13..)?;
            while !inner.is_empty() {
                let (t, b, n) = descriptor(inner)?;
                if t == 5 {
                    return Some((ot, b.to_vec()));
                }
                inner = n;
            }
            return Some((ot, Vec::new()));
        }
        rest = next;
    }
    None
}

#[derive(Default)]
pub(crate) struct Tables<'a> {
    pub(crate) stts: Option<&'a [u8]>,
    pub(crate) ctts: Option<&'a [u8]>,
    pub(crate) stss: Option<&'a [u8]>,
    /// `stsz`, or `stz2` (compact sizes) when the flag is set.
    pub(crate) stsz: Option<(&'a [u8], bool)>,
    pub(crate) stsc: Option<&'a [u8]>,
    pub(crate) stco: Option<(&'a [u8], bool)>,
}

impl Tables<'_> {
    /// Uncompressed audio by chunk (one entry per `stco` chunk, `frame` bytes per packet
    /// of `frames` audio frames, 1 for PCM): QuickTime counts every audio frame as a sample, millions of them, with a
    /// nominal size; the chunk's frame count from `stsc` and the first `stts` delta give its
    /// bytes and duration.
    fn pcm_chunks(&self, frames: u32, frame: usize, movie_len: u64) -> Result<Vec<Sample>> {
        if frame == 0 || frames == 0 {
            return Err(bad("PCM frame without bytes"));
        }
        let (co, wide) = self.stco.ok_or_else(|| bad("no chunk offsets"))?;
        let co = Rd(co);
        let chunks = co.count(4, if wide { 8 } else { 4 })?;
        if chunks > MAX_SAMPLES {
            return Err(Error::Unsupported(format!("{chunks} chunks")));
        }
        let tt = Rd(self.stts.ok_or_else(|| bad("no stts"))?);
        let delta = if tt.count(4, 8)? > 0 {
            tt.u32(12)?.max(1)
        } else {
            1
        };
        let sc = Rd(self.stsc.ok_or_else(|| bad("no stsc"))?);
        let runs = sc.count(4, 12)?;
        let mut out = Vec::with_capacity(chunks);
        let mut t = 0u64;
        for k in 0..runs {
            let first = sc.u32(8 + 12 * k)? as usize;
            let per = sc.u32(12 + 12 * k)?;
            let next = if k + 1 < runs {
                sc.u32(8 + 12 * (k + 1))? as usize
            } else {
                chunks + 1
            };
            if first == 0 || next <= first || next > chunks + 1 {
                return Err(bad("stsc runs out of order"));
            }
            for c in first..next {
                let offset = if wide {
                    co.u64(8 + 8 * (c - 1))?
                } else {
                    co.u32(8 + 4 * (c - 1))? as u64
                };
                // Compressed sound (`ima4`) counts its packets' frames.
                let size = u64::from(per / frames).saturating_mul(frame as u64);
                if size > u32::MAX as u64 || offset.saturating_add(size) > movie_len {
                    // A cut-short file plays what it holds.
                    if !out.is_empty() && size <= u32::MAX as u64 {
                        return Ok(out);
                    }
                    return Err(bad(format!("PCM chunk {c} past the movie")));
                }
                let duration = per.saturating_mul(delta);
                out.push(Sample {
                    offset,
                    size: size as u32,
                    dts: t,
                    cts: t.min(i64::MAX as u64) as i64,
                    duration,
                    sync: true,
                });
                t = t.saturating_add(u64::from(duration));
            }
        }
        Ok(out)
    }

    /// The `n` samples in decode order with offsets checked against the movie's length.
    pub(crate) fn expand(&self, n: usize, movie_len: u64) -> Result<Vec<Sample>> {
        let mut out = vec![
            Sample {
                offset: 0,
                size: 0,
                dts: 0,
                cts: 0,
                duration: 0,
                sync: self.stss.is_none(),
            };
            n
        ];
        // Sizes.
        let (z, compact) = self.stsz.ok_or_else(|| bad("no stsz"))?;
        let z = Rd(z);
        if compact {
            let bits = z.u8(7)? as usize;
            if !matches!(bits, 4 | 8 | 16) {
                return Err(bad(format!("stz2 field size {bits}")));
            }
            if z.0.len() < 12 + (n * bits).div_ceil(8) {
                return Err(bad("stz2 shorter than its count"));
            }
            for (i, s) in out.iter_mut().enumerate() {
                s.size = match bits {
                    4 => {
                        let b = z.u8(12 + i / 2)?;
                        u32::from(if i % 2 == 0 { b >> 4 } else { b & 15 })
                    }
                    8 => u32::from(z.u8(12 + i)?),
                    _ => u32::from(z.u16(12 + 2 * i)?),
                };
            }
        } else {
            let fixed = z.u32(4)?;
            if fixed == 0 {
                let have = z.count(8, 4)?;
                if have < n {
                    return Err(bad("stsz shorter than its count"));
                }
                for (i, s) in out.iter_mut().enumerate() {
                    s.size = z.u32(12 + 4 * i)?;
                }
            } else {
                out.iter_mut().for_each(|s| s.size = fixed);
            }
        }
        // Offsets: chunks from stco / co64, samples per chunk from stsc.
        let (co, wide) = self.stco.ok_or_else(|| bad("no chunk offsets"))?;
        let co = Rd(co);
        let esize = if wide { 8 } else { 4 };
        let chunks = co.count(4, esize)?;
        let chunk_off = |c: usize| -> Result<u64> {
            if wide {
                co.u64(8 + 8 * c)
            } else {
                Ok(co.u32(8 + 4 * c)? as u64)
            }
        };
        let sc = Rd(self.stsc.ok_or_else(|| bad("no stsc"))?);
        let runs = sc.count(4, 12)?;
        let mut i = 0usize;
        // A movie cut short (a copy that stopped early, a download not finished) keeps the
        // samples before the first one past its end, as ffmpeg plays such files.
        let mut cut = None;
        'runs: for k in 0..runs {
            let first = sc.u32(8 + 12 * k)? as usize;
            let per = sc.u32(12 + 12 * k)? as usize;
            let next = if k + 1 < runs {
                sc.u32(8 + 12 * (k + 1))? as usize
            } else {
                chunks + 1
            };
            if first == 0 || next <= first || next > chunks + 1 {
                return Err(bad("stsc runs out of order"));
            }
            for c in first..next {
                let mut off = chunk_off(c - 1)?;
                for _ in 0..per {
                    if i == n {
                        break;
                    }
                    let s = &mut out[i];
                    s.offset = off;
                    let end = off
                        .checked_add(s.size as u64)
                        .ok_or_else(|| bad("sample offset overflows"))?;
                    if end > movie_len {
                        if i == 0 {
                            return Err(bad(format!(
                                "sample {i} ends at {end}, past the movie ({movie_len})"
                            )));
                        }
                        cut = Some(i);
                        break 'runs;
                    }
                    off = end;
                    i += 1;
                }
            }
        }
        if cut.is_none() && i < n {
            return Err(bad(format!("chunks hold {i} of {n} samples")));
        }
        // Decode times.
        let tt = Rd(self.stts.ok_or_else(|| bad("no stts"))?);
        let runs = tt.count(4, 8)?;
        let (mut t, mut i, mut last) = (0u64, 0usize, 0u32);
        for k in 0..runs {
            let (count, delta) = (tt.u32(8 + 8 * k)? as usize, tt.u32(12 + 8 * k)?);
            for _ in 0..count.min(n - i) {
                out[i].dts = t;
                out[i].duration = delta;
                t = t.saturating_add(delta as u64);
                i += 1;
            }
            last = delta;
        }
        for s in &mut out[i..] {
            s.dts = t;
            s.duration = last;
            t = t.saturating_add(last as u64);
        }
        // Composition offsets (version 1 signed; version 0 unsigned but written signed by many).
        for s in &mut out {
            s.cts = s.dts.min(i64::MAX as u64) as i64;
        }
        if let Some(ct) = self.ctts {
            let ct = Rd(ct);
            let runs = ct.count(4, 8)?;
            let mut i = 0usize;
            for k in 0..runs {
                let (count, off) = (ct.u32(8 + 8 * k)? as usize, ct.u32(12 + 8 * k)? as i32);
                for _ in 0..count.min(n - i) {
                    out[i].cts = out[i].cts.saturating_add(off as i64);
                    i += 1;
                }
            }
        }
        if let Some(ss) = self.stss {
            let ss = Rd(ss);
            let k = ss.count(4, 4)?;
            for j in 0..k {
                let idx = ss.u32(8 + 4 * j)? as usize;
                if (1..=n).contains(&idx) {
                    out[idx - 1].sync = true;
                }
            }
        }
        if let Some(fit) = cut {
            out.truncate(fit);
        }
        Ok(out)
    }
}

/// A metadata item's name: printable four-character codes as text (`©` kept), others as hex.
fn item_name(kind: [u8; 4]) -> String {
    if kind[0] == 0xA9 {
        let mut s = String::from("©");
        s.push_str(&String::from_utf8_lossy(&kind[1..]));
        s
    } else {
        String::from_utf8_lossy(&kind).into_owned()
    }
}

/// The items of a `meta` box (QuickTime: a plain container; ISO: a full box): through `keys`
/// when present (QuickTime `mdta`), else by their four-character codes (iTunes style).
/// Appends (name, text) pairs to `out`.
fn meta_items(meta: &[u8], depth: usize, out: &mut Vec<(String, String)>) -> Result<()> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    // A full box has four bytes of version and flags before its first child.
    let body = if meta.len() >= 12 && !is_fourcc(&meta[4..8]) && is_fourcc(&meta[8..12]) {
        &meta[4..]
    } else {
        meta
    };
    let Some(ilst) = child(body, b"ilst")? else {
        return Ok(());
    };
    let mut names = Vec::new();
    if let Some(keys) = child(body, b"keys")? {
        let r = Rd(keys);
        let n = r.u32(4)? as usize;
        let mut pos = 8;
        for _ in 0..n.min(4096) {
            let size = r.u32(pos)? as usize;
            if size < 8 || pos + size > keys.len() {
                return Err(bad("keys entry outside the box"));
            }
            names.push(String::from_utf8_lossy(&keys[pos + 8..pos + size]).into_owned());
            pos += size;
        }
    }
    for item in boxes(ilst) {
        let item = item?;
        let index = u32::from_be_bytes(item.kind) as usize;
        let name = if !names.is_empty() && (1..=names.len()).contains(&index) {
            names[index - 1].clone()
        } else {
            item_name(item.kind)
        };
        if let Some(data) = child(item.body, b"data")?
            && data.len() > 8
        {
            // Type 1 is UTF-8; other types (numbers, images) are not text.
            let ty = Rd(data).u32(0)? & 0xff_ffff;
            if ty == 1 || ty == 0 {
                let s = String::from_utf8_lossy(&data[8..])
                    .trim_matches(char::from(0))
                    .trim()
                    .to_owned();
                out.push((name, s));
            }
        }
    }
    Ok(())
}

/// `udta`: classic QuickTime text atoms (`©day`, `©mak`, `©mod`: a 16-bit length, a
/// language code, then the text), an embedded `meta`, and `XMP_`.
fn udta(body: &[u8], out: &mut Vec<(String, String)>, xmp: &mut Option<Vec<u8>>) -> Result<()> {
    for b in boxes(body) {
        let Ok(b) = b else { break };
        match &b.kind {
            b"meta" => meta_items(b.body, 1, out)?,
            b"XMP_" if xmp.is_none() => *xmp = Some(b.body.to_vec()),
            [0xA9, ..] => {
                let r = Rd(b.body);
                if let Ok(len) = r.u16(0)
                    && let Some(text) = b.body.get(4..4 + len as usize)
                {
                    let s = String::from_utf8_lossy(text)
                        .trim_matches(char::from(0))
                        .trim()
                        .to_owned();
                    out.push((item_name(b.kind), s));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The coded sample's bytes.
pub fn sample_bytes<'a>(movie: &'a [u8], s: &Sample) -> Option<&'a [u8]> {
    let start = usize::try_from(s.offset).ok()?;
    let end = start.checked_add(s.size as usize)?;
    // A stream's web file still being fetched: wait for the bytes (missing when they do not
    // come in time), never read the zeros of a hole.
    if !specular_core::fill::wait(movie, start..end.min(movie.len())) {
        return None;
    }
    movie.get(start..end)
}

#[cfg(test)]
pub(crate) mod build {
    //! Writes small synthetic movies for the tests.

    pub fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    pub fn full(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
        let mut b = vec![version, 0, 0, 0];
        b.extend_from_slice(body);
        bx(kind, &b)
    }

    fn be32(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_be_bytes()).collect()
    }

    /// Options of the synthetic movie.
    pub struct Spec<'a> {
        pub codec: &'a [u8; 4],
        pub config: &'a [u8],
        pub samples: &'a [&'a [u8]],
        /// Composition offsets per sample (`ctts`), when any.
        pub ctts: Option<&'a [u32]>,
        /// 1-based sync sample numbers (`stss`), when any.
        pub stss: Option<&'a [u32]>,
        pub rotation_matrix: [i32; 4],
        pub content_id: Option<&'a str>,
        pub co64: bool,
        /// The `moov/udta` box's body, when any.
        pub udta: Option<&'a [u8]>,
    }

    impl Default for Spec<'_> {
        fn default() -> Self {
            Spec {
                codec: b"avc1",
                config: &[1, 0x64, 0, 0x1f, 0xff, 0xe0, 0x00],
                samples: &[b"frame-one", b"frame-2", b"f3"],
                ctts: None,
                stss: None,
                rotation_matrix: [0x1_0000, 0, 0, 0x1_0000],
                content_id: None,
                co64: false,
                udta: None,
            }
        }
    }

    /// A movie: `ftyp`, `mdat` with the samples (one chunk), then `moov`.
    pub fn movie(s: &Spec<'_>) -> Vec<u8> {
        let mut out = bx(b"ftyp", b"isom\0\0\0\0isomiso2avc1mp41");
        let mdat_body: Vec<u8> = s.samples.concat();
        let first_sample = out.len() as u64 + 8;
        out.extend(bx(b"mdat", &mdat_body));
        let n = s.samples.len() as u32;
        // mvhd v0: times, timescale 1000, duration 100 ms per sample.
        let mut mvhd = be32(&[0, 0, 1000, 100 * n]);
        mvhd.extend(vec![0u8; 80]);
        let mut tkhd = be32(&[0, 0, 1, 0, 100 * n, 0, 0, 0, 0]);
        tkhd.truncate(28);
        tkhd.extend(be32(&[0, 0]));
        let m = s.rotation_matrix;
        tkhd.extend(be32(&[
            m[0] as u32,
            m[1] as u32,
            0,
            m[2] as u32,
            m[3] as u32,
            0,
            0,
            0,
            0x4000_0000,
        ]));
        tkhd.extend(be32(&[640 << 16, 480 << 16]));
        let mdhd = be32(&[0, 0, 90_000, 9_000 * n, 0]);
        let mut hdlr = be32(&[0]);
        hdlr.extend_from_slice(b"vide");
        hdlr.extend(vec![0u8; 13]);
        // Visual sample entry: 6 reserved + data ref index, 16 pre-defined/reserved, size, …
        let mut entry = vec![0u8; 6];
        entry.extend([0, 1]);
        entry.extend(vec![0u8; 16]);
        entry.extend(640u16.to_be_bytes());
        entry.extend(480u16.to_be_bytes());
        entry.extend(vec![0u8; 50]);
        entry.extend(bx(
            if s.codec == b"avc1" { b"avcC" } else { b"hvcC" },
            s.config,
        ));
        entry.extend(bx(b"colr", b"nclx\0\x01\0\x01\0\x01\x80"));
        let mut stsd = be32(&[1]);
        stsd.extend(bx(s.codec, &entry));
        let mut stbl = full(b"stsd", 0, &stsd);
        stbl.extend(full(b"stts", 0, &be32(&[1, n, 9_000])));
        if let Some(c) = s.ctts {
            let mut b = be32(&[c.len() as u32]);
            for &o in c {
                b.extend(be32(&[1, o]));
            }
            stbl.extend(full(b"ctts", 0, &b));
        }
        if let Some(ss) = s.stss {
            let mut b = be32(&[ss.len() as u32]);
            b.extend(be32(ss));
            stbl.extend(full(b"stss", 0, &b));
        }
        let mut stsz = be32(&[0, n]);
        stsz.extend(be32(
            &s.samples.iter().map(|x| x.len() as u32).collect::<Vec<_>>(),
        ));
        stbl.extend(full(b"stsz", 0, &stsz));
        stbl.extend(full(b"stsc", 0, &be32(&[1, 1, n, 1])));
        if s.co64 {
            let mut b = be32(&[1]);
            b.extend(first_sample.to_be_bytes());
            stbl.extend(full(b"co64", 0, &b));
        } else {
            stbl.extend(full(b"stco", 0, &be32(&[1, first_sample as u32])));
        }
        let minf = bx(b"stbl", &stbl);
        let mut mdia = full(b"mdhd", 0, &mdhd);
        mdia.extend(full(b"hdlr", 0, &hdlr));
        mdia.extend(bx(b"minf", &minf));
        let mut trak = full(b"tkhd", 0, &tkhd);
        trak.extend(bx(b"mdia", &mdia));
        let mut moov = full(b"mvhd", 0, &mvhd);
        moov.extend(bx(b"trak", &trak));
        if let Some(id) = s.content_id {
            // QuickTime meta: hdlr 'mdta', keys, ilst.
            let mut keys = be32(&[0, 1]);
            let k = b"com.apple.quicktime.content.identifier";
            keys.extend(((k.len() + 8) as u32).to_be_bytes());
            keys.extend_from_slice(b"mdta");
            keys.extend_from_slice(k);
            let mut data = be32(&[1, 0]);
            data.extend_from_slice(id.as_bytes());
            let item = bx(&1u32.to_be_bytes(), &bx(b"data", &data));
            let mut meta = full(b"hdlr", 0, b"\0\0\0\0mdta\0\0\0\0\0\0\0\0\0\0\0\0\0");
            meta.extend(bx(b"keys", &keys));
            meta.extend(bx(b"ilst", &item));
            moov.extend(bx(b"meta", &meta));
        }
        if let Some(u) = s.udta {
            moov.extend(bx(b"udta", u));
        }
        out.extend(bx(b"moov", &moov));
        out
    }
}

/// The quarter turns and mirror of a `tkhd` matrix's (a, b, c, d) (16.16 fixed point; a point
/// goes to x' = a·x + c·y, y' = b·x + d·y, y down): (0, false) for anything but a quarter
/// turn, with or without a mirror.
fn matrix_turn(a: i32, b: i32, c: i32, d: i32) -> (u16, bool) {
    let one = 0x1_0000;
    let u = |x: i32| match x {
        x if x == one => Some(1),
        x if x == -one => Some(-1),
        _ => None,
    };
    match (a, b, c, d) {
        (a, 0, 0, d) => match (u(a), u(d)) {
            (Some(1), Some(1)) => (0, false),
            (Some(-1), Some(-1)) => (180, false),
            (Some(-1), Some(1)) => (0, true),
            (Some(1), Some(-1)) => (180, true),
            _ => (0, false),
        },
        (0, b, c, 0) => match (u(b), u(c)) {
            (Some(1), Some(-1)) => (90, false),
            (Some(-1), Some(1)) => (270, false),
            (Some(-1), Some(-1)) => (90, true),
            (Some(1), Some(1)) => (270, true),
            _ => (0, false),
        },
        _ => (0, false),
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    /// Where a point of a 2×1 picture lands under a matrix and under the EXIF orientation
    /// read from it agree for every quarter turn, mirrored or not.
    #[test]
    fn tkhd_matrices_turn_and_mirror_like_exif() {
        let one = 0x1_0000;
        let mut seen = std::collections::HashSet::new();
        for (a, b, c, d) in [
            (one, 0, 0, one),
            (0, one, -one, 0),
            (-one, 0, 0, -one),
            (0, -one, one, 0),
            (-one, 0, 0, one),
            (0, -one, -one, 0),
            (one, 0, 0, -one),
            (0, one, one, 0),
        ] {
            let (rotation, mirror) = matrix_turn(a, b, c, d);
            let v = VideoTrack {
                rotation,
                mirror,
                ..VideoTrack::default()
            };
            let o = specular_core::Orientation(v.orientation());
            seen.insert(o.0);
            // The matrix's image of the unit axes, against the orientation's.
            let m = |x: i32, y: i32| ((a * x + c * y) / one, (b * x + d * y) / one);
            let (q, f) = o.to_rotation_flip();
            let e = |x: i32, y: i32| {
                let (mut x, mut y) = (if f { -x } else { x }, y);
                for _ in 0..q {
                    (x, y) = (-y, x);
                }
                (x, y)
            };
            assert_eq!(m(1, 0), e(1, 0), "{a} {b} {c} {d}");
            assert_eq!(m(0, 1), e(0, 1), "{a} {b} {c} {d}");
        }
        assert_eq!(seen.len(), 8);
        assert_eq!(matrix_turn(one / 2, 0, 0, one), (0, false));
    }

    #[test]
    fn a_mirrored_track_reads_as_mirrored() {
        let data = movie(&Spec {
            rotation_matrix: [0, -0x1_0000, -0x1_0000, 0],
            ..Spec::default()
        });
        let v = parse(&data, true).unwrap().video.unwrap();
        assert_eq!((v.rotation, v.mirror), (90, true));
        assert_eq!(v.orientation(), 7);
    }

    /// The movie's text tags: QuickTime `keys` items and classic `udta` atoms, in file order,
    /// without Apple's content identifier. exiftool on the movie (written as `tags.mov` with
    /// `SPECULAR_FIXTURE_DIR` set): GPSCoordinates 38.7223 -9.1393 45, Title "Tram 28".
    #[test]
    fn text_tags_are_kept() {
        let atom = |kind: &[u8; 4], text: &str| {
            let mut b = (text.len() as u16).to_be_bytes().to_vec();
            b.extend([0x15, 0xC7]);
            b.extend(text.as_bytes());
            bx(kind, &b)
        };
        let udta = [
            atom(b"\xA9xyz", "+38.7223-009.1393+045.000/"),
            atom(b"\xA9nam", "Tram 28"),
        ]
        .concat();
        let data = movie(&Spec {
            content_id: Some("ABC"),
            udta: Some(&udta),
            ..Spec::default()
        });
        let m = parse(&data, false).unwrap();
        assert_eq!(
            m.tags,
            [
                ("©xyz".to_owned(), "+38.7223-009.1393+045.000/".to_owned()),
                ("©nam".to_owned(), "Tram 28".to_owned()),
            ]
        );
        assert_eq!(m.content_id.as_deref(), Some("ABC"));
        if let Some(dir) = std::env::var_os("SPECULAR_FIXTURE_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("tags.mov"), &data).unwrap();
        }
    }

    #[test]
    fn parses_a_synthetic_movie() {
        let data = movie(&Spec::default());
        let m = parse(&data, true).unwrap();
        assert_eq!(&m.brand, b"isom");
        assert_eq!(m.duration_us, Some(300_000));
        let v = m.video.unwrap();
        assert_eq!(v.codec, Some(Codec::H264));
        assert_eq!((v.width, v.height), (640, 480));
        assert_eq!(v.display, (640, 480));
        assert_eq!(v.rotation, 0);
        assert_eq!(v.timescale, 90_000);
        assert_eq!(v.duration_us(), Some(300_000));
        assert_eq!(v.sample_count, 3);
        assert_eq!(v.config, [1, 0x64, 0, 0x1f, 0xff, 0xe0, 0x00]);
        assert_eq!(
            v.colour,
            Some(Colour {
                primaries: 1,
                transfer: 1,
                matrix: 1,
                full_range: true
            })
        );
        let got: Vec<&[u8]> = v
            .samples
            .iter()
            .map(|s| sample_bytes(&data, s).unwrap())
            .collect();
        assert_eq!(got, [&b"frame-one"[..], b"frame-2", b"f3"]);
        assert_eq!(
            v.samples.iter().map(|s| s.dts).collect::<Vec<_>>(),
            [0, 9_000, 18_000]
        );
        assert!(v.samples.iter().all(|s| s.sync));
    }

    #[test]
    fn headers_only_skips_the_sample_table() {
        let m = parse(&movie(&Spec::default()), false).unwrap();
        let v = m.video.unwrap();
        assert_eq!(v.sample_count, 3);
        assert!(v.samples.is_empty());
    }

    #[test]
    fn composition_offsets_sync_samples_co64_rotation_content_id() {
        let data = movie(&Spec {
            codec: b"hvc1",
            ctts: Some(&[9_000, 27_000, 0]),
            stss: Some(&[1]),
            rotation_matrix: [0, 0x1_0000, -0x1_0000, 0],
            content_id: Some("8A6C1F5E-0000-4C1D-9C0A-123456789ABC"),
            co64: true,
            ..Spec::default()
        });
        let m = parse(&data, true).unwrap();
        assert_eq!(
            m.content_id.as_deref(),
            Some("8A6C1F5E-0000-4C1D-9C0A-123456789ABC")
        );
        let v = m.video.unwrap();
        assert_eq!(v.codec, Some(Codec::Hevc));
        assert_eq!(v.rotation, 90);
        assert_eq!(
            v.samples.iter().map(|s| s.cts).collect::<Vec<_>>(),
            [9_000, 36_000, 18_000]
        );
        assert_eq!(
            v.samples.iter().map(|s| s.sync).collect::<Vec<_>>(),
            [true, false, false]
        );
        assert_eq!(sample_bytes(&data, &v.samples[2]).unwrap(), b"f3");
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        let good = movie(&Spec::default());
        // Every truncation and a sweep of single-byte corruptions.
        for n in 0..good.len() {
            let _ = parse(&good[..n], true);
        }
        for i in 0..good.len() {
            for v in [0u8, 0xff, 0x7f, 1] {
                let mut d = good.clone();
                d[i] = v;
                if let Ok(m) = parse(&d, true) {
                    for s in m.video.iter().flat_map(|v| &v.samples) {
                        assert!(s.offset + s.size as u64 <= d.len() as u64);
                    }
                }
            }
        }
        assert!(parse(b"", false).is_err());
        assert!(parse(b"\0\0\0\x08moov", false).unwrap().video.is_none());
        // A box larger than the data.
        assert!(parse(b"\0\0\x10\0moov\0\0\0\0", false).is_err());
    }

    #[test]
    fn huge_counts_are_rejected_before_allocating() {
        let mut stsz = vec![0, 0, 0, 0, 0, 0, 0, 0];
        stsz.extend(u32::MAX.to_be_bytes());
        let stbl = bx(b"stbl", &bx(b"stsz", &stsz));
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(b"vide");
        let mdia = [bx(b"hdlr", &hdlr), bx(b"minf", &stbl)].concat();
        let trak = bx(b"trak", &bx(b"mdia", &mdia));
        let data = bx(b"moov", &trak);
        assert!(matches!(parse(&data, true), Err(Error::Unsupported(_))));
    }
}
