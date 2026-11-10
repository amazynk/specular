//! `ComicInfo.xml` (ComicRack / Anansi schema v1–v2.1): the fields reading mode needs.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

/// The schema's `Manga` field. `YesAndRightToLeft` is what turns on right-to-left page order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Manga {
    #[default]
    Unknown,
    No,
    Yes,
    YesAndRightToLeft,
}

/// The schema's page type (first one when several are listed).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PageType {
    FrontCover,
    InnerCover,
    Roundup,
    #[default]
    Story,
    Advertisement,
    Editorial,
    Letters,
    Preview,
    BackCover,
    Other,
    /// Marked deleted in ComicRack: hide in reading mode.
    Deleted,
    /// A value outside the schema, kept verbatim.
    Unknown(String),
}

impl PageType {
    fn parse(s: &str) -> PageType {
        let first = s.split_whitespace().next().unwrap_or("");
        match first {
            "FrontCover" => PageType::FrontCover,
            "InnerCover" => PageType::InnerCover,
            "Roundup" => PageType::Roundup,
            "Story" | "" => PageType::Story,
            "Advertisement" => PageType::Advertisement,
            "Editorial" => PageType::Editorial,
            "Letters" => PageType::Letters,
            "Preview" => PageType::Preview,
            "BackCover" => PageType::BackCover,
            "Other" => PageType::Other,
            "Deleted" => PageType::Deleted,
            s => PageType::Unknown(s.to_owned()),
        }
    }
}

/// One `<Page>` element.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageInfo {
    /// Index into the archive's image pages in sorted order ([`crate::Archive::pages`]).
    pub image: u32,
    pub kind: PageType,
    /// The image is already a two-page spread: show it alone, full width.
    pub double_page: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bookmark: Option<String>,
}

/// Reading-mode metadata from `ComicInfo.xml`. Unknown elements are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComicInfo {
    pub title: Option<String>,
    pub series: Option<String>,
    pub number: Option<String>,
    pub volume: Option<i32>,
    pub writer: Option<String>,
    pub page_count: Option<u32>,
    /// `LanguageISO`, e.g. "ja".
    pub language: Option<String>,
    pub manga: Manga,
    pub pages: Vec<PageInfo>,
}

/// What [`crate::Archive::comic_info_lookup`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComicLookup {
    Found(ComicInfo),
    /// No `ComicInfo.xml`, or one too large or not a `ComicInfo` document.
    Absent,
    /// Not read: too costly to reach, or reading it failed.
    Unread,
}

impl ComicLookup {
    pub fn found(self) -> Option<ComicInfo> {
        match self {
            ComicLookup::Found(c) => Some(c),
            _ => None,
        }
    }
}

/// Most `<Page>` elements kept.
const MAX_PAGES: usize = 100_000;
/// Longest text field kept, in bytes.
const MAX_TEXT: usize = 4096;

fn parse_bool(v: &str) -> bool {
    v.trim().eq_ignore_ascii_case("true") || v.trim() == "1"
}

fn page(e: &BytesStart<'_>) -> PageInfo {
    let mut p = PageInfo::default();
    for attr in e.attributes().with_checks(false).flatten() {
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map(|v| v.into_owned())
            .unwrap_or_default();
        match attr.key.local_name().as_ref() {
            b"Image" => p.image = value.trim().parse().unwrap_or(0),
            b"Type" => p.kind = PageType::parse(&value),
            b"DoublePage" => p.double_page = parse_bool(&value),
            b"ImageWidth" => p.width = value.trim().parse().ok(),
            b"ImageHeight" => p.height = value.trim().parse().ok(),
            b"Bookmark" if !value.is_empty() => p.bookmark = Some(truncate(value)),
            _ => {}
        }
    }
    p
}

fn truncate(mut s: String) -> String {
    if s.len() > MAX_TEXT {
        let mut end = MAX_TEXT;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

impl ComicInfo {
    /// Parses the XML; `None` if it is not a `ComicInfo` document. Lenient: a document cut off
    /// or broken half-way keeps what was read before the error.
    pub fn parse(xml: &[u8]) -> Option<ComicInfo> {
        let mut reader = Reader::from_reader(xml);
        reader.config_mut().trim_text(false);
        let mut info = ComicInfo::default();
        let mut path: Vec<Vec<u8>> = Vec::new();
        let mut text = String::new();
        let mut root_seen = false;
        // A read error ends parsing; what was read so far is kept.
        while let Ok(ev) = reader.read_event() {
            match ev {
                Event::Start(e) => {
                    let name = e.local_name().as_ref().to_vec();
                    if path.is_empty() {
                        if name != b"ComicInfo" {
                            return None;
                        }
                        root_seen = true;
                    }
                    if name == b"Page" && path.len() == 2 && info.pages.len() < MAX_PAGES {
                        info.pages.push(page(&e));
                    }
                    if path.len() > 64 {
                        break; // absurd nesting
                    }
                    path.push(name);
                    text.clear();
                }
                Event::Empty(e) => {
                    if path.is_empty() {
                        return (e.local_name().as_ref() == b"ComicInfo").then(ComicInfo::default);
                    }
                    if e.local_name().as_ref() == b"Page"
                        && path.len() == 2
                        && info.pages.len() < MAX_PAGES
                    {
                        info.pages.push(page(&e));
                    }
                }
                Event::Text(t) => {
                    if let Ok(s) = t.decode()
                        && text.len() < MAX_TEXT
                    {
                        text.push_str(&s);
                    }
                }
                Event::CData(t) => {
                    if let Ok(s) = t.decode()
                        && text.len() < MAX_TEXT
                    {
                        text.push_str(&s);
                    }
                }
                Event::GeneralRef(r) => {
                    if let Ok(Some(c)) = r.resolve_char_ref() {
                        text.push(c);
                    } else if let Ok(name) = r.decode()
                        && let Some(v) = quick_xml::escape::resolve_predefined_entity(&name)
                    {
                        text.push_str(v);
                    }
                }
                Event::End(_) => {
                    let name = path.pop().unwrap_or_default();
                    if path.len() == 1 {
                        info.field(&name, truncate(std::mem::take(&mut text)));
                    }
                    text.clear();
                }
                Event::Eof => break,
                _ => {}
            }
        }
        root_seen.then_some(info)
    }

    fn field(&mut self, name: &[u8], v: String) {
        let t = v.trim();
        let opt = || (!t.is_empty()).then(|| t.to_owned());
        match name {
            b"Title" => self.title = opt(),
            b"Series" => self.series = opt(),
            b"Number" => self.number = opt(),
            b"Volume" => self.volume = t.parse().ok(),
            b"Writer" => self.writer = opt(),
            b"PageCount" => self.page_count = t.parse().ok(),
            b"LanguageISO" => self.language = opt(),
            b"Manga" => {
                self.manga = match t {
                    "YesAndRightToLeft" => Manga::YesAndRightToLeft,
                    "Yes" => Manga::Yes,
                    "No" => Manga::No,
                    _ => Manga::Unknown,
                }
            }
            _ => {}
        }
    }

    /// Pages read right to left (`Manga=YesAndRightToLeft`).
    pub fn right_to_left(&self) -> bool {
        self.manga == Manga::YesAndRightToLeft
    }

    /// The `<Page>` entry for image `index`, if listed.
    pub fn page(&self, index: u32) -> Option<&PageInfo> {
        self.pages.iter().find(|p| p.image == index)
    }

    /// Image indices marked as double-page spreads.
    pub fn spreads(&self) -> impl Iterator<Item = u32> + '_ {
        self.pages.iter().filter(|p| p.double_page).map(|p| p.image)
    }
}
