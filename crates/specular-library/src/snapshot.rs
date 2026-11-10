//! A read-only view of the catalogue for another process (the desktop search provider): no
//! indexer, no migrations, nothing written. It searches the catalogue as the running viewer
//! left it, with the same query language and results as [`crate::Library::search`].

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags};

use crate::search::{self, Found, Row, Search, SideCache};
use crate::{Error, Fold, schema};

/// The catalogue opened read-only.
pub struct Snapshot {
    conn: Mutex<Connection>,
    side: SideCache,
    fold: Fold,
    fold_id: &'static str,
}

impl Snapshot {
    /// Opens the catalogue at `path` read-only. Fails when there is none (it is never made
    /// here) or it is older than this build (it would need a migration this view cannot run).
    /// `fold` and `fold_id` are the viewer's ([`crate::Config::fold`]), so the full-text
    /// index it built is used.
    pub fn open(path: &Path, fold: Fold, fold_id: &'static str) -> Result<Snapshot, Error> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(Duration::from_secs(1))?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        schema::configure_reader(&conn)?;
        let found: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if found < schema::VERSION {
            return Err(Error::Io(std::io::Error::other(format!(
                "catalogue schema {found} is older than {}: open Specular once to update it",
                schema::VERSION
            ))));
        }
        Ok(Snapshot {
            conn: Mutex::new(conn),
            side: SideCache::default(),
            fold,
            fold_id,
        })
    }

    /// Runs `req`, handing each row to `emit` in order (see [`search::run_each`]). After
    /// `budget` the query is interrupted: what was found by then is kept and `Ok` returned.
    pub fn search_each(
        &self,
        req: &Search,
        budget: Duration,
        emit: &mut dyn FnMut(Row),
    ) -> Result<Found, Error> {
        let conn = self.conn.lock();
        let handle = conn.get_interrupt_handle();
        let (done, wait) = mpsc::channel::<()>();
        let timer = specular_thread::builder("lib-budget").spawn(move || {
            if wait.recv_timeout(budget) == Err(mpsc::RecvTimeoutError::Timeout) {
                handle.interrupt();
            }
        });
        let mut n = 0usize;
        let r = search::run_each_cached(
            &conn,
            req,
            self.fold,
            self.fold_id,
            Some(&self.side),
            &mut |row| {
                n += 1;
                emit(row);
            },
        );
        let _ = done.send(());
        if let Ok(t) = timer {
            let _ = t.join();
        }
        match r {
            Err(Error::Sql(rusqlite::Error::SqliteFailure(e, _)))
                if e.code == rusqlite::ErrorCode::OperationInterrupted =>
            {
                specular_log::debug!(target: specular_log::FOLDER, "library: search cut off after {budget:?} ({n} rows)");
                Ok(Found::default())
            }
            r => r,
        }
    }
}
