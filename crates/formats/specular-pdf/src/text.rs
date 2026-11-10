//! PDF text strings and the document information dictionary.

use hayro::hayro_syntax::metadata::Metadata;

/// Longest information value kept, in characters.
const MAX_CHARS: usize = 4096;

/// The decoded document information dictionary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
}

impl Info {
    pub(crate) fn from_metadata(m: &Metadata) -> Self {
        let get = |v: &Option<Vec<u8>>| v.as_deref().and_then(decode_text_string);
        Self {
            title: get(&m.title),
            author: get(&m.author),
            subject: get(&m.subject),
            keywords: get(&m.keywords),
            creator: get(&m.creator),
            producer: get(&m.producer),
        }
    }

    /// Whether every field is empty.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// An XMP packet carrying these values (`dc:title`, `dc:creator`, `dc:description`,
    /// `dc:subject`, `xmp:CreatorTool`, `pdf:Producer`, `pdf:Keywords`), or `None` when empty.
    pub fn to_xmp(&self) -> Option<Vec<u8>> {
        if self.is_empty() {
            return None;
        }
        let mut s = String::from(
            "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
             <x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
             <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
             <rdf:Description rdf:about=\"\" \
             xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
             xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" \
             xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\">\n",
        );
        let alt = |s: &mut String, name: &str, v: &str| {
            s.push_str(&format!(
                "<{name}><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></{name}>\n",
                escape(v)
            ));
        };
        let list = |s: &mut String, name: &str, kind: &str, items: &[&str]| {
            s.push_str(&format!("<{name}><rdf:{kind}>"));
            for i in items {
                s.push_str(&format!("<rdf:li>{}</rdf:li>", escape(i)));
            }
            s.push_str(&format!("</rdf:{kind}></{name}>\n"));
        };
        let simple = |s: &mut String, name: &str, v: &str| {
            s.push_str(&format!("<{name}>{}</{name}>\n", escape(v)));
        };
        if let Some(v) = &self.title {
            alt(&mut s, "dc:title", v);
        }
        if let Some(v) = &self.author {
            list(&mut s, "dc:creator", "Seq", &[v]);
        }
        if let Some(v) = &self.subject {
            alt(&mut s, "dc:description", v);
        }
        if let Some(v) = &self.keywords {
            let words: Vec<&str> = v
                .split([',', ';'])
                .map(str::trim)
                .filter(|w| !w.is_empty())
                .collect();
            if !words.is_empty() {
                list(&mut s, "dc:subject", "Bag", &words);
            }
            simple(&mut s, "pdf:Keywords", v);
        }
        if let Some(v) = &self.creator {
            simple(&mut s, "xmp:CreatorTool", v);
        }
        if let Some(v) = &self.producer {
            simple(&mut s, "pdf:Producer", v);
        }
        s.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"r\"?>");
        Some(s.into_bytes())
    }
}

fn escape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Decodes a PDF text string (ISO 32000 7.9.2.2): UTF-16BE or UTF-8 with a byte order mark,
/// otherwise PDFDocEncoding. A UTF-16LE mark, seen in the wild, is accepted too. Control
/// characters are dropped, whitespace is trimmed and the result is capped; empty strings are
/// `None`.
pub fn decode_text_string(b: &[u8]) -> Option<String> {
    let raw: String = if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        char::decode_utf16(
            rest.chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]])),
        )
        .map(|c| c.unwrap_or('\u{fffd}'))
        .collect()
    } else if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        char::decode_utf16(
            rest.chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]])),
        )
        .map(|c| c.unwrap_or('\u{fffd}'))
        .collect()
    } else if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest).into_owned()
    } else {
        b.iter().map(|&c| pdf_doc_char(c)).collect()
    };
    let s: String = raw
        .chars()
        // Drop controls (and language escapes' ESC marks) but keep line breaks as spaces.
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .filter(|c| !c.is_control())
        .take(MAX_CHARS)
        .collect();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// PDFDocEncoding (ISO 32000 Annex D): Latin-1 except for 0x18–0x1F and 0x7F–0xA0.
fn pdf_doc_char(c: u8) -> char {
    const LOW: [char; 8] = ['˘', 'ˇ', 'ˆ', '˙', '˝', '˛', '˚', '˜'];
    const HIGH: [char; 34] = [
        '\u{fffd}', '•', '†', '‡', '…', '—', '–', 'ƒ', '⁄', '‹', '›', '−', '‰', '„', '“', '”', '‘',
        '’', '‚', '™', 'ﬁ', 'ﬂ', 'Ł', 'Œ', 'Š', 'Ÿ', 'Ž', 'ı', 'ł', 'œ', 'š', 'ž', '\u{fffd}', '€',
    ];
    match c {
        0x18..=0x1F => LOW[usize::from(c - 0x18)],
        0x7F..=0xA0 => HIGH[usize::from(c - 0x7F)],
        _ => char::from(c),
    }
}
