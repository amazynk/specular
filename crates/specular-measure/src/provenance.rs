//! Where exported measurements came from: the build that made them, the decoder that read
//! the pixels, and the file's content hash.

use std::io;
use std::path::Path;

/// Recorded in every export.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Provenance {
    /// The program and its version, e.g. `Specular 0.1.0`.
    pub software: String,
    /// The image file's name (not its path).
    pub file: String,
    /// The decoder that read the pixels and its version.
    pub decoder: Option<String>,
    /// The file's BLAKE3 hash, lower-case hex.
    pub blake3: Option<String>,
    /// The image's stored size.
    pub size: Option<(u32, u32)>,
    /// When the export was made (RFC 3339), if the caller supplies it.
    pub created: Option<String>,
}

impl Provenance {
    /// `(key, value)` pairs, in a fixed order, of the fields that are set.
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        let mut v = vec![
            ("software", self.software.clone()),
            ("file", self.file.clone()),
        ];
        if let Some(d) = &self.decoder {
            v.push(("decoder", d.clone()));
        }
        if let Some(h) = &self.blake3 {
            v.push(("blake3", h.clone()));
        }
        if let Some((w, h)) = self.size {
            v.push(("size", format!("{w}x{h}")));
        }
        if let Some(c) = &self.created {
            v.push(("created", c.clone()));
        }
        v
    }

    /// Parses what [`fields`](Self::fields) wrote.
    pub fn from_fields<'a>(fields: impl IntoIterator<Item = (&'a str, &'a str)>) -> Provenance {
        let mut p = Provenance::default();
        for (k, v) in fields {
            match k {
                "software" => p.software = v.to_owned(),
                "file" => p.file = v.to_owned(),
                "decoder" => p.decoder = Some(v.to_owned()),
                "blake3" => p.blake3 = Some(v.to_owned()),
                "size" => {
                    p.size = v
                        .split_once('x')
                        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                }
                "created" => p.created = Some(v.to_owned()),
                _ => {}
            }
        }
        p
    }
}

/// The BLAKE3 hash of a file, lower-case hex, read in blocks.
pub fn hash_file(path: &Path) -> io::Result<String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(std::fs::File::open(path)?)?;
    Ok(h.finalize().to_hex().to_string())
}

/// The BLAKE3 hash of bytes, lower-case hex.
pub fn hash_bytes(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}
