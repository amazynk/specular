//! Converting reconstructed (unfiltered) PNG rows into packed 8- or 16-bit rows.

use crate::{ColorType, PngInfo, Transparency};

/// Packed row formats the decoder produces internally (and, except for planar RGB, outputs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Packed {
    Gray8,
    Rgb8,
    Rgba8,
    /// Native-endian u16 samples.
    Rgba16,
    /// Native-endian u16 grey (16-bit grey without a tRNS key).
    Gray16,
    /// Native-endian u16 grey and alpha.
    GrayA16,
}

impl Packed {
    pub(crate) fn bytes_per_pixel(self) -> usize {
        match self {
            Packed::Gray8 => 1,
            Packed::Rgb8 => 3,
            Packed::Rgba8 => 4,
            Packed::Rgba16 => 8,
            Packed::Gray16 => 2,
            Packed::GrayA16 => 4,
        }
    }
    pub(crate) fn channels(self) -> usize {
        match self {
            Packed::Gray8 | Packed::Gray16 => 1,
            Packed::GrayA16 => 2,
            Packed::Rgb8 => 3,
            Packed::Rgba8 | Packed::Rgba16 => 4,
        }
    }
    /// Whether the samples are native-endian u16.
    pub(crate) fn is16(self) -> bool {
        matches!(self, Packed::Rgba16 | Packed::Gray16 | Packed::GrayA16)
    }
}

/// 16-bit to 8-bit with rounding: `round(v * 255 / 65535)`.
#[inline(always)]
pub(crate) fn r16(hi: u8, lo: u8) -> u8 {
    ((u32::from(u16::from_be_bytes([hi, lo])) * 255 + 32895) >> 16) as u8
}

/// Row converter for one image (or APNG frame / Adam7 pass, which share the format).
pub(crate) struct Converter {
    ct: ColorType,
    depth: u8,
    target: Packed,
    /// Palette as RGBA (alpha from tRNS); out-of-range indices are opaque black.
    pal: Box<[[u8; 4]; 256]>,
    trns_gray: Option<u16>,
    trns_rgb: Option<[u16; 3]>,
}

impl Converter {
    pub(crate) fn new(info: &PngInfo, target: Packed) -> Converter {
        let mut pal = Box::new([[0, 0, 0, 255]; 256]);
        if let Some(p) = &info.palette {
            for (d, s) in pal.iter_mut().zip(p) {
                d[..3].copy_from_slice(s);
            }
        }
        let (mut trns_gray, mut trns_rgb) = (None, None);
        match &info.transparency {
            Some(Transparency::Palette(a)) if info.color_type == ColorType::Indexed => {
                for (d, &a) in pal.iter_mut().zip(a) {
                    d[3] = a;
                }
            }
            Some(Transparency::Gray(g)) if info.color_type == ColorType::Gray => {
                // Only the low `depth` bits are significant.
                trns_gray = Some(*g & mask16(info.bit_depth));
            }
            Some(Transparency::Rgb(r, g, b)) if info.color_type == ColorType::Rgb => {
                let m = mask16(info.bit_depth);
                trns_rgb = Some([*r & m, *g & m, *b & m]);
            }
            _ => {}
        }
        Converter {
            ct: info.color_type,
            depth: info.bit_depth,
            target,
            pal,
            trns_gray,
            trns_rgb,
        }
    }

    /// Bytes per complete pixel of the raw data (1 for sub-byte depths), the filter unit.
    pub(crate) fn filter_bpp(&self) -> usize {
        (self.ct.channels() * self.depth as usize).div_ceil(8)
    }

    /// Whether raw rows are already in the target format (8-bit gray / RGB / RGBA without a
    /// tRNS colour key), so rows can be unfiltered straight into the output.
    pub(crate) fn identity(&self) -> bool {
        self.depth == 8
            && match (self.ct, self.target) {
                (ColorType::Gray, Packed::Gray8) => self.trns_gray.is_none(),
                (ColorType::Rgb, Packed::Rgb8) => self.trns_rgb.is_none(),
                (ColorType::Rgba, Packed::Rgba8) => true,
                _ => false,
            }
    }

    /// Convert the first `n` pixels of the reconstructed row `raw` into `dst`
    /// (`n * target.bytes_per_pixel()` bytes).
    pub(crate) fn convert(&self, raw: &[u8], n: usize, dst: &mut [u8]) {
        let dst = &mut dst[..n * self.target.bytes_per_pixel()];
        match self.ct {
            ColorType::Indexed => self.indexed(raw, n, dst),
            ColorType::Gray if self.depth < 8 => self.gray_low(raw, n, dst),
            ColorType::Gray => self.gray(raw, n, dst),
            ColorType::GrayAlpha => self.gray_alpha(raw, n, dst),
            ColorType::Rgb => self.rgb(raw, n, dst),
            ColorType::Rgba => self.rgba(raw, n, dst),
        }
    }

    fn put(&self, dst: &mut [u8], i: usize, rgba: [u8; 4]) {
        match self.target {
            Packed::Gray8 => dst[i] = rgba[0],
            Packed::Rgb8 => dst[i * 3..i * 3 + 3].copy_from_slice(&rgba[..3]),
            Packed::Rgba8 => dst[i * 4..i * 4 + 4].copy_from_slice(&rgba),
            Packed::Rgba16 | Packed::Gray16 | Packed::GrayA16 => {
                self.put16(dst, i, rgba.map(|v| u16::from(v) * 257));
            }
        }
    }

    /// Store one pixel in a 16-bit target (grey targets take red as grey: their sources are
    /// grey).
    fn put16(&self, dst: &mut [u8], i: usize, rgba: [u16; 4]) {
        let (n, ch): (usize, &[usize]) = match self.target {
            Packed::Gray16 => (2, &[0]),
            Packed::GrayA16 => (4, &[0, 3]),
            _ => (8, &[0, 1, 2, 3]),
        };
        for (k, &c) in ch.iter().enumerate() {
            dst[i * n + k * 2..i * n + k * 2 + 2].copy_from_slice(&rgba[c].to_ne_bytes());
        }
    }

    fn indexed(&self, raw: &[u8], n: usize, dst: &mut [u8]) {
        let pal = &*self.pal;
        let d = self.depth as usize;
        match (self.target, d) {
            (Packed::Rgba8, 8) => {
                for (o, &i) in dst.chunks_exact_mut(4).zip(&raw[..n]) {
                    o.copy_from_slice(&pal[i as usize]);
                }
            }
            (Packed::Rgb8, 8) => {
                for (o, &i) in dst.chunks_exact_mut(3).zip(&raw[..n]) {
                    o.copy_from_slice(&pal[i as usize][..3]);
                }
            }
            (Packed::Gray8, 8) => {
                for (o, &i) in dst.iter_mut().zip(&raw[..n]) {
                    *o = pal[i as usize][0];
                }
            }
            _ => {
                for i in 0..n {
                    self.put(dst, i, pal[low_sample(raw, i, d) as usize]);
                }
            }
        }
    }

    fn gray_low(&self, raw: &[u8], n: usize, dst: &mut [u8]) {
        let d = self.depth as usize;
        let mul = 255 / ((1u16 << d) - 1) as u8;
        if self.target == Packed::Gray8 {
            for (i, o) in dst.iter_mut().enumerate().take(n) {
                *o = low_sample(raw, i, d) * mul;
            }
            return;
        }
        for i in 0..n {
            let s = low_sample(raw, i, d);
            let a = if self.trns_gray == Some(u16::from(s)) {
                0
            } else {
                255
            };
            let g = s * mul;
            self.put(dst, i, [g, g, g, a]);
        }
    }

    fn gray(&self, raw: &[u8], n: usize, dst: &mut [u8]) {
        let key = self.trns_gray;
        if self.depth == 8 {
            match self.target {
                Packed::Gray8 => dst.copy_from_slice(&raw[..n]),
                Packed::Rgba8 => {
                    for (o, &g) in dst.chunks_exact_mut(4).zip(&raw[..n]) {
                        let a = if key == Some(u16::from(g)) { 0 } else { 255 };
                        o.copy_from_slice(&[g, g, g, a]);
                    }
                }
                _ => {
                    for (i, &g) in raw[..n].iter().enumerate() {
                        let a = if key == Some(u16::from(g)) { 0 } else { 255 };
                        self.put(dst, i, [g, g, g, a]);
                    }
                }
            }
            return;
        }
        if self.target == Packed::Gray16 && key.is_none() {
            for (o, s) in dst.chunks_exact_mut(2).zip(raw[..n * 2].chunks_exact(2)) {
                o.copy_from_slice(&u16::from_be_bytes([s[0], s[1]]).to_ne_bytes());
            }
            return;
        }
        if self.target == Packed::Rgba8 && key.is_none() {
            for (o, s) in dst.chunks_exact_mut(4).zip(raw[..n * 2].chunks_exact(2)) {
                let g = r16(s[0], s[1]);
                o.copy_from_slice(&[g, g, g, 255]);
            }
            return;
        }
        for (i, s) in raw[..n * 2].chunks_exact(2).enumerate() {
            let v = u16::from_be_bytes([s[0], s[1]]);
            if self.target.is16() {
                let a = if key == Some(v) { 0 } else { 0xffff };
                self.put16(dst, i, [v, v, v, a]);
            } else {
                let g = r16(s[0], s[1]);
                let a = if key == Some(v) { 0 } else { 255 };
                self.put(dst, i, [g, g, g, a]);
            }
        }
    }

    fn gray_alpha(&self, raw: &[u8], n: usize, dst: &mut [u8]) {
        if self.depth == 8 {
            if self.target == Packed::Rgba8 {
                for (o, s) in dst.chunks_exact_mut(4).zip(raw[..n * 2].chunks_exact(2)) {
                    o.copy_from_slice(&[s[0], s[0], s[0], s[1]]);
                }
            } else {
                for (i, s) in raw[..n * 2].chunks_exact(2).enumerate() {
                    self.put(dst, i, [s[0], s[0], s[0], s[1]]);
                }
            }
            return;
        }
        if self.target == Packed::GrayA16 {
            for (o, s) in dst.chunks_exact_mut(2).zip(raw[..n * 4].chunks_exact(2)) {
                o.copy_from_slice(&u16::from_be_bytes([s[0], s[1]]).to_ne_bytes());
            }
            return;
        }
        for (i, s) in raw[..n * 4].chunks_exact(4).enumerate() {
            if self.target.is16() {
                let g = u16::from_be_bytes([s[0], s[1]]);
                self.put16(dst, i, [g, g, g, u16::from_be_bytes([s[2], s[3]])]);
            } else {
                let g = r16(s[0], s[1]);
                self.put(dst, i, [g, g, g, r16(s[2], s[3])]);
            }
        }
    }

    fn rgb(&self, raw: &[u8], n: usize, dst: &mut [u8]) {
        let key = self.trns_rgb;
        if self.depth == 8 {
            match self.target {
                Packed::Rgb8 => dst.copy_from_slice(&raw[..n * 3]),
                Packed::Rgba8 if key.is_none() => {
                    for (o, s) in dst.chunks_exact_mut(4).zip(raw[..n * 3].chunks_exact(3)) {
                        o.copy_from_slice(&[s[0], s[1], s[2], 255]);
                    }
                }
                _ => {
                    for (i, s) in raw[..n * 3].chunks_exact(3).enumerate() {
                        let k = [u16::from(s[0]), u16::from(s[1]), u16::from(s[2])];
                        let a = if key == Some(k) { 0 } else { 255 };
                        self.put(dst, i, [s[0], s[1], s[2], a]);
                    }
                }
            }
            return;
        }
        // The usual targets in their own loops: no target match per pixel, and the
        // transparent key tested only when there is one.
        match (self.target, key) {
            (Packed::Rgba16, None) => {
                for (o, s) in dst.chunks_exact_mut(8).zip(raw[..n * 6].chunks_exact(6)) {
                    o[0..2].copy_from_slice(&u16::from_be_bytes([s[0], s[1]]).to_ne_bytes());
                    o[2..4].copy_from_slice(&u16::from_be_bytes([s[2], s[3]]).to_ne_bytes());
                    o[4..6].copy_from_slice(&u16::from_be_bytes([s[4], s[5]]).to_ne_bytes());
                    o[6..8].copy_from_slice(&0xffffu16.to_ne_bytes());
                }
                return;
            }
            (Packed::Rgba8, None) => {
                for (o, s) in dst.chunks_exact_mut(4).zip(raw[..n * 6].chunks_exact(6)) {
                    o.copy_from_slice(&[r16(s[0], s[1]), r16(s[2], s[3]), r16(s[4], s[5]), 255]);
                }
                return;
            }
            _ => {}
        }
        for (i, s) in raw[..n * 6].chunks_exact(6).enumerate() {
            let v = [
                u16::from_be_bytes([s[0], s[1]]),
                u16::from_be_bytes([s[2], s[3]]),
                u16::from_be_bytes([s[4], s[5]]),
            ];
            let keyed = key == Some(v);
            if self.target.is16() {
                self.put16(dst, i, [v[0], v[1], v[2], if keyed { 0 } else { 0xffff }]);
            } else {
                let a = if keyed { 0 } else { 255 };
                self.put(
                    dst,
                    i,
                    [r16(s[0], s[1]), r16(s[2], s[3]), r16(s[4], s[5]), a],
                );
            }
        }
    }

    fn rgba(&self, raw: &[u8], n: usize, dst: &mut [u8]) {
        if self.depth == 8 {
            if self.target == Packed::Rgba8 {
                dst.copy_from_slice(&raw[..n * 4]);
            } else {
                for (i, s) in raw[..n * 4].chunks_exact(4).enumerate() {
                    self.put(dst, i, [s[0], s[1], s[2], s[3]]);
                }
            }
            return;
        }
        match self.target {
            Packed::Rgba16 => {
                for (o, s) in dst.chunks_exact_mut(2).zip(raw[..n * 8].chunks_exact(2)) {
                    o.copy_from_slice(&u16::from_be_bytes([s[0], s[1]]).to_ne_bytes());
                }
            }
            Packed::Rgba8 => {
                for (o, s) in dst.iter_mut().zip(raw[..n * 8].chunks_exact(2)) {
                    *o = r16(s[0], s[1]);
                }
            }
            _ => {
                for (i, s) in raw[..n * 8].chunks_exact(8).enumerate() {
                    let p = [
                        r16(s[0], s[1]),
                        r16(s[2], s[3]),
                        r16(s[4], s[5]),
                        r16(s[6], s[7]),
                    ];
                    self.put(dst, i, p);
                }
            }
        }
    }
}

fn mask16(depth: u8) -> u16 {
    if depth >= 16 {
        0xffff
    } else {
        (1u16 << depth) - 1
    }
}

/// Sample `i` of a row with `d` (1, 2 or 4) bits per sample, or 8.
#[inline(always)]
fn low_sample(raw: &[u8], i: usize, d: usize) -> u8 {
    if d == 8 {
        return raw[i];
    }
    let bit = i * d;
    let shift = 8 - d - (bit & 7);
    (raw[bit >> 3] >> shift) & ((1u8 << d) - 1)
}

/// Split a packed RGB8 row into three planes.
///
/// Written as one zip with no indexing, so the loop vectorises whatever the
/// inliner decides; indexed by position it kept a bounds check per plane,
/// which a profile-guided build left scalar (PNG RGB photos +13% there).
#[inline(never)]
pub(crate) fn deinterleave_rgb(src: &[u8], r: &mut [u8], g: &mut [u8], b: &mut [u8]) {
    let px = src
        .chunks_exact(3)
        .zip(r.iter_mut().zip(g.iter_mut().zip(b)));
    for (s, (r, (g, b))) in px {
        *r = s[0];
        *g = s[1];
        *b = s[2];
    }
}
