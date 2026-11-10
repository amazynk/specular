//! Frame buffers while decoding: one plane per channel (Y/Co/Cg/alpha after the transforms,
//! plus the frame-lookback plane of animations), full resolution, and the interlacing
//! geometry.
//!
//! Samples are kept as `i32` but stored with the wrap-around of the reference decoder's
//! plane types (8-bit images: unsigned bytes for channels 0 and 3, 16-bit signed for 1 and 2;
//! deeper images: 16-bit unsigned and 32-bit signed), so an out-of-range prediction stored for
//! an invisible pixel reads back the same way.

/// How a plane's samples wrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Store {
    U8,
    I16,
    U16,
    I32,
}

impl Store {
    #[inline]
    pub(crate) fn wrap(self, v: i32) -> i32 {
        match self {
            Store::U8 => i32::from(v as u8),
            Store::I16 => i32::from(v as i16),
            Store::U16 => i32::from(v as u16),
            Store::I32 => v,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Plane {
    pub(crate) data: Vec<i32>,
    /// A constant plane holds no samples and ignores writes.
    pub(crate) constant: Option<i32>,
    pub(crate) store: Store,
}

impl Plane {
    pub(crate) fn constant(v: i32, store: Store) -> Plane {
        Plane {
            data: Vec::new(),
            constant: Some(v),
            store,
        }
    }

    #[inline]
    pub(crate) fn get(&self, i: usize) -> i32 {
        match self.constant {
            Some(v) => v,
            None => self.data[i],
        }
    }

    #[inline]
    pub(crate) fn set(&mut self, i: usize, v: i32) {
        if self.constant.is_none() {
            self.data[i] = self.store.wrap(v);
        }
    }

    /// Turns a constant plane into one with samples (all the constant).
    pub(crate) fn make_real(&mut self, n: usize) {
        if let Some(v) = self.constant.take() {
            self.data = vec![v; n];
        }
    }
}

/// One frame.
#[derive(Clone)]
pub(crate) struct Img {
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) planes: Vec<Plane>,
    /// FrameShape: the decoded columns of each row (frames after the first).
    pub(crate) col_begin: Vec<u32>,
    pub(crate) col_end: Vec<u32>,
    /// DuplicateFrame: the frame this one repeats.
    pub(crate) seen_before: i32,
    /// Set by the palette transforms until they are undone.
    pub(crate) palette: bool,
}

pub(crate) fn rowps(z: usize) -> usize {
    1 << z.div_ceil(2).min(62)
}
pub(crate) fn colps(z: usize) -> usize {
    1 << (z / 2).min(62)
}

impl Img {
    pub(crate) fn rows(&self, z: usize) -> usize {
        1 + (self.h - 1) / rowps(z)
    }
    pub(crate) fn cols(&self, z: usize) -> usize {
        1 + (self.w - 1) / colps(z)
    }
    /// The zoomlevel with a single pixel.
    pub(crate) fn zooms(&self) -> usize {
        let mut z = 0;
        while rowps(z) < self.h || colps(z) < self.w {
            z += 1;
        }
        z
    }
    #[inline]
    pub(crate) fn idx(&self, z: usize, r: usize, c: usize) -> usize {
        r * rowps(z) * self.w + c * colps(z)
    }
    #[inline]
    pub(crate) fn at(&self, p: usize, i: usize) -> i32 {
        self.planes[p].get(i)
    }
    pub(crate) fn num(&self) -> usize {
        self.planes.len()
    }
    pub(crate) fn len(&self) -> usize {
        self.w * self.h
    }

    /// Animations with frame lookback get the missing chroma and alpha planes (constant)
    /// and the lookback plane.
    pub(crate) fn ensure_frame_lookbacks(&mut self, store_small: bool) {
        if self.planes.len() >= 5 {
            return;
        }
        let (s12, s3) = if store_small {
            (Store::I16, Store::U8)
        } else {
            (Store::I32, Store::U16)
        };
        while self.planes.len() < 3 {
            self.planes.push(Plane::constant(0, s12));
        }
        if self.planes.len() < 4 {
            self.planes.push(Plane::constant(1, s3));
        }
        self.planes.push(Plane {
            data: vec![0; self.len()],
            constant: None,
            store: Store::U8,
        });
    }
}
