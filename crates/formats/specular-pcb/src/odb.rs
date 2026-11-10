//! Own ODB++ reader (job folders, and the files of a `.tgz`/`.zip` job listed by the caller):
//! the matrix (layer order, types and sides), the chosen step's profile (the board's shape),
//! each board layer's features (pads with standard and user symbols, lines, arcs, surfaces),
//! drill layers as holes and slots, and components with their reference designators and
//! package outlines (`eda/data`). Written from the ODB++ format specification (v7/v8).
//!
//! Text records (`T`) are drawn in the job's stroke font (`fonts/<name>`, else the built-in
//! one); barcodes are counted but not drawn. A NEGATIVE layer is drawn as its step's profile
//! with the features cut out (clear where they are positive). Every step is read: the
//! chosen one first, the others after it as pages of their own, step-and-repeat copies
//! expanded.

use std::collections::HashMap;

use specular_vector::kurbo::{Affine, BezPath, Point, Rect, Shape as _};
use specular_vector::{PcbKind, VectorScene};

use crate::excellon::{Drill, Hole, Slot, Tool};
use crate::features::{self, Contour, Pen, Prim};
use crate::gerber::{Object, Segment};
use crate::stack::{self, Component, Extra, Plating, Role, Side};
use crate::text::StrokeFont;
use crate::{Error, Limits, warn};

/// Subsymbol nesting of user symbols.
const MAX_DEPTH: u32 = 4;

/// A job's files by path relative to the job's folder (`matrix/matrix`,
/// `steps/pcb/layers/top/features`), any case; compressed `.z` / `.gz` variants are the
/// caller's to unpack (see [`unpack`]).
pub struct Job<'a> {
    files: HashMap<String, &'a [u8]>,
}

impl<'a> Job<'a> {
    /// From `(path, bytes)`; a common top folder (the job's name) is taken off.
    pub fn new(files: &'a [(String, Vec<u8>)]) -> Job<'a> {
        let prefix = job_prefix(files.iter().map(|(n, _)| n.as_str()));
        let mut m = HashMap::new();
        for (n, b) in files {
            let n = n.replace('\\', "/");
            let rel = n
                .strip_prefix(&prefix)
                .unwrap_or(&n)
                .trim_start_matches('/');
            m.insert(rel.to_ascii_lowercase(), b.as_slice());
        }
        Job { files: m }
    }

    fn get(&self, path: &str) -> Option<&'a [u8]> {
        self.files.get(&path.to_ascii_lowercase()).copied()
    }

    fn has_prefix(&self, p: &str) -> bool {
        let p = p.to_ascii_lowercase();
        self.files.keys().any(|k| k.starts_with(&p))
    }
}

/// The folder above `matrix/matrix` ("job/"), or empty.
fn job_prefix<'n>(names: impl Iterator<Item = &'n str>) -> String {
    for n in names {
        let n = n.replace('\\', "/").to_ascii_lowercase();
        if let Some(i) = n.find("matrix/matrix")
            && (i == 0 || n.as_bytes()[i - 1] == b'/')
        {
            return n[..i].to_string();
        }
    }
    String::new()
}

/// Whether paths (of a folder or archive) are an ODB++ job: `matrix/matrix` and `steps/`.
pub fn looks_like_job(names: &[&str]) -> bool {
    let prefix = job_prefix(names.iter().copied());
    let has = |p: &str| {
        names.iter().any(|n| {
            n.replace('\\', "/")
                .to_ascii_lowercase()
                .starts_with(&format!("{prefix}{p}"))
        })
    };
    has("matrix/matrix") && has("steps/")
}

/// Whether the first paths of an archive (it may be cut short) look like an ODB++ job:
/// `matrix/matrix`, or a `steps/` folder beside `misc/info`, `symbols/`, `fonts/` or
/// `matrix/` under one top folder.
pub fn looks_like_job_start(names: &[&str]) -> bool {
    if looks_like_job(names) {
        return true;
    }
    let norm: Vec<String> = names
        .iter()
        .map(|n| n.replace('\\', "/").to_ascii_lowercase())
        .collect();
    // Top folders that hold a steps/ folder.
    let mut tops: Vec<&str> = Vec::new();
    for n in &norm {
        if let Some(i) = n.find("steps/")
            && (i == 0 || n.as_bytes()[i - 1] == b'/')
        {
            tops.push(&n[..i]);
        }
    }
    tops.iter().any(|t| {
        ["misc/info", "symbols/", "fonts/", "matrix/"]
            .iter()
            .any(|k| norm.iter().any(|n| n.starts_with(&format!("{t}{k}"))))
    })
}

/// The entry names of a tar archive's headers, as far as `b` goes (it may be a prefix).
pub fn tar_names(b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 512 <= b.len() && out.len() < 10_000 {
        let h = &b[at..at + 512];
        let ustar = &h[257..262] == b"ustar";
        let known = matches!(h[156], 0 | b'0' | b'5' | b'7' | b'L' | b'x');
        if h.iter().all(|&c| c == 0) || !(ustar || known) {
            break;
        }
        let size = std::str::from_utf8(&h[124..136])
            .ok()
            .map(|s| s.trim_matches(|c: char| c == '\0' || c == ' '))
            .and_then(|s| {
                if s.is_empty() {
                    Some(0)
                } else {
                    usize::from_str_radix(s, 8).ok()
                }
            });
        let Some(size) = size else { break };
        let end = h[0..100].iter().position(|&c| c == 0).unwrap_or(100);
        let mut name = String::from_utf8_lossy(&h[..end]).into_owned();
        if &h[257..262] == b"ustar" {
            let pe = h[345..500].iter().position(|&c| c == 0).unwrap_or(155);
            if pe > 0 {
                name = format!("{}/{name}", String::from_utf8_lossy(&h[345..345 + pe]));
            }
        }
        if h[156] == b'L' && at + 512 + size <= b.len() {
            let d = &b[at + 512..at + 512 + size];
            let e = d.iter().position(|&c| c == 0).unwrap_or(d.len());
            name = String::from_utf8_lossy(&d[..e]).into_owned();
        }
        out.push(name);
        at = at + 512 + size.div_ceil(512) * 512;
    }
    out
}

/// Unpacks a job file stored compressed: `.z` (Unix compress) here; `.gz` is the caller's
/// (gzip). Returns the plain name and bytes, or the input unchanged.
pub fn unpack(name: &str, bytes: Vec<u8>, max: usize) -> Result<(String, Vec<u8>), Error> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".z") && bytes.starts_with(&[0x1F, 0x9D]) {
        let out = lzw::decompress(&bytes, max)?;
        return Ok((name[..name.len() - 2].to_string(), out));
    }
    Ok((name.to_string(), bytes))
}

/// `KEY=VALUE` blocks of a matrix or `stephdr`: (block name, fields).
fn blocks(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out = Vec::new();
    let mut cur: Option<(String, Vec<(String, String)>)> = None;
    for line in text.lines() {
        let l = line.trim();
        if let Some(name) = l.strip_suffix('{') {
            cur = Some((name.trim().to_ascii_uppercase(), Vec::new()));
        } else if l == "}" {
            if let Some(c) = cur.take() {
                out.push(c);
            }
        } else if let (Some(c), Some((k, v))) = (cur.as_mut(), l.split_once('=')) {
            c.1.push((k.trim().to_ascii_uppercase(), v.trim().to_string()));
        }
    }
    out
}

fn field<'b>(f: &'b [(String, String)], k: &str) -> Option<&'b str> {
    f.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

/// One board layer of the matrix.
#[derive(Clone, Debug)]
struct MatLayer {
    row: i64,
    name: String,
    ty: String,
    negative: bool,
}

/// Units of a features file: millimetres per coordinate unit, per symbol unit.
#[derive(Clone, Copy)]
struct Units {
    coord: f64,
    sym: f64,
}

const MM: Units = Units {
    coord: 1.0,
    sym: 0.001,
};
const INCH: Units = Units {
    coord: 25.4,
    sym: 0.0254,
};

fn units_of(text: &str) -> Units {
    for l in text.lines().take(50) {
        let l = l.trim().to_ascii_uppercase();
        let v = l
            .strip_prefix("UNITS=")
            .or_else(|| l.strip_prefix("U "))
            .map(str::trim);
        match v {
            Some("MM") => return MM,
            Some("INCH") => return INCH,
            _ => {}
        }
    }
    INCH
}

fn num(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The numbers after a symbol name's prefix, split at `x`.
fn dims(s: &str) -> Option<Vec<f64>> {
    s.split('x').map(num).collect()
}

/// A standard symbol by name, sizes in symbol units scaled by `k` to mm; `None` for a name
/// that is not a standard symbol.
fn standard(name: &str, k: f64) -> Option<Prim> {
    let n = name.to_ascii_lowercase();
    let d = |rest: &str| dims(rest).map(|v| v.into_iter().map(|x| x * k).collect::<Vec<_>>());
    let p = if let Some(r) = n.strip_prefix("rect") {
        // rect<w>x<h>[xr<rad>[x<corners>]|xc<cut>[x<corners>]]
        let (base, mode) = match r.find("xr").or_else(|| r.find("xc")) {
            Some(i) => (&r[..i], Some(&r[i + 1..])),
            None => (r, None),
        };
        let v = d(base)?;
        let (w, h) = (*v.first()?, *v.get(1)?);
        match mode {
            Some(m) if m.starts_with('r') => {
                let rad = num(m[1..].split('x').next()?)? * k;
                Prim::RoundRect(w, h, rad)
            }
            Some(m) => {
                let cut = num(m[1..].split('x').next()?)? * k;
                Prim::Chamfer(w, h, cut)
            }
            None => Prim::Rect(w, h),
        }
    } else if let Some(r) = n.strip_prefix("oval_h") {
        let v = d(r)?;
        Prim::Hexagon(*v.first()?, *v.get(1)?, v.get(1)? / 2.0)
    } else if let Some(r) = n.strip_prefix("oval") {
        let v = d(r)?;
        Prim::Oval(*v.first()?, *v.get(1)?)
    } else if let Some(r) = n.strip_prefix("donut_r") {
        let v = d(r)?;
        Prim::Donut(*v.first()?, *v.get(1)?)
    } else if let Some(r) = n.strip_prefix("donut_s") {
        let v = d(r)?;
        Prim::SquareDonut(*v.first()?, *v.get(1)?)
    } else if let Some(r) = n.strip_prefix("di") {
        let v = d(r)?;
        Prim::Diamond(*v.first()?, *v.get(1)?)
    } else if let Some(r) = n.strip_prefix("oct") {
        let v = d(r)?;
        Prim::Octagon(*v.first()?, *v.get(1)?, *v.get(2)?)
    } else if let Some(r) = n.strip_prefix("hex_l") {
        let v = d(r)?;
        Prim::Hexagon(*v.first()?, *v.get(1)?, *v.get(2)?)
    } else if let Some(r) = n.strip_prefix("hex_s") {
        // Pointy top and bottom: a hex_l turned a quarter.
        let v = d(r)?;
        let p = Prim::Hexagon(*v.get(1)?, *v.first()?, *v.get(2)?).outline();
        Prim::Path(Affine::rotate(std::f64::consts::FRAC_PI_2) * p)
    } else if let Some(r) = n.strip_prefix("el") {
        let v = d(r)?;
        Prim::Ellipse(*v.first()?, *v.get(1)?)
    } else if let Some(r) = n.strip_prefix("tri") {
        let v = d(r)?;
        Prim::Triangle(*v.first()?, *v.get(1)?)
    } else if let Some(r) = n.strip_prefix("thr") {
        // thr<od>x<id>x<angle>x<spokes>x<gap>: angle and count are not sizes.
        let v = dims(r)?;
        Prim::Thermal(
            *v.first()? * k,
            *v.get(1)? * k,
            *v.get(2)?,
            *v.get(3)? as u32,
            *v.get(4)? * k,
        )
    } else if let Some(r) = n.strip_prefix("hole") {
        Prim::Circle(*d(r)?.first()?)
    } else if let Some(r) = n.strip_prefix("r") {
        Prim::Circle(*d(r)?.first()?)
    } else {
        let side = *d(n.strip_prefix("s")?)?.first()?;
        Prim::Rect(side, side)
    };
    Some(p)
}

/// A features file's symbol: standard, a user symbol, or unknown.
#[derive(Clone, Debug)]
enum Sym {
    Std(Prim),
    /// A user symbol's objects at the origin (mm).
    User(std::sync::Arc<Vec<Object>>),
    Unknown(String),
}

/// What reading a job's features found.
#[derive(Default)]
struct Tally {
    pads: u64,
    lines: u64,
    arcs: u64,
    surfaces: u64,
    texts: u64,
    barcodes: u64,
    /// Characters no font had.
    missing_chars: u64,
    /// Fonts the text names that the job lacks.
    missing_fonts: Vec<String>,
    unknown_symbols: Vec<String>,
}

struct Reader<'j, 'a> {
    job: &'j Job<'a>,
    /// User symbols read so far (mm, at the origin).
    user: HashMap<String, Option<std::sync::Arc<Vec<Object>>>>,
    tally: Tally,
    limits: Limits,
    items: usize,
    warnings: Vec<String>,
    /// The job's fonts read so far (`None`: not in the job).
    fonts: HashMap<String, Option<std::sync::Arc<StrokeFont>>>,
    /// Each step's layers as read, with their step-and-repeat copies (mm).
    steps: HashMap<(String, String), std::sync::Arc<Vec<Object>>>,
    /// Step-and-repeat copies made so far.
    copies: usize,
}

/// The text of a features-like file: `name` or its unpacked twin.
fn text_of(job: &Job<'_>, path: &str) -> Option<String> {
    job.get(path)
        .map(|b| String::from_utf8_lossy(b).into_owned())
}

impl Reader<'_, '_> {
    /// A symbol's prim (mm) or user objects; `units_sym` scales sizes; a trailing ` M`/` I`
    /// on the definition overrides it.
    fn symbol(&mut self, def: &str, u: Units, depth: u32) -> Sym {
        let mut parts = def.split_whitespace();
        let name = parts.next().unwrap_or("");
        let k = match parts.next().map(str::to_ascii_uppercase).as_deref() {
            Some("M") => 0.001,
            Some("I") => 0.0254,
            _ => u.sym,
        };
        if let Some(p) = standard(name, k) {
            return Sym::Std(p);
        }
        let key = name.to_ascii_lowercase();
        if let Some(hit) = self.user.get(&key) {
            return hit
                .clone()
                .map_or_else(|| Sym::Unknown(name.into()), Sym::User);
        }
        let mut made = None;
        if depth < MAX_DEPTH
            && let Some(t) = text_of(self.job, &format!("symbols/{key}/features"))
        {
            let objs = self.features(&t, depth + 1);
            made = Some(std::sync::Arc::new(objs));
        }
        self.user.insert(key, made.clone());
        made.map_or_else(|| Sym::Unknown(name.into()), Sym::User)
    }

    fn unknown(&mut self, name: &str) {
        if !self.tally.unknown_symbols.iter().any(|s| s == name)
            && self.tally.unknown_symbols.len() < 32
        {
            self.tally.unknown_symbols.push(name.to_string());
        }
    }

    fn push(&mut self, out: &mut Vec<Object>, o: Object) {
        if self.items >= self.limits.items {
            return;
        }
        self.items += 1;
        out.push(o);
    }

    /// The objects of a features file (mm).
    fn features(&mut self, text: &str, depth: u32) -> Vec<Object> {
        let u = units_of(text);
        let mut syms: Vec<(usize, String)> = Vec::new();
        let mut out = Vec::new();
        let mut contour: Option<(Contour, bool)> = None;
        let pt = |x: &str, y: &str| -> Option<Point> {
            Some(Point::new(num(x)? * u.coord, num(y)? * u.coord))
        };
        let mut resolved: HashMap<usize, Sym> = HashMap::new();
        for line in text.lines() {
            let line = line.split(';').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let t: Vec<&str> = line.split_whitespace().collect();
            let rec = t[0];
            if let Some(n) = rec.strip_prefix('$') {
                if let Ok(i) = n.parse::<usize>() {
                    syms.push((i, t[1..].join(" ")));
                }
                continue;
            }
            match rec {
                "P" if t.len() >= 6 => {
                    // P x y apt_def pol dcode orient_def; apt_def may be "-1 sym resize".
                    let Some(at) = pt(t[1], t[2]) else { continue };
                    let (sym, resize, rest) = if t[3] == "-1" && t.len() >= 8 {
                        (t[4], num(t[5]).map(|r| r / 1000.0), &t[6..])
                    } else {
                        (t[3], None, &t[4..])
                    };
                    let dark = rest.first() != Some(&"N");
                    let (rot, mirror) = orient(rest.get(2..).unwrap_or(&[]));
                    self.tally.pads += 1;
                    let Ok(si) = sym.parse::<usize>() else {
                        continue;
                    };
                    let s = self.resolve(&mut resolved, &syms, si, u, depth);
                    match s {
                        Sym::Std(p) => {
                            let p = match resize {
                                Some(f) if f > 0.0 => Prim::Path(Affine::scale(f) * p.outline()),
                                _ => p,
                            };
                            let o = features::flash(&p, at, -rot, mirror, dark);
                            self.push(&mut out, o);
                        }
                        Sym::User(objs) => {
                            let mut a =
                                Affine::translate(at.to_vec2()) * Affine::rotate(-rot.to_radians());
                            if mirror {
                                a *= Affine::FLIP_X;
                            }
                            for o in objs.iter() {
                                let mut q = o.clone();
                                q.path = crate::geom::transformed(&o.path, a);
                                q.segment = o.segment.map(|s| s.transformed(a));
                                // A negative pad inverts the symbol.
                                q.dark = o.dark == dark;
                                self.push(&mut out, q);
                            }
                        }
                        Sym::Unknown(n) => self.unknown(&n),
                    }
                }
                "L" if t.len() >= 7 => {
                    let (Some(a), Some(b)) = (pt(t[1], t[2]), pt(t[3], t[4])) else {
                        continue;
                    };
                    self.tally.lines += 1;
                    let dark = t[6] != "N";
                    if let Some(pen) = self.pen(&mut resolved, &syms, t[5], u, depth) {
                        let o = features::line(a, b, pen, dark);
                        self.push(&mut out, o);
                    }
                }
                "A" if t.len() >= 11 => {
                    let (Some(a), Some(b), Some(c)) =
                        (pt(t[1], t[2]), pt(t[3], t[4]), pt(t[5], t[6]))
                    else {
                        continue;
                    };
                    self.tally.arcs += 1;
                    let dark = t[8] != "N";
                    let cw = t[10].eq_ignore_ascii_case("Y");
                    if let Some(pen) = self.pen(&mut resolved, &syms, t[7], u, depth) {
                        let o = features::arc(a, b, c, cw, pen, dark);
                        self.push(&mut out, o);
                    }
                }
                "T" => {
                    self.tally.texts += 1;
                    for o in self.text(line, u) {
                        self.push(&mut out, o);
                    }
                }
                "B" => self.tally.barcodes += 1,
                "S" if t.len() >= 2 => {
                    contour = Some((Contour::default(), t[1] != "N"));
                }
                "OB" if t.len() >= 4 => {
                    if let (Some((c, _)), Some(p)) = (contour.as_mut(), pt(t[1], t[2])) {
                        c.begin(p, t[3].eq_ignore_ascii_case("H"));
                    }
                }
                "OS" if t.len() >= 3 => {
                    if let (Some((c, _)), Some(p)) = (contour.as_mut(), pt(t[1], t[2])) {
                        c.line_to(p);
                    }
                }
                "OC" if t.len() >= 6 => {
                    if let (Some((c, _)), Some(p), Some(cc)) =
                        (contour.as_mut(), pt(t[1], t[2]), pt(t[3], t[4]))
                    {
                        c.arc_to(p, cc, t[5].eq_ignore_ascii_case("Y"));
                    }
                }
                "OE" => {
                    if let Some((c, _)) = contour.as_mut() {
                        c.end();
                    }
                }
                "SE" => {
                    if let Some((c, dark)) = contour.take() {
                        self.tally.surfaces += 1;
                        if let Some(o) = c.finish(dark) {
                            self.push(&mut out, o);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The job's font `name` (`fonts/<name>`), read once.
    fn font(&mut self, name: &str) -> Option<std::sync::Arc<StrokeFont>> {
        let key = name.to_ascii_lowercase();
        if let Some(f) = self.fonts.get(&key) {
            return f.clone();
        }
        let f = text_of(self.job, &format!("fonts/{key}"))
            .and_then(|t| StrokeFont::odb(&t))
            .map(std::sync::Arc::new);
        if f.is_none()
            && !self.tally.missing_fonts.contains(&key)
            && self.tally.missing_fonts.len() < 8
        {
            self.tally.missing_fonts.push(key.clone());
        }
        self.fonts.insert(key, f.clone());
        f
    }

    /// A text record: `T x y font polarity orient_def xsize ysize width_factor 'text'
    /// version`, the text's lower-left corner at x y; orient_def is one number, or two
    /// for any angle (8, 9). The strokes are `width_factor` × 12 mil wide.
    fn text(&mut self, line: &str, u: Units) -> Vec<Object> {
        let (head, quoted) = match (line.find('\''), line.rfind('\'')) {
            (Some(a), Some(b)) if b > a => (&line[..a], &line[a + 1..b]),
            _ => return Vec::new(),
        };
        let t: Vec<&str> = head.split_whitespace().collect();
        if t.len() < 9 {
            return Vec::new();
        }
        let two = matches!(t[5], "8" | "9");
        let rest = if two { &t[7..] } else { &t[6..] };
        let (Some(x), Some(y), Some(xs), Some(ys)) = (
            num(t[1]),
            num(t[2]),
            rest.first().and_then(|s| num(s)),
            rest.get(1).and_then(|s| num(s)),
        ) else {
            return Vec::new();
        };
        let wf = rest.get(2).and_then(|s| num(s)).unwrap_or(1.0).max(0.0);
        let dark = t[4] != "N";
        let (rot, mirror) = orient(&t[5..]);
        let mut place =
            Affine::translate((x * u.coord, y * u.coord)) * Affine::rotate(-rot.to_radians());
        if mirror {
            place *= Affine::FLIP_X;
        }
        let job_font = self.font(t[3]);
        let font = job_font.as_deref().unwrap_or(crate::text::builtin());
        let (cw, ch) = (xs * u.coord, ys * u.coord);
        let gap = font.offset * cw / font.xsize;
        font.draw(
            quoted,
            cw,
            ch,
            gap,
            wf * 12.0 * 0.0254,
            place,
            dark,
            &mut self.tally.missing_chars,
        )
    }

    fn resolve(
        &mut self,
        cache: &mut HashMap<usize, Sym>,
        syms: &[(usize, String)],
        i: usize,
        u: Units,
        depth: u32,
    ) -> Sym {
        if let Some(s) = cache.get(&i) {
            return s.clone();
        }
        let s = match syms.iter().find(|(n, _)| *n == i) {
            Some((_, def)) => self.symbol(def, u, depth),
            None => Sym::Unknown(format!("${i}")),
        };
        cache.insert(i, s.clone());
        s
    }

    /// The pen of a line or arc: a round or square standard symbol.
    fn pen(
        &mut self,
        cache: &mut HashMap<usize, Sym>,
        syms: &[(usize, String)],
        sym: &str,
        u: Units,
        depth: u32,
    ) -> Option<Pen> {
        let i = sym.parse::<usize>().ok()?;
        match self.resolve(cache, syms, i, u, depth) {
            Sym::Std(Prim::Circle(d)) => Some(Pen::Round(d)),
            Sym::Std(Prim::Rect(w, h)) => Some(Pen::Square(w.max(h))),
            Sym::Std(p) => {
                // Other pens draw as round of their width.
                let b = p.outline().bounding_box();
                Some(Pen::Round(b.width().min(b.height())))
            }
            Sym::Unknown(n) => {
                self.unknown(&n);
                None
            }
            Sym::User(_) => Some(Pen::Round(0.0)),
        }
    }
}

/// `orient_def`: 0–7 are quarter turns (4–7 mirrored); 8 and 9 take an angle (9 mirrored).
/// Angles are clockwise, in degrees.
fn orient(t: &[&str]) -> (f64, bool) {
    match t.first().and_then(|s| s.parse::<i32>().ok()) {
        Some(8) => (t.get(1).and_then(|s| num(s)).unwrap_or(0.0), false),
        Some(9) => (t.get(1).and_then(|s| num(s)).unwrap_or(0.0), true),
        Some(n @ 0..=7) => (f64::from(n % 4) * 90.0, n >= 4),
        _ => (0.0, false),
    }
}

/// The step to show: one named like a board ("pcb"), else one without step-and-repeat, else
/// the first.
fn choose_step(job: &Job<'_>, steps: &[String]) -> Option<String> {
    let board = |s: &&String| {
        let l = s.to_ascii_lowercase();
        l == "pcb" || l == "board" || l == "single" || l.starts_with("pcb")
    };
    if let Some(s) = steps.iter().find(board) {
        return Some(s.clone());
    }
    let repeats = |s: &String| {
        text_of(job, &format!("steps/{s}/stephdr"))
            .is_some_and(|t| t.to_ascii_uppercase().contains("STEP-REPEAT"))
    };
    steps
        .iter()
        .find(|s| !repeats(s))
        .or(steps.first())
        .cloned()
}

/// Roles of the board layers in matrix order.
fn roles(layers: &[MatLayer]) -> Vec<Role> {
    let signal = |l: &MatLayer| matches!(l.ty.as_str(), "SIGNAL" | "POWER_GROUND" | "MIXED");
    let first = layers.iter().position(signal);
    let last = layers.iter().rposition(signal);
    let nsig = layers.iter().filter(|l| signal(l)).count();
    let mut inner = 0;
    // Layers above the first signal layer are the top's, below the last the bottom's; in
    // between, the nearer side's.
    let side = |i: usize| match (first, last) {
        (Some(f), Some(l)) if i > l || (i > f && i - f > l - i) => Side::Bottom,
        _ => Side::Top,
    };
    layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let lname = l.name.to_ascii_lowercase();
            match l.ty.as_str() {
                "SIGNAL" | "POWER_GROUND" | "MIXED" => {
                    if Some(i) == first {
                        Role::TopCopper
                    } else if Some(i) == last && nsig > 1 {
                        Role::BottomCopper
                    } else {
                        inner += 1;
                        Role::InnerCopper(inner)
                    }
                }
                "SOLDER_MASK" | "MASK" => Role::SolderMask(side(i)),
                "SILK_SCREEN" => Role::Silkscreen(side(i)),
                "SOLDER_PASTE" | "CONDUCTIVE_PASTE" => Role::Paste(side(i)),
                "DRILL" => Role::Drill(
                    if lname.contains("npth")
                        || lname.contains("non_plated")
                        || lname.contains("non-plated")
                    {
                        Plating::NonPlated
                    } else {
                        Plating::Plated
                    },
                ),
                "ROUT" | "DOCUMENT" => Role::Mechanical,
                _ => Role::Other,
            }
        })
        .collect()
}

/// Holes and slots of a drill layer's objects (pads and lines with round symbols).
fn drill_of(objs: &[Object]) -> Drill {
    let mut d = Drill::default();
    let tool_of = |dia: f64, d: &mut Drill| -> usize {
        let key = (dia * 1e4).round();
        if let Some(i) = d
            .tools
            .iter()
            .position(|t| (t.diameter * 1e4).round() == key)
        {
            return i;
        }
        d.tools.push(Tool {
            number: d.tools.len() as u32 + 1,
            diameter: dia,
            plated: None,
        });
        d.tools.len() - 1
    };
    for o in objs {
        let b = o.path.bounding_box();
        match o.segment {
            Some(seg) => {
                // A routed slot: the band's width is the tool.
                let w = match seg {
                    Segment::Line(a, c) => {
                        let len = (c - a).hypot();
                        (o.path.area().abs() / len.max(1e-9)).min(b.width().min(b.height()))
                    }
                    Segment::Arc { .. } => b.width().min(b.height()) / 4.0,
                };
                let t = tool_of(w, &mut d);
                d.slots.push(Slot {
                    segment: seg,
                    tool: t,
                });
            }
            None => {
                let t = tool_of(b.width().min(b.height()), &mut d);
                d.holes.push(Hole {
                    at: b.center(),
                    tool: t,
                });
            }
        }
    }
    d.bounds = features::bounds(objs);
    d
}

/// Package outlines from `eda/data`: `PKG name pitch xmin ymin xmax ymax` in order.
fn packages(text: &str) -> Vec<Rect> {
    let u = units_of(text);
    let mut out = Vec::new();
    for l in text.lines() {
        let t: Vec<&str> = l
            .split(';')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        if t.first() == Some(&"PKG") && t.len() >= 7 {
            let v: Option<Vec<f64>> = t[3..7].iter().map(|s| num(s)).collect();
            out.push(v.map_or(Rect::ZERO, |v| {
                Rect::new(
                    v[0] * u.coord,
                    v[1] * u.coord,
                    v[2] * u.coord,
                    v[3] * u.coord,
                )
            }));
        }
    }
    out
}

/// Components of a `components` file: `CMP pkg x y rot mirror refdes part`.
fn components(text: &str, side: Side, pkgs: &[Rect]) -> Vec<Component> {
    let u = units_of(text);
    let mut out = Vec::new();
    for l in text.lines() {
        let t: Vec<&str> = l
            .split(';')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        if t.first() != Some(&"CMP") || t.len() < 7 {
            continue;
        }
        let (Some(x), Some(y), Some(rot)) = (num(t[2]), num(t[3]), num(t[4])) else {
            continue;
        };
        let at = Point::new(x * u.coord, y * u.coord);
        let mirror = t[5].eq_ignore_ascii_case("M");
        // Clockwise in ODB++.
        let rotation = -rot;
        let outline = t[1]
            .parse::<usize>()
            .ok()
            .and_then(|i| pkgs.get(i))
            .filter(|r| r.area() > 0.0)
            .map(|r| {
                let mut a = Affine::translate(at.to_vec2()) * Affine::rotate(rotation.to_radians());
                if mirror {
                    a *= Affine::FLIP_X;
                }
                a * r.to_path(0.1)
            });
        out.push(Component {
            refdes: t[6].to_string(),
            at,
            rotation,
            side,
            outline,
        });
    }
    out
}

/// `c` on a repeated board placed by `a`: turned `turn` degrees counter-clockwise, after
/// mirroring in x when `mirror`.
fn placed(c: &Component, a: Affine, turn: f64, mirror: bool) -> Component {
    let rotation = if mirror {
        turn + 180.0 - c.rotation
    } else {
        turn + c.rotation
    };
    Component {
        refdes: c.refdes.clone(),
        at: a * c.at,
        rotation: rotation.rem_euclid(360.0),
        side: c.side,
        outline: c.outline.as_ref().map(|o| a * o.clone()),
    }
}

/// Step-and-repeat nesting followed at most.
const MAX_STEP_DEPTH: u32 = 4;

/// One `STEP-REPEAT` of a `stephdr`: `NX` × `NY` copies of step `NAME`, the first with its
/// datum at `X Y`, the next `DX`/`DY` on, turned `ANGLE` degrees clockwise, mirrored first
/// when `MIRROR`.
#[derive(Clone, Debug, PartialEq)]
struct Repeat {
    name: String,
    at: Point,
    step: (f64, f64),
    count: (u32, u32),
    angle: f64,
    mirror: bool,
}

/// A field of a `KEY=VALUE` file outside any block.
fn top_field(text: &str, key: &str) -> Option<String> {
    let mut depth = 0i32;
    for line in text.lines() {
        let l = line.trim();
        if l.ends_with('{') {
            depth += 1;
        } else if l == "}" {
            depth -= 1;
        } else if depth == 0
            && let Some((k, v)) = l.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

/// A step's datum and its step-and-repeat table (mm).
fn step_header(job: &Job<'_>, step: &str) -> (Point, Vec<Repeat>) {
    let Some(t) = text_of(job, &format!("steps/{step}/stephdr")) else {
        return (Point::ZERO, Vec::new());
    };
    let u = units_of(&t);
    let g = |k: &str| top_field(&t, k).and_then(|v| num(&v)).unwrap_or(0.0) * u.coord;
    let datum = Point::new(g("X_DATUM"), g("Y_DATUM"));
    let reps = blocks(&t)
        .into_iter()
        .filter(|(n, _)| n == "STEP-REPEAT")
        .filter_map(|(_, f)| {
            let v = |k: &str| field(&f, k).and_then(num);
            let n = |k: &str| v(k).map_or(1, |x| x.clamp(0.0, 1e6) as u32);
            Some(Repeat {
                name: field(&f, "NAME")?.to_string(),
                at: Point::new(
                    v("X").unwrap_or(0.0) * u.coord,
                    v("Y").unwrap_or(0.0) * u.coord,
                ),
                step: (
                    v("DX").unwrap_or(0.0) * u.coord,
                    v("DY").unwrap_or(0.0) * u.coord,
                ),
                count: (n("NX"), n("NY")),
                angle: v("ANGLE").unwrap_or(0.0),
                mirror: field(&f, "MIRROR").is_some_and(|m| m.eq_ignore_ascii_case("YES")),
            })
        })
        .collect();
    (datum, reps)
}

impl Reader<'_, '_> {
    /// A step's layer (mm): its own features, then its step-and-repeat copies of other
    /// steps' same layer. Read once per step and layer.
    fn step_layer(&mut self, step: &str, lname: &str, depth: u32) -> std::sync::Arc<Vec<Object>> {
        let key = (step.to_ascii_lowercase(), lname.to_string());
        if let Some(o) = self.steps.get(&key) {
            return o.clone();
        }
        let mut objs = text_of(self.job, &format!("steps/{step}/layers/{lname}/features"))
            .map(|t| self.features(&t, 0))
            .unwrap_or_default();
        if depth < MAX_STEP_DEPTH {
            let (_, reps) = step_header(self.job, step);
            for rep in reps {
                let child = self.step_layer(&rep.name, lname, depth + 1);
                if child.is_empty() {
                    continue;
                }
                let (datum, _) = step_header(self.job, &rep.name);
                let mut base = Affine::rotate(-rep.angle.to_radians());
                if rep.mirror {
                    base *= Affine::FLIP_X;
                }
                base *= Affine::translate(-datum.to_vec2());
                'copies: for j in 0..rep.count.1 {
                    for i in 0..rep.count.0 {
                        if self.copies >= self.limits.copies || self.items >= self.limits.items {
                            warn(
                                &mut self.warnings,
                                "The panel has more step-and-repeat copies than are drawn.",
                            );
                            break 'copies;
                        }
                        self.copies += 1;
                        let at = rep.at
                            + specular_vector::kurbo::Vec2::new(
                                f64::from(i) * rep.step.0,
                                f64::from(j) * rep.step.1,
                            );
                        let a = Affine::translate(at.to_vec2()) * base;
                        for o in child.iter() {
                            let q = o.transformed(a);
                            self.push(&mut objs, q);
                        }
                    }
                }
            }
        }
        let objs = std::sync::Arc::new(objs);
        self.steps.insert(key, objs.clone());
        objs
    }

    /// A step's profile (its board's shape), not counted with the layers' features.
    fn profile(&mut self, step: &str) -> Option<BezPath> {
        let saved = std::mem::take(&mut self.tally);
        let shape = text_of(self.job, &format!("steps/{step}/profile")).and_then(|t| {
            let objs = self.features(&t, 0);
            let mut p = BezPath::new();
            for o in objs.iter().filter(|o| o.dark) {
                p.extend(o.path.elements().iter().copied());
            }
            (!p.elements().is_empty()).then_some(p)
        });
        self.tally = saved;
        shape
    }

    /// The profiles of the boards a step repeats (a panel's boards), placed as its
    /// step-and-repeat table says, nested steps' too.
    fn repeated_profiles(&mut self, step: &str, depth: u32, out: &mut Vec<BezPath>) {
        if depth >= MAX_STEP_DEPTH {
            return;
        }
        let (_, reps) = step_header(self.job, step);
        for rep in reps {
            let mut child: Vec<BezPath> = self.profile(&rep.name).into_iter().collect();
            self.repeated_profiles(&rep.name, depth + 1, &mut child);
            if child.is_empty() {
                continue;
            }
            let (datum, _) = step_header(self.job, &rep.name);
            let mut base = Affine::rotate(-rep.angle.to_radians());
            if rep.mirror {
                base *= Affine::FLIP_X;
            }
            base *= Affine::translate(-datum.to_vec2());
            for j in 0..rep.count.1 {
                for i in 0..rep.count.0 {
                    if out.len() >= self.limits.copies {
                        return;
                    }
                    let at = rep.at
                        + specular_vector::kurbo::Vec2::new(
                            f64::from(i) * rep.step.0,
                            f64::from(j) * rep.step.1,
                        );
                    let a = Affine::translate(at.to_vec2()) * base;
                    out.extend(child.iter().map(|p| a * p.clone()));
                }
            }
        }
    }

    /// A step's components (from its component layers), then those of the steps it repeats,
    /// once per copy, moved, turned and mirrored as its step-and-repeat table says.
    fn step_components(&mut self, step: &str, layers: &[MatLayer], depth: u32) -> Vec<Component> {
        let job = self.job;
        let pkgs = text_of(job, &format!("steps/{step}/eda/data"))
            .map(|t| packages(&t))
            .unwrap_or_default();
        let mut comps = Vec::new();
        for l in layers.iter().filter(|l| l.ty == "COMPONENT") {
            let lname = l.name.to_ascii_lowercase();
            if let Some(t) = text_of(job, &format!("steps/{step}/layers/{lname}/components")) {
                let side = if lname.contains("bot") {
                    Side::Bottom
                } else {
                    Side::Top
                };
                comps.extend(components(&t, side, &pkgs));
            }
        }
        if depth >= MAX_STEP_DEPTH {
            return comps;
        }
        let (_, reps) = step_header(job, step);
        for rep in reps {
            let child = self.step_components(&rep.name, layers, depth + 1);
            if child.is_empty() {
                continue;
            }
            let (datum, _) = step_header(job, &rep.name);
            let mut base = Affine::rotate(-rep.angle.to_radians());
            if rep.mirror {
                base *= Affine::FLIP_X;
            }
            base *= Affine::translate(-datum.to_vec2());
            'copies: for j in 0..rep.count.1 {
                for i in 0..rep.count.0 {
                    if comps.len() + child.len() > self.limits.items {
                        warn(
                            &mut self.warnings,
                            "The panel has more components than are listed.",
                        );
                        break 'copies;
                    }
                    let at = rep.at
                        + specular_vector::kurbo::Vec2::new(
                            f64::from(i) * rep.step.0,
                            f64::from(j) * rep.step.1,
                        );
                    let a = Affine::translate(at.to_vec2()) * base;
                    comps.extend(child.iter().map(|c| placed(c, a, -rep.angle, rep.mirror)));
                }
            }
        }
        comps
    }

    /// A step's board: its layers in matrix order, components and profile; `None` when no
    /// layer has features.
    fn board(&mut self, step: &str, layers: &[MatLayer], roles: &[Role]) -> Option<StepBoard> {
        let job = self.job;
        // The board's shape, and the outlines of the boards it repeats.
        let shape = self.profile(step);
        let mut outlines = Vec::new();
        self.repeated_profiles(step, 0, &mut outlines);
        let mut out = Vec::new();
        // Its own components and those of the boards it repeats, each placed.
        let comps = self.step_components(step, layers, 0);
        for (l, role) in layers.iter().zip(roles) {
            let lname = l.name.to_ascii_lowercase();
            if l.ty == "COMPONENT" {
                continue;
            }
            let objs = self.step_layer(step, &lname, 0);
            // A layer whose features file is empty is still listed.
            if objs.is_empty()
                && job
                    .get(&format!("steps/{step}/layers/{lname}/features"))
                    .is_none()
            {
                continue;
            }
            if role.is_drill() {
                out.push(stack::drill_layer(l.name.clone(), *role, drill_of(&objs)));
                continue;
            }
            let mut objs = objs.as_ref().clone();
            if l.negative {
                negate(&mut objs, shape.as_ref());
            }
            let b = features::bounds(&objs);
            out.push(stack::gerber_layer(l.name.clone(), *role, objs, b));
        }
        (!out.is_empty()).then_some(StepBoard {
            layers: out,
            components: comps,
            shape,
            outlines,
        })
    }
}

/// A negative layer as drawn: the step's profile (else the features' box, 1 mm larger)
/// dark under the features, each feature's polarity turned over.
fn negate(objs: &mut Vec<Object>, profile: Option<&BezPath>) {
    let back = match profile {
        Some(p) => p.clone(),
        None => match features::bounds(objs) {
            Some(b) => crate::geom::rect(b.center(), b.width() + 2.0, b.height() + 2.0),
            None => return,
        },
    };
    for o in objs.iter_mut() {
        o.dark = !o.dark;
    }
    objs.insert(
        0,
        Object {
            path: back,
            dark: true,
            kind: crate::gerber::ObjectKind::Region,
            segment: None,
            holes: true,
        },
    );
}

/// One step's layers, components and shape.
struct StepBoard {
    layers: Vec<stack::LayerFile>,
    components: Vec<Component>,
    shape: Option<BezPath>,
    /// The repeated boards' outlines (a panel's).
    outlines: Vec<BezPath>,
}

/// Reads a job into a board scene: the chosen step's views first, then each other step's
/// (named by the step).
pub fn scene(files: &[(String, Vec<u8>)], limits: &Limits) -> Result<VectorScene, Error> {
    let job = Job::new(files);
    let matrix = text_of(&job, "matrix/matrix")
        .ok_or_else(|| Error::Malformed("the job has no matrix/matrix file".into()))?;
    let bl = blocks(&matrix);
    let mut steps: Vec<(i64, String)> = bl
        .iter()
        .filter(|(n, _)| n == "STEP")
        .filter_map(|(_, f)| {
            Some((
                field(f, "COL").and_then(|c| c.parse().ok()).unwrap_or(0),
                field(f, "NAME")?.to_string(),
            ))
        })
        .collect();
    steps.sort();
    let steps: Vec<String> = steps.into_iter().map(|s| s.1).collect();
    let mut layers: Vec<MatLayer> = bl
        .iter()
        .filter(|(n, f)| {
            n == "LAYER" && field(f, "CONTEXT").is_some_and(|c| c.eq_ignore_ascii_case("BOARD"))
        })
        .filter_map(|(_, f)| {
            Some(MatLayer {
                row: field(f, "ROW").and_then(|r| r.parse().ok()).unwrap_or(0),
                name: field(f, "NAME")?.to_string(),
                ty: field(f, "TYPE").unwrap_or("").to_ascii_uppercase(),
                negative: field(f, "POLARITY").is_some_and(|p| p.eq_ignore_ascii_case("NEGATIVE")),
            })
        })
        .collect();
    layers.sort_by_key(|l| l.row);
    let step = choose_step(&job, &steps)
        .ok_or_else(|| Error::Malformed("the job's matrix names no step".into()))?;
    if !job.has_prefix(&format!("steps/{step}/")) {
        return Err(Error::Malformed(format!(
            "the job has no files for step {step}"
        )));
    }
    let mut r = Reader {
        job: &job,
        user: HashMap::new(),
        tally: Tally::default(),
        limits: *limits,
        items: 0,
        warnings: Vec::new(),
        fonts: HashMap::new(),
        steps: HashMap::new(),
        copies: 0,
    };
    let roles = roles(&layers);
    let main = r.board(&step, &layers, &roles).ok_or_else(|| {
        Error::Malformed(format!("step {step} has no board layers with features"))
    })?;
    let t = std::mem::take(&mut r.tally);
    // The other steps, in matrix order, each with views of its own.
    let mut others = Vec::new();
    for s in steps.iter().filter(|s| **s != step) {
        if job.has_prefix(&format!("steps/{s}/"))
            && let Some(b) = r.board(s, &layers, &roles)
        {
            others.push((s.clone(), b));
        }
    }
    let late = std::mem::take(&mut r.tally);
    let mut names = t.unknown_symbols.clone();
    names.extend(late.unknown_symbols.iter().cloned());
    names.dedup();
    if !names.is_empty() {
        warn(
            &mut r.warnings,
            format!(
                "Symbols not drawn (not standard and not in the job): {}",
                names.join(", ")
            ),
        );
    }
    let mut fonts = t.missing_fonts.clone();
    fonts.extend(late.missing_fonts.iter().cloned());
    fonts.dedup();
    if !fonts.is_empty() {
        warn(
            &mut r.warnings,
            format!(
                "Fonts not in the job, text drawn in the built-in font: {}",
                fonts.join(", ")
            ),
        );
    }
    if t.missing_chars + late.missing_chars > 0 {
        warn(
            &mut r.warnings,
            format!(
                "{} characters of text are in no font and are left out.",
                t.missing_chars + late.missing_chars
            ),
        );
    }
    if t.barcodes > 0 {
        warn(
            &mut r.warnings,
            format!("{} barcode features are not drawn.", t.barcodes),
        );
    }
    if r.items >= limits.items {
        warn(
            &mut r.warnings,
            "The board has more features than are drawn.",
        );
    }
    let info = text_of(&job, "misc/info").unwrap_or_default();
    let info_field = |k: &str| {
        info.lines().find_map(|l| {
            let (a, b) = l.split_once('=')?;
            a.trim()
                .eq_ignore_ascii_case(k)
                .then(|| b.trim().to_string())
        })
    };
    let counts = [
        ("Pad", t.pads),
        ("Line", t.lines),
        ("Arc", t.arcs),
        ("Surface", t.surfaces),
        ("Text", t.texts),
        ("Barcode", t.barcodes),
    ]
    .into_iter()
    .filter(|(_, n)| *n > 0)
    .map(|(k, n)| (k.to_string(), n))
    .collect();
    let extra = Extra {
        format: "ODB++".into(),
        version: info_field("ODB_VERSION_MAJOR")
            .map(|m| match info_field("ODB_VERSION_MINOR") {
                Some(n) => format!("{m}.{n}"),
                None => m,
            })
            .unwrap_or_default(),
        kind: Some(PcbKind::Odb),
        shape: main.shape,
        outlines: main.outlines,
        components: main.components,
        generator: info_field("ODB_SOURCE").or_else(|| info_field("SAVE_APP")),
        project: info_field("JOB_NAME"),
        counts,
        ..Extra::default()
    };
    let warnings = std::mem::take(&mut r.warnings);
    let mut scene = stack::board_of(main.layers, warnings, extra);
    if !others.is_empty() {
        stack::name_step(&mut scene, &step);
    }
    for (name, b) in others {
        let extra = Extra {
            format: "ODB++".into(),
            kind: Some(PcbKind::Odb),
            shape: b.shape,
            outlines: b.outlines,
            components: b.components,
            ..Extra::default()
        };
        let other = stack::board_of(b.layers, Vec::new(), extra);
        stack::add_step(&mut scene, other, &name);
    }
    Ok(scene)
}

/// The regular files of a tar archive (ustar, GNU long names, pax `path`), at most
/// `max_total` bytes in all; names as stored.
pub fn untar(b: &[u8], max_total: usize) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut out = Vec::new();
    let mut at = 0usize;
    let mut total = 0usize;
    let mut long: Option<String> = None;
    let octal = |f: &[u8]| -> Option<usize> {
        let s = std::str::from_utf8(f).ok()?;
        let s = s.trim_matches(|c: char| c == '\0' || c == ' ');
        if s.is_empty() {
            return Some(0);
        }
        usize::from_str_radix(s, 8).ok()
    };
    let cstr = |f: &[u8]| -> String {
        let end = f.iter().position(|&c| c == 0).unwrap_or(f.len());
        String::from_utf8_lossy(&f[..end]).into_owned()
    };
    while at + 512 <= b.len() {
        let h = &b[at..at + 512];
        if h.iter().all(|&c| c == 0) {
            break;
        }
        let size =
            octal(&h[124..136]).ok_or_else(|| Error::Malformed("damaged tar header".into()))?;
        let body = at + 512;
        let end = body
            .checked_add(size)
            .filter(|&e| e <= b.len())
            .ok_or_else(|| Error::Truncated("the archive ends inside a file".into()))?;
        let data = &b[body..end];
        let mut name = cstr(&h[0..100]);
        if &h[257..262] == b"ustar" {
            let prefix = cstr(&h[345..500]);
            if !prefix.is_empty() {
                name = format!("{prefix}/{name}");
            }
        }
        match h[156] {
            b'L' => long = Some(cstr(data)),
            b'x' => {
                // pax: "len path=value\n" records.
                for rec in String::from_utf8_lossy(data).split('\n') {
                    if let Some((_, kv)) = rec.split_once(' ')
                        && let Some(v) = kv.strip_prefix("path=")
                    {
                        long = Some(v.to_string());
                    }
                }
            }
            b'0' | 0 | b'7' => {
                total += size;
                if total > max_total {
                    return Err(Error::TooLarge);
                }
                out.push((long.take().unwrap_or(name), data.to_vec()));
            }
            _ => long = None,
        }
        at = body + size.div_ceil(512) * 512;
    }
    Ok(out)
}

/// Unix `compress` (`.Z`) streams: LZW with 9–16-bit codes.
pub mod lzw {
    use crate::Error;

    /// Decompresses a `.Z` stream (magic `1F 9D`), at most `max` bytes out.
    pub fn decompress(b: &[u8], max: usize) -> Result<Vec<u8>, Error> {
        if b.len() < 3 || b[0] != 0x1F || b[1] != 0x9D {
            return Err(Error::Malformed("not a compress (.Z) stream".into()));
        }
        let maxbits = u32::from(b[2] & 0x1F).clamp(9, 16);
        let block = b[2] & 0x80 != 0;
        let data = &b[3..];
        let mut prefix: Vec<u32> = vec![0; 1 << 16];
        let mut suffix: Vec<u8> = vec![0; 1 << 16];
        for (i, s) in suffix.iter_mut().enumerate().take(256) {
            *s = i as u8;
        }
        let first_free = if block { 257 } else { 256 };
        let mut free = first_free;
        let mut bits = 9u32;
        let mut out = Vec::new();
        let mut old: Option<u32> = None;
        let mut fin = 0u8;
        let mut stack = Vec::new();
        // Codes come in groups of 8 of one width; a width change skips the rest of a group.
        let mut pos_bits: usize = 0;
        let total = data.len() * 8;
        let mut group_start = 0usize;
        while pos_bits + bits as usize <= total {
            let mut code = 0u32;
            for k in 0..bits as usize {
                let bit = pos_bits + k;
                if data[bit / 8] >> (bit % 8) & 1 != 0 {
                    code |= 1 << k;
                }
            }
            pos_bits += bits as usize;
            if block && code == 256 {
                // Clear: back to 9 bits after the group.
                let gbits = (bits as usize) * 8;
                let used = pos_bits - group_start;
                pos_bits += (gbits - used % gbits) % gbits;
                group_start = pos_bits;
                free = first_free;
                bits = 9;
                old = None;
                continue;
            }
            let Some(o) = old else {
                if code > 255 {
                    return Err(Error::Malformed("bad compress stream".into()));
                }
                fin = code as u8;
                out.push(fin);
                old = Some(code);
                continue;
            };
            let mut c = code;
            if c >= free {
                if c > free {
                    return Err(Error::Malformed("bad compress stream".into()));
                }
                stack.push(fin);
                c = o;
            }
            while c > 255 {
                stack.push(suffix[c as usize]);
                c = prefix[c as usize];
            }
            fin = c as u8;
            stack.push(fin);
            while let Some(x) = stack.pop() {
                out.push(x);
            }
            if out.len() > max {
                return Err(Error::TooLarge);
            }
            if free < (1 << maxbits) {
                prefix[free as usize] = o;
                suffix[free as usize] = fin;
                free += 1;
                if free >= (1 << bits) && bits < maxbits {
                    let gbits = (bits as usize) * 8;
                    let used = pos_bits - group_start;
                    pos_bits += (gbits - used % gbits) % gbits;
                    group_start = pos_bits;
                    bits += 1;
                }
            }
            old = Some(code);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
