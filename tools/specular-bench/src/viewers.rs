//! Viewer-level benchmark: launch an image viewer on the X display (Xwayland under GNOME), find
//! its window, and poll the window contents with core-protocol GetImage to time
//!   * first pixels: the first frame whose centre region is no longer a flat background, and
//!   * sharp:        the first frame identical to the final, stable frame.
//!
//! Specular reports both numbers itself via `--bench-open <file>` (one JSON line on stdout), which
//! is more precise than screen polling, so it is run that way when its binary exists.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConfigureWindowAux, ConnectionExt, ImageFormat, Window};
use x11rb::rust_connection::RustConnection;

use crate::{Args, CorpusFile, load_manifest};

/// Size of the centre region that is captured on each poll.
const REGION: (u16, u16) = (320, 160);
/// Fraction of region pixels that must differ from the most common colour to count as content.
const CONTENT_FRACTION: f64 = 0.2;
/// The frame must stay unchanged this long to count as final.
const STABLE_FOR: Duration = Duration::from_millis(1500);
const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize, Clone)]
struct ViewerRun {
    viewer: String,
    file: String,
    megapixels: f64,
    run: usize,
    window_ms: Option<f64>,
    first_pixels_ms: Option<f64>,
    sharp_ms: Option<f64>,
    error: Option<String>,
    /// Specular only: the full `--bench-open` report (scale, per-level events, RSS, GPU, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<serde_json::Value>,
}

struct Viewer {
    name: &'static str,
    /// argv before the file name
    argv: Vec<String>,
    /// resize the window ourselves once it appears (viewer has no geometry option)
    resize: bool,
}

fn have(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null")])
        .status()
        .is_ok_and(|s| s.success())
}

fn running(name: &str) -> bool {
    Command::new("pgrep")
        .args(["-x", name])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn specular_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SPECULAR_BIN") {
        return Some(PathBuf::from(p));
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cands = vec![root.join("target/release/specular")];
    if let Ok(t) = std::env::var("CARGO_TARGET_DIR") {
        cands.push(PathBuf::from(t).join("release/specular"));
    }
    cands.into_iter().find(|p| p.exists())
}

fn atom(c: &RustConnection, name: &str) -> Result<u32> {
    Ok(c.intern_atom(false, name.as_bytes())?.reply()?.atom)
}

fn client_list(c: &RustConnection, root: Window, a: u32) -> Vec<Window> {
    c.get_property(false, root, a, AtomEnum::WINDOW, 0, 4096)
        .ok()
        .and_then(|r| r.reply().ok())
        .and_then(|r| r.value32().map(|v| v.collect()))
        .unwrap_or_default()
}

fn window_pid(c: &RustConnection, w: Window, a: u32) -> Option<u32> {
    c.get_property(false, w, a, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
}

/// Capture the centre region of `w`; returns (hash, non-mode fraction).
fn capture(c: &RustConnection, w: Window) -> Option<(u64, f64)> {
    let g = c.get_geometry(w).ok()?.reply().ok()?;
    let rw = REGION.0.min(g.width);
    let rh = REGION.1.min(g.height);
    let x = ((g.width - rw) / 2) as i16;
    let y = ((g.height - rh) / 2) as i16;
    let img = c
        .get_image(ImageFormat::Z_PIXMAP, w, x, y, rw, rh, !0)
        .ok()?
        .reply()
        .ok()?;
    let px: Vec<u32> = img
        .data
        .chunks_exact(4)
        .map(|p| u32::from_le_bytes([p[0], p[1], p[2], 0]))
        .collect();
    if px.is_empty() {
        return None;
    }
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for &p in &px {
        *counts.entry(p).or_default() += 1;
    }
    let mode = counts.values().copied().max().unwrap_or(0);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    px.hash(&mut h);
    Some((h.finish(), 1.0 - mode as f64 / px.len() as f64))
}

fn kill(child: &mut Child, extra_pid: Option<u32>) {
    // SIGTERM first (the viewer and, if it forked, its real process), SIGKILL after 2 s.
    #[cfg(unix)]
    {
        use rustix::process::{Pid, Signal, kill_process};
        for p in [Some(child.id()), extra_pid.filter(|&p| p != child.id())]
            .into_iter()
            .flatten()
        {
            if let Some(pid) = i32::try_from(p).ok().and_then(Pid::from_raw) {
                let _ = kill_process(pid, Signal::TERM);
            }
        }
    }
    #[cfg(not(unix))]
    let _ = extra_pid;
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(2) {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn x11_env(cmd: &mut Command, display: &str) {
    cmd.env("DISPLAY", display)
        .env("GDK_BACKEND", "x11")
        .env("WINIT_UNIX_BACKEND", "x11")
        .env_remove("WAYLAND_DISPLAY")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
}

fn measure_x11(
    c: &RustConnection,
    root: Window,
    atoms: (u32, u32),
    v: &Viewer,
    f: &CorpusFile,
    display: &str,
    win: (u32, u32),
) -> Result<ViewerRun> {
    let before: Vec<Window> = client_list(c, root, atoms.0);
    let mut cmd = Command::new(&v.argv[0]);
    cmd.args(&v.argv[1..]).arg(&f.abs);
    x11_env(&mut cmd, display);
    let t0 = Instant::now();
    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning {}", v.name))?;
    let pid = child.id();
    let mut run = ViewerRun {
        viewer: v.name.into(),
        file: f.path.clone(),
        megapixels: f.mp,
        run: 0,
        window_ms: None,
        first_pixels_ms: None,
        sharp_ms: None,
        error: None,
        report: None,
    };
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    // 1. find the window: a new managed client whose _NET_WM_PID is ours (or any new client)
    let mut window = None;
    while t0.elapsed() < TIMEOUT {
        if let Ok(Some(st)) = child.try_wait() {
            run.error = Some(format!("viewer exited early ({st})"));
            return Ok(run);
        }
        let now = client_list(c, root, atoms.0);
        let new: Vec<Window> = now.into_iter().filter(|w| !before.contains(w)).collect();
        if let Some(&w) = new
            .iter()
            .find(|&&w| window_pid(c, w, atoms.1) == Some(pid))
            .or(new.first())
        {
            window = Some(w);
            run.window_ms = Some(ms(t0.elapsed()));
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let Some(w) = window else {
        kill(&mut child, None);
        run.error = Some("no window appeared".into());
        return Ok(run);
    };
    let wpid = window_pid(c, w, atoms.1);
    if v.resize {
        let _ = c.configure_window(w, &ConfigureWindowAux::new().width(win.0).height(win.1));
        let _ = c.flush();
    }
    // 2. poll frames until the image has been stable for STABLE_FOR
    let mut frames: Vec<(Duration, u64, f64)> = Vec::new();
    let mut last_change = t0.elapsed();
    while t0.elapsed() < TIMEOUT {
        if let Some((h, frac)) = capture(c, w) {
            let t = t0.elapsed();
            if frames.last().is_none_or(|l| l.1 != h) {
                last_change = t;
            }
            frames.push((t, h, frac));
            if run.first_pixels_ms.is_none() && frac > CONTENT_FRACTION {
                run.first_pixels_ms = Some(ms(t));
            }
            if run.first_pixels_ms.is_some() && t - last_change > STABLE_FOR {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    kill(&mut child, wpid);
    match frames.last() {
        Some(&(_, fin, frac)) if frac > CONTENT_FRACTION => {
            // first frame of the final run of identical frames
            let mut i = frames.len() - 1;
            while i > 0 && frames[i - 1].1 == fin {
                i -= 1;
            }
            run.sharp_ms = Some(ms(frames[i].0));
            if t0.elapsed() >= TIMEOUT {
                run.error = Some("did not stabilise before timeout".into());
            }
        }
        Some(_) => run.error = Some("centre region never showed image content".into()),
        None => run.error = Some("could not capture the window (GetImage failed)".into()),
    }
    Ok(run)
}

fn measure_specular(
    bin: &PathBuf,
    f: &CorpusFile,
    display: &str,
    win: (u32, u32),
) -> Result<ViewerRun> {
    let mut cmd = Command::new(bin);
    cmd.arg("--size")
        .arg(format!("{}x{}", win.0, win.1))
        .arg("--bench-open")
        .arg(&f.abs);
    x11_env(&mut cmd, display);
    cmd.stdout(Stdio::piped());
    let c = crate::decoders::run_child(cmd, TIMEOUT)?;
    let mut run = ViewerRun {
        viewer: "specular".into(),
        file: f.path.clone(),
        megapixels: f.mp,
        run: 0,
        window_ms: None,
        first_pixels_ms: None,
        sharp_ms: None,
        error: None,
        report: None,
    };
    // the report is a (pretty-printed) JSON object: take the first '{' to the last '}'
    let json = match (c.stdout.find('{'), c.stdout.rfind('}')) {
        (Some(i), Some(j)) if j > i => &c.stdout[i..=j],
        _ => "",
    };
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(v) => {
            run.first_pixels_ms = v["first_pixels_ms"].as_f64();
            run.sharp_ms = v["sharp_ms"].as_f64();
            run.report = Some(v);
        }
        Err(_) => {
            run.error = Some(if c.timed_out {
                "timed out".into()
            } else {
                format!(
                    "no JSON from --bench-open (status {:?}, signal {:?})",
                    c.status, c.signal
                )
            })
        }
    }
    Ok(run)
}

pub fn run(a: &Args) -> Result<()> {
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
    let files = load_manifest(&a.corpus)?;
    let default_set = if a.quick {
        vec!["photo/24mp-420"]
    } else {
        vec![
            "photo/24mp-420",
            "photo/100mp-norst",
            "photo/200mp",
            "erlaufsee",
            "goats",
        ]
    };
    let sel: Vec<&CorpusFile> = files
        .iter()
        .filter(|f| f.abs.exists() && !f.expected_error)
        .filter(|f| {
            if a.only.is_empty() {
                default_set.iter().any(|s| f.path.contains(s))
            } else {
                a.only.iter().any(|s| f.path.contains(s.as_str()))
            }
        })
        .collect();
    if sel.is_empty() {
        bail!("no files selected");
    }
    let geo = format!("{}x{}", a.viewer_window.0, a.viewer_window.1);
    let mut skipped: Vec<String> = Vec::new();
    let mut viewers: Vec<Viewer> = Vec::new();
    let wanted = |n: &str| {
        a.viewer_list
            .as_ref()
            .is_none_or(|l| l.iter().any(|x| x == n))
    };
    if wanted("feh") {
        if have("feh") {
            viewers.push(Viewer {
                name: "feh",
                argv: vec![
                    "feh".into(),
                    "-B".into(),
                    "black".into(),
                    "-g".into(),
                    geo.clone(),
                    "--scale-down".into(),
                ],
                resize: false,
            });
        } else {
            skipped.push("feh: not installed".into());
        }
    }
    if wanted("sxiv") {
        if have("sxiv") {
            viewers.push(Viewer {
                name: "sxiv",
                argv: vec!["sxiv".into(), "-g".into(), geo.clone()],
                resize: false,
            });
        } else {
            skipped.push("sxiv: not installed".into());
        }
    }
    if wanted("eog") {
        if !have("eog") {
            skipped.push("eog: not installed".into());
        } else {
            viewers.push(Viewer {
                name: "eog",
                argv: vec!["eog".into(), "-n".into()],
                resize: true,
            });
        }
    }
    if wanted("loupe") {
        if !have("loupe") {
            skipped.push("loupe: not installed".into());
        } else if running("loupe") {
            skipped.push("loupe: an instance is already running (GApplication would hand the file to it; not killing the user's window)".into());
        } else {
            viewers.push(Viewer {
                name: "loupe",
                argv: vec!["loupe".into()],
                resize: true,
            });
        }
    }
    let spec = if wanted("specular") {
        let b = specular_bin();
        if b.is_none() {
            skipped.push(
                "specular: target/release/specular not built (set SPECULAR_BIN to override)".into(),
            );
        }
        b
    } else {
        None
    };

    let (conn, screen) = x11rb::connect(Some(&display))
        .with_context(|| format!("connecting to X display {display}"))?;
    let root = conn.setup().roots[screen].root;
    let atoms = (
        atom(&conn, "_NET_CLIENT_LIST")?,
        atom(&conn, "_NET_WM_PID")?,
    );
    let has_list = conn
        .get_property(false, root, atoms.0, AtomEnum::ANY, 0, 0)
        .ok()
        .and_then(|r| r.reply().ok())
        .is_some_and(|r| r.type_ != 0);
    if !has_list {
        skipped.push("warning: root window has no _NET_CLIENT_LIST; no EWMH window manager on this display, windows will not be found".into());
    }
    println!(
        "viewer benchmark on {display}: {} files x [{}{}] x {} runs",
        sel.len(),
        viewers.iter().map(|v| v.name).collect::<Vec<_>>().join(","),
        if spec.is_some() { ",specular" } else { "" },
        a.viewer_runs
    );
    for s in &skipped {
        println!("  skipped {s}");
    }
    let mut runs: Vec<ViewerRun> = Vec::new();
    for f in &sel {
        if let Some(lim) = a.max_load {
            crate::wait_for_load(lim);
        }
        for r in 0..a.viewer_runs {
            if let Some(b) = &spec {
                let mut x = measure_specular(b, f, &display, a.viewer_window)?;
                x.run = r;
                report(&x);
                runs.push(x);
            }
            for v in &viewers {
                let mut x = measure_x11(&conn, root, atoms, v, f, &display, a.viewer_window)?;
                x.run = r;
                report(&x);
                runs.push(x);
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    }
    let md = markdown(&runs, &skipped, a);
    std::fs::create_dir_all(&a.results)?;
    let stamp = Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;
    let json = serde_json::json!({
        "display": display,
        "method": "x11 window polling (GetImage of a centred 320x160 region every ~5 ms); specular self-reports via --bench-open",
        "content_fraction_threshold": CONTENT_FRACTION,
        "stable_for_ms": STABLE_FOR.as_millis() as u64,
        "skipped": skipped,
        "runs": runs,
    });
    std::fs::write(
        a.results.join(format!("viewers-{stamp}.json")),
        serde_json::to_string_pretty(&json)?,
    )?;
    std::fs::write(a.results.join(format!("viewers-{stamp}.md")), &md)?;
    std::fs::write(a.results.join("viewers-latest.md"), &md)?;
    println!("\n{md}");
    Ok(())
}

fn report(x: &ViewerRun) {
    let f = |v: Option<f64>| v.map(|m| format!("{m:.0}")).unwrap_or_else(|| "-".into());
    println!(
        "  {:<8} {:<48} run {}  window {:>6}  first {:>6}  sharp {:>6} ms {}",
        x.viewer,
        x.file,
        x.run,
        f(x.window_ms),
        f(x.first_pixels_ms),
        f(x.sharp_ms),
        x.error.as_deref().unwrap_or("")
    );
}

fn markdown(runs: &[ViewerRun], skipped: &[String], a: &Args) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "## Viewer-level benchmark\n\nMedian of {} runs, ms from process spawn. first = centre region shows image content; sharp = first frame identical to the final frame (stable for {} ms). Window {}x{} requested.\n",
        a.viewer_runs,
        STABLE_FOR.as_millis(),
        a.viewer_window.0,
        a.viewer_window.1
    );
    let _ = writeln!(
        s,
        "| File | MP | Viewer | window | first pixels | sharp | ok runs |"
    );
    let _ = writeln!(s, "|---|--:|---|--:|--:|--:|--:|");
    let mut keys: Vec<(&str, &str)> = Vec::new();
    for r in runs {
        if !keys.contains(&(r.file.as_str(), r.viewer.as_str())) {
            keys.push((&r.file, &r.viewer));
        }
    }
    let med = |mut v: Vec<f64>| -> String {
        if v.is_empty() {
            return "–".into();
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        format!("{:.0}", v[v.len() / 2])
    };
    for (file, viewer) in keys {
        let rs: Vec<&ViewerRun> = runs
            .iter()
            .filter(|r| r.file == file && r.viewer == viewer)
            .collect();
        let ok: Vec<&&ViewerRun> = rs.iter().filter(|r| r.error.is_none()).collect();
        let _ = writeln!(
            s,
            "| {file} | {:.1} | {viewer} | {} | {} | {} | {}/{} |",
            rs[0].megapixels,
            med(ok.iter().filter_map(|r| r.window_ms).collect()),
            med(ok.iter().filter_map(|r| r.first_pixels_ms).collect()),
            med(ok.iter().filter_map(|r| r.sharp_ms).collect()),
            ok.len(),
            rs.len()
        );
    }
    let errs: Vec<String> = runs
        .iter()
        .filter_map(|r| {
            r.error
                .as_ref()
                .map(|e| format!("{} / {} run {}: {e}", r.viewer, r.file, r.run))
        })
        .collect();
    if !errs.is_empty() {
        let _ = writeln!(s, "\nFailed runs:\n");
        for e in errs {
            let _ = writeln!(s, "- {e}");
        }
    }
    if !skipped.is_empty() {
        let _ = writeln!(s, "\nSkipped:\n");
        for e in skipped {
            let _ = writeln!(s, "- {e}");
        }
    }
    s
}
