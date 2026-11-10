//! Kodak Cineon 4.5 (`.cin`), in either byte order.
//!
//! One to eight channels of 1 to 16 bits each (depths may differ between channels), unsigned
//! or signed, in any of the format's seven packings (tight, byte, 16-bit or 32-bit units,
//! left or right justified) and three interleaves (pixel, line, channel). The code values are
//! printing density and are shown scene-linear through the printing-density conversion (see
//! the crate documentation). The red, green and blue channels (by designator, else the first
//! three) are shown; others are reported only. Pixel-interleaved channels of different
//! depths are read tightly packed only (the format does not say how other packings share a
//! unit between them).

use crate::log::{self, LogParams};
use crate::unpack::{Layout, Rows, Shape, for_rows};
use crate::{
    Error, Image, Info, Model, Pixels, Reader, Result, check_dims, check_payload, exif_orientation,
    num, orientation_name,
};

const MAGIC: [u8; 4] = [0x80, 0x2a, 0x5f, 0xd7];

/// The magic number 0x802A5FD7 in either byte order.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&MAGIC) || head.starts_with(&[0xd7, 0x5f, 0x2a, 0x80])
}

/// One channel's depth and how its codes become linear light.
struct Chan {
    bits: u8,
    /// Designator: 0 grey, 1 red, 2 green, 3 blue.
    desig: u8,
    layout: Layout,
    params: LogParams,
}

struct Header<'a> {
    r: Reader<'a>,
    offset: u32,
    orientation: u32,
    channels: usize,
    width: u32,
    height: u32,
    chans: Vec<Chan>,
    /// Channels differ in depth.
    mixed: bool,
    signed: bool,
    interleave: u8,
    packing: u8,
}

fn header(data: &[u8]) -> Result<Header<'_>> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    if data.len() < 1024 {
        return Err(Error::Truncated("Cineon header".into()));
    }
    let r = Reader {
        d: data,
        be: data.starts_with(&MAGIC),
    };
    let offset = r.u32(4).unwrap_or(0);
    let orientation = r.u8(192).unwrap_or(0) as u32;
    let channels = r.u8(193).unwrap_or(0) as usize;
    match channels {
        1..=8 => {}
        0 => return Err(Error::Malformed("no channels".into())),
        n => {
            return Err(Error::Malformed(format!(
                "{n} channels (the format allows up to 8)"
            )));
        }
    }
    let ch = |i: usize| 196 + i * 28;
    let width = r.u32(ch(0) + 4).unwrap_or(0);
    let height = r.u32(ch(0) + 8).unwrap_or(0);
    for i in 1..channels {
        if r.u32(ch(i) + 4) != Some(width) || r.u32(ch(i) + 8) != Some(height) {
            return Err(Error::Unsupported(
                "Cineon files whose channels differ in size are not supported".into(),
            ));
        }
    }
    let signed = r.u8(682) == Some(1);
    let interleave = r.u8(680).unwrap_or(0);
    if interleave > 2 {
        return Err(Error::Unsupported(format!(
            "Cineon interleave {interleave} is not one the format defines"
        )));
    }
    let packing = r.u8(681).unwrap_or(5);
    let mut chans = Vec::with_capacity(channels);
    for i in 0..channels {
        let bits = r.u8(ch(i) + 2).unwrap_or(0);
        if !(1..=16).contains(&bits) {
            return Err(Error::Unsupported(format!(
                "Cineon samples of {bits} bits are not supported"
            )));
        }
        let mut params = LogParams::default();
        let max = ((1u32 << bits) - 1) as f32;
        if let (Some(mind), Some(minq), Some(maxd), Some(maxq)) = (
            r.f32_set(ch(i) + 12),
            r.f32_set(ch(i) + 16),
            r.f32_set(ch(i) + 20),
            r.f32_set(ch(i) + 24),
        ) && maxd > mind
        {
            let d = (maxq - minq) / (maxd - mind) * (max / 1023.0);
            if (0.0005..0.01).contains(&d) {
                params.density = d;
            }
        }
        chans.push(Chan {
            bits,
            desig: r.u8(ch(i) + 1).unwrap_or(0xff),
            layout: layout(bits, packing, r.be)?,
            params,
        });
    }
    let mixed = chans.iter().any(|c| c.bits != chans[0].bits);
    if mixed && interleave == 0 && packing != 0 {
        return Err(Error::Unsupported(
            "Cineon pixel-interleaved channels of different depths are only supported tightly packed (packing 0)".into(),
        ));
    }
    Ok(Header {
        r,
        offset,
        orientation,
        channels,
        width,
        height,
        chans,
        mixed,
        signed,
        interleave,
        packing,
    })
}

fn layout(bits: u8, packing: u8, be: bool) -> Result<Layout> {
    let unit_for = |min: u8| -> u8 {
        if bits <= 8 && min <= 1 {
            1
        } else if bits <= 16 && min <= 2 {
            2
        } else {
            4
        }
    };
    let shape = match packing {
        0 => Shape::Stream { lsb_first: false },
        1 | 2 => Shape::Container {
            unit: unit_for(1),
            left: packing == 1,
        },
        3 | 4 => Shape::Container {
            unit: unit_for(2),
            left: packing == 3,
        },
        5 | 6 => Shape::Container {
            unit: 4,
            left: packing == 5,
        },
        p => {
            return Err(Error::Unsupported(format!(
                "Cineon packing {p} is not one the format defines"
            )));
        }
    };
    let l = Layout {
        bits,
        shape,
        be,
        low_first: false,
    };
    l.validate()?;
    Ok(l)
}

/// The channels shown as red, green and blue (by designator, else the first three), or the
/// one shown as grey.
fn shown(h: &Header) -> Vec<usize> {
    let find = |d: u8| h.chans.iter().position(|c| c.desig == d);
    match (find(1), find(2), find(3)) {
        (Some(r), Some(g), Some(b)) => vec![r, g, b],
        _ if h.channels >= 3 => vec![0, 1, 2],
        _ => vec![0],
    }
}

/// The largest channel depth.
fn depth(h: &Header) -> u8 {
    h.chans.iter().map(|c| c.bits).max().unwrap_or(0)
}

fn packing_name(p: u8) -> &'static str {
    match p {
        0 => "tightly packed",
        1 => "byte, left justified",
        2 => "byte, right justified",
        3 => "16-bit word, left justified",
        4 => "16-bit word, right justified",
        5 => "32-bit word, left justified",
        6 => "32-bit word, right justified",
        _ => "unknown packing",
    }
}

/// Header facts (Cineon has one image: `page` must be 0).
pub fn info(data: &[u8], page: u32) -> Result<Info> {
    let h = header(data)?;
    if page != 0 {
        return Err(Error::Malformed(format!("no page {page}")));
    }
    check_dims(h.width, h.height)?;
    let bits = if h.mixed {
        let d: Vec<String> = h.chans.iter().map(|c| c.bits.to_string()).collect();
        d.join("/")
    } else {
        h.chans[0].bits.to_string()
    };
    let rgb = shown(&h).len() == 3;
    let what = match h.channels {
        1 => "grey".to_owned(),
        3 if rgb => "RGB".to_owned(),
        n if rgb => format!("{n}-channel (RGB shown)"),
        n => format!("{n}-channel (first shown as grey)"),
    };
    let coding = format!(
        "{bits}-bit {what}{} log, {}, {}",
        if h.signed { " signed" } else { "" },
        packing_name(h.packing),
        if h.r.be {
            "big-endian"
        } else {
            "little-endian"
        }
    );
    let (summary, fields) = facts(&h);
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: 1,
        bit_depth: depth(&h),
        model: if rgb { Model::Rgb } else { Model::Grey },
        has_alpha: false,
        orientation: exif_orientation(h.orientation),
        coding,
        summary,
        fields,
    })
}

fn facts(h: &Header) -> crate::Facts {
    let r = h.r;
    let mut s: Vec<(&'static str, String)> = Vec::new();
    let mut f: Vec<(String, String)> = Vec::new();
    let mut field = |name: &str, v: Option<String>| {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            f.push((name.to_owned(), v));
        }
    };
    let u = |o: usize| r.u32_set(o).filter(|&v| v != 0).map(|v| v.to_string());
    let fl = |o: usize| r.f32_set(o).filter(|&v| v != 0.0).map(num);
    let date = |d: usize, t: usize| match (r.str(d, 12), r.str(t, 12)) {
        (Some(d), Some(t)) => Some(format!(
            "{} {}",
            d.replace(':', "-"),
            t.replace(['-', '_'], ":")
        )),
        (Some(d), None) => Some(d),
        _ => None,
    };
    field("Version", r.str(24, 8));
    field("Image data offset", r.u32_set(4).map(|v| v.to_string()));
    field("File size", u(20));
    field("File name", r.str(32, 100));
    let created = date(132, 144);
    field("Created", created.clone());
    field(
        "Orientation",
        Some(orientation_name(h.orientation).to_owned()),
    );
    field("Channels", Some(h.channels.to_string()));
    for i in 0..h.channels {
        let o = 196 + i * 28;
        let name = match r.u8(o + 1) {
            Some(0) => "grey",
            Some(1) => "red",
            Some(2) => "green",
            Some(3) => "blue",
            _ => "other",
        };
        let n = i + 1;
        field(
            &format!("Channel {n}"),
            Some(format!(
                "{name}, {} bits, {}×{}",
                r.u8(o + 2).unwrap_or(0),
                r.u32(o + 4).unwrap_or(0),
                r.u32(o + 8).unwrap_or(0)
            )),
        );
        if let (Some(a), Some(b)) = (r.f32_set(o + 12), r.f32_set(o + 20)) {
            field(
                &format!("Channel {n} data range"),
                Some(format!("{} – {}", num(a), num(b))),
            );
        }
        if let (Some(a), Some(b)) = (r.f32_set(o + 16), r.f32_set(o + 24))
            && b != 0.0
        {
            field(
                &format!("Channel {n} density range"),
                Some(format!("{} – {}", num(a), num(b))),
            );
        }
    }
    let pair = |o: usize| match (r.f32_set(o), r.f32_set(o + 4)) {
        (Some(x), Some(y)) if x != 0.0 || y != 0.0 => Some(format!("{}, {}", num(x), num(y))),
        _ => None,
    };
    field("White point", pair(420));
    field("Red primary", pair(428));
    field("Green primary", pair(436));
    field("Blue primary", pair(444));
    let label = r.str(452, 200);
    field("Label", label.clone());
    field(
        "Interleave",
        Some(
            match h.interleave {
                0 => "pixel",
                1 => "line",
                _ => "channel",
            }
            .to_owned(),
        ),
    );
    field("Packing", Some(packing_name(h.packing).to_owned()));
    field(
        "Image sense",
        r.u8(683)
            .map(|v| if v == 1 { "negative" } else { "positive" }.to_owned()),
    );
    field("End-of-line padding", u(684));
    field("End-of-channel padding", u(688));
    field("X offset", u(712));
    field("Y offset", u(716));
    field("Source file name", r.str(720, 100));
    field("Source created", date(820, 832));
    let device = r.str(844, 64);
    let model = r.str(908, 32);
    let serial = r.str(940, 32);
    field("Input device", device.clone());
    field("Device model", model.clone());
    field("Device serial", serial.clone());
    field("X device pitch", fl(972));
    field("Y device pitch", fl(976));
    field("Gamma", fl(980));
    let dev: Vec<String> = [device, model].into_iter().flatten().collect();
    if !dev.is_empty() {
        let mut d = dev.join(" ");
        if let Some(sn) = serial {
            d = format!("{d} ({sn})");
        }
        s.push(("device", d));
    }
    if let Some(c) = created {
        s.push(("created", c));
    }
    if r.d.len() >= 1308 && h.offset >= 1308 {
        let b = |o: usize| r.u8(o).filter(|&v| v != 0xff && v != 0);
        let mfg = b(1024);
        let ftype = b(1025);
        let perfs = b(1026);
        let prefix = r.u32_set(1028).filter(|&v| v != 0);
        let count = r.u32_set(1032).filter(|&v| v != 0);
        field("Film manufacturer ID", mfg.map(|v| v.to_string()));
        field("Film type", ftype.map(|v| v.to_string()));
        field("Perfs offset", perfs.map(|v| v.to_string()));
        field("Prefix", prefix.map(|v| v.to_string()));
        field("Count", count.map(|v| v.to_string()));
        if let (Some(p), Some(c)) = (prefix, count)
            && (p != 0 || c != 0)
        {
            let mut k = String::new();
            if let (Some(m), Some(t)) = (mfg, ftype) {
                k = format!("{m:02} {t:02} ");
            }
            k.push_str(&format!("{p:06} {c:04}"));
            if let Some(pf) = perfs {
                k.push_str(&format!("+{pf:02}"));
            }
            s.push(("edge-code", k));
        }
        let format = r.str(1036, 32);
        field("Film format", format.clone());
        let pos = u(1068);
        field("Frame position", pos.clone());
        let rate = fl(1072);
        field("Frame rate", rate.clone());
        let id = r.str(1076, 32);
        field("Frame ID", id.clone());
        let slate = r.str(1108, 200);
        field("Slate", slate.clone());
        if let Some(rate) = rate {
            s.push(("frame-rate", format!("{rate} fps")));
        }
        if let Some(p) = pos.or(id) {
            s.push(("frame", p));
        }
        // Format and slate as two values of one fact: the app joins them in its direction.
        for v in [format, slate].into_iter().flatten() {
            s.push(("film", v));
        }
    }
    s.push(("transfer", "printing density".to_owned()));
    if let Some(l) = label {
        s.push(("comment", l));
    }
    (s, f)
}

/// Where the rows of every channel are.
enum Place {
    /// All channels share one layout: `rows` rows of the stored layout (whole pixels, or one
    /// channel's line).
    Uniform(Rows),
    /// Channels of different depths: each line's start for line and channel interleave, or
    /// tightly packed pixel rows (`stride` 0: one continuous stream).
    Mixed { stride: usize, starts: Vec<usize> },
}

struct Source<'a> {
    h: &'a Header<'a>,
    data: &'a [u8],
    place: Place,
}

fn source<'a>(h: &'a Header<'a>, data: &'a [u8]) -> Result<Source<'a>> {
    let (w, ht, c) = (h.width as usize, h.height as usize, h.channels);
    let offset = h.offset as usize;
    if offset < 712 || offset > data.len() {
        return Err(Error::Truncated(
            "image data starts past the end of the file".into(),
        ));
    }
    let avail = data.len() - offset;
    let need: u64 = h
        .chans
        .iter()
        .map(|ch| ch.layout.bytes_for((w * ht) as u64))
        .sum();
    check_payload(avail, need, 1)?;
    let eol = h.r.u32_set(684).filter(|&v| v < 1 << 20).unwrap_or(0) as usize;
    let place = if !h.mixed {
        let l = h.chans[0].layout;
        let (n, rows_n) = if h.interleave == 0 {
            (w * c, ht)
        } else {
            (w, ht * c)
        };
        Place::Uniform(Rows::choose(&l, offset, avail, n, rows_n, eol))
    } else if h.interleave == 0 {
        let bits: usize = h.chans.iter().map(|ch| ch.bits as usize).sum();
        let row = (w * bits).div_ceil(32) * 4 + eol;
        // Rows padded to 32 bits, or one continuous stream when the data is too short.
        let stride = if avail < row * ht && avail * 8 >= w * ht * bits && eol == 0 {
            0
        } else {
            row
        };
        Place::Mixed {
            stride,
            starts: vec![offset],
        }
    } else {
        let line = |ch: &Chan| ch.layout.row_bytes(w as u64) as usize + eol;
        let eoc = h.r.u32_set(688).filter(|&v| v < 1 << 20).unwrap_or(0) as usize;
        let mut starts = Vec::with_capacity(c);
        let mut at = offset;
        if h.interleave == 1 {
            // Line y: one line of every channel in turn.
            for ch in &h.chans {
                starts.push(at);
                at += line(ch);
            }
            Place::Mixed {
                stride: at - offset,
                starts,
            }
        } else {
            for ch in &h.chans {
                starts.push(at);
                at += line(ch) * ht + eoc;
            }
            Place::Mixed { stride: 0, starts }
        }
    };
    Ok(Source { h, data, place })
}

impl Source<'_> {
    /// Fills `s` (`channels` samples per pixel) with the codes of row `y`; whether they were
    /// all there (missing ones are mid-grey).
    fn row(&self, y: usize, s: &mut [u16], line: &mut [u16]) -> bool {
        let (h, d) = (self.h, self.data);
        let (w, ht, c) = (h.width as usize, h.height as usize, h.channels);
        let mut ok = true;
        let fill = |dst: &mut [u16], got: usize, bits: u8| {
            if got < dst.len() {
                dst[got..].fill(1u16 << (bits - 1));
                false
            } else {
                true
            }
        };
        match &self.place {
            Place::Uniform(rows) => {
                let l = h.chans[0].layout;
                if h.interleave == 0 {
                    let (base, first) = rows.at(y);
                    let got = l.unpack(d, base, first, s);
                    ok &= fill(s, got, l.bits);
                } else {
                    for ch in 0..c {
                        let vy = if h.interleave == 1 {
                            y * c + ch
                        } else {
                            ch * ht + y
                        };
                        let (base, first) = rows.at(vy);
                        let got = l.unpack(d, base, first, line);
                        ok &= fill(line, got, l.bits);
                        for (x, &v) in line.iter().enumerate() {
                            s[x * c + ch] = v;
                        }
                    }
                }
            }
            Place::Mixed { stride, starts } if h.interleave == 0 => {
                // Fields of each channel's width in turn, most significant bit first.
                let bits: usize = h.chans.iter().map(|ch| ch.bits as usize).sum();
                let mut bit = if *stride == 0 {
                    starts[0] * 8 + y * w * bits
                } else {
                    (starts[0] + y * stride) * 8
                };
                for (k, o) in s.iter_mut().enumerate() {
                    let b = h.chans[k % c].bits as usize;
                    match read_bits(d, bit, b) {
                        Some(v) => *o = v,
                        None => {
                            *o = 1 << (b - 1);
                            ok = false;
                        }
                    }
                    bit += b;
                }
            }
            Place::Mixed { stride, starts } => {
                for (ch, chan) in h.chans.iter().enumerate() {
                    let base = if h.interleave == 1 {
                        starts[ch] + y * stride
                    } else {
                        starts[ch] + y * (chan.layout.row_bytes(w as u64) as usize + self.eol())
                    };
                    let got = chan.layout.unpack(d, base, 0, line);
                    ok &= fill(line, got, chan.bits);
                    for (x, &v) in line.iter().enumerate() {
                        s[x * c + ch] = v;
                    }
                }
            }
        }
        ok
    }

    fn eol(&self) -> usize {
        self.h.r.u32_set(684).filter(|&v| v < 1 << 20).unwrap_or(0) as usize
    }
}

/// `n` bits (most significant first) at bit `at` of `d`.
fn read_bits(d: &[u8], at: usize, n: usize) -> Option<u16> {
    let mut v = 0u32;
    for k in at..at + n {
        let byte = *d.get(k / 8)?;
        v = (v << 1) | ((byte >> (7 - k % 8)) & 1) as u32;
    }
    Some(v as u16)
}

/// Code value to linear light, for every code of a channel: printing density, or for signed
/// samples the two's-complement value with the positive half spanning the 10-bit range.
fn table(h: &Header, ch: &Chan) -> Vec<f32> {
    if !h.signed {
        return log::log_table(ch.bits, &ch.params);
    }
    let half = 1i32 << (ch.bits - 1);
    let scale = 1023.0 / (half - 1).max(1) as f32;
    (0..1i32 << ch.bits)
        .map(|c| {
            let v = if c >= half { c - 2 * half } else { c };
            ch.params.linear(v as f32 * scale)
        })
        .collect()
}

/// Decode the image as linear RGBA.
pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    let h = header(data)?;
    if page != 0 {
        return Err(Error::Malformed(format!("no page {page}")));
    }
    check_dims(h.width, h.height)?;
    let (w, ht, c) = (h.width as usize, h.height as usize, h.channels);
    let src = source(&h, data)?;
    let show = shown(&h);
    let tables: Vec<Vec<f32>> = show.iter().map(|&k| table(&h, &h.chans[k])).collect();
    let mut out = vec![0f32; w * ht * 4];
    let short = for_rows(&mut out, w * 4, w * c + w, |y, row, s: &mut [u16]| {
        let (s, line) = s.split_at_mut(w * c);
        let ok = src.row(y, s, line);
        let t = |k: usize, x: usize| {
            tables[k]
                .get(s[x * c + show[k]] as usize)
                .copied()
                .unwrap_or(0.0)
        };
        for (x, o) in row.chunks_exact_mut(4).enumerate() {
            let px = if show.len() == 3 {
                [t(0, x), t(1, x), t(2, x), 1.0]
            } else {
                let v = t(0, x);
                [v, v, v, 1.0]
            };
            o.copy_from_slice(&px);
        }
        ok
    });
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels: Pixels::RgbaF32(out),
        truncated: short,
    })
}

/// The stored code values, `channels` per pixel in pixel order (for tests against other
/// readers): (width, height, channels, samples, bits). Channels of different depths are
/// widened to 16 bits (rounded), and `bits` is then 16.
#[doc(hidden)]
pub fn decode_codes(data: &[u8]) -> Result<(u32, u32, usize, Vec<u16>, u8)> {
    let h = header(data)?;
    check_dims(h.width, h.height)?;
    let (w, ht, c) = (h.width as usize, h.height as usize, h.channels);
    let src = source(&h, data)?;
    let mut out = vec![0u16; w * ht * c];
    let mut line = vec![0u16; w];
    for (y, row) in out.chunks_exact_mut(w * c).enumerate() {
        src.row(y, row, &mut line);
        if h.mixed {
            for (k, v) in row.iter_mut().enumerate() {
                let max = ((1u32 << h.chans[k % c].bits) - 1) as f64;
                *v = (*v as f64 * 65535.0 / max).round() as u16;
            }
        }
    }
    let bits = if h.mixed { 16 } else { h.chans[0].bits };
    Ok((h.width, h.height, c, out, bits))
}
