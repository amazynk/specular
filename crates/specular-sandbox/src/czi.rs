//! The facts of a Zeiss CZI document a host sends back for
//! [`crate::protocol::Request::CziInfo`], and the checks on the planes it sends.
//!
//! The host's libCZI bridge (`specular-decode-host/czi/specular_czi.cpp`) writes the facts;
//! [`CziInfo::decode`] reads them in the viewer, checking every count, rectangle and string,
//! since the bytes come from a process that parsed a hostile file.
//!
//! ```text
//! magic "SPCZII01", then records: u8 tag · u32 length · body[length], ending with tag 0.
//!  1 HEADER   u32 major · u32 minor · u32 sub-blocks · i32 min M · i32 max M
//!  2 DIM      u8 letter · i32 start · i32 size
//!  3 BOX      i32 x y w h (all layers) · i32 x y w h (layer 0)
//!  4 SCENE    i32 index · i32 x y w h (all) · i32 x y w h (layer 0)
//!  5 PYRAMID  i32 scene (-1: none) · u32 n · n × (u8 minification · u8 layer · u32 count)
//!  6 CHANNEL  i32 index · u8 pixel type (255 unknown) · u8 flags (1 display settings,
//!             2 enabled, 4 tinted) · u8 r g b · f32 black · f32 white · u8 gradation
//!             · f32 gamma · i32 bits (-1) · str name · str id
//!  7 SCALING  f64 x y z (metres per pixel; NaN when unknown)
//!  8 ATTACH   i32 index · str content type · str name
//!  9 CODING   i32 raw compression · u32 sub-blocks
//! 10 PIXEL    u8 pixel type · u32 sub-blocks
//! 11 DOC      str name · str title · str description · str created · str user · str comment
//! 12 XML      str metadata XML
//! ```
//!
//! Little endian; strings are a `u32` length and UTF-8. Unknown tags are skipped.

use crate::protocol::{CziPlane, CziPlaneKind, czi_pixel_bytes};

/// First bytes of a document's facts.
pub const MAGIC: &[u8; 8] = b"SPCZII01";

const MAX_SCENES: usize = 1 << 16;
const MAX_CHANNELS: usize = 4096;
const MAX_ATTACHMENTS: usize = 1 << 16;
const MAX_LAYERS: usize = 256;
const MAX_TEXT: usize = 64 << 10;

/// A rectangle in the document's raw sub-block coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CziRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl CziRect {
    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }
}

/// A dimension of the document: its letter (`Z`, `C`, `T`, `S`, `M`...), first index and
/// count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CziDim {
    pub letter: u8,
    pub start: i32,
    pub size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CziScene {
    pub index: i32,
    pub bbox: CziRect,
    pub bbox_layer0: CziRect,
}

/// One pyramid layer of a scene: minification factor between layers (0 for layer 0), layer
/// number, sub-blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CziLayer {
    pub minification: u8,
    pub layer: u8,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CziPyramid {
    pub scene: Option<i32>,
    pub layers: Vec<CziLayer>,
}

/// A channel's display settings (ZEN's look).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CziChannelDisplay {
    pub enabled: bool,
    pub tint: Option<[u8; 3]>,
    /// Black and white point, fractions of the sample range (finite).
    pub black: f32,
    pub white: f32,
    /// libCZI's gradation curve mode (0 linear, 1 gamma, 2 spline).
    pub gradation: u8,
    pub gamma: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CziChannel {
    pub index: i32,
    /// libCZI's pixel type of the channel's sub-blocks.
    pub pixel: Option<u8>,
    pub display: Option<CziChannelDisplay>,
    /// Significant bits per sample, when the metadata says.
    pub bits: Option<u32>,
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CziAttachment {
    pub index: u32,
    pub content_type: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CziDocument {
    pub name: String,
    pub title: String,
    pub description: String,
    pub created: String,
    pub user: String,
    pub comment: String,
}

/// What libCZI says about a document.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CziInfo {
    /// File format version (major, minor).
    pub version: (u32, u32),
    pub subblocks: u32,
    /// The mosaic tile index range, when the sub-blocks have one.
    pub m_range: Option<(i32, i32)>,
    pub dims: Vec<CziDim>,
    pub bbox: CziRect,
    pub bbox_layer0: CziRect,
    pub scenes: Vec<CziScene>,
    pub pyramids: Vec<CziPyramid>,
    pub channels: Vec<CziChannel>,
    /// Metres per pixel in x, y and z.
    pub scaling: [Option<f64>; 3],
    pub attachments: Vec<CziAttachment>,
    /// (raw compression mode, sub-blocks).
    pub codings: Vec<(i32, u32)>,
    /// (pixel type, sub-blocks).
    pub pixels: Vec<(u8, u32)>,
    pub document: Option<CziDocument>,
    pub xml: Option<String>,
}

struct Rd<'a>(&'a [u8]);

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.0.len() < n {
            return Err("CZI facts truncated".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn f64(&mut self) -> Result<f64, String> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(f64::from_le_bytes(a))
    }
    fn str(&mut self, max: usize) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > max {
            return Err("CZI text too long".into());
        }
        let b = self.take(n)?;
        Ok(String::from_utf8_lossy(b).into_owned())
    }
    fn rect(&mut self) -> Result<CziRect, String> {
        let (x, y, w, h) = (self.i32()?, self.i32()?, self.i32()?, self.i32()?);
        // libCZI marks an invalid rectangle with negative sizes: an empty one.
        let (w, h) = (u32::try_from(w).unwrap_or(0), u32::try_from(h).unwrap_or(0));
        if i64::from(x) + i64::from(w) > i64::from(i32::MAX)
            || i64::from(y) + i64::from(h) > i64::from(i32::MAX)
        {
            return Err("CZI rectangle outside the coordinate range".into());
        }
        Ok(CziRect { x, y, w, h })
    }
}

impl CziInfo {
    /// Reads a document's facts, refusing metadata XML longer than `max_xml` bytes.
    pub fn decode(b: &[u8], max_xml: usize) -> Result<CziInfo, String> {
        let mut r = Rd(b);
        if r.take(8)? != MAGIC {
            return Err("not CZI facts".into());
        }
        let mut info = CziInfo::default();
        let mut saw_header = false;
        loop {
            let tag = r.u8()?;
            if tag == 0 {
                break;
            }
            let len = r.u32()? as usize;
            let mut body = Rd(r.take(len)?);
            match tag {
                1 => {
                    info.version = (body.u32()?, body.u32()?);
                    info.subblocks = body.u32()?;
                    let (lo, hi) = (body.i32()?, body.i32()?);
                    info.m_range = (lo <= hi).then_some((lo, hi));
                    saw_header = true;
                }
                2 => {
                    let letter = body.u8()?;
                    let start = body.i32()?;
                    let size = u32::try_from(body.i32()?)
                        .map_err(|_| "CZI dimension with a negative size")?;
                    if !b"ZCTRSIHVBM".contains(&letter) {
                        return Err(format!("unknown CZI dimension {letter}"));
                    }
                    if info.dims.iter().any(|d| d.letter == letter) {
                        return Err("CZI dimension given twice".into());
                    }
                    if i64::from(start) + i64::from(size) > i64::from(i32::MAX) {
                        return Err("CZI dimension outside the coordinate range".into());
                    }
                    info.dims.push(CziDim {
                        letter,
                        start,
                        size,
                    });
                }
                3 => {
                    info.bbox = body.rect()?;
                    info.bbox_layer0 = body.rect()?;
                }
                4 => {
                    if info.scenes.len() >= MAX_SCENES {
                        return Err("too many CZI scenes".into());
                    }
                    let s = CziScene {
                        index: body.i32()?,
                        bbox: body.rect()?,
                        bbox_layer0: body.rect()?,
                    };
                    if info.scenes.iter().any(|o| o.index == s.index) {
                        return Err("CZI scene given twice".into());
                    }
                    info.scenes.push(s);
                }
                5 => {
                    if info.pyramids.len() >= MAX_SCENES {
                        return Err("too many CZI pyramids".into());
                    }
                    let scene = body.i32()?;
                    let n = body.u32()? as usize;
                    if n > MAX_LAYERS || n.saturating_mul(6) > body.0.len() {
                        return Err("bad CZI pyramid".into());
                    }
                    let mut layers = Vec::with_capacity(n);
                    for _ in 0..n {
                        layers.push(CziLayer {
                            minification: body.u8()?,
                            layer: body.u8()?,
                            count: body.u32()?,
                        });
                    }
                    info.pyramids.push(CziPyramid {
                        scene: (scene >= 0).then_some(scene),
                        layers,
                    });
                }
                6 => {
                    if info.channels.len() >= MAX_CHANNELS {
                        return Err("too many CZI channels".into());
                    }
                    let index = body.i32()?;
                    let pixel = body.u8()?;
                    let flags = body.u8()?;
                    let rgb = [body.u8()?, body.u8()?, body.u8()?];
                    let (black, white) = (body.f32()?, body.f32()?);
                    let gradation = body.u8()?;
                    let gamma = body.f32()?;
                    let bits = body.i32()?;
                    let name = body.str(MAX_TEXT)?;
                    let id = body.str(MAX_TEXT)?;
                    let display = (flags & 1 != 0 && black.is_finite() && white.is_finite())
                        .then_some(CziChannelDisplay {
                            enabled: flags & 2 != 0,
                            tint: (flags & 4 != 0).then_some(rgb),
                            black,
                            white,
                            gradation,
                            gamma: Some(gamma).filter(|g| g.is_finite() && *g > 0.0),
                        });
                    info.channels.push(CziChannel {
                        index,
                        pixel: czi_pixel_bytes(pixel).map(|_| pixel),
                        display,
                        bits: u32::try_from(bits).ok().filter(|b| (1..=64).contains(b)),
                        name,
                        id,
                    });
                }
                7 => {
                    let ok = |v: f64| (v.is_finite() && v > 0.0 && v < 1.0).then_some(v);
                    info.scaling = [ok(body.f64()?), ok(body.f64()?), ok(body.f64()?)];
                }
                8 => {
                    if info.attachments.len() >= MAX_ATTACHMENTS {
                        return Err("too many CZI attachments".into());
                    }
                    let index = u32::try_from(body.i32()?).map_err(|_| "bad CZI attachment")?;
                    info.attachments.push(CziAttachment {
                        index,
                        content_type: body.str(8)?,
                        name: body.str(MAX_TEXT)?,
                    });
                }
                9 => {
                    if info.codings.len() < 256 {
                        info.codings.push((body.i32()?, body.u32()?));
                    }
                }
                10 => {
                    if info.pixels.len() < 256 {
                        info.pixels.push((body.u8()?, body.u32()?));
                    }
                }
                11 => {
                    info.document = Some(CziDocument {
                        name: body.str(MAX_TEXT)?,
                        title: body.str(MAX_TEXT)?,
                        description: body.str(MAX_TEXT)?,
                        created: body.str(MAX_TEXT)?,
                        user: body.str(MAX_TEXT)?,
                        comment: body.str(MAX_TEXT)?,
                    });
                }
                12 => info.xml = Some(body.str(max_xml)?),
                // A newer bridge's records.
                _ => {}
            }
        }
        if !saw_header {
            return Err("CZI facts without a header".into());
        }
        if !r.0.is_empty() {
            return Err("bytes after the CZI facts".into());
        }
        Ok(info)
    }

    /// The dimension `letter`, when the document has it.
    pub fn dim(&self, letter: u8) -> Option<CziDim> {
        self.dims.iter().copied().find(|d| d.letter == letter)
    }

    /// The pyramid of `scene` (or of the document without scenes).
    pub fn pyramid(&self, scene: Option<i32>) -> Option<&CziPyramid> {
        self.pyramids
            .iter()
            .find(|p| p.scene == scene)
            .or_else(|| self.pyramids.iter().find(|p| p.scene.is_none()))
    }
}

/// Checks a plane the host returned for a region of `want` (width, height) pixels of type
/// `pixel`, and returns its byte length: libCZI's size for a zoomed region may differ from the
/// asked one by a pixel or two, never more.
pub fn check_plane(
    p: &CziPlane,
    pixel: u8,
    want: (u32, u32),
    max_bytes: u64,
) -> Result<u64, String> {
    if p.kind != CziPlaneKind::Pixels || p.pixel != pixel {
        return Err("libCZI answered with another kind of plane".into());
    }
    let bpp = czi_pixel_bytes(pixel).ok_or("unknown pixel type")? as u64;
    let near = |got: u32, want: u32| got.abs_diff(want) <= 2 + want / 512;
    if p.width == 0 || p.height == 0 || !near(p.width, want.0) || !near(p.height, want.1) {
        return Err(format!(
            "libCZI returned {}×{} pixels for {}×{}",
            p.width, p.height, want.0, want.1
        ));
    }
    let len = u64::from(p.width) * u64::from(p.height) * bpp;
    if len > max_bytes {
        return Err("libCZI returned more pixels than asked for".into());
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Payload;

    fn rec(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
        out.push(tag);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
    }

    fn i32s(v: &[i32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn str_(s: &str) -> Vec<u8> {
        let mut b = (s.len() as u32).to_le_bytes().to_vec();
        b.extend_from_slice(s.as_bytes());
        b
    }

    fn sample() -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        rec(&mut b, 1, &i32s(&[1, 0, 150, 0, 26]));
        rec(&mut b, 2, &[b"C".as_slice(), &i32s(&[0, 2])].concat());
        rec(&mut b, 2, &[b"S".as_slice(), &i32s(&[0, 2])].concat());
        rec(&mut b, 3, &i32s(&[-10, 5, 100, 50, -10, 5, 99, 49]));
        rec(&mut b, 4, &i32s(&[1, 40, 5, 50, 50, 40, 5, 49, 49]));
        let mut p = i32s(&[1, 2]);
        p.extend_from_slice(&[0, 0, 4, 0, 0, 0, 2, 1, 1, 0, 0, 0]);
        rec(&mut b, 5, &p);
        let mut c = i32s(&[0]);
        c.extend_from_slice(&[1, 7, 0, 255, 91]);
        c.extend_from_slice(&0.0f32.to_le_bytes());
        c.extend_from_slice(&0.25f32.to_le_bytes());
        c.push(0);
        c.extend_from_slice(&f32::NAN.to_le_bytes());
        c.extend_from_slice(&12i32.to_le_bytes());
        c.extend_from_slice(&str_("DAPI"));
        c.extend_from_slice(&str_("Channel:0"));
        rec(&mut b, 6, &c);
        let mut s = Vec::new();
        for v in [3.4e-7f64, 3.4e-7, f64::NAN] {
            s.extend_from_slice(&v.to_le_bytes());
        }
        rec(&mut b, 7, &s);
        rec(
            &mut b,
            8,
            &[i32s(&[2]), str_("CZI"), str_("Label")].concat(),
        );
        rec(&mut b, 9, &i32s(&[4, 150]));
        rec(&mut b, 42, b"future record");
        rec(&mut b, 12, &str_("<ImageDocument/>"));
        b.push(0);
        b
    }

    #[test]
    fn reads_facts() {
        let i = CziInfo::decode(&sample(), 1 << 20).unwrap();
        assert_eq!(i.subblocks, 150);
        assert_eq!(i.m_range, Some((0, 26)));
        assert_eq!(i.dim(b'S').unwrap().size, 2);
        assert_eq!(
            i.bbox,
            CziRect {
                x: -10,
                y: 5,
                w: 100,
                h: 50
            }
        );
        assert_eq!(i.scenes[0].index, 1);
        assert_eq!(i.pyramid(Some(1)).unwrap().layers.len(), 2);
        let c = &i.channels[0];
        assert_eq!(c.pixel, Some(1));
        assert_eq!(c.bits, Some(12));
        assert_eq!(c.name, "DAPI");
        let d = c.display.unwrap();
        assert!(d.enabled);
        assert_eq!(d.tint, Some([0, 255, 91]));
        assert_eq!(d.gamma, None);
        assert_eq!(i.scaling[2], None);
        assert_eq!(i.attachments[0].name, "Label");
        assert_eq!(i.xml.as_deref(), Some("<ImageDocument/>"));
        // XML past the limit is refused.
        assert!(CziInfo::decode(&sample(), 4).is_err());
    }

    #[test]
    fn hostile_facts_never_panic() {
        let good = sample();
        for n in 0..good.len() {
            assert!(CziInfo::decode(&good[..n], 1 << 20).is_err(), "prefix {n}");
        }
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for i in 0..good.len() {
            for v in [0u8, 1, 0x7F, 0x80, 0xFF] {
                let mut m = good.clone();
                m[i] = v;
                let _ = CziInfo::decode(&m, 1 << 20);
            }
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let mut m = good.clone();
            m[i] ^= s as u8;
            let _ = CziInfo::decode(&m, 1 << 20);
        }
        // A huge layer count is refused without allocating.
        let mut b = MAGIC.to_vec();
        rec(&mut b, 1, &i32s(&[1, 0, 1, 0, 0]));
        rec(&mut b, 5, &i32s(&[0, i32::MAX]));
        b.push(0);
        assert!(CziInfo::decode(&b, 0).is_err());
        // A rectangle reaching past the coordinate range.
        let mut b = MAGIC.to_vec();
        rec(&mut b, 1, &i32s(&[1, 0, 1, 0, 0]));
        rec(&mut b, 3, &i32s(&[i32::MAX, 0, 10, 10, 0, 0, 1, 1]));
        b.push(0);
        assert!(CziInfo::decode(&b, 0).is_err());
    }

    #[test]
    fn planes_are_checked() {
        let p = |w, h, pixel| CziPlane {
            kind: CziPlaneKind::Pixels,
            width: w,
            height: h,
            pixel,
            content_type: String::new(),
            display: None,
            payload: Payload::Inline(Vec::new()),
        };
        assert_eq!(
            check_plane(&p(100, 50, 1), 1, (100, 50), 1 << 20),
            Ok(10_000)
        );
        assert_eq!(check_plane(&p(101, 49, 1), 1, (100, 50), 1 << 20), Ok(9898));
        assert!(check_plane(&p(200, 50, 1), 1, (100, 50), 1 << 20).is_err());
        assert!(check_plane(&p(100, 50, 3), 1, (100, 50), 1 << 20).is_err());
        assert!(check_plane(&p(100, 50, 1), 1, (100, 50), 100).is_err());
        assert!(check_plane(&p(0, 50, 1), 1, (0, 50), 1 << 20).is_err());
    }
}
