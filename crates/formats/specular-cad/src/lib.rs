//! Own DXF reader: ASCII and binary DXF, R12 to 2018, into a [`VectorScene`].
//!
//! The file is tokenized once into borrowed groups; sections, tables, blocks and entities
//! are slices of that. Each block definition is converted once into nodes, and references
//! are expanded when the sheets are emitted (with limits on nesting, arrays and the total
//! number of items, so a hostile file cannot make it run away).

mod aci;
mod build;
mod doc;
mod emit;
mod geom;
mod hatch;
pub mod hpgl;
mod text;
mod tokens;

use std::collections::HashMap;
use std::fmt;

use kurbo::{BezPath, Rect};
use specular_vector::{Shape, Sheet, SheetKind, SheetUnits, Units, VectorScene};

use build::{Builder, Node, Space};
use doc::Ent;
use text::Decoder;

/// The text encoding of a `$DWGCODEPAGE` name ("ANSI_932"): what turns text back into
/// Big Font codes.
pub fn codepage_encoding(cp: &str) -> &'static encoding_rs::Encoding {
    text::codepage_encoding(cp)
}

/// Bounds on the work a file can cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Items after block references are expanded, over all sheets.
    pub max_items: usize,
    /// Nesting of block references.
    pub max_depth: u32,
    /// Points of one polyline, spline or hatch boundary; dashes of one hatch pattern.
    pub max_points: usize,
    /// Copies of one arrayed INSERT (columns × rows).
    pub max_array: u32,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_items: 4_000_000,
            max_depth: 24,
            max_points: 2_000_000,
            max_array: 100_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CadError {
    /// Not a DXF file.
    NotDxf,
    /// Not an HP-GL plot file.
    NotHpgl,
    /// A DXF file too damaged to read.
    Malformed(String),
    /// More items than [`Limits::max_items`].
    TooLarge,
}

impl fmt::Display for CadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CadError::NotDxf => f.write_str("not a DXF file"),
            CadError::NotHpgl => f.write_str("not an HP-GL plot file"),
            CadError::Malformed(m) => write!(f, "damaged DXF file: {m}"),
            CadError::TooLarge => f.write_str("the drawing has too many items"),
        }
    }
}

impl std::error::Error for CadError {}

/// Whether `head` (the first bytes of a file) looks like DXF: the binary sentinel, or
/// ASCII groups that open with `0 SECTION` (after optional `999` comments).
pub fn is_dxf(head: &[u8]) -> bool {
    if head.starts_with(tokens::BINARY_SENTINEL) {
        return true;
    }
    let b = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    let mut lines = b
        .split(|&c| c == b'\n')
        .map(tokens::trim)
        .skip_while(|l| l.is_empty());
    for _ in 0..8 {
        let (Some(code), Some(val)) = (lines.next(), lines.next()) else {
            return false;
        };
        match code {
            b"0" => return val.eq_ignore_ascii_case(b"SECTION"),
            b"999" => continue,
            _ => return false,
        }
    }
    false
}

/// The AutoCAD release of a `$ACADVER` value.
pub fn release_name(version: &str) -> Option<&'static str> {
    Some(match version.trim() {
        "AC1002" => "AutoCAD 2.5",
        "AC1003" => "AutoCAD 2.6",
        "AC1004" => "AutoCAD R9",
        "AC1006" => "AutoCAD R10",
        "AC1009" => "AutoCAD R11/R12",
        "AC1012" => "AutoCAD R13",
        "AC1014" => "AutoCAD R14",
        "AC1015" => "AutoCAD 2000",
        "AC1018" => "AutoCAD 2004",
        "AC1021" => "AutoCAD 2007",
        "AC1024" => "AutoCAD 2010",
        "AC1027" => "AutoCAD 2013",
        "AC1032" => "AutoCAD 2018",
        _ => return None,
    })
}

/// What the header says, without converting the drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeaderInfo {
    pub version: String,
    pub release: Option<String>,
    pub units: Units,
    pub codepage: Option<String>,
    pub binary: bool,
    /// Model-space extents as stated (`$EXTMIN`/`$EXTMAX`), when valid.
    pub extents: Option<Rect>,
}

/// Reads only the HEADER section.
pub fn parse_header_only(bytes: &[u8]) -> Result<HeaderInfo, CadError> {
    if !is_dxf(&bytes[..bytes.len().min(4096)]) {
        return Err(CadError::NotDxf);
    }
    // The header comes first: tokenize only up to the end of its section.
    let mut warnings = Vec::new();
    let binary = bytes.starts_with(tokens::BINARY_SENTINEL);
    let cut = if binary {
        bytes
    } else {
        find(bytes, b"ENDSEC").map_or(bytes, |i| &bytes[..i + 6])
    };
    let gs = tokens::tokenize(cut, &mut warnings)?;
    let h = doc::sections(&gs)
        .into_iter()
        .find(|s| s.0.eq_ignore_ascii_case(b"HEADER"))
        .map(|s| doc::header(s.1))
        .unwrap_or_default();
    Ok(HeaderInfo {
        release: release_name(&h.version).map(Into::into),
        version: h.version,
        units: Units { code: h.insunits },
        codepage: h.codepage,
        binary,
        extents: h.ext,
    })
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Parses a whole drawing.
pub fn parse(bytes: &[u8], limits: &Limits) -> Result<VectorScene, CadError> {
    if !is_dxf(&bytes[..bytes.len().min(4096)]) {
        return Err(CadError::NotDxf);
    }
    let mut warnings = Vec::new();
    let gs = tokens::tokenize(bytes, &mut warnings)?;
    let secs = doc::sections(&gs);
    if secs.is_empty() {
        return Err(CadError::Malformed("no sections".into()));
    }
    let sec = |name: &str| -> &[tokens::Group<'_>] {
        secs.iter()
            .find(|s| s.0.eq_ignore_ascii_case(name.as_bytes()))
            .map_or(&[][..], |s| s.1)
    };
    let header = doc::header(sec("HEADER"));
    let dec = if header.version.is_empty() && std::str::from_utf8(bytes).is_ok() {
        Decoder::utf8()
    } else {
        Decoder::new(&header.version, header.codepage.as_deref())
    };

    let mut b = Builder::new(limits, &header, dec);
    b.warnings = warnings;

    // Tables: linetypes before layers (layers name them), styles, block records.
    let tables = doc::tables(sec("TABLES"));
    let table = |name: &'static str| {
        tables
            .iter()
            .filter(|t| t.0.eq_ignore_ascii_case(name.as_bytes()))
            .flat_map(|t| t.1.iter())
    };
    let ltypes: Vec<&Ent<'_>> = table("LTYPE").collect();
    b.linetype_table(&ltypes.iter().map(|e| clone_ent(e)).collect::<Vec<_>>());
    let lays: Vec<Ent<'_>> = table("LAYER").map(clone_ent).collect();
    b.layer_table(&lays);
    let styles: Vec<Ent<'_>> = table("STYLE").map(clone_ent).collect();
    b.style_table(&styles);
    let mut record_names: HashMap<u64, String> = HashMap::new();
    for e in table("BLOCK_RECORD") {
        if e.has(5) {
            record_names.insert(e.handle(), dec.decode(e.s(2)));
        }
    }

    // Blocks: names first (references can point forward), then contents.
    let blocks = doc::blocks(sec("BLOCKS"), &|s| dec.decode(s));
    for (i, bd) in blocks.iter().enumerate() {
        b.block_map.entry(bd.name.to_lowercase()).or_insert(i);
        b.block_bases.push(bd.base);
    }
    let mut templates: Vec<Vec<Node>> = Vec::with_capacity(blocks.len());
    for bd in &blocks {
        let n = bd.name.to_ascii_lowercase();
        let space = if n.starts_with("*model_space") || n.starts_with("*paper_space") {
            Space::Top
        } else {
            Space::Block
        };
        // External references (flag 4) have no contents here.
        if bd.flags & 4 != 0 {
            b.warn(format!("external reference {} is not loaded", bd.name));
        }
        templates.push(b.convert_all(&bd.ents, space));
    }

    // Entities: model space, and the active layout's paper space (67 = 1).
    let ents = doc::attach(doc::records(sec("ENTITIES")));
    let (paper_ents, model_ents): (Vec<Ent<'_>>, Vec<Ent<'_>>) =
        ents.into_iter().partition(|e| e.i(67, 0) == 1);
    let model_nodes = b.convert_all(&model_ents, Space::Top);
    let paper_nodes = b.convert_all(&paper_ents, Space::Top);
    let layouts = doc::layouts(sec("OBJECTS"), &|s| dec.decode(s));

    let layer0 = b.layer0();
    let mut counts: Vec<(String, u64)> = b.counts.drain().collect();
    counts.sort();
    let Builder {
        mut layers,
        linetypes,
        fonts,
        layer_handles,
        block_map,
        warnings: bwarn,
        ..
    } = b;

    let mut em = emit::Emitter {
        templates: &templates,
        layers: &mut layers,
        layer0,
        ltscale: header.ltscale,
        max_items: limits.max_items,
        max_depth: limits.max_depth,
        total: 0,
        copies: 0,
        elements: 0,
        warnings: Vec::new(),
    };
    let mut sheets = Vec::new();

    // Model space's unit: `$INSUNITS`, else millimetres or inches as `$MEASUREMENT` says.
    let model_code = match (header.insunits, header.measurement) {
        (0, Some(1)) => 4,
        (0, Some(0)) => 1,
        (c, _) => c,
    };
    let sheet_units = |code: i16| SheetUnits {
        code,
        model: model_code,
        lunits: header.lunits,
        luprec: header.luprec,
        dimscale: header.dimscale,
        paper_mm: None,
    };
    // A layout's paper unit: its plot settings', else what model space's unit suggests.
    let paper_code = |inches: Option<bool>| match inches {
        Some(true) => 1,
        Some(false) => 4,
        None => match model_code {
            1 | 2 | 3 | 8 | 9 | 10 | 21 => 1,
            0 => match header.measurement {
                Some(0) => 1,
                Some(_) => 4,
                None => 0,
            },
            _ => 4,
        },
    };

    let mut model = Sheet::new("Model", SheetKind::Model);
    model.units = Some(sheet_units(model_code));
    let mut out = emit::Out::default();
    em.sheet(&model_nodes, &mut out)?;
    model.extents = em.finish(&mut out)?;
    model.items = out.items;
    model.file_extents = header.ext;
    sheets.push(model);

    // Layouts in tab order: (name, paper, stated extents, nodes, paper in inches, plot
    // scale).
    type Plan<'n> = (
        String,
        Option<Rect>,
        Option<Rect>,
        Vec<&'n [Node]>,
        Option<bool>,
        Option<f64>,
    );
    let mut lay_list: Vec<Plan> = Vec::new();
    let mut sorted: Vec<&doc::LayoutDef> = layouts.iter().collect();
    sorted.sort_by_key(|l| l.order);
    let mut paper_used = false;
    for l in sorted {
        let rec = record_names
            .get(&l.block_record)
            .map(|s| s.to_ascii_lowercase());
        let Some(rec) = rec else {
            continue;
        };
        if rec == "*model_space" || l.name.eq_ignore_ascii_case("Model") {
            continue;
        }
        let mut nodes: Vec<&[Node]> = Vec::new();
        if rec == "*paper_space" {
            nodes.push(&paper_nodes);
            paper_used = true;
        }
        if let Some(&i) = block_map.get(&rec) {
            nodes.push(&templates[i]);
        }
        let stated = if rec == "*paper_space" {
            l.ext.or(header.pext)
        } else {
            l.ext
        };
        lay_list.push((
            l.name.clone(),
            l.paper,
            stated,
            nodes,
            l.paper_inches,
            l.plot_scale,
        ));
    }
    if !paper_used && !paper_nodes.is_empty() {
        // R12, or no LAYOUT objects: one layout with the paper limits.
        lay_list.push((
            "Layout1".into(),
            header.plim,
            header.pext,
            vec![&paper_nodes[..]],
            None,
            None,
        ));
    }

    for (name, paper, stated, nodes, inches, scale) in lay_list {
        let mut out = emit::Out::default();
        for n in &nodes {
            em.sheet(n, &mut out)?;
        }
        let extents = em.finish(&mut out)?;
        if out.items.is_empty() && out.viewports.is_empty() {
            continue;
        }
        let mut s = Sheet::new(name, SheetKind::Layout);
        let clip_of = |h: u64| -> Option<BezPath> {
            nodes.iter().flat_map(|ns| ns.iter()).find_map(|n| match n {
                Node::Draw { props, shapes, .. } if props.handle == h => {
                    shapes.iter().find_map(|s| match s {
                        Shape::Stroke { path, .. } | Shape::Fill { path, .. } => Some(path.clone()),
                        _ => None,
                    })
                }
                _ => None,
            })
        };
        s.viewports = emit::viewports(
            std::mem::take(&mut out.viewports),
            &|h| layer_handles.get(&h).copied(),
            &clip_of,
        );
        s.items = out.items;
        s.extents = extents;
        s.paper = paper;
        s.file_extents = stated;
        let code = paper_code(inches);
        s.units = Some(match scale {
            // At a plot scale paper space is drawn in drawing units (model space's, when
            // known): one of them prints `scale` paper units long.
            Some(k) => {
                let paper_mm = if code == 1 { 25.4 } else { 1.0 };
                let mut u = sheet_units(if model_code != 0 { model_code } else { code });
                u.paper_mm = Some(k * paper_mm);
                u
            }
            None => sheet_units(code),
        });
        sheets.push(s);
    }

    let mut all_warnings = bwarn;
    for w in std::mem::take(&mut em.warnings) {
        if !all_warnings.contains(&w) {
            all_warnings.push(w);
        }
    }
    drop(em);

    Ok(VectorScene {
        layer_opacity: Vec::new(),
        pcb: None,
        survey: None,
        format: if bytes.starts_with(tokens::BINARY_SENTINEL) {
            "DXF (binary)".into()
        } else {
            "DXF".into()
        },
        release: release_name(&header.version).map(Into::into),
        version: header.version.clone(),
        units: Units {
            code: header.insunits,
        },
        layers,
        linetypes,
        fonts,
        sheets,
        counts,
        warnings: all_warnings,
        lineweight_display: header.lwdisplay,
        point_mode: header.pdmode,
        point_size: header.pdsize,
        codepage: header.codepage.clone(),
        plot: None,
        dot_radius: 0.0,
        layout: None,
    })
}

fn clone_ent<'a>(e: &Ent<'a>) -> Ent<'a> {
    Ent {
        ty: e.ty,
        g: e.g,
        all: e.all,
        subs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &[&[u8]] = &[
        include_bytes!("../tests/data/entities.dxf"),
        include_bytes!("../tests/data/r12.dxf"),
        include_bytes!("../tests/data/layout.dxf"),
    ];

    fn binary(ascii: &[u8]) -> Vec<u8> {
        let mut w = Vec::new();
        let groups = tokens::tokenize(ascii, &mut w).unwrap();
        tokens::to_binary(&groups)
    }

    /// The same drawing as ASCII and as binary DXF reads the same.
    #[test]
    fn binary_round_trip() {
        for f in FIXTURES {
            let a = parse(f, &Limits::default()).unwrap();
            let bin = binary(f);
            assert!(is_dxf(&bin));
            let b = parse(&bin, &Limits::default()).unwrap();
            assert_eq!(b.format, "DXF (binary)");
            assert_eq!(a.version, b.version);
            assert_eq!(a.counts, b.counts);
            assert_eq!(a.warnings, b.warnings);
            assert_eq!(a.layers, b.layers);
            assert_eq!(a.sheets.len(), b.sheets.len());
            for (sa, sb) in a.sheets.iter().zip(&b.sheets) {
                assert_eq!(sa.name, sb.name);
                assert_eq!(sa.extents, sb.extents);
                assert_eq!(sa.file_extents, sb.file_extents);
                assert_eq!(sa.viewports, sb.viewports);
                assert_eq!(format!("{:?}", sa.items), format!("{:?}", sb.items));
            }
            let h = parse_header_only(&bin).unwrap();
            assert!(h.binary);
            assert_eq!(h.version, a.version);
        }
    }

    /// Damaged binary files fail cleanly too.
    #[test]
    fn damaged_binary() {
        let mut x: u64 = 0x2545_f491_4f6c_dd1d;
        for f in FIXTURES {
            let bin = binary(f);
            for n in 0..bin.len() {
                let _ = parse(&bin[..n], &Limits::default());
            }
            let head = tokens::BINARY_SENTINEL.len();
            for _ in 0..300 {
                let mut b = bin.clone();
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let at = head + (x % (b.len() - head) as u64) as usize;
                b[at] = (x >> 32) as u8;
                let _ = parse(&b, &Limits::default());
            }
        }
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// Twenty blocks, each an array of ten copies of the next, the last only an XLINE:
    /// 10²⁰ copies that add no drawn items.
    #[test]
    fn nested_arrays_of_blocks_that_draw_nothing() {
        let mut f = String::from("0\nSECTION\n2\nBLOCKS\n");
        for k in 0..20 {
            f += &format!("0\nBLOCK\n2\nB{k}\n10\n0\n20\n0\n");
            f += &format!("0\nINSERT\n2\nB{}\n70\n10\n44\n1\n", k + 1);
            f += "0\nENDBLK\n";
        }
        f += "0\nBLOCK\n2\nB20\n10\n0\n20\n0\n0\nXLINE\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n";
        f += "0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nB0\n0\nENDSEC\n0\nEOF\n";
        let limits = Limits {
            max_items: 100_000,
            ..Limits::default()
        };
        assert!(matches!(
            parse(f.as_bytes(), &limits),
            Err(CadError::TooLarge)
        ));
    }
}
