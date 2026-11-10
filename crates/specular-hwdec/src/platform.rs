//! Hardware decoders on macOS: design and stub; Windows is done (`crate::d3d12`, D3D12 Video
//! Decode for HEVC stills and H.264 / HEVC video; the notes below are what is still to come
//! there: AV1, JPEG, and the decoder on wgpu's own device).
//!
//! macOS is not implemented yet (no machine to test on); [`probe`] reports nothing, so every
//! decode stays in software there. The plan, so the FFI lands in one module per platform with
//! the same shape as `vaapi`:
//!
//! **Windows (Media Foundation / D3D12 Video / WIC).**
//! * HEVC (HEIC) and AV1 (AVIF): `ID3D12VideoDecoder` on the device wgpu already created (wgpu-hal
//!   gives the raw `ID3D12Device`), so the decoded NV12/P010 texture is shared with wgpu without a
//!   copy. We parse the HEIF/AVIF container and the codec headers (SPS/PPS/slice headers,
//!   AV1 sequence/frame headers) in Rust and fill `DXVA_PicParams_HEVC` / `DXVA_PicParams_AV1`;
//!   the driver only sees parameter structs and slice data, as with VA-API.
//! * JPEG: `IMFTransform` MJPEG decoder (DXVA where the GPU offers it, otherwise Microsoft's
//!   software MFT, which is slower than ours: the cost model only picks it when DXVA is present).
//! * WIC is not used for decoding: its codecs parse the whole file in-process (C++), which
//!   DESIGN.md §5 allows only in the sandbox host, and they are not faster than ours.
//! * Bindings: the `windows` crate (MIT/Apache, no build-time C), feature-gated to the D3D12
//!   video and Media Foundation interfaces.
//!
//! **macOS (VideoToolbox / ImageIO).**
//! * HEVC and AV1 (M3 and later): `VTDecompressionSession` with a `CMVideoFormatDescription`
//!   built from our parsed `hvcC` / `av1C`, output `CVPixelBuffer`s backed by IOSurface, which
//!   Metal (and so wgpu-hal) can wrap as a texture without a copy.
//! * JPEG: VideoToolbox has no still-JPEG decoder; Apple's hardware JPEG is reached only through
//!   ImageIO (`CGImageSourceCreateWithData` + `kCGImageSourceUseHardwareAcceleration`), which
//!   parses the file in Apple's C code. That needs the sandbox host, which costs the zero-copy
//!   path; our NEON decoder is expected to win below ~50 MP on M-series cores anyway.
//! * Bindings: `objc2-video-toolbox` / `objc2-core-video` (MIT/Apache, no build-time C).
//!
//! Both follow the same rules as VA-API: off by default, per-decode fallback to software on any
//! error, and the first decode of a session checked against software
//! ([`check_quality`](crate::check_quality)).

use crate::Capabilities;

/// Hardware decoders of this platform (only reached on platforms without VA-API or D3D12).
#[allow(dead_code)]
pub fn probe() -> Option<Capabilities> {
    #[cfg(target_os = "macos")]
    {
        macos::probe()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::Capabilities;

    /// Not implemented: would check `VTIsHardwareDecodeSupported` for `kCMVideoCodecType_HEVC`
    /// and `kCMVideoCodecType_AV1`.
    pub fn probe() -> Option<Capabilities> {
        None
    }
}
