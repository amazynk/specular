//! JB2 bitonal images and shape dictionaries (DjVu v3 specification, appendix 2): `Sjbz`
//! page masks and `Djbz` shared dictionaries.
//!
//! A JB2 stream is a sequence of records coded with the ZP-coder: new shapes coded directly
//! (a 10-pixel template), shapes refined from a library shape (an 11-pixel template across
//! both bitmaps, aligned on their centres), copies of library shapes, and their placement
//! relative to the previous shape on the text line. The decoded image is kept as the shape
//! library plus the list of placements ("blits"), so regions render without a page-sized
//! bitmap.
//!
//! Coordinates here are top-down: row 0 of a shape is its top row, and a blit's `top` is the
//! page row of the shape's top row.

use std::sync::Arc;

use crate::Error;
use crate::zp::Zp;

/// Largest JB2 image or shape side the format can code.
const BIG_POSITIVE: i32 = 262_142;
const BIG_NEGATIVE: i32 = -262_143;
/// Most number-coder cells (the reference encoder resets the coder at 20,000).
const MAX_CELLS: usize = 4 << 20;
/// Most shapes per image or dictionary.
const MAX_SHAPES: usize = 1 << 20;
/// Most blits per image.
const MAX_BLITS: usize = 4 << 20;
/// Most pixels of one shape.
const MAX_SHAPE_PIXELS: usize = 1 << 26;
/// Most shape pixels per stream, all shapes together.
const MAX_TOTAL_PIXELS: usize = 1 << 30;

/// One bitonal shape, one byte per pixel (0 or 1), rows top-down.
#[derive(Debug, Clone, Default)]
pub struct Shape {
    pub width: u32,
    pub height: u32,
    pub bits: Vec<u8>,
}

impl Shape {
    #[inline]
    pub(crate) fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            0
        } else {
            self.bits[y as usize * self.width as usize + x as usize]
        }
    }
}

/// Bounding box of a library shape's black pixels, in the reference decoder's bottom-up
/// convention (rows counted from the bottom).
#[derive(Debug, Clone, Copy)]
struct LibRect {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl LibRect {
    fn of(s: &Shape) -> LibRect {
        let (w, h) = (s.width as i32, s.height as i32);
        // Bottom-up row `r` is top-down row `h - 1 - r`.
        let col = |x: i32| (0..h).any(|y| s.get(x, y) != 0);
        let row_bu = |r: i32| (0..w).any(|x| s.get(x, h - 1 - r) != 0);
        let mut right = w - 1;
        while right >= 0 && !col(right) {
            right -= 1;
        }
        let mut top = h - 1;
        while top >= 0 && !row_bu(top) {
            top -= 1;
        }
        let mut left = 0;
        while left <= right && !col(left) {
            left += 1;
        }
        let mut bottom = 0;
        while bottom <= top && !row_bu(bottom) {
            bottom += 1;
        }
        LibRect {
            left,
            right,
            top,
            bottom,
        }
    }
}

/// A placed shape: its index in [`Image::shapes`] and the page position of its top-left
/// pixel (top-down rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blit {
    pub shape: u32,
    pub left: i32,
    pub top: i32,
}

/// A decoded shape dictionary (`Djbz`), shared by the pages that include it.
#[derive(Debug, Default)]
pub struct Dict {
    pub shapes: Vec<Arc<Shape>>,
}

/// A decoded JB2 image: its size, every shape it uses (inherited dictionary shapes first)
/// and its blits in coding order (the order FGbz colours follow).
#[derive(Debug, Default)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub shapes: Vec<Arc<Shape>>,
    pub blits: Vec<Blit>,
    /// The data broke off: the blits decoded before the damage.
    pub damaged: bool,
}

impl Image {
    /// Memory held by the image (shapes counted fully even when shared).
    pub fn memory_bytes(&self) -> usize {
        self.shapes.iter().map(|s| s.bits.len()).sum::<usize>() + self.blits.len() * 12
    }
}

/// The multivalued integer coder: a lazily grown binary tree of contexts per number kind.
struct NumCoder {
    bits: Vec<u8>,
    left: Vec<u32>,
    right: Vec<u32>,
}

impl NumCoder {
    fn new() -> NumCoder {
        // Cell 0 is the "not allocated" marker.
        NumCoder {
            bits: vec![0],
            left: vec![0],
            right: vec![0],
        }
    }

    fn reset(&mut self) {
        *self = NumCoder::new();
    }

    fn decode(
        &mut self,
        zp: &mut Zp<'_>,
        root: &mut u32,
        low: i32,
        high: i32,
    ) -> Result<i32, Error> {
        let (mut low, mut high) = (low, high);
        let mut negative = false;
        let mut cutoff: i32 = 0;
        let mut phase = 1;
        let mut range: i64 = -1;
        // The cell decided last and which of its children comes next (None: the root).
        let mut node: Option<u32> = None;
        let mut slot_is_right = false;
        let mut parent = 0u32;
        while range != 1 {
            // Allocate the current cell if needed.
            let cur = match node {
                None => {
                    if *root == 0 {
                        *root = self.alloc()?;
                    }
                    *root
                }
                Some(_) => {
                    let p = parent as usize;
                    let slot = if slot_is_right {
                        self.right[p]
                    } else {
                        self.left[p]
                    };
                    if slot == 0 {
                        let c = self.alloc()?;
                        if slot_is_right {
                            self.right[p] = c;
                        } else {
                            self.left[p] = c;
                        }
                        c
                    } else {
                        slot
                    }
                }
            };
            let decision =
                low >= cutoff || (high >= cutoff && zp.decode(&mut self.bits[cur as usize]));
            parent = cur;
            slot_is_right = decision;
            node = Some(cur);
            match phase {
                1 => {
                    negative = !decision;
                    if negative {
                        let t = -low - 1;
                        low = -high - 1;
                        high = t;
                    }
                    phase = 2;
                    cutoff = 1;
                }
                2 => {
                    if decision {
                        cutoff = cutoff
                            .checked_mul(2)
                            .and_then(|c| c.checked_add(1))
                            .ok_or(Error::Malformed("JB2 number out of range"))?;
                    } else {
                        phase = 3;
                        range = (i64::from(cutoff) + 1) / 2;
                        if range == 1 {
                            cutoff = 0;
                        } else {
                            cutoff -= (range / 2) as i32;
                        }
                    }
                }
                _ => {
                    range /= 2;
                    if range != 1 {
                        if decision {
                            cutoff += (range / 2) as i32;
                        } else {
                            cutoff -= (range / 2) as i32;
                        }
                    } else if !decision {
                        cutoff -= 1;
                    }
                }
            }
        }
        Ok(if negative { -cutoff - 1 } else { cutoff })
    }

    fn alloc(&mut self) -> Result<u32, Error> {
        if self.bits.len() >= MAX_CELLS {
            return Err(Error::Malformed("JB2 number coder too large"));
        }
        self.bits.push(0);
        self.left.push(0);
        self.right.push(0);
        Ok((self.bits.len() - 1) as u32)
    }
}

/// Roots of the number contexts.
#[derive(Default)]
struct Roots {
    record_type: u32,
    image_size: u32,
    match_index: u32,
    abs_size_x: u32,
    abs_size_y: u32,
    rel_size_x: u32,
    rel_size_y: u32,
    abs_loc_x: u32,
    abs_loc_y: u32,
    rel_loc_x_current: u32,
    rel_loc_y_current: u32,
    rel_loc_x_last: u32,
    rel_loc_y_last: u32,
    comment_length: u32,
    comment_byte: u32,
    inherited: u32,
}

const START_OF_DATA: i32 = 0;
const NEW_MARK: i32 = 1;
const NEW_MARK_LIBRARY_ONLY: i32 = 2;
const NEW_MARK_IMAGE_ONLY: i32 = 3;
const MATCHED_REFINE: i32 = 4;
const MATCHED_REFINE_LIBRARY_ONLY: i32 = 5;
const MATCHED_REFINE_IMAGE_ONLY: i32 = 6;
const MATCHED_COPY: i32 = 7;
const NON_MARK_DATA: i32 = 8;
const REQUIRED_DICT_OR_RESET: i32 = 9;
const PRESERVED_COMMENT: i32 = 10;
const END_OF_DATA: i32 = 11;

struct Decoder<'a, 'z> {
    zp: Zp<'z>,
    nums: NumCoder,
    roots: Roots,
    refinement_flag: u8,
    offset_type: u8,
    direct: Vec<u8>,
    cross: Vec<u8>,
    /// Library: shape index (in `shapes`) and its bounding box, by library number.
    library: Vec<(u32, LibRect)>,
    shapes: Vec<Arc<Shape>>,
    blits: Vec<Blit>,
    width: i32,
    height: i32,
    got_start: bool,
    // Placement state (the reference decoder's bottom-up, 1-based coordinates).
    last_left: i32,
    last_right: i32,
    last_bottom: i32,
    last_row_left: i32,
    last_row_bottom: i32,
    short_list: [i32; 3],
    short_pos: usize,
    total_pixels: usize,
    cancel: &'a dyn Fn() -> bool,
}

impl<'a, 'z> Decoder<'a, 'z> {
    fn new(data: &'z [u8], cancel: &'a dyn Fn() -> bool) -> Self {
        Decoder {
            zp: Zp::new(data),
            nums: NumCoder::new(),
            roots: Roots::default(),
            refinement_flag: 0,
            offset_type: 0,
            direct: vec![0; 1024],
            cross: vec![0; 2048],
            library: Vec::new(),
            shapes: Vec::new(),
            blits: Vec::new(),
            width: 0,
            height: 0,
            got_start: false,
            last_left: 0,
            last_right: 0,
            last_bottom: 0,
            last_row_left: 0,
            last_row_bottom: 0,
            short_list: [0; 3],
            short_pos: 0,
            total_pixels: 0,
            cancel,
        }
    }

    fn num(
        &mut self,
        which: fn(&mut Roots) -> &mut u32,
        low: i32,
        high: i32,
    ) -> Result<i32, Error> {
        let mut root = *which(&mut self.roots);
        let v = self.nums.decode(&mut self.zp, &mut root, low, high)?;
        *which(&mut self.roots) = root;
        Ok(v)
    }

    fn inherit(&mut self, dict: Option<&Dict>) -> Result<(), Error> {
        let n = self.num(|r| &mut r.inherited, 0, BIG_POSITIVE)? as usize;
        let Some(dict) = dict else {
            return Err(Error::Malformed(
                "JB2 data needs a shared dictionary that is missing",
            ));
        };
        if dict.shapes.len() < n {
            return Err(Error::Malformed("JB2 shared dictionary is too small"));
        }
        for s in &dict.shapes[..n] {
            let idx = self.shapes.len() as u32;
            let rect = LibRect::of(s);
            self.shapes.push(s.clone());
            self.library.push((idx, rect));
        }
        Ok(())
    }

    fn new_shape(&mut self, w: i32, h: i32) -> Result<Shape, Error> {
        if !(0..=i32::from(u16::MAX)).contains(&w) || !(0..=i32::from(u16::MAX)).contains(&h) {
            return Err(Error::Malformed("JB2 shape too large"));
        }
        let px = w as usize * h as usize;
        self.total_pixels += px;
        if px > MAX_SHAPE_PIXELS || self.total_pixels > MAX_TOTAL_PIXELS {
            return Err(Error::TooLarge);
        }
        Ok(Shape {
            width: w as u32,
            height: h as u32,
            bits: vec![0; px],
        })
    }

    fn abs_size(&mut self) -> Result<Shape, Error> {
        let w = self.num(|r| &mut r.abs_size_x, 0, BIG_POSITIVE)?;
        let h = self.num(|r| &mut r.abs_size_y, 0, BIG_POSITIVE)?;
        self.new_shape(w, h)
    }

    fn rel_size(&mut self, cw: u32, ch: u32) -> Result<Shape, Error> {
        let dw = self.num(|r| &mut r.rel_size_x, BIG_NEGATIVE, BIG_POSITIVE)?;
        let dh = self.num(|r| &mut r.rel_size_y, BIG_NEGATIVE, BIG_POSITIVE)?;
        self.new_shape(cw as i32 + dw, ch as i32 + dh)
    }

    fn check_cancel(&self) -> Result<(), Error> {
        if (self.cancel)() {
            return Err(Error::Cancelled);
        }
        if self.zp.overrun() > 1024 {
            return Err(Error::Malformed("truncated JB2 data"));
        }
        Ok(())
    }

    fn direct(&mut self, s: &mut Shape) -> Result<(), Error> {
        let (w, h) = (s.width as usize, s.height as usize);
        if w == 0 || h == 0 {
            return Ok(());
        }
        // Two blank rows above and two blank columns each side.
        let pw = w + 4;
        let mut p = vec![0u8; pw * (h + 2)];
        for y in 0..h {
            if y % 64 == 0 {
                self.check_cancel()?;
            }
            let r0 = (y + 2) * pw + 2;
            let r1 = r0 - pw;
            let r2 = r1 - pw;
            let mut ctx = (usize::from(p[r2 - 1]) << 9)
                | (usize::from(p[r2]) << 8)
                | (usize::from(p[r2 + 1]) << 7)
                | (usize::from(p[r1 - 2]) << 6)
                | (usize::from(p[r1 - 1]) << 5)
                | (usize::from(p[r1]) << 4)
                | (usize::from(p[r1 + 1]) << 3)
                | (usize::from(p[r1 + 2]) << 2)
                | (usize::from(p[r0 - 2]) << 1)
                | usize::from(p[r0 - 1]);
            for x in 0..w {
                let n = usize::from(self.zp.decode(&mut self.direct[ctx]));
                p[r0 + x] = n as u8;
                let c = x + 1;
                ctx = ((ctx << 1) & 0x37a)
                    | (usize::from(p[r1 + c + 2]) << 2)
                    | (usize::from(p[r2 + c + 1]) << 7)
                    | n;
            }
        }
        for y in 0..h {
            let r0 = (y + 2) * pw + 2;
            s.bits[y * w..(y + 1) * w].copy_from_slice(&p[r0..r0 + w]);
        }
        Ok(())
    }

    fn refine(&mut self, s: &mut Shape, lib: usize) -> Result<(), Error> {
        let (shape_idx, l) = self.library[lib];
        let c = self.shapes[shape_idx as usize].clone();
        let (dw, dh) = (s.width as i32, s.height as i32);
        if dw == 0 || dh == 0 {
            return Ok(());
        }
        let ch = c.height as i32;
        // Centres aligned: the left of two central columns, the lower of two central rows
        // (rows counted from the bottom).
        let xd2c = l.left + (l.right - l.left) / 2 - (dw - 1) / 2;
        let yd2c = l.bottom + (l.top - l.bottom) / 2 - (dh - 1) / 2;
        // Reference row (top-down) for current row r is r + off_y; column x maps to x + xd2c.
        let off_y = ch - dh - yd2c;
        let (w, h) = (dw as usize, dh as usize);
        // Aligned reference with a one-pixel margin: ref[(r + 1) * rw + (x + 1)].
        let rw = w + 2;
        let mut rf = vec![0u8; rw * (h + 2)];
        for r in 0..h + 2 {
            let cy = r as i32 - 1 + off_y;
            for x in 0..rw {
                rf[r * rw + x] = c.get(x as i32 - 1 + xd2c, cy);
            }
        }
        // Current bitmap with one blank row above and margins.
        let pw = w + 2;
        let mut p = vec![0u8; pw * (h + 1)];
        for y in 0..h {
            if y % 64 == 0 {
                self.check_cancel()?;
            }
            let up1 = y * pw + 1;
            let up0 = up1 + pw;
            let xup1 = y * rw + 1;
            let xup0 = xup1 + rw;
            let xdn1 = xup0 + rw;
            for x in 0..w {
                let ctx = (usize::from(p[up1 + x - 1]) << 10)
                    | (usize::from(p[up1 + x]) << 9)
                    | (usize::from(p[up1 + x + 1]) << 8)
                    | (usize::from(p[up0 + x - 1]) << 7)
                    | (usize::from(rf[xup1 + x]) << 6)
                    | (usize::from(rf[xup0 + x - 1]) << 5)
                    | (usize::from(rf[xup0 + x]) << 4)
                    | (usize::from(rf[xup0 + x + 1]) << 3)
                    | (usize::from(rf[xdn1 + x - 1]) << 2)
                    | (usize::from(rf[xdn1 + x]) << 1)
                    | usize::from(rf[xdn1 + x + 1]);
                let n = self.zp.decode(&mut self.cross[ctx]);
                p[up0 + x] = u8::from(n);
            }
        }
        for y in 0..h {
            let r0 = (y + 1) * pw + 1;
            s.bits[y * w..(y + 1) * w].copy_from_slice(&p[r0..r0 + w]);
        }
        Ok(())
    }

    fn match_index(&mut self) -> Result<usize, Error> {
        if self.library.is_empty() {
            return Err(Error::Malformed("JB2 match without a library"));
        }
        let hi = self.library.len() as i32 - 1;
        Ok(self.num(|r| &mut r.match_index, 0, hi)? as usize)
    }

    fn fill_short_list(&mut self, v: i32) {
        self.short_list = [v; 3];
        self.short_pos = 0;
    }

    fn update_short_list(&mut self, v: i32) -> i32 {
        self.short_pos = (self.short_pos + 1) % 3;
        self.short_list[self.short_pos] = v;
        let s = self.short_list;
        if s[0] >= s[1] {
            if s[0] > s[2] {
                if s[1] >= s[2] { s[1] } else { s[2] }
            } else {
                s[0]
            }
        } else if s[0] < s[2] {
            if s[1] >= s[2] { s[2] } else { s[1] }
        } else {
            s[0]
        }
    }

    /// Decodes a location relative to the previous shapes; returns the blit's top-left
    /// (top-down page coordinates).
    fn rel_location(&mut self, rows: i32, cols: i32) -> Result<(i32, i32), Error> {
        if !self.got_start {
            return Err(Error::Malformed("JB2 shape before the start record"));
        }
        let new_row = self.zp.decode(&mut self.offset_type);
        let (left, bottom, top);
        if new_row {
            let dx = self.num(|r| &mut r.rel_loc_x_last, BIG_NEGATIVE, BIG_POSITIVE)?;
            let dy = self.num(|r| &mut r.rel_loc_y_last, BIG_NEGATIVE, BIG_POSITIVE)?;
            left = self.last_row_left.saturating_add(dx);
            top = self.last_row_bottom.saturating_add(dy);
            let right = left.saturating_add(cols - 1);
            bottom = top.saturating_sub(rows - 1);
            self.last_left = left;
            self.last_row_left = left;
            self.last_right = right;
            self.last_bottom = bottom;
            self.last_row_bottom = bottom;
            self.fill_short_list(bottom);
        } else {
            let dx = self.num(|r| &mut r.rel_loc_x_current, BIG_NEGATIVE, BIG_POSITIVE)?;
            let dy = self.num(|r| &mut r.rel_loc_y_current, BIG_NEGATIVE, BIG_POSITIVE)?;
            left = self.last_right.saturating_add(dx);
            bottom = self.last_bottom.saturating_add(dy);
            let right = left.saturating_add(cols - 1);
            top = bottom.saturating_add(rows - 1);
            self.last_left = left;
            self.last_right = right;
            self.last_bottom = self.update_short_list(bottom);
        }
        let _ = bottom;
        // 1-based bottom-up `top` row → 0-based top-down row.
        Ok((left - 1, self.height - top))
    }

    fn abs_location(&mut self, rows: i32) -> Result<(i32, i32), Error> {
        let (w, h) = (self.width.max(1), self.height.max(1));
        let left = self.num(|r| &mut r.abs_loc_x, 1, w)?;
        let top = self.num(|r| &mut r.abs_loc_y, 1, h)?;
        let _ = rows;
        Ok((left - 1, self.height - top))
    }

    fn add_library(&mut self, idx: u32) -> Result<(), Error> {
        if self.library.len() >= MAX_SHAPES {
            return Err(Error::TooLarge);
        }
        // Empty edge rows and columns are removed before a shape enters the library (the
        // specification, 11.5.10): later sizes, refinements and copies refer to the crop.
        let rect = LibRect::of(&self.shapes[idx as usize]);
        let s = &self.shapes[idx as usize];
        let (w, h) = (s.width as i32, s.height as i32);
        if rect.right >= rect.left
            && rect.top >= rect.bottom
            && !(rect.left == 0 && rect.right == w - 1 && rect.bottom == 0 && rect.top == h - 1)
        {
            let cw = (rect.right - rect.left + 1) as usize;
            let ch = (rect.top - rect.bottom + 1) as usize;
            let y0 = (h - 1 - rect.top) as usize;
            let x0 = rect.left as usize;
            let mut bits = Vec::with_capacity(cw * ch);
            for y in y0..y0 + ch {
                let row = y * w as usize + x0;
                bits.extend_from_slice(&s.bits[row..row + cw]);
            }
            let cropped = Shape {
                width: cw as u32,
                height: ch as u32,
                bits,
            };
            let r = LibRect::of(&cropped);
            let ci = self.push_shape(cropped)?;
            self.library.push((ci, r));
            return Ok(());
        }
        self.library.push((idx, rect));
        Ok(())
    }

    fn push_shape(&mut self, s: Shape) -> Result<u32, Error> {
        if self.shapes.len() >= MAX_SHAPES {
            return Err(Error::TooLarge);
        }
        self.shapes.push(Arc::new(s));
        Ok((self.shapes.len() - 1) as u32)
    }

    fn blit(&mut self, shape: u32, left: i32, top: i32) -> Result<(), Error> {
        if self.blits.len() >= MAX_BLITS {
            return Err(Error::TooLarge);
        }
        self.blits.push(Blit { shape, left, top });
        Ok(())
    }

    /// Runs the records; `dict_mode` for a shared dictionary.
    fn run(&mut self, dict: Option<&Dict>, dict_mode: bool) -> Result<(), Error> {
        let mut records = 0usize;
        loop {
            records += 1;
            if records.is_multiple_of(256) {
                self.check_cancel()?;
            }
            let t = self.num(|r| &mut r.record_type, START_OF_DATA, END_OF_DATA)?;
            if !self.got_start
                && !matches!(
                    t,
                    START_OF_DATA | REQUIRED_DICT_OR_RESET | PRESERVED_COMMENT
                )
            {
                return Err(Error::Malformed("JB2 data without a start record"));
            }
            if dict_mode
                && matches!(
                    t,
                    NEW_MARK
                        | NEW_MARK_IMAGE_ONLY
                        | MATCHED_REFINE
                        | MATCHED_REFINE_IMAGE_ONLY
                        | MATCHED_COPY
                        | NON_MARK_DATA
                )
            {
                return Err(Error::Malformed("JB2 dictionary with image records"));
            }
            match t {
                START_OF_DATA => {
                    if self.got_start {
                        return Err(Error::Malformed("JB2 data with two start records"));
                    }
                    let w = self.num(|r| &mut r.image_size, 0, BIG_POSITIVE)?;
                    let h = self.num(|r| &mut r.image_size, 0, BIG_POSITIVE)?;
                    if self.zp.decode(&mut self.refinement_flag) {
                        return Err(Error::Unsupported("JB2 image refinement data"));
                    }
                    self.width = w;
                    self.height = h;
                    self.last_left = 1 + w;
                    self.last_row_bottom = h;
                    self.last_row_left = 0;
                    self.last_right = 0;
                    self.last_bottom = h;
                    self.fill_short_list(h);
                    self.got_start = true;
                }
                NEW_MARK | NEW_MARK_LIBRARY_ONLY | NEW_MARK_IMAGE_ONLY => {
                    let mut s = self.abs_size()?;
                    self.direct(&mut s)?;
                    let (rows, cols) = (s.height as i32, s.width as i32);
                    let idx = self.push_shape(s)?;
                    if t != NEW_MARK_LIBRARY_ONLY {
                        let (x, y) = self.rel_location(rows, cols)?;
                        self.blit(idx, x, y)?;
                    }
                    if t != NEW_MARK_IMAGE_ONLY {
                        self.add_library(idx)?;
                    }
                }
                MATCHED_REFINE | MATCHED_REFINE_LIBRARY_ONLY | MATCHED_REFINE_IMAGE_ONLY => {
                    let m = self.match_index()?;
                    let base = &self.shapes[self.library[m].0 as usize];
                    let (cw, ch) = (base.width, base.height);
                    let mut s = self.rel_size(cw, ch)?;
                    self.refine(&mut s, m)?;
                    let (rows, cols) = (s.height as i32, s.width as i32);
                    let idx = self.push_shape(s)?;
                    if t != MATCHED_REFINE_LIBRARY_ONLY {
                        let (x, y) = self.rel_location(rows, cols)?;
                        self.blit(idx, x, y)?;
                    }
                    if t != MATCHED_REFINE_IMAGE_ONLY {
                        self.add_library(idx)?;
                    }
                }
                MATCHED_COPY => {
                    let m = self.match_index()?;
                    let idx = self.library[m].0;
                    let s = &self.shapes[idx as usize];
                    let (rows, cols) = (s.height as i32, s.width as i32);
                    let (x, y) = self.rel_location(rows, cols)?;
                    self.blit(idx, x, y)?;
                }
                NON_MARK_DATA => {
                    let mut s = self.abs_size()?;
                    self.direct(&mut s)?;
                    let rows = s.height as i32;
                    let idx = self.push_shape(s)?;
                    let (x, y) = self.abs_location(rows)?;
                    self.blit(idx, x, y)?;
                }
                REQUIRED_DICT_OR_RESET => {
                    if self.got_start {
                        self.nums.reset();
                        self.roots = Roots::default();
                    } else {
                        if !self.library.is_empty() {
                            return Err(Error::Malformed("JB2 data with two dictionaries"));
                        }
                        self.inherit(dict)?;
                    }
                }
                PRESERVED_COMMENT => {
                    let n = self.num(|r| &mut r.comment_length, 0, BIG_POSITIVE)?;
                    for i in 0..n {
                        if i % 4096 == 0 {
                            self.check_cancel()?;
                        }
                        self.num(|r| &mut r.comment_byte, 0, 255)?;
                    }
                }
                _ => return Ok(()),
            }
        }
    }
}

/// Decodes a page mask (`Sjbz`), with the shared dictionary its pages include.
pub fn decode_image(
    data: &[u8],
    dict: Option<&Dict>,
    cancel: &dyn Fn() -> bool,
) -> Result<Image, Error> {
    let mut d = Decoder::new(data, cancel);
    let r = d.run(dict, false);
    // Damaged data after the start keeps the shapes placed so far.
    let damaged = match r {
        Ok(()) => false,
        Err(e @ (Error::Cancelled | Error::TooLarge)) => return Err(e),
        Err(e) if !d.got_start || d.blits.is_empty() => return Err(e),
        Err(_) => true,
    };
    if !d.got_start {
        return Err(Error::Malformed("JB2 data without a start record"));
    }
    Ok(Image {
        width: d.width as u32,
        height: d.height as u32,
        shapes: d.shapes,
        blits: d.blits,
        damaged,
    })
}

/// Decodes a shared shape dictionary (`Djbz`), itself possibly inheriting from `parent`.
pub fn decode_dict(
    data: &[u8],
    parent: Option<&Dict>,
    cancel: &dyn Fn() -> bool,
) -> Result<Dict, Error> {
    let mut d = Decoder::new(data, cancel);
    d.run(parent, true)?;
    // Library order: inherited shapes, then the ones this dictionary defines.
    let shapes = d
        .library
        .iter()
        .map(|&(i, _)| d.shapes[i as usize].clone())
        .collect();
    Ok(Dict { shapes })
}
