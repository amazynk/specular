//! SQLite in the host ([`specular_sandbox::protocol::Request::SqlQuery`]): GeoPackage and
//! MBTiles databases are hostile files, and SQLite is a large C parser.
//!
//! * **Files.** The host cannot open files, so SQLite reads through a VFS of our own
//!   ("specular-fd", registered as the default before the sandbox): the main database is the
//!   file the viewer passed, read with positioned reads only, immutable (no journal, no locks,
//!   nothing written); temporary files SQLite wants for sorting live in memory, capped.
//! * **WAL.** A database whose latest transactions are still in its `-wal` file (the program
//!   that wrote it has it open, or did not checkpoint) is read as of its last commit: when a
//!   connection opens, the host asks the viewer for the WAL ([`Response::NeedVolume`] "wal",
//!   as for RAR volumes), checks its frames (salts and the running checksum, as SQLite does)
//!   and lays the committed ones over the main file in the VFS. Neither file is written; no
//!   `-shm` index is needed.
//! * **Views** are expanded by SQLite before any authorizer call, and a view may name another
//!   view many times. Before a query that allows views runs, the SQL it would expand to is
//!   measured from the views' text (every mention of a view counted, a cycle counted as
//!   endless) and refused past [`VIEW_SQL_MAX`]; the authorizer also stops a statement after
//!   [`AUTH_MAX`] checks. Views stay read-only like everything else (the connection cannot
//!   write).
//! * **Hardening** of every connection: read-only and `query_only`; `sqlite3_limit` on value,
//!   SQL and expression sizes, columns, compound selects, VDBE ops, functions arguments,
//!   attached databases, variables and trigger depth; `trusted_schema` off, defensive mode,
//!   triggers off, views only when the query allows them; a page cache of 16 MiB, temporary
//!   storage in memory; `cell_size_check` on; a hard heap limit for SQLite as a whole.
//! * **Authorizer** on every statement: reads and an allowlist of plain functions only; no
//!   recursive CTEs, no virtual tables, no access through views unless allowed, no pragmas
//!   but `table_info`, nothing that writes or attaches.
//! * **Work.** A progress handler stops a query at its deadline ([`SqlLimits::millis`]);
//!   rows and bytes are counted against [`SqlLimits`] as they are read.
//!
//! A few connections stay open between requests, by the viewer's key (as CZI documents do).
//!
//! [`Response::NeedVolume`]: specular_sandbox::protocol::Response::NeedVolume

use crate::heif::Failure;
use rusqlite::ffi;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, config::DbConfig};
use specular_sandbox::protocol::ErrorKind;
use specular_sandbox::sql::{SqlLimits, SqlValue};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Name of our VFS.
const VFS: &CStr = c"specular-fd";
/// Connections kept open between requests.
const OPEN_MAX: usize = 4;
/// SQLite's page cache per connection (KiB, as `PRAGMA cache_size` takes it negative).
const CACHE_KIB: i64 = 16 << 10;
/// A temporary file (sorting, `ORDER BY` without an index) at most.
const TEMP_MAX: usize = 256 << 20;
/// The viewer's SQL is short.
const SQL_MAX: i32 = 64 << 10;
/// SQL text a query may expand to through views (its own text, plus each view's text for
/// every time it is named, recursively).
pub const VIEW_SQL_MAX: u64 = 1 << 20;
/// Views a database may have for any of them to be read; beyond, or past
/// [`VIEW_TEXT_MAX`] bytes of view definitions, views are not read.
const VIEWS_MAX: usize = 1000;
const VIEW_TEXT_MAX: usize = 4 << 20;
/// Authorizer checks one statement may make.
pub const AUTH_MAX: usize = 200_000;
/// WAL frames read at most (a page each).
const WAL_FRAMES_MAX: u64 = 4 << 20;

/// Functions a query, or a view it reads, may call.
const FUNCTIONS: &[&str] = &[
    "count",
    "min",
    "max",
    "sum",
    "total",
    "avg",
    "length",
    "typeof",
    "coalesce",
    "ifnull",
    "nullif",
    "abs",
    "lower",
    "upper",
    "substr",
    "substring",
    "trim",
    "ltrim",
    "rtrim",
    "round",
    "instr",
    "iif",
];

thread_local! {
    /// The file the next connection opens as its main database (set right before opening),
    /// with the committed frames of its WAL.
    static PENDING: RefCell<Option<(File, Option<Wal>)>> = const { RefCell::new(None) };
}

/// Registers the VFS and caps SQLite's heap (before the sandbox: nothing is loaded later).
pub fn init(memory_limit: u64) -> Result<(), String> {
    let vfs = Box::leak(Box::new(ffi::sqlite3_vfs {
        iVersion: 2,
        szOsFile: std::mem::size_of::<VfsFile>() as c_int,
        mxPathname: 512,
        pNext: std::ptr::null_mut(),
        zName: VFS.as_ptr(),
        pAppData: std::ptr::null_mut(),
        xOpen: Some(x_open),
        xDelete: Some(x_delete),
        xAccess: Some(x_access),
        xFullPathname: Some(x_full_pathname),
        xDlOpen: None,
        xDlError: None,
        xDlSym: None,
        xDlClose: None,
        xRandomness: Some(x_randomness),
        xSleep: Some(x_sleep),
        xCurrentTime: Some(x_current_time),
        xGetLastError: Some(x_get_last_error),
        xCurrentTimeInt64: Some(x_current_time_i64),
        xSetSystemCall: None,
        xGetSystemCall: None,
        xNextSystemCall: None,
    }));
    #[allow(unsafe_code)]
    // SAFETY: `vfs` is leaked, so it lives (unmoved) as long as the process, as SQLite requires;
    // its callbacks match the declared signatures; registering initialises SQLite first.
    let rc = unsafe { ffi::sqlite3_vfs_register(vfs, 1) };
    if rc != ffi::SQLITE_OK {
        return Err(format!("cannot register the SQLite VFS ({rc})"));
    }
    // SQLite as a whole may use half the host's memory; past it allocations fail (SQLITE_NOMEM).
    let heap = i64::try_from(memory_limit / 2).unwrap_or(i64::MAX);
    #[allow(unsafe_code)]
    // SAFETY: a plain setter, callable at any time after initialisation (done above).
    unsafe {
        ffi::sqlite3_hard_heap_limit64(heap);
    }
    Ok(())
}

/// Open connections by key, most recent last.
#[derive(Default)]
pub struct Sqlite {
    open: Vec<(u64, Db)>,
}

struct Db {
    conn: Connection,
    /// Whether the running query may read through views (read by the authorizer).
    views: Arc<AtomicBool>,
    /// Authorizer checks of the running statement.
    checks: Arc<AtomicUsize>,
    /// What each view expands to.
    view_cost: ViewCost,
}

/// One query's answer: columns, rows, whether there were more, the encoded rows.
pub type Rows = (u32, u32, bool, Vec<u8>);

impl Sqlite {
    /// Runs `sql` on database `key` (`file`); `wal` asks the viewer for the database's WAL
    /// when the connection is opened.
    pub fn query(
        &mut self,
        key: u64,
        file: File,
        sql: &str,
        params: &[SqlValue],
        limits: &SqlLimits,
        wal: &mut dyn FnMut() -> Option<File>,
    ) -> Result<Rows, Failure> {
        let db = self.db(key, file, wal)?;
        db.run(sql, params, limits)
    }

    fn db(
        &mut self,
        key: u64,
        file: File,
        wal: &mut dyn FnMut() -> Option<File>,
    ) -> Result<&mut Db, Failure> {
        if let Some(i) = self.open.iter().position(|(k, _)| *k == key) {
            // Already open: the new descriptor is the same file; it closes here.
            drop(file);
            let e = self.open.remove(i);
            self.open.push(e);
        } else {
            let wal = match wal() {
                Some(w) => Wal::open(&file, w)?,
                None => None,
            };
            let db = Db::open(file, wal)?;
            if self.open.len() >= OPEN_MAX {
                self.open.remove(0);
            }
            self.open.push((key, db));
        }
        Ok(&mut self.open.last_mut().expect("just pushed").1)
    }
}

fn sql_err(e: rusqlite::Error) -> Failure {
    use rusqlite::ErrorCode as C;
    let msg = e.to_string();
    // The authorizer's and the view switch's refusals arrive as plain SQLITE_ERROR.
    if msg.contains("not authorized") || msg.contains("prohibited") {
        return (
            ErrorKind::Unsupported,
            format!("the database needs a feature that is not read for safety ({msg})"),
        );
    }
    let kind = match e.sqlite_error_code() {
        Some(C::DatabaseCorrupt | C::NotADatabase | C::CannotOpen) => ErrorKind::Malformed,
        Some(C::TooBig | C::OutOfMemory | C::DiskFull) => ErrorKind::TooLarge,
        Some(C::OperationInterrupted) => {
            return (
                ErrorKind::TooLarge,
                "the database query took too long".into(),
            );
        }
        Some(C::AuthorizationForStatementDenied) => {
            return (
                ErrorKind::Unsupported,
                format!("the database needs a feature that is not read for safety ({e})"),
            );
        }
        Some(C::SystemIoFailure) => ErrorKind::Io,
        _ => ErrorKind::Malformed,
    };
    (kind, e.to_string())
}

impl Db {
    fn open(file: File, wal: Option<Wal>) -> Result<Db, Failure> {
        PENDING.with(|p| *p.borrow_mut() = Some((file, wal)));
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags_and_vfs("specular-db", flags, VFS);
        // Taken by `x_open`; drop it if the open failed before.
        PENDING.with(|p| p.borrow_mut().take());
        let mut conn = conn.map_err(sql_err)?;
        harden(&conn).map_err(sql_err)?;
        let views = Arc::new(AtomicBool::new(false));
        let checks = Arc::new(AtomicUsize::new(0));
        // Only the file's ordinary tables (and views) are read: never its virtual tables
        // (GeoPackage R-trees, anything else) nor SQLite's built-in ones (dbstat, json_each...).
        let tables = plain_tables(&conn).map_err(sql_err)?;
        let view_cost = ViewCost::of(&conn).map_err(sql_err)?;
        let allow = views.clone();
        let count = checks.clone();
        conn.authorizer(Some(move |ctx: AuthContext<'_>| {
            if count.fetch_add(1, Ordering::Relaxed) >= AUTH_MAX {
                return Authorization::Deny;
            }
            authorize(&ctx, &tables, allow.load(Ordering::Relaxed))
        }))
        .map_err(sql_err)?;
        Ok(Db {
            conn,
            views,
            checks,
            view_cost,
        })
    }

    fn run(&mut self, sql: &str, params: &[SqlValue], limits: &SqlLimits) -> Result<Rows, Failure> {
        let c = &mut self.conn;
        let max_value = i32::try_from(limits.max_value)
            .unwrap_or(i32::MAX)
            .clamp(1024, 1 << 30);
        c.set_limit(Limit::SQLITE_LIMIT_LENGTH, max_value)
            .map_err(sql_err)?;
        if limits.allow_views {
            self.view_cost.check(sql)?;
        }
        c.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, limits.allow_views)
            .map_err(sql_err)?;
        self.views.store(limits.allow_views, Ordering::Relaxed);
        self.checks.store(0, Ordering::Relaxed);
        let deadline = Instant::now() + Duration::from_millis(u64::from(limits.millis.max(1)));
        c.progress_handler(1000, Some(move || Instant::now() > deadline))
            .map_err(sql_err)?;
        let out = rows(c, sql, params, limits);
        c.progress_handler(0, None::<fn() -> bool>)
            .map_err(sql_err)?;
        out
    }
}

fn rows(
    c: &Connection,
    sql: &str,
    params: &[SqlValue],
    limits: &SqlLimits,
) -> Result<Rows, Failure> {
    let mut st = c.prepare(sql).map_err(sql_err)?;
    let columns = st.column_count();
    if params.len() != st.parameter_count() {
        return Err((ErrorKind::Protocol, "wrong number of SQL parameters".into()));
    }
    for (i, p) in params.iter().enumerate() {
        let r = match p {
            SqlValue::Null => st.raw_bind_parameter(i + 1, rusqlite::types::Null),
            SqlValue::Int(v) => st.raw_bind_parameter(i + 1, v),
            SqlValue::Real(v) => st.raw_bind_parameter(i + 1, v),
            SqlValue::Text(v) => st.raw_bind_parameter(i + 1, v),
            SqlValue::Blob(v) => st.raw_bind_parameter(i + 1, v),
        };
        r.map_err(sql_err)?;
    }
    let mut it = st.raw_query();
    let mut out = Vec::new();
    let mut n = 0u32;
    let mut truncated = false;
    while let Some(row) = it.next().map_err(sql_err)? {
        if n >= limits.max_rows {
            truncated = true;
            break;
        }
        for i in 0..columns {
            let v = match row.get_ref(i).map_err(sql_err)? {
                ValueRef::Null => SqlValue::Null,
                ValueRef::Integer(v) => SqlValue::Int(v),
                ValueRef::Real(v) => SqlValue::Real(v),
                ValueRef::Text(t) => SqlValue::Text(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Blob(b) => SqlValue::Blob(b.to_vec()),
            };
            if (out.len() + v.encoded_len()) as u64 > limits.max_bytes {
                return Err((
                    ErrorKind::TooLarge,
                    format!("the query's rows exceed {} MiB", limits.max_bytes >> 20),
                ));
            }
            out.try_reserve(v.encoded_len())
                .map_err(|_| (ErrorKind::TooLarge, "out of memory".to_string()))?;
            v.encode(&mut out);
        }
        n += 1;
    }
    Ok((columns as u32, n, truncated, out))
}

/// Settings of every connection, before its authorizer.
fn harden(c: &Connection) -> rusqlite::Result<()> {
    for (l, v) in [
        (Limit::SQLITE_LIMIT_LENGTH, 64 << 20),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, SQL_MAX),
        (Limit::SQLITE_LIMIT_COLUMN, 2000),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 100),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 20),
        (Limit::SQLITE_LIMIT_VDBE_OP, 25_000),
        (Limit::SQLITE_LIMIT_FUNCTION_ARG, 8),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 100),
        (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 32),
        (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 1),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        c.set_limit(l, v)?;
    }
    for (cfg, on) in [
        (DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true),
        (DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, false),
    ] {
        c.set_db_config(cfg, on)?;
    }
    c.execute_batch(&format!(
        "PRAGMA query_only = ON; PRAGMA trusted_schema = OFF; PRAGMA temp_store = MEMORY; \
         PRAGMA cell_size_check = ON; PRAGMA mmap_size = 0; PRAGMA cache_size = -{CACHE_KIB};"
    ))
}

/// The names of the database's ordinary tables and views, and its schema table (lower case).
fn plain_tables(c: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut st = c.prepare(
        "SELECT name FROM sqlite_master WHERE type IN ('table', 'view') \
         AND lower(substr(ltrim(sql), 1, 20)) != 'create virtual table' LIMIT 100000",
    )?;
    let names = st.query_map([], |r| r.get::<_, String>(0))?;
    let mut set: HashSet<String> = names
        .map(|n| n.map(|n| n.to_lowercase()))
        .collect::<rusqlite::Result<_>>()?;
    set.extend(["sqlite_master".to_string(), "sqlite_schema".to_string()]);
    Ok(set)
}

/// Identifier-like words of SQL text, lower case (ASCII): runs of letters, digits, `_`, `$`
/// and non-ASCII bytes, wherever they are (inside quotes and comments too). Any way SQL can
/// name a table whose name is such a word contains it as one of these runs.
fn words(sql: &str) -> impl Iterator<Item = String> + '_ {
    sql.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$' || !c.is_ascii()))
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
}

/// What naming each view costs: the SQL text it expands to, through the views it names.
struct ViewCost {
    /// By lower-case name; `u64::MAX` for a cycle.
    bytes: HashMap<String, u64>,
    /// Views are not read at all: too many, too much text, or a name [`words`] cannot find.
    refused: Option<&'static str>,
}

impl ViewCost {
    fn of(c: &Connection) -> rusqlite::Result<ViewCost> {
        let mut st =
            c.prepare("SELECT name, sql FROM sqlite_master WHERE type = 'view' LIMIT ?1")?;
        let mut views: Vec<(String, String)> = Vec::new();
        let mut text = 0usize;
        let mut refused = None;
        let mut rows = st.query([VIEWS_MAX as i64 + 1])?;
        while let Some(r) = rows.next()? {
            let name: String = r.get(0)?;
            let sql: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            text += sql.len();
            if views.len() >= VIEWS_MAX || text > VIEW_TEXT_MAX {
                refused = Some("the database has too many views to read safely");
                break;
            }
            let name = name.to_ascii_lowercase();
            if words(&name).count() != 1 || words(&name).next().as_deref() != Some(&*name) {
                refused = Some("a view's name cannot be checked safely");
            }
            views.push((name, sql));
        }
        let index: HashMap<&str, usize> = views
            .iter()
            .enumerate()
            .map(|(i, (n, _))| (n.as_str(), i))
            .collect();
        // Each view's mentions of other views (by index, with counts), from one pass over its
        // text. Its own name is in its `CREATE VIEW` text; SQLite refuses a view that names
        // itself, so that mention costs nothing.
        let mentions: Vec<Vec<(usize, u64)>> = views
            .iter()
            .enumerate()
            .map(|(i, (_, sql))| {
                let mut m: HashMap<usize, u64> = HashMap::new();
                for w in words(sql) {
                    if let Some(&j) = index.get(w.as_str())
                        && j != i
                    {
                        *m.entry(j).or_default() += 1;
                    }
                }
                m.into_iter().collect()
            })
            .collect();
        // Expanded size, depth first; a view met again while expanding is a cycle.
        #[derive(Clone, Copy, PartialEq)]
        enum State {
            New,
            Open,
            Done(u64),
        }
        let mut state = vec![State::New; views.len()];
        for root in 0..views.len() {
            let mut stack = vec![(root, 0usize)];
            while let Some(&mut (v, ref mut next)) = stack.last_mut() {
                if *next == 0 && state[v] == State::New {
                    state[v] = State::Open;
                }
                if let Some(&(w, _)) = mentions[v].get(*next) {
                    *next += 1;
                    match state[w] {
                        State::New => stack.push((w, 0)),
                        State::Open => state[w] = State::Done(u64::MAX),
                        State::Done(_) => {}
                    }
                    continue;
                }
                stack.pop();
                if state[v] == State::Open {
                    let mut b = views[v].1.len() as u64;
                    for &(w, k) in &mentions[v] {
                        let wb = match state[w] {
                            State::Done(x) => x,
                            // Still open: v is part of a cycle through w.
                            _ => u64::MAX,
                        };
                        b = b.saturating_add(wb.saturating_mul(k));
                    }
                    state[v] = State::Done(b);
                }
            }
        }
        let bytes = views
            .into_iter()
            .zip(state)
            .map(|((n, _), s)| {
                (
                    n,
                    match s {
                        State::Done(b) => b,
                        _ => u64::MAX,
                    },
                )
            })
            .collect();
        Ok(ViewCost { bytes, refused })
    }

    /// The SQL text `sql` expands to through the views it names, at most [`VIEW_SQL_MAX`].
    fn check(&self, sql: &str) -> Result<(), Failure> {
        if self.bytes.is_empty() {
            return Ok(());
        }
        if let Some(why) = self.refused {
            return Err((ErrorKind::Unsupported, why.into()));
        }
        let mut total = sql.len() as u64;
        for w in words(sql) {
            if let Some(&b) = self.bytes.get(&w) {
                total = total.saturating_add(b);
            }
        }
        if total > VIEW_SQL_MAX {
            return Err((
                ErrorKind::Unsupported,
                "the database's views are too complex to read safely".into(),
            ));
        }
        Ok(())
    }
}

fn authorize(ctx: &AuthContext<'_>, tables: &HashSet<String>, views: bool) -> Authorization {
    if ctx.accessor.is_some() && !views {
        return Authorization::Deny;
    }
    if ctx.database_name.is_some_and(|d| d != "main") && !matches!(ctx.action, AuthAction::Select) {
        return Authorization::Deny;
    }
    match ctx.action {
        AuthAction::Select => Authorization::Allow,
        AuthAction::Read { table_name, .. } => {
            if tables.contains(&table_name.to_lowercase()) {
                Authorization::Allow
            } else {
                Authorization::Deny
            }
        }
        AuthAction::Function { function_name } => {
            if FUNCTIONS
                .iter()
                .any(|f| f.eq_ignore_ascii_case(function_name))
            {
                Authorization::Allow
            } else {
                Authorization::Deny
            }
        }
        AuthAction::Pragma { pragma_name, .. }
            if ctx.accessor.is_none() && pragma_name.eq_ignore_ascii_case("table_info") =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }
}

// ---------------------------------------------------------------------------------------------
// The VFS

/// What a VFS file reads: the database the viewer passed (with its WAL's committed pages laid
/// over it), or a temporary file in memory.
enum Backing {
    Db {
        file: File,
        len: u64,
        wal: Option<Wal>,
    },
    Mem(Vec<u8>),
}

/// The committed frames of a database's WAL file.
pub struct Wal {
    file: File,
    page_size: u64,
    /// Page number (from 1) to where its latest committed copy starts in the WAL.
    pages: HashMap<u32, u64>,
    /// The database's size in pages after the last commit.
    db_pages: u64,
}

/// SQLite's WAL checksum over `b` (a multiple of 8 bytes), continuing from `s`.
fn wal_checksum(b: &[u8], big_endian: bool, s: (u32, u32)) -> (u32, u32) {
    let (mut s0, mut s1) = s;
    for w in b.chunks_exact(8) {
        let (x0, x1) = if big_endian {
            (
                u32::from_be_bytes([w[0], w[1], w[2], w[3]]),
                u32::from_be_bytes([w[4], w[5], w[6], w[7]]),
            )
        } else {
            (
                u32::from_le_bytes([w[0], w[1], w[2], w[3]]),
                u32::from_le_bytes([w[4], w[5], w[6], w[7]]),
            )
        };
        s0 = s0.wrapping_add(x0).wrapping_add(s1);
        s1 = s1.wrapping_add(x1).wrapping_add(s0);
    }
    (s0, s1)
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl Wal {
    /// The committed frames of `wal`, the WAL of database `db`; `None` when it has none, or
    /// is not a valid WAL of this database (SQLite ignores such a file too). Reads every frame
    /// once (at most [`WAL_FRAMES_MAX`]); remembers only where each page's latest copy is.
    pub fn open(db: &File, wal: File) -> Result<Option<Wal>, Failure> {
        let io = |e: std::io::Error| (ErrorKind::Io, format!("cannot read the WAL file: {e}"));
        let mut h = [0u8; 32];
        if read_at(&wal, &mut h, 0).map_err(io)? < 32 {
            return Ok(None);
        }
        let magic = be32(&h, 0);
        if magic & !1 != 0x377f_0682 || be32(&h, 4) != 3_007_000 {
            return Ok(None);
        }
        let big = magic & 1 == 1;
        let page_size = u64::from(be32(&h, 8));
        if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Ok(None);
        }
        let salts = (be32(&h, 16), be32(&h, 20));
        let mut sum = wal_checksum(&h[..24], big, (0, 0));
        if sum != (be32(&h, 24), be32(&h, 28)) {
            return Ok(None);
        }
        // The database's own page size, when it has a header yet.
        let mut dh = [0u8; 100];
        if read_at(db, &mut dh, 0).map_err(io)? == 100 {
            let ps = u16::from_be_bytes([dh[16], dh[17]]);
            let ps = if ps == 1 { 65536 } else { u64::from(ps) };
            if ps != page_size {
                return Ok(None);
            }
        }
        let len = wal.metadata().map_err(io)?.len();
        let frame = 24 + page_size;
        let frames = len.saturating_sub(32) / frame;
        if frames > WAL_FRAMES_MAX {
            return Err((
                ErrorKind::TooLarge,
                format!("the database's WAL file has more than {WAL_FRAMES_MAX} pages"),
            ));
        }
        let mut buf = vec![0u8; frame as usize];
        let mut pending: Vec<(u32, u64)> = Vec::new();
        let mut pages: HashMap<u32, u64> = HashMap::new();
        let mut db_pages = 0u64;
        for k in 0..frames {
            let at = 32 + k * frame;
            if read_at(&wal, &mut buf, at).map_err(io)? < buf.len() {
                break;
            }
            let page = be32(&buf, 0);
            let commit = be32(&buf, 4);
            if page == 0 || (be32(&buf, 8), be32(&buf, 12)) != salts {
                break;
            }
            let s = wal_checksum(&buf[..8], big, sum);
            let s = wal_checksum(&buf[24..], big, s);
            if s != (be32(&buf, 16), be32(&buf, 20)) {
                break;
            }
            sum = s;
            pending.push((page, at + 24));
            if commit != 0 {
                pages.extend(pending.drain(..));
                db_pages = u64::from(commit);
            }
        }
        if pages.is_empty() {
            return Ok(None);
        }
        Ok(Some(Wal {
            file: wal,
            page_size,
            pages,
            db_pages,
        }))
    }

    /// Reads `out` from offset `off` of the database as of the WAL's last commit: each page
    /// from the WAL when it has one, else from `db`. Returns the bytes read.
    fn read_page_range(&self, db: &File, out: &mut [u8], off: u64) -> std::io::Result<usize> {
        let end = self.db_pages * self.page_size;
        let mut done = 0usize;
        while done < out.len() {
            let at = off + done as u64;
            if at >= end {
                break;
            }
            let page = at / self.page_size;
            let within = at % self.page_size;
            let take = ((self.page_size - within) as usize)
                .min(out.len() - done)
                .min((end - at) as usize);
            let part = &mut out[done..done + take];
            let got = match u32::try_from(page + 1)
                .ok()
                .and_then(|p| self.pages.get(&p))
            {
                Some(&f) => read_at(&self.file, part, f + within)?,
                None => read_at(db, part, at)?,
            };
            done += got;
            if got < take {
                break;
            }
        }
        Ok(done)
    }
}

/// SQLite's file object: the base SQLite sees, then our state.
#[repr(C)]
struct VfsFile {
    base: ffi::sqlite3_file,
    backing: *mut Backing,
}

static METHODS: ffi::sqlite3_io_methods = ffi::sqlite3_io_methods {
    iVersion: 1,
    xClose: Some(x_close),
    xRead: Some(x_read),
    xWrite: Some(x_write),
    xTruncate: Some(x_truncate),
    xSync: Some(x_sync),
    xFileSize: Some(x_file_size),
    xLock: Some(x_lock),
    xUnlock: Some(x_lock),
    xCheckReservedLock: Some(x_check_reserved_lock),
    xFileControl: Some(x_file_control),
    xSectorSize: Some(x_sector_size),
    xDeviceCharacteristics: Some(x_device_characteristics),
    xShmMap: None,
    xShmLock: None,
    xShmBarrier: None,
    xShmUnmap: None,
    xFetch: None,
    xUnfetch: None,
};

/// The state of an open file, or None (closed, or a null pointer).
fn backing<'a>(f: *mut ffi::sqlite3_file) -> Option<&'a mut Backing> {
    if f.is_null() {
        return None;
    }
    #[allow(unsafe_code)]
    // SAFETY: SQLite passes back the file object `x_open` initialised (it allocated
    // `szOsFile` = size of `VfsFile` bytes for it), and calls one method of a file at a time.
    let b = unsafe { (*f.cast::<VfsFile>()).backing };
    if b.is_null() {
        return None;
    }
    #[allow(unsafe_code)]
    // SAFETY: `backing` is the box `x_open` leaked for this file, freed only by `x_close`
    // (which nulls it); no other reference to it lives during this call.
    Some(unsafe { &mut *b })
}

extern "C" fn x_open(
    _vfs: *mut ffi::sqlite3_vfs,
    _name: ffi::sqlite3_filename,
    f: *mut ffi::sqlite3_file,
    flags: c_int,
    out_flags: *mut c_int,
) -> c_int {
    if f.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    let backing = if flags & ffi::SQLITE_OPEN_MAIN_DB != 0 {
        let Some((file, wal)) = PENDING.with(|p| p.borrow_mut().take()) else {
            return ffi::SQLITE_CANTOPEN;
        };
        let len = match &wal {
            Some(w) => w.db_pages * w.page_size,
            None => match file.metadata() {
                Ok(m) => m.len(),
                Err(_) => return ffi::SQLITE_CANTOPEN,
            },
        };
        Backing::Db { file, len, wal }
    } else if flags
        & (ffi::SQLITE_OPEN_TEMP_DB
            | ffi::SQLITE_OPEN_TEMP_JOURNAL
            | ffi::SQLITE_OPEN_TRANSIENT_DB
            | ffi::SQLITE_OPEN_SUBJOURNAL)
        != 0
    {
        Backing::Mem(Vec::new())
    } else {
        // Journals and WAL files: an immutable database has none.
        return ffi::SQLITE_CANTOPEN;
    };
    let file = VfsFile {
        base: ffi::sqlite3_file { pMethods: &METHODS },
        backing: Box::into_raw(Box::new(backing)),
    };
    #[allow(unsafe_code)]
    // SAFETY: SQLite gives `szOsFile` (= size of `VfsFile`) writable bytes, suitably aligned
    // (it allocates with 8-byte alignment), at `f` for this file object.
    unsafe {
        f.cast::<VfsFile>().write(file);
    }
    if !out_flags.is_null() {
        let ro = (flags & !ffi::SQLITE_OPEN_READWRITE) | ffi::SQLITE_OPEN_READONLY;
        #[allow(unsafe_code)]
        // SAFETY: a valid out pointer when not null.
        unsafe {
            *out_flags = if flags & ffi::SQLITE_OPEN_MAIN_DB != 0 {
                ro
            } else {
                flags
            };
        }
    }
    ffi::SQLITE_OK
}

extern "C" fn x_close(f: *mut ffi::sqlite3_file) -> c_int {
    if f.is_null() {
        return ffi::SQLITE_OK;
    }
    #[allow(unsafe_code)]
    // SAFETY: as in `backing`: the object `x_open` wrote.
    let b = unsafe { std::mem::replace(&mut (*f.cast::<VfsFile>()).backing, std::ptr::null_mut()) };
    if !b.is_null() {
        #[allow(unsafe_code)]
        // SAFETY: the box `x_open` leaked, taken back exactly once (the pointer is now null).
        drop(unsafe { Box::from_raw(b) });
    }
    ffi::SQLITE_OK
}

fn read_at(file: &File, buf: &mut [u8], at: u64) -> std::io::Result<usize> {
    let mut done = 0;
    while done < buf.len() {
        #[cfg(unix)]
        let r = std::os::unix::fs::FileExt::read_at(file, &mut buf[done..], at + done as u64);
        #[cfg(windows)]
        let r = std::os::windows::fs::FileExt::seek_read(file, &mut buf[done..], at + done as u64);
        match r {
            Ok(0) => break,
            Ok(k) => done += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(done)
}

extern "C" fn x_read(f: *mut ffi::sqlite3_file, buf: *mut c_void, amt: c_int, off: i64) -> c_int {
    let (Some(b), Ok(n), Ok(off)) = (backing(f), usize::try_from(amt), u64::try_from(off)) else {
        return ffi::SQLITE_IOERR_READ;
    };
    if buf.is_null() {
        return ffi::SQLITE_IOERR_READ;
    }
    #[allow(unsafe_code)]
    // SAFETY: SQLite passes a buffer writable for `amt` bytes for this call.
    let out = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), n) };
    let got = match b {
        Backing::Db {
            file, wal: None, ..
        } => match read_at(file, out, off) {
            Ok(k) => k,
            Err(_) => return ffi::SQLITE_IOERR_READ,
        },
        Backing::Db {
            file, wal: Some(w), ..
        } => match w.read_page_range(file, out, off) {
            Ok(k) => k,
            Err(_) => return ffi::SQLITE_IOERR_READ,
        },
        Backing::Mem(v) => {
            let start = usize::try_from(off).unwrap_or(usize::MAX).min(v.len());
            let k = (v.len() - start).min(n);
            out[..k].copy_from_slice(&v[start..start + k]);
            k
        }
    };
    if got < n {
        // SQLite requires the rest zeroed on a short read.
        out[got..].fill(0);
        return ffi::SQLITE_IOERR_SHORT_READ;
    }
    ffi::SQLITE_OK
}

extern "C" fn x_write(
    f: *mut ffi::sqlite3_file,
    buf: *const c_void,
    amt: c_int,
    off: i64,
) -> c_int {
    let (Some(Backing::Mem(v)), Ok(n), Ok(off)) =
        (backing(f), usize::try_from(amt), usize::try_from(off))
    else {
        return ffi::SQLITE_READONLY;
    };
    if buf.is_null() {
        return ffi::SQLITE_IOERR_WRITE;
    }
    let Some(end) = off.checked_add(n).filter(|&e| e <= TEMP_MAX) else {
        return ffi::SQLITE_FULL;
    };
    if end > v.len() {
        if v.try_reserve(end - v.len()).is_err() {
            return ffi::SQLITE_NOMEM;
        }
        v.resize(end, 0);
    }
    #[allow(unsafe_code)]
    // SAFETY: SQLite passes a buffer readable for `amt` bytes for this call.
    let src = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), n) };
    v[off..end].copy_from_slice(src);
    ffi::SQLITE_OK
}

extern "C" fn x_truncate(f: *mut ffi::sqlite3_file, size: i64) -> c_int {
    match (backing(f), usize::try_from(size)) {
        (Some(Backing::Mem(v)), Ok(n)) => {
            v.truncate(n);
            ffi::SQLITE_OK
        }
        _ => ffi::SQLITE_READONLY,
    }
}

extern "C" fn x_sync(_f: *mut ffi::sqlite3_file, _flags: c_int) -> c_int {
    ffi::SQLITE_OK
}

extern "C" fn x_file_size(f: *mut ffi::sqlite3_file, out: *mut i64) -> c_int {
    let len = match backing(f) {
        Some(Backing::Db { len, .. }) => *len,
        Some(Backing::Mem(v)) => v.len() as u64,
        None => return ffi::SQLITE_IOERR_FSTAT,
    };
    if out.is_null() {
        return ffi::SQLITE_IOERR_FSTAT;
    }
    #[allow(unsafe_code)]
    // SAFETY: a valid out pointer (checked not null).
    unsafe {
        *out = i64::try_from(len).unwrap_or(i64::MAX);
    }
    ffi::SQLITE_OK
}

/// Locks: the database is immutable and read by one connection at a time.
extern "C" fn x_lock(_f: *mut ffi::sqlite3_file, _level: c_int) -> c_int {
    ffi::SQLITE_OK
}

extern "C" fn x_check_reserved_lock(_f: *mut ffi::sqlite3_file, out: *mut c_int) -> c_int {
    if !out.is_null() {
        #[allow(unsafe_code)]
        // SAFETY: a valid out pointer (checked not null).
        unsafe {
            *out = 0;
        }
    }
    ffi::SQLITE_OK
}

extern "C" fn x_file_control(_f: *mut ffi::sqlite3_file, _op: c_int, _arg: *mut c_void) -> c_int {
    ffi::SQLITE_NOTFOUND
}

extern "C" fn x_sector_size(_f: *mut ffi::sqlite3_file) -> c_int {
    4096
}

extern "C" fn x_device_characteristics(f: *mut ffi::sqlite3_file) -> c_int {
    match backing(f) {
        Some(Backing::Db { .. }) => ffi::SQLITE_IOCAP_IMMUTABLE,
        _ => 0,
    }
}

extern "C" fn x_delete(_vfs: *mut ffi::sqlite3_vfs, _name: *const c_char, _sync: c_int) -> c_int {
    ffi::SQLITE_OK
}

/// No other file exists (no journal, no WAL).
extern "C" fn x_access(
    _vfs: *mut ffi::sqlite3_vfs,
    _name: *const c_char,
    _flags: c_int,
    out: *mut c_int,
) -> c_int {
    if !out.is_null() {
        #[allow(unsafe_code)]
        // SAFETY: a valid out pointer (checked not null).
        unsafe {
            *out = 0;
        }
    }
    ffi::SQLITE_OK
}

extern "C" fn x_full_pathname(
    _vfs: *mut ffi::sqlite3_vfs,
    name: *const c_char,
    n_out: c_int,
    out: *mut c_char,
) -> c_int {
    let Ok(cap) = usize::try_from(n_out) else {
        return ffi::SQLITE_CANTOPEN;
    };
    if name.is_null() || out.is_null() || cap == 0 {
        return ffi::SQLITE_CANTOPEN;
    }
    #[allow(unsafe_code)]
    // SAFETY: SQLite passes a NUL-terminated name.
    let src = unsafe { CStr::from_ptr(name) }.to_bytes_with_nul();
    if src.len() > cap {
        return ffi::SQLITE_CANTOPEN;
    }
    #[allow(unsafe_code)]
    // SAFETY: `out` is writable for `n_out` bytes, and `src` (NUL included) fits.
    unsafe {
        std::ptr::copy_nonoverlapping(src.as_ptr().cast::<c_char>(), out, src.len());
    }
    ffi::SQLITE_OK
}

extern "C" fn x_randomness(_vfs: *mut ffi::sqlite3_vfs, n: c_int, out: *mut c_char) -> c_int {
    use std::hash::{BuildHasher, Hasher};
    let Ok(n) = usize::try_from(n) else { return 0 };
    if out.is_null() {
        return 0;
    }
    #[allow(unsafe_code)]
    // SAFETY: SQLite passes a buffer writable for `n` bytes.
    let buf = unsafe { std::slice::from_raw_parts_mut(out.cast::<u8>(), n) };
    let state = std::collections::hash_map::RandomState::new();
    for (i, chunk) in buf.chunks_mut(8).enumerate() {
        let mut h = state.build_hasher();
        h.write_usize(i);
        chunk.copy_from_slice(&h.finish().to_le_bytes()[..chunk.len()]);
    }
    n as c_int
}

extern "C" fn x_sleep(_vfs: *mut ffi::sqlite3_vfs, micros: c_int) -> c_int {
    std::thread::sleep(Duration::from_micros(u64::try_from(micros).unwrap_or(0)));
    micros
}

/// Milliseconds since the Julian epoch.
fn julian_ms() -> i64 {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    210_866_760_000_000 + unix
}

extern "C" fn x_current_time(_vfs: *mut ffi::sqlite3_vfs, out: *mut f64) -> c_int {
    if !out.is_null() {
        #[allow(unsafe_code)]
        // SAFETY: a valid out pointer (checked not null).
        unsafe {
            *out = julian_ms() as f64 / 86_400_000.0;
        }
    }
    ffi::SQLITE_OK
}

extern "C" fn x_current_time_i64(_vfs: *mut ffi::sqlite3_vfs, out: *mut i64) -> c_int {
    if !out.is_null() {
        #[allow(unsafe_code)]
        // SAFETY: a valid out pointer (checked not null).
        unsafe {
            *out = julian_ms();
        }
    }
    ffi::SQLITE_OK
}

extern "C" fn x_get_last_error(_vfs: *mut ffi::sqlite3_vfs, _n: c_int, _out: *mut c_char) -> c_int {
    0
}
