//! `memory_estimate` against the measured peak heap usage (a counting global allocator).

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

use common::*;
use specular_jpeg::*;

struct Counting;

static CURRENT: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

fn grow(n: u64) {
    let now = CURRENT.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

// Test-only: a global allocator can only be written with `unsafe` (see UNSAFE.md).
#[allow(unsafe_code)]
// SAFETY: every method forwards to `System` unchanged (same pointers, layouts and sizes) and
// only updates two counters, so `System`'s guarantees carry over.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded to the system allocator with the caller's layout.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            grow(l.size() as u64);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: as above.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            grow(l.size() as u64);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` was allocated by `System` with layout `l`.
        unsafe { System.dealloc(p, l) };
        CURRENT.fetch_sub(l.size() as u64, Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        // SAFETY: `p` was allocated by `System` with layout `l`; the caller upholds `new`.
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            if new >= l.size() {
                grow((new - l.size()) as u64);
            } else {
                CURRENT.fetch_sub((l.size() - new) as u64, Ordering::Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Peak heap growth while running `f` (everything `f` returns is still live at the end, so
/// it counts).
fn peak_of<T>(f: impl FnOnce() -> T) -> (T, u64) {
    let base = CURRENT.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let r = f();
    (r, PEAK.load(Ordering::Relaxed) - base)
}

fn check(what: &str, measured: u64, est: u64, bad: &mut Vec<String>) {
    check_range(what, measured, est, 0, bad);
}

/// As [`check`], but the estimate may also exceed the measured peak by `slack` (a worst case
/// that depends on timing).
fn check_range(what: &str, measured: u64, est: u64, slack: u64, bad: &mut Vec<String>) {
    let tol = (measured.max(est) / 10).max(512 << 10);
    let ok = measured <= est + tol && est <= measured + tol + slack;
    println!(
        "{what:<52} measured {:>9.2} MB  estimate {:>9.2} MB  {:+6.1}%{}",
        measured as f64 / 1e6,
        est as f64 / 1e6,
        (est as f64 / measured.max(1) as f64 - 1.0) * 100.0,
        if ok { "" } else { "  <-- off" }
    );
    if !ok {
        bad.push(what.to_string());
    }
}

/// Compare the estimates for `jpg` with measured peaks: index build and size, and for each
/// scale a full decode and two regions, with and without the index. Both estimates are checked,
/// [`memory_estimate_for`] (actual scan script) and [`memory_estimate`] (headers only; for its
/// index terms only if `standard`, i.e. the file uses the standard progression script).
fn measure(name: &str, jpg: &[u8], scales: &[Scale], standard: bool, bad: &mut Vec<String>) {
    let par = Parallelism::Auto;
    let info = read_info(jpg).unwrap();
    let (w, h) = (info.width, info.height);
    let o = |scale, region| DecodeOptions {
        scale,
        region,
        parallelism: par,
        cancel: None,
    };
    // Warm up (thread pool, lazily initialised tables).
    decode(jpg, &o(Scale::Eighth, None)).unwrap();

    let ests = |scale, region| {
        let mut v = vec![("", memory_estimate_for(jpg, scale, region).unwrap())];
        v.push((" (info)", memory_estimate(&info, scale, region)));
        v
    };
    let (ix, m) = peak_of(|| build_index_with(jpg, &o(Scale::Full, None)).unwrap());
    for (how, e) in ests(Scale::Full, None) {
        if how.is_empty() || standard {
            // The index build's peak depends on timing (how many bands a refinement scan's
            // speculation snapshots); the estimate is its worst case: a full snapshot of the
            // nonzero masks (8 bytes per block) per refinement scan, at most two per component
            // in the usual scripts, above the luckiest run. The upper bound stays strict.
            let slack = if info.coding == Coding::Progressive {
                w.div_ceil(8) as u64 * h.div_ceil(8) as u64 * 8 * 2 * info.components.len() as u64
            } else {
                0
            };
            check_range(
                &format!("{name} build_index{how}"),
                m,
                e.index_build_bytes,
                slack,
                bad,
            );
            check(
                &format!("{name} index size{how}"),
                ix.memory_bytes() as u64,
                e.index_bytes,
                bad,
            );
        }
    }
    for &scale in scales {
        let (ow, oh) = (w.div_ceil(scale as u32), h.div_ceil(scale as u32));
        let regions = [
            None,
            Some(Region {
                x: ow / 4,
                y: oh / 3,
                w: ow / 2,
                h: oh / 3,
            }),
            Some(Region {
                x: 0,
                y: oh * 3 / 4,
                w: ow,
                h: (oh / 8).max(1),
            }),
        ];
        for (ri, region) in regions.into_iter().enumerate() {
            let es = ests(scale, region);
            let tag = format!("{name} {scale:?} {}", ["full", "region", "stripe"][ri]);
            let (d, m) = peak_of(|| decode(jpg, &o(scale, region)).unwrap());
            let out: u64 = d.planes.iter().map(|p| p.data.capacity() as u64).sum();
            drop(d);
            let (_, mi) = peak_of(|| decode_with_index(jpg, &ix, &o(scale, region)).unwrap());
            for (how, e) in es {
                check(&format!("{tag} output{how}"), out, e.output_bytes, bad);
                check(&format!("{tag} decode{how}"), m, e.peak(), bad);
                check(
                    &format!("{tag} decode_with_index{how}"),
                    mi,
                    e.peak_with_index(),
                    bad,
                );
            }
        }
    }
}

#[test]
fn estimate_matches_measured_peak() {
    if !tools_available() {
        eprintln!("skipping: cjpeg/djpeg/magick not found");
        return;
    }
    let all = [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth];
    let mut bad = Vec::new();

    let (w, h) = (4000, 3000);
    let src = source_ppm(w, h, 77);
    let script = tmp_dir().join("multiscan.txt");
    std::fs::write(&script, "0: 0 63 0 0;\n1: 0 63 0 0;\n2: 0 63 0 0;\n").unwrap();
    let script = script.to_str().unwrap().to_string();
    let variants: Vec<(&str, Vec<&str>)> = vec![
        ("base420", vec!["-quality", "90", "-sample", "2x2"]),
        ("base444", vec!["-quality", "90", "-sample", "1x1"]),
        (
            "base420_rst",
            vec!["-quality", "90", "-sample", "2x2", "-restart", "4"],
        ),
        ("gray", vec!["-quality", "90", "-grayscale"]),
        (
            "prog420",
            vec!["-quality", "90", "-sample", "2x2", "-progressive"],
        ),
        (
            "prog444",
            vec!["-quality", "90", "-sample", "1x1", "-progressive"],
        ),
        (
            "proggray",
            vec!["-quality", "90", "-grayscale", "-progressive"],
        ),
        (
            "prog420_rst",
            vec![
                "-quality",
                "90",
                "-sample",
                "2x2",
                "-progressive",
                "-restart",
                "4",
            ],
        ),
        (
            "multiscan444",
            vec!["-quality", "90", "-sample", "1x1", "-scans", &script],
        ),
        (
            "arith420",
            vec!["-quality", "90", "-sample", "2x2", "-arithmetic"],
        ),
        (
            "arith444_rst",
            vec![
                "-quality",
                "90",
                "-sample",
                "1x1",
                "-restart",
                "4",
                "-arithmetic",
            ],
        ),
        (
            "arith_prog420",
            vec![
                "-quality",
                "90",
                "-sample",
                "2x2",
                "-progressive",
                "-arithmetic",
            ],
        ),
        (
            "arith_proggray",
            vec![
                "-quality",
                "90",
                "-grayscale",
                "-progressive",
                "-arithmetic",
            ],
        ),
    ];
    for (name, args) in &variants {
        let jpg = cjpeg(&src, &format!("mem_{name}_{w}x{h}"), args);
        measure(name, &jpg, &all, true, &mut bad);
    }

    // CMYK: planes are converted to one RGBA plane at the end.
    let p = parse_pnm(&std::fs::read(source_ppm(2000, 1500, 78)).unwrap());
    let cmyk: Vec<u8> = p
        .data
        .chunks_exact(3)
        .flat_map(|q| [255 - q[0], 255 - q[1], 255 - q[2], 0])
        .collect();
    let mut jpg = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut jpg, 90);
    e.set_progressive(true);
    e.encode(&cmyk, 2000, 1500, jpeg_encoder::ColorType::Cmyk)
        .unwrap();
    // jpeg_encoder's progressive script has no refinement scans.
    measure("cmyk_prog", &jpg, &all, false, &mut bad);

    // The owner's real photos (read in place, never copied) and big benchmark files, if present.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    // erlaufsee's script has no refinement scans (DC, 1-5 and 6-63 per component).
    for (f, standard) in [
        (
            "tools/corpus/real/erlaufsee-panorama-34mp-progressive-444.jpg",
            false,
        ),
        (
            "tools/corpus/real/goats-panorama-108mp-baseline-444.jpg",
            true,
        ),
        ("target/jpeg-bench/50mp-420-prog.jpg", true),
    ] {
        if let Ok(jpg) = std::fs::read(root.join(f)) {
            let name = f.rsplit('/').next().unwrap().split('-').next().unwrap();
            measure(
                name,
                &jpg,
                &[Scale::Full, Scale::Quarter],
                standard,
                &mut bad,
            );
        }
    }
    assert!(bad.is_empty(), "estimates off by more than 10%: {bad:#?}");
}
