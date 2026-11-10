//! Slides whose pixels come from elsewhere: a reader outside this crate (Zeiss CZI through
//! libCZI in Specular's sandboxed decode host) supplies regions of channels or RGB, this
//! backend gives them the slide's levels, compositing and caches.
//!
//! Fluorescence channels are composited in their colours through their display windows (the
//! file's, else the coarsest level's histogram), as for the other slide formats.

use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;

use crate::composite::{self, Plane};
use crate::{
    AssociatedKind, Channel, Error, Level, Props, ReadOptions, Region, Result, Rgba, Slide,
};

/// Most channels composited.
const MAX_CHANNELS: usize = 16;

/// A reader of regions. Regions are given in the pixels of the downscaled image
/// (`ReadOptions::scale`), already clipped to it; reads must return exactly that size.
pub trait External: Send + Sync {
    /// One channel's values (any sample type as f32, NaN where nothing was recorded).
    fn read_channel(&self, channel: u32, o: &ReadOptions) -> Result<Plane>;
    /// The region of an RGB slide.
    fn read_rgb(&self, o: &ReadOptions) -> Result<Rgba>;
    /// Associated image `i` of [`Slide::associated`].
    fn read_associated(&self, i: usize, cancel: Option<&AtomicBool>) -> Result<Rgba>;
    /// Bytes the reader keeps cached.
    fn memory_bytes(&self) -> usize {
        0
    }
    /// Drop the caches; returns the bytes released.
    fn trim(&self) -> usize {
        0
    }
}

/// What an external slide is.
pub struct Spec {
    pub props: Props,
    pub width: u32,
    pub height: u32,
    /// Stored levels, full resolution first.
    pub levels: Vec<Level>,
    /// Channels (one for RGB slides).
    pub channels: Vec<Channel>,
    pub size_z: u32,
    pub size_t: u32,
    pub associated: Vec<(AssociatedKind, u32, u32)>,
    /// Read as separate channels composited in colour (else as RGB).
    pub multichannel: bool,
}

pub(crate) struct ExternalSlide {
    reader: Box<dyn External>,
    multichannel: bool,
    channels: Vec<Channel>,
    windows: Mutex<Option<Vec<(f64, f64)>>>,
}

impl Slide {
    /// A slide read by `reader`.
    pub fn open_external(spec: Spec, reader: Box<dyn External>) -> Result<Slide> {
        if spec.width == 0 || spec.height == 0 {
            return Err(Error::Malformed("empty slide".into()));
        }
        if spec.multichannel && spec.channels.is_empty() {
            return Err(Error::Malformed("no channels".into()));
        }
        let levels = if spec.levels.is_empty() {
            vec![Level {
                width: spec.width,
                height: spec.height,
                downsample: 1.0,
            }]
        } else {
            spec.levels
        };
        Ok(Slide {
            props: spec.props,
            width: spec.width,
            height: spec.height,
            levels,
            channels: spec.channels.clone(),
            size_z: spec.size_z.max(1),
            size_t: spec.size_t.max(1),
            associated: spec.associated,
            files: Vec::new(),
            backend: crate::Backend::External(Box::new(ExternalSlide {
                reader,
                multichannel: spec.multichannel,
                channels: spec.channels,
                windows: Mutex::new(None),
            })),
        })
    }
}

impl ExternalSlide {
    pub fn multichannel(&self) -> bool {
        self.multichannel
    }

    pub fn memory_bytes(&self) -> usize {
        self.reader.memory_bytes()
    }

    pub fn trim(&self) -> usize {
        self.reader.trim()
    }

    fn windows(&self, slide: &Slide, o: &ReadOptions) -> Result<Vec<(f64, f64)>> {
        if let Some(w) = self.windows.lock().clone() {
            return Ok(w);
        }
        let n = self.channels.len().clamp(1, MAX_CHANNELS);
        let mut scale = 1u32;
        while scale < 1 << 16 && (slide.width / scale > 1024 || slide.height / scale > 1024) {
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
                w: slide.width.div_ceil(scale),
                h: slide.height.div_ceil(scale),
            };
            let p = self.reader.read_channel(
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

    pub fn read(&self, slide: &Slide, o: &ReadOptions) -> Result<Rgba> {
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let img = if self.multichannel {
            let windows = self.windows(slide, o)?;
            let n = self.channels.len().clamp(1, MAX_CHANNELS);
            let mut planes = Vec::with_capacity(n);
            for c in 0..n {
                crate::check_cancel(o.cancel)?;
                let p = self.reader.read_channel(c as u32, o)?;
                check_size(p.width, p.height, r)?;
                planes.push(p);
            }
            composite::composite(&planes, &self.channels, &windows)
        } else {
            self.reader.read_rgb(o)?
        };
        check_size(img.width, img.height, r)?;
        if img.data.len() != img.width as usize * img.height as usize * 4 {
            return Err(Error::Malformed("region of the wrong size".into()));
        }
        Ok(img)
    }

    pub fn read_channel(&self, c: u32, o: &ReadOptions) -> Result<Plane> {
        if !self.multichannel {
            return Err(Error::Unsupported("channels of an RGB slide".into()));
        }
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let p = self.reader.read_channel(c, o)?;
        check_size(p.width, p.height, r)?;
        if p.values.len() != p.width as usize * p.height as usize {
            return Err(Error::Malformed("region of the wrong size".into()));
        }
        Ok(p)
    }

    pub fn read_associated(&self, i: usize, cancel: Option<&AtomicBool>) -> Result<Rgba> {
        let img = self.reader.read_associated(i, cancel)?;
        if img.data.len() != img.width as usize * img.height as usize * 4 {
            return Err(Error::Malformed(
                "associated image of the wrong size".into(),
            ));
        }
        Ok(img)
    }
}

fn check_size(w: u32, h: u32, r: Region) -> Result<()> {
    if (w, h) == (r.w, r.h) {
        Ok(())
    } else {
        Err(Error::Malformed(format!(
            "the reader returned {w}×{h} for a region of {}×{}",
            r.w, r.h
        )))
    }
}

/// Decode an associated image stored as JPEG, PNG or BMP.
pub fn decode_image(bytes: &[u8], cancel: Option<&AtomicBool>) -> Result<Rgba> {
    crate::blob::rgba(bytes, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, Vendor};

    /// A gradient reader: channel c's value at (x, y) of the full image is c·1000 + x + y.
    struct Ramp;

    impl External for Ramp {
        fn read_channel(&self, c: u32, o: &ReadOptions) -> Result<Plane> {
            let r = o.region.unwrap();
            let s = o.scale as f32;
            let mut values = Vec::new();
            for y in 0..r.h {
                for x in 0..r.w {
                    values.push(c as f32 * 1000.0 + (r.x + x) as f32 * s + (r.y + y) as f32 * s);
                }
            }
            Ok(Plane {
                width: r.w,
                height: r.h,
                values,
                truncated: false,
            })
        }
        fn read_rgb(&self, o: &ReadOptions) -> Result<Rgba> {
            let r = o.region.unwrap();
            Ok(Rgba {
                width: r.w,
                height: r.h,
                data: vec![128; r.w as usize * r.h as usize * 4],
                truncated: false,
            })
        }
        fn read_associated(&self, _i: usize, _c: Option<&AtomicBool>) -> Result<Rgba> {
            Ok(Rgba {
                width: 2,
                height: 1,
                data: vec![0; 7],
                truncated: false,
            })
        }
    }

    fn spec(multichannel: bool) -> Spec {
        Spec {
            props: Props::new(Vendor::Zeiss),
            width: 100,
            height: 60,
            levels: Vec::new(),
            channels: vec![
                Channel {
                    name: "A".into(),
                    colour: Some([255, 0, 0]),
                    window: Some((0.0, 200.0)),
                },
                Channel::default(),
            ],
            size_z: 1,
            size_t: 1,
            associated: vec![(AssociatedKind::Label, 2, 1)],
            multichannel,
        }
    }

    #[test]
    fn composites_and_checks_sizes() {
        let s = Slide::open_external(spec(true), Box::new(Ramp)).unwrap();
        assert!(s.multichannel());
        let o = ReadOptions {
            scale: 2,
            region: Some(Region {
                x: 40,
                y: 0,
                w: 20,
                h: 30,
            }),
            position: Position::default(),
            cancel: None,
            parallel: false,
        };
        // Clipped to the 50×30 downscaled image.
        let img = s.read(crate::Files { data: &[] }, &o).unwrap();
        assert_eq!((img.width, img.height), (10, 30));
        let p = s.read_channel(crate::Files { data: &[] }, 1, &o).unwrap();
        assert_eq!(p.values[0], 1000.0 + 80.0);
        // An associated image of the wrong length is refused.
        assert!(
            s.read_associated(crate::Files { data: &[] }, 0, None)
                .is_err()
        );
        let rgb = Slide::open_external(spec(false), Box::new(Ramp)).unwrap();
        assert!(rgb.read_channel(crate::Files { data: &[] }, 0, &o).is_err());
        assert_eq!(
            rgb.read(crate::Files { data: &[] }, &o).unwrap().data[0],
            128
        );
    }
}
