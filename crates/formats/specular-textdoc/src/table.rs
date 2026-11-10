//! Delimited tables (CSV, TSV): the delimiter guessed from the first lines (comma,
//! semicolon, tab or bar), RFC 4180 quoting (a field starting with `"` runs to the closing
//! quote, `""` inside is one quote, line ends inside it are part of the field), and an index
//! of where every 64th row starts, so any row of a mapped file of millions is found at once.

use std::borrow::Cow;

use crate::scan;

/// Rows between index marks.
pub const STEP: u64 = 64;
/// Rows looked at for the column count and the header guess.
const SAMPLE_ROWS: usize = 1000;

#[derive(Clone, Debug)]
pub struct Table {
    pub delimiter: u8,
    start: usize,
    end: usize,
    marks: Vec<u64>,
    rows: u64,
    /// The most fields in a sampled row.
    pub columns: usize,
    /// The first row names the columns.
    pub header: bool,
}

/// The delimiter of the lines in `sample` (outside quotes): the candidate whose count per
/// line is the same on most lines; comma when nothing settles it.
pub fn sniff_delimiter(sample: &[u8]) -> u8 {
    let lines: Vec<&[u8]> = sample.split(|&b| b == b'\n').take(50).collect();
    // A last line cut off by the sample's edge is left out.
    let lines = if lines.len() > 1 {
        &lines[..lines.len() - 1]
    } else {
        &lines[..]
    };
    let mut best = (b',', 0usize);
    for &d in b",;\t|" {
        let counts: Vec<usize> = lines
            .iter()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let mut q = false;
                l.iter()
                    .filter(|&&c| {
                        if c == b'"' {
                            q = !q;
                        }
                        c == d && !q
                    })
                    .count()
            })
            .collect();
        // The commonest non-zero count and how many lines have it.
        let mut freq: Vec<(usize, usize)> = Vec::new();
        for &c in counts.iter().filter(|&&c| c > 0) {
            match freq.iter_mut().find(|(k, _)| *k == c) {
                Some((_, n)) => *n += 1,
                None => freq.push((c, 1)),
            }
        }
        let score = freq.iter().map(|&(_, n)| n).max().unwrap_or(0);
        if score > best.1 {
            best = (d, score);
        }
    }
    best.0
}

/// Where the row starting at `at` ends (its line end, or `end`), skipping quoted fields.
fn row_end(data: &[u8], mut at: usize, end: usize, d: u8) -> usize {
    let data = &data[..end];
    let mut field_start = true;
    loop {
        if at >= end {
            return end;
        }
        if field_start && data[at] == b'"' {
            // A quoted field: to its closing quote (`""` is a quote inside).
            at += 1;
            loop {
                match scan::find(data, at, b'"') {
                    None => return end,
                    Some(q) if data.get(q + 1) == Some(&b'"') => at = q + 2,
                    Some(q) => {
                        at = q + 1;
                        break;
                    }
                }
            }
        }
        match scan::find2(data, at, d, b'\n') {
            None => return end,
            Some(i) if data[i] == b'\n' => return i,
            Some(i) => {
                at = i + 1;
                field_start = true;
            }
        }
    }
}

/// The fields of one row (`row` without its line end).
pub fn fields(row: &[u8], d: u8) -> Vec<Cow<'_, [u8]>> {
    let row = row.strip_suffix(b"\r").unwrap_or(row);
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        if at < row.len() && row[at] == b'"' {
            let mut v = Vec::new();
            at += 1;
            loop {
                match scan::find(row, at, b'"') {
                    None => {
                        v.extend_from_slice(&row[at..]);
                        at = row.len();
                        break;
                    }
                    Some(q) => {
                        v.extend_from_slice(&row[at..q]);
                        if row.get(q + 1) == Some(&b'"') {
                            v.push(b'"');
                            at = q + 2;
                        } else {
                            at = q + 1;
                            break;
                        }
                    }
                }
            }
            // Anything between the closing quote and the delimiter is kept.
            let stop = scan::find(row, at, d).unwrap_or(row.len());
            v.extend_from_slice(&row[at.min(stop)..stop]);
            out.push(Cow::Owned(v));
            if stop >= row.len() {
                return out;
            }
            at = stop + 1;
        } else {
            let stop = scan::find(row, at, d).unwrap_or(row.len());
            out.push(Cow::Borrowed(&row[at..stop]));
            if stop >= row.len() {
                return out;
            }
            at = stop + 1;
        }
    }
}

fn blank(row: &[u8]) -> bool {
    row.is_empty() || row == b"\r"
}

fn numeric(f: &[u8]) -> bool {
    let s = String::from_utf8_lossy(f);
    let s = s.trim().trim_end_matches('%');
    !s.is_empty() && s.replace([',', ' '], "").parse::<f64>().is_ok()
}

impl Table {
    /// Indexes the rows of `data[start..]`; `delimiter` forces one (TSV), else it is guessed.
    /// `None` when `cancel` says stop.
    pub fn build(
        data: &[u8],
        start: usize,
        delimiter: Option<u8>,
        cancel: &dyn Fn() -> bool,
    ) -> Option<Table> {
        let end = data.len();
        let start = start.min(end);
        let d = delimiter.unwrap_or_else(|| {
            sniff_delimiter(&data[start..end.min(start + crate::encoding::SAMPLE)])
        });
        let mut marks = vec![start as u64];
        let mut rows = 0u64;
        let mut at = start;
        while at < end {
            let e = row_end(data, at, end, d);
            // Blank lines are no rows (as other readers skip them).
            if blank(&data[at..e]) {
                at = e + 1;
                continue;
            }
            rows += 1;
            at = e + 1;
            if rows.is_multiple_of(STEP) && at < end {
                marks.push(at as u64);
            }
            if rows.is_multiple_of(1 << 20) && cancel() {
                return None;
            }
        }
        let mut t = Table {
            delimiter: d,
            start,
            end,
            marks,
            rows,
            columns: 0,
            header: false,
        };
        let sample = t.rows(data, 0, SAMPLE_ROWS);
        t.columns = sample.iter().map(Vec::len).max().unwrap_or(0);
        t.header = sample.first().is_some_and(|first| {
            let text = first.iter().all(|f| !numeric(f));
            let below_numeric = sample.iter().skip(1).any(|r| r.iter().any(|f| numeric(f)));
            let mut names: Vec<&[u8]> = first.iter().map(|f| f.as_ref()).collect();
            let all_named = names.iter().all(|n| !n.is_empty());
            names.sort_unstable();
            names.dedup();
            text && sample.len() > 1 && (below_numeric || (all_named && names.len() == first.len()))
        });
        Some(t)
    }

    /// How many rows (the header included).
    pub fn len(&self) -> u64 {
        self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// The raw bytes of up to `n` rows from row `first`.
    pub fn raw_rows<'a>(&self, data: &'a [u8], first: u64, n: usize) -> Vec<&'a [u8]> {
        let mut out = Vec::with_capacity(n.min(4096));
        if first >= self.rows || data.len() < self.end {
            return out;
        }
        let mark = (first / STEP) as usize;
        let mut at = self.marks.get(mark).copied().unwrap_or(self.start as u64) as usize;
        let mut row = mark as u64 * STEP;
        while row < self.rows && out.len() < n && at <= self.end {
            let e = row_end(data, at, self.end, self.delimiter);
            if blank(&data[at..e]) {
                at = e + 1;
                continue;
            }
            if row >= first {
                out.push(&data[at..e]);
            }
            row += 1;
            at = e + 1;
        }
        out
    }

    /// Up to `n` rows from row `first`, split into fields (still bytes in the file's
    /// character set).
    pub fn rows<'a>(&self, data: &'a [u8], first: u64, n: usize) -> Vec<Vec<Cow<'a, [u8]>>> {
        self.raw_rows(data, first, n)
            .into_iter()
            .map(|r| fields(r, self.delimiter))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_delimiters() {
        assert_eq!(sniff_delimiter(b"a,b,c\n1,2,3\n4,5,6\n"), b',');
        assert_eq!(sniff_delimiter(b"a;b;c\n1,5;2;3\n4;5,5;6\n"), b';');
        assert_eq!(sniff_delimiter(b"a\tb\n1\t2\n"), b'\t');
        assert_eq!(sniff_delimiter(b"\"x,y\"|b\n1|2\n"), b'|');
        assert_eq!(sniff_delimiter(b"single column\n"), b',');
    }

    #[test]
    fn quoting_and_header() {
        let d =
            b"name,note,n\r\n\"Smith, J\",\"said \"\"hi\"\"\nthen left\",3\r\nplain,5\" screen,4\n";
        let t = Table::build(d, 0, None, &|| false).unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(t.columns, 3);
        assert!(t.header);
        let rows = t.rows(d, 1, 5);
        assert_eq!(rows[0][0].as_ref(), b"Smith, J");
        assert_eq!(rows[0][1].as_ref(), b"said \"hi\"\nthen left");
        assert_eq!(rows[1][1].as_ref(), b"5\" screen");
        assert_eq!(rows[1][2].as_ref(), b"4");
        let t = Table::build(b"1,2\n3,4\n", 0, None, &|| false).unwrap();
        assert!(!t.header);
    }

    /// The `csv` crate as the oracle: every row and field of a generated table with quotes,
    /// doubled quotes, embedded line ends, empty fields and ragged rows.
    #[test]
    fn matches_csv_crate() {
        let mut s = String::new();
        let mut seed = 12345u32;
        let mut rnd = |n: u32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 16) % n
        };
        for r in 0..3000 {
            let cols = 1 + rnd(6);
            for c in 0..cols {
                if c > 0 {
                    s.push(',');
                }
                match rnd(6) {
                    0 => {}
                    1 => s.push_str(&format!("\"q{r},{c}\"")),
                    2 => s.push_str("\"a \"\"b\"\"\nc\""),
                    3 => s.push_str("x\"y"),
                    4 => s.push_str(&"long".repeat(rnd(20) as usize)),
                    _ => s.push_str(&format!("{}", rnd(100_000))),
                }
            }
            s.push_str(if rnd(4) == 0 { "\r\n" } else { "\n" });
        }
        let data = s.as_bytes();
        let t = Table::build(data, 0, Some(b','), &|| false).unwrap();
        let mut rd = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(data);
        let want: Vec<Vec<Vec<u8>>> = rd
            .byte_records()
            .map(|r| r.unwrap().iter().map(<[u8]>::to_vec).collect())
            .collect();
        assert_eq!(t.len(), want.len() as u64);
        let mut mismatches = 0;
        for first in (0..t.len()).step_by(97) {
            for (k, row) in t.rows(data, first, 100).iter().enumerate() {
                let w = &want[first as usize + k];
                let got: Vec<Vec<u8>> = row.iter().map(|f| f.to_vec()).collect();
                if &got != w {
                    mismatches += 1;
                }
            }
        }
        assert_eq!(mismatches, 0);
    }
}
