//! A read-only SQLite database reader, from the file format documentation: the header, table
//! B-trees (interior and leaf pages), overflow chains and records. Enough to read whole tables
//! of a database held in memory; indexes, WAL files and the freelist are not needed.
//!
//! Bounds: every page is visited at most once per table scan (cycles end the scan), B-trees
//! nest at most [`MAX_DEPTH`] levels, a record is at most [`MAX_RECORD`] bytes and a scan
//! yields at most [`MAX_ROWS`] rows. Damaged databases give errors or fewer rows, never
//! panics.

use std::collections::HashSet;

/// Deepest B-tree.
const MAX_DEPTH: usize = 32;
/// Largest record assembled (a preview PNG is a few megabytes).
pub const MAX_RECORD: usize = 512 << 20;
/// Most rows read from one table.
pub const MAX_ROWS: usize = 1 << 20;

/// A column value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Real(r) if r.is_finite() => Some(*r as i64),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Real(r) => Some(*r),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
    pub fn into_blob(self) -> Option<Vec<u8>> {
        match self {
            Value::Blob(b) => Some(b),
            _ => None,
        }
    }
}

/// A table row: the rowid and the values in column order.
#[derive(Debug, Clone)]
pub struct Row {
    pub rowid: i64,
    pub values: Vec<Value>,
}

/// A table: its root page and column names (from its CREATE TABLE statement).
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub root: u32,
    pub columns: Vec<String>,
    /// The column that is the rowid (INTEGER PRIMARY KEY), stored as NULL in records.
    pub rowid_column: Option<usize>,
}

impl Table {
    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
    }
}

/// An open database.
pub struct Db<'a> {
    data: &'a [u8],
    page_size: usize,
    usable: usize,
    pages: u32,
    utf16: Option<bool>,
    tables: Vec<Table>,
}

fn be16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn be32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// A varint at `o`: (value, length).
fn varint(b: &[u8], o: usize) -> Option<(u64, usize)> {
    let mut v: u64 = 0;
    for i in 0..9 {
        let c = *b.get(o + i)?;
        if i == 8 {
            return Some(((v << 8) | u64::from(c), 9));
        }
        v = (v << 7) | u64::from(c & 0x7f);
        if c & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

impl<'a> Db<'a> {
    /// Opens the database in `data` and reads its schema.
    pub fn open(data: &'a [u8]) -> Result<Db<'a>, String> {
        if !data.starts_with(b"SQLite format 3\0") || data.len() < 100 {
            return Err("not an SQLite database".into());
        }
        let ps = be16(data, 16).ok_or("short header")?;
        let page_size = if ps == 1 { 65536 } else { usize::from(ps) };
        if page_size < 512 || !page_size.is_power_of_two() {
            return Err("bad page size".into());
        }
        let reserved = usize::from(data[20]);
        let usable = page_size.saturating_sub(reserved);
        if usable < 480 {
            return Err("bad reserved space".into());
        }
        let pages = u32::try_from(data.len() / page_size).unwrap_or(u32::MAX);
        let utf16 = match be32(data, 56) {
            Some(2) => Some(true),
            Some(3) => Some(false),
            _ => None,
        };
        let mut db = Db {
            data,
            page_size,
            usable,
            pages,
            utf16,
            tables: Vec::new(),
        };
        let master = Table {
            name: "sqlite_master".into(),
            root: 1,
            columns: ["type", "name", "tbl_name", "rootpage", "sql"]
                .map(String::from)
                .to_vec(),
            rowid_column: None,
        };
        let mut tables = Vec::new();
        db.scan(&master, &mut |row| {
            let v = &row.values;
            if v.first().and_then(Value::as_str) == Some("table")
                && let (Some(name), Some(root)) = (
                    v.get(1).and_then(Value::as_str),
                    v.get(3).and_then(Value::as_i64),
                )
            {
                let sql = v.get(4).and_then(Value::as_str).unwrap_or("");
                let (columns, rowid_column) = columns_of(sql);
                if let Ok(root) = u32::try_from(root) {
                    tables.push(Table {
                        name: name.to_string(),
                        root,
                        columns,
                        rowid_column,
                    });
                }
            }
            true
        })?;
        db.tables = tables;
        Ok(db)
    }

    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    fn page(&self, n: u32) -> Option<&'a [u8]> {
        if n == 0 || n > self.pages {
            return None;
        }
        let start = (n as usize - 1) * self.page_size;
        self.data.get(start..start + self.page_size)
    }

    /// Calls `f` with every row of `t`, in rowid order, until it returns `false`.
    pub fn scan(&self, t: &Table, f: &mut dyn FnMut(Row) -> bool) -> Result<(), String> {
        let mut seen = HashSet::new();
        let mut rows = 0usize;
        self.walk(t, t.root, 0, &mut seen, &mut rows, f).map(|_| ())
    }

    /// Every row of `t` (up to [`MAX_ROWS`]).
    pub fn rows(&self, t: &Table) -> Result<Vec<Row>, String> {
        let mut out = Vec::new();
        self.scan(t, &mut |r| {
            out.push(r);
            true
        })?;
        Ok(out)
    }

    /// Walks the subtree at `page`; `Ok(false)` when `f` asked to stop.
    fn walk(
        &self,
        t: &Table,
        page: u32,
        depth: usize,
        seen: &mut HashSet<u32>,
        rows: &mut usize,
        f: &mut dyn FnMut(Row) -> bool,
    ) -> Result<bool, String> {
        if depth > MAX_DEPTH || !seen.insert(page) {
            return Err("damaged database (B-tree loop)".into());
        }
        let p = self
            .page(page)
            .ok_or("damaged database (page out of range)")?;
        let h = if page == 1 { 100 } else { 0 };
        let kind = *p.get(h).ok_or("short page")?;
        let cells = usize::from(be16(p, h + 3).ok_or("short page")?);
        match kind {
            0x05 => {
                let ptrs = h + 12;
                for i in 0..cells {
                    let c = usize::from(be16(p, ptrs + i * 2).ok_or("short page")?);
                    let child = be32(p, c).ok_or("short cell")?;
                    if !self.walk(t, child, depth + 1, seen, rows, f)? {
                        return Ok(false);
                    }
                }
                let right = be32(p, h + 8).ok_or("short page")?;
                self.walk(t, right, depth + 1, seen, rows, f)
            }
            0x0d => {
                let ptrs = h + 8;
                for i in 0..cells {
                    let c = usize::from(be16(p, ptrs + i * 2).ok_or("short page")?);
                    let Some(row) = self.leaf_cell(t, p, c, seen) else {
                        continue;
                    };
                    *rows += 1;
                    if *rows > MAX_ROWS || !f(row) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Err(format!("damaged database (page type {kind:#x})")),
        }
    }

    fn leaf_cell(&self, t: &Table, p: &[u8], c: usize, seen: &mut HashSet<u32>) -> Option<Row> {
        let (size, n1) = varint(p, c)?;
        let (rowid, n2) = varint(p, c + n1)?;
        let size = usize::try_from(size).ok()?;
        if size > MAX_RECORD {
            return None;
        }
        let at = c + n1 + n2;
        let u = self.usable;
        let x = u - 35;
        let local = if size <= x {
            size
        } else {
            let m = ((u - 12) * 32 / 255) - 23;
            let k = m + (size - m) % (u - 4);
            if k <= x { k } else { m }
        };
        let mut payload = Vec::with_capacity(size.min(1 << 20));
        payload.extend_from_slice(p.get(at..at + local)?);
        if local < size {
            let mut next = be32(p, at + local)?;
            while payload.len() < size {
                if !seen.insert(next) {
                    return None;
                }
                let op = self.page(next)?;
                next = be32(op, 0)?;
                let take = (size - payload.len()).min(u - 4);
                payload.extend_from_slice(op.get(4..4 + take)?);
            }
        }
        let mut values = self.record(&payload)?;
        if let Some(i) = t.rowid_column
            && let Some(v) = values.get_mut(i)
            && *v == Value::Null
        {
            *v = Value::Int(rowid as i64);
        }
        Some(Row {
            rowid: rowid as i64,
            values,
        })
    }

    fn record(&self, r: &[u8]) -> Option<Vec<Value>> {
        let (hlen, n) = varint(r, 0)?;
        let hlen = usize::try_from(hlen).ok()?;
        if hlen > r.len() {
            return None;
        }
        let mut types = Vec::new();
        let mut o = n;
        while o < hlen {
            let (t, k) = varint(r, o)?;
            types.push(t);
            o += k;
        }
        let mut d = hlen;
        let mut out = Vec::with_capacity(types.len());
        for t in types {
            let int = |len: usize, d: usize| -> Option<i64> {
                let b = r.get(d..d + len)?;
                let mut v: i64 = if b[0] & 0x80 != 0 { -1 } else { 0 };
                for &x in b {
                    v = (v << 8) | i64::from(x);
                }
                Some(v)
            };
            let (v, len) = match t {
                0 => (Value::Null, 0),
                1 => (Value::Int(int(1, d)?), 1),
                2 => (Value::Int(int(2, d)?), 2),
                3 => (Value::Int(int(3, d)?), 3),
                4 => (Value::Int(int(4, d)?), 4),
                5 => (Value::Int(int(6, d)?), 6),
                6 => (Value::Int(int(8, d)?), 8),
                7 => {
                    let b: [u8; 8] = r.get(d..d + 8)?.try_into().ok()?;
                    (Value::Real(f64::from_be_bytes(b)), 8)
                }
                8 => (Value::Int(0), 0),
                9 => (Value::Int(1), 0),
                10 | 11 => return None,
                t => {
                    let len = usize::try_from((t - 12) / 2).ok()?;
                    let b = r.get(d..d.checked_add(len)?)?;
                    if t % 2 == 0 {
                        (Value::Blob(b.to_vec()), len)
                    } else {
                        (Value::Text(self.text(b)), len)
                    }
                }
            };
            out.push(v);
            d += len;
        }
        Some(out)
    }

    fn text(&self, b: &[u8]) -> String {
        match self.utf16 {
            None => String::from_utf8_lossy(b).into_owned(),
            Some(le) => {
                let u: Vec<u16> = b
                    .chunks_exact(2)
                    .map(|c| {
                        if le {
                            u16::from_le_bytes([c[0], c[1]])
                        } else {
                            u16::from_be_bytes([c[0], c[1]])
                        }
                    })
                    .collect();
                String::from_utf16_lossy(&u)
            }
        }
    }
}

/// Column names of a `CREATE TABLE name(col type …, …)` statement, and which one is an
/// INTEGER PRIMARY KEY (the rowid).
fn columns_of(sql: &str) -> (Vec<String>, Option<usize>) {
    let Some(open) = sql.find('(') else {
        return (Vec::new(), None);
    };
    let body = &sql[open + 1..];
    let body = body.rfind(')').map_or(body, |c| &body[..c]);
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    let mut quote: Option<char> = None;
    for (i, ch) in body.char_indices() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'' | '`') => quote = Some(ch),
            (None, '[') => quote = Some(']'),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, ',') if depth == 0 => {
                parts.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    let mut cols = Vec::new();
    let mut rowid = None;
    for p in parts {
        let p = p.trim();
        // A quoted name may hold spaces.
        let first = match p.chars().next() {
            Some(q @ ('"' | '`' | '[')) => {
                let close = if q == '[' { ']' } else { q };
                p[1..].find(close).map_or(p, |e| &p[..e + 2])
            }
            _ => p.split_whitespace().next().unwrap_or(""),
        };
        let upper = first.to_ascii_uppercase();
        if p.is_empty()
            || matches!(
                upper.as_str(),
                "PRIMARY" | "UNIQUE" | "CHECK" | "FOREIGN" | "CONSTRAINT"
            )
        {
            continue;
        }
        let name = first.trim_matches(['"', '\'', '`', '[', ']']).to_string();
        let rest = p[first.len()..].to_ascii_uppercase();
        let rest: Vec<&str> = rest.split_whitespace().collect();
        if rest.first() == Some(&"INTEGER") && rest.windows(2).any(|w| w == ["PRIMARY", "KEY"]) {
            rowid = Some(cols.len());
        }
        cols.push(name);
    }
    (cols, rowid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_table_columns() {
        let (c, r) = columns_of(
            "CREATE TABLE T(_PW_ID INTEGER PRIMARY KEY AUTOINCREMENT, \"A b\" TEXT DEFAULT (1,2), \
             C BLOB, PRIMARY KEY(C))",
        );
        assert_eq!(c, ["_PW_ID", "A b", "C"]);
        assert_eq!(r, Some(0));
        assert_eq!(columns_of("CREATE TABLE s(name,seq)").0, ["name", "seq"]);
    }

    #[test]
    fn varints() {
        assert_eq!(varint(&[0x05], 0), Some((5, 1)));
        assert_eq!(varint(&[0x81, 0x00], 0), Some((128, 2)));
        assert_eq!(varint(&[0xff; 9], 0), Some((u64::MAX, 9)));
        assert_eq!(varint(&[0x81], 0), None);
    }
}
