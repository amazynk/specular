//! Font files: TrueType and OpenType (`glyf`, `CFF `, `CFF2`), TrueType/OpenType
//! collections (TTC/OTC), WOFF 1.0 and 2.0, which unwrap to standalone SFNT fonts the
//! text stack can load (WOFF2's transformed `glyf`/`loca` and `hmtx` rebuilt), and
//! PostScript Type 1 fonts (`.pfb`, `.pfa`), rebuilt as OpenType CFF fonts, and bitmap fonts
//! (PCF, BDF, Windows FNT/FON, PSF) as glyph bitmaps ([`open_bitmap`], drawn by [`sheet`]).
//!
//! Each face's facts are read from its tables: names, metrics, weight and width, embedding
//! permissions (OS/2 `fsType`), variation axes and named instances (`fvar`), colour tables
//! (`COLR`/`CPAL`, `sbix`, `CBDT`/`CBLC`, `SVG `), the best Unicode character map, advances
//! and glyph names. Outlines come from `glyf` (composites included) or Type 2 charstrings
//! (CFF, CID-keyed CFF, and CFF2 at its default instance), as path commands in font units.
//! [`preview`] draws a thumbnail from them.
//!
//! The parsers are our own. Every count, size and nesting depth is bounded: hostile files
//! fail with a reason or show less, never panic or allocate without limit.

mod bitmap;
mod cff;
mod cmap;
mod colr;
mod face;
mod glyf;
mod names_std;
mod outline;
mod preview;
mod read;
mod sfnt;
mod type1;
mod woff;

use std::sync::Arc;

pub use bitmap::{
    BitmapFont, BitmapGlyph, BitmapKind, open_bitmap, sheet, sheet_size, sniff_bitmap,
};
pub use face::{
    Axis, Colour, Embedding, Face, Instance, Metrics, Names, OutlineKind, Outliner, Permission,
};
pub use outline::{Outline, PathEl};
pub use preview::preview;
pub use sfnt::TableRecord;
#[doc(hidden)]
pub use type1::fixture as type1_fixture;
pub use type1::{Type1Glyphs, glyphs_by_code as type1_glyphs_by_code};

/// What a WOFF file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    TrueType,
    Cff,
    Collection,
}

/// The kind of font file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    /// An SFNT with TrueType outlines (or none): `.ttf`.
    TrueType,
    /// An SFNT with PostScript outlines: `.otf`.
    OpenTypeCff,
    /// A TrueType/OpenType collection: `.ttc`, `.otc`.
    Collection,
    Woff(Flavor),
    Woff2(Flavor),
    /// A PostScript Type 1 font (`.pfb`, `.pfa`), rebuilt as an OpenType CFF font.
    Type1,
}

impl Container {
    /// "TrueType", "OpenType (CFF)", "Font collection", "WOFF", "WOFF2".
    pub fn name(self) -> &'static str {
        match self {
            Container::TrueType => "TrueType",
            Container::OpenTypeCff => "OpenType (CFF)",
            Container::Collection => "Font collection",
            Container::Woff(_) => "WOFF",
            Container::Woff2(_) => "WOFF2",
            Container::Type1 => "PostScript Type 1",
        }
    }
}

fn flavor(v: u32) -> Flavor {
    match v {
        sfnt::OTTO => Flavor::Cff,
        sfnt::TTCF => Flavor::Collection,
        _ => Flavor::TrueType,
    }
}

/// The kind of font `bytes` starts, by its signature.
pub fn sniff(bytes: &[u8]) -> Option<Container> {
    if type1::sniff(bytes) {
        return Some(Container::Type1);
    }
    let tag = read::u32_at(bytes, 0)?;
    Some(match tag {
        sfnt::OTTO => Container::OpenTypeCff,
        sfnt::TRUETYPE | sfnt::TRUE => {
            // Tables follow; a 0x00010000 start alone is too weak a signature.
            let n = read::u16_at(bytes, 4)?;
            if n == 0 || n > sfnt::MAX_TABLES {
                return None;
            }
            Container::TrueType
        }
        sfnt::TTCF => Container::Collection,
        t if t == u32::from_be_bytes(*b"wOFF") => Container::Woff(flavor(read::u32_at(bytes, 4)?)),
        t if t == u32::from_be_bytes(*b"wOF2") => Container::Woff2(flavor(read::u32_at(bytes, 4)?)),
        _ => return None,
    })
}

/// Size caps for unwrapping WOFF and WOFF2.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Most bytes of SFNT data one file may unwrap to.
    pub max_sfnt_bytes: usize,
    /// Most bytes of one table.
    pub max_table_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_sfnt_bytes: 512 << 20,
            max_table_bytes: 256 << 20,
        }
    }
}

/// Why a font file cannot be opened.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a font file")]
    NotAFont,
    #[error("the font file is cut short")]
    Truncated,
    #[error("the {0} data is damaged")]
    Damaged(&'static str),
    #[error("the font lists too many tables")]
    TooManyTables,
    #[error("the collection holds too many fonts")]
    TooManyFaces,
    #[error("the collection holds no fonts")]
    NoFaces,
    #[error("the font is too large to open")]
    TooLarge,
    #[error("the WOFF2 file uses a table transform this reader does not know")]
    UnknownTransform,
    #[error("a WOFF 1 file cannot hold a font collection")]
    WoffCollection,
    #[error("the font has no {0} table")]
    MissingTable(&'static str),
    #[error("the font has no glyphs")]
    NoGlyphs,
    #[error("Apple PostScript Type 1 fonts in an SFNT wrapper are not read")]
    Type1,
    /// A bitmap font that does not read: what is wrong.
    #[error("the bitmap font is damaged: {0}")]
    Malformed(String),
    /// A kind of bitmap font that is not drawn, in words.
    #[error("{0}")]
    Unsupported(String),
}

/// An opened font file: its faces (one, or one per font of a collection).
#[derive(Debug)]
pub struct FontFile {
    pub container: Container,
    pub faces: Vec<Arc<Face>>,
    /// A WOFF file's metadata XML, decompressed.
    pub woff_metadata: Option<String>,
    /// The size of a WOFF file's private data block (0 without one).
    pub woff_private_len: u32,
}

/// Most character mappings over all faces of a collection (8 bytes each).
const MAX_COLLECTION_CHARS: usize = 1 << 24;

/// Opens the font file `bytes`.
pub fn open(bytes: &[u8], limits: Limits) -> Result<FontFile, Error> {
    let container = sniff(bytes).ok_or(Error::NotAFont)?;
    match container {
        Container::Type1 => {
            let data = Arc::new(type1::to_sfnt(bytes)?);
            let face = Face::parse(data, 0, 0)?;
            Ok(FontFile {
                container,
                faces: vec![Arc::new(face)],
                woff_metadata: None,
                woff_private_len: 0,
            })
        }
        Container::TrueType | Container::OpenTypeCff => {
            let data = Arc::new(bytes.to_vec());
            let face = Face::parse(data, 0, 0)?;
            Ok(FontFile {
                container,
                faces: vec![Arc::new(face)],
                woff_metadata: None,
                woff_private_len: 0,
            })
        }
        Container::Collection => {
            let offsets = sfnt::collection_offsets(bytes)?;
            let data = Arc::new(bytes.to_vec());
            let mut faces = Vec::with_capacity(offsets.len());
            // Each face reads its own character map: 4096 faces sharing one large `cmap`
            // would hold 4096 copies of it.
            let mut chars = 0usize;
            for (i, o) in offsets.iter().enumerate() {
                let face = Face::parse(data.clone(), i as u32, *o as usize)?;
                chars += face.chars().len();
                if chars > MAX_COLLECTION_CHARS {
                    return Err(Error::TooLarge);
                }
                faces.push(Arc::new(face));
            }
            Ok(FontFile {
                container,
                faces,
                woff_metadata: None,
                woff_private_len: 0,
            })
        }
        Container::Woff(_) | Container::Woff2(_) => {
            let u = if matches!(container, Container::Woff(_)) {
                woff::woff1(bytes, &limits)?
            } else {
                woff::woff2(bytes, &limits)?
            };
            let mut faces = Vec::with_capacity(u.fonts.len());
            for f in u.fonts {
                faces.push(Arc::new(Face::parse(Arc::new(f), 0, 0)?));
            }
            Ok(FontFile {
                container,
                faces,
                woff_metadata: u.metadata,
                woff_private_len: u.private_len,
            })
        }
    }
}

#[cfg(test)]
mod collection_tests {
    use super::*;

    /// 4096 faces of a collection sharing one font whose `cmap` maps every code point: each
    /// face used to read its own 1.1 million mappings (36 GB in all).
    #[test]
    fn shared_cmap_is_bounded() {
        let mut head = vec![0u8; 54];
        head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        let mut maxp = 0x5000u32.to_be_bytes().to_vec();
        maxp.extend_from_slice(&2u16.to_be_bytes());
        let mut cmap = Vec::new();
        for x in [0u16, 1, 3, 10] {
            cmap.extend_from_slice(&x.to_be_bytes());
        }
        cmap.extend_from_slice(&12u32.to_be_bytes());
        for x in [13u32 << 16, 28, 0, 1, 0, 0x10_FFFF, 1] {
            cmap.extend_from_slice(&x.to_be_bytes());
        }
        let font = sfnt::write(
            0x0001_0000,
            vec![(*b"head", head), (*b"maxp", maxp), (*b"cmap", cmap)],
        );
        let one = open(&font, Limits::default()).unwrap();
        assert_eq!(one.faces[0].chars().len(), 0x11_0000);
        let n = 4096u32;
        let mut ttc = b"ttcf".to_vec();
        ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        ttc.extend_from_slice(&n.to_be_bytes());
        let at = 12 + 4 * n;
        for _ in 0..n {
            ttc.extend_from_slice(&at.to_be_bytes());
        }
        // Table offsets count from the start of the collection.
        let mut font = font;
        for i in 0..3 {
            let o = 12 + 16 * i + 8;
            let v = u32::from_be_bytes(font[o..o + 4].try_into().unwrap()) + at;
            font[o..o + 4].copy_from_slice(&v.to_be_bytes());
        }
        ttc.extend_from_slice(&font);
        let r = open(&ttc, Limits::default()).map(|f| f.faces.len());
        assert!(matches!(r, Err(Error::TooLarge)), "{r:?}");
    }
}
