//! The SQLite databases GeoPackage and MBTiles are made of are never parsed here: their
//! readers ([`crate::gpkg`], [`crate::mbtiles`]) send their own SQL through an [`SqlDb`], which
//! the viewer implements with SQLite in the sandboxed decode host
//! (`specular_sandbox::Host::sql_query`). Tests use SQLite in process, as an oracle.

pub use specular_sandbox::sql::{SqlLimits, SqlRows, SqlValue};

/// A read-only SQLite database the readers query.
pub trait SqlDb: Send + Sync {
    /// Runs `sql` with `params` inside `limits`; the error is a plain-language reason.
    fn query(&self, sql: &str, params: &[SqlValue], limits: SqlLimits) -> Result<SqlRows, String>;
}

/// Limits for small catalogue queries (contents, columns, reference systems, metadata).
pub(crate) fn small() -> SqlLimits {
    SqlLimits {
        max_rows: 100_000,
        max_bytes: 64 << 20,
        max_value: 16 << 20,
        allow_views: false,
        millis: 5_000,
    }
}

/// `sql` with `params`, every row's values.
pub(crate) fn rows(
    db: &dyn SqlDb,
    sql: &str,
    params: &[SqlValue],
    limits: SqlLimits,
) -> Result<Vec<Vec<SqlValue>>, String> {
    db.query(sql, params, limits).map(SqlRows::into_rows)
}

/// The first row of `sql`, if any.
pub(crate) fn row(
    db: &dyn SqlDb,
    sql: &str,
    params: &[SqlValue],
    limits: SqlLimits,
) -> Result<Option<Vec<SqlValue>>, String> {
    let limits = SqlLimits {
        max_rows: 1,
        ..limits
    };
    Ok(rows(db, sql, params, limits)?.into_iter().next())
}

pub(crate) fn text(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) => Some(s.clone()),
        _ => None,
    }
}

/// SQLite's in-process reader for tests (the oracle; never used by the viewer).
#[cfg(test)]
pub(crate) mod local {
    use super::*;
    use rusqlite::{Connection, OpenFlags, types::ValueRef};
    use std::path::Path;
    use std::sync::Mutex;

    pub struct LocalDb(Mutex<Connection>);

    impl LocalDb {
        pub fn open(path: &Path) -> LocalDb {
            let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            LocalDb(Mutex::new(c))
        }
    }

    impl SqlDb for LocalDb {
        fn query(
            &self,
            sql: &str,
            params: &[SqlValue],
            limits: SqlLimits,
        ) -> Result<SqlRows, String> {
            let c = self.0.lock().unwrap();
            let mut st = c.prepare(sql).map_err(|e| e.to_string())?;
            for (i, p) in params.iter().enumerate() {
                let r = match p {
                    SqlValue::Null => st.raw_bind_parameter(i + 1, rusqlite::types::Null),
                    SqlValue::Int(v) => st.raw_bind_parameter(i + 1, v),
                    SqlValue::Real(v) => st.raw_bind_parameter(i + 1, v),
                    SqlValue::Text(v) => st.raw_bind_parameter(i + 1, v),
                    SqlValue::Blob(v) => st.raw_bind_parameter(i + 1, v),
                };
                r.map_err(|e| e.to_string())?;
            }
            let columns = st.column_count();
            let mut it = st.raw_query();
            let mut out = SqlRows {
                columns,
                ..SqlRows::default()
            };
            while let Some(r) = it.next().map_err(|e| e.to_string())? {
                if out.len() >= limits.max_rows as usize {
                    out.truncated = true;
                    break;
                }
                for i in 0..columns {
                    out.values
                        .push(match r.get_ref(i).map_err(|e| e.to_string())? {
                            ValueRef::Null => SqlValue::Null,
                            ValueRef::Integer(v) => SqlValue::Int(v),
                            ValueRef::Real(v) => SqlValue::Real(v),
                            ValueRef::Text(t) => SqlValue::Text(String::from_utf8_lossy(t).into()),
                            ValueRef::Blob(b) => SqlValue::Blob(b.to_vec()),
                        });
                }
            }
            Ok(out)
        }
    }
}
