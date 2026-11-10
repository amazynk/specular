//! Develop decoded sensor data: linearize, subtract black, scale to white, white balance and
//! clip (one lookup table per CFA position), demosaic (2x2 binning for half size,
//! Malvar-He-Cutler for full size) and convert to linear Rec.709 with the camera matrix. Row
//! bands run in parallel on the rayon pool. A DNG OpcodeList3 runs between demosaic and the
//! colour matrix, tile by tile on white-balanced camera RGB (`opcode::develop_stage3`).

use std::sync::atomic::Ordering;

use rayon::prelude::*;

use crate::cfa::{BLUE, GREEN, RED};
use crate::error::{RawError, Result};
use crate::{DecodeOptions, OutputSize, RawImage, Rect, RgbImage, WbSource};

pub(crate) const BAND: usize = 16;

/// Fixed-point (Q12) colour matrix.
pub(crate) struct Matrix {
    m: [[i32; 3]; 3],
    /// DNG profile with look tables: evaluated in floating point instead of `m`.
    profile: Option<std::sync::Arc<crate::ProfileTransform>>,
}

impl Matrix {
    pub(crate) fn new(m: &[[f32; 3]; 3]) -> Matrix {
        Matrix {
            m: std::array::from_fn(|i| std::array::from_fn(|j| (m[i][j] * 4096.0).round() as i32)),
            profile: None,
        }
    }
    /// The colour transform of `raw`: its profile when it has one, else `rgb_cam`.
    pub(crate) fn of(raw: &RawImage) -> Matrix {
        Matrix {
            profile: raw.profile.clone(),
            ..Matrix::new(&raw.rgb_cam)
        }
    }
    #[inline(always)]
    pub(crate) fn apply(&self, r: i32, g: i32, b: i32, out: &mut [u16]) {
        if let Some(p) = &self.profile {
            const S: f32 = 1.0 / 65535.0;
            let o = p.apply([r as f32 * S, g as f32 * S, b as f32 * S]);
            for (d, v) in out.iter_mut().take(3).zip(o) {
                // NaN maps to 0.
                *d = (v * 65535.0 + 0.5).clamp(0.0, 65535.0) as u16;
            }
            return;
        }
        let m = &self.m;
        for (i, o) in out.iter_mut().take(3).enumerate() {
            let v = (m[i][0] as i64 * r as i64
                + m[i][1] as i64 * g as i64
                + m[i][2] as i64 * b as i64
                + 2048)
                >> 12;
            *o = v.clamp(0, 65535) as u16;
        }
    }
}

/// Grey-world white balance from black-subtracted data (sparse sample).
fn grey_world(raw: &RawImage, colors: &dyn Fn(usize, usize) -> (usize, usize)) -> [f32; 3] {
    let cpp = raw.cpp as usize;
    let win = raw.window;
    let ww = win.w as usize;
    // Only what the decoded window holds (all of the crop for a whole decode).
    let c = intersect(raw.crop, win);
    let lin = |v: u16| -> f32 {
        match &raw.linearization {
            Some(t) if !t.is_empty() => t[(v as usize).min(t.len() - 1)] as f32,
            _ => v as f32,
        }
    };
    let mut sum = [0f64; 3];
    let mut cnt = [0u64; 3];
    let step = ((c.h as usize / 256).max(1)) & !1;
    let step = step.max(2);
    for y in (c.y as usize..(c.y + c.h) as usize).step_by(step) {
        for yy in [y, y + 1] {
            if yy >= (c.y + c.h) as usize {
                continue;
            }
            for x in (c.x as usize..(c.x + c.w) as usize).step_by(3) {
                for s in 0..cpp {
                    let (col, k) = if cpp == 3 { (s, s) } else { colors(yy, x) };
                    let i = (yy - win.y as usize) * ww + x - win.x as usize;
                    let v = lin(raw.data[i * cpp + s]) - raw.black[k];
                    let lim = raw.white - raw.black[k];
                    if v > 0.0 && v < lim * 0.95 {
                        sum[col] += v as f64;
                        cnt[col] += 1;
                    }
                }
            }
        }
    }
    let m: [f64; 3] = std::array::from_fn(|i| {
        if cnt[i] > 0 {
            sum[i] / cnt[i] as f64
        } else {
            0.0
        }
    });
    if m.iter().all(|&v| v > 0.0) {
        [(m[1] / m[0]) as f32, 1.0, (m[1] / m[2]) as f32]
    } else {
        [1.0; 3]
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a.x.saturating_add(a.w).min(b.x.saturating_add(b.w));
    let y1 = a.y.saturating_add(a.h).min(b.y.saturating_add(b.h));
    Rect {
        x: x0,
        y: y0,
        w: x1.saturating_sub(x0),
        h: y1.saturating_sub(y0),
    }
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x as u64 + inner.w as u64 <= outer.x as u64 + outer.w as u64
        && inner.y as u64 + inner.h as u64 <= outer.y as u64 + outer.h as u64
}

/// Mirror `i` into `0..n` the way the full-size demosaic pads the crop.
#[inline(always)]
fn reflect(i: isize, n: usize) -> usize {
    let n = n as isize;
    let mut i = i;
    if i < 0 {
        i = -i;
    }
    if i >= n {
        i = 2 * n - 2 - i;
    }
    i.clamp(0, n - 1) as usize
}

/// Output size of [`develop`] for a crop.
pub(crate) fn output_size(crop: Rect, size: OutputSize) -> (u32, u32) {
    match size {
        OutputSize::Half => ((crop.w / 2).max(1), (crop.h / 2).max(1)),
        OutputSize::Full => (crop.w, crop.h),
    }
}

/// The output region `opts.region` asks for, clipped to the output; `None` when it is empty.
pub(crate) fn output_region(crop: Rect, opts: &DecodeOptions) -> Option<Rect> {
    let (ow, oh) = output_size(crop, opts.size);
    let all = Rect {
        x: 0,
        y: 0,
        w: ow,
        h: oh,
    };
    let r = opts.region.map_or(all, |r| intersect(r, all));
    (r.w > 0 && r.h > 0).then_some(r)
}

/// The sensor samples (sensor coordinates) [`develop`] reads for the non-empty output region
/// `r` of a 2x2-pattern or linear image with a non-empty crop: the region plus the demosaic's
/// two-pixel margin, mirrored at the crop's edges, at full size; the 2x2 blocks behind it at
/// half size.
pub(crate) fn needed_window(crop: Rect, r: Rect, size: OutputSize) -> Rect {
    let span = |o: u32, n: u32, c: u32| -> (u32, u32) {
        // `o..o + n` in output units, `c` the crop extent along this axis.
        match size {
            OutputSize::Half => {
                let a = (2 * o as u64).min(c as u64 - 1) as u32;
                let b = (2 * (o as u64 + n as u64)).clamp(a as u64 + 1, c as u64) as u32;
                (a, b)
            }
            OutputSize::Full => {
                let (mut a, mut b) = (u32::MAX, 0);
                for i in o as isize - 2..(o + n) as isize + 2 {
                    let v = reflect(i, c as usize) as u32;
                    a = a.min(v);
                    b = b.max(v + 1);
                }
                (a, b)
            }
        }
    };
    let (x0, x1) = span(r.x, r.w, crop.w);
    let (y0, y1) = span(r.y, r.h, crop.h);
    Rect {
        x: crop.x + x0,
        y: crop.y + y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

/// What [`develop`] derives from all of a [`RawImage`] before developing any pixel: the white
/// balance (as shot, or a grey-world estimate over the decoded data) and the LUT length (the
/// largest stored sample). [`DevelopStats::of`] a whole-sensor image, kept, lets
/// [`develop_with`] develop region after region of it without scanning it again, each equal to
/// the same pixels of the whole develop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevelopStats {
    /// Multipliers for R, G, B.
    pub wb: [f32; 3],
    pub wb_source: WbSource,
    /// Entries of the per-position (X-Trans: per-colour) LUTs.
    lut_len: usize,
    /// The white level the LUTs scale to: `raw.white`, or the level the data shows it clips
    /// at (see [`Highlights`]).
    white: f32,
    /// Per LUT (2x2 position, linear channel or X-Trans colour): samples at or above this
    /// are clipped and map to full scale whatever the white balance (`u32::MAX`: no clipping
    /// seen).
    clip: [u32; 4],
}

/// Samples that make a clipping pile-up, at the least.
const PILE: u64 = 4;
/// The band below the top value a pile-up is measured in: this fraction of the range.
const PILE_BAND: f32 = 128.0;
/// A pile-up's first code holds at least this many times the samples per code of the band
/// below it.
const PILE_RATIO: u64 = 4;
/// Samples per LUT below which the histogram is not used (the top value's count is).
const PILE_MIN_SAMPLES: u64 = 4096;
/// Bands below the top value a pile-up may start in, plus one it is measured against.
const PILE_SPAN: u16 = 5;

/// How the data clips, so clipped highlights stay white.
///
/// The stated white level is not always where a sensor clips: Canon's bodies clip at one
/// level per ISO (the EOS 6D at ISO 100 at 15024 for a table level of 15490), and channels
/// can clip a few units apart (EOS 300D: 4002 and 4007). A white level above the clip leaves
/// a clipped green below full scale while red and blue, multiplied by their white balance
/// gains, reach it: magenta highlights. So, per LUT, a pile-up in the top quarter of the
/// range is a clip: samples at or above its lowest code map to full scale, and the highest
/// top value becomes the white level (as LibRaw's default lowers its maximum to the data's).
///
/// A pile-up starts at a code within four bands of [`PILE_BAND`] of the range below the top
/// value that stands out from the band below it (see [`pile_start`]). That takes one value
/// (Canon; the DJI FC220 file's 19 samples at 16380 with 3 in the 380 codes below), a clip
/// spread by noise (the Olympus E-PL1 file: about 20 000 samples per channel at 3928–3958,
/// 70 in the band below, 5 at the top value) and one with a tail above it (Sony ARW2 codes
/// in steps of 32: the NEX-5N piles up at 16340–16404, then 116 samples reach 16596) alike,
/// and leaves data running evenly up to its largest value. A fixed count at the top value
/// alone took the E-PL1's 5 for a clip at 3958.
struct Highlights {
    white: f32,
    clip: [u32; 4],
    /// The largest visible sample.
    top: u16,
}

impl Highlights {
    /// `index` gives the LUT of sample (y, x); it repeats every `period` columns.
    fn of(
        raw: &RawImage,
        luts: usize,
        period: usize,
        index: &(dyn Fn(usize, usize) -> usize + Sync),
    ) -> Self {
        let tops = top_samples(raw, period, index);
        let none = Highlights {
            white: raw.white,
            clip: [u32::MAX; 4],
            top: tops.iter().map(|t| t.0).max().unwrap_or(0),
        };
        // Linearized samples (DNG LinearizationTable): the pile-up is found on the stored
        // values and its level read through the table. Leica's M8 and M9 store 8-bit codes
        // whose top maps to 16256 under a white of 16383: clipped green stayed below full
        // scale, so blown highlights showed magenta (fidelity round 4, raw.pixls.us).
        let table = raw.linearization.as_ref().filter(|t| !t.is_empty());
        let linear = |m: u16| -> f32 {
            table.map_or(f32::from(m), |t| {
                f32::from(t[usize::from(m).min(t.len() - 1)])
            })
        };
        let black = raw.black.iter().copied().fold(f32::INFINITY, f32::min);
        let floor = black + 0.75 * (raw.white - black);
        let mut out = none;
        let mut white = None::<f32>;
        let band = ((raw.white - black) / PILE_BAND).max(1.0) as u16;
        let c = intersect(raw.crop, raw.window);
        let per_lut = u64::from(c.w) * u64::from(c.h) * u64::from(raw.cpp) / luts.max(1) as u64;
        let mut low = [None::<u16>; 4];
        for (k, &(m, n)) in tops.iter().enumerate().take(luts) {
            let level = linear(m);
            if m == 0 || level < floor {
                continue;
            }
            if table.is_some() || m <= PILE_SPAN * band || per_lut < PILE_MIN_SAMPLES {
                // Linearized codes (the band would be in other units), tiny ranges and too
                // few samples for a histogram (small regions and test images): the count at
                // the top value alone.
                if u64::from(n) >= PILE {
                    out.clip[k] = u32::from(m);
                    white = Some(white.map_or(level, |w| w.max(level)));
                }
            } else {
                low[k] = Some(m - PILE_SPAN * band);
            }
        }
        if low.iter().any(Option::is_some) {
            let hist = top_histograms(raw, period, index, &low, &tops);
            for (k, h) in hist.iter().enumerate() {
                let Some(lo) = low[k] else {
                    continue;
                };
                if let Some(c) = pile_start(h, usize::from(band)) {
                    out.clip[k] = u32::from(lo) + c as u32;
                    let level = linear(tops[k].0);
                    white = Some(white.map_or(level, |w| w.max(level)));
                }
            }
        }
        if let Some(w) = white {
            out.white = w;
        }
        out
    }
}

/// The offset in `h` (counts per code from `PILE_SPAN` bands below the top value up to it)
/// where a pile-up starts, if there is one (see [`Highlights`]): the lowest code, within
/// `PILE_SPAN - 1` bands of the top, holding at least 2 samples ([`PILE`] with an empty band
/// below) and [`PILE_RATIO`] times as
/// many as the codes in use in the band below it hold on average, with [`PILE`] samples from
/// it up to the top. Codes in use, not all codes: Sony's ARW2 curve leaves 31 of every 32
/// codes empty near the top.
fn pile_start(h: &[u32], band: usize) -> Option<usize> {
    let band = band.max(1);
    let (mut sum, mut used) = (vec![0u64], vec![0u64]);
    for &v in h {
        sum.push(sum.last().copied().unwrap_or(0) + u64::from(v));
        used.push(used.last().copied().unwrap_or(0) + u64::from(v > 0));
    }
    let total = sum[h.len()];
    (band..h.len()).find(|&c| {
        let below = sum[c] - sum[c - band];
        let n = (used[c] - used[c - band]).max(1);
        // With nothing below, a pile-up's first code needs PILE samples itself.
        let per_code = if below == 0 {
            PILE
        } else {
            (PILE_RATIO * below).div_ceil(n).max(2)
        };
        u64::from(h[c]) >= per_code && total - sum[c] >= PILE
    })
}

/// The visible samples of row `y` of `raw` and their LUT indices: one per sample of a
/// period that repeats along the row (`index` repeats every `period` columns; linear RGB:
/// the channel).
fn row_samples<'a>(
    raw: &'a RawImage,
    y: usize,
    period: usize,
    index: &(dyn Fn(usize, usize) -> usize + Sync),
) -> (&'a [u16], ([usize; 16], usize)) {
    let cpp = raw.cpp as usize;
    let win = raw.window;
    let c = intersect(raw.crop, win);
    let (x0, x1) = (c.x as usize, (c.x + c.w) as usize);
    let row = (y - win.y as usize) * win.w as usize;
    let a = (row + x0 - win.x as usize) * cpp;
    let b = (row + x1 - win.x as usize) * cpp;
    let mut idx = [0usize; 16];
    let p = if cpp == 3 {
        idx[..3].copy_from_slice(&[0, 1, 2]);
        3
    } else {
        let p = period.clamp(1, 16);
        for (i, e) in idx.iter_mut().enumerate().take(p) {
            *e = index(y, x0 + i) & 3;
        }
        p
    };
    (&raw.data[a..b], (idx, p))
}

/// Per LUT index with a `low` code, the count of each code from `low` to the LUT's top value.
fn top_histograms(
    raw: &RawImage,
    period: usize,
    index: &(dyn Fn(usize, usize) -> usize + Sync),
    low: &[Option<u16>; 4],
    tops: &[(u16, u32); 4],
) -> [Vec<u32>; 4] {
    let c = intersect(raw.crop, raw.window);
    let lo = low.map(|l| l.unwrap_or(u16::MAX));
    let lens: [usize; 4] =
        std::array::from_fn(|k| low[k].map_or(0, |l| usize::from(tops[k].0 - l) + 1));
    let empty = || -> [Vec<u32>; 4] { std::array::from_fn(|k| vec![0u32; lens[k]]) };
    (c.y as usize..(c.y + c.h) as usize)
        .into_par_iter()
        .fold(empty, |mut t, y| {
            let (row, (idx, p)) = row_samples(raw, y, period, index);
            let lo_p: [u16; 16] = std::array::from_fn(|i| lo[idx[i]]);
            let count = |t: &mut [Vec<u32>; 4], i: usize, v: u16| {
                if v >= lo_p[i] {
                    let k = idx[i];
                    if let Some(e) = t[k].get_mut(usize::from(v - lo[k])) {
                        *e += 1;
                    }
                }
            };
            let mut it = row.chunks_exact(p);
            for ch in &mut it {
                for (i, &v) in ch.iter().enumerate() {
                    count(&mut t, i, v);
                }
            }
            for (i, &v) in it.remainder().iter().enumerate() {
                count(&mut t, i, v);
            }
            t
        })
        .reduce(empty, |mut a, b| {
            for (a, b) in a.iter_mut().zip(b) {
                for (a, b) in a.iter_mut().zip(b) {
                    *a += b;
                }
            }
            a
        })
}

/// Per phase of `row` (period `p`), the largest sample and how many have it (0 and the
/// count of zeros for a phase of zeros, as the per-sample scan gives). Two passes over the
/// row (in the cache by then), each one the compiler can vectorise.
fn phase_tops(row: &[u16], p: usize) -> [(u16, u32); 16] {
    let mut m = [(0u16, 0u32); 16];
    if p == 2 {
        let (mut a, mut b) = (0u16, 0u16);
        let mut it = row.chunks_exact(2);
        for ch in &mut it {
            a = a.max(ch[0]);
            b = b.max(ch[1]);
        }
        let rest = it.remainder().first().copied();
        if let Some(v) = rest {
            a = a.max(v);
        }
        let (mut na, mut nb) = (0u32, 0u32);
        for ch in row.chunks_exact(2) {
            na += u32::from(ch[0] == a);
            nb += u32::from(ch[1] == b);
        }
        if rest == Some(a) {
            na += 1;
        }
        m[0] = (a, na);
        m[1] = (b, nb);
        return m;
    }
    let mut it = row.chunks_exact(p);
    for ch in &mut it {
        for (e, &v) in m.iter_mut().zip(ch) {
            e.0 = e.0.max(v);
        }
    }
    for (e, &v) in m.iter_mut().zip(it.remainder()) {
        e.0 = e.0.max(v);
    }
    let mut it = row.chunks_exact(p);
    for ch in &mut it {
        for (e, &v) in m.iter_mut().zip(ch) {
            e.1 += u32::from(v == e.0);
        }
    }
    for (e, &v) in m.iter_mut().zip(it.remainder()) {
        e.1 += u32::from(v == e.0);
    }
    m
}

/// The largest sample `raw` holds outside its visible part (masked borders).
fn border_max(raw: &RawImage) -> u16 {
    let cpp = raw.cpp as usize;
    let win = raw.window;
    let c = intersect(raw.crop, win);
    let ww = win.w as usize * cpp;
    let n = win.h as usize * ww;
    let mut m = raw
        .data
        .get(n..)
        .map_or(0, |t| t.iter().copied().max().unwrap_or(0));
    for (r, row) in raw.data[..n.min(raw.data.len())]
        .chunks(ww.max(1))
        .enumerate()
    {
        let y = win.y as usize + r;
        if y < c.y as usize || y >= (c.y + c.h) as usize || c.w == 0 {
            m = m.max(row.iter().copied().max().unwrap_or(0));
        } else {
            let a = (c.x - win.x) as usize * cpp;
            let b = a + c.w as usize * cpp;
            for &v in row[..a].iter().chain(&row[b.min(row.len())..]) {
                m = m.max(v);
            }
        }
    }
    m
}

/// Per LUT index, the largest sample in the visible part `raw` holds and how many samples
/// have it.
fn top_samples(
    raw: &RawImage,
    period: usize,
    index: &(dyn Fn(usize, usize) -> usize + Sync),
) -> [(u16, u32); 4] {
    let c = intersect(raw.crop, raw.window);
    let merge = |mut a: [(u16, u32); 4], b: [(u16, u32); 4]| {
        for (a, b) in a.iter_mut().zip(b) {
            if b.0 > a.0 {
                *a = b;
            } else if b.0 == a.0 {
                a.1 += b.1;
            }
        }
        a
    };
    (c.y as usize..(c.y + c.h) as usize)
        .into_par_iter()
        .map(|y| {
            let (row, (idx, p)) = row_samples(raw, y, period, index);
            // Per phase the top and its count in one pass, then per LUT.
            let m = phase_tops(row, p);
            let mut t = [(0u16, 0u32); 4];
            for (i, &(v, n)) in m.iter().enumerate().take(p) {
                let e = &mut t[idx[i]];
                if v > e.0 {
                    *e = (v, n);
                } else if v == e.0 {
                    e.1 += n;
                }
            }
            t
        })
        .reduce(|| [(0, 0); 4], merge)
}

/// The sample pattern of a checked [`RawImage`].
#[derive(Clone, Copy)]
enum Pattern<'a> {
    /// Linear RGB (3 samples per pixel).
    Linear,
    /// 2x2 CFA: colour per position `(y & 1) * 2 + (x & 1)`.
    Bayer([u8; 4]),
    /// X-Trans and other supported non-2x2 RGB patterns.
    Other(&'a crate::Cfa),
}

/// Checks `raw`'s buffer and crop; its pattern.
fn checked(raw: &RawImage) -> Result<Pattern<'_>> {
    let (w, h, cpp) = (raw.width as usize, raw.height as usize, raw.cpp as usize);
    let win = raw.window;
    let sensor = Rect {
        x: 0,
        y: 0,
        w: raw.width,
        h: raw.height,
    };
    if !(cpp == 1 || cpp == 3)
        || !contains(sensor, win)
        || (raw.data.len() as u64) < win.w as u64 * win.h as u64 * cpp as u64
    {
        return Err(RawError::Malformed("raw image buffer"));
    }
    let crop = raw.crop;
    if crop.w == 0
        || crop.h == 0
        || (crop.x as u64 + crop.w as u64) > w as u64
        || (crop.y as u64 + crop.h as u64) > h as u64
    {
        return Err(RawError::Malformed("crop outside the sensor"));
    }
    if cpp == 3 {
        return Ok(Pattern::Linear);
    }
    let cfa = raw
        .cfa
        .as_ref()
        .ok_or(RawError::Unsupported("CFA image without pattern"))?;
    match cfa.bayer2x2() {
        Some(p) => Ok(Pattern::Bayer(p)),
        None if crate::xtrans::supported(cfa) => Ok(Pattern::Other(cfa)),
        None => Err(RawError::Unsupported("non-Bayer CFA")),
    }
}

impl DevelopStats {
    /// The statistics [`develop`] uses for `raw` (over the data it holds).
    pub fn of(raw: &RawImage) -> Result<DevelopStats> {
        let pattern = checked(raw)?;
        Ok(Self::with(raw, pattern))
    }

    /// The white level the develop scales to: the file's, or where the data shows it clips.
    pub fn white(&self) -> f32 {
        self.white
    }

    pub(crate) fn lut_len(&self) -> usize {
        self.lut_len
    }

    /// The sample value from which LUT `k`'s samples count as clipped.
    pub(crate) fn clip(&self, k: usize) -> usize {
        self.clip.get(k).map_or(usize::MAX, |&c| c as usize)
    }

    fn with(raw: &RawImage, pattern: Pattern) -> DevelopStats {
        let estimate = || match pattern {
            Pattern::Other(cfa) => grey_world(raw, &|y: usize, x: usize| {
                (
                    cfa.color(y as u32, x as u32) as usize,
                    (y & 1) * 2 + (x & 1),
                )
            }),
            Pattern::Bayer(p) => grey_world(raw, &|y: usize, x: usize| {
                let k = (y & 1) * 2 + (x & 1);
                (p[k] as usize, k)
            }),
            Pattern::Linear => grey_world(raw, &|_, _| (0, 0)),
        };
        let (wb, wb_source) = match raw.wb {
            Some(wb) if wb.iter().all(|v| v.is_finite() && *v > 0.0) => (wb, WbSource::AsShot),
            _ => (estimate(), WbSource::Estimated),
        };
        let hl = match pattern {
            Pattern::Other(cfa) => {
                Highlights::of(raw, 3, cfa.width as usize, &|y: usize, x: usize| {
                    cfa.color(y as u32, x as u32) as usize
                })
            }
            Pattern::Bayer(_) => {
                Highlights::of(raw, 4, 2, &|y: usize, x: usize| (y & 1) * 2 + (x & 1))
            }
            Pattern::Linear => Highlights::of(raw, 3, 1, &|_, _| 0),
        };
        // The largest stored sample: the visible part's (from the highlight scan) or the
        // masked borders'.
        let lut_len = match &raw.linearization {
            Some(t) if !t.is_empty() => 65536,
            _ => (raw.white.max(1.0) as usize + 2)
                .max(usize::from(hl.top.max(border_max(raw))) + 1)
                .min(65536),
        };
        DevelopStats {
            wb,
            wb_source,
            lut_len,
            white: hl.white,
            clip: hl.clip,
        }
    }
}

/// Develop a [`RawImage`] to linear Rec.709 RGB (16-bit, white = 65535).
///
/// With [`DecodeOptions::region`] only that part of the output is developed, the same, sample
/// for sample, as the corresponding part of the whole develop. For a 2x2 pattern or linear
/// data `raw` may hold just the sensor window behind it (as a region
/// [`crate::RawFile::decode_raw`] returns); the white balance must then be known, since a
/// grey-world estimate only sees the decoded window. Other patterns (X-Trans) need the whole
/// sensor data.
pub fn develop(raw: &RawImage, opts: &DecodeOptions) -> Result<RgbImage> {
    let pattern = checked(raw)?;
    develop_checked(raw, pattern, &DevelopStats::with(raw, pattern), opts)
}

/// [`develop`] with statistics computed before by [`DevelopStats::of`] on all of `raw`. For a
/// whole-sensor image any region then equals the same pixels of its whole develop, whatever
/// the white balance source, and costs only its own pixels. Statistics of another image give
/// wrong colours, never a panic.
pub fn develop_with(
    raw: &RawImage,
    stats: &DevelopStats,
    opts: &DecodeOptions,
) -> Result<RgbImage> {
    let pattern = checked(raw)?;
    develop_checked(raw, pattern, stats, opts)
}

fn develop_checked(
    raw: &RawImage,
    pattern: Pattern,
    stats: &DevelopStats,
    opts: &DecodeOptions,
) -> Result<RgbImage> {
    let crop = raw.crop;
    let (wb, wb_source) = (stats.wb, stats.wb_source);
    // Normalise so the smallest multiplier is 1: all channels clip together (white highlights).
    let mn = wb.iter().copied().fold(f32::INFINITY, f32::min).max(1e-6);
    let mul = wb.map(|v| v / mn);
    let luts = match pattern {
        Pattern::Other(cfa) => crate::xtrans::luts(raw, cfa, mul, stats),
        Pattern::Bayer(p) => luts(raw, Some(p), mul, stats),
        Pattern::Linear => luts(raw, None, mul, stats),
    };
    let mat = Matrix::of(raw);
    let reg = output_region(crop, opts).ok_or(RawError::Malformed("empty output region"))?;
    let data = match raw.opcodes.as_deref().filter(|o| o.has_stage3()) {
        None => develop_region(raw, pattern, &luts, &mat, reg, opts)?,
        Some(ops) => {
            // OpcodeList3 between the demosaic and the colour transform: tiles of white
            // balanced camera RGB (an exact identity in Q12), then `mat`.
            let camera = Matrix::new(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
            let tile_opts = DecodeOptions {
                parallel: false,
                cancel: None,
                ..*opts
            };
            let base = |r: Rect| develop_region(raw, pattern, &luts, &camera, r, &tile_opts);
            let finish = |row: &mut [u16]| {
                for px in row.chunks_exact_mut(3) {
                    let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
                    mat.apply(r, g, b, px);
                }
            };
            ops.develop_stage3(crop, reg, mul, opts, &base, &finish)?
        }
    };
    Ok(RgbImage {
        width: reg.w,
        height: reg.h,
        data,
        orientation: raw.orientation,
        matrix_source: raw.matrix_source,
        wb_source,
        truncated: raw.truncated,
    })
}

/// LUTs of a 2x2 pattern `pattern` (4 CFA positions) or linear data (`None`, 3 channels):
/// linearize, subtract black, scale to white, white balance by `mul` and clip; samples the
/// data shows clipped map to full scale.
fn luts(
    raw: &RawImage,
    pattern: Option<[u8; 4]>,
    mul: [f32; 3],
    stats: &DevelopStats,
) -> Vec<Vec<u16>> {
    let cpp = raw.cpp as usize;
    let nlut = if cpp == 1 { 4 } else { 3 };
    let lut_len = stats.lut_len.clamp(1, 65536);
    (0..nlut)
        .map(|k| {
            let col = if cpp == 1 {
                pattern.map_or(GREEN, |p| p[k]) as usize
            } else {
                k
            };
            let black = raw.black[k];
            let range = (stats.white - black).max(1.0);
            let scale = mul[col] / range * 65535.0;
            let clip = stats.clip(k);
            (0..lut_len)
                .map(|v| {
                    if v >= clip {
                        return 65535;
                    }
                    let lin = match &raw.linearization {
                        Some(t) if !t.is_empty() => t[v.min(t.len() - 1)] as f32,
                        _ => v as f32,
                    };
                    ((lin - black) * scale).clamp(0.0, 65535.0).round() as u16
                })
                .collect()
        })
        .collect()
}

/// Develops output region `reg` (non-empty, inside the output) of `raw` through `luts` and
/// the colour transform `mat`.
fn develop_region(
    raw: &RawImage,
    pattern: Pattern,
    luts: &[Vec<u16>],
    mat: &Matrix,
    reg: Rect,
    opts: &DecodeOptions,
) -> Result<Vec<u16>> {
    let cpp = raw.cpp as usize;
    let win = raw.window;
    let crop = raw.crop;
    let pattern = match pattern {
        Pattern::Linear => None,
        Pattern::Bayer(p) => Some(p),
        Pattern::Other(cfa) => {
            let sensor = Rect {
                x: 0,
                y: 0,
                w: raw.width,
                h: raw.height,
            };
            if win != sensor {
                return Err(RawError::Unsupported(
                    "partial sensor data with a non-2x2 pattern",
                ));
            }
            let wb_source = WbSource::AsShot;
            return Ok(crate::xtrans::develop(raw, cfa, luts, mat, wb_source, reg, opts)?.data);
        }
    };
    let lut = |k: usize, v: u16| -> i32 {
        let l = &luts[k];
        l[(v as usize).min(l.len() - 1)] as i32
    };
    let cancel = || opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed));

    let half = opts.size == OutputSize::Half;
    if !contains(win, needed_window(crop, reg, opts.size)) {
        return Err(RawError::Malformed(
            "output region outside the decoded sensor data",
        ));
    }
    let (rx, ry) = (reg.x as usize, reg.y as usize);
    let (ow, oh) = (reg.w as usize, reg.h as usize);
    let mut out = vec![0u16; ow * oh * 3];
    let (cx, cy) = (crop.x as usize, crop.y as usize);
    let (cw, ch) = (crop.w as usize, crop.h as usize);
    // Samples are addressed in sensor coordinates; `data` starts at the window's corner.
    let ww = win.w as usize;
    let base0 = win.y as usize * ww + win.x as usize;
    let sample = |y: usize, x: usize, s: usize| raw.data[(y * ww + x - base0) * cpp + s];

    let band = |(bi, chunk): (usize, &mut [u16])| -> Result<()> {
        if cancel() {
            return Err(RawError::Cancelled);
        }
        let rows = chunk.len() / (ow * 3);
        let oy0 = ry + bi * BAND;
        if cpp == 3 {
            for r in 0..rows {
                let oy = oy0 + r;
                for j in 0..ow {
                    let ox = rx + j;
                    let mut v = [0i32; 3];
                    if half {
                        let n = |a: usize, lim: usize| (a * 2 + 1).min(lim - 1);
                        for (s, vs) in v.iter_mut().enumerate() {
                            let (y0, x0) = (cy + oy * 2, cx + ox * 2);
                            let (y1, x1) = (cy + n(oy, ch), cx + n(ox, cw));
                            *vs = (lut(s, sample(y0, x0, s))
                                + lut(s, sample(y0, x1, s))
                                + lut(s, sample(y1, x0, s))
                                + lut(s, sample(y1, x1, s))
                                + 2)
                                >> 2;
                        }
                    } else {
                        for (s, vs) in v.iter_mut().enumerate() {
                            *vs = lut(s, sample(cy + oy, cx + ox, s));
                        }
                    }
                    mat.apply(v[0], v[1], v[2], &mut chunk[(r * ow + j) * 3..]);
                }
            }
            return Ok(());
        }
        let p = pattern.unwrap_or([RED, GREEN, GREEN, BLUE]);
        if half {
            for r in 0..rows {
                let y = cy + (oy0 + r) * 2;
                let y1 = (y + 1).min(cy + ch - 1);
                for j in 0..ow {
                    let x = cx + (rx + j) * 2;
                    let x1 = (x + 1).min(cx + cw - 1);
                    let mut acc = [0i32; 3];
                    let mut n = [0i32; 3];
                    for (yy, xx) in [(y, x), (y, x1), (y1, x), (y1, x1)] {
                        let k = (yy & 1) * 2 + (xx & 1);
                        let c = p[k] as usize;
                        acc[c] += lut(k, sample(yy, xx, 0));
                        n[c] += 1;
                    }
                    let v: [i32; 3] =
                        std::array::from_fn(|c| if n[c] > 0 { acc[c] / n[c] } else { 0 });
                    mat.apply(v[0], v[1], v[2], &mut chunk[(r * ow + j) * 3..]);
                }
            }
            return Ok(());
        }
        // Full size: padded, normalised plane for rows oy0-2 .. oy0+rows+2 and columns
        // rx-2 .. rx+ow+2.
        let pw = ow + 4;
        let ph = rows + 4;
        let mut plane = vec![0i32; pw * ph];
        let (wx, wy) = (win.x as usize, win.y as usize);
        // Columns `lo..hi` of the plane need no mirroring.
        let lo = 2usize.saturating_sub(rx).min(pw);
        let hi = (cw + 2).saturating_sub(rx).clamp(lo, pw);
        for py in 0..ph {
            let y = cy + reflect(oy0 as isize + py as isize - 2, ch);
            // The window row `y` and this row's two LUTs (even and odd columns).
            let src = &raw.data[(y - wy) * ww..(y - wy + 1) * ww];
            let (le, lo_) = (&luts[(y & 1) * 2], &luts[(y & 1) * 2 + 1]);
            let pick = |l: &[u16], v: u16| l[(v as usize).min(l.len() - 1)] as i32;
            let rowp = &mut plane[py * pw..(py + 1) * pw];
            let edge = |px: usize| {
                let x = cx + reflect((rx + px) as isize - 2, cw);
                pick(if x & 1 == 0 { le } else { lo_ }, src[x - wx])
            };
            for px in (0..lo).chain(hi..pw) {
                rowp[px] = edge(px);
            }
            // Interior: sensor columns x0.. in order, LUTs alternating by column parity.
            let x0 = cx + rx + lo - 2;
            let (l0, l1) = if x0 & 1 == 0 { (le, lo_) } else { (lo_, le) };
            let s = &src[x0 - wx..x0 - wx + (hi - lo)];
            let d = &mut rowp[lo..hi];
            let mut dp = d.chunks_exact_mut(2);
            let mut sp = s.chunks_exact(2);
            for (o, v) in (&mut dp).zip(&mut sp) {
                o[0] = pick(l0, v[0]);
                o[1] = pick(l1, v[1]);
            }
            if let ([o], [v]) = (dp.into_remainder(), sp.remainder()) {
                *o = pick(l0, *v);
            }
        }
        // Colour of crop-relative (row, col); reflection preserves parity.
        let color = |oy: usize, ox: usize| p[((cy + oy) & 1) * 2 + ((cx + ox) & 1)];
        for r in 0..rows {
            let oy = oy0 + r;
            let base = (r + 2) * pw + 2;
            let at = |i: usize, dy: isize, dx: isize| {
                plane[(i as isize + dy * pw as isize + dx) as usize]
            };
            let line = &mut chunk[r * ow * 3..(r + 1) * ow * 3];
            for j in 0..ow {
                let ox = rx + j;
                let i = base + j;
                let c0 = at(i, 0, 0);
                let c = color(oy, ox);
                let cross1 = || at(i, -1, 0) + at(i, 1, 0) + at(i, 0, -1) + at(i, 0, 1);
                let cross2 = || at(i, -2, 0) + at(i, 2, 0) + at(i, 0, -2) + at(i, 0, 2);
                let diag = || at(i, -1, -1) + at(i, -1, 1) + at(i, 1, -1) + at(i, 1, 1);
                let horiz = || {
                    (10 * c0 + 8 * (at(i, 0, -1) + at(i, 0, 1))
                        - 2 * diag()
                        - 2 * (at(i, 0, -2) + at(i, 0, 2))
                        + (at(i, -2, 0) + at(i, 2, 0))
                        + 8)
                        >> 4
                };
                let vert = || {
                    (10 * c0 + 8 * (at(i, -1, 0) + at(i, 1, 0))
                        - 2 * diag()
                        - 2 * (at(i, -2, 0) + at(i, 2, 0))
                        + (at(i, 0, -2) + at(i, 0, 2))
                        + 8)
                        >> 4
                };
                let (rr, gg, bb) = if c == GREEN {
                    let h = horiz().clamp(0, 65535);
                    let v = vert().clamp(0, 65535);
                    if color(oy, ox + 1) == RED {
                        (h, c0, v)
                    } else {
                        (v, c0, h)
                    }
                } else {
                    let g = ((8 * c0 + 4 * cross1() - 2 * cross2() + 8) >> 4).clamp(0, 65535);
                    let o = ((12 * c0 + 4 * diag() - 3 * cross2() + 8) >> 4).clamp(0, 65535);
                    if c == RED { (c0, g, o) } else { (o, g, c0) }
                };
                mat.apply(rr, gg, bb, &mut line[j * 3..]);
            }
        }
        Ok(())
    };
    let chunk = BAND * ow * 3;
    if opts.parallel {
        out.par_chunks_mut(chunk).enumerate().try_for_each(band)?;
    } else {
        out.chunks_mut(chunk).enumerate().try_for_each(band)?;
    }
    Ok(out)
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    use crate::{Cfa, MatrixSource};

    /// The scans as first written: every sample through `index`.
    fn reference(
        raw: &RawImage,
        index: &dyn Fn(usize, usize) -> usize,
        low: &[Option<u16>; 4],
    ) -> ([(u16, u32); 4], [Vec<u32>; 4]) {
        let cpp = raw.cpp as usize;
        let win = raw.window;
        let c = intersect(raw.crop, win);
        let mut t = [(0u16, 0u32); 4];
        let each = |f: &mut dyn FnMut(usize, u16)| {
            for y in c.y as usize..(c.y + c.h) as usize {
                for x in c.x as usize..(c.x + c.w) as usize {
                    let i = ((y - win.y as usize) * win.w as usize + x - win.x as usize) * cpp;
                    for s in 0..cpp {
                        let k = if cpp == 3 { s } else { index(y, x) } & 3;
                        f(k, raw.data[i + s]);
                    }
                }
            }
        };
        each(&mut |k, v| {
            let e = &mut t[k];
            if v > e.0 {
                *e = (v, 1);
            } else if v == e.0 {
                e.1 += 1;
            }
        });
        let mut h: [Vec<u32>; 4] = std::array::from_fn(|k| {
            low[k].map_or(Vec::new(), |l| {
                vec![0; usize::from(t[k].0.saturating_sub(l)) + 1]
            })
        });
        each(&mut |k, v| {
            if let Some(l) = low[k]
                && v >= l
                && let Some(e) = h[k].get_mut(usize::from(v - l))
            {
                *e += 1;
            }
        });
        (t, h)
    }

    fn image(w: u32, h: u32, cpp: u32, cfa: Option<Cfa>, seed: u32) -> RawImage {
        let mut s = seed | 1;
        let data = (0..w * h * cpp)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                // A pile near the top, a spread below it.
                if s.is_multiple_of(7) {
                    4000 + (s >> 8) as u16 % 3
                } else {
                    (s >> 4) as u16 % 4000
                }
            })
            .collect();
        RawImage {
            width: w,
            height: h,
            cpp,
            data,
            // A window and crop off the origin, odd sizes.
            window: Rect { x: 0, y: 0, w, h },
            cfa,
            crop: Rect {
                x: 3,
                y: 1,
                w: w - 6,
                h: h - 4,
            },
            black: [0.0; 4],
            white: 4095.0,
            linearization: None,
            wb: None,
            rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            matrix_source: MatrixSource::Generic,
            profile: None,
            orientation: 1,
            truncated: false,
            opcodes: None,
        }
    }

    type Index<'a> = dyn Fn(usize, usize) -> usize + Sync + 'a;

    #[test]
    fn row_scans_equal_per_sample_scans() {
        let xtrans = Cfa::new(
            6,
            6,
            vec![
                1, 1, 0, 1, 1, 2, 1, 1, 2, 1, 1, 0, 2, 0, 1, 0, 2, 1, 1, 1, 2, 1, 1, 0, 1, 1, 0, 1,
                1, 2, 0, 2, 1, 2, 0, 1,
            ],
        )
        .unwrap();
        let bayer = |y: usize, x: usize| (y & 1) * 2 + (x & 1);
        let xt = |y: usize, x: usize| xtrans.color(y as u32, x as u32) as usize;
        let lin = |_: usize, _: usize| 0;
        let low = [Some(3990), Some(3995), None, Some(3000)];
        for (w, h) in [(37, 23), (64, 40), (101, 9)] {
            for seed in [1, 7, 99] {
                let cases: [(RawImage, usize, &Index<'_>); 3] = [
                    (image(w, h, 1, None, seed), 2, &bayer),
                    (image(w, h, 1, Some(xtrans.clone()), seed), 6, &xt),
                    (image(w, h, 3, None, seed), 1, &lin),
                ];
                for (mut raw, period, index) in cases {
                    let (t, _) = reference(&raw, index, &[None; 4]);
                    // The stored maximum from the scan and the borders.
                    let top = t.iter().map(|t| t.0).max().unwrap_or(0);
                    let all = |r: &RawImage| r.data.iter().copied().max().unwrap_or(0);
                    assert_eq!(top.max(border_max(&raw)), all(&raw));
                    let mut hot = raw.clone();
                    let last = hot.data.len() - 1;
                    hot.data[last] = 4095;
                    assert_eq!(border_max(&hot), 4095);
                    raw.data[1] = raw.data[1].max(1);
                    assert_eq!(top_samples(&raw, period, index), t);
                    let lows = std::array::from_fn(|k| low[k].filter(|&l| l <= t[k].0));
                    let want = reference(&raw, index, &lows).1;
                    assert_eq!(top_histograms(&raw, period, index, &lows, &t), want);
                }
            }
        }
    }
}
