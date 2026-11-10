//! GeoPackage and MBTiles are SQLite databases, and SQLite never parses a file in the viewer:
//! their readers ([`specular_gis::gpkg`], [`specular_gis::mbtiles`]) query through
//! [`HostDb`], SQLite in the sandboxed decode host ([`specular_sandbox::Host::sql_query`]),
//! which keeps the connection open between queries.

use specular_core::Source;
use specular_gis::sql::{SqlDb, SqlLimits, SqlRows, SqlValue};
use specular_sandbox::{Error as HostError, Host, SqlSource};

use crate::{DecodeError, Result};

/// A database file for the host's SQLite.
pub(crate) struct HostDb {
    src: SqlSource,
    host: &'static Host,
}

impl HostDb {
    /// The source's file as it is on disk, else a private copy of its bytes (a database inside
    /// an archive).
    pub(crate) fn open(source: &Source) -> Result<HostDb> {
        let src = if crate::geo::on_disk(source) {
            SqlSource::open(source.path())
        } else {
            SqlSource::from_bytes(source.bytes())
        }
        .map_err(|e| DecodeError::Unsupported(describe(&e)))?;
        Ok(HostDb {
            src,
            host: Host::global(),
        })
    }
}

/// A host failure in plain language.
fn describe(e: &HostError) -> String {
    match e {
        HostError::NotInstalled(m) | HostError::Unavailable(m) => {
            format!("the database needs the sandboxed decode host, which cannot run: {m}")
        }
        HostError::Remote { message, .. } => message.clone(),
        e => format!("the decode host failed: {e}"),
    }
}

impl SqlDb for HostDb {
    fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
        limits: SqlLimits,
    ) -> std::result::Result<SqlRows, String> {
        self.host
            .sql_query(&self.src, sql, params, limits)
            .map_err(|e| describe(&e))
    }
}
