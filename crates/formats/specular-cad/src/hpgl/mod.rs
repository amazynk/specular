//! Own HP-GL and HP-GL/2 reader: pen-plotter files (7475A era), HP-GL/2 with polyline
//! encoding, fills and fonts, and HP-GL/2 inside PCL 5 printer jobs (with PJL), into a
//! [`VectorScene`] whose pages are sheets and whose pens are layers.
//!
//! Coordinates are plotter units (40 per millimetre); the scene is in millimetres, so a
//! plot measures true to the paper. Labels are text in the drawing font (the stick font is
//! not reproduced). Unknown commands are skipped.

mod interp;
mod lex;

use kurbo::Rect;
use specular_vector::{FontSpec, Generated, PlotInfo, Sheet, SheetKind, Units, VectorScene};

use crate::{CadError, Limits};

/// How many bytes [`is_hpgl`] wants to see (PJL headers can be long).
pub const PROBE_LEN: usize = 8192;

/// Commands a plot opens with; one of these must come early.
const OPENERS: &[&[u8; 2]] = &[
    b"IN", b"DF", b"SP", b"PU", b"PD", b"PA", b"PR", b"BP", b"IP", b"SC", b"PS", b"PG", b"NP",
    b"PW", b"WU", b"RO", b"LT", b"CO",
];

/// Whether `head` (the first [`PROBE_LEN`] bytes of a file) is an HP-GL plot. Strict, so
/// other `.plt` files (Gnuplot scripts, AutoCAD plot logs) are not taken for one: a PCL
/// or PJL job must switch to HP-GL/2; plain HP-GL must open with known commands whose
/// parameters are numbers.
pub fn is_hpgl(head: &[u8]) -> bool {
    let mut b = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    while let Some((&c, rest)) = b.split_first() {
        // A stray label terminator may come first (output appended to a label).
        if lex::is_space(c) || c == 0 || c == b';' || c == lex::ETX {
            b = rest;
        } else {
            break;
        }
    }
    if b.first() == Some(&lex::ESC) && b.get(1) != Some(&b'.') {
        // A printer job: it must enter HP-GL/2 before anything else is decided.
        return enters_hpgl2(b);
    }
    // Device-control escapes of serial plotters (`ESC . (`, `ESC . I81;;17:`).
    while b.first() == Some(&lex::ESC) {
        let Some(&c) = b.get(2) else {
            return false;
        };
        let mut j = 3;
        if b"@HIMNPST".contains(&c) {
            while j < b.len() && j < 64 && b[j] != b':' {
                j += 1;
            }
            j += 1;
        }
        b = b.get(j..).unwrap_or(&[]);
        while let Some((&c, rest)) = b.split_first() {
            if lex::is_space(c) || c == b';' {
                b = rest;
            } else {
                break;
            }
        }
    }
    commands_ok(b)
}

/// The first commands: at least two known ones (or a truncated head after one), each with
/// numeric parameters, among them one a plot opens with.
fn commands_ok(b: &[u8]) -> bool {
    let mut i = 0;
    let mut seen = 0;
    let mut opener = false;
    while seen < 6 {
        while i < b.len() && (lex::is_space(b[i]) || b[i] == b';' || b[i] == b',') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i] == lex::ESC {
            // An escape after HP-GL commands: the head is HP-GL so far.
            break;
        }
        let Some(&c2) = b.get(i + 1) else {
            break;
        };
        // Several good commands make a plot even if a later one is odd (`PD:`).
        let settled = seen >= 3 && opener;
        if !(b[i].is_ascii_alphabetic() && c2.is_ascii_alphabetic()) {
            return settled;
        }
        let mn = [b[i].to_ascii_uppercase(), c2.to_ascii_uppercase()];
        if !interp::known(&mn) {
            return settled;
        }
        // Files are upper-case, or entirely lower-case; mixed case is prose.
        if b[i].is_ascii_lowercase() != c2.is_ascii_lowercase() {
            return settled;
        }
        if &mn == b"CO" && lex::unquoted_comment(&b[i + 2..]) {
            // A comment without quotes: skipped, and it proves nothing.
            while i < b.len() && b[i] != b';' {
                i += 1;
            }
            continue;
        }
        opener |= OPENERS.contains(&&mn);
        i += 2;
        match &mn {
            b"LB" | b"BL" | b"WD" => {
                // Text to the terminator.
                while i < b.len() && b[i] != lex::ETX {
                    i += 1;
                }
                i += 1;
            }
            b"DT" | b"SM" => {
                if i < b.len() && b[i] != b';' {
                    i += 1;
                }
            }
            b"PE" => {
                while i < b.len() && b[i] != b';' && b[i] != lex::ESC {
                    i += 1;
                }
            }
            _ => {
                let s = i;
                while i < b.len() {
                    let c = b[i];
                    if c == b'"' {
                        i += 1;
                        while i < b.len() && b[i] != b'"' {
                            i += 1;
                        }
                        i += 1;
                        continue;
                    }
                    if c == b';' || c == lex::ESC || c.is_ascii_alphabetic() {
                        break;
                    }
                    if !(c.is_ascii_digit()
                        || matches!(c, b'+' | b'-' | b'.' | b',')
                        || lex::is_space(c))
                    {
                        return settled;
                    }
                    i += 1;
                }
                // Parameters must be numbers, not `1..2` or `--`.
                let mut nums = Vec::new();
                let args = &b[s..i.min(b.len())];
                if !args.contains(&b'"') {
                    lex::numbers(args, &mut nums);
                    let tokens = args
                        .split(|&c| c == b',' || lex::is_space(c))
                        .filter(|t| !t.is_empty())
                        .count();
                    if nums.len() < tokens && i < b.len() {
                        return settled;
                    }
                }
                // A letter right after numbers needs a separator in practice only for
                // a few writers; accept either.
            }
        }
        seen += 1;
    }
    opener && (seen >= 2 || (seen == 1 && i >= b.len()))
}

/// A PCL or PJL job that switches to HP-GL/2.
fn enters_hpgl2(b: &[u8]) -> bool {
    let up: Vec<u8> = b.iter().map(|c| c.to_ascii_uppercase()).collect();
    if find(&up, b"ENTER LANGUAGE").is_some_and(|i| {
        let rest = &up[i..up.len().min(i + 40)];
        find(rest, b"HPGL2").is_some() || find(rest, b"HP-GL/2").is_some()
    }) {
        return true;
    }
    // ESC % [-]n B
    let mut i = 0;
    while let Some(p) = find(&b[i..], b"\x1b%") {
        let mut j = i + p + 2;
        while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'-') {
            j += 1;
        }
        if j > i + p + 2 && b.get(j) == Some(&b'B') {
            return true;
        }
        i = i + p + 2;
    }
    false
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Parses a plot. Fails with [`CadError::NotHpgl`] when hardly anything in it is HP-GL.
pub fn parse(bytes: &[u8], limits: &Limits) -> Result<VectorScene, CadError> {
    let hpgl2 = interp::looks_hpgl2(bytes);
    let out = interp::Interp::new(limits, hpgl2).run(bytes);
    if out.commands == 0 || out.unknown > out.commands {
        return Err(CadError::NotHpgl);
    }
    let sheets: Vec<Sheet> = out
        .pages
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut s = Sheet::generated(Generated::Page(i as u32 + 1), SheetKind::Layout);
            s.items = p.items;
            s.extents = p.extents;
            s
        })
        .collect();
    let mut sheets = sheets;
    if sheets.is_empty() {
        // Nothing drawn: one empty page, so the file still opens.
        sheets.push(Sheet::generated(Generated::Page(1), SheetKind::Layout));
    }
    let mut counts = out.counts;
    counts.retain(|c| c.1 > 0);
    Ok(VectorScene {
        layer_opacity: Vec::new(),
        pcb: None,
        survey: None,
        format: if out.hpgl2 { "HP-GL/2" } else { "HP-GL" }.into(),
        version: String::new(),
        release: None,
        units: Units { code: 4 },
        layers: out.layers,
        linetypes: out.linetypes,
        fonts: vec![FontSpec {
            style: "HP-GL".into(),
            width_factor: 1.0,
            ..FontSpec::default()
        }],
        sheets,
        counts,
        warnings: out.warnings,
        lineweight_display: out.hpgl2 || out.widths_seen,
        point_mode: 0,
        point_size: 0.0,
        codepage: None,
        dot_radius: 0.0,
        layout: None,
        plot: Some(PlotInfo {
            hpgl2: out.hpgl2,
            pcl: out.pcl,
            pens: out.pens,
            paper_mm: out.paper_mm,
            name: out.name,
        }),
    })
}

/// Extents of all pages together, mm.
pub fn extents(scene: &VectorScene) -> Option<Rect> {
    scene
        .sheets
        .iter()
        .filter_map(|s| s.extents)
        .reduce(|a, b| a.union(b))
}

#[cfg(test)]
mod tests;
