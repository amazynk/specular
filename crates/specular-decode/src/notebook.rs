//! Jupyter notebooks (`.ipynb`, nbformat 4, and 3's worksheets): the cells and the outputs
//! saved in the file, laid out as pages like a Markdown document. Nothing is ever run: what
//! a code cell shows is what its last run stored.
//!
//! Markdown cells go through the Markdown renderer (their `attachment:` pictures from the
//! cell); code cells show their prompt number and source; outputs show stream text, error
//! tracebacks (terminal colour codes dropped), plain text, LaTeX source, and PNG, JPEG, GIF
//! and SVG pictures. HTML outputs show as their plain-text twin, else as their text without
//! tags; scripts and widgets are never run, only named.

use std::sync::Arc;

use serde_json::Value;
use specular_core::Source;
use specular_epub::{HtmlDoc, HtmlSection, html_escape as esc};

use crate::{DecodeError, Decoder, Format, HeaderFacts, Result, Session};

/// Largest notebook laid out.
const MAX_NOTEBOOK: usize = 256 << 20;
/// Most pictures a notebook shows.
const MAX_PICTURES: usize = 2000;

/// One cell as read.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// "markdown", "code" or "raw".
    pub kind: String,
    pub source: String,
    pub count: Option<i64>,
    pub outputs: Vec<Output>,
}

/// One saved output.
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// stdout or stderr text.
    Stream { name: String, text: String },
    /// An error: its name, value and traceback (colour codes dropped).
    Error {
        name: String,
        value: String,
        traceback: String,
    },
    /// A result or display: its count (results) and MIME bundle.
    Data {
        count: Option<i64>,
        bundle: Vec<(String, String)>,
    },
    /// An output of a type this reader does not know (a newer nbformat's): named only.
    Other(String),
}

/// A notebook as read.
#[derive(Clone, Debug, Default)]
pub struct Notebook {
    pub format: (i64, i64),
    pub kernel: Option<String>,
    pub language: Option<String>,
    pub cells: Vec<Cell>,
}

/// A JSON string or list of strings (nbformat's "multiline string").
fn text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => String::new(),
    }
}

/// Terminal colour and cursor codes (ESC [ … letter) dropped.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Whether `d` looks like a notebook: a JSON object naming nbformat near its start.
pub fn sniff(d: &[u8]) -> bool {
    let h = &d[..d.len().min(1 << 16)];
    let start = h.iter().position(|b| !b.is_ascii_whitespace());
    start.is_some_and(|i| h[i] == b'{')
        && (contains(h, b"\"cells\"") || contains(h, b"\"worksheets\""))
        && (contains(h, b"\"nbformat\"") || d.len() <= 1 << 16 || contains_tail(d))
}

fn contains(h: &[u8], n: &[u8]) -> bool {
    h.windows(n.len()).any(|w| w == n)
}

/// nbformat writes its version keys after the cells: look at the end too.
fn contains_tail(d: &[u8]) -> bool {
    contains(&d[d.len().saturating_sub(4096)..], b"\"nbformat\"")
}

/// Reads a notebook's cells and outputs.
pub fn read(d: &[u8]) -> std::result::Result<Notebook, String> {
    let v: Value = serde_json::from_slice(d).map_err(|e| format!("not valid JSON: {e}"))?;
    let major = v.get("nbformat").and_then(Value::as_i64).unwrap_or(0);
    let minor = v.get("nbformat_minor").and_then(Value::as_i64).unwrap_or(0);
    if major < 3 {
        return Err(format!(
            "notebooks of nbformat {major} (IPython before 1.0) are not read"
        ));
    }
    let meta = v.get("metadata");
    let kernel = meta
        .and_then(|m| m.pointer("/kernelspec/display_name"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let language = meta
        .and_then(|m| m.pointer("/language_info/name"))
        .or_else(|| meta.and_then(|m| m.pointer("/kernelspec/language")))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let cells: Vec<&Value> = if major == 3 {
        v.get("worksheets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|w| w.get("cells").and_then(Value::as_array))
            .flatten()
            .collect()
    } else {
        v.get("cells")
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    };
    let mut nb = Notebook {
        format: (major, minor),
        kernel,
        language,
        cells: Vec::new(),
    };
    for c in cells {
        let mut kind = c
            .get("cell_type")
            .and_then(Value::as_str)
            .unwrap_or("raw")
            .to_owned();
        let source = if major == 3 && kind == "code" {
            text(c.get("input"))
        } else {
            text(c.get("source"))
        };
        // nbformat 3's heading cells are Markdown headings.
        let source = if kind == "heading" {
            kind = "markdown".into();
            let level = c
                .get("level")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 6);
            format!("{} {source}", "#".repeat(level as usize))
        } else {
            source
        };
        let count = c
            .get(if major == 3 {
                "prompt_number"
            } else {
                "execution_count"
            })
            .and_then(Value::as_i64);
        let mut outputs = Vec::new();
        for o in c
            .get("outputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let t = o.get("output_type").and_then(Value::as_str).unwrap_or("");
            match t {
                "stream" => outputs.push(Output::Stream {
                    name: o
                        .get(if major == 3 { "stream" } else { "name" })
                        .and_then(Value::as_str)
                        .unwrap_or("stdout")
                        .to_owned(),
                    text: text(o.get("text")),
                }),
                "error" | "pyerr" => outputs.push(Output::Error {
                    name: o
                        .get("ename")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    value: o
                        .get("evalue")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    traceback: strip_ansi(
                        &o.get("traceback")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                }),
                "execute_result" | "display_data" | "pyout" => {
                    let mut bundle = Vec::new();
                    if major == 3 {
                        // Version 3 keeps the bundle in the output itself, by short names.
                        for (k, mime) in [
                            ("text", "text/plain"),
                            ("html", "text/html"),
                            ("latex", "text/latex"),
                            ("png", "image/png"),
                            ("jpeg", "image/jpeg"),
                            ("svg", "image/svg+xml"),
                            ("javascript", "application/javascript"),
                            ("json", "application/json"),
                            ("pdf", "application/pdf"),
                        ] {
                            if o.get(k).is_some() {
                                bundle.push((mime.to_owned(), text(o.get(k))));
                            }
                        }
                    } else if let Some(Value::Object(m)) = o.get("data") {
                        for (k, val) in m {
                            bundle.push((k.clone(), text(Some(val))));
                        }
                    }
                    outputs.push(Output::Data {
                        count: o
                            .get(if major == 3 {
                                "prompt_number"
                            } else {
                                "execution_count"
                            })
                            .and_then(Value::as_i64),
                        bundle,
                    });
                }
                "" => {}
                t => outputs.push(Output::Other(t.to_owned())),
            }
        }
        // Markdown pictures attached to the cell travel in its source as data.
        if let Some(Value::Object(att)) = c.get("attachments") {
            for (name, b) in att {
                if let Some(Value::Object(m)) = Some(b)
                    && let Some((mime, data)) = m.iter().next()
                {
                    outputs.push(Output::Data {
                        count: None,
                        bundle: vec![(
                            format!("attachment:{name}"),
                            format!("{mime};{}", text(Some(data))),
                        )],
                    });
                }
            }
        }
        nb.cells.push(Cell {
            kind,
            source,
            count,
            outputs,
        });
    }
    Ok(nb)
}

/// Standard base64 (whitespace skipped); `None` on a bad character.
fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// HTML reduced to its text: tags dropped, the common entities read, blocks on lines.
fn html_text(s: &str) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    let mut skip = false;
    for c in s.chars() {
        match (in_tag, c) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let t = tag
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if matches!(t.as_str(), "script" | "style") {
                    skip = !tag.starts_with('/');
                }
                if matches!(
                    t.as_str(),
                    "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "table"
                ) && !out.ends_with('\n')
                {
                    out.push('\n');
                }
                if matches!(t.as_str(), "td" | "th") && !tag.starts_with('/') {
                    out.push('\t');
                }
            }
            (true, c) => tag.push(c),
            (false, c) if !skip => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// What a notebook became: its XHTML, pictures and counts.
pub struct Rendered {
    pub doc: HtmlDoc,
    pub pictures: usize,
    pub missing: usize,
    pub not_shown: usize,
}

/// Lays a notebook out as XHTML.
pub fn render(nb: &Notebook, title: &str) -> Rendered {
    let mut body = String::new();
    let mut headings = Vec::new();
    let mut pictures: Vec<(String, Vec<u8>)> = Vec::new();
    let mut missing = 0;
    let mut not_shown = 0;
    let add_picture = |bytes: Vec<u8>, pictures: &mut Vec<(String, Vec<u8>)>| -> Option<String> {
        if pictures.len() >= MAX_PICTURES {
            return None;
        }
        let name = format!("o{}", pictures.len());
        pictures.push((name.clone(), bytes));
        Some(name)
    };
    for (i, c) in nb.cells.iter().enumerate() {
        match c.kind.as_str() {
            "markdown" => {
                // Attachments by name.
                let att: Vec<(String, Vec<u8>)> = c
                    .outputs
                    .iter()
                    .filter_map(|o| match o {
                        Output::Data { bundle, .. } => bundle.first().and_then(|(k, v)| {
                            let name = k.strip_prefix("attachment:")?;
                            let (_, data) = v.split_once(';')?;
                            Some((name.to_owned(), base64(data)?))
                        }),
                        _ => None,
                    })
                    .collect();
                let load = |rel: &str| -> Option<Vec<u8>> {
                    let name = rel.strip_prefix("attachment:")?;
                    att.iter().find(|(n, _)| n == name).map(|(_, b)| b.clone())
                };
                let r = specular_textdoc::markdown::to_xhtml(&c.source, &load);
                let mut cell = r.section.body;
                missing += r.missing_pictures;
                // Each cell's pictures renamed apart from the other cells'.
                for (k, (name, bytes)) in r.pictures.into_iter().enumerate().rev() {
                    let new = format!("m{i}_{k}");
                    cell = cell.replace(&format!("images/{name}\""), &format!("images/{new}\""));
                    pictures.push((new, bytes));
                }
                for (t, id, depth) in r.section.headings {
                    let new = format!("c{i}-{id}");
                    cell = cell.replace(&format!("id=\"{id}\""), &format!("id=\"{new}\""));
                    headings.push((t, new, depth));
                }
                body += "<div class=\"md\">";
                body += &cell;
                body += "</div>\n";
            }
            "code" => {
                let prompt = c.count.map_or_else(|| " ".to_owned(), |n| n.to_string());
                body += &format!(
                    "<p class=\"prompt\">In [{}]:</p>\n<pre class=\"code\">{}</pre>\n",
                    esc(&prompt),
                    esc(c.source.trim_end())
                );
                for o in &c.outputs {
                    match o {
                        Output::Stream { name, text } => {
                            let class = if name == "stderr" { "err" } else { "out" };
                            body +=
                                &format!("<pre class=\"{class}\">{}</pre>\n", esc(text.trim_end()));
                        }
                        Output::Error {
                            name,
                            value,
                            traceback,
                        } => {
                            let t = if traceback.trim().is_empty() {
                                format!("{name}: {value}")
                            } else {
                                traceback.clone()
                            };
                            body += &format!("<pre class=\"err\">{}</pre>\n", esc(t.trim_end()));
                        }
                        Output::Other(t) => {
                            not_shown += 1;
                            body += &format!(
                                "<p class=\"missing\">Output not shown ({}).</p>\n",
                                esc(t)
                            );
                        }
                        Output::Data { count, bundle } => {
                            if let Some(n) = count {
                                body += &format!("<p class=\"prompt\">Out [{n}]:</p>\n");
                            }
                            let get = |m: &str| bundle.iter().find(|(k, _)| k == m).map(|(_, v)| v);
                            let picture = ["image/png", "image/jpeg", "image/gif"]
                                .iter()
                                .find_map(|m| get(m).and_then(|v| base64(v)))
                                .or_else(|| get("image/svg+xml").map(|v| v.clone().into_bytes()));
                            if let Some(bytes) = picture {
                                match add_picture(bytes, &mut pictures) {
                                    Some(name) => {
                                        body += &format!(
                                            "<p class=\"pic\"><img src=\"images/{name}\" alt=\"output\"/></p>\n"
                                        )
                                    }
                                    None => not_shown += 1,
                                }
                            } else if let Some(t) = get("text/plain") {
                                body +=
                                    &format!("<pre class=\"out\">{}</pre>\n", esc(t.trim_end()));
                            } else if let Some(t) =
                                get("text/latex").or_else(|| get("text/markdown"))
                            {
                                body +=
                                    &format!("<pre class=\"out\">{}</pre>\n", esc(t.trim_end()));
                            } else if let Some(t) = get("text/html") {
                                body += &format!(
                                    "<pre class=\"out\">{}</pre>\n",
                                    esc(html_text(t).trim())
                                );
                            } else if !bundle.is_empty() {
                                not_shown += 1;
                                let kinds: Vec<&str> =
                                    bundle.iter().map(|(k, _)| k.as_str()).collect();
                                body += &format!(
                                    "<p class=\"missing\">Output not shown ({}).</p>\n",
                                    esc(&kinds.join(", "))
                                );
                            }
                        }
                    }
                }
            }
            _ => body += &format!("<pre class=\"raw\">{}</pre>\n", esc(c.source.trim_end())),
        }
    }
    let n = pictures.len();
    Rendered {
        doc: HtmlDoc {
            title: title.to_owned(),
            css: format!(
                "{}p.prompt{{color:#3050a0;font-family:monospace;margin:0.6em 0 0.1em}}\n\
pre.code{{border-left:3px solid #8aa0d0}}\n\
pre.out{{background:none}}\n\
pre.err{{background:#fdecec;color:#900}}\n\
pre.raw{{background:none;color:#555}}\n\
p.pic img{{max-width:100%}}\n",
                specular_textdoc::markdown::CSS
            ),
            sections: vec![HtmlSection { body, headings }],
            pictures,
            ..HtmlDoc::default()
        },
        pictures: n,
        missing,
        not_shown,
    }
}

pub struct NotebookDecoder;

impl Decoder for NotebookDecoder {
    fn format(&self) -> Format {
        Format::Notebook
    }
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        sniff(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ipynb"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        if source.len() > MAX_NOTEBOOK {
            return Err(DecodeError::Unsupported(format!(
                "notebooks over {} MB are not laid out",
                MAX_NOTEBOOK >> 20
            )));
        }
        let nb = read(source.bytes()).map_err(DecodeError::Malformed)?;
        let title = source
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let r = render(&nb, &title);
        let count = |k: &str| nb.cells.iter().filter(|c| c.kind == k).count();
        let mut summary = Vec::new();
        if r.pictures > 0 {
            summary.push(("pictures", r.pictures.to_string()));
        }
        if r.missing + r.not_shown > 0 {
            summary.push(("pictures-not-shown", (r.missing + r.not_shown).to_string()));
        }
        let mut fields = vec![
            (
                "nbformat".to_owned(),
                format!("{}.{}", nb.format.0, nb.format.1),
            ),
            ("Cells".into(), nb.cells.len().to_string()),
            ("Code cells".into(), count("code").to_string()),
            ("Markdown cells".into(), count("markdown").to_string()),
        ];
        if let Some(k) = &nb.kernel {
            fields.push(("Kernel".into(), k.clone()));
        }
        if let Some(l) = &nb.language {
            fields.push(("Language".into(), l.clone()));
        }
        let outputs: usize = nb
            .cells
            .iter()
            .filter(|c| c.kind == "code")
            .map(|c| c.outputs.len())
            .sum();
        fields.push(("Saved outputs".into(), outputs.to_string()));
        let search: String = nb
            .cells
            .iter()
            .map(|c| c.source.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(256 << 10)
            .collect();
        let coding = format!(
            "Jupyter notebook{}, saved outputs only",
            nb.language
                .as_ref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default()
        );
        crate::text_dec::pages(
            source,
            Format::Notebook,
            &r.doc,
            None,
            coding,
            HeaderFacts { summary, fields },
            Some(search),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NB: &str = r##"{"cells":[
      {"cell_type":"markdown","metadata":{},"source":["# Title\n","Some *text*"]},
      {"cell_type":"code","execution_count":3,"metadata":{},"source":"print(1)",
       "outputs":[{"output_type":"stream","name":"stdout","text":["1\n"]},
                  {"output_type":"execute_result","execution_count":3,"metadata":{},
                   "data":{"text/plain":["2"],"image/png":"iVBORw0KGgo="}},
                  {"output_type":"error","ename":"E","evalue":"v","traceback":["\u001b[31mBoom\u001b[0m"]}]}],
     "metadata":{"kernelspec":{"display_name":"Python 3","language":"python"}},
     "nbformat":4,"nbformat_minor":5}"##;

    #[test]
    fn cells_and_outputs() {
        assert!(sniff(NB.as_bytes()));
        let nb = read(NB.as_bytes()).unwrap();
        assert_eq!(nb.cells.len(), 2);
        assert_eq!(nb.cells[0].source, "# Title\nSome *text*");
        assert_eq!(nb.cells[1].count, Some(3));
        assert_eq!(nb.cells[1].outputs.len(), 3);
        match &nb.cells[1].outputs[2] {
            Output::Error { traceback, .. } => assert_eq!(traceback, "Boom"),
            o => panic!("{o:?}"),
        }
        let r = render(&nb, "t");
        let body = &r.doc.sections[0].body;
        assert!(body.contains("In [3]:"));
        assert!(body.contains("print(1)"));
        assert!(body.contains("images/o0"));
        assert_eq!(r.pictures, 1);
        assert_eq!(&r.doc.pictures[0].1[..4], b"\x89PNG");
        assert!(!sniff(b"{\"a\": 1}"));
    }

    #[test]
    fn base64_and_html() {
        assert_eq!(base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(
            html_text("<table><tr><td>a</td><td>b&amp;c</td></tr></table>").trim(),
            "a\tb&c"
        );
    }
}
