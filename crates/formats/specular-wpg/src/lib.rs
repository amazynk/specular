//! WordPerfect Graphics (`.wpg`), versions 1 and 2, as one page each.
//!
//! [`open`] parses the record stream once into a [`specular_drawpage::Page`]: geometry in page
//! points (WPG is y-up; the page is y-down), colours resolved through the file's palette,
//! bitmaps decoded to RGBA. The page is the picture's own size from its Start WPG record.
//!
//! **Version 1** (WordPerfect 5): lines, polylines, rectangles, polygons, ellipses and
//! elliptical arcs, curved polylines (Béziers), fill and line attributes (hollow or solid
//! fill — hatch patterns are filled solid in their colour; line styles as dashes; widths),
//! the colour map over the default 256-colour palette, bitmaps type 1 and 2 (run-length
//! coded, 1/2/4/8 bits per pixel), graphics text (type 1, with its attributes).
//!
//! **Version 2** (WordPerfect 6 and later, Presentations): polylines, polysplines (as
//! polylines), polycurves, rectangles (rounded), arcs and ellipses, compound polygons, groups
//! and object capsules, with every object's transform (scale, rotation, skew, translation);
//! pen colour, style (dash definitions), size, caps and joins; brush colour (a gradient as its
//! first and last colours along its angle), colour palettes, bitmaps (raw and run-length coded,
//! 1/2/4/8/24 bits per pixel) in 16- and 32-bit precision files.
//!
//! Not drawn, and counted in [`Facts::skipped`]: PostScript and EPS data (no PostScript
//! interpreter), version 2 text, charts, markers, text types 2 and 3, textures and envelopes.
//!
//! Bounds: [`MAX_RECORDS`] records, [`MAX_POINTS`] points, and
//! [`specular_drawpage::MAX_IMAGE_PIXELS`] bitmap pixels per file. Every count and length is
//! checked against the record holding it; hostile files draw less or fail, never panic or loop.

mod bitmap;
mod palette;
mod reader;
mod v1;
mod v2;

use specular_drawpage::Page;

/// Most records read per file.
pub const MAX_RECORDS: usize = 4_000_000;
/// Most points per file (all primitives together).
pub const MAX_POINTS: usize = 8_000_000;
/// Largest page side, points (200 inches); larger pictures are scaled down to it.
pub const MAX_PAGE_SIDE: f64 = 200.0 * 72.0;

/// Why a file cannot be opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a WordPerfect Graphics file")]
    NotWpg,
    #[error("password-protected WordPerfect graphic: not supported")]
    Encrypted,
    #[error("{0}")]
    Unsupported(String),
    #[error("damaged WordPerfect graphic: {0}")]
    Malformed(String),
}

/// What a file says about itself, for the info panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    /// Major version: 1 or 2.
    pub version: u8,
    pub minor: u8,
    /// Page size, points.
    pub size: (f64, f64),
    /// Records read.
    pub records: u64,
    /// Embedded bitmaps drawn.
    pub bitmaps: u32,
    /// Records not drawn, by name, with counts.
    pub skipped: Vec<(String, u64)>,
    /// PostScript or EPS data is present (not drawn).
    pub postscript: bool,
    /// The file ends inside a record: what came before is drawn.
    pub truncated: bool,
    /// A limit was hit (records, points, bitmap pixels): some content is left out.
    pub limited: bool,
    /// 32-bit precision coordinates (version 2).
    pub double_precision: bool,
    /// Bitmaps the memory budget could not hold (not drawn), and the largest refusal.
    pub refused_bitmaps: u32,
    pub needs_memory: Option<specular_core::NeedsMemory>,
}

impl Facts {
    /// A bitmap left out because the memory budget refused it.
    pub(crate) fn refused_memory(&mut self, n: specular_core::NeedsMemory) {
        self.refused_bitmaps += 1;
        if self.needs_memory.is_none_or(|m| n.needed > m.needed) {
            self.needs_memory = Some(n);
        }
    }

    pub(crate) fn skip(&mut self, name: &str) {
        if let Some(e) = self.skipped.iter_mut().find(|e| e.0 == name) {
            e.1 += 1;
        } else {
            self.skipped.push((name.to_string(), 1));
        }
    }
}

/// A parsed WordPerfect graphic: one page.
#[derive(Debug, Clone)]
pub struct Document {
    pub pages: Vec<Page>,
    pub facts: Facts,
}

impl Document {
    /// A short English description: "WordPerfect Graphics 2, 3 bitmaps, PostScript not drawn".
    pub fn describe(&self, _page: usize) -> String {
        let f = &self.facts;
        let mut s = format!("WordPerfect Graphics {}", f.version);
        if f.double_precision {
            s.push_str(" (32-bit precision)");
        }
        match f.bitmaps {
            0 => {}
            1 => s.push_str(", 1 bitmap"),
            n => s.push_str(&format!(", {n} bitmaps")),
        }
        if f.postscript {
            s.push_str(", PostScript data not drawn");
        }
        if let Some(n) = f.needs_memory {
            let noun = if f.refused_bitmaps == 1 {
                "bitmap"
            } else {
                "bitmaps"
            };
            s.push_str(&format!(", {} {noun} not drawn: {n}", f.refused_bitmaps));
        }
        let other: u64 = f
            .skipped
            .iter()
            .filter(|(n, _)| !n.starts_with("PostScript"))
            .map(|(_, n)| n)
            .sum();
        if other > 0 {
            let noun = if other == 1 { "record" } else { "records" };
            s.push_str(&format!(", {other} {noun} not drawn"));
        }
        if f.limited {
            s.push_str(", limits reached");
        }
        if f.truncated {
            s.push_str(", file ends early");
        }
        s
    }
}

/// The 16-byte WordPerfect prefix: FF "WPC", data offset, product 1, file type 0x16 (WPG),
/// major version 1 or 2.
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 16
        && head.starts_with(b"\xffWPC")
        && head[8] == 1
        && head[9] == 0x16
        && matches!(head[10], 1 | 2)
}

/// Parses a WPG file.
pub fn open(data: &[u8]) -> Result<Document, Error> {
    if data.len() < 16 || !data.starts_with(b"\xffWPC") {
        return Err(Error::NotWpg);
    }
    if data[9] != 0x16 {
        return Err(Error::Unsupported(
            "a WordPerfect file that is not a graphic (documents are not shown)".into(),
        ));
    }
    let major = data[10];
    if u16::from_le_bytes([data[12], data[13]]) != 0 {
        return Err(Error::Encrypted);
    }
    let start = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    if start < 16 || start > data.len() {
        return Err(Error::Malformed(
            "the record data starts outside the file".into(),
        ));
    }
    let mut facts = Facts {
        version: major,
        minor: data[11],
        ..Facts::default()
    };
    let page = match major {
        1 => v1::parse(&data[start..], &mut facts)?,
        2 => v2::parse(&data[start..], &mut facts)?,
        v => {
            return Err(Error::Unsupported(format!(
                "WordPerfect Graphics version {v}: only versions 1 and 2 are read"
            )));
        }
    };
    facts.size = page.size;
    Ok(Document {
        pages: vec![page],
        facts,
    })
}

/// Fits a page of `w`×`h` points within [`MAX_PAGE_SIDE`]: the factor to scale by.
pub(crate) fn page_fit(w: f64, h: f64) -> f64 {
    let m = w.max(h);
    if m > MAX_PAGE_SIDE {
        MAX_PAGE_SIDE / m
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests;
