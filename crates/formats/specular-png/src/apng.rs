//! APNG: per-frame decoding and sequential compositing.

use std::sync::atomic::AtomicBool;

use crate::convert::Packed;
use crate::decode::{Canvas, canvas_image, decode_stream, output_layout};
use crate::error::{PngError, Result};
use crate::{
    BlendOp, DecodeOptions, DisposeOp, FrameInfo, Image, Layout, Output, Parallelism, PngInfo,
};

/// Decode frame `index` of [`PngInfo::frames`] on its own: the frame's sub-image
/// (`FrameInfo::width x height`, at `opts.scale`), not composited.
pub fn decode_frame(
    data: &[u8],
    info: &PngInfo,
    index: usize,
    opts: &DecodeOptions<'_>,
) -> Result<Image> {
    let f = info.frames.get(index).ok_or(PngError::NoFrame(index))?;
    let layout = output_layout(info, opts.output);
    decode_stream(
        data,
        info,
        &f.data,
        f.width as usize,
        f.height as usize,
        layout,
        opts,
        if f.from_idat {
            info.idot.as_deref()
        } else {
            None
        },
    )
}

/// A composited animation frame.
#[derive(Clone, Debug)]
pub struct CompositedFrame {
    /// Index into [`PngInfo::frames`].
    pub index: usize,
    /// The whole canvas after this frame was drawn (RGBA8 or RGBA16), at the animator's scale.
    pub image: Image,
    pub delay_num: u16,
    /// 0 means 100.
    pub delay_den: u16,
}

impl CompositedFrame {
    /// Delay in seconds, as stored.
    pub fn delay_secs(&self) -> f64 {
        let den = if self.delay_den == 0 {
            100
        } else {
            self.delay_den
        };
        f64::from(self.delay_num) / f64::from(den)
    }
}

/// Composites APNG frames one at a time. Holds the full-resolution canvas and, when a frame
/// uses "previous" disposal, a copy of the area it covers; never more than one frame's pixels.
pub struct Animator<D> {
    data: D,
    info: PngInfo,
    layout: Layout,
    /// Layout of the canvas and decoded frames.
    work: Layout,
    scale: usize,
    parallelism: Parallelism,
    canvas: Canvas,
    /// Region saved for `DisposeOp::Previous` of the last frame.
    saved: Vec<u8>,
    next: usize,
    /// Disposal still to apply before the next frame (frame index).
    pending: Option<usize>,
    failed: bool,
}

impl<D: AsRef<[u8]>> Animator<D> {
    /// Parse `data` and prepare to composite. `output` selects RGBA8 (`Auto8`, `Rgba8`) or
    /// RGBA16 (`Rgba16`, `Auto16`); `scale` downscales each composited frame (any value >= 1).
    /// Still images give a one-frame animation of the default image.
    pub fn new(data: D, output: Output, scale: u32) -> Result<Animator<D>> {
        let info = crate::chunk::parse(data.as_ref(), false)?;
        Animator::with_info(data, info, output, scale)
    }

    /// Like [`Animator::new`] with already parsed info (from [`crate::read_info`] on `data`).
    pub fn with_info(
        data: D,
        mut info: PngInfo,
        output: Output,
        scale: u32,
    ) -> Result<Animator<D>> {
        if info.frames.is_empty() {
            // A still image: the default image as the only frame.
            info.frames.push(FrameInfo {
                width: info.width,
                height: info.height,
                x: 0,
                y: 0,
                delay_num: 0,
                delay_den: 0,
                dispose: DisposeOp::None,
                blend: BlendOp::Source,
                from_idat: true,
                data: info.idat.clone(),
            });
        }
        let layout = match output {
            Output::Rgba16 | Output::Auto16 => Layout::Rgba16,
            _ => Layout::Rgba8,
        };
        // 16-bit sources are composited at 16 bits (blending at 8 bits loses precision).
        let work = if layout == Layout::Rgba16 || info.bit_depth == 16 {
            Layout::Rgba16
        } else {
            Layout::Rgba8
        };
        let packed = if work == Layout::Rgba16 {
            Packed::Rgba16
        } else {
            Packed::Rgba8
        };
        // The canvas must be plausible for the data the file holds (see `check_plausible`).
        let compressed: usize = info
            .frames
            .iter()
            .flat_map(|f| &f.data)
            .map(|r| r.len())
            .sum();
        crate::decode::check_plausible(
            (info.width as usize).saturating_mul(info.height as usize),
            compressed.saturating_mul(4),
        )?;
        let canvas = Canvas::new(packed, info.width as usize, info.height as usize)?;
        Ok(Animator {
            data,
            info,
            layout,
            work,
            scale: scale.max(1) as usize,
            parallelism: Parallelism::Auto,
            canvas,
            saved: Vec::new(),
            next: 0,
            pending: None,
            failed: false,
        })
    }

    /// Threads used to decode each frame (default `Auto`).
    pub fn set_parallelism(&mut self, p: Parallelism) {
        self.parallelism = p;
    }

    pub fn info(&self) -> &PngInfo {
        &self.info
    }

    pub fn frame_count(&self) -> usize {
        self.info.frames.len()
    }

    /// Index of the frame [`Animator::next_frame`] returns next.
    pub fn position(&self) -> usize {
        self.next
    }

    /// Back to frame 0 (clears the canvas).
    pub fn rewind(&mut self) {
        self.canvas.data.fill(0);
        self.saved.clear();
        self.next = 0;
        self.pending = None;
        self.failed = false;
    }

    /// Composite and return the next frame; `None` after the last one (or after an error).
    pub fn next_frame(&mut self, cancel: Option<&AtomicBool>) -> Option<Result<CompositedFrame>> {
        if self.failed || self.next >= self.info.frames.len() {
            return None;
        }
        let r = self.step(cancel);
        if r.is_err() {
            self.failed = true;
        }
        Some(r)
    }

    fn step(&mut self, cancel: Option<&AtomicBool>) -> Result<CompositedFrame> {
        let i = self.next;
        if let Some(p) = self.pending.take() {
            self.dispose(p);
        }
        let f = self.info.frames[i].clone();
        let opts = DecodeOptions {
            output: if self.work == Layout::Rgba16 {
                Output::Rgba16
            } else {
                Output::Rgba8
            },
            scale: 1,
            parallelism: self.parallelism,
            wavefront: crate::Wavefront::Auto,
            verify_checksums: false,
            cancel,
        };
        let img = decode_stream(
            self.data.as_ref(),
            &self.info,
            &f.data,
            f.width as usize,
            f.height as usize,
            self.work,
            &opts,
            if f.from_idat {
                self.info.idot.as_deref()
            } else {
                None
            },
        )?;
        let mut dispose = f.dispose;
        if i == 0 && dispose == DisposeOp::Previous {
            // Nothing to go back to: the spec treats it as "background".
            dispose = DisposeOp::Background;
        }
        let rect = (
            f.x as usize,
            f.y as usize,
            f.width as usize,
            f.height as usize,
        );
        let bpp = self.canvas.packed.bytes_per_pixel();
        if dispose == DisposeOp::Previous {
            self.saved.clear();
            let stride = self.canvas.stride();
            for y in rect.1..rect.1 + rect.3 {
                let o = y * stride + rect.0 * bpp;
                self.saved
                    .extend_from_slice(&self.canvas.data[o..o + rect.2 * bpp]);
            }
        }
        // The first frame is drawn on a transparent canvas: "over" equals "source" there.
        let blend = if i == 0 { BlendOp::Source } else { f.blend };
        self.draw(&img, rect, blend);
        let (delay_num, delay_den) = (f.delay_num, f.delay_den);
        self.pending = Some(i);
        self.next += 1;
        let mut image = canvas_image(self.layout, &self.canvas, self.scale)?;
        image.truncated = img.truncated;
        let _ = dispose;
        Ok(CompositedFrame {
            index: i,
            image,
            delay_num,
            delay_den,
        })
    }

    fn dispose(&mut self, i: usize) {
        let f = &self.info.frames[i];
        let mut op = f.dispose;
        if i == 0 && op == DisposeOp::Previous {
            op = DisposeOp::Background;
        }
        let bpp = self.canvas.packed.bytes_per_pixel();
        let stride = self.canvas.stride();
        let (x, y0, w, h) = (
            f.x as usize,
            f.y as usize,
            f.width as usize,
            f.height as usize,
        );
        match op {
            DisposeOp::None => {}
            DisposeOp::Background => {
                for y in y0..y0 + h {
                    let o = y * stride + x * bpp;
                    self.canvas.data[o..o + w * bpp].fill(0);
                }
            }
            DisposeOp::Previous => {
                if self.saved.len() == w * h * bpp {
                    for (r, y) in (y0..y0 + h).enumerate() {
                        let o = y * stride + x * bpp;
                        self.canvas.data[o..o + w * bpp]
                            .copy_from_slice(&self.saved[r * w * bpp..(r + 1) * w * bpp]);
                    }
                }
            }
        }
    }

    fn draw(&mut self, img: &Image, (x0, y0, w, h): (usize, usize, usize, usize), blend: BlendOp) {
        let bpp = self.canvas.packed.bytes_per_pixel();
        let stride = self.canvas.stride();
        let p = &img.planes[0];
        for y in 0..h.min(img.height as usize) {
            let src = &p.data[y * p.stride..y * p.stride + w * bpp];
            let o = (y0 + y) * stride + x0 * bpp;
            let dst = &mut self.canvas.data[o..o + w * bpp];
            match (blend, self.work) {
                (BlendOp::Source, _) => dst.copy_from_slice(src),
                (BlendOp::Over, Layout::Rgba16) => {
                    for (d, s) in dst.chunks_exact_mut(8).zip(src.chunks_exact(8)) {
                        over16(d, s);
                    }
                }
                (BlendOp::Over, _) => {
                    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                        over8(d, s);
                    }
                }
            }
        }
    }
}

/// Straight-alpha "over" for RGBA8, rounded.
pub(crate) fn over8(d: &mut [u8], s: &[u8]) {
    let sa = u32::from(s[3]);
    if sa == 255 {
        d.copy_from_slice(s);
        return;
    }
    if sa == 0 {
        return;
    }
    let da = u32::from(d[3]);
    // Alpha scaled by 255.
    let ia = da * (255 - sa);
    let oa = sa * 255 + ia;
    if oa == 0 {
        d.fill(0);
        return;
    }
    for c in 0..3 {
        let v = u32::from(s[c]) * sa * 255 + u32::from(d[c]) * ia;
        d[c] = ((v + oa / 2) / oa) as u8;
    }
    d[3] = ((oa + 127) / 255) as u8;
}

/// Straight-alpha "over" for native-endian RGBA16, rounded.
pub(crate) fn over16(d: &mut [u8], s: &[u8]) {
    let g = |b: &[u8], c: usize| u64::from(u16::from_ne_bytes([b[c * 2], b[c * 2 + 1]]));
    let sa = g(s, 3);
    if sa == 65535 {
        d.copy_from_slice(s);
        return;
    }
    if sa == 0 {
        return;
    }
    let da = g(d, 3);
    let ia = da * (65535 - sa);
    let oa = sa * 65535 + ia;
    let mut out = [0u16; 4];
    for (c, o) in out.iter_mut().take(3).enumerate() {
        let v = g(s, c) * sa * 65535 + g(d, c) * ia;
        *o = ((v + oa / 2) / oa) as u16;
    }
    out[3] = ((oa + 32767) / 65535) as u16;
    for c in 0..4 {
        d[c * 2..c * 2 + 2].copy_from_slice(&out[c].to_ne_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over() {
        let mut d = [0, 0, 255, 255];
        over8(&mut d, &[255, 0, 0, 128]);
        assert_eq!(d, [128, 0, 127, 255]);
        let mut d = [0, 0, 0, 0];
        over8(&mut d, &[10, 20, 30, 40]);
        assert_eq!(d, [10, 20, 30, 40]);
    }
}
