//! specular-bench: decode-level and viewer-level benchmarks for Specular Stage 1.
//!
//! Decode level (default): times libjpeg-turbo (`djpeg`), zune-jpeg, jpeg-decoder and (with the
//! `specular` feature) specular-jpeg on every corpus file, full size and at the 3840x2160 viewport
//! scale, plus peak RSS and a robustness pass over broken / unsupported files.
//!
//! Viewer level (`--viewers`): launches installed viewers on the Xwayland display and measures
//! open -> first pixels and open -> stable image by polling the window contents.

mod decoders;
#[cfg(feature = "gate")]
mod gate;
#[cfg(feature = "gate")]
mod headline;
#[cfg(feature = "specular")]
mod specular_glue;
mod viewers;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use decoders::{Dec, run_child, viewport_denom};

pub struct Args {
    corpus: PathBuf,
    results: PathBuf,
    quick: bool,
    runs: usize,
    warmup: usize,
    cold: bool,
    only: Vec<String>,
    decs: Vec<Dec>,
    viewport: (u32, u32),
    memory: bool,
    robustness: bool,
    viewers: bool,
    viewer_runs: usize,
    viewer_list: Option<Vec<String>>,
    viewer_window: (u32, u32),
    decode: bool,
    max_load: Option<f64>,
}

fn usage() -> &'static str {
    "specular-bench [options]\n\
     \n\
     Decode-level benchmark (default):\n\
       --corpus DIR        corpus directory containing manifest.json (default tools/corpus/out)\n\
       --results DIR       where to write results (default <corpus>/bench)\n\
       --quick             CI mode: small-tier files only, 3 runs, no memory pass\n\
       --runs N            timed runs per decoder/file (default 7; median reported)\n\
       --warmup N          untimed warm-up runs (default 1)\n\
       --cold              drop the file from the page cache before every run (posix_fadvise)\n\
       --only S[,S..]      only files whose path contains one of the substrings\n\
       --decoders a,b      subset of: djpeg-full,djpeg-view,zune-full,jpegdec-full,jpegdec-view,\n\
                           specular-full,specular-full-1t,specular-view,specular-view-1t\n\
       --viewport WxH      viewport for the scaled decode (default 3840x2160)\n\
       --no-memory         skip the peak-RSS pass (one extra decode per decoder in a child)\n\
       --no-robustness     skip the broken/unsupported-file pass\n\
       --max-load L        before each file, wait (up to 15 min) until the 1-min load average < L\n\
     Viewer-level benchmark (opt-in; opens windows on DISPLAY):\n\
       --viewers           run the viewer benchmark instead of the decode benchmark\n\
       --viewer-runs N     runs per viewer and file (default 3)\n\
       --viewer-list a,b   subset of: specular,feh,sxiv,eog,loupe\n\
       --window WxH        window size requested from viewers that accept one (default 1920x1080)\n\
     Instruction-count gate (needs `--features gate`; driven by tools/bench/perf-gate.py):\n\
       gate list | gate gen DIR | gate run CASE FILE\n"
}

fn parse_wh(s: &str) -> Result<(u32, u32)> {
    let (a, b) = s.split_once('x').context("expected WxH")?;
    Ok((a.parse()?, b.parse()?))
}

fn parse_args() -> Result<Args> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus/out");
    let mut a = Args {
        corpus: root,
        results: PathBuf::new(),
        quick: false,
        runs: 7,
        warmup: 1,
        cold: false,
        only: Vec::new(),
        decs: Dec::all(),
        viewport: (3840, 2160),
        memory: true,
        robustness: true,
        viewers: false,
        viewer_runs: 3,
        viewer_list: None,
        viewer_window: (1920, 1080),
        decode: true,
        max_load: None,
    };
    let mut runs_set = false;
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        let mut val = || it.next().with_context(|| format!("{x} needs a value"));
        match x.as_str() {
            "--corpus" => a.corpus = PathBuf::from(val()?),
            "--results" => a.results = PathBuf::from(val()?),
            "--quick" => a.quick = true,
            "--runs" => {
                a.runs = val()?.parse()?;
                runs_set = true;
            }
            "--warmup" => a.warmup = val()?.parse()?,
            "--cold" => a.cold = true,
            "--only" => a.only = val()?.split(',').map(String::from).collect(),
            "--decoders" => {
                a.decs = val()?
                    .split(',')
                    .map(|n| Dec::from_name(n).with_context(|| format!("unknown decoder {n}")))
                    .collect::<Result<_>>()?;
                if !cfg!(feature = "specular") && a.decs.iter().any(|d| d.is_specular()) {
                    bail!("specular decoders need `--features specular`");
                }
            }
            "--max-load" => a.max_load = Some(val()?.parse()?),
            "--viewport" => a.viewport = parse_wh(&val()?)?,
            "--no-memory" => a.memory = false,
            "--no-robustness" => a.robustness = false,
            "--viewers" => {
                a.viewers = true;
                a.decode = false;
            }
            "--viewer-runs" => a.viewer_runs = val()?.parse()?,
            "--viewer-list" => a.viewer_list = Some(val()?.split(',').map(String::from).collect()),
            "--window" => a.viewer_window = parse_wh(&val()?)?,
            "-h" | "--help" => {
                print!("{}", usage());
                std::process::exit(0);
            }
            other => bail!("unknown argument {other}\n\n{}", usage()),
        }
    }
    if a.quick {
        if !runs_set {
            a.runs = 3;
        }
        a.memory = false;
    }
    if a.results.as_os_str().is_empty() {
        a.results = a.corpus.join("bench");
    }
    Ok(a)
}

#[derive(Clone, Debug)]
pub struct CorpusFile {
    pub path: String,
    pub abs: PathBuf,
    pub category: String,
    pub tier: String,
    pub w: u32,
    pub h: u32,
    pub mp: f64,
    pub desc: String,
    pub expected_error: bool,
    pub size: u64,
}

pub fn load_manifest(dir: &Path) -> Result<Vec<CorpusFile>> {
    let p = dir.join("manifest.json");
    let v: Value = serde_json::from_slice(
        &std::fs::read(&p)
            .with_context(|| format!("reading {} (run specular-corpus first)", p.display()))?,
    )?;
    let mut out = Vec::new();
    for f in v["files"].as_array().context("manifest has no files")? {
        let path = f["path"].as_str().unwrap_or_default().to_string();
        let mut desc = String::new();
        if let Some(s) = f["sof"].as_str() {
            desc.push_str(match s {
                "SOF0" => "baseline",
                "SOF1" => "ext-seq",
                "SOF2" => "progressive",
                "SOF9" => "arith",
                "SOF10" => "arith-prog",
                other => other,
            });
        }
        if let Some(s) = f["sampling"].as_str() {
            let cm = f["colour_model"].as_str().unwrap_or("");
            let s = if cm == "YCbCr" {
                s.to_string()
            } else {
                cm.to_string()
            };
            let _ = write!(desc, " {s}");
        }
        if f["restart_interval_mcus"].as_u64().unwrap_or(0) > 0 {
            desc.push_str(" rst");
        }
        out.push(CorpusFile {
            // canonical: GIO-based viewers (eog, loupe) resolve `..` lexically, which breaks
            // through symlinked directories
            abs: std::fs::canonicalize(dir.join(&path)).unwrap_or_else(|_| dir.join(&path)),
            category: f["category"]
                .as_str()
                .unwrap_or_else(|| path.split('/').next().unwrap_or(""))
                .to_string(),
            tier: f["tier"].as_str().unwrap_or("").to_string(),
            w: f["width"].as_u64().unwrap_or(0) as u32,
            h: f["height"].as_u64().unwrap_or(0) as u32,
            mp: f["megapixels"].as_f64().unwrap_or(0.0),
            desc,
            expected_error: f["expected_error"].as_bool().unwrap_or(false),
            size: f["file_size"].as_u64().unwrap_or(0),
            path,
        });
    }
    Ok(out)
}

#[derive(Serialize, Clone)]
struct DecodeResult {
    file: String,
    category: String,
    width: u32,
    height: u32,
    megapixels: f64,
    view_denom: u32,
    decoder: &'static str,
    median_ms: Option<f64>,
    min_ms: Option<f64>,
    max_ms: Option<f64>,
    samples_ms: Vec<f64>,
    out_w: u32,
    out_h: u32,
    loadavg_1m: f64,
    peak_rss_mb: Option<f64>,
    error: Option<String>,
    note: Option<String>,
}

#[derive(Serialize)]
struct RobustResult {
    file: String,
    decoder: &'static str,
    outcome: String,
    ms: f64,
    peak_rss_mb: f64,
    detail: Option<String>,
}

fn load1() -> f64 {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse().ok())
        .unwrap_or(-1.0)
}

fn wait_for_load(limit: f64) {
    let t = std::time::Instant::now();
    let mut said = false;
    while load1() >= limit && t.elapsed() < Duration::from_secs(900) {
        if !said {
            println!("  (waiting for load average < {limit}, now {:.1})", load1());
            said = true;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn main() -> Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(String::as_str) == Some("--decode-one") {
        return decode_one(&argv[2..]);
    }
    if argv.get(1).map(String::as_str) == Some("gate") {
        #[cfg(feature = "gate")]
        return gate::main(&argv[2..]);
        #[cfg(not(feature = "gate"))]
        bail!("the instruction-count gate needs `--features gate`");
    }
    let a = parse_args()?;
    if a.viewers {
        return viewers::run(&a);
    }
    if a.decode {
        decode_bench(&a)?;
    }
    Ok(())
}

/// Child-process mode used for peak-RSS and robustness runs (a panic or abort there cannot take
/// down the harness, and wait4 gives the child's peak RSS).
fn decode_one(args: &[String]) -> Result<()> {
    let dec = Dec::from_name(&args[0]).context("decoder")?;
    let path = PathBuf::from(&args[1]);
    let w: u32 = args[2].parse()?;
    let h: u32 = args[3].parse()?;
    let vp = parse_wh(&args[4])?;
    let res = decoders::run(dec, &path, w, h, vp);
    // Peak RSS: our own high-water mark, or djpeg's (the only waited-for child) for djpeg runs.
    // ru_maxrss of a process spawned from the big harness would include the harness's own RSS.
    let rss_kb = if matches!(dec, Dec::DjpegFull | Dec::DjpegView) {
        decoders::children_maxrss_kb()
    } else {
        decoders::self_hwm_kb()
    };
    match res {
        Ok(o) => {
            println!(
                "{}",
                serde_json::json!({"ok": true, "ms": ms(o.elapsed), "out_w": o.out_w, "out_h": o.out_h, "rss_kb": rss_kb})
            );
            Ok(())
        }
        Err(e) => {
            println!(
                "{}",
                serde_json::json!({"ok": false, "err": format!("{e:#}"), "rss_kb": rss_kb})
            );
            std::process::exit(1);
        }
    }
}

/// Peak RSS of a `--decode-one` child: what it reported itself, or wait4's value if it died.
fn child_rss_mb(c: &decoders::ChildResult) -> f64 {
    serde_json::from_str::<Value>(c.stdout.trim())
        .ok()
        .and_then(|v| v["rss_kb"].as_i64())
        .unwrap_or(c.max_rss_kb) as f64
        / 1024.0
}

fn child_cmd(dec: Dec, f: &CorpusFile, vp: (u32, u32)) -> Command {
    let mut c = Command::new(std::env::current_exe().expect("current exe"));
    c.arg("--decode-one")
        .arg(dec.name())
        .arg(&f.abs)
        .arg(f.w.to_string())
        .arg(f.h.to_string())
        .arg(format!("{}x{}", vp.0, vp.1));
    c
}

fn sysinfo() -> Value {
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        })
        .unwrap_or_default();
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let djv = Command::new("djpeg")
        .arg("-version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string())
        .unwrap_or_default();
    let date = Command::new("date")
        .arg("-Iseconds")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    serde_json::json!({
        "date": date,
        "cpu": cpu,
        "threads": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        "loadavg_at_start": load.trim(),
        "djpeg": djv,
        "zune_jpeg": "0.5 (crate)",
        "jpeg_decoder": "0.3 (crate, rayon feature)",
        "specular_jpeg": if cfg!(feature = "specular") { "enabled (path dependency)" } else { "not built (feature `specular` off)" },
    })
}

fn decode_bench(a: &Args) -> Result<()> {
    let files = load_manifest(&a.corpus)?;
    let want =
        |f: &CorpusFile| a.only.is_empty() || a.only.iter().any(|s| f.path.contains(s.as_str()));
    let bench_files: Vec<&CorpusFile> = files
        .iter()
        .filter(|f| {
            !f.expected_error
                && f.category != "broken"
                && f.category != "orientation"
                && f.category != "flip"
                && f.w > 0
        })
        .filter(|f| !a.quick || f.tier == "small")
        .filter(|f| f.abs.exists())
        .filter(|f| want(f))
        .collect();
    if bench_files.is_empty() {
        bail!("no corpus files selected; run specular-corpus first");
    }
    std::fs::create_dir_all(&a.results)?;
    let spawn = decoders::djpeg_spawn_overhead(&a.results)?;
    println!(
        "specular-bench: {} files, decoders [{}], runs={} warmup={} cold={} viewport={}x{}; djpeg spawn overhead {:.2} ms (subtracted)",
        bench_files.len(),
        a.decs
            .iter()
            .map(|d| d.name())
            .collect::<Vec<_>>()
            .join(","),
        a.runs,
        a.warmup,
        a.cold,
        a.viewport.0,
        a.viewport.1,
        ms(spawn)
    );
    let mut results: Vec<DecodeResult> = Vec::new();
    let mut cold_check = BTreeMap::new();
    for f in &bench_files {
        let denom = viewport_denom(f.w, f.h, a.viewport.0, a.viewport.1);
        if let Some(lim) = a.max_load {
            wait_for_load(lim);
        }
        if a.cold {
            decoders::drop_cache(&f.abs)?;
            cold_check.insert(
                f.path.clone(),
                decoders::resident_fraction(&f.abs).unwrap_or(-1.0),
            );
        }
        for &dec in &a.decs {
            let mut r = DecodeResult {
                file: f.path.clone(),
                category: f.category.clone(),
                width: f.w,
                height: f.h,
                megapixels: f.mp,
                view_denom: denom,
                decoder: dec.name(),
                median_ms: None,
                min_ms: None,
                max_ms: None,
                samples_ms: Vec::new(),
                out_w: 0,
                out_h: 0,
                loadavg_1m: load1(),
                peak_rss_mb: None,
                error: None,
                note: None,
            };
            let is_djpeg = matches!(dec, Dec::DjpegFull | Dec::DjpegView);
            for i in 0..a.warmup + a.runs {
                if a.cold {
                    decoders::drop_cache(&f.abs)?;
                }
                match decoders::run(dec, &f.abs, f.w, f.h, a.viewport) {
                    Ok(o) => {
                        if i >= a.warmup {
                            let mut t = ms(o.elapsed);
                            if is_djpeg {
                                t = (t - ms(spawn)).max(0.0);
                            }
                            r.samples_ms.push(t);
                        }
                        r.out_w = o.out_w;
                        r.out_h = o.out_h;
                        if o.note.is_some() {
                            r.note = o.note;
                        }
                    }
                    Err(e) => {
                        r.error = Some(format!("{e:#}"));
                        break;
                    }
                }
            }
            if r.error.is_none() && !r.samples_ms.is_empty() {
                let mut s = r.samples_ms.clone();
                r.median_ms = Some(median(&mut s));
                r.min_ms = s.first().copied();
                r.max_ms = s.last().copied();
            }
            if a.memory
                && r.error.is_none()
                && let Ok(c) = run_child(child_cmd(dec, f, a.viewport), Duration::from_secs(300))
                && c.status == Some(0)
            {
                r.peak_rss_mb = Some(child_rss_mb(&c));
            }
            println!(
                "  {:<52} {:<16} {}",
                f.path,
                dec.name(),
                match (&r.median_ms, &r.error) {
                    (Some(m), _) => format!(
                        "{m:>9.1} ms  (min {:.1}, max {:.1}) load {:.1}{}",
                        r.min_ms.unwrap(),
                        r.max_ms.unwrap(),
                        r.loadavg_1m,
                        r.peak_rss_mb
                            .map(|m| format!("  peak {m:.0} MB"))
                            .unwrap_or_default()
                    ),
                    (_, Some(e)) => format!("ERROR {e}"),
                    _ => "-".into(),
                }
            );
            results.push(r);
        }
    }
    let robust = if a.robustness {
        robustness(a, &files)?
    } else {
        Vec::new()
    };
    let md = markdown(a, &results, &robust, ms(spawn), &cold_check);
    let stamp = Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;
    let json = serde_json::json!({
        "system": sysinfo(),
        "config": {
            "runs": a.runs, "warmup": a.warmup, "cold": a.cold, "quick": a.quick,
            "viewport": [a.viewport.0, a.viewport.1],
            "djpeg_spawn_overhead_ms": ms(spawn),
            "timing": "wall time of read file + decode to RGB (zune, jpeg-decoder, djpeg->/dev/null) or to planes (specular); djpeg has the measured spawn overhead subtracted",
        },
        "cold_resident_fraction_after_drop": cold_check,
        "results": results,
        "robustness": robust,
    });
    let jp = a.results.join(format!("decode-{stamp}.json"));
    let mp = a.results.join(format!("decode-{stamp}.md"));
    std::fs::write(&jp, serde_json::to_string_pretty(&json)?)?;
    std::fs::write(&mp, &md)?;
    std::fs::write(
        a.results.join("decode-latest.json"),
        serde_json::to_string_pretty(&json)?,
    )?;
    std::fs::write(a.results.join("decode-latest.md"), &md)?;
    println!("\n{md}");
    println!("results: {}\n         {}", jp.display(), mp.display());
    Ok(())
}

fn robustness(a: &Args, files: &[CorpusFile]) -> Result<Vec<RobustResult>> {
    let sel: Vec<&CorpusFile> = files
        .iter()
        .filter(|f| f.expected_error && f.abs.exists())
        .filter(|f| a.only.is_empty() || a.only.iter().any(|s| f.path.contains(s.as_str())))
        .collect();
    let mut out = Vec::new();
    if sel.is_empty() {
        return Ok(out);
    }
    println!(
        "robustness pass: {} files x {} decoders (child processes, 30 s timeout)",
        sel.len(),
        a.decs.len()
    );
    for f in sel {
        for &dec in &a.decs {
            if matches!(
                dec,
                Dec::DjpegView | Dec::JdView | Dec::SpecView | Dec::SpecView1 | Dec::SpecFull1
            ) {
                continue; // one entry per decoder library is enough here
            }
            let c = run_child(child_cmd(dec, f, a.viewport), Duration::from_secs(30))?;
            let detail = serde_json::from_str::<Value>(c.stdout.trim())
                .ok()
                .and_then(|v| {
                    v["err"]
                        .as_str()
                        .map(|s| s.chars().take(120).collect::<String>())
                });
            let outcome = if c.timed_out {
                "TIMEOUT".to_string()
            } else if let Some(sig) = c.signal {
                format!("CRASH (signal {sig})")
            } else if detail
                .as_deref()
                .is_some_and(|d| d.contains("killed by signal"))
            {
                "CRASH (djpeg signal)".to_string()
            } else if c.status == Some(0) {
                "decoded".to_string()
            } else {
                "error (graceful)".to_string()
            };
            out.push(RobustResult {
                file: f.path.clone(),
                decoder: dec.name(),
                outcome,
                ms: ms(c.elapsed),
                peak_rss_mb: child_rss_mb(&c),
                detail,
            });
        }
    }
    Ok(out)
}

fn fmt_ms(v: Option<f64>) -> String {
    match v {
        Some(m) if m >= 100.0 => format!("{m:.0}"),
        Some(m) => format!("{m:.1}"),
        None => "–".into(),
    }
}

fn markdown(
    a: &Args,
    res: &[DecodeResult],
    robust: &[RobustResult],
    spawn_ms: f64,
    cold: &BTreeMap<String, f64>,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "## Decode-level benchmark\n");
    let _ = writeln!(
        s,
        "Median of {} runs after {} warm-up, {} page cache; times in ms = read file + decode. djpeg times have {:.2} ms process-spawn overhead subtracted. \
         \"view\" = DCT-scaled decode at the largest power-of-two reduction that still covers a {}x{} viewport at fit (column `1/N`).\n",
        a.runs,
        a.warmup,
        if a.cold {
            "cold (posix_fadvise DONTNEED before each run)"
        } else {
            "warm"
        },
        spawn_ms,
        a.viewport.0,
        a.viewport.1
    );
    if !res.is_empty() {
        let lo = res
            .iter()
            .map(|r| r.loadavg_1m)
            .fold(f64::INFINITY, f64::min);
        let hi = res.iter().map(|r| r.loadavg_1m).fold(0.0f64, f64::max);
        let _ = writeln!(
            s,
            "1-min load average during the run: {lo:.1}–{hi:.1} on {} hardware threads{}.\n",
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0),
            a.max_load
                .map(|l| format!(" (waited for < {l} before each file)"))
                .unwrap_or_default()
        );
    }
    let decs: Vec<&'static str> = a.decs.iter().map(|d| d.name()).collect();
    let mut by_file: Vec<(&str, Vec<&DecodeResult>)> = Vec::new();
    for r in res {
        match by_file.iter_mut().find(|(f, _)| *f == r.file) {
            Some((_, v)) => v.push(r),
            None => by_file.push((&r.file, vec![r])),
        }
    }
    let have_spec = a.decs.iter().any(|d| d.is_specular());
    let _ = write!(s, "| File | MP | 1/N |");
    for d in &decs {
        let _ = write!(s, " {d} |");
    }
    let _ = writeln!(
        s,
        " fastest full | fastest view |{}",
        if have_spec {
            " specular fastest? |"
        } else {
            ""
        }
    );
    let _ = write!(s, "|---|--:|--:|");
    for _ in &decs {
        let _ = write!(s, "--:|");
    }
    let _ = writeln!(s, "---|---|{}", if have_spec { "---|" } else { "" });
    let mut flagged = Vec::new();
    for (file, rows) in &by_file {
        let r0 = rows[0];
        let _ = write!(s, "| {} | {:.1} | {} |", file, r0.megapixels, r0.view_denom);
        for d in &decs {
            let r = rows.iter().find(|r| r.decoder == *d);
            let cell = match r {
                Some(r) if r.error.is_some() => "err".to_string(),
                Some(r) => fmt_ms(r.median_ms),
                None => "–".into(),
            };
            let _ = write!(s, " {cell} |");
        }
        let best = |view: bool, spec: Option<bool>| {
            rows.iter()
                .filter(|r| {
                    Dec::from_name(r.decoder).is_some_and(|d| {
                        d.is_view() == view && spec.is_none_or(|sp| d.is_specular() == sp)
                    })
                })
                .filter_map(|r| r.median_ms.map(|m| (m, r.decoder)))
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        };
        let bf = best(false, None);
        let bv = best(true, None);
        let _ = write!(
            s,
            " {} | {} |",
            bf.map(|(m, d)| format!("{d} {}", fmt_ms(Some(m))))
                .unwrap_or_default(),
            bv.map(|(m, d)| format!("{d} {}", fmt_ms(Some(m))))
                .unwrap_or_default()
        );
        if have_spec {
            let mut verdict = Vec::new();
            for view in [false, true] {
                let comp = best(view, Some(false));
                let sp = best(view, Some(true));
                match (sp, comp) {
                    (Some((sm, _)), Some((cm, cd))) if sm > cm => {
                        verdict.push(format!(
                            "**NO ({}: {:.0}% slower than {cd})**",
                            if view { "view" } else { "full" },
                            (sm / cm - 1.0) * 100.0
                        ));
                        flagged.push(format!("{file} ({})", if view { "view" } else { "full" }));
                    }
                    (None, Some(_)) => {
                        verdict.push(format!(
                            "**NO ({}: specular failed)**",
                            if view { "view" } else { "full" }
                        ));
                        flagged.push(format!(
                            "{file} ({}, failed)",
                            if view { "view" } else { "full" }
                        ));
                    }
                    _ => {}
                }
            }
            let _ = write!(
                s,
                " {} |",
                if verdict.is_empty() {
                    "yes".to_string()
                } else {
                    verdict.join(" ")
                }
            );
        }
        let _ = writeln!(s);
    }
    if have_spec {
        let _ = writeln!(
            s,
            "\n{}",
            if flagged.is_empty() {
                "specular-jpeg is the fastest in every row.".to_string()
            } else {
                format!(
                    "**specular-jpeg is NOT the fastest in {} case(s):** {}",
                    flagged.len(),
                    flagged.join(", ")
                )
            }
        );
    }
    // Errors/notes
    let errs: Vec<&DecodeResult> = res.iter().filter(|r| r.error.is_some()).collect();
    if !errs.is_empty() {
        let _ = writeln!(s, "\nErrors:\n");
        for r in errs {
            let _ = writeln!(
                s,
                "- {} / {}: {}",
                r.file,
                r.decoder,
                r.error.as_deref().unwrap_or("")
            );
        }
    }
    if res.iter().any(|r| r.peak_rss_mb.is_some()) {
        let _ = writeln!(s, "\n### Peak RSS (MB, single decode in a child process)\n");
        let _ = write!(s, "| File |");
        for d in &decs {
            let _ = write!(s, " {d} |");
        }
        let _ = writeln!(s);
        let _ = write!(s, "|---|");
        for _ in &decs {
            let _ = write!(s, "--:|");
        }
        let _ = writeln!(s);
        for (file, rows) in &by_file {
            let _ = write!(s, "| {file} |");
            for d in &decs {
                let v = rows
                    .iter()
                    .find(|r| r.decoder == *d)
                    .and_then(|r| r.peak_rss_mb);
                let _ = write!(
                    s,
                    " {} |",
                    v.map(|m| format!("{m:.0}")).unwrap_or_else(|| "–".into())
                );
            }
            let _ = writeln!(s);
        }
    }
    if !cold.is_empty() {
        let worst = cold.values().cloned().fold(0.0f64, f64::max);
        let _ = writeln!(
            s,
            "\nCold-cache check: max resident fraction right after posix_fadvise(DONTNEED) = {:.1}%",
            worst * 100.0
        );
    }
    if !robust.is_empty() {
        let _ = writeln!(
            s,
            "\n### Robustness (broken / unsupported files; child process, 30 s timeout)\n"
        );
        let rdecs: Vec<&str> = {
            let mut v: Vec<&str> = Vec::new();
            for r in robust {
                if !v.contains(&r.decoder) {
                    v.push(r.decoder);
                }
            }
            v
        };
        let _ = write!(s, "| File |");
        for d in &rdecs {
            let _ = write!(s, " {d} |");
        }
        let _ = writeln!(s);
        let _ = write!(s, "|---|");
        for _ in &rdecs {
            let _ = write!(s, "---|");
        }
        let _ = writeln!(s);
        let mut files: Vec<&str> = Vec::new();
        for r in robust {
            if !files.contains(&r.file.as_str()) {
                files.push(&r.file);
            }
        }
        for f in files {
            let _ = write!(s, "| {f} |");
            for d in &rdecs {
                let c = robust.iter().find(|r| r.file == f && r.decoder == *d);
                let _ = write!(
                    s,
                    " {} |",
                    c.map(|r| format!("{} ({:.0} MB)", r.outcome, r.peak_rss_mb))
                        .unwrap_or_default()
                );
            }
            let _ = writeln!(s);
        }
        let crashes: Vec<String> = robust
            .iter()
            .filter(|r| r.outcome.starts_with("CRASH") || r.outcome == "TIMEOUT")
            .map(|r| format!("{} / {}", r.file, r.decoder))
            .collect();
        if !crashes.is_empty() {
            let _ = writeln!(s, "\n**Crashes/timeouts:** {}", crashes.join(", "));
        }
    }
    s
}
