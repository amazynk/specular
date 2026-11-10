//! The ShapeSheet model: cells, sections and rows of shapes, style sheets and page sheets,
//! parsed from the package XML into owned values.

use std::collections::HashMap;

use specular_drawpage::xml::roxmltree::Node;

/// Most shapes read per part (nested included).
pub(crate) const MAX_SHAPES: usize = 200_000;
/// Deepest group nesting read.
pub(crate) const MAX_DEPTH: usize = 64;
/// Most rows per section.
const MAX_ROWS: usize = 100_000;

#[derive(Debug, Clone, Default)]
pub(crate) struct Cell {
    pub v: String,
    pub f: Option<String>,
}

impl Cell {
    pub fn num(&self) -> Option<f64> {
        let v = self.v.trim();
        v.parse::<f64>().ok().filter(|x| x.is_finite())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Row {
    /// Row type (geometry rows: "MoveTo", "LineTo"…).
    pub t: Option<String>,
    /// Index or name: the key rows of the same section are matched by.
    pub key: String,
    pub del: bool,
    pub cells: HashMap<String, Cell>,
}

impl Row {
    pub fn num(&self, n: &str) -> Option<f64> {
        self.cells.get(n).and_then(Cell::num)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Section {
    pub n: String,
    pub ix: Option<u32>,
    pub del: bool,
    /// Cells of the section itself (Geometry: NoFill, NoLine, NoShow).
    pub cells: HashMap<String, Cell>,
    pub rows: Vec<Row>,
}

/// A piece of a shape's text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Piece {
    Text(String),
    /// Character format row from here on.
    Cp(u32),
    /// Paragraph format row from here on.
    Pp(u32),
    Tab,
    /// A field's displayed text.
    Field(String),
}

/// A shape, style sheet or page sheet.
#[derive(Debug, Clone, Default)]
pub(crate) struct Sheet {
    pub id: u32,
    /// "Shape", "Group", "Foreign", "Guide".
    pub kind: String,
    pub master: Option<u32>,
    pub master_shape: Option<u32>,
    pub line_style: Option<u32>,
    pub fill_style: Option<u32>,
    pub text_style: Option<u32>,
    pub del: bool,
    pub cells: HashMap<String, Cell>,
    pub sections: Vec<Section>,
    pub text: Option<Vec<Piece>>,
    /// Relationship id of the foreign (image) data.
    pub foreign: Option<String>,
    pub foreign_type: Option<String>,
    pub children: Vec<Sheet>,
}

impl Sheet {
    pub fn num(&self, n: &str) -> Option<f64> {
        self.cells.get(n).and_then(Cell::num)
    }

    pub fn section(&self, n: &str, ix: Option<u32>) -> Option<&Section> {
        self.sections.iter().find(|s| s.n == n && s.ix == ix)
    }

    /// The shape with `id` in this sheet's subtree (itself included).
    pub fn find(&self, id: u32) -> Option<&Sheet> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }
}

fn u32_attr(n: Node, name: &str) -> Option<u32> {
    n.attribute(name).and_then(|v| v.trim().parse().ok())
}

fn cell_of(n: Node) -> Option<(String, Cell)> {
    let name = n.attribute("N")?;
    Some((
        name.to_string(),
        Cell {
            v: n.attribute("V").unwrap_or("").to_string(),
            f: n.attribute("F").map(str::to_string),
        },
    ))
}

/// Reads the cells, sections, text and nested shapes of a `Shape`, `StyleSheet` or
/// `PageSheet` element. `count` caps the shapes read across a part.
pub(crate) fn parse_sheet(n: Node, count: &mut usize, depth: usize) -> Sheet {
    *count += 1;
    let mut s = Sheet {
        id: u32_attr(n, "ID").unwrap_or(0),
        kind: n.attribute("Type").unwrap_or("Shape").to_string(),
        master: u32_attr(n, "Master"),
        master_shape: u32_attr(n, "MasterShape"),
        line_style: u32_attr(n, "LineStyle"),
        fill_style: u32_attr(n, "FillStyle"),
        text_style: u32_attr(n, "TextStyle"),
        del: n.attribute("Del") == Some("1"),
        ..Sheet::default()
    };
    for c in n.children().filter(Node::is_element) {
        match c.tag_name().name() {
            "Cell" => {
                if let Some((k, v)) = cell_of(c) {
                    s.cells.insert(k, v);
                }
            }
            "Section" => {
                let mut sec = Section {
                    n: c.attribute("N").unwrap_or("").to_string(),
                    ix: u32_attr(c, "IX"),
                    del: c.attribute("Del") == Some("1"),
                    cells: HashMap::new(),
                    rows: Vec::new(),
                };
                for r in c.children().filter(|r| r.is_element()) {
                    if sec.rows.len() >= MAX_ROWS {
                        break;
                    }
                    match r.tag_name().name() {
                        "Row" => {
                            let mut row = Row {
                                t: r.attribute("T").map(str::to_string),
                                key: r
                                    .attribute("IX")
                                    .or_else(|| r.attribute("N"))
                                    .unwrap_or("")
                                    .to_string(),
                                del: r.attribute("Del") == Some("1"),
                                cells: HashMap::new(),
                            };
                            if row.key.is_empty() {
                                row.key = format!("#{}", sec.rows.len());
                            }
                            for rc in r.children().filter(|x| x.is_element()) {
                                if rc.tag_name().name() == "Cell"
                                    && let Some((k, v)) = cell_of(rc)
                                {
                                    row.cells.insert(k, v);
                                }
                            }
                            sec.rows.push(row);
                        }
                        "Cell" => {
                            if let Some((k, v)) = cell_of(r) {
                                sec.cells.insert(k, v);
                            }
                        }
                        _ => {}
                    }
                }
                s.sections.push(sec);
            }
            "Text" => s.text = Some(parse_text(c)),
            "ForeignData" => {
                s.foreign_type = c.attribute("ForeignType").map(str::to_string);
                if let Some(rel) = c
                    .children()
                    .find(|x| x.is_element() && x.tag_name().name() == "Rel")
                {
                    s.foreign = rel
                        .attributes()
                        .find(|a| a.name() == "id")
                        .map(|a| a.value().to_string());
                }
            }
            "Shapes" if depth < MAX_DEPTH => {
                for sh in c
                    .children()
                    .filter(|x| x.is_element() && x.tag_name().name() == "Shape")
                {
                    if *count >= MAX_SHAPES {
                        break;
                    }
                    s.children.push(parse_sheet(sh, count, depth + 1));
                }
            }
            _ => {}
        }
    }
    s
}

fn parse_text(n: Node) -> Vec<Piece> {
    let mut out = Vec::new();
    for c in n.children() {
        if c.is_text() {
            if let Some(t) = c.text() {
                out.push(Piece::Text(t.to_string()));
            }
            continue;
        }
        if !c.is_element() {
            continue;
        }
        match c.tag_name().name() {
            "cp" => out.push(Piece::Cp(u32_attr(c, "IX").unwrap_or(0))),
            "pp" => out.push(Piece::Pp(u32_attr(c, "IX").unwrap_or(0))),
            "tp" => out.push(Piece::Tab),
            "fld" => out.push(Piece::Field(
                c.descendants()
                    .filter(|d| d.is_text())
                    .filter_map(|d| d.text())
                    .collect(),
            )),
            _ => {}
        }
    }
    out
}

/// The top-level shapes of a `PageContents` / `MasterContents` element.
pub(crate) fn parse_shapes(root: Node, count: &mut usize) -> Vec<Sheet> {
    let mut out = Vec::new();
    if let Some(shapes) = root
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "Shapes")
    {
        for sh in shapes
            .children()
            .filter(|x| x.is_element() && x.tag_name().name() == "Shape")
        {
            if *count >= MAX_SHAPES {
                break;
            }
            out.push(parse_sheet(sh, count, 1));
        }
    }
    out
}
