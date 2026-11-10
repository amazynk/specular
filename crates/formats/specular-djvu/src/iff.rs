//! The IFF85 container of DjVu files: `AT&T` + `FORM` chunks holding chunks, each a 4-byte
//! id, a big-endian length and the data, padded to an even offset.

use crate::Error;

/// One chunk: its id and data, and where its header starts in the buffer it was read from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Chunk<'a> {
    pub id: [u8; 4],
    pub data: &'a [u8],
    pub offset: usize,
}

impl<'a> Chunk<'a> {
    /// For a `FORM`: its secondary id and the bytes of its children.
    pub fn form(&self) -> Option<([u8; 4], &'a [u8], usize)> {
        if &self.id != b"FORM" || self.data.len() < 4 {
            return None;
        }
        let kind = [self.data[0], self.data[1], self.data[2], self.data[3]];
        Some((kind, &self.data[4..], self.offset + 12))
    }
}

/// Iterates the chunks in `data`, whose first byte is at `base` in the file. Stops at the
/// end or at a chunk running past it (a truncated file keeps its complete chunks; a final
/// chunk cut short is returned with the bytes present).
pub(crate) struct Chunks<'a> {
    data: &'a [u8],
    pos: usize,
    base: usize,
}

pub(crate) fn chunks(data: &[u8], base: usize) -> Chunks<'_> {
    Chunks { data, pos: 0, base }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Chunk<'a>;
    fn next(&mut self) -> Option<Chunk<'a>> {
        let rest = self.data.get(self.pos..)?;
        if rest.len() < 8 {
            return None;
        }
        let id = [rest[0], rest[1], rest[2], rest[3]];
        let len = u32::from_be_bytes([rest[4], rest[5], rest[6], rest[7]]) as usize;
        let end = 8usize.saturating_add(len).min(rest.len());
        let chunk = Chunk {
            id,
            data: &rest[8..end],
            offset: self.base + self.pos,
        };
        self.pos = self.pos.saturating_add(8).saturating_add(len);
        self.pos = self.pos.saturating_add(self.pos & 1);
        if self.pos > self.data.len() {
            self.pos = usize::MAX;
        }
        Some(chunk)
    }
}

/// The top-level `FORM` of a DjVu file (after the `AT&T` magic), or of a bundled component
/// starting at `offset`.
pub(crate) fn top_form(data: &[u8], offset: usize) -> Result<Chunk<'_>, Error> {
    let start = if offset == 0 && data.starts_with(b"AT&T") {
        4
    } else {
        offset
    };
    let rest = data.get(start..).ok_or(Error::Malformed(
        "component offset past the end of the file",
    ))?;
    let c = chunks(rest, start)
        .next()
        .ok_or(Error::Malformed("no DjVu FORM chunk"))?;
    if &c.id != b"FORM" {
        return Err(Error::Malformed("no DjVu FORM chunk"));
    }
    Ok(c)
}

/// Whether `head` looks like a DjVu file.
pub fn looks_like_djvu(head: &[u8]) -> bool {
    head.len() >= 16
        && &head[..8] == b"AT&TFORM"
        && matches!(
            &head[12..16],
            b"DJVU" | b"DJVM" | b"DJVI" | b"THUM" | b"BM44" | b"PM44"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_pad_to_even_offsets_and_tolerate_truncation() {
        let mut d = Vec::new();
        d.extend_from_slice(b"ABCD\0\0\0\x03xyz\0");
        d.extend_from_slice(b"EFGH\0\0\0\x10ab");
        let v: Vec<_> = chunks(&d, 100).collect();
        assert_eq!(v.len(), 2);
        assert_eq!(&v[0].id, b"ABCD");
        assert_eq!(v[0].data, b"xyz");
        assert_eq!(v[1].offset, 112);
        assert_eq!(v[1].data, b"ab");
    }

    #[test]
    fn magic() {
        assert!(looks_like_djvu(b"AT&TFORM\0\0\0\x10DJVU"));
        assert!(!looks_like_djvu(b"AT&TFORM\0\0\0\x10ILBM"));
    }
}
