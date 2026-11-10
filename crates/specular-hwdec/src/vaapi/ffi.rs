//! The only `unsafe` in `specular-hwdec`: libva loaded with `dlopen` at run time and wrapped in
//! a safe [`Device`] (UNSAFE.md). No libva headers or link-time dependency: the few types we use
//! are declared here from `va.h` / `va_dec_jpeg.h` / `va_dec_hevc.h` / `va_enc_h264.h` (libva 2.x, VA-API 1.x ABI, which has been
//! stable since 2017).
//!
//! Every wrapper keeps the same invariants: the display pointer comes from `vaGetDisplayDRM` and
//! stays valid until `Drop` calls `vaTerminate`; the render-node file descriptor it uses lives
//! as long as the `Device`; pointers passed to libva come from live Rust references or slices
//! whose lengths are passed with them; IDs are plain integers that libva validates (an unknown
//! ID is an error status, not undefined behaviour).

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::fs::File;
use std::os::fd::AsRawFd;

pub type Status = c_int;
pub use crate::params::{INVALID_ID, Id};
pub const SUCCESS: Status = 0;

pub const PROFILE_JPEG_BASELINE: c_int = 12;
pub const PROFILE_HEVC_MAIN: c_int = 17;
pub const PROFILE_HEVC_MAIN10: c_int = 18;
pub const PROFILE_HEVC_MAIN422_10: c_int = 24;
pub const PROFILE_HEVC_MAIN444: c_int = 26;
pub const PROFILE_AV1_PROFILE0: c_int = 32;
pub const PROFILE_AV1_PROFILE1: c_int = 33;
pub const PROFILE_H264_HIGH: c_int = 7;
pub const PROFILE_H264_HIGH10: c_int = 36;
pub const PROFILE_VP9_PROFILE0: c_int = 19;

pub const ENTRYPOINT_VLD: c_int = 1;

pub const ATTRIB_RT_FORMAT: c_int = 0;
pub const ATTRIB_MAX_PICTURE_WIDTH: c_int = 18;
pub const ATTRIB_MAX_PICTURE_HEIGHT: c_int = 19;
pub const ATTRIB_NOT_SUPPORTED: u32 = 0x8000_0000;

pub const RT_FORMAT_YUV420: u32 = 0x01;
pub const RT_FORMAT_YUV422: u32 = 0x02;
pub const RT_FORMAT_YUV444: u32 = 0x04;
pub const RT_FORMAT_YUV400: u32 = 0x10;
pub const RT_FORMAT_YUV420_10: u32 = 0x100;

/// `VA_STATUS_ERROR_TIMEDOUT` (`vaSyncSurface2`).
pub const STATUS_ERROR_TIMEDOUT: Status = 0x26;

pub const PROGRESSIVE: c_int = 1;

pub const BUF_PICTURE_PARAMETER: c_int = 0;
pub const BUF_IQ_MATRIX: c_int = 1;
pub const BUF_SLICE_PARAMETER: c_int = 4;
pub const BUF_SLICE_DATA: c_int = 5;
pub const BUF_HUFFMAN_TABLE: c_int = 12;

pub const fn fourcc(s: &[u8; 4]) -> u32 {
    s[0] as u32 | (s[1] as u32) << 8 | (s[2] as u32) << 16 | (s[3] as u32) << 24
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ConfigAttrib {
    pub kind: c_int,
    pub value: u32,
}

/// `VASurfaceAttrib` with an integer `VAGenericValue`: the value union is 8-byte aligned (it
/// has a pointer arm, never used here), so the `int` sits at offset 16.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct SurfaceAttrib {
    kind: c_int,
    flags: u32,
    value_type: c_int,
    _pad: u32,
    value: i32,
    _pad2: u32,
}

const SURFACE_ATTRIB_PIXEL_FORMAT: c_int = 1;
const SURFACE_ATTRIB_MEMORY_TYPE: c_int = 6;
const SURFACE_ATTRIB_SETTABLE: u32 = 2;
const GENERIC_VALUE_INTEGER: c_int = 1;
const SURFACE_MEM_TYPE_VA: i32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ImageFormat {
    pub fourcc: u32,
    pub byte_order: u32,
    pub bits_per_pixel: u32,
    pub depth: u32,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub alpha_mask: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Image {
    pub image_id: Id,
    pub format: ImageFormat,
    pub buf: Id,
    pub width: u16,
    pub height: u16,
    pub data_size: u32,
    pub num_planes: u32,
    pub pitches: [u32; 3],
    pub offsets: [u32; 3],
    pub num_palette_entries: i32,
    pub entry_bytes: i32,
    pub component_order: [i8; 4],
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct JpegFrameComponent {
    pub component_id: u8,
    pub h_sampling_factor: u8,
    pub v_sampling_factor: u8,
    pub quantiser_table_selector: u8,
}

/// `VAPictureParameterBufferJPEGBaseline`. Newer libva splits the first reserved words into a
/// crop rectangle; zero means "no crop", so the layout and meaning are the same.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JpegPictureParameter {
    pub picture_width: u16,
    pub picture_height: u16,
    pub components: [JpegFrameComponent; 255],
    pub num_components: u8,
    pub color_space: u8,
    pub rotation: u32,
    pub reserved: [u32; 7],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JpegIqMatrix {
    pub load_quantiser_table: [u8; 4],
    /// In zig-zag order, as stored in the file.
    pub quantiser_table: [[u8; 64]; 4],
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JpegHuffmanTable {
    pub num_dc_codes: [u8; 16],
    pub dc_values: [u8; 12],
    pub num_ac_codes: [u8; 16],
    pub ac_values: [u8; 162],
    pub pad: [u8; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JpegHuffmanTables {
    pub load_huffman_table: [u8; 2],
    pub huffman_table: [JpegHuffmanTable; 2],
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct JpegScanComponent {
    pub component_selector: u8,
    pub dc_table_selector: u8,
    pub ac_table_selector: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct JpegSliceParameter {
    pub slice_data_size: u32,
    pub slice_data_offset: u32,
    pub slice_data_flag: u32,
    pub slice_horizontal_position: u32,
    pub slice_vertical_position: u32,
    pub components: [JpegScanComponent; 4],
    pub num_components: u8,
    pub restart_interval: u16,
    pub num_mcus: u32,
    pub reserved: [u32; 4],
}

// The sizes libva and the drivers expect (checked against va.h with a C compiler once; the
// asserts keep the declarations honest).
const _: () = assert!(size_of::<ImageFormat>() == 48);
const _: () = assert!(size_of::<SurfaceAttrib>() == 24);
const _: () = assert!(size_of::<Image>() == 120);
const _: () = assert!(size_of::<JpegPictureParameter>() == 1060);
const _: () = assert!(size_of::<JpegIqMatrix>() == 276);
const _: () = assert!(size_of::<JpegHuffmanTables>() == 436);
const _: () = assert!(size_of::<JpegSliceParameter>() == 56);

/// Plain-old-data parameter structs that may be handed to `vaCreateBuffer` as bytes.
pub trait Pod: Copy {}
impl Pod for JpegPictureParameter {}
impl Pod for JpegIqMatrix {}
impl Pod for JpegHuffmanTables {}
impl Pod for JpegSliceParameter {}

// ------------------------------------------------------------------------- va_dec_hevc.h

// The HEVC parameter structs are `crate::params`, shared with the D3D12 backend (which
// translates them to DXVA's); their layout asserts are there too.
pub use crate::params::{IqMatrixHevc, PicParamHevc, SliceParamHevc};

impl Pod for PicParamHevc {}
impl Pod for SliceParamHevc {}
impl Pod for IqMatrixHevc {}

// ------------------------------------------------------------------------- va.h (H.264)

// The H.264 parameter structs (and the HEVC reference flags) are `crate::params`, shared
// with the D3D12 backend.
pub use crate::params::{
    IqMatrixH264, PICTURE_H264_SHORT_TERM_REFERENCE, PicParamH264, PictureH264, SliceParamH264,
};

impl Pod for PicParamH264 {}
impl Pod for SliceParamH264 {}
impl Pod for IqMatrixH264 {}

// ------------------------------------------------------------------------- va_enc_h264.h

pub const PROFILE_H264_MAIN: c_int = 6;
pub const ENTRYPOINT_ENC_SLICE: c_int = 6;
/// The low-power (fixed-function only) encode entrypoint some drivers offer instead.
pub const ENTRYPOINT_ENC_SLICE_LP: c_int = 8;

pub const ATTRIB_RATE_CONTROL: c_int = 5;
pub const ATTRIB_ENC_PACKED_HEADERS: c_int = 10;
pub const ATTRIB_ENC_MAX_REF_FRAMES: c_int = 13;
pub const RC_CQP: u32 = 0x10;
pub const ENC_PACKED_HEADER_NONE: u32 = 0;
pub const ENC_PACKED_HEADER_SEQUENCE: u32 = 1;
pub const ENC_PACKED_HEADER_SLICE: u32 = 4;
/// `VAEncPackedHeaderType`: SPS/PPS, slice header.
pub const PACKED_HEADER_SEQUENCE: u32 = 1;
pub const PACKED_HEADER_SLICE: u32 = 3;

pub const BUF_ENC_CODED: c_int = 21;
pub const BUF_ENC_SEQUENCE_PARAMETER: c_int = 22;
pub const BUF_ENC_PICTURE_PARAMETER: c_int = 23;
pub const BUF_ENC_SLICE_PARAMETER: c_int = 24;
pub const BUF_ENC_PACKED_HEADER_PARAMETER: c_int = 25;
pub const BUF_ENC_PACKED_HEADER_DATA: c_int = 26;
pub const BUF_ENC_MISC_PARAMETER: c_int = 27;
/// `VAEncMiscParameterTypeFrameRate`.
pub const ENC_MISC_FRAME_RATE: i32 = 0;

/// `VAEncSequenceParameterBufferH264` (`seq_fields` / `vui_fields` bit-fields as `u32`s).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EncSeqH264 {
    pub seq_parameter_set_id: u8,
    pub level_idc: u8,
    pub intra_period: u32,
    pub intra_idr_period: u32,
    pub ip_period: u32,
    pub bits_per_second: u32,
    pub max_num_ref_frames: u32,
    pub picture_width_in_mbs: u16,
    pub picture_height_in_mbs: u16,
    pub seq_fields: u32,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub num_ref_frames_in_pic_order_cnt_cycle: u8,
    pub offset_for_non_ref_pic: i32,
    pub offset_for_top_to_bottom_field: i32,
    pub offset_for_ref_frame: [i32; 256],
    pub frame_cropping_flag: u8,
    pub frame_crop_left_offset: u32,
    pub frame_crop_right_offset: u32,
    pub frame_crop_top_offset: u32,
    pub frame_crop_bottom_offset: u32,
    pub vui_parameters_present_flag: u8,
    pub vui_fields: u32,
    pub aspect_ratio_idc: u8,
    pub sar_width: u32,
    pub sar_height: u32,
    pub num_units_in_tick: u32,
    pub time_scale: u32,
    pub reserved: [u32; 4],
}

/// `VAEncPictureParameterBufferH264` (`pic_fields` as a `u32`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EncPicH264 {
    pub curr_pic: PictureH264,
    pub reference_frames: [PictureH264; 16],
    pub coded_buf: Id,
    pub pic_parameter_set_id: u8,
    pub seq_parameter_set_id: u8,
    pub last_picture: u8,
    pub frame_num: u16,
    pub pic_init_qp: u8,
    pub num_ref_idx_l0_active_minus1: u8,
    pub num_ref_idx_l1_active_minus1: u8,
    pub chroma_qp_index_offset: i8,
    pub second_chroma_qp_index_offset: i8,
    pub pic_fields: u32,
    pub reserved: [u32; 4],
}

/// `VAEncSliceParameterBufferH264`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EncSliceH264 {
    pub macroblock_address: u32,
    pub num_macroblocks: u32,
    pub macroblock_info: Id,
    pub slice_type: u8,
    pub pic_parameter_set_id: u8,
    pub idr_pic_id: u16,
    pub pic_order_cnt_lsb: u16,
    pub delta_pic_order_cnt_bottom: i32,
    pub delta_pic_order_cnt: [i32; 2],
    pub direct_spatial_mv_pred_flag: u8,
    pub num_ref_idx_active_override_flag: u8,
    pub num_ref_idx_l0_active_minus1: u8,
    pub num_ref_idx_l1_active_minus1: u8,
    pub ref_pic_list0: [PictureH264; 32],
    pub ref_pic_list1: [PictureH264; 32],
    pub luma_log2_weight_denom: u8,
    pub chroma_log2_weight_denom: u8,
    pub luma_weight_l0_flag: u8,
    pub luma_weight_l0: [i16; 32],
    pub luma_offset_l0: [i16; 32],
    pub chroma_weight_l0_flag: u8,
    pub chroma_weight_l0: [[i16; 2]; 32],
    pub chroma_offset_l0: [[i16; 2]; 32],
    pub luma_weight_l1_flag: u8,
    pub luma_weight_l1: [i16; 32],
    pub luma_offset_l1: [i16; 32],
    pub chroma_weight_l1_flag: u8,
    pub chroma_weight_l1: [[i16; 2]; 32],
    pub chroma_offset_l1: [[i16; 2]; 32],
    pub cabac_init_idc: u8,
    pub slice_qp_delta: i8,
    pub disable_deblocking_filter_idc: u8,
    pub slice_alpha_c0_offset_div2: i8,
    pub slice_beta_offset_div2: i8,
    pub reserved: [u32; 4],
}

/// `VACodedBufferSegment`: one link of the list a mapped coded buffer starts with.
#[repr(C)]
struct CodedSegment {
    size: u32,
    bit_offset: u32,
    status: u32,
    reserved: u32,
    buf: *mut c_void,
    next: *mut c_void,
    va_reserved: [u32; 4],
}

// Checked against libva 2.22's va.h / va_enc_h264.h compiled with gcc (sizeof / offsetof).
const _: () = assert!(size_of::<EncSeqH264>() == 1132);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, picture_width_in_mbs) == 24);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, seq_fields) == 28);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, offset_for_ref_frame) == 44);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, frame_cropping_flag) == 1068);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, frame_crop_left_offset) == 1072);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, vui_fields) == 1092);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, sar_width) == 1100);
const _: () = assert!(std::mem::offset_of!(EncSeqH264, time_scale) == 1112);
const _: () = assert!(size_of::<EncPicH264>() == 648);
const _: () = assert!(std::mem::offset_of!(EncPicH264, coded_buf) == 612);
const _: () = assert!(std::mem::offset_of!(EncPicH264, frame_num) == 620);
const _: () = assert!(std::mem::offset_of!(EncPicH264, pic_init_qp) == 622);
const _: () = assert!(std::mem::offset_of!(EncPicH264, chroma_qp_index_offset) == 625);
const _: () = assert!(std::mem::offset_of!(EncPicH264, pic_fields) == 628);
const _: () = assert!(size_of::<EncSliceH264>() == 3140);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, slice_type) == 12);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, pic_order_cnt_lsb) == 16);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, direct_spatial_mv_pred_flag) == 32);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, ref_pic_list0) == 36);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, ref_pic_list1) == 1188);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, luma_weight_l0) == 2344);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, chroma_weight_l0_flag) == 2472);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, luma_weight_l1_flag) == 2730);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, cabac_init_idc) == 3118);
const _: () = assert!(std::mem::offset_of!(EncSliceH264, slice_beta_offset_div2) == 3122);
const _: () = assert!(size_of::<CodedSegment>() == 48);
const _: () = assert!(std::mem::offset_of!(CodedSegment, buf) == 16);
const _: () = assert!(std::mem::offset_of!(CodedSegment, next) == 24);

impl Pod for EncSeqH264 {}
impl Pod for EncPicH264 {}
impl Pod for EncSliceH264 {}

type Display = *mut c_void;
type MessageCallback = Option<unsafe extern "C" fn(*mut c_void, *const c_char)>;
/// `vaSyncSurface2(dpy, surface, timeout_ns)` (libva 2.9+; optional).
type SyncSurface2 = unsafe extern "C" fn(Display, Id, u64) -> Status;

macro_rules! va_fns {
    ($($name:ident: fn($($arg:ty),*) -> $ret:ty;)*) => {
        #[allow(non_snake_case)]
        struct Fns {
            $($name: unsafe extern "C" fn($($arg),*) -> $ret,)*
        }

        impl Fns {
            #[allow(unsafe_code)]
            fn load(lib: &libloading::Library) -> Result<Fns, libloading::Error> {
                Ok(Fns {
                    $($name: {
                        // SAFETY: the declared signature matches the libva 2.x prototype of the
                        // symbol with this name (va.h); the pointer is copied out and only used
                        // while `lib` (kept in `Device`) is loaded.
                        let s = unsafe {
                            lib.get::<unsafe extern "C" fn($($arg),*) -> $ret>(
                                concat!(stringify!($name), "\0").as_bytes(),
                            )
                        }?;
                        *s
                    },)*
                })
            }
        }
    };
}

va_fns! {
    vaInitialize: fn(Display, *mut c_int, *mut c_int) -> Status;
    vaTerminate: fn(Display) -> Status;
    vaSetErrorCallback: fn(Display, MessageCallback, *mut c_void) -> MessageCallback;
    vaSetInfoCallback: fn(Display, MessageCallback, *mut c_void) -> MessageCallback;
    vaQueryVendorString: fn(Display) -> *const c_char;
    vaMaxNumProfiles: fn(Display) -> c_int;
    vaMaxNumEntrypoints: fn(Display) -> c_int;
    vaQueryConfigProfiles: fn(Display, *mut c_int, *mut c_int) -> Status;
    vaQueryConfigEntrypoints: fn(Display, c_int, *mut c_int, *mut c_int) -> Status;
    vaGetConfigAttributes: fn(Display, c_int, c_int, *mut ConfigAttrib, c_int) -> Status;
    vaCreateConfig: fn(Display, c_int, c_int, *mut ConfigAttrib, c_int, *mut Id) -> Status;
    vaDestroyConfig: fn(Display, Id) -> Status;
    vaCreateSurfaces: fn(Display, c_uint, c_uint, c_uint, *mut Id, c_uint, *mut c_void, c_uint) -> Status;
    vaDestroySurfaces: fn(Display, *mut Id, c_int) -> Status;
    vaCreateContext: fn(Display, Id, c_int, c_int, c_int, *mut Id, c_int, *mut Id) -> Status;
    vaDestroyContext: fn(Display, Id) -> Status;
    vaCreateBuffer: fn(Display, Id, c_int, c_uint, c_uint, *mut c_void, *mut Id) -> Status;
    vaDestroyBuffer: fn(Display, Id) -> Status;
    vaBeginPicture: fn(Display, Id, Id) -> Status;
    vaRenderPicture: fn(Display, Id, *mut Id, c_int) -> Status;
    vaEndPicture: fn(Display, Id) -> Status;
    vaSyncSurface: fn(Display, Id) -> Status;
    vaCreateImage: fn(Display, *mut ImageFormat, c_int, c_int, *mut Image) -> Status;
    vaGetImage: fn(Display, Id, c_int, c_int, c_uint, c_uint, Id) -> Status;
    vaDestroyImage: fn(Display, Id) -> Status;
    vaPutImage: fn(Display, Id, Id, c_int, c_int, c_uint, c_uint, c_int, c_int, c_uint, c_uint) -> Status;
    vaMapBuffer: fn(Display, Id, *mut *mut c_void) -> Status;
    vaUnmapBuffer: fn(Display, Id) -> Status;
    vaErrorStr: fn(Status) -> *const c_char;
}

/// An initialised VA display on a DRM render node.
pub struct Device {
    fns: Fns,
    sync2: Option<SyncSurface2>,
    dpy: Display,
    pub version: (i32, i32),
    _render_node: File,
    _drm: libloading::Library,
    _va: libloading::Library,
}

// SAFETY: a VADisplay is not tied to the thread that created it (libva and the Mesa / Intel
// drivers lock internally); `Device` is only ever used behind a `Mutex`, so calls never overlap.
#[allow(unsafe_code)]
unsafe impl Send for Device {}

#[allow(unsafe_code)]
fn open_library(name: &str) -> Result<libloading::Library, String> {
    // SAFETY: libva's initialisers only set up its own state; loading it has no other effect on
    // this process. (Its drivers are loaded later by `vaInitialize`.)
    unsafe { libloading::Library::new(name) }.map_err(|e| format!("{name}: {e}"))
}

impl Device {
    /// Loads libva, opens `render_node` and initialises a display on it.
    #[allow(unsafe_code)]
    pub fn open(render_node: &str) -> Result<Device, String> {
        let va = open_library("libva.so.2")?;
        let drm = open_library("libva-drm.so.2")?;
        let fns = Fns::load(&va).map_err(|e| format!("libva: {e}"))?;
        // SAFETY: the signature matches `VAStatus vaSyncSurface2(VADisplay, VASurfaceID,
        // uint64_t timeout_ns)` (va.h, libva 2.9+); absent from older libva, hence optional.
        let sync2 = unsafe { va.get::<SyncSurface2>(b"vaSyncSurface2\0") }
            .ok()
            .map(|s| *s);
        // SAFETY: the signature matches `VADisplay vaGetDisplayDRM(int fd)` (va_drm.h).
        let get_display =
            unsafe { drm.get::<unsafe extern "C" fn(c_int) -> Display>(b"vaGetDisplayDRM\0") }
                .map_err(|e| format!("libva-drm: {e}"))?;
        let get_display = *get_display;
        let node = File::options()
            .read(true)
            .write(true)
            .open(render_node)
            .map_err(|e| format!("{render_node}: {e}"))?;
        // SAFETY: the fd is open and stays open as long as `Device` (field `_render_node`).
        let dpy = unsafe { get_display(node.as_raw_fd()) };
        if dpy.is_null() {
            return Err("vaGetDisplayDRM failed".into());
        }
        // Silence libva's own stderr messages; failures are reported through status codes.
        // SAFETY: `dpy` is a display from vaGetDisplayDRM; a null callback restores silence.
        unsafe { (fns.vaSetInfoCallback)(dpy, None, std::ptr::null_mut()) };
        // SAFETY: as above.
        unsafe { (fns.vaSetErrorCallback)(dpy, None, std::ptr::null_mut()) };
        let (mut major, mut minor) = (0, 0);
        // SAFETY: `dpy` is valid; the out-pointers are live locals.
        let st = unsafe { (fns.vaInitialize)(dpy, &mut major, &mut minor) };
        let dev = Device {
            fns,
            sync2,
            dpy,
            version: (major, minor),
            _render_node: node,
            _drm: drm,
            _va: va,
        };
        if st != SUCCESS {
            return Err(format!("vaInitialize: {}", dev.error_str(st)));
        }
        Ok(dev)
    }

    #[allow(unsafe_code)]
    pub fn error_str(&self, st: Status) -> String {
        // SAFETY: vaErrorStr returns a pointer to a static NUL-terminated string for any value.
        let p = unsafe { (self.fns.vaErrorStr)(st) };
        if p.is_null() {
            return format!("VA status {st}");
        }
        // SAFETY: non-null, static and NUL-terminated (see above).
        let s = unsafe { CStr::from_ptr(p) };
        format!("{} ({st})", s.to_string_lossy())
    }

    #[allow(unsafe_code)]
    pub fn vendor(&self) -> String {
        // SAFETY: `dpy` is initialised; the result is owned by libva and valid until vaTerminate.
        let p = unsafe { (self.fns.vaQueryVendorString)(self.dpy) };
        if p.is_null() {
            return String::new();
        }
        // SAFETY: non-null and NUL-terminated, alive while `self` is (see above).
        let s = unsafe { CStr::from_ptr(p) };
        s.to_string_lossy().into_owned()
    }

    fn check(&self, what: &str, st: Status) -> Result<(), String> {
        if st == SUCCESS {
            Ok(())
        } else {
            Err(format!("{what}: {}", self.error_str(st)))
        }
    }

    #[allow(unsafe_code)]
    pub fn profiles(&self) -> Vec<c_int> {
        // SAFETY: `dpy` is initialised.
        let max = unsafe { (self.fns.vaMaxNumProfiles)(self.dpy) }.max(0);
        let mut v = vec![0; max as usize];
        let mut n = 0;
        // SAFETY: `v` has room for vaMaxNumProfiles entries, which is what libva writes at most.
        let st = unsafe { (self.fns.vaQueryConfigProfiles)(self.dpy, v.as_mut_ptr(), &mut n) };
        if st != SUCCESS {
            return Vec::new();
        }
        v.truncate(n.clamp(0, max) as usize);
        v
    }

    #[allow(unsafe_code)]
    pub fn entrypoints(&self, profile: c_int) -> Vec<c_int> {
        // SAFETY: `dpy` is initialised.
        let max = unsafe { (self.fns.vaMaxNumEntrypoints)(self.dpy) }.max(0);
        let mut v = vec![0; max as usize];
        let mut n = 0;
        // SAFETY: `v` has room for vaMaxNumEntrypoints entries.
        let st = unsafe {
            (self.fns.vaQueryConfigEntrypoints)(self.dpy, profile, v.as_mut_ptr(), &mut n)
        };
        if st != SUCCESS {
            return Vec::new();
        }
        v.truncate(n.clamp(0, max) as usize);
        v
    }

    /// Values of the given attributes (`ATTRIB_NOT_SUPPORTED` where unknown).
    #[allow(unsafe_code)]
    pub fn attributes(&self, profile: c_int, entry: c_int, kinds: &[c_int]) -> Vec<u32> {
        let mut a: Vec<ConfigAttrib> = kinds
            .iter()
            .map(|&kind| ConfigAttrib { kind, value: 0 })
            .collect();
        // SAFETY: `a` holds `a.len()` attributes, which libva fills in place.
        let st = unsafe {
            (self.fns.vaGetConfigAttributes)(
                self.dpy,
                profile,
                entry,
                a.as_mut_ptr(),
                a.len() as c_int,
            )
        };
        if st != SUCCESS {
            return vec![ATTRIB_NOT_SUPPORTED; kinds.len()];
        }
        a.into_iter().map(|a| a.value).collect()
    }

    #[allow(unsafe_code)]
    pub fn create_config(
        &self,
        profile: c_int,
        entry: c_int,
        rt_format: u32,
    ) -> Result<Id, String> {
        let mut a = [ConfigAttrib {
            kind: ATTRIB_RT_FORMAT,
            value: rt_format,
        }];
        let mut id = INVALID_ID;
        // SAFETY: one attribute in a live array; `id` is a live out-parameter.
        let st = unsafe {
            (self.fns.vaCreateConfig)(self.dpy, profile, entry, a.as_mut_ptr(), 1, &mut id)
        };
        self.check("vaCreateConfig", st).map(|_| id)
    }

    #[allow(unsafe_code)]
    pub fn destroy_config(&self, id: Id) {
        // SAFETY: `dpy` is initialised; libva validates the id.
        unsafe { (self.fns.vaDestroyConfig)(self.dpy, id) };
    }

    #[allow(unsafe_code)]
    pub fn create_surface(&self, rt_format: u32, w: u32, h: u32, code: u32) -> Result<Id, String> {
        let mut id = INVALID_ID;
        let attr = |kind, value| SurfaceAttrib {
            kind,
            flags: SURFACE_ATTRIB_SETTABLE,
            value_type: GENERIC_VALUE_INTEGER,
            _pad: 0,
            value,
            _pad2: 0,
        };
        let mut attribs = [
            attr(SURFACE_ATTRIB_PIXEL_FORMAT, code as i32),
            attr(SURFACE_ATTRIB_MEMORY_TYPE, SURFACE_MEM_TYPE_VA),
        ];
        // SAFETY: room for exactly one surface id; `attribs` is a live array of two
        // `VASurfaceAttrib`s that libva reads during the call.
        let st = unsafe {
            (self.fns.vaCreateSurfaces)(
                self.dpy,
                rt_format,
                w,
                h,
                &mut id,
                1,
                attribs.as_mut_ptr().cast(),
                attribs.len() as c_uint,
            )
        };
        self.check("vaCreateSurfaces", st).map(|_| id)
    }

    #[allow(unsafe_code)]
    pub fn destroy_surface(&self, mut id: Id) {
        // SAFETY: one surface id in a live local.
        unsafe { (self.fns.vaDestroySurfaces)(self.dpy, &mut id, 1) };
    }

    #[allow(unsafe_code)]
    pub fn create_context(
        &self,
        config: Id,
        w: u32,
        h: u32,
        mut surface: Id,
    ) -> Result<Id, String> {
        let mut id = INVALID_ID;
        // SAFETY: one render target in a live local; `id` is a live out-parameter.
        let st = unsafe {
            (self.fns.vaCreateContext)(
                self.dpy,
                config,
                w as c_int,
                h as c_int,
                PROGRESSIVE,
                &mut surface,
                1,
                &mut id,
            )
        };
        self.check("vaCreateContext", st).map(|_| id)
    }

    #[allow(unsafe_code)]
    pub fn destroy_context(&self, id: Id) {
        // SAFETY: `dpy` is initialised; libva validates the id.
        unsafe { (self.fns.vaDestroyContext)(self.dpy, id) };
    }

    /// A buffer holding a copy of `data` (libva copies it at creation).
    #[allow(unsafe_code)]
    pub fn create_buffer_bytes(&self, ctx: Id, kind: c_int, data: &[u8]) -> Result<Id, String> {
        let mut id = INVALID_ID;
        // SAFETY: `data` is a live slice of `data.len()` bytes; libva only reads it (the
        // parameter is not const in the C prototype) and copies it before returning.
        let st = unsafe {
            (self.fns.vaCreateBuffer)(
                self.dpy,
                ctx,
                kind,
                data.len() as c_uint,
                1,
                data.as_ptr() as *mut c_void,
                &mut id,
            )
        };
        self.check("vaCreateBuffer", st).map(|_| id)
    }

    /// A buffer holding a copy of the parameter struct `v`.
    #[allow(unsafe_code)]
    pub fn create_buffer<T: Pod>(&self, ctx: Id, kind: c_int, v: &T) -> Result<Id, String> {
        let mut id = INVALID_ID;
        // SAFETY: `v` is a live `repr(C)` struct of `size_of::<T>()` bytes (Pod: no pointers),
        // read and copied by libva before returning.
        let st = unsafe {
            (self.fns.vaCreateBuffer)(
                self.dpy,
                ctx,
                kind,
                size_of::<T>() as c_uint,
                1,
                v as *const T as *mut c_void,
                &mut id,
            )
        };
        self.check("vaCreateBuffer", st).map(|_| id)
    }

    #[allow(unsafe_code)]
    pub fn destroy_buffer(&self, id: Id) {
        // SAFETY: `dpy` is initialised; libva validates the id.
        unsafe { (self.fns.vaDestroyBuffer)(self.dpy, id) };
    }

    #[allow(unsafe_code)]
    pub fn begin_picture(&self, ctx: Id, surface: Id) -> Result<(), String> {
        // SAFETY: `dpy` is initialised; ids are validated by libva.
        let st = unsafe { (self.fns.vaBeginPicture)(self.dpy, ctx, surface) };
        self.check("vaBeginPicture", st)
    }

    #[allow(unsafe_code)]
    pub fn render_picture(&self, ctx: Id, buffers: &mut [Id]) -> Result<(), String> {
        // SAFETY: `buffers` is a live slice of `buffers.len()` ids.
        let st = unsafe {
            (self.fns.vaRenderPicture)(self.dpy, ctx, buffers.as_mut_ptr(), buffers.len() as c_int)
        };
        self.check("vaRenderPicture", st)
    }

    #[allow(unsafe_code)]
    pub fn end_picture(&self, ctx: Id) -> Result<(), String> {
        // SAFETY: `dpy` is initialised; the id is validated by libva.
        let st = unsafe { (self.fns.vaEndPicture)(self.dpy, ctx) };
        self.check("vaEndPicture", st)
    }

    #[allow(unsafe_code)]
    pub fn sync_surface(&self, surface: Id) -> Result<(), String> {
        // SAFETY: `dpy` is initialised; the id is validated by libva.
        let st = unsafe { (self.fns.vaSyncSurface)(self.dpy, surface) };
        self.check("vaSyncSurface", st)
    }

    /// Waits for `surface` at most `timeout_ns` (`vaSyncSurface2`), or without a limit where
    /// libva is older than 2.9. A timeout is [`STATUS_ERROR_TIMEDOUT`] in the error text.
    #[allow(unsafe_code)]
    pub fn sync_surface_timeout(&self, surface: Id, timeout_ns: u64) -> Result<(), String> {
        let Some(sync2) = self.sync2 else {
            return self.sync_surface(surface);
        };
        // SAFETY: `dpy` is initialised; libva validates the surface id.
        let st = unsafe { sync2(self.dpy, surface, timeout_ns) };
        if st == STATUS_ERROR_TIMEDOUT {
            return Err(format!(
                "vaSyncSurface2: timed out after {} ms",
                timeout_ns / 1_000_000
            ));
        }
        self.check("vaSyncSurface2", st)
    }

    /// `n` surfaces of one size and format (a decode pool).
    #[allow(unsafe_code)]
    pub fn create_surfaces(
        &self,
        rt_format: u32,
        w: u32,
        h: u32,
        code: u32,
        n: usize,
    ) -> Result<Vec<Id>, String> {
        let mut ids = vec![INVALID_ID; n];
        let attr = |kind, value| SurfaceAttrib {
            kind,
            flags: SURFACE_ATTRIB_SETTABLE,
            value_type: GENERIC_VALUE_INTEGER,
            _pad: 0,
            value,
            _pad2: 0,
        };
        let mut attribs = [
            attr(SURFACE_ATTRIB_PIXEL_FORMAT, code as i32),
            attr(SURFACE_ATTRIB_MEMORY_TYPE, SURFACE_MEM_TYPE_VA),
        ];
        // SAFETY: `ids` has room for exactly `n` surface ids, the count passed; `attribs` is a
        // live array of two `VASurfaceAttrib`s that libva reads during the call.
        let st = unsafe {
            (self.fns.vaCreateSurfaces)(
                self.dpy,
                rt_format,
                w,
                h,
                ids.as_mut_ptr(),
                n as c_uint,
                attribs.as_mut_ptr().cast(),
                attribs.len() as c_uint,
            )
        };
        self.check("vaCreateSurfaces", st).map(|_| ids)
    }

    #[allow(unsafe_code)]
    pub fn destroy_surfaces(&self, ids: &mut [Id]) {
        // SAFETY: a live slice of `ids.len()` surface ids, the count passed.
        unsafe { (self.fns.vaDestroySurfaces)(self.dpy, ids.as_mut_ptr(), ids.len() as c_int) };
    }

    /// A decode context whose render targets are `surfaces`.
    #[allow(unsafe_code)]
    pub fn create_context_pool(
        &self,
        config: Id,
        w: u32,
        h: u32,
        surfaces: &mut [Id],
    ) -> Result<Id, String> {
        let mut id = INVALID_ID;
        // SAFETY: `surfaces` is a live slice of `surfaces.len()` ids, the count passed; `id` is
        // a live out-parameter.
        let st = unsafe {
            (self.fns.vaCreateContext)(
                self.dpy,
                config,
                w as c_int,
                h as c_int,
                PROGRESSIVE,
                surfaces.as_mut_ptr(),
                surfaces.len() as c_int,
                &mut id,
            )
        };
        self.check("vaCreateContext", st).map(|_| id)
    }

    #[allow(unsafe_code)]
    pub fn create_image(&self, code: u32, w: u32, h: u32) -> Result<Image, String> {
        let mut f = ImageFormat {
            fourcc: code,
            byte_order: 1,
            bits_per_pixel: match code {
                x if x == fourcc(b"444P") => 24,
                x if x == fourcc(b"422H") || x == fourcc(b"YUY2") => 16,
                x if x == fourcc(b"Y800") => 8,
                _ => 12,
            },
            ..Default::default()
        };
        let mut img = Image::default();
        // SAFETY: `f` and `img` are live locals of the C layouts.
        let st =
            unsafe { (self.fns.vaCreateImage)(self.dpy, &mut f, w as c_int, h as c_int, &mut img) };
        self.check("vaCreateImage", st).map(|_| img)
    }

    #[allow(unsafe_code)]
    pub fn get_image(&self, surface: Id, w: u32, h: u32, image: Id) -> Result<(), String> {
        // SAFETY: `dpy` is initialised; ids are validated by libva.
        let st = unsafe { (self.fns.vaGetImage)(self.dpy, surface, 0, 0, w, h, image) };
        self.check("vaGetImage", st)
    }

    #[allow(unsafe_code)]
    pub fn destroy_image(&self, id: Id) {
        // SAFETY: `dpy` is initialised; the id is validated by libva.
        unsafe { (self.fns.vaDestroyImage)(self.dpy, id) };
    }

    /// Maps an image's buffer and passes its `data_size` bytes to `f`.
    #[allow(unsafe_code)]
    pub fn with_image_data<R>(&self, img: &Image, f: impl FnOnce(&[u8]) -> R) -> Result<R, String> {
        let mut p: *mut c_void = std::ptr::null_mut();
        // SAFETY: `p` is a live out-parameter.
        let st = unsafe { (self.fns.vaMapBuffer)(self.dpy, img.buf, &mut p) };
        self.check("vaMapBuffer", st)?;
        if p.is_null() {
            return Err("vaMapBuffer returned null".into());
        }
        // SAFETY: a mapped image buffer is `data_size` readable bytes until vaUnmapBuffer, which
        // runs only after `f` (the slice cannot escape it).
        let data = unsafe { std::slice::from_raw_parts(p as *const u8, img.data_size as usize) };
        let r = f(data);
        // SAFETY: the buffer was mapped above and `data` is no longer used.
        unsafe { (self.fns.vaUnmapBuffer)(self.dpy, img.buf) };
        Ok(r)
    }

    /// A config with several attributes (an encoder's RT format, rate control, packed headers).
    #[allow(unsafe_code)]
    pub fn create_config_with(
        &self,
        profile: c_int,
        entry: c_int,
        attribs: &[ConfigAttrib],
    ) -> Result<Id, String> {
        let mut a = attribs.to_vec();
        let mut id = INVALID_ID;
        // SAFETY: `a` is a live array of `a.len()` attributes, the count passed; `id` is a live
        // out-parameter.
        let st = unsafe {
            (self.fns.vaCreateConfig)(
                self.dpy,
                profile,
                entry,
                a.as_mut_ptr(),
                a.len() as c_int,
                &mut id,
            )
        };
        self.check("vaCreateConfig", st).map(|_| id)
    }

    /// An uninitialised buffer of `size` bytes that the driver writes (an encoder's coded
    /// buffer).
    #[allow(unsafe_code)]
    pub fn create_output_buffer(&self, ctx: Id, kind: c_int, size: u32) -> Result<Id, String> {
        let mut id = INVALID_ID;
        // SAFETY: a null data pointer asks libva to allocate without copying anything; `id` is
        // a live out-parameter.
        let st = unsafe {
            (self.fns.vaCreateBuffer)(self.dpy, ctx, kind, size, 1, std::ptr::null_mut(), &mut id)
        };
        self.check("vaCreateBuffer", st).map(|_| id)
    }

    /// Copies the `w`×`h` top-left area of `image` into `surface` (encoder input upload).
    #[allow(unsafe_code)]
    pub fn put_image(&self, surface: Id, image: Id, w: u32, h: u32) -> Result<(), String> {
        // SAFETY: `dpy` is initialised; ids and the rectangle are validated by libva.
        let st = unsafe { (self.fns.vaPutImage)(self.dpy, surface, image, 0, 0, w, h, 0, 0, w, h) };
        self.check("vaPutImage", st)
    }

    /// Maps an image's buffer for writing and passes its `data_size` bytes to `f`.
    #[allow(unsafe_code)]
    pub fn with_image_data_mut<R>(
        &self,
        img: &Image,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, String> {
        let mut p: *mut c_void = std::ptr::null_mut();
        // SAFETY: `p` is a live out-parameter.
        let st = unsafe { (self.fns.vaMapBuffer)(self.dpy, img.buf, &mut p) };
        self.check("vaMapBuffer", st)?;
        if p.is_null() {
            return Err("vaMapBuffer returned null".into());
        }
        // SAFETY: a mapped image buffer is `data_size` writable bytes, exclusively ours until
        // vaUnmapBuffer, which runs only after `f` (the slice cannot escape it).
        let data = unsafe { std::slice::from_raw_parts_mut(p as *mut u8, img.data_size as usize) };
        let r = f(data);
        // SAFETY: the buffer was mapped above and `data` is no longer used.
        unsafe { (self.fns.vaUnmapBuffer)(self.dpy, img.buf) };
        Ok(r)
    }

    /// Appends the bitstream of a finished coded buffer (every segment, in order) to `out`.
    /// The caller has synced the encoded surface first.
    #[allow(unsafe_code)]
    pub fn read_coded(&self, buf: Id, out: &mut Vec<u8>) -> Result<(), String> {
        let mut p: *mut c_void = std::ptr::null_mut();
        // SAFETY: `p` is a live out-parameter.
        let st = unsafe { (self.fns.vaMapBuffer)(self.dpy, buf, &mut p) };
        self.check("vaMapBuffer", st)?;
        let mut seg = p as *const CodedSegment;
        let mut n = 0;
        while !seg.is_null() && n < 64 {
            // SAFETY: a mapped coded buffer starts with a `VACodedBufferSegment` list owned by
            // the driver; each `next` is null or the next segment, all valid until unmapped.
            let s = unsafe { &*seg };
            if !s.buf.is_null() && s.size > 0 {
                // SAFETY: the segment's `buf` holds `size` readable bytes until unmapped; they
                // are copied out before vaUnmapBuffer.
                let data =
                    unsafe { std::slice::from_raw_parts(s.buf as *const u8, s.size as usize) };
                out.extend_from_slice(data);
            }
            seg = s.next as *const CodedSegment;
            n += 1;
        }
        // SAFETY: the buffer was mapped above and no segment reference outlives this point.
        unsafe { (self.fns.vaUnmapBuffer)(self.dpy, buf) };
        if p.is_null() {
            return Err("vaMapBuffer returned null".into());
        }
        Ok(())
    }
}

impl Drop for Device {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: `dpy` was initialised in `open` and is not used after this.
        unsafe { (self.fns.vaTerminate)(self.dpy) };
    }
}
