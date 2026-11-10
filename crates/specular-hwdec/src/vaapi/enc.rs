//! H.264 encoding on the VA-API encode engine (AMD VCN, Intel QSV) for the slideshow export.
//!
//! One IDR, then P pictures each referencing the one before (no B pictures, so decode order is
//! presentation order and the MP4 needs no `ctts`), a single slice per picture, constant QP.
//! The driver writes the SPS and PPS itself (no packed headers); the first ones are harvested
//! from a warm-up picture so the `avcC` exists before the first real frame, and every later
//! IDR must repeat them byte for byte, or the encode fails (and the caller falls back).
//!
//! The input arrives as planar 4:2:0, is interleaved into an NV12 `VAImage` (padded to whole
//! macroblocks by repeating the last column and row) and copied to the input surface with
//! `vaPutImage`; `vaDeriveImage` is avoided as for decoding (Mesa reallocates tiled surfaces).

use super::enc_headers::Stream;
use super::ffi::{self, ConfigAttrib, Device, PictureH264, fourcc};
use super::render_nodes;
use crate::encode::{EncodeConfig, EncodeProfile, collect_avcc};
use crate::{CodecCaps, Error};

/// The longest a single picture may take before the device is considered wedged.
const TIMEOUT_NS: u64 = 2_000_000_000;

/// The profile and entrypoint this device encodes H.264 with: High where offered, else Main;
/// the full encode entrypoint where offered, else the low-power one. Also the size limits.
fn pick(dev: &Device) -> Option<(EncodeProfile, i32, CodecCaps)> {
    let profiles = dev.profiles();
    for (profile, va) in [
        (EncodeProfile::High, ffi::PROFILE_H264_HIGH),
        (EncodeProfile::Main, ffi::PROFILE_H264_MAIN),
    ] {
        if !profiles.contains(&va) {
            continue;
        }
        let entries = dev.entrypoints(va);
        for entry in [ffi::ENTRYPOINT_ENC_SLICE, ffi::ENTRYPOINT_ENC_SLICE_LP] {
            if !entries.contains(&entry) {
                continue;
            }
            let a = dev.attributes(
                va,
                entry,
                &[
                    ffi::ATTRIB_RT_FORMAT,
                    ffi::ATTRIB_RATE_CONTROL,
                    ffi::ATTRIB_MAX_PICTURE_WIDTH,
                    ffi::ATTRIB_MAX_PICTURE_HEIGHT,
                    ffi::ATTRIB_ENC_MAX_REF_FRAMES,
                ],
            );
            let ok = |v: u32| v != ffi::ATTRIB_NOT_SUPPORTED;
            if !ok(a[0]) || a[0] & ffi::RT_FORMAT_YUV420 == 0 {
                continue;
            }
            if !ok(a[1]) || a[1] & ffi::RC_CQP == 0 {
                continue;
            }
            // Low 16 bits: references in list 0; P pictures need one.
            if ok(a[4]) && a[4] & 0xffff == 0 {
                continue;
            }
            let lim = |v: u32| if ok(v) && v != 0 { v } else { 4096 };
            let caps = CodecCaps {
                max_width: lim(a[2]),
                max_height: lim(a[3]),
                formats: format!("4:2:0, {profile:?}"),
            };
            return Some((profile, entry, caps));
        }
    }
    None
}

/// The encoder this device offers, for the capability report.
pub fn caps(dev: &Device) -> Option<CodecCaps> {
    pick(dev).map(|(_, _, c)| c)
}

pub struct VaEnc {
    dev: Device,
    pub profile: EncodeProfile,
    pub device: String,
    config: ffi::Id,
    ctx: ffi::Id,
    input: ffi::Id,
    recon: [ffi::Id; 2],
    coded: ffi::Id,
    image: ffi::Image,
    cfg: EncodeConfig,
    mb_w: u32,
    mb_h: u32,
    level_idc: u8,
    log2_frame_num: u32,
    log2_poc: u32,
    /// Pictures since the last IDR (0: the next one is an IDR).
    since_idr: u32,
    idr_count: u32,
    /// Index into `recon` of the picture the next P references.
    cur: usize,
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
    coded_bytes: Vec<u8>,
    /// The driver takes packed SPS/PPS and slice headers.
    packed: bool,
}

impl Drop for VaEnc {
    fn drop(&mut self) {
        let d = &self.dev;
        d.destroy_buffer(self.coded);
        d.destroy_image(self.image.image_id);
        d.destroy_context(self.ctx);
        let mut s = [self.input, self.recon[0], self.recon[1]];
        d.destroy_surfaces(&mut s);
        d.destroy_config(self.config);
    }
}

/// `ceil(log2(n))`, clamped to the 4..=16 that `log2_max_*_minus4` can signal.
fn bits_for(n: u32) -> u32 {
    (32 - n.max(1).saturating_sub(1).leading_zeros()).clamp(4, 16)
}

/// (level_idc, MaxMBPS, MaxFS) of H.264 Table A-1, from 3.0 up.
const LEVELS: [(u8, u64, u64); 10] = [
    (30, 40_500, 1_620),
    (31, 108_000, 3_600),
    (32, 216_000, 5_120),
    (40, 245_760, 8_192),
    (41, 245_760, 8_192),
    (42, 522_240, 8_704),
    (50, 589_824, 22_080),
    (51, 983_040, 36_864),
    (52, 2_073_600, 36_864),
    (60, 4_177_920, 139_264),
];

fn level(mb_w: u32, mb_h: u32, fps_num: u32, fps_den: u32) -> u8 {
    let fs = u64::from(mb_w) * u64::from(mb_h);
    let mbps = (fs * u64::from(fps_num)).div_ceil(u64::from(fps_den.max(1)));
    for &(idc, max_mbps, max_fs) in &LEVELS {
        let dim = ((max_fs * 8) as f64).sqrt() as u64;
        if mbps <= max_mbps && fs <= max_fs && u64::from(mb_w) <= dim && u64::from(mb_h) <= dim {
            // 1080p and up get at least 4.1, whose bit-rate ceiling leaves room for CQP peaks.
            return if fs >= 8_160 && idc < 41 { 41 } else { idc };
        }
    }
    62
}

impl VaEnc {
    /// Opens the first render node that encodes H.264 and prepares an encoder for `cfg`; the
    /// SPS and PPS are known when this returns.
    pub fn open(cfg: &EncodeConfig) -> Result<VaEnc, Error> {
        let mut last = Error::Unavailable;
        for node in render_nodes() {
            let dev = match Device::open(&node) {
                Ok(d) => d,
                Err(e) => {
                    last = Error::Hardware(e);
                    continue;
                }
            };
            let Some((profile, entry, caps)) = pick(&dev) else {
                last = Error::Unsupported("no H.264 encoder".into());
                continue;
            };
            if cfg.width > caps.max_width || cfg.height > caps.max_height {
                last = Error::Unsupported(format!(
                    "{}×{} is above the encoder's {}×{}",
                    cfg.width, cfg.height, caps.max_width, caps.max_height
                ));
                continue;
            }
            let device = format!("{node}, {}", dev.vendor());
            let mut enc = VaEnc::with_device(dev, profile, entry, cfg, device)?;
            enc.warm_up()?;
            return Ok(enc);
        }
        Err(last)
    }

    fn with_device(
        dev: Device,
        profile: EncodeProfile,
        entry: i32,
        cfg: &EncodeConfig,
        device: String,
    ) -> Result<VaEnc, Error> {
        let hw = Error::Hardware;
        let (w, h) = (cfg.width, cfg.height);
        if w == 0 || h == 0 || w % 2 != 0 || h % 2 != 0 || cfg.fps_num == 0 || cfg.fps_den == 0 {
            return Err(Error::Unsupported("size or frame rate".into()));
        }
        let (mb_w, mb_h) = (w.div_ceil(16), h.div_ceil(16));
        let (aw, ah) = (mb_w * 16, mb_h * 16);
        let va_profile = match profile {
            EncodeProfile::High => ffi::PROFILE_H264_HIGH,
            EncodeProfile::Main => ffi::PROFILE_H264_MAIN,
        };
        // Mesa writes the NAL headers and parameter sets only from packed headers; drivers
        // without them write their own.
        let offered = dev.attributes(va_profile, entry, &[ffi::ATTRIB_ENC_PACKED_HEADERS])[0];
        let want = ffi::ENC_PACKED_HEADER_SEQUENCE | ffi::ENC_PACKED_HEADER_SLICE;
        let packed = offered != ffi::ATTRIB_NOT_SUPPORTED && offered & want == want;
        let attribs = [
            ConfigAttrib {
                kind: ffi::ATTRIB_RT_FORMAT,
                value: ffi::RT_FORMAT_YUV420,
            },
            ConfigAttrib {
                kind: ffi::ATTRIB_RATE_CONTROL,
                value: ffi::RC_CQP,
            },
            ConfigAttrib {
                kind: ffi::ATTRIB_ENC_PACKED_HEADERS,
                value: if packed {
                    want
                } else {
                    ffi::ENC_PACKED_HEADER_NONE
                },
            },
        ];
        let config = dev
            .create_config_with(va_profile, entry, &attribs)
            .map_err(hw)?;
        let nv12 = fourcc(b"NV12");
        let mut surfaces = match dev.create_surfaces(ffi::RT_FORMAT_YUV420, aw, ah, nv12, 3) {
            Ok(s) => s,
            Err(e) => {
                dev.destroy_config(config);
                return Err(hw(e));
            }
        };
        let (input, recon) = (surfaces[0], [surfaces[1], surfaces[2]]);
        let ctx = match dev.create_context_pool(config, aw, ah, &mut surfaces[1..]) {
            Ok(c) => c,
            Err(e) => {
                dev.destroy_surfaces(&mut surfaces);
                dev.destroy_config(config);
                return Err(hw(e));
            }
        };
        let cleanup = |dev: &Device, surfaces: &mut [ffi::Id]| {
            dev.destroy_context(ctx);
            dev.destroy_surfaces(surfaces);
            dev.destroy_config(config);
        };
        let image = match dev.create_image(nv12, aw, ah) {
            Ok(i) => i,
            Err(e) => {
                cleanup(&dev, &mut surfaces);
                return Err(hw(e));
            }
        };
        // Room for an intra picture at a very low QP; the driver reports overflow as an error.
        let coded_size = (aw * ah * 3 / 2).max(1 << 20);
        let coded = match dev.create_output_buffer(ctx, ffi::BUF_ENC_CODED, coded_size) {
            Ok(b) => b,
            Err(e) => {
                dev.destroy_image(image.image_id);
                cleanup(&dev, &mut surfaces);
                return Err(hw(e));
            }
        };
        let keyint = cfg.keyint.max(1);
        Ok(VaEnc {
            dev,
            profile,
            device,
            config,
            ctx,
            input,
            recon,
            coded,
            image,
            cfg: cfg.clone(),
            mb_w,
            mb_h,
            level_idc: level(mb_w, mb_h, cfg.fps_num, cfg.fps_den),
            log2_frame_num: bits_for(keyint),
            log2_poc: bits_for(keyint * 2),
            since_idr: 0,
            idr_count: 0,
            cur: 0,
            sps: Vec::new(),
            pps: Vec::new(),
            coded_bytes: Vec::new(),
            packed,
        })
    }

    /// Encodes one mid-grey IDR to learn the driver's SPS and PPS, then starts over.
    fn warm_up(&mut self) -> Result<(), Error> {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        let y = vec![128u8; w * h];
        let c = vec![128u8; cw * ch];
        self.encode(&y, &c, &c, w, cw, true, self.cfg.qp)?;
        if self.sps.is_empty() || self.pps.is_empty() {
            return Err(Error::Unsupported(
                "the driver writes no SPS/PPS without packed headers".into(),
            ));
        }
        self.since_idr = 0;
        self.idr_count = 0;
        Ok(())
    }

    /// Copies planar 4:2:0 into the NV12 image, repeating the last column and row out to whole
    /// macroblocks, and the image into the input surface.
    fn upload(
        &mut self,
        y: &[u8],
        u: &[u8],
        v: &[u8],
        y_stride: usize,
        c_stride: usize,
    ) -> Result<(), Error> {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let (aw, ah) = (self.mb_w as usize * 16, self.mb_h as usize * 16);
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        if y.len() < y_stride * (h - 1) + w
            || u.len() < c_stride * (ch - 1) + cw
            || v.len() < c_stride * (ch - 1) + cw
        {
            return Err(Error::Malformed("planes too small for the picture".into()));
        }
        let img = self.image;
        let (yo, yp) = (img.offsets[0] as usize, img.pitches[0] as usize);
        let (co, cp) = (img.offsets[1] as usize, img.pitches[1] as usize);
        if yp < aw || cp < aw {
            return Err(Error::Hardware("NV12 image pitch below its width".into()));
        }
        self.dev
            .with_image_data_mut(&img, |data| {
                if data.len() < yo + yp * (ah - 1) + aw || data.len() < co + cp * (ah / 2 - 1) + aw
                {
                    return Err(Error::Hardware("NV12 image smaller than its planes".into()));
                }
                for r in 0..ah {
                    let src = &y[r.min(h - 1) * y_stride..][..w];
                    let dst = &mut data[yo + r * yp..][..aw];
                    dst[..w].copy_from_slice(src);
                    let last = src[w - 1];
                    dst[w..].fill(last);
                }
                for r in 0..ah / 2 {
                    let sr = r.min(ch - 1) * c_stride;
                    let (su, sv) = (&u[sr..][..cw], &v[sr..][..cw]);
                    let dst = &mut data[co + r * cp..][..aw];
                    for (i, pair) in dst.chunks_exact_mut(2).enumerate() {
                        let x = i.min(cw - 1);
                        pair[0] = su[x];
                        pair[1] = sv[x];
                    }
                }
                Ok(())
            })
            .map_err(Error::Hardware)??;
        let (aw, ah) = (aw as u32, ah as u32);
        self.dev
            .put_image(self.input, img.image_id, aw, ah)
            .map_err(Error::Hardware)
    }

    pub fn qp(&self) -> u8 {
        self.cfg.qp
    }

    fn stream(&self) -> Stream {
        Stream {
            profile: self.profile,
            level_idc: self.level_idc,
            mb_w: self.mb_w,
            mb_h: self.mb_h,
            width: self.cfg.width,
            height: self.cfg.height,
            fps_num: self.cfg.fps_num,
            fps_den: self.cfg.fps_den,
            log2_frame_num: self.log2_frame_num,
            log2_poc: self.log2_poc,
            init_qp: self.cfg.qp,
        }
    }

    fn seq_params(&self) -> ffi::EncSeqH264 {
        let c = &self.cfg;
        let (aw, ah) = (self.mb_w * 16, self.mb_h * 16);
        let crop = aw != c.width || ah != c.height;
        let seq_fields = 1 // chroma_format_idc 4:2:0
            | 1 << 2 // frame_mbs_only_flag
            | 1 << 5 // direct_8x8_inference_flag
            | (self.log2_frame_num - 4) << 6
            | (self.log2_poc - 4) << 12; // pic_order_cnt_type 0 (bits 10–11)
        let vui_fields = 1 << 1 // timing_info_present_flag
            | 1 << 2 // bitstream_restriction_flag
            | 15 << 3 // log2_max_mv_length_horizontal
            | 15 << 8 // log2_max_mv_length_vertical
            | 1 << 13 // fixed_frame_rate_flag
            | 1 << 15; // motion_vectors_over_pic_boundaries_flag
        ffi::EncSeqH264 {
            seq_parameter_set_id: 0,
            level_idc: self.level_idc,
            intra_period: c.keyint.max(1),
            intra_idr_period: c.keyint.max(1),
            ip_period: 1,
            bits_per_second: 0,
            max_num_ref_frames: 1,
            picture_width_in_mbs: self.mb_w as u16,
            picture_height_in_mbs: self.mb_h as u16,
            seq_fields,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            num_ref_frames_in_pic_order_cnt_cycle: 0,
            offset_for_non_ref_pic: 0,
            offset_for_top_to_bottom_field: 0,
            offset_for_ref_frame: [0; 256],
            frame_cropping_flag: u8::from(crop),
            frame_crop_left_offset: 0,
            frame_crop_right_offset: (aw - c.width) / 2,
            frame_crop_top_offset: 0,
            frame_crop_bottom_offset: (ah - c.height) / 2,
            vui_parameters_present_flag: 1,
            vui_fields,
            aspect_ratio_idc: 0,
            sar_width: 0,
            sar_height: 0,
            num_units_in_tick: c.fps_den,
            time_scale: c.fps_num * 2,
            reserved: [0; 4],
        }
    }

    /// `VAEncMiscParameterBuffer` with a `VAEncMiscParameterFrameRate`.
    fn frame_rate_param(&self) -> Vec<u8> {
        let rate = if self.cfg.fps_den == 1 {
            self.cfg.fps_num
        } else {
            (self.cfg.fps_den & 0xffff) << 16 | (self.cfg.fps_num & 0xffff)
        };
        let mut b = Vec::with_capacity(28);
        b.extend_from_slice(&ffi::ENC_MISC_FRAME_RATE.to_ne_bytes());
        b.extend_from_slice(&rate.to_ne_bytes());
        b.extend_from_slice(&[0; 20]); // flags, reserved[4]
        b
    }

    /// Encodes one picture into AVCC (4-byte length) NAL units at quantiser `qp` (10..=51; the
    /// slice header's delta from the PPS's initial QP).
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
        if idr {
            self.since_idr = 0;
        }
        self.upload(y, u, v, y_stride, c_stride)?;
        let frame_num = self.since_idr & ((1 << self.log2_frame_num) - 1);
        let poc = (self.since_idr * 2) & ((1 << self.log2_poc) - 1);
        let recon = if idr { 0 } else { 1 - self.cur };
        let curr = PictureH264 {
            picture_id: self.recon[recon],
            frame_idx: frame_num,
            flags: 0,
            top_field_order_cnt: (self.since_idr * 2) as i32,
            bottom_field_order_cnt: (self.since_idr * 2) as i32,
            reserved: [0; 4],
        };
        let reference = (!idr).then(|| PictureH264 {
            picture_id: self.recon[self.cur],
            frame_idx: (self.since_idr - 1) & ((1 << self.log2_frame_num) - 1),
            flags: ffi::PICTURE_H264_SHORT_TERM_REFERENCE,
            top_field_order_cnt: (self.since_idr as i32 - 1) * 2,
            bottom_field_order_cnt: (self.since_idr as i32 - 1) * 2,
            reserved: [0; 4],
        });
        let high = self.profile == EncodeProfile::High;
        let mut pic = ffi::EncPicH264 {
            curr_pic: curr,
            reference_frames: [PictureH264::INVALID; 16],
            coded_buf: self.coded,
            pic_parameter_set_id: 0,
            seq_parameter_set_id: 0,
            last_picture: 0,
            frame_num: frame_num as u16,
            pic_init_qp: self.cfg.qp,
            num_ref_idx_l0_active_minus1: 0,
            num_ref_idx_l1_active_minus1: 0,
            chroma_qp_index_offset: 0,
            second_chroma_qp_index_offset: 0,
            pic_fields: u32::from(idr) // idr_pic_flag
                | 1 << 1 // reference_pic_flag
                | 1 << 3 // entropy_coding_mode_flag: CABAC
                | u32::from(high) << 8 // transform_8x8_mode_flag
                | 1 << 9, // deblocking_filter_control_present_flag
            reserved: [0; 4],
        };
        let mut slice = ffi::EncSliceH264 {
            macroblock_address: 0,
            num_macroblocks: self.mb_w * self.mb_h,
            macroblock_info: ffi::INVALID_ID,
            slice_type: if idr { 2 } else { 0 },
            pic_parameter_set_id: 0,
            idr_pic_id: (self.idr_count & 0xffff) as u16,
            pic_order_cnt_lsb: poc as u16,
            delta_pic_order_cnt_bottom: 0,
            delta_pic_order_cnt: [0; 2],
            direct_spatial_mv_pred_flag: 0,
            num_ref_idx_active_override_flag: 0,
            num_ref_idx_l0_active_minus1: 0,
            num_ref_idx_l1_active_minus1: 0,
            ref_pic_list0: [PictureH264::INVALID; 32],
            ref_pic_list1: [PictureH264::INVALID; 32],
            luma_log2_weight_denom: 0,
            chroma_log2_weight_denom: 0,
            luma_weight_l0_flag: 0,
            luma_weight_l0: [0; 32],
            luma_offset_l0: [0; 32],
            chroma_weight_l0_flag: 0,
            chroma_weight_l0: [[0; 2]; 32],
            chroma_offset_l0: [[0; 2]; 32],
            luma_weight_l1_flag: 0,
            luma_weight_l1: [0; 32],
            luma_offset_l1: [0; 32],
            chroma_weight_l1_flag: 0,
            chroma_weight_l1: [[0; 2]; 32],
            chroma_offset_l1: [[0; 2]; 32],
            cabac_init_idc: 0,
            slice_qp_delta: (i16::from(qp.clamp(10, 51)) - i16::from(self.cfg.qp)) as i8,
            disable_deblocking_filter_idc: 0,
            slice_alpha_c0_offset_div2: 0,
            slice_beta_offset_div2: 0,
            reserved: [0; 4],
        };
        if let Some(r) = reference {
            pic.reference_frames[0] = r;
            slice.ref_pic_list0[0] = r;
        }
        let d = &self.dev;
        let hw = Error::Hardware;
        let hdr = self.stream();
        let mut bufs = Vec::with_capacity(8);
        let mut coded = std::mem::take(&mut self.coded_bytes);
        coded.clear();
        let res = (|| {
            if idr {
                bufs.push(
                    d.create_buffer(
                        self.ctx,
                        ffi::BUF_ENC_SEQUENCE_PARAMETER,
                        &self.seq_params(),
                    )
                    .map_err(hw)?,
                );
                bufs.push(
                    d.create_buffer_bytes(
                        self.ctx,
                        ffi::BUF_ENC_MISC_PARAMETER,
                        &self.frame_rate_param(),
                    )
                    .map_err(hw)?,
                );
            }
            bufs.push(
                d.create_buffer(self.ctx, ffi::BUF_ENC_PICTURE_PARAMETER, &pic)
                    .map_err(hw)?,
            );
            if self.packed && idr {
                let (data, bits) = hdr.parameter_sets();
                bufs.extend(packed(
                    d,
                    self.ctx,
                    ffi::PACKED_HEADER_SEQUENCE,
                    &data,
                    bits,
                )?);
            }
            if self.packed {
                let (data, bits) = hdr.slice_header(
                    idr,
                    frame_num,
                    u32::from(slice.idr_pic_id),
                    poc,
                    i32::from(slice.slice_qp_delta),
                );
                bufs.extend(packed(d, self.ctx, ffi::PACKED_HEADER_SLICE, &data, bits)?);
            }
            bufs.push(
                d.create_buffer(self.ctx, ffi::BUF_ENC_SLICE_PARAMETER, &slice)
                    .map_err(hw)?,
            );
            d.begin_picture(self.ctx, self.input).map_err(hw)?;
            let r = d.render_picture(self.ctx, &mut bufs);
            // A begun picture is always ended, even when a buffer was refused.
            let e = d.end_picture(self.ctx);
            r.map_err(hw)?;
            e.map_err(hw)?;
            d.sync_surface_timeout(self.input, TIMEOUT_NS).map_err(hw)?;
            d.read_coded(self.coded, &mut coded).map_err(hw)
        })();
        for b in bufs {
            d.destroy_buffer(b);
        }
        self.coded_bytes = coded;
        res?;
        let out = self.take_avcc(idr)?;
        self.cur = recon;
        self.since_idr += 1;
        if idr {
            self.idr_count += 1;
        }
        Ok((out, idr))
    }

    /// The coded picture as AVCC NAL units: access unit delimiters and filler dropped, the SPS
    /// and PPS taken out (the first ones kept for the `avcC`, later ones must match them).
    fn take_avcc(&mut self, idr: bool) -> Result<Vec<u8>, Error> {
        let (out, key) = collect_avcc(&self.coded_bytes, &mut self.sps, &mut self.pps)?;
        if key != idr {
            return Err(Error::Hardware("unexpected slice type in output".into()));
        }
        Ok(out)
    }
}

/// A packed header's parameter and data buffers.
fn packed(
    d: &Device,
    ctx: ffi::Id,
    kind: u32,
    data: &[u8],
    bits: u32,
) -> Result<[ffi::Id; 2], Error> {
    // VAEncPackedHeaderParameterBuffer: type, bit_length, has_emulation_bytes, reserved[4].
    let mut p = Vec::with_capacity(28);
    p.extend_from_slice(&kind.to_ne_bytes());
    p.extend_from_slice(&bits.to_ne_bytes());
    p.extend_from_slice(&[1, 0, 0, 0]);
    p.extend_from_slice(&[0; 16]);
    let a = d
        .create_buffer_bytes(ctx, ffi::BUF_ENC_PACKED_HEADER_PARAMETER, &p)
        .map_err(Error::Hardware)?;
    match d.create_buffer_bytes(ctx, ffi::BUF_ENC_PACKED_HEADER_DATA, data) {
        Ok(b) => Ok([a, b]),
        Err(e) => {
            d.destroy_buffer(a);
            Err(Error::Hardware(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_and_bits() {
        assert_eq!(level(120, 68, 30, 1), 41);
        assert_eq!(level(240, 135, 30, 1), 51);
        assert_eq!(level(80, 45, 30, 1), 31);
        assert_eq!(bits_for(150), 8);
        assert_eq!(bits_for(1), 4);
        assert_eq!(bits_for(300), 9);
    }
}
