//! Video decode on VA-API: a device of its own (so a playing clip never holds the still-image
//! decoder's lock), one decode config, a surface pool sized to the stream's DPB, and a
//! synchronous decode → read-back per sample. HEVC pictures are held back (read back, in
//! decoding order) until the stream's output process says whether they are output at all.

use super::ffi::{self, Device, Pod, fourcc};
use super::{HevcHw, HevcPool, copy_420, release_pool, render_nodes};
use crate::Error;
use crate::h264::H264Stream;
use crate::hevc_video::HevcStream;
use crate::video::{VideoCodec, VideoFrame};
use crate::video_stream::{Output, Stream, reject};

/// The longest a single picture may take before the device is considered wedged.
const TIMEOUT_NS: u64 = 2_000_000_000;

pub struct VaVideo {
    dev: Device,
    cfg: ffi::Id,
    pool: Option<HevcPool>,
    stream: Stream,
    out: Output,
}

/// An H.264 High VLD config for 8-bit 4:2:0 (High decodes Constrained Baseline and Main
/// streams too) and the largest picture it takes.
fn h264_config(dev: &Device) -> Option<(ffi::Id, u32, u32)> {
    let p = ffi::PROFILE_H264_HIGH;
    if !dev.profiles().contains(&p) || !dev.entrypoints(p).contains(&ffi::ENTRYPOINT_VLD) {
        return None;
    }
    let a = dev.attributes(
        p,
        ffi::ENTRYPOINT_VLD,
        &[
            ffi::ATTRIB_RT_FORMAT,
            ffi::ATTRIB_MAX_PICTURE_WIDTH,
            ffi::ATTRIB_MAX_PICTURE_HEIGHT,
        ],
    );
    if a[0] == ffi::ATTRIB_NOT_SUPPORTED || a[0] & ffi::RT_FORMAT_YUV420 == 0 {
        return None;
    }
    let known = |v: u32| v != ffi::ATTRIB_NOT_SUPPORTED && v != 0;
    let (w, h) = if known(a[1]) && known(a[2]) {
        (a[1], a[2])
    } else {
        (4096, 4096)
    };
    let cfg = dev
        .create_config(p, ffi::ENTRYPOINT_VLD, ffi::RT_FORMAT_YUV420)
        .ok()?;
    Some((cfg, w, h))
}

/// Sends one picture's buffers and runs it.
fn submit<P: Pod, I: Pod, S: Pod>(
    d: &Device,
    ctx: ffi::Id,
    surface: ffi::Id,
    pic: &P,
    iq: Option<&I>,
    slices: &[(S, &[u8])],
) -> Result<(), Error> {
    let hw = Error::Hardware;
    let mut bufs = Vec::with_capacity(2 + 2 * slices.len());
    let res = (|| {
        bufs.push(
            d.create_buffer(ctx, ffi::BUF_PICTURE_PARAMETER, pic)
                .map_err(hw)?,
        );
        if let Some(iq) = iq {
            bufs.push(d.create_buffer(ctx, ffi::BUF_IQ_MATRIX, iq).map_err(hw)?);
        }
        for (sp, data) in slices {
            bufs.push(
                d.create_buffer(ctx, ffi::BUF_SLICE_PARAMETER, sp)
                    .map_err(hw)?,
            );
            bufs.push(
                d.create_buffer_bytes(ctx, ffi::BUF_SLICE_DATA, data)
                    .map_err(hw)?,
            );
        }
        d.begin_picture(ctx, surface).map_err(hw)?;
        d.render_picture(ctx, &mut bufs).map_err(hw)?;
        d.end_picture(ctx).map_err(hw)
    })();
    for b in bufs {
        d.destroy_buffer(b);
    }
    res
}

impl VaVideo {
    pub fn open(codec: VideoCodec, config: &[u8]) -> Result<VaVideo, Error> {
        let mut last = Error::Unavailable;
        for node in render_nodes() {
            let dev = match Device::open(&node) {
                Ok(d) => d,
                Err(e) => {
                    last = Error::Hardware(e);
                    continue;
                }
            };
            let (cfg, stream) = match codec {
                VideoCodec::Hevc => {
                    let Some(hw) = HevcHw::open(&dev) else {
                        last = Error::Unsupported("no HEVC Main decoder".into());
                        continue;
                    };
                    let st = match HevcStream::new(config, hw.lim) {
                        Ok(s) => s,
                        Err(e) => {
                            for c in [Some(hw.cfg8), hw.cfg10].into_iter().flatten() {
                                dev.destroy_config(c);
                            }
                            return Err(reject(e));
                        }
                    };
                    let ten = st.bit_depth > 8;
                    let (cfg, other) = if ten {
                        (hw.cfg10, Some(hw.cfg8))
                    } else {
                        (Some(hw.cfg8), hw.cfg10)
                    };
                    if let Some(o) = other {
                        dev.destroy_config(o);
                    }
                    let Some(cfg) = cfg else {
                        return Err(Error::Unsupported("no HEVC Main 10 decoder".into()));
                    };
                    (cfg, Stream::Hevc(st))
                }
                VideoCodec::H264 => {
                    let Some((cfg, w, h)) = h264_config(&dev) else {
                        last = Error::Unsupported("no H.264 decoder".into());
                        continue;
                    };
                    match H264Stream::new(config, w, h) {
                        Ok(s) => (cfg, Stream::H264(s)),
                        Err(e) => {
                            dev.destroy_config(cfg);
                            return Err(reject(e));
                        }
                    }
                }
            };
            let mut v = VaVideo {
                dev,
                cfg,
                pool: None,
                stream,
                out: Output::default(),
            };
            v.make_pool()?;
            return Ok(v);
        }
        Err(last)
    }

    fn make_pool(&mut self) -> Result<(), Error> {
        let st = &mut self.stream;
        let (w, h) = st.coded();
        let ten_bit = st.ten_bit();
        // Every reference, the picture being decoded, and one spare for streams that keep a
        // reference a picture longer than they declare.
        let n = (st.surfaces_needed() + 1).min(18);
        let (rt, code) = if ten_bit {
            (ffi::RT_FORMAT_YUV420_10, fourcc(b"P010"))
        } else {
            (ffi::RT_FORMAT_YUV420, fourcc(b"NV12"))
        };
        let d = &self.dev;
        let mut surfaces = d
            .create_surfaces(rt, w, h, code, n)
            .map_err(Error::Hardware)?;
        let ctx = match d.create_context_pool(self.cfg, w, h, &mut surfaces) {
            Ok(c) => c,
            Err(e) => {
                d.destroy_surfaces(&mut surfaces);
                return Err(Error::Hardware(e));
            }
        };
        let image = match d.create_image(code, w, h) {
            Ok(i) => i,
            Err(e) => {
                d.destroy_context(ctx);
                d.destroy_surfaces(&mut surfaces);
                return Err(Error::Hardware(e));
            }
        };
        st.set_slots(surfaces.len());
        self.pool = Some(HevcPool {
            w,
            h,
            ten_bit,
            surfaces,
            ctx,
            image,
        });
        Ok(())
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
    /// A sample may give no picture (parameter sets only, a leading picture that can't be
    /// decoded after a seek, an HEVC picture that is not output).
    pub fn decode(&mut self, sample: &[u8], tag: i64, show: bool) -> Result<(), Error> {
        let r = self.decode_one(sample, tag, show);
        self.out.settle(&mut self.stream);
        r
    }

    fn decode_one(&mut self, sample: &[u8], tag: i64, show: bool) -> Result<(), Error> {
        let p = self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Hardware("no surface pool".into()))?;
        let d = &self.dev;
        let (slot, res, hevc) = match &mut self.stream {
            Stream::Hevc(st) => {
                let Some(job) = st.prepare(sample, &p.surfaces).map_err(reject)? else {
                    return Ok(());
                };
                let s = p.surfaces[job.slot];
                let r = crate::gate::hevc(
                    &job.pic,
                    job.iq.as_ref(),
                    &job.slices,
                    &p.surfaces,
                    (p.w, p.h),
                )
                .map_err(Error::from)
                .and_then(|()| submit(d, p.ctx, s, &job.pic, job.iq.as_ref(), &job.slices));
                (job.slot, r, Some((job.id, job.output)))
            }
            Stream::H264(st) => {
                // A frame, or both fields of one (each into the frame's surface).
                let acc = st.prepare(sample, &p.surfaces).map_err(reject)?;
                let mut r = Ok(());
                for job in &acc.jobs {
                    let s = p.surfaces[job.slot];
                    let size = (p.w, p.h);
                    r = crate::gate::h264(&job.pic, Some(&job.iq), &job.slices, &p.surfaces, size)
                        .map_err(Error::from)
                        .and_then(|()| submit(d, p.ctx, s, &job.pic, Some(&job.iq), &job.slices));
                    if r.is_err() {
                        break;
                    }
                }
                match (r, acc.show) {
                    (Err(e), _) => (acc.jobs[0].slot, Err(e), None),
                    (Ok(()), Some(slot)) => (slot, Ok(()), None),
                    (Ok(()), None) => return Ok(()),
                }
            }
        };
        let surface = p.surfaces[slot];
        if let Err(e) = res {
            self.stream.forget_last();
            return Err(e);
        }
        if let Err(e) = d.sync_surface_timeout(surface, TIMEOUT_NS) {
            if e.contains("timed out") {
                crate::heic::mark_wedged();
            }
            self.stream.forget_last();
            return Err(Error::Hardware(e));
        }
        if !show || hevc.is_some_and(|(_, output)| !output) {
            // Decoded as a reference only (pic_output_flag 0, or a picture the caller will
            // not show): nothing to read back.
            return Ok(());
        }
        let (x, y, w, h) = self.stream.region();
        d.get_image(surface, p.w, p.h, p.image.image_id)
            .map_err(Error::Hardware)?;
        let img = &p.image;
        let planes = d
            .with_image_data(img, |data| copy_420(img, data, p.ten_bit, (x, y, w, h)))
            .map_err(Error::Hardware)??;
        let frame = VideoFrame {
            width: planes.width,
            height: planes.height,
            bit_depth: planes.bit_depth,
            chroma: crate::video::Chroma::Yuv420,
            planes: planes.planes,
            signal: self.stream.signal(),
        };
        self.out.push(hevc.map(|(id, _)| id), tag, frame);
        Ok(())
    }
}

impl Drop for VaVideo {
    fn drop(&mut self) {
        release_pool(&self.dev, self.pool.take());
        self.dev.destroy_config(self.cfg);
    }
}
