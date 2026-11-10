//! Text-like files (`specular-textdoc`, `specular-subs`): plain text and logs, DOS art
//! (`.nfo`, `.diz`), Markdown, CSV and TSV tables, RTF, and standalone subtitle files (SRT,
//! WebVTT, ASS/SSA, MicroDVD, VobSub, PGS, LRC, TTML/DFXP, SCC).
//!
//! Every one is laid out as pages through the EPUB renderer: Markdown and RTF whole, the
//! others as a preview of their start (the first lines, rows or cues), which thumbnails and
//! printing use. The viewer shows the whole file itself from [`TextModel`]
//! ([`Session::text`]): every line of a log, every row of a table, every cue, from the
//! indexes built at open over the mapped file.

use std::sync::Arc;

use specular_core::{CancelToken, Raster, Source};
use specular_epub::{Book, HtmlDoc, HtmlSection, html_escape as esc};
use specular_subs::{Body, FileKind, Track};
use specular_textdoc::{TableFile, TextFile};

use crate::{
    ContentsEntry, DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, PageLabel,
    Result, Session,
};

/// The kinds read here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Nfo,
    Markdown,
    Csv,
    Tsv,
    Rtf,
    SubRip,
    WebVtt,
    Ssa,
    MicroDvd,
    VobSub,
    Pgs,
    Lrc,
    Ttml,
    Scc,
}

/// What the viewer shows of a text-like file.
#[derive(Clone, Debug)]
pub enum TextModel {
    /// Plain text, a log or DOS art (`dos`: code page 437 art, not wrapped).
    Plain {
        file: Arc<TextFile>,
        dos: bool,
    },
    /// Markdown: its source; the session's pages are the rendered document.
    Markdown(Arc<TextFile>),
    Table(Arc<TableFile>),
    Subtitles(Arc<SubtitleModel>),
}

/// A subtitle file's cues.
#[derive(Clone, Debug)]
pub struct SubtitleModel {
    pub track: Track,
    /// "SubRip", "WebVTT"…
    pub format: &'static str,
    /// The character set of a text format.
    pub charset: Option<&'static str>,
}

pub struct TextDecoder(pub Kind);

/// The source's bytes, mapped or in memory, for the indexes.
struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl Kind {
    fn format(self) -> Format {
        match self {
            Kind::Plain => Format::PlainText,
            Kind::Nfo => Format::Nfo,
            Kind::Markdown => Format::Markdown,
            Kind::Csv => Format::Csv,
            Kind::Tsv => Format::Tsv,
            Kind::Rtf => Format::Rtf,
            Kind::SubRip => Format::SubRip,
            Kind::WebVtt => Format::WebVtt,
            Kind::Ssa => Format::Ssa,
            Kind::MicroDvd => Format::MicroDvd,
            Kind::VobSub => Format::VobSub,
            Kind::Pgs => Format::Pgs,
            Kind::Lrc => Format::Lrc,
            Kind::Ttml => Format::Ttml,
            Kind::Scc => Format::Scc,
        }
    }

    fn subtitle_kind(self) -> Option<FileKind> {
        Some(match self {
            Kind::SubRip => FileKind::Srt,
            Kind::WebVtt => FileKind::Vtt,
            Kind::Ssa => FileKind::Ass,
            Kind::MicroDvd => FileKind::Sub,
            Kind::Lrc => FileKind::Lrc,
            Kind::Ttml => FileKind::Ttml,
            Kind::Scc => FileKind::Scc,
            _ => return None,
        })
    }
}

/// The text of the start of a file, for content checks.
fn head_str(head: &[u8], n: usize) -> std::borrow::Cow<'_, str> {
    let h = &head[..head.len().min(n)];
    let h = h.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(h);
    String::from_utf8_lossy(h)
}

/// A PGS segment header at `at`: "PG", times, a known type and a size.
fn pgs_segment(d: &[u8], at: usize) -> Option<usize> {
    let s = d.get(at..at + 13)?;
    (s.starts_with(b"PG") && matches!(s[10], 0x14..=0x18 | 0x80))
        .then(|| at + 13 + usize::from(u16::from_be_bytes([s[11], s[12]])))
}

impl Decoder for TextDecoder {
    fn format(&self) -> Format {
        self.0.format()
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Rtf => specular_textdoc::rtf::sniff(head),
            Kind::WebVtt => head_str(head, 64).starts_with("WEBVTT"),
            Kind::Scc => specular_subs::scc::sniff(&head_str(head, 64)),
            Kind::VobSub => head_str(head, 64).starts_with("# VobSub index file"),
            Kind::Ssa => {
                let h = head_str(head, 64);
                h.trim_start().starts_with("[Script Info]")
            }
            // Two PGS segments in a row.
            Kind::Pgs => pgs_segment(head, 0)
                .is_some_and(|n| n + 13 > head.len() || pgs_segment(head, n).is_some()),
            _ => false,
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            // SubRip: a number line, then a time line.
            Kind::SubRip => {
                let h = head_str(data, 512);
                let mut lines = h.lines().map(str::trim).filter(|l| !l.is_empty());
                lines
                    .next()
                    .is_some_and(|l| l.chars().all(|c| c.is_ascii_digit()))
                    && lines.next().is_some_and(|l| {
                        l.contains("-->") && l.as_bytes().first().is_some_and(u8::is_ascii_digit)
                    })
            }
            Kind::MicroDvd => specular_subs::microdvd::sniff(&head_str(data, 512)),
            Kind::Ttml => {
                specular_subs::ttml::sniff(&head_str(data, 4096))
                    && head_str(data, 4096).contains("<tt")
            }
            _ => self.probe(&data[..data.len().min(crate::PROBE_HEAD)]),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Plain => &["txt", "text", "log"],
            Kind::Nfo => &["nfo", "diz"],
            Kind::Markdown => &["md", "markdown", "mdown", "mkd"],
            Kind::Csv => &["csv"],
            Kind::Tsv => &["tsv"],
            Kind::Rtf => &["rtf"],
            Kind::SubRip => &["srt"],
            Kind::WebVtt => &["vtt", "webvtt"],
            Kind::Ssa => &["ass", "ssa"],
            // A VobSub `.sub` is opened through its `.idx` (`vobsub_by_name`).
            Kind::MicroDvd => &["sub"],
            Kind::VobSub => &["idx"],
            Kind::Pgs => &["sup"],
            Kind::Lrc => &["lrc"],
            Kind::Ttml => &["ttml", "dfxp"],
            Kind::Scc => &["scc"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        match self.0 {
            Kind::Plain | Kind::Nfo => open_plain(source, self.0),
            Kind::Markdown => open_markdown(source),
            Kind::Csv | Kind::Tsv => open_table(source, self.0),
            Kind::Rtf => open_rtf(source),
            k => open_subtitles(source, k),
        }
    }
}

/// Every kind read here.
const KINDS: [Kind; 15] = [
    Kind::Plain,
    Kind::Nfo,
    Kind::Markdown,
    Kind::Csv,
    Kind::Tsv,
    Kind::Rtf,
    Kind::SubRip,
    Kind::WebVtt,
    Kind::Ssa,
    Kind::MicroDvd,
    Kind::VobSub,
    Kind::Pgs,
    Kind::Lrc,
    Kind::Ttml,
    Kind::Scc,
];

/// Whether `path` is named as one of the text-like files read here (text, tables,
/// Markdown, RTF, subtitles), by its extension; a Bruker SkyScan `.log` is a volume, not text.
/// Stepping through a folder, slideshows and play queues keep text files and pictures apart.
pub fn is_text_file(path: &std::path::Path) -> bool {
    let Some(ext) = crate::path_extensions(path)[1].clone() else {
        return false;
    };
    (KINDS
        .iter()
        .any(|k| TextDecoder(*k).extensions().contains(&ext.as_str()))
        || crate::notebook::NotebookDecoder
            .extensions()
            .contains(&ext.as_str()))
        && !(ext == "log" && crate::sniff::is_skyscan_log(path))
        && !crate::sniff::is_msx_scc(path)
}

/// A `.sub` that is VobSub pictures (a program stream with an `.idx` beside it): opened
/// through the pair. Checked before the content probes (MPEG-PS would take it as video).
pub(crate) fn is_vobsub_sub(source: &Source) -> bool {
    crate::path_extensions(source.path())[1].as_deref() == Some("sub")
        && specular_subs::is_vobsub_data(source.bytes())
        && crate::geo::on_disk(source)
}

fn map_text_err(e: specular_textdoc::Error) -> DecodeError {
    match e {
        specular_textdoc::Error::Binary => {
            DecodeError::Unsupported("the file holds binary data, not text".into())
        }
        specular_textdoc::Error::TooLarge(s) => DecodeError::Unsupported(format!("too large: {s}")),
        specular_textdoc::Error::Cancelled => DecodeError::Malformed("cancelled".into()),
    }
}

fn map_epub_err(e: specular_epub::Error) -> DecodeError {
    match e {
        specular_epub::Error::TooLarge(s) => DecodeError::Unsupported(format!("too large: {s}")),
        e => DecodeError::Malformed(e.to_string()),
    }
}

fn file_title(source: &Source) -> String {
    source
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Lays `doc` out as pages and wraps the session with the text model and facts.
pub(crate) fn pages(
    source: Arc<Source>,
    format: Format,
    doc: &HtmlDoc,
    model: Option<TextModel>,
    coding: String,
    facts: HeaderFacts,
    search: Option<String>,
) -> Result<Arc<dyn Session>> {
    let book = Book::open_html(doc, &Default::default()).map_err(map_epub_err)?;
    let inner = crate::epub_dec::open_book(book, source.clone(), format)?;
    let shared = Arc::new(Shared {
        model,
        facts,
        coding,
        search,
        source,
    });
    Ok(wrap(shared, inner))
}

fn count(n: u64) -> String {
    n.to_string()
}

fn open_plain(source: Arc<Source>, kind: Kind) -> Result<Arc<dyn Session>> {
    let dos = kind == Kind::Nfo;
    let file = TextFile::open(Arc::new(SourceBytes(source.clone())), dos, &|| false)
        .map_err(map_text_err)?;
    let doc = specular_textdoc::preview::text(&file, &file_title(&source), dos);
    let lines = file.index.len();
    let coding = format!(
        "{} text, {} line{}",
        file.charset.name(),
        lines,
        if lines == 1 { "" } else { "s" }
    );
    let mut summary = vec![
        ("encoding", file.charset.name().to_owned()),
        ("lines", count(lines)),
    ];
    let ends = file.index.endings.name();
    if !ends.is_empty() {
        summary.push(("line-ends", ends));
    }
    let search = Some(file.head_text(SEARCH_BYTES));
    let file = Arc::new(file);
    pages(
        source,
        kind.format(),
        &doc,
        Some(TextModel::Plain { file, dos }),
        coding,
        HeaderFacts {
            summary,
            fields: Vec::new(),
        },
        search,
    )
}

/// Bytes of text given to the Library's search.
const SEARCH_BYTES: usize = 256 << 10;
/// Largest Markdown or RTF file laid out.
const MAX_DOCUMENT: usize = 64 << 20;
/// Largest picture file a Markdown document loads.
const MAX_PICTURE_FILE: u64 = 64 << 20;

fn open_markdown(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    if source.len() > MAX_DOCUMENT {
        return Err(DecodeError::Unsupported(format!(
            "Markdown files over {} MB are not laid out",
            MAX_DOCUMENT >> 20
        )));
    }
    let file = TextFile::open(Arc::new(SourceBytes(source.clone())), false, &|| false)
        .map_err(map_text_err)?;
    let text = file.head_text(usize::MAX);
    // Pictures next to the file, by relative path; never from the network.
    let dir = crate::geo::on_disk(&source)
        .then(|| source.path().parent().map(std::path::Path::to_path_buf))
        .flatten();
    let load = |rel: &str| -> Option<Vec<u8>> {
        let dir = dir.as_ref()?;
        let p = dir.join(rel);
        let meta = std::fs::metadata(&p).ok()?;
        if !meta.is_file() || meta.len() > MAX_PICTURE_FILE {
            return None;
        }
        let bytes = std::fs::read(&p).ok()?;
        picture_for_book(bytes, &p, &source)
    };
    let r = specular_textdoc::markdown::to_xhtml(&text, &load);
    let lines = file.index.len();
    let title = r.title.clone().unwrap_or_else(|| file_title(&source));
    let doc = HtmlDoc {
        title: title.clone(),
        css: specular_textdoc::markdown::CSS.to_owned(),
        sections: vec![r.section],
        pictures: r.pictures.clone(),
        ..HtmlDoc::default()
    };
    let mut summary = vec![
        ("encoding", file.charset.name().to_owned()),
        ("lines", count(lines)),
    ];
    if !r.pictures.is_empty() {
        summary.push(("pictures", count(r.pictures.len() as u64)));
    }
    if r.missing_pictures > 0 {
        summary.push(("pictures-not-shown", count(r.missing_pictures as u64)));
    }
    let coding = format!("Markdown, {} text", file.charset.name());
    let search = Some(file.head_text(SEARCH_BYTES));
    pages(
        source,
        Format::Markdown,
        &doc,
        Some(TextModel::Markdown(Arc::new(file))),
        coding,
        HeaderFacts {
            summary,
            fields: Vec::new(),
        },
        search,
    )
}

/// A picture as the EPUB renderer reads it (PNG, JPEG, GIF, BMP, WebP, SVG); other formats
/// we decode (AVIF, JPEG XL, TIFF…) are converted to PNG.
fn picture_for_book(bytes: Vec<u8>, path: &std::path::Path, within: &Source) -> Option<Vec<u8>> {
    let b = &bytes;
    let native = b.starts_with(&[0xFF, 0xD8, 0xFF])
        || b.starts_with(b"\x89PNG\r\n\x1a\n")
        || b.starts_with(b"GIF8")
        || (b.starts_with(b"BM") && b.len() > 26)
        || (b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP");
    if native {
        return Some(bytes);
    }
    let name = path.file_name()?.to_string_lossy().into_owned();
    let s = crate::svg::registry()
        .open(Arc::new(Source::from_vec(name, bytes).within(within)))
        .ok()?;
    let (w, h) = s.info().display_size();
    if u64::from(w) * u64::from(h) > 64 << 20 {
        return None;
    }
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .ok()?;
    let (rw, rh, rgba) = crate::svg::to_rgba8(r)?;
    specular_png::encode(rw, rh, specular_png::ColorType::Rgba, &rgba).ok()
}

fn open_rtf(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    if source.len() > MAX_DOCUMENT * 4 {
        return Err(DecodeError::Unsupported(format!(
            "RTF files over {} MB are not laid out",
            (MAX_DOCUMENT * 4) >> 20
        )));
    }
    let r = specular_textdoc::rtf::to_xhtml(source.bytes());
    let title = r.title.clone().unwrap_or_else(|| file_title(&source));
    let doc = HtmlDoc {
        title: title.clone(),
        authors: r.author.iter().cloned().collect(),
        css: specular_textdoc::rtf::CSS.to_owned(),
        sections: vec![r.section],
        pictures: r.pictures,
        ..HtmlDoc::default()
    };
    let words = r.plain.split_whitespace().count() as u64;
    let mut summary = vec![("words", count(words))];
    let pictures = doc.pictures.len() as u64;
    if pictures > 0 {
        summary.push(("pictures", count(pictures)));
    }
    if r.missing_pictures > 0 {
        summary.push(("pictures-not-shown", count(r.missing_pictures as u64)));
    }
    if r.tables > 0 {
        summary.push(("tables", count(r.tables as u64)));
    }
    // The `\info` fields by RTF's own names.
    let mut fields = r
        .title
        .iter()
        .map(|t| ("title".to_owned(), t.clone()))
        .collect::<Vec<_>>();
    if let Some(a) = r.author {
        fields.push(("author".into(), a));
    }
    let plain = r.plain;
    let search = Some(plain.chars().take(SEARCH_BYTES).collect());
    pages(
        source,
        Format::Rtf,
        &doc,
        None,
        "Rich Text Format".into(),
        HeaderFacts { summary, fields },
        search,
    )
}

fn open_table(source: Arc<Source>, kind: Kind) -> Result<Arc<dyn Session>> {
    let delimiter = (kind == Kind::Tsv).then_some(b'\t');
    let file = TableFile::open(Arc::new(SourceBytes(source.clone())), delimiter, &|| false)
        .map_err(map_text_err)?;
    let doc = specular_textdoc::preview::table(&file, &file_title(&source));
    let t = &file.table;
    let rows = t.len();
    let coding = format!(
        "{} table, {} row{}, {} column{}",
        file.charset.name(),
        rows,
        if rows == 1 { "" } else { "s" },
        t.columns,
        if t.columns == 1 { "" } else { "s" }
    );
    let mut summary = vec![
        ("encoding", file.charset.name().to_owned()),
        ("rows", count(rows)),
        ("columns", count(t.columns as u64)),
        ("delimiter", file.delimiter_symbol()),
    ];
    // The column names, when the first row gives them.
    if t.header
        && let Some(first) = file.rows(0, 1).into_iter().next()
    {
        let names: String = first.join(", ").chars().take(300).collect();
        summary.push(("column-names", names));
    }
    let file = Arc::new(file);
    pages(
        source,
        kind.format(),
        &doc,
        Some(TextModel::Table(file)),
        coding,
        HeaderFacts {
            summary,
            fields: Vec::new(),
        },
        None,
    )
}

/// Cues in a subtitle file's preview pages.
const PREVIEW_CUES: usize = 300;
/// Picture cues encoded for the preview.
const PREVIEW_PICTURES: usize = 60;

/// `h:mm:ss.mmm`.
pub fn cue_time(us: i64) -> String {
    let us = us.max(0);
    let ms = us / 1000;
    format!(
        "{}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

fn subtitle_name(k: Kind) -> &'static str {
    match k {
        Kind::SubRip => "SubRip",
        Kind::WebVtt => "WebVTT",
        Kind::Ssa => "SubStation Alpha",
        Kind::MicroDvd => "MicroDVD",
        Kind::VobSub => "VobSub",
        Kind::Pgs => "PGS",
        Kind::Lrc => "LRC lyrics",
        Kind::Ttml => "TTML",
        Kind::Scc => "Scenarist closed captions",
        _ => "text",
    }
}

fn read_track(
    source: &Source,
    kind: Kind,
) -> Result<(
    Track,
    Option<&'static str>,
    Option<specular_subs::lrc::Tags>,
)> {
    let err = |e: specular_subs::Error| match e {
        specular_subs::Error::Unsupported(s) => DecodeError::Unsupported(s),
        specular_subs::Error::Malformed(s) => DecodeError::Malformed(s),
        specular_subs::Error::Io(s) => DecodeError::Malformed(s),
    };
    let data = source.bytes();
    match kind {
        Kind::Pgs => Ok((specular_subs::pgs::from_sup(data).map_err(err)?, None, None)),
        Kind::VobSub => {
            // The `.idx` opened, or the `.sub` beside it.
            let on_disk = crate::geo::on_disk(source);
            let (idx, sub) = if specular_subs::is_vobsub_data(data) {
                let idx = on_disk
                    .then(|| specular_subs::idx_of(source.path()))
                    .flatten()
                    .ok_or_else(|| {
                        DecodeError::Unsupported(
                            "a VobSub .sub needs the .idx file beside it".into(),
                        )
                    })?;
                (
                    std::fs::read(idx).map_err(|e| DecodeError::Malformed(e.to_string()))?,
                    data.to_vec(),
                )
            } else {
                let sub = on_disk
                    .then(|| {
                        ["sub", "SUB"]
                            .iter()
                            .map(|e| source.path().with_extension(e))
                            .find(|p| p.is_file())
                    })
                    .flatten()
                    .ok_or_else(|| {
                        DecodeError::Unsupported(
                            "a VobSub index needs the .sub file beside it".into(),
                        )
                    })?;
                (
                    data.to_vec(),
                    std::fs::read(sub).map_err(|e| DecodeError::Malformed(e.to_string()))?,
                )
            };
            let (text, _) = specular_subs::charset::decode(&idx, None, None);
            Ok((
                specular_subs::vobsub::from_files(&text, &sub).map_err(err)?,
                None,
                None,
            ))
        }
        k => {
            let fk = k.subtitle_kind().ok_or(DecodeError::UnsupportedFormat)?;
            let (text, enc) = specular_subs::charset::decode(data, None, None);
            let tags = (k == Kind::Lrc)
                .then(|| specular_subs::lrc::parse(&text).ok().map(|(_, t)| t))
                .flatten();
            let track = specular_subs::parse_text(fk, &text).map_err(err)?;
            Ok((track, Some(enc.name()), tags))
        }
    }
}

/// A text cue as XHTML lines (bold, italic and underline kept; colours are for the video
/// and left out on the page).
fn cue_xhtml(c: &specular_subs::TextCue) -> String {
    let mut out = String::new();
    for (i, line) in c.lines.iter().enumerate() {
        if i > 0 {
            out.push_str("<br/>");
        }
        for s in line {
            let mut t = esc(&s.text);
            if s.bold {
                t = format!("<b>{t}</b>");
            }
            if s.italic {
                t = format!("<i>{t}</i>");
            }
            if s.underline {
                t = format!("<u>{t}</u>");
            }
            out.push_str(&t);
        }
    }
    out
}

const CUES_CSS: &str = "body{margin:0}\ntable{border-collapse:collapse;font-size:9pt;width:100%}\ntd{border-bottom:1px solid #ddd;padding:2px 6px;vertical-align:top}\ntd.t{white-space:nowrap;color:#666;font-family:monospace}\n";

fn open_subtitles(source: Arc<Source>, kind: Kind) -> Result<Arc<dyn Session>> {
    let (track, charset, tags) = read_track(&source, kind)?;
    if track.cues.is_empty() {
        return Err(DecodeError::Malformed("the file holds no subtitles".into()));
    }
    let mut body = String::from("<table>\n");
    let mut pictures = Vec::new();
    for c in track.cues.iter().take(PREVIEW_CUES) {
        body.push_str(&format!(
            "<tr><td class=\"t\">{}</td><td>",
            cue_time(c.start_us)
        ));
        match &c.body {
            Body::Text(t) => body.push_str(&cue_xhtml(t)),
            Body::Bitmap(b) if pictures.len() < PREVIEW_PICTURES => {
                if let Ok(png) =
                    specular_png::encode(b.w, b.h, specular_png::ColorType::Rgba, &b.rgba)
                {
                    let name = format!("c{}.png", pictures.len());
                    body.push_str(&format!("<img src=\"images/{name}\" alt=\"\"/>"));
                    pictures.push((name, png));
                }
            }
            Body::Bitmap(_) => {}
        }
        body.push_str("</td></tr>\n");
    }
    body.push_str("</table>");
    let name = subtitle_name(kind);
    let doc = HtmlDoc {
        title: file_title(&source),
        css: CUES_CSS.to_owned(),
        sections: vec![HtmlSection {
            body,
            headings: Vec::new(),
        }],
        pictures,
        ..HtmlDoc::default()
    };
    let n = track.cues.len() as u64;
    let first = track.cues.first().map_or(0, |c| c.start_us);
    let last = track.cues.iter().map(|c| c.end_us).max().unwrap_or(0);
    let mut summary = vec![
        ("cues", count(n)),
        (
            "cue-span",
            format!("{} – {}", cue_time(first), cue_time(last)),
        ),
    ];
    if let Some(c) = charset {
        summary.insert(0, ("encoding", c.to_owned()));
    }
    let mut fields = Vec::new();
    if let Some(t) = tags {
        // The ID tags by LRC's own names.
        for (k, v) in [("ti", t.title), ("ar", t.artist), ("al", t.album)] {
            if let Some(v) = v {
                fields.push((k.to_owned(), v));
            }
        }
    }
    let search: String = track
        .cues
        .iter()
        .filter_map(|c| c.text().map(specular_subs::TextCue::plain_text))
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(SEARCH_BYTES)
        .collect();
    let coding = format!("{name}, {n} cue{}", if n == 1 { "" } else { "s" });
    let model = SubtitleModel {
        track,
        format: name,
        charset,
    };
    pages(
        source,
        kind.format(),
        &doc,
        Some(TextModel::Subtitles(Arc::new(model))),
        coding,
        HeaderFacts { summary, fields },
        (!search.is_empty()).then_some(search),
    )
}

struct Shared {
    model: Option<TextModel>,
    facts: HeaderFacts,
    coding: String,
    search: Option<String>,
    source: Arc<Source>,
}

fn wrap(shared: Arc<Shared>, inner: Arc<dyn Session>) -> Arc<dyn Session> {
    let mut info = inner.info().clone();
    info.coding = shared.coding.clone();
    let mut facts = shared.facts.clone();
    let pages = inner.page_count();
    if pages > 0
        && shared
            .model
            .as_ref()
            .is_none_or(|m| matches!(m, TextModel::Markdown(_)))
    {
        facts.summary.push(("pages", pages.to_string()));
    }
    Arc::new(TextSession {
        shared,
        inner,
        info,
        facts,
    })
}

struct TextSession {
    shared: Arc<Shared>,
    inner: Arc<dyn Session>,
    info: ImageInfo,
    facts: HeaderFacts,
}

impl Session for TextSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn page_count(&self) -> u32 {
        self.inner.page_count()
    }
    fn page_index(&self) -> u32 {
        self.inner.page_index()
    }
    fn page_label(&self) -> Option<PageLabel> {
        self.inner.page_label()
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(wrap(self.shared.clone(), self.inner.open_page(page)?))
    }
    fn contents(&self) -> Option<Arc<[ContentsEntry]>> {
        self.inner.contents()
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.facts)
    }
    fn text(&self) -> Option<&TextModel> {
        self.shared.model.as_ref()
    }
    fn search_text(&self) -> Option<String> {
        self.shared.search.clone()
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        self.inner.alpha()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    fn open(name: &str, data: &[u8]) -> Result<Arc<dyn Session>> {
        Registry::default().open(Arc::new(Source::from_vec(name, data.to_vec())))
    }

    fn fact<'a>(s: &'a dyn Session, k: &str) -> Option<&'a str> {
        s.header()?
            .summary
            .iter()
            .find(|(key, _)| *key == k)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn plain_text_and_logs() {
        let s = open("notes.txt", "Hello\nשלום\n".as_bytes()).unwrap();
        assert_eq!(s.info().format, Format::PlainText);
        assert_eq!(fact(s.as_ref(), "lines"), Some("2"));
        assert_eq!(fact(s.as_ref(), "encoding"), Some("UTF-8"));
        let Some(TextModel::Plain { file, dos: false }) = s.text() else {
            panic!("no text model")
        };
        assert_eq!(file.lines(1, 1), ["שלום"]);
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert!(r.width > 0);
        let s = open("server.log", b"2026-01-01 start\r\n2026-01-01 stop\r\n").unwrap();
        assert_eq!(s.info().format, Format::PlainText);
        assert_eq!(fact(s.as_ref(), "line-ends"), Some("CRLF"));
        let s = open("art.nfo", b"\xDB\xDB\xB2\xB1\xB0 crew\n").unwrap();
        assert_eq!(s.info().format, Format::Nfo);
        assert_eq!(fact(s.as_ref(), "encoding"), Some("CP437 (DOS)"));
        let Some(TextModel::Plain { file, dos: true }) = s.text() else {
            panic!("no DOS model")
        };
        assert_eq!(file.lines(0, 1), ["██▓▒░ crew"]);
        let e = open("data.txt", &[0, 1, 2, 3, 0xFF, 0, 0, 9, 8, 7])
            .err()
            .unwrap();
        assert!(e.to_string().contains("binary"), "{e}");
    }

    #[test]
    fn markdown_rtf_and_tables() {
        let s = open("README.md", b"# Read me\n\nSome *text*.\n\n## Part\n").unwrap();
        assert_eq!(s.info().format, Format::Markdown);
        assert!(matches!(s.text(), Some(TextModel::Markdown(_))));
        assert_eq!(s.contents().map(|c| c.len()), Some(2));
        assert!(fact(s.as_ref(), "pages").is_some());
        let s = open(
            "letter.rtf",
            br"{\rtf1\ansi{\info{\title Hi}}\b Bold\b0  words\par}",
        )
        .unwrap();
        assert_eq!(s.info().format, Format::Rtf);
        assert!(s.text().is_none());
        assert_eq!(fact(s.as_ref(), "words"), Some("2"));
        // RTF by content whatever its name.
        assert_eq!(
            open("x.doc", br"{\rtf1 x}").unwrap().info().format,
            Format::Rtf
        );
        let s = open("t.csv", b"city;pop\nA;1\nB;2\n").unwrap();
        assert_eq!(s.info().format, Format::Csv);
        assert_eq!(fact(s.as_ref(), "rows"), Some("3"));
        assert_eq!(fact(s.as_ref(), "delimiter"), Some(";"));
        assert_eq!(fact(s.as_ref(), "column-names"), Some("city, pop"));
        let s = open("t.tsv", b"a,b\tc\n").unwrap();
        assert_eq!(s.info().format, Format::Tsv);
        assert_eq!(fact(s.as_ref(), "columns"), Some("2"));
    }

    #[test]
    fn subtitle_files() {
        let srt =
            b"1\n00:00:01,000 --> 00:00:02,000\nHello\n\n2\n00:00:03,000 --> 00:00:04,500\nWorld\n";
        let s = open("movie.srt", srt).unwrap();
        assert_eq!(s.info().format, Format::SubRip);
        assert_eq!(fact(s.as_ref(), "cues"), Some("2"));
        assert_eq!(
            fact(s.as_ref(), "cue-span"),
            Some("0:00:01.000 – 0:00:04.500")
        );
        assert_eq!(s.search_text().as_deref(), Some("Hello\nWorld"));
        // SubRip without its extension, by content.
        assert_eq!(
            open("movie.txt", srt).unwrap().info().format,
            Format::SubRip
        );
        let cases: [(&str, &[u8], Format); 6] = [
            ("a.vtt", b"WEBVTT\n\n00:01.000 --> 00:02.000\nHi\n", Format::WebVtt),
            ("a.ssa", b"[Script Info]\nScriptType: v4.00+\n\n[Events]\nFormat: Layer, Start, End, Style, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,Hi\n", Format::Ssa),
            ("a.sub", b"{1}{1}25\n{25}{50}Hi|there\n", Format::MicroDvd),
            ("a.lrc", b"[ti:Song]\n[00:01.00]La la\n[00:03.00]Lo\n", Format::Lrc),
            ("a.dfxp", b"<?xml version=\"1.0\"?>\n<tt xmlns=\"http://www.w3.org/ns/ttml\"><body><div><p begin=\"1s\" end=\"2s\">Hi</p></div></body></tt>", Format::Ttml),
            ("a.scc", b"Scenarist_SCC V1.0\n\n00:00:00:00\t9420 9420 94d0 94d0 c8e9 942f 942f\n\n00:00:02:00\t942c 942c\n", Format::Scc),
        ];
        for (name, data, f) in cases {
            let s = open(name, data).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.info().format, f, "{name}");
            let Some(TextModel::Subtitles(m)) = s.text() else {
                panic!("{name}: no cues")
            };
            assert!(!m.track.cues.is_empty(), "{name}");
        }
        let s = open("a.lrc", b"[ti:Song]\n[00:01.00]La la\n").unwrap();
        assert_eq!(
            s.header().unwrap().fields[0],
            ("ti".to_owned(), "Song".to_owned())
        );
        assert!(open("empty.srt", b"nothing here").is_err());
    }

    #[test]
    fn pgs_and_vobsub() {
        use specular_subs::pgs::fixture as pf;
        let sup = pf::sup(&[
            (90_000, pf::show(720, 480, 10, 400, 40, 20)),
            (180_000, pf::clear(720, 480)),
        ]);
        let s = open("a.sup", &sup).unwrap();
        assert_eq!(s.info().format, Format::Pgs);
        assert_eq!(fact(s.as_ref(), "cues"), Some("1"));
        let Some(TextModel::Subtitles(m)) = s.text() else {
            panic!("no cues")
        };
        assert!(m.track.is_bitmap());
        // A VobSub index alone, without its pictures, says what is missing.
        let e = open(
            "a.idx",
            b"# VobSub index file, v7 (do not modify this line!)\nsize: 720x480\n",
        )
        .err()
        .unwrap();
        assert!(e.to_string().contains(".sub"), "{e}");
    }

    #[test]
    fn text_files_by_name() {
        use std::path::Path;
        for n in [
            "a.txt", "B.LOG", "x.md", "t.csv", "r.rtf", "s.srt", "v.idx", "p.sup", "c.scc",
        ] {
            assert!(is_text_file(Path::new(n)), "{n}");
        }
        for n in ["a.jpg", "a.wav", "a", "notes.zzq", "a.tab"] {
            assert!(!is_text_file(Path::new(n)), "{n}");
        }
        // Every extension of every text decoder counts.
        let reg = Registry::default();
        for e in reg.extensions() {
            // A shared extension (`.scc`: captions or an MSX screen) counts when a text
            // decoder claims it; the file's content settles it.
            let text = reg
                .decoders
                .iter()
                .filter(|d| d.extensions().contains(&e))
                .any(|d| d.format().is_text());
            assert_eq!(is_text_file(Path::new(&format!("f.{e}"))), text, ".{e}");
        }
    }

    #[test]
    fn cue_times() {
        assert_eq!(cue_time(3_723_456_789), "1:02:03.456");
        assert_eq!(cue_time(-5), "0:00:00.000");
    }
}
