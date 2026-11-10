//! `multipart/related` responses (RFC 2046 / 2387, as WADO-RS uses them), parsed as the body
//! streams in: feed chunks of any size, get whole parts out.

use memchr::memmem;

/// Media-type parameters: `type/subtype; a=b; c="d;e"` → (`type/subtype`, [(a,b), (c,d;e)]).
/// Names are lower-cased; quoted values are unquoted (with `\"` escapes).
pub fn media_type(value: &str) -> (String, Vec<(String, String)>) {
    let mut params = Vec::new();
    let b = value.as_bytes();
    let first = value.find(';').unwrap_or(value.len());
    let mt = value[..first].trim().to_ascii_lowercase();
    let mut i = first;
    while i < b.len() {
        // Skip `;` and spaces.
        while i < b.len() && (b[i] == b';' || b[i].is_ascii_whitespace()) {
            i += 1;
        }
        let ns = i;
        while i < b.len() && b[i] != b'=' && b[i] != b';' {
            i += 1;
        }
        let name = value[ns..i].trim().to_ascii_lowercase();
        if i >= b.len() || b[i] == b';' {
            if !name.is_empty() {
                params.push((name, String::new()));
            }
            continue;
        }
        i += 1; // '='
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut v = String::new();
        if i < b.len() && b[i] == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 1;
                }
                // Values are ASCII in practice; keep UTF-8 intact anyway.
                let ch = value[i..].chars().next().unwrap_or('?');
                v.push(ch);
                i += ch.len_utf8();
            }
            i += 1; // closing quote
            while i < b.len() && b[i] != b';' {
                i += 1;
            }
        } else {
            let vs = i;
            while i < b.len() && b[i] != b';' {
                i += 1;
            }
            v = value[vs..i].trim().to_string();
        }
        if !name.is_empty() {
            params.push((name, v));
        }
    }
    (mt, params)
}

/// The value of parameter `name` in a media type.
pub fn param(value: &str, name: &str) -> Option<String> {
    media_type(value)
        .1
        .into_iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

/// One body part.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Part {
    /// Part headers, names lower-cased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Part {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    /// Media type without parameters (`application/dicom`); `text/plain` if absent.
    pub fn content_type(&self) -> String {
        self.header("content-type")
            .map_or_else(|| "text/plain".into(), |v| media_type(v).0)
    }
    /// The `transfer-syntax` parameter of the part's Content-Type.
    pub fn transfer_syntax(&self) -> Option<String> {
        param(self.header("content-type")?, "transfer-syntax")
    }
    pub fn location(&self) -> Option<&str> {
        self.header("content-location")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Before the first boundary.
    Preamble,
    /// After a boundary line: reading part headers.
    Headers,
    Body,
    Done,
}

/// Incremental parser.
#[derive(Debug)]
pub struct Multipart {
    /// `\r\n--boundary`.
    delim: Vec<u8>,
    buf: Vec<u8>,
    /// Where the next delimiter search in `buf` starts.
    scanned: usize,
    state: State,
    part: Part,
    /// Size limit per part.
    max_part: usize,
}

/// The largest body part (one instance, frame or bulk data item) read by default: a server
/// sending more must not fill memory.
pub const MAX_PART: usize = 1 << 30;

/// Parse failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MultipartError {
    NoBoundary,
    PartTooLarge,
    BadHeaders,
    /// The body ended before the closing boundary.
    Truncated,
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MultipartError::NoBoundary => "multipart response without a boundary",
            MultipartError::PartTooLarge => "multipart part too large",
            MultipartError::BadHeaders => "malformed multipart part headers",
            MultipartError::Truncated => "multipart response ended early",
        })
    }
}

impl std::error::Error for MultipartError {}

impl Multipart {
    /// A parser for a response with this `Content-Type`.
    pub fn new(content_type: &str) -> Result<Multipart, MultipartError> {
        let b = param(content_type, "boundary").ok_or(MultipartError::NoBoundary)?;
        if b.is_empty() || b.len() > 200 {
            return Err(MultipartError::NoBoundary);
        }
        Ok(Multipart::with_boundary(&b))
    }

    pub fn with_boundary(boundary: &str) -> Multipart {
        let mut delim = b"\r\n--".to_vec();
        delim.extend_from_slice(boundary.as_bytes());
        // The first boundary may start the body without a preceding CRLF.
        let buf = b"\r\n".to_vec();
        Multipart {
            delim,
            buf,
            scanned: 0,
            state: State::Preamble,
            part: Part::default(),
            max_part: MAX_PART,
        }
    }

    pub fn max_part(mut self, bytes: usize) -> Self {
        self.max_part = bytes;
        self
    }

    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }

    /// Bytes of the part being received (for progress).
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    /// Adds bytes; calls `on_part` for every part completed by them.
    pub fn feed(
        &mut self,
        data: &[u8],
        mut on_part: impl FnMut(Part),
    ) -> Result<(), MultipartError> {
        if self.state == State::Done {
            return Ok(());
        }
        self.buf.extend_from_slice(data);
        while self.step(&mut on_part)? {}
        if self.buf.len() > self.max_part.saturating_add(self.delim.len() + 4096) {
            return Err(MultipartError::PartTooLarge);
        }
        Ok(())
    }

    /// Call when the body ended.
    pub fn finish(&mut self) -> Result<(), MultipartError> {
        if self.state == State::Done {
            Ok(())
        } else {
            Err(MultipartError::Truncated)
        }
    }

    /// One state transition if the buffer allows; `Ok(true)` when it made progress.
    fn step(&mut self, on_part: &mut impl FnMut(Part)) -> Result<bool, MultipartError> {
        match self.state {
            State::Done => Ok(false),
            State::Preamble | State::Body => {
                let from = self.scanned.min(self.buf.len());
                let Some(at) = memmem::find(&self.buf[from..], &self.delim).map(|i| i + from)
                else {
                    // A delimiter may straddle the end: rescan its length minus one.
                    self.scanned = self.buf.len().saturating_sub(self.delim.len() - 1);
                    return Ok(false);
                };
                // After the delimiter: transport padding, then CRLF (next part) or `--` (end).
                let after = at + self.delim.len();
                let rest = &self.buf[after..];
                let pad = rest
                    .iter()
                    .take_while(|b| **b == b' ' || **b == b'\t')
                    .count();
                let rest = &rest[pad..];
                if rest.len() < 2 {
                    // Need more bytes to tell.
                    self.scanned = at;
                    return Ok(false);
                }
                let closing = rest.starts_with(b"--");
                let next = rest.starts_with(b"\r\n");
                if !closing && !next {
                    // `\r\n--boundary` followed by more text is part of the body (the boundary
                    // must not occur in content, but a longer string with it as prefix may).
                    self.scanned = at + 1;
                    return Ok(true);
                }
                if self.state == State::Body {
                    let mut body = std::mem::take(&mut self.buf);
                    let tail = body.split_off(at);
                    self.part.body = body;
                    on_part(std::mem::take(&mut self.part));
                    self.buf = tail;
                } else {
                    self.buf.drain(..at);
                }
                let skip = self.delim.len() + pad + 2;
                self.buf.drain(..skip);
                self.scanned = 0;
                self.state = if closing { State::Done } else { State::Headers };
                if closing {
                    self.buf.clear();
                }
                Ok(true)
            }
            State::Headers => {
                // An empty header block is a bare CRLF right away.
                let end = if self.buf.starts_with(b"\r\n") {
                    Some((0, 2))
                } else {
                    memmem::find(&self.buf, b"\r\n\r\n").map(|i| (i, i + 4))
                };
                let Some((hend, body_start)) = end else {
                    if self.buf.len() > 64 * 1024 {
                        return Err(MultipartError::BadHeaders);
                    }
                    return Ok(false);
                };
                let text = std::str::from_utf8(&self.buf[..hend])
                    .map_err(|_| MultipartError::BadHeaders)?;
                let mut headers: Vec<(String, String)> = Vec::new();
                for line in text.split("\r\n") {
                    if line.is_empty() {
                        continue;
                    }
                    if line.starts_with([' ', '\t']) {
                        // Folded continuation.
                        if let Some(last) = headers.last_mut() {
                            last.1.push(' ');
                            last.1.push_str(line.trim());
                        }
                        continue;
                    }
                    let (k, v) = line.split_once(':').ok_or(MultipartError::BadHeaders)?;
                    headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
                }
                self.part.headers = headers;
                // An empty body leaves `\r\n--boundary` at the front, which the search finds.
                self.buf.drain(..body_start);
                self.state = State::Body;
                self.scanned = 0;
                Ok(true)
            }
        }
    }
}
