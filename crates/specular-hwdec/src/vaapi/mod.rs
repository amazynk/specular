//! VA-API on Linux: libva and the Mesa / Intel media driver loaded at run time.
//!
//! JPEG baseline goes through the fixed-function decoder (AMD VCN JPEG, Intel MFX). The decoded
//! surface is read back into Y/Cb/Cr planes, so the result is an ordinary [`Raster`] that the GPU
//! converts like a software decode. HEVC (HEIC tiles) goes through [`hevc`]: the parameter sets
//! and slice headers parsed in Rust into VA buffers, several pictures in flight on a pool of
//! surfaces, each read back with `vaGetImage` into planes. AV1 is probed and reported.

pub(crate) mod enc;
mod enc_headers;
mod ffi;
// The HEVC picture parser, shared with the D3D12 backend.
pub(crate) use crate::hevc;
pub(crate) mod video;

use crate::jpeg::{Baseline, Sampling};
use crate::{Capabilities, CodecCaps, Error, SoftwareOnly};
use ffi::{Device, fourcc};
use specular_core::{PixelLayout, Raster, RasterPlane};

pub struct Va {
    dev: Device,
    jpeg_rt: u32,
    hevc: Option<HevcHw>,
}

/// HEVC Main / Main10 decode configs and the current surface pool.
struct HevcHw {
    cfg8: ffi::Id,
    cfg10: Option<ffi::Id>,
    lim: hevc::Limits,
    pool: Option<HevcPool>,
}

/// A decode context over `surfaces` (all one picture size and format) and one image to read
/// them back through.
struct HevcPool {
    w: u32,
    h: u32,
    ten_bit: bool,
    surfaces: Vec<ffi::Id>,
    ctx: ffi::Id,
    image: ffi::Image,
}

use crate::params::{HevcPlanes, planes_from_420};

fn render_nodes() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("/dev/dri")
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("renderD"))
                .map(|n| format!("/dev/dri/{n}"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

impl Va {
    /// Opens the first render node with a working VA driver.
    pub fn open() -> Result<(Va, Capabilities), String> {
        let mut last = "no DRM render node".to_owned();
        for node in render_nodes() {
            match Device::open(&node) {
                Ok(dev) => {
                    let caps = probe(&dev, &node);
                    let jpeg_rt = if caps.jpeg.is_some() {
                        dev.attributes(
                            ffi::PROFILE_JPEG_BASELINE,
                            ffi::ENTRYPOINT_VLD,
                            &[ffi::ATTRIB_RT_FORMAT],
                        )[0]
                    } else {
                        0
                    };
                    let hevc = HevcHw::open(&dev);
                    return Ok((Va { dev, jpeg_rt, hevc }, caps));
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Decodes a baseline JPEG to Y/Cb/Cr (or grey) planes.
    pub fn decode_jpeg(&self, j: &Baseline, caps: &CodecCaps) -> Result<Raster, Error> {
        let (w, h) = (j.width as u32, j.height as u32);
        if w > caps.max_width || h > caps.max_height {
            return Err(Error::Unsupported(format!(
                "{w}×{h} is above the decoder's {}×{}",
                caps.max_width, caps.max_height
            )));
        }
        let rt = match j.sampling {
            Sampling::Gray => ffi::RT_FORMAT_YUV400,
            Sampling::S420 => ffi::RT_FORMAT_YUV420,
            Sampling::S422 | Sampling::S440 => ffi::RT_FORMAT_YUV422,
            Sampling::S444 => ffi::RT_FORMAT_YUV444,
        };
        if self.jpeg_rt & rt == 0 || j.sampling == Sampling::S440 {
            return Err(Error::Unsupported(format!(
                "{:?} output not offered by the driver",
                j.sampling
            )));
        }
        let d = &self.dev;
        let hw = |e: String| Error::Hardware(e);
        let config = d
            .create_config(ffi::PROFILE_JPEG_BASELINE, ffi::ENTRYPOINT_VLD, rt)
            .map_err(hw)?;
        let res = (|| {
            let surface = d
                .create_surface(rt, w, h, surface_fourcc(j.sampling))
                .map_err(hw)?;
            let res = (|| {
                let ctx = d.create_context(config, w, h, surface).map_err(hw)?;
                let t0 = std::time::Instant::now();
                let res = self
                    .submit(ctx, surface, j)
                    .and_then(|_| d.sync_surface(surface).map_err(hw));
                d.destroy_context(ctx);
                res?;
                let t1 = std::time::Instant::now();
                let r = self.read_back(surface, j);
                specular_log::debug!(target: specular_log::DECODE,
                    "VA-API JPEG {w}×{h}: decode {:.1} ms, read back {:.1} ms",
                    (t1 - t0).as_secs_f64() * 1e3,
                    t1.elapsed().as_secs_f64() * 1e3);
                r
            })();
            d.destroy_surface(surface);
            res
        })();
        d.destroy_config(config);
        res
    }

    /// The HEVC decoder's limits (`None`: no HEVC Main decoder).
    pub fn hevc_limits(&self) -> Option<hevc::Limits> {
        self.hevc.as_ref().map(|h| h.lim)
    }

    /// Makes the surface pool `n` surfaces of `w`×`h` (coded size), 8- or 10-bit, keeping the
    /// current one when it matches.
    pub fn hevc_prepare(&mut self, w: u32, h: u32, ten_bit: bool, n: usize) -> Result<(), Error> {
        let hw = self.hevc.as_mut().ok_or(Error::Unavailable)?;
        if let Some(p) = &hw.pool
            && (p.w, p.h, p.ten_bit) == (w, h, ten_bit)
            && p.surfaces.len() >= n
        {
            return Ok(());
        }
        release_pool(&self.dev, hw.pool.take());
        let (rt, code, cfg) = if ten_bit {
            let cfg = hw
                .cfg10
                .ok_or_else(|| Error::Unsupported("no HEVC Main 10 decoder".into()))?;
            (ffi::RT_FORMAT_YUV420_10, fourcc(b"P010"), cfg)
        } else {
            (ffi::RT_FORMAT_YUV420, fourcc(b"NV12"), hw.cfg8)
        };
        let d = &self.dev;
        let mut surfaces = d
            .create_surfaces(rt, w, h, code, n)
            .map_err(Error::Hardware)?;
        let ctx = match d.create_context_pool(cfg, w, h, &mut surfaces) {
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
        hw.pool = Some(HevcPool {
            w,
            h,
            ten_bit,
            surfaces,
            ctx,
            image,
        });
        Ok(())
    }

    /// Frees the surface pool (the configs stay).
    pub fn hevc_release(&mut self) {
        if let Some(hw) = self.hevc.as_mut() {
            release_pool(&self.dev, hw.pool.take());
        }
    }

    fn hevc_pool(&self) -> Result<&HevcPool, Error> {
        self.hevc
            .as_ref()
            .and_then(|h| h.pool.as_ref())
            .ok_or_else(|| Error::Hardware("no HEVC surface pool".into()))
    }

    /// Queues `pic` for decoding into pool surface `slot` (does not wait for it).
    pub fn hevc_submit(&self, pic: &hevc::Picture<'_>, slot: usize) -> Result<(), Error> {
        let p = self.hevc_pool()?;
        if (pic.coded_w, pic.coded_h, pic.bit_depth > 8) != (p.w, p.h, p.ten_bit) {
            return Err(Error::Hardware(
                "picture does not match the surface pool".into(),
            ));
        }
        let surface = p.surfaces[slot % p.surfaces.len()];
        let mut pp = pic.pic;
        pp.curr_pic.picture_id = surface;
        crate::gate::hevc(&pp, pic.iq.as_ref(), &pic.slices, &p.surfaces, (p.w, p.h))?;
        let d = &self.dev;
        let hw = Error::Hardware;
        let mut bufs = Vec::with_capacity(2 + 2 * pic.slices.len());
        let res = (|| {
            bufs.push(
                d.create_buffer(p.ctx, ffi::BUF_PICTURE_PARAMETER, &pp)
                    .map_err(hw)?,
            );
            if let Some(iq) = &pic.iq {
                bufs.push(d.create_buffer(p.ctx, ffi::BUF_IQ_MATRIX, iq).map_err(hw)?);
            }
            for (sp, data) in &pic.slices {
                bufs.push(
                    d.create_buffer(p.ctx, ffi::BUF_SLICE_PARAMETER, sp)
                        .map_err(hw)?,
                );
                bufs.push(
                    d.create_buffer_bytes(p.ctx, ffi::BUF_SLICE_DATA, data)
                        .map_err(hw)?,
                );
            }
            d.begin_picture(p.ctx, surface).map_err(hw)?;
            d.render_picture(p.ctx, &mut bufs).map_err(hw)?;
            d.end_picture(p.ctx).map_err(hw)
        })();
        for b in bufs {
            d.destroy_buffer(b);
        }
        res
    }

    /// Waits (at most `timeout_ns` where libva can) for pool surface `slot` and reads the
    /// `w`×`h` region at (`x`, `y`) back into planes. `x` and `y` are even.
    pub fn hevc_read(
        &self,
        slot: usize,
        timeout_ns: u64,
        (x, y, w, h): (u32, u32, u32, u32),
    ) -> Result<HevcPlanes, Error> {
        let p = self.hevc_pool()?;
        let surface = p.surfaces[slot % p.surfaces.len()];
        let d = &self.dev;
        d.sync_surface_timeout(surface, timeout_ns)
            .map_err(Error::Hardware)?;
        if x % 2 != 0 || y % 2 != 0 || x + w > p.w || y + h > p.h || w == 0 || h == 0 {
            return Err(Error::Hardware(
                "read-back region outside the surface".into(),
            ));
        }
        d.get_image(surface, p.w, p.h, p.image.image_id)
            .map_err(Error::Hardware)?;
        let img = &p.image;
        d.with_image_data(img, |data| copy_420(img, data, p.ten_bit, (x, y, w, h)))
            .map_err(Error::Hardware)?
    }

    fn submit(&self, ctx: ffi::Id, surface: ffi::Id, j: &Baseline) -> Result<(), Error> {
        let d = &self.dev;
        let hw = |e: String| Error::Hardware(e);
        let mut pic = ffi::JpegPictureParameter {
            picture_width: j.width,
            picture_height: j.height,
            components: [ffi::JpegFrameComponent::default(); 255],
            num_components: j.components.len() as u8,
            color_space: 0,
            rotation: 0,
            reserved: [0; 7],
        };
        for (k, c) in j.components.iter().enumerate() {
            pic.components[k] = ffi::JpegFrameComponent {
                component_id: c.id,
                h_sampling_factor: c.h,
                v_sampling_factor: c.v,
                quantiser_table_selector: c.tq,
            };
        }
        let mut iq = ffi::JpegIqMatrix {
            load_quantiser_table: [0; 4],
            quantiser_table: [[0; 64]; 4],
            reserved: [0; 4],
        };
        for (k, q) in j.quant.iter().enumerate() {
            if let Some(q) = q {
                iq.load_quantiser_table[k] = 1;
                iq.quantiser_table[k] = *q;
            }
        }
        let empty = ffi::JpegHuffmanTable {
            num_dc_codes: [0; 16],
            dc_values: [0; 12],
            num_ac_codes: [0; 16],
            ac_values: [0; 162],
            pad: [0; 2],
        };
        let mut huff = ffi::JpegHuffmanTables {
            load_huffman_table: [0; 2],
            huffman_table: [empty; 2],
            reserved: [0; 4],
        };
        for k in 0..2 {
            if let (Some(dc), Some(ac)) = (&j.dc[k], &j.ac[k]) {
                let t = &mut huff.huffman_table[k];
                huff.load_huffman_table[k] = 1;
                t.num_dc_codes = dc.bits;
                t.dc_values[..dc.values.len()].copy_from_slice(&dc.values);
                t.num_ac_codes = ac.bits;
                t.ac_values[..ac.values.len()].copy_from_slice(&ac.values);
            }
        }
        let mut slice = ffi::JpegSliceParameter {
            slice_data_size: j.data.len() as u32,
            num_components: j.scan.len() as u8,
            restart_interval: j.restart_interval,
            num_mcus: j.num_mcus(),
            ..Default::default()
        };
        for (k, &(cs, td, ta)) in j.scan.iter().enumerate() {
            slice.components[k] = ffi::JpegScanComponent {
                component_selector: cs,
                dc_table_selector: td,
                ac_table_selector: ta,
            };
        }
        let mut bufs = Vec::with_capacity(5);
        let res = (|| {
            bufs.push(
                d.create_buffer(ctx, ffi::BUF_PICTURE_PARAMETER, &pic)
                    .map_err(hw)?,
            );
            bufs.push(d.create_buffer(ctx, ffi::BUF_IQ_MATRIX, &iq).map_err(hw)?);
            bufs.push(
                d.create_buffer(ctx, ffi::BUF_HUFFMAN_TABLE, &huff)
                    .map_err(hw)?,
            );
            bufs.push(
                d.create_buffer(ctx, ffi::BUF_SLICE_PARAMETER, &slice)
                    .map_err(hw)?,
            );
            bufs.push(
                d.create_buffer_bytes(ctx, ffi::BUF_SLICE_DATA, j.data)
                    .map_err(hw)?,
            );
            d.begin_picture(ctx, surface).map_err(hw)?;
            d.render_picture(ctx, &mut bufs).map_err(hw)?;
            d.end_picture(ctx).map_err(hw)
        })();
        for b in bufs {
            d.destroy_buffer(b);
        }
        res
    }

    fn read_back(&self, surface: ffi::Id, j: &Baseline) -> Result<Raster, Error> {
        let d = &self.dev;
        let (w, h) = (j.width as u32, j.height as u32);
        // A copy with vaGetImage, not vaDeriveImage: Mesa reallocates a tiled surface as
        // linear when it is derived, which drops the decoded picture (all-zero planes).
        let img = d
            .create_image(surface_fourcc(j.sampling), w, h)
            .map_err(Error::Hardware)?;
        if let Err(e) = d.get_image(surface, w, h, img.image_id) {
            d.destroy_image(img.image_id);
            return Err(Error::Hardware(e));
        }
        let res = d
            .with_image_data(&img, |data| planes_from_image(&img, data, w, h, j.sampling))
            .map_err(Error::Hardware);
        d.destroy_image(img.image_id);
        res?
    }
}

impl Drop for Va {
    fn drop(&mut self) {
        if let Some(hw) = self.hevc.take() {
            release_pool(&self.dev, hw.pool);
            for c in [Some(hw.cfg8), hw.cfg10].into_iter().flatten() {
                self.dev.destroy_config(c);
            }
        }
    }
}

impl HevcHw {
    /// HEVC Main (and Main 10 where offered) VLD configs for 4:2:0.
    fn open(dev: &Device) -> Option<HevcHw> {
        let profiles = dev.profiles();
        let mut lim = hevc::Limits {
            max_w: 0,
            max_h: 0,
            main10: false,
        };
        let mut cfg = |p: i32, rt: u32| -> Option<ffi::Id> {
            if !profiles.contains(&p) || !dev.entrypoints(p).contains(&ffi::ENTRYPOINT_VLD) {
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
            if a[0] == ffi::ATTRIB_NOT_SUPPORTED || a[0] & rt == 0 {
                return None;
            }
            let known = |v: u32| v != ffi::ATTRIB_NOT_SUPPORTED && v != 0;
            let (w, h) = if known(a[1]) && known(a[2]) {
                (a[1], a[2])
            } else {
                (4096, 4096)
            };
            if lim.max_w == 0 {
                (lim.max_w, lim.max_h) = (w, h);
            } else {
                (lim.max_w, lim.max_h) = (lim.max_w.min(w), lim.max_h.min(h));
            }
            dev.create_config(p, ffi::ENTRYPOINT_VLD, rt).ok()
        };
        let cfg8 = cfg(ffi::PROFILE_HEVC_MAIN, ffi::RT_FORMAT_YUV420)?;
        let cfg10 = cfg(ffi::PROFILE_HEVC_MAIN10, ffi::RT_FORMAT_YUV420_10);
        lim.main10 = cfg10.is_some();
        Some(HevcHw {
            cfg8,
            cfg10,
            lim,
            pool: None,
        })
    }
}

fn release_pool(d: &Device, pool: Option<HevcPool>) {
    if let Some(mut p) = pool {
        d.destroy_image(p.image.image_id);
        d.destroy_context(p.ctx);
        d.destroy_surfaces(&mut p.surfaces);
    }
}

/// Copies the `w`×`h` region at (`x`, `y`) of an NV12 / P010 image into tightly packed Y, Cb,
/// Cr planes (P010 samples are MSB-aligned: shifted down to their 10-bit value).
fn copy_420(
    img: &ffi::Image,
    data: &[u8],
    ten_bit: bool,
    (x, y, w, h): (u32, u32, u32, u32),
) -> Result<HevcPlanes, Error> {
    let expect = if ten_bit {
        fourcc(b"P010")
    } else {
        fourcc(b"NV12")
    };
    if img.format.fourcc != expect || img.num_planes < 2 {
        return Err(Error::Hardware("unexpected read-back image format".into()));
    }
    let (yo, yp) = (img.offsets[0] as usize, img.pitches[0] as usize);
    let (uo, up) = (img.offsets[1] as usize, img.pitches[1] as usize);
    planes_from_420(data, (yo, yp), (uo, up), ten_bit, (x, y, w, h))
}

/// The surface layout the driver decodes each sampling into (Mesa: grey Y800, 4:2:0 NV12,
/// 4:2:2 YUY2, 4:4:4 three planes).
fn surface_fourcc(s: Sampling) -> u32 {
    match s {
        Sampling::Gray => fourcc(b"Y800"),
        Sampling::S420 => fourcc(b"NV12"),
        Sampling::S422 | Sampling::S440 => fourcc(b"YUY2"),
        Sampling::S444 => fourcc(b"444P"),
    }
}

fn plane(data: &[u8], off: u32, pitch: u32, w: u32, h: u32) -> Result<RasterPlane, Error> {
    let (off, pitch, wu, hu) = (off as usize, pitch as usize, w as usize, h as usize);
    if hu > 0 && (pitch < wu || off + pitch * (hu - 1) + wu > data.len()) {
        return Err(Error::Hardware(
            "image plane outside the mapped buffer".into(),
        ));
    }
    let mut out = vec![0u8; wu * hu];
    for (y, row) in out.chunks_exact_mut(wu.max(1)).enumerate().take(hu) {
        row.copy_from_slice(&data[off + y * pitch..off + y * pitch + wu]);
    }
    Ok(RasterPlane {
        width: w,
        height: h,
        stride: wu,
        data: out,
        h_div: 1,
        v_div: 1,
    })
}

/// Separates an interleaved UV plane (NV12) into Cb and Cr.
fn split_uv(
    data: &[u8],
    off: u32,
    pitch: u32,
    cw: u32,
    ch: u32,
) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let (off, pitch, cw, ch) = (off as usize, pitch as usize, cw as usize, ch as usize);
    if ch > 0 && (pitch < 2 * cw || off + pitch * (ch - 1) + 2 * cw > data.len()) {
        return Err(Error::Hardware(
            "chroma plane outside the mapped buffer".into(),
        ));
    }
    let (mut u, mut v) = (vec![0u8; cw * ch], vec![0u8; cw * ch]);
    for y in 0..ch {
        let row = &data[off + y * pitch..off + y * pitch + 2 * cw];
        let (ur, vr) = (&mut u[y * cw..(y + 1) * cw], &mut v[y * cw..(y + 1) * cw]);
        for (x, p) in row.chunks_exact(2).enumerate() {
            ur[x] = p[0];
            vr[x] = p[1];
        }
    }
    Ok((u, v))
}

/// Splits packed 4:2:2 (Y0 U Y1 V) into planes.
fn yuy2(data: &[u8], off: u32, pitch: u32, w: u32, h: u32) -> Result<Raster, Error> {
    let (off, pitch, wu, hu) = (off as usize, pitch as usize, w as usize, h as usize);
    let cw = wu.div_ceil(2);
    if hu > 0 && (pitch < 4 * cw || off + pitch * (hu - 1) + 4 * cw > data.len()) {
        return Err(Error::Hardware("image outside the mapped buffer".into()));
    }
    let (mut y, mut u, mut v) = (vec![0u8; wu * hu], vec![0u8; cw * hu], vec![0u8; cw * hu]);
    for r in 0..hu {
        let row = &data[off + r * pitch..off + r * pitch + 4 * cw];
        let yr = &mut y[r * wu..(r + 1) * wu];
        for (x, q) in row.chunks_exact(4).enumerate() {
            yr[2 * x] = q[0];
            if 2 * x + 1 < wu {
                yr[2 * x + 1] = q[2];
            }
            u[r * cw + x] = q[1];
            v[r * cw + x] = q[3];
        }
    }
    let p = |data: Vec<u8>, pw: u32, hd: u8| RasterPlane {
        width: pw,
        height: h,
        stride: pw as usize,
        data,
        h_div: hd,
        v_div: 1,
    };
    Ok(Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::YCbCr,
        planes: vec![p(y, w, 1), p(u, cw as u32, 2), p(v, cw as u32, 2)],
        truncated: false,
    })
}

fn planes_from_image(
    img: &ffi::Image,
    data: &[u8],
    w: u32,
    h: u32,
    s: Sampling,
) -> Result<Raster, Error> {
    let f = img.format.fourcc;
    let (cw, ch, hd, vd) = match s {
        Sampling::S420 => (w.div_ceil(2), h.div_ceil(2), 2, 2),
        Sampling::S422 => (w.div_ceil(2), h, 2, 1),
        Sampling::S440 => (w, h.div_ceil(2), 1, 2),
        _ => (w, h, 1, 1),
    };
    if f == fourcc(b"YUY2") && s == Sampling::S422 {
        return yuy2(data, img.offsets[0], img.pitches[0], w, h);
    }
    let y = plane(data, img.offsets[0], img.pitches[0], w, h)?;
    if s == Sampling::Gray {
        return Ok(Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Gray,
            planes: vec![y],
            truncated: false,
        });
    }
    let chroma = |data: Vec<u8>| RasterPlane {
        width: cw,
        height: ch,
        stride: cw as usize,
        data,
        h_div: hd,
        v_div: vd,
    };
    let (cb, cr) = if f == fourcc(b"NV12") && s == Sampling::S420 {
        let (u, v) = split_uv(data, img.offsets[1], img.pitches[1], cw, ch)?;
        (chroma(u), chroma(v))
    } else if img.num_planes >= 3
        && (f == fourcc(b"I420")
            || (f == fourcc(b"444P") && s == Sampling::S444)
            || (f == fourcc(b"422H") && s == Sampling::S422)
            || (f == fourcc(b"YV12") && s == Sampling::S420))
    {
        let (a, b) = if f == fourcc(b"YV12") { (2, 1) } else { (1, 2) };
        let u = plane(data, img.offsets[a], img.pitches[a], cw, ch)?;
        let v = plane(data, img.offsets[b], img.pitches[b], cw, ch)?;
        (chroma(u.data), chroma(v.data))
    } else {
        let b = f.to_le_bytes();
        return Err(Error::Unsupported(format!(
            "surface format {} for {s:?}",
            String::from_utf8_lossy(&b)
        )));
    };
    Ok(Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::YCbCr,
        planes: vec![y, cb, cr],
        truncated: false,
    })
}

fn codec(dev: &Device, profiles: &[i32], p: i32) -> Option<CodecCaps> {
    if !profiles.contains(&p) || !dev.entrypoints(p).contains(&ffi::ENTRYPOINT_VLD) {
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
    let lim = |v: u32, d: u32| {
        if v == ffi::ATTRIB_NOT_SUPPORTED || v == 0 {
            d
        } else {
            v
        }
    };
    let rt = lim(a[0], 0);
    let mut formats = Vec::new();
    for (bit, n) in [
        (ffi::RT_FORMAT_YUV400, "4:0:0"),
        (ffi::RT_FORMAT_YUV420, "4:2:0"),
        (ffi::RT_FORMAT_YUV422, "4:2:2"),
        (ffi::RT_FORMAT_YUV444, "4:4:4"),
    ] {
        if rt & bit != 0 {
            formats.push(n);
        }
    }
    if rt & 0x100 != 0 {
        formats.push("10-bit 4:2:0");
    }
    Some(CodecCaps {
        max_width: lim(a[1], 4096),
        max_height: lim(a[2], 4096),
        formats: formats.join(", "),
    })
}

fn probe(dev: &Device, node: &str) -> Capabilities {
    let p = dev.profiles();
    let any = |ps: &[i32]| ps.iter().find_map(|&x| codec(dev, &p, x));
    Capabilities {
        api: "VA-API",
        device: format!(
            "{node}, VA-API {}.{}, {}",
            dev.version.0,
            dev.version.1,
            dev.vendor()
        ),
        jpeg: codec(dev, &p, ffi::PROFILE_JPEG_BASELINE),
        h264: codec(dev, &p, ffi::PROFILE_H264_HIGH),
        hevc: any(&[
            ffi::PROFILE_HEVC_MAIN,
            ffi::PROFILE_HEVC_MAIN10,
            ffi::PROFILE_HEVC_MAIN444,
        ]),
        av1: any(&[ffi::PROFILE_AV1_PROFILE0, ffi::PROFILE_AV1_PROFILE1]),
        h264_encode: enc::caps(dev),
        others: [(ffi::PROFILE_VP9_PROFILE0, "VP9")]
            .into_iter()
            .filter(|(x, _)| codec(dev, &p, *x).is_some())
            .map(|(_, n)| n)
            .collect(),
        software_only: software_only(|x| codec(dev, &p, x).is_some()),
    }
}

/// The H.264 and HEVC stream formats that decode in software here (`has`: the driver decodes
/// this VA profile), as the decoders in `h264.rs` and `hevc.rs` refuse them.
fn software_only(has: impl Fn(i32) -> bool) -> Vec<SoftwareOnly> {
    const NO_PROFILE: &str = "no driver profile";
    let mut v = Vec::new();
    let mut add = |format, reason| v.push(SoftwareOnly { format, reason });
    if has(ffi::PROFILE_H264_HIGH) {
        // 8-bit 4:2:0 surfaces only, even where the driver has High 10 (few do).
        add(
            "H.264 High 10",
            if has(ffi::PROFILE_H264_HIGH10) {
                "not decoded in hardware"
            } else {
                NO_PROFILE
            },
        );
        add("H.264 4:2:2", "no VA-API profile");
    }
    if has(ffi::PROFILE_HEVC_MAIN) && !has(ffi::PROFILE_HEVC_MAIN10) {
        add("HEVC Main 10", NO_PROFILE);
    }
    for (format, profile) in [
        ("HEVC 4:2:2", ffi::PROFILE_HEVC_MAIN422_10),
        ("HEVC 4:4:4", ffi::PROFILE_HEVC_MAIN444),
    ] {
        // Decoded into 4:2:0 surfaces only.
        add(
            format,
            if has(profile) {
                "not decoded in hardware"
            } else {
                NO_PROFILE
            },
        );
    }
    v
}

impl crate::HevcDevice for Va {
    fn hevc_limits(&self) -> Option<hevc::Limits> {
        Va::hevc_limits(self)
    }
    fn hevc_prepare(&mut self, w: u32, h: u32, ten_bit: bool, n: usize) -> Result<(), Error> {
        Va::hevc_prepare(self, w, h, ten_bit, n)
    }
    fn hevc_release(&mut self) {
        Va::hevc_release(self);
    }
    fn hevc_submit(&mut self, pic: &hevc::Picture<'_>, slot: usize) -> Result<(), Error> {
        Va::hevc_submit(self, pic, slot)
    }
    fn hevc_read(
        &mut self,
        slot: usize,
        timeout_ns: u64,
        region: (u32, u32, u32, u32),
    ) -> Result<HevcPlanes, Error> {
        Va::hevc_read(self, slot, timeout_ns, region)
    }
}
