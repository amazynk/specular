//! The metadata a recipe writes: everything (optionally without location or the people
//! shown), only the copyright, the copyright with the creator's contact details, or nothing.
//! The colour profile is not metadata here: it follows the recipe's colour choices.

use std::path::Path;

use super::{Keep, Metadata};
use crate::exif::{Entry, Exif, TAG_ORIENTATION};
use crate::xmp::{self, Filter};

/// Exif IFD0 tags of the copyright.
const COPYRIGHT_TAGS: &[u16] = &[0x8298];
/// … and of the creator (Artist, XPAuthor).
const CONTACT_TAGS: &[u16] = &[0x8298, 0x013B, 0x9C9D];

/// The Exif and XMP for an upright `w × h` output of a file with `sm` (at `path`, whose
/// XMP sidecar keywords are added when everything is kept).
pub fn for_output(
    sm: &crate::SourceMeta,
    path: Option<&Path>,
    w: u32,
    h: u32,
    m: &Metadata,
) -> (Option<Exif>, Option<Vec<u8>>) {
    if m.keep == Keep::None {
        return (None, None);
    }
    let keep_location = !m.strip_gps && m.keep == Keep::All;
    let filter = Filter {
        keep_location,
        keep_people: !m.strip_people,
        only: match m.keep {
            Keep::Copyright => Some(xmp::COPYRIGHT),
            Keep::CopyrightContact => Some(xmp::COPYRIGHT_CONTACT),
            _ => None,
        },
    };
    let exif = sm.exif.as_ref().and_then(|e| match m.keep {
        Keep::All => e.for_output(w, h, keep_location),
        Keep::Copyright => only(e, COPYRIGHT_TAGS),
        _ => only(e, CONTACT_TAGS),
    });
    let mut packet = sm.xmp.as_deref().and_then(|x| xmp::filter(x, &filter));
    if m.keep == Keep::All
        && let Some(p) = path
    {
        packet = xmp::with_sidecar_keywords(p, packet);
    }
    (exif, packet)
}

/// The star rating of a photo (0–5; rejected counts as 0): its XMP sidecar's, else its own
/// XMP's or Exif's.
pub fn rating_of(path: &Path, sm: &crate::SourceMeta) -> Option<u8> {
    let parse = |x: &[u8]| {
        specular_metadata::xmp::parse(x)
            .get("xmp:Rating")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .map(|r| r.clamp(0.0, 5.0).round() as u8)
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned());
    let dir = path.parent().unwrap_or(Path::new(""));
    let side = [name, stem]
        .into_iter()
        .flatten()
        .flat_map(|n| [format!("{n}.xmp"), format!("{n}.XMP")])
        .map(|n| dir.join(n))
        .find(|p| p.is_file() && p.as_path() != path)
        .and_then(|p| std::fs::read(p).ok());
    side.as_deref()
        .and_then(parse)
        .or_else(|| sm.xmp.as_deref().and_then(parse))
        .or_else(|| {
            let e = sm.exif.as_ref()?.ifd0.iter().find(|e| e.tag == 0x4746)?;
            let v = e.data.get(..2)?;
            Some(u16::from_le_bytes([v[0], v[1]]).min(5) as u8)
        })
}

/// IFD0 with only `tags` (and orientation 1); `None` when none of them is there.
fn only(e: &Exif, tags: &[u16]) -> Option<Exif> {
    let mut ifd0: Vec<Entry> = e
        .ifd0
        .iter()
        .filter(|x| tags.contains(&x.tag))
        .cloned()
        .collect();
    if ifd0.is_empty() {
        return None;
    }
    ifd0.push(Entry::short(TAG_ORIENTATION, &[1]));
    Some(Exif {
        ifd0,
        exif: Vec::new(),
        gps: Vec::new(),
    })
}
