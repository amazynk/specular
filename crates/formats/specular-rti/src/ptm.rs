//! HP polynomial texture maps (PTM file format 1.2 of November 2001, and 1.1).
//!
//! An ASCII header (version, format, size, six scales and six integer biases, then
//! format-specific fields) ends with a newline, after which the coefficients follow as bytes,
//! `final = (byte − bias) · scale`. Uncompressed data is stored bottom row first; JPEG
//! planes are ordinary upright images.
//!
//! JPEG files code nine (LRGB) or eighteen (RGB) planes, each optionally predicted from
//! another already decoded plane: the stored image is the difference plus 128 modulo 256,
//! the reference optionally inverted (`255 − v`) first. Each plane's side information then
//! overwrites the pixels the lossy coding wrapped around: five-byte records, a big-endian
//! position in top-down raster order and the original value.

use crate::{Basis, Colour, Container, Error, Layer, Result, Rti, check_size, layer};

const LRGB_PLANES: usize = 9;
const RGB_PLANES: usize = 18;

/// The first bytes of a PTM file.
pub(crate) fn probe(head: &[u8]) -> bool {
    head.starts_with(b"PTM_1.") && head.len() > 7 && head[6].is_ascii_digit()
}

struct Header<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Header<'a> {
    fn token(&mut self) -> Result<&'a str> {
        let d = self.data;
        while self.pos < d.len() && d[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        let start = self.pos;
        while self.pos < d.len() && !d[self.pos].is_ascii_whitespace() && self.pos - start < 64 {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(Error::Truncated("PTM header"));
        }
        std::str::from_utf8(&d[start..self.pos]).map_err(|_| malformed("header is not text"))
    }

    fn number<T: std::str::FromStr>(&mut self, what: &str) -> Result<T> {
        let t = self.token()?;
        t.parse()
            .map_err(|_| malformed(&format!("{what} {t:?} is not a number")))
    }

    fn numbers<T: std::str::FromStr>(&mut self, n: usize, what: &str) -> Result<Vec<T>> {
        (0..n).map(|_| self.number(what)).collect()
    }

    /// Skips the rest of the header line (spaces and one newline) to the binary data.
    fn end_line(&mut self) -> usize {
        let d = self.data;
        while self.pos < d.len() && (d[self.pos] == b' ' || d[self.pos] == b'\r') {
            self.pos += 1;
        }
        if self.pos < d.len() && d[self.pos] == b'\n' {
            self.pos += 1;
        }
        self.pos
    }
}

fn malformed(what: &str) -> Error {
    Error::Malformed(format!("PTM: {what}"))
}

pub(crate) fn read(data: &[u8]) -> Result<Rti> {
    let mut h = Header { data, pos: 0 };
    let version = h.token()?.to_string();
    if version != "PTM_1.2" && version != "PTM_1.1" {
        return Err(Error::Unsupported(format!("PTM version {version}")));
    }
    let format = h.token()?.to_string();
    let width: u32 = h.number("width")?;
    let height: u32 = h.number("height")?;
    check_size(width, height)?;
    let scale: Vec<f32> = h.numbers(6, "scale")?;
    let bias: Vec<i32> = h.numbers(6, "bias")?;
    let scale: [f32; 6] = std::array::from_fn(|i| scale[i]);
    let bias: [i32; 6] = std::array::from_fn(|i| bias[i]);
    let (w, hh) = (width as usize, height as usize);
    let n = w * hh;
    // Up to six layers (RGB coefficients).
    let _layers = crate::reserve_layers(6, width, height)?;

    // Byte-to-value mapping of coefficient k, in display units (0..1 for 0..255).
    let map = |k: usize| (scale[k] / 255.0, -(bias[k] as f32) * scale[k] / 255.0);
    let lrgb_layers = || -> Vec<Layer> {
        let mut out = vec![layer(width, height, [1.0 / 255.0; 3], [0.0; 3])];
        for g in 0..2 {
            let (s, o): (Vec<f32>, Vec<f32>) = (0..3).map(|c| map(3 * g + c)).unzip();
            out.push(layer(width, height, [s[0], s[1], s[2]], [o[0], o[1], o[2]]));
        }
        out
    };
    let rgb_layers = || -> Vec<Layer> {
        (0..6)
            .map(|k| {
                let (s, o) = map(k);
                layer(width, height, [s; 3], [o; 3])
            })
            .collect()
    };
    // Puts coefficient `k` (0..6) or colour channel `k - 6` of an LRGB pixel in place.
    let put_lrgb = |layers: &mut [Layer], k: usize, i: usize, v: u8| {
        let (l, c) = if k < 6 {
            (1 + k / 3, k % 3)
        } else {
            (0, k - 6)
        };
        layers[l].data[i * 3 + c] = v;
    };
    // Row-flipped index: the file's `i`-th pixel bottom-up is this top-down index.
    let flip = |i: usize| (hh - 1 - i / w) * w + i % w;

    let (colour, layers, quality) = match format.as_str() {
        "PTM_FORMAT_LRGB" => {
            let start = h.end_line();
            let need = n * 9;
            let body = data
                .get(start..start + need)
                .ok_or(Error::Truncated("PTM coefficients"))?;
            let mut layers = lrgb_layers();
            if version == "PTM_1.1" {
                for i in 0..n {
                    let o = flip(i);
                    for k in 0..9 {
                        put_lrgb(&mut layers, k, o, body[i * 9 + k]);
                    }
                }
            } else {
                for i in 0..n {
                    let o = flip(i);
                    for k in 0..6 {
                        put_lrgb(&mut layers, k, o, body[i * 6 + k]);
                    }
                    for c in 0..3 {
                        put_lrgb(&mut layers, 6 + c, o, body[n * 6 + i * 3 + c]);
                    }
                }
            }
            (Colour::Lrgb, layers, None)
        }
        "PTM_FORMAT_RGB" => {
            let start = h.end_line();
            let body = data
                .get(start..start + n * 18)
                .ok_or(Error::Truncated("PTM coefficients"))?;
            let mut layers = rgb_layers();
            for c in 0..3 {
                let block = &body[c * n * 6..(c + 1) * n * 6];
                for i in 0..n {
                    let o = flip(i);
                    for (k, l) in layers.iter_mut().enumerate() {
                        l.data[o * 3 + c] = block[i * 6 + k];
                    }
                }
            }
            (Colour::Rgb, layers, None)
        }
        "PTM_FORMAT_PTM_LUT" | "PTM_FORMAT_PTM_C_LUT" => {
            let colour_in_lut = format == "PTM_FORMAT_PTM_C_LUT";
            let mut t = h.token()?;
            if t.eq_ignore_ascii_case("nentries") {
                t = h.token()?;
            }
            let entries: usize = t
                .parse()
                .map_err(|_| malformed("lookup table size is not a number"))?;
            if entries == 0 || entries > 65536 {
                return Err(malformed("lookup table size out of range"));
            }
            let start = h.end_line();
            let per = if colour_in_lut { 9 } else { 6 };
            let lut = data
                .get(start..start + entries * per)
                .ok_or(Error::Truncated("PTM lookup table"))?;
            let wide = entries > 256;
            let ib = if wide { 2 } else { 1 };
            let at = start + entries * per;
            let idx = data
                .get(at..at + n * ib)
                .ok_or(Error::Truncated("PTM lookup indices"))?;
            let rgb = if colour_in_lut {
                &[][..]
            } else {
                data.get(at + n * ib..at + n * ib + n * 3)
                    .ok_or(Error::Truncated("PTM colours"))?
            };
            let mut layers = lrgb_layers();
            for i in 0..n {
                let e = if wide {
                    u16::from_le_bytes([idx[2 * i], idx[2 * i + 1]]) as usize
                } else {
                    idx[i] as usize
                };
                if e >= entries {
                    return Err(malformed("lookup index past the table"));
                }
                let o = flip(i);
                for k in 0..per {
                    put_lrgb(&mut layers, k, o, lut[e * per + k]);
                }
                if !colour_in_lut {
                    for c in 0..3 {
                        put_lrgb(&mut layers, 6 + c, o, rgb[i * 3 + c]);
                    }
                }
            }
            (Colour::Lrgb, layers, None)
        }
        "PTM_FORMAT_JPEG_LRGB" | "PTM_FORMAT_JPEG_RGB" => {
            let lrgb = format == "PTM_FORMAT_JPEG_LRGB";
            let np = if lrgb { LRGB_PLANES } else { RGB_PLANES };
            let quality: i64 = h.number("compression parameter")?;
            let xform: Vec<i64> = h.numbers(np, "transform")?;
            let motion: Vec<i64> = h.numbers(np * 2, "motion vector")?;
            let order: Vec<i64> = h.numbers(np, "order")?;
            let refs: Vec<i64> = h.numbers(np, "reference plane")?;
            let sizes: Vec<u64> = h.numbers(np, "compressed size")?;
            let side: Vec<u64> = h.numbers(np, "side information size")?;
            let mut pos = h.end_line();
            let mut streams = Vec::with_capacity(np);
            for i in 0..np {
                let a = pos;
                let b = a + sizes[i] as usize;
                let c = b + side[i] as usize;
                if c > data.len() {
                    return Err(Error::Truncated("PTM coefficient planes"));
                }
                streams.push((&data[a..b], &data[b..c]));
                pos = c;
            }
            let planes = predict(np, &xform, &motion, &order, &refs, &streams, width, height)?;
            let mut layers = if lrgb { lrgb_layers() } else { rgb_layers() };
            for (p, plane) in planes.iter().enumerate() {
                let (l, c) = if lrgb {
                    if p < 6 {
                        (1 + p / 3, p % 3)
                    } else {
                        (0, p - 6)
                    }
                } else {
                    (p % 6, p / 6)
                };
                for (i, &v) in plane.iter().enumerate() {
                    layers[l].data[i * 3 + c] = v;
                }
            }
            let colour = if lrgb { Colour::Lrgb } else { Colour::Rgb };
            (colour, layers, u32::try_from(quality).ok())
        }
        "PTM_FORMAT_LUM" => return Err(Error::Unsupported("PTM format PTM_FORMAT_LUM".into())),
        f if f.starts_with("PTM_FORMAT_JPEGLS") => {
            return Err(Error::Unsupported(format!("PTM format {f}")));
        }
        f => return Err(Error::Unsupported(format!("PTM format {f:?}"))),
    };
    let planes = if colour == Colour::Lrgb { 9 } else { 18 };
    Ok(Rti {
        width,
        height,
        container: Container::Ptm {
            version,
            format,
            scale,
            bias,
            quality,
        },
        basis: Basis::Ptm {
            relight_order: false,
        },
        colour,
        planes,
        layers,
    })
}

/// Decodes the JPEG planes in their decoding order and undoes the prediction.
#[allow(clippy::too_many_arguments)]
fn predict(
    np: usize,
    xform: &[i64],
    motion: &[i64],
    order: &[i64],
    refs: &[i64],
    streams: &[(&[u8], &[u8])],
    width: u32,
    height: u32,
) -> Result<Vec<Vec<u8>>> {
    let n = width as usize * height as usize;
    let mut planes: Vec<Option<Vec<u8>>> = vec![None; np];
    for rank in 0..np {
        let i = order
            .iter()
            .position(|&o| o == rank as i64)
            .ok_or_else(|| malformed(&format!("no plane has decoding order {rank}")))?;
        if planes[i].is_some() {
            return Err(malformed("decoding order repeats a plane"));
        }
        let (stream, side) = streams[i];
        let mut plane = decode_plane(stream, width, height)?;
        let r = refs[i];
        if r >= 0 {
            let r = r as usize;
            let reference = planes.get(r).and_then(|p| p.as_ref()).ok_or_else(|| {
                malformed(&format!("plane {i} predicted from undecoded plane {r}"))
            })?;
            if xform[i] & 2 != 0 && (motion[i] != 0 || motion[np + i] != 0) {
                return Err(Error::Unsupported("PTM motion compensation".into()));
            }
            let invert = xform[i] & 1 != 0;
            for (v, &rv) in plane.iter_mut().zip(reference) {
                let base = if invert { 255 - rv } else { rv };
                *v = (base as i32 + *v as i32 - 128).rem_euclid(256) as u8;
            }
        }
        for rec in side.chunks_exact(5) {
            let at = u32::from_be_bytes([rec[0], rec[1], rec[2], rec[3]]) as usize;
            if at < n {
                plane[at] = rec[4];
            }
        }
        planes[i] = Some(plane);
    }
    Ok(planes.into_iter().map(|p| p.unwrap_or_default()).collect())
}

/// One coefficient plane: a greyscale JPEG (the luma of a colour one) of the image's size.
pub(crate) fn decode_plane(stream: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let opts = specular_jpeg::DecodeOptions {
        parallelism: specular_jpeg::Parallelism::Single,
        ..Default::default()
    };
    crate::jpeg_fits(stream, width, height)?;
    let d = specular_jpeg::decode(stream, &opts).map_err(|e| Error::Jpeg(e.to_string()))?;
    if d.width != width || d.height != height {
        return Err(malformed(&format!(
            "coefficient plane is {}×{}, the image {width}×{height}",
            d.width, d.height
        )));
    }
    let p = &d.planes[0];
    if d.layout == specular_jpeg::PlanesLayout::Rgba || p.width != width || p.height != height {
        return Err(Error::Unsupported(
            "PTM coefficient plane colour layout".into(),
        ));
    }
    let w = width as usize;
    let mut out = Vec::with_capacity(w * height as usize);
    for y in 0..height as usize {
        out.extend_from_slice(&p.data[y * p.stride..y * p.stride + w]);
    }
    Ok(out)
}
