//! Fast parsing of the whitespace-separated number tables that .ang and .ctf files are: lines
//! split into chunks parsed in parallel, numbers read without going through `str`.

use rayon::prelude::*;

/// Powers of ten exactly representable in f64.
const POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// Parses a decimal number ("-12.5", "3", "1.2e-3", "0,75" with a decimal comma, "NaN").
pub fn number(s: &[u8]) -> Option<f64> {
    let (neg, mut i) = match s.first()? {
        b'-' => (true, 1),
        b'+' => (false, 1),
        _ => (false, 0),
    };
    let mut mant: u64 = 0;
    let mut digits = 0u32;
    let mut exp: i32 = 0;
    let mut any = false;
    while let Some(&c) = s.get(i) {
        if !c.is_ascii_digit() {
            break;
        }
        any = true;
        if digits < 19 {
            mant = mant * 10 + u64::from(c - b'0');
            if mant != 0 {
                digits += 1;
            }
        } else {
            exp += 1;
        }
        i += 1;
    }
    if matches!(s.get(i), Some(b'.' | b',')) {
        i += 1;
        while let Some(&c) = s.get(i) {
            if !c.is_ascii_digit() {
                break;
            }
            any = true;
            if digits < 19 {
                mant = mant * 10 + u64::from(c - b'0');
                if mant != 0 {
                    digits += 1;
                }
                exp -= 1;
            }
            i += 1;
        }
    }
    if !any {
        return slow(s);
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        i += 1;
        let (eneg, j) = match s.get(i) {
            Some(b'-') => (true, i + 1),
            Some(b'+') => (false, i + 1),
            _ => (false, i),
        };
        i = j;
        let mut e: i32 = 0;
        let start = i;
        while let Some(&c) = s.get(i) {
            if !c.is_ascii_digit() {
                break;
            }
            e = e.saturating_mul(10).saturating_add(i32::from(c - b'0'));
            i += 1;
        }
        if i == start {
            return None;
        }
        exp = exp.saturating_add(if eneg { -e } else { e });
    }
    if i != s.len() {
        return None;
    }
    let m = mant as f64;
    let v = if exp == 0 {
        m
    } else if (0..=22).contains(&exp) {
        m * POW10[exp as usize]
    } else if (-22..0).contains(&exp) {
        m / POW10[(-exp) as usize]
    } else {
        m * 10f64.powi(exp)
    };
    Some(if neg { -v } else { v })
}

fn slow(s: &[u8]) -> Option<f64> {
    let t = std::str::from_utf8(s).ok()?;
    match t.to_ascii_lowercase().as_str() {
        "nan" | "-nan" | "+nan" => Some(f64::NAN),
        _ => t.parse().ok(),
    }
}

/// Splits a line into fields at spaces and tabs.
pub fn fields(line: &[u8]) -> impl Iterator<Item = &[u8]> {
    line.split(|&c| c == b' ' || c == b'\t' || c == b'\r')
        .filter(|f| !f.is_empty())
}

/// A table of `cols` numbers per row, row-major.
pub struct Table {
    pub cols: usize,
    pub rows: usize,
    pub data: Vec<f32>,
}

impl Table {
    pub fn get(&self, row: usize, col: usize) -> f32 {
        self.data[row * self.cols + col]
    }
}

/// Most columns a table may have.
pub const MAX_COLUMNS: usize = 4096;

/// Parses the rows of `body` (lines of numbers; blank lines and lines starting with `#` are
/// skipped) into `cols` columns: missing or unparsable fields become NaN, extra ones are
/// dropped. More than [`MAX_COLUMNS`] columns: no rows.
pub fn table(body: &[u8], cols: usize) -> Table {
    if cols > MAX_COLUMNS {
        return Table {
            cols,
            rows: 0,
            data: Vec::new(),
        };
    }
    let chunks = 1.max(body.len() / (1 << 20)).min(256);
    let mut cuts = vec![0usize];
    for k in 1..chunks {
        let at = body.len() * k / chunks;
        let at = body[at..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(body.len(), |p| at + p + 1);
        if at > *cuts.last().unwrap_or(&0) {
            cuts.push(at);
        }
    }
    cuts.push(body.len());
    let parts: Vec<Vec<f32>> = cuts
        .par_windows(2)
        .map(|w| parse_rows(&body[w[0]..w[1]], cols))
        .collect();
    let mut data = Vec::with_capacity(parts.iter().map(Vec::len).sum());
    for p in parts {
        data.extend_from_slice(&p);
    }
    let rows = data.len().checked_div(cols).unwrap_or(0);
    Table { cols, rows, data }
}

fn parse_rows(part: &[u8], cols: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(part.len() / 8);
    // Short rows are padded with NaN, so "1\n" lines in a 4096-column table would make 8000
    // times their size; a full row takes at least two bytes per value. Rows past twice
    // that are dropped.
    let most = part.len().max(cols);
    for line in part.split(|&c| c == b'\n') {
        if out.len() + cols > most {
            break;
        }
        let line = line.trim_ascii();
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        let start = out.len();
        for f in fields(line).take(cols) {
            out.push(number(f).map_or(f32::NAN, |v| v as f32));
        }
        out.resize(start + cols, f32::NAN);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(number(b"12.5"), Some(12.5));
        assert_eq!(number(b"-0.25"), Some(-0.25));
        assert_eq!(number(b"3"), Some(3.0));
        assert_eq!(number(b"1,5"), Some(1.5));
        assert_eq!(number(b"1.5e-3"), Some(1.5e-3));
        assert_eq!(number(b"2E2"), Some(200.0));
        assert_eq!(number(b".5"), Some(0.5));
        assert!(number(b"NaN").unwrap().is_nan());
        assert_eq!(number(b"x"), None);
        assert_eq!(number(b"1.2.3"), None);
        assert_eq!(number(b""), None);
        assert_eq!(
            number(b"12345678901234567890123"),
            Some(1.2345678901234568e22)
        );
        let v = number(b"0.000000000000000000000000001").unwrap();
        assert!((v - 1e-27).abs() < 1e-40);
    }

    #[test]
    fn tables() {
        let t = table(b"# c\n1 2 3\n\n4\t5 6 7\r\n8 x\n", 3);
        assert_eq!(t.rows, 3);
        assert_eq!(&t.data[..6], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(t.get(2, 0), 8.0);
        assert!(t.get(2, 1).is_nan() && t.get(2, 2).is_nan());
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// One-value lines in a 4096-column table used to be padded to 8000 times their size.
    #[test]
    fn padding_is_bounded() {
        let body = b"1\n".repeat(500_000);
        let t = table(&body, MAX_COLUMNS);
        assert!(t.data.len() <= body.len() + MAX_COLUMNS);
        assert_eq!(t.data.len(), t.rows * t.cols);
        assert_eq!(table(b"1 2 3\n", MAX_COLUMNS + 1).rows, 0);
        // Full rows are all kept.
        let t = table(&b"1 2 3\n".repeat(1000), 3);
        assert_eq!(t.rows, 1000);
    }
}
