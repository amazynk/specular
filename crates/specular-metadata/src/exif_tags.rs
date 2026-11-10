//! EXIF/TIFF tags beyond the `kamadak-exif` table, and how each is shown.
//!
//! `kamadak-exif` names the tags of EXIF 2.32 in the IFD where that standard puts them. Real
//! files carry more: TIFF 6.0 structure tags, TIFF/EP tags in IFD0, DNG 1.7 tags, EXIF 3.0
//! additions, Microsoft's XP* and padding tags, a few vendor tags, and standard tags written to
//! the wrong IFD (Exif tags in IFD0, TIFF tags in the Exif IFD). [`resolve`] maps a parsed tag
//! to a standard tag or an entry of [`EXTRA`]; only what neither covers is "unknown".
//!
//! Sources (facts only; names are the identifiers the specifications use): TIFF 6.0 and its
//! Adobe supplements, TIFF/EP (ISO 12234-2), CIPA DC-008 (EXIF 2.32 and 3.0), DNG 1.7, GeoTIFF
//! 1.1, Microsoft's Windows property tags, with the ExifTool and Exiv2 tag tables as
//! cross-checks for IDs.

use exif::{Context, Tag};

/// How the value of an [`Extra`] tag is shown in "All fields".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Numbers as stored (rationals as decimals), blobs by size.
    Plain,
    /// Text, whatever the field type (ASCII, BYTE, UNDEFINED or EXIF 3.0 UTF-8): UTF-8 with a
    /// Latin-1 fallback.
    Text,
    /// Windows Explorer XP* properties: UTF-16LE text in a BYTE field.
    Utf16,
    /// Structure or writer-internal data (offsets, padding, embedded blocks shown elsewhere):
    /// flagged as carrying no information, so the info panel hides it.
    Internal,
    /// A version as bytes, e.g. DNGVersion 1.6.0.0.
    Version,
    /// Microsoft's star rating (0–5, -1 rejected).
    Rating,
    /// A code decoded through a Fluent message (`$value` selector); other codes shown as stored.
    Coded(&'static str, &'static [u32]),
    /// Same meaning and encoding as this standard tag (e.g. TIFF/EP numbering of an Exif tag).
    Alias(Tag),
}

#[derive(Debug)]
pub(crate) struct Extra {
    pub num: u16,
    pub name: &'static str,
    pub kind: Kind,
}

/// How a parsed tag is named and decoded.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Resolved {
    /// A tag `kamadak-exif` knows; may differ from the parsed tag when it sat in the wrong IFD.
    Std(Tag),
    Extra(&'static Extra),
    Unknown,
}

/// Names a parsed tag. IFD0 and the Exif IFD share one tag number space (TIFF's registry), so
/// a number is looked up in the other one too; GPS and Interop numbers are separate spaces.
pub(crate) fn resolve(tag: Tag) -> Resolved {
    if tag.description().is_some() {
        return Resolved::Std(tag);
    }
    let other = match tag.context() {
        Context::Tiff => Context::Exif,
        Context::Exif => Context::Tiff,
        _ => return Resolved::Unknown,
    };
    if let Some(x) = extra(tag.number()) {
        return Resolved::Extra(x);
    }
    let t = Tag(other, tag.number());
    if t.description().is_some() {
        return Resolved::Std(t);
    }
    Resolved::Unknown
}

pub(crate) fn extra(num: u16) -> Option<&'static Extra> {
    EXTRA
        .binary_search_by_key(&num, |x| x.num)
        .ok()
        .map(|i| &EXTRA[i])
}

macro_rules! extras {
    ($( $num:literal $name:ident $kind:expr; )*) => {
        &[ $( Extra { num: $num, name: stringify!($name), kind: $kind }, )* ]
    };
}

use Kind::*;

/// Sorted by number (checked by a test).
pub(crate) static EXTRA: &[Extra] = extras! {
    // TIFF 6.0 baseline and extensions.
    0x00FE NewSubfileType Coded("subfile-type", &[0, 1, 2, 4]);
    0x00FF SubfileType Plain;
    0x0107 Thresholding Plain;
    0x0108 CellWidth Plain;
    0x0109 CellLength Plain;
    0x010A FillOrder Plain;
    0x010D DocumentName Text;
    0x0118 MinSampleValue Plain;
    0x0119 MaxSampleValue Plain;
    0x011D PageName Text;
    0x011E XPosition Plain;
    0x011F YPosition Plain;
    0x0120 FreeOffsets Internal;
    0x0121 FreeByteCounts Internal;
    0x0122 GrayResponseUnit Plain;
    0x0123 GrayResponseCurve Plain;
    0x0124 T4Options Plain;
    0x0125 T6Options Plain;
    0x0129 PageNumber Plain;
    0x013C HostComputer Text;
    0x013D Predictor Coded("predictor", &[1, 2, 3]);
    0x0140 ColorMap Plain;
    0x0141 HalftoneHints Plain;
    0x0142 TileWidth Plain;
    0x0143 TileLength Plain;
    0x014A SubIFDs Internal;
    0x014C InkSet Plain;
    0x014D InkNames Text;
    0x014E NumberOfInks Plain;
    0x0150 DotRange Plain;
    0x0151 TargetPrinter Text;
    0x0152 ExtraSamples Coded("extra-samples", &[0, 1, 2]);
    0x0153 SampleFormat Coded("sample-format", &[1, 2, 3, 4]);
    0x0154 SMinSampleValue Plain;
    0x0155 SMaxSampleValue Plain;
    0x0156 TransferRange Plain;
    0x0157 ClipPath Internal;
    0x0158 XClipPathUnits Plain;
    0x0159 YClipPathUnits Plain;
    0x015A Indexed Plain;
    0x015B JPEGTables Internal;
    0x015F OPIProxy Plain;
    0x0200 JPEGProc Plain;
    0x0203 JPEGRestartInterval Plain;
    0x0205 JPEGLosslessPredictors Plain;
    0x0206 JPEGPointTransforms Plain;
    0x0207 JPEGQTables Internal;
    0x0208 JPEGDCTables Internal;
    0x0209 JPEGACTables Internal;
    // XMP packet in TIFF (XMP spec part 3); its properties are listed under XMP.
    0x02BC XMLPacket Internal;
    // Microsoft Windows properties.
    0x4746 Rating Rating;
    0x4747 XP_DIP_XML Internal;
    0x4749 RatingPercent Plain;
    0x800D ImageID Text;
    0x827D Model2 Text;
    // TIFF/EP (ISO 12234-2) IFD0 tags.
    0x828D CFARepeatPatternDim Plain;
    0x828E CFAPattern Plain;
    0x828F BatteryLevel Plain;
    // GeoTIFF.
    0x830E ModelPixelScaleTag Plain;
    // IPTC-NAA record, Photoshop image resources: shown by their own readers, not here.
    0x83BB IPTCNAA Internal;
    0x8482 ModelTiepointTag Plain;
    0x85D8 ModelTransformationTag Plain;
    0x8649 ImageResources Internal;
    // ICC profile in TIFF; its header is listed under "ICC profile header".
    0x8773 InterColorProfile Internal;
    0x87AF GeoKeyDirectoryTag Plain;
    0x87B0 GeoDoubleParamsTag Plain;
    0x87B1 GeoAsciiParamsTag Text;
    0x8829 Interlace Plain;
    0x882A TimeZoneOffset Plain;
    0x882B SelfTimerMode Plain;
    0x920B FlashEnergy Alias(Tag::FlashEnergy);
    0x920C SpatialFrequencyResponse Plain;
    0x920D Noise Plain;
    0x920E FocalPlaneXResolution Alias(Tag::FocalPlaneXResolution);
    0x920F FocalPlaneYResolution Alias(Tag::FocalPlaneYResolution);
    0x9210 FocalPlaneResolutionUnit Alias(Tag::FocalPlaneResolutionUnit);
    0x9211 ImageNumber Plain;
    0x9212 SecurityClassification Text;
    0x9213 ImageHistory Text;
    0x9215 ExposureIndex Alias(Tag::ExposureIndex);
    0x9216 TIFFEPStandardID Version;
    // TIFF/EP codes differ from Exif's SensingMethod (0xA217), so no alias.
    0x9217 SensingMethod Plain;
    // Xiaomi phones: a JSON settings blob and the model name, in IFD0.
    0x9999 XiaomiSettings Internal;
    0x9A00 XiaomiModel Text;
    // Windows Explorer properties (UTF-16LE).
    0x9C9B XPTitle Utf16;
    0x9C9C XPComment Utf16;
    0x9C9D XPAuthor Utf16;
    0x9C9E XPKeywords Utf16;
    0x9C9F XPSubject Utf16;
    // EXIF 3.0 (CIPA DC-008-2023).
    0xA436 ImageTitle Text;
    0xA437 Photographer Text;
    0xA438 ImageEditor Text;
    0xA439 CameraFirmware Text;
    0xA43A RAWDevelopingSoftware Text;
    0xA43B ImageEditingSoftware Text;
    0xA43C MetadataEditingSoftware Text;
    // GDAL (GeoTIFF writers).
    0xA480 GDALMetadata Text;
    0xA481 GDALNoData Text;
    // Epson Print Image Matching: printer settings, nothing about the picture.
    0xC4A5 PrintIM Internal;
    // Canon CR2 raw IFD.
    0xC5E0 CR2CFAPattern Plain;
    // DNG 1.0–1.7.
    0xC612 DNGVersion Version;
    0xC613 DNGBackwardVersion Version;
    0xC614 UniqueCameraModel Text;
    0xC615 LocalizedCameraModel Text;
    0xC616 CFAPlaneColor Plain;
    0xC617 CFALayout Plain;
    0xC618 LinearizationTable Plain;
    0xC619 BlackLevelRepeatDim Plain;
    0xC61A BlackLevel Plain;
    0xC61B BlackLevelDeltaH Plain;
    0xC61C BlackLevelDeltaV Plain;
    0xC61D WhiteLevel Plain;
    0xC61E DefaultScale Plain;
    0xC61F DefaultCropOrigin Plain;
    0xC620 DefaultCropSize Plain;
    0xC621 ColorMatrix1 Plain;
    0xC622 ColorMatrix2 Plain;
    0xC623 CameraCalibration1 Plain;
    0xC624 CameraCalibration2 Plain;
    0xC625 ReductionMatrix1 Plain;
    0xC626 ReductionMatrix2 Plain;
    0xC627 AnalogBalance Plain;
    0xC628 AsShotNeutral Plain;
    0xC629 AsShotWhiteXY Plain;
    0xC62A BaselineExposure Plain;
    0xC62B BaselineNoise Plain;
    0xC62C BaselineSharpness Plain;
    0xC62D BayerGreenSplit Plain;
    0xC62E LinearResponseLimit Plain;
    0xC62F CameraSerialNumber Text;
    0xC630 LensInfo Alias(Tag::LensSpecification);
    0xC631 ChromaBlurRadius Plain;
    0xC632 AntiAliasStrength Plain;
    0xC633 ShadowScale Plain;
    0xC634 DNGPrivateData Internal;
    0xC635 MakerNoteSafety Plain;
    0xC640 RawImageSegmentation Plain;
    0xC65A CalibrationIlluminant1 Alias(Tag::LightSource);
    0xC65B CalibrationIlluminant2 Alias(Tag::LightSource);
    0xC65C BestQualityScale Plain;
    0xC65D RawDataUniqueID Plain;
    0xC68B OriginalRawFileName Text;
    0xC68C OriginalRawFileData Internal;
    0xC68D ActiveArea Plain;
    0xC68E MaskedAreas Plain;
    0xC68F AsShotICCProfile Plain;
    0xC690 AsShotPreProfileMatrix Plain;
    0xC691 CurrentICCProfile Plain;
    0xC692 CurrentPreProfileMatrix Plain;
    0xC6BF ColorimetricReference Plain;
    0xC6C5 SRawType Plain;
    // Panasonic: the title set in camera, in IFD0.
    0xC6D2 PanasonicTitle Text;
    0xC6D3 PanasonicTitle2 Text;
    0xC6F3 CameraCalibrationSignature Text;
    0xC6F4 ProfileCalibrationSignature Text;
    0xC6F5 ExtraCameraProfiles Internal;
    0xC6F6 AsShotProfileName Text;
    0xC6F7 NoiseReductionApplied Plain;
    0xC6F8 ProfileName Text;
    0xC6F9 ProfileHueSatMapDims Plain;
    0xC6FA ProfileHueSatMapData1 Plain;
    0xC6FB ProfileHueSatMapData2 Plain;
    0xC6FC ProfileToneCurve Plain;
    0xC6FD ProfileEmbedPolicy Plain;
    0xC6FE ProfileCopyright Text;
    0xC714 ForwardMatrix1 Plain;
    0xC715 ForwardMatrix2 Plain;
    0xC716 PreviewApplicationName Text;
    0xC717 PreviewApplicationVersion Text;
    0xC718 PreviewSettingsName Text;
    0xC719 PreviewSettingsDigest Plain;
    0xC71A PreviewColorSpace Plain;
    0xC71B PreviewDateTime Text;
    0xC71C RawImageDigest Plain;
    0xC71D OriginalRawFileDigest Plain;
    0xC71E SubTileBlockSize Plain;
    0xC71F RowInterleaveFactor Plain;
    0xC725 ProfileLookTableDims Plain;
    0xC726 ProfileLookTableData Plain;
    0xC740 OpcodeList1 Plain;
    0xC741 OpcodeList2 Plain;
    0xC74E OpcodeList3 Plain;
    0xC761 NoiseProfile Plain;
    // CinemaDNG.
    0xC763 TimeCodes Plain;
    0xC764 FrameRate Plain;
    0xC772 TStop Plain;
    0xC789 ReelName Text;
    0xC791 OriginalDefaultFinalSize Plain;
    0xC792 OriginalBestQualityFinalSize Plain;
    0xC793 OriginalDefaultCropSize Plain;
    0xC7A1 CameraLabel Text;
    0xC7A3 ProfileHueSatMapEncoding Plain;
    0xC7A4 ProfileLookTableEncoding Plain;
    0xC7A5 BaselineExposureOffset Plain;
    0xC7A6 DefaultBlackRender Plain;
    0xC7A7 NewRawImageDigest Plain;
    0xC7A8 RawToPreviewGain Plain;
    0xC7B5 DefaultUserCrop Plain;
    0xC7E9 DepthFormat Plain;
    0xC7EA DepthNear Plain;
    0xC7EB DepthFar Plain;
    0xC7EC DepthUnits Plain;
    0xC7ED DepthMeasureType Plain;
    0xC7EE EnhanceParams Text;
    0xCD2D ProfileGainTableMap Plain;
    0xCD2E SemanticName Text;
    0xCD30 SemanticInstanceID Text;
    0xCD31 CalibrationIlluminant3 Alias(Tag::LightSource);
    0xCD32 CameraCalibration3 Plain;
    0xCD33 ColorMatrix3 Plain;
    0xCD34 ForwardMatrix3 Plain;
    0xCD35 IlluminantData1 Plain;
    0xCD36 IlluminantData2 Plain;
    0xCD37 IlluminantData3 Plain;
    0xCD38 MaskSubArea Plain;
    0xCD39 ProfileHueSatMapData3 Plain;
    0xCD3A ReductionMatrix3 Plain;
    0xCD3F RGBTables Plain;
    0xCD40 ProfileGainTableMap2 Plain;
    0xCD43 ColumnInterleaveFactor Plain;
    0xCD44 ImageSequenceInfo Plain;
    0xCD46 ImageStats Plain;
    0xCD47 ProfileDynamicRange Plain;
    0xCD48 ProfileGroupName Text;
    0xCD49 JXLDistance Plain;
    0xCD4A JXLEffort Plain;
    0xCD4B JXLDecodeSpeed Plain;
    // Space reserved by Windows writers for in-place edits.
    0xEA1C Padding Internal;
    0xEA1D OffsetSchema Internal;
};

/// Text in a field of any type: UTF-8 (EXIF 3.0 type 129, or BYTE/UNDEFINED/ASCII in
/// practice) with a Latin-1 fallback, cut at the first NUL after any leading ones (the rest is
/// what was left in the writer's buffer) and trimmed of spaces.
pub(crate) fn text(b: &[u8]) -> String {
    let start = b.iter().position(|&c| c != 0).unwrap_or(b.len());
    let b = b[start..].split(|&c| c == 0).next().unwrap_or_default();
    let s = match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        Err(_) => b.iter().map(|&c| c as char).collect(),
    };
    s.trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_owned()
}

/// UTF-16LE text, as Windows writes the XP* tags; stops at the first NUL.
pub(crate) fn utf16le(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units).trim().to_owned()
}

/// The bytes of an EXIF 3.0 UTF-8 (type 129) field, which `kamadak-exif` keeps as
/// `Value::Unknown(129, count, at)` where `at` is the entry's value/offset slot.
pub(crate) fn utf8_bytes(buf: &[u8], little_endian: bool, count: u32, at: u32) -> Option<&[u8]> {
    let (n, at) = (count as usize, at as usize);
    let start = if n <= 4 {
        at
    } else {
        let w: [u8; 4] = buf.get(at..at + 4)?.try_into().ok()?;
        (if little_endian {
            u32::from_le_bytes(w)
        } else {
            u32::from_be_bytes(w)
        }) as usize
    };
    buf.get(start..start.checked_add(n)?)
}

/// Serial numbers and the owner's name: CameraOwnerName (0xA430), BodySerialNumber (0xA431),
/// LensSerialNumber (0xA435) and DNG's CameraSerialNumber (0xC62F), in IFD0 or the Exif IFD.
/// They tie a picture to one camera and its owner and say nothing about the picture, so they
/// are not listed, like the serial numbers of the Canon maker note, which are never read.
pub(crate) fn identifies_someone(tag: Tag) -> bool {
    matches!(tag.context(), Context::Tiff | Context::Exif)
        && matches!(tag.number(), 0xA430 | 0xA431 | 0xA435 | 0xC62F)
}

/// The XMP equivalents of [`identifies_someone`] (EXIF 2.3 for XMP, Adobe's `aux`).
pub(crate) const XMP_IDENTIFYING: [&str; 6] = [
    "exifEX:CameraOwnerName",
    "exifEX:BodySerialNumber",
    "exifEX:LensSerialNumber",
    "aux:SerialNumber",
    "aux:LensSerialNumber",
    "aux:OwnerName",
];

/// EXIF 3.0 UTF-8 field type.
pub(crate) const TYPE_UTF8: u16 = 129;

/// Tag numbers the summary reads.
pub(crate) const RATING: u16 = 0x4746;
pub(crate) const XP_TITLE: u16 = 0x9C9B;
pub(crate) const XP_COMMENT: u16 = 0x9C9C;
pub(crate) const XP_AUTHOR: u16 = 0x9C9D;
pub(crate) const XP_KEYWORDS: u16 = 0x9C9E;
pub(crate) const IMAGE_TITLE: u16 = 0xA436;
pub(crate) const PHOTOGRAPHER: u16 = 0xA437;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_unique_and_disjoint_from_the_standard_tables() {
        for w in EXTRA.windows(2) {
            assert!(w[0].num < w[1].num, "0x{:04X} out of order", w[1].num);
        }
        for x in EXTRA {
            for ctx in [Context::Tiff, Context::Exif] {
                assert!(
                    Tag(ctx, x.num).description().is_none(),
                    "0x{:04X} {} is already a standard tag",
                    x.num,
                    x.name
                );
            }
        }
    }

    #[test]
    fn resolution_order() {
        // Standard in place.
        assert!(matches!(resolve(Tag::FNumber), Resolved::Std(t) if t == Tag::FNumber));
        // TIFF/EP: Exif tags in IFD0.
        assert!(
            matches!(resolve(Tag(Context::Tiff, 0x829D)), Resolved::Std(t) if t == Tag::FNumber)
        );
        // TIFF tags in the Exif IFD.
        assert!(
            matches!(resolve(Tag(Context::Exif, 0x0100)), Resolved::Std(t) if t == Tag::ImageWidth)
        );
        // Extra tags in either IFD.
        assert!(
            matches!(resolve(Tag(Context::Exif, 0xEA1C)), Resolved::Extra(x) if x.name == "Padding")
        );
        assert!(
            matches!(resolve(Tag(Context::Tiff, 0x9C9B)), Resolved::Extra(x) if x.name == "XPTitle")
        );
        // GPS numbers are their own space.
        assert!(matches!(
            resolve(Tag(Context::Gps, 0x0100)),
            Resolved::Unknown
        ));
        assert!(matches!(
            resolve(Tag(Context::Exif, 0xBEEF)),
            Resolved::Unknown
        ));
    }

    #[test]
    fn text_decoders() {
        let mut b: Vec<u8> = "Été 🌞".encode_utf16().flat_map(u16::to_le_bytes).collect();
        b.extend([0, 0, 0x41, 0]);
        assert_eq!(utf16le(&b), "Été 🌞");
        assert_eq!(utf16le(&[0x41, 0, 0x42]), "A");
        assert_eq!(text(b"caf\xC3\xA9\0\0"), "café");
        assert_eq!(text(b"caf\xE9"), "café");
        // Cut at the first NUL: Samsung's 20-byte Make.
        assert_eq!(text(b"SAMSUNG\0\x01\x02junk\0\0\0"), "SAMSUNG");
        assert_eq!(text(b"\0\0 Lens \0x"), "Lens");
        assert_eq!(text(b"\0\0"), "");
        // Inline (≤ 4 bytes) and by offset, both byte orders.
        let buf = [0, 0, b'a', b'b', 0, 0, 0, 0, 0, 6, 0, 0];
        assert_eq!(utf8_bytes(&buf, true, 2, 2), Some(&b"ab"[..]));
        let buf = b"\0\0\0\0\0\x08\0\0hello";
        assert_eq!(utf8_bytes(buf, false, 5, 2), Some(&b"hello"[..]));
        assert_eq!(utf8_bytes(buf, false, 9, 2), None);
    }
}
