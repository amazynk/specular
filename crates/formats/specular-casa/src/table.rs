//! A casacore table's description (`table.dat`): the row count, the byte order of the data,
//! the table keywords, and each column's type, shape and storage manager.

use crate::aipsio::Reader;
use crate::record::{Record, dtype, read_record};
use crate::{Error, Result};

/// The largest `table.dat` read (they hold keywords only; CASA's are a few kilobytes).
pub const MAX_TABLE_DAT: u64 = 64 << 20;

/// What `table.dat` describes.
#[derive(Clone, Debug)]
pub struct Table {
    pub rows: u64,
    /// The data of standard storage managers are little-endian (the tiled managers record
    /// their own byte order).
    pub little_endian: bool,
    pub keywords: Record,
    pub columns: Vec<Column>,
    /// Storage managers: type name and sequence number (`table.f<seqnr>`).
    pub managers: Vec<(String, u32)>,
}

#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    /// casacore data type of the elements (see [`dtype`]).
    pub dtype: i32,
    /// An array column, else a scalar one.
    pub array: bool,
    /// Dimensions of the cells (`-1` when they vary).
    pub ndim: i32,
    /// Fixed cell shape when the description gives one.
    pub shape: Vec<i64>,
    pub keywords: Record,
    /// Sequence number of the storage manager holding the column.
    pub manager: u32,
}

/// Whether `head` (the start of a file) is a casacore table description.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 18 && head[..4] == [0xbe; 4] && &head[12..17] == b"Table"
}

impl Table {
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// The type name of the manager with sequence number `seqnr`.
    pub fn manager(&self, seqnr: u32) -> Option<&str> {
        self.managers
            .iter()
            .find(|(_, s)| *s == seqnr)
            .map(|(t, _)| t.as_str())
    }

    pub fn parse(data: &[u8]) -> Result<Table> {
        let mut r = Reader::new(data);
        let (version, outer) = r.open(true, &["Table"])?;
        let rows = if version >= 3 {
            r.u64()?
        } else {
            u64::from(r.u32()?)
        };
        let format = r.u32()?;
        let kind = r.string()?;
        if kind != "PlainTable" {
            return Err(Error::Unsupported(format!("{kind} tables")));
        }
        // The description: name, version, comment, keywords, private keywords, columns.
        let (_, desc_outer) = r.open(false, &["TableDesc"])?;
        r.string()?;
        r.string()?;
        r.string()?;
        let keywords = read_record(&mut r)?;
        let _private = read_record(&mut r)?;
        let _set_version = r.u32()?;
        let ncol = r.count(16)?;
        let mut columns = Vec::with_capacity(ncol);
        for _ in 0..ncol {
            columns.push(read_column_desc(&mut r)?);
        }
        r.close(desc_outer);
        // The column set: row count, storage managers, and which manager holds each column.
        let tag = r.i32()?;
        let set_rows = match tag {
            -2 => u64::from(r.u32()?),
            -3 => r.u64()?,
            _ => {
                return Err(Error::Unsupported(format!("column set version {tag}")));
            }
        };
        if set_rows != rows {
            return Err(Error::Malformed("row counts disagree".into()));
        }
        let _seq_count = r.u32()?;
        let ndm = r.count(8)?;
        let mut managers = Vec::with_capacity(ndm);
        for _ in 0..ndm {
            let t = r.string()?;
            let s = r.u32()?;
            managers.push((t, s));
        }
        for c in &mut columns {
            let _v = r.u32()?;
            let name = r.string()?;
            if name != c.name {
                return Err(Error::Malformed(format!(
                    "column {name:?} where {:?} was described",
                    c.name
                )));
            }
            let _dv = r.u32()?;
            c.manager = r.u32()?;
            if c.array && r.bool()? {
                let shape = r.iposition()?;
                if c.shape.is_empty() {
                    c.shape = shape;
                }
            }
        }
        r.close(outer);
        Ok(Table {
            rows,
            little_endian: format == 1,
            keywords,
            columns,
            managers,
        })
    }
}

fn read_column_desc(r: &mut Reader) -> Result<Column> {
    let class = r.string()?;
    let _v = r.u32()?;
    let name = r.string()?;
    let _comment = r.string()?;
    let _dm_type = r.string()?;
    let _dm_group = r.string()?;
    let dt = r.i32()?;
    let _option = r.i32()?;
    let ndim = r.i32()?;
    let shape = r.iposition()?;
    let _max_length = r.u32()?;
    let keywords = read_record(r)?;
    let array = if class.starts_with("ArrayColumnDesc") {
        let _v = r.u32()?;
        r.bool()?;
        true
    } else if class.starts_with("ScalarColumnDesc") {
        let _v = r.u32()?;
        skip_default(r, dt)?;
        false
    } else {
        return Err(Error::Unsupported(format!(
            "column {name:?} of class {}",
            class.trim()
        )));
    };
    Ok(Column {
        name,
        dtype: dt,
        array,
        ndim,
        shape,
        keywords,
        manager: 0,
    })
}

/// Skips a scalar column's default value.
fn skip_default(r: &mut Reader, dt: i32) -> Result<()> {
    use dtype::*;
    let n = match dt {
        BOOL | CHAR | UCHAR => 1,
        SHORT | USHORT => 2,
        INT | UINT | FLOAT => 4,
        DOUBLE | COMPLEX | INT64 => 8,
        DCOMPLEX => 16,
        STRING => {
            r.string()?;
            0
        }
        t => {
            return Err(Error::Unsupported(format!(
                "scalar column of {}",
                dtype::name(t)
            )));
        }
    };
    r.bytes(n)?;
    Ok(())
}
