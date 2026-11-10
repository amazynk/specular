//! A line index over text bytes (UTF-8 or an ASCII-compatible code page): where every 64th
//! line starts, so any line is found by scanning at most 63 others. A file of 20 million
//! lines costs about 2.5 MB of index; the bytes stay where they are (mapped).

use crate::scan;

/// Lines between index marks.
pub const STEP: u64 = 64;

/// How the lines end, counted over the file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Endings {
    pub lf: u64,
    pub crlf: u64,
    /// Old Mac files: carriage returns alone.
    pub cr: u64,
}

impl Endings {
    /// "LF", "CRLF", "CR", or the kinds found joined ("LF + CRLF"); empty for none.
    pub fn name(&self) -> String {
        [(self.lf, "LF"), (self.crlf, "CRLF"), (self.cr, "CR")]
            .iter()
            .filter(|(n, _)| *n > 0)
            .map(|(_, s)| *s)
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

#[derive(Clone, Debug)]
pub struct LineIndex {
    start: usize,
    end: usize,
    sep: u8,
    marks: Vec<u64>,
    lines: u64,
    /// The longest line, in bytes.
    pub longest: usize,
    pub endings: Endings,
}

impl LineIndex {
    /// Indexes `data[start..]`. `cancel` is asked now and then; `None` when it says stop.
    pub fn build(data: &[u8], start: usize, cancel: &dyn Fn() -> bool) -> Option<LineIndex> {
        let end = data.len();
        let start = start.min(end);
        let body = &data[..end];
        // Carriage returns alone end lines only when no line feed does.
        let sep = if scan::find(body, start, b'\n').is_none()
            && scan::find(body, start, b'\r').is_some()
        {
            b'\r'
        } else {
            b'\n'
        };
        let mut marks = vec![start as u64];
        let mut lines = 0u64;
        let mut longest = 0usize;
        let mut endings = Endings::default();
        let mut at = start;
        while at < end {
            let nl = scan::find(body, at, sep);
            let stop = nl.unwrap_or(end);
            let mut len = stop - at;
            if sep == b'\n' && nl.is_some() {
                if len > 0 && body[stop - 1] == b'\r' {
                    endings.crlf += 1;
                    len -= 1;
                } else {
                    endings.lf += 1;
                }
            } else if nl.is_some() {
                endings.cr += 1;
            }
            longest = longest.max(len);
            lines += 1;
            at = stop + 1;
            if lines.is_multiple_of(STEP) && at < end {
                marks.push(at as u64);
            }
            if lines.is_multiple_of(1 << 20) && cancel() {
                return None;
            }
        }
        Some(LineIndex {
            start,
            end,
            sep,
            marks,
            lines,
            longest,
            endings,
        })
    }

    /// How many lines (a final line end does not start another).
    pub fn len(&self) -> u64 {
        self.lines
    }

    pub fn is_empty(&self) -> bool {
        self.lines == 0
    }

    /// Up to `n` lines from line `first`, without their line ends.
    pub fn lines<'a>(&self, data: &'a [u8], first: u64, n: usize) -> Vec<&'a [u8]> {
        let mut out = Vec::with_capacity(n.min(4096));
        if first >= self.lines || data.len() < self.end {
            return out;
        }
        let mark = (first / STEP) as usize;
        let mut at = self.marks.get(mark).copied().unwrap_or(self.start as u64) as usize;
        let mut line = mark as u64 * STEP;
        while line < self.lines && out.len() < n && at <= self.end {
            let stop = scan::find(&data[..self.end], at, self.sep).unwrap_or(self.end);
            if line >= first {
                let mut s = &data[at..stop];
                if self.sep == b'\n' && s.last() == Some(&b'\r') {
                    s = &s[..s.len() - 1];
                }
                out.push(s);
            }
            line += 1;
            at = stop + 1;
        }
        out
    }

    /// Line `i`, without its line end.
    pub fn line<'a>(&self, data: &'a [u8], i: u64) -> Option<&'a [u8]> {
        self.lines(data, i, 1).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_matches_split() {
        let mut text = String::new();
        for i in 0..1000 {
            text.push_str(&"x".repeat(i % 17));
            text.push_str(if i % 3 == 0 { "\r\n" } else { "\n" });
        }
        text.push_str("tail");
        let data = text.as_bytes();
        let idx = LineIndex::build(data, 0, &|| false).unwrap();
        let want: Vec<&str> = text.lines().collect();
        assert_eq!(idx.len(), want.len() as u64);
        for first in [0u64, 1, 63, 64, 65, 500, 999, 1000] {
            let got = idx.lines(data, first, 70);
            let exp: Vec<&[u8]> = want
                .iter()
                .skip(first as usize)
                .take(70)
                .map(|s| s.as_bytes())
                .collect();
            assert_eq!(got, exp, "from {first}");
        }
        assert_eq!(idx.line(data, 1000), Some(&b"tail"[..]));
        assert_eq!(idx.line(data, 1001), None);
        assert_eq!(idx.longest, 16);
        assert_eq!(idx.endings.crlf, 334);
        assert_eq!(idx.endings.name(), "LF + CRLF");
    }

    #[test]
    fn edges() {
        let i = LineIndex::build(b"", 0, &|| false).unwrap();
        assert!(i.is_empty());
        let d = b"a\rb\rc";
        let i = LineIndex::build(d, 0, &|| false).unwrap();
        assert_eq!(i.lines(d, 0, 9), [&b"a"[..], b"b", b"c"]);
        assert_eq!(i.endings.name(), "CR");
        // A byte-order mark is skipped; a final newline adds no line.
        let d = b"\xEF\xBB\xBFone\ntwo\n";
        let i = LineIndex::build(d, 3, &|| false).unwrap();
        assert_eq!(i.len(), 2);
        assert_eq!(i.line(d, 0), Some(&b"one"[..]));
        assert_eq!(i.endings.name(), "LF");
        assert!(LineIndex::build(&vec![b'\n'; 3 << 20], 0, &|| true).is_none());
    }
}
