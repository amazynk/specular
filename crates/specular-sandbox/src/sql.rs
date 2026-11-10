//! SQLite queries in the host ([`crate::protocol::Request::SqlQuery`]): GeoPackage and MBTiles
//! are SQLite databases, and SQLite (C) never parses a file in the viewer.
//!
//! The viewer sends the open database file, the SQL (always the viewer's own, never taken from
//! the file) and its parameters; the host answers with the rows ([`SqlRows`]), encoded in a
//! [`crate::protocol::Payload`]:
//!
//! ```text
//! per value, row by row: u8 kind · body
//!   0 NULL
//!   1 INTEGER  i64
//!   2 REAL     f64 bits
//!   3 TEXT     u32 length · UTF-8
//!   4 BLOB     u32 length · bytes
//! ```
//!
//! Little endian. [`SqlRows::decode`] checks the count and every length, since the bytes come
//! from a process that read a hostile file.

/// A value of an SQL parameter or row.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Eq for SqlValue {}

impl SqlValue {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            SqlValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// An integer or a real (as SQLite's `REAL` columns hold either).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SqlValue::Int(i) => Some(*i as f64),
            SqlValue::Real(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            SqlValue::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Text, or an integer or real written out (`metadata` values of MBTiles are untyped).
    pub fn to_text(&self) -> Option<String> {
        match self {
            SqlValue::Text(s) => Some(s.clone()),
            SqlValue::Int(i) => Some(i.to_string()),
            SqlValue::Real(f) => Some(f.to_string()),
            _ => None,
        }
    }

    /// Bytes of a blob, or of a text (some writers store tiles as text).
    pub fn into_bytes(self) -> Option<Vec<u8>> {
        match self {
            SqlValue::Blob(b) => Some(b),
            SqlValue::Text(s) => Some(s.into_bytes()),
            _ => None,
        }
    }

    /// Bytes this value takes in the row encoding.
    pub fn encoded_len(&self) -> usize {
        match self {
            SqlValue::Null => 1,
            SqlValue::Int(_) | SqlValue::Real(_) => 9,
            SqlValue::Text(s) => 5 + s.len(),
            SqlValue::Blob(b) => 5 + b.len(),
        }
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            SqlValue::Null => out.push(0),
            SqlValue::Int(i) => {
                out.push(1);
                out.extend_from_slice(&i.to_le_bytes());
            }
            SqlValue::Real(f) => {
                out.push(2);
                out.extend_from_slice(&f.to_bits().to_le_bytes());
            }
            SqlValue::Text(s) => {
                out.push(3);
                out.extend_from_slice(&(s.len() as u32).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            }
            SqlValue::Blob(b) => {
                out.push(4);
                out.extend_from_slice(&(b.len() as u32).to_le_bytes());
                out.extend_from_slice(b);
            }
        }
    }

    /// One value from the front of `b`, which advances past it.
    pub fn decode(b: &mut &[u8]) -> Result<SqlValue, String> {
        fn take<'a>(b: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
            if b.len() < n {
                return Err("row data truncated".into());
            }
            let (a, rest) = b.split_at(n);
            *b = rest;
            Ok(a)
        }
        let kind = take(b, 1)?[0];
        Ok(match kind {
            0 => SqlValue::Null,
            1 => SqlValue::Int(i64::from_le_bytes(take(b, 8)?.try_into().unwrap_or([0; 8]))),
            2 => SqlValue::Real(f64::from_bits(u64::from_le_bytes(
                take(b, 8)?.try_into().unwrap_or([0; 8]),
            ))),
            3 | 4 => {
                let n = u32::from_le_bytes(take(b, 4)?.try_into().unwrap_or([0; 4])) as usize;
                let v = take(b, n)?.to_vec();
                if kind == 3 {
                    SqlValue::Text(String::from_utf8(v).map_err(|_| "row text is not UTF-8")?)
                } else {
                    SqlValue::Blob(v)
                }
            }
            k => return Err(format!("bad value kind {k}")),
        })
    }
}

/// What a query may do in the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlLimits {
    /// Rows returned at most; [`SqlRows::truncated`] says when there were more.
    pub max_rows: u32,
    /// Bytes of all rows together (their encoding) at most; past it the query fails.
    pub max_bytes: u64,
    /// The largest value (string or blob) SQLite may make, and the cap on `SQLITE_LIMIT_LENGTH`.
    pub max_value: u32,
    /// Whether the query may read views (MBTiles `tiles` is often a view over two tables).
    /// Triggers and virtual tables are refused always.
    pub allow_views: bool,
    /// Wall-clock time the query may take in the host, in milliseconds.
    pub millis: u32,
}

impl Default for SqlLimits {
    fn default() -> SqlLimits {
        SqlLimits {
            max_rows: 1_000_000,
            max_bytes: 256 << 20,
            max_value: 64 << 20,
            allow_views: false,
            millis: 10_000,
        }
    }
}

/// The rows of a query, row-major.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SqlRows {
    pub columns: usize,
    pub values: Vec<SqlValue>,
    /// The query had more rows than [`SqlLimits::max_rows`].
    pub truncated: bool,
}

impl SqlRows {
    pub fn len(&self) -> usize {
        self.values.len().checked_div(self.columns).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Row `i`.
    pub fn row(&self, i: usize) -> &[SqlValue] {
        &self.values[i * self.columns..(i + 1) * self.columns]
    }

    pub fn rows(&self) -> impl Iterator<Item = &[SqlValue]> {
        self.values.chunks_exact(self.columns.max(1))
    }

    /// The rows taken apart, each a `Vec` of its values.
    pub fn into_rows(self) -> Vec<Vec<SqlValue>> {
        let c = self.columns.max(1);
        let mut out = Vec::with_capacity(self.values.len() / c);
        let mut it = self.values.into_iter();
        loop {
            let row: Vec<SqlValue> = it.by_ref().take(c).collect();
            if row.len() < c {
                break;
            }
            out.push(row);
        }
        out
    }

    /// `rows × columns` values from `data` (exactly; nothing may be left over).
    pub fn decode(
        columns: usize,
        rows: usize,
        truncated: bool,
        mut data: &[u8],
    ) -> Result<SqlRows, String> {
        let n = columns
            .checked_mul(rows)
            .filter(|&n| n <= data.len())
            .ok_or("row count exceeds the data")?;
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(SqlValue::decode(&mut data)?);
        }
        if !data.is_empty() {
            return Err("trailing row data".into());
        }
        Ok(SqlRows {
            columns,
            values,
            truncated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        let vals = vec![
            SqlValue::Null,
            SqlValue::Int(-7),
            SqlValue::Real(f64::INFINITY),
            SqlValue::Text("Grüße".into()),
            SqlValue::Blob(vec![0, 1, 2]),
            SqlValue::Text(String::new()),
        ];
        let mut b = Vec::new();
        for v in &vals {
            v.encode(&mut b);
        }
        assert_eq!(
            b.len(),
            vals.iter().map(SqlValue::encoded_len).sum::<usize>()
        );
        let r = SqlRows::decode(2, 3, false, &b).unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(r.row(1), &vals[2..4]);
        assert_eq!(r.clone().into_rows().len(), 3);
        assert_eq!(r.values, vals);
    }

    #[test]
    fn hostile_rows_are_refused() {
        // More values than bytes.
        assert!(SqlRows::decode(1000, 1000, false, &[0; 10]).is_err());
        assert!(SqlRows::decode(usize::MAX, 2, false, &[0; 10]).is_err());
        // A length past the end.
        let mut b = vec![4];
        b.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(SqlRows::decode(1, 1, false, &b).is_err());
        // Text that is not UTF-8.
        let b = [3, 1, 0, 0, 0, 0xFF];
        assert!(SqlRows::decode(1, 1, false, &b).is_err());
        // Unknown kind, trailing bytes.
        assert!(SqlRows::decode(1, 1, false, &[9]).is_err());
        assert!(SqlRows::decode(1, 1, false, &[0, 0]).is_err());
    }
}
