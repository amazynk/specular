//! Text-like files as documents: plain text and logs in any common character set (DOS art
//! in code page 437), Markdown, CSV and TSV tables and RTF.
//!
//! Plain text and tables are indexed where they lie ([`lines`], [`table`]): a mapped log of
//! gigabytes opens after one pass over it and shows any line at once, without being read
//! into memory or decoded whole. Markdown ([`markdown`]) and RTF ([`rtf`]) become XHTML that
//! `specular-epub` lays out into pages; [`preview`] makes the same of the start of a text
//! file or table (for thumbnails and printing).

use std::sync::Arc;

pub mod encoding;
pub mod lines;
pub mod markdown;
pub mod preview;
pub mod rtf;
pub mod scan;
pub mod table;

pub use encoding::Charset;
pub use lines::LineIndex;
pub use table::Table;

/// The bytes of a file: mapped or in memory.
pub type Bytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// Largest UTF-16 file shown (it is converted to UTF-8 in memory first).
pub const MAX_UTF16: usize = 256 << 20;
/// Characters of one line shown; the rest of a longer line is cut.
pub const MAX_LINE_CHARS: usize = 20_000;

/// Why a file cannot be shown as text, in plain words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The bytes are not text.
    Binary,
    TooLarge(String),
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Binary => f.write_str("the file holds binary data, not text"),
            Error::TooLarge(s) => write!(f, "too large: {s}"),
            Error::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for Error {}

/// The bytes to index: the file's own when it is UTF-8 or an ASCII-compatible code page,
/// else (UTF-16) converted to UTF-8. Returns (bytes, start after the mark, the set to
/// decode them with).
fn prepare(data: Bytes, d: encoding::Detected) -> Result<(Bytes, usize, Charset), Error> {
    if !d.charset.is_utf16() {
        return Ok((data, d.bom, d.charset));
    }
    let raw = (*data).as_ref();
    if raw.len() > MAX_UTF16 {
        return Err(Error::TooLarge(format!(
            "UTF-16 text over {} MB",
            MAX_UTF16 >> 20
        )));
    }
    let s = d.charset.decode(&raw[d.bom.min(raw.len())..]);
    let v: Vec<u8> = s.into_owned().into_bytes();
    Ok((Arc::new(v), 0, Charset::Utf8))
}

/// A text file: its character set and line index over its bytes.
pub struct TextFile {
    data: Bytes,
    start: usize,
    decode: Charset,
    /// The file's character set.
    pub charset: Charset,
    pub index: LineIndex,
}

impl std::fmt::Debug for TextFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextFile")
            .field("charset", &self.charset)
            .field("lines", &self.index.len())
            .finish()
    }
}

/// `s` cut to [`MAX_LINE_CHARS`] characters, with an ellipsis when cut.
fn clip(s: std::borrow::Cow<'_, str>) -> String {
    match s.char_indices().nth(MAX_LINE_CHARS) {
        Some((i, _)) => {
            let mut o = s[..i].to_owned();
            o.push('…');
            o
        }
        None => s.into_owned(),
    }
}

impl TextFile {
    /// Opens `data` as text; `dos` reads non-UTF-8 bytes as code page 437 (`.nfo`, `.diz`).
    pub fn open(data: Bytes, dos: bool, cancel: &dyn Fn() -> bool) -> Result<TextFile, Error> {
        let raw = (*data).as_ref();
        let d = encoding::detect(&raw[..raw.len().min(encoding::SAMPLE)], dos, None)
            .ok_or(Error::Binary)?;
        let (data, start, decode) = prepare(data, d)?;
        let index = LineIndex::build((*data).as_ref(), start, cancel).ok_or(Error::Cancelled)?;
        Ok(TextFile {
            data,
            start,
            decode,
            charset: d.charset,
            index,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        (*self.data).as_ref()
    }

    /// The text without the byte-order mark (in the decode set).
    pub fn body(&self) -> &[u8] {
        &self.bytes()[self.start..]
    }

    /// Up to `n` lines from `first`, decoded (very long lines cut).
    pub fn lines(&self, first: u64, n: usize) -> Vec<String> {
        self.index
            .lines(self.bytes(), first, n)
            .into_iter()
            .map(|l| clip(self.decode.decode(l)))
            .collect()
    }

    /// The first `max` bytes of text, decoded (for search and previews).
    pub fn head_text(&self, max: usize) -> String {
        let b = self.body();
        let mut end = b.len().min(max);
        // Not inside a UTF-8 character.
        while end < b.len() && end > 0 && (b[end] & 0xC0) == 0x80 {
            end -= 1;
        }
        self.decode.decode(&b[..end]).into_owned()
    }
}

/// A CSV or TSV file: its character set and row index.
pub struct TableFile {
    data: Bytes,
    decode: Charset,
    pub charset: Charset,
    pub table: Table,
}

impl std::fmt::Debug for TableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableFile")
            .field("charset", &self.charset)
            .field("rows", &self.table.len())
            .field("columns", &self.table.columns)
            .finish()
    }
}

impl TableFile {
    /// Opens `data` as a table; `delimiter` forces one (TSV's tab).
    pub fn open(
        data: Bytes,
        delimiter: Option<u8>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<TableFile, Error> {
        let raw = (*data).as_ref();
        let d = encoding::detect(&raw[..raw.len().min(encoding::SAMPLE)], false, None)
            .ok_or(Error::Binary)?;
        let (data, start, decode) = prepare(data, d)?;
        let table =
            Table::build((*data).as_ref(), start, delimiter, cancel).ok_or(Error::Cancelled)?;
        Ok(TableFile {
            data,
            decode,
            charset: d.charset,
            table,
        })
    }

    /// Up to `n` rows from `first`, their fields decoded (very long ones cut).
    pub fn rows(&self, first: u64, n: usize) -> Vec<Vec<String>> {
        self.table
            .rows((*self.data).as_ref(), first, n)
            .into_iter()
            .map(|r| r.iter().map(|f| clip(self.decode.decode(f))).collect())
            .collect()
    }

    /// The delimiter as the info panel shows it: the character, `⇥` for a tab.
    pub fn delimiter_symbol(&self) -> String {
        match self.table.delimiter {
            b'\t' => "⇥".to_owned(),
            d => (d as char).to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_files() {
        let t = TextFile::open(Arc::new(b"one\r\ntwo\r\n".to_vec()), false, &|| false).unwrap();
        assert_eq!(t.lines(0, 5), ["one", "two"]);
        assert_eq!(t.charset, Charset::Utf8);
        // UTF-16 with a mark: converted, lines found.
        let mut u = vec![0xFF, 0xFE];
        for c in "a\nשלום\n".encode_utf16() {
            u.extend_from_slice(&c.to_le_bytes());
        }
        let t = TextFile::open(Arc::new(u), false, &|| false).unwrap();
        assert_eq!(t.charset, Charset::Utf16Le);
        assert_eq!(t.lines(0, 5), ["a", "שלום"]);
        let nfo = TextFile::open(Arc::new(b"\xC9\xCD\xBB\n".to_vec()), true, &|| false).unwrap();
        assert_eq!(nfo.lines(0, 1), ["╔═╗"]);
        assert_eq!(
            TextFile::open(Arc::new(vec![0u8, 1, 2, 3, 0, 0, 7]), false, &|| false).unwrap_err(),
            Error::Binary
        );
        let long = "x".repeat(MAX_LINE_CHARS + 5);
        let t = TextFile::open(Arc::new(long.into_bytes()), false, &|| false).unwrap();
        assert_eq!(t.lines(0, 1)[0].chars().count(), MAX_LINE_CHARS + 1);
    }

    #[test]
    fn table_files() {
        let t = TableFile::open(Arc::new(b"a;b\n1;2\n".to_vec()), None, &|| false).unwrap();
        assert_eq!(t.delimiter_symbol(), ";");
        assert_eq!(t.rows(0, 9), [["a", "b"], ["1", "2"]]);
    }
}
