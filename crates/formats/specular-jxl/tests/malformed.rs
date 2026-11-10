//! Truncated and corrupted inputs must give `Err` (or a partial render), never a panic.
//!
//! The library contains panics with `catch_unwind` as a last resort, but the app is built with
//! `panic = "abort"`, so this test counts *every* panic (caught or not) through a panic hook.

use specular_jxl::{Error, JxlFile};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static PANICS: AtomicUsize = AtomicUsize::new(0);
static FIRST: Mutex<Option<String>> = Mutex::new(None);

fn install_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            PANICS.fetch_add(1, Ordering::SeqCst);
            let mut f = FIRST.lock().unwrap_or_else(|e| e.into_inner());
            if f.is_none() {
                let bt = std::backtrace::Backtrace::force_capture();
                *f = Some(format!("{info}\n{bt}"));
            }
        }));
    });
}

const FIXTURES: &[&str] = &[
    "vardct.jxl",
    "vardct_multigroup.jxl",
    "modular.jxl",
    "modular_lossy.jxl",
    "alpha.jxl",
    "alpha_lossless.jxl",
    "gray.jxl",
    "rgb16.jxl",
    "anim3.jxl",
];

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Exercise every entry point on `data`; results are ignored, only panics matter.
fn exercise(data: &[u8]) {
    let Ok(f) = JxlFile::open(data) else {
        return;
    };
    let _ = f.decode(1, false);
    let _ = f.decode(4, false);
    let _ = f.decode_dc(false);
    if let Ok(frames) = f.frames(2, false) {
        let _ = frames.len();
        for fr in frames.take(4) {
            let _ = fr;
        }
    }
}

fn check_panics(what: &str) {
    let n = PANICS.load(Ordering::SeqCst);
    let first = FIRST.lock().unwrap_or_else(|e| e.into_inner()).clone();
    assert_eq!(
        n,
        0,
        "{what}: {n} panic(s); first:\n{}",
        first.unwrap_or_default()
    );
}

#[test]
fn truncated() {
    install_hook();
    for name in FIXTURES {
        let data = fixture(name);
        let step = (data.len() / 150).max(1);
        let mut partial = 0;
        for len in (0..data.len()).step_by(step) {
            let cut = &data[..len];
            if let Ok(f) = JxlFile::open(cut) {
                match f.decode(1, false) {
                    Ok(d) => {
                        assert!(!d.complete || len + 1 >= data.len() || f.info().animated);
                        partial += 1;
                    }
                    Err(Error::Truncated(_) | Error::Malformed(_)) => {}
                    Err(e) => panic!("{name} cut at {len}: unexpected {e}"),
                }
            }
            exercise(cut);
        }
        eprintln!("{name}: {partial} partial renders");
    }
    check_panics("truncated");
}

#[test]
fn corrupted() {
    install_hook();
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for name in FIXTURES {
        let data = fixture(name);
        for _ in 0..120 {
            let mut d = data.clone();
            let flips = 1 + next() % 4;
            for _ in 0..flips {
                // Mostly the headers and the first groups, where the structure lives.
                let span = if next() % 2 == 0 {
                    d.len().min(256)
                } else {
                    d.len()
                };
                let i = (next() as usize) % span;
                d[i] ^= 1 << (next() % 8);
            }
            exercise(&d);
        }
    }
    check_panics("corrupted");
}

#[test]
fn garbage() {
    install_hook();
    assert!(matches!(
        JxlFile::open(&b"not a jxl"[..]),
        Err(Error::NotJxl)
    ));
    assert!(JxlFile::open(&[0xffu8, 0x0a][..]).is_err());
    let mut d = vec![0xffu8, 0x0a];
    d.extend(std::iter::repeat_n(0xff, 64));
    exercise(&d);
    let mut d = vec![0xffu8, 0x0a];
    d.extend(std::iter::repeat_n(0x00, 64));
    exercise(&d);
    check_panics("garbage");
}
