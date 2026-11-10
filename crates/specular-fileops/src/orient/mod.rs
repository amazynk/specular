//! Permanent rotation: the user's quarter turns and flips written into the file, so other
//! programs show the image the same way.
//!
//! - JPEG: the DCT blocks are rearranged (no re-encoding, no quality lost) when the image
//!   allows it; when it would cut a partial edge block, only with the user's consent
//!   ([`RotateOptions::trim`]); otherwise, and for files with more than one picture or data
//!   after the image, the Exif Orientation tag is set instead. [`Method`] says which happened.
//! - TIFF, PNG, WebP, JPEG XL, HEIF and AVIF: the orientation field the format's readers
//!   honour, nothing else.
//! - RAW (any TIFF-based RAW, DNG included, and every other RAW container): never written.
//!   `tiff:Orientation` goes into the XMP sidecar, which the RAW decoder reads on open.
//!
//! Every write replaces the whole file through a temporary in the same folder (flushed, then
//! renamed over), so a crash leaves either the old file or the new one.

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use specular_jpeg::Transform;
pub use xmp::sidecar_orientation;

use crate::atomic::{self, Stamp};
use crate::history::{Kind, Op, Step};

pub mod heif;
pub mod jpeg;
pub mod jxl;
pub mod png;
pub(crate) mod tiff;
pub mod webp;
pub mod xmp;

/// Largest file rotated (it is held in memory twice, plus once more for undo).
const MAX_BYTES: u64 = 1 << 30;

/// Why a JPEG got the tag rather than the lossless rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagReason {
    /// The JPEG data couldn't be parsed far enough to rearrange it.
    Unreadable,
    /// The file holds more than one picture (MPF, a motion photo): rearranging the first
    /// would break the others' offsets.
    MultiPicture,
    /// Data follows the image; rearranging would move or drop it.
    Trailer,
    /// The size isn't a whole number of blocks along an edge that would move; the lossless
    /// route would trim the image to `width` × `height`, which the user didn't allow.
    NotAligned { width: u32, height: u32 },
}

impl TagReason {
    pub fn message_id(self) -> &'static str {
        match self {
            TagReason::Unreadable => "rotate-tag-unreadable",
            TagReason::MultiPicture => "rotate-tag-multi-picture",
            TagReason::Trailer => "rotate-tag-trailer",
            TagReason::NotAligned { .. } => "rotate-tag-not-aligned",
        }
    }
}

/// How the rotation was saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// The JPEG's pixels were rearranged without re-encoding. `trimmed` is the new stored
    /// size when a partial edge block was dropped (with consent).
    Lossless { trimmed: Option<(u32, u32)> },
    /// The orientation tag was set. For a JPEG, `reason` says why the lossless route wasn't
    /// taken; other formats only have the tag.
    Tag { reason: Option<TagReason> },
    /// RAW: written to the XMP sidecar at this path.
    Sidecar,
}

impl Method {
    pub fn message_id(self) -> &'static str {
        match self {
            Method::Lossless { trimmed: None } => "rotate-done-lossless",
            Method::Lossless { trimmed: Some(_) } => "rotate-done-lossless-trimmed",
            Method::Tag { .. } => "rotate-done-tag",
            Method::Sidecar => "rotate-done-sidecar",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RotateOptions {
    /// The user agreed to lose a partial edge block of a JPEG for a lossless rotation.
    pub trim: bool,
    /// Keep the file's modification time.
    pub keep_mtime: bool,
    /// For a RAW without a sidecar orientation: how it is shown now (the camera's
    /// orientation as the decoder reads it); the identity when unknown.
    pub displayed: Option<Transform>,
}

/// A saved rotation.
#[derive(Clone, Debug)]
pub struct Rotated {
    pub method: Method,
    /// The journal entry (empty when nothing had to change).
    pub op: Op,
    /// The orientation now recorded (Exif numbering); 1 after a lossless JPEG rotation.
    pub orientation: u8,
    /// The file written: the image, or its sidecar.
    pub written: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RotateError {
    /// Not a format whose rotation can be saved (GIF, BMP, PSD, ...).
    Unsupported,
    /// Too big to rotate in memory.
    TooLarge,
    /// The file is damaged where the orientation goes; carries what was wrong.
    Damaged(String),
    /// The system refused to read or write; carries its message.
    Io(String),
}

impl RotateError {
    pub fn message_id(&self) -> &'static str {
        match self {
            RotateError::Unsupported => "rotate-problem-unsupported",
            RotateError::TooLarge => "rotate-problem-too-large",
            RotateError::Damaged(_) => "rotate-problem-damaged",
            RotateError::Io(_) => "rotate-problem-io",
        }
    }
}

fn io(e: std::io::Error) -> RotateError {
    RotateError::Io(e.to_string())
}

/// What is in the file, by its first bytes (and, for RAW, by its name or a DNG tag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Jxl,
    Heif,
    Tiff,
    Raw,
}

/// Extensions of the formats [`rotate`] handles (a RAW goes by [`crate::names::is_raw`]).
const ROTATABLE: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "webp", "jxl", "heic", "heif", "hif", "avif", "tif",
    "tiff",
];

/// Whether a file of this name can be rotated, judged by its extension alone (for the
/// enabled state of the actions; [`rotate`] checks the bytes).
pub fn rotatable_name(path: &Path) -> bool {
    crate::names::is_raw(path)
        || path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| ROTATABLE.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

pub fn detect(path: &Path, head: &[u8]) -> Option<Format> {
    if crate::names::is_raw(path) {
        return Some(Format::Raw);
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(Format::Jpeg);
    }
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(Format::Png);
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(Format::Webp);
    }
    if head.starts_with(&[0xFF, 0x0A]) || head.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
        return Some(Format::Jxl);
    }
    if head.get(4..8) == Some(b"ftyp") {
        const BRANDS: &[&[u8; 4]] = &[
            b"heic", b"heix", b"heim", b"heis", b"mif1", b"mif2", b"avif", b"avis", b"miaf",
        ];
        let size = u32::from_be_bytes(head[..4].try_into().ok()?) as usize;
        let ftyp = head.get(8..size.min(head.len()))?;
        let major = ftyp.get(..4)?;
        let compat = ftyp.get(8..).unwrap_or(&[]).chunks_exact(4);
        if std::iter::once(major)
            .chain(compat)
            .any(|b| BRANDS.iter().any(|x| &x[..] == b))
        {
            return Some(Format::Heif);
        }
        return None;
    }
    if let Some(t) = tiff::Tiff::detect(head) {
        // A DNG under another name is still a RAW.
        let dng = t.ifd0(head).and_then(|i| t.find(head, i, 0xC612)).is_some();
        return Some(if dng { Format::Raw } else { Format::Tiff });
    }
    None
}

/// The orientation recorded in `d` (Exif numbering, 1 when there is none).
pub fn current(format: Format, d: &[u8]) -> u8 {
    match format {
        Format::Jpeg => jpeg::orientation(d),
        Format::Png => png::orientation(d).unwrap_or(1),
        Format::Webp => webp::orientation(d).unwrap_or(1),
        Format::Jxl => jxl::orientation(d).unwrap_or(1),
        Format::Heif => heif::orientation(d).unwrap_or(1),
        Format::Tiff => tiff::orientation(d).unwrap_or(1),
        Format::Raw => 1,
    }
}

/// What [`rotate`] would do, without writing: for asking before a JPEG is trimmed. `None`
/// for the lossless route without trimming, else the reason the tag would be used.
pub fn jpeg_plan(path: &Path, t: Transform) -> Result<Option<TagReason>, RotateError> {
    let d = read(path)?;
    let total = Transform::from_exif(jpeg::orientation(&d)).then(t);
    if total == Transform::IDENTITY {
        return Ok(None);
    }
    Ok(jpeg::lossless_blocker(&d, total, false))
}

fn read(path: &Path) -> Result<Vec<u8>, RotateError> {
    let m = std::fs::metadata(path).map_err(io)?;
    if !m.is_file() {
        return Err(RotateError::Unsupported);
    }
    if m.len() > MAX_BYTES {
        return Err(RotateError::TooLarge);
    }
    std::fs::read(path).map_err(io)
}

/// The new bytes for an image file, the method, and the orientation recorded.
pub fn rotated_bytes(
    format: Format,
    d: &[u8],
    t: Transform,
    trim: bool,
) -> Result<(Vec<u8>, Method, u8), RotateError> {
    let damaged = |what: &str| RotateError::Damaged(what.to_owned());
    let tag = Method::Tag { reason: None };
    let simple = |set: &dyn Fn(u8) -> Option<Vec<u8>>| {
        let o = Transform::from_exif(current(format, d)).then(t).to_exif();
        let out = set(o).ok_or_else(|| damaged("the orientation can't be written"))?;
        Ok((out, tag, o))
    };
    match format {
        Format::Jpeg => {
            let total = Transform::from_exif(jpeg::orientation(d)).then(t);
            let o = total.to_exif();
            if total == Transform::IDENTITY {
                let out = jpeg::set_tag(d, 1).ok_or_else(|| damaged("the Exif block"))?;
                return Ok((out, Method::Lossless { trimmed: None }, 1));
            }
            let reason = jpeg::lossless_blocker(d, total, trim);
            if reason.is_none() {
                let trimmed = if trim {
                    jpeg::trimmed_size(d, total)
                } else {
                    None
                };
                match jpeg::lossless(d, total, trim) {
                    Ok(out) => return Ok((out, Method::Lossless { trimmed }, 1)),
                    Err(e) => tracing::warn!("lossless rotation failed, writing the tag: {e}"),
                }
            }
            let out = jpeg::set_tag(d, o).ok_or_else(|| damaged("the Exif block is too large"))?;
            Ok((
                out,
                Method::Tag {
                    reason: Some(reason.unwrap_or(TagReason::Unreadable)),
                },
                o,
            ))
        }
        Format::Png => simple(&|o| png::set_orientation(d, o)),
        Format::Webp => simple(&|o| webp::set_orientation(d, o)),
        Format::Jxl => simple(&|o| jxl::set_orientation(d, o)),
        Format::Tiff => simple(&|o| {
            let mut v = d.to_vec();
            tiff::set_orientation(&mut v, o)?;
            Some(v)
        }),
        Format::Heif => {
            let (out, o) = heif::rotate(d, t).map_err(damaged)?;
            Ok((out, tag, o))
        }
        Format::Raw => Err(RotateError::Unsupported),
    }
}

/// Save the rotation `t` (applied after how the image is shown now) into `path`.
///
/// Specular never modifies images, with this one exception: permanent rotation is an
/// explicit user action, and it is lossless where the format allows (JPEG blocks
/// rearranged, or only an orientation field changed; RAW files are never written, their
/// rotation goes to the XMP sidecar). The old bytes are journalled so it can be undone.
pub fn rotate(path: &Path, t: Transform, opts: RotateOptions) -> Result<Rotated, RotateError> {
    let mut head = [0u8; 4096];
    let n = {
        use std::io::Read;
        let mut f = std::fs::File::open(path).map_err(io)?;
        let mut n = 0;
        while n < head.len() {
            match f.read(&mut head[n..]).map_err(io)? {
                0 => break,
                k => n += k,
            }
        }
        n
    };
    let format = detect(path, &head[..n]).ok_or(RotateError::Unsupported)?;
    if format == Format::Raw {
        return rotate_raw(path, t, opts);
    }
    let before = read(path)?;
    let before_stamp = Stamp::of(path).map_err(io)?;
    let (after, method, orientation) = rotated_bytes(format, &before, t, opts.trim)?;
    let mut op = Op::new(Kind::Rotate);
    if after != before {
        atomic::replace_file(path, &after, opts.keep_mtime).map_err(io)?;
        op.steps.push(Step::Wrote {
            path: path.to_path_buf(),
            before: Some(Arc::from(before)),
            before_stamp: Some(before_stamp),
            after: Arc::from(after),
            after_stamp: Stamp::of(path).map_err(io)?,
        });
    }
    Ok(Rotated {
        method,
        op,
        orientation,
        written: path.to_path_buf(),
    })
}

fn rotate_raw(image: &Path, t: Transform, opts: RotateOptions) -> Result<Rotated, RotateError> {
    let (side, exists) = xmp::sidecar_path(image);
    let old = if exists { Some(read(&side)?) } else { None };
    let shown = old
        .as_deref()
        .and_then(xmp::read_orientation)
        .map(Transform::from_exif)
        .or(opts.displayed)
        .unwrap_or(Transform::IDENTITY);
    let o = shown.then(t).to_exif();
    let text = old.as_deref().map(String::from_utf8_lossy);
    let after = xmp::update(text.as_deref(), o).into_bytes();
    let mut op = Op::new(Kind::Rotate);
    if old.as_deref() != Some(&after[..]) {
        let before_stamp = if exists {
            Some(Stamp::of(&side).map_err(io)?)
        } else {
            None
        };
        if exists {
            atomic::replace_file(&side, &after, opts.keep_mtime).map_err(io)?;
        } else {
            atomic::write_new(&side, &after).map_err(io)?;
        }
        op.steps.push(Step::Wrote {
            path: side.clone(),
            before: old.map(Arc::from),
            before_stamp,
            after: Arc::from(after),
            after_stamp: Stamp::of(&side).map_err(io)?,
        });
    }
    Ok(Rotated {
        method: Method::Sidecar,
        op,
        orientation: o,
        written: side,
    })
}
