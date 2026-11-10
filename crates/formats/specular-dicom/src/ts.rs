//! Transfer syntaxes (PS3.5 10, PS3.6 A): how the data set is encoded and how the pixel data
//! is compressed.

/// Byte order and VR form of the data set after the file meta information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    ImplicitLittle,
    ExplicitLittle,
    ExplicitBig,
    /// Explicit VR little endian, the whole data set raw-deflated (RFC 1951).
    DeflatedLittle,
}

/// How pixel data is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    /// Native (uncompressed) pixel data.
    Native,
    /// Uncompressed frames in encapsulated fragments (1.2.840.10008.1.2.1.98).
    EncapsulatedNative,
    Rle,
    /// Each frame raw-deflated in its own fragment (Deflated Image Frame Compression).
    DeflatedFrame,
    /// JPEG baseline / extended (DCT, 8 or 12 bits).
    JpegDct,
    /// JPEG lossless (process 14, any predictor).
    JpegLossless,
    JpegLs,
    Jpeg2000,
    /// High-Throughput JPEG 2000 (Part 15).
    Htj2k,
    /// JPEG XL (Supplement 232): lossless, JPEG recompression, lossy.
    JpegXl,
    /// A JPEG process DICOM has retired (progressive, hierarchical, arithmetic).
    JpegRetired,
    /// MPEG-2, H.264, H.265 video.
    Video,
    /// JPIP references, XML / MIME encodings and other syntaxes without local pixel data.
    Other,
}

/// A transfer syntax with its display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferSyntax {
    pub uid: String,
    /// "JPEG Lossless, first-order prediction", or the UID when unknown.
    pub name: String,
    pub encoding: Encoding,
    pub compression: Compression,
    /// The syntax may lose information (lossy JPEG / JPEG 2000 / JPEG-LS near-lossless, video).
    pub lossy: bool,
}

// (uid, name, encoding, compression, lossy)
#[rustfmt::skip]
const TABLE: &[(&str, &str, Encoding, Compression, bool)] = {
    use Compression::*;
    use Encoding::*;
    &[
        ("1.2.840.10008.1.2", "Implicit VR Little Endian", ImplicitLittle, Native, false),
        ("1.2.840.10008.1.2.1", "Explicit VR Little Endian", ExplicitLittle, Native, false),
        ("1.2.840.10008.1.2.1.98", "Encapsulated Uncompressed Explicit VR Little Endian", ExplicitLittle, EncapsulatedNative, false),
        ("1.2.840.10008.1.2.1.99", "Deflated Explicit VR Little Endian", DeflatedLittle, Native, false),
        ("1.2.840.10008.1.2.2", "Explicit VR Big Endian", ExplicitBig, Native, false),
        ("1.2.840.10008.1.2.4.50", "JPEG Baseline (8-bit)", ExplicitLittle, JpegDct, true),
        ("1.2.840.10008.1.2.4.51", "JPEG Extended (12-bit)", ExplicitLittle, JpegDct, true),
        ("1.2.840.10008.1.2.4.52", "JPEG Extended (process 3 and 5)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.53", "JPEG Spectral Selection (process 6 and 8)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.54", "JPEG Spectral Selection (process 7 and 9)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.55", "JPEG Full Progression (process 10 and 12)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.56", "JPEG Full Progression (process 11 and 13)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.57", "JPEG Lossless (process 14)", ExplicitLittle, JpegLossless, false),
        ("1.2.840.10008.1.2.4.58", "JPEG Lossless (process 15)", ExplicitLittle, JpegRetired, false),
        ("1.2.840.10008.1.2.4.59", "JPEG Extended Hierarchical (process 16 and 18)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.60", "JPEG Extended Hierarchical (process 17 and 19)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.61", "JPEG Spectral Selection Hierarchical (process 20 and 22)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.62", "JPEG Spectral Selection Hierarchical (process 21 and 23)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.63", "JPEG Full Progression Hierarchical (process 24 and 26)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.64", "JPEG Full Progression Hierarchical (process 25 and 27)", ExplicitLittle, JpegRetired, true),
        ("1.2.840.10008.1.2.4.65", "JPEG Lossless Hierarchical (process 28)", ExplicitLittle, JpegRetired, false),
        ("1.2.840.10008.1.2.4.66", "JPEG Lossless Hierarchical (process 29)", ExplicitLittle, JpegRetired, false),
        ("1.2.840.10008.1.2.4.70", "JPEG Lossless, first-order prediction", ExplicitLittle, JpegLossless, false),
        ("1.2.840.10008.1.2.4.80", "JPEG-LS Lossless", ExplicitLittle, JpegLs, false),
        ("1.2.840.10008.1.2.4.81", "JPEG-LS Near-Lossless", ExplicitLittle, JpegLs, true),
        ("1.2.840.10008.1.2.4.90", "JPEG 2000 (lossless only)", ExplicitLittle, Jpeg2000, false),
        ("1.2.840.10008.1.2.4.91", "JPEG 2000", ExplicitLittle, Jpeg2000, true),
        ("1.2.840.10008.1.2.4.92", "JPEG 2000 Part 2 multi-component (lossless only)", ExplicitLittle, Jpeg2000, false),
        ("1.2.840.10008.1.2.4.93", "JPEG 2000 Part 2 multi-component", ExplicitLittle, Jpeg2000, true),
        ("1.2.840.10008.1.2.4.94", "JPIP Referenced", ExplicitLittle, Other, true),
        ("1.2.840.10008.1.2.4.95", "JPIP Referenced Deflate", DeflatedLittle, Other, true),
        ("1.2.840.10008.1.2.4.100", "MPEG-2 Main Profile", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.100.1", "MPEG-2 Main Profile (fragmentable)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.101", "MPEG-2 Main Profile / High Level", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.101.1", "MPEG-2 Main Profile / High Level (fragmentable)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.102", "H.264 High Profile 4.1", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.102.1", "H.264 High Profile 4.1 (fragmentable)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.103", "H.264 BD-compatible High Profile 4.1", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.103.1", "H.264 BD-compatible High Profile 4.1 (fragmentable)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.104", "H.264 High Profile 4.2 (2D)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.104.1", "H.264 High Profile 4.2 (2D, fragmentable)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.105", "H.264 High Profile 4.2 (3D)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.105.1", "H.264 High Profile 4.2 (3D, fragmentable)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.106", "H.264 Stereo High Profile 4.2", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.106.1", "H.264 Stereo High Profile 4.2 (fragmentable)", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.107", "HEVC Main Profile 5.1", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.108", "HEVC Main 10 Profile 5.1", ExplicitLittle, Video, true),
        ("1.2.840.10008.1.2.4.110", "JPEG XL Lossless", ExplicitLittle, JpegXl, false),
        ("1.2.840.10008.1.2.4.111", "JPEG XL JPEG Recompression", ExplicitLittle, JpegXl, true),
        ("1.2.840.10008.1.2.4.112", "JPEG XL", ExplicitLittle, JpegXl, true),
        ("1.2.840.10008.1.2.4.201", "HTJ2K (lossless only)", ExplicitLittle, Htj2k, false),
        ("1.2.840.10008.1.2.4.202", "HTJ2K with RPCL (lossless only)", ExplicitLittle, Htj2k, false),
        ("1.2.840.10008.1.2.4.203", "HTJ2K", ExplicitLittle, Htj2k, true),
        ("1.2.840.10008.1.2.4.204", "JPIP HTJ2K Referenced", ExplicitLittle, Other, true),
        ("1.2.840.10008.1.2.4.205", "JPIP HTJ2K Referenced Deflate", DeflatedLittle, Other, true),
        ("1.2.840.10008.1.2.5", "RLE Lossless", ExplicitLittle, Rle, false),
        ("1.2.840.10008.1.2.8.1", "Deflated Image Frame Compression", ExplicitLittle, DeflatedFrame, false),
        ("1.2.840.10008.1.2.6.1", "RFC 2557 MIME encapsulation", ExplicitLittle, Other, false),
        ("1.2.840.10008.1.2.6.2", "XML encoding", ExplicitLittle, Other, false),
        ("1.2.840.10008.1.2.7.1", "SMPTE ST 2110-20 progressive video", ExplicitLittle, Video, false),
        ("1.2.840.10008.1.2.7.2", "SMPTE ST 2110-20 interlaced video", ExplicitLittle, Video, false),
        ("1.2.840.10008.1.2.7.3", "SMPTE ST 2110-30 audio", ExplicitLittle, Other, false),
        ("1.2.840.10008.1.20", "Papyrus 3 Implicit VR Little Endian", ImplicitLittle, Native, false),
    ]
};

impl TransferSyntax {
    /// The syntax named by `uid` (trailing NUL / space padding ignored). Unknown UIDs are
    /// taken as explicit VR little endian with encapsulated pixel data of unknown kind, the
    /// most likely layout for a private syntax.
    pub fn from_uid(uid: &str) -> TransferSyntax {
        let uid = uid.trim_end_matches(['\0', ' ']).trim();
        match TABLE.iter().find(|e| e.0 == uid) {
            Some(&(u, name, encoding, compression, lossy)) => TransferSyntax {
                uid: u.to_owned(),
                name: name.to_owned(),
                encoding,
                compression,
                lossy,
            },
            None => TransferSyntax {
                uid: uid.to_owned(),
                name: uid.to_owned(),
                encoding: Encoding::ExplicitLittle,
                compression: Compression::Other,
                lossy: false,
            },
        }
    }

    /// Implicit VR little endian, the default when a file has no meta information.
    pub fn implicit_little() -> TransferSyntax {
        TransferSyntax::from_uid("1.2.840.10008.1.2")
    }

    /// Whether pixel data is stored as encapsulated fragments.
    pub fn encapsulated(&self) -> bool {
        !matches!(self.compression, Compression::Native)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uids_resolve() {
        let t = TransferSyntax::from_uid("1.2.840.10008.1.2.4.70\0");
        assert_eq!(t.compression, Compression::JpegLossless);
        assert_eq!(t.encoding, Encoding::ExplicitLittle);
        assert!(!t.lossy);
        assert_eq!(
            TransferSyntax::from_uid("1.2.840.10008.1.2.2").encoding,
            Encoding::ExplicitBig
        );
        assert_eq!(
            TransferSyntax::from_uid("1.2.3").compression,
            Compression::Other
        );
        assert!(TABLE.iter().all(|e| !e.0.ends_with('.')));
    }
}
