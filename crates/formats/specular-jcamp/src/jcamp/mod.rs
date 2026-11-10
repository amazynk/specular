//! JCAMP-DX 4.24–6.0 spectra (IR, Raman, UV-Vis, NMR, MS, chromatograms, ion mobility).
//!
//! A file is blocks of labelled data records; each block with data is a page: an
//! `##XYDATA=` table (`(X++(Y..Y))` in AFFN or any ASDF form, or `(XY..XY)`), an
//! `##XYPOINTS=` or `##PEAK TABLE=` list, or the pages of an `##NTUPLES=` table. LINK files
//! nest one block per spectrum. The abscissa of an `(X++(Y..Y))` table is `##FIRSTX=` to
//! `##LASTX=` over `##NPOINTS=`, as the standard asks; the values written on the lines are
//! checks only. NMR shifts in Hz are shown in ppm when the observe frequency is known.

mod asdf;
mod ldr;
mod units;

use std::collections::HashMap;

use specular_spectra::{Error, Facts, Peak, Plot, Spectra, Trace, warn};

use ldr::{Block, Ldr};

/// Pages in a file at most.
const MAX_PAGES: usize = 10_000;
/// Values in a file at most.
const MAX_VALUES: usize = 64 << 20;
/// Said on a page of peak assignments.
pub const ASSIGNMENTS_NOTE: &str =
    "Peak assignments: each label names what the peak is assigned to.";
/// Labelled fields kept for the info panel at most, and characters of each value.
const MAX_FIELDS: usize = 400;
const MAX_FIELD_CHARS: usize = 200;
/// Peaks labelled when a peak table stands for the spectrum (mass spectra).
const LABELLED_PEAKS: usize = 20;

/// Whether `head` starts like a JCAMP-DX file: `##TITLE=` as the first record, after
/// blank lines and `$$` comments.
pub fn sniff(head: &[u8]) -> bool {
    let text = ldr::decode(&head[..head.len().min(4096)]);
    for line in ldr::lines(&text) {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with("$$") {
            continue;
        }
        let Some(rest) = t.strip_prefix("##") else {
            return false;
        };
        return rest
            .find('=')
            .is_some_and(|e| ldr::normalise(&rest[..e]) == "TITLE");
    }
    false
}

/// Reads a JCAMP-DX file.
pub fn parse(bytes: &[u8]) -> Result<Spectra, Error> {
    let text = ldr::decode(bytes);
    let lines = ldr::lines(&text);
    let records = ldr::records(&lines);
    if !records.iter().any(|l| l.key == "TITLE") {
        return Err(Error::NotThisFormat);
    }
    let roots = ldr::blocks(records);
    let mut r = Reader {
        lines: &lines,
        pages: Vec::new(),
        warnings: Vec::new(),
        values: 0,
        structure: false,
        first_data: None,
    };
    for (i, b) in roots.iter().enumerate() {
        r.block(b, &[i]);
    }
    if r.values > MAX_VALUES {
        return Err(Error::TooLarge);
    }
    if r.pages.is_empty() {
        if r.structure {
            return Err(Error::Unsupported(
                "This JCAMP-DX file holds a chemical structure (JCAMP-CS), not a spectrum.".into(),
            ));
        }
        return Err(Error::Unsupported(
            "This JCAMP-DX file holds no spectrum data.".into(),
        ));
    }
    let root = &roots[0];
    let data_block = r.first_data.as_ref().and_then(|p| find(&roots, p));
    let mut facts = facts(root, data_block, &lines);
    for b in roots.iter().flat_map(walk) {
        if !b.closed {
            warn(&mut r.warnings, "The file ends inside a block (no ##END=).");
            break;
        }
    }
    let mut warnings = r.warnings;
    if r.pages.len() > MAX_PAGES {
        r.pages.truncate(MAX_PAGES);
        warn(
            &mut warnings,
            format!("Only the first {MAX_PAGES} spectra are shown."),
        );
    }
    facts.spectra = r.pages.len() as u64;
    facts.points = r
        .pages
        .iter()
        .flat_map(|p| p.traces.iter())
        .map(|t| t.y.len() as u64)
        .max()
        .unwrap_or(0);
    facts.warnings = warnings;
    Ok(Spectra {
        facts,
        pages: r.pages,
    })
}

fn walk<'b, 'a>(b: &'b Block<'a>) -> Box<dyn Iterator<Item = &'b Block<'a>> + 'b> {
    Box::new(std::iter::once(b).chain(b.children.iter().flat_map(walk)))
}

fn find<'b, 'a>(roots: &'b [Block<'a>], path: &[usize]) -> Option<&'b Block<'a>> {
    let mut b = roots.get(*path.first()?)?;
    for &i in &path[1..] {
        b = b.children.get(i)?;
    }
    Some(b)
}

struct Reader<'l, 'a> {
    lines: &'l [&'a str],
    pages: Vec<Plot>,
    warnings: Vec<String>,
    values: usize,
    /// A block described a chemical structure.
    structure: bool,
    /// The path of the first block with data.
    first_data: Option<Vec<usize>>,
}

/// Header values of a block as they stand at some record (later ones replace earlier).
#[derive(Default, Clone)]
struct Header {
    values: HashMap<String, String>,
}

impl Header {
    fn text(&self, k: &str) -> Option<&str> {
        self.values
            .get(k)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }
    fn num(&self, k: &str) -> Option<f64> {
        self.text(k).and_then(units::number)
    }
}

/// How a table lists its values.
#[derive(Debug, PartialEq)]
enum Table {
    /// `(X++(Y..Y))`: the abscissa symbol, then the ordinate's.
    Incremental(char, char),
    /// `(XY..XY)`, `(XYW..XYW)`…: the symbols of each group.
    Groups(Vec<char>),
}

fn table(spec: &str) -> Option<Table> {
    let s: String = spec
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    let s = s.split("),").next().unwrap_or(&s);
    let inner = s.trim_start_matches('(');
    if let Some(p) = inner.find("++") {
        let x = inner[..p].chars().next()?;
        let y = inner[p + 2..].trim_start_matches('(').chars().next()?;
        return Some(Table::Incremental(x, y));
    }
    let group = inner.split("..").next()?;
    let syms: Vec<char> = group.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    (!syms.is_empty()).then_some(Table::Groups(syms))
}

/// What one table yielded.
#[derive(Default)]
struct Decoded {
    x: Vec<f64>,
    y: Vec<f64>,
    /// Groups of a peak table (x, y), and their labels (assignments).
    groups: Vec<(f64, f64)>,
    labels: Vec<String>,
    /// Points listed one by one (`(XY..XY)`), not an evenly spaced table.
    listed: bool,
}

/// `##PEAK ASSIGNMENTS=(XYA)` / `(XYWA)` / `(XYMA)`: one `(x, y, …, <assignment>)` per
/// line; a missing y is 1.
fn assignments(lines: &[&str], h: &Header) -> Decoded {
    let xf = h.num("XFACTOR").unwrap_or(1.0);
    let yf = h.num("YFACTOR").unwrap_or(1.0);
    let mut d = Decoded::default();
    for line in lines {
        let l = asdf::strip_comment(line).trim();
        let Some(inner) = l.strip_prefix('(') else {
            continue;
        };
        let (nums, label) = match inner.find('<') {
            Some(i) => (
                &inner[..i],
                inner[i + 1..].split('>').next().unwrap_or("").trim(),
            ),
            None => (inner.trim_end_matches(')'), ""),
        };
        let mut f = nums.split(',').map(|v| units::number(v.trim()));
        let Some(Some(x)) = f.next() else {
            continue;
        };
        let y = f.next().flatten().unwrap_or(1.0);
        d.groups.push((x * xf, y * yf));
        d.labels.push(label.to_string());
    }
    d
}

impl<'a> Reader<'_, 'a> {
    fn block(&mut self, b: &Block<'a>, path: &[usize]) {
        let data_type = b
            .get("DATATYPE")
            .map(|l| l.head().to_ascii_uppercase())
            .unwrap_or_default();
        let link = data_type.contains("LINK");
        if data_type.contains("STRUCTURE") || b.get("MOLFORM").is_some() && !has_data(b) {
            self.structure = true;
        }
        if !link {
            self.data(b, path, &data_type);
        }
        for (i, c) in b.children.iter().enumerate() {
            let mut p = path.to_vec();
            p.push(i);
            self.block(c, &p);
        }
    }

    fn data(&mut self, b: &Block<'a>, path: &[usize], data_type: &str) {
        let lines = self.lines;
        let mut h = Header::default();
        let mut plot: Option<(Plot, Option<f64>)> = None;
        let title = b.get("TITLE").map(|l| l.value(lines)).unwrap_or_default();
        let mut i = 0;
        while i < b.ldrs.len() {
            let l = &b.ldrs[i];
            match l.key.as_str() {
                "PEAKASSIGNMENTS" => {
                    let d = assignments(&self.lines[l.more.clone()], &h);
                    let (p, div) =
                        plot.get_or_insert_with(|| self.new_plot(&title, data_type, &h, None));
                    add(p, d, true, *div);
                    p.notes.push(ASSIGNMENTS_NOTE.into());
                }
                "XYDATA" | "XYPOINTS" | "PEAKTABLE" | "PEAKS" => {
                    let Some(spec) = table(l.head()) else {
                        warn(
                            &mut self.warnings,
                            format!("A table with no variable list (##{}) was skipped.", l.raw),
                        );
                        i += 1;
                        continue;
                    };
                    let peaks = matches!(l.key.as_str(), "PEAKTABLE" | "PEAKS");
                    let d = self.table(l, &spec, &h, None);
                    let (p, div) =
                        plot.get_or_insert_with(|| self.new_plot(&title, data_type, &h, None));
                    add(p, d, peaks, *div);
                }
                "NTUPLES" => {
                    let end = b.ldrs[i..]
                        .iter()
                        .position(|l| l.key == "ENDNTUPLES")
                        .map_or(b.ldrs.len(), |e| i + e);
                    self.ntuples(&b.ldrs[i + 1..end], &h, &title, data_type);
                    if self.first_data.is_none() && !self.pages.is_empty() {
                        self.first_data = Some(path.to_vec());
                    }
                    i = end;
                }
                _ => {
                    h.values.insert(l.key.clone(), l.value(lines));
                }
            }
            i += 1;
        }
        if let Some((p, _)) = plot
            && (!p.traces.is_empty() || !p.peaks.is_empty())
        {
            let p = finish(p);
            self.pages.push(p);
            if self.first_data.is_none() {
                self.first_data = Some(path.to_vec());
            }
        }
    }

    /// A page's axes from the header as it stands (`vars` for an NTUPLES table: units of
    /// the abscissa and the ordinate, and the abscissa's name).
    fn new_plot(
        &self,
        title: &str,
        data_type: &str,
        h: &Header,
        vars: Option<(&str, &str, &str)>,
    ) -> (Plot, Option<f64>) {
        let (xu, yu, xname) = match vars {
            Some(v) => (v.0.to_string(), v.1.to_string(), v.2.to_string()),
            None => (
                h.text("XUNITS").unwrap_or("").to_string(),
                h.text("YUNITS").unwrap_or("").to_string(),
                String::new(),
            ),
        };
        let technique = units::technique(data_type, &xu);
        let fid = data_type.contains("FID") || xname.to_ascii_uppercase().contains("TIME");
        let (x, divisor) = units::x_axis(&xu, technique, fid, observe_mhz(h));
        let mut p = Plot {
            title: title.to_string(),
            x,
            y: units::y_axis(&yu),
            ..Plot::default()
        };
        if let Some(f) = divisor {
            p.notes.push(format!(
                "Shifts converted from Hz to ppm at {} MHz.",
                units::short(f)
            ));
        }
        (p, divisor)
    }

    /// Decodes a table. `nt` gives an NTUPLES table's first, last, factors and points for
    /// its abscissa and ordinate; otherwise the header's `##FIRSTX=` and kin apply.
    fn table(&mut self, l: &Ldr<'a>, spec: &Table, h: &Header, nt: Option<&NtVars>) -> Decoded {
        let lines = &self.lines[l.more.clone()];
        let mut d = Decoded::default();
        match spec {
            Table::Incremental(..) => {
                let mut o = asdf::Ordinates::default();
                for line in lines {
                    let t = asdf::strip_comment(line);
                    if t.trim().is_empty() {
                        continue;
                    }
                    o.line(t);
                }
                if !o.bad_chars.is_empty() {
                    warn(
                        &mut self.warnings,
                        format!(
                            "Lines with characters that are not numbers ({}) were skipped.",
                            o.bad_chars.iter().collect::<String>()
                        ),
                    );
                }
                if o.y_check_failures > 0 {
                    warn(
                        &mut self.warnings,
                        format!(
                            "{} data lines failed their Y-check (the file may be damaged).",
                            o.y_check_failures
                        ),
                    );
                }
                let (first, last, npoints, xf, yf, delta) = match nt {
                    Some(v) => (v.x_first, v.x_last, v.points, v.x_factor, v.y_factor, None),
                    None => (
                        h.num("FIRSTX"),
                        h.num("LASTX"),
                        h.num("NPOINTS").map(|n| n as usize),
                        h.num("XFACTOR").unwrap_or(1.0),
                        h.num("YFACTOR").unwrap_or(1.0),
                        h.num("DELTAX"),
                    ),
                };
                let n = o.y.len();
                self.values += n;
                if let Some(np) = npoints
                    && np != n
                {
                    warn(
                        &mut self.warnings,
                        format!("The file says {np} points but holds {n}."),
                    );
                }
                let x0_line = o.line_x.first().map(|p| p.0 * xf);
                // X-check: lines whose abscissa is off the computed one.
                let misses = |x0: f64, step: f64| {
                    let tol = (step / xf).abs().max(1.0);
                    o.line_x
                        .iter()
                        .filter(|(x, at)| {
                            x.is_finite() && ((x0 + step * *at as f64) / xf - x).abs() > tol
                        })
                        .count()
                };
                let (x0, step) = match (first, last, npoints) {
                    (Some(f), Some(l), Some(np)) if np > 1 => {
                        let by_np = (l - f) / (np - 1) as f64;
                        // A wrong ##NPOINTS=: the ends and the points found, if the lines
                        // agree with that better.
                        if np != n && n > 1 {
                            let by_n = (l - f) / (n - 1) as f64;
                            if misses(f, by_n) < misses(f, by_np) {
                                (f, by_n)
                            } else {
                                (f, by_np)
                            }
                        } else {
                            (f, by_np)
                        }
                    }
                    (Some(f), Some(l), _) if n > 1 => (f, (l - f) / (n - 1) as f64),
                    (f, _, _) => {
                        let x0 = f.or(x0_line).unwrap_or(0.0);
                        let step = delta.or_else(|| line_step(&o, xf)).unwrap_or(1.0);
                        (x0, step)
                    }
                };
                if let (Some(dx), true) = (delta, step != 0.0)
                    && (dx - step).abs() > 1e-3 * step.abs()
                {
                    warn(
                        &mut self.warnings,
                        "##DELTAX= disagrees with ##FIRSTX=, ##LASTX= and ##NPOINTS=.",
                    );
                }
                let bad = misses(x0, step);
                if bad > 0 {
                    warn(
                        &mut self.warnings,
                        format!("{bad} data lines failed their X-check."),
                    );
                }
                d.x = (0..n).map(|i| x0 + step * i as f64).collect();
                d.y = o.y.into_iter().map(|v| v * yf).collect();
            }
            Table::Groups(syms) => {
                let mut v = Vec::new();
                let mut text = String::new();
                for line in lines {
                    text.push_str(line);
                    text.push('\n');
                }
                if let Err(c) = asdf::affn_values(&text, &mut v) {
                    warn(
                        &mut self.warnings,
                        format!("A point list with an unexpected character ({c}) was cut short."),
                    );
                }
                self.values += v.len();
                let k = syms.len().max(2);
                let (xf, yf) = match nt {
                    Some(n) => (n.x_factor, n.y_factor),
                    None => (
                        h.num("XFACTOR").unwrap_or(1.0),
                        h.num("YFACTOR").unwrap_or(1.0),
                    ),
                };
                if v.len() % k != 0 {
                    warn(
                        &mut self.warnings,
                        "A point list ends with an incomplete point.",
                    );
                }
                for g in v.chunks_exact(k) {
                    d.groups.push((g[0] * xf, g[1] * yf));
                }
                d.listed = true;
                d.x = d.groups.iter().map(|g| g.0).collect();
                d.y = d.groups.iter().map(|g| g.1).collect();
            }
        }
        d
    }

    fn ntuples(&mut self, ldrs: &[Ldr<'a>], outer: &Header, title: &str, data_type: &str) {
        let lines = self.lines;
        let mut attrs: HashMap<String, Vec<String>> = HashMap::new();
        let mut page_h = outer.clone();
        let mut page_name = String::new();
        // (ordinate symbol, plot) of each page.
        let mut pages: Vec<(char, Plot)> = Vec::new();
        for l in ldrs {
            match l.key.as_str() {
                "VARNAME" | "SYMBOL" | "VARTYPE" | "VARFORM" | "VARDIM" | "UNITS" | "FIRST"
                | "LAST" | "MIN" | "MAX" | "FACTOR" => {
                    let v = l.value(lines);
                    attrs.insert(
                        l.key.clone(),
                        v.split(',').map(|s| s.trim().to_string()).collect(),
                    );
                }
                "PAGE" => {
                    page_h = outer.clone();
                    page_name = l.value(lines);
                }
                "DATATABLE" => {
                    let Some(spec) = table(l.head()) else {
                        continue;
                    };
                    let kind = l
                        .head()
                        .rsplit(',')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_ascii_uppercase();
                    let peaks = kind.starts_with("PEAK");
                    let vars = NtVars::new(&attrs, &spec, &page_h);
                    let d = self.table(l, &spec, &page_h, Some(&vars));
                    let name = if !page_name.is_empty() {
                        page_name.clone()
                    } else if !vars.y_name.is_empty() {
                        vars.y_name.clone()
                    } else {
                        title.to_string()
                    };
                    let (mut p, div) = self.new_plot(
                        &name,
                        data_type,
                        &page_h,
                        Some((&vars.x_units, &vars.y_units, &vars.x_name)),
                    );
                    add(&mut p, d, peaks, div);
                    pages.push((vars.y_symbol, finish(p)));
                }
                _ => {
                    page_h.values.insert(l.key.clone(), l.value(lines));
                }
            }
        }
        // Real and imaginary parts: the real part is shown.
        let has_real = pages.iter().any(|(s, _)| *s == 'R');
        let has_imag = pages.iter().any(|(s, _)| *s == 'I');
        for (s, mut p) in pages {
            if has_real && s == 'I' {
                continue;
            }
            if has_real && has_imag {
                p.title = title.to_string();
                p.notes.push("The imaginary part is not shown.".into());
            }
            if !p.traces.is_empty() || !p.peaks.is_empty() {
                self.pages.push(p);
            }
        }
    }
}

/// The step between points from the abscissas of the first two lines.
fn line_step(o: &asdf::Ordinates, xf: f64) -> Option<f64> {
    let (a, b) = (o.line_x.first()?, o.line_x.get(1)?);
    let n = (b.1 - a.1) as f64;
    (n > 0.0).then(|| (b.0 - a.0) * xf / n)
}

/// What an NTUPLES table says about one page's abscissa and ordinate.
struct NtVars {
    x_first: Option<f64>,
    x_last: Option<f64>,
    points: Option<usize>,
    x_factor: f64,
    y_factor: f64,
    x_units: String,
    y_units: String,
    x_name: String,
    y_name: String,
    y_symbol: char,
}

impl NtVars {
    fn new(attrs: &HashMap<String, Vec<String>>, spec: &Table, page: &Header) -> NtVars {
        let (xs, ys) = match spec {
            Table::Incremental(x, y) => (*x, *y),
            Table::Groups(g) => (g[0], g.get(1).copied().unwrap_or('Y')),
        };
        let col = |s: char| {
            attrs.get("SYMBOL").and_then(|v| {
                v.iter()
                    .position(|t| t.eq_ignore_ascii_case(&s.to_string()))
            })
        };
        let (ix, iy) = (col(xs), col(ys));
        let get = |k: &str, i: Option<usize>| -> Option<String> {
            attrs
                .get(k)
                .and_then(|v| v.get(i?))
                .filter(|s| !s.is_empty())
                .cloned()
        };
        let num = |k: &str, i: Option<usize>| get(k, i).and_then(|s| units::number(&s));
        let points = page
            .num("NPOINTS")
            .or_else(|| num("VARDIM", iy))
            .map(|n| n as usize);
        NtVars {
            x_first: page.num("FIRSTX").or_else(|| num("FIRST", ix)),
            x_last: page.num("LASTX").or_else(|| num("LAST", ix)),
            points,
            x_factor: num("FACTOR", ix).unwrap_or(1.0),
            y_factor: num("FACTOR", iy).unwrap_or(1.0),
            x_units: get("UNITS", ix).unwrap_or_default(),
            y_units: get("UNITS", iy).unwrap_or_default(),
            x_name: get("VARNAME", ix).unwrap_or_default(),
            y_name: get("VARNAME", iy).unwrap_or_default(),
            y_symbol: ys,
        }
    }
}

fn has_data(b: &Block<'_>) -> bool {
    b.ldrs.iter().any(|l| {
        matches!(
            l.key.as_str(),
            "XYDATA" | "XYPOINTS" | "PEAKTABLE" | "PEAKASSIGNMENTS" | "NTUPLES" | "DATATABLE"
        )
    })
}

/// The observe frequency in MHz, when the header states it.
fn observe_mhz(h: &Header) -> Option<f64> {
    h.num(".OBSERVEFREQUENCY")
        .or_else(|| h.num("$SFO1"))
        .filter(|f| *f > 0.0)
}

/// Adds a table's points to a page: a trace, or the peaks of a peak table; abscissas
/// divided by `div` (Hz to ppm).
fn add(p: &mut Plot, mut d: Decoded, peaks: bool, div: Option<f64>) {
    if let Some(f) = div {
        d.x.iter_mut().for_each(|x| *x /= f);
        d.groups.iter_mut().for_each(|g| g.0 /= f);
    }
    if peaks {
        p.peaks
            .extend(d.groups.iter().enumerate().map(|(i, &(x, y))| Peak {
                x,
                y,
                label: d.labels.get(i).cloned().unwrap_or_default(),
            }));
    } else if !d.y.is_empty() {
        // Points listed one by one on a mass axis are centroided peaks.
        let sticks = d.listed && p.x.title == "m/z";
        p.traces.push(Trace {
            name: String::new(),
            x: d.x,
            y: d.y,
            sticks,
        });
    }
}

/// Final touches: a peak table alone becomes sticks with its largest peaks labelled; the
/// abscissa in ppm when it was Hz on an NMR page.
fn finish(mut p: Plot) -> Plot {
    if p.traces.is_empty() && !p.peaks.is_empty() {
        let mut pk = std::mem::take(&mut p.peaks);
        pk.sort_by(|a, b| a.x.total_cmp(&b.x));
        p.traces.push(Trace {
            name: String::new(),
            x: pk.iter().map(|q| q.x).collect(),
            y: pk.iter().map(|q| q.y).collect(),
            sticks: true,
        });
        let mut big: Vec<Peak> = pk;
        big.sort_by(|a, b| b.y.total_cmp(&a.y));
        big.truncate(LABELLED_PEAKS);
        p.peaks = big;
    }
    for q in &mut p.peaks {
        if q.label.is_empty() {
            q.label = units::short(q.x);
        }
    }
    p
}

/// The facts of the file: the first block's, filled in from the first block with data.
fn facts<'a>(root: &Block<'a>, data: Option<&Block<'a>>, lines: &[&'a str]) -> Facts {
    let pick = |k: &str| -> Option<String> {
        [Some(root), data]
            .into_iter()
            .flatten()
            .find_map(|b| b.get(k).map(|l| l.value(lines)))
            .filter(|s| !s.is_empty())
    };
    let data_type = pick("DATATYPE").unwrap_or_default().to_ascii_uppercase();
    let xu = pick("XUNITS").unwrap_or_default();
    let mut f = Facts {
        format: "JCAMP-DX".into(),
        version: pick("JCAMPDX").unwrap_or_default(),
        technique: units::technique(&data_type, &xu),
        title: pick("TITLE"),
        origin: pick("ORIGIN").or_else(|| pick("OWNER")),
        instrument: pick("SPECTROMETERDATASYSTEM").or_else(|| pick("INSTRUMENTALPARAMETERS")),
        date: pick("LONGDATE").or_else(|| match (pick("DATE"), pick("TIME")) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (d, _) => d,
        }),
        nucleus: pick(".OBSERVENUCLEUS").map(|n| n.trim_start_matches('^').to_string()),
        frequency_mhz: pick(".OBSERVEFREQUENCY").and_then(|v| units::number(&v)),
        solvent: pick(".SOLVENTNAME"),
        pulse_program: pick(".PULSESEQUENCE")
            .or_else(|| pick("$PULPROG").map(|p| p.trim_matches(['<', '>']).to_string())),
        ..Facts::default()
    };
    // Every other record of those blocks, as written.
    const SHOWN: &[&str] = &[
        "TITLE",
        "JCAMPDX",
        "DATATYPE",
        "DATACLASS",
        "XYDATA",
        "XYPOINTS",
        "PEAKTABLE",
        "PEAKS",
        "NTUPLES",
        "ENDNTUPLES",
        "DATATABLE",
        "PAGE",
        "VARNAME",
        "SYMBOL",
        "VARTYPE",
        "VARFORM",
        "VARDIM",
        "UNITS",
        "FIRST",
        "LAST",
        "MIN",
        "MAX",
        "FACTOR",
        "FIRSTX",
        "LASTX",
        "FIRSTY",
        "MINX",
        "MAXX",
        "MINY",
        "MAXY",
        "XFACTOR",
        "YFACTOR",
        "NPOINTS",
        "DELTAX",
        "BLOCKS",
        "BLOCKID",
        "END",
        "ORIGIN",
        "OWNER",
        "SPECTROMETERDATASYSTEM",
        "LONGDATE",
        "DATE",
        "TIME",
        ".OBSERVENUCLEUS",
        ".OBSERVEFREQUENCY",
        ".SOLVENTNAME",
        ".PULSESEQUENCE",
    ];
    let mut seen: Vec<String> = Vec::new();
    for b in [Some(root), data].into_iter().flatten() {
        for l in &b.ldrs {
            if f.fields.len() >= MAX_FIELDS {
                break;
            }
            if SHOWN.contains(&l.key.as_str()) || seen.contains(&l.key) {
                continue;
            }
            seen.push(l.key.clone());
            let mut v = l.value(lines);
            if v.is_empty() {
                continue;
            }
            if v.chars().count() > MAX_FIELD_CHARS {
                v = v.chars().take(MAX_FIELD_CHARS).collect::<String>() + "…";
            }
            f.fields.push((l.raw.to_string(), v));
        }
    }
    f
}

#[cfg(test)]
mod tests;
