//! H.264 encoding on the GPU's encoder through Media Foundation, for the slideshow export: the
//! hardware encoder MFT the GPU's driver registers (AMD, Intel and NVIDIA each ship one), the
//! same stream the VA-API path makes (one IDR then P pictures, no B pictures, constant QP,
//! CABAC where the encoder offers it) and the same contract: the SPS and PPS are known when the
//! encoder opens (a warm-up picture), every picture comes back as AVCC NAL units, and any
//! failure is the caller's cue to fall back to the software encoder.
//!
//! The input arrives as planar 4:2:0 and is interleaved into an NV12 system-memory sample on
//! the CPU (hardware MFTs take those; no D3D device manager is set). Hardware MFTs are
//! asynchronous: the transform is driven by its `METransformNeedInput` / `METransformHaveOutput`
//! events, one picture in and one out, in low-latency mode so a picture's output is there
//! before the next is asked for. An encoder that holds pictures back fails the export over to
//! the software encoder.

use std::cell::Cell;
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{E_NOTIMPL, VARIANT_BOOL};
use windows::Win32::Media::MediaFoundation::{
    CODECAPI_AVEncCommonLowLatency, CODECAPI_AVEncCommonRateControlMode,
    CODECAPI_AVEncH264CABACEnable, CODECAPI_AVEncMPVDefaultBPictureCount, CODECAPI_AVEncMPVGOPSize,
    CODECAPI_AVEncVideoEncodeQP, CODECAPI_AVEncVideoForceKeyFrame,
    CODECAPI_AVEncVideoMaxNumRefFrame, CODECAPI_AVLowLatencyMode, ICodecAPI, IMFActivate,
    IMFMediaEventGenerator, IMFMediaType, IMFSample, IMFTransform, METransformHaveOutput,
    METransformNeedInput, MF_E_NO_EVENTS_AVAILABLE, MF_E_TRANSFORM_STREAM_CHANGE,
    MF_EVENT_FLAG_NO_WAIT, MF_LOW_LATENCY, MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE,
    MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE,
    MF_MT_MPEG_SEQUENCE_HEADER, MF_MT_MPEG2_PROFILE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_TRANSFORM_ASYNC, MF_TRANSFORM_ASYNC_UNLOCK, MF_VERSION, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFMediaType_Video, MFSTARTUP_FULL,
    MFSampleExtension_VideoEncodePictureType, MFSampleExtension_VideoEncodeQP, MFStartup,
    MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE, MFT_ENUM_FLAG_SORTANDFILTER,
    MFT_FRIENDLY_NAME_Attribute, MFT_MESSAGE_COMMAND_FLUSH, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
    MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_OUTPUT_DATA_BUFFER,
    MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES,
    MFT_REGISTER_TYPE_INFO, MFTEnumEx, MFVideoFormat_H264, MFVideoFormat_NV12,
    MFVideoInterlace_Progressive, eAVEncCommonRateControlMode_Quality, eAVEncH264PictureType_IDR,
    eAVEncH264VProfile, eAVEncH264VProfile_High, eAVEncH264VProfile_Main,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree};
use windows::Win32::System::Variant::{
    VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4, VT_UI8,
};
use windows::core::{GUID, Interface, PWSTR};

use crate::encode::{EncodeConfig, EncodeProfile, annex_b_nals, collect_avcc};
use crate::{CodecCaps, Error};

/// The longest a picture may take before the encoder is considered wedged.
const TIMEOUT: Duration = Duration::from_secs(2);

/// Pictures above this are not offered to the MFT (the drivers' H.264 encoders stop there; the
/// MFTs do not report their limit).
const MAX_DIM: u32 = 4096;

fn hw(e: windows::core::Error) -> Error {
    Error::Hardware(e.to_string())
}

/// Media Foundation, started once for the process and never shut down (one export may follow
/// another).
fn startup() -> Result<(), Error> {
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED
        .get_or_init(|| {
            // SAFETY: a plain call without pointers.
            #[allow(unsafe_code)]
            let r = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) };
            r.map_err(|e| e.to_string())
        })
        .clone()
        .map_err(Error::Hardware)
}

/// COM on this thread (the MFT is created with `CoCreateInstance`): initialised once per
/// thread and never uninitialised. A thread already in another apartment keeps it.
fn com_init() {
    thread_local! {
        static DONE: Cell<bool> = const { Cell::new(false) };
    }
    if DONE.replace(true) {
        return;
    }
    // SAFETY: a plain call without pointers; its failure modes (already initialised, another
    // apartment model) leave COM usable on this thread.
    #[allow(unsafe_code)]
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// The friendly name of an MFT ("AMDh264Encoder"), or a stand-in.
fn friendly_name(a: &IMFActivate) -> String {
    let mut s = PWSTR::null();
    let mut n = 0u32;
    // SAFETY: the out-pointers are to locals; the string comes back CoTaskMemAlloc'd.
    #[allow(unsafe_code)]
    let r = unsafe { a.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut s, &mut n) };
    if r.is_err() || s.is_null() {
        return "hardware H.264 encoder".into();
    }
    // SAFETY: a NUL-terminated string Media Foundation allocated.
    #[allow(unsafe_code)]
    let name = unsafe { s.to_string() };
    // SAFETY: freeing what GetAllocatedString allocated, once.
    #[allow(unsafe_code)]
    let () = unsafe { CoTaskMemFree(Some(s.0 as *const _)) };
    name.unwrap_or_default()
}

/// The hardware H.264 encoder MFTs of this machine, best first, with their names.
fn encoders() -> Result<Vec<(IMFActivate, String)>, Error> {
    startup()?;
    com_init();
    let input = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };
    let output = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };
    let mut list: *mut Option<IMFActivate> = ptr::null_mut();
    let mut n = 0u32;
    // SAFETY: the type infos and out-pointers are locals; the array comes back
    // CoTaskMemAlloc'd with n entries.
    #[allow(unsafe_code)]
    let r = unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input),
            Some(&output),
            &mut list,
            &mut n,
        )
    };
    r.map_err(hw)?;
    let mut found = Vec::new();
    if list.is_null() {
        return Ok(found);
    }
    for i in 0..n as usize {
        // SAFETY: MFTEnumEx filled n entries; each is read out once, taking its reference.
        #[allow(unsafe_code)]
        let a = unsafe { ptr::read(list.wrapping_add(i)) };
        if let Some(a) = a {
            let name = friendly_name(&a);
            found.push((a, name));
        }
    }
    // SAFETY: the array MFTEnumEx allocated, its entries moved out above; freed once.
    #[allow(unsafe_code)]
    let () = unsafe { CoTaskMemFree(Some(list as *const _)) };
    Ok(found)
}

/// The encoder this machine offers, for the capability report: the first MFT (the limits are
/// the common ones; the MFTs do not report theirs).
pub fn caps() -> Option<CodecCaps> {
    let (_, name) = encoders().ok()?.into_iter().next()?;
    Some(CodecCaps {
        max_width: MAX_DIM,
        max_height: MAX_DIM,
        formats: format!("4:2:0, High or Main, {name}"),
    })
}

/// `CODECAPI_AVEncVideoEncodeQP` and `MFSampleExtension_VideoEncodeQP`: the QP of I, P and B
/// pictures in 16-bit fields.
fn qp_triplet(qp: u8) -> u64 {
    let q = u64::from(qp.clamp(10, 51));
    q | q << 16 | q << 32
}

fn variant(vt: VARENUM, value: VARIANT_0_0_0) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: value,
            }),
        },
    }
}

fn ui4(v: u32) -> VARIANT {
    variant(VT_UI4, VARIANT_0_0_0 { ulVal: v })
}

fn ui8(v: u64) -> VARIANT {
    variant(VT_UI8, VARIANT_0_0_0 { ullVal: v })
}

fn vbool(b: bool) -> VARIANT {
    variant(
        VT_BOOL,
        VARIANT_0_0_0 {
            boolVal: VARIANT_BOOL(if b { -1 } else { 0 }),
        },
    )
}

/// Sets one codec property.
fn set(c: &ICodecAPI, key: &GUID, value: VARIANT) -> Result<(), Error> {
    // SAFETY: the key and value are locals that outlive the call, which copies the value.
    #[allow(unsafe_code)]
    let r = unsafe { c.SetValue(key, &value) };
    r.map_err(hw)
}

pub struct MfEnc {
    activate: IMFActivate,
    mft: IMFTransform,
    /// The event queue of an asynchronous MFT (every hardware one so far); `None` drives a
    /// synchronous one with `ProcessOutput` directly.
    events: Option<IMFMediaEventGenerator>,
    codec: Option<ICodecAPI>,
    pub profile: EncodeProfile,
    /// The MFT's friendly name, for logs.
    pub device: String,
    cfg: EncodeConfig,
    in_id: u32,
    out_id: u32,
    /// Output buffer size for an MFT that does not provide its output samples (0: it does).
    out_size: u32,
    /// Pictures since the last IDR (0: the next one is an IDR).
    since_idr: u32,
    /// Pictures handed in, for the sample times.
    count: u64,
    /// `METransformNeedInput` events received and not used yet.
    credits: u32,
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
    nv12: Vec<u8>,
}

// SAFETY: the MFT is created in the multithreaded apartment (`com_init`) and Media Foundation
// objects are free-threaded; the encoder moves to the export's thread and is used from one
// thread at a time.
#[allow(unsafe_code)]
unsafe impl Send for MfEnc {}

impl Drop for MfEnc {
    fn drop(&mut self) {
        for m in [MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_COMMAND_FLUSH] {
            // SAFETY: messages to a live transform; a failure (never streamed) does not matter.
            #[allow(unsafe_code)]
            let _ = unsafe { self.mft.ProcessMessage(m, 0) };
        }
        // SAFETY: the activation object releases the MFT's resources; once, at the end.
        #[allow(unsafe_code)]
        let _ = unsafe { self.activate.ShutdownObject() };
    }
}

impl MfEnc {
    /// Opens the first hardware encoder that takes `cfg`; the SPS and PPS are known when this
    /// returns.
    pub fn open(cfg: &EncodeConfig) -> Result<MfEnc, Error> {
        let (w, h) = (cfg.width, cfg.height);
        if w == 0 || h == 0 || w % 2 != 0 || h % 2 != 0 || cfg.fps_num == 0 || cfg.fps_den == 0 {
            return Err(Error::Unsupported("size or frame rate".into()));
        }
        if w > MAX_DIM || h > MAX_DIM {
            return Err(Error::Unsupported(format!(
                "{w}×{h} is above the encoder's {MAX_DIM}×{MAX_DIM}"
            )));
        }
        let list = encoders()?;
        if list.is_empty() {
            return Err(Error::Unsupported("no hardware H.264 encoder".into()));
        }
        let mut last = Error::Unavailable;
        for (activate, name) in list {
            let opened = MfEnc::with_activate(activate, name, cfg).and_then(|mut e| {
                e.warm_up()?;
                Ok(e)
            });
            match opened {
                Ok(e) => return Ok(e),
                Err(e) => {
                    specular_log::debug!(
                        target: specular_log::DECODE,
                        "Media Foundation H.264 encoder: {e}"
                    );
                    last = e;
                }
            }
        }
        Err(last)
    }

    fn with_activate(
        activate: IMFActivate,
        device: String,
        cfg: &EncodeConfig,
    ) -> Result<MfEnc, Error> {
        // SAFETY: activation of the enumerated MFT; the result is a smart pointer.
        #[allow(unsafe_code)]
        let mft: IMFTransform = unsafe { activate.ActivateObject() }.map_err(hw)?;
        let codec = mft.cast().ok();
        let mut enc = MfEnc {
            activate,
            mft,
            events: None,
            codec,
            profile: EncodeProfile::High,
            device,
            cfg: cfg.clone(),
            in_id: 0,
            out_id: 0,
            out_size: 0,
            since_idr: 0,
            count: 0,
            credits: 0,
            sps: Vec::new(),
            pps: Vec::new(),
            nv12: Vec::new(),
        };
        enc.configure()?;
        Ok(enc)
    }

    /// Types, codec properties and the start of streaming.
    fn configure(&mut self) -> Result<(), Error> {
        let cfg = self.cfg.clone();
        // SAFETY: a getter on a live transform.
        #[allow(unsafe_code)]
        let attrs = unsafe { self.mft.GetAttributes() }.ok();
        if let Some(a) = &attrs {
            // SAFETY: the key is a static GUID.
            #[allow(unsafe_code)]
            let is_async = unsafe { a.GetUINT32(&MF_TRANSFORM_ASYNC) }.unwrap_or(0) == 1;
            if is_async {
                // SAFETY: as above.
                #[allow(unsafe_code)]
                let r = unsafe { a.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1) };
                r.map_err(hw)?;
                self.events = Some(self.mft.cast().map_err(hw)?);
            }
            // SAFETY: as above; a hint the MFT may ignore.
            #[allow(unsafe_code)]
            let _ = unsafe { a.SetUINT32(&MF_LOW_LATENCY, 1) };
        }
        let (mut ins, mut outs) = ([0u32], [0u32]);
        // SAFETY: the id slices are locals of the length passed.
        #[allow(unsafe_code)]
        let r = unsafe { self.mft.GetStreamIDs(&mut ins, &mut outs) };
        match r {
            Ok(()) => {
                self.in_id = ins[0];
                self.out_id = outs[0];
            }
            // Fixed stream ids 0.
            Err(e) if e.code() == E_NOTIMPL => {}
            Err(e) => return Err(hw(e)),
        }
        let codec = self.codec.clone().ok_or_else(|| {
            Error::Unsupported("the encoder has no ICodecAPI (no constant QP)".into())
        })?;
        // Rate control first: some encoders fix it when the output type is set; others take
        // it only afterwards.
        let quality = ui4(eAVEncCommonRateControlMode_Quality.0 as u32);
        let early = set(&codec, &CODECAPI_AVEncCommonRateControlMode, quality);
        let mut profile = None;
        for (p, mf) in [
            (EncodeProfile::High, eAVEncH264VProfile_High),
            (EncodeProfile::Main, eAVEncH264VProfile_Main),
        ] {
            let t = self.media_type(MFVideoFormat_H264, Some(mf))?;
            // SAFETY: the media type outlives the call, which keeps its own reference.
            #[allow(unsafe_code)]
            let r = unsafe { self.mft.SetOutputType(self.out_id, &t, 0) };
            if r.is_ok() {
                profile = Some(p);
                break;
            }
        }
        self.profile =
            profile.ok_or_else(|| Error::Unsupported("neither High nor Main is offered".into()))?;
        if early.is_err() {
            let quality = ui4(eAVEncCommonRateControlMode_Quality.0 as u32);
            set(&codec, &CODECAPI_AVEncCommonRateControlMode, quality)
                .map_err(|_| Error::Unsupported("no quality (constant QP) rate control".into()))?;
        }
        set(
            &codec,
            &CODECAPI_AVEncVideoEncodeQP,
            ui8(qp_triplet(cfg.qp)),
        )
        .map_err(|_| Error::Unsupported("no constant QP".into()))?;
        // The rest is best effort: the stream is checked for what it is afterwards.
        let _ = set(&codec, &CODECAPI_AVEncMPVGOPSize, ui4(cfg.keyint.max(1)));
        let _ = set(&codec, &CODECAPI_AVEncMPVDefaultBPictureCount, ui4(0));
        let _ = set(&codec, &CODECAPI_AVEncVideoMaxNumRefFrame, ui4(1));
        let _ = set(&codec, &CODECAPI_AVLowLatencyMode, vbool(true));
        let _ = set(&codec, &CODECAPI_AVEncCommonLowLatency, vbool(true));
        let _ = set(&codec, &CODECAPI_AVEncH264CABACEnable, vbool(true));
        let t = self.media_type(MFVideoFormat_NV12, None)?;
        // SAFETY: as for the output type.
        #[allow(unsafe_code)]
        let r = unsafe { self.mft.SetInputType(self.in_id, &t, 0) };
        r.map_err(|e| {
            Error::Unsupported(format!(
                "NV12 {}×{} input refused: {e}",
                cfg.width, cfg.height
            ))
        })?;
        // SAFETY: a getter on a live transform.
        #[allow(unsafe_code)]
        let info = unsafe { self.mft.GetOutputStreamInfo(self.out_id) }.map_err(hw)?;
        let provides =
            (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0) as u32;
        self.out_size = if info.dwFlags & provides != 0 {
            0
        } else {
            info.cbSize.max(cfg.width * cfg.height * 3 / 2)
        };
        for m in [
            MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
            MFT_MESSAGE_NOTIFY_START_OF_STREAM,
        ] {
            // SAFETY: messages without parameters to a live transform.
            #[allow(unsafe_code)]
            let r = unsafe { self.mft.ProcessMessage(m, 0) };
            r.map_err(hw)?;
        }
        Ok(())
    }

    /// A video media type of this stream in `subtype`; an H.264 one with `profile`.
    fn media_type(
        &self,
        subtype: GUID,
        profile: Option<eAVEncH264VProfile>,
    ) -> Result<IMFMediaType, Error> {
        let c = &self.cfg;
        // SAFETY: a plain constructor.
        #[allow(unsafe_code)]
        let t = unsafe { MFCreateMediaType() }.map_err(hw)?;
        let set_guid = |k: &GUID, v: &GUID| {
            // SAFETY: the key and value are static GUIDs.
            #[allow(unsafe_code)]
            let r = unsafe { t.SetGUID(k, v) };
            r.map_err(hw)
        };
        let set32 = |k: &GUID, v: u32| {
            // SAFETY: the key is a static GUID.
            #[allow(unsafe_code)]
            let r = unsafe { t.SetUINT32(k, v) };
            r.map_err(hw)
        };
        let set64 = |k: &GUID, v: u64| {
            // SAFETY: the key is a static GUID.
            #[allow(unsafe_code)]
            let r = unsafe { t.SetUINT64(k, v) };
            r.map_err(hw)
        };
        set_guid(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        set_guid(&MF_MT_SUBTYPE, &subtype)?;
        set64(
            &MF_MT_FRAME_SIZE,
            u64::from(c.width) << 32 | u64::from(c.height),
        )?;
        set64(
            &MF_MT_FRAME_RATE,
            u64::from(c.fps_num) << 32 | u64::from(c.fps_den),
        )?;
        set64(&MF_MT_PIXEL_ASPECT_RATIO, 1 << 32 | 1)?;
        set32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        match profile {
            Some(p) => {
                set32(&MF_MT_MPEG2_PROFILE, p.0 as u32)?;
                // Quality mode ignores the bit rate, but the type must carry one: a generous
                // one, so an encoder that caps at it does not starve the pictures.
                let fps = f64::from(c.fps_num) / f64::from(c.fps_den);
                let bits = (f64::from(c.width) * f64::from(c.height) * fps * 0.3).clamp(1e6, 2e8);
                set32(&MF_MT_AVG_BITRATE, bits as u32)?;
            }
            None => set32(&MF_MT_DEFAULT_STRIDE, c.width)?,
        }
        Ok(t)
    }

    /// Encodes one mid-grey IDR to learn the encoder's SPS and PPS, then starts over.
    fn warm_up(&mut self) -> Result<(), Error> {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let (cw, ch) = (w / 2, h / 2);
        let y = vec![128u8; w * h];
        let c = vec![128u8; cw * ch];
        self.encode(&y, &c, &c, w, cw, true, self.cfg.qp)?;
        if self.sps.is_empty() || self.pps.is_empty() {
            self.sequence_header()?;
        }
        self.since_idr = 0;
        Ok(())
    }

    /// The SPS and PPS from the output type's sequence header, for an encoder that writes
    /// none into its stream.
    fn sequence_header(&mut self) -> Result<(), Error> {
        let none = || Error::Unsupported("the encoder writes no SPS/PPS".into());
        // SAFETY: a getter on a live transform.
        #[allow(unsafe_code)]
        let t = unsafe { self.mft.GetOutputCurrentType(self.out_id) }.map_err(hw)?;
        let mut p: *mut u8 = ptr::null_mut();
        let mut n = 0u32;
        // SAFETY: the out-pointers are to locals; the blob comes back CoTaskMemAlloc'd.
        #[allow(unsafe_code)]
        let r = unsafe { t.GetAllocatedBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut p, &mut n) };
        if r.is_err() || p.is_null() {
            return Err(none());
        }
        // SAFETY: n bytes Media Foundation allocated at p, copied before they are freed.
        #[allow(unsafe_code)]
        let blob = unsafe { std::slice::from_raw_parts(p, n as usize) }.to_vec();
        // SAFETY: freeing what GetAllocatedBlob allocated, once.
        #[allow(unsafe_code)]
        let () = unsafe { CoTaskMemFree(Some(p as *const _)) };
        for nal in annex_b_nals(&blob) {
            match nal[0] & 0x1f {
                7 if self.sps.is_empty() => self.sps = nal.to_vec(),
                8 if self.pps.is_empty() => self.pps = nal.to_vec(),
                _ => {}
            }
        }
        if self.sps.is_empty() || self.pps.is_empty() {
            return Err(none());
        }
        Ok(())
    }

    pub fn qp(&self) -> u8 {
        self.cfg.qp
    }

    /// Encodes one picture (planar 4:2:0) at quantiser `qp`: its AVCC NAL units and whether it
    /// is an IDR picture. The first picture and every `keyint`-th are IDR, as is any with
    /// `force_idr`; an IDR the encoder makes on its own is reported as one.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        y: &[u8],
        u: &[u8],
        v: &[u8],
        y_stride: usize,
        c_stride: usize,
        force_idr: bool,
        qp: u8,
    ) -> Result<(Vec<u8>, bool), Error> {
        let idr = force_idr || self.since_idr == 0 || self.since_idr >= self.cfg.keyint.max(1);
        let sample = self.input_sample(y, u, v, y_stride, c_stride, idr, qp)?;
        if idr && let Some(c) = &self.codec {
            // Beside the sample's picture type: not every encoder reads both.
            let _ = set(c, &CODECAPI_AVEncVideoForceKeyFrame, ui4(1));
        }
        let coded = self.run(&sample)?;
        let (out, key) = collect_avcc(&coded, &mut self.sps, &mut self.pps)?;
        if idr && !key {
            return Err(Error::Hardware(
                "the encoder did not code the IDR picture asked for".into(),
            ));
        }
        self.since_idr = if key { 1 } else { self.since_idr + 1 };
        self.count += 1;
        Ok((out, key))
    }

    /// The picture as an NV12 sample, timed, with its picture type and QP where they differ
    /// from the stream's.
    #[allow(clippy::too_many_arguments)]
    fn input_sample(
        &mut self,
        y: &[u8],
        u: &[u8],
        v: &[u8],
        y_stride: usize,
        c_stride: usize,
        idr: bool,
        qp: u8,
    ) -> Result<IMFSample, Error> {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let (cw, ch) = (w / 2, h / 2);
        if y.len() < y_stride * (h - 1) + w
            || u.len() < c_stride * (ch - 1) + cw
            || v.len() < c_stride * (ch - 1) + cw
        {
            return Err(Error::Malformed("planes too small for the picture".into()));
        }
        let len = w * h * 3 / 2;
        self.nv12.resize(len, 0);
        for r in 0..h {
            self.nv12[r * w..][..w].copy_from_slice(&y[r * y_stride..][..w]);
        }
        for r in 0..ch {
            let (su, sv) = (&u[r * c_stride..][..cw], &v[r * c_stride..][..cw]);
            let dst = &mut self.nv12[w * h + r * w..][..w];
            for (i, pair) in dst.chunks_exact_mut(2).enumerate() {
                pair[0] = su[i];
                pair[1] = sv[i];
            }
        }
        // SAFETY: a plain constructor.
        #[allow(unsafe_code)]
        let buffer = unsafe { MFCreateMemoryBuffer(len as u32) }.map_err(hw)?;
        let mut p: *mut u8 = ptr::null_mut();
        // SAFETY: the out-pointer is to a local.
        #[allow(unsafe_code)]
        let r = unsafe { buffer.Lock(&mut p, None, None) };
        r.map_err(hw)?;
        // SAFETY: the buffer was made with len bytes and is locked for writing at p.
        #[allow(unsafe_code)]
        let () = unsafe { ptr::copy_nonoverlapping(self.nv12.as_ptr(), p, len) };
        // SAFETY: the matching unlock.
        #[allow(unsafe_code)]
        let r = unsafe { buffer.Unlock() };
        r.map_err(hw)?;
        // SAFETY: within the buffer's size.
        #[allow(unsafe_code)]
        let r = unsafe { buffer.SetCurrentLength(len as u32) };
        r.map_err(hw)?;
        // SAFETY: a plain constructor.
        #[allow(unsafe_code)]
        let sample = unsafe { MFCreateSample() }.map_err(hw)?;
        // SAFETY: the sample keeps its own reference to the buffer.
        #[allow(unsafe_code)]
        let r = unsafe { sample.AddBuffer(&buffer) };
        r.map_err(hw)?;
        let hns =
            |n: u64| n * 10_000_000 * u64::from(self.cfg.fps_den) / u64::from(self.cfg.fps_num);
        // SAFETY: setters with plain values.
        #[allow(unsafe_code)]
        let r = unsafe { sample.SetSampleTime(hns(self.count) as i64) };
        r.map_err(hw)?;
        // SAFETY: as above.
        #[allow(unsafe_code)]
        let r = unsafe { sample.SetSampleDuration(hns(1).max(1) as i64) };
        r.map_err(hw)?;
        if idr {
            // SAFETY: the key is a static GUID; an encoder may ignore the attribute.
            #[allow(unsafe_code)]
            let _ = unsafe {
                sample.SetUINT32(
                    &MFSampleExtension_VideoEncodePictureType,
                    eAVEncH264PictureType_IDR.0 as u32,
                )
            };
        }
        if qp != self.cfg.qp {
            // SAFETY: as above.
            #[allow(unsafe_code)]
            let _ = unsafe { sample.SetUINT64(&MFSampleExtension_VideoEncodeQP, qp_triplet(qp)) };
        }
        Ok(sample)
    }

    fn process_input(&self, sample: &IMFSample) -> Result<(), Error> {
        // SAFETY: the transform keeps its own reference to the sample.
        #[allow(unsafe_code)]
        let r = unsafe { self.mft.ProcessInput(self.in_id, sample, 0) };
        r.map_err(hw)
    }

    /// One picture through the transform: its coded bytes (Annex B).
    fn run(&mut self, sample: &IMFSample) -> Result<Vec<u8>, Error> {
        let Some(events) = self.events.clone() else {
            self.process_input(sample)?;
            return self.take_output();
        };
        let deadline = Instant::now() + TIMEOUT;
        let mut fed = false;
        if self.credits > 0 {
            self.credits -= 1;
            self.process_input(sample)?;
            fed = true;
        }
        loop {
            // SAFETY: a non-blocking poll of the transform's event queue.
            #[allow(unsafe_code)]
            let ev = unsafe { events.GetEvent(MF_EVENT_FLAG_NO_WAIT) };
            let ev = match ev {
                Ok(ev) => ev,
                Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => {
                    if Instant::now() > deadline {
                        return Err(Error::Hardware(if fed && self.credits > 0 {
                            "the encoder holds pictures back (it asked for more input before \
                             giving output)"
                                .into()
                        } else {
                            "the encoder gave no output within 2 s".into()
                        }));
                    }
                    std::thread::sleep(Duration::from_micros(250));
                    continue;
                }
                Err(e) => return Err(hw(e)),
            };
            // SAFETY: a getter on the event.
            #[allow(unsafe_code)]
            let kind = unsafe { ev.GetType() }.map_err(hw)?;
            if kind == METransformNeedInput.0 as u32 {
                if fed {
                    self.credits += 1;
                } else {
                    self.process_input(sample)?;
                    fed = true;
                }
            } else if kind == METransformHaveOutput.0 as u32 {
                return self.take_output();
            }
        }
    }

    /// A sample with an output buffer, for an MFT that does not provide its own.
    fn output_sample(&self) -> Result<IMFSample, Error> {
        // SAFETY: plain constructors.
        #[allow(unsafe_code)]
        let sample = unsafe { MFCreateSample() }.map_err(hw)?;
        // SAFETY: as above.
        #[allow(unsafe_code)]
        let buffer = unsafe { MFCreateMemoryBuffer(self.out_size) }.map_err(hw)?;
        // SAFETY: the sample keeps its own reference to the buffer.
        #[allow(unsafe_code)]
        let r = unsafe { sample.AddBuffer(&buffer) };
        r.map_err(hw)?;
        Ok(sample)
    }

    /// The output type again after the MFT changed it (its own choice of attributes).
    fn renegotiate(&mut self) -> Result<(), Error> {
        // SAFETY: getters and setters on a live transform; the type outlives the call.
        #[allow(unsafe_code)]
        let t = unsafe { self.mft.GetOutputAvailableType(self.out_id, 0) }.map_err(hw)?;
        // SAFETY: as above.
        #[allow(unsafe_code)]
        let r = unsafe { self.mft.SetOutputType(self.out_id, &t, 0) };
        r.map_err(hw)
    }

    /// The coded picture waiting in the transform.
    fn take_output(&mut self) -> Result<Vec<u8>, Error> {
        let own = if self.out_size == 0 {
            None
        } else {
            Some(self.output_sample()?)
        };
        let mut out = MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: self.out_id,
            pSample: ManuallyDrop::new(own),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        };
        let mut status = 0u32;
        // SAFETY: the buffer array is one local entry; the transform fills its sample and
        // events, which are taken out below.
        #[allow(unsafe_code)]
        let mut r = unsafe {
            self.mft
                .ProcessOutput(0, std::slice::from_mut(&mut out), &mut status)
        };
        if r.as_ref()
            .is_err_and(|e| e.code() == MF_E_TRANSFORM_STREAM_CHANGE)
        {
            self.renegotiate()?;
            // SAFETY: as above, with the new output type.
            #[allow(unsafe_code)]
            let again = unsafe {
                self.mft
                    .ProcessOutput(0, std::slice::from_mut(&mut out), &mut status)
            };
            r = again;
        }
        // SAFETY: the buffer's sample and event collection are ours, taken out once each.
        #[allow(unsafe_code)]
        let sample = unsafe { ManuallyDrop::take(&mut out.pSample) };
        // SAFETY: as above.
        #[allow(unsafe_code)]
        let events = unsafe { ManuallyDrop::take(&mut out.pEvents) };
        drop(events);
        r.map_err(hw)?;
        let sample = sample
            .ok_or_else(|| Error::Hardware("the encoder returned no output sample".into()))?;
        // SAFETY: a getter on the sample; the result is a smart pointer.
        #[allow(unsafe_code)]
        let buffer = unsafe { sample.ConvertToContiguousBuffer() }.map_err(hw)?;
        let mut p: *mut u8 = ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: the out-pointers are to locals.
        #[allow(unsafe_code)]
        let r = unsafe { buffer.Lock(&mut p, None, Some(&mut len)) };
        r.map_err(hw)?;
        let coded = if len == 0 || p.is_null() {
            Vec::new()
        } else {
            // SAFETY: the buffer is locked and holds len bytes at p; copied before the unlock.
            #[allow(unsafe_code)]
            let bytes = unsafe { std::slice::from_raw_parts(p, len as usize) };
            bytes.to_vec()
        };
        // SAFETY: the matching unlock.
        #[allow(unsafe_code)]
        let r = unsafe { buffer.Unlock() };
        r.map_err(hw)?;
        Ok(coded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qp_fields() {
        assert_eq!(qp_triplet(24), 24 | 24 << 16 | 24 << 32);
        assert_eq!(qp_triplet(0), 10 | 10 << 16 | 10 << 32);
    }

    #[test]
    fn variants_are_tagged() {
        for (v, vt) in [(ui4(1), VT_UI4), (ui8(1), VT_UI8), (vbool(true), VT_BOOL)] {
            // SAFETY: the variant was built with its Anonymous member.
            #[allow(unsafe_code)]
            let tag = unsafe { v.Anonymous.Anonymous.vt };
            assert_eq!(tag, vt);
        }
    }
}
