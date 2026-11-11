//! Recipes: user commands whose output comes back (`specular_settings::commands`). A recipe
//! runs like any user command (no shell unless it asks for one, never with Specular's
//! terminal input), after the user confirmed it once (the confirmation is remembered per
//! program, arguments and output), and is stopped after its time limit. Its output is read
//! through the normal decoders: opened like any file, read as GeoJSON shapes, or decoded as a
//! label image whose regions are drawn as outlines over the source image (a script overlay
//! layer named after the recipe).

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use specular_i18n::t;
use specular_log::{UI, info, warn};
use specular_platform::launch::{self, Target};
use specular_settings::{Load, UserCommand};

use super::App;
use crate::remote::protocol::{Shape, ShapeKind};

/// The confirmation's palette item.
pub(super) const CONFIRM_ITEM: u64 = super::items::RECIPE_CONFIRM;
/// The largest output read as GeoJSON.
const MAX_GEOJSON: u64 = 64 << 20;
/// The largest label image, in pixels.
const MAX_LABEL_PIXELS: u64 = 1 << 26;
/// The most outlines one label image gives.
const MAX_OUTLINES: usize = 100_000;
/// Run folders a viewer keeps (older ones go).
const KEEP_RUNS: usize = 8;
/// Where the trusted recipes are listed, in the config folder.
const TRUST_FILE: &str = "recipes-trusted.txt";
/// The folder of recipe files, in the config folder.
pub(super) const RECIPE_DIR: &str = "recipes";

/// Shapes by the file they name (`None`: the source file).
pub(super) type ByFile = Vec<(Option<String>, Vec<Shape>)>;

/// What a finished run sends back.
pub(super) enum Done {
    /// The program ended (or could not be waited for).
    Exited {
        run: u64,
        result: Result<(), String>,
    },
    /// The output, read: shapes for the overlays, by the file they name (`None`: the source
    /// file's).
    Loaded {
        run: u64,
        shapes: Result<ByFile, String>,
    },
}

struct Run {
    name: String,
    source: PathBuf,
    /// A series recipe's files: its shapes may name any of them.
    files: Vec<PathBuf>,
    output: PathBuf,
    load: Load,
}

pub(super) struct Recipes {
    tx: Sender<Done>,
    rx: Receiver<Done>,
    runs: HashMap<u64, Run>,
    next: u64,
    /// The recipe waiting for the user's first confirmation (its command index).
    confirm: Option<usize>,
    trusted: Option<HashSet<u64>>,
    folders: Vec<PathBuf>,
}

impl Default for Recipes {
    fn default() -> Recipes {
        let (tx, rx) = crossbeam_channel::unbounded();
        Recipes {
            tx,
            rx,
            runs: HashMap::new(),
            next: 0,
            confirm: None,
            trusted: None,
            folders: Vec::new(),
        }
    }
}

/// A stable fingerprint of what a recipe runs (FNV-1a): a changed program, argument, output
/// or shell flag asks again.
pub(super) fn fingerprint(c: &UserCommand) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |s: &str| {
        for b in s.bytes().chain([0]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(&c.program);
    for a in &c.args {
        eat(a);
    }
    if let Some(r) = &c.recipe {
        eat(&r.output);
        eat(if r.shell { "shell" } else { "direct" });
    }
    h
}

/// The trusted fingerprints in `path` (missing or unreadable: none).
pub(super) fn read_trust(path: &Path) -> HashSet<u64> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| u64::from_str_radix(l.trim(), 16).ok())
        .collect()
}

/// Adds `fp` to the list in `path`.
pub(super) fn write_trust(path: &Path, all: &HashSet<u64>) -> std::io::Result<()> {
    let mut v: Vec<u64> = all.iter().copied().collect();
    v.sort_unstable();
    let text: String = v.iter().map(|f| format!("{f:016x}\n")).collect();
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, text)
}

/// Reads the recipe files of `dir` (`*.toml`, by name). Problems are logged and skipped.
pub(super) fn read_dir(dir: &Path) -> Vec<UserCommand> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml") && p.is_file())
        .collect();
    files.sort();
    let mut out = Vec::new();
    for p in files.into_iter().take(1000) {
        let text = match std::fs::metadata(&p) {
            Ok(m) if m.len() <= 1 << 20 => std::fs::read_to_string(&p),
            _ => continue,
        };
        match text
            .map_err(|e| e.to_string())
            .and_then(|t| specular_settings::commands::parse_recipe(&t))
        {
            Ok((c, notes)) => {
                for n in notes {
                    warn!(target: UI, "recipe {}: {n}", p.display());
                }
                out.push(c);
            }
            Err(e) => warn!(target: UI, "recipe {}: {e}", p.display()),
        }
    }
    out
}

/// Escapes `p` for a template (braces doubled).
fn literal(p: &Path) -> String {
    p.to_string_lossy().replace('{', "{{").replace('}', "}}")
}

/// Expands a template that names one path (`output`) with `{out}` as `out`.
pub(super) fn expand_one(template: &str, t: &Target, out: &Path) -> Result<PathBuf, String> {
    let s = template.replace("{out}", &literal(out));
    let v = launch::expand_args(&[s], t).map_err(|e| e.to_string())?;
    match v.as_slice() {
        [one] => Ok(PathBuf::from(one)),
        _ => Err("`output` must name one file".into()),
    }
}

/// The program and arguments to start: directly, or through `sh -c` with every placeholder
/// quoted.
pub(super) fn command_line(
    c: &UserCommand,
    t: &Target,
    out: &Path,
) -> Result<(OsString, Vec<OsString>), String> {
    let shell = c.recipe.as_ref().is_some_and(|r| r.shell);
    if !shell {
        let args: Vec<String> = c
            .args
            .iter()
            .map(|a| a.replace("{out}", &literal(out)))
            .collect();
        let args = launch::expand_args(&args, t).map_err(|e| e.to_string())?;
        let prog: OsString = launch::find_program(&c.program)
            .map_or_else(|| c.program.clone().into(), PathBuf::into_os_string);
        return Ok((prog, args));
    }
    if cfg!(windows) {
        return Err("shell recipes run on Linux and macOS only".into());
    }
    // Each placeholder is expanded alone and quoted; the text around stays as written.
    let src = c.program.as_str();
    let mut line = String::new();
    let mut rest = src;
    while let Some(i) = rest.find('{') {
        line.push_str(&rest[..i].replace("}}", "}"));
        rest = &rest[i..];
        if let Some(r) = rest.strip_prefix("{{") {
            line.push('{');
            rest = r;
            continue;
        }
        let end = rest.find('}').ok_or("unbalanced { in `program`")?;
        let name = &rest[1..end];
        let vals: Vec<String> = if name == "out" {
            vec![out.to_string_lossy().into_owned()]
        } else {
            launch::expand_args(&[format!("{{{name}}}")], t)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|v| v.to_string_lossy().into_owned())
                .collect()
        };
        let quoted: Vec<String> = vals.iter().map(|v| launch::shell_quote(v)).collect();
        line.push_str(&quoted.join(" "));
        rest = &rest[end + 1..];
    }
    line.push_str(&rest.replace("}}", "}"));
    Ok(("sh".into(), vec!["-c".into(), line.into()]))
}

/// A colour per label, well apart for neighbouring numbers.
fn label_colour(l: u32) -> [u8; 4] {
    let h = (f64::from(l) * 0.618_033_988_75).fract() * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let c = |v: f64| (55.0 + v * 200.0) as u8;
    [c(r), c(g), c(b), 255]
}

/// The outlines of every region of a label image (`w`×`h`, 0 is background): one closed
/// polygon per boundary loop, along pixel edges, collinear steps merged.
pub(super) fn label_outlines(w: u32, h: u32, labels: &[u32]) -> Result<Vec<Shape>, String> {
    let (w, h) = (w as usize, h as usize);
    if labels.len() != w * h {
        return Err("label image size mismatch".into());
    }
    let at = |x: isize, y: isize| -> u32 {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            0
        } else {
            labels[y as usize * w + x as usize]
        }
    };
    // Directed edges, clockwise around each pixel (y down), between a label and anything
    // else: start vertex → ends, per label.
    type Vertex = (u32, u32);
    let mut edges: HashMap<u32, HashMap<Vertex, Vec<Vertex>>> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let l = labels[y * w + x];
            if l == 0 {
                continue;
            }
            let (xi, yi) = (x as isize, y as isize);
            let (x0, y0, x1, y1) = (x as u32, y as u32, x as u32 + 1, y as u32 + 1);
            let e = edges.entry(l).or_default();
            if at(xi, yi - 1) != l {
                e.entry((x0, y0)).or_default().push((x1, y0));
            }
            if at(xi + 1, yi) != l {
                e.entry((x1, y0)).or_default().push((x1, y1));
            }
            if at(xi, yi + 1) != l {
                e.entry((x1, y1)).or_default().push((x0, y1));
            }
            if at(xi - 1, yi) != l {
                e.entry((x0, y1)).or_default().push((x0, y0));
            }
        }
    }
    let mut keys: Vec<u32> = edges.keys().copied().collect();
    keys.sort_unstable();
    let mut out = Vec::new();
    for l in keys {
        let mut e = edges.remove(&l).unwrap_or_default();
        let mut starts: Vec<Vertex> = e.keys().copied().collect();
        starts.sort_unstable();
        for s in starts {
            while e.get(&s).is_some_and(|v| !v.is_empty()) {
                let mut pts = vec![s];
                let mut cur = s;
                while let Some(next) = e.get_mut(&cur).and_then(Vec::pop) {
                    cur = next;
                    if cur == s {
                        break;
                    }
                    pts.push(cur);
                }
                // Merge collinear steps.
                let n = pts.len();
                let corners: Vec<[f64; 2]> = (0..n)
                    .filter(|&i| {
                        let a = pts[(i + n - 1) % n];
                        let b = pts[i];
                        let c = pts[(i + 1) % n];
                        let d1 = (
                            i64::from(b.0) - i64::from(a.0),
                            i64::from(b.1) - i64::from(a.1),
                        );
                        let d2 = (
                            i64::from(c.0) - i64::from(b.0),
                            i64::from(c.1) - i64::from(b.1),
                        );
                        d1.0 * d2.1 - d1.1 * d2.0 != 0
                    })
                    .map(|i| [f64::from(pts[i].0), f64::from(pts[i].1)])
                    .collect();
                if corners.len() >= 3 {
                    if out.len() >= MAX_OUTLINES {
                        return Err(format!("more than {MAX_OUTLINES} outlines"));
                    }
                    let mut sh = Shape::new(ShapeKind::Polygon, corners);
                    sh.color = label_colour(l);
                    sh.width = 1.5;
                    out.push(sh);
                }
            }
        }
    }
    Ok(out)
}

/// The labels of a decoded raster: grey 8 or 16 bits, or packed RGB.
fn raster_labels(r: &specular_core::Raster) -> Result<Vec<u32>, String> {
    use specular_core::PixelLayout as L;
    let (w, h) = (r.width as usize, r.height as usize);
    if r.width as u64 * r.height as u64 > MAX_LABEL_PIXELS {
        return Err("the label image is too large".into());
    }
    let p0 = r.planes.first().ok_or("no pixels")?;
    fn plane_row(p: &specular_core::RasterPlane, y: usize, len: usize) -> Result<&[u8], String> {
        p.data
            .get(y * p.stride..y * p.stride + len)
            .ok_or_else(|| "short pixel data".to_owned())
    }
    let row = |p, y, bpp: usize| plane_row(p, y, w * bpp);
    let mut out = Vec::with_capacity(w * h);
    match r.layout {
        L::Gray => {
            for y in 0..h {
                out.extend(row(p0, y, 1)?.iter().map(|&v| u32::from(v)));
            }
        }
        L::Gray16 => {
            for y in 0..h {
                out.extend(
                    row(p0, y, 2)?
                        .chunks_exact(2)
                        .map(|c| u32::from(u16::from_ne_bytes([c[0], c[1]]))),
                );
            }
        }
        L::Rgb if r.planes.len() >= 3 => {
            for y in 0..h {
                let (a, b, c) = (
                    row(&r.planes[0], y, 1)?,
                    row(&r.planes[1], y, 1)?,
                    row(&r.planes[2], y, 1)?,
                );
                out.extend(
                    (0..w).map(|x| {
                        (u32::from(a[x]) << 16) | (u32::from(b[x]) << 8) | u32::from(c[x])
                    }),
                );
            }
        }
        L::Rgba => {
            for y in 0..h {
                out.extend(
                    row(p0, y, 4)?.chunks_exact(4).map(|c| {
                        (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2])
                    }),
                );
            }
        }
        _ => return Err("not a label image (grey or RGB integers)".into()),
    }
    Ok(out)
}

/// Reads a label image through the decoders and outlines its regions.
pub(super) fn load_labels(
    registry: &specular_decode::Registry,
    path: &Path,
) -> Result<Vec<Shape>, String> {
    let session = registry.open_path(path).map_err(|e| e.to_string())?;
    let cancel = specular_core::CancelToken::new();
    let r = session
        .decode(&specular_decode::DecodeRequest::full(1), &cancel)
        .map_err(|e| e.to_string())?;
    let labels = raster_labels(&r)?;
    label_outlines(r.width, r.height, &labels)
}

/// Reads GeoJSON shapes (as measurements are written) as overlay shapes.
#[cfg(test)]
#[cfg_attr(not(unix), allow(dead_code))] // only the Unix tests use it
pub(super) fn load_geojson(path: &Path) -> Result<Vec<Shape>, String> {
    let m = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > MAX_GEOJSON {
        return Err("not a file, or larger than 64 MiB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    geojson_shapes(&text)
}

/// A series recipe's GeoJSON, split by the file each feature names in `properties.file`
/// (a path or a file name; `None` for features that name none, which go on the current
/// file).
pub(super) fn load_geojson_by_file(path: &Path) -> Result<ByFile, String> {
    let m = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > MAX_GEOJSON {
        return Err("not a file, or larger than 64 MiB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let Some(features) = v.get("features").and_then(serde_json::Value::as_array) else {
        return Ok(vec![(None, geojson_shapes(&text)?)]);
    };
    let mut groups: Vec<(Option<String>, Vec<serde_json::Value>)> = Vec::new();
    for f in features {
        let file = f
            .get("properties")
            .and_then(|p| p.get("file"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        match groups.iter_mut().find(|g| g.0 == file) {
            Some(g) => g.1.push(f.clone()),
            None => groups.push((file, vec![f.clone()])),
        }
    }
    groups
        .into_iter()
        .map(|(file, fs)| {
            let fc = serde_json::json!({"type": "FeatureCollection", "features": fs});
            Ok((file, geojson_shapes(&fc.to_string())?))
        })
        .collect()
}

/// Which of `files` a feature's `file` names: the same path, or the same file name.
pub(super) fn named_file<'a>(files: &'a [PathBuf], name: &str) -> Option<&'a PathBuf> {
    let p = Path::new(name);
    files.iter().find(|f| f.as_path() == p).or_else(|| {
        files
            .iter()
            .find(|f| f.file_name().is_some_and(|n| Some(n) == p.file_name()))
    })
}

fn geojson_shapes(text: &str) -> Result<Vec<Shape>, String> {
    let read = specular_measure::geojson::read(text).map_err(|e| e.to_string())?;
    use specular_measure::Shape as M;
    let mut out = Vec::new();
    for it in read
        .items
        .iter()
        .take(crate::remote::protocol::MAX_SHAPES * 10)
    {
        let pts = |v: Vec<specular_measure::P>| -> Vec<[f64; 2]> {
            v.iter().map(|p| [p.x, p.y]).collect()
        };
        let (kind, points) = match &it.shape {
            M::Point(p) => (ShapeKind::Point, vec![[p.x, p.y]]),
            M::Line(a, b) => (ShapeKind::Line, vec![[a.x, a.y], [b.x, b.y]]),
            s @ (M::Polyline(_) | M::Angle { .. }) => (ShapeKind::Polyline, pts(s.points())),
            s => (ShapeKind::Polygon, pts(s.outline())),
        };
        let (lo, _) = kind.points();
        if points.len() < lo {
            continue;
        }
        let mut sh = Shape::new(kind, points);
        sh.text = it.label.clone();
        sh.color = [0, 229, 255, 255];
        out.push(sh);
    }
    Ok(out)
}

impl App {
    /// Runs recipe `i` (a user command with an output): asks first the first time.
    pub(super) fn recipe_start(&mut self, i: usize) {
        let Some(c) = self.user_command(i) else {
            return;
        };
        let path = self.integration_locations().config_dir.join(TRUST_FILE);
        let trusted = self
            .recipes
            .trusted
            .get_or_insert_with(|| read_trust(&path));
        if trusted.contains(&fingerprint(&c)) {
            self.recipe_run(i);
            return;
        }
        self.recipes.confirm = Some(i);
        let line = std::iter::once(c.program.clone())
            .chain(c.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        self.chrome.confirm(
            t!(
                "recipe-confirm-title",
                name = c.name.clone(),
                command = line
            ),
            t!("recipe-confirm-run"),
            CONFIRM_ITEM,
        );
        self.redraw = true;
    }

    /// The user accepted the first run of a recipe: it is trusted from now on.
    pub(super) fn recipe_confirmed(&mut self) {
        let Some(i) = self.recipes.confirm.take() else {
            return;
        };
        let Some(c) = self.user_command(i) else {
            return;
        };
        let path = self.integration_locations().config_dir.join(TRUST_FILE);
        let trusted = self
            .recipes
            .trusted
            .get_or_insert_with(|| read_trust(&path));
        trusted.insert(fingerprint(&c));
        if let Err(e) = write_trust(&path, trusted) {
            warn!(target: UI, "recipes: could not remember the confirmation: {e}");
        }
        self.recipe_run(i);
    }

    fn recipe_run(&mut self, i: usize) {
        let Some(c) = self.user_command(i) else {
            return;
        };
        let Some(r) = c.recipe.clone() else {
            return;
        };
        let Some(file) = self.integration_file() else {
            self.integration_toast(t!("recipe-no-file"));
            return;
        };
        let files: Vec<PathBuf> = if r.series {
            self.entries
                .iter()
                .take(crate::remote::protocol::MAX_FILES)
                .cloned()
                .collect()
        } else if self.gallery.active && !self.gallery.targets().is_empty() {
            self.gallery.targets()
        } else {
            vec![file.clone()]
        };
        let fail = |app: &mut App, reason: String| {
            app.integration_toast(t!("recipe-failed", name = c.name.clone(), reason = reason));
        };
        self.recipes.next += 1;
        let run = self.recipes.next;
        let out = self
            .integration_locations()
            .cache_dir
            .join(RECIPE_DIR)
            .join(format!("{}-{run}", std::process::id()));
        if let Err(e) = std::fs::create_dir_all(&out) {
            fail(self, crate::error_text::io_reason(&e));
            return;
        }
        self.recipes.folders.push(out.clone());
        while self.recipes.folders.len() > KEEP_RUNS {
            let old = self.recipes.folders.remove(0);
            let _ = std::fs::remove_dir_all(old);
        }
        let target = Target {
            file: &file,
            files: &files,
            page: 1,
        };
        let (output, (prog, args)) = match (
            expand_one(&r.output, &target, &out),
            command_line(&c, &target, &out),
        ) {
            (Ok(o), Ok(cl)) => (o, cl),
            (Err(e), _) | (_, Err(e)) => return fail(self, e),
        };
        let tx = self.recipes.tx.clone();
        let wake = self.loader.waker();
        let on_exit: launch::OnExit = Box::new(move |res| {
            let result = match res {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(s.to_string()),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    Err(t!("recipe-timeout", seconds = i64::from(r.timeout)))
                }
                Err(e) => Err(crate::error_text::spawn_reason(&e)),
            };
            let _ = tx.send(Done::Exited { run, result });
            wake();
        });
        info!(target: UI, "recipe {:?}: {} {args:?}", c.name, prog.to_string_lossy());
        let timeout = Duration::from_secs(u64::from(r.timeout));
        if let Err(e) = launch::spawn_with_timeout(&prog, &args, file.parent(), timeout, on_exit) {
            return fail(self, crate::error_text::spawn_reason(&e));
        }
        self.recipes.runs.insert(
            run,
            Run {
                name: c.name.clone(),
                source: file,
                files: if r.series { files } else { Vec::new() },
                output,
                load: r.load,
            },
        );
        self.integration_toast(t!("recipe-running", name = c.name));
    }

    /// Once per turn: finished runs, and their outputs once read.
    pub(super) fn recipes_poll(&mut self) {
        while let Ok(d) = self.recipes.rx.try_recv() {
            match d {
                Done::Exited { run, result } => self.recipe_exited(run, result),
                Done::Loaded { run, shapes } => {
                    let Some(r) = self.recipes.runs.remove(&run) else {
                        continue;
                    };
                    let r2 = shapes.and_then(|groups| {
                        // Each shape on the file it names (a series recipe), else the source.
                        let mut n = 0;
                        let mut cleared: Vec<PathBuf> = Vec::new();
                        for (file, s) in groups {
                            let target = match &file {
                                None => &r.source,
                                Some(name) => match named_file(&r.files, name) {
                                    Some(f) => f,
                                    None => continue,
                                },
                            };
                            if !cleared.contains(target) {
                                self.remote.script.clear(target, Some(&r.name));
                                cleared.push(target.clone());
                            }
                            n += s.len();
                            self.remote
                                .script
                                .add(target, &r.name, s)
                                .map_err(|f| f.message)?;
                        }
                        Ok(n)
                    });
                    match r2 {
                        Ok(n) => {
                            self.integration_toast(t!(
                                "recipe-done",
                                name = r.name,
                                shapes = n as i64
                            ));
                        }
                        Err(e) => {
                            self.integration_toast(t!("recipe-failed", name = r.name, reason = e));
                        }
                    }
                }
            }
        }
    }

    fn recipe_exited(&mut self, run: u64, result: Result<(), String>) {
        let Some(r) = self.recipes.runs.get(&run) else {
            return;
        };
        let name = r.name.clone();
        if let Err(e) = result {
            self.recipes.runs.remove(&run);
            self.integration_toast(t!("recipe-failed", name = name, reason = e));
            return;
        }
        if !r.output.is_file() {
            let shown = r.output.to_string_lossy().into_owned();
            self.recipes.runs.remove(&run);
            self.integration_toast(t!("recipe-no-output", name = name, file = shown));
            return;
        }
        match r.load {
            Load::Open => {
                let out = r.output.clone();
                self.recipes.runs.remove(&run);
                self.open_pasted(vec![out]);
            }
            Load::Overlay | Load::Labels => {
                let (path, load) = (r.output.clone(), r.load);
                let registry = self.loader.registry();
                let tx = self.recipes.tx.clone();
                self.loader.background("recipe-load", move |l| {
                    let shapes = if load == Load::Labels {
                        load_labels(&registry, &path).map(|s| vec![(None, s)])
                    } else {
                        load_geojson_by_file(&path)
                    };
                    let _ = tx.send(Done::Loaded { run, shapes });
                    l.waker()();
                });
            }
        }
    }
}

#[cfg(test)]
mod tests;
