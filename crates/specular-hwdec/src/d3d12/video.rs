//! Video decode on D3D12: a device of its own (so a playing clip never holds the still-image
//! decoder's lock), one decoder and heap, a texture per decoded picture buffer slot, and a
//! synchronous decode → read-back per sample. The stream state and HEVC's held pictures are
//! the shared [`crate::video_stream`]; the parameters are the VA-API-layout ones the parsers
//! build, translated to DXVA ([`super::dxva`]). Interlaced H.264 is not decoded here (the
//! decoder would need a field-based configuration): it plays in software.

use std::time::Duration;

use windows::Win32::Graphics::Direct3D12::{D3D12_RESOURCE_FLAG_NONE, ID3D12Device};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_P010};
use windows::Win32::Media::MediaFoundation::{
    D3D12_VIDEO_DECODE_PROFILE_H264, D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN,
    D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10,
};

use super::dxva::{self, Blob};
use super::{
    ALIGN_H, ALIGN_W, Engine, Mode, Scratch, Textures, copy_out, decode_frame, map_planes,
    open_adapter, probe_profile, texture_desc,
};
use crate::Error;
use crate::h264::H264Stream;
use crate::hevc::Limits;
use crate::hevc_video::HevcStream;
use crate::params::{
    INVALID_ID, PICTURE_H264_INVALID, PICTURE_HEVC_INVALID, PicParamH264, PicParamHevc,
};
use crate::video::{Chroma, VideoCodec, VideoFrame};
use crate::video_stream::{Output, Stream, reject};

/// The longest a single picture may take before the device is considered wedged.
const TIMEOUT: Duration = Duration::from_secs(2);

pub struct D3dVideo {
    device: ID3D12Device,
    engine: Engine,
    scratch: Scratch,
    /// One per decoded picture buffer slot; DXVA entries index this list.
    slots: Vec<Textures>,
    mode: Mode,
    ten_bit: bool,
    stream: Stream,
    out: Output,
    /// The slots as the parsers' "surface ids": the slot numbers themselves.
    ids: Vec<u32>,
    status: u32,
}

impl D3dVideo {
    pub fn open(codec: VideoCodec, config: &[u8]) -> Result<D3dVideo, Error> {
        let (device, video, _) = open_adapter().map_err(Error::Hardware)?;
        let (mut stream, profile, format, mode) = match codec {
            VideoCodec::Hevc => {
                let (main, max_w, max_h) = probe_profile(
                    &video,
                    D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN,
                    DXGI_FORMAT_NV12,
                    false,
                )
                .ok_or_else(|| Error::Unsupported("no HEVC Main decoder".into()))?;
                let main10 = probe_profile(
                    &video,
                    D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10,
                    DXGI_FORMAT_P010,
                    true,
                );
                let lim = Limits {
                    max_w,
                    max_h,
                    main10: main10.is_some(),
                };
                let st = HevcStream::new(config, lim).map_err(reject)?;
                if st.bit_depth > 8 {
                    let (m10, _, _) = main10
                        .ok_or_else(|| Error::Unsupported("no HEVC Main 10 decoder".into()))?;
                    (
                        Stream::Hevc(st),
                        D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10,
                        DXGI_FORMAT_P010,
                        m10,
                    )
                } else {
                    (
                        Stream::Hevc(st),
                        D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN,
                        DXGI_FORMAT_NV12,
                        main,
                    )
                }
            }
            VideoCodec::H264 => {
                let (mode, max_w, max_h) = probe_profile(
                    &video,
                    D3D12_VIDEO_DECODE_PROFILE_H264,
                    DXGI_FORMAT_NV12,
                    false,
                )
                .ok_or_else(|| Error::Unsupported("no H.264 decoder".into()))?;
                let st = H264Stream::new(config, max_w, max_h).map_err(reject)?;
                if st.interlaced() {
                    return Err(Error::Unsupported(
                        "interlaced H.264: not decoded in hardware".into(),
                    ));
                }
                (
                    Stream::H264(st),
                    D3D12_VIDEO_DECODE_PROFILE_H264,
                    DXGI_FORMAT_NV12,
                    mode,
                )
            }
        };
        let (w, h) = stream.coded();
        let ten_bit = stream.ten_bit();
        // Every reference, the picture being decoded, and one spare for streams that keep a
        // reference a picture longer than they declare.
        let n = (stream.surfaces_needed() + 1).min(18);
        let (tw, th) = (w.next_multiple_of(ALIGN_W), h.next_multiple_of(ALIGN_H));
        let engine = Engine::new(&device, &video, profile, format, tw, th, n as u32 + 1)?;
        let slots = (0..n)
            .map(|_| Textures::new(&device, tw, th, format, mode.ref_only))
            .collect::<Result<Vec<_>, _>>()?;
        let scratch = Scratch::new(
            &device,
            &texture_desc(tw, th, format, D3D12_RESOURCE_FLAG_NONE),
        )?;
        stream.set_slots(n);
        Ok(D3dVideo {
            device,
            engine,
            scratch,
            slots,
            mode,
            ten_bit,
            stream,
            out: Output::default(),
            ids: (0..n as u32).collect(),
            status: 1,
        })
    }

    /// Forgets the reference pictures and the pictures not handed out yet: the next sample
    /// must be a sync sample.
    pub fn reset(&mut self) {
        self.out.reset(&mut self.stream);
    }

    /// The end of the stream: the pictures still held back are output.
    pub fn flush(&mut self) {
        self.out.flush(&mut self.stream);
    }

    /// The next picture to show, with its sample's tag (decoding order).
    pub fn next_output(&mut self) -> Option<(i64, VideoFrame)> {
        self.out.next()
    }

    /// Decodes one sample (in decode order); its picture, if it is to be shown, comes out of
    /// [`Self::next_output`] tagged with `tag`, once the pictures decoded before it are out.
    pub fn decode(&mut self, sample: &[u8], tag: i64, show: bool) -> Result<(), Error> {
        let r = self.decode_one(sample, tag, show);
        self.out.settle(&mut self.stream);
        r
    }

    fn decode_one(&mut self, sample: &[u8], tag: i64, show: bool) -> Result<(), Error> {
        let status = self.status;
        self.status = self.status.wrapping_add(1).max(1);
        let D3dVideo {
            device,
            engine,
            scratch,
            slots,
            mode,
            ten_bit,
            stream,
            out,
            ids,
            ..
        } = self;
        let list: Vec<&Textures> = slots.iter().collect();
        let size = stream.coded();
        // The slot shown, the HEVC picture's id and output flag, the decode's fence value.
        let (slot, hevc, decoded) = match stream {
            Stream::Hevc(st) => {
                let Some(job) = st.prepare(sample, ids).map_err(reject)? else {
                    return Ok(());
                };
                if let Err(e) = crate::gate::hevc(&job.pic, job.iq.as_ref(), &job.slices, ids, size)
                {
                    st.forget_last();
                    return Err(e.into());
                }
                let (buf, mut controls) = dxva::bitstream(&job.slices);
                let mut pp = dxva::pic_params(
                    &job.pic,
                    job.uniform_spacing,
                    job.num_delta_pocs_of_ref_rps_idx,
                    status,
                );
                let mut qm = job.iq.as_ref().map(dxva::qmatrix);
                let reads = hevc_refs(&job.pic, job.slot);
                let r = decode_frame(
                    engine,
                    scratch,
                    device,
                    *mode,
                    &list,
                    job.slot,
                    &reads,
                    &mut pp,
                    qm.as_mut().map(|q| q as &mut dyn Blob),
                    &buf,
                    &mut controls,
                );
                match r {
                    Ok(v) => (job.slot, Some((job.id, job.output)), v),
                    Err(e) => {
                        st.forget_last();
                        return Err(e);
                    }
                }
            }
            Stream::H264(st) => {
                let acc = st.prepare(sample, ids).map_err(reject)?;
                let mut last = Ok(0);
                for job in &acc.jobs {
                    if let Err(e) =
                        crate::gate::h264(&job.pic, Some(&job.iq), &job.slices, ids, size)
                    {
                        last = Err(e.into());
                        break;
                    }
                    let (buf, mut controls) = dxva::bitstream(&job.slices);
                    let intra = job.slices.iter().all(|(sp, _)| sp.slice_type % 5 == 2);
                    let first = job
                        .slices
                        .first()
                        .map(|(sp, _)| *sp)
                        .ok_or_else(|| Error::Malformed("a picture without slices".into()))?;
                    let mut pp = dxva::pic_params_h264(&job.pic, &first, intra, status);
                    let mut qm = dxva::qmatrix_h264(&job.iq);
                    let reads = h264_refs(&job.pic, job.slot);
                    last = decode_frame(
                        engine,
                        scratch,
                        device,
                        *mode,
                        &list,
                        job.slot,
                        &reads,
                        &mut pp,
                        Some(&mut qm),
                        &buf,
                        &mut controls,
                    );
                    if last.is_err() {
                        break;
                    }
                }
                match (last, acc.show) {
                    (Err(e), _) => {
                        st.forget_last();
                        return Err(e);
                    }
                    (Ok(v), Some(slot)) => (slot, None, v),
                    (Ok(_), None) => return Ok(()),
                }
            }
        };
        // Every picture is waited for (as the VA-API path syncs each surface): a wedged
        // decoder is noticed on the picture it stops at.
        if let Err(e) = engine.wait(decoded, TIMEOUT) {
            if e.to_string().contains("timed out") {
                crate::heic::mark_wedged();
            }
            stream.forget_last();
            return Err(e);
        }
        if !show || hevc.is_some_and(|(_, output)| !output) {
            // Decoded as a reference only (pic_output_flag 0, or a picture the caller will
            // not show): nothing to read back.
            return Ok(());
        }
        let copied = copy_out(engine, scratch, &slots[slot].texture, decoded)?;
        let planes = map_planes(engine, scratch, copied, TIMEOUT, *ten_bit, stream.region())?;
        let frame = VideoFrame {
            width: planes.width,
            height: planes.height,
            bit_depth: planes.bit_depth,
            chroma: Chroma::Yuv420,
            planes: planes.planes,
            signal: stream.signal(),
        };
        out.push(hevc.map(|(id, _)| id), tag, frame);
        Ok(())
    }
}

/// The slots an HEVC picture references (not the one it decodes into).
fn hevc_refs(p: &PicParamHevc, current: usize) -> Vec<usize> {
    p.reference_frames
        .iter()
        .filter(|r| r.picture_id != INVALID_ID && r.flags & PICTURE_HEVC_INVALID == 0)
        .map(|r| r.picture_id as usize)
        .filter(|&s| s != current)
        .collect()
}

/// The slots an H.264 picture references (not the one it decodes into).
fn h264_refs(p: &PicParamH264, current: usize) -> Vec<usize> {
    p.reference_frames
        .iter()
        .filter(|r| r.picture_id != INVALID_ID && r.flags & PICTURE_H264_INVALID == 0)
        .map(|r| r.picture_id as usize)
        .filter(|&s| s != current)
        .collect()
}
