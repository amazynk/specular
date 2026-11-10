//! Decode timing: `bench-jp2 file.jp2 [reps] [max_reduce]`.
//!
//! Prints the best-of-`reps` time per reduce level for specular-jp2 (all cores and one thread),
//! `hayro-jpeg2000` (full resolution of Part 1 streams only; it has no reduced or HTJ2K
//! decode) and, when found, OpenJPEG's `opj_decompress` (its own reported decode time,
//! excluding file output).

use specular_jp2::{DecodeOptions, Decoder, Parallelism};
use std::process::Command;
use std::time::Instant;

fn best(reps: usize, mut f: impl FnMut()) -> f64 {
    let mut b = f64::MAX;
    for _ in 0..reps {
        let t = Instant::now();
        f();
        b = b.min(t.elapsed().as_secs_f64() * 1e3);
    }
    b
}

fn opj_bin() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("SPECULAR_OPJ_DECOMPRESS") {
        return Some(p.into());
    }
    let local = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/agent-app/jp2-tools/build/bin/opj_decompress");
    local.is_file().then_some(local)
}

/// Best reported "decode time" of `opj_decompress` over `reps` runs.
fn opj(bin: &std::path::Path, file: &str, reduce: u8, threads: usize, reps: usize) -> Option<f64> {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/agent-app/jp2-samples/bench-opj-out.raw");
    let mut b = f64::MAX;
    for _ in 0..reps {
        let o = Command::new(bin)
            .args(["-i", file, "-o"])
            .arg(&out)
            .args(["-r", &reduce.to_string(), "-threads", &threads.to_string()])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&o.stdout);
        let ms: f64 = s
            .lines()
            .find_map(|l| l.strip_prefix("decode time: "))?
            .trim_end_matches(" ms")
            .trim()
            .parse()
            .ok()?;
        b = b.min(ms);
    }
    let _ = std::fs::remove_file(&out);
    Some(b)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = args.get(1).expect("usage: bench file [reps] [max_reduce]");
    let reps: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(5);
    let data = std::fs::read(file).unwrap();
    let dec = Decoder::new(&data).unwrap();
    let info = dec.info().clone();
    let max_r: u8 = args
        .get(3)
        .map(|s| s.parse().unwrap())
        .unwrap_or(3)
        .min(info.levels);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!(
        "{file}: {}x{} {} comps, {} levels, {} layers, {}, tiles {}x{}, {} threads",
        info.width,
        info.height,
        info.components.len(),
        info.levels,
        info.layers,
        if info.reversible { "5/3" } else { "9/7" },
        info.tiles_x,
        info.tiles_y,
        threads
    );
    let opj_bin = opj_bin();
    println!("reduce  size          ours(mt)  ours(1t)  opj(mt)  opj(1t)  hayro(full)");
    for r in 0..=max_r {
        let (w, h) = info.reduced_size(r);
        let run = |par| {
            let img = dec
                .decode(&DecodeOptions {
                    reduce: r,
                    parallelism: par,
                    ..Default::default()
                })
                .unwrap();
            assert!(!img.truncated);
            std::hint::black_box(img);
        };
        let mt = best(reps, || run(Parallelism::Threads(threads)));
        let st = best(reps.min(3), || run(Parallelism::Single));
        let (omt, ost) = match &opj_bin {
            Some(b) => (
                opj(b, file, r, threads, reps),
                opj(b, file, r, 1, reps.min(3)),
            ),
            None => (None, None),
        };
        // hayro-jpeg2000 has no HTJ2K decoder.
        let hay = if r == 0 && !info.htj2k {
            let s = hayro_jpeg2000::DecodeSettings::default();
            Some(best(reps.min(3), || {
                let img = hayro_jpeg2000::Image::new(&data, &s).unwrap();
                std::hint::black_box(img.decode().unwrap());
            }))
        } else {
            None
        };
        let f = |v: Option<f64>| v.map_or("-".to_string(), |v| format!("{v:.1}"));
        println!(
            "{r:>6}  {:<12}  {mt:>8.1}  {st:>8.1}  {:>7}  {:>7}  {:>11}",
            format!("{w}x{h}"),
            f(omt),
            f(ost),
            f(hay)
        );
    }
}
