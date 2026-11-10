//! An own reader for GDSII stream files (IC layouts).
//!
//! [`parse`] reads the records into a [`Library`]: units, cells with their boundaries,
//! paths, boxes, texts and references (SREF, AREF with reflection, magnification and
//! rotation), nodes and properties counted. Nothing is expanded: [`hier`] finds the top
//! cells, cuts reference cycles, and computes exact bounds and flat counts without
//! flattening; [`hier::flatten`] expands a cell with a cap for the tests; [`scene`] turns
//! the library into a vector scene whose cells are prepared once and drawn instanced.
//!
//! Records are bounded by their 16-bit length and checked against the file; a truncated
//! file, an element left open or a record out of place is refused with the offset.
//! OASIS files are recognised and refused (a different format).

pub mod geom;
pub mod hier;
pub mod records;
pub mod scene;

pub use specular_vector::{bounds_of, kurbo};

use std::collections::HashMap;

use records::{Reader, Record};

/// Why a file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum GdsError {
    #[error("not a GDSII stream file (no HEADER record)")]
    NotGds,
    #[error("an OASIS layout, which is not supported (only GDSII is)")]
    Oasis,
    #[error("the file is truncated (the record at offset {0} is cut off)")]
    Truncated(usize),
    #[error("malformed GDSII: {0}")]
    Malformed(String),
    #[error("too large: {0}")]
    TooLarge(String),
}

/// Limits on what [`parse`] keeps.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Coordinates stored over all cells.
    pub max_points: usize,
    /// Elements over all cells.
    pub max_elements: usize,
    pub max_cells: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_points: 200_000_000,
            max_elements: 50_000_000,
            max_cells: 2_000_000,
        }
    }
}

/// Whether `bytes` start like a GDSII stream: a HEADER record (16-bit version), then
/// BGNLIB when there is enough of the file to see it.
pub fn is_gds(bytes: &[u8]) -> bool {
    if bytes.len() < 6 || bytes[..4] != [0x00, 0x06, records::HEADER, records::DT_I16] {
        return false;
    }
    bytes.len() < 10 || bytes[8..10] == [records::BGNLIB, records::DT_I16]
}

/// Whether `bytes` are an OASIS layout (`%SEMI-OASIS` magic).
pub fn is_oasis(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%SEMI-OASIS")
}

/// Reflection, magnification and rotation of a reference or text.
#[derive(Clone, Debug, PartialEq)]
pub struct Strans {
    /// Reflected about the x axis (before rotation).
    pub reflect: bool,
    /// Magnification and angle are absolute (not multiplied by the parent's).
    pub abs_mag: bool,
    pub abs_angle: bool,
    pub mag: f64,
    /// Counter-clockwise, degrees.
    pub angle: f64,
}

impl Default for Strans {
    fn default() -> Self {
        Strans {
            reflect: false,
            abs_mag: false,
            abs_angle: false,
            mag: 1.0,
            angle: 0.0,
        }
    }
}

/// What a shape is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeKind {
    /// A polygon (closing point dropped).
    Boundary,
    /// A box (stored as its outline).
    Box,
    /// A path: width (negative: absolute, not magnified), pathtype (0 flush, 1 round,
    /// 2 half-width extension, 4 custom extensions) and the extensions of pathtype 4.
    Path {
        width: i32,
        pathtype: i16,
        bgn: i32,
        end: i32,
    },
}

/// A polygon, box or path; its points are `Cell::points[start..end]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// Index into [`Library::layers`].
    pub layer: u32,
    pub kind: ShapeKind,
    pub start: u32,
    pub end: u32,
}

/// A text label.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    /// Index into [`Library::layers`] (layer and texttype).
    pub layer: u32,
    pub text: String,
    pub origin: [i32; 2],
    /// Font (bits 4-5), vertical (2-3: top, middle, bottom) and horizontal (0-1: left,
    /// centre, right) justification.
    pub presentation: u16,
    pub strans: Strans,
    pub width: i32,
}

/// An array of placements: columns, rows and the steps between them (database units,
/// parent coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Array {
    pub cols: u16,
    pub rows: u16,
    pub col: [f64; 2],
    pub row: [f64; 2],
}

/// A placement of another cell (SREF, or AREF with an array).
#[derive(Clone, Debug, PartialEq)]
pub struct Ref {
    /// The cell placed; `None` when it is not defined, or the reference closes a cycle.
    pub cell: Option<u32>,
    pub name: String,
    pub origin: [i32; 2],
    pub strans: Strans,
    pub array: Option<Array>,
}

impl Ref {
    /// Placements it makes.
    pub fn count(&self) -> u64 {
        self.array
            .map_or(1, |a| u64::from(a.cols) * u64::from(a.rows))
    }
}

/// One structure (cell).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub name: String,
    /// Coordinates of every shape, database units.
    pub points: Vec<[i32; 2]>,
    pub shapes: Vec<Shape>,
    pub texts: Vec<Text>,
    pub refs: Vec<Ref>,
    /// NODE elements (electrical nets, not drawn).
    pub nodes: u32,
}

impl Cell {
    /// The points of a shape.
    pub fn shape_points(&self, s: &Shape) -> &[[i32; 2]] {
        self.points
            .get(s.start as usize..s.end as usize)
            .unwrap_or(&[])
    }
}

/// Elements by kind as stored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub boundaries: u64,
    pub paths: u64,
    pub boxes: u64,
    pub texts: u64,
    pub nodes: u64,
    pub srefs: u64,
    pub arefs: u64,
    pub properties: u64,
}

/// A GDSII library.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub name: String,
    /// The stream version (HEADER), e.g. 600.
    pub version: i16,
    /// Last modification as stored (year, month, day, hour, minute, second).
    pub modified: Option<[i16; 6]>,
    /// User units per database unit (0.001 for a micrometre user unit and nanometre
    /// database unit).
    pub user_unit: f64,
    /// Metres per database unit.
    pub db_unit_m: f64,
    /// Layer and datatype (texttype, boxtype) pairs, sorted.
    pub layers: Vec<(u16, u16)>,
    pub cells: Vec<Cell>,
    /// Cells nothing references (after cycles are cut), in file order.
    pub top: Vec<u32>,
    /// Problems that did not stop the parse, deduplicated.
    pub warnings: Vec<String>,
    pub counts: Counts,
}

impl Library {
    /// The index of the cell named `name`.
    pub fn cell_index(&self, name: &str) -> Option<u32> {
        self.cells
            .iter()
            .position(|c| c.name == name)
            .map(|i| i as u32)
    }

    /// The name of a layer, "layer/datatype".
    pub fn layer_name(&self, i: u32) -> String {
        self.layers
            .get(i as usize)
            .map_or_else(String::new, |(l, d)| format!("{l}/{d}"))
    }
}

/// Warnings kept (further ones are counted in the last).
const MAX_WARNINGS: usize = 64;

fn warn(warnings: &mut Vec<String>, w: String) {
    if warnings.len() < MAX_WARNINGS && !warnings.contains(&w) {
        warnings.push(w);
    }
}

/// An element being read.
#[derive(Default)]
struct Element {
    rtype: u8,
    offset: usize,
    layer: Option<i16>,
    datatype: i16,
    width: i32,
    pathtype: i16,
    bgn: i32,
    end: i32,
    xy: Vec<[i32; 2]>,
    sname: String,
    strans: Strans,
    colrow: Option<(i16, i16)>,
    string: String,
    presentation: u16,
}

struct Parser {
    lib: Library,
    limits: Limits,
    layer_ids: HashMap<(u16, u16), u32>,
    points: usize,
    elements: usize,
}

/// Reads a GDSII stream.
pub fn parse(bytes: &[u8], limits: Limits) -> Result<Library, GdsError> {
    if is_oasis(bytes) {
        return Err(GdsError::Oasis);
    }
    let mut rd = Reader::new(bytes);
    let first = match rd.next_record() {
        Ok(Some(r)) => r,
        _ => return Err(GdsError::NotGds),
    };
    if first.rtype != records::HEADER || !matches!(first.dtype, records::DT_I16 | records::DT_NONE)
    {
        return Err(GdsError::NotGds);
    }
    let mut p = Parser {
        lib: Library {
            version: first.i16().unwrap_or(0),
            user_unit: 1e-3,
            db_unit_m: 1e-9,
            ..Library::default()
        },
        limits,
        layer_ids: HashMap::new(),
        points: 0,
        elements: 0,
    };
    let mut cell: Option<Cell> = None;
    let mut units_seen = false;
    loop {
        let Some(r) = rd.next_record()? else {
            return Err(GdsError::Truncated(rd.pos()));
        };
        match r.rtype {
            records::BGNLIB => {
                let v: Vec<i16> = r.i16s().collect();
                if v.len() >= 6 {
                    p.lib.modified = Some([v[0], v[1], v[2], v[3], v[4], v[5]]);
                }
            }
            records::LIBNAME => p.lib.name = r.string(),
            records::UNITS => {
                let u = r.reals()?;
                if u.len() < 2 {
                    return Err(GdsError::Malformed(format!(
                        "UNITS record at offset {} has {} values",
                        r.offset,
                        u.len()
                    )));
                }
                let (user, db) = (u[0], u[1]);
                if user.is_finite() && user > 0.0 && db.is_finite() && db > 0.0 {
                    p.lib.user_unit = user;
                    p.lib.db_unit_m = db;
                } else {
                    warn(
                        &mut p.lib.warnings,
                        format!("invalid units ({user}, {db} m); assuming 1 nm"),
                    );
                }
                units_seen = true;
            }
            records::BGNSTR => {
                if cell.is_some() {
                    return Err(misplaced(&r, "inside a structure"));
                }
                if p.lib.cells.len() >= p.limits.max_cells {
                    return Err(GdsError::TooLarge(format!(
                        "more than {} cells",
                        p.limits.max_cells
                    )));
                }
                cell = Some(Cell::default());
            }
            records::STRNAME => match cell.as_mut() {
                Some(c) => c.name = r.string(),
                None => return Err(misplaced(&r, "outside a structure")),
            },
            records::ENDSTR => match cell.take() {
                Some(mut c) => {
                    c.points.shrink_to_fit();
                    if c.name.is_empty() {
                        c.name = format!("#{}", p.lib.cells.len());
                    }
                    p.lib.cells.push(c);
                }
                None => return Err(misplaced(&r, "outside a structure")),
            },
            records::BOUNDARY
            | records::PATH
            | records::SREF
            | records::AREF
            | records::TEXT
            | records::NODE
            | records::BOX => {
                let Some(c) = cell.as_mut() else {
                    return Err(misplaced(&r, "outside a structure"));
                };
                let e = read_element(&mut rd, r, &mut p.lib.counts)?;
                p.add(c, e)?;
            }
            records::ENDLIB => {
                if cell.is_some() {
                    return Err(misplaced(&r, "inside a structure"));
                }
                break;
            }
            records::ENDEL
            | records::XY
            | records::LAYER
            | records::DATATYPE
            | records::SNAME
            | records::COLROW => {
                return Err(misplaced(&r, "outside an element"));
            }
            // REFLIBS, FONTS, GENERATIONS, ATTRTABLE, FORMAT, masks, LIBDIRSIZE,
            // SRFNAME, LIBSECUR and stray properties: nothing to draw.
            _ => {}
        }
    }
    if !units_seen {
        warn(
            &mut p.lib.warnings,
            "no UNITS record; assuming a 1 nm database unit".into(),
        );
    }
    let mut lib = p.finish();
    hier::resolve(&mut lib);
    Ok(lib)
}

fn misplaced(r: &Record<'_>, where_: &str) -> GdsError {
    GdsError::Malformed(format!(
        "{} record at offset {} {where_}",
        records::name(r.rtype),
        r.offset
    ))
}

/// Reads the records of an element up to its ENDEL.
fn read_element(
    rd: &mut Reader<'_>,
    start: Record<'_>,
    counts: &mut Counts,
) -> Result<Element, GdsError> {
    let mut e = Element {
        rtype: start.rtype,
        offset: start.offset,
        ..Element::default()
    };
    loop {
        let Some(r) = rd.next_record()? else {
            return Err(GdsError::Truncated(rd.pos()));
        };
        match r.rtype {
            records::ENDEL => return Ok(e),
            records::LAYER => e.layer = Some(r.i16()?),
            records::DATATYPE | records::TEXTTYPE | records::BOXTYPE | records::NODETYPE => {
                e.datatype = r.i16()?
            }
            records::WIDTH => e.width = r.i32()?,
            records::PATHTYPE => e.pathtype = r.i16()?,
            records::BGNEXTN => e.bgn = r.i32()?,
            records::ENDEXTN => e.end = r.i32()?,
            records::XY => e.xy.extend(r.points()?),
            records::SNAME => e.sname = r.string(),
            records::STRANS => {
                let bits = r.i16()? as u16;
                e.strans.reflect = bits & 0x8000 != 0;
                e.strans.abs_mag = bits & 0x0004 != 0;
                e.strans.abs_angle = bits & 0x0002 != 0;
            }
            records::MAG => e.strans.mag = r.real()?,
            records::ANGLE => e.strans.angle = r.real()?,
            records::COLROW => {
                let v: Vec<i16> = r.i16s().collect();
                if v.len() < 2 {
                    return Err(GdsError::Malformed(format!(
                        "COLROW record at offset {} is short",
                        r.offset
                    )));
                }
                e.colrow = Some((v[0], v[1]));
            }
            records::STRING => e.string = r.string(),
            records::PRESENTATION => e.presentation = r.i16()? as u16,
            records::PROPATTR => counts.properties += 1,
            records::BOUNDARY
            | records::PATH
            | records::SREF
            | records::AREF
            | records::TEXT
            | records::NODE
            | records::BOX
            | records::BGNSTR
            | records::STRNAME
            | records::ENDSTR
            | records::BGNLIB
            | records::ENDLIB
            | records::HEADER
            | records::UNITS => {
                return Err(GdsError::Malformed(format!(
                    "{} record at offset {} inside the {} element at offset {} (no ENDEL)",
                    records::name(r.rtype),
                    r.offset,
                    records::name(e.rtype),
                    e.offset
                )));
            }
            // PROPVALUE, ELFLAGS, PLEX, SPACING, ...: not needed.
            _ => {}
        }
    }
}

impl Parser {
    fn layer(&mut self, layer: i16, datatype: i16) -> u32 {
        let key = (layer as u16, datatype as u16);
        let next = self.layer_ids.len() as u32;
        *self.layer_ids.entry(key).or_insert(next)
    }

    fn skip(&mut self, e: &Element, why: &str) {
        warn(
            &mut self.lib.warnings,
            format!(
                "skipped a {} element ({why}), first at offset {}",
                records::name(e.rtype),
                e.offset
            ),
        );
    }

    fn add(&mut self, c: &mut Cell, mut e: Element) -> Result<(), GdsError> {
        self.elements += 1;
        if self.elements > self.limits.max_elements {
            return Err(GdsError::TooLarge(format!(
                "more than {} elements",
                self.limits.max_elements
            )));
        }
        let counts = &mut self.lib.counts;
        match e.rtype {
            records::BOUNDARY => counts.boundaries += 1,
            records::BOX => counts.boxes += 1,
            records::PATH => counts.paths += 1,
            records::TEXT => counts.texts += 1,
            records::NODE => counts.nodes += 1,
            records::SREF => counts.srefs += 1,
            _ => counts.arefs += 1,
        }
        match e.rtype {
            records::NODE => {
                c.nodes += 1;
            }
            records::BOUNDARY | records::BOX | records::PATH => {
                let Some(layer) = e.layer else {
                    self.skip(&e, "no LAYER");
                    return Ok(());
                };
                let closed = e.rtype != records::PATH;
                if closed && e.xy.len() > 1 && e.xy.first() == e.xy.last() {
                    e.xy.pop();
                }
                let need = if closed { 3 } else { 2 };
                if e.xy.len() < need {
                    self.skip(&e, "too few points");
                    return Ok(());
                }
                self.points += e.xy.len();
                if self.points > self.limits.max_points {
                    return Err(GdsError::TooLarge(format!(
                        "more than {} coordinates",
                        self.limits.max_points
                    )));
                }
                let kind = match e.rtype {
                    records::BOUNDARY => ShapeKind::Boundary,
                    records::BOX => ShapeKind::Box,
                    _ => ShapeKind::Path {
                        width: e.width,
                        pathtype: e.pathtype,
                        bgn: e.bgn,
                        end: e.end,
                    },
                };
                let layer = self.layer(layer, e.datatype);
                let start = c.points.len() as u32;
                c.points.extend_from_slice(&e.xy);
                c.shapes.push(Shape {
                    layer,
                    kind,
                    start,
                    end: c.points.len() as u32,
                });
            }
            records::TEXT => {
                let (Some(layer), Some(&origin)) = (e.layer, e.xy.first()) else {
                    self.skip(&e, "no LAYER or position");
                    return Ok(());
                };
                let layer = self.layer(layer, e.datatype);
                c.texts.push(Text {
                    layer,
                    text: std::mem::take(&mut e.string),
                    origin,
                    presentation: e.presentation,
                    strans: e.strans.clone(),
                    width: e.width,
                });
            }
            records::SREF => {
                let Some(&origin) = e.xy.first() else {
                    self.skip(&e, "no position");
                    return Ok(());
                };
                c.refs.push(Ref {
                    cell: None,
                    name: std::mem::take(&mut e.sname),
                    origin,
                    strans: e.strans.clone(),
                    array: None,
                });
            }
            _ => {
                let (Some((cols, rows)), [p1, p2, p3, ..]) = (e.colrow, e.xy.as_slice()) else {
                    self.skip(&e, "no COLROW or fewer than 3 points");
                    return Ok(());
                };
                if cols <= 0 || rows <= 0 {
                    self.skip(&e, "no columns or rows");
                    return Ok(());
                }
                let d = |a: [i32; 2], b: [i32; 2], n: i16| {
                    [
                        (f64::from(b[0]) - f64::from(a[0])) / f64::from(n),
                        (f64::from(b[1]) - f64::from(a[1])) / f64::from(n),
                    ]
                };
                c.refs.push(Ref {
                    cell: None,
                    name: std::mem::take(&mut e.sname),
                    origin: *p1,
                    strans: e.strans.clone(),
                    array: Some(Array {
                        cols: cols as u16,
                        rows: rows as u16,
                        col: d(*p1, *p2, cols),
                        row: d(*p1, *p3, rows),
                    }),
                });
            }
        }
        Ok(())
    }

    /// Sorts the layers and renumbers what uses them.
    fn finish(mut self) -> Library {
        let mut layers: Vec<((u16, u16), u32)> = self.layer_ids.into_iter().collect();
        layers.sort_unstable();
        let mut map = vec![0u32; layers.len()];
        for (new, (_, old)) in layers.iter().enumerate() {
            map[*old as usize] = new as u32;
        }
        for c in &mut self.lib.cells {
            for s in &mut c.shapes {
                s.layer = map[s.layer as usize];
            }
            for t in &mut c.texts {
                t.layer = map[t.layer as usize];
            }
        }
        self.lib.layers = layers.into_iter().map(|(k, _)| k).collect();
        self.lib
    }
}

#[cfg(test)]
mod tests;
