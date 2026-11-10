//! Decoded pictures shared between decoding threads.
//!
//! A picture is stored as its two fields: each field is a column of units of 8 field lines
//! (16 frame lines hold one unit of each parity). Units are published once final (fully
//! deblocked): a frame publishes both parities one macroblock row at a time, a field picture
//! publishes only its own parity. Pictures decoding in parallel read their references through
//! a [`View`] (the frame, or one field), which waits for the units it needs, so a later
//! picture can start motion compensation from the top of a reference that is still being
//! decoded, and the second field of a frame can start while the first is still decoding.
//!
//! A field that never gets its opposite field (a lone field) is shown with each of its lines
//! repeated: reads of the missing parity are redirected to the present one.

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

use crate::pixel::{Format, Pix, Samples};

/// How a picture was coded (Table 8-7, PicCodingStruct).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    /// A frame without macroblock-adaptive frame/field coding.
    Frame,
    /// A frame with macroblock-adaptive frame/field coding (MBAFF).
    Mbaff,
    /// One or two coded fields.
    Field,
}

/// Motion of one macroblock as needed when it is the co-located macroblock of a direct
/// prediction (8.4.1.2.1).
#[derive(Clone, Copy, Default)]
pub struct MbMotion {
    pub intra: bool,
    /// The macroblock was field coded (a field picture or a field macroblock pair).
    pub field: bool,
    /// mvCol per 4x4 block (raster) and refIdxCol per 8x8, from list 0 where the
    /// partition used it, else list 1.
    pub mv: [[i16; 2]; 16],
    pub ref_idx: [i8; 4],
    /// Picture referenced by each 8x8 partition (`RefPic::id`), 0 if none.
    pub ref_id: [u32; 4],
}

pub const INTRA_MOTION: MbMotion = MbMotion {
    intra: true,
    field: false,
    mv: [[0; 2]; 16],
    ref_idx: [-1; 4],
    ref_id: [0; 4],
};

/// 8 lines of one field: luma, the chroma lines that go with them (4 for 4:2:0, 8 for
/// 4:2:2), and (on some units) the motion of a macroblock row.
pub struct Unit {
    pub y: Samples,
    pub cb: Samples,
    pub cr: Samples,
    pub motion: Option<Box<[MbMotion]>>,
}

/// A frame ready for output: the cropped Y, Cb and Cr planes tightly packed as bytes (as
/// [`PicShared::planes`] returns them), written by the decoding thread as its rows became
/// final.
pub struct Planes(pub [Vec<u8>; 3]);

pub struct PicShared {
    pub width_mbs: usize,
    /// Frame macroblock rows; each field has this many units.
    pub height_mbs: usize,
    pub structure: Structure,
    pub fmt: Format,
    /// A mid-grey luma line and chroma line (stand-ins for missing units).
    grey: [Samples; 2],
    units: [Box<[OnceLock<Unit>]>; 2],
    progress: [AtomicUsize; 2],
    /// 0, or 1 + the parity of the only field there will ever be (a lone field).
    lone: AtomicU8,
    /// The frame for output, when its decoder wrote one (set before the last units are
    /// published). Units then need samples only if the picture is a reference.
    frame: Mutex<Option<Planes>>,
    lock: Mutex<()>,
    cv: Condvar,
}

/// `n` samples of value `v`, of the type `fmt` stores.
fn filled(fmt: Format, n: usize, v: i32) -> Samples {
    if fmt.wide() {
        Samples::W(vec![v as u16; n].into())
    } else {
        Samples::B(vec![v as u8; n].into())
    }
}

impl PicShared {
    pub fn new(
        width_mbs: usize,
        height_mbs: usize,
        structure: Structure,
        fmt: Format,
    ) -> PicShared {
        let col = || (0..height_mbs).map(|_| OnceLock::new()).collect();
        PicShared {
            width_mbs,
            height_mbs,
            structure,
            fmt,
            grey: [
                filled(fmt, width_mbs * 16, fmt.grey(0)),
                filled(fmt, width_mbs * fmt.mbcw(), fmt.grey(1)),
            ],
            units: [col(), col()],
            progress: [AtomicUsize::new(0), AtomicUsize::new(0)],
            lone: AtomicU8::new(0),
            frame: Mutex::new(None),
            lock: Mutex::new(()),
            cv: Condvar::new(),
        }
    }

    /// A finished mid-grey picture (stand-in for missing references).
    pub fn grey(width_mbs: usize, height_mbs: usize, fmt: Format) -> PicShared {
        let p = PicShared::new(width_mbs, height_mbs, Structure::Frame, fmt);
        p.fill_grey(0);
        p.fill_grey(1);
        p
    }

    /// Publishes mid-grey intra units for the rest of field `parity`.
    pub fn fill_grey(&self, parity: usize) {
        for u in self.progress[parity].load(Ordering::Acquire)..self.height_mbs {
            self.publish(parity, u, self.grey_unit(parity == 0 || u % 2 == 0));
        }
    }

    fn grey_unit(&self, motion: bool) -> Unit {
        let w = self.width_mbs * 16;
        let f = self.fmt;
        let cn = (w >> f.cxshift()) * (8 >> f.cshift());
        Unit {
            y: filled(f, w * 8, f.grey(0)),
            cb: filled(f, cn, f.grey(1)),
            cr: filled(f, cn, f.grey(1)),
            motion: motion.then(|| vec![INTRA_MOTION; self.width_mbs].into()),
        }
    }

    /// Publishes unit `u` of field `parity`; the units of a field must be published in order.
    pub fn publish(&self, parity: usize, u: usize, unit: Unit) {
        let _ = self.units[parity][u].set(unit);
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.progress[parity].store(u + 1, Ordering::Release);
        self.cv.notify_all();
    }

    /// Hands over the whole frame for output; call before publishing its last units.
    pub fn set_frame(&self, f: Planes) {
        *self.frame.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
    }

    /// Marks the picture as the lone field `parity`: the other field reads as this one.
    pub fn set_lone(&self, parity: usize) {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.lone.store(1 + parity as u8, Ordering::Release);
        self.cv.notify_all();
    }

    /// The field actually holding the samples of field `p`.
    #[inline(always)]
    fn src(&self, p: usize) -> usize {
        match self.lone.load(Ordering::Acquire) {
            0 => p,
            l => (l - 1) as usize,
        }
    }

    pub fn units_done(&self, parity: usize) -> usize {
        self.progress[self.src(parity)].load(Ordering::Acquire)
    }

    pub fn field_done(&self, parity: usize) -> bool {
        self.units_done(parity) >= self.height_mbs
    }

    pub fn is_done(&self) -> bool {
        self.field_done(0) && self.field_done(1)
    }

    /// Blocks until units `0..need` of field `p` are final. The check is inlined (it is
    /// made for every motion-compensated block); the wait is not.
    #[inline(always)]
    fn wait_units(&self, p: usize, need: usize) {
        if self.units_done(p) < need.min(self.height_mbs) {
            self.wait_units_slow(p, need.min(self.height_mbs));
        }
    }

    #[cold]
    #[inline(never)]
    fn wait_units_slow(&self, p: usize, need: usize) {
        let mut g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        while self.units_done(p) < need {
            g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn wait_field(&self, parity: usize) {
        self.wait_units(parity, self.height_mbs);
    }

    pub fn wait_done(&self) {
        self.wait_field(0);
        self.wait_field(1);
    }

    #[inline(always)]
    fn unit(&self, p: usize, u: usize) -> Option<&Unit> {
        self.units[self.src(p)].get(u).and_then(|c| c.get())
    }

    /// Plane `plane` (0 = Y, 1 = Cb, 2 = Cr) of unit `u` of field `p`, if published with
    /// samples of type `P` (units of a non-reference frame have none).
    #[inline(always)]
    fn unit_plane<P: Pix>(&self, p: usize, u: usize, plane: usize) -> Option<&[P]> {
        self.unit(p, u)
            .and_then(|un| {
                P::slice(match plane {
                    0 => &un.y,
                    1 => &un.cb,
                    _ => &un.cr,
                })
            })
            .filter(|l| !l.is_empty())
    }

    /// Motion of frame macroblock (`mbx`, `mby`) of a frame-coded picture, waiting for it.
    pub fn motion_frame(&self, mbx: usize, mby: usize) -> MbMotion {
        self.wait_units(0, mby + 1);
        self.unit(0, mby)
            .and_then(|u| u.motion.as_ref())
            .and_then(|m| m.get(mbx).copied())
            .unwrap_or(INTRA_MOTION)
    }

    /// Motion of macroblock (`mbx`, `mby`) of field `parity` of a field-coded picture.
    pub fn motion_field(&self, parity: usize, mbx: usize, mby: usize) -> MbMotion {
        self.wait_units(parity, 2 * mby + 1);
        self.unit(parity, 2 * mby)
            .and_then(|u| u.motion.as_ref())
            .and_then(|m| m.get(mbx).copied())
            .unwrap_or(INTRA_MOTION)
    }

    /// Copies the cropped picture into tightly packed planes (Cb and Cr subsampled as the
    /// stream is; samples deeper than 8 bits as little-endian `u16`).
    pub fn planes(&self, crop: [u32; 4]) -> (u32, u32, [Vec<u8>; 3]) {
        if self.fmt.wide() {
            self.planes_of::<u16>(crop)
        } else {
            self.planes_of::<u8>(crop)
        }
    }

    fn planes_of<P: Pix>(&self, crop: [u32; 4]) -> (u32, u32, [Vec<u8>; 3]) {
        self.wait_done();
        let w = self.width_mbs * 16 - (crop[0] + crop[1]) as usize;
        let h = self.height_mbs * 16 - (crop[2] + crop[3]) as usize;
        let (cl, ct) = (crop[0] as usize, crop[2] as usize);
        let s = self.fmt.cshift();
        let sx = self.fmt.cxshift();
        let (cw, ch) = (w.div_ceil(1 << sx), h.div_ceil(1 << s));
        let frame = self.frame.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(f) = frame {
            return (w as u32, h as u32, f.0);
        }
        let v = View::frame(self);
        let bps = size_of::<P>();
        let mut y = Vec::with_capacity(w * h * bps);
        for line in 0..h {
            let l = v.luma_line::<P>((line + ct) as isize);
            P::put_le(&mut y, &l[cl..cl + w]);
        }
        let mut c = [
            Vec::with_capacity(cw * ch * bps),
            Vec::with_capacity(cw * ch * bps),
        ];
        for (p, out) in c.iter_mut().enumerate() {
            for line in 0..ch {
                let l = v.chroma_line::<P>(p, (line + (ct >> s)) as isize);
                P::put_le(out, &l[cl >> sx..(cl >> sx) + cw]);
            }
        }
        let [cb, cr] = c;
        (w as u32, h as u32, [y, cb, cr])
    }
}

/// A picture as a reference sees it: the whole frame, or one of its fields.
#[derive(Clone, Copy)]
pub struct View<'a> {
    pub pic: &'a PicShared,
    pub parity: Option<usize>,
}

impl<'a> View<'a> {
    pub fn frame(pic: &'a PicShared) -> View<'a> {
        View { pic, parity: None }
    }

    pub fn new(pic: &'a PicShared, parity: Option<u8>) -> View<'a> {
        View {
            pic,
            parity: parity.map(|p| p as usize),
        }
    }

    /// Luma lines.
    #[inline(always)]
    pub fn height(&self) -> usize {
        match self.parity {
            None => self.pic.height_mbs * 16,
            Some(_) => self.pic.height_mbs * 8,
        }
    }

    /// Blocks until luma lines `0..=y` (clamped) and the chroma lines with them are final.
    #[inline(always)]
    pub fn wait_luma(&self, y: isize) {
        let y = y.clamp(0, self.height() as isize - 1) as usize;
        match self.parity {
            None => {
                self.pic.wait_units(0, (y >> 4) + 1);
                self.pic.wait_units(1, (y >> 4) + 1);
            }
            Some(p) => self.pic.wait_units(p, (y >> 3) + 1),
        }
    }

    /// Blocks until chroma lines `0..=y` (clamped) are final.
    #[inline]
    pub fn wait_chroma(&self, y: isize) {
        let s = self.pic.fmt.cshift();
        self.wait_luma((y << s) + s as isize);
    }

    /// Luma line `y` (clamped into the picture or field). Must be final.
    #[inline(always)]
    pub fn luma_line<P: Pix>(&self, y: isize) -> &'a [P] {
        let y = y.clamp(0, self.height() as isize - 1) as usize;
        let (p, fy) = match self.parity {
            None => (y & 1, y >> 1),
            Some(p) => (p, y),
        };
        let w = self.pic.width_mbs * 16;
        match self.pic.unit_plane::<P>(p, fy >> 3, 0) {
            Some(l) => &l[(fy & 7) * w..(fy & 7) * w + w],
            None => self.fallback(0, w),
        }
    }

    /// Calls `f(r, line)` for lines `y0 + r` (r in 0..n, clamped like `luma_line` and
    /// `chroma_line`) of `plane` (0 = Y, 1 = Cb, 2 = Cr), looking each unit up once. The
    /// lines must be final.
    #[inline(always)]
    pub fn rows<P: Pix>(
        &self,
        plane: usize,
        y0: isize,
        n: usize,
        mut f: impl FnMut(usize, &'a [P]),
    ) {
        let s = if plane == 0 { 0 } else { self.pic.fmt.cshift() };
        let h = (self.height() >> s) as isize;
        let w = self.pic.width_mbs * if plane == 0 { 16 } else { self.pic.fmt.mbcw() };
        // Lines per unit (of one field): 8, or 4 for 4:2:0 chroma.
        let ul = 3 - s;
        // The unit last looked up per field, and its plane (empty: not available).
        let mut cache: [(usize, &'a [P]); 2] = [(usize::MAX, &[]); 2];
        for r in 0..n {
            let y = (y0 + r as isize).clamp(0, h - 1) as usize;
            let (p, fy) = match self.parity {
                None => (y & 1, y >> 1),
                Some(p) => (p, y),
            };
            let u = fy >> ul;
            if cache[p].0 != u {
                cache[p] = (u, self.unit_plane::<P>(p, u, plane).unwrap_or(&[]));
            }
            let l = cache[p].1;
            let line = if l.is_empty() {
                self.fallback(plane.min(1), w)
            } else {
                let o = (fy & ((1 << ul) - 1)) * w;
                &l[o..o + w]
            };
            f(r, line);
        }
    }

    /// Copies samples `x..x + N` of lines `y0..y0 + n` of `plane` (0 = Y, 1 = Cb, 2 = Cr)
    /// into rows of `dst` (stride `ds`), positions clamped into the picture or field. The
    /// lines must be final. A window inside the picture is copied unit by unit with no
    /// per-line clamping or lookups; one crossing an edge goes line by line.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    pub fn fetch<P: Pix, const N: usize>(
        &self,
        plane: usize,
        x: i32,
        y0: i32,
        n: usize,
        dst: &mut [P],
        ds: usize,
    ) {
        let s = if plane == 0 { 0 } else { self.pic.fmt.cshift() };
        let h = (self.height() >> s) as i32;
        let w = self.pic.width_mbs * if plane == 0 { 16 } else { self.pic.fmt.mbcw() };
        if x < 0 || x as usize + N > w || y0 < 0 || y0 + n as i32 > h {
            self.fetch_clamped::<P, N>(plane, x, y0, n, dst, ds);
            return;
        }
        let (x, y0) = (x as usize, y0 as usize);
        // Lines per unit (of one field): 8, or 4 for 4:2:0 chroma.
        let ul = 3 - s;
        match self.parity {
            None => {
                // Rows k, k + 2, ... are consecutive lines of field (y0 + k) & 1.
                for k in 0..n.min(2) {
                    let y = y0 + k;
                    let m = (n - k).div_ceil(2);
                    self.fetch_field::<P, N>(
                        plane,
                        y & 1,
                        y >> 1,
                        m,
                        x,
                        w,
                        ul,
                        &mut dst[k * ds..],
                        2 * ds,
                    );
                }
            }
            Some(p) => self.fetch_field::<P, N>(plane, p, y0, n, x, w, ul, dst, ds),
        }
    }

    /// Copies samples `x..x + N` of lines `fy..fy + m` (all inside the field) of field `p`.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn fetch_field<P: Pix, const N: usize>(
        &self,
        plane: usize,
        p: usize,
        mut fy: usize,
        m: usize,
        x: usize,
        w: usize,
        ul: usize,
        dst: &mut [P],
        ds: usize,
    ) {
        let end = fy + m;
        let mut d = 0;
        while fy < end {
            let u = fy >> ul;
            let k = ((u + 1) << ul).min(end) - fy;
            // The unit's lines from `fy`, and the distance between them (0: a stand-in line).
            let (src, ls) = match self.unit_plane::<P>(p, u, plane) {
                Some(l) => (&l[(fy & ((1 << ul) - 1)) * w..], w),
                None => (self.fallback(plane.min(1), w), 0),
            };
            for i in 0..k {
                let o = i * ls + x;
                dst[d..d + N].copy_from_slice(&src[o..o + N]);
                d += ds;
            }
            fy += k;
        }
    }

    /// `fetch` for a window crossing an edge of the picture: line by line, clamped.
    #[inline(never)]
    fn fetch_clamped<P: Pix, const N: usize>(
        &self,
        plane: usize,
        x: i32,
        y0: i32,
        n: usize,
        dst: &mut [P],
        ds: usize,
    ) {
        let pw = (self.pic.width_mbs * if plane == 0 { 16 } else { self.pic.fmt.mbcw() }) as i32;
        self.rows::<P>(plane, y0 as isize, n, |r, line| {
            let d = &mut dst[r * ds..r * ds + N];
            if x >= 0 && x + N as i32 <= pw {
                d.copy_from_slice(&line[x as usize..x as usize + N]);
            } else {
                for (c, d) in d.iter_mut().enumerate() {
                    *d = line[(x + c as i32).clamp(0, pw - 1) as usize];
                }
            }
        });
    }

    /// Plane `plane` of unit `u` of field `p`, if published with samples of type `P`.
    #[inline(always)]
    fn unit_plane<P: Pix>(&self, p: usize, u: usize, plane: usize) -> Option<&'a [P]> {
        self.pic.unit_plane(p, u, plane)
    }

    /// Hints the cache to fetch samples `x..x + w` of luma lines `y..y + h` and the chroma
    /// samples with them (all clamped into the picture). Nothing waits: lines that are not
    /// final yet are skipped (their stand-ins are fetched instead).
    #[inline]
    pub fn prefetch<P: Pix>(&self, x: i32, y: i32, w: i32, h: i32) {
        let pw = (self.pic.width_mbs * 16) as i32;
        let (x0, x1) = (
            x.clamp(0, pw - 1) as usize,
            (x + w - 1).clamp(0, pw - 1) as usize,
        );
        self.prefetch_plane::<P>(0, x0, x1, y, h as usize);
        let s = self.pic.fmt.cshift() as i32;
        let sx = self.pic.fmt.cxshift();
        let (c0, c1) = (x0 >> sx, x1 >> sx);
        let (cy, ch) = (y >> s, ((y + h - 1) >> s) - (y >> s) + 1);
        for c in 1..3 {
            self.prefetch_plane::<P>(c, c0, c1, cy, ch as usize);
        }
    }

    /// Prefetches samples `x0` and `x1` (inside the line) of lines `y0..y0 + n` (clamped) of
    /// `plane`; unit by unit when the lines are inside the picture or field.
    #[inline(always)]
    fn prefetch_plane<P: Pix>(&self, plane: usize, x0: usize, x1: usize, y0: i32, n: usize) {
        use std::hint::{Locality, prefetch_read};
        let s = if plane == 0 { 0 } else { self.pic.fmt.cshift() };
        let h = (self.height() >> s) as i32;
        if y0 < 0 || y0 + n as i32 > h {
            self.rows::<P>(plane, y0 as isize, n, |_, l| {
                prefetch_read(&l[x0], Locality::L1);
                prefetch_read(&l[x1], Locality::L1);
            });
            return;
        }
        let y0 = y0 as usize;
        match self.parity {
            None => {
                for k in 0..n.min(2) {
                    let y = y0 + k;
                    self.prefetch_field::<P>(plane, y & 1, y >> 1, (n - k).div_ceil(2), x0, x1);
                }
            }
            Some(p) => self.prefetch_field::<P>(plane, p, y0, n, x0, x1),
        }
    }

    /// [`View::prefetch_plane`] for `m` lines of field `p` from field line `fy`, all inside it.
    #[inline(always)]
    fn prefetch_field<P: Pix>(
        &self,
        plane: usize,
        p: usize,
        mut fy: usize,
        m: usize,
        x0: usize,
        x1: usize,
    ) {
        use std::hint::{Locality, prefetch_read};
        let s = if plane == 0 { 0 } else { self.pic.fmt.cshift() };
        let w = self.pic.width_mbs * if plane == 0 { 16 } else { self.pic.fmt.mbcw() };
        let ul = 3 - s;
        let end = fy + m;
        while fy < end {
            let u = fy >> ul;
            let k = ((u + 1) << ul).min(end) - fy;
            if let Some(l) = self.unit_plane::<P>(p, u, plane) {
                let l = &l[(fy & ((1 << ul) - 1)) * w..];
                for i in 0..k {
                    prefetch_read(&l[i * w + x0], Locality::L1);
                    prefetch_read(&l[i * w + x1], Locality::L1);
                }
            }
            fy += k;
        }
    }

    /// A mid-grey line, or zeros when the picture holds another sample type.
    #[cold]
    fn fallback<P: Pix>(&self, c: usize, w: usize) -> &'a [P] {
        P::slice(&self.pic.grey[c]).unwrap_or(&P::zeros()[..w.min(P::zeros().len())])
    }

    /// Chroma line `y` of plane `c` (0 = Cb, 1 = Cr), clamped.
    #[inline(always)]
    pub fn chroma_line<P: Pix>(&self, c: usize, y: isize) -> &'a [P] {
        let s = self.pic.fmt.cshift();
        let y = y.clamp(0, (self.height() >> s) as isize - 1) as usize;
        let (p, fy) = match self.parity {
            None => (y & 1, y >> 1),
            Some(p) => (p, y),
        };
        let w = self.pic.width_mbs * self.pic.fmt.mbcw();
        // Chroma lines per unit: 4 (4:2:0) or 8 (4:2:2, 4:4:4).
        let ul = 3 - s;
        let r = fy & ((1 << ul) - 1);
        match self.pic.unit_plane::<P>(p, fy >> ul, 1 + c) {
            Some(l) => &l[r * w..r * w + w],
            None => self.fallback(1, w),
        }
    }
}
