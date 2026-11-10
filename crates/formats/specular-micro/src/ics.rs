//! Image Cytometry Standard (ICS 1.0 and 2.0): a text header (`.ics`) and the samples, in
//! a separate `.ids` file (version 1.0, and 2.0 written that way), after the header's `end`
//! line, or at an offset of a `source file`.
//!
//! The header's first two bytes are its field and line separators (usually a tab and a line
//! feed). Each line is a category, a subcategory and values:
//!
//! - `layout parameters / order / sizes`: the dimensions, the first being `bits` (the
//!   sample's storage size) and the others in storage order, fastest first (`x y z t ch` or
//!   any other order and names); `layout significant_bits`.
//! - `representation format / sign / compression / byte_order`: integer, real or complex;
//!   signed or unsigned; uncompressed, gzip or compress (LZW); the byte order as the
//!   significance (1 the least) of each stored byte.
//! - `parameter origin / scale / units / labels`: per dimension (the first for the values).
//! - `sensor`, `history` and anything else: kept as facts for the info panel.
//!
//! Samples are read through each dimension's stride, so any order, any byte order and every
//! sample type (8–64-bit integers, 32/64-bit floats, complex pairs) read the same way.
//! Malformed headers never panic; sizes are checked before anything is allocated.

use std::borrow::Cow;

use crate::{Error, Result};

/// Longest header read (its text, up to `end`).
pub const MAX_HEADER: usize = 1 << 20;
/// Most dimensions past the sample's bits.
const MAX_DIMS: usize = 10;
/// Most header facts kept.
const MAX_FACTS: usize = 4096;

/// Whether `head` starts like an ICS header: two separator bytes then `ics_version`.
pub fn sniff(head: &[u8]) -> bool {
    if head.len() < 13 {
        return false;
    }
    let (fs, ls) = (head[0], head[1]);
    if !(fs == b'\t' || fs == b' ' || fs == b',') || !(ls == b'\n' || ls == b'\r') {
        return false;
    }
    let mut rest = &head[2..];
    while let [b'\r' | b'\n', r @ ..] = rest {
        rest = r;
    }
    rest.starts_with(b"ics_version")
}

/// How the samples are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Integer,
    Real,
    /// Pairs of reals (real, imaginary).
    Complex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Gzip,
    /// Unix `compress` (LZW, `.Z`).
    Compress,
}

impl Compression {
    pub fn name(self) -> &'static str {
        match self {
            Compression::None => "uncompressed",
            Compression::Gzip => "gzip",
            Compression::Compress => "compress (LZW)",
        }
    }
}

/// Where the samples are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataAt {
    /// After the header's `end` line, at this offset of the `.ics` file.
    Inline(usize),
    /// In the `.ids` file beside the header (this name).
    Ids(String),
    /// In a `source file` (its file name only) at an offset.
    Source { file: String, offset: u64 },
}

/// One dimension past the sample's bits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dim {
    /// The `layout order` name, as written (`x`, `ch`, `probe`…).
    pub name: String,
    pub len: usize,
    pub origin: f64,
    pub scale: f64,
    /// As written (`micrometers`, `seconds`, `undefined`).
    pub unit: String,
    /// The `parameter labels` entry, if any.
    pub label: String,
}

/// A parsed ICS header.
#[derive(Clone, Debug)]
pub struct Ics {
    /// `1.0` or `2.0`.
    pub version: String,
    /// Storage bits per sample (a complex pair counts both parts).
    pub bits: u32,
    pub significant_bits: Option<u32>,
    pub format: Format,
    pub signed: bool,
    pub compression: Compression,
    /// Significance (1 the least) of each stored byte of a sample (or of one part of a
    /// complex sample).
    pub byte_order: Vec<u8>,
    /// Dimensions in storage order, fastest first.
    pub dims: Vec<Dim>,
    /// Indices into `dims` of the image's columns and rows.
    pub x: usize,
    pub y: usize,
    /// The channel dimension, if any (`ch`, `c`, `channel`, `probe`).
    pub channel: Option<usize>,
    pub data: DataAt,
    /// The values' unit and calibration (the `bits` parameter).
    pub value_unit: String,
    pub value_origin: f64,
    pub value_scale: f64,
    /// `layout coordinates` (`video`: rows top-down).
    pub coordinates: String,
    /// Every other header line: (key, value).
    pub facts: Vec<(String, String)>,
    /// Emission and excitation wavelengths per channel, nm (`sensor s_params`).
    pub emission: Vec<f64>,
    pub excitation: Vec<f64>,
}

impl Ics {
    /// Bytes of one sample.
    pub fn sample_bytes(&self) -> usize {
        (self.bits / 8) as usize
    }

    /// Bytes of one part of a sample (half a complex pair).
    fn part_bytes(&self) -> usize {
        match self.format {
            Format::Complex => self.sample_bytes() / 2,
            _ => self.sample_bytes(),
        }
    }

    pub fn width(&self) -> usize {
        self.dims[self.x].len
    }

    pub fn height(&self) -> usize {
        self.dims[self.y].len
    }

    /// Bytes of every sample.
    pub fn data_bytes(&self) -> Option<usize> {
        self.dims
            .iter()
            .try_fold(self.sample_bytes(), |a, d| a.checked_mul(d.len))
    }

    /// The dimensions past the image's columns and rows (channel included), in storage
    /// order.
    pub fn plane_dims(&self) -> Vec<usize> {
        (0..self.dims.len())
            .filter(|&d| d != self.x && d != self.y)
            .collect()
    }

    /// The sample stream from the bytes of the file that holds it (the `.ics` itself for
    /// inline data, else the `.ids` or source file): sliced at its start and decompressed.
    pub fn stream<'a>(&self, file: &'a [u8]) -> Result<Cow<'a, [u8]>> {
        let start = match &self.data {
            DataAt::Inline(o) => *o,
            DataAt::Ids(_) => 0,
            DataAt::Source { offset, .. } => usize::try_from(*offset).unwrap_or(usize::MAX),
        };
        let raw = file
            .get(start..)
            .ok_or(Error::Truncated("ICS data offset"))?;
        let want = self.data_bytes().ok_or(Error::TooLarge)?;
        match self.compression {
            Compression::None => Ok(Cow::Borrowed(raw)),
            Compression::Gzip => {
                let out = match specular_png::inflate_gzip_members(raw, want, Some(want)) {
                    Ok(v) => v,
                    // A zlib stream, or a gzip stream cut short, still gives what it has.
                    Err(e) => specular_png::inflate_zlib(raw, want)
                        .or_else(|_| specular_png::inflate_gzip_prefix(raw, want))
                        .map_err(|_| Error::Malformed(format!("ICS gzip data: {e}")))?,
                };
                Ok(Cow::Owned(out))
            }
            Compression::Compress => specular_xtal::lzw::decompress(raw, want)
                .map(Cow::Owned)
                .map_err(|e| Error::Malformed(format!("ICS compress data: {e}"))),
        }
    }

    /// One plane (rows top-down) at `pos` (a position per dimension; the image's are
    /// ignored), for complex samples the `part` (0 magnitude, 1 real, 2 imaginary, 3
    /// phase), from the decoded stream. Samples past its end read as zero and the plane is
    /// truncated.
    pub fn read_plane(
        &self,
        stream: &[u8],
        pos: &[usize],
        part: u8,
        out: &mut [f32],
    ) -> Result<bool> {
        let (w, h) = (self.width(), self.height());
        if out.len() != w * h {
            return Err(Error::Malformed("plane buffer".into()));
        }
        let sb = self.sample_bytes();
        let mut strides = Vec::with_capacity(self.dims.len());
        let mut s = sb;
        for d in &self.dims {
            strides.push(s);
            s = s.checked_mul(d.len).ok_or(Error::TooLarge)?;
        }
        let mut base = 0usize;
        for (d, dim) in self.dims.iter().enumerate() {
            if d == self.x || d == self.y {
                continue;
            }
            let p = pos.get(d).copied().unwrap_or(0);
            if p >= dim.len {
                return Err(Error::Malformed("no such plane".into()));
            }
            base += p * strides[d];
        }
        let (sx, sy) = (strides[self.x], strides[self.y]);
        let pb = self.part_bytes();
        let order = self.order();
        let read = |at: usize| -> Option<f32> {
            let b = stream.get(at..at.checked_add(sb)?)?;
            Some(match self.format {
                Format::Complex => {
                    let re = self.value(&b[..pb], &order);
                    let im = self.value(&b[pb..], &order);
                    match part {
                        1 => re,
                        2 => im,
                        3 => im.atan2(re),
                        _ => re.hypot(im),
                    }
                }
                _ => self.value(b, &order),
            } as f32)
        };
        let mut truncated = false;
        // Rows as stored, whatever `layout coordinates` says (as libics and DIPlib read them).
        for row in 0..h {
            let o = &mut out[row * w..(row + 1) * w];
            let r0 = base + row * sy;
            for (col, v) in o.iter_mut().enumerate() {
                match read(r0 + col * sx) {
                    Some(x) => *v = x,
                    None => {
                        *v = 0.0;
                        truncated = true;
                    }
                }
            }
        }
        Ok(truncated)
    }

    /// For each significance (0 the least), the stored byte holding it.
    fn order(&self) -> Vec<usize> {
        let n = self.part_bytes();
        let mut perm: Vec<usize> = (0..n).collect();
        if self.byte_order.len() == n {
            for (stored, &sig) in self.byte_order.iter().enumerate() {
                if let Some(p) = perm.get_mut(usize::from(sig).wrapping_sub(1)) {
                    *p = stored;
                }
            }
        }
        perm
    }

    /// One stored value (`b` holds one part) as f64.
    fn value(&self, b: &[u8], order: &[usize]) -> f64 {
        let mut le = [0u8; 8];
        for (k, &i) in order.iter().enumerate().take(8) {
            le[k] = b.get(i).copied().unwrap_or(0);
        }
        let n = b.len();
        match self.format {
            Format::Real | Format::Complex => match n {
                4 => f64::from(f32::from_le_bytes([le[0], le[1], le[2], le[3]])),
                _ => f64::from_le_bytes(le),
            },
            Format::Integer => {
                let u = u64::from_le_bytes(le);
                if self.signed && n < 8 {
                    let shift = 64 - 8 * n as u32;
                    (((u << shift) as i64) >> shift) as f64
                } else if self.signed {
                    u as i64 as f64
                } else {
                    u as f64
                }
            }
        }
    }
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Whether a `layout order` name is the channel dimension.
fn is_channel(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "ch" | "c" | "channel" | "channels" | "probe" | "rgb" | "lambda" | "tensor"
    )
}

/// The last part of a path (`\` or `/` separated).
fn base_name(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or_default()
}

/// Parse the header at the start of an `.ics` file (`data` may run on into inline samples).
pub fn parse(data: &[u8]) -> Result<Ics> {
    if !sniff(data) {
        return Err(Error::Malformed("not an ICS header".into()));
    }
    let (fs, ls) = (data[0], data[1]);
    let mut i = 2;
    let mut version = String::new();
    let mut order: Vec<String> = Vec::new();
    let mut sizes: Vec<usize> = Vec::new();
    let mut params: Option<usize> = None;
    let (mut origin, mut scale, mut units, mut labels) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut format = None;
    let mut signed = None;
    let mut compression = Compression::None;
    let mut byte_order = Vec::new();
    let mut significant_bits = None;
    let mut coordinates = String::new();
    let mut ids_name = None;
    let (mut source_file, mut source_offset) = (None, None);
    let mut facts = Vec::new();
    let (mut emission, mut excitation) = (Vec::new(), Vec::new());
    let mut end = None;
    let fact = |facts: &mut Vec<(String, String)>, k: String, v: String| {
        if facts.len() < MAX_FACTS {
            facts.push((k, v));
        }
    };
    while i < data.len() && i < MAX_HEADER {
        let j = data[i..]
            .iter()
            .position(|&b| b == ls)
            .map_or(data.len(), |p| i + p);
        let line = String::from_utf8_lossy(&data[i..j]);
        i = j + 1;
        let line = line.trim_matches(|c| c == '\r' || c == '\n');
        let tokens: Vec<&str> = if fs != b' ' && line.as_bytes().contains(&fs) {
            line.split(fs as char)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .collect()
        } else {
            line.split_whitespace().collect()
        };
        let Some((&cat, rest)) = tokens.split_first() else {
            continue;
        };
        let sub = rest.first().copied().unwrap_or("");
        let vals = rest.get(1..).unwrap_or(&[]);
        match (cat, sub) {
            ("end", _) => {
                end = Some(i.min(data.len()));
                break;
            }
            ("ics_version", _) => version = sub.to_owned(),
            ("filename", _) => ids_name = Some(rest.join(" ")),
            ("layout", "parameters") => params = vals.first().and_then(|v| v.parse().ok()),
            ("layout", "order") => order = vals.iter().map(|s| s.to_string()).collect(),
            ("layout", "sizes") => {
                sizes = vals
                    .iter()
                    .map(|v| v.parse::<usize>())
                    .collect::<std::result::Result<_, _>>()
                    .map_err(|_| Error::Malformed("ICS layout sizes".into()))?
            }
            ("layout", "significant_bits") => {
                significant_bits = vals.first().and_then(|v| v.parse().ok())
            }
            ("layout", "coordinates") => coordinates = vals.join(" "),
            ("representation", "format") => {
                let f = vals.first().map(|s| s.to_ascii_lowercase());
                format = Some(match f.as_deref() {
                    Some("integer") => Format::Integer,
                    Some("real" | "float") => Format::Real,
                    Some("complex") => Format::Complex,
                    other => {
                        return Err(Error::Unsupported(format!(
                            "ICS sample format {}",
                            other.unwrap_or("(none)")
                        )));
                    }
                })
            }
            ("representation", "sign") => {
                signed = Some(
                    vals.first()
                        .is_some_and(|s| s.eq_ignore_ascii_case("signed")),
                )
            }
            ("representation", "compression") => {
                let c = vals.first().map(|s| s.to_ascii_lowercase());
                compression = match c.as_deref() {
                    None | Some("uncompressed" | "none") => Compression::None,
                    Some("gzip") => Compression::Gzip,
                    Some("compress") => Compression::Compress,
                    Some(other) => {
                        return Err(Error::Unsupported(format!("ICS compression {other}")));
                    }
                }
            }
            ("representation", "byte_order") => {
                byte_order = vals.iter().filter_map(|v| v.parse::<u8>().ok()).collect()
            }
            ("parameter", "origin") => origin = vals.iter().map(|v| num(v)).collect(),
            ("parameter", "scale") => scale = vals.iter().map(|v| num(v)).collect(),
            ("parameter", "units") => units = vals.iter().map(|s| s.to_string()).collect(),
            ("parameter", "labels") => labels = vals.iter().map(|s| s.to_string()).collect(),
            ("source", "file") => source_file = Some(vals.join(" ")),
            ("source", "offset") => source_offset = vals.first().and_then(|v| v.parse().ok()),
            ("sensor", "s_params" | "s_settings") => {
                let key = vals.first().copied().unwrap_or("");
                let v = vals.get(1..).unwrap_or(&[]);
                let nums = || v.iter().filter_map(|x| num(x)).collect::<Vec<_>>();
                match key.to_ascii_lowercase().as_str() {
                    "lambdaem" => emission = nums(),
                    "lambdaex" => excitation = nums(),
                    _ => {}
                }
                fact(&mut facts, format!("sensor {key}"), v.join(" "));
            }
            _ => fact(
                &mut facts,
                format!("{cat} {sub}").trim().to_owned(),
                vals.join(" "),
            ),
        }
    }
    // Version 1.0 headers (and some 2.0 ones with an .ids) may stop without an `end` line.
    let end = match end {
        Some(e) => e,
        None if i >= data.len() => data.len(),
        None => return Err(Error::Truncated("ICS header (no end line)")),
    };
    if sizes.is_empty() || order.len() != sizes.len() {
        return Err(Error::Malformed("ICS layout order and sizes differ".into()));
    }
    if params.is_some_and(|p| p != sizes.len()) {
        return Err(Error::Malformed("ICS layout parameters".into()));
    }
    if sizes.len() > MAX_DIMS + 1 {
        return Err(Error::Unsupported(format!(
            "{} ICS dimensions",
            sizes.len() - 1
        )));
    }
    if !order[0].eq_ignore_ascii_case("bits") {
        return Err(Error::Malformed(
            "ICS layout order does not start with bits".into(),
        ));
    }
    let bits = u32::try_from(sizes[0]).map_err(|_| Error::TooLarge)?;
    let format = format.unwrap_or(Format::Integer);
    let ok_bits = match format {
        Format::Integer => matches!(bits, 8 | 16 | 32 | 64),
        Format::Real => matches!(bits, 32 | 64),
        Format::Complex => matches!(bits, 64 | 128),
    };
    if !ok_bits {
        return Err(Error::Unsupported(format!(
            "{bits}-bit ICS {} samples",
            match format {
                Format::Integer => "integer",
                Format::Real => "real",
                Format::Complex => "complex",
            }
        )));
    }
    let at = |v: &Vec<Option<f64>>, k: usize| v.get(k).copied().flatten();
    let dims: Vec<Dim> = (1..sizes.len())
        .map(|k| Dim {
            name: order[k].clone(),
            len: sizes[k],
            origin: at(&origin, k).unwrap_or(0.0),
            scale: at(&scale, k).unwrap_or(1.0),
            unit: units.get(k).cloned().unwrap_or_default(),
            label: labels.get(k).cloned().unwrap_or_default(),
        })
        .collect();
    if dims.is_empty() || dims.iter().any(|d| d.len == 0) {
        return Err(Error::Malformed("ICS image without samples".into()));
    }
    let find = |n: &str| dims.iter().position(|d| d.name.eq_ignore_ascii_case(n));
    let channel = dims.iter().position(|d| is_channel(&d.name));
    let others: Vec<usize> = (0..dims.len()).filter(|&d| Some(d) != channel).collect();
    let x = find("x").or_else(|| others.first().copied()).unwrap_or(0);
    let Some(y) = find("y").or_else(|| others.iter().copied().find(|&d| d != x)) else {
        return Err(Error::Unsupported("one-dimensional ICS data".into()));
    };
    let own_stem = ids_name.as_deref().map(|n| {
        let b = base_name(n);
        let lower = b.to_ascii_lowercase();
        if lower.ends_with(".ics") || lower.ends_with(".ids") {
            b[..b.len() - 4].to_owned()
        } else {
            b.to_owned()
        }
    });
    let data_at = if let Some(file) = source_file {
        let name = base_name(&file).to_owned();
        let offset = source_offset.unwrap_or(0);
        let lower = name.to_ascii_lowercase();
        // A version 2.0 file naming itself holds its data inline (at the offset).
        if lower.ends_with(".ics")
            && own_stem
                .as_deref()
                .is_none_or(|o| lower == format!("{}.ics", o.to_ascii_lowercase()))
        {
            DataAt::Inline(
                usize::try_from(offset)
                    .map_err(|_| Error::TooLarge)?
                    .max(end),
            )
        } else {
            DataAt::Source { file: name, offset }
        }
    } else if version.starts_with('2') && data.len() > end {
        DataAt::Inline(end)
    } else {
        DataAt::Ids(format!("{}.ids", own_stem.unwrap_or_default()))
    };
    let mut ics = Ics {
        version,
        bits,
        significant_bits,
        format,
        signed: signed.unwrap_or(format != Format::Integer),
        compression,
        byte_order,
        dims,
        x,
        y,
        channel,
        data: data_at,
        value_unit: units.first().cloned().unwrap_or_default(),
        value_origin: at(&origin, 0).unwrap_or(0.0),
        value_scale: at(&scale, 0).unwrap_or(1.0),
        coordinates,
        facts,
        emission,
        excitation,
    };
    if ics.data_bytes().is_none() {
        return Err(Error::TooLarge);
    }
    if ics
        .width()
        .checked_mul(ics.height())
        .is_none_or(|n| n as u64 > crate::MAX_PLANE)
    {
        return Err(Error::TooLarge);
    }
    if ics.byte_order.len() != ics.part_bytes() {
        // Unstated or malformed: little-endian, as libics writes on most machines.
        ics.byte_order = (1..=ics.part_bytes() as u8).collect();
    }
    Ok(ics)
}

/// A header from its lines, tab and line feed separated (test fixtures).
#[cfg(test)]
pub(crate) fn header(lines: &[&str]) -> Vec<u8> {
    let mut h = b"\t\n".to_vec();
    for l in lines {
        h.extend_from_slice(l.as_bytes());
        h.push(b'\n');
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gzip member of one stored DEFLATE block.
    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut g = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1];
        let n = data.len() as u16;
        g.extend_from_slice(&n.to_le_bytes());
        g.extend_from_slice(&(!n).to_le_bytes());
        g.extend_from_slice(data);
        g.extend_from_slice(&crc32(data).to_le_bytes());
        g.extend_from_slice(&(data.len() as u32).to_le_bytes());
        g
    }

    fn crc32(d: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in d {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    #[test]
    fn sniffs_and_parses_a_v1_header() {
        let h = header(&[
            "ics_version\t1.0",
            "filename\tcells",
            "layout\tparameters\t4",
            "layout\torder\tbits\tx\ty\tz",
            "layout\tsizes\t16\t3\t2\t2",
            "layout\tcoordinates\tvideo",
            "layout\tsignificant_bits\t12",
            "representation\tformat\tinteger",
            "representation\tsign\tunsigned",
            "representation\tcompression\tuncompressed",
            "representation\tbyte_order\t1\t2",
            "parameter\torigin\t0\t0\t0\t0",
            "parameter\tscale\t1\t0.1\t0.1\t0.5",
            "parameter\tunits\trelative\tmicrometers\tmicrometers\tmicrometers",
            "sensor\ts_params\tLambdaEm\t520",
            "history\tsoftware\ttest",
            "end",
        ]);
        assert!(sniff(&h));
        let ics = parse(&h).expect("parse");
        assert_eq!((ics.width(), ics.height()), (3, 2));
        assert_eq!(ics.data, DataAt::Ids("cells.ids".into()));
        assert_eq!(ics.dims[2].scale, 0.5);
        assert_eq!(ics.emission, vec![520.0]);
        let s: Vec<u8> = (0..12u16).flat_map(|x| x.to_le_bytes()).collect();
        let mut out = vec![0.0; 6];
        assert!(!ics.read_plane(&s, &[0, 0, 1], 0, &mut out).unwrap());
        assert_eq!(out, vec![6.0, 7.0, 8.0, 9.0, 10.0, 11.0]);
    }

    #[test]
    fn inline_big_endian_signed_any_order_and_gzip() {
        // ch first (interleaved), then x, y; big-endian signed 16-bit.
        let vals: Vec<i16> = (0..12).map(|v| v as i16 - 6).collect();
        let raw: Vec<u8> = vals.iter().flat_map(|v| v.to_be_bytes()).collect();
        let mk = |comp: &str| {
            header(&[
                "ics_version\t2.0",
                "filename\tx.ics",
                "layout\tparameters\t4",
                "layout\torder\tbits\tch\tx\ty",
                "layout\tsizes\t16\t2\t3\t2",
                "representation\tformat\tinteger",
                "representation\tsign\tsigned",
                &format!("representation\tcompression\t{comp}"),
                "representation\tbyte_order\t2\t1",
                "end",
            ])
        };
        let mut h = mk("uncompressed");
        h.extend_from_slice(&raw);
        let ics = parse(&h).unwrap();
        assert_eq!(ics.channel, Some(0));
        assert!(matches!(ics.data, DataAt::Inline(_)));
        let st = ics.stream(&h).unwrap();
        let mut out = vec![0.0; 6];
        ics.read_plane(&st, &[1, 0, 0], 0, &mut out).unwrap();
        assert_eq!(out, vec![-5.0, -3.0, -1.0, 1.0, 3.0, 5.0]);
        // A plane cut short is truncated, not an error.
        let cut = &h[..h.len() - 4];
        assert!(
            ics.read_plane(&ics.stream(cut).unwrap(), &[1, 0, 0], 0, &mut out)
                .unwrap()
        );
        // The same samples gzipped.
        let mut g = mk("gzip");
        g.extend_from_slice(&gzip(&raw));
        let gi = parse(&g).unwrap();
        assert_eq!(gi.compression, Compression::Gzip);
        gi.read_plane(&gi.stream(&g).unwrap(), &[1, 0, 0], 0, &mut out)
            .unwrap();
        assert_eq!(out, vec![-5.0, -3.0, -1.0, 1.0, 3.0, 5.0]);
    }

    #[test]
    fn complex_parts_and_refusals() {
        let mut h = header(&[
            "ics_version\t2.0",
            "layout\tparameters\t3",
            "layout\torder\tbits\tx\ty",
            "layout\tsizes\t64\t1\t1",
            "representation\tformat\tcomplex",
            "end",
        ]);
        h.extend_from_slice(&3f32.to_le_bytes());
        h.extend_from_slice(&4f32.to_le_bytes());
        let ics = parse(&h).unwrap();
        let st = ics.stream(&h).unwrap();
        let mut out = [0.0f32];
        for (part, want) in [(0u8, 5.0f32), (1, 3.0), (2, 4.0)] {
            ics.read_plane(&st, &[], part, &mut out).unwrap();
            assert_eq!(out[0], want);
        }
        let bad = header(&[
            "ics_version\t2.0",
            "layout\tparameters\t3",
            "layout\torder\tbits\tx\ty",
            "layout\tsizes\t12\t1\t1",
            "end",
        ]);
        assert!(matches!(parse(&bad), Err(Error::Unsupported(_))));
        let huge = header(&[
            "ics_version\t2.0",
            "layout\tparameters\t4",
            "layout\torder\tbits\tx\ty\tz",
            "layout\tsizes\t64\t4294967296\t4294967296\t4294967296",
            "end",
        ]);
        assert!(parse(&huge).is_err());
        assert!(parse(&header(&["ics_version\t1.0"])).is_err());
    }
}
