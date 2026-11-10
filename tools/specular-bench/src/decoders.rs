//! Decoder runners. Every in-process runner reads the file and decodes it (both inside the timed
//! region) so the numbers are comparable with `djpeg`, which also reads the file itself.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
pub enum Dec {
    /// libjpeg-turbo `djpeg -dct int`, full size, spawn overhead subtracted.
    DjpegFull,
    /// libjpeg-turbo `djpeg -dct int -scale 1/N` at the viewport scale.
    DjpegView,
    /// zune-jpeg 0.5, full size (it has no DCT scaling; single-threaded).
    Zune,
    /// jpeg-decoder 0.3, full size (rayon feature on).
    JdFull,
    /// jpeg-decoder 0.3 with `scale()` to the viewport scale.
    JdView,
    /// specular-jpeg, full size, Parallelism::Auto.
    SpecFull,
    /// specular-jpeg, full size, single thread.
    SpecFull1,
    /// specular-jpeg at `choose_scale(viewport)`, Parallelism::Auto.
    SpecView,
    /// specular-jpeg at `choose_scale(viewport)`, single thread.
    SpecView1,
}

impl Dec {
    pub fn all() -> Vec<Dec> {
        let mut v = vec![
            Dec::DjpegFull,
            Dec::DjpegView,
            Dec::Zune,
            Dec::JdFull,
            Dec::JdView,
        ];
        if cfg!(feature = "specular") {
            v.extend([Dec::SpecFull, Dec::SpecFull1, Dec::SpecView, Dec::SpecView1]);
        }
        v
    }
    pub fn name(self) -> &'static str {
        match self {
            Dec::DjpegFull => "djpeg-full",
            Dec::DjpegView => "djpeg-view",
            Dec::Zune => "zune-full",
            Dec::JdFull => "jpegdec-full",
            Dec::JdView => "jpegdec-view",
            Dec::SpecFull => "specular-full",
            Dec::SpecFull1 => "specular-full-1t",
            Dec::SpecView => "specular-view",
            Dec::SpecView1 => "specular-view-1t",
        }
    }
    pub fn from_name(s: &str) -> Option<Dec> {
        [
            Dec::DjpegFull,
            Dec::DjpegView,
            Dec::Zune,
            Dec::JdFull,
            Dec::JdView,
            Dec::SpecFull,
            Dec::SpecFull1,
            Dec::SpecView,
            Dec::SpecView1,
        ]
        .into_iter()
        .find(|d| d.name() == s)
    }
    pub fn is_view(self) -> bool {
        matches!(
            self,
            Dec::DjpegView | Dec::JdView | Dec::SpecView | Dec::SpecView1
        )
    }
    pub fn is_specular(self) -> bool {
        matches!(
            self,
            Dec::SpecFull | Dec::SpecFull1 | Dec::SpecView | Dec::SpecView1
        )
    }
}

/// Smallest power-of-two DCT scale denominator (1, 2, 4, 8) whose output still covers the
/// image fitted into the viewport, i.e. the cheapest decode that is still screen-sharp at "fit".
pub fn viewport_denom(w: u32, h: u32, vw: u32, vh: u32) -> u32 {
    let fit = (vw as f64 / w as f64).min(vh as f64 / h as f64).min(1.0);
    let (fw, fh) = (
        (w as f64 * fit).floor() as u32,
        (h as f64 * fit).floor() as u32,
    );
    let mut best = 1;
    for d in [2, 4, 8] {
        if w.div_ceil(d) >= fw && h.div_ceil(d) >= fh {
            best = d;
        }
    }
    best
}

pub struct Outcome {
    pub elapsed: Duration,
    pub out_w: u32,
    pub out_h: u32,
    pub note: Option<String>,
}

pub fn djpeg(path: &Path, denom: u32) -> Result<Outcome> {
    let mut c = Command::new("djpeg");
    c.args(["-dct", "int"]);
    if denom > 1 {
        c.arg("-scale").arg(format!("1/{denom}"));
    }
    c.args(["-outfile", "/dev/null"])
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let t = Instant::now();
    let out = c.output().context("spawning djpeg")?;
    let elapsed = t.elapsed();
    if !out.status.success() {
        if let Some(sig) = exit_signal(&out.status) {
            bail!("djpeg killed by signal {sig}");
        }
        bail!("djpeg: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let warn = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Ok(Outcome {
        elapsed,
        out_w: 0,
        out_h: 0,
        note: (!warn.is_empty()).then_some(warn),
    })
}

pub fn zune(path: &Path) -> Result<Outcome> {
    use zune_core::bytestream::ZCursor;
    use zune_core::options::DecoderOptions;
    let t = Instant::now();
    let data = std::fs::read(path)?;
    let opts = DecoderOptions::default()
        .set_max_width(65535)
        .set_max_height(65535);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(&data[..]), opts);
    let px = d
        .decode()
        .map_err(|e| anyhow::anyhow!("zune-jpeg: {e:?}"))?;
    let elapsed = t.elapsed();
    let (w, h) = d
        .info()
        .map(|i| (i.width as u32, i.height as u32))
        .unwrap_or((0, 0));
    drop(px);
    Ok(Outcome {
        elapsed,
        out_w: w,
        out_h: h,
        note: None,
    })
}

pub fn jpeg_decoder(path: &Path, denom: u32) -> Result<Outcome> {
    let t = Instant::now();
    let data = std::fs::read(path)?;
    let mut d = jpeg_decoder::Decoder::new(&data[..]);
    let (mut w, mut h) = (0u32, 0u32);
    if denom > 1 {
        d.read_info()
            .map_err(|e| anyhow::anyhow!("jpeg-decoder: {e}"))?;
        let info = d.info().context("no info")?;
        let (sw, sh) = d
            .scale(
                (info.width as u32).div_ceil(denom) as u16,
                (info.height as u32).div_ceil(denom) as u16,
            )
            .map_err(|e| anyhow::anyhow!("jpeg-decoder scale: {e}"))?;
        (w, h) = (sw as u32, sh as u32);
    }
    let px = d
        .decode()
        .map_err(|e| anyhow::anyhow!("jpeg-decoder: {e}"))?;
    let elapsed = t.elapsed();
    if denom == 1
        && let Some(i) = d.info()
    {
        (w, h) = (i.width as u32, i.height as u32);
    }
    drop(px);
    Ok(Outcome {
        elapsed,
        out_w: w,
        out_h: h,
        note: None,
    })
}

#[cfg(feature = "specular")]
pub fn specular(path: &Path, view: Option<(u32, u32)>, single: bool) -> Result<Outcome> {
    crate::specular_glue::decode_file(path, view, single)
}

#[cfg(not(feature = "specular"))]
pub fn specular(_path: &Path, _view: Option<(u32, u32)>, _single: bool) -> Result<Outcome> {
    bail!("built without the `specular` feature")
}

/// Run one decoder once.
pub fn run(dec: Dec, path: &Path, w: u32, h: u32, viewport: (u32, u32)) -> Result<Outcome> {
    let denom = viewport_denom(w, h, viewport.0, viewport.1);
    match dec {
        Dec::DjpegFull => djpeg(path, 1),
        Dec::DjpegView => djpeg(path, denom),
        Dec::Zune => zune(path),
        Dec::JdFull => jpeg_decoder(path, 1),
        Dec::JdView => jpeg_decoder(path, denom),
        Dec::SpecFull => specular(path, None, false),
        Dec::SpecFull1 => specular(path, None, true),
        Dec::SpecView => specular(path, Some(viewport), false),
        Dec::SpecView1 => specular(path, Some(viewport), true),
    }
}

/// Drop the file from the page cache (Linux; no root needed for clean pages).
#[cfg(target_os = "linux")]
pub fn drop_cache(path: &Path) -> Result<()> {
    let f = std::fs::File::open(path)?;
    rustix::fs::fdatasync(&f)?;
    rustix::fs::fadvise(&f, 0, None, rustix::fs::Advice::DontNeed)
        .context("posix_fadvise(DONTNEED)")?;
    Ok(())
}

/// No per-file eviction outside Linux (macOS: run `sudo purge` before a cold run; docs/macos.md).
#[cfg(not(target_os = "linux"))]
pub fn drop_cache(_path: &Path) -> Result<()> {
    bail!("dropping one file from the page cache is Linux-only (macOS: sudo purge)")
}

/// Fraction of the file's pages currently resident in the page cache, via util-linux `fincore`
/// (which uses mincore(2); doing that here would need an unsafe mmap).
pub fn resident_fraction(path: &Path) -> Option<f64> {
    let o = Command::new("fincore")
        .args(["--bytes", "--noheadings", "--raw", "--output", "RES,SIZE"])
        .arg(path)
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&o.stdout);
    let mut it = s.split_whitespace().map(|v| v.parse::<f64>().ok());
    let (res, size) = (it.next()??, it.next()??);
    (size > 0.0).then(|| res / size)
}

/// Process-spawn overhead of djpeg: median wall time of decoding a 16x16 JPEG.
pub fn djpeg_spawn_overhead(scratch: &Path) -> Result<Duration> {
    let tiny = scratch.join(".spawn-probe.jpg");
    if !tiny.exists() {
        let mut ppm = b"P6\n16 16\n255\n".to_vec();
        ppm.extend((0..16 * 16 * 3).map(|i| (i * 7 % 251) as u8));
        let mut child = Command::new("cjpeg")
            .args(["-quality", "90", "-outfile"])
            .arg(&tiny)
            .stdin(Stdio::piped())
            .spawn()
            .context("spawning cjpeg")?;
        use std::io::Write;
        child.stdin.take().unwrap().write_all(&ppm)?;
        if !child.wait()?.success() {
            bail!("cjpeg failed to make the spawn probe");
        }
    }
    let mut v: Vec<Duration> = (0..25)
        .map(|_| djpeg(&tiny, 1).map(|o| o.elapsed))
        .collect::<Result<_>>()?;
    v.sort();
    Ok(v[v.len() / 2])
}

pub struct ChildResult {
    pub status: Option<i32>,
    pub signal: Option<i32>,
    pub timed_out: bool,
    /// Largest VmHWM seen while polling the child (KiB); lower bound for very short-lived children.
    pub max_rss_kb: i64,
    pub stdout: String,
    pub elapsed: Duration,
}

fn vm_hwm_kb(pid: u32) -> Option<i64> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    s.lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// Run a command with a timeout (SIGKILL), polling its peak RSS from /proc while it runs.
pub fn run_child(mut cmd: Command, timeout: Duration) -> Result<ChildResult> {
    use std::io::Read;
    let t = Instant::now();
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let pid = child.id();
    let mut out = child.stdout.take().context("child stdout")?;
    let reader = specular_thread::spawn("bench-stdout", move || {
        let mut s = String::new();
        out.read_to_string(&mut s).ok();
        s
    });
    let mut timed_out = false;
    let mut hwm = 0i64;
    let status = loop {
        if let Some(k) = vm_hwm_kb(pid) {
            hwm = hwm.max(k);
        }
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if t.elapsed() > timeout && !timed_out {
            timed_out = true;
            let _ = child.kill();
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let elapsed = t.elapsed();
    let stdout = reader.join().unwrap_or_default();
    Ok(ChildResult {
        status: status.code(),
        signal: exit_signal(&status),
        timed_out,
        max_rss_kb: hwm,
        stdout,
        elapsed,
    })
}

/// VmHWM (peak resident set) of this process, in KiB.
pub fn self_hwm_kb() -> i64 {
    vm_hwm_kb(std::process::id()).unwrap_or(-1)
}

/// Largest ru_maxrss among waited-for children, in KiB (-1 where unknown).
pub fn children_maxrss_kb() -> i64 {
    #[cfg(unix)]
    return nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_CHILDREN)
        .map(|u| u.max_rss())
        .unwrap_or(-1);
    #[cfg(not(unix))]
    -1
}

/// The signal that ended a process (Unix only).
pub fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}
