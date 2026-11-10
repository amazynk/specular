//! Kodak Photo CD image packs (`.pcd`): the `PCD_IPI` header in the second 2 KiB sector,
//! then the image at several resolutions. Base/16 (192×128), Base/4 (384×256) and Base
//! (768×512) are stored whole as PhotoYCC, two luma lines followed by one line of each
//! chroma at half resolution; they are read here. Base×4 and Base×16 add Huffman-coded
//! residuals on top of Base: not read (the file is shown at Base).
//!
//! PhotoYCC is converted to sRGB with Kodak's published scale factors; values beyond
//! display white are clipped. The scan's orientation byte becomes an EXIF orientation.

use crate::{Error, Image, Info, Result, field};

const SECTOR: usize = 2048;
const IPI_AT: usize = SECTOR;
/// Where Base ends: Base×4 data follows it.
const BASE_END: usize = 96 * SECTOR + 768 * 512 * 3 / 2;

/// The resolutions stored whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Base16,
    Base4,
    Base,
}

impl Resolution {
    /// (width, height, byte offset).
    pub fn layout(self) -> (u32, u32, usize) {
        match self {
            Resolution::Base16 => (192, 128, 4 * SECTOR),
            Resolution::Base4 => (384, 256, 23 * SECTOR),
            Resolution::Base => (768, 512, 96 * SECTOR),
        }
    }
}

pub fn probe(data: &[u8]) -> bool {
    data.get(IPI_AT..IPI_AT + 7) == Some(b"PCD_IPI")
}

fn check(data: &[u8]) -> Result<()> {
    if data.starts_with(b"PCD_OPA") {
        return Err(Error::Unsupported(
            "this is a Photo CD overview pack (the disc's index of thumbnails), not an image"
                .into(),
        ));
    }
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    Ok(())
}

/// EXIF orientation from the scan's orientation (byte &E02, low 2 bits: quarter turns).
fn orientation(data: &[u8]) -> u8 {
    match data.get(0xE02).map(|b| b & 3) {
        Some(1) => 8,
        Some(2) => 3,
        Some(3) => 6,
        _ => 1,
    }
}

pub fn info(data: &[u8]) -> Result<Info> {
    check(data)?;
    let (w, h, _) = Resolution::Base.layout();
    let mut stored = vec!["Base/16 (192×128)", "Base/4 (384×256)", "Base (768×512)"];
    let extra = data.len() > BASE_END + 4 * SECTOR;
    if extra {
        stored.push("Base×4 and up (not read)");
    }
    let mut fields = vec![
        field("Resolutions", stored.join(", ")),
        field("Colour coding", "PhotoYCC 4:2:0"),
    ];
    if data.len() < BASE_END {
        fields.push(field(
            "Note",
            "the file ends before the Base image is complete",
        ));
    }
    Ok(Info {
        width: w,
        height: h,
        pages: 1,
        bit_depth: 8,
        palette: false,
        orientation: orientation(data),
        coding: if extra {
            "PhotoYCC 4:2:0, Base (higher resolutions not read)".into()
        } else {
            "PhotoYCC 4:2:0, Base".into()
        },
        fields,
    })
}

/// sRGB of a PhotoYCC sample.
pub fn ycc_to_rgb(y: u8, c1: u8, c2: u8) -> [u8; 3] {
    let l = 1.3584 * y as f32;
    let c1 = 2.2179 * (c1 as f32 - 156.0);
    let c2 = 1.8215 * (c2 as f32 - 137.0);
    let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [c(l + c2), c(l - 0.194 * c1 - 0.509 * c2), c(l + c1)]
}

pub fn decode(data: &[u8], res: Resolution) -> Result<Image> {
    check(data)?;
    let (w, h, off) = res.layout();
    let (wu, hu) = (w as usize, h as usize);
    let mut rgba = vec![255u8; wu * hu * 4];
    let pair = wu * 3; // two luma lines, then both chroma lines
    let mut truncated = false;
    for py in 0..hu / 2 {
        let start = off + py * pair;
        let Some(block) = data.get(start..start + pair) else {
            truncated = true;
            // Missing lines stay black.
            for p in rgba[py * 2 * wu * 4..].chunks_exact_mut(4) {
                p[..3].fill(0);
            }
            break;
        };
        let (luma, chroma) = block.split_at(wu * 2);
        let (c1, c2) = chroma.split_at(wu / 2);
        for dy in 0..2 {
            let row = py * 2 + dy;
            for x in 0..wu {
                let rgb = ycc_to_rgb(luma[dy * wu + x], c1[x / 2], c2[x / 2]);
                let i = (row * wu + x) * 4;
                rgba[i..i + 3].copy_from_slice(&rgb);
            }
        }
    }
    Ok(Image {
        width: w,
        height: h,
        rgba,
        truncated,
    })
}
