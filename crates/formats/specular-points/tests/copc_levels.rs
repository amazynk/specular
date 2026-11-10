//! COPC read by ranges, level by level: the levels fetched open as a cloud of their own,
//! all levels give the same cloud as the whole file, and fewer levels give the coarse
//! points only. The files come from SPECULAR_POINTS_COPC (a folder of `.copc.laz`); the test
//! skips without it.

use specular_points::{Limits, PointFormat, RangeRead, fetch_levels, read};
use std::sync::atomic::{AtomicU64, Ordering};

struct Counted<'a>(&'a [u8], AtomicU64);

impl RangeRead for Counted<'_> {
    fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        self.1.fetch_add(len, Ordering::Relaxed);
        self.0.read_range(offset, len)
    }
}

#[test]
fn levels_by_range() {
    let Some(dir) = std::env::var_os("SPECULAR_POINTS_COPC") else {
        eprintln!("SPECULAR_POINTS_COPC not set; skipping");
        return;
    };
    let limits = Limits {
        max_points: u64::MAX,
        ..Limits::default()
    };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".copc.laz"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let whole = read(PointFormat::Laz, &bytes, &limits).unwrap();
        // Everything.
        let src = Counted(&bytes, AtomicU64::new(0));
        let t0 = std::time::Instant::now();
        let all = fetch_levels(&src, u64::MAX, u64::MAX, &mut |_, _| {}).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        assert_eq!(all.points, all.total, "{}", path.display());
        let cloud = read(PointFormat::Laz, &all.image, &limits).unwrap();
        assert_eq!(cloud.len(), whole.len(), "{}", path.display());
        assert!(cloud.info.copc);
        let (a, b) = (cloud.bounds, whole.bounds);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        // The root level only.
        let mut seen = Vec::new();
        let root = fetch_levels(&src, 1, u64::MAX, &mut |l, n| seen.push((l, n))).unwrap();
        assert_eq!(root.levels, 1);
        assert_eq!(seen.len(), 1);
        let coarse = read(PointFormat::Laz, &root.image, &limits).unwrap();
        assert_eq!(coarse.len() as u64, root.points);
        eprintln!(
            "{}: {} levels, {} points; all levels {} B in {} requests ({ms:.1} ms), root {} points in {} B of {}",
            path.file_name().unwrap().to_string_lossy(),
            all.file_levels,
            all.total,
            all.bytes_read,
            all.requests,
            root.points,
            root.bytes_read,
            bytes.len()
        );
    }
}
