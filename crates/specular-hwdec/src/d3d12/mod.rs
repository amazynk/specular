//! D3D12 Video Decode on Windows: the GPU's fixed-function decoders through the API Windows
//! ships, for HEIC ([`crate::heic`]) and for motion-photo clips ([`video`]). The parameter
//! sets and slice headers are parsed in Rust ([`crate::hevc`], [`crate::hevc_video`],
//! [`crate::h264`]); the driver gets the DXVA parameter structs ([`dxva`]) and the slice bytes,
//! like VA-API's. Each decoded NV12 / P010 texture is copied into a read-back buffer on the copy
//! queue and deinterleaved into planes, as the VA-API path reads its surfaces back with
//! `vaGetImage`. AV1 is probed and reported; JPEG has no D3D12 decoder.
//!
//! **Reference-only allocations.** Some drivers (AMD) decode only into textures flagged
//! reference-only, which nothing else may read; the picture then reaches an ordinary texture
//! through the decoder's own output conversion, set to the same format and colour space (a
//! copy). Each pool slot has such a texture when the driver asks for it.
//!
//! The device is one of its own on the adapter wgpu would choose (the high-performance one).
//! Decoding straight into a texture wgpu samples would save the copy out, but needs the
//! `ID3D12Device` behind wgpu-hal; not yet.
//!
//! Every call into D3D12 is `unsafe` in windows-rs; each is one statement with its reasoning
//! (UNSAFE.md). Objects are windows-rs smart pointers, released on drop; an [`Engine`] waits
//! for the GPU before its resources go.

mod dxva;
pub mod video;

use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::{
    D3D12_COMMAND_LIST_TYPE, D3D12_COMMAND_LIST_TYPE_COPY, D3D12_COMMAND_LIST_TYPE_VIDEO_DECODE,
    D3D12_COMMAND_QUEUE_DESC, D3D12_COMMAND_QUEUE_FLAG_NONE, D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
    D3D12_FENCE_FLAG_NONE, D3D12_HEAP_FLAG_NONE, D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE,
    D3D12_HEAP_TYPE_DEFAULT, D3D12_HEAP_TYPE_READBACK, D3D12_HEAP_TYPE_UPLOAD,
    D3D12_MEMORY_POOL_UNKNOWN, D3D12_PLACED_SUBRESOURCE_FOOTPRINT, D3D12_RANGE,
    D3D12_RESOURCE_BARRIER, D3D12_RESOURCE_BARRIER_0, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
    D3D12_RESOURCE_BARRIER_FLAG_NONE, D3D12_RESOURCE_BARRIER_TYPE_TRANSITION, D3D12_RESOURCE_DESC,
    D3D12_RESOURCE_DIMENSION_BUFFER, D3D12_RESOURCE_DIMENSION_TEXTURE2D,
    D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE, D3D12_RESOURCE_FLAG_NONE,
    D3D12_RESOURCE_FLAG_VIDEO_DECODE_REFERENCE_ONLY, D3D12_RESOURCE_FLAGS,
    D3D12_RESOURCE_STATE_COMMON, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_GENERIC_READ,
    D3D12_RESOURCE_STATE_VIDEO_DECODE_READ, D3D12_RESOURCE_STATE_VIDEO_DECODE_WRITE,
    D3D12_RESOURCE_STATES, D3D12_RESOURCE_TRANSITION_BARRIER, D3D12_TEXTURE_COPY_LOCATION,
    D3D12_TEXTURE_COPY_LOCATION_0, D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
    D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
    D3D12_TEXTURE_LAYOUT_UNKNOWN, D3D12CreateDevice, ID3D12CommandAllocator, ID3D12CommandList,
    ID3D12CommandQueue, ID3D12Device, ID3D12Fence, ID3D12GraphicsCommandList, ID3D12Resource,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_TYPE, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601,
    DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020,
    DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020, DXGI_FORMAT, DXGI_FORMAT_NV12,
    DXGI_FORMAT_P010, DXGI_FORMAT_UNKNOWN, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS,
    DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE, IDXGIAdapter1, IDXGIFactory6,
};
use windows::Win32::Media::MediaFoundation::{
    D3D12_BITSTREAM_ENCRYPTION_TYPE_NONE, D3D12_FEATURE_DATA_VIDEO_DECODE_CONVERSION_SUPPORT,
    D3D12_FEATURE_DATA_VIDEO_DECODE_SUPPORT, D3D12_FEATURE_VIDEO_DECODE_CONVERSION_SUPPORT,
    D3D12_FEATURE_VIDEO_DECODE_SUPPORT,
    D3D12_VIDEO_DECODE_ARGUMENT_TYPE_INVERSE_QUANTIZATION_MATRIX,
    D3D12_VIDEO_DECODE_ARGUMENT_TYPE_PICTURE_PARAMETERS,
    D3D12_VIDEO_DECODE_ARGUMENT_TYPE_SLICE_CONTROL, D3D12_VIDEO_DECODE_COMPRESSED_BITSTREAM,
    D3D12_VIDEO_DECODE_CONFIGURATION,
    D3D12_VIDEO_DECODE_CONFIGURATION_FLAG_REFERENCE_ONLY_ALLOCATIONS_REQUIRED,
    D3D12_VIDEO_DECODE_CONVERSION_ARGUMENTS, D3D12_VIDEO_DECODE_CONVERSION_SUPPORT_FLAG_SUPPORTED,
    D3D12_VIDEO_DECODE_FRAME_ARGUMENT, D3D12_VIDEO_DECODE_INPUT_STREAM_ARGUMENTS,
    D3D12_VIDEO_DECODE_OUTPUT_STREAM_ARGUMENTS, D3D12_VIDEO_DECODE_PROFILE_AV1_PROFILE0,
    D3D12_VIDEO_DECODE_PROFILE_H264, D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN,
    D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10, D3D12_VIDEO_DECODE_REFERENCE_FRAMES,
    D3D12_VIDEO_DECODE_SUPPORT_FLAG_SUPPORTED, D3D12_VIDEO_DECODER_DESC,
    D3D12_VIDEO_DECODER_HEAP_DESC, D3D12_VIDEO_FORMAT, D3D12_VIDEO_FRAME_CODED_INTERLACE_TYPE_NONE,
    D3D12_VIDEO_SAMPLE, ID3D12VideoDecodeCommandList, ID3D12VideoDecoder, ID3D12VideoDecoderHeap,
    ID3D12VideoDevice,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::{GUID, Interface};

use crate::hevc::{Limits, Picture};
use crate::params::{HevcPlanes, planes_from_420};
use crate::{Capabilities, CodecCaps, Error, HevcDevice, SoftwareOnly};
use dxva::Blob;

fn win(e: windows::core::Error) -> Error {
    Error::Hardware(e.message())
}

/// Luma widths are allocated in multiples of this, heights in [`ALIGN_H`] (what every D3D12
/// decoder accepts; some require the height one).
pub(super) const ALIGN_W: u32 = 16;
pub(super) const ALIGN_H: u32 = 32;
/// The bitstream upload buffer's first size (grown when a picture needs more).
const BITSTREAM_START: u64 = 1 << 20;
/// How long an engine waits for the GPU before releasing its resources, and for a slot to be
/// reused.
const RELEASE_WAIT: Duration = Duration::from_secs(2);

/// A D3D12 device with its video decoder, for still pictures (HEIC).
pub struct Device {
    device: ID3D12Device,
    video: ID3D12VideoDevice,
    hevc: Option<HevcHw>,
    /// `StatusReportFeedbackNumber` of the next submission (nonzero).
    status: u32,
}

/// How one decode profile is driven on this device.
#[derive(Clone, Copy, Debug)]
pub(super) struct Mode {
    /// Decode into a reference-only texture and convert into the output (see the module docs).
    pub ref_only: bool,
    /// The colour space the conversion is declared in (the same in and out).
    pub color_space: DXGI_COLOR_SPACE_TYPE,
}

/// The HEVC decoder's limits, how its profiles are driven, and the current picture pool.
struct HevcHw {
    lim: Limits,
    main: Mode,
    main10: Option<Mode>,
    pool: Option<Pool>,
}

fn config(profile: GUID) -> D3D12_VIDEO_DECODE_CONFIGURATION {
    D3D12_VIDEO_DECODE_CONFIGURATION {
        DecodeProfile: profile,
        BitstreamEncryption: D3D12_BITSTREAM_ENCRYPTION_TYPE_NONE,
        InterlaceType: D3D12_VIDEO_FRAME_CODED_INTERLACE_TYPE_NONE,
    }
}

const FRAME_RATE: DXGI_RATIONAL = DXGI_RATIONAL {
    Numerator: 30,
    Denominator: 1,
};

/// Whether `video` decodes `profile` into `format` at `w`×`h`, with what the driver says about
/// it.
fn support(
    video: &ID3D12VideoDevice,
    profile: GUID,
    format: DXGI_FORMAT,
    w: u32,
    h: u32,
) -> Option<D3D12_FEATURE_DATA_VIDEO_DECODE_SUPPORT> {
    let mut d = D3D12_FEATURE_DATA_VIDEO_DECODE_SUPPORT {
        NodeIndex: 0,
        Configuration: config(profile),
        Width: w,
        Height: h,
        DecodeFormat: format,
        FrameRate: FRAME_RATE,
        BitRate: 0,
        ..Default::default()
    };
    #[allow(unsafe_code)]
    // SAFETY: the struct outlives the call and its size is passed with it; the driver fills
    // its output fields in place.
    let r = unsafe {
        video.CheckFeatureSupport(
            D3D12_FEATURE_VIDEO_DECODE_SUPPORT,
            (&raw mut d).cast::<c_void>(),
            size_of::<D3D12_FEATURE_DATA_VIDEO_DECODE_SUPPORT>() as u32,
        )
    };
    r.ok()?;
    (d.SupportFlags.0 & D3D12_VIDEO_DECODE_SUPPORT_FLAG_SUPPORTED.0 != 0).then_some(d)
}

/// Whether the decoder's output conversion takes `format` in colour space `cs` to the same
/// (the copy a reference-only driver needs).
fn conversion(
    video: &ID3D12VideoDevice,
    profile: GUID,
    format: DXGI_FORMAT,
    cs: DXGI_COLOR_SPACE_TYPE,
) -> bool {
    let vf = D3D12_VIDEO_FORMAT {
        Format: format,
        ColorSpace: cs,
    };
    let mut d = D3D12_FEATURE_DATA_VIDEO_DECODE_CONVERSION_SUPPORT {
        NodeIndex: 0,
        Configuration: config(profile),
        DecodeSample: D3D12_VIDEO_SAMPLE {
            Width: 1920,
            Height: 1088,
            Format: vf,
        },
        OutputFormat: vf,
        FrameRate: FRAME_RATE,
        BitRate: 0,
        ..Default::default()
    };
    #[allow(unsafe_code)]
    // SAFETY: as in `support`.
    let r = unsafe {
        video.CheckFeatureSupport(
            D3D12_FEATURE_VIDEO_DECODE_CONVERSION_SUPPORT,
            (&raw mut d).cast::<c_void>(),
            size_of::<D3D12_FEATURE_DATA_VIDEO_DECODE_CONVERSION_SUPPORT>() as u32,
        )
    };
    r.is_ok() && d.SupportFlags.0 & D3D12_VIDEO_DECODE_CONVERSION_SUPPORT_FLAG_SUPPORTED.0 != 0
}

/// How `profile` is driven and the largest width and height it decodes (each probed with the
/// other at 1080p), when it decodes 1080p at all.
pub(super) fn probe_profile(
    video: &ID3D12VideoDevice,
    profile: GUID,
    format: DXGI_FORMAT,
    ten_bit: bool,
) -> Option<(Mode, u32, u32)> {
    let base = support(video, profile, format, 1920, 1088)?;
    let ref_only = base.ConfigurationFlags.0
        & D3D12_VIDEO_DECODE_CONFIGURATION_FLAG_REFERENCE_ONLY_ALLOCATIONS_REQUIRED.0
        != 0;
    // The colour space is a label for a conversion that converts nothing; the driver must
    // accept one.
    let candidates: &[DXGI_COLOR_SPACE_TYPE] = if ten_bit {
        &[
            DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020,
            DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
            DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020,
        ]
    } else {
        &[
            DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
            DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601,
            DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020,
        ]
    };
    let color_space = if ref_only {
        *candidates
            .iter()
            .find(|&&cs| conversion(video, profile, format, cs))?
    } else {
        candidates[0]
    };
    let mut max_w = 1920;
    for w in [4096, 8192, 16384] {
        if support(video, profile, format, w, 1088).is_none() {
            break;
        }
        max_w = w;
    }
    let mut max_h = 1088;
    for h in [2304, 4352, 8192, 16384] {
        if support(video, profile, format, 1920, h).is_none() {
            break;
        }
        max_h = h;
    }
    Some((
        Mode {
            ref_only,
            color_space,
        },
        max_w,
        max_h,
    ))
}

/// The first non-software adapter, high-performance ones first, whose D3D12 device has a video
/// decoder: the device, its video interface and the adapter's name.
pub(super) fn open_adapter() -> Result<(ID3D12Device, ID3D12VideoDevice, String), String> {
    let err = |e: windows::core::Error| e.message();
    #[allow(unsafe_code)]
    // SAFETY: creates the DXGI factory with no flags; nothing of ours is referenced.
    let factory: IDXGIFactory6 =
        unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }.map_err(err)?;
    let mut i = 0;
    loop {
        #[allow(unsafe_code)]
        // SAFETY: enumerates by index; past the end it fails, which ends the loop.
        let adapter: IDXGIAdapter1 = match unsafe {
            factory.EnumAdapterByGpuPreference(i, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE)
        } {
            Ok(a) => a,
            Err(_) => return Err("no GPU with a D3D12 video decoder".into()),
        };
        i += 1;
        #[allow(unsafe_code)]
        // SAFETY: fills a descriptor returned by value.
        let desc = unsafe { adapter.GetDesc1() }.map_err(err)?;
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let mut created: Option<ID3D12Device> = None;
        #[allow(unsafe_code)]
        // SAFETY: the adapter is live; the device comes back in `created`.
        let r = unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut created) };
        let Some(device) = r.ok().and(created) else {
            continue;
        };
        let Ok(video) = device.cast::<ID3D12VideoDevice>() else {
            continue;
        };
        let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(128);
        let name = String::from_utf16_lossy(&desc.Description[..len]);
        return Ok((device, video, name));
    }
}

impl Device {
    /// Opens the adapter ([`open_adapter`]) and probes what it decodes; its capabilities with
    /// it.
    pub fn open() -> Result<(Device, Capabilities), String> {
        let (device, video, name) = open_adapter()?;
        let (caps, hevc) = probe(&video, name);
        if caps.hevc.is_none() && caps.h264.is_none() && caps.others.is_empty() {
            return Err("the GPU has no video decoder".into());
        }
        Ok((
            Device {
                device,
                video,
                hevc,
                status: 1,
            },
            caps,
        ))
    }

    /// The HEVC decoder's limits (`None`: no HEVC Main decoder).
    pub fn hevc_limits(&self) -> Option<Limits> {
        self.hevc.as_ref().map(|h| h.lim)
    }

    /// Makes the pool `n` pictures of `w`×`h` (coded size), 8- or 10-bit, keeping the current
    /// one when it matches.
    pub fn hevc_prepare(&mut self, w: u32, h: u32, ten_bit: bool, n: usize) -> Result<(), Error> {
        let hw = self.hevc.as_mut().ok_or(Error::Unavailable)?;
        if let Some(p) = &hw.pool
            && (p.w, p.h, p.ten_bit) == (w, h, ten_bit)
            && p.slots.len() >= n
        {
            return Ok(());
        }
        hw.pool = None;
        let mode = if ten_bit {
            hw.main10
                .ok_or_else(|| Error::Unsupported("no HEVC Main 10 decoder".into()))?
        } else {
            hw.main
        };
        hw.pool = Some(Pool::new(
            &self.device,
            &self.video,
            w,
            h,
            ten_bit,
            mode,
            n.max(1),
        )?);
        Ok(())
    }

    /// Frees the pool (the decoder's limits stay).
    pub fn hevc_release(&mut self) {
        if let Some(hw) = self.hevc.as_mut() {
            hw.pool = None;
        }
    }

    fn pool(&mut self) -> Result<&mut Pool, Error> {
        self.hevc
            .as_mut()
            .and_then(|h| h.pool.as_mut())
            .ok_or_else(|| Error::Hardware("no HEVC picture pool".into()))
    }

    /// Queues `pic` for decoding into pool slot `slot` and its copy-out (does not wait).
    pub fn hevc_submit(&mut self, pic: &Picture<'_>, slot: usize) -> Result<(), Error> {
        let status = self.status;
        self.status = self.status.wrapping_add(1).max(1);
        let device = self.device.clone();
        let p = self.pool()?;
        if (pic.coded_w, pic.coded_h, pic.bit_depth > 8) != (p.w, p.h, p.ten_bit) {
            return Err(Error::Hardware(
                "picture does not match the picture pool".into(),
            ));
        }
        p.submit(&device, pic, slot % p.slots.len(), status)
    }

    /// Waits (at most `timeout_ns`) for pool slot `slot` and reads the `w`×`h` region at
    /// (`x`, `y`) back into planes. `x` and `y` are even.
    pub fn hevc_read(
        &mut self,
        slot: usize,
        timeout_ns: u64,
        region: (u32, u32, u32, u32),
    ) -> Result<HevcPlanes, Error> {
        let p = self.pool()?;
        p.read(
            slot % p.slots.len(),
            Duration::from_nanos(timeout_ns),
            region,
        )
    }
}

impl HevcDevice for Device {
    fn hevc_limits(&self) -> Option<Limits> {
        Device::hevc_limits(self)
    }
    fn hevc_prepare(&mut self, w: u32, h: u32, ten_bit: bool, n: usize) -> Result<(), Error> {
        Device::hevc_prepare(self, w, h, ten_bit, n)
    }
    fn hevc_release(&mut self) {
        Device::hevc_release(self);
    }
    fn hevc_submit(&mut self, pic: &Picture<'_>, slot: usize) -> Result<(), Error> {
        Device::hevc_submit(self, pic, slot)
    }
    fn hevc_read(
        &mut self,
        slot: usize,
        timeout_ns: u64,
        region: (u32, u32, u32, u32),
    ) -> Result<HevcPlanes, Error> {
        Device::hevc_read(self, slot, timeout_ns, region)
    }
}

/// What the adapter decodes, for the hardware report and the path selector.
fn probe(video: &ID3D12VideoDevice, name: String) -> (Capabilities, Option<HevcHw>) {
    let main = probe_profile(
        video,
        D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN,
        DXGI_FORMAT_NV12,
        false,
    );
    let main10 = probe_profile(
        video,
        D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10,
        DXGI_FORMAT_P010,
        true,
    );
    let h264 = probe_profile(
        video,
        D3D12_VIDEO_DECODE_PROFILE_H264,
        DXGI_FORMAT_NV12,
        false,
    );
    let hevc = main.map(|(mode, max_w, max_h)| HevcHw {
        lim: Limits {
            max_w,
            max_h,
            main10: main10.is_some(),
        },
        main: mode,
        main10: main10.map(|(m, _, _)| m),
        pool: None,
    });
    let mut others = Vec::new();
    // A decoder the hardware has that nothing drives yet (AVIF decodes in software).
    if support(
        video,
        D3D12_VIDEO_DECODE_PROFILE_AV1_PROFILE0,
        DXGI_FORMAT_NV12,
        1920,
        1088,
    )
    .is_some()
    {
        others.push("AV1 (not used yet)");
    }
    // The stream formats the decoders here refuse ([`crate::h264`] takes 8-bit 4:2:0
    // frames; [`crate::hevc_video`] 4:2:0 Main and Main 10; interlaced H.264 would need a
    // field-based decoder configuration).
    let mut software_only = Vec::new();
    const NOT_HERE: &str = "not decoded in hardware";
    if h264.is_some() {
        for format in ["H.264 High 10", "H.264 4:2:2", "H.264 interlaced"] {
            software_only.push(SoftwareOnly {
                format,
                reason: NOT_HERE,
            });
        }
    }
    if main.is_some() {
        for format in ["HEVC 4:2:2", "HEVC 4:4:4"] {
            software_only.push(SoftwareOnly {
                format,
                reason: NOT_HERE,
            });
        }
    }
    let caps = Capabilities {
        api: "D3D12 Video",
        device: name,
        jpeg: None,
        h264: h264.map(|(_, max_width, max_height)| CodecCaps {
            max_width,
            max_height,
            formats: "4:2:0 8-bit".into(),
        }),
        hevc: main.map(|(_, max_width, max_height)| CodecCaps {
            max_width,
            max_height,
            formats: if main10.is_some() {
                "4:2:0 8-bit, 10-bit".into()
            } else {
                "4:2:0 8-bit".into()
            },
        }),
        av1: None,
        // The encoder is Media Foundation's, not D3D12's: the GPU driver's MFT.
        h264_encode: crate::mf_enc::caps(),
        others,
        software_only,
    };
    (caps, hevc)
}

/// An event handle, closed on drop. Kept as the integer it is (a `HANDLE` wraps a raw
/// pointer, which would make the device `!Send`; the device lives in a static behind a mutex).
struct Event(isize);

impl Event {
    fn handle(&self) -> HANDLE {
        HANDLE(self.0 as *mut c_void)
    }
}

impl Drop for Event {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateEventW and is closed exactly once, here.
        let _ = unsafe { CloseHandle(self.handle()) };
    }
}

fn heap_props(kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES {
        Type: kind,
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 1,
        VisibleNodeMask: 1,
    }
}

fn buffer_desc(len: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: len,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    }
}

pub(super) fn texture_desc(
    tw: u32,
    th: u32,
    format: DXGI_FORMAT,
    flags: D3D12_RESOURCE_FLAGS,
) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: u64::from(tw),
        Height: th,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: flags,
    }
}

/// A committed resource of `desc` on a heap of `kind`, in `state`.
fn committed(
    device: &ID3D12Device,
    kind: D3D12_HEAP_TYPE,
    desc: &D3D12_RESOURCE_DESC,
    state: D3D12_RESOURCE_STATES,
) -> Result<ID3D12Resource, Error> {
    let props = heap_props(kind);
    let mut made: Option<ID3D12Resource> = None;
    #[allow(unsafe_code)]
    // SAFETY: the descriptors outlive the call; the resource comes back in `made`.
    let r = unsafe {
        device.CreateCommittedResource(&props, D3D12_HEAP_FLAG_NONE, desc, state, None, &mut made)
    };
    r.map_err(win)?;
    made.ok_or_else(|| Error::Hardware("no resource".into()))
}

fn queue(
    device: &ID3D12Device,
    kind: D3D12_COMMAND_LIST_TYPE,
) -> Result<ID3D12CommandQueue, Error> {
    let desc = D3D12_COMMAND_QUEUE_DESC {
        Type: kind,
        Priority: 0,
        Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
        NodeMask: 0,
    };
    #[allow(unsafe_code)]
    // SAFETY: the descriptor outlives the call.
    unsafe { device.CreateCommandQueue(&desc) }.map_err(win)
}

/// A transition barrier on the whole of `r`.
fn transition(
    r: &ID3D12Resource,
    from: D3D12_RESOURCE_STATES,
    to: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: ManuallyDrop::new(Some(r.clone())),
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: from,
                StateAfter: to,
            }),
        },
    }
}

/// Releases the resource reference a [`transition`] holds.
#[allow(unsafe_code)]
fn release_transition(b: D3D12_RESOURCE_BARRIER) {
    // SAFETY: `b` came from `transition`, so its union holds the transition member; reading
    // it moves the member out, and nothing reads `b` again.
    let t = unsafe { ManuallyDrop::into_inner(b.Anonymous.Transition) };
    drop(ManuallyDrop::into_inner(t.pResource));
}

/// The textures of one decoded picture buffer slot: the output and, where the driver decodes
/// into one, the reference-only texture.
pub(super) struct Textures {
    pub texture: ID3D12Resource,
    pub reference: Option<ID3D12Resource>,
}

impl Textures {
    pub fn new(
        device: &ID3D12Device,
        tw: u32,
        th: u32,
        format: DXGI_FORMAT,
        ref_only: bool,
    ) -> Result<Textures, Error> {
        let desc = texture_desc(tw, th, format, D3D12_RESOURCE_FLAG_NONE);
        let texture = committed(
            device,
            D3D12_HEAP_TYPE_DEFAULT,
            &desc,
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        let reference = if ref_only {
            let desc = texture_desc(
                tw,
                th,
                format,
                D3D12_RESOURCE_FLAG_VIDEO_DECODE_REFERENCE_ONLY
                    | D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE,
            );
            Some(committed(
                device,
                D3D12_HEAP_TYPE_DEFAULT,
                &desc,
                D3D12_RESOURCE_STATE_COMMON,
            )?)
        } else {
            None
        };
        Ok(Textures { texture, reference })
    }

    /// The texture the decoder decodes into (and reads references from).
    fn target(&self) -> &ID3D12Resource {
        self.reference.as_ref().unwrap_or(&self.texture)
    }
}

/// What one decode needs besides its textures: the bitstream and read-back buffers and the
/// command lists.
pub(super) struct Scratch {
    bitstream: ID3D12Resource,
    bitstream_len: u64,
    readback: ID3D12Resource,
    readback_len: u64,
    layout: [D3D12_PLACED_SUBRESOURCE_FOOTPRINT; 2],
    decode_alloc: ID3D12CommandAllocator,
    decode_list: ID3D12VideoDecodeCommandList,
    copy_alloc: ID3D12CommandAllocator,
    copy_list: ID3D12GraphicsCommandList,
}

impl Scratch {
    /// Buffers and lists for pictures of `tex_desc`.
    pub fn new(device: &ID3D12Device, tex_desc: &D3D12_RESOURCE_DESC) -> Result<Scratch, Error> {
        let mut layout = [D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default(); 2];
        let mut rows = [0u32; 2];
        let mut row_bytes = [0u64; 2];
        let mut total = 0u64;
        #[allow(unsafe_code)]
        // SAFETY: the arrays hold two entries, one per subresource asked for (the luma and
        // chroma planes), and the descriptor outlives the call.
        unsafe {
            device.GetCopyableFootprints(
                tex_desc,
                0,
                2,
                0,
                Some(layout.as_mut_ptr()),
                Some(rows.as_mut_ptr()),
                Some(row_bytes.as_mut_ptr()),
                Some(&mut total),
            )
        };
        let readback = committed(
            device,
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(total),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let bitstream = committed(
            device,
            D3D12_HEAP_TYPE_UPLOAD,
            &buffer_desc(BITSTREAM_START),
            D3D12_RESOURCE_STATE_GENERIC_READ,
        )?;
        #[allow(unsafe_code)]
        // SAFETY: plain object creation.
        let decode_alloc: ID3D12CommandAllocator =
            unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_VIDEO_DECODE) }
                .map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: the allocator is live; no pipeline state for a video decode list.
        let decode_list: ID3D12VideoDecodeCommandList = unsafe {
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_VIDEO_DECODE, &decode_alloc, None)
        }
        .map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: a new list is open; closed until it records.
        unsafe { decode_list.Close() }.map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: plain object creation.
        let copy_alloc: ID3D12CommandAllocator =
            unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_COPY) }.map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: the allocator is live; no pipeline state for a copy list.
        let copy_list: ID3D12GraphicsCommandList =
            unsafe { device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_COPY, &copy_alloc, None) }
                .map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: as above.
        unsafe { copy_list.Close() }.map_err(win)?;
        Ok(Scratch {
            bitstream,
            bitstream_len: BITSTREAM_START,
            readback,
            readback_len: total,
            layout,
            decode_alloc,
            decode_list,
            copy_alloc,
            copy_list,
        })
    }

    /// Writes `buf` into the bitstream buffer, growing it when needed.
    fn upload(&mut self, device: &ID3D12Device, buf: &[u8]) -> Result<(), Error> {
        let len = buf.len() as u64;
        if len > self.bitstream_len {
            let want = len.next_power_of_two();
            self.bitstream = committed(
                device,
                D3D12_HEAP_TYPE_UPLOAD,
                &buffer_desc(want),
                D3D12_RESOURCE_STATE_GENERIC_READ,
            )?;
            self.bitstream_len = want;
        }
        let mut p: *mut c_void = std::ptr::null_mut();
        let none = D3D12_RANGE { Begin: 0, End: 0 };
        #[allow(unsafe_code)]
        // SAFETY: maps the upload buffer (host-visible) without reading it back.
        unsafe { self.bitstream.Map(0, Some(&none), Some(&mut p)) }.map_err(win)?;
        if p.is_null() {
            return Err(Error::Hardware("bitstream buffer not mapped".into()));
        }
        #[allow(unsafe_code)]
        // SAFETY: the mapping is at least `bitstream_len` ≥ `buf.len()` bytes and nothing else
        // writes it; the two ranges cannot overlap.
        unsafe {
            std::ptr::copy_nonoverlapping(buf.as_ptr(), p.cast::<u8>(), buf.len())
        };
        let written = D3D12_RANGE {
            Begin: 0,
            End: buf.len(),
        };
        #[allow(unsafe_code)]
        // SAFETY: balances the Map, telling the driver what was written.
        unsafe {
            self.bitstream.Unmap(0, Some(&written))
        };
        Ok(())
    }

    /// Records the decode of `args` into `out`: `write` the textures it writes, `read` the
    /// ones it references.
    fn record_decode(
        &self,
        decoder: &ID3D12VideoDecoder,
        out: &D3D12_VIDEO_DECODE_OUTPUT_STREAM_ARGUMENTS,
        args: &D3D12_VIDEO_DECODE_INPUT_STREAM_ARGUMENTS,
        write: &[&ID3D12Resource],
        read: &[&ID3D12Resource],
    ) -> Result<(), Error> {
        #[allow(unsafe_code)]
        // SAFETY: the allocator's last list has executed (the caller waited for it).
        unsafe { self.decode_alloc.Reset() }.map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: the list was closed; it reopens on its own allocator.
        unsafe { self.decode_list.Reset(&self.decode_alloc) }.map_err(win)?;
        let before: Vec<D3D12_RESOURCE_BARRIER> = write
            .iter()
            .map(|t| {
                transition(
                    t,
                    D3D12_RESOURCE_STATE_COMMON,
                    D3D12_RESOURCE_STATE_VIDEO_DECODE_WRITE,
                )
            })
            .chain(read.iter().map(|t| {
                transition(
                    t,
                    D3D12_RESOURCE_STATE_COMMON,
                    D3D12_RESOURCE_STATE_VIDEO_DECODE_READ,
                )
            }))
            .collect();
        #[allow(unsafe_code)]
        // SAFETY: the barriers name live textures of the pool.
        unsafe {
            self.decode_list.ResourceBarrier(&before)
        };
        #[allow(unsafe_code)]
        // SAFETY: both argument structs are live for the call and point at buffers (the
        // parameter structs, the slice controls, the bitstream and texture resources) that stay
        // valid until the list has executed; the driver copies the parameters when recording.
        unsafe {
            self.decode_list.DecodeFrame(decoder, out, args)
        };
        let after: Vec<D3D12_RESOURCE_BARRIER> = write
            .iter()
            .map(|t| {
                transition(
                    t,
                    D3D12_RESOURCE_STATE_VIDEO_DECODE_WRITE,
                    D3D12_RESOURCE_STATE_COMMON,
                )
            })
            .chain(read.iter().map(|t| {
                transition(
                    t,
                    D3D12_RESOURCE_STATE_VIDEO_DECODE_READ,
                    D3D12_RESOURCE_STATE_COMMON,
                )
            }))
            .collect();
        #[allow(unsafe_code)]
        // SAFETY: as above.
        unsafe {
            self.decode_list.ResourceBarrier(&after)
        };
        #[allow(unsafe_code)]
        // SAFETY: ends recording; the list is executed by the caller.
        let r = unsafe { self.decode_list.Close() };
        // Barriers are copied into the list when recorded: the references can go now.
        before.into_iter().for_each(release_transition);
        after.into_iter().for_each(release_transition);
        r.map_err(win)
    }

    /// Records the copy of both planes of `texture` into the read-back buffer.
    fn record_copy(&self, texture: &ID3D12Resource) -> Result<(), Error> {
        #[allow(unsafe_code)]
        // SAFETY: the allocator's last list has executed (the caller waited for it).
        unsafe { self.copy_alloc.Reset() }.map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: the list was closed; it reopens on its own allocator, no pipeline state.
        unsafe { self.copy_list.Reset(&self.copy_alloc, None) }.map_err(win)?;
        for sub in 0..2u32 {
            let dst = D3D12_TEXTURE_COPY_LOCATION {
                pResource: ManuallyDrop::new(Some(self.readback.clone())),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: self.layout[sub as usize],
                },
            };
            let src = D3D12_TEXTURE_COPY_LOCATION {
                pResource: ManuallyDrop::new(Some(texture.clone())),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: sub,
                },
            };
            #[allow(unsafe_code)]
            // SAFETY: both locations name live resources; the texture is in the common state,
            // which a copy queue reads from without a barrier.
            unsafe {
                self.copy_list.CopyTextureRegion(&dst, 0, 0, 0, &src, None)
            };
            drop(ManuallyDrop::into_inner(dst.pResource));
            drop(ManuallyDrop::into_inner(src.pResource));
        }
        #[allow(unsafe_code)]
        // SAFETY: ends recording.
        unsafe { self.copy_list.Close() }.map_err(win)
    }
}

/// A decoder with its heap, queues and fence: what a pool of pictures of one coded size and
/// format runs on.
pub(super) struct Engine {
    decoder: ID3D12VideoDecoder,
    heap: ID3D12VideoDecoderHeap,
    decode_queue: ID3D12CommandQueue,
    copy_queue: ID3D12CommandQueue,
    fence: ID3D12Fence,
    event: Event,
    /// The next fence value to signal.
    next: u64,
}

impl Engine {
    /// A decoder for `profile` into `format` pictures of `tw`×`th`, with a heap for
    /// `max_pictures` decoded picture buffers.
    pub fn new(
        device: &ID3D12Device,
        video: &ID3D12VideoDevice,
        profile: GUID,
        format: DXGI_FORMAT,
        tw: u32,
        th: u32,
        max_pictures: u32,
    ) -> Result<Engine, Error> {
        let dec_desc = D3D12_VIDEO_DECODER_DESC {
            NodeMask: 0,
            Configuration: config(profile),
        };
        #[allow(unsafe_code)]
        // SAFETY: the descriptor outlives the call.
        let decoder: ID3D12VideoDecoder =
            unsafe { video.CreateVideoDecoder(&dec_desc) }.map_err(win)?;
        let heap_desc = D3D12_VIDEO_DECODER_HEAP_DESC {
            NodeMask: 0,
            Configuration: config(profile),
            DecodeWidth: tw,
            DecodeHeight: th,
            Format: format,
            FrameRate: FRAME_RATE,
            BitRate: 0,
            MaxDecodePictureBufferCount: max_pictures,
        };
        #[allow(unsafe_code)]
        // SAFETY: the descriptor outlives the call.
        let heap: ID3D12VideoDecoderHeap =
            unsafe { video.CreateVideoDecoderHeap(&heap_desc) }.map_err(win)?;
        let decode_queue = queue(device, D3D12_COMMAND_LIST_TYPE_VIDEO_DECODE)?;
        let copy_queue = queue(device, D3D12_COMMAND_LIST_TYPE_COPY)?;
        #[allow(unsafe_code)]
        // SAFETY: a fence starting at 0; no memory of ours is involved.
        let fence: ID3D12Fence =
            unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }.map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: an unnamed auto-reset event with default security.
        let event = Event(
            unsafe { CreateEventW(None, false, false, None) }
                .map_err(win)?
                .0 as isize,
        );
        Ok(Engine {
            decoder,
            heap,
            decode_queue,
            copy_queue,
            fence,
            event,
            next: 1,
        })
    }

    /// Waits until the fence reaches `value`.
    pub fn wait(&self, value: u64, timeout: Duration) -> Result<(), Error> {
        #[allow(unsafe_code)]
        // SAFETY: a plain read of the fence's completed value.
        if unsafe { self.fence.GetCompletedValue() } >= value {
            return Ok(());
        }
        #[allow(unsafe_code)]
        // SAFETY: the event handle is open for as long as the engine; the fence signals it
        // once it reaches `value`.
        unsafe { self.fence.SetEventOnCompletion(value, self.event.handle()) }.map_err(win)?;
        let ms = u32::try_from(timeout.as_millis())
            .unwrap_or(u32::MAX - 1)
            .max(1);
        #[allow(unsafe_code)]
        // SAFETY: waits on our own event handle; no memory is involved.
        let r = unsafe { WaitForSingleObject(self.event.handle(), ms) };
        match r {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(Error::Hardware(format!(
                "the decoder timed out ({} ms)",
                timeout.as_millis()
            ))),
            e => Err(Error::Hardware(format!("waiting for the decoder: {e:?}"))),
        }
    }

    /// Executes `scratch`'s recorded decode list; the fence value its completion signals.
    fn run_decode(&mut self, scratch: &Scratch) -> Result<u64, Error> {
        let value = self.next;
        self.next += 1;
        let list: ID3D12CommandList = scratch.decode_list.cast().map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: the list is closed and the resources it names live in the pool, which waits
        // for the GPU before dropping them.
        unsafe {
            self.decode_queue.ExecuteCommandLists(&[Some(list)])
        };
        #[allow(unsafe_code)]
        // SAFETY: signals our fence from the queue once the decode is done.
        unsafe { self.decode_queue.Signal(&self.fence, value) }.map_err(win)?;
        Ok(value)
    }

    /// Executes `scratch`'s recorded copy list once the fence has passed `after`; the fence
    /// value its completion signals.
    fn run_copy(&mut self, scratch: &Scratch, after: u64) -> Result<u64, Error> {
        let value = self.next;
        self.next += 1;
        let list: ID3D12CommandList = scratch.copy_list.cast().map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: the copy queue waits for the decode's fence value before its work starts.
        unsafe { self.copy_queue.Wait(&self.fence, after) }.map_err(win)?;
        #[allow(unsafe_code)]
        // SAFETY: as for the decode list.
        unsafe {
            self.copy_queue.ExecuteCommandLists(&[Some(list)])
        };
        #[allow(unsafe_code)]
        // SAFETY: signals our fence once the copy-out is done.
        unsafe { self.copy_queue.Signal(&self.fence, value) }.map_err(win)?;
        Ok(value)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // The GPU may still be on the last pictures (a batch that failed midway): its
        // resources must not go while it is.
        let _ = self.wait(self.next.saturating_sub(1), RELEASE_WAIT);
    }
}

/// One picture through the decoder: `pp` and `qm` its DXVA parameters, `buf` and `controls`
/// its bitstream and slice controls, `list` the pictures DXVA's entries index (every slot for
/// a video, the one picture for a still), `current` the entry decoded into and `reads` the
/// entries it references. Returns the decode's fence value; the caller copies the output out
/// ([`copy_out`]) and reads it back ([`map_planes`]).
#[allow(clippy::too_many_arguments)]
pub(super) fn decode_frame(
    engine: &mut Engine,
    scratch: &mut Scratch,
    device: &ID3D12Device,
    mode: Mode,
    list: &[&Textures],
    current: usize,
    reads: &[usize],
    pp: &mut dyn Blob,
    qm: Option<&mut dyn Blob>,
    buf: &[u8],
    controls: &mut [dxva::SliceShort],
) -> Result<u64, Error> {
    let cur = list
        .get(current)
        .ok_or_else(|| Error::Hardware("picture slot outside the pool".into()))?;
    // The caller has waited for the scratch's last decode and copy (a still slot's fence, a
    // video's synchronous read-back) before the buffers and lists are reused here.
    scratch.upload(device, buf)?;

    let mut args = D3D12_VIDEO_DECODE_INPUT_STREAM_ARGUMENTS::default();
    let mut n = 0;
    let mut arg = |kind, (data, size): (*mut c_void, u32)| {
        args.FrameArguments[n] = D3D12_VIDEO_DECODE_FRAME_ARGUMENT {
            Type: kind,
            Size: size,
            pData: data,
        };
        n += 1;
    };
    arg(
        D3D12_VIDEO_DECODE_ARGUMENT_TYPE_PICTURE_PARAMETERS,
        pp.blob(),
    );
    if let Some(q) = qm {
        arg(
            D3D12_VIDEO_DECODE_ARGUMENT_TYPE_INVERSE_QUANTIZATION_MATRIX,
            q.blob(),
        );
    }
    arg(
        D3D12_VIDEO_DECODE_ARGUMENT_TYPE_SLICE_CONTROL,
        (controls.as_mut_ptr().cast(), size_of_val(controls) as u32),
    );
    args.NumFrameArguments = n as u32;
    // The reference list: the decode target of every entry, in order, with the heap.
    let mut texs: Vec<Option<ID3D12Resource>> =
        list.iter().map(|t| Some(t.target().clone())).collect();
    let mut subs = vec![0u32; list.len()];
    let mut heaps: Vec<Option<ID3D12VideoDecoderHeap>> =
        vec![Some(engine.heap.clone()); list.len()];
    args.ReferenceFrames = D3D12_VIDEO_DECODE_REFERENCE_FRAMES {
        NumTexture2Ds: list.len() as u32,
        ppTexture2Ds: texs.as_mut_ptr(),
        pSubresources: subs.as_mut_ptr(),
        ppHeaps: heaps.as_mut_ptr(),
    };
    args.CompressedBitstream = D3D12_VIDEO_DECODE_COMPRESSED_BITSTREAM {
        pBuffer: ManuallyDrop::new(Some(scratch.bitstream.clone())),
        Offset: 0,
        Size: buf.len() as u64,
    };
    args.pHeap = ManuallyDrop::new(Some(engine.heap.clone()));
    let conversion = match &cur.reference {
        // Decoded into the reference-only texture, "converted" (copied) into the output.
        Some(r) => D3D12_VIDEO_DECODE_CONVERSION_ARGUMENTS {
            Enable: true.into(),
            pReferenceTexture2D: ManuallyDrop::new(Some(r.clone())),
            ReferenceSubresource: 0,
            OutputColorSpace: mode.color_space,
            DecodeColorSpace: mode.color_space,
        },
        None => D3D12_VIDEO_DECODE_CONVERSION_ARGUMENTS::default(),
    };
    let out = D3D12_VIDEO_DECODE_OUTPUT_STREAM_ARGUMENTS {
        pOutputTexture2D: ManuallyDrop::new(Some(cur.texture.clone())),
        OutputSubresource: 0,
        ConversionArguments: conversion,
    };
    let mut write = vec![&cur.texture];
    write.extend(cur.reference.as_ref());
    let read: Vec<&ID3D12Resource> = reads
        .iter()
        .filter(|&&i| i != current)
        .filter_map(|&i| list.get(i))
        .map(|t| t.target())
        .collect();
    let r = scratch.record_decode(&engine.decoder, &out, &args, &write, &read);
    drop(ManuallyDrop::into_inner(out.pOutputTexture2D));
    drop(ManuallyDrop::into_inner(
        out.ConversionArguments.pReferenceTexture2D,
    ));
    drop(ManuallyDrop::into_inner(args.CompressedBitstream.pBuffer));
    drop(ManuallyDrop::into_inner(args.pHeap));
    r?;
    engine.run_decode(scratch)
}

/// Copies `texture` into the read-back buffer once decode `after` is done; the copy's fence
/// value.
pub(super) fn copy_out(
    engine: &mut Engine,
    scratch: &Scratch,
    texture: &ID3D12Resource,
    after: u64,
) -> Result<u64, Error> {
    scratch.record_copy(texture)?;
    engine.run_copy(scratch, after)
}

/// Waits (at most `timeout`) for copy `value` and reads `region` of the read-back buffer into
/// planes.
pub(super) fn map_planes(
    engine: &Engine,
    scratch: &Scratch,
    value: u64,
    timeout: Duration,
    ten_bit: bool,
    region: (u32, u32, u32, u32),
) -> Result<HevcPlanes, Error> {
    engine.wait(value, timeout)?;
    let mut p: *mut c_void = std::ptr::null_mut();
    let whole = D3D12_RANGE {
        Begin: 0,
        End: scratch.readback_len as usize,
    };
    #[allow(unsafe_code)]
    // SAFETY: maps the read-back buffer (host-visible) for reading; the pointer stays valid
    // until Unmap below.
    unsafe { scratch.readback.Map(0, Some(&whole), Some(&mut p)) }.map_err(win)?;
    let r = if p.is_null() {
        Err(Error::Hardware("read-back buffer not mapped".into()))
    } else {
        #[allow(unsafe_code)]
        // SAFETY: the buffer is `readback_len` bytes from `p`, mapped, and only read.
        let data =
            unsafe { std::slice::from_raw_parts(p.cast::<u8>(), scratch.readback_len as usize) };
        let l = &scratch.layout;
        planes_from_420(
            data,
            (l[0].Offset as usize, l[0].Footprint.RowPitch as usize),
            (l[1].Offset as usize, l[1].Footprint.RowPitch as usize),
            ten_bit,
            region,
        )
    };
    let none = D3D12_RANGE { Begin: 0, End: 0 };
    #[allow(unsafe_code)]
    // SAFETY: balances the Map; nothing was written, so the written range is empty.
    unsafe {
        scratch.readback.Unmap(0, Some(&none))
    };
    r
}

/// The still-picture pool: an engine and `n` slots, each with its own textures and scratch
/// so that up to `n` pictures are in flight.
struct Pool {
    w: u32,
    h: u32,
    ten_bit: bool,
    mode: Mode,
    engine: Engine,
    slots: Vec<Slot>,
}

struct Slot {
    tex: Textures,
    scratch: Scratch,
    /// The fence value of the slot's last copy-out (0: none yet).
    done: u64,
}

impl Pool {
    fn new(
        device: &ID3D12Device,
        video: &ID3D12VideoDevice,
        w: u32,
        h: u32,
        ten_bit: bool,
        mode: Mode,
        n: usize,
    ) -> Result<Pool, Error> {
        let (profile, format) = if ten_bit {
            (D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10, DXGI_FORMAT_P010)
        } else {
            (D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN, DXGI_FORMAT_NV12)
        };
        let (tw, th) = (w.next_multiple_of(ALIGN_W), h.next_multiple_of(ALIGN_H));
        let engine = Engine::new(device, video, profile, format, tw, th, n as u32 + 1)?;
        let mut slots = Vec::with_capacity(n);
        for _ in 0..n {
            slots.push(Slot {
                tex: Textures::new(device, tw, th, format, mode.ref_only)?,
                scratch: Scratch::new(
                    device,
                    &texture_desc(tw, th, format, D3D12_RESOURCE_FLAG_NONE),
                )?,
                done: 0,
            });
        }
        Ok(Pool {
            w,
            h,
            ten_bit,
            mode,
            engine,
            slots,
        })
    }

    fn submit(
        &mut self,
        device: &ID3D12Device,
        pic: &Picture<'_>,
        slot: usize,
        status: u32,
    ) -> Result<(), Error> {
        // A still: no references, the picture's own id is its slot's.
        let size = (pic.coded_w, pic.coded_h);
        crate::gate::hevc(
            &pic.pic,
            pic.iq.as_ref(),
            &pic.slices,
            &[pic.pic.curr_pic.picture_id],
            size,
        )?;
        let (buf, mut controls) = dxva::bitstream(&pic.slices);
        let mut pp = dxva::pic_params(&pic.pic, pic.uniform_spacing, 0, status);
        let mut qm = pic.iq.as_ref().map(dxva::qmatrix);
        // The slot's last copy-out must be over before its buffers change.
        let done = self.slots[slot].done;
        if done > 0 {
            self.engine.wait(done, RELEASE_WAIT)?;
        }
        let s = &mut self.slots[slot];
        let decoded = decode_frame(
            &mut self.engine,
            &mut s.scratch,
            device,
            self.mode,
            &[&s.tex],
            0,
            &[],
            &mut pp,
            qm.as_mut().map(|q| q as &mut dyn Blob),
            &buf,
            &mut controls,
        )?;
        s.done = copy_out(&mut self.engine, &s.scratch, &s.tex.texture, decoded)?;
        Ok(())
    }

    fn read(
        &mut self,
        slot: usize,
        timeout: Duration,
        region: (u32, u32, u32, u32),
    ) -> Result<HevcPlanes, Error> {
        let (x, y, w, h) = region;
        if x % 2 != 0 || y % 2 != 0 || x + w > self.w || y + h > self.h || w == 0 || h == 0 {
            return Err(Error::Hardware(
                "read-back region outside the picture".into(),
            ));
        }
        let s = &self.slots[slot];
        if s.done == 0 {
            return Err(Error::Hardware("nothing decoded into this slot".into()));
        }
        map_planes(
            &self.engine,
            &s.scratch,
            s.done,
            timeout,
            self.ten_bit,
            region,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What this machine's D3D12 decoder says about each profile (`--nocapture` shows it;
    /// nothing is asserted: a machine without a decoder passes).
    #[test]
    fn probe_report() {
        let (device, caps) = match Device::open() {
            Ok(x) => x,
            Err(e) => {
                eprintln!("D3D12 video: {e}");
                return;
            }
        };
        eprintln!("{caps:#?}");
        if let Some(h) = &device.hevc {
            eprintln!("HEVC Main: {:?}, Main 10: {:?}", h.main, h.main10);
        }
        for (name, profile, format) in [
            (
                "HEVC Main",
                D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN,
                DXGI_FORMAT_NV12,
            ),
            (
                "HEVC Main 10",
                D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10,
                DXGI_FORMAT_P010,
            ),
            ("H.264", D3D12_VIDEO_DECODE_PROFILE_H264, DXGI_FORMAT_NV12),
            (
                "AV1",
                D3D12_VIDEO_DECODE_PROFILE_AV1_PROFILE0,
                DXGI_FORMAT_NV12,
            ),
        ] {
            let s = support(&device.video, profile, format, 1920, 1088);
            eprintln!(
                "{name}: support {:?} (flags, configuration flags, tier)",
                s.map(|s| (s.SupportFlags.0, s.ConfigurationFlags.0, s.DecodeTier.0))
            );
        }
    }
}
