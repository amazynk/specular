//! Leica LIF and LOF.
//!
//! A LIF file starts with a 13-byte header (`0x70`, size, `0x2A`, character count) and a
//! UTF-16LE XML description, then a run of memory blocks, each with its own header (`0x70`,
//! `0x2A`, a 32-bit (version 1) or 64-bit (version 2) size, `0x2A`, and a UTF-16 block id) and
//! its bytes. The XML's element tree names every image (`Data/Image`) with its memory block,
//! its dimensions (`DimensionDescription`: `DimID`, element count, origin, length, unit and
//! the byte increment of one step) and its channels (`ChannelDescription`: data type,
//! resolution in bits, LUT name, byte offset). A sample sits at the block start plus each
//! index times its dimension's `BytesInc` plus the channel's `BytesInc`, which covers
//! interleaved channels, separate planes and padded rows alike.
//!
//! A LOF file holds one image: the header's text is `LMS_Object_File`, one memory block
//! follows without an id, then the XML header (sometimes a bare `<Data>` element).

use specular_wsi::xml::{self, Element};

use crate::{Error, Result, Sample};

/// Most images listed from one file.
const MAX_IMAGES: usize = 1 << 14;

/// What a dimension steps through (Leica's `DimID`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimKind {
    X,
    Y,
    Z,
    T,
    /// Emission wavelength (λ).
    Emission,
    /// Rotation (A).
    Rotation,
    /// XT slices (N).
    Slices,
    /// T slices (Q).
    TimeSlices,
    /// Excitation wavelength (Λ).
    Excitation,
    /// Mosaic tile / stage position (M).
    Tile,
    /// Loop (L).
    Loop,
    Other(u32),
}

impl DimKind {
    fn from_id(id: u32) -> DimKind {
        match id {
            1 => DimKind::X,
            2 => DimKind::Y,
            3 => DimKind::Z,
            4 => DimKind::T,
            5 => DimKind::Emission,
            6 => DimKind::Rotation,
            7 => DimKind::Slices,
            8 => DimKind::TimeSlices,
            9 => DimKind::Excitation,
            10 => DimKind::Tile,
            11 => DimKind::Loop,
            n => DimKind::Other(n),
        }
    }

    /// The one-letter code liffile and LAS X use.
    pub fn code(self) -> &'static str {
        match self {
            DimKind::X => "X",
            DimKind::Y => "Y",
            DimKind::Z => "Z",
            DimKind::T => "T",
            DimKind::Emission => "λ",
            DimKind::Rotation => "A",
            DimKind::Slices => "N",
            DimKind::TimeSlices => "Q",
            DimKind::Excitation => "Λ",
            DimKind::Tile => "M",
            DimKind::Loop => "L",
            DimKind::Other(_) => "?",
        }
    }
}

/// One dimension of an image.
#[derive(Clone, Debug, PartialEq)]
pub struct Dim {
    pub kind: DimKind,
    pub count: usize,
    pub origin: f64,
    /// Extent from the first to the last element, in `unit`.
    pub length: f64,
    pub unit: String,
    pub bytes_inc: u64,
}

impl Dim {
    /// The step between elements, when the dimension is calibrated.
    pub fn step(&self) -> Option<f64> {
        (self.count > 1 && self.length != 0.0 && self.length.is_finite())
            .then(|| self.length / (self.count - 1) as f64)
    }
}

/// One channel of an image.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub sample: Sample,
    /// Resolution: significant bits.
    pub bits: u32,
    pub lut: String,
    pub lut_inverted: bool,
    /// 1, 2, 3 for the red, green, blue of a colour camera; 0 otherwise.
    pub tag: u32,
    /// The dye name, the colour of a colour camera's channel, or `Ch<n>`.
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub unit: String,
    pub bytes_inc: u64,
}

impl Channel {
    /// Display colour (0..=1) from the LUT name or the colour-camera tag.
    pub fn colour(&self) -> Option<[f32; 3]> {
        crate::named_colour(&self.lut).or(match self.tag {
            1 => Some([1.0, 0.0, 0.0]),
            2 => Some([0.0, 1.0, 0.0]),
            3 => Some([0.0, 0.0, 1.0]),
            _ => None,
        })
    }
}

/// One image (series) of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// Element names below the top (project) element, joined with `/`.
    pub path: String,
    pub name: String,
    /// Dimensions in file order (X first).
    pub dims: Vec<Dim>,
    /// Channels in byte order.
    pub channels: Vec<Channel>,
    /// The memory block's position and size in the file (`None` when it is missing).
    pub block: Option<(usize, usize)>,
    /// Objective, numerical aperture, magnification and the like from the hardware settings.
    pub meta: Vec<(String, String)>,
}

/// A parsed LIF or LOF file: everything except the pixels.
#[derive(Clone, Debug)]
pub struct Lif {
    /// Whether this is a LOF (single object) file.
    pub lof: bool,
    pub version: u32,
    /// Name of the top element (the project).
    pub name: String,
    pub images: Vec<Image>,
}

fn bad(m: &str) -> Error {
    Error::Malformed(format!("LIF: {m}"))
}

/// Whether `head` starts like a LIF or LOF file.
pub fn sniff(head: &[u8]) -> bool {
    crate::le_u32(head, 0) == Some(0x70) && head.get(8) == Some(&0x2A) && {
        // The XML (or `LMS_Object_File`) starts right after the header.
        let text = head
            .get(13..13 + 30)
            .map(crate::utf16le)
            .unwrap_or_default();
        text.starts_with('<') || text.starts_with("LMS_Object_File")
    }
}

/// The header at `pos`: (`0x70`, size, `0x2A`, UTF-16 length) and the text after it.
fn text_header(data: &[u8], pos: usize) -> Result<(String, usize)> {
    if crate::le_u32(data, pos) != Some(0x70) || data.get(pos + 8) != Some(&0x2A) {
        return Err(Error::NotLif);
    }
    let n = crate::le_u32(data, pos + 9).ok_or(Error::Truncated("LIF header"))? as usize;
    let start = pos + 13;
    let end = n
        .checked_mul(2)
        .and_then(|b| start.checked_add(b))
        .ok_or(Error::Truncated("LIF header"))?;
    let raw = data.get(start..end).ok_or(Error::Truncated("LIF header"))?;
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Ok((String::from_utf16_lossy(&units), end))
}

/// Parse a LIF or LOF file held in memory. `stem` names a LOF's lone image when its XML does
/// not.
pub fn parse(data: &[u8], stem: &str) -> Result<Lif> {
    let (text, mut pos) = text_header(data, 0)?;
    let mut blocks: Vec<(String, usize, usize)> = Vec::new();
    let lof = text == "LMS_Object_File";
    let xml_text = if lof {
        if data.get(pos) != Some(&0x2A) || data.get(pos + 5) != Some(&0x2A) {
            return Err(bad("LOF header"));
        }
        pos += 10;
        if data.get(pos) != Some(&0x2A) {
            return Err(bad("LOF memory block"));
        }
        let size = crate::le_u64(data, pos + 1).ok_or(Error::Truncated("LOF memory block"))?;
        let start = pos + 9;
        let size = usize::try_from(size).map_err(|_| Error::TooLarge)?;
        let end = start.checked_add(size).ok_or(Error::TooLarge)?;
        blocks.push((String::new(), start, size));
        let (xml_text, _) = text_header(data, end.min(data.len()))?;
        if xml_text.trim_start().starts_with("<Data>") {
            format!(
                "<LMSDataContainerHeader Version=\"2\"><Element Name=\"{}\">{xml_text}</Element></LMSDataContainerHeader>",
                stem.replace(['"', '<', '>', '&'], "_")
            )
        } else {
            xml_text
        }
    } else {
        text
    };
    let root = xml::parse(&xml_text).map_err(|e| bad(&e.to_string()))?;
    let version = root.num("Version").map_or(2, |v| v as u32);
    if !lof {
        // Memory blocks follow until the end of the file.
        let wide = version >= 2;
        let head = if wide { 22 } else { 18 };
        while pos + head <= data.len() {
            if crate::le_u32(data, pos) != Some(0x70) || data[pos + 8] != 0x2A {
                break;
            }
            let (size, rest) = if wide {
                (crate::le_u64(data, pos + 9), pos + 17)
            } else {
                (crate::le_u32(data, pos + 9).map(u64::from), pos + 13)
            };
            let Some(size) = size.and_then(|s| usize::try_from(s).ok()) else {
                break;
            };
            if data.get(rest) != Some(&0x2A) {
                break;
            }
            let Some(idlen) = crate::le_u32(data, rest + 1) else {
                break;
            };
            let id_start = rest + 5;
            let Some(id_end) = (idlen as usize)
                .checked_mul(2)
                .and_then(|b| id_start.checked_add(b))
            else {
                break;
            };
            let Some(id) = data.get(id_start..id_end) else {
                break;
            };
            let id = crate::utf16le(id);
            blocks.push((id, id_end, size));
            match id_end.checked_add(size) {
                Some(next) if next > pos => pos = next,
                _ => break,
            }
            if blocks.len() > MAX_IMAGES * 4 {
                break;
            }
        }
    }
    let top = root.child("Element");
    let name = top.and_then(|e| e.attr("Name")).unwrap_or(stem).to_owned();
    let mut images = Vec::new();
    if let Some(top) = top {
        walk(top, "", &blocks, lof, data.len(), &mut images, 0);
    }
    if images.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(Lif {
        lof,
        version,
        name,
        images,
    })
}

fn walk(
    el: &Element,
    parent: &str,
    blocks: &[(String, usize, usize)],
    lof: bool,
    file_len: usize,
    out: &mut Vec<Image>,
    depth: usize,
) {
    if depth > 64 || out.len() >= MAX_IMAGES {
        return;
    }
    let name = el.attr("Name").unwrap_or("").to_owned();
    let path = if parent.is_empty() {
        name.clone()
    } else {
        format!("{parent}/{name}")
    };
    if let Some(img) = el.child("Data").and_then(|d| d.child("Image"))
        && let Some(image) = image(el, img, &path, &name, blocks, lof, file_len)
    {
        out.push(image);
    }
    // Paths leave out the top element (the project), as LAS X shows them.
    let below = if depth == 0 { "" } else { path.as_str() };
    if let Some(children) = el.child("Children") {
        for c in children.all("Element") {
            walk(c, below, blocks, lof, file_len, out, depth + 1);
        }
    }
}

fn num_u64(e: &Element, k: &str) -> Option<u64> {
    e.attr(k)?.trim().parse().ok()
}

fn image(
    el: &Element,
    img: &Element,
    path: &str,
    name: &str,
    blocks: &[(String, usize, usize)],
    lof: bool,
    file_len: usize,
) -> Option<Image> {
    let mem = el.child("Memory")?;
    let size = num_u64(mem, "Size").unwrap_or(0);
    if size == 0 {
        return None;
    }
    let id = mem.attr("MemoryBlockID").unwrap_or("");
    let block = if lof {
        blocks.first()
    } else {
        blocks.iter().find(|b| b.0 == id)
    }
    .map(|b| (b.1, b.2.min(file_len.saturating_sub(b.1))));
    let desc = img.child("ImageDescription")?;
    let dims: Vec<Dim> = desc
        .child("Dimensions")?
        .all("DimensionDescription")
        .map(|d| Dim {
            kind: DimKind::from_id(num_u64(d, "DimID").unwrap_or(0) as u32),
            count: num_u64(d, "NumberOfElements").unwrap_or(1).max(1) as usize,
            origin: d.num("Origin").unwrap_or(0.0),
            length: d.num("Length").unwrap_or(0.0),
            unit: d.attr("Unit").unwrap_or("").to_owned(),
            bytes_inc: num_u64(d, "BytesInc").unwrap_or(0),
        })
        .collect();
    let mut channels: Vec<Channel> = desc
        .child("Channels")?
        .all("ChannelDescription")
        .filter_map(|c| {
            let bits = num_u64(c, "Resolution").unwrap_or(8) as u32;
            let float = num_u64(c, "DataType") == Some(1);
            let sample = match (float, bits) {
                (_, 0) | (_, 65..) => return None,
                (false, ..=8) => Sample::U8,
                (false, ..=16) => Sample::U16,
                (false, ..=32) => Sample::U32,
                (false, _) => Sample::U64,
                (true, ..=8) => return None,
                (true, ..=32) => Sample::F32,
                (true, _) => Sample::F64,
            };
            let dye = c
                .all("ChannelProperty")
                .find(|p| p.child_text("Key") == Some("DyeName"))
                .and_then(|p| p.child_text("Value"))
                .map(|v| v.strip_prefix("Leica/").unwrap_or(v).to_owned());
            Some(Channel {
                sample,
                bits,
                lut: c.attr("LUTName").unwrap_or("").to_owned(),
                lut_inverted: matches!(c.attr("IsLUTInverted"), Some("1" | "true" | "True")),
                tag: num_u64(c, "ChannelTag").unwrap_or(0) as u32,
                name: dye.unwrap_or_default(),
                min: c.num("Min").unwrap_or(0.0),
                max: c.num("Max").unwrap_or(0.0),
                unit: c.attr("Unit").unwrap_or("").to_owned(),
                bytes_inc: num_u64(c, "BytesInc").unwrap_or(0),
            })
        })
        .collect();
    if channels.is_empty() || !dims.iter().any(|d| d.kind == DimKind::X) {
        return None;
    }
    channels.sort_by_key(|c| c.bytes_inc);
    if channels.iter().all(|c| c.name.is_empty())
        && let Some(names) = hardware_dyes(img, channels.len())
    {
        for (c, n) in channels.iter_mut().zip(names) {
            c.name = n;
        }
    }
    for (i, c) in channels.iter_mut().enumerate() {
        if c.name.is_empty() {
            c.name = match c.tag {
                1 => "Red".into(),
                2 => "Green".into(),
                3 => "Blue".into(),
                _ => format!("Ch{i}"),
            };
        }
    }
    Some(Image {
        path: path.to_owned(),
        name: name.to_owned(),
        dims,
        channels,
        block,
        meta: hardware(img),
    })
}

/// Dye names from the hardware settings when the channels carry none: the detectors' bands
/// of a simultaneous scan, else one band per step of a sequential scan; only when they count
/// exactly `n` (anything else cannot be matched to channels safely).
fn hardware_dyes(img: &Element, n: usize) -> Option<Vec<String>> {
    fn bands<'a>(e: &'a Element, out: &mut Vec<&'a str>) {
        for c in &e.children {
            if c.name == "MultiBand" {
                if let Some(d) = c.attr("DyeName").filter(|d| !d.trim().is_empty()) {
                    out.push(d);
                }
            } else {
                bands(c, out);
            }
        }
    }
    let clean = |d: &str| d.strip_prefix("Leica/").unwrap_or(d).trim().to_owned();
    let hw = img
        .all("Attachment")
        .find(|a| a.attr("Name") == Some("HardwareSetting"))?;
    if let Some(def) = hw.child("ATLConfocalSettingDefinition") {
        let mut v = Vec::new();
        bands(def, &mut v);
        if v.len() == n {
            return Some(v.into_iter().map(clean).collect());
        }
    }
    let seq = hw.child("LDM_Block_Sequential")?;
    let mut steps = Vec::new();
    for list in seq.all("LDM_Block_Sequential_List") {
        for def in list.all("ATLConfocalSettingDefinition") {
            let mut v = Vec::new();
            bands(def, &mut v);
            // A step lighting several channels cannot be told apart.
            if v.len() != 1 {
                return None;
            }
            steps.push(clean(v[0]));
        }
    }
    (steps.len() == n).then_some(steps)
}

/// Objective and acquisition facts from the confocal or camera settings attachment.
fn hardware(img: &Element) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for att in img.all("Attachment") {
        let Some(def) = att
            .child("ATLConfocalSettingDefinition")
            .or_else(|| att.child("ATLCameraSettingDefinition"))
        else {
            continue;
        };
        for (k, label) in [
            ("MicroscopeModel", "Microscope"),
            ("ObjectiveName", "Objective"),
            ("NumericalAperture", "Numerical aperture"),
            ("Magnification", "Magnification"),
            ("RefractionIndex", "Refraction index"),
            ("Immersion", "Immersion"),
            ("Zoom", "Zoom"),
            ("Pinhole", "Pinhole"),
            ("ScanSpeed", "Scan speed"),
        ] {
            if let Some(v) = def.attr(k).map(str::trim).filter(|v| !v.is_empty())
                && !out.iter().any(|(l, _): &(String, String)| l == label)
            {
                out.push((label.to_owned(), v.to_owned()));
            }
        }
    }
    out
}

impl Image {
    fn dim(&self, kind: DimKind) -> Option<&Dim> {
        self.dims.iter().find(|d| d.kind == kind)
    }

    pub fn width(&self) -> usize {
        self.dim(DimKind::X).map_or(1, |d| d.count)
    }

    pub fn height(&self) -> usize {
        self.dim(DimKind::Y).map_or(1, |d| d.count)
    }

    /// Dimensions beyond X and Y, in file order (fastest first).
    pub fn extra_dims(&self) -> impl Iterator<Item = &Dim> {
        self.dims
            .iter()
            .filter(|d| !matches!(d.kind, DimKind::X | DimKind::Y))
    }

    /// Pixel size along X and Y with its unit (usually metres).
    pub fn pixel_size(&self) -> Option<(f64, f64, &str)> {
        let x = self.dim(DimKind::X)?;
        let sx = x.step()?;
        let sy = self.dim(DimKind::Y).and_then(Dim::step).unwrap_or(sx);
        Some((sx.abs(), sy.abs(), x.unit.as_str()))
    }

    /// Bits of the deepest channel.
    pub fn bits(&self) -> u32 {
        self.channels.iter().map(|c| c.bits).max().unwrap_or(8)
    }

    /// Read channel `channel` at `pos` (one index per [`Image::extra_dims`] entry) into `out`
    /// (`width × height`, rows top-down). Returns whether data was missing.
    pub fn read_plane(
        &self,
        data: &[u8],
        channel: usize,
        pos: &[usize],
        out: &mut [f32],
    ) -> Result<bool> {
        let (w, h) = (self.width(), self.height());
        let ch = self.channels.get(channel).ok_or_else(|| bad("channel"))?;
        let n = w
            .checked_mul(h)
            .filter(|&n| out.len() >= n && n as u64 <= crate::MAX_PLANE)
            .ok_or_else(|| bad("plane request"))?;
        out[..n].fill(0.0);
        let Some((start, len)) = self.block else {
            return Ok(true);
        };
        let block = &data[start.min(data.len())..start.saturating_add(len).min(data.len())];
        let mut base = ch.bytes_inc;
        for (d, &i) in self
            .extra_dims()
            .zip(pos.iter().chain(std::iter::repeat(&0)))
        {
            if i >= d.count {
                return Err(bad("plane index"));
            }
            base = base.saturating_add((i as u64).saturating_mul(d.bytes_inc));
        }
        let xi = self.dim(DimKind::X).map_or(0, |d| d.bytes_inc);
        let yi = self.dim(DimKind::Y).map_or(0, |d| d.bytes_inc);
        let bs = ch.sample.bytes();
        let mut missing = false;
        for y in 0..h {
            let row = base.saturating_add((y as u64).saturating_mul(yi));
            let o = &mut out[y * w..(y + 1) * w];
            let last = row
                .saturating_add(((w - 1) as u64).saturating_mul(xi))
                .saturating_add(bs as u64);
            if last > block.len() as u64 {
                missing = true;
                // Read what there is of a cut-off row.
                for (x, v) in o.iter_mut().enumerate() {
                    let at = row.saturating_add((x as u64).saturating_mul(xi));
                    match usize::try_from(at)
                        .ok()
                        .and_then(|a| block.get(a..a.checked_add(bs)?))
                    {
                        Some(b) => *v = ch.sample.read(b),
                        None => break,
                    }
                }
                continue;
            }
            let row = row as usize;
            let xi = xi as usize;
            if xi == bs {
                for (v, b) in o.iter_mut().zip(block[row..row + w * bs].chunks_exact(bs)) {
                    *v = ch.sample.read(b);
                }
            } else {
                for (x, v) in o.iter_mut().enumerate() {
                    let at = row + x * xi;
                    *v = ch.sample.read(&block[at..at + bs]);
                }
            }
        }
        Ok(missing)
    }
}

#[cfg(test)]
mod hostile {
    use super::*;
    use crate::Sample;

    fn dim(kind: DimKind, count: usize, bytes_inc: u64) -> Dim {
        Dim {
            kind,
            count,
            origin: 0.0,
            length: 0.0,
            unit: String::new(),
            bytes_inc,
        }
    }

    fn image(dims: Vec<Dim>, bytes_inc: u64) -> Image {
        Image {
            path: String::new(),
            name: String::new(),
            dims,
            channels: vec![Channel {
                sample: Sample::U16,
                bits: 16,
                lut: String::new(),
                lut_inverted: false,
                tag: 0,
                name: String::new(),
                min: 0.0,
                max: 0.0,
                unit: String::new(),
                bytes_inc,
            }],
            block: Some((0, 64)),
            meta: Vec::new(),
        }
    }

    /// `NumberOfElements` of 2^40 on both X and Y: the plane size overflows.
    #[test]
    fn plane_of_overflowing_size() {
        let img = image(
            vec![dim(DimKind::X, 1 << 40, 2), dim(DimKind::Y, 1 << 40, 0)],
            0,
        );
        let mut out = vec![0f32; 16];
        assert!(img.read_plane(&[0; 64], 0, &[], &mut out).is_err());
    }

    /// `BytesInc` at the integer limit: offsets past the block read as missing.
    #[test]
    fn byte_increments_at_the_integer_limit() {
        let img = image(
            vec![dim(DimKind::X, 4, u64::MAX), dim(DimKind::Y, 2, u64::MAX)],
            0,
        );
        let mut out = vec![0f32; 8];
        assert_eq!(img.read_plane(&[7; 64], 0, &[], &mut out), Ok(true));
        assert_eq!(out[0], f32::from(u16::from_le_bytes([7, 7])));
        let img = image(vec![dim(DimKind::X, 4, 2)], u64::MAX);
        assert_eq!(img.read_plane(&[7; 64], 0, &[], &mut out), Ok(true));
    }
}
