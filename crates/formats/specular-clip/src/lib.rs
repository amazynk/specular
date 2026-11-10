//! Clip Studio Paint illustrations (`.clip`).
//!
//! A `.clip` file is a chunk container: the signature `CSFCHUNK`, the file size and the
//! offset of the first chunk (big-endian 64-bit), then chunks of an 8-byte tag and a 64-bit
//! length: `CHNKHead`, any number of `CHNKExta` (layer tile data, named by an external id),
//! one `CHNKSQLi` (an SQLite database describing the document) and `CHNKFoot`.
//!
//! The database is read with an own read-only SQLite reader ([`sqlite`]). Its `Canvas` table
//! gives the canvas size and resolution, `Layer` the layer list, and `CanvasPreview` holds
//! the flattened canvas as Clip Studio composited it when saving (a PNG, at the canvas size
//! in the files seen). That merged image is what is shown: compositing the layers
//! ourselves (raster tiles, vector strokes, text, tone, filters and every blend mode) would
//! only approximate it.
//!
//! Refused with the reason: files without a preview, and damaged containers or databases.

pub mod sqlite;

use std::ops::Range;

/// Why a file cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a Clip Studio Paint file")]
    NotClip,
    #[error("{0}")]
    Unsupported(String),
    #[error("damaged Clip Studio Paint file: {0}")]
    Malformed(String),
}

/// The canvas as the document gives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Canvas {
    /// Pixels.
    pub width: f64,
    pub height: f64,
    /// Pixels per inch.
    pub dpi: f64,
    /// Bytes per channel (1 for 8-bit).
    pub channel_bytes: Option<i64>,
}

/// The merged canvas image.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub kind: ImageKind,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
}

/// One layer of the document's list.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub folder: bool,
    pub visible: bool,
}

/// What a `.clip` file holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// `Project.ProjectInternalVersion` ("1.1.0").
    pub version: Option<String>,
    pub canvas: Option<Canvas>,
    pub preview: Preview,
    /// Layers and folders, in table order (the root folder left out).
    pub layers: Vec<Layer>,
    /// `CHNKExta` chunks (layer data blocks).
    pub external_chunks: usize,
}

/// The chunk container's signature.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"CSFCHUNK")
}

fn be64(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

/// Walks the chunk list: the SQLite chunk's range and the number of external chunks.
fn chunks(data: &[u8]) -> Result<(Range<usize>, usize), Error> {
    if !probe(data) {
        return Err(Error::NotClip);
    }
    let first = be64(data, 16).ok_or_else(|| Error::Malformed("header cut short".into()))?;
    let mut pos = usize::try_from(first).unwrap_or(usize::MAX);
    let mut sqlite = None;
    let mut extra = 0usize;
    while let Some(tag) = data.get(pos..pos + 8) {
        let len =
            be64(data, pos + 8).ok_or_else(|| Error::Malformed("chunk header cut short".into()))?;
        let body = pos + 16;
        let end = usize::try_from(len)
            .ok()
            .and_then(|l| body.checked_add(l))
            .filter(|&e| e <= data.len());
        match tag {
            b"CHNKSQLi" => {
                let end = end.ok_or_else(|| Error::Malformed("database cut short".into()))?;
                sqlite = Some(body..end);
            }
            b"CHNKExta" => extra += 1,
            b"CHNKFoot" => break,
            b"CHNKHead" => {}
            _ if !tag.starts_with(b"CHNK") => {
                return Err(Error::Malformed("unknown chunk".into()));
            }
            _ => {}
        }
        match end {
            Some(e) => pos = e,
            None => break,
        }
    }
    let sqlite = sqlite.ok_or_else(|| {
        Error::Malformed("no document database (the file may be cut short)".into())
    })?;
    Ok((sqlite, extra))
}

/// Reads the facts, the layer list and the merged preview.
pub fn parse(data: &[u8]) -> Result<Clip, Error> {
    let (range, external_chunks) = chunks(data)?;
    let db = sqlite::Db::open(&data[range]).map_err(Error::Malformed)?;
    let first_row = |table: &str| -> Option<(sqlite::Table, sqlite::Row)> {
        let t = db.table(table)?.clone();
        let mut row = None;
        db.scan(&t, &mut |r| {
            row = Some(r);
            false
        })
        .ok()?;
        Some((t, row?))
    };
    let get = |t: &sqlite::Table, r: &sqlite::Row, c: &str| -> Option<sqlite::Value> {
        r.values.get(t.column(c)?).cloned()
    };
    let version = first_row("Project").and_then(|(t, r)| {
        get(&t, &r, "ProjectInternalVersion").and_then(|v| v.as_str().map(String::from))
    });
    let canvas = first_row("Canvas").and_then(|(t, r)| {
        let w = get(&t, &r, "CanvasWidth")?.as_f64()?;
        let h = get(&t, &r, "CanvasHeight")?.as_f64()?;
        let dpi = get(&t, &r, "CanvasResolution")
            .and_then(|v| v.as_f64())
            .unwrap_or(72.0);
        (w > 0.0 && h > 0.0).then_some(Canvas {
            width: w,
            height: h,
            dpi,
            channel_bytes: get(&t, &r, "CanvasChannelBytes").and_then(|v| v.as_i64()),
        })
    });
    let mut layers = Vec::new();
    if let Some(t) = db.table("Layer") {
        let (name, folder, vis, kind) = (
            t.column("LayerName"),
            t.column("LayerFolder"),
            t.column("LayerVisibility"),
            t.column("LayerType"),
        );
        let col = |r: &sqlite::Row, c: Option<usize>| c.and_then(|c| r.values.get(c).cloned());
        let _ = db.scan(t, &mut |r| {
            // The root folder (type 256) is the canvas itself.
            if col(&r, kind).and_then(|v| v.as_i64()) != Some(256) {
                layers.push(Layer {
                    name: col(&r, name)
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_default(),
                    folder: col(&r, folder).and_then(|v| v.as_i64()).unwrap_or(0) != 0,
                    visible: col(&r, vis).and_then(|v| v.as_i64()).unwrap_or(1) & 1 != 0,
                });
            }
            true
        });
    }
    let no_preview = || {
        Error::Unsupported(
            "Clip Studio Paint file without a canvas preview: its layers are not composited".into(),
        )
    };
    let (t, r) = first_row("CanvasPreview").ok_or_else(no_preview)?;
    let bytes = get(&t, &r, "ImageData")
        .and_then(sqlite::Value::into_blob)
        .ok_or_else(no_preview)?;
    let kind = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        ImageKind::Png
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        ImageKind::Jpeg
    } else {
        return Err(Error::Unsupported(
            "Clip Studio Paint canvas preview in an unknown image format".into(),
        ));
    };
    let dim = |c: &str| {
        get(&t, &r, c)
            .and_then(|v| v.as_i64())
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0)
    };
    Ok(Clip {
        version,
        canvas,
        preview: Preview {
            width: dim("ImageWidth"),
            height: dim("ImageHeight"),
            kind,
            bytes,
        },
        layers,
        external_chunks,
    })
}

impl Clip {
    /// A short description for the info panel: "Clip Studio Paint illustration, 12 layers
    /// in 3 folders, 350 dpi".
    pub fn describe(&self) -> String {
        let mut s = String::from("Clip Studio Paint illustration");
        let folders = self.layers.iter().filter(|l| l.folder).count();
        let layers = self.layers.len() - folders;
        if layers > 0 {
            s += &format!(", {layers} layer{}", if layers == 1 { "" } else { "s" });
            if folders > 0 {
                s += &format!(
                    " in {folders} folder{}",
                    if folders == 1 { "" } else { "s" }
                );
            }
        }
        if let Some(c) = self.canvas {
            if c.dpi > 0.0 {
                s += &format!(", {:.0} dpi", c.dpi);
            }
            let (pw, ph) = (
                f64::from(self.preview.width),
                f64::from(self.preview.height),
            );
            if pw > 0.0 && (pw + 0.5 < c.width || ph + 0.5 < c.height) {
                s += &format!(
                    ", preview of the {:.0}×{:.0} canvas at reduced size",
                    c.width, c.height
                );
            }
        }
        s
    }
}

#[cfg(test)]
mod tests;
