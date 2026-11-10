use super::*;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("specular-clip-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// A database like Clip Studio's, built with SQLite itself.
fn database(preview: &[u8], layers: usize, page_size: u32) -> Vec<u8> {
    let p = temp(&format!("{layers}-{page_size}.sqlite"));
    {
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch(&format!(
            "PRAGMA page_size={page_size}; PRAGMA synchronous=OFF; PRAGMA journal_mode=MEMORY;"
        ))
        .unwrap();
        c.execute_batch(
            "CREATE TABLE Project(_PW_ID INTEGER PRIMARY KEY AUTOINCREMENT, \
             ProjectInternalVersion TEXT DEFAULT NULL, ProjectName TEXT DEFAULT NULL);
             CREATE TABLE Canvas(_PW_ID INTEGER PRIMARY KEY AUTOINCREMENT, MainId INTEGER, \
             CanvasUnit INTEGER, CanvasWidth REAL, CanvasHeight REAL, CanvasResolution REAL, \
             CanvasChannelBytes INTEGER);
             CREATE TABLE CanvasPreview(_PW_ID INTEGER PRIMARY KEY AUTOINCREMENT, MainId \
             INTEGER DEFAULT NULL, CanvasId INTEGER DEFAULT NULL, ImageType INTEGER DEFAULT \
             NULL, ImageWidth INTEGER DEFAULT NULL, ImageHeight INTEGER DEFAULT NULL, \
             ImageData BLOB DEFAULT NULL);
             CREATE TABLE Layer(_PW_ID INTEGER PRIMARY KEY AUTOINCREMENT, MainId INTEGER, \
             CanvasId INTEGER, LayerName TEXT, LayerType INTEGER, LayerVisibility INTEGER, \
             LayerFolder INTEGER);
             INSERT INTO Project VALUES(NULL, '1.1.0', 'x');
             INSERT INTO Canvas VALUES(NULL, 1, 0, 640.0, 480.0, 350.0, 1);
             INSERT INTO Layer VALUES(NULL, 2, 1, '', 256, 1, 1);",
        )
        .unwrap();
        c.execute(
            "INSERT INTO CanvasPreview VALUES(NULL, 1, 1, 1, 320, 240, ?1)",
            [preview],
        )
        .unwrap();
        for i in 0..layers {
            c.execute(
                "INSERT INTO Layer VALUES(NULL, ?1, 1, ?2, 1, ?3, ?4)",
                rusqlite::params![
                    i as i64 + 3,
                    format!("Layer {i} 背景"),
                    (i % 2) as i64,
                    (i % 10 == 0) as i64
                ],
            )
            .unwrap();
        }
    }
    let d = std::fs::read(&p).unwrap();
    let _ = std::fs::remove_file(&p);
    d
}

fn container(db: &[u8]) -> Vec<u8> {
    let mut f = b"CSFCHUNK".to_vec();
    f.extend_from_slice(&0u64.to_be_bytes()); // size, patched below
    f.extend_from_slice(&24u64.to_be_bytes());
    let mut chunk = |tag: &[u8], body: &[u8]| {
        f.extend_from_slice(tag);
        f.extend_from_slice(&(body.len() as u64).to_be_bytes());
        f.extend_from_slice(body);
    };
    chunk(b"CHNKHead", &[0; 40]);
    chunk(b"CHNKExta", &[0; 64]);
    chunk(b"CHNKSQLi", db);
    chunk(b"CHNKFoot", &[]);
    let n = f.len() as u64;
    f[8..16].copy_from_slice(&n.to_be_bytes());
    f
}

fn fake_png(len: usize) -> Vec<u8> {
    let mut p = b"\x89PNG\r\n\x1a\n".to_vec();
    p.extend((0..len).map(|i| (i * 7 % 251) as u8));
    p
}

#[test]
fn reads_preview_facts_and_layers() {
    // A preview spilling over many overflow pages and enough layers for interior pages.
    for page_size in [512, 1024, 4096, 65536] {
        let png = fake_png(200_000);
        let f = container(&database(&png, 400, page_size));
        assert!(probe(&f));
        let c = parse(&f).unwrap();
        assert_eq!(c.preview.bytes, png, "page size {page_size}");
        assert_eq!((c.preview.width, c.preview.height), (320, 240));
        assert_eq!(c.preview.kind, ImageKind::Png);
        assert_eq!(c.version.as_deref(), Some("1.1.0"));
        let cv = c.canvas.unwrap();
        assert_eq!((cv.width, cv.height, cv.dpi), (640.0, 480.0, 350.0));
        assert_eq!(c.layers.len(), 400);
        assert_eq!(c.layers[7].name, "Layer 7 背景");
        assert!(c.layers[7].visible && !c.layers[8].visible);
        assert_eq!(c.layers.iter().filter(|l| l.folder).count(), 40);
        assert_eq!(c.external_chunks, 1);
        assert_eq!(
            c.describe(),
            "Clip Studio Paint illustration, 360 layers in 40 folders, 350 dpi, preview of \
             the 640×480 canvas at reduced size"
        );
    }
}

#[test]
fn refusals_are_plain() {
    assert_eq!(parse(b"PNG"), Err(Error::NotClip));
    let f = container(&database(b"GIF89a", 1, 4096));
    let e = parse(&f).unwrap_err();
    assert!(e.to_string().contains("unknown image format"), "{e}");
    // Cut inside the database.
    let f = container(&database(&fake_png(10), 1, 4096));
    let cut = &f[..f.len() / 2];
    assert!(matches!(parse(cut), Err(Error::Malformed(_))));
}

#[test]
fn damaged_databases_never_panic() {
    let f = container(&database(&fake_png(5000), 50, 512));
    let db_at = f
        .windows(16)
        .position(|w| w == b"SQLite format 3\0")
        .unwrap();
    // Cut anywhere: an error or a result, never a panic.
    for cut in (db_at..f.len()).step_by(37) {
        let _ = parse(&f[..cut]);
    }
    // Every interior page's right-child pointer aimed at page 1: a loop, refused.
    let mut g = f.clone();
    for page in (db_at..g.len() - 512).step_by(512) {
        if g[page] == 0x05 {
            g[page + 8..page + 12].copy_from_slice(&1u32.to_be_bytes());
        }
    }
    let _ = parse(&g);
}

fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_CLIP_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/design2/clip")
        });
    d.is_dir().then_some(d)
}

/// Every corpus file against SQLite itself: the preview blob and the layer names.
#[test]
fn corpus_matches_sqlite() {
    let Some(dir) = corpus() else {
        eprintln!("clip corpus absent: skipped");
        return;
    };
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "clip") {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let c = parse(&data).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let (range, _) = chunks(&data).unwrap();
        let tmp = temp("oracle.sqlite");
        std::fs::write(&tmp, &data[range]).unwrap();
        let db = rusqlite::Connection::open(&tmp).unwrap();
        let blob: Vec<u8> = db
            .query_row("SELECT ImageData FROM CanvasPreview", [], |r| r.get(0))
            .unwrap();
        assert_eq!(blob, c.preview.bytes, "{}", p.display());
        let mut st = db
            .prepare("SELECT LayerName FROM Layer WHERE LayerType != 256 ORDER BY _PW_ID")
            .unwrap();
        let names: Vec<String> = st
            .query_map([], |r| r.get::<_, Option<String>>(0))
            .unwrap()
            .map(|r| r.unwrap().unwrap_or_default())
            .collect();
        let ours: Vec<String> = c.layers.iter().map(|l| l.name.clone()).collect();
        assert_eq!(names, ours, "{}", p.display());
        drop(st);
        drop(db);
        let _ = std::fs::remove_file(&tmp);
        eprintln!("{}: {}", p.display(), c.describe());
        n += 1;
    }
    assert!(n > 0);
}
