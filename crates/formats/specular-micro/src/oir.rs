//! Olympus/Evident FluoView OIR (`OLYMPUSRAWFORMAT`): confocal stacks.
//!
//! The file interleaves XML documents (each after its byte length) with raw pixel blocks.
//! A pixel block is `[length = uid + 12][3][8 bytes][uid length][uid][pixel bytes][4 bytes]`
//! and its pixels; the uid names the plane and the block, e.g.
//! `l002z003t004_0_1_<channel id>_5`: λ, Z and T positions (from 1, each optional), the
//! channel's id and the block's index (blocks are bands of whole rows, top first).
//! `REF_…` blocks are reference images and are skipped.
//!
//! - `frameProperties`: the frame size, bytes per sample and significant bits.
//! - `imageProperties`: the channels (id, order, name, excitation and emission range, pixel
//!   size and unit), the axes (Z stack, time lapse, λ with start and step) and the objective.
//! - `lut:LUT` (after its uid): a channel's colour table; a linear one's last entry is the
//!   channel's colour.
//!
//! Blocks are found by their header wherever they are, so a damaged XML document costs only
//! its facts. Planes that are missing read as zero and mark the plane truncated.

use std::collections::HashMap;

use specular_wsi::xml::{self, Element};

use crate::{Error, Result, Sample};

/// The file signature.
pub const MAGIC: &[u8; 16] = b"OLYMPUSRAWFORMAT";
/// Most pixel blocks of a file.
const MAX_BLOCKS: usize = 1 << 22;
/// Longest XML document parsed.
const MAX_XML: usize = 64 << 20;

pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

/// One channel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    /// The detected wavelength range, nm.
    pub start_nm: Option<f64>,
    pub end_nm: Option<f64>,
    /// From a linear colour table.
    pub colour: Option<[u8; 3]>,
}

/// An axis past X and Y: its size, first position and step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Axis {
    pub size: usize,
    pub start: f64,
    pub step: f64,
}

/// Where one pixel block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Block {
    index: u32,
    offset: usize,
    len: usize,
}

/// A parsed OIR file.
#[derive(Clone, Debug, Default)]
pub struct Oir {
    pub width: usize,
    pub height: usize,
    pub sample: Option<Sample>,
    pub bits: u32,
    pub channels: Vec<Channel>,
    /// Sizes from the pixel blocks (λ, Z, T).
    pub size_l: usize,
    pub size_z: usize,
    pub size_t: usize,
    /// µm.
    pub z: Option<Axis>,
    /// Seconds.
    pub t: Option<Axis>,
    /// nm.
    pub lambda: Option<Axis>,
    /// Pixel size (x, y), µm.
    pub pixel: Option<(f64, f64)>,
    pub objective: Option<String>,
    pub magnification: Option<f64>,
    pub numerical_aperture: Option<f64>,
    pub date: Option<String>,
    /// Facts for the info panel.
    pub meta: Vec<(String, String)>,
    /// Blocks per plane, by (channel, λ, Z, T) from 0, sorted by index.
    planes: HashMap<(usize, usize, usize, usize), Vec<Block>>,
    min: (usize, usize, usize),
}

fn le_i32(b: &[u8], o: usize) -> Option<i32> {
    crate::le_u32(b, o).map(|v| v as i32)
}

/// The block at `o`: uid, pixel offset, pixel length.
fn block_at(b: &[u8], o: usize) -> Option<(&str, usize, usize)> {
    let len = le_i32(b, o)?;
    if le_i32(b, o + 4)? != 3 {
        return None;
    }
    let ul = le_i32(b, o + 16)?;
    if !(1..300).contains(&ul) || len != ul + 12 {
        return None;
    }
    let ul = ul as usize;
    let uid = b.get(o + 20..o + 20 + ul)?;
    if !uid.iter().all(|c| (0x20..0x7f).contains(c)) {
        return None;
    }
    let n = usize::try_from(le_i32(b, o + 20 + ul)?).ok()?;
    let start = o + 20 + ul + 8;
    (n > 0 && start.checked_add(n)? <= b.len())
        .then(|| (std::str::from_utf8(uid).ok().unwrap_or(""), start, n))
}

/// (λ, Z, T) from 1 (0 when absent), channel id and block index of a uid.
fn parse_uid(uid: &str) -> Option<((usize, usize, usize), &str, u32)> {
    let (head, _) = uid.split_once('_')?;
    let (rest, block) = uid.rsplit_once('_')?;
    let (_, channel) = rest.rsplit_once('_')?;
    let block = block.parse().ok()?;
    let mut pos = [0usize; 3];
    let mut s = head;
    while let Some(c) = s.chars().next() {
        let k = match c {
            'l' => 0,
            'z' => 1,
            't' => 2,
            _ => return None,
        };
        let digits: String = s[1..].chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        pos[k] = digits.parse().ok()?;
        s = &s[1 + digits.len()..];
    }
    Some(((pos[0], pos[1], pos[2]), channel, block))
}

fn descendants<'a>(e: &'a Element, name: &str, out: &mut Vec<&'a Element>) {
    for c in &e.children {
        if c.name == name {
            out.push(c);
        }
        descendants(c, name, out);
    }
}

fn all<'a>(e: &'a Element, name: &str) -> Vec<&'a Element> {
    let mut v = Vec::new();
    descendants(e, name, &mut v);
    v
}

fn text(e: &Element, name: &str) -> Option<String> {
    e.find(name)
        .map(|x| x.text.trim().to_owned())
        .filter(|t| !t.is_empty())
}

fn num(e: &Element, name: &str) -> Option<f64> {
    text(e, name)?.parse().ok().filter(|v: &f64| v.is_finite())
}

/// Microns from a value in `unit` ("nm", "um", "mm", "m"; none means µm).
fn microns(v: f64, unit: Option<&str>) -> f64 {
    match unit.map(str::trim) {
        Some("nm") => v / 1000.0,
        Some("mm") => v * 1000.0,
        Some("m") => v * 1e6,
        _ => v,
    }
}

impl Oir {
    fn frame(&mut self, root: &Element) {
        let Some(def) = root.find("imageDefinition") else {
            return;
        };
        let rgb = text(def, "colorType").is_some_and(|c| c.eq_ignore_ascii_case("RGB"));
        if self.width == 0 {
            self.width = num(def, "width").unwrap_or(0.0) as usize;
            self.height = num(def, "height").unwrap_or(0.0) as usize;
        }
        if self.sample.is_none()
            && let Some(d) = num(def, "depth")
        {
            let d = d as usize / if rgb { 3 } else { 1 };
            self.sample = match d {
                1 => Some(Sample::U8),
                2 => Some(Sample::U16),
                4 => Some(Sample::U32),
                _ => None,
            };
        }
        if self.bits == 0 {
            self.bits = num(def, "bitCounts").unwrap_or(0.0) as u32 / if rgb { 3 } else { 1 };
        }
        if self.date.is_none() {
            self.date = root
                .find("general")
                .and_then(|g| text(g, "creationDateTime"));
        }
    }

    fn image(&mut self, root: &Element) {
        if let Some(d) = root
            .find("general")
            .and_then(|g| text(g, "creationDateTime"))
        {
            self.date = Some(d);
        }
        if let Some(info) = root.find("imageInfo") {
            if self.width == 0 {
                self.width = num(info, "width").unwrap_or(0.0) as usize;
                self.height = num(info, "height").unwrap_or(0.0) as usize;
            }
            let mut chans: Vec<(usize, Channel)> = Vec::new();
            for c in all(info, "channel") {
                let Some(id) = c.attr("id") else {
                    continue;
                };
                let order = c
                    .attr("order")
                    .and_then(|o| o.parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                let ch = Channel {
                    id: id.to_owned(),
                    name: text(c, "name").unwrap_or_default(),
                    start_nm: num(c, "startWavelength"),
                    end_nm: num(c, "endWavelength"),
                    colour: None,
                };
                if self.pixel.is_none()
                    && let Some(l) = c.find("length")
                {
                    let unit = c.find("pixelUnit");
                    let u = |k: &str| unit.and_then(|u| text(u, k));
                    if let (Some(x), Some(y)) = (num(l, "x"), num(l, "y")) {
                        self.pixel =
                            Some((microns(x, u("x").as_deref()), microns(y, u("y").as_deref())));
                    }
                }
                let elements = all(c, "elementChannel");
                if elements.is_empty() {
                    chans.push(((order - 1) * 3, ch));
                } else {
                    for e in elements {
                        let mut sub = ch.clone();
                        sub.id = e.attr("id").unwrap_or_default().to_owned();
                        if let Some(n) = text(e, "name") {
                            sub.name = format!("{} - {n}", ch.name);
                        }
                        let k = e
                            .attr("order")
                            .and_then(|o| o.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1);
                        chans.push(((order - 1) * 3 + k - 1, sub));
                    }
                }
            }
            chans.sort_by_key(|c| c.0);
            if self.channels.is_empty() {
                self.channels = chans.into_iter().map(|c| c.1).collect();
            }
            for a in all(info, "axis") {
                self.axis(a);
            }
        }
        let acq = root.find("acquisition");
        if let Some(acq) = acq {
            for a in all(acq, "axis") {
                let on = a.attr("enable") == Some("true")
                    && a.attr("paramEnable").is_none_or(|p| p == "true");
                if on {
                    self.axis(a);
                }
            }
            if let Some(lens) = acq.find("objectiveLens") {
                self.objective = text(lens, "displayName");
                self.magnification = num(lens, "magnification");
                self.numerical_aperture = num(lens, "naValue");
            }
            if self.pixel.is_none()
                && let Some(r) = acq.find("pixelResolution")
                && let (Some(x), Some(y)) = (num(r, "x"), num(r, "y"))
            {
                self.pixel = Some((x, y));
            }
            for p in all(acq, "pmt") {
                if let (Some(id), Some(v)) = (p.attr("channelId"), num(p, "voltage")) {
                    let name = self
                        .channels
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.name.clone());
                    if let Some(n) = name {
                        self.meta
                            .push((format!("{n} detector voltage"), format!("{v} V")));
                        if let Some(g) = num(p, "gain") {
                            self.meta
                                .push((format!("{n} detector gain"), format!("{g}")));
                        }
                    }
                }
            }
        }
    }

    fn axis(&mut self, a: &Element) {
        // The axis element holds the axis name in a child of the same name.
        let name = a
            .children
            .iter()
            .find(|c| c.name == "axis")
            .map(|c| c.text.trim().to_owned())
            .unwrap_or_default();
        let (Some(size), start, step) = (
            num(a, "maxSize"),
            num(a, "startPosition").unwrap_or(0.0),
            num(a, "step").unwrap_or(0.0),
        ) else {
            return;
        };
        let ax = Axis {
            size: size.max(0.0) as usize,
            start,
            step,
        };
        match name.as_str() {
            "ZSTACK" if self.z.is_none() => self.z = Some(ax),
            "TIMELAPSE" if self.t.is_none() => self.t = Some(ax),
            "LAMBDA" if self.lambda.is_none() => self.lambda = Some(ax),
            _ => {}
        }
    }

    fn lut(&mut self, root: &Element, uid: &str) {
        let linear = text(root, "linear").is_some_and(|l| l == "true");
        let Some(data) = text(root, "data") else {
            return;
        };
        if !linear || data.len() < 8 {
            return;
        }
        let last = &data[data.len() - 8..];
        let h = |k: usize| u8::from_str_radix(last.get(k..k + 2).unwrap_or("00"), 16).unwrap_or(0);
        if let Some(c) = self
            .channels
            .iter_mut()
            .find(|c| c.id == uid && c.colour.is_none())
        {
            c.colour = Some([h(0), h(2), h(4)]);
        }
    }

    /// Bytes of one plane.
    pub fn plane_bytes(&self) -> usize {
        self.width * self.height * self.sample.map_or(1, Sample::bytes)
    }

    /// Read plane (channel, λ, Z, T) from 0 into `out` (`width × height`); returns whether
    /// any of it was missing.
    pub fn read_plane(
        &self,
        data: &[u8],
        c: usize,
        l: usize,
        z: usize,
        t: usize,
        out: &mut [f32],
    ) -> Result<bool> {
        let sample = self
            .sample
            .ok_or_else(|| Error::Unsupported("OIR sample type".into()))?;
        let bpp = sample.bytes();
        let row = self.width * bpp;
        let n = self.width * self.height;
        if out.len() < n {
            return Err(Error::Malformed("plane buffer".into()));
        }
        out[..n].fill(0.0);
        let Some(blocks) = self.planes.get(&(c, l, z, t)) else {
            return Ok(true);
        };
        let mut y = 0usize;
        let mut truncated = false;
        let mut expect = 0u32;
        for b in blocks {
            if b.index != expect {
                truncated = true;
            }
            expect = b.index + 1;
            let px = data
                .get(b.offset..b.offset + b.len)
                .ok_or(Error::Truncated("OIR pixel block"))?;
            for r in px.chunks_exact(row.max(1)) {
                if y >= self.height {
                    break;
                }
                for (x, s) in r.chunks_exact(bpp).enumerate() {
                    out[y * self.width + x] = sample.read(s);
                }
                y += 1;
            }
        }
        Ok(truncated || y < self.height)
    }
}

/// Parse an OIR file's metadata and find its pixel blocks.
pub fn parse(data: &[u8]) -> Result<Oir> {
    if !sniff(data) {
        return Err(Error::Malformed("not an OIR file".into()));
    }
    let mut o = Oir::default();
    // XML documents: "<?xml" after their length.
    let mut i = 0;
    while let Some(k) = find(&data[i..], b"<?xml") {
        let at = i + k;
        i = at + 5;
        let Some(len) = at.checked_sub(4).and_then(|p| le_i32(data, p)) else {
            continue;
        };
        let Ok(len) = usize::try_from(len) else {
            continue;
        };
        if !(8..=MAX_XML).contains(&len) || at + len > data.len() {
            continue;
        }
        let Ok(s) = std::str::from_utf8(&data[at..at + len]) else {
            continue;
        };
        let s = s.trim_end_matches('\0').trim();
        let Ok(root) = xml::parse(s) else {
            continue;
        };
        match root.name.as_str() {
            "frameProperties" => o.frame(&root),
            "imageProperties" => o.image(&root),
            "LUT" => {
                // The LUT's uid (a channel id) is 44 bytes before the document.
                let uid = at.checked_sub(44).and_then(|p| {
                    let n = usize::try_from(le_i32(data, p)?)
                        .ok()
                        .filter(|&n| n < 256)?;
                    std::str::from_utf8(data.get(p + 4..p + 4 + n)?).ok()
                });
                if let Some(uid) = uid {
                    o.lut(&root, uid);
                }
            }
            _ => {}
        }
        i = at + len;
    }
    // Pixel blocks, by their headers.
    let mut found: Vec<((usize, usize, usize), String, Block)> = Vec::new();
    let mut p = 16;
    while let Some(k) = find(&data[p..], &[3, 0, 0, 0]) {
        let at = p + k;
        p = at + 1;
        let Some(o_) = at.checked_sub(4) else {
            continue;
        };
        let Some((uid, start, n)) = block_at(data, o_) else {
            continue;
        };
        p = start + n;
        if uid.starts_with("REF_") {
            continue;
        }
        let Some((pos, ch, index)) = parse_uid(uid) else {
            continue;
        };
        if found.len() >= MAX_BLOCKS {
            return Err(Error::TooLarge);
        }
        found.push((
            pos,
            ch.to_owned(),
            Block {
                index,
                offset: start,
                len: n,
            },
        ));
    }
    if found.is_empty() {
        return Err(Error::NoImage);
    }
    if o.width == 0 || o.height == 0 {
        return Err(Error::Malformed("OIR without an image size".into()));
    }
    if (o.width as u64) * (o.height as u64) > crate::MAX_PLANE {
        return Err(Error::TooLarge);
    }
    // Channels: those with pixels, in the file's order; ids with pixels but no description
    // after them.
    let used: Vec<String> = {
        let mut v: Vec<String> = Vec::new();
        for f in &found {
            if !v.contains(&f.1) {
                v.push(f.1.clone());
            }
        }
        v
    };
    o.channels.retain(|c| used.contains(&c.id));
    for id in &used {
        if !o.channels.iter().any(|c| &c.id == id) {
            o.channels.push(Channel {
                id: id.clone(),
                ..Default::default()
            });
        }
    }
    let min = |k: usize| {
        found
            .iter()
            .map(|f| [f.0.0, f.0.1, f.0.2][k])
            .min()
            .unwrap_or(0)
            .max(1)
    };
    let max = |k: usize| {
        found
            .iter()
            .map(|f| [f.0.0, f.0.1, f.0.2][k])
            .max()
            .unwrap_or(0)
            .max(1)
    };
    o.min = (min(0), min(1), min(2));
    o.size_l = max(0) - o.min.0 + 1;
    o.size_z = max(1) - o.min.1 + 1;
    o.size_t = max(2) - o.min.2 + 1;
    for (pos, ch, b) in found {
        let Some(c) = o.channels.iter().position(|c| c.id == ch) else {
            continue;
        };
        let key = (
            c,
            pos.0.max(1) - o.min.0,
            pos.1.max(1) - o.min.1,
            pos.2.max(1) - o.min.2,
        );
        o.planes.entry(key).or_default().push(b);
    }
    for v in o.planes.values_mut() {
        v.sort_by_key(|b| b.index);
        v.dedup_by_key(|b| b.index);
    }
    if o.sample.is_none() {
        // The block size tells the sample size.
        let any = o
            .planes
            .values()
            .next()
            .map(|v| v.iter().map(|b| b.len).sum::<usize>())
            .unwrap_or(0);
        let px = o.width * o.height;
        o.sample = match any.checked_div(px) {
            Some(1) => Some(Sample::U8),
            Some(2) => Some(Sample::U16),
            Some(4) => Some(Sample::U32),
            _ => None,
        };
    }
    if o.sample.is_none() {
        return Err(Error::Unsupported("OIR sample type".into()));
    }
    Ok(o)
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    let first = n[0];
    let mut i = 0;
    while i + n.len() <= h.len() {
        let k = h[i..h.len() - n.len() + 1]
            .iter()
            .position(|&b| b == first)?;
        i += k;
        if &h[i..i + n.len()] == n {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uids() {
        assert_eq!(parse_uid("t001_0_1_abc-1_0"), Some(((0, 0, 1), "abc-1", 0)));
        assert_eq!(
            parse_uid("l002z003t004_0_1_id_5"),
            Some(((2, 3, 4), "id", 5))
        );
        assert_eq!(parse_uid("z001_0_1_id_x"), None);
        assert_eq!(parse_uid("q001_0_1_id_1"), None);
    }

    /// A tiny OIR: a frame description, image properties and two blocks of one plane.
    pub(crate) fn fixture() -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&(-1i32).to_le_bytes());
        b.extend_from_slice(&[0; 4]);
        let xml = |b: &mut Vec<u8>, s: &str| {
            b.extend_from_slice(&(s.len() as i32).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
        };
        xml(
            &mut b,
            r#"<?xml version="1.0"?><lsmframe:frameProperties><commonframe:imageDefinition><base:width>3</base:width><base:height>2</base:height><base:depth>2</base:depth><base:bitCounts>12</base:bitCounts></commonframe:imageDefinition></lsmframe:frameProperties>"#,
        );
        xml(
            &mut b,
            r#"<?xml version="1.0"?><lsmimage:imageProperties><commonimage:imageInfo><commonphase:channel id="CH" order="1"><commonphase:name>GFP</commonphase:name><commonphase:length><commonparam:x>0.5</commonparam:x><commonparam:y>0.5</commonparam:y></commonphase:length></commonphase:channel></commonimage:imageInfo></lsmimage:imageProperties>"#,
        );
        let block = |b: &mut Vec<u8>, uid: &str, px: &[u16]| {
            b.extend_from_slice(&(uid.len() as i32 + 12).to_le_bytes());
            b.extend_from_slice(&3i32.to_le_bytes());
            b.extend_from_slice(&[0; 8]);
            b.extend_from_slice(&(uid.len() as i32).to_le_bytes());
            b.extend_from_slice(uid.as_bytes());
            b.extend_from_slice(&((px.len() * 2) as i32).to_le_bytes());
            b.extend_from_slice(&[0; 4]);
            for v in px {
                b.extend_from_slice(&v.to_le_bytes());
            }
        };
        block(&mut b, "REF_LSM0_CH_0", &[9; 6]);
        block(&mut b, "z001_0_1_CH_1", &[4, 5, 6]);
        block(&mut b, "z001_0_1_CH_0", &[1, 2, 3]);
        block(&mut b, "z002_0_1_CH_0", &[7, 8, 9]);
        b
    }

    #[test]
    fn reads_blocks_in_order_and_marks_missing_ones() {
        let f = fixture();
        let o = parse(&f).unwrap();
        assert_eq!(
            (o.width, o.height, o.sample, o.bits),
            (3, 2, Some(Sample::U16), 12)
        );
        assert_eq!(o.channels.len(), 1);
        assert_eq!(o.channels[0].name, "GFP");
        assert_eq!(o.size_z, 2);
        assert_eq!(o.pixel, Some((0.5, 0.5)));
        let mut out = vec![0f32; 6];
        assert!(!o.read_plane(&f, 0, 0, 0, 0, &mut out).unwrap());
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(o.read_plane(&f, 0, 0, 1, 0, &mut out).unwrap());
        assert_eq!(out, [7.0, 8.0, 9.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn hostile_files_do_not_panic() {
        assert!(parse(b"OLYMPUSRAWFORMAT").is_err());
        let mut f = fixture();
        let n = f.len();
        f.truncate(n - 3);
        let _ = parse(&f);
        // A block claiming more bytes than the file has is ignored.
        let mut g = fixture();
        let k = g.len() - 6 - 4 - 4;
        g[k..k + 4].copy_from_slice(&i32::MAX.to_le_bytes());
        let _ = parse(&g);
    }
}
