//! The slide backend of stitched pyramids ([`crate::mosaic`]): MIRAX, Ventana BIF and
//! Olympus VSI. RGB slides read as stored; fluorescence channels are composited in their
//! colours through their display windows (the file's, else the coarsest level's histogram),
//! like OME-TIFF.

use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use specular_tiff as tf;

use crate::composite::{self, Plane, box_reduce_rgba};
use crate::mosaic::Mosaic;
use crate::{Channel, Error, Files, ReadOptions, Region, Result, Rgba, Slide};

/// The longest side associated images are decoded at.
const ASSOCIATED_MAX: u32 = 8192;
/// Most channels composited.
const MAX_CHANNELS: usize = 16;

/// Where an associated image is stored.
pub(crate) enum Assoc {
    /// A JPEG, PNG or BMP at a byte range of a file of the set.
    Blob { file: usize, offset: u64, len: u64 },
    /// A main-chain IFD of the opened TIFF file.
    Tiff { ifd: u32 },
    /// A stitched pyramid of its own (a VSI overview), with its full size.
    Stitched(Box<Stitched>, u32, u32),
}

pub(crate) struct Stitched {
    pub mosaic: Mosaic,
    pub multichannel: bool,
    pub channels: Vec<Channel>,
    pub assoc: Vec<Assoc>,
    windows: Mutex<Option<Vec<(f64, f64)>>>,
}

impl Stitched {
    pub fn new(
        mosaic: Mosaic,
        multichannel: bool,
        channels: Vec<Channel>,
        assoc: Vec<Assoc>,
    ) -> Stitched {
        Stitched {
            mosaic,
            multichannel,
            channels,
            assoc,
            windows: Mutex::new(None),
        }
    }

    pub fn memory_bytes(&self) -> usize {
        self.mosaic.memory_bytes()
    }

    pub fn trim(&self) -> usize {
        self.mosaic.trim()
    }

    fn windows(&self, size: (u32, u32), files: Files, o: &ReadOptions) -> Result<Vec<(f64, f64)>> {
        if let Some(w) = self.windows.lock().clone() {
            return Ok(w);
        }
        let n = self.channels.len().clamp(1, MAX_CHANNELS);
        let mut scale = 1u32;
        while scale < 1 << 16 && (size.0 / scale > 1024 || size.1 / scale > 1024) {
            scale *= 2;
        }
        let mut out = Vec::with_capacity(n);
        for c in 0..n {
            if let Some(w) = self.channels.get(c).and_then(|c| c.window) {
                out.push(w);
                continue;
            }
            let r = Region {
                x: 0,
                y: 0,
                w: size.0.div_ceil(scale),
                h: size.1.div_ceil(scale),
            };
            let p = self.mosaic.read_channel(
                files,
                c as u32,
                &ReadOptions {
                    scale,
                    region: Some(r),
                    position: o.position,
                    cancel: o.cancel,
                    parallel: true,
                },
            )?;
            out.push(composite::auto_window(&p.values));
        }
        *self.windows.lock() = Some(out.clone());
        Ok(out)
    }

    fn read_sized(&self, size: (u32, u32), files: Files, o: &ReadOptions) -> Result<Rgba> {
        if !self.multichannel {
            return self.mosaic.read(files, o);
        }
        let windows = self.windows(size, files, o)?;
        let n = self.channels.len().clamp(1, MAX_CHANNELS);
        let mut planes = Vec::with_capacity(n);
        for c in 0..n {
            crate::check_cancel(o.cancel)?;
            planes.push(self.mosaic.read_channel(files, c as u32, o)?);
        }
        Ok(composite::composite(&planes, &self.channels, &windows))
    }

    pub fn read(&self, slide: &Slide, files: Files, o: &ReadOptions) -> Result<Rgba> {
        self.read_sized((slide.width, slide.height), files, o)
    }

    pub fn read_channel(&self, files: Files, c: u32, o: &ReadOptions) -> Result<Plane> {
        if !self.multichannel {
            return Err(Error::Unsupported("channels of an RGB slide".into()));
        }
        self.mosaic.read_channel(files, c, o)
    }

    pub fn read_associated(
        &self,
        files: Files,
        i: usize,
        cancel: Option<&AtomicBool>,
    ) -> Result<Rgba> {
        let a = self
            .assoc
            .get(i)
            .ok_or_else(|| Error::Malformed("no such associated image".into()))?;
        match a {
            Assoc::Blob { file, offset, len } => {
                let data = files.get(*file)?;
                let b = usize::try_from(*offset)
                    .ok()
                    .zip(usize::try_from(*len).ok())
                    .and_then(|(o, n)| data.get(o..o.checked_add(n)?))
                    .ok_or_else(|| {
                        Error::Truncated("associated image past the end of its file".into())
                    })?;
                let img = crate::blob::rgba(b, cancel)?;
                let mut f = 1;
                while img.width / f > ASSOCIATED_MAX || img.height / f > ASSOCIATED_MAX {
                    f *= 2;
                }
                Ok(box_reduce_rgba(&img, f))
            }
            Assoc::Tiff { ifd } => {
                let data = files.get(0)?;
                let d = tf::Directories::read(data)?;
                let page = crate::tiffslide::page_of(&d, *ifd)
                    .ok_or_else(|| Error::Malformed("associated image IFD missing".into()))?;
                page.check_supported()?;
                let (w, h) = (page.width(), page.height());
                let mut scale = 1;
                while w / scale > ASSOCIATED_MAX || h / scale > ASSOCIATED_MAX {
                    scale *= 2;
                }
                let dec = page.decode(
                    data,
                    d.little_endian,
                    &tf::DecodeOptions {
                        scale,
                        cancel,
                        ..Default::default()
                    },
                )?;
                Ok(composite::tiff_rgba(dec))
            }
            Assoc::Stitched(s, w, h) => {
                let mut scale = 1;
                while w / scale > ASSOCIATED_MAX || h / scale > ASSOCIATED_MAX {
                    scale *= 2;
                }
                let o = ReadOptions {
                    scale,
                    region: Some(Region {
                        x: 0,
                        y: 0,
                        w: w.div_ceil(scale),
                        h: h.div_ceil(scale),
                    }),
                    position: Default::default(),
                    cancel,
                    parallel: true,
                };
                s.read_sized((*w, *h), files, &o)
            }
        }
    }
}
