//! Rich Text Format to XHTML: paragraphs and their alignment and indent, bold, italic,
//! underline, strike-through, superscript and subscript, font sizes and colours, line breaks
//! and tabs, simple tables (`\trowd … \cell … \row`), Unicode (`\uN` with `\ucN`
//! fallbacks), 8-bit text in the document's or the font's code page (`\ansicpg`,
//! `\fcharset`, double-byte pages too), the title and author, and PNG and JPEG pictures
//! (`\pict` with `\pngblip` / `\jpegblip`, hex or `\bin`). Headers, footers, footnotes,
//! fields' instructions, objects and hidden text are left out; other picture kinds (WMF,
//! EMF, PICT) are counted as not shown.

use encoding_rs::Encoding;
use specular_epub::{HtmlSection, html_escape as esc};

/// Groups nested deeper than this are read as flat text.
const MAX_DEPTH: usize = 512;
/// Pictures kept.
const MAX_PICTURES: usize = 512;

/// Whether `head` starts an RTF document.
pub fn sniff(head: &[u8]) -> bool {
    let h = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    h.starts_with(b"{\\rtf")
}

#[derive(Clone, Debug, Default)]
pub struct Rendered {
    pub section: HtmlSection,
    pub pictures: Vec<(String, Vec<u8>)>,
    pub title: Option<String>,
    pub author: Option<String>,
    /// The text without formatting (paragraphs on lines), for search.
    pub plain: String,
    /// Pictures of kinds not shown (WMF, EMF, PICT) and pictures past the limit.
    pub missing_pictures: usize,
    pub tables: usize,
}

pub const CSS: &str = "p{margin:0 0 0.4em 0}\ntable{border-collapse:collapse}\ntd{border:1px solid #bbb;padding:0.2em 0.4em;vertical-align:top}\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dest {
    Body,
    Skip,
    FontTable,
    ColourTable,
    Title,
    Author,
    Pict,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Fmt {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    /// 1 superscript, -1 subscript.
    script: i8,
    /// Half points.
    size: i32,
    colour: usize,
    hidden: bool,
}

impl Default for Fmt {
    fn default() -> Fmt {
        Fmt {
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            script: 0,
            size: 24,
            colour: 0,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct State {
    dest: Dest,
    fmt: Fmt,
    font: i32,
    uc: usize,
}

#[derive(Default)]
struct Pict {
    kind: Option<&'static str>,
    hex: Vec<u8>,
    pending: Option<u8>,
    width_twips: i32,
}

struct Writer {
    out: String,
    para: String,
    /// The format of the open `<span>` in `para`.
    open: Option<Fmt>,
    align: &'static str,
    indent_twips: i32,
    intbl: bool,
    row: Vec<String>,
    rows: Vec<Vec<String>>,
    colours: Vec<Option<[u8; 3]>>,
    rendered: Rendered,
}

/// The encoding of a `\fcharset` number.
fn charset_encoding(n: i32) -> Option<&'static Encoding> {
    Some(match n {
        0 => encoding_rs::WINDOWS_1252,
        77 => encoding_rs::MACINTOSH,
        128 => encoding_rs::SHIFT_JIS,
        129 => encoding_rs::EUC_KR,
        134 => encoding_rs::GBK,
        136 => encoding_rs::BIG5,
        161 => encoding_rs::WINDOWS_1253,
        162 => encoding_rs::WINDOWS_1254,
        163 => encoding_rs::WINDOWS_1258,
        177 => encoding_rs::WINDOWS_1255,
        178 => encoding_rs::WINDOWS_1256,
        186 => encoding_rs::WINDOWS_1257,
        204 => encoding_rs::WINDOWS_1251,
        222 => encoding_rs::WINDOWS_874,
        238 => encoding_rs::WINDOWS_1250,
        _ => return None,
    })
}

/// The encoding of a Windows code page number (`\ansicpg`).
fn codepage_encoding(cp: i32) -> Option<&'static Encoding> {
    Some(match cp {
        437 | 850 | 1252 => encoding_rs::WINDOWS_1252,
        866 => encoding_rs::IBM866,
        874 => encoding_rs::WINDOWS_874,
        932 => encoding_rs::SHIFT_JIS,
        936 => encoding_rs::GBK,
        949 => encoding_rs::EUC_KR,
        950 => encoding_rs::BIG5,
        1250 => encoding_rs::WINDOWS_1250,
        1251 => encoding_rs::WINDOWS_1251,
        1253 => encoding_rs::WINDOWS_1253,
        1254 => encoding_rs::WINDOWS_1254,
        1255 => encoding_rs::WINDOWS_1255,
        1256 => encoding_rs::WINDOWS_1256,
        1257 => encoding_rs::WINDOWS_1257,
        1258 => encoding_rs::WINDOWS_1258,
        10000 => encoding_rs::MACINTOSH,
        65001 => encoding_rs::UTF_8,
        _ => return None,
    })
}

impl Writer {
    fn style(&self, f: &Fmt) -> String {
        let mut s = String::new();
        if f.bold {
            s.push_str("font-weight:bold;");
        }
        if f.italic {
            s.push_str("font-style:italic;");
        }
        match (f.underline, f.strike) {
            (true, true) => s.push_str("text-decoration:underline line-through;"),
            (true, false) => s.push_str("text-decoration:underline;"),
            (false, true) => s.push_str("text-decoration:line-through;"),
            _ => {}
        }
        if f.size != 24 && f.size > 0 {
            s.push_str(&format!("font-size:{}pt;", f.size as f32 / 2.0));
        }
        if let Some(Some([r, g, b])) = self.colours.get(f.colour) {
            s.push_str(&format!("color:#{r:02x}{g:02x}{b:02x};"));
        }
        s
    }

    fn text(&mut self, t: &str, f: Fmt) {
        if t.is_empty() || f.hidden {
            return;
        }
        if self.open != Some(f) {
            self.close_span();
            let st = self.style(&f);
            let tag = match f.script {
                1 => "<sup>",
                -1 => "<sub>",
                _ => "",
            };
            self.para.push_str(&format!("<span style=\"{st}\">{tag}"));
            self.open = Some(f);
        }
        self.para.push_str(&esc(t));
        self.rendered.plain.push_str(t);
    }

    fn close_span(&mut self) {
        if let Some(f) = self.open.take() {
            self.para.push_str(match f.script {
                1 => "</sup>",
                -1 => "</sub>",
                _ => "",
            });
            self.para.push_str("</span>");
        }
    }

    fn markup(&mut self, m: &str) {
        self.close_span();
        self.para.push_str(m);
    }

    /// Ends a paragraph (`\par`).
    fn par(&mut self) {
        self.close_span();
        let content = std::mem::take(&mut self.para);
        self.rendered.plain.push('\n');
        if self.intbl {
            // Paragraphs inside a cell are lines of it.
            self.para = content;
            self.para.push_str("<br/>");
            return;
        }
        self.flush_table();
        let mut style = format!("text-align:{};", self.align);
        if self.indent_twips > 0 {
            style.push_str(&format!("margin-left:{}pt;", self.indent_twips / 20));
        }
        if content.is_empty() {
            self.out.push_str("<p>\u{a0}</p>\n");
        } else {
            self.out
                .push_str(&format!("<p style=\"{style}\">{content}</p>\n"));
        }
    }

    fn cell(&mut self) {
        self.close_span();
        let mut c = std::mem::take(&mut self.para);
        while c.ends_with("<br/>") {
            c.truncate(c.len() - 5);
        }
        self.row.push(c);
        self.rendered.plain.push('\t');
    }

    fn end_row(&mut self) {
        self.close_span();
        if !self.para.is_empty() {
            self.cell();
        }
        let row = std::mem::take(&mut self.row);
        self.rows.push(row);
        self.rendered.plain.push('\n');
    }

    fn flush_table(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.rendered.tables += 1;
        self.out.push_str("<table>\n");
        for r in std::mem::take(&mut self.rows) {
            self.out.push_str("<tr>");
            for c in r {
                self.out.push_str(&format!("<td>{c}</td>"));
            }
            self.out.push_str("</tr>\n");
        }
        self.out.push_str("</table>\n");
    }

    fn picture(&mut self, p: Pict) {
        let Some(kind) = p.kind else {
            self.rendered.missing_pictures += 1;
            return;
        };
        if self.rendered.pictures.len() >= MAX_PICTURES || p.hex.is_empty() {
            self.rendered.missing_pictures += 1;
            return;
        }
        let name = format!("r{}.{kind}", self.rendered.pictures.len());
        let width = if p.width_twips > 0 {
            format!(" width=\"{}\"", (p.width_twips / 15).max(1))
        } else {
            String::new()
        };
        self.markup(&format!("<img src=\"images/{name}\" alt=\"\"{width}/>"));
        self.rendered.pictures.push((name, p.hex));
    }
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Destinations whose text is not part of the document body.
const SKIPPED: &[&str] = &[
    "stylesheet",
    "header",
    "headerl",
    "headerr",
    "headerf",
    "footer",
    "footerl",
    "footerr",
    "footerf",
    "footnote",
    "fldinst",
    "object",
    "objdata",
    "themedata",
    "colorschememapping",
    "datastore",
    "latentstyles",
    "listtable",
    "listoverridetable",
    "rsidtbl",
    "generator",
    "xmlnstbl",
    "mmathPr",
    "pgdsctbl",
    "filetbl",
    "revtbl",
    "bkmkstart",
    "bkmkend",
    "nonshppict",
    "xe",
    "tc",
    "private",
    "pn",
    "pntxta",
    "pntxtb",
    "atnid",
    "atnauthor",
    "annotation",
    "comment",
    "doccomm",
    "operator",
    "keywords",
    "subject",
    "company",
    "manager",
    "category",
    "hlinkbase",
    "userprops",
    "wgrffmtfilter",
    "fchars",
    "lchars",
    "pgptbl",
    "protusertbl",
    "shpinst",
    "sp",
    "sn",
    "sv",
];

/// Converts an RTF document to XHTML.
pub fn to_xhtml(data: &[u8]) -> Rendered {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let mut w = Writer {
        out: String::with_capacity(data.len() / 2),
        para: String::new(),
        open: None,
        align: "left",
        indent_twips: 0,
        intbl: false,
        row: Vec::new(),
        rows: Vec::new(),
        colours: Vec::new(),
        rendered: Rendered::default(),
    };
    let mut doc_enc: &'static Encoding = encoding_rs::WINDOWS_1252;
    let mut fonts: Vec<(i32, &'static Encoding)> = Vec::new();
    let mut font_def = 0i32;
    let mut colour = [0u8; 3];
    let mut colour_set = false;
    let mut stack: Vec<State> = Vec::new();
    let mut st = State {
        dest: Dest::Body,
        fmt: Fmt::default(),
        font: 0,
        uc: 1,
    };
    let mut pict = Pict::default();
    // Characters still to skip after a `\uN`.
    let mut skip = 0usize;
    // Bytes of `\'hh` escapes, decoded together (double-byte code pages).
    let mut bytes: Vec<u8> = Vec::new();
    let mut title = String::new();
    let mut author = String::new();
    // A `\*` was just read: the group's destination is optional.
    let mut star = false;
    let mut first_in_group = false;

    let font_enc = |fonts: &[(i32, &'static Encoding)], f: i32, doc: &'static Encoding| {
        fonts.iter().find(|(n, _)| *n == f).map_or(doc, |(_, e)| *e)
    };

    macro_rules! flush_bytes {
        () => {
            if !bytes.is_empty() {
                let enc = font_enc(&fonts, st.font, doc_enc);
                let (s, _) = enc.decode_without_bom_handling(&bytes);
                let s = s.into_owned();
                bytes.clear();
                emit(&mut w, &st, &s, &mut title, &mut author);
            }
        };
    }

    fn emit(w: &mut Writer, st: &State, s: &str, title: &mut String, author: &mut String) {
        match st.dest {
            Dest::Body => w.text(s, st.fmt),
            Dest::Title => title.push_str(s),
            Dest::Author => author.push_str(s),
            _ => {}
        }
    }

    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        // Picture data: hex digits until the group ends.
        if st.dest == Dest::Pict && c != b'\\' && c != b'{' && c != b'}' {
            if let Some(v) = hex_val(c) {
                match pict.pending.take() {
                    Some(h) => pict.hex.push(h << 4 | v),
                    None => pict.pending = Some(v),
                }
            }
            i += 1;
            continue;
        }
        match c {
            b'{' => {
                flush_bytes!();
                if stack.len() >= MAX_DEPTH {
                    i += 1;
                    continue;
                }
                stack.push(st);
                first_in_group = true;
                star = false;
                i += 1;
                continue;
            }
            b'}' => {
                flush_bytes!();
                if st.dest == Dest::Pict && stack.last().is_none_or(|s| s.dest != Dest::Pict) {
                    let p = std::mem::take(&mut pict);
                    if stack.last().is_some_and(|s| s.dest == Dest::Body) {
                        w.picture(p);
                    }
                }
                if st.dest == Dest::ColourTable && colour_set {
                    // A last entry without its `;`.
                    w.colours.push(Some(colour));
                    colour_set = false;
                }
                if let Some(s) = stack.pop() {
                    st = s;
                }
                skip = 0;
                i += 1;
                continue;
            }
            b'\\' => {
                let next = data.get(i + 1).copied().unwrap_or(0);
                if next == b'\'' {
                    let v = data
                        .get(i + 2..i + 4)
                        .and_then(|h| Some(hex_val(h[0])? << 4 | hex_val(h[1])?));
                    i += 4;
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    if let Some(v) = v {
                        bytes.push(v);
                    }
                    continue;
                }
                flush_bytes!();
                if next.is_ascii_alphabetic() {
                    let s = i + 1;
                    let mut j = s;
                    while j < data.len() && data[j].is_ascii_alphabetic() && j - s < 32 {
                        j += 1;
                    }
                    let word = std::str::from_utf8(&data[s..j]).unwrap_or("");
                    let ns = j;
                    if j < data.len() && data[j] == b'-' {
                        j += 1;
                    }
                    while j < data.len() && data[j].is_ascii_digit() && j - ns < 10 {
                        j += 1;
                    }
                    let param: Option<i32> = std::str::from_utf8(&data[ns..j])
                        .ok()
                        .and_then(|p| p.parse().ok());
                    if j < data.len() && data[j] == b' ' {
                        j += 1;
                    }
                    i = j;
                    if skip > 0 {
                        skip -= 1;
                        if word != "bin" {
                            continue;
                        }
                    }
                    let first = std::mem::take(&mut first_in_group);
                    let was_star = std::mem::take(&mut star);
                    // A destination word at a group's start.
                    if first {
                        let dest = match word {
                            "fonttbl" => Some(Dest::FontTable),
                            "colortbl" => Some(Dest::ColourTable),
                            "title" => Some(Dest::Title),
                            "author" => Some(Dest::Author),
                            "pict" => Some(Dest::Pict),
                            "info" => Some(Dest::Skip),
                            "shppict" | "pntext" | "listtext" | "field" | "fldrslt" => None,
                            w if SKIPPED.contains(&w) => Some(Dest::Skip),
                            _ if was_star => Some(Dest::Skip),
                            _ => None,
                        };
                        if let Some(d) = dest {
                            // Inside `\info`, title and author are read; all else is skipped.
                            let d = match (st.dest, d) {
                                (Dest::Skip, Dest::Title | Dest::Author) => d,
                                (Dest::Skip, _) => Dest::Skip,
                                // A group inside a picture (`\*\blipuid`, `\*\picprop`).
                                (Dest::Pict, _) => Dest::Skip,
                                _ => d,
                            };
                            st.dest = d;
                            if d == Dest::Pict {
                                pict = Pict::default();
                            }
                            if d != Dest::Pict && d != Dest::Skip {
                                continue;
                            }
                            if d == Dest::Skip {
                                continue;
                            }
                        }
                    }
                    let p = param.unwrap_or(1);
                    let on = param != Some(0);
                    match (st.dest, word) {
                        (Dest::Pict, "pngblip") => pict.kind = Some("png"),
                        (Dest::Pict, "jpegblip") => pict.kind = Some("jpg"),
                        (Dest::Pict, "picwgoal") => pict.width_twips = p,
                        (Dest::Pict, "bin") => {
                            let n = usize::try_from(p).unwrap_or(0).min(data.len() - i);
                            pict.hex.extend_from_slice(&data[i..i + n]);
                            i += n;
                        }
                        (_, "bin") => {
                            // Binary data elsewhere is skipped whole.
                            i += usize::try_from(p).unwrap_or(0).min(data.len() - i);
                        }
                        (Dest::FontTable, "f") => font_def = p,
                        (Dest::FontTable, "fcharset") => {
                            if let Some(e) = charset_encoding(p) {
                                fonts.retain(|(n, _)| *n != font_def);
                                fonts.push((font_def, e));
                            }
                        }
                        (Dest::ColourTable, "red") => {
                            colour[0] = p as u8;
                            colour_set = true;
                        }
                        (Dest::ColourTable, "green") => {
                            colour[1] = p as u8;
                            colour_set = true;
                        }
                        (Dest::ColourTable, "blue") => {
                            colour[2] = p as u8;
                            colour_set = true;
                        }
                        (_, "ansicpg") => {
                            if let Some(e) = codepage_encoding(p) {
                                doc_enc = e;
                            }
                        }
                        (_, "mac") => doc_enc = encoding_rs::MACINTOSH,
                        (_, "pca" | "pc") => doc_enc = encoding_rs::IBM866,
                        (_, "uc") => st.uc = usize::try_from(p).unwrap_or(1),
                        (_, "u") => {
                            let v = param.unwrap_or(0);
                            let v = if v < 0 { v + 65536 } else { v } as u32;
                            let ch = char::from_u32(v).unwrap_or('\u{fffd}');
                            // Surrogate pairs come as two `\u`s.
                            emit(&mut w, &st, &ch.to_string(), &mut title, &mut author);
                            skip = st.uc;
                        }
                        (Dest::Body, _) => body_word(&mut w, &mut st, word, param, on),
                        (_, "f") => st.font = p,
                        _ => {}
                    }
                    continue;
                }
                i += 2;
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                match next {
                    b'*' => star = true,
                    b'\\' | b'{' | b'}' => emit(
                        &mut w,
                        &st,
                        &(next as char).to_string(),
                        &mut title,
                        &mut author,
                    ),
                    b'~' => emit(&mut w, &st, "\u{a0}", &mut title, &mut author),
                    b'_' => emit(&mut w, &st, "\u{2011}", &mut title, &mut author),
                    b'-' => {}
                    b'\n' | b'\r' if st.dest == Dest::Body => w.par(),
                    _ => {}
                }
                continue;
            }
            b'\r' | b'\n' => {
                i += 1;
                continue;
            }
            _ => {}
        }
        first_in_group = false;
        // Plain text up to the next control character.
        let s = i;
        while i < data.len() && !matches!(data[i], b'\\' | b'{' | b'}' | b'\r' | b'\n') {
            if skip > 0 {
                break;
            }
            i += 1;
        }
        if skip > 0 && i == s {
            skip -= 1;
            i += 1;
            continue;
        }
        let run = &data[s..i];
        if st.dest == Dest::ColourTable {
            for &b in run {
                if b == b';' {
                    w.colours.push(colour_set.then_some(colour));
                    colour = [0; 3];
                    colour_set = false;
                }
            }
            continue;
        }
        if matches!(st.dest, Dest::Body | Dest::Title | Dest::Author) {
            bytes.extend_from_slice(run);
        }
    }
    flush_bytes!();
    if !w.para.is_empty() || w.open.is_some() {
        w.par();
    }
    w.intbl = false;
    w.flush_table();
    let title = title.trim().to_owned();
    let author = author.trim().to_owned();
    w.rendered.title = (!title.is_empty()).then_some(title);
    w.rendered.author = (!author.is_empty()).then_some(author);
    w.rendered.section = HtmlSection {
        body: std::mem::take(&mut w.out),
        headings: Vec::new(),
    };
    w.rendered
}

/// A control word in the body.
fn body_word(w: &mut Writer, st: &mut State, word: &str, param: Option<i32>, on: bool) {
    let p = param.unwrap_or(0);
    match word {
        "par" | "sect" => w.par(),
        "line" => w.markup("<br/>"),
        "tab" => w.text("\u{2003}", st.fmt),
        "emdash" => w.text("—", st.fmt),
        "endash" => w.text("–", st.fmt),
        "bullet" => w.text("•", st.fmt),
        "lquote" => w.text("‘", st.fmt),
        "rquote" => w.text("’", st.fmt),
        "ldblquote" => w.text("“", st.fmt),
        "rdblquote" => w.text("”", st.fmt),
        "emspace" => w.text("\u{2003}", st.fmt),
        "enspace" => w.text("\u{2002}", st.fmt),
        "page" => w.par(),
        "pard" => {
            w.align = "left";
            w.indent_twips = 0;
            w.intbl = false;
        }
        "plain" => st.fmt = Fmt::default(),
        "b" => st.fmt.bold = on,
        "i" => st.fmt.italic = on,
        "ul" | "uld" | "uldb" | "ulw" | "uldash" | "ulth" | "ulwave" => st.fmt.underline = on,
        "ulnone" => st.fmt.underline = false,
        "strike" | "striked" => st.fmt.strike = on,
        "super" => st.fmt.script = 1,
        "sub" => st.fmt.script = -1,
        "nosupersub" => st.fmt.script = 0,
        "v" => st.fmt.hidden = on,
        "fs" => st.fmt.size = p.clamp(2, 400),
        "cf" => st.fmt.colour = usize::try_from(p).unwrap_or(0),
        "f" => st.font = p,
        "qc" => w.align = "center",
        "qr" => w.align = "right",
        "qj" => w.align = "justify",
        "ql" => w.align = "left",
        "li" => w.indent_twips = p,
        "intbl" => w.intbl = true,
        "cell" => w.cell(),
        "row" => {
            w.end_row();
            w.intbl = false;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_and_structure() {
        let doc = br"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fswiss Arial;}{\f1\fcharset204 Times;}}
{\colortbl;\red255\green0\blue0;}
{\info{\title My doc}{\author Ann}}
{\*\generator Test;}\pard\qc\b Title\b0\par
\pard Plain \i italic\i0 , {\cf1 red}, caf\'e9, \u8364?, {\f1 \'cf\'f0\'e8\'e2\'e5\'f2}\line next\par
{\header Not shown\par}
\trowd\cellx1000\cellx2000 \intbl A\cell B\cell\row
\pard After{\v hidden}\par}";
        let r = to_xhtml(doc);
        let h = &r.section.body;
        assert_eq!(r.title.as_deref(), Some("My doc"));
        assert_eq!(r.author.as_deref(), Some("Ann"));
        assert!(
            h.contains(
                "<p style=\"text-align:center;\"><span style=\"font-weight:bold;\">Title</span></p>"
            ),
            "{h}"
        );
        assert!(
            h.contains("<span style=\"font-style:italic;\">italic</span>"),
            "{h}"
        );
        assert!(
            h.contains("<span style=\"color:#ff0000;\">red</span>"),
            "{h}"
        );
        assert!(h.contains("café, €, Привет"), "{h}");
        assert!(h.contains("<br/>"), "{h}");
        assert!(h.contains("<table>\n<tr><td><span style=\"\">A</span></td><td><span style=\"\">B</span></td></tr>"), "{h}");
        assert!(
            !h.contains("Not shown") && !h.contains("hidden") && !h.contains("Test"),
            "{h}"
        );
        assert!(h.contains("After"), "{h}");
        assert_eq!(r.tables, 1);
        assert!(r.plain.contains("Plain italic"));
    }

    #[test]
    fn pictures() {
        let png = b"\x89PNG\r\n\x1a\n\0\0";
        let hex: String = png.iter().map(|b| format!("{b:02x}")).collect();
        let doc = format!(
            "{{\\rtf1 {{\\*\\shppict{{\\pict\\pngblip\\picwgoal1500 {hex}}}}}{{\\nonshppict{{\\pict\\wmetafile8 0102}}}}{{\\pict\\emfblip 0102}}\\par}}"
        );
        let r = to_xhtml(doc.as_bytes());
        assert_eq!(r.pictures.len(), 1);
        assert_eq!(r.pictures[0].1, png);
        assert_eq!(r.missing_pictures, 1);
        assert!(
            r.section
                .body
                .contains("<img src=\"images/r0.png\" alt=\"\" width=\"100\"/>")
        );
    }

    #[test]
    fn unicode_skip_and_depth() {
        let r = to_xhtml(br"{\rtf1\uc2 \u1488\'e0\'e0 x\uc1\u-3913 ?}");
        assert!(
            r.section.body.contains("\u{5d0} x\u{f0b7}"),
            "{}",
            r.section.body
        );
        let deep = format!("{{\\rtf1 {}deep{}}}", "{".repeat(2000), "}".repeat(2000));
        assert!(to_xhtml(deep.as_bytes()).plain.contains("deep"));
        assert!(sniff(b"{\\rtf1\\ansi"));
        assert!(!sniff(b"{\\rt"));
    }
}
