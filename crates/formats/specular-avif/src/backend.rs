//! The AV1 backend: a small internal trait plus its implementation over `re_rav1d`'s safe
//! `dav1d` module (a Rust port of dav1d). Another backend (e.g. the C dav1d) could be added
//! behind a feature by implementing [`Av1Decoder`].

use std::ops::Range;
use std::sync::Arc;

use rav1d::dav1d;

use crate::{Error, MAX_PIXELS};

/// One coded AV1 sample handed to the decoder (shared file bytes or an owned copy).
pub(crate) enum Input {
    Shared(Arc<[u8]>, Range<usize>),
    Owned(Vec<u8>),
}

impl AsRef<[u8]> for Input {
    fn as_ref(&self) -> &[u8] {
        match self {
            Input::Shared(d, r) => d.get(r.clone()).unwrap_or(&[]),
            Input::Owned(v) => v,
        }
    }
}

/// Colour description from the bitstream (CICP code points).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cicp {
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}

/// Borrowed view of a decoded YUV picture.
#[derive(Clone, Copy)]
pub(crate) struct Planes<'a> {
    pub planes: [&'a [u8]; 3],
    /// Strides in bytes.
    pub strides: [usize; 3],
    pub width: u32,
    pub height: u32,
    /// Significant bits per sample (8, 10 or 12).
    pub bits: u8,
    /// Samples are stored as native-endian u16 (otherwise u8).
    pub hbd: bool,
    pub ssx: bool,
    pub ssy: bool,
    /// Subsampled chroma is co-sited with the even luma columns / rows (AV1
    /// `chroma_sample_position` VERTICAL: columns; COLOCATED: both). Otherwise, as for
    /// CSP_UNKNOWN and 4:2:2, it sits midway between them, as libavif assumes.
    pub cosited_x: bool,
    pub cosited_y: bool,
    pub mono: bool,
    pub cicp: Cicp,
}

/// A decoded picture owned by the backend.
pub(crate) trait Picture: Send + Sync {
    fn planes(&self) -> Planes<'_>;
}

/// An AV1 decoder instance. `decode` takes one temporal unit and returns its shown frame.
pub(crate) trait Av1Decoder: Send {
    fn decode(&mut self, data: Input) -> Result<Option<Box<dyn Picture>>, Error>;
    /// Drops all decoder state (before replaying a sequence from its start).
    fn reset(&mut self);
}

/// Which picture of a layered (scalable) AV1 item to output, from its `a1op` and `lsel`
/// properties (AVIF 1.1, 4.3 and 4.4; HEIF `lsel`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Layers {
    /// The operating point (`a1op`; 0 without one).
    pub op: u8,
    /// The spatial layer to output (`lsel` layer_id); `None` (0xFFFF or no `lsel`): the
    /// highest layer of the operating point.
    pub layer: Option<u16>,
}

/// Creates the default backend. `threads` = 0 picks one per CPU.
pub(crate) fn new_decoder(threads: u32) -> Result<Box<dyn Av1Decoder>, Error> {
    new_decoder_layers(threads, Layers::default())
}

/// The default backend decoding a layered item's operating point and layer.
pub(crate) fn new_decoder_layers(
    threads: u32,
    layers: Layers,
) -> Result<Box<dyn Av1Decoder>, Error> {
    Ok(Box::new(Rav1d::new(threads, layers)?))
}

struct Rav1d {
    dec: dav1d::Decoder,
    threads: u32,
    layers: Layers,
}

fn dec_err(e: dav1d::Error) -> Error {
    Error::Decoder(e.to_string())
}

impl Rav1d {
    fn new(threads: u32, layers: Layers) -> Result<Self, Error> {
        let mut s = dav1d::Settings::new();
        s.set_n_threads(threads);
        // One frame in flight: every sample yields its picture right away (no frame threading
        // latency); tile/row threads still parallelise each frame.
        s.set_max_frame_delay(1);
        s.set_frame_size_limit(MAX_PIXELS as u32);
        // Layered (spatially scalable) items: only the highest layer of the operating point,
        // as libavif decodes them. With every layer out, the first picture was the base
        // layer (a 1/4-size image in the corner of a black canvas: Apple's and Xiph's
        // multilayer test files).
        // A lower layer (`lsel`) needs every layer out, then the one asked for is picked.
        s.set_all_layers(layers.layer.is_some());
        s.set_operating_point(u32::from(layers.op.min(31)));
        let dec = dav1d::Decoder::with_settings(&s).map_err(dec_err)?;
        Ok(Rav1d {
            dec,
            threads,
            layers,
        })
    }
}

/// Bound on send/get round trips for one sample (each normally needs one or two).
const MAX_ROUNDS: usize = 4096;

impl Av1Decoder for Rav1d {
    fn decode(&mut self, data: Input) -> Result<Option<Box<dyn Picture>>, Error> {
        // Flush out data left over from a previous sample (send_data must not be called while
        // data is pending).
        let mut rounds = 0;
        loop {
            match self.dec.send_pending_data() {
                Ok(()) => break,
                Err(e) if e.is_again() => {
                    rounds += 1;
                    if rounds > MAX_ROUNDS {
                        return Err(Error::Decoder("decoder stalled".into()));
                    }
                    let _ = self.dec.get_picture();
                }
                Err(e) => return Err(dec_err(e)),
            }
        }
        match self.dec.send_data(data, None, None, None) {
            Ok(()) => {}
            Err(e) if e.is_again() => {}
            Err(e) => return Err(dec_err(e)),
        }
        let mut idle = 0;
        // With every layer out, the temporal unit's pictures come in spatial-layer order: the
        // `lsel` layer is the picture of that index (the last one if there are fewer, i.e.
        // the highest layer the operating point holds).
        let want = self.layers.layer.map_or(0, usize::from);
        let mut seen = 0usize;
        let mut last = None;
        for _ in 0..MAX_ROUNDS {
            match self.dec.get_picture() {
                Ok(p) if seen >= want => {
                    if self.layers.layer.is_some() {
                        // Drop the higher layers' pictures: the next sample (a grid's next
                        // tile) must not get them.
                        for _ in 0..MAX_ROUNDS {
                            if self.dec.get_picture().is_err() {
                                break;
                            }
                        }
                    }
                    return Ok(Some(Box::new(Pic::new(p))));
                }
                Ok(p) => {
                    seen += 1;
                    last = Some(p);
                }
                Err(e) if e.is_again() => match self.dec.send_pending_data() {
                    Ok(()) => {
                        // Nothing left to send: a second get_picture drains; after that
                        // there is no picture for this sample.
                        idle += 1;
                        if idle > 2 {
                            return Ok(last.map(|p| Box::new(Pic::new(p)) as Box<dyn Picture>));
                        }
                    }
                    Err(e) if e.is_again() => {}
                    Err(e) => return Err(dec_err(e)),
                },
                Err(e) => return Err(dec_err(e)),
            }
        }
        Err(Error::Decoder("decoder stalled".into()))
    }

    fn reset(&mut self) {
        // Recreating is the only way to also drop pending input; flush() keeps it.
        if let Ok(r) = Rav1d::new(self.threads, self.layers) {
            *self = r;
        } else {
            self.dec.flush();
        }
    }
}

/// A picture plus its plane handles (each `Plane` keeps the shared picture buffer alive and
/// derefs to its bytes, so slices can borrow from `self`).
struct Pic {
    pic: dav1d::Picture,
    planes: [Option<dav1d::Plane>; 3],
}

impl Pic {
    fn new(pic: dav1d::Picture) -> Self {
        use dav1d::{PixelLayout, PlanarImageComponent as C};
        let mono = pic.pixel_layout() == PixelLayout::I400;
        let planes = [
            Some(pic.plane(C::Y)),
            (!mono).then(|| pic.plane(C::U)),
            (!mono).then(|| pic.plane(C::V)),
        ];
        Pic { pic, planes }
    }
}

impl Picture for Pic {
    fn planes(&self) -> Planes<'_> {
        use dav1d::{PixelLayout, PlanarImageComponent as C};
        let p = &self.pic;
        let layout = p.pixel_layout();
        let mono = layout == PixelLayout::I400;
        let (ssx, ssy) = match layout {
            PixelLayout::I420 => (true, true),
            PixelLayout::I422 => (true, false),
            PixelLayout::I400 | PixelLayout::I444 => (false, false),
        };
        let get = |i: usize| -> &[u8] { self.planes[i].as_deref().unwrap_or(&[]) };
        let (y, u, v) = (get(0), get(1), get(2));
        let bits = p.bits_per_component().map_or(8, |b| b.0 as u8);
        let (cosited_x, cosited_y) = match p.chroma_location() {
            dav1d::pixel::ChromaLocation::Left => (true, false),
            dav1d::pixel::ChromaLocation::TopLeft => (true, true),
            _ => (false, false),
        };
        let cicp = Cicp {
            primaries: p.color_primaries() as u16,
            transfer: p.transfer_characteristic() as u16,
            matrix: p.matrix_coefficients() as u16,
            full_range: p.color_range() == dav1d::pixel::YUVRange::Full,
        };
        Planes {
            planes: [y, u, v],
            strides: [
                p.stride(C::Y) as usize,
                if mono { 0 } else { p.stride(C::U) as usize },
                if mono { 0 } else { p.stride(C::V) as usize },
            ],
            width: p.width(),
            height: p.height(),
            bits,
            hbd: p.bit_depth() > 8,
            ssx,
            ssy,
            cosited_x: cosited_x && ssx,
            cosited_y: cosited_y && ssy,
            mono,
            cicp,
        }
    }
}
