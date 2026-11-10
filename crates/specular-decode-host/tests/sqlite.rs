//! SQLite in the sandboxed host: GeoPackage and MBTiles databases are queried there, never in
//! the viewer. Normal reads, then hostile databases (views with recursive CTEs, zeroblob,
//! endless joins, huge page counts, virtual tables, writes).

use specular_sandbox::protocol::ErrorKind;
use specular_sandbox::{Config, Error, Host, SqlLimits, SqlSource, SqlValue};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn host() -> Host {
    Host::new(Config {
        exe: Some(env!("CARGO_BIN_EXE_specular-decode-host").into()),
        timeout: Duration::from_secs(20),
        pool_size: 1,
        ..Config::default()
    })
}

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-host-sqlite-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A database at `name` built with `sql`.
fn db(name: &str, sql: &str) -> PathBuf {
    let p = dir().join(name);
    let _ = std::fs::remove_file(&p);
    let c = rusqlite::Connection::open(&p).unwrap();
    c.execute_batch(sql).unwrap();
    drop(c);
    p
}

fn query(
    h: &Host,
    p: &Path,
    sql: &str,
    params: &[SqlValue],
    limits: SqlLimits,
) -> Result<Vec<Vec<SqlValue>>, Error> {
    let src = SqlSource::open(p).unwrap();
    h.sql_query(&src, sql, params, limits)
        .map(|r| r.into_rows())
}

fn kind(e: &Error) -> Option<ErrorKind> {
    match e {
        Error::Remote { kind, .. } => Some(*kind),
        _ => None,
    }
}

const TABLE: &str = "CREATE TABLE t (a INTEGER PRIMARY KEY, b TEXT, c BLOB, d REAL);
    INSERT INTO t VALUES (1, 'Grüße', x'00FF10', 2.5), (2, NULL, NULL, -1e300), (3, 'x', x'', 0);";

#[test]
fn reads_rows_in_the_sandbox() {
    let h = host();
    let info = h.info().unwrap();
    #[cfg(target_os = "linux")]
    assert!(
        info.sandbox >= specular_sandbox::SandboxLevel::Partial,
        "{info:?}"
    );
    assert!(!info.sandbox_detail.contains("VFS"), "{info:?}");
    let p = db("rows.db", TABLE);
    let rows = query(
        &h,
        &p,
        "SELECT a, b, c, d FROM t WHERE a >= ?1 ORDER BY a",
        &[SqlValue::Int(1)],
        SqlLimits::default(),
    )
    .unwrap();
    assert_eq!(
        rows[0],
        [
            SqlValue::Int(1),
            SqlValue::Text("Grüße".into()),
            SqlValue::Blob(vec![0, 0xFF, 0x10]),
            SqlValue::Real(2.5)
        ]
    );
    assert_eq!(rows[1][1], SqlValue::Null);
    assert_eq!(rows[1][3], SqlValue::Real(-1e300));
    assert_eq!(rows.len(), 3);
    // The same source again reuses the host's connection; a second source works alike.
    let src = SqlSource::open(&p).unwrap();
    for _ in 0..3 {
        let r = h
            .sql_query(&src, "SELECT count(*) FROM t", &[], SqlLimits::default())
            .unwrap();
        assert_eq!(r.values, [SqlValue::Int(3)]);
    }
    // Text parameters, and table_info.
    let r = query(
        &h,
        &p,
        "SELECT count(*) FROM sqlite_master WHERE name = ?1 AND type IN ('table','view')",
        &[SqlValue::Text("t".into())],
        SqlLimits::default(),
    )
    .unwrap();
    assert_eq!(r[0][0], SqlValue::Int(1));
    let cols = query(
        &h,
        &p,
        "PRAGMA table_info(\"t\")",
        &[],
        SqlLimits::default(),
    )
    .unwrap();
    assert_eq!(cols.len(), 4);
    assert_eq!(cols[1][1], SqlValue::Text("b".into()));
}

#[test]
fn rows_and_bytes_are_limited() {
    let h = host();
    let p = db(
        "limits.db",
        "CREATE TABLE n (v BLOB); \
         WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 100) \
         INSERT INTO n SELECT randomblob(2000000) FROM c WHERE x <= 3;",
    );
    let limits = SqlLimits {
        max_rows: 2,
        ..SqlLimits::default()
    };
    let src = SqlSource::open(&p).unwrap();
    let r = h
        .sql_query(&src, "SELECT length(v) FROM n", &[], limits)
        .unwrap();
    assert_eq!(r.len(), 2);
    assert!(r.truncated);
    // A value past `max_value`: SQLite refuses it (SQLITE_TOOBIG).
    let small = SqlLimits {
        max_value: 1 << 20,
        ..SqlLimits::default()
    };
    let e = h
        .sql_query(&src, "SELECT v FROM n", &[], small)
        .unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::TooLarge), "{e}");
    // All rows past `max_bytes`.
    let few = SqlLimits {
        max_bytes: 3 << 20,
        ..SqlLimits::default()
    };
    let e = h.sql_query(&src, "SELECT v FROM n", &[], few).unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::TooLarge), "{e}");
    // The host still answers.
    let r = h.sql_query(&src, "SELECT count(*) FROM n", &[], SqlLimits::default());
    assert_eq!(r.unwrap().values, [SqlValue::Int(3)]);
}

#[test]
fn views_only_when_allowed() {
    let h = host();
    let p = db(
        "view.db",
        &format!("{TABLE} CREATE VIEW v AS SELECT a, b FROM t WHERE a < 3;"),
    );
    let e = query(&h, &p, "SELECT * FROM v", &[], SqlLimits::default()).unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{e}");
    let views = SqlLimits {
        allow_views: true,
        ..SqlLimits::default()
    };
    let r = query(&h, &p, "SELECT * FROM v", &[], views).unwrap();
    assert_eq!(r.len(), 2);
}

#[test]
fn recursive_cte_view_is_refused() {
    let h = host();
    // MBTiles' `tiles` as an endless recursive view.
    let p = db(
        "cte.mbtiles",
        "CREATE VIEW tiles AS WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) \
         SELECT x AS zoom_level, x AS tile_column, x AS tile_row, x AS tile_data FROM c;",
    );
    let views = SqlLimits {
        allow_views: true,
        ..SqlLimits::default()
    };
    let t = Instant::now();
    let e = query(
        &h,
        &p,
        "SELECT min(zoom_level), max(zoom_level), count(*) FROM tiles",
        &[],
        views,
    )
    .unwrap_err();
    assert!(t.elapsed() < Duration::from_secs(5));
    assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{e}");
}

#[test]
fn zeroblob_and_other_functions_are_refused() {
    let h = host();
    let p = db(
        "zeroblob.mbtiles",
        "CREATE VIEW tiles AS SELECT 0 AS zoom_level, 0 AS tile_column, 0 AS tile_row, \
         zeroblob(2000000000) AS tile_data;
         CREATE VIEW meta AS SELECT printf('%.*c', 1000000000, 'x') AS v;",
    );
    let views = SqlLimits {
        allow_views: true,
        ..SqlLimits::default()
    };
    for sql in ["SELECT tile_data FROM tiles", "SELECT v FROM meta"] {
        let e = query(&h, &p, sql, &[], views).unwrap_err();
        assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{sql}: {e}");
    }
    // Directly in the viewer's SQL too (the allowlist is the same).
    let e = query(&h, &p, "SELECT zeroblob(10)", &[], views).unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{e}");
}

#[test]
fn endless_join_stops_at_the_deadline() {
    let h = host();
    let p = db(
        "join.mbtiles",
        "CREATE TABLE a (x); \
         WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 2000) \
         INSERT INTO a SELECT x FROM c; \
         CREATE VIEW tiles AS SELECT a1.x AS zoom_level, a2.x AS tile_column, a3.x AS tile_row, \
         a4.x AS tile_data FROM a a1, a a2, a a3, a a4;",
    );
    let limits = SqlLimits {
        allow_views: true,
        millis: 300,
        ..SqlLimits::default()
    };
    let t = Instant::now();
    let e = query(&h, &p, "SELECT count(*) FROM tiles", &[], limits).unwrap_err();
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert_eq!(kind(&e), Some(ErrorKind::TooLarge), "{e}");
}

#[test]
fn huge_page_counts_and_garbage() {
    let h = host();
    let p = db("pages.db", TABLE);
    let mut b = std::fs::read(&p).unwrap();
    // In-header database size (trusted when the version-valid-for number matches the change
    // counter): two billion pages in a file of a few.
    b[28..32].copy_from_slice(&0x7FFF_FFF0u32.to_be_bytes());
    let counter: [u8; 4] = b[24..28].try_into().unwrap();
    b[92..96].copy_from_slice(&counter);
    let huge = dir().join("pages-huge.db");
    std::fs::write(&huge, &b).unwrap();
    let t = Instant::now();
    let r = query(
        &h,
        &huge,
        "SELECT count(*) FROM t",
        &[],
        SqlLimits::default(),
    );
    assert!(t.elapsed() < Duration::from_secs(5));
    if let Err(e) = &r {
        assert!(matches!(kind(e), Some(ErrorKind::Malformed)), "{e}");
    }
    // Largest page size, tiny file; then a root page far past the end.
    let mut b2 = b.clone();
    b2[16..18].copy_from_slice(&1u16.to_be_bytes());
    let big_pages = dir().join("pages-64k.db");
    std::fs::write(&big_pages, &b2).unwrap();
    let r = query(&h, &big_pages, "SELECT * FROM t", &[], SqlLimits::default());
    assert!(r.is_err());
    // Garbage and an empty file.
    let junk = dir().join("junk.db");
    let mut g = b"SQLite format 3\0".to_vec();
    g.extend((0..5000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8));
    std::fs::write(&junk, &g).unwrap();
    let e = query(
        &h,
        &junk,
        "SELECT count(*) FROM sqlite_master",
        &[],
        SqlLimits::default(),
    )
    .unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::Malformed), "{e}");
    let empty = dir().join("empty.db");
    std::fs::write(&empty, b"").unwrap();
    let r = query(
        &h,
        &empty,
        "SELECT count(*) FROM sqlite_master",
        &[],
        SqlLimits::default(),
    );
    assert_eq!(r.unwrap()[0][0], SqlValue::Int(0));
    // The host survived all of it.
    let r = query(&h, &p, "SELECT count(*) FROM t", &[], SqlLimits::default());
    assert_eq!(r.unwrap()[0][0], SqlValue::Int(3));
}

#[test]
fn writes_attach_and_virtual_tables_are_refused() {
    let h = host();
    let p = db(
        "rw.db",
        &format!(
            "{TABLE} CREATE TABLE log (m); \
             CREATE TRIGGER tr AFTER INSERT ON t BEGIN INSERT INTO log VALUES ('x'); END;"
        ),
    );
    let before = std::fs::read(&p).unwrap();
    for sql in [
        "DELETE FROM t",
        "INSERT INTO t (a) VALUES (9)",
        "CREATE TABLE z (q)",
        "ATTACH DATABASE 'other.db' AS o",
        "PRAGMA writable_schema = ON",
        "SELECT load_extension('x')",
    ] {
        assert!(
            query(&h, &p, sql, &[], SqlLimits::default()).is_err(),
            "{sql} was allowed"
        );
    }
    assert_eq!(std::fs::read(&p).unwrap(), before, "the file changed");
    // A virtual table (R-tree, as GeoPackage spatial indexes are) is never read.
    let v = dir().join("vtab.db");
    let _ = std::fs::remove_file(&v);
    let c = rusqlite::Connection::open(&v).unwrap();
    if c.execute_batch(
        "CREATE VIRTUAL TABLE r USING rtree(id, x0, x1); INSERT INTO r VALUES (1, 0, 1);",
    )
    .is_ok()
    {
        drop(c);
        let e = query(&h, &v, "SELECT * FROM r", &[], SqlLimits::default()).unwrap_err();
        assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{e}");
        // SQLite's built-in virtual tables neither, directly or through a view.
        for sql in [
            "SELECT * FROM dbstat",
            "SELECT * FROM json_each('[1,2]')",
            "SELECT * FROM pragma_table_info('r')",
        ] {
            let e = query(&h, &v, sql, &[], SqlLimits::default()).unwrap_err();
            assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{sql}: {e}");
        }
        // Its shadow tables are plain tables.
        assert!(
            query(
                &h,
                &v,
                "SELECT count(*) FROM r_node",
                &[],
                SqlLimits::default()
            )
            .is_ok()
        );
    }
}

#[test]
fn wal_database_opens() {
    let h = host();
    let p = dir().join("wal.db");
    let _ = std::fs::remove_file(&p);
    let c = rusqlite::Connection::open(&p).unwrap();
    c.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
        .unwrap();
    c.execute_batch(TABLE).unwrap();
    drop(c);
    assert_eq!(std::fs::read(&p).unwrap()[18..20], [2, 2], "WAL mode");
    let r = query(&h, &p, "SELECT count(*) FROM t", &[], SqlLimits::default()).unwrap();
    assert_eq!(r[0][0], SqlValue::Int(3));
}

/// A writer that keeps its transactions in the WAL (as QGIS does while a GeoPackage is open):
/// `TABLE`, then 1000 more rows and an update, none checkpointed. Returns the open writer
/// (closing it would checkpoint) and the database's path.
fn wal_writer(name: &str) -> (rusqlite::Connection, PathBuf) {
    let p = dir().join(name);
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(wal_of(&p));
    let c = rusqlite::Connection::open(&p).unwrap();
    c.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
        .unwrap();
    c.execute_batch("PRAGMA wal_autocheckpoint = 0;").unwrap();
    c.execute_batch(TABLE).unwrap();
    c.execute_batch(
        "WITH RECURSIVE n(x) AS (SELECT 10 UNION ALL SELECT x + 1 FROM n WHERE x < 1009) \
         INSERT INTO t (a, b) SELECT x, 'w' FROM n; UPDATE t SET b = 'new' WHERE a = 1;",
    )
    .unwrap();
    (c, p)
}

fn wal_of(p: &Path) -> PathBuf {
    let mut w = p.as_os_str().to_owned();
    w.push("-wal");
    PathBuf::from(w)
}

/// The committed transactions in a database's `-wal` file are read, and neither file is
/// written.
#[test]
fn wal_transactions_are_read() {
    let h = host();
    let (writer, p) = wal_writer("live.db");
    let wal = wal_of(&p);
    assert!(
        std::fs::metadata(&wal).unwrap().len() > 32,
        "the WAL holds the data"
    );
    let before = (std::fs::read(&p).unwrap(), std::fs::read(&wal).unwrap());
    let r = query(
        &h,
        &p,
        "SELECT count(*), max(a), (SELECT b FROM t WHERE a = 1) FROM t",
        &[],
        SqlLimits::default(),
    )
    .unwrap();
    assert_eq!(
        r[0],
        [
            SqlValue::Int(1003),
            SqlValue::Int(1009),
            SqlValue::Text("new".into())
        ]
    );
    assert_eq!(
        (std::fs::read(&p).unwrap(), std::fs::read(&wal).unwrap()),
        before,
        "a file was written"
    );
    drop(writer);
}

/// Hostile WAL files: a frame whose page was changed ends the WAL there (the transactions
/// committed before it are read); a bad header, a WAL of another page size or of other salts
/// is not used; a WAL declaring more frames than are read at most is refused before any is
/// read.
#[test]
fn hostile_wal_files() {
    let h = host();
    let (writer, live) = wal_writer("src.db");
    let (db, wal) = (
        std::fs::read(&live).unwrap(),
        std::fs::read(wal_of(&live)).unwrap(),
    );
    drop(writer);
    let page = u32::from_be_bytes(wal[8..12].try_into().unwrap()) as usize;
    let frame = 24 + page;
    let frames = (wal.len() - 32) / frame;
    assert!(frames >= 3, "{frames}");
    let case = |name: &str, w: &[u8]| {
        let p = dir().join(name);
        std::fs::write(&p, &db).unwrap();
        std::fs::write(wal_of(&p), w).unwrap();
        query(
            &h,
            &p,
            "SELECT count(*), (SELECT b FROM t WHERE a = 1) FROM t",
            &[],
            SqlLimits::default(),
        )
    };
    // A transaction's last frame is its commit: corrupting the last frame's page drops only
    // the last transaction (the update); the 1000 rows committed before it are read.
    let mut w = wal.clone();
    let last = 32 + (frames - 1) * frame + 24;
    w[last + 100] ^= 0x55;
    let r = case("corrupt.db", &w).unwrap();
    assert_eq!(r[0], [SqlValue::Int(1003), SqlValue::Text("Grüße".into())]);
    // Bad magic, another page size, other salts: the WAL is not used (the main file alone
    // has no table yet).
    for (name, at) in [("magic.db", 0), ("size.db", 10), ("salt.db", 16)] {
        let mut w = wal.clone();
        w[at] ^= 0x80;
        let r = case(name, &w);
        assert!(
            r.as_ref().map_or(true, |r| r[0][0] != SqlValue::Int(1003)),
            "{name}: {r:?}"
        );
    }
    // A WAL of 4 Mi frames and one more (sparse: nothing is read before the refusal).
    let p = dir().join("huge.db");
    std::fs::write(&p, &db).unwrap();
    let f = std::fs::File::create(wal_of(&p)).unwrap();
    std::io::Write::write_all(&mut &f, &wal[..32]).unwrap();
    f.set_len(32 + ((4u64 << 20) + 1) * frame as u64).unwrap();
    drop(f);
    let e = query(&h, &p, "SELECT count(*) FROM t", &[], SqlLimits::default()).unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::TooLarge), "{e}");
    let _ = std::fs::remove_file(wal_of(&p));
}

/// Views naming views: the SQL they expand to is measured before SQLite expands it. A chain
/// doubling at each of 30 levels (2³⁰ references) and a cycle are refused; a short chain
/// reads.
#[test]
fn view_expansion_is_bounded() {
    let h = host();
    let mut sql = format!("{TABLE} CREATE VIEW v0 AS SELECT a FROM t;");
    for k in 1..=30 {
        sql += &format!(
            "CREATE VIEW v{k} AS SELECT a FROM v{} UNION ALL SELECT a FROM v{};",
            k - 1,
            k - 1
        );
    }
    let p = db("chain.db", &sql);
    let views = SqlLimits {
        allow_views: true,
        ..SqlLimits::default()
    };
    let r = query(&h, &p, "SELECT count(*) FROM v3", &[], views).unwrap();
    assert_eq!(r[0][0], SqlValue::Int(24));
    let e = query(&h, &p, "SELECT count(*) FROM v30", &[], views).unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{e}");
    assert!(e.to_string().contains("too complex"), "{e}");
    // Two views naming each other.
    let p = db("cycle.db", TABLE);
    let c = rusqlite::Connection::open(&p).unwrap();
    c.execute_batch(
        "PRAGMA writable_schema = ON;
         INSERT INTO sqlite_master VALUES ('view', 'ca', 'ca', 0, 'CREATE VIEW ca AS SELECT * FROM cb');
         INSERT INTO sqlite_master VALUES ('view', 'cb', 'cb', 0, 'CREATE VIEW cb AS SELECT * FROM ca');",
    )
    .unwrap();
    drop(c);
    let e = query(&h, &p, "SELECT * FROM ca", &[], views).unwrap_err();
    assert_eq!(kind(&e), Some(ErrorKind::Unsupported), "{e}");
    // Its tables read as before.
    let r = query(&h, &p, "SELECT count(*) FROM t", &[], SqlLimits::default()).unwrap();
    assert_eq!(r[0][0], SqlValue::Int(3));
}
