//! NITF 2.1 / NSIF 1.0 and NITF 2.0 containers: the file header, image subheaders (size,
//! pixel type, bands, blocking, interleave, compression, corner coordinates, comments,
//! tagged record extensions) and where each image's pixel data lies. [`Image::block`]
//! gives one block's samples for uncompressed images (NC, NM; any interleave, 1 to 64 bits
//! per pixel); JPEG (C3, M3) blocks and JPEG 2000 (C8, M8) codestreams are located for the
//! caller's decoders.
//!
//! Georeferencing: IGEOLO corners (geographic DMS or decimal, UTM north/south, MGRS) with
//! the corners taken as pixel centres as GDAL does, and the RPC00B/RPC00A rational
//! polynomial model (forward and an iterative inverse at the model's mean height).

use crate::crs::{self, Crs};
use crate::geotiff::Affine2;

/// What a NITF holds, for the viewer and the info panel.
#[derive(Clone, Debug, Default)]
pub struct Nitf {
    /// "NITF 2.1", "NSIF 1.0", "NITF 2.0".
    pub version: String,
    pub title: String,
    pub originator: String,
    pub date: String,
    /// File security classification (U, R, C, S, T).
    pub classification: String,
    pub images: Vec<Image>,
    /// Graphic, text, data extension and reserved segment counts.
    pub graphics: usize,
    pub texts: usize,
    pub des: usize,
    /// File-level TRE names.
    pub tres: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Image {
    pub iid1: String,
    pub iid2: String,
    pub date: String,
    pub source: String,
    pub classification: String,
    pub rows: u32,
    pub cols: u32,
    /// INT, SI, R, C, B.
    pub pvtype: String,
    /// MONO, RGB, MULTI, NODISPLY, …
    pub irep: String,
    /// VIS, SAR, IR, MS, …
    pub icat: String,
    /// Actual bits per pixel (ABPP) and stored bits (NBPP).
    pub abpp: u32,
    pub nbpp: u32,
    /// Left-justified samples (PJUST = L).
    pub left_justified: bool,
    pub icords: char,
    pub igeolo: String,
    pub comments: Vec<String>,
    /// NC, NM, C3, M3, C8, M8, …
    pub ic: String,
    /// Band representations (R, G, B, M, LU, …).
    pub bands: Vec<String>,
    /// Band subcategories (I and Q for complex SAR samples, wavelengths, …).
    pub subcats: Vec<String>,
    /// Band look-up tables (one entry list per output channel, for LU bands).
    pub luts: Vec<Vec<Vec<u8>>>,
    /// B, P, R or S.
    pub imode: char,
    pub blocks_x: u32,
    pub blocks_y: u32,
    pub block_w: u32,
    pub block_h: u32,
    /// TREs of the image subheader: (tag, data).
    pub tres: Vec<(String, Vec<u8>)>,
    /// Where the image data lies in the file.
    pub data_offset: usize,
    pub data_len: usize,
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self
            .b
            .get(self.i..self.i.checked_add(n).ok_or("bad field")?)
            .ok_or("the NITF header is truncated")?;
        self.i += n;
        Ok(s)
    }
    fn s(&mut self, n: usize) -> Result<String, String> {
        Ok(String::from_utf8_lossy(self.take(n)?).trim().to_string())
    }
    fn n(&mut self, n: usize) -> Result<u64, String> {
        let t = self.s(n)?;
        if t.is_empty() {
            return Ok(0);
        }
        t.parse::<u64>()
            .map_err(|_| format!("the NITF header has a bad number {t:?}"))
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(|_| ())
    }
}

pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"NITF02.10")
        || head.starts_with(b"NSIF01.00")
        || head.starts_with(b"NITF02.00")
}

/// TREs: tag (6), length (5), data.
fn tres(b: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 11 <= b.len() {
        let tag = String::from_utf8_lossy(&b[i..i + 6]).trim().to_string();
        let Some(n) = std::str::from_utf8(&b[i + 6..i + 11])
            .ok()
            .and_then(|t| t.trim().parse::<usize>().ok())
        else {
            break;
        };
        let Some(d) = b.get(i + 11..i + 11 + n) else {
            break;
        };
        out.push((tag, d.to_vec()));
        i += 11 + n;
    }
    out
}

/// The security fields of a 2.1 header (after the classification letter's position) or a
/// 2.0 one.
fn security(r: &mut Rd<'_>, v20: bool) -> Result<String, String> {
    let class = r.s(1)?;
    if v20 {
        r.skip(40 + 40 + 40 + 20 + 20)?; // code, control, release, authority, control no.
        let dwng = r.s(6)?;
        if dwng == "999998" {
            r.skip(40)?;
        }
    } else {
        r.skip(2 + 11 + 2 + 20 + 2 + 8 + 4 + 1 + 8 + 43 + 1 + 40 + 1 + 8 + 15)?;
    }
    Ok(class)
}

/// Most blocks (times bands, for band-sequential images) an image may have.
const MAX_BLOCKS: u64 = 1 << 22;

pub fn parse(data: &[u8]) -> Result<Nitf, String> {
    if !sniff(data) {
        if data.starts_with(b"NITF01.10") {
            return Err("NITF 1.1 files are not supported".into());
        }
        return Err("not a NITF file".into());
    }
    let v20 = data.starts_with(b"NITF02.00");
    let mut r = Rd { b: data, i: 0 };
    let fhdr = r.s(9)?;
    let version = match fhdr.as_str() {
        "NITF02.10" => "NITF 2.1",
        "NSIF01.00" => "NSIF 1.0",
        _ => "NITF 2.0",
    }
    .to_string();
    r.skip(2 + 4 + 10)?; // CLEVEL, STYPE, OSTAID
    let originator_station = String::from_utf8_lossy(&data[15..25]).trim().to_string();
    let date = r.s(14)?;
    let title = r.s(80)?;
    let classification = security(&mut r, v20)?;
    r.skip(5 + 5 + 1)?; // FSCOP, FSCPYS, ENCRYP
    if !v20 {
        r.skip(3)?; // FBKGC
    }
    let oname = r.s(if v20 { 27 } else { 24 })?;
    r.skip(18)?; // OPHONE
    let _fl = r.n(12)?;
    let hl = r.n(6)? as usize;
    let numi = r.n(3)? as usize;
    let mut segs = Vec::with_capacity(numi);
    for _ in 0..numi {
        let lish = r.n(6)? as usize;
        let li = r.n(10)? as usize;
        segs.push((lish, li));
    }
    let nums = r.n(3)? as usize;
    let mut skipped = 0usize;
    for _ in 0..nums {
        skipped += r.n(4)? as usize + r.n(6)? as usize;
    }
    if v20 {
        // NITF 2.0 label segments.
        let numl = r.n(3)? as usize;
        for _ in 0..numl {
            r.n(4)?;
            r.n(3)?;
        }
    } else {
        r.n(3)?; // NUMX
    }
    let numt = r.n(3)? as usize;
    for _ in 0..numt {
        r.n(4)?;
        r.n(5)?;
    }
    let numdes = r.n(3)? as usize;
    for _ in 0..numdes {
        r.n(4)?;
        r.n(9)?;
    }
    let numres = r.n(3)? as usize;
    for _ in 0..numres {
        r.n(4)?;
        r.n(7)?;
    }
    let udhdl = r.n(5)? as usize;
    if udhdl > 0 {
        r.skip(udhdl)?;
    }
    let xhdl = r.n(5)? as usize;
    let mut file_tres = Vec::new();
    if xhdl > 3 {
        r.skip(3)?;
        file_tres = tres(r.take(xhdl - 3)?)
            .into_iter()
            .map(|(t, _)| t)
            .collect();
    } else if xhdl > 0 {
        r.skip(xhdl)?;
    }
    let _ = skipped;
    let mut images = Vec::new();
    let mut at = hl;
    for (lish, li) in segs {
        let sub = data
            .get(at..at.checked_add(lish).ok_or("bad segment")?)
            .ok_or("an image segment lies outside the file")?;
        let mut img = image(sub, v20)?;
        img.data_offset = at + lish;
        img.data_len = li.min(data.len().saturating_sub(img.data_offset));
        images.push(img);
        at = at.saturating_add(lish).saturating_add(li);
    }
    Ok(Nitf {
        version,
        title,
        originator: if oname.is_empty() {
            originator_station
        } else {
            oname
        },
        date,
        classification,
        images,
        graphics: nums,
        texts: numt,
        des: numdes,
        tres: file_tres,
    })
}

fn image(sub: &[u8], v20: bool) -> Result<Image, String> {
    let mut r = Rd { b: sub, i: 0 };
    if r.s(2)? != "IM" {
        return Err("an image subheader is damaged".into());
    }
    let mut im = Image {
        iid1: r.s(10)?,
        date: r.s(14)?,
        ..Image::default()
    };
    r.skip(17)?; // TGTID
    im.iid2 = r.s(80)?;
    im.classification = security(&mut r, v20)?;
    r.skip(1)?; // ENCRYP
    im.source = r.s(42)?;
    im.rows = r.n(8)? as u32;
    im.cols = r.n(8)? as u32;
    im.pvtype = r.s(3)?;
    im.irep = r.s(8)?;
    im.icat = r.s(8)?;
    im.abpp = r.n(2)? as u32;
    im.left_justified = r.s(1)? == "L";
    let icords = r.take(1)?[0] as char;
    im.icords = icords;
    // NITF 2.0 marks "no coordinates" with N; 2.1 with a space (N is UTM north there).
    let has_geolo = if v20 {
        icords != 'N' && icords != ' '
    } else {
        icords != ' '
    };
    if has_geolo {
        im.igeolo = String::from_utf8_lossy(r.take(60)?).to_string();
        if v20 && icords == 'N' {
            im.icords = ' ';
        }
    } else if v20 {
        im.icords = ' ';
    }
    let nicom = r.n(1)? as usize;
    for _ in 0..nicom {
        let c = r.s(80)?;
        if !c.is_empty() {
            im.comments.push(c);
        }
    }
    im.ic = r.s(2)?;
    if im.ic != "NC" && im.ic != "NM" {
        r.skip(4)?; // COMRAT
    }
    let mut nbands = r.n(1)? as usize;
    if nbands == 0 {
        nbands = r.n(5)? as usize;
    }
    if nbands == 0 || nbands > 4096 {
        return Err("the image has no bands".into());
    }
    for _ in 0..nbands {
        im.bands.push(r.s(2)?);
        im.subcats.push(r.s(6)?);
        r.skip(1 + 3)?; // IFC, IMFLT
        let nluts = r.n(1)? as usize;
        let mut luts = Vec::new();
        if nluts > 0 {
            let nelut = r.n(5)? as usize;
            for _ in 0..nluts {
                luts.push(r.take(nelut)?.to_vec());
            }
        }
        im.luts.push(luts);
    }
    r.skip(1)?; // ISYNC
    im.imode = r.take(1)?[0] as char;
    im.blocks_x = r.n(4)? as u32;
    im.blocks_y = r.n(4)? as u32;
    im.block_w = r.n(4)? as u32;
    im.block_h = r.n(4)? as u32;
    im.nbpp = r.n(2)? as u32;
    // A block size of 0 means "the whole width/height" (over 8192).
    if im.block_w == 0 {
        im.block_w = im.cols;
    }
    if im.block_h == 0 {
        im.block_h = im.rows;
    }
    r.skip(3 + 3 + 10 + 4)?; // IDLVL, IALVL, ILOC, IMAG
    let udidl = r.n(5)? as usize;
    if udidl > 0 {
        r.skip(udidl)?;
    }
    let ixshdl = r.n(5)? as usize;
    if ixshdl > 3 {
        r.skip(3)?;
        im.tres = tres(r.take(ixshdl - 3)?);
    }
    if im.rows == 0 || im.cols == 0 || im.block_w == 0 || im.block_h == 0 {
        return Err("the image has no pixels".into());
    }
    // The block grid must cover the image exactly (the block tables are sized from it), and
    // the number of blocks (per band) is capped: 1-pixel blocks would make a small header ask
    // for terabytes of tables.
    if im.blocks_x != im.cols.div_ceil(im.block_w) || im.blocks_y != im.rows.div_ceil(im.block_h) {
        return Err("the image's blocks do not match its size".into());
    }
    if u64::from(im.blocks_x) * u64::from(im.blocks_y) * im.bands.len() as u64 > MAX_BLOCKS {
        return Err("the image has too many blocks".into());
    }
    if im.abpp == 0 {
        im.abpp = im.nbpp;
    }
    Ok(im)
}

/// Where one block's data is in a masked image (NM, M3, M8): the block mask table gives
/// offsets (0xFFFFFFFF: absent) after its header.
pub struct Mask {
    /// Offset of the first block from the start of the image data.
    pub data_start: usize,
    /// One offset per block and band (band-major for IMODE S), `None` when absent.
    pub offsets: Vec<Option<usize>>,
}

impl Image {
    pub fn masked(&self) -> bool {
        self.ic.starts_with('M')
    }

    /// Blocks per band.
    pub fn block_count(&self) -> usize {
        self.blocks_x as usize * self.blocks_y as usize
    }

    pub fn data<'a>(&self, file: &'a [u8]) -> &'a [u8] {
        file.get(self.data_offset..self.data_offset + self.data_len)
            .unwrap_or(&[])
    }

    /// The block mask table of a masked image.
    pub fn mask(&self, file: &[u8]) -> Result<Mask, String> {
        let d = self.data(file);
        let u32_at = |i: usize| -> Option<u32> {
            d.get(i..i + 4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        };
        let u16_at =
            |i: usize| -> Option<u16> { d.get(i..i + 2).map(|b| u16::from_be_bytes([b[0], b[1]])) };
        let start = u32_at(0).ok_or("the block mask is truncated")? as usize;
        let bmrlnth = u16_at(4).ok_or("the block mask is truncated")?;
        let tmrlnth = u16_at(6).ok_or("the block mask is truncated")?;
        let tpxcdlnth = u16_at(8).ok_or("the block mask is truncated")?;
        let mut i = 10 + usize::from(tpxcdlnth).div_ceil(8);
        let n = self.block_count()
            * if self.imode == 'S' {
                self.bands.len()
            } else {
                1
            };
        let mut offsets = Vec::with_capacity(n.min(d.len() / 4));
        if bmrlnth > 0 {
            for _ in 0..n {
                let v = u32_at(i).ok_or("the block mask is truncated")?;
                i += 4;
                offsets.push((v != 0xFFFF_FFFF).then_some(v as usize));
            }
        } else {
            // No block mask records: blocks follow one another.
            let size = self.block_bytes();
            offsets = (0..n).map(|k| Some(k * size)).collect();
        }
        let _ = tmrlnth;
        Ok(Mask {
            data_start: start,
            offsets,
        })
    }

    /// Bytes of one uncompressed block (all bands for B/P/R, one band for S).
    pub fn block_bytes(&self) -> usize {
        let px = self.block_w as usize * self.block_h as usize;
        let bands = if self.imode == 'S' {
            1
        } else {
            self.bands.len()
        };
        (px * bands * self.nbpp as usize).div_ceil(8)
    }

    /// The samples of uncompressed block (`bx`, `by`) as f64, band-major: `out[b][y * w + x]`.
    pub fn block(&self, file: &[u8], bx: u32, by: u32) -> Result<Option<Vec<Vec<f64>>>, String> {
        let d = self.data(file);
        let k = (by * self.blocks_x + bx) as usize;
        let bands = self.bands.len();
        let (w, h) = (self.block_w as usize, self.block_h as usize);
        let px = w * h;
        let mask = if self.ic == "NM" {
            Some(self.mask(file)?)
        } else {
            None
        };
        let one = self.block_bytes();
        // The bytes of the block for band `b` (or all bands).
        let span = |band: usize| -> Option<&[u8]> {
            let idx = if self.imode == 'S' {
                band * self.block_count() + k
            } else {
                k
            };
            let off = match &mask {
                Some(m) => m.data_start + (*m.offsets.get(idx)?)?,
                None => idx * one,
            };
            d.get(off..off + one)
        };
        // The block's bytes are found before its samples are allocated (64 f64 bytes per
        // stored bit at most).
        let spans = if self.imode == 'S' { bands } else { 1 };
        if (0..spans).any(|b| span(b).is_none()) {
            return Ok(None);
        }
        let mut out = vec![vec![0f64; px]; bands];
        let nbpp = self.nbpp as usize;
        let read = |b: &[u8], sample: usize| -> f64 { sample_at(b, sample, nbpp, &self.pvtype) };
        match self.imode {
            'S' => {
                for (band, o) in out.iter_mut().enumerate() {
                    let Some(b) = span(band) else { return Ok(None) };
                    for (i, v) in o.iter_mut().enumerate() {
                        *v = read(b, i);
                    }
                }
            }
            mode => {
                let Some(b) = span(0) else { return Ok(None) };
                for i in 0..px {
                    for (band, o) in out.iter_mut().enumerate() {
                        let s = match mode {
                            'P' => i * bands + band,
                            'R' => {
                                let (y, x) = (i / w, i % w);
                                (y * bands + band) * w + x
                            }
                            _ => band * px + i, // B: band blocks one after another
                        };
                        o[i] = read(b, s);
                    }
                }
            }
        }
        Ok(Some(out))
    }

    /// The JPEG streams of a C3/M3 image, one per block (`None` for masked-out blocks).
    pub fn jpeg_blocks<'a>(&self, file: &'a [u8]) -> Result<Vec<Option<&'a [u8]>>, String> {
        let d = self.data(file);
        let n = self.block_count();
        if self.masked() {
            let m = self.mask(file)?;
            let starts: Vec<Option<usize>> = m
                .offsets
                .iter()
                .map(|o| o.map(|o| m.data_start + o))
                .collect();
            let mut sorted: Vec<usize> = starts.iter().flatten().copied().collect();
            sorted.sort_unstable();
            return Ok(starts
                .iter()
                .map(|s| {
                    let s = (*s)?;
                    let e = sorted.iter().find(|&&x| x > s).copied().unwrap_or(d.len());
                    d.get(s..e)
                })
                .collect());
        }
        // Unmasked: the streams follow one another; each starts with SOI and ends with EOI.
        let mut out = Vec::with_capacity(n);
        let mut i = 0usize;
        while out.len() < n {
            let Some(s) = find(d, i, &[0xFF, 0xD8]) else {
                break;
            };
            let e = jpeg_end(d, s).unwrap_or(d.len());
            out.push(Some(&d[s..e]));
            i = e;
        }
        if out.is_empty() {
            return Err("no JPEG data in the image".into());
        }
        out.resize(n, None);
        Ok(out)
    }

    /// The JPEG 2000 codestream of a C8/M8 image.
    pub fn j2k<'a>(&self, file: &'a [u8]) -> Result<&'a [u8], String> {
        let d = self.data(file);
        let s =
            find(d, 0, &[0xFF, 0x4F, 0xFF, 0x51]).ok_or("no JPEG 2000 codestream in the image")?;
        Ok(&d[s..])
    }

    pub fn tre(&self, tag: &str) -> Option<&[u8]> {
        self.tres
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, d)| d.as_slice())
    }
}

fn find(d: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    d.get(from..)?
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|p| p + from)
}

/// The end (after EOI) of the JPEG stream starting at `s`, walking its segments so markers
/// inside entropy-coded data are not mistaken for the end.
fn jpeg_end(d: &[u8], s: usize) -> Option<usize> {
    let mut i = s + 2;
    loop {
        while *d.get(i)? != 0xFF {
            i += 1;
        }
        let m = *d.get(i + 1)?;
        match m {
            0xD9 => return Some(i + 2),
            0x00 | 0xFF | 0xD0..=0xD7 | 0x01 => i += if m == 0xFF { 1 } else { 2 },
            0xDA => {
                let l = usize::from(u16::from_be_bytes([*d.get(i + 2)?, *d.get(i + 3)?]));
                i += 2 + l;
                // Entropy-coded data until a marker that is not stuffing or a restart.
                loop {
                    let p = d.get(i..)?.iter().position(|&b| b == 0xFF)? + i;
                    let n = *d.get(p + 1)?;
                    if n == 0x00 || (0xD0..=0xD7).contains(&n) || n == 0xFF {
                        i = p + if n == 0xFF { 1 } else { 2 };
                        continue;
                    }
                    i = p;
                    break;
                }
            }
            _ => {
                let l = usize::from(u16::from_be_bytes([*d.get(i + 2)?, *d.get(i + 3)?]));
                i += 2 + l;
            }
        }
    }
}

/// Sample `i` of `nbpp` bits, big-endian, of pixel type `pvtype`.
fn sample_at(b: &[u8], i: usize, nbpp: usize, pvtype: &str) -> f64 {
    match nbpp {
        8 => {
            let v = b.get(i).copied().unwrap_or(0);
            if pvtype == "SI" {
                f64::from(v as i8)
            } else {
                f64::from(v)
            }
        }
        16 => {
            let v = b
                .get(i * 2..i * 2 + 2)
                .map_or(0, |x| u16::from_be_bytes([x[0], x[1]]));
            if pvtype == "SI" {
                f64::from(v as i16)
            } else {
                f64::from(v)
            }
        }
        32 => {
            let x = b
                .get(i * 4..i * 4 + 4)
                .map_or([0; 4], |x| [x[0], x[1], x[2], x[3]]);
            match pvtype {
                "R" => f64::from(f32::from_be_bytes(x)),
                "SI" => f64::from(i32::from_be_bytes(x)),
                _ => f64::from(u32::from_be_bytes(x)),
            }
        }
        64 => {
            let x: [u8; 8] = b
                .get(i * 8..i * 8 + 8)
                .and_then(|s| s.try_into().ok())
                .unwrap_or([0; 8]);
            if pvtype == "R" {
                f64::from_be_bytes(x)
            } else {
                u64::from_be_bytes(x) as f64
            }
        }
        n if n < 32 => {
            // Packed bits, most significant first (1-bit masks, 12-bit imagery).
            let bit = i * n;
            let mut v = 0u64;
            for k in 0..n {
                let p = bit + k;
                let byte = b.get(p / 8).copied().unwrap_or(0);
                v = (v << 1) | u64::from(byte >> (7 - p % 8) & 1);
            }
            v as f64
        }
        _ => 0.0,
    }
}

// ------------------------------------------------------------------------ georeference

/// Degrees from "ddmmssX" / "dddmmssY" DMS or "±dd.ddd" decimal.
fn dms(s: &str, deg_digits: usize) -> Option<f64> {
    let s = s.trim();
    let hemi = s.chars().last()?;
    let body = &s[..s.len() - 1];
    if body.len() != deg_digits + 4 {
        return None;
    }
    let d: f64 = body[..deg_digits].parse().ok()?;
    let m: f64 = body[deg_digits..deg_digits + 2].parse().ok()?;
    let sec: f64 = body[deg_digits + 2..].parse().ok()?;
    let v = d + m / 60.0 + sec / 3600.0;
    Some(match hemi {
        'S' | 's' | 'W' | 'w' => -v,
        'N' | 'n' | 'E' | 'e' => v,
        _ => return None,
    })
}

impl Image {
    /// Whether the bands are complex SAR samples (I/Q or magnitude/phase pairs).
    pub fn complex(&self) -> bool {
        self.pvtype == "C"
            || (self.subcats.iter().any(|s| s == "I") && self.subcats.iter().any(|s| s == "Q"))
            || (self.subcats.iter().any(|s| s == "M") && self.subcats.iter().any(|s| s == "P"))
    }
}

/// The four IGEOLO corners (upper left, upper right, lower right, lower left) as map
/// coordinates and their CRS.
pub fn corners(im: &Image) -> Option<([[f64; 2]; 4], Crs)> {
    let g = &im.igeolo;
    if g.len() < 60 {
        return None;
    }
    let part = |k: usize| &g[k * 15..k * 15 + 15];
    let mut pts = [[0f64; 2]; 4];
    let crs = match im.icords {
        'G' | 'C' => {
            for (k, p) in pts.iter_mut().enumerate() {
                let c = part(k);
                *p = [dms(&c[7..], 3)?, dms(&c[..7], 2)?];
            }
            crs::from_epsg(4326)?
        }
        'D' => {
            for (k, p) in pts.iter_mut().enumerate() {
                let c = part(k);
                *p = [c[7..].trim().parse().ok()?, c[..7].trim().parse().ok()?];
            }
            crs::from_epsg(4326)?
        }
        'N' | 'S' => {
            let mut zone = 0u32;
            for (k, p) in pts.iter_mut().enumerate() {
                let c = part(k);
                let z: u32 = c[..2].trim().parse().ok()?;
                if k > 0 && z != zone {
                    return None; // corners in different zones
                }
                zone = z;
                *p = [c[2..8].trim().parse().ok()?, c[8..].trim().parse().ok()?];
            }
            if !(1..=60).contains(&zone) {
                return None;
            }
            crs::from_epsg(if im.icords == 'N' { 32600 } else { 32700 } + zone)?
        }
        'U' => {
            let mut zone = 0u32;
            for (k, p) in pts.iter_mut().enumerate() {
                let (z, n, e, n2) = mgrs(part(k))?;
                if k > 0 && z != zone {
                    return None;
                }
                zone = z;
                *p = [e, n2];
                let _ = n;
            }
            let north = mgrs(part(0))?.1;
            crs::from_epsg(if north { 32600 } else { 32700 } + zone)?
        }
        _ => return None,
    };
    Some((pts, crs))
}

/// An MGRS reference ("zzBJKeeeeennnnn", 1 m) as (zone, northern hemisphere, easting,
/// northing) in UTM.
fn mgrs(s: &str) -> Option<(u32, bool, f64, f64)> {
    let s: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if s.len() != 15 {
        return None;
    }
    let zone: u32 = std::str::from_utf8(&s[..2]).ok()?.parse().ok()?;
    let band = (s[2] as char).to_ascii_uppercase();
    let (col, row) = (
        (s[3] as char).to_ascii_uppercase(),
        (s[4] as char).to_ascii_uppercase(),
    );
    let e: f64 = std::str::from_utf8(&s[5..10]).ok()?.parse().ok()?;
    let n: f64 = std::str::from_utf8(&s[10..15]).ok()?.parse().ok()?;
    let letters = "ABCDEFGHJKLMNPQRSTUVWXYZ"; // no I, O
    let set = (zone - 1) % 6;
    // Column letters: A–H, J–R, S–Z cycling by set.
    let col_i = letters.find(col)? as i64;
    let col_origin = [0i64, 8, 16][(set % 3) as usize];
    let e100 = (col_i - col_origin).rem_euclid(24) + 1;
    if !(1..=8).contains(&e100) {
        return None;
    }
    // Row letters A–V (20), offset by 5 for even sets.
    let rows = "ABCDEFGHJKLMNPQRSTUV";
    let row_i = rows.find(row)? as i64;
    let row_off = if set % 2 == 1 { 5 } else { 0 };
    let n100 = (row_i - row_off).rem_euclid(20);
    let bands = "CDEFGHJKLMNPQRSTUVWX";
    let band_i = bands.find(band)? as f64;
    let north = band >= 'N';
    // The band's southern latitude fixes which 2000 km cycle the row letter means.
    let lat = -80.0 + 8.0 * band_i;
    let utm = crs::from_epsg(if north { 32600 } else { 32700 } + zone)?;
    let (_, band_n) = utm.from_lonlat(f64::from(zone) * 6.0 - 183.0, lat)?;
    let mut northing = n100 as f64 * 100_000.0 + n;
    while northing < band_n - 100_000.0 {
        northing += 2_000_000.0;
    }
    Some((zone, north, e100 as f64 * 100_000.0 + e, northing))
}

/// The pixel-to-map transform from the corners (pixel centres, as GDAL takes them), when
/// they form a parallelogram (within a tenth of a pixel).
pub fn transform(im: &Image) -> Option<(Affine2, Crs)> {
    let (c, crs) = corners(im)?;
    let (w, h) = (f64::from(im.cols), f64::from(im.rows));
    if w < 2.0 || h < 2.0 {
        return None;
    }
    // The corners are the centres of the corner pixels; like GDAL, an affine transform is
    // fitted to all four (exact for a parallelogram).
    let px = [
        [0.5, 0.5],
        [w - 0.5, 0.5],
        [w - 0.5, h - 0.5],
        [0.5, h - 0.5],
    ];
    let pairs: Vec<([f64; 2], [f64; 2])> = px.iter().copied().zip(c).collect();
    let t = Affine2::fit(&pairs)?;
    // Refused when the corners are far from any affine image (more than a pixel off).
    let inv = t.inverse()?;
    let worst = pairs
        .iter()
        .map(|(p, m)| {
            let (x, y) = inv.apply(m[0], m[1]);
            (x - p[0]).hypot(y - p[1])
        })
        .fold(0.0, f64::max);
    (worst <= 1.0).then_some((t, crs))
}

/// The RPC00B (or RPC00A) model.
#[derive(Clone, Debug, PartialEq)]
pub struct Rpc {
    pub line_off: f64,
    pub samp_off: f64,
    pub lat_off: f64,
    pub lon_off: f64,
    pub h_off: f64,
    pub line_scale: f64,
    pub samp_scale: f64,
    pub lat_scale: f64,
    pub lon_scale: f64,
    pub h_scale: f64,
    pub line_num: [f64; 20],
    pub line_den: [f64; 20],
    pub samp_num: [f64; 20],
    pub samp_den: [f64; 20],
}

impl Rpc {
    pub fn parse(tre: &[u8], a: bool) -> Option<Rpc> {
        let t = std::str::from_utf8(tre).ok()?;
        let mut i = 1 + 7 + 7; // SUCCESS, ERR_BIAS, ERR_RAND
        let mut f = |n: usize| -> Option<f64> {
            let s = t.get(i..i + n)?;
            i += n;
            s.trim().parse().ok()
        };
        let (line_off, samp_off, lat_off, lon_off, h_off) = (f(6)?, f(5)?, f(8)?, f(9)?, f(5)?);
        let (line_scale, samp_scale, lat_scale, lon_scale, h_scale) =
            (f(6)?, f(5)?, f(8)?, f(9)?, f(5)?);
        let mut c = || -> Option<[f64; 20]> {
            let mut v = [0f64; 20];
            for x in &mut v {
                *x = f(12)?;
            }
            Some(v)
        };
        let (mut ln, mut ld, mut sn, mut sd) = (c()?, c()?, c()?, c()?);
        if a {
            // RPC00A orders the terms differently; reorder to RPC00B's.
            const A2B: [usize; 20] = [
                0, 1, 2, 3, 4, 5, 6, 10, 7, 8, 9, 13, 17, 11, 12, 14, 18, 15, 16, 19,
            ];
            for v in [&mut ln, &mut ld, &mut sn, &mut sd] {
                let src = *v;
                for (k, &j) in A2B.iter().enumerate() {
                    v[k] = src[j];
                }
            }
        }
        Some(Rpc {
            line_off,
            samp_off,
            lat_off,
            lon_off,
            h_off,
            line_scale,
            samp_scale,
            lat_scale,
            lon_scale,
            h_scale,
            line_num: ln,
            line_den: ld,
            samp_num: sn,
            samp_den: sd,
        })
    }

    fn terms(l: f64, p: f64, h: f64) -> [f64; 20] {
        [
            1.0,
            l,
            p,
            h,
            l * p,
            l * h,
            p * h,
            l * l,
            p * p,
            h * h,
            p * l * h,
            l * l * l,
            l * p * p,
            l * h * h,
            l * l * p,
            p * p * p,
            p * h * h,
            l * l * h,
            p * p * h,
            h * h * h,
        ]
    }

    /// (sample, line) of a longitude, latitude and height.
    pub fn project(&self, lon: f64, lat: f64, h: f64) -> (f64, f64) {
        let t = Self::terms(
            (lon - self.lon_off) / self.lon_scale,
            (lat - self.lat_off) / self.lat_scale,
            (h - self.h_off) / self.h_scale,
        );
        let dot = |c: &[f64; 20]| c.iter().zip(&t).map(|(a, b)| a * b).sum::<f64>();
        let line = dot(&self.line_num) / dot(&self.line_den) * self.line_scale + self.line_off;
        let samp = dot(&self.samp_num) / dot(&self.samp_den) * self.samp_scale + self.samp_off;
        (samp, line)
    }

    /// Longitude and latitude of pixel (`samp`, `line`) at height `h`, by Newton steps.
    pub fn locate(&self, samp: f64, line: f64, h: f64) -> Option<(f64, f64)> {
        let (mut lon, mut lat) = (self.lon_off, self.lat_off);
        for _ in 0..30 {
            let (s0, l0) = self.project(lon, lat, h);
            let (ds, dl) = (samp - s0, line - l0);
            if ds.abs() < 1e-6 && dl.abs() < 1e-6 {
                return Some((lon, lat));
            }
            let e = 1e-7 * self.lon_scale.abs().max(1e-6);
            let (s1, l1) = self.project(lon + e, lat, h);
            let (s2, l2) = self.project(lon, lat + e, h);
            let (a, b, c, d) = ((s1 - s0) / e, (s2 - s0) / e, (l1 - l0) / e, (l2 - l0) / e);
            let det = a * d - b * c;
            if det.abs() < 1e-30 || !det.is_finite() {
                return None;
            }
            lon += (d * ds - b * dl) / det;
            lat += (a * dl - c * ds) / det;
        }
        let (s0, l0) = self.project(lon, lat, h);
        ((samp - s0).abs() < 1e-3 && (line - l0).abs() < 1e-3).then_some((lon, lat))
    }

    /// A least-squares affine fit (stored pixel to lon/lat) over a grid of the image at the
    /// model's mean height, and the largest residual in pixels.
    pub fn fitted(&self, cols: u32, rows: u32) -> Option<(Affine2, f64)> {
        let n = 8;
        let mut pts = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                let (x, y) = (
                    f64::from(cols) * f64::from(i) / f64::from(n),
                    f64::from(rows) * f64::from(j) / f64::from(n),
                );
                // Pixel centres are at +0.5 in stored coordinates.
                let (lon, lat) = self.locate(x - 0.5, y - 0.5, self.h_off)?;
                pts.push((x, y, lon, lat));
            }
        }
        let t = Affine2::fit(
            &pts.iter()
                .map(|p| ([p.0, p.1], [p.2, p.3]))
                .collect::<Vec<_>>(),
        )?;
        let inv = t.inverse()?;
        let worst = pts
            .iter()
            .map(|p| {
                let (px, py) = inv.apply(p.2, p.3);
                (px - p.0).hypot(py - p.1)
            })
            .fold(0.0, f64::max);
        Some((t, worst))
    }
}

impl Image {
    /// The RPC model, from RPC00B or RPC00A.
    pub fn rpc(&self) -> Option<Rpc> {
        self.tre("RPC00B")
            .and_then(|t| Rpc::parse(t, false))
            .or_else(|| self.tre("RPC00A").and_then(|t| Rpc::parse(t, true)))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A NITF 2.1 file with one uncompressed image: `bands` bands of `nbpp`-bit samples
    /// (big-endian), interleave `imode`, blocks of `bw`×`bh`, ICORDS/IGEOLO as given.
    #[allow(clippy::too_many_arguments)] // a test fixture's knobs
    pub fn nitf(
        w: u32,
        h: u32,
        bands: &[&str],
        nbpp: u32,
        imode: char,
        (bw, bh): (u32, u32),
        icords: char,
        igeolo: &str,
        pixels: &[u8],
        tre: Option<(&str, &[u8])>,
    ) -> Vec<u8> {
        let pad = |s: &str, n: usize| format!("{s:<n$}");
        let num = |v: usize, n: usize| format!("{v:0n$}");
        let mut im = String::new();
        im += "IM";
        im += &pad("TESTIMG", 10);
        im += "20240501120000";
        im += &pad("", 17);
        im += &pad("Test image", 80);
        im += "U";
        im += &pad(
            "",
            2 + 11 + 2 + 20 + 2 + 8 + 4 + 1 + 8 + 43 + 1 + 40 + 1 + 8 + 15,
        );
        im += "0";
        im += &pad("Specular test", 42);
        im += &num(h as usize, 8);
        im += &num(w as usize, 8);
        im += "INT";
        im += &pad(if bands.len() == 3 { "RGB" } else { "MONO" }, 8);
        im += &pad("VIS", 8);
        im += &num(nbpp as usize, 2);
        im += "R";
        im.push(icords);
        if icords != ' ' {
            im += &pad(igeolo, 60);
        }
        im += "0"; // NICOM
        im += "NC";
        im += &num(bands.len(), 1);
        for b in bands {
            im += &pad(b, 2);
            im += &pad("", 6);
            im += "N";
            im += "   ";
            im += "0";
        }
        im += "0";
        im.push(imode);
        im += &num(w.div_ceil(bw) as usize, 4);
        im += &num(h.div_ceil(bh) as usize, 4);
        im += &num(bw as usize, 4);
        im += &num(bh as usize, 4);
        im += &num(nbpp as usize, 2);
        im += "001000";
        im += "0000000000";
        im += "1.0 ";
        im += "00000";
        let mut sub = im.into_bytes();
        match tre {
            Some((tag, data)) => {
                let t = format!("{tag:<6}{:05}", data.len());
                sub.extend_from_slice(format!("{:05}000", t.len() + data.len() + 3).as_bytes());
                sub.extend_from_slice(t.as_bytes());
                sub.extend_from_slice(data);
            }
            None => sub.extend_from_slice(b"00000"),
        }
        let mut hdr = String::new();
        hdr += "NITF02.10";
        hdr += "03";
        hdr += "BF01";
        hdr += &pad("SPECULAR", 10);
        hdr += "20240501120000";
        hdr += &pad("Test file", 80);
        hdr += "U";
        hdr += &pad(
            "",
            2 + 11 + 2 + 20 + 2 + 8 + 4 + 1 + 8 + 43 + 1 + 40 + 1 + 8 + 15,
        );
        hdr += "00001";
        hdr += "00001";
        hdr += "0";
        hdr += "\0\0\0";
        hdr += &pad("Originator", 24);
        hdr += &pad("", 18);
        let fixed_len = hdr.len() + 12 + 6 + 3 + 16 + 3 * 5 + 5 + 5;
        let total = fixed_len + sub.len() + pixels.len();
        hdr += &num(total, 12);
        hdr += &num(fixed_len, 6);
        hdr += "001";
        hdr += &num(sub.len(), 6);
        hdr += &num(pixels.len(), 10);
        hdr += "000000000000000"; // NUMS, NUMX, NUMT, NUMDES, NUMRES
        hdr += "00000"; // UDHDL
        hdr += "00000"; // XHDL
        let mut out = hdr.into_bytes();
        assert_eq!(out.len(), fixed_len);
        out.extend_from_slice(&sub);
        out.extend_from_slice(pixels);
        out
    }

    #[test]
    fn block_table_bomb() {
        // 9999 × 9999 one-pixel blocks: a 10^8-entry block table from a few hundred bytes.
        let f = nitf(9999, 9999, &["M"], 8, 'B', (1, 1), ' ', "", &[0; 16], None);
        let e = parse(&f).map(|_| ()).unwrap_err();
        assert!(e.contains("too many blocks"), "{e}");
    }

    #[test]
    fn rgb_blocks_and_corners() {
        // 4×3 RGB, band-interleaved by block, one block; corners on 0.5-degree centres.
        let (w, h) = (4u32, 3u32);
        let mut px = Vec::new();
        for b in 0..3u8 {
            for i in 0..12u8 {
                px.push(b * 50 + i);
            }
        }
        // Pixel centres: (0,0) at 10E 50N, steps 1 degree east and south.
        let geolo = "500000N0100000E500000N0130000E480000N0130000E480000N0100000E";
        let f = nitf(
            w,
            h,
            &["R", "G", "B"],
            8,
            'B',
            (4, 3),
            'G',
            geolo,
            &px,
            None,
        );
        assert!(sniff(&f));
        let n = parse(&f).unwrap();
        assert_eq!(n.version, "NITF 2.1");
        let im = &n.images[0];
        assert_eq!((im.cols, im.rows, im.bands.len()), (4, 3, 3));
        let b = im.block(&f, 0, 0).unwrap().unwrap();
        assert_eq!(b[1][5], 55.0);
        assert_eq!(b[2][11], 111.0);
        let (t, crs) = transform(im).unwrap();
        assert_eq!(crs.epsg, Some(4326));
        let (x, y) = t.apply(0.5, 0.5);
        assert!(
            (x - 10.0).abs() < 1e-9 && (y - 50.0).abs() < 1e-9,
            "{x} {y}"
        );
        assert!((t.a - 1.0).abs() < 1e-12 && (t.e + 1.0).abs() < 1e-12);
    }

    #[test]
    fn pixel_interleaved_16_bit_utm() {
        let (w, h) = (2u32, 2u32);
        let mut px = Vec::new();
        for i in 0..4u16 {
            for b in 0..2u16 {
                px.extend_from_slice(&(1000 * b + i).to_be_bytes());
            }
        }
        let geolo = "325000005200000325000105200000325000105199990325000005199990";
        let f = nitf(w, h, &["M", "M"], 16, 'P', (2, 2), 'N', geolo, &px, None);
        let im = &parse(&f).unwrap().images[0];
        let b = im.block(&f, 0, 0).unwrap().unwrap();
        assert_eq!(b[0], vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(b[1], vec![1000.0, 1001.0, 1002.0, 1003.0]);
        let (t, crs) = transform(im).unwrap();
        assert_eq!(crs.epsg, Some(32632));
        assert_eq!((t.a, t.e), (10.0, -10.0));
    }

    #[test]
    fn rpc_inverse_round_trips() {
        let mut tre = String::from("1");
        tre += "0000.00";
        tre += "0000.00";
        let f = |v: f64, n: usize| format!("{v:>n$}");
        tre += &f(100.0, 6);
        tre += &f(150.0, 5);
        tre += &f(46.95, 8);
        tre += &f(7.45, 9);
        tre += &f(500.0, 5);
        tre += &f(100.0, 6);
        tre += &f(150.0, 5);
        tre += &f(0.05, 8);
        tre += &f(0.05, 9);
        tre += &f(500.0, 5);
        // line = -lat, samp = lon (+ a little cross term).
        let num = |c: f64| format!("{c:+.6E}");
        let twelve = |v: &[(usize, f64)]| {
            let mut s = String::new();
            for k in 0..20 {
                let c = v.iter().find(|(i, _)| *i == k).map_or(0.0, |(_, c)| *c);
                let t = num(c);
                s += &format!("{t:>12}");
            }
            s
        };
        tre += &twelve(&[(2, -1.0), (4, 0.01)]);
        tre += &twelve(&[(0, 1.0)]);
        tre += &twelve(&[(1, 1.0)]);
        tre += &twelve(&[(0, 1.0)]);
        let rpc = Rpc::parse(tre.as_bytes(), false).expect("parses");
        let (s, l) = rpc.project(7.47, 46.96, 500.0);
        let (lon, lat) = rpc.locate(s, l, 500.0).unwrap();
        assert!((lon - 7.47).abs() < 1e-9 && (lat - 46.96).abs() < 1e-9);
        let (t, worst) = rpc.fitted(300, 200).unwrap();
        // The cross term bends the grid by up to a pixel at the corners.
        assert!(worst < 1.5, "{worst}");
        let (x, _) = t.apply(150.5, 100.5);
        assert!((x - 7.45).abs() < 1e-3, "{x}");
    }

    #[test]
    fn damaged_headers_fail_cleanly() {
        let f = nitf(2, 2, &["M"], 8, 'B', (2, 2), ' ', "", &[1, 2, 3, 4], None);
        for cut in [5, 100, 300, f.len() - 2] {
            if let Ok(n) = parse(&f[..cut]) {
                for im in &n.images {
                    let _ = im.block(&f[..cut], 0, 0);
                }
            }
        }
        let mut bad = f.clone();
        for b in bad.iter_mut().skip(300).step_by(11) {
            *b = b'9';
        }
        if let Ok(n) = parse(&bad) {
            for im in &n.images {
                let _ = im.block(&bad, 0, 0);
            }
        }
    }

    #[test]
    fn mgrs_to_utm() {
        // 32TLT 85000 01000 is near 46.95N 7.45E in zone 32 (Bern area).
        let (z, north, e, n) = mgrs("32TLT8500001000").unwrap();
        assert_eq!(z, 32);
        assert!(north);
        let utm = crs::from_epsg(32632).unwrap();
        let (lon, lat) = utm.to_lonlat(e, n).unwrap();
        assert!(
            (lat - 46.9).abs() < 0.5 && (lon - 7.0).abs() < 1.5,
            "{lon} {lat}"
        );
    }
}
