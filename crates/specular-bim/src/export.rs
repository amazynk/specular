//! Check results written out: as BCF topics (one per issue, with a viewpoint looking at the
//! elements involved and them selected), as CSV (UTF-8 with a byte order mark, so
//! spreadsheets read accents right) and as XLSX (one sheet per table, plain cells, the
//! header row bold).

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use crate::bcf::{
    Archive, BcfCamera, CameraKind, Coloring, Component, Components, HeaderFile, Topic, Version,
    Viewpoint, VisInfo,
};
use crate::ids::{new_guid, now};

/// One issue to report as a topic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Issue {
    pub title: String,
    pub description: Option<String>,
    /// Elements the issue is about (IFC GUIDs): selected in the viewpoint.
    pub guids: Vec<String>,
    /// Elements drawn in a colour of their own, with the colour (`RRGGBB`).
    pub coloured: Vec<(String, Vec<String>)>,
    /// What the viewpoint looks at, in BCF's frame (metres, the project's coordinates).
    pub focus: Option<([f64; 3], [f64; 3])>,
    pub labels: Vec<String>,
    pub topic_type: Option<String>,
    pub status: Option<String>,
    /// The model files the issue is about, when not only the one model (a clash between
    /// two models of a federation names both).
    pub files: Vec<HeaderFile>,
}

/// A perspective camera looking at the box from above one corner, the box filling the view.
pub fn camera_on(min: [f64; 3], max: [f64; 3]) -> BcfCamera {
    let c: [f64; 3] = std::array::from_fn(|k| (min[k] + max[k]) / 2.0);
    let r = (0..3)
        .map(|k| (max[k] - min[k]).powi(2))
        .sum::<f64>()
        .sqrt()
        .max(0.5)
        / 2.0;
    let fov: f64 = 60.0;
    let dist = r / (fov.to_radians() / 2.0).sin() * 1.1;
    let d = [-0.577_350_269, 0.577_350_269, -0.577_350_269];
    let up = [-0.408_248_29, 0.408_248_29, 0.816_496_58];
    BcfCamera {
        kind: CameraKind::Perspective { fov },
        viewpoint: std::array::from_fn(|k| c[k] - d[k] * dist),
        direction: d,
        up,
        aspect: Some(16.0 / 9.0),
    }
}

/// An archive of one topic per issue, by `author`, about the model file `model`.
pub fn topics(issues: &[Issue], author: &str, model: Option<&str>, version: Version) -> Archive {
    let stamp = now();
    let mut a = Archive {
        version,
        version_id: match version {
            Version::V2_1 => "2.1".into(),
            Version::V3_0 => "3.0".into(),
        },
        ..Archive::default()
    };
    for (i, is) in issues.iter().enumerate() {
        let components = Components {
            selection: is.guids.iter().map(|g| Component::of(g)).collect(),
            default_visibility: true,
            coloring: is
                .coloured
                .iter()
                .map(|(c, gs)| Coloring {
                    color: c.clone(),
                    components: gs.iter().map(|g| Component::of(g)).collect(),
                })
                .collect(),
            ..Components::default()
        };
        let vis = VisInfo {
            guid: new_guid(),
            components: Some(components),
            camera: is.focus.map(|(lo, hi)| camera_on(lo, hi)),
            ..VisInfo::default()
        };
        let vp = Viewpoint {
            guid: vis.guid.clone(),
            file: Some("viewpoint.bcfv".into()),
            vis: Some(vis),
            ..Viewpoint::default()
        };
        a.topics.push(Topic {
            guid: new_guid(),
            topic_type: is.topic_type.clone().or_else(|| Some("Issue".into())),
            status: is.status.clone().or_else(|| Some("Open".into())),
            title: is.title.clone(),
            index: Some(i as i32 + 1),
            labels: is.labels.clone(),
            creation_date: stamp.clone(),
            creation_author: author.to_owned(),
            description: is.description.clone(),
            files: if !is.files.is_empty() {
                is.files.clone()
            } else {
                model
                    .map(|f| {
                        vec![HeaderFile {
                            is_external: true,
                            filename: Some(f.to_owned()),
                            date: Some(stamp.clone()),
                            reference: Some(f.to_owned()),
                            ..HeaderFile::default()
                        }]
                    })
                    .unwrap_or_default()
            },
            viewpoints: vec![vp],
            ..Topic::default()
        });
    }
    a
}

/// A table cell.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Text(String),
    Number(f64),
    Empty,
}

impl From<&str> for Cell {
    fn from(s: &str) -> Cell {
        Cell::Text(s.to_owned())
    }
}

impl From<String> for Cell {
    fn from(s: String) -> Cell {
        Cell::Text(s)
    }
}

impl From<f64> for Cell {
    fn from(v: f64) -> Cell {
        Cell::Number(v)
    }
}

impl From<Option<f64>> for Cell {
    fn from(v: Option<f64>) -> Cell {
        v.map_or(Cell::Empty, Cell::Number)
    }
}

impl Cell {
    fn csv(&self) -> String {
        match self {
            Cell::Text(s) => {
                // Text that a spreadsheet would run as a formula is quoted as text.
                let s = if s.starts_with(['=', '+', '-', '@']) && s.parse::<f64>().is_err() {
                    format!("'{s}")
                } else {
                    s.clone()
                };
                if s.contains([',', '"', '\n', '\r']) {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s
                }
            }
            Cell::Number(v) if v.is_finite() => format!("{v}"),
            _ => String::new(),
        }
    }
}

/// Rows as CSV: comma separated, CRLF line ends, a UTF-8 byte order mark first.
pub fn csv(rows: &[Vec<Cell>]) -> Vec<u8> {
    let mut out = String::from("\u{FEFF}");
    for r in rows {
        let line: Vec<String> = r.iter().map(Cell::csv).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

fn col_name(mut c: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Sheet names: at most 31 characters, none of `[]:*?/\`, unique.
fn sheet_name(name: &str, taken: &[String]) -> String {
    let mut s: String = name
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    if s.trim().is_empty() {
        s = "Sheet".into();
    }
    let base = s.clone();
    let mut n = 2;
    while taken.iter().any(|t| t.eq_ignore_ascii_case(&s)) {
        let suffix = format!(" {n}");
        s = format!(
            "{}{suffix}",
            base.chars().take(31 - suffix.len()).collect::<String>()
        );
        n += 1;
    }
    s
}

/// Tables as an XLSX workbook: (sheet name, rows), the first row of each as a bold header.
pub fn xlsx(sheets: &[(String, Vec<Vec<Cell>>)]) -> Vec<u8> {
    let esc = crate::write::esc;
    let mut names: Vec<String> = Vec::new();
    for (n, _) in sheets {
        let s = sheet_name(n, &names);
        names.push(s);
    }
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opt = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut put = |path: &str, body: String| {
        let _ = z.start_file(path, opt);
        let _ = z.write_all(body.as_bytes());
    };
    let mut ct = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    for i in 0..sheets.len() {
        ct.push_str(&format!(r#"<Override PartName="/xl/worksheets/sheet{}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#, i + 1));
    }
    ct.push_str("</Types>");
    put("[Content_Types].xml", ct);
    put(
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
            .into(),
    );
    let mut wb = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>"#,
    );
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (i, n) in names.iter().enumerate() {
        wb.push_str(&format!(
            r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#,
            esc(n),
            i + 1,
            i + 1
        ));
        rels.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>"#, i + 1, i + 1));
    }
    wb.push_str("</sheets></workbook>");
    rels.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#, sheets.len() + 1));
    put("xl/workbook.xml", wb);
    put("xl/_rels/workbook.xml.rels", rels);
    put(
        "xl/styles.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#
            .into(),
    );
    for (i, (_, rows)) in sheets.iter().enumerate() {
        let mut s = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        );
        if !rows.is_empty() {
            s.push_str(r#"<sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>"#);
        }
        s.push_str("<sheetData>");
        for (r, row) in rows.iter().enumerate() {
            s.push_str(&format!(r#"<row r="{}">"#, r + 1));
            let style = if r == 0 { r#" s="1""# } else { "" };
            for (c, cell) in row.iter().enumerate() {
                let at = format!("{}{}", col_name(c), r + 1);
                match cell {
                    Cell::Text(t) => s.push_str(&format!(
                        r#"<c r="{at}" t="inlineStr"{style}><is><t xml:space="preserve">{}</t></is></c>"#,
                        esc(t)
                    )),
                    Cell::Number(v) if v.is_finite() => {
                        s.push_str(&format!(r#"<c r="{at}"{style}><v>{v}</v></c>"#))
                    }
                    _ => {}
                }
            }
            s.push_str("</row>");
        }
        s.push_str("</sheetData></worksheet>");
        put(&format!("xl/worksheets/sheet{}.xml", i + 1), s);
    }
    z.finish().map(Cursor::into_inner).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_and_guards_formulas() {
        let rows = vec![
            vec![Cell::from("Name"), Cell::from("Area")],
            vec![Cell::from("Wall, \"north\""), Cell::Number(12.5)],
            vec![Cell::from("=SUM(A1)"), Cell::Empty],
            vec![Cell::from("-3"), Cell::Number(-3.0)],
        ];
        let s = String::from_utf8(csv(&rows)).unwrap();
        assert_eq!(
            s,
            "\u{FEFF}Name,Area\r\n\"Wall, \"\"north\"\"\",12.5\r\n'=SUM(A1),\r\n-3,-3\r\n"
        );
    }

    #[test]
    fn xlsx_is_a_workbook() {
        let rows = vec![
            vec![Cell::from("Class"), Cell::from("Count")],
            vec![Cell::from("IfcWall <ext>"), Cell::Number(3.0)],
        ];
        let b = xlsx(&[("By class".into(), rows.clone()), ("By class".into(), rows)]);
        let mut z = zip::ZipArchive::new(Cursor::new(b)).unwrap();
        let mut wb = String::new();
        std::io::Read::read_to_string(&mut z.by_name("xl/workbook.xml").unwrap(), &mut wb).unwrap();
        assert!(wb.contains(r#"name="By class""#) && wb.contains(r#"name="By class 2""#));
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("xl/worksheets/sheet1.xml").unwrap(), &mut s)
            .unwrap();
        assert!(s.contains("IfcWall &lt;ext&gt;"), "{s}");
        assert!(s.contains(r#"<c r="B2"><v>3</v></c>"#), "{s}");
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(27), "AB");
    }

    #[test]
    fn issues_become_topics_with_viewpoints() {
        let is = Issue {
            title: "Wall clashes with duct".into(),
            guids: vec![
                "2nJrDaLQfJ1QPhdJR0o97J".into(),
                "16MocU_IDOF8_x3Iqllz0d".into(),
            ],
            focus: Some(([0.0; 3], [1.0, 2.0, 3.0])),
            ..Issue::default()
        };
        let a = topics(&[is], "me", Some("model.ifc"), Version::V3_0);
        assert_eq!(a.topics.len(), 1);
        let t = &a.topics[0];
        let vis = t.viewpoints[0].vis.as_ref().unwrap();
        assert_eq!(vis.components.as_ref().unwrap().selection.len(), 2);
        let cam = vis.camera.unwrap();
        // It looks at the box's centre.
        let c = [0.5, 1.0, 1.5];
        let to: Vec<f64> = (0..3).map(|k| c[k] - cam.viewpoint[k]).collect();
        let n = to.iter().map(|x| x * x).sum::<f64>().sqrt();
        for (t, d) in to.iter().zip(cam.direction) {
            assert!((t / n - d).abs() < 1e-6);
        }
        // It writes and reads back.
        let back = crate::read(&crate::write(&a, Version::V3_0)).unwrap();
        assert_eq!(back.topics[0].title, "Wall clashes with duct");
        assert!(
            crate::check::check_members(&crate::write::members(&a, Version::V3_0), Version::V3_0)
                .is_empty()
        );
    }
}
