//! Bruker NMR experiment folders (an experiment number: `acqus` with `fid` or `ser`, and
//! `pdata/<procno>/` with `procs` and `1r` or `2rr`) into the `specular-spectra` model.
//!
//! Pages, in order:
//! 1. each processed spectrum, lowest procno first: `1r` on its ppm axis, or `2rr` as a
//!    grid (F2 across, F1 down, both in ppm);
//! 2. when there is a FID (the first one of a `ser`): the spectrum processed here, from the
//!    FID: exponential window, zero fill, Fourier transform, the digital filter's group delay
//!    removed and the phase set automatically (said in the page's notes);
//! 3. the FID itself, real and imaginary parts against time.
//!
//! [`open`] takes the experiment folder, a `pdata/<procno>` folder in it (that procno comes
//! first) or one of its files (`fid`, `ser`, `acqus`, `1r`, `2rr`, `procs`…).

mod data;
mod dsp;
mod params;
pub mod process;

use std::path::{Path, PathBuf};

use specular_spectra::{Axis, Error, Facts, Grid, Peak, Plot, Spectra, Technique, Trace, warn};

use data::Encoding;
pub use params::Params;
use process::C64;

/// Processed spectra (procnos) read at most.
const MAX_PROCNOS: usize = 16;
/// Grid points of a 2-D spectrum kept at most; larger ones are reduced (said in the notes).
const MAX_GRID: usize = 1 << 24;
/// Spectrum size of the processing here, complex points at most.
const MAX_SIZE: usize = 1 << 20;
/// Parameter and text files read at most, bytes.
const MAX_TEXT: u64 = 4 << 20;

fn is_file(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file())
}

fn has_processed(pdata_n: &Path) -> bool {
    (is_file(&pdata_n.join("procs")))
        && (is_file(&pdata_n.join("1r")) || is_file(&pdata_n.join("2rr")))
}

/// Whether `dir` is a Bruker experiment folder (`acqus` with `fid`, `ser` or a processed
/// spectrum under `pdata/`), or a `pdata/<procno>` folder of one.
pub fn is_experiment_dir(dir: &Path) -> bool {
    if is_file(&dir.join("acqus")) {
        if is_file(&dir.join("fid")) || is_file(&dir.join("ser")) {
            return true;
        }
        return procnos(dir).iter().any(|(_, p)| has_processed(p));
    }
    pdata_experiment(dir).is_some()
}

/// For a `pdata/<procno>` folder with a processed spectrum: the experiment and procno.
fn pdata_experiment(dir: &Path) -> Option<(PathBuf, u32)> {
    let n: u32 = dir.file_name()?.to_str()?.parse().ok()?;
    let pdata = dir.parent()?;
    if pdata.file_name()? != "pdata" || !has_processed(dir) {
        return None;
    }
    let exp = pdata.parent()?;
    is_file(&exp.join("acqus")).then(|| (exp.to_path_buf(), n))
}

/// The experiment a file belongs to, and the procno it is in (for files under `pdata`).
pub fn experiment_of(path: &Path) -> Option<(PathBuf, Option<u32>)> {
    let name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    match name {
        "fid" | "ser" | "acqus" | "acqu2s" | "acqu" => {
            is_experiment_dir(parent).then(|| (parent.to_path_buf(), None))
        }
        "1r" | "1i" | "2rr" | "procs" | "proc2s" | "title" => {
            pdata_experiment(parent).map(|(e, n)| (e, Some(n)))
        }
        _ => None,
    }
}

/// Whether `path` is one of a Bruker experiment's files (`fid`, `ser`, `acqus`, `1r`,
/// `2rr`, `procs`…).
pub fn is_bruker_file(path: &Path) -> bool {
    experiment_of(path).is_some()
}

/// Numbered folders under `pdata`, ascending.
fn procnos(exp: &Path) -> Vec<(u32, PathBuf)> {
    let mut out: Vec<(u32, PathBuf)> = std::fs::read_dir(exp.join("pdata"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n: u32 = e.file_name().to_str()?.parse().ok()?;
            e.file_type().ok()?.is_dir().then(|| (n, e.path()))
        })
        .collect();
    out.sort();
    out.truncate(MAX_PROCNOS);
    out
}

fn read_text(path: &Path) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() || m.len() > MAX_TEXT {
        return None;
    }
    let b = std::fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&b).into_owned())
}

fn read_params(path: &Path) -> Option<Params> {
    read_text(path).map(|t| Params::parse(&t))
}

/// Why a folder without `acqus` is not a Bruker experiment, in plain words.
fn not_bruker(dir: &Path) -> Error {
    let has = |n: &str| dir.join(n).exists();
    if has("procpar") {
        return Error::Unsupported(
            "This is a Varian/Agilent (VnmrJ) experiment, not a Bruker one; it is not supported."
                .into(),
        );
    }
    if has("fid.gz") || has("ser.gz") || has("fid.zip") {
        return Error::Unsupported("The FID is compressed; unpack it first.".into());
    }
    Error::NotThisFormat
}

/// Reads a Bruker experiment (see the crate's documentation for what `path` may be).
pub fn open(path: &Path) -> Result<Spectra, Error> {
    let (exp, first) = if is_file(path) {
        experiment_of(path).ok_or(Error::NotThisFormat)?
    } else if let Some((e, n)) = pdata_experiment(path) {
        (e, Some(n))
    } else {
        (path.to_path_buf(), None)
    };
    let Some(acqus) = read_params(&exp.join("acqus")) else {
        return Err(not_bruker(&exp));
    };
    let acqu2s = read_params(&exp.join("acqu2s"));
    let mut facts = facts(&acqus, acqu2s.as_ref());
    let mut pages = Vec::new();

    let mut procs_list = procnos(&exp);
    if let Some(n) = first
        && let Some(i) = procs_list.iter().position(|(k, _)| *k == n)
    {
        let p = procs_list.remove(i);
        procs_list.insert(0, p);
    }
    let mut first_procs: Option<Params> = None;
    for (n, dir) in &procs_list {
        let Some(procs) = read_params(&dir.join("procs")) else {
            continue;
        };
        if facts.title.is_none() {
            facts.title = read_text(&dir.join("title"))
                .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|t| !t.is_empty());
        }
        let label = if procs_list.len() > 1 {
            format!(" (procno {n})")
        } else {
            String::new()
        };
        let page = if is_file(&dir.join("2rr")) {
            match read_params(&dir.join("proc2s")) {
                Some(proc2s) => grid_page(dir, &procs, &proc2s, &label),
                None => Err(Error::Malformed("2rr without proc2s".into())),
            }
        } else if is_file(&dir.join("1r")) {
            spectrum_page(dir, &procs, &label)
        } else {
            continue;
        };
        match page {
            Ok(p) => {
                pages.push(p);
                if first_procs.is_none() {
                    first_procs = Some(procs);
                }
            }
            Err(e) => warn(&mut facts.warnings, format!("pdata/{n}: {e}")),
        }
    }

    let fid_file = ["fid", "ser"]
        .into_iter()
        .map(|n| exp.join(n))
        .find(|p| is_file(p));
    if let Some(fid_path) = &fid_file {
        let series = fid_path.file_name().is_some_and(|n| n == "ser");
        match read_fid(fid_path, &acqus) {
            Ok(fid) => {
                match processed_page(&fid, &acqus, first_procs.as_ref(), series) {
                    Ok(p) => pages.push(p),
                    Err(e) if !pages.is_empty() => warn(&mut facts.warnings, e.to_string()),
                    Err(e) => return Err(e),
                }
                pages.push(fid_page(&fid, &acqus, series));
            }
            Err(e) if !pages.is_empty() => warn(&mut facts.warnings, e.to_string()),
            Err(e) => return Err(e),
        }
    }
    if pages.is_empty() {
        let has = |n: &str| exp.join(n).exists();
        if has("fid.gz") || has("ser.gz") {
            return Err(Error::Unsupported(
                "The FID is compressed; unpack it first.".into(),
            ));
        }
        return Err(Error::Malformed(
            "the experiment has no fid, ser or processed spectrum".into(),
        ));
    }
    facts.spectra = pages.len() as u64;
    Ok(Spectra { facts, pages })
}

fn shift_axis() -> Axis {
    Axis::new("Chemical shift", "ppm", true)
}

fn intensity_axis() -> Axis {
    Axis::new("Intensity", "", false)
}

fn encoding(p: &Params, dtype: &str, order: &str) -> Result<Encoding, Error> {
    let d = p.i64(dtype).unwrap_or(0);
    Encoding::new(d, p.i64(order).unwrap_or(0)).ok_or_else(|| {
        Error::Unsupported(format!(
            "data type {d} ({dtype}) is not supported: only 32-bit integers and 64-bit floats"
        ))
    })
}

/// The ppm axis of a processed dimension: `OFFSET` at the first point, `SW_p/SF` across.
fn ppm_axis(procs: &Params, si: usize) -> Result<(f64, f64), Error> {
    let off = procs
        .f64("OFFSET")
        .ok_or(Error::Malformed("no OFFSET".into()))?;
    let sw = procs
        .f64("SW_p")
        .ok_or(Error::Malformed("no SW_p".into()))?;
    let sf = procs
        .f64("SF")
        .filter(|v| *v > 0.0)
        .ok_or(Error::Malformed("no SF".into()))?;
    let step = sw / sf / si.max(1) as f64;
    Ok((off, step))
}

fn size(p: &Params) -> Result<usize, Error> {
    let si = p
        .i64("SI")
        .filter(|v| *v > 0)
        .ok_or(Error::Malformed("no SI".into()))?;
    usize::try_from(si).map_err(|_| Error::TooLarge)
}

/// A `1r` (scaled by `2^NC_proc`) on its ppm axis, with the peak list when there is one.
fn spectrum_page(dir: &Path, procs: &Params, label: &str) -> Result<Plot, Error> {
    let si = size(procs)?;
    let enc = encoding(procs, "DTYPP", "BYTORDP")?;
    let scale = 2f64.powi(procs.i64("NC_proc").unwrap_or(0).clamp(-64, 64) as i32);
    let y: Vec<f64> = data::read_values(&dir.join("1r"), enc, si)?
        .into_iter()
        .map(|v| v * scale)
        .collect();
    let (off, step) = ppm_axis(procs, si)?;
    let x: Vec<f64> = (0..si).map(|i| off - step * i as f64).collect();
    let peaks = peaks(dir, &x, &y);
    Ok(Plot {
        title: format!("Spectrum{label}"),
        x: shift_axis(),
        y: intensity_axis(),
        traces: vec![Trace {
            name: String::new(),
            x,
            y,
            sticks: false,
        }],
        peaks,
        grid: None,
        notes: vec![format!(
            "Processed spectrum as stored (pdata/{}).",
            dir.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        )],
    })
}

/// Peaks of `peaklist.xml` (TopSpin) at their ppm, with the spectrum's intensity there.
fn peaks(dir: &Path, x: &[f64], y: &[f64]) -> Vec<Peak> {
    let Some(text) = read_text(&dir.join("peaklist.xml")) else {
        return Vec::new();
    };
    let mut out: Vec<Peak> = Vec::new();
    for part in text.split("<Peak1D").skip(1) {
        let Some(at) = part.find("F1=\"") else {
            continue;
        };
        let rest = &part[at + 4..];
        let Some(ppm) = rest
            .split('"')
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
        else {
            continue;
        };
        let i = nearest(x, ppm);
        let v = i.and_then(|i| y.get(i)).copied().unwrap_or(0.0);
        if out.iter().any(|p| p.x == ppm) {
            continue;
        }
        out.push(Peak {
            x: ppm,
            y: v,
            label: format!("{ppm:.4}"),
        });
        if out.len() >= 4096 {
            break;
        }
    }
    out
}

fn nearest(x: &[f64], v: f64) -> Option<usize> {
    (0..x.len()).min_by(|&a, &b| (x[a] - v).abs().total_cmp(&(x[b] - v).abs()))
}

/// A `2rr` with F2 (procs) across and F1 (proc2s) down, untiled.
fn grid_page(dir: &Path, procs: &Params, proc2s: &Params, label: &str) -> Result<Plot, Error> {
    let (si2, si1) = (size(procs)?, size(proc2s)?);
    let n = si1.checked_mul(si2).ok_or(Error::TooLarge)?;
    if n > data::MAX_VALUES {
        return Err(Error::TooLarge);
    }
    let enc = encoding(procs, "DTYPP", "BYTORDP")?;
    let scale = 2f64.powi(procs.i64("NC_proc").unwrap_or(0).clamp(-64, 64) as i32);
    let raw = data::read_values(&dir.join("2rr"), enc, n)?;
    let xdim = |p: &Params, si: usize| {
        p.i64("XDIM")
            .and_then(|v| usize::try_from(v).ok())
            .filter(|v| *v > 0 && si.is_multiple_of(*v))
            .unwrap_or(si)
    };
    let tile = (xdim(proc2s, si1), xdim(procs, si2));
    let m = data::untile(&raw, (si1, si2), tile)
        .ok_or_else(|| Error::Malformed("2rr submatrices do not tile the spectrum".into()))?;
    drop(raw);
    let (off2, step2) = ppm_axis(procs, si2)?;
    let (off1, step1) = ppm_axis(proc2s, si1)?;
    // Reduced by whole factors when too large, keeping each block's largest magnitude.
    let mut k = 1;
    while (si1 / k) * (si2 / k) > MAX_GRID {
        k *= 2;
    }
    let (h, w) = (si1 / k, si2 / k);
    let mut z = Vec::with_capacity(w * h);
    for r in 0..h {
        for c in 0..w {
            let mut best = 0.0f64;
            for rr in 0..k {
                for cc in 0..k {
                    let v = m[(r * k + rr) * si2 + c * k + cc];
                    if v.abs() > best.abs() {
                        best = v;
                    }
                }
            }
            z.push((best * scale) as f32);
        }
    }
    let mut notes = vec![format!(
        "Processed 2-D spectrum as stored (pdata/{}).",
        dir.file_name().and_then(|n| n.to_str()).unwrap_or("?")
    )];
    if k > 1 {
        notes.push(format!("Shown at 1/{k} of its {si2} × {si1} points."));
    }
    let last = |off: f64, step: f64, n: usize, k: usize| off - step * ((n / k - 1) * k) as f64;
    Ok(Plot {
        title: format!("2-D spectrum{label}"),
        x: Axis::new("Chemical shift F2", "ppm", true),
        y: Axis::new("Chemical shift F1", "ppm", true),
        traces: Vec::new(),
        peaks: Vec::new(),
        grid: Some(Grid {
            width: w,
            height: h,
            z,
            x_range: (off2, last(off2, step2, si2, k)),
            y_range: (off1, last(off1, step1, si1, k)),
        }),
        notes,
    })
}

/// An experiment's acquisition parameters and its FID (the first one of a `ser`).
pub fn fid(exp: &Path) -> Result<(Params, Vec<C64>), Error> {
    let acqus = read_params(&exp.join("acqus")).ok_or_else(|| not_bruker(exp))?;
    let path = ["fid", "ser"]
        .into_iter()
        .map(|n| exp.join(n))
        .find(|p| is_file(p))
        .ok_or(Error::Malformed("no fid or ser".into()))?;
    let f = read_fid(&path, &acqus)?;
    Ok((acqus, f))
}

/// The parameters of the lowest-numbered procno, when there is one.
pub fn first_procs(exp: &Path) -> Option<Params> {
    procnos(exp)
        .iter()
        .find_map(|(_, d)| read_params(&d.join("procs")))
}

/// The FID (the first one of a `ser`) as complex points.
fn read_fid(path: &Path, acqus: &Params) -> Result<Vec<C64>, Error> {
    let td = acqus
        .i64("TD")
        .filter(|v| *v > 1)
        .ok_or(Error::Malformed("acqus has no TD".into()))?;
    let td = usize::try_from(td).map_err(|_| Error::TooLarge)? & !1;
    let enc = encoding(acqus, "DTYPA", "BYTORDA")?;
    let v = data::read_values(path, enc, td)?;
    Ok(data::complex(&v))
}

/// The digital filter's group delay in dwell times: `GRPDLY`, else the firmware table, else
/// none (analogue filtering, or a firmware not in the table: said in the warning).
pub fn group_delay(acqus: &Params) -> (f64, Option<String>) {
    if acqus.i64("DIGMOD") == Some(0) {
        return (0.0, None);
    }
    if let Some(g) = acqus.f64("GRPDLY").filter(|g| *g > 0.0) {
        return (g, None);
    }
    let (Some(fvs), Some(decim)) = (acqus.i64("DSPFVS"), acqus.f64("DECIM")) else {
        return (0.0, None);
    };
    if fvs >= 14 || decim <= 1.0 {
        return (0.0, None);
    }
    match dsp::group_delay(fvs, decim) {
        Some(g) => (g, None),
        None => (
            0.0,
            Some(format!(
                "No group delay known for DSP firmware {fvs} with decimation {decim}; none removed."
            )),
        ),
    }
}

/// How the FID is processed here: size, window and group delay, from `procs` when the
/// experiment has one.
pub fn settings(acqus: &Params, procs: Option<&Params>) -> Result<process::Settings, Error> {
    let td = acqus.i64("TD").unwrap_or(0).max(2) as usize;
    let sw = acqus
        .f64("SW_h")
        .filter(|v| *v > 0.0)
        .ok_or(Error::Malformed("acqus has no SW_h".into()))?;
    let size = procs
        .and_then(|p| p.i64("SI"))
        .and_then(|v| usize::try_from(v).ok())
        .filter(|v| *v >= 8)
        .unwrap_or(td)
        .max(td / 2)
        .next_power_of_two()
        .clamp(8, MAX_SIZE);
    let nucleus = acqus.text("NUC1").unwrap_or_default();
    let default_lb = if matches!(nucleus.as_str(), "1H" | "19F" | "3H") {
        0.3
    } else {
        1.0
    };
    let lb = match procs.and_then(|p| p.i64("WDW")) {
        Some(1) => procs.and_then(|p| p.f64("LB")).unwrap_or(default_lb),
        Some(0) => 0.0,
        _ => default_lb,
    };
    Ok(process::Settings {
        size,
        lb_hz: lb.max(0.0),
        sw_hz: sw,
        group_delay: group_delay(acqus).0,
    })
}

/// The ppm of point `i` of an `n`-point spectrum processed here.
pub fn ppm_of(acqus: &Params, procs: Option<&Params>, n: usize) -> Option<impl Fn(usize) -> f64> {
    let sw = acqus.f64("SW_h")?;
    let sfo1 = acqus.f64("SFO1")?;
    let sf = procs
        .and_then(|p| p.f64("SF"))
        .or_else(|| acqus.f64("BF1"))
        .filter(|v| *v > 0.0)?;
    let nf = n as f64;
    Some(move |i: usize| ((sfo1 - sf) * 1e6 + (nf / 2.0 - i as f64) * sw / nf) / sf)
}

/// The spectrum processed here from `fid`.
fn processed_page(
    fid: &[C64],
    acqus: &Params,
    procs: Option<&Params>,
    series: bool,
) -> Result<Plot, Error> {
    if matches!(acqus.i64("AQ_mod"), Some(0 | 2)) {
        return Err(Error::Unsupported(
            "Sequential (qf or qseq) acquisitions are not processed here; process it in TopSpin first."
                .into(),
        ));
    }
    let s = settings(acqus, procs)?;
    let spec = process::spectrum(fid, &s);
    let (p0, p1) = process::autophase(&spec);
    let y: Vec<f64> = process::phase(&spec, p0, p1).iter().map(|v| v.re).collect();
    let n = y.len();
    let ppm = ppm_of(acqus, procs, n).ok_or(Error::Malformed("acqus has no SFO1".into()))?;
    let x: Vec<f64> = (0..n).map(ppm).collect();
    let mut notes = Vec::new();
    if series {
        notes.push("The first FID of the series.".to_owned());
    }
    notes.push(format!(
        "Processed by Specular from the FID: exponential window {} Hz, {n} points.",
        trim(s.lb_hz)
    ));
    notes.push(format!(
        "Phase set automatically: 0th order {p0:.1}°, 1st order {p1:.1}°."
    ));
    if let (_, Some(w)) = group_delay(acqus) {
        notes.push(w);
    }
    Ok(Plot {
        title: "Spectrum from the FID".into(),
        x: shift_axis(),
        y: intensity_axis(),
        traces: vec![Trace {
            name: String::new(),
            x,
            y,
            sticks: false,
        }],
        peaks: Vec::new(),
        grid: None,
        notes,
    })
}

fn trim(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The FID's real and imaginary parts against time.
fn fid_page(fid: &[C64], acqus: &Params, series: bool) -> Plot {
    let sw = acqus.f64("SW_h").filter(|v| *v > 0.0).unwrap_or(1.0);
    let x: Vec<f64> = (0..fid.len()).map(|j| j as f64 / sw).collect();
    let mut notes = Vec::new();
    if series {
        notes.push("The first FID of the series.".to_owned());
    }
    Plot {
        title: "FID".into(),
        x: Axis::new("Time", "s", false),
        y: Axis::new("Signal", "", false),
        traces: vec![
            Trace {
                name: "Real".into(),
                x: x.clone(),
                y: fid.iter().map(|v| v.re).collect(),
                sticks: false,
            },
            Trace {
                name: "Imaginary".into(),
                x,
                y: fid.iter().map(|v| v.im).collect(),
                sticks: false,
            },
        ],
        peaks: Vec::new(),
        grid: None,
        notes,
    }
}

/// "TopSpin 3.2" from `##TITLE= Parameter file, TOPSPIN  Version 3.2`.
fn version(acqus: &Params) -> String {
    let t = acqus.raw("TITLE").unwrap_or_default();
    let t = t.split_once(',').map_or(t, |(_, r)| r);
    let words: Vec<&str> = t.split_whitespace().filter(|w| *w != "Version").collect();
    let mut s = words.join(" ");
    if let Some(r) = s.strip_prefix("TOPSPIN") {
        s = format!("TopSpin{r}");
    }
    s
}

/// `YYYY-MM-DD HH:MM UTC` of a Unix time.
fn iso(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil from days (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60
    )
}

fn facts(acqus: &Params, acqu2s: Option<&Params>) -> Facts {
    let mut f = Facts {
        format: "Bruker NMR".into(),
        version: version(acqus),
        technique: Technique::Nmr,
        origin: acqus.text("OWNER"),
        instrument: acqus.text("INSTRUM"),
        date: acqus
            .i64("DATE")
            .filter(|d| (0..10_000_000_000).contains(d))
            .map(iso),
        points: acqus.i64("TD").unwrap_or(0).max(0) as u64,
        nucleus: acqus.text("NUC1").filter(|n| n != "off" && n != "NONE"),
        frequency_mhz: acqus.f64("SFO1"),
        solvent: acqus.text("SOLVENT"),
        pulse_program: acqus.text("PULPROG"),
        ..Facts::default()
    };
    let mut add = |label: &str, v: Option<String>| {
        if let Some(v) = v {
            f.fields.push((label.to_owned(), v));
        }
    };
    let num = |p: &Params, k: &str| {
        p.raw(k)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty() && !v.starts_with('('))
    };
    for k in ["NS", "DS", "TD", "SW", "SW_h", "O1", "RG", "TE"] {
        add(k, num(acqus, k));
    }
    add("D1", acqus.element("D", 1).map(trim));
    add("P1", acqus.element("P", 1).map(trim));
    if let (Some(td), Some(sw)) = (acqus.f64("TD"), acqus.f64("SW_h"))
        && sw > 0.0
    {
        add("AQ", Some(trim(td / 2.0 / sw)));
    }
    add("PROBHD", acqus.text("PROBHD"));
    if let Some(a2) = acqu2s {
        add("TD (F1)", num(a2, "TD"));
        add("SW (F1)", num(a2, "SW"));
        add("NUC1 (F1)", a2.text("NUC1"));
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_versions() {
        assert_eq!(iso(1_302_788_098), "2011-04-14 13:34 UTC");
        assert_eq!(iso(0), "1970-01-01 00:00 UTC");
        let p = Params::parse("##TITLE= Parameter file, TOPSPIN\t\tVersion 3.2\n");
        assert_eq!(version(&p), "TopSpin 3.2");
        let p = Params::parse("##TITLE= Parameter file, XWIN-NMR\t\tVersion 3.5\n");
        assert_eq!(version(&p), "XWIN-NMR 3.5");
    }
}
