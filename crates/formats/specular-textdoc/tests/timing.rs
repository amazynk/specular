//! Timings (ignored by default): opening a 100 MB log and scrolling a table of a million
//! rows. Run with `cargo test -p specular-textdoc --test timing -- --ignored --nocapture`.

use std::sync::Arc;
use std::time::Instant;

use specular_textdoc::{TableFile, TextFile};

#[test]
#[ignore = "timing"]
fn open_100_mb_log() {
    let mut log = String::with_capacity(105 << 20);
    let mut i = 0u64;
    while log.len() < 100 << 20 {
        log.push_str(&format!(
            "2026-11-07 10:{:02}:{:02}.{:03} INFO  worker-{}: processed request {} in {} ms\n",
            i / 60 % 60,
            i % 60,
            i % 1000,
            i % 16,
            i,
            i * 7 % 900
        ));
        i += 1;
    }
    let bytes: Arc<Vec<u8>> = Arc::new(log.into_bytes());
    let t = Instant::now();
    let f = TextFile::open(bytes, false, &|| false).unwrap();
    let open = t.elapsed();
    let t = Instant::now();
    let mut n = 0;
    for k in 0..1000u64 {
        n += f.lines(k * (f.index.len() / 1000), 60).len();
    }
    let pages = t.elapsed();
    eprintln!(
        "100 MB log: {} lines, open {:.0} ms; 1000 screens of 60 lines anywhere: {:.1} ms ({:.3} ms each), {n} lines",
        f.index.len(),
        open.as_secs_f64() * 1e3,
        pages.as_secs_f64() * 1e3,
        pages.as_secs_f64() * 1e3 / 1000.0
    );
}

#[test]
#[ignore = "timing"]
fn scroll_million_row_csv() {
    let mut csv = String::with_capacity(80 << 20);
    csv.push_str("id,name,city,amount,note\n");
    for i in 0..1_000_000u64 {
        csv.push_str(&format!(
            "{i},Name {},\"City, {}\",{}.{:02},\"said \"\"hi\"\"\"\n",
            i % 977,
            i % 131,
            i * 37 % 100_000,
            i % 100
        ));
    }
    let mb = csv.len() as f64 / (1 << 20) as f64;
    let bytes: Arc<Vec<u8>> = Arc::new(csv.into_bytes());
    let t = Instant::now();
    let f = TableFile::open(bytes, None, &|| false).unwrap();
    let open = t.elapsed();
    assert_eq!(f.table.len(), 1_000_001);
    // Scrolling: a screen of 40 rows a step, through the whole table, then jumps.
    let t = Instant::now();
    let mut top = 0u64;
    let mut steps = 0;
    while top < f.table.len() {
        let rows = f.rows(top, 40);
        assert!(!rows.is_empty());
        top += 997;
        steps += 1;
    }
    let scroll = t.elapsed();
    eprintln!(
        "1M-row CSV ({mb:.0} MB): open {:.0} ms; {steps} screens of 40 rows: {:.3} ms each",
        open.as_secs_f64() * 1e3,
        scroll.as_secs_f64() * 1e3 / f64::from(steps)
    );
}
